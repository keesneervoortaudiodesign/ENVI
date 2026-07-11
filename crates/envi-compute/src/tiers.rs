//! The hierarchical `points ⊂ coarse ⊂ fine` receiver partition (D-05/D-06) —
//! pure lattice geometry, physics-free.
//!
//! # The subset invariant (D-05)
//!
//! Preview tiers are coarser multiples of the final (fine) spacing:
//! `coarse = k · fine` with integer `k` and a **shared lattice origin**. So a
//! fine lattice point at index `(i, j)` is *also* a coarse-`k` point iff
//! `i % k == 0 && j % k == 0`. Coarse points are therefore a strict subset of
//! the fine lattice — kept, never recomputed. [`partition`] emits the tiers in
//! solve order `[points, coarse…, fine]`, where the fine tier lists ONLY the
//! ~99% gap points not already carried by a coarser tier (no receiver appears
//! twice). The coarse-multiples list is data-driven, so an intermediate tier
//! (e.g. 50 m between 100 m and 10 m) is a one-element config change
//! (10-RESEARCH Open Q2).
//!
//! # Global indices (solve()'s receiver-major contract)
//!
//! Receivers are numbered sequentially in **emission order** (points first, then
//! each coarse tier coarsest→finest, then fine), row-major within each tier. So
//! concatenating the tiers yields a strictly increasing global index — exactly
//! the non-decreasing-receiver order `envi_engine::solver::solve` requires.

/// An axis-aligned calc-area rectangle in SceneXY meters (inclusive bounds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Minimum corner `[x, y]`.
    pub min: [f64; 2],
    /// Maximum corner `[x, y]`.
    pub max: [f64; 2],
}

/// Which resolution tier a receiver belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierKind {
    /// Explicit discrete receiver points (no spacing).
    Points,
    /// A coarse preview lattice (`k · fine`).
    Coarse,
    /// The final fine lattice (gap points only — coarse excluded).
    Fine,
}

/// One receiver in a tier: its global index (receiver-major) and SceneXY `[x, y]`.
///
/// Height (`z`) and any UUID are added at job-assembly / in TS respectively — the
/// tier layer mints no ids (10-RESEARCH Pitfall 9) and is purely 2-D lattice math.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TierReceiver {
    /// Global receiver index (unique, assigned in emission order).
    pub global_index: usize,
    /// SceneXY position `[x, y]`, meters.
    pub position: [f64; 2],
}

/// One emitted tier: its kind, spacing (`None` for discrete points), and the
/// receivers it introduces (NOT already carried by a coarser tier).
#[derive(Debug, Clone, PartialEq)]
pub struct Tier {
    /// The tier kind.
    pub kind: TierKind,
    /// Lattice spacing in meters (`None` for the discrete-points tier).
    pub spacing_m: Option<f64>,
    /// The receivers this tier introduces, row-major, with sequential indices.
    pub receivers: Vec<TierReceiver>,
}

/// The full hierarchical plan: tiers in solve order `[points, coarse…, fine]`.
#[derive(Debug, Clone, PartialEq)]
pub struct TierPlan {
    /// The ordered tiers.
    pub tiers: Vec<Tier>,
}

impl TierPlan {
    /// Total receiver count across every tier.
    #[must_use]
    pub fn total_receivers(&self) -> usize {
        self.tiers.iter().map(|t| t.receivers.len()).sum()
    }
}

/// Partition a calc area into the hierarchical tier plan.
///
/// `fine_spacing_m` is the user's final spacing (D-06); `lattice_origin` anchors
/// the shared lattice; `area` bounds it; `discrete_points` are explicit receiver
/// positions (the points tier); `coarse_multiples` lists the integer coarse
/// factors `k` (e.g. `[10]` → one 100 m preview tier; `[10, 5]` → 100 m + 50 m).
/// Multiples `< 2` are ignored (they would duplicate the fine tier); the list is
/// de-duplicated and ordered coarsest-first.
///
/// A fine lattice point `(i, j)` joins the **coarsest** tier whose factor divides
/// both `i` and `j`; points divisible by none form the fine tier. No receiver is
/// emitted twice (D-05: coarse kept, fine computes only the gaps).
#[must_use]
pub fn partition(
    fine_spacing_m: f64,
    lattice_origin: [f64; 2],
    area: Rect,
    discrete_points: &[[f64; 2]],
    coarse_multiples: &[usize],
) -> TierPlan {
    // Coarse factors, coarsest-first, de-duplicated, `k >= 2` only.
    let mut multiples: Vec<usize> = coarse_multiples
        .iter()
        .copied()
        .filter(|&k| k >= 2)
        .collect();
    multiples.sort_unstable_by(|a, b| b.cmp(a));
    multiples.dedup();

    // Lattice index bounds (guard a degenerate spacing → no lattice points).
    let (ox, oy) = (lattice_origin[0], lattice_origin[1]);
    let mut coarse_buckets: Vec<Vec<[f64; 2]>> = vec![Vec::new(); multiples.len()];
    let mut fine_bucket: Vec<[f64; 2]> = Vec::new();

    if fine_spacing_m.is_finite() && fine_spacing_m > 0.0 {
        let i_min = ((area.min[0] - ox) / fine_spacing_m).ceil() as i64;
        let i_max = ((area.max[0] - ox) / fine_spacing_m).floor() as i64;
        let j_min = ((area.min[1] - oy) / fine_spacing_m).ceil() as i64;
        let j_max = ((area.max[1] - oy) / fine_spacing_m).floor() as i64;

        // Row-major enumeration (i outer, j inner) → receiver-major within a tier.
        for i in i_min..=i_max {
            for j in j_min..=j_max {
                let pos = [
                    ox + i as f64 * fine_spacing_m,
                    oy + j as f64 * fine_spacing_m,
                ];
                // The coarsest factor dividing both i and j wins; else fine.
                let mut placed = false;
                for (bi, &k) in multiples.iter().enumerate() {
                    let k = k as i64;
                    if i.rem_euclid(k) == 0 && j.rem_euclid(k) == 0 {
                        coarse_buckets[bi].push(pos);
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    fine_bucket.push(pos);
                }
            }
        }
    }

    // Emit tiers in solve order, assigning sequential global indices.
    let mut tiers = Vec::with_capacity(multiples.len() + 2);
    let mut gi = 0usize;

    let mut point_recs = Vec::with_capacity(discrete_points.len());
    for &p in discrete_points {
        point_recs.push(TierReceiver {
            global_index: gi,
            position: p,
        });
        gi += 1;
    }
    tiers.push(Tier {
        kind: TierKind::Points,
        spacing_m: None,
        receivers: point_recs,
    });

    for (bi, &k) in multiples.iter().enumerate() {
        let mut recs = Vec::with_capacity(coarse_buckets[bi].len());
        for &p in &coarse_buckets[bi] {
            recs.push(TierReceiver {
                global_index: gi,
                position: p,
            });
            gi += 1;
        }
        tiers.push(Tier {
            kind: TierKind::Coarse,
            spacing_m: Some(k as f64 * fine_spacing_m),
            receivers: recs,
        });
    }

    let mut fine_recs = Vec::with_capacity(fine_bucket.len());
    for &p in &fine_bucket {
        fine_recs.push(TierReceiver {
            global_index: gi,
            position: p,
        });
        gi += 1;
    }
    tiers.push(Tier {
        kind: TierKind::Fine,
        spacing_m: Some(fine_spacing_m),
        receivers: fine_recs,
    });

    TierPlan { tiers }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_grid(p: [f64; 2], origin: [f64; 2], spacing: f64) -> bool {
        let i = (p[0] - origin[0]) / spacing;
        let j = (p[1] - origin[1]) / spacing;
        (i - i.round()).abs() < 1e-9 && (j - j.round()).abs() < 1e-9
    }

    #[test]
    fn coarse_is_a_strict_subset_of_the_fine_lattice() {
        // fine=10 over [0,100]² → 11×11 lattice; coarse k=10 → the 4 corners.
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
        let coarse = &plan.tiers[1];
        assert_eq!(coarse.kind, TierKind::Coarse);
        assert_eq!(coarse.spacing_m, Some(100.0));
        assert_eq!(coarse.receivers.len(), 4, "corners of the 11×11 lattice");
        // Every coarse receiver lies on the fine lattice (strict subset, D-05).
        for r in &coarse.receivers {
            assert!(
                on_grid(r.position, [0.0, 0.0], 10.0),
                "coarse point {:?} must be a fine-lattice node",
                r.position
            );
        }
    }

    #[test]
    fn fine_tier_excludes_every_coarse_point_no_recompute() {
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
        let coarse: std::collections::BTreeSet<(u64, u64)> = plan.tiers[1]
            .receivers
            .iter()
            .map(|r| (r.position[0].to_bits(), r.position[1].to_bits()))
            .collect();
        let fine = &plan.tiers[2];
        assert_eq!(fine.kind, TierKind::Fine);
        // 121 lattice points − 4 coarse = 117 gap points.
        assert_eq!(fine.receivers.len(), 117);
        for r in &fine.receivers {
            let key = (r.position[0].to_bits(), r.position[1].to_bits());
            assert!(
                !coarse.contains(&key),
                "fine tier must NOT re-emit coarse point {:?}",
                r.position
            );
        }
    }

    #[test]
    fn global_indices_are_non_decreasing_across_job_order() {
        // Two discrete points + the hierarchical lattice.
        let plan = partition(
            10.0,
            [0.0, 0.0],
            Rect {
                min: [0.0, 0.0],
                max: [100.0, 100.0],
            },
            &[[5.0, 5.0], [15.0, 25.0]],
            &[10],
        );
        // Flatten in emission order; indices must be exactly 0,1,2,… (contiguous
        // and strictly increasing ⇒ non-decreasing receiver-major order).
        let mut expected = 0usize;
        for tier in &plan.tiers {
            for r in &tier.receivers {
                assert_eq!(r.global_index, expected, "indices must be contiguous");
                expected += 1;
            }
        }
        // points(2) + coarse(4) + fine(117) = 123.
        assert_eq!(plan.total_receivers(), 123);
        assert_eq!(plan.tiers[0].kind, TierKind::Points);
        assert_eq!(plan.tiers[0].receivers.len(), 2);
    }

    #[test]
    fn intermediate_tier_is_one_line_of_config() {
        // coarse_multiples = [10, 5] → a 100 m tier AND an intermediate 50 m tier.
        let plan = partition(
            10.0,
            [0.0, 0.0],
            Rect {
                min: [0.0, 0.0],
                max: [100.0, 100.0],
            },
            &[],
            &[10, 5],
        );
        // Order: points, coarse k=10 (100 m), coarse k=5 (50 m), fine.
        assert_eq!(plan.tiers.len(), 4);
        assert_eq!(plan.tiers[1].spacing_m, Some(100.0));
        assert_eq!(plan.tiers[1].receivers.len(), 4, "k=10 corners");
        assert_eq!(plan.tiers[2].spacing_m, Some(50.0));
        assert_eq!(
            plan.tiers[2].receivers.len(),
            5,
            "k=5 nodes not already in k=10"
        );
        assert_eq!(plan.tiers[3].kind, TierKind::Fine);
        assert_eq!(plan.tiers[3].receivers.len(), 121 - 4 - 5);
        // No point appears in two tiers (disjoint across the whole plan).
        let mut seen = std::collections::BTreeSet::new();
        for tier in &plan.tiers {
            for r in &tier.receivers {
                let key = (r.position[0].to_bits(), r.position[1].to_bits());
                assert!(seen.insert(key), "no receiver may repeat across tiers");
            }
        }
    }
}
