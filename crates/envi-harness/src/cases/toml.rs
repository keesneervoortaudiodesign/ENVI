//! Synthetic TOML case loader.
//!
//! Schema per 01-RESEARCH "Recommended harness input schema": `[meta]`
//! (name/kind/reference), `[source]`, `[receiver]`, `[atmosphere]`,
//! `[expected]`. TOML files are untrusted input (ASVS V5): unknown kinds,
//! non-finite numbers and out-of-domain parameters yield typed errors.

use std::path::Path;

use serde::Deserialize;

use super::{
    CaseDefinition, CaseKind, CaseLoadError, PropagationParams, ReferenceVersion, SyntheticExpected,
};

/// Raw serde mirror of the TOML case schema.
#[derive(Debug, Deserialize)]
struct TomlCaseFile {
    meta: TomlMeta,
    source: TomlSource,
    receiver: TomlReceiver,
    #[serde(default)]
    atmosphere: TomlAtmosphere,
    expected: TomlExpected,
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
    #[allow(dead_code)] // spectrum kinds beyond "unit" arrive in later phases
    spectrum: Option<TomlSpectrum>,
}

#[derive(Debug, Deserialize)]
struct TomlSpectrum {
    #[allow(dead_code)]
    kind: String,
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
}

#[derive(Debug, Deserialize)]
struct TomlExpected {
    tolerance_db: f64,
    bands: String,
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

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unnamed".to_string());

    Ok(CaseDefinition {
        id: format!("toml::{stem}"),
        name: raw.meta.name.clone(),
        kind,
        reference_version,
        description: raw.meta.name,
        source_position: Some(raw.source.position),
        receiver_position: Some(raw.receiver.position),
        propagation,
        terrain_profile: Vec::new(),
        reference_spectrum: None,
        expected: Some(SyntheticExpected {
            tolerance_db: raw.expected.tolerance_db,
            bands: raw.expected.bands,
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
