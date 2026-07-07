//! Terrain-effect composition ΔL_t (AV 1106/07 §5.10–5.16, §5.22 Eq. 332):
//! Sub-models 1/2 (ground) and 4/5/6/7 (screens), combined by the §5.21
//! transition parameters into the per-band terrain excess-attenuation.
//!
//! **Module shell — implemented in plans 02-02 (Sub-models 1/2) and 02-04
//! (screen sub-models + Eq. 332 composition).** Registered here in wave 1 so
//! wave-2 plans own disjoint file sets and never conflict on
//! `propagation/mod.rs`.
//!
//! Nord2000-native complex convention (e^{−jωt}); the single conjugation to
//! ENVI's e^{+jωt} transfer convention happens in `transfer.rs` (plan 02-05).
