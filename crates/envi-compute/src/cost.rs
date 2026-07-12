//! The pre-run cost model + two-level guardrail (SC1) — pure byte/receiver
//! arithmetic, no acoustic math (not an SVC-07 violation).
//!
//! Everything here is cheap arithmetic on the grid spec: it never allocates the
//! tensor, so a runaway grid is surfaced *before* any compute. The estimate keys
//! off the final (fine) spacing (D-06) because coarse tiers add NO receivers —
//! they are a strict subset of the fine lattice (D-05).
//!
//! # Formulas
//!
//! - Receiver count `N = discrete_points + floor(area_m² / spacing_fine²)`.
//! - Tensor bytes `= n_sub · N · N_BANDS · BYTES_PER_CELL_PAIR` (the OPFS
//!   on-disk footprint; the engine constant is used, never a hard-coded 24).
//! - Working-set bytes `= n_workers · chunk_receivers · n_sub · N_BANDS ·
//!   BYTES_PER_CELL_PAIR` (the SC3 RAM bound: workers each hold one chunk).
//! - Time estimate `≈ (n_sub · N · t_pair) / n_workers` — a device-adaptive
//!   extrapolation over a per-pair constant. [`DEFAULT_T_PAIR_MS`] is a
//!   conservative built-in default; a submit-time calibration probe may replace
//!   it (Claude's discretion, 10-RESEARCH §Time estimate — tunable).
//!
//! # Guardrail (SC1)
//!
//! Because `N ∝ 1/spacing²`, **halving the spacing quadruples the cost** (exact).
//! [`guardrail`] returns `Warn` on the soft thresholds (tensor over the 256 MiB
//! streaming budget, a long run, or a very fine grid) and `Block` only on a hard
//! budget breach (e.g. the OPFS quota passed by the caller).

use envi_engine::freq::N_BANDS;
use envi_engine::tensor::{BYTES_PER_CELL_PAIR, DEFAULT_TENSOR_BUDGET_BYTES};

use crate::identity::chunk_receivers;

/// Conservative built-in per-(sub_source, receiver)-pair solve time (ms), used
/// when no calibration probe is supplied. Tunable: 10-RESEARCH recommends timing
/// a small probe solve at submit time to replace this with a device-measured
/// `t_pair`; this default keeps the estimate honest (erring slow) without one.
pub const DEFAULT_T_PAIR_MS: f64 = 0.05;

/// Soft warn threshold on tensor bytes: the engine's 256 MiB streaming budget
/// (D-06 / 10-UI-SPEC "warn ~256 MiB"). Above this the OPFS footprint is large.
/// `u64` so the comparison against the (widened) `tensor_bytes` is 64-bit on
/// wasm32 too (WR-02 — `usize` is 32-bit there).
pub const WARN_TENSOR_BYTES: u64 = DEFAULT_TENSOR_BUDGET_BYTES as u64;

/// Soft warn threshold on the time estimate: 5 minutes (10-RESEARCH §Guardrail).
pub const WARN_TIME_MS: f64 = 5.0 * 60.0 * 1000.0;

/// Soft warn threshold on receiver count: a very fine grid (10-RESEARCH §Guardrail).
pub const WARN_RECEIVERS: usize = 100_000;

/// A pure pre-run cost estimate for a grid solve (SC1). No tensor is allocated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostEstimate {
    /// Total receivers `N = discrete_points + floor(area / spacing_fine²)`.
    /// Coarse tiers add none (subset of fine, D-05).
    pub receiver_count: usize,
    /// Full OPFS on-disk tensor footprint, bytes
    /// (`n_sub · N · N_BANDS · BYTES_PER_CELL_PAIR`). `u64` (not `usize`) so the
    /// product cannot wrap on wasm32's 32-bit `usize` and silently defeat the
    /// `Block` guardrail (WR-02); the boundary DTO already carries it as `f64`.
    pub tensor_bytes: u64,
    /// Resident RAM working set, bytes
    /// (`n_workers · chunk_receivers · n_sub · N_BANDS · BYTES_PER_CELL_PAIR`, SC3).
    /// `u64` for the same wasm32 no-overflow reason as [`tensor_bytes`](Self::tensor_bytes).
    pub working_set_bytes: u64,
    /// Wall-clock time estimate, milliseconds (`n_sub · N · t_pair / n_workers`).
    pub time_estimate_ms: f64,
}

/// Estimate the cost of a grid solve.
///
/// `area_m2` is the calc-area footprint, `spacing_fine_m` the user's final
/// spacing (D-06), `discrete_points` the count of explicit receiver points (which
/// the coarse/fine lattice does not double-count), `n_sub` the sub-source count,
/// and `n_workers` the thread-pool size (`navigator.hardwareConcurrency`).
///
/// Deviation from the plan's 4-arg signature: `n_workers` is threaded through
/// because both the working-set bound and the time extrapolation genuinely
/// require it (10-RESEARCH formulas) — a hidden default would be dishonest.
#[must_use]
pub fn estimate(
    area_m2: f64,
    spacing_fine_m: f64,
    discrete_points: usize,
    n_sub: usize,
    n_workers: usize,
) -> CostEstimate {
    let n_sub = n_sub.max(1);
    let n_workers = n_workers.max(1);
    // Grid points inside the calc area at the final spacing (guard bad spacing).
    let n_fine = if spacing_fine_m.is_finite() && spacing_fine_m > 0.0 && area_m2 > 0.0 {
        (area_m2 / (spacing_fine_m * spacing_fine_m)).floor() as usize
    } else {
        0
    };
    let receiver_count = discrete_points + n_fine;

    // Byte products in u64 with saturating multiplies: on wasm32 `usize` is 32-bit,
    // so a `usize` product would wrap for a large grid and hand the guardrail a tiny
    // (under-budget) `tensor_bytes` — silently bypassing the SC1 hard `Block`. u64
    // holds the true size regardless of `budget_bytes`; an overflow saturates to
    // `u64::MAX`, which always trips `Block` (WR-02).
    let per_receiver = (n_sub as u64) * (N_BANDS as u64) * (BYTES_PER_CELL_PAIR as u64);
    let tensor_bytes = (receiver_count as u64).saturating_mul(per_receiver);

    let chunk = chunk_receivers(n_sub, receiver_count);
    let working_set_bytes = (n_workers as u64)
        .saturating_mul(chunk as u64)
        .saturating_mul(per_receiver);

    let time_estimate_ms =
        (n_sub as f64 * receiver_count as f64 * DEFAULT_T_PAIR_MS) / n_workers as f64;

    CostEstimate {
        receiver_count,
        tensor_bytes,
        working_set_bytes,
        time_estimate_ms,
    }
}

/// Guardrail severity for a cost estimate (SC1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardrailLevel {
    /// Within all soft thresholds and under the hard budget.
    Ok,
    /// Over a soft threshold (large/long/very-fine) but under the hard budget.
    Warn,
    /// Over the hard budget — the run must not proceed as specified.
    Block,
}

/// A guardrail verdict: a [`GuardrailLevel`] plus a human-readable detail that
/// always names the multiplicative spacing/cost relation (SC1).
#[derive(Debug, Clone, PartialEq)]
pub struct Guardrail {
    /// The severity level.
    pub level: GuardrailLevel,
    /// Human-readable explanation (surfaced to the user as text; never HTML).
    pub detail: String,
}

/// Evaluate the guardrail for an estimate against a hard `budget_bytes`
/// (e.g. the OPFS quota). `Block` fires only on a hard budget breach; `Warn`
/// fires on the soft thresholds ([`WARN_TENSOR_BYTES`], [`WARN_TIME_MS`],
/// [`WARN_RECEIVERS`]). The detail always expresses the exact "halving the
/// spacing quadruples the cost" relation (since `N ∝ 1/spacing²`).
#[must_use]
pub fn guardrail(estimate: &CostEstimate, budget_bytes: u64) -> Guardrail {
    // The exact multiplicative relation, always surfaced (SC1).
    let scaling = "halving the final spacing quadruples the cost (receivers ∝ 1/spacing²)";

    if estimate.tensor_bytes > budget_bytes {
        return Guardrail {
            level: GuardrailLevel::Block,
            detail: format!(
                "tensor {tb} B exceeds the {budget} B budget — coarsen the final spacing; {scaling}",
                tb = estimate.tensor_bytes,
                budget = budget_bytes,
            ),
        };
    }

    let mut reasons: Vec<&str> = Vec::new();
    if estimate.tensor_bytes > WARN_TENSOR_BYTES {
        reasons.push("large tensor (may exceed browser storage)");
    }
    if estimate.time_estimate_ms > WARN_TIME_MS {
        reasons.push("long run (consider a coarser final spacing)");
    }
    if estimate.receiver_count > WARN_RECEIVERS {
        reasons.push("very fine grid");
    }

    if reasons.is_empty() {
        Guardrail {
            level: GuardrailLevel::Ok,
            detail: format!("within budget; {scaling}"),
        }
    } else {
        Guardrail {
            level: GuardrailLevel::Warn,
            detail: format!("{}; {scaling}", reasons.join(", ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensor_bytes_uses_engine_constant_for_known_grid() {
        // 4 sub-sources, exactly 1000 receivers (area/spacing² = 1000, no discrete).
        let est = estimate(1000.0, 1.0, 0, 4, 8);
        assert_eq!(est.receiver_count, 1000);
        let expected = 4u64 * 1000 * N_BANDS as u64 * BYTES_PER_CELL_PAIR as u64;
        assert_eq!(
            est.tensor_bytes, expected,
            "tensor_bytes must be n_sub·N·N_BANDS·BYTES_PER_CELL_PAIR"
        );
        // Sanity: BYTES_PER_CELL_PAIR is the engine's 24, not a local literal.
        assert_eq!(BYTES_PER_CELL_PAIR, 24);
    }

    #[test]
    fn halving_spacing_quadruples_receiver_count() {
        // N ∝ 1/spacing² (D-05/D-06): halving spacing → ~4× receivers.
        let coarse = estimate(1_000_000.0, 10.0, 0, 2, 4);
        let fine = estimate(1_000_000.0, 5.0, 0, 2, 4);
        assert_eq!(coarse.receiver_count, 10_000);
        assert_eq!(fine.receiver_count, 40_000);
        assert_eq!(
            fine.receiver_count,
            4 * coarse.receiver_count,
            "halving the spacing must quadruple the receiver count"
        );
        // Tensor bytes scale the same way.
        assert_eq!(fine.tensor_bytes, 4 * coarse.tensor_bytes);
    }

    #[test]
    fn working_set_scales_with_workers_and_stays_a_chunk_bound() {
        let one = estimate(1_000_000.0, 10.0, 0, 2, 1);
        let four = estimate(1_000_000.0, 10.0, 0, 2, 4);
        // Same chunk size, 4× the workers ⇒ 4× the resident working set (SC3).
        assert_eq!(four.working_set_bytes, 4 * one.working_set_bytes);
        // The working set is bounded by workers·chunk, never the full tensor.
        assert!(one.working_set_bytes <= one.tensor_bytes);
    }

    #[test]
    fn large_grid_byte_math_does_not_overflow_u32_and_still_blocks() {
        // WR-02: a grid whose tensor exceeds u32::MAX bytes. On wasm32 `usize` is
        // 32-bit, so the OLD `usize` product would wrap to a small value and the
        // guardrail would return Ok/Warn under a real quota. The u64 arithmetic must
        // report the true size and still `Block`. 500_000 receivers × (4·105·24 =
        // 10_080) B ≈ 5.04 GB — well past u32::MAX (~4.29 GB).
        let est = estimate(500_000.0, 1.0, 0, 4, 8);
        assert_eq!(est.receiver_count, 500_000);
        let per_receiver = 4u64 * N_BANDS as u64 * BYTES_PER_CELL_PAIR as u64;
        assert_eq!(est.tensor_bytes, 500_000u64 * per_receiver);
        assert!(
            est.tensor_bytes > u32::MAX as u64,
            "the tensor must exceed u32::MAX bytes (the wasm32 overflow trigger)"
        );
        // Against CalcPanel's 2 GiB budget the hard Block must fire — never wrap under.
        let budget = 2u64 * 1024 * 1024 * 1024;
        let g = guardrail(&est, budget);
        assert_eq!(
            g.level,
            GuardrailLevel::Block,
            "an overflowing grid must Block, never wrap under budget (WR-02)"
        );
        assert!(g.detail.contains("exceeds"));
    }

    #[test]
    fn guardrail_ok_warn_block_transitions() {
        // Small grid, huge budget → Ok.
        let small = estimate(1000.0, 10.0, 0, 1, 4); // 10 receivers
        let g_ok = guardrail(&small, u64::MAX);
        assert_eq!(g_ok.level, GuardrailLevel::Ok);
        assert!(
            g_ok.detail.contains("quadruples"),
            "detail must state the spacing/cost relation"
        );

        // A grid whose tensor exceeds the 256 MiB warn threshold but sits under a
        // generous hard budget → Warn (not Block).
        let big = estimate(1_000_000.0, 3.0, 0, 4, 8);
        assert!(big.tensor_bytes > WARN_TENSOR_BYTES);
        let g_warn = guardrail(&big, u64::MAX);
        assert_eq!(g_warn.level, GuardrailLevel::Warn, "under budget → Warn");
        assert!(g_warn.detail.contains("quadruples"));

        // Same grid against a hard budget smaller than the tensor → Block.
        let g_block = guardrail(&big, big.tensor_bytes - 1);
        assert_eq!(
            g_block.level,
            GuardrailLevel::Block,
            "tensor over the hard budget → Block"
        );
        assert!(g_block.detail.contains("exceeds"));
    }
}
