//! Per-band spherical directivity balloons (SRC-02/04) and a hand-rolled 3×3
//! rotation (SRC-03), pure engine math.
//!
//! # Angular convention (read this before touching a direction)
//!
//! A [`DirectivityBalloon`] stores `ΔL(azimuth, polar, band)` in dB on a
//! fixed-length equal-angle spherical grid, in a **source-local frame**:
//!
//! - **polar** `p ∈ [0°, 180°]` is the angle from the local **+Z** axis
//!   (`p = 0` → straight up, `p = 90°` → the horizontal plane, `p = 180°` →
//!   straight down). Poles are stored as duplicated rows (one value per polar
//!   endpoint, repeated across azimuth).
//! - **azimuth** `a ∈ [0°, 360°]` is measured **counter-clockwise from +X** in
//!   the local XY plane (`a = atan2(y, x)`), with the `0°` and `360°` rows
//!   duplicated so bilinear interpolation needs no wrap-around branch.
//!
//! A unit direction `[x, y, z]` maps to `(a, p)` via `p = acos(z)`,
//! `a = atan2(y, x)`; [`DirectivityBalloon::eval`] bilinearly interpolates the
//! grid at that `(a, p)` and returns a per-band ΔL dB slice. The slice is what a
//! `SolveJob`'s `directivity_gain_db` carries — a **real magnitude** factor
//! `10^{ΔL/20}` on `H_coh` (phase untouched), never a `propagation/` operator.
//!
//! # Band axis is locked to the grid (never nominal Hz)
//!
//! The balloon's third axis is length [`N_BANDS`] on the shared `FREQ_AXIS`
//! 1/12-octave grid; there is no per-balloon frequency resampling in the engine
//! (the harness resamples imported data at construction). Index by band, never
//! by nominal frequency (`freq.rs` Pitfall 3).
//!
//! # Validating constructor, never a panic on data
//!
//! [`DirectivityBalloon::new`] validates finiteness, strictly-ascending angle
//! axes, the `[0, 360]` / `[0, 180]` spans, and the band-axis length, returning
//! a typed [`DirectivityError`] on any violation (modeled on
//! `TerrainProfile::new`, threat T-04-02-03). The 1/cosθ pass-by degeneracy at
//! `θ → ±90°` is a caller concern (the pass-by integrator caps at ±89°,
//! T-04-02-04); the balloon itself is bounded by its fixed resolution.

use ndarray::Array3;
use thiserror::Error;

use crate::freq::N_BANDS;

/// Errors from constructing or driving a [`DirectivityBalloon`].
///
/// Angle axes and ΔL values are caller-controlled (harness → engine, threat
/// T-04-02-03); every malformed grid yields one of these — the constructor
/// never panics on data.
#[derive(Debug, Error, PartialEq)]
pub enum DirectivityError {
    /// A grid axis had fewer than two nodes (cannot interpolate).
    #[error("directivity {axis} axis needs ≥ 2 nodes, got {got}")]
    TooFewNodes {
        /// Which axis was too short.
        axis: &'static str,
        /// The observed node count.
        got: usize,
    },
    /// An axis length disagreed with the grid dimension it labels.
    #[error("directivity {axis} axis has {axis_len} nodes but the grid has {grid_len}")]
    AxisGridMismatch {
        /// Which axis disagreed.
        axis: &'static str,
        /// The axis vector length.
        axis_len: usize,
        /// The grid dimension length.
        grid_len: usize,
    },
    /// The band axis was not [`N_BANDS`] long.
    #[error("directivity band axis has {got} bands, expected {expected}")]
    BandCountMismatch {
        /// Expected band count ([`N_BANDS`]).
        expected: usize,
        /// Observed band count.
        got: usize,
    },
    /// An angle axis was not strictly ascending.
    #[error(
        "directivity {axis} axis not strictly ascending at index {index} ({value} after {prev})"
    )]
    NonAscending {
        /// Which axis was out of order.
        axis: &'static str,
        /// Offending node index.
        index: usize,
        /// Previous node value.
        prev: f64,
        /// Offending node value.
        value: f64,
    },
    /// An angle axis did not cover its required span.
    #[error("directivity {axis} axis must span [{lo}, {hi}] degrees, got [{first}, {last}]")]
    BadSpan {
        /// Which axis was out of span.
        axis: &'static str,
        /// Required low endpoint.
        lo: f64,
        /// Required high endpoint.
        hi: f64,
        /// The observed first node.
        first: f64,
        /// The observed last node.
        last: f64,
    },
    /// A coordinate or ΔL value was NaN or infinite.
    #[error("non-finite value: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
    /// An equal-angle sampler step did not divide its span evenly.
    #[error("directivity {axis} step {step}° does not divide {span}° evenly")]
    UnevenStep {
        /// Which axis.
        axis: &'static str,
        /// The offending step.
        step: f64,
        /// The span it must divide.
        span: f64,
    },
}

/// A per-band spherical directivity pattern `ΔL(azimuth, polar, band)` in dB.
///
/// The grid is `[azimuth, polar, band]` with the band axis locked to
/// [`N_BANDS`]. Construct via [`DirectivityBalloon::new`] (validated) or
/// [`DirectivityBalloon::from_equirect_sampler`] (equal-angle, sampled from an
/// analytic pattern). See the module docs for the angular convention.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectivityBalloon {
    azimuths_deg: Vec<f64>,
    polars_deg: Vec<f64>,
    grid: Array3<f64>,
}

impl DirectivityBalloon {
    /// A balloon with `ΔL = 0` everywhere (an omnidirectional / no-op pattern),
    /// on a minimal `[0, 360] × [0, 180]` grid.
    #[must_use]
    pub fn omni() -> Self {
        // A 2×2 grid of zeros spanning both endpoints — bilinear-exact zero.
        Self {
            azimuths_deg: vec![0.0, 360.0],
            polars_deg: vec![0.0, 180.0],
            grid: Array3::zeros((2, 2, N_BANDS)),
        }
    }

    /// Validate and construct a balloon from explicit angle axes and a grid.
    ///
    /// `azimuths_deg` must be strictly ascending and span `[0, 360]`;
    /// `polars_deg` must be strictly ascending and span `[0, 180]`; the grid
    /// shape must be `(azimuths.len(), polars.len(), N_BANDS)`.
    ///
    /// # Errors
    ///
    /// A [`DirectivityError`] for any dimension mismatch, non-finite value,
    /// non-ascending or out-of-span angle axis, or wrong band-axis length —
    /// never a panic (threat T-04-02-03).
    pub fn new(
        azimuths_deg: Vec<f64>,
        polars_deg: Vec<f64>,
        grid: Array3<f64>,
    ) -> Result<Self, DirectivityError> {
        let (naz, npol, nband) = grid.dim();
        if azimuths_deg.len() < 2 {
            return Err(DirectivityError::TooFewNodes {
                axis: "azimuth",
                got: azimuths_deg.len(),
            });
        }
        if polars_deg.len() < 2 {
            return Err(DirectivityError::TooFewNodes {
                axis: "polar",
                got: polars_deg.len(),
            });
        }
        if azimuths_deg.len() != naz {
            return Err(DirectivityError::AxisGridMismatch {
                axis: "azimuth",
                axis_len: azimuths_deg.len(),
                grid_len: naz,
            });
        }
        if polars_deg.len() != npol {
            return Err(DirectivityError::AxisGridMismatch {
                axis: "polar",
                axis_len: polars_deg.len(),
                grid_len: npol,
            });
        }
        if nband != N_BANDS {
            return Err(DirectivityError::BandCountMismatch {
                expected: N_BANDS,
                got: nband,
            });
        }
        Self::validate_axis(&azimuths_deg, "azimuth", 0.0, 360.0)?;
        Self::validate_axis(&polars_deg, "polar", 0.0, 180.0)?;
        for (i, v) in grid.iter().enumerate() {
            if !v.is_finite() {
                return Err(DirectivityError::NonFinite {
                    what: format!("grid ΔL (flat index {i})"),
                });
            }
        }
        Ok(Self {
            azimuths_deg,
            polars_deg,
            grid,
        })
    }

    /// Build an equal-angle balloon by sampling an analytic pattern.
    ///
    /// `az_step_deg` must divide 360 evenly and `pol_step_deg` must divide 180
    /// evenly; the sampler `f(azimuth_deg, polar_deg, band_index) -> ΔL dB` is
    /// evaluated at every node (endpoints `0/360` and `0/180` inclusive, poles
    /// duplicated). This is how the harness samples the road analytic
    /// directivities (horn / vertical / heavy-horizontal) onto balloons.
    ///
    /// # Errors
    ///
    /// [`DirectivityError::UnevenStep`] if a step does not divide its span, or
    /// any error from [`DirectivityBalloon::new`] (e.g. a non-finite sample).
    pub fn from_equirect_sampler<F>(
        az_step_deg: f64,
        pol_step_deg: f64,
        f: F,
    ) -> Result<Self, DirectivityError>
    where
        F: Fn(f64, f64, usize) -> f64,
    {
        let azimuths_deg = equal_angle_axis(az_step_deg, 360.0, "azimuth")?;
        let polars_deg = equal_angle_axis(pol_step_deg, 180.0, "polar")?;
        let mut grid = Array3::<f64>::zeros((azimuths_deg.len(), polars_deg.len(), N_BANDS));
        for (ia, &az) in azimuths_deg.iter().enumerate() {
            for (ip, &pol) in polars_deg.iter().enumerate() {
                for band in 0..N_BANDS {
                    grid[[ia, ip, band]] = f(az, pol, band);
                }
            }
        }
        Self::new(azimuths_deg, polars_deg, grid)
    }

    /// The per-band ΔL dB slice in the direction `dir_local` (a vector in the
    /// balloon's source-local frame; it is normalized internally).
    ///
    /// Converts the direction to `(azimuth, polar)`, bilinearly interpolates the
    /// grid, and returns `[ΔL_band; N_BANDS]`. A non-finite or zero-length
    /// direction is a caller bug; rather than panic it returns an all-zero
    /// (no-gain) slice — the safe, non-panicking fallback (never a NaN into the
    /// tensor).
    #[must_use]
    pub fn eval(&self, dir_local: [f64; 3]) -> [f64; N_BANDS] {
        let Some((az, pol)) = dir_to_az_pol(dir_local) else {
            return [0.0; N_BANDS];
        };
        let (a0, a1, ta) = bracket(&self.azimuths_deg, az);
        let (p0, p1, tp) = bracket(&self.polars_deg, pol);
        let mut out = [0.0; N_BANDS];
        for (band, slot) in out.iter_mut().enumerate() {
            let c00 = self.grid[[a0, p0, band]];
            let c10 = self.grid[[a1, p0, band]];
            let c01 = self.grid[[a0, p1, band]];
            let c11 = self.grid[[a1, p1, band]];
            let bottom = c00 + (c10 - c00) * ta;
            let top = c01 + (c11 - c01) * ta;
            *slot = bottom + (top - bottom) * tp;
        }
        out
    }

    /// The azimuth axis (degrees), strictly ascending, spanning `[0, 360]`.
    #[must_use]
    pub fn azimuths_deg(&self) -> &[f64] {
        &self.azimuths_deg
    }

    /// The polar axis (degrees), strictly ascending, spanning `[0, 180]`.
    #[must_use]
    pub fn polars_deg(&self) -> &[f64] {
        &self.polars_deg
    }

    fn validate_axis(
        axis: &[f64],
        name: &'static str,
        lo: f64,
        hi: f64,
    ) -> Result<(), DirectivityError> {
        for (i, &v) in axis.iter().enumerate() {
            if !v.is_finite() {
                return Err(DirectivityError::NonFinite {
                    what: format!("{name} axis (index {i})"),
                });
            }
            if i > 0 && v <= axis[i - 1] {
                return Err(DirectivityError::NonAscending {
                    axis: name,
                    index: i,
                    prev: axis[i - 1],
                    value: v,
                });
            }
        }
        let first = axis[0];
        let last = axis[axis.len() - 1];
        if (first - lo).abs() > 1e-9 || (last - hi).abs() > 1e-9 {
            return Err(DirectivityError::BadSpan {
                axis: name,
                lo,
                hi,
                first,
                last,
            });
        }
        Ok(())
    }
}

/// A hand-rolled 3×3 rotation matrix (SRC-03), engine-local — **no linalg
/// crate** (D-08 precedent, mirroring the Cramer solve in `weather::route3`).
///
/// The caller rotates the src→rcv unit vector into the balloon's local frame
/// with [`Rotation3::apply`] before [`DirectivityBalloon::eval`]. Composing
/// rotations is a plain matrix product ([`Rotation3::then`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation3 {
    m: [[f64; 3]; 3],
}

impl Rotation3 {
    /// The identity rotation.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// A rotation from an explicit row-major 3×3 matrix (caller-validated).
    #[must_use]
    pub fn from_matrix(m: [[f64; 3]; 3]) -> Self {
        Self { m }
    }

    /// Rotation by `angle_rad` about the local +X axis.
    #[must_use]
    pub fn about_x(angle_rad: f64) -> Self {
        let (s, c) = angle_rad.sin_cos();
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        }
    }

    /// Rotation by `angle_rad` about the local +Y axis.
    #[must_use]
    pub fn about_y(angle_rad: f64) -> Self {
        let (s, c) = angle_rad.sin_cos();
        Self {
            m: [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        }
    }

    /// Rotation by `angle_rad` about the local +Z axis.
    #[must_use]
    pub fn about_z(angle_rad: f64) -> Self {
        let (s, c) = angle_rad.sin_cos();
        Self {
            m: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Apply the rotation to a vector: `M · v`.
    #[must_use]
    pub fn apply(&self, v: [f64; 3]) -> [f64; 3] {
        [
            self.m[0][0] * v[0] + self.m[0][1] * v[1] + self.m[0][2] * v[2],
            self.m[1][0] * v[0] + self.m[1][1] * v[1] + self.m[1][2] * v[2],
            self.m[2][0] * v[0] + self.m[2][1] * v[1] + self.m[2][2] * v[2],
        ]
    }

    /// Compose: `self.then(next)` applies `self` first, then `next`
    /// (the matrix product `next.m · self.m`).
    #[must_use]
    pub fn then(&self, next: &Rotation3) -> Rotation3 {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..3).map(|k| next.m[i][k] * self.m[k][j]).sum();
            }
        }
        Rotation3 { m }
    }
}

/// Build a strictly-ascending equal-angle axis `[0, step, 2·step, …, span]`.
fn equal_angle_axis(
    step: f64,
    span: f64,
    name: &'static str,
) -> Result<Vec<f64>, DirectivityError> {
    if !(step.is_finite() && step > 0.0) {
        return Err(DirectivityError::NonFinite {
            what: format!("{name} step"),
        });
    }
    let n = span / step;
    if (n - n.round()).abs() > 1e-9 {
        return Err(DirectivityError::UnevenStep {
            axis: name,
            step,
            span,
        });
    }
    let steps = n.round() as usize;
    let mut axis = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        // Snap the endpoint exactly to the span so validation sees [0, span].
        axis.push(if i == steps { span } else { step * i as f64 });
    }
    Ok(axis)
}

/// Map a (possibly unnormalized) direction to `(azimuth_deg, polar_deg)` in the
/// balloon frame. Returns `None` for a non-finite or zero-length vector.
fn dir_to_az_pol(dir: [f64; 3]) -> Option<(f64, f64)> {
    if dir.iter().any(|c| !c.is_finite()) {
        return None;
    }
    let norm = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if norm < 1e-12 {
        return None;
    }
    let (x, y, z) = (dir[0] / norm, dir[1] / norm, dir[2] / norm);
    let polar = z.clamp(-1.0, 1.0).acos().to_degrees();
    let azimuth = y.atan2(x).to_degrees().rem_euclid(360.0);
    Some((azimuth, polar))
}

/// Bracket a value in a strictly-ascending axis, clamping to the axis range.
///
/// Returns `(i0, i1, t)` with `axis[i0] ≤ v ≤ axis[i1]`, `i1 = i0 + 1` (or a
/// zero-width interval at an endpoint), and `t ∈ [0, 1]` the fractional
/// position for linear interpolation.
fn bracket(axis: &[f64], v: f64) -> (usize, usize, f64) {
    let last = axis.len() - 1;
    if v <= axis[0] {
        return (0, 0, 0.0);
    }
    if v >= axis[last] {
        return (last, last, 0.0);
    }
    // Linear scan is fine: axes are tiny (≤ 73 nodes) and hot loops evaluate
    // the balloon far less than they read the frequency grid.
    let mut i0 = 0;
    while i0 + 1 < axis.len() && axis[i0 + 1] < v {
        i0 += 1;
    }
    let i1 = i0 + 1;
    let span = axis[i1] - axis[i0];
    let t = if span > 0.0 {
        (v - axis[i0]) / span
    } else {
        0.0
    };
    (i0, i1, t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::f64::consts::PI;

    /// A smooth analytic road-style directivity: a horizontal cos lobe (horn
    /// effect) plus a vertical cos term (car-body screening), slightly
    /// band-dependent so the test exercises the per-band axis.
    fn analytic(dir: [f64; 3], band: usize) -> f64 {
        let norm = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let (x, _y, z) = (dir[0] / norm, dir[1] / norm, dir[2] / norm);
        let scale = 1.0 + 0.01 * band as f64;
        // Horizontal lobe peaking toward +X, vertical term from the elevation.
        scale * (2.0 * x - 1.0 * z)
    }

    fn sampled_balloon() -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler(5.0, 5.0, |az, pol, band| {
            let (x, y, z) = az_pol_to_unit(az, pol);
            analytic([x, y, z], band)
        })
        .expect("equal-angle sampler builds a valid balloon")
    }

    fn az_pol_to_unit(az_deg: f64, pol_deg: f64) -> (f64, f64, f64) {
        let a = az_deg.to_radians();
        let p = pol_deg.to_radians();
        (p.sin() * a.cos(), p.sin() * a.sin(), p.cos())
    }

    #[test]
    fn ctor_rejects_malformed_grids_with_typed_errors() {
        // Wrong band-axis length.
        let bad_band = Array3::<f64>::zeros((2, 2, N_BANDS - 1));
        assert!(matches!(
            DirectivityBalloon::new(vec![0.0, 360.0], vec![0.0, 180.0], bad_band),
            Err(DirectivityError::BandCountMismatch { .. })
        ));

        // Non-finite ΔL.
        let mut nan_grid = Array3::<f64>::zeros((2, 2, N_BANDS));
        nan_grid[[1, 1, 3]] = f64::NAN;
        assert!(matches!(
            DirectivityBalloon::new(vec![0.0, 360.0], vec![0.0, 180.0], nan_grid),
            Err(DirectivityError::NonFinite { .. })
        ));

        // Non-monotonic azimuth axis (needs 3 nodes to be non-ascending in-range).
        let g3 = Array3::<f64>::zeros((3, 2, N_BANDS));
        assert!(matches!(
            DirectivityBalloon::new(vec![0.0, 200.0, 180.0], vec![0.0, 180.0], g3.clone()),
            Err(DirectivityError::NonAscending { .. })
        ));

        // Bad span (azimuth does not reach 360°).
        let g2 = Array3::<f64>::zeros((2, 2, N_BANDS));
        assert!(matches!(
            DirectivityBalloon::new(vec![0.0, 300.0], vec![0.0, 180.0], g2.clone()),
            Err(DirectivityError::BadSpan { .. })
        ));

        // Axis / grid dimension mismatch.
        assert!(matches!(
            DirectivityBalloon::new(vec![0.0, 180.0, 360.0], vec![0.0, 180.0], g2),
            Err(DirectivityError::AxisGridMismatch { .. })
        ));

        // A valid balloon constructs.
        assert!(sampled_balloon().azimuths_deg().len() == 73);
    }

    #[test]
    fn eval_at_a_grid_node_returns_that_node_slice_exactly() {
        let b = sampled_balloon();
        // Node at azimuth 45°, polar 60° (both multiples of 5°).
        let (x, y, z) = az_pol_to_unit(45.0, 60.0);
        let got = b.eval([x, y, z]);
        for (band, &g) in got.iter().enumerate() {
            let want = analytic([x, y, z], band);
            assert_relative_eq!(g, want, epsilon = 1e-9);
        }
    }

    #[test]
    fn eval_between_nodes_is_the_bilinear_interpolant() {
        let b = sampled_balloon();
        // Half-way between azimuth nodes 40° and 45° at the polar node 90°.
        let (x, y, z) = az_pol_to_unit(42.5, 90.0);
        let got = b.eval([x, y, z]);
        for (band, &g) in got.iter().enumerate() {
            let (x0, y0, z0) = az_pol_to_unit(40.0, 90.0);
            let (x1, y1, z1) = az_pol_to_unit(45.0, 90.0);
            let want = 0.5 * (analytic([x0, y0, z0], band) + analytic([x1, y1, z1], band));
            assert_relative_eq!(g, want, epsilon = 1e-9);
        }
    }

    #[test]
    fn sampling_error_under_0_05_db_across_the_179_passby_azimuths() {
        // Pitfall 8: a 5° grid read back at the 179 pass-by evaluation azimuths
        // (θ ∈ [−89°, +89°] at 1° steps, in the horizontal plane) must deviate
        // < 0.05 dB from the smooth analytic form.
        let b = sampled_balloon();
        let mut max_err = 0.0_f64;
        for step in 0..179 {
            let theta = -89.0 + step as f64; // −89 … +89
            let az = theta.rem_euclid(360.0);
            let (x, y, z) = az_pol_to_unit(az, 90.0);
            let got = b.eval([x, y, z]);
            for (band, &g) in got.iter().enumerate() {
                let want = analytic([x, y, z], band);
                max_err = max_err.max((g - want).abs());
            }
        }
        assert!(
            max_err < 0.05,
            "balloon sampling error {max_err:.4} dB exceeds the 0.05 dB budget"
        );
    }

    #[test]
    fn rotating_a_lobe_away_from_a_fixed_receiver_lowers_the_level() {
        // A front lobe peaking toward +X: ΔL grows with the +X component.
        let b = DirectivityBalloon::from_equirect_sampler(5.0, 5.0, |az, pol, band| {
            let (x, _y, _z) = az_pol_to_unit(az, pol);
            (6.0 + 0.02 * band as f64) * x
        })
        .unwrap();

        // Fixed receiver straight along +X in the world frame.
        let rcv_world = [1.0, 0.0, 0.0];

        // No rotation: the receiver looks straight into the lobe → peak ΔL.
        let peak = b.eval(Rotation3::identity().apply(rcv_world));

        // Rotate the source +90° about Z: its local +X lobe now points to world
        // +Y, so the fixed +X receiver falls on the 0-dB flank. The lookup
        // applies the inverse (−90°) rotation to the world direction first.
        let rot = Rotation3::about_z(-PI / 2.0);
        let turned = b.eval(rot.apply(rcv_world));

        for band in 0..N_BANDS {
            assert!(
                peak[band] > 5.0,
                "band {band}: expected a strong on-axis lobe, got {}",
                peak[band]
            );
            assert!(
                turned[band] < peak[band] - 5.0,
                "band {band}: rotating the lobe away must drop the level \
                 (peak {:.3} → turned {:.3})",
                peak[band],
                turned[band]
            );
            // 90° off-axis of a linear-in-x lobe ⇒ ΔL ≈ 0.
            assert_relative_eq!(turned[band], 0.0, epsilon = 1e-9);
        }
    }

    #[test]
    fn rotation_about_z_is_a_proper_rotation() {
        // 90° about Z maps +X → +Y (right-handed), preserving length.
        let r = Rotation3::about_z(PI / 2.0);
        let v = r.apply([1.0, 0.0, 0.0]);
        assert_relative_eq!(v[0], 0.0, epsilon = 1e-12);
        assert_relative_eq!(v[1], 1.0, epsilon = 1e-12);
        assert_relative_eq!(v[2], 0.0, epsilon = 1e-12);
        // Composition: two 90° turns = one 180° turn (+X → −X).
        let two = r.then(&r).apply([1.0, 0.0, 0.0]);
        assert_relative_eq!(two[0], -1.0, epsilon = 1e-12);
        assert_relative_eq!(two[1], 0.0, epsilon = 1e-12);
    }

    #[test]
    fn eval_is_non_panicking_on_a_degenerate_direction() {
        let b = sampled_balloon();
        // Zero-length and non-finite directions return the no-gain slice.
        assert_eq!(b.eval([0.0, 0.0, 0.0]), [0.0; N_BANDS]);
        assert_eq!(b.eval([f64::NAN, 0.0, 1.0]), [0.0; N_BANDS]);
    }

    #[test]
    fn omni_balloon_is_zero_everywhere() {
        let b = DirectivityBalloon::omni();
        for dir in [[1.0, 0.0, 0.0], [0.0, 1.0, 1.0], [-1.0, -1.0, -1.0]] {
            assert_eq!(b.eval(dir), [0.0; N_BANDS]);
        }
    }
}
