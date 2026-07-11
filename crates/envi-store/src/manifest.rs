//! The calculation manifest: the reserved chunked-tensor layout that mirrors the
//! engine's `TensorPair`, plus honest-stub provenance (D-04, SVC-01).
//!
//! # Frozen dims + chunk layout
//!
//! A `calc/<calc_id>/manifest.json` records `dims: [S, R, F]` — `[sub_source,
//! receiver, freq]`, mirroring `envi_engine::tensor::TensorPair` (row-major, the
//! frequency axis contiguous), with `F` always [`N_BANDS`] (= 105). Chunking is
//! on the **receiver axis**: [`chunk_receivers`] is computed from the engine's
//! own budget constants (never re-derived) so Phase-9 chunk-file naming is
//! already decided. Two channel dirs are reserved empty — `tensor/` (complex
//! `H_coh`, 16 B cells) and `pincoh/` (real `P_incoh`, 8 B cells).
//!
//! # Honest-stub provenance
//!
//! `stub: bool` is the CONTEXT "must not claim real acoustic results" flag —
//! Phase 6 always writes `true`; no manifest can masquerade as real acoustics
//! (threat T-06-02-06).

use std::path::Path;

use uuid::Uuid;

use crate::StoreError;
use crate::project_dir::atomic_write;

// The pure format struct + chunk-size math moved verbatim into
// `envi_compute::identity` (Phase 10, 10-01) so the browser can build the
// manifest and size chunks without `std::fs`. Re-exported here so
// `envi_store::manifest::{CalcManifest, chunk_receivers}` stay source-compatible;
// the `std::fs` I/O below (`write_manifest`/`read_manifest`) stays in `envi-store`.
pub use envi_compute::identity::{CalcManifest, chunk_receivers};

/// Write `calc/<calc_id>/manifest.json` (atomically) and reserve the empty
/// `tensor/` + `pincoh/` channel dirs.
///
/// # Errors
/// [`StoreError`] on any filesystem or serialization failure.
pub fn write_manifest(project_dir: &Path, manifest: &CalcManifest) -> Result<(), StoreError> {
    let calc_dir = project_dir.join("calc").join(manifest.calc_id.to_string());
    std::fs::create_dir_all(&calc_dir).map_err(|source| StoreError::Io {
        path: calc_dir.clone(),
        source,
    })?;
    for channel in ["tensor", "pincoh"] {
        let dir = calc_dir.join(channel);
        std::fs::create_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })?;
    }
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| StoreError::Json {
        path: calc_dir.join("manifest.json"),
        message: e.to_string(),
    })?;
    atomic_write(&calc_dir, "manifest.json", &bytes)
}

/// Read `calc/<calc_id>/manifest.json`.
///
/// # Errors
/// [`StoreError::CalcNotFound`] if absent; [`StoreError::Json`] if malformed.
pub fn read_manifest(project_dir: &Path, calc_id: Uuid) -> Result<CalcManifest, StoreError> {
    let path = project_dir
        .join("calc")
        .join(calc_id.to_string())
        .join("manifest.json");
    let bytes = std::fs::read(&path).map_err(|_| StoreError::CalcNotFound { calc_id })?;
    serde_json::from_slice(&bytes).map_err(|e| StoreError::Json {
        path,
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::freq::N_BANDS;
    use envi_engine::tensor::{BYTES_PER_CELL_PAIR, DEFAULT_TENSOR_BUDGET_BYTES};

    #[test]
    fn manifest_reserves_chunk_layout() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let project_dir = tmp.path();
        let calc_id = Uuid::from_u128(7);

        // dims [1, 3, 105]: chunk_receivers = floor(budget / (1 · 105 · 24)) capped at 3.
        let expected = chunk_receivers(1, 3);
        assert_eq!(expected, 3, "capped at the receiver count");

        let manifest = CalcManifest {
            calc_id,
            dims: [1, 3, N_BANDS],
            chunk_receivers: expected,
            tensor_hash: "0".repeat(64),
            stub: true,
            created_at_unix: 1_700_000_000,
        };
        write_manifest(project_dir, &manifest).expect("write manifest");

        let calc_dir = project_dir.join("calc").join(calc_id.to_string());
        assert!(calc_dir.join("manifest.json").exists(), "manifest written");
        assert!(calc_dir.join("tensor").is_dir(), "tensor/ reserved");
        assert!(calc_dir.join("pincoh").is_dir(), "pincoh/ reserved");

        let round = read_manifest(project_dir, calc_id).expect("read manifest");
        assert_eq!(round, manifest, "manifest round-trips");
        assert_eq!(round.dims[2], N_BANDS, "F axis is always 105");
        assert!(round.stub, "honest-stub provenance");
    }

    #[test]
    fn chunk_formula_matches_engine_constants() {
        // Two sub-sources, 100_000 receivers: budget-bound, below R.
        let n_sub = 2;
        let per_receiver = n_sub * N_BANDS * BYTES_PER_CELL_PAIR;
        let expected = (DEFAULT_TENSOR_BUDGET_BYTES / per_receiver).min(100_000);
        assert_eq!(chunk_receivers(n_sub, 100_000), expected);
    }

    #[test]
    fn read_missing_manifest_is_calc_not_found() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let err = read_manifest(tmp.path(), Uuid::from_u128(999)).unwrap_err();
        assert!(
            matches!(err, StoreError::CalcNotFound { .. }),
            "got {err:?}"
        );
    }
}
