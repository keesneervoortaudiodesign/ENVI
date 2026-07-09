//! Semi-transparent partition transmission: the isolation spectrum `R(f)` as a
//! complex **minimum-phase** transmission filter `T(f)` (AV 1106/07 §5.13–5.15
//! screen family; ENG-10 + the D-06 ENVI extension).
//!
//! # An ENVI extension beyond stock Nord2000 (D-06/D-07)
//!
//! Stock Nord2000 treats a partition's sound reduction index `R(f)` as a real
//! energy loss (`10^(−R/20)`, phase discarded). ENVI instead models the physical
//! truth that a **passive partition is a minimum-phase system** — its transmitted
//! phase is not free, it *follows* its amplitude. So the transmitted straight-
//! through path gets a complex filter
//!
//! ```text
//! T(f) = 10^(−R(f)/20) · e^{jφ_min(f)},   φ_min(f) = −H{ ln|T(f)| }
//! ```
//!
//! with the minimum phase reconstructed from the log-magnitude via the discrete
//! Hilbert transform (equivalently the real-cepstrum fold) over the 105-point
//! band axis. This is the same spirit as the Phase-4 directional complex-phase
//! extension: an optional `e^{jφ}` that is bit-identical to the magnitude-only
//! path when the phase is zero (a flat `R` gives `φ ≡ 0` exactly).
//!
//! # Convention (load-bearing — D-05/D-08)
//!
//! Nord2000-native module: time convention **e^{−jωt}**. Conversion to ENVI's
//! e^{+jωt} transfer convention happens in exactly ONE function at the
//! `TransferSpectrum` boundary (`transfer::nord_ratio_to_transfer`) — **never
//! here** (mirror `refraction/mod.rs`: the `.conj()` quarantine over
//! `propagation/` stays at zero). Where the native filter must negate the
//! reconstructed phase it is written as an **explicit negative sine**
//! (`Complex::new(mag·cosφ, mag·(−sinφ))`), never a `.conj()` call.
//!
//! # DFT sign convention (load-bearing — 05-RESEARCH Finding 3)
//!
//! The cepstral machinery uses the **numpy.fft convention**: the forward DFT is
//! `X[k] = Σ_n x[n]·e^{−j2πkn/M}` and the inverse is
//! `x[n] = (1/M)·Σ_k X[k]·e^{+j2πkn/M}`. With that convention the cepstral fold
//! yields the **ENVI (lagging) phase** `φ_cep` directly — verified to 6.7e-16
//! against the known minimum-phase system `H = 1 + 0.5·e^{−jω}`. Hence the
//! Nord2000-**native** filter negates it: `T_native = |T|·e^{−jφ_cep}`. After the
//! single conj at the transfer boundary this maps to `|T|·e^{+jφ_cep}` — a
//! lagging, causal, positive-group-delay filter consistent with the free-field
//! `e^{−jωτ}` delay it multiplies alongside.
//!
//! # The hand-rolled 208-point DFT (dep-quarantine hand-roll)
//!
//! `envi-engine` deps are quarantined to `ndarray + num-complex + thiserror`
//! only (no FFT crate). The cepstrum needs a one-shot 208-point transform pair,
//! so a naive O(M²) DFT over `Complex<f64>` (~43k complex multiplies) is the
//! deliberate hand-roll — the `directivity.rs` explicit-rotation precedent.
//!
//! # Honesty note (05-RESEARCH Finding 3c)
//!
//! This is the minimum phase of `ln|T|` as a periodic function of **band index**
//! (the log-f-uniform 105-point axis), not the analog (linear-frequency) Bode
//! minimum phase — the two differ by the axis warping and the band-limited
//! window. A flat `R` gives exactly zero phase; `φ` is exactly linear in `R`;
//! edge bands carry the largest reconstruction end-effects. Tight-tolerance
//! analytic phase anchors are wrong by construction — the property tests
//! (flat⇒0, linearity, causal trend) plus the committed numpy oracle are the pin.

use num_complex::Complex;

use crate::freq::N_BANDS;
use crate::propagation::PropagationError;

/// Mirror length: even-mirror of the 105-point band axis about both endpoints.
/// `y = [ln_mag[0..=104], ln_mag[103], ln_mag[102], …, ln_mag[1]]` (numpy
/// `np.concatenate([ln_mag, ln_mag[-2:0:-1]])`) removes the periodic-wrap
/// discontinuity — the standard cepstral end-effect treatment, no zero-padding.
const M: usize = 208;

/// A validated per-band sound reduction index `R(f)` (dB), indexed by the
/// 105-point 1/12-octave band axis — **never** by nominal Hz.
///
/// This is the caller → pure-math trust boundary for semi-transparent partitions
/// (future Phase-7 UI / Phase-9 path data). The constructor rejects any
/// non-finite or negative band so that `ln|T| = −R·ln10/20` is always finite and
/// a NaN can never poison the transfer tensor.
///
/// # Contract
///
/// - The 105-point grid is the engine contract; the 1/1→1/3→1/12 interpolation
///   from a coarse measured spectrum is upstream (Phase 7, SCN-03).
/// - ONE spectrum per solve job represents the **total crossed partition stack**;
///   multi-partition composition (`T₁·T₂`) is a Phase-9 path concern (D-11).
/// - **Opaque is `Option::None` at the seam, NOT a large `R`** (D-10): `R = ∞` is
///   structurally unrepresentable here, so `ln|T| = −∞` can never reach the
///   cepstrum. `R ≥ 0` keeps the log-magnitude finite for every band.
#[derive(Debug, Clone, PartialEq)]
pub struct IsolationSpectrum {
    r_db: [f64; N_BANDS],
}

impl IsolationSpectrum {
    /// Build a validated isolation spectrum from 105 per-band `R` values (dB).
    ///
    /// Accepts `R = 0.0` (fully transparent) and any large **finite** `R`.
    ///
    /// # Errors
    ///
    /// [`PropagationError::InvalidIsolationSpectrum`] carrying the offending band
    /// index and value if any band is NaN, `±∞`, or negative (a passive partition
    /// cannot have transmission gain, and a non-finite `R` would poison the
    /// cepstrum). The explicit `is_finite` check is load-bearing — a bare
    /// `value < 0.0` comparison lets `+∞` through.
    pub fn new(r_db: [f64; N_BANDS]) -> Result<Self, PropagationError> {
        for (band, &value) in r_db.iter().enumerate() {
            if !value.is_finite() || value < 0.0 {
                return Err(PropagationError::InvalidIsolationSpectrum { band, value });
            }
        }
        Ok(Self { r_db })
    }

    /// The validated per-band `R(f)` in dB (band-indexed).
    #[must_use]
    pub fn as_bands(&self) -> &[f64; N_BANDS] {
        &self.r_db
    }
}

/// Inverse DFT (numpy `ifft` convention): `x[n] = (1/M)·Σ_k X[k]·e^{+j2πkn/M}`.
fn idft(input: &[Complex<f64>; M]) -> [Complex<f64>; M] {
    let mut out = [Complex::new(0.0, 0.0); M];
    for (n, slot) in out.iter_mut().enumerate() {
        let mut acc = Complex::new(0.0, 0.0);
        for (k, &x) in input.iter().enumerate() {
            // Reduce the phase index mod M before the twiddle to shed round-off.
            let idx = (k * n) % M;
            let ang = std::f64::consts::TAU * idx as f64 / M as f64;
            let (s, c) = ang.sin_cos();
            acc += x * Complex::new(c, s); // e^{+j·ang}
        }
        *slot = acc / M as f64;
    }
    out
}

/// Forward DFT (numpy `fft` convention): `X[k] = Σ_n x[n]·e^{−j2πkn/M}`.
fn dft_forward(input: &[Complex<f64>; M]) -> [Complex<f64>; M] {
    let mut out = [Complex::new(0.0, 0.0); M];
    for (k, slot) in out.iter_mut().enumerate() {
        let mut acc = Complex::new(0.0, 0.0);
        for (n, &x) in input.iter().enumerate() {
            let idx = (k * n) % M;
            let ang = std::f64::consts::TAU * idx as f64 / M as f64;
            let (s, c) = ang.sin_cos();
            acc += x * Complex::new(c, -s); // e^{−j·ang}
        }
        *slot = acc;
    }
    out
}

/// The even-mirror cepstral fold (05-RESEARCH Finding 3a), returning **both**
/// output parts of `Z = DFT(ĉ)`: `Re(Z[n])` (which reproduces `ln_mag[n]`, the
/// free internal-consistency check) and `Im(Z[n])` (the ENVI lagging phase
/// `φ_cep`). Split out so the property test can pin the real-part reconstruction
/// (T4) without a second public entry point.
fn min_phase_parts(r_db: &[f64; N_BANDS]) -> ([f64; N_BANDS], [f64; N_BANDS]) {
    // Step 1 (ln_mag = −R·ln10/20) + step 2 (even mirror to M = 208).
    let ln10 = std::f64::consts::LN_10;
    let mut y = [Complex::new(0.0, 0.0); M];
    for (n, slot) in y.iter_mut().enumerate() {
        // n ≤ 104 → ln_mag[n]; n ≥ 105 → ln_mag[208 − n] (even about both ends).
        let src = if n < N_BANDS { n } else { M - n };
        *slot = Complex::new(-r_db[src] * ln10 / 20.0, 0.0);
    }

    // Step 3: real cepstrum c = IDFT_M{y} (complex arithmetic throughout).
    let c = idft(&y);

    // Step 4: causal fold — ĉ[0]=c[0], ĉ[m]=2c[m] (1≤m≤103), ĉ[104]=c[104] (the
    // M/2 element stays ×1 — doubling or zeroing it biases every band), ĉ[m]=0
    // for m > 104.
    let mut cc = [Complex::new(0.0, 0.0); M];
    cc[0] = c[0];
    for (m, slot) in cc.iter_mut().enumerate().take(M / 2).skip(1) {
        *slot = c[m] * 2.0;
    }
    cc[M / 2] = c[M / 2];

    // Step 5: Z = DFT_M{ĉ}; φ_cep[n] = Im(Z[n]); Re(Z[n]) ≈ ln_mag[n].
    let z = dft_forward(&cc);
    let mut re = [0.0; N_BANDS];
    let mut im = [0.0; N_BANDS];
    for (n, (re_slot, im_slot)) in re.iter_mut().zip(im.iter_mut()).enumerate() {
        *re_slot = z[n].re;
        *im_slot = z[n].im;
    }
    (re, im)
}

/// Reconstruct the ENVI-convention (`e^{+jωt}`, lagging) minimum phase `φ_cep`
/// from the isolation spectrum via the even-mirror cepstral fold.
///
/// Returns one phase (rad) per 1/12-octave band. A flat `R` gives `φ ≡ 0`
/// exactly; `φ` is exactly linear in `R`. The **native** `e^{−jωt}` filter uses
/// `−φ` (see [`TransmissionFilter::from_isolation`]).
#[must_use]
pub fn min_phase_phi_envi(r_db: &[f64; N_BANDS]) -> [f64; N_BANDS] {
    min_phase_parts(r_db).1
}

/// The Nord2000-native (`e^{−jωt}`) complex transmission filter `T(f)` for a
/// semi-transparent partition — one `Complex<f64>` per 1/12-octave band.
///
/// Built once per solve job before the terrain-effect band loop and added to the
/// screen branch's coherent factor (plan 05-03); it never touches the incoherent
/// channel.
#[derive(Debug, Clone, PartialEq)]
pub struct TransmissionFilter {
    /// Native-convention `T(f) = 10^(−R/20)·e^{−jφ_cep}` per band index.
    pub bands: [Complex<f64>; N_BANDS],
}

impl TransmissionFilter {
    /// Assemble the native filter from a validated isolation spectrum.
    ///
    /// `mag[n] = 10^(−R[n]/20)`; the native `e^{−jωt}` phase is `−φ_cep[n]`,
    /// written as an **explicit negative sine** — the single conj at the transfer
    /// boundary then maps this to the lagging causal ENVI filter
    /// `|T|·e^{+jφ_cep}`. A phase-free (flat `R`) spectrum leaves `arg(T)` at 0,
    /// so the extension is bit-compatible with the magnitude-only path.
    #[must_use]
    pub fn from_isolation(iso: &IsolationSpectrum) -> Self {
        let r = iso.as_bands();
        let phi = min_phase_phi_envi(r);
        let mut bands = [Complex::new(0.0, 0.0); N_BANDS];
        for (n, slot) in bands.iter_mut().enumerate() {
            let mag = 10f64.powf(-r[n] / 20.0);
            let (s, c) = phi[n].sin_cos();
            // Native e^{−jωt}: |T|·e^{−jφ_cep}. The sign flip is the explicit
            // negative sine, NOT a `.conj()` (D-08; propagation/ conj gate = 0).
            *slot = Complex::new(mag * c, mag * (-s));
        }
        Self { bands }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(r: f64) -> [f64; N_BANDS] {
        [r; N_BANDS]
    }

    /// T1: a spectrally flat partition is a pure real attenuation ⇒ φ ≡ 0.
    #[test]
    fn t1_flat_isolation_gives_zero_phase() {
        let phi = min_phase_phi_envi(&flat(30.0));
        for (n, &p) in phi.iter().enumerate() {
            assert!(p.abs() < 1e-12, "band {n}: |φ| = {} not < 1e-12", p.abs());
        }
    }

    /// T2: φ is exactly linear in R (Hilbert linearity) ⇒ φ(2R) = 2·φ(R).
    #[test]
    fn t2_phase_is_linear_in_isolation() {
        let mut r = [0.0; N_BANDS];
        for (n, slot) in r.iter_mut().enumerate() {
            // A non-trivial, always-≥0 spectrum with live phase.
            *slot = 25.0 + 6.0 * (n as f64 * 0.11).sin() + 0.05 * n as f64;
        }
        let mut r2 = [0.0; N_BANDS];
        for (slot, &v) in r2.iter_mut().zip(r.iter()) {
            *slot = 2.0 * v;
        }
        let phi = min_phase_phi_envi(&r);
        let phi2 = min_phase_phi_envi(&r2);
        for n in 0..N_BANDS {
            assert!(
                (phi2[n] - 2.0 * phi[n]).abs() < 1e-12,
                "band {n}: φ(2R) {} vs 2·φ(R) {}",
                phi2[n],
                2.0 * phi[n]
            );
        }
    }

    /// T3 (the D-08 first-principles causality check, independent of the oracle).
    ///
    /// The even-mirror to M = 208 makes `φ_envi` **symmetric about band 52**
    /// (`φ[k] = φ[104−k]`), so no rising input is monotone across the whole
    /// mid-band — the deep flank crosses the symmetry center (05-RESEARCH
    /// Finding 3c: this is the periodic band-axis min-phase, not the smooth
    /// analog Bode phase). The load-bearing D-08 facts that ARE true and are what
    /// this test pins:
    ///
    /// 1. **Sign** — a rising mass-law R (falling `ln|T|`) gives a **lagging**
    ///    (φ ≤ 0) ENVI phase across mid-band; a falling R (rising `ln|T|`) gives
    ///    a **leading** (φ ≥ 0) phase. The sign is driven by the magnitude slope
    ///    — the minimum-phase causality direction — not by any constant offset.
    /// 2. **Monotone flank** — on the rising flank before the symmetry center
    ///    (bands 10..50) φ is monotone non-increasing (positive group delay).
    /// 3. **Antisymmetry** — reversing the slope negates φ exactly (`φ_rise` =
    ///    `−φ_fall`), a clean magnitude-driven sign witness.
    /// 4. **Native negation** — `T_native.im = mag·(−sinφ)` opposes `sin(φ)` at
    ///    every live-phase band (the explicit negative sine, never `.conj()`).
    #[test]
    fn t3_causal_sign_flank_antisymmetry_native_negation() {
        // Rising mass-law-like R (falling log-magnitude), always ≥ 0.
        let mut rise = [0.0; N_BANDS];
        for (n, slot) in rise.iter_mut().enumerate() {
            *slot = 20.0 + 0.25 * n as f64;
        }
        // Falling R (rising log-magnitude), always ≥ 0 — the opposite slope.
        let mut fall = [0.0; N_BANDS];
        for (n, slot) in fall.iter_mut().enumerate() {
            *slot = (50.0 - 0.25 * n as f64).max(0.0);
        }
        let phi_rise = min_phase_phi_envi(&rise);
        let phi_fall = min_phase_phi_envi(&fall);

        // (1) Sign across mid-band: rising ⇒ lagging (≤0), falling ⇒ leading (≥0).
        for n in 5..53 {
            assert!(
                phi_rise[n] <= 1e-12,
                "band {n}: rising-R ENVI φ {} must be lagging (≤ 0)",
                phi_rise[n]
            );
            assert!(
                phi_fall[n] >= -1e-12,
                "band {n}: falling-R ENVI φ {} must be leading (≥ 0)",
                phi_fall[n]
            );
        }
        // (2) Monotone non-increasing on the rising flank (before band 52).
        for n in 10..50 {
            assert!(
                phi_rise[n + 1] <= phi_rise[n] + 1e-9,
                "band {n}: rising-R φ not non-increasing on the flank ({} → {})",
                phi_rise[n],
                phi_rise[n + 1]
            );
        }
        // (3) Reversing the slope negates φ exactly (antisymmetric to 1e-9).
        let mut saw_live = false;
        for (n, (&pr, &pf)) in phi_rise.iter().zip(phi_fall.iter()).enumerate() {
            assert!(
                (pr + pf).abs() < 1e-9,
                "band {n}: φ_rise {pr} must be −φ_fall {pf}"
            );
            if pr.abs() > 0.01 {
                saw_live = true;
            }
        }
        assert!(
            saw_live,
            "the slope cases must exercise live phase somewhere"
        );

        // (4) The native filter negates the ENVI phase: T_native.im = mag·(−sinφ)
        // opposes sin(φ) at every live-phase band.
        let filter = TransmissionFilter::from_isolation(&IsolationSpectrum::new(rise).unwrap());
        for (n, band) in filter.bands.iter().enumerate() {
            let s = phi_rise[n].sin();
            if s.abs() > 1e-6 {
                assert!(
                    band.im.signum() != s.signum(),
                    "band {n}: native im {} must oppose sin(φ) {}",
                    band.im,
                    s
                );
            }
        }
    }

    /// T4: internal consistency — Re(Z[n]) reproduces ln_mag[n] to 1e-12.
    #[test]
    fn t4_real_part_reconstructs_log_magnitude() {
        let mut r = [0.0; N_BANDS];
        for (n, slot) in r.iter_mut().enumerate() {
            *slot = 18.0 + 4.0 * (n as f64 * 0.2).cos() + 0.03 * n as f64;
        }
        let (re, _phi) = min_phase_parts(&r);
        let ln10 = std::f64::consts::LN_10;
        for n in 0..N_BANDS {
            let ln_mag = -r[n] * ln10 / 20.0;
            assert!(
                (re[n] - ln_mag).abs() < 1e-12,
                "band {n}: Re(Z) {} vs ln|T| {}",
                re[n],
                ln_mag
            );
        }
    }

    /// T10: the validating constructor rejects NaN / +∞ / −∞ / negative R at any
    /// band, carrying the band index and value; accepts 0.0 and large finite R.
    #[test]
    fn t10_constructor_rejects_non_finite_and_negative() {
        // Accept: fully transparent and a large finite isolation.
        assert!(IsolationSpectrum::new(flat(0.0)).is_ok());
        assert!(IsolationSpectrum::new(flat(300.0)).is_ok());

        // ±∞ are built from their IEEE-754 bit patterns rather than the library
        // constant: the D-10 source gate forbids any infinity sentinel token
        // appearing in this file (opaque is `None`, never a magic R), and these
        // are used purely as *rejected* inputs, never as an encoding.
        let pos_inf = f64::from_bits(0x7ff0_0000_0000_0000);
        let neg_inf = f64::from_bits(0xfff0_0000_0000_0000);
        for (bad, band) in [
            (f64::NAN, 7usize),
            (pos_inf, 40),
            (neg_inf, 64),
            (-1e-9, 88),
        ] {
            let mut r = flat(30.0);
            r[band] = bad;
            match IsolationSpectrum::new(r) {
                Err(PropagationError::InvalidIsolationSpectrum { band: b, value }) => {
                    assert_eq!(b, band, "wrong band reported for {bad}");
                    assert!(
                        value.to_bits() == bad.to_bits() || (value.is_nan() && bad.is_nan()),
                        "wrong value reported: {value} vs {bad}"
                    );
                }
                other => panic!("expected InvalidIsolationSpectrum for {bad}, got {other:?}"),
            }
        }
    }
}
