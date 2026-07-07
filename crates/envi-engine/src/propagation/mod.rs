//! Direct-path propagation: geometrical divergence, ISO 9613-1 air absorption,
//! and the complex transfer value per 1/12-octave point.
//!
//! This module composes the free-field physics of the Nord2000 compound model
//! (AV 1106/07 Eq. (1)/(329)) for the terms Phase 1 implements:
//!
//! ```text
//! L_R(f) = L_W(f) + ΔL_d + ΔL_a(f) + ΔL_t + ΔL_s + ΔL_r        per band
//!          \_____ Phase 1 _____/     \____ 0 (free field) ____/
//! ```
//!
//! - [`air_absorption`] — ISO 9613-1 pure-tone α (Eq. 286) + the Nord2000
//!   band correction (Eq. 287).
//! - `divergence` / `sound_speed_ms` / `direct_path` — added in plan 01-03
//!   Task 2 (the complex transfer assembly with the phase primitive `τ = R/c`).
//!
//! All functions are pure over `f64` / `Complex<f64>`; domain violations return
//! a typed [`PropagationError`] and never panic on data.

use thiserror::Error;

pub mod air_absorption;

/// Errors from the direct-path physics: domain violations at the
/// `CaseDefinition` → physics trust boundary (threat T-01-08).
///
/// Every out-of-range input yields one of these — the propagation constructors
/// and `divergence_db` never panic on data (RH ∈ [0,100], T > −273.15 °C,
/// p > 0, R > 0).
#[derive(Debug, Error, PartialEq)]
pub enum PropagationError {
    /// Air temperature at or below absolute zero (or non-finite).
    #[error("air temperature {t_air_c} °C is at or below absolute zero (−273.15 °C)")]
    InvalidTemperature {
        /// The rejected temperature, °C.
        t_air_c: f64,
    },
    /// Relative humidity outside `[0, 100]` (or non-finite).
    #[error("relative humidity {rh_percent}% is outside [0, 100]")]
    InvalidHumidity {
        /// The rejected relative humidity, %.
        rh_percent: f64,
    },
    /// Ambient pressure not strictly positive (or non-finite).
    #[error("ambient pressure {pressure_kpa} kPa must be positive")]
    InvalidPressure {
        /// The rejected pressure, kPa.
        pressure_kpa: f64,
    },
    /// Path range `R` not strictly positive and finite — a domain error, never
    /// a clamp (01-RESEARCH §6). Used by `divergence_db` in Task 2.
    #[error("degenerate range: R = {r_m} m must be positive and finite")]
    DegenerateRange {
        /// The rejected range, m.
        r_m: f64,
    },
}
