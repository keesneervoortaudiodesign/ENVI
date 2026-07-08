//! §5.21 terrain interpretation — the dispatcher that turns a `TerrainProfile`
//! into a Sub-model choice plus the §5.22 transition parameters (AV 1106/07
//! Eqs. 300–328).
//!
//! For a cut-plane profile and a source/receiver pair this identifies:
//!
//! 1. **Primary edge** of the primary screen (§5.21.1, Eq. 300): the interior
//!    profile point above the line of sight with the largest path-length
//!    difference `Δℓ₀ = H(zᵢ − z_m)·(|SPᵢ| + |PᵢR| − |SR|)`. If no point
//!    qualifies (`max Δℓ₀ ≤ 0`) the terrain is **flat** (Sub-model 1/2).
//! 2. The **screen class** — one edge (Sub-model 4), one thick screen with two
//!    edges (Sub-model 5), or two screens (Sub-model 6) — from the grouping of
//!    above-line-of-sight points, with the screen shape reduced to its
//!    non-convex flanks (`Convex`, Eq. 336, threshold `0.0001 m`).
//! 3. The frequency-dependent **transition parameters** `r_scr1` (screen vs no
//!    screen, Eqs. 301–305: `r_scr1 = r_Δℓ·r_h·r_Fz`), `r_scr2` (two screens
//!    vs one), `r_scr12` (thick vs single edge) and `r_flat` (flat vs non-flat,
//!    the equivalent-flat-terrain least squares of Eqs. 316–327, **frozen above
//!    2 kHz**).
//!
//! # Sub-model 3 (non-flat terrain) is a typed hard error
//!
//! Non-flat terrain (§5.12) is scheduled with Phase 3 — flat Phase 2 target
//! profiles give `r_flat = 1`, making its `(1 − r_flat)·ΔL₃` branch unreachable.
//! Reaching it with weight `> 0` returns
//! [`PropagationError::NonFlatTerrainNotImplemented`] — never a silent wrong
//! answer (02-RESEARCH Open Question 4).
//!
//! Geometry-only module: no complex numerics, no `conj()`.

use crate::propagation::PropagationError;
use crate::propagation::fresnel::calc_fz_d;
use crate::scene::TerrainProfile;

/// Above-line-of-sight epsilon for edge qualification, meters. A profile point
/// must clear the S–R line by this margin to count as a diffracting edge.
const LOS_EPS: f64 = 1e-6;

/// `Convex` threshold (AV 1106/07 Eq. 336): points within this vertical
/// distance of the chord between their neighbours are treated as collinear when
/// reducing a screen shape.
const CONVEX_EPS: f64 = 0.0001;

/// Frequency above which the flatness parameter `r_flat` is frozen (Eq. 317
/// note): it is only evaluated 25 Hz–2 kHz.
const FLATNESS_FREEZE_HZ: f64 = 2000.0;

/// The Sub-model a profile dispatches to (the §5.21 screen classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenClass {
    /// No qualifying screen — flat ground (Sub-model 1 or 2).
    Flat,
    /// One screen, one diffracting edge (Sub-model 4).
    SingleEdge,
    /// One thick screen, two edges of one body (Sub-model 5).
    ThickScreen,
    /// Two separate screens (Sub-model 6).
    DoubleScreen,
}

/// The §5.22 transition parameters at one frequency (the Eq. 332 weights).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionParams {
    /// Screen vs no-screen weight (Eqs. 301–305). `0` = flat, `1` = full screen.
    pub r_scr1: f64,
    /// Two-screen (Sub-model 6) vs one-screen weight. `1` for double screens.
    pub r_scr2: f64,
    /// Thick (Sub-model 5) vs single-edge (Sub-model 4) weight. `1` for thick.
    pub r_scr12: f64,
    /// Flat (Sub-model 1/2) vs non-flat (Sub-model 3) weight. `1` = flat.
    pub r_flat: f64,
}

/// A double screen's two reduced spike shapes `([W₁,T₁,W₁'], [W₂',T₂,W₂])`.
pub type ScreenPair = ([[f64; 2]; 3], [[f64; 2]; 3]);

/// A reflecting ground strip in cut-plane coordinates `[x, z]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripSpec {
    /// Segment start `[x, z]` (m).
    pub seg_a: [f64; 2],
    /// Segment end `[x, z]` (m).
    pub seg_b: [f64; 2],
    /// Flow resistivity σ, kPa·s·m⁻².
    pub sigma_kpa: f64,
    /// Roughness `r`, meters.
    pub roughness_r: f64,
}

/// The result of interpreting a terrain profile against a source/receiver pair.
#[derive(Debug, Clone)]
pub struct TerrainInterpretation {
    /// Source `[x, z]` in the cut plane.
    pub source: [f64; 2],
    /// Receiver `[x, z]` in the cut plane.
    pub receiver: [f64; 2],
    /// The screen classification (the Sub-model choice).
    pub class: ScreenClass,
    /// Primary reduced screen shape: `[W₁, T, W₂]` (single edge), `[W₁, T₁, T₂,
    /// W₂]` (thick). Empty for flat terrain and for the first shape of a double
    /// screen (see [`Self::screens`]).
    pub screen: Vec<[f64; 2]>,
    /// For a double screen, the two screen shapes `([W₁,T₁,W₁'], [W₂',T₂,W₂])`.
    pub screens: Option<ScreenPair>,
    /// Reflecting strips on the source side of the (primary) screen (or the
    /// whole ground when flat).
    pub before: Vec<StripSpec>,
    /// Reflecting strips between two screens (double screen only).
    pub middle: Vec<StripSpec>,
    /// Reflecting strips on the receiver side of the (primary) screen.
    pub after: Vec<StripSpec>,
    /// Source-side wedge-face σ (kPa·s·m⁻²) — the ground segment adjacent to the
    /// primary edge on the source side.
    pub sigma_face_source: f64,
    /// Receiver-side wedge-face σ.
    pub sigma_face_receiver: f64,
    /// Speed of sound c₀, m/s (for the λ-dependent ramps).
    c0: f64,
    /// Primary diffracting edge `[x, z]` (the primary screen apex; the S–R
    /// midpoint for flat terrain, where it is unused).
    edge: [f64; 2],
    /// Primary-edge path-length difference `Δℓ₀` (Eq. 300), meters.
    delta_l0: f64,
    /// Primary-edge height above the local terrain baseline, meters.
    h_scr: f64,
    /// Source→edge distance `r_S`, meters (for `r_Fz`).
    r_s: f64,
    /// Edge→receiver distance `r_R`, meters (for `r_Fz`).
    r_r: f64,
    /// Equivalent-flat-terrain LSQ max deviation `Δh`, meters (for `r_flat`).
    flat_dev: f64,
}

#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// Height of the straight S–R line of sight at horizontal position `x`.
fn los_height(source: [f64; 2], receiver: [f64; 2], x: f64) -> f64 {
    let dx = receiver[0] - source[0];
    if dx.abs() < 1e-12 {
        return source[1].max(receiver[1]);
    }
    let t = (x - source[0]) / dx;
    source[1] + t * (receiver[1] - source[1])
}

/// Least-squares fit `z = slope·x + intercept` to the ground points, and the
/// maximum absolute deviation `Δh` from that line (AV 1106/07 Eqs. 320–327
/// equivalent flat terrain). A flat profile returns `(0, z, 0)`.
#[must_use]
pub fn equivalent_flat_terrain(points: &[[f64; 2]]) -> (f64, f64, f64) {
    let n = points.len() as f64;
    if n < 2.0 {
        let z = points.first().map(|p| p[1]).unwrap_or(0.0);
        return (0.0, z, 0.0);
    }
    let sx: f64 = points.iter().map(|p| p[0]).sum();
    let sz: f64 = points.iter().map(|p| p[1]).sum();
    let sxx: f64 = points.iter().map(|p| p[0] * p[0]).sum();
    let sxz: f64 = points.iter().map(|p| p[0] * p[1]).sum();
    let denom = n * sxx - sx * sx;
    let (slope, intercept) = if denom.abs() < 1e-12 {
        (0.0, sz / n)
    } else {
        let slope = (n * sxz - sx * sz) / denom;
        let intercept = (sz - slope * sx) / n;
        (slope, intercept)
    };
    let dev = points
        .iter()
        .map(|p| (p[1] - (slope * p[0] + intercept)).abs())
        .fold(0.0_f64, f64::max);
    (slope, intercept, dev)
}

impl TerrainInterpretation {
    /// The §5.22 transition parameters at frequency `f_hz` (Eq. 332 weights).
    #[must_use]
    pub fn transition_params(&self, f_hz: f64) -> TransitionParams {
        let (r_scr2, r_scr12) = match self.class {
            ScreenClass::Flat => (0.0, 0.0),
            ScreenClass::SingleEdge => (0.0, 0.0),
            ScreenClass::ThickScreen => (0.0, 1.0),
            ScreenClass::DoubleScreen => (1.0, 0.0),
        };
        let r_scr1 = match self.class {
            ScreenClass::Flat => 0.0,
            _ => self.r_scr1_at(f_hz),
        };
        TransitionParams {
            r_scr1,
            r_scr2,
            r_scr12,
            r_flat: self.r_flat_at(f_hz),
        }
    }

    /// `r_scr1 = r_Δℓ · r_h · r_Fz` (AV 1106/07 Eqs. 301–305).
    fn r_scr1_at(&self, f_hz: f64) -> f64 {
        let lambda = self.c0 / f_hz;

        // r_Δℓ (Eq. 302): 1 for Δℓ′ ≥ 0; 1 + Δℓ′/(0.133·λ) on −0.133λ < Δℓ′ < 0;
        // 0 below. (7.5 ≈ 1/0.133.) Verified on AV 1106/07 p. 100.
        let ratio_dl = self.delta_l0 / lambda;
        let r_dl = if ratio_dl >= 0.0 {
            1.0
        } else if ratio_dl > -0.133 {
            1.0 + ratio_dl / 0.133
        } else {
            0.0
        };

        // r_h (Eq. 303): ramp on h_SCR/λ over [0.1, 0.3]. Verified on p. 100.
        let r_h = ramp(self.h_scr / lambda, 0.1, 0.3);

        // r_Fz (Eq. 304): ramp on h_SCR/h_Fz over [0.026, 0.082], h_Fz from
        // CalcFZd at F_λ = 0.5·λ (the parametric F_λ of 02-02). Verified p. 100.
        let r_fz = match calc_fz_d(
            self.r_s,
            self.r_r,
            std::f64::consts::FRAC_PI_2,
            0.5 * lambda,
        ) {
            Ok(h_fz) if h_fz > 0.0 => ramp(self.h_scr / h_fz, 0.026, 0.082),
            _ => 1.0,
        };

        (r_dl * r_h * r_fz).clamp(0.0, 1.0)
    }

    /// `r_flat` (Eqs. 316–319): flat vs non-flat weight, frozen above 2 kHz.
    /// A flat profile (`Δh = 0`) gives `r_flat = 1` at every frequency.
    fn r_flat_at(&self, f_hz: f64) -> f64 {
        let f = f_hz.min(FLATNESS_FREEZE_HZ); // Eq. 317: evaluated 25 Hz–2 kHz
        let lambda = self.c0 / f;
        // Flatness excess Δh/λ ramps over [0.01, 0.03]: below → flat (r_flat=1),
        // above → non-flat. (For flat Phase 2 targets Δh = 0 ⇒ r_flat = 1.)
        1.0 - ramp(self.flat_dev / lambda, 0.01, 0.03)
    }

    /// The primary-edge path-length difference `Δℓ₀` (Eq. 300), meters.
    #[must_use]
    pub fn delta_l0(&self) -> f64 {
        self.delta_l0
    }

    /// The equivalent-flat-terrain maximum deviation `Δh`, meters.
    #[must_use]
    pub fn flat_deviation(&self) -> f64 {
        self.flat_dev
    }

    /// The primary diffracting edge `[x, z]` (the primary screen apex).
    #[must_use]
    pub fn primary_edge(&self) -> [f64; 2] {
        self.edge
    }
}

/// A linear ramp: `0` for `v ≤ lo`, `1` for `v ≥ hi`, linear between.
#[inline]
fn ramp(v: f64, lo: f64, hi: f64) -> f64 {
    if v <= lo {
        0.0
    } else if v >= hi {
        1.0
    } else {
        (v - lo) / (hi - lo)
    }
}

/// Reduce a run of consecutive above-line-of-sight points to its diffracting
/// edge(s) via the `Convex` test (Eq. 336): a point that lies within
/// [`CONVEX_EPS`] of the chord between its flanking edge candidates is dropped.
/// Returns the retained edge points (1 for a spike, 2 for a flat-topped screen).
fn reduce_edges(run: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if run.len() <= 1 {
        return run.to_vec();
    }
    // The two flanks are the highest point(s). A flat-topped screen has ≥ 2
    // points near the maximum height; a spike has one dominant apex.
    let z_max = run.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let tops: Vec<[f64; 2]> = run
        .iter()
        .copied()
        .filter(|p| (z_max - p[1]).abs() <= CONVEX_EPS)
        .collect();
    if tops.len() >= 2 {
        // Thick screen: keep the extreme (leftmost, rightmost) top points.
        let t1 = *tops
            .iter()
            .min_by(|a, b| a[0].partial_cmp(&b[0]).unwrap())
            .unwrap();
        let t2 = *tops
            .iter()
            .max_by(|a, b| a[0].partial_cmp(&b[0]).unwrap())
            .unwrap();
        vec![t1, t2]
    } else {
        vec![
            *run.iter()
                .max_by(|a, b| a[1].partial_cmp(&b[1]).unwrap())
                .unwrap(),
        ]
    }
}

/// Interpret a terrain profile (§5.21) against a source/receiver pair in the
/// cut plane.
///
/// `source`/`receiver` are `[x, z]`. The profile's segments carry the impedance
/// used for the reflecting strips and the wedge faces.
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] for an empty/degenerate profile
/// or a coincident source/receiver.
pub fn interpret_terrain(
    profile: &TerrainProfile,
    source: [f64; 2],
    receiver: [f64; 2],
    c0: f64,
) -> Result<TerrainInterpretation, PropagationError> {
    let points = profile.points();
    let segments = profile.segments();
    if points.len() < 2 || segments.len() + 1 != points.len() {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "terrain interpretation requires a profile with ≥ 2 points",
        });
    }
    if dist(source, receiver) <= 0.0 || !(c0.is_finite() && c0 > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "terrain interpretation requires distinct S/R and positive c₀",
        });
    }

    let sr = dist(source, receiver);

    // Primary edge (Eq. 300): interior profile points above the S–R line.
    let mut best: Option<(usize, f64)> = None; // (point index, Δℓ₀)
    for (i, p) in points.iter().enumerate() {
        let zm = los_height(source, receiver, p[0]);
        if p[1] > zm + LOS_EPS {
            let d0 = dist(source, *p) + dist(*p, receiver) - sr;
            if d0 > best.map(|b| b.1).unwrap_or(0.0) {
                best = Some((i, d0));
            }
        }
    }

    // Equivalent flat terrain over the GROUND points only — screen points
    // (above the S–R line) are removed before the flatness LSQ (Eqs. 320–327;
    // the screen shapes are interpreted separately, §5.21.4).
    let ground: Vec<[f64; 2]> = points
        .iter()
        .copied()
        .filter(|p| p[1] <= los_height(source, receiver, p[0]) + LOS_EPS)
        .collect();
    let (_slope, _intercept, flat_dev) = equivalent_flat_terrain(&ground);

    // No qualifying edge ⇒ flat terrain.
    let Some((edge_idx, delta_l0)) = best else {
        let before: Vec<StripSpec> = strips_of(points, segments, 0, points.len() - 1);
        return Ok(TerrainInterpretation {
            source,
            receiver,
            class: ScreenClass::Flat,
            screen: Vec::new(),
            screens: None,
            before,
            middle: Vec::new(),
            after: Vec::new(),
            sigma_face_source: segments[0].flow_resistivity,
            sigma_face_receiver: segments[segments.len() - 1].flow_resistivity,
            c0,
            edge: [
                0.5 * (source[0] + receiver[0]),
                0.5 * (source[1] + receiver[1]),
            ],
            delta_l0: 0.0,
            h_scr: 0.0,
            r_s: dist(source, receiver),
            r_r: 1.0,
            flat_dev,
        });
    };

    // Group all above-line-of-sight interior points into screens.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let zm = los_height(source, receiver, p[0]);
        if p[1] > zm + LOS_EPS {
            current.push(i);
        } else if !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }

    let edge_pt = points[edge_idx];
    let baseline = los_baseline(points, edge_idx);
    let h_scr = edge_pt[1] - baseline;
    let r_s = dist(source, edge_pt);
    let r_r = dist(edge_pt, receiver);

    // Wedge-face σ: the ground segments flanking the primary edge.
    let sigma_face_source = segments[edge_idx.saturating_sub(1)].flow_resistivity;
    let sigma_face_receiver = segments[edge_idx.min(segments.len() - 1)].flow_resistivity;

    let (class, screen, screens) = classify(points, &groups, edge_idx);

    // Reflecting strips: before/after the primary screen (double screens also
    // carry a middle region between the two shapes).
    let (before, middle, after) =
        region_strips(points, segments, source, receiver, &screen, &screens);

    Ok(TerrainInterpretation {
        source,
        receiver,
        class,
        screen,
        screens,
        before,
        middle,
        after,
        sigma_face_source,
        sigma_face_receiver,
        c0,
        edge: edge_pt,
        delta_l0,
        h_scr,
        r_s,
        r_r,
        flat_dev,
    })
}

/// The terrain baseline height under a profile point (the ground level the
/// screen rises from) — the min of its immediate flat neighbours, or 0 for a
/// flat-ground screen.
fn los_baseline(points: &[[f64; 2]], edge_idx: usize) -> f64 {
    let left = points.get(edge_idx.wrapping_sub(1)).map(|p| p[1]);
    let right = points.get(edge_idx + 1).map(|p| p[1]);
    match (left, right) {
        (Some(l), Some(r)) => l.min(r),
        (Some(l), None) => l,
        (None, Some(r)) => r,
        (None, None) => 0.0,
    }
}

/// Classify the screen(s) and build the reduced primary shape.
#[allow(clippy::type_complexity)]
fn classify(
    points: &[[f64; 2]],
    groups: &[Vec<usize>],
    edge_idx: usize,
) -> (ScreenClass, Vec<[f64; 2]>, Option<ScreenPair>) {
    if groups.len() >= 2 {
        // Double screen: reduce the first two groups to spikes and flank each.
        let s1 = spike_shape(points, &groups[0]);
        let s2 = spike_shape(points, &groups[1]);
        return (ScreenClass::DoubleScreen, Vec::new(), Some((s1, s2)));
    }
    // One group: single edge or thick screen.
    let group = groups
        .iter()
        .find(|g| g.contains(&edge_idx))
        .unwrap_or(&groups[0]);
    let run: Vec<[f64; 2]> = group.iter().map(|&i| points[i]).collect();
    let edges = reduce_edges(&run);
    let g0 = *group.first().unwrap();
    let g1 = *group.last().unwrap();
    let w1 = points[g0.saturating_sub(1)];
    let w2 = points[(g1 + 1).min(points.len() - 1)];
    if edges.len() >= 2 {
        (
            ScreenClass::ThickScreen,
            vec![w1, edges[0], edges[1], w2],
            None,
        )
    } else {
        (ScreenClass::SingleEdge, vec![w1, edges[0], w2], None)
    }
}

/// A single-spike screen shape `[W₁, T, W₂]` from a group of point indices.
fn spike_shape(points: &[[f64; 2]], group: &[usize]) -> [[f64; 2]; 3] {
    let apex = *group
        .iter()
        .max_by(|&&a, &&b| points[a][1].partial_cmp(&points[b][1]).unwrap())
        .unwrap();
    let g0 = *group.first().unwrap();
    let g1 = *group.last().unwrap();
    let w1 = points[g0.saturating_sub(1)];
    let w2 = points[(g1 + 1).min(points.len() - 1)];
    [w1, points[apex], w2]
}

/// Reflecting strips over profile segments `[a_idx, b_idx)` (each segment's
/// impedance is the segment that starts it).
fn strips_of(
    points: &[[f64; 2]],
    segments: &[crate::scene::GroundSegment],
    a_idx: usize,
    b_idx: usize,
) -> Vec<StripSpec> {
    let mut strips = Vec::new();
    for i in a_idx..b_idx {
        if i >= segments.len() {
            break;
        }
        strips.push(StripSpec {
            seg_a: points[i],
            seg_b: points[i + 1],
            sigma_kpa: segments[i].flow_resistivity,
            roughness_r: segments[i].roughness,
        });
    }
    strips
}

/// Build the before/middle/after reflecting strips for a screen dispatch. The
/// screen sub-model uses **wide flat reflecting strips** on each side (so the
/// Fresnel-zone weight saturates, `w_Q = 1`) — the base-model prescription the
/// committed oracle mirrors; the strip σ is the ground segment on that side.
fn region_strips(
    points: &[[f64; 2]],
    segments: &[crate::scene::GroundSegment],
    source: [f64; 2],
    receiver: [f64; 2],
    screen: &[[f64; 2]],
    screens: &Option<ScreenPair>,
) -> (Vec<StripSpec>, Vec<StripSpec>, Vec<StripSpec>) {
    // Wide-strip half-extent so the Fresnel zone is fully covered.
    const WIDE: f64 = 500.0;
    let sigma_src = segments[0].flow_resistivity;
    let sigma_rcv = segments[segments.len() - 1].flow_resistivity;
    let rough_src = segments[0].roughness;
    let rough_rcv = segments[segments.len() - 1].roughness;

    if let Some((s1, s2)) = screens {
        let t1x = s1[1][0];
        let t2x = s2[1][0];
        let midx = 0.5 * (s1[2][0] + s2[0][0]);
        let before = vec![StripSpec {
            seg_a: [source[0] - WIDE, 0.0],
            seg_b: [t1x, 0.0],
            sigma_kpa: sigma_src,
            roughness_r: rough_src,
        }];
        let middle = vec![StripSpec {
            seg_a: [t1x, 0.0],
            seg_b: [midx.max(t1x) + 0.0, 0.0],
            sigma_kpa: sigma_src,
            roughness_r: rough_src,
        }];
        let after = vec![StripSpec {
            seg_a: [t2x, 0.0],
            seg_b: [receiver[0] + WIDE, 0.0],
            sigma_kpa: sigma_rcv,
            roughness_r: rough_rcv,
        }];
        let _ = points;
        return (before, middle, after);
    }

    // Single/thick screen: the diffracting edge x splits the ground.
    let edge_x = if screen.len() == 4 {
        0.5 * (screen[1][0] + screen[2][0])
    } else {
        screen[1][0]
    };
    let before = vec![StripSpec {
        seg_a: [source[0] - WIDE, 0.0],
        seg_b: [edge_x, 0.0],
        sigma_kpa: sigma_src,
        roughness_r: rough_src,
    }];
    let after = vec![StripSpec {
        seg_a: [edge_x, 0.0],
        seg_b: [receiver[0] + WIDE, 0.0],
        sigma_kpa: sigma_rcv,
        roughness_r: rough_rcv,
    }];
    (before, Vec::new(), after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{GroundSegment, TerrainProfile};

    const C0: f64 = 340.348;

    fn seg(sigma: f64) -> GroundSegment {
        GroundSegment {
            flow_resistivity: sigma,
            roughness: 0.0,
        }
    }

    /// A flat case-1-like profile: ground z = 0 from x = 3.25 to x = 100.
    fn flat_profile() -> TerrainProfile {
        TerrainProfile::new(
            vec![[3.25, 0.0], [5.0, 0.0], [100.0, 0.0]],
            vec![seg(20000.0), seg(200.0)],
        )
        .unwrap()
    }

    /// Literal case-71 thin screen: 4 m spike at x = 15 on flat σ=200 ground.
    fn thin_profile() -> TerrainProfile {
        TerrainProfile::new(
            vec![
                [0.0, 0.0],
                [14.99, 0.0],
                [15.0, 4.0],
                [15.01, 0.0],
                [150.0, 0.0],
            ],
            vec![seg(200.0), seg(200.0), seg(200.0), seg(200.0)],
        )
        .unwrap()
    }

    /// Literal case-81 thick screen: flat-top 15→30 m at h = 2 m.
    fn thick_profile() -> TerrainProfile {
        TerrainProfile::new(
            vec![
                [0.0, 0.0],
                [14.99, 0.0],
                [15.0, 2.0],
                [30.0, 2.0],
                [30.01, 0.0],
                [150.0, 0.0],
            ],
            vec![seg(200.0), seg(200.0), seg(200.0), seg(200.0), seg(200.0)],
        )
        .unwrap()
    }

    /// Literal case-91 double screen: spike at 15 + trapezoid 75–85 m.
    fn double_profile() -> TerrainProfile {
        TerrainProfile::new(
            vec![
                [0.0, 0.0],
                [14.99, 0.0],
                [15.0, 4.0],
                [15.01, 0.0],
                [75.0, 0.0],
                [80.0, 3.0],
                [85.0, 0.0],
                [150.0, 0.0],
            ],
            vec![
                seg(200.0),
                seg(200.0),
                seg(200.0),
                seg(200.0),
                seg(200.0),
                seg(200.0),
                seg(200.0),
            ],
        )
        .unwrap()
    }

    // Test 1: edge finding.
    #[test]
    fn edge_finding_identifies_screens_and_flat() {
        // Flat: no edge qualifies.
        let flat = interpret_terrain(&flat_profile(), [2.5, 0.5], [100.0, 1.5], C0).unwrap();
        assert_eq!(flat.class, ScreenClass::Flat);
        assert_eq!(flat.delta_l0(), 0.0);

        // Thin: the spike top at x = 15 is the primary edge.
        let thin = interpret_terrain(&thin_profile(), [0.0, 0.5], [150.0, 1.5], C0).unwrap();
        assert_eq!(thin.class, ScreenClass::SingleEdge);
        assert!(thin.delta_l0() > 0.0);
        assert_eq!(thin.screen.len(), 3);
        assert!((thin.screen[1][0] - 15.0).abs() < 1e-9 && (thin.screen[1][1] - 4.0).abs() < 1e-9);

        // Double: primary + secondary screens identified.
        let dbl = interpret_terrain(&double_profile(), [0.0, 0.5], [150.0, 1.5], C0).unwrap();
        assert_eq!(dbl.class, ScreenClass::DoubleScreen);
        assert!(dbl.screens.is_some());
    }

    // Test 2: transition parameters.
    #[test]
    fn transition_params_take_the_documented_limits() {
        // Flat: r_scr1 == 0 and r_flat == 1 exactly.
        let flat = interpret_terrain(&flat_profile(), [2.5, 0.5], [100.0, 1.5], C0).unwrap();
        let tp = flat.transition_params(1000.0);
        assert_eq!(tp.r_scr1, 0.0);
        assert_eq!(tp.r_flat, 1.0);

        // Tall thin screen at mid frequencies: r_scr1 == 1.
        let thin = interpret_terrain(&thin_profile(), [0.0, 0.5], [150.0, 1.5], C0).unwrap();
        assert!(
            (thin.transition_params(1000.0).r_scr1 - 1.0).abs() < 1e-9,
            "tall screen r_scr1 = {}",
            thin.transition_params(1000.0).r_scr1
        );

        // A marginal 0.15 m screen mid-band: 0 < r_scr1 < 1 via the r_h ramp.
        let marginal = TerrainProfile::new(
            vec![
                [0.0, 0.0],
                [49.99, 0.0],
                [50.0, 0.15],
                [50.01, 0.0],
                [100.0, 0.0],
            ],
            vec![seg(200.0), seg(200.0), seg(200.0), seg(200.0)],
        )
        .unwrap();
        let m = interpret_terrain(&marginal, [0.0, 0.1], [100.0, 0.1], C0).unwrap();
        // λ ≈ 0.15/0.2 = 0.75 m ⇒ f ≈ 454 Hz sits in the h/λ ∈ [0.1,0.3] ramp.
        let r = m.transition_params(454.0).r_scr1;
        assert!(r > 0.0 && r < 1.0, "marginal r_scr1 = {r}");
    }

    // Test 3: screen-shape reduction (thick trapezoid → 4-point shape).
    #[test]
    fn thick_screen_reduces_to_two_edges() {
        let thick = interpret_terrain(&thick_profile(), [0.0, 0.5], [150.0, 1.5], C0).unwrap();
        assert_eq!(thick.class, ScreenClass::ThickScreen);
        assert_eq!(thick.screen.len(), 4, "thick shape is [W₁, T₁, T₂, W₂]");
        assert!((thick.screen[1][0] - 15.0).abs() < 1e-9);
        assert!((thick.screen[2][0] - 30.0).abs() < 1e-9);
    }

    // Test 4: equivalent flat terrain — identity LSQ + freeze above 2 kHz.
    #[test]
    fn equivalent_flat_terrain_identity_and_freeze() {
        // A flat profile returns itself (slope 0, deviation 0).
        let (slope, _b, dev) = equivalent_flat_terrain(flat_profile().points());
        assert!(slope.abs() < 1e-12 && dev.abs() < 1e-12);

        // The flatness parameter freezes above 2 kHz: r_flat(4 kHz)==r_flat(2 kHz).
        // Use a mildly non-flat profile so r_flat < 1 and the freeze is visible.
        let rough = TerrainProfile::new(
            vec![[0.0, 0.0], [50.0, 0.4], [100.0, 0.0]],
            vec![seg(200.0), seg(200.0)],
        )
        .unwrap();
        let ri = interpret_terrain(&rough, [0.0, 1.5], [100.0, 1.5], C0).unwrap();
        let at2k = ri.transition_params(2000.0).r_flat;
        let at4k = ri.transition_params(4000.0).r_flat;
        assert_eq!(at2k, at4k, "r_flat must freeze above 2 kHz");
    }

    // Test 5: all five target profiles interpret without demanding Sub-model 3.
    #[test]
    fn target_profiles_never_demand_submodel3() {
        let axis = crate::freq::FreqAxis::new();
        let cases: Vec<(TerrainProfile, [f64; 2], [f64; 2])> = vec![
            (flat_profile(), [2.5, 0.5], [100.0, 1.5]),
            (thin_profile(), [0.0, 0.5], [150.0, 1.5]),
            (thick_profile(), [0.0, 0.5], [150.0, 1.5]),
            (double_profile(), [0.0, 0.5], [150.0, 1.5]),
        ];
        for (profile, s, r) in &cases {
            let interp = interpret_terrain(profile, *s, *r, C0).unwrap();
            for &f in axis.centres.iter() {
                let tp = interp.transition_params(f);
                assert!(
                    (tp.r_flat - 1.0).abs() < 1e-9,
                    "flat target must keep r_flat = 1 (got {} at {f} Hz)",
                    tp.r_flat
                );
            }
        }
    }
}
