//! Sub-model 7 — turbulence scattering behind screens (AV 1106/07 §5.16,
//! Eqs. 271–274).
//!
//! Scattered sound reaches the shadow zone behind a screen via atmospheric
//! turbulence (wind velocity and temperature fluctuations). This **floors** the
//! screen attenuation: it only ever ADDS energy to the shadow (raises ΔL₇), never
//! increases the screen's insertion loss. It is a real, additive-energy term
//! (returns `f64`, feeds the `p_incoh` / level side of Eq. 332) — structurally it
//! can never touch the coherent channel's phase (user-locked contract).
//!
//! # The deliberate ×10 (Pitfall 7)
//!
//! `Cve² = 10·Cv²`, `CTe² = 10·CT²` (Eq. 271) — the original model underestimated
//! scattering, so the effective turbulence strengths are boosted ×10. **This is
//! deliberate; do not "fix" it back to `Cv²`/`CT²`.**
//!
//! # Model (Eqs. 272–274)
//!
//! `L_ws0` / `L_ts0` are read from the 2-D Tables 6/7 (transcribed below from
//! AV 1106/07 pp. 117–118 page images) by bilinear interpolation in
//! `(40·R₂/R₁, 40·h_e/R₁)`, edge-clamped. A reciprocity correction takes the max
//! of both source/receiver orientations. `ΔL_ws`/`ΔL_ts` add the ground-effect
//! correction `C_SR`, a frequency scaling `(10/3)·lg(f/2000)`, the effective
//! strength `10·lg(C{v,T}e²)`, and a low-frequency roll-off around
//! `f₀ = Coft(t)/(2·sin(θ/2))`. Finally `ΔL₇ = 10·lg(10^{ΔL_ws/10}+10^{ΔL_ts/10})`
//! (Eq. 274, incoherent sum). If the edge is below the source–receiver line
//! (`h_e ≤ 0`) there is no contribution.

use std::f64::consts::PI;

use crate::propagation::PropagationError;
use crate::propagation::sound_speed_ms;

/// A dB value standing in for "no scattered energy" (`10^{-300/10} ≈ 0`). Kept
/// finite so callers never propagate `-inf` through the Eq. 332 combination.
const NO_SCATTER_DB: f64 = -300.0;

/// The `40·R₂/R₁` column axis of Tables 6/7 (AV 1106/07 pp. 117–118).
const COL_AXIS: [f64; 10] = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
/// The `40·h_e/R₁` row axis of Tables 6/7.
const ROW_AXIS: [f64; 8] = [5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0];

/// Table 6 — normalized scattered level `L_ws0` from **wind** turbulence
/// (AV 1106/07 p. 117, transcribed from the page image). Rows = `40·h_e/R₁`,
/// columns = `40·R₂/R₁`.
const TABLE6: [[f64; 10]; 8] = [
    [
        -41.6, -33.9, -30.2, -27.9, -26.2, -24.9, -23.9, -23.1, -22.4, -21.8,
    ],
    [
        -49.9, -44.0, -39.5, -36.7, -34.7, -33.1, -31.8, -30.9, -30.1, -29.3,
    ],
    [
        -52.1, -48.9, -45.8, -42.9, -40.7, -39.1, -37.7, -36.6, -35.5, -34.7,
    ],
    [
        -53.8, -51.0, -48.8, -46.8, -45.0, -43.5, -42.1, -40.9, -39.9, -39.1,
    ],
    [
        -55.4, -52.4, -50.4, -48.8, -47.5, -46.2, -45.1, -44.0, -43.1, -42.2,
    ],
    [
        -57.0, -53.8, -51.5, -50.6, -48.9, -47.8, -46.8, -45.9, -45.2, -44.4,
    ],
    [
        -58.6, -55.1, -52.7, -51.1, -49.8, -48.8, -47.9, -47.1, -46.4, -45.7,
    ],
    [
        -59.9, -56.5, -53.9, -52.1, -50.7, -49.6, -48.7, -48.0, -47.3, -46.6,
    ],
];

/// Table 7 — normalized scattered level `L_ts0` from **temperature** turbulence
/// (AV 1106/07 p. 118, transcribed from the page image). Same axes as Table 6.
const TABLE7: [[f64; 10]; 8] = [
    [
        -44.0, -39.1, -36.0, -34.0, -32.5, -31.3, -30.4, -29.6, -28.9, -28.3,
    ],
    [
        -47.4, -44.7, -42.4, -40.5, -39.1, -37.9, -36.9, -36.0, -35.3, -34.7,
    ],
    [
        -48.9, -46.7, -45.1, -43.6, -42.4, -41.4, -40.5, -39.7, -39.0, -38.4,
    ],
    [
        -50.2, -48.0, -46.4, -45.2, -44.1, -43.2, -42.4, -41.7, -41.1, -40.5,
    ],
    [
        -51.4, -49.0, -47.4, -46.2, -45.2, -44.3, -43.6, -42.9, -42.3, -41.8,
    ],
    [
        -52.5, -50.0, -48.3, -47.0, -46.0, -45.1, -44.4, -43.7, -43.2, -42.6,
    ],
    [
        -53.6, -51.0, -49.2, -47.8, -46.7, -45.8, -45.0, -44.4, -43.8, -43.3,
    ],
    [
        -54.6, -52.0, -50.0, -48.5, -47.4, -46.4, -45.6, -44.9, -44.3, -43.8,
    ],
];

/// Geometry for the turbulence-scattering shadow (AV 1106/07 Fig. 26).
#[derive(Debug, Clone, Copy)]
pub struct ScreenScatterGeometry {
    /// `R₁` — source → edge-foot distance along the S–R line (m).
    pub r1: f64,
    /// `R₂` — edge-foot → receiver distance along the S–R line (m).
    pub r2: f64,
    /// `h_e` — edge height above the S–R line (m); `≤ 0` ⇒ no contribution.
    pub h_e: f64,
    /// Mean air temperature `t_mean` (°C) for `Coft(t_mean)` in `f₀`.
    pub t_air_c: f64,
}

/// Heaviside step `H(x)` (AV 1106/07 Eq. 354): `1` for `x > 0`, else `0`.
#[inline]
fn heaviside(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else { 0.0 }
}

/// Bilinear interpolation into a Table 6/7 grid at `(col_val = 40·R₂/R₁,
/// row_val = 40·h_e/R₁)`, clamped to the nearest edge value when a parameter is
/// outside the tabulated range (AV 1106/07 §5.16 "nearest values are used").
fn table_interp(table: &[[f64; 10]; 8], col_val: f64, row_val: f64) -> f64 {
    let frac = |v: f64, axis: &[f64]| -> (usize, usize, f64) {
        if v <= axis[0] {
            return (0, 0, 0.0);
        }
        let n = axis.len();
        if v >= axis[n - 1] {
            return (n - 1, n - 1, 0.0);
        }
        let mut i = 0;
        while i + 1 < n && axis[i + 1] < v {
            i += 1;
        }
        let t = (v - axis[i]) / (axis[i + 1] - axis[i]);
        (i, i + 1, t)
    };
    let (r0, r1, tr) = frac(row_val, &ROW_AXIS);
    let (c0, c1, tc) = frac(col_val, &COL_AXIS);
    let top = table[r0][c0] * (1.0 - tc) + table[r0][c1] * tc;
    let bot = table[r1][c0] * (1.0 - tc) + table[r1][c1] * tc;
    top * (1.0 - tr) + bot * tr
}

/// One turbulence channel (wind or temperature): `ΔL` per Eqs. 272/273.
#[allow(clippy::too_many_arguments)]
fn scatter_channel(
    table: &[[f64; 10]; 8],
    f_hz: f64,
    r1: f64,
    r2: f64,
    h_e: f64,
    ce2: f64,
    c_sr: f64,
    f0: f64,
) -> f64 {
    if ce2 <= 0.0 {
        return NO_SCATTER_DB;
    }
    // Reciprocity: max over both source/receiver orientations (Eq. 272 top).
    let l_a = table_interp(table, 40.0 * r2 / r1, 40.0 * h_e / r1) + 10.0 * (r1 / 40.0).log10();
    let l_b = table_interp(table, 40.0 * r1 / r2, 40.0 * h_e / r2) + 10.0 * (r2 / 40.0).log10();
    let l0 = l_a.max(l_b);

    l0 + 10.0 * c_sr.log10()
        + (10.0 / 3.0) * (f_hz / 2000.0).log10()
        + 10.0 * ce2.log10()
        + 15.0 * heaviside(f0 - f_hz) * (f_hz / f0).log10()
        + 15.0 * heaviside(0.5 * f0 - f_hz) * (f_hz / (0.5 * f0)).log10()
}

/// Sub-model 7 scattered-energy contribution `ΔL₇` (dB) behind a screen
/// (AV 1106/07 Eqs. 271–274).
///
/// `cv2`/`ct2` are the **base** structure parameters `Cv²`/`CT²`; the ×10
/// effective boost (Eq. 271) is applied internally. `c_sr` is the ground-effect
/// correction (the ground part of Eq. 188 floored at 1 — from
/// [`super::screen::submodel4_c_sr`]). Returns a large negative dB (finite,
/// effectively zero energy) when there is no scattering (`h_e ≤ 0` or
/// `Cv² = CT² = 0`).
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] if `R₁`/`R₂` are not positive and
/// finite.
pub fn submodel7_delta_l(
    f_hz: f64,
    geo: &ScreenScatterGeometry,
    cv2: f64,
    ct2: f64,
    c_sr: f64,
) -> Result<f64, PropagationError> {
    if !(geo.r1.is_finite() && geo.r1 > 0.0 && geo.r2.is_finite() && geo.r2 > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "Sub-model 7 requires positive finite R₁, R₂",
        });
    }
    // Edge below the S–R line ⇒ no scattered contribution (finite sentinel).
    if !(geo.h_e.is_finite() && geo.h_e > 0.0) {
        return Ok(NO_SCATTER_DB);
    }
    if cv2 == 0.0 && ct2 == 0.0 {
        return Ok(NO_SCATTER_DB);
    }

    // Effective strengths (Eq. 271) — the deliberate ×10 boost (Pitfall 7).
    let cve2 = 10.0 * cv2;
    let cte2 = 10.0 * ct2;
    let c_sr = c_sr.max(1.0); // floored at 1 (Eq. 272 C_SR definition)

    // f₀ = Coft(t)/(2·sin(θ/2)), θ = π − arctan(R₁/h_e) − arctan(R₂/h_e).
    let theta = PI - (geo.r1 / geo.h_e).atan() - (geo.r2 / geo.h_e).atan();
    let c0 = sound_speed_ms(geo.t_air_c);
    let sin_half = (theta / 2.0).sin();
    let f0 = if sin_half.abs() > 1e-12 {
        c0 / (2.0 * sin_half)
    } else {
        f_hz // degenerate grazing: no low-frequency roll-off
    };

    let dl_ws = scatter_channel(&TABLE6, f_hz, geo.r1, geo.r2, geo.h_e, cve2, c_sr, f0);
    let dl_ts = scatter_channel(&TABLE7, f_hz, geo.r1, geo.r2, geo.h_e, cte2, c_sr, f0);

    // Eq. 274 — incoherent sum of the two channels.
    Ok(10.0 * (10f64.powf(dl_ws / 10.0) + 10f64.powf(dl_ts / 10.0)).log10())
}

/// Combine a screen sub-model level `ΔL_scr` (dB, e.g. ΔL₄) with the Sub-model 7
/// scattered energy `ΔL₇` (dB) — the incoherent addition of Eq. 332
/// (`10·lg(10^{ΔL_scr/10} + 10^{ΔL₇/10})`). Scattering only ever RAISES the
/// level (floors the attenuation), never lowers it.
#[must_use]
pub fn combine_scatter(delta_l_scr_db: f64, delta_l7_db: f64) -> f64 {
    10.0 * (10f64.powf(delta_l_scr_db / 10.0) + 10f64.powf(delta_l7_db / 10.0)).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo() -> ScreenScatterGeometry {
        // Case-71-like: S–edge 15 m, edge–R 135 m, edge 6 m above the S–R line.
        ScreenScatterGeometry {
            r1: 15.0,
            r2: 135.0,
            h_e: 6.0,
            t_air_c: 15.0,
        }
    }

    // Test 1: table interpolation reproduces exact nodes and is continuous.
    #[test]
    fn table_interp_reproduces_nodes_and_is_continuous() {
        // Exact node: 40R2/R1 = 40, 40he/R1 = 10 ⇒ Table 6 row 1 col 3 = -36.7.
        assert!((table_interp(&TABLE6, 40.0, 10.0) - (-36.7)).abs() < 1e-9);
        assert!((table_interp(&TABLE7, 100.0, 40.0) - (-43.8)).abs() < 1e-9);
        // Continuity: 1 % steps at an interior point change < 0.5 dB.
        let a = table_interp(&TABLE6, 55.0, 22.0);
        let b = table_interp(&TABLE6, 55.55, 22.22);
        assert!((a - b).abs() < 0.5);
        // Edge clamp: below/above the axis uses the nearest value.
        assert!((table_interp(&TABLE6, 1.0, 1.0) - TABLE6[0][0]).abs() < 1e-9);
        assert!((table_interp(&TABLE6, 999.0, 999.0) - TABLE6[7][9]).abs() < 1e-9);
    }

    // Test 2: scattering only adds energy (ΔL₄+ΔL₇ ≥ ΔL₄); zero strengths ⇒ none.
    #[test]
    fn scattering_only_adds_energy() {
        let g = geo();
        for &f in &[250.0, 500.0, 1000.0, 2000.0, 4000.0] {
            let dl7 = submodel7_delta_l(f, &g, 0.12, 0.008, 1.0).unwrap();
            let dl4 = -25.0; // a representative deep-shadow screen level
            let combined = combine_scatter(dl4, dl7);
            assert!(
                combined >= dl4 - 1e-9,
                "scattering must not reduce the level (f={f})"
            );
            // Zero turbulence ⇒ effectively no contribution.
            let dl7_zero = submodel7_delta_l(f, &g, 0.0, 0.0, 1.0).unwrap();
            assert!(combine_scatter(dl4, dl7_zero) - dl4 < 1e-6);
        }
    }

    // Test 3: reciprocity — swapping source/receiver yields the same ΔL₇.
    #[test]
    fn reciprocity_swap_is_invariant() {
        let g = geo();
        let swapped = ScreenScatterGeometry {
            r1: g.r2,
            r2: g.r1,
            ..g
        };
        for &f in &[500.0, 1000.0, 2000.0] {
            let a = submodel7_delta_l(f, &g, 0.12, 0.008, 1.0).unwrap();
            let b = submodel7_delta_l(f, &swapped, 0.12, 0.008, 1.0).unwrap();
            assert!(
                (a - b).abs() < 1e-9,
                "ΔL₇ must be reciprocal (f={f}): {a} vs {b}"
            );
        }
    }

    // Test 4: edge below the S–R line ⇒ zero contribution, finite.
    #[test]
    fn edge_below_line_is_zero_and_finite() {
        let g = ScreenScatterGeometry { h_e: -1.0, ..geo() };
        let dl7 = submodel7_delta_l(1000.0, &g, 0.12, 0.008, 1.0).unwrap();
        assert!(dl7.is_finite());
        assert!(combine_scatter(-25.0, dl7) - (-25.0) < 1e-6);
    }

    // The ×10 boost is present and cited (Pitfall 7 guard).
    #[test]
    fn effective_strength_is_ten_times_base() {
        let g = geo();
        // Doubling the effective strength (via cv2) raises ΔL_ws by ~10·lg(2)=3 dB.
        let a = submodel7_delta_l(1000.0, &g, 0.12, 0.0, 1.0).unwrap();
        let b = submodel7_delta_l(1000.0, &g, 0.24, 0.0, 1.0).unwrap();
        assert!(
            (b - a - 3.0103).abs() < 0.05,
            "10·lg(2) step expected: {a} → {b}"
        );
    }
}
