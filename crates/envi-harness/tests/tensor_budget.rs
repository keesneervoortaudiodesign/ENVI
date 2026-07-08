//! Integration test: a 100 000-receiver × 3-sub-source solve streams within the
//! stated 256 MiB budget — proven by the sink's high-water-mark byte counter,
//! with the full complex tensor NEVER resident (OUT-06).
//!
//! The proof is structural (byte accounting), not an RSS sample: the solver
//! fills one receiver-axis chunk at a time and the `CountingSink` folds `Σ_s`
//! per chunk into a compact real readout, so neither the full complex tensor
//! nor a 100k-receiver complex readout is ever materialized.

use envi_engine::freq::{FreqAxis, N_BANDS};
use envi_engine::propagation::air_absorption::Atmosphere;
use envi_engine::propagation::coherence::CoherenceInputs;
use envi_engine::scene::{GroundSegment, TerrainProfile};
use envi_engine::solver::{SolveJob, solve};
use envi_engine::tensor::{BYTES_PER_CELL_PAIR, CountingSink, DEFAULT_TENSOR_BUDGET_BYTES};

const C0: f64 = 340.348;

#[test]
fn hundred_k_receiver_solve_stays_within_the_budget() {
    let axis = FreqAxis::new();
    let atmos = Atmosphere::new(15.0, 70.0, 101.325).unwrap();
    let coh = CoherenceInputs {
        cv2: 0.0,
        ct2: 0.0,
        t_air_c: 15.0,
        c0: C0,
        roughness_r: 0.0,
        f_delta_nu: 1.0,
        d_m: 97.5,
    };
    let profile = TerrainProfile::new(
        vec![[2.5, 0.0], [100.0, 0.0]],
        vec![GroundSegment {
            flow_resistivity: 200.0,
            roughness: 0.0,
        }],
    )
    .unwrap();

    let n_sub = 3usize;
    let n_rcv = 100_000usize;
    // Chunk sizing from the stated budget (04-RESEARCH Pattern 2).
    let chunk_receivers = DEFAULT_TENSOR_BUDGET_BYTES / (n_sub * N_BANDS * BYTES_PER_CELL_PAIR);
    assert!(
        chunk_receivers > 0 && chunk_receivers < n_rcv,
        "budget must force real chunking (chunk_receivers = {chunk_receivers})"
    );

    // Lazy receiver-major job stream — the 300k-job list is never resident.
    let profile_ref = &profile;
    let atmos_ref = &atmos;
    let coh_ref = &coh;
    let axis_ref = &axis;
    let jobs = (0..n_rcv).flat_map(move |r| {
        (0..n_sub).map(move |s| SolveJob {
            sub_source: s,
            receiver: r,
            profile: profile_ref,
            src: [2.5, 0.0, 0.3 + 0.2 * s as f64],
            rcv: [100.0, 0.0, 1.5],
            atmosphere: atmos_ref,
            coh: coh_ref,
            axis: axis_ref,
            weather: None,
            directivity_gain_db: None,
            directivity_phase_rad: None,
        })
    });

    let mut sink = CountingSink::new(n_rcv);
    solve(jobs, n_sub, chunk_receivers, &mut sink).unwrap();

    let budget = 256 * 1024 * 1024;
    assert!(
        sink.high_water_bytes() <= budget,
        "high-water {} B exceeds the {} B budget",
        sink.high_water_bytes(),
        budget
    );
    // The largest chunk is exactly one full chunk_receivers block.
    assert_eq!(
        sink.high_water_bytes(),
        n_sub * chunk_receivers * N_BANDS * BYTES_PER_CELL_PAIR
    );
    // A folded readout exists for every receiver (compact real energy, not the
    // full complex tensor).
    assert_eq!(sink.readout().dim(), (n_rcv, N_BANDS));
    assert!(
        sink.readout()[[0, 0]].is_finite() && sink.readout()[[0, 0]] > 0.0,
        "folded readout must be a finite positive energy"
    );
}
