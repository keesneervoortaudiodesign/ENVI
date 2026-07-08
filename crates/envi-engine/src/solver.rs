//! The tensor-fill solver: the `SolveJob` seam and the chunked `solve()` loop.
//!
//! `solve()` runs the frozen forward chain for each `(sub_source, receiver)`
//! pair and streams the result into a [`TensorSink`] one receiver-axis chunk at
//! a time, so the full tensor need never be resident (OUT-06). It adds NO new
//! physics — it composes the Phase 1–3 chain:
//!
//! ```text
//! 1. PathGeometry::direct(src, rcv) → direct_path(&atmos, axis) → H_ff(f)
//! 2. terrain_effect(profile, src, rcv, coh.c0, coh, axis, weather)
//!      → { h_coh_factor(f), p_incoh(f) }
//! 3. H_coh[s,r,f]      = H_ff · nord_ratio_to_transfer(h_coh_factor) · 10^{ΔL/20}
//! 4. P_incoh_abs[s,r,f] = |H_ff|² · p_incoh · 10^{ΔL/10}
//! ```
//!
//! `nord_ratio_to_transfer` (the ONE convention conjugation) lives in
//! `transfer.rs`; the solver runs entirely on the ENVI-convention (post-conj)
//! side. Conditioning (filter `G_s`, delay ramp) is NOT applied here — it is a
//! readout-time multiply (see [`crate::tensor`]). The optional per-band
//! directivity gain is a REAL magnitude factor only: `arg(H_coh)` is never
//! touched, and directivity never enters `propagation/`.

use ndarray::{Array3, s};
use num_complex::Complex;

use crate::freq::{FreqAxis, N_BANDS};
use crate::geometry::PathGeometry;
use crate::propagation::air_absorption::Atmosphere;
use crate::propagation::coherence::CoherenceInputs;
use crate::propagation::refraction::SoundSpeedProfile;
use crate::propagation::terrain_effect::terrain_effect;
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
    /// `P_incoh_abs`. The harness / plan 04-02 evaluates a balloon into this
    /// slice; the solver only applies the factor.
    pub directivity_gain_db: Option<[f64; N_BANDS]>,
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
        // Optional directivity: a REAL magnitude factor only (phase untouched).
        if let Some(dir) = job.directivity_gain_db.as_ref() {
            hc *= 10f64.powf(dir[i] / 20.0);
            pa *= 10f64.powf(dir[i] / 10.0);
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
        };
        let mut sink = InMemorySink::new(1, 1);
        solve([job], 1, 1, &mut sink).unwrap();

        // Hand-assembled reference chain (same order, same c0).
        let path = PathGeometry::direct(src, rcv).unwrap();
        let h_ff = direct_path(&path, &a, &axis).unwrap();
        let te = terrain_effect(&profile, src, rcv, c.c0, &c, &axis, None).unwrap();
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
