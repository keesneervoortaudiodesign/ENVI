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
pub use geojson::{GeoJsonEncodeError, IsoBandLonLat, PolygonLonLat, encode_isophone_geojson};
pub use geotiff::{EpsgOverflow, encode_geotiff};

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
    /// A single-line footer, e.g. for the GeoTIFF `ImageDescription` tag. Free-text
    /// fields have their CR/LF collapsed to a space so the footer stays one line.
    #[must_use]
    pub fn one_line(&self) -> String {
        format!(
            "ENVI Nord2000 export | CRS=EPSG:{} | Weighting={} | Engine={} | Tensor={} | Attribution={}",
            self.epsg,
            one_line_field(&self.weighting_label),
            one_line_field(&self.engine_version),
            one_line_field(&self.tensor_hash),
            one_line_field(&self.attribution)
        )
    }

    /// The footer as `#`-prefixed CSV comment lines (newline-terminated, one field
    /// per line) — the header block a downloaded spectra CSV opens with.
    ///
    /// Each free-text field has its CR/LF collapsed to a space (WR-02): a comment
    /// line is not a CSV field, so an embedded newline must not break out of the `#`
    /// comment block and inject a fake data row.
    #[must_use]
    pub fn csv_comment_lines(&self) -> String {
        format!(
            "# ENVI Nord2000 spectra export\n# CRS: EPSG:{}\n# Weighting: {}\n# Engine: {}\n# Tensor: {}\n# Attribution: {}\n",
            self.epsg,
            one_line_field(&self.weighting_label),
            one_line_field(&self.engine_version),
            one_line_field(&self.tensor_hash),
            one_line_field(&self.attribution)
        )
    }
}

/// Collapse CR/LF in a free-text footer field to a single space so an embedded
/// newline cannot break out of a single-line footer (the CSV `#` comment block or
/// the GeoTIFF `ImageDescription` tag) — WR-02.
fn one_line_field(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
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

    #[test]
    fn comment_header_neutralizes_embedded_newlines() {
        // WR-02: a free-text field carrying a newline must NOT break out of the `#`
        // comment block — every line of the header must still be a `#` comment.
        let meta = ExportMeta {
            epsg: 32631,
            weighting_label: "dB(A)\n# fake data row,1,2,3".to_string(),
            engine_version: "e\r\nvil".to_string(),
            tensor_hash: "abc".to_string(),
            attribution: "attr".to_string(),
        };
        let s = meta.csv_comment_lines();
        for line in s.lines() {
            assert!(
                line.starts_with('#'),
                "an embedded newline must not escape the comment block: {line:?}"
            );
        }
        // The one-line footer likewise stays a single line.
        assert_eq!(meta.one_line().lines().count(), 1);
    }
}
