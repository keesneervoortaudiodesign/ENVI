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
//! `10^{ΔL/20}` on `H_coh`, never a `propagation/` operator.
//!
//! # Directional phase (optional complex directivity)
//!
//! A balloon may ALSO carry a per-band **phase** grid `Δφ(azimuth, polar, band)`
//! in radians (source-local, ENVI's `e^{+jωt}` convention). It models the phase
//! of a directional source's far-field response — the load-bearing datum for
//! coherently summing multiple sub-sources (loudspeaker arrays / GLL-style
//! complex balloons), and consistent with the project pillar that phase is
//! preserved through the coherent chain. [`DirectivityBalloon::eval_complex`]
//! returns the complex linear gain `10^{ΔL/20}·e^{+jΔφ}` and
//! [`DirectivityBalloon::eval_phase`] returns the interpolated phase alone; a
//! balloon built without a phase grid reports phase `0` everywhere, so
//! `eval_complex` is exactly `10^{ΔL/20}` (bit-identical to the magnitude-only
//! path).
//!
//! The phase enters the **coherent** channel only: a `SolveJob` multiplies
//! `H_coh` by `10^{ΔL/20}·e^{+jΔφ}` while the incoherent energy channel keeps the
//! magnitude-only `10^{ΔL/10}` (`|·|²`) — the two-channel contract holds
//! (`F→1 ⇒ P_incoh→0` unaffected). Directivity is applied on the ENVI (post-conj)
//! side, never inside `propagation/`.
//!
//! **Interpolation:** magnitude is interpolated bilinearly in dB (so existing
//! real balloons are bit-identical); phase is interpolated as a unit complex
//! vector (`e^{+jΔφ}` corners, bilinear on re/im, then `arg`) so it is robust
//! across the ±π wrap. Supply a smoothly-sampled grid (the analytic samplers do);
//! near a phase antipode the unit-vector interpolant degenerates gracefully to
//! `0` rather than a NaN.
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
use num_complex::Complex;
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
    /// Optional per-band phase `Δφ(azimuth, polar, band)` in radians, same shape
    /// as `grid`. `None` is an all-real balloon (phase `0` everywhere).
    phase_grid_rad: Option<Array3<f64>>,
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
            phase_grid_rad: None,
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
            phase_grid_rad: None,
        })
    }

    /// Validate and construct a complex balloon: a magnitude grid `ΔL` (dB) plus
    /// a per-band phase grid `Δφ` (radians), sharing the same angle axes and
    /// shape.
    ///
    /// The magnitude grid follows the same rules as [`DirectivityBalloon::new`];
    /// the phase grid must have the identical shape `(azimuths.len(),
    /// polars.len(), N_BANDS)` and be everywhere finite. Phase is in ENVI's
    /// `e^{+jωt}` convention (see the module docs).
    ///
    /// # Errors
    ///
    /// Any [`DirectivityError`] from [`DirectivityBalloon::new`], or an
    /// [`AxisGridMismatch`](DirectivityError::AxisGridMismatch) /
    /// [`NonFinite`](DirectivityError::NonFinite) for a phase grid whose shape
    /// disagrees with the magnitude grid or that carries a non-finite value.
    pub fn new_with_phase(
        azimuths_deg: Vec<f64>,
        polars_deg: Vec<f64>,
        grid: Array3<f64>,
        phase_grid_rad: Array3<f64>,
    ) -> Result<Self, DirectivityError> {
        if phase_grid_rad.dim() != grid.dim() {
            let (_, _, pb) = phase_grid_rad.dim();
            return Err(DirectivityError::AxisGridMismatch {
                axis: "phase",
                axis_len: pb,
                grid_len: grid.dim().2,
            });
        }
        for (i, v) in phase_grid_rad.iter().enumerate() {
            if !v.is_finite() {
                return Err(DirectivityError::NonFinite {
                    what: format!("phase Δφ (flat index {i})"),
                });
            }
        }
        let base = Self::new(azimuths_deg, polars_deg, grid)?;
        Ok(Self {
            phase_grid_rad: Some(phase_grid_rad),
            ..base
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

    /// Build an equal-angle **complex** balloon by sampling magnitude and phase
    /// analytic patterns: `f_gain(az, pol, band) -> ΔL dB` and
    /// `f_phase(az, pol, band) -> Δφ rad`, evaluated on the same equal-angle grid
    /// as [`DirectivityBalloon::from_equirect_sampler`].
    ///
    /// # Errors
    ///
    /// [`DirectivityError::UnevenStep`] if a step does not divide its span, or
    /// any error from [`DirectivityBalloon::new_with_phase`] (e.g. a non-finite
    /// magnitude or phase sample).
    pub fn from_equirect_sampler_with_phase<G, P>(
        az_step_deg: f64,
        pol_step_deg: f64,
        f_gain: G,
        f_phase: P,
    ) -> Result<Self, DirectivityError>
    where
        G: Fn(f64, f64, usize) -> f64,
        P: Fn(f64, f64, usize) -> f64,
    {
        let azimuths_deg = equal_angle_axis(az_step_deg, 360.0, "azimuth")?;
        let polars_deg = equal_angle_axis(pol_step_deg, 180.0, "polar")?;
        let shape = (azimuths_deg.len(), polars_deg.len(), N_BANDS);
        let mut grid = Array3::<f64>::zeros(shape);
        let mut phase = Array3::<f64>::zeros(shape);
        for (ia, &az) in azimuths_deg.iter().enumerate() {
            for (ip, &pol) in polars_deg.iter().enumerate() {
                for band in 0..N_BANDS {
                    grid[[ia, ip, band]] = f_gain(az, pol, band);
                    phase[[ia, ip, band]] = f_phase(az, pol, band);
                }
            }
        }
        Self::new_with_phase(azimuths_deg, polars_deg, grid, phase)
    }

    /// Whether this balloon carries directional phase data.
    #[must_use]
    pub fn has_phase(&self) -> bool {
        self.phase_grid_rad.is_some()
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
        let Some(cell) = self.cell(dir_local) else {
            return [0.0; N_BANDS];
        };
        let mut out = [0.0; N_BANDS];
        for (band, slot) in out.iter_mut().enumerate() {
            *slot = cell.bilinear(&self.grid, band);
        }
        out
    }

    /// The per-band directional **phase** slice `Δφ` (radians) in the direction
    /// `dir_local`. A balloon without a phase grid returns all-zero (an all-real
    /// pattern); a degenerate direction returns all-zero (matching [`eval`]).
    ///
    /// Phase is interpolated as a unit complex vector (`e^{+jΔφ}` corners,
    /// bilinear on re/im, then `arg`) so the result is robust across the ±π wrap.
    ///
    /// [`eval`]: DirectivityBalloon::eval
    #[must_use]
    pub fn eval_phase(&self, dir_local: [f64; 3]) -> [f64; N_BANDS] {
        let Some(phase) = self.phase_grid_rad.as_ref() else {
            return [0.0; N_BANDS];
        };
        let Some(cell) = self.cell(dir_local) else {
            return [0.0; N_BANDS];
        };
        let mut out = [0.0; N_BANDS];
        for (band, slot) in out.iter_mut().enumerate() {
            *slot = cell.bilinear_phase(phase, band);
        }
        out
    }

    /// The per-band **complex** linear gain `10^{ΔL/20}·e^{+jΔφ}` in the
    /// direction `dir_local` — magnitude from the dB grid (bit-identical to
    /// `10^{eval/20}`), phase from the phase grid (`0` if absent).
    ///
    /// This is the datum a `SolveJob` multiplies into `H_coh`; the incoherent
    /// channel uses only its magnitude (`|·|² = 10^{ΔL/10}`).
    #[must_use]
    pub fn eval_complex(&self, dir_local: [f64; 3]) -> [Complex<f64>; N_BANDS] {
        let mag_db = self.eval(dir_local);
        let phase = self.eval_phase(dir_local);
        let mut out = [Complex::new(0.0, 0.0); N_BANDS];
        for (band, slot) in out.iter_mut().enumerate() {
            let mag = 10f64.powf(mag_db[band] / 20.0);
            // ENVI e^{+jωt}: explicit (cos, sin), never `.conj()`.
            *slot = Complex::new(mag * phase[band].cos(), mag * phase[band].sin());
        }
        out
    }

    /// Resolve a direction to a bilinear cell (bracketed indices + weights),
    /// shared by [`eval`], [`eval_phase`], and [`eval_complex`].
    ///
    /// [`eval`]: DirectivityBalloon::eval
    fn cell(&self, dir_local: [f64; 3]) -> Option<BilinearCell> {
        let (az, pol) = dir_to_az_pol(dir_local)?;
        let (a0, a1, ta) = bracket(&self.azimuths_deg, az);
        let (p0, p1, tp) = bracket(&self.polars_deg, pol);
        Some(BilinearCell {
            a0,
            a1,
            p0,
            p1,
            ta,
            tp,
        })
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

/// A resolved bilinear interpolation cell: the four bracketing grid indices and
/// the fractional weights, computed once per direction and reused across bands.
#[derive(Debug, Clone, Copy)]
struct BilinearCell {
    a0: usize,
    a1: usize,
    p0: usize,
    p1: usize,
    ta: f64,
    tp: f64,
}

impl BilinearCell {
    /// Bilinearly interpolate a real grid at this cell for one band.
    fn bilinear(&self, grid: &Array3<f64>, band: usize) -> f64 {
        let c00 = grid[[self.a0, self.p0, band]];
        let c10 = grid[[self.a1, self.p0, band]];
        let c01 = grid[[self.a0, self.p1, band]];
        let c11 = grid[[self.a1, self.p1, band]];
        let bottom = c00 + (c10 - c00) * self.ta;
        let top = c01 + (c11 - c01) * self.ta;
        bottom + (top - bottom) * self.tp
    }

    /// Interpolate a phase grid (radians) as a unit complex vector, returning the
    /// `arg` of the interpolant so the result is robust across the ±π wrap. A
    /// degenerate (near-zero-length) interpolant returns `0`.
    fn bilinear_phase(&self, phase: &Array3<f64>, band: usize) -> f64 {
        let unit = |ia: usize, ip: usize| -> (f64, f64) {
            let p = phase[[ia, ip, band]];
            (p.cos(), p.sin())
        };
        let (r00, i00) = unit(self.a0, self.p0);
        let (r10, i10) = unit(self.a1, self.p0);
        let (r01, i01) = unit(self.a0, self.p1);
        let (r11, i11) = unit(self.a1, self.p1);
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let re_b = lerp(r00, r10, self.ta);
        let re_t = lerp(r01, r11, self.ta);
        let im_b = lerp(i00, i10, self.ta);
        let im_t = lerp(i01, i11, self.ta);
        let re = lerp(re_b, re_t, self.tp);
        let im = lerp(im_b, im_t, self.tp);
        if re.abs() < 1e-15 && im.abs() < 1e-15 {
            0.0
        } else {
            im.atan2(re)
        }
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

    /// A smooth analytic directional phase (radians): an azimuthal ramp plus a
    /// mild band tilt, kept well inside [−π, π] to exercise interpolation.
    fn analytic_phase(dir: [f64; 3], band: usize) -> f64 {
        let (az, _pol) = dir_to_az_pol(dir).unwrap();
        0.5 * (az.to_radians()).sin() + 0.001 * band as f64
    }

    fn complex_balloon() -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler_with_phase(
            5.0,
            5.0,
            |az, pol, band| {
                let (x, y, z) = az_pol_to_unit(az, pol);
                analytic([x, y, z], band)
            },
            |az, pol, band| {
                let (x, y, z) = az_pol_to_unit(az, pol);
                analytic_phase([x, y, z], band)
            },
        )
        .expect("complex equal-angle sampler builds a valid balloon")
    }

    #[test]
    fn real_balloon_reports_no_phase_and_real_complex_gain() {
        // A magnitude-only balloon carries no phase; eval_complex is exactly
        // 10^{eval/20} bit-for-bit (the backward-compatible invariant).
        let b = sampled_balloon();
        assert!(!b.has_phase());
        for dir in [[1.0, 0.0, 0.0], [0.3, -0.7, 0.5], [0.0, 0.0, -1.0]] {
            assert_eq!(b.eval_phase(dir), [0.0; N_BANDS]);
            let mag = b.eval(dir);
            let cx = b.eval_complex(dir);
            for band in 0..N_BANDS {
                let want = Complex::new(10f64.powf(mag[band] / 20.0), 0.0);
                assert_eq!(cx[band], want, "band {band}");
            }
        }
    }

    #[test]
    fn complex_balloon_evals_magnitude_and_phase_at_a_node() {
        let b = complex_balloon();
        assert!(b.has_phase());
        // Node at azimuth 45°, polar 60° (both multiples of 5°).
        let (x, y, z) = az_pol_to_unit(45.0, 60.0);
        let dir = [x, y, z];
        let phase = b.eval_phase(dir);
        let cx = b.eval_complex(dir);
        for band in 0..N_BANDS {
            let want_phase = analytic_phase(dir, band);
            assert_relative_eq!(phase[band], want_phase, epsilon = 1e-9);
            let want_mag = 10f64.powf(analytic(dir, band) / 20.0);
            assert_relative_eq!(cx[band].norm(), want_mag, epsilon = 1e-9);
            assert_relative_eq!(cx[band].arg(), want_phase, epsilon = 1e-9);
        }
    }

    #[test]
    fn phase_interpolation_is_robust_across_the_pi_wrap() {
        // Two azimuth nodes straddling the ±π branch cut: +170° and +190°
        // (i.e. −170°). The unit-vector interpolant at the midpoint (180°) must
        // land at ±π, NOT at 0° (which naive radian averaging would give).
        let b = DirectivityBalloon::from_equirect_sampler_with_phase(
            10.0,
            10.0,
            |_az, _pol, _band| 0.0,
            |az, _pol, _band| {
                // Phase = the azimuth itself mapped to (−π, π].
                let a = az.to_radians();
                if a > PI { a - 2.0 * PI } else { a }
            },
        )
        .unwrap();
        let (x, y, z) = az_pol_to_unit(180.0, 90.0);
        let phase = b.eval_phase([x, y, z]);
        // Interpolated phase magnitude ≈ π (either branch), never ≈ 0.
        assert!(
            (phase[0].abs() - PI).abs() < 1e-9,
            "phase at 180° should be ±π, got {}",
            phase[0]
        );
    }

    #[test]
    fn new_with_phase_rejects_a_shape_mismatched_phase_grid() {
        let grid = Array3::<f64>::zeros((2, 2, N_BANDS));
        let bad_phase = Array3::<f64>::zeros((2, 2, N_BANDS - 1));
        assert!(matches!(
            DirectivityBalloon::new_with_phase(vec![0.0, 360.0], vec![0.0, 180.0], grid, bad_phase),
            Err(DirectivityError::AxisGridMismatch { axis: "phase", .. })
        ));
    }

    #[test]
    fn eval_complex_is_non_panicking_on_a_degenerate_direction() {
        let b = complex_balloon();
        assert_eq!(
            b.eval_complex([0.0, 0.0, 0.0]),
            [Complex::new(1.0, 0.0); N_BANDS]
        );
        assert_eq!(b.eval_phase([f64::NAN, 0.0, 1.0]), [0.0; N_BANDS]);
    }
}
