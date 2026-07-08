//! Sub-model 8 — multiple ground reflections (AV 1106/07 §5.17, Eqs. 278–283).
//!
//! Under **strong downward refraction** over **long distances**, rays can reflect
//! off the ground more than once before reaching the receiver — extra arrivals
//! beyond the fixed ray sets of Sub-models 1–6. Sub-model 8 adds their (energy-
//! only) contribution. The document's trigger (Eq. 279) is the approximate ray
//! count `N`; the additional-ray contribution is **ignored for `N < 4`** and
//! computed (Eq. 283) only for `N ≥ 4`.
//!
//! # Decision: accepted gap (Assumption A6), evidence-driven
//!
//! This module provides ONLY the Eq. 279 ray-count trigger [`ray_count_eq279`].
//! The additional-ray energy sum (Eqs. 280–283, plus the `RayCurvature` Eq. 284
//! B-profile reduction) is **not** implemented. The decision is driven by
//! evaluating Eq. 279 over the FORCE straight-road downwind geometries (see the
//! test below): only the single long-range case (d = 1000 m, u = 5 m/s downwind)
//! reaches `N ≥ 4`; every d ≤ 100–200 m downwind case stays `N < 4`, so
//! Sub-models 1–6 already carry all significant rays there. The one long-range
//! case additionally gates on the [ASSUMED] road-emission coefficients, so it
//! stays capability-gated (`multiple-ground-reflections`) regardless. Wiring the
//! full energy sum is therefore an explicit accepted gap for a later plan rather
//! than a VAL-02 straight-road blocker (honest-green — never a false pass).
//!
//! Energy-only by construction (returns `f64` dB, like [`super::submodel7`]) —
//! structurally incapable of touching the coherent phase channel.

/// The approximate multiple-ground-reflection ray count `N` (AV 1106/07 Eq. 279,
/// a decimal number):
///
/// ```text
/// N = (4·d / h_max)·√(A / (2π·C)),   h_max = Max(hS, hR)
/// ```
///
/// `d` is the horizontal propagation distance (m), `h_s`/`h_r` the source/
/// receiver heights (m), `a` the logarithmic weather coefficient `A` (m/s;
/// Eq. 278 — for a profile with a linear part B, the `RayCurvature` Eq. 284
/// equivalent `A'` is used), and `c` the ground sound speed `C` (m/s). The
/// document ignores the additional-ray contribution for `N < 4`.
///
/// Returns `0` for a non-downward (`A ≤ 0`) or degenerate profile — multiple
/// ground reflections are a downward-refraction (`A > 0`) phenomenon only.
#[must_use]
pub fn ray_count_eq279(d: f64, h_s: f64, h_r: f64, a: f64, c: f64) -> f64 {
    let h_max = h_s.max(h_r);
    let all_positive_finite = d.is_finite()
        && d > 0.0
        && h_max.is_finite()
        && h_max > 0.0
        && a.is_finite()
        && a > 0.0
        && c.is_finite()
        && c > 0.0;
    if !all_positive_finite {
        return 0.0;
    }
    (4.0 * d / h_max) * (a / (2.0 * std::f64::consts::PI * c)).sqrt()
}

/// The document threshold below which Sub-model 8's additional rays are ignored
/// (AV 1106/07 §5.17: "For a value of N less than 4 the contribution from
/// additional rays are ignored").
pub const SM8_RAY_COUNT_THRESHOLD: f64 = 4.0;

#[cfg(test)]
mod tests {
    use super::*;

    const C0: f64 = 340.348;

    /// Eq. 279 over the FORCE straight-road downwind envelope drives the SM8
    /// implement-or-accept-gap decision (Assumption A6). Representative
    /// logarithmic coefficients `A` from the Nord2000 wind route
    /// (A ≈ u*/κ·cos φ; u* = κ·u/ln(zu/z0)): u = 3 m/s ⇒ A ≈ 0.43 m/s,
    /// u = 5 m/s ⇒ A ≈ 0.72 m/s (zu = 10 m, z0 = 0.01 m).
    #[test]
    fn eq279_over_force_downwind_geometries_records_the_sm8_decision() {
        // (label, d, h_s, h_r, A) — the downwind FORCE straight-road cases.
        let cases = [
            ("d=100 u=3 hr=1.5", 100.0, 0.01, 1.5, 0.43),
            ("d=100 u=3 hr=4", 100.0, 0.01, 4.0, 0.43),
            ("d=1000 u=5 hr=1.5", 1000.0, 0.01, 1.5, 0.72),
        ];
        let mut max_n = 0.0_f64;
        let mut any_ge_4 = false;
        for (label, d, h_s, h_r, a) in cases {
            let n = ray_count_eq279(d, h_s, h_r, a, C0);
            eprintln!("SM8 Eq.279 N({label}) = {n:.3}");
            max_n = max_n.max(n);
            if n >= SM8_RAY_COUNT_THRESHOLD {
                any_ge_4 = true;
            }
        }
        // Short-range downwind (d ≤ 100 m) stays below the threshold: Sub-models
        // 1–6 carry all significant rays. The long-range d = 1000 m case exceeds
        // it — the documented, accepted gap (that case also gates on the
        // [ASSUMED] emission model, so it stays Skipped regardless).
        let short_range = ray_count_eq279(100.0, 0.01, 1.5, 0.43, C0);
        assert!(
            short_range < SM8_RAY_COUNT_THRESHOLD,
            "short-range downwind must not demand SM8: N = {short_range}"
        );
        assert!(
            any_ge_4,
            "the long-range downwind case reaches N ≥ 4 (evidence)"
        );
        assert!(max_n.is_finite());
    }

    /// A non-downward profile (A ≤ 0) never triggers multiple ground reflections.
    #[test]
    fn upward_or_neutral_profile_gives_zero_rays() {
        assert_eq!(ray_count_eq279(1000.0, 0.5, 1.5, 0.0, C0), 0.0);
        assert_eq!(ray_count_eq279(1000.0, 0.5, 1.5, -0.5, C0), 0.0);
    }
}
