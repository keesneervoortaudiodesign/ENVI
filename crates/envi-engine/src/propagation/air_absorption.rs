//! ISO 9613-1 pure-tone atmospheric absorption + the Nord2000 band correction.
//!
//! # Sources
//!
//! - Pure-tone attenuation coefficient α(f): **ISO 9613-1 per AV 1106/07
//!   Eq. (286)**. The transcription here was numerically cross-verified against
//!   the published ISO 9613-2 Table 2 values (20 °C / 70 % / 1 atm: 5.0 / 22.9 /
//!   76.6 dB/km at 1 / 4 / 8 kHz) during research — implemented verbatim.
//! - Band correction ΔLₐ from pure-tone α·R: **AV 1106/07 Eq. (287)**
//!   (identical to AV 1849/00 Eq. (3)).
//!
//! # Validity note
//!
//! ISO 9613-1 specifies the range 50 Hz – 10 kHz. Nord2000 applies it from
//! 25 Hz, where α is negligible (~0.02 dB/km); this extrapolation is standard
//! practice and documented here rather than special-cased.
//!
//! # Three-stage transcription pinning
//!
//! The intermediates [`molar_h2o_percent`], [`f_r_oxygen`] and [`f_r_nitrogen`]
//! are exposed so unit tests can anchor the transcription stage by stage: a
//! wrong constant surfaces at the intermediate it corrupts, not as a mystery
//! 2 % drift in the final α.

use super::PropagationError;

/// Reference air temperature `T0 = 293.15 K` (20 °C).
const T0_K: f64 = 293.15;

/// Triple-point temperature of water `T01 = 273.16 K`.
const T01_K: f64 = 273.16;

/// Reference ambient pressure `p_r = 101.325 kPa`.
const P_R_KPA: f64 = 101.325;

/// Band-correction input clamp, dB. The Eq. (287) polynomial turns over near
/// ~410 dB; clamping A₀ at 300 dB (beyond audibility) keeps it monotonic
/// (01-RESEARCH §4 guard).
const BAND_CORRECTION_CLAMP_DB: f64 = 300.0;

/// Atmospheric state for air-absorption evaluation.
///
/// Construct via [`Atmosphere::new`], which validates domains (T > −273.15 °C,
/// RH ∈ [0,100], p > 0) and returns a typed [`PropagationError`] — never panics
/// on untrusted case data (threat T-01-08 / ASVS V5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atmosphere {
    /// Air temperature at ground, °C.
    pub t_air_c: f64,
    /// Relative humidity, %.
    pub rh_percent: f64,
    /// Ambient pressure, kPa.
    pub pressure_kpa: f64,
}

impl Atmosphere {
    /// Validate and construct an atmosphere.
    ///
    /// # Errors
    ///
    /// - [`PropagationError::InvalidTemperature`] if `t_air_c` ≤ −273.15 °C or
    ///   non-finite
    /// - [`PropagationError::InvalidHumidity`] if `rh_percent` ∉ [0, 100] or
    ///   non-finite
    /// - [`PropagationError::InvalidPressure`] if `pressure_kpa` ≤ 0 or
    ///   non-finite
    pub fn new(t_air_c: f64, rh_percent: f64, pressure_kpa: f64) -> Result<Self, PropagationError> {
        // Reject non-finite explicitly (NaN and ±∞): a bare `x > bound` lets
        // +∞ through, which would poison every downstream α as NaN/∞.
        if !t_air_c.is_finite() || t_air_c <= -273.15 {
            return Err(PropagationError::InvalidTemperature { t_air_c });
        }
        if !rh_percent.is_finite() || !(0.0..=100.0).contains(&rh_percent) {
            return Err(PropagationError::InvalidHumidity { rh_percent });
        }
        if !pressure_kpa.is_finite() || pressure_kpa <= 0.0 {
            return Err(PropagationError::InvalidPressure { pressure_kpa });
        }
        Ok(Self {
            t_air_c,
            rh_percent,
            pressure_kpa,
        })
    }

    /// Absolute temperature in kelvin.
    fn temperature_k(&self) -> f64 {
        self.t_air_c + 273.15
    }

    /// Pressure ratio `p_a / p_r`.
    fn pressure_ratio(&self) -> f64 {
        self.pressure_kpa / P_R_KPA
    }
}

/// Molar concentration of water vapour `h` (%), ISO 9613-1:
///
/// ```text
/// C = −6.8346·(T01/T)^1.261 + 4.6151
/// h = RH · 10^C / p_rel
/// ```
#[must_use]
pub fn molar_h2o_percent(atmos: &Atmosphere) -> f64 {
    let t = atmos.temperature_k();
    let p_rel = atmos.pressure_ratio();
    let c = -6.8346 * (T01_K / t).powf(1.261) + 4.6151;
    atmos.rh_percent * 10f64.powf(c) / p_rel
}

/// Oxygen relaxation frequency `f_rO` (Hz), ISO 9613-1:
///
/// ```text
/// f_rO = p_rel · ( 24 + 4.04e4·h·(0.02 + h)/(0.391 + h) )
/// ```
#[must_use]
pub fn f_r_oxygen(atmos: &Atmosphere) -> f64 {
    let p_rel = atmos.pressure_ratio();
    let h = molar_h2o_percent(atmos);
    p_rel * (24.0 + 4.04e4 * h * (0.02 + h) / (0.391 + h))
}

/// Nitrogen relaxation frequency `f_rN` (Hz), ISO 9613-1:
///
/// ```text
/// f_rN = p_rel · (T/T0)^(−1/2) · ( 9 + 280·h·exp(−4.170·((T/T0)^(−1/3) − 1)) )
/// ```
#[must_use]
pub fn f_r_nitrogen(atmos: &Atmosphere) -> f64 {
    let t = atmos.temperature_k();
    let p_rel = atmos.pressure_ratio();
    let h = molar_h2o_percent(atmos);
    let tr = t / T0_K;
    p_rel * tr.powf(-0.5) * (9.0 + 280.0 * h * (-4.170 * (tr.powf(-1.0 / 3.0) - 1.0)).exp())
}

/// Pure-tone atmospheric absorption coefficient α(f), dB/m (ISO 9613-1
/// Eq. (286)):
///
/// ```text
/// α = 8.686·f²·[ 1.84e-11·(1/p_rel)·(T/T0)^(1/2)
///        + (T/T0)^(−5/2)·( 0.01275·e^(−2239.1/T)/(f_rO + f²/f_rO)
///                        + 0.1068 ·e^(−3352.0/T)/(f_rN + f²/f_rN) ) ]
/// ```
///
/// Evaluate at the EXACT grid centre frequency, never the nominal label — α
/// grows as f², so nominal-vs-exact is a systematic error (01-RESEARCH
/// Pitfall 3).
#[must_use]
pub fn alpha_db_per_m(f_hz: f64, atmos: &Atmosphere) -> f64 {
    let t = atmos.temperature_k();
    let p_rel = atmos.pressure_ratio();
    let f_ro = f_r_oxygen(atmos);
    let f_rn = f_r_nitrogen(atmos);
    let tr = t / T0_K;
    let f2 = f_hz * f_hz;
    8.686
        * f2
        * (1.84e-11 * (1.0 / p_rel) * tr.sqrt()
            + tr.powf(-2.5)
                * (0.01275 * (-2239.1 / t).exp() / (f_ro + f2 / f_ro)
                    + 0.1068 * (-3352.0 / t).exp() / (f_rn + f2 / f_rn)))
}

/// Nord2000 band correction (AV 1106/07 Eq. (287)) converting a pure-tone path
/// attenuation `A₀ = α·R` (dB) into a 1/3-octave **band** attenuation
/// magnitude (dB, non-negative):
///
/// ```text
/// ΔLₐ = A₀ · (1.0053255 − 0.00122622·A₀)^1.6      (A₀ clamped at 300 dB)
/// ```
///
/// This is the ONLY function that converts pure-tone α·R into a band-level
/// correction (01-RESEARCH Pitfall 4): raw `alpha·R` must never be used as a
/// level correction anywhere else. `direct_path` calls this at every one of the
/// 105 grid points, each treated as a band centre — the deliberate finer-grid
/// deviation (01-RESEARCH Assumption A5, revisit in Phase 4).
#[must_use]
pub fn band_attenuation_db(a0_db: f64) -> f64 {
    let a0 = a0_db.min(BAND_CORRECTION_CLAMP_DB);
    a0 * (1.0053255 - 0.00122622 * a0).powf(1.6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freq::FreqAxis;
    use approx::assert_relative_eq;

    /// FORCE reference conditions: 15 °C, 70 % RH, 101.325 kPa.
    fn force_atmosphere() -> Atmosphere {
        Atmosphere::new(15.0, 70.0, 101.325).unwrap()
    }

    #[test]
    fn intermediates_pin_the_transcription_at_force_conditions() {
        let atmos = force_atmosphere();
        // Stage-by-stage anchors (01-RESEARCH Verified Physics §3): a wrong
        // constant surfaces at the intermediate it corrupts.
        assert_relative_eq!(molar_h2o_percent(&atmos), 1.177_222, max_relative = 1e-6);
        assert_relative_eq!(f_r_oxygen(&atmos), 36_332.37, max_relative = 1e-4);
        assert_relative_eq!(f_r_nitrogen(&atmos), 333.6691, max_relative = 1e-4);
    }

    #[test]
    fn published_iso_9613_2_table_2_cross_check_20c_70rh() {
        // Independent published anchors (ISO 9613-2 Table 2), 20 °C / 70 % / 1 atm,
        // evaluated at the table's nominal frequencies; within table rounding (2 %).
        let atmos = Atmosphere::new(20.0, 70.0, 101.325).unwrap();
        assert_relative_eq!(
            alpha_db_per_m(1000.0, &atmos) * 1000.0,
            5.0,
            max_relative = 0.02
        );
        assert_relative_eq!(
            alpha_db_per_m(4000.0, &atmos) * 1000.0,
            22.9,
            max_relative = 0.02
        );
        assert_relative_eq!(
            alpha_db_per_m(8000.0, &atmos) * 1000.0,
            76.6,
            max_relative = 0.02
        );
    }

    #[test]
    fn alpha_regression_anchors_at_exact_grid_centres_force_conditions() {
        let atmos = force_atmosphere();
        let axis = FreqAxis::new();
        // Assert at EXACT grid frequencies (third-octave picks), not nominal
        // labels (Pitfall 3). Anchors printed to 7 sig digits — 1e-6 relative.
        let cases = [
            (0usize, 1.709_912e-5), // 25.118864 Hz
            (6, 2.512_361e-4),      // ~100 Hz (exact 100.0-nominal centre)
            (12, 1.921_459e-3),     // 398.107 Hz
            (16, 4.079_240e-3),     // 1000 Hz
            (22, 2.638_566e-2),     // 3981.07 Hz
            (26, 1.435_243e-1),     // 10000 Hz
        ];
        for (third_idx, want) in cases {
            let f = axis.third_octave_pick(third_idx);
            assert_relative_eq!(alpha_db_per_m(f, &atmos), want, max_relative = 1e-6);
        }
    }

    #[test]
    fn band_correction_anchors_and_clamp() {
        // Anchors (01-RESEARCH §4): the "0.6 dB less than pure tone at 20 dB".
        assert_relative_eq!(band_attenuation_db(20.0), 19.389_18, epsilon = 1e-4);
        assert_relative_eq!(band_attenuation_db(10.0), 9.89, epsilon = 0.01);
        assert_relative_eq!(band_attenuation_db(100.0), 81.9, epsilon = 0.1);

        // Monotonic non-decreasing across 0..300.
        let mut prev = f64::NEG_INFINITY;
        for i in 0..=300 {
            let v = band_attenuation_db(f64::from(i));
            assert!(
                v >= prev,
                "band correction must be non-decreasing at A0 = {i}"
            );
            prev = v;
        }

        // Above the clamp threshold the result does not decrease (polynomial
        // would otherwise turn over): 350 dB clamps to the 300 dB value.
        assert_relative_eq!(band_attenuation_db(350.0), band_attenuation_db(300.0));
        assert!(band_attenuation_db(350.0) >= band_attenuation_db(300.0));
    }

    #[test]
    fn atmosphere_domain_validation_is_typed() {
        // Humidity outside [0, 100].
        assert_eq!(
            Atmosphere::new(15.0, 150.0, 101.325).unwrap_err(),
            PropagationError::InvalidHumidity { rh_percent: 150.0 }
        );
        assert!(matches!(
            Atmosphere::new(15.0, -1.0, 101.325).unwrap_err(),
            PropagationError::InvalidHumidity { .. }
        ));
        // Temperature at/below absolute zero.
        assert!(matches!(
            Atmosphere::new(-300.0, 70.0, 101.325).unwrap_err(),
            PropagationError::InvalidTemperature { .. }
        ));
        // Non-positive pressure.
        assert!(matches!(
            Atmosphere::new(15.0, 70.0, 0.0).unwrap_err(),
            PropagationError::InvalidPressure { .. }
        ));
        // Non-finite inputs are rejected, never propagated as NaN.
        assert!(Atmosphere::new(f64::NAN, 70.0, 101.325).is_err());
        assert!(Atmosphere::new(15.0, f64::NAN, 101.325).is_err());
        assert!(Atmosphere::new(15.0, 70.0, f64::INFINITY).is_err());

        // A valid atmosphere constructs.
        assert!(Atmosphere::new(15.0, 70.0, 101.325).is_ok());
    }
}
