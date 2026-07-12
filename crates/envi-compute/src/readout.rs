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

use crate::weighting::{a_weighting_db, c_weighting_db, weighted_total_db};

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
    /// dB(A) total over all bands.
    pub total_dba: f64,
    /// dB(C) total over all bands.
    pub total_dbc: f64,
}

/// The default readout-law policy (Open Q2): a conditioned/directional source
/// (any filter present or non-zero delay) is **coherent**, otherwise **incoherent**.
#[must_use]
pub fn default_law(_conditioning: &[Conditioning]) -> ReadoutLaw {
    todo!("GREEN phase")
}

/// Read out one receiver's two-channel spectrum + dB(A)/dB(C) totals by driving
/// the engine laws over the decoded per-receiver tensor slice.
///
/// # Errors
/// [`SinkError`] on any dimension/gain mismatch (receiver out of range, wrong
/// sub-source or band count, non-finite conditioning) — never a panic on data.
pub fn readout_receiver(
    _h_coh: ArrayView3<'_, Complex<f64>>,
    _p_incoh_abs: ArrayView3<'_, f64>,
    _r: usize,
    _spectra: &[BandSpectrum],
    _conditioning: &[Conditioning],
    _law: ReadoutLaw,
    _axis: &FreqAxis,
) -> Result<ReceiverReadout, SinkError> {
    todo!("GREEN phase")
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
        // C-weighting attenuates the low/high extremes less than A → different totals.
        assert!((ro.total_dba - ro.total_dbc).abs() > 1.0);
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
