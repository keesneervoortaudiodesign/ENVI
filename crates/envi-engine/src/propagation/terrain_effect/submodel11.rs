//! Sub-model 11 — reflection effect (AV 1106/07 §5.20, Eqs. 296–299 + the
//! `CalcFZd` auxiliary, Eq. 339).
//!
//! Sound reflected from an obstacle (building façade, noise screen) is handled
//! by adding a reflection path S → O → R. Its propagation effect is predicted by
//! the SAME propagation model used for the direct path (the harness builds the
//! image-source path and runs the standard chain); Sub-model 11 supplies the
//! extra **reflection-efficiency correction** `L11` of Eq. (296):
//!
//! ```text
//! L11 = 10·log10(ρE(f)) + 20·log10(Srefl(f) / SFz(f))            (296)
//! ρE(f) = 1 − α(f)                                               (297)
//! ```
//!
//! The first term corrects for the effective **energy reflection coefficient**
//! `ρE` (`α` = surface absorption); the second for the **"effective" size** of
//! the reflector — the ratio of the reflector area within the Fresnel-zone
//! (`Srefl`) to the whole Fresnel-zone area (`SFz`). A reflector is **ignored**
//! (no reflection path) if the reflection point O is more than 2 m outside one
//! of its edges.
//!
//! # Energy-only by construction (phase channel untouched)
//!
//! `L11` is a real dB correction (a **magnitude** efficiency factor) applied to
//! the already-complex reflection path — exactly like the directivity factor or
//! Sub-model 7/10 energy terms. This module returns `f64`; it is *structurally*
//! incapable of corrupting the coherent phase channel, and there is **no
//! `conj()`** here (the single convention boundary stays in `transfer.rs`).
//!
//! # Transcription (house rule)
//!
//! Eqs. 296/297 and the `CalcFZd` closed form (Eq. 339) were transcribed from
//! the AV 1106/07 rev.4 PDF **page images** (pp. 129–130 and p. 146), not the
//! garbled `pdftotext` extraction — the quartic `C` term is `−(rS²−rR²)²`. The
//! Fresnel-zone rectangle simplification (circumscribed rectangle around the
//! ellipse, §5.8) is used, valid for the vertical-reflector case the standard
//! itself restricts §5.20 to. The scipy oracle cross-checks the arithmetic;
//! per the standing oracle-independence caveat it does NOT independently verify
//! the spec reading (there is no FORCE numeric gate on SM11 — the city cases
//! stay `Skipped` on the [ASSUMED] emission coefficients).

use crate::propagation::PropagationError;

/// The Fresnel fraction `F` used for reflection efficiency (§5.8: `F = 1/8` for
/// reflections by vertically-erected surfaces).
pub const F_REFLECTION: f64 = 1.0 / 8.0;

/// A dB value standing in for "no reflection contribution" (`10^{-300/10} ≈ 0`),
/// kept finite so callers never propagate `-inf` through an energy sum.
pub const NO_REFLECTION_DB: f64 = -300.0;

/// Edge tolerance (m): a reflector whose reflection point O lies more than this
/// far outside an edge is ignored (AV 1106/07 §5.20).
pub const EDGE_TOLERANCE_M: f64 = 2.0;

/// The energy reflection coefficient `ρE = 1 − α` (Eq. 297), clamped to
/// `[0, 1]` (a physical reflector neither absorbs negative energy nor amplifies).
#[must_use]
pub fn energy_reflection_coefficient(alpha: f64) -> f64 {
    (1.0 - alpha).clamp(0.0, 1.0)
}

/// `CalcFZd` (AV 1106/07 Eq. 339): the distance `d = |OP|` from the reflection
/// point O to the elliptic Fresnel-zone border in the direction making angle
/// `theta` with the major axis, for foci at O→S (`r_s`) and O→R (`r_r`) and
/// `f_lambda_lambda = Fλ·λ`.
///
/// ```text
/// r = rS + rR ;  ℓ = r + Fλλ
/// A = 4·(ℓ² − (r·cosθ)²)
/// B = 4·r·cosθ·(rR² − rS²) + 4·(rS − rR)·ℓ²·cosθ
/// C = −ℓ⁴ + 2·(rS² + rR²)·ℓ² − (rS² − rR²)²
/// d = (−B + √(B² − 4AC)) / (2A)
/// ```
///
/// Returns `None` for a degenerate (`A ≈ 0`) or non-physical (negative
/// discriminant) configuration rather than producing a NaN.
#[must_use]
pub fn calc_fz_d(r_s: f64, r_r: f64, theta: f64, f_lambda_lambda: f64) -> Option<f64> {
    if !(r_s.is_finite() && r_r.is_finite() && f_lambda_lambda.is_finite()) {
        return None;
    }
    let r = r_s + r_r;
    let l = r + f_lambda_lambda;
    let cos = theta.cos();
    let a = 4.0 * (l * l - (r * cos).powi(2));
    let b = 4.0 * r * cos * (r_r * r_r - r_s * r_s) + 4.0 * (r_s - r_r) * l * l * cos;
    let c = -l.powi(4) + 2.0 * (r_s * r_s + r_r * r_r) * l * l - (r_s * r_s - r_r * r_r).powi(2);
    if a.abs() < 1e-12 {
        return None;
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let d = (-b + disc.sqrt()) / (2.0 * a);
    d.is_finite().then_some(d)
}

/// Inputs to the Sub-model 11 reflection-efficiency correction.
///
/// The reflector extent is given relative to the reflection point O, in the
/// plane of the (vertical) reflector: `[−half_left, +half_right]` along the
/// façade and `[−half_down, +half_up]` vertically. All extents are the signed
/// distances from O to the corresponding edge (a negative value means O lies
/// *outside* that edge).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReflectionConfig {
    /// Reflection-path leg length O→S (m).
    pub r_s: f64,
    /// Reflection-path leg length O→R (m).
    pub r_r: f64,
    /// Surface absorption coefficient `α(f)` (ρE = 1 − α); FORCE city uses
    /// ρE = 1.0 (α = 0) and 0.7 (α = 0.3).
    pub alpha: f64,
    /// Sound speed `c0` (m/s) — with the frequency it gives the wavelength.
    pub c0: f64,
    /// Distance O → upper reflector edge (m; negative if O is above the top).
    pub half_up: f64,
    /// Distance O → lower reflector edge (m; negative if O is below the bottom).
    pub half_down: f64,
    /// Distance O → left reflector edge along the façade (m).
    pub half_left: f64,
    /// Distance O → right reflector edge along the façade (m).
    pub half_right: f64,
}

/// The Sub-model 11 reflection-efficiency correction `L11` (dB, `≤ 0`) at
/// frequency `f_hz` (AV 1106/07 Eq. 296).
///
/// The Fresnel-zone half-width perpendicular to the ray, `b = CalcFZd(rS, rR,
/// π/2, Fλ)` (§5.8 circumscribed-rectangle simplification, symmetric minor axis
/// for a vertical reflector), gives the Fresnel rectangle `SFz = (2b)²`; the
/// reflector-in-Fresnel area `Srefl` is the overlap of the reflector rectangle
/// with it. Returns [`NO_REFLECTION_DB`] when the reflector is more than
/// [`EDGE_TOLERANCE_M`] outside the Fresnel rectangle (the reflector is ignored)
/// or the overlap is empty.
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] if `rS`/`rR`/`f_hz`/`c0` are not
/// positive and finite.
pub fn submodel11(f_hz: f64, cfg: &ReflectionConfig) -> Result<f64, PropagationError> {
    if !(cfg.r_s.is_finite()
        && cfg.r_s > 0.0
        && cfg.r_r.is_finite()
        && cfg.r_r > 0.0
        && f_hz.is_finite()
        && f_hz > 0.0
        && cfg.c0.is_finite()
        && cfg.c0 > 0.0)
    {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "Sub-model 11 requires positive finite rS, rR, f, c0",
        });
    }

    // Reflector fully outside the surface by more than the 2 m tolerance ⇒
    // ignored (no reflection path).
    if cfg.half_up < -EDGE_TOLERANCE_M
        || cfg.half_down < -EDGE_TOLERANCE_M
        || cfg.half_left < -EDGE_TOLERANCE_M
        || cfg.half_right < -EDGE_TOLERANCE_M
    {
        return Ok(NO_REFLECTION_DB);
    }

    let lambda = cfg.c0 / f_hz;
    let f_lambda_lambda = F_REFLECTION * lambda;
    // Fresnel half-width perpendicular to the ray (minor-axis half-width).
    let Some(b) = calc_fz_d(
        cfg.r_s,
        cfg.r_r,
        std::f64::consts::FRAC_PI_2,
        f_lambda_lambda,
    ) else {
        return Ok(NO_REFLECTION_DB);
    };
    if !(b.is_finite() && b > 0.0) {
        return Ok(NO_REFLECTION_DB);
    }

    // Overlap of the reflector rectangle with the Fresnel rectangle [−b, b]².
    let vert = (cfg.half_up.min(b).max(0.0)) + (cfg.half_down.min(b).max(0.0));
    let horiz = (cfg.half_left.min(b).max(0.0)) + (cfg.half_right.min(b).max(0.0));
    let s_refl = vert * horiz;
    let s_fz = (2.0 * b) * (2.0 * b);
    if !(s_refl > 0.0 && s_fz > 0.0) {
        return Ok(NO_REFLECTION_DB);
    }
    let ratio = (s_refl / s_fz).clamp(0.0, 1.0);

    let rho_e = energy_reflection_coefficient(cfg.alpha);
    if rho_e <= 0.0 {
        return Ok(NO_REFLECTION_DB);
    }

    // Eq. 296. Both terms are ≤ 0 (ρE ≤ 1, ratio ≤ 1), so a reflection never
    // adds more than a perfect infinite mirror.
    Ok(10.0 * rho_e.log10() + 20.0 * ratio.log10())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cfg() -> ReflectionConfig {
        // A large, near-perfect reflector (edges well beyond the Fresnel zone),
        // ρE = 1: the size term → 0 dB, so L11 → 0 dB.
        ReflectionConfig {
            r_s: 20.0,
            r_r: 80.0,
            alpha: 0.0,
            c0: 340.0,
            half_up: 100.0,
            half_down: 100.0,
            half_left: 1000.0,
            half_right: 1000.0,
        }
    }

    #[test]
    fn energy_reflection_coefficient_is_one_minus_alpha_clamped() {
        assert!((energy_reflection_coefficient(0.0) - 1.0).abs() < 1e-12);
        assert!((energy_reflection_coefficient(0.3) - 0.7).abs() < 1e-12);
        assert_eq!(energy_reflection_coefficient(-1.0), 1.0); // clamp high
        assert_eq!(energy_reflection_coefficient(2.0), 0.0); // clamp low
    }

    #[test]
    fn calc_fz_d_is_positive_and_grows_with_wavelength() {
        let d1 = calc_fz_d(20.0, 80.0, std::f64::consts::FRAC_PI_2, F_REFLECTION * 0.34).unwrap();
        let d2 = calc_fz_d(20.0, 80.0, std::f64::consts::FRAC_PI_2, F_REFLECTION * 3.4).unwrap();
        assert!(d1 > 0.0 && d2 > 0.0);
        // A longer wavelength (lower frequency) ⇒ a larger Fresnel zone.
        assert!(d2 > d1, "Fresnel half-width must grow with λ: {d1} → {d2}");
    }

    #[test]
    fn large_perfect_reflector_gives_zero_correction() {
        let l11 = submodel11(500.0, &base_cfg()).unwrap();
        assert!(
            l11.abs() < 1e-9,
            "perfect infinite mirror ⇒ 0 dB, got {l11}"
        );
    }

    #[test]
    fn absorption_lowers_the_correction_by_ten_log_rho_e() {
        // ρE = 0.7 ⇒ the energy term is exactly 10·lg(0.7) ≈ −1.549 dB (size term
        // still 0 for the large reflector).
        let mut cfg = base_cfg();
        cfg.alpha = 0.3;
        let l11 = submodel11(500.0, &cfg).unwrap();
        assert!(
            (l11 - 10.0 * 0.7_f64.log10()).abs() < 1e-9,
            "ρE=0.7 ⇒ 10·lg(0.7): {l11}"
        );
    }

    #[test]
    fn small_reflector_attenuates_via_the_size_term() {
        // A reflector smaller than the Fresnel zone loses efficiency: L11 < the
        // large-reflector value.
        let mut cfg = base_cfg();
        cfg.half_up = 0.2;
        cfg.half_down = 0.2;
        cfg.half_left = 0.2;
        cfg.half_right = 0.2;
        let small = submodel11(500.0, &cfg).unwrap();
        let large = submodel11(500.0, &base_cfg()).unwrap();
        assert!(
            small < large - 1.0,
            "a sub-Fresnel reflector must attenuate: small={small} large={large}"
        );
        assert!(small.is_finite());
    }

    #[test]
    fn reflector_outside_the_edge_tolerance_is_ignored() {
        let mut cfg = base_cfg();
        cfg.half_up = -3.0; // O is 3 m above the reflector top (> 2 m)
        let l11 = submodel11(500.0, &cfg).unwrap();
        assert_eq!(l11, NO_REFLECTION_DB, "ignored reflector ⇒ sentinel");
    }

    #[test]
    fn degenerate_geometry_is_a_typed_error() {
        let mut cfg = base_cfg();
        cfg.r_s = 0.0;
        assert!(matches!(
            submodel11(500.0, &cfg),
            Err(PropagationError::DegenerateRayGeometry { .. })
        ));
        assert!(matches!(
            submodel11(f64::NAN, &base_cfg()),
            Err(PropagationError::DegenerateRayGeometry { .. })
        ));
    }
}
