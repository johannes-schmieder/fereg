//! Certified separation over immutable prepared PPML generations.
//!
//! This is the production bridge between the PPML-owned prepared source
//! owner and the sample ledger.  It never constructs the legacy `FeDesign`.
//! Exact FE and singleton methods consume prepared membership directly. The
//! prepared simplex constructs one explicitly bounded full finite-plus-FE
//! design, including absorbed slope channels. Comprehensive projection uses
//! the shared certified prepared weighted-linear boundary.

use crate::linalg::neumaier_add;
use crate::memory::{fallible_copy, fallible_filled};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::data::{DenseMatrix, DropReason, WeightKind};
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::fe::{BorrowedMapWeights, FeSolverKind, LevelIndex};
use crate::ols::{
    fit_prepared_weighted_linear_projection_once, LinearSolveFrame, PreparedOlsBreadWeightScale,
    PreparedOlsFitConfig,
};
use crate::ppml_separation::{
    model_simplex_interruptible, separation_generation_capacity, separation_method_count,
    simplex_workspace_peak_cells, PREFLIGHT_REPORT_GENERATIONS,
};
use crate::ppml_separation::{
    DetectionProposal, ExactFeLevelIdentity, ExactFeSeparationCertificate, MethodStatus,
    PreparedSeparationCascade, PreparedSeparationCoordinator, SeparationCertificate,
    SeparationConfig, SeparationDropReason, SeparationMethod, SeparationMethodReport,
    SeparationNote, SeparationReport,
};
use crate::prepared_fit_view::PreparedOlsFitView;

use super::memory::{begin_phase_allocation, end_phase_allocation};
use super::prepared_estimator::PpmlInterruptPoint;
use super::prepared_irls::PreparedPpmlMuExploration;
use super::prepared_model::{
    PreparedPpmlModel, PreparedPpmlModelGeneration, PreparedPpmlModelOwner,
};
use super::{PpmlConfig, PpmlMemoryPlan, PpmlPhase, PpmlPhaseAllocatorProbe};

const SEPARATION_PHASES: [PpmlPhase; 4] = [
    PpmlPhase::SeparationFe,
    PpmlPhase::SeparationSimplex,
    PpmlPhase::SeparationIr,
    PpmlPhase::SeparationMu,
];

// Test fault plans; shipped builds compile these branches out.
crate::fault::fault_plan! {
    static FAIL_EXACT_FE_LEVEL_COMPACTION: bool = false;
    static FAIL_REPORT_BANK_RESERVATION: bool = false;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PreparedSeparationPhasePeaks {
    values: [Option<u64>; SEPARATION_PHASES.len()],
}

/// Production numerical-detector handoff.
///
/// The numerical kernels are allowed to use compact row positions while they
/// work, but compact positions never cross the immutable-generation boundary.
/// This proposal binds the detector result to one prepared generation and
/// carries only stable original-row identifiers.  In particular, binding
/// preserves detector order verbatim: it must not sort or deduplicate a
/// malformed result into something the coordinator could accept.
#[derive(Debug, Clone, PartialEq)]
struct PreparedNumericalProposal {
    generation: usize,
    generation_rows_digest: u64,
    generation_outcome_digest: u64,
    method: SeparationMethod,
    dropped_original_rows: Vec<usize>,
    iterations: usize,
    status: MethodStatus,
    certificate: Option<SeparationCertificate>,
    exact_fe_certificate: Option<ExactFeSeparationCertificate>,
    /// Observation-space direction on this proposal's compact generation.
    direction: Option<Vec<f64>>,
    exact_intercept_span: Option<graph::ExactInterceptSpan>,
    note: SeparationNote,
    /// Stored projection of a terminal no-separation rectifier iteration; the
    /// no-drop check verifies it instead of re-running the detector.
    fixed_point: Option<IrFixedPointEvidence>,
}

/// The rectifier's final recurrence vector, the residual of its certified
/// weighted projection onto the regressor and FE span, and the kept finite
/// columns of that projection. Fitted values are `recurrence - residual`,
/// exactly as the projection formed them.
#[derive(Debug, Clone, PartialEq)]
struct IrFixedPointEvidence {
    recurrence: Vec<f64>,
    residual: Vec<f64>,
    kept_columns: Vec<usize>,
}

/// Validation capability consumed by the atomic commit path.  Keeping this
/// distinct from the detector proposal prevents a caller from pairing stable
/// rows from one method/generation with a reason or certificate from another.
#[derive(Debug, Clone, PartialEq)]
struct ValidatedNumericalProposal {
    proposal: PreparedNumericalProposal,
    reason: SeparationDropReason,
    certificate: SeparationCertificate,
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
    pub(crate) initial_preparation_elapsed: Duration,
    pub(crate) initial_preparation_peak: Option<u64>,
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
        let bound = if self.config != config
            || self.final_generation != report.generations
            || self.final_generation_hash != report.generation_hashes.last().copied().unwrap_or(0)
        {
            false
        } else {
            let (rows, outcome, trace) = if parallel_digest_team(final_original_rows.len()) {
                let ((rows, outcome), trace) = rayon::join(
                    || final_frame_digests(final_original_rows, final_outcome),
                    || separation_report_digest(report),
                );
                (rows, outcome, trace)
            } else {
                let (rows, outcome) = final_frame_digests(final_original_rows, final_outcome);
                (rows, outcome, separation_report_digest(report))
            };
            // Independent byte streams retain their complete traversal and
            // comparison/error precedence; no digest is cached or reassociated.
            self.final_rows_digest == rows?
                && self.final_outcome_digest == outcome?
                && self.report_digest == trace?
                && self.replay_mask == expected_mask
        };
        if !bound {
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
        let (rows, outcome) = final_frame_digests(final_original_rows, final_outcome);
        Ok(Self {
            final_generation,
            final_rows_digest: rows?,
            final_outcome_digest: outcome?,
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

fn parallel_digest_team(rows: usize) -> bool {
    // Public PPML already owns an installed, scoped team. Standalone callers
    // keep serial execution rather than initializing an unrelated global pool.
    crate::parallel::StagePlan::current(rows as u128 * 2, 2).lanes > 1
        && rayon::current_thread_index().is_some()
}

fn final_frame_digests(rows: &[usize], outcome: &[f64]) -> (Result<u64>, Result<u64>) {
    let row_digest = || stable_usize_slice_digest(b"final-rows", rows);
    let outcome_digest = || stable_f64_slice_digest(b"final-outcome", outcome);
    if parallel_digest_team(rows.len()) {
        rayon::join(row_digest, outcome_digest)
    } else {
        (row_digest(), outcome_digest())
    }
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
    mut execution: PreparedPpmlSeparationExecution,
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
    let (coordinator, initial_preparation_elapsed, initial_preparation_peak) =
        prepare_initial_generation(owner, phase_allocation_probe)?;
    let (projection_ppml, projection_linear) =
        admit_detector_projection(coordinator.view().prepared, ppml, config, &mut execution)?;
    let (mut outcome, fit) = coordinate_prepared_ppml_separation(
        coordinator,
        projection_ppml,
        config,
        phase_allocation_probe,
        projection_linear,
        execution.preliminary_plan.separation_projection_bytes > 0,
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
    )?;
    outcome.initial_preparation_elapsed = initial_preparation_elapsed;
    outcome.initial_preparation_peak = initial_preparation_peak;
    Ok((outcome, fit))
}

/// Graph proposals and retained CG need extra detector workspace, not an IRLS cache. Resolve
/// its complete bound on generation zero; later generations only shrink.
fn admit_detector_projection(
    generation: &PreparedPpmlModelGeneration,
    mut ppml: PpmlConfig,
    config: SeparationConfig,
    execution: &mut PreparedPpmlSeparationExecution,
) -> Result<(PpmlConfig, PreparedOlsFitConfig)> {
    let mut linear = execution.linear;
    if ppml.absorb.solver != FeSolverKind::Map
        || !config.methods.iterated_rectifier
        || !generation.outcome().contains(&0.0)
    {
        return Ok((ppml, linear));
    }
    let Some(design) = generation.fit_view().fit_borrowed_intercept_map_design()? else {
        return Ok((ppml, linear));
    };
    let Some(graph_bytes) = graph::workspace_bytes(&design)? else {
        return Ok((ppml, linear));
    };
    let levels = fallible_collect(
        design.effects().iter().map(|e| e.n_levels()),
        design.effects().len(),
        "unable to reserve separation solver level counts",
    )?;
    let rhs = generation
        .model()
        .columns
        .len()
        .checked_add(1)
        .ok_or_else(|| allocation("separation projection width overflows"))?;
    let estimate = crate::memory::estimate_fe_solver_memory(
        generation.original_rows().len(),
        rhs,
        &levels,
        linear.execution.threads.resolved(),
        linear.absorb.lsmr.local_size,
    )?;
    // CG's retained products preserve its FE coefficient representation. Charge
    // return/conversion overlap and full raw/within banks even for Low's otherwise
    // streamed finite solve. Graph traversal dies before any projection begins.
    let cg_bytes = estimate
        .coefficient_batch_bytes
        .checked_mul(5)
        .and_then(|n| n.checked_add(estimate.cg_bytes))
        .and_then(|n| {
            estimate
                .rhs_copy_bytes
                .checked_mul(2)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            (generation.original_rows().len() as u128)
                .checked_mul(16)
                .and_then(|m| n.checked_add(m))
        })
        .ok_or_else(|| allocation("separation projection memory overflows"))?;
    let bytes = graph_bytes.max(cg_bytes);
    let base = execution.preliminary_plan.selected_bytes();
    if !execution
        .preliminary_plan
        .admit_separation_projection(bytes)
    {
        return Ok((ppml, linear));
    }
    linear.absorb.solver = FeSolverKind::Cg;
    linear.absorb.retain_coefficients = true;
    linear.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
        execution_bytes: base,
        usable_bytes: base
            .checked_add(bytes)
            .ok_or_else(|| allocation("separation projection budget overflows"))?,
    });
    ppml.absorb = linear.absorb;
    Ok((ppml, linear))
}

#[path = "prepared_separation_graph.rs"]
mod graph;

/// A separate scope makes success and failed initial construction observable;
/// later rebuilds remain nested in the requesting separation phase.
fn prepare_initial_generation(
    owner: PreparedPpmlModelOwner,
    phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
) -> Result<(
    PreparedSeparationCoordinator<PreparedPpmlModelOwner>,
    Duration,
    Option<u64>,
)> {
    let started = Instant::now();
    begin_phase_allocation(phase_allocation_probe, PpmlPhase::Preparation);
    let coordinator = PreparedSeparationCoordinator::new(owner);
    // Close this window even when a failed allocation/rebuild rejects the
    // initial generation. Later atomic rebuilds are inside separation phases.
    let initial_preparation_peak =
        end_phase_allocation(phase_allocation_probe, PpmlPhase::Preparation);
    let initial_preparation_elapsed = started.elapsed();
    let coordinator = coordinator?;
    Ok((
        coordinator,
        initial_preparation_elapsed,
        initial_preparation_peak,
    ))
}

#[allow(clippy::too_many_arguments)]
fn coordinate_prepared_ppml_separation<T, E, F>(
    mut coordinator: PreparedSeparationCoordinator<PreparedPpmlModelOwner>,
    ppml: PpmlConfig,
    config: SeparationConfig,
    phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    linear: PreparedOlsFitConfig,
    graph_admitted: bool,
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
    let generation_capacity = separation_generation_capacity(
        candidate_observations,
        coordinator
            .view()
            .prepared
            .outcome()
            .iter()
            .all(|value| *value > 0.0),
        config.drop_singletons,
    );
    let method_count = separation_method_count(config);
    let method_report_capacity = checked_method_report_capacity(
        generation_capacity.min(PREFLIGHT_REPORT_GENERATIONS),
        method_count,
    )?;
    if FAIL_REPORT_BANK_RESERVATION.replace(false) {
        return Err(allocation(
            "injected prepared separation report-bank allocation failure",
        ));
    }
    let mut reports = Vec::new();
    reports
        .try_reserve_exact(method_report_capacity)
        .map_err(|_| allocation("unable to reserve prepared separation method reports"))?;
    let mut generation_hashes = Vec::new();
    generation_hashes
        .try_reserve_exact(generation_capacity)
        .map_err(|_| allocation("unable to reserve prepared separation generation hashes"))?;
    generation_hashes.push(candidate_ledger_hash_original_axis(
        &candidate_original_rows,
        coordinator.active(),
        coordinator.reasons(),
    )?);
    let mut phase_allocation_peaks = PreparedSeparationPhasePeaks::default();

    let (final_fit, final_replay) = loop {
        interrupt(PpmlInterruptPoint::SeparationGeneration)?;
        // A generation records at most one report per method. Beyond the
        // preflighted generations the bank grows here, fallibly.
        reports
            .try_reserve(method_count)
            .map_err(|_| allocation("unable to grow prepared separation method reports"))?;
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
                        compact_to_original_exact_capacity(
                            singleton_fixed_point_all(&topology, view.prepared)?,
                            view.original_rows,
                        )?
                    };
                    if !singleton_rows.is_empty() {
                        let commit = coordinator.transact_validated_proposal(
                            SeparationDropReason::InitialSingleton,
                            singleton_rows,
                            PreparedSeparationCascade::default(),
                        )?;
                        bounded_push(
                            &mut reports,
                            method_report_capacity,
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
                                "prepared singleton pruning reached its fixed point".into(),
                            ),
                            "unable to reserve prepared separation method report",
                        )?;
                        bounded_push(
                            &mut generation_hashes,
                            generation_capacity,
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
                    drop(singleton_rows);
                    let replayed_singletons = {
                        let view = coordinator.view();
                        let topology = DetectorTopology::new(view.prepared.prepared())?;
                        compact_to_original(
                            singleton_fixed_point_all(&topology, view.prepared)?,
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
                    bounded_push(
                        &mut reports,
                        method_report_capacity,
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
                            "prepared singleton pruning independently certified a stable fixed point".into(),
                        ),
                        "unable to reserve prepared singleton no-drop report",
                    )?;
                }

                if config.methods.fixed_effects {
                    let started = Instant::now();
                    let exact = {
                        let view = coordinator.view();
                        exact_fe_proposal_all(view.prepared)?
                    };
                    if !exact.rows.is_empty() {
                        let (primary_rows, cascade) = {
                            let view = coordinator.view();
                            let primary_rows =
                                compact_to_original_exact_capacity(exact.rows, view.original_rows)?;
                            let cascade = complete_cascade(view.prepared, &primary_rows, config)?;
                            (primary_rows, cascade)
                        };
                        let PreparedCascadeProposal {
                            cascade,
                            exact_fe_certificate: exact_followup_certificate,
                        } = cascade;
                        let commit = coordinator.transact_validated_proposal(
                            SeparationDropReason::FixedEffectZeroLevel,
                            primary_rows,
                            cascade,
                        )?;
                        bounded_push(
                            &mut reports,
                            method_report_capacity,
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
                                "prepared exact zero-only FE certificate passed".into(),
                            ),
                            "unable to reserve prepared separation method report",
                        )?;
                        if !commit.exact_fe_rows.is_empty() {
                            bounded_push(
                                &mut reports,
                                method_report_capacity,
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
                        bounded_push(
                            &mut generation_hashes,
                            generation_capacity,
                            candidate_ledger_hash_original_axis(
                                &candidate_original_rows,
                                coordinator.active(),
                                coordinator.reasons(),
                            )?,
                            "unable to reserve prepared separation generation hash",
                        )?;
                        return Ok(true);
                    }
                    bounded_push(
                        &mut reports,
                        method_report_capacity,
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
                            "prepared exact FE scan found no zero-only intercept level".into(),
                        ),
                        "unable to reserve prepared separation method report",
                    )?;
                    drop(exact.rows);
                    let replay = {
                        let view = coordinator.view();
                        exact_fe_proposal_all(view.prepared)?
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
            append_no_boundary_reports(
                &mut reports,
                method_report_capacity,
                coordinator.view().generation,
                config,
            )?;
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
                    bind_numerical_proposal(
                        view.prepared,
                        prepared_simplex_proposal(view.prepared, config, interrupt)?,
                    )?
                };
                let simplex_unresolved = matches!(
                    proposal.status,
                    MethodStatus::SkippedMemory
                        | MethodStatus::SkippedUnsupported
                        | MethodStatus::Uncertified
                )
                .then_some(proposal.status);
                if !proposal.dropped_original_rows.is_empty() {
                    let certified = {
                        let view = coordinator.view();
                        validate_numerical_proposal(view.prepared, proposal, ppml, config, linear)?
                    };
                    commit_numerical(
                        &mut coordinator,
                        &mut reports,
                        &mut generation_hashes,
                        method_report_capacity,
                        generation_capacity,
                        &candidate_original_rows,
                        certified,
                        started,
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
                    graph_admitted,
                    interrupt,
                    None,
                )?;
                final_generation_replay_mask |= replay_bit(SeparationMethod::Simplex);
                bounded_push(
                    &mut reports,
                    method_report_capacity,
                    proposal_report(proposal, replayed_certificate, started, config),
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
                    prepared_bound_iterated_rectifier(
                        view.prepared,
                        ppml,
                        config,
                        linear,
                        graph_admitted,
                        interrupt,
                    )?
                };
                fail_closed_comprehensive(&proposal)?;
                if !proposal.dropped_original_rows.is_empty() {
                    let certified = {
                        let view = coordinator.view();
                        validate_numerical_proposal(view.prepared, proposal, ppml, config, linear)?
                    };
                    commit_numerical(
                        &mut coordinator,
                        &mut reports,
                        &mut generation_hashes,
                        method_report_capacity,
                        generation_capacity,
                        &candidate_original_rows,
                        certified,
                        started,
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
                    graph_admitted,
                    interrupt,
                    None,
                )?;
                let comprehensive_certified = proposal.status == MethodStatus::NoSeparation
                    && replayed_certificate
                        .as_ref()
                        .is_some_and(|certificate| certificate.passed);
                final_generation_replay_mask |= replay_bit(SeparationMethod::IteratedRectifier);
                bounded_push(
                    &mut reports,
                    method_report_capacity,
                    proposal_report(proposal, replayed_certificate, started, config),
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
                exploration.validate_generation(coordinator.view().prepared)?;
                if !exploration.certificate().passed {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        "prepared Mu exploratory trajectory did not certify",
                    ));
                }
                let proposal = {
                    let view = coordinator.view();
                    bind_numerical_proposal(
                        view.prepared,
                        prepared_mu_from_mean(
                            view.prepared,
                            ppml,
                            config,
                            linear,
                            exploration.mean(),
                            exploration.certificate().accepted_iterations,
                        )?,
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
                if !proposal.dropped_original_rows.is_empty() {
                    let certified = {
                        let view = coordinator.view();
                        validate_numerical_proposal(view.prepared, proposal, ppml, config, linear)?
                    };
                    commit_numerical(
                        &mut coordinator,
                        &mut reports,
                        &mut generation_hashes,
                        method_report_capacity,
                        generation_capacity,
                        &candidate_original_rows,
                        certified,
                        started,
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
                    graph_admitted,
                    interrupt,
                    Some(&exploration),
                )?;
                final_generation_replay_mask |= replay_bit(SeparationMethod::Mu);
                bounded_push(
                    &mut reports,
                    method_report_capacity,
                    proposal_report(proposal, replayed_certificate, started, config),
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
        initial_preparation_elapsed: Duration::ZERO,
        initial_preparation_peak: None,
    };
    Ok((outcome, final_fit))
}

#[allow(clippy::too_many_arguments)]
fn commit_numerical(
    coordinator: &mut PreparedSeparationCoordinator<PreparedPpmlModelOwner>,
    reports: &mut Vec<SeparationMethodReport>,
    generation_hashes: &mut Vec<u64>,
    method_report_capacity: usize,
    generation_hash_capacity: usize,
    candidate_original_rows: &[usize],
    validated: ValidatedNumericalProposal,
    started: Instant,
    config: SeparationConfig,
) -> Result<()> {
    let ValidatedNumericalProposal {
        proposal,
        reason,
        certificate,
    } = validated;
    if reason != numerical_drop_reason(proposal.method)? {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "validated numerical proposal reason does not derive from its detector method",
        ));
    }
    {
        let view = coordinator.view();
        validate_prepared_numerical_metadata(view.prepared, &proposal, config)?;
    }
    let PreparedNumericalProposal {
        method,
        dropped_original_rows: primary_rows,
        iterations,
        status,
        exact_fe_certificate,
        direction,
        exact_intercept_span,
        note,
        ..
    } = proposal;
    // The direction has already passed the independent span/sign
    // certificate. It is not needed to construct or publish the atomic
    // deletion transaction, so release its N-vector before cascade/rebuild.
    drop((direction, exact_intercept_span));
    let cascade = {
        let view = coordinator.view();
        complete_cascade(view.prepared, &primary_rows, config)?
    };
    let PreparedCascadeProposal {
        cascade,
        exact_fe_certificate: exact_followup_certificate,
    } = cascade;
    let commit = coordinator.transact_validated_proposal(reason, primary_rows, cascade)?;
    bounded_push(
        reports,
        method_report_capacity,
        method_report(
            commit.generation,
            method,
            status,
            iterations,
            commit.primary_rows,
            commit.singleton_rows,
            Some(certificate),
            exact_fe_certificate,
            started,
            config,
            note,
        ),
        "unable to reserve committed separation method report",
    )?;
    if !commit.exact_fe_rows.is_empty() {
        bounded_push(
            reports,
            method_report_capacity,
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
    bounded_push(
        generation_hashes,
        generation_hash_capacity,
        candidate_ledger_hash_original_axis(
            candidate_original_rows,
            coordinator.active(),
            coordinator.reasons(),
        )?,
        "unable to reserve committed separation generation hash",
    )?;
    Ok(())
}

/// Convert detector-local compact positions to stable original identifiers
/// without changing their order or multiplicity.  Any malformed ordering is
/// rejected later as part of the atomic proposal validation; the adapter must
/// never make an invalid detector result valid by normalization.
fn bind_numerical_proposal(
    generation: &PreparedPpmlModelGeneration,
    proposal: DetectionProposal,
) -> Result<PreparedNumericalProposal> {
    let mut dropped_original_rows = Vec::new();
    dropped_original_rows
        .try_reserve_exact(proposal.dropped.len())
        .map_err(|_| allocation("unable to reserve stable numerical proposal rows"))?;
    for compact_row in proposal.dropped {
        dropped_original_rows.push(*generation.original_rows().get(compact_row).ok_or_else(
            || {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared numerical detector returned an out-of-range compact row",
                )
            },
        )?);
    }
    Ok(PreparedNumericalProposal {
        generation: generation.generation(),
        generation_rows_digest: stable_usize_slice_digest(
            b"numerical-proposal-rows",
            generation.original_rows(),
        )?,
        generation_outcome_digest: stable_f64_slice_digest(
            b"numerical-proposal-outcome",
            generation.outcome(),
        )?,
        method: proposal.method,
        dropped_original_rows,
        iterations: proposal.iterations,
        status: proposal.status,
        certificate: proposal.certificate,
        exact_fe_certificate: proposal.exact_fe_certificate,
        direction: proposal.direction,
        exact_intercept_span: None,
        note: proposal.note,
        fixed_point: None,
    })
}

fn numerical_drop_reason(method: SeparationMethod) -> Result<SeparationDropReason> {
    match method {
        SeparationMethod::Simplex => Ok(SeparationDropReason::Simplex),
        SeparationMethod::IteratedRectifier => Ok(SeparationDropReason::IteratedRectifier),
        SeparationMethod::Mu => Ok(SeparationDropReason::Mu),
        SeparationMethod::Singletons | SeparationMethod::FixedEffects => Err(FeregError::new(
            ErrorCode::Internal,
            "exact separation method cannot supply a numerical drop reason",
        )),
    }
}

struct DetectorTopology<'a> {
    sources: Vec<&'a LevelIndex>,
    effect_to_source: &'a [usize],
    effect_specs: &'a [crate::topology_dof::CategoricalTopologyDofEffectSpec],
}

impl<'a> DetectorTopology<'a> {
    fn new(prepared: &'a PreparedPpmlModel) -> Result<Self> {
        let categorical = prepared.as_ref();
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
        let effect_to_source = topology.effect_to_topology();
        let effect_specs = categorical.effect_specs();
        if effect_to_source.len() != effect_specs.len()
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
            effect_specs,
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
    let active = fallible_copy(
        initially_active,
        "unable to reserve prepared singleton working mask",
    )?;
    singleton_fixed_point_owned(topology, generation, active)
}

/// Initial/replay singleton scans start from the complete immutable prepared
/// generation. Construct that one working mask directly instead of first
/// allocating a caller mask and then copying it inside the fixed-point scan.
fn singleton_fixed_point_all(
    topology: &DetectorTopology<'_>,
    generation: &PreparedPpmlModelGeneration,
) -> Result<Vec<usize>> {
    let active = fallible_filled(
        generation.original_rows().len(),
        true,
        "unable to reserve prepared singleton working mask",
    )?;
    singleton_fixed_point_owned(topology, generation, active)
}

fn singleton_fixed_point_owned(
    topology: &DetectorTopology<'_>,
    generation: &PreparedPpmlModelGeneration,
    active: Vec<bool>,
) -> Result<Vec<usize>> {
    let categorical = generation.prepared().as_ref();
    let finalized = categorical.fe_topology();
    let views = fallible_collect(
        finalized
            .compacted_sources()
            .iter()
            .map(|source| source.view()),
        finalized.compacted_sources().len(),
        "unable to reserve prepared singleton categorical views",
    )?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(topology.sources.len())
        .map_err(|_| allocation("unable to reserve prepared singleton source views"))?;
    for (source, members) in finalized.registry().sources().iter().zip(&topology.sources) {
        bounded_push(
            &mut sources,
            topology.sources.len(),
            (source.codes(&views)?, *members),
            "prepared singleton source-view capacity exhausted",
        )?;
    }
    let frequency = (generation.weight_report().kind == Some(WeightKind::Frequency))
        .then(|| generation.prior_weights());
    singleton::prune(&sources, active, frequency)
}

#[path = "prepared_singleton.rs"]
mod singleton;

struct ExactFeProposal {
    rows: Vec<usize>,
    certificate: ExactFeSeparationCertificate,
}

fn exact_fe_proposal(
    generation: &PreparedPpmlModelGeneration,
    active: &[bool],
) -> Result<ExactFeProposal> {
    if active.len() != generation.original_rows().len() {
        return Err(invalid_argument(
            "prepared exact-FE active mask is not compact-axis aligned",
        ));
    }
    exact_fe_proposal_with_activity(generation, Some(active))
}

/// Initial and replay exact-FE scans inspect the complete immutable prepared
/// generation. Avoid allocating an all-true caller mask merely to read it in
/// the scan; cascade scans still enter through `exact_fe_proposal` with their
/// transaction-local activity mask.
fn exact_fe_proposal_all(generation: &PreparedPpmlModelGeneration) -> Result<ExactFeProposal> {
    exact_fe_proposal_with_activity(generation, None)
}

fn exact_fe_proposal_with_activity(
    generation: &PreparedPpmlModelGeneration,
    active: Option<&[bool]>,
) -> Result<ExactFeProposal> {
    let topology = DetectorTopology::new(generation.prepared())?;
    let nobs = generation.original_rows().len();
    let is_active = |row: usize| active.is_none_or(|mask| mask[row]);
    let mut rows = Vec::new();
    rows.try_reserve_exact(nobs)
        .map_err(|_| allocation("unable to reserve prepared exact-FE drop rows"))?;
    let mut selected = fallible_filled(
        nobs,
        false,
        "unable to reserve prepared exact-FE row selection mask",
    )?;
    let mut levels = Vec::new();
    let level_capacity = topology
        .effect_to_source
        .iter()
        .zip(topology.effect_specs)
        .filter(|(_, effect)| effect.intercept)
        .try_fold(0_usize, |total, (source, _)| {
            total.checked_add(topology.sources[*source].n_levels())
        })
        .ok_or_else(|| allocation("prepared exact-FE level capacity overflowed"))?;
    levels
        .try_reserve_exact(level_capacity)
        .map_err(|_| allocation("unable to reserve prepared exact-FE level certificate"))?;
    let mut checked = 0_usize;
    for (effect, &source_index) in topology.effect_to_source.iter().enumerate() {
        let effect_spec = &topology.effect_specs[effect];
        if !effect_spec.intercept {
            continue;
        }
        checked += 1;
        let source = topology.sources[source_index];
        for level in 0..source.n_levels() {
            let members = source.checked_members(level)?;
            let mut any = false;
            let mut zero_only = true;
            let mut representative_original_row = usize::MAX;
            for &row in members {
                if !is_active(row) {
                    continue;
                }
                any = true;
                representative_original_row =
                    representative_original_row.min(generation.original_rows()[row]);
                if generation.outcome()[row] > 0.0 {
                    zero_only = false;
                    break;
                }
            }
            if any && zero_only {
                fallible_push(
                    &mut levels,
                    ExactFeLevelIdentity {
                        effect: effect_spec.term_id,
                        representative_original_row,
                    },
                    "prepared exact-FE level certificate capacity exhausted",
                )?;
                for &row in members {
                    if is_active(row) && !selected[row] {
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
    if levels.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared exact-FE scan produced a duplicate stable level identity",
        ));
    }
    // Do not let the detector's construction pass certify itself. Reuse the
    // already-owned selection mask for an allocation-free second scan that
    // independently reconstructs the complete zero-only level identity set
    // and its exact row union. This catches a missing/extra identity, a stale
    // compact code, a non-minimal original-row anchor, and any row-union
    // mismatch before `passed` can become authoritative or a transaction can
    // mutate the separation ledger.
    verify_exact_fe_proposal_with_activity(
        generation,
        &topology,
        active,
        &rows,
        &levels,
        checked,
        &mut selected,
    )?;
    let levels = compact_exact_fe_levels(levels)?;
    Ok(ExactFeProposal {
        rows,
        certificate: ExactFeSeparationCertificate {
            checked_intercept_effects: checked,
            zero_only_levels: levels,
            passed: true,
        },
    })
}

/// Independently replay one exact-FE detector result without allocating.
///
/// `selection_scratch` is the construction pass's N-bit workspace, cleared
/// and rebuilt here from immutable prepared topology/outcome state. Stable
/// identities are checked by binary search against the sorted proposal bank;
/// the scratch mask then proves that `rows` is exactly (not merely a subset
/// of) the union of all active members of every replayed zero-only level.
fn verify_exact_fe_proposal_with_activity(
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    active: Option<&[bool]>,
    rows: &[usize],
    identities: &[ExactFeLevelIdentity],
    checked_intercept_effects: usize,
    selection_scratch: &mut [bool],
) -> Result<()> {
    let nobs = generation.original_rows().len();
    let replay_failed = || {
        FeregError::new(
            ErrorCode::NonConvergence,
            "prepared exact-FE certificate failed allocation-free independent replay",
        )
    };
    if generation.outcome().len() != nobs
        || selection_scratch.len() != nobs
        || active.is_some_and(|mask| mask.len() != nobs)
        || rows.windows(2).any(|pair| pair[0] >= pair[1])
        || rows.last().is_some_and(|row| *row >= nobs)
        || identities.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(replay_failed());
    }

    selection_scratch.fill(false);
    let is_active = |row: usize| active.is_none_or(|mask| mask[row]);
    let mut replay_checked = 0_usize;
    let mut replay_level_count = 0_usize;
    for (effect, &source_index) in topology.effect_to_source.iter().enumerate() {
        let effect_spec = &topology.effect_specs[effect];
        if !effect_spec.intercept {
            continue;
        }
        replay_checked = replay_checked.checked_add(1).ok_or_else(replay_failed)?;
        let source = topology.sources[source_index];
        for level in 0..source.n_levels() {
            let members = source.checked_members(level)?;
            let mut any = false;
            let mut zero_only = true;
            let mut representative_original_row = usize::MAX;
            for &row in members {
                if !is_active(row) {
                    continue;
                }
                any = true;
                representative_original_row =
                    representative_original_row.min(generation.original_rows()[row]);
                if generation.outcome()[row] > 0.0 {
                    zero_only = false;
                }
            }
            if !any || !zero_only {
                continue;
            }

            replay_level_count = replay_level_count
                .checked_add(1)
                .ok_or_else(replay_failed)?;
            let identity = ExactFeLevelIdentity {
                effect: effect_spec.term_id,
                representative_original_row,
            };
            if identities.binary_search(&identity).is_err() {
                return Err(replay_failed());
            }
            for &row in members {
                if is_active(row) {
                    selection_scratch[row] = true;
                }
            }
        }
    }
    if replay_checked != checked_intercept_effects || replay_level_count != identities.len() {
        return Err(replay_failed());
    }

    let mut proposal_position = 0_usize;
    for (row, selected) in selection_scratch.iter().copied().enumerate() {
        if !selected {
            continue;
        }
        if rows.get(proposal_position).copied() != Some(row) {
            return Err(replay_failed());
        }
        proposal_position += 1;
    }
    if proposal_position != rows.len() {
        return Err(replay_failed());
    }
    Ok(())
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
    if !config.drop_singletons && !config.methods.fixed_effects {
        return Ok(PreparedCascadeProposal {
            cascade: PreparedSeparationCascade::default(),
            exact_fe_certificate: None,
        });
    }
    const CASCADE_SINGLETON: u8 = 1;
    const CASCADE_EXACT_FE: u8 = 2;
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
    // One byte per row records the eventual cascade reason.  Keeping two
    // separately N-reserved row vectors would retain 2*N*usize throughout
    // every inner scan even though singleton and exact rows are disjoint.
    // Exact-capacity stable-row vectors are materialized only after the fixed
    // point, when all inner scan workspaces have been released.
    let mut cascade_reason = fallible_filled(
        active.len(),
        0_u8,
        "unable to reserve prepared cascade reason tags",
    )?;
    let mut exact_checked_effects = 0_usize;
    let mut exact_zero_only_levels = Vec::new();
    let exact_level_capacity = if config.methods.fixed_effects {
        topology
            .effect_to_source
            .iter()
            .zip(topology.effect_specs)
            .filter(|(_, effect)| effect.intercept)
            .try_fold(0_usize, |total, (source, _)| {
                total.checked_add(topology.sources[*source].n_levels())
            })
            .ok_or_else(|| allocation("prepared cascade exact-FE level capacity overflowed"))?
    } else {
        0
    };
    if exact_level_capacity > 0 {
        exact_zero_only_levels
            .try_reserve_exact(exact_level_capacity)
            .map_err(|_| allocation("unable to reserve prepared cascade exact-FE levels"))?;
    }
    loop {
        let mut changed = false;
        if config.drop_singletons {
            let rows = singleton_fixed_point(&topology, generation, &active)?;
            for row in rows {
                if active[row] {
                    active[row] = false;
                    cascade_reason[row] = CASCADE_SINGLETON;
                    changed = true;
                }
            }
        }
        if config.methods.fixed_effects {
            let proposal = exact_fe_proposal(generation, &active)?;
            exact_checked_effects =
                exact_checked_effects.max(proposal.certificate.checked_intercept_effects);
            let extended_len = exact_zero_only_levels
                .len()
                .checked_add(proposal.certificate.zero_only_levels.len())
                .ok_or_else(|| allocation("prepared cascade exact-FE level count overflowed"))?;
            if extended_len > exact_level_capacity {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "prepared cascade exact-FE identities exceed the initial topology bound",
                ));
            }
            exact_zero_only_levels.extend(proposal.certificate.zero_only_levels);
            for row in proposal.rows {
                if active[row] {
                    active[row] = false;
                    cascade_reason[row] = CASCADE_EXACT_FE;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    exact_zero_only_levels.sort_unstable();
    if exact_zero_only_levels
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared exact-FE cascade reused a stable level identity",
        ));
    }
    let exact_zero_only_levels = compact_exact_fe_levels(exact_zero_only_levels)?;
    let singleton_count = cascade_reason
        .iter()
        .filter(|reason| **reason == CASCADE_SINGLETON)
        .count();
    let exact_fe_count = cascade_reason
        .iter()
        .filter(|reason| **reason == CASCADE_EXACT_FE)
        .count();
    let singleton_rows = fallible_collect(
        cascade_reason
            .iter()
            .enumerate()
            .filter_map(|(row, reason)| {
                (*reason == CASCADE_SINGLETON).then_some(generation.original_rows()[row])
            }),
        singleton_count,
        "unable to reserve exact-capacity prepared cascade singleton rows",
    )?;
    let exact_fe_rows = fallible_collect(
        cascade_reason
            .iter()
            .enumerate()
            .filter_map(|(row, reason)| {
                (*reason == CASCADE_EXACT_FE).then_some(generation.original_rows()[row])
            }),
        exact_fe_count,
        "unable to reserve exact-capacity prepared cascade exact-FE rows",
    )?;
    Ok(PreparedCascadeProposal {
        exact_fe_certificate: (!exact_fe_rows.is_empty()).then_some(ExactFeSeparationCertificate {
            checked_intercept_effects: exact_checked_effects,
            zero_only_levels: exact_zero_only_levels,
            passed: true,
        }),
        cascade: PreparedSeparationCascade {
            singleton_rows,
            exact_fe_rows,
        },
    })
}

fn prepared_simplex_proposal(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<DetectionProposal> {
    let categorical = generation.prepared().as_ref();
    // Resolve the enum-backed model exactly once before entering the dense
    // row/column construction loop. The concrete view keeps both this loop
    // and the shared linear-session loops statically dispatched.
    let fit_view = generation.fit_view();
    let finite_columns = fit_view.model().columns.len();
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
        .and_then(|value| value.checked_add(topology.effect_specs.len()))
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
            SeparationNote::with_detail(
                "prepared full-model simplex exceeded its configured cell limit",
                peak_cells,
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
            values[row * columns + column] = fit_view.column_value(column, row)?;
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
        // The design is zero-filled, so only each row's own level is written;
        // every other level's indicator and slope cells stay exactly zero.
        for (row, &code) in codes.iter().enumerate() {
            let base = row * columns + column_offset + code as usize * channels;
            let mut channel = 0_usize;
            if spec.intercept {
                values[base] = 1.0;
                channel = 1;
            }
            for slope in &spec.slopes {
                values[base + channel] = categorical.numeric_source_values(slope.source)?[row];
                channel += 1;
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

/// A coefficient-backed submodel proposal is independently reconstructed at
/// acceptance. No graph result can certify absence of full-model separation.
fn prepared_bound_iterated_rectifier(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    graph_admitted: bool,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedNumericalProposal> {
    if graph_admitted && generation.outcome().contains(&0.0) {
        if let Some(budget) = linear.absorb.solver_memory_budget {
            if let Some(design) = generation.fit_view().fit_borrowed_intercept_map_design()? {
                if graph::workspace_bytes(&design)?.is_some_and(|bytes| budget.fits(bytes)) {
                    if let Some(candidate) =
                        graph::direction(&design, generation.outcome(), &mut || {
                            interrupt(PpmlInterruptPoint::SeparationIrIteration)
                        })?
                    {
                        let scale = infinity_norm(&candidate.direction);
                        let dropped =
                            fallible_collect(
                                candidate.direction.iter().enumerate().filter_map(
                                    |(row, &value)| {
                                        (generation.outcome()[row] == 0.0
                                            && value < -config.tolerance * scale)
                                            .then_some(row)
                                    },
                                ),
                                candidate.direction.len(),
                                "unable to reserve graph separation proposal",
                            )?;
                        let certificate = detector_direction_certificate(
                            generation.outcome(),
                            &candidate.direction,
                            &dropped,
                            config.certificate_tolerance,
                        )?;
                        let mut proposal = bind_numerical_proposal(
                            generation,
                            DetectionProposal {
                                method: SeparationMethod::IteratedRectifier,
                                dropped,
                                iterations: 0,
                                status: MethodStatus::Certified,
                                certificate: Some(certificate),
                                exact_fe_certificate: None,
                                direction: Some(candidate.direction),
                                note: "prepared iterated-rectifier candidate direction".into(),
                            },
                        )?;
                        proposal.exact_intercept_span = Some(candidate.span);
                        return Ok(proposal);
                    }
                }
            }
        }
    }
    let (proposal, fixed_point) =
        prepared_iterated_rectifier(generation, ppml, config, linear, interrupt)?;
    let mut bound = bind_numerical_proposal(generation, proposal)?;
    bound.fixed_point = fixed_point;
    Ok(bound)
}

fn prepared_iterated_rectifier(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<(DetectionProposal, Option<IrFixedPointEvidence>)> {
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
        return Ok((
            DetectionProposal::empty(
                SeparationMethod::IteratedRectifier,
                MethodStatus::NoCandidates,
                "no zero-outcome observations",
            ),
            None,
        ));
    }
    let boundary_weight = ir_boundary_weight(config, boundary.len());
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
        let fitted = &projection.fitted;
        let assessment = assess_ir_projection(outcome, |row| fitted[row], config);
        if let Some(certificate) = assessment.certificate {
            let note = if assessment.scale <= f64::MIN_POSITIVE {
                "prepared iterated rectifier converged to zero"
            } else {
                "prepared iterated rectifier certified no separation"
            };
            let PreparedProjection {
                residual,
                kept_columns,
                ..
            } = projection;
            return Ok((
                DetectionProposal {
                    method: SeparationMethod::IteratedRectifier,
                    dropped: Vec::new(),
                    iterations: iteration,
                    status: MethodStatus::NoSeparation,
                    certificate: Some(certificate),
                    exact_fe_certificate: None,
                    direction: None,
                    note: note.into(),
                },
                Some(IrFixedPointEvidence {
                    recurrence: u,
                    residual,
                    kept_columns,
                }),
            ));
        }
        let positive_direction = projection.fitted;
        let IrProjectionAssessment {
            scale,
            threshold,
            minimum_boundary,
            maximum_boundary,
            ..
        } = assessment;
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
                let certificate = detector_direction_certificate(
                    outcome,
                    &direction,
                    &dropped,
                    config.certificate_tolerance,
                )?;
                return Ok((
                    DetectionProposal {
                        method: SeparationMethod::IteratedRectifier,
                        dropped,
                        iterations: iteration,
                        status: MethodStatus::Certified,
                        certificate: Some(certificate),
                        exact_fe_certificate: None,
                        direction: Some(direction),
                        note: "prepared iterated-rectifier candidate direction".into(),
                    },
                    None,
                ));
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
    Ok((
        DetectionProposal {
            method: SeparationMethod::IteratedRectifier,
            dropped: Vec::new(),
            iterations: config.max_ir_iterations,
            status: MethodStatus::MaximumIterations,
            certificate: None,
            exact_fe_certificate: None,
            direction: None,
            note: SeparationNote::with_detail(
                "prepared iterated rectifier exhausted its iteration budget",
                config.max_ir_iterations,
            ),
        },
        None,
    ))
}

fn nonnegative_ir_boundary_violation(minimum_boundary: f64) -> f64 {
    (-minimum_boundary).max(0.0)
}

/// Weight of every zero-outcome row in the rectifier projection.
fn ir_boundary_weight(config: SeparationConfig, boundary_rows: usize) -> f64 {
    (config.tolerance * config.tolerance / boundary_rows as f64).max(f64::MIN_POSITIVE)
}

/// Sign summary of one rectifier projection, with its no-separation
/// certificate when the projection is terminal: either it vanished, or it is
/// within tolerance on zero-outcome rows and near zero on positive rows.
struct IrProjectionAssessment {
    scale: f64,
    threshold: f64,
    minimum_boundary: f64,
    maximum_boundary: f64,
    certificate: Option<SeparationCertificate>,
}

fn assess_ir_projection(
    outcome: &[f64],
    fitted: impl Fn(usize) -> f64,
    config: SeparationConfig,
) -> IrProjectionAssessment {
    let mut scale = 0.0_f64;
    let mut interior = 0.0_f64;
    let mut minimum_boundary = f64::INFINITY;
    let mut maximum_boundary = f64::NEG_INFINITY;
    for (row, value) in outcome.iter().enumerate() {
        let fitted = fitted(row);
        scale = scale.max(fitted.abs());
        if *value > 0.0 {
            interior = interior.max(fitted.abs());
        } else if *value == 0.0 {
            minimum_boundary = minimum_boundary.min(fitted);
            maximum_boundary = maximum_boundary.max(fitted);
        }
    }
    let recurrence_scale = scale.max(1.0);
    let threshold = config.tolerance * recurrence_scale;
    let boundary_violation = nonnegative_ir_boundary_violation(minimum_boundary);
    let proof = crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint;
    let certificate = if scale <= f64::MIN_POSITIVE {
        Some(SeparationCertificate {
            proof,
            interior_zero_relative: 0.0,
            boundary_violation_relative: 0.0,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale: 0.0,
            positive_maximum: 0.0,
            passed: true,
        })
    } else if minimum_boundary >= -threshold
        && maximum_boundary <= threshold
        && interior <= config.certificate_tolerance * recurrence_scale
        && boundary_violation <= config.certificate_tolerance * recurrence_scale
    {
        Some(SeparationCertificate {
            proof,
            interior_zero_relative: interior / recurrence_scale,
            boundary_violation_relative: boundary_violation / recurrence_scale,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale: scale,
            positive_maximum: boundary_violation,
            passed: true,
        })
    } else {
        None
    };
    IrProjectionAssessment {
        scale,
        threshold,
        minimum_boundary,
        maximum_boundary,
        certificate,
    }
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
    if dropped.is_empty() {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Mu,
            MethodStatus::Uncertified,
            "prepared mu direction has no certified strict-negative boundary row",
        ));
    }
    let certificate = detector_direction_certificate(
        generation.outcome(),
        &direction,
        &dropped,
        config.certificate_tolerance,
    )?;
    Ok(DetectionProposal {
        method: SeparationMethod::Mu,
        dropped,
        iterations: trajectory_iterations,
        status: MethodStatus::Certified,
        certificate: Some(certificate),
        exact_fe_certificate: None,
        direction: Some(direction),
        note: "prepared mu candidate direction".into(),
    })
}

struct PreparedProjection {
    fitted: Vec<f64>,
    residual: Vec<f64>,
    kept_columns: Vec<usize>,
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
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=separation_projection|generation={}|rows={}|solver={:?}|minimum_weight={:.17e}",
            generation.generation(), response.len(), fit_config.absorb.solver,
            weights.iter().copied().fold(f64::INFINITY, f64::min)));
    }
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
    let kept_columns = fallible_copy(
        fit.kept_generated_columns(),
        "unable to reserve prepared separation projection basis",
    )?;
    Ok(PreparedProjection {
        fitted,
        residual,
        kept_columns,
    })
}

/// Certify a stored terminal rectifier projection without re-solving it.
///
/// The no-separation conclusion rests on two facts about the stored vectors:
/// the fitted values `recurrence - residual` satisfy the rectifier's sign
/// conditions, and the residual is the weighted least-squares residual of the
/// recurrence on the regressor and FE span. The second fact is checked from
/// the data: every kept regressor and every FE intercept or slope channel has
/// a weighted residual moment that is zero relative to the channel norm and
/// the larger of the recurrence and residual norms, within the certificate
/// tolerance the rectifier decides with. Its denominators (the raw channel
/// norm and the larger recurrence or residual norm) are at least as large as
/// those of the projection's own FE and score certificates, so a projection
/// that passed them passes this check; it is an independent recomputation,
/// not a stricter test. An all-zero recurrence with an all-zero residual is the
/// trivially certified vanishing projection. Returns `None` when any check
/// fails; the caller then re-executes the detector.
///
/// Like those certificates, the check works in the rectifier's weighted
/// metric. Zero-outcome rows carry weight `tolerance^2 / zeros`, so their
/// fitted values are only weakly constrained by any weighted moment. The check
/// certifies that the stored vectors are a genuine certified projection
/// (catching stale, corrupted or uncertified state); it does not improve the
/// method's resolution on zero-outcome rows, which the former deterministic
/// re-run did not improve either.
fn certify_ir_fixed_point_evidence(
    generation: &PreparedPpmlModelGeneration,
    evidence: &IrFixedPointEvidence,
    config: SeparationConfig,
) -> Result<Option<SeparationCertificate>> {
    let outcome = generation.outcome();
    let rows = outcome.len();
    let IrFixedPointEvidence {
        recurrence,
        residual,
        kept_columns,
    } = evidence;
    if recurrence.len() != rows || residual.len() != rows {
        return Ok(None);
    }
    // The recurrence is finite, nonnegative and supported on zero outcomes;
    // it is either scaled to a unit maximum or, after a step that removed
    // every boundary row, identically zero.
    let mut maximum = 0.0_f64;
    let mut boundary_rows = 0_usize;
    for ((value, entry), residual) in outcome.iter().zip(recurrence).zip(residual) {
        if !entry.is_finite()
            || !residual.is_finite()
            || *entry < 0.0
            || (*value > 0.0 && *entry != 0.0)
        {
            return Ok(None);
        }
        boundary_rows += usize::from(*value == 0.0);
        maximum = maximum.max(*entry);
    }
    let columns = generation.fit_view().model().columns.len();
    if boundary_rows == 0
        || kept_columns.iter().any(|column| *column >= columns)
        || kept_columns.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Ok(None);
    }
    if maximum == 0.0 {
        // Projecting the zero vector certifies trivially; anything but a zero
        // residual is not that projection.
        return Ok(residual
            .iter()
            .all(|value| *value == 0.0)
            .then(|| assess_ir_projection(outcome, |_| 0.0, config).certificate)
            .flatten());
    }
    if maximum != 1.0 {
        return Ok(None);
    }
    let Some(certificate) =
        assess_ir_projection(outcome, |row| recurrence[row] - residual[row], config).certificate
    else {
        return Ok(None);
    };
    let boundary_weight = ir_boundary_weight(config, boundary_rows);
    let weight = |row: usize| {
        if outcome[row] > 0.0 {
            1.0
        } else {
            boundary_weight
        }
    };
    let response_scale = ir_weighted_norm(&weight, &|row| recurrence[row], 0..rows)
        .max(ir_weighted_norm(&weight, &|row| residual[row], 0..rows));
    if !response_scale.is_finite() || response_scale <= 0.0 {
        return Ok(None);
    }
    let largest = ir_regressor_moment_ratio(generation, &weight, residual, kept_columns)?
        .max(ir_fe_moment_ratio(generation, &weight, residual)?)
        / response_scale;
    let passed = largest.is_finite() && largest <= config.certificate_tolerance;
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=ir_fixed_point_check|largest_relative_moment={largest:.17e}|tolerance={:.17e}|passed={}",
            config.certificate_tolerance,
            u8::from(passed),
        ));
    }
    Ok(passed.then_some(certificate))
}

/// Overflow-safe `sqrt(sum w * value^2)` over `members`.
fn ir_weighted_norm(
    weight: &dyn Fn(usize) -> f64,
    values: &dyn Fn(usize) -> f64,
    members: impl Iterator<Item = usize>,
) -> f64 {
    let mut scale = 0.0_f64;
    let mut sum = 1.0_f64;
    for row in members {
        let absolute = (weight(row).sqrt() * values(row)).abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            let ratio = scale / absolute;
            sum = 1.0 + sum * ratio * ratio;
            scale = absolute;
        } else {
            let ratio = absolute / scale;
            sum += ratio * ratio;
        }
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * sum.sqrt()
    }
}

/// Largest `|moment| / channel norm`; a nonzero moment on a zero or
/// nonfinite norm is infinite.
fn ir_moment_ratio(largest: f64, moment: f64, norm: f64) -> f64 {
    if norm > 0.0 && norm.is_finite() {
        largest.max(moment.abs() / norm)
    } else if moment != 0.0 || !norm.is_finite() {
        f64::INFINITY
    } else {
        largest
    }
}

/// Weighted residual moments of the kept regressors, each over its norm.
fn ir_regressor_moment_ratio(
    generation: &PreparedPpmlModelGeneration,
    weight: &dyn Fn(usize) -> f64,
    residual: &[f64],
    kept_columns: &[usize],
) -> Result<f64> {
    let fit_view = generation.fit_view();
    let mut largest = 0.0_f64;
    for &column in kept_columns {
        let (mut sum, mut correction) = (0.0_f64, 0.0_f64);
        let (mut scale, mut squares) = (0.0_f64, 1.0_f64);
        crate::prepared_fit_view::visit_prepared_column_range(
            fit_view,
            column,
            0..residual.len(),
            |row, value| {
                let w = weight(row);
                crate::linalg::neumaier_add(w * value * residual[row], &mut sum, &mut correction);
                let absolute = (w.sqrt() * value).abs();
                if absolute != 0.0 {
                    if scale < absolute {
                        let ratio = scale / absolute;
                        squares = 1.0 + squares * ratio * ratio;
                        scale = absolute;
                    } else {
                        let ratio = absolute / scale;
                        squares += ratio * ratio;
                    }
                }
                Ok(())
            },
        )?;
        let norm = if scale == 0.0 {
            0.0
        } else {
            scale * squares.sqrt()
        };
        largest = ir_moment_ratio(largest, sum + correction, norm);
    }
    Ok(largest)
}

/// Weighted residual moments of every FE intercept and slope channel, each
/// over its level norm.
fn ir_fe_moment_ratio(
    generation: &PreparedPpmlModelGeneration,
    weight: &dyn Fn(usize) -> f64,
    residual: &[f64],
) -> Result<f64> {
    let categorical = generation.prepared().as_ref();
    let topology = DetectorTopology::new(generation.prepared())?;
    let mut largest = 0.0_f64;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        let source = topology.sources[topology.effect_to_source[effect]];
        for level in 0..source.n_levels() {
            let members = source.checked_members(level)?;
            if spec.intercept {
                let (mut sum, mut correction) = (0.0_f64, 0.0_f64);
                let (mut mass, mut mass_correction) = (0.0_f64, 0.0_f64);
                for &row in members {
                    crate::linalg::neumaier_add(
                        weight(row) * residual[row],
                        &mut sum,
                        &mut correction,
                    );
                    crate::linalg::neumaier_add(weight(row), &mut mass, &mut mass_correction);
                }
                largest =
                    ir_moment_ratio(largest, sum + correction, (mass + mass_correction).sqrt());
            }
            for slope in &spec.slopes {
                let values = categorical.numeric_source_values(slope.source)?;
                let (mut sum, mut correction) = (0.0_f64, 0.0_f64);
                for &row in members {
                    crate::linalg::neumaier_add(
                        weight(row) * values[row] * residual[row],
                        &mut sum,
                        &mut correction,
                    );
                }
                let norm = ir_weighted_norm(weight, &|row| values[row], members.iter().copied());
                largest = ir_moment_ratio(largest, sum + correction, norm);
            }
        }
    }
    Ok(largest)
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
    observed: &PreparedNumericalProposal,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    graph_admitted: bool,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    mu_exploration: Option<&PreparedPpmlMuExploration>,
) -> Result<Option<SeparationCertificate>> {
    validate_prepared_numerical_metadata(generation, observed, config)?;
    if !observed.dropped_original_rows.is_empty() || observed.direction.is_some() {
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
    // A terminal rectifier projection is certified from its stored evidence
    // in one pass over the data. Only if that check fails, or if it does not
    // reproduce the reported certificate, does the detector run again.
    if observed.method == SeparationMethod::IteratedRectifier
        && observed.status == MethodStatus::NoSeparation
        && observed.exact_fe_certificate.is_none()
        && (1..=config.max_ir_iterations).contains(&observed.iterations)
    {
        if let Some(evidence) = observed.fixed_point.as_ref() {
            interrupt(PpmlInterruptPoint::SeparationIrIteration)?;
            if let Some(certificate) =
                certify_ir_fixed_point_evidence(generation, evidence, config)?
            {
                if observed.certificate.as_ref() == Some(&certificate) {
                    return Ok(Some(certificate));
                }
            }
        }
    }
    let replayed = if observed.method == SeparationMethod::IteratedRectifier {
        prepared_bound_iterated_rectifier(
            generation,
            ppml,
            config,
            linear,
            graph_admitted,
            interrupt,
        )?
    } else {
        bind_numerical_proposal(
            generation,
            match observed.method {
                SeparationMethod::Simplex => {
                    prepared_simplex_proposal(generation, config, interrupt)?
                }
                SeparationMethod::IteratedRectifier => {
                    prepared_iterated_rectifier(generation, ppml, config, linear, interrupt)?.0
                }
                SeparationMethod::Mu => {
                    let exploration = mu_exploration.ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::Internal,
                            "prepared Mu certification replay omitted its fitted generation state",
                        )
                    })?;
                    exploration.validate_generation(generation)?;
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
            },
        )?
    };
    if replayed.method != observed.method
        || replayed.status != observed.status
        || replayed.iterations != observed.iterations
        || replayed.generation != observed.generation
        || !replayed.dropped_original_rows.is_empty()
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

fn validate_prepared_numerical_metadata(
    generation: &PreparedPpmlModelGeneration,
    proposal: &PreparedNumericalProposal,
    config: SeparationConfig,
) -> Result<()> {
    if proposal.generation != generation.generation()
        || proposal.generation_rows_digest
            != stable_usize_slice_digest(b"numerical-proposal-rows", generation.original_rows())?
        || proposal.generation_outcome_digest
            != stable_f64_slice_digest(b"numerical-proposal-outcome", generation.outcome())?
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared numerical proposal is stale for the active generation",
        ));
    }
    let enabled = match proposal.method {
        SeparationMethod::Simplex => config.methods.simplex,
        SeparationMethod::IteratedRectifier => config.methods.iterated_rectifier,
        SeparationMethod::Mu => config.methods.mu,
        SeparationMethod::Singletons | SeparationMethod::FixedEffects => {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "exact separation method entered the numerical proposal boundary",
            ));
        }
    };
    if !enabled {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared numerical proposal names a detector that was not requested",
        ));
    }
    if proposal.note.is_empty() || proposal.exact_fe_certificate.is_some() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared numerical proposal metadata is malformed",
        ));
    }
    if proposal
        .dropped_original_rows
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared numerical proposal original rows are not sorted and unique",
        ));
    }
    if proposal
        .dropped_original_rows
        .iter()
        .any(|row| generation.original_rows().binary_search(row).is_err())
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared numerical proposal contains an inactive or out-of-range original row",
        ));
    }

    let has_drops = !proposal.dropped_original_rows.is_empty();
    if proposal.exact_intercept_span.is_some()
        && (!has_drops || proposal.method != SeparationMethod::IteratedRectifier)
    {
        return Err(invalid_argument(
            "exact separation proof requires an IR deletion proposal",
        ));
    }
    if has_drops {
        if proposal.status != MethodStatus::Certified || proposal.direction.is_none() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "prepared numerical deletion proposal has an inconsistent method status",
            ));
        }
        let claimed = proposal.certificate.as_ref().ok_or_else(|| {
            FeregError::new(
                ErrorCode::NonConvergence,
                "prepared numerical deletion proposal omitted its detector certificate",
            )
        })?;
        if claimed.proof != crate::ppml_separation::SeparationCertificateProof::RecessionDirection
            || !claimed.passed
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "prepared numerical deletion proposal carries an inconsistent certificate",
            ));
        }
    } else {
        if proposal.direction.is_some() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared numerical no-drop proposal carries a deletion direction",
            ));
        }
        let shape_valid = match proposal.status {
            MethodStatus::NoCandidates => {
                proposal.iterations == 0 && proposal.certificate.is_none()
            }
            MethodStatus::NoSeparation => proposal
                .certificate
                .as_ref()
                .is_some_and(|certificate| {
                    certificate.passed
                        && matches!(
                            (proposal.method, certificate.proof),
                            (
                                SeparationMethod::Simplex,
                                crate::ppml_separation::SeparationCertificateProof::SimplexFullPositiveRank { .. }
                                    | crate::ppml_separation::SeparationCertificateProof::SimplexDualInfeasibility { .. },
                            ) | (
                                SeparationMethod::IteratedRectifier,
                                crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
                            )
                        )
                }),
            MethodStatus::SkippedMemory | MethodStatus::SkippedUnsupported => {
                proposal.method == SeparationMethod::Simplex && proposal.certificate.is_none()
            }
            MethodStatus::Uncertified => {
                proposal.method == SeparationMethod::Simplex && proposal.certificate.is_none()
            }
            MethodStatus::MaximumIterations => {
                proposal.method == SeparationMethod::IteratedRectifier
                    && proposal.certificate.is_none()
            }
            MethodStatus::Certified => false,
        };
        if !shape_valid {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "prepared numerical no-drop proposal has an inconsistent method status or certificate",
            ));
        }
    }
    Ok(())
}

fn detector_direction_certificate(
    outcome: &[f64],
    direction: &[f64],
    dropped_compact_rows: &[usize],
    tolerance: f64,
) -> Result<SeparationCertificate> {
    if outcome.len() != direction.len()
        || direction.iter().any(|value| !value.is_finite())
        || dropped_compact_rows
            .iter()
            .any(|row| *row >= outcome.len() || outcome[*row] != 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared detector certificate inputs are not finite and aligned",
        ));
    }
    let scale = infinity_norm(direction);
    if !scale.is_finite() || scale <= f64::MIN_POSITIVE {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared detector certificate direction is zero",
        ));
    }
    let interior_zero_relative = outcome
        .iter()
        .zip(direction)
        .filter(|(outcome, _)| **outcome > 0.0)
        .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()))
        / scale;
    let positive_maximum = outcome
        .iter()
        .zip(direction)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(f64::NEG_INFINITY, f64::max)
        .max(0.0);
    let negative_margin_relative = dropped_compact_rows
        .iter()
        .map(|row| -direction[*row] / scale)
        .fold(f64::INFINITY, f64::min);
    let certificate = SeparationCertificate {
        proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
        interior_zero_relative,
        boundary_violation_relative: positive_maximum / scale,
        span_residual_relative: 0.0,
        negative_margin_relative,
        direction_scale: scale,
        positive_maximum,
        passed: interior_zero_relative <= tolerance
            && positive_maximum <= tolerance * scale
            && negative_margin_relative >= tolerance,
    };
    if !certificate.passed {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared detector's proposed direction failed its own sign certificate",
        ));
    }
    Ok(certificate)
}

fn validate_numerical_proposal(
    generation: &PreparedPpmlModelGeneration,
    proposal: PreparedNumericalProposal,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
) -> Result<ValidatedNumericalProposal> {
    validate_prepared_numerical_metadata(generation, &proposal, config)?;
    if proposal.dropped_original_rows.is_empty() {
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
    let compact_proposed_rows = fallible_collect(
        proposal.dropped_original_rows.iter().map(|row| {
            generation
                .original_rows()
                .binary_search(row)
                .expect("metadata validation proved stable-row membership")
        }),
        proposal.dropped_original_rows.len(),
        "unable to reserve numerical proposal certificate row map",
    )?;
    let detector_claim = detector_direction_certificate(
        generation.outcome(),
        direction,
        &compact_proposed_rows,
        config.certificate_tolerance,
    )?;
    if proposal.certificate.as_ref() != Some(&detector_claim) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared numerical proposal certificate is inconsistent with its direction and stable rows",
        ));
    }
    let span_residual_relative = if let Some(proof) = &proposal.exact_intercept_span {
        let design = generation
            .fit_view()
            .fit_borrowed_intercept_map_design()?
            .ok_or_else(|| {
                invalid_argument("exact separation proof requires categorical intercepts")
            })?;
        proof.verify(&design, direction)?;
        0.0
    } else {
        let unit = fallible_filled(
            direction.len(),
            1.0,
            "unable to reserve prepared separation certificate weights",
        )?;
        let projection = prepared_project(generation, direction, &unit, ppml, config, linear)?;
        euclidean_norm(&projection.residual) / euclidean_norm(direction)
    };
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
                (*outcome == 0.0 && *value < -threshold).then_some(generation.original_rows()[row])
            }),
        direction.len(),
        "unable to reserve prepared separation certified rows",
    )?;
    if certified_rows != proposal.dropped_original_rows {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared numerical proposal does not equal its strict-negative certificate set",
        ));
    }
    let negative_margin_relative = certified_rows
        .iter()
        .map(|row| {
            let compact = generation
                .original_rows()
                .binary_search(row)
                .expect("validated stable row belongs to the generation");
            -direction[compact] / scale
        })
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
    Ok(ValidatedNumericalProposal {
        reason: numerical_drop_reason(proposal.method)?,
        proposal,
        certificate,
    })
}

fn proposal_report(
    proposal: PreparedNumericalProposal,
    certificate: Option<SeparationCertificate>,
    started: Instant,
    config: SeparationConfig,
) -> SeparationMethodReport {
    method_report(
        proposal.generation,
        proposal.method,
        proposal.status,
        proposal.iterations,
        Vec::new(),
        Vec::new(),
        certificate,
        proposal.exact_fe_certificate,
        started,
        config,
        proposal.note,
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
    note: SeparationNote,
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
        note,
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
        note: "prepared exact-FE follow-up in the same atomic generation".into(),
    }
}

fn append_no_boundary_reports(
    reports: &mut Vec<SeparationMethodReport>,
    method_report_capacity: usize,
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
            bounded_push(
                reports,
                method_report_capacity,
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
                    note: "no zero-outcome observations".into(),
                },
                "unable to reserve prepared no-boundary method report",
            )?;
        }
    }
    Ok(())
}

fn fail_closed_comprehensive(proposal: &PreparedNumericalProposal) -> Result<()> {
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
                proposal.note.code
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

/// Convert a detector's deliberately over-reserved row workspace into a
/// capacity-exact transaction/report vector.  Accepted reports persist across
/// later separation generations, so moving an N-capacity cascade vector with
/// only a handful of rows would make multi-round memory grow as generations*N
/// rather than with the total number of deleted identities.
fn compact_to_original_exact_capacity(
    compact: Vec<usize>,
    original_rows: &[usize],
) -> Result<Vec<usize>> {
    let compact = compact_to_original(compact, original_rows)?;
    fallible_copy(
        &compact,
        "unable to reserve exact-capacity prepared cascade rows",
    )
}

/// Persist accepted exact-FE level identities at their logical size. Detector
/// scans intentionally reserve the complete intercept-level ceiling, but an
/// accepted certificate may outlive multiple later generations. Moving that
/// scan buffer into a report would make retained memory grow by generations
/// times the full level count rather than by unique accepted identities.
fn compact_exact_fe_levels(levels: Vec<ExactFeLevelIdentity>) -> Result<Vec<ExactFeLevelIdentity>> {
    if FAIL_EXACT_FE_LEVEL_COMPACTION.replace(false) {
        return Err(allocation(
            "injected exact-FE level certificate compaction failure",
        ));
    }
    fallible_copy(
        &levels,
        "unable to reserve exact-capacity exact-FE level certificate",
    )
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
        neumaier_add(value, &mut sum, &mut correction);
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
    for byte in b"fereg.ppml.certified-separation.v2" {
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
                for identity in &certificate.zero_only_levels {
                    digest.u64(u64::from(identity.effect.get()));
                    digest.usize(identity.representative_original_row)?;
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
        crate::ppml_separation::SeparationCertificateProof::SimplexDualInfeasibility {
            boundary_rows,
            nullity,
        } => {
            digest.byte(4);
            digest.usize(boundary_rows)?;
            digest.usize(nullity)?;
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

fn checked_method_report_capacity(
    candidate_observations: usize,
    method_count: usize,
) -> Result<usize> {
    candidate_observations
        .checked_mul(method_count)
        .ok_or_else(|| allocation("prepared separation method-report capacity overflowed"))
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

/// Push into a vector whose complete publication capacity was admitted and
/// reserved before detector execution. Crossing the logical ceiling is an
/// execution-chain defect; silently using allocator spare capacity or growing
/// here would invalidate the preflight receipt.
fn bounded_push<T>(
    output: &mut Vec<T>,
    hard_limit: usize,
    value: T,
    message: &'static str,
) -> Result<()> {
    if output.len() >= hard_limit || output.len() >= output.capacity() {
        return Err(FeregError::new(ErrorCode::Internal, message));
    }
    output.push(value);
    Ok(())
}

#[cfg(test)]
mod tests;
