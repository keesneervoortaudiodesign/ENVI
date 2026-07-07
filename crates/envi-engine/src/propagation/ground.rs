//! Ground reflection-coefficient chain (AV 1106/07 §5.6, Eqs. 57–77).
//!
//! **Module shell — implemented in Task 2 of plan 02-01.** Provides the
//! Delany–Bazley impedance `Ẑ_G`, plane-wave `Γ̂_p`, boundary-loss `Ê(ρ̂)`,
//! spherical-wave `Q̂`, and incoherent `ρᵢ`. Registered in wave 1.
//!
//! Nord2000-native complex convention (e^{−jωt}); see [`super::special`].
