//! Sub-model 1 — flat terrain, one surface type (AV 1106/07 §5.10, Eqs. 115–123).
//!
//! The homogeneous (straight-ray, ξ = 0) specialization of the partially-coherent
//! two-ray sum. Eq. 120:
//!
//! ```text
//! ΔL_flat(f) = 10·lg( |1 + F·(R₁/R₂)·e^{+j2πfΔτ}·Q̂(f,ψ_G,τ₂,Ẑ_G)|²
//!                    + (1 − F²)·(R₁/R₂)²·ρᵢ(f,Ẑ_G)² )
//! ```
//!
//! The `e^{+j2πfΔτ}` sign is **Nord2000-native** (e^{−jωt} time convention); the
//! single conjugation to ENVI's transfer convention is plan 02-05's job — there
//! is no `conj()` here. The `10·lg` prefix of Eq. 120 was confirmed against the
//! PDF page image during transcription (research Assumption A2 resolved).
//!
//! # Two channels
//!
//! The coherent term `1 + F·(R₁/R₂)·e^{+j2πfΔτ}·Q̂` is the live-phase
//! [`GroundResult::h_coh_factor`]; the `(1−F²)·(R₁/R₂)²·ρᵢ²` residual is the real
//! [`GroundResult::p_incoh`]. See the [module docs](super) for the contract.
//!
//! # Shadow zone (Eqs. 121–122) — Phase 3 seam
//!
//! The shadow-zone branch needs downward refraction (ξ < 0). The homogeneous
//! engine has `dSZ = ∞` and always returns the non-shadow branch; Phase 3
//! attaches the `ShadowZoneShielding` term at the documented seam.

use super::GroundResult;
use crate::propagation::PropagationError;
use crate::propagation::coherence::{CoherenceInputs, coherence_f};
use crate::propagation::ground::{ground_impedance, incoherent_rho, spherical_q};
use crate::propagation::rays::RayPair;

/// Inputs to Sub-model 1 (Eq. 123 signature shape — the weather/turbulence
/// params ride inside [`CoherenceInputs`] so Phase 3 attaches without an API
/// break).
#[derive(Debug, Clone, Copy)]
pub struct SubModel1Inputs<'a> {
    /// The direct + ground-reflected ray pair (from [`crate::propagation::rays`]).
    pub rays: &'a RayPair,
    /// Ground flow resistivity σ, kPa·s·m⁻².
    pub sigma_kpa: f64,
    /// Terrain roughness `r`, meters (feeds `Fr`; `0` for Phase 2 targets).
    pub roughness_r: f64,
    /// Coherence weather/turbulence inputs.
    pub coh: &'a CoherenceInputs,
}

/// Sub-model 1 flat-ground effect for one surface type (Eqs. 119–120).
///
/// # Errors
///
/// [`PropagationError::InvalidFlowResistivity`] for σ ≤ 0 / non-finite;
/// [`PropagationError::DegenerateRayGeometry`] if the ray pair carries no
/// reflection (a homogeneous flat path always reflects).
pub fn submodel1(f_hz: f64, inp: &SubModel1Inputs) -> Result<GroundResult, PropagationError> {
    eval(
        f_hz,
        inp.rays,
        inp.sigma_kpa,
        inp.roughness_r,
        inp.coh,
        None,
    )
}

/// Shared evaluator. `force_f = Some(v)` overrides the coherence coefficient
/// (unit-test hook: `Some(1.0)` gives fully-coherent, `p_incoh == 0`); `None`
/// computes `F` from [`coherence_f`]. Also used by Sub-model 2 per surface type.
pub(crate) fn eval(
    f_hz: f64,
    rays: &RayPair,
    sigma_kpa: f64,
    roughness_r: f64,
    coh: &CoherenceInputs,
    force_f: Option<f64>,
) -> Result<GroundResult, PropagationError> {
    let _ = (f_hz, rays, sigma_kpa, roughness_r, coh, force_f);
    let _ = (ground_impedance, incoherent_rho, spherical_q, coherence_f);
    todo!("Eq. 120")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freq::FreqAxis;
    use crate::propagation::coherence::{CoherenceInputs, coherence_ff};
    use crate::propagation::rays::straight_rays;
    use crate::propagation::sound_speed_ms;

    const C0: f64 = 340.348;

    fn zero_turb(d_m: f64) -> CoherenceInputs {
        CoherenceInputs {
            cv2: 0.0,
            ct2: 0.0,
            t_air_c: 15.0,
            c0: C0,
            roughness_r: 0.0,
            f_delta_nu: 1.0,
            d_m,
        }
    }

    fn anchor_rays() -> RayPair {
        straight_rays(97.5, 0.5, 1.5, sound_speed_ms(15.0)).unwrap()
    }

    // Test 1: ΔL anchor table (σ=200, hS=0.5, hR=1.5, d=97.5, F=Ff), ±0.05 dB.
    #[test]
    fn delta_l_reproduces_research_anchor_table() {
        let rays = anchor_rays();
        let coh = zero_turb(97.5);
        let anchors = [
            (100.0, 5.2099),
            (200.0, 1.7311),
            (400.0, -12.6247),
            (800.0, -18.1225),
            (1000.0, -15.4567),
            (2000.0, -6.7805),
            (4000.0, -0.0726),
        ];
        for (f, want) in anchors {
            let inp = SubModel1Inputs {
                rays: &rays,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
                coh: &coh,
            };
            let got = submodel1(f, &inp).unwrap().delta_l_db;
            assert!(
                (got - want).abs() <= 0.05,
                "ΔL_flat({f} Hz): got {got:.4}, want {want:.4} (±0.05)"
            );
        }
    }

    // Test 2: the deepest dip lands on grid point 630.96 Hz or 667.42 Hz (the
    // discrete neighbours of the continuous 646.7 Hz), value ≈ −19.16 dB.
    #[test]
    fn deepest_dip_lands_on_predicted_grid_band() {
        let rays = anchor_rays();
        let coh = zero_turb(97.5);
        let axis = FreqAxis::new();
        let (mut imin, mut vmin) = (0usize, f64::INFINITY);
        for (i, &f) in axis.centres.iter().enumerate() {
            let inp = SubModel1Inputs {
                rays: &rays,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
                coh: &coh,
            };
            let dl = submodel1(f, &inp).unwrap().delta_l_db;
            if dl < vmin {
                vmin = dl;
                imin = i;
            }
        }
        let f_dip = axis.centres[imin];
        assert!(
            (f_dip - 630.96).abs() < 1.0 || (f_dip - 667.42).abs() < 1.0,
            "deepest dip at {f_dip:.2} Hz (idx {imin}), expected 630.96 or 667.42"
        );
        assert!(
            (vmin - (-19.16)).abs() < 0.3,
            "dip depth {vmin:.3} dB, expected ≈ −19.16 (±0.3)"
        );
    }

    // Test 3 (convention pin): the σ=200 dip sits BELOW the hard-ground
    // 1/(2Δτ) = c₀/(2ΔR) frequency — arg Q̂ > 0 pulls dips down. Computed from
    // the RayPair, never hard-coded.
    #[test]
    fn dip_sits_below_hard_ground_prediction() {
        let rays = anchor_rays();
        let coh = zero_turb(97.5);
        let axis = FreqAxis::new();
        let hard_ground_dip = 1.0 / (2.0 * rays.dtau); // c₀/(2ΔR)
        let (mut imin, mut vmin) = (0usize, f64::INFINITY);
        for (i, &f) in axis.centres.iter().enumerate() {
            let inp = SubModel1Inputs {
                rays: &rays,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
                coh: &coh,
            };
            let dl = submodel1(f, &inp).unwrap().delta_l_db;
            if dl < vmin {
                vmin = dl;
                imin = i;
            }
        }
        assert!(
            axis.centres[imin] < hard_ground_dip,
            "soft-ground dip {:.1} Hz must sit below hard-ground {:.1} Hz",
            axis.centres[imin],
            hard_ground_dip
        );
    }

    // Test 4 (two-channel identity + F→1 ⇒ p_incoh == 0).
    #[test]
    fn two_channel_identity_and_f_one_zeroes_incoherent() {
        let rays = anchor_rays();
        let coh = zero_turb(97.5);
        let axis = FreqAxis::new();
        for &f in axis.centres.iter() {
            let inp = SubModel1Inputs {
                rays: &rays,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
                coh: &coh,
            };
            let g = submodel1(f, &inp).unwrap();
            let identity = 10.0 * (g.h_coh_factor.norm_sqr() + g.p_incoh).log10();
            assert!(
                (identity - g.delta_l_db).abs() < 1e-12,
                "two-channel identity at {f} Hz: {identity} vs {}",
                g.delta_l_db
            );
        }
        // Zero-turbulence, small Δτ ⇒ F = Ff ≈ 1 ⇒ p_incoh ≪ |h_coh|².
        let inp = SubModel1Inputs {
            rays: &rays,
            sigma_kpa: 200.0,
            roughness_r: 0.0,
            coh: &coh,
        };
        let g = submodel1(1000.0, &inp).unwrap();
        assert!(
            g.p_incoh < 1e-3 * g.h_coh_factor.norm_sqr(),
            "near-coherent: p_incoh {} not ≪ |h|² {}",
            g.p_incoh,
            g.h_coh_factor.norm_sqr()
        );
        assert!(coherence_ff(1000.0, rays.dtau) > 0.999);
        // Forcing F = 1 exactly ⇒ p_incoh == 0.0 (bit-exact).
        let g1 = eval(1000.0, &rays, 200.0, 0.0, &coh, Some(1.0)).unwrap();
        assert_eq!(g1.p_incoh, 0.0, "F = 1 must zero the incoherent channel");
    }

    // Test 5: soft ground (class A, σ=12.5) attenuates more than hard (class G,
    // σ=20000) in the 200–2000 Hz band.
    #[test]
    fn soft_ground_attenuates_more_than_hard() {
        let rays = anchor_rays();
        let coh = zero_turb(97.5);
        let axis = FreqAxis::new();
        let mean = |sigma: f64| -> f64 {
            let vals: Vec<f64> = axis
                .centres
                .iter()
                .filter(|&&f| (200.0..=2000.0).contains(&f))
                .map(|&f| {
                    let inp = SubModel1Inputs {
                        rays: &rays,
                        sigma_kpa: sigma,
                        roughness_r: 0.0,
                        coh: &coh,
                    };
                    submodel1(f, &inp).unwrap().delta_l_db
                })
                .collect();
            vals.iter().sum::<f64>() / vals.len() as f64
        };
        assert!(
            mean(12.5) < mean(20000.0),
            "soft (A) mean {:.2} must be below hard (G) mean {:.2}",
            mean(12.5),
            mean(20000.0)
        );
    }

    // Test 6: finiteness across σ ∈ {12.5, 200, 20000, 200000} × anchor geometry
    // plus an extreme geometry, at all 105 grid points.
    #[test]
    fn all_outputs_finite_across_the_sweep() {
        let axis = FreqAxis::new();
        let geoms = [
            straight_rays(97.5, 0.5, 1.5, C0).unwrap(),
            straight_rays(1000.0, 0.01, 1.5, C0).unwrap(),
        ];
        for rays in &geoms {
            let coh = zero_turb(1000.0);
            for &sigma in &[12.5, 200.0, 20000.0, 200000.0] {
                for &f in axis.centres.iter() {
                    let inp = SubModel1Inputs {
                        rays,
                        sigma_kpa: sigma,
                        roughness_r: 0.0,
                        coh: &coh,
                    };
                    let g = submodel1(f, &inp).unwrap();
                    assert!(
                        g.delta_l_db.is_finite()
                            && g.h_coh_factor.re.is_finite()
                            && g.h_coh_factor.im.is_finite()
                            && g.p_incoh.is_finite(),
                        "non-finite at σ={sigma} f={f}"
                    );
                }
            }
        }
    }
}
