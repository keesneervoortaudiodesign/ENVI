//! Source registry as data (D-04): the pluggable GIS-source table.
//!
//! # Module I/O
//! - **Inputs:** a WGS84 lon/lat query [`Bbox`] (the import viewport), or a
//!   [`SourceId`] string.
//! - **Output:** a [`SourceDescriptor`] selection per layer — terrain (AHN4 DTM
//!   inside its coverage hull, GLO-30 DSM globally), land cover (WorldCover), and
//!   buildings (OSM Overpass) — plus the committed AHN kaartblad index
//!   ([`AHN_INDEX_TOML`]).
//! - **Invariant (load-bearing):** the registry is **pure data**. A new national
//!   DTM slots in later as a `SourceDescriptor` row plus a coverage polygon — no
//!   control-flow change (08-RESEARCH Pattern 3, D-04). Coverage selection is a
//!   deterministic point-in-polygon test on the query bbox center; the exact
//!   kaartblad tile name is resolved from the committed [`AHN_INDEX_TOML`] index
//!   (generated at dev time from the PDOK ATOM feed, no runtime feed dependency).
//!
//! # CORS capability (D-02)
//! Each descriptor carries its verified [`Cors`] mode (08-RESEARCH §Per-Source
//! CORS Capability Map, live-probed): AHN and Overpass are browser-**Direct**;
//! GLO-30 and WorldCover have no `Access-Control-Allow-Origin` and MUST route
//! through the same-origin byte proxy (**Proxy**).

use geo::{Contains, LineString, MultiPolygon, Point, Polygon};
use std::sync::OnceLock;

/// The committed AHN kaartblad tile index (tile name ↔ RD New bbox), embedded at
/// compile time. Generated at dev time from the PDOK ATOM feed and committed with
/// a DO-NOT-EDIT + sha256 provenance banner (08-PATTERNS committed-artifact
/// pattern). Parsed only where a tile-name lookup is needed; the coverage gate
/// uses the WGS84 hull below.
pub const AHN_INDEX_TOML: &str = include_str!("registry/ahn_index.toml");

/// Browser CORS reachability of a source (D-02 capability map).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cors {
    /// Fetched cross-origin directly from the browser (server sends `ACAO`).
    Direct,
    /// No CORS headers — routed through the same-origin allowlisted byte proxy.
    Proxy,
}

/// The GIS data kind a source provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Bare-earth digital terrain model (e.g. AHN4).
    Dtm,
    /// Digital surface model — includes vegetation/roofs (e.g. GLO-30).
    Dsm,
    /// Land-cover raster (e.g. ESA WorldCover).
    Landcover,
    /// Vector building footprints (e.g. OSM Overpass).
    Buildings,
}

/// A stable, human-readable source identifier (`"ahn4-dtm"`, `"glo30"`, ...).
pub type SourceId = &'static str;

/// A WGS84 lon/lat bounding box (min/max **degrees**). The import viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox {
    /// Western edge, degrees east.
    pub min_lon: f64,
    /// Southern edge, degrees north.
    pub min_lat: f64,
    /// Eastern edge, degrees east.
    pub max_lon: f64,
    /// Northern edge, degrees north.
    pub max_lat: f64,
}

impl Bbox {
    /// The bbox center `(lon, lat)`.
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (
            (self.min_lon + self.max_lon) / 2.0,
            (self.min_lat + self.max_lat) / 2.0,
        )
    }
}

/// The geographic reach of a source, in WGS84.
pub enum Coverage {
    /// Available everywhere (e.g. GLO-30, WorldCover, Overpass).
    Global,
    /// A national/regional coverage hull (e.g. the AHN Netherlands hull).
    Region(MultiPolygon<f64>),
}

impl Coverage {
    /// Whether the query `bbox` is served by this coverage — a deterministic
    /// point-in-polygon test on the bbox center (never a silent default).
    #[must_use]
    pub fn covers(&self, bbox: &Bbox) -> bool {
        match self {
            Coverage::Global => true,
            Coverage::Region(mp) => {
                let (lon, lat) = bbox.center();
                mp.contains(&Point::new(lon, lat))
            }
        }
    }
}

/// A single registry row: everything needed to fetch and attribute a source.
///
/// Fields follow 08-RESEARCH Pattern 3. `coverage` gates selection; `crs` is the
/// source's native CRS (the terrain reprojection boundary consumes it); `cors`
/// is the verified D-02 reachability; `license`/`attribution` feed the D-11
/// provenance stamp and the SC5 map attribution.
pub struct SourceDescriptor {
    /// Stable identifier (`"ahn4-dtm"`, `"glo30"`, `"worldcover"`, `"osm-overpass"`).
    pub id: SourceId,
    /// The data kind this source provides.
    pub kind: SourceKind,
    /// Geographic reach in WGS84.
    pub coverage: Coverage,
    /// Native CRS EPSG label (`"EPSG:28992"` RD New, `"EPSG:4326"` WGS84).
    pub crs: &'static str,
    /// How tiles/queries are addressed (`"kaartblad"`, `"1deg"`, `"bbox-query"`).
    pub tile_scheme: &'static str,
    /// The fetch URL template (TS fills the `{...}` slots; this crate never fetches).
    pub endpoint_template: &'static str,
    /// Verified browser CORS reachability (D-02).
    pub cors: Cors,
    /// SPDX-ish license tag for the provenance stamp.
    pub license: &'static str,
    /// Human-readable attribution string (SC5 map credit).
    pub attribution: &'static str,
}

/// The AHN Netherlands coverage hull, WGS84.
///
/// A **coarse** bounding hull of the Dutch mainland + Wadden extent (derived from
/// the ATOM feed's overall georss extent). It is deliberately conservative: the
/// precise per-tile coverage lives in [`AHN_INDEX_TOML`]; this hull only gates
/// "prefer AHN here vs fall back to GLO-30". Vertices trace the national outline
/// closely enough to exclude neighbouring capitals (Brussels/Paris/Berlin/London).
fn nl_coverage() -> MultiPolygon<f64> {
    // (lon, lat) ring, closed, tracing the NL outline coarsely.
    let ring = LineString::from(vec![
        (3.36, 51.30),
        (3.36, 53.62),
        (5.20, 53.62),
        (7.23, 53.30),
        (7.23, 52.00),
        (6.02, 51.75),
        (6.22, 51.36),
        (5.65, 50.75),
        (5.80, 51.05),
        (4.55, 51.42),
        (3.90, 51.20),
        (3.36, 51.30),
    ]);
    MultiPolygon::new(vec![Polygon::new(ring, vec![])])
}

/// Build the static registry table (four sources). Lazily initialised because
/// [`Coverage::Region`] carries a non-`const` [`MultiPolygon`].
fn build_registry() -> Vec<SourceDescriptor> {
    vec![
        SourceDescriptor {
            id: "ahn4-dtm",
            kind: SourceKind::Dtm,
            coverage: Coverage::Region(nl_coverage()),
            crs: "EPSG:28992",
            tile_scheme: "kaartblad",
            endpoint_template: "https://service.pdok.nl/rws/actueel-hoogtebestand-nederland/atom/downloads/dtm_05m/{tile}.tif",
            cors: Cors::Direct,
            license: "CC0-1.0",
            attribution: "AHN (Actueel Hoogtebestand Nederland), PDOK — CC0",
        },
        SourceDescriptor {
            id: "glo30",
            kind: SourceKind::Dsm,
            coverage: Coverage::Global,
            crs: "EPSG:4326",
            tile_scheme: "1deg",
            endpoint_template: "https://copernicus-dem-30m.s3.amazonaws.com/Copernicus_DSM_COG_10_{ns}_00_{ew}_00_DEM/Copernicus_DSM_COG_10_{ns}_00_{ew}_00_DEM.tif",
            cors: Cors::Proxy,
            license: "Copernicus-Free",
            attribution: "Copernicus DEM GLO-30 © ESA / Copernicus (credit required)",
        },
        SourceDescriptor {
            id: "worldcover",
            kind: SourceKind::Landcover,
            coverage: Coverage::Global,
            crs: "EPSG:4326",
            tile_scheme: "3deg",
            endpoint_template: "https://esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/map/ESA_WorldCover_10m_2021_v200_{tile}_Map.tif",
            cors: Cors::Proxy,
            license: "CC-BY-4.0",
            attribution: "ESA WorldCover 2021 v200 — © ESA WorldCover, CC BY 4.0",
        },
        SourceDescriptor {
            id: "osm-overpass",
            kind: SourceKind::Buildings,
            coverage: Coverage::Global,
            crs: "EPSG:4326",
            tile_scheme: "bbox-query",
            endpoint_template: "https://overpass-api.de/api/interpreter",
            cors: Cors::Direct,
            license: "ODbL-1.0",
            attribution: "© OpenStreetMap contributors (ODbL)",
        },
    ]
}

/// The process-wide source registry (built once).
#[must_use]
pub fn registry() -> &'static [SourceDescriptor] {
    static REG: OnceLock<Vec<SourceDescriptor>> = OnceLock::new();
    REG.get_or_init(build_registry)
}

/// Look up a descriptor by its [`SourceId`], or `None` if unknown.
#[must_use]
pub fn source(id: SourceId) -> Option<&'static SourceDescriptor> {
    registry().iter().find(|d| d.id == id)
}

/// The terrain source for `bbox`: AHN4 DTM where its coverage hull applies,
/// GLO-30 DSM everywhere else (D-04 AHN-preferred, GLO-30 fallback).
///
/// # Panics
/// Never in practice — the two terrain descriptors are compile-time table rows.
#[must_use]
pub fn terrain_source(bbox: Bbox) -> &'static SourceDescriptor {
    let ahn = source("ahn4-dtm").expect("ahn4-dtm is a registry row");
    if ahn.coverage.covers(&bbox) {
        ahn
    } else {
        source("glo30").expect("glo30 is a registry row")
    }
}

/// A per-layer import plan for a viewport: which source feeds terrain, land
/// cover, and buildings.
pub struct ImportPlan {
    /// Terrain source (AHN4 or GLO-30 by coverage).
    pub terrain: &'static SourceDescriptor,
    /// Land-cover source (WorldCover).
    pub landcover: &'static SourceDescriptor,
    /// Buildings source (OSM Overpass).
    pub buildings: &'static SourceDescriptor,
}

/// Select every layer's source for a WGS84 viewport `bbox` (D-04 coverage lookup).
#[must_use]
pub fn plan(bbox: Bbox) -> ImportPlan {
    ImportPlan {
        terrain: terrain_source(bbox),
        landcover: source("worldcover").expect("worldcover is a registry row"),
        buildings: source("osm-overpass").expect("osm-overpass is a registry row"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small bbox around Dam Square, Amsterdam — inside AHN coverage.
    fn amsterdam() -> Bbox {
        Bbox {
            min_lon: 4.88,
            min_lat: 52.36,
            max_lon: 4.91,
            max_lat: 52.38,
        }
    }

    /// A small bbox around Berlin — well outside AHN coverage.
    fn berlin() -> Bbox {
        Bbox {
            min_lon: 13.36,
            min_lat: 52.50,
            max_lon: 13.42,
            max_lat: 52.53,
        }
    }

    #[test]
    fn registry_lists_the_four_expected_sources_with_verified_cors() {
        let ids: Vec<_> = registry().iter().map(|d| d.id).collect();
        assert_eq!(ids, ["ahn4-dtm", "glo30", "worldcover", "osm-overpass"]);

        // D-02 CORS capability map (live-probed): AHN + Overpass Direct;
        // GLO-30 + WorldCover Proxy.
        assert_eq!(source("ahn4-dtm").unwrap().cors, Cors::Direct);
        assert_eq!(source("osm-overpass").unwrap().cors, Cors::Direct);
        assert_eq!(source("glo30").unwrap().cors, Cors::Proxy);
        assert_eq!(source("worldcover").unwrap().cors, Cors::Proxy);
    }

    #[test]
    fn nl_interior_bbox_selects_ahn4_dtm_non_nl_selects_glo30_dsm() {
        // NL interior → AHN4 DTM.
        let t = terrain_source(amsterdam());
        assert_eq!(t.id, "ahn4-dtm");
        assert_eq!(t.kind, SourceKind::Dtm);

        // Outside NL → GLO-30 DSM (flagged Dsm — surface model, D-05 badge).
        let t = terrain_source(berlin());
        assert_eq!(t.id, "glo30");
        assert_eq!(t.kind, SourceKind::Dsm);
    }

    #[test]
    fn plan_selects_worldcover_and_overpass_for_every_viewport() {
        let p = plan(amsterdam());
        assert_eq!(p.terrain.id, "ahn4-dtm");
        assert_eq!(p.landcover.id, "worldcover");
        assert_eq!(p.landcover.kind, SourceKind::Landcover);
        assert_eq!(p.buildings.id, "osm-overpass");
        assert_eq!(p.buildings.kind, SourceKind::Buildings);

        // Landcover/buildings are global — same picks outside NL.
        let p = plan(berlin());
        assert_eq!(p.terrain.id, "glo30");
        assert_eq!(p.landcover.id, "worldcover");
        assert_eq!(p.buildings.id, "osm-overpass");
    }

    #[test]
    fn coverage_hull_excludes_neighbouring_capitals() {
        // Brussels, Paris, London are outside the NL hull → GLO-30.
        for (lon, lat) in [(4.35, 50.85), (2.35, 48.85), (-0.13, 51.51)] {
            let bbox = Bbox {
                min_lon: lon - 0.01,
                min_lat: lat - 0.01,
                max_lon: lon + 0.01,
                max_lat: lat + 0.01,
            };
            assert_eq!(
                terrain_source(bbox).id,
                "glo30",
                "({lon}, {lat}) must fall back to GLO-30"
            );
        }
    }

    #[test]
    fn ahn_index_carries_do_not_edit_banner_and_sha256_provenance() {
        let first = AHN_INDEX_TOML.lines().next().unwrap();
        assert!(
            first.contains("DO NOT EDIT"),
            "index must open with the DO-NOT-EDIT banner, got: {first}"
        );
        assert!(
            AHN_INDEX_TOML.contains("sha256:"),
            "index must carry a sha256 provenance line"
        );
    }

    #[test]
    fn ahn_index_parses_and_tiles_have_ordered_rd_bboxes() {
        // The committed artifact is valid TOML with well-formed RD bboxes.
        let doc: toml::Value = toml::from_str(AHN_INDEX_TOML).expect("ahn_index.toml parses");
        let tiles = doc
            .get("tile")
            .and_then(toml::Value::as_array)
            .expect("has [[tile]] entries");
        assert!(!tiles.is_empty(), "index has at least one tile");
        for t in tiles {
            let name = t.get("name").and_then(toml::Value::as_str).unwrap();
            assert!(name.starts_with("M_"), "AHN tile name: {name}");
            let min_x = t.get("rd_min_x").and_then(toml::Value::as_float).unwrap();
            let max_x = t.get("rd_max_x").and_then(toml::Value::as_float).unwrap();
            let min_y = t.get("rd_min_y").and_then(toml::Value::as_float).unwrap();
            let max_y = t.get("rd_max_y").and_then(toml::Value::as_float).unwrap();
            assert!(
                max_x > min_x && max_y > min_y,
                "{name} bbox must be ordered"
            );
        }
    }
}
