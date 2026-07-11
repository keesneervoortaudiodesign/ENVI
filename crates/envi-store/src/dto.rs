//! Serde twins of the engine scene vocabulary + settings/met/conditioning DTOs.
//!
//! # Quarantine (D-05, Anti-Pattern 1)
//!
//! Serde lives **HERE**, never in `envi-engine`. Each type below mirrors an
//! `envi_engine::scene` type (see the 06-PATTERNS section-6 table) as a
//! serde-derivable DTO. Conversion into engine space goes through
//! `From`/`TryFrom` that call the engine's *validating constructors*
//! (`BandSpectrum::from_values`, `TerrainProfile::new`) — the engine keeps those
//! fields private precisely so untrusted data cannot bypass validation. This is
//! the seam that keeps the engine's `ndarray + num-complex + thiserror`
//! dependency quarantine byte-identical.
//!
//! # Frozen wire contracts
//!
//! Request-facing DTOs carry `#[serde(deny_unknown_fields)]` so client drift is
//! caught early (06-RESEARCH Security Domain V5). The extensible settings trio
//! ([`MetDto`], [`SettingsDto`], [`ConditioningDto`]) instead use
//! `#[serde(default)]` so new fields can be added without breaking old files.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use envi_engine::forest::ForestCrossing;
use envi_engine::freq::N_BANDS;
use envi_engine::propagation::transmission::IsolationSpectrum;
use envi_engine::scene::{Barrier, Building, GroundSegment, Source, SubSource, TerrainProfile};

use crate::StoreError;
use crate::interpolate::{Resolution, interpolate};

// The identity DTOs `MetDto` + `ReceiverDto` (and the `TryFrom<&ReceiverDto> for
// Receiver` impl) moved into `envi_compute::identity` (Phase 10, 10-01) so the
// tensor hash can consume them WASM-side. Re-exported here so
// `envi_store::dto::{MetDto, ReceiverDto}` stay source-compatible and keep
// generating into the committed `web/src/generated/wire.ts`.
pub use envi_compute::identity::{MetDto, ReceiverDto};

/// Dense per-band sound-power spectrum, keyed by band **INDEX** `0..=104` — never
/// nominal Hz (the SVC-07 wire contract; the 1/12-octave axis is served once at
/// `GET /meta/freq-axis`). Length must equal [`N_BANDS`] (= 105).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct BandSpectrumDto {
    /// The 105 per-band `L_W` values (dB), indexed by band position.
    ///
    /// Generated TS is `number[]` — ts-rs erases the fixed 105 length (Pitfall 5).
    /// That is accepted by design: the length invariant is enforced server-side by
    /// the `BadBandCount` check in [`envi_engine::scene::BandSpectrum`]'s `TryFrom`
    /// (above), so the wire type alone can never forge a valid 105-length spectrum.
    pub band_db: Vec<f64>,
}

impl TryFrom<&BandSpectrumDto> for envi_engine::scene::BandSpectrum {
    type Error = StoreError;

    /// Validate length (== 105) and finiteness, then build via the engine's
    /// `from_values` constructor (the SVC-07 dense-[105]-by-index wire shape).
    fn try_from(d: &BandSpectrumDto) -> Result<Self, StoreError> {
        if d.band_db.len() != N_BANDS {
            return Err(StoreError::BadBandCount {
                got: d.band_db.len(),
            });
        }
        for (i, v) in d.band_db.iter().enumerate() {
            if !v.is_finite() {
                return Err(StoreError::NonFinite {
                    what: format!("band_db[{i}] = {v}"),
                });
            }
        }
        let arr: [f64; N_BANDS] =
            d.band_db
                .as_slice()
                .try_into()
                .map_err(|_| StoreError::BadBandCount {
                    got: d.band_db.len(),
                })?;
        Ok(Self::from_values(arr))
    }
}

/// Serde twin of `envi_engine::scene::SubSource`: a position + its spectrum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct SubSourceDto {
    /// `[x, y, z]` in SceneXY meters (Z-up).
    pub position: [f64; 3],
    /// Per-band sound-power spectrum.
    pub spectrum: BandSpectrumDto,
}

impl TryFrom<&SubSourceDto> for SubSource {
    type Error = StoreError;

    fn try_from(d: &SubSourceDto) -> Result<Self, StoreError> {
        check_finite_3(&d.position, "sub_source.position")?;
        Ok(Self {
            position: d.position,
            spectrum: (&d.spectrum).try_into()?,
        })
    }
}

/// Serde twin of `envi_engine::scene::Source`, plus a stable feature `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct SourceDto {
    /// Stable feature id (doubles as the path-safe key on the wire).
    pub id: Uuid,
    /// The sub-sources composing this source.
    pub sub_sources: Vec<SubSourceDto>,
}

impl TryFrom<&SourceDto> for Source {
    type Error = StoreError;

    fn try_from(d: &SourceDto) -> Result<Self, StoreError> {
        let sub_sources = d
            .sub_sources
            .iter()
            .map(SubSource::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { sub_sources })
    }
}

/// Serde twin of `envi_engine::scene::Barrier`.
///
/// `thickness_m` is load-bearing: JSON `null` (`None`) is a **thin** screen,
/// `Some(t)` a thick screen of thickness `t`. `null != 0.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct BarrierDto {
    /// Polyline of top-edge vertices `[x, y, z]`.
    pub top_edge: Vec<[f64; 3]>,
    /// `None` (JSON `null`) = thin screen; `Some(t)` = thick screen thickness `t` m.
    #[serde(default)]
    pub thickness_m: Option<f64>,
}

impl TryFrom<&BarrierDto> for Barrier {
    type Error = StoreError;

    fn try_from(d: &BarrierDto) -> Result<Self, StoreError> {
        for (i, v) in d.top_edge.iter().enumerate() {
            check_finite_3(v, &format!("barrier.top_edge[{i}]"))?;
        }
        if let Some(t) = d.thickness_m
            && !t.is_finite()
        {
            return Err(StoreError::NonFinite {
                what: format!("barrier.thickness_m = {t}"),
            });
        }
        Ok(Self {
            top_edge: d.top_edge.clone(),
            thickness_m: d.thickness_m,
        })
    }
}

/// Serde twin of `envi_engine::scene::Building`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct BuildingDto {
    /// Footprint polygon vertices `[x, y]` in SceneXY meters.
    pub footprint: Vec<[f64; 2]>,
    /// Eaves height above local ground, meters.
    pub eaves_height_m: f64,
}

impl TryFrom<&BuildingDto> for Building {
    type Error = StoreError;

    fn try_from(d: &BuildingDto) -> Result<Self, StoreError> {
        for (i, v) in d.footprint.iter().enumerate() {
            if !v[0].is_finite() || !v[1].is_finite() {
                return Err(StoreError::NonFinite {
                    what: format!("building.footprint[{i}]"),
                });
            }
        }
        if !d.eaves_height_m.is_finite() {
            return Err(StoreError::NonFinite {
                what: format!("building.eaves_height_m = {}", d.eaves_height_m),
            });
        }
        Ok(Self {
            footprint: d.footprint.clone(),
            eaves_height_m: d.eaves_height_m,
        })
    }
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
    type Error = StoreError;

    fn try_from(d: &TerrainProfileDto) -> Result<Self, StoreError> {
        let segments = d.segments.iter().map(GroundSegment::from).collect();
        TerrainProfile::new(d.points.clone(), segments).map_err(|e| StoreError::Engine {
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
    type Error = StoreError;

    /// Interpolate authored → dense `[105]`, then validate via the engine's
    /// `new()` (rejects NaN/±∞/negative/`> MAX_R_DB`).
    fn try_from(d: &IsolationSpectrumDto) -> Result<Self, StoreError> {
        let dense = interpolate(d.authored.resolution, &d.authored.values)?;
        IsolationSpectrum::new(dense).map_err(|e| StoreError::Engine {
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
    type Error = StoreError;

    /// Validate the authored subset through the engine's `new()` constructor,
    /// supplying a `d_m = 0.0` placeholder (Phase-9 geometry, not authored here).
    fn try_from(d: &ForestParamsDto) -> Result<Self, StoreError> {
        ForestCrossing::new(
            0.0, // d_m placeholder — Phase-9 solve-time geometry, not authored (D-01)
            d.density_per_m2,
            d.stem_radius_m,
            d.absorption.unwrap_or(0.0),
            d.height_m,
        )
        .map_err(|e| StoreError::Engine {
            message: e.to_string(),
        })
    }
}

/// Serde twin of the pinned project CRS (`envi_geo::ProjectCrs`).
///
/// Descriptive persistence form: `utm_zone` + hemisphere + a human-readable
/// `label` (e.g. `"utm-31n"`). Round-trips to a live `ProjectCrs` via
/// [`CrsDto::to_project_crs`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct CrsDto {
    /// UTM zone `1..=60`.
    pub utm_zone: u8,
    /// Southern hemisphere flag.
    pub south: bool,
    /// Human-readable label, e.g. `"utm-31n"`.
    pub label: String,
}

impl From<&envi_geo::ProjectCrs> for CrsDto {
    fn from(crs: &envi_geo::ProjectCrs) -> Self {
        Self {
            utm_zone: crs.utm_zone,
            south: crs.south,
            label: crs.label(),
        }
    }
}

impl CrsDto {
    /// Rebuild a live `ProjectCrs` from the persisted zone + hemisphere.
    ///
    /// # Errors
    /// [`StoreError::Geo`] if the zone is outside `1..=60`.
    pub fn to_project_crs(&self) -> Result<envi_geo::ProjectCrs, StoreError> {
        Ok(envi_geo::ProjectCrs::from_zone(self.utm_zone, self.south)?)
    }
}

/// Project-level settings. Extensible via `#[serde(default)]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct SettingsDto {
    /// Atmosphere readout parameters.
    #[serde(default)]
    pub met: MetDto,
    /// Default ground impedance class letter `A..=H` (default `'D'`).
    #[serde(default = "default_ground_class")]
    pub default_ground_class: char,
}

fn default_ground_class() -> char {
    'D'
}

impl Default for SettingsDto {
    fn default() -> Self {
        Self {
            met: MetDto::default(),
            default_ground_class: default_ground_class(),
        }
    }
}

/// Project metadata persisted to `project.json`.
///
/// Timestamps are unix epoch **seconds** (`u64`) — dependency-free, monotone,
/// and human-inspectable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ProjectMetaDto {
    /// Project id (also the folder name under `projects/`).
    pub id: Uuid,
    /// Human-readable project name.
    pub name: String,
    /// Free-text description.
    pub description: String,
    /// Creation time, unix epoch seconds.
    pub created_at_unix: u64,
    /// Last-modified time, unix epoch seconds.
    pub modified_at_unix: u64,
    /// The CRS pinned at project creation (never re-derived).
    pub crs: CrsDto,
    /// Project settings.
    pub settings: SettingsDto,
}

/// Per-source conditioning: a **READOUT** parameter (gain/delay/filter/mute)
/// applied at level readout via `envi_engine::tensor::compose_gain`.
///
/// It is structurally **excluded from tensor identity** (D-07): conditioning
/// appears nowhere in `envi_engine::solver::SolveJob`, only at readout, so
/// hashing it would make Tier-1 recondition requests self-invalidating. See
/// [`crate::hash::tensor_hash`] — its signature cannot accept this type.
/// Extensible via `#[serde(default)]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct ConditioningDto {
    /// Broadband gain, dB (default 0.0).
    #[serde(default)]
    pub gain_db: f64,
    /// Delay, milliseconds (default 0.0).
    #[serde(default)]
    pub delay_ms: f64,
    /// Optional per-band filter, dB (dense [105] when present).
    #[serde(default)]
    pub filter_band_db: Option<Vec<f64>>,
    /// Mute flag (default false).
    #[serde(default)]
    pub muted: bool,
}

impl Default for ConditioningDto {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            delay_ms: 0.0,
            filter_band_db: None,
            muted: false,
        }
    }
}

/// Reject any NaN/∞ component in a 3-vector before it reaches engine space.
fn check_finite_3(v: &[f64; 3], what: &str) -> Result<(), StoreError> {
    if v.iter().any(|c| !c.is_finite()) {
        return Err(StoreError::NonFinite {
            what: what.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_spectrum(v: f64) -> BandSpectrumDto {
        BandSpectrumDto {
            band_db: vec![v; N_BANDS],
        }
    }

    #[test]
    fn band_spectrum_dto_validates_length() {
        use envi_engine::scene::BandSpectrum;

        // 104 and 106 -> BadBandCount.
        let short = BandSpectrumDto {
            band_db: vec![1.0; 104],
        };
        assert!(
            matches!(
                BandSpectrum::try_from(&short),
                Err(StoreError::BadBandCount { got: 104 })
            ),
            "104 must be rejected"
        );
        let long = BandSpectrumDto {
            band_db: vec![1.0; 106],
        };
        assert!(
            matches!(
                BandSpectrum::try_from(&long),
                Err(StoreError::BadBandCount { got: 106 })
            ),
            "106 must be rejected"
        );

        // Exactly 105 finite values -> Ok, echoed bit-for-bit.
        let mut vals = vec![0.0; N_BANDS];
        for (i, v) in vals.iter_mut().enumerate() {
            *v = i as f64 * 0.5 - 3.0;
        }
        let dto = BandSpectrumDto {
            band_db: vals.clone(),
        };
        let spectrum = BandSpectrum::try_from(&dto).expect("105 finite values must convert");
        for (a, b) in spectrum.as_slice().iter().zip(vals.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "bit-for-bit echo");
        }

        // NaN / Inf -> NonFinite.
        let mut nan = flat_spectrum(0.0);
        nan.band_db[7] = f64::NAN;
        assert!(
            matches!(
                BandSpectrum::try_from(&nan),
                Err(StoreError::NonFinite { .. })
            ),
            "NaN must be rejected"
        );
        let mut inf = flat_spectrum(0.0);
        inf.band_db[64] = f64::INFINITY;
        assert!(
            matches!(
                BandSpectrum::try_from(&inf),
                Err(StoreError::NonFinite { .. })
            ),
            "Inf must be rejected"
        );
    }

    #[test]
    fn barrier_dto_preserves_thin_vs_thick() {
        // null thickness round-trips to None (thin screen).
        let thin_json = r#"{ "top_edge": [[0.0,0.0,3.0],[10.0,0.0,3.0]], "thickness_m": null }"#;
        let thin: BarrierDto = serde_json::from_str(thin_json).expect("parse thin");
        assert_eq!(thin.thickness_m, None, "null -> None (thin)");

        // 0.4 round-trips to Some(0.4) (thick screen).
        let thick_json = r#"{ "top_edge": [[0.0,0.0,3.0],[10.0,0.0,3.0]], "thickness_m": 0.4 }"#;
        let thick: BarrierDto = serde_json::from_str(thick_json).expect("parse thick");
        assert_eq!(thick.thickness_m, Some(0.4), "0.4 -> Some(0.4) (thick)");

        // null != 0.0 is load-bearing.
        assert_ne!(thin.thickness_m, Some(0.0));

        // Conversion preserves the distinction.
        let thin_eng = Barrier::try_from(&thin).expect("thin converts");
        assert_eq!(thin_eng.thickness_m, None);
        let thick_eng = Barrier::try_from(&thick).expect("thick converts");
        assert_eq!(thick_eng.thickness_m, Some(0.4));
    }

    #[test]
    fn terrain_profile_dto_converts_through_validating_constructor() {
        let seg = GroundSegmentDto {
            flow_resistivity: 200.0,
            roughness: 0.0,
        };

        // Non-ascending X -> engine rejection surfaces as StoreError::Engine.
        let bad_x = TerrainProfileDto {
            points: vec![[10.0, 0.0], [5.0, 0.0]],
            segments: vec![seg],
        };
        assert!(
            matches!(
                TerrainProfile::try_from(&bad_x),
                Err(StoreError::Engine { .. })
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
                Err(StoreError::Engine { .. })
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
    fn dto_json_round_trip_is_lossless() {
        let source = SourceDto {
            id: Uuid::from_u128(1),
            sub_sources: vec![SubSourceDto {
                position: [1.0, 2.0, 3.0],
                spectrum: flat_spectrum(90.0),
            }],
        };
        let receiver = ReceiverDto {
            id: Uuid::from_u128(2),
            position: [10.0, 20.0, 1.5],
        };
        let barrier = BarrierDto {
            top_edge: vec![[0.0, 0.0, 4.0], [5.0, 0.0, 4.0]],
            thickness_m: Some(0.3),
        };
        let building = BuildingDto {
            footprint: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            eaves_height_m: 6.0,
        };
        let terrain = TerrainProfileDto {
            points: vec![[0.0, 0.0], [50.0, 1.0]],
            segments: vec![GroundSegmentDto {
                flow_resistivity: 200.0,
                roughness: 0.0,
            }],
        };

        for (label, ok) in [
            (
                "source",
                serde_json::to_string(&source)
                    .map(|s| serde_json::from_str::<SourceDto>(&s).unwrap() == source),
            ),
            (
                "receiver",
                serde_json::to_string(&receiver)
                    .map(|s| serde_json::from_str::<ReceiverDto>(&s).unwrap() == receiver),
            ),
            (
                "barrier",
                serde_json::to_string(&barrier)
                    .map(|s| serde_json::from_str::<BarrierDto>(&s).unwrap() == barrier),
            ),
            (
                "building",
                serde_json::to_string(&building)
                    .map(|s| serde_json::from_str::<BuildingDto>(&s).unwrap() == building),
            ),
            (
                "terrain",
                serde_json::to_string(&terrain)
                    .map(|s| serde_json::from_str::<TerrainProfileDto>(&s).unwrap() == terrain),
            ),
        ] {
            assert!(
                ok.expect("serialize"),
                "{label} DTO must round-trip losslessly"
            );
        }
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
                Err(StoreError::Engine { .. })
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
            Err(StoreError::BadBandCount { got: 8 })
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
            Err(StoreError::NonFinite { .. })
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
                Err(StoreError::Engine { .. })
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
            Err(StoreError::Engine { .. })
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
            Err(StoreError::Engine { .. })
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
    fn met_dto_defaults_apply() {
        let met: MetDto = serde_json::from_str("{}").expect("empty object deserializes");
        assert_eq!(met.temperature_c, 15.0);
        assert_eq!(met.humidity_pct, 70.0);

        // Settings default likewise, including the ground class 'D'.
        let settings: SettingsDto = serde_json::from_str("{}").expect("empty settings");
        assert_eq!(settings.default_ground_class, 'D');
        assert_eq!(settings.met.temperature_c, 15.0);

        // Conditioning defaults: unmuted, zero gain/delay, no filter.
        let cond: ConditioningDto = serde_json::from_str("{}").expect("empty conditioning");
        assert_eq!(cond.gain_db, 0.0);
        assert_eq!(cond.delay_ms, 0.0);
        assert_eq!(cond.filter_band_db, None);
        assert!(!cond.muted);
    }
}
