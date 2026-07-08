//! Log-lin sound-speed profile `c(z)` (AV 1106/07 §5.3.2, Eqs. 2–3).
//!
//! # Convention
//!
//! Nord2000-native (time e^{−jωt}); this module is purely real-valued. The
//! effective sound speed `c(z) = A·ln(z/z₀+1) + B·z + C`, with the roughness
//! length `z₀` **clamped ≥ 0.001 m** (§5.3.2 p.16 explicit) to avoid ray-calc
//! singularities. `C = Coft(t₀)` is the ground sound speed (Eq. 3), reusing the
//! frozen [`crate::propagation::sound_speed_ms`] primitive (Eq. 335) — do NOT
//! reintroduce the textbook 340.29 constant.

/// The minimum roughness length `z₀` (AV 1106/07 §5.3.2 p.16): values below
/// 0.001 m are clamped to 0.001 m to avoid ray-calculation singularities.
pub const Z0_MIN_M: f64 = 0.001;

/// Log-lin effective sound speed `c(z) = A·ln(z/z₀+1) + B·z + C` (Eq. 2).
///
/// `z₀` is clamped to [`Z0_MIN_M`] before use (MET-01). `z` is the height above
/// ground (or the perpendicular slant distance to a terrain segment).
#[must_use]
pub fn sound_speed_profile(z: f64, a: f64, b: f64, c: f64, z0: f64) -> f64 {
    let z0 = z0.max(Z0_MIN_M);
    a * (z / z0 + 1.0).ln() + b * z + c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::sound_speed_ms;
    use approx::assert_relative_eq;

    // Eq. 2 verbatim at a hand-computed point.
    #[test]
    fn profile_matches_eq2() {
        // A=2, B=0.01, C=340, z0=0.1, z=5: 2·ln(51)+0.01·5+340.
        let want = 2.0 * (5.0 / 0.1 + 1.0f64).ln() + 0.01 * 5.0 + 340.0;
        assert_relative_eq!(
            sound_speed_profile(5.0, 2.0, 0.01, 340.0, 0.1),
            want,
            max_relative = 1e-12
        );
    }

    // z₀ < 0.001 clamps to 0.001 m (MET-01, success criterion 2).
    #[test]
    fn z0_below_floor_clamps_to_0_001() {
        let c = sound_speed_ms(15.0);
        // z₀ = 1e-9 and z₀ = 0.001 must give the identical result (clamp).
        let clamped = sound_speed_profile(2.0, 1.5, 0.0, c, 1e-9);
        let at_floor = sound_speed_profile(2.0, 1.5, 0.0, c, 0.001);
        assert_eq!(clamped, at_floor);
        // ... and it differs from an unclamped z₀ = 0.01 profile (proving the
        // clamp is active, not a no-op).
        let larger = sound_speed_profile(2.0, 1.5, 0.0, c, 0.01);
        assert!((clamped - larger).abs() > 1e-6);
    }

    // C = Coft(t₀) reuse: at z=0 the profile equals C exactly (ln(1)=0).
    #[test]
    fn ground_value_is_c() {
        let c = sound_speed_ms(10.0);
        assert_relative_eq!(
            sound_speed_profile(0.0, 3.0, 0.02, c, 0.05),
            c,
            max_relative = 1e-12
        );
    }
}
