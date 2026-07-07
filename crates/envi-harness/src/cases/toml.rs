//! Synthetic TOML case loader.
//!
//! Schema per 01-RESEARCH "Recommended harness input schema": `[meta]`
//! (name/kind/reference), `[source]`, `[receiver]`, `[atmosphere]`,
//! `[expected]`. TOML files are untrusted input (ASVS V5): unknown kinds,
//! non-finite numbers and out-of-domain parameters yield typed errors.

use std::path::Path;

use serde::Deserialize;

use super::{
    CaseDefinition, CaseKind, CaseLoadError, PropagationParams, ReferenceVersion,
    SyntheticExpected,
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
    let _ = path;
    todo!("Task 2 GREEN: parse + validate the TOML case schema")
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
        let expected = case.expected.expect("synthetic case carries an expected block");
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
        assert!(matches!(err, CaseLoadError::UnknownKind { .. }), "got {err:?}");
        let msg = err.to_string();
        assert!(msg.contains("free-field"), "error must list accepted kinds: {msg}");
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
        assert!(matches!(err, CaseLoadError::NonFinite { .. }), "got {err:?}");
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
