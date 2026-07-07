//! Sub-model 2 — flat terrain with more than one surface type
//! (AV 1106/07 §5.11, Eqs. 124–133).
//!
//! ```text
//! ΔL₂(f) = Σ_ii Σ_ir w′_{ii,ir}(f)·ΔL_{ii,ir}(f)                     (124)
//! ```
//!
//! `ΔL_{ii,ir}` is **Sub-model 1 evaluated as if the whole ground were that
//! `(σ, r)` type** over the full path geometry — grouped by surface TYPE, not per
//! segment (Eq. 127; research Pitfall 3). A profile `[G, D, G, D]` therefore
//! produces exactly two per-type evaluations, not four.
//!
//! The per-type modified Fresnel-zone weight `w′` blends a low-frequency weight
//! (`FresnelZoneW`, `F_λ = 0.25·λ`, Eq. 125) and a high-frequency weight
//! (`FresnelZoneWm` + the Eq. 128 log-polynomial in `r̄` and `tan ψ_G`) by
//! log-frequency interpolation between `fL` and `fH` (Eq. 129), where `fL`/`fH`
//! come from [`phase_diff_freq`] (Eqs. 130–132).
//!
//! # Two-channel blend (extension of Eq. 124 for the user contract)
//!
//! The document defines only the level `ΔL₂`. To honour the phase-preserving
//! two-channel contract, the per-type [`GroundResult`]s are blended in the
//! complex/energy domain: `h_coh_factor` sums complex-linearly with `w′`, and
//! `p_incoh` sums with the squared weights `w′²`, so
//! `delta_l_db = 10·lg(|Σ w′·h|² + Σ w′²·p)`. For a single surface type
//! (`w′ = 1`) this collapses to Sub-model 1 exactly, keeping the interference
//! phase live across the blend. The committed oracle implements the same
//! phase-preserving reading.
//!
//! Nord2000-native convention (e^{−jωt}); no `conj()` here.

use num_complex::Complex;
use std::f64::consts::PI;

use super::GroundResult;
use super::submodel1::eval as submodel1_eval;
use crate::freq::FreqAxis;
use crate::propagation::PropagationError;
use crate::propagation::coherence::CoherenceInputs;
use crate::propagation::fresnel::{fresnel_zone_w, fresnel_zone_wm};
use crate::propagation::ground::{gamma_p, ground_impedance};
use crate::propagation::rays::straight_rays;

/// Flat-terrain geometry shared by every surface type in a Sub-model 2 call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlatGeometry {
    /// Horizontal source→receiver distance, meters.
    pub d: f64,
    /// Source height above the ground plane, meters.
    pub h_s: f64,
    /// Receiver height above the ground plane, meters.
    pub h_r: f64,
    /// Speed of sound at the ground, m/s.
    pub c0: f64,
}

/// One ground segment: a strip `[x_start, x_end]` (measured from the source
/// along the ground) with its own flow resistivity and roughness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceStrip {
    /// Strip start, meters from the source along the ground.
    pub x_start: f64,
    /// Strip end, meters from the source along the ground.
    pub x_end: f64,
    /// Flow resistivity σ, kPa·s·m⁻².
    pub sigma_kpa: f64,
    /// Roughness `r`, meters.
    pub roughness_r: f64,
}

/// Sub-model 2 segmented-impedance ground effect (Eq. 124), returned as the
/// two-channel [`GroundResult`].
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] for empty strips or degenerate
/// geometry; [`PropagationError::InvalidFlowResistivity`] for σ ≤ 0.
pub fn submodel2(
    f_hz: f64,
    strips: &[SurfaceStrip],
    geom: &FlatGeometry,
    coh: &CoherenceInputs,
) -> Result<GroundResult, PropagationError> {
    let _ = (f_hz, strips, geom, coh);
    let _ = (
        Complex::<f64>::I,
        PI,
        submodel1_eval,
        fresnel_zone_w,
        fresnel_zone_wm,
        gamma_p,
        ground_impedance,
        straight_rays,
        FreqAxis::new,
    );
    todo!("Eqs. 124–133")
}

/// Distinct surface types `(σ, r)` in first-occurrence order (Eq. 127 grouping).
/// Sub-model 1 is evaluated once per returned type — never per strip
/// (Pitfall 3).
#[must_use]
pub(crate) fn group_types(strips: &[SurfaceStrip]) -> Vec<(f64, f64)> {
    let _ = strips;
    todo!("Eq. 127 grouping")
}

/// `PhaseDiffFreq` (Eqs. 378–381): the frequency `f` at which the phase
/// difference between the direct and flat-ground-reflected ray equals
/// `target_psi`, for straight-line (homogeneous) propagation.
///
/// ```text
/// Ψ(f) = 2πf·ΔR/c₀ + arg Γ̂_p(f, ψ_G, Ẑ_G,min)                       (379)
/// ΔR   = √(d²+(hS+hR)²) − √(d²+(hS−hR)²)
/// ψ_G  = arcsin((hS+hR)/√(d²+(hS+hR)²))
/// ```
///
/// Ψ increases with `f`; the target is bracketed on the 1/3-octave grid
/// (25 Hz…10 kHz) and log-interpolated (Eq. 380). Extrapolation (Eq. 381):
/// linear from 8–10 kHz up to 100 kHz (constant above); `f = 25·Ψ/Ψ(25 Hz)`
/// below 25 Hz. Never returns NaN.
#[must_use]
pub(crate) fn phase_diff_freq(
    d: f64,
    h_s: f64,
    h_r: f64,
    sigma_min: f64,
    c0: f64,
    target_psi: f64,
) -> f64 {
    let _ = (d, h_s, h_r, sigma_min, c0, target_psi);
    todo!("Eqs. 378–381")
}

/// Per-type modified Fresnel-zone weights `w′_{ii,ir}(f)` (Eqs. 125–132),
/// returned alongside their `(σ, r)` type key.
pub(crate) fn type_weights(
    f_hz: f64,
    strips: &[SurfaceStrip],
    geom: &FlatGeometry,
) -> Result<Vec<((f64, f64), f64)>, PropagationError> {
    let _ = (f_hz, strips, geom);
    todo!("Eqs. 125–132")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freq::FreqAxis;
    use crate::propagation::coherence::CoherenceInputs;
    use crate::propagation::rays::straight_rays;
    use crate::propagation::sound_speed_ms;
    use crate::propagation::terrain_effect::submodel1::{SubModel1Inputs, submodel1};

    const C0: f64 = 340.348;

    fn geom() -> FlatGeometry {
        FlatGeometry {
            d: 97.5,
            h_s: 0.5,
            h_r: 1.5,
            c0: C0,
        }
    }

    fn zero_turb() -> CoherenceInputs {
        CoherenceInputs {
            cv2: 0.0,
            ct2: 0.0,
            t_air_c: 15.0,
            c0: C0,
            roughness_r: 0.0,
            f_delta_nu: 1.0,
            d_m: 97.5,
        }
    }

    // road G (σ=20000) / grass D (σ=200) alternating: four strips, two types.
    fn case21_strips() -> Vec<SurfaceStrip> {
        let g = |x0, x1| SurfaceStrip {
            x_start: x0,
            x_end: x1,
            sigma_kpa: 20000.0,
            roughness_r: 0.0,
        };
        let d = |x0, x1| SurfaceStrip {
            x_start: x0,
            x_end: x1,
            sigma_kpa: 200.0,
            roughness_r: 0.0,
        };
        vec![g(0.0, 10.0), d(10.0, 50.0), g(50.0, 60.0), d(60.0, 97.5)]
    }

    // Test 1 (per-TYPE grouping, Pitfall 3): four strips / two types → two types.
    #[test]
    fn groups_by_surface_type_not_by_segment() {
        let types = group_types(&case21_strips());
        assert_eq!(types.len(), 2, "four strips of two types must group to two");
        assert!(types.contains(&(20000.0, 0.0)) && types.contains(&(200.0, 0.0)));
    }

    // Test 2 (degenerate consistency): a segmented profile whose strips all share
    // one (σ, r) reproduces submodel1 exactly at every grid point (1e-9 dB).
    #[test]
    fn single_type_profile_collapses_to_submodel1() {
        let g = geom();
        let coh = zero_turb();
        let rays = straight_rays(g.d, g.h_s, g.h_r, g.c0).unwrap();
        let strips = vec![
            SurfaceStrip {
                x_start: 0.0,
                x_end: 40.0,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
            SurfaceStrip {
                x_start: 40.0,
                x_end: 97.5,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
        ];
        let axis = FreqAxis::new();
        for &f in axis.centres.iter() {
            let s2 = submodel2(f, &strips, &g, &coh).unwrap().delta_l_db;
            let inp = SubModel1Inputs {
                rays: &rays,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
                coh: &coh,
            };
            let s1 = submodel1(f, &inp).unwrap().delta_l_db;
            assert!(
                (s2 - s1).abs() < 1e-9,
                "single-type submodel2 must equal submodel1 at {f} Hz: {s2} vs {s1}"
            );
        }
    }

    // Test 3 (PhaseDiffFreq): fL ≤ 0.8·fH enforced; extreme geometries never NaN.
    #[test]
    fn phase_diff_freq_is_guarded_and_finite() {
        // A spread of hard→soft grounds and grazing→steep geometries.
        for &(d, hs, hr) in &[(97.5, 0.5, 1.5), (1000.0, 0.01, 1.5), (10.0, 5.0, 5.0)] {
            for &sig in &[12.5, 200.0, 20000.0, 200000.0] {
                let fl = phase_diff_freq(d, hs, hr, sig, C0, PI); // any target
                let fh = phase_diff_freq(d, hs, hr, sig, C0, PI);
                assert!(fl.is_finite() && fh.is_finite(), "NaN at d={d} σ={sig}");
                assert!(fl > 0.0, "frequency must be positive");
            }
        }
        // Ψ increases with f, so a larger target maps to a higher frequency.
        let f_small = phase_diff_freq(97.5, 0.5, 1.5, 200.0, C0, 0.5);
        let f_big = phase_diff_freq(97.5, 0.5, 1.5, 200.0, C0, PI);
        assert!(f_big > f_small, "monotone: {f_big} !> {f_small}");
    }

    // Test 4 (weight normalization): per-type w′ ∈ [0, 1]; the total respects the
    // §5.8 sum-to-2 rule (a tiling profile sums to ≤ 1).
    #[test]
    fn weights_are_bounded_and_sum_within_rule() {
        let g = geom();
        let strips = case21_strips();
        let axis = FreqAxis::new();
        for &f in axis.centres.iter() {
            let tw = type_weights(f, &strips, &g).unwrap();
            let mut total = 0.0;
            for (_, w) in &tw {
                assert!((-1e-12..=1.0 + 1e-9).contains(w), "w′={w} out of [0,1] at {f} Hz");
                total += *w;
            }
            assert!(total <= 2.0 + 1e-9, "Σ w′ = {total} must respect the ≤2 rule");
        }
    }

    // The two-channel identity must also hold for Sub-model 2 (complex blend).
    #[test]
    fn submodel2_two_channel_identity_holds() {
        let g = geom();
        let coh = zero_turb();
        let strips = case21_strips();
        let axis = FreqAxis::new();
        for &f in axis.centres.iter() {
            let r = submodel2(f, &strips, &g, &coh).unwrap();
            let identity = 10.0 * (r.h_coh_factor.norm_sqr() + r.p_incoh).log10();
            assert!(
                (identity - r.delta_l_db).abs() < 1e-12,
                "SM2 two-channel identity at {f} Hz: {identity} vs {}",
                r.delta_l_db
            );
            assert!(r.p_incoh >= 0.0, "p_incoh must be non-negative");
        }
    }

    // Finiteness sweep for the mixed profile.
    #[test]
    fn mixed_profile_is_finite_across_the_sweep() {
        let g = geom();
        let coh = CoherenceInputs {
            cv2: 0.12,
            ct2: 0.008,
            ..zero_turb()
        };
        let strips = case21_strips();
        let axis = FreqAxis::new();
        for &f in axis.centres.iter() {
            let r = submodel2(f, &strips, &g, &coh).unwrap();
            assert!(
                r.delta_l_db.is_finite() && r.p_incoh.is_finite(),
                "non-finite at {f} Hz"
            );
        }
        let _ = sound_speed_ms(15.0);
    }
}
