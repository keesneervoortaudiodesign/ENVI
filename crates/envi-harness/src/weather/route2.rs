//! Route 2 — FORCE surface-met → `(A, B, C)` (MET-02, D-06).
//!
//! Maps the FORCE per-worksheet meteorology (`u@zu`, `φ`, `dt/dz`, `t₀`, `z₀`)
//! onto the engine's log-lin profile. Drives the up/downwind straight-road
//! cases.
//!
//! # ⚠️ `[ASSUMED]` scaling (RESEARCH Route 2 banner, Pitfall 7)
//!
//! AV 1106/07 does **not** specify the wind/temperature → `A/B` conversion. The
//! **structure** here is physically grounded, but the **scaling constants are
//! `[ASSUMED]`** (locked pending the 03-03 Open-Q1 checkpoint) and are validated
//! by **direction/structure property tests only** — never a false FORCE numeric
//! pass. What IS certain:
//!
//! - `A_wind ∝ (u*/κ)·cos(bearing − φ_u)` with the neutral log law
//!   `u(z) = (u*/κ)·ln(z/z₀+1)`; inverting at the anemometer height `zu` gives
//!   `(u*/κ) = u / ln(zu/z₀+1)` — the wind log coefficient magnitude.
//! - `B` follows the temperature gradient through `c = 20.05·√(T+273.15)`:
//!   `dc/dz = (c / (2·(T₀+273.15)))·dt/dz`, so an **inversion** (`dt/dz > 0`)
//!   gives `B > 0` (downward refraction) — success criterion 2. This factor is
//!   the exact derivative of `Coft`, not an assumed constant.
//! - `C = Coft(t₀)`.
//! - The isotropic **temperature** contribution to the **log** coefficient `A`
//!   is taken as **0** under the neutral log-law model (temperature enters via
//!   the linear `B` and the ground `C`, not the log part) — `[ASSUMED]`, but it
//!   keeps the "temperature once + wind per bearing" split explicit and honest.

use crate::cases::{CaseLoadError, PropagationParams};
use envi_engine::propagation::sound_speed_ms;

use super::{ReflectionProfiles, WeatherComponents, WeatherProfile, profile_for_bearing};

/// Absolute-zero offset for the Kelvin conversion in `Coft` (Eq. 335).
const KELVIN_OFFSET: f64 = 273.15;
/// Default anemometer height when a case omits `zu`, m (standard 10 m mast).
const DEFAULT_ZU_M: f64 = 10.0;
/// Default air temperature when a case omits `t₀`, °C (matches the terrain
/// loader's `unwrap_or(15.0)`).
const DEFAULT_T0_C: f64 = 15.0;
/// Default roughness length when a case omits `z₀`, m (clamped floor).
const DEFAULT_Z0_M: f64 = 0.001;

/// Validate an optional met field: absent ⇒ `default` (absence is data, not an
/// error); present-but-non-finite ⇒ a typed [`CaseLoadError::NonFinite`].
fn finite_or(field: Option<f64>, default: f64, what: &'static str) -> Result<f64, CaseLoadError> {
    match field {
        None => Ok(default),
        Some(v) if v.is_finite() => Ok(v),
        Some(_) => Err(CaseLoadError::NonFinite {
            context: "weather route 2".to_string(),
            what: what.to_string(),
        }),
    }
}

/// Build the **bearing-independent** [`WeatherComponents`] from FORCE surface
/// met (Route 2). Reused by [`route2`] and by the reflection-path split
/// (ENG-06) so the isotropic temperature part is computed once and the wind part
/// is projected per sub-path bearing.
///
/// # Errors
///
/// [`CaseLoadError::NonFinite`] if any present met field is NaN/∞, or if the
/// derived profile would be non-finite.
pub fn route2_components(params: &PropagationParams) -> Result<WeatherComponents, CaseLoadError> {
    let t0 = finite_or(params.t0_c, DEFAULT_T0_C, "t0")?;
    let u = finite_or(params.u_ms, 0.0, "u")?;
    let zu = finite_or(params.zu_m, DEFAULT_ZU_M, "zu")?;
    let dtdz = finite_or(params.dtdz, 0.0, "dt/dz")?;
    let su = finite_or(params.su_ms, 0.0, "su")?;
    let sdtdz = finite_or(params.sdtdz, 0.0, "sdt/dz")?;
    let z0_raw = finite_or(params.z0_m, DEFAULT_Z0_M, "z0")?;
    let z0 = z0_raw.max(DEFAULT_Z0_M); // clamp ≥ 0.001 m (D-16)

    // C = Coft(t₀) (Eq. 3, 335). t0 is a physical temperature; Coft rejects
    // ≤ −273.15 °C internally by producing a non-finite value we then guard.
    let c = sound_speed_ms(t0);
    if !(c.is_finite() && c > 0.0) {
        return Err(CaseLoadError::NonFinite {
            context: "weather route 2".to_string(),
            what: "C = Coft(t0)".to_string(),
        });
    }
    // t0 was validated finite above, so t0_kelvin is finite (NaN-safe compare).
    let t0_kelvin = t0 + KELVIN_OFFSET;
    if t0_kelvin <= 0.0 {
        return Err(CaseLoadError::NonFinite {
            context: "weather route 2".to_string(),
            what: "T0 (Kelvin) must be positive".to_string(),
        });
    }

    // Wind log coefficient magnitude: (u*/κ) = u / ln(zu/z₀ + 1). zu > 0 and
    // z₀ ≥ 0.001 ⇒ the log denominator is finite and positive.
    let log_zu = (zu / z0 + 1.0).ln();
    let a_wind = if log_zu.abs() > 1e-12 {
        u / log_zu
    } else {
        0.0
    };

    // B from the temperature gradient: dc/dz = (c / (2·(T₀+273.15)))·dt/dz.
    // Inversion (dt/dz > 0) ⇒ B > 0 (success criterion 2).
    let b_scale = c / (2.0 * t0_kelvin);
    let b = b_scale * dtdz;

    // Fluctuation std-devs (Eq. 10 feeds A⁺/B⁺). [ASSUMED] scaling — same shape
    // as A/B but from the met std-devs; 0 when absent (instantaneous levels).
    let s_a = if log_zu.abs() > 1e-12 {
        su / log_zu
    } else {
        0.0
    };
    let s_b = b_scale * sdtdz;

    let comps = WeatherComponents {
        a_temp: 0.0, // isotropic temperature log-part [ASSUMED] 0 (see module docs)
        a_wind,
        b,
        c,
        s_a,
        s_b,
        z0,
    };
    // Final finiteness guard on every derived coefficient (never a NaN profile).
    for (v, what) in [
        (comps.a_temp, "A_temp"),
        (comps.a_wind, "A_wind"),
        (comps.b, "B"),
        (comps.c, "C"),
        (comps.s_a, "sA"),
        (comps.s_b, "sB"),
        (comps.z0, "z0"),
    ] {
        if !v.is_finite() {
            return Err(CaseLoadError::NonFinite {
                context: "weather route 2".to_string(),
                what: what.to_string(),
            });
        }
    }
    Ok(comps)
}

/// Route 2: FORCE surface met + a propagation `bearing_deg` → a concrete
/// [`WeatherProfile`] (wind projected onto the bearing, MET-02).
///
/// `φ_u` is read from `params.phi_deg` (wind direction re north; absent ⇒ 0).
///
/// # Errors
///
/// [`CaseLoadError::NonFinite`] on any malformed met field (never a panic —
/// threat T-03-02-02).
pub fn route2(
    params: &PropagationParams,
    bearing_deg: f64,
) -> Result<WeatherProfile, CaseLoadError> {
    let comps = route2_components(params)?;
    let phi_u = finite_or(params.phi_deg, 0.0, "phi")?;
    Ok(profile_for_bearing(&comps, bearing_deg, phi_u))
}

/// Route 2 for a **reflection path** (ENG-06): FORCE surface met + the two
/// sub-path bearings → a [`ReflectionProfiles`] before/after pair. The
/// isotropic temperature part is computed once (via [`route2_components`]) and
/// the wind part is projected onto each sub-path bearing.
///
/// # Errors
///
/// [`CaseLoadError::NonFinite`] on any malformed met field (never a panic).
pub fn route2_reflection(
    params: &PropagationParams,
    bearing_before_deg: f64,
    bearing_after_deg: f64,
) -> Result<ReflectionProfiles, CaseLoadError> {
    let comps = route2_components(params)?;
    let phi_u = finite_or(params.phi_deg, 0.0, "phi")?;
    Ok(ReflectionProfiles::from_components(
        &comps,
        bearing_before_deg,
        bearing_after_deg,
        phi_u,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn met(u: f64, phi: f64, dtdz: f64) -> PropagationParams {
        PropagationParams {
            t0_c: Some(15.0),
            z0_m: Some(0.02),
            zu_m: Some(10.0),
            u_ms: Some(u),
            phi_deg: Some(phi),
            dtdz: Some(dtdz),
            ..PropagationParams::default()
        }
    }

    // Inversion (dt/dz > 0) ⇒ B > 0; a lapse (dt/dz < 0) ⇒ B < 0 (criterion 2).
    #[test]
    fn weather_route2_inversion_gives_positive_b() {
        let inv = route2(&met(0.0, 0.0, 0.05), 0.0).unwrap();
        assert!(inv.b > 0.0, "inversion must give B > 0, got {}", inv.b);
        let lapse = route2(&met(0.0, 0.0, -0.05), 0.0).unwrap();
        assert!(lapse.b < 0.0, "lapse must give B < 0, got {}", lapse.b);
        let neutral = route2(&met(0.0, 0.0, 0.0), 0.0).unwrap();
        assert_eq!(neutral.b, 0.0, "neutral gradient ⇒ B = 0");
    }

    // Downwind A > upwind A for the same wind, purely from the bearing (MET-02).
    #[test]
    fn weather_route2_downwind_a_exceeds_upwind() {
        let params = met(5.0, 90.0, 0.0); // 5 m/s toward east
        let downwind = route2(&params, 90.0).unwrap();
        let upwind = route2(&params, 270.0).unwrap();
        assert!(
            downwind.a > upwind.a,
            "downwind A {} must exceed upwind A {}",
            downwind.a,
            upwind.a
        );
        assert!(
            downwind.a > 0.0,
            "downwind wind ⇒ positive A log coefficient"
        );
    }

    // C = Coft(t₀): warmer ground raises C.
    #[test]
    fn weather_route2_c_tracks_temperature() {
        let cold = route2(&met(0.0, 0.0, 0.0), 0.0).unwrap();
        let warm = {
            let mut p = met(0.0, 0.0, 0.0);
            p.t0_c = Some(25.0);
            route2(&p, 0.0).unwrap()
        };
        assert!(warm.c > cold.c, "warmer t₀ must raise C");
    }

    // z₀ is clamped ≥ 0.001 m even when a case supplies a smaller value.
    #[test]
    fn weather_profile_z0_clamped_via_route2() {
        let mut p = met(1.0, 0.0, 0.0);
        p.z0_m = Some(1e-9);
        let prof = route2(&p, 0.0).unwrap();
        assert_eq!(prof.z0, 0.001);
    }

    // Malformed met fields return a typed error, never a panic (T-03-02-02).
    #[test]
    fn weather_route2_rejects_non_finite_met() {
        let mut p = met(1.0, 0.0, 0.0);
        p.u_ms = Some(f64::NAN);
        assert!(matches!(
            route2(&p, 0.0),
            Err(CaseLoadError::NonFinite { .. })
        ));

        let mut q = met(1.0, 0.0, 0.0);
        q.dtdz = Some(f64::INFINITY);
        assert!(matches!(
            route2(&q, 0.0),
            Err(CaseLoadError::NonFinite { .. })
        ));
    }

    // Absent optional fields are data, not an error: a bare params still routes.
    #[test]
    fn weather_route2_absent_fields_use_defaults() {
        let prof = route2(&PropagationParams::default(), 45.0).unwrap();
        assert!(prof.c.is_finite() && prof.c > 0.0);
        assert_eq!(prof.a, 0.0, "no wind ⇒ A = 0");
        assert_eq!(prof.b, 0.0, "no gradient ⇒ B = 0");
    }

    // The reflection variant projects the same met onto two sub-path bearings:
    // A₁ ≠ A₂ across differing bearings, B₁ = B₂ (bearing-independent).
    #[test]
    fn weather_route2_reflection_splits_a_shares_b() {
        let params = met(5.0, 90.0, 0.04); // wind east, an inversion
        let rp = route2_reflection(&params, 90.0, 180.0).unwrap();
        assert!(
            (rp.before.a - rp.after.a).abs() > 1e-9,
            "sub-path A₁ {} and A₂ {} must differ",
            rp.before.a,
            rp.after.a
        );
        assert_eq!(rp.before.b, rp.after.b, "B shared across sub-paths");
        assert!(rp.before.b > 0.0, "inversion ⇒ B > 0 on both sub-paths");
    }
}
