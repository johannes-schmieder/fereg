use super::accounting::{map_proposal_to_original, validate_detection_inputs, validate_proposal};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{FeDesign, SingletonMultiplicity};
use crate::ols::FiniteSolver;
use crate::ppml::{PpmlConfig, PpmlStart};
use crate::ppml_separation::fe::propose_zero_only_fixed_effect_levels;
use crate::ppml_separation::ir::{iterated_rectifier, IteratedRectifierConfig};
use crate::ppml_separation::ledger::{LedgerCommit, ProposalCommitContext, SampleLedger};
use crate::ppml_separation::mu::detect_mu;
use crate::ppml_separation::projection::{ProjectionConfig, LEGACY_PROJECTION_RANK_TOLERANCE};
use crate::ppml_separation::simplex::model_simplex;
use crate::ppml_separation::subset::subset_problem;
use crate::ppml_separation::{
    DetectionProposal, MethodStatus, SeparationCertificate, SeparationConfig, SeparationDropReason,
    SeparationMethod, SeparationMethodReport, SeparationReport,
};

pub fn detect_separation(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    ppml: PpmlConfig,
    config: SeparationConfig,
    start: Option<&PpmlStart>,
) -> Result<SeparationReport> {
    detect_separation_with_multiplicity(
        outcome,
        regressors,
        fixed_effects,
        ppml,
        config,
        start,
        SingletonMultiplicity::Unit,
    )
}

/// Internal prepared-data entry point.  PPML preparation passes literal
/// fweight multiplicities here; unweighted and pweighted samples use `Unit`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn detect_separation_with_multiplicity(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    ppml: PpmlConfig,
    config: SeparationConfig,
    start: Option<&PpmlStart>,
    multiplicity: SingletonMultiplicity<'_>,
) -> Result<SeparationReport> {
    validate_detection_inputs(outcome, regressors, fixed_effects, config)?;
    if let SingletonMultiplicity::Frequency(values) = multiplicity {
        if values.len() != outcome.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "frequency singleton multiplicity has the wrong observation count",
            ));
        }
    }

    let mut ledger = SampleLedger::new(outcome.len());
    let mut reports = Vec::new();
    loop {
        require_positive_outcome(outcome, ledger.active())?;

        let singleton_started = Instant::now();
        if let Some(commit) =
            ledger.commit_initial_singletons(fixed_effects, config.drop_singletons, multiplicity)?
        {
            reports.push(with_method_telemetry(
                SeparationMethodReport {
                    generation: commit.generation,
                    method: SeparationMethod::Singletons,
                    status: MethodStatus::Certified,
                    iterations: 1,
                    dropped_observations: Vec::new(),
                    singleton_drops: commit.singleton_drops,
                    certificate: None,
                    exact_fe_certificate: None,
                    elapsed_seconds: 0.0,
                    workspace_observation_vectors: 0,
                    simplex_tableau_cells: 0,
                    note: "singleton pruning reached its queue fixed point".into(),
                },
                singleton_started,
                config,
            ));
            continue;
        }

        if config.methods.fixed_effects {
            let method_started = Instant::now();
            let proposal =
                propose_zero_only_fixed_effect_levels(outcome, fixed_effects, ledger.active());
            let certificate = validate_proposal(
                &proposal,
                SeparationMethod::FixedEffects,
                outcome,
                fixed_effects,
                ledger.active(),
                None,
                ppml,
                config,
            )?;
            if !proposal.dropped.is_empty() {
                let commit = ledger.commit_validated_proposal(
                    &proposal.dropped,
                    SeparationDropReason::FixedEffectZeroLevel,
                    ProposalCommitContext {
                        outcome,
                        fixed_effects,
                        drop_singletons: config.drop_singletons,
                        run_exact_fe_followup: true,
                        multiplicity,
                    },
                )?;
                reports.push(with_method_telemetry(
                    committed_report(proposal, certificate, &commit, true),
                    method_started,
                    config,
                ));
                continue;
            }
            reports.push(with_method_telemetry(
                uncommitted_report(proposal, certificate, ledger.generation()),
                method_started,
                config,
            ));
        }

        if !has_active_boundary(outcome, ledger.active()) {
            append_boundary_empty_reports(&mut reports, ledger.generation(), config);
            break;
        }

        let mut simplex_skipped_for_memory = false;
        if config.methods.simplex {
            let method_started = Instant::now();
            let subset = subset_problem(outcome, regressors, fixed_effects, ledger.active())?;
            let proposal = model_simplex(
                &subset.outcome,
                &subset.regressors,
                config.tolerance,
                config.certificate_tolerance,
                config.simplex_max_iterations,
                config.simplex_max_tableau_cells,
            )?;
            let proposal = map_proposal_to_original(proposal, &subset.original_rows)?;
            let certificate = validate_proposal(
                &proposal,
                SeparationMethod::Simplex,
                outcome,
                fixed_effects,
                ledger.active(),
                Some(&subset),
                ppml,
                config,
            )?;
            simplex_skipped_for_memory = proposal.status == MethodStatus::SkippedMemory;
            if !proposal.dropped.is_empty() {
                let commit = ledger.commit_validated_proposal(
                    &proposal.dropped,
                    SeparationDropReason::Simplex,
                    ProposalCommitContext {
                        outcome,
                        fixed_effects,
                        drop_singletons: config.drop_singletons,
                        run_exact_fe_followup: config.methods.fixed_effects,
                        multiplicity,
                    },
                )?;
                append_committed_reports(
                    &mut reports,
                    proposal,
                    certificate,
                    &commit,
                    method_started,
                    config,
                );
                continue;
            }
            reports.push(with_method_telemetry(
                uncommitted_report(proposal, certificate, ledger.generation()),
                method_started,
                config,
            ));
        }

        let mut comprehensive_certified = false;
        if config.methods.iterated_rectifier {
            let method_started = Instant::now();
            let subset = subset_problem(outcome, regressors, fixed_effects, ledger.active())?;
            let proposal = iterated_rectifier(
                &subset.outcome,
                &subset.regressors,
                subset.fixed_effects.as_ref(),
                IteratedRectifierConfig {
                    projection: ProjectionConfig {
                        absorb: ppml.absorb,
                        finite_solver: FiniteSolver::Auto,
                        rank_tolerance: LEGACY_PROJECTION_RANK_TOLERANCE,
                        collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
                        certificate_tolerance: config.projection_tolerance,
                    },
                    tolerance: config.tolerance,
                    certificate_tolerance: config.certificate_tolerance,
                    max_iterations: config.max_ir_iterations,
                    strict: false,
                },
            )?;
            let proposal = map_proposal_to_original(proposal, &subset.original_rows)?;
            fail_closed_comprehensive_status(&proposal)?;
            let certificate = validate_proposal(
                &proposal,
                SeparationMethod::IteratedRectifier,
                outcome,
                fixed_effects,
                ledger.active(),
                Some(&subset),
                ppml,
                config,
            )?;
            comprehensive_certified = proposal.status == MethodStatus::NoSeparation
                && certificate
                    .as_ref()
                    .is_some_and(|certificate| certificate.passed);
            if !proposal.dropped.is_empty() {
                let commit = ledger.commit_validated_proposal(
                    &proposal.dropped,
                    SeparationDropReason::IteratedRectifier,
                    ProposalCommitContext {
                        outcome,
                        fixed_effects,
                        drop_singletons: config.drop_singletons,
                        run_exact_fe_followup: config.methods.fixed_effects,
                        multiplicity,
                    },
                )?;
                append_committed_reports(
                    &mut reports,
                    proposal,
                    certificate,
                    &commit,
                    method_started,
                    config,
                );
                continue;
            }
            reports.push(with_method_telemetry(
                uncommitted_report(proposal, certificate, ledger.generation()),
                method_started,
                config,
            ));
        }
        if simplex_skipped_for_memory && !comprehensive_certified {
            return Err(FeregError::new(
                ErrorCode::Allocation,
                "simplex tableau exceeds its memory budget and comprehensive IR did not certify the generation",
            ));
        }

        if config.methods.mu {
            let method_started = Instant::now();
            let subset = subset_problem(outcome, regressors, fixed_effects, ledger.active())?;
            let proposal = detect_mu(&subset, ppml, config, start, ledger.active())?;
            let proposal = map_proposal_to_original(proposal, &subset.original_rows)?;
            if matches!(
                proposal.status,
                MethodStatus::MaximumIterations | MethodStatus::Uncertified
            ) {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "mu separation did not produce a certified result",
                ));
            }
            let certificate = validate_proposal(
                &proposal,
                SeparationMethod::Mu,
                outcome,
                fixed_effects,
                ledger.active(),
                Some(&subset),
                ppml,
                config,
            )?;
            if !proposal.dropped.is_empty() {
                let commit = ledger.commit_validated_proposal(
                    &proposal.dropped,
                    SeparationDropReason::Mu,
                    ProposalCommitContext {
                        outcome,
                        fixed_effects,
                        drop_singletons: config.drop_singletons,
                        run_exact_fe_followup: config.methods.fixed_effects,
                        multiplicity,
                    },
                )?;
                append_committed_reports(
                    &mut reports,
                    proposal,
                    certificate,
                    &commit,
                    method_started,
                    config,
                );
                continue;
            }
            reports.push(with_method_telemetry(
                uncommitted_report(proposal, certificate, ledger.generation()),
                method_started,
                config,
            ));
        }
        break;
    }

    require_positive_outcome(outcome, ledger.active())?;
    Ok(ledger.into_report(reports))
}

fn append_committed_reports(
    reports: &mut Vec<SeparationMethodReport>,
    proposal: DetectionProposal,
    certificate: Option<SeparationCertificate>,
    commit: &LedgerCommit,
    method_started: Instant,
    config: SeparationConfig,
) {
    reports.push(with_method_telemetry(
        committed_report(proposal, certificate, commit, false),
        method_started,
        config,
    ));
    if !commit.exact_fe_drops.is_empty() {
        reports.push(SeparationMethodReport {
            generation: commit.generation,
            method: SeparationMethod::FixedEffects,
            status: MethodStatus::Certified,
            iterations: 1,
            dropped_observations: commit.exact_fe_drops.clone(),
            singleton_drops: Vec::new(),
            certificate: None,
            exact_fe_certificate: commit.exact_fe_certificate.clone(),
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 0,
            simplex_tableau_cells: 0,
            note: "exact zero-only FE follow-up in the same atomic ledger generation".into(),
        });
    }
}

fn committed_report(
    mut proposal: DetectionProposal,
    certificate: Option<SeparationCertificate>,
    commit: &LedgerCommit,
    include_exact_followups: bool,
) -> SeparationMethodReport {
    let mut dropped_observations = commit.primary_drops.clone();
    if include_exact_followups {
        dropped_observations.extend(&commit.exact_fe_drops);
        dropped_observations.sort_unstable();
        dropped_observations.dedup();
    }
    SeparationMethodReport {
        generation: commit.generation,
        method: proposal.method,
        status: proposal.status,
        iterations: proposal.iterations,
        dropped_observations,
        singleton_drops: commit.singleton_drops.clone(),
        certificate,
        exact_fe_certificate: if include_exact_followups {
            merge_exact_certificates(
                proposal.exact_fe_certificate.take(),
                commit.exact_fe_certificate.clone(),
            )
        } else {
            proposal.exact_fe_certificate.take()
        },
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 0,
        simplex_tableau_cells: 0,
        note: proposal.note,
    }
}

fn merge_exact_certificates(
    left: Option<crate::ppml_separation::ExactFeSeparationCertificate>,
    right: Option<crate::ppml_separation::ExactFeSeparationCertificate>,
) -> Option<crate::ppml_separation::ExactFeSeparationCertificate> {
    match (left, right) {
        (None, None) => None,
        (Some(certificate), None) | (None, Some(certificate)) => Some(certificate),
        (Some(left), Some(right)) => {
            let mut zero_only_levels = left.zero_only_levels;
            zero_only_levels.extend(right.zero_only_levels);
            zero_only_levels.sort_unstable();
            zero_only_levels.dedup();
            Some(crate::ppml_separation::ExactFeSeparationCertificate {
                checked_intercept_effects: left
                    .checked_intercept_effects
                    .max(right.checked_intercept_effects),
                zero_only_levels,
                passed: left.passed && right.passed,
            })
        }
    }
}

fn uncommitted_report(
    mut proposal: DetectionProposal,
    certificate: Option<SeparationCertificate>,
    generation: usize,
) -> SeparationMethodReport {
    SeparationMethodReport {
        generation,
        method: proposal.method,
        status: proposal.status,
        iterations: proposal.iterations,
        dropped_observations: proposal.dropped,
        singleton_drops: Vec::new(),
        certificate,
        exact_fe_certificate: proposal.exact_fe_certificate.take(),
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 0,
        simplex_tableau_cells: 0,
        note: proposal.note,
    }
}

fn fail_closed_comprehensive_status(proposal: &DetectionProposal) -> Result<()> {
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
                "requested comprehensive iterated-rectifier separation did not certify: {}",
                proposal.note
            ),
        ));
    }
    Ok(())
}

fn append_boundary_empty_reports(
    reports: &mut Vec<SeparationMethodReport>,
    generation: usize,
    config: SeparationConfig,
) {
    for (enabled, method, note) in [
        (
            config.methods.simplex,
            SeparationMethod::Simplex,
            "no zero-outcome observations",
        ),
        (
            config.methods.iterated_rectifier,
            SeparationMethod::IteratedRectifier,
            "no zero-outcome observations",
        ),
        (
            config.methods.mu,
            SeparationMethod::Mu,
            "no zero-outcome observations",
        ),
    ] {
        if enabled {
            reports.push(SeparationMethodReport {
                generation,
                method,
                status: MethodStatus::NoCandidates,
                iterations: 0,
                dropped_observations: Vec::new(),
                singleton_drops: Vec::new(),
                certificate: None,
                exact_fe_certificate: None,
                elapsed_seconds: 0.0,
                workspace_observation_vectors: method_workspace_observation_vectors(method),
                simplex_tableau_cells: if method == SeparationMethod::Simplex {
                    config.simplex_max_tableau_cells
                } else {
                    0
                },
                note: note.into(),
            });
        }
    }
}

fn with_method_telemetry(
    mut report: SeparationMethodReport,
    started: Instant,
    config: SeparationConfig,
) -> SeparationMethodReport {
    report.elapsed_seconds = started.elapsed().as_secs_f64();
    report.workspace_observation_vectors = method_workspace_observation_vectors(report.method);
    report.simplex_tableau_cells = if report.method == SeparationMethod::Simplex {
        config.simplex_max_tableau_cells
    } else {
        0
    };
    report
}

const fn method_workspace_observation_vectors(method: SeparationMethod) -> usize {
    match method {
        SeparationMethod::Singletons | SeparationMethod::FixedEffects => 0,
        SeparationMethod::Simplex => 2,
        SeparationMethod::IteratedRectifier | SeparationMethod::Mu => 8,
    }
}

fn has_active_boundary(outcome: &[f64], active: &[bool]) -> bool {
    outcome
        .iter()
        .zip(active)
        .any(|(outcome, active)| *active && *outcome == 0.0)
}

fn require_positive_outcome(outcome: &[f64], active: &[bool]) -> Result<()> {
    if !outcome
        .iter()
        .zip(active)
        .any(|(outcome, active)| *active && *outcome > 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "separation and singleton removal left no positive weighted outcome",
        ));
    }
    Ok(())
}
use std::time::Instant;
