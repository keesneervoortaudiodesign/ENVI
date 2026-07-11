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

// The weather-route profile types + the per-bearing projection + the
// reflection-path split were **lifted** into the WASM-safe `envi-gis::weather`
// (09-03, METX-01) so the log-lin LSQ has exactly one home. The harness
// re-exports them verbatim, so every existing call site (`route2`, the oracle
// tests) and the public `envi_harness::weather::{WeatherProfile, …}` path are
// unchanged — there is no second copy of the math here.
pub use envi_gis::weather::{
    ReflectionProfiles, SubPathCollapse, WeatherComponents, WeatherProfile, profile_for_bearing,
};

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::propagation::PropagationError;
    use envi_engine::propagation::refraction::profile::Z0_MIN_M;

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
