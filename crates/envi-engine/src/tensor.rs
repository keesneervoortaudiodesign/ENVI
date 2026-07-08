//! The Phase-4 dense transfer-tensor store and its two readout laws.
//!
//! This module turns the frozen forward contract
//! [`TransferTensor`](crate::transfer::TransferTensor) —
//! `Array3<Complex<f64>>` indexed `[sub_source, receiver, freq]`, row-major so
//! the frequency axis is contiguous — into a real, streamable artifact paired
//! with the incoherent energy channel, plus the trait seam a Milestone-2
//! file-backed store plugs into.
//!
//! # The tensor is a PAIR (two-channel contract)
//!
//! [`TensorPair`] holds `H_coh: Array3<Complex<f64>>` (the phase-preserving
//! coherent transfer) and `P_incoh_abs: Array3<f64>` (the turbulence-decorrelated
//! energy). Per the user-locked two-channel readout law (ENG-07, mirrored from
//! [`transfer::band_levels_db_two_channel`](crate::transfer)) the incoherent
//! store is kept in **absolute** form `|H_ff|²·p_incoh` at fill time, so
//! `F → 1 ⇒ P_incoh → 0` stays bit-exact and readout needs only the two stores.
//!
//! # Two readout laws — never conflate them
//!
//! - [`readout_coherent`] is the OUT-03 MAC `p[r,f] = Σ_s H_coh[s,r,f]·G_s(f)`.
//!   `G_s(f)` is a complex per-band gain composed ONCE (filter × delay ×
//!   `10^{L_W/20}`) so the MAC equals a full recompute *bit-for-bit* — the
//!   conditioning use case (M2 correlated loudspeakers).
//! - [`readout_incoherent`] is the AV 1106/07 Annex-A energy sum
//!   `e[r,f] = Σ_s w_s(f)·(|H_coh|² + P_incoh_abs)`, mandatory for road/FORCE
//!   sources: two identical co-located sub-sources give **+3 dB**, never +6 dB.
//!
//! Conditioning lives on the ENVI-convention (post-`nord_ratio_to_transfer`)
//! side — never inside `propagation/`.
//!
//! # Streaming seam (OUT-06)
//!
//! [`TensorSink::put_chunk`] receives receiver-axis chunks so the solver need
//! never allocate the whole tensor. [`InMemorySink`] materializes the full pair
//! (small cases) and tracks a high-water-mark byte counter; a file-backed sink
//! is a Milestone-2 concern the trait already accommodates.

use ndarray::{Array2, Array3, ArrayView3, Axis, s};
use num_complex::Complex;
use thiserror::Error;

use crate::freq::N_BANDS;

/// Bytes held per `(sub_source, receiver, band)` cell across the pair:
/// 16 B for one `Complex<f64>` (`H_coh`) + 8 B for one `f64` (`P_incoh_abs`).
pub const BYTES_PER_CELL_PAIR: usize = 16 + 8;

/// Errors from the tensor sink and readout trust boundaries.
///
/// Chunk dimensions, gain-vector lengths, and receiver offsets are all
/// caller-controlled (harness → engine, threat T-04-01-02/03); every mismatch
/// or non-finite value yields one of these — the sink and readouts never
/// `unwrap`/panic on data.
#[derive(Debug, Error, PartialEq)]
pub enum SinkError {
    /// The two channels of a chunk disagree in shape.
    #[error("chunk channel shapes disagree: H_coh {h_coh:?} vs P_incoh_abs {p_incoh:?}")]
    ChannelShapeMismatch {
        /// `H_coh` chunk dims.
        h_coh: (usize, usize, usize),
        /// `P_incoh_abs` chunk dims.
        p_incoh: (usize, usize, usize),
    },
    /// A chunk's sub-source count does not match the sink's backing store.
    #[error("chunk has {got} sub-sources but the sink expects {expected}")]
    SubCountMismatch {
        /// Sink sub-source count.
        expected: usize,
        /// Chunk sub-source count.
        got: usize,
    },
    /// A chunk's band count is not [`N_BANDS`].
    #[error("chunk has {got} bands but the grid is {expected}")]
    BandCountMismatch {
        /// Expected band count ([`N_BANDS`]).
        expected: usize,
        /// Chunk band count.
        got: usize,
    },
    /// The chunk's receiver span `[r_offset, r_offset+chunk_len)` runs past the
    /// receiver axis of the backing store.
    #[error("chunk [{r_offset}, {r_offset}+{chunk_len}) exceeds receiver axis {n_receivers}")]
    ChunkOutOfBounds {
        /// Chunk receiver offset.
        r_offset: usize,
        /// Chunk receiver length.
        chunk_len: usize,
        /// Backing-store receiver count.
        n_receivers: usize,
    },
    /// A readout gain vector has the wrong sub-source count.
    #[error("gain has {got} sub-sources but the tensor has {expected}")]
    GainSubCountMismatch {
        /// Tensor sub-source count.
        expected: usize,
        /// Gain sub-source count.
        got: usize,
    },
    /// A per-sub-source readout gain vector has the wrong band count.
    #[error("gain for sub-source {sub} has {got} bands, expected {expected}")]
    GainBandCountMismatch {
        /// Offending sub-source index.
        sub: usize,
        /// Expected band count.
        expected: usize,
        /// Actual band count.
        got: usize,
    },
    /// A caller-supplied value (chunk cell or gain) was NaN or infinite.
    #[error("non-finite value in {what}")]
    NonFinite {
        /// Which input carried the non-finite value.
        what: &'static str,
    },
}

/// The paired dense transfer tensor: coherent complex `H_coh` and the absolute
/// incoherent energy `P_incoh_abs`, both `[sub_source, receiver, freq]`,
/// row-major (frequency-contiguous). Never Fortran-order.
#[derive(Debug, Clone, PartialEq)]
pub struct TensorPair {
    /// Phase-preserving coherent transfer per `(sub_source, receiver, band)`.
    pub h_coh: Array3<Complex<f64>>,
    /// Absolute incoherent energy `|H_ff|²·p_incoh` per cell (real, ≥ 0).
    pub p_incoh_abs: Array3<f64>,
}

impl TensorPair {
    /// A zero-initialized pair of the given `(n_sub, n_rcv, N_BANDS)` shape.
    ///
    /// `ndarray`'s default constructor is row-major, so the frequency axis is
    /// contiguous on the last index (the frozen layout).
    #[must_use]
    pub fn zeros(shape: (usize, usize, usize)) -> Self {
        Self {
            h_coh: Array3::zeros(shape),
            p_incoh_abs: Array3::zeros(shape),
        }
    }

    /// The `(n_sub, n_rcv, n_band)` shape of both stores.
    #[must_use]
    pub fn dim(&self) -> (usize, usize, usize) {
        self.h_coh.dim()
    }
}

/// The streaming seam (OUT-06): the solver hands the sink one receiver-axis
/// chunk at a time so the whole tensor need never be resident.
pub trait TensorSink {
    /// Receive one receiver-axis chunk covering `[r_offset, r_offset+chunk_len)`.
    ///
    /// Both views are `[n_sub, chunk_len, N_BANDS]`.
    ///
    /// # Errors
    ///
    /// [`SinkError`] on any dimension mismatch, out-of-bounds receiver span, or
    /// non-finite cell — never a panic.
    fn put_chunk(
        &mut self,
        r_offset: usize,
        h_coh: ArrayView3<'_, Complex<f64>>,
        p_incoh_abs: ArrayView3<'_, f64>,
    ) -> Result<(), SinkError>;
}

/// A sink that materializes the full pair in memory and tracks the largest
/// single chunk it has been handed (the high-water-mark byte counter).
#[derive(Debug, Clone)]
pub struct InMemorySink {
    pair: TensorPair,
    high_water_bytes: usize,
}

impl InMemorySink {
    /// A sink backing an `(n_sub, n_rcv, N_BANDS)` tensor.
    #[must_use]
    pub fn new(n_sub: usize, n_rcv: usize) -> Self {
        Self {
            pair: TensorPair::zeros((n_sub, n_rcv, N_BANDS)),
            high_water_bytes: 0,
        }
    }

    /// The largest single chunk handed in, in bytes across the pair
    /// (`n_sub · chunk_len · N_BANDS · 24`).
    #[must_use]
    pub fn high_water_bytes(&self) -> usize {
        self.high_water_bytes
    }

    /// Borrow the assembled tensor pair.
    #[must_use]
    pub fn tensor(&self) -> &TensorPair {
        &self.pair
    }

    /// Consume the sink, returning the assembled tensor pair.
    #[must_use]
    pub fn into_tensor(self) -> TensorPair {
        self.pair
    }
}

impl TensorSink for InMemorySink {
    fn put_chunk(
        &mut self,
        r_offset: usize,
        h_coh: ArrayView3<'_, Complex<f64>>,
        p_incoh_abs: ArrayView3<'_, f64>,
    ) -> Result<(), SinkError> {
        // Validate the two channels agree, the band axis is the grid, the
        // sub-source count matches the backing store, and the receiver span is
        // in bounds — all before touching memory (threat T-04-01-02).
        let (hs, hr, hf) = h_coh.dim();
        if p_incoh_abs.dim() != (hs, hr, hf) {
            return Err(SinkError::ChannelShapeMismatch {
                h_coh: (hs, hr, hf),
                p_incoh: p_incoh_abs.dim(),
            });
        }
        if hf != N_BANDS {
            return Err(SinkError::BandCountMismatch {
                expected: N_BANDS,
                got: hf,
            });
        }
        let (n_sub, n_rcv, _) = self.pair.h_coh.dim();
        if hs != n_sub {
            return Err(SinkError::SubCountMismatch {
                expected: n_sub,
                got: hs,
            });
        }
        if r_offset + hr > n_rcv {
            return Err(SinkError::ChunkOutOfBounds {
                r_offset,
                chunk_len: hr,
                n_receivers: n_rcv,
            });
        }
        // Reject non-finite operator-controlled data (never store NaN/∞).
        if h_coh.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
            return Err(SinkError::NonFinite {
                what: "chunk H_coh",
            });
        }
        if p_incoh_abs.iter().any(|v| !v.is_finite()) {
            return Err(SinkError::NonFinite {
                what: "chunk P_incoh_abs",
            });
        }

        // Copy the chunk into its receiver slice of the backing store.
        self.pair
            .h_coh
            .slice_mut(s![.., r_offset..r_offset + hr, ..])
            .assign(&h_coh);
        self.pair
            .p_incoh_abs
            .slice_mut(s![.., r_offset..r_offset + hr, ..])
            .assign(&p_incoh_abs);

        // High-water mark = largest single chunk's byte footprint across the pair.
        let bytes = hs * hr * hf * BYTES_PER_CELL_PAIR;
        self.high_water_bytes = self.high_water_bytes.max(bytes);
        Ok(())
    }
}

/// The coherent MAC readout (OUT-03): `p[r,f] = Σ_s H_coh[s,r,f]·g_s(f)`.
///
/// `g[s]` is the pre-composed complex per-band gain for sub-source `s` (length
/// [`N_BANDS`]); accumulation runs sub-source-outermost, band-contiguous
/// innermost. Composing `g` once and multiplying once is what makes the MAC
/// equal a full recompute bit-for-bit (see [`compose_gain`]).
///
/// # Errors
///
/// [`SinkError`] if `g` has the wrong sub-source or band count, or carries a
/// non-finite value.
pub fn readout_coherent(
    h_coh: ArrayView3<'_, Complex<f64>>,
    g: &[Vec<Complex<f64>>],
) -> Result<Array2<Complex<f64>>, SinkError> {
    let (n_sub, n_rcv, n_f) = h_coh.dim();
    validate_complex_gain(g, n_sub, n_f)?;

    let mut p = Array2::<Complex<f64>>::zeros((n_rcv, n_f));
    // Sub-source outermost, band-contiguous innermost — the single frozen
    // accumulation order that makes the MAC bit-identical to a full recompute.
    for (s, gs) in g.iter().enumerate() {
        let hs = h_coh.index_axis(Axis(0), s);
        for (mut prow, hrow) in p.outer_iter_mut().zip(hs.outer_iter()) {
            for (pf, (hf, gf)) in prow.iter_mut().zip(hrow.iter().zip(gs.iter())) {
                *pf += *hf * *gf;
            }
        }
    }
    Ok(p)
}

/// The incoherent Annex-A energy readout: `e[r,f] = Σ_s w_s(f)·(|H_coh|² +
/// P_incoh_abs)`.
///
/// `w[s]` is the real per-band energy weight for sub-source `s` (e.g.
/// `|G_s|² = 10^{L_W/10}` times a pass-by time weight), length [`N_BANDS`]. The
/// result is an ENERGY; the caller converts to dB via `10·log10(e)`.
///
/// # Errors
///
/// [`SinkError`] if the channels disagree in shape, `w` has the wrong sub-source
/// or band count, or any input is non-finite.
pub fn readout_incoherent(
    h_coh: ArrayView3<'_, Complex<f64>>,
    p_incoh_abs: ArrayView3<'_, f64>,
    w: &[Vec<f64>],
) -> Result<Array2<f64>, SinkError> {
    let (n_sub, n_rcv, n_f) = h_coh.dim();
    if p_incoh_abs.dim() != (n_sub, n_rcv, n_f) {
        return Err(SinkError::ChannelShapeMismatch {
            h_coh: (n_sub, n_rcv, n_f),
            p_incoh: p_incoh_abs.dim(),
        });
    }
    validate_real_gain(w, n_sub, n_f)?;

    let mut e = Array2::<f64>::zeros((n_rcv, n_f));
    // Sub-source outermost, band-contiguous innermost. Energy per cell =
    // |H_coh|² + P_incoh_abs; the real weight w_s(f) carries |G_s|²·(time weight).
    for (s, ws) in w.iter().enumerate() {
        let hs = h_coh.index_axis(Axis(0), s);
        let ps = p_incoh_abs.index_axis(Axis(0), s);
        for ((mut erow, hrow), prow) in e.outer_iter_mut().zip(hs.outer_iter()).zip(ps.outer_iter())
        {
            for (ef, ((hf, pf), wf)) in erow
                .iter_mut()
                .zip(hrow.iter().zip(prow.iter()).zip(ws.iter()))
            {
                *ef += *wf * (hf.norm_sqr() + *pf);
            }
        }
    }
    Ok(e)
}

/// Validate a complex per-sub-source gain against the tensor dimensions and
/// reject non-finite entries.
fn validate_complex_gain(
    g: &[Vec<Complex<f64>>],
    n_sub: usize,
    n_f: usize,
) -> Result<(), SinkError> {
    if g.len() != n_sub {
        return Err(SinkError::GainSubCountMismatch {
            expected: n_sub,
            got: g.len(),
        });
    }
    for (sub, gs) in g.iter().enumerate() {
        if gs.len() != n_f {
            return Err(SinkError::GainBandCountMismatch {
                sub,
                expected: n_f,
                got: gs.len(),
            });
        }
        if gs.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
            return Err(SinkError::NonFinite {
                what: "coherent gain G_s",
            });
        }
    }
    Ok(())
}

/// Validate a real per-sub-source weight against the tensor dimensions and
/// reject non-finite entries.
fn validate_real_gain(w: &[Vec<f64>], n_sub: usize, n_f: usize) -> Result<(), SinkError> {
    if w.len() != n_sub {
        return Err(SinkError::GainSubCountMismatch {
            expected: n_sub,
            got: w.len(),
        });
    }
    for (sub, ws) in w.iter().enumerate() {
        if ws.len() != n_f {
            return Err(SinkError::GainBandCountMismatch {
                sub,
                expected: n_f,
                got: ws.len(),
            });
        }
        if ws.iter().any(|v| !v.is_finite()) {
            return Err(SinkError::NonFinite {
                what: "incoherent weight w_s",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::BandSpectrum;
    use crate::transfer::band_levels_db;
    use approx::assert_relative_eq;

    #[test]
    fn tensor_pair_is_row_major_frequency_contiguous() {
        let t = TensorPair::zeros((2, 3, N_BANDS));
        assert_eq!(t.dim(), (2, 3, N_BANDS));
        assert!(
            t.h_coh.is_standard_layout(),
            "H_coh must be row-major (frequency contiguous)"
        );
        assert!(
            t.p_incoh_abs.is_standard_layout(),
            "P_incoh_abs must be row-major (frequency contiguous)"
        );
    }

    #[test]
    fn incoherent_readout_single_unit_source_matches_band_levels_db() {
        // One sub-source, one receiver; P_incoh_abs all zero. With weight
        // w = |G|² = 10^{L_W/10}, 10·lg(e) must equal band_levels_db(H_coh).
        let lw = 75.0;
        let mut pair = TensorPair::zeros((1, 1, N_BANDS));
        let h: Vec<Complex<f64>> = (0..N_BANDS)
            .map(|i| Complex::from_polar(0.1 + 0.001 * i as f64, 0.21 * i as f64))
            .collect();
        for (i, &z) in h.iter().enumerate() {
            pair.h_coh[[0, 0, i]] = z;
        }
        let w = vec![vec![10f64.powf(lw / 10.0); N_BANDS]];
        let e = readout_incoherent(pair.h_coh.view(), pair.p_incoh_abs.view(), &w).unwrap();
        let spectrum = BandSpectrum::uniform(lw);
        let want = band_levels_db(&h, &spectrum);
        for i in 0..N_BANDS {
            assert_relative_eq!(10.0 * e[[0, i]].log10(), want[i], epsilon = 1e-9);
        }
    }

    #[test]
    fn two_identical_incoherent_sources_add_three_db_not_six() {
        // Anti-phantom-interference guard (Pitfall 3): incoherent sum of two
        // identical co-located sub-sources is +10·lg 2 = +3.0103 dB, NOT +6 dB.
        let h: Vec<Complex<f64>> = (0..N_BANDS)
            .map(|i| Complex::from_polar(0.2, -0.13 * i as f64))
            .collect();
        let mut one = TensorPair::zeros((1, 1, N_BANDS));
        let mut two = TensorPair::zeros((2, 1, N_BANDS));
        for (i, &z) in h.iter().enumerate() {
            one.h_coh[[0, 0, i]] = z;
            two.h_coh[[0, 0, i]] = z;
            two.h_coh[[1, 0, i]] = z;
        }
        let w1 = vec![vec![1.0; N_BANDS]];
        let w2 = vec![vec![1.0; N_BANDS], vec![1.0; N_BANDS]];
        let e1 = readout_incoherent(one.h_coh.view(), one.p_incoh_abs.view(), &w1).unwrap();
        let e2 = readout_incoherent(two.h_coh.view(), two.p_incoh_abs.view(), &w2).unwrap();
        for i in 0..N_BANDS {
            let delta_db = 10.0 * (e2[[0, i]] / e1[[0, i]]).log10();
            assert_relative_eq!(delta_db, 10.0 * 2f64.log10(), epsilon = 1e-12);
            // Explicitly NOT +6 dB.
            assert!((delta_db - 6.0206).abs() > 1.0);
        }
    }

    #[test]
    fn coherent_readout_single_source_matches_band_levels_db() {
        // With a real gain g = 10^{L_W/20} the coherent MAC reproduces the
        // single-source seed band_levels_db (magnitude, any phase).
        let lw = 80.0;
        let mut pair = TensorPair::zeros((1, 1, N_BANDS));
        let h: Vec<Complex<f64>> = (0..N_BANDS)
            .map(|i| Complex::from_polar(0.1, 0.37 * i as f64))
            .collect();
        for (i, &z) in h.iter().enumerate() {
            pair.h_coh[[0, 0, i]] = z;
        }
        let g = vec![
            (0..N_BANDS)
                .map(|_| Complex::new(10f64.powf(lw / 20.0), 0.0))
                .collect::<Vec<_>>(),
        ];
        let p = readout_coherent(pair.h_coh.view(), &g).unwrap();
        let spectrum = BandSpectrum::uniform(lw);
        let want = band_levels_db(&h, &spectrum);
        for i in 0..N_BANDS {
            assert_relative_eq!(20.0 * p[[0, i]].norm().log10(), want[i], epsilon = 1e-9);
        }
    }

    #[test]
    fn in_memory_sink_high_water_matches_analytic_chunk_bytes() {
        let n_sub = 3;
        let n_rcv = 5;
        let mut sink = InMemorySink::new(n_sub, n_rcv);
        // Two chunks of receiver lengths 2 then 3.
        for &(r_off, len) in &[(0usize, 2usize), (2, 3)] {
            let hc =
                Array3::<Complex<f64>>::from_elem((n_sub, len, N_BANDS), Complex::new(1.0, 0.0));
            let pi = Array3::<f64>::zeros((n_sub, len, N_BANDS));
            sink.put_chunk(r_off, hc.view(), pi.view()).unwrap();
        }
        // High-water = largest single chunk = n_sub · 3 · N_BANDS · 24.
        let want = n_sub * 3 * N_BANDS * BYTES_PER_CELL_PAIR;
        assert_eq!(sink.high_water_bytes(), want);
    }

    #[test]
    fn in_memory_sink_rejects_mismatched_chunk_without_panic() {
        let mut sink = InMemorySink::new(3, 4);
        // Wrong sub-source count (2 instead of 3).
        let hc = Array3::<Complex<f64>>::zeros((2, 2, N_BANDS));
        let pi = Array3::<f64>::zeros((2, 2, N_BANDS));
        assert!(matches!(
            sink.put_chunk(0, hc.view(), pi.view()),
            Err(SinkError::SubCountMismatch { .. })
        ));
        // Out-of-bounds receiver span.
        let hc = Array3::<Complex<f64>>::zeros((3, 3, N_BANDS));
        let pi = Array3::<f64>::zeros((3, 3, N_BANDS));
        assert!(matches!(
            sink.put_chunk(2, hc.view(), pi.view()),
            Err(SinkError::ChunkOutOfBounds { .. })
        ));
        // Channel-shape disagreement.
        let hc = Array3::<Complex<f64>>::zeros((3, 2, N_BANDS));
        let pi = Array3::<f64>::zeros((3, 3, N_BANDS));
        assert!(matches!(
            sink.put_chunk(0, hc.view(), pi.view()),
            Err(SinkError::ChannelShapeMismatch { .. })
        ));
    }
}
