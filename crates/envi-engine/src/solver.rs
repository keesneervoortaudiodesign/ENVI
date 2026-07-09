//! The tensor-fill solver: the `SolveJob` seam and the chunked `solve()` loop.
//!
//! `solve()` runs the frozen forward chain for each `(sub_source, receiver)`
//! pair and streams the result into a [`TensorSink`] one receiver-axis chunk at
//! a time, so the full tensor need never be resident (OUT-06). It adds NO new
//! physics — it composes the Phase 1–3 chain:
//!
//! ```text
//! 1. PathGeometry::direct(src, rcv) → direct_path(&atmos, axis) → H_ff(f)
//! 2. terrain_effect(profile, src, rcv, coh.c0, coh, axis, weather, isolation)
//!      → { h_coh_factor(f), p_incoh(f) }
//! 3. H_coh[s,r,f]      = H_ff · nord_ratio_to_transfer(h_coh_factor) · 10^{ΔL/20} · 10^{ΔL_s/20}
//! 4. P_incoh_abs[s,r,f] = |H_ff|² · p_incoh · 10^{ΔL/10} · 10^{ΔL_s/10}
//! ```
//!
//! The optional semi-transparent partition `isolation` (`R(f)`,
//! [`IsolationSpectrum`]) is threaded INTO `terrain_effect` (step 2) — it is a
//! `propagation/`-side, pre-conj complex min-phase filter `T(f)` added to the
//! screen branch's coherent factor (D-05), NOT a post-conj magnitude factor like
//! forest/directivity. Opaque is `None` (bit-exact opaque screen, D-10).
//!
//! The optional forest excess attenuation `ΔL_s ≤ 0` (Sub-Model 10,
//! [`crate::forest`]) is a **per-path** property applied on the post-conj side
//! alongside directivity — a real magnitude factor on BOTH channels
//! (`arg(H_coh)` untouched), never a `propagation/` operator.
//!
//! `nord_ratio_to_transfer` (the ONE convention conjugation) lives in
//! `transfer.rs`; the solver runs entirely on the ENVI-convention (post-conj)
//! side. Conditioning (filter `G_s`, delay ramp) is NOT applied here — it is a
//! readout-time multiply (see [`crate::tensor`]). The optional per-band
//! directivity is a **source** property applied on the post-conj side, never in
//! `propagation/`: its magnitude `10^{ΔL/20}` scales `H_coh` (and `10^{ΔL/10}`
//! the incoherent energy), and its optional directional **phase** `e^{+jΔφ}`
//! multiplies `H_coh` only — the two-channel contract holds (phase never enters
//! `P_incoh`). A balloon without phase leaves `arg(H_coh)` untouched, bit-for-bit.

use ndarray::{Array3, s};
use num_complex::Complex;

use crate::forest::{ForestCrossing, forest_delta_l};
use crate::freq::{FreqAxis, N_BANDS};
use crate::geometry::PathGeometry;
use crate::propagation::air_absorption::Atmosphere;
use crate::propagation::coherence::CoherenceInputs;
use crate::propagation::refraction::SoundSpeedProfile;
use crate::propagation::terrain_effect::terrain_effect;
use crate::propagation::transmission::IsolationSpectrum;
use crate::propagation::{PropagationError, direct_path};
use crate::scene::TerrainProfile;
use crate::tensor::TensorSink;
use crate::transfer::nord_ratio_to_transfer;

/// One `(sub_source, receiver)` propagation job — the Phase-9 `PropagationPath`
/// coordination seam.
///
/// The harness builds the geometry (cut-plane profile, positions, atmosphere,
/// coherence, weather, and a pre-evaluated directivity gain); the engine just
/// runs the frozen chain. `terrain_effect` consumes `coh.c0` as its sound speed
/// (consistent with the harness's `build_terrain_inputs`), so no separate
/// `atmos_c0` is carried — the [`Atmosphere`] provides the air-absorption state
/// for `H_ff` and, via `coh.c0`, the terrain sound speed.
#[derive(Debug, Clone, Copy)]
pub struct SolveJob<'a> {
    /// Tensor row: the sub-source index (`0..n_sub`).
    pub sub_source: usize,
    /// Tensor column: the global receiver index.
    pub receiver: usize,
    /// The source→receiver cut-plane terrain profile.
    pub profile: &'a TerrainProfile,
    /// Source position `[x, y, z]`, meters.
    pub src: [f64; 3],
    /// Receiver position `[x, y, z]`, meters.
    pub rcv: [f64; 3],
    /// Atmospheric state for the free-field direct path (air absorption).
    pub atmosphere: &'a Atmosphere,
    /// Coherence inputs (its `c0` is also the terrain sound speed).
    pub coh: &'a CoherenceInputs,
    /// The shared 105-point frequency axis.
    pub axis: &'a FreqAxis,
    /// Optional refraction profile (`None` = homogeneous atmosphere).
    pub weather: Option<&'a SoundSpeedProfile>,
    /// Optional pre-evaluated per-band directivity gain ΔL (dB), applied as a
    /// real magnitude factor `10^{ΔL/20}` to `H_coh` and `10^{ΔL/10}` to
    /// `P_incoh_abs`. The harness evaluates a balloon (`DirectivityBalloon::eval`)
    /// into this slice; the solver only applies the factor.
    pub directivity_gain_db: Option<[f64; N_BANDS]>,
    /// Optional pre-evaluated per-band directional **phase** Δφ (radians),
    /// applied as `e^{+jΔφ}` to `H_coh` ONLY — the coherent channel carries a
    /// directional source's phase (`DirectivityBalloon::eval_phase`), while
    /// `P_incoh_abs` stays magnitude-only (incoherent energy). `None` leaves
    /// `arg(H_coh)` untouched (bit-identical to the magnitude-only path).
    pub directivity_phase_rad: Option<[f64; N_BANDS]>,
    /// Optional forest (scattering-zone) crossing on this path. When `Some`, the
    /// solver evaluates the Sub-Model 10 excess attenuation `ΔL_s ≤ 0` dB
    /// ([`forest_delta_l`]) per band and applies it as a real magnitude factor —
    /// `10^{ΔL_s/20}` on `H_coh` (`arg` untouched) and `10^{ΔL_s/10}` on
    /// `P_incoh_abs` — a **per-path** property applied post-conj, exactly like
    /// `directivity_gain_db` and never inside `propagation/` (D-03/D-04). The
    /// crossing carries a single pre-computed scalar length `d_m` (`R_sc`); the
    /// rubber-band geometry extraction over obstacles (Figure 29) and
    /// multiple/heterogeneous crossings are Phase-9 upstream concerns. `None`
    /// leaves every output bit unchanged (pinned by the `solve_baseline`
    /// regression).
    pub forest: Option<ForestCrossing>,
    /// Optional semi-transparent partition on this path — a per-band sound
    /// reduction index `R(f)` on the 105-point grid, validated at construction
    /// ([`IsolationSpectrum`]). When `Some`, `terrain_effect` threads it through
    /// the screen branch as a complex minimum-phase transmission filter `T(f)`
    /// (native `e^{−jωt}`, pre-conj, D-05) added to the coherent screen factor —
    /// the straight-through leakage that makes a screen/façade semi-transparent.
    /// It joins the **coherent channel only** (never `P_incoh`; Pitfall 6).
    ///
    /// ONE spectrum per job = the **total crossed partition stack**;
    /// multi-partition composition (`T₁·T₂`) and façade→`R(f)` selection are
    /// upstream Phase-7/9 concerns (D-11). Opaque/no-partition is `None` (D-10),
    /// the structural absence that reproduces the standard opaque screen
    /// bit-for-bit — never a large-`R` sentinel. An `isolation` spectrum over flat
    /// terrain is a typed [`PropagationError::IsolationWithoutScreen`].
    pub isolation: Option<&'a IsolationSpectrum>,
}

/// Fill a paired tensor by streaming receiver-axis chunks into `sink`.
///
/// Jobs MUST arrive in **non-decreasing receiver order** (receiver-major); the
/// loop groups consecutive jobs sharing a chunk index
/// (`receiver / chunk_receivers`), fills a fixed `[n_sub, chunk_receivers,
/// N_BANDS]` working buffer, and flushes the used receiver span via
/// [`TensorSink::put_chunk`]. The working set is therefore one chunk, never the
/// full tensor.
///
/// # Errors
///
/// - [`PropagationError::DegenerateChunkSize`] if `chunk_receivers == 0`.
/// - [`PropagationError::SubSourceOutOfRange`] if a job's `sub_source ≥ n_sub`.
/// - [`PropagationError::DegenerateJobGeometry`] on coincident source/receiver.
/// - any [`PropagationError`] from the underlying chain, or a wrapped
///   [`SinkError`](crate::tensor::SinkError) from the sink.
pub fn solve<'a, I>(
    jobs: I,
    n_sub: usize,
    chunk_receivers: usize,
    sink: &mut dyn TensorSink,
) -> Result<(), PropagationError>
where
    I: IntoIterator<Item = SolveJob<'a>>,
{
    if chunk_receivers == 0 {
        return Err(PropagationError::DegenerateChunkSize);
    }

    // One reusable working buffer bounds the resident set to a single chunk
    // (OUT-06) — the full tensor is never allocated by the solver.
    let mut h = Array3::<Complex<f64>>::zeros((n_sub, chunk_receivers, N_BANDS));
    let mut p = Array3::<f64>::zeros((n_sub, chunk_receivers, N_BANDS));

    let mut iter = jobs.into_iter().peekable();
    while let Some(first) = iter.peek() {
        let chunk_index = first.receiver / chunk_receivers;
        let r_offset = chunk_index * chunk_receivers;

        h.fill(Complex::new(0.0, 0.0));
        p.fill(0.0);
        let mut used_len = 0usize;

        // Consume every consecutive job in this chunk (receiver-major order).
        while let Some(job) = iter.peek() {
            if job.receiver / chunk_receivers != chunk_index {
                break;
            }
            let job = iter.next().expect("peeked job exists");
            if job.sub_source >= n_sub {
                return Err(PropagationError::SubSourceOutOfRange {
                    sub_source: job.sub_source,
                    n_sub,
                });
            }
            let r_local = job.receiver - r_offset;
            let (h_pair, p_pair) = solve_pair(&job)?;

            let mut h_dst = h.slice_mut(s![job.sub_source, r_local, ..]);
            for (dst, src) in h_dst.iter_mut().zip(h_pair.iter()) {
                *dst = *src;
            }
            let mut p_dst = p.slice_mut(s![job.sub_source, r_local, ..]);
            for (dst, src) in p_dst.iter_mut().zip(p_pair.iter()) {
                *dst = *src;
            }
            used_len = used_len.max(r_local + 1);
        }

        let h_view = h.slice(s![.., 0..used_len, ..]);
        let p_view = p.slice(s![.., 0..used_len, ..]);
        sink.put_chunk(r_offset, h_view, p_view)?;
    }

    Ok(())
}

/// Run the frozen forward chain for one job, returning the coherent transfer
/// `H_coh(f)` and the absolute incoherent energy `P_incoh_abs(f)` per band.
fn solve_pair(job: &SolveJob<'_>) -> Result<(Vec<Complex<f64>>, Vec<f64>), PropagationError> {
    let path = PathGeometry::direct(job.src, job.rcv).map_err(|e| {
        PropagationError::DegenerateJobGeometry {
            detail: e.to_string(),
        }
    })?;
    let h_ff = direct_path(&path, job.atmosphere, job.axis)?;
    // terrain_effect consumes coh.c0 as its sound speed (see build_terrain_inputs).
    let te = terrain_effect(
        job.profile,
        job.src,
        job.rcv,
        job.coh.c0,
        job.coh,
        job.axis,
        job.weather,
        job.isolation,
    )?;

    let mut h_coh = Vec::with_capacity(N_BANDS);
    let mut p_abs = Vec::with_capacity(N_BANDS);
    for (i, ((&hf, &factor), &pi)) in h_ff
        .iter()
        .zip(&te.h_coh_factor)
        .zip(&te.p_incoh)
        .enumerate()
    {
        // Coherent transfer: H_ff · (Nord2000→ENVI conjugated ground factor).
        // No conditioning here — that is a readout-time multiply.
        let mut hc = hf * nord_ratio_to_transfer(factor);
        // Absolute incoherent energy rides at the free-field magnitude.
        let mut pa = hf.norm_sqr() * pi;
        // Optional directivity magnitude: scales both channels (10^{ΔL/20} on the
        // coherent transfer, 10^{ΔL/10} on the incoherent energy).
        if let Some(dir) = job.directivity_gain_db.as_ref() {
            hc *= 10f64.powf(dir[i] / 20.0);
            pa *= 10f64.powf(dir[i] / 10.0);
        }
        // Optional directional phase: e^{+jΔφ} on the COHERENT channel only
        // (ENVI e^{+jωt}; explicit (cos, sin), never `.conj()`). The incoherent
        // energy channel is phase-agnostic and is left untouched.
        if let Some(phase) = job.directivity_phase_rad.as_ref() {
            let (s, c) = phase[i].sin_cos();
            hc *= Complex::new(c, s);
        }
        // Optional forest excess attenuation (Sub-Model 10, D-03/D-04): a real
        // per-band dB factor on BOTH channels — 10^{ΔL_s/20} on the coherent
        // transfer (arg untouched) and 10^{ΔL_s/10} on the incoherent energy.
        // ΔL_s ≤ 0 (bar a ≤ ~0.01 dB PCHIP interpolation-corner tolerance the f4
        // sweep pins), so this is attenuation-only; a real scale of an exact-zero
        // P_incoh stays exactly zero regardless of that sign (F→1 ⇒ P_incoh→0).
        if let Some(fc) = job.forest.as_ref() {
            let dls = forest_delta_l(job.axis.centres[i], fc, job.coh.c0);
            hc *= 10f64.powf(dls / 20.0);
            pa *= 10f64.powf(dls / 10.0);
        }
        h_coh.push(hc);
        p_abs.push(pa);
    }
    Ok((h_coh, p_abs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::sound_speed_ms;
    use crate::scene::GroundSegment;
    use crate::tensor::InMemorySink;

    const C0: f64 = 340.348;

    fn atmos() -> Atmosphere {
        Atmosphere::new(15.0, 70.0, 101.325).unwrap()
    }

    fn coh() -> CoherenceInputs {
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

    fn flat_profile() -> TerrainProfile {
        TerrainProfile::new(vec![[2.5, 0.0], [100.0, 0.0]], vec![seg(200.0)]).unwrap()
    }

    #[test]
    fn single_pair_matches_hand_assembled_chain_bit_for_bit() {
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        let src = [2.5, 0.0, 0.5];
        let rcv = [100.0, 0.0, 1.5];

        let job = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src,
            rcv,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut sink = InMemorySink::new(1, 1);
        solve([job], 1, 1, &mut sink).unwrap();

        // Hand-assembled reference chain (same order, same c0).
        let path = PathGeometry::direct(src, rcv).unwrap();
        let h_ff = direct_path(&path, &a, &axis).unwrap();
        let te = terrain_effect(&profile, src, rcv, c.c0, &c, &axis, None, None).unwrap();
        let pair = sink.tensor();
        for (f, ((&hf, &factor), &pi)) in h_ff
            .iter()
            .zip(&te.h_coh_factor)
            .zip(&te.p_incoh)
            .enumerate()
        {
            let want_h = hf * nord_ratio_to_transfer(factor);
            let want_p = hf.norm_sqr() * pi;
            assert_eq!(pair.h_coh[[0, 0, f]], want_h, "H_coh mismatch at band {f}");
            assert_eq!(
                pair.p_incoh_abs[[0, 0, f]].to_bits(),
                want_p.to_bits(),
                "P_incoh_abs mismatch at band {f}"
            );
        }
    }

    #[test]
    fn chunked_solve_equals_single_chunk_solve_index_for_index() {
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        let n_sub = 3;
        let n_rcv = 5;

        // Distinct receiver heights so columns differ; receiver-major order.
        let build_jobs = || {
            let mut v = Vec::new();
            for r in 0..n_rcv {
                for s in 0..n_sub {
                    v.push(SolveJob {
                        sub_source: s,
                        receiver: r,
                        profile: &profile,
                        src: [2.5, 0.0, 0.3 + 0.1 * s as f64],
                        rcv: [100.0, 0.0, 1.0 + 0.5 * r as f64],
                        atmosphere: &a,
                        coh: &c,
                        axis: &axis,
                        weather: None,
                        directivity_gain_db: None,
                        directivity_phase_rad: None,
                        forest: None,
                        isolation: None,
                    });
                }
            }
            v
        };

        let mut chunked = InMemorySink::new(n_sub, n_rcv);
        solve(build_jobs(), n_sub, 2, &mut chunked).unwrap();
        let mut single = InMemorySink::new(n_sub, n_rcv);
        solve(build_jobs(), n_sub, n_rcv, &mut single).unwrap();

        assert_eq!(chunked.tensor().h_coh, single.tensor().h_coh);
        assert_eq!(chunked.tensor().p_incoh_abs, single.tensor().p_incoh_abs);
        // chunk_receivers = 2 over 5 receivers ⇒ largest chunk = 2 receivers.
        assert_eq!(
            chunked.high_water_bytes(),
            n_sub * 2 * N_BANDS * crate::tensor::BYTES_PER_CELL_PAIR
        );
    }

    #[test]
    fn directivity_gain_scales_magnitude_only_leaving_phase_unchanged() {
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        let src = [2.5, 0.0, 0.5];
        let rcv = [100.0, 0.0, 1.5];
        let base = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src,
            rcv,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut plain = InMemorySink::new(1, 1);
        solve([base], 1, 1, &mut plain).unwrap();

        let gain_db = -3.0;
        let dir = SolveJob {
            directivity_gain_db: Some([gain_db; N_BANDS]),
            ..base
        };
        let mut with_dir = InMemorySink::new(1, 1);
        solve([dir], 1, 1, &mut with_dir).unwrap();

        let mag = 10f64.powf(gain_db / 20.0);
        let ener = 10f64.powf(gain_db / 10.0);
        for f in 0..N_BANDS {
            let p0 = plain.tensor().h_coh[[0, 0, f]];
            let p1 = with_dir.tensor().h_coh[[0, 0, f]];
            // Phase unchanged (magnitude-only factor).
            assert!(
                (p1.arg() - p0.arg()).abs() < 1e-12,
                "arg changed at band {f}"
            );
            // Magnitude scaled by 10^{ΔL/20}.
            assert!((p1.norm() - p0.norm() * mag).abs() < 1e-12 * (1.0 + p0.norm()));
            // P_incoh_abs scaled by 10^{ΔL/10}.
            let q0 = plain.tensor().p_incoh_abs[[0, 0, f]];
            let q1 = with_dir.tensor().p_incoh_abs[[0, 0, f]];
            assert!((q1 - q0 * ener).abs() < 1e-12 * (1.0 + q0));
        }
    }

    #[test]
    fn directivity_phase_rotates_coherent_channel_only() {
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        let src = [2.5, 0.0, 0.5];
        let rcv = [100.0, 0.0, 1.5];
        let base = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src,
            rcv,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut plain = InMemorySink::new(1, 1);
        solve([base], 1, 1, &mut plain).unwrap();

        // A pure directional phase (no magnitude change).
        let phi = 0.7_f64;
        let phased = SolveJob {
            directivity_phase_rad: Some([phi; N_BANDS]),
            ..base
        };
        let mut with_phase = InMemorySink::new(1, 1);
        solve([phased], 1, 1, &mut with_phase).unwrap();

        let rot = Complex::new(phi.cos(), phi.sin());
        for f in 0..N_BANDS {
            let h0 = plain.tensor().h_coh[[0, 0, f]];
            let h1 = with_phase.tensor().h_coh[[0, 0, f]];
            // H_coh is multiplied by e^{+jφ}: magnitude preserved, arg advanced.
            let want = h0 * rot;
            assert!(
                (h1 - want).norm() <= 1e-12 * (1.0 + h0.norm()),
                "band {f}: H_coh should be e^(+jφ)·H0"
            );
            assert!(
                (h1.norm() - h0.norm()).abs() <= 1e-12 * (1.0 + h0.norm()),
                "band {f}: phase must not change |H_coh|"
            );
            // P_incoh is phase-agnostic — bit-for-bit unchanged.
            assert_eq!(
                with_phase.tensor().p_incoh_abs[[0, 0, f]].to_bits(),
                plain.tensor().p_incoh_abs[[0, 0, f]].to_bits(),
                "band {f}: directional phase must not touch P_incoh"
            );
        }
    }

    /// F6: a forest crossing scales `|H_coh|` by exactly `10^{ΔL_s/20}` (arg
    /// unchanged) and `P_incoh_abs` by `10^{ΔL_s/10}`, per band — a real
    /// two-channel magnitude factor, `ΔL_s` recomputed in-test.
    #[test]
    fn forest_scales_magnitude_two_channels_leaving_phase_unchanged() {
        use crate::forest::{ForestCrossing, forest_delta_l};
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        let src = [2.5, 0.0, 0.5];
        let rcv = [100.0, 0.0, 1.5];
        let base = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src,
            rcv,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut plain = InMemorySink::new(1, 1);
        solve([base], 1, 1, &mut plain).unwrap();

        // A crossing that attenuates across the mid/high bands (ka in range).
        let fc = ForestCrossing::new(80.0, 0.4, 0.12, 0.2, 12.0).unwrap();
        let forested = SolveJob {
            forest: Some(fc),
            ..base
        };
        let mut with_forest = InMemorySink::new(1, 1);
        solve([forested], 1, 1, &mut with_forest).unwrap();

        let mut saw_attenuation = false;
        for f in 0..N_BANDS {
            let dls = forest_delta_l(axis.centres[f], &fc, c.c0);
            let mag = 10f64.powf(dls / 20.0);
            let ener = 10f64.powf(dls / 10.0);
            let h0 = plain.tensor().h_coh[[0, 0, f]];
            let h1 = with_forest.tensor().h_coh[[0, 0, f]];
            // arg unchanged (any finite ΔL_s ≤ 0 ⇒ mag = 10^(ΔL_s/20) > 0, a
            // positive real factor).
            assert!(
                (h1.arg() - h0.arg()).abs() < 1e-12,
                "band {f}: forest must not change arg(H_coh)"
            );
            assert!(
                (h1.norm() - h0.norm() * mag).abs() <= 1e-12 * (1.0 + h0.norm()),
                "band {f}: |H_coh| must scale by 10^(ΔL_s/20)"
            );
            let q0 = plain.tensor().p_incoh_abs[[0, 0, f]];
            let q1 = with_forest.tensor().p_incoh_abs[[0, 0, f]];
            assert!(
                (q1 - q0 * ener).abs() <= 1e-12 * (1.0 + q0),
                "band {f}: P_incoh_abs must scale by 10^(ΔL_s/10)"
            );
            if dls < -0.001 {
                saw_attenuation = true;
            }
        }
        assert!(
            saw_attenuation,
            "the test crossing must attenuate some bands (else it is vacuous)"
        );
    }

    /// F7: `F→1 ⇒ P_incoh→0` stays bit-exact with forest enabled. A source on
    /// the ground gives `Δτ = 0 ⇒ F = 1 ⇒ P_incoh = 0` exactly; a real forest
    /// scale of an exact `0.0` stays exact `0.0` (bit-for-bit).
    #[test]
    fn forest_preserves_exact_zero_incoherent_channel() {
        use crate::forest::ForestCrossing;
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        // Source on the ground (h_s = 0) ⇒ Δτ = 0 ⇒ F = 1 ⇒ p_incoh = 0.
        let src = [2.5, 0.0, 0.0];
        let rcv = [100.0, 0.0, 1.5];
        let base = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src,
            rcv,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut plain = InMemorySink::new(1, 1);
        solve([base], 1, 1, &mut plain).unwrap();
        // Sanity: the zero-turbulence geometry really does give exact-zero P.
        for f in 0..N_BANDS {
            assert_eq!(
                plain.tensor().p_incoh_abs[[0, 0, f]].to_bits(),
                0.0f64.to_bits(),
                "band {f}: baseline geometry must give exact-zero P_incoh"
            );
        }

        let fc = ForestCrossing::new(80.0, 0.4, 0.12, 0.2, 12.0).unwrap();
        let forested = SolveJob {
            forest: Some(fc),
            ..base
        };
        let mut with_forest = InMemorySink::new(1, 1);
        solve([forested], 1, 1, &mut with_forest).unwrap();
        for f in 0..N_BANDS {
            assert_eq!(
                with_forest.tensor().p_incoh_abs[[0, 0, f]].to_bits(),
                0.0f64.to_bits(),
                "band {f}: forest must keep P_incoh exactly zero (F→1 contract)"
            );
        }
    }

    #[test]
    fn degenerate_geometry_is_a_typed_error_not_a_panic() {
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        let p = [2.5, 0.0, 0.5];
        let job = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src: p,
            rcv: p, // coincident ⇒ degenerate path
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut sink = InMemorySink::new(1, 1);
        assert!(matches!(
            solve([job], 1, 1, &mut sink),
            Err(PropagationError::DegenerateJobGeometry { .. })
        ));
    }

    #[test]
    fn zero_chunk_receivers_is_a_typed_error() {
        let profile = flat_profile();
        let axis = FreqAxis::new();
        let a = atmos();
        let c = coh();
        let job = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src: [2.5, 0.0, 0.5],
            rcv: [100.0, 0.0, 1.5],
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut sink = InMemorySink::new(1, 1);
        assert!(matches!(
            solve([job], 1, 0, &mut sink),
            Err(PropagationError::DegenerateChunkSize)
        ));
        // Also assert we never panic on the unused sound-speed helper import.
        let _ = sound_speed_ms(15.0);
    }
}
