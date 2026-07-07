//! Geometrical (spherical) divergence — AV 1106/07 Eq. (330).
//!
//! For the homogeneous-atmosphere straight source→receiver distance `R`:
//!
//! ```text
//! ΔL_d = −10·log10(4π·R²)          (equivalently 20·log10(1/√(4πR²)))
//! ```
//!
//! The amplitude form `1/√(4πR²)` is the free-field transfer magnitude `|H|`
//! (see [`crate::transfer`]); `divergence_db` is `20·log10|H|`, so
//! `L_p = L_W + ΔL_d` falls out of the convention by addition.

use super::PropagationError;

/// Free-field divergence **amplitude** `1/√(4π·R²)` (the transfer magnitude).
///
/// Shared by [`divergence_db`] and `direct_path` so the normalization lives in
/// one place.
///
/// # Errors
///
/// [`PropagationError::DegenerateRange`] if `r_m` is not strictly positive and
/// finite — a domain error, never a clamp (01-RESEARCH §6).
pub(crate) fn divergence_amplitude(r_m: f64) -> Result<f64, PropagationError> {
    todo!("GREEN")
}

/// Geometrical divergence `ΔL_d = −10·log10(4π·R²)`, dB (AV 1106/07 Eq. 330).
///
/// # Errors
///
/// [`PropagationError::DegenerateRange`] if `r_m` is not strictly positive and
/// finite.
pub fn divergence_db(r_m: f64) -> Result<f64, PropagationError> {
    todo!("GREEN")
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn divergence_anchors_match_eq_330_table() {
        assert_relative_eq!(divergence_db(1.0).unwrap(), -10.992_099, epsilon = 1e-6);
        assert_relative_eq!(divergence_db(10.0).unwrap(), -30.992_099, epsilon = 1e-6);
        assert_relative_eq!(divergence_db(100.0).unwrap(), -50.992_099, epsilon = 1e-6);
        assert_relative_eq!(divergence_db(1000.0).unwrap(), -70.992_099, epsilon = 1e-6);
    }

    #[test]
    fn non_positive_range_is_a_domain_error_not_a_clamp() {
        assert!(matches!(
            divergence_db(0.0).unwrap_err(),
            PropagationError::DegenerateRange { .. }
        ));
        assert!(matches!(
            divergence_db(-5.0).unwrap_err(),
            PropagationError::DegenerateRange { .. }
        ));
        assert!(divergence_db(f64::NAN).is_err());
        assert!(divergence_db(f64::INFINITY).is_err());
    }
}
