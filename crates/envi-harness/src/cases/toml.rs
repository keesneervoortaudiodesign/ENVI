//! Synthetic TOML case loader.
//!
//! Schema per 01-RESEARCH "Recommended harness input schema": `[meta]`
//! (name/kind/reference), `[source]`, `[receiver]`, `[atmosphere]`,
//! `[expected]`. TOML files are untrusted input (ASVS V5): unknown kinds,
//! non-finite numbers and out-of-domain parameters yield typed errors.

use std::path::Path;

use serde::Deserialize;

use super::{
    CaseDefinition, CaseKind, CaseLoadError, GeometryExpected, PropagationParams, ReferenceVersion,
    SourceSpectrum, SyntheticExpected, TerrainRow,
};

/// Raw serde mirror of the TOML case schema.
#[derive(Debug, Deserialize)]
struct TomlCaseFile {
    meta: TomlMeta,
    source: TomlSource,
    receiver: TomlReceiver,
    #[serde(default)]
    atmosphere: TomlAtmosphere,
    /// Terrain-profile rows (terrain cases only).
    #[serde(default)]
    terrain: Vec<TomlTerrainRow>,
    expected: TomlExpected,
}

/// One terrain-profile row `(x, z, σ, r)` for a terrain-effect case.
#[derive(Debug, Deserialize)]
struct TomlTerrainRow {
    x: f64,
    z: f64,
    sigma_kpa: f64,
    #[serde(default)]
    roughness: f64,
}

#[derive(Debug, Deserialize)]
struct TomlMeta {
    name: String,
    kind: String,
    reference: String,
}

#[derive(Debug, Deserialize)]
struct TomlSource {
    position: [f64; 3],
    spectrum: Option<TomlSpectrum>,
}

/// `[source.spectrum]` — the point sub-source's `L_W` (SRC-01). `kind` is one
/// of `"unit"` / `"uniform"` (needs `base_db`) / `"ramp"` (needs `base_db` and
/// `slope_db_per_band`).
#[derive(Debug, Deserialize)]
struct TomlSpectrum {
    kind: String,
    base_db: Option<f64>,
    slope_db_per_band: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TomlReceiver {
    position: [f64; 3],
}

#[derive(Debug, Default, Deserialize)]
struct TomlAtmosphere {
    t_air_c: Option<f64>,
    rh_percent: Option<f64>,
    pressure_kpa: Option<f64>,
    /// Wind-velocity turbulence structure parameter `Cv²` (terrain cases).
    cv2: Option<f64>,
    /// Temperature turbulence structure parameter `CT²` (terrain cases).
    ct2: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TomlExpected {
    tolerance_db: f64,
    bands: String,
    #[serde(default)]
    geometry: Option<TomlGeometryExpected>,
    /// Oracle-pinned 105-point `ΔL_t` reference curve (terrain cases).
    #[serde(default)]
    values: Option<Vec<f64>>,
}

/// `[expected.geometry]` — hand-computed geometry anchors. All optional so one
/// schema serves both the azimuth and reflection cases.
#[derive(Debug, Deserialize)]
struct TomlGeometryExpected {
    azimuth_deg: Option<f64>,
    reflection_x: Option<f64>,
    path_length_m: Option<f64>,
    reflection_segment: Option<[[f64; 2]; 2]>,
    tolerance: Option<f64>,
}

/// Load one synthetic case from a TOML file.
///
/// # Errors
///
/// Returns a typed [`CaseLoadError`] on I/O failure, TOML syntax errors,
/// unknown `kind`/`reference` strings, non-finite numbers, or out-of-domain
/// atmosphere parameters. Never panics on file content.
pub fn load_toml_case(path: &Path) -> Result<CaseDefinition, CaseLoadError> {
    let context = path.display().to_string();

    let text = std::fs::read_to_string(path).map_err(|source| CaseLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw: TomlCaseFile = ::toml::from_str(&text).map_err(|e| CaseLoadError::TomlParse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let kind = match raw.meta.kind.as_str() {
        "free-field" => CaseKind::FreeField,
        "geometry" => CaseKind::Geometry,
        "terrain" => CaseKind::Terrain,
        "refraction" => CaseKind::Refraction,
        other => {
            return Err(CaseLoadError::UnknownKind {
                got: other.to_string(),
                path: path.to_path_buf(),
            });
        }
    };
    let reference_version = match raw.meta.reference.as_str() {
        "analytic" => ReferenceVersion::Analytic,
        "force-2009" => ReferenceVersion::Force2009,
        "force-2010" => ReferenceVersion::Force2010,
        other => {
            return Err(CaseLoadError::UnknownReference {
                got: other.to_string(),
                path: path.to_path_buf(),
            });
        }
    };

    for (what, arr) in [
        ("source.position", &raw.source.position),
        ("receiver.position", &raw.receiver.position),
    ] {
        for (i, &v) in arr.iter().enumerate() {
            super::xls::require_finite(v, &context, &format!("{what}[{i}]"))?;
        }
    }

    let mut propagation = PropagationParams::default();
    if let Some(t) = raw.atmosphere.t_air_c {
        super::xls::require_finite(t, &context, "atmosphere.t_air_c")?;
        if t <= -273.15 {
            return Err(CaseLoadError::Invalid {
                context,
                message: format!("t_air_c = {t} °C is at or below absolute zero"),
            });
        }
        propagation.t0_c = Some(t);
    }
    if let Some(rh) = raw.atmosphere.rh_percent {
        super::xls::require_finite(rh, &context, "atmosphere.rh_percent")?;
        if !(0.0..=100.0).contains(&rh) {
            return Err(CaseLoadError::Invalid {
                context,
                message: format!("rh_percent = {rh} outside [0, 100]"),
            });
        }
        propagation.rh_percent = rh;
    }
    if let Some(p) = raw.atmosphere.pressure_kpa {
        super::xls::require_finite(p, &context, "atmosphere.pressure_kpa")?;
        if p <= 0.0 {
            return Err(CaseLoadError::Invalid {
                context,
                message: format!("pressure_kpa = {p} must be positive"),
            });
        }
        propagation.pressure_kpa = p;
    }
    if let Some(cv2) = raw.atmosphere.cv2 {
        super::xls::require_finite(cv2, &context, "atmosphere.cv2")?;
        if cv2 < 0.0 {
            return Err(CaseLoadError::Invalid {
                context,
                message: format!("cv2 = {cv2} must be non-negative"),
            });
        }
        propagation.cv2 = Some(cv2);
    }
    if let Some(ct2) = raw.atmosphere.ct2 {
        super::xls::require_finite(ct2, &context, "atmosphere.ct2")?;
        if ct2 < 0.0 {
            return Err(CaseLoadError::Invalid {
                context,
                message: format!("ct2 = {ct2} must be non-negative"),
            });
        }
        propagation.ct2 = Some(ct2);
    }

    // Terrain-profile rows (terrain cases). X must be strictly ascending — the
    // TerrainProfile constructor re-validates, but reject early with context.
    let mut terrain_profile: Vec<TerrainRow> = Vec::new();
    for (i, row) in raw.terrain.iter().enumerate() {
        super::xls::require_finite(row.x, &context, &format!("terrain[{i}].x"))?;
        super::xls::require_finite(row.z, &context, &format!("terrain[{i}].z"))?;
        super::xls::require_finite(row.sigma_kpa, &context, &format!("terrain[{i}].sigma_kpa"))?;
        super::xls::require_finite(row.roughness, &context, &format!("terrain[{i}].roughness"))?;
        if row.sigma_kpa <= 0.0 {
            return Err(CaseLoadError::Invalid {
                context,
                message: format!(
                    "terrain[{i}].sigma_kpa = {} must be positive",
                    row.sigma_kpa
                ),
            });
        }
        terrain_profile.push(TerrainRow {
            x_m: row.x,
            z_m: row.z,
            flow_resistivity_kns_m4: row.sigma_kpa,
            roughness_m: row.roughness,
        });
    }
    if kind == CaseKind::Terrain && terrain_profile.len() < 2 {
        return Err(CaseLoadError::Invalid {
            context,
            message: "terrain case requires at least 2 [[terrain]] rows".to_string(),
        });
    }

    super::xls::require_finite(raw.expected.tolerance_db, &context, "expected.tolerance_db")?;
    if raw.expected.tolerance_db <= 0.0 {
        return Err(CaseLoadError::Invalid {
            context,
            message: format!(
                "expected.tolerance_db = {} must be positive",
                raw.expected.tolerance_db
            ),
        });
    }

    // Resolve the source spectrum (SRC-01). Each provided level is checked
    // finite; ramp/uniform require their parameters so a malformed fixture is a
    // typed error, not a silent 0 dB.
    let source_spectrum = match raw.source.spectrum {
        None => SourceSpectrum::Unit,
        Some(s) => {
            if let Some(v) = s.base_db {
                super::xls::require_finite(v, &context, "source.spectrum.base_db")?;
            }
            if let Some(v) = s.slope_db_per_band {
                super::xls::require_finite(v, &context, "source.spectrum.slope_db_per_band")?;
            }
            match s.kind.as_str() {
                "unit" => SourceSpectrum::Unit,
                "uniform" => {
                    let base_db = s.base_db.ok_or_else(|| CaseLoadError::Invalid {
                        context: context.clone(),
                        message: "source.spectrum kind \"uniform\" requires base_db".to_string(),
                    })?;
                    SourceSpectrum::Uniform(base_db)
                }
                "ramp" => {
                    let base_db = s.base_db.ok_or_else(|| CaseLoadError::Invalid {
                        context: context.clone(),
                        message: "source.spectrum kind \"ramp\" requires base_db".to_string(),
                    })?;
                    let slope_db_per_band =
                        s.slope_db_per_band.ok_or_else(|| CaseLoadError::Invalid {
                            context: context.clone(),
                            message: "source.spectrum kind \"ramp\" requires slope_db_per_band"
                                .to_string(),
                        })?;
                    SourceSpectrum::Ramp {
                        base_db,
                        slope_db_per_band,
                    }
                }
                other => {
                    return Err(CaseLoadError::Invalid {
                        context,
                        message: format!(
                            "unknown source.spectrum kind {other:?} \
                             (accepted: \"unit\", \"uniform\", \"ramp\")"
                        ),
                    });
                }
            }
        }
    };

    // Optional [expected.geometry] block: validate every provided value is
    // finite, and require a reflection segment whenever a reflection anchor is
    // present (so run_case never reflects over a missing segment).
    let geometry = match raw.expected.geometry {
        None => None,
        Some(g) => {
            for (what, v) in [
                ("expected.geometry.azimuth_deg", g.azimuth_deg),
                ("expected.geometry.reflection_x", g.reflection_x),
                ("expected.geometry.path_length_m", g.path_length_m),
                ("expected.geometry.tolerance", g.tolerance),
            ] {
                if let Some(v) = v {
                    super::xls::require_finite(v, &context, what)?;
                }
            }
            if let Some(seg) = g.reflection_segment {
                for (i, p) in seg.iter().enumerate() {
                    for (j, &v) in p.iter().enumerate() {
                        super::xls::require_finite(
                            v,
                            &context,
                            &format!("expected.geometry.reflection_segment[{i}][{j}]"),
                        )?;
                    }
                }
            }
            if (g.reflection_x.is_some() || g.path_length_m.is_some())
                && g.reflection_segment.is_none()
            {
                return Err(CaseLoadError::Invalid {
                    context,
                    message: "expected.geometry sets a reflection anchor but no \
                              reflection_segment"
                        .to_string(),
                });
            }
            Some(GeometryExpected {
                azimuth_deg: g.azimuth_deg,
                reflection_x: g.reflection_x,
                path_length_m: g.path_length_m,
                reflection_segment: g.reflection_segment,
                tolerance: g.tolerance,
            })
        }
    };

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unnamed".to_string());

    // Oracle-pinned 105-point ΔL_t reference (terrain cases).
    let reference_bands = match raw.expected.values {
        None => None,
        Some(vals) => {
            for (i, &v) in vals.iter().enumerate() {
                super::xls::require_finite(v, &context, &format!("expected.values[{i}]"))?;
            }
            if kind == CaseKind::Terrain && vals.len() != 105 {
                return Err(CaseLoadError::Invalid {
                    context,
                    message: format!(
                        "terrain reference has {} values (expected exactly 105)",
                        vals.len()
                    ),
                });
            }
            Some(vals)
        }
    };

    Ok(CaseDefinition {
        id: format!("toml::{stem}"),
        name: raw.meta.name.clone(),
        kind,
        reference_version,
        description: raw.meta.name,
        source_position: Some(raw.source.position),
        source_spectrum,
        receiver_position: Some(raw.receiver.position),
        propagation,
        terrain_profile,
        reference_spectrum: None,
        expected: Some(SyntheticExpected {
            tolerance_db: raw.expected.tolerance_db,
            bands: raw.expected.bands,
            geometry,
            reference_bands,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("cases")
            .join("freefield_100m.toml")
    }

    fn write_temp(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn loads_the_freefield_fixture() {
        let case = load_toml_case(&fixture_path()).expect("fixture must load");
        assert_eq!(case.kind, CaseKind::FreeField);
        assert_eq!(case.reference_version, ReferenceVersion::Analytic);
        assert_eq!(case.source_position, Some([0.0, 0.0, 0.5]));
        assert_eq!(case.receiver_position, Some([100.0, 0.0, 1.5]));
        assert_relative_eq!(case.propagation.t0_c.unwrap(), 15.0);
        assert_relative_eq!(case.propagation.rh_percent, 70.0);
        let expected = case
            .expected
            .expect("synthetic case carries an expected block");
        assert_relative_eq!(expected.tolerance_db, 1e-9);
        assert_eq!(expected.bands, "analytic:divergence+iso9613");
        assert_eq!(case.id, "toml::freefield_100m");
    }

    #[test]
    fn ramp_spectrum_parses_into_a_source_spectrum() {
        let path = write_temp(
            "envi_ramp_case.toml",
            r#"
[meta]
name = "ramp"
kind = "free-field"
reference = "analytic"
[source]
position = [0.0, 0.0, 0.5]
spectrum = { kind = "ramp", base_db = 80.0, slope_db_per_band = 0.1 }
[receiver]
position = [100.0, 0.0, 1.5]
[atmosphere]
t_air_c = 15.0
[expected]
tolerance_db = 1e-9
bands = "analytic:divergence+iso9613"
"#,
        );
        let case = load_toml_case(&path).expect("ramp case must load");
        assert_eq!(
            case.source_spectrum,
            SourceSpectrum::Ramp {
                base_db: 80.0,
                slope_db_per_band: 0.1
            }
        );
    }

    #[test]
    fn ramp_missing_slope_is_a_typed_error() {
        let path = write_temp(
            "envi_ramp_missing_slope.toml",
            r#"
[meta]
name = "ramp"
kind = "free-field"
reference = "analytic"
[source]
position = [0.0, 0.0, 0.5]
spectrum = { kind = "ramp", base_db = 80.0 }
[receiver]
position = [100.0, 0.0, 1.5]
[expected]
tolerance_db = 1e-9
bands = "analytic:divergence"
"#,
        );
        let err = load_toml_case(&path).unwrap_err();
        assert!(matches!(err, CaseLoadError::Invalid { .. }), "got {err:?}");
    }

    #[test]
    fn unknown_kind_is_a_typed_error_listing_accepted_kinds() {
        let path = write_temp(
            "envi_bad_kind.toml",
            r#"
[meta]
name = "bad"
kind = "warp-drive"
reference = "analytic"
[source]
position = [0.0, 0.0, 0.5]
[receiver]
position = [100.0, 0.0, 1.5]
[expected]
tolerance_db = 1e-9
bands = "analytic:divergence"
"#,
        );
        let err = load_toml_case(&path).unwrap_err();
        assert!(
            matches!(err, CaseLoadError::UnknownKind { .. }),
            "got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("free-field"),
            "error must list accepted kinds: {msg}"
        );
    }

    #[test]
    fn nan_values_are_rejected_with_a_typed_error() {
        let path = write_temp(
            "envi_nan_case.toml",
            r#"
[meta]
name = "nan"
kind = "free-field"
reference = "analytic"
[source]
position = [nan, 0.0, 0.5]
[receiver]
position = [100.0, 0.0, 1.5]
[expected]
tolerance_db = 1e-9
bands = "analytic:divergence"
"#,
        );
        let err = load_toml_case(&path).unwrap_err();
        assert!(
            matches!(err, CaseLoadError::NonFinite { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn out_of_range_humidity_is_rejected() {
        let path = write_temp(
            "envi_bad_rh.toml",
            r#"
[meta]
name = "bad rh"
kind = "free-field"
reference = "analytic"
[source]
position = [0.0, 0.0, 0.5]
[receiver]
position = [100.0, 0.0, 1.5]
[atmosphere]
rh_percent = 250.0
[expected]
tolerance_db = 1e-9
bands = "analytic:divergence"
"#,
        );
        let err = load_toml_case(&path).unwrap_err();
        assert!(matches!(err, CaseLoadError::Invalid { .. }), "got {err:?}");
    }
}
