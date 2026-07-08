//! Nord2000 road-emission coefficient tables (Jonasson source model) — the
//! transcribed-constant layer, provenance-commented per the house rule
//! (`scene::impedance_class` pattern).
//!
//! # Provenance — CITED (verified against the report page images)
//!
//! The per-band rolling/propulsion sound-power coefficients below are transcribed
//! from **H. Jonasson, "Source modelling report" (Nord2000 road / "DK Nord 2005"),
//! rev. 2006-10-17, Annex A, Table A.1** — committed at
//! `docs/references/nord2000-source-modelling-jonasson-rev061017.pdf` (sha256
//! d087db64…) with a verified markdown transcription alongside it. Every array
//! here was checked against the PDF page image (p. 46), per the house rule.
//!
//! **Lineage note (honest, load-bearing):** Table A.1 is the "**DK Nord 2005**"
//! base set (the table header states this) — the Danish lineage the FORCE (DELTA)
//! cases use, so **no Table A.2 correction** (that is Sweden/Norway/Finland only).
//! The report labels these values *intermediate* (§2.3.2/§2.4: "a definite set …
//! expected around December 2006"), so whether they reproduce the FORCE overall
//! levels within Ch.6 tolerance is an **empirical** question — see the FORCE
//! `run_case` arms, which report the measured delta and never force a Pass.
//!
//! The rolling/propulsion **speed laws** are the report's eqs (A.1)/(A.3):
//! - rolling  `L_WR(f) = aR(f) + bR(f)·lg(v / v_ref)`             (logarithmic)
//! - propulsion `L_WP(f) = aP(f) + bP(f)·(v − v_ref)/v_ref`       (LINEAR — A.3)
//!
//! with `v_ref = 70 km/h`. The auxiliary temperature (Table 2.1) and DAC-12
//! surface corrections remain **approximate** placeholders (clearly tagged
//! `[ASSUMED]` below); they are secondary to the cited sound-power tables and are
//! refined separately.

use envi_engine::freq::N_THIRD_OCT;

/// The provenance of the rolling/propulsion sound-power tables in this module.
///
/// Now [`Provenance::Cited`] — Table A.1 is transcribed from the committed report
/// and verified against its page image. A test asserts it. (The auxiliary
/// temperature/surface corrections remain approximate; they are not part of this
/// provenance claim.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Transcribed from the source-modelling report and verified against the page
    /// images (Table A.1).
    Cited,
    /// Plausible placeholder; validated by structure/property tests only.
    Assumed,
}

/// Current provenance of the rolling/propulsion `aR/bR/aP/bP` tables below:
/// `[CITED: Jonasson source-modelling report rev 061017, Table A.1]`.
pub const PROVENANCE: Provenance = Provenance::Cited;

/// Road vehicle category (AV 1171/06 §3.1.4; report Table A.1 columns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoadCategory {
    /// Category 1 — light vehicles (passenger cars, vans).
    Light,
    /// Category 2 — medium-heavy (dual-axle trucks, buses).
    MediumHeavy,
    /// Category 3 — heavy multi-axle vehicles (tabulated for 4 axles, eq. A.2).
    Heavy,
}

/// Reference speed of the rolling/propulsion speed laws (km/h), report eqs
/// (A.1)/(A.3): `v_ref = 70 km/h`.
pub const V_REF_KMH: f64 = 70.0;

/// Rolling-noise energy fraction routed to the **low** sub-source (0.01 m); the
/// remainder goes to the high sub-source. **`[CITED: Kragh Update §2.1;
/// User's Guide §3.1.1]`** — rolling: 80 % low / 20 % high.
pub const ROLLING_LOW_FRACTION: f64 = 0.8;

/// Propulsion-noise energy fraction routed to the **high** sub-source; the
/// remainder goes to the low sub-source. **`[CITED: Kragh Update §2.1]`** —
/// propulsion: 80 % high / 20 % low.
pub const PROPULSION_HIGH_FRACTION: f64 = 0.8;

// ---------------------------------------------------------------------------
// Table A.1 — Basic sound power coefficients (DK Nord 2005, input 051229rev).
// Rows are the 27 one-third-octave nominal centres 25 Hz … 10 kHz. Verified
// against the report page image (p. 46). aR/aP in dB re 1 pW; bR/bP the speed
// slopes for eqs (A.1)/(A.3).
// ---------------------------------------------------------------------------

/// `[CITED]` rolling `aR(f)` — Category 1 (light).
const A_ROLLING_CAT1: [f64; N_THIRD_OCT] = [
    69.9, 69.9, 69.9, 74.9, 74.9, 74.9, 79.3, 82.5, 81.3, 80.9, 78.9, 78.8, 80.5, 87.0, 88.7, 90.8,
    93.3, 92.5, 92.8, 90.4, 88.4, 85.6, 82.7, 79.7, 75.6, 72.0, 67.5,
];
/// `[CITED]` rolling `aR(f)` — Category 2 (medium-heavy).
const A_ROLLING_CAT2: [f64; N_THIRD_OCT] = [
    76.5, 76.5, 76.5, 78.5, 79.5, 79.5, 82.5, 84.3, 84.3, 84.3, 87.4, 88.2, 92.0, 94.1, 96.5, 96.8,
    95.6, 93.0, 93.9, 91.5, 88.1, 86.1, 84.2, 80.3, 77.3, 77.3, 77.3,
];
/// `[CITED]` rolling `aR(f)` — Category 3 (heavy, 4 axles).
const A_ROLLING_CAT3: [f64; N_THIRD_OCT] = [
    79.5, 79.5, 79.5, 81.5, 82.5, 82.5, 85.5, 87.3, 87.3, 87.3, 90.4, 91.2, 95.0, 97.1, 99.5, 99.8,
    98.6, 96.0, 96.9, 94.5, 91.1, 89.1, 87.2, 83.3, 80.3, 80.3, 80.3,
];

/// `[CITED]` rolling slope `bR(f)` — identical across categories in Table A.1.
const B_ROLLING: [f64; N_THIRD_OCT] = [
    33.0, 33.0, 33.0, 30.0, 30.0, 30.0, 41.0, 41.2, 42.3, 41.8, 38.6, 35.5, 31.7, 25.9, 26.5, 32.5,
    37.7, 41.4, 41.6, 42.3, 38.9, 39.5, 39.6, 39.8, 40.2, 40.8, 41.0,
];

/// `[CITED]` propulsion `aP(f)` — Category 1 (light).
const A_PROPULSION_CAT1: [f64; N_THIRD_OCT] = [
    89.8, 91.6, 91.5, 92.5, 96.6, 94.2, 92.0, 87.4, 86.1, 86.1, 87.2, 86.5, 85.6, 80.6, 80.7, 78.8,
    79.3, 82.4, 83.7, 83.4, 81.3, 81.8, 79.9, 77.9, 75.1, 73.1, 69.5,
];
/// `[CITED]` propulsion `aP(f)` — Category 2 (medium-heavy).
const A_PROPULSION_CAT2: [f64; N_THIRD_OCT] = [
    97.0, 97.7, 98.5, 98.5, 101.5, 101.4, 97.0, 96.5, 95.2, 99.6, 100.7, 101.0, 98.3, 94.2, 92.4,
    93.4, 95.5, 96.0, 93.8, 93.4, 92.1, 90.1, 87.9, 85.6, 85.7, 82.6, 79.5,
];
/// `[CITED]` propulsion `aP(f)` — Category 3 (heavy).
const A_PROPULSION_CAT3: [f64; N_THIRD_OCT] = [
    97.7, 97.3, 98.2, 103.3, 107.9, 105.4, 101.0, 101.0, 101.3, 101.3, 102.5, 103.0, 102.0, 101.4,
    99.4, 95.1, 95.8, 95.3, 92.2, 93.2, 90.7, 88.8, 87.5, 85.9, 86.9, 83.8, 80.3,
];

/// `[CITED]` propulsion slope `bP(f)` — Category 1.
const B_PROPULSION_CAT1: [f64; N_THIRD_OCT] = [
    2.0, 2.0, 0.0, 0.0, 2.0, 2.0, 4.0, 2.0, 2.0, 6.0, 8.2, 8.2, 8.2, 8.2, 8.2, 8.2, 8.2, 8.2, 8.2,
    9.5, 9.5, 9.5, 9.5, 9.5, 9.5, 9.5, 9.5,
];
/// `[CITED]` propulsion slope `bP(f)` — Category 2.
const B_PROPULSION_CAT2: [f64; N_THIRD_OCT] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 8.5, 8.5, 8.5, 8.5, 8.5, 12.5, 12.5, 12.5,
    12.5, 12.5, 12.5, 12.5, 12.5, 12.5, 8.5, 8.5, 8.5,
];
/// `[CITED]` propulsion slope `bP(f)` — Category 3.
const B_PROPULSION_CAT3: [f64; N_THIRD_OCT] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5,
    8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5,
];

/// Rolling `(aR, bR)` table for a category (Table A.1).
const fn rolling_table(
    cat: RoadCategory,
) -> (&'static [f64; N_THIRD_OCT], &'static [f64; N_THIRD_OCT]) {
    let a = match cat {
        RoadCategory::Light => &A_ROLLING_CAT1,
        RoadCategory::MediumHeavy => &A_ROLLING_CAT2,
        RoadCategory::Heavy => &A_ROLLING_CAT3,
    };
    (a, &B_ROLLING)
}

/// Propulsion `(aP, bP)` table for a category (Table A.1).
const fn propulsion_table(
    cat: RoadCategory,
) -> (&'static [f64; N_THIRD_OCT], &'static [f64; N_THIRD_OCT]) {
    match cat {
        RoadCategory::Light => (&A_PROPULSION_CAT1, &B_PROPULSION_CAT1),
        RoadCategory::MediumHeavy => (&A_PROPULSION_CAT2, &B_PROPULSION_CAT2),
        RoadCategory::Heavy => (&A_PROPULSION_CAT3, &B_PROPULSION_CAT3),
    }
}

/// `[ASSUMED A8]` DAC-12 road-surface correction (dB per 1/3-octave), added to
/// rolling noise. The FORCE reference surface is the DK average of DAC 11 /
/// SMA 11; DAC 12 differs by an aggregate-size curve (AV 1171/06 §3.1.7
/// Figure 4). Small mid-frequency-weighted placeholder until transcribed —
/// secondary to the cited Table A.1 sound-power values.
const SURFACE_DAC12_CORRECTION: [f64; N_THIRD_OCT] = [
    0.0, 0.0, 0.0, 0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4, 0.5, 0.5, 0.5, 0.4, 0.4, 0.3, 0.3, 0.2,
    0.2, 0.1, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0,
];

/// `[ASSUMED]` rolling-noise temperature coefficient (dB/°C), reference 20 °C.
/// K ≈ 0.1 dB/°C for Cat-1 DAC, halved for the heavier categories (report
/// Table 2.1 not yet transcribed — approximate auxiliary correction).
const fn temperature_coeff_db_per_c(cat: RoadCategory) -> f64 {
    match cat {
        RoadCategory::Light => 0.1,
        RoadCategory::MediumHeavy | RoadCategory::Heavy => 0.05,
    }
}

/// Reference air temperature for the rolling-noise correction, °C
/// (`[ASSUMED]`, report structure).
const TEMPERATURE_REF_C: f64 = 20.0;

/// Rolling sound-power spectrum `L_W,R(f)` for a category at a speed, dB per
/// 1/3-octave: report eq. (A.1) `aR(f) + bR(f)·lg(v/v_ref)` + the approximate
/// temperature correction.
///
/// The road-surface correction is applied separately by the emission layer (see
/// [`surface_dac12_correction`]) so the surface is a caller choice. `speed_kmh`
/// is guarded for the logarithm; `temperature_c` applies the rolling
/// temperature correction. Never panics on input.
#[must_use]
pub fn rolling_lw(cat: RoadCategory, speed_kmh: f64, temperature_c: f64) -> [f64; N_THIRD_OCT] {
    let (a_r, b_r) = rolling_table(cat);
    let lg = speed_ratio_lg(speed_kmh);
    let temp_term = -temperature_coeff_db_per_c(cat) * (temperature_c - TEMPERATURE_REF_C);
    std::array::from_fn(|i| a_r[i] + b_r[i] * lg + temp_term)
}

/// The `[ASSUMED A8]` DAC-12 road-surface correction (dB per 1/3-octave) added
/// to rolling noise on the FORCE reference surface.
#[must_use]
pub fn surface_dac12_correction() -> [f64; N_THIRD_OCT] {
    SURFACE_DAC12_CORRECTION
}

/// Propulsion sound-power spectrum `L_W,P(f)` for a category at a speed, dB per
/// 1/3-octave: report eq. (A.3) `aP(f) + bP(f)·(v − v_ref)/v_ref` (LINEAR in the
/// normalized speed excess, NOT logarithmic). Never panics.
#[must_use]
pub fn propulsion_lw(cat: RoadCategory, speed_kmh: f64) -> [f64; N_THIRD_OCT] {
    let (a_p, b_p) = propulsion_table(cat);
    let v = if speed_kmh.is_finite() {
        speed_kmh
    } else {
        V_REF_KMH
    };
    let speed_excess = (v - V_REF_KMH) / V_REF_KMH;
    std::array::from_fn(|i| a_p[i] + b_p[i] * speed_excess)
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
    fn provenance_is_cited_from_the_committed_report() {
        // The rolling/propulsion tables are transcribed from Table A.1 of the
        // committed source-modelling report and verified against the page image.
        assert_eq!(PROVENANCE, Provenance::Cited);
    }

    #[test]
    fn table_a1_anchor_values_match_the_report() {
        // Spot-check verified anchors against the page image (p. 46), by band
        // index (25 Hz = 0, 1 kHz = 16, 10 kHz = 26).
        assert_eq!(A_ROLLING_CAT1[16], 93.3); // Cat1 rolling aR @ 1 kHz
        assert_eq!(B_ROLLING[13], 25.9); // rolling bR @ 500 Hz (all cats)
        assert_eq!(A_ROLLING_CAT3[14], 99.5); // Cat3 rolling aR @ 630 Hz
        assert_eq!(A_PROPULSION_CAT1[0], 89.8); // Cat1 propulsion aP @ 25 Hz
        assert_eq!(B_PROPULSION_CAT1[10], 8.2); // Cat1 propulsion bP @ 250 Hz
        assert_eq!(A_PROPULSION_CAT3[4], 107.9); // Cat3 propulsion aP @ 63 Hz
    }

    #[test]
    fn at_reference_speed_the_speed_terms_vanish() {
        // At v_ref both laws reduce to the base a(f) (lg 1 = 0; excess = 0).
        let r = rolling_lw(RoadCategory::Light, V_REF_KMH, TEMPERATURE_REF_C);
        let p = propulsion_lw(RoadCategory::Light, V_REF_KMH);
        for i in 0..N_THIRD_OCT {
            assert!((r[i] - A_ROLLING_CAT1[i]).abs() < 1e-12);
            assert!((p[i] - A_PROPULSION_CAT1[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn speed_law_raises_rolling_more_than_propulsion() {
        // At 500 Hz rolling bR = 25.9 d/decade, propulsion bP = 8.2; from 70→90
        // km/h rolling rises more.
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
    }

    #[test]
    fn heavier_categories_are_louder_and_temperature_correction_signs_are_right() {
        // Cat-3 rolling exceeds Cat-1 at 500 Hz (97.1 vs 87.0 base).
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
        let p = propulsion_lw(RoadCategory::Light, f64::NAN);
        assert!(p.iter().all(|v| v.is_finite()));
    }
}
