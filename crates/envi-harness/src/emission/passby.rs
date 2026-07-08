//! Straight-road pass-by integration: the 179-point 1° immission-angle
//! discretization, the oblique-profile stretch, and the LE / LAeq,24h / LAmax
//! energy integrals (04-RESEARCH "Pass-by integration" + Pattern 7).
//!
//! # The tensor is the natural home (04-01 is load-bearing)
//!
//! The sub-source axis is `(source point × height)`; the free-field pass-by
//! level `LE(f)` is the 04-01 **incoherent** readout
//! (`readout_incoherent`) with per-sub-source energy weights
//! `w_s(f) = (t_i/t₀)·10^{L_W(f)/10}` and the divergence-only transfer in the
//! `H_coh` channel. This routes the integration THROUGH the frozen tensor
//! contract rather than around it.
//!
//! # Discretization (EP 1335 Ch. 2)
//!
//! 179 source points at 1° steps, `θ ∈ [−89°, +89°]` (never ±90° — `tan θ`
//! blows up, T-04-02-04). Segment length `ℓ_i = d⊥·[tan(θ_i+½°) −
//! tan(θ_i−½°)]`, time weight `t_i = ℓ_i / v`. Energy integral
//! `LE(f) = 10·lg Σ_i (t_i/t₀)·10^{L_i(f)/10}` — the accumulate-energy-then-log
//! shape of `weather::route1::l_den`, reused here. A-weighting is
//! `compare::a_weighting_db` at the EXACT `FREQ_AXIS` centres (never nominal
//! Hz). `LAeq,24h = LAE + 10·lg N − 10·lg 86400`.
//!
//! Non-finite inputs are rejected with a typed [`PassbyError`] — never a panic.

use envi_engine::directivity::DirectivityBalloon;
use envi_engine::freq::{FreqAxis, N_BANDS};
use envi_engine::scene::{GroundSegment, SceneError, TerrainProfile};
use envi_engine::tensor::{TensorPair, readout_incoherent};
use num_complex::Complex;
use thiserror::Error;

use crate::compare::a_weighting_db;

/// Immission-angle half-span: `θ ∈ [−89°, +89°]` (never ±90°, T-04-02-04).
pub const PASSBY_HALF_SPAN_DEG: f64 = 89.0;
/// Immission-angle step, degrees.
pub const PASSBY_STEP_DEG: f64 = 1.0;
/// Number of 1° source points spanning `[−89°, +89°]` inclusive.
pub const N_PASSBY_POINTS: usize = 179;
/// Reference exposure time `t₀`, seconds (LE normalization).
pub const T0_S: f64 = 1.0;
/// Seconds in 24 h (the LAeq,24h denominator).
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// Typed pass-by integration error — never a panic on data (T-04-02-04).
#[derive(Debug, Error, PartialEq)]
pub enum PassbyError {
    /// A geometric or physical input was NaN, infinite, or out of range.
    #[error("pass-by {what} is not finite or out of range: {value}")]
    NonFinite {
        /// Which quantity.
        what: &'static str,
        /// The offending value.
        value: f64,
    },
    /// Parallel input slices disagreed in length.
    #[error("pass-by length mismatch: {a_len} {a_name} vs {b_len} {b_name}")]
    LengthMismatch {
        /// First slice name.
        a_name: &'static str,
        /// First slice length.
        a_len: usize,
        /// Second slice name.
        b_name: &'static str,
        /// Second slice length.
        b_len: usize,
    },
    /// An engine sink/readout error surfaced through the tensor path.
    #[error("pass-by tensor readout error: {0}")]
    Readout(String),
    /// A stretched terrain profile was invalid.
    #[error("pass-by profile stretch error: {0}")]
    Profile(#[from] SceneError),
}

/// One pass-by source point: its immission angle, the road segment it subtends,
/// its along-road offset, and its time weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassbyPoint {
    /// Immission angle `θ_i`, degrees (the perpendicular is 0°).
    pub theta_deg: f64,
    /// Along-road offset `y_i = d⊥·tan θ_i`, meters.
    pub y_offset_m: f64,
    /// Subtended road-segment length `ℓ_i`, meters.
    pub segment_len_m: f64,
    /// Time weight `t_i = ℓ_i / v`, seconds.
    pub time_weight_s: f64,
}

/// Convert a speed in km/h to m/s (guarded positive).
#[must_use]
pub fn speed_ms(speed_kmh: f64) -> f64 {
    (speed_kmh / 3.6).max(0.0)
}

/// Build the 179-point 1° discretization for a perpendicular distance `d⊥` and
/// travel speed `v` (m/s).
///
/// `ℓ_i = d⊥·[tan(θ_i+½°) − tan(θ_i−½°)]`, `t_i = ℓ_i/v`, `y_i = d⊥·tan θ_i`.
/// A centred receiver gives `±θ` segments equal weight (a symmetry test
/// asserts it).
///
/// # Errors
///
/// [`PassbyError::NonFinite`] if `d_perp_m` or `speed_ms` is non-finite or
/// non-positive.
pub fn passby_points(d_perp_m: f64, speed_ms: f64) -> Result<Vec<PassbyPoint>, PassbyError> {
    if !(d_perp_m.is_finite() && d_perp_m > 0.0) {
        return Err(PassbyError::NonFinite {
            what: "perpendicular distance d⊥",
            value: d_perp_m,
        });
    }
    if !(speed_ms.is_finite() && speed_ms > 0.0) {
        return Err(PassbyError::NonFinite {
            what: "speed",
            value: speed_ms,
        });
    }
    let half = PASSBY_STEP_DEG / 2.0;
    let mut points = Vec::with_capacity(N_PASSBY_POINTS);
    for k in 0..N_PASSBY_POINTS {
        let theta = -PASSBY_HALF_SPAN_DEG + PASSBY_STEP_DEG * k as f64;
        let hi = (theta + half).to_radians().tan();
        let lo = (theta - half).to_radians().tan();
        let segment_len_m = d_perp_m * (hi - lo);
        let time_weight_s = segment_len_m / speed_ms;
        points.push(PassbyPoint {
            theta_deg: theta,
            y_offset_m: d_perp_m * theta.to_radians().tan(),
            segment_len_m,
            time_weight_s,
        });
    }
    Ok(points)
}

/// Stretch a perpendicular cut-plane profile to an oblique immission angle `θ`
/// by scaling every profile X by `1/cos θ` about `origin_x` (04-RESEARCH
/// Pattern 7 — exact for laterally-invariant terrain, [ASSUMED A5]).
///
/// Heights, impedances and roughness are unchanged; only horizontal distances
/// stretch. `θ` is capped away from ±90° by the caller (the 179-point grid caps
/// at ±89°).
///
/// # Errors
///
/// [`PassbyError::NonFinite`] if `theta` is non-finite or `|θ| ≥ 90°`, or
/// [`PassbyError::Profile`] if the stretched profile fails validation.
pub fn stretch_profile_x(
    profile: &TerrainProfile,
    theta_deg: f64,
    origin_x: f64,
) -> Result<TerrainProfile, PassbyError> {
    if !theta_deg.is_finite() || theta_deg.abs() >= 90.0 {
        return Err(PassbyError::NonFinite {
            what: "oblique angle θ",
            value: theta_deg,
        });
    }
    let inv_cos = 1.0 / theta_deg.to_radians().cos();
    let points: Vec<[f64; 2]> = profile
        .points()
        .iter()
        .map(|p| [origin_x + (p[0] - origin_x) * inv_cos, p[1]])
        .collect();
    let segments: Vec<GroundSegment> = profile.segments().to_vec();
    Ok(TerrainProfile::new(points, segments)?)
}

/// Energy-integrate per-point band levels into the pass-by spectrum
/// `LE(f) = 10·lg Σ_i (t_i/t₀)·10^{L_i(f)/10}`.
///
/// `per_point[i]` is the free-field band spectrum from source point `i`;
/// `time_weights[i] = t_i`. This is the explicit LE law (a hand-computed
/// two-segment case is exact); [`free_field_passby_le`] routes the same
/// integral through the tensor readout.
///
/// # Errors
///
/// [`PassbyError::LengthMismatch`] on unequal lengths;
/// [`PassbyError::NonFinite`] on a non-finite level or weight.
pub fn passby_le(
    per_point: &[[f64; N_BANDS]],
    time_weights: &[f64],
) -> Result<[f64; N_BANDS], PassbyError> {
    if per_point.len() != time_weights.len() {
        return Err(PassbyError::LengthMismatch {
            a_name: "per-point levels",
            a_len: per_point.len(),
            b_name: "time weights",
            b_len: time_weights.len(),
        });
    }
    let mut energy = [0.0_f64; N_BANDS];
    for (levels, &t) in per_point.iter().zip(time_weights) {
        if !(t.is_finite() && t >= 0.0) {
            return Err(PassbyError::NonFinite {
                what: "time weight",
                value: t,
            });
        }
        for (e, &l) in energy.iter_mut().zip(levels.iter()) {
            if !l.is_finite() {
                return Err(PassbyError::NonFinite {
                    what: "per-point band level",
                    value: l,
                });
            }
            *e += (t / T0_S) * 10f64.powf(l / 10.0);
        }
    }
    Ok(std::array::from_fn(|i| {
        10.0 * energy[i].max(f64::MIN_POSITIVE).log10()
    }))
}

/// The free-field pass-by spectrum `LE(f)` via the 04-01 tensor readout.
///
/// Each sub-source is `(source point × height)`; its `H_coh` is the
/// **divergence-only** transfer `1/(√(4π)·R)` (NO ground / screen / air
/// absorption — the `LE − dL` anchor's free field) times the balloon's real
/// directivity factor `10^{ΔL/20}`; `P_incoh_abs = 0`. The incoherent readout
/// weight is `w_s(f) = (t_s/t₀)·10^{L_W_s(f)/10}`, and `LE(f) = 10·lg e[0,f]`.
///
/// # Errors
///
/// [`PassbyError::LengthMismatch`] if the parallel sub-source inputs disagree,
/// [`PassbyError::NonFinite`] on a degenerate range, or
/// [`PassbyError::Readout`] if the engine readout rejects the tensor.
pub fn free_field_passby_le(
    sub_positions: &[[f64; 3]],
    balloons: &[&DirectivityBalloon],
    l_w_105: &[[f64; N_BANDS]],
    time_weights: &[f64],
    receiver: [f64; 3],
    _axis: &FreqAxis,
) -> Result<[f64; N_BANDS], PassbyError> {
    let n = sub_positions.len();
    for (name, len) in [
        ("balloons", balloons.len()),
        ("L_W", l_w_105.len()),
        ("time weights", time_weights.len()),
    ] {
        if len != n {
            return Err(PassbyError::LengthMismatch {
                a_name: "sub positions",
                a_len: n,
                b_name: name,
                b_len: len,
            });
        }
    }

    let mut pair = TensorPair::zeros((n, 1, N_BANDS));
    let mut weights: Vec<Vec<f64>> = Vec::with_capacity(n);
    for (s, &pos) in sub_positions.iter().enumerate() {
        let dir = [
            receiver[0] - pos[0],
            receiver[1] - pos[1],
            receiver[2] - pos[2],
        ];
        let r = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if !(r.is_finite() && r > 1e-9) {
            return Err(PassbyError::NonFinite {
                what: "sub-source→receiver range",
                value: r,
            });
        }
        // Divergence-only magnitude: |H|² = 1/(4π R²) ⇒ 10·lg|H|² = −10·lg(4πR²).
        let base_mag = 1.0 / ((4.0 * std::f64::consts::PI).sqrt() * r);
        let dl = balloons[s].eval(dir);
        let t = time_weights[s];
        if !(t.is_finite() && t >= 0.0) {
            return Err(PassbyError::NonFinite {
                what: "time weight",
                value: t,
            });
        }
        let mut w = vec![0.0; N_BANDS];
        for f in 0..N_BANDS {
            // Directivity is a real magnitude factor on H_coh (phase untouched).
            let mag = base_mag * 10f64.powf(dl[f] / 20.0);
            pair.h_coh[[s, 0, f]] = Complex::new(mag, 0.0);
            w[f] = (t / T0_S) * 10f64.powf(l_w_105[s][f] / 10.0);
        }
        weights.push(w);
    }

    let e = readout_incoherent(pair.h_coh.view(), pair.p_incoh_abs.view(), &weights)
        .map_err(|err| PassbyError::Readout(err.to_string()))?;
    Ok(std::array::from_fn(|i| {
        10.0 * e[[0, i]].max(f64::MIN_POSITIVE).log10()
    }))
}

/// `LAeq,24h = LAE + 10·lg N − 10·lg 86400` for `n_vehicles` pass-by events.
///
/// # Errors
///
/// [`PassbyError::NonFinite`] if `lae_db` is non-finite or `n_vehicles == 0`.
pub fn laeq_24h_from_lae(lae_db: f64, n_vehicles: u64) -> Result<f64, PassbyError> {
    if !lae_db.is_finite() {
        return Err(PassbyError::NonFinite {
            what: "LAE",
            value: lae_db,
        });
    }
    if n_vehicles == 0 {
        return Err(PassbyError::NonFinite {
            what: "vehicle count",
            value: 0.0,
        });
    }
    Ok(lae_db + 10.0 * (n_vehicles as f64).log10() - 10.0 * SECONDS_PER_DAY.log10())
}

/// A-weighted overall level from a 27-band 1/3-octave spectrum, using the exact
/// `FreqAxis` third-octave centres (never nominal Hz).
///
/// `L_A = 10·lg Σ_k 10^{(L_k + A(f_k))/10}`.
///
/// # Errors
///
/// [`PassbyError::NonFinite`] on a non-finite band level.
pub fn a_weighted_overall_third_octave(
    third_octave_db: &[f64],
    axis: &FreqAxis,
) -> Result<f64, PassbyError> {
    let mut energy = 0.0_f64;
    for (k, &l) in third_octave_db.iter().enumerate() {
        if !l.is_finite() {
            return Err(PassbyError::NonFinite {
                what: "third-octave band level",
                value: l,
            });
        }
        let f = axis.third_octave_pick(k);
        energy += 10f64.powf((l + a_weighting_db(f)) / 10.0);
    }
    Ok(10.0 * energy.max(f64::MIN_POSITIVE).log10())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn discretization_is_179_points_symmetric_about_the_perpendicular() {
        let pts = passby_points(96.5, speed_ms(80.0)).unwrap();
        assert_eq!(pts.len(), N_PASSBY_POINTS);
        // θ spans −89 … +89 at 1° steps; the centre point is θ = 0.
        assert_relative_eq!(pts[0].theta_deg, -89.0, epsilon = 1e-12);
        assert_relative_eq!(pts[89].theta_deg, 0.0, epsilon = 1e-12);
        assert_relative_eq!(pts[178].theta_deg, 89.0, epsilon = 1e-12);
        // ±θ segments carry equal weight for a centred receiver (tan is odd, so
        // the segment width is symmetric).
        for k in 0..89 {
            assert_relative_eq!(
                pts[k].time_weight_s,
                pts[178 - k].time_weight_s,
                max_relative = 1e-12
            );
        }
        // All weights strictly positive and finite.
        assert!(
            pts.iter()
                .all(|p| p.time_weight_s > 0.0 && p.time_weight_s.is_finite())
        );
    }

    #[test]
    fn le_integral_reproduces_a_hand_computed_two_segment_case() {
        // Two source points, unit t₀. Band 0: levels 60 and 63 dB with weights
        // 1.0 and 2.0 s ⇒ LE = 10·lg(1·10^6 + 2·10^6.3).
        let mut a = [0.0_f64; N_BANDS];
        let mut b = [0.0_f64; N_BANDS];
        a[0] = 60.0;
        b[0] = 63.0;
        let le = passby_le(&[a, b], &[1.0, 2.0]).unwrap();
        let want = 10.0 * (1.0 * 10f64.powf(6.0) + 2.0 * 10f64.powf(6.3)).log10();
        assert_relative_eq!(le[0], want, max_relative = 1e-12);
    }

    #[test]
    fn free_field_le_matches_the_explicit_integral_via_the_tensor() {
        // The tensor readout path must equal the explicit passby_le law (04-01
        // is load-bearing, not parallel plumbing). Omni balloons, one band lit.
        let axis = FreqAxis::new();
        let subs = [[3.5, -10.0, 0.3], [3.5, 0.0, 0.3], [3.5, 10.0, 0.3]];
        let rcv = [100.0, 0.0, 1.5];
        let omni = DirectivityBalloon::omni();
        let balloons: Vec<&DirectivityBalloon> = vec![&omni; 3];
        let mut lw = [[0.0_f64; N_BANDS]; 3];
        for (s, row) in lw.iter_mut().enumerate() {
            row[0] = 80.0 + s as f64; // distinct so weights differ
        }
        let tw = [0.5, 1.0, 0.75];

        let via_tensor = free_field_passby_le(&subs, &balloons, &lw, &tw, rcv, &axis).unwrap();

        // Explicit reference: per-point divergence-only level then passby_le.
        let mut per_point = [[0.0_f64; N_BANDS]; 3];
        for (s, &pos) in subs.iter().enumerate() {
            let r =
                ((rcv[0] - pos[0]).powi(2) + (rcv[1] - pos[1]).powi(2) + (rcv[2] - pos[2]).powi(2))
                    .sqrt();
            let div_db = -10.0 * (4.0 * std::f64::consts::PI * r * r).log10();
            for f in 0..N_BANDS {
                per_point[s][f] = lw[s][f] + div_db;
            }
        }
        let explicit = passby_le(&per_point, &tw).unwrap();
        for f in 0..N_BANDS {
            assert_relative_eq!(via_tensor[f], explicit[f], max_relative = 1e-9);
        }
    }

    #[test]
    fn laeq_24h_is_lae_minus_9_37_for_10000_vehicles() {
        // N = 10 000 ⇒ LAeq,24h = LAE + 10·lg 10⁴ − 10·lg 86400
        //            = LAE + 40 − 49.3651 = LAE − 9.3651.
        let lae = 48.77;
        let laeq = laeq_24h_from_lae(lae, 10_000).unwrap();
        let want_offset = 10.0 * 10_000f64.log10() - 10.0 * SECONDS_PER_DAY.log10();
        assert_relative_eq!(laeq, lae + want_offset, epsilon = 1e-12);
        assert_relative_eq!(want_offset, -9.365_1, epsilon = 1e-3);
        assert!(matches!(
            laeq_24h_from_lae(f64::NAN, 10),
            Err(PassbyError::NonFinite { .. })
        ));
        assert!(matches!(
            laeq_24h_from_lae(50.0, 0),
            Err(PassbyError::NonFinite { .. })
        ));
    }

    #[test]
    fn oblique_stretch_scales_x_by_inverse_cos_leaving_heights() {
        let profile = TerrainProfile::new(
            vec![[3.25, 0.0], [5.0, 0.0], [100.0, 2.0]],
            vec![
                GroundSegment {
                    flow_resistivity: 20000.0,
                    roughness: 0.0,
                },
                GroundSegment {
                    flow_resistivity: 12.5,
                    roughness: 0.0,
                },
            ],
        )
        .unwrap();
        let stretched = stretch_profile_x(&profile, 60.0, 3.25).unwrap();
        let inv_cos = 1.0 / 60.0_f64.to_radians().cos(); // = 2.0
        // X distances from the origin double at 60°; heights unchanged.
        assert_relative_eq!(
            stretched.points()[1][0],
            3.25 + 1.75 * inv_cos,
            epsilon = 1e-9
        );
        assert_relative_eq!(stretched.points()[2][1], 2.0, epsilon = 1e-12);
        // θ → ±90° is rejected (tan/1/cos blow-up guard).
        assert!(matches!(
            stretch_profile_x(&profile, 90.0, 3.25),
            Err(PassbyError::NonFinite { .. })
        ));
    }

    #[test]
    fn passby_points_rejects_degenerate_geometry() {
        assert!(matches!(
            passby_points(0.0, 22.0),
            Err(PassbyError::NonFinite { .. })
        ));
        assert!(matches!(
            passby_points(96.5, 0.0),
            Err(PassbyError::NonFinite { .. })
        ));
    }
}
