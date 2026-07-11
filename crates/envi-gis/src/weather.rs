//! Weather-route sound-speed profile math (MET-02/05/06, METX-01) — the
//! WASM-safe home of the log-lin `(A, B, C, sA, sB, z₀)` fit.
//!
//! # Module I/O
//! - **Inputs:** an Open-Meteo multi-level profile (temperature, wind speed,
//!   wind direction, AMSL geopotential height per pressure level) plus the site
//!   elevation and a reference downwind bearing; or, for the low-level pure
//!   math, `(heights, values, z₀)` samples.
//! - **Output:** a bearing-independent [`WeatherComponents`] decomposition and,
//!   per path azimuth, a concrete [`WeatherProfile`] / engine
//!   [`SoundSpeedProfile`]. Absence / non-finite input is a typed [`GisError`],
//!   never a fabricated value or a panic.
//! - **Invariants (load-bearing):**
//!   1. **Single source of truth for the LSQ** (09-03 prohibition): the 3×3
//!      Cramer normal-equations fit lives ONLY here; `envi-harness` consumes it
//!      (Route 2/3 re-export/delegate, never a second copy).
//!   2. **AMSL → AGL** (Pitfall 5): geopotential height is above mean sea level;
//!      the site elevation is subtracted before fitting — a level height is
//!      never sampled as height-above-ground.
//!   3. **No linalg crate** (D-08): the 3-parameter solve is a hand-rolled 3×3
//!      Cramer's rule; the singular case is a typed error, never a NaN profile.
//!
//! # Lifted from `envi-harness` (09-03, RESEARCH Don't-Hand-Roll)
//!
//! [`WeatherComponents`], [`WeatherProfile`], [`profile_for_bearing`],
//! [`ReflectionProfiles`] and [`fit_profile`] were **lifted verbatim** out of
//! `envi-harness/src/weather/{mod,route3}.rs` into this WASM-safe crate (they
//! depend only on `envi_engine`). The harness re-exports/delegates to them so
//! the Phase-3 round-trip oracle still covers the fit and there is exactly one
//! copy of the LSQ. Open-Meteo gives `u(z), T(z)` **directly**, so METX-01 is
//! the Route-2 "profile fit" — it **skips** the Monin–Obukhov reconstruction
//! (that is Route 3's job when only surface met is available).
//!
//! # ⚠️ Route derivations are `[ASSUMED]` (RESEARCH Weather-Routes banner)
//!
//! AV 1106/07 takes `A, B, C` as **inputs** and does **not** specify how to
//! derive them from wind/temperature. The Route-2 A/B **scaling constants** are
//! therefore `[ASSUMED]` and are validated by **direction/structure property
//! tests only** — never a false FORCE numeric pass (locked pending the 03-03
//! Open-Q1 checkpoint). Only the **sign logic** is physically certain:
//! downwind ⇒ larger A, an inversion (`dt/dz > 0`) ⇒ `B > 0`.
//!
//! # Per-azimuth A (MET-02)
//!
//! The **isotropic temperature part** of `A` is computed **once**; the
//! **wind part is projected per bearing** as `A_wind·cos(bearing − φ_u)` using
//! the [`envi_engine::geometry::azimuth_deg`] clockwise-from-north convention
//! (`φ_u` is the downwind bearing). `B` (stability / temperature gradient) is
//! **bearing-independent**.

use crate::GisError;
use envi_engine::propagation::PropagationError;
use envi_engine::propagation::refraction::SoundSpeedProfile;
use envi_engine::propagation::refraction::eqssp::calc_eq_ssp;
use envi_engine::propagation::refraction::profile::Z0_MIN_M;
use envi_engine::propagation::sound_speed_ms;

/// The pressure levels requested from Open-Meteo (hPa), surface → ~1.5 km, where
/// the sound-relevant profile lives (RESEARCH Pattern 5). Compared/fit by height
/// after the AMSL → AGL conversion, never by nominal pressure.
pub const PRESSURE_LEVELS_HPA: [u32; 6] = [1000, 975, 950, 925, 900, 850];

/// Near-surface AGL anchor height, m. The pressure levels alone start at ~90 m
/// AGL, so the `[ln(z/z₀+1), z, 1]` basis is ill-conditioned over that narrow
/// high band (the log curvature lives near the ground). Open-Meteo's height-AGL
/// variables (`temperature_2m`, `wind_speed_10m`, `wind_direction_10m`) supply a
/// low anchor; using the 2 m temperature as the ~10 m value is a sub-0.1 °C
/// approximation, well inside the `[ASSUMED]` weather-route tolerance
/// (RESEARCH Pattern 5, A5). The value is height-above-ground already — NO
/// elevation subtraction (that is only for the AMSL geopotential levels).
pub const NEAR_SURFACE_HEIGHT_M: f64 = 10.0;

/// A log-lin sound-speed profile for one propagation bearing:
/// `c(z) = A·ln(z/z₀+1) + B·z + C`, plus the fluctuation std-devs `sA`/`sB`
/// (Eq. 10 upper-refraction profile `A⁺ = A + 1.7·sA`).
///
/// This is the harness-side mirror of the engine's
/// [`envi_engine::propagation::refraction::SoundSpeedProfile`]: the two structs
/// carry the same `(A, B, C, sA, sB, z₀)` fields (the engine struct *does* have
/// `s_a`/`s_b`) and differ only in crate location — I/O-side (this crate) vs
/// pure-math (engine). Use [`sound_speed_profile_for_azimuth`] to cross over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherProfile {
    /// Logarithmic coefficient `A`, m/s (wind-projected per bearing).
    pub a: f64,
    /// Linear coefficient `B`, s⁻¹ (bearing-independent; inversion ⇒ B > 0).
    pub b: f64,
    /// Ground sound speed `C = Coft(t₀)`, m/s.
    pub c: f64,
    /// Std-dev of `A` (fluctuating refraction), m/s.
    pub s_a: f64,
    /// Std-dev of `B` (fluctuating refraction), s⁻¹.
    pub s_b: f64,
    /// Roughness length `z₀`, m (clamped ≥ 0.001 m).
    pub z0: f64,
}

impl WeatherProfile {
    /// Construct a profile, **clamping `z₀ ≥ 0.001 m`** (MET-01 / D-16, matching
    /// the engine's [`Z0_MIN_M`] posture) so the ray calculations never hit the
    /// `z₀ → 0` singularity. Non-finite inputs are the caller's responsibility
    /// to reject (the derivation does so with a typed error before calling here).
    #[must_use]
    pub fn new(a: f64, b: f64, c: f64, s_a: f64, s_b: f64, z0: f64) -> Self {
        Self {
            a,
            b,
            c,
            s_a,
            s_b,
            z0: z0.max(Z0_MIN_M),
        }
    }
}

/// The **bearing-independent** decomposition of a weather profile: the wind
/// contribution to `A` is kept as a magnitude `a_wind` (projected per bearing),
/// separate from the isotropic temperature part `a_temp` (added once). `B`, `C`,
/// `sA`, `sB`, `z₀` are all bearing-independent.
///
/// [`profile_for_bearing`] turns this plus a bearing into a [`WeatherProfile`].
/// Keeping the decomposition lets a **reflection path** (ENG-06) project the
/// wind part at two different sub-path bearings while computing the temperature
/// part only once (MET-02).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherComponents {
    /// Isotropic temperature part of `A` (computed once), m/s.
    pub a_temp: f64,
    /// Wind magnitude of `A` (projected per bearing as `a_wind·cos(Δ)`), m/s.
    pub a_wind: f64,
    /// Linear coefficient `B`, s⁻¹ (bearing-independent).
    pub b: f64,
    /// Ground sound speed `C`, m/s.
    pub c: f64,
    /// Std-dev of `A`, m/s.
    pub s_a: f64,
    /// Std-dev of `B`, s⁻¹.
    pub s_b: f64,
    /// Roughness length `z₀`, m.
    pub z0: f64,
}

/// Project the wind part of `base` onto `bearing_deg` and return the concrete
/// [`WeatherProfile`] (MET-02).
///
/// `A(bearing) = a_temp + a_wind·cos(bearing − φ_u)` — downwind
/// (`bearing ≈ φ_u`) gives the largest `A`; a bearing 180° opposite gives the
/// smallest. Bearings are degrees clockwise from north
/// ([`envi_engine::geometry::azimuth_deg`] convention). `B`/`C`/`sA`/`sB`/`z₀`
/// pass through unchanged (bearing-independent).
#[must_use]
pub fn profile_for_bearing(
    base: &WeatherComponents,
    bearing_deg: f64,
    phi_u_deg: f64,
) -> WeatherProfile {
    let delta = (bearing_deg - phi_u_deg).to_radians();
    let a = base.a_temp + base.a_wind * delta.cos();
    WeatherProfile::new(a, base.b, base.c, base.s_a, base.s_b, base.z0)
}

/// Build the engine's [`SoundSpeedProfile`] for a given path azimuth from the
/// bearing-independent components (METX-01 → solver seam). The wind part of `A`
/// is projected onto `path_az_deg` via [`profile_for_bearing`]; `sA`/`sB` are
/// emitted as `0` (non-fluctuating ⇒ `FΔν = 1` bit-exact) — the Open-Meteo
/// single-hour profile carries no fluctuation std-devs.
#[must_use]
pub fn sound_speed_profile_for_azimuth(
    base: &WeatherComponents,
    path_az_deg: f64,
    phi_u_deg: f64,
) -> SoundSpeedProfile {
    let wp = profile_for_bearing(base, path_az_deg, phi_u_deg);
    SoundSpeedProfile {
        a: wp.a,
        b: wp.b,
        c: wp.c,
        s_a: 0.0,
        s_b: 0.0,
        z0: wp.z0,
    }
}

/// One sub-path CalcEqSSP collapse `(ξ, c₀)` — the equivalent relative gradient
/// and ground sound speed for a single reflection sub-path leg.
pub type SubPathCollapse = (f64, f64);

/// The before/after weather-profile pair for a **reflection path** (ENG-06,
/// §5.3.4): the propagation model runs on each sub-path with its own equivalent
/// profile — `before` between source and the reflection point (`A₁, B₁`),
/// `after` between the reflection point and the receiver (`A₂, B₂`).
///
/// `A` differs per sub-path because each leg has its own bearing (the wind
/// projection `A ∝ u·cos(bearing − φ_u)` differs); `B`/`C`/`z₀` are shared
/// (bearing-independent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReflectionProfiles {
    /// Source → reflection-point sub-path profile (`A₁, B₁`).
    pub before: WeatherProfile,
    /// Reflection-point → receiver sub-path profile (`A₂, B₂`).
    pub after: WeatherProfile,
}

impl ReflectionProfiles {
    /// Build the before/after pair from the shared [`WeatherComponents`] and the
    /// two sub-path bearings (MET-02: the wind part of `A` is projected onto
    /// each bearing; the isotropic temperature part and `B` are shared).
    #[must_use]
    pub fn from_components(
        base: &WeatherComponents,
        bearing_before_deg: f64,
        bearing_after_deg: f64,
        phi_u_deg: f64,
    ) -> Self {
        Self {
            before: profile_for_bearing(base, bearing_before_deg, phi_u_deg),
            after: profile_for_bearing(base, bearing_after_deg, phi_u_deg),
        }
    }

    /// Collapse each sub-path to its own `(ξ, c₀)` via CalcEqSSP on its leg
    /// geometry (§5.5.2 "separate equivalent profiles … between source and the
    /// nearest screen … receiver and the nearest screen"): the `before` leg uses
    /// `(before_hs, before_hr)`, the `after` leg `(after_hs, after_hr)`.
    ///
    /// Returns `((ξ₁, c₀₁), (ξ₂, c₀₂))`.
    ///
    /// # Errors
    ///
    /// [`PropagationError::DegenerateProfile`] if either sub-path profile is
    /// degenerate (non-finite / non-physical) — the same typed error the engine
    /// raises, propagated so a bad reflector geometry never panics.
    pub fn sub_path_collapses(
        &self,
        before_hs: f64,
        before_hr: f64,
        after_hs: f64,
        after_hr: f64,
    ) -> Result<(SubPathCollapse, SubPathCollapse), PropagationError> {
        let e1 = calc_eq_ssp(
            before_hs,
            before_hr,
            self.before.z0,
            self.before.a,
            self.before.b,
            self.before.c,
        )?;
        let e2 = calc_eq_ssp(
            after_hs,
            after_hr,
            self.after.z0,
            self.after.a,
            self.after.b,
            self.after.c,
        )?;
        Ok((e1, e2))
    }
}

/// Solve the 3×3 system `M·β = r` by Cramer's rule (cofactor determinants).
///
/// Returns `None` when the matrix is singular (`|det| ≤ eps·‖M‖`), which the
/// caller maps to a typed error — no `nalgebra`, no panic (D-08 / T-03-03-02).
fn solve_3x3(m: [[f64; 3]; 3], r: [f64; 3]) -> Option<[f64; 3]> {
    let det3 = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let det = det3(m);
    // Scale guard: reject a determinant that is negligible vs the matrix norm.
    let scale: f64 = m.iter().flatten().map(|v| v.abs()).sum::<f64>().max(1e-30);
    if !det.is_finite() || det.abs() <= 1e-12 * scale.powi(3) {
        return None;
    }
    let mut beta = [0.0; 3];
    for (i, b) in beta.iter_mut().enumerate() {
        let mut mi = m;
        for row in 0..3 {
            mi[row][i] = r[row];
        }
        *b = det3(mi) / det;
    }
    if beta.iter().all(|v| v.is_finite()) {
        Some(beta)
    } else {
        None
    }
}

/// Least-squares fit of the log-lin model `values(z) ≈ A·ln(z/z₀+1) + B·z + C`
/// to the `(heights, values)` samples via the hand-rolled 3×3 normal equations
/// `XᵀX β = Xᵀy` (D-08). Returns `(A, B, C)`.
///
/// The basis is `[ln(z/z₀+1), z, 1]`; `z₀` is clamped ≥ 0.001 m. With ≥ 3
/// non-degenerate heights the normal matrix is well-conditioned; a synthetic
/// profile generated from an exact `(A, B, C)` is recovered exactly (the
/// round-trip identity, the oracle for MET-06 / METX-01).
///
/// **This is the single source of truth for the weather LSQ** — `envi-harness`
/// Route 2/3 delegate to it (no forked copy, 09-03 prohibition).
///
/// # Errors
///
/// - [`GisError::WeatherFit`] if fewer than 3 samples, mismatched lengths, or a
///   singular normal matrix (collinear heights) — never a panic (T-09-03-01).
/// - [`GisError::NonFinite`] if any sample height/value is non-finite.
pub fn fit_profile(heights: &[f64], values: &[f64], z0: f64) -> Result<(f64, f64, f64), GisError> {
    if heights.len() != values.len() || heights.len() < 3 {
        return Err(GisError::WeatherFit {
            message: format!(
                "need ≥ 3 matched samples, got {} heights / {} values",
                heights.len(),
                values.len()
            ),
        });
    }
    let z0 = z0.max(Z0_MIN_M);
    // Accumulate XᵀX (symmetric 3×3) and Xᵀy.
    let mut m = [[0.0_f64; 3]; 3];
    let mut r = [0.0_f64; 3];
    for (&z, &y) in heights.iter().zip(values) {
        if !(z.is_finite() && z >= 0.0 && y.is_finite()) {
            return Err(GisError::NonFinite {
                what: "weather-fit sample height or value".to_string(),
            });
        }
        let phi = [(z / z0 + 1.0).ln(), z, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += phi[i] * phi[j];
            }
            r[i] += phi[i] * y;
        }
    }
    let beta = solve_3x3(m, r).ok_or_else(|| GisError::WeatherFit {
        message: "singular normal matrix (collinear / insufficient heights)".to_string(),
    })?;
    Ok((beta[0], beta[1], beta[2]))
}

/// One vertical level of an Open-Meteo multi-level profile, already converted to
/// **height above ground** (AMSL geopotential minus site elevation, Pitfall 5).
///
/// `wind_direction_deg` is the meteorological direction the wind blows **from**
/// (degrees clockwise from north); the downwind bearing is `+ 180°`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    /// Height above ground level, m (`geopotential_height − site_elevation`).
    pub height_agl_m: f64,
    /// Air temperature at this level, °C.
    pub temperature_c: f64,
    /// Wind speed at this level, m/s.
    pub wind_speed_ms: f64,
    /// Meteorological wind direction (blows **from**), degrees clockwise from N.
    pub wind_direction_deg: f64,
}

/// Fit the bearing-independent [`WeatherComponents`] from an Open-Meteo
/// multi-level profile (METX-01, the Route-2 "profile fit"), using the SAME
/// `[ASSUMED]` separable model the validated harness Route 2 uses (`route2.rs`):
///
/// - **Temperature → linear gradient.** The temperature-only effective sound
///   speed `c_T(z) = 20.05·√(T(z)+273.15)` (Eq. 335) is fit to `B·z + C`
///   (`B` = gradient, `C` = ground sound speed). `a_temp = 0` — temperature does
///   **not** enter the log part (the neutral-log-law model, `route2` module
///   docs). An inversion (`T` rising) ⇒ `c_T` rising ⇒ `B > 0`.
/// - **Wind → neutral log law.** The wind projected onto the reference downwind
///   bearing `phi_wind_deg` (`u_along(z) = speed·cos(downwind_bearing(z) −
///   phi_wind_deg)`) is fit to `a_wind·ln(z/z₀+1)` → `a_wind`. Wind that
///   strengthens downwind with height ⇒ `a_wind > 0` ⇒ downwind A > upwind A.
///
/// [`profile_for_bearing`] then projects the wind part per path azimuth.
///
/// # Why not a full 3-param [`fit_profile`] of both terms (RESEARCH A5)
///
/// The research code sketch fit `(a_temp, b, c)` and `a_wind` with the full 3×3
/// LSQ. Over **real** Open-Meteo pressure levels (sparse, all ≥ ~90 m AGL) the
/// `[ln(z/z₀+1), z, 1]` basis is **ill-conditioned** — the log and linear terms
/// are not separable there — so the naive fit is singular. The separable model
/// above is well-conditioned for any height spread and matches the validated
/// Route-2 `[ASSUMED]` derivation, so the lifted [`fit_profile`] stays the
/// single-source LSQ (consumed by harness Route 3, covered by the round-trip
/// oracle) without being forced onto ill-posed data here.
///
/// `sA`/`sB` are `0` (the single-hour profile carries no fluctuation std-devs).
///
/// # `[ASSUMED]` (do not promote to a FORCE numeric pass)
///
/// The temperature/wind split is the `[ASSUMED]` Route-2 choice. Validate
/// structurally only: downwind A > upwind A; inversion ⇒ `B > 0`; round-trip
/// identity (on [`fit_profile`]).
///
/// # Errors
///
/// [`GisError::NonFinite`] if `phi_wind_deg` or any level field is non-finite;
/// [`GisError::WeatherFit`] if the temperature or wind fit is under-determined
/// (fewer than the needed distinct heights / identical heights).
pub fn components_from_levels(
    levels: &[Level],
    phi_wind_deg: f64,
    z0: f64,
) -> Result<WeatherComponents, GisError> {
    if !phi_wind_deg.is_finite() {
        return Err(GisError::NonFinite {
            what: "reference downwind bearing phi_wind_deg".to_string(),
        });
    }
    let mut heights = Vec::with_capacity(levels.len());
    let mut c_temp = Vec::with_capacity(levels.len());
    let mut u_along = Vec::with_capacity(levels.len());
    for lvl in levels {
        for (v, what) in [
            (lvl.height_agl_m, "level height_agl"),
            (lvl.temperature_c, "level temperature"),
            (lvl.wind_speed_ms, "level wind_speed"),
            (lvl.wind_direction_deg, "level wind_direction"),
        ] {
            if !v.is_finite() {
                return Err(GisError::NonFinite {
                    what: what.to_string(),
                });
            }
        }
        // Temperature-only effective sound speed (isotropic part), Eq. 335.
        heights.push(lvl.height_agl_m);
        c_temp.push(sound_speed_ms(lvl.temperature_c));
        // Project the level wind (blows FROM `wind_direction_deg`, i.e. TOWARD
        // `+180°`) onto the reference downwind bearing so `a_wind > 0` for a wind
        // that strengthens downwind with height (MET-02 sign convention).
        let downwind_bearing = lvl.wind_direction_deg + 180.0;
        let delta = (downwind_bearing - phi_wind_deg).to_radians();
        u_along.push(lvl.wind_speed_ms * delta.cos());
    }
    let (b, c) = fit_linear(&heights, &c_temp)?; // temperature gradient + ground C
    let a_wind = fit_log_coeff(&heights, &u_along, z0)?; // neutral-log-law wind coeff
    Ok(WeatherComponents {
        a_temp: 0.0, // temperature contributes 0 to the log part (route2 [ASSUMED])
        a_wind,
        b,
        c,
        s_a: 0.0,
        s_b: 0.0,
        z0: z0.max(Z0_MIN_M),
    })
}

/// 2-parameter linear least squares `y ≈ slope·z + intercept`; returns
/// `(slope, intercept)`. Well-conditioned for any ≥ 2 distinct heights (the
/// temperature-gradient fit). Not the 3×3 LSQ — a different, simpler model.
///
/// # Errors
///
/// [`GisError::NonFinite`] on a non-finite sample; [`GisError::WeatherFit`] on
/// fewer than 2 samples, mismatched lengths, or all-identical heights (singular).
fn fit_linear(heights: &[f64], values: &[f64]) -> Result<(f64, f64), GisError> {
    let n = heights.len();
    if n != values.len() || n < 2 {
        return Err(GisError::WeatherFit {
            message: format!(
                "linear fit needs ≥ 2 matched samples, got {} / {}",
                heights.len(),
                values.len()
            ),
        });
    }
    let (mut sz, mut sy, mut szz, mut szy) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for (&z, &y) in heights.iter().zip(values) {
        if !(z.is_finite() && y.is_finite()) {
            return Err(GisError::NonFinite {
                what: "linear-fit sample height or value".to_string(),
            });
        }
        sz += z;
        sy += y;
        szz += z * z;
        szy += z * y;
    }
    let nf = n as f64;
    let denom = nf * szz - sz * sz;
    let scale = (nf * szz).abs().max(sz * sz).max(1e-30);
    if !denom.is_finite() || denom.abs() <= 1e-12 * scale {
        return Err(GisError::WeatherFit {
            message: "linear fit singular (identical heights)".to_string(),
        });
    }
    let slope = (nf * szy - sz * sy) / denom;
    let intercept = (sy - slope * sz) / nf;
    Ok((slope, intercept))
}

/// 1-parameter neutral-log-law least squares `y ≈ a·ln(z/z₀+1)` through the
/// origin; returns `a` (the log coefficient). Clamps `z₀ ≥ 0.001 m` and is
/// well-conditioned for any height spread (the wind-coefficient fit). Not the
/// 3×3 LSQ.
///
/// # Errors
///
/// [`GisError::NonFinite`] on a non-finite sample; [`GisError::WeatherFit`] on
/// no samples or all heights at `z = 0` (degenerate log basis).
fn fit_log_coeff(heights: &[f64], values: &[f64], z0: f64) -> Result<f64, GisError> {
    if heights.len() != values.len() || heights.is_empty() {
        return Err(GisError::WeatherFit {
            message: format!(
                "log fit needs ≥ 1 matched sample, got {} / {}",
                heights.len(),
                values.len()
            ),
        });
    }
    let z0 = z0.max(Z0_MIN_M);
    let (mut sww, mut swy) = (0.0_f64, 0.0_f64);
    for (&z, &y) in heights.iter().zip(values) {
        if !(z.is_finite() && z >= 0.0 && y.is_finite()) {
            return Err(GisError::NonFinite {
                what: "log-fit sample height or value".to_string(),
            });
        }
        let w = (z / z0 + 1.0).ln();
        sww += w * w;
        swy += w * y;
    }
    if !sww.is_finite() || sww <= 1e-30 {
        return Err(GisError::WeatherFit {
            message: "log fit degenerate (all heights at z = 0)".to_string(),
        });
    }
    Ok(swy / sww)
}

/// Parse an Open-Meteo Archive/Forecast pressure-level response (bytes) into the
/// [`Level`] set for one hour, converting AMSL geopotential height to AGL by
/// subtracting the response `elevation` (Pitfall 5). The levels are returned in
/// **ascending AGL height** order (the fit wants strictly-ascending heights).
///
/// Only the [`PRESSURE_LEVELS_HPA`] set is read; the schema/units are shared by
/// the Archive and Forecast endpoints (D-02), so one parser serves both.
///
/// # Errors
///
/// [`GisError::Json`] if the response, a required hourly array, or the hour
/// index is missing/malformed; [`GisError::NonFinite`] if a value is non-finite.
pub fn levels_from_openmeteo(json: &[u8], hour_index: usize) -> Result<Vec<Level>, GisError> {
    let resp: OpenMeteoResponse = serde_json::from_slice(json).map_err(|e| GisError::Json {
        message: e.to_string(),
    })?;
    let elevation = resp.elevation;
    if !elevation.is_finite() {
        return Err(GisError::NonFinite {
            what: "open-meteo response elevation".to_string(),
        });
    }
    let hourly = &resp.hourly;
    let mut levels = Vec::with_capacity(PRESSURE_LEVELS_HPA.len() + 1);

    // Near-surface AGL anchor (conditions the log fit). Present in a full
    // Open-Meteo request; best-effort — if any near-surface variable is absent,
    // fall back to pressure levels only (the fit then reports a typed error if it
    // is under-determined, never a panic).
    if let (Ok(t2m), Ok(ws10), Ok(wd10)) = (
        hourly_at(hourly, "temperature_2m", hour_index),
        hourly_at(hourly, "wind_speed_10m", hour_index),
        hourly_at(hourly, "wind_direction_10m", hour_index),
    ) {
        for (v, what) in [
            (t2m, "temperature_2m"),
            (ws10, "wind_speed_10m"),
            (wd10, "wind_direction_10m"),
        ] {
            if !v.is_finite() {
                return Err(GisError::NonFinite {
                    what: format!("open-meteo {what}"),
                });
            }
        }
        levels.push(Level {
            height_agl_m: NEAR_SURFACE_HEIGHT_M,
            temperature_c: t2m,
            wind_speed_ms: ws10,
            wind_direction_deg: wd10,
        });
    }

    for hpa in PRESSURE_LEVELS_HPA {
        let t = hourly_at(hourly, &format!("temperature_{hpa}hPa"), hour_index)?;
        let ws = hourly_at(hourly, &format!("wind_speed_{hpa}hPa"), hour_index)?;
        let wd = hourly_at(hourly, &format!("wind_direction_{hpa}hPa"), hour_index)?;
        let gph = hourly_at(hourly, &format!("geopotential_height_{hpa}hPa"), hour_index)?;
        let agl = gph - elevation; // AMSL → AGL (Pitfall 5)
        for (v, what) in [
            (t, "temperature"),
            (ws, "wind_speed"),
            (wd, "wind_direction"),
            (agl, "height_agl"),
        ] {
            if !v.is_finite() {
                return Err(GisError::NonFinite {
                    what: format!("open-meteo {what} at {hpa}hPa"),
                });
            }
        }
        levels.push(Level {
            height_agl_m: agl,
            temperature_c: t,
            wind_speed_ms: ws,
            wind_direction_deg: wd,
        });
    }
    // Ascending AGL height (1000 hPa is nearest the ground; guard against an
    // out-of-order response with an explicit sort by finite height).
    levels.sort_by(|a, b| {
        a.height_agl_m
            .partial_cmp(&b.height_agl_m)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(levels)
}

/// The Open-Meteo response fields this crate reads: the site `elevation` (AMSL,
/// m) and the `hourly` object of parallel arrays keyed by variable name.
#[derive(serde::Deserialize)]
struct OpenMeteoResponse {
    #[serde(default)]
    elevation: f64,
    hourly: serde_json::Map<String, serde_json::Value>,
}

/// Pull the numeric value at `index` from the hourly array `key`, or a typed
/// [`GisError::Json`] if the array or element is absent/non-numeric.
fn hourly_at(
    hourly: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    index: usize,
) -> Result<f64, GisError> {
    let arr = hourly
        .get(key)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| GisError::Json {
            message: format!("missing hourly array '{key}'"),
        })?;
    arr.get(index)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| GisError::Json {
            message: format!("hourly '{key}' has no numeric value at index {index}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const C0: f64 = 340.348_380_890_816_03;

    fn synth(a: f64, b: f64, c: f64, z0: f64, heights: &[f64]) -> Vec<f64> {
        heights
            .iter()
            .map(|&z| a * (z / z0 + 1.0).ln() + b * z + c)
            .collect()
    }

    fn default_heights() -> Vec<f64> {
        vec![0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 50.0]
    }

    // ----- fit_profile: the lifted LSQ (single source of truth) -----

    // Round-trip identity (the METX-01 oracle): a profile generated from a known
    // (A, B, C) is recovered to tolerance by the 3×3 LSQ fit.
    #[test]
    fn fit_profile_round_trips_known_coefficients() {
        let z0 = 0.02;
        let heights = default_heights();
        for &(a, b, c) in &[
            (1.0, 0.05, C0),
            (-0.8, -0.04, C0),
            (0.0, 0.0, C0),
            (2.5, 0.1, 335.0),
        ] {
            let samples = synth(a, b, c, z0, &heights);
            let (fa, fb, fc) = fit_profile(&heights, &samples, z0).unwrap();
            assert!((fa - a).abs() < 1e-6, "A: got {fa} want {a}");
            assert!((fb - b).abs() < 1e-6, "B: got {fb} want {b}");
            assert!((fc - c).abs() < 1e-6, "C: got {fc} want {c}");
        }
    }

    // The z₀ used in the fit matters: fitting with the correct z₀ recovers A
    // exactly; a mismatched z₀ perturbs it (proving the log basis is active).
    #[test]
    fn fit_profile_uses_the_log_basis_at_z0() {
        let z0 = 0.01;
        let heights = default_heights();
        let samples = synth(1.5, 0.0, C0, z0, &heights);
        let (a_ok, _, _) = fit_profile(&heights, &samples, z0).unwrap();
        assert!((a_ok - 1.5).abs() < 1e-6);
        let (a_bad, _, _) = fit_profile(&heights, &samples, 0.5).unwrap();
        assert!((a_bad - 1.5).abs() > 1e-3, "wrong z₀ must perturb A");
    }

    // Collinear / insufficient heights ⇒ singular normal matrix ⇒ typed error.
    #[test]
    fn fit_profile_singular_is_typed_error() {
        assert!(matches!(
            fit_profile(&[1.0, 2.0], &[340.0, 341.0], 0.02),
            Err(GisError::WeatherFit { .. })
        ));
        let h = [3.0, 3.0, 3.0, 3.0];
        let y = [340.0, 340.0, 340.0, 340.0];
        assert!(matches!(
            fit_profile(&h, &y, 0.02),
            Err(GisError::WeatherFit { .. })
        ));
    }

    #[test]
    fn fit_profile_rejects_non_finite_samples() {
        let h = default_heights();
        let mut y = synth(1.0, 0.0, C0, 0.02, &h);
        y[2] = f64::NAN;
        assert!(matches!(
            fit_profile(&h, &y, 0.02),
            Err(GisError::NonFinite { .. })
        ));
    }

    // ----- profile_for_bearing: per-azimuth projection (MET-02) -----

    #[test]
    fn weather_profile_downwind_a_exceeds_upwind_a() {
        let base = WeatherComponents {
            a_temp: 0.1,
            a_wind: 0.8,
            b: 0.03,
            c: 340.0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.02,
        };
        let phi_u = 90.0;
        let downwind = profile_for_bearing(&base, phi_u, phi_u);
        let upwind = profile_for_bearing(&base, phi_u + 180.0, phi_u);
        assert!(downwind.a > upwind.a);
        assert!((downwind.a - (base.a_temp + base.a_wind)).abs() < 1e-12);
        assert!((upwind.a - (base.a_temp - base.a_wind)).abs() < 1e-12);
    }

    #[test]
    fn weather_profile_crosswind_is_isotropic_temperature_only() {
        let base = WeatherComponents {
            a_temp: 0.15,
            a_wind: 0.9,
            b: 0.0,
            c: 340.0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.02,
        };
        let p = profile_for_bearing(&base, 90.0 + 90.0, 90.0);
        assert!((p.a - base.a_temp).abs() < 1e-12);
    }

    #[test]
    fn weather_profile_z0_is_clamped_on_construction() {
        let p = WeatherProfile::new(0.5, 0.02, 340.0, 0.0, 0.0, 1e-9);
        assert_eq!(p.z0, Z0_MIN_M);
        let q = WeatherProfile::new(0.5, 0.02, 340.0, 0.0, 0.0, 0.05);
        assert_eq!(q.z0, 0.05);
    }

    // ----- components_from_levels: Open-Meteo → per-azimuth A/B/C (METX-01) -----

    // A wind that strengthens with height, blowing FROM the west (270°) i.e.
    // TOWARD the east (downwind bearing 90°). With phi = 90° the along-wind fit
    // yields a_wind > 0, so downwind A > upwind A.
    fn wind_shear_levels() -> Vec<Level> {
        // Neutral temperature (constant) so the direction test isolates wind.
        [
            (30.0, 15.0, 3.0),
            (120.0, 15.0, 6.0),
            (250.0, 15.0, 8.0),
            (420.0, 15.0, 10.0),
            (650.0, 15.0, 12.0),
            (950.0, 15.0, 14.0),
        ]
        .into_iter()
        .map(|(h, t, ws)| Level {
            height_agl_m: h,
            temperature_c: t,
            wind_speed_ms: ws,
            wind_direction_deg: 270.0, // FROM west ⇒ downwind bearing 90° (east)
        })
        .collect()
    }

    #[test]
    fn components_from_levels_downwind_exceeds_upwind() {
        let levels = wind_shear_levels();
        let phi = 90.0; // downwind (toward east)
        let comp = components_from_levels(&levels, phi, 0.03).unwrap();
        assert!(comp.a_wind > 0.0, "wind shear must give a_wind > 0");
        let downwind = profile_for_bearing(&comp, phi, phi);
        let upwind = profile_for_bearing(&comp, phi + 180.0, phi);
        assert!(
            downwind.a > upwind.a,
            "downwind A {} must exceed upwind A {}",
            downwind.a,
            upwind.a
        );
        // Crosswind ⇒ wind projection zero ⇒ A == a_temp.
        let cross = profile_for_bearing(&comp, phi + 90.0, phi);
        assert!((cross.a - comp.a_temp).abs() < 1e-9);
    }

    // A temperature inversion (T rising with height) makes the temperature-only
    // sound speed rise with height ⇒ downward-refracting: the fitted profile's
    // value at a high level exceeds its value near the ground.
    #[test]
    fn components_from_levels_inversion_refracts_downward() {
        // Strong linear inversion; wind constant so B reflects the temperature.
        let levels: Vec<Level> = [
            (30.0, 8.0),
            (120.0, 10.0),
            (250.0, 12.0),
            (420.0, 14.0),
            (650.0, 16.0),
            (950.0, 18.0),
        ]
        .into_iter()
        .map(|(h, t)| Level {
            height_agl_m: h,
            temperature_c: t,
            wind_speed_ms: 4.0,
            wind_direction_deg: 270.0,
        })
        .collect();
        let comp = components_from_levels(&levels, 90.0, 0.03).unwrap();
        // Evaluate the fitted temperature-only profile c_T(z) = a_temp·ln + b·z + c.
        let eval = |z: f64| comp.a_temp * (z / comp.z0 + 1.0).ln() + comp.b * z + comp.c;
        let low = eval(30.0);
        let high = eval(950.0);
        assert!(
            high > low,
            "inversion ⇒ c_T must increase with height (downward refraction): {low} → {high}"
        );
        // The linear coefficient carries the inversion's monotone rise: B > 0.
        assert!(comp.b > 0.0, "inversion ⇒ B > 0, got {}", comp.b);
    }

    #[test]
    fn components_from_levels_rejects_non_finite() {
        let mut levels = wind_shear_levels();
        levels[2].temperature_c = f64::NAN;
        assert!(matches!(
            components_from_levels(&levels, 90.0, 0.03),
            Err(GisError::NonFinite { .. })
        ));
        let ok = wind_shear_levels();
        assert!(matches!(
            components_from_levels(&ok, f64::INFINITY, 0.03),
            Err(GisError::NonFinite { .. })
        ));
    }

    // ----- sound_speed_profile_for_azimuth: the solver seam -----

    #[test]
    fn sound_speed_profile_carries_projected_a_and_zero_std_devs() {
        let comp = components_from_levels(&wind_shear_levels(), 90.0, 0.03).unwrap();
        let ssp = sound_speed_profile_for_azimuth(&comp, 90.0, 90.0);
        let wp = profile_for_bearing(&comp, 90.0, 90.0);
        assert_eq!(ssp.a, wp.a);
        assert_eq!(ssp.b, comp.b);
        assert_eq!(ssp.c, comp.c);
        assert_eq!(ssp.s_a, 0.0);
        assert_eq!(ssp.s_b, 0.0);
        assert!(ssp.z0 >= Z0_MIN_M);
    }
}
