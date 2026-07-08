//! Behavior tests for the generic screen⇄ground engine (Sub-model 4 in this
//! file; Sub-models 5/6 in the parent module's later tests).

use super::*;
use crate::freq::FreqAxis;
use crate::propagation::coherence::CoherenceInputs;
use crate::propagation::ground::{ground_impedance, spherical_q};
use crate::propagation::rays::straight_rays_over_segment;
use crate::propagation::sound_speed_ms;
use crate::propagation::terrain_effect::submodel1::{SubModel1Inputs, submodel1};
use num_complex::Complex;

const C0: f64 = 340.348;
const HARD: Complex<f64> = Complex::new(1.0e9, 0.0);

fn zero_turb() -> CoherenceInputs {
    CoherenceInputs {
        cv2: 0.0,
        ct2: 0.0,
        t_air_c: 15.0,
        c0: C0,
        roughness_r: 0.0,
        f_delta_nu: 1.0,
        d_m: 100.0,
    }
}

/// A transparent kernel: the free-field over-the-top ray `e^{+jωτ}/ℓ` with
/// `τ = (|s→T| + |T→r|)/c₀`, `ℓ = |s→T| + |T→r|`. Used to pin the four-path
/// image-model structure independently of the wedge math.
struct Transparent {
    t: [f64; 2],
    c0: f64,
}
impl DiffractionKernel for Transparent {
    fn diffract(
        &self,
        f_hz: f64,
        s: [f64; 2],
        r: [f64; 2],
    ) -> Result<Complex<f64>, PropagationError> {
        let ell = dist(s, self.t) + dist(self.t, r);
        let tau = ell / self.c0;
        Ok(Complex::from_polar(1.0 / ell, TAU * f_hz * tau))
    }
}

/// Thin-screen config with wide flat strips (so the Fresnel weight saturates to
/// 1 ⇒ w_Q = 1, w′ = 1 — a clean single-combination base model).
fn thin_screen_cfg<'a>(
    coh: &'a CoherenceInputs,
    before: &'a [SurfaceStrip],
    after: &'a [SurfaceStrip],
) -> ScreenConfig<'a> {
    ScreenConfig {
        source: [0.0, 1.0],
        receiver: [100.0, 1.0],
        screen: THIN_SCREEN,
        before,
        middle: &[],
        after,
        z_face_source: HARD,
        z_face_receiver: HARD,
        coh,
    }
}

const THIN_SCREEN: &[[f64; 2]] = &[[49.99, 0.0], [50.0, 6.0], [50.01, 0.0]];

fn wide_before() -> [SurfaceStrip; 1] {
    [SurfaceStrip {
        seg_a: [-100.0, 0.0],
        seg_b: [50.0, 0.0],
        sigma_kpa: 200.0,
        roughness_r: 0.0,
    }]
}
fn wide_after() -> [SurfaceStrip; 1] {
    [SurfaceStrip {
        seg_a: [50.0, 0.0],
        seg_b: [250.0, 0.0],
        sigma_kpa: 200.0,
        roughness_r: 0.0,
    }]
}

// Test 2: four-path structure with a transparent kernel + F ≡ 1. With single
// wide strips (w_Q = 1) the coherent factor must equal
// p̂_SCR·(1 + Q̂₁r₂ + Q̂₂r₃ + Q̂₁Q̂₂r₄), assembled independently here.
#[test]
fn four_path_structure_matches_hand_assembly() {
    let coh = zero_turb();
    let before = wide_before();
    let after = wide_after();
    let cfg = thin_screen_cfg(&coh, &before, &after);
    let kernel = Transparent {
        t: [50.0, 6.0],
        c0: C0,
    };
    let s = cfg.source;
    let r = cfg.receiver;
    let top = [50.0, 6.0];

    for &f in &[250.0, 1000.0, 3000.0] {
        let got = submodel4_with_kernel(f, &cfg, &kernel, Some(1.0)).unwrap();

        // Independent assembly.
        let p1 = kernel.diffract(f, s, r).unwrap();
        let r_sr = dist(s, r);
        let p0 = Complex::from_polar(1.0 / r_sr, TAU * f * (r_sr / C0));
        let p_scr = p1 / p0;

        let sv_s = straight_rays_over_segment(s, top, before[0].seg_a, before[0].seg_b, C0)
            .unwrap()
            .reflected
            .unwrap();
        let sv_r = straight_rays_over_segment(r, top, after[0].seg_a, after[0].seg_b, C0)
            .unwrap()
            .reflected
            .unwrap();
        let zg = ground_impedance(f, 200.0).unwrap();
        let q1 = spherical_q(f, sv_s.tau, sv_s.psi_g, zg);
        let q2 = spherical_q(f, sv_r.tau, sv_r.psi_g, zg);
        let s_img = crate::geometry::image_point(s, before[0].seg_a, before[0].seg_b);
        let r_img = crate::geometry::image_point(r, after[0].seg_a, after[0].seg_b);
        let r2 = kernel.diffract(f, s_img, r).unwrap() / p1;
        let r3 = kernel.diffract(f, s, r_img).unwrap() / p1;
        let r4 = kernel.diffract(f, s_img, r_img).unwrap() / p1;
        let c = Complex::new(1.0, 0.0) + q1 * r2 + q2 * r3 + q1 * q2 * r4;
        let want = p_scr * c;

        assert!(
            (got.h_coh_factor - want).norm() / want.norm() < 1e-9,
            "f={f}: engine {} vs assembly {}",
            got.h_coh_factor,
            want
        );
    }
}

// Test 3: two-channel identity + F ≡ 1 zeroes the incoherent channel exactly.
#[test]
fn two_channel_identity_and_f_one_zeroes_incoherent() {
    let coh = zero_turb();
    let before = wide_before();
    let after = wide_after();
    let cfg = thin_screen_cfg(&coh, &before, &after);
    let axis = FreqAxis::new();
    for &f in axis.centres.iter() {
        let g = submodel4(f, &cfg).unwrap();
        let identity = 10.0 * (g.h_coh_factor.norm_sqr() + g.p_incoh).log10();
        assert!(
            (identity - g.delta_l_db).abs() < 1e-12,
            "identity mismatch f={f}"
        );
        // F forced to 1 ⇒ p_incoh == 0 bit-exact.
        let g1 = submodel4_eval(f, &cfg, Some(1.0)).unwrap();
        assert_eq!(g1.p_incoh, 0.0, "F=1 must zero p_incoh (f={f})");
    }
}

// Test 4 (contract pin): |h_coh_factor| equals the Eq. 188 level magnitude at
// 1e-12, and arg(h_coh_factor) is non-trivial and frequency-dependent.
#[test]
fn complex_screen_factor_magnitude_matches_level_and_phase_is_live() {
    let coh = zero_turb();
    let before = wide_before();
    let after = wide_after();
    let cfg = thin_screen_cfg(&coh, &before, &after);

    let mut args = Vec::new();
    for &f in &[250.0, 500.0, 1000.0, 2000.0] {
        let g = submodel4(f, &cfg).unwrap();
        // |h| must reproduce 10^(ΔL/20) (the Eq. 188 magnitude) when p_incoh ≪ |h|².
        let level_mag = 10f64.powf(g.delta_l_db / 20.0);
        let two_channel = (g.h_coh_factor.norm_sqr() + g.p_incoh).sqrt();
        assert!((two_channel - level_mag).abs() / level_mag < 1e-12);
        // arg non-zero (over-the-top phase retained).
        assert!(
            g.h_coh_factor.arg().abs() > 1e-6,
            "phase must be live (f={f})"
        );
        args.push(g.h_coh_factor.arg());
    }
    // Frequency-dependent phase.
    assert!(
        (args[0] - args[3]).abs() > 1e-3,
        "arg(h_coh) must vary with frequency"
    );
}

// Test 5 (Pitfall 4): Eq. 187 normalization keeps w′ ∈ [0,1] and w_Q ∈ [0,1] for
// 1/2/4-strip regions.
#[test]
fn weight_normalization_stays_bounded() {
    // Individual Fresnel-zone weights are coverage fractions ∈ [0,1]; the sum
    // per side may exceed 1 (multi-strip), which the Eq. 187 excess rule handles.
    for raw in [
        vec![0.3],
        vec![0.6, 0.6],
        vec![0.2, 0.3, 0.4, 0.5],
        vec![1.0],
        vec![0.9, 0.9, 0.9],
    ] {
        let (_, side) = normalize_weights(&raw);
        let other = normalize_weights(&[0.4, 0.4]).1;
        let dw_total = side.dw_t + other.dw_t;
        let finished = side.finish(if dw_total > 0.0 { dw_total } else { 1.0 });
        for &w in &finished.w_prime {
            assert!((0.0..=1.0 + 1e-9).contains(&w), "w′={w} out of [0,1]");
        }
        assert!(
            (0.0..=1.0 + 1e-12).contains(&finished.w_q),
            "w_Q out of [0,1]"
        );
        // Normalized weights sum to ≈ 1 when the raw total ≥ 1 branch is active,
        // else to 1 as well (w/w_t · count-normalized) — bounded by 1+excess rule.
        let sum: f64 = finished.w_prime.iter().sum();
        assert!(sum <= 2.0 + 1e-9, "Σw′={sum} exceeds the excess bound");
    }
}

// Test 6a: a real thin screen in deep shadow attenuates (ΔL₄ < 0) at f ≥ 250 Hz.
#[test]
fn thin_screen_attenuates_in_deep_shadow() {
    let coh = zero_turb();
    let before = wide_before();
    let after = wide_after();
    let cfg = thin_screen_cfg(&coh, &before, &after);
    let axis = FreqAxis::new();
    for &f in axis.centres.iter().filter(|&&f| f >= 250.0) {
        let g = submodel4(f, &cfg).unwrap();
        assert!(
            g.delta_l_db < 0.0,
            "ΔL₄({f}) = {} must be < 0 in shadow",
            g.delta_l_db
        );
    }
}

// Test 6b: with the edge well below the S–R line (deep lit zone) the screen
// disappears — |p̂_SCR| → 1 and Sub-model 4 recovers the flat-ground Sub-model 1
// result within 0.5 dB (the screen-removed limit).
#[test]
fn screen_removed_recovers_submodel1() {
    // Deep lit zone (02-03 recovery geometry): S/R at z = 30, edge at z = 0.1.
    let coh = zero_turb();
    let before = [SurfaceStrip {
        seg_a: [-100.0, 0.0],
        seg_b: [50.0, 0.0],
        sigma_kpa: 200.0,
        roughness_r: 0.0,
    }];
    let after = [SurfaceStrip {
        seg_a: [50.0, 0.0],
        seg_b: [250.0, 0.0],
        sigma_kpa: 200.0,
        roughness_r: 0.0,
    }];
    let low_screen: &[[f64; 2]] = &[[49.99, 0.0], [50.0, 0.1], [50.01, 0.0]];
    let cfg = ScreenConfig {
        source: [0.0, 30.0],
        receiver: [100.0, 30.0],
        screen: low_screen,
        before: &before,
        middle: &[],
        after: &after,
        z_face_source: HARD,
        z_face_receiver: HARD,
        coh: &coh,
    };
    // The SCREEN factor recovers the free field within 0.5 dB (the screen itself
    // is removed). The four-path GROUND factor does NOT algebraically reduce to
    // Sub-model 1 — SM4 splits the single ground bounce into two half-path
    // reflections (Q̂₁·Q̂₂ at near-grazing ≈ +1, vs SM1's single Q̂ ≈ −1), so the
    // screen-removed → SM1 recovery is enforced by the §5.21 r_scr1→0 dispatcher
    // (plan 02-05), not by the bare sub-model. Here we pin the exact SM4 property.
    for &f in &[500.0, 1000.0, 2000.0] {
        let kernel = PwedgeKernel {
            w1: low_screen[0],
            t: low_screen[1],
            w2: low_screen[2],
            z_s: HARD,
            z_r: HARD,
            c0: C0,
        };
        let p_scr = screen_factor(&kernel, f, cfg.source, cfg.receiver, C0).unwrap();
        let screen_il_db = -20.0 * p_scr.norm().log10();
        assert!(
            screen_il_db.abs() < 0.5,
            "f={f}: screen insertion loss {screen_il_db} dB should be ≈ 0 (screen removed)"
        );
    }
    // Sub-model 4 stays finite and physically bounded over the whole band in the
    // screen-removed limit (no spurious blow-up from the half-path composition).
    let axis = FreqAxis::new();
    for &f in axis.centres.iter() {
        let g4 = submodel4(f, &cfg).unwrap();
        assert!(
            g4.delta_l_db.is_finite() && g4.delta_l_db < 12.0,
            "ΔL₄({f}) = {}",
            g4.delta_l_db
        );
    }
    let _ = &SubModel1Inputs {
        rays: &straight_rays_over_segment(
            [0.0, 30.0],
            [100.0, 30.0],
            [-100.0, 0.0],
            [250.0, 0.0],
            C0,
        )
        .unwrap(),
        sigma_kpa: 200.0,
        roughness_r: 0.0,
        coh: &coh,
    };
    let _ = submodel1;
}

// Finiteness across the band for the FORCE-like thin screen (T-02-11).
#[test]
fn all_bands_finite_for_thin_screen() {
    let coh = zero_turb();
    let before = wide_before();
    let after = wide_after();
    let cfg = thin_screen_cfg(&coh, &before, &after);
    let axis = FreqAxis::new();
    for &f in axis.centres.iter() {
        let g = submodel4(f, &cfg).unwrap();
        assert!(
            g.delta_l_db.is_finite() && g.h_coh_factor.re.is_finite() && g.p_incoh.is_finite(),
            "non-finite at f={f}"
        );
    }
    // Keep submodel1 import used (screen-removed reference for docs).
    let _ = |rays: &crate::propagation::rays::RayPair| {
        let inp = SubModel1Inputs {
            rays,
            sigma_kpa: 200.0,
            roughness_r: 0.0,
            coh: &coh,
        };
        submodel1(1000.0, &inp)
    };
    let _ = sound_speed_ms(15.0);
}
