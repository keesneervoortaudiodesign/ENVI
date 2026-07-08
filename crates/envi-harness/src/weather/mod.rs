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

pub mod route1;
pub mod route2;
pub mod route3;

use envi_engine::propagation::PropagationError;
use envi_engine::propagation::refraction::eqssp::calc_eq_ssp;
use envi_engine::propagation::refraction::profile::Z0_MIN_M;

/// A log-lin sound-speed profile for one propagation bearing:
/// `c(z) = A·ln(z/z₀+1) + B·z + C`, plus the fluctuation std-devs `sA`/`sB`
/// (Eq. 10 upper-refraction profile `A⁺ = A + 1.7·sA`).
///
/// This is the harness-side mirror of the engine's
/// [`envi_engine::propagation::refraction::SoundSpeedProfile`]: the two structs
/// carry the same `(A, B, C, sA, sB, z₀)` fields (the engine struct *does* have
/// `s_a`/`s_b`) and differ only in crate location — I/O-side (harness) vs
/// pure-math (engine). They are intentionally NOT unified here; a future
/// (Phase-4) refactor may collapse them into one type. Until then the harness
/// owns weather-route derivation and the engine consumes the plain profile.
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

/// One sub-path CalcEqSSP collapse `(ξ, c₀)` — the equivalent relative gradient
/// and ground sound speed for a single reflection sub-path leg.
pub type SubPathCollapse = (f64, f64);

/// The before/after weather-profile pair for a **reflection path** (ENG-06,
/// §5.3.4): the propagation model runs on each sub-path with its own equivalent
/// profile — `before` between source and the reflection point (`A₁, B₁`),
/// `after` between the reflection point and the receiver (`A₂, B₂`).
///
/// `A` differs per sub-path because each leg has its own bearing (the wind
/// projection `A ∝ u·cos(bearing − φ_u)` differs); `B`/`C`/`z₀` are shared
/// (bearing-independent). No FORCE numeric target this phase (Open Q2) — this is
/// the clean interface Phase-4 obstacle reflection consumes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReflectionProfiles {
    /// Source → reflection-point sub-path profile (`A₁, B₁`).
    pub before: WeatherProfile,
    /// Reflection-point → receiver sub-path profile (`A₂, B₂`).
    pub after: WeatherProfile,
}

impl ReflectionProfiles {
    /// Build the before/after pair from the shared [`WeatherComponents`] and the
    /// two sub-path bearings (MET-02: the wind part of `A` is projected onto
    /// each bearing; the isotropic temperature part and `B` are shared).
    #[must_use]
    pub fn from_components(
        base: &WeatherComponents,
        bearing_before_deg: f64,
        bearing_after_deg: f64,
        phi_u_deg: f64,
    ) -> Self {
        Self {
            before: profile_for_bearing(base, bearing_before_deg, phi_u_deg),
            after: profile_for_bearing(base, bearing_after_deg, phi_u_deg),
        }
    }

    /// Collapse each sub-path to its own `(ξ, c₀)` via CalcEqSSP on its leg
    /// geometry (§5.5.2 "separate equivalent profiles … between source and the
    /// nearest screen … receiver and the nearest screen"): the `before` leg uses
    /// `(before_hs, before_hr)`, the `after` leg `(after_hs, after_hr)`.
    ///
    /// Returns `((ξ₁, c₀₁), (ξ₂, c₀₂))`.
    ///
    /// # Errors
    ///
    /// [`PropagationError::DegenerateProfile`] if either sub-path profile is
    /// degenerate (non-finite / non-physical) — the same typed error the engine
    /// raises, propagated so a bad reflector geometry never panics.
    pub fn sub_path_collapses(
        &self,
        before_hs: f64,
        before_hr: f64,
        after_hs: f64,
        after_hr: f64,
    ) -> Result<(SubPathCollapse, SubPathCollapse), PropagationError> {
        let e1 = calc_eq_ssp(
            before_hs,
            before_hr,
            self.before.z0,
            self.before.a,
            self.before.b,
            self.before.c,
        )?;
        let e2 = calc_eq_ssp(
            after_hs,
            after_hr,
            self.after.z0,
            self.after.a,
            self.after.b,
            self.after.c,
        )?;
        Ok((e1, e2))
    }
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

    // ----- Reflection-path before/after split (ENG-06) -----

    fn wind_components() -> WeatherComponents {
        WeatherComponents {
            a_temp: 0.1,
            a_wind: 0.8,
            b: 0.03,
            c: 340.0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.02,
        }
    }

    // Differing sub-path bearings ⇒ A₁ ≠ A₂; B is shared (bearing-independent).
    #[test]
    fn reflection_path_profiles_split_a_share_b() {
        let base = wind_components();
        // φ_u = 90°: before leg downwind (bearing 90°, cos = 1), after leg
        // crosswind (bearing 180°, cos = 0) — the projections genuinely differ.
        let rp = ReflectionProfiles::from_components(&base, 90.0, 180.0, 90.0);
        assert!(
            (rp.before.a - rp.after.a).abs() > 1e-9,
            "A₁ {} and A₂ {} must differ across bearings",
            rp.before.a,
            rp.after.a
        );
        assert_eq!(rp.before.b, rp.after.b, "B must be shared (B₁ = B₂)");
        assert_eq!(rp.before.c, rp.after.c, "C shared");
    }

    // Homogeneous atmosphere: both sub-path profiles collapse to (0, C), so the
    // combined reflection reduces to the single-profile 03-01 result. The
    // reflection point is cross-checked against reflect_over_segment.
    #[test]
    fn reflection_path_homogeneous_collapses_to_single_profile() {
        use envi_engine::geometry::{azimuth_deg, reflect_over_segment};

        let c = 340.348;
        // Homogeneous ⇒ no wind, no gradient: every projection gives A = B = 0.
        let base = WeatherComponents {
            a_temp: 0.0,
            a_wind: 0.0,
            b: 0.0,
            c,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.001,
        };

        // A reflection scenario in the vertical cut plane (x, z): source at
        // (0, 0.5), receiver at (100, 1.5), reflecting off the ground segment.
        let s = [0.0, 0.5];
        let r = [100.0, 1.5];
        let refl = reflect_over_segment(s, r, [0.0, 0.0], [100.0, 0.0])
            .expect("homogeneous reflection must be defined");
        assert!(refl.valid, "reflection point must lie within the segment");
        // Homogeneous flat-ground image reflection point: x = d·hS/(hS+hR).
        let want_x = 100.0 * 0.5 / (0.5 + 1.5);
        assert!(
            (refl.point_x - want_x).abs() < 1e-9,
            "reflect_over_segment point_x {} must match the analytic {want_x}",
            refl.point_x
        );

        // Two sub-path bearings from the reflection point (plan-view azimuths;
        // here the cut-plane x maps to a north bearing — the convention only has
        // to be self-consistent for the projection).
        let refl_xy = [refl.point_x, 0.0];
        let bearing_before = azimuth_deg([s[0], 0.0], refl_xy);
        let bearing_after = azimuth_deg(refl_xy, [r[0], 0.0]);
        let rp = ReflectionProfiles::from_components(&base, bearing_before, bearing_after, 0.0);

        // Both sub-paths collapse to (0, C) — the homogeneous single-profile limit.
        let ((xi1, c01), (xi2, c02)) = rp
            .sub_path_collapses(0.5, refl.point_x.max(0.5), refl.point_x.max(0.5), 1.5)
            .unwrap();
        assert_eq!((xi1, c01), (0.0, c), "before leg homogeneous ⇒ (0, C)");
        assert_eq!((xi2, c02), (0.0, c), "after leg homogeneous ⇒ (0, C)");
    }

    // Degenerate sub-path geometry (non-finite height) returns a typed error,
    // never a panic.
    #[test]
    fn reflection_path_degenerate_geometry_is_typed_error() {
        let base = wind_components();
        let rp = ReflectionProfiles::from_components(&base, 0.0, 90.0, 0.0);
        assert!(matches!(
            rp.sub_path_collapses(f64::NAN, 1.5, 0.5, 1.5),
            Err(PropagationError::DegenerateProfile { .. })
        ));
    }
}
