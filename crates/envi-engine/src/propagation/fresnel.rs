//! Fresnel-zone machinery (AV 1106/07 §5.23.4–5.23.7, Eqs. 338–353).
//!
//! **Module shell — implemented in plan 02-02** (Sub-model 2 surface blending
//! and the screen sub-models consume `CalcFZd` / `FresnelZoneSize` /
//! `FresnelZoneW` / `FresnelZoneWm`). Registered here in wave 1 so wave-2 plans
//! own disjoint file sets and never conflict on `propagation/mod.rs`.
//!
//! Nord2000-native complex convention (e^{−jωt}); see [`super::special`].
