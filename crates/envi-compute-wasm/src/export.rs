//! The WASM export boundary (GRID-05 / D-20/21/22, 11-04) — turns the cached
//! level grid / iso-band polygons / receiver spectra into browser-download bytes.
//!
//! # What it does
//! [`export`] deserializes an [`ExportReq`], dispatches its [`ExportFormat`], and
//! returns `Vec<u8>` the browser wraps in a `Blob` and downloads (D-20, nothing
//! leaves the device):
//! - **GeoTIFF** — the level grid as a single-strip Float32 raster
//!   ([`envi_compute::export::encode_geotiff`]), EPSG in the GeoKeyDirectory.
//! - **GeoJSON** — the iso-bands traced over the grid
//!   ([`envi_compute::isoband::trace_isobands`]) then reprojected SceneXY→LonLat
//!   and encoded as RFC-7946 fill polygons
//!   ([`envi_compute::export::encode_isophone_geojson`]).
//! - **CSV** — the receiver spectra with band index + exact Hz
//!   ([`envi_compute::export::encode_spectra_csv`]).
//!
//! # Reprojection at the ONE seam (GEOX-04)
//! GeoTIFF stays in projected UTM meters (the EPSG identifies the CRS); only the
//! GeoJSON needs WGS84, and that SceneXY→LonLat reprojection goes through
//! `envi_geo::ProjectCrs::to_wgs84` — never an inline proj string.
//!
//! # Boundary discipline (mirror of `recondition.rs`)
//! The `#[wasm_bindgen]` entries do only marshalling; the typed
//! [`run_export`]/[`sanitize_export_filename`] cores are natively `cargo test`-able.
//! Every failure is a typed [`ComputeError::Export`], never a panic (T-11-04-03).
//! The export filename is program-derived and sanitized (V12 path-traversal,
//! T-11-04-02); the bytes ride a `Blob`, never a filesystem path.

use envi_compute::export::{
    ExportMeta, IsoBandLonLat, PolygonLonLat, encode_geotiff, encode_isophone_geojson,
    encode_spectra_csv,
};
use envi_compute::grid::LevelGrid;
use envi_compute::isoband::trace_isobands;
use envi_compute::readout::ReceiverReadout;
use envi_engine::freq::FreqAxis;
use envi_geo::{ProjectCrs, SceneXY};
use wasm_bindgen::prelude::*;

use crate::dto::{ExportFormat, ExportGridDto, ExportReq, ReceiverReadoutDto};
use crate::{ComputeError, compute_err, from_js};

/// The UTM EPSG code for a zone + hemisphere (`326zz` north / `327zz` south).
fn utm_epsg(utm_zone: u32, south: bool) -> u32 {
    (if south { 32700 } else { 32600 }) + utm_zone
}

/// The [`ExportMeta`] footer for a request (D-22).
fn meta_of(req: &ExportReq) -> ExportMeta {
    ExportMeta {
        epsg: utm_epsg(req.crs.utm_zone, req.crs.south),
        weighting_label: req.weighting_label.clone(),
        engine_version: req.engine_version.clone(),
        tensor_hash: req.tensor_hash.clone(),
        attribution: req.attribution.clone(),
    }
}

/// Reconstruct a [`LevelGrid`] from its wire DTO.
fn grid_of(dto: &ExportGridDto) -> LevelGrid {
    LevelGrid {
        rows: dto.rows as usize,
        cols: dto.cols as usize,
        origin: dto.origin,
        spacing_m: dto.spacing_m,
        values: dto.values.clone(),
    }
}

/// A [`ReceiverReadout`] from its wire DTO. The CSV encoder only reads
/// `band_levels_db` + the dB(A)/dB(C) totals, so the per-band energy channels the
/// DTO does not carry are left empty — never fabricated.
fn readout_of(dto: &ReceiverReadoutDto) -> ReceiverReadout {
    ReceiverReadout {
        band_levels_db: dto.band_levels_db.clone(),
        coherent_energy: Vec::new(),
        incoherent_energy: Vec::new(),
        coherent_db: dto.coherent_db.clone(),
        incoherent_db: dto.incoherent_db.clone(),
        total_dba: dto.total_dba,
        total_dbc: dto.total_dbc,
        total_coherent_db: dto.total_coherent_db,
        total_incoherent_db: dto.total_incoherent_db,
    }
}

/// The typed, natively-testable core of [`export`]: dispatch the format and
/// produce the download bytes.
///
/// # Errors
/// [`ComputeError::Export`] when the format's required payload is absent, the
/// break scale is invalid, or a reprojection fails at the CRS seam.
pub fn run_export(req: &ExportReq) -> Result<Vec<u8>, ComputeError> {
    let meta = meta_of(req);
    match req.format {
        ExportFormat::GeoTiff => {
            let grid = grid_of(require_grid(req)?);
            Ok(encode_geotiff(&grid, &meta))
        }
        ExportFormat::GeoJson => {
            let grid = grid_of(require_grid(req)?);
            let breaks = req.breaks.as_deref().ok_or_else(|| {
                ComputeError::Export("GeoJSON export requires a `breaks` scale".to_string())
            })?;
            let bands = trace_isobands(&grid, breaks)
                .map_err(|e| ComputeError::Export(format!("iso-band tracing failed: {e}")))?;
            // Reproject each band's fill polygons SceneXY→LonLat at the one CRS seam.
            let crs = ProjectCrs::from_zone(req.crs.utm_zone as u8, req.crs.south)
                .map_err(|e| ComputeError::Export(format!("bad project CRS: {e}")))?;
            let mut lonlat = Vec::with_capacity(bands.len());
            for (i, band) in bands.iter().enumerate() {
                let mut polygons = Vec::new();
                for (exterior, holes) in band.fill_polygons() {
                    polygons.push(PolygonLonLat {
                        exterior: reproject_ring(&crs, &exterior)?,
                        holes: holes
                            .iter()
                            .map(|h| reproject_ring(&crs, h))
                            .collect::<Result<_, _>>()?,
                    });
                }
                lonlat.push(IsoBandLonLat {
                    lower: band.lower,
                    upper: band.upper,
                    fill: req.band_fills.get(i).cloned(),
                    polygons,
                });
            }
            Ok(encode_isophone_geojson(&lonlat, &meta).into_bytes())
        }
        ExportFormat::Csv => {
            let receivers = req.receivers.as_deref().ok_or_else(|| {
                ComputeError::Export("CSV export requires `receivers`".to_string())
            })?;
            let readouts: Vec<ReceiverReadout> = receivers.iter().map(readout_of).collect();
            let axis = FreqAxis::new();
            Ok(encode_spectra_csv(&req.receiver_labels, &readouts, &axis, &meta).into_bytes())
        }
    }
}

/// The grid payload, or a typed error naming the missing field.
fn require_grid(req: &ExportReq) -> Result<&ExportGridDto, ComputeError> {
    req.grid
        .as_ref()
        .ok_or_else(|| ComputeError::Export(format!("{:?} export requires a `grid`", req.format)))
}

/// Reproject a SceneXY `[x, y]` ring to WGS84 `[lon, lat]` through the ONE CRS seam.
fn reproject_ring(crs: &ProjectCrs, ring: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, ComputeError> {
    ring.iter()
        .map(|&[x, y]| {
            crs.to_wgs84(SceneXY { x_m: x, y_m: y })
                .map(|ll| [ll.lon_deg, ll.lat_deg])
                .map_err(|e| ComputeError::Export(format!("reprojection failed: {e}")))
        })
        .collect()
}

/// Sanitize a program-derived export filename base (V12 path-traversal,
/// T-11-04-02) and append the format's extension.
///
/// Any path separator (`/`, `\`), parent marker (`..`), NUL, or control character
/// is replaced with `_`; an empty result falls back to `export`. The bytes are a
/// `Blob`, never written to a filesystem path — this keeps the SUGGESTED download
/// name safe for the browser save dialog too.
#[must_use]
pub fn sanitize_export_filename(base: &str, format: ExportFormat) -> String {
    let mut safe: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Collapse any `..` parent markers that survived (dots are allowed above).
    while safe.contains("..") {
        safe = safe.replace("..", "_");
    }
    // Collapse runs of `_` (from replaced separators) into a single one.
    while safe.contains("__") {
        safe = safe.replace("__", "_");
    }
    let stem = safe.trim_matches(['.', '_']);
    let stem = if stem.is_empty() { "export" } else { stem };
    let ext = match format {
        ExportFormat::GeoTiff => "tif",
        ExportFormat::GeoJson => "geojson",
        ExportFormat::Csv => "csv",
    };
    format!("{stem}.{ext}")
}

/// Generate an export as browser-download bytes (GRID-05 / D-20). Dispatches the
/// [`ExportFormat`], returning `Vec<u8>` for a `Blob` download — nothing leaves the
/// device.
///
/// # Errors
/// A shape error in the request DTO, or a [`ComputeError::Export`] when the
/// format's payload is missing/invalid or reprojection fails.
#[wasm_bindgen]
pub fn export(req: JsValue) -> Result<Vec<u8>, JsValue> {
    let req: ExportReq = from_js(req)?;
    run_export(&req).map_err(|e| compute_err(&e))
}

/// A safe, program-derived download filename for an export (V12, T-11-04-02).
/// Sanitizes `base` and appends the format extension.
///
/// # Errors
/// A shape error in the `format` DTO.
#[wasm_bindgen]
pub fn export_filename(base: &str, format: JsValue) -> Result<String, JsValue> {
    let format: ExportFormat = from_js(format)?;
    Ok(sanitize_export_filename(base, format))
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use envi_engine::freq::N_BANDS;

    fn base_req(format: ExportFormat) -> ExportReq {
        ExportReq {
            format,
            crs: crate::dto::ExportCrsDto {
                utm_zone: 31,
                south: false,
            },
            weighting_label: "dB(A)".to_string(),
            engine_version: "envi-test".to_string(),
            tensor_hash: "abc123".to_string(),
            attribution: "© OpenStreetMap contributors; ESA WorldCover".to_string(),
            grid: None,
            breaks: None,
            band_fills: Vec::new(),
            receiver_labels: Vec::new(),
            receivers: None,
        }
    }

    /// A ramped grid near UTM 31N origin (Amsterdam-ish eastings), value = col.
    fn ramp_grid() -> ExportGridDto {
        let (rows, cols) = (4usize, 8usize);
        let mut values = Vec::with_capacity(rows * cols);
        for _r in 0..rows {
            for c in 0..cols {
                values.push(40.0 + c as f64 * 5.0);
            }
        }
        ExportGridDto {
            rows: rows as u32,
            cols: cols as u32,
            origin: [500_000.0, 5_800_000.0],
            spacing_m: 10.0,
            values,
        }
    }

    fn readout_dto(base: f64) -> ReceiverReadoutDto {
        ReceiverReadoutDto {
            band_levels_db: (0..N_BANDS).map(|f| base + f as f64).collect(),
            coherent_db: vec![0.0; N_BANDS],
            incoherent_db: vec![0.0; N_BANDS],
            total_dba: base + 90.0,
            total_dbc: base + 92.0,
            total_coherent_db: 0.0,
            total_incoherent_db: 0.0,
        }
    }

    #[test]
    fn geotiff_export_returns_a_valid_tiff_header() {
        let mut req = base_req(ExportFormat::GeoTiff);
        req.grid = Some(ramp_grid());
        let bytes = run_export(&req).unwrap();
        // Little-endian TIFF magic + non-empty raster.
        assert_eq!(&bytes[0..2], b"II");
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 42);
        assert!(bytes.len() > 4 * 4 * 8, "carries the pixel strip");
    }

    #[test]
    fn geojson_export_is_valid_wgs84_rfc7946_with_bands() {
        let mut req = base_req(ExportFormat::GeoJson);
        req.grid = Some(ramp_grid());
        req.breaks = Some(vec![50.0, 60.0, 70.0]);
        req.band_fills = vec!["#111111".to_string(), "#222222".to_string()];
        let bytes = run_export(&req).unwrap();
        let s = String::from_utf8(bytes).unwrap();

        // Parse as generic JSON (no geojson dev-dep here; the pure encoder's own
        // tests already assert RFC-7946 validity via the geojson crate).
        let v: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
        assert_eq!(v["type"], "FeatureCollection");
        let features = v["features"].as_array().unwrap();
        assert_eq!(features.len(), 2, "breaks.len() - 1 bands");
        assert_eq!(features[0]["geometry"]["type"], "MultiPolygon");
        // Coordinates are reprojected to WGS84 degrees near NL (lon ~5, lat ~52).
        let p = &features[0]["geometry"]["coordinates"][0][0][0];
        let (lon, lat) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
        // Easting 500 000 m sits on zone 31's central meridian (3°E); northing
        // 5.8 M m ≈ 52.3°N — the reprojection lands where UTM 31N expects.
        assert!((2.5..3.5).contains(&lon), "lon in WGS84 range, got {lon}");
        assert!((51.5..53.0).contains(&lat), "lat in WGS84 range, got {lat}");
        // Attribution footer present (D-22).
        assert!(s.contains("OpenStreetMap"));
        assert!(s.contains("EPSG:32631"));
    }

    #[test]
    fn csv_export_carries_band_index_exact_hz_and_attribution() {
        let mut req = base_req(ExportFormat::Csv);
        req.receivers = Some(vec![readout_dto(30.0), readout_dto(40.0)]);
        req.receiver_labels = vec!["rcv-A".to_string(), "rcv-B".to_string()];
        let bytes = run_export(&req).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains("band_index,exact_hz,rcv-A,rcv-B"));
        assert!(s.contains("# Attribution: © OpenStreetMap"));
        assert!(s.contains("dBA_total,"));
    }

    #[test]
    fn all_three_formats_produce_nonempty_bytes() {
        let mut tif = base_req(ExportFormat::GeoTiff);
        tif.grid = Some(ramp_grid());
        let mut gj = base_req(ExportFormat::GeoJson);
        gj.grid = Some(ramp_grid());
        gj.breaks = Some(vec![50.0, 60.0]);
        let mut csv = base_req(ExportFormat::Csv);
        csv.receivers = Some(vec![readout_dto(30.0)]);
        for req in [tif, gj, csv] {
            assert!(!run_export(&req).unwrap().is_empty());
        }
    }

    #[test]
    fn missing_payload_is_a_typed_error_never_a_panic() {
        // GeoTIFF without a grid, CSV without receivers, GeoJSON without breaks.
        assert!(matches!(
            run_export(&base_req(ExportFormat::GeoTiff)),
            Err(ComputeError::Export(_))
        ));
        assert!(matches!(
            run_export(&base_req(ExportFormat::Csv)),
            Err(ComputeError::Export(_))
        ));
        let mut gj = base_req(ExportFormat::GeoJson);
        gj.grid = Some(ramp_grid());
        assert!(matches!(run_export(&gj), Err(ComputeError::Export(_))));
    }

    #[test]
    fn filename_is_sanitized_against_path_traversal() {
        // Path separators, parent markers, NUL → replaced; extension appended.
        assert_eq!(
            sanitize_export_filename("../../etc/passwd", ExportFormat::GeoTiff),
            "etc_passwd.tif"
        );
        assert_eq!(
            sanitize_export_filename("results\\..\\x", ExportFormat::GeoJson),
            "results_x.geojson"
        );
        assert_eq!(
            sanitize_export_filename("scene\0name", ExportFormat::Csv),
            "scene_name.csv"
        );
        // An all-junk base falls back to a safe stem.
        assert_eq!(
            sanitize_export_filename("../..", ExportFormat::Csv),
            "export.csv"
        );
        // A clean base is preserved.
        assert_eq!(
            sanitize_export_filename("my-export_01", ExportFormat::GeoTiff),
            "my-export_01.tif"
        );
    }
}
