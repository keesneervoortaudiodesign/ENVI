//! Integration test: the coherent MAC over a stored transfer tensor equals a
//! full recompute — conditioning folded into the source — BIT-FOR-BIT (OUT-03).
//!
//! The conditioning gain `G_s(f)` is composed ONCE (filter × delay × 10^{L_W/20})
//! and reused on both paths, so the two agree to the raw f64 bits, not merely to
//! an epsilon (04-RESEARCH Pitfall 6). Two paths:
//!   A (MAC):       apply G at readout over the stored H.
//!   B (recompute): fold the SAME G into the tensor, then read out with unit gain.

use envi_engine::freq::{FreqAxis, N_BANDS};
use envi_engine::propagation::air_absorption::Atmosphere;
use envi_engine::propagation::coherence::CoherenceInputs;
use envi_engine::scene::{BandSpectrum, GroundSegment, TerrainProfile};
use envi_engine::solver::{SolveJob, solve};
use envi_engine::tensor::{InMemorySink, compose_gain, readout_coherent};
use num_complex::Complex;

const C0: f64 = 340.348;

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

#[test]
fn coherent_mac_equals_full_recompute_bit_for_bit() {
    let axis = FreqAxis::new();
    let atmos = Atmosphere::new(15.0, 70.0, 101.325).unwrap();
    let c = coh();
    let profile = TerrainProfile::new(
        vec![[2.5, 0.0], [100.0, 0.0]],
        vec![GroundSegment {
            flow_resistivity: 200.0,
            roughness: 0.0,
        }],
    )
    .unwrap();

    let n_sub = 2;
    let n_rcv = 3;

    // Receiver-major jobs; distinct heights so columns and rows differ.
    let mut jobs = Vec::new();
    for r in 0..n_rcv {
        for s in 0..n_sub {
            jobs.push(SolveJob {
                sub_source: s,
                receiver: r,
                profile: &profile,
                src: [2.5, 0.0, 0.3 + 0.2 * s as f64],
                rcv: [100.0, 0.0, 1.0 + 0.5 * r as f64],
                atmosphere: &atmos,
                coh: &c,
                axis: &axis,
                weather: None,
                directivity_gain_db: None,
                directivity_phase_rad: None,
                forest: None,
                isolation: None,
            });
        }
    }
    let mut sink = InMemorySink::new(n_sub, n_rcv);
    solve(jobs, n_sub, 2, &mut sink).unwrap();
    let pair = sink.tensor();

    // Compose per-sub conditioning ONCE: distinct L_W, a non-trivial complex
    // filter, and a per-sub delay.
    let mut g = Vec::new();
    for s in 0..n_sub {
        let mut lw = [0.0f64; N_BANDS];
        for (i, v) in lw.iter_mut().enumerate() {
            *v = 60.0 + 0.05 * i as f64 + 3.0 * s as f64;
        }
        let lw = BandSpectrum::from_values(lw);
        let filter: Vec<Complex<f64>> = (0..N_BANDS)
            .map(|i| Complex::from_polar(0.8 + 0.001 * i as f64, 0.03 * i as f64))
            .collect();
        let delay = 1.0e-3 * (s as f64 + 1.0);
        g.push(compose_gain(&lw, Some(&filter), delay, &axis).unwrap());
    }

    // Path A: MAC over stored H.
    let p_mac = readout_coherent(pair.h_coh.view(), &g).unwrap();

    // Path B: fold the SAME G into the tensor, then read out with unit gain.
    let mut h_cond = pair.h_coh.clone();
    for ((s, _r, f), v) in h_cond.indexed_iter_mut() {
        *v *= g[s][f];
    }
    let unit = vec![vec![Complex::new(1.0, 0.0); N_BANDS]; n_sub];
    let p_rec = readout_coherent(h_cond.view(), &unit).unwrap();

    // Bit-for-bit equality on both real and imaginary parts.
    for ((r, f), &pm) in p_mac.indexed_iter() {
        let pr = p_rec[[r, f]];
        assert_eq!(
            pm.re.to_bits(),
            pr.re.to_bits(),
            "Re mismatch at receiver {r}, band {f}"
        );
        assert_eq!(
            pm.im.to_bits(),
            pr.im.to_bits(),
            "Im mismatch at receiver {r}, band {f}"
        );
    }
}
