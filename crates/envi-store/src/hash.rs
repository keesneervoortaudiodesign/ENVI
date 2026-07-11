//! The frozen tensor-identity content hash (D-07) — re-exported.
//!
//! The implementation moved verbatim into `envi_compute::identity` (Phase 10,
//! 10-01) so the browser compute crate can hash the tensor identity without
//! `std::fs`. `envi-store` re-exports it here so `envi_store::hash::tensor_hash`
//! stays source-compatible for every existing consumer.
//!
//! # Frozen encoding (see `envi_compute::identity`)
//!
//! `tensor_hash` is `blake3` over a **canonical byte encoding** — never over
//! serialized JSON text. The frozen layout (version prefix, domain-separated
//! field tags, `u64`-LE length prefixes, per-`f64` `to_bits().to_le_bytes()`,
//! uuid-sorted features) and the D-07 structural exclusion of conditioning live
//! in `envi_compute::identity`; the identity-covers/never-covers contract is
//! documented there. This re-export changes no bytes: a fixed input hashes to
//! the exact pre-refactor digest (pinned by a regression test in `envi-compute`).

pub use envi_compute::identity::tensor_hash;
