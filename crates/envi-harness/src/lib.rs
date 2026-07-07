//! # envi-harness
//!
//! Validation harness for the ENVI Nord2000 engine. **All I/O lives here**:
//! FORCE `.xls` parsing (calamine), synthetic TOML cases (serde), reference
//! comparison with FORCE tolerances, capability gating, and reporting.
//! The engine crate (`envi-engine`) never sees a file format.

pub mod cases;
