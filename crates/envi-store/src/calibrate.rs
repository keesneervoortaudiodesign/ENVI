//! Source SPL-at-reference → sound-power (`L_W`) back-calculation (WEB-02, SVC-07).
//!
//! # Module I/O
//! - **Input:** a measured/authored free-field SPL spectrum `spl_db: &[f64]` (dense
//!   `[105]` by band **index**, never nominal Hz) and a positive
//!   `reference_distance_m` (the distance the SPL was specified at).
//! - **Output:** the dense `[105]` sound-power `L_W` spectrum, one value per band
//!   index. `L_W[i] = SPL[i] + 20·log10(r) + 10·log10(4π)` — free-field spherical
//!   spreading, applied per band. The `+20·log10(r)+10·log10(4π)` term is
//!   **frequency-independent** (broadband distance geometry), so this is not
//!   Hz-keyed acoustic math — but it IS acoustic, so it lives SERVER-side here
//!   (SVC-07: the browser never does it), mirroring how the interpolation core
//!   lives in [`crate::interpolate`] rather than the client.
//! - **Valid input range:** `spl_db.len()` must equal [`N_BANDS`] (= 105); every
//!   value must be finite; `reference_distance_m` must be finite and `> 0`.
//!
//! # Why the store owns this (not the service handler, not the browser)
//!
//! `envi-service`'s meta handler stays a thin wrapper (no acoustic math inline,
//! same discipline as `interpolate-spectrum`); `envi-engine` is byte-identical
//! (serde/DTO-free quarantine). The store already owns the serde DTOs and the
//! band-index interpolation core, so the one small acoustic back-calc the SPL
//! calibration needs belongs beside them.

use envi_engine::freq::N_BANDS;

use crate::StoreError;

/// Free-field spherical-spreading level term `10·log10(4π)` (≈ 10.99 dB): the
/// constant offset between a point source's sound power `L_W` and the SPL it
/// produces at 1 m in a free field. Computed once from first principles (not a
/// hardcoded 11.0) so it is exact.
fn free_field_constant() -> f64 {
    10.0 * (4.0 * std::f64::consts::PI).log10()
}

/// Back-calculate the dense `[105]` sound-power `L_W` from a free-field SPL
/// spectrum measured/specified at `reference_distance_m`.
///
/// `L_W[i] = SPL[i] + 20·log10(r) + 10·log10(4π)` for every band index `i`. The
/// distance term is frequency-independent, so the SPL spectrum's shape is
/// preserved and only its level is lifted.
///
/// # Errors
/// - [`StoreError::BadBandCount`] if `spl_db.len()` ≠ [`N_BANDS`].
/// - [`StoreError::NonFinite`] if any SPL value is non-finite, or if
///   `reference_distance_m` is non-finite or `<= 0`.
pub fn spl_to_lw(spl_db: &[f64], reference_distance_m: f64) -> Result<Vec<f64>, StoreError> {
    if spl_db.len() != N_BANDS {
        return Err(StoreError::BadBandCount { got: spl_db.len() });
    }
    if !reference_distance_m.is_finite() || reference_distance_m <= 0.0 {
        return Err(StoreError::NonFinite {
            what: format!("reference_distance_m = {reference_distance_m} (must be finite and > 0)"),
        });
    }
    for (i, v) in spl_db.iter().enumerate() {
        if !v.is_finite() {
            return Err(StoreError::NonFinite {
                what: format!("spl_db[{i}] = {v}"),
            });
        }
    }

    let offset = 20.0 * reference_distance_m.log10() + free_field_constant();
    Ok(spl_db.iter().map(|&lp| lp + offset).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At 1 m the correction is exactly `10·log10(4π)` (the free-field constant),
    /// applied identically to every band (shape preserved).
    #[test]
    fn at_one_metre_offset_is_free_field_constant() {
        let spl: Vec<f64> = (0..N_BANDS).map(|k| k as f64 * 0.5 + 3.0).collect();
        let lw = spl_to_lw(&spl, 1.0).expect("valid");
        let k = free_field_constant();
        for (i, (&lp, &lw_i)) in spl.iter().zip(lw.iter()).enumerate() {
            assert!((lw_i - (lp + k)).abs() < 1e-12, "band {i}");
        }
    }

    /// Doubling the distance adds `20·log10(2)` ≈ 6.02 dB uniformly.
    #[test]
    fn distance_term_is_twenty_log10_r() {
        let spl = vec![50.0; N_BANDS];
        let lw = spl_to_lw(&spl, 10.0).expect("valid");
        let expected = 50.0 + 20.0 * 10.0_f64.log10() + free_field_constant();
        for &v in &lw {
            assert!((v - expected).abs() < 1e-12);
        }
    }

    /// Wrong length ⇒ BadBandCount; non-positive / non-finite distance ⇒ NonFinite.
    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            spl_to_lw(&[0.0; 8], 1.0),
            Err(StoreError::BadBandCount { got: 8 })
        ));
        assert!(matches!(
            spl_to_lw(&[0.0; N_BANDS], 0.0),
            Err(StoreError::NonFinite { .. })
        ));
        assert!(matches!(
            spl_to_lw(&[0.0; N_BANDS], -2.0),
            Err(StoreError::NonFinite { .. })
        ));
        let mut spl = vec![1.0; N_BANDS];
        spl[7] = f64::NAN;
        assert!(matches!(
            spl_to_lw(&spl, 1.0),
            Err(StoreError::NonFinite { .. })
        ));
    }
}
