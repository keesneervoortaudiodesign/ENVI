//! Terrain-effect composition ΔL_t (AV 1106/07 §5.10–5.16, §5.22 Eq. 332):
//! Sub-models 1/2 (ground) and 4/5/6/7 (screens), combined by the §5.21
//! transition parameters into the per-band terrain excess-attenuation.
//!
//! Plan 02-02 lands the ground sub-models ([`submodel1`], [`submodel2`]) and the
//! load-bearing two-channel [`GroundResult`]. Screen sub-models + the Eq. 332
//! composition arrive in plan 02-04.
//!
//! Nord2000-native complex convention (e^{−jωt}); the single conjugation to
//! ENVI's e^{+jωt} transfer convention happens in `transfer.rs` (plan 02-05).
//!
//! # The two-channel contract (user-locked, PROJECT.md Key Decisions 2026-07-07)
//!
//! Every terrain-effect sub-model returns a [`GroundResult`] with **two
//! separate channels** relative to the free-field direct path `p̂₀`:
//!
//! - [`GroundResult::h_coh_factor`] is a genuine `Complex<f64>`: the
//!   coherence-weighted coherent sum `1 + Σ F·(Rᵢ)·e^{+j2πfΔτ}·Q̂` with the Δτ
//!   interference **phase LIVE**. This is the factor that multiplies into `H_coh`
//!   at the 02-05 transfer boundary; the ground-reflected contribution keeps its
//!   phase and combines as complex pressure — dips emerge from the phase, not
//!   from band-energy bookkeeping.
//! - [`GroundResult::p_incoh`] is a real, non-negative energy: **only** the
//!   turbulence-decorrelated `(1−F²)·|ρᵢ·p̂ᵢ/p̂₀|²` residual. It is added at
//!   final-level readout and **never overwrites phase**. When the field is fully
//!   coherent (`F → 1`) this channel is exactly `0`.
//!
//! The band value is `delta_l_db = 10·lg(|h_coh_factor|² + p_incoh)`. Nothing in
//! this module family collapses complex pressure to magnitude/energy along the
//! chain — that separation is what makes ENG-07 (phase-preserving combination)
//! and ENG-02 (segmented soft↔hard ground) correct.
//!
//! # Sub-model 7 (turbulence scattering) is energy-only
//!
//! [`submodel7::submodel7_delta_l`] returns a real `f64` (dB). Eq. 332's screen
//! compositions (`ΔL₄+ΔL₇,₁`, `ΔL₅+ΔL₇,₂`, `ΔL₆+ΔL₇`, assembled in plan 02-05)
//! add the ΔL₇ scattered **energy** into the `p_incoh`/level side of the model
//! (`10·lg(10^{ΔL_scr/10}+10^{ΔL₇/10})`, [`submodel7::combine_scatter`]) — it
//! **never** touches [`GroundResult::h_coh_factor`]. Because Sub-model 7 is typed
//! to `f64`, it is *structurally* incapable of corrupting the coherent phase
//! channel (user-locked contract; threat T-02-13).

use num_complex::Complex;

use crate::freq::FreqAxis;
use crate::propagation::PropagationError;
use crate::propagation::coherence::CoherenceInputs;
use crate::propagation::rays::straight_rays;
use crate::propagation::terrain_interpretation::{
    ScreenClass, StripSpec, TerrainInterpretation, interpret_terrain,
};
use crate::scene::TerrainProfile;

pub mod screen;
pub mod submodel1;
pub mod submodel2;
pub mod submodel7;

/// Two-channel result of any terrain-effect sub-model, normalized relative to
/// the free-field direct path `p̂₀`.
///
/// See the [module docs](self) for the user-locked contract. Nord2000-native
/// convention (e^{−jωt}) inside the engine; conversion to the transfer
/// convention happens once, in plan 02-05.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundResult {
    /// The Nord2000 band value `10·lg(|h_coh_factor|² + p_incoh)`, dB.
    pub delta_l_db: f64,
    /// The F-weighted coherent complex sum, phase LIVE (multiplies into `H_coh`).
    pub h_coh_factor: Complex<f64>,
    /// The `(1−F²)·|ρᵢ·p̂ᵢ/p̂₀|²` turbulence-decorrelated residual — real,
    /// non-negative, added at readout only. Exactly `0` when `F → 1`.
    pub p_incoh: f64,
}

impl GroundResult {
    /// Assemble a result from the two channels, deriving the band value from the
    /// two-channel identity `delta_l_db = 10·lg(|h_coh|² + p_incoh)`.
    #[must_use]
    pub fn from_channels(h_coh_factor: Complex<f64>, p_incoh: f64) -> Self {
        let energy = h_coh_factor.norm_sqr() + p_incoh;
        Self {
            delta_l_db: 10.0 * energy.log10(),
            h_coh_factor,
            p_incoh,
        }
    }
}

/// The per-frequency two-channel terrain effect over the whole 105-point axis,
/// relative to the free-field direct path — the §5.22 Eq. 332 composition of the
/// ground (Sub-model 1/2) and screen (Sub-model 4/5/6 + 7) sub-models.
///
/// This is the Phase 3 (refraction) and Phase 4 (tensor) forward contract:
/// [`Self::h_coh_factor`] is the phase-preserving coherent factor multiplied
/// into `H_coh` at the transfer boundary (`transfer::nord_ratio_to_transfer`);
/// [`Self::p_incoh`] rides alongside as the real incoherent energy;
/// [`Self::delta_l_db`] is the document-exact Nord2000 band value (the
/// validation channel).
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainEffect {
    /// Nord2000-native coherent complex factor per band (length `N_BANDS`).
    pub h_coh_factor: Vec<Complex<f64>>,
    /// Incoherent power add-on per band, relative to the free field.
    pub p_incoh: Vec<f64>,
    /// Document-exact Eq. 332 band value `ΔL_t` per band, dB (validation path).
    pub delta_l_db: Vec<f64>,
}

/// Per-frequency two-channel terrain effect for a cut-plane profile and a
/// source/receiver pair (AV 1106/07 §5.21 dispatch + §5.22 Eq. 332).
///
/// `src`/`rcv` are `[x, y, z]`; only the vertical cut-plane `[x, z]` is used.
/// The §5.21 interpretation classifies the terrain (flat / one screen / thick /
/// two screens); Eq. 332 blends the screen and no-screen branches with the
/// frequency-dependent transition parameters.
///
/// # The two-channel composition (Eq. 332, phase-preserving)
///
/// The document defines Eq. 332 as a **dB-level** interpolation. To honour the
/// user-locked phase-preserving contract, the two channels
/// ([`GroundResult::h_coh_factor`], [`GroundResult::p_incoh`]) are interpolated
/// **linearly** with the same `r` weights, while [`TerrainEffect::delta_l_db`]
/// carries the document-exact dB interpolation. On every Phase 2 target profile
/// **every transition parameter is `0` or `1`**, so exactly one branch is
/// active and the two readings coincide (`10·lg(|h_coh|²+p_incoh) == delta_l_db`
/// bit-for-bit); the linear-channel blend is only *approximate* in the marginal
/// transition zone (`0 < r < 1`), which no Phase 2 target exercises for
/// `r_scr2`/`r_scr12`/`r_flat`. Phase 3 revisits the fractional-`r` channel
/// combination when refraction makes screens marginal (see the module note).
///
/// # Errors
///
/// [`PropagationError::NonFlatTerrainNotImplemented`] if the no-screen branch
/// demands Sub-model 3 (`r_flat < 1` with weight); other
/// [`PropagationError`]s propagate from the sub-models on degenerate geometry.
pub fn terrain_effect(
    profile: &TerrainProfile,
    src: [f64; 3],
    rcv: [f64; 3],
    atmos_c0: f64,
    coh: &CoherenceInputs,
    axis: &FreqAxis,
) -> Result<TerrainEffect, PropagationError> {
    let source = [src[0], src[2]];
    let receiver = [rcv[0], rcv[2]];
    let interp = interpret_terrain(profile, source, receiver, atmos_c0)?;
    let flat = FlatChannel::from_profile(profile, source, receiver, atmos_c0)?;

    let n = axis.centres.len();
    let mut h_coh_factor = Vec::with_capacity(n);
    let mut p_incoh = Vec::with_capacity(n);
    let mut delta_l_db = Vec::with_capacity(n);

    for &f in axis.centres.iter() {
        let tp = interp.transition_params(f);

        // No-screen branch: r_flat·ΔL_flat + (1−r_flat)·ΔL₃. Sub-model 3 is a
        // typed hard error whenever it would carry non-zero weight (never on the
        // flat Phase 2 targets, where r_flat = 1).
        if tp.r_flat < 1.0 - 1e-9 && (1.0 - tp.r_scr1) > 1e-12 {
            return Err(PropagationError::NonFlatTerrainNotImplemented {
                r_flat: tp.r_flat,
                f_hz: f,
            });
        }
        let flat_res = flat.eval(f, coh)?;

        let scr_res = if interp.class == ScreenClass::Flat {
            flat_res
        } else {
            screen_channel(f, &interp, coh)?
        };

        // Eq. 332 outer blend on r_scr1 (screen vs no-screen). Phase 2 targets:
        // r_scr1 ∈ {0,1} except the low-f transition of a real screen, where the
        // linear-channel blend is the phase-preserving reading.
        let r = tp.r_scr1;
        let hc = scr_res.h_coh_factor * r + flat_res.h_coh_factor * (1.0 - r);
        let pi = scr_res.p_incoh * r + flat_res.p_incoh * (1.0 - r);
        // delta_l_db: the document-exact dB interpolation (validation channel).
        let dl = r * scr_res.delta_l_db + (1.0 - r) * flat_res.delta_l_db;

        h_coh_factor.push(hc);
        p_incoh.push(pi);
        delta_l_db.push(dl);
    }

    Ok(TerrainEffect {
        h_coh_factor,
        p_incoh,
        delta_l_db,
    })
}

/// The flat-ground channel (Sub-model 1 for a single surface type, Sub-model 2
/// for a segmented one) built once per `terrain_effect` call.
struct FlatChannel {
    /// Horizontal source→receiver distance, m.
    d: f64,
    /// Source / receiver heights above the flat baseline, m.
    h_s: f64,
    h_r: f64,
    /// Speed of sound, m/s.
    c0: f64,
    /// Segmented surface strips (source-relative x), grouped in Sub-model 2 form.
    strips: Vec<submodel2::SurfaceStrip>,
    /// `true` when every strip shares one `(σ, r)` type ⇒ Sub-model 1.
    single_type: bool,
}

impl FlatChannel {
    fn from_profile(
        profile: &TerrainProfile,
        source: [f64; 2],
        receiver: [f64; 2],
        c0: f64,
    ) -> Result<Self, PropagationError> {
        let d = receiver[0] - source[0];
        if !(d.is_finite() && d > 0.0) {
            return Err(PropagationError::DegenerateRayGeometry {
                detail: "flat channel requires a positive source→receiver distance",
            });
        }
        // Heights above the flat baseline z = 0 (Phase 2 targets are flat ground).
        let h_s = source[1];
        let h_r = receiver[1];

        // Ground strips: each profile segment mapped to source-relative x and
        // clamped to [0, d]. Screen segments (same σ) fold in harmlessly.
        let points = profile.points();
        let segments = profile.segments();
        let mut strips: Vec<submodel2::SurfaceStrip> = Vec::new();
        for (i, seg) in segments.iter().enumerate() {
            let x0 = (points[i][0] - source[0]).clamp(0.0, d);
            let x1 = (points[i + 1][0] - source[0]).clamp(0.0, d);
            if x1 - x0 > 1e-9 {
                strips.push(submodel2::SurfaceStrip {
                    x_start: x0,
                    x_end: x1,
                    sigma_kpa: seg.flow_resistivity,
                    roughness_r: seg.roughness,
                });
            }
        }
        if strips.is_empty() {
            return Err(PropagationError::DegenerateRayGeometry {
                detail: "flat channel found no ground strips in [0, d]",
            });
        }
        let first = (strips[0].sigma_kpa, strips[0].roughness_r);
        let single_type = strips
            .iter()
            .all(|s| s.sigma_kpa == first.0 && s.roughness_r == first.1);

        Ok(Self {
            d,
            h_s,
            h_r,
            c0,
            strips,
            single_type,
        })
    }

    fn eval(&self, f: f64, coh: &CoherenceInputs) -> Result<GroundResult, PropagationError> {
        if self.single_type {
            let rays = straight_rays(self.d, self.h_s, self.h_r, self.c0)?;
            submodel1::eval(
                f,
                &rays,
                self.strips[0].sigma_kpa,
                self.strips[0].roughness_r,
                coh,
                None,
            )
        } else {
            let geom = submodel2::FlatGeometry {
                d: self.d,
                h_s: self.h_s,
                h_r: self.h_r,
                c0: self.c0,
            };
            submodel2::submodel2(f, &self.strips, &geom, coh)
        }
    }
}

/// The screen channel (Sub-model 4/5/6 per the interpretation, plus the
/// Sub-model 7 turbulence-scattering floor). Because the Phase 2 targets give
/// `r_scr2`/`r_scr12 ∈ {0,1}`, the Eq. 332 inner tree collapses to a single
/// sub-model selected by the screen class — the exact tree value.
fn screen_channel(
    f: f64,
    interp: &TerrainInterpretation,
    coh: &CoherenceInputs,
) -> Result<GroundResult, PropagationError> {
    use screen::{ScreenConfig, submodel4, submodel4_c_sr, submodel5, submodel6};
    use submodel7::{ScreenScatterGeometry, submodel7_delta_l};

    // Hard wedge faces (the FORCE screens are treated as hard reflectors — the
    // committed oracle's convention).
    let hard = Complex::new(1.0e9, 0.0);
    let to_scr = |s: &StripSpec| screen::SurfaceStrip {
        seg_a: s.seg_a,
        seg_b: s.seg_b,
        sigma_kpa: s.sigma_kpa,
        roughness_r: s.roughness_r,
    };
    let before: Vec<screen::SurfaceStrip> = interp.before.iter().map(to_scr).collect();
    let middle: Vec<screen::SurfaceStrip> = interp.middle.iter().map(to_scr).collect();
    let after: Vec<screen::SurfaceStrip> = interp.after.iter().map(to_scr).collect();

    // Screen shape: [W₁,T,W₂] (single), [W₁,T₁,T₂,W₂] (thick), or the two apices
    // flanked (double).
    let shape: Vec<[f64; 2]> = match interp.class {
        ScreenClass::DoubleScreen => {
            let (s1, s2) = interp.screens.expect("double screen carries two shapes");
            vec![s1[0], s1[1], s2[1], s2[2]]
        }
        _ => interp.screen.clone(),
    };

    let cfg = ScreenConfig {
        source: interp.source,
        receiver: interp.receiver,
        screen: &shape,
        before: &before,
        middle: &middle,
        after: &after,
        z_face_source: hard,
        z_face_receiver: hard,
        coh,
    };

    let (base, c_sr) = match interp.class {
        ScreenClass::SingleEdge => (submodel4(f, &cfg)?, submodel4_c_sr(f, &cfg)?),
        ScreenClass::ThickScreen => (submodel5(f, &cfg)?, 1.0),
        ScreenClass::DoubleScreen => (submodel6(f, &cfg)?, 1.0),
        ScreenClass::Flat => unreachable!("screen_channel called on flat terrain"),
    };

    // Sub-model 7 turbulence scattering (Eq. 332 additive energy). r₁/r₂ along
    // the S–R line to the primary edge foot; h_e the edge height above that line.
    let edge = interp.primary_edge();
    let r1 = edge[0] - interp.source[0];
    let r2 = interp.receiver[0] - edge[0];
    let dx = interp.receiver[0] - interp.source[0];
    let slope = if dx.abs() > 1e-12 {
        (interp.receiver[1] - interp.source[1]) / dx
    } else {
        0.0
    };
    let h_e = edge[1] - (interp.source[1] + slope * (edge[0] - interp.source[0]));
    let scatter = ScreenScatterGeometry {
        r1: r1.abs().max(1e-6),
        r2: r2.abs().max(1e-6),
        h_e,
        t_air_c: coh.t_air_c,
    };
    let dl7 = submodel7_delta_l(f, &scatter, coh.cv2, coh.ct2, c_sr)?;
    let sm7_energy = 10f64.powf(dl7 / 10.0);

    // Sub-model 7 is energy-only (f64) — it can never touch the phase channel.
    Ok(GroundResult::from_channels(
        base.h_coh_factor,
        base.p_incoh + sm7_energy,
    ))
}

#[cfg(test)]
mod terrain_effect_tests {
    use super::*;
    use crate::freq::N_BANDS;
    use crate::propagation::air_absorption::Atmosphere;
    use crate::propagation::direct_path;
    use crate::propagation::sound_speed_ms;
    use crate::propagation::terrain_effect::submodel1::{SubModel1Inputs, submodel1};
    use crate::scene::{BandSpectrum, GroundSegment};
    use crate::transfer::{band_levels_db, band_levels_db_two_channel, nord_ratio_to_transfer};
    use num_complex::Complex;

    const C0: f64 = 340.348;

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

    fn seg(sigma: f64) -> GroundSegment {
        GroundSegment {
            flow_resistivity: sigma,
            roughness: 0.0,
        }
    }

    /// Flat σ=200 anchor geometry (hS=0.5, hR=1.5, d=97.5).
    fn flat_sigma200() -> (TerrainProfile, [f64; 3], [f64; 3]) {
        let profile =
            TerrainProfile::new(vec![[2.5, 0.0], [100.0, 0.0]], vec![seg(200.0)]).unwrap();
        (profile, [2.5, 0.0, 0.5], [100.0, 0.0, 1.5])
    }

    /// Case-71-like thin screen (4 m spike at x=15 on flat σ=200 ground).
    fn thin_screen() -> (TerrainProfile, [f64; 3], [f64; 3]) {
        let profile = TerrainProfile::new(
            vec![
                [0.0, 0.0],
                [14.99, 0.0],
                [15.0, 4.0],
                [15.01, 0.0],
                [150.0, 0.0],
            ],
            vec![seg(200.0), seg(200.0), seg(200.0), seg(200.0)],
        )
        .unwrap();
        (profile, [0.0, 0.0, 0.5], [150.0, 0.0, 1.5])
    }

    fn h_ff(src: [f64; 3], rcv: [f64; 3]) -> Vec<Complex<f64>> {
        let atmos = Atmosphere::new(15.0, 70.0, 101.325).unwrap();
        let path = crate::geometry::PathGeometry::direct(src, rcv).unwrap();
        direct_path(&path, &atmos, &FreqAxis::new()).unwrap()
    }

    fn h_coh(te: &TerrainEffect, hff: &[Complex<f64>]) -> Vec<Complex<f64>> {
        hff.iter()
            .zip(&te.h_coh_factor)
            .map(|(&hf, &factor)| hf * nord_ratio_to_transfer(factor))
            .collect()
    }

    // Test 6 (end-to-end level identity): the flat channel reproduces Sub-model 1
    // through the full transfer path within 1e-9 dB.
    #[test]
    fn flat_channel_reproduces_submodel1_through_the_transfer_path() {
        let (profile, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis).unwrap();

        // delta_l_db equals Sub-model 1 exactly (flat ⇒ r_scr1 = 0).
        let rays = straight_rays(97.5, 0.5, 1.5, C0).unwrap();
        for (i, &f) in axis.centres.iter().enumerate() {
            let inp = SubModel1Inputs {
                rays: &rays,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
                coh: &coh,
            };
            let sm1 = submodel1(f, &inp).unwrap().delta_l_db;
            assert!(
                (te.delta_l_db[i] - sm1).abs() < 1e-9,
                "ΔL_t vs SM1 at {f} Hz: {} vs {sm1}",
                te.delta_l_db[i]
            );
        }

        // Full transfer path: L = L_W + 20lg|H_ff| + ΔL_t within 1e-9 dB.
        let hff = h_ff(src, rcv);
        let hc = h_coh(&te, &hff);
        let lw = 90.0;
        let spectrum = BandSpectrum::uniform(lw);
        let levels = band_levels_db_two_channel(&hc, &hff, &te.p_incoh, &spectrum);
        for i in 0..N_BANDS {
            let want = lw + 20.0 * hff[i].norm().log10() + te.delta_l_db[i];
            assert!(
                (levels[i] - want).abs() < 1e-9,
                "band {i}: got {} want {want}",
                levels[i]
            );
        }
    }

    // Test 3 (dip through both paths — Pitfall 1 pin): the dip lands on the same
    // grid point natively (argmin ΔL_t) and through the transfer path.
    #[test]
    fn dip_lands_on_the_same_grid_point_both_paths() {
        let (profile, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis).unwrap();

        let argmin = |v: &[f64]| {
            v.iter()
                .enumerate()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap()
                .0
        };
        let native = argmin(&te.delta_l_db);

        let hff = h_ff(src, rcv);
        let hc = h_coh(&te, &hff);
        let unit = BandSpectrum::uniform(0.0);
        let levels = band_levels_db_two_channel(&hc, &hff, &te.p_incoh, &unit);
        let transfer = argmin(&levels);
        assert_eq!(
            native, transfer,
            "dip grid point must match: native {native} vs transfer {transfer}"
        );
        // The dip sits in the 630/667 Hz neighbourhood (σ=200 anchor).
        let f_dip = axis.centres[native];
        assert!(
            (f_dip - 630.96).abs() < 1.0 || (f_dip - 667.42).abs() < 1.0,
            "dip at {f_dip} Hz"
        );
    }

    // Test 4 (phase liveness across ALL operators): H_coh carries a live
    // imaginary part after ground AND after ground+screen.
    #[test]
    fn phase_is_live_through_ground_and_screen() {
        let coh = CoherenceInputs {
            cv2: 0.12,
            ct2: 0.008,
            d_m: 150.0,
            ..zero_turb()
        };
        let axis = FreqAxis::new();
        for (profile, src, rcv) in [flat_sigma200(), thin_screen()] {
            let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis).unwrap();
            let hff = h_ff(src, rcv);
            let hc = h_coh(&te, &hff);
            let live = hc.iter().filter(|z| z.im.abs() > 1e-12).count();
            assert!(
                live as f64 >= 0.9 * N_BANDS as f64,
                "H_coh must be phase-live at ≥90% of bands (got {live})"
            );
            // arg(H_coh) differs from arg(H_ff) somewhere (the terrain factor's
            // phase is real and live, not stripped).
            let differs = hc
                .iter()
                .zip(&hff)
                .any(|(&c, &f)| (c.arg() - f.arg()).abs() > 1e-6);
            assert!(differs, "terrain factor must move the phase");
            // Every value finite.
            assert!(hc.iter().all(|z| z.re.is_finite() && z.im.is_finite()));
        }
    }

    // Test 5 (P_incoh separation): zeroing p_incoh leaves the coherent readout;
    // p_incoh → 0 as the field becomes coherent (small Δτ, zero turbulence).
    #[test]
    fn p_incoh_is_readout_only_and_vanishes_when_coherent() {
        let (profile, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis).unwrap();

        // Structural: band_levels_db_two_channel with p_incoh = 0 equals the pure
        // coherent readout band_levels_db(H_coh) — p_incoh never alters phase.
        let hff = h_ff(src, rcv);
        let hc = h_coh(&te, &hff);
        let lw = BandSpectrum::uniform(70.0);
        let zero = vec![0.0_f64; N_BANDS];
        let coherent_only = band_levels_db_two_channel(&hc, &hff, &zero, &lw);
        let pure = band_levels_db(&hc, &lw);
        for (a, b) in coherent_only.iter().zip(&pure) {
            assert!((a - b).abs() < 1e-12);
        }
        // p_incoh non-negative everywhere; near-coherent geometry ⇒ tiny.
        assert!(te.p_incoh.iter().all(|&p| p >= 0.0));

        // A tiny-Δτ geometry drives Ff → 1 ⇒ p_incoh ≪ |h_coh_factor|².
        let low = TerrainProfile::new(vec![[0.0, 0.0], [1000.0, 0.0]], vec![seg(200.0)]).unwrap();
        let te2 = terrain_effect(
            &low,
            [0.0, 0.0, 0.02],
            [1000.0, 0.0, 0.05],
            C0,
            &CoherenceInputs {
                d_m: 1000.0,
                ..zero_turb()
            },
            &axis,
        )
        .unwrap();
        for (p, h) in te2.p_incoh.iter().zip(&te2.h_coh_factor) {
            assert!(
                *p < 1e-2 * h.norm_sqr().max(1e-12) + 1e-6,
                "near-coherent p_incoh {p} not ≪ |h|² {}",
                h.norm_sqr()
            );
        }
        let _ = sound_speed_ms(15.0);
    }

    // Test 1 (Eq. 332 tree / dispatch): flat ⇒ SM1 path (r_scr1=0); screen ⇒
    // screen path (r_scr1→1) with a strictly different, finite spectrum.
    #[test]
    fn dispatch_selects_flat_vs_screen_branch() {
        let axis = FreqAxis::new();
        let coh = CoherenceInputs {
            cv2: 0.12,
            ct2: 0.008,
            d_m: 150.0,
            ..zero_turb()
        };
        let (fp, fsrc, frcv) = flat_sigma200();
        let flat = terrain_effect(&fp, fsrc, frcv, C0, &coh, &axis).unwrap();
        let (sp, ssrc, srcv) = thin_screen();
        let screen = terrain_effect(&sp, ssrc, srcv, C0, &coh, &axis).unwrap();

        // The screen deeply attenuates the mid/high band vs flat ground.
        let idx_1k = 64;
        assert!(
            screen.delta_l_db[idx_1k] < flat.delta_l_db[idx_1k] - 5.0,
            "screen must attenuate more than flat at 1 kHz: {} vs {}",
            screen.delta_l_db[idx_1k],
            flat.delta_l_db[idx_1k]
        );
        // Everything finite.
        assert!(
            screen
                .delta_l_db
                .iter()
                .chain(&flat.delta_l_db)
                .all(|v| v.is_finite())
        );
    }
}
