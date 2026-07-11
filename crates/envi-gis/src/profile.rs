//! Source→receiver DEM cut-profile extractor (GEOX-01).
//!
//! # Module I/O
//! - **Inputs:** a built [`Tin`] (the imported DGM surface — see `envi_dgm`), the
//!   planar source and receiver **ground** positions `[x, y]` (scene meters), and a
//!   sampling step (meters; the caller passes the DEM cell size to match GRASS
//!   `r.profile`'s cell-resolution walk).
//! - **Output:** the strictly-ascending-x `(x, z)` ground points of an
//!   [`envi_engine::scene::TerrainProfile`] cut plane, where `x` is horizontal
//!   distance from the source ground point and `z` is GROUND elevation sampled from
//!   the TIN. Absence (a sample outside the TIN hull) is a typed
//!   [`GisError::OutsideHull`] — **never a fabricated `0.0`** (D-07).
//! - **Invariants (load-bearing):**
//!   1. **Bounded output** (threat T-09-01-01): the sample count is hard-capped at
//!      [`MAX_PROFILE_POINTS`]; an over-long request is [`GisError::ProfileTooLong`],
//!      rejected before allocation (mirrors `terrain::MAX_TERRAIN_POINTS`).
//!   2. **No silent 0.0** (D-07, threat T-09-01-02): a sample whose planar point
//!      leaves the TIN hull maps `Tin::interpolate_z`'s `None` to
//!      [`GisError::OutsideHull`], never a default elevation.
//!   3. **Ground z only — the hSv/hRv trap** (09-RESEARCH Pitfall 2): this emits
//!      GROUND elevation only; the source/receiver acoustic heights are added later
//!      at `SolveJob` assembly via `TerrainProfile::endpoints` (which places the
//!      source above the FIRST point and the receiver above the LAST). Never bake a
//!      src/rcv height into a profile z.
//!   4. **Strictly-ascending x** (Pitfall 3): collinear/duplicate-x samples are
//!      deduped so the point list builds a valid `TerrainProfile` (`N` points ⇒
//!      `N − 1` segments, filled by GEOX-02).
//!
//! # Why sample the TIN, not the raw raster
//! The scene's ground model is a constrained-Delaunay TIN (`envi_dgm`), so
//! barycentric TIN interpolation is the natural exact-on-the-mesh sampler and
//! avoids re-deciding a raster resampling kernel. Because ENVI samples the TIN
//! while the `r.profile` oracle reads the raster (bilinear), the oracle test pins a
//! documented tolerance (the TIN-linear vs raster-bilinear kernel delta), not
//! bit-equality — see `tests/profile_oracle.rs`.

use envi_dgm::tin::Tin;

use crate::{GisError, X_EPSILON_M};

/// Hard cap on cut-profile sample points (`[ASSUMED]` engineering bound, threat
/// T-09-01-01). At the DEM cell resolution (~0.5 m) this is a ~50 km source→
/// receiver path — far beyond any real acoustic propagation distance — so it only
/// ever trips on a pathological tiny-step / huge-distance request, which is
/// rejected before allocation (mirrors `terrain::MAX_TERRAIN_POINTS`).
pub const MAX_PROFILE_POINTS: usize = 100_000;

/// Extract the source→receiver DEM cut-profile as strictly-ascending `(x, z)`
/// ground points over the DGM `tin`.
///
/// Walks the S→R line at `step_m` resolution (the caller passes the DEM cell size
/// to mirror `r.profile`), sampling ground elevation at each interpolated planar
/// point via [`Tin::interpolate_z`]. `x` is horizontal distance from the source
/// ground point (`x = 0` at the source), increasing toward the receiver; `z` is
/// ground elevation ONLY (no src/rcv height — Invariant 3).
///
/// # Errors
/// - [`GisError::NonFinite`] — any endpoint coordinate or the step is NaN/∞.
/// - [`GisError::DegenerateProfile`] — a non-positive step, a zero-length path, or
///   fewer than two strictly-ascending points result.
/// - [`GisError::ProfileTooLong`] — the request would exceed [`MAX_PROFILE_POINTS`].
/// - [`GisError::OutsideHull`] — a sample's planar point leaves the TIN hull.
pub fn cut_profile(
    tin: &Tin,
    s_xy: [f64; 2],
    r_xy: [f64; 2],
    step_m: f64,
) -> Result<Vec<[f64; 2]>, GisError> {
    // Finiteness guards — never sample on NaN/inf (V5 input validation).
    for (label, v) in [
        ("s_x", s_xy[0]),
        ("s_y", s_xy[1]),
        ("r_x", r_xy[0]),
        ("r_y", r_xy[1]),
        ("step_m", step_m),
    ] {
        if !v.is_finite() {
            return Err(GisError::NonFinite {
                what: format!("cut_profile {label} = {v}"),
            });
        }
    }
    if step_m <= 0.0 {
        return Err(GisError::DegenerateProfile {
            what: format!("sampling step must be positive, got {step_m}"),
        });
    }

    let dx = r_xy[0] - s_xy[0];
    let dy = r_xy[1] - s_xy[1];
    let d = dx.hypot(dy);
    if d <= 0.0 {
        return Err(GisError::DegenerateProfile {
            what: "source and receiver coincide (zero-length path)".to_string(),
        });
    }

    // Sample at cell resolution: n intervals ⇒ n + 1 points from t = 0..=1. Guard
    // a pathological tiny-step / huge-distance request in f64 BEFORE any cast or
    // allocation (threat T-09-01-01) — `f64 as usize` saturates, so an unchecked
    // cast could otherwise wrap on `+ 1`.
    let n_f = (d / step_m).ceil().max(1.0);
    if n_f + 1.0 > MAX_PROFILE_POINTS as f64 {
        let got = if n_f.is_finite() && n_f < 1e18 {
            n_f as usize + 1
        } else {
            usize::MAX
        };
        return Err(GisError::ProfileTooLong {
            got,
            limit: MAX_PROFILE_POINTS,
        });
    }
    let n = n_f as usize;

    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(n + 1);
    let mut last_x = f64::NEG_INFINITY;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let x = t * d;
        let px = s_xy[0] + t * dx;
        let py = s_xy[1] + t * dy;
        // `None` outside the hull → typed error, never a fabricated 0.0 (D-07).
        let z = tin.interpolate_z(px, py).ok_or(GisError::OutsideHull)?;
        if x > last_x + X_EPSILON_M {
            pts.push([x, z]);
            last_x = x;
        }
    }

    if pts.len() < 2 {
        return Err(GisError::DegenerateProfile {
            what: format!("fewer than two strictly-ascending points ({})", pts.len()),
        });
    }
    Ok(pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::scene::{GroundSegment, TerrainProfile};

    /// A square whose Z equals its y at every corner, so the surface is the plane
    /// `z = y` and any interior sample interpolates exactly to its y (regardless of
    /// the triangulation's diagonal). Hull is `[0,10] × [0,10]`.
    fn plane_z_eq_y() -> Tin {
        let pts = [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0],
        ];
        envi_dgm::tin::build_tin(&pts, &[]).expect("valid square builds a TIN")
    }

    #[test]
    fn cut_profile_is_ascending_ground_z_along_the_line() {
        let tin = plane_z_eq_y();
        // Diagonal S→R inside the hull; along it px == py, so z == y == py.
        let profile = cut_profile(&tin, [1.0, 1.0], [9.0, 9.0], 1.0).expect("inside the hull");

        // Strictly ascending x, starting at 0.
        assert_eq!(profile[0][0], 0.0, "x starts at the source ground point");
        for w in profile.windows(2) {
            assert!(w[1][0] > w[0][0], "x is strictly ascending");
        }

        // Ground z only: on the plane z = y, and along the diagonal py = 1 + t*8,
        // so z must equal the sampled py — never a source/receiver height.
        let d = ((9.0f64 - 1.0).powi(2) * 2.0).sqrt();
        for p in &profile {
            let t = p[0] / d;
            let py = 1.0 + t * 8.0;
            assert!(
                (p[1] - py).abs() < 1e-9,
                "z {} must be ground elevation (py = {py})",
                p[1]
            );
        }
    }

    #[test]
    fn extracted_points_build_a_valid_terrain_profile() {
        // The whole point of ascending-x + dedupe: the list is a valid cut plane.
        let tin = plane_z_eq_y();
        let profile = cut_profile(&tin, [1.0, 2.0], [8.0, 7.0], 0.5).expect("inside the hull");
        let segs = vec![
            GroundSegment {
                flow_resistivity: 12.5,
                roughness: 0.0,
            };
            profile.len() - 1
        ];
        let tp = TerrainProfile::new(profile.clone(), segs);
        assert!(
            tp.is_ok(),
            "N points must build N-1 segments as a valid TerrainProfile: {tp:?}"
        );
    }

    #[test]
    fn outside_hull_sample_is_typed_error_not_zero() {
        let tin = plane_z_eq_y();
        // The receiver is well outside the [0,10]^2 hull, so a late sample misses.
        let err = cut_profile(&tin, [5.0, 5.0], [100.0, 100.0], 1.0).unwrap_err();
        assert_eq!(
            err,
            GisError::OutsideHull,
            "hull miss is OutsideHull, got {err:?}"
        );
    }

    #[test]
    fn over_length_request_is_rejected_before_allocation() {
        let tin = plane_z_eq_y();
        // A ~11 m diagonal at a 1e-9 m step implies ~1.1e10 points ≫ the cap.
        let err = cut_profile(&tin, [1.0, 1.0], [9.0, 9.0], 1e-9).unwrap_err();
        assert!(
            matches!(
                err,
                GisError::ProfileTooLong {
                    limit: MAX_PROFILE_POINTS,
                    ..
                }
            ),
            "tiny step must be ProfileTooLong, got {err:?}"
        );
    }

    #[test]
    fn degenerate_step_and_coincident_endpoints_are_typed_errors() {
        let tin = plane_z_eq_y();
        assert!(matches!(
            cut_profile(&tin, [1.0, 1.0], [9.0, 9.0], 0.0).unwrap_err(),
            GisError::DegenerateProfile { .. }
        ));
        assert!(matches!(
            cut_profile(&tin, [1.0, 1.0], [9.0, 9.0], -1.0).unwrap_err(),
            GisError::DegenerateProfile { .. }
        ));
        // Coincident source/receiver → zero-length path.
        assert!(matches!(
            cut_profile(&tin, [5.0, 5.0], [5.0, 5.0], 1.0).unwrap_err(),
            GisError::DegenerateProfile { .. }
        ));
    }

    #[test]
    fn non_finite_inputs_are_rejected() {
        let tin = plane_z_eq_y();
        assert!(matches!(
            cut_profile(&tin, [f64::NAN, 1.0], [9.0, 9.0], 1.0).unwrap_err(),
            GisError::NonFinite { .. }
        ));
        assert!(matches!(
            cut_profile(&tin, [1.0, 1.0], [9.0, f64::INFINITY], 1.0).unwrap_err(),
            GisError::NonFinite { .. }
        ));
        assert!(matches!(
            cut_profile(&tin, [1.0, 1.0], [9.0, 9.0], f64::NAN).unwrap_err(),
            GisError::NonFinite { .. }
        ));
    }

    #[test]
    fn short_path_below_one_step_still_yields_two_points() {
        // d < step ⇒ n = 1 ⇒ exactly the two endpoints, strictly ascending.
        let tin = plane_z_eq_y();
        let profile = cut_profile(&tin, [4.0, 4.0], [4.3, 4.3], 1.0).expect("inside the hull");
        assert_eq!(profile.len(), 2, "one interval yields two endpoints");
        assert!(profile[1][0] > profile[0][0]);
    }
}
