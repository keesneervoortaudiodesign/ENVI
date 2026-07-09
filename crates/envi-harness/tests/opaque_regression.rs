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
use envi_engine::propagation::PropagationError;
use envi_engine::propagation::coherence::CoherenceInputs;
use envi_engine::propagation::terrain_effect::{TerrainEffect, terrain_effect};
use envi_engine::propagation::transmission::{IsolationSpectrum, TransmissionFilter};
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
    let te = terrain_effect(&profile, src, rcv, coh.c0, &coh, axis, None, None)
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

/// Run `terrain_effect` on a committed screen case with an optional isolation
/// spectrum (and optionally forced-calm turbulence), returning the full
/// two-channel [`TerrainEffect`]. Used by the semi-transparent tests T7–T9.
fn run_screen(case_file: &str, iso: Option<&IsolationSpectrum>, calm: bool) -> TerrainEffect {
    let path = repo_root().join("cases").join(case_file);
    let case = envi_harness::cases::toml::load_toml_case(&path)
        .unwrap_or_else(|e| panic!("screen case {case_file} must load: {e}"));
    let (profile, src, rcv, coh) =
        build_terrain_inputs(&case).unwrap_or_else(|e| panic!("inputs for {case_file}: {e}"));
    let coh = if calm {
        CoherenceInputs {
            cv2: 0.0,
            ct2: 0.0,
            ..coh
        }
    } else {
        coh
    };
    let axis = &*FREQ_AXIS;
    terrain_effect(&profile, src, rcv, coh.c0, &coh, axis, None, iso)
        .unwrap_or_else(|e| panic!("terrain_effect on {case_file}: {e}"))
}

/// T7 (direction + exactness, D-05/D-08): a semi-transparent SingleEdge screen
/// with a flat `R = 15 dB` partition (a) raises deep-shadow band levels vs
/// opaque and (b) adds EXACTLY the native `T(f)` where the screen branch is
/// fully engaged.
///
/// The composition is `h_semi = h_opaque + r_scr1·T` (the Eq. 332 outer blend
/// weights the whole screen branch, including `T`, by `r_scr1`). A flat `R`
/// gives `φ ≡ 0`, so `T` is real and positive (`|T| = 10^(−0.75) ≈ 0.178`);
/// hence `diff = h_semi − h_opaque` is always a real, non-negative multiple of
/// `T`, and at full engagement (`r_scr1 = 1`) `diff == T` — re-pinning the native
/// sign end-to-end.
#[test]
fn transmission_raises_deep_shadow_and_equals_t_native_where_engaged() {
    let iso = IsolationSpectrum::new([15.0; N_BANDS]).unwrap();
    let t_ref = TransmissionFilter::from_isolation(&iso).bands;
    // Flat R ⇒ T real & positive; sanity-anchor its magnitude/sign.
    assert!(
        (t_ref[64].re - 10f64.powf(-0.75)).abs() < 1e-12 && t_ref[64].im.abs() < 1e-12,
        "flat R = 15 dB must give a real T = 10^(-0.75): {}",
        t_ref[64]
    );

    let opaque = run_screen("terrain_screen_thin_case71.toml", None, false);
    let semi = run_screen("terrain_screen_thin_case71.toml", Some(&iso), false);

    let mut deep_engaged = 0usize;
    let mut engaged = 0usize;
    for (f, &t) in t_ref.iter().enumerate() {
        let ho = opaque.h_coh_factor[f];
        let hs = semi.h_coh_factor[f];
        let diff = hs - ho;
        // diff = r_scr1·T with T real ⇒ r_scr1 = diff.re / T.re, and diff.im ≈ 0.
        let r_scr1 = diff.re / t.re;
        assert!(
            (-1e-9..=1.0 + 1e-9).contains(&r_scr1),
            "band {f}: diff must be r_scr1·T_native (r_scr1 = {r_scr1})"
        );
        assert!(
            diff.im.abs() < 1e-9,
            "band {f}: flat-R T is real ⇒ diff must stay real (im = {})",
            diff.im
        );
        if (r_scr1 - 1.0).abs() < 1e-9 {
            engaged += 1;
            // (b) exact native T where the screen branch is fully engaged.
            assert!(
                (diff - t).norm() < 1e-9,
                "band {f}: (h_semi − h_opaque) must equal T_native at full engagement: {diff} vs {t}"
            );
            // (a) deep shadow rises: |h_opaque| ≪ |T| ⇒ adding T raises the level.
            if ho.norm() < 0.05 {
                deep_engaged += 1;
                assert!(
                    hs.norm() > ho.norm(),
                    "band {f}: transmission must raise the deep-shadow level ({} !> {})",
                    hs.norm(),
                    ho.norm()
                );
            }
        }
    }
    assert!(
        engaged >= 10,
        "expected a run of fully-engaged high-frequency bands (got {engaged})"
    );
    assert!(
        deep_engaged > 0,
        "expected at least one deep-shadow, fully-engaged band (got {deep_engaged})"
    );
}

/// T8 (two-channel contract, SC2 / Pitfall 6): enabling the partition spectrum
/// leaves the incoherent channel `p_incoh` BIT-FOR-BIT unchanged — transmission
/// joins the coherent factor only and never touches `p_incoh` or the Sub-model 7
/// scattered energy.
///
/// Deviation note: the plan's literal phrasing was "assert `p_incoh.to_bits() ==
/// 0.0` per band", assuming a zero-turbulence screen yields exact-zero `p_incoh`.
/// It cannot: Sub-model 7 floors the shadow at `NO_SCATTER_DB = −300 dB`, so a
/// screen's `p_incoh` carries `10^(−30) ≈ 1e-30` (never exact-zero) regardless of
/// transmission. The exact, achievable, and STRONGER pin of the same contract is
/// bit-identity between the opaque and semi runs — transmission perturbs
/// `p_incoh` by not one bit. (Same class of plan-test-spec correction as plan
/// 05-02's T3.)
#[test]
fn transmission_never_touches_the_incoherent_channel() {
    let iso = IsolationSpectrum::new([12.0; N_BANDS]).unwrap();
    // Zero turbulence removes the Sub-model 7 scatter and the turbulence
    // decorrelation; any surviving p_incoh is the coherent-path's own Δτ residual,
    // which transmission (a coherent-channel-only add) must leave UNTOUCHED.
    let opaque = run_screen("terrain_screen_thin_case71.toml", None, true);
    let semi = run_screen("terrain_screen_thin_case71.toml", Some(&iso), true);
    // The coherent factor genuinely moves (the contract is not vacuous)...
    let mut coherent_moved = false;
    for f in 0..N_BANDS {
        if (semi.h_coh_factor[f] - opaque.h_coh_factor[f]).norm() > 1e-6 {
            coherent_moved = true;
        }
        // ...while p_incoh stays bit-for-bit identical — transmission is
        // coherent-channel-only (SC2, Pitfall 6).
        assert_eq!(
            opaque.p_incoh[f].to_bits(),
            semi.p_incoh[f].to_bits(),
            "band {f}: transmission must not touch p_incoh (bit-for-bit; T is coherent-only)"
        );
    }
    assert!(
        coherent_moved,
        "transmission must actually move the coherent channel (else the p_incoh identity is vacuous)"
    );
}

/// T9 (typed error, threat T-05-03-04): an isolation spectrum over flat terrain
/// (no partition on the path) is a typed `IsolationWithoutScreen` error — never a
/// silent no-op.
#[test]
fn isolation_over_flat_terrain_is_a_typed_error() {
    let iso = IsolationSpectrum::new([20.0; N_BANDS]).unwrap();
    let path = repo_root().join("cases").join("terrain_flat_sigma200.toml");
    let case = envi_harness::cases::toml::load_toml_case(&path).expect("flat case must load");
    let (profile, src, rcv, coh) = build_terrain_inputs(&case).expect("flat inputs");
    let axis = &*FREQ_AXIS;
    let err = terrain_effect(&profile, src, rcv, coh.c0, &coh, axis, None, Some(&iso))
        .expect_err("isolation over flat terrain must be a typed error");
    assert!(
        matches!(err, PropagationError::IsolationWithoutScreen),
        "flat + isolation must be IsolationWithoutScreen, got {err:?}"
    );
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
