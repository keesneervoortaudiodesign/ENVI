//! Homogeneous ray variables (AV 1106/07 §5.5.4–5.5.6).
//!
//! The straight-ray specialization of Nord2000's ray machinery: direct and
//! image-source-reflected travel times, distances, grazing angle, and the
//! interference time difference `Δτ`. AV 1106/07 computes the homogeneous case
//! by clamping `|ξ| < 10⁻¹⁰` into the circular-ray machinery (p. 29); ENVI
//! implements the exact straight-ray limit instead.
//!
//! # The Phase 3 seam
//!
//! [`RayVars`]/[`RayPair`] are the struct the circular-ray (refracted)
//! constructor will fill in Phase 3 behind identical fields — design the seam
//! now (02-RESEARCH Pattern 2).
//!
//! # Numerics house rule (cancellation)
//!
//! `Δτ = τ₂ − τ₁` subtracts two nearly-equal travel times; the document itself
//! warns to "use the highest possible precision" (§5.5.6). Both constructors
//! compute `ΔR` via the **cancellation-free identity**
//! `ΔR = (R₂² − R₁²)/(R₂ + R₁)`, which for flat ground is `4·hS·hR/(R₁ + R₂)`
//! (CLAUDE.md numerics house rule; 02-RESEARCH §9).

use crate::geometry::reflect_over_segment;

use super::PropagationError;

/// One ray's variables: travel time, distance, grazing angle, and (for
/// reflected rays) the two partial legs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayVars {
    /// Travel time along the ray, seconds (`τ = R/c₀` homogeneous).
    pub tau: f64,
    /// Travel distance, meters.
    pub r: f64,
    /// Grazing angle at reflection, radians (reflected rays only; `0` for the
    /// direct ray).
    pub psi_g: f64,
    /// Incident partial leg S→reflection point, meters (reflected rays;
    /// `= r` for the direct ray).
    pub r1: f64,
    /// Reflected partial leg reflection point→R, meters (reflected rays;
    /// `= 0` for the direct ray).
    pub r2: f64,
}

/// A direct ray plus its optional ground reflection, with the cancellation-safe
/// interference time difference `Δτ = τ₂ − τ₁`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayPair {
    /// The direct S→R ray.
    pub direct: RayVars,
    /// The image-source-reflected ray, if a reflection exists.
    pub reflected: Option<RayVars>,
    /// `Δτ = τ₂ − τ₁` seconds, via the cancellation-free `ΔR` identity.
    pub dtau: f64,
}

/// Straight-ray variables for flat ground (AV 1106/07 §5.5.4–5.5.6 homogeneous
/// limit).
///
/// ```text
/// R₁ = √(d² + (hR − hS)²)      direct
/// R₂ = √(d² + (hR + hS)²)      image-source reflected
/// ψ_G = atan((hS + hR)/d)      grazing angle
/// ΔR = 4·hS·hR/(R₁ + R₂)       cancellation-free ; Δτ = ΔR/c₀
/// ```
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] for non-positive horizontal
/// distance, negative height, or non-finite input (threat T-02-01).
pub fn straight_rays(d: f64, h_s: f64, h_r: f64, c0: f64) -> Result<RayPair, PropagationError> {
    if !(d.is_finite() && d > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "horizontal distance d must be positive and finite",
        });
    }
    let height_ok = |h: f64| h.is_finite() && h >= 0.0;
    if !(height_ok(h_s) && height_ok(h_r)) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "source/receiver heights must be non-negative and finite",
        });
    }
    if !(c0.is_finite() && c0 > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "speed of sound c₀ must be positive and finite",
        });
    }

    let r1 = (d * d + (h_r - h_s).powi(2)).sqrt();
    let r2 = (d * d + (h_r + h_s).powi(2)).sqrt();
    let psi_g = (h_s + h_r).atan2(d);
    // Cancellation-free ΔR = (R₂²−R₁²)/(R₂+R₁) = 4·hS·hR/(R₁+R₂).
    let dr = 4.0 * h_s * h_r / (r1 + r2);
    let dtau = dr / c0;

    // Reflection point splits R₂ in proportion to the heights (similar
    // triangles): r1 : r2 = hS : hR. Guard the both-zero (grazing) case.
    let hsum = h_s + h_r;
    let (leg1, leg2) = if hsum > 0.0 {
        (r2 * h_s / hsum, r2 * h_r / hsum)
    } else {
        (0.0, r2)
    };

    Ok(RayPair {
        direct: RayVars {
            tau: r1 / c0,
            r: r1,
            psi_g: 0.0,
            r1,
            r2: 0.0,
        },
        reflected: Some(RayVars {
            tau: r2 / c0,
            r: r2,
            psi_g,
            r1: leg1,
            r2: leg2,
        }),
        dtau,
    })
}

/// Straight-ray variables over a (possibly sloped) ground segment, via
/// image-source coordinates.
///
/// Reuses [`reflect_over_segment`] for the reflection point / grazing angle /
/// partial legs, and computes `ΔR` without catastrophic cancellation from the
/// image point `S′`: `R₂² − R₁² = |S′R|² − |SR|² = (S′−S)·(S′+S−2R)`
/// (02-RESEARCH §9 sloped-segment guidance).
///
/// `s`, `r` are `[horizontal, height]` points in the vertical cut plane.
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] if no reflection is defined
/// (degenerate segment / parallel path), the source and receiver are
/// coincident, or any input is non-finite.
pub fn straight_rays_over_segment(
    s: [f64; 2],
    r: [f64; 2],
    seg_a: [f64; 2],
    seg_b: [f64; 2],
    c0: f64,
) -> Result<RayPair, PropagationError> {
    let finite2 = |p: [f64; 2]| p[0].is_finite() && p[1].is_finite();
    if !(finite2(s) && finite2(r) && finite2(seg_a) && finite2(seg_b)) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "ray endpoints must be finite",
        });
    }
    if !(c0.is_finite() && c0 > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "speed of sound c₀ must be positive and finite",
        });
    }

    let refl = reflect_over_segment(s, r, seg_a, seg_b).ok_or(
        PropagationError::DegenerateRayGeometry {
            detail: "no reflection defined (degenerate segment or parallel path)",
        },
    )?;

    let r1 = ((r[0] - s[0]).powi(2) + (r[1] - s[1]).powi(2)).sqrt();
    let r2 = refl.r1_m + refl.r2_m;
    if r1 <= 0.0 {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "source and receiver are coincident",
        });
    }

    // Image point S′ (reflection of S across the segment line), then the
    // cancellation-free ΔR = (|S′R|² − |SR|²)/(R₂+R₁) = (S′−S)·(S′+S−2R)/(R₂+R₁).
    let e = [seg_b[0] - seg_a[0], seg_b[1] - seg_a[1]];
    let ee = e[0] * e[0] + e[1] * e[1];
    let ap = [s[0] - seg_a[0], s[1] - seg_a[1]];
    let t = (ap[0] * e[0] + ap[1] * e[1]) / ee;
    let foot = [seg_a[0] + t * e[0], seg_a[1] + t * e[1]];
    let s_img = [2.0 * foot[0] - s[0], 2.0 * foot[1] - s[1]];
    let ds = [s_img[0] - s[0], s_img[1] - s[1]];
    let sum = [s_img[0] + s[0] - 2.0 * r[0], s_img[1] + s[1] - 2.0 * r[1]];
    let dr = (ds[0] * sum[0] + ds[1] * sum[1]) / (r2 + r1);
    let dtau = dr / c0;

    Ok(RayPair {
        direct: RayVars {
            tau: r1 / c0,
            r: r1,
            psi_g: 0.0,
            r1,
            r2: 0.0,
        },
        reflected: Some(RayVars {
            tau: r2 / c0,
            r: r2,
            psi_g: refl.grazing_angle_rad,
            r1: refl.r1_m,
            r2: refl.r2_m,
        }),
        dtau,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::sound_speed_ms;
    use approx::assert_relative_eq;

    // Test 1: geometry anchor hS=0.5, hR=1.5, d=97.5, c₀=340.348.
    #[test]
    fn straight_rays_match_research_geometry_anchor() {
        let c0 = sound_speed_ms(15.0);
        let pair = straight_rays(97.5, 0.5, 1.5, c0).unwrap();
        let refl = pair.reflected.expect("flat ground has a reflection");
        assert_relative_eq!(pair.direct.r, 97.505128070, max_relative = 1e-8);
        assert_relative_eq!(refl.r, 97.520510663, max_relative = 1e-8);
        assert_relative_eq!(refl.r - pair.direct.r, 1.538259287e-2, max_relative = 1e-7);
        assert_relative_eq!(pair.dtau, 4.519660952e-5, max_relative = 1e-8);
        assert_relative_eq!(refl.psi_g.to_degrees(), 1.175133, max_relative = 1e-5);
        // r1 + r2 == R2 (partial legs consistent)
        assert_relative_eq!(refl.r1 + refl.r2, refl.r, max_relative = 1e-12);
    }

    // Test 2: cancellation regression at hS=0.01, hR=1.5, d=1000.
    #[test]
    fn dtau_uses_cancellation_free_identity() {
        let c0 = sound_speed_ms(15.0);
        let (h_s, h_r, d) = (0.01_f64, 1.5_f64, 1000.0_f64);
        let pair = straight_rays(d, h_s, h_r, c0).unwrap();
        let r1 = (d * d + (h_r - h_s).powi(2)).sqrt();
        let r2 = (d * d + (h_r + h_s).powi(2)).sqrt();
        // R₂² − R₁² = 4·hS·hR exactly (expanded form) ⇒ ΔR = 4hShR/(R₁+R₂).
        let dr_identity = 4.0 * h_s * h_r / (r1 + r2);
        let dr_engine = pair.dtau * c0;
        assert_relative_eq!(dr_engine, dr_identity, max_relative = 1e-12);
        // The naive f64 subtraction loses precision here (documented magnitude).
        let dr_naive = r2 - r1;
        assert!(
            (dr_naive - dr_identity).abs() < 1e-12,
            "naive R₂−R₁ deviates from the identity by {:.2e} (the cancellation \
             the identity avoids)",
            (dr_naive - dr_identity).abs()
        );
    }

    // Test 3: sloped segment (01-02 anchor: reflection at x=5, path √104).
    #[test]
    fn sloped_segment_reflection_has_no_nan() {
        let c0 = sound_speed_ms(15.0);
        let pair =
            straight_rays_over_segment([0.0, 4.0], [6.0, 10.0], [0.0, 0.0], [10.0, 10.0], c0)
                .unwrap();
        let refl = pair.reflected.expect("valid reflection");
        assert_relative_eq!(refl.r, 104.0_f64.sqrt(), max_relative = 1e-12);
        assert!(pair.dtau.is_finite() && refl.r.is_finite());
        // ΔR consistent with the direct √72 vs reflected √104 lengths.
        assert_relative_eq!(
            pair.dtau * c0,
            104.0_f64.sqrt() - 72.0_f64.sqrt(),
            max_relative = 1e-10
        );
    }

    #[test]
    fn degenerate_inputs_are_typed_errors() {
        let c0 = sound_speed_ms(15.0);
        assert!(matches!(
            straight_rays(0.0, 0.5, 1.5, c0),
            Err(PropagationError::DegenerateRayGeometry { .. })
        ));
        assert!(matches!(
            straight_rays(f64::NAN, 0.5, 1.5, c0),
            Err(PropagationError::DegenerateRayGeometry { .. })
        ));
        assert!(matches!(
            straight_rays(100.0, -0.5, 1.5, c0),
            Err(PropagationError::DegenerateRayGeometry { .. })
        ));
    }
}
