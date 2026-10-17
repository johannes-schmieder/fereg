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
    prefix: Option<PrefixPlan>,
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
enum PrefixPlan {
    Isolated(CategoricalPrefix),
    PortfolioV1,
    PortfolioUnseeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RoutingAblation {
    Diagonal,
    Cells,
    CellsV2,
    CellsV2Df,
    CellsDeferred,
    CellsM,
    CellsMLanes,
    CellsS,
    Direct,
}

impl std::fmt::Display for RoutingPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.break_builder {
            return f.write_str("routing_overlap_break");
        }
        if let Some(prefix) = self.prefix {
            return f.write_str(match prefix {
                PrefixPlan::Isolated(CategoricalPrefix::None) => "routing_prefix_none",
                PrefixPlan::Isolated(CategoricalPrefix::Map4) => "routing_prefix_map4",
                PrefixPlan::Isolated(CategoricalPrefix::CompactIt4) => "routing_prefix_it4",
                PrefixPlan::PortfolioV1 => "routing_portfolio_v1",
                PrefixPlan::PortfolioUnseeded => "routing_portfolio_unseeded",
            });
        }
        if self.overlap {
            return write!(f, "routing_overlap_h{}_k{}", self.checkpoint, self.samples);
        }
        match self.ablation {
            None => write!(f, "routing_h{}_k{}", self.checkpoint, self.samples),
            Some(RoutingAblation::Diagonal) => f.write_str("routing_diagonal"),
            Some(RoutingAblation::Cells) => f.write_str("routing_cells"),
            Some(RoutingAblation::CellsV2) => f.write_str("routing_cells_v2"),
            Some(RoutingAblation::CellsV2Df) => f.write_str("routing_cells_v2_df"),
            Some(RoutingAblation::CellsDeferred) => f.write_str("routing_cells_deferred"),
            Some(RoutingAblation::CellsM) => f.write_str("routing_cells_m"),
            Some(RoutingAblation::CellsMLanes) => f.write_str("routing_cells_m_lanes"),
            Some(RoutingAblation::CellsS) => f.write_str("routing_cells_s"),
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
            "cells_v2" => RoutingAblation::CellsV2,
            "cells_v2_df" => RoutingAblation::CellsV2Df,
            "cells_deferred" => RoutingAblation::CellsDeferred,
            "cells_m" => RoutingAblation::CellsM,
            "cells_m_lanes" => RoutingAblation::CellsMLanes,
            "cells_s" => RoutingAblation::CellsS,
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
            prefix: Some(PrefixPlan::Isolated(prefix)),
            break_builder: false,
        }
    }

    /// Two-FE direct-or-h16/k1 portfolio, or strict MAP4 followed by seeded
    /// incumbent LSMR for more categorical effects. No general pair setup.
    pub fn portfolio_v1() -> Self {
        Self {
            checkpoint: 16,
            samples: 1,
            prefix: Some(PrefixPlan::PortfolioV1),
            ..Self::prefix(CategoricalPrefix::Map4)
        }
    }

    /// Private causal control: V1 two-FE behavior, with strict unseeded LSMR
    /// and no substitute geometry pilot for more categorical effects.
    pub fn portfolio_unseeded() -> Self {
        Self {
            prefix: Some(PrefixPlan::PortfolioUnseeded),
            ..Self::portfolio_v1()
        }
    }

    pub(crate) fn portfolio(self) -> bool {
        matches!(
            self.prefix,
            Some(PrefixPlan::PortfolioV1 | PrefixPlan::PortfolioUnseeded)
        )
    }

    pub(crate) fn strict_categorical_prefix(self) -> bool {
        self.portfolio()
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
        self.prefix.map(|prefix| match prefix {
            PrefixPlan::Isolated(prefix) => prefix,
            PrefixPlan::PortfolioV1 => CategoricalPrefix::Map4,
            PrefixPlan::PortfolioUnseeded => CategoricalPrefix::None,
        })
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

    /// Prefix arms cannot nominate or build a general within pair replacement.
    /// Their two-FE policy may independently have a finite checkpoint.
    pub(crate) fn general_pair_checkpoint(self) -> usize {
        if self.prefix.is_some() {
            usize::MAX
        } else {
            self.checkpoint
        }
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
    // The choice occupies padding in the existing accuracy key, preserving
    // the retained route owner's layout and memory admission.
    accuracy: Option<(u64, bool, PortfolioChoice)>,
    pub attempted: bool,
    pub limit: u64,
    pub used: u64,
    pub restart_reserve: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PortfolioChoice {
    Undecided,
    Direct,
    IncumbentCmg,
}

impl SetupLedger {
    pub fn matches_accuracy(&self, tolerance: f64, final_residual: bool) -> bool {
        self.accuracy.is_some_and(|(bits, final_phase, _)| {
            bits == tolerance.to_bits() && final_phase == final_residual
        })
    }

    pub fn portfolio_choice(&self) -> PortfolioChoice {
        self.accuracy
            .map_or(PortfolioChoice::Undecided, |(_, _, choice)| choice)
    }

    pub fn select_portfolio(&mut self, choice: PortfolioChoice) {
        self.accuracy.as_mut().expect("routing accuracy bound").2 = choice;
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
        if self.matches_accuracy(tolerance, final_residual) {
            return;
        }
        self.accuracy = Some((
            tolerance.to_bits(),
            final_residual,
            PortfolioChoice::Undecided,
        ));
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

    #[test]
    fn portfolio_v1_separates_two_fe_escalation_from_general_prefix() {
        let preset = RoutingPreset::portfolio_v1();
        assert_eq!(preset.to_string(), "routing_portfolio_v1");
        assert!(preset.strict_categorical_prefix());
        assert_eq!(preset.categorical_prefix(), Some(CategoricalPrefix::Map4));
        assert_eq!(preset.checkpoint(), 16);
        assert_eq!(preset.samples(), 1);
        assert_eq!(preset.general_pair_checkpoint(), usize::MAX);
        assert!(!preset.overlaps());
        assert_eq!(preset.kernel(), None);
        let legacy = RoutingPreset::prefix(CategoricalPrefix::Map4);
        assert!(!legacy.strict_categorical_prefix());
        assert_ne!(preset, legacy);
        assert_eq!(legacy.to_string(), "routing_prefix_map4");
        assert_eq!(legacy.checkpoint(), usize::MAX);
        assert_eq!(legacy.general_pair_checkpoint(), usize::MAX);
        let old_pair = RoutingPreset::new(16, 1).unwrap();
        assert!(!old_pair.strict_categorical_prefix());
        assert_eq!(old_pair.general_pair_checkpoint(), 16);
    }

    #[test]
    fn portfolio_unseeded_changes_only_the_general_prefix() {
        let control = RoutingPreset::portfolio_unseeded();
        let v1 = RoutingPreset::portfolio_v1();
        assert_ne!(control, v1);
        assert_eq!(control.to_string(), "routing_portfolio_unseeded");
        assert_eq!(v1.to_string(), "routing_portfolio_v1");
        assert!(control.portfolio() && v1.portfolio());
        assert!(control.strict_categorical_prefix());
        assert_eq!(control.categorical_prefix(), Some(CategoricalPrefix::None));
        assert_eq!(v1.categorical_prefix(), Some(CategoricalPrefix::Map4));
        assert_eq!(control.general_pair_checkpoint(), usize::MAX);
        assert_eq!(control.checkpoint(), v1.checkpoint());
        assert_eq!(control.samples(), v1.samples());
        assert_eq!(control.kernel(), v1.kernel());
        assert_eq!(control.overlaps(), v1.overlaps());
        assert_eq!(control.breaks_builder(), v1.breaks_builder());
        for isolated in [
            CategoricalPrefix::None,
            CategoricalPrefix::Map4,
            CategoricalPrefix::CompactIt4,
        ] {
            let preset = RoutingPreset::prefix(isolated);
            assert!(!preset.portfolio());
            assert!(!preset.strict_categorical_prefix());
            assert_eq!(preset.categorical_prefix(), Some(isolated));
        }
    }

    #[test]
    fn portfolio_v1_preserves_the_original_preset_resource_layout() {
        use std::mem::{align_of, size_of};
        // The original preset has usize/u32 values and four one-byte controls.
        // Keep the owner admission unchanged on each supported pointer width.
        let alignment = align_of::<usize>().max(align_of::<u32>());
        let original_payload = size_of::<usize>() + size_of::<u32>() + 4;
        let original_size = original_payload.div_ceil(alignment) * alignment;
        assert_eq!(size_of::<Option<PrefixPlan>>(), 1);
        assert_eq!(align_of::<RoutingPreset>(), alignment);
        assert_eq!(size_of::<RoutingPreset>(), original_size);
        for prefix in [
            CategoricalPrefix::None,
            CategoricalPrefix::Map4,
            CategoricalPrefix::CompactIt4,
        ] {
            let isolated = RoutingPreset::prefix(prefix);
            assert_eq!(isolated.categorical_prefix(), Some(prefix));
            assert!(!isolated.strict_categorical_prefix());
        }
    }
}
