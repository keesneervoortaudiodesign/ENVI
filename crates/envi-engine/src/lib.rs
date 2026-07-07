//! # envi-engine
//!
//! Pure-math core of the ENVI Nord2000 sound-propagation engine.
//!
//! This crate implements the Nord2000 method (AV 1106/07 rev. 4) as pure
//! functions over `f64` / `Complex<f64>` values. It performs **no I/O**: no
//! file parsing, no network, no environment access. Case loading, reference
//! comparison, and reporting live in the sibling `envi-harness` crate — that
//! quarantine is what makes "test harness before propagation code" an
//! architectural property rather than a sequencing note.
//!
//! Numerics house rules (see `.planning/PROJECT.md`):
//! - `f64` throughout; no `f32` in engine code.
//! - Typed errors, never panics, on data-dependent paths.
//! - Prefer formulations that avoid subtraction of near-equal numbers.

#![deny(unsafe_code)]

pub mod freq;
pub mod geometry;
pub mod scene;
