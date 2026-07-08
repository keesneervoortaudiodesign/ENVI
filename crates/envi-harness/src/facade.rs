//! City-street façade reflection paths — the image-source path builder feeding
//! Sub-model 11 (plan 04-04, EP 1335 Ch. 5).
//!
//! Building façades reflect road noise toward receivers. Nord2000 handles this
//! with a reflection path S → O → R (§5.20); the **path construction** is
//! harness/case logic (this module), the **efficiency correction** `L11` is
//! engine math ([`envi_engine::propagation::terrain_effect::submodel11`]). First-
//! and second-order reflections are built by composing
//! [`envi_engine::geometry::reflect_over_segment`] per reflecting face (the
//! image-source primitive), in the horizontal (x, y) plan view.
//!
//! # Free-field SPL convention (own-façade exclusion)
//!
//! City-street receivers sit at building façades; the reported level is the
//! "free-field" SPL, which **excludes the reflection off the receiver's OWN
//! façade** (that reflection is part of the façade-level definition, not an
//! external path). [`first_order_paths`] takes the own-façade index and skips it.
//!
//! The ρE energy factor is applied at the SM11 kernel, never here — this module
//! is pure geometry (path lengths + on-segment validity).

use envi_engine::geometry::reflect_over_segment;

/// A reflecting façade face in the horizontal plan view: the segment `[a, b]`
/// (x, y) of a building wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    /// Segment start `[x, y]` (m).
    pub a: [f64; 2],
    /// Segment end `[x, y]` (m).
    pub b: [f64; 2],
}

/// A constructed reflection path S → (O₁ …) → R.
#[derive(Debug, Clone, PartialEq)]
pub struct FacadePath {
    /// Reflection order (1 = single façade, 2 = two façades).
    pub order: usize,
    /// The face indices (into the caller's face list) reflected off, in order.
    pub faces: Vec<usize>,
    /// Total path length `Σ legs` (m). Equals the straight-line image-source
    /// distance by construction.
    pub total_len_m: f64,
    /// The reflection points `O` in the (x, y) plane, in path order.
    pub points: Vec<[f64; 2]>,
    /// `true` iff every reflection point lies **within** its face segment (an
    /// off-segment reflection is flagged, never silently extrapolated).
    pub valid: bool,
}

/// Image of point `p` across the line containing face `[a, b]` (None for a
/// degenerate zero-length face).
fn image_over_face(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let e = [b[0] - a[0], b[1] - a[1]];
    let ee = e[0] * e[0] + e[1] * e[1];
    if ee < 1e-18 {
        return None;
    }
    let ap = [p[0] - a[0], p[1] - a[1]];
    let t = (ap[0] * e[0] + ap[1] * e[1]) / ee;
    let foot = [a[0] + t * e[0], a[1] + t * e[1]];
    Some([2.0 * foot[0] - p[0], 2.0 * foot[1] - p[1]])
}

/// First-order façade reflection paths S → O → R, one per reflecting face,
/// **excluding** `own_face` (the receiver's own façade — the free-field SPL
/// convention). Faces whose reflection is geometrically undefined are dropped;
/// an off-segment reflection is returned with `valid = false`.
#[must_use]
pub fn first_order_paths(
    source: [f64; 2],
    receiver: [f64; 2],
    faces: &[Face],
    own_face: Option<usize>,
) -> Vec<FacadePath> {
    let mut paths = Vec::new();
    for (i, face) in faces.iter().enumerate() {
        if own_face == Some(i) {
            continue; // own-façade reflection excluded
        }
        if let Some(refl) = reflect_over_segment(source, receiver, face.a, face.b) {
            paths.push(FacadePath {
                order: 1,
                faces: vec![i],
                total_len_m: refl.r1_m + refl.r2_m,
                points: vec![[
                    refl.point_x,
                    reflection_y(source, receiver, face, refl.point_x),
                ]],
                valid: refl.valid,
            });
        }
    }
    paths
}

/// Recover the reflection point's `y` coordinate from `point_x` on the face
/// line (the engine primitive reports only `point_x`; the face segment gives the
/// full point via linear parameterization).
fn reflection_y(_s: [f64; 2], _r: [f64; 2], face: &Face, point_x: f64) -> f64 {
    let dx = face.b[0] - face.a[0];
    if dx.abs() < 1e-12 {
        // Vertical face line in plan view (constant x): y is undetermined by x;
        // return the segment midpoint y as a representative.
        0.5 * (face.a[1] + face.b[1])
    } else {
        let t = (point_x - face.a[0]) / dx;
        face.a[1] + t * (face.b[1] - face.a[1])
    }
}

/// Second-order façade reflection paths S → O₁ (face `i`) → O₂ (face `j`) → R,
/// for every ordered pair of distinct faces (excluding `own_face`), built by
/// double image-source construction.
///
/// The total length is the straight-line distance from the twice-imaged source
/// to the receiver; the two reflection points are back-traced and each checked
/// for on-segment validity (`valid` is the AND of both).
#[must_use]
pub fn second_order_paths(
    source: [f64; 2],
    receiver: [f64; 2],
    faces: &[Face],
    own_face: Option<usize>,
) -> Vec<FacadePath> {
    let mut paths = Vec::new();
    for i in 0..faces.len() {
        for j in 0..faces.len() {
            if i == j || own_face == Some(i) || own_face == Some(j) {
                continue;
            }
            let fa = &faces[i];
            let fb = &faces[j];
            // Image of S over face i, then the second reflection off face j.
            let Some(s_img) = image_over_face(source, fa.a, fa.b) else {
                continue;
            };
            // O₂ on face j from the once-imaged source toward R.
            let Some(refl2) = reflect_over_segment(s_img, receiver, fb.a, fb.b) else {
                continue;
            };
            let o2 = [
                refl2.point_x,
                reflection_y(s_img, receiver, fb, refl2.point_x),
            ];
            // O₁ on face i from S toward O₂.
            let Some(refl1) = reflect_over_segment(source, o2, fa.a, fa.b) else {
                continue;
            };
            let o1 = [refl1.point_x, reflection_y(source, o2, fa, refl1.point_x)];
            let leg1 = dist(source, o1);
            let leg2 = dist(o1, o2);
            let leg3 = dist(o2, receiver);
            paths.push(FacadePath {
                order: 2,
                faces: vec![i, j],
                total_len_m: leg1 + leg2 + leg3,
                points: vec![o1, o2],
                valid: refl1.valid && refl2.valid,
            });
        }
    }
    paths
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A single wall parallel to the x-axis at y = 10; source and receiver on the
    // same side (y = 2 and y = 4). The first-order reflection is the mirror path.
    #[test]
    fn first_order_reflection_is_longer_than_the_direct_path_and_valid() {
        let face = Face {
            a: [-50.0, 10.0],
            b: [50.0, 10.0],
        };
        let source = [-5.0, 2.0];
        let receiver = [5.0, 4.0];
        let paths = first_order_paths(source, receiver, &[face], None);
        assert_eq!(paths.len(), 1);
        let p = &paths[0];
        assert!(p.valid, "reflection point must lie on the wall");
        // Reflected path length = distance from the image of S (y = 18) to R.
        let s_img = image_over_face(source, face.a, face.b).unwrap();
        let want = dist(s_img, receiver);
        assert!((p.total_len_m - want).abs() < 1e-9);
        assert!(
            p.total_len_m > dist(source, receiver),
            "reflection is longer"
        );
        // The reflection point sits on the wall (y = 10).
        assert!((p.points[0][1] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn own_facade_reflection_is_excluded() {
        let faces = [
            Face {
                a: [-50.0, 10.0],
                b: [50.0, 10.0],
            },
            Face {
                a: [-50.0, -10.0],
                b: [50.0, -10.0],
            },
        ];
        // Excluding face 0 leaves only the reflection off face 1.
        let paths = first_order_paths([-5.0, 2.0], [5.0, 4.0], &faces, Some(0));
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].faces, vec![1]);
    }

    #[test]
    fn off_segment_reflection_is_flagged_invalid() {
        // A short wall the mirror path misses ⇒ reflection point off-segment.
        let face = Face {
            a: [100.0, 10.0],
            b: [101.0, 10.0],
        };
        let paths = first_order_paths([-5.0, 2.0], [5.0, 4.0], &[face], None);
        assert_eq!(paths.len(), 1);
        assert!(
            !paths[0].valid,
            "the reflection point lies outside the short wall"
        );
    }

    // Two parallel facing walls (y = 10 and y = -10): a second-order path
    // bounces off both. Its length exceeds the direct path and both reflection
    // points are valid for a centred geometry.
    #[test]
    fn second_order_path_bounces_off_two_faces() {
        let faces = [
            Face {
                a: [-100.0, 10.0],
                b: [100.0, 10.0],
            },
            Face {
                a: [-100.0, -10.0],
                b: [100.0, -10.0],
            },
        ];
        let source = [-20.0, 0.0];
        let receiver = [20.0, 0.0];
        let paths = second_order_paths(source, receiver, &faces, None);
        // Two ordered pairs (0→1, 1→0).
        assert_eq!(paths.len(), 2);
        for p in &paths {
            assert_eq!(p.order, 2);
            assert_eq!(p.points.len(), 2);
            assert!(p.total_len_m > dist(source, receiver));
            assert!(p.total_len_m.is_finite());
        }
    }

    #[test]
    fn degenerate_face_is_dropped() {
        let face = Face {
            a: [0.0, 5.0],
            b: [0.0, 5.0], // zero length
        };
        assert!(first_order_paths([-5.0, 2.0], [5.0, 4.0], &[face], None).is_empty());
    }
}
