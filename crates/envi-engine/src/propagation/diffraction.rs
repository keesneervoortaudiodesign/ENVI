//! Wedge diffraction (AV 1106/07 §5.7, Eqs. 78–107): Hadden–Pierce
//! finite-impedance wedge `pwedge`/`Dwedge`, two-wedge `p2wedge`, thick-screen
//! `p2edge`, non-reflecting `pwedge0`.
//!
//! **Module shell — implemented in plan 02-03.** The only consumer of the
//! Fresnel-integral fits [`super::special::fresnel_f`] / [`super::special::fresnel_g`]
//! and of ground `Q̂` on wedge faces ([`super::ground::spherical_q`], Eq. 80).
//! Registered here in wave 1 so wave-2 plans own disjoint file sets.
//!
//! Nord2000-native complex convention (e^{−jωt}); see [`super::special`].
