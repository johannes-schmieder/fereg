//! Candidate routing policy. The ordinary default remains the qualified incumbent.
//!
//! Iterations belong to a projection action; optional setup belongs to its
//! fixed topology/weight/accuracy session. In particular, sharing setup across
//! Low chunks never takes iterations from a later chunk.

use crate::error::{invalid_argument, Result};

/// Finite developer qualification arm, not public estimator syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingPreset {
    checkpoint: usize,
    samples: u32,
    ablation: Option<RoutingAblation>,
    overlap: bool,
    prefix: Option<CategoricalPrefix>,
    break_builder: bool,
}

/// Isolated many-FE prefix qualification arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoricalPrefix {
    None,
    Map4,
    CompactIt4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RoutingAblation {
    Diagonal,
    Cells,
    Direct,
}

impl std::fmt::Display for RoutingPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.break_builder {
            return f.write_str("routing_overlap_break");
        }
        if let Some(prefix) = self.prefix {
            return f.write_str(match prefix {
                CategoricalPrefix::None => "routing_prefix_none",
                CategoricalPrefix::Map4 => "routing_prefix_map4",
                CategoricalPrefix::CompactIt4 => "routing_prefix_it4",
            });
        }
        if self.overlap {
            return write!(f, "routing_overlap_h{}_k{}", self.checkpoint, self.samples);
        }
        match self.ablation {
            None => write!(f, "routing_h{}_k{}", self.checkpoint, self.samples),
            Some(RoutingAblation::Diagonal) => f.write_str("routing_diagonal"),
            Some(RoutingAblation::Cells) => f.write_str("routing_cells"),
            Some(RoutingAblation::Direct) => f.write_str("routing_direct"),
        }
    }
}

impl RoutingPreset {
    pub fn new(checkpoint: usize, samples: u32) -> Result<Self> {
        if !matches!(checkpoint, 4 | 8 | 16 | 32) || samples > 2 {
            return Err(invalid_argument(
                "invalid solver-routing qualification preset",
            ));
        }
        Ok(Self {
            checkpoint,
            samples,
            ablation: None,
            overlap: false,
            prefix: None,
            break_builder: false,
        })
    }

    /// Independent kernel experiment, selected before the finite h/k tournament.
    pub fn ablation(name: &str) -> Result<Self> {
        let ablation = match name {
            "diagonal" => RoutingAblation::Diagonal,
            "cells" => RoutingAblation::Cells,
            "direct" => RoutingAblation::Direct,
            _ => return Err(invalid_argument("invalid solver-routing kernel experiment")),
        };
        Ok(Self {
            checkpoint: usize::MAX,
            samples: 0,
            ablation: Some(ablation),
            overlap: false,
            prefix: None,
            break_builder: false,
        })
    }

    pub fn overlap(checkpoint: usize, samples: u32) -> Result<Self> {
        let mut preset = Self::new(checkpoint, samples)?;
        if checkpoint <= 4 {
            return Err(invalid_argument(
                "overlap routing requires a checkpoint after four iterations",
            ));
        }
        preset.overlap = true;
        Ok(preset)
    }

    pub fn prefix(prefix: CategoricalPrefix) -> Self {
        Self {
            checkpoint: usize::MAX,
            samples: 0,
            ablation: None,
            overlap: false,
            prefix: Some(prefix),
            break_builder: false,
        }
    }

    #[doc(hidden)]
    pub fn overlap_break() -> Self {
        let mut preset = Self::overlap(8, 1).expect("fixed developer overlap preset");
        preset.break_builder = true;
        preset
    }

    #[doc(hidden)]
    pub fn breaks_builder(self) -> bool {
        self.break_builder
    }

    pub(crate) fn categorical_prefix(self) -> Option<CategoricalPrefix> {
        self.prefix
    }

    #[doc(hidden)]
    pub fn overlaps(self) -> bool {
        self.overlap
    }

    pub(super) fn kernel(self) -> Option<RoutingAblation> {
        self.ablation
    }

    pub(crate) fn checkpoint(self) -> usize {
        self.checkpoint
    }
    pub(crate) fn samples(self) -> u32 {
        self.samples
    }
}

/// Checked deterministic currency: conservative scalar arithmetic/access work,
/// never elapsed time. A full initial-batch incumbent iteration fixes W0 once.
pub(super) fn schur_iteration_work(
    rows: usize,
    left: usize,
    right: usize,
    rhs: usize,
    stride: usize,
) -> Option<u64> {
    // Two Schur traversals: 5 shared row operations and 5 per RHS each.
    // Left division, right initialization/dot/step/energy/proxy/direction,
    // complete bank fills (including inactive capacity) and scalar guards.
    // A scalar read/write, arithmetic op, sqrt/division or predicate costs one;
    // FMA costs one. Address arithmetic and loop mechanics are excluded.
    let (n, p, q, r, s) = (
        rows as u128,
        left as u128,
        right as u128,
        rhs as u128,
        stride as u128,
    );
    let work = n
        .checked_mul(10)?
        .checked_mul(r)?
        .checked_add(n.checked_mul(10)?)?
        .checked_add(
            p.checked_mul(4)?
                .checked_add(q.checked_mul(64)?)?
                .checked_mul(r)?,
        )?
        .checked_add(p.checked_add(q.checked_mul(3)?)?.checked_mul(s)?)?
        .checked_add(p.checked_mul(8)?)?
        .checked_add(q.checked_mul(12)?)?
        .checked_add(r.checked_mul(64)?)?;
    u64::try_from(work).ok()
}

/// A single optional attempt is shared by all committed RHS chunks. The owner
/// itself binds canonical sample, topology and weights; the accuracy key starts
/// a new setup allowance for final residual projection, retaining a built M.
pub(super) struct SetupLedger {
    accuracy: Option<(u64, bool)>,
    pub attempted: bool,
    pub limit: u64,
    pub used: u64,
    pub restart_reserve: u64,
}

impl SetupLedger {
    pub fn matches_accuracy(&self, tolerance: f64, final_residual: bool) -> bool {
        self.accuracy == Some((tolerance.to_bits(), final_residual))
    }

    pub fn new() -> Self {
        Self {
            accuracy: None,
            attempted: false,
            limit: 0,
            used: 0,
            restart_reserve: 0,
        }
    }

    pub fn begin(&mut self, tolerance: f64, final_residual: bool, iteration_work: Option<u64>) {
        let key = (tolerance.to_bits(), final_residual);
        if self.accuracy == Some(key) {
            return;
        }
        self.accuracy = Some(key);
        self.used = 0;
        self.attempted = false;
        // Reserve four incumbent iterations for reconstruction and recurrence
        // restart. The final original-design certificate keeps its existing
        // reservation; no extra certificate is run merely to switch at h.
        // The two-FE handoff bound is
        // 28*n*r + 24*n + (16*p + 36*q)*r + (4*p + 4*q)*s
        // + 32*p + 32*q + 64*r <= 4*W0, term by term. The first
        // CMG application has a separate checked startup charge.
        let allowance = iteration_work.and_then(|w| Some((w.checked_mul(32)?, w.checked_mul(4)?)));
        (self.limit, self.restart_reserve) = allowance.unwrap_or((0, 0));
    }

    pub fn builder_allowance(&self) -> u64 {
        self.limit
            .saturating_sub(self.used)
            .saturating_sub(self.restart_reserve)
    }

    pub fn charge(&mut self, work: u64) -> Result<()> {
        self.used = self
            .used
            .checked_add(work)
            .filter(|&total| total <= self.limit)
            .ok_or_else(|| {
                invalid_argument("solver-routing setup exceeded its checked allowance")
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_share_setup_but_new_accuracy_gets_own_allowance() {
        let mut ledger = SetupLedger::new();
        ledger.begin(1e-8, false, Some(100));
        ledger.charge(600).unwrap();
        ledger.attempted = true;
        ledger.begin(1e-8, false, Some(20));
        assert_eq!(
            (ledger.limit, ledger.used, ledger.attempted),
            (3200, 600, true)
        );
        ledger.begin(1e-14, true, Some(20));
        assert_eq!(
            (ledger.limit, ledger.used, ledger.attempted),
            (640, 0, false)
        );
        assert_eq!(ledger.builder_allowance(), 560);
    }

    #[test]
    fn overflow_and_exhaustion_decline_without_wrapping() {
        let mut ledger = SetupLedger::new();
        ledger.begin(1e-8, false, Some(u64::MAX));
        assert_eq!(ledger.builder_allowance(), 0);
        assert!(ledger.charge(1).is_err());
        assert!(RoutingPreset::new(7, 0).is_err());
        assert!(RoutingPreset::new(8, 3).is_err());
    }
}
