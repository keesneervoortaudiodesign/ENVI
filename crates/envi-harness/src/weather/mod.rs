//! Weather-input routes: convert meteorology into the engine's
//! `(A, B, C, sA, sB, z₀)` log-lin sound-speed profile (MET-02/05/06).
//!
//! # I/O quarantine (D-15)
//!
//! All weather-route logic lives in **`envi-harness`** — the engine consumes
//! `(A, B, C, …)` regardless of how they were derived. This module owns the
//! `[WeatherProfile]` the engine's refraction entry point reads, the
//! per-azimuth wind projection, and the FORCE-driven Route 2.
//!
//! # ⚠️ Route derivations are `[ASSUMED]` (RESEARCH Weather-Routes banner)
//!
//! AV 1106/07 takes `A, B, C` as **inputs** and does **not** specify how to
//! derive them from wind/temperature. The Route-2 A/B **scaling constants** are
//! therefore `[ASSUMED]` and are validated by **direction/structure property
//! tests only** — never a false FORCE numeric pass (locked pending the 03-03
//! Open-Q1 checkpoint). Only the **sign logic** is physically certain:
//! downwind ⇒ larger A, an inversion (`dt/dz > 0`) ⇒ `B > 0`.
//!
//! # Per-azimuth A (MET-02)
//!
//! The **isotropic temperature part** of `A` is computed **once**; the
//! **wind part is projected per bearing** as `A_wind·cos(bearing − φ_u)` using
//! the [`envi_engine::geometry::azimuth_deg`] clockwise-from-north convention
//! (`φ_u` is the downwind bearing). `B` (stability / temperature gradient) is
//! **bearing-independent**.

pub mod route2;

use envi_engine::propagation::refraction::profile::Z0_MIN_M;

/// A log-lin sound-speed profile for one propagation bearing:
/// `c(z) = A·ln(z/z₀+1) + B·z + C`, plus the fluctuation std-devs `sA`/`sB`
/// (Eq. 10 upper-refraction profile `A⁺ = A + 1.7·sA`).
///
/// This is the harness-side mirror of the engine's
/// [`envi_engine::propagation::refraction::SoundSpeedProfile`] (which carries no
/// `sA`/`sB`); the extra fields feed the Phase-3 F_τ turbulence coherence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherProfile {
    /// Logarithmic coefficient `A`, m/s (wind-projected per bearing).
    pub a: f64,
    /// Linear coefficient `B`, s⁻¹ (bearing-independent; inversion ⇒ B > 0).
    pub b: f64,
    /// Ground sound speed `C = Coft(t₀)`, m/s.
    pub c: f64,
    /// Std-dev of `A` (fluctuating refraction), m/s.
    pub s_a: f64,
    /// Std-dev of `B` (fluctuating refraction), s⁻¹.
    pub s_b: f64,
    /// Roughness length `z₀`, m (clamped ≥ 0.001 m).
    pub z0: f64,
}

impl WeatherProfile {
    /// Construct a profile, **clamping `z₀ ≥ 0.001 m`** (MET-01 / D-16, matching
    /// the engine's [`Z0_MIN_M`] posture) so the ray calculations never hit the
    /// `z₀ → 0` singularity. Non-finite inputs are the caller's responsibility
    /// to reject (Route 2 does so with a typed error before calling here).
    #[must_use]
    pub fn new(a: f64, b: f64, c: f64, s_a: f64, s_b: f64, z0: f64) -> Self {
        Self {
            a,
            b,
            c,
            s_a,
            s_b,
            z0: z0.max(Z0_MIN_M),
        }
    }
}

/// The **bearing-independent** decomposition of a weather profile: the wind
/// contribution to `A` is kept as a magnitude `a_wind` (projected per bearing),
/// separate from the isotropic temperature part `a_temp` (added once). `B`, `C`,
/// `sA`, `sB`, `z₀` are all bearing-independent.
///
/// [`profile_for_bearing`] turns this plus a bearing into a [`WeatherProfile`].
/// Keeping the decomposition lets a **reflection path** (ENG-06) project the
/// wind part at two different sub-path bearings while computing the temperature
/// part only once (MET-02).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherComponents {
    /// Isotropic temperature part of `A` (computed once), m/s.
    pub a_temp: f64,
    /// Wind magnitude of `A` (projected per bearing as `a_wind·cos(Δ)`), m/s.
    pub a_wind: f64,
    /// Linear coefficient `B`, s⁻¹ (bearing-independent).
    pub b: f64,
    /// Ground sound speed `C`, m/s.
    pub c: f64,
    /// Std-dev of `A`, m/s.
    pub s_a: f64,
    /// Std-dev of `B`, s⁻¹.
    pub s_b: f64,
    /// Roughness length `z₀`, m.
    pub z0: f64,
}

/// Project the wind part of `base` onto `bearing_deg` and return the concrete
/// [`WeatherProfile`] (MET-02).
///
/// `A(bearing) = a_temp + a_wind·cos(bearing − φ_u)` — downwind
/// (`bearing ≈ φ_u`) gives the largest `A`; a bearing 180° opposite gives the
/// smallest. Bearings are degrees clockwise from north
/// ([`envi_engine::geometry::azimuth_deg`] convention). `B`/`C`/`sA`/`sB`/`z₀`
/// pass through unchanged (bearing-independent).
#[must_use]
pub fn profile_for_bearing(
    base: &WeatherComponents,
    bearing_deg: f64,
    phi_u_deg: f64,
) -> WeatherProfile {
    let delta = (bearing_deg - phi_u_deg).to_radians();
    let a = base.a_temp + base.a_wind * delta.cos();
    WeatherProfile::new(a, base.b, base.c, base.s_a, base.s_b, base.z0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_profile_z0_is_clamped_on_construction() {
        let p = WeatherProfile::new(0.5, 0.02, 340.0, 0.0, 0.0, 1e-9);
        assert_eq!(p.z0, Z0_MIN_M, "z₀ below the floor must clamp to 0.001 m");
        let q = WeatherProfile::new(0.5, 0.02, 340.0, 0.0, 0.0, 0.05);
        assert_eq!(q.z0, 0.05, "z₀ above the floor is preserved");
    }

    // Downwind A > upwind A: the same met at two bearings 180° apart differs in
    // the wind sign only (MET-02 per-azimuth projection).
    #[test]
    fn weather_profile_downwind_a_exceeds_upwind_a() {
        let base = WeatherComponents {
            a_temp: 0.1,
            a_wind: 0.8,
            b: 0.03,
            c: 340.0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.02,
        };
        let phi_u = 90.0; // wind toward the east
        let downwind = profile_for_bearing(&base, phi_u, phi_u); // along the wind
        let upwind = profile_for_bearing(&base, phi_u + 180.0, phi_u); // against it
        assert!(
            downwind.a > upwind.a,
            "downwind A {} must exceed upwind A {}",
            downwind.a,
            upwind.a
        );
        // Downwind = a_temp + a_wind; upwind = a_temp − a_wind (the temp part is
        // the mean, computed once).
        assert!((downwind.a - (base.a_temp + base.a_wind)).abs() < 1e-12);
        assert!((upwind.a - (base.a_temp - base.a_wind)).abs() < 1e-12);
    }

    // B is bearing-independent: projecting at any bearing leaves B untouched.
    #[test]
    fn weather_profile_b_is_bearing_independent() {
        let base = WeatherComponents {
            a_temp: 0.0,
            a_wind: 0.5,
            b: 0.04,
            c: 340.0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.02,
        };
        for bearing in [0.0, 45.0, 137.0, 250.0, 359.0] {
            let p = profile_for_bearing(&base, bearing, 30.0);
            assert_eq!(p.b, base.b, "B must not vary with bearing");
        }
    }

    // Crosswind (bearing 90° off φ_u) zeroes the wind projection ⇒ A = a_temp.
    #[test]
    fn weather_profile_crosswind_is_isotropic_temperature_only() {
        let base = WeatherComponents {
            a_temp: 0.15,
            a_wind: 0.9,
            b: 0.0,
            c: 340.0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.02,
        };
        let p = profile_for_bearing(&base, 90.0 + 90.0, 90.0);
        assert!(
            (p.a - base.a_temp).abs() < 1e-12,
            "crosswind A {} must equal the isotropic temperature part {}",
            p.a,
            base.a_temp
        );
    }
}
