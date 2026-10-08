//! Generic prepared-model handoff for production PPML separation.
//!
//! This module deliberately knows nothing about `FeDesign` or dense generated
//! columns.  A PPML categorical source owner implements the rebuilder, and
//! detectors borrow the immutable generation view.  Accepted original-row
//! proposals are cascaded and rebased as one transaction before the method
//! chain restarts.

use crate::memory::{fallible_copy, fallible_filled};
use std::sync::Arc;

use crate::error::{ErrorCode, FeregError, Result};

use super::SeparationDropReason;

pub(crate) trait PreparedSeparationRebuilder {
    type Generation;

    /// Length of the original Stata observation axis used for output scatter.
    fn original_observations(&self) -> usize;

    /// Sorted unique original observations surviving numeric/domain
    /// preparation and therefore eligible for separation.
    fn candidate_original_rows(&self) -> Arc<Vec<usize>>;

    /// Rebuild every row-scoped source and categorical/cluster topology on
    /// the supplied stable original rows.  The returned generation must not
    /// borrow detector scratch from an earlier generation.
    fn rebuild_generation(
        &mut self,
        retained_original_rows: &[usize],
        generation: usize,
    ) -> Result<Self::Generation>;
}

#[derive(Debug)]
pub(crate) struct PreparedSeparationGeneration<'a, G> {
    pub(crate) generation: usize,
    pub(crate) original_rows: &'a [usize],
    pub(crate) prepared: &'a G,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PreparedSeparationCascade {
    /// Rows deleted by the complete singleton cascade after the primary
    /// detector proposal, including singleton rows induced by exact-FE drops.
    pub(crate) singleton_rows: Vec<usize>,
    /// Rows deleted by the exact zero-only FE fixed point after the primary
    /// proposal.  These must be disjoint from `singleton_rows`.
    pub(crate) exact_fe_rows: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedGenerationCommit {
    pub(crate) generation: usize,
    pub(crate) primary_rows: Vec<usize>,
    pub(crate) singleton_rows: Vec<usize>,
    pub(crate) exact_fe_rows: Vec<usize>,
}

pub(crate) struct PreparedSeparationOutcome<R: PreparedSeparationRebuilder> {
    pub(crate) prepared: R::Generation,
    pub(crate) active: Vec<bool>,
    pub(crate) reasons: Vec<Option<SeparationDropReason>>,
    pub(crate) retained_original_rows: Arc<Vec<usize>>,
    pub(crate) generation: usize,
}

/// Generic stable-row coordinator used by the prepared categorical PPML path.
///
/// Detector and cascade closures never receive mutable production state.  A
/// failed proposal, cascade, allocation, or topology rebase leaves the prior
/// generation and its prepared model untouched.
pub(crate) struct PreparedSeparationCoordinator<R: PreparedSeparationRebuilder> {
    rebuilder: R,
    active: Vec<bool>,
    reasons: Vec<Option<SeparationDropReason>>,
    candidate_original_rows: Arc<Vec<usize>>,
    retained_original_rows: Arc<Vec<usize>>,
    generation: usize,
    prepared: R::Generation,
}

impl<R: PreparedSeparationRebuilder> PreparedSeparationCoordinator<R> {
    pub(crate) fn new(mut rebuilder: R) -> Result<Self> {
        let original_observations = rebuilder.original_observations();
        let retained_original_rows = rebuilder.candidate_original_rows();
        if retained_original_rows.is_empty() {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "prepared PPML separation received an empty candidate sample",
            ));
        }
        if retained_original_rows
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
            || retained_original_rows
                .iter()
                .any(|row| *row >= original_observations)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "prepared separation candidate original rows are not sorted, unique, and in range",
            ));
        }
        let prepared = rebuilder.rebuild_generation(&retained_original_rows, 0)?;
        let mut active = fallible_filled(
            original_observations,
            false,
            "unable to reserve prepared separation original-axis activity ledger",
        )?;
        for &row in retained_original_rows.iter() {
            active[row] = true;
        }
        Ok(Self {
            rebuilder,
            active,
            reasons: fallible_filled(
                original_observations,
                None,
                "unable to reserve prepared separation original-axis reason ledger",
            )?,
            candidate_original_rows: Arc::clone(&retained_original_rows),
            retained_original_rows,
            generation: 0,
            prepared,
        })
    }

    pub(crate) fn view(&self) -> PreparedSeparationGeneration<'_, R::Generation> {
        PreparedSeparationGeneration {
            generation: self.generation,
            original_rows: self.retained_original_rows.as_ref(),
            prepared: &self.prepared,
        }
    }

    pub(crate) fn active(&self) -> &[bool] {
        &self.active
    }

    pub(crate) fn reasons(&self) -> &[Option<SeparationDropReason>] {
        &self.reasons
    }

    pub(crate) fn candidate_original_rows(&self) -> Arc<Vec<usize>> {
        Arc::clone(&self.candidate_original_rows)
    }

    /// Apply one already-certified detector proposal.  `complete_cascade`
    /// sees a temporary mask with only the primary rows removed and returns
    /// the complete singleton/exact-FE fixed point on stable original IDs.
    pub(crate) fn transact_validated_proposal<F>(
        &mut self,
        primary_reason: SeparationDropReason,
        primary_rows: Vec<usize>,
        complete_cascade: F,
    ) -> Result<PreparedGenerationCommit>
    where
        F: FnOnce(&[bool]) -> Result<PreparedSeparationCascade>,
    {
        validate_sorted_active_rows(&primary_rows, &self.active, "primary")?;
        if primary_rows.is_empty() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared separation transaction has an empty primary proposal",
            ));
        }

        let mut staged_active = fallible_copy(
            &self.active,
            "unable to reserve prepared separation staged activity ledger",
        )?;
        let mut staged_reasons = fallible_copy(
            &self.reasons,
            "unable to reserve prepared separation staged reason ledger",
        )?;
        for &row in &primary_rows {
            staged_active[row] = false;
            staged_reasons[row] = Some(primary_reason);
        }
        let cascade = complete_cascade(&staged_active)?;
        validate_sorted_active_rows(&cascade.singleton_rows, &staged_active, "singleton cascade")?;
        for &row in &cascade.singleton_rows {
            staged_active[row] = false;
            staged_reasons[row] = Some(SeparationDropReason::SingletonCascade);
        }
        validate_sorted_active_rows(&cascade.exact_fe_rows, &staged_active, "exact-FE cascade")?;
        for &row in &cascade.exact_fe_rows {
            staged_active[row] = false;
            staged_reasons[row] = Some(SeparationDropReason::FixedEffectZeroLevel);
        }

        let retained_count = staged_active.iter().filter(|active| **active).count();
        let mut retained = Vec::new();
        retained.try_reserve_exact(retained_count).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve prepared separation retained-row rebase",
            )
        })?;
        retained.extend(
            staged_active
                .iter()
                .enumerate()
                .filter_map(|(row, active)| (*active).then_some(row)),
        );
        let retained_original_rows = Arc::new(retained);
        if retained_original_rows.is_empty() {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "prepared PPML separation transaction deleted the full candidate sample",
            ));
        }
        let generation = self.generation.checked_add(1).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared separation generation count overflowed",
            )
        })?;

        // Rebuild before publishing the staged ledger.  Thus even a late
        // categorical/cluster topology failure is transactionally invisible.
        let prepared = self
            .rebuilder
            .rebuild_generation(&retained_original_rows, generation)?;
        self.active = staged_active;
        self.reasons = staged_reasons;
        self.retained_original_rows = retained_original_rows;
        self.generation = generation;
        self.prepared = prepared;
        Ok(PreparedGenerationCommit {
            generation,
            // The caller has already fallibly reserved and validated this
            // exact-capacity proposal.  Move it into the commit instead of
            // allocating a third simultaneously live copy at publication.
            primary_rows,
            singleton_rows: cascade.singleton_rows,
            exact_fe_rows: cascade.exact_fe_rows,
        })
    }

    pub(crate) fn into_outcome(self) -> PreparedSeparationOutcome<R> {
        PreparedSeparationOutcome {
            prepared: self.prepared,
            active: self.active,
            reasons: self.reasons,
            retained_original_rows: self.retained_original_rows,
            generation: self.generation,
        }
    }
}

fn validate_sorted_active_rows(rows: &[usize], active: &[bool], label: &str) -> Result<()> {
    if rows.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            format!("prepared separation {label} rows are not sorted and unique"),
        ));
    }
    if rows.iter().any(|row| *row >= active.len() || !active[*row]) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            format!("prepared separation {label} contains an inactive or invalid row"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct MockRebuilder {
        original_nobs: usize,
        candidate_rows: Vec<usize>,
        fail_generation: Option<usize>,
    }

    impl PreparedSeparationRebuilder for MockRebuilder {
        type Generation = Vec<usize>;

        fn original_observations(&self) -> usize {
            self.original_nobs
        }

        fn candidate_original_rows(&self) -> Arc<Vec<usize>> {
            Arc::new(self.candidate_rows.clone())
        }

        fn rebuild_generation(
            &mut self,
            retained_original_rows: &[usize],
            generation: usize,
        ) -> Result<Self::Generation> {
            if self.fail_generation == Some(generation) {
                return Err(FeregError::new(
                    ErrorCode::Allocation,
                    "mock topology rebuild failed",
                ));
            }
            Ok(retained_original_rows.to_vec())
        }
    }

    #[test]
    fn immutable_generation_rebases_once_on_stable_original_rows() {
        let mut coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 6,
            candidate_rows: vec![0, 1, 2, 3, 4, 5],
            fail_generation: None,
        })
        .expect("coordinator");
        let initial = coordinator.view();
        assert!(Arc::ptr_eq(
            &coordinator.candidate_original_rows,
            &coordinator.retained_original_rows
        ));
        assert_eq!(initial.generation, 0);
        assert_eq!(initial.original_rows, &[0, 1, 2, 3, 4, 5]);
        assert_eq!(initial.prepared, &[0, 1, 2, 3, 4, 5]);

        let commit = coordinator
            .transact_validated_proposal(SeparationDropReason::Simplex, vec![1], |primary_active| {
                assert!(!primary_active[1]);
                Ok(PreparedSeparationCascade {
                    singleton_rows: vec![3],
                    exact_fe_rows: vec![4],
                })
            })
            .expect("transaction");
        assert_eq!(commit.generation, 1);
        let rebased = coordinator.view();
        assert!(!Arc::ptr_eq(
            &coordinator.candidate_original_rows,
            &coordinator.retained_original_rows
        ));
        assert_eq!(rebased.original_rows, &[0, 2, 5]);
        assert_eq!(rebased.prepared, &[0, 2, 5]);
        assert_eq!(
            coordinator.reasons()[1],
            Some(SeparationDropReason::Simplex)
        );
        assert_eq!(
            coordinator.reasons()[3],
            Some(SeparationDropReason::SingletonCascade)
        );
        assert_eq!(
            coordinator.reasons()[4],
            Some(SeparationDropReason::FixedEffectZeroLevel)
        );
    }

    #[test]
    fn accepted_multi_round_rebase_keeps_candidate_owner_and_latest_generation_distinct() {
        let mut coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 10,
            candidate_rows: vec![0, 1, 2, 3, 4, 5, 6, 7],
            fail_generation: None,
        })
        .expect("coordinator");
        let candidate_owner = coordinator.candidate_original_rows();

        let first = coordinator
            .transact_validated_proposal(SeparationDropReason::Simplex, vec![1], |_| {
                Ok(PreparedSeparationCascade {
                    singleton_rows: vec![2],
                    exact_fe_rows: vec![3],
                })
            })
            .expect("first accepted rebase");
        assert_eq!(first.generation, 1);
        assert_eq!(coordinator.view().original_rows, &[0, 4, 5, 6, 7]);
        assert!(!Arc::ptr_eq(
            &candidate_owner,
            &coordinator.retained_original_rows
        ));

        let first_retained = Arc::clone(&coordinator.retained_original_rows);
        let second = coordinator
            .transact_validated_proposal(SeparationDropReason::Mu, vec![5], |_| {
                Ok(PreparedSeparationCascade {
                    singleton_rows: vec![6],
                    exact_fe_rows: Vec::new(),
                })
            })
            .expect("second accepted rebase");
        assert_eq!(second.generation, 2);
        assert_eq!(coordinator.view().original_rows, &[0, 4, 7]);
        assert_eq!(coordinator.view().prepared, &[0, 4, 7]);
        assert!(!Arc::ptr_eq(
            &first_retained,
            &coordinator.retained_original_rows
        ));
        assert_eq!(candidate_owner.as_ref(), &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(
            coordinator
                .active()
                .iter()
                .filter(|active| **active)
                .count(),
            3
        );
    }

    #[test]
    fn accepted_commit_moves_the_reserved_primary_proposal_without_copying() {
        let mut coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 6,
            candidate_rows: vec![0, 1, 2, 3, 4, 5],
            fail_generation: None,
        })
        .expect("coordinator");
        let primary = vec![1_usize, 3];
        let primary_pointer = primary.as_ptr();
        let primary_capacity = primary.capacity();
        let commit = coordinator
            .transact_validated_proposal(SeparationDropReason::Simplex, primary, |_| {
                Ok(PreparedSeparationCascade::default())
            })
            .expect("accepted transaction");
        assert_eq!(commit.primary_rows, [1, 3]);
        assert_eq!(commit.primary_rows.as_ptr(), primary_pointer);
        assert_eq!(commit.primary_rows.capacity(), primary_capacity);
    }

    #[test]
    fn generation_zero_candidate_and_retained_axes_share_one_allocation() {
        let coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 5,
            candidate_rows: vec![0, 2, 4],
            fail_generation: None,
        })
        .expect("coordinator");
        let candidate = coordinator.candidate_original_rows();
        assert!(Arc::ptr_eq(&candidate, &coordinator.retained_original_rows));
        let outcome = coordinator.into_outcome();
        assert!(Arc::ptr_eq(&candidate, &outcome.retained_original_rows));
    }

    #[test]
    fn data_sized_ledger_reservation_failure_maps_to_allocation() {
        let error = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: usize::MAX,
            candidate_rows: vec![0],
            fail_generation: None,
        })
        .err()
        .expect("impossible original-axis ledger must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn failed_rebase_leaves_prior_generation_untouched() {
        let mut coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 4,
            candidate_rows: vec![0, 1, 2, 3],
            fail_generation: Some(1),
        })
        .expect("coordinator");
        let error = coordinator
            .transact_validated_proposal(SeparationDropReason::IteratedRectifier, vec![1], |_| {
                Ok(PreparedSeparationCascade::default())
            })
            .expect_err("rebase failure");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(coordinator.view().generation, 0);
        assert_eq!(coordinator.view().original_rows, &[0, 1, 2, 3]);
        assert_eq!(coordinator.active(), &[true, true, true, true]);
        assert!(coordinator.reasons().iter().all(Option::is_none));
    }

    #[test]
    fn invalid_cascade_is_atomic() {
        let mut coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 4,
            candidate_rows: vec![0, 1, 2, 3],
            fail_generation: None,
        })
        .expect("coordinator");
        let error = coordinator
            .transact_validated_proposal(SeparationDropReason::Mu, vec![1], |_| {
                Ok(PreparedSeparationCascade {
                    singleton_rows: vec![2, 2],
                    exact_fe_rows: Vec::new(),
                })
            })
            .expect_err("duplicate cascade");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(coordinator.view().generation, 0);
        assert_eq!(coordinator.active(), &[true, true, true, true]);
    }

    #[test]
    fn named_outcome_keeps_final_prepared_generation_and_row_map_together() {
        let mut coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 3,
            candidate_rows: vec![0, 1, 2],
            fail_generation: None,
        })
        .expect("coordinator");
        coordinator
            .transact_validated_proposal(
                SeparationDropReason::FixedEffectZeroLevel,
                vec![1],
                |_| Ok(PreparedSeparationCascade::default()),
            )
            .expect("transaction");
        let outcome = coordinator.into_outcome();
        assert_eq!(outcome.generation, 1);
        assert_eq!(outcome.retained_original_rows.as_ref(), &[0, 2]);
        assert_eq!(outcome.prepared, vec![0, 2]);
        assert_eq!(outcome.active, vec![true, false, true]);
        assert_eq!(
            outcome.reasons[1],
            Some(SeparationDropReason::FixedEffectZeroLevel)
        );
    }

    #[test]
    fn sparse_candidate_rows_remain_on_the_full_original_axis() {
        let coordinator = PreparedSeparationCoordinator::new(MockRebuilder {
            original_nobs: 7,
            candidate_rows: vec![1, 4, 6],
            fail_generation: None,
        })
        .expect("coordinator");
        assert_eq!(coordinator.view().original_rows, &[1, 4, 6]);
        assert_eq!(
            coordinator.active(),
            &[false, true, false, false, true, false, true]
        );
    }
}
