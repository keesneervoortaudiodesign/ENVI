//! ERA5 Obukhov length + wind×stability class-occurrence statistics (METX-02
//! groundwork) — pure derivation only.
//!
//! # Module I/O
//! - **Inputs:** ERA5 single-level hourly fields per hour (`iews`, `inss`,
//!   `ishf`, `2t`, `2d`, `sp`, `sdfor`, and the 10 m wind `u10`/`v10` for the
//!   wind-speed binning).
//! - **Output:** the inverse Obukhov length `1/L` per hour, and — over a period
//!   — a wind-speed × stability **class-occurrence table** (counts + frequencies)
//!   with an `sdfor`-based reliability count. Non-finite / degenerate fields are
//!   a typed [`GisError`], never a panic.
//! - **Invariants (load-bearing):**
//!   1. **Occurrence statistics ONLY** (D-05, GRID-03): this module counts
//!      class occurrences. It does **NOT** map a class to an `(A, B, C)` profile
//!      and does **NOT** combine classes into an energy-weighted `L_den` — both
//!      stay deferred to GRID-03.
//!   2. **No false FORCE pass** (T-09-03-03): the bin edges are `[ASSUMED]`
//!      named consts; only the *counting* + the `1/L` sign are validated against
//!      a committed synthetic fixture, never a numeric weather Pass.
//!   3. **No panic on data** (T-09-03-02): finiteness + degeneracy checks map
//!      bad ERA5 fields to a typed error (mirrors `route3` posture).
//!
//! # Obukhov recipe (`[CITED: confluence.ecmwf.int … ERA5: How to calculate
//! Obukhov Length]`), sign convention
//!
//! From the eastward/northward turbulent surface stress `iews`/`inss`, the
//! instantaneous surface sensible heat flux `ishf`, and the 2 m temperature /
//! dewpoint / surface pressure:
//! - friction velocity `u* = √(|τ|/ρ)` with `|τ| = √(iews²+inss²)` and air
//!   density `ρ = sp/(R_d·T_v)`;
//! - turbulent temperature scale `θ* = −ishf/(ρ·c_p·u*)`;
//! - `1/L = −κ·g·θ*/(T_v·u*²)`, with virtual temperature `T_v` from `2t`/`2d`/`sp`.
//!
//! ECMWF fluxes are **downward-positive**, so a daytime (upward) sensible-heat
//! flux is `ishf < 0` ⇒ `θ* > 0` ⇒ `1/L < 0` (**unstable**); night `ishf > 0`
//! ⇒ `1/L > 0` (**stable**). This is the physically-certain part the fixture
//! test pins; the class **bin edges** are `[ASSUMED]`.

use crate::GisError;

/// Von Kármán constant κ (dimensionless).
pub const KARMAN: f64 = 0.4;
/// Gravitational acceleration `g`, m/s².
pub const GRAVITY_MS2: f64 = 9.81;
/// Specific gas constant for dry air `R_d`, J/(kg·K).
pub const R_DRY_AIR: f64 = 287.05;
/// Specific heat of air at constant pressure `c_p`, J/(kg·K).
pub const C_P_AIR: f64 = 1004.7;
/// Ratio `R_d/R_v ≈ 0.622` for the specific-humidity conversion.
pub const EPSILON_RD_RV: f64 = 0.622;
/// Tetens saturation-vapour-pressure reference `e₀`, Pa (`[CITED: Bolton 1980]`).
pub const TETENS_E0_PA: f64 = 611.2;
/// Tetens coefficient `a` (dimensionless).
pub const TETENS_A: f64 = 17.67;
/// Tetens coefficient `b`, °C.
pub const TETENS_B_C: f64 = 243.5;
/// Kelvin ↔ Celsius offset.
pub const KELVIN_OFFSET: f64 = 273.15;

/// `[ASSUMED]` neutral band: `|1/L| < STABILITY_NEUTRAL_INV_L` ⇒ neutral. The
/// value is an engineering choice (ERA5 → class is not spec-defined, D-05); it
/// is a named const so the reviewer can sanity-check it and only the *counting*
/// is validated (never a numeric weather Pass, T-09-03-03).
pub const STABILITY_NEUTRAL_INV_L: f64 = 0.002;

/// `[ASSUMED]` wind-speed class edges (m/s): `[calm|light|moderate|strong]` at
/// `< 2`, `[2,5)`, `[5,8)`, `≥ 8`. Named const, `[ASSUMED]` (Beaufort-flavoured
/// engineering bins, not a spec value).
pub const WIND_BIN_EDGES_MS: [f64; 3] = [2.0, 5.0, 8.0];

/// Number of wind-speed bins ([`WIND_BIN_EDGES_MS`] has one fewer edge).
pub const N_WIND_BINS: usize = 4;

/// `sdfor` (std-dev of sub-grid orography, m) below this marks an hour as
/// **reliable** (low sub-grid orography ⇒ the surface-layer MO assumptions hold);
/// above it the derived stability is flagged unreliable (`[CITED: ECMWF]`).
pub const SDFOR_RELIABLE_MAX_M: f64 = 50.0;

/// Minimum friction velocity `u*` (m/s) below which the Obukhov length is
/// undefined (calm / zero-stress) — a typed error rather than a divide-by-zero.
const U_STAR_MIN_MS: f64 = 1e-4;

/// One hour of ERA5 single-level fields (SI units, ECMWF short names).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Era5Hour {
    /// Eastward turbulent surface stress `iews`, N/m² (kg·m⁻¹·s⁻²).
    pub iews: f64,
    /// Northward turbulent surface stress `inss`, N/m².
    pub inss: f64,
    /// Instantaneous surface sensible heat flux `ishf`, W/m² (downward-positive).
    pub ishf: f64,
    /// 2 m air temperature `2t`, K.
    pub t2m_k: f64,
    /// 2 m dewpoint temperature `2d`, K.
    pub d2m_k: f64,
    /// Surface pressure `sp`, Pa.
    pub sp_pa: f64,
    /// Std-dev of sub-grid orography `sdfor`, m (reliability gate).
    pub sdfor_m: f64,
    /// Eastward 10 m wind component `u10`, m/s (for wind-speed binning).
    pub u10_ms: f64,
    /// Northward 10 m wind component `v10`, m/s.
    pub v10_ms: f64,
}

/// Virtual temperature `T_v` (K) from `2t`, `2d`, `sp` via Tetens vapour pressure
/// and the specific-humidity conversion.
fn virtual_temperature_k(t2m_k: f64, d2m_k: f64, sp_pa: f64) -> Result<f64, GisError> {
    let td_c = d2m_k - KELVIN_OFFSET;
    // Saturation vapour pressure at the dewpoint = actual vapour pressure e.
    let e = TETENS_E0_PA * (TETENS_A * td_c / (td_c + TETENS_B_C)).exp();
    let denom = sp_pa - (1.0 - EPSILON_RD_RV) * e;
    if !(denom.is_finite() && denom > 0.0) {
        return Err(GisError::Era5Field {
            message: "degenerate surface pressure vs vapour pressure".to_string(),
        });
    }
    let q = EPSILON_RD_RV * e / denom; // specific humidity
    let t_v = t2m_k * (1.0 + 0.608 * q);
    if !(t_v.is_finite() && t_v > 0.0) {
        return Err(GisError::Era5Field {
            message: "non-physical virtual temperature".to_string(),
        });
    }
    Ok(t_v)
}

/// Compute the **inverse** Obukhov length `1/L` (m⁻¹) for one hour (METX-02).
///
/// Returns `1/L`: **negative ⇒ unstable** (daytime), **positive ⇒ stable**
/// (night), `≈ 0` ⇒ neutral — the sign convention pinned by the fixture test.
///
/// # Errors
///
/// [`GisError::NonFinite`] if any used field is non-finite; [`GisError::Era5Field`]
/// if the state is degenerate (zero stress / non-physical density or T_v) — never
/// a panic or a NaN (T-09-03-02).
pub fn obukhov(hour: &Era5Hour) -> Result<f64, GisError> {
    for (v, what) in [
        (hour.iews, "iews"),
        (hour.inss, "inss"),
        (hour.ishf, "ishf"),
        (hour.t2m_k, "2t"),
        (hour.d2m_k, "2d"),
        (hour.sp_pa, "sp"),
    ] {
        if !v.is_finite() {
            return Err(GisError::NonFinite {
                what: format!("ERA5 field {what}"),
            });
        }
    }
    let t_v = virtual_temperature_k(hour.t2m_k, hour.d2m_k, hour.sp_pa)?;
    let rho = hour.sp_pa / (R_DRY_AIR * t_v); // ideal-gas air density
    if !(rho.is_finite() && rho > 0.0) {
        return Err(GisError::Era5Field {
            message: "non-physical air density".to_string(),
        });
    }
    let tau_mag = hour.iews.hypot(hour.inss); // |τ| = √(iews²+inss²)
    let u_star = (tau_mag / rho).sqrt();
    if !(u_star.is_finite() && u_star >= U_STAR_MIN_MS) {
        return Err(GisError::Era5Field {
            message: "friction velocity below the calm threshold (1/L undefined)".to_string(),
        });
    }
    // θ* = −ishf/(ρ·c_p·u*); 1/L = −κ·g·θ*/(T_v·u*²). Downward-positive ishf ⇒
    // daytime ishf < 0 ⇒ θ* > 0 ⇒ 1/L < 0 (unstable).
    let theta_star = -hour.ishf / (rho * C_P_AIR * u_star);
    let inv_l = -KARMAN * GRAVITY_MS2 * theta_star / (t_v * u_star * u_star);
    if !inv_l.is_finite() {
        return Err(GisError::Era5Field {
            message: "non-finite inverse Obukhov length".to_string(),
        });
    }
    Ok(inv_l)
}

/// Stability class from the inverse Obukhov length (`[ASSUMED]` neutral band).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stability {
    /// `1/L ≤ −STABILITY_NEUTRAL_INV_L` — unstable (daytime).
    Unstable = 0,
    /// `|1/L| < STABILITY_NEUTRAL_INV_L` — neutral.
    Neutral = 1,
    /// `1/L ≥ STABILITY_NEUTRAL_INV_L` — stable (night).
    Stable = 2,
}

/// Number of stability classes.
pub const N_STABILITY: usize = 3;

impl Stability {
    /// Classify an inverse Obukhov length into the `[ASSUMED]` neutral/unstable/
    /// stable band.
    #[must_use]
    pub fn classify(inv_l: f64) -> Self {
        if inv_l <= -STABILITY_NEUTRAL_INV_L {
            Stability::Unstable
        } else if inv_l >= STABILITY_NEUTRAL_INV_L {
            Stability::Stable
        } else {
            Stability::Neutral
        }
    }
}

/// The wind-speed bin index (`0..N_WIND_BINS`) for a 10 m wind speed, per the
/// `[ASSUMED]` [`WIND_BIN_EDGES_MS`].
#[must_use]
pub fn wind_bin(speed_ms: f64) -> usize {
    let mut bin = 0;
    for &edge in &WIND_BIN_EDGES_MS {
        if speed_ms >= edge {
            bin += 1;
        }
    }
    bin
}

/// A wind-speed × stability **class-occurrence table** over a period (METX-02,
/// D-05). Counts only — no class → `(A, B, C)` mapping, no `L_den` combination
/// (both deferred to GRID-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassOccurrence {
    /// `counts[wind_bin][stability]` — hours falling in each class.
    pub counts: [[u32; N_STABILITY]; N_WIND_BINS],
    /// Total hours counted.
    pub total: u32,
    /// Hours flagged reliable (`sdfor < SDFOR_RELIABLE_MAX_M`).
    pub reliable: u32,
}

impl ClassOccurrence {
    /// Frequency (fraction of the period, `0..=1`) in one class; `0` when the
    /// period is empty.
    #[must_use]
    pub fn frequency(&self, wind_bin: usize, stability: Stability) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        f64::from(self.counts[wind_bin][stability as usize]) / f64::from(self.total)
    }

    /// Fraction of the period flagged reliable (low sub-grid orography); `0` when
    /// the period is empty.
    #[must_use]
    pub fn reliable_fraction(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        f64::from(self.reliable) / f64::from(self.total)
    }
}

/// Bin every hour into the wind-speed × stability class-occurrence table
/// (METX-02 derivation — occurrence statistics ONLY, D-05).
///
/// Each hour's `1/L` comes from [`obukhov`] and its wind speed from
/// `√(u10²+v10²)`; the hour increments `counts[wind_bin][stability]`, `total`,
/// and (if `sdfor < SDFOR_RELIABLE_MAX_M`) `reliable`.
///
/// # Errors
///
/// Propagates [`obukhov`]'s typed error for a non-finite / degenerate hour, or
/// [`GisError::NonFinite`] if a 10 m wind component is non-finite — never a panic.
pub fn occurrence_stats(hours: &[Era5Hour]) -> Result<ClassOccurrence, GisError> {
    let mut occ = ClassOccurrence {
        counts: [[0; N_STABILITY]; N_WIND_BINS],
        total: 0,
        reliable: 0,
    };
    for hour in hours {
        for (v, what) in [(hour.u10_ms, "u10"), (hour.v10_ms, "v10")] {
            if !v.is_finite() {
                return Err(GisError::NonFinite {
                    what: format!("ERA5 field {what}"),
                });
            }
        }
        let inv_l = obukhov(hour)?;
        let speed = hour.u10_ms.hypot(hour.v10_ms);
        let bin = wind_bin(speed);
        let stab = Stability::classify(inv_l);
        occ.counts[bin][stab as usize] += 1;
        occ.total += 1;
        if hour.sdfor_m.is_finite() && hour.sdfor_m < SDFOR_RELIABLE_MAX_M {
            occ.reliable += 1;
        }
    }
    Ok(occ)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_hour() -> Era5Hour {
        Era5Hour {
            iews: -0.15,
            inss: 0.05,
            ishf: -120.0, // daytime upward flux (downward-positive ⇒ negative)
            t2m_k: 293.15,
            d2m_k: 283.15,
            sp_pa: 101_325.0,
            sdfor_m: 5.0,
            u10_ms: -4.0,
            v10_ms: 2.0,
        }
    }

    #[test]
    fn daytime_unstable_night_stable_sign() {
        let day = obukhov(&base_hour()).unwrap();
        assert!(
            day < 0.0,
            "daytime (ishf<0) ⇒ 1/L < 0 (unstable), got {day}"
        );

        let mut night = base_hour();
        night.ishf = 60.0; // downward flux ⇒ stable
        let n = obukhov(&night).unwrap();
        assert!(n > 0.0, "night (ishf>0) ⇒ 1/L > 0 (stable), got {n}");

        let mut neutral = base_hour();
        neutral.ishf = 0.0; // no heat flux ⇒ neutral
        let z = obukhov(&neutral).unwrap();
        assert!(
            z.abs() < STABILITY_NEUTRAL_INV_L,
            "no flux ⇒ neutral, got {z}"
        );
    }

    #[test]
    fn obukhov_rejects_non_finite_and_calm() {
        let mut nan = base_hour();
        nan.ishf = f64::NAN;
        assert!(matches!(obukhov(&nan), Err(GisError::NonFinite { .. })));

        // Zero stress ⇒ u* below the calm threshold ⇒ typed error, not NaN.
        let mut calm = base_hour();
        calm.iews = 0.0;
        calm.inss = 0.0;
        assert!(matches!(obukhov(&calm), Err(GisError::Era5Field { .. })));
    }

    #[test]
    fn wind_bins_and_stability_classify() {
        assert_eq!(wind_bin(1.5), 0);
        assert_eq!(wind_bin(2.0), 1);
        assert_eq!(wind_bin(4.9), 1);
        assert_eq!(wind_bin(5.0), 2);
        assert_eq!(wind_bin(8.0), 3);
        assert_eq!(wind_bin(20.0), 3);

        assert_eq!(Stability::classify(-0.03), Stability::Unstable);
        assert_eq!(Stability::classify(0.0), Stability::Neutral);
        assert_eq!(Stability::classify(0.001), Stability::Neutral);
        assert_eq!(Stability::classify(0.03), Stability::Stable);
    }

    #[test]
    fn occurrence_counts_and_reliability() {
        // Two unstable moderate-wind hours + one stable calm hour + one
        // unreliable (high sdfor) hour.
        let h_unstable = base_hour(); // ishf -120, spd √20≈4.47 (bin 1), unstable
        let mut h_unstable2 = base_hour();
        h_unstable2.u10_ms = 0.0;
        h_unstable2.v10_ms = 4.0; // spd 4.0, bin 1, unstable
        let mut h_stable = base_hour();
        h_stable.ishf = 80.0;
        h_stable.u10_ms = 1.0;
        h_stable.v10_ms = 1.0; // spd √2≈1.41, bin 0, stable
        let mut h_unreliable = base_hour();
        h_unreliable.sdfor_m = 120.0; // high sub-grid orography

        let occ = occurrence_stats(&[h_unstable, h_unstable2, h_stable, h_unreliable]).unwrap();
        assert_eq!(occ.total, 4);
        assert_eq!(occ.counts[1][Stability::Unstable as usize], 3); // 3 moderate-wind unstable
        assert_eq!(occ.counts[0][Stability::Stable as usize], 1); // 1 calm stable
        assert_eq!(occ.reliable, 3, "the sdfor=120 m hour is unreliable");
        assert!((occ.reliable_fraction() - 0.75).abs() < 1e-12);
        assert!((occ.frequency(1, Stability::Unstable) - 0.75).abs() < 1e-12);
    }

    #[test]
    fn occurrence_rejects_non_finite_wind() {
        let mut bad = base_hour();
        bad.u10_ms = f64::INFINITY;
        assert!(matches!(
            occurrence_stats(&[bad]),
            Err(GisError::NonFinite { .. })
        ));
    }
}
