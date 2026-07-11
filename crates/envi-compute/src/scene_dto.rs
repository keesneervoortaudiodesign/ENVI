//! WASM-safe serde + ts-rs scene DTOs the browser solve marshals (10-06).
//!
//! # Why these live here (D-05, Anti-Pattern 1 — factored, not duplicated)
//!
//! These are the scene DTOs the client-side `solve_chunk_range` needs across the
//! WASM boundary: terrain, ground, the authored isolation spectrum, forest
//! params, and the sound-speed (weather) profile. They were MOVED here from
//! `envi-store::dto` (terrain/ground/isolation/forest) and `envi-gis-wasm::dto`
//! (`SoundSpeedProfileDto`) — a pure relocation, no field / serde / `ts(export)`
//! change — exactly as 10-01 factored the tensor-identity closure. `envi-store`
//! and `envi-gis-wasm` re-export them at their original paths so their public
//! APIs stay source-compatible and the committed `wire.ts` is byte-identical.
//!
//! Rationale for factoring rather than adding `envi-store` as a wasm dep:
//! `envi-store` pulls `tempfile` + `envi-geo` (fs / reprojection), which must
//! never enter the browser cdylib; `envi-compute` is already the compute crate's
//! WASM-safe dep. Each type's `TryFrom`/`From` routes through the engine's
//! *validating* constructor (`TerrainProfile::new`, `IsolationSpectrum::new`,
//! `ForestCrossing::new`) — the engine keeps those fields private precisely so
//! untrusted DTO data cannot bypass validation. This keeps the engine's
//! `ndarray + num-complex + thiserror` quarantine byte-identical.
//!
//! # Frozen wire contracts
//!
//! Every request-facing DTO carries `#[serde(deny_unknown_fields)]` so client
//! drift is caught early. Every type derives `ts_rs::TS` with
//! `#[ts(export_to = "wire.ts")]` and is registered in the `wire_no_drift`
//! no-drift test.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use envi_engine::forest::ForestCrossing;
use envi_engine::propagation::transmission::IsolationSpectrum;
use envi_engine::scene::{GroundSegment, TerrainProfile};

use crate::interpolate::{Resolution, interpolate};

/// Typed error for the WASM-safe scene-DTO conversions (mirrors the subset of
/// `envi-store`'s `StoreError` the moved conversions used). `envi-store` maps
/// this into its own `StoreError` via a `From` impl so re-exported call sites
/// stay source-compatible; the browser boundary surfaces it as a typed JsValue.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SceneDtoError {
    /// A spectrum DTO was not exactly [`envi_engine::freq::N_BANDS`] long.
    #[error(
        "band spectrum has {got} values, expected {} (dense [105] by band index)",
        envi_engine::freq::N_BANDS
    )]
    BadBandCount {
        /// The wrong length received.
        got: usize,
    },
    /// A value that must be finite was NaN or infinite.
    #[error("non-finite value: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
    /// An engine validating constructor rejected DTO input (e.g.
    /// `TerrainProfile::new` on a non-ascending profile).
    #[error("engine validation rejected DTO input: {message}")]
    Engine {
        /// The engine's error message.
        message: String,
    },
}

/// Serde twin of `envi_engine::scene::GroundSegment`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct GroundSegmentDto {
    /// Ground flow resistivity, kNs·m⁻⁴ (Nordtest σ).
    pub flow_resistivity: f64,
    /// Terrain roughness, meters.
    pub roughness: f64,
}

impl From<&GroundSegmentDto> for GroundSegment {
    fn from(d: &GroundSegmentDto) -> Self {
        Self {
            flow_resistivity: d.flow_resistivity,
            roughness: d.roughness,
        }
    }
}

/// Serde twin of `envi_engine::scene::TerrainProfile`.
///
/// Conversion is inherently `TryFrom`: the engine's `TerrainProfile::new`
/// validates (non-empty, strictly-ascending X, `N-1` segments, finite) and its
/// fields are private, so untrusted DTO input cannot bypass the check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct TerrainProfileDto {
    /// Profile points `(x, z)`.
    pub points: Vec<[f64; 2]>,
    /// Ground segments (length `points.len() - 1`).
    pub segments: Vec<GroundSegmentDto>,
}

impl TryFrom<&TerrainProfileDto> for TerrainProfile {
    type Error = SceneDtoError;

    fn try_from(d: &TerrainProfileDto) -> Result<Self, SceneDtoError> {
        let segments = d.segments.iter().map(GroundSegment::from).collect();
        TerrainProfile::new(d.points.clone(), segments).map_err(|e| SceneDtoError::Engine {
            message: e.to_string(),
        })
    }
}

/// The authored (coarse) representation of an isolation spectrum (D-06).
///
/// Persists ONLY `{ resolution, values }` — the length of `values` must match
/// the resolution's anchor count (9 octave / 27 third / 105 twelfth). The dense
/// `r_db[105]` grid is **DERIVED on read** via [`crate::interpolate::interpolate`],
/// never a second persisted field: storing both would reintroduce the Phase-6
/// `CalcRecord.tensor_hash` shadow-cache anti-pattern that was deleted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct AuthoredSpectrumDto {
    /// Authoring resolution (how many anchors `values` carries and where they land).
    pub resolution: Resolution,
    /// The authored per-band `R` values (dB) at the resolution's grid anchors.
    pub values: Vec<f64>,
}

/// Serde twin of `envi_engine::propagation::transmission::IsolationSpectrum`
/// (D-01 / D-06): a partition's sound-reduction index `R(f)`.
///
/// Holds only the [`AuthoredSpectrumDto`]; conversion into the engine type first
/// interpolates the authored values onto the dense `[105]` grid (the single
/// shared core, so read-path / endpoint / `PUT /scene` cannot diverge — D-05),
/// then routes through the engine's validating `IsolationSpectrum::new`
/// constructor. The engine's private `r_db` field forces that path, so an
/// out-of-range authored value (`R > MAX_R_DB`, negative, non-finite) is
/// REJECTED, never silently clamped (threat T-07-01-02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct IsolationSpectrumDto {
    /// The authored coarse spectrum (`r_db[105]` is derived on read).
    pub authored: AuthoredSpectrumDto,
}

impl TryFrom<&IsolationSpectrumDto> for IsolationSpectrum {
    type Error = SceneDtoError;

    /// Interpolate authored → dense `[105]`, then validate via the engine's
    /// `new()` (rejects NaN/±∞/negative/`> MAX_R_DB`).
    fn try_from(d: &IsolationSpectrumDto) -> Result<Self, SceneDtoError> {
        let dense = interpolate(d.authored.resolution, &d.authored.values)?;
        IsolationSpectrum::new(dense).map_err(|e| SceneDtoError::Engine {
            message: e.to_string(),
        })
    }
}

/// Serde twin of the **authored subset** of `envi_engine::forest::ForestCrossing`
/// (D-01, SCN-04).
///
/// Authors only `{ density_per_m2, stem_radius_m, height_m, absorption? }`. The
/// engine's `d_m` (through-forest path length) is a Phase-9 solve-time geometry
/// input, NOT authored here — the `TryFrom` supplies a documented `0.0`
/// placeholder purely so the tested conversion proves the authored fields
/// validate through `ForestCrossing::new`. `absorption` defaults to `0.0` when
/// unauthored (the engine edge-clamps to the table domain `[0, 0.4]` at lookup).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ForestParamsDto {
    /// `n″` — mean tree density, m⁻². Zero is a valid persisted value (the UI
    /// marks it a `warn`, not a store rejection); negative is rejected.
    pub density_per_m2: f64,
    /// `a` — mean stem radius, m (must be positive).
    pub stem_radius_m: f64,
    /// `h` — mean tree height, m (must be positive).
    pub height_m: f64,
    /// `α` — mean absorption, `[0, 1]`. Absent ⇒ `0.0` (neutral placeholder).
    #[serde(default)]
    pub absorption: Option<f64>,
}

impl TryFrom<&ForestParamsDto> for ForestCrossing {
    type Error = SceneDtoError;

    /// Validate the authored subset through the engine's `new()` constructor,
    /// supplying a `d_m = 0.0` placeholder (Phase-9 geometry, not authored here).
    fn try_from(d: &ForestParamsDto) -> Result<Self, SceneDtoError> {
        ForestCrossing::new(
            0.0, // d_m placeholder — Phase-9 solve-time geometry, not authored (D-01)
            d.density_per_m2,
            d.stem_radius_m,
            d.absorption.unwrap_or(0.0),
            d.height_m,
        )
        .map_err(|e| SceneDtoError::Engine {
            message: e.to_string(),
        })
    }
}

/// A concrete per-azimuth sound-speed profile (wire mirror of the engine
/// `SoundSpeedProfile` `(A, B, C, sA, sB, z₀)`).
///
/// Produced by the `envi-gis-wasm` weather-derive boundary (result-facing there)
/// and consumed by the solve boundary as the optional refraction input
/// (request-facing here) — one wire type, shared by both wasm crates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct SoundSpeedProfileDto {
    /// Logarithmic coefficient `A`, m/s (wind-projected onto the path azimuth).
    pub a: f64,
    /// Linear coefficient `B`, s⁻¹.
    pub b: f64,
    /// Ground sound speed `C`, m/s.
    pub c: f64,
    /// Std-dev of `A`, m/s (0 for a single-hour profile).
    pub s_a: f64,
    /// Std-dev of `B`, s⁻¹ (0 for a single-hour profile).
    pub s_b: f64,
    /// Roughness length `z₀`, m (clamped ≥ 0.001 m).
    pub z0: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_profile_dto_converts_through_validating_constructor() {
        let seg = GroundSegmentDto {
            flow_resistivity: 200.0,
            roughness: 0.0,
        };

        // Non-ascending X -> engine rejection surfaces as SceneDtoError::Engine.
        let bad_x = TerrainProfileDto {
            points: vec![[10.0, 0.0], [5.0, 0.0]],
            segments: vec![seg],
        };
        assert!(
            matches!(
                TerrainProfile::try_from(&bad_x),
                Err(SceneDtoError::Engine { .. })
            ),
            "non-ascending X must surface as Engine error"
        );

        // Wrong segment count -> engine rejection.
        let bad_count = TerrainProfileDto {
            points: vec![[0.0, 0.0], [10.0, 0.0]],
            segments: vec![seg, seg],
        };
        assert!(
            matches!(
                TerrainProfile::try_from(&bad_count),
                Err(SceneDtoError::Engine { .. })
            ),
            "wrong segment count must surface as Engine error"
        );

        // Valid input -> Ok with points/segments matching.
        let good = TerrainProfileDto {
            points: vec![[0.0, 10.0], [100.0, 20.0]],
            segments: vec![seg],
        };
        let profile = TerrainProfile::try_from(&good).expect("valid profile converts");
        assert_eq!(profile.points(), &[[0.0, 10.0], [100.0, 20.0]]);
        assert_eq!(profile.segments().len(), 1);
        assert_eq!(profile.segments()[0].flow_resistivity, 200.0);
    }

    #[test]
    fn terrain_profile_dto_round_trips_losslessly() {
        let terrain = TerrainProfileDto {
            points: vec![[0.0, 0.0], [50.0, 1.0]],
            segments: vec![GroundSegmentDto {
                flow_resistivity: 200.0,
                roughness: 0.0,
            }],
        };
        let s = serde_json::to_string(&terrain).expect("serialize");
        assert_eq!(
            serde_json::from_str::<TerrainProfileDto>(&s).unwrap(),
            terrain,
            "TerrainProfileDto must round-trip losslessly"
        );
    }

    #[test]
    fn isolation_spectrum_dto_converts_and_matches_interpolation() {
        // Valid octave (9 values, all in [0,1000]) converts; as_bands() matches
        // interpolate(Octave, values) bit-for-bit.
        let values: Vec<f64> = (0..9).map(|k| k as f64 * 5.0 + 10.0).collect();
        let dto = IsolationSpectrumDto {
            authored: AuthoredSpectrumDto {
                resolution: Resolution::Octave,
                values: values.clone(),
            },
        };
        let spectrum = IsolationSpectrum::try_from(&dto).expect("valid octave converts");
        let dense = interpolate(Resolution::Octave, &values).expect("interpolates");
        for (a, b) in spectrum.as_bands().iter().zip(dense.iter()) {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "as_bands matches interpolate bit-for-bit"
            );
        }
    }

    #[test]
    fn isolation_spectrum_dto_rejects_out_of_range_never_clamps() {
        // An authored value of 2000 exceeds MAX_R_DB (1000): the engine new()
        // must REJECT it (Engine error), never a clamped-to-1000 result.
        let mut values = vec![10.0; 9];
        values[4] = 2000.0;
        let dto = IsolationSpectrumDto {
            authored: AuthoredSpectrumDto {
                resolution: Resolution::Octave,
                values,
            },
        };
        assert!(
            matches!(
                IsolationSpectrum::try_from(&dto),
                Err(SceneDtoError::Engine { .. })
            ),
            "R > MAX_R_DB must surface as Engine error, never clamped"
        );

        // Wrong length surfaces from the shared core as BadBandCount.
        let bad_len = IsolationSpectrumDto {
            authored: AuthoredSpectrumDto {
                resolution: Resolution::Octave,
                values: vec![10.0; 8],
            },
        };
        assert!(matches!(
            IsolationSpectrum::try_from(&bad_len),
            Err(SceneDtoError::BadBandCount { got: 8 })
        ));

        // A non-finite authored value is rejected before the engine.
        let nan = IsolationSpectrumDto {
            authored: AuthoredSpectrumDto {
                resolution: Resolution::Octave,
                values: {
                    let mut v = vec![10.0; 9];
                    v[2] = f64::NAN;
                    v
                },
            },
        };
        assert!(matches!(
            IsolationSpectrum::try_from(&nan),
            Err(SceneDtoError::NonFinite { .. })
        ));
    }

    #[test]
    fn forest_params_dto_converts_authored_subset() {
        // density = 0.0 is a valid persisted value (UI warns, store accepts).
        let zero_density = ForestParamsDto {
            density_per_m2: 0.0,
            stem_radius_m: 0.15,
            height_m: 20.0,
            absorption: Some(0.2),
        };
        let crossing =
            ForestCrossing::try_from(&zero_density).expect("zero density is a valid store value");
        assert_eq!(crossing.density_per_m2, 0.0);
        assert_eq!(crossing.stem_radius_m, 0.15);
        assert_eq!(crossing.height_m, 20.0);
        assert_eq!(crossing.absorption, 0.2);
        assert_eq!(crossing.d_m, 0.0, "d_m placeholder (Phase-9 geometry)");

        // Absent absorption defaults to 0.0 (neutral placeholder).
        let no_absorption = ForestParamsDto {
            density_per_m2: 0.05,
            stem_radius_m: 0.1,
            height_m: 15.0,
            absorption: None,
        };
        let crossing = ForestCrossing::try_from(&no_absorption).expect("None absorption converts");
        assert_eq!(crossing.absorption, 0.0);
    }

    #[test]
    fn forest_params_dto_rejects_invalid() {
        // Negative density -> Engine rejection.
        let neg_density = ForestParamsDto {
            density_per_m2: -1.0,
            stem_radius_m: 0.1,
            height_m: 15.0,
            absorption: None,
        };
        assert!(
            matches!(
                ForestCrossing::try_from(&neg_density),
                Err(SceneDtoError::Engine { .. })
            ),
            "negative density must be rejected"
        );

        // Non-positive stem radius -> Engine rejection.
        let bad_radius = ForestParamsDto {
            density_per_m2: 0.05,
            stem_radius_m: 0.0,
            height_m: 15.0,
            absorption: None,
        };
        assert!(matches!(
            ForestCrossing::try_from(&bad_radius),
            Err(SceneDtoError::Engine { .. })
        ));

        // Non-positive height -> Engine rejection.
        let bad_height = ForestParamsDto {
            density_per_m2: 0.05,
            stem_radius_m: 0.1,
            height_m: 0.0,
            absorption: None,
        };
        assert!(matches!(
            ForestCrossing::try_from(&bad_height),
            Err(SceneDtoError::Engine { .. })
        ));
    }

    #[test]
    fn new_dtos_round_trip_and_reject_unknown_fields() {
        // IsolationSpectrumDto JSON round-trip is lossless.
        let iso = IsolationSpectrumDto {
            authored: AuthoredSpectrumDto {
                resolution: Resolution::Third,
                values: (0..27).map(|k| k as f64).collect(),
            },
        };
        let s = serde_json::to_string(&iso).expect("serialize iso");
        assert_eq!(
            serde_json::from_str::<IsolationSpectrumDto>(&s).unwrap(),
            iso,
            "IsolationSpectrumDto round-trips"
        );
        // Resolution renders lowercase in the persisted form.
        assert!(
            s.contains("\"third\""),
            "resolution serialized lowercase: {s}"
        );

        // ForestParamsDto JSON round-trip is lossless.
        let forest = ForestParamsDto {
            density_per_m2: 0.05,
            stem_radius_m: 0.12,
            height_m: 18.0,
            absorption: Some(0.3),
        };
        let s = serde_json::to_string(&forest).expect("serialize forest");
        assert_eq!(
            serde_json::from_str::<ForestParamsDto>(&s).unwrap(),
            forest,
            "ForestParamsDto round-trips"
        );

        // deny_unknown_fields rejects drift.
        let unknown = r#"{ "authored": { "resolution": "octave", "values": [1,2,3,4,5,6,7,8,9], "extra": 1 } }"#;
        assert!(
            serde_json::from_str::<IsolationSpectrumDto>(unknown).is_err(),
            "unknown field must be rejected"
        );
    }

    #[test]
    fn sound_speed_profile_dto_round_trips_and_rejects_unknown_fields() {
        let p = SoundSpeedProfileDto {
            a: 1.5,
            b: -0.02,
            c: 340.0,
            s_a: 0.1,
            s_b: 0.001,
            z0: 0.03,
        };
        let s = serde_json::to_string(&p).expect("serialize");
        assert_eq!(
            serde_json::from_str::<SoundSpeedProfileDto>(&s).unwrap(),
            p,
            "SoundSpeedProfileDto round-trips losslessly"
        );
        let unknown = r#"{ "a":0,"b":0,"c":340,"s_a":0,"s_b":0,"z0":0.03,"extra":1 }"#;
        assert!(
            serde_json::from_str::<SoundSpeedProfileDto>(unknown).is_err(),
            "unknown field must be rejected"
        );
    }
}
