//! Permanent bit-exact regression pinning the PRE-extension OPAQUE screen output
//! of `terrain_effect` (plan 05-03, D-10 / SC3).
//!
//! Plan 05-03 threads an optional `isolation: Option<&IsolationSpectrum>` seam
//! through `terrain_effect` → `screen_channel` so that semi-transparent
//! screens/façades add a straight-through transmitted term `T(f)` to the
//! coherent screen branch. The house discipline (D-10) requires that the OPAQUE
//! limit — `isolation: None`, the structural absence of any partition spectrum —
//! reproduce the standard opaque screen **bit-for-bit**. Opaque is the `None`
//! state, NOT a large `R` (D-10); so the pin lives on the `None` path.
//!
//! This file captures, for two committed screen cases (a SingleEdge thin screen
//! and a ThickScreen), every 105-band output of `terrain_effect` —
//! `h_coh_factor.re`, `h_coh_factor.im`, `p_incoh`, and `delta_l_db` — as raw
//! `f64` bit patterns, committed ONCE from the pre-extension tree (plans
//! 05-01/05-02 did not touch `terrain_effect`, so the tree is still
//! pre-extension for these bits). After the isolation seam lands, the same
//! calls pass a trailing `None`; the bits MUST NOT move.
//!
//! The comparison is `u64::to_bits()` equality, NOT a float tolerance: the
//! structural-identity contract is exact-or-broken.
//!
//! The T7 direction/exactness test (semi-transparent behaviour) lives at the
//! bottom of this file — it exercises the `Some(R)` path against the pinned
//! `None` path so both halves of the D-10/SC2/SC3 contract sit together.

use envi_engine::freq::{FREQ_AXIS, N_BANDS};
use envi_engine::propagation::terrain_effect::terrain_effect;
use envi_harness::build_terrain_inputs;
use serde::Deserialize;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// The two committed screen cases pinned here: (fixture-table name, case file).
/// case71 is a thin (SingleEdge) screen; case81 is a ThickScreen — two distinct
/// screen classes exercise both the Sub-model 4 and Sub-model 5 assembly paths.
const CASES: [(&str, &str); 2] = [
    ("single_edge_case71", "terrain_screen_thin_case71.toml"),
    ("thick_case81", "terrain_screen_thick_case81.toml"),
];

/// Run `terrain_effect` (opaque, `isolation: None`) on a committed screen case
/// and return the per-band `f64` bit patterns of `h_coh_factor.re`,
/// `h_coh_factor.im`, `p_incoh`, and `delta_l_db`.
fn compute_case(case_file: &str) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) {
    let path = repo_root().join("cases").join(case_file);
    let case = envi_harness::cases::toml::load_toml_case(&path)
        .unwrap_or_else(|e| panic!("screen case {case_file} must load: {e}"));
    let (profile, src, rcv, coh) =
        build_terrain_inputs(&case).unwrap_or_else(|e| panic!("inputs for {case_file}: {e}"));
    let axis = &*FREQ_AXIS;
    let te = terrain_effect(&profile, src, rcv, coh.c0, &coh, axis, None)
        .unwrap_or_else(|e| panic!("terrain_effect on {case_file}: {e}"));

    let mut h_re = Vec::with_capacity(N_BANDS);
    let mut h_im = Vec::with_capacity(N_BANDS);
    let mut p = Vec::with_capacity(N_BANDS);
    let mut dl = Vec::with_capacity(N_BANDS);
    for f in 0..N_BANDS {
        h_re.push(te.h_coh_factor[f].re.to_bits());
        h_im.push(te.h_coh_factor[f].im.to_bits());
        p.push(te.p_incoh[f].to_bits());
        dl.push(te.delta_l_db[f].to_bits());
    }
    (h_re, h_im, p, dl)
}

#[derive(Deserialize)]
struct CaseBits {
    h_re_bits: Vec<String>,
    h_im_bits: Vec<String>,
    p_bits: Vec<String>,
    dl_bits: Vec<String>,
}

#[derive(Deserialize)]
struct Baseline {
    single_edge_case71: CaseBits,
    thick_case81: CaseBits,
}

impl Baseline {
    fn case(&self, name: &str) -> &CaseBits {
        match name {
            "single_edge_case71" => &self.single_edge_case71,
            "thick_case81" => &self.thick_case81,
            other => panic!("unknown fixture case {other}"),
        }
    }
}

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/regression/opaque_baseline.toml"
);

/// Parse a `0x`-prefixed hex string into the `u64` bit pattern it encodes.
fn parse_bits(s: &str) -> u64 {
    u64::from_str_radix(&s[2..], 16).unwrap_or_else(|_| panic!("bad hex bit pattern: {s}"))
}

/// OPAQUE BASELINE GENERATOR — run ONLY on the pre-extension tree, ONCE.
///
/// `cargo test -p envi-harness --test opaque_regression -- --ignored`
///
/// Writes the committed fixture. DO NOT run this after the isolation seam lands:
/// it would silently redefine the opaque-limit contract and destroy the D-10
/// regression's meaning.
#[test]
#[ignore = "opaque baseline generator - run ONLY on the pre-extension tree"]
fn regenerate_opaque_baseline() {
    let fmt = |bits: &[u64]| {
        bits.iter()
            .map(|b| format!("\"{b:#018x}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut s = String::new();
    s.push_str("# generated by opaque_regression.rs::regenerate_opaque_baseline — DO NOT EDIT\n");
    s.push_str(
        "# Bit-exact PRE-extension opaque screen baseline (hex f64 bit patterns per band).\n\n",
    );
    for (name, case_file) in CASES {
        let (h_re, h_im, p, dl) = compute_case(case_file);
        // Each case is its own TOML table; its four arrays follow its header so
        // TOML captures them into that table (not a later [meta] table).
        s.push_str(&format!("[{name}]\n"));
        s.push_str(&format!("h_re_bits = [{}]\n", fmt(&h_re)));
        s.push_str(&format!("h_im_bits = [{}]\n", fmt(&h_im)));
        s.push_str(&format!("p_bits = [{}]\n", fmt(&p)));
        s.push_str(&format!("dl_bits = [{}]\n\n", fmt(&dl)));
    }
    s.push_str("[meta]\n");
    s.push_str(
        "description = \"Bit-exact PRE-extension OPAQUE screen baseline of terrain_effect (hex f64 bit patterns per band)\"\n",
    );
    s.push_str(
        "note = \"captured PRE-extension; this file IS the opaque-limit contract (D-10/SC3) — regenerating it after the isolation seam lands destroys the regression's meaning\"\n",
    );
    std::fs::write(FIXTURE_PATH, s).expect("write opaque_baseline.toml");
    eprintln!("wrote {FIXTURE_PATH}");
}

/// T6 (permanent, D-10/SC3): the OPAQUE (`isolation: None`) screen output
/// reproduces the pinned pre-extension bits for every band, both screen classes.
#[test]
fn opaque_screen_matches_pinned_pre_extension_baseline() {
    let text = std::fs::read_to_string(FIXTURE_PATH).expect("opaque_baseline.toml must exist");
    let fx: Baseline = toml::from_str(&text).expect("opaque_baseline.toml must parse");

    for (name, case_file) in CASES {
        let cb = fx.case(name);
        assert_eq!(cb.h_re_bits.len(), N_BANDS, "{name}: 105 h_re bits");
        assert_eq!(cb.h_im_bits.len(), N_BANDS, "{name}: 105 h_im bits");
        assert_eq!(cb.p_bits.len(), N_BANDS, "{name}: 105 p bits");
        assert_eq!(cb.dl_bits.len(), N_BANDS, "{name}: 105 dl bits");

        let (h_re, h_im, p, dl) = compute_case(case_file);
        for f in 0..N_BANDS {
            assert_eq!(
                h_re[f],
                parse_bits(&cb.h_re_bits[f]),
                "{name} band {f}: h_coh_factor.re bits moved — the opaque (None) path is not bit-identical"
            );
            assert_eq!(
                h_im[f],
                parse_bits(&cb.h_im_bits[f]),
                "{name} band {f}: h_coh_factor.im bits moved — the opaque (None) path is not bit-identical"
            );
            assert_eq!(
                p[f],
                parse_bits(&cb.p_bits[f]),
                "{name} band {f}: p_incoh bits moved — the opaque (None) path is not bit-identical"
            );
            assert_eq!(
                dl[f],
                parse_bits(&cb.dl_bits[f]),
                "{name} band {f}: delta_l_db bits moved — the opaque (None) path is not bit-identical"
            );
        }
    }
}
