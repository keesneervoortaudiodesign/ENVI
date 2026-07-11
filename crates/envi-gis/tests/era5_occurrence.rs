//! METX-02 ERA5 Obukhov + occurrence-statistics test over a committed synthetic
//! fixture (09-PATTERNS S-5, mirroring `cog_window.rs`).
//!
//! Loads `tests/fixtures/era5_synthetic.toml` (SYNTHETIC — see its provenance
//! banner; the expected `1/L` is an INDEPENDENT-recipe oracle) and asserts:
//! the hour-0 inverse Obukhov length within tolerance + correct sign, the
//! daytime-unstable / night-stable sign split, and the wind×stability occurrence
//! counts **exactly**. Occurrence statistics ONLY (D-05) — no class→A/B/C, no
//! `L_den`; the bin edges are `[ASSUMED]`, never a FORCE numeric weather pass.
//!
//! **No CDS key / network / Python needed at test time** — committed data.

use envi_gis::era5::{Era5Hour, N_STABILITY, N_WIND_BINS, obukhov, occurrence_stats};
use serde::Deserialize;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");

#[derive(Deserialize)]
struct Fixture {
    meta: Meta,
    expected: Expected,
    hour: Vec<HourToml>,
}

#[derive(Deserialize)]
struct Meta {
    inv_l_tol: f64,
    inv_l_hour0: f64,
}

#[derive(Deserialize)]
struct Expected {
    total: u32,
    reliable: u32,
    counts: Vec<Vec<u32>>,
}

#[derive(Deserialize)]
struct HourToml {
    iews: f64,
    inss: f64,
    ishf: f64,
    t2m_k: f64,
    d2m_k: f64,
    sp_pa: f64,
    sdfor_m: f64,
    u10_ms: f64,
    v10_ms: f64,
}

impl HourToml {
    fn to_hour(&self) -> Era5Hour {
        Era5Hour {
            iews: self.iews,
            inss: self.inss,
            ishf: self.ishf,
            t2m_k: self.t2m_k,
            d2m_k: self.d2m_k,
            sp_pa: self.sp_pa,
            sdfor_m: self.sdfor_m,
            u10_ms: self.u10_ms,
            v10_ms: self.v10_ms,
        }
    }
}

fn load() -> Fixture {
    let text = std::fs::read_to_string(format!("{DIR}era5_synthetic.toml"))
        .unwrap_or_else(|e| panic!("era5_synthetic.toml must exist: {e}"));
    toml::from_str(&text).unwrap_or_else(|e| panic!("era5_synthetic.toml must parse: {e}"))
}

#[test]
fn hour0_inverse_obukhov_matches_independent_oracle() {
    let fx = load();
    let hours: Vec<Era5Hour> = fx.hour.iter().map(HourToml::to_hour).collect();
    let inv_l = obukhov(&hours[0]).expect("hour 0 obukhov");
    assert!(
        (inv_l - fx.meta.inv_l_hour0).abs() <= fx.meta.inv_l_tol,
        "hour-0 1/L = {inv_l} vs oracle {} (tol {})",
        fx.meta.inv_l_hour0,
        fx.meta.inv_l_tol
    );
    assert!(inv_l < 0.0, "hour 0 is daytime ⇒ 1/L < 0 (unstable)");
}

#[test]
fn daytime_unstable_night_stable_across_the_fixture() {
    let fx = load();
    // h0/h1/h5 are unstable (ishf < 0); h2/h3 stable (ishf > 0); h4 near-neutral.
    let inv = |i: usize| obukhov(&fx.hour[i].to_hour()).unwrap();
    assert!(
        inv(0) < 0.0 && inv(1) < 0.0 && inv(5) < 0.0,
        "ishf<0 ⇒ unstable"
    );
    assert!(inv(2) > 0.0 && inv(3) > 0.0, "ishf>0 ⇒ stable");
    assert!(inv(4).abs() < 0.002, "tiny flux ⇒ near-neutral");
}

#[test]
fn occurrence_counts_match_the_fixture_exactly() {
    let fx = load();
    let hours: Vec<Era5Hour> = fx.hour.iter().map(HourToml::to_hour).collect();
    let occ = occurrence_stats(&hours).expect("occurrence stats");

    assert_eq!(occ.total, fx.expected.total, "total hours");
    assert_eq!(occ.reliable, fx.expected.reliable, "reliable hours");
    assert_eq!(
        fx.expected.counts.len(),
        N_WIND_BINS,
        "fixture has all wind bins"
    );
    for (bin, row) in fx.expected.counts.iter().enumerate() {
        assert_eq!(row.len(), N_STABILITY, "each row has all stability classes");
        for (stab, &want) in row.iter().enumerate() {
            assert_eq!(
                occ.counts[bin][stab], want,
                "counts[{bin}][{stab}] mismatch (occurrence statistics ONLY)"
            );
        }
    }
    // The counts sum to the total (no hour dropped/double-counted).
    let summed: u32 = occ.counts.iter().flatten().sum();
    assert_eq!(summed, occ.total);
}
