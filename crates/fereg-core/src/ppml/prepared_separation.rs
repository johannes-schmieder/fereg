//! Certified separation over immutable prepared PPML generations.
//!
//! This is the production bridge between the PPML-owned prepared source
//! owner and the sample ledger.  It never constructs the legacy `FeDesign`.
//! Exact FE and singleton methods consume prepared membership directly. The
//! prepared simplex constructs one explicitly bounded full finite-plus-FE
//! design, including absorbed slope channels. Comprehensive projection uses
//! the shared certified prepared weighted-linear boundary.

use std::sync::Arc;
use std::time::Instant;

use crate::data::{DenseMatrix, DropReason, WeightKind};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{BorrowedMapWeights, LevelIndex};
use crate::ols::{
    fit_prepared_weighted_linear_projection_once, LinearSolveFrame, PreparedOlsBreadWeightScale,
    PreparedOlsFitConfig,
};
use crate::ppml_separation::{model_simplex_interruptible, simplex_workspace_peak_cells};
use crate::ppml_separation::{
    DetectionProposal, ExactFeSeparationCertificate, MethodStatus, PreparedSeparationCascade,
    PreparedSeparationCoordinator, SeparationCertificate, SeparationConfig, SeparationDropReason,
    SeparationMethod, SeparationMethodReport, SeparationReport,
};

use super::memory::{begin_phase_allocation, end_phase_allocation};
use super::prepared_estimator::PpmlInterruptPoint;
use super::prepared_irls::PreparedPpmlMuExploration;
use super::prepared_model::{
    PreparedPpmlModel, PreparedPpmlModelGeneration, PreparedPpmlModelOwner,
};
#[cfg(test)]
use super::PpmlStart;
use super::{PpmlConfig, PpmlMemoryPlan, PpmlPhase, PpmlPhaseAllocatorProbe};

const SEPARATION_PHASES: [PpmlPhase; 4] = [
    PpmlPhase::SeparationFe,
    PpmlPhase::SeparationSimplex,
    PpmlPhase::SeparationIr,
    PpmlPhase::SeparationMu,
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PreparedSeparationPhasePeaks {
    values: [Option<u64>; SEPARATION_PHASES.len()],
}

impl PreparedSeparationPhasePeaks {
    fn observe(&mut self, phase: PpmlPhase, bytes: Option<u64>) -> Result<()> {
        let Some(bytes) = bytes else {
            return Ok(());
        };
        let index = separation_phase_index(phase)?;
        let slot = &mut self.values[index];
        *slot = Some(slot.map_or(bytes, |observed| observed.max(bytes)));
        Ok(())
    }

    fn get(self, phase: PpmlPhase) -> Option<u64> {
        separation_phase_index(phase)
            .ok()
            .and_then(|index| self.values[index])
    }
}

pub(crate) struct PreparedPpmlSeparationOutcome {
    pub(crate) prepared: PreparedPpmlModelGeneration,
    pub(crate) separation: SeparationReport,
    /// Opaque, independently replayed binding between the exact requested
    /// detector policy and the terminal prepared generation.  Only this
    /// module can mint the seal; the publication boundary rechecks its digest
    /// after the report has been mapped to the candidate-row axis.
    pub(crate) certification_seal: CertifiedSeparationSeal,
    /// Numeric-candidate position -> stable original observation identifier.
    pub(crate) candidate_original_rows: Arc<Vec<usize>>,
    /// Separation-only reasons on the full original observation axis.  The
    /// final generation merges this with numeric/domain reasons exactly once.
    pub(crate) original_axis_reasons: Vec<Option<SeparationDropReason>>,
    phase_allocation_peaks: PreparedSeparationPhasePeaks,
}

/// Unforgeable (outside this module) terminal separation certification.
///
/// The detector result itself is not accepted as its own proof.  Every
/// enabled detector that reaches the stable generation is independently
/// replayed while its registered phase boundary is still active.  The token
/// from those replays is then sealed over the complete, remapped report.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CertifiedSeparationSeal {
    config: SeparationConfig,
    final_generation: usize,
    final_generation_hash: u64,
    final_rows_digest: u64,
    final_outcome_digest: u64,
    report_digest: u64,
    replay_mask: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FinalGenerationReplayToken {
    final_generation: usize,
    final_rows_digest: u64,
    final_outcome_digest: u64,
    replay_mask: u8,
}

impl CertifiedSeparationSeal {
    pub(crate) fn validate(
        &self,
        config: SeparationConfig,
        report: &SeparationReport,
        final_original_rows: &[usize],
        final_outcome: &[f64],
    ) -> Result<()> {
        let expected_mask = requested_replay_mask(config);
        if self.config != config
            || self.final_generation != report.generations
            || self.final_generation_hash != report.generation_hashes.last().copied().unwrap_or(0)
            || self.final_rows_digest
                != stable_usize_slice_digest(b"final-rows", final_original_rows)?
            || self.final_outcome_digest
                != stable_f64_slice_digest(b"final-outcome", final_outcome)?
            || self.report_digest != separation_report_digest(report)?
            || self.replay_mask != expected_mask
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML separation certification seal does not bind the requested detector chain and terminal generation",
            ));
        }
        Ok(())
    }
}

impl FinalGenerationReplayToken {
    fn new(
        generation: &PreparedPpmlModelGeneration,
        replay_mask: u8,
        config: SeparationConfig,
    ) -> Result<Self> {
        Self::from_parts(
            generation.generation(),
            generation.original_rows(),
            generation.outcome(),
            replay_mask,
            config,
        )
    }

    fn from_parts(
        final_generation: usize,
        final_original_rows: &[usize],
        final_outcome: &[f64],
        replay_mask: u8,
        config: SeparationConfig,
    ) -> Result<Self> {
        if replay_mask != requested_replay_mask(config) {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML terminal separation generation did not independently replay every requested detector",
            ));
        }
        Ok(Self {
            final_generation,
            final_rows_digest: stable_usize_slice_digest(b"final-rows", final_original_rows)?,
            final_outcome_digest: stable_f64_slice_digest(b"final-outcome", final_outcome)?,
            replay_mask,
        })
    }

    fn seal(
        self,
        config: SeparationConfig,
        report: &SeparationReport,
    ) -> Result<CertifiedSeparationSeal> {
        if report.generations != self.final_generation {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML separation replay generation changed before certification was sealed",
            ));
        }
        Ok(CertifiedSeparationSeal {
            config,
            final_generation: self.final_generation,
            final_generation_hash: report.generation_hashes.last().copied().ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "PPML separation report omitted its terminal generation hash",
                )
            })?,
            final_rows_digest: self.final_rows_digest,
            final_outcome_digest: self.final_outcome_digest,
            report_digest: separation_report_digest(report)?,
            replay_mask: self.replay_mask,
        })
    }
}

/// Generation-zero memory authority and resolved shared-linear policy used
/// by every separation projection and every post-separation PPML fit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlSeparationExecution {
    pub(crate) preliminary_plan: PpmlMemoryPlan,
    pub(crate) linear: PreparedOlsFitConfig,
}

impl PreparedPpmlSeparationOutcome {
    pub(crate) fn final_drop_reasons(&self) -> Result<Vec<DropReason>> {
        self.prepared
            .final_drop_reasons(&self.original_axis_reasons)
    }

    pub(crate) fn phase_allocation_peak(&self, phase: PpmlPhase) -> Option<u64> {
        self.phase_allocation_peaks.get(phase)
    }
}

fn separation_phase_index(phase: PpmlPhase) -> Result<usize> {
    SEPARATION_PHASES
        .iter()
        .position(|candidate| *candidate == phase)
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared separation received a non-separation allocation phase",
            )
        })
}

fn measure_separation_phase<T>(
    probe: Option<PpmlPhaseAllocatorProbe>,
    peaks: &mut PreparedSeparationPhasePeaks,
    phase: PpmlPhase,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    // Always emit a real boundary, including disabled and zero-candidate
    // stages. Evaluate `operation` before ending the boundary but return its
    // error only afterward, so qualification state is not left open by an
    // ordinary fail-closed detector result.
    begin_phase_allocation(probe, phase);
    let result = operation();
    peaks.observe(phase, end_phase_allocation(probe, phase))?;
    result
}

/// Separation-only adapter retained for focused detector tests. Production
/// uses `fit_with_prepared_ppml_separation`, so a requested Mu method without
/// an explicit fitted start fails instead of silently publishing a skip.
#[cfg(test)]
pub(crate) fn detect_prepared_ppml_separation(
    owner: PreparedPpmlModelOwner,
    ppml: PpmlConfig,
    config: SeparationConfig,
    phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    start: Option<&PpmlStart>,
) -> Result<PreparedPpmlSeparationOutcome> {
    let supplied_start = start.cloned();
    validate_config(config)?;
    let coordinator = PreparedSeparationCoordinator::new(owner)?;
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let mut interrupt = |_| Ok(());
    let (outcome, ()) = coordinate_prepared_ppml_separation(
        coordinator,
        ppml,
        config,
        phase_allocation_probe,
        linear,
        &mut interrupt,
        |generation, _| {
            let fitted_start = match &supplied_start {
                Some(start) => start,
                None if config.methods.mu => {
                    return Err(FeregError::new(
                        ErrorCode::Unsupported,
                        "requested prepared Mu separation requires a fitted generation start",
                    ));
                }
                None => unreachable!("Mu exploration is disabled"),
            };
            if fitted_start.linear_predictor.len() != generation.original_observations() {
                return Err(invalid_argument(
                    "prepared Mu test start does not match the original axis",
                ));
            }
            let eta = generation
                .original_rows()
                .iter()
                .map(|row| fitted_start.linear_predictor[*row])
                .collect();
            PreparedPpmlMuExploration::from_test_eta(
                eta,
                generation.outcome(),
                generation.prior_weights(),
                ppml.bounds,
            )
        },
        |_, _| Ok(()),
    )?;
    Ok(outcome)
}

/// Execute singleton -> exact FE -> simplex -> IR, then run a private fitted-
/// mean exploration when Mu is requested. Every accepted Mu transaction
/// invalidates that exploratory artifact and restarts the full chain; only a
/// fresh fit of the stable generation may cross the publication boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_with_prepared_ppml_separation<T, E, F>(
    owner: PreparedPpmlModelOwner,
    ppml: PpmlConfig,
    config: SeparationConfig,
    phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    execution: PreparedPpmlSeparationExecution,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    mut explore_generation: E,
    mut fit_generation: F,
) -> Result<(PreparedPpmlSeparationOutcome, T)>
where
    E: FnMut(
        &PreparedPpmlModelGeneration,
        &PpmlMemoryPlan,
        PreparedOlsFitConfig,
        &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<PreparedPpmlMuExploration>,
    F: FnMut(
        &PreparedPpmlModelGeneration,
        &PpmlMemoryPlan,
        PreparedOlsFitConfig,
        &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<T>,
{
    validate_config(config)?;
    let coordinator = PreparedSeparationCoordinator::new(owner)?;
    coordinate_prepared_ppml_separation(
        coordinator,
        ppml,
        config,
        phase_allocation_probe,
        execution.linear,
        interrupt,
        |generation, interrupt| {
            explore_generation(
                generation,
                &execution.preliminary_plan,
                execution.linear,
                interrupt,
            )
        },
        |generation, interrupt| {
            fit_generation(
                generation,
                &execution.preliminary_plan,
                execution.linear,
                interrupt,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn coordinate_prepared_ppml_separation<T, E, F>(
    mut coordinator: PreparedSeparationCoordinator<PreparedPpmlModelOwner>,
    ppml: PpmlConfig,
    config: SeparationConfig,
    phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    linear: PreparedOlsFitConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    mut explore_generation: E,
    mut fit_generation: F,
) -> Result<(PreparedPpmlSeparationOutcome, T)>
where
    E: FnMut(
        &PreparedPpmlModelGeneration,
        &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<PreparedPpmlMuExploration>,
    F: FnMut(
        &PreparedPpmlModelGeneration,
        &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<T>,
{
    let candidate_original_rows = coordinator.candidate_original_rows();
    let candidate_observations = candidate_original_rows.len();
    let mut reports = Vec::new();
    let mut generation_hashes = Vec::new();
    generation_hashes
        .try_reserve_exact(1)
        .map_err(|_| allocation("unable to reserve prepared separation generation hash"))?;
    generation_hashes.push(candidate_ledger_hash_original_axis(
        &candidate_original_rows,
        coordinator.active(),
        coordinator.reasons(),
    )?);
    let mut phase_allocation_peaks = PreparedSeparationPhasePeaks::default();

    let (final_fit, final_replay) = loop {
        interrupt(PpmlInterruptPoint::SeparationGeneration)?;
        require_positive_outcome(coordinator.view().prepared.outcome())?;
        // Reinitialized after every committed deletion.  Consequently only
        // detector replays performed on the eventual stable generation can
        // contribute to the publication seal.
        let mut final_generation_replay_mask = 0_u8;

        let restart_after_fe = measure_separation_phase(
            phase_allocation_probe,
            &mut phase_allocation_peaks,
            PpmlPhase::SeparationFe,
            || {
                interrupt(PpmlInterruptPoint::SeparationFixedEffects)?;
                if config.drop_singletons {
                    let started = Instant::now();
                    let singleton_rows = {
                        let view = coordinator.view();
                        let topology = DetectorTopology::new(view.prepared.prepared())?;
                        compact_to_original(
                            singleton_fixed_point(
                                &topology,
                                view.prepared,
                                &fallible_filled(
                                    view.original_rows.len(),
                                    true,
                                    "unable to reserve prepared singleton activity mask",
                                )?,
                            )?,
                            view.original_rows,
                        )?
                    };
                    if !singleton_rows.is_empty() {
                        let commit = coordinator.transact_validated_proposal(
                            SeparationDropReason::InitialSingleton,
                            &singleton_rows,
                            |_| Ok(PreparedSeparationCascade::default()),
                        )?;
                        fallible_push(
                            &mut reports,
                            method_report(
                                commit.generation,
                                SeparationMethod::Singletons,
                                MethodStatus::Certified,
                                1,
                                Vec::new(),
                                commit.primary_rows,
                                None,
                                None,
                                started,
                                config,
                                "prepared singleton pruning reached its fixed point",
                            ),
                            "unable to reserve prepared separation method report",
                        )?;
                        fallible_push(
                            &mut generation_hashes,
                            candidate_ledger_hash_original_axis(
                                &candidate_original_rows,
                                coordinator.active(),
                                coordinator.reasons(),
                            )?,
                            "unable to reserve prepared separation generation hash",
                        )?;
                        return Ok(true);
                    }
                    // A no-drop singleton pass used to be invisible in the
                    // report, making it impossible to distinguish an
                    // executed policy from a skipped one.  Replay it from the
                    // immutable generation and record the terminal evidence.
                    let replayed_singletons = {
                        let view = coordinator.view();
                        let topology = DetectorTopology::new(view.prepared.prepared())?;
                        compact_to_original(
                            singleton_fixed_point(
                                &topology,
                                view.prepared,
                                &fallible_filled(
                                    view.original_rows.len(),
                                    true,
                                    "unable to reserve singleton certification replay mask",
                                )?,
                            )?,
                            view.original_rows,
                        )?
                    };
                    if !replayed_singletons.is_empty() {
                        return Err(FeregError::new(
                            ErrorCode::NonConvergence,
                            "prepared singleton no-drop result failed independent replay",
                        ));
                    }
                    final_generation_replay_mask |= replay_bit(SeparationMethod::Singletons);
                    fallible_push(
                        &mut reports,
                        method_report(
                            coordinator.view().generation,
                            SeparationMethod::Singletons,
                            MethodStatus::NoSeparation,
                            1,
                            Vec::new(),
                            Vec::new(),
                            None,
                            None,
                            started,
                            config,
                            "prepared singleton pruning independently certified a stable fixed point",
                        ),
                        "unable to reserve prepared singleton no-drop report",
                    )?;
                }

                if config.methods.fixed_effects {
                    let started = Instant::now();
                    let exact = {
                        let view = coordinator.view();
                        exact_fe_proposal(
                            view.prepared,
                            &fallible_filled(
                                view.original_rows.len(),
                                true,
                                "unable to reserve prepared exact-FE activity mask",
                            )?,
                        )?
                    };
                    if !exact.rows.is_empty() {
                        let (primary_rows, cascade) = {
                            let view = coordinator.view();
                            let primary_rows = compact_to_original(
                                fallible_copy(
                                    &exact.rows,
                                    "unable to reserve prepared exact-FE proposal rows",
                                )?,
                                view.original_rows,
                            )?;
                            let cascade = complete_cascade(view.prepared, &primary_rows, config)?;
                            (primary_rows, cascade)
                        };
                        let PreparedCascadeProposal {
                            cascade,
                            exact_fe_certificate: exact_followup_certificate,
                        } = cascade;
                        let commit = coordinator.transact_validated_proposal(
                            SeparationDropReason::FixedEffectZeroLevel,
                            &primary_rows,
                            |_| Ok(cascade),
                        )?;
                        fallible_push(
                            &mut reports,
                            method_report(
                                commit.generation,
                                SeparationMethod::FixedEffects,
                                MethodStatus::Certified,
                                1,
                                commit.primary_rows,
                                commit.singleton_rows,
                                None,
                                Some(exact.certificate),
                                started,
                                config,
                                "prepared exact zero-only FE certificate passed",
                            ),
                            "unable to reserve prepared separation method report",
                        )?;
                        if !commit.exact_fe_rows.is_empty() {
                            fallible_push(
                                &mut reports,
                                exact_followup_report(
                                    commit.generation,
                                    commit.exact_fe_rows,
                                    exact_followup_certificate.ok_or_else(|| {
                                        FeregError::new(
                                            ErrorCode::Internal,
                                            "prepared exact-FE cascade omitted its certificate",
                                        )
                                    })?,
                                ),
                                "unable to reserve prepared exact-FE follow-up report",
                            )?;
                        }
                        fallible_push(
                            &mut generation_hashes,
                            candidate_ledger_hash_original_axis(
                                &candidate_original_rows,
                                coordinator.active(),
                                coordinator.reasons(),
                            )?,
                            "unable to reserve prepared separation generation hash",
                        )?;
                        return Ok(true);
                    }
                    fallible_push(
                        &mut reports,
                        method_report(
                            coordinator.view().generation,
                            SeparationMethod::FixedEffects,
                            MethodStatus::NoSeparation,
                            1,
                            Vec::new(),
                            Vec::new(),
                            None,
                            Some(exact.certificate.clone()),
                            started,
                            config,
                            "prepared exact FE scan found no zero-only intercept level",
                        ),
                        "unable to reserve prepared separation method report",
                    )?;
                    let replay = {
                        let view = coordinator.view();
                        exact_fe_proposal(
                            view.prepared,
                            &fallible_filled(
                                view.original_rows.len(),
                                true,
                                "unable to reserve exact-FE certification replay mask",
                            )?,
                        )?
                    };
                    if !replay.rows.is_empty() || replay.certificate != exact.certificate {
                        return Err(FeregError::new(
                            ErrorCode::NonConvergence,
                            "prepared exact-FE no-separation result failed independent replay",
                        ));
                    }
                    final_generation_replay_mask |= replay_bit(SeparationMethod::FixedEffects);
                }
                Ok(false)
            },
        )?;
        if restart_after_fe {
            continue;
        }

        if !coordinator.view().prepared.outcome().contains(&0.0) {
            append_no_boundary_reports(&mut reports, coordinator.view().generation, config)?;
            final_generation_replay_mask |= requested_numerical_replay_mask(config);
            for phase in [
                PpmlPhase::SeparationSimplex,
                PpmlPhase::SeparationIr,
                PpmlPhase::SeparationMu,
            ] {
                measure_separation_phase(
                    phase_allocation_probe,
                    &mut phase_allocation_peaks,
                    phase,
                    || Ok(()),
                )?;
            }
            let replay = FinalGenerationReplayToken::new(
                coordinator.view().prepared,
                final_generation_replay_mask,
                config,
            )?;
            let fit = fit_generation(coordinator.view().prepared, interrupt)?;
            break (fit, replay);
        }

        let (simplex_unresolved, restart_after_simplex) = measure_separation_phase(
            phase_allocation_probe,
            &mut phase_allocation_peaks,
            PpmlPhase::SeparationSimplex,
            || {
                interrupt(PpmlInterruptPoint::SeparationSimplex)?;
                if !config.methods.simplex {
                    return Ok((None, false));
                }
                let started = Instant::now();
                let proposal = {
                    let view = coordinator.view();
                    prepared_simplex_proposal(view.prepared, config, interrupt)?
                };
                let simplex_unresolved = matches!(
                    proposal.status,
                    MethodStatus::SkippedMemory
                        | MethodStatus::SkippedUnsupported
                        | MethodStatus::Uncertified
                )
                .then_some(proposal.status);
                if !proposal.dropped.is_empty() {
                    let certified = {
                        let view = coordinator.view();
                        validate_numerical_proposal(view.prepared, &proposal, ppml, config, linear)?
                    };
                    commit_numerical(
                        &mut coordinator,
                        &mut reports,
                        &mut generation_hashes,
                        &candidate_original_rows,
                        proposal,
                        certified,
                        SeparationDropReason::Simplex,
                        started,
                        ppml,
                        config,
                    )?;
                    return Ok((simplex_unresolved, true));
                }
                let replayed_certificate = replay_numerical_no_drop(
                    coordinator.view().prepared,
                    &proposal,
                    ppml,
                    config,
                    linear,
                    interrupt,
                    None,
                )?;
                final_generation_replay_mask |= replay_bit(SeparationMethod::Simplex);
                fallible_push(
                    &mut reports,
                    proposal_report(
                        coordinator.view().generation,
                        proposal,
                        replayed_certificate,
                        started,
                        config,
                    ),
                    "unable to reserve prepared simplex report",
                )?;
                Ok((simplex_unresolved, false))
            },
        )?;
        if restart_after_simplex {
            continue;
        }

        let (comprehensive_certified, restart_after_ir) = measure_separation_phase(
            phase_allocation_probe,
            &mut phase_allocation_peaks,
            PpmlPhase::SeparationIr,
            || {
                interrupt(PpmlInterruptPoint::SeparationIteratedRectifier)?;
                if !config.methods.iterated_rectifier {
                    return Ok((false, false));
                }
                let started = Instant::now();
                let proposal = {
                    let view = coordinator.view();
                    prepared_iterated_rectifier(view.prepared, ppml, config, linear, interrupt)?
                };
                fail_closed_comprehensive(&proposal)?;
                if !proposal.dropped.is_empty() {
                    let certified = {
                        let view = coordinator.view();
                        validate_numerical_proposal(view.prepared, &proposal, ppml, config, linear)?
                    };
                    commit_numerical(
                        &mut coordinator,
                        &mut reports,
                        &mut generation_hashes,
                        &candidate_original_rows,
                        proposal,
                        certified,
                        SeparationDropReason::IteratedRectifier,
                        started,
                        ppml,
                        config,
                    )?;
                    return Ok((false, true));
                }
                let replayed_certificate = replay_numerical_no_drop(
                    coordinator.view().prepared,
                    &proposal,
                    ppml,
                    config,
                    linear,
                    interrupt,
                    None,
                )?;
                let comprehensive_certified = proposal.status == MethodStatus::NoSeparation
                    && replayed_certificate
                        .as_ref()
                        .is_some_and(|certificate| certificate.passed);
                final_generation_replay_mask |= replay_bit(SeparationMethod::IteratedRectifier);
                fallible_push(
                    &mut reports,
                    proposal_report(
                        coordinator.view().generation,
                        proposal,
                        replayed_certificate,
                        started,
                        config,
                    ),
                    "unable to reserve prepared IR report",
                )?;
                Ok((comprehensive_certified, false))
            },
        )?;
        if restart_after_ir {
            continue;
        }
        if let Some(simplex_status) = simplex_unresolved.filter(|_| !comprehensive_certified) {
            return Err(FeregError::new(
                if simplex_status == MethodStatus::SkippedMemory {
                    ErrorCode::Allocation
                } else {
                    ErrorCode::NonConvergence
                },
                "prepared simplex remained unresolved and IR did not certify the same generation",
            ));
        }

        // Mu is a post-fit detector. Its fitted start is intrinsic detector
        // work, so both the fit and the fitted-mean check live inside the Mu
        // phase boundary. The artifact belongs to this exact prepared
        // generation: an accepted transaction drops it before restarting the
        // full chain, while a stable generation publishes that same artifact.
        let restart_after_mu = measure_separation_phase(
            phase_allocation_probe,
            &mut phase_allocation_peaks,
            PpmlPhase::SeparationMu,
            || {
                interrupt(PpmlInterruptPoint::SeparationMu)?;
                if !config.methods.mu {
                    return Ok(false);
                }
                let started = Instant::now();
                let exploration = explore_generation(coordinator.view().prepared, interrupt)?;
                if !exploration.certificate().passed {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        "prepared Mu exploratory trajectory did not certify",
                    ));
                }
                let proposal = {
                    let view = coordinator.view();
                    prepared_mu_from_mean(
                        view.prepared,
                        ppml,
                        config,
                        linear,
                        exploration.mean(),
                        exploration.certificate().accepted_iterations,
                    )?
                };
                if matches!(
                    proposal.status,
                    MethodStatus::MaximumIterations | MethodStatus::Uncertified
                ) {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        "prepared mu separation did not produce a certified result",
                    ));
                }
                if !proposal.dropped.is_empty() {
                    let certified = {
                        let view = coordinator.view();
                        validate_numerical_proposal(view.prepared, &proposal, ppml, config, linear)?
                    };
                    commit_numerical(
                        &mut coordinator,
                        &mut reports,
                        &mut generation_hashes,
                        &candidate_original_rows,
                        proposal,
                        certified,
                        SeparationDropReason::Mu,
                        started,
                        ppml,
                        config,
                    )?;
                    return Ok(true);
                }
                let replayed_certificate = replay_numerical_no_drop(
                    coordinator.view().prepared,
                    &proposal,
                    ppml,
                    config,
                    linear,
                    interrupt,
                    Some(&exploration),
                )?;
                final_generation_replay_mask |= replay_bit(SeparationMethod::Mu);
                fallible_push(
                    &mut reports,
                    proposal_report(
                        coordinator.view().generation,
                        proposal,
                        replayed_certificate,
                        started,
                        config,
                    ),
                    "unable to reserve prepared Mu report",
                )?;
                Ok(false)
            },
        )?;
        if restart_after_mu {
            continue;
        }
        // Exploratory state is deliberately discarded even on a stable
        // generation. Only a fresh converged strict/final-Hessian fit may
        // cross the publication boundary.
        let replay = FinalGenerationReplayToken::new(
            coordinator.view().prepared,
            final_generation_replay_mask,
            config,
        )?;
        let fit = fit_generation(coordinator.view().prepared, interrupt)?;
        break (fit, replay);
    };

    let outcome = coordinator.into_outcome();
    require_positive_outcome(outcome.prepared.outcome())?;
    if outcome.retained_original_rows.as_ref() != outcome.prepared.original_rows() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared separation coordinator and final generation row maps disagree",
        ));
    }
    if outcome.generation != outcome.prepared.generation() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared separation coordinator and final prepared generation tags disagree",
        ));
    }
    let original_axis_reasons = outcome.reasons;
    let mut active = Vec::new();
    active
        .try_reserve_exact(candidate_observations)
        .map_err(|_| allocation("unable to reserve prepared separation candidate mask"))?;
    let mut drop_reasons = Vec::new();
    drop_reasons
        .try_reserve_exact(candidate_observations)
        .map_err(|_| allocation("unable to reserve prepared separation candidate reasons"))?;
    for &original_row in candidate_original_rows.iter() {
        active.push(outcome.active[original_row]);
        drop_reasons.push(original_axis_reasons[original_row]);
    }
    let retained_observations = fallible_collect(
        active
            .iter()
            .enumerate()
            .filter_map(|(row, retained)| (*retained).then_some(row)),
        candidate_observations,
        "unable to reserve prepared separation retained candidate rows",
    )?;
    let dropped_observations = fallible_collect(
        drop_reasons
            .iter()
            .enumerate()
            .filter_map(|(row, reason)| reason.is_some().then_some(row)),
        candidate_observations,
        "unable to reserve prepared separation dropped candidate rows",
    )?;
    let singleton_count = drop_reasons
        .iter()
        .filter(|reason| {
            matches!(
                reason,
                Some(
                    SeparationDropReason::InitialSingleton | SeparationDropReason::SingletonCascade
                )
            )
        })
        .count();
    let separated_count = dropped_observations.len().saturating_sub(singleton_count);
    for report in &mut reports {
        remap_original_rows_to_candidate(
            &mut report.dropped_observations,
            &candidate_original_rows,
            "method drops",
        )?;
        remap_original_rows_to_candidate(
            &mut report.singleton_drops,
            &candidate_original_rows,
            "method singleton drops",
        )?;
    }
    let separation = SeparationReport {
        candidate_observations,
        active,
        retained_observations,
        dropped_observations,
        drop_reasons,
        singleton_count,
        separated_count,
        generations: outcome.generation,
        generation_hashes,
        methods: reports,
    };
    let certification_seal = final_replay.seal(config, &separation)?;
    let outcome = PreparedPpmlSeparationOutcome {
        prepared: outcome.prepared,
        separation,
        certification_seal,
        candidate_original_rows,
        original_axis_reasons,
        phase_allocation_peaks,
    };
    Ok((outcome, final_fit))
}

#[allow(clippy::too_many_arguments)]
fn commit_numerical(
    coordinator: &mut PreparedSeparationCoordinator<PreparedPpmlModelOwner>,
    reports: &mut Vec<SeparationMethodReport>,
    generation_hashes: &mut Vec<u64>,
    candidate_original_rows: &[usize],
    proposal: DetectionProposal,
    certificate: Option<SeparationCertificate>,
    reason: SeparationDropReason,
    started: Instant,
    _ppml: PpmlConfig,
    config: SeparationConfig,
) -> Result<()> {
    let (primary_rows, cascade) = {
        let view = coordinator.view();
        let primary_rows = compact_to_original(
            fallible_copy(
                &proposal.dropped,
                "unable to reserve committed numerical proposal rows",
            )?,
            view.original_rows,
        )?;
        let cascade = complete_cascade(view.prepared, &primary_rows, config)?;
        (primary_rows, cascade)
    };
    let PreparedCascadeProposal {
        cascade,
        exact_fe_certificate: exact_followup_certificate,
    } = cascade;
    let commit = coordinator.transact_validated_proposal(reason, &primary_rows, |_| Ok(cascade))?;
    let method = proposal.method;
    fallible_push(
        reports,
        method_report(
            commit.generation,
            method,
            proposal.status,
            proposal.iterations,
            commit.primary_rows,
            commit.singleton_rows,
            certificate,
            proposal.exact_fe_certificate,
            started,
            config,
            &proposal.note,
        ),
        "unable to reserve committed separation method report",
    )?;
    if !commit.exact_fe_rows.is_empty() {
        fallible_push(
            reports,
            exact_followup_report(
                commit.generation,
                commit.exact_fe_rows,
                exact_followup_certificate.ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "prepared exact-FE cascade omitted its certificate",
                    )
                })?,
            ),
            "unable to reserve committed exact-FE follow-up report",
        )?;
    }
    fallible_push(
        generation_hashes,
        candidate_ledger_hash_original_axis(
            candidate_original_rows,
            coordinator.active(),
            coordinator.reasons(),
        )?,
        "unable to reserve committed separation generation hash",
    )?;
    Ok(())
}

struct DetectorTopology<'a> {
    sources: Vec<&'a LevelIndex>,
    effect_to_source: Vec<usize>,
    intercept_effect: Vec<bool>,
}

impl<'a> DetectorTopology<'a> {
    fn new(prepared: &'a PreparedPpmlModel) -> Result<Self> {
        let categorical = prepared.categorical().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared PPML model has no categorical fit topology",
            )
        })?;
        let topology = categorical.fe_topology();
        let sources = fallible_collect(
            topology
                .registry()
                .sources()
                .iter()
                .map(|source| source.level_index()),
            topology.registry().sources().len(),
            "unable to reserve prepared detector topology sources",
        )?;
        let effect_to_source = fallible_copy(
            topology.effect_to_topology(),
            "unable to reserve prepared detector effect mapping",
        )?;
        let intercept_effect = fallible_collect(
            categorical
                .effect_specs()
                .iter()
                .map(|effect| effect.intercept),
            categorical.effect_specs().len(),
            "unable to reserve prepared detector intercept flags",
        )?;
        if effect_to_source.len() != intercept_effect.len()
            || effect_to_source
                .iter()
                .any(|source| *source >= sources.len())
        {
            return Err(invalid_argument(
                "prepared categorical separation topology is inconsistent",
            ));
        }
        Ok(Self {
            sources,
            effect_to_source,
            intercept_effect,
        })
    }
}

fn singleton_fixed_point(
    topology: &DetectorTopology<'_>,
    generation: &PreparedPpmlModelGeneration,
    initially_active: &[bool],
) -> Result<Vec<usize>> {
    if initially_active.len() != generation.original_rows().len() {
        return Err(invalid_argument(
            "prepared singleton active mask is not compact-axis aligned",
        ));
    }
    let mut active = fallible_copy(
        initially_active,
        "unable to reserve prepared singleton working mask",
    )?;
    let mut dropped = Vec::new();
    dropped
        .try_reserve_exact(initially_active.len())
        .map_err(|_| allocation("unable to reserve prepared singleton drop rows"))?;
    let mut pass = Vec::new();
    pass.try_reserve_exact(initially_active.len())
        .map_err(|_| allocation("unable to reserve prepared singleton pass rows"))?;
    let mut selected = fallible_filled(
        initially_active.len(),
        false,
        "unable to reserve prepared singleton pass selection mask",
    )?;
    loop {
        pass.clear();
        selected.fill(false);
        for source in &topology.sources {
            for level in 0..source.n_levels() {
                let mut multiplicity = 0_u128;
                let mut sole_row = None;
                for &row in source.checked_members(level)? {
                    if !active[row] {
                        continue;
                    }
                    let row_multiplicity =
                        if generation.weight_report().kind == Some(WeightKind::Frequency) {
                            generation.prior_weights()[row] as u128
                        } else {
                            1
                        };
                    multiplicity = multiplicity.checked_add(row_multiplicity).ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::InvalidData,
                            "prepared singleton multiplicity overflowed",
                        )
                    })?;
                    sole_row = Some(row);
                }
                if multiplicity == 1 {
                    let row = sole_row.expect("unit singleton has one active row");
                    if !selected[row] {
                        selected[row] = true;
                        pass.push(row);
                    }
                }
            }
        }
        pass.sort_unstable();
        pass.dedup();
        pass.retain(|row| active[*row]);
        if pass.is_empty() {
            break;
        }
        for &row in &pass {
            active[row] = false;
        }
        dropped.extend(pass.iter().copied());
    }
    dropped.sort_unstable();
    dropped.dedup();
    Ok(dropped)
}

struct ExactFeProposal {
    rows: Vec<usize>,
    certificate: ExactFeSeparationCertificate,
}

fn exact_fe_proposal(
    generation: &PreparedPpmlModelGeneration,
    active: &[bool],
) -> Result<ExactFeProposal> {
    let topology = DetectorTopology::new(generation.prepared())?;
    if active.len() != generation.original_rows().len() {
        return Err(invalid_argument(
            "prepared exact-FE active mask is not compact-axis aligned",
        ));
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(active.len())
        .map_err(|_| allocation("unable to reserve prepared exact-FE drop rows"))?;
    let mut selected = fallible_filled(
        active.len(),
        false,
        "unable to reserve prepared exact-FE row selection mask",
    )?;
    let mut levels = Vec::new();
    let level_capacity = topology
        .effect_to_source
        .iter()
        .zip(&topology.intercept_effect)
        .filter(|(_, intercept)| **intercept)
        .try_fold(0_usize, |total, (source, _)| {
            total.checked_add(topology.sources[*source].n_levels())
        })
        .ok_or_else(|| allocation("prepared exact-FE level capacity overflowed"))?;
    levels
        .try_reserve_exact(level_capacity)
        .map_err(|_| allocation("unable to reserve prepared exact-FE level certificate"))?;
    let mut checked = 0_usize;
    for (effect, &source_index) in topology.effect_to_source.iter().enumerate() {
        if !topology.intercept_effect[effect] {
            continue;
        }
        checked += 1;
        let source = topology.sources[source_index];
        for level in 0..source.n_levels() {
            let members = source.checked_members(level)?;
            let mut any = false;
            let mut zero_only = true;
            for &row in members {
                if !active[row] {
                    continue;
                }
                any = true;
                if generation.outcome()[row] > 0.0 {
                    zero_only = false;
                    break;
                }
            }
            if any && zero_only {
                fallible_push(
                    &mut levels,
                    (effect, level as u32),
                    "prepared exact-FE level certificate capacity exhausted",
                )?;
                for &row in members {
                    if active[row] && !selected[row] {
                        selected[row] = true;
                        fallible_push(
                            &mut rows,
                            row,
                            "prepared exact-FE drop-row capacity exhausted",
                        )?;
                    }
                }
            }
        }
    }
    rows.sort_unstable();
    rows.dedup();
    levels.sort_unstable();
    levels.dedup();
    Ok(ExactFeProposal {
        rows,
        certificate: ExactFeSeparationCertificate {
            checked_intercept_effects: checked,
            zero_only_levels: levels,
            passed: true,
        },
    })
}

struct PreparedCascadeProposal {
    cascade: PreparedSeparationCascade,
    exact_fe_certificate: Option<ExactFeSeparationCertificate>,
}

fn complete_cascade(
    generation: &PreparedPpmlModelGeneration,
    primary_original_rows: &[usize],
    config: SeparationConfig,
) -> Result<PreparedCascadeProposal> {
    let mut active = fallible_filled(
        generation.original_rows().len(),
        true,
        "unable to reserve prepared cascade activity mask",
    )?;
    for &original_row in primary_original_rows {
        let position = generation
            .original_rows()
            .binary_search(&original_row)
            .map_err(|_| invalid_argument("prepared cascade primary row is not in generation"))?;
        active[position] = false;
    }
    let topology = DetectorTopology::new(generation.prepared())?;
    let mut singleton = Vec::new();
    let mut exact = Vec::new();
    singleton
        .try_reserve_exact(active.len())
        .map_err(|_| allocation("unable to reserve prepared cascade singleton rows"))?;
    exact
        .try_reserve_exact(active.len())
        .map_err(|_| allocation("unable to reserve prepared cascade exact-FE rows"))?;
    let mut exact_checked_effects = 0_usize;
    let mut exact_zero_only_levels = Vec::new();
    let exact_level_capacity = topology
        .effect_to_source
        .iter()
        .zip(&topology.intercept_effect)
        .filter(|(_, intercept)| **intercept)
        .try_fold(0_usize, |total, (source, _)| {
            total.checked_add(topology.sources[*source].n_levels())
        })
        .ok_or_else(|| allocation("prepared cascade exact-FE level capacity overflowed"))?;
    exact_zero_only_levels
        .try_reserve_exact(exact_level_capacity)
        .map_err(|_| allocation("unable to reserve prepared cascade exact-FE levels"))?;
    loop {
        let before = singleton.len() + exact.len();
        if config.drop_singletons {
            let rows = singleton_fixed_point(&topology, generation, &active)?;
            for row in rows {
                if active[row] {
                    active[row] = false;
                    singleton.push(row);
                }
            }
        }
        if config.methods.fixed_effects {
            let proposal = exact_fe_proposal(generation, &active)?;
            exact_checked_effects =
                exact_checked_effects.max(proposal.certificate.checked_intercept_effects);
            exact_zero_only_levels
                .try_reserve(proposal.certificate.zero_only_levels.len())
                .map_err(|_| allocation("unable to extend prepared cascade exact-FE levels"))?;
            exact_zero_only_levels.extend(proposal.certificate.zero_only_levels);
            for row in proposal.rows {
                if active[row] {
                    active[row] = false;
                    exact.push(row);
                }
            }
        }
        if singleton.len() + exact.len() == before {
            break;
        }
    }
    exact_zero_only_levels.sort_unstable();
    exact_zero_only_levels.dedup();
    let exact_fe_rows = compact_to_original(exact, generation.original_rows())?;
    Ok(PreparedCascadeProposal {
        exact_fe_certificate: (!exact_fe_rows.is_empty()).then_some(ExactFeSeparationCertificate {
            checked_intercept_effects: exact_checked_effects,
            zero_only_levels: exact_zero_only_levels,
            passed: true,
        }),
        cascade: PreparedSeparationCascade {
            singleton_rows: compact_to_original(singleton, generation.original_rows())?,
            exact_fe_rows,
        },
    })
}

fn prepared_simplex_proposal(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<DetectionProposal> {
    let categorical = generation.prepared().categorical().ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "prepared simplex has no categorical topology",
        )
    })?;
    let finite_columns = generation.fit_view().fit_model().columns.len();
    let topology = DetectorTopology::new(generation.prepared())?;
    let mut absorbed_columns = 0_usize;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        let channels = usize::from(spec.intercept)
            .checked_add(spec.slopes.len())
            .ok_or_else(|| allocation("prepared simplex channel count overflowed"))?;
        let levels = topology.sources[topology.effect_to_source[effect]].n_levels();
        absorbed_columns =
            absorbed_columns
                .checked_add(levels.checked_mul(channels).ok_or_else(|| {
                    allocation("prepared simplex absorbed-column count overflowed")
                })?)
                .ok_or_else(|| allocation("prepared simplex column count overflowed"))?;
    }
    let columns = finite_columns
        .checked_add(absorbed_columns)
        .ok_or_else(|| allocation("prepared simplex full-model width overflowed"))?;
    if columns == 0 {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Simplex,
            MethodStatus::NoCandidates,
            "prepared full-model simplex has no finite or absorbed columns",
        ));
    }
    let rows = generation.outcome().len();
    let cells = rows
        .checked_mul(columns)
        .ok_or_else(|| allocation("prepared simplex design dimensions overflowed"))?;
    let positive = generation
        .outcome()
        .iter()
        .filter(|value| **value > 0.0)
        .count();
    let boundary = rows.saturating_sub(positive);
    let kernel_peak_cells = simplex_workspace_peak_cells(rows, positive, boundary, columns)?;
    // The temporary topology copies remain live while the dense design and
    // one N-sized u32 FE-code vector are built. Counting each topology entry
    // and u32/bool entry as a full eight-byte cell is deliberately
    // conservative on every supported 64-bit target.
    let topology_cells = topology
        .sources
        .len()
        .checked_add(topology.effect_to_source.len())
        .and_then(|value| value.checked_add(topology.intercept_effect.len()))
        .ok_or_else(|| allocation("prepared simplex topology workspace overflowed"))?;
    let construction_peak_cells = cells
        .checked_add(rows)
        .and_then(|value| value.checked_add(topology_cells))
        .ok_or_else(|| allocation("prepared simplex construction workspace overflowed"))?;
    let peak_cells = kernel_peak_cells.max(construction_peak_cells);
    if peak_cells > config.simplex_max_tableau_cells {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Simplex,
            MethodStatus::SkippedMemory,
            format!(
                "prepared full-model simplex requires at most {peak_cells} simultaneous design-plus-kernel cells, above the configured {} limit",
                config.simplex_max_tableau_cells,
            ),
        ));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(cells)
        .map_err(|_| allocation("unable to reserve prepared full-model simplex design"))?;
    values.resize(cells, 0.0);
    for row in 0..rows {
        for column in 0..finite_columns {
            values[row * columns + column] = generation.fit_view().fit_column_value(column, row)?;
        }
    }
    let mut column_offset = finite_columns;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        let source = topology.sources[topology.effect_to_source[effect]];
        let mut codes = fallible_filled(
            rows,
            u32::MAX,
            "unable to reserve prepared simplex FE row codes",
        )?;
        for level in 0..source.n_levels() {
            let code = u32::try_from(level)
                .map_err(|_| allocation("prepared simplex FE level exceeds u32"))?;
            for &row in source.checked_members(level)? {
                codes[row] = code;
            }
        }
        if codes.contains(&u32::MAX) {
            return Err(invalid_argument("prepared simplex row has no FE level"));
        }
        let channels = usize::from(spec.intercept) + spec.slopes.len();
        for row in 0..rows {
            for level in 0..source.n_levels() {
                let selected = codes[row] as usize == level;
                let mut channel = 0_usize;
                if spec.intercept {
                    values[row * columns + column_offset + level * channels] = f64::from(selected);
                    channel = 1;
                }
                for slope in &spec.slopes {
                    let values_for_slope = categorical.numeric_source_values(slope.source)?;
                    values[row * columns + column_offset + level * channels + channel] =
                        if selected { values_for_slope[row] } else { 0.0 };
                    channel += 1;
                }
            }
        }
        column_offset += source.n_levels() * channels;
    }
    if column_offset != columns || values.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "prepared full-model simplex design is nonfinite or misaligned",
        ));
    }
    // These topology maps and the final FE-code vector are construction
    // scratch. Make their non-overlap with the registered kernel explicit.
    drop(topology);
    let design = DenseMatrix::from_row_major(rows, columns, values)?;
    let mut poll_pivot = || interrupt(PpmlInterruptPoint::SeparationSimplexPivot);
    model_simplex_interruptible(
        generation.outcome(),
        &design,
        config.projection_tolerance,
        config.certificate_tolerance,
        config.simplex_max_iterations,
        config.simplex_max_tableau_cells,
        &mut poll_pivot,
    )
}

fn prepared_iterated_rectifier(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<DetectionProposal> {
    let outcome = generation.outcome();
    let boundary = fallible_collect(
        outcome
            .iter()
            .enumerate()
            .filter_map(|(row, value)| (*value == 0.0).then_some(row)),
        outcome.len(),
        "unable to reserve prepared IR boundary rows",
    )?;
    if boundary.is_empty() {
        return Ok(DetectionProposal::empty(
            SeparationMethod::IteratedRectifier,
            MethodStatus::NoCandidates,
            "no zero-outcome observations",
        ));
    }
    let boundary_weight =
        (config.tolerance * config.tolerance / boundary.len() as f64).max(f64::MIN_POSITIVE);
    let weights = fallible_collect(
        outcome
            .iter()
            .map(|value| if *value > 0.0 { 1.0 } else { boundary_weight }),
        outcome.len(),
        "unable to reserve prepared IR weights",
    )?;
    let mut u = fallible_collect(
        outcome
            .iter()
            .map(|value| if *value == 0.0 { 1.0 } else { 0.0 }),
        outcome.len(),
        "unable to reserve prepared IR recurrence",
    )?;

    for iteration in 1..=config.max_ir_iterations {
        interrupt(PpmlInterruptPoint::SeparationIrIteration)?;
        let projection = prepared_project(generation, &u, &weights, ppml, config, linear)?;
        let positive_direction = projection.fitted;
        let scale = infinity_norm(&positive_direction);
        if scale <= f64::MIN_POSITIVE {
            return Ok(no_separation_proposal(
                SeparationMethod::IteratedRectifier,
                iteration,
                0.0,
                "prepared iterated rectifier converged to zero",
            ));
        }
        let recurrence_scale = scale.max(1.0);
        let threshold = config.tolerance * recurrence_scale;
        let interior = outcome
            .iter()
            .zip(&positive_direction)
            .filter(|(outcome, _)| **outcome > 0.0)
            .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()));
        let minimum_boundary = boundary
            .iter()
            .map(|row| positive_direction[*row])
            .fold(f64::INFINITY, f64::min);
        let maximum_boundary = boundary
            .iter()
            .map(|row| positive_direction[*row])
            .fold(f64::NEG_INFINITY, f64::max);
        let boundary_violation = nonnegative_ir_boundary_violation(minimum_boundary);
        if minimum_boundary >= -threshold
            && maximum_boundary <= threshold
            && interior <= config.certificate_tolerance * recurrence_scale
            && boundary_violation <= config.certificate_tolerance * recurrence_scale
        {
            return Ok(DetectionProposal {
                method: SeparationMethod::IteratedRectifier,
                dropped: Vec::new(),
                iterations: iteration,
                status: MethodStatus::NoSeparation,
                certificate: Some(SeparationCertificate {
                    proof: crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
                    interior_zero_relative: interior / recurrence_scale,
                    boundary_violation_relative: boundary_violation / recurrence_scale,
                    span_residual_relative: 0.0,
                    negative_margin_relative: 0.0,
                    direction_scale: scale,
                    positive_maximum: boundary_violation,
                    passed: true,
                }),
                exact_fe_certificate: None,
                direction: None,
                note: "prepared iterated rectifier certified no separation".to_owned(),
            });
        }
        if minimum_boundary >= -threshold && maximum_boundary > threshold {
            let direction = fallible_collect(
                positive_direction.iter().map(|value| -*value),
                positive_direction.len(),
                "unable to reserve prepared IR direction",
            )?;
            let dropped = fallible_collect(
                boundary
                    .iter()
                    .copied()
                    .filter(|row| direction[*row] < -config.tolerance * scale),
                boundary.len(),
                "unable to reserve prepared IR proposal rows",
            )?;
            if !dropped.is_empty() {
                return Ok(DetectionProposal {
                    method: SeparationMethod::IteratedRectifier,
                    dropped,
                    iterations: iteration,
                    status: MethodStatus::Certified,
                    certificate: None,
                    exact_fe_certificate: None,
                    direction: Some(direction),
                    note: "prepared iterated-rectifier candidate direction".to_owned(),
                });
            }
        }
        let mut maximum = 0.0_f64;
        for row in 0..u.len() {
            u[row] = if outcome[row] > 0.0 || positive_direction[row].abs() <= threshold {
                0.0
            } else {
                positive_direction[row].max(0.0)
            };
            maximum = maximum.max(u[row]);
        }
        if maximum > 0.0 {
            u.iter_mut().for_each(|value| *value /= maximum);
        }
    }
    Ok(DetectionProposal {
        method: SeparationMethod::IteratedRectifier,
        dropped: Vec::new(),
        iterations: config.max_ir_iterations,
        status: MethodStatus::MaximumIterations,
        certificate: None,
        exact_fe_certificate: None,
        direction: None,
        note: "prepared iterated rectifier exhausted its iteration budget".to_owned(),
    })
}

fn nonnegative_ir_boundary_violation(minimum_boundary: f64) -> f64 {
    (-minimum_boundary).max(0.0)
}

fn prepared_mu_from_mean(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    mean: &[f64],
    trajectory_iterations: usize,
) -> Result<DetectionProposal> {
    if mean.len() != generation.original_rows().len()
        || mean.iter().any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(invalid_argument(
            "prepared mu means are not finite, positive, compact-axis aligned values",
        ));
    }
    let minimum_positive = generation
        .outcome()
        .iter()
        .zip(mean)
        .filter_map(|(outcome, mean)| (*outcome > 0.0).then_some(*mean))
        .fold(f64::INFINITY, f64::min);
    let candidate = fallible_collect(
        generation
            .outcome()
            .iter()
            .zip(mean)
            .map(|(outcome, mean)| {
                if *outcome == 0.0 && *mean <= config.mu_tolerance * minimum_positive {
                    1.0
                } else {
                    0.0
                }
            }),
        mean.len(),
        "unable to reserve prepared Mu candidate vector",
    )?;
    if candidate.iter().all(|value| *value == 0.0) {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Mu,
            MethodStatus::NoCandidates,
            "prepared mu found no small zero-outcome means",
        ));
    }
    let weights = fallible_filled(
        candidate.len(),
        1.0,
        "unable to reserve prepared Mu projection weights",
    )?;
    let projected = prepared_project(generation, &candidate, &weights, ppml, config, linear)?;
    let direction = fallible_collect(
        projected.fitted.iter().map(|value| -*value),
        projected.fitted.len(),
        "unable to reserve prepared Mu direction",
    )?;
    let scale = infinity_norm(&direction);
    if scale <= f64::MIN_POSITIVE {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Mu,
            MethodStatus::Uncertified,
            "prepared mu returned a zero direction",
        ));
    }
    let dropped = fallible_collect(
        generation
            .outcome()
            .iter()
            .zip(&direction)
            .enumerate()
            .filter_map(|(row, (outcome, value))| {
                (*outcome == 0.0 && *value < -config.tolerance * scale).then_some(row)
            }),
        direction.len(),
        "unable to reserve prepared Mu proposal rows",
    )?;
    Ok(DetectionProposal {
        method: SeparationMethod::Mu,
        dropped,
        iterations: trajectory_iterations,
        status: MethodStatus::Certified,
        certificate: None,
        exact_fe_certificate: None,
        direction: Some(direction),
        note: "prepared mu candidate direction".to_owned(),
    })
}

struct PreparedProjection {
    fitted: Vec<f64>,
    residual: Vec<f64>,
}

fn prepared_project(
    generation: &PreparedPpmlModelGeneration,
    response: &[f64],
    weights: &[f64],
    ppml: PpmlConfig,
    config: SeparationConfig,
    mut fit_config: PreparedOlsFitConfig,
) -> Result<PreparedProjection> {
    if response.len() != generation.original_rows().len() || weights.len() != response.len() {
        return Err(invalid_argument(
            "prepared separation projection inputs are not compact-axis aligned",
        ));
    }
    let weight_sum = compensated_sum(weights.iter().copied())?;
    let divisor = weights.iter().copied().fold(0.0_f64, f64::max);
    let frame = LinearSolveFrame::from_parts(
        response,
        BorrowedMapWeights::Provided(weights),
        PreparedOlsBreadWeightScale {
            weight_kind: Some(WeightKind::Analytic),
            solver_weight_divisor: divisor,
            reference_weight_multiplier: divisor,
            reference_weight_sum: weight_sum,
        },
    )?;
    let mut expected_absorb = ppml.absorb;
    if expected_absorb.solver == crate::fe::FeSolverKind::Auto {
        expected_absorb.solver = fit_config.absorb.solver;
    }
    // These fields select the already-preflighted execution route or retained
    // publication products; neither changes the detector's scientific solve.
    expected_absorb.solver_memory_budget = fit_config.absorb.solver_memory_budget;
    expected_absorb.retain_coefficients = fit_config.absorb.retain_coefficients;
    if fit_config.absorb != expected_absorb
        || fit_config.fixed_effect_collinearity_tolerance
            != ppml.fixed_effect_collinearity_tolerance
    {
        return Err(invalid_argument(
            "prepared separation linear policy differs from the PPML fit policy",
        ));
    }
    fit_config.score_tolerance = config.projection_tolerance;
    let fit =
        fit_prepared_weighted_linear_projection_once(generation.fit_view(), frame, fit_config)?;
    if !fit.certified() {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared separation projection did not pass its rank, score, FE, and reconstruction certificates",
        ));
    }
    let residual = fallible_copy(
        fit.residuals(),
        "unable to reserve prepared separation projection residuals",
    )?;
    let fitted = fallible_copy(
        fit.fitted(),
        "unable to reserve prepared separation projection fitted values",
    )?;
    if fitted
        .iter()
        .chain(&residual)
        .any(|value| !value.is_finite())
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared separation projection produced a nonfinite value",
        ));
    }
    Ok(PreparedProjection { fitted, residual })
}

/// Re-execute a terminal numerical detector from the immutable generation.
///
/// This is deliberately distinct from `validate_numerical_proposal`: that
/// function certifies a recession direction attached to a deletion proposal,
/// while this replay prevents a no-separation report (especially the IR
/// fixed-point certificate) from certifying itself by pass-through.
#[allow(clippy::too_many_arguments)]
fn replay_numerical_no_drop(
    generation: &PreparedPpmlModelGeneration,
    observed: &DetectionProposal,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    mu_exploration: Option<&PreparedPpmlMuExploration>,
) -> Result<Option<SeparationCertificate>> {
    if !observed.dropped.is_empty() || observed.direction.is_some() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "no-drop separation replay received a deletion direction",
        ));
    }
    if !matches!(
        observed.status,
        MethodStatus::NoCandidates
            | MethodStatus::NoSeparation
            | MethodStatus::SkippedMemory
            | MethodStatus::SkippedUnsupported
            | MethodStatus::Uncertified
    ) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "terminal numerical detector did not produce a replayable no-drop status",
        ));
    }
    let replayed = match observed.method {
        SeparationMethod::Simplex => prepared_simplex_proposal(generation, config, interrupt)?,
        SeparationMethod::IteratedRectifier => {
            prepared_iterated_rectifier(generation, ppml, config, linear, interrupt)?
        }
        SeparationMethod::Mu => {
            let exploration = mu_exploration.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared Mu certification replay omitted its fitted generation state",
                )
            })?;
            prepared_mu_from_mean(
                generation,
                ppml,
                config,
                linear,
                exploration.mean(),
                exploration.certificate().accepted_iterations,
            )?
        }
        SeparationMethod::Singletons | SeparationMethod::FixedEffects => {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "exact separation method entered the numerical replay path",
            ));
        }
    };
    if replayed.method != observed.method
        || replayed.status != observed.status
        || replayed.iterations != observed.iterations
        || !replayed.dropped.is_empty()
        || replayed.direction.is_some()
        || replayed.certificate != observed.certificate
        || replayed.exact_fe_certificate != observed.exact_fe_certificate
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "terminal numerical separation result failed independent deterministic replay",
        ));
    }
    // The publication report carries the independently replayed proof, not a
    // clone of the detector's original self-issued certificate.  Equality
    // above still ensures deterministic agreement between the two executions.
    Ok(replayed.certificate)
}

fn validate_numerical_proposal(
    generation: &PreparedPpmlModelGeneration,
    proposal: &DetectionProposal,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
) -> Result<Option<SeparationCertificate>> {
    if proposal.dropped.is_empty() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "deletion-certificate validation received a no-drop separation proposal",
        ));
    }
    let direction = proposal.direction.as_deref().ok_or_else(|| {
        FeregError::new(
            ErrorCode::NonConvergence,
            "prepared numerical separation proposal omitted its direction",
        )
    })?;
    if direction.len() != generation.original_rows().len()
        || direction.iter().any(|value| !value.is_finite())
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared numerical separation direction is not finite and aligned",
        ));
    }
    let scale = infinity_norm(direction);
    if scale <= f64::MIN_POSITIVE {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared numerical separation direction is zero",
        ));
    }
    let unit = fallible_filled(
        direction.len(),
        1.0,
        "unable to reserve prepared separation certificate weights",
    )?;
    let projection = prepared_project(generation, direction, &unit, ppml, config, linear)?;
    let span_residual_relative = euclidean_norm(&projection.residual) / euclidean_norm(direction);
    let interior_zero_relative = generation
        .outcome()
        .iter()
        .zip(direction)
        .filter(|(outcome, _)| **outcome > 0.0)
        .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()))
        / scale;
    let positive_maximum = generation
        .outcome()
        .iter()
        .zip(direction)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(f64::NEG_INFINITY, f64::max);
    let threshold = config.tolerance * scale;
    let certified_rows = fallible_collect(
        generation
            .outcome()
            .iter()
            .zip(direction)
            .enumerate()
            .filter_map(|(row, (outcome, value))| {
                (*outcome == 0.0 && *value < -threshold).then_some(row)
            }),
        direction.len(),
        "unable to reserve prepared separation certified rows",
    )?;
    if certified_rows != proposal.dropped {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared numerical proposal does not equal its strict-negative certificate set",
        ));
    }
    let negative_margin_relative = certified_rows
        .iter()
        .map(|row| -direction[*row] / scale)
        .fold(f64::INFINITY, f64::min);
    let certificate = SeparationCertificate {
        proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
        interior_zero_relative,
        boundary_violation_relative: positive_maximum.max(0.0) / scale,
        span_residual_relative,
        negative_margin_relative,
        direction_scale: scale,
        positive_maximum: positive_maximum.max(0.0),
        passed: interior_zero_relative <= config.certificate_tolerance
            && positive_maximum <= config.certificate_tolerance * scale
            && span_residual_relative <= config.projection_tolerance
            && negative_margin_relative >= config.certificate_tolerance,
    };
    if !certificate.passed {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "prepared separation direction failed certification: interior={:.3e}, boundary={:.3e}, span={:.3e}",
                certificate.interior_zero_relative,
                certificate.boundary_violation_relative,
                certificate.span_residual_relative,
            ),
        ));
    }
    Ok(Some(certificate))
}

fn no_separation_proposal(
    method: SeparationMethod,
    iterations: usize,
    scale: f64,
    note: &str,
) -> DetectionProposal {
    DetectionProposal {
        method,
        dropped: Vec::new(),
        iterations,
        status: MethodStatus::NoSeparation,
        certificate: Some(SeparationCertificate {
            proof: crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
            interior_zero_relative: 0.0,
            boundary_violation_relative: 0.0,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale: scale,
            positive_maximum: 0.0,
            passed: true,
        }),
        exact_fe_certificate: None,
        direction: None,
        note: note.to_owned(),
    }
}

fn proposal_report(
    generation: usize,
    proposal: DetectionProposal,
    certificate: Option<SeparationCertificate>,
    started: Instant,
    config: SeparationConfig,
) -> SeparationMethodReport {
    method_report(
        generation,
        proposal.method,
        proposal.status,
        proposal.iterations,
        Vec::new(),
        Vec::new(),
        certificate,
        proposal.exact_fe_certificate,
        started,
        config,
        &proposal.note,
    )
}

#[allow(clippy::too_many_arguments)]
fn method_report(
    generation: usize,
    method: SeparationMethod,
    status: MethodStatus,
    iterations: usize,
    dropped_observations: Vec<usize>,
    singleton_drops: Vec<usize>,
    certificate: Option<SeparationCertificate>,
    exact_fe_certificate: Option<ExactFeSeparationCertificate>,
    started: Instant,
    config: SeparationConfig,
    note: &str,
) -> SeparationMethodReport {
    SeparationMethodReport {
        generation,
        method,
        status,
        iterations,
        dropped_observations,
        singleton_drops,
        certificate,
        exact_fe_certificate,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        workspace_observation_vectors: workspace_vectors(method),
        simplex_tableau_cells: if method == SeparationMethod::Simplex {
            config.simplex_max_tableau_cells
        } else {
            0
        },
        note: note.to_owned(),
    }
}

fn exact_followup_report(
    generation: usize,
    rows: Vec<usize>,
    certificate: ExactFeSeparationCertificate,
) -> SeparationMethodReport {
    SeparationMethodReport {
        generation,
        method: SeparationMethod::FixedEffects,
        status: MethodStatus::Certified,
        iterations: 1,
        dropped_observations: rows,
        singleton_drops: Vec::new(),
        certificate: None,
        exact_fe_certificate: Some(certificate),
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 1,
        simplex_tableau_cells: 0,
        note: "prepared exact-FE follow-up in the same atomic generation".to_owned(),
    }
}

fn append_no_boundary_reports(
    reports: &mut Vec<SeparationMethodReport>,
    generation: usize,
    config: SeparationConfig,
) -> Result<()> {
    for (enabled, method) in [
        (config.methods.simplex, SeparationMethod::Simplex),
        (
            config.methods.iterated_rectifier,
            SeparationMethod::IteratedRectifier,
        ),
        (config.methods.mu, SeparationMethod::Mu),
    ] {
        if enabled {
            fallible_push(
                reports,
                SeparationMethodReport {
                    generation,
                    method,
                    status: MethodStatus::NoCandidates,
                    iterations: 0,
                    dropped_observations: Vec::new(),
                    singleton_drops: Vec::new(),
                    certificate: None,
                    exact_fe_certificate: None,
                    elapsed_seconds: 0.0,
                    workspace_observation_vectors: workspace_vectors(method),
                    simplex_tableau_cells: if method == SeparationMethod::Simplex {
                        config.simplex_max_tableau_cells
                    } else {
                        0
                    },
                    note: "no zero-outcome observations".to_owned(),
                },
                "unable to reserve prepared no-boundary method report",
            )?;
        }
    }
    Ok(())
}

fn fail_closed_comprehensive(proposal: &DetectionProposal) -> Result<()> {
    if matches!(
        proposal.status,
        MethodStatus::MaximumIterations
            | MethodStatus::Uncertified
            | MethodStatus::SkippedMemory
            | MethodStatus::SkippedUnsupported
    ) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "prepared comprehensive separation did not certify: {}",
                proposal.note
            ),
        ));
    }
    Ok(())
}

fn compact_to_original(mut compact: Vec<usize>, original_rows: &[usize]) -> Result<Vec<usize>> {
    for row in &mut compact {
        *row = *original_rows
            .get(*row)
            .ok_or_else(|| invalid_argument("prepared detector compact row is out of range"))?;
    }
    compact.sort_unstable();
    compact.dedup();
    Ok(compact)
}

fn remap_original_rows_to_candidate(
    rows: &mut [usize],
    candidate_original_rows: &[usize],
    label: &str,
) -> Result<()> {
    for row in rows {
        *row = candidate_original_rows.binary_search(row).map_err(|_| {
            FeregError::new(
                ErrorCode::Internal,
                format!("prepared separation {label} contains a noncandidate original row"),
            )
        })?;
    }
    Ok(())
}

fn validate_config(config: SeparationConfig) -> Result<()> {
    if [
        config.tolerance,
        config.certificate_tolerance,
        config.projection_tolerance,
        config.mu_tolerance,
    ]
    .iter()
    .any(|value| !value.is_finite() || *value <= 0.0 || *value >= 1.0)
    {
        return Err(invalid_argument(
            "prepared separation tolerances must lie strictly between zero and one",
        ));
    }
    if config.max_ir_iterations == 0
        || config.simplex_max_iterations == 0
        || config.simplex_max_tableau_cells == 0
    {
        return Err(invalid_argument(
            "prepared separation iteration and memory budgets must be positive",
        ));
    }
    Ok(())
}

fn require_positive_outcome(outcome: &[f64]) -> Result<()> {
    if !outcome.iter().any(|outcome| *outcome > 0.0) {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "prepared separation left no positive outcome",
        ));
    }
    Ok(())
}

fn infinity_norm(values: &[f64]) -> f64 {
    values
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
}

fn euclidean_norm(values: &[f64]) -> f64 {
    values
        .iter()
        .fold(0.0_f64, |sum, value| value.mul_add(*value, sum))
        .sqrt()
}

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> Result<f64> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for value in values {
        if !value.is_finite() || value <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "prepared separation projection weights must be finite and positive",
            ));
        }
        let next = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
    }
    let total = sum + correction;
    if !total.is_finite() || total <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "prepared separation projection weight sum is invalid",
        ));
    }
    Ok(total)
}

fn candidate_ledger_hash_original_axis(
    candidate_original_rows: &[usize],
    original_axis_active: &[bool],
    original_axis_reasons: &[Option<SeparationDropReason>],
) -> Result<u64> {
    if original_axis_active.len() != original_axis_reasons.len()
        || candidate_original_rows
            .iter()
            .any(|row| *row >= original_axis_active.len())
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared separation cannot hash a misaligned candidate ledger",
        ));
    }
    candidate_ledger_hash_with(candidate_original_rows, |_, original_row| {
        (
            original_axis_active[original_row],
            original_axis_reasons[original_row],
        )
    })
}

/// Replay the same canonical hash from a compact separation-report ledger.
/// The stable candidate-to-original map supplies row identity without adding
/// another observation-sized vector to `SeparationReport`.
pub(super) fn candidate_ledger_hash_candidate_axis(
    candidate_original_rows: &[usize],
    candidate_axis_active: &[bool],
    candidate_axis_reasons: &[Option<SeparationDropReason>],
) -> Result<u64> {
    if candidate_original_rows.len() != candidate_axis_active.len()
        || candidate_axis_active.len() != candidate_axis_reasons.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared separation cannot hash a misaligned compact candidate ledger",
        ));
    }
    candidate_ledger_hash_with(candidate_original_rows, |candidate_row, _| {
        (
            candidate_axis_active[candidate_row],
            candidate_axis_reasons[candidate_row],
        )
    })
}

fn candidate_ledger_hash_with<F>(candidate_original_rows: &[usize], mut state: F) -> Result<u64>
where
    F: FnMut(usize, usize) -> (bool, Option<SeparationDropReason>),
{
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for (candidate_row, &original_row) in candidate_original_rows.iter().enumerate() {
        let (is_active, reason) = state(candidate_row, original_row);
        hash ^= u64::try_from(candidate_row).map_err(|_| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared separation candidate ordinal does not fit the ledger hash",
            )
        })?;
        hash = hash.wrapping_mul(PRIME);
        // Stable original-row identity is part of the generation ledger.
        // Omitting it made equal-length subsets with equal active/reason
        // patterns collide (for example [1, 2] and [3, 4]).
        hash ^= u64::try_from(original_row).map_err(|_| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared separation original row does not fit the ledger hash",
            )
        })?;
        hash = hash.wrapping_mul(PRIME);
        hash ^= u64::from(is_active);
        hash = hash.wrapping_mul(PRIME);
        hash ^= reason_code(reason);
        hash = hash.wrapping_mul(PRIME);
    }
    Ok(hash)
}

const fn reason_code(reason: Option<SeparationDropReason>) -> u64 {
    match reason {
        None => 0,
        Some(SeparationDropReason::InitialSingleton) => 1,
        Some(SeparationDropReason::FixedEffectZeroLevel) => 2,
        Some(SeparationDropReason::Simplex) => 3,
        Some(SeparationDropReason::IteratedRectifier) => 4,
        Some(SeparationDropReason::Mu) => 5,
        Some(SeparationDropReason::SingletonCascade) => 6,
    }
}

const fn workspace_vectors(method: SeparationMethod) -> usize {
    match method {
        SeparationMethod::Singletons => 0,
        SeparationMethod::FixedEffects => 1,
        SeparationMethod::Simplex => 0,
        SeparationMethod::IteratedRectifier | SeparationMethod::Mu => 8,
    }
}

const fn replay_bit(method: SeparationMethod) -> u8 {
    match method {
        SeparationMethod::Singletons => 1 << 0,
        SeparationMethod::FixedEffects => 1 << 1,
        SeparationMethod::Simplex => 1 << 2,
        SeparationMethod::IteratedRectifier => 1 << 3,
        SeparationMethod::Mu => 1 << 4,
    }
}

const fn requested_numerical_replay_mask(config: SeparationConfig) -> u8 {
    let mut mask = 0_u8;
    if config.methods.simplex {
        mask |= replay_bit(SeparationMethod::Simplex);
    }
    if config.methods.iterated_rectifier {
        mask |= replay_bit(SeparationMethod::IteratedRectifier);
    }
    if config.methods.mu {
        mask |= replay_bit(SeparationMethod::Mu);
    }
    mask
}

const fn requested_replay_mask(config: SeparationConfig) -> u8 {
    let mut mask = requested_numerical_replay_mask(config);
    if config.drop_singletons {
        mask |= replay_bit(SeparationMethod::Singletons);
    }
    if config.methods.fixed_effects {
        mask |= replay_bit(SeparationMethod::FixedEffects);
    }
    mask
}

#[derive(Debug, Clone, Copy)]
struct SeparationDigest(u64);

impl SeparationDigest {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, value: u8) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    fn usize(&mut self, value: usize) -> Result<()> {
        self.u64(u64::try_from(value).map_err(|_| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML separation report value does not fit its certification digest",
            )
        })?);
        Ok(())
    }

    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }
}

fn stable_usize_slice_digest(domain: &[u8], values: &[usize]) -> Result<u64> {
    let mut digest = SeparationDigest::new();
    digest.usize(domain.len())?;
    for byte in domain {
        digest.byte(*byte);
    }
    digest.usize(values.len())?;
    for value in values {
        digest.usize(*value)?;
    }
    Ok(digest.0)
}

fn stable_f64_slice_digest(domain: &[u8], values: &[f64]) -> Result<u64> {
    let mut digest = SeparationDigest::new();
    digest.usize(domain.len())?;
    for byte in domain {
        digest.byte(*byte);
    }
    digest.usize(values.len())?;
    for value in values {
        if !value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML separation generation binding contains a nonfinite value",
            ));
        }
        digest.u64(value.to_bits());
    }
    Ok(digest.0)
}

fn separation_report_digest(report: &SeparationReport) -> Result<u64> {
    let mut digest = SeparationDigest::new();
    for byte in b"fereg.ppml.certified-separation.v1" {
        digest.byte(*byte);
    }
    digest.usize(report.candidate_observations)?;
    digest.usize(report.singleton_count)?;
    digest.usize(report.separated_count)?;
    digest.usize(report.generations)?;
    digest.usize(report.active.len())?;
    for active in &report.active {
        digest.bool(*active);
    }
    digest.usize(report.retained_observations.len())?;
    for row in &report.retained_observations {
        digest.usize(*row)?;
    }
    digest.usize(report.dropped_observations.len())?;
    for row in &report.dropped_observations {
        digest.usize(*row)?;
    }
    digest.usize(report.drop_reasons.len())?;
    for reason in &report.drop_reasons {
        digest.byte(reason_code(*reason) as u8);
    }
    digest.usize(report.generation_hashes.len())?;
    for hash in &report.generation_hashes {
        digest.u64(*hash);
    }
    digest.usize(report.methods.len())?;
    for method in &report.methods {
        digest.usize(method.generation)?;
        digest.byte(match method.method {
            SeparationMethod::Singletons => 1,
            SeparationMethod::FixedEffects => 2,
            SeparationMethod::Simplex => 3,
            SeparationMethod::IteratedRectifier => 4,
            SeparationMethod::Mu => 5,
        });
        digest.byte(match method.status {
            MethodStatus::NoCandidates => 1,
            MethodStatus::NoSeparation => 2,
            MethodStatus::Certified => 3,
            MethodStatus::SkippedMemory => 4,
            MethodStatus::SkippedUnsupported => 5,
            MethodStatus::MaximumIterations => 6,
            MethodStatus::Uncertified => 7,
        });
        digest.usize(method.iterations)?;
        digest.usize(method.dropped_observations.len())?;
        for row in &method.dropped_observations {
            digest.usize(*row)?;
        }
        digest.usize(method.singleton_drops.len())?;
        for row in &method.singleton_drops {
            digest.usize(*row)?;
        }
        digest_certificate(&mut digest, method.certificate.as_ref())?;
        match &method.exact_fe_certificate {
            None => digest.byte(0),
            Some(certificate) => {
                digest.byte(1);
                digest.usize(certificate.checked_intercept_effects)?;
                digest.usize(certificate.zero_only_levels.len())?;
                for (effect, level) in &certificate.zero_only_levels {
                    digest.usize(*effect)?;
                    digest.u64(u64::from(*level));
                }
                digest.bool(certificate.passed);
            }
        }
        digest.usize(method.workspace_observation_vectors)?;
        digest.usize(method.simplex_tableau_cells)?;
    }
    Ok(digest.0)
}

fn digest_certificate(
    digest: &mut SeparationDigest,
    certificate: Option<&SeparationCertificate>,
) -> Result<()> {
    let Some(certificate) = certificate else {
        digest.byte(0);
        return Ok(());
    };
    digest.byte(1);
    match certificate.proof {
        crate::ppml_separation::SeparationCertificateProof::RecessionDirection => digest.byte(1),
        crate::ppml_separation::SeparationCertificateProof::SimplexFullPositiveRank { columns } => {
            digest.byte(2);
            digest.usize(columns)?;
        }
        crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint => {
            digest.byte(3);
        }
    }
    for value in [
        certificate.interior_zero_relative,
        certificate.boundary_violation_relative,
        certificate.span_residual_relative,
        certificate.negative_margin_relative,
        certificate.direction_scale,
        certificate.positive_maximum,
    ] {
        digest.u64(value.to_bits());
    }
    digest.bool(certificate.passed);
    Ok(())
}

/// Test-only constructor for the strictly disabled detector path.  It cannot
/// manufacture replay evidence for a requested method.
#[cfg(test)]
pub(crate) fn certify_disabled_separation_for_test(
    config: SeparationConfig,
    report: &SeparationReport,
    final_original_rows: &[usize],
    final_outcome: &[f64],
) -> Result<CertifiedSeparationSeal> {
    if requested_replay_mask(config) != 0 || !report.methods.is_empty() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "test-only disabled separation seal cannot certify an enabled detector",
        ));
    }
    FinalGenerationReplayToken::from_parts(
        report.generations,
        final_original_rows,
        final_outcome,
        0,
        config,
    )?
    .seal(config, report)
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn fallible_filled<T: Clone>(len: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| allocation(message))?;
    output.resize(len, value);
    Ok(output)
}

fn fallible_copy<T: Clone>(values: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation(message))?;
    output.extend_from_slice(values);
    Ok(output)
}

fn fallible_collect<T>(
    values: impl IntoIterator<Item = T>,
    capacity: usize,
    message: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| allocation(message))?;
    output.extend(values);
    Ok(output)
}

fn fallible_push<T>(output: &mut Vec<T>, value: T, message: &'static str) -> Result<()> {
    output.try_reserve(1).map_err(|_| allocation(message))?;
    output.push(value);
    Ok(())
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{NumericColumnView, NumericSourceUse, WeightInput};
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use crate::ols::{
        LoadedNumericColumn, LoadedOlsInput, OlsClusterTermV1, OlsInputBinding, OlsInputRole,
        OlsRequestV1,
    };
    use crate::ppml::prepared::{prepare_ppml_numeric_data, PpmlNumericPreparationInput};
    use crate::ppml::PreparedPpmlNumericData;
    use crate::ppml_separation::SeparationMethods;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;

    static PHASE_PROBE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static PHASE_PROBE_EVENTS: Mutex<Vec<(PpmlPhase, super::super::PpmlPhaseAllocatorEvent, u64)>> =
        Mutex::new(Vec::new());
    static PHASE_PROBE_ENDS: AtomicU64 = AtomicU64::new(0);
    static MU_PHASE_ACTIVE: AtomicBool = AtomicBool::new(false);
    static MU_FIT_CALLS: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn impossible_detector_workspace_reservation_maps_to_allocation() {
        let error = fallible_collect(
            std::iter::empty::<usize>(),
            usize::MAX,
            "injected prepared separation allocation failure",
        )
        .expect_err("an impossible data-sized reservation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
    }

    fn deterministic_phase_probe(
        phase: PpmlPhase,
        event: super::super::PpmlPhaseAllocatorEvent,
    ) -> u64 {
        let value = match event {
            super::super::PpmlPhaseAllocatorEvent::Begin => 0,
            super::super::PpmlPhaseAllocatorEvent::End => {
                PHASE_PROBE_ENDS.fetch_add(1, Ordering::SeqCst) + 1
            }
        };
        PHASE_PROBE_EVENTS
            .lock()
            .expect("phase-probe events")
            .push((phase, event, value));
        value
    }

    fn mu_scope_probe(phase: PpmlPhase, event: super::super::PpmlPhaseAllocatorEvent) -> u64 {
        if phase == PpmlPhase::SeparationMu {
            MU_PHASE_ACTIVE.store(
                event == super::super::PpmlPhaseAllocatorEvent::Begin,
                Ordering::SeqCst,
            );
        }
        0
    }

    #[test]
    fn phase_probe_brackets_disabled_stages_and_max_merges_restarts() {
        let _guard = PHASE_PROBE_TEST_LOCK.lock().expect("phase-probe lock");
        PHASE_PROBE_ENDS.store(0, Ordering::SeqCst);
        PHASE_PROBE_EVENTS
            .lock()
            .expect("phase-probe events")
            .clear();
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: true,
                    iterated_rectifier: true,
                    mu: false,
                },
                drop_singletons: true,
                ..SeparationConfig::default()
            },
            Some(deterministic_phase_probe),
            None,
        )
        .expect("phase-observed separation");
        let events = PHASE_PROBE_EVENTS.lock().expect("phase-probe events");
        for phase in SEPARATION_PHASES {
            let begins = events
                .iter()
                .filter(|(observed, event, _)| {
                    *observed == phase && *event == super::super::PpmlPhaseAllocatorEvent::Begin
                })
                .count();
            let ends = events
                .iter()
                .filter(|(observed, event, _)| {
                    *observed == phase && *event == super::super::PpmlPhaseAllocatorEvent::End
                })
                .count();
            assert!(begins > 0, "missing phase boundary for {phase:?}");
            assert_eq!(begins, ends, "unbalanced phase boundary for {phase:?}");
            let expected_peak = events
                .iter()
                .filter_map(|(observed, event, value)| {
                    (*observed == phase && *event == super::super::PpmlPhaseAllocatorEvent::End)
                        .then_some(*value)
                })
                .max();
            assert_eq!(result.phase_allocation_peak(phase), expected_peak);
        }
        assert!(
            events
                .iter()
                .filter(|(phase, event, _)| {
                    *phase == PpmlPhase::SeparationFe
                        && *event == super::super::PpmlPhaseAllocatorEvent::End
                })
                .count()
                > 1,
            "the fixture must exercise max-merging across an FE restart"
        );
    }

    #[test]
    fn mu_phase_and_report_charge_the_fitted_start_solve() {
        let _guard = PHASE_PROBE_TEST_LOCK.lock().expect("phase-probe lock");
        MU_PHASE_ACTIVE.store(false, Ordering::SeqCst);
        MU_FIT_CALLS.store(0, Ordering::SeqCst);
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
        let ppml = PpmlConfig::default();
        let linear = PreparedOlsFitConfig {
            absorb: ppml.absorb,
            fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            ..PreparedOlsFitConfig::default()
        };
        let mut interrupt = |_| Ok(());
        let (result, artifact) = coordinate_prepared_ppml_separation(
            coordinator,
            ppml,
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: true,
                    mu: true,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            Some(mu_scope_probe),
            linear,
            &mut interrupt,
            |generation, _| {
                assert!(
                    MU_PHASE_ACTIVE.load(Ordering::SeqCst),
                    "the fitted Mu start must be solved inside the Mu phase"
                );
                MU_FIT_CALLS.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(5));
                PreparedPpmlMuExploration::from_test_eta(
                    vec![0.0; generation.original_rows().len()],
                    generation.outcome(),
                    generation.prior_weights(),
                    ppml.bounds,
                )
            },
            |_, _| Ok(17_u8),
        )
        .expect("Mu-coordinated fit");

        assert_eq!(artifact, 17);
        assert_eq!(MU_FIT_CALLS.load(Ordering::SeqCst), 1);
        assert!(!MU_PHASE_ACTIVE.load(Ordering::SeqCst));
        let mu = result
            .separation
            .methods
            .iter()
            .find(|report| report.method == SeparationMethod::Mu)
            .expect("requested Mu report");
        assert_ne!(mu.status, MethodStatus::SkippedUnsupported);
        assert!(
            mu.elapsed_seconds >= 0.004,
            "Mu report excluded its fitted-start solve: {} seconds",
            mu.elapsed_seconds
        );
    }

    #[test]
    fn candidate_ledger_hash_binds_stable_original_rows() {
        let active = [false, true, true, true, true];
        let reasons = [None; 5];
        let first = candidate_ledger_hash_original_axis(&[1, 2], &active, &reasons)
            .expect("first candidate ledger hash");
        let second = candidate_ledger_hash_original_axis(&[3, 4], &active, &reasons)
            .expect("second candidate ledger hash");
        assert_ne!(first, second);

        let compact_active = [true, true];
        let compact_reasons = [None, None];
        assert_eq!(
            first,
            candidate_ledger_hash_candidate_axis(&[1, 2], &compact_active, &compact_reasons)
                .expect("compact replay hash")
        );
    }

    #[test]
    fn ir_no_separation_boundary_diagnostic_is_never_negative() {
        assert_eq!(nonnegative_ir_boundary_violation(0.25), 0.0);
        assert_eq!(nonnegative_ir_boundary_violation(0.0), 0.0);
        assert_eq!(nonnegative_ir_boundary_violation(-0.25), 0.25);
    }

    #[test]
    fn invalid_detector_tolerance_fails_closed_before_preparation() {
        let error = validate_config(SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: true,
            },
            mu_tolerance: f64::NAN,
            ..SeparationConfig::default()
        })
        .expect_err("nonfinite mu tolerance must fail closed");
        assert_eq!(error.code, ErrorCode::InvalidArgument);

        let error = validate_config(SeparationConfig {
            tolerance: 1.0,
            ..SeparationConfig::default()
        })
        .expect_err("unit separation tolerance must fail closed");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn exact_fe_proposal_deduplicates_rows_across_absorbed_effects() {
        let (mut request, clusters, mut loaded, mut uses) = fixture();
        let h = SourceId::new(3).expect("h");
        request.absorbed_terms.push(AbsorbedTerm {
            id: AbsorbTermId::new(2).expect("second absorb"),
            intercept: true,
            categorical_sources: vec![h],
            slope_sources: Vec::new(),
            label: "h".to_owned(),
            saved_name: String::new(),
        });
        uses[2].fixed_effect = true;
        loaded.outcome = column(&[0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("two-way prepared owner");
        let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
        let generation = coordinator.view().prepared;
        let proposal = exact_fe_proposal(generation, &[true; 6]).expect("exact FE proposal");

        // The raw zero-only memberships contain seven entries because rows
        // 0 and 2 belong to certified levels in both effects.  The proposal
        // remains a unique, bounded original-row list.
        assert_eq!(proposal.rows, vec![0, 1, 2, 3, 4]);
        assert_eq!(proposal.certificate.zero_only_levels.len(), 3);
        assert!(proposal.certificate.passed);
    }

    #[test]
    fn singleton_pass_deduplicates_rows_across_absorbed_effects() {
        let (mut request, clusters, loaded, mut uses) = fixture();
        let h = SourceId::new(3).expect("h");
        request.absorbed_terms.push(AbsorbedTerm {
            id: AbsorbTermId::new(2).expect("second absorb"),
            intercept: true,
            categorical_sources: vec![h],
            slope_sources: Vec::new(),
            label: "h".to_owned(),
            saved_name: String::new(),
        });
        uses[2].fixed_effect = true;
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("two-way prepared owner");
        let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
        let generation = coordinator.view().prepared;
        let topology = DetectorTopology::new(generation.prepared()).expect("topology");

        // Row zero is the sole active member of a level in both absorbed
        // effects. The pass emits it once and never grows beyond N.
        let rows = singleton_fixed_point(
            &topology,
            generation,
            &[true, false, false, false, false, false],
        )
        .expect("singleton pass");
        assert_eq!(rows, vec![0]);
    }

    #[test]
    fn exact_then_comprehensive_ir_rebuilds_fe_and_cluster_topology_twice() {
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: true,
                    iterated_rectifier: true,
                    mu: false,
                },
                drop_singletons: true,
                ..SeparationConfig::default()
            },
            None,
            None,
        )
        .expect("prepared separation");

        assert_eq!(result.prepared.generation(), 2);
        assert_eq!(result.prepared.original_rows(), &[4, 5]);
        assert_eq!(result.prepared.outcome(), &[1.0, 0.0]);
        assert_eq!(result.prepared.cluster_codes(0).expect("cluster"), &[0, 1]);
        assert_eq!(
            result
                .prepared
                .cluster_design()
                .expect("cluster design")
                .dimensions()[0]
                .groups(),
            2
        );
        assert_eq!(result.separation.generations, 2);
        assert_eq!(result.separation.generation_hashes.len(), 3);
        assert_eq!(
            result.separation.drop_reasons[0],
            Some(SeparationDropReason::FixedEffectZeroLevel)
        );
        assert_eq!(
            result.separation.drop_reasons[1],
            Some(SeparationDropReason::FixedEffectZeroLevel)
        );
        assert_eq!(
            result.separation.drop_reasons[2],
            Some(SeparationDropReason::SingletonCascade)
        );
        assert_eq!(
            result.separation.drop_reasons[3],
            Some(SeparationDropReason::Simplex)
        );
        assert!(result.separation.methods.iter().any(|report| {
            report.method == SeparationMethod::FixedEffects
                && report.status == MethodStatus::Certified
                && report.generation == 1
        }));
        assert!(result.separation.methods.iter().any(|report| {
            report.method == SeparationMethod::Simplex
                && report.status == MethodStatus::Certified
                && report.generation == 2
                && report.singleton_drops == vec![2]
        }));
        assert!(result.separation.methods.iter().any(|report| {
            report.method == SeparationMethod::Simplex
                && report.status == MethodStatus::Certified
                && report
                    .certificate
                    .as_ref()
                    .is_some_and(|certificate| certificate.passed)
        }));
    }

    #[test]
    fn over_budget_prepared_simplex_requires_ir_or_returns_allocation() {
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let error = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: true,
                    iterated_rectifier: false,
                    mu: false,
                },
                drop_singletons: true,
                simplex_max_tableau_cells: 1,
                ..SeparationConfig::default()
            },
            None,
            None,
        )
        .err()
        .expect("simplex-only memory skip must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn prepared_simplex_executes_the_full_absorbed_slope_model() {
        let (mut request, clusters, loaded, mut uses) = fixture();
        request.absorbed_terms[0]
            .slope_sources
            .push(SourceId::new(1).expect("x"));
        uses[0].fixed_effect = true;
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared slope owner");
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: false,
                    simplex: true,
                    iterated_rectifier: true,
                    mu: false,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
            None,
        )
        .expect("bounded full-model slope simplex");
        let simplex = result
            .separation
            .methods
            .iter()
            .find(|report| report.method == SeparationMethod::Simplex)
            .expect("simplex report");
        assert_ne!(simplex.status, MethodStatus::SkippedUnsupported);
        assert_ne!(simplex.status, MethodStatus::SkippedMemory);
        assert!(simplex.simplex_tableau_cells > 0);
    }

    #[test]
    fn no_separation_is_an_identity_generation_on_the_candidate_axis() {
        let (request, clusters, mut loaded, uses) = fixture();
        loaded.outcome = column(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig::default(),
            None,
            None,
        )
        .expect("prepared no-separation result");

        assert_eq!(result.prepared.generation(), 0);
        assert_eq!(result.prepared.original_rows(), &[0, 1, 2, 3, 4, 5]);
        assert_eq!(result.candidate_original_rows.as_ref(), &[0, 1, 2, 3, 4, 5]);
        assert_eq!(result.separation.active, vec![true; 6]);
        assert_eq!(
            result.separation.retained_observations,
            vec![0, 1, 2, 3, 4, 5]
        );
        assert!(result.separation.dropped_observations.is_empty());
        assert!(result.separation.drop_reasons.iter().all(Option::is_none));
        assert_eq!(result.separation.generation_hashes.len(), 1);
        assert_eq!(
            result
                .separation
                .methods
                .iter()
                .map(|report| (report.method, report.status))
                .collect::<Vec<_>>(),
            vec![
                (SeparationMethod::Singletons, MethodStatus::NoSeparation),
                (SeparationMethod::FixedEffects, MethodStatus::NoSeparation),
                (SeparationMethod::Simplex, MethodStatus::NoCandidates),
                (
                    SeparationMethod::IteratedRectifier,
                    MethodStatus::NoCandidates,
                ),
            ]
        );
    }

    #[test]
    fn disabled_detector_chain_preserves_boundary_rows() {
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: false,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
            None,
        )
        .expect("disabled separation chain");

        assert_eq!(result.prepared.generation(), 0);
        assert_eq!(result.prepared.original_rows(), &[0, 1, 2, 3, 4, 5]);
        assert!(result.separation.methods.is_empty());
        assert_eq!(result.separation.active, vec![true; 6]);
    }

    #[test]
    fn prepared_ir_is_comprehensive_without_the_simplex_adapter() {
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let config = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let result =
            detect_prepared_ppml_separation(owner, PpmlConfig::default(), config, None, None)
                .expect("prepared IR separation");

        assert_eq!(result.separation.dropped_observations, vec![0, 1, 3]);
        assert_eq!(result.prepared.original_rows(), &[2, 4, 5]);
        assert!(result.separation.methods.iter().any(|report| {
            report.method == SeparationMethod::IteratedRectifier
                && report.status == MethodStatus::Certified
                && report.dropped_observations == vec![0, 1, 3]
                && report
                    .certificate
                    .as_ref()
                    .is_some_and(|value| value.passed)
        }));
        assert!(result.separation.methods.iter().any(|report| {
            report.method == SeparationMethod::IteratedRectifier
                && report.status == MethodStatus::NoSeparation
                && report.certificate.as_ref().is_some_and(|certificate| {
                    certificate.positive_maximum >= 0.0
                        && certificate.boundary_violation_relative >= 0.0
                })
        }));
        result
            .certification_seal
            .validate(
                config,
                &result.separation,
                result.prepared.original_rows(),
                result.prepared.outcome(),
            )
            .expect("terminal IR replay seal");

        let mut mutated = result.separation.clone();
        let terminal_ir = mutated
            .methods
            .iter_mut()
            .rev()
            .find(|report| {
                report.method == SeparationMethod::IteratedRectifier
                    && report.status == MethodStatus::NoSeparation
            })
            .expect("terminal IR fixed-point report");
        terminal_ir
            .certificate
            .as_mut()
            .expect("terminal IR certificate")
            .positive_maximum += f64::EPSILON;
        assert_eq!(
            result
                .certification_seal
                .validate(
                    config,
                    &mutated,
                    result.prepared.original_rows(),
                    result.prepared.outcome(),
                )
                .expect_err("mutated pass-through IR certificate must not match the replay seal")
                .code,
            ErrorCode::Internal
        );

        let disabled = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            ..config
        };
        assert!(result
            .certification_seal
            .validate(
                disabled,
                &result.separation,
                result.prepared.original_rows(),
                result.prepared.outcome(),
            )
            .is_err());
    }

    #[test]
    fn prepared_mu_uses_original_axis_start_and_certifies_its_candidate() {
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let start = PpmlStart {
            coefficients: Vec::new(),
            linear_predictor: vec![0.0, 0.0, 0.0, -30.0, 0.0, 0.0],
        };
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: true,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
            Some(&start),
        )
        .expect("prepared mu separation");

        assert_eq!(result.separation.dropped_observations, vec![3]);
        assert_eq!(result.prepared.original_rows(), &[0, 1, 2, 4, 5]);
        assert_eq!(
            result.separation.drop_reasons[3],
            Some(SeparationDropReason::Mu)
        );
        assert!(result.separation.methods.iter().any(|report| {
            report.method == SeparationMethod::Mu
                && report.status == MethodStatus::Certified
                && report.dropped_observations == vec![3]
                && report
                    .certificate
                    .as_ref()
                    .is_some_and(|value| value.passed)
        }));
    }

    #[test]
    fn default_chain_is_deterministic_and_uses_the_declared_restart_order() {
        let run = || {
            let (request, clusters, loaded, uses) = fixture();
            let numeric = prepare_numeric(&loaded, &uses);
            let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
                .expect("prepared owner");
            detect_prepared_ppml_separation(
                owner,
                PpmlConfig::default(),
                SeparationConfig::default(),
                None,
                None,
            )
            .expect("default prepared separation")
        };
        let first = run();
        let second = run();
        let signature = |outcome: &PreparedPpmlSeparationOutcome| {
            outcome
                .separation
                .methods
                .iter()
                .map(|report| {
                    (
                        report.generation,
                        report.method,
                        report.status,
                        report.dropped_observations.clone(),
                        report.singleton_drops.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(first.prepared.original_rows(), &[4, 5]);
        assert_eq!(
            first.separation.generation_hashes,
            second.separation.generation_hashes
        );
        assert_eq!(
            first.separation.drop_reasons,
            second.separation.drop_reasons
        );
        assert_eq!(signature(&first), signature(&second));
        assert_eq!(
            signature(&first)
                .iter()
                .map(|(_, method, _, _, _)| *method)
                .collect::<Vec<_>>(),
            vec![
                SeparationMethod::Singletons,
                SeparationMethod::FixedEffects,
                SeparationMethod::Singletons,
                SeparationMethod::FixedEffects,
                SeparationMethod::Simplex,
                SeparationMethod::Singletons,
                SeparationMethod::FixedEffects,
                SeparationMethod::Simplex,
                SeparationMethod::IteratedRectifier,
            ]
        );
    }

    #[test]
    fn failed_prepared_rebase_leaves_generation_and_ledger_unchanged() {
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let mut coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");

        let error = coordinator
            .transact_validated_proposal(SeparationDropReason::Simplex, &[1, 3, 5], |_| {
                Ok(PreparedSeparationCascade::default())
            })
            .expect_err("one cluster group must reject the staged generation");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(coordinator.view().generation, 0);
        assert_eq!(coordinator.view().original_rows, &[0, 1, 2, 3, 4, 5]);
        assert!(coordinator.reasons().iter().all(Option::is_none));
        assert!(coordinator.active().iter().all(|active| *active));

        coordinator
            .transact_validated_proposal(SeparationDropReason::Simplex, &[0], |_| {
                Ok(PreparedSeparationCascade::default())
            })
            .expect("valid transaction after rollback");
        assert_eq!(coordinator.view().generation, 1);
        assert_eq!(coordinator.view().original_rows, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn public_report_is_candidate_compact_while_final_reasons_use_original_rows() {
        let (request, clusters, mut loaded, uses) = fixture();
        loaded.candidate[0] = false;
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: false,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
            None,
        )
        .expect("candidate-axis separation");

        assert_eq!(result.candidate_original_rows.as_ref(), &[1, 2, 3, 4, 5]);
        assert_eq!(result.separation.candidate_observations, 5);
        assert_eq!(result.separation.dropped_observations, vec![0]);
        assert_eq!(
            result.separation.drop_reasons[0],
            Some(SeparationDropReason::FixedEffectZeroLevel)
        );
        assert_eq!(
            result.original_axis_reasons[1],
            Some(SeparationDropReason::FixedEffectZeroLevel)
        );
        assert_eq!(result.original_axis_reasons[0], None);
        let final_reasons = result.final_drop_reasons().expect("merged original ledger");
        assert_eq!(final_reasons[0], DropReason::ExcludedByIfIn);
        assert_eq!(final_reasons[1], DropReason::FixedEffectSeparation);
    }

    #[test]
    fn separation_rebase_preserves_offset_cluster_and_final_pweight_scale() {
        let (mut request, clusters, mut loaded, uses) = fixture();
        request.bindings.push(OlsInputBinding {
            stata_var_position: 6,
            role: OlsInputRole::ProbabilityWeight,
        });
        loaded.probability_weight = Some(column(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]));
        let offset = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5];
        let probability = loaded.probability_weight.as_ref().expect("pweights");
        let numeric = prepare_numeric_with(
            &loaded,
            &uses,
            WeightInput::Provided {
                kind: WeightKind::Probability,
                column: NumericColumnView::with_missing(&probability.values, &probability.missing),
            },
            Some(NumericColumnView::new(&offset)),
        );
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared weighted owner");
        let result = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: true,
                    iterated_rectifier: true,
                    mu: false,
                },
                drop_singletons: true,
                ..SeparationConfig::default()
            },
            None,
            None,
        )
        .expect("weighted prepared separation");

        assert_eq!(result.prepared.original_rows(), &[4, 5]);
        assert_eq!(result.prepared.offset(), &[0.4, 0.5]);
        assert_eq!(result.prepared.cluster_codes(0).expect("codes"), &[0, 1]);
        assert_eq!(
            result
                .prepared
                .cluster_design()
                .expect("cluster design")
                .dimensions()[0]
                .groups(),
            2
        );
        let weights = result.prepared.prior_weights();
        assert!((weights.iter().sum::<f64>() - 2.0).abs() < 1e-12);
        assert_eq!(result.prepared.weight_report().physical_observations, 2);
        assert!((result.prepared.weight_report().prepared_weight_sum - 2.0).abs() < 1e-12);
        assert!(
            (result
                .prepared
                .weight_report()
                .raw_weight_sum
                .expect("raw sum")
                - 11.0)
                .abs()
                < 1e-12
        );
        assert!(
            (result
                .prepared
                .weight_report()
                .raw_to_prepared_multiplier
                .expect("representable multiplier")
                - 2.0 / 11.0)
                .abs()
                < 1e-12
        );

        let weight_sum = weights.iter().sum::<f64>();
        let divisor = weights.iter().copied().fold(0.0_f64, f64::max);
        let frame = LinearSolveFrame::from_parts(
            result.prepared.outcome(),
            BorrowedMapWeights::Provided(weights),
            PreparedOlsBreadWeightScale {
                weight_kind: Some(WeightKind::Probability),
                solver_weight_divisor: divisor,
                reference_weight_multiplier: result
                    .prepared
                    .weight_report()
                    .raw_to_prepared_multiplier
                    .expect("representable projection multiplier"),
                reference_weight_sum: weight_sum,
            },
        )
        .expect("rank frame");
        let fit = fit_prepared_weighted_linear_projection_once(
            result.prepared.fit_view(),
            frame,
            PreparedOlsFitConfig::default(),
        )
        .expect("rank-certified final generation");
        assert_eq!(fit.finite_rank(), 0);
    }

    fn fixture() -> (
        OlsRequestV1,
        Vec<OlsClusterTermV1>,
        LoadedOlsInput,
        Vec<NumericSourceUse>,
    ) {
        let x = SourceId::new(1).expect("x");
        let g = SourceId::new(2).expect("g");
        let h = SourceId::new(3).expect("h");
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![source(x, "x"), source(g, "g"), source(h, "h")],
                terms: vec![ExpandedTerm {
                    id: TermId::new(1).expect("term"),
                    order: TermOrder::new(1).expect("order"),
                    coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: false,
                    canonical_token: "c.x".to_owned(),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).expect("component"),
                        source: x,
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.0,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::Unavailable,
                        raw_operator: "c".to_owned(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    }],
                }],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorb"),
                intercept: true,
                categorical_sources: vec![g],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: String::new(),
            }],
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(x),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(g),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(h),
                },
            ],
            include_intercept: true,
            keep_singletons: false,
        };
        let loaded = LoadedOlsInput {
            candidate: vec![true; 6],
            outcome: column(&[0.0, 0.0, 1.0, 0.0, 1.0, 0.0]),
            numeric_sources: vec![
                (x, column(&[0.0, 0.0, 0.0, -1.0, 0.0, 0.0])),
                (g, column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0])),
                (h, column(&[10.0, 20.0, 10.0, 20.0, 10.0, 20.0])),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        (
            request,
            vec![OlsClusterTermV1 {
                label: "h".to_owned(),
                sources: vec![h],
            }],
            loaded,
            vec![
                NumericSourceUse::ordinary(),
                NumericSourceUse::fixed_effect(),
                NumericSourceUse {
                    ordinary: false,
                    fixed_effect: false,
                    cluster: true,
                },
            ],
        )
    }

    fn prepare_numeric(
        loaded: &LoadedOlsInput,
        uses: &[NumericSourceUse],
    ) -> PreparedPpmlNumericData {
        prepare_numeric_with(loaded, uses, WeightInput::None, None)
    }

    fn prepare_numeric_with<'a>(
        loaded: &'a LoadedOlsInput,
        uses: &[NumericSourceUse],
        weights: WeightInput<'a>,
        offset: Option<NumericColumnView<'a>>,
    ) -> PreparedPpmlNumericData {
        let sources = loaded
            .numeric_sources
            .iter()
            .map(|(_, source)| NumericColumnView::with_missing(&source.values, &source.missing))
            .collect::<Vec<_>>();
        prepare_ppml_numeric_data(&PpmlNumericPreparationInput {
            candidate: &loaded.candidate,
            outcome: NumericColumnView::with_missing(
                &loaded.outcome.values,
                &loaded.outcome.missing,
            ),
            numeric_sources: &sources,
            source_uses: uses,
            weights,
            offset,
            exposure: None,
        })
        .expect("numeric preparation")
    }

    fn source(id: SourceId, label: &str) -> SourceEntry {
        SourceEntry {
            id,
            canonical_name: label.to_owned(),
        }
    }

    fn column(values: &[f64]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        }
    }
}
