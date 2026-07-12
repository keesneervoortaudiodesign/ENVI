//! Fine-tier receiver lattice → dense 2-D level grid reconstruction (GRID-04,
//! verifies RESEARCH Assumption A3) — the cached scalar field the isophone
//! tracer contours WITHOUT re-running propagation (SC3, D-04).
//!
//! # Module I/O
//! - **Input:** the fine [`Tier`] from [`crate::tiers::partition`] (a regular
//!   lattice enumerated row-major, `i` outer over x, `j` inner over y, at the
//!   tier's `spacing_m` from a shared lattice origin) and a receiver-major
//!   `dba: &[f64]` slice indexed by [`TierReceiver::global_index`] (the readout
//!   vector produced over ALL receivers in solve order; see [`crate::readout`]).
//! - **Output:** a [`LevelGrid`] — a dense `[rows × cols]` row-major `Vec<f64>`
//!   keyed STRICTLY by lattice index (`round((pos − origin) / spacing)`), never
//!   by nominal position. Any lattice node the fine tier did not introduce
//!   (coarse-tier nodes, or a `global_index` absent from `dba`) is a
//!   `f64::NAN` **no-data hole** the tracer treats as below every break.
//!
//! # Why reconstruct rather than carry the lattice indices
//! [`crate::tiers::partition`] emits receivers as a flat, index-tagged list (the
//! solve()-order contract), not a 2-D array. The fine tier introduces only the
//! ~99% gap nodes (coarse nodes are a strict subset kept by a coarser tier), so
//! this step scatters each fine value back to its `(row, col)` and leaves the
//! coarse-node cells as holes — the coarse readout fills them separately, or the
//! tracer skips them as no-data. The grid origin/spacing are recovered from the
//! fine tier itself (A3: a regular lattice), so a downstream contour never needs
//! the original `partition` arguments.
//!
//! # A3 verification
//! A3 (fine-tier receivers form a regular reconstructable lattice) holds against
//! [`crate::tiers`]: `partition` enumerates the lattice at a single `spacing_m`
//! from one origin, so `round((pos − origin) / spacing)` recovers exact integer
//! indices. Were A3 false (irregular spacing), the round-trip would collide two
//! receivers onto one cell — [`reconstruct_level_grid`] would still not panic,
//! but the plan's acceptance test asserts the injective round-trip, so a
//! regression would surface there.
//!
//! `#![deny(unsafe_code)]` (crate-wide); typed/no-data on bad input, never a panic.

use crate::tiers::Tier;

/// A dense 2-D scalar level grid over the fine-tier lattice.
///
/// `values` is row-major (`values[row * cols + col]`); a `f64::NAN` entry is a
/// **no-data hole** (a lattice node the fine tier did not introduce). Node
/// `(row, col)` sits at SceneXY `[origin[0] + col * spacing_m, origin[1] + row *
/// spacing_m]` — `col` indexes the x axis, `row` the y axis.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelGrid {
    /// Number of lattice rows (y axis).
    pub rows: usize,
    /// Number of lattice columns (x axis).
    pub cols: usize,
    /// SceneXY position `[x, y]` of node `(row = 0, col = 0)`.
    pub origin: [f64; 2],
    /// Lattice spacing in meters (equal on both axes).
    pub spacing_m: f64,
    /// Row-major values; `f64::NAN` marks a no-data hole.
    pub values: Vec<f64>,
}

impl LevelGrid {
    /// An empty grid (no lattice) — the degenerate result for an empty or
    /// non-reconstructable fine tier.
    #[must_use]
    pub fn empty(spacing_m: f64) -> Self {
        Self {
            rows: 0,
            cols: 0,
            origin: [0.0, 0.0],
            spacing_m,
            values: Vec::new(),
        }
    }

    /// `true` when the grid holds no lattice nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows == 0 || self.cols == 0
    }

    /// The finite value at `(row, col)`, or `None` for an out-of-range index or a
    /// no-data hole (`NAN`).
    #[must_use]
    pub fn at(&self, row: usize, col: usize) -> Option<f64> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let v = self.values[row * self.cols + col];
        if v.is_finite() { Some(v) } else { None }
    }

    /// SceneXY position `[x, y]` of node `(row, col)`.
    #[must_use]
    pub fn node_pos(&self, row: usize, col: usize) -> [f64; 2] {
        [
            self.origin[0] + col as f64 * self.spacing_m,
            self.origin[1] + row as f64 * self.spacing_m,
        ]
    }
}

/// Reconstruct the fine-tier lattice into a dense index-keyed [`LevelGrid`].
///
/// `fine_tier` is the [`crate::tiers::TierKind::Fine`] tier; `dba` is the
/// receiver-major readout vector indexed by [`TierReceiver::global_index`]. Each
/// fine receiver's `position` is mapped back to integer `(col, row)` lattice
/// indices via `round((pos − origin) / spacing)` (origin = the componentwise min
/// corner of the fine tier's receivers) and its `dba[global_index]` value is
/// scattered into the dense grid. Nodes with no fine receiver — and receivers
/// whose `global_index` is out of `dba`'s range — are left as `f64::NAN` holes.
///
/// Returns [`LevelGrid::empty`] (no panic) when the fine tier is empty or its
/// spacing is non-finite/non-positive (a degenerate, non-reconstructable lattice).
#[must_use]
pub fn reconstruct_level_grid(fine_tier: &Tier, dba: &[f64]) -> LevelGrid {
    let spacing = fine_tier.spacing_m.unwrap_or(f64::NAN);
    if !(spacing.is_finite() && spacing > 0.0) || fine_tier.receivers.is_empty() {
        return LevelGrid::empty(if spacing.is_finite() { spacing } else { 0.0 });
    }

    // Grid origin = componentwise min corner of the fine-tier receivers.
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for r in &fine_tier.receivers {
        min_x = min_x.min(r.position[0]);
        min_y = min_y.min(r.position[1]);
        max_x = max_x.max(r.position[0]);
        max_y = max_y.max(r.position[1]);
    }
    if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
        return LevelGrid::empty(spacing);
    }

    let origin = [min_x, min_y];
    // Lattice extent in cells (inclusive) → node counts.
    let cols = ((max_x - min_x) / spacing).round() as i64 + 1;
    let rows = ((max_y - min_y) / spacing).round() as i64 + 1;
    if cols <= 0 || rows <= 0 {
        return LevelGrid::empty(spacing);
    }
    let (cols, rows) = (cols as usize, rows as usize);

    let mut values = vec![f64::NAN; rows * cols];
    for r in &fine_tier.receivers {
        let col = ((r.position[0] - origin[0]) / spacing).round();
        let row = ((r.position[1] - origin[1]) / spacing).round();
        // Guard the round-trip (A3): a well-formed lattice lands in range.
        if !(col.is_finite() && row.is_finite()) || col < 0.0 || row < 0.0 {
            continue;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= cols || row >= rows {
            continue;
        }
        // Value keyed by global_index (receiver-major readout vector); an index
        // beyond `dba` stays a no-data hole rather than panicking.
        if let Some(&v) = dba.get(r.global_index) {
            values[row * cols + col] = v;
        }
    }

    LevelGrid {
        rows,
        cols,
        origin,
        spacing_m: spacing,
        values,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiers::{Rect, TierKind, partition};

    #[test]
    fn fine_tier_reconstructs_to_an_index_keyed_grid_with_exact_values() {
        // fine=10 over [0,100]² → 11×11 lattice; one coarse tier k=10 (the 4
        // corners) so the fine tier carries the 117 gap nodes.
        let plan = partition(
            10.0,
            [0.0, 0.0],
            Rect {
                min: [0.0, 0.0],
                max: [100.0, 100.0],
            },
            &[],
            &[10],
        );
        let total = plan.total_receivers();
        // Synthetic readout keyed by global_index: value = 100 + global_index.
        let dba: Vec<f64> = (0..total).map(|i| 100.0 + i as f64).collect();

        let fine = plan
            .tiers
            .iter()
            .find(|t| t.kind == TierKind::Fine)
            .expect("fine tier present");
        let grid = reconstruct_level_grid(fine, &dba);

        assert_eq!(grid.rows, 11);
        assert_eq!(grid.cols, 11);
        assert_eq!(grid.origin, [0.0, 0.0]);
        assert_eq!(grid.spacing_m, 10.0);

        // Every fine receiver lands at its expected (row, col) with its value.
        for r in &fine.receivers {
            let col = ((r.position[0] - grid.origin[0]) / grid.spacing_m).round() as usize;
            let row = ((r.position[1] - grid.origin[1]) / grid.spacing_m).round() as usize;
            assert_eq!(
                grid.at(row, col),
                Some(100.0 + r.global_index as f64),
                "fine receiver {} at ({row},{col})",
                r.global_index
            );
        }

        // The four coarse corners are no-data holes (fine tier never introduced
        // them): (0,0), (0,10), (10,0), (10,10).
        for (row, col) in [(0, 0), (0, 10), (10, 0), (10, 10)] {
            assert_eq!(
                grid.at(row, col),
                None,
                "coarse corner ({row},{col}) is a hole"
            );
        }

        // Exactly the 117 fine nodes are populated; the rest are holes.
        let filled = grid.values.iter().filter(|v| v.is_finite()).count();
        assert_eq!(filled, 117);
    }

    #[test]
    fn empty_lattice_returns_an_empty_grid_without_panic() {
        // A degenerate fine spacing yields no lattice points.
        let plan = partition(
            0.0,
            [0.0, 0.0],
            Rect {
                min: [0.0, 0.0],
                max: [100.0, 100.0],
            },
            &[],
            &[],
        );
        let fine = plan
            .tiers
            .iter()
            .find(|t| t.kind == TierKind::Fine)
            .expect("fine tier present");
        assert!(fine.receivers.is_empty());
        let grid = reconstruct_level_grid(fine, &[]);
        assert!(grid.is_empty());
        assert_eq!(grid.values.len(), 0);
        assert_eq!(grid.at(0, 0), None);
    }

    #[test]
    fn out_of_range_global_index_stays_a_hole_never_panics() {
        // A single fine node whose global_index exceeds a too-short dba slice.
        let plan = partition(
            10.0,
            [0.0, 0.0],
            Rect {
                min: [0.0, 0.0],
                max: [20.0, 20.0],
            },
            &[],
            &[],
        );
        let fine = plan
            .tiers
            .iter()
            .find(|t| t.kind == TierKind::Fine)
            .unwrap();
        // Deliberately empty dba: every node becomes a hole, no panic.
        let grid = reconstruct_level_grid(fine, &[]);
        assert!(!grid.is_empty());
        assert!(grid.values.iter().all(|v| v.is_nan()));
    }
}
