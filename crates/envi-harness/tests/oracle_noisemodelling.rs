//! NoiseModelling CNOSSOS cross-validation (VAL-03) — the committed-fixture oracle
//! for ENVI's *shared* sub-effects, mirroring the scipy-oracle pattern.
//!
//! Reads ONLY `tests/fixtures/oracle/noisemodelling.toml` (produced by the one-time
//! human-run recipe in `tools/noisemodelling_oracle/README.md`). **No Java, no
//! network, no credentials at test time.** NoiseModelling v6.0.0 (GPLv3, CNOSSOS-EU
//! Directive (EU) 2015/996) is run once by an operator and its numeric OUTPUTS are
//! committed here — its source is never ported (CLAUDE.md licensing rule).
//!
//! Posture (04-RESEARCH "Comparable quantities and the delta posture"):
//! - **Equality gates** only where the physics is genuinely identical:
//!   geometrical divergence (≤0.1 dB) and ISO 9613-1 air absorption (≤0.2 dB/octave;
//!   a larger 8 kHz delta is DOCUMENTED, not forced).
//! - **Report-only** expected-delta tables for barrier insertion loss (Nord2000
//!   Hadden-Pierce wedge vs CNOSSOS empirical `Adif`) and flat-ground excess
//!   attenuation (complex spherical-wave Q̂ interference vs CNOSSOS empirical
//!   G-model) — equality would indicate a comparison bug, not success.
//!
//! Comparison is strictly **by band index** on the 105-point 1/12-octave grid: the 8
//! CNOSSOS octave centres (63 Hz–8 kHz) are grid indices 16, 28, 40, 52, 64, 76, 88,
//! 100. Never compare by nominal frequency.
//!
//! When the fixture carries placeholder rows (`[meta] placeholder = true`, e.g. Java
//! absent so NoiseModelling has not been run), the fixture-driven gates **SKIP**
//! cleanly (honest fail-soft — never a false Pass) with a pointer to the recipe.

use envi_engine::freq::FreqAxis;
use envi_engine::propagation::air_absorption::{Atmosphere, alpha_db_per_m, band_attenuation_db};
use envi_engine::propagation::divergence::divergence_db;
use serde::Deserialize;

/// The 8 CNOSSOS octave centres (63 Hz–8 kHz) as 1/12-octave grid indices.
const OCTAVE_BAND_INDICES: [usize; 8] = [16, 28, 40, 52, 64, 76, 88, 100];

#[derive(Deserialize)]
struct Fixture {
    meta: Meta,
    divergence: Vec<DivergenceRow>,
    air_absorption: Vec<AirAbsorptionRow>,
    barrier: Vec<BarrierRow>,
    ground: Vec<GroundRow>,
}

#[derive(Deserialize)]
struct Meta {
    placeholder: bool,
    nm_version: String,
    direct_distance_m: f64,
    atmosphere_t_c: f64,
    atmosphere_rh_percent: f64,
    atmosphere_pressure_kpa: f64,
    divergence_tol_db: f64,
    air_absorption_tol_db: f64,
    air_absorption_8k_documented_tol_db: f64,
}

#[derive(Deserialize)]
struct DivergenceRow {
    scene: String,
    band_index: usize,
    adiv_db: f64,
}

#[derive(Deserialize)]
struct AirAbsorptionRow {
    scene: String,
    band_index: usize,
    aatm_db: f64,
}

#[derive(Deserialize)]
struct BarrierRow {
    scene: String,
    band_index: usize,
    abar_db: f64,
}

#[derive(Deserialize)]
struct GroundRow {
    scene: String,
    band_index: usize,
    aground_db: f64,
}

fn load() -> Fixture {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/oracle/noisemodelling.toml"
    );
    let text = std::fs::read_to_string(path).expect("noisemodelling oracle TOML must exist");
    toml::from_str(&text).expect("noisemodelling oracle TOML must parse")
}

/// Honest fail-soft: on a placeholder fixture (NoiseModelling not yet run) the
/// fixture-driven gates report a SKIP instead of a false Pass.
fn skip_if_placeholder(meta: &Meta, test: &str) -> bool {
    if meta.placeholder {
        eprintln!(
            "SKIP [{test}]: noisemodelling.toml carries placeholder rows (NoiseModelling \
             {} not yet run — Java absent). Regenerate per tools/noisemodelling_oracle/\
             README.md; this gate activates automatically once [meta] placeholder = false.",
            meta.nm_version
        );
    }
    meta.placeholder
}

/// A truncated fixture must not silently pass: every one of the 8 CNOSSOS octave
/// centres must be present exactly once.
fn assert_octave_coverage(indices: &[usize], what: &str) {
    for want in OCTAVE_BAND_INDICES {
        assert!(
            indices.contains(&want),
            "{what}: fixture missing octave band index {want} — a truncated fixture \
             cannot silently pass"
        );
    }
    assert_eq!(
        indices.len(),
        OCTAVE_BAND_INDICES.len(),
        "{what}: expected exactly {} octave rows, got {}",
        OCTAVE_BAND_INDICES.len(),
        indices.len()
    );
}

fn force_atmosphere(meta: &Meta) -> Atmosphere {
    Atmosphere::new(
        meta.atmosphere_t_c,
        meta.atmosphere_rh_percent,
        meta.atmosphere_pressure_kpa,
    )
    .expect("fixture atmosphere must be valid")
}

/// EQUALITY GATE: ENVI's geometrical divergence equals CNOSSOS `Adiv` within ≤0.1 dB
/// at every octave band index. ENVI's ΔL_d = −10·lg(4πR²) is a (negative) level
/// change; CNOSSOS `Adiv` = 20·lg d + 11 is a positive attenuation — they agree
/// because 10·lg(4π) = 10.99 ≈ 11.
#[test]
fn envi_divergence_equals_cnossos_adiv_by_band_index() {
    let fx = load();
    if skip_if_placeholder(&fx.meta, "divergence") {
        return;
    }
    let indices: Vec<usize> = fx.divergence.iter().map(|r| r.band_index).collect();
    assert_octave_coverage(&indices, "divergence");

    // Divergence is frequency-independent; evaluated once at the shared geometry and
    // compared at every octave band index (coverage proves all 8 were checked).
    let envi_atten = -divergence_db(fx.meta.direct_distance_m).expect("valid direct range");
    for row in &fx.divergence {
        let delta = (envi_atten - row.adiv_db).abs();
        assert!(
            delta <= fx.meta.divergence_tol_db,
            "[{}] band {}: ENVI divergence {envi_atten:.4} dB vs CNOSSOS Adiv {:.4} dB, \
             Δ={delta:.4} dB > {:.2} dB gate",
            row.scene,
            row.band_index,
            row.adiv_db,
            fx.meta.divergence_tol_db
        );
    }
}

/// EQUALITY GATE: ENVI's ISO 9613-1 band attenuation equals CNOSSOS `Aatm` within
/// ≤0.2 dB per octave point, evaluated at the EXACT grid centre for each band index.
/// The 8 kHz point (band 100) is DOCUMENTED, not forced: ENVI's Eq. 287 band
/// conversion vs CNOSSOS's α·d centre value may exceed 0.2 dB (band-averaging vs
/// centre value); it is reported and checked only against a looser documented bound.
#[test]
fn envi_air_absorption_equals_cnossos_aatm_by_band_index() {
    let fx = load();
    if skip_if_placeholder(&fx.meta, "air_absorption") {
        return;
    }
    let indices: Vec<usize> = fx.air_absorption.iter().map(|r| r.band_index).collect();
    assert_octave_coverage(&indices, "air_absorption");

    let axis = FreqAxis::new();
    let atmos = force_atmosphere(&fx.meta);
    let d = fx.meta.direct_distance_m;
    for row in &fx.air_absorption {
        // Compare BY BAND INDEX: evaluate α at the exact 1/12-octave centre.
        let f = axis.centres[row.band_index];
        let envi_atten = band_attenuation_db(alpha_db_per_m(f, &atmos) * d);
        let delta = (envi_atten - row.aatm_db).abs();
        if row.band_index == 100 {
            eprintln!(
                "[{}] 8 kHz (band 100): ENVI Aatm {envi_atten:.4} dB vs CNOSSOS {:.4} dB, \
                 Δ={delta:.4} dB — DOCUMENTED (Eq. 287 band conversion vs α·d centre value), \
                 not forced to the {:.2} dB octave gate",
                row.scene, row.aatm_db, fx.meta.air_absorption_tol_db
            );
            assert!(
                delta <= fx.meta.air_absorption_8k_documented_tol_db,
                "8 kHz air-absorption Δ={delta:.4} dB exceeds even the documented bound \
                 {:.2} dB — this would be a real bug, not the expected band/centre delta",
                fx.meta.air_absorption_8k_documented_tol_db
            );
        } else {
            assert!(
                delta <= fx.meta.air_absorption_tol_db,
                "[{}] band {}: ENVI Aatm {envi_atten:.4} dB vs CNOSSOS {:.4} dB, \
                 Δ={delta:.4} dB > {:.2} dB gate",
                row.scene,
                row.band_index,
                row.aatm_db,
                fx.meta.air_absorption_tol_db
            );
        }
    }
}

/// REPORT-ONLY: barrier insertion loss. Nord2000's Hadden-Pierce wedge and CNOSSOS's
/// empirical `Adif` are different models (0–6 dB deltas expected, largest at high f_c
/// and grazing incidence). Equality would indicate a comparison bug, so NOTHING is
/// asserted on the values — the CNOSSOS terms are tabulated with the expected-delta
/// rationale; ENVI's wedge values are pinned by its own module oracle tests.
#[test]
fn barrier_insertion_loss_is_report_only_expected_delta() {
    let fx = load();
    if skip_if_placeholder(&fx.meta, "barrier") {
        return;
    }
    let indices: Vec<usize> = fx.barrier.iter().map(|r| r.band_index).collect();
    assert_octave_coverage(&indices, "barrier");

    eprintln!(
        "REPORT-ONLY barrier insertion loss — CNOSSOS Adif (empirical) vs ENVI \
         Hadden-Pierce wedge; 0–6 dB delta EXPECTED, NOT gated (different models):"
    );
    for row in &fx.barrier {
        eprintln!(
            "  [{}] band {}: CNOSSOS Abar = {:.4} dB",
            row.scene, row.band_index, row.abar_db
        );
    }
    // No equality assertion by design.
}

/// REPORT-ONLY: flat-ground excess attenuation. ENVI's complex spherical-wave Q̂
/// interference produces frequency dips; CNOSSOS's empirical octave-band G-model does
/// not. Fundamentally different — only magnitude and trend are documented, no
/// equality is asserted.
#[test]
fn ground_excess_attenuation_is_report_only_expected_delta() {
    let fx = load();
    if skip_if_placeholder(&fx.meta, "ground") {
        return;
    }
    let indices: Vec<usize> = fx.ground.iter().map(|r| r.band_index).collect();
    assert_octave_coverage(&indices, "ground");

    eprintln!(
        "REPORT-ONLY flat-ground excess attenuation — CNOSSOS Aground (empirical G-model, \
         no dips) vs ENVI complex Q̂ (interference dips); trend/magnitude only, NOT gated:"
    );
    for row in &fx.ground {
        eprintln!(
            "  [{}] band {}: CNOSSOS Aground = {:.4} dB",
            row.scene, row.band_index, row.aground_db
        );
    }
    // No equality assertion by design.
}

/// Analytic self-check of the divergence comparison (NOT the NoiseModelling
/// cross-check): proves ENVI's −10·lg(4πR²) equals the CNOSSOS-EU **directive** closed
/// form 20·lg d + 11 to within the 0.1 dB gate (the exact delta is |10·lg 4π − 11| =
/// 0.0079 dB). This uses the published Directive (EU) 2015/996 closed form — not any
/// NoiseModelling binary output — so the fixture-driven gate above is meaningful the
/// moment real NM numbers land, and this assertion is live even while the fixture is a
/// placeholder.
#[test]
fn divergence_matches_cnossos_directive_closed_form_identity() {
    for &d in &[10.0_f64, 50.0, 100.004_999_875, 250.0, 1000.0] {
        let envi_atten = -divergence_db(d).unwrap();
        let cnossos_adiv = 20.0 * d.log10() + 11.0;
        let delta = (envi_atten - cnossos_adiv).abs();
        assert!(
            delta <= 0.1,
            "d={d} m: ENVI divergence {envi_atten:.4} dB vs CNOSSOS directive closed form \
             {cnossos_adiv:.4} dB, Δ={delta:.4} dB > 0.1 dB",
        );
    }
}
