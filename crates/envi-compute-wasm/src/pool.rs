//! Caller-side rayon sharding driver (GRID-02) — the parallelism the engine does
//! NOT have.
//!
//! `envi_engine::solver::solve` is **sequential** (verified: no `rayon` anywhere
//! in `envi-engine`); GRID-02's "parallelized (rayon)" is a *caller-side*
//! obligation. [`solve_tier`] shards a tier's receiver axis into DISJOINT chunk
//! ranges and runs one UNCHANGED engine `solve()` per range on the
//! `wasm-bindgen-rayon` pool, each range streaming into its OWN sink (its own OPFS
//! chunk file). Disjoint ranges → disjoint chunk indices → disjoint files → **no
//! shared-mutable sink, no locking** (the engine stays byte-identical, D-02).
//!
//! # Module I/O
//! - **Input:** the tier's `[ChunkRange]`, the sub-source count, a shared
//!   [`AtomicBool`] cancel flag, an `assemble` closure yielding one range's
//!   local-indexed [`SolveJob`]s, and an `open_sink` factory yielding one range's
//!   [`RangeSink`] (its own file(s)).
//! - **Output:** a [`RangeProgress`] per range (chunk index + receivers written),
//!   or the first [`ComputeError`] (a `Cancelled`, a sink I/O error, or a wrapped
//!   engine `PropagationError`). The whole tier short-circuits on the first error.
//!
//! # Local-indexed ranges (one range = one file at offset 0)
//! Each range writes its own self-contained chunk file, so its jobs carry
//! **local** receiver indices `0..len` and the driver passes `chunk_receivers =
//! range.len` — the engine then emits exactly one `put_chunk(0, …)` into that
//! range's sink. The range's GLOBAL receiver offset lives in the
//! [`ChunkRange::r_offset`] metadata the tier-complete span records (D-07), never
//! in the file layout.
//!
//! # Cooperative abort at chunk boundaries (D-11)
//! Every range checks the shared cancel flag BEFORE assembling/solving; a set flag
//! makes the range return [`ComputeError::Cancelled`] without opening its file.
//! Because `wasm-bindgen-rayon`'s linear memory IS a `SharedArrayBuffer`, a single
//! `static AtomicBool` (the crate's `CANCEL`, flipped by `request_cancel()`) is
//! visible to every pool thread — abort granularity is one chunk range, and there
//! is **no `worker.terminate()`** (the pool stays reusable; emitted tiers stay
//! valid).
//!
//! # SC3 working-set bound
//! The resident set is `workers × chunk`, never the full OPFS tensor: each range's
//! `solve()` holds ONE reusable chunk buffer, and at most `n_workers` ranges run
//! concurrently, so peak resident chunks ≤ `n_workers`. This is proven natively by
//! the `sc3_*` high-water-mark test below (the Phase-4 `CountingSink` pattern).

#![cfg(any(not(target_arch = "wasm32"), feature = "threads"))]

use std::sync::atomic::{AtomicBool, Ordering};

use envi_engine::solver::{SolveJob, solve};
use envi_engine::tensor::TensorSink;
use rayon::prelude::*;

use crate::ComputeError;
use crate::opfs_sink::OpfsChunkSink;

/// One disjoint receiver-chunk range in a tier: its OPFS chunk index and its
/// receiver-axis span. Ranges within a tier are disjoint, so their chunk indices
/// (and therefore files) are disjoint — the correctness of the lock-free design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRange {
    /// The OPFS chunk index (unique per disjoint range → unique file).
    pub chunk_index: usize,
    /// GLOBAL receiver-axis offset of this range within the tier (span metadata).
    pub r_offset: usize,
    /// Receiver count in this range (`chunk_receivers` handed to `solve()`).
    pub len: usize,
}

/// Progress for one solved range: its chunk index and the receivers written. The
/// worker folds these into a `Running { progress, message }` (chunks done / total).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeProgress {
    /// The range's OPFS chunk index.
    pub chunk_index: usize,
    /// Receivers written for this range.
    pub receivers: usize,
}

/// A per-range sink the engine `solve()` streams into, plus a `finish` that
/// flushes+closes the range's file and surfaces any I/O error as a
/// [`ComputeError`]. Implemented for the real [`OpfsChunkSink`] and for the native
/// test sinks below; each instance is created, filled, and finished ENTIRELY
/// within one rayon task (never shared across threads).
pub trait RangeSink: TensorSink {
    /// Flush + close the range's file(s); surface the first I/O error.
    ///
    /// # Errors
    /// A [`ComputeError::Sink`] wrapping the first underlying I/O failure.
    fn finish(&mut self) -> Result<(), ComputeError>;
}

impl RangeSink for OpfsChunkSink {
    fn finish(&mut self) -> Result<(), ComputeError> {
        OpfsChunkSink::finish(self).map_err(ComputeError::Sink)
    }
}

/// Solve one tier by sharding its disjoint ranges across the rayon pool.
///
/// Runs `ranges.par_iter()` — one UNCHANGED `envi_engine::solver::solve` per range
/// into that range's own [`RangeSink`] (its own file). The `assemble` closure
/// yields a range's LOCAL-indexed jobs (`receiver` in `0..range.len`) and
/// `open_sink` yields its sink; `solve()` runs with `chunk_receivers = range.len`
/// so exactly one `put_chunk(0, …)` lands per file. The shared `cancel` flag is
/// checked at each range boundary (D-11) — a set flag returns
/// [`ComputeError::Cancelled`] before the range opens its file. Disjoint ranges ⇒
/// disjoint files ⇒ no shared-mutable sink, no locking.
///
/// # Errors
/// The first [`ComputeError`] across the tier: [`ComputeError::Cancelled`] if the
/// flag was set, [`ComputeError::Sink`] on a file I/O failure, or
/// [`ComputeError::Solve`] wrapping an engine `PropagationError`. On the first
/// error the whole tier short-circuits (rayon stops scheduling new ranges).
pub fn solve_tier<'jobs, A, O, S>(
    ranges: &[ChunkRange],
    n_sub: usize,
    cancel: &AtomicBool,
    assemble: A,
    open_sink: O,
) -> Result<Vec<RangeProgress>, ComputeError>
where
    A: Fn(&ChunkRange) -> Vec<SolveJob<'jobs>> + Sync,
    O: Fn(&ChunkRange) -> Result<S, ComputeError> + Sync,
    S: RangeSink,
{
    ranges
        .par_iter()
        .map(|range| -> Result<RangeProgress, ComputeError> {
            // Cooperative abort at the chunk boundary (D-11): stop BEFORE opening
            // this range's file so a cancelled run writes nothing new.
            if cancel.load(Ordering::Relaxed) {
                return Err(ComputeError::Cancelled);
            }
            let jobs = assemble(range);
            let mut sink = open_sink(range)?;
            // One range = one file: `chunk_receivers = range.len` makes `solve()`
            // emit a single `put_chunk(0, …)` at the file's local offset 0. The
            // engine is called UNCHANGED (byte-identical, D-02).
            solve(jobs, n_sub, range.len.max(1), &mut sink)
                .map_err(|e| ComputeError::Solve(e.to_string()))?;
            sink.finish()?;
            Ok(RangeProgress {
                chunk_index: range.chunk_index,
                receivers: range.len,
            })
        })
        .collect()
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use envi_engine::directivity::{DirectivityBalloon, Rotation3};
    use envi_engine::freq::{FreqAxis, N_BANDS};
    use envi_engine::propagation::air_absorption::Atmosphere;
    use envi_engine::propagation::coherence::CoherenceInputs;
    use envi_engine::scene::{GroundSegment, TerrainProfile};
    use envi_engine::tensor::{BYTES_PER_CELL_PAIR, SinkError};
    use ndarray::ArrayView3;
    use num_complex::Complex;
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use envi_compute::job_assembly::{
        Directional, ReceiverPlacement, SolveCtx, SubSourcePlacement, assemble_jobs,
    };

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
        TerrainProfile::new(
            vec![[2.5, 0.0], [200.0, 0.0]],
            vec![GroundSegment {
                flow_resistivity: 200.0,
                roughness: 0.0,
            }],
        )
        .unwrap()
    }

    fn balloon() -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler(10.0, 10.0, |az, pol, band| {
            let a = az.to_radians();
            let p = pol.to_radians();
            (2.0 + 0.01 * band as f64) * (p.sin() * a.cos())
        })
        .unwrap()
    }

    /// A single test scene: geometry the `assemble` closures borrow (lifetime).
    struct Scene {
        profile: TerrainProfile,
        atmosphere: Atmosphere,
        coh: CoherenceInputs,
        axis: FreqAxis,
        balloon: DirectivityBalloon,
    }

    impl Scene {
        fn new() -> Self {
            Self {
                profile: flat_profile(),
                atmosphere: atmos(),
                coh: coh(),
                axis: FreqAxis::new(),
                balloon: balloon(),
            }
        }
    }

    /// The `n_sub` sub-sources for a scene (co-located omni + one directional).
    fn subs(scene: &Scene) -> Vec<SubSourcePlacement<'_>> {
        vec![
            SubSourcePlacement {
                position: [2.5, 0.0, 0.5],
                directivity: Some(Directional {
                    balloon: &scene.balloon,
                    orientation: Rotation3::identity(),
                }),
            },
            SubSourcePlacement {
                position: [2.5, 0.0, 0.8],
                directivity: None,
            },
        ]
    }

    /// LOCAL-indexed receivers `0..len` for one range (each range's own file
    /// starts at receiver 0). Positions are spread down-range so no geometry is
    /// degenerate.
    fn local_receivers(range: &ChunkRange) -> Vec<ReceiverPlacement> {
        (0..range.len)
            .map(|i| ReceiverPlacement {
                global_index: i,
                position: [100.0 + (range.r_offset + i) as f64, 0.0, 1.5],
            })
            .collect()
    }

    /// Build one range's LOCAL-indexed jobs from a scene (receiver-major).
    fn assemble_range<'a>(
        range: &ChunkRange,
        scene: &'a Scene,
        subs: &'a [SubSourcePlacement<'a>],
        receivers: &'a [ReceiverPlacement],
    ) -> Vec<SolveJob<'a>> {
        let ctx = SolveCtx {
            profile: &scene.profile,
            atmosphere: &scene.atmosphere,
            coh: &scene.coh,
            axis: &scene.axis,
            weather: None,
            sub_sources: subs,
            receivers,
            forest: None,
            isolation: None,
        };
        assemble_jobs(0..range.len, &ctx)
    }

    // --- A recording sink: proves disjoint files + captures written bytes ------

    /// A [`RangeSink`] that records `(chunk_index, byte_len)` into a shared log and
    /// the H_coh bytes into a per-index buffer — so a test can prove each disjoint
    /// range wrote its OWN file (distinct chunk index, expected size).
    struct RecordingSink {
        chunk_index: usize,
        n_sub: usize,
        n_rcv: usize,
        h_bytes: usize,
        log: std::sync::Arc<Mutex<Vec<(usize, usize)>>>,
    }

    impl TensorSink for RecordingSink {
        fn put_chunk(
            &mut self,
            r_offset: usize,
            h_coh: ArrayView3<'_, Complex<f64>>,
            p_incoh_abs: ArrayView3<'_, f64>,
        ) -> Result<(), SinkError> {
            let (hs, hr, hf) = h_coh.dim();
            // One file = one chunk at local offset 0 spanning the whole range.
            assert_eq!(r_offset, 0, "each range writes a self-contained file at 0");
            assert_eq!(hf, N_BANDS);
            assert_eq!(hs, self.n_sub);
            assert_eq!(hr, self.n_rcv);
            assert_eq!(p_incoh_abs.dim(), (hs, hr, hf));
            self.h_bytes += h_coh.len() * 16;
            Ok(())
        }
    }

    impl RangeSink for RecordingSink {
        fn finish(&mut self) -> Result<(), ComputeError> {
            self.log
                .lock()
                .unwrap()
                .push((self.chunk_index, self.h_bytes));
            Ok(())
        }
    }

    #[test]
    fn two_disjoint_ranges_write_two_disjoint_files() {
        let scene = Scene::new();
        let subs = subs(&scene);
        let n_sub = subs.len();
        let ranges = [
            ChunkRange {
                chunk_index: 0,
                r_offset: 0,
                len: 3,
            },
            ChunkRange {
                chunk_index: 1,
                r_offset: 3,
                len: 2,
            },
        ];
        // Receivers per range must outlive the closures.
        let recv0 = local_receivers(&ranges[0]);
        let recv1 = local_receivers(&ranges[1]);
        let log = std::sync::Arc::new(Mutex::new(Vec::new()));
        let cancel = AtomicBool::new(false);

        let progress = solve_tier(
            &ranges,
            n_sub,
            &cancel,
            |range| {
                let recv = if range.chunk_index == 0 {
                    &recv0
                } else {
                    &recv1
                };
                assemble_range(range, &scene, &subs, recv)
            },
            |range| {
                Ok(RecordingSink {
                    chunk_index: range.chunk_index,
                    n_sub,
                    n_rcv: range.len,
                    h_bytes: 0,
                    log: log.clone(),
                })
            },
        )
        .unwrap();

        assert_eq!(progress.len(), 2);
        let indices: BTreeSet<usize> = log.lock().unwrap().iter().map(|(i, _)| *i).collect();
        assert_eq!(
            indices,
            BTreeSet::from([0, 1]),
            "each disjoint range must write its own distinct chunk file"
        );
        // Each file holds exactly its range: n_sub · len · N_BANDS · 16 H bytes.
        for (idx, bytes) in log.lock().unwrap().iter() {
            let len = if *idx == 0 { 3 } else { 2 };
            assert_eq!(*bytes, n_sub * len * N_BANDS * 16);
        }
    }

    #[test]
    fn cancel_before_a_range_returns_cancelled_and_writes_nothing() {
        let scene = Scene::new();
        let subs = subs(&scene);
        let n_sub = subs.len();
        let ranges = [
            ChunkRange {
                chunk_index: 0,
                r_offset: 0,
                len: 3,
            },
            ChunkRange {
                chunk_index: 1,
                r_offset: 3,
                len: 3,
            },
        ];
        let recv = local_receivers(&ranges[0]);
        let log = std::sync::Arc::new(Mutex::new(Vec::new()));
        // Flag set BEFORE the solve — every range aborts at its boundary (D-11).
        let cancel = AtomicBool::new(true);

        let result = solve_tier(
            &ranges,
            n_sub,
            &cancel,
            |range| assemble_range(range, &scene, &subs, &recv),
            |range| {
                Ok(RecordingSink {
                    chunk_index: range.chunk_index,
                    n_sub,
                    n_rcv: range.len,
                    h_bytes: 0,
                    log: log.clone(),
                })
            },
        );

        assert!(
            matches!(result, Err(ComputeError::Cancelled)),
            "a set cancel flag must land the tier Cancelled"
        );
        assert!(
            log.lock().unwrap().is_empty(),
            "no range may open/finish a file once cancellation is observed"
        );
    }

    #[test]
    fn driver_runs_on_the_sized_rayon_pool() {
        // The driver runs on whatever pool it is `install`ed under — assert it sees
        // the sized pool (proves the sharding is on the wasm-bindgen-rayon pool the
        // worker sizes to navigator.hardwareConcurrency, not an ad-hoc thread).
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(3)
            .build()
            .unwrap();
        let scene = Scene::new();
        let subs = subs(&scene);
        let n_sub = subs.len();
        let ranges: Vec<ChunkRange> = (0..6)
            .map(|i| ChunkRange {
                chunk_index: i,
                r_offset: i * 2,
                len: 2,
            })
            .collect();
        let recvs: Vec<Vec<ReceiverPlacement>> = ranges.iter().map(local_receivers).collect();
        let log = std::sync::Arc::new(Mutex::new(Vec::new()));
        let cancel = AtomicBool::new(false);

        let progress = pool.install(|| {
            assert_eq!(
                rayon::current_num_threads(),
                3,
                "the driver must run on the sized pool"
            );
            solve_tier(
                &ranges,
                n_sub,
                &cancel,
                |range| assemble_range(range, &scene, &subs, &recvs[range.chunk_index]),
                |range| {
                    Ok(RecordingSink {
                        chunk_index: range.chunk_index,
                        n_sub,
                        n_rcv: range.len,
                        h_bytes: 0,
                        log: log.clone(),
                    })
                },
            )
        });
        assert_eq!(progress.unwrap().len(), 6);
    }

    // --- SC3: the working-set high-water bound (Phase-4 CountingSink pattern) ---

    /// Shared residency tracker: how many range chunk buffers are resident at once,
    /// and the peak byte high-water. Mirrors `CountingSink::high_water_bytes`.
    #[derive(Default)]
    struct Residency {
        live: AtomicUsize,
        high_water: AtomicUsize,
        live_bytes: AtomicUsize,
        high_water_bytes: AtomicUsize,
        total_bytes: AtomicUsize,
    }

    impl Residency {
        fn enter(&self, chunk_bytes: usize) {
            let live = self.live.fetch_add(1, Ordering::SeqCst) + 1;
            self.high_water.fetch_max(live, Ordering::SeqCst);
            let lb = self.live_bytes.fetch_add(chunk_bytes, Ordering::SeqCst) + chunk_bytes;
            self.high_water_bytes.fetch_max(lb, Ordering::SeqCst);
            self.total_bytes.fetch_add(chunk_bytes, Ordering::SeqCst);
        }
        fn leave(&self, chunk_bytes: usize) {
            self.live.fetch_sub(1, Ordering::SeqCst);
            self.live_bytes.fetch_sub(chunk_bytes, Ordering::SeqCst);
        }
    }

    /// A sink that marks one chunk buffer resident for its whole lifetime (open →
    /// drop spans the range's `solve()`), so peak `live` == peak concurrent ranges.
    struct ResidentSink {
        chunk_bytes: usize,
        residency: std::sync::Arc<Residency>,
    }

    impl ResidentSink {
        fn new(n_sub: usize, len: usize, residency: std::sync::Arc<Residency>) -> Self {
            // One resident chunk buffer = n_sub · len · N_BANDS · BYTES_PER_CELL_PAIR
            // (the engine's paired working buffer — the frozen constant, not a 24).
            let chunk_bytes = n_sub * len * N_BANDS * BYTES_PER_CELL_PAIR;
            residency.enter(chunk_bytes);
            Self {
                chunk_bytes,
                residency,
            }
        }
    }

    impl Drop for ResidentSink {
        fn drop(&mut self) {
            self.residency.leave(self.chunk_bytes);
        }
    }

    impl TensorSink for ResidentSink {
        fn put_chunk(
            &mut self,
            _r_offset: usize,
            _h_coh: ArrayView3<'_, Complex<f64>>,
            _p_incoh_abs: ArrayView3<'_, f64>,
        ) -> Result<(), SinkError> {
            Ok(())
        }
    }

    impl RangeSink for ResidentSink {
        fn finish(&mut self) -> Result<(), ComputeError> {
            Ok(())
        }
    }

    #[test]
    fn sc3_peak_resident_chunks_never_exceed_n_workers() {
        const N_WORKERS: usize = 4;
        const N_RANGES: usize = 64;
        const CHUNK: usize = 4;

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(N_WORKERS)
            .build()
            .unwrap();
        let scene = Scene::new();
        let subs = subs(&scene);
        let n_sub = subs.len();
        let ranges: Vec<ChunkRange> = (0..N_RANGES)
            .map(|i| ChunkRange {
                chunk_index: i,
                r_offset: i * CHUNK,
                len: CHUNK,
            })
            .collect();
        let recvs: Vec<Vec<ReceiverPlacement>> = ranges.iter().map(local_receivers).collect();
        let residency = std::sync::Arc::new(Residency::default());
        let cancel = AtomicBool::new(false);

        pool.install(|| {
            solve_tier(
                &ranges,
                n_sub,
                &cancel,
                |range| assemble_range(range, &scene, &subs, &recvs[range.chunk_index]),
                |range| Ok(ResidentSink::new(n_sub, range.len, residency.clone())),
            )
        })
        .unwrap();

        let chunk_bytes = n_sub * CHUNK * N_BANDS * BYTES_PER_CELL_PAIR;
        let full_tensor_bytes = n_sub * N_RANGES * CHUNK * N_BANDS * BYTES_PER_CELL_PAIR;

        // SC3: the resident working set is bounded by workers × chunk — never the
        // full OPFS tensor (which is workers×-larger here).
        let peak = residency.high_water.load(Ordering::SeqCst);
        assert!(
            peak <= N_WORKERS,
            "peak resident chunks ({peak}) must be ≤ n_workers ({N_WORKERS})"
        );
        assert!(peak >= 1, "the tier did solve at least one range");
        let peak_bytes = residency.high_water_bytes.load(Ordering::SeqCst);
        assert!(
            peak_bytes <= N_WORKERS * chunk_bytes,
            "peak resident bytes ({peak_bytes}) must be ≤ workers×chunk ({})",
            N_WORKERS * chunk_bytes
        );
        assert!(
            peak_bytes < full_tensor_bytes,
            "the full tensor ({full_tensor_bytes} B) must never be resident (peak {peak_bytes} B)"
        );
        // Every range's chunk was produced (all data flowed through the sink).
        assert_eq!(
            residency.total_bytes.load(Ordering::SeqCst),
            full_tensor_bytes,
            "every range must have contributed its chunk to the stream"
        );
        // The live counter is balanced back to zero.
        assert_eq!(residency.live.load(Ordering::SeqCst), 0);
    }
}
