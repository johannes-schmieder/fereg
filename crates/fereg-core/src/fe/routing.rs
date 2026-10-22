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
    IncrementalMany,
    IncrementalTwo,
    RemainingTwo,
    GatesTwo,
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
            return f.write_str("routing_builder_break");
        }
        if let Some(prefix) = self.prefix {
            return f.write_str(match prefix {
                PrefixPlan::Isolated(CategoricalPrefix::None) => "routing_prefix_none",
                PrefixPlan::Isolated(CategoricalPrefix::Map4) => "routing_prefix_map4",
                PrefixPlan::Isolated(CategoricalPrefix::CompactIt4) => "routing_prefix_it4",
                PrefixPlan::PortfolioV1 => "routing_portfolio_v1",
                PrefixPlan::PortfolioUnseeded => "routing_portfolio_unseeded",
                PrefixPlan::IncrementalMany => "routing_incremental_many",
                PrefixPlan::IncrementalTwo => "routing_incremental_two",
                PrefixPlan::GatesTwo => "routing_two_gates_v1",
                PrefixPlan::RemainingTwo => match self.checkpoint {
                    0 => "routing_two_remaining_v1",
                    16 => "routing_two_remaining_force16",
                    32 => "routing_two_remaining_force32",
                    _ => "routing_two_remaining_never",
                },
            });
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
            prefix: None,
            break_builder: false,
        })
    }

    pub fn prefix(prefix: CategoricalPrefix) -> Self {
        Self {
            checkpoint: usize::MAX,
            samples: 0,
            ablation: None,
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

    /// Isolated qualification of strict unseeded LSMR for three or more FEs.
    pub fn incremental_many() -> Self {
        Self {
            prefix: Some(PrefixPlan::IncrementalMany),
            ..Self::portfolio_unseeded()
        }
    }

    /// Isolated qualification of the existing two-FE direct-or-h16/k1 policy.
    pub fn incremental_two() -> Self {
        Self {
            prefix: Some(PrefixPlan::IncrementalTwo),
            ..Self::portfolio_unseeded()
        }
    }

    /// Successor two-FE policy. Historical fixed-checkpoint arms stay separate.
    pub fn remaining_two() -> Self {
        Self {
            checkpoint: 0,
            prefix: Some(PrefixPlan::RemainingTwo),
            ..Self::portfolio_unseeded()
        }
    }

    /// Prospective matched-state calibration controls for the same policy.
    pub fn remaining_two_control(checkpoint: Option<usize>) -> Result<Self> {
        if checkpoint.is_some_and(|age| !matches!(age, 16 | 32)) {
            return Err(invalid_argument(
                "invalid remaining-cost calibration control",
            ));
        }
        Ok(Self {
            checkpoint: checkpoint.unwrap_or(usize::MAX),
            ..Self::remaining_two()
        })
    }

    pub fn gates_two() -> Self {
        Self {
            checkpoint: 0,
            prefix: Some(PrefixPlan::GatesTwo),
            ..Self::portfolio_unseeded()
        }
    }

    pub(crate) fn gates(self) -> bool {
        self.prefix == Some(PrefixPlan::GatesTwo)
    }

    pub(crate) fn requires_economic_grant(self) -> bool {
        self.remaining_mode().is_some() || self.gates()
    }

    pub(crate) fn remaining_mode(self) -> Option<super::routing_remaining::Mode> {
        (self.prefix == Some(PrefixPlan::RemainingTwo)).then_some(match self.checkpoint {
            0 => super::routing_remaining::Mode::Predict,
            16 | 32 => super::routing_remaining::Mode::Force(self.checkpoint),
            _ => super::routing_remaining::Mode::Never,
        })
    }

    /// Resolve family selectors before allocating any optional preparation.
    /// None means the qualified incumbent; all older experiments pass through.
    pub(crate) fn resolve_ols(
        self,
        solver: super::FeSolverKind,
        preconditioner: super::LsmrPreconditioner,
        categorical_effects: Option<usize>,
    ) -> Option<Self> {
        let eligible = match self.prefix {
            Some(PrefixPlan::IncrementalMany) => categorical_effects.is_some_and(|n| n >= 3),
            Some(PrefixPlan::IncrementalTwo) => categorical_effects == Some(2),
            Some(PrefixPlan::RemainingTwo | PrefixPlan::GatesTwo) => categorical_effects == Some(2),
            _ => return Some(self),
        };
        (eligible
            && solver == super::FeSolverKind::Auto
            && preconditioner == super::LsmrPreconditioner::Auto)
            .then(|| {
                if self.requires_economic_grant() {
                    self
                } else {
                    Self::portfolio_unseeded()
                }
            })
    }

    pub(crate) fn portfolio(self) -> bool {
        matches!(
            self.prefix,
            Some(
                PrefixPlan::PortfolioV1
                    | PrefixPlan::PortfolioUnseeded
                    | PrefixPlan::RemainingTwo
                    | PrefixPlan::GatesTwo
            )
        )
    }

    pub(crate) fn strict_categorical_prefix(self) -> bool {
        self.portfolio()
    }

    /// Developer Break arm for the synchronous bounded CMG builder: the h4/k1
    /// two-FE build parks at its first cancellation poll after executed
    /// construction work until the host requests cancellation.
    #[doc(hidden)]
    pub fn builder_break() -> Self {
        let mut preset = Self::new(4, 1).expect("fixed developer builder Break preset");
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
            PrefixPlan::PortfolioUnseeded
            | PrefixPlan::IncrementalMany
            | PrefixPlan::IncrementalTwo
            | PrefixPlan::RemainingTwo
            | PrefixPlan::GatesTwo => CategoricalPrefix::None,
        })
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

/// Conservative scalar arithmetic/access count of one packed incumbent Schur
/// iteration, never elapsed time. It is the reference `W_ref` of the small
/// direct criterion `A <= 4 W_ref`; it is not a construction allowance.
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
/// itself binds canonical sample, topology and weights; a new accuracy key
/// (for example final residual projection) starts a new optional attempt while
/// retaining a built M. There is no construction-work allowance.
pub(super) struct SetupLedger {
    // The portfolio choice is bound to its accuracy key.
    accuracy: Option<(u64, bool, PortfolioChoice)>,
    pub attempted: bool,
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
        }
    }

    pub fn begin(&mut self, tolerance: f64, final_residual: bool) {
        if self.matches_accuracy(tolerance, final_residual) {
            return;
        }
        self.accuracy = Some((
            tolerance.to_bits(),
            final_residual,
            PortfolioChoice::Undecided,
        ));
        self.attempted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_selectors_resolve_only_their_auto_categorical_family() {
        use super::super::{FeSolverKind, LsmrPreconditioner};
        for (preset, name, many) in [
            (
                RoutingPreset::incremental_many(),
                "routing_incremental_many",
                true,
            ),
            (
                RoutingPreset::incremental_two(),
                "routing_incremental_two",
                false,
            ),
            (
                RoutingPreset::remaining_two(),
                "routing_two_remaining_v1",
                false,
            ),
            (
                RoutingPreset::remaining_two_control(None).unwrap(),
                "routing_two_remaining_never",
                false,
            ),
            (
                RoutingPreset::remaining_two_control(Some(16)).unwrap(),
                "routing_two_remaining_force16",
                false,
            ),
            (
                RoutingPreset::remaining_two_control(Some(32)).unwrap(),
                "routing_two_remaining_force32",
                false,
            ),
            (RoutingPreset::gates_two(), "routing_two_gates_v1", false),
        ] {
            assert_eq!(preset.to_string(), name);
            for effects in [None, Some(0), Some(1), Some(2), Some(3), Some(7)] {
                for solver in [
                    FeSolverKind::Auto,
                    FeSolverKind::Map,
                    FeSolverKind::Lsmr,
                    FeSolverKind::Cmg,
                    FeSolverKind::CmgSchur,
                    FeSolverKind::Cg,
                    FeSolverKind::IronsTuck,
                    FeSolverKind::Squarem,
                ] {
                    for preconditioner in [
                        LsmrPreconditioner::Auto,
                        LsmrPreconditioner::Off,
                        LsmrPreconditioner::Diagonal,
                        LsmrPreconditioner::Schwarz,
                    ] {
                        let eligible = solver == FeSolverKind::Auto
                            && preconditioner == LsmrPreconditioner::Auto
                            && effects.is_some_and(|n| if many { n >= 3 } else { n == 2 });
                        assert_eq!(
                            preset.resolve_ols(solver, preconditioner, effects),
                            eligible.then(|| if preset.requires_economic_grant() {
                                preset
                            } else {
                                RoutingPreset::portfolio_unseeded()
                            })
                        );
                        // Historical hidden experiments keep their existing contract.
                        let old = RoutingPreset::portfolio_unseeded();
                        assert_eq!(old.resolve_ols(solver, preconditioner, effects), Some(old));
                    }
                }
            }
        }
    }

    #[test]
    fn chunks_share_one_attempt_but_new_accuracy_gets_its_own() {
        let mut ledger = SetupLedger::new();
        ledger.begin(1e-8, false);
        ledger.attempted = true;
        ledger.select_portfolio(PortfolioChoice::Direct);
        ledger.begin(1e-8, false);
        assert!(ledger.attempted);
        assert_eq!(ledger.portfolio_choice(), PortfolioChoice::Direct);
        ledger.begin(1e-14, true);
        assert!(!ledger.attempted);
        assert_eq!(ledger.portfolio_choice(), PortfolioChoice::Undecided);
        assert!(RoutingPreset::new(7, 0).is_err());
        assert!(RoutingPreset::new(8, 3).is_err());
    }

    #[test]
    fn builder_break_is_a_synchronous_h4_k1_developer_arm() {
        let preset = RoutingPreset::builder_break();
        assert_eq!(preset.to_string(), "routing_builder_break");
        assert!(preset.breaks_builder());
        assert_eq!(preset.checkpoint(), 4);
        assert_eq!(preset.samples(), 1);
        assert_eq!(preset.kernel(), None);
        assert_eq!(preset.categorical_prefix(), None);
        assert!(!RoutingPreset::new(4, 1).unwrap().breaks_builder());
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
        // The original preset has usize/u32 values and at most four one-byte
        // controls; retiring the overlap flag left three.
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
