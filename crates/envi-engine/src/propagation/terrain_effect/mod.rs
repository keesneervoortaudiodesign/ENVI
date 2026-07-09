//! Terrain-effect composition ΔL_t (AV 1106/07 §5.10–5.16, §5.22 Eq. 332):
//! Sub-models 1/2 (ground) and 4/5/6/7 (screens), combined by the §5.21
//! transition parameters into the per-band terrain excess-attenuation.
//!
//! Plan 02-02 lands the ground sub-models ([`submodel1`], [`submodel2`]) and the
//! load-bearing two-channel [`GroundResult`]. Screen sub-models + the Eq. 332
//! composition arrive in plan 02-04.
//!
//! Nord2000-native complex convention (e^{−jωt}); the single conjugation to
//! ENVI's e^{+jωt} transfer convention happens in `transfer.rs` (plan 02-05).
//!
//! # The two-channel contract (user-locked, PROJECT.md Key Decisions 2026-07-07)
//!
//! Every terrain-effect sub-model returns a [`GroundResult`] with **two
//! separate channels** relative to the free-field direct path `p̂₀`:
//!
//! - [`GroundResult::h_coh_factor`] is a genuine `Complex<f64>`: the
//!   coherence-weighted coherent sum `1 + Σ F·(Rᵢ)·e^{+j2πfΔτ}·Q̂` with the Δτ
//!   interference **phase LIVE**. This is the factor that multiplies into `H_coh`
//!   at the 02-05 transfer boundary; the ground-reflected contribution keeps its
//!   phase and combines as complex pressure — dips emerge from the phase, not
//!   from band-energy bookkeeping.
//! - [`GroundResult::p_incoh`] is a real, non-negative energy: **only** the
//!   turbulence-decorrelated `(1−F²)·|ρᵢ·p̂ᵢ/p̂₀|²` residual. It is added at
//!   final-level readout and **never overwrites phase**. When the field is fully
//!   coherent (`F → 1`) this channel is exactly `0`.
//!
//! The band value is `delta_l_db = 10·lg(|h_coh_factor|² + p_incoh)`. Nothing in
//! this module family collapses complex pressure to magnitude/energy along the
//! chain — that separation is what makes ENG-07 (phase-preserving combination)
//! and ENG-02 (segmented soft↔hard ground) correct.
//!
//! # Sub-model 7 (turbulence scattering) is energy-only
//!
//! [`submodel7::submodel7_delta_l`] returns a real `f64` (dB). Eq. 332's screen
//! compositions (`ΔL₄+ΔL₇,₁`, `ΔL₅+ΔL₇,₂`, `ΔL₆+ΔL₇`, assembled in plan 02-05)
//! add the ΔL₇ scattered **energy** into the `p_incoh`/level side of the model
//! (`10·lg(10^{ΔL_scr/10}+10^{ΔL₇/10})`, [`submodel7::combine_scatter`]) — it
//! **never** touches [`GroundResult::h_coh_factor`]. Because Sub-model 7 is typed
//! to `f64`, it is *structurally* incapable of corrupting the coherent phase
//! channel (user-locked contract; threat T-02-13).

use num_complex::Complex;

use crate::freq::FreqAxis;
use crate::propagation::PropagationError;
use crate::propagation::coherence::{CoherenceInputs, coherence_f_delta_nu};
use crate::propagation::rays::{RayPair, circular_rays, straight_rays};
use crate::propagation::refraction::SoundSpeedProfile;
use crate::propagation::refraction::eqssp::{calc_eq_ssp, calc_eq_ssp_ground};
use crate::propagation::refraction::shadow_zone::shadow_zone_shielding;
use crate::propagation::terrain_interpretation::{
    ScreenClass, StripSpec, TerrainInterpretation, interpret_terrain,
};
use crate::propagation::transmission::{IsolationSpectrum, TransmissionFilter};
use crate::scene::TerrainProfile;

pub mod screen;
pub mod submodel1;
pub mod submodel11;
pub mod submodel2;
pub mod submodel3;
pub mod submodel7;
pub mod submodel8;

/// Two-channel result of any terrain-effect sub-model, normalized relative to
/// the free-field direct path `p̂₀`.
///
/// See the [module docs](self) for the user-locked contract. Nord2000-native
/// convention (e^{−jωt}) inside the engine; conversion to the transfer
/// convention happens once, in plan 02-05.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundResult {
    /// The Nord2000 band value `10·lg(|h_coh_factor|² + p_incoh)`, dB.
    pub delta_l_db: f64,
    /// The F-weighted coherent complex sum, phase LIVE (multiplies into `H_coh`).
    pub h_coh_factor: Complex<f64>,
    /// The `(1−F²)·|ρᵢ·p̂ᵢ/p̂₀|²` turbulence-decorrelated residual — real,
    /// non-negative, added at readout only. Exactly `0` when `F → 1`.
    pub p_incoh: f64,
}

impl GroundResult {
    /// Assemble a result from the two channels, deriving the band value from the
    /// two-channel identity `delta_l_db = 10·lg(|h_coh|² + p_incoh)`.
    #[must_use]
    pub fn from_channels(h_coh_factor: Complex<f64>, p_incoh: f64) -> Self {
        let energy = h_coh_factor.norm_sqr() + p_incoh;
        Self {
            delta_l_db: 10.0 * energy.log10(),
            h_coh_factor,
            p_incoh,
        }
    }
}

/// The per-frequency two-channel terrain effect over the whole 105-point axis,
/// relative to the free-field direct path — the §5.22 Eq. 332 composition of the
/// ground (Sub-model 1/2) and screen (Sub-model 4/5/6 + 7) sub-models.
///
/// This is the Phase 3 (refraction) and Phase 4 (tensor) forward contract:
/// [`Self::h_coh_factor`] is the phase-preserving coherent factor multiplied
/// into `H_coh` at the transfer boundary (`transfer::nord_ratio_to_transfer`);
/// [`Self::p_incoh`] rides alongside as the real incoherent energy;
/// [`Self::delta_l_db`] is the document-exact Nord2000 band value (the
/// validation channel).
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainEffect {
    /// Nord2000-native coherent complex factor per band (length `N_BANDS`).
    pub h_coh_factor: Vec<Complex<f64>>,
    /// Incoherent power add-on per band, relative to the free field.
    pub p_incoh: Vec<f64>,
    /// Document-exact Eq. 332 band value `ΔL_t` per band, dB (validation path).
    pub delta_l_db: Vec<f64>,
}

/// Per-frequency two-channel terrain effect for a cut-plane profile and a
/// source/receiver pair (AV 1106/07 §5.21 dispatch + §5.22 Eq. 332).
///
/// `src`/`rcv` are `[x, y, z]`; only the vertical cut-plane `[x, z]` is used.
/// The §5.21 interpretation classifies the terrain (flat / one screen / thick /
/// two screens); Eq. 332 blends the screen and no-screen branches with the
/// frequency-dependent transition parameters.
///
/// # The two-channel composition (Eq. 332, phase-preserving)
///
/// The document defines Eq. 332 as a **dB-level** interpolation. To honour the
/// user-locked phase-preserving contract, the two channels
/// ([`GroundResult::h_coh_factor`], [`GroundResult::p_incoh`]) are interpolated
/// **linearly** with the same `r` weights, while [`TerrainEffect::delta_l_db`]
/// carries the document-exact dB interpolation. On every Phase 2 target profile
/// **every transition parameter is `0` or `1`**, so exactly one branch is
/// active and the two readings coincide (`10·lg(|h_coh|²+p_incoh) == delta_l_db`
/// bit-for-bit); the linear-channel blend is only *approximate* in the marginal
/// transition zone (`0 < r < 1`), which no Phase 2 target exercises for
/// `r_scr2`/`r_scr12`/`r_flat`. Phase 3 revisits the fractional-`r` channel
/// combination when refraction makes screens marginal (see the module note).
///
/// # Refraction (plan 03-01)
///
/// `refraction = Some(profile)` swaps the flat-channel `straight_rays` for
/// `circular_rays` with `(ξ, c₀)` from CalcEqSSP; `|ξ|<1e-6` keeps the result
/// bit-for-bit the homogeneous Phase-2 path (D-02). `None` is the homogeneous
/// atmosphere. Both the single-surface (Sub-model 1) and the **segmented**
/// (Sub-model 2) ground paths are refracted: the segmented path uses the
/// frequency-dependent `calc_eq_ssp_ground` collapse (§5.5.3, WR-01 wired).
/// A **screen** under an active weather profile is still a typed error
/// ([`PropagationError::WeatherScreenNotImplemented`], Pitfall 9) — refusing to
/// compute an unrefracted screen.
///
/// # Semi-transparent partitions (plan 05-03, ENG-10 + the D-06 extension)
///
/// `isolation = Some(R)` threads a partition's sound reduction index `R(f)` into
/// the screen branch as a complex **minimum-phase** transmission filter `T(f)`
/// ([`TransmissionFilter::from_isolation`], native `e^{−jωt}`). The filter is
/// built ONCE before the band loop (the min-phase transform needs the whole
/// 105-point grid, never per-band — the [`FlatChannel::from_profile`] precompute
/// precedent) and the per-band `T` is added to the screen branch's coherent
/// factor at [`screen_channel`] — the single §5.13–5.15 assembly point covering
/// Sub-models 4/5/6. Since every screen sub-model's `h_coh_factor` is a ratio
/// relative to the free-field direct `p̂₀` and the straight-through path travels
/// the same S→R line, the transmitted term relative to `p̂₀` is exactly `T(f)`:
/// `h_semi = h_opaque + T`, one complex addition. It joins the **coherent channel
/// only** — the deterministic min-phase filter is never decorrelated by `F` and
/// never leaks into `p_incoh` (Pitfall 6).
///
/// In the marginal transition zone the whole screen branch — including `T` — is
/// weighted by `r_scr1` (the Eq. 332 outer blend), which is correct: leakage only
/// exists where the screen does. This is documented behaviour, not "fixed".
///
/// **Opaque is `None` (D-10), structural.** `isolation = None` takes the exact
/// pre-extension code path — the transmission term and the min-phase computation
/// are never constructed, so the opaque screen result is bit-identical (pinned by
/// the `opaque_regression` fixture, T6). The `R → 0` corner is a documented model
/// property, NOT a bug: `R ≡ 0` restores the direct field PLUS the diffracted
/// residue (inherent to the locked additive composition — benign for physical
/// partitions, never renormalized; Pitfall 7). Opaque is the `None` state, never
/// a large-`R` / `INFINITY` sentinel.
///
/// # Errors
///
/// [`PropagationError::ConvexSegmentNotImplemented`] if the no-screen branch
/// demands Sub-model 3 (`r_flat < 1` with weight) over a **convex**/transition
/// ground segment (the §5.12 concave path IS wired);
/// [`PropagationError::WeatherScreenNotImplemented`] if a weather profile is
/// supplied over a screen terrain (Pitfall 9);
/// [`PropagationError::IsolationWithoutScreen`] if an `isolation` spectrum is
/// supplied over flat terrain (no partition on the path); other
/// [`PropagationError`]s propagate from the sub-models on degenerate geometry.
///
/// Note: `isolation + weather + engaged screen` is already unreachable —
/// [`PropagationError::WeatherScreenNotImplemented`] fires first for a refracting
/// screen — so no separate guard is needed for that combination.
// Q2 disposition (plan 05-03): `isolation` is the 8th positional parameter, not
// an options struct — minimal mechanical churn on a bit-exactness-critical
// change. An options struct can absorb `refraction`+`isolation` when the Fs
// coherence seam lands (see deferred-items.md).
#[allow(clippy::too_many_arguments)]
pub fn terrain_effect(
    profile: &TerrainProfile,
    src: [f64; 3],
    rcv: [f64; 3],
    atmos_c0: f64,
    coh: &CoherenceInputs,
    axis: &FreqAxis,
    refraction: Option<&SoundSpeedProfile>,
    isolation: Option<&IsolationSpectrum>,
) -> Result<TerrainEffect, PropagationError> {
    let source = [src[0], src[2]];
    let receiver = [rcv[0], rcv[2]];
    let interp = interpret_terrain(profile, source, receiver, atmos_c0)?;

    // Contradictory-input guard (D-10 / threat T-05-03-04): a partition spectrum
    // over terrain with no screen has no partition to describe. Refuse loudly
    // (typed error) rather than silently no-op — opaque/no-partition is `None`.
    if isolation.is_some() && interp.class == ScreenClass::Flat {
        return Err(PropagationError::IsolationWithoutScreen);
    }

    // Build the native minimum-phase transmission filter ONCE (the cepstral fold
    // needs the whole 105-point grid; never per-band). `None` ⇒ the opaque path
    // never constructs it — the D-10 structural guarantee.
    let tfilter = isolation.map(TransmissionFilter::from_isolation);

    let flat = FlatChannel::from_profile(profile, source, receiver, atmos_c0, refraction)?;

    // Sub-model 3 (§5.12 non-flat terrain without screens): built once from the
    // actual sloped ground segments. Evaluated per band ONLY when the no-screen
    // flatness blend carries a non-zero `(1 − r_flat)` weight (below); flat Phase
    // 2/3 targets give `r_flat = 1`, so this is never touched there.
    let sm3 = {
        let points = profile.points();
        let segments = profile.segments();
        let seg3: Vec<submodel3::Segment3> = segments
            .iter()
            .enumerate()
            .map(|(i, seg)| submodel3::Segment3 {
                seg_a: points[i],
                seg_b: points[i + 1],
                sigma_kpa: seg.flow_resistivity,
                roughness_r: seg.roughness,
            })
            .collect();
        submodel3::SubModel3::new(source, receiver, atmos_c0, seg3, refraction.copied())?
    };

    let n = axis.centres.len();
    let mut h_coh_factor = Vec::with_capacity(n);
    let mut p_incoh = Vec::with_capacity(n);
    let mut delta_l_db = Vec::with_capacity(n);

    for (band, &f) in axis.centres.iter().enumerate() {
        let tp = interp.transition_params(f);

        let flat_res = flat.eval(f, coh)?;

        // No-screen branch: r_flat·ΔL_flat + (1−r_flat)·ΔL₃ (Eq. 332 flatness
        // blend). Sub-model 3 (§5.12) supplies ΔL₃ when the terrain is non-flat
        // without screening (`r_flat < 1` carrying weight). Flat Phase 2/3 targets
        // give r_flat = 1, so `no_screen_res == flat_res` bit-for-bit there.
        let no_screen_res = if tp.r_flat < 1.0 - 1e-9 && (1.0 - tp.r_scr1) > 1e-12 {
            let sm3_res = sm3.eval(f, coh)?;
            let rf = tp.r_flat;
            let hc = flat_res.h_coh_factor * rf + sm3_res.h_coh_factor * (1.0 - rf);
            let pi = flat_res.p_incoh * rf + sm3_res.p_incoh * (1.0 - rf);
            let dl = rf * flat_res.delta_l_db + (1.0 - rf) * sm3_res.delta_l_db;
            GroundResult {
                delta_l_db: dl,
                h_coh_factor: hc,
                p_incoh: pi,
            }
        } else {
            flat_res
        };

        let scr_res = if interp.class == ScreenClass::Flat {
            no_screen_res
        } else {
            // Pitfall 9 guard: the screen diffraction rays are straight-line, so a
            // screen case under an active weather profile would be silently
            // UNREFRACTED. Refuse (typed error) rather than compute a wrong screen
            // — but only when the screen branch actually carries weight
            // (r_scr1 > 0); a marginal screen at r_scr1 = 0 collapses to the
            // refracted no-screen result and is fine.
            if refraction.is_some() && tp.r_scr1 > 1e-12 {
                return Err(PropagationError::WeatherScreenNotImplemented { f_hz: f });
            }
            // The per-band native transmission `T(f)` (coherent-channel only) when
            // a partition spectrum is present; `None` on the opaque path leaves the
            // screen assembly bit-identical (D-10). Compare by BAND INDEX.
            let transmission = tfilter.as_ref().map(|t| t.bands[band]);
            screen_channel(f, &interp, coh, transmission)?
        };

        // Eq. 332 outer blend on r_scr1 (screen vs no-screen). Phase 2 targets:
        // r_scr1 ∈ {0,1} except the low-f transition of a real screen, where the
        // linear-channel blend is the phase-preserving reading.
        let r = tp.r_scr1;
        let hc = scr_res.h_coh_factor * r + no_screen_res.h_coh_factor * (1.0 - r);
        let pi = scr_res.p_incoh * r + no_screen_res.p_incoh * (1.0 - r);
        // delta_l_db: the document-exact dB interpolation (validation channel).
        let dl = r * scr_res.delta_l_db + (1.0 - r) * no_screen_res.delta_l_db;

        h_coh_factor.push(hc);
        p_incoh.push(pi);
        delta_l_db.push(dl);
    }

    Ok(TerrainEffect {
        h_coh_factor,
        p_incoh,
        delta_l_db,
    })
}

/// Frequency-independent refraction state for the flat channel: the
/// equivalent-linear gradient `ξ`, the equivalent ground sound speed for the
/// rays `c₀`, and the mean-profile ray pair (whose `d_sz` is the shadow-zone
/// distance, `∞` for `ξ ≥ 0`).
#[derive(Debug, Clone, Copy)]
struct RefractionState {
    xi: f64,
    c0_ray: f64,
    /// Mean-profile ray pair `circular_rays(d, hS, hR, ξ, c₀)`, computed ONCE in
    /// [`FlatChannel::from_profile`] — its geometry is band-independent, so
    /// [`FlatChannel::eval`] reads this stored pair instead of recomputing the
    /// identical `circular_rays` on every one of the ~105 bands. `rays.d_sz`
    /// supplies the shadow-zone distance (mirroring the direct ray's own
    /// construction), removing the separate `direct_ray` fetch.
    rays: RayPair,
    /// Interference travel-time difference `Δτ⁺` under the upper-refraction
    /// profile `A⁺ = A + 1.7·sA`, `B⁺ = B + 1.7·sB` (Eq. 10). Frequency-
    /// independent geometry; equals the mean-profile `Δτ` bit-for-bit when
    /// `sA=sB=0`, so the FΔν factor (Eq. 112) is then exactly 1.
    dtau_plus: f64,
}

/// The flat-ground channel (Sub-model 1 for a single surface type, Sub-model 2
/// for a segmented one) built once per `terrain_effect` call.
struct FlatChannel {
    /// Horizontal source→receiver distance, m.
    d: f64,
    /// Source / receiver heights above the flat baseline, m.
    h_s: f64,
    h_r: f64,
    /// Speed of sound, m/s.
    c0: f64,
    /// Segmented surface strips (source-relative x), grouped in Sub-model 2 form.
    strips: Vec<submodel2::SurfaceStrip>,
    /// `true` when every strip shares one `(σ, r)` type ⇒ Sub-model 1.
    single_type: bool,
    /// Refraction state (CalcEqSSP), `None` for a homogeneous atmosphere. The
    /// single-type (Sub-model 1) path consumes this precomputed mean-profile
    /// state; the segmented (Sub-model 2) path uses the raw [`Self::profile`]
    /// with the frequency-dependent `calc_eq_ssp_ground` collapse (§5.5.3).
    refraction: Option<RefractionState>,
    /// The raw weather profile, retained for the segmented (Sub-model 2) path so
    /// `calc_eq_ssp_ground` can be applied **per band** (its `(ξ, c₀)` is
    /// frequency-dependent over soft ground). `None` ⇒ homogeneous.
    profile: Option<SoundSpeedProfile>,
}

impl FlatChannel {
    fn from_profile(
        profile: &TerrainProfile,
        source: [f64; 2],
        receiver: [f64; 2],
        c0: f64,
        refraction: Option<&SoundSpeedProfile>,
    ) -> Result<Self, PropagationError> {
        // Retain the raw weather profile for the segmented (Sub-model 2) path
        // (`refraction` below is shadowed by the collapsed RefractionState).
        let weather_profile = refraction.copied();
        let d = receiver[0] - source[0];
        if !(d.is_finite() && d > 0.0) {
            return Err(PropagationError::DegenerateRayGeometry {
                detail: "flat channel requires a positive source→receiver distance",
            });
        }
        // Heights above the flat baseline z = 0 (Phase 2 targets are flat ground).
        let h_s = source[1];
        let h_r = receiver[1];

        // Ground strips: each profile segment mapped to source-relative x and
        // clamped to [0, d]. Screen segments (same σ) fold in harmlessly.
        let points = profile.points();
        let segments = profile.segments();
        let mut strips: Vec<submodel2::SurfaceStrip> = Vec::new();
        for (i, seg) in segments.iter().enumerate() {
            let x0 = (points[i][0] - source[0]).clamp(0.0, d);
            let x1 = (points[i + 1][0] - source[0]).clamp(0.0, d);
            if x1 - x0 > 1e-9 {
                strips.push(submodel2::SurfaceStrip {
                    x_start: x0,
                    x_end: x1,
                    sigma_kpa: seg.flow_resistivity,
                    roughness_r: seg.roughness,
                });
            }
        }
        if strips.is_empty() {
            return Err(PropagationError::DegenerateRayGeometry {
                detail: "flat channel found no ground strips in [0, d]",
            });
        }
        let first = (strips[0].sigma_kpa, strips[0].roughness_r);
        let single_type = strips
            .iter()
            .all(|s| s.sigma_kpa == first.0 && s.roughness_r == first.1);

        // CalcEqSSP once (frequency-independent, plan 03-01): collapse the
        // weather profile to (ξ, c₀). Below the |ξ|<1e-6 clamp it returns (0, C)
        // and the ray path stays bit-for-bit the Phase-2 straight-ray path.
        let refraction = match refraction {
            Some(p) => {
                let (xi, c0_ray) = calc_eq_ssp(h_s, h_r, p.z0, p.a, p.b, p.c)?;
                // Mean-profile ray pair, computed ONCE (band-independent): eval
                // reads this stored pair rather than recomputing the identical
                // circular_rays on every band. The shadow-zone distance dSZ
                // rides along inside the pair (`rays.d_sz`, from the direct ray's
                // own construction) — no separate direct_ray fetch needed.
                let rays = circular_rays(d, h_s, h_r, xi, c0_ray)?;
                // Upper-refraction profile A⁺=A+1.7·sA, B⁺=B+1.7·sB (Eq. 10) →
                // ξ⁺ → Δτ⁺ for the FΔν factor (Eq. 112). When sA=sB=0 the plus
                // profile equals the mean profile, so ξ⁺=ξ, c₀⁺=c₀ and Δτ⁺=Δτ
                // bit-for-bit (⇒ FΔν=1, the non-fluctuating regression guard).
                let a_plus = p.a + 1.7 * p.s_a;
                let b_plus = p.b + 1.7 * p.s_b;
                let (xi_plus, c0_plus) = calc_eq_ssp(h_s, h_r, p.z0, a_plus, b_plus, p.c)?;
                let dtau_plus = circular_rays(d, h_s, h_r, xi_plus, c0_plus)?.dtau;
                Some(RefractionState {
                    xi,
                    c0_ray,
                    rays,
                    dtau_plus,
                })
            }
            None => None,
        };

        Ok(Self {
            d,
            h_s,
            h_r,
            c0,
            strips,
            single_type,
            refraction,
            profile: weather_profile,
        })
    }

    fn eval(&self, f: f64, coh: &CoherenceInputs) -> Result<GroundResult, PropagationError> {
        if self.single_type {
            // Refraction dispatch (D-02): swap straight_rays → circular_rays when
            // a weather profile is active; |ξ|<1e-6 keeps the case bit-identical.
            if let Some(rs) = self.refraction {
                // Band-independent mean-profile pair, precomputed in
                // `from_profile` — was recomputed here per band before.
                let rays = rs.rays;
                // Upward-refraction shadow zone: no reflected ray (Eq. 45 note)
                // ⇒ the Sub-model 1 shadow branch (Eq. 121) subtracts L_SZ.
                let shadow_l_sz = if rs.xi < 0.0 && rays.reflected.is_none() {
                    Some(shadow_zone_shielding(
                        f, self.d, self.h_s, self.h_r, rs.xi, rs.c0_ray, rays.d_sz,
                    )?)
                } else {
                    None
                };
                // FΔν (Eq. 112) injected through the CoherenceInputs seam: the
                // decoherence from the mean-vs-plus travel-time difference. The
                // incoming `f_delta_nu` (default 1.0) is multiplied, never
                // overwritten; when sA=sB=0 ⇒ Δτ⁺=Δτ ⇒ factor 1.0 bit-exact so
                // this stays identical to the non-fluctuating refraction path.
                let f_delta_nu = coh.f_delta_nu * coherence_f_delta_nu(f, rays.dtau, rs.dtau_plus);
                let coh_ft = CoherenceInputs { f_delta_nu, ..*coh };
                submodel1::eval(
                    f,
                    &rays,
                    self.strips[0].sigma_kpa,
                    self.strips[0].roughness_r,
                    &coh_ft,
                    None,
                    shadow_l_sz,
                )
            } else {
                let rays = straight_rays(self.d, self.h_s, self.h_r, self.c0)?;
                submodel1::eval(
                    f,
                    &rays,
                    self.strips[0].sigma_kpa,
                    self.strips[0].roughness_r,
                    coh,
                    None,
                    None,
                )
            }
        } else {
            let geom = submodel2::FlatGeometry {
                d: self.d,
                h_s: self.h_s,
                h_r: self.h_r,
                c0: self.c0,
            };
            // Segmented ground with an active weather profile: the frequency-
            // dependent `calc_eq_ssp_ground` collapse (§5.5.3) gives the per-band
            // (ξ, c₀) over the softest ground (Ẑ_G,min — the same representative
            // impedance Sub-model 2's PhaseDiffFreq uses), then `circular_rays`
            // replaces the straight rays. |ξ|<1e-6 keeps the case bit-identical to
            // the homogeneous Sub-model 2 path (WR-01 resolved).
            if let Some(p) = self.profile {
                let sigma_min = self
                    .strips
                    .iter()
                    .map(|s| s.sigma_kpa)
                    .fold(f64::INFINITY, f64::min);
                let (xi, c0_ray) = calc_eq_ssp_ground(
                    f, self.d, self.h_s, self.h_r, sigma_min, p.z0, p.a, p.b, p.c,
                )?;
                let rays = circular_rays(self.d, self.h_s, self.h_r, xi, c0_ray)?;
                // Upward-refraction shadow zone: no reflected ray ⇒ the Eq. 121
                // shadow branch applies to every surface type.
                let shadow_l_sz = if xi < 0.0 && rays.reflected.is_none() {
                    Some(shadow_zone_shielding(
                        f, self.d, self.h_s, self.h_r, xi, c0_ray, rays.d_sz,
                    )?)
                } else {
                    None
                };
                // FΔν (Eq. 112) from the mean-vs-plus travel-time difference,
                // multiplied into the coherence seam (sA=sB=0 ⇒ factor 1 exactly).
                let a_plus = p.a + 1.7 * p.s_a;
                let b_plus = p.b + 1.7 * p.s_b;
                let (xi_plus, c0_plus) = calc_eq_ssp_ground(
                    f, self.d, self.h_s, self.h_r, sigma_min, p.z0, a_plus, b_plus, p.c,
                )?;
                let dtau_plus = circular_rays(self.d, self.h_s, self.h_r, xi_plus, c0_plus)?.dtau;
                let f_delta_nu = coh.f_delta_nu * coherence_f_delta_nu(f, rays.dtau, dtau_plus);
                let coh_ft = CoherenceInputs { f_delta_nu, ..*coh };
                return submodel2::submodel2_with_rays(
                    f,
                    &self.strips,
                    &geom,
                    &coh_ft,
                    &rays,
                    shadow_l_sz,
                );
            }
            submodel2::submodel2(f, &self.strips, &geom, coh)
        }
    }
}

/// The screen channel (Sub-model 4/5/6 per the interpretation, plus the
/// Sub-model 7 turbulence-scattering floor). Because the Phase 2 targets give
/// `r_scr2`/`r_scr12 ∈ {0,1}`, the Eq. 332 inner tree collapses to a single
/// sub-model selected by the screen class — the exact tree value.
///
/// `transmission = Some(t)` adds the straight-through partition leakage `T(f)`
/// (native `e^{−jωt}`, relative to the free-field `p̂₀`) to the coherent factor —
/// the single §5.13–5.15 composition point for semi-transparent screens/façades
/// (plan 05-03, ENG-10+). It joins the **coherent channel only**: `p_incoh` and
/// the Sub-model 7 scattered energy are untouched (Pitfall 6). `None` returns
/// `base.h_coh_factor` EXACTLY as before — a structural match, never a `+ 0.0`
/// (adding a zero complex can flip a negative-zero component's bits; the `None`
/// arm is the D-10 bit-exact guarantee).
fn screen_channel(
    f: f64,
    interp: &TerrainInterpretation,
    coh: &CoherenceInputs,
    transmission: Option<Complex<f64>>,
) -> Result<GroundResult, PropagationError> {
    use screen::{ScreenConfig, submodel4, submodel4_c_sr, submodel5, submodel6};
    use submodel7::{ScreenScatterGeometry, submodel7_delta_l};

    // Hard wedge faces (the FORCE screens are treated as hard reflectors — the
    // committed oracle's convention).
    let hard = Complex::new(1.0e9, 0.0);
    let to_scr = |s: &StripSpec| screen::SurfaceStrip {
        seg_a: s.seg_a,
        seg_b: s.seg_b,
        sigma_kpa: s.sigma_kpa,
        roughness_r: s.roughness_r,
    };
    let before: Vec<screen::SurfaceStrip> = interp.before.iter().map(to_scr).collect();
    let middle: Vec<screen::SurfaceStrip> = interp.middle.iter().map(to_scr).collect();
    let after: Vec<screen::SurfaceStrip> = interp.after.iter().map(to_scr).collect();

    // Screen shape: [W₁,T,W₂] (single), [W₁,T₁,T₂,W₂] (thick), or the two apices
    // flanked (double).
    let shape: Vec<[f64; 2]> = match interp.class {
        ScreenClass::DoubleScreen => {
            // A DoubleScreen interpretation must carry its two apex shapes; a
            // missing pair is a terrain-interpreter invariant violation (terrain
            // state is case-derived) — a typed error, never a panic on data.
            let (s1, s2) = interp.screens.ok_or(PropagationError::DegenerateProfile {
                detail: "double-screen class without its two screen shapes",
            })?;
            vec![s1[0], s1[1], s2[1], s2[2]]
        }
        _ => interp.screen.clone(),
    };

    let cfg = ScreenConfig {
        source: interp.source,
        receiver: interp.receiver,
        screen: &shape,
        before: &before,
        middle: &middle,
        after: &after,
        z_face_source: hard,
        z_face_receiver: hard,
        coh,
    };

    let (base, c_sr) = match interp.class {
        ScreenClass::SingleEdge => (submodel4(f, &cfg)?, submodel4_c_sr(f, &cfg)?),
        ScreenClass::ThickScreen => (submodel5(f, &cfg)?, 1.0),
        ScreenClass::DoubleScreen => (submodel6(f, &cfg)?, 1.0),
        ScreenClass::Flat => unreachable!("screen_channel called on flat terrain"),
    };

    // Sub-model 7 turbulence scattering (Eq. 332 additive energy). r₁/r₂ along
    // the S–R line to the primary edge foot; h_e the edge height above that line.
    let edge = interp.primary_edge();
    let r1 = edge[0] - interp.source[0];
    let r2 = interp.receiver[0] - edge[0];
    let dx = interp.receiver[0] - interp.source[0];
    let slope = if dx.abs() > 1e-12 {
        (interp.receiver[1] - interp.source[1]) / dx
    } else {
        0.0
    };
    let h_e = edge[1] - (interp.source[1] + slope * (edge[0] - interp.source[0]));
    let scatter = ScreenScatterGeometry {
        r1: r1.abs().max(1e-6),
        r2: r2.abs().max(1e-6),
        h_e,
        t_air_c: coh.t_air_c,
    };
    let dl7 = submodel7_delta_l(f, &scatter, coh.cv2, coh.ct2, c_sr)?;
    let sm7_energy = 10f64.powf(dl7 / 10.0);

    // Semi-transparent partition (plan 05-03): the straight-through leakage T(f)
    // relative to p̂₀ joins the COHERENT factor as complex pressure (native
    // e^{−jωt}, deterministic min-phase filter — ENVI extension ENG-10+). The
    // `None` arm is a structural match returning the opaque factor UNCHANGED — no
    // `+ 0.0`, which could flip a negative-zero component's bits (D-10). No
    // sub-model file changes: screen.rs stays document-exact.
    let h_coh_factor = match transmission {
        Some(t) => base.h_coh_factor + t,
        None => base.h_coh_factor,
    };

    // Sub-model 7 is energy-only (f64) — it can never touch the phase channel;
    // transmission never touches p_incoh/sm7 (Pitfall 6).
    Ok(GroundResult::from_channels(
        h_coh_factor,
        base.p_incoh + sm7_energy,
    ))
}

#[cfg(test)]
mod terrain_effect_tests {
    use super::*;
    use crate::freq::N_BANDS;
    use crate::propagation::air_absorption::Atmosphere;
    use crate::propagation::direct_path;
    use crate::propagation::sound_speed_ms;
    use crate::propagation::terrain_effect::submodel1::{SubModel1Inputs, submodel1};
    use crate::scene::{BandSpectrum, GroundSegment};
    use crate::transfer::{band_levels_db, band_levels_db_two_channel, nord_ratio_to_transfer};
    use num_complex::Complex;

    const C0: f64 = 340.348;

    fn zero_turb() -> CoherenceInputs {
        CoherenceInputs {
            cv2: 0.0,
            ct2: 0.0,
            t_air_c: 15.0,
            c0: C0,
            roughness_r: 0.0,
            f_delta_nu: 1.0,
            d_m: 97.5,
        }
    }

    fn seg(sigma: f64) -> GroundSegment {
        GroundSegment {
            flow_resistivity: sigma,
            roughness: 0.0,
        }
    }

    /// Flat σ=200 anchor geometry (hS=0.5, hR=1.5, d=97.5).
    fn flat_sigma200() -> (TerrainProfile, [f64; 3], [f64; 3]) {
        let profile =
            TerrainProfile::new(vec![[2.5, 0.0], [100.0, 0.0]], vec![seg(200.0)]).unwrap();
        (profile, [2.5, 0.0, 0.5], [100.0, 0.0, 1.5])
    }

    /// Case-71-like thin screen (4 m spike at x=15 on flat σ=200 ground).
    fn thin_screen() -> (TerrainProfile, [f64; 3], [f64; 3]) {
        let profile = TerrainProfile::new(
            vec![
                [0.0, 0.0],
                [14.99, 0.0],
                [15.0, 4.0],
                [15.01, 0.0],
                [150.0, 0.0],
            ],
            vec![seg(200.0), seg(200.0), seg(200.0), seg(200.0)],
        )
        .unwrap();
        (profile, [0.0, 0.0, 0.5], [150.0, 0.0, 1.5])
    }

    fn h_ff(src: [f64; 3], rcv: [f64; 3]) -> Vec<Complex<f64>> {
        let atmos = Atmosphere::new(15.0, 70.0, 101.325).unwrap();
        let path = crate::geometry::PathGeometry::direct(src, rcv).unwrap();
        direct_path(&path, &atmos, &FreqAxis::new()).unwrap()
    }

    fn h_coh(te: &TerrainEffect, hff: &[Complex<f64>]) -> Vec<Complex<f64>> {
        hff.iter()
            .zip(&te.h_coh_factor)
            .map(|(&hf, &factor)| hf * nord_ratio_to_transfer(factor))
            .collect()
    }

    // Test 6 (end-to-end level identity): the flat channel reproduces Sub-model 1
    // through the full transfer path within 1e-9 dB.
    #[test]
    fn flat_channel_reproduces_submodel1_through_the_transfer_path() {
        let (profile, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis, None, None).unwrap();

        // delta_l_db equals Sub-model 1 exactly (flat ⇒ r_scr1 = 0).
        let rays = straight_rays(97.5, 0.5, 1.5, C0).unwrap();
        for (i, &f) in axis.centres.iter().enumerate() {
            let inp = SubModel1Inputs {
                rays: &rays,
                sigma_kpa: 200.0,
                roughness_r: 0.0,
                coh: &coh,
            };
            let sm1 = submodel1(f, &inp).unwrap().delta_l_db;
            assert!(
                (te.delta_l_db[i] - sm1).abs() < 1e-9,
                "ΔL_t vs SM1 at {f} Hz: {} vs {sm1}",
                te.delta_l_db[i]
            );
        }

        // Full transfer path: L = L_W + 20lg|H_ff| + ΔL_t within 1e-9 dB.
        let hff = h_ff(src, rcv);
        let hc = h_coh(&te, &hff);
        let lw = 90.0;
        let spectrum = BandSpectrum::uniform(lw);
        let levels = band_levels_db_two_channel(&hc, &hff, &te.p_incoh, &spectrum);
        for i in 0..N_BANDS {
            let want = lw + 20.0 * hff[i].norm().log10() + te.delta_l_db[i];
            assert!(
                (levels[i] - want).abs() < 1e-9,
                "band {i}: got {} want {want}",
                levels[i]
            );
        }
    }

    // Test 3 (dip through both paths — Pitfall 1 pin): the dip lands on the same
    // grid point natively (argmin ΔL_t) and through the transfer path.
    #[test]
    fn dip_lands_on_the_same_grid_point_both_paths() {
        let (profile, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis, None, None).unwrap();

        let argmin = |v: &[f64]| {
            v.iter()
                .enumerate()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap()
                .0
        };
        let native = argmin(&te.delta_l_db);

        let hff = h_ff(src, rcv);
        let hc = h_coh(&te, &hff);
        let unit = BandSpectrum::uniform(0.0);
        let levels = band_levels_db_two_channel(&hc, &hff, &te.p_incoh, &unit);
        let transfer = argmin(&levels);
        assert_eq!(
            native, transfer,
            "dip grid point must match: native {native} vs transfer {transfer}"
        );
        // The dip sits in the 630/667 Hz neighbourhood (σ=200 anchor).
        let f_dip = axis.centres[native];
        assert!(
            (f_dip - 630.96).abs() < 1.0 || (f_dip - 667.42).abs() < 1.0,
            "dip at {f_dip} Hz"
        );
    }

    // Test 4 (phase liveness across ALL operators): H_coh carries a live
    // imaginary part after ground AND after ground+screen.
    #[test]
    fn phase_is_live_through_ground_and_screen() {
        let coh = CoherenceInputs {
            cv2: 0.12,
            ct2: 0.008,
            d_m: 150.0,
            ..zero_turb()
        };
        let axis = FreqAxis::new();
        for (profile, src, rcv) in [flat_sigma200(), thin_screen()] {
            let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis, None, None).unwrap();
            let hff = h_ff(src, rcv);
            let hc = h_coh(&te, &hff);
            let live = hc.iter().filter(|z| z.im.abs() > 1e-12).count();
            assert!(
                live as f64 >= 0.9 * N_BANDS as f64,
                "H_coh must be phase-live at ≥90% of bands (got {live})"
            );
            // arg(H_coh) differs from arg(H_ff) somewhere (the terrain factor's
            // phase is real and live, not stripped).
            let differs = hc
                .iter()
                .zip(&hff)
                .any(|(&c, &f)| (c.arg() - f.arg()).abs() > 1e-6);
            assert!(differs, "terrain factor must move the phase");
            // Every value finite.
            assert!(hc.iter().all(|z| z.re.is_finite() && z.im.is_finite()));
        }
    }

    // Test 5 (P_incoh separation): zeroing p_incoh leaves the coherent readout;
    // p_incoh → 0 as the field becomes coherent (small Δτ, zero turbulence).
    #[test]
    fn p_incoh_is_readout_only_and_vanishes_when_coherent() {
        let (profile, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let te = terrain_effect(&profile, src, rcv, C0, &coh, &axis, None, None).unwrap();

        // Structural: band_levels_db_two_channel with p_incoh = 0 equals the pure
        // coherent readout band_levels_db(H_coh) — p_incoh never alters phase.
        let hff = h_ff(src, rcv);
        let hc = h_coh(&te, &hff);
        let lw = BandSpectrum::uniform(70.0);
        let zero = vec![0.0_f64; N_BANDS];
        let coherent_only = band_levels_db_two_channel(&hc, &hff, &zero, &lw);
        let pure = band_levels_db(&hc, &lw);
        for (a, b) in coherent_only.iter().zip(&pure) {
            assert!((a - b).abs() < 1e-12);
        }
        // p_incoh non-negative everywhere; near-coherent geometry ⇒ tiny.
        assert!(te.p_incoh.iter().all(|&p| p >= 0.0));

        // A tiny-Δτ geometry drives Ff → 1 ⇒ p_incoh ≪ |h_coh_factor|².
        let low = TerrainProfile::new(vec![[0.0, 0.0], [1000.0, 0.0]], vec![seg(200.0)]).unwrap();
        let te2 = terrain_effect(
            &low,
            [0.0, 0.0, 0.02],
            [1000.0, 0.0, 0.05],
            C0,
            &CoherenceInputs {
                d_m: 1000.0,
                ..zero_turb()
            },
            &axis,
            None,
            None,
        )
        .unwrap();
        for (p, h) in te2.p_incoh.iter().zip(&te2.h_coh_factor) {
            assert!(
                *p < 1e-2 * h.norm_sqr().max(1e-12) + 1e-6,
                "near-coherent p_incoh {p} not ≪ |h|² {}",
                h.norm_sqr()
            );
        }
        let _ = sound_speed_ms(15.0);
    }

    // Test 1 (Eq. 332 tree / dispatch): flat ⇒ SM1 path (r_scr1=0); screen ⇒
    // screen path (r_scr1→1) with a strictly different, finite spectrum.
    #[test]
    fn dispatch_selects_flat_vs_screen_branch() {
        let axis = FreqAxis::new();
        let coh = CoherenceInputs {
            cv2: 0.12,
            ct2: 0.008,
            d_m: 150.0,
            ..zero_turb()
        };
        let (fp, fsrc, frcv) = flat_sigma200();
        let flat = terrain_effect(&fp, fsrc, frcv, C0, &coh, &axis, None, None).unwrap();
        let (sp, ssrc, srcv) = thin_screen();
        let screen = terrain_effect(&sp, ssrc, srcv, C0, &coh, &axis, None, None).unwrap();

        // The screen deeply attenuates the mid/high band vs flat ground.
        let idx_1k = 64;
        assert!(
            screen.delta_l_db[idx_1k] < flat.delta_l_db[idx_1k] - 5.0,
            "screen must attenuate more than flat at 1 kHz: {} vs {}",
            screen.delta_l_db[idx_1k],
            flat.delta_l_db[idx_1k]
        );
        // Everything finite.
        assert!(
            screen
                .delta_l_db
                .iter()
                .chain(&flat.delta_l_db)
                .all(|v| v.is_finite())
        );
    }

    fn profile(a: f64, b: f64) -> SoundSpeedProfile {
        SoundSpeedProfile {
            a,
            b,
            c: C0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.01,
        }
    }

    fn fluctuating_profile(a: f64, b: f64, s_a: f64, s_b: f64) -> SoundSpeedProfile {
        SoundSpeedProfile {
            a,
            b,
            c: C0,
            s_a,
            s_b,
            z0: 0.01,
        }
    }

    // Refraction dispatch regression (D-02): a homogeneous SoundSpeedProfile
    // (A=B=0 ⇒ |ξ|<1e-6) yields a level bit-identical to the None (Phase-2) path.
    #[test]
    fn refraction_homogeneous_profile_is_bit_identical() {
        let (fp, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let base = terrain_effect(&fp, src, rcv, C0, &coh, &axis, None, None).unwrap();
        let hom = terrain_effect(
            &fp,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&SoundSpeedProfile::homogeneous(C0)),
            None,
        )
        .unwrap();
        for i in 0..N_BANDS {
            assert_eq!(
                base.delta_l_db[i], hom.delta_l_db[i],
                "band {i}: homogeneous refraction must match the Phase-2 path bit-for-bit"
            );
        }
    }

    // Direction property (D-02 rung c): downwind refraction (ξ>0) gives a level
    // GAIN somewhere vs the homogeneous baseline; upward refraction into a shadow
    // zone (ξ<0, long range) gives a LOSS. Compared by BAND INDEX (D-14).
    #[test]
    fn refraction_direction_downwind_gains_upwind_shadow_loses() {
        let coh = zero_turb();
        let axis = FreqAxis::new();
        // Downwind at the σ=200 anchor geometry.
        let (fp, src, rcv) = flat_sigma200();
        let base = terrain_effect(&fp, src, rcv, C0, &coh, &axis, None, None).unwrap();
        let down = terrain_effect(
            &fp,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&profile(1.0, 0.05)),
            None,
        )
        .unwrap();
        let max_gain = (0..N_BANDS)
            .map(|i| down.delta_l_db[i] - base.delta_l_db[i])
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max_gain > 0.5,
            "downwind refraction must gain at some band (max Δ = {max_gain:.3} dB)"
        );

        // Upward refraction into a shadow zone: strong negative gradient so the
        // receiver sits beyond 0.95·dSZ at the anchor distance.
        let up = terrain_effect(
            &fp,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&profile(-1.5, -0.08)),
            None,
        )
        .unwrap();
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        assert!(
            mean(&up.delta_l_db) < mean(&base.delta_l_db) - 1.0,
            "upward-refraction shadow must lose vs homogeneous: up {:.2} vs base {:.2}",
            mean(&up.delta_l_db),
            mean(&base.delta_l_db)
        );
    }

    // WR-01 (now wired): a refraction profile over SEGMENTED ground routes
    // through `calc_eq_ssp_ground` + `circular_rays` (Sub-model 2) — a finite
    // result, never a NaN and never the old typed error. A homogeneous
    // (A=B=0) weather profile is BIT-IDENTICAL to the `None` path (D-02).
    #[test]
    fn segmented_refraction_is_wired_and_homogeneous_limit_is_bit_identical() {
        let axis = FreqAxis::new();
        let coh = zero_turb();
        // Flat ground, two surface types (σ=200 then σ=20) ⇒ Sub-model 2 path.
        let seg_profile = TerrainProfile::new(
            vec![[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]],
            vec![seg(200.0), seg(20.0)],
        )
        .unwrap();
        let src = [0.0, 0.0, 0.5];
        let rcv = [100.0, 0.0, 1.5];
        let base = terrain_effect(&seg_profile, src, rcv, C0, &coh, &axis, None, None).unwrap();
        // A homogeneous weather profile (A=B=0 ⇒ |ξ|<1e-6) is bit-identical.
        let hom = terrain_effect(
            &seg_profile,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&SoundSpeedProfile::homogeneous(C0)),
            None,
        )
        .unwrap();
        for i in 0..N_BANDS {
            assert_eq!(
                base.delta_l_db[i], hom.delta_l_db[i],
                "band {i}: homogeneous segmented refraction must match the None path"
            );
        }
        // A real downwind profile now COMPUTES (finite, no NaN, no typed error).
        let down = terrain_effect(
            &seg_profile,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&profile(1.0, 0.05)),
            None,
        )
        .unwrap();
        assert!(
            down.delta_l_db.iter().all(|v| v.is_finite())
                && down.p_incoh.iter().all(|p| p.is_finite() && *p >= 0.0)
                && down
                    .h_coh_factor
                    .iter()
                    .all(|z| z.re.is_finite() && z.im.is_finite()),
            "segmented refraction must produce a finite two-channel result"
        );
    }

    // Pitfall 9: a SCREEN terrain under an active weather profile is a typed
    // error (never a silently-unrefracted screen). The same screen with `None`
    // (homogeneous) evaluates fine — the guard is refraction-specific.
    #[test]
    fn weather_over_a_screen_is_a_typed_error_not_silent() {
        let axis = FreqAxis::new();
        let coh = zero_turb();
        let (sp, ssrc, srcv) = thin_screen();
        // Homogeneous (None) computes fine.
        assert!(terrain_effect(&sp, ssrc, srcv, C0, &coh, &axis, None, None).is_ok());
        // With a weather profile: a typed WeatherScreenNotImplemented error,
        // never a silent unrefracted screen result.
        let err = terrain_effect(
            &sp,
            ssrc,
            srcv,
            C0,
            &coh,
            &axis,
            Some(&profile(1.0, 0.05)),
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, PropagationError::WeatherScreenNotImplemented { .. }),
            "weather+screen must be a typed error, got {err:?}"
        );
    }

    // SM8 decision (Eq. 279): the short-range downwind FORCE geometry stays below
    // the N ≥ 4 multiple-ground-reflection threshold, so Sub-models 1–6 carry all
    // significant rays (accepted-gap evidence, Assumption A6).
    #[test]
    fn submodel8_ray_count_short_range_downwind_below_threshold() {
        let n = submodel8::ray_count_eq279(100.0, 0.01, 1.5, 0.43, C0);
        assert!(
            n < submodel8::SM8_RAY_COUNT_THRESHOLD,
            "short-range downwind N = {n} must stay below the SM8 threshold"
        );
    }

    // FΔν wiring (Eq. 112): with sA=sB=0 the fluctuating-refraction factor is
    // exactly 1, so a refracted case is bit-for-bit identical whether or not the
    // profile carries (zero) fluctuation std-devs — the non-fluctuating guard.
    #[test]
    fn f_delta_nu_zero_fluctuation_is_bit_identical() {
        let (fp, src, rcv) = flat_sigma200();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let base = terrain_effect(
            &fp,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&profile(1.0, 0.05)),
            None,
        )
        .unwrap();
        let with_zero_std = terrain_effect(
            &fp,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&fluctuating_profile(1.0, 0.05, 0.0, 0.0)),
            None,
        )
        .unwrap();
        for i in 0..N_BANDS {
            assert_eq!(
                base.delta_l_db[i], with_zero_std.delta_l_db[i],
                "band {i}: sA=sB=0 must be bit-identical to the non-fluctuating path"
            );
        }
    }

    // FΔν direction (D-11/D-12): fluctuating refraction decoheres the coherent
    // two-ray cancellation, so the deepest interference dip becomes SHALLOWER
    // (less negative) than the non-fluctuating baseline — the coherent cross term
    // is weakened and the incoherent floor rises. Compared by BAND INDEX (D-14).
    #[test]
    fn f_delta_nu_fluctuation_fills_the_dip() {
        // Turbulence on (nonzero Cv²/CT²) so P_incoh is a live channel.
        let coh = CoherenceInputs {
            cv2: 0.12,
            ct2: 0.008,
            ..zero_turb()
        };
        let axis = FreqAxis::new();
        let (fp, src, rcv) = flat_sigma200();
        let base = terrain_effect(
            &fp,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            Some(&profile(1.0, 0.05)),
            None,
        )
        .unwrap();
        let fluct = terrain_effect(
            &fp,
            src,
            rcv,
            C0,
            &coh,
            &axis,
            // Sizeable fluctuation std-devs ⇒ Δτ⁺ ≠ Δτ ⇒ FΔν < 1.
            Some(&fluctuating_profile(1.0, 0.05, 0.6, 0.03)),
            None,
        )
        .unwrap();

        // The deepest COHERENT null (min |h_coh_factor|²) is where the two-ray
        // cancellation is strongest; there decoherence (FΔν<1) provably raises
        // the level — both the coherent cross term and the incoherent floor grow.
        let dip = base
            .h_coh_factor
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.norm_sqr().partial_cmp(&b.1.norm_sqr()).unwrap())
            .unwrap()
            .0;
        assert!(
            fluct.delta_l_db[dip] > base.delta_l_db[dip] + 0.1,
            "fluctuating refraction must fill the dip: fluct {:.3} vs base {:.3} (band {dip})",
            fluct.delta_l_db[dip],
            base.delta_l_db[dip]
        );
        // And the fluctuating spectrum genuinely differs across many bands.
        let changed = (0..N_BANDS)
            .filter(|&i| (fluct.delta_l_db[i] - base.delta_l_db[i]).abs() > 1e-9)
            .count();
        assert!(
            changed > N_BANDS / 4,
            "FΔν must move a substantial share of bands (got {changed})"
        );
        // Everything stays finite.
        assert!(
            fluct
                .delta_l_db
                .iter()
                .zip(&fluct.h_coh_factor)
                .zip(&fluct.p_incoh)
                .all(|((d, h), p)| d.is_finite()
                    && h.re.is_finite()
                    && h.im.is_finite()
                    && p.is_finite())
        );
    }

    // Finiteness sweep: every 105-band level is finite across up / down / shadow
    // geometries (T-03-01-01) — never NaN/Inf.
    #[test]
    fn refraction_finiteness_sweep_all_bands() {
        let coh = zero_turb();
        let axis = FreqAxis::new();
        let (fp, src, rcv) = flat_sigma200();
        for prof in [
            profile(1.0, 0.05),   // downwind
            profile(-1.5, -0.08), // upward / shadow
            profile(0.5, 0.0),    // log-only downward
            profile(-0.5, 0.02),  // mixed
        ] {
            let te = terrain_effect(&fp, src, rcv, C0, &coh, &axis, Some(&prof), None).unwrap();
            assert_eq!(te.delta_l_db.len(), N_BANDS);
            for i in 0..N_BANDS {
                assert!(
                    te.delta_l_db[i].is_finite()
                        && te.h_coh_factor[i].re.is_finite()
                        && te.h_coh_factor[i].im.is_finite()
                        && te.p_incoh[i].is_finite(),
                    "non-finite refracted level (A={}, B={}) at band {i}",
                    prof.a,
                    prof.b
                );
            }
        }
    }
}
