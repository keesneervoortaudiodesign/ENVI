//! The reviewed WorldCover → Nord2000 impedance-class table (SC3).
//!
//! # Module I/O
//! - **Inputs:** an ESA WorldCover v200 class code (`u8`).
//! - **Output:** the reviewed Nord2000 ground-impedance **class letter** for that
//!   land cover ([`worldcover_to_class`]); the flow resistivity σ is resolved
//!   downstream through [`envi_engine::scene::impedance_class`] — **never**
//!   restated here (one source of truth for σ).
//! - **Invariant (load-bearing, SC3):** [`WORLDCOVER_TABLE`] maps **every one** of
//!   the 11 WorldCover classes to a valid engine class letter (`A..=H`). The
//!   per-row unit test resolves σ through the engine and asserts it exists — the
//!   numbers live once, in the engine (`scene.rs:329`). Roughness is not derivable
//!   from land cover, so imported `ground_zone`s default to roughness class **N**
//!   (surfaced in the table review).
//!
//! Provenance: values are research-owned and user-reviewed (08-RESEARCH
//! §WorldCover → Nordtest σ / Impedance Mapping); the review IS the SC3 mechanism.
//! The engine's σ ladder (for reference, in this doc comment only — never as a
//! code literal): A=12.5, B=31.5, C=80, D=200, E=500, F=2000, G=20000, H=200000
//! kPa·s/m² (`envi_engine::scene::impedance_class`).

/// One reviewed mapping row: a WorldCover class → a Nord2000 impedance class
/// letter, with the review rationale. σ is intentionally absent — it is resolved
/// through `envi_engine::scene::impedance_class(nord_class)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpedanceRow {
    /// ESA WorldCover v200 raster class code.
    pub wc_code: u8,
    /// Human-readable WorldCover class name.
    pub wc_class: &'static str,
    /// Reviewed Nord2000 impedance class letter (`A..=H`).
    pub nord_class: char,
    /// Why this land cover maps to this class (the SC3 review note).
    pub rationale: &'static str,
}

/// The reviewed WorldCover → Nord2000 impedance table — all 11 WorldCover v200
/// classes (08-RESEARCH §WorldCover mapping, user-reviewed). σ is never stated
/// here; resolve it via `envi_engine::scene::impedance_class(nord_class)`.
pub const WORLDCOVER_TABLE: [ImpedanceRow; 11] = [
    ImpedanceRow {
        wc_code: 10,
        wc_class: "Tree cover",
        nord_class: 'B',
        rationale: "Soft forest floor — B's literal descriptor",
    },
    ImpedanceRow {
        wc_code: 20,
        wc_class: "Shrubland",
        nord_class: 'C',
        rationale: "Uncompacted loose vegetated ground",
    },
    ImpedanceRow {
        wc_code: 30,
        wc_class: "Grassland",
        nord_class: 'D',
        rationale: "Grassland includes pasture — D's pasture-field character (user-reviewed)",
    },
    ImpedanceRow {
        wc_code: 40,
        wc_class: "Cropland",
        nord_class: 'D',
        rationale: "Tilled / normal uncompacted ground",
    },
    ImpedanceRow {
        wc_code: 50,
        wc_class: "Built-up",
        nord_class: 'G',
        rationale: "Predominantly sealed surfaces; conservative for noise (hard = louder)",
    },
    ImpedanceRow {
        wc_code: 60,
        wc_class: "Bare / sparse vegetation",
        nord_class: 'E',
        rationale: "Compacted bare-field / gravel character",
    },
    ImpedanceRow {
        wc_code: 70,
        wc_class: "Snow and ice",
        nord_class: 'A',
        rationale: "Snow — A's literal descriptor",
    },
    ImpedanceRow {
        wc_code: 80,
        wc_class: "Permanent water bodies",
        nord_class: 'H',
        rationale: "Water is acoustically hard — H's descriptor names water",
    },
    ImpedanceRow {
        wc_code: 90,
        wc_class: "Herbaceous wetland",
        nord_class: 'B',
        rationale: "Saturated soft vegetated ground",
    },
    ImpedanceRow {
        wc_code: 95,
        wc_class: "Mangroves",
        nord_class: 'B',
        rationale: "Wet forest-floor analog",
    },
    ImpedanceRow {
        wc_code: 100,
        wc_class: "Moss and lichen",
        nord_class: 'A',
        rationale: "Moss-like — A's literal descriptor",
    },
];

/// The roughness class imported land-cover zones default to. Land cover does not
/// determine surface roughness, so every imported `ground_zone` starts at `'N'`
/// (nil roughness), to be corrected by the user (08-RESEARCH SC3 note).
pub const DEFAULT_ROUGHNESS_CLASS: char = 'N';

/// Reviewed Nord2000 impedance class letter for a WorldCover class `code`, or
/// `None` for an unknown code (never a silent default — the caller falls back to
/// the project default and flags "no data").
#[must_use]
pub fn worldcover_to_class(code: u8) -> Option<char> {
    WORLDCOVER_TABLE
        .iter()
        .find(|row| row.wc_code == code)
        .map(|row| row.nord_class)
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::scene::impedance_class;

    #[test]
    fn table_covers_all_eleven_worldcover_classes_with_unique_codes() {
        assert_eq!(
            WORLDCOVER_TABLE.len(),
            11,
            "all 11 WorldCover classes mapped"
        );
        // Codes are unique.
        for (i, a) in WORLDCOVER_TABLE.iter().enumerate() {
            for b in &WORLDCOVER_TABLE[i + 1..] {
                assert_ne!(a.wc_code, b.wc_code, "duplicate WC code {}", a.wc_code);
            }
        }
    }

    #[test]
    fn every_row_resolves_sigma_through_the_engine_never_restated() {
        // The one-source-of-truth contract: σ is pulled from the engine for every
        // row's letter and must exist and be a valid resistivity. No σ literal is
        // stated in this crate.
        for row in &WORLDCOVER_TABLE {
            assert!(
                ('A'..='H').contains(&row.nord_class),
                "WC {} maps to a valid engine letter, got {:?}",
                row.wc_code,
                row.nord_class
            );
            let sigma = impedance_class(row.nord_class).unwrap_or_else(|| {
                panic!(
                    "WC {} → class {} must resolve through envi_engine::scene::impedance_class",
                    row.wc_code, row.nord_class
                )
            });
            assert!(
                sigma.is_finite() && sigma > 0.0,
                "engine σ for {} must be a positive finite resistivity, got {sigma}",
                row.nord_class
            );
        }
    }

    #[test]
    fn worldcover_to_class_maps_known_codes_and_rejects_unknown() {
        assert_eq!(worldcover_to_class(80), Some('H')); // water → hard
        assert_eq!(worldcover_to_class(70), Some('A')); // snow
        assert_eq!(worldcover_to_class(50), Some('G')); // built-up
        // Every table row round-trips.
        for row in &WORLDCOVER_TABLE {
            assert_eq!(worldcover_to_class(row.wc_code), Some(row.nord_class));
        }
        // Unknown code → None (no silent default).
        assert_eq!(worldcover_to_class(0), None);
        assert_eq!(worldcover_to_class(255), None);
    }
}
