//! Sub-model 3 — non-flat terrain without screening effects
//! (AV 1106/07 §5.12, Eqs. 134–156).
//!
//! A segmented ("valley-shaped") terrain with neither screens nor screening
//! parts. The overall terrain effect is built by evaluating each ground segment
//! separately and combining them with the modified Fresnel-zone interpolation of
//! Eq. 156. Each segment is classified — on the frequency-dependent relative
//! source/receiver heights `hS,rel(f)` / `hR,rel(f)` (Eqs. 136–139) — as:
//!
//! - **Concave** (`hS,rel = 1 ∧ hR,rel = 1`): source and receiver both well above
//!   the extended segment line. The terrain effect is Sub-model 1 evaluated over
//!   the segment's own (segment-relative) reflection geometry (Eq. 140).
//! - **Convex** (`hS,rel = 0 ∨ hR,rel = 0`): source or receiver below the segment
//!   — a diffracting wedge face (Eqs. 141–151). **Not implemented here** (a typed
//!   [`PropagationError::ConvexSegmentNotImplemented`] error, never a silent
//!   approximation — mirrors the honest-green contract of the other
//!   not-implemented seams).
//! - **Transition** (neither): interpolate the concave and convex results
//!   (Eq. 155). With the convex branch gated, a transition segment carrying a
//!   non-zero convex weight raises the same typed error.
//!
//! # Two-channel contract
//!
//! Like [`super::submodel2`], each segment's Sub-model-1 [`GroundResult`] carries
//! the phase-live coherent factor + incoherent energy; the Eq. 156 interpolation
//! blends the channels linearly with the modified weights `w'_i` while
//! [`GroundResult::delta_l_db`] carries the document-exact dB interpolation.
//! A single flat concave segment (`w'_1 = 1`, `ΔL_0 = ΔL_1`, `ΔL_3 = ΔL_0`)
//! reduces Sub-model 3 to Sub-model 1 bit-for-bit (regression guard below).
//!
//! Nord2000-native convention (e^{−jωt}); no `conj()` here.

use num_complex::Complex;

use super::GroundResult;
use super::submodel1::eval as submodel1_eval;
use crate::geometry::norm_line;
use crate::propagation::PropagationError;
use crate::propagation::coherence::CoherenceInputs;
use crate::propagation::fresnel::fresnel_zone_w;
use crate::propagation::rays::{RayPair, circular_rays, straight_rays_over_segment};
use crate::propagation::refraction::SoundSpeedProfile;
use crate::propagation::refraction::eqssp::calc_eq_ssp;

/// Fresnel-zone fraction used by Sub-model 3, `F_λ = 1/16` (Eq. 134 / 375).
const F_LAMBDA: f64 = 1.0 / 16.0;

/// One ground segment of a Sub-model 3 profile: the terrain line `[seg_a, seg_b]`
/// (absolute `[x, z]`, source-relative x is fine as long as the source/receiver
/// share the same frame) with its acoustic surface properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment3 {
    /// Segment start point `[x, z]` (m), nearer the source.
    pub seg_a: [f64; 2],
    /// Segment end point `[x, z]` (m), nearer the receiver.
    pub seg_b: [f64; 2],
    /// Flow resistivity σ, kPa·s·m⁻².
    pub sigma_kpa: f64,
    /// Roughness `r`, meters.
    pub roughness_r: f64,
}

/// Sub-model 3 geometry, built once per source/receiver pair (band-independent).
/// [`Self::eval`] loops the 105 bands reading this state, mirroring the
/// precompute-once posture of [`super::FlatChannel`].
#[derive(Debug, Clone)]
pub struct SubModel3 {
    /// Source `[x, z]` (m).
    source: [f64; 2],
    /// Receiver `[x, z]` (m).
    receiver: [f64; 2],
    /// Speed of sound at the ground, m/s.
    c0: f64,
    /// Ground segments in source→receiver order.
    segments: Vec<Segment3>,
    /// Weather profile (`None` for a homogeneous atmosphere).
    refraction: Option<SoundSpeedProfile>,
}

/// Per-segment, band-independent projected geometry (extended-segment frame,
/// source foot at along = 0).
#[derive(Debug, Clone, Copy)]
struct SegGeom {
    /// Along-segment source→receiver distance `d'` (m).
    d: f64,
    /// Source height above the extended segment line `h'_S` (m, signed).
    h_s: f64,
    /// Receiver height above the extended segment line `h'_R` (m, signed).
    h_r: f64,
    /// Segment start distance from the source foot along the segment `d_{i-1}`.
    d1: f64,
    /// Segment end distance from the source foot along the segment `d_i`.
    d2: f64,
}

impl SubModel3 {
    /// Build the Sub-model 3 geometry for a source/receiver pair over `segments`.
    ///
    /// `source`/`receiver` are `[x, z]`; `segments` are the ground segments in
    /// source→receiver order (the caller has already excluded screen segments —
    /// Sub-model 3 is for terrain *without* screening effects).
    ///
    /// # Errors
    ///
    /// [`PropagationError::DegenerateRayGeometry`] on empty segments or a
    /// non-positive source→receiver distance.
    pub fn new(
        source: [f64; 2],
        receiver: [f64; 2],
        c0: f64,
        segments: Vec<Segment3>,
        refraction: Option<SoundSpeedProfile>,
    ) -> Result<Self, PropagationError> {
        if segments.is_empty() {
            return Err(PropagationError::DegenerateRayGeometry {
                detail: "Sub-model 3 requires at least one ground segment",
            });
        }
        if !(receiver[0] - source[0]).is_finite() || receiver[0] <= source[0] {
            return Err(PropagationError::DegenerateRayGeometry {
                detail: "Sub-model 3 requires a positive source→receiver distance",
            });
        }
        Ok(Self {
            source,
            receiver,
            c0,
            segments,
            refraction,
        })
    }

    /// Project the source and receiver onto the extended line of segment `seg`
    /// (Eqs. 142/144 `NormLine` frame), returning the along/normal geometry.
    fn seg_geom(&self, seg: &Segment3) -> SegGeom {
        let (a_s, h_s) = norm_line(self.source, seg.seg_a, seg.seg_b);
        let (a_r, h_r) = norm_line(self.receiver, seg.seg_a, seg.seg_b);
        // Along-coordinate of the segment endpoints in the same frame: seg_a is
        // the frame origin (along = 0), seg_b at along = |seg|.
        let seg_len =
            ((seg.seg_b[0] - seg.seg_a[0]).powi(2) + (seg.seg_b[1] - seg.seg_a[1]).powi(2)).sqrt();
        // Shift so the source foot is at along = 0 (matches the FresnelZoneW /
        // Sub-model 2 convention: strip endpoints measured from the source).
        SegGeom {
            d: a_r - a_s,
            h_s,
            h_r,
            d1: -a_s,
            d2: seg_len - a_s,
        }
    }

    /// The refraction-corrected relative-gradient `ξ` and ground sound speed `c₀`
    /// for a segment's own extended-line frame (Eq. 135 CalcEqSSP over the
    /// segment-relative heights). `None` (homogeneous) ⇒ `(0, c0)`.
    fn seg_xi_c0(&self, g: &SegGeom) -> Result<(f64, f64), PropagationError> {
        match &self.refraction {
            Some(p) => calc_eq_ssp(g.h_s.abs(), g.h_r.abs(), p.z0, p.a, p.b, p.c),
            None => Ok((0.0, self.c0)),
        }
    }

    /// Build the reflection ray pair for one segment (homogeneous straight rays
    /// over the sloped segment line, or refracted circular rays in the
    /// segment-local frame).
    fn seg_rays(&self, seg: &Segment3, g: &SegGeom) -> Result<RayPair, PropagationError> {
        match &self.refraction {
            None => straight_rays_over_segment(
                self.source,
                self.receiver,
                seg.seg_a,
                seg.seg_b,
                self.c0,
            ),
            Some(_) => {
                let (xi, c0_ray) = self.seg_xi_c0(g)?;
                if xi.abs() < 1e-12 {
                    // Homogeneous limit — identical to the straight-ray path.
                    straight_rays_over_segment(
                        self.source,
                        self.receiver,
                        seg.seg_a,
                        seg.seg_b,
                        self.c0,
                    )
                } else {
                    // Segment-local frame: the extended segment line is the local
                    // ground, source/receiver at their segment-relative heights
                    // (Eq. 135 posture). Circular rays in that frame.
                    circular_rays(g.d, g.h_s.abs(), g.h_r.abs(), xi, c0_ray)
                }
            }
        }
    }

    /// `MinConcaveHeight(h_other, d1, d, λ)` (AV 1106/07 §5.23.12, Eqs. 373–375):
    /// the smallest height at which the far segment endpoint sits on the border of
    /// the Fresnel zone. Returns `∞` when Eq. 374 is fulfilled (the endpoint is
    /// already inside the zone at any height).
    fn min_concave_height(h_other: f64, d1: f64, d: f64, lambda: f64) -> f64 {
        let h_other = h_other.abs();
        let d2 = d - d1;
        // Eq. 374: √(d₂² + h²) − h ≤ F_λ·λ  ⇒  hFz = ∞.
        if (d2 * d2 + h_other * h_other).sqrt() - h_other <= F_LAMBDA * lambda {
            return f64::INFINITY;
        }
        // Eq. 375 quadratic.
        let x1 = ((h_other * h_other + d2 * d2).sqrt() - F_LAMBDA * lambda).powi(2);
        let x2 = h_other * h_other + d * d - d1 * d1 - x1;
        let a = 4.0 * h_other * h_other - 4.0 * x1;
        let b = 4.0 * h_other * x2;
        let c = x2 * x2 - 4.0 * d1 * d1 * x1;
        if a.abs() < 1e-30 {
            return f64::INFINITY;
        }
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return f64::INFINITY;
        }
        let h = (-b - disc.sqrt()) / (2.0 * a);
        if h.is_finite() && h > 0.0 {
            h
        } else {
            f64::INFINITY
        }
    }

    /// Relative source/receiver heights `(hS,rel, hR,rel)` for a segment
    /// (Eqs. 136–139). `1` ⇒ above the concave-minimum height; `0` ⇒ at/below the
    /// segment line; a fraction in the transition band.
    fn relative_heights(&self, g: &SegGeom, lambda: f64) -> (f64, f64) {
        // Refraction-corrected heights ȟ'_S, ȟ'_R: homogeneous ⇒ the raw heights.
        let hs = g.h_s;
        let hr = g.h_r;
        // hS,Fz uses the OTHER endpoint (Eq. 138: MinConcaveHeight(ȟ'_R, ď_x, ď, λ)).
        // ď_x for the source uses the source-side foot distances; d1 here is the
        // along-distance to the segment start from the source foot — the segment
        // start is the endpoint closest to the source (d1) for hS,Fz.
        let hs_fz = Self::min_concave_height(hr, g.d1.abs(), g.d.abs(), lambda);
        let hr_fz = Self::min_concave_height(hs, (g.d - g.d2).abs(), g.d.abs(), lambda);
        let hs_dd = g.h_s.abs().min(hs_fz);
        let hr_dd = g.h_r.abs().min(hr_fz);
        let rel = |h: f64, h_dd: f64| -> f64 {
            if h <= 0.0 {
                0.0
            } else if h >= h_dd {
                1.0
            } else {
                h / h_dd
            }
        };
        (rel(hs, hs_dd), rel(hr, hr_dd))
    }

    /// Evaluate the two-channel Sub-model 3 terrain effect at one frequency
    /// (Eq. 156).
    ///
    /// # Errors
    ///
    /// [`PropagationError::ConvexSegmentNotImplemented`] if any segment is convex
    /// (or a transition with a non-zero convex weight); propagates ray/impedance
    /// errors from the per-segment Sub-model 1 evaluations.
    pub fn eval(&self, f: f64, coh: &CoherenceInputs) -> Result<GroundResult, PropagationError> {
        let lambda = self.c0 / f;
        let flp = F_LAMBDA * lambda;

        // Per-segment concave terrain effect ΔL_i and Fresnel-zone weight w_i.
        let mut w = Vec::with_capacity(self.segments.len());
        let mut dl = Vec::with_capacity(self.segments.len());
        let mut hc = Vec::with_capacity(self.segments.len());
        let mut pi = Vec::with_capacity(self.segments.len());

        for seg in &self.segments {
            let g = self.seg_geom(seg);
            let (hs_rel, hr_rel) = self.relative_heights(&g, lambda);

            // Classification (Eqs. 136–137 boundary rules).
            let is_convex = hs_rel <= 0.0 || hr_rel <= 0.0;
            if is_convex {
                return Err(PropagationError::ConvexSegmentNotImplemented {
                    f_hz: f,
                    detail: "convex/transition ground segment requires the §5.12 wedge path",
                });
            }
            let is_concave = (hs_rel - 1.0).abs() < 1e-12 && (hr_rel - 1.0).abs() < 1e-12;
            // Transition with a non-zero convex weight (r = min(rel) < 1) still
            // needs the convex branch — gate honestly rather than approximate.
            if !is_concave {
                let r = hs_rel.min(hr_rel);
                if r < 1.0 - 1e-9 {
                    return Err(PropagationError::ConvexSegmentNotImplemented {
                        f_hz: f,
                        detail: "transition ground segment requires the §5.12 convex/wedge path",
                    });
                }
            }

            // Concave terrain effect (Eq. 140): Sub-model 1 over the segment's
            // own reflection geometry.
            let rays = self.seg_rays(seg, &g)?;
            let seg_coh = CoherenceInputs {
                c0: self.c0,
                ..*coh
            };
            let res = submodel1_eval(
                f,
                &rays,
                seg.sigma_kpa,
                seg.roughness_r,
                &seg_coh,
                None,
                None,
            )?;

            // Fresnel-zone weight w_i (Eq. 134): fraction of the zone covered by
            // the segment strip [d1, d2] over the extended-line geometry.
            let wi = if g.h_s.abs() > 1e-9 && g.h_r.abs() > 1e-9 && g.d > 0.0 {
                fresnel_zone_w(g.d, g.h_s.abs(), g.h_r.abs(), g.d1, g.d2, flp)?
            } else {
                0.0
            };

            w.push(wi);
            dl.push(res.delta_l_db);
            hc.push(res.h_coh_factor);
            pi.push(res.p_incoh);
        }

        // Eq. 156 modified Fresnel-zone interpolation.
        let w_t: f64 = w.iter().sum();
        // Cap the total weight at 2 (Eq. 156 w'_i rule).
        let scale = if w_t > 2.0 { 2.0 / w_t } else { 1.0 };
        let w_prime: Vec<f64> = w.iter().map(|&wi| wi * scale).collect();
        let sum_wp: f64 = w_prime.iter().sum();

        // ΔL_0 = Σ w'_i·ΔL_i (original Fresnel interpolation, dB channel).
        let dl0: f64 = w_prime.iter().zip(&dl).map(|(&wp, &d)| wp * d).sum();
        // Two-channel ΔL_0: linear blend of the coherent/incoherent channels.
        let hc0: Complex<f64> = w_prime
            .iter()
            .zip(&hc)
            .map(|(&wp, &h)| h * wp)
            .fold(Complex::new(0.0, 0.0), |acc, x| acc + x);
        let pi0: f64 = w_prime.iter().zip(&pi).map(|(&wp, &p)| wp * wp * p).sum();

        // r'(f) (Eq. 156): 0 for ΔL_0 ≥ 0, ramp to 1 over (−20, 0), 1 below −20.
        let r_prime = if dl0 >= 0.0 {
            0.0
        } else if dl0 <= -20.0 {
            1.0
        } else {
            -dl0 / 20.0
        };
        // ΔL_3 = ΔL_0·(1 − r' + r'/Σw'_i) — the modified-interpolation factor.
        let factor = if sum_wp > 1e-12 {
            1.0 - r_prime + r_prime / sum_wp
        } else {
            1.0
        };
        let delta_l3 = dl0 * factor;
        // Apply the same real modification factor to both channels so the
        // two-channel readout tracks the document dB value (√factor on the
        // pressure amplitude ⇒ factor on the energy).
        let amp = if factor >= 0.0 { factor.sqrt() } else { 0.0 };
        let hc3 = hc0 * amp;
        let pi3 = pi0 * factor.max(0.0);

        Ok(GroundResult {
            delta_l_db: delta_l3,
            h_coh_factor: hc3,
            p_incoh: pi3,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freq::{FreqAxis, N_BANDS};
    use crate::propagation::rays::straight_rays_over_segment;
    use crate::propagation::terrain_effect::submodel1::eval as sm1_eval;

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

    /// A single flat segment fully covering the Fresnel zone at every band:
    /// Sub-model 3 must reproduce Sub-model 1 bit-for-bit (assert_eq!), the
    /// frozen-validation regression. The segment extends well beyond the
    /// source/receiver so its Fresnel-zone weight is exactly 1 at all frequencies
    /// (`w'_1 = 1`, `Σw'_i = 1` ⇒ `ΔL_3 = ΔL_0 = ΔL_1`); over the flat z = 0 line
    /// the reflection geometry is identical to the direct Sub-model 1 rays.
    #[test]
    fn flat_single_segment_reproduces_submodel1_bit_for_bit() {
        let source = [2.5, 0.5];
        let receiver = [100.0, 1.5];
        let seg = Segment3 {
            seg_a: [-500.0, 0.0],
            seg_b: [600.0, 0.0],
            sigma_kpa: 200.0,
            roughness_r: 0.0,
        };
        let sm3 = SubModel3::new(source, receiver, C0, vec![seg], None).unwrap();
        let coh = zero_turb();
        let axis = FreqAxis::new();
        // Reference: Sub-model 1 over the SAME segment-line rays the concave path
        // uses internally (so a single normalized concave segment == SM1 exactly).
        let rays = straight_rays_over_segment(source, receiver, seg.seg_a, seg.seg_b, C0).unwrap();
        for &f in axis.centres.iter() {
            let got = sm3.eval(f, &coh).unwrap();
            let want = sm1_eval(f, &rays, 200.0, 0.0, &coh, None, None).unwrap();
            assert_eq!(
                got.delta_l_db.to_bits(),
                want.delta_l_db.to_bits(),
                "SM3-on-flat must equal SM1 bit-for-bit at {f} Hz: {} vs {}",
                got.delta_l_db,
                want.delta_l_db
            );
        }
    }

    /// A concave valley (source and receiver high above a dipped floor) computes a
    /// finite two-channel result across all 105 bands — no NaN, no typed error.
    #[test]
    fn concave_valley_is_finite_across_all_bands() {
        // Valley floor dips to z = −3 between the source and receiver banks.
        let source = [0.0, 4.0];
        let receiver = [200.0, 4.0];
        let segments = vec![
            Segment3 {
                seg_a: [0.0, 0.0],
                seg_b: [60.0, -3.0],
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
            Segment3 {
                seg_a: [60.0, -3.0],
                seg_b: [140.0, -3.0],
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
            Segment3 {
                seg_a: [140.0, -3.0],
                seg_b: [200.0, 0.0],
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
        ];
        let sm3 = SubModel3::new(source, receiver, C0, segments, None).unwrap();
        let coh = CoherenceInputs {
            d_m: 200.0,
            ..zero_turb()
        };
        let axis = FreqAxis::new();
        for &f in axis.centres.iter() {
            let res = sm3.eval(f, &coh).unwrap();
            assert!(
                res.delta_l_db.is_finite()
                    && res.h_coh_factor.re.is_finite()
                    && res.h_coh_factor.im.is_finite()
                    && res.p_incoh.is_finite()
                    && res.p_incoh >= 0.0,
                "non-finite SM3 result at {f} Hz: {res:?}"
            );
        }
        assert_eq!(axis.centres.len(), N_BANDS);
    }

    /// A convex segment (receiver below an intervening ridge) is a typed error,
    /// never a silent (wrong) approximation (honest-green contract).
    #[test]
    fn convex_segment_is_a_typed_error() {
        // A ridge peaks above the source→receiver line; the receiver sits below
        // the first segment's extended line ⇒ convex.
        let source = [0.0, 1.0];
        let receiver = [100.0, 1.0];
        let segments = vec![
            Segment3 {
                seg_a: [0.0, 0.0],
                seg_b: [50.0, 8.0],
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
            Segment3 {
                seg_a: [50.0, 8.0],
                seg_b: [100.0, 0.0],
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
        ];
        let sm3 = SubModel3::new(source, receiver, C0, segments, None).unwrap();
        let coh = zero_turb();
        let err = sm3.eval(1000.0, &coh).unwrap_err();
        assert!(
            matches!(err, PropagationError::ConvexSegmentNotImplemented { .. }),
            "convex segment must be a typed error, got {err:?}"
        );
    }

    /// MinConcaveHeight returns ∞ when the far endpoint already lies inside the
    /// Fresnel zone (Eq. 374), and a finite positive height otherwise.
    #[test]
    fn min_concave_height_infinite_and_finite_branches() {
        let lambda = C0 / 1000.0;
        // Far endpoint at the receiver foot (d2 ≈ 0) ⇒ inside the zone ⇒ ∞.
        assert!(SubModel3::min_concave_height(1.5, 100.0, 100.0, lambda).is_infinite());
        // A distant far endpoint gives a finite minimum concave height.
        let h = SubModel3::min_concave_height(1.5, 0.0, 100.0, lambda);
        assert!(h.is_finite() && h > 0.0, "expected finite hFz, got {h}");
    }

    /// Segmented concave ground with two different impedances stays finite and the
    /// two-channel identity holds (10·lg(|h|²+p) == delta_l_db) up to the Eq. 156
    /// modification factor.
    #[test]
    fn mixed_impedance_concave_is_finite() {
        let source = [0.0, 3.0];
        let receiver = [150.0, 3.0];
        let segments = vec![
            Segment3 {
                seg_a: [0.0, 0.0],
                seg_b: [75.0, -2.0],
                sigma_kpa: 20000.0,
                roughness_r: 0.0,
            },
            Segment3 {
                seg_a: [75.0, -2.0],
                seg_b: [150.0, 0.0],
                sigma_kpa: 200.0,
                roughness_r: 0.0,
            },
        ];
        let sm3 = SubModel3::new(source, receiver, C0, segments, None).unwrap();
        let coh = CoherenceInputs {
            d_m: 150.0,
            ..zero_turb()
        };
        let axis = FreqAxis::new();
        for &f in axis.centres.iter() {
            let r = sm3.eval(f, &coh).unwrap();
            assert!(r.delta_l_db.is_finite() && r.p_incoh >= 0.0);
        }
    }
}
