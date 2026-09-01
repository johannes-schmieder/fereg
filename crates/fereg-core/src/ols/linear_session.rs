//! Reusable, crate-private state for certified prepared weighted-linear solves.
//!
//! Public OLS remains deliberately one-shot.  This session is the iterative
//! estimator boundary: it owns changing response/weight buffers, freezes one
//! canonical rank basis, and retains the certified within design needed by
//! PPML inference. Canonical fresh/strict solves keep the unchanged prepared
//! OLS kernel; frozen-basis iterations use session-owned streaming moments and
//! factorization workspaces. Reuse is reported per route and never inferred.

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{BorrowedMapWeights, FeDesign, FeSolverKind, ReusableMapAbsorber, SolveReport};
use crate::metadata::CoefficientSlot;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::sandwich::SandwichDesign;
#[cfg(test)]
use std::cell::Cell;
use std::time::Instant;

use super::prepared_fit::{
    fit_prepared_weighted_linear_once, LinearSolveFrame, PreparedOlsFastScoreCache,
};
use super::prepared_scratch::PreparedOlsScratch;
use super::{
    CertifiedPreparedOlsFit, PreparedOlsBreadCoordinateKind, PreparedOlsExecutionMode,
    PreparedOlsFitConfig, PreparedOlsFitStatus,
};

const SCRATCH_TRANSPOSE_BYTES: usize = 1 << 20;

/// Monotone identities for every input class that can invalidate a solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LinearSessionGenerations {
    pub(crate) sample: u64,
    pub(crate) topology: u64,
    pub(crate) weights: u64,
    pub(crate) response: u64,
    pub(crate) basis: u64,
}

impl LinearSessionGenerations {
    pub(crate) const fn new(
        sample: u64,
        topology: u64,
        weights: u64,
        response: u64,
        basis: u64,
    ) -> Self {
        Self {
            sample,
            topology,
            weights,
            response,
            basis,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinearSessionSolveKind {
    Fresh,
    Warm,
    Strict,
}

/// Allocation phases observed only when an embedding explicitly installs a
/// qualification probe on a reusable linear session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum LinearSessionAllocationPhase {
    Projection,
    FiniteSolve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum LinearSessionPhaseAllocationEvent {
    Begin,
    End,
}

/// Qualification-only allocator callback. `Begin` establishes the phase
/// boundary and its return value is ignored. `End` returns the phase peak in
/// bytes relative to the action baseline maintained by the embedding layer.
#[doc(hidden)]
pub type LinearSessionPhaseAllocatorProbe =
    fn(LinearSessionAllocationPhase, LinearSessionPhaseAllocationEvent) -> u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LinearSessionPhaseAllocationObservation {
    pub(crate) projection_peak_bytes: Option<u64>,
    pub(crate) finite_solve_peak_bytes: Option<u64>,
}

impl LinearSessionPhaseAllocationObservation {
    fn observe(&mut self, phase: LinearSessionAllocationPhase, peak_bytes: u64) {
        let slot = match phase {
            LinearSessionAllocationPhase::Projection => &mut self.projection_peak_bytes,
            LinearSessionAllocationPhase::FiniteSolve => &mut self.finite_solve_peak_bytes,
        };
        *slot = Some(slot.map_or(peak_bytes, |prior| prior.max(peak_bytes)));
    }
}

/// Scope guard so a failed observed solve always closes the allocator phase.
/// The disabled OLS path supplies `None` and performs no callback or heap work.
pub(super) struct LinearSessionPhaseAllocationGuard<'a> {
    probe: Option<LinearSessionPhaseAllocatorProbe>,
    observation: Option<&'a mut LinearSessionPhaseAllocationObservation>,
    phase: LinearSessionAllocationPhase,
}

impl<'a> LinearSessionPhaseAllocationGuard<'a> {
    pub(super) fn new(
        probe: Option<LinearSessionPhaseAllocatorProbe>,
        observation: Option<&'a mut LinearSessionPhaseAllocationObservation>,
        phase: LinearSessionAllocationPhase,
    ) -> Self {
        if let Some(probe) = probe {
            let _ = probe(phase, LinearSessionPhaseAllocationEvent::Begin);
        }
        Self {
            probe,
            observation,
            phase,
        }
    }
}

impl Drop for LinearSessionPhaseAllocationGuard<'_> {
    fn drop(&mut self) {
        let Some(probe) = self.probe else {
            return;
        };
        let peak_bytes = probe(self.phase, LinearSessionPhaseAllocationEvent::End);
        if let Some(observation) = self.observation.as_deref_mut() {
            observation.observe(self.phase, peak_bytes);
        }
    }
}

/// Truthful reuse accounting for the most recent solve.
///
/// Canonical one-shot solves leave the kernel flags false. Frozen-basis solves
/// set them only after every required session-owned buffer has enough capacity.
/// FE projection has separate internal workspaces, so whole-solve allocation
/// freedom is never inferred from the finite kernel flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinearSessionReuseReport {
    pub(crate) solve_kind: LinearSessionSolveKind,
    pub(crate) response_buffer_reused: bool,
    pub(crate) weight_buffer_reused: bool,
    pub(crate) within_design_reused: bool,
    pub(crate) warm_projection_attempted: bool,
    pub(crate) warm_projection_certified: bool,
    pub(crate) canonical_fallback: bool,
    pub(crate) certificate_workspace_reused: bool,
    pub(crate) predictor_workspace_reused: bool,
    pub(crate) kernel_moment_workspace_reused: bool,
    pub(crate) kernel_solver_workspace_reused: bool,
    pub(crate) fe_projection_workspace_reused: bool,
    pub(crate) steady_state_allocation_free: bool,
    pub(crate) phase_timing: LinearSessionPhaseTiming,
    pub(crate) phase_allocation: LinearSessionPhaseAllocationObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct LinearSessionPhaseTiming {
    /// `None` on canonical one-shot routes whose internal phase clock is not
    /// exposed by the unchanged OLS kernel.
    pub(crate) projection_seconds: Option<f64>,
    pub(crate) finite_solve_seconds: Option<f64>,
}

impl LinearSessionReuseReport {
    const fn new(solve_kind: LinearSessionSolveKind) -> Self {
        Self {
            solve_kind,
            response_buffer_reused: false,
            weight_buffer_reused: false,
            within_design_reused: false,
            warm_projection_attempted: false,
            warm_projection_certified: false,
            canonical_fallback: false,
            certificate_workspace_reused: false,
            predictor_workspace_reused: false,
            kernel_moment_workspace_reused: false,
            kernel_solver_workspace_reused: false,
            fe_projection_workspace_reused: false,
            steady_state_allocation_free: false,
            phase_timing: LinearSessionPhaseTiming {
                projection_seconds: None,
                finite_solve_seconds: None,
            },
            phase_allocation: LinearSessionPhaseAllocationObservation {
                projection_peak_bytes: None,
                finite_solve_peak_bytes: None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinearWithinDesignCertificate {
    pub(crate) left_inverse_relative_infinity_norm: f64,
    pub(crate) right_inverse_relative_infinity_norm: f64,
    pub(crate) passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinearReconstructionCertificate {
    pub(crate) relative_infinity_norm: f64,
    pub(crate) passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LinearWithinMemoryReport {
    pub(crate) resident_bytes: u128,
    pub(crate) scratch_bytes: u64,
    pub(crate) rows: usize,
    pub(crate) columns: usize,
    pub(crate) mode: PreparedOlsExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LinearSessionWorkspaceCapacities {
    pub(crate) response_elements: usize,
    pub(crate) weight_elements: usize,
    pub(crate) finite_predictor_elements: usize,
    pub(crate) absorbed_predictor_elements: usize,
    pub(crate) moment_elements: usize,
    pub(crate) row_elements: usize,
    pub(crate) frozen_response_elements: usize,
    pub(crate) frozen_gram_elements: usize,
    pub(crate) frozen_factor_elements: usize,
    pub(crate) frozen_rhs_elements: usize,
    pub(crate) frozen_row_elements: usize,
    pub(crate) frozen_solve_elements: usize,
    pub(crate) frozen_score_elements: usize,
    pub(crate) frozen_coefficient_elements: usize,
    pub(crate) frozen_bread_elements: usize,
    pub(crate) frozen_residual_elements: usize,
    pub(crate) frozen_reported_coefficient_elements: usize,
    pub(crate) frozen_kept_elements: usize,
    pub(crate) frozen_basis_slot_elements: usize,
    pub(crate) map_weight_elements: usize,
    pub(crate) projection_block_elements: usize,
    pub(crate) within_resident_elements: usize,
    pub(crate) within_scratch_bytes: u64,
    pub(crate) within_io_buffer_elements: usize,
    pub(crate) fe_rhs_elements: usize,
    pub(crate) fe_max_channels: usize,
    pub(crate) fe_max_level_members: usize,
    pub(crate) fe_reference_norm_elements: usize,
    pub(crate) fe_residual_norm_elements: usize,
    pub(crate) fe_rhs_scale_elements: usize,
    pub(crate) fe_norm_scale_elements: usize,
    pub(crate) fe_norm_sum_elements: usize,
    pub(crate) fe_group_sum_elements: usize,
    pub(crate) fe_group_correction_elements: usize,
    pub(crate) fe_projection_gram_elements: usize,
    pub(crate) fe_projection_rhs_elements: usize,
    pub(crate) fe_certificate_column_elements: usize,
    pub(crate) fe_certificate_basis_elements: usize,
    pub(crate) fe_topology_effect_elements: usize,
    pub(crate) fe_topology_code_elements: usize,
    pub(crate) fe_topology_slope_vector_elements: usize,
    pub(crate) fe_topology_slope_value_elements: usize,
    pub(crate) fe_topology_level_offset_elements: usize,
    pub(crate) fe_topology_level_observation_elements: usize,
    pub(crate) fe_topology_resident_bytes: u128,
}

/// Independent certificates for one full-rank solve on the frozen finite
/// coordinate basis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrozenLinearSolveCertificate {
    pub(crate) score_relative_infinity_norm: f64,
    pub(crate) left_inverse_relative_infinity_norm: f64,
    pub(crate) right_inverse_relative_infinity_norm: f64,
    pub(crate) condition_proxy: f64,
    pub(crate) passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrozenLinearBasisSlot {
    pub(crate) coefficient_slot: CoefficientSlot,
    pub(crate) generated_column_index: Option<usize>,
    pub(crate) status: PreparedOlsFitStatus,
}

/// Rank-dependent identities frozen at the first successful solve in a basis
/// generation.  Coefficient values are intentionally excluded.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FrozenLinearBasis {
    slots: Vec<FrozenLinearBasisSlot>,
    kept_generated_columns: Vec<usize>,
    dropped_generated_columns: Vec<usize>,
}

/// Owned rollback state for a speculative strict rank decision.  A caller must
/// either commit the candidate or restore this token before continuing from
/// the previously accepted estimator state.
pub(crate) struct StrictLinearCandidateRollback {
    basis: FrozenLinearBasis,
    basis_generation: u64,
    within: Option<RetainedWithinDesign>,
    fit: Option<CertifiedPreparedOlsFit>,
    frozen_step: Option<CertifiedFrozenLinearStep>,
    finite_predictor: Vec<f64>,
    absorbed_predictor: Vec<f64>,
    reconstruction: Option<LinearReconstructionCertificate>,
    reuse: Option<LinearSessionReuseReport>,
    projection_refresh_required: bool,
}

impl FrozenLinearBasis {
    pub(crate) fn slots(&self) -> &[FrozenLinearBasisSlot] {
        &self.slots
    }

    pub(crate) fn kept_generated_columns(&self) -> &[usize] {
        &self.kept_generated_columns
    }

    fn from_fit(fit: &CertifiedPreparedOlsFit) -> Result<Self> {
        let mut slots = fallible_empty(
            fit.coefficients().len(),
            "unable to reserve frozen weighted-linear basis slots",
        )?;
        slots.extend(
            fit.coefficients()
                .iter()
                .map(|coefficient| FrozenLinearBasisSlot {
                    coefficient_slot: coefficient.coefficient_slot,
                    generated_column_index: coefficient.generated_column_index,
                    status: coefficient.status,
                }),
        );
        Ok(Self {
            slots,
            kept_generated_columns: fallible_copy(
                fit.kept_generated_columns(),
                "unable to reserve frozen weighted-linear kept basis",
            )?,
            dropped_generated_columns: fallible_copy(
                fit.dropped_generated_columns(),
                "unable to reserve frozen weighted-linear dropped basis",
            )?,
        })
    }

    fn matches(&self, fit: &CertifiedPreparedOlsFit) -> bool {
        self.kept_generated_columns == fit.kept_generated_columns()
            && self.dropped_generated_columns == fit.dropped_generated_columns()
            && self.slots.len() == fit.coefficients().len()
            && self
                .slots
                .iter()
                .zip(fit.coefficients())
                .all(|(frozen, current)| {
                    frozen.coefficient_slot == current.coefficient_slot
                        && frozen.generated_column_index == current.generated_column_index
                        && frozen.status == current.status
                })
    }
}

#[derive(Debug)]
enum OwnedLinearWeights {
    Unit,
    Provided(Vec<f64>),
}

impl OwnedLinearWeights {
    fn from_borrowed(weights: BorrowedMapWeights<'_>) -> Result<Self> {
        match weights {
            BorrowedMapWeights::Unit => Ok(Self::Unit),
            BorrowedMapWeights::Provided(values) => Ok(Self::Provided(fallible_copy(
                values,
                "unable to reserve weighted-linear session weights",
            )?)),
        }
    }

    fn borrowed(&self) -> BorrowedMapWeights<'_> {
        match self {
            Self::Unit => BorrowedMapWeights::Unit,
            Self::Provided(values) => BorrowedMapWeights::Provided(values),
        }
    }

    fn replace(&mut self, weights: BorrowedMapWeights<'_>) -> Result<bool> {
        match (self, weights) {
            (Self::Unit, BorrowedMapWeights::Unit) => Ok(true),
            (Self::Provided(target), BorrowedMapWeights::Provided(values)) => {
                let reused = target.capacity() >= values.len();
                target.clear();
                target
                    .try_reserve(values.len())
                    .map_err(|_| allocation("unable to refresh weighted-linear session weights"))?;
                target.extend_from_slice(values);
                Ok(reused)
            }
            (target, BorrowedMapWeights::Unit) => {
                *target = Self::Unit;
                Ok(false)
            }
            (target, BorrowedMapWeights::Provided(values)) => {
                *target = Self::Provided(fallible_copy(
                    values,
                    "unable to refresh weighted-linear session weights",
                )?);
                Ok(false)
            }
        }
    }

    fn capacity(&self) -> usize {
        match self {
            Self::Unit => 0,
            Self::Provided(values) => values.capacity(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WithinGenerations {
    sample: u64,
    topology: u64,
    weights: u64,
    basis: u64,
}

impl WithinGenerations {
    const fn from_session(generations: LinearSessionGenerations) -> Self {
        Self {
            sample: generations.sample,
            topology: generations.topology,
            weights: generations.weights,
            basis: generations.basis,
        }
    }

    const fn same_design(self, generations: LinearSessionGenerations) -> bool {
        self.sample == generations.sample
            && self.topology == generations.topology
            && self.weights == generations.weights
            && self.basis == generations.basis
    }

    const fn eligible_for_weight_refresh(self, generations: LinearSessionGenerations) -> bool {
        self.sample == generations.sample
            && self.topology == generations.topology
            && self.basis == generations.basis
            && self.weights != generations.weights
    }
}

#[derive(Debug)]
enum RetainedWithinStorage {
    Dense(DenseMatrix),
    /// The scratch file is row-major so a complete inference row is one read.
    Scratch(PreparedOlsScratch),
}

impl RetainedWithinStorage {
    fn rows(&self) -> usize {
        match self {
            Self::Dense(values) => values.nrows(),
            Self::Scratch(values) => values.rows(),
        }
    }

    fn columns(&self) -> usize {
        match self {
            Self::Dense(values) => values.ncols(),
            Self::Scratch(values) => values.columns(),
        }
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        let rows = self.rows();
        let columns = self.columns();
        if row >= rows || output.len() != columns {
            return Err(invalid_argument(
                "weighted-linear within-design row dimensions are inconsistent",
            ));
        }
        match self {
            Self::Dense(values) => {
                let start = row
                    .checked_mul(columns)
                    .ok_or_else(|| allocation("weighted-linear within row offset overflows"))?;
                output.copy_from_slice(&values.as_slice()[start..start + columns]);
                Ok(())
            }
            Self::Scratch(values) => {
                let start = row
                    .checked_mul(columns)
                    .ok_or_else(|| allocation("weighted-linear scratch row offset overflows"))?;
                values.read_linear_shared(start, output)
            }
        }
    }

    fn scratch_bytes(&self) -> u64 {
        match self {
            Self::Dense(_) => 0,
            Self::Scratch(values) => values.bytes(),
        }
    }

    fn memory_report(&self) -> LinearWithinMemoryReport {
        let rows = self.rows();
        let columns = self.columns();
        match self {
            Self::Dense(values) => LinearWithinMemoryReport {
                resident_bytes: values.as_slice().len() as u128 * 8,
                scratch_bytes: 0,
                rows,
                columns,
                mode: PreparedOlsExecutionMode::Fast,
            },
            Self::Scratch(values) => LinearWithinMemoryReport {
                resident_bytes: 0,
                scratch_bytes: values.bytes(),
                rows,
                columns,
                mode: PreparedOlsExecutionMode::Low,
            },
        }
    }
}

#[derive(Debug)]
struct RetainedWithinDesign {
    storage: RetainedWithinStorage,
    generations: WithinGenerations,
    certificate: LinearWithinDesignCertificate,
}

/// Copyable, thread-safe borrowed access to the final compact within design.
/// Dense rows are borrowed directly; low-memory rows use positional scratch
/// reads and therefore do not share or lock a mutable file cursor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CertifiedLinearWithinDesign<'a> {
    storage: &'a RetainedWithinStorage,
}

impl SandwichDesign for CertifiedLinearWithinDesign<'_> {
    fn nrows(&self) -> usize {
        self.storage.rows()
    }

    fn dimension(&self) -> usize {
        self.storage.columns()
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        self.storage.fill_row(row, output)
    }
}

/// Borrowed certified products exposed to an iterative estimator and its
/// final inference stage.  Nothing here is exported from `fereg-core`.
pub(crate) struct CertifiedLinearSessionOutput<'a> {
    fit: &'a CertifiedPreparedOlsFit,
    finite_predictor: &'a [f64],
    absorbed_predictor: &'a [f64],
    within: &'a RetainedWithinDesign,
    reconstruction: LinearReconstructionCertificate,
    reuse: LinearSessionReuseReport,
}

impl<'a> CertifiedLinearSessionOutput<'a> {
    pub(crate) const fn fit(&self) -> &'a CertifiedPreparedOlsFit {
        self.fit
    }

    pub(crate) const fn finite_predictor(&self) -> &'a [f64] {
        self.finite_predictor
    }

    pub(crate) const fn absorbed_predictor(&self) -> &'a [f64] {
        self.absorbed_predictor
    }

    pub(crate) const fn reuse_report(&self) -> LinearSessionReuseReport {
        self.reuse
    }

    pub(crate) fn retained_within_memory(&self) -> LinearWithinMemoryReport {
        self.within.storage.memory_report()
    }

    pub(crate) fn certified(&self) -> bool {
        self.fit.rank_certificate().passed
            && self.fit.score_certificate().passed
            && self.fit.bread().certificate().passed
            && self.fit.absorption_report().certified()
            && self.fit.residual_absorption_report().certified()
            && self.within.certificate.passed
            && self.reconstruction.passed
    }
}

#[derive(Debug)]
struct CertifiedFrozenLinearStep {
    coefficients: Vec<f64>,
    reported_coefficients: Vec<Option<f64>>,
    bread: Vec<f64>,
    within_residuals: Vec<f64>,
    response_absorption_report: SolveReport,
    certificate: FrozenLinearSolveCertificate,
    generations: LinearSessionGenerations,
}

/// Borrowed products from the no-N-by-K-rebuild iteration path. Coefficients
/// and bread follow `basis().kept_generated_columns()` order.
pub(crate) struct CertifiedFrozenLinearOutput<'a> {
    step: &'a CertifiedFrozenLinearStep,
    basis: &'a FrozenLinearBasis,
    response: &'a [f64],
    finite_predictor: &'a [f64],
    absorbed_predictor: &'a [f64],
    within: &'a RetainedWithinDesign,
    reconstruction: LinearReconstructionCertificate,
    reuse: LinearSessionReuseReport,
}

impl<'a> CertifiedFrozenLinearOutput<'a> {
    pub(crate) fn coefficients(&self) -> &'a [f64] {
        &self.step.coefficients
    }

    pub(crate) fn reported_coefficients(&self) -> &'a [Option<f64>] {
        &self.step.reported_coefficients
    }

    #[cfg(test)]
    pub(crate) fn bread(&self) -> &'a [f64] {
        &self.step.bread
    }

    pub(crate) const fn response_absorption_report(&self) -> &'a SolveReport {
        &self.step.response_absorption_report
    }

    pub(crate) const fn certificate(&self) -> FrozenLinearSolveCertificate {
        self.step.certificate
    }

    pub(crate) const fn basis(&self) -> &'a FrozenLinearBasis {
        self.basis
    }

    pub(crate) const fn response(&self) -> &'a [f64] {
        self.response
    }

    pub(crate) const fn finite_predictor(&self) -> &'a [f64] {
        self.finite_predictor
    }

    pub(crate) const fn absorbed_predictor(&self) -> &'a [f64] {
        self.absorbed_predictor
    }

    #[cfg(test)]
    pub(crate) fn finite_within_rows(&self) -> usize {
        self.within.storage.rows()
    }

    #[cfg(test)]
    pub(crate) fn finite_within_columns(&self) -> usize {
        self.within.storage.columns()
    }

    pub(crate) const fn within_certificate(&self) -> LinearWithinDesignCertificate {
        self.within.certificate
    }

    pub(crate) const fn reconstruction_certificate(&self) -> LinearReconstructionCertificate {
        self.reconstruction
    }

    pub(crate) const fn reuse_report(&self) -> LinearSessionReuseReport {
        self.reuse
    }

    pub(crate) fn certified(&self) -> bool {
        self.step.response_absorption_report.certified()
            && self.step.certificate.passed
            && self.within.certificate.passed
            && self.reconstruction.passed
    }
}

#[derive(Debug, Default)]
struct FrozenLinearWorkspace {
    response: Vec<f64>,
    gram: Vec<f64>,
    factor: Vec<f64>,
    rhs: Vec<f64>,
    row: Vec<f64>,
    solve: Vec<f64>,
    scores: Vec<f64>,
    coefficients: Vec<f64>,
    bread: Vec<f64>,
    residuals: Vec<f64>,
    reported_coefficients: Vec<Option<f64>>,
    kept: Vec<usize>,
    basis_slots: Vec<FrozenLinearBasisSlot>,
}

impl FrozenLinearWorkspace {
    fn can_reuse(&self, rows: usize, rank: usize, slots: usize) -> bool {
        let square = rank.saturating_mul(rank);
        self.response.capacity() >= rows
            && self.gram.capacity() >= square
            && self.factor.capacity() >= square
            && self.rhs.capacity() >= rank
            && self.row.capacity() >= rank
            && self.solve.capacity() >= rank
            && self.scores.capacity() >= rank
            && self.coefficients.capacity() >= rank
            && self.bread.capacity() >= square
            && self.residuals.capacity() >= rows
            && self.reported_coefficients.capacity() >= slots
            && self.kept.capacity() >= rank
            && self.basis_slots.capacity() >= slots
    }
}

/// Owned handoff after a final strict solve. This keeps dense or scratch
/// within-design row access alive after the iterative session is consumed.
pub(crate) struct CertifiedLinearSessionProducts {
    fit: CertifiedPreparedOlsFit,
    identified_bread: Vec<f64>,
    generations: LinearSessionGenerations,
    basis: FrozenLinearBasis,
    weights: OwnedLinearWeights,
    finite_predictor: Vec<f64>,
    absorbed_predictor: Vec<f64>,
    within: RetainedWithinDesign,
    reconstruction: LinearReconstructionCertificate,
}

impl CertifiedLinearSessionProducts {
    pub(crate) const fn fit(&self) -> &CertifiedPreparedOlsFit {
        &self.fit
    }

    /// Compact inverse cross product aligned exactly to
    /// `basis().kept_generated_columns()`. A normalized absorbed intercept is
    /// a reported coordinate and is deliberately excluded.
    pub(crate) fn identified_bread(&self) -> &[f64] {
        &self.identified_bread
    }

    pub(crate) const fn generations(&self) -> LinearSessionGenerations {
        self.generations
    }

    pub(crate) const fn basis(&self) -> &FrozenLinearBasis {
        &self.basis
    }

    pub(crate) fn weights(&self) -> BorrowedMapWeights<'_> {
        self.weights.borrowed()
    }

    pub(crate) fn finite_predictor(&self) -> &[f64] {
        &self.finite_predictor
    }

    pub(crate) fn absorbed_predictor(&self) -> &[f64] {
        &self.absorbed_predictor
    }

    pub(crate) const fn within_certificate(&self) -> LinearWithinDesignCertificate {
        self.within.certificate
    }

    pub(crate) const fn finite_within_design(&self) -> CertifiedLinearWithinDesign<'_> {
        CertifiedLinearWithinDesign {
            storage: &self.within.storage,
        }
    }

    pub(crate) const fn reconstruction_certificate(&self) -> LinearReconstructionCertificate {
        self.reconstruction
    }
}

/// Stateful owner for repeated solves on one immutable prepared sample and FE
/// topology.  Sample or topology changes require a new session by design.
pub(crate) struct CertifiedLinearSession<'a, P: PreparedOlsFitView + Sync + ?Sized> {
    prepared: &'a P,
    config: PreparedOlsFitConfig,
    response: Vec<f64>,
    weights: OwnedLinearWeights,
    bread_weight_scale: super::PreparedOlsBreadWeightScale,
    generations: LinearSessionGenerations,
    prepared_alignment_valid: bool,
    frozen_basis: Option<FrozenLinearBasis>,
    within: Option<RetainedWithinDesign>,
    fit: Option<CertifiedPreparedOlsFit>,
    frozen_step: Option<CertifiedFrozenLinearStep>,
    finite_predictor: Vec<f64>,
    absorbed_predictor: Vec<f64>,
    moment_workspace: Vec<f64>,
    row_workspace: Vec<f64>,
    frozen_workspace: FrozenLinearWorkspace,
    map_weights: Vec<f64>,
    projection_block: Vec<f64>,
    fe_projector: ReusableMapAbsorber,
    reconstruction: Option<LinearReconstructionCertificate>,
    reuse: Option<LinearSessionReuseReport>,
    pending_response_buffer_reused: bool,
    pending_weight_buffer_reused: bool,
    projection_refresh_required: bool,
    phase_allocator_probe: Option<LinearSessionPhaseAllocatorProbe>,
}

/// A frozen solve may fail because its reused numerical state no longer
/// certifies even though the canonical prepared problem remains valid.  This
/// classification is deliberately narrower than `InvalidData`: caller/input,
/// allocation, interruption, and invariant failures are never fallback
/// candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrozenLinearFallbackKind {
    WarmProjection,
    Moment,
    Rank,
    Bread,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrozenLinearFailureInjection {
    RecoverableWarmProjection,
    RecoverableThenCanonicalAllocation,
    Domain,
    Interrupted,
}

#[cfg(test)]
thread_local! {
    static FROZEN_FAILURE_INJECTION: Cell<u8> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn inject_frozen_linear_failure(injection: FrozenLinearFailureInjection) {
    let state = match injection {
        FrozenLinearFailureInjection::RecoverableWarmProjection => 1,
        FrozenLinearFailureInjection::RecoverableThenCanonicalAllocation => 2,
        FrozenLinearFailureInjection::Domain => 3,
        FrozenLinearFailureInjection::Interrupted => 4,
    };
    FROZEN_FAILURE_INJECTION.set(state);
}

pub(crate) fn classify_frozen_linear_fallback(
    error: &FeregError,
) -> Option<FrozenLinearFallbackKind> {
    if !matches!(
        error.code,
        ErrorCode::InvalidData | ErrorCode::NonConvergence
    ) {
        return None;
    }
    let message = error.message.as_str();
    if message.starts_with("frozen weighted-linear reusable MAP projection failed;")
        || message.starts_with("frozen weighted-linear warm projection failed;")
        || message == "frozen weighted-linear response absorption did not certify"
    {
        Some(FrozenLinearFallbackKind::WarmProjection)
    } else if matches!(
        message,
        "frozen weighted-linear moments are not finite"
            | "frozen weighted-linear residual or score is not finite"
    ) {
        Some(FrozenLinearFallbackKind::Moment)
    } else if matches!(
        message,
        "frozen weighted-linear basis lost full rank"
            | "frozen weighted-linear Cholesky factor is not finite"
            | "frozen weighted-linear condition proxy is not finite"
            | "frozen weighted-linear solution is not finite"
    ) {
        Some(FrozenLinearFallbackKind::Rank)
    } else if matches!(
        message,
        "frozen weighted-linear inverse certificate is not finite"
            | "frozen weighted-linear bread is not finite"
            | "frozen weighted-linear score or inverse certificate did not pass"
    ) {
        Some(FrozenLinearFallbackKind::Bread)
    } else {
        None
    }
}

impl<'a, P: PreparedOlsFitView + Sync + ?Sized> CertifiedLinearSession<'a, P> {
    #[cfg(test)]
    pub(crate) fn new(
        prepared: &'a P,
        frame: LinearSolveFrame<'_>,
        config: PreparedOlsFitConfig,
        generations: LinearSessionGenerations,
    ) -> Result<Self> {
        let fe_design = prepared.fit_owned_fe_design()?;
        Self::new_with_fe_design(prepared, frame, config, generations, fe_design)
    }

    /// Construct from the estimator's one owned FE design. PPML uses this
    /// boundary after initialization so it does not duplicate categorical
    /// topology solely for the reusable projector.
    pub(crate) fn new_with_fe_design(
        prepared: &'a P,
        frame: LinearSolveFrame<'_>,
        config: PreparedOlsFitConfig,
        generations: LinearSessionGenerations,
        fe_design: FeDesign,
    ) -> Result<Self> {
        frame.validate_for(prepared)?;
        let response = fallible_copy(
            frame.response(),
            "unable to reserve weighted-linear session response",
        )?;
        let weights = OwnedLinearWeights::from_borrowed(frame.raw_weights())?;
        let maximum_fe_rhs = match config.execution.mode {
            PreparedOlsExecutionMode::Fast => prepared.fit_model().columns.len().max(1),
            PreparedOlsExecutionMode::Low => config.execution.poolsize.max(1),
        };
        if fe_design.nobs() != response.len() {
            return Err(invalid_argument(
                "weighted-linear owned FE design is not aligned on the prepared sample",
            ));
        }
        let fe_projector = ReusableMapAbsorber::new(fe_design, maximum_fe_rhs)?;
        let map_weights = fallible_filled(
            response.len(),
            0.0,
            "unable to reserve weighted-linear reusable MAP weights",
        )?;
        Ok(Self {
            prepared,
            config,
            response,
            weights,
            bread_weight_scale: frame.bread_weight_scale(),
            generations,
            prepared_alignment_valid: true,
            frozen_basis: None,
            within: None,
            fit: None,
            frozen_step: None,
            finite_predictor: Vec::new(),
            absorbed_predictor: Vec::new(),
            moment_workspace: Vec::new(),
            row_workspace: Vec::new(),
            frozen_workspace: FrozenLinearWorkspace::default(),
            map_weights,
            projection_block: Vec::new(),
            fe_projector,
            reconstruction: None,
            reuse: None,
            pending_response_buffer_reused: false,
            pending_weight_buffer_reused: false,
            projection_refresh_required: false,
            phase_allocator_probe: None,
        })
    }

    pub(crate) const fn fe_design(&self) -> &FeDesign {
        self.fe_projector.design()
    }

    /// Install or remove the qualification-only phase allocator observer.
    /// Normal OLS never constructs a session and the default session path is
    /// `None`, so this adds no allocator state to the one-shot OLS boundary.
    pub(crate) fn set_phase_allocator_probe(
        &mut self,
        probe: Option<LinearSessionPhaseAllocatorProbe>,
    ) {
        self.phase_allocator_probe = probe;
    }

    /// Update the numerical policy used by subsequent projection/solve calls.
    /// Tightening or otherwise changing the absorption policy forces a warm
    /// re-projection before another frozen solve can be published.
    pub(crate) fn refresh_config(&mut self, config: PreparedOlsFitConfig) {
        if self.config.absorb != config.absorb {
            self.projection_refresh_required = true;
        }
        self.config = config;
        self.fit = None;
        self.recycle_frozen_step();
        self.reconstruction = None;
        self.reuse = None;
    }

    pub(crate) fn set_config(&mut self, config: PreparedOlsFitConfig) -> Result<()> {
        self.ensure_bound()?;
        self.refresh_config(config);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn requires_rebind(&self) -> bool {
        !self.prepared_alignment_valid
    }

    pub(crate) fn retained_within_memory(&self) -> Option<LinearWithinMemoryReport> {
        self.within
            .as_ref()
            .map(|within| within.storage.memory_report())
    }

    #[cfg(test)]
    pub(crate) fn retained_scratch_identity(&self) -> Option<&std::path::Path> {
        match &self.within.as_ref()?.storage {
            RetainedWithinStorage::Dense(_) => None,
            RetainedWithinStorage::Scratch(scratch) => Some(scratch.identity_path()),
        }
    }

    pub(crate) fn workspace_capacities(&self) -> LinearSessionWorkspaceCapacities {
        let current_step = self.frozen_step.as_ref();
        let fe = self.fe_projector.capacities();
        let (within_resident_elements, within_scratch_bytes, within_io_buffer_elements) = self
            .within
            .as_ref()
            .map_or((0, 0, 0), |within| match &within.storage {
                RetainedWithinStorage::Dense(values) => (values.storage_capacity(), 0, 0),
                RetainedWithinStorage::Scratch(scratch) => {
                    (0, scratch.bytes(), scratch.io_buffer_capacity())
                }
            });
        LinearSessionWorkspaceCapacities {
            response_elements: self.response.capacity(),
            weight_elements: self.weights.capacity(),
            finite_predictor_elements: self.finite_predictor.capacity(),
            absorbed_predictor_elements: self.absorbed_predictor.capacity(),
            moment_elements: self.moment_workspace.capacity(),
            row_elements: self.row_workspace.capacity(),
            frozen_response_elements: self.frozen_workspace.response.capacity(),
            frozen_gram_elements: self.frozen_workspace.gram.capacity(),
            frozen_factor_elements: self.frozen_workspace.factor.capacity(),
            frozen_rhs_elements: self.frozen_workspace.rhs.capacity(),
            frozen_row_elements: self.frozen_workspace.row.capacity(),
            frozen_solve_elements: self.frozen_workspace.solve.capacity(),
            frozen_score_elements: self.frozen_workspace.scores.capacity(),
            frozen_coefficient_elements: current_step.map_or_else(
                || self.frozen_workspace.coefficients.capacity(),
                |step| step.coefficients.capacity(),
            ),
            frozen_bread_elements: current_step.map_or_else(
                || self.frozen_workspace.bread.capacity(),
                |step| step.bread.capacity(),
            ),
            frozen_residual_elements: current_step.map_or_else(
                || self.frozen_workspace.residuals.capacity(),
                |step| step.within_residuals.capacity(),
            ),
            frozen_reported_coefficient_elements: current_step.map_or_else(
                || self.frozen_workspace.reported_coefficients.capacity(),
                |step| step.reported_coefficients.capacity(),
            ),
            frozen_kept_elements: self.frozen_workspace.kept.capacity(),
            frozen_basis_slot_elements: self.frozen_workspace.basis_slots.capacity(),
            map_weight_elements: self.map_weights.capacity(),
            projection_block_elements: self.projection_block.capacity(),
            within_resident_elements,
            within_scratch_bytes,
            within_io_buffer_elements,
            fe_rhs_elements: fe.rhs,
            fe_max_channels: fe.max_channels,
            fe_max_level_members: fe.max_level_members,
            fe_reference_norm_elements: fe.reference_norms,
            fe_residual_norm_elements: fe.residual_norms,
            fe_rhs_scale_elements: fe.rhs_scales,
            fe_norm_scale_elements: fe.norm_scales,
            fe_norm_sum_elements: fe.norm_sums,
            fe_group_sum_elements: fe.group_sums,
            fe_group_correction_elements: fe.group_corrections,
            fe_projection_gram_elements: fe.projection_gram,
            fe_projection_rhs_elements: fe.projection_rhs,
            fe_certificate_column_elements: fe.certificate_columns,
            fe_certificate_basis_elements: fe.certificate_basis,
            fe_topology_effect_elements: fe.topology_effects,
            fe_topology_code_elements: fe.topology_codes,
            fe_topology_slope_vector_elements: fe.topology_slope_vectors,
            fe_topology_slope_value_elements: fe.topology_slope_values,
            fe_topology_level_offset_elements: fe.topology_level_offsets,
            fe_topology_level_observation_elements: fe.topology_level_observations,
            fe_topology_resident_bytes: fe.topology_resident_bytes,
        }
    }

    /// Canonical solve using the current frame.  An already-frozen basis must
    /// still match; call `refresh_basis` to authorize a new rank decision.
    pub(crate) fn solve_fresh(&mut self) -> Result<CertifiedLinearSessionOutput<'_>> {
        #[cfg(test)]
        if FROZEN_FAILURE_INJECTION.get() == 5 {
            FROZEN_FAILURE_INJECTION.set(0);
            return Err(allocation("injected canonical fallback allocation failure"));
        }
        self.solve(LinearSessionSolveKind::Fresh)
    }

    /// Solve after a response or weight refresh, reusing or re-projecting the
    /// retained within design when its generation tags permit it.
    #[cfg(test)]
    pub(crate) fn solve_warm(&mut self) -> Result<CertifiedLinearSessionOutput<'_>> {
        self.solve(LinearSessionSolveKind::Warm)
    }

    /// Absorb only the changing response and stream the retained finite within
    /// design into a frozen-basis solve. This is the PPML iteration path.
    pub(crate) fn solve_frozen(&mut self) -> Result<CertifiedFrozenLinearOutput<'_>> {
        #[cfg(test)]
        match FROZEN_FAILURE_INJECTION.get() {
            1 => {
                FROZEN_FAILURE_INJECTION.set(0);
                return Err(invalid_data(
                    "frozen weighted-linear response absorption did not certify",
                ));
            }
            2 => {
                FROZEN_FAILURE_INJECTION.set(5);
                return Err(invalid_data(
                    "frozen weighted-linear response absorption did not certify",
                ));
            }
            3 => {
                FROZEN_FAILURE_INJECTION.set(0);
                return Err(invalid_data(
                    "frozen weighted-linear normalized weight is invalid",
                ));
            }
            4 => {
                FROZEN_FAILURE_INJECTION.set(0);
                return Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "injected frozen solve interruption",
                ));
            }
            _ => {}
        }
        self.solve_frozen_internal()
    }

    #[cfg(test)]
    pub(crate) fn strictify(&mut self) -> Result<CertifiedLinearSessionOutput<'_>> {
        self.within = None;
        self.fit = None;
        self.reconstruction = None;
        self.reuse = None;
        self.solve(LinearSessionSolveKind::Strict)
    }

    /// Discard every product derived from a failed frozen attempt while
    /// retaining the canonical prepared inputs, current response/weights, and
    /// frozen basis.  A subsequent `solve_fresh` must independently rebuild
    /// and certify the current frame.
    pub(crate) fn discard_invalid_frozen_state(&mut self) {
        self.within = None;
        self.fit = None;
        self.recycle_frozen_step();
        self.reconstruction = None;
        self.reuse = None;
        self.projection_refresh_required = false;
    }

    /// Build a canonical strict candidate while explicitly authorizing a new
    /// rank decision. The returned flag compares the complete canonical basis
    /// identity (kept/dropped coordinates and every coefficient-slot status)
    /// with the basis that was authoritative before the refresh. PPML uses
    /// this at convergence checkpoints so a same-rank pivot change cannot be
    /// mistaken for an unchanged publication basis.
    pub(crate) fn strict_candidate(
        &mut self,
    ) -> Result<(
        CertifiedLinearSessionOutput<'_>,
        bool,
        StrictLinearCandidateRollback,
    )> {
        let basis_generation = self.generations.basis;
        let previous_basis = self.frozen_basis.take().ok_or_else(|| {
            invalid_argument("strict weighted-linear candidate requires a frozen canonical basis")
        })?;
        let candidate_basis_generation = basis_generation.checked_add(1).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "weighted-linear basis generation overflowed",
            )
        })?;
        require_newer(self.generations.basis, candidate_basis_generation, "basis")?;
        let rollback = StrictLinearCandidateRollback {
            basis: previous_basis,
            basis_generation,
            within: self.within.take(),
            fit: self.fit.take(),
            frozen_step: self.frozen_step.take(),
            finite_predictor: std::mem::take(&mut self.finite_predictor),
            absorbed_predictor: std::mem::take(&mut self.absorbed_predictor),
            reconstruction: self.reconstruction.take(),
            reuse: self.reuse.take(),
            projection_refresh_required: self.projection_refresh_required,
        };
        self.generations.basis = candidate_basis_generation;
        self.frozen_basis = None;
        self.projection_refresh_required = false;
        let output = self.solve(LinearSessionSolveKind::Strict)?;
        let basis_unchanged = rollback.basis.matches(output.fit());
        Ok((output, basis_unchanged, rollback))
    }

    /// Commit a speculative strict candidate by consuming its rollback token.
    pub(crate) fn commit_strict_candidate(&mut self, _rollback: StrictLinearCandidateRollback) {}

    /// Restore the exact canonical basis that preceded a rejected strict
    /// candidate and discard every solve product derived from the speculative
    /// basis.  The current response and weights remain bound, so the next
    /// iteration can independently rebuild its linear products.
    pub(crate) fn rollback_strict_candidate(&mut self, rollback: StrictLinearCandidateRollback) {
        self.recycle_frozen_step();
        self.generations.basis = rollback.basis_generation;
        self.frozen_basis = Some(rollback.basis);
        self.within = rollback.within;
        self.fit = rollback.fit;
        self.frozen_step = rollback.frozen_step;
        self.finite_predictor = rollback.finite_predictor;
        self.absorbed_predictor = rollback.absorbed_predictor;
        self.reconstruction = rollback.reconstruction;
        self.reuse = rollback.reuse;
        self.projection_refresh_required = rollback.projection_refresh_required;
    }

    #[cfg(test)]
    pub(crate) fn inject_same_rank_strict_basis_pivot(&mut self) -> bool {
        let Some(basis) = self.frozen_basis.as_mut() else {
            return false;
        };
        let (Some(kept), Some(dropped)) = (
            basis.kept_generated_columns.first_mut(),
            basis.dropped_generated_columns.first_mut(),
        ) else {
            return false;
        };
        std::mem::swap(kept, dropped);
        true
    }

    #[cfg(test)]
    pub(crate) const fn basis_generation_for_test(&self) -> u64 {
        self.generations.basis
    }

    pub(crate) fn output(&self) -> Result<CertifiedLinearSessionOutput<'_>> {
        let fit = self.fit.as_ref().ok_or_else(|| {
            invalid_argument("weighted-linear session has no current certified solve")
        })?;
        self.frozen_basis.as_ref().ok_or_else(|| {
            invalid_argument("weighted-linear session has no frozen canonical basis")
        })?;
        let within = self.within.as_ref().ok_or_else(|| {
            invalid_argument("weighted-linear session has no certified within design")
        })?;
        if !within.generations.same_design(self.generations) {
            return Err(invalid_argument(
                "weighted-linear session within design has stale generations",
            ));
        }
        Ok(CertifiedLinearSessionOutput {
            fit,
            finite_predictor: &self.finite_predictor,
            absorbed_predictor: &self.absorbed_predictor,
            within,
            reconstruction: self.reconstruction.ok_or_else(|| {
                invalid_argument("weighted-linear session reconstruction is not certified")
            })?,
            reuse: self
                .reuse
                .ok_or_else(|| invalid_argument("weighted-linear session has no reuse report"))?,
        })
    }

    /// Consume a session after its final canonical solve while preserving the
    /// final within storage for inference.
    pub(crate) fn into_current_products(mut self) -> Result<CertifiedLinearSessionProducts> {
        self.output()?;
        fill_identified_bread(
            self.fit.as_ref().ok_or_else(|| {
                invalid_argument("weighted-linear session has no final fit to hand off")
            })?,
            self.frozen_basis.as_ref().ok_or_else(|| {
                invalid_argument("weighted-linear session has no final basis to hand off")
            })?,
            &mut self.frozen_workspace.bread,
        )?;
        let identified_bread = std::mem::take(&mut self.frozen_workspace.bread);
        Ok(CertifiedLinearSessionProducts {
            fit: self.fit.ok_or_else(|| {
                invalid_argument("weighted-linear session has no final fit to hand off")
            })?,
            identified_bread,
            generations: self.generations,
            basis: self.frozen_basis.ok_or_else(|| {
                invalid_argument("weighted-linear session has no final basis to hand off")
            })?,
            weights: self.weights,
            finite_predictor: self.finite_predictor,
            absorbed_predictor: self.absorbed_predictor,
            within: self.within.ok_or_else(|| {
                invalid_argument("weighted-linear session has no final within design to hand off")
            })?,
            reconstruction: self.reconstruction.ok_or_else(|| {
                invalid_argument("weighted-linear session has no final reconstruction")
            })?,
        })
    }

    /// Response generations preserve all certified design-only products.
    pub(crate) fn refresh_response(
        &mut self,
        response: &[f64],
        response_generation: u64,
    ) -> Result<()> {
        require_newer(self.generations.response, response_generation, "response")?;
        self.ensure_bound()?;
        LinearSolveFrame::from_parts(response, self.weights.borrowed(), self.bread_weight_scale)?
            .validate_for(self.prepared)?;
        let reused = self.response.capacity() >= response.len();
        self.response.clear();
        self.response
            .try_reserve(response.len())
            .map_err(|_| allocation("unable to refresh weighted-linear session response"))?;
        self.response.extend_from_slice(response);
        self.generations.response = response_generation;
        self.fit = None;
        self.recycle_frozen_step();
        self.reconstruction = None;
        self.reuse = None;
        self.pending_response_buffer_reused = reused;
        Ok(())
    }

    /// Weight generations invalidate every weighted product but retain the old
    /// within design as the input to one certified warm re-projection attempt.
    pub(crate) fn refresh_weights(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        bread_weight_scale: super::PreparedOlsBreadWeightScale,
        weight_generation: u64,
    ) -> Result<()> {
        require_newer(self.generations.weights, weight_generation, "weight")?;
        self.ensure_bound()?;
        LinearSolveFrame::from_parts(&self.response, weights, bread_weight_scale)?
            .validate_for(self.prepared)?;
        self.pending_weight_buffer_reused = self.weights.replace(weights)?;
        self.bread_weight_scale = bread_weight_scale;
        self.generations.weights = weight_generation;
        self.fit = None;
        self.recycle_frozen_step();
        self.reconstruction = None;
        self.reuse = None;
        Ok(())
    }

    /// Authorize a new canonical rank decision and invalidate all products
    /// whose dimensions or coordinate identities depend on that decision.
    #[cfg(test)]
    pub(crate) fn refresh_basis(&mut self, basis_generation: u64) -> Result<()> {
        require_newer(self.generations.basis, basis_generation, "basis")?;
        self.generations.basis = basis_generation;
        self.frozen_basis = None;
        self.within = None;
        self.fit = None;
        self.recycle_frozen_step();
        self.reconstruction = None;
        self.reuse = None;
        Ok(())
    }

    /// An immutable prepared view cannot be rebound in place.  This explicit
    /// invalidation prevents accidental reuse after a compact-sample change.
    #[cfg(test)]
    pub(crate) fn invalidate_sample(&mut self, sample_generation: u64) -> Result<()> {
        require_newer(self.generations.sample, sample_generation, "sample")?;
        self.generations.sample = sample_generation;
        self.invalidate_prepared_alignment();
        Ok(())
    }

    /// FE topology changes have the same rebuild boundary as sample changes.
    #[cfg(test)]
    pub(crate) fn invalidate_topology(&mut self, topology_generation: u64) -> Result<()> {
        require_newer(self.generations.topology, topology_generation, "topology")?;
        self.generations.topology = topology_generation;
        self.invalidate_prepared_alignment();
        Ok(())
    }

    #[cfg(test)]
    fn invalidate_prepared_alignment(&mut self) {
        self.prepared_alignment_valid = false;
        self.frozen_basis = None;
        self.within = None;
        self.fit = None;
        self.recycle_frozen_step();
        self.reconstruction = None;
        self.reuse = None;
    }

    fn ensure_bound(&self) -> Result<()> {
        if self.prepared_alignment_valid {
            Ok(())
        } else {
            Err(invalid_argument(
                "weighted-linear session requires reconstruction after sample or topology change",
            ))
        }
    }

    fn recycle_frozen_step(&mut self) {
        if let Some(step) = self.frozen_step.take() {
            self.frozen_workspace.coefficients = step.coefficients;
            self.frozen_workspace.reported_coefficients = step.reported_coefficients;
            self.frozen_workspace.bread = step.bread;
            self.frozen_workspace.residuals = step.within_residuals;
        }
    }

    fn solve(&mut self, kind: LinearSessionSolveKind) -> Result<CertifiedLinearSessionOutput<'_>> {
        self.ensure_bound()?;
        let mut report = LinearSessionReuseReport::new(kind);
        report.response_buffer_reused = self.pending_response_buffer_reused;
        report.weight_buffer_reused = self.pending_weight_buffer_reused;

        let same_design = !self.projection_refresh_required
            && self
                .within
                .as_ref()
                .is_some_and(|within| within.generations.same_design(self.generations));
        let weight_refresh_candidate = self.within.as_ref().is_some_and(|within| {
            within
                .generations
                .eligible_for_weight_refresh(self.generations)
                || self.projection_refresh_required
                    && within.generations.sample == self.generations.sample
                    && within.generations.topology == self.generations.topology
                    && within.generations.basis == self.generations.basis
        });
        let mut projected = None;
        if kind == LinearSessionSolveKind::Warm && weight_refresh_candidate {
            report.warm_projection_attempted = true;
            match self.reproject_retained_within() {
                Ok(storage) => projected = Some(storage),
                Err(_) => report.canonical_fallback = true,
            }
        }
        let may_publish_retained =
            kind == LinearSessionSolveKind::Warm && (same_design || projected.is_some());
        let retain_fast_cache = !may_publish_retained;
        let frame = LinearSolveFrame::from_parts(
            &self.response,
            self.weights.borrowed(),
            self.bread_weight_scale,
        )?;
        let mut phase_allocation = LinearSessionPhaseAllocationObservation::default();
        let phase_observation = self.phase_allocator_probe.map(|_| &mut phase_allocation);
        let mut fit = fit_prepared_weighted_linear_once(
            self.prepared,
            frame,
            self.config,
            retain_fast_cache,
            self.phase_allocator_probe,
            phase_observation,
        )?;
        report.phase_allocation = phase_allocation;

        if let Some(basis) = &self.frozen_basis {
            if !basis.matches(&fit) {
                return Err(invalid_data(
                    "weighted-linear solve changed the frozen canonical rank basis",
                ));
            }
        }

        let candidate_basis = if self.frozen_basis.is_none() {
            Some(FrozenLinearBasis::from_fit(&fit)?)
        } else {
            None
        };
        let mut next_within = None;
        if kind == LinearSessionSolveKind::Warm && same_design {
            report.within_design_reused = true;
        } else if let Some(storage) = projected {
            let certificate = self.certify_within_design(&storage, &fit)?;
            if certificate.passed {
                report.within_design_reused = true;
                report.warm_projection_certified = true;
                next_within = Some(RetainedWithinDesign {
                    storage,
                    generations: WithinGenerations::from_session(self.generations),
                    certificate,
                });
            } else {
                report.canonical_fallback = true;
            }
        }

        if kind != LinearSessionSolveKind::Warm
            || !same_design && next_within.is_none()
            || report.canonical_fallback && next_within.is_none()
        {
            let cache = fit.take_any_fast_score_cache();
            let storage = self.build_canonical_within(&fit, cache)?;
            let certificate = self.certify_within_design(&storage, &fit)?;
            if !certificate.passed {
                return Err(invalid_data(
                    "canonical weighted-linear within design did not match certified bread",
                ));
            }
            next_within = Some(RetainedWithinDesign {
                storage,
                generations: WithinGenerations::from_session(self.generations),
                certificate,
            });
        }

        if same_design && kind == LinearSessionSolveKind::Warm {
            let within = self.within.as_ref().ok_or_else(|| {
                invalid_argument("weighted-linear retained design disappeared during warm solve")
            })?;
            if !within.certificate.passed {
                return Err(invalid_data(
                    "weighted-linear retained within certificate is not valid",
                ));
            }
        } else if let Some(within) = next_within.take() {
            let scratch_bytes = within.storage.scratch_bytes();
            fit.record_additional_scratch_peak(scratch_bytes);
            self.within = Some(within);
        }

        let predictor_workspace_reused = self.finite_predictor.capacity() >= self.response.len()
            && self.absorbed_predictor.capacity() >= self.response.len();
        let reconstruction = self.rebuild_predictors(&fit)?;
        report.predictor_workspace_reused = predictor_workspace_reused;
        report.certificate_workspace_reused = self.moment_workspace.capacity()
            >= fit
                .kept_generated_columns()
                .len()
                .saturating_mul(fit.kept_generated_columns().len());
        self.fit = Some(fit);
        self.recycle_frozen_step();
        if let Some(basis) = candidate_basis {
            self.frozen_basis = Some(basis);
        }
        self.reconstruction = Some(reconstruction);
        self.reuse = Some(report);
        self.pending_response_buffer_reused = false;
        self.pending_weight_buffer_reused = false;
        self.projection_refresh_required = false;
        self.output()
    }

    fn solve_frozen_internal(&mut self) -> Result<CertifiedFrozenLinearOutput<'_>> {
        self.ensure_bound()?;
        self.recycle_frozen_step();
        let basis = self.frozen_basis.as_ref().ok_or_else(|| {
            invalid_argument("frozen weighted-linear solve requires an initial fresh solve")
        })?;
        let rank = basis.kept_generated_columns().len();
        let slot_count = basis.slots().len();
        let kernel_workspace_reused =
            self.frozen_workspace
                .can_reuse(self.response.len(), rank, slot_count);
        self.frozen_workspace.kept.clear();
        self.frozen_workspace
            .kept
            .try_reserve(rank)
            .map_err(|_| allocation("unable to reserve frozen weighted-linear kept coordinates"))?;
        self.frozen_workspace
            .kept
            .extend_from_slice(basis.kept_generated_columns());
        self.frozen_workspace.basis_slots.clear();
        self.frozen_workspace
            .basis_slots
            .try_reserve(slot_count)
            .map_err(|_| {
                allocation("unable to reserve frozen weighted-linear reported coordinates")
            })?;
        self.frozen_workspace
            .basis_slots
            .extend_from_slice(basis.slots());
        let kept = std::mem::take(&mut self.frozen_workspace.kept);
        let basis_slots = std::mem::take(&mut self.frozen_workspace.basis_slots);
        let mut report = LinearSessionReuseReport::new(LinearSessionSolveKind::Warm);
        report.response_buffer_reused = self.pending_response_buffer_reused;
        report.weight_buffer_reused = self.pending_weight_buffer_reused;
        let phase_probe = self.phase_allocator_probe;
        let mut phase_allocation = LinearSessionPhaseAllocationObservation::default();
        let projection_phase = LinearSessionPhaseAllocationGuard::new(
            phase_probe,
            phase_probe.map(|_| &mut phase_allocation),
            LinearSessionAllocationPhase::Projection,
        );
        let projection_started = Instant::now();
        let reusable_map = self.config.absorb.solver == FeSolverKind::Map;
        let mut projection_workspace_reused = if reusable_map {
            self.fill_reusable_map_weights()?
        } else {
            false
        };

        let same_design = !self.projection_refresh_required
            && self
                .within
                .as_ref()
                .is_some_and(|within| within.generations.same_design(self.generations));
        if !same_design {
            let eligible = self.within.as_ref().is_some_and(|within| {
                within
                    .generations
                    .eligible_for_weight_refresh(self.generations)
                    || self.projection_refresh_required
                        && within.generations.sample == self.generations.sample
                        && within.generations.topology == self.generations.topology
                        && within.generations.basis == self.generations.basis
            });
            if !eligible {
                return Err(invalid_argument(
                    "frozen weighted-linear solve has no reusable design generation",
                ));
            }
            report.warm_projection_attempted = true;
            if reusable_map {
                projection_workspace_reused &=
                    self.reproject_retained_within_map().map_err(|error| {
                        FeregError::new(
                            error.code,
                            format!(
                                "frozen weighted-linear reusable MAP projection failed; strict canonical solve required: {}",
                                error.message
                            ),
                        )
                    })?;
            } else {
                let storage = self.reproject_retained_within().map_err(|error| {
                    FeregError::new(
                        error.code,
                        format!(
                            "frozen weighted-linear warm projection failed; strict canonical solve required: {}",
                            error.message
                        ),
                    )
                })?;
                self.within = Some(RetainedWithinDesign {
                    storage,
                    generations: WithinGenerations::from_session(self.generations),
                    certificate: LinearWithinDesignCertificate {
                        left_inverse_relative_infinity_norm: f64::INFINITY,
                        right_inverse_relative_infinity_norm: f64::INFINITY,
                        passed: false,
                    },
                });
                projection_workspace_reused = false;
            }
            report.warm_projection_certified = true;
        }
        report.within_design_reused = true;

        resize_zeroed(
            &mut self.frozen_workspace.response,
            self.response.len(),
            "unable to reserve frozen weighted-linear response absorption",
        )?;
        self.frozen_workspace
            .response
            .copy_from_slice(&self.response);
        let raw_response = DenseMatrix::from_row_major(
            self.response.len(),
            1,
            std::mem::take(&mut self.frozen_workspace.response),
        )?;
        let absorbed = if reusable_map {
            self.fe_projector.absorb(
                &self.map_weights,
                raw_response,
                self.config.absorb.map,
                self.prepared.fit_has_absorbed_intercept(),
            )?
        } else {
            self.prepared.fit_absorb_matrix(
                self.weights.borrowed(),
                raw_response,
                self.config.absorb,
            )?
        };
        if !absorbed.report().certified() {
            return Err(invalid_data(
                "frozen weighted-linear response absorption did not certify",
            ));
        }
        let (within_response_matrix, response_absorption_report) = absorbed.into_parts();
        drop(projection_phase);
        let within_response = within_response_matrix.as_slice();
        let within = self.within.as_ref().ok_or_else(|| {
            invalid_argument("frozen weighted-linear retained design disappeared")
        })?;
        if within.storage.rows() != self.response.len() || within.storage.columns() != kept.len() {
            return Err(invalid_argument(
                "frozen weighted-linear retained design dimensions are inconsistent",
            ));
        }

        let projection_seconds = projection_started.elapsed().as_secs_f64();
        let finite_started = Instant::now();
        let finite_phase = LinearSessionPhaseAllocationGuard::new(
            phase_probe,
            phase_probe.map(|_| &mut phase_allocation),
            LinearSessionAllocationPhase::FiniteSolve,
        );
        let frozen = stream_frozen_solve(
            &within.storage,
            within_response,
            self.weights.borrowed(),
            self.bread_weight_scale,
            self.config,
            &mut self.frozen_workspace,
        )?;
        drop(finite_phase);
        let finite_solve_seconds = finite_started.elapsed().as_secs_f64();
        self.frozen_workspace.response = within_response_matrix.into_row_major();
        if !frozen.certificate.passed {
            return Err(invalid_data(
                "frozen weighted-linear score or inverse certificate did not pass",
            ));
        }
        let within_certificate = LinearWithinDesignCertificate {
            left_inverse_relative_infinity_norm: frozen
                .certificate
                .left_inverse_relative_infinity_norm,
            right_inverse_relative_infinity_norm: frozen
                .certificate
                .right_inverse_relative_infinity_norm,
            passed: true,
        };
        self.within
            .as_mut()
            .ok_or_else(|| invalid_argument("frozen weighted-linear design is absent"))?
            .certificate = within_certificate;

        let predictor_workspace_reused = self.finite_predictor.capacity() >= self.response.len()
            && self.absorbed_predictor.capacity() >= self.response.len();
        let reconstruction =
            self.rebuild_frozen_predictors(&kept, &frozen.coefficients, &frozen.within_residuals)?;
        let mut reported_coefficients =
            std::mem::take(&mut self.frozen_workspace.reported_coefficients);
        self.build_frozen_reported_coefficients(
            &basis_slots,
            &kept,
            &frozen.coefficients,
            &mut reported_coefficients,
        )?;
        self.frozen_workspace.kept = kept;
        self.frozen_workspace.basis_slots = basis_slots;
        report.predictor_workspace_reused = predictor_workspace_reused;
        report.kernel_moment_workspace_reused = kernel_workspace_reused;
        report.kernel_solver_workspace_reused = kernel_workspace_reused;
        report.fe_projection_workspace_reused = projection_workspace_reused;
        report.steady_state_allocation_free = reusable_map
            && projection_workspace_reused
            && kernel_workspace_reused
            && predictor_workspace_reused;
        report.phase_timing = LinearSessionPhaseTiming {
            projection_seconds: Some(projection_seconds),
            finite_solve_seconds: Some(finite_solve_seconds),
        };
        report.phase_allocation = phase_allocation;
        self.fit = None;
        self.frozen_step = Some(CertifiedFrozenLinearStep {
            coefficients: frozen.coefficients,
            reported_coefficients,
            bread: frozen.bread,
            within_residuals: frozen.within_residuals,
            response_absorption_report,
            certificate: frozen.certificate,
            generations: self.generations,
        });
        self.reconstruction = Some(reconstruction);
        self.reuse = Some(report);
        self.pending_response_buffer_reused = false;
        self.pending_weight_buffer_reused = false;
        self.projection_refresh_required = false;
        self.frozen_output()
    }

    fn frozen_output(&self) -> Result<CertifiedFrozenLinearOutput<'_>> {
        let step = self.frozen_step.as_ref().ok_or_else(|| {
            invalid_argument("weighted-linear session has no current frozen solve")
        })?;
        if step.generations != self.generations {
            return Err(invalid_argument(
                "weighted-linear frozen solve has stale generations",
            ));
        }
        Ok(CertifiedFrozenLinearOutput {
            step,
            basis: self
                .frozen_basis
                .as_ref()
                .ok_or_else(|| invalid_argument("weighted-linear session has no frozen basis"))?,
            response: &self.response,
            finite_predictor: &self.finite_predictor,
            absorbed_predictor: &self.absorbed_predictor,
            within: self.within.as_ref().ok_or_else(|| {
                invalid_argument("weighted-linear session has no retained within design")
            })?,
            reconstruction: self.reconstruction.ok_or_else(|| {
                invalid_argument("weighted-linear frozen reconstruction is not certified")
            })?,
            reuse: self.reuse.ok_or_else(|| {
                invalid_argument("weighted-linear frozen solve has no reuse report")
            })?,
        })
    }

    fn rebuild_frozen_predictors(
        &mut self,
        kept: &[usize],
        coefficients: &[f64],
        within_residuals: &[f64],
    ) -> Result<LinearReconstructionCertificate> {
        if kept.len() != coefficients.len() || within_residuals.len() != self.response.len() {
            return Err(invalid_argument(
                "frozen weighted-linear predictor dimensions are inconsistent",
            ));
        }
        resize_zeroed(
            &mut self.finite_predictor,
            self.response.len(),
            "unable to reserve frozen weighted-linear finite predictor",
        )?;
        resize_zeroed(
            &mut self.absorbed_predictor,
            self.response.len(),
            "unable to reserve frozen weighted-linear absorbed predictor",
        )?;
        for (generated, coefficient) in kept.iter().copied().zip(coefficients.iter().copied()) {
            for row in 0..self.response.len() {
                self.finite_predictor[row] = coefficient.mul_add(
                    self.prepared.fit_column_value(generated, row)?,
                    self.finite_predictor[row],
                );
            }
        }
        let mut relative_infinity_norm = 0.0_f64;
        for (row, within_residual) in within_residuals.iter().copied().enumerate() {
            self.absorbed_predictor[row] =
                self.response[row] - self.finite_predictor[row] - within_residual;
            let reconstructed =
                self.finite_predictor[row] + self.absorbed_predictor[row] + within_residual;
            let scale = self.response[row].abs().max(reconstructed.abs()).max(1.0);
            relative_infinity_norm =
                relative_infinity_norm.max((reconstructed - self.response[row]).abs() / scale);
            if !self.finite_predictor[row].is_finite()
                || !self.absorbed_predictor[row].is_finite()
                || !relative_infinity_norm.is_finite()
            {
                return Err(invalid_data(
                    "frozen weighted-linear predictor reconstruction is not finite",
                ));
            }
        }
        Ok(LinearReconstructionCertificate {
            relative_infinity_norm,
            passed: relative_infinity_norm <= 64.0 * f64::EPSILON,
        })
    }

    fn build_frozen_reported_coefficients(
        &self,
        slots: &[FrozenLinearBasisSlot],
        kept: &[usize],
        coefficients: &[f64],
        reported: &mut Vec<Option<f64>>,
    ) -> Result<()> {
        if kept.len() != coefficients.len() {
            return Err(invalid_argument(
                "frozen weighted-linear reported coefficient dimensions are inconsistent",
            ));
        }
        let needs_normalized_intercept = slots
            .iter()
            .any(|slot| slot.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept);
        let normalized_intercept = if needs_normalized_intercept {
            let mut numerator = 0.0_f64;
            let mut denominator = 0.0_f64;
            for row in 0..self.response.len() {
                let weight = normalized_solver_weight(
                    self.weights.borrowed(),
                    self.bread_weight_scale,
                    row,
                )?;
                numerator =
                    weight.mul_add(self.response[row] - self.finite_predictor[row], numerator);
                denominator += weight;
            }
            let value = numerator / denominator;
            if !value.is_finite() || !denominator.is_finite() || denominator <= 0.0 {
                return Err(invalid_data(
                    "frozen weighted-linear normalized intercept is not finite",
                ));
            }
            Some(value)
        } else {
            None
        };
        reported.clear();
        reported.try_reserve(slots.len()).map_err(|_| {
            allocation("unable to reserve frozen weighted-linear reported coefficients")
        })?;
        reported.resize(slots.len(), None);
        for (target, slot) in reported.iter_mut().zip(slots) {
            *target = match slot.status {
                PreparedOlsFitStatus::Kept => {
                    let generated = slot.generated_column_index.ok_or_else(|| {
                        invalid_argument(
                            "frozen weighted-linear kept reported slot has no generated column",
                        )
                    })?;
                    let position = kept
                        .iter()
                        .position(|candidate| *candidate == generated)
                        .ok_or_else(|| {
                            invalid_argument(
                                "frozen weighted-linear kept reported slot is outside the basis",
                            )
                        })?;
                    Some(coefficients[position])
                }
                PreparedOlsFitStatus::NormalizedAbsorbedIntercept => normalized_intercept,
                PreparedOlsFitStatus::SyntaxOmitted
                | PreparedOlsFitStatus::AbsorbedInterceptDeferred
                | PreparedOlsFitStatus::CollinearWithFixedEffects
                | PreparedOlsFitStatus::NumericallyRankDeficient => None,
            };
        }
        Ok(())
    }

    fn build_canonical_within(
        &self,
        fit: &CertifiedPreparedOlsFit,
        cache: Option<PreparedOlsFastScoreCache>,
    ) -> Result<RetainedWithinStorage> {
        let kept = fit.kept_generated_columns();
        match self.config.execution.mode {
            PreparedOlsExecutionMode::Fast => {
                if let Some(cache) = cache {
                    self.dense_from_fast_cache(cache, kept)
                } else {
                    self.build_dense_from_raw_columns(kept)
                }
            }
            PreparedOlsExecutionMode::Low => self.build_scratch_from_raw_columns(kept),
        }
    }

    fn dense_from_fast_cache(
        &self,
        cache: PreparedOlsFastScoreCache,
        kept: &[usize],
    ) -> Result<RetainedWithinStorage> {
        let rows = self.prepared.fit_sample().n_active();
        let elements = checked_elements(rows, kept.len())?;
        let mut values = fallible_filled(
            elements,
            0.0,
            "unable to reserve weighted-linear packed within design",
        )?;
        match cache {
            PreparedOlsFastScoreCache::Packed {
                generated_columns,
                within,
                ..
            } => {
                for (target, generated) in kept.iter().copied().enumerate() {
                    let source = generated_columns
                        .iter()
                        .position(|candidate| *candidate == generated)
                        .ok_or_else(|| {
                            invalid_argument("weighted-linear fast cache is missing a kept column")
                        })?;
                    for row in 0..rows {
                        values[row * kept.len() + target] = within[(row, source)];
                    }
                }
            }
            PreparedOlsFastScoreCache::Strided {
                generated_columns,
                within,
                reference_norms,
                ..
            } => {
                for (target, generated) in kept.iter().copied().enumerate() {
                    let source = generated_columns
                        .iter()
                        .position(|candidate| *candidate == generated)
                        .ok_or_else(|| {
                            invalid_argument(
                                "weighted-linear strided cache is missing a kept column",
                            )
                        })?;
                    let norm = reference_norms[source];
                    for row in 0..rows {
                        values[row * kept.len() + target] =
                            within[(row, generated + 1)] * norm / self.root_solver_weight(row)?;
                    }
                }
            }
        }
        Ok(RetainedWithinStorage::Dense(DenseMatrix::from_row_major(
            rows,
            kept.len(),
            values,
        )?))
    }

    fn build_dense_from_raw_columns(&self, kept: &[usize]) -> Result<RetainedWithinStorage> {
        let rows = self.prepared.fit_sample().n_active();
        let elements = checked_elements(rows, kept.len())?;
        let mut values = fallible_filled(
            elements,
            0.0,
            "unable to reserve weighted-linear canonical dense design",
        )?;
        for row in 0..rows {
            for (column, generated) in kept.iter().copied().enumerate() {
                values[row * kept.len() + column] =
                    self.prepared.fit_column_value(generated, row)?;
            }
        }
        let raw = DenseMatrix::from_row_major(rows, kept.len(), values)?;
        let within = self.absorb_design(raw)?;
        Ok(RetainedWithinStorage::Dense(within))
    }

    fn build_scratch_from_raw_columns(&self, kept: &[usize]) -> Result<RetainedWithinStorage> {
        let rows = self.prepared.fit_sample().n_active();
        let columns = kept.len();
        let mut column_scratch = PreparedOlsScratch::create(rows, columns)?;
        let block_width = self.config.execution.poolsize.max(1).min(columns.max(1));
        let mut column_buffer = fallible_filled(
            rows,
            0.0,
            "unable to reserve weighted-linear scratch column buffer",
        )?;
        for start in (0..columns).step_by(block_width) {
            let end = start.saturating_add(block_width).min(columns);
            let block = end - start;
            let mut values = fallible_filled(
                checked_elements(rows, block)?,
                0.0,
                "unable to reserve weighted-linear scratch design block",
            )?;
            for row in 0..rows {
                for local in 0..block {
                    values[row * block + local] =
                        self.prepared.fit_column_value(kept[start + local], row)?;
                }
            }
            let raw = DenseMatrix::from_row_major(rows, block, values)?;
            let within = self.absorb_design(raw)?;
            for local in 0..block {
                for row in 0..rows {
                    column_buffer[row] = within[(row, local)];
                }
                column_scratch.write_column(start + local, &column_buffer)?;
            }
        }
        Ok(RetainedWithinStorage::Scratch(transpose_column_scratch(
            column_scratch,
        )?))
    }

    fn fill_reusable_map_weights(&mut self) -> Result<bool> {
        let rows = self.response.len();
        let reused = self.map_weights.capacity() >= rows;
        self.map_weights.resize(rows, 0.0);
        match self.weights.borrowed() {
            BorrowedMapWeights::Unit => self.map_weights.fill(1.0),
            BorrowedMapWeights::Provided(values) => {
                let divisor = self.bread_weight_scale.solver_weight_divisor;
                if !divisor.is_finite() || divisor <= 0.0 || values.len() != rows {
                    return Err(invalid_argument(
                        "weighted-linear reusable MAP weights are not aligned",
                    ));
                }
                for (target, &value) in self.map_weights.iter_mut().zip(values) {
                    *target = value / divisor;
                    if !target.is_finite() || *target <= 0.0 {
                        return Err(invalid_data(
                            "weighted-linear reusable MAP weight is invalid",
                        ));
                    }
                }
            }
        }
        Ok(reused)
    }

    /// Re-project the retained equivalence-class representative in place.
    /// Since `M_new D = 0`, projecting the prior `M_old X` under new weights
    /// yields exactly the same within coordinate as projecting raw `X`, while
    /// preserving the already-owned dense buffer or scratch-file identity.
    fn reproject_retained_within_map(&mut self) -> Result<bool> {
        let prior = self.within.take().ok_or_else(|| {
            invalid_argument("weighted-linear reusable MAP projection has no retained design")
        })?;
        let rows = prior.storage.rows();
        let columns = prior.storage.columns();
        if rows != self.response.len() {
            return Err(invalid_argument(
                "weighted-linear reusable MAP projection exceeds its fixed workspace",
            ));
        }
        let mut workspace_reused = true;
        let storage = match prior.storage {
            RetainedWithinStorage::Dense(values) => {
                if columns == 0 {
                    RetainedWithinStorage::Dense(values)
                } else {
                    if columns > self.fe_projector.max_rhs() {
                        return Err(invalid_argument(
                            "weighted-linear reusable dense MAP projection exceeds its fixed workspace",
                        ));
                    }
                    let absorbed = self.fe_projector.absorb(
                        &self.map_weights,
                        values,
                        self.config.absorb.map,
                        self.prepared.fit_has_absorbed_intercept(),
                    )?;
                    let (values, report) = absorbed.into_parts();
                    if !report.certified() {
                        return Err(invalid_data(
                            "weighted-linear reusable dense projection did not certify",
                        ));
                    }
                    RetainedWithinStorage::Dense(values)
                }
            }
            RetainedWithinStorage::Scratch(mut scratch) => {
                let block_width = self.config.execution.poolsize.max(1).min(columns.max(1));
                if block_width > self.fe_projector.max_rhs() {
                    return Err(invalid_argument(
                        "weighted-linear reusable scratch MAP projection exceeds its fixed workspace",
                    ));
                }
                let maximum_elements = checked_elements(rows, block_width)?;
                workspace_reused = self.projection_block.capacity() >= maximum_elements;
                for start in (0..columns).step_by(block_width) {
                    let end = start.saturating_add(block_width).min(columns);
                    let block = end - start;
                    resize_zeroed(
                        &mut self.projection_block,
                        checked_elements(rows, block)?,
                        "unable to reserve weighted-linear reusable low-memory projection block",
                    )?;
                    for row in 0..rows {
                        let source = row
                            .checked_mul(columns)
                            .and_then(|offset| offset.checked_add(start))
                            .ok_or_else(|| {
                                allocation("weighted-linear reusable scratch read offset overflows")
                            })?;
                        scratch.read_linear_shared(
                            source,
                            &mut self.projection_block[row * block..(row + 1) * block],
                        )?;
                    }
                    let raw = DenseMatrix::from_row_major(
                        rows,
                        block,
                        std::mem::take(&mut self.projection_block),
                    )?;
                    let absorbed = self.fe_projector.absorb(
                        &self.map_weights,
                        raw,
                        self.config.absorb.map,
                        self.prepared.fit_has_absorbed_intercept(),
                    )?;
                    let (values, report) = absorbed.into_parts();
                    if !report.certified() {
                        return Err(invalid_data(
                            "weighted-linear reusable scratch projection did not certify",
                        ));
                    }
                    self.projection_block = values.into_row_major();
                    for row in 0..rows {
                        let destination = row
                            .checked_mul(columns)
                            .and_then(|offset| offset.checked_add(start))
                            .ok_or_else(|| {
                                allocation(
                                    "weighted-linear reusable scratch write offset overflows",
                                )
                            })?;
                        scratch.write_linear(
                            destination,
                            &self.projection_block[row * block..(row + 1) * block],
                        )?;
                    }
                }
                RetainedWithinStorage::Scratch(scratch)
            }
        };
        self.within = Some(RetainedWithinDesign {
            storage,
            generations: WithinGenerations::from_session(self.generations),
            certificate: LinearWithinDesignCertificate {
                left_inverse_relative_infinity_norm: f64::INFINITY,
                right_inverse_relative_infinity_norm: f64::INFINITY,
                passed: false,
            },
        });
        Ok(workspace_reused)
    }

    fn reproject_retained_within(&self) -> Result<RetainedWithinStorage> {
        let prior = self.within.as_ref().ok_or_else(|| {
            invalid_argument("weighted-linear warm projection has no retained design")
        })?;
        let rows = prior.storage.rows();
        let columns = prior.storage.columns();
        match self.config.execution.mode {
            PreparedOlsExecutionMode::Fast => {
                let mut values = fallible_filled(
                    checked_elements(rows, columns)?,
                    0.0,
                    "unable to reserve weighted-linear warm projection",
                )?;
                let mut row_values = fallible_filled(
                    columns,
                    0.0,
                    "unable to reserve weighted-linear warm projection row",
                )?;
                for row in 0..rows {
                    prior.storage.fill_row(row, &mut row_values)?;
                    values[row * columns..(row + 1) * columns].copy_from_slice(&row_values);
                }
                let raw = DenseMatrix::from_row_major(rows, columns, values)?;
                Ok(RetainedWithinStorage::Dense(self.absorb_design(raw)?))
            }
            PreparedOlsExecutionMode::Low => {
                let mut column_scratch = PreparedOlsScratch::create(rows, columns)?;
                let block_width = self.config.execution.poolsize.max(1).min(columns.max(1));
                let mut full_row = fallible_filled(
                    columns,
                    0.0,
                    "unable to reserve weighted-linear warm scratch row",
                )?;
                let mut column_buffer = fallible_filled(
                    rows,
                    0.0,
                    "unable to reserve weighted-linear warm scratch column",
                )?;
                for start in (0..columns).step_by(block_width) {
                    let end = start.saturating_add(block_width).min(columns);
                    let block = end - start;
                    let mut values = fallible_filled(
                        checked_elements(rows, block)?,
                        0.0,
                        "unable to reserve weighted-linear warm scratch block",
                    )?;
                    for row in 0..rows {
                        prior.storage.fill_row(row, &mut full_row)?;
                        values[row * block..(row + 1) * block]
                            .copy_from_slice(&full_row[start..end]);
                    }
                    let raw = DenseMatrix::from_row_major(rows, block, values)?;
                    let within = self.absorb_design(raw)?;
                    for local in 0..block {
                        for row in 0..rows {
                            column_buffer[row] = within[(row, local)];
                        }
                        column_scratch.write_column(start + local, &column_buffer)?;
                    }
                }
                Ok(RetainedWithinStorage::Scratch(transpose_column_scratch(
                    column_scratch,
                )?))
            }
        }
    }

    fn absorb_design(&self, values: DenseMatrix) -> Result<DenseMatrix> {
        if values.ncols() == 0 {
            return Ok(values);
        }
        let absorbed =
            self.prepared
                .fit_absorb_matrix(self.weights.borrowed(), values, self.config.absorb)?;
        if !absorbed.report().certified() {
            return Err(invalid_data(
                "weighted-linear session design absorption did not certify",
            ));
        }
        Ok(absorbed.into_parts().0)
    }

    fn certify_within_design(
        &mut self,
        storage: &RetainedWithinStorage,
        fit: &CertifiedPreparedOlsFit,
    ) -> Result<LinearWithinDesignCertificate> {
        let kept = fit.kept_generated_columns();
        if storage.rows() != self.response.len() || storage.columns() != kept.len() {
            return Err(invalid_argument(
                "weighted-linear within certificate dimensions are inconsistent",
            ));
        }
        let dimension = fit.bread().dimension();
        let mut bread_positions = fallible_filled(
            kept.len(),
            0_usize,
            "unable to reserve weighted-linear bread positions",
        )?;
        for (target, generated) in kept.iter().copied().enumerate() {
            bread_positions[target] = fit
                .bread()
                .coordinates()
                .iter()
                .position(|coordinate| {
                    coordinate.generated_column_index == generated
                        && coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                })
                .ok_or_else(|| {
                    invalid_argument("weighted-linear bread is missing a kept finite coordinate")
                })?;
        }
        let square = checked_elements(kept.len(), kept.len())?;
        let workspace_reused = self.moment_workspace.capacity() >= square;
        self.moment_workspace.clear();
        self.moment_workspace
            .try_reserve(square)
            .map_err(|_| allocation("unable to reserve weighted-linear certificate moments"))?;
        self.moment_workspace.resize(square, 0.0);
        self.row_workspace.clear();
        self.row_workspace
            .try_reserve(kept.len())
            .map_err(|_| allocation("unable to reserve weighted-linear certificate row"))?;
        self.row_workspace.resize(kept.len(), 0.0);
        for row in 0..storage.rows() {
            storage.fill_row(row, &mut self.row_workspace)?;
            let weight = self.reference_weight(row)?;
            for left in 0..kept.len() {
                for right in 0..kept.len() {
                    let term = weight * self.row_workspace[left] * self.row_workspace[right];
                    let target = &mut self.moment_workspace[left * kept.len() + right];
                    *target += term;
                    if !target.is_finite() {
                        return Err(invalid_data("weighted-linear within moment is not finite"));
                    }
                }
            }
        }
        let mut left_error = 0.0_f64;
        let mut right_error = 0.0_f64;
        for row in 0..kept.len() {
            let mut left_row_error = 0.0_f64;
            let mut right_row_error = 0.0_f64;
            let mut left_scale = 1.0_f64;
            let mut right_scale = 1.0_f64;
            for column in 0..kept.len() {
                let mut left = 0.0_f64;
                let mut right = 0.0_f64;
                for inner in 0..kept.len() {
                    let bread_left = fit.bread().values()
                        [bread_positions[row] * dimension + bread_positions[inner]];
                    let bread_right = fit.bread().values()
                        [bread_positions[inner] * dimension + bread_positions[column]];
                    left += bread_left * self.moment_workspace[inner * kept.len() + column];
                    right += self.moment_workspace[row * kept.len() + inner] * bread_right;
                }
                let target = f64::from(row == column);
                left_row_error += (left - target).abs();
                right_row_error += (right - target).abs();
                left_scale += left.abs();
                right_scale += right.abs();
            }
            left_error = left_error.max(left_row_error / left_scale);
            right_error = right_error.max(right_row_error / right_scale);
        }
        if !left_error.is_finite() || !right_error.is_finite() {
            return Err(invalid_data(
                "weighted-linear within certificate is not finite",
            ));
        }
        let passed =
            left_error <= self.config.bread_tolerance && right_error <= self.config.bread_tolerance;
        if let Some(report) = self.reuse.as_mut() {
            report.certificate_workspace_reused |= workspace_reused;
        }
        Ok(LinearWithinDesignCertificate {
            left_inverse_relative_infinity_norm: left_error,
            right_inverse_relative_infinity_norm: right_error,
            passed,
        })
    }

    fn rebuild_predictors(
        &mut self,
        fit: &CertifiedPreparedOlsFit,
    ) -> Result<LinearReconstructionCertificate> {
        let rows = self.response.len();
        resize_zeroed(
            &mut self.finite_predictor,
            rows,
            "unable to reserve weighted-linear finite predictor",
        )?;
        resize_zeroed(
            &mut self.absorbed_predictor,
            rows,
            "unable to reserve weighted-linear absorbed predictor",
        )?;
        for coefficient in fit.coefficients() {
            if coefficient.status != PreparedOlsFitStatus::Kept {
                continue;
            }
            let generated = coefficient.generated_column_index.ok_or_else(|| {
                invalid_argument("kept weighted-linear coefficient has no generated column")
            })?;
            let value = coefficient
                .value
                .ok_or_else(|| invalid_argument("kept weighted-linear coefficient has no value"))?;
            for row in 0..rows {
                self.finite_predictor[row] = value.mul_add(
                    self.prepared.fit_column_value(generated, row)?,
                    self.finite_predictor[row],
                );
            }
        }
        let mut relative_infinity_norm = 0.0_f64;
        for row in 0..rows {
            self.absorbed_predictor[row] =
                self.response[row] - self.finite_predictor[row] - fit.residuals()[row];
            let reconstructed =
                self.finite_predictor[row] + self.absorbed_predictor[row] + fit.residuals()[row];
            let scale = self.response[row].abs().max(reconstructed.abs()).max(1.0);
            relative_infinity_norm =
                relative_infinity_norm.max((reconstructed - self.response[row]).abs() / scale);
            if !self.finite_predictor[row].is_finite()
                || !self.absorbed_predictor[row].is_finite()
                || !relative_infinity_norm.is_finite()
            {
                return Err(invalid_data(
                    "weighted-linear predictor reconstruction is not finite",
                ));
            }
        }
        Ok(LinearReconstructionCertificate {
            relative_infinity_norm,
            passed: relative_infinity_norm <= 64.0 * f64::EPSILON,
        })
    }

    fn root_solver_weight(&self, row: usize) -> Result<f64> {
        let weight = match self.weights.borrowed() {
            BorrowedMapWeights::Unit => 1.0,
            BorrowedMapWeights::Provided(values) => {
                values[row] / self.bread_weight_scale.solver_weight_divisor
            }
        };
        let root = weight.sqrt();
        if root.is_finite() && root > 0.0 {
            Ok(root)
        } else {
            Err(invalid_data(
                "weighted-linear normalized root weight is invalid",
            ))
        }
    }

    fn reference_weight(&self, row: usize) -> Result<f64> {
        let weight = self.root_solver_weight(row)?;
        let reference = weight * weight * self.bread_weight_scale.reference_weight_multiplier;
        if reference.is_finite() && reference > 0.0 {
            Ok(reference)
        } else {
            Err(invalid_data("weighted-linear reference weight is invalid"))
        }
    }
}

struct FrozenSolveDraft {
    coefficients: Vec<f64>,
    bread: Vec<f64>,
    within_residuals: Vec<f64>,
    certificate: FrozenLinearSolveCertificate,
}

fn stream_frozen_solve(
    storage: &RetainedWithinStorage,
    within_response: &[f64],
    weights: BorrowedMapWeights<'_>,
    bread_weight_scale: super::PreparedOlsBreadWeightScale,
    config: PreparedOlsFitConfig,
    workspace: &mut FrozenLinearWorkspace,
) -> Result<FrozenSolveDraft> {
    let rows = storage.rows();
    let rank = storage.columns();
    if within_response.len() != rows {
        return Err(invalid_argument(
            "frozen weighted-linear response and design axes are inconsistent",
        ));
    }
    let square = checked_elements(rank, rank)?;
    resize_zeroed(
        &mut workspace.gram,
        square,
        "unable to reserve frozen weighted-linear Gram",
    )?;
    resize_zeroed(
        &mut workspace.rhs,
        rank,
        "unable to reserve frozen weighted-linear RHS moments",
    )?;
    resize_zeroed(
        &mut workspace.row,
        rank,
        "unable to reserve frozen weighted-linear row buffer",
    )?;
    for (row, response) in within_response.iter().copied().enumerate().take(rows) {
        storage.fill_row(row, &mut workspace.row)?;
        let weight = normalized_solver_weight(weights, bread_weight_scale, row)?;
        for left in 0..rank {
            workspace.rhs[left] =
                (weight * workspace.row[left]).mul_add(response, workspace.rhs[left]);
            for right in left..rank {
                workspace.gram[left * rank + right] = (weight * workspace.row[left])
                    .mul_add(workspace.row[right], workspace.gram[left * rank + right]);
            }
        }
    }
    for left in 0..rank {
        for right in 0..left {
            workspace.gram[left * rank + right] = workspace.gram[right * rank + left];
        }
    }
    if workspace
        .gram
        .iter()
        .chain(&workspace.rhs)
        .any(|value| !value.is_finite())
    {
        return Err(invalid_data(
            "frozen weighted-linear moments are not finite",
        ));
    }

    resize_zeroed(
        &mut workspace.factor,
        square,
        "unable to reserve frozen weighted-linear Cholesky factor",
    )?;
    let maximum_diagonal = (0..rank)
        .map(|index| workspace.gram[index * rank + index])
        .fold(0.0_f64, f64::max);
    let relative_threshold =
        config.rank_tolerance_multiplier * f64::EPSILON * rows.max(rank).max(1) as f64;
    let pivot_threshold = maximum_diagonal * relative_threshold * relative_threshold;
    let mut minimum_pivot = f64::INFINITY;
    let mut maximum_pivot = 0.0_f64;
    for row in 0..rank {
        for column in 0..=row {
            let mut value: f64 = workspace.gram[row * rank + column];
            for inner in 0..column {
                value = (-workspace.factor[row * rank + inner])
                    .mul_add(workspace.factor[column * rank + inner], value);
            }
            if row == column {
                if !value.is_finite() || value <= pivot_threshold.max(0.0) {
                    return Err(invalid_data("frozen weighted-linear basis lost full rank"));
                }
                let diagonal = value.sqrt();
                workspace.factor[row * rank + column] = diagonal;
                minimum_pivot = minimum_pivot.min(value);
                maximum_pivot = maximum_pivot.max(value);
            } else {
                let diagonal = workspace.factor[column * rank + column];
                let entry = value / diagonal;
                if !entry.is_finite() {
                    return Err(invalid_data(
                        "frozen weighted-linear Cholesky factor is not finite",
                    ));
                }
                workspace.factor[row * rank + column] = entry;
            }
        }
    }
    let condition_proxy = if rank == 0 {
        1.0
    } else {
        maximum_pivot / minimum_pivot
    };
    if !condition_proxy.is_finite() {
        return Err(invalid_data(
            "frozen weighted-linear condition proxy is not finite",
        ));
    }

    workspace.coefficients.clear();
    workspace
        .coefficients
        .try_reserve(rank)
        .map_err(|_| allocation("unable to reserve frozen weighted-linear coefficients"))?;
    workspace.coefficients.extend_from_slice(&workspace.rhs);
    solve_cholesky_in_place(&workspace.factor, rank, &mut workspace.coefficients)?;
    resize_zeroed(
        &mut workspace.bread,
        square,
        "unable to reserve frozen weighted-linear inverse",
    )?;
    resize_zeroed(
        &mut workspace.solve,
        rank,
        "unable to reserve frozen weighted-linear inverse RHS",
    )?;
    for column in 0..rank {
        workspace.solve.fill(0.0);
        workspace.solve[column] = 1.0;
        solve_cholesky_in_place(&workspace.factor, rank, &mut workspace.solve)?;
        for row in 0..rank {
            workspace.bread[row * rank + column] = workspace.solve[row];
        }
    }
    for row in 0..rank {
        for column in 0..row {
            let symmetric =
                0.5 * (workspace.bread[row * rank + column] + workspace.bread[column * rank + row]);
            workspace.bread[row * rank + column] = symmetric;
            workspace.bread[column * rank + row] = symmetric;
        }
    }
    let (left_inverse_error, right_inverse_error) =
        inverse_errors(&workspace.gram, &workspace.bread, rank)?;

    workspace.residuals.clear();
    workspace
        .residuals
        .try_reserve(rows)
        .map_err(|_| allocation("unable to reserve frozen weighted-linear residuals"))?;
    workspace.residuals.extend_from_slice(within_response);
    let mut weighted_residual_square = 0.0_f64;
    let mut maximum_score = 0.0_f64;
    resize_zeroed(
        &mut workspace.scores,
        rank,
        "unable to reserve frozen weighted-linear scores",
    )?;
    for row in 0..rows {
        storage.fill_row(row, &mut workspace.row)?;
        let mut fitted = 0.0_f64;
        for column in 0..rank {
            fitted = workspace.coefficients[column].mul_add(workspace.row[column], fitted);
        }
        workspace.residuals[row] -= fitted;
        let weight = normalized_solver_weight(weights, bread_weight_scale, row)?;
        weighted_residual_square = (weight * workspace.residuals[row])
            .mul_add(workspace.residuals[row], weighted_residual_square);
        for column in 0..rank {
            workspace.scores[column] = (weight * workspace.row[column])
                .mul_add(workspace.residuals[row], workspace.scores[column]);
        }
    }
    let residual_norm = weighted_residual_square.max(0.0).sqrt();
    for column in 0..rank {
        let column_norm = workspace.gram[column * rank + column].max(0.0).sqrt();
        let denominator = (column_norm * residual_norm)
            .max(workspace.rhs[column].abs())
            .max(1.0);
        maximum_score = maximum_score.max(workspace.scores[column].abs() / denominator);
    }
    if workspace
        .residuals
        .iter()
        .chain(&workspace.scores)
        .any(|value| !value.is_finite())
        || !maximum_score.is_finite()
    {
        return Err(invalid_data(
            "frozen weighted-linear residual or score is not finite",
        ));
    }
    let passed = maximum_score <= config.score_tolerance
        && left_inverse_error <= config.bread_tolerance
        && right_inverse_error <= config.bread_tolerance;
    let reference_multiplier = bread_weight_scale.reference_weight_multiplier;
    for value in &mut workspace.bread {
        *value /= reference_multiplier;
        if !value.is_finite() {
            return Err(invalid_data("frozen weighted-linear bread is not finite"));
        }
    }
    Ok(FrozenSolveDraft {
        coefficients: std::mem::take(&mut workspace.coefficients),
        bread: std::mem::take(&mut workspace.bread),
        within_residuals: std::mem::take(&mut workspace.residuals),
        certificate: FrozenLinearSolveCertificate {
            score_relative_infinity_norm: maximum_score,
            left_inverse_relative_infinity_norm: left_inverse_error,
            right_inverse_relative_infinity_norm: right_inverse_error,
            condition_proxy,
            passed,
        },
    })
}

fn solve_cholesky_in_place(factor: &[f64], rank: usize, solved: &mut [f64]) -> Result<()> {
    if factor.len() != rank.saturating_mul(rank) || solved.len() != rank {
        return Err(invalid_argument(
            "frozen weighted-linear factor solve dimensions are inconsistent",
        ));
    }
    for row in 0..rank {
        for column in 0..row {
            solved[row] = (-factor[row * rank + column]).mul_add(solved[column], solved[row]);
        }
        solved[row] /= factor[row * rank + row];
    }
    for row in (0..rank).rev() {
        for column in (row + 1)..rank {
            solved[row] = (-factor[column * rank + row]).mul_add(solved[column], solved[row]);
        }
        solved[row] /= factor[row * rank + row];
        if !solved[row].is_finite() {
            return Err(invalid_data(
                "frozen weighted-linear solution is not finite",
            ));
        }
    }
    Ok(())
}

fn inverse_errors(gram: &[f64], inverse: &[f64], rank: usize) -> Result<(f64, f64)> {
    if gram.len() != rank.saturating_mul(rank) || inverse.len() != gram.len() {
        return Err(invalid_argument(
            "frozen weighted-linear inverse dimensions are inconsistent",
        ));
    }
    let mut left_error = 0.0_f64;
    let mut right_error = 0.0_f64;
    for row in 0..rank {
        let mut left_row_error = 0.0_f64;
        let mut right_row_error = 0.0_f64;
        let mut left_scale = 1.0_f64;
        let mut right_scale = 1.0_f64;
        for column in 0..rank {
            let mut left = 0.0_f64;
            let mut right = 0.0_f64;
            for inner in 0..rank {
                left += gram[row * rank + inner] * inverse[inner * rank + column];
                right += inverse[row * rank + inner] * gram[inner * rank + column];
            }
            let target = f64::from(row == column);
            left_row_error += (left - target).abs();
            right_row_error += (right - target).abs();
            left_scale += left.abs();
            right_scale += right.abs();
        }
        left_error = left_error.max(left_row_error / left_scale);
        right_error = right_error.max(right_row_error / right_scale);
    }
    if left_error.is_finite() && right_error.is_finite() {
        Ok((left_error, right_error))
    } else {
        Err(invalid_data(
            "frozen weighted-linear inverse certificate is not finite",
        ))
    }
}

fn normalized_solver_weight(
    weights: BorrowedMapWeights<'_>,
    bread_weight_scale: super::PreparedOlsBreadWeightScale,
    row: usize,
) -> Result<f64> {
    let value = match weights {
        BorrowedMapWeights::Unit => 1.0,
        BorrowedMapWeights::Provided(values) => {
            values.get(row).copied().ok_or_else(|| {
                invalid_argument("frozen weighted-linear weight row is out of range")
            })? / bread_weight_scale.solver_weight_divisor
        }
    };
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid_data(
            "frozen weighted-linear normalized weight is invalid",
        ))
    }
}

fn fill_identified_bread(
    fit: &CertifiedPreparedOlsFit,
    basis: &FrozenLinearBasis,
    output: &mut Vec<f64>,
) -> Result<()> {
    let kept = basis.kept_generated_columns();
    if fit.kept_generated_columns() != kept {
        return Err(invalid_argument(
            "weighted-linear final bread basis differs from the frozen basis",
        ));
    }
    let rank = kept.len();
    let square = checked_elements(rank, rank)?;
    resize_zeroed(
        output,
        square,
        "unable to reserve weighted-linear compact identified bread",
    )?;
    let coordinates = fit.bread().coordinates();
    let values = fit.bread().values();
    let dimension = fit.bread().dimension();
    for (target_row, generated_row) in kept.iter().copied().enumerate() {
        let source_row = coordinates
            .iter()
            .position(|coordinate| {
                coordinate.generated_column_index == generated_row
                    && coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
            })
            .ok_or_else(|| {
                invalid_argument("weighted-linear final bread is missing an identified row")
            })?;
        for (target_column, generated_column) in kept.iter().copied().enumerate() {
            let source_column = coordinates
                .iter()
                .position(|coordinate| {
                    coordinate.generated_column_index == generated_column
                        && coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                })
                .ok_or_else(|| {
                    invalid_argument("weighted-linear final bread is missing an identified column")
                })?;
            output[target_row * rank + target_column] =
                values[source_row * dimension + source_column];
        }
    }
    Ok(())
}

fn transpose_column_scratch(mut columns: PreparedOlsScratch) -> Result<PreparedOlsScratch> {
    let rows = columns.rows();
    let rank = columns.columns();
    let mut row_major = PreparedOlsScratch::create(rows, rank)?;
    if rank == 0 || rows == 0 {
        return Ok(row_major);
    }
    let target_elements = (SCRATCH_TRANSPOSE_BYTES / 8).max(rank);
    let tile_rows = (target_elements / rank).max(1).min(rows);
    let mut tile = fallible_filled(
        checked_elements(tile_rows, rank)?,
        0.0,
        "unable to reserve weighted-linear scratch transpose tile",
    )?;
    let mut column = fallible_filled(
        tile_rows,
        0.0,
        "unable to reserve weighted-linear scratch transpose column",
    )?;
    for start_row in (0..rows).step_by(tile_rows) {
        let rows_in_tile = (rows - start_row).min(tile_rows);
        for finite in 0..rank {
            columns.read_column_range(finite, start_row, &mut column[..rows_in_tile])?;
            for row in 0..rows_in_tile {
                tile[row * rank + finite] = column[row];
            }
        }
        row_major.write_linear(
            start_row * rank,
            &tile[..rows_in_tile
                .checked_mul(rank)
                .ok_or_else(|| allocation("weighted-linear scratch transpose length overflows"))?],
        )?;
    }
    Ok(row_major)
}

fn require_newer(current: u64, next: u64, label: &'static str) -> Result<()> {
    if next > current {
        Ok(())
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidArgument,
            format!("weighted-linear {label} generation must increase"),
        ))
    }
}

fn checked_elements(rows: usize, columns: usize) -> Result<usize> {
    rows.checked_mul(columns)
        .ok_or_else(|| allocation("weighted-linear matrix dimensions overflow"))
}

fn fallible_empty<T>(capacity: usize, message: &'static str) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| allocation(message))?;
    Ok(values)
}

fn fallible_copy<T: Copy>(values: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut copied = fallible_empty(values.len(), message)?;
    copied.extend_from_slice(values);
    Ok(copied)
}

fn fallible_filled<T: Clone>(length: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut values = fallible_empty(length, message)?;
    values.resize(length, value);
    Ok(values)
}

fn resize_zeroed(target: &mut Vec<f64>, length: usize, message: &'static str) -> Result<()> {
    target.clear();
    target
        .try_reserve(length)
        .map_err(|_| allocation(message))?;
    target.resize(length, 0.0);
    Ok(())
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}
