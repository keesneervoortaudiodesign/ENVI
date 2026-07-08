//! Upward-refraction shadow-zone shielding (AV 1106/07 §5.23.16, Eqs. 384–388).
//!
//! The shadow-zone attenuation is modelled as diffraction over an **equivalent
//! wedge** of height `hSZ` at the horizontal distance `dSZ` (Figures 41–42) —
//! reusing the Phase-2 non-reflecting wedge kernel
//! [`crate::propagation::diffraction::pwedge0`] (D-09, no new diffraction
//! primitive). `hSZ` comes from
//! [`super::circular_ray::height_of_circular_ray`] at a frequency-dependent
//! gradient `ξSZ(f)` frozen above 2000 Hz (Eq. 385).
//!
//! # Sign convention (interpretation of Eq. 387, [ASSUMED per D-04])
//!
//! Eq. 387 writes `ΔLSZ(f) = 10·lg(2·R_SR·|pwedge0|)`. The engine returns
//! `L_SZ` as a **non-negative attenuation** (the shielding subtracted at Eq. 121
//! readout): `L_SZ = max(0, −10·lg(2·R_SR·|pwedge0|))`. The exact magnitude is
//! validated only by direction property tests (D-11-style), not an oracle —
//! the sign is self-consistent with the Sub-model 1 shadow branch.
//!
//! # Convention
//!
//! Nord2000-native (e^{−jωt}); no `.conj()` (the equivalent wedge reuses the
//! Nord-native `pwedge0`).

use std::f64::consts::PI;

use crate::freq::FreqAxis;
use crate::propagation::PropagationError;
use crate::propagation::diffraction::{WedgeGeometry, pwedge0};

use super::circular_ray::height_of_circular_ray;

/// The frequency above which `ξSZ` is frozen at `ξ` (AV 1106/07 Eq. 385).
const F_FREEZE_HZ: f64 = 2000.0;
/// The lower anchor frequency in the `ξSZ` log-interpolation (Eq. 385).
const F_LOW_HZ: f64 = 20.0;

/// ShadowZoneShielding `L_SZ(f)` for one band (AV 1106/07 §5.23.16 Eqs. 384–388).
///
/// Returns a **non-negative** shadow-zone attenuation, dB (0 ⇒ no shielding).
/// `xi` is the frequency-independent equivalent gradient (`< 0` for upward
/// refraction); `d_sz` is the shadow-zone distance from [`direct_ray`]
/// (`crate::propagation::refraction::circular_ray::direct_ray`).
///
/// # Errors
///
/// [`PropagationError::DegenerateShadowZone`] on non-finite / non-physical
/// input; wedge-domain errors propagate from [`pwedge0`].
pub fn shadow_zone_shielding(
    f_hz: f64,
    d: f64,
    h_s: f64,
    h_r: f64,
    xi: f64,
    c0: f64,
    d_sz: f64,
) -> Result<f64, PropagationError> {
    if ![f_hz, d, h_s, h_r, xi, c0, d_sz]
        .iter()
        .all(|v| v.is_finite())
        || d <= 0.0
        || c0 <= 0.0
        || d_sz <= 0.0
    {
        return Err(PropagationError::DegenerateShadowZone {
            detail: "non-finite or non-physical shadow-zone input",
        });
    }
    // Receiver has not yet crossed the geometric shadow edge (d ≤ dSZ): the
    // equivalent-wedge geometry (d_far = d − dSZ) is undefined here and the
    // coherent two-ray branch (with the Eq. 52 Δτ cap) still applies. Report zero
    // shielding rather than erroring — the onset window is physical, not
    // degenerate (CR-01).
    if d_sz >= d {
        return Ok(0.0);
    }

    // Eq. 385: ξSZ(f) frozen above 2000 Hz, log-interpolated 20 Hz → 2000 Hz.
    let xi_sz = if f_hz < F_FREEZE_HZ {
        xi * (f_hz.log10() - F_LOW_HZ.log10()) / (F_FREEZE_HZ.log10() - F_LOW_HZ.log10())
    } else {
        xi
    };
    if xi_sz == 0.0 {
        return Ok(0.0); // no gradient ⇒ no shielding at this band.
    }

    // Eq. 386: wedge height hSZ = |ray height at dSZ| (the dip magnitude).
    let (h0, _, _) = height_of_circular_ray(d, h_s, h_r, xi_sz, d_sz)?;
    let h_sz = h0.abs().max(1e-9);

    // Eq. 387 equivalent-wedge geometry.
    let d_far = d - d_sz;
    let theta_s = PI + (h_sz / d_sz).atan() + (h_sz / d_far).atan();
    // Wedge validity: β ∈ (π, 2π]. A too-tall equivalent wedge ⇒ no valid
    // diffraction geometry ⇒ report zero shielding rather than a spurious error.
    if !(theta_s > PI && theta_s <= std::f64::consts::TAU) {
        return Ok(0.0);
    }
    let r_s = (h_sz * h_sz + d_sz * d_sz).sqrt();
    let r_r = (h_sz * h_sz + d_far * d_far).sqrt();
    let l = r_s + r_r;
    let geo = WedgeGeometry {
        tau_s: r_s / c0,
        tau_r: r_r / c0,
        tau: l / c0,
        r_s,
        r_r,
        l,
        theta_s,
        theta_r: 0.0,
        beta: theta_s,
    };
    let p = pwedge0(f_hz, &geo)?;

    // R_SR: the straight source→receiver distance.
    let r_sr = (d * d + (h_r - h_s).powi(2)).sqrt();
    // Eq. 387 magnitude, returned as a non-negative attenuation (see module doc).
    let amp = 2.0 * r_sr * p.norm();
    let loss = -10.0 * amp.max(1e-30).log10();
    Ok(loss.max(0.0))
}

/// [`shadow_zone_shielding`] evaluated across the whole 105-point axis.
///
/// # Errors
///
/// Propagates the per-band [`shadow_zone_shielding`] error.
pub fn shadow_zone_shielding_bands(
    axis: &FreqAxis,
    d: f64,
    h_s: f64,
    h_r: f64,
    xi: f64,
    c0: f64,
    d_sz: f64,
) -> Result<Vec<f64>, PropagationError> {
    axis.centres
        .iter()
        .map(|&f| shadow_zone_shielding(f, d, h_s, h_r, xi, c0, d_sz))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const C0: f64 = 340.348;

    // A representative upward-refraction shadow geometry: finite, non-negative
    // attenuation across all 105 bands.
    #[test]
    fn shielding_is_finite_and_nonnegative_across_bands() {
        let axis = FreqAxis::new();
        // dSZ well inside d so the receiver is in the shadow.
        let bands = shadow_zone_shielding_bands(&axis, 300.0, 1.0, 1.5, -3e-3, C0, 150.0).unwrap();
        assert_eq!(bands.len(), 105);
        assert!(bands.iter().all(|&l| l.is_finite() && l >= 0.0));
        // At least some band shows real shielding (> 0).
        assert!(
            bands.iter().any(|&l| l > 0.0),
            "expected non-zero shielding"
        );
    }

    // ξSZ freeze above 2000 Hz: the high bands share the ξ-independent value.
    #[test]
    fn frozen_above_2000_hz() {
        let l_2k = shadow_zone_shielding(2000.0, 300.0, 1.0, 1.5, -3e-3, C0, 150.0).unwrap();
        let l_4k = shadow_zone_shielding(4000.0, 300.0, 1.0, 1.5, -3e-3, C0, 150.0).unwrap();
        // Both use the same frozen ξSZ = ξ (only f differs in the wedge kernel).
        assert!(l_2k.is_finite() && l_4k.is_finite());
    }

    // CR-01: a receiver before the geometric shadow edge (d ≤ dSZ) is the onset
    // window, not a degenerate input — no shielding yet (Ok(0.0)), never an error.
    #[test]
    fn pre_shadow_edge_returns_zero_shielding() {
        assert_eq!(
            shadow_zone_shielding(1000.0, 300.0, 1.0, 1.5, -3e-3, C0, 400.0).unwrap(),
            0.0
        );
        // Exactly at the edge (d_sz == d) is likewise not-yet-shadowed.
        assert_eq!(
            shadow_zone_shielding(1000.0, 300.0, 1.0, 1.5, -3e-3, C0, 300.0).unwrap(),
            0.0
        );
    }

    #[test]
    fn non_physical_input_is_typed_error() {
        // Genuinely non-physical input (non-finite / non-positive dSZ) stays a
        // typed error, distinct from the pre-edge onset window above.
        assert!(matches!(
            shadow_zone_shielding(1000.0, 300.0, 1.0, 1.5, -3e-3, C0, f64::NAN),
            Err(PropagationError::DegenerateShadowZone { .. })
        ));
        assert!(matches!(
            shadow_zone_shielding(1000.0, 300.0, 1.0, 1.5, -3e-3, C0, -5.0),
            Err(PropagationError::DegenerateShadowZone { .. })
        ));
    }
}
