//! Import-viewport tile planning + bbox→pixel-window resolution (the TS-facing
//! geometry the WASM orchestrator needs but must NOT re-implement in TypeScript).
//!
//! # Module I/O
//! - **Inputs:** a WGS84 lon/lat viewport [`Bbox`] (tile enumeration), or a whole
//!   cached COG tile's bytes plus that viewport + its [`TerrainSourceCrs`]
//!   (window resolution).
//! - **Output:** [`plan_tiles`] returns the covering [`TileRef`]s per raster layer
//!   (terrain + land cover), each carrying the absolute upstream fetch URL built
//!   from the registry's `endpoint_template` (TS routes Direct vs the byte proxy
//!   per the descriptor's `cors`). [`window_for_bbox`] reads the tile's IFD
//!   geometry, reprojects the WGS84 viewport into the tile's source CRS through
//!   [`envi_geo`] (the single reprojection boundary, GEOX-04), and returns the
//!   [`PixelWindow`] to decode — or `None` when the viewport covers no in-image
//!   pixel (never a guessed clamp).
//! - **Invariant (load-bearing):** sans-I/O and no-panic. Tile names for the
//!   integer-grid sources (GLO-30 1°, WorldCover 3°) are computed here; AHN
//!   kaartblad names come from the committed index ([`registry::ahn_tiles_for`],
//!   parsed from [`registry::AHN_INDEX_TOML`] with a hand-rolled line reader —
//!   **no `toml`/`serde` runtime dependency**). Buildings (Overpass) are a
//!   bbox-query, not a tile grid, so they carry no [`TileRef`] here — TS builds
//!   the Overpass query from the viewport directly.
//! - **Invariant (load-bearing, DATA-01):** the terrain tiles [`plan_tiles`]
//!   returns ALWAYS belong to [`registry::terrain_source`] for the same viewport —
//!   one selection, used by both. That is what keeps the descriptor TS reads (CRS,
//!   CORS mode, DTM/DSM badge) in step with the tiles it is handed, and it is why
//!   the GLO-30 fallback lives in `terrain_source` (which gates on a real
//!   kaartblad) rather than being patched in here.

use envi_geo::{LonLat, RdNewCrs};

use crate::GisError;
use crate::cog::plan::{self, ReadPlan};
use crate::cog::sparse::{ByteRange, CogBytes};
use crate::cog::window::{PixelWindow, bbox_to_pixel_window};
use crate::cog::{MAX_DECODED_PX, geo_tags, header};
use crate::registry::{self, Bbox, SourceDescriptor, reproject_bbox_to_rd};
use crate::terrain::TerrainSourceCrs;

/// One covering source tile: everything TS needs to fetch + provenance-stamp it.
///
/// `url` is the ABSOLUTE upstream URL (registry `endpoint_template` with its
/// `{...}` slots filled). For a `Proxy` source (GLO-30, WorldCover) the TS
/// fetcher rewrites this to the same-origin `/api/v1/proxy/{source_id}/{path}`
/// relay; for a `Direct` source (AHN) it is fetched cross-origin as-is. `tile`
/// is the stable per-tile cache key + provenance `source_ref`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileRef {
    /// Registry source id (`"ahn4-dtm"`, `"glo30"`, `"worldcover"`).
    pub source_id: &'static str,
    /// The tile identifier (kaartblad name / lat-lon grid name) — cache key +
    /// provenance `source_ref`.
    pub tile: String,
    /// Absolute upstream fetch URL (endpoint template with slots filled).
    pub url: String,
}

/// The per-raster-layer covering tiles for a viewport (buildings are a
/// bbox-query, handled TS-side, so they are absent here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportTiles {
    /// Covering terrain tiles (AHN kaartblads in NL, else GLO-30 1° cells).
    pub terrain: Vec<TileRef>,
    /// Covering WorldCover 3° land-cover tiles.
    pub landcover: Vec<TileRef>,
}

/// Enumerate the covering source tiles for a WGS84 viewport `bbox` (D-04 / D-06).
///
/// Terrain uses [`registry::terrain_source`] (AHN where a real kaartblad covers
/// the viewport, else the global GLO-30 DSM); land cover is always WorldCover.
/// Names for the integer-grid sources are computed here; AHN names come from the
/// committed kaartblad index.
#[must_use]
pub fn plan_tiles(bbox: Bbox) -> ImportTiles {
    let terrain_src = registry::terrain_source(bbox);
    // Dispatch on the descriptor's tile addressing scheme, not a hardcoded source
    // id, so a new terrain source slots in by declaring its scheme (D-04). An
    // unknown scheme yields NO tiles rather than a silent GLO-30 fallback that
    // would mislabel it — a new scheme must be wired here deliberately.
    let terrain = match terrain_src.tile_scheme {
        "kaartblad" => ahn_tiles(bbox, terrain_src),
        "1deg" => glo30_tiles(bbox, terrain_src),
        _ => Vec::new(),
    };
    let landcover = registry::source("worldcover")
        .map(|d| worldcover_tiles(bbox, d))
        .unwrap_or_default();
    ImportTiles { terrain, landcover }
}

/// Resolve the [`PixelWindow`] of `bbox` (WGS84) within a cached terrain/land-cover
/// tile, reprojecting the viewport into the tile's `source_crs` through
/// [`envi_geo`] (GEOX-04) before the geometry math.
///
/// Returns `Ok(None)` when the viewport covers no in-image pixel center (the tile
/// is adjacent, not overlapping) — never a guessed/clamped full-tile window. The
/// `max_decoded_px` budget is enforced from the resolved window's pixel count so
/// an oversized viewport is a typed [`GisError::DecodeBudgetExceeded`] here,
/// before TS ever calls a decode entry point.
///
/// # Errors
/// - [`GisError::Tiff`] / [`GisError::MissingGeoTag`] / [`GisError::InvalidGeoTransform`]
///   / [`GisError::TooManyImages`] / [`GisError::WindowOutOfBounds`] from the IFD read.
/// - [`GisError::Reproject`] if the RD-New reprojection fails.
/// - [`GisError::DecodeBudgetExceeded`] if the resolved window exceeds the budget.
pub fn window_for_bbox(
    tile_bytes: &[u8],
    bbox: Bbox,
    source_crs: TerrainSourceCrs,
    max_decoded_px: usize,
) -> Result<Option<PixelWindow>, GisError> {
    let cog = CogBytes::whole(tile_bytes);
    // Cap the IFD chain up front (T-08-02-02), mirroring `decode_window`.
    header::guard_image_count(&cog)?;
    let mut dec = header::open_cog(&cog)?;
    let hdr = header::read_header(&mut dec)?;
    let geo = geo_tags::read_geotransform(&mut dec)?;

    // Reproject the WGS84 viewport into the tile's source CRS (GEOX-04: RD New
    // goes through `envi_geo` only). WGS84 tiles need no transform.
    let (min_x, min_y, max_x, max_y) = match source_crs {
        TerrainSourceCrs::Wgs84 => (bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat),
        TerrainSourceCrs::RdNew => reproject_bbox_to_rd(bbox)?,
    };

    let Some(win) = bbox_to_pixel_window(&geo, &hdr, min_x, min_y, max_x, max_y) else {
        return Ok(None);
    };

    // Budget guard (T-08-02-01): reject an oversized window as a typed error
    // rather than letting the later decode allocate — same bound `decode_window`
    // enforces, surfaced one step earlier so the import UI can warn cleanly.
    let px = (win.width as usize)
        .checked_mul(win.height as usize)
        .ok_or(GisError::DecodeBudgetExceeded {
            requested_px: usize::MAX,
            limit: max_decoded_px,
        })?;
    if px > max_decoded_px {
        return Err(GisError::DecodeBudgetExceeded {
            requested_px: px,
            limit: max_decoded_px,
        });
    }
    Ok(Some(win))
}

/// The default decoded-pixel budget (re-exported so the boundary can pass the
/// core default when the caller does not override it).
#[must_use]
pub fn default_budget() -> usize {
    MAX_DECODED_PX
}

// --- windowed COG range planning (the TS-facing two-pass entry point) --------

/// The complete plan for range-reading ONE viewport out of ONE COG tile: what the
/// TS orchestrator must fetch, and the window it will then decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CogReadPlan {
    /// `Some(n)` ⇒ the header prefix supplied was too short: fetch bytes `0..n`
    /// and plan again (converges — see [`crate::cog::plan`] invariant 2).
    pub need_header: Option<u64>,
    /// The pixel window of the viewport inside this tile — `None` when the
    /// viewport covers no in-image pixel centre (the tile is adjacent, not
    /// overlapping; never a guessed clamp). Nothing is fetched in that case.
    pub window: Option<PixelWindow>,
    /// The byte ranges still to fetch, sorted + coalesced, already minus the
    /// caller's cached ranges. EMPTY when the cache covers the window — the
    /// DATA-04 network-off replay.
    pub fetch: Vec<ByteRange>,
    /// Bytes in [`CogReadPlan::fetch`].
    pub fetch_bytes: u64,
    /// Bytes of every chunk the window needs, cached or not (the honest footprint
    /// of the window in the file — the number to compare against the whole-tile
    /// size when judging the range read).
    pub window_bytes: u64,
    /// How many COG chunks the window overlaps.
    pub chunks: usize,
}

/// Plan the windowed range read of `bbox` out of the COG whose leading bytes are
/// `header_prefix`: resolve the viewport to a pixel window through the tile's IFD
/// geometry (reprojecting into `source_crs` via [`envi_geo`], GEOX-04), then emit
/// exactly the byte ranges of the chunks that window overlaps, minus the `have`
/// ranges the caller already holds.
///
/// This is the ONE call the TypeScript import path makes before fetching: it
/// replaces the whole-tile GET that made an Amsterdam terrain import download
/// 330 MB. `envi-gis` still performs NO I/O — it only says *which bytes*.
///
/// # Errors
/// - [`GisError::HeaderTooLarge`] — the header exceeds [`plan::MAX_HEADER_BYTES`].
/// - [`GisError::FetchBudgetExceeded`] — the window's chunks exceed `max_fetch_bytes`.
/// - [`GisError::DecodeBudgetExceeded`] — the window exceeds `max_decoded_px`.
/// - [`GisError::Reproject`] — the RD-New reprojection failed.
/// - [`GisError::Tiff`] / [`GisError::MissingGeoTag`] / [`GisError::InvalidGeoTransform`]
///   / [`GisError::TooManyImages`] — from the IFD read.
pub fn plan_cog_reads(
    header_prefix: &[u8],
    bbox: Bbox,
    source_crs: TerrainSourceCrs,
    max_decoded_px: usize,
    max_fetch_bytes: u64,
    have: &[ByteRange],
) -> Result<CogReadPlan, GisError> {
    // (1) Header-fit FIRST: everything below reads IFD-declared geometry, so the
    //     IFD must be fully present before any of it can be trusted.
    if let Some(need) = plan::header_needed_bytes(header_prefix)? {
        return Ok(CogReadPlan {
            need_header: Some(need),
            window: None,
            fetch: Vec::new(),
            fetch_bytes: 0,
            window_bytes: 0,
            chunks: 0,
        });
    }

    // (2) The viewport -> pixel window, through the tile's own geotransform. This
    //     also enforces `max_decoded_px` (T-08-02-01) — an oversized viewport is
    //     rejected BEFORE a single byte is fetched, not after.
    let Some(window) = window_for_bbox(header_prefix, bbox, source_crs, max_decoded_px)? else {
        return Ok(CogReadPlan {
            need_header: None,
            window: None,
            fetch: Vec::new(),
            fetch_bytes: 0,
            window_bytes: 0,
            chunks: 0,
        });
    };

    // (3) The window -> its chunks' byte ranges, minus what the caller caches.
    let ReadPlan {
        need_header,
        fetch,
        fetch_bytes,
        window_bytes,
        chunks,
    } = plan::plan_window_reads(header_prefix, window, have, max_fetch_bytes)?;
    Ok(CogReadPlan {
        need_header,
        window: Some(window),
        fetch,
        fetch_bytes,
        window_bytes,
        chunks,
    })
}

/// Reproject a WGS84 `[lon, lat]` ring into a terrain tile's `source_crs` so it
/// can feed [`crate::terrain::sample_base_elevation`] (which takes a source-CRS
/// ring). WGS84 terrain (GLO-30) is the identity; RD-New terrain (AHN) goes
/// through [`envi_geo`] (GEOX-04) — the reprojection TS must NOT do itself. Used
/// by the buildings layer to sample footprint-boundary base elevation off the
/// retained terrain tile (SC4).
///
/// # Errors
/// [`GisError::Reproject`] if the RD-New reprojection of any vertex fails.
pub fn reproject_ring_to_source(
    ring_wgs84: &[[f64; 2]],
    source_crs: TerrainSourceCrs,
) -> Result<Vec<[f64; 2]>, GisError> {
    match source_crs {
        TerrainSourceCrs::Wgs84 => Ok(ring_wgs84.to_vec()),
        TerrainSourceCrs::RdNew => {
            let crs = RdNewCrs::new().map_err(|e| GisError::Reproject {
                message: e.to_string(),
            })?;
            ring_wgs84
                .iter()
                .map(|&[lon, lat]| {
                    crs.to_rd(LonLat {
                        lon_deg: lon,
                        lat_deg: lat,
                    })
                    .map(|p| [p.x_m, p.y_m])
                    .map_err(|e| GisError::Reproject {
                        message: e.to_string(),
                    })
                })
                .collect()
        }
    }
}

// --- AHN kaartblad enumeration (committed index, no runtime toml dep) --------

/// AHN covering tiles: the kaartbladen the committed index resolves for the
/// viewport ([`registry::ahn_tiles_for`] — the SAME lookup [`registry::terrain_source`]
/// gates on), turned into fetchable [`TileRef`]s. Non-empty whenever AHN was
/// selected at all; a viewport with no kaartblad never reaches here (it was
/// resolved to GLO-30 upstream).
fn ahn_tiles(bbox: Bbox, desc: &SourceDescriptor) -> Vec<TileRef> {
    registry::ahn_tiles_for(bbox)
        .into_iter()
        .map(|t| TileRef {
            source_id: desc.id,
            url: desc.endpoint_template.replace("{tile}", &t.name),
            tile: t.name.clone(),
        })
        .collect()
}

// --- GLO-30 (1°) + WorldCover (3°) integer-grid enumeration ------------------

/// GLO-30 covering tiles: every 1°×1° cell whose SW corner integer lat/lon the
/// viewport overlaps. The tile file is named by its south/west integer corner
/// (`Copernicus_DSM_COG_10_N52_00_E004_00_DEM`).
fn glo30_tiles(bbox: Bbox, desc: &SourceDescriptor) -> Vec<TileRef> {
    let mut tiles = Vec::new();
    for lat in grid_cells(bbox.min_lat, bbox.max_lat, 1) {
        for lon in grid_cells(bbox.min_lon, bbox.max_lon, 1) {
            let ns = ns_label(lat, 2);
            let ew = ew_label(lon, 3);
            let url = desc
                .endpoint_template
                .replace("{ns}", &ns)
                .replace("{ew}", &ew);
            tiles.push(TileRef {
                source_id: desc.id,
                tile: format!("{ns}_{ew}"),
                url,
            });
        }
    }
    tiles
}

/// WorldCover covering tiles: every 3°×3° cell whose SW corner (a multiple of 3)
/// the viewport overlaps. Named `N51E003` etc. from that corner.
fn worldcover_tiles(bbox: Bbox, desc: &SourceDescriptor) -> Vec<TileRef> {
    let mut tiles = Vec::new();
    for lat in grid_cells(bbox.min_lat, bbox.max_lat, 3) {
        for lon in grid_cells(bbox.min_lon, bbox.max_lon, 3) {
            let name = format!("{}{}", ns_label(lat, 2), ew_label(lon, 3));
            let url = desc.endpoint_template.replace("{tile}", &name);
            tiles.push(TileRef {
                source_id: desc.id,
                tile: name,
                url,
            });
        }
    }
    tiles
}

/// The SW-corner coordinates (multiples of `step`) of every grid cell the closed
/// interval `[lo, hi]` overlaps. Uses floor-to-`step` so a cell is included iff
/// the interval reaches into it.
fn grid_cells(lo: f64, hi: f64, step: i64) -> Vec<i64> {
    let s = step.max(1) as f64;
    let first = (lo / s).floor() as i64 * step;
    let last = (hi / s).floor() as i64 * step;
    (0..)
        .map(|k| first + k * step)
        .take_while(|&c| c <= last)
        .collect()
}

/// `N{width}` / `S{width}` label for an integer latitude corner.
fn ns_label(lat: i64, width: usize) -> String {
    let hemi = if lat < 0 { 'S' } else { 'N' };
    format!("{hemi}{:0width$}", lat.abs(), width = width)
}

/// `E{width}` / `W{width}` label for an integer longitude corner.
fn ew_label(lon: i64, width: usize) -> String {
    let hemi = if lon < 0 { 'W' } else { 'E' };
    format!("{hemi}{:0width$}", lon.abs(), width = width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amsterdam() -> Bbox {
        Bbox {
            min_lon: 4.88,
            min_lat: 52.36,
            max_lon: 4.91,
            max_lat: 52.38,
        }
    }

    fn berlin() -> Bbox {
        Bbox {
            min_lon: 13.36,
            min_lat: 52.50,
            max_lon: 13.42,
            max_lat: 52.53,
        }
    }

    /// The exact viewport of the failing in-browser import (Amsterdam centre,
    /// Dam Square / Nieuwmarkt), which planned ZERO terrain tiles and so never
    /// issued a single network request ("No terrain tiles cover this viewport").
    fn amsterdam_centre() -> Bbox {
        Bbox {
            min_lon: 4.894,
            min_lat: 52.363,
            max_lon: 4.914,
            max_lat: 52.373,
        }
    }

    #[test]
    fn nl_viewport_plans_ahn_tiles_with_direct_pdok_urls() {
        // A viewport over a kaartblad present in the committed index subset.
        let bbox = Bbox {
            min_lon: 4.9,
            min_lat: 52.34,
            max_lon: 5.0,
            max_lat: 52.40,
        };
        let tiles = ahn_tiles(bbox, registry::source("ahn4-dtm").unwrap());
        assert!(!tiles.is_empty(), "an NL viewport must resolve kaartbladen");
        for t in &tiles {
            assert_eq!(t.source_id, "ahn4-dtm");
            assert!(t.url.starts_with("https://service.pdok.nl/"), "{}", t.url);
            assert!(!t.url.contains("{tile}"), "slot filled: {}", t.url);
            assert!(t.url.contains(&t.tile), "url names its tile: {}", t.url);
        }
    }

    /// REGRESSION (the reported defect): the real Amsterdam-centre viewport must
    /// plan a NON-EMPTY terrain tile list. Before the fix the committed kaartblad
    /// index held only a 9-tile "representative subset" with fabricated RD
    /// bboxes — no tile intersected Amsterdam, `plan_tiles` returned zero terrain
    /// tiles, and the import died before it ever hit the network.
    #[test]
    fn amsterdam_centre_viewport_plans_the_real_ahn_kaartblad() {
        let plan = plan_tiles(amsterdam_centre());
        assert!(
            !plan.terrain.is_empty(),
            "Amsterdam must plan terrain tiles (it planned zero — the reported bug)"
        );
        assert!(plan.terrain.iter().all(|t| t.source_id == "ahn4-dtm"));
        // Amsterdam centre sits in kaartblad M_25GN1 (RD 120000..125000 x
        // 481250..487500) — verified live against the PDOK download endpoint.
        assert!(
            plan.terrain.iter().any(|t| t.tile == "M_25GN1"),
            "expected M_25GN1, got {:?}",
            plan.terrain.iter().map(|t| &t.tile).collect::<Vec<_>>()
        );
        // And the land-cover tile the same viewport needs.
        assert_eq!(plan.landcover.len(), 1);
        assert_eq!(plan.landcover[0].tile, "N51E003");
    }

    /// The tile plan and the descriptor TS reads MUST come from one selection —
    /// otherwise TS fetches GLO-30 bytes with AHN's CRS/CORS. Pin that they agree.
    #[test]
    fn planned_terrain_tiles_always_belong_to_the_selected_terrain_source() {
        for bbox in [amsterdam_centre(), berlin(), amsterdam()] {
            let src = registry::terrain_source(bbox);
            let plan = plan_tiles(bbox);
            assert!(!plan.terrain.is_empty(), "every viewport plans terrain");
            assert!(
                plan.terrain.iter().all(|t| t.source_id == src.id),
                "tiles must belong to the selected source {}",
                src.id
            );
        }
    }

    #[test]
    fn glo30_names_the_south_west_integer_corner() {
        let tiles = glo30_tiles(berlin(), registry::source("glo30").unwrap());
        assert_eq!(tiles.len(), 1, "berlin viewport is inside one 1deg cell");
        let t = &tiles[0];
        assert_eq!(t.tile, "N52_E013");
        assert!(t.url.contains("N52") && t.url.contains("E013"));
        assert!(!t.url.contains("{ns}") && !t.url.contains("{ew}"));
    }

    #[test]
    fn worldcover_names_the_three_degree_corner() {
        let tiles = worldcover_tiles(amsterdam(), registry::source("worldcover").unwrap());
        assert_eq!(tiles.len(), 1);
        // Amsterdam (4.88E, 52.36N) → 3deg SW corner (3E, 51N) → N51E003.
        assert_eq!(tiles[0].tile, "N51E003");
        assert!(tiles[0].url.contains("N51E003"));
    }

    #[test]
    fn plan_tiles_picks_glo30_outside_nl() {
        let plan = plan_tiles(berlin());
        assert!(plan.terrain.iter().all(|t| t.source_id == "glo30"));
        assert!(plan.landcover.iter().all(|t| t.source_id == "worldcover"));
    }

    #[test]
    fn grid_cells_span_a_multi_cell_viewport() {
        // A viewport straddling two 1deg cells in each axis → 4 cells.
        let lats = grid_cells(52.5, 53.5, 1);
        assert_eq!(lats, vec![52, 53]);
        let lons = grid_cells(-0.5, 1.5, 1);
        assert_eq!(lons, vec![-1, 0, 1]);
    }

    #[test]
    fn reproject_ring_identity_for_wgs84_and_rd_for_ahn() {
        let ring = [[4.9, 52.37], [4.91, 52.37], [4.91, 52.38], [4.9, 52.37]];
        // WGS84 terrain → identity.
        let same = reproject_ring_to_source(&ring, TerrainSourceCrs::Wgs84).unwrap();
        assert_eq!(same, ring.to_vec());
        // RD-New terrain → Amsterdam RD is ~ (120000, 487000) m, orders above degrees.
        let rd = reproject_ring_to_source(&ring, TerrainSourceCrs::RdNew).unwrap();
        assert_eq!(rd.len(), ring.len());
        assert!(
            rd.iter().all(|[x, y]| *x > 100_000.0 && *y > 400_000.0),
            "{rd:?}"
        );
    }

    #[test]
    fn hemisphere_labels_pad_and_sign() {
        assert_eq!(ns_label(52, 2), "N52");
        assert_eq!(ns_label(-3, 2), "S03");
        assert_eq!(ew_label(4, 3), "E004");
        assert_eq!(ew_label(-77, 3), "W077");
    }
}
