//! The owned prepared scene + the natively-testable marshalled range-solve
//! (10-06) — the last Phase-10 seam: turning a `PrepareSolveReq` into REAL
//! Nord2000 range-solve output.
//!
//! # Module I/O
//! - **Input:** a [`PrepareSolveReq`](crate::dto::PrepareSolveReq) built into an
//!   owned [`PreparedScene`] via the engine's **validating** constructors
//!   (`TerrainProfile::try_from`, `Atmosphere::new`, `IsolationSpectrum::try_from`,
//!   `ForestCrossing::new` with the real `forest_path_length_m`,
//!   `DirectivityBalloon::new`/`new_with_phase`, `Rotation3::from_matrix`), plus a
//!   receiver-axis range `(r_offset, len)`.
//! - **Output:** the assembled `[n_sub, len, 105]` paired chunk
//!   `(H_coh, P_incoh_abs)` from [`solve_prepared_range`], bit-equal (f64::to_bits)
//!   to a direct `envi_engine::solver::solve` of `assemble_jobs(0..len, ctx)` —
//!   `lib.rs` writes that one chunk into its OPFS file pair.
//!
//! # Pattern 1 — one solve path, engine byte-identical (D-02)
//! Every cell comes from the UNCHANGED `envi_engine::solver::solve`. The chunk's
//! `len` LOCAL receivers are sharded into pool-sized disjoint sub-ranges run on
//! the `wasm-bindgen-rayon` pool via the proven [`crate::pool::solve_tier`] (its
//! cancel-check + one-`solve()`-per-range + SC3 residency), each sub-range into
//! its own in-memory sink; the sub-range tensors are then assembled in receiver
//! order into ONE sub-source-major `[s][r_local][f]` chunk (one OPFS file per
//! `chunk_index`, so the frozen worker.ts/opfs.ts layout is unchanged). Receivers
//! are independent, so the engine output is bit-identical regardless of shard
//! boundaries; the resident working set stays O(one chunk) (≤ `n_workers`
//! in-memory sub-chunks summing to one chunk, plus the one assembled chunk).
//!
//! # No self-referential struct
//! [`PreparedScene`] owns its engine data; [`PreparedScene::with_local_ctx`] and
//! the shard closures build the borrowing `SubSourcePlacement` slice + a
//! [`SolveCtx`] INSIDE the call, so no `'self` borrow ever leaks out of the
//! struct.

use ndarray::Array3;
use num_complex::Complex;

use envi_engine::directivity::{DirectivityBalloon, Rotation3};
use envi_engine::forest::ForestCrossing;
use envi_engine::freq::{FreqAxis, N_BANDS};
use envi_engine::propagation::air_absorption::Atmosphere;
use envi_engine::propagation::coherence::CoherenceInputs;
use envi_engine::propagation::refraction::SoundSpeedProfile;
use envi_engine::propagation::transmission::IsolationSpectrum;
use envi_engine::scene::TerrainProfile;

use envi_compute::job_assembly::{Directional, ReceiverPlacement, SolveCtx, SubSourcePlacement};
use envi_compute::scene_dto::SoundSpeedProfileDto;

use crate::ComputeError;
use crate::dto::{DirectivityBalloonDto, PrepareSolveReq, SubSourcePlacementDto};

/// One prepared sub-source: its position and optional owned directivity (balloon
/// + source→local-frame rotation). `None` = omnidirectional.
#[derive(Debug, Clone)]
struct PreparedSubSource {
    position: [f64; 3],
    directivity: Option<(DirectivityBalloon, Rotation3)>,
}

/// The OWNED transfer scene the browser solve marshals ONCE per submit
/// (`prepare_solve`), keyed by [`tensor_hash`](Self::tensor_hash). Every
/// subsequent `solve_chunk_range` carrying the same hash lends a [`SolveCtx`] for
/// its receiver range and runs the unchanged engine solve.
#[derive(Debug, Clone)]
pub struct PreparedScene {
    tensor_hash: String,
    profile: TerrainProfile,
    atmosphere: Atmosphere,
    coh: CoherenceInputs,
    axis: FreqAxis,
    weather: Option<SoundSpeedProfile>,
    sub_sources: Vec<PreparedSubSource>,
    receivers: Vec<ReceiverPlacement>,
    forest: Option<ForestCrossing>,
    isolation: Option<IsolationSpectrum>,
    n_sub: usize,
}

/// Reject a non-finite 3-vector before it reaches engine space.
fn check_finite_3(v: &[f64; 3], what: &str) -> Result<(), ComputeError> {
    if v.iter().any(|c| !c.is_finite()) {
        return Err(ComputeError::Prepare(format!("non-finite {what}: {v:?}")));
    }
    Ok(())
}

/// Reject a non-finite scalar before it reaches engine space.
fn check_finite(v: f64, what: &str) -> Result<(), ComputeError> {
    if !v.is_finite() {
        return Err(ComputeError::Prepare(format!("non-finite {what}: {v}")));
    }
    Ok(())
}

/// Build an owned [`SoundSpeedProfile`] from its DTO (finiteness-checked; the
/// engine floors `z0` to `Z0_MIN_M` internally at lookup).
fn build_sound_speed(d: &SoundSpeedProfileDto) -> Result<SoundSpeedProfile, ComputeError> {
    for (v, what) in [
        (d.a, "weather.a"),
        (d.b, "weather.b"),
        (d.c, "weather.c"),
        (d.s_a, "weather.s_a"),
        (d.s_b, "weather.s_b"),
        (d.z0, "weather.z0"),
    ] {
        check_finite(v, what)?;
    }
    Ok(SoundSpeedProfile {
        a: d.a,
        b: d.b,
        c: d.c,
        s_a: d.s_a,
        s_b: d.s_b,
        z0: d.z0,
    })
}

/// Reshape a flattened row-major `(az, pol, band=105)` grid + optional phase grid
/// into an owned [`DirectivityBalloon`] via the engine's validating constructor.
fn build_balloon(d: &DirectivityBalloonDto) -> Result<DirectivityBalloon, ComputeError> {
    let naz = d.azimuths_deg.len();
    let npol = d.polars_deg.len();
    let expected = naz.saturating_mul(npol).saturating_mul(N_BANDS);
    if d.grid_db.len() != expected {
        return Err(ComputeError::Prepare(format!(
            "directivity grid_db length {} != {naz}×{npol}×{N_BANDS}",
            d.grid_db.len()
        )));
    }
    let grid = Array3::from_shape_vec((naz, npol, N_BANDS), d.grid_db.clone())
        .map_err(|e| ComputeError::Prepare(e.to_string()))?;
    match &d.phase_grid_rad {
        Some(ph) => {
            if ph.len() != expected {
                return Err(ComputeError::Prepare(format!(
                    "directivity phase_grid_rad length {} != {naz}×{npol}×{N_BANDS}",
                    ph.len()
                )));
            }
            let phase = Array3::from_shape_vec((naz, npol, N_BANDS), ph.clone())
                .map_err(|e| ComputeError::Prepare(e.to_string()))?;
            DirectivityBalloon::new_with_phase(
                d.azimuths_deg.clone(),
                d.polars_deg.clone(),
                grid,
                phase,
            )
            .map_err(|e| ComputeError::Prepare(e.to_string()))
        }
        None => DirectivityBalloon::new(d.azimuths_deg.clone(), d.polars_deg.clone(), grid)
            .map_err(|e| ComputeError::Prepare(e.to_string())),
    }
}

/// Build one owned prepared sub-source from its DTO.
fn build_sub_source(d: &SubSourcePlacementDto) -> Result<PreparedSubSource, ComputeError> {
    check_finite_3(&d.position, "sub_source.position")?;
    let directivity = match &d.directivity {
        Some(dir) => {
            let balloon = build_balloon(&dir.balloon)?;
            let orientation = Rotation3::from_matrix(dir.orientation.matrix);
            Some((balloon, orientation))
        }
        None => None,
    };
    Ok(PreparedSubSource {
        position: d.position,
        directivity,
    })
}

impl PreparedScene {
    /// Build the owned scene from a marshalled [`PrepareSolveReq`], surfacing any
    /// validating-constructor rejection as [`ComputeError::Prepare`] (never a
    /// panic on data — T-10-06-03).
    ///
    /// # Errors
    /// [`ComputeError::Prepare`] on degenerate geometry, a non-finite value, a
    /// bad directivity grid shape, or an out-of-range isolation/atmosphere value.
    pub fn build(req: &PrepareSolveReq) -> Result<Self, ComputeError> {
        let profile = TerrainProfile::try_from(&req.terrain)
            .map_err(|e| ComputeError::Prepare(e.to_string()))?;
        let atmosphere = Atmosphere::new(
            req.atmosphere.temperature_c,
            req.atmosphere.humidity_pct,
            req.atmosphere.pressure_kpa,
        )
        .map_err(|e| ComputeError::Prepare(e.to_string()))?;

        // Coherence inputs: finiteness-checked before they reach the engine.
        let c = &req.coherence;
        for (v, what) in [
            (c.cv2, "coherence.cv2"),
            (c.ct2, "coherence.ct2"),
            (c.t_air_c, "coherence.t_air_c"),
            (c.c0, "coherence.c0"),
            (c.roughness_r, "coherence.roughness_r"),
            (c.f_delta_nu, "coherence.f_delta_nu"),
            (c.d_m, "coherence.d_m"),
        ] {
            check_finite(v, what)?;
        }
        let coh = CoherenceInputs {
            cv2: c.cv2,
            ct2: c.ct2,
            t_air_c: c.t_air_c,
            c0: c.c0,
            roughness_r: c.roughness_r,
            f_delta_nu: c.f_delta_nu,
            d_m: c.d_m,
        };

        let weather = req.weather.as_ref().map(build_sound_speed).transpose()?;

        let isolation = req
            .isolation
            .as_ref()
            .map(IsolationSpectrum::try_from)
            .transpose()
            .map_err(|e| ComputeError::Prepare(e.to_string()))?;

        // Forest: the authored subset + the REAL through-forest crossing length
        // (Phase-9 geometry) supplied here, not the ForestParamsDto 0.0 placeholder.
        let forest = match &req.forest {
            Some(f) => {
                let d_m = req.forest_path_length_m.unwrap_or(0.0);
                check_finite(d_m, "forest_path_length_m")?;
                Some(
                    ForestCrossing::new(
                        d_m,
                        f.density_per_m2,
                        f.stem_radius_m,
                        f.absorption.unwrap_or(0.0),
                        f.height_m,
                    )
                    .map_err(|e| ComputeError::Prepare(e.to_string()))?,
                )
            }
            None => None,
        };

        let sub_sources = req
            .sub_sources
            .iter()
            .map(build_sub_source)
            .collect::<Result<Vec<_>, _>>()?;

        let receivers = req
            .receivers
            .iter()
            .map(|r| {
                check_finite_3(&r.position, "receiver.position")?;
                Ok(ReceiverPlacement {
                    global_index: r.global_index as usize,
                    position: r.position,
                })
            })
            .collect::<Result<Vec<_>, ComputeError>>()?;

        Ok(Self {
            tensor_hash: req.tensor_hash.clone(),
            profile,
            atmosphere,
            coh,
            axis: FreqAxis::new(),
            weather,
            sub_sources,
            receivers,
            forest,
            isolation,
            n_sub: req.n_sub as usize,
        })
    }

    /// The frozen tensor-identity hash this scene is keyed under.
    #[must_use]
    pub fn tensor_hash(&self) -> &str {
        &self.tensor_hash
    }

    /// The sub-source count (tensor rows).
    #[must_use]
    pub fn n_sub(&self) -> usize {
        self.n_sub
    }

    /// The borrowing [`SubSourcePlacement`] slice for this scene (built inside a
    /// call so its balloon references never leak past the caller).
    fn subs(&self) -> Vec<SubSourcePlacement<'_>> {
        self.sub_sources
            .iter()
            .map(|s| SubSourcePlacement {
                position: s.position,
                directivity: s
                    .directivity
                    .as_ref()
                    .map(|(balloon, orientation)| Directional {
                        balloon,
                        orientation: *orientation,
                    }),
            })
            .collect()
    }

    /// How many prepared receivers have a GLOBAL index in `[global_lo, global_lo +
    /// len)`. Used to validate a `solve_chunk_range` request BEFORE any slicing
    /// (WR-01) and to report the actually-solved count (WR-06). Cheap — a single
    /// filter pass, no allocation.
    pub(crate) fn local_receiver_count(&self, global_lo: usize, len: usize) -> usize {
        let hi = global_lo.saturating_add(len);
        self.receivers
            .iter()
            .filter(|r| (global_lo..hi).contains(&r.global_index))
            .count()
    }

    /// The receivers whose GLOBAL index lies in `[global_lo, global_lo + len)`,
    /// REINDEXED to local `0..k` (ascending by global index) — mirroring the pool
    /// `local_receivers` pattern so each range writes a self-contained chunk at
    /// local offset 0.
    fn local_receivers(&self, global_lo: usize, len: usize) -> Vec<ReceiverPlacement> {
        let hi = global_lo + len;
        let mut sel: Vec<&ReceiverPlacement> = self
            .receivers
            .iter()
            .filter(|r| (global_lo..hi).contains(&r.global_index))
            .collect();
        sel.sort_by_key(|r| r.global_index);
        sel.iter()
            .enumerate()
            .map(|(i, r)| ReceiverPlacement {
                global_index: i,
                position: r.position,
            })
            .collect()
    }

    /// Assemble a [`SolveCtx`] borrowing this scene's owned data plus the given
    /// `subs` + `receivers` slices (both built by the caller so no borrow leaks).
    fn ctx_with<'a>(
        &'a self,
        subs: &'a [SubSourcePlacement<'a>],
        receivers: &'a [ReceiverPlacement],
    ) -> SolveCtx<'a> {
        SolveCtx {
            profile: &self.profile,
            atmosphere: &self.atmosphere,
            coh: &self.coh,
            axis: &self.axis,
            weather: self.weather.as_ref(),
            sub_sources: subs,
            receivers,
            forest: self.forest,
            isolation: self.isolation.as_ref(),
        }
    }

    /// Build the local `SolveCtx` for `[global_lo, global_lo + len)` and run `f`
    /// against it — everything the ctx borrows is built inside this call, so no
    /// `'self` borrow escapes (avoids a self-referential struct). Used by the
    /// sequential stable-wasm32 solve path and the native equivalence tests.
    #[cfg(any(all(target_arch = "wasm32", not(feature = "threads")), test))]
    pub(crate) fn with_local_ctx<R>(
        &self,
        global_lo: usize,
        len: usize,
        f: impl FnOnce(&SolveCtx<'_>) -> R,
    ) -> R {
        let subs = self.subs();
        let recv = self.local_receivers(global_lo, len);
        let ctx = self.ctx_with(&subs, &recv);
        f(&ctx)
    }
}

/// Solve one receiver-chunk range against a prepared scene, returning the
/// assembled `[n_sub, len, 105]` paired chunk `(H_coh, P_incoh_abs)`.
///
/// `r_offset` is the GLOBAL index of the chunk's first receiver; `len` its
/// receiver count. The output is bit-equal (f64::to_bits) to a single direct
/// `envi_engine::solver::solve` of `assemble_jobs(0..len, ctx)` — sharding across
/// the rayon pool never changes a bit (receivers are independent).
///
/// # Errors
/// [`ComputeError::Cancelled`] if the cooperative flag is set mid-tier;
/// [`ComputeError::Solve`] wrapping an engine `PropagationError`.
pub fn solve_prepared_range(
    scene: &PreparedScene,
    r_offset: usize,
    len: usize,
) -> Result<(Array3<Complex<f64>>, Array3<f64>), ComputeError> {
    if len == 0 {
        return Ok((
            Array3::zeros((scene.n_sub, 0, N_BANDS)),
            Array3::zeros((scene.n_sub, 0, N_BANDS)),
        ));
    }
    // WR-01: validate the range is densely covered BEFORE any sharding/slicing. If
    // the prepared scene's receivers do not fully cover `[r_offset, r_offset+len)`,
    // the shard slice `chunk_receivers[..]` would index out of bounds and, under
    // `panic = abort` (wasm), trap the whole module. Return a typed `Range` error
    // instead so a malformed request is a `JsValue` error, never a wasm trap.
    let covered = scene.local_receiver_count(r_offset, len);
    if covered != len {
        return Err(ComputeError::Range {
            r_offset,
            len,
            covered,
        });
    }
    solve_prepared_shards(scene, r_offset, len)
}

// --- Pool-sharded assembly (native `cargo test` + threaded wasm) ------------

/// The pool-parallel range solve: shard the chunk's LOCAL receivers into
/// pool-sized disjoint sub-ranges, run each on the rayon pool via the proven
/// [`crate::pool::solve_tier`] into its own in-memory sink, then assemble the
/// sub-range tensors in receiver order into one sub-source-major chunk.
#[cfg(any(not(target_arch = "wasm32"), feature = "threads"))]
fn solve_prepared_shards(
    scene: &PreparedScene,
    r_offset: usize,
    len: usize,
) -> Result<(Array3<Complex<f64>>, Array3<f64>), ComputeError> {
    use std::sync::{Arc, Mutex};

    use envi_engine::tensor::{InMemorySink, SinkError, TensorPair, TensorSink};
    use ndarray::{ArrayView3, s as sl};

    use envi_compute::job_assembly::assemble_jobs;

    use crate::CANCEL;
    use crate::pool::{ChunkRange, RangeSink, solve_tier};

    /// An in-memory [`RangeSink`] that captures one shard's `[n_sub, len, 105]`
    /// tensor and, on `finish`, reports `(parent_local_offset, len, TensorPair)`
    /// into the shared collector for the caller to assemble.
    struct ShardSink {
        parent_offset: usize,
        len: usize,
        inner: InMemorySink,
        collector: Arc<Mutex<Vec<(usize, usize, TensorPair)>>>,
    }

    impl TensorSink for ShardSink {
        fn put_chunk(
            &mut self,
            r_offset: usize,
            h_coh: ArrayView3<'_, Complex<f64>>,
            p_incoh_abs: ArrayView3<'_, f64>,
        ) -> Result<(), SinkError> {
            self.inner.put_chunk(r_offset, h_coh, p_incoh_abs)
        }
    }

    impl RangeSink for ShardSink {
        fn finish(&mut self) -> Result<(), ComputeError> {
            self.collector.lock().expect("shard collector lock").push((
                self.parent_offset,
                self.len,
                self.inner.tensor().clone(),
            ));
            Ok(())
        }
    }

    let n_sub = scene.n_sub;
    // The chunk's LOCAL receivers (positions), local index 0..len.
    let chunk_receivers = scene.local_receivers(r_offset, len);

    // Shard the LOCAL receiver axis into pool-sized disjoint sub-ranges.
    let n_threads = rayon::current_num_threads().max(1);
    let shard_len = len.div_ceil(n_threads).max(1);
    let mut shards: Vec<ChunkRange> = Vec::new();
    let mut lo = 0usize;
    let mut idx = 0usize;
    while lo < len {
        let l = (len - lo).min(shard_len);
        shards.push(ChunkRange {
            chunk_index: idx,
            r_offset: lo,
            len: l,
        });
        lo += l;
        idx += 1;
    }

    // Pre-build the borrowing sub-source slice (lives for the whole solve_tier
    // call) + each shard's LOCAL-indexed receivers (0..shard.len).
    let subs = scene.subs();
    let shard_receivers: Vec<Vec<ReceiverPlacement>> = shards
        .iter()
        .map(|s| {
            chunk_receivers[s.r_offset..s.r_offset + s.len]
                .iter()
                .enumerate()
                .map(|(i, r)| ReceiverPlacement {
                    global_index: i,
                    position: r.position,
                })
                .collect()
        })
        .collect();

    let collector: Arc<Mutex<Vec<(usize, usize, TensorPair)>>> = Arc::new(Mutex::new(Vec::new()));

    solve_tier(
        &shards,
        n_sub,
        &CANCEL,
        |shard| {
            let recv = &shard_receivers[shard.chunk_index];
            let ctx = scene.ctx_with(&subs, recv);
            assemble_jobs(0..shard.len, &ctx)
        },
        |shard| {
            Ok(ShardSink {
                parent_offset: shard.r_offset,
                len: shard.len,
                inner: InMemorySink::new(n_sub, shard.len),
                collector: collector.clone(),
            })
        },
    )
    .map(drop)?;

    // Assemble the disjoint shard tensors into ONE sub-source-major chunk.
    let mut h = Array3::<Complex<f64>>::zeros((n_sub, len, N_BANDS));
    let mut p = Array3::<f64>::zeros((n_sub, len, N_BANDS));
    for (off, l, pair) in collector.lock().expect("shard collector lock").drain(..) {
        h.slice_mut(sl![.., off..off + l, ..]).assign(&pair.h_coh);
        p.slice_mut(sl![.., off..off + l, ..])
            .assign(&pair.p_incoh_abs);
    }
    Ok((h, p))
}

// --- Sequential fallback (stable single-threaded wasm32, no rayon) ----------

/// The single-threaded range solve for the stable `wasm32-unknown-unknown` smoke
/// build (no `threads` feature ⇒ no rayon/pool): one direct engine `solve()` over
/// the whole chunk. Bit-identical to the sharded path (receivers are independent).
#[cfg(all(target_arch = "wasm32", not(feature = "threads")))]
fn solve_prepared_shards(
    scene: &PreparedScene,
    r_offset: usize,
    len: usize,
) -> Result<(Array3<Complex<f64>>, Array3<f64>), ComputeError> {
    use envi_engine::solver::solve;
    use envi_engine::tensor::InMemorySink;

    use envi_compute::job_assembly::assemble_jobs;

    let n_sub = scene.n_sub;
    let mut sink = InMemorySink::new(n_sub, len);
    scene.with_local_ctx(r_offset, len, |ctx| {
        let jobs = assemble_jobs(0..len, ctx);
        solve(jobs, n_sub, len.max(1), &mut sink).map_err(|e| ComputeError::Solve(e.to_string()))
    })?;
    let pair = sink.tensor();
    Ok((pair.h_coh.clone(), pair.p_incoh_abs.clone()))
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use envi_engine::solver::solve;
    use envi_engine::tensor::InMemorySink;
    use std::f64::consts::PI;

    use envi_compute::job_assembly::assemble_jobs;

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

    fn flat_profile() -> TerrainProfile {
        use envi_engine::scene::GroundSegment;
        TerrainProfile::new(
            vec![[2.5, 0.0], [400.0, 0.0]],
            vec![GroundSegment {
                flow_resistivity: 200.0,
                roughness: 0.0,
            }],
        )
        .unwrap()
    }

    fn az_pol_to_unit(az_deg: f64, pol_deg: f64) -> (f64, f64, f64) {
        let a = az_deg.to_radians();
        let p = pol_deg.to_radians();
        (p.sin() * a.cos(), p.sin() * a.sin(), p.cos())
    }

    fn magnitude_balloon() -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler(10.0, 10.0, |az, pol, band| {
            let (x, _y, _z) = az_pol_to_unit(az, pol);
            (2.0 + 0.01 * band as f64) * x
        })
        .unwrap()
    }

    fn phased_balloon() -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler_with_phase(
            10.0,
            10.0,
            |az, pol, band| {
                let (x, _y, _z) = az_pol_to_unit(az, pol);
                (2.0 + 0.01 * band as f64) * x
            },
            |az, _pol, _band| 0.6 * az.to_radians().sin(),
        )
        .unwrap()
    }

    /// N receivers spread down-range, global indices 0..N (contiguous).
    fn receivers(n: usize) -> Vec<ReceiverPlacement> {
        (0..n)
            .map(|i| ReceiverPlacement {
                global_index: i,
                position: [100.0 + i as f64, 0.0, 1.5],
            })
            .collect()
    }

    /// A scene: co-located directional + omni sub-source (n_sub = 2), with the
    /// given forest / isolation / balloon-with-phase options.
    fn scene(
        n_rcv: usize,
        forest: Option<ForestCrossing>,
        isolation: Option<IsolationSpectrum>,
        phased: bool,
    ) -> PreparedScene {
        let balloon = if phased {
            phased_balloon()
        } else {
            magnitude_balloon()
        };
        PreparedScene {
            tensor_hash: "deadbeef".to_string(),
            profile: flat_profile(),
            atmosphere: atmos(),
            coh: coh(),
            axis: FreqAxis::new(),
            weather: None,
            sub_sources: vec![
                PreparedSubSource {
                    position: [2.5, 0.0, 0.5],
                    directivity: Some((balloon, Rotation3::identity())),
                },
                PreparedSubSource {
                    position: [2.5, 0.0, 0.8],
                    directivity: None,
                },
            ],
            receivers: receivers(n_rcv),
            forest,
            isolation,
            n_sub: 2,
        }
    }

    /// A direct `envi_engine::solver::solve` of `assemble_jobs(0..len, ctx)` over
    /// the full receiver set — the equivalence reference.
    fn direct_solve(scene: &PreparedScene, len: usize) -> (Array3<Complex<f64>>, Array3<f64>) {
        scene.with_local_ctx(0, len, |ctx| {
            let jobs = assemble_jobs(0..len, ctx);
            let mut sink = InMemorySink::new(scene.n_sub, len);
            solve(jobs, scene.n_sub, len, &mut sink).unwrap();
            let pair = sink.tensor();
            (pair.h_coh.clone(), pair.p_incoh_abs.clone())
        })
    }

    fn assert_bits_equal(a: &Array3<Complex<f64>>, b: &Array3<Complex<f64>>, what: &str) {
        assert_eq!(a.dim(), b.dim(), "{what}: dims differ");
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.re.to_bits(), y.re.to_bits(), "{what}: re bits differ");
            assert_eq!(x.im.to_bits(), y.im.to_bits(), "{what}: im bits differ");
        }
    }

    fn assert_p_bits_equal(a: &Array3<f64>, b: &Array3<f64>, what: &str) {
        assert_eq!(a.dim(), b.dim(), "{what}: dims differ");
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.to_bits(), y.to_bits(), "{what}: P_incoh bits differ");
        }
    }

    #[test]
    fn marshalled_range_solve_is_bit_equal_to_a_direct_engine_solve() {
        let sc = scene(5, None, None, false);
        let (h, p) = solve_prepared_range(&sc, 0, 5).unwrap();
        let (wh, wp) = direct_solve(&sc, 5);
        assert_bits_equal(&h, &wh, "marshalled vs direct H_coh");
        assert_p_bits_equal(&p, &wp, "marshalled vs direct P_incoh");
    }

    /// A thin-screen cut-plane profile (peak at x=15, z=4) so the diffraction /
    /// screen branch is selected — the branch the isolation spectrum enters (ENG-10).
    fn screen_profile() -> TerrainProfile {
        use envi_engine::scene::GroundSegment;
        let seg = || GroundSegment {
            flow_resistivity: 200.0,
            roughness: 0.0,
        };
        TerrainProfile::new(
            vec![
                [0.0, 0.0],
                [14.99, 0.0],
                [15.0, 4.0],
                [15.01, 0.0],
                [150.0, 0.0],
            ],
            vec![seg(), seg(), seg(), seg()],
        )
        .unwrap()
    }

    /// A single-omni-sub-source scene over the thin-screen profile, with an
    /// optional isolation spectrum on the screen path.
    fn screen_scene(isolation: Option<IsolationSpectrum>) -> PreparedScene {
        PreparedScene {
            tensor_hash: "screen".to_string(),
            profile: screen_profile(),
            atmosphere: atmos(),
            coh: coh(),
            axis: FreqAxis::new(),
            weather: None,
            sub_sources: vec![PreparedSubSource {
                position: [2.5, 0.0, 0.5],
                directivity: None,
            }],
            receivers: vec![ReceiverPlacement {
                global_index: 0,
                position: [100.0, 0.0, 1.5],
            }],
            forest: None,
            isolation,
            n_sub: 1,
        }
    }

    fn any_band_differs(a: &Array3<Complex<f64>>, b: &Array3<Complex<f64>>) -> bool {
        a.iter().zip(b.iter()).any(|(x, y)| (x - y).norm() > 1e-30)
    }

    #[test]
    fn forest_crossing_effect_is_observable_not_dropped() {
        // ENG-09: a drawn forest crossing must change H_coh (never silently inert).
        let forest = ForestCrossing::new(80.0, 0.4, 0.12, 0.2, 12.0).unwrap();
        let bare = scene(2, None, None, false);
        let with_forest = scene(2, Some(forest), None, false);

        let (h_bare, _) = solve_prepared_range(&bare, 0, 2).unwrap();
        let (h_forest, _) = solve_prepared_range(&with_forest, 0, 2).unwrap();
        assert!(
            any_band_differs(&h_bare, &h_forest),
            "a forest crossing must change H_coh (ENG-09)"
        );
    }

    #[test]
    fn isolation_spectrum_effect_is_observable_not_dropped() {
        // ENG-10: an isolation spectrum on a SCREEN path must change H_coh (the
        // straight-through leakage that makes a screen/façade semi-transparent).
        let iso = IsolationSpectrum::new([12.0; N_BANDS]).unwrap();
        let bare_screen = screen_scene(None);
        let with_iso = screen_scene(Some(iso));

        let (h_bare, _) = solve_prepared_range(&bare_screen, 0, 1).unwrap();
        let (h_iso, _) = solve_prepared_range(&with_iso, 0, 1).unwrap();
        assert!(
            any_band_differs(&h_bare, &h_iso),
            "an isolation spectrum must change H_coh on a screen path (ENG-10)"
        );
    }

    #[test]
    fn directional_phase_is_visible_and_phase_free_is_bit_identical() {
        // SC4: a phase-carrying balloon rotated on one co-located sub-source changes
        // the coherent-sum argument; a phase-free baseline leaves arg(H_coh) identical.
        let base = scene(1, None, None, true); // both subs identity-oriented
        // Rotate sub-source 0's phased balloon 90° about Z.
        let mut rotated = base.clone();
        if let Some((_, orientation)) = rotated.sub_sources[0].directivity.as_mut() {
            *orientation = Rotation3::about_z(PI / 2.0);
        }

        let (h_base, _) = solve_prepared_range(&base, 0, 1).unwrap();
        let (h_rot, _) = solve_prepared_range(&rotated, 0, 1).unwrap();

        // Coherent sum over sub-sources per band; rotating the phased balloon must
        // move the argument on some band.
        let mut saw_arg_change = false;
        for f in 0..N_BANDS {
            let sb: Complex<f64> = (0..2).map(|s| h_base[[s, 0, f]]).sum();
            let sr: Complex<f64> = (0..2).map(|s| h_rot[[s, 0, f]]).sum();
            if sb.norm() > 1e-30 && sr.norm() > 1e-30 && (sr.arg() - sb.arg()).abs() > 1e-9 {
                saw_arg_change = true;
            }
        }
        assert!(
            saw_arg_change,
            "rotating a phase-carrying balloon must change the coherent-sum argument (SC4)"
        );

        // Phase-free baseline: arg(H_coh) bit-identical to the magnitude-only path.
        let mag = scene(1, None, None, false);
        let (h_mag, _) = solve_prepared_range(&mag, 0, 1).unwrap();
        let (wh, _) = direct_solve(&mag, 1);
        for f in 0..N_BANDS {
            for s in 0..2 {
                assert_eq!(
                    h_mag[[s, 0, f]].arg().to_bits(),
                    wh[[s, 0, f]].arg().to_bits(),
                    "phase-free arg(H_coh) must be bit-identical to the direct solve"
                );
            }
        }
    }

    #[test]
    fn two_shard_assembly_equals_a_single_range_solve_bit_for_bit() {
        // Force ≥ 2 shards by running under a 2-thread pool with 4 receivers, and
        // assert the assembled chunk is bit-equal to a single direct solve.
        let sc = scene(4, None, None, false);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        let (h, p) = pool.install(|| solve_prepared_range(&sc, 0, 4)).unwrap();
        let (wh, wp) = direct_solve(&sc, 4);
        assert_bits_equal(&h, &wh, "two-shard vs single-range H_coh");
        assert_p_bits_equal(&p, &wp, "two-shard vs single-range P_incoh");
    }

    #[test]
    fn build_from_dto_marshals_a_solvable_omni_scene() {
        // The DTO marshalling path (PreparedScene::build) yields a scene whose
        // range-solve matches a direct solve.
        use crate::dto::{
            AtmosphereDto, CoherenceInputsDto, PrepareSolveReq, ReceiverPlacementDto,
            SubSourcePlacementDto,
        };
        use envi_compute::scene_dto::{GroundSegmentDto, TerrainProfileDto};

        let req = PrepareSolveReq {
            tensor_hash: "abc123".to_string(),
            n_sub: 1,
            terrain: TerrainProfileDto {
                points: vec![[2.5, 0.0], [400.0, 0.0]],
                segments: vec![GroundSegmentDto {
                    flow_resistivity: 200.0,
                    roughness: 0.0,
                }],
            },
            atmosphere: AtmosphereDto {
                temperature_c: 15.0,
                humidity_pct: 70.0,
                pressure_kpa: 101.325,
            },
            coherence: CoherenceInputsDto {
                cv2: 0.0,
                ct2: 0.0,
                t_air_c: 15.0,
                c0: C0,
                roughness_r: 0.0,
                f_delta_nu: 1.0,
                d_m: 97.5,
            },
            weather: None,
            sub_sources: vec![SubSourcePlacementDto {
                position: [2.5, 0.0, 0.5],
                directivity: None,
            }],
            receivers: vec![
                ReceiverPlacementDto {
                    global_index: 0,
                    position: [100.0, 0.0, 1.5],
                },
                ReceiverPlacementDto {
                    global_index: 1,
                    position: [101.0, 0.0, 1.5],
                },
            ],
            forest: None,
            forest_path_length_m: None,
            isolation: None,
        };
        let sc = PreparedScene::build(&req).expect("valid scene builds");
        assert_eq!(sc.tensor_hash(), "abc123");
        let (h, p) = solve_prepared_range(&sc, 0, 2).unwrap();
        let (wh, wp) = direct_solve(&sc, 2);
        assert_bits_equal(&h, &wh, "dto-built H_coh");
        assert_p_bits_equal(&p, &wp, "dto-built P_incoh");
    }

    #[test]
    fn range_exceeding_receiver_coverage_is_a_typed_error_not_a_panic() {
        // WR-01: a `len` beyond the prepared scene's receiver coverage must return a
        // typed `Range` error, never slice out of bounds / trap the wasm module.
        let sc = scene(2, None, None, false); // receivers global 0,1 (coverage = 2)
        let err = solve_prepared_range(&sc, 0, 5).unwrap_err();
        assert!(
            matches!(
                err,
                ComputeError::Range {
                    r_offset: 0,
                    len: 5,
                    covered: 2
                }
            ),
            "an over-long range must be a typed Range error, got {err:?}"
        );
        // A sparse range (offset past the receivers) is likewise a typed error.
        let err2 = solve_prepared_range(&sc, 10, 2).unwrap_err();
        assert!(matches!(
            err2,
            ComputeError::Range {
                r_offset: 10,
                len: 2,
                covered: 0
            }
        ));
    }

    #[test]
    fn build_rejects_degenerate_terrain_without_panicking() {
        use crate::dto::{AtmosphereDto, CoherenceInputsDto, PrepareSolveReq};
        use envi_compute::scene_dto::{GroundSegmentDto, TerrainProfileDto};

        let req = PrepareSolveReq {
            tensor_hash: "x".to_string(),
            n_sub: 1,
            // Non-ascending X — the engine's validating constructor rejects it.
            terrain: TerrainProfileDto {
                points: vec![[10.0, 0.0], [5.0, 0.0]],
                segments: vec![GroundSegmentDto {
                    flow_resistivity: 200.0,
                    roughness: 0.0,
                }],
            },
            atmosphere: AtmosphereDto {
                temperature_c: 15.0,
                humidity_pct: 70.0,
                pressure_kpa: 101.325,
            },
            coherence: CoherenceInputsDto {
                cv2: 0.0,
                ct2: 0.0,
                t_air_c: 15.0,
                c0: C0,
                roughness_r: 0.0,
                f_delta_nu: 1.0,
                d_m: 97.5,
            },
            weather: None,
            sub_sources: vec![],
            receivers: vec![],
            forest: None,
            forest_path_length_m: None,
            isolation: None,
        };
        assert!(matches!(
            PreparedScene::build(&req),
            Err(ComputeError::Prepare(_))
        ));
    }
}
