//! WASM export encoders (GRID-05, D-20/D-21/D-22) — the byte generators the
//! browser downloads directly (nothing leaves the device, D-20).
//!
//! # Module I/O
//! - **Inputs:** a cached [`crate::grid::LevelGrid`] (the continuous dB(A)/dB(C)
//!   level field), already-reprojected isophone fill polygons in WGS84 lon/lat
//!   ([`geojson::encode_isophone_geojson`]), and per-receiver
//!   [`crate::readout::ReceiverReadout`] spectra, plus an [`ExportMeta`] carrying
//!   the CRS + dB weighting label + engine/scene identity + data attribution.
//! - **Outputs:** three self-describing byte payloads (D-21):
//!   - **GeoTIFF** — a hand-rolled minimal single-strip Float32 raster of the level
//!     grid ([`geotiff::encode_geotiff`]), zero new dependency (D-20/D-21). The
//!     `tiff`/`geotiff-writer` crates are intentionally NOT added (RESEARCH
//!     Package Legitimacy): the ~1 KB header we own outright keeps the engine's
//!     dependency posture unchanged.
//!   - **GeoJSON** — RFC-7946 isophone fill polygons via the in-tree `geojson`
//!     crate ([`geojson::encode_isophone_geojson`]), reused from `envi-gis`.
//!   - **CSV** — receiver spectra with a BAND INDEX column AND an exact-Hz column
//!     ([`csv::encode_spectra_csv`]); nominal labels are never the identity
//!     (RESEARCH Pitfall 3 — band index only, exact Hz from
//!     [`FreqAxis::centres`](envi_engine::freq::FreqAxis)).
//! - **Invariant (D-22):** every export embeds the full metadata/attribution
//!   footer — CRS (EPSG), the dB weighting label, the engine version + scene
//!   `tensor_hash`, and the OSM/Overture/ESA WorldCover/Copernicus attribution —
//!   so a downloaded file is self-identifying and honours open-data attribution.
//!
//! # Reprojection stays at the ONE boundary (GEOX-04)
//! These encoders are coordinate-agnostic: the GeoTIFF carries the level grid in
//! its native projected (UTM) meters with the EPSG in the GeoKeyDirectory, and the
//! GeoJSON takes polygons ALREADY in WGS84 lon/lat. SceneXY→LonLat reprojection is
//! done once, in the WASM boundary (`envi-compute-wasm::export`) through
//! `envi-geo`, never inline here (there is exactly one reprojection seam).
//!
//! `#![deny(unsafe_code)]` (crate-wide); the encoders never panic on data.

pub mod csv;
pub mod geojson;
pub mod geotiff;

pub use csv::encode_spectra_csv;
pub use geojson::{IsoBandLonLat, PolygonLonLat, encode_isophone_geojson};
pub use geotiff::encode_geotiff;

/// The metadata + attribution footer stamped onto every export (D-22).
///
/// One value object shared by all three encoders so the CRS, dB weighting label,
/// engine/scene identity, and open-data attribution live in exactly one place —
/// a downloaded GeoTIFF/GeoJSON/CSV is always self-identifying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportMeta {
    /// The projected-CRS EPSG code (e.g. `32631` for UTM 31N) — stamped in the
    /// GeoTIFF GeoKeyDirectory and named in the GeoJSON/CSV footers.
    pub epsg: u32,
    /// The dB weighting label the level grid is expressed in, e.g. `"dB(A)"` /
    /// `"dB(C)"` — the raster/CSV are meaningless without it (band index rule).
    pub weighting_label: String,
    /// The engine version string (scene identity, half of the reproducibility key).
    pub engine_version: String,
    /// The frozen tensor-identity hash (scene identity, D-09) — ties an export to
    /// the exact solved tensor it was read out from.
    pub tensor_hash: String,
    /// The open-data attribution string (OSM / Overture / ESA WorldCover /
    /// Copernicus), honouring each source's attribution requirement.
    pub attribution: String,
}

impl ExportMeta {
    /// A single-line footer, e.g. for the GeoTIFF `ImageDescription` tag.
    #[must_use]
    pub fn one_line(&self) -> String {
        format!(
            "ENVI Nord2000 export | CRS=EPSG:{} | Weighting={} | Engine={} | Tensor={} | Attribution={}",
            self.epsg,
            self.weighting_label,
            self.engine_version,
            self.tensor_hash,
            self.attribution
        )
    }

    /// The footer as `#`-prefixed CSV comment lines (newline-terminated, one field
    /// per line) — the header block a downloaded spectra CSV opens with.
    #[must_use]
    pub fn csv_comment_lines(&self) -> String {
        format!(
            "# ENVI Nord2000 spectra export\n# CRS: EPSG:{}\n# Weighting: {}\n# Engine: {}\n# Tensor: {}\n# Attribution: {}\n",
            self.epsg,
            self.weighting_label,
            self.engine_version,
            self.tensor_hash,
            self.attribution
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> ExportMeta {
        ExportMeta {
            epsg: 32631,
            weighting_label: "dB(A)".to_string(),
            engine_version: "envi 0.1.0".to_string(),
            tensor_hash: "deadbeef".to_string(),
            attribution: "© OpenStreetMap contributors; ESA WorldCover; Copernicus".to_string(),
        }
    }

    #[test]
    fn one_line_footer_carries_every_identity_field() {
        let s = meta().one_line();
        assert!(s.contains("EPSG:32631"));
        assert!(s.contains("dB(A)"));
        assert!(s.contains("envi 0.1.0"));
        assert!(s.contains("deadbeef"));
        assert!(s.contains("OpenStreetMap"));
    }

    #[test]
    fn csv_comment_lines_are_hash_prefixed_and_complete() {
        let s = meta().csv_comment_lines();
        for line in s.lines() {
            assert!(
                line.starts_with('#'),
                "every metadata line is a CSV comment"
            );
        }
        assert!(s.contains("CRS: EPSG:32631"));
        assert!(s.contains("Weighting: dB(A)"));
        assert!(s.contains("Attribution:"));
    }
}
