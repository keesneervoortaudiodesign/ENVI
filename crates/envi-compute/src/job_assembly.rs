//! `SolveJob` assembly + the directional-phase seam (SRC-03 Folded Todo).
//!
//! This is the FIRST construction site to populate
//! `envi_engine::solver::SolveJob::directivity_phase_rad` — the coherent
//! directional-source composition path the Phase-04 deferred-items note points
//! to. For each directional sub-source at its local `src→rcv` direction, the
//! assembly evaluates the balloon's magnitude (`eval → directivity_gain_db`) and,
//! **gated on `has_phase()`**, its phase (`eval_phase → directivity_phase_rad`).
//! A phase-free (magnitude-only) balloon yields `directivity_phase_rad = None`,
//! leaving `arg(H_coh)` bit-identical to the pre-seam path (backward-compatible).
//!
//! # The convention boundary stays in the engine
//!
//! The assembly supplies the raw `[f64; 105]` phase slice ONLY. The engine solver
//! applies `e^{+jΔφ}` to `H_coh` (and never to `P_incoh`) at its single
//! ENVI-convention boundary via explicit `(cos, sin)` — this module performs no
//! sign inversion of the imaginary part and no phase transform whatsoever
//! (the convention-boundary grep gate over this file stays at zero). It is pure
//! orchestration over the frozen, FORCE-validated `solve()` path.
//!
//! # ENG-09/10 threading
//!
//! A drawn forest object populates [`SolveJob::forest`] and a drawn
//! semi-transparent screen/façade populates [`SolveJob::isolation`], threaded
//! from the [`SolveCtx`], so those drawn objects are never silently inert. Per-path
//! geometry selection (which forest/partition a given `src→rcv` path crosses) is a
//! Phase-9/11 upstream concern; this assembly threads the context's crossing.
//!
//! # Receiver-major order
//!
//! [`assemble_jobs`] yields jobs receiver-major within the requested range (outer
//! loop receivers, inner loop sub-sources), i.e. non-decreasing receiver index —
//! exactly the order `envi_engine::solver::solve` requires.

use std::ops::Range;

use envi_engine::directivity::{DirectivityBalloon, Rotation3};
use envi_engine::forest::ForestCrossing;
use envi_engine::freq::FreqAxis;
use envi_engine::propagation::air_absorption::Atmosphere;
use envi_engine::propagation::coherence::CoherenceInputs;
use envi_engine::propagation::refraction::SoundSpeedProfile;
use envi_engine::propagation::transmission::IsolationSpectrum;
use envi_engine::scene::TerrainProfile;
use envi_engine::solver::SolveJob;

/// A sub-source placement: its position and optional directivity (balloon +
/// source orientation). `None` directivity is an omnidirectional sub-source
/// (both directivity fields become `None`).
#[derive(Debug, Clone, Copy)]
pub struct SubSourcePlacement<'a> {
    /// Sub-source position `[x, y, z]`, meters.
    pub position: [f64; 3],
    /// Optional directional pattern for this sub-source.
    pub directivity: Option<Directional<'a>>,
}

/// A directional pattern: a balloon plus the source→local-frame rotation applied
/// to the `src→rcv` world direction before evaluating the balloon.
#[derive(Debug, Clone, Copy)]
pub struct Directional<'a> {
    /// The per-band spherical directivity balloon (may carry phase).
    pub balloon: &'a DirectivityBalloon,
    /// Rotation from the world `src→rcv` direction into the balloon's local frame.
    pub orientation: Rotation3,
}

/// A receiver placement: its global (receiver-major) index and position.
#[derive(Debug, Clone, Copy)]
pub struct ReceiverPlacement {
    /// Global receiver index (the `SolveJob.receiver` / sink axis coordinate).
    pub global_index: usize,
    /// Receiver position `[x, y, z]`, meters.
    pub position: [f64; 3],
}

/// The shared context an assembled range of [`SolveJob`]s borrows from: the
/// geometry/atmosphere/coherence/axis/weather, the sub-source and receiver sets,
/// and the optional per-corridor forest/partition crossings (ENG-09/10).
#[derive(Debug, Clone, Copy)]
pub struct SolveCtx<'a> {
    /// The `src→rcv` cut-plane terrain profile (shared across this range).
    pub profile: &'a TerrainProfile,
    /// Atmospheric state for the free-field direct path.
    pub atmosphere: &'a Atmosphere,
    /// Coherence inputs (its `c0` is also the terrain sound speed).
    pub coh: &'a CoherenceInputs,
    /// The shared 105-point frequency axis.
    pub axis: &'a FreqAxis,
    /// Optional refraction profile (`None` = homogeneous atmosphere).
    pub weather: Option<&'a SoundSpeedProfile>,
    /// The sub-sources (tensor rows).
    pub sub_sources: &'a [SubSourcePlacement<'a>],
    /// The receivers (tensor columns), ordered by global index.
    pub receivers: &'a [ReceiverPlacement],
    /// Optional forest crossing on this corridor (ENG-09) — drawn forest.
    pub forest: Option<ForestCrossing>,
    /// Optional semi-transparent partition on this corridor (ENG-10) — drawn
    /// screen/façade sound-reduction spectrum.
    pub isolation: Option<&'a IsolationSpectrum>,
}

/// Assemble the [`SolveJob`]s for receivers whose global index lies in `range`,
/// receiver-major (non-decreasing receiver index), one job per
/// `(receiver, sub_source)` pair.
///
/// The directional-phase seam (SRC-03) is wired here: for a directional
/// sub-source, `directivity_gain_db = Some(eval(dir_local))` and
/// `directivity_phase_rad = balloon.has_phase().then(|| eval_phase(dir_local))`,
/// where `dir_local` is the source orientation applied to the unit `src→rcv`
/// world direction. No sign inversion or phase transform is performed here.
#[must_use]
pub fn assemble_jobs<'a>(range: Range<usize>, ctx: &SolveCtx<'a>) -> Vec<SolveJob<'a>> {
    let mut jobs = Vec::new();
    // Receiver-major: outer loop over receivers in the range (non-decreasing
    // index), inner loop over sub-sources.
    for rcv in ctx.receivers {
        if !range.contains(&rcv.global_index) {
            continue;
        }
        for (s, sub) in ctx.sub_sources.iter().enumerate() {
            // Directional-phase seam (SRC-03): evaluate the balloon at the local
            // src→rcv direction. `directivity_phase_rad` is populated ONLY when
            // the balloon carries phase (`has_phase()`); a magnitude-only balloon
            // leaves it `None`, keeping `arg(H_coh)` bit-identical. No sign
            // inversion or phase transform happens here — the engine solver
            // applies `e^{+jΔφ}` at its single ENVI-convention boundary.
            let (directivity_gain_db, directivity_phase_rad) = match sub.directivity {
                Some(dir) => {
                    let dir_local = dir.orientation.apply(unit(sub.position, rcv.position));
                    let gain = dir.balloon.eval(dir_local);
                    let phase = dir
                        .balloon
                        .has_phase()
                        .then(|| dir.balloon.eval_phase(dir_local));
                    (Some(gain), phase)
                }
                None => (None, None),
            };
            jobs.push(SolveJob {
                sub_source: s,
                receiver: rcv.global_index,
                profile: ctx.profile,
                src: sub.position,
                rcv: rcv.position,
                atmosphere: ctx.atmosphere,
                coh: ctx.coh,
                axis: ctx.axis,
                weather: ctx.weather,
                directivity_gain_db,
                directivity_phase_rad,
                // ENG-09/10: drawn forest/partition crossings threaded so they are
                // never silently inert (per-path selection is a Phase-9/11 concern).
                forest: ctx.forest,
                isolation: ctx.isolation,
            });
        }
    }
    jobs
}

/// The unit vector from `from` to `to`; a degenerate (zero-length) separation
/// yields `[0, 0, 0]` (the balloon then returns its no-gain slice).
fn unit(from: [f64; 3], to: [f64; 3]) -> [f64; 3] {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if n < 1e-12 {
        [0.0, 0.0, 0.0]
    } else {
        [d[0] / n, d[1] / n, d[2] / n]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::freq::N_BANDS;
    use envi_engine::propagation::air_absorption::Atmosphere;
    use envi_engine::scene::GroundSegment;
    use envi_engine::tensor::InMemorySink;
    use num_complex::Complex;
    use std::f64::consts::PI;

    const C0: f64 = 340.348;

    fn atmos() -> Atmosphere {
        Atmosphere::new(15.0, 70.0, 101.325).unwrap()
    }

    fn coh() -> CoherenceInputs {
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

    fn flat_profile() -> TerrainProfile {
        TerrainProfile::new(
            vec![[2.5, 0.0], [100.0, 0.0]],
            vec![GroundSegment {
                flow_resistivity: 200.0,
                roughness: 0.0,
            }],
        )
        .unwrap()
    }

    fn az_pol_to_unit(az_deg: f64, pol_deg: f64) -> (f64, f64, f64) {
        let a = az_deg.to_radians();
        let p = pol_deg.to_radians();
        (p.sin() * a.cos(), p.sin() * a.sin(), p.cos())
    }

    /// A magnitude-only (phase-free) balloon with a mild directional lobe.
    fn magnitude_balloon() -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler(10.0, 10.0, |az, pol, band| {
            let (x, _y, _z) = az_pol_to_unit(az, pol);
            (2.0 + 0.01 * band as f64) * x
        })
        .unwrap()
    }

    /// A phased balloon: same mild magnitude lobe plus an azimuth-dependent phase
    /// so a source rotation changes the coherent interference (not just level).
    fn phased_balloon() -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler_with_phase(
            10.0,
            10.0,
            |az, pol, band| {
                let (x, _y, _z) = az_pol_to_unit(az, pol);
                (2.0 + 0.01 * band as f64) * x
            },
            |az, _pol, _band| 0.6 * az.to_radians().sin(),
        )
        .unwrap()
    }

    const SRC: [f64; 3] = [2.5, 0.0, 0.5];
    const RCV: [f64; 3] = [100.0, 0.0, 1.5];

    /// Solve one range with two co-located sub-sources and return the per-band
    /// coherent SUM Σ_s H_coh[s, 0, f] (the coherent readout the phase enters).
    fn coherent_sum(ctx: &SolveCtx<'_>) -> Vec<Complex<f64>> {
        let jobs = assemble_jobs(0..1, ctx);
        let mut sink = InMemorySink::new(ctx.sub_sources.len(), 1);
        envi_engine::solver::solve(jobs, ctx.sub_sources.len(), 1, &mut sink).unwrap();
        let pair = sink.tensor();
        (0..N_BANDS)
            .map(|f| {
                (0..ctx.sub_sources.len())
                    .map(|s| pair.h_coh[[s, 0, f]])
                    .sum()
            })
            .collect()
    }

    #[test]
    fn rotating_a_phased_balloon_changes_the_coherent_sum_argument() {
        let profile = flat_profile();
        let a = atmos();
        let c = coh();
        let axis = FreqAxis::new();
        let balloon = phased_balloon();
        let receivers = [ReceiverPlacement {
            global_index: 0,
            position: RCV,
        }];

        // Baseline: both sub-sources oriented identically.
        let subs_base = [
            SubSourcePlacement {
                position: SRC,
                directivity: Some(Directional {
                    balloon: &balloon,
                    orientation: Rotation3::identity(),
                }),
            },
            SubSourcePlacement {
                position: SRC,
                directivity: Some(Directional {
                    balloon: &balloon,
                    orientation: Rotation3::identity(),
                }),
            },
        ];
        let ctx_base = SolveCtx {
            profile: &profile,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            sub_sources: &subs_base,
            receivers: &receivers,
            forest: None,
            isolation: None,
        };

        // Rotated: sub-source 1's balloon is turned 90° about Z, changing its
        // local src→rcv direction ⇒ a different directional phase ⇒ different
        // coherent interference.
        let subs_rot = [
            subs_base[0],
            SubSourcePlacement {
                position: SRC,
                directivity: Some(Directional {
                    balloon: &balloon,
                    orientation: Rotation3::about_z(PI / 2.0),
                }),
            },
        ];
        let ctx_rot = SolveCtx {
            sub_sources: &subs_rot,
            ..ctx_base
        };

        let base = coherent_sum(&ctx_base);
        let rot = coherent_sum(&ctx_rot);

        let mut saw_arg_change = false;
        for f in 0..N_BANDS {
            if base[f].norm() > 1e-30
                && rot[f].norm() > 1e-30
                && (rot[f].arg() - base[f].arg()).abs() > 1e-9
            {
                saw_arg_change = true;
            }
        }
        assert!(
            saw_arg_change,
            "rotating a phased sub-source must change the coherent sum's argument"
        );
    }

    #[test]
    fn phase_free_balloon_leaves_arg_bit_identical_to_the_magnitude_only_path() {
        let profile = flat_profile();
        let a = atmos();
        let c = coh();
        let axis = FreqAxis::new();
        let balloon = magnitude_balloon();
        assert!(
            !balloon.has_phase(),
            "the baseline balloon carries no phase"
        );

        let receivers = [ReceiverPlacement {
            global_index: 0,
            position: RCV,
        }];
        let subs = [SubSourcePlacement {
            position: SRC,
            directivity: Some(Directional {
                balloon: &balloon,
                orientation: Rotation3::identity(),
            }),
        }];
        let ctx = SolveCtx {
            profile: &profile,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            sub_sources: &subs,
            receivers: &receivers,
            forest: None,
            isolation: None,
        };

        // The assembled job must NOT wire a phase slice for a phase-free balloon.
        let jobs = assemble_jobs(0..1, &ctx);
        assert_eq!(jobs.len(), 1, "one (receiver, sub_source) job");
        assert!(
            jobs[0].directivity_phase_rad.is_none(),
            "a phase-free balloon must leave directivity_phase_rad = None (SRC-03 gating)"
        );
        assert!(
            jobs[0].directivity_gain_db.is_some(),
            "the magnitude gain is still applied"
        );

        // Assembled (phase-free) solve.
        let mut sink_pf = InMemorySink::new(1, 1);
        envi_engine::solver::solve(assemble_jobs(0..1, &ctx), 1, 1, &mut sink_pf).unwrap();

        // Hand-built magnitude-only baseline: the SAME gain, phase explicitly None.
        let dir_local = Rotation3::identity().apply(unit(SRC, RCV));
        let gain = balloon.eval(dir_local);
        let baseline = SolveJob {
            sub_source: 0,
            receiver: 0,
            profile: &profile,
            src: SRC,
            rcv: RCV,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            directivity_gain_db: Some(gain),
            directivity_phase_rad: None,
            forest: None,
            isolation: None,
        };
        let mut sink_base = InMemorySink::new(1, 1);
        envi_engine::solver::solve([baseline], 1, 1, &mut sink_base).unwrap();

        // arg(H_coh) is bit-identical (f64::to_bits) — the phase-free balloon is
        // exactly the magnitude-only path (no spurious phase / sign inversion).
        for f in 0..N_BANDS {
            let pf = sink_pf.tensor().h_coh[[0, 0, f]];
            let bl = sink_base.tensor().h_coh[[0, 0, f]];
            assert_eq!(
                pf.arg().to_bits(),
                bl.arg().to_bits(),
                "band {f}: phase-free arg(H_coh) must be bit-identical to the baseline"
            );
        }
    }

    #[test]
    fn jobs_are_receiver_major_and_thread_forest_isolation() {
        let profile = flat_profile();
        let a = atmos();
        let c = coh();
        let axis = FreqAxis::new();
        let balloon = magnitude_balloon();
        let receivers = [
            ReceiverPlacement {
                global_index: 0,
                position: [100.0, 0.0, 1.5],
            },
            ReceiverPlacement {
                global_index: 1,
                position: [100.0, 0.0, 2.5],
            },
        ];
        let subs = [
            SubSourcePlacement {
                position: SRC,
                directivity: Some(Directional {
                    balloon: &balloon,
                    orientation: Rotation3::identity(),
                }),
            },
            SubSourcePlacement {
                position: [2.5, 0.0, 0.8],
                directivity: None,
            },
        ];
        let forest = ForestCrossing::new(80.0, 0.4, 0.12, 0.2, 12.0).unwrap();
        let ctx = SolveCtx {
            profile: &profile,
            atmosphere: &a,
            coh: &c,
            axis: &axis,
            weather: None,
            sub_sources: &subs,
            receivers: &receivers,
            forest: Some(forest),
            isolation: None,
        };

        let jobs = assemble_jobs(0..2, &ctx);
        assert_eq!(jobs.len(), 4, "2 receivers × 2 sub-sources");
        // Receiver-major: non-decreasing receiver index across the job order.
        let mut last = 0usize;
        for j in &jobs {
            assert!(j.receiver >= last, "jobs must be receiver-major");
            last = j.receiver;
            // ENG-09: the drawn forest is threaded, never silently dropped.
            assert!(j.forest.is_some(), "forest crossing must be threaded");
        }
        // The omnidirectional sub-source carries no directivity.
        let omni = jobs.iter().find(|j| j.sub_source == 1).unwrap();
        assert!(omni.directivity_gain_db.is_none());
        assert!(omni.directivity_phase_rad.is_none());
    }
}
