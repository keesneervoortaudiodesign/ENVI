//! The OPFS tensor **reader** — the exact inverse of [`opfs_sink`](crate::opfs_sink)
//! `put_chunk`. Phase 10 only WROTE chunk files; Phase 11 must decode them back
//! into arrays for the client-side readout (RESEARCH Pattern 1).
//!
//! # Module I/O
//! - **Input:** the frozen chunk byte pair produced by
//!   [`OpfsChunkSink`](crate::opfs_sink::OpfsChunkSink):
//!   `H_coh` as interleaved `(re, im)` f64 little-endian (16 B/cell),
//!   `P_incoh_abs` as f64 LE (8 B/cell), both in `[s][r_local][f]`
//!   freq-contiguous LOGICAL order (freq fastest — RESEARCH Pitfall 2), plus the
//!   `(n_sub, chunk_len)` dims from the manifest.
//! - **Output:** `(H_coh, P_incoh_abs)` as `[n_sub, chunk_len, N_BANDS]`
//!   [`Array3`]s in `[s][r_local][f]` logical order.
//!
//! # Trust boundary (threat T-11-01-01)
//! The chunk bytes are storage-controlled and untrusted at decode. [`read_chunk`]
//! validates the byte length against `n_sub·chunk_len·N_BANDS·bytes_per_cell` and
//! rejects any non-finite value with a typed [`ChunkDecodeError`] — it NEVER
//! panics/UB on data (mirrors the sink's `put_chunk` validation discipline).
//!
//! # Worker-only glue lands in 11-05 (Pitfall 4)
//! The JS worker-side pre-open-read handles (`createSyncAccessHandle` is
//! dedicated-worker-only) are added in plan 11-05; this module is the pure,
//! natively `cargo test`-able decode core.

use envi_engine::freq::N_BANDS;
use ndarray::Array3;
use num_complex::Complex;

/// Bytes per `H_coh` cell: one `Complex<f64>` as interleaved `(re, im)` f64 LE.
/// Mirrors `opfs_sink::H_BYTES_PER_CELL` — the frozen layout the reader inverts.
const H_BYTES_PER_CELL: usize = 16;
/// Bytes per `P_incoh_abs` cell: one f64 LE. Mirrors `opfs_sink::P_BYTES_PER_CELL`.
const P_BYTES_PER_CELL: usize = 8;

/// A typed decode error for the OPFS chunk trust boundary (threat T-11-01-01).
///
/// The bytes + dims are storage-controlled; every length mismatch or non-finite
/// value yields one of these — [`read_chunk`] never panics on data.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ChunkDecodeError {
    /// A channel's byte length disagrees with `n_sub·chunk_len·N_BANDS·cell`.
    #[error("{channel} chunk has {got} bytes, expected {expected}")]
    LengthMismatch {
        /// Which channel (`"H_coh"` / `"P_incoh_abs"`).
        channel: &'static str,
        /// Expected byte count.
        expected: usize,
        /// Actual byte count.
        got: usize,
    },
    /// A decoded cell was NaN or infinite.
    #[error("non-finite value in {what}")]
    NonFinite {
        /// Which channel carried the non-finite value.
        what: &'static str,
    },
}

/// Decode a frozen OPFS chunk byte pair back into `[n_sub, chunk_len, N_BANDS]`
/// arrays in `[s][r_local][f]` logical order — the exact inverse of
/// [`OpfsChunkSink::put_chunk`](crate::opfs_sink::OpfsChunkSink).
///
/// `hi_bytes` holds `H_coh` in 16-byte `(re, im)` f64-LE strides; `pi_bytes`
/// holds `P_incoh_abs` in 8-byte f64-LE strides. Both are read freq-fastest.
///
/// # Errors
/// [`ChunkDecodeError::LengthMismatch`] if either channel's byte length is not
/// `n_sub·chunk_len·N_BANDS·bytes_per_cell`; [`ChunkDecodeError::NonFinite`] if a
/// decoded cell is NaN or infinite.
pub fn read_chunk(
    hi_bytes: &[u8],
    pi_bytes: &[u8],
    n_sub: usize,
    chunk_len: usize,
) -> Result<(Array3<Complex<f64>>, Array3<f64>), ChunkDecodeError> {
    let cells = n_sub * chunk_len * N_BANDS;
    let want_h = cells * H_BYTES_PER_CELL;
    let want_p = cells * P_BYTES_PER_CELL;
    if hi_bytes.len() != want_h {
        return Err(ChunkDecodeError::LengthMismatch {
            channel: "H_coh",
            expected: want_h,
            got: hi_bytes.len(),
        });
    }
    if pi_bytes.len() != want_p {
        return Err(ChunkDecodeError::LengthMismatch {
            channel: "P_incoh_abs",
            expected: want_p,
            got: pi_bytes.len(),
        });
    }

    let mut h = Array3::<Complex<f64>>::zeros((n_sub, chunk_len, N_BANDS));
    let mut p = Array3::<f64>::zeros((n_sub, chunk_len, N_BANDS));
    let mut hi = 0usize;
    let mut pi = 0usize;
    // Decode in [s][r_local][f] strides (freq fastest) — the frozen logical order
    // the sink serialized in via `.iter()`, never the raw memory slice (Pitfall 2).
    for s in 0..n_sub {
        for r in 0..chunk_len {
            for f in 0..N_BANDS {
                let re = f64::from_le_bytes(hi_bytes[hi..hi + 8].try_into().unwrap());
                let im = f64::from_le_bytes(hi_bytes[hi + 8..hi + 16].try_into().unwrap());
                hi += H_BYTES_PER_CELL;
                if !re.is_finite() || !im.is_finite() {
                    return Err(ChunkDecodeError::NonFinite { what: "H_coh" });
                }
                h[[s, r, f]] = Complex::new(re, im);

                let v = f64::from_le_bytes(pi_bytes[pi..pi + 8].try_into().unwrap());
                pi += P_BYTES_PER_CELL;
                if !v.is_finite() {
                    return Err(ChunkDecodeError::NonFinite {
                        what: "P_incoh_abs",
                    });
                }
                p[[s, r, f]] = v;
            }
        }
    }
    Ok((h, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opfs_sink::{ChunkHandle, OpfsChunkSink, OpfsError};
    use envi_engine::tensor::TensorSink;
    use ndarray::{Array3, s};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A `ChunkHandle` over a shared `Vec<u8>` — the sync-access-handle stand-in,
    /// used to drive the real `OpfsChunkSink` serializer and capture its bytes.
    struct VecHandle {
        buf: Rc<RefCell<Vec<u8>>>,
    }

    impl ChunkHandle for VecHandle {
        fn write_at(&mut self, bytes: &[u8], at: u64) -> Result<(), OpfsError> {
            let at = at as usize;
            let mut b = self.buf.borrow_mut();
            if b.len() < at + bytes.len() {
                b.resize(at + bytes.len(), 0);
            }
            b[at..at + bytes.len()].copy_from_slice(bytes);
            Ok(())
        }
        fn flush(&mut self) -> Result<(), OpfsError> {
            Ok(())
        }
        fn close(&mut self) -> Result<(), OpfsError> {
            Ok(())
        }
    }

    fn boxed(buf: &Rc<RefCell<Vec<u8>>>) -> Box<dyn ChunkHandle> {
        Box::new(VecHandle { buf: buf.clone() })
    }

    fn hval(s: usize, r: usize, f: usize) -> Complex<f64> {
        let k = (s * 100 + r * 10 + f) as f64;
        Complex::from_polar(0.1 + 0.001 * k, 0.017 * k - 0.3)
    }

    fn pval(s: usize, r: usize, f: usize) -> f64 {
        let k = (s * 100 + r * 10 + f) as f64;
        1.0e-6 * (1.0 + k)
    }

    #[test]
    fn read_chunk_round_trips_byte_exact_vs_sink() {
        // Write a known tensor through the frozen sink, then decode the produced
        // bytes and assert index-for-index f64::to_bits equality on both channels.
        let (n_sub, n_rcv) = (2usize, 3usize);
        let full_h = Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| hval(s, r, f));
        let full_p = Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| pval(s, r, f));

        let hbuf = Rc::new(RefCell::new(Vec::new()));
        let pbuf = Rc::new(RefCell::new(Vec::new()));
        let mut sink = OpfsChunkSink::new(boxed(&hbuf), boxed(&pbuf), n_sub, n_rcv);
        sink.put_chunk(0, full_h.view(), full_p.view()).unwrap();
        sink.finish().unwrap();

        let (h, p) = read_chunk(&hbuf.borrow(), &pbuf.borrow(), n_sub, n_rcv).unwrap();
        for si in 0..n_sub {
            for ri in 0..n_rcv {
                for fi in 0..N_BANDS {
                    assert_eq!(
                        h[[si, ri, fi]].re.to_bits(),
                        full_h[[si, ri, fi]].re.to_bits()
                    );
                    assert_eq!(
                        h[[si, ri, fi]].im.to_bits(),
                        full_h[[si, ri, fi]].im.to_bits()
                    );
                    assert_eq!(p[[si, ri, fi]].to_bits(), full_p[[si, ri, fi]].to_bits());
                }
            }
        }
    }

    #[test]
    fn read_chunk_decodes_non_contiguous_sink_slice() {
        // A sliced receiver-axis view is non-standard-layout (Pitfall 7). The sink
        // serializes it in logical order; the reader must decode it identically.
        let full_h = Array3::from_shape_fn((2usize, 5usize, N_BANDS), |(s, r, f)| hval(s, r, f));
        let full_p = Array3::from_shape_fn((2usize, 5usize, N_BANDS), |(s, r, f)| pval(s, r, f));
        let sub_h = full_h.slice(s![.., 1..4, ..]);
        let sub_p = full_p.slice(s![.., 1..4, ..]);

        let hbuf = Rc::new(RefCell::new(Vec::new()));
        let pbuf = Rc::new(RefCell::new(Vec::new()));
        let mut sink = OpfsChunkSink::new(boxed(&hbuf), boxed(&pbuf), 2, 3);
        sink.put_chunk(0, sub_h, sub_p).unwrap();
        sink.finish().unwrap();

        let (h, p) = read_chunk(&hbuf.borrow(), &pbuf.borrow(), 2, 3).unwrap();
        for si in 0..2 {
            for ri in 0..3 {
                for fi in 0..N_BANDS {
                    assert_eq!(
                        h[[si, ri, fi]].re.to_bits(),
                        sub_h[[si, ri, fi]].re.to_bits()
                    );
                    assert_eq!(
                        h[[si, ri, fi]].im.to_bits(),
                        sub_h[[si, ri, fi]].im.to_bits()
                    );
                    assert_eq!(p[[si, ri, fi]].to_bits(), sub_p[[si, ri, fi]].to_bits());
                }
            }
        }
    }

    #[test]
    fn read_chunk_rejects_malformed_length() {
        // H_coh one byte short → typed LengthMismatch, never a panic.
        let cells = N_BANDS;
        let hbuf = vec![0u8; cells * H_BYTES_PER_CELL - 1];
        let pbuf = vec![0u8; cells * P_BYTES_PER_CELL];
        assert!(matches!(
            read_chunk(&hbuf, &pbuf, 1, 1),
            Err(ChunkDecodeError::LengthMismatch {
                channel: "H_coh",
                ..
            })
        ));
        // P_incoh_abs wrong length.
        let hbuf = vec![0u8; cells * H_BYTES_PER_CELL];
        let pbuf = vec![0u8; cells * P_BYTES_PER_CELL + 8];
        assert!(matches!(
            read_chunk(&hbuf, &pbuf, 1, 1),
            Err(ChunkDecodeError::LengthMismatch {
                channel: "P_incoh_abs",
                ..
            })
        ));
    }

    #[test]
    fn read_chunk_rejects_non_finite() {
        let cells = N_BANDS;
        // A NaN in the H_coh real part of cell 0.
        let mut hbuf = vec![0u8; cells * H_BYTES_PER_CELL];
        hbuf[0..8].copy_from_slice(&f64::NAN.to_le_bytes());
        let pbuf = vec![0u8; cells * P_BYTES_PER_CELL];
        assert!(matches!(
            read_chunk(&hbuf, &pbuf, 1, 1),
            Err(ChunkDecodeError::NonFinite { what: "H_coh" })
        ));
        // An infinity in P_incoh_abs.
        let hbuf = vec![0u8; cells * H_BYTES_PER_CELL];
        let mut pbuf = vec![0u8; cells * P_BYTES_PER_CELL];
        pbuf[0..8].copy_from_slice(&f64::INFINITY.to_le_bytes());
        assert!(matches!(
            read_chunk(&hbuf, &pbuf, 1, 1),
            Err(ChunkDecodeError::NonFinite {
                what: "P_incoh_abs"
            })
        ));
    }
}
