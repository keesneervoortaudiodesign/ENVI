//! The OPFS-backed [`TensorSink`] — a NEW impl of the engine's existing streaming
//! trait, writing receiver-axis chunks to `FileSystemSyncAccessHandle` files
//! (D-08). No engine change (D-02): this is a fresh trait impl in the boundary
//! crate, never an edit to `envi-engine`.
//!
//! # Module I/O
//! - **Input:** receiver-axis chunk pairs `(H_coh: ArrayView3<Complex<f64>>,
//!   P_incoh_abs: ArrayView3<f64>)` from `envi_engine::solver::solve` via
//!   [`TensorSink::put_chunk`]. Both views are `[n_sub, chunk_len, N_BANDS]`.
//! - **Output:** the frozen chunk byte format written through a [`ChunkHandle`]
//!   seam — `H_coh` as interleaved `(re, im)` f64 little-endian (16 B/cell),
//!   `P_incoh_abs` as f64 LE (8 B/cell), in `[s][r_local][f]` freq-contiguous
//!   logical order, to a parallel `tensor/` + `pincoh/` file pair.
//! - **Trust boundary (threat T-10-03-02):** the chunk dims + values are
//!   caller-controlled; [`OpfsChunkSink::put_chunk`] replicates `InMemorySink`'s
//!   validation gates and returns the matching [`SinkError`] — it NEVER
//!   `unwrap`s/panics on data.
//! - **Byte-format integrity (threat T-10-03-03):** the frozen layout is proven by
//!   a native round-trip test that reads a written chunk back and asserts
//!   index-for-index equality against `InMemorySink` (Pitfall 7 — a sliced,
//!   non-standard-layout view serializes correctly because `put_chunk` iterates the
//!   view in **logical** order, never the raw memory slice).
//!
//! # Worker-only (Pitfall 4)
//! The real [`ChunkHandle`] wraps an OPFS `FileSystemSyncAccessHandle`, whose
//! `createSyncAccessHandle()` is dedicated-worker-only — all sink I/O runs inside
//! the compute worker's rayon tasks (plan 10-04). Disjoint chunk ranges → disjoint
//! files → one exclusive-lock handle per file (Pitfall 5).
//!
//! # I/O errors vs [`SinkError`] (interface note)
//! `SinkError` (the engine's fixed contract) has no I/O variant, and the engine is
//! unchanged (D-02). So a write/flush/close failure is captured as an
//! [`OpfsError`] and surfaced by [`OpfsChunkSink::finish`] — `put_chunk` stays
//! within the engine's `SinkError` contract for input validation and never panics.

use envi_engine::freq::N_BANDS;
use envi_engine::tensor::{SinkError, TensorSink};
use ndarray::ArrayView3;
use num_complex::Complex;

/// Bytes per `H_coh` cell: one `Complex<f64>` as interleaved `(re, im)` f64 LE.
const H_BYTES_PER_CELL: usize = 16;
/// Bytes per `P_incoh_abs` cell: one f64 LE.
const P_BYTES_PER_CELL: usize = 8;

/// A byte-addressable write target for one chunk file — the sink's I/O seam. The
/// real impl (wasm) wraps an OPFS `FileSystemSyncAccessHandle`; the native tests
/// use an in-memory `Vec<u8>` stand-in. Offsets are byte positions in the file.
pub trait ChunkHandle {
    /// Write `bytes` at byte offset `at`.
    ///
    /// # Errors
    /// [`OpfsError::Write`] on an underlying handle failure.
    fn write_at(&mut self, bytes: &[u8], at: u64) -> Result<(), OpfsError>;
    /// Commit buffered writes to storage.
    ///
    /// # Errors
    /// [`OpfsError::Flush`] on an underlying handle failure.
    fn flush(&mut self) -> Result<(), OpfsError>;
    /// Release the exclusive lock (final).
    ///
    /// # Errors
    /// [`OpfsError::Close`] on an underlying handle failure.
    fn close(&mut self) -> Result<(), OpfsError>;
}

/// An OPFS chunk-file I/O error (distinct from the engine's `SinkError`, which has
/// no I/O variant — the engine stays unchanged, D-02). Surfaced by
/// [`OpfsChunkSink::finish`].
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum OpfsError {
    /// A chunk `write` failed.
    #[error("OPFS chunk write failed: {0}")]
    Write(String),
    /// A chunk `flush` failed.
    #[error("OPFS chunk flush failed: {0}")]
    Flush(String),
    /// A chunk `close` failed.
    #[error("OPFS chunk close failed: {0}")]
    Close(String),
}

/// An OPFS-backed [`TensorSink`] over a parallel `H_coh` + `P_incoh_abs` chunk-file
/// handle pair. It validates each chunk exactly like `InMemorySink`, serializes the
/// frozen `[s][r_local][f]` layout, and appends to the two files at advancing byte
/// offsets. Flush + close happen on [`finish`](Self::finish) (and best-effort on
/// `Drop`). The handles are boxed [`ChunkHandle`] trait objects so the same logic
/// drives the wasm OPFS handle and the native test stand-in.
pub struct OpfsChunkSink {
    tensor: Box<dyn ChunkHandle>,
    pincoh: Box<dyn ChunkHandle>,
    n_sub: usize,
    n_rcv: usize,
    h_off: u64,
    p_off: u64,
    /// First I/O error seen (writes short-circuit once set; surfaced by `finish`).
    io_err: Option<OpfsError>,
    closed: bool,
}

impl OpfsChunkSink {
    /// A sink over the `H_coh` (`tensor`) + `P_incoh_abs` (`pincoh`) handle pair,
    /// backing an `(n_sub, n_rcv, N_BANDS)` receiver span.
    #[must_use]
    pub fn new(
        tensor: Box<dyn ChunkHandle>,
        pincoh: Box<dyn ChunkHandle>,
        n_sub: usize,
        n_rcv: usize,
    ) -> Self {
        Self {
            tensor,
            pincoh,
            n_sub,
            n_rcv,
            h_off: 0,
            p_off: 0,
            io_err: None,
            closed: false,
        }
    }

    /// Flush + close both handles and surface the first I/O error (idempotent). The
    /// pool driver (10-04) calls this before marking a tier complete.
    ///
    /// # Errors
    /// The first [`OpfsError`] from any write/flush/close in this sink's lifetime.
    pub fn finish(&mut self) -> Result<(), OpfsError> {
        if !self.closed {
            self.closed = true;
            // Flush then close both handles; `get_or_insert` keeps the FIRST error
            // (a prior write error takes precedence over a later flush/close error).
            let steps = [
                self.tensor.flush(),
                self.pincoh.flush(),
                self.tensor.close(),
                self.pincoh.close(),
            ];
            for step in steps {
                if let Err(e) = step {
                    let _ = self.io_err.get_or_insert(e);
                }
            }
        }
        self.io_err.take().map_or(Ok(()), Err)
    }
}

impl Drop for OpfsChunkSink {
    fn drop(&mut self) {
        // Best-effort flush+close so a dropped sink does not leak the OPFS lock.
        let _ = self.finish();
    }
}

impl TensorSink for OpfsChunkSink {
    fn put_chunk(
        &mut self,
        r_offset: usize,
        h_coh: ArrayView3<'_, Complex<f64>>,
        p_incoh_abs: ArrayView3<'_, f64>,
    ) -> Result<(), SinkError> {
        // --- Validation gates (copied from InMemorySink, tensor.rs:228-268) ---
        // Same order + variants, so a malformed marshalled chunk yields the exact
        // typed SinkError and never panics (threat T-10-03-02).
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
        if hs != self.n_sub {
            return Err(SinkError::SubCountMismatch {
                expected: self.n_sub,
                got: hs,
            });
        }
        if r_offset + hr > self.n_rcv {
            return Err(SinkError::ChunkOutOfBounds {
                r_offset,
                chunk_len: hr,
                n_receivers: self.n_rcv,
            });
        }
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

        // --- Serialize the frozen [s][r_local][f] freq-contiguous layout ---
        // `.iter()` visits elements in LOGICAL order (rightmost/freq fastest),
        // regardless of the view's memory layout, so a sliced non-contiguous chunk
        // serializes correctly (Pitfall 7) — never grab the raw memory slice.
        let mut hbuf = Vec::with_capacity(h_coh.len() * H_BYTES_PER_CELL);
        for z in h_coh.iter() {
            hbuf.extend_from_slice(&z.re.to_le_bytes());
            hbuf.extend_from_slice(&z.im.to_le_bytes());
        }
        let mut pbuf = Vec::with_capacity(p_incoh_abs.len() * P_BYTES_PER_CELL);
        for v in p_incoh_abs.iter() {
            pbuf.extend_from_slice(&v.to_le_bytes());
        }

        // Append each buffer to its file at the advancing offset. Capture (not panic
        // on) an I/O failure; once errored, subsequent writes short-circuit and
        // `finish` surfaces it — put_chunk stays within the engine's SinkError
        // contract and only advances an offset on a successful write.
        if self.io_err.is_none() {
            match self.tensor.write_at(&hbuf, self.h_off) {
                Ok(()) => self.h_off += hbuf.len() as u64,
                Err(e) => self.io_err = Some(e),
            }
        }
        if self.io_err.is_none() {
            match self.pincoh.write_at(&pbuf, self.p_off) {
                Ok(()) => self.p_off += pbuf.len() as u64,
                Err(e) => self.io_err = Some(e),
            }
        }
        Ok(())
    }
}

// --- Real OPFS handle (wasm-only; JS glue authored in web/src/compute/opfs.ts,
// plan 10-04) --------------------------------------------------------------------
//
// The extern signatures the sink calls; `createSyncAccessHandle` is worker-only
// (Pitfall 4). The `module` is a BARE specifier (not a `/`-path, which wasm-bindgen
// would try to read at COMPILE time): the Vite bundler resolves it at bundle time
// via a `resolve.alias` (`envi-compute-opfs` → `./src/compute/opfs.ts`) added in
// plan 10-04, so the wasm crate compiles before the TS glue exists.
#[cfg(target_arch = "wasm32")]
mod opfs_handle {
    use super::{ChunkHandle, OpfsChunkSink, OpfsError};
    use wasm_bindgen::JsValue;
    use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen(module = "envi-compute-opfs")]
    extern "C" {
        #[wasm_bindgen(js_name = "openChunk", catch)]
        fn open_chunk(path: &str) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_name = "writeChunk", catch)]
        fn write_chunk(handle: &JsValue, bytes: &[u8], at: f64) -> Result<(), JsValue>;
        #[wasm_bindgen(js_name = "flushChunk", catch)]
        fn flush_chunk(handle: &JsValue) -> Result<(), JsValue>;
        #[wasm_bindgen(js_name = "closeChunk", catch)]
        fn close_chunk(handle: &JsValue) -> Result<(), JsValue>;
    }

    /// Stringify a `JsValue` error for an [`OpfsError`] message.
    fn js_msg(e: &JsValue) -> String {
        e.as_string()
            .unwrap_or_else(|| "OPFS handle error".to_string())
    }

    /// A `ChunkHandle` over an OPFS `FileSystemSyncAccessHandle` (worker-only).
    pub struct OpfsSyncHandle {
        handle: JsValue,
    }

    impl OpfsSyncHandle {
        /// Open (exclusive-lock) the chunk file at `path` in the worker.
        ///
        /// # Errors
        /// [`OpfsError::Write`] if the sync-access handle cannot be opened.
        pub fn open(path: &str) -> Result<Self, OpfsError> {
            open_chunk(path)
                .map(|handle| Self { handle })
                .map_err(|e| OpfsError::Write(js_msg(&e)))
        }
    }

    impl ChunkHandle for OpfsSyncHandle {
        fn write_at(&mut self, bytes: &[u8], at: u64) -> Result<(), OpfsError> {
            write_chunk(&self.handle, bytes, at as f64).map_err(|e| OpfsError::Write(js_msg(&e)))
        }
        fn flush(&mut self) -> Result<(), OpfsError> {
            flush_chunk(&self.handle).map_err(|e| OpfsError::Flush(js_msg(&e)))
        }
        fn close(&mut self) -> Result<(), OpfsError> {
            close_chunk(&self.handle).map_err(|e| OpfsError::Close(js_msg(&e)))
        }
    }

    impl OpfsChunkSink {
        /// Open an OPFS-backed sink over the `tensor_path` + `pincoh_path` chunk
        /// files (worker-only). The 10-04 pool driver calls this per disjoint range.
        ///
        /// # Errors
        /// [`OpfsError`] if either exclusive-lock handle cannot be opened.
        pub fn open_opfs(
            tensor_path: &str,
            pincoh_path: &str,
            n_sub: usize,
            n_rcv: usize,
        ) -> Result<Self, OpfsError> {
            let tensor = OpfsSyncHandle::open(tensor_path)?;
            let pincoh = OpfsSyncHandle::open(pincoh_path)?;
            Ok(Self::new(Box::new(tensor), Box::new(pincoh), n_sub, n_rcv))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::tensor::InMemorySink;
    use ndarray::{Array3, s};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A `ChunkHandle` over a shared `Vec<u8>` (the sync-access-handle stand-in).
    /// The test keeps a clone of the `Rc<RefCell<..>>` to read the bytes back.
    struct VecHandle {
        buf: Rc<RefCell<Vec<u8>>>,
        fail_writes: bool,
    }

    impl VecHandle {
        fn new(buf: Rc<RefCell<Vec<u8>>>) -> Self {
            Self {
                buf,
                fail_writes: false,
            }
        }
        fn failing() -> Self {
            Self {
                buf: Rc::new(RefCell::new(Vec::new())),
                fail_writes: true,
            }
        }
    }

    impl ChunkHandle for VecHandle {
        fn write_at(&mut self, bytes: &[u8], at: u64) -> Result<(), OpfsError> {
            if self.fail_writes {
                return Err(OpfsError::Write("mock write failure".to_string()));
            }
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

    /// A deterministic non-trivial complex value for cell `(s, r, f)`.
    fn hval(s: usize, r: usize, f: usize) -> Complex<f64> {
        let k = (s * 100 + r * 10 + f) as f64;
        Complex::from_polar(0.1 + 0.001 * k, 0.017 * k - 0.3)
    }

    /// A deterministic non-negative energy for cell `(s, r, f)`.
    fn pval(s: usize, r: usize, f: usize) -> f64 {
        let k = (s * 100 + r * 10 + f) as f64;
        1.0e-6 * (1.0 + k)
    }

    /// Wrap a shared buffer as a boxed `ChunkHandle`.
    fn boxed(buf: &Rc<RefCell<Vec<u8>>>) -> Box<dyn ChunkHandle> {
        Box::new(VecHandle::new(buf.clone()))
    }

    /// Read a chunk of shape `[n_sub, len, N_BANDS]` back from the frozen byte
    /// format into `(H_coh, P_incoh_abs)` arrays in `[s][r_local][f]` order.
    fn read_chunk(
        hbuf: &[u8],
        pbuf: &[u8],
        n_sub: usize,
        len: usize,
    ) -> (Array3<Complex<f64>>, Array3<f64>) {
        let mut h = Array3::<Complex<f64>>::zeros((n_sub, len, N_BANDS));
        let mut p = Array3::<f64>::zeros((n_sub, len, N_BANDS));
        let mut hi = 0usize;
        let mut pi = 0usize;
        for s in 0..n_sub {
            for r in 0..len {
                for f in 0..N_BANDS {
                    let re = f64::from_le_bytes(hbuf[hi..hi + 8].try_into().unwrap());
                    let im = f64::from_le_bytes(hbuf[hi + 8..hi + 16].try_into().unwrap());
                    hi += 16;
                    h[[s, r, f]] = Complex::new(re, im);
                    let v = f64::from_le_bytes(pbuf[pi..pi + 8].try_into().unwrap());
                    pi += 8;
                    p[[s, r, f]] = v;
                }
            }
        }
        (h, p)
    }

    /// Assert an OPFS-written chunk equals the `InMemorySink` tensor slice bit-for
    /// bit, index-for-index over `[s][r][f]`.
    fn assert_bit_equal(
        h_read: &Array3<Complex<f64>>,
        p_read: &Array3<f64>,
        want: &envi_engine::tensor::TensorPair,
        n_sub: usize,
        n_rcv: usize,
    ) {
        for si in 0..n_sub {
            for ri in 0..n_rcv {
                for fi in 0..N_BANDS {
                    assert_eq!(
                        h_read[[si, ri, fi]].re.to_bits(),
                        want.h_coh[[si, ri, fi]].re.to_bits()
                    );
                    assert_eq!(
                        h_read[[si, ri, fi]].im.to_bits(),
                        want.h_coh[[si, ri, fi]].im.to_bits()
                    );
                    assert_eq!(
                        p_read[[si, ri, fi]].to_bits(),
                        want.p_incoh_abs[[si, ri, fi]].to_bits()
                    );
                }
            }
        }
    }

    #[test]
    fn opfs_sink_round_trips_byte_exact_vs_in_memory() {
        let (n_sub, n_rcv) = (2usize, 3usize);
        let full_h = Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| hval(s, r, f));
        let full_p = Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| pval(s, r, f));

        // Reference: the engine's InMemorySink.
        let mut mem = InMemorySink::new(n_sub, n_rcv);
        mem.put_chunk(0, full_h.view(), full_p.view()).unwrap();

        // OPFS mock over shared Vec<u8> buffers.
        let hbuf = Rc::new(RefCell::new(Vec::new()));
        let pbuf = Rc::new(RefCell::new(Vec::new()));
        let mut sink = OpfsChunkSink::new(boxed(&hbuf), boxed(&pbuf), n_sub, n_rcv);
        sink.put_chunk(0, full_h.view(), full_p.view()).unwrap();
        sink.finish().unwrap();

        let (h_read, p_read) = read_chunk(&hbuf.borrow(), &pbuf.borrow(), n_sub, n_rcv);
        assert_bit_equal(&h_read, &p_read, mem.tensor(), n_sub, n_rcv);
    }

    #[test]
    fn opfs_sink_serializes_non_contiguous_slice_correctly() {
        // A receiver-axis slice is NOT standard-layout — the Pitfall-7 case. It must
        // still serialize in logical [s][r_local][f] order (via .iter()).
        let full_h = Array3::from_shape_fn((2usize, 5usize, N_BANDS), |(s, r, f)| hval(s, r, f));
        let full_p = Array3::from_shape_fn((2usize, 5usize, N_BANDS), |(s, r, f)| pval(s, r, f));
        let sub_h = full_h.slice(s![.., 1..4, ..]);
        let sub_p = full_p.slice(s![.., 1..4, ..]);
        assert!(
            !sub_h.is_standard_layout(),
            "the sliced receiver-axis view must be non-standard-layout to exercise Pitfall 7"
        );

        let mut mem = InMemorySink::new(2, 3);
        mem.put_chunk(0, sub_h, sub_p).unwrap();

        let hbuf = Rc::new(RefCell::new(Vec::new()));
        let pbuf = Rc::new(RefCell::new(Vec::new()));
        let mut sink = OpfsChunkSink::new(boxed(&hbuf), boxed(&pbuf), 2, 3);
        sink.put_chunk(0, sub_h, sub_p).unwrap();
        sink.finish().unwrap();

        let (h_read, p_read) = read_chunk(&hbuf.borrow(), &pbuf.borrow(), 2, 3);
        assert_bit_equal(&h_read, &p_read, mem.tensor(), 2, 3);
    }

    fn fresh_sink(n_sub: usize, n_rcv: usize) -> OpfsChunkSink {
        OpfsChunkSink::new(
            Box::new(VecHandle::new(Rc::new(RefCell::new(Vec::new())))),
            Box::new(VecHandle::new(Rc::new(RefCell::new(Vec::new())))),
            n_sub,
            n_rcv,
        )
    }

    #[test]
    fn opfs_sink_rejects_channel_shape_mismatch() {
        let mut sink = fresh_sink(3, 4);
        let hc = Array3::<Complex<f64>>::zeros((3, 2, N_BANDS));
        let pi = Array3::<f64>::zeros((3, 3, N_BANDS));
        assert!(matches!(
            sink.put_chunk(0, hc.view(), pi.view()),
            Err(SinkError::ChannelShapeMismatch { .. })
        ));
    }

    #[test]
    fn opfs_sink_rejects_wrong_band_count() {
        let mut sink = fresh_sink(3, 4);
        let hc = Array3::<Complex<f64>>::zeros((3, 2, N_BANDS - 1));
        let pi = Array3::<f64>::zeros((3, 2, N_BANDS - 1));
        assert!(matches!(
            sink.put_chunk(0, hc.view(), pi.view()),
            Err(SinkError::BandCountMismatch { .. })
        ));
    }

    #[test]
    fn opfs_sink_rejects_wrong_sub_count() {
        let mut sink = fresh_sink(3, 4);
        let hc = Array3::<Complex<f64>>::zeros((2, 2, N_BANDS));
        let pi = Array3::<f64>::zeros((2, 2, N_BANDS));
        assert!(matches!(
            sink.put_chunk(0, hc.view(), pi.view()),
            Err(SinkError::SubCountMismatch { .. })
        ));
    }

    #[test]
    fn opfs_sink_rejects_out_of_bounds_span() {
        let mut sink = fresh_sink(3, 4);
        let hc = Array3::<Complex<f64>>::zeros((3, 3, N_BANDS));
        let pi = Array3::<f64>::zeros((3, 3, N_BANDS));
        // r_offset 2 + len 3 = 5 > n_rcv 4.
        assert!(matches!(
            sink.put_chunk(2, hc.view(), pi.view()),
            Err(SinkError::ChunkOutOfBounds { .. })
        ));
    }

    #[test]
    fn opfs_sink_rejects_non_finite_cells() {
        let mut sink = fresh_sink(3, 4);
        let mut hc = Array3::<Complex<f64>>::zeros((3, 2, N_BANDS));
        hc[[0, 0, 0]] = Complex::new(f64::NAN, 0.0);
        let pi = Array3::<f64>::zeros((3, 2, N_BANDS));
        assert!(matches!(
            sink.put_chunk(0, hc.view(), pi.view()),
            Err(SinkError::NonFinite { .. })
        ));
        // And a non-finite P_incoh cell (with finite H_coh).
        let mut sink = fresh_sink(3, 4);
        let hc = Array3::<Complex<f64>>::zeros((3, 2, N_BANDS));
        let mut pi = Array3::<f64>::zeros((3, 2, N_BANDS));
        pi[[1, 1, 4]] = f64::INFINITY;
        assert!(matches!(
            sink.put_chunk(0, hc.view(), pi.view()),
            Err(SinkError::NonFinite { .. })
        ));
    }

    #[test]
    fn opfs_sink_surfaces_io_error_at_finish() {
        // A write failure is captured (put_chunk stays Ok — the validation contract)
        // and surfaced by finish(), never panicking.
        let mut sink = OpfsChunkSink::new(
            Box::new(VecHandle::failing()),
            Box::new(VecHandle::failing()),
            1,
            1,
        );
        let hc = Array3::<Complex<f64>>::zeros((1, 1, N_BANDS));
        let pi = Array3::<f64>::zeros((1, 1, N_BANDS));
        assert!(sink.put_chunk(0, hc.view(), pi.view()).is_ok());
        assert!(matches!(sink.finish(), Err(OpfsError::Write(_))));
    }
}
