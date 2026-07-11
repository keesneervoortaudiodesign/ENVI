//! METX-01 Open-Meteo derivation test over committed fixtures (09-PATTERNS S-5,
//! mirroring `cog_window.rs`).
//!
//! Loads the committed `tests/fixtures/openmeteo_{archive,forecast}.json`
//! (SYNTHETIC, real-shaped — see each file's `_provenance`) and drives the
//! pure-Rust `envi_gis::weather` derivation: parse the multi-level profile,
//! convert AMSL geopotential height to AGL, fit per-azimuth `A/B/C`, and assert
//! the **structural / direction properties** — never a FORCE numeric pass
//! ([ASSUMED] quarantine carried from Phase 3, 09-03).
//!
//! **No network / Python needed at test time** — the fixtures are committed data.

use envi_gis::GisError;
use envi_gis::weather::{
    Level, PRESSURE_LEVELS_HPA, components_from_levels, levels_from_openmeteo, profile_for_bearing,
    sound_speed_profile_for_azimuth,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");

fn load(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DIR}{name}"))
        .unwrap_or_else(|e| panic!("fixture {name} must exist: {e}"))
}

/// The Archive fixture is a temperature inversion + wind shear, wind FROM the
/// west (downwind bearing 90°). The derivation must: parse 6 ascending-AGL
/// levels, give `a_wind > 0` (⇒ downwind A > upwind A), a downward-refracting
/// temperature profile with `B > 0` (inversion), and a crosswind A equal to the
/// isotropic temperature part.
#[test]
fn archive_fixture_derives_direction_and_inversion_properties() {
    let bytes = load("openmeteo_archive.json");
    let levels = levels_from_openmeteo(&bytes, 0).expect("archive levels parse");
    assert_eq!(
        levels.len(),
        PRESSURE_LEVELS_HPA.len() + 1,
        "one level per pressure level + the near-surface AGL anchor"
    );
    // Strictly ascending AGL height (the fit requires it).
    for w in levels.windows(2) {
        assert!(
            w[1].height_agl_m > w[0].height_agl_m,
            "levels must be strictly ascending in AGL height: {:?}",
            levels
        );
    }

    let phi = 90.0; // downwind bearing = wind_direction (270° FROM) + 180°
    let comp = components_from_levels(&levels, phi, 0.03).expect("archive components");
    assert!(
        comp.a_wind > 0.0,
        "wind shear ⇒ a_wind > 0, got {}",
        comp.a_wind
    );
    assert!(comp.b > 0.0, "inversion ⇒ B > 0, got {}", comp.b);

    let downwind = profile_for_bearing(&comp, phi, phi);
    let upwind = profile_for_bearing(&comp, phi + 180.0, phi);
    assert!(
        downwind.a > upwind.a,
        "downwind A {} must exceed upwind A {}",
        downwind.a,
        upwind.a
    );
    let cross = profile_for_bearing(&comp, phi + 90.0, phi);
    assert!(
        (cross.a - comp.a_temp).abs() < 1e-9,
        "crosswind A {} must equal the isotropic temperature part {}",
        cross.a,
        comp.a_temp
    );

    // The temperature-only fitted profile rises with height (downward refraction).
    let eval = |z: f64| comp.a_temp * (z / comp.z0 + 1.0).ln() + comp.b * z + comp.c;
    assert!(
        eval(levels.last().unwrap().height_agl_m) > eval(levels[0].height_agl_m),
        "inversion ⇒ temperature-only c_T must increase with height"
    );

    // The solver seam carries the projected A with zero fluctuation std-devs.
    let ssp = sound_speed_profile_for_azimuth(&comp, phi, phi);
    assert_eq!(ssp.a, downwind.a);
    assert_eq!(ssp.s_a, 0.0);
    assert_eq!(ssp.s_b, 0.0);
}

/// The Forecast fixture (same schema, D-02) is a normal lapse + wind shear, wind
/// FROM the SSW (downwind bearing 20°). The derivation runs to a finite
/// per-azimuth profile and the wind direction property still holds.
#[test]
fn forecast_fixture_derives_finite_profile_and_direction() {
    let bytes = load("openmeteo_forecast.json");
    let levels = levels_from_openmeteo(&bytes, 0).expect("forecast levels parse");
    let phi = 20.0; // 200° FROM + 180°
    let comp = components_from_levels(&levels, phi, 0.03).expect("forecast components");
    assert!(comp.a_temp.is_finite() && comp.a_wind.is_finite() && comp.b.is_finite());
    assert!(comp.a_wind > 0.0, "forecast wind shear ⇒ a_wind > 0");

    let downwind = profile_for_bearing(&comp, phi, phi);
    let upwind = profile_for_bearing(&comp, phi + 180.0, phi);
    assert!(downwind.a > upwind.a);
    // Normal lapse ⇒ temperature-only sound speed falls with height (upward
    // refraction from temperature alone): B < 0.
    assert!(comp.b < 0.0, "normal lapse ⇒ B < 0, got {}", comp.b);
}

/// AMSL → AGL conversion (Pitfall 5): the parsed level height is the response's
/// geopotential height minus the site elevation, per level.
#[test]
fn geopotential_height_is_converted_amsl_to_agl() {
    // Archive: elevation 10.0; 1000 hPa gph 100.0 ⇒ AGL 90.0; 850 hPa gph 1460.0
    // ⇒ AGL 1450.0. The near-surface anchor (2 m temp / 10 m wind) sits at 10 m
    // AGL directly (NO elevation subtraction — it is height-above-ground).
    let levels = levels_from_openmeteo(&load("openmeteo_archive.json"), 0).unwrap();
    let has = |h: f64| levels.iter().any(|l| (l.height_agl_m - h).abs() < 1e-9);
    assert!(has(90.0), "1000 hPa AGL = gph(100) − elevation(10) = 90");
    assert!(
        has(1450.0),
        "850 hPa AGL = gph(1460) − elevation(10) = 1450"
    );
    assert!(
        has(10.0),
        "near-surface anchor at 10 m AGL (not elevation-shifted)"
    );
    // The anchor is the lowest sample and conditions the log fit.
    assert!(
        (levels.first().unwrap().height_agl_m - 10.0).abs() < 1e-9,
        "the 10 m anchor must sort to the bottom of the profile"
    );
}

/// A missing hourly array is a typed `Json` error, and a non-finite value is a
/// typed `NonFinite` error — the parser never panics on malformed third-party
/// JSON (threat T-09-03-01, V5).
#[test]
fn malformed_openmeteo_json_is_typed_error() {
    // Missing the geopotential_height arrays entirely.
    let missing = br#"{"elevation":10.0,"hourly":{"time":["t"],"temperature_1000hPa":[8.0]}}"#;
    assert!(matches!(
        levels_from_openmeteo(missing, 0),
        Err(GisError::Json { .. })
    ));

    // A hole (null) where a numeric value is required.
    let hole = br#"{"elevation":10.0,"hourly":{"temperature_1000hPa":[null]}}"#;
    assert!(matches!(
        levels_from_openmeteo(hole, 0),
        Err(GisError::Json { .. })
    ));

    // An out-of-range hour index is a typed error, not a panic.
    let bytes = load("openmeteo_archive.json");
    assert!(matches!(
        levels_from_openmeteo(&bytes, 99),
        Err(GisError::Json { .. })
    ));
}

/// `components_from_levels` rejects a non-finite level field with a typed error
/// (defence in depth beyond the parser, for callers building `Level`s directly).
#[test]
fn components_reject_non_finite_level() {
    let levels = vec![
        Level {
            height_agl_m: 10.0,
            temperature_c: 15.0,
            wind_speed_ms: 3.0,
            wind_direction_deg: 270.0,
        },
        Level {
            height_agl_m: 50.0,
            temperature_c: f64::NAN,
            wind_speed_ms: 5.0,
            wind_direction_deg: 270.0,
        },
        Level {
            height_agl_m: 100.0,
            temperature_c: 12.0,
            wind_speed_ms: 7.0,
            wind_direction_deg: 270.0,
        },
    ];
    assert!(matches!(
        components_from_levels(&levels, 90.0, 0.03),
        Err(GisError::NonFinite { .. })
    ));
}
