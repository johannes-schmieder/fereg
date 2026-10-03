use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{prune_singletons_with_multiplicity, FeDesign, SingletonMultiplicity};

use super::fe::propose_zero_only_fixed_effect_levels;
use super::{
    ExactFeSeparationCertificate, SeparationDropReason, SeparationMethodReport, SeparationReport,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LedgerCommit {
    pub(super) generation: usize,
    pub(super) primary_drops: Vec<usize>,
    pub(super) singleton_drops: Vec<usize>,
    pub(super) exact_fe_drops: Vec<usize>,
    pub(super) exact_fe_certificate: Option<ExactFeSeparationCertificate>,
}

pub(super) struct ProposalCommitContext<'a> {
    pub(super) outcome: &'a [f64],
    pub(super) fixed_effects: Option<&'a FeDesign>,
    pub(super) drop_singletons: bool,
    pub(super) run_exact_fe_followup: bool,
    pub(super) multiplicity: SingletonMultiplicity<'a>,
}

/// Sole mutable owner of the separation-stage active mask and reasons.
///
/// Transactions operate on temporary masks and replace this state only after
/// every direct drop, singleton cascade, and exact-FE follow-up succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SampleLedger {
    active: Vec<bool>,
    reasons: Vec<Option<SeparationDropReason>>,
    generation: usize,
    generation_hashes: Vec<u64>,
}

impl SampleLedger {
    pub(super) fn new(candidate_observations: usize) -> Self {
        let active = vec![true; candidate_observations];
        let reasons = vec![None; candidate_observations];
        let initial_hash = generation_hash(&active, &reasons);
        Self {
            active,
            reasons,
            generation: 0,
            generation_hashes: vec![initial_hash],
        }
    }

    pub(super) fn active(&self) -> &[bool] {
        &self.active
    }

    pub(super) const fn generation(&self) -> usize {
        self.generation
    }

    pub(super) fn commit_initial_singletons(
        &mut self,
        fixed_effects: Option<&FeDesign>,
        drop_singletons: bool,
        multiplicity: SingletonMultiplicity<'_>,
    ) -> Result<Option<LedgerCommit>> {
        if !drop_singletons {
            return Ok(None);
        }
        let mut staged_active = self.active.clone();
        let mut staged_reasons = self.reasons.clone();
        let singleton_drops = stage_singleton_fixed_point(
            &mut staged_active,
            &mut staged_reasons,
            fixed_effects,
            multiplicity,
            SeparationDropReason::InitialSingleton,
        )?;
        if singleton_drops.is_empty() {
            return Ok(None);
        }
        let generation = self.commit(staged_active, staged_reasons)?;
        Ok(Some(LedgerCommit {
            generation,
            primary_drops: Vec::new(),
            singleton_drops,
            exact_fe_drops: Vec::new(),
            exact_fe_certificate: None,
        }))
    }

    /// Atomically accept a validated detector proposal, its complete
    /// singleton cascade, and the ensuing exact-FE/singleton fixed point.
    pub(super) fn commit_validated_proposal(
        &mut self,
        proposed_rows: &[usize],
        primary_reason: SeparationDropReason,
        context: ProposalCommitContext<'_>,
    ) -> Result<LedgerCommit> {
        let ProposalCommitContext {
            outcome,
            fixed_effects,
            drop_singletons,
            run_exact_fe_followup,
            multiplicity,
        } = context;
        if proposed_rows.is_empty() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "cannot commit an empty separation proposal",
            ));
        }
        let mut staged_active = self.active.clone();
        let mut staged_reasons = self.reasons.clone();
        for &row in proposed_rows {
            if row >= staged_active.len() || !staged_active[row] {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "validated separation proposal is not active at commit",
                ));
            }
            staged_active[row] = false;
            staged_reasons[row] = Some(primary_reason);
        }

        let mut singleton_drops = if drop_singletons {
            stage_singleton_fixed_point(
                &mut staged_active,
                &mut staged_reasons,
                fixed_effects,
                multiplicity,
                SeparationDropReason::SingletonCascade,
            )?
        } else {
            Vec::new()
        };
        let mut exact_fe_drops = Vec::new();
        let mut exact_checked_effects = 0_usize;
        let mut exact_zero_only_levels = Vec::new();
        if run_exact_fe_followup {
            loop {
                let exact =
                    propose_zero_only_fixed_effect_levels(outcome, fixed_effects, &staged_active);
                if exact.dropped.is_empty() {
                    break;
                }
                let certificate = exact.exact_fe_certificate.ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "exact-FE follow-up omitted its combinatorial certificate",
                    )
                })?;
                if !certificate.passed {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "exact-FE follow-up returned a failed combinatorial certificate",
                    ));
                }
                exact_checked_effects =
                    exact_checked_effects.max(certificate.checked_intercept_effects);
                exact_zero_only_levels.extend(certificate.zero_only_levels);
                for row in exact.dropped {
                    if !staged_active[row] {
                        return Err(FeregError::new(
                            ErrorCode::Internal,
                            "exact-FE follow-up proposed an inactive observation",
                        ));
                    }
                    staged_active[row] = false;
                    staged_reasons[row] = Some(SeparationDropReason::FixedEffectZeroLevel);
                    exact_fe_drops.push(row);
                }
                if drop_singletons {
                    singleton_drops.extend(stage_singleton_fixed_point(
                        &mut staged_active,
                        &mut staged_reasons,
                        fixed_effects,
                        multiplicity,
                        SeparationDropReason::SingletonCascade,
                    )?);
                }
            }
        }
        singleton_drops.sort_unstable();
        singleton_drops.dedup();
        exact_fe_drops.sort_unstable();
        exact_fe_drops.dedup();
        exact_zero_only_levels.sort_unstable();
        exact_zero_only_levels.dedup();
        let exact_fe_certificate =
            (!exact_fe_drops.is_empty()).then_some(ExactFeSeparationCertificate {
                checked_intercept_effects: exact_checked_effects,
                zero_only_levels: exact_zero_only_levels,
                passed: true,
            });

        let generation = self.commit(staged_active, staged_reasons)?;
        Ok(LedgerCommit {
            generation,
            primary_drops: proposed_rows.to_vec(),
            singleton_drops,
            exact_fe_drops,
            exact_fe_certificate,
        })
    }

    fn commit(
        &mut self,
        staged_active: Vec<bool>,
        staged_reasons: Vec<Option<SeparationDropReason>>,
    ) -> Result<usize> {
        if staged_active.len() != self.active.len() || staged_reasons.len() != self.reasons.len() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "sample-ledger transaction changed the observation axis",
            ));
        }
        if staged_active
            .iter()
            .zip(&self.active)
            .any(|(staged, current)| *staged && !*current)
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "sample-ledger transaction attempted to reactivate an observation",
            ));
        }
        if staged_active == self.active {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "sample-ledger transaction made no progress",
            ));
        }
        self.generation = self.generation.checked_add(1).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "sample-ledger generation count overflowed",
            )
        })?;
        self.active = staged_active;
        self.reasons = staged_reasons;
        self.generation_hashes
            .push(generation_hash(&self.active, &self.reasons));
        Ok(self.generation)
    }

    pub(super) fn into_report(self, methods: Vec<SeparationMethodReport>) -> SeparationReport {
        let retained_observations = self
            .active
            .iter()
            .enumerate()
            .filter_map(|(row, active)| (*active).then_some(row))
            .collect::<Vec<_>>();
        let dropped_observations = self
            .active
            .iter()
            .enumerate()
            .filter_map(|(row, active)| (!*active).then_some(row))
            .collect::<Vec<_>>();
        let singleton_count = self
            .reasons
            .iter()
            .filter(|reason| {
                matches!(
                    reason,
                    Some(
                        SeparationDropReason::InitialSingleton
                            | SeparationDropReason::SingletonCascade
                    )
                )
            })
            .count();
        let separated_count = dropped_observations.len() - singleton_count;
        SeparationReport {
            candidate_observations: self.active.len(),
            active: self.active,
            retained_observations,
            dropped_observations,
            drop_reasons: self.reasons,
            singleton_count,
            separated_count,
            generations: self.generation,
            generation_hashes: self.generation_hashes,
            methods,
        }
    }
}

fn stage_singleton_fixed_point(
    active: &mut [bool],
    reasons: &mut [Option<SeparationDropReason>],
    fixed_effects: Option<&FeDesign>,
    multiplicity: SingletonMultiplicity<'_>,
    reason: SeparationDropReason,
) -> Result<Vec<usize>> {
    let Some(design) = fixed_effects else {
        return Ok(Vec::new());
    };
    if design.effects().is_empty() {
        return Ok(Vec::new());
    }
    let codes = design
        .effects()
        .iter()
        .map(|effect| effect.codes())
        .collect::<Vec<_>>();
    let levels = design
        .effects()
        .iter()
        .map(|effect| effect.n_levels())
        .collect::<Vec<_>>();
    let pruned = prune_singletons_with_multiplicity(&codes, &levels, active, multiplicity, false)?;
    let mut dropped = pruned.dropped_observations;
    dropped.sort_unstable();
    for &row in &dropped {
        if !active[row] {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "singleton pruner returned an observation already outside the sample",
            ));
        }
        reasons[row] = Some(reason);
    }
    active.copy_from_slice(&pruned.active);
    Ok(dropped)
}

fn generation_hash(active: &[bool], reasons: &[Option<SeparationDropReason>]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for (is_active, reason) in active.iter().zip(reasons) {
        let reason_code = match reason {
            None => 0_u8,
            Some(SeparationDropReason::InitialSingleton) => 1,
            Some(SeparationDropReason::FixedEffectZeroLevel) => 2,
            Some(SeparationDropReason::Simplex) => 3,
            Some(SeparationDropReason::IteratedRectifier) => 4,
            Some(SeparationDropReason::Mu) => 5,
            Some(SeparationDropReason::SingletonCascade) => 6,
        };
        for byte in [u8::from(*is_active), reason_code] {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{EffectTerm, FeDesign};

    #[test]
    fn rejected_transaction_leaves_ledger_unchanged() {
        let mut ledger = SampleLedger::new(3);
        let before = ledger.clone();
        let error = ledger
            .commit_validated_proposal(
                &[3],
                SeparationDropReason::Simplex,
                ProposalCommitContext {
                    outcome: &[0.0, 1.0, 1.0],
                    fixed_effects: None,
                    drop_singletons: true,
                    run_exact_fe_followup: true,
                    multiplicity: SingletonMultiplicity::Unit,
                },
            )
            .expect_err("out-of-range proposal must fail");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(ledger, before);
    }

    #[test]
    fn frequency_multiplicity_prevents_false_physical_singleton() {
        let effect = EffectTerm::new(vec![0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(3, vec![effect]).expect("design");
        let frequency = [2.0, 1.0, 1.0];
        let mut ledger = SampleLedger::new(3);
        assert!(ledger
            .commit_initial_singletons(
                Some(&design),
                true,
                SingletonMultiplicity::Frequency(&frequency),
            )
            .expect("singleton pass")
            .is_none());
        assert_eq!(ledger.active(), &[true, true, true]);
    }

    #[test]
    fn direct_drop_and_cascades_commit_as_one_generation() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("effect");
        let design = FeDesign::new(6, vec![effect]).expect("design");
        let mut ledger = SampleLedger::new(6);
        let commit = ledger
            .commit_validated_proposal(
                &[2],
                SeparationDropReason::Simplex,
                ProposalCommitContext {
                    outcome: &[1.0, 1.0, 0.0, 1.0, 0.0, 0.0],
                    fixed_effects: Some(&design),
                    drop_singletons: true,
                    run_exact_fe_followup: true,
                    multiplicity: SingletonMultiplicity::Unit,
                },
            )
            .expect("transaction");
        assert_eq!(commit.generation, 1);
        assert_eq!(commit.primary_drops, vec![2]);
        assert_eq!(commit.singleton_drops, vec![3]);
        assert_eq!(commit.exact_fe_drops, vec![4, 5]);
        assert_eq!(ledger.generation_hashes.len(), 2);
    }
}
