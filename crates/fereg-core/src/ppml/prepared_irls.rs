//! PPML IRLS over the reusable certified prepared weighted-linear session.
//!
//! PPML owns initialization, likelihood, convergence, rank-freezing, and
//! publication policy. The shared session owns only a certified weighted
//! linear subproblem on an immutable prepared model.

#[cfg(test)]
use crate::fe::PARALLEL_PROJECTION_MIN_ROWS;
#[cfg(test)]
use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, invalid_data, ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_fixed_effects, BorrowedMapWeights, CmgConfig, CmgTopologyProfile, FeDesign,
    FeSolverKind, PreparedCmgTopology, SolveReport, CMG_MAX_NUISANCE_COLUMNS,
};
use crate::memory::{fallible_copy, fallible_filled, FeSolverMemoryBudget};
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::ols::{
    classify_frozen_linear_fallback, CertifiedLinearSession, CertifiedLinearSessionOutput,
    CertifiedLinearSessionProducts, FrozenLinearFallbackKind, LinearSessionGenerations,
    LinearSessionPhaseAllocatorProbe, LinearSessionReuseReport, LinearSessionWorkspaceCapacities,
    LinearSolveFrame, PpmlOrdinaryLinearOutput, PreparedOlsBreadWeightScale, PreparedOlsColumnKind,
    PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsFitStatus,
    PreparedOlsFittedCoefficient,
};

use super::categorical_fwl_cmg::{
    estimate_fused_cmg_workspace_bytes, fused_cmg_batch_size, FusedFwlCmgConfig,
    FusedFwlCmgFailureReport, FusedFwlCmgReport, FusedFwlCmgSolveOutcome,
    ReusableCategoricalFwlCmg,
};
use super::categorical_lsmr::{
    estimate_checkpoint_lsmr_workspace_bytes, estimate_joint_lsmr_workspace_bytes,
    joint_lsmr_policy, JointLsmrFrameDiagnostic, JointLsmrReport, ReusableCategoricalJointLsmr,
};
#[cfg(test)]
use super::family::poisson_objective_from_eta;
use super::family::{means_from_eta, CompensatedSum, PreparedPoissonInputs};
#[cfg(test)]
use super::memory::DIAGNOSTIC_RECORD_BYTES;
use super::memory::{begin_phase_allocation, end_phase_allocation};
use super::one_fe_profile::{ProfiledColumns, ProfiledOneFe, ProfiledRawColumns};
#[cfg(test)]
use super::plan_ppml_memory;
use super::preparation_trace::PreparationTrace;
use super::prepared_estimator::PpmlInterruptPoint;
use super::prepared_model::{PpmlGenerationBinding, PreparedPpmlModelGeneration};
use super::profiled_intercept_curve::ProfiledInterceptCurve;
use super::{
    scheduled_inner_tolerance, strict_inner_tolerance, PoissonObjective, PpmlAllocatorSnapshot,
    PpmlAllocatorSnapshotProbe, PpmlCertificate, PpmlConfig, PpmlFallbackKind,
    PpmlInnerToleranceProgress, PpmlIterationWorkspace, PpmlLinearWorkspaceCapacityReport,
    PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryPlan, PpmlMemoryRoute, PpmlMemoryTelemetry,
    PpmlPhase, PpmlPhaseAllocatorProbe, PpmlPhaseClock, PpmlSolverEngine, PpmlSolverFrameKind,
    PpmlSolverObservation, PpmlSolverPlan, PpmlSolverReport, PpmlSteadyStateAllocationObservation,
    PpmlWorkspaceCapacityReport, PPML_STEADY_STATE_AUDIT_ITERATIONS,
};

mod controller;

mod retry;

mod direction;
use direction::OrdinaryDirection;

mod admission;
use admission::LinearAdmission;

mod audit;
use audit::PpmlAllocationAuditWindow;

const MAX_RETAINED_ITERATION_REPORTS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileRecoveryAction {
    Continue,
    Rebuild,
    Exhausted,
}

#[derive(Default)]
struct ProfileRecovery {
    discrepancies: [f64; 3],
    count: usize,
    // The rebuilt checkpoint and at most two further strict corrections.
    attempts_left: Option<usize>,
}

impl ProfileRecovery {
    fn observe(
        &mut self,
        eligible: bool,
        same_basis: bool,
        difference: f64,
    ) -> ProfileRecoveryAction {
        if !same_basis {
            *self = Self::default();
        }
        if !eligible {
            self.count = 0;
            return ProfileRecoveryAction::Continue;
        }
        if let Some(left) = &mut self.attempts_left {
            *left = left.saturating_sub(1);
            return if *left == 0 {
                ProfileRecoveryAction::Exhausted
            } else {
                ProfileRecoveryAction::Continue
            };
        }
        self.discrepancies[self.count] = difference;
        self.count += 1;
        if self.count < self.discrepancies.len() {
            return ProfileRecoveryAction::Continue;
        }
        if difference < 0.5 * self.discrepancies[0] {
            self.discrepancies[0] = difference;
            self.count = 1;
            return ProfileRecoveryAction::Continue;
        }
        self.count = 0;
        self.attempts_left = Some(3);
        ProfileRecoveryAction::Rebuild
    }
}

fn refined_checkpoint_tolerance(current: f64) -> f64 {
    // A certified warm residual may retain an old profile whose movement is
    // too large for publication. Refine computation, never the acceptance
    // envelope; preserve any user target already below the recurrence floor.
    (current * 0.01).max(1e-14).min(current)
}

fn use_checkpoint_lsmr_accelerator(observations: usize, effects: usize) -> bool {
    matches!(effects, 2 | 3) && observations > 0
}

fn use_initial_checkpoint_lsmr_accelerator(
    config: &PreparedPpmlIrlsConfig,
    purpose: PreparedPpmlIrlsPurpose,
    observations: usize,
    effects: usize,
) -> bool {
    matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
        && matches!(
            config.solver_plan.selected,
            super::PpmlSolverFamily::Lsmr | super::PpmlSolverFamily::Cmg
        )
        && use_checkpoint_lsmr_accelerator(observations, effects)
}

// Test fault plans; shipped builds compile these branches out.
crate::fault::fault_plan! {
    static FORCE_GENERIC_OUTER_SCORE: bool = false;
    static CANONICAL_FINAL_TEST_DISABLED: bool = false;
    static PROFILE_CURVE_TEST_DISABLED: bool = false;
    static PROFILE_FINAL_TEST_MODE: u8 = 0;
    static JOINT_FINAL_TEST_MODE: u8 = 0;
    static STRICT_RESUME_INJECTION: u8 = 0;
    static REPEATED_STRICT_RESUME_INJECTION: usize = 0;
    static STRICT_BASIS_CHANGE_INJECTION: u8 = 0;
}

// Test-only observation counters; they never change control flow.
#[cfg(test)]
thread_local! {
    static PROFILE_FINAL_ACCEPTED: Cell<usize> = const { Cell::new(0) };
    static JOINT_FINAL_TEST_COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
    static CANONICAL_FRAME_REFRESH_COUNT: Cell<usize> = const { Cell::new(0) };
    static FUSED_STRICT_WITNESS_COUNT: Cell<usize> = const { Cell::new(0) };
    static CMG_EARLY_MEMORY_PEAKS: Cell<Option<(u128, u128)>> = const { Cell::new(None) };
}

#[cfg(test)]
pub(super) fn inject_repeated_strict_resume(count: usize) {
    REPEATED_STRICT_RESUME_INJECTION.set(count);
}

#[cfg(test)]
fn inject_strict_resume(first_strict: bool, final_hessian: bool) {
    STRICT_RESUME_INJECTION.set(u8::from(first_strict) | (u8::from(final_hessian) << 1));
}

fn take_strict_resume(bit: u8) -> bool {
    if bit == 1 && REPEATED_STRICT_RESUME_INJECTION.get() > 0 {
        REPEATED_STRICT_RESUME_INJECTION.set(REPEATED_STRICT_RESUME_INJECTION.get() - 1);
        return true;
    }
    let state = STRICT_RESUME_INJECTION.get();
    let selected = state & bit != 0;
    STRICT_RESUME_INJECTION.set(state & !bit);
    selected
}

#[cfg(test)]
pub(super) fn inject_strict_basis_change(first_strict: bool, final_hessian: bool) {
    STRICT_BASIS_CHANGE_INJECTION.set(u8::from(first_strict) | (u8::from(final_hessian) << 1));
}

fn take_strict_basis_change(bit: u8) -> bool {
    let state = STRICT_BASIS_CHANGE_INJECTION.get();
    let selected = state & bit != 0;
    STRICT_BASIS_CHANGE_INJECTION.set(state & !bit);
    selected
}

#[cfg(test)]
pub(super) fn reset_canonical_frame_refresh_count() {
    CANONICAL_FRAME_REFRESH_COUNT.set(0);
    FUSED_STRICT_WITNESS_COUNT.set(0);
}

#[cfg(test)]
pub(super) fn take_canonical_frame_refresh_count() -> usize {
    CANONICAL_FRAME_REFRESH_COUNT.replace(0)
}

#[cfg(test)]
pub(super) fn take_fused_strict_witness_count() -> usize {
    FUSED_STRICT_WITNESS_COUNT.replace(0)
}

#[cfg(test)]
pub(super) fn take_cmg_early_memory_peaks() -> Option<(u128, u128)> {
    CMG_EARLY_MEMORY_PEAKS.replace(None)
}

fn inject_candidate_profile_movement(workspace: &mut PpmlIterationWorkspace) {
    // Move a complete candidate profile, not merely a control-flow flag, so
    // the strict resume tests exercise eta, beta, and objective comparisons.
    const DELTA: f64 = 1e-4;
    for value in workspace.candidate_eta_mut() {
        *value += DELTA;
    }
    if let Some(value) = workspace.candidate_coefficients_mut().last_mut() {
        *value += DELTA;
    }
}

fn profile_curve_enabled() -> bool {
    !PROFILE_CURVE_TEST_DISABLED.get()
}

fn canonical_final_enabled() -> bool {
    !CANONICAL_FINAL_TEST_DISABLED.get()
}

#[cfg(test)]
pub(super) struct CanonicalFinalTestGuard(bool);
#[cfg(test)]
impl CanonicalFinalTestGuard {
    pub(super) fn disabled() -> Self {
        Self(CANONICAL_FINAL_TEST_DISABLED.replace(true))
    }
}
#[cfg(test)]
impl Drop for CanonicalFinalTestGuard {
    fn drop(&mut self) {
        CANONICAL_FINAL_TEST_DISABLED.set(self.0);
    }
}

#[cfg(test)]
pub(super) struct ProfileCurveTestGuard(bool);
#[cfg(test)]
impl ProfileCurveTestGuard {
    pub(super) fn disabled() -> Self {
        Self(PROFILE_CURVE_TEST_DISABLED.replace(true))
    }
}
#[cfg(test)]
impl Drop for ProfileCurveTestGuard {
    fn drop(&mut self) {
        PROFILE_CURVE_TEST_DISABLED.set(self.0);
    }
}

#[cfg(test)]
pub(super) fn profile_final_test_mode(mode: u8) -> usize {
    PROFILE_FINAL_TEST_MODE.set(mode);
    PROFILE_FINAL_ACCEPTED.replace(0)
}

/// PPML-owned policy for calls into the shared linear engine.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedPpmlIrlsConfig {
    /// Experimental immutable component indexing; numerical frame policies stay fixed.
    pub(crate) topology_sharing: bool,
    pub(crate) ppml: PpmlConfig,
    pub(crate) linear: PreparedOlsFitConfig,
    pub(crate) memory: PpmlMemoryConfig,
    /// Qualification-only allocator observer. `None` is the production-fast
    /// default and imposes no callback or counter-read cost on OLS.
    pub(crate) allocation_snapshot_probe: Option<PpmlAllocatorSnapshotProbe>,
    pub(crate) phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    /// Qualification-only neutral probe installed on the shared linear
    /// session. It observes projection and finite-solve segments without
    /// coupling the OLS engine to PPML phase types.
    pub(crate) linear_phase_allocation_probe: Option<LinearSessionPhaseAllocatorProbe>,
    /// Public request/selection identity. MAP remains the canonical linear
    /// configuration even when the selected ordinary-frame engine is LSMR
    /// or prepared CMG.
    pub(crate) solver_plan: PpmlSolverPlan,
}

impl Default for PreparedPpmlIrlsConfig {
    fn default() -> Self {
        let mut ppml = PpmlConfig::default();
        if ppml.absorb.solver == FeSolverKind::Auto {
            ppml.absorb.solver = FeSolverKind::Map;
        }
        let linear = PreparedOlsFitConfig {
            absorb: ppml.absorb,
            fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            ..PreparedOlsFitConfig::default()
        };
        Self {
            topology_sharing: false,
            ppml,
            linear,
            memory: PpmlMemoryConfig::default(),
            allocation_snapshot_probe: None,
            phase_allocation_probe: None,
            linear_phase_allocation_probe: None,
            solver_plan: PpmlSolverPlan::new(super::PpmlSolverFamily::Auto),
        }
    }
}

/// Warm start on the canonical prepared coefficient-slot axis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlStart {
    pub(crate) coefficient_values: Vec<f64>,
    /// Full linear predictor, including offset and absorbed contribution.
    pub(crate) eta: Vec<f64>,
}

/// Why a certified nonpublication trajectory stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparedPpmlMuTerminal {
    Converged,
    MaximumIterations,
    LineSearchStalled,
    CandidateThresholdReached,
}

/// Independent integrity certificate for an exploratory Mu trajectory.
///
/// This deliberately does not require the PPML score certificate to pass: an
/// exact separated sample has no finite score root. It instead certifies that
/// every accepted state came from a certified shared-linear subproblem and
/// the unchanged bounded line search, that deviance respected its registered
/// slack, and that the retained means independently reconstruct from eta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreparedPpmlMuExplorationCertificate {
    pub(crate) accepted_iterations: usize,
    pub(crate) certified_linear_subproblems: usize,
    pub(crate) maximum_deviance_increase_relative: f64,
    pub(crate) mean_reconstruction_relative: f64,
    pub(crate) terminal: PreparedPpmlMuTerminal,
    pub(crate) passed: bool,
}

/// Certified exploratory state usable only by fitted-mean Mu separation.
///
/// It intentionally has no conversion to `PreparedPpmlIrlsFit`; consuming it
/// can only inspect the compact eta/mean trajectory. Publication must perform
/// a new full converged, strict, final-Hessian-synchronized fit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlMuExploration {
    binding: Option<PpmlGenerationBinding>,
    mean: Vec<f64>,
    certificate: PreparedPpmlMuExplorationCertificate,
}

impl PreparedPpmlMuExploration {
    pub(super) fn validate_generation(
        &self,
        generation: &PreparedPpmlModelGeneration,
    ) -> Result<()> {
        self.binding
            .as_ref()
            .ok_or_else(|| invalid_argument("PPML artifact has no prepared-generation binding"))?
            .validate(generation, self.mean.len())
    }
    pub(crate) fn mean(&self) -> &[f64] {
        &self.mean
    }

    pub(crate) const fn certificate(&self) -> PreparedPpmlMuExplorationCertificate {
        self.certificate
    }

    #[cfg(test)]
    pub(crate) fn from_test_eta(
        eta: Vec<f64>,
        generation: &PreparedPpmlModelGeneration,
        bounds: super::PoissonBounds,
    ) -> Result<Self> {
        let mut mean = vec![0.0; eta.len()];
        means_from_eta(&eta, bounds, &mut mean)?;
        let _objective = poisson_objective_from_eta(
            generation.outcome(),
            &eta,
            &mean,
            generation.prior_weights(),
        )?;
        Ok(Self {
            binding: Some(PpmlGenerationBinding::capture(generation)?),
            mean,
            certificate: PreparedPpmlMuExplorationCertificate {
                accepted_iterations: 1,
                certified_linear_subproblems: 1,
                maximum_deviance_increase_relative: 0.0,
                mean_reconstruction_relative: 0.0,
                terminal: PreparedPpmlMuTerminal::MaximumIterations,
                passed: true,
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PreparedPpmlIrlsPurpose {
    Publication,
    MuExploration { mu_tolerance: f64 },
}

pub(super) enum PreparedPpmlIrlsRun {
    Publication(Box<PreparedPpmlIrlsFit>),
    MuExploration(Box<PreparedPpmlMuExploration>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlIterationReport {
    pub(crate) iteration: usize,
    pub(crate) deviance: f64,
    pub(crate) log_likelihood: f64,
    pub(crate) relative_deviance_change: f64,
    pub(crate) step: f64,
    pub(crate) step_halvings: usize,
    pub(crate) inner_tolerance: f64,
    pub(crate) finite_rank: usize,
    pub(crate) score: PpmlCertificate,
    pub(crate) linear_reuse: LinearSessionReuseReport,
    pub(crate) joint_lsmr: Option<JointLsmrFrameDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlIrlsDiagnostics {
    pub(crate) converged: bool,
    pub(crate) two_consecutive_convergence: bool,
    pub(crate) rank_frozen: bool,
    pub(crate) iterations: usize,
    /// Checked sum over every executed certified linear subproblem. Each
    /// contributes at least one unit, or its FE solver iteration count when
    /// larger; line-search trials are not linear subproblems.
    pub(crate) total_inner_iterations: usize,
    pub(crate) retained_iterations: Vec<PreparedPpmlIterationReport>,
    pub(crate) total_step_halvings: usize,
    pub(crate) strict_profile_rejections: usize,
    pub(crate) final_score: PpmlCertificate,
    pub(crate) solver: PpmlSolverReport,
    pub(crate) memory: PpmlMemoryTelemetry,
}

/// Converged PPML state and its owned final Hessian products.
///
/// The owned handoff keeps either dense or scratch-backed compact within rows
/// alive for inference without reconstructing an observation-by-slot matrix.
pub(crate) struct PreparedPpmlIrlsFit {
    binding: Option<PpmlGenerationBinding>,
    bias: Option<super::PpmlBiasResult>,
    linear: CertifiedLinearSessionProducts,
    coefficients: Vec<PreparedOlsFittedCoefficient>,
    eta: Vec<f64>,
    mean: Vec<f64>,
    score_residuals: Vec<f64>,
    objective: PoissonObjective,
    diagnostics: PreparedPpmlIrlsDiagnostics,
}

impl PreparedPpmlIrlsFit {
    pub(super) fn validate_generation(
        &self,
        generation: &PreparedPpmlModelGeneration,
    ) -> Result<()> {
        self.binding
            .as_ref()
            .ok_or_else(|| invalid_argument("PPML artifact has no prepared-generation binding"))?
            .validate(generation, self.mean.len())
    }
    pub(crate) fn take_bias(&mut self) -> Option<super::PpmlBiasResult> {
        self.bias.take()
    }
    pub(crate) fn coefficients(&self) -> &[PreparedOlsFittedCoefficient] {
        &self.coefficients
    }

    pub(crate) fn eta(&self) -> &[f64] {
        &self.eta
    }

    pub(crate) fn mean(&self) -> &[f64] {
        &self.mean
    }

    pub(crate) fn score_residuals(&self) -> &[f64] {
        &self.score_residuals
    }

    pub(crate) const fn objective(&self) -> PoissonObjective {
        self.objective
    }

    pub(crate) const fn diagnostics(&self) -> &PreparedPpmlIrlsDiagnostics {
        &self.diagnostics
    }

    pub(crate) const fn linear(&self) -> &CertifiedLinearSessionProducts {
        &self.linear
    }
}

struct PpmlScoreWorkspace {
    team: crate::parallel::InstalledThreadTeam,
    fe_moments: Vec<f64>,
    fe_basis_squares: Vec<f64>,
}

impl PpmlScoreWorkspace {
    fn new(design: &FeDesign, threads: crate::options::ThreadConfig) -> Result<Self> {
        let directions = design.effects().iter().try_fold(0_usize, |total, effect| {
            let effect_directions = effect
                .n_levels()
                .checked_mul(effect.channels())
                .ok_or_else(|| allocation("prepared PPML FE-score effect dimensions overflow"))?;
            total
                .checked_add(effect_directions)
                .ok_or_else(|| allocation("prepared PPML FE-score dimensions overflow"))
        })?;
        Ok(Self {
            team: crate::parallel::ThreadTeam::new(threads).acquire()?,
            fe_moments: fallible_filled(
                directions,
                0.0,
                "unable to reserve prepared PPML FE-score moments",
            )?,
            fe_basis_squares: fallible_filled(
                directions,
                0.0,
                "unable to reserve prepared PPML FE-score norms",
            )?,
        })
    }

    /// FE banks are idle until the certificate's FE pass clears them. Reuse
    /// their admitted storage without changing a capacity or memory route.
    /// Waves only schedule fixed leaves: every column merges each leaf's sum
    /// then correction in ascending row order, including the stack fallback.
    fn reduce_columns(
        &mut self,
        rows: usize,
        output: &mut [(f64, f64)],
        leaf: impl Fn(usize, std::ops::Range<usize>) -> Result<(CompensatedSum, CompensatedSum)> + Sync,
    ) -> Result<()> {
        use rayon::prelude::*;
        const WIDTH: usize = crate::ols::MAX_BOUND_NUMERIC_COLUMNS;
        const LEAF_ROWS: usize = crate::fe::PARALLEL_DOT_CHUNK_ROWS;
        const MAX_WAVE_LEAVES: usize = 128;
        debug_assert!(output.len() <= WIDTH);
        let leaves = rows.div_ceil(LEAF_ROWS);
        if leaves <= 1 {
            for (column, target) in output.iter_mut().enumerate() {
                // Preserve the former scalar total for a single leaf.
                let (moment, square) = leaf(column, 0..rows)?;
                *target = (moment.total(), square.total());
            }
            return Ok(());
        }
        // Each bank stores two components per column/leaf. Halving usable
        // capacity changes waves only; it never increases admitted storage.
        let capacity = self.fe_moments.len().min(self.fe_basis_squares.len()) / 2;
        let tile = output.len().min(capacity.max(1)).max(1);
        for first in (0..output.len()).step_by(tile) {
            let width = tile.min(output.len() - first);
            let mut moments = [CompensatedSum::default(); WIDTH];
            let mut squares = [CompensatedSum::default(); WIDTH];
            let wave = (capacity / width).min(MAX_WAVE_LEAVES);
            if wave == 0 {
                for index in 0..leaves {
                    let start = index * LEAF_ROWS;
                    let (moment, square) = leaf(first, start..(start + LEAF_ROWS).min(rows))?;
                    for component in moment.components() {
                        moments[0].add(component);
                    }
                    for component in square.components() {
                        squares[0].add(component);
                    }
                }
            } else {
                for start in (0..leaves).step_by(wave) {
                    let count = wave.min(leaves - start);
                    let cells = count * width;
                    let first_bank = &mut self.fe_moments[..2 * cells];
                    let second_bank = &mut self.fe_basis_squares[..2 * cells];
                    let compute = |(index, (moment, square)): (usize, (&mut [f64], &mut [f64]))| {
                        let row = (start + index / width) * LEAF_ROWS;
                        let (first, second) =
                            leaf(first + index % width, row..(row + LEAF_ROWS).min(rows))?;
                        moment.copy_from_slice(&first.components());
                        square.copy_from_slice(&second.components());
                        Ok(())
                    };
                    let work = (count * LEAF_ROWS).min(rows - start * LEAF_ROWS) as u128
                        * width as u128
                        * 6;
                    if crate::parallel::StagePlan::new(work, cells, self.team.threads()).lanes > 1 {
                        self.team.install(|| {
                            crate::parallel::stage_try_for_each(
                                first_bank
                                    .par_chunks_exact_mut(2)
                                    .zip(second_bank.par_chunks_exact_mut(2))
                                    .enumerate(),
                                work,
                                compute,
                            )
                        })?;
                    } else {
                        // A cold serial call must not allocate a Rayon queue.
                        first_bank
                            .chunks_exact_mut(2)
                            .zip(second_bank.chunks_exact_mut(2))
                            .enumerate()
                            .try_for_each(compute)?;
                    }
                    for index in 0..cells {
                        for component in 0..2 {
                            moments[index % width].add(first_bank[2 * index + component]);
                            squares[index % width].add(second_bank[2 * index + component]);
                        }
                    }
                }
            }
            for column in 0..width {
                output[first + column] = (moments[column].total(), squares[column].total());
            }
        }
        Ok(())
    }
}

/// Fit PPML after separation and topology rebasing are complete.
#[cfg(test)]
pub(crate) fn fit_prepared_ppml_irls<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
) -> Result<PreparedPpmlIrlsFit> {
    fit_prepared_ppml_irls_interruptible(
        prepared,
        outcome,
        prior_weights,
        offset,
        config,
        memory_plan,
        start,
        &mut |_| Ok(()),
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_prepared_ppml_irls_interruptible<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsFit> {
    match run_prepared_ppml_irls(
        PreparedPpmlFitFrame {
            prepared,
            shared_prepared: None,
            outcome,
            prior_weights,
            offset,
            binding: None,
        },
        config,
        memory_plan,
        start,
        PreparedPpmlIrlsPurpose::Publication,
        None,
        interrupt,
    )? {
        PreparedPpmlIrlsRun::Publication(fit) => Ok(*fit),
        PreparedPpmlIrlsRun::MuExploration(_) => Err(FeregError::new(
            ErrorCode::Internal,
            "publication PPML run returned an exploratory state",
        )),
    }
}

/// Every numerical consumer borrows the same finalized original-row axis.
/// The model epoch is independent of changing response/weight/basis stamps.
struct PreparedPpmlFitFrame<'a, P: ?Sized> {
    prepared: &'a P,
    shared_prepared: Option<crate::ols::prepared_fit_view::SharedPreparedOlsData>,
    outcome: &'a [f64],
    prior_weights: &'a [f64],
    offset: &'a [f64],
    binding: Option<PpmlGenerationBinding>,
}

pub(super) fn fit_generation_ppml_irls_interruptible(
    generation: &super::prepared_model::PreparedPpmlModelGeneration,
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
    bias: Option<(super::PpmlBiasRequest, &[f64])>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsRun> {
    let frame = PreparedPpmlFitFrame {
        prepared: generation.fit_view(),
        // Preserve exploratory storage admission: only publication may retain
        // the shared implicit-column source beside its canonical checkpoint.
        shared_prepared: matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            .then(|| generation.prepared().clone()),
        outcome: generation.outcome(),
        prior_weights: generation.prior_weights(),
        offset: generation.offset(),
        binding: Some(PpmlGenerationBinding::capture(generation)?),
    };
    run_prepared_ppml_irls(frame, config, memory_plan, start, purpose, bias, interrupt)
}

fn run_prepared_ppml_irls<P: PreparedOlsFitView + Sync + ?Sized>(
    frame: PreparedPpmlFitFrame<'_, P>,
    mut config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
    bias: Option<(super::PpmlBiasRequest, &[f64])>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsRun> {
    let PreparedPpmlFitFrame {
        prepared,
        mut shared_prepared,
        outcome,
        prior_weights,
        offset,
        binding,
    } = frame;
    let model_generation = binding
        .as_ref()
        .map_or(0, PpmlGenerationBinding::generation);
    // Direct internal callers may start from the shared FE default. Resolve
    // that default to the canonical PPML checkpoint engine before enforcing
    // the invariant; public route selection remains fixed in `solver_plan`.
    if config.ppml.absorb.solver == FeSolverKind::Auto {
        config.ppml.absorb.solver = FeSolverKind::Map;
    }
    if config.linear.absorb.solver == FeSolverKind::Auto {
        config.linear.absorb.solver = FeSolverKind::Map;
    }
    begin_phase_allocation(config.phase_allocation_probe, PpmlPhase::Preparation);
    let preparation_clock = PpmlPhaseClock::start();
    let mut preparation_trace = PreparationTrace::new();
    validate_inputs(prepared, outcome, prior_weights, offset, config)?;
    validate_memory_dimensions(prepared, memory_plan.dimensions, config)?;
    let validation_seconds = preparation_trace.lap();
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=prepared_irls_start|original_rows={}|active_rows={}|finite_columns={}|memory={:?}",
            memory_plan.dimensions.original_observations,
            outcome.len(),
            memory_plan.dimensions.finite_columns,
            memory_plan.selected,
        );
    }
    // Route selection happened exactly once on generation zero, before any
    // separation detector workspace. The composite owner rebases only the
    // exact final dimensions into that conservative fit-lifetime plan.
    let admission = LinearAdmission::new_public(prepared, memory_plan, &mut config, purpose)?;
    let linear_admission_seconds = preparation_trace.lap();
    // The route and its complete fit-lifetime budget are admitted before this
    // observation-scale owned copy of the already-finalized FE topology.
    let fe_design = prepared.fit_owned_fe_design()?;
    let fe_design_seconds = preparation_trace.lap();
    let requested_threads = config
        .linear
        .execution
        .threads
        .requested
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(0);
    let effective_threads = config.linear.execution.threads.resolved();
    let (memory_plan, solver_admission, solver_memory_budget, installation) = admission
        .admit_installation(
            prepared,
            &fe_design,
            &mut config,
            purpose,
            effective_threads,
            shared_prepared.is_some(),
        )?;
    let mut memory = PpmlMemoryTelemetry::new(memory_plan, requested_threads, effective_threads)?;
    if config.solver_plan.selected == super::PpmlSolverFamily::Cmg {
        memory.expected_solver_workspace_peak =
            Some(crate::memory::SolverWorkspacePeak::from_bytes(
                solver_admission.checkpoint_workspace_bytes
                    + solver_admission.owned_fe_design_shortfall_bytes,
            )?);
    }
    // Admission and telemetry bind this fit to one resolved team. Repeated
    // automatic resolution may allocate while querying OS CPU quotas (Linux),
    // and must not change the team behind the admitted workspace dimensions.
    // Keep the caller's requested value in telemetry, then use this explicit
    // count for every ordinary, fallback and strict linear frame in the fit.
    config.linear.execution.threads.requested = std::num::NonZeroUsize::new(effective_threads);
    let installation_admission_seconds = preparation_trace.lap();

    let (coefficient_values, eta) = initialize(
        prepared,
        outcome,
        prior_weights,
        offset,
        &fe_design,
        config,
        start,
        purpose,
    )?;
    let starts_seconds = preparation_trace.lap();
    let objective_inputs = PreparedPoissonInputs::new_with_threads(
        outcome,
        prior_weights,
        offset,
        config.linear.execution.threads,
    )?;
    let mut workspace =
        PpmlIterationWorkspace::from_initial_state(memory_plan, eta, coefficient_values)?;
    workspace.refresh_prepared_working_state(&objective_inputs, config.ppml.bounds)?;
    let mut objective = workspace.evaluate_current_objective(&objective_inputs)?;
    let workspace_seconds = preparation_trace.lap();
    let mut score_workspace = PpmlScoreWorkspace::new(&fe_design, config.linear.execution.threads)?;
    let initial_tolerance =
        scheduled_inner_tolerance(config.ppml, 1, config.ppml.inner_tolerance_initial, None);
    let initial_config = ppml_linear_config(config, initial_tolerance)?;
    let frame = LinearSolveFrame::from_parts(
        workspace.working_response(),
        BorrowedMapWeights::Provided(workspace.hessian_weights()),
        ppml_bread_weight_scale(workspace.hessian_weights())?,
    )?;
    let frame_seconds = preparation_trace.lap();
    let (mut session, profiles, initial_checkpoint_lsmr) = installation.install(
        prepared,
        frame,
        initial_config,
        LinearSessionGenerations::new(model_generation, model_generation, 1, 1, 1),
        fe_design,
        &mut shared_prepared,
        &config,
        purpose,
        effective_threads,
    )?;
    let installation_seconds = preparation_trace.lap();
    let mut direction = OrdinaryDirection::Canonical;
    let mut profiled_curve: Option<ProfiledInterceptCurve> = None;
    let mut strict_control = controller::StrictCheckpointControl::new(
        strict_inner_tolerance(config.ppml),
        canonical_final_enabled()
            && matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && session.fe_design().effects().len() >= 2,
    );
    let mut solver_report = None;
    let retained_limit = MAX_RETAINED_ITERATION_REPORTS
        .min(config.memory.diagnostic_capacity)
        .min(config.ppml.max_iterations);
    let mut retained_iterations = Vec::new();
    retained_iterations
        .try_reserve_exact(retained_limit)
        .map_err(|_| allocation("unable to reserve prepared PPML diagnostics"))?;
    let mut total_step_halvings = 0_usize;
    let diagnostics_seconds = preparation_trace.lap();
    let mut final_score = prepared_score_certificate(
        prepared,
        session.fe_design(),
        outcome,
        workspace.mean(),
        prior_weights,
        config.ppml.score_tolerance,
        &mut score_workspace,
    )?;
    let initial_score_seconds = preparation_trace.lap();
    // Set with every accepted-iteration score below, before any strict cycle.
    let mut final_score_generation: u64;
    let mut previous_inner_tolerance = initial_tolerance;
    let mut previous_outer_progress = None;
    let mut rank_frozen = false;
    let mut completed_iterations = 0_usize;
    let mut total_inner_iterations = 0_usize;
    let mut frame_generation = 1_u64;
    let mut accepted_iterations = 0_usize;
    let mut certified_linear_subproblems = 0_usize;
    let mut maximum_deviance_increase_relative = 0.0_f64;
    let mut exploration_terminal = PreparedPpmlMuTerminal::MaximumIterations;
    let mut allocation_audit = PpmlAllocationAuditWindow::new(config.allocation_snapshot_probe);
    let finish_seconds = preparation_trace.lap();
    let preparation_elapsed = preparation_clock.elapsed();
    memory.record_phase(PpmlPhase::Preparation, preparation_elapsed);
    record_phase_allocation(
        &mut memory,
        config.phase_allocation_probe,
        PpmlPhase::Preparation,
    );
    if preparation_trace.enabled() {
        let purpose = match purpose {
            PreparedPpmlIrlsPurpose::Publication => "publication",
            PreparedPpmlIrlsPurpose::MuExploration { .. } => "mu_exploration",
        };
        eprintln!(
            "FEREG_PHASE|ppml_prepare_irls|generation={model_generation}|purpose={purpose}|validation={validation_seconds:.9}|linear_admission={linear_admission_seconds:.9}|fe_design={fe_design_seconds:.9}|installation_admission={installation_admission_seconds:.9}|starts={starts_seconds:.9}|workspace={workspace_seconds:.9}|frame={frame_seconds:.9}|installation={installation_seconds:.9}|diagnostics={diagnostics_seconds:.9}|initial_score={initial_score_seconds:.9}|finish={finish_seconds:.9}|total={:.9}|rows={}|threads={effective_threads}|team_workers={}",
            preparation_elapsed.as_secs_f64(),
            outcome.len(),
            PreparationTrace::team_workers(),
        );
    }

    for iteration in 1..=config.ppml.max_iterations {
        crate::interrupt::checkpoint()?;
        interrupt(PpmlInterruptPoint::IrlsIteration)?;
        // The qualification-only median covers the same complete warmed
        // iterations as the allocator window.  `None` is the normal path and
        // avoids even an extra clock read.
        let audited_iteration_clock = allocation_audit.is_measuring().then(Instant::now);
        completed_iterations = iteration;
        let inner_tolerance = scheduled_inner_tolerance(
            config.ppml,
            iteration,
            previous_inner_tolerance,
            previous_outer_progress,
        );
        previous_inner_tolerance = inner_tolerance;
        begin_phase_allocation(
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        let refresh_clock = PpmlPhaseClock::start();
        session.set_config(ppml_linear_config(config, inner_tolerance)?)?;
        if iteration > 1 {
            frame_generation = next_frame_generation(frame_generation)?;
            if direction.is_canonical() {
                refresh_linear_frame(&mut session, &workspace, frame_generation)?;
            }
        }
        memory.record_phase(PpmlPhase::WeightResponseRefresh, refresh_clock.elapsed());
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        let mut profiled_step = None;
        if iteration > 1 {
            if let Some(profile) = direction.profile_mut() {
                let started = Instant::now();
                let profile_direction = if config.linear.execution.mode
                    == PreparedOlsExecutionMode::Low
                    || session.has_implicit_one_fe()
                {
                    let columns = session.one_fe_original_columns(profile.active_columns())?;
                    let data = ProfiledRawColumns::new(prepared, columns)?;
                    profile.prepare_direction(
                        &data,
                        session.fe_design(),
                        outcome,
                        prior_weights,
                        offset,
                        workspace.coefficients(),
                        config.ppml,
                    )
                } else {
                    let (values, positions) =
                        session.one_fe_representatives(profile.active_columns())?;
                    profile.prepare_direction(
                        ProfiledColumns::selected(values, positions)?,
                        session.fe_design(),
                        outcome,
                        prior_weights,
                        offset,
                        workspace.coefficients(),
                        config.ppml,
                    )
                };
                match profile_direction {
                    Ok(error) => {
                        profiled_step = Some((
                            profile.active_rank(),
                            error,
                            started.elapsed().as_secs_f64(),
                        ));
                        if allocation_audit.needs_equivalence() {
                            let equivalent = profile.fresh_direction_equivalent(
                                prepared,
                                session.fe_design(),
                                outcome,
                                prior_weights,
                                offset,
                                workspace.coefficients(),
                                config.linear.execution.threads,
                                config.ppml,
                                fresh_warm_tolerance(config.ppml, inner_tolerance),
                            )?;
                            let observation = allocation_audit.complete_equivalence(equivalent)?;
                            memory.observe_steady_state_allocation(observation)?;
                        }
                    }
                    Err(error) if error.code == ErrorCode::NonConvergence => {
                        if allocation_audit.needs_equivalence() {
                            let observation = allocation_audit.complete_equivalence(false)?;
                            memory.observe_steady_state_allocation(observation)?;
                        }
                        direction.retire_profile();
                        memory.record_fallback(PpmlFallbackKind::Route);
                        refresh_linear_frame(&mut session, &workspace, frame_generation)?;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        let mut joint_lsmr_frame = None;
        let (finite_rank, reuse, linear_route) = if iteration == 1 {
            let output = session.solve_fresh()?;
            if !output.certified() {
                return Err(numerical_nonconvergence(
                    "prepared PPML initial weighted-linear solve did not certify",
                ));
            }
            certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
            accumulate_inner_iterations(
                &mut total_inner_iterations,
                output.frame().absorption_report(),
            )?;
            let selected_engine = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication) {
                config
                    .solver_plan
                    .selected_engine(output.basis().kept_generated_columns().len())
            } else {
                PpmlSolverEngine::Map
            };
            let mut initial_solver_report =
                PpmlSolverReport::new(config.solver_plan, selected_engine);
            if initial_checkpoint_lsmr {
                initial_solver_report.enable_lsmr_checkpoint();
                record_checkpoint_solver_observation(
                    &mut initial_solver_report,
                    output.frame().absorption_report(),
                    false,
                )?;
            } else {
                initial_solver_report.observe(map_solver_observation(
                    output.frame().absorption_report(),
                    PpmlSolverFrameKind::Checkpoint,
                ))?;
            }
            fill_candidate_eta_from_linear_output(&output, offset, workspace.candidate_eta_mut())?;
            fill_candidate_coefficients_from_linear_output(
                &output,
                workspace.candidate_coefficients_mut(),
            )?;
            let specialized_columns = (selected_engine != PpmlSolverEngine::Map
                || profiles.one_fe.is_some()
                || profiles.curve.is_some())
            .then(|| {
                fallible_copy(
                    output.basis().kept_generated_columns(),
                    "unable to reserve PPML specialized active-column identity",
                )
            })
            .transpose()?;
            let result = (
                output.frame().finite_rank(),
                output.reuse_report(),
                output.frame().absorption_report().used_solver,
            );
            profiled_curve = direction.initialize(
                prepared,
                session.fe_design(),
                specialized_columns.as_deref(),
                &profiles,
                outcome,
                prior_weights,
                config,
                selected_engine,
                solver_admission,
                solver_memory_budget,
            )?;
            if selected_engine != PpmlSolverEngine::Map
                && !initial_checkpoint_lsmr
                && use_checkpoint_lsmr_accelerator(
                    session.fe_design().nobs(),
                    session.fe_design().effects().len(),
                )
            {
                session.enable_checkpoint_lsmr(config.linear.execution.threads.deterministic);
                initial_solver_report.enable_lsmr_checkpoint();
            }
            solver_report = Some(initial_solver_report);
            if let Some(cmg) = direction.cmg() {
                record_cmg_heap_forecast(&mut memory, cmg, solver_admission)?;
            }
            result
        } else if let Some((rank, error, seconds)) = profiled_step {
            total_inner_iterations = total_inner_iterations
                .checked_add(1)
                .ok_or_else(|| internal("profile step count overflow"))?;
            solver_report
                .as_mut()
                .ok_or_else(|| internal("profile step lacks diagnostics"))?
                .observe(PpmlSolverObservation {
                    engine: PpmlSolverEngine::Map,
                    frame: PpmlSolverFrameKind::Ordinary,
                    retries: 0,
                    iterations: 1,
                    setup_seconds: 0.,
                    solve_seconds: seconds,
                    certification_seconds: 0.,
                    absolute_infinity_norm: None,
                    relative_infinity_norm: Some(error),
                    relative_two_norm: None,
                    normal_equation: Some(error),
                    projection_absolute_infinity_norm: None,
                    backward_error: Some(error),
                })?;
            rank_frozen = true;
            let mut reuse = LinearSessionReuseReport::ppml_specialized(0., seconds);
            reuse.steady_state_allocation_free = direction
                .profile()
                .is_some_and(ProfiledOneFe::capacities_unchanged);
            (rank, reuse, FeSolverKind::Map)
        } else if let Some((joint, engine)) = direction.lsmr_mut() {
            let report = joint.solve(
                session.fe_design(),
                outcome,
                workspace.mean(),
                workspace.hessian_weights(),
                inner_tolerance,
                config.linear.absorb.lsmr.max_iterations,
            )?;
            joint_lsmr_frame = Some(report.frame);
            joint.fill_candidate(&mut workspace)?;
            certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
            total_inner_iterations = total_inner_iterations
                .checked_add(report.iterations.max(1))
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "prepared PPML total inner iteration count overflowed",
                    )
                })?;
            solver_report
                .as_mut()
                .ok_or_else(|| internal("PPML LSMR solve lacks solver diagnostics"))?
                .observe(joint_solver_observation(report, engine))?;
            rank_frozen = true;
            (
                joint.active_rank(),
                LinearSessionReuseReport::ppml_specialized(
                    report.setup_seconds + report.solve_seconds,
                    report.certification_seconds,
                ),
                FeSolverKind::Lsmr,
            )
        } else if direction.cmg().is_some() {
            let attempt = direction
                .cmg_mut()
                .expect("fused CMG presence checked")
                .solve(
                    prepared,
                    session.fe_design(),
                    workspace.working_response(),
                    workspace.hessian_weights(),
                    inner_tolerance,
                )?;
            record_cmg_heap_forecast(
                &mut memory,
                direction.cmg().expect("CMG attempt retained"),
                solver_admission,
            )?;
            match attempt {
                FusedFwlCmgSolveOutcome::Certified(report) => {
                    let cmg = direction.cmg().expect("successful fused CMG is retained");
                    cmg.fill_candidate(&mut workspace, offset)?;
                    certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
                    total_inner_iterations = total_inner_iterations
                        .checked_add(report.iterations.max(1))
                        .ok_or_else(|| {
                            FeregError::new(
                                ErrorCode::Internal,
                                "prepared PPML total inner iteration count overflowed",
                            )
                        })?;
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML CMG solve lacks solver diagnostics"))?
                        .observe(cmg_solver_observation(report))?;
                    rank_frozen = true;
                    (
                        cmg.active_rank(),
                        LinearSessionReuseReport::ppml_specialized(
                            report.setup_seconds + report.solve_seconds,
                            report.finish_seconds,
                        ),
                        FeSolverKind::Cmg,
                    )
                }
                FusedFwlCmgSolveOutcome::NumericalFailure(report) => {
                    let result = retry::canonical_retry(
                        retry::CanonicalRetryReason::Cmg(report),
                        &mut direction,
                        &mut session,
                        &mut workspace,
                        offset,
                        frame_generation,
                        &mut memory,
                        &mut solver_report,
                        &mut certified_linear_subproblems,
                        &mut total_inner_iterations,
                        &mut allocation_audit,
                    )?;
                    rank_frozen = true;
                    result
                }
            }
        } else {
            #[derive(Clone, Copy)]
            enum FrozenAttempt {
                Certified {
                    rank: usize,
                    reuse: LinearSessionReuseReport,
                    route: FeSolverKind,
                    fully_certified: bool,
                },
                Fallback(FrozenLinearFallbackKind),
            }
            let frozen_attempt = match session.solve_ordinary_ppml() {
                Ok(output) if output.certified() => {
                    certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
                    accumulate_inner_iterations(
                        &mut total_inner_iterations,
                        output.response_absorption_report(),
                    )?;
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML MAP solve lacks solver diagnostics"))?
                        .observe(map_solver_observation(
                            output.response_absorption_report(),
                            PpmlSolverFrameKind::Ordinary,
                        ))?;
                    fill_candidate_eta_from_frozen_output(
                        &output,
                        offset,
                        workspace.candidate_eta_mut(),
                    )?;
                    fill_candidate_coefficients_from_frozen_output(
                        &output,
                        workspace.candidate_coefficients_mut(),
                    )?;
                    FrozenAttempt::Certified {
                        rank: output.basis().kept_generated_columns().len(),
                        reuse: output.reuse_report(),
                        route: output.response_absorption_report().used_solver,
                        fully_certified: output.certified(),
                    }
                }
                Ok(_) => FrozenAttempt::Fallback(FrozenLinearFallbackKind::Bread),
                Err(error) => match classify_frozen_linear_fallback(&error) {
                    Some(kind) => FrozenAttempt::Fallback(kind),
                    None => return Err(error),
                },
            };
            rank_frozen = true;
            if let FrozenAttempt::Certified {
                rank: frozen_rank,
                reuse: frozen_reuse,
                route: frozen_route,
                fully_certified: frozen_certified,
            } = frozen_attempt
            {
                if !allocation_audit.needs_equivalence() {
                    (frozen_rank, frozen_reuse, frozen_route)
                } else {
                    let frozen_objective = workspace
                        .evaluate_candidate_objective(&objective_inputs, config.ppml.bounds)?;
                    record_linear_phase_timings(&mut memory, frozen_reuse)?;
                    // This canonical same-frame solve is deliberately after the
                    // end allocator snapshot. It may allocate and therefore can
                    // validate warm/fresh science without contaminating the
                    // warmed steady-state window.
                    let canonical = session.solve_fresh()?;
                    accumulate_inner_iterations(
                        &mut total_inner_iterations,
                        canonical.frame().absorption_report(),
                    )?;
                    let solver = solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML MAP equivalence solve lacks diagnostics"))?;
                    if initial_checkpoint_lsmr {
                        record_checkpoint_solver_observation(
                            solver,
                            canonical.frame().absorption_report(),
                            false,
                        )?;
                    } else {
                        solver.observe(map_solver_observation(
                            canonical.frame().absorption_report(),
                            PpmlSolverFrameKind::Ordinary,
                        ))?;
                    }
                    let canonical_route = canonical.frame().absorption_report().used_solver;
                    let mut equivalent = frozen_certified
                        && frozen_route == FeSolverKind::Map
                        && (canonical_route == frozen_route
                            || (initial_checkpoint_lsmr && canonical_route == FeSolverKind::Lsmr))
                        && fresh_warm_linear_equivalent(
                            &canonical,
                            offset,
                            workspace.candidate_eta(),
                            workspace.candidate_coefficients(),
                            frozen_rank,
                            config.ppml,
                            inner_tolerance,
                        )?;
                    fill_candidate_eta_from_linear_output(
                        &canonical,
                        offset,
                        workspace.candidate_eta_mut(),
                    )?;
                    fill_candidate_coefficients_from_linear_output(
                        &canonical,
                        workspace.candidate_coefficients_mut(),
                    )?;
                    let canonical_objective = workspace
                        .evaluate_candidate_objective(&objective_inputs, config.ppml.bounds)?;
                    equivalent &= relative_scalar_difference(
                        frozen_objective.deviance,
                        canonical_objective.deviance,
                    ) <= fresh_warm_tolerance(config.ppml, inner_tolerance)
                        && relative_scalar_difference(
                            frozen_objective.log_likelihood,
                            canonical_objective.log_likelihood,
                        ) <= fresh_warm_tolerance(config.ppml, inner_tolerance);
                    let canonical_rank = canonical.frame().finite_rank();
                    let canonical_reuse = canonical.reuse_report();
                    let observation = allocation_audit.complete_equivalence(equivalent)?;
                    memory.observe_steady_state_allocation(observation)?;
                    (canonical_rank, canonical_reuse, canonical_route)
                }
            } else if let FrozenAttempt::Fallback(kind) = frozen_attempt {
                retry::canonical_retry(
                    retry::CanonicalRetryReason::Frozen(kind),
                    &mut direction,
                    &mut session,
                    &mut workspace,
                    offset,
                    frame_generation,
                    &mut memory,
                    &mut solver_report,
                    &mut certified_linear_subproblems,
                    &mut total_inner_iterations,
                    &mut allocation_audit,
                )?
            } else {
                unreachable!("frozen attempt is exhaustively classified")
            }
        };
        record_linear_phase_timings(&mut memory, reuse)?;
        begin_phase_allocation(
            config.phase_allocation_probe,
            PpmlPhase::ObjectiveLineSearch,
        );
        let line_search_clock = PpmlPhaseClock::start();
        let previous_objective = objective;
        let accepted = {
            let mut poll_line_search = || interrupt(PpmlInterruptPoint::IrlsLineSearchTrial);
            let attempt = if profiled_step.is_some() {
                let profile = direction.profile_mut().expect("prepared profile step");
                if config.linear.execution.mode == PreparedOlsExecutionMode::Low
                    || session.has_implicit_one_fe()
                {
                    let columns = session.one_fe_original_columns(profile.active_columns())?;
                    let data = ProfiledRawColumns::new(prepared, columns)?;
                    profile.accept(
                        &data,
                        session.fe_design(),
                        &objective_inputs,
                        offset,
                        &mut workspace,
                        objective,
                        config.ppml,
                        &mut poll_line_search,
                    )
                } else {
                    let (values, positions) =
                        session.one_fe_representatives(profile.active_columns())?;
                    profile.accept(
                        ProfiledColumns::selected(values, positions)?,
                        session.fe_design(),
                        &objective_inputs,
                        offset,
                        &mut workspace,
                        objective,
                        config.ppml,
                        &mut poll_line_search,
                    )
                }
            } else if let Some(curve) = profiled_curve.as_mut() {
                curve.accept(
                    session.fe_design(),
                    &objective_inputs,
                    &mut workspace,
                    objective,
                    config.ppml,
                    &mut poll_line_search,
                )
            } else {
                workspace.accept_candidate_interruptible(
                    &objective_inputs,
                    objective,
                    config.ppml,
                    &mut poll_line_search,
                )
            };
            match attempt {
                Ok(accepted) => accepted,
                Err(error)
                    if matches!(purpose, PreparedPpmlIrlsPurpose::MuExploration { .. })
                        && error.code == ErrorCode::NonConvergence
                        && accepted_iterations > 0 =>
                {
                    memory
                        .record_phase(PpmlPhase::ObjectiveLineSearch, line_search_clock.elapsed());
                    record_phase_allocation(
                        &mut memory,
                        config.phase_allocation_probe,
                        PpmlPhase::ObjectiveLineSearch,
                    );
                    exploration_terminal = PreparedPpmlMuTerminal::LineSearchStalled;
                    break;
                }
                Err(error) => return Err(error),
            }
        };
        if let Some((joint, _)) = direction.lsmr_mut() {
            // The analytic FE shift changes the accepted increment. Discard
            // the LSMR history witness before solving under the new weights.
            if profiled_curve.is_some() {
                joint.reset_warm_start();
            } else {
                joint.accept_step(accepted.step);
            }
        }
        session.accept_pending_preconditioner();
        accepted_iterations = accepted_iterations.saturating_add(1);
        let increase = (accepted.objective.deviance - previous_objective.deviance).max(0.0)
            / previous_objective.deviance.abs().max(1.0);
        maximum_deviance_increase_relative = maximum_deviance_increase_relative.max(increase);
        total_step_halvings = total_step_halvings
            .checked_add(accepted.step_halvings)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared PPML step-halving count overflowed",
                )
            })?;
        let relative_change = relative_deviance_change(objective, accepted.objective);
        objective = accepted.objective;
        final_score = prepared_score_certificate(
            prepared,
            session.fe_design(),
            outcome,
            workspace.mean(),
            prior_weights,
            config.ppml.score_tolerance,
            &mut score_workspace,
        )?;
        final_score_generation = workspace.mean_generation();
        memory.record_phase(PpmlPhase::ObjectiveLineSearch, line_search_clock.elapsed());
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::ObjectiveLineSearch,
        );
        if let PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance } = purpose {
            // One pass: some zero-outcome mean is at or below the threshold
            // exactly when the smallest one is.
            let mut minimum_positive = f64::INFINITY;
            let mut minimum_zero = f64::INFINITY;
            let mut has_zero = false;
            for (y, mean) in outcome.iter().zip(workspace.mean()) {
                if *y > 0.0 {
                    minimum_positive = minimum_positive.min(*mean);
                } else if *y == 0.0 {
                    has_zero = true;
                    minimum_zero = minimum_zero.min(*mean);
                }
            }
            let reached = has_zero && minimum_zero <= mu_tolerance * minimum_positive;
            if reached {
                exploration_terminal = PreparedPpmlMuTerminal::CandidateThresholdReached;
                break;
            }
        }
        let iteration_converged = relative_change <= config.ppml.tolerance && final_score.passed;
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=irls_convergence|iteration={iteration}|deviance_change={relative_change:.17e}|score={:.17e}|passed={}|step={:.17e}",
                final_score.relative_infinity_norm,
                u8::from(iteration_converged),
                accepted.step,
            ));
        }
        strict_control.observe_accepted_iteration(iteration_converged);
        if retained_iterations.len() < retained_limit {
            retained_iterations.push(PreparedPpmlIterationReport {
                iteration,
                deviance: objective.deviance,
                log_likelihood: objective.log_likelihood,
                relative_deviance_change: relative_change,
                step: accepted.step,
                step_halvings: accepted.step_halvings,
                inner_tolerance,
                finite_rank,
                score: final_score.clone(),
                linear_reuse: reuse,
                joint_lsmr: joint_lsmr_frame,
            });
        }
        let audit_can_start = allocation_audit.can_start_after(
            iteration,
            config.ppml.max_iterations,
            reuse,
            linear_route,
        );
        let audit_defers_strict = allocation_audit.qualification_pending();
        if strict_control.may_enter(audit_defers_strict) {
            let outcome = controller::StrictCycle {
                candidate: controller::CandidatePhase {
                    prepared,
                    workspace: &mut workspace,
                    inputs: &objective_inputs,
                    outcome,
                    prior_weights,
                    offset,
                    config,
                    finite_rank,
                    objective: &mut objective,
                    total_inner_iterations: &mut total_inner_iterations,
                    total_step_halvings: &mut total_step_halvings,
                    score_workspace: &mut score_workspace,
                    final_score: &mut final_score,
                    final_score_generation: &mut final_score_generation,
                },
                session: &mut session,
                direction: &mut direction,
                curve: &mut profiled_curve,
                control: &mut strict_control,
                memory: &mut memory,
                solver: &mut solver_report,
                frame_generation: &mut frame_generation,
                previous_inner_tolerance: &mut previous_inner_tolerance,
                interrupt: &mut *interrupt,
                iteration,
                solver_admission,
                solver_memory_budget,
            }
            .run()?;
            match outcome {
                controller::StrictCycleOutcome::JointAccepted => break,
                controller::StrictCycleOutcome::CheckpointAccepted => {
                    exploration_terminal = PreparedPpmlMuTerminal::Converged;
                    break;
                }
                controller::StrictCycleOutcome::Resume => {}
            }
        }
        begin_phase_allocation(
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        let working_refresh_clock = PpmlPhaseClock::start();
        workspace.refresh_prepared_working_state(&objective_inputs, config.ppml.bounds)?;
        memory.record_phase(
            PpmlPhase::WeightResponseRefresh,
            working_refresh_clock.elapsed(),
        );
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        if allocation_audit.is_measuring() {
            let elapsed = audited_iteration_clock.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "PPML allocation audit iteration lacks its timing boundary",
                )
            })?;
            allocation_audit.finish_iteration(
                &session,
                &workspace,
                reuse,
                linear_route,
                elapsed.elapsed(),
                direction.profile(),
                profiled_curve.as_ref(),
            )?;
        } else if audit_can_start {
            allocation_audit.start_after(
                &session,
                &workspace,
                direction.profile(),
                profiled_curve.as_ref(),
            )?;
        }
        previous_outer_progress = Some(PpmlInnerToleranceProgress::new(
            relative_deviance_change(previous_objective, objective),
            final_score.relative_infinity_norm,
        ));
    }

    if matches!(purpose, PreparedPpmlIrlsPurpose::MuExploration { .. }) {
        let (_coefficient_values, eta, mean, _score_residuals) = workspace.into_final_buffers();
        let mean_reconstruction_relative = mean
            .iter()
            .zip(&eta)
            .map(|(left, eta)| relative_scalar_difference(*left, eta.exp()))
            .fold(0.0_f64, f64::max);
        let certificate = PreparedPpmlMuExplorationCertificate {
            accepted_iterations,
            certified_linear_subproblems,
            maximum_deviance_increase_relative,
            mean_reconstruction_relative,
            terminal: exploration_terminal,
            passed: accepted_iterations > 0
                && certified_linear_subproblems >= accepted_iterations
                && maximum_deviance_increase_relative
                    <= config.ppml.objective_slack + 64.0 * f64::EPSILON
                && mean_reconstruction_relative <= 64.0 * f64::EPSILON
                && eta.iter().all(|value| value.is_finite())
                && mean.iter().all(|value| value.is_finite() && *value > 0.0)
                && objective.deviance.is_finite()
                && objective.log_likelihood.is_finite(),
        };
        if !certificate.passed {
            return Err(numerical_nonconvergence(
                "prepared Mu exploratory trajectory did not certify",
            ));
        }
        return Ok(PreparedPpmlIrlsRun::MuExploration(Box::new(
            PreparedPpmlMuExploration {
                binding,
                mean,
                certificate,
            },
        )));
    }

    let accepted_checkpoint = strict_control.accepted_checkpoint().ok_or_else(|| {
        let strict_profile_rejections = strict_control.rejected_cycles();
        FeregError::new(ErrorCode::NonConvergence, format!(
            "prepared PPML failed to converge after {} iterations; deviance={:.16e}, score={:.3e}, strict_profile_rejections={strict_profile_rejections}",
            config.ppml.max_iterations, objective.deviance, final_score.relative_infinity_norm,
        ))
    })?;
    let final_output = session.output()?;
    if accepted_checkpoint.needs_observation() {
        record_checkpoint_solver_observation(
            solver_report
                .as_mut()
                .ok_or_else(|| internal("PPML final Hessian lacks solver diagnostics"))?,
            final_output.frame().absorption_report(),
            false,
        )?;
    }
    let coefficients = final_coefficients(&final_output, workspace.coefficients())?;
    let retained_memory = final_output.retained_within_memory();
    memory.observe_scratch(retained_memory.scratch_bytes);
    memory.observe_linear_route(final_output.frame().absorption_report().used_solver);
    memory.observe_diagnostic_records(retained_iterations.len())?;
    memory.certify_workspace(&workspace)?;
    workspace.refresh_score_residuals(outcome)?;
    let (_coefficient_values, eta, mean, score_residuals) = workspace.into_final_buffers();
    if bias.is_some() {
        validate_final_hessian_frame(session.bias_weights(), prior_weights, &mean)?;
    }
    let bias = match bias {
        Some((request, units)) => Some(super::bias::compute(
            &mut session,
            request,
            super::bias::AcceptedBiasFrame {
                units,
                y: outcome,
                mu: &mean,
                eta: &eta,
                offset,
            },
            config.ppml.tolerance,
            interrupt,
        )?),
        None => None,
    };
    let linear = session
        .into_current_products(&mut || interrupt(super::PpmlInterruptPoint::InferenceSubset))?;
    validate_final_hessian_frame(linear.weights(), prior_weights, &mean)?;
    let final_generations = linear.generations();
    if final_generations.sample != model_generation
        || final_generations.topology != model_generation
        || final_generations.response != frame_generation
        || final_generations.weights != frame_generation
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML final linear products have stale frame generations",
        ));
    }
    Ok(PreparedPpmlIrlsRun::Publication(Box::new(
        PreparedPpmlIrlsFit {
            binding,
            bias,
            linear,
            coefficients,
            eta,
            mean,
            score_residuals,
            objective,
            diagnostics: PreparedPpmlIrlsDiagnostics {
                converged: true,
                two_consecutive_convergence: true,
                rank_frozen,
                iterations: completed_iterations,
                total_inner_iterations,
                retained_iterations,
                total_step_halvings,
                strict_profile_rejections: strict_control.rejected_cycles(),
                final_score,
                solver: solver_report.ok_or_else(|| {
                    internal("prepared PPML publication lacks solver diagnostics")
                })?,
                memory,
            },
        },
    )))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PublicSolverMemoryAdmission {
    incremental_bytes: u128,
    owned_fe_design_shortfall_bytes: u128,
    cmg_batch_capacity: usize,
    checkpoint_workspace_bytes: u128,
}

fn record_cmg_heap_forecast(
    memory: &mut PpmlMemoryTelemetry,
    cmg: &ReusableCategoricalFwlCmg,
    admission: PublicSolverMemoryAdmission,
) -> Result<()> {
    let peak = cmg
        .expected_heap_peak(admission.checkpoint_workspace_bytes)?
        .with_retained(admission.owned_fe_design_shortfall_bytes)?;
    memory.expected_solver_workspace_peak = Some(
        memory
            .expected_solver_workspace_peak
            .unwrap_or(crate::memory::SolverWorkspacePeak::ZERO)
            .max(peak),
    );
    Ok(())
}

fn public_solver_memory_admission<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    memory_plan: PpmlMemoryPlan,
    config: &PreparedPpmlIrlsConfig,
    rhs: usize,
    purpose: PreparedPpmlIrlsPurpose,
) -> Result<PublicSolverMemoryAdmission> {
    let solver = config.solver_plan.selected;
    if matches!(
        solver,
        super::PpmlSolverFamily::Auto | super::PpmlSolverFamily::Map
    ) || matches!(purpose, PreparedPpmlIrlsPurpose::MuExploration { .. })
    {
        return Ok(PublicSolverMemoryAdmission::default());
    }
    let design = prepared.fit_borrowed_cmg_design()?;
    let dimensions = memory_plan.dimensions;
    let admitted_rhs = dimensions
        .finite_columns
        .checked_add(1)
        .ok_or_else(|| allocation("admitted PPML FE solver RHS width overflows"))?;
    if design.nobs() != dimensions.observations
        || design.effects().len() != dimensions.fe_dimensions
        || rhs != admitted_rhs
        || config.linear.execution.threads.resolved() != dimensions.threads
    {
        return Err(internal(
            "public PPML solver preflight does not match its admitted immutable dimensions",
        ));
    }
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(design.effects().len())
        .map_err(|_| allocation("unable to reserve PPML FE solver preflight levels"))?;
    levels.extend(design.effects().iter().map(|effect| effect.n_levels()));
    let total_levels = levels.iter().try_fold(0_usize, |total, levels| {
        total
            .checked_add(*levels)
            .ok_or_else(|| allocation("PPML FE solver level count overflows"))
    })?;
    if total_levels != dimensions.fe_total_levels
        || dimensions.fe_total_channels != dimensions.fe_dimensions
        || dimensions.fe_max_channels != usize::from(!levels.is_empty())
        || levels.iter().copied().max().unwrap_or(0) != dimensions.fe_max_levels
    {
        return Err(internal(
            "public PPML solver topology does not match its admitted FE dimensions",
        ));
    }
    let owned_fe_design_peak =
        conservative_owned_intercept_fe_design_peak_bytes(design.nobs(), &levels)?;
    let owned_fe_design_shortfall_bytes = owned_fe_design_peak
        .saturating_sub(memory_plan.selected_inventory().fe_solver_topology_bytes);
    let mut cmg_batch_capacity = 0;
    let route_workspace = match solver {
        super::PpmlSolverFamily::Lsmr => {
            let active_columns = rhs.saturating_sub(1);
            estimate_joint_lsmr_workspace_bytes(
                dimensions.observations,
                active_columns,
                &levels,
                config.linear.execution.threads.resolved(),
                joint_lsmr_policy(levels.len()),
            )?
        }
        super::PpmlSolverFamily::Cmg => {
            let fused_workspace = estimate_fused_cmg_workspace_bytes(
                dimensions.observations,
                rhs.saturating_sub(1),
                config.linear.execution.threads.resolved(),
            )?;
            let profile = CmgTopologyProfile::analyze(&design)?;
            let fused_workspace = fused_workspace
                .checked_add(crate::fe::CmgControlSeed::estimated_bytes(
                    profile.vertices(),
                    rhs.saturating_sub(1),
                )?)
                .ok_or_else(|| allocation("prepared PPML CMG seed workspace overflows"))?;
            if profile.nuisance_columns() > CMG_MAX_NUISANCE_COLUMNS {
                return Err(FeregError::new(
                    ErrorCode::Unsupported,
                    format!(
                        "solver(cmg) supports at most {CMG_MAX_NUISANCE_COLUMNS} non-base nuisance columns; found {}",
                        profile.nuisance_columns()
                    ),
                ));
            }
            cmg_batch_capacity =
                fused_cmg_batch_size(config.memory.poolsize, rhs, profile.nuisance_columns());
            let cmg_config = CmgConfig {
                map: config.ppml.absorb.map,
                threads: config.linear.execution.threads,
                batch_size: cmg_batch_capacity,
                retain_coefficients: false,
                requested_solver: FeSolverKind::Cmg,
                components: crate::fe::explicit_cmg_components(FeSolverKind::Cmg),
                solver_memory_budget: None,
            };
            let topology_prepare = PreparedCmgTopology::conservative_prepare_peak_bytes(&design)?;
            let retained_frame =
                PreparedCmgTopology::conservative_retained_frame_peak_bytes(&design, cmg_config)?;
            let steady_peak = fused_workspace
                .checked_add(retained_frame)
                .ok_or_else(|| allocation("prepared PPML CMG steady-state peak overflows"))?;
            let complete_peak = topology_prepare.max(steady_peak);
            // Observation only: saturating sums never fail, so test and
            // shipped builds take the same error paths.
            #[cfg(test)]
            {
                let topology_only = fused_workspace
                    .saturating_add(topology_prepare)
                    .saturating_add(owned_fe_design_shortfall_bytes);
                let complete_incremental =
                    complete_peak.saturating_add(owned_fe_design_shortfall_bytes);
                CMG_EARLY_MEMORY_PEAKS.set(Some((topology_only, complete_incremental)));
            }
            complete_peak
        }
        super::PpmlSolverFamily::Auto | super::PpmlSolverFamily::Map => 0,
    };
    // Initial basis freezing, strict convergence, and final-Hessian
    // synchronization execute serially with the same maximum full width.
    // The existing full-width checkpoint peak therefore admits the initial
    // accelerator exactly once; summing three mutually exclusive workspaces
    // would overstate the fit-lifetime peak.
    let checkpoint_workspace =
        if use_checkpoint_lsmr_accelerator(dimensions.observations, levels.len()) {
            estimate_checkpoint_lsmr_workspace_bytes(
                dimensions.observations,
                rhs,
                &levels,
                config.linear.execution.threads.resolved(),
            )?
        } else {
            0
        };
    Ok(PublicSolverMemoryAdmission {
        incremental_bytes: route_workspace
            .checked_add(checkpoint_workspace)
            .ok_or_else(|| allocation("public PPML checkpoint memory admission overflows"))?
            .checked_add(owned_fe_design_shortfall_bytes)
            .ok_or_else(|| allocation("public PPML solver admission memory overflows"))?,
        owned_fe_design_shortfall_bytes,
        cmg_batch_capacity,
        checkpoint_workspace_bytes: checkpoint_workspace,
    })
}

fn conservative_owned_intercept_fe_design_peak_bytes(
    rows: usize,
    levels: &[usize],
) -> Result<u128> {
    let n = rows as u128;
    let effects = levels.len() as u128;
    let total_levels = levels.iter().try_fold(0_u128, |total, levels| {
        total
            .checked_add(*levels as u128)
            .ok_or_else(|| allocation("owned PPML FE level count overflows"))
    })?;
    let maximum_levels = levels.iter().copied().max().unwrap_or(0) as u128;
    let word = std::mem::size_of::<usize>() as u128;
    let retained = effects
        .checked_mul(std::mem::size_of::<crate::fe::EffectTerm>() as u128)
        .and_then(|value| {
            value.checked_add(
                n.checked_mul(effects)?
                    .checked_mul(std::mem::size_of::<u32>() as u128)?,
            )
        })
        .and_then(|value| value.checked_add(n.checked_mul(effects)?.checked_mul(word)?))
        .and_then(|value| value.checked_add(total_levels.checked_add(effects)?.checked_mul(word)?))
        .ok_or_else(|| allocation("owned PPML FE topology estimate overflows"))?;
    // LevelIndex construction holds one count and one cursor for every level
    // beside the retained codes, offsets, and memberships of the current and
    // previously completed effects.
    retained
        .checked_add(
            maximum_levels
                .checked_mul(2)
                .and_then(|value| value.checked_mul(word))
                .ok_or_else(|| allocation("owned PPML FE construction scratch overflows"))?,
        )
        .ok_or_else(|| allocation("owned PPML FE construction peak overflows"))
}

fn map_solver_observation(
    report: &SolveReport,
    frame: PpmlSolverFrameKind,
) -> PpmlSolverObservation {
    PpmlSolverObservation {
        engine: PpmlSolverEngine::Map,
        frame,
        retries: usize::from(report.fallback_used),
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: Some(report.certificate.absolute_infinity_norm),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: Some(report.normal_equation_residual),
        projection_absolute_infinity_norm: None,
        backward_error: None,
    }
}

fn checkpoint_solver_observation(report: &SolveReport) -> Result<PpmlSolverObservation> {
    let engine = match report.used_solver {
        FeSolverKind::Map => PpmlSolverEngine::Map,
        FeSolverKind::Lsmr => PpmlSolverEngine::LsmrFused,
        _ => {
            return Err(internal(
                "PPML checkpoint projection used an unsupported solver engine",
            ));
        }
    };
    Ok(PpmlSolverObservation {
        engine,
        frame: PpmlSolverFrameKind::Checkpoint,
        retries: report.checkpoint_retries + usize::from(report.fallback_used),
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: Some(report.certificate.absolute_infinity_norm),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: Some(report.normal_equation_residual),
        projection_absolute_infinity_norm: None,
        backward_error: None,
    })
}

fn record_checkpoint_solver_observation(
    solver: &mut PpmlSolverReport,
    report: &SolveReport,
    profile_refinement: bool,
) -> Result<()> {
    let mut observation = checkpoint_solver_observation(report)?;
    observation.retries = observation
        .retries
        .checked_add(usize::from(profile_refinement))
        .ok_or_else(|| internal("PPML checkpoint refinement count overflowed"))?;
    if report.requested_solver == FeSolverKind::Lsmr
        && report.used_solver == FeSolverKind::Map
        && report.fallback_used
    {
        solver.record_checkpoint_lsmr_fallback(observation)
    } else {
        solver.observe(observation)
    }
}

fn joint_solver_observation(
    report: JointLsmrReport,
    engine: PpmlSolverEngine,
) -> PpmlSolverObservation {
    debug_assert!(matches!(
        engine,
        PpmlSolverEngine::LsmrFused | PpmlSolverEngine::LsmrJoint
    ));
    PpmlSolverObservation {
        engine,
        frame: PpmlSolverFrameKind::Ordinary,
        retries: report.retries,
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: Some(
            report
                .certificate
                .finite_absolute_infinity_norm
                .max(report.certificate.fixed_effect_absolute_infinity_norm),
        ),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: (engine == PpmlSolverEngine::LsmrJoint)
            .then_some(report.recurrence_residual),
        projection_absolute_infinity_norm: None,
        backward_error: None,
    }
}

fn cmg_solver_observation(report: FusedFwlCmgReport) -> PpmlSolverObservation {
    PpmlSolverObservation {
        engine: PpmlSolverEngine::CmgPrepared,
        frame: PpmlSolverFrameKind::Ordinary,
        retries: report.retried_right_hand_sides,
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.finish_seconds,
        absolute_infinity_norm: Some(
            report
                .certificate
                .finite_absolute_infinity_norm
                .max(report.certificate.fixed_effect_absolute_infinity_norm),
        ),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: None,
        projection_absolute_infinity_norm: Some(report.projection_absolute_infinity_norm),
        backward_error: Some(report.backward_error),
    }
}

fn cmg_failure_observation(report: FusedFwlCmgFailureReport) -> PpmlSolverObservation {
    PpmlSolverObservation {
        engine: PpmlSolverEngine::CmgPrepared,
        frame: PpmlSolverFrameKind::Ordinary,
        retries: report.retries,
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: report.absolute_infinity_norm,
        relative_infinity_norm: report.relative_infinity_norm,
        relative_two_norm: report.relative_two_norm,
        normal_equation: None,
        projection_absolute_infinity_norm: report.projection_absolute_infinity_norm,
        backward_error: report.backward_error,
    }
}

fn accumulate_inner_iterations(total: &mut usize, report: &SolveReport) -> Result<()> {
    *total = total.checked_add(report.iterations.max(1)).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "prepared PPML inner-iteration count overflowed",
        )
    })?;
    Ok(())
}

fn validate_inputs<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
) -> Result<()> {
    let rows = prepared.fit_sample().n_active();
    if rows == 0 || outcome.len() != rows || prior_weights.len() != rows || offset.len() != rows {
        return Err(invalid_argument(
            "prepared PPML inputs are empty or not aligned on the prepared sample",
        ));
    }
    if outcome
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(invalid_data(
            "prepared PPML outcomes must be finite and nonnegative",
        ));
    }
    if !outcome.iter().any(|value| *value > 0.0) {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "prepared PPML sample has no positive outcome",
        ));
    }
    if prior_weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
        || offset.iter().any(|value| !value.is_finite())
    {
        return Err(invalid_data(
            "prepared PPML weights must be finite and positive and offsets finite",
        ));
    }
    if !prepared.fit_structural_intercept() {
        return Err(invalid_argument(
            "prepared PPML requires its structural constant direction",
        ));
    }
    config.ppml.bounds.validate()?;
    let valid = finite_unit(config.ppml.fixed_effect_collinearity_tolerance)
        && finite_unit(config.ppml.tolerance)
        && finite_unit(config.ppml.score_tolerance)
        && finite_unit(config.ppml.inner_tolerance_initial)
        && finite_unit(config.ppml.inner_tolerance_final)
        && config.ppml.inner_tolerance_initial >= config.ppml.inner_tolerance_final
        && config.ppml.max_iterations > 0
        && config.ppml.minimum_step.is_finite()
        && config.ppml.minimum_step > 0.0
        && config.ppml.minimum_step <= 1.0
        && config.ppml.objective_slack.is_finite()
        && config.ppml.objective_slack >= 0.0;
    if !valid {
        return Err(invalid_argument(
            "prepared PPML numerical configuration is invalid",
        ));
    }
    validate_solver_admission(config)?;
    Ok(())
}

fn validate_memory_dimensions<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    dimensions: PpmlMemoryDimensions,
    config: PreparedPpmlIrlsConfig,
) -> Result<()> {
    let expected_threads = config.linear.execution.threads.resolved();
    if dimensions.observations != prepared.fit_sample().n_active()
        || dimensions.finite_columns != prepared.fit_model().columns.len()
        || dimensions.coefficient_slots != prepared.fit_model().coefficients.len()
        || dimensions.threads != expected_threads
    {
        return Err(invalid_argument(
            "prepared PPML memory dimensions do not match the final prepared generation",
        ));
    }
    Ok(())
}

fn next_frame_generation(current: u64) -> Result<u64> {
    current.checked_add(1).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "prepared PPML weighted-linear generation overflowed",
        )
    })
}

fn refresh_linear_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    generation: u64,
) -> Result<()> {
    session.refresh_response(workspace.working_response(), generation)?;
    session.refresh_weights(
        BorrowedMapWeights::Provided(workspace.hessian_weights()),
        ppml_bread_weight_scale(workspace.hessian_weights())?,
        generation,
    )?;
    #[cfg(test)]
    CANONICAL_FRAME_REFRESH_COUNT.set(CANONICAL_FRAME_REFRESH_COUNT.get().saturating_add(1));
    Ok(())
}

fn refresh_final_linear_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    generation: u64,
) -> Result<bool> {
    let reused_identical_weights = session.refresh_frame(
        workspace.working_response(),
        BorrowedMapWeights::Provided(workspace.hessian_weights()),
        ppml_bread_weight_scale(workspace.hessian_weights())?,
        generation,
    )?;
    #[cfg(test)]
    CANONICAL_FRAME_REFRESH_COUNT.set(CANONICAL_FRAME_REFRESH_COUNT.get().saturating_add(1));
    Ok(reused_identical_weights)
}

/// Rebuild the reusable linear products on the accepted PPML Hessian without
/// advancing the Newton map. Projecting the accepted linear predictor itself
/// leaves the accepted beta/eta state fixed while strictly recertifying the
/// within design and bread under the exact accepted `q * mu` weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalHessianSynchronization {
    Certified,
    ResumeWithCandidate,
    ResumeAcceptedState,
}

/// Joint Newton witness followed by canonical predictor finalization. This
/// remains necessary when canonical Newton reconstruction moves the point.
#[allow(clippy::too_many_arguments)]
fn try_joint_strict_final_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &mut PpmlIterationWorkspace,
    objective_inputs: &PreparedPoissonInputs<'_>,
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    checkpoint_tolerance: f64,
    frame_generation: &mut u64,
    total_inner_iterations: &mut usize,
    expected_rank: usize,
    accepted_objective: PoissonObjective,
    solver_report: &mut PpmlSolverReport,
    mut categorical: Option<(&mut ReusableCategoricalJointLsmr, PpmlSolverEngine)>,
) -> Result<bool> {
    if JOINT_FINAL_TEST_MODE.get() == 1
        || categorical.is_some()
            && (STRICT_RESUME_INJECTION.get() != 0
                || STRICT_BASIS_CHANGE_INJECTION.get() != 0
                || REPEATED_STRICT_RESUME_INJECTION.get() != 0)
    {
        // Existing injections explicitly exercise canonical checkpoint/resume.
        // Shared witness refusal and rollback have their own modes below.
        return Ok(false);
    }
    #[cfg(test)]
    JOINT_FINAL_TEST_COUNTS.set((
        JOINT_FINAL_TEST_COUNTS.get().0 + 1,
        JOINT_FINAL_TEST_COUNTS.get().1,
    ));
    workspace.refresh_prepared_working_state(objective_inputs, config.ppml.bounds)?;
    if categorical.is_none() {
        *frame_generation = next_frame_generation(*frame_generation)?;
        refresh_linear_frame(session, workspace, *frame_generation)?;
    }
    session.set_config(ppml_linear_config(config, checkpoint_tolerance)?)?;
    let rank = if let Some((joint, engine)) = categorical.as_mut() {
        let result = joint.solve(
            session.fe_design(),
            objective_inputs.outcome(),
            workspace.mean(),
            workspace.hessian_weights(),
            checkpoint_tolerance,
            config.linear.absorb.lsmr.max_iterations,
        );
        let result = if JOINT_FINAL_TEST_MODE.get() == 4 {
            JOINT_FINAL_TEST_MODE.set(0);
            Err(FeregError::new(
                ErrorCode::NonConvergence,
                "injected strict LSMR witness refusal",
            ))
        } else {
            result
        };
        let report = match result {
            Ok(report) => report,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                joint.reset_warm_start();
                if crate::diagnostics::solver_trace_enabled() {
                    crate::diagnostics::emit_solver_trace(
                        "FEREG_SOLVER_TRACE|phase=joint_strict_direction|refusal=lsmr_nonconvergence",
                    );
                }
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        *total_inner_iterations = total_inner_iterations
            .checked_add(report.iterations.max(1))
            .ok_or_else(|| internal("strict LSMR iteration count overflowed"))?;
        let mut observation = joint_solver_observation(report, *engine);
        observation.frame = PpmlSolverFrameKind::Checkpoint;
        solver_report.observe(observation)?;
        #[cfg(test)]
        if *engine == PpmlSolverEngine::LsmrFused {
            FUSED_STRICT_WITNESS_COUNT.set(FUSED_STRICT_WITNESS_COUNT.get() + 1);
        }
        joint.fill_candidate(workspace)?;
        joint.active_rank()
    } else {
        let direction = match session.solve_ordinary_ppml() {
            Ok(output) => output,
            Err(error) if classify_frozen_linear_fallback(&error).is_some() => return Ok(false),
            Err(error) => return Err(error),
        };
        if !matches!(direction, PpmlOrdinaryLinearOutput::Joint(_)) || !direction.certified() {
            return Ok(false);
        }
        accumulate_inner_iterations(
            total_inner_iterations,
            direction.response_absorption_report(),
        )?;
        record_checkpoint_solver_observation(
            solver_report,
            direction.response_absorption_report(),
            false,
        )?;
        fill_candidate_eta_from_frozen_output(&direction, offset, workspace.candidate_eta_mut())?;
        fill_candidate_coefficients_from_frozen_output(
            &direction,
            workspace.candidate_coefficients_mut(),
        )?;
        direction.basis().kept_generated_columns().len()
    };
    // Categorical LSMR updates identified finite coordinates. Its inherited
    // absorbed intercept is a normalization coordinate, as on the canonical
    // specialized path; the final canonical fit independently normalizes it.
    let coefficient_slots = categorical.as_ref().map(|(joint, _)| joint.active_slots());
    if JOINT_FINAL_TEST_MODE.get() == 2 {
        JOINT_FINAL_TEST_MODE.set(0);
        inject_candidate_profile_movement(workspace);
    }
    let objective =
        match workspace.evaluate_candidate_objective(objective_inputs, config.ppml.bounds) {
            Ok(value) => Some(value),
            Err(error) if error.code == ErrorCode::NonConvergence => None,
            Err(error) => return Err(error),
        };
    let consistent = strict_candidate_decision_with_coefficient_slots(
        true,
        true,
        expected_rank,
        rank,
        workspace.eta(),
        workspace.coefficients(),
        accepted_objective,
        workspace.candidate_eta(),
        workspace.candidate_coefficients(),
        objective,
        config.ppml,
        coefficient_slots,
    ) == FinalHessianSynchronization::Certified;
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=joint_strict_direction|consistent={}|eta_change={:.17e}|beta_change={:.17e}",
            u8::from(consistent),
            relative_infinity_difference(workspace.eta(), workspace.candidate_eta()),
            relative_infinity_difference(
                workspace.coefficients(),
                workspace.candidate_coefficients()
            )
        ));
    }
    if !consistent {
        return Ok(false);
    }
    // Keep the accepted point. The strict direction is only a witness; the
    // canonical final rank/inverse below independently validates its basis on
    // these exact weights before either result can authorize publication.
    workspace.prepare_final_hessian_response(offset)?;
    *frame_generation = next_frame_generation(*frame_generation)?;
    if categorical.is_some() {
        // The external LSMR witness read these same workspace weights. It did
        // not project the session response, so refresh that response only now:
        // two unsolved refreshes would break its retained response-delta seed.
        refresh_linear_frame(session, workspace, *frame_generation)?;
    } else if !refresh_final_linear_frame(session, workspace, *frame_generation)? {
        return Ok(false);
    }
    let transaction = session.strict_candidate()?;
    let canonical = transaction.candidate();
    let unchanged = transaction.basis_unchanged();
    let unchanged = if JOINT_FINAL_TEST_MODE.get() == 3 {
        JOINT_FINAL_TEST_MODE.set(0);
        false
    } else {
        unchanged
    };
    accumulate_inner_iterations(
        total_inner_iterations,
        canonical.frame().absorption_report(),
    )?;
    fill_candidate_eta_from_linear_output(&canonical, offset, workspace.candidate_eta_mut())?;
    fill_candidate_coefficients_from_linear_output(
        &canonical,
        workspace.candidate_coefficients_mut(),
    )?;
    let objective =
        match workspace.evaluate_candidate_objective(objective_inputs, config.ppml.bounds) {
            Ok(value) => Some(value),
            Err(error) if error.code == ErrorCode::NonConvergence => None,
            Err(error) => return Err(error),
        };
    let accepted = strict_candidate_decision_with_coefficient_slots(
        canonical.certified(),
        unchanged,
        expected_rank,
        canonical.frame().finite_rank(),
        workspace.eta(),
        workspace.coefficients(),
        accepted_objective,
        workspace.candidate_eta(),
        workspace.candidate_coefficients(),
        objective,
        config.ppml,
        coefficient_slots,
    ) == FinalHessianSynchronization::Certified
        && relative_infinity_difference(workspace.eta(), workspace.candidate_eta())
            <= config.ppml.tolerance
        && coefficient_slots.map_or_else(
            || {
                relative_infinity_difference(
                    workspace.coefficients(),
                    workspace.candidate_coefficients(),
                )
            },
            |slots| {
                relative_infinity_difference_at_slots(
                    workspace.coefficients(),
                    workspace.candidate_coefficients(),
                    slots,
                )
            },
        ) <= config.ppml.tolerance;
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=joint_strict_final|accepted={}|basis_unchanged={}",
            u8::from(accepted),
            u8::from(unchanged)
        ));
    }
    if accepted {
        transaction.commit();
    } else {
        record_checkpoint_solver_observation(
            solver_report,
            canonical.frame().absorption_report(),
            false,
        )?;
        transaction.rollback();
    }
    #[cfg(test)]
    if accepted {
        JOINT_FINAL_TEST_COUNTS.set((
            JOINT_FINAL_TEST_COUNTS.get().0,
            JOINT_FINAL_TEST_COUNTS.get().1 + 1,
        ));
    }
    Ok(accepted)
}

/// Rank, within rows and inverse from the strict Newton WLS depend on its
/// exact weights, not on whether its RHS was the working response or predictor.
/// Recheck the accepted predictor against that canonical decomposition before
/// allowing those products to serve final inference without a second WLS.
fn reuse_canonical_final_products<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    generation: u64,
) -> Result<bool> {
    if PROFILE_FINAL_TEST_MODE.get() == 1 {
        return Ok(false);
    }
    let generation = if PROFILE_FINAL_TEST_MODE.get() == 2 {
        generation + 1
    } else {
        generation
    };
    let weights = workspace.hessian_weights();
    if !session.current_hessian_products_match(
        BorrowedMapWeights::Provided(weights),
        ppml_bread_weight_scale(weights)?,
        generation,
    )? {
        return Ok(false);
    }
    validate_final_hessian_frame(
        BorrowedMapWeights::Provided(weights),
        prior_weights,
        workspace.mean(),
    )?;
    let output = session.output()?;
    let tolerance = config
        .ppml
        .tolerance
        .min(config.ppml.score_tolerance)
        .min(1e-8);
    if offset.len() != workspace.eta().len()
        || output.finite_predictor().len() != offset.len()
        || output.absorbed_predictor().len() != offset.len()
    {
        return Err(internal("canonical final predictor is misaligned"));
    }
    let mut absorbed_scale = 0.0_f64;
    let mut working_scale = 0.0_f64;
    // Keep the legacy matrix indices and numerical traversal order explicit.
    #[allow(clippy::needless_range_loop)]
    for row in 0..offset.len() {
        absorbed_scale = absorbed_scale.max(output.absorbed_predictor()[row].abs());
        working_scale = working_scale
            .max(output.response()[row].abs())
            .max(output.frame().residuals()[row].abs())
            .max(output.finite_predictor()[row].abs());
        let candidate =
            offset[row] + (output.finite_predictor()[row] + output.absorbed_predictor()[row]);
        let candidate = if row == 0 && PROFILE_FINAL_TEST_MODE.get() == 3 {
            candidate + 0.01
        } else {
            candidate
        };
        if !candidate.is_finite()
            || relative_scalar_difference(workspace.eta()[row], candidate) > tolerance
        {
            return Ok(false);
        }
    }
    // A strict Newton response can be enormous when a fitted mean is tiny.
    // Its response-minus-residual decomposition can pass the outer predictor
    // check while losing the precision needed to recover separate FE channels.
    // Decline this shortcut when component rounding exceeds the strict inner
    // budget. The existing final-Hessian replay uses the small accepted
    // predictor as its RHS and retains all rank, score and frame checks.
    if absorbed_scale > 0.0
        && 8.0 * f64::EPSILON * working_scale > strict_inner_tolerance(config.ppml) * absorbed_scale
    {
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(
                "FEREG_SOLVER_TRACE|phase=profile_final_products|accepted=0|reason=working_response_cancellation",
            );
        }
        return Ok(false);
    }
    #[cfg(test)]
    PROFILE_FINAL_ACCEPTED.set(PROFILE_FINAL_ACCEPTED.get() + 1);
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(
            "FEREG_SOLVER_TRACE|phase=profile_final_products|accepted=1",
        );
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn synchronize_final_hessian_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &mut PpmlIterationWorkspace,
    objective_inputs: &PreparedPoissonInputs<'_>,
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    checkpoint_tolerance: f64,
    frame_generation: &mut u64,
    total_inner_iterations: &mut usize,
    expected_rank: usize,
    accepted_objective: PoissonObjective,
    coefficient_slots: Option<&[usize]>,
) -> Result<FinalHessianSynchronization> {
    workspace.refresh_prepared_working_state(objective_inputs, config.ppml.bounds)?;
    workspace.prepare_final_hessian_response(offset)?;
    *frame_generation = next_frame_generation(*frame_generation)?;
    // The accepted strict point usually changes the Hessian. Only a bitwise
    // identical weight frame retains its current weight generation and skips
    // the redundant retained-control projection; the final response is still
    // independently absorbed, solved, reconstructed, and certified below.
    let _reused_identical_weights =
        refresh_final_linear_frame(session, workspace, *frame_generation)?;
    let strict_tolerance = checkpoint_tolerance;
    session.set_config(ppml_linear_config(config, strict_tolerance)?)?;
    let synchronized_transaction = session.strict_candidate()?;
    let synchronized_basis_unchanged = synchronized_transaction.basis_unchanged();
    let synchronized_basis_unchanged = synchronized_basis_unchanged && !take_strict_basis_change(2);
    let (synchronized_certified, synchronized_rank) = {
        let synchronized = synchronized_transaction.candidate();
        let synchronized_certified = synchronized.certified();
        let synchronized_rank = synchronized.frame().finite_rank();
        accumulate_inner_iterations(
            total_inner_iterations,
            synchronized.frame().absorption_report(),
        )?;
        fill_candidate_eta_from_linear_output(
            &synchronized,
            offset,
            workspace.candidate_eta_mut(),
        )?;
        fill_candidate_coefficients_from_linear_output(
            &synchronized,
            workspace.candidate_coefficients_mut(),
        )?;
        (synchronized_certified, synchronized_rank)
    };
    if !synchronized_basis_unchanged || expected_rank != synchronized_rank {
        return Err(numerical_nonconvergence(
            "prepared PPML final Hessian changed the canonical rank basis",
        ));
    }
    let predictor_difference =
        relative_infinity_difference(workspace.eta(), workspace.candidate_eta());
    let coefficient_difference = coefficient_slots.map_or_else(
        || {
            relative_infinity_difference(
                workspace.coefficients(),
                workspace.candidate_coefficients(),
            )
        },
        |slots| {
            relative_infinity_difference_at_slots(
                workspace.coefficients(),
                workspace.candidate_coefficients(),
                slots,
            )
        },
    );
    let injected_movement = take_strict_resume(2);
    if injected_movement {
        inject_candidate_profile_movement(workspace);
    }
    let candidate_objective =
        match workspace.evaluate_candidate_objective(objective_inputs, config.ppml.bounds) {
            Ok(objective) => objective,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                synchronized_transaction.rollback();
                return Ok(FinalHessianSynchronization::ResumeAcceptedState);
            }
            Err(error) => return Err(error),
        };
    let decision = strict_candidate_decision_with_coefficient_slots(
        synchronized_certified,
        synchronized_basis_unchanged,
        expected_rank,
        synchronized_rank,
        workspace.eta(),
        workspace.coefficients(),
        accepted_objective,
        workspace.candidate_eta(),
        workspace.candidate_coefficients(),
        Some(candidate_objective),
        config.ppml,
        coefficient_slots,
    );
    let decision = if decision == FinalHessianSynchronization::Certified
        && (predictor_difference > config.ppml.tolerance
            || coefficient_difference > config.ppml.tolerance)
    {
        FinalHessianSynchronization::ResumeWithCandidate
    } else {
        decision
    };
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=final_hessian_decision|eta_change={predictor_difference:.17e}|beta_change={coefficient_difference:.17e}|decision={decision:?}",
        ));
    }
    if decision == FinalHessianSynchronization::Certified {
        synchronized_transaction.commit();
    } else {
        synchronized_transaction.rollback();
    }
    Ok(decision)
}

fn validate_final_hessian_frame(
    weights: BorrowedMapWeights<'_>,
    prior_weights: &[f64],
    mean: &[f64],
) -> Result<()> {
    if prior_weights.len() != mean.len() || prior_weights.is_empty() {
        return Err(invalid_argument(
            "prepared PPML final-Hessian evidence is not sample-aligned",
        ));
    }
    let supplied = match weights {
        BorrowedMapWeights::Provided(values) if values.len() == mean.len() => Some(values),
        BorrowedMapWeights::Provided(_) => {
            return Err(invalid_argument(
                "prepared PPML final-Hessian weights are not sample-aligned",
            ));
        }
        BorrowedMapWeights::Unit => None,
    };
    for row in 0..mean.len() {
        let expected = prior_weights[row] * mean[row];
        let actual = supplied.map_or(1.0, |values| values[row]);
        if !expected.is_finite() || expected <= 0.0 || !actual.is_finite() || actual <= 0.0 {
            return Err(numerical_nonconvergence(
                "prepared PPML final Hessian is not finitely representable",
            ));
        }
        if actual.to_bits() != expected.to_bits() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared PPML linear products are not on the accepted final Hessian",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn initialize<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    fe_design: &FeDesign,
    config: PreparedPpmlIrlsConfig,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
) -> Result<(Vec<f64>, Vec<f64>)> {
    if let Some(start) = start {
        validate_start(prepared, start, prior_weights, offset, fe_design, config)?;
        return Ok((
            fallible_copy(
                &start.coefficient_values,
                "unable to copy prepared PPML warm-start coefficients",
            )?,
            fallible_copy(
                &start.eta,
                "unable to copy prepared PPML warm-start predictor",
            )?,
        ));
    }
    let mut coefficient_values = fallible_filled(
        prepared.fit_model().coefficients.len(),
        0.0,
        "unable to reserve prepared PPML initial coefficients",
    )?;
    let mut intercept_position = None;
    if prepared.fit_report_intercept() {
        for (position, coefficient) in prepared.fit_model().coefficients.iter().enumerate() {
            let Some(generated) = coefficient.generated_column_index else {
                continue;
            };
            let column = prepared.fit_model().columns.get(generated).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared PPML intercept coefficient points outside the generated design",
                )
            })?;
            if matches!(column.kind, PreparedOlsColumnKind::Intercept) {
                if intercept_position.is_some() {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "prepared PPML model contains multiple reported structural constants",
                    ));
                }
                intercept_position = Some(position);
            }
        }
        if intercept_position.is_none() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared PPML reports a structural constant without an intercept slot",
            ));
        }
    }
    let mut eta = fallible_filled(
        outcome.len(),
        0.0,
        "unable to reserve prepared PPML initial predictor",
    )?;
    if purpose == PreparedPpmlIrlsPurpose::Publication
        && seed_multi_effect_predictor(
            outcome,
            prior_weights,
            offset,
            fe_design,
            config.ppml,
            &mut eta,
        )
    {
        // Absorbed intercepts represent the entire initial predictor. The
        // reported constant may start at zero; canonical solving establishes
        // its final normalization independently.
        return Ok((coefficient_values, eta));
    }
    let shift = if intercept_position.is_some() {
        log_weighted_outcome_sum(outcome, prior_weights)?
            - log_weighted_offset_sum(offset, prior_weights)?
    } else {
        0.0
    };
    if let Some(position) = intercept_position {
        coefficient_values[position] = shift;
    }
    if purpose == PreparedPpmlIrlsPurpose::Publication
        && seed_one_effect_predictor(
            outcome,
            prior_weights,
            offset,
            fe_design,
            config.ppml,
            &mut eta,
        )
    {
        return Ok((coefficient_values, eta));
    }
    // The first conditional intercept eliminates the global starting shift.
    // Compute that shift only when the complete provisional seed was refused;
    // overwriting every row also discards any partially completed sweep.
    let shift = if intercept_position.is_some() {
        shift
    } else {
        log_weighted_outcome_sum(outcome, prior_weights)?
            - log_weighted_offset_sum(offset, prior_weights)?
    };
    for row in 0..outcome.len() {
        eta[row] = offset[row] + shift;
        if !eta[row].is_finite() {
            return Err(numerical_nonconvergence(
                "prepared PPML initialization is not finitely representable",
            ));
        }
    }
    Ok((coefficient_values, eta))
}

fn seed_multi_effect_predictor(
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    design: &FeDesign,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    if design.effects().len() < 2
        || design
            .effects()
            .iter()
            .any(|effect| !effect.has_intercept())
    {
        return false;
    }
    // Solve the first partition directly from original offsets. Its exact
    // conditional optimum does not depend on a preceding global intercept.
    // Reuse the guarded one-partition arithmetic, including log-space fallback.
    if !seed_group_intercept_predictor(outcome, weights, offset, &design.effects()[0], config, eta)
    {
        return false;
    }
    // Start with exact conditional intercepts, then one safeguarded Newton
    // update per slope with that intercept profiled out. These model-space
    // initial guesses have no rank, convergence or final-Hessian authority.
    for effect in &design.effects()[1..] {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().members(level);
            let mut observed = CompensatedSum::default();
            let mut fitted = CompensatedSum::default();
            for &row in members {
                let y = weights[row] * outcome[row];
                let mu = weights[row] * eta[row].exp();
                if !y.is_finite()
                    || !mu.is_finite()
                    || mu <= 0.0
                    || (y == 0.0 && outcome[row] > 0.0)
                {
                    return false;
                }
                observed.add(y);
                fitted.add(mu);
            }
            let (observed, fitted) = (observed.total(), fitted.total());
            if !observed.is_finite() || observed <= 0.0 || !fitted.is_finite() || fitted <= 0.0 {
                return false;
            }
            let shift = observed.ln() - fitted.ln();
            for &row in members {
                let value = eta[row] + shift;
                if !usable_seed_value(value, config) {
                    return false;
                }
                eta[row] = value;
            }
        }
    }
    // Establish all intercepts before slopes can explain another partition's
    // intercept variation. The second stage profiles the current group mass
    // afresh: later partition updates may have moved its conditional intercept.
    for effect in design
        .effects()
        .iter()
        .filter(|effect| !effect.slopes().is_empty())
    {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().members(level);
            let mut observed = CompensatedSum::default();
            for &row in members {
                observed.add(weights[row] * outcome[row]);
            }
            for slope in effect.slopes() {
                if !seed_profiled_slope(
                    members,
                    slope,
                    outcome,
                    weights,
                    observed.total(),
                    config,
                    eta,
                ) {
                    return false;
                }
            }
        }
    }
    true
}

// Profile the group intercept exactly for each proposed slope step. Other
// coefficients remain fixed; only a nondecreasing finite group Poisson
// objective permits a provisional update, including the intercept correction.
fn seed_profiled_slope(
    members: &[usize],
    slope: &[f64],
    outcome: &[f64],
    weights: &[f64],
    observed: f64,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    let scale = members
        .iter()
        .map(|&r| slope[r].abs())
        .fold(0.0_f64, f64::max);
    let constant = members.iter().all(|&r| slope[r] == slope[members[0]]);
    let scale = if scale == 0.0 { 1.0 } else { scale };
    let mut target = CompensatedSum::default();
    let mut first = CompensatedSum::default();
    let mut mass = CompensatedSum::default();
    for &r in members {
        let z = slope[r] / scale;
        let wmu = weights[r] * eta[r].exp();
        target.add(weights[r] * outcome[r] * z);
        first.add(wmu * z);
        mass.add(wmu);
    }
    let center = first.total() / mass.total();
    let mut curvature = CompensatedSum::default();
    for &r in members {
        let z = slope[r] / scale - center;
        curvature.add(weights[r] * eta[r].exp() * z * z);
    }
    let score = target.total() - observed * center;
    let curvature = curvature.total() * (observed / mass.total());
    if !score.is_finite() || !curvature.is_finite() {
        return false;
    }
    // A constant slope has no profiled direction, but the current intercept
    // can still need its conditional correction after another partition moved.
    let mut step = if constant || curvature == 0.0 {
        0.0
    } else {
        score / curvature
    };
    if !step.is_finite() {
        return false;
    }
    for _ in 0..=config.max_step_halvings {
        // A scaled sum avoids overflow of the unnormalized trial means.
        let maximum = members
            .iter()
            .map(|&r| weights[r].ln() + eta[r] + step * (slope[r] / scale))
            .fold(f64::NEG_INFINITY, f64::max);
        let mut trial_mass = CompensatedSum::default();
        for &r in members {
            trial_mass.add((weights[r].ln() + eta[r] + step * (slope[r] / scale) - maximum).exp());
        }
        let shift = observed.ln() - (maximum + trial_mass.total().ln());
        let change = step * target.total() + observed * shift - (observed - mass.total());
        if change.is_finite()
            && change >= 0.0
            && members
                .iter()
                .all(|&r| usable_seed_value(eta[r] + step * (slope[r] / scale) + shift, config))
        {
            for &r in members {
                eta[r] += step * (slope[r] / scale) + shift;
            }
            return true;
        }
        step *= 0.5;
    }
    // A refused coordinate leaves this group unchanged. The seed remains a
    // finite model-space guess; full IRLS still establishes its own acceptance.
    true
}

fn seed_one_effect_predictor(
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    design: &FeDesign,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    let [effect] = design.effects() else {
        return false;
    };
    if !effect.has_intercept() || !effect.slopes().is_empty() {
        return false;
    }
    seed_group_intercept_predictor(outcome, weights, offset, effect, config, eta)
}

fn seed_group_intercept_predictor(
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    effect: &crate::fe::EffectTerm,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    // At beta=0 each one-way intercept has an analytic Poisson optimum.
    // Evaluate both sums in log space; this is an initialization only, before
    // canonical rank freezing or any accepted IRLS state exists. A failed
    // numerical seed is overwritten by the original global initialization.
    for level in 0..effect.n_levels() {
        let members = effect.level_index().members(level);
        if let Some(value) = constant_offset_group_log_mean(members, outcome, weights, offset) {
            if !usable_seed_value(value, config) {
                return false;
            }
            for &row in members {
                eta[row] = value;
            }
            continue;
        }
        let mut maximum_y = f64::NEG_INFINITY;
        let mut maximum_offset = f64::NEG_INFINITY;
        for &row in members {
            if outcome[row] > 0.0 {
                maximum_y = maximum_y.max(outcome[row].ln() + weights[row].ln());
            }
            maximum_offset = maximum_offset.max(offset[row] + weights[row].ln());
        }
        if !maximum_y.is_finite() || !maximum_offset.is_finite() {
            return false;
        }
        let mut sum_y = CompensatedSum::default();
        let mut sum_offset = CompensatedSum::default();
        for &row in members {
            if outcome[row] > 0.0 {
                sum_y.add((outcome[row].ln() + weights[row].ln() - maximum_y).exp());
            }
            sum_offset.add((offset[row] + weights[row].ln() - maximum_offset).exp());
        }
        let shift = maximum_y + sum_y.total().ln() - sum_offset.total().ln();
        for &row in members {
            let value = (offset[row] - maximum_offset) + shift;
            if !usable_seed_value(value, config) {
                return false;
            }
            eta[row] = value;
        }
    }
    true
}

fn constant_offset_group_log_mean(
    members: &[usize],
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
) -> Option<f64> {
    // A group-constant offset cancels analytically. Avoid per-row logarithms
    // and exponentials in this common case; nonrepresentable raw moments
    // return to the compensated log-sum-exp path before any predictor write.
    let &first = members.first()?;
    let mut numerator = CompensatedSum::default();
    let mut denominator = CompensatedSum::default();
    for &row in members {
        if offset[row] != offset[first] {
            return None;
        }
        let value = weights[row] * outcome[row];
        if !value.is_finite() || (value == 0.0 && outcome[row] > 0.0) {
            return None;
        }
        numerator.add(value);
        denominator.add(weights[row]);
    }
    let numerator = numerator.total();
    let denominator = denominator.total();
    if !numerator.is_finite() || numerator <= 0.0 || !denominator.is_finite() || denominator <= 0.0
    {
        return None;
    }
    let mean = numerator / denominator;
    (mean.is_finite() && mean > 0.0).then(|| mean.ln())
}

fn usable_seed_value(value: f64, config: PpmlConfig) -> bool {
    let mean = value.exp();
    value.is_finite()
        && value >= config.bounds.eta_min
        && value <= config.bounds.eta_max
        && mean.is_finite()
        && mean > 0.0
        && mean >= config.bounds.mu_min
        && mean <= config.bounds.mu_max
}

fn validate_start<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    start: &PreparedPpmlStart,
    prior_weights: &[f64],
    offset: &[f64],
    fe_design: &FeDesign,
    config: PreparedPpmlIrlsConfig,
) -> Result<()> {
    let rows = prepared.fit_sample().n_active();
    if start.coefficient_values.len() != prepared.fit_model().coefficients.len()
        || start.eta.len() != rows
    {
        return Err(invalid_argument(
            "prepared PPML warm start has inconsistent dimensions",
        ));
    }
    if start
        .coefficient_values
        .iter()
        .chain(&start.eta)
        .any(|value| !value.is_finite())
    {
        return Err(invalid_data("prepared PPML warm start must be finite"));
    }
    let mut means = fallible_filled(
        rows,
        0.0,
        "unable to reserve prepared PPML warm-start means",
    )?;
    means_from_eta(&start.eta, config.ppml.bounds, &mut means)?;
    let mut remainder = fallible_filled(
        rows,
        0.0,
        "unable to reserve prepared PPML warm-start remainder",
    )?;
    for row in 0..rows {
        let mut finite = 0.0_f64;
        for (slot, coefficient) in prepared
            .fit_model()
            .coefficients
            .iter()
            .zip(&start.coefficient_values)
        {
            if let Some(generated) = slot.generated_column_index {
                finite = coefficient.mul_add(prepared.fit_column_value(generated, row)?, finite);
            } else if *coefficient != 0.0 {
                return Err(invalid_argument(
                    "prepared PPML warm start assigns a syntax-omitted coefficient",
                ));
            }
        }
        remainder[row] = start.eta[row] - offset[row] - finite;
        if !remainder[row].is_finite() {
            return Err(invalid_data(
                "prepared PPML warm-start decomposition is nonfinite",
            ));
        }
    }
    let mut residualized = DenseMatrix::from_columns(&[remainder])?;
    let report = absorb_fixed_effects(
        fe_design,
        prior_weights,
        &mut residualized,
        strict_absorb(config.linear, config.ppml.inner_tolerance_final).absorb,
    )?;
    if !report.certified() {
        return Err(invalid_data(
            "prepared PPML warm-start FE decomposition did not certify",
        ));
    }
    let reference = start
        .eta
        .iter()
        .chain(offset)
        .fold(1.0_f64, |scale, value| scale.max(value.abs()));
    let residual = residualized
        .as_slice()
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    let tolerance = config
        .ppml
        .tolerance
        .max(config.ppml.inner_tolerance_final)
        .max(256.0 * f64::EPSILON);
    if residual <= tolerance * reference {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared PPML warm start is outside the prepared model span",
        ))
    }
}

fn ppml_linear_config(
    config: PreparedPpmlIrlsConfig,
    inner_tolerance: f64,
) -> Result<PreparedOlsFitConfig> {
    let mut linear = config.linear;
    linear.absorb = config.ppml.absorb;
    linear.coefficient_tolerance = Some(
        config
            .ppml
            .tolerance
            .min(config.ppml.score_tolerance)
            .min(1e-8),
    );
    if linear.absorb.solver == FeSolverKind::Auto {
        linear.absorb.solver = FeSolverKind::Map;
    }
    validate_solver_admission(config)?;
    linear.fixed_effect_collinearity_tolerance = config.ppml.fixed_effect_collinearity_tolerance;
    Ok(strict_absorb(linear, inner_tolerance))
}

fn validate_solver_admission(config: PreparedPpmlIrlsConfig) -> Result<()> {
    config.linear.absorb.validate_cmg_schur_options()?;
    config.ppml.absorb.validate_cmg_schur_options()?;
    if config.linear.absorb.solver != FeSolverKind::Map
        || config.ppml.absorb.solver != FeSolverKind::Map
        || config.solver_plan.selected == super::PpmlSolverFamily::Auto
    {
        return Err(invalid_argument(
            "prepared PPML canonical sessions require a MAP-configured fallback under a resolved public solver plan",
        ));
    }
    Ok(())
}

fn strict_absorb(mut config: PreparedOlsFitConfig, tolerance: f64) -> PreparedOlsFitConfig {
    config.absorb.map.tolerance = tolerance;
    config.absorb.lsmr.tolerance = tolerance;
    // The linear score is an inner-solve certificate. It may be no tighter
    // than the scheduled projection during optimization, while the strict
    // final solve resets this to publication tolerance.
    config.score_tolerance = config.score_tolerance.max(tolerance);
    config
}

fn ppml_bread_weight_scale(weights: &[f64]) -> Result<PreparedOlsBreadWeightScale> {
    let mut maximum = 0.0_f64;
    let mut sum = CompensatedSum::default();
    for weight in weights {
        if !weight.is_finite() || *weight <= 0.0 {
            return Err(numerical_nonconvergence(
                "prepared PPML Hessian weights must be finite and positive",
            ));
        }
        maximum = maximum.max(*weight);
        sum.add(*weight);
    }
    let reference_weight_sum = sum.total();
    if maximum <= 0.0 || !maximum.is_finite() || !reference_weight_sum.is_finite() {
        return Err(numerical_nonconvergence(
            "prepared PPML Hessian weight scale is not finite",
        ));
    }
    Ok(PreparedOlsBreadWeightScale {
        weight_kind: None,
        solver_weight_divisor: maximum,
        reference_weight_multiplier: maximum,
        reference_weight_sum,
    })
}

fn fill_candidate_eta_from_linear_output(
    output: &CertifiedLinearSessionOutput<'_>,
    offset: &[f64],
    eta: &mut [f64],
) -> Result<()> {
    if output.response().len() != offset.len()
        || output.frame().residuals().len() != offset.len()
        || output.finite_predictor().len() != offset.len()
        || output.absorbed_predictor().len() != offset.len()
        || eta.len() != offset.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML linear output has inconsistent dimensions",
        ));
    }
    for row in 0..offset.len() {
        // The canonical response and residual already identify the total
        // predictor. Splitting and recombining its FE part adds cancellation.
        eta[row] = offset[row] + (output.response()[row] - output.frame().residuals()[row]);
        if !eta[row].is_finite() {
            return Err(numerical_nonconvergence(
                "prepared PPML candidate predictor is not finite",
            ));
        }
    }
    Ok(())
}

fn fill_candidate_coefficients_from_linear_output(
    output: &CertifiedLinearSessionOutput<'_>,
    coefficients: &mut [f64],
) -> Result<()> {
    if coefficients.len() != output.coefficients().len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML linear coefficient output has inconsistent dimensions",
        ));
    }
    for (target, source) in coefficients.iter_mut().zip(output.coefficients()) {
        *target = source.value.unwrap_or(0.0);
    }
    Ok(())
}

fn fill_candidate_eta_from_frozen_output(
    output: &PpmlOrdinaryLinearOutput<'_>,
    offset: &[f64],
    eta: &mut [f64],
) -> Result<()> {
    if output.response().len() != offset.len()
        || output.within_residuals().len() != offset.len()
        || eta.len() != offset.len()
        || output.coefficients().len() != output.basis().kept_generated_columns().len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML frozen output has inconsistent dimensions",
        ));
    }
    for row in 0..offset.len() {
        eta[row] = offset[row] + output.response()[row] - output.within_residuals()[row];
        if !eta[row].is_finite() {
            return Err(numerical_nonconvergence(
                "prepared PPML frozen candidate predictor is not finite",
            ));
        }
    }
    Ok(())
}

fn fill_candidate_coefficients_from_frozen_output(
    output: &PpmlOrdinaryLinearOutput<'_>,
    coefficients: &mut [f64],
) -> Result<()> {
    if coefficients.len() != output.basis().slots().len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML frozen coefficient output has inconsistent dimensions",
        ));
    }
    coefficients.fill(0.0);
    if output.reported_coefficients().len() != coefficients.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML frozen reported coefficients are misaligned",
        ));
    }
    for (target, source) in coefficients.iter_mut().zip(output.reported_coefficients()) {
        *target = source.unwrap_or(0.0);
    }
    Ok(())
}

fn record_linear_phase_timings(
    memory: &mut PpmlMemoryTelemetry,
    reuse: LinearSessionReuseReport,
) -> Result<()> {
    for (phase, seconds) in [
        (
            PpmlPhase::FeProjection,
            reuse.phase_timing.projection_seconds,
        ),
        (
            PpmlPhase::FiniteSolve,
            reuse.phase_timing.finite_solve_seconds,
        ),
    ] {
        let Some(seconds) = seconds else {
            continue;
        };
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared PPML linear phase timer is invalid",
            ));
        }
        memory.record_phase(phase, std::time::Duration::from_secs_f64(seconds));
    }
    for (phase, peak_bytes) in [
        (
            PpmlPhase::FeProjection,
            reuse.phase_allocation.projection_peak_bytes,
        ),
        (
            PpmlPhase::FiniteSolve,
            reuse.phase_allocation.finite_solve_peak_bytes,
        ),
    ] {
        if let Some(peak_bytes) = peak_bytes {
            memory.observe_phase_peak(phase, peak_bytes);
        }
    }
    Ok(())
}

fn record_phase_allocation(
    memory: &mut PpmlMemoryTelemetry,
    probe: Option<PpmlPhaseAllocatorProbe>,
    phase: PpmlPhase,
) {
    if let Some(bytes) = end_phase_allocation(probe, phase) {
        memory.observe_phase_peak(phase, bytes);
    }
}

fn fresh_warm_tolerance(config: PpmlConfig, inner_tolerance: f64) -> f64 {
    config
        .tolerance
        .max(64.0 * inner_tolerance)
        .max(512.0 * f64::EPSILON)
}

/// Same-frame equivalence evidence collected after the allocation window.
/// `solve_fresh` independently rebuilds the canonical current-frame solution
/// and the session rejects any disagreement with its frozen basis identity;
/// this comparison then covers every reported coefficient slot and the
/// complete finite-plus-absorbed predictor under the registered tolerance.
fn fresh_warm_linear_equivalent(
    fresh: &CertifiedLinearSessionOutput<'_>,
    offset: &[f64],
    frozen_eta: &[f64],
    frozen_coefficients: &[f64],
    frozen_rank: usize,
    config: PpmlConfig,
    inner_tolerance: f64,
) -> Result<bool> {
    if fresh.response().len() != offset.len()
        || fresh.frame().residuals().len() != offset.len()
        || fresh.finite_predictor().len() != offset.len()
        || fresh.absorbed_predictor().len() != offset.len()
        || frozen_eta.len() != offset.len()
        || fresh.coefficients().len() != frozen_coefficients.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML fresh/warm audit dimensions are inconsistent",
        ));
    }
    let tolerance = fresh_warm_tolerance(config, inner_tolerance);
    let mut predictor_difference = 0.0_f64;
    for row in 0..offset.len() {
        let value = offset[row] + (fresh.response()[row] - fresh.frame().residuals()[row]);
        if !value.is_finite() || !frozen_eta[row].is_finite() {
            return Ok(false);
        }
        predictor_difference = predictor_difference
            .max((value - frozen_eta[row]).abs() / value.abs().max(frozen_eta[row].abs()).max(1.0));
    }
    let mut coefficient_difference = 0.0_f64;
    for (fresh, frozen) in fresh.coefficients().zip(frozen_coefficients) {
        let value = fresh.value.unwrap_or(0.0);
        if !value.is_finite() || !frozen.is_finite() {
            return Ok(false);
        }
        coefficient_difference = coefficient_difference
            .max((value - frozen).abs() / value.abs().max(frozen.abs()).max(1.0));
    }
    Ok(fresh.certified()
        && fresh.frame().finite_rank() == frozen_rank
        && predictor_difference <= tolerance
        && coefficient_difference <= tolerance)
}

#[allow(clippy::too_many_arguments)]
fn strict_profile_consistent(
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    strict_eta: &[f64],
    strict_coefficients: &[f64],
    strict_objective: PoissonObjective,
    config: PpmlConfig,
    coefficient_slots: Option<&[usize]>,
) -> bool {
    // Publication may be stricter than the attainable inner-solve or machine
    // floor. In that case fail closed instead of silently weakening the
    // caller's requested outer/score tolerance.
    let tolerance = config.tolerance.min(config.score_tolerance).min(1e-8);
    strict_profile_difference(
        eta,
        coefficients,
        objective,
        strict_eta,
        strict_coefficients,
        strict_objective,
        coefficient_slots,
    ) <= tolerance
}

fn strict_profile_difference(
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    strict_eta: &[f64],
    strict_coefficients: &[f64],
    strict_objective: PoissonObjective,
    coefficient_slots: Option<&[usize]>,
) -> f64 {
    let differences = [
        relative_infinity_difference(eta, strict_eta),
        strict_coefficient_difference(coefficients, strict_coefficients, coefficient_slots),
        relative_scalar_difference(objective.deviance, strict_objective.deviance),
        relative_scalar_difference(objective.log_likelihood, strict_objective.log_likelihood),
    ];
    differences
        .into_iter()
        .fold(0.0_f64, |maximum, difference| {
            if difference.is_finite() {
                maximum.max(difference)
            } else {
                f64::INFINITY
            }
        })
}

fn strict_coefficient_difference(left: &[f64], right: &[f64], slots: Option<&[usize]>) -> f64 {
    if left.len() != right.len() {
        return f64::INFINITY;
    }
    let difference = |slot: usize| {
        let (Some(&left), Some(&right)) = (left.get(slot), right.get(slot)) else {
            return f64::INFINITY;
        };
        if !left.is_finite() || !right.is_finite() {
            return f64::INFINITY;
        }
        // The registered coefficient envelope has atol/rtol = 1e-3. With
        // the strict tolerance capped at 1e-8, this requires Newton movement
        // within one tenth of that envelope even near zero. Retain the old
        // scale as an upper bound so no coefficient gets a weaker check.
        let scale = crate::ols::coefficient_accuracy_scale(left, right);
        (left - right).abs() / scale
    };
    match slots {
        Some(slots) => slots.iter().copied().map(difference).fold(0.0, f64::max),
        None => (0..left.len()).map(difference).fold(0.0, f64::max),
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn strict_candidate_decision(
    certified: bool,
    basis_unchanged: bool,
    accepted_rank: usize,
    candidate_rank: usize,
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    candidate_eta: &[f64],
    candidate_coefficients: &[f64],
    candidate_objective: Option<PoissonObjective>,
    config: PpmlConfig,
) -> FinalHessianSynchronization {
    strict_candidate_decision_with_coefficient_slots(
        certified,
        basis_unchanged,
        accepted_rank,
        candidate_rank,
        eta,
        coefficients,
        objective,
        candidate_eta,
        candidate_coefficients,
        candidate_objective,
        config,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn strict_candidate_decision_with_coefficient_slots(
    certified: bool,
    basis_unchanged: bool,
    accepted_rank: usize,
    candidate_rank: usize,
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    candidate_eta: &[f64],
    candidate_coefficients: &[f64],
    candidate_objective: Option<PoissonObjective>,
    config: PpmlConfig,
    coefficient_slots: Option<&[usize]>,
) -> FinalHessianSynchronization {
    let Some(candidate_objective) = candidate_objective else {
        return FinalHessianSynchronization::ResumeAcceptedState;
    };
    if !certified {
        return FinalHessianSynchronization::ResumeAcceptedState;
    }
    if !basis_unchanged
        || accepted_rank != candidate_rank
        || !strict_profile_consistent(
            eta,
            coefficients,
            objective,
            candidate_eta,
            candidate_coefficients,
            candidate_objective,
            config,
            coefficient_slots,
        )
    {
        FinalHessianSynchronization::ResumeWithCandidate
    } else {
        FinalHessianSynchronization::Certified
    }
}

fn relative_deviance_change(old: PoissonObjective, new: PoissonObjective) -> f64 {
    (old.deviance - new.deviance).abs() / old.deviance.min(new.deviance).max(0.1)
}

fn relative_infinity_difference(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() {
        return f64::INFINITY;
    }
    left.iter()
        .zip(right)
        .map(|(left, right)| (left - right).abs() / left.abs().max(right.abs()).max(1.0))
        .fold(0.0_f64, f64::max)
}

fn relative_infinity_difference_at_slots(left: &[f64], right: &[f64], slots: &[usize]) -> f64 {
    if left.len() != right.len() || slots.iter().any(|slot| *slot >= left.len()) {
        return f64::INFINITY;
    }
    slots
        .iter()
        .map(|slot| {
            let left = left[*slot];
            let right = right[*slot];
            (left - right).abs() / left.abs().max(right.abs()).max(1.0)
        })
        .fold(0.0_f64, f64::max)
}

fn relative_scalar_difference(left: f64, right: f64) -> f64 {
    (left - right).abs() / left.abs().max(right.abs()).max(1.0)
}

fn final_coefficients(
    fit: &CertifiedLinearSessionOutput<'_>,
    values: &[f64],
) -> Result<Vec<PreparedOlsFittedCoefficient>> {
    if fit.coefficients().len() != values.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML final coefficient dimensions differ",
        ));
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(values.len())
        .map_err(|_| allocation("unable to reserve prepared PPML final coefficients"))?;
    for (coefficient, value) in fit.coefficients().zip(values) {
        result.push(PreparedOlsFittedCoefficient {
            coefficient_slot: coefficient.coefficient_slot,
            generated_column_index: coefficient.generated_column_index,
            status: coefficient.status,
            value: match coefficient.status {
                PreparedOlsFitStatus::Kept | PreparedOlsFitStatus::NormalizedAbsorbedIntercept => {
                    Some(*value)
                }
                PreparedOlsFitStatus::SyntaxOmitted
                | PreparedOlsFitStatus::AbsorbedInterceptDeferred
                | PreparedOlsFitStatus::CollinearWithFixedEffects
                | PreparedOlsFitStatus::NumericallyRankDeficient => None,
            },
        });
    }
    Ok(result)
}

fn prepared_score_certificate<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    fe_design: &FeDesign,
    outcome: &[f64],
    mean: &[f64],
    prior_weights: &[f64],
    tolerance: f64,
    workspace: &mut PpmlScoreWorkspace,
) -> Result<PpmlCertificate> {
    let rows = outcome.len();
    if rows != mean.len()
        || rows != prior_weights.len()
        || rows != prepared.fit_sample().n_active()
        || rows != fe_design.nobs()
        || !tolerance.is_finite()
        || tolerance <= 0.0
    {
        return Err(invalid_argument(
            "prepared PPML score-certificate inputs are inconsistent",
        ));
    }
    let mut scales = [(0.0, 0.0)];
    workspace.reduce_columns(rows, &mut scales, |_, range| {
        let mut residual_scale = CompensatedSum::default();
        let mut reference_scale = CompensatedSum::default();
        for row in range {
            let residual = outcome[row] - mean[row];
            let inverse_variance_weight = prior_weights[row] / mean[row];
            residual_scale.add(inverse_variance_weight * residual * residual);
            reference_scale.add(inverse_variance_weight * outcome[row] * outcome[row]);
        }
        Ok((residual_scale, reference_scale))
    })?;
    let score_scale = scales[0].0.sqrt().max(scales[0].1.sqrt());
    if !score_scale.is_finite() {
        return Err(numerical_nonconvergence(
            "prepared PPML score scale is not finite",
        ));
    }
    let mut finite_absolute = 0.0_f64;
    let mut fe_absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_square = CompensatedSum::default();
    let mut relative_count = 0_usize;
    let numeric = crate::ols::NumericOlsColumns::bind_score_means(
        prepared.fit_model(),
        prepared.fit_sample(),
    )?;
    let numeric = if FORCE_GENERIC_OUTER_SCORE.get() {
        None
    } else {
        numeric
    };
    let generic = FORCE_GENERIC_OUTER_SCORE.get();
    let mut numeric_moments = [(0.0_f64, 0.0_f64); crate::ols::MAX_BOUND_NUMERIC_COLUMNS];
    for column in 0..prepared.fit_model().columns.len() {
        if column % numeric_moments.len() == 0 {
            let width = numeric_moments
                .len()
                .min(prepared.fit_model().columns.len() - column);
            // Independently form each score from original rows, never within
            // controls or a cached Newton score. All access paths use the same
            // fixed leaves; this scalar bank remains separate from FE scratch.
            let result =
                workspace.reduce_columns(rows, &mut numeric_moments[..width], |local, range| {
                    let mut moment = CompensatedSum::default();
                    let mut square = CompensatedSum::default();
                    let mut add = |row: usize, basis: f64| {
                        moment.add(prior_weights[row] * (outcome[row] - mean[row]) * basis);
                        square.add(prior_weights[row] * mean[row] * basis * basis);
                        Ok(())
                    };
                    if let Some(numeric) = numeric.as_ref() {
                        let source = numeric.raw_columns()[column + local];
                        for row in range {
                            add(row, source.map_or(1.0, |values| values[row]))?;
                        }
                    } else if generic {
                        for row in range {
                            add(row, prepared.fit_column_value(column + local, row)?)?;
                        }
                    } else {
                        crate::ols::prepared_fit_view::visit_prepared_column_range(
                            prepared,
                            column + local,
                            range,
                            &mut add,
                        )?;
                    }
                    Ok((moment, square))
                });
            if let Err(error) = result {
                // Only accessor errors need canonical column/row replay; an
                // existing numerical failure must never be replaced by it.
                if matches!(
                    error.code,
                    ErrorCode::InvalidArgument | ErrorCode::InvalidData
                ) {
                    for failed_column in column..column + width {
                        for row in 0..rows {
                            prepared.fit_column_value(failed_column, row)?;
                        }
                    }
                }
                return Err(error);
            }
        }
        let (moment, basis_square) = numeric_moments[column % numeric_moments.len()];
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative_score(
            moment,
            basis_square.sqrt(),
            score_scale,
            &mut relative_infinity,
            &mut relative_square,
            &mut relative_count,
        );
    }
    workspace.fe_moments.fill(0.0);
    workspace.fe_basis_squares.fill(0.0);
    let moments = &mut workspace.fe_moments;
    let squares = &mut workspace.fe_basis_squares;
    let budget = workspace.team.threads();
    let parallel = fe_design.effects().iter().any(|effect| {
        crate::parallel::StagePlan::new(
            rows as u128 * effect.channels() as u128 * 2,
            effect.n_levels(),
            budget,
        )
        .lanes
            > 1
    });
    let mut accumulate_effects = || -> Result<()> {
        let mut effect_offset = 0_usize;
        for effect in fe_design.effects() {
            let directions = effect.n_levels().saturating_mul(effect.channels());
            let plan = crate::parallel::StagePlan::new(
                rows as u128 * effect.channels() as u128 * 2,
                effect.n_levels(),
                budget,
            );
            if plan.lanes > 1 {
                // A direction owns its two accumulators. Membership order is
                // original observation order, so independent groups/channels
                // can run together without reassociating either score sum.
                let mut contexts = vec![(); plan.lanes];
                crate::parallel::group_blocks(
                    crate::parallel::GroupBuffers {
                        first: &mut moments[effect_offset..effect_offset + directions],
                        first_stride: effect.channels(),
                        second: &mut squares[effect_offset..effect_offset + directions],
                        second_stride: effect.channels(),
                        scratch: &mut contexts,
                    },
                    effect.level_index().membership_offsets(),
                    0,
                    &|first, moments, squares, _| {
                        for (local, (moment, square)) in moments.iter_mut().zip(squares).enumerate()
                        {
                            let level = first + local / effect.channels();
                            let channel = local % effect.channels();
                            let mut total = 0.0;
                            let mut norm = 0.0;
                            for &row in effect.level_index().members(level) {
                                let basis = effect.basis_value(channel, row);
                                total += prior_weights[row] * (outcome[row] - mean[row]) * basis;
                                norm += prior_weights[row] * mean[row] * basis * basis;
                            }
                            *moment = total;
                            *square = norm;
                        }
                        Ok(())
                    },
                )?;
            } else {
                for row in 0..rows {
                    let level = effect.codes()[row] as usize;
                    for channel in 0..effect.channels() {
                        let direction = effect_offset + level * effect.channels() + channel;
                        let basis = effect.basis_value(channel, row);
                        moments[direction] +=
                            prior_weights[row] * (outcome[row] - mean[row]) * basis;
                        squares[direction] += prior_weights[row] * mean[row] * basis * basis;
                    }
                }
            }
            for direction in effect_offset..effect_offset + directions {
                let moment = moments[direction];
                fe_absolute = fe_absolute.max(moment.abs());
                add_relative_score(
                    moment,
                    squares[direction].sqrt(),
                    score_scale,
                    &mut relative_infinity,
                    &mut relative_square,
                    &mut relative_count,
                );
            }
            effect_offset += directions;
        }
        Ok(())
    };
    // Do not submit a serial task: external Rayon submissions can allocate
    // queue blocks inside the required allocation-free iteration window.
    if parallel {
        workspace.team.install(accumulate_effects)?;
    } else {
        accumulate_effects()?;
    }
    let relative_two = if relative_count == 0 {
        0.0
    } else {
        (relative_square.total() / relative_count as f64).sqrt()
    };
    if !finite_absolute.is_finite()
        || !fe_absolute.is_finite()
        || !relative_infinity.is_finite()
        || !relative_two.is_finite()
    {
        return Err(numerical_nonconvergence(
            "prepared PPML score certificate is not finite",
        ));
    }
    Ok(PpmlCertificate {
        finite_absolute_infinity_norm: finite_absolute,
        fixed_effect_absolute_infinity_norm: fe_absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance && relative_two <= tolerance,
    })
}

fn add_relative_score(
    moment: f64,
    basis_norm: f64,
    score_scale: f64,
    infinity: &mut f64,
    square: &mut CompensatedSum,
    count: &mut usize,
) {
    let relative = super::certificate::relative_moment(moment, basis_norm, score_scale);
    *infinity = (*infinity).max(relative);
    square.add(relative * relative);
    *count += 1;
}

fn log_weighted_outcome_sum(outcome: &[f64], weights: &[f64]) -> Result<f64> {
    let mut maximum = f64::NEG_INFINITY;
    for (outcome, weight) in outcome.iter().zip(weights) {
        if *outcome > 0.0 {
            maximum = maximum.max(outcome.ln() + weight.ln());
        }
    }
    if !maximum.is_finite() {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "prepared PPML sample has no positive weighted outcome",
        ));
    }
    let mut sum = CompensatedSum::default();
    for (outcome, weight) in outcome.iter().zip(weights) {
        if *outcome > 0.0 {
            sum.add((outcome.ln() + weight.ln() - maximum).exp());
        }
    }
    let value = maximum + sum.total().ln();
    if value.is_finite() {
        Ok(value)
    } else {
        Err(numerical_nonconvergence(
            "prepared PPML weighted outcome sum is not finite",
        ))
    }
}

fn log_weighted_offset_sum(offset: &[f64], weights: &[f64]) -> Result<f64> {
    let mut maximum = f64::NEG_INFINITY;
    for (offset, weight) in offset.iter().zip(weights) {
        maximum = maximum.max(*offset + weight.ln());
    }
    if !maximum.is_finite() {
        return Err(numerical_nonconvergence(
            "prepared PPML weighted offset scale is not finite",
        ));
    }
    let mut sum = CompensatedSum::default();
    for (offset, weight) in offset.iter().zip(weights) {
        sum.add((*offset + weight.ln() - maximum).exp());
    }
    let value = maximum + sum.total().ln();
    if value.is_finite() {
        Ok(value)
    } else {
        Err(numerical_nonconvergence(
            "prepared PPML weighted offset sum is not finite",
        ))
    }
}

fn finite_unit(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

fn internal(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}

fn numerical_nonconvergence(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

const fn ppml_fallback_kind(kind: FrozenLinearFallbackKind) -> PpmlFallbackKind {
    match kind {
        FrozenLinearFallbackKind::WarmProjection => PpmlFallbackKind::WarmProjection,
        FrozenLinearFallbackKind::Moment => PpmlFallbackKind::Moment,
        FrozenLinearFallbackKind::Rank => PpmlFallbackKind::Rank,
        FrozenLinearFallbackKind::Bread => PpmlFallbackKind::Bread,
    }
}

#[cfg(test)]
mod tests;
