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
//! - [`divergence`] — `ΔL_d = −10·log10(4πR²)` (Eq. 330).
//! - [`air_absorption`] — ISO 9613-1 pure-tone α (Eq. 286) + the Nord2000
//!   band correction (Eq. 287).
//! - [`sound_speed_ms`] — `c = 20.05·√(t + 273.15)` (Eq. 335).
//! - [`direct_path`] — assembles the complex [`TransferSpectrum`] with the
//!   phase primitive `τ = R/c` (see [`crate::transfer`] for the convention).
//!
//! All functions are pure over `f64` / `Complex<f64>`; domain violations return
//! a typed [`PropagationError`] and never panic on data.

use thiserror::Error;

use crate::freq::FreqAxis;
use crate::geometry::PathGeometry;
use crate::transfer::TransferSpectrum;

pub mod air_absorption;
pub mod divergence;

use air_absorption::{Atmosphere, alpha_db_per_m, band_attenuation_db};
use divergence::divergence_amplitude;

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
    /// a clamp (01-RESEARCH §6).
    #[error("degenerate range: R = {r_m} m must be positive and finite")]
    DegenerateRange {
        /// The rejected range, m.
        r_m: f64,
    },
}

/// Speed of sound in air, `c = Coft(t) = 20.05·√(t + 273.15)` (AV 1106/07
/// Eq. 335). `t` in °C, `c` in m/s (15 °C → 340.29 m/s).
#[must_use]
pub fn sound_speed_ms(t_air_c: f64) -> f64 {
    20.05 * (t_air_c + 273.15).sqrt()
}

/// The complex free-field direct-path transfer spectrum for one
/// source→receiver path.
///
/// Returns one [`Complex<f64>`](num_complex::Complex) per 1/12-octave point
/// (length [`N_BANDS`](crate::freq::N_BANDS)). Per grid point `f`:
///
/// ```text
/// τ   = R / c                             travel time (the carried primitive)
/// A₀  = alpha_db_per_m(f, atmos) · R      pure-tone path attenuation, dB
/// ΔLₐ = band_attenuation_db(A₀)           Nord2000 band correction, dB ≥ 0
/// |H| = 10^(−ΔLₐ/20) · 1/√(4πR²)          magnitude ⇒ L_p = L_W + 20·log10|H|
/// H   = |H| · e^(−j·2π·f·τ)               phase from τ, not kR
/// ```
///
/// The phase convention is live from day one so Phase 2's two-path interference
/// already rides on `e^(−jωτ)` (ENG-01).
///
/// # Errors
///
/// [`PropagationError::DegenerateRange`] if the path range `R` is not strictly
/// positive and finite (propagated from [`divergence::divergence_amplitude`]).
pub fn direct_path(
    path: &PathGeometry,
    atmos: &Atmosphere,
    axis: &FreqAxis,
) -> Result<TransferSpectrum, PropagationError> {
    use num_complex::Complex;

    let r = path.r_m;
    // Divergence amplitude 1/√(4πR²); also validates R > 0 (domain error).
    let amp_div = divergence_amplitude(r)?;
    let c = sound_speed_ms(atmos.t_air_c);
    let tau = r / c; // the carried primitive (Phase 3 replaces Δτ here)

    let values: TransferSpectrum = axis
        .centres
        .iter()
        .map(|&f| {
            let a0 = alpha_db_per_m(f, atmos) * r; // pure-tone path attenuation, dB
            let dla = band_attenuation_db(a0); // Nord2000 band correction, dB ≥ 0
            let amp = amp_div * 10f64.powf(-dla / 20.0);
            let phase = -std::f64::consts::TAU * f * tau; // e^(−j·2πf·τ)
            Complex::from_polar(amp, phase)
        })
        .collect();

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freq::FreqAxis;
    use approx::assert_relative_eq;
    use divergence::divergence_db;

    fn force_atmosphere() -> Atmosphere {
        Atmosphere::new(15.0, 70.0, 101.325).unwrap()
    }

    /// A straight-line path of range `r_m` (only `r_m` is consumed by
    /// `direct_path`; `PathGeometry` fields are public).
    fn path_of_range(r_m: f64) -> PathGeometry {
        PathGeometry {
            r_m,
            horizontal_m: r_m,
            azimuth_deg: 90.0,
        }
    }

    #[test]
    fn sound_speed_at_15c_matches_eq_335() {
        // Eq. 335 verbatim: 20.05·√(15 + 273.15) = 20.05·√288.15 = 340.348 m/s.
        // (The frequently-cited textbook 340.3 m/s uses the sharper coefficient
        // ≈20.047; Nord2000's rounded 20.05 is the frozen contract every phase-τ
        // depends on, so the formula — not the ~0.05 m/s-lower reference — wins.)
        assert_relative_eq!(sound_speed_ms(15.0), 340.348, epsilon = 1e-2);
    }

    #[test]
    fn half_wavelength_paths_cancel_proving_the_phase_is_live() {
        // Two unit-amplitude phasors (phase extracted from direct_path, so the
        // production phase formula is exercised) differing by λ/2 must cancel.
        let atmos = force_atmosphere();
        let axis = FreqAxis::new();
        let f = axis.centres[64]; // exactly 1000 Hz
        let c = sound_speed_ms(15.0);
        let r1 = 100.0;
        let r2 = r1 + c / f / 2.0; // + half a wavelength

        let h1 = direct_path(&path_of_range(r1), &atmos, &axis).unwrap()[64];
        let h2 = direct_path(&path_of_range(r2), &atmos, &axis).unwrap()[64];
        // Normalize out the (real, positive) magnitudes → pure phase phasors.
        let unit1 = h1 / h1.norm();
        let unit2 = h2 / h2.norm();
        assert!(
            (unit1 + unit2).norm() < 1e-10,
            "half-wavelength phasors must cancel: |sum| = {}",
            (unit1 + unit2).norm()
        );
    }

    #[test]
    fn magnitude_identity_matches_divergence_plus_band_absorption() {
        let atmos = force_atmosphere();
        let axis = FreqAxis::new();
        let r = 100.0;
        let h = direct_path(&path_of_range(r), &atmos, &axis).unwrap();
        assert_eq!(h.len(), crate::freq::N_BANDS);

        let f = axis.centres[64]; // 1000 Hz grid point
        let got_db = 20.0 * h[64].norm().log10();
        let want_db =
            divergence_db(r).unwrap() - band_attenuation_db(alpha_db_per_m(f, &atmos) * r);
        assert_relative_eq!(got_db, want_db, epsilon = 1e-9);
    }

    #[test]
    fn phase_identity_is_minus_two_pi_f_tau() {
        let atmos = force_atmosphere();
        let axis = FreqAxis::new();
        let r = 100.0;
        let c = sound_speed_ms(15.0);
        let tau = r / c;
        let h = direct_path(&path_of_range(r), &atmos, &axis).unwrap();

        // Wrap into (−π, π] before comparing (τ is the computed primitive).
        let wrap = |a: f64| {
            ((a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)) - std::f64::consts::PI
        };
        for &i in &[8usize, 40, 64, 88, 104] {
            let f = axis.centres[i];
            let want = -std::f64::consts::TAU * f * tau;
            assert_relative_eq!(wrap(h[i].arg()), wrap(want), epsilon = 1e-9);
        }
    }

    #[test]
    fn output_is_genuinely_complex_nonzero_imaginary() {
        let atmos = force_atmosphere();
        let axis = FreqAxis::new();
        let h = direct_path(&path_of_range(100.0), &atmos, &axis).unwrap();
        assert!(
            h.iter().any(|z| z.im.abs() > 1e-12),
            "the transfer spectrum must carry a live (non-zero) imaginary part"
        );
    }

    #[test]
    fn degenerate_range_propagates_a_domain_error() {
        let atmos = force_atmosphere();
        let axis = FreqAxis::new();
        assert!(matches!(
            direct_path(&path_of_range(0.0), &atmos, &axis).unwrap_err(),
            PropagationError::DegenerateRange { .. }
        ));
    }
}
