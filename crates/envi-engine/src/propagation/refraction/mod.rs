//! Meteorological refraction (AV 1106/07 §5.3.2, §5.5, §5.23.9, §5.23.16).
//!
//! The circular-ray machinery that turns a log-lin sound-speed profile into the
//! ray variables (`τ`, `R`, `Δτ`, `dSZ`) the existing Sub-model 1/2 composition
//! consumes. Refraction in Nord2000 is a *transformation of the ray inputs* to
//! the already-built sub-models, not a new propagation model:
//!
//! - [`profile`] — the log-lin effective sound speed `c(z)` (Eqs. 2–3).
//! - [`eqssp`] — CalcEqSSP: collapse `c(z)` to an equivalent-linear profile
//!   `(ξ, c₀)` (Eqs. 15–21 + Annex F Eq. 403).
//! - [`circular_ray`] — DirectRay / ReflectedRay / reflection-point cubic /
//!   travel-time difference / HeightOfCircularRay (Eqs. 29–56, 355–368).
//! - [`shadow_zone`] — upward-refraction shadow-zone shielding via the
//!   equivalent-wedge diffraction kernel (Eqs. 384–388).
//!
//! # Convention (load-bearing, D-13)
//!
//! All refraction math is **Nord2000-native** (time e^{−jωt}). There is **no
//! `.conj()` anywhere in this module** — the single conjugation to ENVI's
//! e^{+jωt} transfer convention stays at `transfer::nord_ratio_to_transfer`.
//! Most of the refraction math is real-valued; where a conjugate would be
//! needed it is written as an explicit `Complex::new(re, -im)`.
//!
//! # The two ξ clamps (do not confuse — RESEARCH Pitfall 1)
//!
//! - `|ξ| < 1e-6` ⇒ homogeneous shortcut (CalcEqSSP returns `ξ=0, c₀=C`);
//!   `rays::circular_rays` (task 2) delegates to `straight_rays` so the D-02
//!   bit-for-bit anchor is structural.
//! - `|ξ'| < 1e-10` ⇒ the *inner* DirectRay division guard (the circular
//!   formulas are undefined at exactly ξ=0). Distinct threshold, distinct role.

pub mod circular_ray;
pub mod eqssp;
pub mod profile;
pub mod shadow_zone;

/// A log-lin sound-speed profile `c(z) = A·ln(z/z₀+1) + B·z + C` (Eq. 2).
///
/// The weather-route output the engine's refraction entry point consumes.
/// `a`/`b`/`c` are the log/linear/ground coefficients (m/s, s⁻¹, m/s);
/// `z0` is the roughness length, m (clamped ≥ 0.001 m at use).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundSpeedProfile {
    /// Coefficient of the logarithmic part `A`, m/s.
    pub a: f64,
    /// Coefficient of the linear part `B`, s⁻¹.
    pub b: f64,
    /// Ground sound speed `C = Coft(t₀)`, m/s.
    pub c: f64,
    /// Roughness length `z₀`, m (clamped ≥ 0.001 m).
    pub z0: f64,
}

impl SoundSpeedProfile {
    /// A homogeneous profile (`A=B=0`) with ground sound speed `c` — the
    /// `|ξ|<1e-6` limit that routes through the straight-ray path.
    #[must_use]
    pub fn homogeneous(c: f64) -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            c,
            z0: profile::Z0_MIN_M,
        }
    }
}
