//! Fresnel-zone machinery (AV 1106/07 §5.23.4–5.23.7, Eqs. 338–353).
//!
//! The weight machinery Sub-models 2–6 share: `CalcFZd` (the Fresnel-zone
//! ellipse distance, Eqs. 338–339), `FresnelZoneSize` (Eqs. 340–344),
//! `FresnelZoneW` (the low-frequency strip weight, Eqs. 345–351) and
//! `FresnelZoneWm` (the symmetric high-frequency variant, Eqs. 352–353).
//!
//! # `F_λ` is a parameter, never hard-coded
//!
//! The document parameterizes every zone by the product `F_λ·λ` where `F_λ` is
//! a fraction and `λ` the wavelength. Different consumers use different `F_λ`:
//! Sub-model 2 uses `F_λ = 0.25·λ` (Eq. 125), the screen sub-models `λ/16`
//! (Eq. 174), terrain interpretation `0.5·λ` (Eq. 304). These functions take the
//! product `f_lambda_prod = F_λ·λ` as an argument so callers pass their own
//! `F_λ` — the value is never baked in.
//!
//! # Homogeneous specialization
//!
//! The document's `FresnelZoneW`/`FresnelZoneWm` transform the curved-ray
//! (refracting) case into the straight-ray case (Eq. 348) before calling
//! `FresnelZoneSize`. Phase 2 is homogeneous (ξ = 0), so the straight-ray
//! positions are the actual geometry and the transform is the identity — the
//! reflection point sits at horizontal distance `d·hS/(hS+hR)` from the source.
//! Phase 3 attaches the circular-ray transform at the marked seam.
//!
//! Nord2000-native complex convention (e^{−jωt}); see [`super::special`]. All
//! functions are pure `f64`; degenerate geometry returns a typed
//! [`PropagationError`] and never a NaN (threat T-02-05).

use super::PropagationError;

/// `CalcFZd` — distance `d = |OP|` from the reflection point `O` to the elliptic
/// Fresnel-zone border in direction `θ` (AV 1106/07 §5.23.4, Eqs. 338–339).
///
/// `S` and `R` are the ellipse foci with `r_S = |SO|`, `r_R = |RO|`. The zone is
/// the locus where the extra path over the direct reflection equals `F_λ·λ`.
///
/// ```text
/// r = r_S + r_R
/// ℓ = r + F_λ·λ                                  (Eq. 339)
/// A = 4·(ℓ² − (r·cos θ)²)
/// B = 4·r·cos θ·(r_R² − r_S²) + 4·(r_S − r_R)·ℓ²·cos θ
/// C = −ℓ⁴ + 2·(r_S² + r_R²)·ℓ² − (r_S² − r_R²)²
/// d = (−B + √(B² − 4AC)) / (2A)
/// ```
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] if `r_S`/`r_R` are not positive
/// and finite, `f_lambda_prod` (= `F_λ·λ`) is not positive and finite, or the
/// ellipse is degenerate (`A ≈ 0` or negative discriminant).
pub fn calc_fz_d(
    r_s: f64,
    r_r: f64,
    theta: f64,
    f_lambda_prod: f64,
) -> Result<f64, PropagationError> {
    let _ = (r_s, r_r, theta, f_lambda_prod);
    todo!("Eq. 339")
}

/// `FresnelZoneSize` — the source-side (`a₁`), receiver-side (`a₂`) reach of the
/// Fresnel zone along the propagation direction, and half its width `b`, for a
/// reflecting plane (AV 1106/07 §5.23.5, Eqs. 340–344).
///
/// ```text
/// ψ_G = arctan((hS + hR)/d)                       (Eq. 344)
/// r   = √((hS + hR)² + d²)          (= R₂, image-source→receiver)
/// r_S = hS/(hS + hR)·r,  r_R = hR/(hS + hR)·r
/// a₁ = CalcFZd(r_S, r_R, π − ψ_G, F_λ·λ)          (Eq. 341, source side)
/// a₂ = CalcFZd(r_S, r_R, ψ_G,     F_λ·λ)          (Eq. 342, receiver side)
/// b  = √( CalcFZd(r_S, r_R, π/2, F_λ·λ)² / (1 − ((a₂−a₁)/(a₂+a₁))²) )   (Eq. 343)
/// ```
///
/// `d` is the horizontal image-source→receiver distance along the plane; `hS`,
/// `hR` are the source/receiver heights above it.
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] for non-positive/non-finite `d`,
/// heights, or `f_lambda_prod`, or a degenerate ellipse (propagated).
pub fn fresnel_zone_size(
    d: f64,
    h_s: f64,
    h_r: f64,
    f_lambda_prod: f64,
) -> Result<(f64, f64, f64), PropagationError> {
    let _ = (d, h_s, h_r, f_lambda_prod);
    todo!("Eqs. 340–344")
}

/// `FresnelZoneW` — the frequency-dependent low-frequency Fresnel-zone weight of
/// a ground strip (AV 1106/07 §5.23.6, Eqs. 345–351, homogeneous ξ = 0).
///
/// The weight is the fraction of the Fresnel zone (in the direction of
/// propagation) covered by the strip `[d₁, d₂]`:
///
/// ```text
/// d_refl   = d·hS/(hS + hR)                       reflection point (Eq. 348: R_S·cos ψ_G)
/// (a₁,a₂,_) = FresnelZoneSize(d, hS, hR, F_λ·λ)   (Eq. 349)
/// d_{1,Fz} = d_refl − a₁,  d_{2,Fz} = d_refl + a₂  (Eq. 350)
/// w(f)     = |[d₁,d₂] ∩ [d_{1,Fz}, d_{2,Fz}]| / (d_{2,Fz} − d_{1,Fz})   (Eq. 351)
/// ```
///
/// `d` is the horizontal source→receiver distance along the extended segment;
/// `hS`, `hR` are the source/receiver heights (clamped to ≥ 0.01 m per Eq. 345);
/// `d₁`, `d₂` are the horizontal distances from the source to the strip
/// endpoints. `w ∈ [0, 1]`.
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] for degenerate geometry.
pub fn fresnel_zone_w(
    d: f64,
    h_s: f64,
    h_r: f64,
    d1: f64,
    d2: f64,
    f_lambda_prod: f64,
) -> Result<f64, PropagationError> {
    let _ = (d, h_s, h_r, d1, d2, f_lambda_prod);
    todo!("Eqs. 345–351")
}

/// `FresnelZoneWm` — the modified (symmetric) high-frequency Fresnel-zone weight
/// (AV 1106/07 §5.23.7, Eqs. 352–353).
///
/// Same inputs as [`fresnel_zone_w`]; Eq. 351 is replaced by Eq. 353 so the
/// contribution on each side of the reflection point is the same size:
///
/// ```text
/// w(f) = 0.5·(w_S(f) + w_R(f))                    (Eq. 353)
/// w_S  = |[d₁,d₂] ∩ [d_{1,Fz}, d_refl]| / (d_refl − d_{1,Fz})   (source half)
/// w_R  = |[d₁,d₂] ∩ [d_refl, d_{2,Fz}]| / (d_{2,Fz} − d_refl)   (receiver half)
/// ```
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] for degenerate geometry.
pub fn fresnel_zone_wm(
    d: f64,
    h_s: f64,
    h_r: f64,
    d1: f64,
    d2: f64,
    f_lambda_prod: f64,
) -> Result<f64, PropagationError> {
    let _ = (d, h_s, h_r, d1, d2, f_lambda_prod);
    todo!("Eqs. 352–353")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const C0: f64 = 340.348;

    fn lambda(f: f64) -> f64 {
        C0 / f
    }

    // Test 1: degenerate guards — no NaN for any finite input; typed errors on
    // zero-length propagation, coincident projections, and F_λ = 0.
    #[test]
    fn degenerate_geometry_is_typed_errors_never_nan() {
        // F_λ = 0 → degenerate ellipse (zero excess path).
        assert!(calc_fz_d(50.0, 50.0, 0.5, 0.0).is_err());
        assert!(fresnel_zone_size(97.5, 0.5, 1.5, 0.0).is_err());
        assert!(fresnel_zone_w(97.5, 0.5, 1.5, 0.0, 97.5, 0.0).is_err());
        // Zero / negative horizontal distance.
        assert!(fresnel_zone_w(0.0, 0.5, 1.5, 0.0, 10.0, 0.25 * lambda(1000.0)).is_err());
        assert!(fresnel_zone_size(-1.0, 0.5, 1.5, 0.25 * lambda(1000.0)).is_err());
        // Non-finite input.
        assert!(fresnel_zone_w(f64::NAN, 0.5, 1.5, 0.0, 10.0, 0.25 * lambda(1000.0)).is_err());
        // Finite inputs across the FORCE band never produce NaN.
        for &f in &[25.0, 100.0, 646.7, 1000.0, 4000.0, 10000.0] {
            let flp = 0.25 * lambda(f);
            let w = fresnel_zone_w(97.5, 0.5, 1.5, 0.0, 50.0, flp).unwrap();
            let wm = fresnel_zone_wm(97.5, 0.5, 1.5, 0.0, 50.0, flp).unwrap();
            assert!(w.is_finite() && wm.is_finite(), "f={f}: w={w} wm={wm}");
        }
    }

    // Test 2: weight bounds — w ∈ [0, 1]; a strip covering the whole plane gets
    // full weight; a strip far outside the zone gets zero.
    #[test]
    fn weights_are_bounded_and_track_coverage() {
        let flp = 0.25 * lambda(1000.0);
        // Interior partial strip → in [0, 1].
        let w = fresnel_zone_w(97.5, 0.5, 1.5, 40.0, 60.0, flp).unwrap();
        assert!((0.0..=1.0).contains(&w), "w={w}");
        // Whole plane → full weight.
        let full = fresnel_zone_w(97.5, 0.5, 1.5, -1.0e6, 1.0e6, flp).unwrap();
        assert!((full - 1.0).abs() < 1e-9, "covering strip must weight ~1: {full}");
        // Far outside the zone → zero.
        let none = fresnel_zone_w(97.5, 0.5, 1.5, 1.0e5, 2.0e5, flp).unwrap();
        assert!(none.abs() < 1e-12, "far strip must weight 0: {none}");
        // FresnelZoneWm bounded and full for a covering strip too.
        let wm_full = fresnel_zone_wm(97.5, 0.5, 1.5, -1.0e6, 1.0e6, flp).unwrap();
        assert!((wm_full - 1.0).abs() < 1e-9, "wm covering must weight ~1: {wm_full}");
    }

    // Test 3: frequency behaviour — the zone shrinks with frequency; a smaller
    // F_λ fraction gives a smaller zone for identical geometry.
    #[test]
    fn zone_shrinks_with_frequency_and_with_smaller_f_lambda() {
        // Zone reach a₁+a₂ decreases as frequency rises (λ, hence F_λ·λ, shrinks).
        let (a1_lo, a2_lo, _) = fresnel_zone_size(97.5, 0.5, 1.5, 0.25 * lambda(250.0)).unwrap();
        let (a1_hi, a2_hi, _) = fresnel_zone_size(97.5, 0.5, 1.5, 0.25 * lambda(4000.0)).unwrap();
        assert!(
            a1_hi + a2_hi < a1_lo + a2_lo,
            "zone must shrink with frequency: {} !< {}",
            a1_hi + a2_hi,
            a1_lo + a2_lo
        );
        // A strip sitting at the zone edge loses weight as frequency rises.
        let w_lo = fresnel_zone_w(97.5, 0.5, 1.5, 20.0, 30.0, 0.25 * lambda(250.0)).unwrap();
        let w_hi = fresnel_zone_w(97.5, 0.5, 1.5, 20.0, 30.0, 0.25 * lambda(4000.0)).unwrap();
        assert!(w_hi <= w_lo + 1e-12, "edge strip weight must not grow: {w_hi} vs {w_lo}");
        // Zone at F_λ = λ/16 is smaller than at F_λ = λ/4.
        let lam = lambda(1000.0);
        let (a1_s, a2_s, _) = fresnel_zone_size(97.5, 0.5, 1.5, lam / 16.0).unwrap();
        let (a1_b, a2_b, _) = fresnel_zone_size(97.5, 0.5, 1.5, 0.25 * lam).unwrap();
        assert!(a1_s + a2_s < a1_b + a2_b, "λ/16 zone must be smaller than λ/4");
    }

    // Test 4: symmetry / reciprocity — hS == hR puts the zone centre at the
    // midpoint (a₁ == a₂); swapping S and R with a mirrored strip is invariant.
    #[test]
    fn symmetric_geometry_centres_the_zone_and_swap_is_reciprocal() {
        let flp = 0.25 * lambda(1000.0);
        // hS == hR → symmetric ellipse, a₁ == a₂, reflection point at d/2.
        let (a1, a2, _) = fresnel_zone_size(100.0, 1.0, 1.0, flp).unwrap();
        assert!((a1 - a2).abs() < 1e-9, "symmetric geometry: a₁={a1} a₂={a2}");
        // Swapping S↔R with a mirrored strip leaves the weight unchanged.
        let (d, h_s, h_r) = (97.5, 0.5, 1.5);
        let (d1, d2) = (20.0, 35.0);
        let w = fresnel_zone_w(d, h_s, h_r, d1, d2, flp).unwrap();
        let w_swap = fresnel_zone_w(d, h_r, h_s, d - d2, d - d1, flp).unwrap();
        assert!((w - w_swap).abs() < 1e-9, "reciprocity: {w} vs {w_swap}");
    }

    // CalcFZd direction sanity: the π/2 (perpendicular) reach is positive and
    // the source/receiver reaches are positive for the anchor geometry.
    #[test]
    fn calc_fz_d_reaches_are_positive() {
        let flp = 0.25 * lambda(1000.0);
        let r = ((0.5f64 + 1.5).powi(2) + 97.5f64.powi(2)).sqrt();
        let (r_s, r_r) = (0.5 / 2.0 * r, 1.5 / 2.0 * r);
        let psi_g = ((0.5f64 + 1.5) / 97.5).atan();
        assert!(calc_fz_d(r_s, r_r, psi_g, flp).unwrap() > 0.0);
        assert!(calc_fz_d(r_s, r_r, PI - psi_g, flp).unwrap() > 0.0);
        assert!(calc_fz_d(r_s, r_r, PI / 2.0, flp).unwrap() > 0.0);
    }
}
