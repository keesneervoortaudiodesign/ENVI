//! Circular-ray variables for a refracting atmosphere (AV 1106/07 §5.5.4–5.5.7,
//! §5.23.9): DirectRay, ReflectedRay + the reflection-point cubic,
//! TravelTimeDiff, and HeightOfCircularRay.
//!
//! # Convention
//!
//! Nord2000-native (time e^{−jωt}); real-valued geometry. These functions fill
//! the SAME [`RayVars`]/[`RayPair`](crate::propagation::rays::RayPair) fields as
//! `straight_rays`; the `|ξ|<1e-6` homogeneous limit is handled by
//! [`crate::propagation::rays::circular_rays`] (it delegates to `straight_rays`
//! so the D-02 bit-for-bit anchor is structural). There is **no `.conj()`** here.
//!
//! # The two ξ clamps (RESEARCH Pitfall 1)
//!
//! [`direct_ray`] applies the *inner* `|ξ'|<1e-10 ⇒ 1e-10` division guard
//! (§5.5.4 p.29) — distinct from the `|ξ|<1e-6` homogeneous shortcut in
//! [`crate::propagation::rays::circular_rays`]. Never use 1e-10 for the
//! homogeneous shortcut.

use std::f64::consts::PI;

use crate::propagation::PropagationError;
use crate::propagation::rays::RayVars;

/// Direct-ray variables (AV 1106/07 §5.5.4 Eq. 44): travel time, arc length,
/// the ray-angle change `Δθ`, the launch/grazing angle `ψ_L`, and the
/// shadow-zone distance `dSZ` (`∞` when `ξ ≥ 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectRayVars {
    /// Travel time `τ` along the circular ray, s (Eqs. 35/40).
    pub tau: f64,
    /// Travel distance `R` (arc length) along the ray, m (Eqs. 34/39).
    pub r: f64,
    /// Change in vertical ray angle `Δθ` vs a straight line, rad (Eqs. 41–42).
    pub delta_theta: f64,
    /// Launch angle `ψ_L` at the lower point (grazing angle when the lower
    /// point is on the ground), rad (Eq. 32).
    pub psi_l: f64,
    /// Distance to a possible shadow zone `dSZ`, m; `∞` when `ξ ≥ 0` (Eq. 43).
    pub d_sz: f64,
}

/// Reject a non-finite / non-physical direct-ray input before any sqrt/division.
fn guard(d: f64, h_a: f64, h_b: f64, xi: f64, c0: f64) -> Result<(), PropagationError> {
    if !(d.is_finite() && d > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "horizontal distance d must be positive and finite",
        });
    }
    let height_ok = |h: f64| h.is_finite() && h >= 0.0;
    if !(height_ok(h_a) && height_ok(h_b)) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "source/receiver heights must be non-negative and finite",
        });
    }
    if !(c0.is_finite() && c0 > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "speed of sound c₀ must be positive and finite",
        });
    }
    if !xi.is_finite() {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "relative sound-speed gradient ξ must be finite",
        });
    }
    Ok(())
}

/// DirectRay (Eqs. 29–44): the circular-ray variables from the lower to the
/// upper of the two endpoints at heights `h_a`, `h_b`, horizontal distance `d`.
///
/// `xi` is the equivalent relative gradient `ξ`; `c0` is the ground sound speed
/// `c₀`. For `ξ < 0` the computation runs with `|ξ'|` and `Δθ` is negated
/// (Eq. 42); `dSZ` is finite only for `ξ < 0` (Eq. 43).
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] on non-finite / non-physical
/// input or a non-finite intermediate.
pub fn direct_ray(
    d: f64,
    h_a: f64,
    h_b: f64,
    xi: f64,
    c0: f64,
) -> Result<DirectRayVars, PropagationError> {
    guard(d, h_a, h_b, xi, c0)?;

    let (z_l, z_u) = if h_a <= h_b { (h_a, h_b) } else { (h_b, h_a) };
    let mut dz = z_u - z_l;
    if dz < 0.01 {
        dz = 0.01; // §5.5.4 p.29 guard (z → 0 blow-up).
    }

    // Local gradient/speed at L (Eqs. 30–31).
    let denom = 1.0 + xi * z_l;
    if !(denom.is_finite() && denom.abs() > 0.0) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "degenerate local profile (1 + ξ·z_L → 0)",
        });
    }
    let c0_l = c0 * denom; // Eq. 31
    let mut xi_l = xi / denom; // Eq. 30
    // Inner division guard |ξ'| < 1e-10 ⇒ 1e-10 (sign-preserving) — §5.5.4 p.29.
    if xi_l.abs() < 1e-10 {
        xi_l = if xi_l < 0.0 { -1e-10 } else { 1e-10 };
    }
    let xa = xi_l.abs(); // run Eqs. 32–41 with |ξ'| (Eq. 42 negates Δθ later).

    // Eq. 32: tan ψ_L. Eq. 33: dm.
    let tan_psi = xa * d / 2.0 + dz * (2.0 + xa * dz) / (2.0 * d);
    let psi = tan_psi.atan();
    let cp = psi.cos();
    let dm = tan_psi / xa;

    // Eq. 34 R(Δz) and Eqs. 35–37 τ(Δz), with the |x|≤1 clamps for asin.
    let r_of = |dzv: f64| -> f64 {
        let a = ((1.0 + xa * dzv) * cp).clamp(-1.0, 1.0);
        (a.asin() - PI / 2.0 + psi) / (xa * cp)
    };
    let tau_of = |dzv: f64| -> f64 {
        let sp = psi.sin();
        let f0 = (1.0 + sp) / (1.0 - sp);
        let rad = (1.0 - (1.0 + xa * dzv).powi(2) * cp * cp).max(0.0);
        let s = rad.sqrt();
        let fz = (1.0 + s) / (1.0 - s);
        (f0 / fz).ln() / (2.0 * xa * c0_l)
    };

    let (r, tau) = if d <= dm {
        (r_of(dz), tau_of(dz))
    } else {
        // Ray passed the circle top at dm (Eqs. 38–40).
        let dz_m = (1.0 / cp - 1.0) / xa; // Eq. 38
        (
            2.0 * r_of(dz_m) - r_of(dz),     // Eq. 39
            2.0 * tau_of(dz_m) - tau_of(dz), // Eq. 40
        )
    };

    // Eq. 41 Δθ = ψ_L − arctan(Δz/d); Eq. 42 negates it for ξ < 0.
    let mut delta_theta = psi - (dz / d).atan();
    if xi < 0.0 {
        delta_theta = -delta_theta;
    }

    // Eq. 43 dSZ (ξ < 0 only), using |ξ'|.
    let d_sz = if xi >= 0.0 {
        f64::INFINITY
    } else {
        let rl = (z_l * (2.0 / xa - z_l)).max(0.0);
        let ru = (z_u * (2.0 / xa - z_u)).max(0.0);
        rl.sqrt() + ru.sqrt()
    };

    if !(r.is_finite() && tau.is_finite()) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "non-finite circular-ray R/τ",
        });
    }
    Ok(DirectRayVars {
        tau,
        r,
        delta_theta,
        psi_l: psi,
        d_sz,
    })
}

/// Real roots of the monic cubic `x³ + a2·x² + a1·x + a0` (depressed-cubic
/// trigonometric / Cardano solver). Returns up to three real roots.
fn real_cubic_roots(a2: f64, a1: f64, a0: f64) -> Vec<f64> {
    let p = a1 - a2 * a2 / 3.0;
    let q = 2.0 * a2 * a2 * a2 / 27.0 - a2 * a1 / 3.0 + a0;
    let shift = -a2 / 3.0;
    let disc = (q / 2.0).powi(2) + (p / 3.0).powi(3);
    if disc <= 0.0 && p < 0.0 {
        // Three real roots (trigonometric form).
        let m = 2.0 * (-p / 3.0).sqrt();
        let arg = (3.0 * q / (p * m)).clamp(-1.0, 1.0);
        let theta = arg.acos() / 3.0;
        vec![
            m * theta.cos() + shift,
            m * (theta - 2.0 * PI / 3.0).cos() + shift,
            m * (theta - 4.0 * PI / 3.0).cos() + shift,
        ]
    } else {
        // One real root (Cardano; cbrt preserves sign).
        let s = disc.max(0.0).sqrt();
        let u = (-q / 2.0 + s).cbrt();
        let v = (-q / 2.0 - s).cbrt();
        vec![u + v + shift]
    }
}

/// The reflection-point distance `d_refl` from the source (AV 1106/07 §5.5.5
/// Eq. 49): the root of `2·d³ − 3d·d² + (b_R²+b_S²+d²)·d − b_S²·d = 0` in
/// `(0, d)`, selected closest to the source when `hS < hR`, else closest to the
/// receiver (up to three real roots for strong downward refraction; a single
/// root for upward refraction).
///
/// # Errors
///
/// [`PropagationError::NoReflectionRoot`] when no real root lands in `(0, d)`.
pub fn reflection_point_cubic(
    d: f64,
    h_s: f64,
    h_r: f64,
    xi: f64,
) -> Result<f64, PropagationError> {
    let b_s2 = (h_s / xi) * (2.0 + xi * h_s);
    let b_r2 = (h_r / xi) * (2.0 + xi * h_r);
    // Monic form x³ + a2 x² + a1 x + a0 (divide Eq. 49 by 2).
    let a2 = -3.0 * d / 2.0;
    let a1 = (b_r2 + b_s2 + d * d) / 2.0;
    let a0 = -b_s2 * d / 2.0;
    let roots = real_cubic_roots(a2, a1, a0);
    let mut candidates: Vec<f64> = roots
        .into_iter()
        .filter(|r| r.is_finite() && *r > 1e-9 && *r < d - 1e-9)
        .collect();
    if candidates.is_empty() {
        return Err(PropagationError::NoReflectionRoot);
    }
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Closest to source (smallest) if hS < hR, else closest to receiver.
    let chosen = if h_s < h_r {
        candidates[0]
    } else {
        candidates[candidates.len() - 1]
    };
    Ok(chosen)
}

/// ReflectedRay (Eqs. 45–50): the ground-reflected circular ray, computed by
/// running [`direct_ray`] on the two half-paths from the reflection point to
/// the source and to the receiver. Fills a [`RayVars`] (`r1 = R_S`, `r2 = R_R`,
/// `psi_g` = grazing angle).
///
/// # Errors
///
/// [`PropagationError::NoReflectionRoot`] if the cubic has no valid root;
/// [`PropagationError::DegenerateRayGeometry`] on degenerate half-path geometry.
pub fn reflected_ray(
    d: f64,
    h_s: f64,
    h_r: f64,
    xi: f64,
    c0: f64,
) -> Result<RayVars, PropagationError> {
    let d_refl = reflection_point_cubic(d, h_s, h_r, xi)?;
    // Half-paths from the reflection point (lower point on the ground, z=0).
    let src = direct_ray(d_refl, 0.0, h_s, xi, c0)?; // Eq. 45
    let rcv = direct_ray(d - d_refl, 0.0, h_r, xi, c0)?; // Eq. 46
    Ok(RayVars {
        tau: src.tau + rcv.tau,
        r: src.r + rcv.r,
        psi_g: src.psi_l, // ψ_G = ψ_L of the ground-starting half (§5.5.5).
        r1: src.r,
        r2: rcv.r,
    })
}

/// Geometry inputs to [`travel_time_diff`]'s upward-refraction shadow-edge cap
/// (Eq. 52) — grouped so the interference-`Δτ` call stays clippy-clean.
#[derive(Debug, Clone, Copy)]
pub struct TravelTimeGeometry {
    /// Horizontal source→receiver distance `d`, m.
    pub d: f64,
    /// Source height `hS`, m.
    pub h_s: f64,
    /// Receiver height `hR`, m.
    pub h_r: f64,
    /// Equivalent relative gradient `ξ`.
    pub xi: f64,
    /// Ground sound speed `C = c₀`, m/s.
    pub c0: f64,
    /// Shadow-zone distance `dSZ`, m (`∞` when `ξ ≥ 0`).
    pub d_sz: f64,
}

/// TravelTimeDiff Δτ (Eqs. 51–53) from the direct/reflected travel times, with
/// the upward-refraction shadow-edge cap Δτ₀ (Eq. 52) and Δτ = 0 past the
/// geometric shadow edge (`d > dSZ`).
#[must_use]
pub fn travel_time_diff(tau_direct: f64, tau_reflected: f64, geom: &TravelTimeGeometry) -> f64 {
    let mut dtau = tau_reflected - tau_direct; // Eq. 51
    if geom.xi < 0.0 {
        // Receiver past the geometric shadow edge ⇒ Δτ = 0 (§5.5.6). In the onset
        // window (d ≤ dSZ) the Eq. 52 cap below ramps Δτ smoothly to 0 as d → dSZ;
        // this threshold matches the reflected-ray drop in `circular_rays` so the
        // onset window is handled consistently (CR-01).
        if geom.d_sz.is_finite() && geom.d > geom.d_sz {
            return 0.0;
        }
        if geom.d_sz.is_finite() && geom.d_sz > 0.0 {
            // Eq. 52 Δτ₀ shadow-edge cap.
            let d = geom.d;
            let dtau0 = (1.0 - (d / geom.d_sz).powi(2))
                * ((d * d + (geom.h_s + geom.h_r).powi(2)).sqrt()
                    - (d * d + (geom.h_s - geom.h_r).powi(2)).sqrt())
                / geom.c0;
            if dtau > dtau0 {
                dtau = dtau0;
            }
        }
    }
    dtau
}

/// HeightOfCircularRay (AV 1106/07 §5.23.9, Eqs. 355–368): the height `h0` of a
/// circular ray (start `(0, h1)`, end `(d, h2)`, relative gradient `ξ`) at the
/// horizontal distance `d0`, plus the ray lengths `R1`/`R2` from that point to
/// the endpoints.
///
/// # Errors
///
/// [`PropagationError::DegenerateShadowZone`] on non-finite / non-physical
/// input or a non-finite intermediate.
pub fn height_of_circular_ray(
    d: f64,
    h1: f64,
    h2: f64,
    xi: f64,
    d0: f64,
) -> Result<(f64, f64, f64), PropagationError> {
    if ![d, h1, h2, xi, d0].iter().all(|v| v.is_finite()) || d <= 0.0 {
        return Err(PropagationError::DegenerateShadowZone {
            detail: "non-finite or non-positive HeightOfCircularRay input",
        });
    }
    let mut xa = xi.abs(); // Eq. 356 ξ' = |ξ|.
    if xa < 1e-10 {
        xa = 1e-10;
    }
    let dh = (h1 - h2).abs(); // Eq. 357.
    // Eqs. 358–361 normalisation.
    let (h1p, d0p) = if xi > 0.0 {
        if h1 > h2 { (h2, d - d0) } else { (h1, d0) }
    } else if h1 >= h2 {
        (-h1, d0)
    } else {
        (-h2, d - d0)
    };
    // Eq. 362 tan ψ_L; Eq. 363 x0.
    let tan_psi = xa * d / 2.0 + dh * (2.0 + xa * dh) / (2.0 * d);
    let psi = tan_psi.atan();
    let x0 = tan_psi / xa;
    let z0c = h1p - 1.0 / xa; // Eq. 364.
    let rc = 1.0 / (xa * psi.cos()); // Eq. 365.
    let theta = ((d0p - x0) / rc).clamp(-1.0, 1.0).acos(); // Eq. 366.
    let sign_xi = if xi > 0.0 {
        1.0
    } else if xi < 0.0 {
        -1.0
    } else {
        0.0
    };
    let h0 = sign_xi * (rc * theta.sin() + z0c); // Eq. 367.
    // Eq. 368 arc lengths from chord lengths r1/r2.
    let r1_chord = (d0 * d0 + (h0 - h1).powi(2)).sqrt();
    let r2_chord = ((d - d0).powi(2) + (h2 - h0).powi(2)).sqrt();
    let arc = |chord: f64| 2.0 * rc * (chord / (2.0 * rc)).clamp(-1.0, 1.0).asin();
    let r1 = arc(r1_chord);
    let r2 = arc(r2_chord);
    if ![h0, r1, r2].iter().all(|v| v.is_finite()) {
        return Err(PropagationError::DegenerateShadowZone {
            detail: "non-finite HeightOfCircularRay result",
        });
    }
    Ok((h0, r1, r2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::sound_speed_ms;

    const C0: f64 = 340.348;

    // DirectRay reduces to ~the straight distance for a weak gradient.
    #[test]
    fn direct_ray_near_straight_for_weak_gradient() {
        let dr = direct_ray(97.5, 0.5, 1.5, 1e-3, C0).unwrap();
        let straight = (97.5f64 * 97.5 + 1.0).sqrt();
        assert!(
            (dr.r - straight).abs() < 0.5,
            "arc {} vs straight {straight}",
            dr.r
        );
        assert!(dr.tau.is_finite() && dr.tau > 0.0);
    }

    // Downward refraction: a positive gradient bends the ray; dSZ = ∞.
    #[test]
    fn downward_has_infinite_dsz() {
        let dr = direct_ray(200.0, 0.5, 3.0, 2e-3, C0).unwrap();
        assert!(dr.d_sz.is_infinite());
    }

    // Upward refraction: dSZ is finite and positive.
    #[test]
    fn upward_has_finite_dsz() {
        let dr = direct_ray(200.0, 0.5, 3.0, -2e-3, C0).unwrap();
        assert!(dr.d_sz.is_finite() && dr.d_sz > 0.0, "dSZ = {}", dr.d_sz);
    }

    // The reflection point lies strictly inside (0, d).
    #[test]
    fn reflection_point_in_interval() {
        let d = 97.5;
        let d_refl = reflection_point_cubic(d, 0.5, 1.5, 2e-3).unwrap();
        assert!(d_refl > 0.0 && d_refl < d, "d_refl = {d_refl}");
    }

    // Cubic degenerate → typed error, never a panic.
    #[test]
    fn cubic_no_root_is_typed_error() {
        // Zero distance leaves no interior root.
        assert!(matches!(
            reflection_point_cubic(0.0, 0.5, 1.5, 2e-3),
            Err(PropagationError::NoReflectionRoot)
        ));
    }

    // HeightOfCircularRay returns a finite height + arc lengths.
    #[test]
    fn height_of_circular_ray_is_finite() {
        let (h0, r1, r2) = height_of_circular_ray(200.0, 0.5, 1.5, -2e-3, 100.0).unwrap();
        assert!(h0.is_finite() && r1.is_finite() && r2.is_finite());
        assert!(r1 > 0.0 && r2 > 0.0);
    }

    #[test]
    fn degenerate_direct_ray_is_typed_error() {
        assert!(matches!(
            direct_ray(0.0, 0.5, 1.5, 1e-3, sound_speed_ms(15.0)),
            Err(PropagationError::DegenerateRayGeometry { .. })
        ));
    }
}
