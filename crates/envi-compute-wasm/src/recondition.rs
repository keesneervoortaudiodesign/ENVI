//! The hash-gated recondition MAC boundary (SVC-06 / D-01 / D-12, 11-03) — the
//! flagship client-side fast-recalc backend.
//!
//! # What it does
//! Given the OPFS-read tensor (11-01 `read_chunk`), a per-source conditioning
//! drive, and the tensor identity the client believes it is reconditioning, it
//! re-mints the CURRENT tensor identity, **refuses** a mismatched hash with a
//! typed [`ComputeError::HashMismatch`] (never a silently-served stale readout —
//! the honest client-side 409, D-12 / Open Q1), and on a match composes each
//! source's gain/filter/delay via the engine `compose_gain` and drives the
//! FORCE-validated [`readout_receiver`] core (11-01) over the reused tensor —
//! producing reconditioned receiver spectra with **no re-propagation**.
//!
//! # Conditioning never stales (D-07)
//! Conditioning is a READOUT parameter, structurally excluded from tensor identity
//! (Phase-6 D-07), so a conditioning edit never changes the tensor hash — the
//! same reused tensor answers every recondition. The gate only refuses when the
//! SCENE (geometry/met/receivers) changed, which is the honest 409/recompute case.
//!
//! # Boundary discipline (mirror of `lib.rs`)
//! The `#[wasm_bindgen]` [`recondition`] entry does only marshalling: re-mint the
//! identity from the passed CURRENT scene ([`crate::identity::marshalled_tensor_hash`]),
//! decode the OPFS chunk bytes, drive the typed [`recondition_receivers`] core, and
//! serialize back. All validation is a typed [`ComputeError`], never a panic
//! (T-11-03-02). The worker-side OPFS pre-open-read glue lands in 11-05.

use envi_engine::freq::{FreqAxis, N_BANDS};
use envi_engine::scene::BandSpectrum;
use ndarray::ArrayView3;
use num_complex::Complex;
use wasm_bindgen::prelude::*;

use envi_compute::readout::{Conditioning, ConditioningDto, default_law, readout_receiver};

use crate::dto::{PrepareSolveReq, ReconditionReq, ReconditionResult};
use crate::opfs_reader::read_chunk;
use crate::{ComputeError, compute_err, from_js, js_err, to_js};

/// Re-mint the tensor identity from the CURRENT scene, gate the request's claimed
/// hash against it, and — on a match — drive the 11-01 readout core over the
/// decoded tensor to produce reconditioned per-receiver spectra.
///
/// The typed, natively `cargo test`-able core of [`recondition`] (no `JsValue`).
///
/// # Errors
/// [`ComputeError::HashMismatch`] if `req.tensor_hash != current_tensor_hash`
/// (refused BEFORE any MAC — no spectra produced); [`ComputeError::Recondition`]
/// on a sub-source/receiver count mismatch, a non-dense/non-finite filter (V5), or
/// an engine readout `SinkError`.
pub fn recondition_receivers(
    req: &ReconditionReq,
    current_tensor_hash: &str,
    h_coh: ArrayView3<'_, Complex<f64>>,
    p_incoh_abs: ArrayView3<'_, f64>,
    axis: &FreqAxis,
) -> Result<ReconditionResult, ComputeError> {
    // The hash gate FIRST (D-12 / Open Q1): refuse a mismatched hash BEFORE any MAC
    // so a stale readout is never produced — the honest client-side 409. The
    // {expected, got} fields mirror the server 409 body.
    if req.tensor_hash != current_tensor_hash {
        return Err(ComputeError::HashMismatch {
            expected: current_tensor_hash.to_string(),
            got: req.tensor_hash.clone(),
        });
    }

    let (n_sub, n_rcv, _) = h_coh.dim();
    if req.per_source_conditioning.len() != n_sub {
        return Err(ComputeError::Recondition(format!(
            "per_source_conditioning has {} entries, expected one per sub-source (n_sub = {n_sub})",
            req.per_source_conditioning.len()
        )));
    }
    if req.receiver_ids.len() != n_rcv {
        return Err(ComputeError::Recondition(format!(
            "receiver_ids has {} entries, expected the chunk receiver count ({n_rcv})",
            req.receiver_ids.len()
        )));
    }

    // Build the per-source engine readout inputs, validating each filter's dense
    // [105] length + finiteness (V5) up front — a typed error, never a panic.
    let mut spectra = Vec::with_capacity(n_sub);
    let mut conditioning = Vec::with_capacity(n_sub);
    for c in &req.per_source_conditioning {
        let (l_w, cond) = to_engine(c)?;
        spectra.push(l_w);
        conditioning.push(cond);
    }

    // The readout law is derived from the conditioning composition (Open Q2): a
    // conditioned/directional drive sums coherently (loudspeaker-array MAC), a
    // plain no-op drive sums incoherently — so a default recondition reads out
    // identically to the plain 11-01 readout (the never-stale invariant, D-07).
    let law = default_law(&conditioning);

    // Read out every receiver in receiver_ids order (index = tensor column) by
    // driving the FORCE-validated 11-01 readout core (compose_gain + readout_coherent
    // internally) — never a bespoke MAC/dB loop here.
    let mut out = Vec::with_capacity(n_rcv);
    for r in 0..n_rcv {
        let ro = readout_receiver(h_coh, p_incoh_abs, r, &spectra, &conditioning, law, axis)
            .map_err(|e| ComputeError::Recondition(e.to_string()))?;
        out.push(ro.band_levels_db);
    }

    // `stale` is always false: a mismatch is refused above (never served stale), and
    // conditioning is excluded from tensor identity so a match can never be stale.
    Ok(ReconditionResult {
        spectra: out,
        stale: false,
    })
}

/// Map a wire [`ConditioningDto`] to the engine readout inputs: a broadband
/// `L_W` [`BandSpectrum`] (the `gain_db`) and a [`Conditioning`] carrying the
/// complex per-band filter + the delay in seconds.
///
/// A muted source is silenced with an all-zero complex filter (an exact `0` gain);
/// a present filter is validated dense `[105]` + finite (V5) and converted from dB
/// magnitude to a real complex gain `10^{dB/20}`. `gain_db`/`delay_ms` are
/// finiteness-checked. A typed [`ComputeError::Recondition`], never a panic.
fn to_engine(c: &ConditioningDto) -> Result<(BandSpectrum, Conditioning), ComputeError> {
    if !c.gain_db.is_finite() {
        return Err(ComputeError::Recondition(format!(
            "conditioning gain_db is not finite: {}",
            c.gain_db
        )));
    }
    if !c.delay_ms.is_finite() {
        return Err(ComputeError::Recondition(format!(
            "conditioning delay_ms is not finite: {}",
            c.delay_ms
        )));
    }

    let filter = if c.muted {
        // A muted source contributes exactly zero to both the coherent and the
        // incoherent channel — an all-zero complex filter silences it exactly.
        Some(vec![Complex::new(0.0, 0.0); N_BANDS])
    } else if let Some(f) = &c.filter_band_db {
        if f.len() != N_BANDS {
            return Err(ComputeError::Recondition(format!(
                "filter_band_db has {} values, expected a dense [{N_BANDS}] by band index",
                f.len()
            )));
        }
        let mut cf = Vec::with_capacity(N_BANDS);
        for &db in f {
            if !db.is_finite() {
                return Err(ComputeError::Recondition(
                    "filter_band_db carries a non-finite value".to_string(),
                ));
            }
            cf.push(Complex::new(10f64.powf(db / 20.0), 0.0));
        }
        Some(cf)
    } else {
        None
    };

    Ok((
        BandSpectrum::uniform(c.gain_db),
        Conditioning {
            filter,
            delay_s: c.delay_ms / 1000.0,
        },
    ))
}

/// Recondition the OPFS tensor for the target receivers (SVC-06 / D-01), hash-gated
/// against the CURRENT scene identity (client-side 409, D-12).
///
/// # Errors
/// A shape error in either DTO; a [`crate::opfs_reader::ChunkDecodeError`] on a
/// malformed chunk; a [`ComputeError::HashMismatch`] on a stale/mismatched hash; or
/// a [`ComputeError::Recondition`] validation/readout failure.
#[wasm_bindgen]
pub fn recondition(
    scene: JsValue,
    req: JsValue,
    hi_bytes: &[u8],
    pi_bytes: &[u8],
) -> Result<JsValue, JsValue> {
    let scene: PrepareSolveReq = from_js(scene)?;
    let req: ReconditionReq = from_js(req)?;
    let current = crate::identity::marshalled_tensor_hash(&scene);
    let n_sub = scene.n_sub as usize;
    let chunk_len = req.receiver_ids.len();
    let (h, p) =
        read_chunk(hi_bytes, pi_bytes, n_sub, chunk_len).map_err(|e| js_err(&e.to_string()))?;
    let axis = FreqAxis::new();
    let result = recondition_receivers(&req, &current, h.view(), p.view(), &axis)
        .map_err(|e| compute_err(&e))?;
    to_js(&result)
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use envi_compute::readout::ReadoutLaw;
    use envi_engine::tensor::{compose_gain, readout_coherent};
    use ndarray::Array3;

    fn hval(s: usize, r: usize, f: usize) -> Complex<f64> {
        let k = (s * 100 + r * 10 + f) as f64;
        Complex::from_polar(0.1 + 0.001 * k, 0.017 * k - 0.3)
    }

    fn pval(s: usize, r: usize, f: usize) -> f64 {
        let k = (s * 100 + r * 10 + f) as f64;
        1.0e-6 * (1.0 + k)
    }

    fn synth_tensor(n_sub: usize, n_rcv: usize) -> (Array3<Complex<f64>>, Array3<f64>) {
        (
            Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| hval(s, r, f)),
            Array3::from_shape_fn((n_sub, n_rcv, N_BANDS), |(s, r, f)| pval(s, r, f)),
        )
    }

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("rcv-{i}")).collect()
    }

    fn conditioned(n_sub: usize) -> Vec<ConditioningDto> {
        (0..n_sub)
            .map(|s| ConditioningDto {
                gain_db: 70.0 + s as f64,
                delay_ms: 0.1 * (s as f64 + 1.0),
                filter_band_db: Some(vec![-1.5 + 0.02 * s as f64; N_BANDS]),
                muted: false,
            })
            .collect()
    }

    /// THE MAC ≡ recompute gate: a matching-hash recondition equals a full
    /// compose_gain + readout_coherent engine path bit-for-bit (`f64::to_bits`).
    #[test]
    fn matching_hash_recondition_equals_engine_path_bit_for_bit() {
        let axis = FreqAxis::new();
        let (n_sub, n_rcv) = (3usize, 4usize);
        let (h, p) = synth_tensor(n_sub, n_rcv);
        let cond = conditioned(n_sub);
        let req = ReconditionReq {
            tensor_hash: "match".to_string(),
            per_source_conditioning: cond.clone(),
            receiver_ids: ids(n_rcv),
        };
        let out = recondition_receivers(&req, "match", h.view(), p.view(), &axis).unwrap();

        // Reconstruct the engine coherent MAC directly for every receiver.
        let g: Vec<Vec<Complex<f64>>> = cond
            .iter()
            .map(|c| {
                let filt: Vec<Complex<f64>> = c
                    .filter_band_db
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|&db| Complex::new(10f64.powf(db / 20.0), 0.0))
                    .collect();
                compose_gain(
                    &BandSpectrum::uniform(c.gain_db),
                    Some(&filt),
                    c.delay_ms / 1000.0,
                    &axis,
                )
                .unwrap()
            })
            .collect();
        let p_direct = readout_coherent(h.view(), &g).unwrap();

        // And the full 11-01 readout core (the coherent law) — band_levels_db must
        // match recondition's output bit-for-bit.
        let conds: Vec<Conditioning> = cond
            .iter()
            .map(|c| Conditioning {
                filter: Some(
                    c.filter_band_db
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|&db| Complex::new(10f64.powf(db / 20.0), 0.0))
                        .collect(),
                ),
                delay_s: c.delay_ms / 1000.0,
            })
            .collect();
        let spectra: Vec<BandSpectrum> = cond
            .iter()
            .map(|c| BandSpectrum::uniform(c.gain_db))
            .collect();

        for r in 0..n_rcv {
            let reference = readout_receiver(
                h.view(),
                p.view(),
                r,
                &spectra,
                &conds,
                ReadoutLaw::Coherent,
                &axis,
            )
            .unwrap();
            for f in 0..N_BANDS {
                assert_eq!(
                    out.spectra[r][f].to_bits(),
                    reference.band_levels_db[f].to_bits(),
                    "recondition band level diverged from the readout core at r={r} f={f}"
                );
                assert_eq!(
                    reference.coherent_energy[f].to_bits(),
                    p_direct[[r, f]].norm_sqr().to_bits(),
                    "the coherent channel is not the direct compose_gain+readout_coherent MAC"
                );
            }
        }
        assert!(!out.stale, "a matched recondition is never stale");
    }

    /// A mismatched hash returns HashMismatch{expected, got} and produces NO spectra.
    #[test]
    fn mismatched_hash_returns_hashmismatch_and_no_spectra() {
        let axis = FreqAxis::new();
        let (h, p) = synth_tensor(1, 2);
        let req = ReconditionReq {
            tensor_hash: "client-believes-this".to_string(),
            per_source_conditioning: vec![ConditioningDto::default()],
            receiver_ids: ids(2),
        };
        let err = recondition_receivers(&req, "actual-current-identity", h.view(), p.view(), &axis)
            .unwrap_err();
        match err {
            ComputeError::HashMismatch { expected, got } => {
                assert_eq!(expected, "actual-current-identity");
                assert_eq!(got, "client-believes-this");
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    /// The never-stale invariant: a default (no-op) conditioning recondition equals
    /// the plain 11-01 readout of the same tensor bit-for-bit.
    #[test]
    fn default_conditioning_equals_plain_readout() {
        let axis = FreqAxis::new();
        let (n_sub, n_rcv) = (2usize, 3usize);
        let (h, p) = synth_tensor(n_sub, n_rcv);
        let cond = vec![ConditioningDto::default(); n_sub];
        let req = ReconditionReq {
            tensor_hash: "id".to_string(),
            per_source_conditioning: cond,
            receiver_ids: ids(n_rcv),
        };
        let out = recondition_receivers(&req, "id", h.view(), p.view(), &axis).unwrap();

        // Plain readout: unit L_W, no conditioning, default law (Incoherent for a
        // no-op drive) — the same path a road/omni source reads out under.
        let spectra = vec![BandSpectrum::uniform(0.0); n_sub];
        let conds = vec![
            Conditioning {
                filter: None,
                delay_s: 0.0,
            };
            n_sub
        ];
        let law = default_law(&conds);
        for r in 0..n_rcv {
            let plain =
                readout_receiver(h.view(), p.view(), r, &spectra, &conds, law, &axis).unwrap();
            for f in 0..N_BANDS {
                assert_eq!(
                    out.spectra[r][f].to_bits(),
                    plain.band_levels_db[f].to_bits(),
                    "default-conditioning recondition must equal the plain readout at r={r} f={f}"
                );
            }
        }
    }

    /// A non-dense `[105]` filter is a typed Recondition error, never a panic (V5).
    #[test]
    fn non_dense_filter_is_a_typed_error() {
        let axis = FreqAxis::new();
        let (h, p) = synth_tensor(1, 1);
        let req = ReconditionReq {
            tensor_hash: "id".to_string(),
            per_source_conditioning: vec![ConditioningDto {
                gain_db: 0.0,
                delay_ms: 0.0,
                filter_band_db: Some(vec![0.0; 104]), // one short of 105
                muted: false,
            }],
            receiver_ids: ids(1),
        };
        assert!(matches!(
            recondition_receivers(&req, "id", h.view(), p.view(), &axis),
            Err(ComputeError::Recondition(_))
        ));
    }

    /// A muted source contributes exactly zero — a single-muted + single-active pair
    /// reads out identically to the active source alone.
    #[test]
    fn muted_source_is_silenced() {
        let axis = FreqAxis::new();
        let (h, p) = synth_tensor(2, 1);
        // Source 0 muted, source 1 active with a broadband gain.
        let req_pair = ReconditionReq {
            tensor_hash: "id".to_string(),
            per_source_conditioning: vec![
                ConditioningDto {
                    gain_db: 50.0,
                    delay_ms: 0.0,
                    filter_band_db: None,
                    muted: true,
                },
                ConditioningDto {
                    gain_db: 80.0,
                    delay_ms: 0.0,
                    filter_band_db: None,
                    muted: false,
                },
            ],
            receiver_ids: ids(1),
        };
        let out = recondition_receivers(&req_pair, "id", h.view(), p.view(), &axis).unwrap();
        // Every band level must be finite (a silenced source never produces -inf/NaN
        // that would poison the coherent sum).
        assert!(out.spectra[0].iter().all(|v| v.is_finite()));
    }
}
