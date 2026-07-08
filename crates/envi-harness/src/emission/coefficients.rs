//! Nord2000 road-emission coefficient tables (Jonasson source model) — the
//! transcribed-constant layer, provenance-commented per the house rule
//! (`scene::impedance_class` pattern).
//!
//! # Provenance quarantine (honest-green, threat T-04-02-01/05)
//!
//! The authoritative per-band rolling/propulsion coefficient tables live in
//! **H. Jonasson, "Acoustic Source Modelling of Nordic Road Vehicles", SP
//! Rapport 2006:12** (EP 1335 ref [1], AV 1171/06 ref [4]). That report is
//! **not freely available** (verified across four acquisition paths,
//! 04-RESEARCH Open Q1). Per the approved honest-green directive (mirroring the
//! 03-03 `[ASSUMED]` weather-constant posture) every coefficient here is
//! `[ASSUMED]`: a plausible, structurally-correct placeholder validated ONLY by
//! structure / property tests, **never presented as verified and never on a
//! FORCE numeric-Pass path**. The `LE − dL` free-field anchor (Task 3) fits the
//! *combined effective* free-field spectrum straight from the `.xls`, so these
//! placeholder tables never feed a numeric FORCE comparison.
//!
//! Wrong-lineage data (Swedish KCB-2015-adjusted tables, Harmonoise/CNOSSOS)
//! is explicitly NOT used — it would corrupt every FORCE case by design
//! (Pitfall 7).
//!
//! If SP 2006:12 is later dropped into `refs/`, replace each `[ASSUMED]` array
//! with the transcribed `[CITED: SP 2006:12 Table X]` values (verified against
//! the PDF page images per the house rule) and flip [`PROVENANCE`].

use envi_engine::freq::N_THIRD_OCT;

/// The provenance of the coefficient tables in this module.
///
/// While SP 2006:12 is unobtainable this is [`Provenance::Assumed`]; a test
/// asserts it, and the honest-green invariant forbids these values on a FORCE
/// numeric-Pass path (T-04-02-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Transcribed from SP 2006:12 and verified against the PDF page images.
    Cited,
    /// Plausible placeholder; validated by structure/property tests only.
    Assumed,
}

/// Current provenance of every table below: `[ASSUMED]` until SP 2006:12 is in
/// hand. **Never** treat an `Assumed` table as a verified FORCE reference.
pub const PROVENANCE: Provenance = Provenance::Assumed;

/// Road vehicle category (AV 1171/06 §3.1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoadCategory {
    /// Category 1 — light vehicles (passenger cars, vans).
    Light,
    /// Category 2 — medium-heavy (dual-axle trucks, buses).
    MediumHeavy,
    /// Category 3 — heavy multi-axle vehicles.
    Heavy,
}

/// Reference speed of the rolling/propulsion speed law `L_W = a + b·lg(v/v_ref)`
/// (km/h). **`[ASSUMED A1]`** — v_ref per the User's Guide structure; verify
/// against SP 2006:12 the moment it is in hand.
pub const V_REF_KMH: f64 = 70.0;

/// Rolling-noise energy fraction routed to the **low** sub-source (0.01 m); the
/// remainder goes to the high sub-source. **`[CITED: Kragh Update §2.1;
/// User's Guide §3.1.1]`** — rolling: 80 % low / 20 % high.
pub const ROLLING_LOW_FRACTION: f64 = 0.8;

/// Propulsion-noise energy fraction routed to the **high** sub-source; the
/// remainder goes to the low sub-source. **`[CITED: Kragh Update §2.1]`** —
/// propulsion: 80 % high / 20 % low.
pub const PROPULSION_HIGH_FRACTION: f64 = 0.8;

/// `[ASSUMED A1]` rolling speed slope `b_R` (dB per decade of v/v_ref), applied
/// uniformly across bands. Rolling noise is strongly speed-dependent
/// (~30·lg v); the per-band variation of `b_R` needs SP 2006:12.
const B_ROLLING: f64 = 30.0;

/// `[ASSUMED A1]` propulsion speed slope `b_P` (dB per decade). Propulsion noise
/// is weakly speed-dependent (~10·lg v).
const B_PROPULSION: f64 = 10.0;

/// `[ASSUMED]` rolling sound-power `a_R` for **Category 1** at `V_REF_KMH`, dB
/// re 1 pW per 1/3-octave (25 Hz … 10 kHz). Smooth A-shaped placeholder peaking
/// near 800–1000 Hz — structural only, NOT SP 2006:12 values.
const A_ROLLING_CAT1: [f64; N_THIRD_OCT] = [
    50.0, 52.0, 54.0, 57.0, 60.0, 63.0, 66.0, 69.0, 72.0, 75.0, 78.0, 80.0, 82.0, 84.0, 85.0, 86.0,
    86.0, 85.0, 84.0, 82.0, 80.0, 77.0, 74.0, 70.0, 66.0, 62.0, 58.0,
];

/// `[ASSUMED]` propulsion sound-power `a_P` for **Category 1** at `V_REF_KMH`,
/// dB per 1/3-octave. Low-frequency-weighted placeholder.
const A_PROPULSION_CAT1: [f64; N_THIRD_OCT] = [
    60.0, 62.0, 64.0, 66.0, 68.0, 69.0, 70.0, 70.0, 69.0, 68.0, 66.0, 64.0, 62.0, 60.0, 58.0, 56.0,
    54.0, 52.0, 50.0, 48.0, 46.0, 44.0, 42.0, 40.0, 38.0, 36.0, 34.0,
];

/// `[ASSUMED]` per-category sound-power offset (dB) added to the Cat-1 rolling
/// and propulsion tables for heavier categories — heavier vehicles are louder,
/// propulsion more so. Placeholder magnitudes only.
const fn category_offset_db(cat: RoadCategory) -> (f64, f64) {
    // (rolling_offset, propulsion_offset)
    match cat {
        RoadCategory::Light => (0.0, 0.0),
        RoadCategory::MediumHeavy => (3.0, 8.0),
        RoadCategory::Heavy => (5.0, 12.0),
    }
}

/// `[ASSUMED A8]` DAC-12 road-surface correction (dB per 1/3-octave), added to
/// rolling noise. The FORCE reference surface is the DK average of DAC 11 /
/// SMA 11; DAC 12 differs by an aggregate-size curve (AV 1171/06 §3.1.7
/// Figure 4). Small mid-frequency-weighted placeholder until transcribed.
const SURFACE_DAC12_CORRECTION: [f64; N_THIRD_OCT] = [
    0.0, 0.0, 0.0, 0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4, 0.5, 0.5, 0.5, 0.4, 0.4, 0.3, 0.3, 0.2,
    0.2, 0.1, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0,
];

/// `[ASSUMED]` rolling-noise temperature coefficient (dB/°C), reference 20 °C.
/// K ≈ 0.1 dB/°C for Cat-1 DAC, halved for the heavier categories.
const fn temperature_coeff_db_per_c(cat: RoadCategory) -> f64 {
    match cat {
        RoadCategory::Light => 0.1,
        RoadCategory::MediumHeavy | RoadCategory::Heavy => 0.05,
    }
}

/// Reference air temperature for the rolling-noise correction, °C
/// (`[ASSUMED]`, User's Guide structure).
const TEMPERATURE_REF_C: f64 = 20.0;

/// Rolling sound-power spectrum `L_W,R(f)` for a category at a speed, dB per
/// 1/3-octave (`[ASSUMED]`, speed law `a_R + b_R·lg(v/v_ref)` + temperature).
///
/// The road-surface correction is applied separately by the emission layer (see
/// [`surface_dac12_correction`]) so the surface is a caller choice. `speed_kmh`
/// is guarded for the logarithm; `temperature_c` applies the rolling
/// temperature correction. Never panics on input.
#[must_use]
pub fn rolling_lw(cat: RoadCategory, speed_kmh: f64, temperature_c: f64) -> [f64; N_THIRD_OCT] {
    let (roll_off, _) = category_offset_db(cat);
    let speed_term = B_ROLLING * speed_ratio_lg(speed_kmh);
    let temp_term = -temperature_coeff_db_per_c(cat) * (temperature_c - TEMPERATURE_REF_C);
    std::array::from_fn(|i| A_ROLLING_CAT1[i] + roll_off + speed_term + temp_term)
}

/// The `[ASSUMED A8]` DAC-12 road-surface correction (dB per 1/3-octave) added
/// to rolling noise on the FORCE reference surface.
#[must_use]
pub fn surface_dac12_correction() -> [f64; N_THIRD_OCT] {
    SURFACE_DAC12_CORRECTION
}

/// Propulsion sound-power spectrum `L_W,P(f)` for a category at a speed, dB per
/// 1/3-octave (`[ASSUMED]`, speed law `a_P + b_P·lg(v/v_ref)`). Never panics.
#[must_use]
pub fn propulsion_lw(cat: RoadCategory, speed_kmh: f64) -> [f64; N_THIRD_OCT] {
    let (_, prop_off) = category_offset_db(cat);
    let speed_term = B_PROPULSION * speed_ratio_lg(speed_kmh);
    std::array::from_fn(|i| A_PROPULSION_CAT1[i] + prop_off + speed_term)
}

/// `lg(v / v_ref)` with a positive-speed guard (a non-positive or non-finite
/// speed collapses the speed term to `v_ref`, i.e. 0 dB — never a NaN/∞).
fn speed_ratio_lg(speed_kmh: f64) -> f64 {
    if speed_kmh.is_finite() && speed_kmh > 0.0 {
        (speed_kmh / V_REF_KMH).log10()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_is_assumed_until_sp_2006_12_is_in_hand() {
        // Honest-green invariant (T-04-02-05): the tables are ASSUMED, so no
        // value here may be used as a verified FORCE reference. The LE−dL
        // anchor (Task 3) fits the effective spectrum from the .xls instead.
        assert_eq!(PROVENANCE, Provenance::Assumed);
    }

    #[test]
    fn speed_law_raises_rolling_more_than_propulsion() {
        // Rolling is more speed-dependent than propulsion (b_R > b_P).
        let r70 = rolling_lw(RoadCategory::Light, 70.0, 20.0);
        let r90 = rolling_lw(RoadCategory::Light, 90.0, 20.0);
        let p70 = propulsion_lw(RoadCategory::Light, 70.0);
        let p90 = propulsion_lw(RoadCategory::Light, 90.0);
        let d_roll = r90[13] - r70[13];
        let d_prop = p90[13] - p70[13];
        assert!(
            d_roll > d_prop,
            "rolling must rise faster: {d_roll} vs {d_prop}"
        );
        // At v_ref the speed term is exactly 0 dB.
        assert!((r70[13] - rolling_lw(RoadCategory::Light, 70.0, 20.0)[13]).abs() < 1e-12);
    }

    #[test]
    fn heavier_categories_are_louder_and_temperature_correction_signs_are_right() {
        let light = rolling_lw(RoadCategory::Light, 80.0, 20.0)[13];
        let heavy = rolling_lw(RoadCategory::Heavy, 80.0, 20.0)[13];
        assert!(heavy > light, "Cat-3 must exceed Cat-1 rolling power");
        // Colder than the 20 °C reference raises rolling noise (positive K).
        let cold = rolling_lw(RoadCategory::Light, 80.0, 15.0)[13];
        let warm = rolling_lw(RoadCategory::Light, 80.0, 25.0)[13];
        assert!(cold > warm, "colder surface must raise rolling noise");
    }

    #[test]
    fn non_positive_speed_is_guarded_not_a_nan() {
        let r = rolling_lw(RoadCategory::Light, 0.0, 20.0);
        assert!(r.iter().all(|v| v.is_finite()));
        let r = rolling_lw(RoadCategory::Light, f64::NAN, 20.0);
        assert!(r.iter().all(|v| v.is_finite()));
    }
}
