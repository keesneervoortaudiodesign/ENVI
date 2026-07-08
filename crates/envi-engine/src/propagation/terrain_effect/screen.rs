//! Generic screen⇄ground engine — Sub-models 4/5/6 (AV 1106/07 §5.13–5.15,
//! Eqs. 157–270).
//!
//! One engine, three kernels. Sub-model 4 (single screen, one edge) uses the
//! `pwedge` diffraction kernel and a four-path image model (Eq. 157); Sub-model 5
//! (thick screen, two edges of one body) swaps in `p2edge`; Sub-model 6 (two
//! screens) uses `p2wedge` with an eight-ray set (Eq. 222) — the four-path set
//! crossed with a middle-region reflection, encoded as a three-region bitmask.
//!
//! # The user-locked two-channel contract (PROJECT.md Key Decisions 2026-07-07)
//!
//! Every screen⇄ground path stays a **complex** pressure. The full screen factor
//! handed upward is
//!
//! ```text
//! ĥ = (p̂₁ / p̂₀) · Δp̂_G       (complex, phase LIVE)
//! ```
//!
//! where `p̂₁ = ` the diffracted pressure over the top (kernel call), `p̂₀ =
//! e^{+jωτ_SR}/R_SR` the free-field direct pressure (Nord-native e^{−jωt}), so
//! `|p̂₁/p̂₀| = |p̂₁|·R_SR = Δp_SCR` reproduces Eq. 163 exactly while its phase
//! carries the over-the-top path delay. `Δp̂_G` is the F-weighted **coherent**
//! four-ray sum (Eq. 182 first term). The turbulence-decorrelated `(1−F²)`
//! residuals live only in [`GroundResult::p_incoh`] — never touching phase.
//!
//! The band value is `10·lg(|ĥ|² + p_incoh)`, and `|ĥ|` equals Nord2000's
//! Eq. 188 level magnitude at 1e-12 (a unit-tested contract pin). Eq. 188's
//! `20·lg| p̂_SCR · ∏∏ |p̂_{G,i1,i2}|^{w′_i1 w′_i2} |` weighted-geometric-mean of
//! ground magnitudes is realized by combining the per-combination energies with
//! the same exponents; the coherent/incoherent split is preserved through the
//! geometric mean (both channels use the same weights, so their sum reproduces
//! the document's magnitude exactly).
//!
//! # Convention & scope
//!
//! Nord2000-native (e^{−jωt}); no `conj()` here (that boundary is plan 02-05).
//! Shadow-zone branches (Eqs. 184–186) are ξ<0 refraction — Phase 3; the
//! non-shadow branch is always taken here (`dSZ = ∞`), documented at the seam.

use num_complex::Complex;
use std::f64::consts::{FRAC_PI_2, TAU};

use super::GroundResult;
use crate::geometry::{image_point, segment_variables};
use crate::propagation::PropagationError;
use crate::propagation::coherence::{CoherenceInputs, coherence_ff};
use crate::propagation::diffraction::{
    TwoWedgeGeometry, TwoWedgeImpedances, WedgeGeometry, WedgePrimary, p2edge, p2wedge, pwedge,
};
use crate::propagation::fresnel::fresnel_zone_w;
use crate::propagation::ground::{ground_impedance, incoherent_rho, spherical_q};
use crate::propagation::rays::straight_rays_over_segment;
use crate::propagation::special::exp_clamped;

const MIN_LEN: f64 = 1e-9;

/// A reflecting ground strip: the (possibly sloped) terrain segment line plus
/// its acoustic surface properties. Screen-shape segments are excluded from
/// these lists (they form the wedge, not a reflector).
#[derive(Debug, Clone, Copy)]
pub struct SurfaceStrip {
    /// Segment start point `[x, z]` (m).
    pub seg_a: [f64; 2],
    /// Segment end point `[x, z]` (m).
    pub seg_b: [f64; 2],
    /// Flow resistivity σ, kPa·s·m⁻².
    pub sigma_kpa: f64,
    /// Roughness `r`, meters (feeds `Fr`; `0` for Phase 2 targets).
    pub roughness_r: f64,
}

/// Inputs to a screen sub-model: source/receiver, the reduced screen shape
/// (`≤3` points per edge), the reflecting strips before/(middle/)after the
/// screen, the wedge-face impedances (σ of the segments adjacent to the edge)
/// and the coherence weather inputs.
#[derive(Debug, Clone, Copy)]
pub struct ScreenConfig<'a> {
    /// Source `[x, z]` (m).
    pub source: [f64; 2],
    /// Receiver `[x, z]` (m).
    pub receiver: [f64; 2],
    /// Screen shape points: `[W₁, T, W₂]` (SM4, one edge) or
    /// `[W₁, T₁, T₂, W₂]` (SM5 thick / SM6 two-screen). `W₁`/`W₂` are the
    /// source-/receiver-side wedge bases; `T`(₁,₂) the diffracting edge(s).
    pub screen: &'a [[f64; 2]],
    /// Reflecting strips on the source side of the screen.
    pub before: &'a [SurfaceStrip],
    /// Reflecting strips between the two screens (SM6 middle region; empty
    /// otherwise).
    pub middle: &'a [SurfaceStrip],
    /// Reflecting strips on the receiver side of the screen.
    pub after: &'a [SurfaceStrip],
    /// Source-side wedge-face impedance `Ẑ_S` (Nord-native, `Im > 0`).
    pub z_face_source: Complex<f64>,
    /// Receiver-side wedge-face impedance `Ẑ_R`.
    pub z_face_receiver: Complex<f64>,
    /// Coherence weather/turbulence inputs (`c0`, `Cv²`, `CT²`, …).
    pub coh: &'a CoherenceInputs,
}

/// A per-ray diffraction kernel: the diffracted complex pressure for a source
/// point `s` and receiver point `r` over a fixed screen shape (AV 1106/07 §5.7).
///
/// `pwedge` (SM4) / `p2edge` (SM5) / `p2wedge` (SM6) realize this trait. The
/// engine calls it with the real and image source/receiver points to build the
/// four/eight-path image model.
pub trait DiffractionKernel {
    /// Diffracted complex pressure `p̂` (Nord-native, carries `e^{+jωτ}`).
    ///
    /// # Errors
    ///
    /// Propagates [`PropagationError`] from the underlying wedge kernel for a
    /// degenerate/non-physical geometry.
    fn diffract(
        &self,
        f_hz: f64,
        s: [f64; 2],
        r: [f64; 2],
    ) -> Result<Complex<f64>, PropagationError>;
}

/// Wedge angles `(β, θ_S, θ_R)` from screen coordinates (AV 1106/07 Eq. 162,
/// homogeneous `Δθ_S = Δθ_R = 0`).
///
/// `θ` is measured CCW from the receiver-side wedge face; validity
/// `0 ≤ θ_R ≤ θ_S ≤ β ≤ 2π` (thin screens sit at `β ≈ 2π`).
fn wedge_angles(
    w1: [f64; 2],
    t: [f64; 2],
    w2: [f64; 2],
    s: [f64; 2],
    r: [f64; 2],
) -> (f64, f64, f64) {
    // β₁/β₂ = arctan((z_face − z_T)/(±(x_T − x_face))) + π/2 (Eq. 162).
    let b1 = ((w1[1] - t[1]) / (t[0] - w1[0])).atan() + FRAC_PI_2;
    let b2 = ((w2[1] - t[1]) / (w2[0] - t[0])).atan() + FRAC_PI_2;
    let th1 = ((s[1] - t[1]) / (t[0] - s[0])).atan() + FRAC_PI_2;
    let th2 = ((r[1] - t[1]) / (r[0] - t[0])).atan() + FRAC_PI_2;
    let beta = TAU - b1 - b2;
    let theta_s = TAU - th1 - b2;
    let theta_r = th2 - b2;
    (beta, theta_s, theta_r)
}

#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// Single-edge `pwedge` kernel (Sub-model 4).
pub struct PwedgeKernel {
    /// Source-side wedge base `W₁`.
    pub w1: [f64; 2],
    /// Diffracting edge `T`.
    pub t: [f64; 2],
    /// Receiver-side wedge base `W₂`.
    pub w2: [f64; 2],
    /// Source-face impedance `Ẑ_S`.
    pub z_s: Complex<f64>,
    /// Receiver-face impedance `Ẑ_R`.
    pub z_r: Complex<f64>,
    /// Speed of sound c₀, m/s.
    pub c0: f64,
}

impl DiffractionKernel for PwedgeKernel {
    fn diffract(
        &self,
        f_hz: f64,
        s: [f64; 2],
        r: [f64; 2],
    ) -> Result<Complex<f64>, PropagationError> {
        let (beta, theta_s, theta_r) = wedge_angles(self.w1, self.t, self.w2, s, r);
        let r_s = dist(s, self.t);
        let r_r = dist(self.t, r);
        let geo = WedgeGeometry {
            tau_s: r_s / self.c0,
            tau_r: r_r / self.c0,
            tau: (r_s + r_r) / self.c0,
            r_s,
            r_r,
            l: r_s + r_r,
            theta_s,
            theta_r,
            beta,
        };
        pwedge(f_hz, &geo, self.z_s, self.z_r)
    }
}

/// Build the two-wedge geometry for a `p2edge`/`p2wedge` kernel from a source
/// point, the two edges `T₁`/`T₂`, and a receiver point.
fn two_wedge_geom(
    s: [f64; 2],
    t1: [f64; 2],
    t2: [f64; 2],
    r: [f64; 2],
    w1: [f64; 2],
    w2: [f64; 2],
    c0: f64,
) -> TwoWedgeGeometry {
    // Wedge 1 at T₁: faces W₁ (source) and T₂; ray S → (edge T₂).
    let (beta1, theta_1s, theta_1r) = wedge_angles(w1, t1, t2, s, t2);
    // Wedge 2 at T₂: faces T₁ and W₂; ray (edge T₁) → R.
    let (beta2, theta_2s, theta_2r) = wedge_angles(t1, t2, w2, t1, r);
    let r_s = dist(s, t1);
    let r_m = dist(t1, t2);
    let r_r = dist(t2, r);
    TwoWedgeGeometry {
        beta1,
        theta_1s,
        theta_1r,
        beta2,
        theta_2s,
        theta_2r,
        tau_s: r_s / c0,
        tau_m: r_m / c0,
        tau_r: r_r / c0,
        r_s,
        r_m,
        r_r,
    }
}

/// Two-edge `p2edge` kernel (Sub-model 5, thick screen — forced-hard top).
pub struct P2edgeKernel {
    /// Source-side wedge base `W₁`.
    pub w1: [f64; 2],
    /// First (source-side) edge `T₁`.
    pub t1: [f64; 2],
    /// Second (receiver-side) edge `T₂`.
    pub t2: [f64; 2],
    /// Receiver-side wedge base `W₂`.
    pub w2: [f64; 2],
    /// Source-face impedance `Ẑ_{1S}`.
    pub z_s: Complex<f64>,
    /// Receiver-face impedance `Ẑ_{2R}`.
    pub z_r: Complex<f64>,
    /// Speed of sound c₀, m/s.
    pub c0: f64,
}

impl DiffractionKernel for P2edgeKernel {
    fn diffract(
        &self,
        f_hz: f64,
        s: [f64; 2],
        r: [f64; 2],
    ) -> Result<Complex<f64>, PropagationError> {
        let geo = two_wedge_geom(s, self.t1, self.t2, r, self.w1, self.w2, self.c0);
        p2edge(f_hz, &geo, WedgePrimary::First, self.z_s, self.z_r)
    }
}

/// Two-screen `p2wedge` kernel (Sub-model 6).
pub struct P2wedgeKernel {
    /// Source-side wedge base `W₁`.
    pub w1: [f64; 2],
    /// First-screen edge `T₁`.
    pub t1: [f64; 2],
    /// Second-screen edge `T₂`.
    pub t2: [f64; 2],
    /// Receiver-side wedge base `W₂`.
    pub w2: [f64; 2],
    /// Wedge-face impedances.
    pub z: TwoWedgeImpedances,
    /// Speed of sound c₀, m/s.
    pub c0: f64,
}

impl DiffractionKernel for P2wedgeKernel {
    fn diffract(
        &self,
        f_hz: f64,
        s: [f64; 2],
        r: [f64; 2],
    ) -> Result<Complex<f64>, PropagationError> {
        let geo = two_wedge_geom(s, self.t1, self.t2, r, self.w1, self.w2, self.c0);
        p2wedge(f_hz, &geo, WedgePrimary::First, &self.z)
    }
}

/// The turbulence-decorrelation coherence factor `Fc` (AV 1106/07 Eq. 113),
/// computed locally so the screen engine can form `F₄ = Ff·Fc_S·Fc_R` (two `Fc`
/// factors, Eq. 181) without an API change to [`super::super::coherence`].
fn fc(f_hz: f64, cv2: f64, ct2: f64, t_air_c: f64, c0: f64, rho: f64, d: f64) -> f64 {
    if cv2 == 0.0 && ct2 == 0.0 {
        return 1.0;
    }
    let t_abs = 273.15 + t_air_c;
    let turb = ct2 / (t_abs * t_abs) + (22.0 / 3.0) * cv2 / (c0 * c0);
    let x = 5.888e-3 * turb * f_hz * f_hz * rho.abs().powf(5.0 / 3.0) * d;
    exp_clamped(-x)
}

/// Per-side reflection variables (source or receiver side of the screen).
#[derive(Clone, Copy)]
struct SideVars {
    /// Spherical-wave reflection coefficient `Q̂` at the screen top (Eq. 173).
    q: Complex<f64>,
    /// Incoherent reflection coefficient `ℜ = ρᵢ` (Eq. 183).
    rho_i: f64,
    /// Side reflection delay `Δτ` (cancellation-free).
    dtau: f64,
    /// Transversal separation `ρ` (Eqs. 178/180).
    rho_sep: f64,
    /// Reflected path length `d′` (Fc integral length).
    d_prime: f64,
    /// Image of the endpoint over the strip.
    image: [f64; 2],
    /// Modified Fresnel-zone weight `w″` for this strip (Eq. 174).
    w_dbl: f64,
    /// Strip roughness `r`, m.
    roughness: f64,
}

/// Build the source-side reflection variables for one strip (endpoint = S, other
/// endpoint = the screen top T; AV 1106/07 §5.13.1 base-model prescription).
fn side_vars(
    f_hz: f64,
    endpoint: [f64; 2],
    top: [f64; 2],
    strip: &SurfaceStrip,
    coh: &CoherenceInputs,
    receiver_side: bool,
) -> Result<SideVars, PropagationError> {
    let rp = straight_rays_over_segment(endpoint, top, strip.seg_a, strip.seg_b, coh.c0)?;
    let refl = rp
        .reflected
        .ok_or(PropagationError::DegenerateRayGeometry {
            detail: "screen side reflection undefined",
        })?;
    let z_g = ground_impedance(f_hz, strip.sigma_kpa)?;
    let q = spherical_q(f_hz, refl.tau, refl.psi_g, z_g);
    let rho_i = incoherent_rho(f_hz, z_g);
    let sin_psi = refl.psi_g.sin();
    let h1 = refl.r1 * sin_psi;
    let h2 = refl.r2 * sin_psi;
    let rho_sep = if h1 + h2 > 0.0 {
        2.0 * h1 * h2 / (h1 + h2)
    } else {
        0.0
    };
    let image = image_point(endpoint, strip.seg_a, strip.seg_b);

    // Modified Fresnel-zone weight w″ (Eq. 174): w·r_S·r_R with F_λ = λ/16 and
    // the edge-proximity modifiers of Eqs. 175/176.
    let (a, b) = if receiver_side {
        (top, endpoint) // A = T, B = R (Eq. 164 receiver-side ordering)
    } else {
        (endpoint, top) // A = S, B = T (Eq. 164 source-side ordering)
    };
    let sv = segment_variables(a, b, strip.seg_a, strip.seg_b);
    let h_a = sv.h_a; // A above segment
    let h_b = sv.h_b; // B above segment
    let d_prime_h = sv.d_prime.abs();
    let lambda = coh.c0 / f_hz;
    let w_dbl = if h_a <= 0.0 || h_b <= 0.0 || d_prime_h <= MIN_LEN {
        0.0
    } else {
        let w = fresnel_zone_w(d_prime_h, h_a, h_b, sv.d1, sv.d2, lambda / 16.0).unwrap_or(0.0);
        // Eq. 175/176 edge-proximity modifiers. h_max clamps the near-extension
        // fade; for typical geometry both modifiers are 1.
        let h_max = (0.0005 * (b[0] - a[0]).abs()).min(0.2);
        // Source-side: r_S on A (source/T height), r_R on B (T/receiver height).
        let h_pp = h_a.min(h_max);
        let r_first = if h_a >= h_pp {
            1.0
        } else if h_a > 0.0 {
            h_a / h_pp
        } else {
            0.0
        };
        let r_second = if h_b >= h_max {
            1.0
        } else if h_b > 0.0 {
            h_b / h_max
        } else {
            0.0
        };
        w * r_first * r_second
    };

    Ok(SideVars {
        q,
        rho_i,
        dtau: rp.dtau,
        rho_sep,
        d_prime: refl.r,
        image,
        w_dbl,
        roughness: strip.roughness_r,
    })
}

/// Eq. 187 normalized weights + Q-weights for one side of the screen.
struct SideWeights {
    /// Normalized per-strip weights `w′_i` (Eq. 188 exponents; `Σ ≈ 1`).
    w_prime: Vec<f64>,
    /// The Q-weight `w_Q` (Eq. 187 bottom): `1` if `Σw″ ≥ 1`, else `(Σw″)²`.
    w_q: f64,
}

/// Apply Eq. 187 normalization to a side's raw `w″` weights.
fn normalize_weights(w_dbl: &[f64]) -> (f64, SideWeightsRaw) {
    let w_t: f64 = w_dbl.iter().sum();
    let dw_t = if w_t > 1.0 { w_t - 1.0 } else { 0.0 };
    (
        w_t,
        SideWeightsRaw {
            w_t,
            dw_t,
            w_dbl: w_dbl.to_vec(),
        },
    )
}

struct SideWeightsRaw {
    w_t: f64,
    dw_t: f64,
    w_dbl: Vec<f64>,
}

impl SideWeightsRaw {
    /// Finish Eq. 187 once both sides' `Δw_t` are known (`Δw_t = Δw_1t + Δw_2t`).
    fn finish(&self, dw_total: f64) -> SideWeights {
        let w_prime = self
            .w_dbl
            .iter()
            .map(|&w| {
                if self.w_t > 1.0 {
                    (w / self.w_t) * (self.dw_t / dw_total + 1.0)
                } else if self.w_t > 0.0 {
                    w / self.w_t
                } else {
                    0.0
                }
            })
            .collect();
        let w_q = if self.w_t >= 1.0 {
            1.0
        } else {
            self.w_t * self.w_t
        };
        SideWeights { w_prime, w_q }
    }
}

/// The complex screen factor `p̂_SCR = p̂₁ / p̂₀` (Eq. 163, phase-preserving).
fn screen_factor(
    kernel: &dyn DiffractionKernel,
    f_hz: f64,
    source: [f64; 2],
    receiver: [f64; 2],
    c0: f64,
) -> Result<Complex<f64>, PropagationError> {
    let p1 = kernel.diffract(f_hz, source, receiver)?;
    let r_sr = dist(source, receiver);
    let tau_sr = r_sr / c0;
    // p̂₀ = e^{+jωτ_SR}/R_SR (Nord-native free-field direct).
    let p0 = Complex::from_polar(1.0 / r_sr, TAU * f_hz * tau_sr);
    Ok(p1 / p0)
}

/// Shared four-path engine for Sub-models 4 and 5 (no middle region).
///
/// `force_f = Some(v)` overrides every coherence coefficient (test hook:
/// `Some(1.0)` ⇒ fully coherent, `p_incoh == 0`).
fn run_four_path(
    f_hz: f64,
    cfg: &ScreenConfig,
    kernel: &dyn DiffractionKernel,
    force_f: Option<f64>,
) -> Result<GroundResult, PropagationError> {
    let c0 = cfg.coh.c0;
    let s = cfg.source;
    let r = cfg.receiver;
    let top = screen_top(cfg);

    let p_scr = screen_factor(kernel, f_hz, s, r, c0)?;
    let p1 = kernel.diffract(f_hz, s, r)?;

    // Per-strip side variables.
    let mut src: Vec<SideVars> = Vec::with_capacity(cfg.before.len());
    for strip in cfg.before {
        src.push(side_vars(f_hz, s, top, strip, cfg.coh, false)?);
    }
    let mut rcv: Vec<SideVars> = Vec::with_capacity(cfg.after.len());
    for strip in cfg.after {
        rcv.push(side_vars(f_hz, r, top, strip, cfg.coh, true)?);
    }

    // Eq. 187 normalization (needs both sides' Δw_t).
    let (_, src_raw) = normalize_weights(&src.iter().map(|v| v.w_dbl).collect::<Vec<_>>());
    let (_, rcv_raw) = normalize_weights(&rcv.iter().map(|v| v.w_dbl).collect::<Vec<_>>());
    let dw_total = src_raw.dw_t + rcv_raw.dw_t;
    let src_w = src_raw.finish(dw_total);
    let rcv_w = rcv_raw.finish(dw_total);

    // Precompute the per-strip diffracted ratios p̂₂/p̂₁ (source images) and
    // p̂₃/p̂₁ (receiver images).
    let mut r2: Vec<Complex<f64>> = Vec::with_capacity(src.len());
    for v in &src {
        r2.push(kernel.diffract(f_hz, v.image, r)? / p1);
    }
    let mut r3: Vec<Complex<f64>> = Vec::with_capacity(rcv.len());
    for v in &rcv {
        r3.push(kernel.diffract(f_hz, s, v.image)? / p1);
    }

    // Combine all (i1, i2) combinations per Eqs. 182 + 188.
    let mut cc_eff_ln = 0.0_f64; // Σ W·ln|c|²   (coherent energy geometric mean)
    let mut g_eff_ln = 0.0_f64; // Σ W·ln(|c|²+e)
    let mut phase_acc = 0.0_f64; // Σ W·arg(c)
    let mut any = false;

    for (i1, vs) in src.iter().enumerate() {
        for (i2, vr) in rcv.iter().enumerate() {
            let w = src_w.w_prime[i1] * rcv_w.w_prime[i2];
            if w <= 0.0 {
                continue;
            }
            let p4 = kernel.diffract(f_hz, vs.image, vr.image)? / p1;

            // Coherence coefficients F₂/F₃/F₄ (Eqs. 177/179/181), homogeneous.
            let (f2, f3, f4) = match force_f {
                Some(v) => (v, v, v),
                None => {
                    let fc_s = fc(
                        f_hz,
                        cfg.coh.cv2,
                        cfg.coh.ct2,
                        cfg.coh.t_air_c,
                        c0,
                        vs.rho_sep,
                        vs.d_prime,
                    );
                    let fc_r = fc(
                        f_hz,
                        cfg.coh.cv2,
                        cfg.coh.ct2,
                        cfg.coh.t_air_c,
                        c0,
                        vr.rho_sep,
                        vr.d_prime,
                    );
                    let f2 = coherence_ff(f_hz, vs.dtau) * fc_s;
                    let f3 = coherence_ff(f_hz, vr.dtau) * fc_r;
                    let f4 = coherence_ff(f_hz, vs.dtau + vr.dtau) * fc_s * fc_r;
                    (f2, f3, f4)
                }
            };
            let _ = (vs.roughness, vr.roughness); // Fr = 1 (r = 0 Phase 2 targets)

            let wq1 = src_w.w_q;
            let wq2 = rcv_w.w_q;
            // Coherent Δp̂_G (Eq. 182 first term).
            let c = Complex::new(1.0, 0.0)
                + f2 * wq1 * vs.q * r2[i1]
                + f3 * wq2 * vr.q * r3[i2]
                + f4 * wq1 * wq2 * vs.q * vr.q * p4;
            // Incoherent residual (Eq. 182 remaining terms).
            let e = (1.0 - f2 * f2) * (wq1 * vs.rho_i * r2[i1]).norm_sqr()
                + (1.0 - f3 * f3) * (wq2 * vr.rho_i * r3[i2]).norm_sqr()
                + (1.0 - f4 * f4) * (wq1 * vs.rho_i * wq2 * vr.rho_i * p4).norm_sqr();

            let cc = c.norm_sqr().max(1e-300);
            let gg = (cc + e).max(1e-300);
            cc_eff_ln += w * cc.ln();
            g_eff_ln += w * gg.ln();
            phase_acc += w * c.arg();
            any = true;
        }
    }

    if !any {
        // No reflecting combination contributes: the bare screen factor.
        return Ok(GroundResult::from_channels(p_scr, 0.0));
    }

    let cc_eff = cc_eff_ln.exp();
    let g_eff = g_eff_ln.exp();
    let scr_mag = p_scr.norm();
    let scr_arg = p_scr.arg();
    // ĥ = p̂_SCR · √CC_eff · e^{j(arg p̂_SCR + Σ W·arg c)} — |ĥ|² = |p̂_SCR|²·CC_eff.
    let h_coh = Complex::from_polar(scr_mag * cc_eff.sqrt(), scr_arg + phase_acc);
    // p_incoh = |p̂_SCR|²·(G_eff − CC_eff) ≥ 0 (weighted GM is monotone).
    let p_incoh = scr_mag * scr_mag * (g_eff - cc_eff).max(0.0);
    Ok(GroundResult::from_channels(h_coh, p_incoh))
}

/// The diffracting edge closest to the receiver (used as the "screen top" for
/// side reflections). For one edge this is `screen[1]`; for two edges the base
/// prescription evaluates each side against its nearest edge, but the base model
/// uses the single representative top — here the first edge `T₁`.
fn screen_top(cfg: &ScreenConfig) -> [f64; 2] {
    match cfg.screen.len() {
        0 => cfg.source,
        3 => cfg.screen[1],
        _ => cfg.screen[1], // T₁ for multi-edge shapes
    }
}

/// Sub-model 4 — one screen, one diffracting edge (AV 1106/07 §5.13,
/// Eqs. 157–188). Four-path image model with the two-channel [`GroundResult`].
///
/// # Errors
///
/// [`PropagationError`] for degenerate screen/strip geometry or invalid σ.
pub fn submodel4(f_hz: f64, cfg: &ScreenConfig) -> Result<GroundResult, PropagationError> {
    submodel4_eval(f_hz, cfg, None)
}

/// Sub-model 4 with a coherence override (test hook).
pub(crate) fn submodel4_eval(
    f_hz: f64,
    cfg: &ScreenConfig,
    force_f: Option<f64>,
) -> Result<GroundResult, PropagationError> {
    let kernel = PwedgeKernel {
        w1: cfg.screen[0],
        t: cfg.screen[1],
        w2: cfg.screen[2],
        z_s: cfg.z_face_source,
        z_r: cfg.z_face_receiver,
        c0: cfg.coh.c0,
    };
    run_four_path(f_hz, cfg, &kernel, force_f)
}

/// Sub-model 4 with a caller-supplied kernel (test hook for transparent kernels).
#[cfg(test)]
pub(crate) fn submodel4_with_kernel(
    f_hz: f64,
    cfg: &ScreenConfig,
    kernel: &dyn DiffractionKernel,
    force_f: Option<f64>,
) -> Result<GroundResult, PropagationError> {
    run_four_path(f_hz, cfg, kernel, force_f)
}

/// Sub-model 5 — one thick screen, two edges (AV 1106/07 §5.14, Eqs. 189–221).
/// The same four-path engine with the `p2edge` kernel (shared top segment,
/// forced-hard top, factor 0.5 — all inside the 02-03 kernel).
///
/// # Errors
///
/// [`PropagationError`] for degenerate geometry or invalid σ.
pub fn submodel5(f_hz: f64, cfg: &ScreenConfig) -> Result<GroundResult, PropagationError> {
    let kernel = P2edgeKernel {
        w1: cfg.screen[0],
        t1: cfg.screen[1],
        t2: cfg.screen[2],
        w2: cfg.screen[3],
        z_s: cfg.z_face_source,
        z_r: cfg.z_face_receiver,
        c0: cfg.coh.c0,
    };
    run_four_path(f_hz, cfg, &kernel, None)
}

/// Sub-model 6 — two screens (AV 1106/07 §5.15, Eqs. 222–270). The eight-ray
/// image model: the four-path set crossed with a middle-region reflection,
/// encoded as a three-region bitmask `{before(1), middle(2), after(4)}`. Uses the
/// `p2wedge` kernel.
///
/// # Errors
///
/// [`PropagationError`] for degenerate geometry or invalid σ.
pub fn submodel6(f_hz: f64, cfg: &ScreenConfig) -> Result<GroundResult, PropagationError> {
    let kernel = P2wedgeKernel {
        w1: cfg.screen[0],
        t1: cfg.screen[1],
        t2: cfg.screen[2],
        w2: cfg.screen[3],
        z: TwoWedgeImpedances {
            z_1s: cfg.z_face_source,
            z_1r: cfg.z_face_receiver,
            z_2s: cfg.z_face_source,
            z_2r: cfg.z_face_receiver,
        },
        c0: cfg.coh.c0,
    };
    run_eight_path(f_hz, cfg, &kernel, None)
}

/// Sub-model 6 with a caller-supplied kernel / coherence override (test hook,
/// exercised by the Sub-model 6 structural tests in this module).
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn submodel6_with_kernel(
    f_hz: f64,
    cfg: &ScreenConfig,
    kernel: &dyn DiffractionKernel,
    force_f: Option<f64>,
) -> Result<GroundResult, PropagationError> {
    run_eight_path(f_hz, cfg, kernel, force_f)
}

/// Eight-ray engine for Sub-model 6 (Eq. 222). Rays are the 2³ subsets of the
/// three reflecting regions `{before, middle, after}`; each region reflects the
/// appropriate endpoint (before→S, after→R, middle→R over the mid strip) and
/// multiplies in its `Q̂`. The two-channel split is applied per ray exactly as in
/// the four-path engine.
fn run_eight_path(
    f_hz: f64,
    cfg: &ScreenConfig,
    kernel: &dyn DiffractionKernel,
    force_f: Option<f64>,
) -> Result<GroundResult, PropagationError> {
    let c0 = cfg.coh.c0;
    let s = cfg.source;
    let r = cfg.receiver;
    let t1 = cfg.screen[1];
    let t2 = cfg.screen[2];

    let p_scr = screen_factor(kernel, f_hz, s, r, c0)?;
    let p1 = kernel.diffract(f_hz, s, r)?;

    // Region reflection variables (may be absent → identity/unit Q).
    let before = cfg
        .before
        .first()
        .map(|st| side_vars(f_hz, s, t1, st, cfg.coh, false))
        .transpose()?;
    let after = cfg
        .after
        .first()
        .map(|st| side_vars(f_hz, r, t2, st, cfg.coh, true))
        .transpose()?;
    let middle = cfg
        .middle
        .first()
        .map(|st| side_vars(f_hz, r, t2, st, cfg.coh, true))
        .transpose()?;

    let s_of = |b: usize, v: &Option<SideVars>| -> [f64; 2] {
        if b != 0 {
            v.map(|x| x.image).unwrap_or(s)
        } else {
            s
        }
    };

    // Accumulate the coherent sum and the incoherent energy directly (Eq. 222 is
    // a plain coherent sum of the eight rays; the F/ρ split mirrors Eq. 182).
    let mut coherent = Complex::new(0.0, 0.0);
    let mut incoh = 0.0_f64;

    for mask in 0u8..8 {
        let use_before = mask & 1 != 0;
        let use_middle = mask & 2 != 0;
        let use_after = mask & 4 != 0;
        if use_before && before.is_none() {
            continue;
        }
        if use_middle && middle.is_none() {
            continue;
        }
        if use_after && after.is_none() {
            continue;
        }

        // Endpoint images: before reflects S, after/middle reflect R.
        let sp = if use_before { before.unwrap().image } else { s };
        let rp = if use_after {
            after.unwrap().image
        } else if use_middle {
            middle.unwrap().image
        } else {
            r
        };
        let _ = s_of; // (kept for documentation of the reflection convention)

        let ratio = kernel.diffract(f_hz, sp, rp)? / p1;

        // Product of region Q̂'s and coherence / incoherent coefficients.
        let mut q = Complex::new(1.0, 0.0);
        let mut rho = 1.0_f64;
        let mut dtau = 0.0_f64;
        let mut rho_sep = 0.0_f64;
        let mut d_prime = 0.0_f64;
        let mut n_reg = 0u32;
        for (used, v) in [
            (use_before, &before),
            (use_middle, &middle),
            (use_after, &after),
        ] {
            if used {
                let sv = v.unwrap();
                q *= sv.q;
                rho *= sv.rho_i;
                dtau += sv.dtau;
                rho_sep += sv.rho_sep;
                d_prime += sv.d_prime;
                n_reg += 1;
            }
        }

        if n_reg == 0 {
            coherent += ratio; // p̂₁/p̂₁ = 1
            continue;
        }

        let f_coh = match force_f {
            Some(v) => v,
            None => {
                let fcv = fc(
                    f_hz,
                    cfg.coh.cv2,
                    cfg.coh.ct2,
                    cfg.coh.t_air_c,
                    c0,
                    rho_sep / n_reg as f64,
                    d_prime,
                );
                coherence_ff(f_hz, dtau) * fcv
            }
        };
        coherent += f_coh * q * ratio;
        incoh += (1.0 - f_coh * f_coh) * (rho * ratio.norm()).powi(2);
    }

    // ĥ = p̂_SCR · coherent (both complex, phase live); p_incoh = |p̂_SCR|²·incoh.
    let h_coh = p_scr * coherent;
    let p_incoh = p_scr.norm_sqr() * incoh;
    Ok(GroundResult::from_channels(h_coh, p_incoh))
}

#[cfg(test)]
mod tests;
