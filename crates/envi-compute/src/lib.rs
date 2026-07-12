//! # envi-compute
//!
//! The pure-Rust, **WASM-safe** compute core of Phase 10 — the natively
//! `cargo test`-able heart of the browser solve. It holds four physics-free
//! responsibilities, none of which touch `std::fs`, C, or `rayon`:
//!
//! - [`identity`] — the tensor-identity closure factored out of `envi-store`:
//!   the frozen `blake3` [`identity::tensor_hash`], the [`identity::CalcManifest`]
//!   struct, and [`identity::chunk_receivers`] (which reads the engine's own
//!   budget constants — never re-derives them). `envi-store` re-exports these so
//!   its public API stays source-compatible; the browser calls them directly
//!   without `std::fs`.
//! - [`cost`] — the pure cost model + two-level guardrail (SC1): receiver count,
//!   tensor bytes, working-set bytes, and a device-adaptive time estimate keyed
//!   off the final spacing (D-06), plus `Warn`/`Block` thresholds.
//! - [`tiers`] — the hierarchical `points ⊂ coarse ⊂ fine` receiver partition
//!   (D-05/D-06): coarse points are a strict subset of fine, so the fine tier
//!   lists only the gap points — no receiver is ever recomputed.
//! - [`job_assembly`] — the `SolveJob` assembly that wires the directional-phase
//!   seam (SRC-03): it populates `SolveJob::directivity_phase_rad` from
//!   `DirectivityBalloon::eval_phase`, the first construction site to do so.
//! - [`scene_dto`] + [`interpolate`] — the WASM-safe scene DTOs (terrain, ground,
//!   authored isolation spectrum, forest params, sound-speed profile) and the
//!   band-index interpolation core, factored out of `envi-store`/`envi-gis-wasm`
//!   (10-06) so the browser solve boundary can marshal a scene without dragging
//!   `std::fs`/`tempfile` into wasm. Both re-export at their original paths.
//!
//! # Why pure Rust
//!
//! Everything downstream (the wasm cdylib, the worker, the UI) calls into this
//! crate. Keeping it pure Rust means the exact FORCE-validated
//! `envi_engine::solver::solve` path runs unchanged in the browser, and every
//! non-boundary responsibility is unit-testable with plain `cargo test`. The
//! engine stays byte-identical (D-02): this crate depends on `envi-engine` but
//! adds nothing to the engine's `ndarray + num-complex + thiserror` quarantine.
//!
//! # No I/O, no logic drift
//!
//! `#![deny(unsafe_code)]`; typed errors, never panics on data. The `std::fs`
//! manifest I/O (`write_manifest`/`read_manifest`) and the OPFS/rayon/wasm glue
//! live OUTSIDE this crate (`envi-store` and `envi-compute-wasm` respectively).
#![deny(unsafe_code)]

pub mod cost;
pub mod identity;
pub mod interpolate;
pub mod job_assembly;
pub mod readout;
pub mod scene_dto;
pub mod tiers;
pub mod weighting;
