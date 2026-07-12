//! Per-receiver two-channel readout orchestration (D-08) — drives the
//! FORCE-validated engine laws over a decoded tensor slice to produce band
//! levels, dB(A)/dB(C) totals, and the coherent/incoherent split.
//!
//! # Module I/O
//! - **Input:** a decoded tensor pair (`H_coh: ArrayView3<Complex<f64>>`,
//!   `P_incoh_abs: ArrayView3<f64>`, both `[n_sub, n_rcv, N_BANDS]`; see
//!   [`opfs_reader`](../../envi_compute_wasm/opfs_reader/index.html)), a receiver
//!   index, per-sub-source `L_W` [`BandSpectrum`]s, per-sub-source
//!   [`Conditioning`], a [`ReadoutLaw`], and the [`FreqAxis`].
//! - **Output:** a [`ReceiverReadout`] carrying `band_levels_db`,
//!   `coherent_energy`, `incoherent_energy`, and the `total_dba`/`total_dbc`
//!   weighted totals (the D-08 split is ALWAYS present).
//!
//! # Drives engine laws — never re-derives dB/MAC (RESEARCH Pitfall 1)
//! The single biggest risk this phase carries is a divergent readout. So this
//! module NEVER hand-rolls a dB/weighting/MAC sum: it calls
//! [`compose_gain`](envi_engine::tensor::compose_gain) once per sub-source,
//! [`readout_coherent`](envi_engine::tensor::readout_coherent) for the OUT-03
//! MAC, [`readout_incoherent`](envi_engine::tensor::readout_incoherent) for the
//! Annex-A energy channels, and
//! [`band_levels_db_two_channel`](envi_engine::transfer::band_levels_db_two_channel)
//! for the per-band dB conversion. The MAC ≡ engine bit-exact test is the
//! acceptance gate (threat T-11-01-02).
//!
//! # Readout-law policy (Open Q2 — no explicit source-type flag exists today)
//! The scene DTO carries no coherent-vs-incoherent flag, so [`default_law`]
//! derives it from source composition: a sub-source with a conditioning filter
//! present OR a non-zero delay is a conditioned/directional source → **coherent**
//! (loudspeaker-array summation); a plain road/omnidirectional sub-source →
//! **incoherent** (AV 1106/07 Annex-A energy sum, two co-located = +3 dB not +6).
//! The caller may override by passing an explicit [`ReadoutLaw`] (e.g. force
//! coherent when a directivity phase grid is present in the tensor).

use envi_engine::freq::{FreqAxis, N_BANDS};
use envi_engine::scene::BandSpectrum;
use envi_engine::tensor::{SinkError, compose_gain, readout_coherent, readout_incoherent};
use envi_engine::transfer::band_levels_db_two_channel;
use ndarray::{Array3, ArrayView3, s};
use num_complex::Complex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::weighting::{a_weighting_db, c_weighting_db, weighted_total_db};

/// Per-source conditioning: a **READOUT** parameter (gain/delay/filter/mute)
/// applied at level readout via [`compose_gain`].
///
/// It is structurally **excluded from tensor identity** (D-07): conditioning
/// appears nowhere in `envi_engine::solver::SolveJob`, only at readout, so
/// hashing it would make Tier-1 recondition requests self-invalidating. See
/// [`crate::identity::tensor_hash`] — its signature cannot accept this type.
/// Extensible via `#[serde(default)]`.
///
/// Lives in `envi-compute` (not `envi-store`) so the WASM recondition boundary
/// can consume it without dragging `std::fs`/`tempfile` into wasm — the same
/// relocation discipline the Phase-10 `MetDto`/`ReceiverDto`/scene DTOs follow.
/// Re-exported at `envi_store::dto::ConditioningDto` so that path stays
/// source-compatible and keeps generating into the committed `wire.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct ConditioningDto {
    /// Broadband gain, dB (default 0.0).
    #[serde(default)]
    pub gain_db: f64,
    /// Delay, milliseconds (default 0.0).
    #[serde(default)]
    pub delay_ms: f64,
    /// Optional per-band filter, dB (dense [105] when present).
    #[serde(default)]
    pub filter_band_db: Option<Vec<f64>>,
    /// Mute flag (default false).
    #[serde(default)]
    pub muted: bool,
}

impl Default for ConditioningDto {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            delay_ms: 0.0,
            filter_band_db: None,
            muted: false,
        }
    }
}

/// Per-sub-source readout conditioning (the WEB-05 gain/filter/delay param).
///
/// `filter` is the dense `[N_BANDS]` complex per-band gain from the reused
/// `SpectrumEditor` (`None` = flat unit filter); `delay_s` is the source delay in
/// seconds. The source `L_W` amplitude is carried separately by the `spectra`
/// argument; [`compose_gain`] multiplies the three into `G_s(f)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Conditioning {
    /// Dense `[N_BANDS]` complex filter gain, or `None` for a flat unit filter.
    pub filter: Option<Vec<Complex<f64>>>,
    /// Source delay in seconds.
    pub delay_s: f64,
}

/// Which readout law a source's sub-sources are summed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadoutLaw {
    /// Coherent MAC (OUT-03): `|Σ_s H_coh·G_s|²` — conditioned/directional sources.
    Coherent,
    /// Incoherent Annex-A energy sum: `Σ_s |G_s|²·(|H_coh|²+P_incoh)` — road/omni.
    Incoherent,
}

/// One receiver's two-channel readout (D-08): band levels, the coherent/incoherent
/// energy split (always present), and the dB(A)/dB(C) weighted totals.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceiverReadout {
    /// Per-band total level in dB, length [`N_BANDS`] (band index order).
    pub band_levels_db: Vec<f64>,
    /// Per-band coherent-channel energy, length [`N_BANDS`].
    pub coherent_energy: Vec<f64>,
    /// Per-band incoherent-channel energy, length [`N_BANDS`].
    pub incoherent_energy: Vec<f64>,
    /// Per-band coherent-channel level in dB, length [`N_BANDS`] — the coherent
    /// half of the D-08 split, `10·log₁₀(coherent_energy)` via the engine law, a
    /// silent band clamped to [`SILENCE_FLOOR_DB`] so the wire stays finite.
    pub coherent_db: Vec<f64>,
    /// Per-band incoherent-channel level in dB, length [`N_BANDS`] — the
    /// incoherent half of the D-08 split (same floor discipline).
    pub incoherent_db: Vec<f64>,
    /// dB(A) total over all bands.
    pub total_dba: f64,
    /// dB(C) total over all bands.
    pub total_dbc: f64,
    /// Unweighted energetic total of the coherent channel, dB (the always-shown
    /// split total; energetic sum of `coherent_db` by band index).
    pub total_coherent_db: f64,
    /// Unweighted energetic total of the incoherent channel, dB.
    pub total_incoherent_db: f64,
}

/// Display floor (dB) for a per-channel level: a fully silent channel band is
/// `10·log₁₀(0) = −∞`, which cannot ride the JS-number wire cleanly. The engine
/// law is driven verbatim, then any non-finite result is clamped to this floor —
/// a presentation clamp, NOT a re-derivation of the dB law (RESEARCH Pitfall 1).
/// −400 dB is far below any physical level, so a live band is never affected and
/// an energetic total of an all-floored channel is still finite (≈ −380 dB).
pub const SILENCE_FLOOR_DB: f64 = -400.0;

/// Clamp any non-finite per-band level (a `−∞` from an empty channel) to
/// [`SILENCE_FLOOR_DB`] so the readout carries only finite values on the wire.
fn floor_levels(levels: Vec<f64>) -> Vec<f64> {
    levels
        .into_iter()
        .map(|v| if v.is_finite() { v } else { SILENCE_FLOOR_DB })
        .collect()
}

/// The default readout-law policy (Open Q2): a conditioned/directional source
/// (any filter present or non-zero delay) is **coherent**, otherwise **incoherent**.
#[must_use]
pub fn default_law(conditioning: &[Conditioning]) -> ReadoutLaw {
    let conditioned = conditioning
        .iter()
        .any(|c| c.filter.is_some() || c.delay_s != 0.0);
    if conditioned {
        ReadoutLaw::Coherent
    } else {
        ReadoutLaw::Incoherent
    }
}

/// Read out one receiver's two-channel spectrum + dB(A)/dB(C) totals by driving
/// the engine laws over the decoded per-receiver tensor slice.
///
/// # Errors
/// [`SinkError`] on any dimension/gain mismatch (receiver out of range, wrong
/// sub-source or band count, non-finite conditioning) — never a panic on data.
pub fn readout_receiver(
    h_coh: ArrayView3<'_, Complex<f64>>,
    p_incoh_abs: ArrayView3<'_, f64>,
    r: usize,
    spectra: &[BandSpectrum],
    conditioning: &[Conditioning],
    law: ReadoutLaw,
    axis: &FreqAxis,
) -> Result<ReceiverReadout, SinkError> {
    let (n_sub, n_rcv, n_f) = h_coh.dim();
    // Validate the trust boundary before touching the engine laws (never panic).
    if p_incoh_abs.dim() != (n_sub, n_rcv, n_f) {
        return Err(SinkError::ChannelShapeMismatch {
            h_coh: (n_sub, n_rcv, n_f),
            p_incoh: p_incoh_abs.dim(),
        });
    }
    if n_f != N_BANDS {
        return Err(SinkError::BandCountMismatch {
            expected: N_BANDS,
            got: n_f,
        });
    }
    if r >= n_rcv {
        return Err(SinkError::ChunkOutOfBounds {
            r_offset: r,
            chunk_len: 1,
            n_receivers: n_rcv,
        });
    }
    if spectra.len() != n_sub {
        return Err(SinkError::GainSubCountMismatch {
            expected: n_sub,
            got: spectra.len(),
        });
    }
    if conditioning.len() != n_sub {
        return Err(SinkError::GainSubCountMismatch {
            expected: n_sub,
            got: conditioning.len(),
        });
    }

    // Compose the complex per-sub-source gain ONCE via the engine's frozen
    // compose_gain (amplitude · filter · delay) — the composition order that makes
    // the coherent MAC bit-identical to a full recompute.
    let g: Vec<Vec<Complex<f64>>> = spectra
        .iter()
        .zip(conditioning)
        .map(|(sp, c)| compose_gain(sp, c.filter.as_deref(), c.delay_s, axis))
        .collect::<Result<_, _>>()?;
    // Real energy weights w_s = |G_s|² for the incoherent (Annex-A) law.
    let w: Vec<Vec<f64>> = g
        .iter()
        .map(|gs| gs.iter().map(num_complex::Complex::norm_sqr).collect())
        .collect();

    // Single-receiver tensor slices [n_sub, 1, N_BANDS] (freq fastest).
    let h_r = h_coh.slice(s![.., r..r + 1, ..]);
    let p_r = p_incoh_abs.slice(s![.., r..r + 1, ..]);
    let zeros_h = Array3::<Complex<f64>>::zeros((n_sub, 1, N_BANDS));
    let zeros_p = Array3::<f64>::zeros((n_sub, 1, N_BANDS));

    // Incoherent (turbulence-decorrelated) channel = Σ_s |G_s|²·P_incoh_abs, via
    // readout_incoherent with a zero H_coh — drives the engine law, never a
    // bespoke energy loop.
    let e_incoh = readout_incoherent(zeros_h.view(), p_r, &w)?;
    let incoherent_energy: Vec<f64> = e_incoh.row(0).to_vec();

    // Coherent channel — the readout law selects HOW the sub-sources combine.
    let coherent_energy: Vec<f64> = match law {
        // OUT-03 MAC: |Σ_s H_coh·G_s|² (phase-preserving, loudspeaker-array sum).
        ReadoutLaw::Coherent => {
            let p = readout_coherent(h_r, &g)?;
            p.row(0)
                .iter()
                .map(num_complex::Complex::norm_sqr)
                .collect()
        }
        // Annex-A energetic sum of the coherent-transfer magnitude: Σ_s |G_s|²·|H_coh|²,
        // via readout_incoherent with a zero P_incoh channel (two co-located = +3 dB).
        ReadoutLaw::Incoherent => {
            let e_h = readout_incoherent(h_r, zeros_p.view(), &w)?;
            e_h.row(0).to_vec()
        }
    };

    // Per-band total dB via the FROZEN two-channel dB law (never a bespoke log10
    // loop, RESEARCH Pitfall 1): encode the coherent energy as |H_coh|² and the
    // incoherent channel as |H_ff|²·p_incoh with p_incoh ≡ 1, so the engine law
    // returns 10·log10(coherent + incoherent) with a zero L_W.
    let h_eff: Vec<Complex<f64>> = coherent_energy
        .iter()
        .map(|&e| Complex::new(e.sqrt(), 0.0))
        .collect();
    let hff_eff: Vec<Complex<f64>> = incoherent_energy
        .iter()
        .map(|&e| Complex::new(e.sqrt(), 0.0))
        .collect();
    let ones = vec![1.0_f64; N_BANDS];
    let band_levels_db =
        band_levels_db_two_channel(&h_eff, &hff_eff, &ones, &BandSpectrum::uniform(0.0));

    // Per-channel dB for the D-08 split overlay: drive the SAME two-channel dB law
    // with the other channel zeroed (never a bespoke 10·log10) — coherent-only is
    // `band_levels(h_eff, 0)` = 10·log10(coherent_energy); incoherent-only is
    // `band_levels(0, hff_eff)` = 10·log10(incoherent_energy). A silent channel's
    // −∞ is clamped to the finite SILENCE_FLOOR_DB for the wire.
    let zeros_ch = vec![Complex::new(0.0, 0.0); N_BANDS];
    let coherent_db = floor_levels(band_levels_db_two_channel(
        &h_eff,
        &zeros_ch,
        &ones,
        &BandSpectrum::uniform(0.0),
    ));
    let incoherent_db = floor_levels(band_levels_db_two_channel(
        &zeros_ch,
        &hff_eff,
        &ones,
        &BandSpectrum::uniform(0.0),
    ));

    // dB(A)/dB(C) totals via the Task-1 A/C tables (D-08 split always present).
    let total_dba = weighted_total_db(&band_levels_db, &a_weighting_db(axis));
    let total_dbc = weighted_total_db(&band_levels_db, &c_weighting_db(axis));
    // Unweighted energetic per-channel totals (the always-shown split): a zero
    // weighting table makes `weighted_total_db` a pure energetic sum by band index.
    let zeros_w = [0.0_f64; N_BANDS];
    let total_coherent_db = weighted_total_db(&coherent_db, &zeros_w);
    let total_incoherent_db = weighted_total_db(&incoherent_db, &zeros_w);

    Ok(ReceiverReadout {
        band_levels_db,
        coherent_energy,
        incoherent_energy,
        coherent_db,
        incoherent_db,
        total_dba,
        total_dbc,
        total_coherent_db,
        total_incoherent_db,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn hval(s: usize, r: usize, f: usize) -> Complex<f64> {
        let k = (s * 100 + r * 10 + f) as f64;
        Complex::from_polar(0.1 + 0.001 * k, 0.017 * k - 0.3)
    }

    fn pval(s: usize, r: usize, f: usize) -> f64 {
        let k = (s * 100 + r * 10 + f) as f64;
        1.0e-6 * (1.0 + k)
    }

    #[test]
    fn coherent_output_equals_direct_readout_coherent_bit_for_bit() {
        // THE load-bearing gate (T-11-01-02): readout_receiver's coherent energy
        // equals |direct readout_coherent|² bit-for-bit over a synthetic tensor.
        let axis = FreqAxis::new();
        let (n_sub, n_rcv) = (3usize, 4usize);
        let h = Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| hval(s, r, f));
        let p = Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| pval(s, r, f));
        let spectra: Vec<BandSpectrum> = (0..n_sub)
            .map(|s| BandSpectrum::uniform(70.0 + s as f64))
            .collect();
        let conditioning: Vec<Conditioning> = (0..n_sub)
            .map(|s| Conditioning {
                filter: Some(vec![Complex::new(0.5 + 0.1 * s as f64, 0.05); N_BANDS]),
                delay_s: 1.0e-4 * (s as f64 + 1.0),
            })
            .collect();

        // Direct engine gains + MAC over the full tensor.
        let g: Vec<Vec<Complex<f64>>> = spectra
            .iter()
            .zip(&conditioning)
            .map(|(sp, c)| compose_gain(sp, c.filter.as_deref(), c.delay_s, &axis).unwrap())
            .collect();
        let p_direct = readout_coherent(h.view(), &g).unwrap();

        for r in 0..n_rcv {
            let ro = readout_receiver(
                h.view(),
                p.view(),
                r,
                &spectra,
                &conditioning,
                ReadoutLaw::Coherent,
                &axis,
            )
            .unwrap();
            for f in 0..N_BANDS {
                assert_eq!(
                    ro.coherent_energy[f].to_bits(),
                    p_direct[[r, f]].norm_sqr().to_bits(),
                    "coherent MAC diverged at r={r} f={f}"
                );
            }
        }
    }

    #[test]
    fn two_colocated_incoherent_subs_add_three_db_not_six() {
        // Annex-A anti-phantom-interference guard: two identical co-located
        // incoherent sub-sources give +3.0103 dB, never +6 dB.
        let axis = FreqAxis::new();
        let row: Vec<Complex<f64>> = (0..N_BANDS)
            .map(|f| Complex::from_polar(0.2, -0.13 * f as f64))
            .collect();

        let mut h1 = Array3::<Complex<f64>>::zeros((1, 1, N_BANDS));
        let p1 = Array3::<f64>::zeros((1, 1, N_BANDS));
        let mut h2 = Array3::<Complex<f64>>::zeros((2, 1, N_BANDS));
        let p2 = Array3::<f64>::zeros((2, 1, N_BANDS));
        for f in 0..N_BANDS {
            h1[[0, 0, f]] = row[f];
            h2[[0, 0, f]] = row[f];
            h2[[1, 0, f]] = row[f];
        }
        let spec1 = vec![BandSpectrum::uniform(0.0)];
        let cond1 = vec![Conditioning {
            filter: None,
            delay_s: 0.0,
        }];
        let spec2 = vec![BandSpectrum::uniform(0.0), BandSpectrum::uniform(0.0)];
        let cond2 = vec![
            Conditioning {
                filter: None,
                delay_s: 0.0,
            },
            Conditioning {
                filter: None,
                delay_s: 0.0,
            },
        ];

        let r1 = readout_receiver(
            h1.view(),
            p1.view(),
            0,
            &spec1,
            &cond1,
            ReadoutLaw::Incoherent,
            &axis,
        )
        .unwrap();
        let r2 = readout_receiver(
            h2.view(),
            p2.view(),
            0,
            &spec2,
            &cond2,
            ReadoutLaw::Incoherent,
            &axis,
        )
        .unwrap();
        for f in 0..N_BANDS {
            let delta = r2.band_levels_db[f] - r1.band_levels_db[f];
            assert_relative_eq!(delta, 10.0 * 2f64.log10(), epsilon = 1e-9);
            assert!((delta - 6.0206).abs() > 1.0, "must not be +6 dB at f={f}");
        }
    }

    #[test]
    fn default_law_selects_by_source_composition() {
        let plain = vec![Conditioning {
            filter: None,
            delay_s: 0.0,
        }];
        assert_eq!(default_law(&plain), ReadoutLaw::Incoherent);
        let filtered = vec![Conditioning {
            filter: Some(vec![Complex::new(1.0, 0.0); N_BANDS]),
            delay_s: 0.0,
        }];
        assert_eq!(default_law(&filtered), ReadoutLaw::Coherent);
        let delayed = vec![Conditioning {
            filter: None,
            delay_s: 1.0e-4,
        }];
        assert_eq!(default_law(&delayed), ReadoutLaw::Coherent);
    }

    #[test]
    fn readout_exposes_split_and_distinct_weighted_totals() {
        // A single source with non-zero P_incoh, coherent law: the D-08 split is
        // present, both totals are finite, and dB(A) ≠ dB(C) for a broadband input.
        let axis = FreqAxis::new();
        let h = Array3::from_shape_fn((1, 1, N_BANDS), |(s, r, f)| hval(s, r, f));
        let p = Array3::from_shape_fn((1, 1, N_BANDS), |(s, r, f)| pval(s, r, f));
        let spectra = vec![BandSpectrum::uniform(90.0)];
        let cond = vec![Conditioning {
            filter: None,
            delay_s: 0.0,
        }];
        let ro = readout_receiver(
            h.view(),
            p.view(),
            0,
            &spectra,
            &cond,
            ReadoutLaw::Coherent,
            &axis,
        )
        .unwrap();
        assert_eq!(ro.band_levels_db.len(), N_BANDS);
        assert_eq!(ro.coherent_energy.len(), N_BANDS);
        assert_eq!(ro.incoherent_energy.len(), N_BANDS);
        assert!(ro.incoherent_energy.iter().all(|&e| e > 0.0));
        assert!(ro.total_dba.is_finite() && ro.total_dbc.is_finite());
        // The dB(A)/dB(C) totals are wired to the Task-1 A/C tables bit-exactly
        // (no bespoke weighting sum), and the two tables genuinely differ.
        let a = a_weighting_db(&axis);
        let c = c_weighting_db(&axis);
        assert_eq!(
            ro.total_dba.to_bits(),
            weighted_total_db(&ro.band_levels_db, &a).to_bits()
        );
        assert_eq!(
            ro.total_dbc.to_bits(),
            weighted_total_db(&ro.band_levels_db, &c).to_bits()
        );
        assert_ne!(ro.total_dba, ro.total_dbc);
    }

    #[test]
    fn per_channel_split_reconstructs_the_combined_band_level() {
        // The D-08 split gate: for every band, the combined level is the energetic
        // sum of the coherent + incoherent per-channel dB (10·log10(10^(coh/10) +
        // 10^(inco/10)) == band_levels_db). Both per-channel dBs and both energetic
        // totals are finite. Uses the coherent law with a non-zero P_incoh so both
        // channels are live.
        let axis = FreqAxis::new();
        let h = Array3::from_shape_fn((2, 2, N_BANDS), |(s, r, f)| hval(s, r, f));
        let p = Array3::from_shape_fn((2, 2, N_BANDS), |(s, r, f)| pval(s, r, f));
        let spectra = vec![BandSpectrum::uniform(85.0), BandSpectrum::uniform(88.0)];
        let cond = vec![
            Conditioning {
                filter: Some(vec![Complex::new(0.7, 0.1); N_BANDS]),
                delay_s: 1.0e-4,
            },
            Conditioning {
                filter: Some(vec![Complex::new(0.6, -0.05); N_BANDS]),
                delay_s: 2.0e-4,
            },
        ];
        let ro = readout_receiver(
            h.view(),
            p.view(),
            1,
            &spectra,
            &cond,
            ReadoutLaw::Coherent,
            &axis,
        )
        .unwrap();
        assert_eq!(ro.coherent_db.len(), N_BANDS);
        assert_eq!(ro.incoherent_db.len(), N_BANDS);
        assert!(ro.coherent_db.iter().all(|v| v.is_finite()));
        assert!(ro.incoherent_db.iter().all(|v| v.is_finite()));
        assert!(ro.total_coherent_db.is_finite() && ro.total_incoherent_db.is_finite());
        for f in 0..N_BANDS {
            let energetic_sum = 10.0
                * (10f64.powf(ro.coherent_db[f] / 10.0) + 10f64.powf(ro.incoherent_db[f] / 10.0))
                    .log10();
            assert_relative_eq!(ro.band_levels_db[f], energetic_sum, epsilon = 1e-9);
        }
    }

    #[test]
    fn empty_incoherent_channel_floors_instead_of_neg_infinity() {
        // P_incoh all-zero + coherent law ⇒ the incoherent channel is silent; its
        // per-band dB clamps to the finite floor, never −∞, and its energetic total
        // stays finite (so the wire/DTO carries only finite values).
        let axis = FreqAxis::new();
        let h = Array3::from_shape_fn((1, 1, N_BANDS), |(s, r, f)| hval(s, r, f));
        let p = Array3::<f64>::zeros((1, 1, N_BANDS));
        let spectra = vec![BandSpectrum::uniform(80.0)];
        let cond = vec![Conditioning {
            filter: None,
            delay_s: 0.0,
        }];
        let ro = readout_receiver(
            h.view(),
            p.view(),
            0,
            &spectra,
            &cond,
            ReadoutLaw::Coherent,
            &axis,
        )
        .unwrap();
        assert!(ro.incoherent_db.iter().all(|&v| v == SILENCE_FLOOR_DB));
        assert!(ro.total_incoherent_db.is_finite());
        assert!(ro.coherent_db.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn readout_receiver_rejects_bad_receiver_index() {
        let axis = FreqAxis::new();
        let h = Array3::<Complex<f64>>::zeros((1, 2, N_BANDS));
        let p = Array3::<f64>::zeros((1, 2, N_BANDS));
        let spectra = vec![BandSpectrum::uniform(0.0)];
        let cond = vec![Conditioning {
            filter: None,
            delay_s: 0.0,
        }];
        assert!(
            readout_receiver(
                h.view(),
                p.view(),
                2,
                &spectra,
                &cond,
                ReadoutLaw::Incoherent,
                &axis,
            )
            .is_err()
        );
    }
}
