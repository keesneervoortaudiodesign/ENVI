//! Forest excess attenuation — AV 1106/07 §5.19 Sub-Model 10 "Scattering Zones"
//! (Eqs. 288–291, Tables 8/9), engine-owned pure math.
//!
//! # What this is (and what the roadmap paraphrase actually names)
//!
//! REQUIREMENTS ENG-09 and the roadmap describe a forest term "A = d·a(f) from
//! mean tree density, mean stem radius, factor kₚ and mean absorption
//! coefficient". Nord2000 defines **no per-metre `a(f)`**: that phrase is the
//! NoizCalc UI paraphrase of AV 1106/07 §5.19 Sub-Model 10, whose parameter list
//! (density n″, stem radius a, "kₚ" ≡ the tabulated weighting k_f, absorption α)
//! matches 1:1. The actual law is
//!
//! ```text
//! nQ   = 2·a·n″                                    (Eq. 290)
//! T    = Min( (R_sc·nQ / 1.75)² , 1 )              (Eq. 289)
//! A_e  = ΔL(h′, α, R′) + 20·log₁₀(8·R′)            (Eq. 291 inner; Table 9)
//! ΔL_s = Max( 1.25·k_f·T·A_e , −15 )               (Eq. 291)
//! ```
//!
//! with `k_f` from Table 8 (a function of `ka = 2π·f·a/c₀`, linear-interpolated),
//! `h′ = nQ·h`, `R′ = nQ·R_sc`. It is quadratic-then-saturating in the crossing
//! length `R_sc`, floored at −15 dB, and exactly `0` below `ka ≤ 0.7`. This is
//! Nord2000's own bounding of the crossing length — the ISO 9613-2
//! 10/20/200 m distance-clamp regimes are a DIFFERENT (ISO) model and are
//! deliberately NOT imported here.
//!
//! Equations and both tables were verified against the AV 1106/07 rev. 4 page
//! images (pp. 125–127) during 05-RESEARCH (2026-07-09); cited by report + page
//! only (licensing rule — transcribed table *data* is the Sub-Model 7 precedent).
//!
//! # Placement and the two-channel contract (D-02/D-03/D-04)
//!
//! `ΔL_s` is a **real per-band dB effect** (return type `f64` — it is
//! type-incapable of carrying phase). The solver applies it post-conj as a real
//! magnitude factor on BOTH channels — `10^{ΔL_s/20}` on `H_coh` (argument
//! untouched) and `10^{ΔL_s/10}` on `P_incoh_abs` — exactly like
//! `directivity_gain_db`. Forest is therefore an engine-root module (like
//! `directivity`), **never** a `propagation/` operator: it never touches the
//! Nord2000-native `e^{−jωt}` side and the single-`.conj()` quarantine is
//! unaffected. Because `ΔL_s ≤ 0`, forest can only attenuate; a real scale of an
//! exactly-zero `P_incoh` stays exactly zero (`F→1 ⇒ P_incoh→0` preserved).
//!
//! # Deferred: the Fs coherence factor (Eq. 288) — documented seam, not dropped
//!
//! Nord2000 ALSO reduces the terrain-effect coherence via `Fs = 1 − k_f·T`
//! (Eq. 288), which enters the overall coherence coefficient
//! `F = Ff·FΔν·Fc·Fr·Fs` (Eq. 110). That is a *decorrelation* mechanism, which
//! the locked D-03 scope ("forest is excess attenuation, NOT decorrelation
//! routed into the incoherent channel") deliberately excludes for Phase 5. `Fs`
//! is therefore **deferred with a documented seam**: `CoherenceInputs` already
//! carries a caller-multiplied `f_delta_nu`-style factor into which a future
//! `Fs = 1 − k_f·T` multiply can land without structural change (the screen
//! engine's F₂/F₃/F₄ would need the same multiply). This mirrors the Phase-4
//! directional-phase-seam discipline; see plan 05-03's `deferred-items.md`. It
//! is NOT silently dropped — only its coherence-reduction channel is out of
//! scope until real forest crossings land (Phase 9).
//!
//! # Assumptions (05-RESEARCH A1–A3)
//!
//! - **A1:** Table 9's "cubic interpolation" is realized as a tensor-product
//!   monotone cubic (PCHIP, scipy `PchipInterpolator`-equivalent), nested
//!   R′ → α → log₁₀(h′). PCHIP cannot overshoot the tabulated values, so no
//!   spurious positive `A_e` (a forest that amplifies) can appear.
//! - **A2:** `k_f` is edge-clamped: `ka ≤ 0.7 ⇒ 0` (exact), `ka ≥ 20 ⇒ 1.0`.
//! - **A3:** `R′` is clamped to `[0.0625, 10]` **consistently** in BOTH the
//!   Table-9 lookup AND the `20·log₁₀(8·R′)` term (clamping only one side
//!   manufactures spurious attenuation); the −15 dB floor dominates beyond
//!   anyway.

use std::f64::consts::PI;

use thiserror::Error;

/// Table 8 `ka` axis (AV 1106/07 p. 125, verified against the page image
/// 2026-07-09). `ka = 2π·f·a / c₀`.
const KA_AXIS: [f64; 8] = [0.0, 0.7, 1.0, 1.5, 3.0, 5.0, 10.0, 20.0];
/// Table 8 `k_f` weighting values paired with [`KA_AXIS`]. `ka ≤ 0.7 ⇒ 0`
/// (exact), `ka ≥ 20 ⇒ 1.0` (edge clamp, A2).
const KF_VALS: [f64; 8] = [0.00, 0.00, 0.05, 0.20, 0.70, 0.82, 0.95, 1.00];

/// Table 9 `R′` (normalized effective distance) row axis.
const R_NORM_AXIS: [f64; 12] = [
    0.0625, 0.125, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0,
];
/// Table 9 `h′` (normalized scatter-obstacle height) axis. Interpolated on
/// `log₁₀(h′)` — the nodes are exact decades.
const H_NORM_AXIS: [f64; 3] = [0.01, 0.1, 1.0];
/// Table 9 `α` (absorption) axis, edge-clamped to `[0, 0.4]` at lookup.
const ALPHA_AXIS: [f64; 3] = [0.0, 0.2, 0.4];

/// Table 9 `ΔL(h′, α, R′)` in dB, indexed `[h_idx][alpha_idx][r_idx]`
/// (AV 1106/07 pp. 126–127 Table 9, verified against page images 2026-07-09).
const TABLE9: [[[f64; 12]; 3]; 3] = [
    // h′ = 0.01
    [
        // α = 0.0
        [
            6.0, 0.0, -7.5, -14.0, -18.0, -21.5, -26.3, -31.0, -40.0, -49.5, -67.0, -102.5,
        ],
        // α = 0.2
        [
            6.0, 0.0, -7.5, -14.25, -18.8, -22.5, -27.5, -32.5, -42.5, -52.5, -72.5, -113.0,
        ],
        // α = 0.4
        [
            6.0, 0.0, -7.5, -14.5, -19.5, -23.5, -29.5, -34.5, -45.5, -56.3, -78.0, -122.5,
        ],
    ],
    // h′ = 0.1
    [
        // α = 0.0
        [
            6.0, 0.0, -6.0, -12.5, -17.3, -20.5, -25.5, -30.0, -37.5, -45.5, -62.0, -94.7,
        ],
        // α = 0.2
        [
            6.0, 0.0, -7.0, -13.5, -18.0, -21.6, -27.2, -32.0, -40.5, -49.5, -67.0, -103.7,
        ],
        // α = 0.4
        [
            6.0, 0.0, -7.5, -14.5, -19.0, -22.8, -29.0, -33.3, -42.9, -52.5, -72.0, -112.0,
        ],
    ],
    // h′ = 1.0
    [
        // α = 0.0
        [
            6.0, 0.0, -6.0, -12.5, -16.0, -19.3, -24.0, -27.5, -34.2, -40.4, -52.5, -78.8,
        ],
        // α = 0.2
        [
            6.0, 0.0, -7.0, -13.0, -16.8, -20.5, -25.5, -29.5, -36.0, -42.8, -56.2, -84.0,
        ],
        // α = 0.4
        [
            6.0, 0.0, -7.5, -14.0, -17.7, -21.3, -26.3, -30.8, -37.8, -45.5, -60.0, -89.7,
        ],
    ],
];

/// The Eq. 291 lower floor on `ΔL_s`, dB (AV 1106/07 p. 126: "ΔL_s has been
/// limited downwards to a value of −15 dB").
const DELTA_L_FLOOR_DB: f64 = -15.0;

/// Errors from constructing a [`ForestCrossing`].
///
/// Physical parameters are caller-controlled (a future service caller crosses
/// this trust boundary, threat T-05-01-01); every malformed value yields a typed
/// error — the constructor never panics on data, and NaN/±Inf can never enter
/// the tensor as a poisoned `ΔL_s`.
#[derive(Debug, Error, PartialEq)]
pub enum ForestError {
    /// The crossing length `d_m` (`R_sc`) was negative or non-finite.
    #[error("forest crossing length d_m = {d_m} m must be finite and ≥ 0")]
    InvalidLength {
        /// The offending value.
        d_m: f64,
    },
    /// The tree density `n″` was negative or non-finite.
    #[error("forest density = {density} m⁻² must be finite and ≥ 0")]
    InvalidDensity {
        /// The offending value.
        density: f64,
    },
    /// The mean stem radius `a` was non-positive or non-finite.
    #[error("forest stem radius = {stem_radius} m must be finite and > 0")]
    InvalidStemRadius {
        /// The offending value.
        stem_radius: f64,
    },
    /// The mean tree height `h` was non-positive or non-finite.
    #[error("forest height = {height} m must be finite and > 0")]
    InvalidHeight {
        /// The offending value.
        height: f64,
    },
    /// The absorption `α` was outside `[0, 1]` or non-finite.
    #[error("forest absorption = {absorption} must be finite and in [0, 1]")]
    InvalidAbsorption {
        /// The offending value.
        absorption: f64,
    },
}

/// A single forest (scattering-zone) crossing of a propagation path
/// (AV 1106/07 §5.19 Sub-Model 10 inputs).
///
/// Carries the pre-computed through-forest path length `d_m` (= `R_sc`, a
/// Phase-9 geometry concern upstream) and the physical scatter parameters. The
/// engine owns the SM10 formula (D-02) — this struct carries only raw inputs.
///
/// # Interface amendment vs CONTEXT D-01 (research-mandated, 05-RESEARCH F1)
///
/// D-01's locked field list `{d_m, density, stem_radius, kₚ, absorption}` is
/// amended: **`kₚ` is dropped** — TI 386's "factor kₚ" IS Table 8's `k_f`, a
/// tabulated function of `ka` the engine computes (precisely D-02's "engine owns
/// the formula", A5) — and **`height_m` is added**, since `h′ = nQ·h` is
/// un-evaluable without the average tree height (Pitfall 5). Phase-7 SCN-04
/// already carries height, so this is downstream-consistent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForestCrossing {
    /// `R_sc` — through-forest path length, m (pre-computed upstream).
    pub d_m: f64,
    /// `n″` — mean tree density, m⁻².
    pub density_per_m2: f64,
    /// `a` — mean stem radius, m.
    pub stem_radius_m: f64,
    /// `α` — mean absorption coefficient of the scatter obstacles (Table 9
    /// axis; "normally 0.1–0.4", AV 1106/07 p. 126). Accepted range `[0, 1]`,
    /// edge-clamped to the table domain `[0, 0.4]` at lookup.
    pub absorption: f64,
    /// `h` — mean tree height, m (required for `h′ = nQ·h`).
    pub height_m: f64,
}

impl ForestCrossing {
    /// Validate and construct a forest crossing.
    ///
    /// # Errors
    ///
    /// A typed [`ForestError`] if any parameter is non-finite, if `d_m` or
    /// `density_per_m2` is negative, if `stem_radius_m` or `height_m` is
    /// non-positive, or if `absorption` is outside `[0, 1]`. Non-finite values
    /// are rejected explicitly (a bare comparison lets `+∞` through).
    pub fn new(
        d_m: f64,
        density_per_m2: f64,
        stem_radius_m: f64,
        absorption: f64,
        height_m: f64,
    ) -> Result<Self, ForestError> {
        if !d_m.is_finite() || d_m < 0.0 {
            return Err(ForestError::InvalidLength { d_m });
        }
        if !density_per_m2.is_finite() || density_per_m2 < 0.0 {
            return Err(ForestError::InvalidDensity {
                density: density_per_m2,
            });
        }
        if !stem_radius_m.is_finite() || stem_radius_m <= 0.0 {
            return Err(ForestError::InvalidStemRadius {
                stem_radius: stem_radius_m,
            });
        }
        if !height_m.is_finite() || height_m <= 0.0 {
            return Err(ForestError::InvalidHeight { height: height_m });
        }
        if !absorption.is_finite() || !(0.0..=1.0).contains(&absorption) {
            return Err(ForestError::InvalidAbsorption { absorption });
        }
        Ok(Self {
            d_m,
            density_per_m2,
            stem_radius_m,
            absorption,
            height_m,
        })
    }
}

/// Table 8 frequency weighting `k_f(ka)` — linear interpolation between nodes,
/// edge-clamped (`ka ≤ 0.7 ⇒ exactly 0`, `ka ≥ 20 ⇒ 1.0`; A2).
fn table8_kf(ka: f64) -> f64 {
    if ka <= KA_AXIS[0] {
        return KF_VALS[0];
    }
    let last = KA_AXIS.len() - 1;
    if ka >= KA_AXIS[last] {
        return KF_VALS[last];
    }
    let mut i = 0;
    while i < last && ka > KA_AXIS[i + 1] {
        i += 1;
    }
    let t = (ka - KA_AXIS[i]) / (KA_AXIS[i + 1] - KA_AXIS[i]);
    KF_VALS[i] + t * (KF_VALS[i + 1] - KF_VALS[i])
}

/// Three-valued sign (matching numpy `sign`: `+1 / 0 / −1`).
///
/// `f64::signum` returns `±1.0` for `±0.0` and never `0`, so the scipy
/// PCHIP shape-preservation conditions (which special-case a zero secant)
/// need this explicit three-valued sign.
fn sgn(x: f64) -> i32 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// One-sided three-point endpoint derivative with the scipy PCHIP shape
/// fixups (`scipy.interpolate.PchipInterpolator._edge_case`): `h0`/`m0` are the
/// nearest interval width / secant, `h1`/`m1` the next-nearest.
fn pchip_edge_derivative(h0: f64, h1: f64, m0: f64, m1: f64) -> f64 {
    let d = ((2.0 * h0 + h1) * m0 - h0 * m1) / (h0 + h1);
    if sgn(d) != sgn(m0) {
        0.0
    } else if sgn(m0) != sgn(m1) && d.abs() > 3.0 * m0.abs() {
        3.0 * m0
    } else {
        d
    }
}

/// Monotone cubic (PCHIP) interpolation on `(xs, ys)` at `x`, evaluated as a
/// cubic Hermite segment — the scipy `PchipInterpolator`-equivalent
/// Fritsch–Carlson–Butland scheme (A1). The query is edge-clamped to
/// `[xs[0], xs[last]]` (no extrapolation). `xs` must be strictly ascending with
/// `3 ≤ xs.len() ≤ 12`.
fn pchip_interp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    debug_assert!((3..=12).contains(&n) && ys.len() == n);
    let xq = x.clamp(xs[0], xs[n - 1]);

    // Interval widths h_k and secants δ_k.
    let mut h = [0.0f64; 12];
    let mut delta = [0.0f64; 12];
    for k in 0..n - 1 {
        h[k] = xs[k + 1] - xs[k];
        delta[k] = (ys[k + 1] - ys[k]) / h[k];
    }

    // Node derivatives d_k.
    let mut d = [0.0f64; 12];
    for k in 1..n - 1 {
        let dkm1 = delta[k - 1];
        let dk = delta[k];
        if dkm1 == 0.0 || dk == 0.0 || sgn(dkm1) != sgn(dk) {
            d[k] = 0.0;
        } else {
            let w1 = 2.0 * h[k] + h[k - 1];
            let w2 = h[k] + 2.0 * h[k - 1];
            d[k] = (w1 + w2) / (w1 / dkm1 + w2 / dk);
        }
    }
    d[0] = pchip_edge_derivative(h[0], h[1], delta[0], delta[1]);
    d[n - 1] = pchip_edge_derivative(h[n - 2], h[n - 3], delta[n - 2], delta[n - 3]);

    // Locate the segment [xs[i], xs[i+1]] containing xq.
    let mut i = 0;
    while i < n - 2 && xq > xs[i + 1] {
        i += 1;
    }
    let hh = h[i];
    let t = (xq - xs[i]) / hh;
    let t2 = t * t;
    let t3 = t2 * t;
    // Cubic Hermite basis.
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * ys[i] + h10 * hh * d[i] + h01 * ys[i + 1] + h11 * hh * d[i + 1]
}

/// Table 9 lookup `ΔL(h′, α, R′)` in dB via tensor-product PCHIP with the fixed
/// nesting order R′ → α → log₁₀(h′) (A1). `h′` is edge-clamped to `[0.01, 1]`
/// (via `log₁₀`), `α` to `[0, 0.4]`; `R′` is expected pre-clamped to
/// `[0.0625, 10]` by the caller (and re-clamped here, a no-op).
fn table9_delta_l(h_norm: f64, alpha: f64, r_norm: f64) -> f64 {
    // Step 1: PCHIP along R′ for each of the 9 (h′, α) columns.
    let mut grid = [[0.0f64; 3]; 3]; // [h_idx][alpha_idx]
    for (hi, block) in TABLE9.iter().enumerate() {
        for (ai, col) in block.iter().enumerate() {
            grid[hi][ai] = pchip_interp(&R_NORM_AXIS, col, r_norm);
        }
    }
    // Step 2: PCHIP along α for each h′.
    let mut col = [0.0f64; 3];
    for (hi, g) in grid.iter().enumerate() {
        col[hi] = pchip_interp(&ALPHA_AXIS, g, alpha);
    }
    // Step 3: PCHIP along log₁₀(h′) (exact decades) at log₁₀(h_norm).
    let log_h_axis = [
        H_NORM_AXIS[0].log10(),
        H_NORM_AXIS[1].log10(),
        H_NORM_AXIS[2].log10(),
    ];
    pchip_interp(&log_h_axis, &col, h_norm.log10())
}

/// Sub-Model 10 forest excess attenuation `ΔL_s(f) ≤ 0` dB for one crossing
/// (AV 1106/07 Eqs. 288–291). Infallible on a validated [`ForestCrossing`]:
/// the `f64` return type is type-incapable of carrying phase (D-03/D-04).
///
/// `f_hz` MUST be an exact grid centre (`axis.centres[i]`), never a nominal
/// label; `c0` is the sound speed the terrain effect uses (`coh.c0`).
#[must_use]
pub fn forest_delta_l(f_hz: f64, fc: &ForestCrossing, c0: f64) -> f64 {
    let n_q = 2.0 * fc.stem_radius_m * fc.density_per_m2; // Eq. 290
    let t = ((fc.d_m * n_q) / 1.75).powi(2).min(1.0); // Eq. 289
    let ka = 2.0 * PI * f_hz * fc.stem_radius_m / c0;
    let kf = table8_kf(ka);
    // Exact zero: no scattering below ka = 0.7, or no crossing (T = 0). No
    // table math is constructed — F1's analytic zero is bit-exact.
    if kf == 0.0 || t == 0.0 {
        return 0.0;
    }
    let h_norm = n_q * fc.height_m; // h′ = nQ·h
    // R′ clamped consistently in BOTH the table lookup and the log term (A3).
    let r_norm = (n_q * fc.d_m).clamp(0.0625, 10.0);
    let a_e = table9_delta_l(h_norm, fc.absorption, r_norm) + 20.0 * (8.0 * r_norm).log10();
    (1.25 * kf * t * a_e).max(DELTA_L_FLOOR_DB) // Eq. 291
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freq::FreqAxis;

    const C0: f64 = 340.348;

    /// F1: below `ka = 0.7` every band returns EXACTLY 0.0 (analytic anchor, no
    /// tolerance). With `a = 0.1 m`, `ka ≤ 0.7 ⇔ f ≤ 0.7·c₀/(2π·a) ≈ 379 Hz`.
    #[test]
    fn f1_below_ka_threshold_is_exactly_zero() {
        let axis = FreqAxis::new();
        let a = 0.1;
        let fc = ForestCrossing::new(50.0, 0.5, a, 0.2, 10.0).unwrap();
        let f_threshold = 0.7 * C0 / (2.0 * PI * a);
        let mut saw_low = false;
        for &f in axis.centres.iter() {
            let ka = 2.0 * PI * f * a / C0;
            if ka <= 0.7 {
                saw_low = true;
                assert_eq!(
                    forest_delta_l(f, &fc, C0),
                    0.0,
                    "ka = {ka} (f = {f} ≤ {f_threshold}) must give exactly 0.0 dB"
                );
            }
        }
        assert!(
            saw_low,
            "grid must contain bands below the ka = 0.7 threshold"
        );
    }

    /// F2: on-node hand anchor, ZERO interpolation anywhere. At 1 kHz with
    /// `ka = 3` (node, k_f = 0.70), `R′ = 1` (node), `h′ = 0.1` (node), `α = 0`
    /// (node): `ΔL_s = 1.25·0.70·(1/1.75)²·(−20.5 + 20·log₁₀(8))`. The expected
    /// value is COMPUTED from the formula in-test, never a hardcoded decimal
    /// (05-RESEARCH's printed anchor digits contained a transcription typo; the
    /// formula is the source of truth here).
    #[test]
    fn f2_on_node_hand_anchor() {
        let axis = FreqAxis::new();
        let f = axis.centres[64]; // exactly 1000 Hz
        // a chosen so ka = 3 exactly at 1 kHz.
        let a = 3.0 * C0 / (2.0 * PI * 1000.0);
        // nQ = 1 ⇒ R′ = nQ·d = 1, h′ = nQ·h = 0.1.
        let n_q = 1.0_f64;
        let density = n_q / (2.0 * a);
        let d = 1.0 / n_q;
        let h = 0.1 / n_q;
        let fc = ForestCrossing::new(d, density, a, 0.0, h).unwrap();

        let t = (1.0_f64 / 1.75).powi(2);
        let expected = 1.25 * 0.70 * t * (-20.5 + 20.0 * 8.0_f64.log10());
        let got = forest_delta_l(f, &fc, C0);
        assert!(
            (got - expected).abs() < 1e-12,
            "on-node anchor: got {got}, expected {expected}"
        );
    }

    /// F3: a very large crossing floors at exactly −15.0 dB, and `ΔL_s` is
    /// monotone non-increasing in `d_m` until the floor engages.
    #[test]
    fn f3_floor_and_monotone_in_length() {
        let axis = FreqAxis::new();
        let f = axis.centres[96]; // a high band (ka well into scattering)
        let a = 0.15;
        let density = 0.3;
        let mut prev = f64::INFINITY;
        let mut hit_floor = false;
        for step in 0..200 {
            let d = 0.5 + step as f64 * 2.0;
            let fc = ForestCrossing::new(d, density, a, 0.2, 12.0).unwrap();
            let dls = forest_delta_l(f, &fc, C0);
            assert!(
                dls <= prev + 1e-12,
                "ΔL_s must be non-increasing in d: {dls} > {prev} at d = {d}"
            );
            assert!(dls >= DELTA_L_FLOOR_DB, "floor breached: {dls} at d = {d}");
            if dls == DELTA_L_FLOOR_DB {
                hit_floor = true;
            }
            prev = dls;
        }
        assert!(
            hit_floor,
            "a large crossing must reach the −15 dB floor exactly"
        );
    }

    /// F4: randomized sweep — every `ΔL_s` is finite, `≤ 0.01` (Pitfall 4
    /// interpolation-corner tolerance), and `≥ −15.0` exactly.
    #[test]
    fn f4_parameter_sweep_stays_in_bounds() {
        let axis = FreqAxis::new();
        // Deterministic LCG (no rand dependency in the engine).
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 11) as f64) / ((1u64 << 53) as f64)
        };
        for _ in 0..20_000 {
            let d = next() * 300.0;
            let density = next() * 1.0;
            let a = 0.02 + next() * 0.4;
            let alpha = next() * 1.0;
            let h = 0.5 + next() * 30.0;
            let bi = (next() * 105.0) as usize % 105;
            let f = axis.centres[bi];
            let fc = ForestCrossing::new(d, density, a, alpha, h).unwrap();
            let dls = forest_delta_l(f, &fc, C0);
            assert!(dls.is_finite(), "ΔL_s not finite: {dls}");
            assert!(dls <= 0.01, "ΔL_s should be ≤ 0.01, got {dls}");
            assert!(dls >= DELTA_L_FLOOR_DB, "ΔL_s below floor: {dls}");
        }
    }

    /// Table 8 endpoints and a node are exact; the low clamp is a hard zero.
    #[test]
    fn table8_nodes_and_clamps() {
        assert_eq!(table8_kf(0.0), 0.0);
        assert_eq!(table8_kf(0.5), 0.0); // ≤ 0.7 clamp
        assert_eq!(table8_kf(0.7), 0.0);
        assert_eq!(table8_kf(3.0), 0.70);
        assert_eq!(table8_kf(20.0), 1.0);
        assert_eq!(table8_kf(50.0), 1.0); // ≥ 20 clamp
        // Midpoint linear interp between ka = 1 (0.05) and ka = 1.5 (0.20).
        assert!((table8_kf(1.25) - 0.125).abs() < 1e-12);
    }

    /// Table 9 reproduces tabulated values exactly on its nodes.
    #[test]
    fn table9_on_nodes_is_exact() {
        // (h′ = 0.1, α = 0, R′ = 1) ⇒ −20.5 (the F2 node).
        assert!((table9_delta_l(0.1, 0.0, 1.0) - (-20.5)).abs() < 1e-9);
        // (h′ = 0.01, α = 0.4, R′ = 2) ⇒ −34.5.
        assert!((table9_delta_l(0.01, 0.4, 2.0) - (-34.5)).abs() < 1e-9);
        // (h′ = 1, α = 0.2, R′ = 0.0625) ⇒ 6.0 (flat first row).
        assert!((table9_delta_l(1.0, 0.2, 0.0625) - 6.0).abs() < 1e-9);
    }

    /// The constructor rejects every degenerate/non-finite input as a typed
    /// error (T-05-01-01), never a panic.
    #[test]
    fn constructor_rejects_bad_inputs() {
        assert!(matches!(
            ForestCrossing::new(-1.0, 0.5, 0.1, 0.2, 10.0),
            Err(ForestError::InvalidLength { .. })
        ));
        assert!(matches!(
            ForestCrossing::new(50.0, -0.1, 0.1, 0.2, 10.0),
            Err(ForestError::InvalidDensity { .. })
        ));
        assert!(matches!(
            ForestCrossing::new(50.0, 0.5, 0.0, 0.2, 10.0),
            Err(ForestError::InvalidStemRadius { .. })
        ));
        assert!(matches!(
            ForestCrossing::new(50.0, 0.5, 0.1, 0.2, 0.0),
            Err(ForestError::InvalidHeight { .. })
        ));
        assert!(matches!(
            ForestCrossing::new(50.0, 0.5, 0.1, 1.5, 10.0),
            Err(ForestError::InvalidAbsorption { .. })
        ));
        assert!(matches!(
            ForestCrossing::new(50.0, 0.5, 0.1, -0.1, 10.0),
            Err(ForestError::InvalidAbsorption { .. })
        ));
        // Non-finite inputs are rejected explicitly (a bare `< 0` lets +∞ pass).
        assert!(ForestCrossing::new(f64::NAN, 0.5, 0.1, 0.2, 10.0).is_err());
        assert!(ForestCrossing::new(f64::INFINITY, 0.5, 0.1, 0.2, 10.0).is_err());
        assert!(ForestCrossing::new(50.0, f64::INFINITY, 0.1, 0.2, 10.0).is_err());
        assert!(ForestCrossing::new(50.0, 0.5, f64::NAN, 0.2, 10.0).is_err());
        assert!(ForestCrossing::new(50.0, 0.5, 0.1, f64::NAN, 10.0).is_err());
        assert!(ForestCrossing::new(50.0, 0.5, 0.1, 0.2, f64::INFINITY).is_err());
        // A valid crossing constructs.
        assert!(ForestCrossing::new(50.0, 0.5, 0.1, 0.2, 10.0).is_ok());
    }

    /// `d_m = 0` (or density 0) ⇒ T = 0 ⇒ exactly 0.0 dB, no table math.
    #[test]
    fn zero_crossing_is_exactly_zero() {
        let axis = FreqAxis::new();
        let f = axis.centres[96];
        let fc = ForestCrossing::new(0.0, 0.5, 0.15, 0.2, 10.0).unwrap();
        assert_eq!(forest_delta_l(f, &fc, C0), 0.0);
        let fc2 = ForestCrossing::new(50.0, 0.0, 0.15, 0.2, 10.0).unwrap();
        assert_eq!(forest_delta_l(f, &fc2, C0), 0.0);
    }
}
