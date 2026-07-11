//! Route 3 — surface met → `(A, B, C)` via Monin–Obukhov reconstruction + a
//! least-squares fit of the log-lin profile (MET-06, D-08).
//!
//! The general route: from surface meteorology (wind at the anemometer, a
//! stability proxy, ground temperature and its gradient) reconstruct the
//! vertical wind and temperature profiles `u(z)`, `T(z)` via Monin–Obukhov
//! similarity, form the effective sound-speed profile
//! `c_eff(z) = 20.05·√(T(z)+273.15) + u(z)·cos(az − φ)`, then **fit** the
//! engine's log-lin model `c_eff(z) ≈ A·ln(z/z₀+1) + B·z + C` by least squares.
//!
//! # I/O quarantine (D-15) + no linalg crate (D-08)
//!
//! The Route-3 reconstruction lives in `envi-harness`. The 3-parameter
//! least-squares solve is a **hand-rolled 3×3 normal-equations** system
//! (`XᵀX β = Xᵀy`) solved closed-form by Cramer's rule — **no `nalgebra` /
//! `ndarray-linalg`** (a checkpoint guards any linalg-crate proposal). As of
//! 09-03 (METX-01) that LSQ was **lifted** into the WASM-safe
//! [`envi_gis::weather::fit_profile`] (single source of truth); [`fit_profile`]
//! here delegates to it and maps the error back to [`CaseLoadError`]. The
//! singular case is a typed error, never a panic or a NaN profile (T-03-03-02).
//!
//! # ⚠️ The stability constants are `[ASSUMED]` (RESEARCH Open Q1, D-04)
//!
//! AV 1106/07 does not define the surface-met → profile reconstruction. The
//! Monin–Obukhov stability functions and the cloud-cover → Obukhov-length
//! mapping are `[ASSUMED]`; the reconstruction is validated by **structural /
//! direction property tests** (monotonic `u(z)`; a warmer surface raises `c₀`)
//! and the fit by a **round-trip identity** (a synthetic profile generated from
//! a known `(A, B, C)` is recovered to tolerance) — never a FORCE numeric Pass.

use crate::cases::CaseLoadError;
use envi_engine::propagation::refraction::profile::Z0_MIN_M;
use envi_engine::propagation::sound_speed_ms;

/// Absolute-zero offset for the Kelvin conversion (Eq. 335).
const KELVIN_OFFSET: f64 = 273.15;
/// `[ASSUMED]` stable-stratification coefficient of the MO Ψ_m function
/// (Businger–Dyer `Ψ_m = −β·z/L` for `z/L > 0`).
const PSI_M_STABLE_BETA: f64 = 5.0;

/// Surface meteorology for the Monin–Obukhov reconstruction (Route 3 input).
///
/// All fields are physical surface observations. `azimuth_deg` is the
/// propagation bearing (clockwise from north); `phi_deg` the downwind bearing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceMet {
    /// Wind speed at the anemometer height, m/s.
    pub u_zu: f64,
    /// Anemometer height `zu`, m.
    pub zu: f64,
    /// Roughness length `z₀`, m (clamped ≥ 0.001 m).
    pub z0: f64,
    /// Downwind bearing `φ_u`, degrees clockwise from north.
    pub phi_deg: f64,
    /// Propagation bearing, degrees clockwise from north.
    pub azimuth_deg: f64,
    /// Ground air temperature `t₀`, °C.
    pub t0_c: f64,
    /// Temperature gradient `dt/dz`, °C/m (inversion `> 0`, lapse `< 0`).
    pub dtdz: f64,
    /// Inverse Obukhov length `1/L`, m⁻¹ (`[ASSUMED]` stability proxy; `0` =
    /// neutral). Positive = stable, negative = unstable.
    pub inv_obukhov: f64,
}

/// Default reconstruction heights (m): a log-spaced set from near the ground to
/// ~50 m, dense low where the profile curves most.
#[must_use]
pub fn default_heights() -> Vec<f64> {
    vec![0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 50.0]
}

/// The `[ASSUMED]` Businger–Dyer momentum stability function `Ψ_m(ζ)`, `ζ=z/L`.
///
/// Stable (`ζ ≥ 0`): `Ψ_m = −β·ζ`. Unstable (`ζ < 0`): the Dyer–Hicks form with
/// `x = (1 − 16ζ)^{1/4}`. Both are standard MO closures (constants `[ASSUMED]`).
fn psi_m(zeta: f64) -> f64 {
    if zeta >= 0.0 {
        -PSI_M_STABLE_BETA * zeta
    } else {
        let x = (1.0 - 16.0 * zeta).powf(0.25);
        2.0 * ((1.0 + x) / 2.0).ln() + ((1.0 + x * x) / 2.0).ln() - 2.0 * x.atan()
            + std::f64::consts::FRAC_PI_2
    }
}

/// Reconstruct the effective sound-speed profile `c_eff(z)` at `heights` from
/// surface met via Monin–Obukhov similarity (MET-06).
///
/// `u(z) = (u*/κ)·[ln(z/z₀+1) − Ψ_m(z/L)]`, `T(z) = t₀ + (dt/dz)·z`, and
/// `c_eff(z) = 20.05·√(T(z)+273.15) + u(z)·cos(azimuth − φ)`. Returns the
/// `c_eff` samples aligned with `heights`.
///
/// # Errors
///
/// [`CaseLoadError::NonFinite`] if any surface field is non-finite or a
/// reconstructed sample is non-finite (never a panic — T-03-03-02).
pub fn reconstruct_profiles(met: &SurfaceMet, heights: &[f64]) -> Result<Vec<f64>, CaseLoadError> {
    let nonfinite = |what: &str| CaseLoadError::NonFinite {
        context: "weather route 3 (reconstruct)".to_string(),
        what: what.to_string(),
    };
    for (v, what) in [
        (met.u_zu, "u_zu"),
        (met.zu, "zu"),
        (met.z0, "z0"),
        (met.phi_deg, "phi"),
        (met.azimuth_deg, "azimuth"),
        (met.t0_c, "t0"),
        (met.dtdz, "dt/dz"),
        (met.inv_obukhov, "1/L"),
    ] {
        if !v.is_finite() {
            return Err(nonfinite(what));
        }
    }
    let z0 = met.z0.max(Z0_MIN_M); // clamp ≥ 0.001 m (D-16)
    // zu was validated finite above, so a plain comparison is unambiguous.
    if met.zu <= 0.0 {
        return Err(nonfinite("zu must be positive"));
    }
    // Friction-velocity magnitude (u*/κ) from the anemometer wind, inverting the
    // MO wind law at zu: u_zu = (u*/κ)·[ln(zu/z₀+1) − Ψ_m(zu/L)].
    let denom = (met.zu / z0 + 1.0).ln() - psi_m(met.zu * met.inv_obukhov);
    let u_star_kappa = if denom.abs() > 1e-9 {
        met.u_zu / denom
    } else {
        0.0
    };
    let delta_az = (met.azimuth_deg - met.phi_deg).to_radians();
    let cos_az = delta_az.cos();

    let mut c_eff = Vec::with_capacity(heights.len());
    for &z in heights {
        if !(z.is_finite() && z >= 0.0) {
            return Err(nonfinite("reconstruction height"));
        }
        let u_z = u_star_kappa * ((z / z0 + 1.0).ln() - psi_m(z * met.inv_obukhov));
        let t_z = met.t0_c + met.dtdz * z;
        let t_kelvin = t_z + KELVIN_OFFSET;
        // t_z is finite (t0, dtdz, z all finite), so a plain compare is safe.
        if t_kelvin <= 0.0 {
            return Err(nonfinite("reconstructed T (Kelvin) must be positive"));
        }
        // c_eff = 20.05·√(T+273.15) + u(z)·cos(az−φ) — the Coft primitive reused
        // for the temperature part (Eq. 335), the wind projected onto the bearing.
        let c = sound_speed_ms(t_z) + u_z * cos_az;
        if !c.is_finite() {
            return Err(nonfinite("reconstructed c_eff"));
        }
        c_eff.push(c);
    }
    Ok(c_eff)
}

/// Least-squares fit of the log-lin model `c_eff(z) ≈ A·ln(z/z₀+1) + B·z + C` to
/// the `(heights, c_eff)` samples via the hand-rolled 3×3 normal equations
/// `XᵀX β = Xᵀy` (D-08). Returns `(A, B, C)`.
///
/// # Single source of truth (09-03, METX-01)
///
/// The 3×3 Cramer LSQ was **lifted** into the WASM-safe [`envi_gis::weather`];
/// this function is a thin delegator that maps the lifted [`envi_gis::GisError`]
/// back to the harness's [`CaseLoadError`] so every existing Route-3 call site
/// and test is unchanged. There is **no second copy** of the fit here — the
/// round-trip identity oracle (below) still covers the one implementation.
///
/// # Errors
///
/// - [`CaseLoadError::Invalid`] if fewer than 3 samples, mismatched lengths, or
///   the normal matrix is singular (collinear heights) — a typed error, never a
///   panic (T-03-03-02).
/// - [`CaseLoadError::NonFinite`] if any sample is non-finite.
pub fn fit_profile(
    heights: &[f64],
    c_eff: &[f64],
    z0: f64,
) -> Result<(f64, f64, f64), CaseLoadError> {
    envi_gis::weather::fit_profile(heights, c_eff, z0).map_err(|e| match e {
        envi_gis::GisError::NonFinite { what } => CaseLoadError::NonFinite {
            context: "weather route 3 (fit)".to_string(),
            what,
        },
        other => CaseLoadError::Invalid {
            context: "weather route 3 (fit)".to_string(),
            message: other.to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const C0: f64 = 340.348_380_890_816_03;

    fn synth(a: f64, b: f64, c: f64, z0: f64, heights: &[f64]) -> Vec<f64> {
        heights
            .iter()
            .map(|&z| a * (z / z0 + 1.0).ln() + b * z + c)
            .collect()
    }

    // Round-trip identity (the MET-06 oracle): a profile generated from a known
    // (A, B, C) is recovered to tolerance by the 3×3 LSQ fit.
    #[test]
    fn fit_profile_round_trips_known_coefficients() {
        let z0 = 0.02;
        let heights = default_heights();
        for &(a, b, c) in &[
            (1.0, 0.05, C0),
            (-0.8, -0.04, C0),
            (0.0, 0.0, C0),
            (2.5, 0.1, 335.0),
        ] {
            let samples = synth(a, b, c, z0, &heights);
            let (fa, fb, fc) = fit_profile(&heights, &samples, z0).unwrap();
            assert!((fa - a).abs() < 1e-6, "A: got {fa} want {a}");
            assert!((fb - b).abs() < 1e-6, "B: got {fb} want {b}");
            assert!((fc - c).abs() < 1e-6, "C: got {fc} want {c}");
        }
    }

    // The z₀ used in the fit matters: fitting with the correct z₀ recovers A
    // exactly; a mismatched z₀ perturbs it (proving the log basis is active).
    #[test]
    fn fit_profile_uses_the_log_basis_at_z0() {
        let z0 = 0.01;
        let heights = default_heights();
        let samples = synth(1.5, 0.0, C0, z0, &heights);
        let (a_ok, _, _) = fit_profile(&heights, &samples, z0).unwrap();
        assert!((a_ok - 1.5).abs() < 1e-6);
        let (a_bad, _, _) = fit_profile(&heights, &samples, 0.5).unwrap();
        assert!((a_bad - 1.5).abs() > 1e-3, "wrong z₀ must perturb A");
    }

    // Collinear / insufficient heights ⇒ singular normal matrix ⇒ typed error.
    #[test]
    fn fit_profile_singular_is_typed_error() {
        // Fewer than three samples.
        assert!(matches!(
            fit_profile(&[1.0, 2.0], &[340.0, 341.0], 0.02),
            Err(CaseLoadError::Invalid { .. })
        ));
        // All heights identical ⇒ the basis columns collapse ⇒ singular.
        let h = [3.0, 3.0, 3.0, 3.0];
        let y = [340.0, 340.0, 340.0, 340.0];
        assert!(matches!(
            fit_profile(&h, &y, 0.02),
            Err(CaseLoadError::Invalid { .. })
        ));
    }

    #[test]
    fn fit_profile_rejects_non_finite_samples() {
        let h = default_heights();
        let mut y = synth(1.0, 0.0, C0, 0.02, &h);
        y[2] = f64::NAN;
        assert!(matches!(
            fit_profile(&h, &y, 0.02),
            Err(CaseLoadError::NonFinite { .. })
        ));
    }

    fn base_met() -> SurfaceMet {
        SurfaceMet {
            u_zu: 5.0,
            zu: 10.0,
            z0: 0.02,
            phi_deg: 0.0,
            azimuth_deg: 0.0, // downwind
            t0_c: 15.0,
            dtdz: 0.0,
            inv_obukhov: 0.0, // neutral
        }
    }

    // Downwind neutral reconstruction: u(z) is monotonically increasing (the log
    // law), so the reconstructed c_eff rises with height ⇒ downward refraction.
    #[test]
    fn reconstruct_downwind_profile_increases_with_height() {
        let heights = default_heights();
        let c_eff = reconstruct_profiles(&base_met(), &heights).unwrap();
        for w in c_eff.windows(2) {
            assert!(
                w[1] >= w[0],
                "downwind c_eff must be non-decreasing with height"
            );
        }
        assert!(c_eff.iter().all(|c| c.is_finite() && *c > 0.0));
    }

    // Upwind (azimuth 180° from φ) flips the wind projection: c_eff decreases
    // with height ⇒ upward refraction (opposite sign to downwind).
    #[test]
    fn reconstruct_upwind_profile_decreases_with_height() {
        let mut met = base_met();
        met.azimuth_deg = 180.0;
        let heights = default_heights();
        let c_eff = reconstruct_profiles(&met, &heights).unwrap();
        for w in c_eff.windows(2) {
            assert!(
                w[1] <= w[0],
                "upwind c_eff must be non-increasing with height"
            );
        }
    }

    // A warmer surface raises the ground-level c_eff (the √T temperature term).
    #[test]
    fn reconstruct_warmer_surface_raises_c_eff() {
        let heights = [0.5_f64];
        let cold = reconstruct_profiles(&base_met(), &heights).unwrap()[0];
        let warm = {
            let mut m = base_met();
            m.t0_c = 30.0;
            reconstruct_profiles(&m, &heights).unwrap()[0]
        };
        assert!(warm > cold, "warmer t₀ must raise c_eff: {warm} vs {cold}");
    }

    // reconstruct → fit round-trips into a finite (A, B, C) whose ground value
    // C = c_eff(0) is close (the fitted intercept ≈ the surface sound speed).
    #[test]
    fn reconstruct_then_fit_gives_finite_coefficients() {
        let mut met = base_met();
        met.dtdz = 0.05; // inversion
        let heights = default_heights();
        let c_eff = reconstruct_profiles(&met, &heights).unwrap();
        let (a, b, c) = fit_profile(&heights, &c_eff, met.z0).unwrap();
        assert!(a.is_finite() && b.is_finite() && c.is_finite());
        // Downwind + inversion ⇒ upward-curving profile ⇒ positive log slope.
        assert!(a > 0.0, "downwind log slope must be positive, got A={a}");
        // The fitted intercept is near the surface sound speed (within a few m/s).
        assert!(
            (c - sound_speed_ms(met.t0_c)).abs() < 5.0,
            "C≈c_eff(0), got {c}"
        );
    }

    #[test]
    fn reconstruct_rejects_non_finite_met() {
        let mut met = base_met();
        met.u_zu = f64::NAN;
        assert!(matches!(
            reconstruct_profiles(&met, &default_heights()),
            Err(CaseLoadError::NonFinite { .. })
        ));
    }
}
