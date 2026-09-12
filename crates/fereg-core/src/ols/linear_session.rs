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
use crate::fe::{
    BorrowedMapWeights, CertifiedBorrowedMap, FeDesign, FeSolverKind,
    ReusableCategoricalMapAbsorber, ReusableMapAbsorber, ReusableMapCapacities, SolveReport,
};
use crate::metadata::CoefficientSlot;
use crate::parallel::ThreadTeam;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::sandwich::SandwichDesign;
#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::finite_wls_kernel::{
    certify_normal_equations, certify_retained_qr_dropped_span, factor_guarded_cholesky_in_place,
    factor_rank_revealing_retained_qr, fill_finite_wls_moments, finite_wls_moment_layout,
    inverse_relative_infinity_norm, invert_guarded_cholesky, invert_rank_revealing_qr_leading,
    solve_guarded_cholesky, solve_rank_revealing_qr_leading, CholeskyPivotPolicy,
    FixedBlockMomentSource, NormalEquationScale,
};
#[cfg(test)]
use super::prepared_fit::PreparedOlsFastScoreCache;
use super::prepared_fit::{
    assemble_retained_weighted_linear_fit, merge_solve_report, LinearSolveFrame,
    RetainedFiniteFitDraft,
};
use super::prepared_scratch::PreparedOlsScratch;
use super::{
    CertifiedPreparedOlsFit, PreparedOlsBreadCoordinateKind, PreparedOlsColumnKind,
    PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsFitStatus,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrozenLinearOutputKind {
    #[cfg(test)]
    FullOlsDecomposition,
    PpmlCombinedPredictor,
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

    /// PPML-only specialized-solve accounting. The ordinary-frame engines
    /// reuse fit-lifetime design and response buffers, while their upstream
    /// solvers can still allocate per call; the stronger allocation-free flags
    /// therefore remain false.
    pub(crate) fn ppml_specialized(projection_seconds: f64, finite_solve_seconds: f64) -> Self {
        let mut report = Self::new(LinearSessionSolveKind::Warm);
        report.response_buffer_reused = true;
        report.weight_buffer_reused = true;
        report.within_design_reused = true;
        report.warm_projection_attempted = true;
        report.warm_projection_certified = true;
        report.certificate_workspace_reused = true;
        report.predictor_workspace_reused = true;
        report.phase_timing.projection_seconds = Some(projection_seconds);
        report.phase_timing.finite_solve_seconds = Some(finite_solve_seconds);
        report
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
    // Keep the response-relative diagnostic even when cancellation requires a
    // larger, componentwise rounding allowance for the acceptance check.
    pub(crate) relative_infinity_norm: f64,
    pub(crate) passed: bool,
}

impl LinearReconstructionCertificate {
    fn new() -> Self {
        Self {
            relative_infinity_norm: 0.0,
            passed: true,
        }
    }

    fn observe(&mut self, response: f64, finite: f64, absorbed: f64, residual: f64) -> Result<()> {
        let reconstructed = (finite + absorbed) + residual;
        let error = (reconstructed - response).abs();
        if ![response, finite, absorbed, residual, reconstructed, error]
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(invalid_data(
                "weighted-linear predictor reconstruction is not finite",
            ));
        }
        self.relative_infinity_norm = self
            .relative_infinity_norm
            .max(error / response.abs().max(reconstructed.abs()).max(1.0));
        // Forming the absorbed predictor takes two subtractions; reconstructing
        // the response and measuring its discrepancy take three more operations.
        // gamma(8) covers these roundings and evaluation of the scaled bound.
        // Scaling each magnitude first prevents overflow in their sum. The unit
        // floor retains the existing absolute guard for tiny working responses.
        let unit_roundoff = 0.5 * f64::EPSILON;
        let gamma = (8.0 * unit_roundoff) / (1.0 - 8.0 * unit_roundoff);
        let scale = response
            .abs()
            .max(finite.abs())
            .max(absorbed.abs())
            .max(residual.abs())
            .max(1.0);
        let magnitude = (response.abs() / scale
            + finite.abs() / scale
            + absorbed.abs() / scale
            + residual.abs() / scale)
            .max(1.0 / scale);
        self.passed &= error / scale <= gamma * magnitude;
        Ok(())
    }
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
    pub(crate) frozen_inverse_lower_elements: usize,
    pub(crate) frozen_rhs_elements: usize,
    pub(crate) frozen_moment_partial_elements: usize,
    pub(crate) frozen_row_elements: usize,
    pub(crate) frozen_solve_elements: usize,
    pub(crate) frozen_permutation_elements: usize,
    pub(crate) frozen_score_elements: usize,
    pub(crate) frozen_coefficient_elements: usize,
    pub(crate) frozen_bread_elements: usize,
    pub(crate) frozen_residual_elements: usize,
    pub(crate) frozen_reported_coefficient_elements: usize,
    pub(crate) frozen_kept_elements: usize,
    pub(crate) frozen_candidate_position_elements: usize,
    pub(crate) frozen_candidate_identity_elements: usize,
    pub(crate) frozen_candidate_scale_elements: usize,
    pub(crate) frozen_reference_mean_correction_elements: usize,
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
    pub(crate) fe_accelerator_state_elements: usize,
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

/// Owned state restored by `StrictLinearCandidateTransaction::drop` unless a
/// caller explicitly commits the candidate.
struct StrictLinearRollbackState {
    basis: FrozenLinearBasis,
    basis_generation: u64,
    active_positions: Vec<usize>,
    within_certificate: LinearWithinDesignCertificate,
    fit: Option<CertifiedPreparedOlsFit>,
    frozen_step: Option<CertifiedFrozenLinearStep>,
    finite_predictor: Vec<f64>,
    absorbed_predictor: Vec<f64>,
    reconstruction: Option<LinearReconstructionCertificate>,
    reuse: Option<LinearSessionReuseReport>,
    projection_refresh_required: bool,
}

/// RAII authority boundary for a speculative strict basis. Every exit that
/// does not consume `commit` restores the prior state without allocation.
pub(crate) struct StrictLinearCandidateTransaction<
    'session,
    'prepared,
    P: PreparedOlsFitView + Sync + ?Sized,
> {
    session: &'session mut CertifiedLinearSession<'prepared, P>,
    rollback: Option<StrictLinearRollbackState>,
    basis_unchanged: bool,
    #[cfg(test)]
    fused_projection: bool,
}

impl<P: PreparedOlsFitView + Sync + ?Sized> StrictLinearCandidateTransaction<'_, '_, P> {
    pub(crate) fn candidate(&self) -> CertifiedLinearSessionOutput<'_> {
        self.session.validated_output()
    }

    pub(crate) const fn basis_unchanged(&self) -> bool {
        self.basis_unchanged
    }

    #[cfg(test)]
    pub(crate) const fn used_fused_projection_for_test(&self) -> bool {
        self.fused_projection
    }

    pub(crate) fn commit(mut self) {
        self.rollback = None;
    }

    pub(crate) fn rollback(self) {
        // Drop performs the infallible restoration.
    }

    #[cfg(test)]
    pub(crate) fn inject_same_rank_basis_pivot(&mut self) -> bool {
        self.session.inject_same_rank_strict_basis_pivot()
    }

    #[cfg(test)]
    pub(crate) const fn basis_generation_for_test(&self) -> u64 {
        self.session.basis_generation_for_test()
    }

    #[cfg(test)]
    pub(crate) fn workspace_capacities(&self) -> LinearSessionWorkspaceCapacities {
        self.session.workspace_capacities()
    }
}

impl<P: PreparedOlsFitView + Sync + ?Sized> Drop for StrictLinearCandidateTransaction<'_, '_, P> {
    fn drop(&mut self) {
        if let Some(rollback) = self.rollback.take() {
            self.session.restore_strict_rollback(rollback);
        }
    }
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
}

impl WithinGenerations {
    const fn from_session(generations: LinearSessionGenerations) -> Self {
        Self {
            sample: generations.sample,
            topology: generations.topology,
            weights: generations.weights,
        }
    }

    const fn same_design(self, generations: LinearSessionGenerations) -> bool {
        self.sample == generations.sample
            && self.topology == generations.topology
            && self.weights == generations.weights
    }

    const fn eligible_for_weight_refresh(self, generations: LinearSessionGenerations) -> bool {
        self.sample == generations.sample
            && self.topology == generations.topology
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
                // In-place cache compaction deliberately preserves the
                // source allocation so no fallible N-by-K reallocation can
                // occur after the canonical solve. Report retained capacity,
                // not logical length, to keep preflight and telemetry honest
                // for the N-by-(K+1) strided cache representation.
                resident_bytes: values.storage_capacity() as u128 * 8,
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
    /// Canonical generated-column identity of every physical stored column.
    stored_generated_columns: Vec<usize>,
    /// Active finite-basis positions into `stored_generated_columns`.
    active_positions: Vec<usize>,
    /// Current raw and within weighted norms aligned to stored columns.
    reference_norms: Vec<f64>,
    /// Current raw weighted means aligned to stored columns. PPML needs these
    /// scalars to report the normalized absorbed intercept without rebuilding
    /// an observation-scale finite predictor on every IRLS frame.
    reference_means: Vec<f64>,
    within_norms: Vec<f64>,
    generations: WithinGenerations,
    certificate: LinearWithinDesignCertificate,
}

impl RetainedWithinDesign {
    fn compact_for_inference(self) -> Result<Self> {
        let Self {
            storage,
            stored_generated_columns,
            active_positions,
            reference_norms,
            reference_means,
            within_norms,
            generations,
            certificate,
        } = self;
        let stored_columns = storage.columns();
        if stored_generated_columns.len() != stored_columns
            || reference_norms.len() != stored_columns
            || reference_means.len() != stored_columns
            || within_norms.len() != stored_columns
            || active_positions.windows(2).any(|pair| pair[0] >= pair[1])
            || active_positions
                .last()
                .is_some_and(|position| *position >= stored_columns)
        {
            return Err(invalid_argument(
                "weighted-linear final retained selection is inconsistent",
            ));
        }
        let selected_columns = active_positions.len();
        let mut selected_generated_columns = fallible_empty(
            selected_columns,
            "unable to reserve weighted-linear final retained identities",
        )?;
        let mut selected_reference_norms = fallible_empty(
            selected_columns,
            "unable to reserve weighted-linear final reference norms",
        )?;
        let mut selected_within_norms = fallible_empty(
            selected_columns,
            "unable to reserve weighted-linear final within norms",
        )?;
        let mut selected_reference_means = fallible_empty(
            selected_columns,
            "unable to reserve weighted-linear final reference means",
        )?;
        for position in active_positions.iter().copied() {
            selected_generated_columns.push(stored_generated_columns[position]);
            selected_reference_norms.push(reference_norms[position]);
            selected_reference_means.push(reference_means[position]);
            selected_within_norms.push(within_norms[position]);
        }
        let rows = storage.rows();
        let storage = match storage {
            RetainedWithinStorage::Dense(values) => {
                let mut row_major = values.into_row_major();
                for row in 0..rows {
                    let source_start = row.checked_mul(stored_columns).ok_or_else(|| {
                        allocation("weighted-linear final source offset overflows")
                    })?;
                    let target_start = row.checked_mul(selected_columns).ok_or_else(|| {
                        allocation("weighted-linear final target offset overflows")
                    })?;
                    for (target, source) in active_positions.iter().copied().enumerate() {
                        row_major[target_start + target] = row_major[source_start + source];
                    }
                }
                row_major.truncate(checked_elements(rows, selected_columns)?);
                RetainedWithinStorage::Dense(DenseMatrix::from_row_major(
                    rows,
                    selected_columns,
                    row_major,
                )?)
            }
            RetainedWithinStorage::Scratch(mut values) => {
                values.compact_row_major_columns_forward(&active_positions)?;
                RetainedWithinStorage::Scratch(values)
            }
        };
        let mut compact_positions = fallible_empty(
            selected_columns,
            "unable to reserve weighted-linear compact positions",
        )?;
        compact_positions.extend(0..selected_columns);
        Ok(Self {
            storage,
            stored_generated_columns: selected_generated_columns,
            active_positions: compact_positions,
            reference_norms: selected_reference_norms,
            reference_means: selected_reference_means,
            within_norms: selected_within_norms,
            generations,
            certificate,
        })
    }
}

/// Copyable, thread-safe borrowed access to the final compact within design.
/// Dense rows are borrowed directly; low-memory rows use positional scratch
/// reads and therefore do not share or lock a mutable file cursor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CertifiedLinearWithinDesign<'a> {
    storage: &'a RetainedWithinStorage,
    active_positions: &'a [usize],
}

impl SandwichDesign for CertifiedLinearWithinDesign<'_> {
    fn nrows(&self) -> usize {
        self.storage.rows()
    }

    fn dimension(&self) -> usize {
        self.active_positions.len()
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        if output.len() != self.active_positions.len() {
            return Err(invalid_argument(
                "weighted-linear selected within row dimensions are inconsistent",
            ));
        }
        match self.storage {
            RetainedWithinStorage::Dense(values) => {
                if row >= values.nrows() {
                    return Err(invalid_argument(
                        "weighted-linear selected within row is out of range",
                    ));
                }
                let source = &values.as_slice()[row * values.ncols()..(row + 1) * values.ncols()];
                for (target, position) in output.iter_mut().zip(self.active_positions) {
                    *target = source[*position];
                }
                Ok(())
            }
            RetainedWithinStorage::Scratch(values) => {
                if self
                    .active_positions
                    .iter()
                    .copied()
                    .enumerate()
                    .any(|(expected, position)| expected != position)
                    || values.columns() != output.len()
                {
                    return Err(invalid_argument(
                        "weighted-linear scratch inference design was not compacted",
                    ));
                }
                self.storage.fill_row(row, output)
            }
        }
    }
}

/// Borrowed certified products exposed to an iterative estimator and its
/// final inference stage.  Nothing here is exported from `fereg-core`.
pub(crate) struct CertifiedLinearSessionOutput<'a> {
    fit: &'a CertifiedPreparedOlsFit,
    response: &'a [f64],
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

    pub(crate) const fn response(&self) -> &'a [f64] {
        self.response
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
#[cfg(test)]
pub(crate) struct CertifiedFrozenLinearOutput<'a> {
    step: &'a CertifiedFrozenLinearStep,
    basis: &'a FrozenLinearBasis,
    within: &'a RetainedWithinDesign,
    reconstruction: LinearReconstructionCertificate,
    reuse: LinearSessionReuseReport,
}

#[cfg(test)]
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

    pub(crate) const fn basis(&self) -> &'a FrozenLinearBasis {
        self.basis
    }

    #[cfg(test)]
    pub(crate) fn finite_within_rows(&self) -> usize {
        self.within.storage.rows()
    }

    #[cfg(test)]
    pub(crate) fn finite_within_columns(&self) -> usize {
        self.within.active_positions.len()
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

/// PPML-only frozen-basis output. The working response decomposes as
/// `combined_predictor + within_residual`; exposing those two owned slices
/// lets IRLS construct its candidate predictor in O(N) without materializing
/// the separate finite and absorbed predictor vectors required by OLS.
///
/// This is deliberately a distinct type from `CertifiedFrozenLinearOutput`:
/// callers that need the OLS decomposition and its certificate must continue
/// to use `solve_frozen`.
pub(crate) struct CertifiedFrozenPpmlLinearOutput<'a> {
    step: &'a CertifiedFrozenLinearStep,
    basis: &'a FrozenLinearBasis,
    response: &'a [f64],
    within: &'a RetainedWithinDesign,
    reconstruction: LinearReconstructionCertificate,
    reuse: LinearSessionReuseReport,
}

impl<'a> CertifiedFrozenPpmlLinearOutput<'a> {
    pub(crate) fn coefficients(&self) -> &'a [f64] {
        &self.step.coefficients
    }

    pub(crate) fn reported_coefficients(&self) -> &'a [Option<f64>] {
        &self.step.reported_coefficients
    }

    pub(crate) fn within_residuals(&self) -> &'a [f64] {
        &self.step.within_residuals
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
    inverse_lower: Vec<f64>,
    rhs: Vec<f64>,
    moment_partials: Vec<f64>,
    row: Vec<f64>,
    solve: Vec<f64>,
    permutation: Vec<usize>,
    scores: Vec<f64>,
    coefficients: Vec<f64>,
    bread: Vec<f64>,
    residuals: Vec<f64>,
    reported_coefficients: Vec<Option<f64>>,
    kept: Vec<usize>,
    candidate_positions: Vec<usize>,
    candidate_ids: Vec<usize>,
    candidate_scales: Vec<f64>,
    reference_mean_corrections: Vec<f64>,
    basis_slots: Vec<FrozenLinearBasisSlot>,
}

impl FrozenLinearWorkspace {
    fn can_reuse(
        &self,
        rows: usize,
        rank: usize,
        stored_columns: usize,
        slots: usize,
        moment_partial_elements: usize,
    ) -> bool {
        let square = rank.saturating_mul(rank);
        self.response.capacity() >= rows
            && self.gram.capacity() >= square
            && self.factor.capacity() >= square
            && self.inverse_lower.capacity() >= square
            && self.rhs.capacity() >= rank
            && self.moment_partials.capacity() >= moment_partial_elements
            && self.row.capacity() >= stored_columns
            && self.solve.capacity() >= rank
            && self.permutation.capacity() >= rank
            && self.scores.capacity() >= rank
            && self.coefficients.capacity() >= rank
            && self.bread.capacity() >= square
            && self.residuals.capacity() >= rows
            && self.reported_coefficients.capacity() >= slots
            && self.kept.capacity() >= rank
            && self.candidate_positions.capacity() >= rank
            && self.candidate_ids.capacity() >= rank
            && self.candidate_scales.capacity() >= rank
            && self.reference_mean_corrections.capacity() >= stored_columns
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

    pub(crate) fn finite_within_design(&self) -> CertifiedLinearWithinDesign<'_> {
        CertifiedLinearWithinDesign {
            storage: &self.within.storage,
            active_positions: &self.within.active_positions,
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
    fe_design: Option<FeDesign>,
    fe_projector: Option<ReusableLinearMapProjector<'a>>,
    reconstruction: Option<LinearReconstructionCertificate>,
    reuse: Option<LinearSessionReuseReport>,
    pending_response_buffer_reused: bool,
    pending_weight_buffer_reused: bool,
    projection_refresh_required: bool,
    phase_allocator_probe: Option<LinearSessionPhaseAllocatorProbe>,
    #[cfg(test)]
    fresh_rank_failure_injection: u8,
}

/// The generic variant is the unchanged reusable path used by existing
/// callers. Only PPML's dedicated constructor may select the borrowed
/// categorical variant; it retains the owned design solely for PPML score and
/// final-state certificates that deliberately remain independent of the fast
/// projection kernel.
enum ReusableLinearMapProjector<'a> {
    Generic(Box<ReusableMapAbsorber>),
    PpmlCategorical {
        projector: Box<ReusableCategoricalMapAbsorber<'a>>,
        score_design: FeDesign,
    },
}

impl ReusableLinearMapProjector<'_> {
    const fn design(&self) -> &FeDesign {
        match self {
            Self::Generic(projector) => projector.design(),
            Self::PpmlCategorical { score_design, .. } => score_design,
        }
    }

    const fn max_rhs(&self) -> usize {
        match self {
            Self::Generic(projector) => projector.max_rhs(),
            Self::PpmlCategorical { projector, .. } => projector.max_rhs(),
        }
    }

    fn capacities(&self) -> ReusableMapCapacities {
        match self {
            Self::Generic(projector) => projector.capacities(),
            Self::PpmlCategorical { projector, .. } => projector.capacities(),
        }
    }

    const fn borrows_raw_weights(&self) -> bool {
        matches!(self, Self::PpmlCategorical { .. })
    }

    const fn is_ppml_categorical(&self) -> bool {
        matches!(self, Self::PpmlCategorical { .. })
    }

    fn enable_checkpoint_lsmr(&mut self) {
        if let Self::PpmlCategorical { projector, .. } = self {
            projector.enable_checkpoint_lsmr();
        }
    }

    #[cfg(test)]
    fn inject_checkpoint_lsmr_certificate_failure(&mut self) {
        if let Self::PpmlCategorical { projector, .. } = self {
            projector.inject_checkpoint_lsmr_certificate_failure();
        } else {
            panic!("checkpoint LSMR injection requires the PPML categorical projector");
        }
    }

    #[cfg(test)]
    fn inject_checkpoint_lsmr_fatal_failure(&mut self, code: ErrorCode) {
        if let Self::PpmlCategorical { projector, .. } = self {
            projector.inject_checkpoint_lsmr_fatal_failure(code);
        } else {
            panic!("checkpoint LSMR injection requires the PPML categorical projector");
        }
    }

    fn absorb(
        &mut self,
        raw_weights: BorrowedMapWeights<'_>,
        normalized_weights: &[f64],
        values: DenseMatrix,
        config: crate::fe::MapConfig,
        structural_constant_if_empty: bool,
    ) -> Result<CertifiedBorrowedMap> {
        match self {
            Self::Generic(projector) => projector.absorb(
                normalized_weights,
                values,
                config,
                structural_constant_if_empty,
            ),
            Self::PpmlCategorical { projector, .. } => {
                projector.absorb(raw_weights, values, config)
            }
        }
    }
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
static FROZEN_QR_FALLBACK_OBSERVED: AtomicBool = AtomicBool::new(false);

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

#[cfg(test)]
pub(crate) fn begin_frozen_qr_fallback_observation() {
    FROZEN_QR_FALLBACK_OBSERVED.store(false, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn finish_frozen_qr_fallback_observation() -> bool {
    FROZEN_QR_FALLBACK_OBSERVED.load(Ordering::SeqCst)
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
        || message.starts_with("frozen weighted-linear fused MAP projection failed;")
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
    #[cfg(test)]
    pub(crate) fn new_with_fe_design(
        prepared: &'a P,
        frame: LinearSolveFrame<'_>,
        config: PreparedOlsFitConfig,
        generations: LinearSessionGenerations,
        fe_design: FeDesign,
    ) -> Result<Self> {
        Self::new_with_fe_design_mode(prepared, frame, config, generations, fe_design, false)
    }

    /// PPML-only constructor which may select the reusable categorical MAP
    /// fast lane. Normal OLS and every existing generic session constructor
    /// continue to select the prior owned-design projector.
    pub(crate) fn new_with_ppml_fe_design(
        prepared: &'a P,
        frame: LinearSolveFrame<'_>,
        config: PreparedOlsFitConfig,
        generations: LinearSessionGenerations,
        fe_design: FeDesign,
    ) -> Result<Self> {
        Self::new_with_fe_design_mode(prepared, frame, config, generations, fe_design, true)
    }

    fn new_with_fe_design_mode(
        prepared: &'a P,
        frame: LinearSolveFrame<'_>,
        config: PreparedOlsFitConfig,
        generations: LinearSessionGenerations,
        fe_design: FeDesign,
        admit_ppml_categorical_map: bool,
    ) -> Result<Self> {
        frame.validate_for(prepared)?;
        let response = fallible_copy(
            frame.response(),
            "unable to reserve weighted-linear session response",
        )?;
        let weights = OwnedLinearWeights::from_borrowed(frame.raw_weights())?;
        let maximum_fe_rhs = match config.execution.mode {
            PreparedOlsExecutionMode::Fast => prepared
                .fit_model()
                .columns
                .len()
                .checked_add(1)
                .ok_or_else(|| allocation("weighted-linear projection width overflows"))?,
            PreparedOlsExecutionMode::Low => config.execution.poolsize.max(1),
        };
        if fe_design.nobs() != response.len() {
            return Err(invalid_argument(
                "weighted-linear owned FE design is not aligned on the prepared sample",
            ));
        }
        let (fe_design, fe_projector, map_weights) = if config.absorb.solver == FeSolverKind::Map {
            let categorical = if admit_ppml_categorical_map
                && config.execution.mode == PreparedOlsExecutionMode::Fast
                && !config.absorb.retain_coefficients
            {
                prepared.fit_borrowed_intercept_map_design()?
            } else {
                None
            };
            if let Some(categorical) = categorical.filter(|design| {
                matches!(design.effects().len(), 2 | 3) && design.nobs() == response.len()
            }) {
                (
                    None,
                    Some(ReusableLinearMapProjector::PpmlCategorical {
                        projector: Box::new(ReusableCategoricalMapAbsorber::new(
                            categorical,
                            maximum_fe_rhs,
                            config.execution.threads.resolved(),
                        )?),
                        score_design: fe_design,
                    }),
                    fallible_copy(
                        &response,
                        "unable to reserve weighted-linear prior response",
                    )?,
                )
            } else {
                (
                    None,
                    Some(ReusableLinearMapProjector::Generic(Box::new(
                        ReusableMapAbsorber::new(fe_design, maximum_fe_rhs)?,
                    ))),
                    fallible_filled(
                        response.len(),
                        0.0,
                        "unable to reserve weighted-linear reusable MAP weights",
                    )?,
                )
            }
        } else {
            (Some(fe_design), None, Vec::new())
        };
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
            fe_design,
            fe_projector,
            reconstruction: None,
            reuse: None,
            pending_response_buffer_reused: false,
            pending_weight_buffer_reused: false,
            projection_refresh_required: false,
            phase_allocator_probe: None,
            #[cfg(test)]
            fresh_rank_failure_injection: 0,
        })
    }

    pub(crate) const fn fe_design(&self) -> &FeDesign {
        match (&self.fe_projector, &self.fe_design) {
            (Some(projector), None) => projector.design(),
            (None, Some(design)) => design,
            _ => panic!("weighted-linear FE owner invariant failed"),
        }
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
        if (config.absorb.solver == FeSolverKind::Map) != self.fe_projector.is_some() {
            return Err(invalid_argument(
                "weighted-linear session cannot change its FE solver ownership route",
            ));
        }
        self.refresh_config(config);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn requires_rebind(&self) -> bool {
        !self.prepared_alignment_valid
    }

    #[cfg(test)]
    pub(crate) fn uses_ppml_categorical_map_for_test(&self) -> bool {
        self.fe_projector
            .as_ref()
            .is_some_and(ReusableLinearMapProjector::is_ppml_categorical)
    }

    pub(crate) fn retained_within_memory(&self) -> Option<LinearWithinMemoryReport> {
        self.within
            .as_ref()
            .map(|within| within.storage.memory_report())
    }

    #[cfg(test)]
    pub(super) fn compact_fast_cache_memory_for_test(
        &self,
        cache: PreparedOlsFastScoreCache,
        kept: &[usize],
    ) -> Result<(LinearWithinMemoryReport, bool, Vec<f64>)> {
        let source_pointer = match &cache {
            PreparedOlsFastScoreCache::Packed { within, .. }
            | PreparedOlsFastScoreCache::Strided { within, .. } => within.as_slice().as_ptr(),
        };
        let storage = self.dense_from_fast_cache(cache, kept)?;
        let pointer_preserved = match &storage {
            RetainedWithinStorage::Dense(within) => within.as_slice().as_ptr() == source_pointer,
            RetainedWithinStorage::Scratch(_) => false,
        };
        let retained_values = match &storage {
            RetainedWithinStorage::Dense(within) => within.as_slice().to_vec(),
            RetainedWithinStorage::Scratch(_) => Vec::new(),
        };
        Ok((storage.memory_report(), pointer_preserved, retained_values))
    }

    #[cfg(test)]
    pub(crate) fn retained_scratch_identity(&self) -> Option<&std::path::Path> {
        match &self.within.as_ref()?.storage {
            RetainedWithinStorage::Dense(_) => None,
            RetainedWithinStorage::Scratch(scratch) => Some(scratch.identity_path()),
        }
    }

    #[cfg(test)]
    pub(crate) fn retained_dense_identity(&self) -> Option<*const f64> {
        match &self.within.as_ref()?.storage {
            RetainedWithinStorage::Dense(values) => Some(values.as_slice().as_ptr()),
            RetainedWithinStorage::Scratch(_) => None,
        }
    }

    pub(crate) fn workspace_capacities(&self) -> LinearSessionWorkspaceCapacities {
        let current_step = self.frozen_step.as_ref();
        let fe = self
            .fe_projector
            .as_ref()
            .map(ReusableLinearMapProjector::capacities)
            .unwrap_or_default();
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
            frozen_inverse_lower_elements: self.frozen_workspace.inverse_lower.capacity(),
            frozen_rhs_elements: self.frozen_workspace.rhs.capacity(),
            frozen_moment_partial_elements: self.frozen_workspace.moment_partials.capacity(),
            frozen_row_elements: self.frozen_workspace.row.capacity(),
            frozen_solve_elements: self.frozen_workspace.solve.capacity(),
            frozen_permutation_elements: self.frozen_workspace.permutation.capacity(),
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
            frozen_candidate_position_elements: self
                .frozen_workspace
                .candidate_positions
                .capacity(),
            frozen_candidate_identity_elements: self.frozen_workspace.candidate_ids.capacity(),
            frozen_candidate_scale_elements: self.frozen_workspace.candidate_scales.capacity(),
            frozen_reference_mean_correction_elements: self
                .frozen_workspace
                .reference_mean_corrections
                .capacity(),
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
            fe_accelerator_state_elements: fe.accelerator_state,
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

    pub(crate) fn enable_checkpoint_lsmr(&mut self) {
        if let Some(projector) = self.fe_projector.as_mut() {
            projector.enable_checkpoint_lsmr();
        }
    }

    #[cfg(test)]
    pub(crate) fn inject_checkpoint_lsmr_certificate_failure(&mut self) {
        self.fe_projector
            .as_mut()
            .expect("checkpoint LSMR injection requires a projector")
            .inject_checkpoint_lsmr_certificate_failure();
    }

    #[cfg(test)]
    pub(crate) fn inject_checkpoint_lsmr_fatal_failure(&mut self, code: ErrorCode) {
        self.fe_projector
            .as_mut()
            .expect("checkpoint LSMR injection requires a projector")
            .inject_checkpoint_lsmr_fatal_failure(code);
    }

    #[cfg(test)]
    pub(crate) fn inject_fresh_rank_failure(&mut self, code: ErrorCode) {
        self.fresh_rank_failure_injection = match code {
            ErrorCode::Allocation => 1,
            ErrorCode::Interrupted => 2,
            _ => panic!("fresh-rank test injection supports allocation or interruption"),
        };
    }

    #[cfg(test)]
    fn take_fresh_rank_failure(&mut self) -> Option<FeregError> {
        match std::mem::take(&mut self.fresh_rank_failure_injection) {
            0 => None,
            1 => Some(allocation(
                "injected fresh canonical-rank allocation failure",
            )),
            2 => Some(FeregError::new(
                ErrorCode::Interrupted,
                "injected fresh canonical-rank interruption",
            )),
            _ => unreachable!("fresh-rank failure injection is validated"),
        }
    }

    /// Solve after a response or weight refresh, reusing or re-projecting the
    /// retained within design when its generation tags permit it.
    #[cfg(test)]
    pub(crate) fn solve_warm(&mut self) -> Result<CertifiedLinearSessionOutput<'_>> {
        self.solve(LinearSessionSolveKind::Warm)
    }

    /// Absorb only the changing response and stream the retained finite within
    /// design into a frozen-basis solve. This is the PPML iteration path.
    #[cfg(test)]
    pub(crate) fn solve_frozen(&mut self) -> Result<CertifiedFrozenLinearOutput<'_>> {
        self.check_frozen_failure_injection()?;
        let team = ThreadTeam::new(self.config.execution.threads);
        team.install(|| self.solve_frozen_internal(FrozenLinearOutputKind::FullOlsDecomposition))??;
        self.frozen_output()
    }

    /// PPML frozen-basis solve that certifies the exact combined predictor
    /// identity without rebuilding separate finite and absorbed predictors.
    pub(crate) fn solve_frozen_ppml(&mut self) -> Result<CertifiedFrozenPpmlLinearOutput<'_>> {
        self.check_frozen_failure_injection()?;
        let team = ThreadTeam::new(self.config.execution.threads);
        team.install(|| {
            self.solve_frozen_internal(FrozenLinearOutputKind::PpmlCombinedPredictor)
        })??;
        self.frozen_ppml_output()
    }

    fn check_frozen_failure_injection(&self) -> Result<()> {
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
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn strictify(&mut self) -> Result<CertifiedLinearSessionOutput<'_>> {
        self.strict_candidate()?.commit();
        self.output()
    }

    /// Discard every product derived from a failed frozen attempt while
    /// retaining the canonical prepared inputs, current response/weights, and
    /// frozen basis.  A subsequent `solve_fresh` must independently rebuild
    /// and certify the current frame.
    pub(crate) fn discard_invalid_frozen_state(&mut self) {
        self.fit = None;
        self.recycle_frozen_step();
        self.reconstruction = None;
        self.reuse = None;
        if let Some(within) = self.within.as_mut() {
            within.certificate = LinearWithinDesignCertificate {
                left_inverse_relative_infinity_norm: f64::INFINITY,
                right_inverse_relative_infinity_norm: f64::INFINITY,
                passed: false,
            };
        }
    }

    /// Build a canonical strict candidate while explicitly authorizing a new
    /// rank decision. The returned flag compares the complete canonical basis
    /// identity (kept/dropped coordinates and every coefficient-slot status)
    /// with the basis that was authoritative before the refresh. PPML uses
    /// this at convergence checkpoints so a same-rank pivot change cannot be
    /// mistaken for an unchanged publication basis.
    pub(crate) fn strict_candidate(
        &mut self,
    ) -> Result<StrictLinearCandidateTransaction<'_, 'a, P>> {
        if self.within.as_ref().is_none_or(|within| {
            within.generations.sample != self.generations.sample
                || within.generations.topology != self.generations.topology
        }) {
            return Err(invalid_argument(
                "strict weighted-linear candidate requires an aligned retained design",
            ));
        }
        let basis_generation = self.generations.basis;
        self.frozen_basis.as_ref().ok_or_else(|| {
            invalid_argument("strict weighted-linear candidate requires a frozen canonical basis")
        })?;
        let candidate_basis_generation = basis_generation.checked_add(1).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "weighted-linear basis generation overflowed",
            )
        })?;
        require_newer(self.generations.basis, candidate_basis_generation, "basis")?;
        // Pool acquisition is the last fallible external-resource action
        // before accepted state moves into the transaction.
        let installed_team = ThreadTeam::new(self.config.execution.threads).acquire()?;
        let previous_within = self.within.as_ref().ok_or_else(|| {
            invalid_argument("strict weighted-linear candidate requires a retained within design")
        })?;
        let active_positions = fallible_copy(
            &previous_within.active_positions,
            "unable to reserve strict weighted-linear rollback positions",
        )?;
        // Re-project and completely solve the strict candidate while the
        // accepted fit, basis, predictors, and reconstruction remain
        // authoritative.  In particular, the rank solve independently
        // certifies the accepted active positions under the *current*
        // projection before any of that authority moves into the RAII guard.
        let stage = installed_team.install(|| {
            self.prepare_retained_solve(LinearSessionSolveKind::Strict, Some(&active_positions))
        })?;
        #[cfg(test)]
        let fused_projection = stage.response_fused_with_design;
        let prior_certificate = stage.candidate.prior_basis_certificate.ok_or_else(|| {
            invalid_data("strict weighted-linear prior basis could not be recertified")
        })?;
        if !prior_certificate.passed {
            return Err(invalid_data(
                "strict weighted-linear prior basis certificate did not pass",
            ));
        }
        let basis_unchanged = self
            .frozen_basis
            .as_ref()
            .expect("frozen basis was validated before strict staging")
            .matches(&stage.candidate.fit);

        // The retained physical design was refreshed in place during
        // staging.  Make the still-authoritative active view publishable under
        // those current weights before constructing the rollback state.
        let within = self.within.as_mut().ok_or_else(|| {
            invalid_argument("strict weighted-linear retained design disappeared")
        })?;
        within.certificate = prior_certificate;
        within.generations = WithinGenerations::from_session(self.generations);
        let previous_basis = self
            .frozen_basis
            .take()
            .expect("frozen basis was validated before strict transaction");
        let rollback = StrictLinearRollbackState {
            basis: previous_basis,
            basis_generation,
            active_positions,
            within_certificate: prior_certificate,
            fit: self.fit.take(),
            frozen_step: self.frozen_step.take(),
            finite_predictor: std::mem::take(&mut self.finite_predictor),
            absorbed_predictor: std::mem::take(&mut self.absorbed_predictor),
            reconstruction: self.reconstruction.take(),
            reuse: self.reuse.take(),
            projection_refresh_required: self.projection_refresh_required,
        };
        self.frozen_basis = None;
        let transaction = StrictLinearCandidateTransaction {
            session: self,
            rollback: Some(rollback),
            basis_unchanged,
            #[cfg(test)]
            fused_projection,
        };
        // Every fallible operation from this point is protected by the guard:
        // dropping `transaction` restores the concrete current-weight prior
        // certificate and all accepted authority without allocating.
        transaction.session.install_retained_stage(stage)?;
        transaction.session.output()?;
        transaction.session.generations.basis = candidate_basis_generation;
        Ok(transaction)
    }

    fn restore_strict_rollback(&mut self, rollback: StrictLinearRollbackState) {
        self.recycle_frozen_step();
        self.generations.basis = rollback.basis_generation;
        self.frozen_basis = Some(rollback.basis);
        if let Some(within) = self.within.as_mut() {
            within.active_positions = rollback.active_positions;
            within.certificate = rollback.within_certificate;
            within.generations = WithinGenerations::from_session(self.generations);
        }
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
            response: &self.response,
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

    fn validated_output(&self) -> CertifiedLinearSessionOutput<'_> {
        CertifiedLinearSessionOutput {
            fit: self
                .fit
                .as_ref()
                .expect("strict candidate output was prevalidated"),
            response: &self.response,
            finite_predictor: &self.finite_predictor,
            absorbed_predictor: &self.absorbed_predictor,
            within: self
                .within
                .as_ref()
                .expect("strict candidate within design was prevalidated"),
            reconstruction: self
                .reconstruction
                .expect("strict candidate reconstruction was prevalidated"),
            reuse: self
                .reuse
                .expect("strict candidate reuse report was prevalidated"),
        }
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
        let within = self
            .within
            .take()
            .ok_or_else(|| {
                invalid_argument("weighted-linear session has no final within design to hand off")
            })?
            .compact_for_inference()?;
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
            within,
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
        let mut reused = self.response.capacity() >= response.len();
        if self
            .fe_projector
            .as_ref()
            .is_some_and(ReusableLinearMapProjector::is_ppml_categorical)
        {
            reused &= self.map_weights.capacity() >= self.response.len();
            self.map_weights.clear();
            self.map_weights
                .try_reserve(self.response.len())
                .map_err(|_| allocation("unable to refresh weighted-linear prior response"))?;
            self.map_weights.extend_from_slice(&self.response);
        }
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

    /// Refresh one response/weight frame while advancing an already-aligned
    /// retained design to the new generation when every weight-scale field and
    /// raw weight bit is unchanged. This exact identity is the only condition
    /// under which a later strict solve may reuse weighted design products.
    pub(crate) fn refresh_frame(
        &mut self,
        response: &[f64],
        weights: BorrowedMapWeights<'_>,
        bread_weight_scale: super::PreparedOlsBreadWeightScale,
        generation: u64,
    ) -> Result<bool> {
        let identical_weights = self.weights_exactly_match(weights, bread_weight_scale);
        if identical_weights {
            require_newer(self.generations.weights, generation, "weight")?;
        }
        let prior_weight_generation = self.generations.weights;
        self.refresh_response(response, generation)?;
        if identical_weights {
            self.generations.weights = generation;
            if let Some(within) = self.within.as_mut() {
                if within.generations.sample == self.generations.sample
                    && within.generations.topology == self.generations.topology
                    && within.generations.weights == prior_weight_generation
                {
                    within.generations.weights = generation;
                }
            }
            self.pending_weight_buffer_reused = true;
        } else {
            self.refresh_weights(weights, bread_weight_scale, generation)?;
        }
        Ok(identical_weights)
    }

    fn weights_exactly_match(
        &self,
        weights: BorrowedMapWeights<'_>,
        bread_weight_scale: super::PreparedOlsBreadWeightScale,
    ) -> bool {
        let scale_matches = self.bread_weight_scale.weight_kind == bread_weight_scale.weight_kind
            && self.bread_weight_scale.solver_weight_divisor.to_bits()
                == bread_weight_scale.solver_weight_divisor.to_bits()
            && self
                .bread_weight_scale
                .reference_weight_multiplier
                .to_bits()
                == bread_weight_scale.reference_weight_multiplier.to_bits()
            && self.bread_weight_scale.reference_weight_sum.to_bits()
                == bread_weight_scale.reference_weight_sum.to_bits();
        scale_matches
            && match (&self.weights, weights) {
                (OwnedLinearWeights::Unit, BorrowedMapWeights::Unit) => true,
                (
                    OwnedLinearWeights::Provided(current),
                    BorrowedMapWeights::Provided(candidate),
                ) => {
                    current.len() == candidate.len()
                        && current
                            .iter()
                            .zip(candidate)
                            .all(|(left, right)| left.to_bits() == right.to_bits())
                }
                _ => false,
            }
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
        let team = ThreadTeam::new(self.config.execution.threads);
        team.install(|| self.solve_retained_internal(kind, None).map(|_| ()))??;
        self.output()
    }

    fn solve_retained_internal(
        &mut self,
        kind: LinearSessionSolveKind,
        prior_active_positions: Option<&[usize]>,
    ) -> Result<Option<LinearWithinDesignCertificate>> {
        self.recycle_frozen_step();
        let stage = self.prepare_retained_solve(kind, prior_active_positions)?;
        self.install_retained_stage(stage)
    }

    fn prepare_retained_solve(
        &mut self,
        kind: LinearSessionSolveKind,
        prior_active_positions: Option<&[usize]>,
    ) -> Result<RetainedSolveStage> {
        self.ensure_bound()?;
        let mut report = LinearSessionReuseReport::new(kind);
        report.response_buffer_reused = self.pending_response_buffer_reused;
        report.weight_buffer_reused = self.pending_weight_buffer_reused;
        let phase_probe = self.phase_allocator_probe;
        let mut phase_allocation = LinearSessionPhaseAllocationObservation::default();
        let projection_started = Instant::now();
        let projection_phase = LinearSessionPhaseAllocationGuard::new(
            phase_probe,
            phase_probe.map(|_| &mut phase_allocation),
            LinearSessionAllocationPhase::Projection,
        );
        #[cfg(test)]
        let mut response_fused_with_design = false;
        let (within_response, absorption_report) = if self.within.is_none() {
            let (within, response, absorption_report) = self.build_full_retained_design()?;
            self.within = Some(within);
            (response, absorption_report)
        } else {
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
                });
                if !eligible {
                    return Err(invalid_argument(
                        "weighted-linear retained solve has no reusable design generation",
                    ));
                }
                report.warm_projection_attempted = true;
                let mut fused_response_report = None;
                if self.config.absorb.solver == FeSolverKind::Map
                    && self
                        .fe_projector
                        .as_ref()
                        .is_some_and(ReusableLinearMapProjector::is_ppml_categorical)
                {
                    report.fe_projection_workspace_reused = self.fill_reusable_map_weights()?;
                    let (reused, fused_report) =
                        self.reproject_retained_with_response_in_place()?;
                    report.fe_projection_workspace_reused &= reused;
                    fused_response_report = Some(fused_report);
                    #[cfg(test)]
                    {
                        response_fused_with_design = true;
                    }
                } else if self.config.absorb.solver == FeSolverKind::Map {
                    report.fe_projection_workspace_reused = self.fill_reusable_map_weights()?;
                    report.fe_projection_workspace_reused &=
                        self.reproject_retained_within_in_place()?;
                } else {
                    self.reproject_retained_within_in_place()?;
                }
                self.refresh_retained_norms()?;
                report.warm_projection_certified = true;
                report.within_design_reused = true;
                if let Some(fused_report) = fused_response_report {
                    (
                        std::mem::take(&mut self.frozen_workspace.response),
                        fused_report,
                    )
                } else {
                    self.absorb_current_response()?
                }
            } else {
                report.fe_projection_workspace_reused =
                    self.config.absorb.solver == FeSolverKind::Map;
                report.within_design_reused = true;
                self.absorb_current_response()?
            }
        };
        drop(projection_phase);
        let projection_seconds = projection_started.elapsed().as_secs_f64();

        let finite_started = Instant::now();
        let finite_phase = LinearSessionPhaseAllocationGuard::new(
            phase_probe,
            phase_probe.map(|_| &mut phase_allocation),
            LinearSessionAllocationPhase::FiniteSolve,
        );
        #[cfg(test)]
        let candidate = match self.take_fresh_rank_failure() {
            Some(error) => Err(error),
            None => self.solve_full_retained_rank(
                &within_response,
                absorption_report,
                prior_active_positions,
            ),
        };
        #[cfg(not(test))]
        let candidate = self.solve_full_retained_rank(
            &within_response,
            absorption_report,
            prior_active_positions,
        );
        // `absorb_current_response` transfers the session-owned response
        // allocation into `within_response`.  Return that exact allocation to
        // the reusable workspace on both success and every downstream rank /
        // certificate error before propagating the result.
        self.frozen_workspace.response = within_response;
        let candidate = candidate?;
        drop(finite_phase);
        let finite_solve_seconds = finite_started.elapsed().as_secs_f64();
        if kind != LinearSessionSolveKind::Strict {
            if let Some(basis) = &self.frozen_basis {
                if !basis.matches(&candidate.fit) {
                    return Err(invalid_data(
                        "weighted-linear solve changed the frozen canonical rank basis",
                    ));
                }
            }
        }
        let candidate_basis =
            if kind == LinearSessionSolveKind::Strict || self.frozen_basis.is_none() {
                Some(FrozenLinearBasis::from_fit(&candidate.fit)?)
            } else {
                None
            };
        report.phase_timing = LinearSessionPhaseTiming {
            projection_seconds: Some(projection_seconds),
            finite_solve_seconds: Some(finite_solve_seconds),
        };
        report.phase_allocation = phase_allocation;
        Ok(RetainedSolveStage {
            candidate,
            candidate_basis,
            report,
            #[cfg(test)]
            response_fused_with_design,
        })
    }

    fn install_retained_stage(
        &mut self,
        stage: RetainedSolveStage,
    ) -> Result<Option<LinearWithinDesignCertificate>> {
        let RetainedSolveStage {
            candidate,
            candidate_basis,
            mut report,
            ..
        } = stage;
        if let Some(basis) = &self.frozen_basis {
            if !basis.matches(&candidate.fit) {
                return Err(invalid_data(
                    "weighted-linear solve changed the frozen canonical rank basis",
                ));
            }
        }
        let predictor_workspace_reused = self.finite_predictor.capacity() >= self.response.len()
            && self.absorbed_predictor.capacity() >= self.response.len();
        let reconstruction = self.rebuild_predictors(&candidate.fit)?;
        let within = self
            .within
            .as_mut()
            .ok_or_else(|| invalid_argument("weighted-linear retained design disappeared"))?;
        within.active_positions = candidate.active_positions;
        within.certificate = candidate.within_certificate;
        report.predictor_workspace_reused = predictor_workspace_reused;
        report.certificate_workspace_reused = true;
        self.fit = Some(candidate.fit);
        if let Some(basis) = candidate_basis {
            self.frozen_basis = Some(basis);
        }
        self.reconstruction = Some(reconstruction);
        self.reuse = Some(report);
        self.pending_response_buffer_reused = false;
        self.pending_weight_buffer_reused = false;
        self.projection_refresh_required = false;
        Ok(candidate.prior_basis_certificate)
    }

    fn absorb_current_response(&mut self) -> Result<(Vec<f64>, SolveReport)> {
        resize_zeroed(
            &mut self.frozen_workspace.response,
            self.response.len(),
            "unable to reserve weighted-linear response absorption",
        )?;
        self.frozen_workspace
            .response
            .copy_from_slice(&self.response);
        let raw = DenseMatrix::from_row_major(
            self.response.len(),
            1,
            std::mem::take(&mut self.frozen_workspace.response),
        )?;
        let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
            self.fill_reusable_map_weights()?;
            self.absorb_reusable_map(raw)?
        } else {
            self.prepared
                .fit_absorb_matrix(self.weights.borrowed(), raw, self.config.absorb)?
        };
        if !absorbed.report().certified() {
            return Err(invalid_data(
                "weighted-linear response absorption did not certify",
            ));
        }
        let (response, report) = absorbed.into_parts();
        Ok((response.into_row_major(), report))
    }

    fn absorb_reusable_map(&mut self, values: DenseMatrix) -> Result<CertifiedBorrowedMap> {
        let raw_weights = self.weights.borrowed();
        let projector = self.fe_projector.as_mut().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "weighted-linear MAP route has no reusable projector",
            )
        })?;
        projector.absorb(
            raw_weights,
            &self.map_weights,
            values,
            self.config.absorb.map,
            self.prepared.fit_has_absorbed_intercept(),
        )
    }

    fn reusable_map_max_rhs(&self) -> Result<usize> {
        self.fe_projector
            .as_ref()
            .map(ReusableLinearMapProjector::max_rhs)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "weighted-linear MAP route has no reusable projector",
                )
            })
    }

    fn solve_frozen_internal(&mut self, output_kind: FrozenLinearOutputKind) -> Result<()> {
        self.ensure_bound()?;
        self.recycle_frozen_step();
        if let Some(fit) = self.fit.take() {
            let mut residual_storage = fit.into_session_residual_storage();
            residual_storage.clear();
            if residual_storage.capacity() >= self.frozen_workspace.residuals.capacity() {
                self.frozen_workspace.residuals = residual_storage;
            }
        }
        let basis = self.frozen_basis.as_ref().ok_or_else(|| {
            invalid_argument("frozen weighted-linear solve requires an initial fresh solve")
        })?;
        let rank = basis.kept_generated_columns().len();
        let slot_count = basis.slots().len();
        let stored_columns = self
            .within
            .as_ref()
            .map_or(rank, |within| within.storage.columns());
        let retained_moment_elements = self.within.as_ref().map_or(0, |within| {
            if matches!(within.storage, RetainedWithinStorage::Scratch(_)) {
                if rank == 0 {
                    return 0;
                }
                let requested = rayon::current_num_threads().min(rank).max(1);
                let columns_per_partition = rank.div_ceil(requested);
                let partitions = rank.div_ceil(columns_per_partition);
                stored_columns.saturating_mul(partitions)
            } else {
                0
            }
        });
        let kernel_workspace_reused = self.frozen_workspace.can_reuse(
            self.response.len(),
            rank,
            stored_columns,
            slot_count,
            retained_moment_elements,
        );
        self.frozen_workspace.kept.clear();
        self.frozen_workspace
            .kept
            .try_reserve(rank)
            .map_err(|_| allocation("unable to reserve frozen weighted-linear kept coordinates"))?;
        self.frozen_workspace
            .kept
            .extend_from_slice(basis.kept_generated_columns());
        let kept = std::mem::take(&mut self.frozen_workspace.kept);
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
        let mut fused_response_report = None;
        if !same_design {
            let eligible = self.within.as_ref().is_some_and(|within| {
                within
                    .generations
                    .eligible_for_weight_refresh(self.generations)
                    || self.projection_refresh_required
                        && within.generations.sample == self.generations.sample
                        && within.generations.topology == self.generations.topology
            });
            if !eligible {
                return Err(invalid_argument(
                    "frozen weighted-linear solve has no reusable design generation",
                ));
            }
            report.warm_projection_attempted = true;
            if reusable_map
                && self
                    .fe_projector
                    .as_ref()
                    .is_some_and(ReusableLinearMapProjector::is_ppml_categorical)
            {
                let (reused, fused_report) = self
                    .reproject_retained_with_response_in_place()
                    .map_err(|error| {
                        FeregError::new(
                            error.code,
                            format!(
                                "frozen weighted-linear fused MAP projection failed; strict canonical solve required: {}",
                                error.message
                            ),
                        )
                    })?;
                projection_workspace_reused &= reused;
                fused_response_report = Some(fused_report);
            } else if reusable_map {
                projection_workspace_reused &=
                    self.reproject_retained_within_in_place().map_err(|error| {
                        FeregError::new(
                            error.code,
                            format!(
                                "frozen weighted-linear reusable MAP projection failed; strict canonical solve required: {}",
                                error.message
                            ),
                        )
                    })?;
            } else {
                self.reproject_retained_within_in_place().map_err(|error| {
                    FeregError::new(
                        error.code,
                        format!(
                            "frozen weighted-linear warm projection failed; strict canonical solve required: {}",
                            error.message
                        ),
                    )
                })?;
                projection_workspace_reused = false;
            }
            self.refresh_retained_norms()?;
            report.warm_projection_certified = true;
        }
        report.within_design_reused = true;

        let (within_response_matrix, mut response_absorption_report) =
            if let Some(fused_report) = fused_response_report {
                (
                    DenseMatrix::from_row_major(
                        self.response.len(),
                        1,
                        std::mem::take(&mut self.frozen_workspace.response),
                    )?,
                    fused_report,
                )
            } else {
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
                    self.absorb_reusable_map(raw_response)?
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
                absorbed.into_parts()
            };
        // The frozen report is retained across IRLS iterations and may overlap
        // a speculative strict candidate. Diagnostic warning strings have no
        // finite memory bound and do not carry scientific authority; retained
        // sessions therefore persist an exactly empty warning bank. The
        // ordinary one-shot fereg report remains untouched.
        response_absorption_report.warnings = Vec::new();
        drop(projection_phase);
        let within_response = within_response_matrix.as_slice();
        let within = self.within.as_ref().ok_or_else(|| {
            invalid_argument("frozen weighted-linear retained design disappeared")
        })?;
        if within.storage.rows() != self.response.len()
            || within.storage.columns() != within.stored_generated_columns.len()
            || within.active_positions.len() != kept.len()
            || within
                .active_positions
                .iter()
                .copied()
                .zip(kept.iter().copied())
                .any(|(position, generated)| {
                    within.stored_generated_columns.get(position).copied() != Some(generated)
                })
        {
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
            &within.active_positions,
            &within.reference_norms,
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

        let (predictor_workspace_reused, reconstruction) = match output_kind {
            #[cfg(test)]
            FrozenLinearOutputKind::FullOlsDecomposition => (
                self.finite_predictor.capacity() >= self.response.len()
                    && self.absorbed_predictor.capacity() >= self.response.len(),
                self.rebuild_frozen_predictors(
                    &kept,
                    &frozen.coefficients,
                    &frozen.within_residuals,
                )?,
            ),
            FrozenLinearOutputKind::PpmlCombinedPredictor => (
                true,
                certify_frozen_combined_predictor(&self.response, &frozen.within_residuals)?,
            ),
        };
        let mut reported_coefficients =
            std::mem::take(&mut self.frozen_workspace.reported_coefficients);
        // Reborrow the authoritative immutable basis after predictor rebuild;
        // retaining a second S-sized slot copy in the warm workspace would
        // make strict candidate overlap both larger and harder to preflight.
        let basis_slots = self
            .frozen_basis
            .as_ref()
            .ok_or_else(|| invalid_argument("frozen weighted-linear basis disappeared"))?
            .slots();
        match output_kind {
            #[cfg(test)]
            FrozenLinearOutputKind::FullOlsDecomposition => {
                self.build_frozen_reported_coefficients(
                    basis_slots,
                    &kept,
                    &frozen.coefficients,
                    &mut reported_coefficients,
                )?;
            }
            FrozenLinearOutputKind::PpmlCombinedPredictor => {
                self.build_frozen_ppml_reported_coefficients(
                    basis_slots,
                    &kept,
                    &frozen.coefficients,
                    &mut reported_coefficients,
                )?;
            }
        }
        self.frozen_workspace.kept = kept;
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
        Ok(())
    }

    #[cfg(test)]
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

    fn frozen_ppml_output(&self) -> Result<CertifiedFrozenPpmlLinearOutput<'_>> {
        let step = self.frozen_step.as_ref().ok_or_else(|| {
            invalid_argument("weighted-linear session has no current PPML frozen solve")
        })?;
        if step.generations != self.generations {
            return Err(invalid_argument(
                "weighted-linear PPML frozen solve has stale generations",
            ));
        }
        Ok(CertifiedFrozenPpmlLinearOutput {
            step,
            basis: self.frozen_basis.as_ref().ok_or_else(|| {
                invalid_argument("weighted-linear session has no PPML frozen basis")
            })?,
            response: &self.response,
            within: self.within.as_ref().ok_or_else(|| {
                invalid_argument("weighted-linear session has no PPML retained within design")
            })?,
            reconstruction: self.reconstruction.ok_or_else(|| {
                invalid_argument("weighted-linear PPML reconstruction is not certified")
            })?,
            reuse: self.reuse.ok_or_else(|| {
                invalid_argument("weighted-linear PPML solve has no reuse report")
            })?,
        })
    }

    #[cfg(test)]
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
        let mut certificate = LinearReconstructionCertificate::new();
        for (row, within_residual) in within_residuals.iter().copied().enumerate() {
            self.absorbed_predictor[row] =
                (self.response[row] - within_residual) - self.finite_predictor[row];
            certificate.observe(
                self.response[row],
                self.finite_predictor[row],
                self.absorbed_predictor[row],
                within_residual,
            )?;
        }
        Ok(certificate)
    }

    #[cfg(test)]
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

    fn build_frozen_ppml_reported_coefficients(
        &self,
        slots: &[FrozenLinearBasisSlot],
        kept: &[usize],
        coefficients: &[f64],
        reported: &mut Vec<Option<f64>>,
    ) -> Result<()> {
        if kept.len() != coefficients.len() {
            return Err(invalid_argument(
                "frozen PPML reported coefficient dimensions are inconsistent",
            ));
        }
        let needs_normalized_intercept = slots
            .iter()
            .any(|slot| slot.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept);
        let normalized_intercept = if needs_normalized_intercept {
            let within = self
                .within
                .as_ref()
                .ok_or_else(|| invalid_argument("frozen PPML retained design is unavailable"))?;
            if within.active_positions.len() != kept.len()
                || within.reference_means.len() != within.storage.columns()
            {
                return Err(invalid_argument(
                    "frozen PPML retained means are inconsistent",
                ));
            }
            let outcome_mean = weighted_mean_slice(
                &self.response,
                self.weights.borrowed(),
                self.bread_weight_scale,
            )?;
            let mut finite_mean = 0.0_f64;
            let mut finite_mean_correction = 0.0_f64;
            for ((position, generated), coefficient) in within
                .active_positions
                .iter()
                .copied()
                .zip(kept.iter().copied())
                .zip(coefficients.iter().copied())
            {
                if within.stored_generated_columns.get(position).copied() != Some(generated) {
                    return Err(invalid_argument(
                        "frozen PPML retained mean identity is inconsistent",
                    ));
                }
                compensated_add(
                    coefficient * within.reference_means[position],
                    &mut finite_mean,
                    &mut finite_mean_correction,
                )?;
            }
            let value = outcome_mean - (finite_mean + finite_mean_correction);
            if !value.is_finite() {
                return Err(invalid_data(
                    "frozen PPML normalized intercept is not finite",
                ));
            }
            Some(value)
        } else {
            None
        };

        reported.clear();
        reported
            .try_reserve(slots.len())
            .map_err(|_| allocation("unable to reserve frozen PPML reported coefficients"))?;
        reported.resize(slots.len(), None);
        for (target, slot) in reported.iter_mut().zip(slots) {
            *target = match slot.status {
                PreparedOlsFitStatus::Kept => {
                    let generated = slot.generated_column_index.ok_or_else(|| {
                        invalid_argument("frozen PPML kept slot has no generated column")
                    })?;
                    let position = kept
                        .iter()
                        .position(|candidate| *candidate == generated)
                        .ok_or_else(|| {
                            invalid_argument("frozen PPML kept slot is outside the basis")
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

    fn solve_full_retained_rank(
        &mut self,
        within_response: &[f64],
        absorption_report: SolveReport,
        prior_active_positions: Option<&[usize]>,
    ) -> Result<RetainedRankSolveDraft> {
        let rows = self.response.len();
        let generated_count = self.prepared.fit_model().columns.len();
        let relative_threshold = self.config.rank_tolerance_multiplier
            * f64::EPSILON
            * rows.max(generated_count).max(1) as f64;
        let retained = self.within.as_mut().ok_or_else(|| {
            invalid_argument("retained weighted-linear rank solve has no full design")
        })?;
        if within_response.len() != rows
            || retained.storage.rows() != rows
            || retained.storage.columns() != retained.stored_generated_columns.len()
            || retained.reference_norms.len() != retained.storage.columns()
            || retained.reference_means.len() != retained.storage.columns()
            || retained.within_norms.len() != retained.storage.columns()
        {
            return Err(invalid_argument(
                "retained weighted-linear rank inputs are inconsistent",
            ));
        }

        let workspace = &mut self.frozen_workspace;
        workspace.candidate_positions.clear();
        workspace.candidate_ids.clear();
        workspace.candidate_scales.clear();
        let stored_columns = retained.storage.columns();
        for target in [&mut workspace.scores, &mut workspace.solve] {
            if target.capacity() < stored_columns {
                target
                    .try_reserve(stored_columns.saturating_sub(target.len()))
                    .map_err(|_| {
                        allocation("unable to reserve retained reusable norm workspace")
                    })?;
            }
        }
        for target in [
            &mut workspace.candidate_positions,
            &mut workspace.candidate_ids,
        ] {
            target
                .try_reserve(stored_columns)
                .map_err(|_| allocation("unable to reserve retained rank candidates"))?;
        }
        workspace
            .candidate_scales
            .try_reserve(stored_columns)
            .map_err(|_| allocation("unable to reserve retained rank scales"))?;
        let mut generated_statuses = fallible_filled(
            generated_count,
            PreparedOlsFitStatus::NumericallyRankDeficient,
            "unable to reserve retained generated statuses",
        )?;
        let has_absorbed_intercept = self.prepared.fit_has_absorbed_intercept();
        for (generated, column) in self.prepared.fit_model().columns.iter().enumerate() {
            if has_absorbed_intercept && matches!(column.kind, PreparedOlsColumnKind::Intercept) {
                generated_statuses[generated] = PreparedOlsFitStatus::AbsorbedInterceptDeferred;
            }
        }
        for (position, generated) in retained
            .stored_generated_columns
            .iter()
            .copied()
            .enumerate()
        {
            let reference = retained.reference_norms[position];
            let within = retained.within_norms[position];
            if !reference.is_finite() || reference < 0.0 || !within.is_finite() || within < 0.0 {
                return Err(invalid_data(
                    "retained weighted-linear column norm is invalid",
                ));
            }
            if reference == 0.0 {
                continue;
            }
            let ratio = within / reference;
            let squared_ratio = ratio * ratio;
            if !squared_ratio.is_finite() {
                return Err(invalid_data(
                    "retained weighted-linear FE-collinearity ratio is not finite",
                ));
            }
            if has_absorbed_intercept
                && squared_ratio <= self.config.fixed_effect_collinearity_tolerance
            {
                generated_statuses[generated] = PreparedOlsFitStatus::CollinearWithFixedEffects;
                continue;
            }
            workspace.candidate_positions.push(position);
            workspace.candidate_ids.push(generated);
            workspace.candidate_scales.push(reference);
        }

        let candidate_count = workspace.candidate_ids.len();
        let candidate_square = checked_elements(candidate_count, candidate_count)?;
        let (rank, maximum_dropped_relative_norm, condition_proxy) = if candidate_count == 0 {
            workspace.gram.clear();
            workspace.rhs.clear();
            workspace.factor.clear();
            workspace.inverse_lower.clear();
            workspace.solve.clear();
            workspace.permutation.clear();
            workspace.coefficients.clear();
            workspace.bread.clear();
            workspace.kept.clear();
            (0, 0.0, 1.0)
        } else {
            let source = match &retained.storage {
                RetainedWithinStorage::Dense(values) => FixedBlockMomentSource::RetainedDense {
                    values: values.as_slice(),
                    rows,
                    stored_columns,
                    selected_columns: Some(&workspace.candidate_positions),
                    response: within_response,
                    weights: self.weights.borrowed(),
                    solver_weight_divisor: self.bread_weight_scale.solver_weight_divisor,
                    column_scales: Some(&workspace.candidate_scales),
                },
                RetainedWithinStorage::Scratch(values) => FixedBlockMomentSource::RetainedScratch {
                    values,
                    selected_columns: Some(&workspace.candidate_positions),
                    response: within_response,
                    weights: self.weights.borrowed(),
                    solver_weight_divisor: self.bread_weight_scale.solver_weight_divisor,
                    column_scales: Some(&workspace.candidate_scales),
                },
            };
            let layout = finite_wls_moment_layout(source)?;
            resize_zeroed(
                &mut workspace.moment_partials,
                layout.partial_elements,
                "unable to reserve retained weighted-linear moment partials",
            )?;
            resize_zeroed(
                &mut workspace.gram,
                candidate_square,
                "unable to reserve retained weighted-linear Gram",
            )?;
            resize_zeroed(
                &mut workspace.rhs,
                candidate_count,
                "unable to reserve retained weighted-linear RHS",
            )?;
            fill_finite_wls_moments(
                source,
                layout,
                &mut workspace.moment_partials,
                &mut workspace.rhs,
                &mut workspace.gram,
            )?;
            resize_zeroed(
                &mut workspace.factor,
                candidate_square,
                "unable to reserve retained rank QR factor",
            )?;
            resize_zeroed(
                &mut workspace.solve,
                candidate_count,
                "unable to reserve retained transformed RHS",
            )?;
            resize_zeroed(
                &mut workspace.scores,
                candidate_count,
                "unable to reserve retained QR trailing norms",
            )?;
            resize_zeroed(
                &mut workspace.row,
                stored_columns,
                "unable to reserve retained QR row workspace",
            )?;
            workspace.permutation.clear();
            workspace
                .permutation
                .try_reserve(candidate_count)
                .map_err(|_| allocation("unable to reserve retained rank permutation"))?;
            workspace.permutation.resize(candidate_count, 0);
            resize_zeroed(
                &mut workspace.inverse_lower,
                candidate_square,
                "unable to reserve retained guarded-Cholesky Gram",
            )?;
            workspace.inverse_lower.copy_from_slice(&workspace.gram);
            workspace.solve.copy_from_slice(&workspace.rhs);
            let maximum_diagonal = (0..candidate_count)
                .map(|position| workspace.gram[position * candidate_count + position])
                .fold(0.0_f64, f64::max);
            let strong_pivot_guard = maximum_diagonal * relative_threshold.sqrt();
            let fast_factor = if maximum_diagonal.is_finite()
                && maximum_diagonal > 0.0
                && strong_pivot_guard.is_finite()
                && strong_pivot_guard > 0.0
            {
                factor_guarded_cholesky_in_place(
                    &mut workspace.inverse_lower,
                    &mut workspace.solve,
                    &mut workspace.factor,
                    &mut workspace.permutation,
                    CholeskyPivotPolicy::MaximumDiagonal,
                    strong_pivot_guard,
                )?
            } else {
                None
            };
            if let Some(factor_report) = fast_factor {
                resize_zeroed(
                    &mut workspace.scores,
                    candidate_count,
                    "unable to reserve retained Cholesky forward solve",
                )?;
                resize_zeroed(
                    &mut workspace.coefficients,
                    candidate_count,
                    "unable to reserve retained Cholesky coefficients",
                )?;
                if !solve_guarded_cholesky(
                    &workspace.factor,
                    &workspace.solve,
                    &mut workspace.scores,
                    &mut workspace.coefficients,
                )? {
                    return Err(invalid_data(
                        "retained weighted-linear Cholesky solution is not finite",
                    ));
                }
                workspace.row[..candidate_count].fill(0.0);
                for position in 0..candidate_count {
                    workspace.row[workspace.permutation[position]] =
                        workspace.coefficients[position];
                }
                workspace
                    .coefficients
                    .copy_from_slice(&workspace.row[..candidate_count]);
                workspace.kept.clear();
                workspace.kept.extend_from_slice(&workspace.candidate_ids);
                resize_zeroed(
                    &mut workspace.bread,
                    candidate_square,
                    "unable to reserve retained Cholesky inverse",
                )?;
                if !invert_guarded_cholesky(
                    &workspace.factor,
                    &workspace.permutation,
                    &mut workspace.inverse_lower,
                    &mut workspace.bread,
                )? {
                    return Err(invalid_data(
                        "retained weighted-linear Cholesky inverse is not finite",
                    ));
                }
                (
                    candidate_count,
                    0.0,
                    factor_report.maximum_diagonal / factor_report.minimum_diagonal,
                )
            } else {
                let report = factor_rank_revealing_retained_qr(
                    source,
                    &workspace.candidate_ids,
                    relative_threshold,
                    &mut workspace.row,
                    &mut workspace.factor,
                    &mut workspace.solve,
                    &mut workspace.scores,
                    &mut workspace.permutation,
                )?;
                if report.ambiguous {
                    return Err(invalid_data(
                        "retained weighted-linear rank decision is numerically ambiguous",
                    ));
                }
                let rank = report.rank;
                resize_zeroed(
                    &mut workspace.scores,
                    rank,
                    "unable to reserve retained rank pivot solution",
                )?;
                resize_zeroed(
                    &mut workspace.coefficients,
                    rank,
                    "unable to reserve retained rank coefficients",
                )?;
                workspace.kept.clear();
                workspace
                    .kept
                    .try_reserve(rank)
                    .map_err(|_| allocation("unable to reserve retained rank identities"))?;
                workspace.kept.resize(rank, 0);
                if !solve_rank_revealing_qr_leading(
                    &workspace.factor,
                    &workspace.solve,
                    &workspace.permutation,
                    &workspace.candidate_ids,
                    rank,
                    &mut workspace.scores,
                    &mut workspace.kept,
                    &mut workspace.coefficients,
                )? {
                    return Err(invalid_data(
                        "retained weighted-linear rank solution is not finite",
                    ));
                }
                let dropped_count = candidate_count - rank;
                let projection_elements = checked_elements(rank, dropped_count)?;
                resize_zeroed(
                    &mut workspace.inverse_lower,
                    projection_elements,
                    "unable to reserve retained dropped-span projection",
                )?;
                resize_zeroed(
                    &mut workspace.scores,
                    candidate_count,
                    "unable to reserve retained dropped-span row",
                )?;
                resize_zeroed(
                    &mut workspace.solve,
                    dropped_count,
                    "unable to reserve retained dropped-span norms",
                )?;
                let dropped_span = certify_retained_qr_dropped_span(
                    source,
                    &workspace.factor,
                    &workspace.permutation,
                    rank,
                    report.initial_norm,
                    relative_threshold,
                    &mut workspace.row,
                    &mut workspace.scores,
                    &mut workspace.inverse_lower,
                    &mut workspace.solve,
                )?;
                if !dropped_span.passed {
                    return Err(invalid_data(
                        "retained weighted-linear direct dropped-span certificate did not pass",
                    ));
                }
                let rank_square = checked_elements(rank, rank)?;
                resize_zeroed(
                    &mut workspace.inverse_lower,
                    rank_square,
                    "unable to reserve retained inverse-R workspace",
                )?;
                resize_zeroed(
                    &mut workspace.bread,
                    rank_square,
                    "unable to reserve retained rank inverse",
                )?;
                if !invert_rank_revealing_qr_leading(
                    &workspace.factor,
                    &workspace.permutation,
                    &workspace.candidate_ids,
                    rank,
                    &mut workspace.inverse_lower,
                    &mut workspace.kept,
                    &mut workspace.bread,
                )? {
                    return Err(invalid_data(
                        "retained weighted-linear rank inverse is not finite",
                    ));
                }
                (
                    rank,
                    report
                        .maximum_dropped_relative_norm
                        .max(dropped_span.maximum_dropped_relative_norm),
                    report.condition_proxy,
                )
            }
        };

        let mut active_positions =
            fallible_empty(rank, "unable to reserve retained active positions")?;
        let mut selected_candidate_positions = fallible_empty(
            rank,
            "unable to reserve retained selected candidate positions",
        )?;
        let mut generated_coefficients = fallible_filled(
            generated_count,
            None,
            "unable to reserve retained generated coefficients",
        )?;
        for (selected, generated) in workspace.kept.iter().copied().enumerate() {
            let storage_position = retained
                .stored_generated_columns
                .iter()
                .position(|candidate| *candidate == generated)
                .ok_or_else(|| invalid_argument("retained selected identity is absent"))?;
            let candidate_position = workspace
                .candidate_ids
                .iter()
                .position(|candidate| *candidate == generated)
                .ok_or_else(|| invalid_argument("retained selected candidate is absent"))?;
            let reference = retained.reference_norms[storage_position];
            let coefficient = workspace.coefficients[selected] / reference;
            if !coefficient.is_finite() {
                return Err(invalid_data(
                    "retained weighted-linear coefficient is not finite",
                ));
            }
            active_positions.push(storage_position);
            selected_candidate_positions.push(candidate_position);
            generated_coefficients[generated] = Some(coefficient);
            generated_statuses[generated] = PreparedOlsFitStatus::Kept;
        }
        let left_inverse_error = inverse_relative_infinity_norm(
            &workspace.gram,
            candidate_count,
            &selected_candidate_positions,
            &workspace.bread,
            rank,
            false,
        )?;
        let right_inverse_error = inverse_relative_infinity_norm(
            &workspace.gram,
            candidate_count,
            &selected_candidate_positions,
            &workspace.bread,
            rank,
            true,
        )?;
        if left_inverse_error > self.config.bread_tolerance
            || right_inverse_error > self.config.bread_tolerance
        {
            return Err(invalid_data(
                "retained weighted-linear inverse certificate did not pass",
            ));
        }

        let rank_square = checked_elements(rank, rank)?;
        let mut finite_bread = fallible_filled(
            rank_square,
            0.0_f64,
            "unable to reserve retained reference-scale bread",
        )?;
        let mut reference_crossproduct = fallible_filled(
            rank_square,
            0.0_f64,
            "unable to reserve retained reference crossproduct",
        )?;
        for left in 0..rank {
            let left_storage = active_positions[left];
            let left_reference = retained.reference_norms[left_storage];
            for right in 0..rank {
                let right_storage = active_positions[right];
                let right_reference = retained.reference_norms[right_storage];
                finite_bread[left * rank + right] = workspace.bread[left * rank + right]
                    / left_reference
                    / right_reference
                    / self.bread_weight_scale.reference_weight_multiplier;
                reference_crossproduct[left * rank + right] = workspace.gram
                    [selected_candidate_positions[left] * candidate_count
                        + selected_candidate_positions[right]]
                    * left_reference
                    * right_reference
                    * self.bread_weight_scale.reference_weight_multiplier;
            }
        }

        resize_zeroed(
            &mut workspace.row,
            stored_columns,
            "unable to reserve retained residual row",
        )?;
        workspace.residuals.clear();
        workspace
            .residuals
            .try_reserve(rows)
            .map_err(|_| allocation("unable to reserve retained within residuals"))?;
        workspace.residuals.resize(rows, 0.0);
        for (row, &within_value) in within_response.iter().enumerate() {
            retained.storage.fill_row(row, &mut workspace.row)?;
            let root_weight =
                normalized_solver_weight(self.weights.borrowed(), self.bread_weight_scale, row)?
                    .sqrt();
            let mut weighted_residual = root_weight * within_value;
            for (selected, storage_position) in active_positions.iter().copied().enumerate() {
                let standardized_value = root_weight * workspace.row[storage_position]
                    / retained.reference_norms[storage_position];
                weighted_residual = (-workspace.coefficients[selected])
                    .mul_add(standardized_value, weighted_residual);
            }
            workspace.residuals[row] = weighted_residual / root_weight;
            if !workspace.residuals[row].is_finite() {
                return Err(invalid_data(
                    "retained weighted-linear residual is not finite",
                ));
            }
        }
        let within_outcome_norm = stable_weighted_norm(rows, |row| {
            Ok((
                within_response[row],
                normalized_solver_weight(self.weights.borrowed(), self.bread_weight_scale, row)?,
            ))
        })?;
        let uncentered_initial_sum_of_squares =
            within_outcome_norm.mul_add(within_outcome_norm, 0.0);
        let normalized_initial_within_sum_of_squares = if self.prepared.fit_report_intercept()
            && !self.prepared.fit_has_absorbed_intercept()
        {
            let intercept = self
                .prepared
                .fit_model()
                .columns
                .iter()
                .position(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
                .ok_or_else(|| {
                    invalid_argument("retained weighted-linear ordinary intercept is absent")
                })?;
            let candidate = workspace
                .candidate_ids
                .iter()
                .position(|generated| *generated == intercept)
                .ok_or_else(|| {
                    invalid_data("retained weighted-linear ordinary intercept is not eligible")
                })?;
            let storage_position = workspace.candidate_positions[candidate];
            let standardized_diagonal = workspace.gram[candidate * candidate_count + candidate];
            let standardized_mean = workspace.rhs[candidate] / standardized_diagonal;
            let raw_mean = standardized_mean / retained.reference_norms[storage_position];
            if !standardized_diagonal.is_finite()
                || standardized_diagonal <= 0.0
                || !raw_mean.is_finite()
            {
                return Err(invalid_data(
                    "retained weighted-linear centered outcome mean is invalid",
                ));
            }
            let centered_norm = stable_weighted_norm(rows, |row| {
                retained.storage.fill_row(row, &mut workspace.row)?;
                Ok((
                    (-raw_mean).mul_add(workspace.row[storage_position], within_response[row]),
                    normalized_solver_weight(
                        self.weights.borrowed(),
                        self.bread_weight_scale,
                        row,
                    )?,
                ))
            })?;
            centered_norm * centered_norm
        } else {
            uncentered_initial_sum_of_squares
        };
        let mut reference_norms = fallible_filled(
            generated_count,
            0.0_f64,
            "unable to reserve retained full reference norms",
        )?;
        let mut score_norms = fallible_filled(
            generated_count,
            0.0_f64,
            "unable to reserve retained full score norms",
        )?;
        for (position, generated) in retained
            .stored_generated_columns
            .iter()
            .copied()
            .enumerate()
        {
            reference_norms[generated] = retained.reference_norms[position];
            score_norms[generated] = retained.within_norms[position];
        }
        for (generated, column) in self.prepared.fit_model().columns.iter().enumerate() {
            if matches!(column.kind, PreparedOlsColumnKind::Intercept)
                && reference_norms[generated] == 0.0
            {
                reference_norms[generated] = stable_weighted_norm(rows, |row| {
                    Ok((
                        self.prepared.fit_column_value(generated, row)?,
                        normalized_solver_weight(
                            self.weights.borrowed(),
                            self.bread_weight_scale,
                            row,
                        )?,
                    ))
                })?;
            }
        }
        let mut dropped = fallible_empty(
            generated_count.saturating_sub(rank),
            "unable to reserve retained dropped identities",
        )?;
        for (generated, status) in generated_statuses.iter().copied().enumerate() {
            if status != PreparedOlsFitStatus::Kept {
                dropped.push(generated);
            }
        }
        let prior_basis_certificate = if let Some(prior_active_positions) = prior_active_positions {
            if prior_active_positions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
                || prior_active_positions
                    .last()
                    .is_some_and(|position| *position >= stored_columns)
            {
                return Err(invalid_argument(
                    "strict weighted-linear prior basis positions are inconsistent",
                ));
            }
            let prior_rank = prior_active_positions.len();
            if prior_rank == 0 {
                Some(LinearWithinDesignCertificate {
                    left_inverse_relative_infinity_norm: 0.0,
                    right_inverse_relative_infinity_norm: 0.0,
                    passed: true,
                })
            } else {
                (|| -> Result<Option<LinearWithinDesignCertificate>> {
                    let mut prior_candidate_positions = fallible_empty(
                        prior_rank,
                        "unable to reserve strict weighted-linear prior candidate positions",
                    )?;
                    for storage_position in prior_active_positions.iter().copied() {
                        let Some(candidate_position) = workspace
                            .candidate_positions
                            .iter()
                            .position(|candidate| *candidate == storage_position)
                        else {
                            return Ok(None);
                        };
                        prior_candidate_positions.push(candidate_position);
                    }
                    let prior_square = checked_elements(prior_rank, prior_rank)?;
                    resize_zeroed(
                        &mut workspace.inverse_lower,
                        prior_square,
                        "unable to reserve strict weighted-linear prior Gram scratch",
                    )?;
                    for left in 0..prior_rank {
                        for right in 0..prior_rank {
                            workspace.inverse_lower[left * prior_rank + right] = workspace.gram
                                [prior_candidate_positions[left] * candidate_count
                                    + prior_candidate_positions[right]];
                        }
                    }
                    resize_zeroed(
                        &mut workspace.factor,
                        prior_square,
                        "unable to reserve strict weighted-linear prior factor",
                    )?;
                    resize_zeroed(
                        &mut workspace.scores,
                        prior_rank,
                        "unable to reserve strict weighted-linear prior RHS scratch",
                    )?;
                    for (target, candidate) in workspace
                        .scores
                        .iter_mut()
                        .zip(prior_candidate_positions.iter().copied())
                    {
                        *target = workspace.rhs[candidate];
                    }
                    workspace.permutation.clear();
                    workspace
                        .permutation
                        .try_reserve(prior_rank)
                        .map_err(|_| allocation("unable to reserve strict prior permutation"))?;
                    workspace.permutation.extend(0..prior_rank);
                    let maximum_diagonal = (0..prior_rank)
                        .map(|position| workspace.inverse_lower[position * prior_rank + position])
                        .fold(0.0_f64, f64::max);
                    if factor_guarded_cholesky_in_place(
                        &mut workspace.inverse_lower,
                        &mut workspace.scores,
                        &mut workspace.factor,
                        &mut workspace.permutation,
                        CholeskyPivotPolicy::Canonical,
                        maximum_diagonal * relative_threshold * relative_threshold,
                    )?
                    .is_none()
                    {
                        return Ok(None);
                    }
                    resize_zeroed(
                        &mut workspace.bread,
                        prior_square,
                        "unable to reserve strict weighted-linear prior inverse",
                    )?;
                    if !invert_guarded_cholesky(
                        &workspace.factor,
                        &workspace.permutation,
                        &mut workspace.inverse_lower,
                        &mut workspace.bread,
                    )? {
                        return Ok(None);
                    }
                    for left in 0..prior_rank {
                        for right in 0..prior_rank {
                            workspace.inverse_lower[left * prior_rank + right] = workspace.gram
                                [prior_candidate_positions[left] * candidate_count
                                    + prior_candidate_positions[right]];
                        }
                    }
                    let left = inverse_relative_infinity_norm(
                        &workspace.inverse_lower,
                        prior_rank,
                        &workspace.permutation,
                        &workspace.bread,
                        prior_rank,
                        false,
                    )?;
                    let right = inverse_relative_infinity_norm(
                        &workspace.inverse_lower,
                        prior_rank,
                        &workspace.permutation,
                        &workspace.bread,
                        prior_rank,
                        true,
                    )?;
                    if left > self.config.bread_tolerance || right > self.config.bread_tolerance {
                        return Ok(None);
                    }
                    Ok(Some(LinearWithinDesignCertificate {
                        left_inverse_relative_infinity_norm: left,
                        right_inverse_relative_infinity_norm: right,
                        passed: true,
                    }))
                })()?
            }
        } else {
            None
        };
        let within_certificate = LinearWithinDesignCertificate {
            left_inverse_relative_infinity_norm: left_inverse_error,
            right_inverse_relative_infinity_norm: right_inverse_error,
            passed: true,
        };
        let scratch_bytes = retained.storage.scratch_bytes();
        let kept = fallible_copy(&workspace.kept, "unable to copy retained kept identities")?;
        let canonical_candidates = fallible_copy(
            &workspace.candidate_ids,
            "unable to copy retained canonical candidates",
        )?;
        let finite_bread_generated_columns =
            fallible_copy(&workspace.kept, "unable to copy retained bread identities")?;
        let residual_matrix =
            DenseMatrix::from_row_major(rows, 1, std::mem::take(&mut workspace.residuals))?;
        let residual_absorbed = if self.config.absorb.solver == FeSolverKind::Map {
            self.fill_reusable_map_weights()?;
            self.absorb_reusable_map(residual_matrix)?
        } else {
            self.prepared.fit_absorb_matrix(
                self.weights.borrowed(),
                residual_matrix,
                self.config.absorb,
            )?
        };
        if !residual_absorbed.report().certified() {
            return Err(invalid_data(
                "retained weighted-linear residual absorption did not certify",
            ));
        }
        let (residuals, residual_absorption_report) = residual_absorbed.into_parts();
        let draft = RetainedFiniteFitDraft {
            generated_coefficients,
            generated_statuses,
            kept,
            dropped,
            rank_certificate: super::PreparedOlsRankCertificate {
                relative_threshold,
                maximum_dropped_trailing_norm: maximum_dropped_relative_norm,
                condition_proxy,
                passed: true,
            },
            canonical_candidates,
            standardized_gram: &self.frozen_workspace.gram,
            standardized_rhs: &self.frozen_workspace.rhs,
            finite_bread_generated_columns,
            finite_bread_values: finite_bread,
            finite_reference_crossproduct: reference_crossproduct,
            left_inverse_relative_infinity_norm: left_inverse_error,
            right_inverse_relative_infinity_norm: right_inverse_error,
            reference_norms,
            score_norms,
            within_outcome_norm,
            normalized_initial_within_sum_of_squares,
            residuals: residuals.into_row_major(),
            residual_absorption_report,
            absorption_report,
            scratch_bytes,
        };
        let fit = assemble_retained_weighted_linear_fit(
            self.prepared,
            LinearSolveFrame::from_parts(
                &self.response,
                self.weights.borrowed(),
                self.bread_weight_scale,
            )?,
            self.config,
            draft,
        )?;
        Ok(RetainedRankSolveDraft {
            fit,
            active_positions,
            within_certificate,
            prior_basis_certificate,
        })
    }

    fn full_retained_generated_columns(&self) -> Result<Vec<usize>> {
        let generated = self.prepared.fit_model().columns.len();
        let mut columns = fallible_empty(
            generated,
            "unable to reserve weighted-linear full retained identities",
        )?;
        for (column, metadata) in self.prepared.fit_model().columns.iter().enumerate() {
            if self.prepared.fit_has_absorbed_intercept()
                && matches!(metadata.kind, PreparedOlsColumnKind::Intercept)
            {
                continue;
            }
            columns.push(column);
        }
        Ok(columns)
    }

    /// Build the complete projected candidate design exactly once. Fast mode
    /// consumes the combined N-by-(K+1) absorbed matrix and compacts it
    /// forward in the same allocation; Low mode projects the complete design
    /// into one row-readable scratch artifact.
    fn build_full_retained_design(
        &mut self,
    ) -> Result<(RetainedWithinDesign, Vec<f64>, SolveReport)> {
        let rows = self.response.len();
        let stored_generated_columns = self.full_retained_generated_columns()?;
        let stored_columns = stored_generated_columns.len();
        let mut reference_norms = fallible_filled(
            stored_columns,
            0.0_f64,
            "unable to reserve weighted-linear retained reference norms",
        )?;
        let mut reference_means = fallible_filled(
            stored_columns,
            0.0_f64,
            "unable to reserve weighted-linear retained reference means",
        )?;
        let mut within_norms = fallible_filled(
            stored_columns,
            0.0_f64,
            "unable to reserve weighted-linear retained within norms",
        )?;

        let (storage, within_response, absorption_report) = match self.config.execution.mode {
            PreparedOlsExecutionMode::Fast => {
                let raw = self.prepared.fit_build_rhs_with_response(&self.response)?;
                let expected_columns = self
                    .prepared
                    .fit_model()
                    .columns
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| allocation("weighted-linear combined width overflows"))?;
                if raw.nrows() != rows || raw.ncols() != expected_columns {
                    return Err(invalid_argument(
                        "weighted-linear combined retained design is inconsistent",
                    ));
                }
                let raw_norms = self.weighted_dense_column_norms(&raw)?;
                let raw_means = self.weighted_dense_column_means(&raw)?;
                for (position, generated) in stored_generated_columns.iter().copied().enumerate() {
                    reference_norms[position] = raw_norms[generated + 1];
                    reference_means[position] = raw_means[generated + 1];
                }
                drop(raw_norms);
                drop(raw_means);
                let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
                    self.fill_reusable_map_weights()?;
                    self.absorb_reusable_map(raw)?
                } else {
                    self.prepared.fit_absorb_matrix(
                        self.weights.borrowed(),
                        raw,
                        self.config.absorb,
                    )?
                };
                if !absorbed.report().certified() {
                    return Err(invalid_data(
                        "weighted-linear combined design absorption did not certify",
                    ));
                }
                let (within, report) = absorbed.into_parts();
                let stride = within.ncols();
                let mut values = within.into_row_major();
                let mut norm_scales = fallible_filled(
                    stored_columns,
                    0.0_f64,
                    "unable to reserve weighted-linear retained norm scales",
                )?;
                let mut norm_sums = fallible_filled(
                    stored_columns,
                    1.0_f64,
                    "unable to reserve weighted-linear retained norm sums",
                )?;
                let mut response = fallible_filled(
                    rows,
                    0.0_f64,
                    "unable to reserve weighted-linear retained within response",
                )?;
                for (row, response_value) in response.iter_mut().enumerate().take(rows) {
                    let source_start = row
                        .checked_mul(stride)
                        .ok_or_else(|| allocation("weighted-linear combined offset overflows"))?;
                    *response_value = values[source_start];
                    let target_start = row.checked_mul(stored_columns).ok_or_else(|| {
                        allocation("weighted-linear retained compact offset overflows")
                    })?;
                    let root = normalized_solver_weight(
                        self.weights.borrowed(),
                        self.bread_weight_scale,
                        row,
                    )?
                    .sqrt();
                    for (target, generated) in stored_generated_columns.iter().copied().enumerate()
                    {
                        let value = values[source_start + generated + 1];
                        update_scaled_square(
                            root * value,
                            &mut norm_scales[target],
                            &mut norm_sums[target],
                        )?;
                        values[target_start + target] = value;
                    }
                }
                for column in 0..stored_columns {
                    within_norms[column] = stable_norm(norm_scales[column], norm_sums[column])?;
                }
                values.truncate(checked_elements(rows, stored_columns)?);
                (
                    RetainedWithinStorage::Dense(DenseMatrix::from_row_major(
                        rows,
                        stored_columns,
                        values,
                    )?),
                    response,
                    report,
                )
            }
            PreparedOlsExecutionMode::Low => {
                let raw_response = DenseMatrix::from_row_major(
                    rows,
                    1,
                    fallible_copy(
                        &self.response,
                        "unable to reserve weighted-linear low-memory response",
                    )?,
                )?;
                let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
                    self.fill_reusable_map_weights()?;
                    self.absorb_reusable_map(raw_response)?
                } else {
                    self.prepared.fit_absorb_matrix(
                        self.weights.borrowed(),
                        raw_response,
                        self.config.absorb,
                    )?
                };
                if !absorbed.report().certified() {
                    return Err(invalid_data(
                        "weighted-linear low-memory response absorption did not certify",
                    ));
                }
                let (response, mut report) = absorbed.into_parts();
                let mut scratch = PreparedOlsScratch::create_row_major(rows, stored_columns)?;
                let block_width = self
                    .config
                    .execution
                    .poolsize
                    .max(1)
                    .min(stored_columns.max(1));
                for start in (0..stored_columns).step_by(block_width) {
                    let end = start.saturating_add(block_width).min(stored_columns);
                    let block = end - start;
                    let mut raw_values = fallible_filled(
                        checked_elements(rows, block)?,
                        0.0_f64,
                        "unable to reserve weighted-linear row-major scratch block",
                    )?;
                    for row in 0..rows {
                        for local in 0..block {
                            raw_values[row * block + local] = self
                                .prepared
                                .fit_column_value(stored_generated_columns[start + local], row)?;
                        }
                    }
                    let raw = DenseMatrix::from_row_major(rows, block, raw_values)?;
                    let norms = self.weighted_dense_column_norms(&raw)?;
                    let means = self.weighted_dense_column_means(&raw)?;
                    reference_norms[start..end].copy_from_slice(&norms);
                    reference_means[start..end].copy_from_slice(&means);
                    drop(norms);
                    drop(means);
                    let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
                        self.absorb_reusable_map(raw)?
                    } else {
                        self.prepared.fit_absorb_matrix(
                            self.weights.borrowed(),
                            raw,
                            self.config.absorb,
                        )?
                    };
                    if !absorbed.report().certified() {
                        return Err(invalid_data(
                            "weighted-linear low-memory design absorption did not certify",
                        ));
                    }
                    let (within, block_report) = absorbed.into_parts();
                    merge_solve_report(&mut report, block_report)?;
                    let norms = self.weighted_dense_column_norms(&within)?;
                    within_norms[start..end].copy_from_slice(&norms);
                    drop(norms);
                    scratch.patch_row_major_block(start, block, within.as_slice())?;
                }
                (
                    RetainedWithinStorage::Scratch(scratch),
                    response.into_row_major(),
                    report,
                )
            }
        };
        Ok((
            RetainedWithinDesign {
                storage,
                stored_generated_columns,
                active_positions: Vec::new(),
                reference_norms,
                reference_means,
                within_norms,
                generations: WithinGenerations::from_session(self.generations),
                certificate: LinearWithinDesignCertificate {
                    left_inverse_relative_infinity_norm: f64::INFINITY,
                    right_inverse_relative_infinity_norm: f64::INFINITY,
                    passed: false,
                },
            },
            within_response,
            absorption_report,
        ))
    }

    fn weighted_dense_column_norms(&self, values: &DenseMatrix) -> Result<Vec<f64>> {
        if values.nrows() != self.response.len() {
            return Err(invalid_argument(
                "weighted-linear norm matrix is not aligned on observations",
            ));
        }
        let columns = values.ncols();
        let mut scales = fallible_filled(
            columns,
            0.0_f64,
            "unable to reserve weighted-linear matrix norm scales",
        )?;
        let mut sums = fallible_filled(
            columns,
            1.0_f64,
            "unable to reserve weighted-linear matrix norm sums",
        )?;
        for row in 0..values.nrows() {
            let root =
                normalized_solver_weight(self.weights.borrowed(), self.bread_weight_scale, row)?
                    .sqrt();
            for column in 0..columns {
                update_scaled_square(
                    root * values[(row, column)],
                    &mut scales[column],
                    &mut sums[column],
                )?;
            }
        }
        for column in 0..columns {
            scales[column] = stable_norm(scales[column], sums[column])?;
        }
        Ok(scales)
    }

    fn weighted_dense_column_means(&self, values: &DenseMatrix) -> Result<Vec<f64>> {
        if values.nrows() != self.response.len() {
            return Err(invalid_argument(
                "weighted-linear mean matrix is not aligned on observations",
            ));
        }
        let columns = values.ncols();
        let mut sums = fallible_filled(
            columns,
            0.0_f64,
            "unable to reserve weighted-linear matrix mean sums",
        )?;
        let mut corrections = fallible_filled(
            columns,
            0.0_f64,
            "unable to reserve weighted-linear matrix mean corrections",
        )?;
        let mut weight_sum = 0.0_f64;
        let mut weight_correction = 0.0_f64;
        let weights = self.weights.borrowed();
        let weight_scale = self.bread_weight_scale;
        for row in 0..values.nrows() {
            let weight = normalized_solver_weight(weights, weight_scale, row)?;
            compensated_add(weight, &mut weight_sum, &mut weight_correction)?;
            for column in 0..columns {
                compensated_add(
                    weight * values[(row, column)],
                    &mut sums[column],
                    &mut corrections[column],
                )?;
            }
        }
        let denominator = weight_sum + weight_correction;
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(invalid_data(
                "weighted-linear matrix mean denominator is invalid",
            ));
        }
        for column in 0..columns {
            sums[column] = (sums[column] + corrections[column]) / denominator;
            if !sums[column].is_finite() {
                return Err(invalid_data("weighted-linear matrix mean is not finite"));
            }
        }
        Ok(sums)
    }

    fn fill_weighted_dense_column_norms_reusing(
        &mut self,
        values: &DenseMatrix,
        output: &mut [f64],
    ) -> Result<bool> {
        if values.nrows() != self.response.len() || values.ncols() != output.len() {
            return Err(invalid_argument(
                "weighted-linear reusable norm matrix is not aligned",
            ));
        }
        let columns = output.len();
        let reused = self.frozen_workspace.scores.capacity() >= columns
            && self.frozen_workspace.solve.capacity() >= columns;
        resize_zeroed(
            &mut self.frozen_workspace.scores,
            columns,
            "unable to reserve weighted-linear reusable norm scales",
        )?;
        resize_zeroed(
            &mut self.frozen_workspace.solve,
            columns,
            "unable to reserve weighted-linear reusable norm sums",
        )?;
        self.frozen_workspace.solve.fill(1.0);
        let weights = self.weights.borrowed();
        let weight_scale = self.bread_weight_scale;
        for row in 0..values.nrows() {
            let root = normalized_solver_weight(weights, weight_scale, row)?.sqrt();
            for column in 0..columns {
                update_scaled_square(
                    root * values[(row, column)],
                    &mut self.frozen_workspace.scores[column],
                    &mut self.frozen_workspace.solve[column],
                )?;
            }
        }
        for (column, destination) in output.iter_mut().enumerate().take(columns) {
            *destination = stable_norm(
                self.frozen_workspace.scores[column],
                self.frozen_workspace.solve[column],
            )?;
        }
        Ok(reused)
    }

    fn refresh_retained_norms(&mut self) -> Result<()> {
        let mut retained = self
            .within
            .take()
            .ok_or_else(|| invalid_argument("weighted-linear retained design disappeared"))?;
        let result = (|| -> Result<()> {
            let columns = retained.stored_generated_columns.len();
            if retained.reference_norms.len() != columns
                || retained.reference_means.len() != columns
                || retained.within_norms.len() != columns
            {
                return Err(invalid_argument(
                    "weighted-linear refreshed reference workspaces are absent",
                ));
            }
            let reused = self.frozen_workspace.scores.capacity() >= columns
                && self.frozen_workspace.solve.capacity() >= columns
                && self.frozen_workspace.reference_mean_corrections.capacity() >= columns;
            resize_zeroed(
                &mut self.frozen_workspace.scores,
                columns,
                "unable to reserve weighted-linear refreshed reference scales",
            )?;
            resize_zeroed(
                &mut self.frozen_workspace.solve,
                columns,
                "unable to reserve weighted-linear refreshed reference sums",
            )?;
            resize_zeroed(
                &mut self.frozen_workspace.reference_mean_corrections,
                columns,
                "unable to reserve weighted-linear refreshed mean corrections",
            )?;
            self.frozen_workspace.solve.fill(1.0);
            retained.reference_means.fill(0.0);
            let weights = self.weights.borrowed();
            let weight_scale = self.bread_weight_scale;
            let mut weight_sum = 0.0_f64;
            let mut weight_correction = 0.0_f64;
            for row in 0..self.response.len() {
                let weight = normalized_solver_weight(weights, weight_scale, row)?;
                compensated_add(weight, &mut weight_sum, &mut weight_correction)?;
                let root = weight.sqrt();
                for (column, generated) in retained
                    .stored_generated_columns
                    .iter()
                    .copied()
                    .enumerate()
                {
                    let raw_value = self.prepared.fit_column_value(generated, row)?;
                    update_scaled_square(
                        root * raw_value,
                        &mut self.frozen_workspace.scores[column],
                        &mut self.frozen_workspace.solve[column],
                    )?;
                    compensated_add(
                        weight * raw_value,
                        &mut retained.reference_means[column],
                        &mut self.frozen_workspace.reference_mean_corrections[column],
                    )?;
                }
            }
            let denominator = weight_sum + weight_correction;
            if !denominator.is_finite() || denominator <= 0.0 {
                return Err(invalid_data(
                    "weighted-linear refreshed mean denominator is invalid",
                ));
            }
            for column in 0..columns {
                retained.reference_norms[column] = stable_norm(
                    self.frozen_workspace.scores[column],
                    self.frozen_workspace.solve[column],
                )?;
                retained.reference_means[column] = (retained.reference_means[column]
                    + self.frozen_workspace.reference_mean_corrections[column])
                    / denominator;
                if !retained.reference_means[column].is_finite() {
                    return Err(invalid_data("weighted-linear refreshed mean is not finite"));
                }
            }
            retained.generations = WithinGenerations::from_session(self.generations);
            if let Some(report) = self.reuse.as_mut() {
                report.kernel_solver_workspace_reused |= reused;
            }
            Ok(())
        })();
        self.within = Some(retained);
        result
    }

    #[cfg(test)]
    fn dense_from_fast_cache(
        &self,
        cache: PreparedOlsFastScoreCache,
        kept: &[usize],
    ) -> Result<RetainedWithinStorage> {
        let rows = self.prepared.fit_sample().n_active();
        let columns = kept.len();
        let model_columns = self.prepared.fit_model().columns.len();
        if kept.windows(2).any(|pair| pair[0] >= pair[1])
            || kept
                .last()
                .is_some_and(|generated| *generated >= model_columns)
        {
            return Err(invalid_argument(
                "weighted-linear retained coordinates are not canonical",
            ));
        }
        let elements = checked_elements(rows, columns)?;
        let mut sources = fallible_empty(
            columns,
            "unable to reserve weighted-linear retained cache coordinates",
        )?;
        let (mut values, source_stride, scales) = match cache {
            PreparedOlsFastScoreCache::Packed {
                generated_columns,
                within,
                means,
            } => {
                if within.nrows() != rows
                    || within.ncols() != generated_columns.len()
                    || means.len() != generated_columns.len()
                    || columns > generated_columns.len()
                    || generated_columns.windows(2).any(|pair| pair[0] >= pair[1])
                    || generated_columns
                        .last()
                        .is_some_and(|generated| *generated >= model_columns)
                {
                    return Err(invalid_argument(
                        "weighted-linear packed cache dimensions are inconsistent",
                    ));
                }
                for generated in kept.iter().copied() {
                    let source = generated_columns
                        .iter()
                        .position(|candidate| *candidate == generated)
                        .ok_or_else(|| {
                            invalid_argument("weighted-linear fast cache is missing a kept column")
                        })?;
                    sources.push(source);
                }
                (within.into_row_major(), generated_columns.len(), None)
            }
            PreparedOlsFastScoreCache::Strided {
                generated_columns,
                within,
                reference_norms,
                means,
            } => {
                if within.nrows() != rows
                    || reference_norms.len() != generated_columns.len()
                    || means.len() != generated_columns.len()
                    || columns > generated_columns.len()
                    || generated_columns.windows(2).any(|pair| pair[0] >= pair[1])
                    || generated_columns
                        .last()
                        .is_some_and(|generated| *generated >= model_columns)
                    || within.ncols()
                        != self
                            .prepared
                            .fit_model()
                            .columns
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| {
                                allocation("weighted-linear strided cache width overflows")
                            })?
                {
                    return Err(invalid_argument(
                        "weighted-linear strided cache dimensions are inconsistent",
                    ));
                }
                let mut scales = fallible_empty(
                    columns,
                    "unable to reserve weighted-linear retained cache scales",
                )?;
                for generated in kept.iter().copied() {
                    let source = generated_columns
                        .iter()
                        .position(|candidate| *candidate == generated)
                        .ok_or_else(|| {
                            invalid_argument(
                                "weighted-linear strided cache is missing a kept column",
                            )
                        })?;
                    let norm = reference_norms[source];
                    if !norm.is_finite() || norm <= 0.0 {
                        return Err(invalid_data(
                            "weighted-linear strided cache scale is not finite and positive",
                        ));
                    }
                    sources.push(generated.checked_add(1).ok_or_else(|| {
                        allocation("weighted-linear strided cache coordinate overflows")
                    })?);
                    scales.push(norm);
                }
                let stride = within.ncols();
                (within.into_row_major(), stride, Some(scales))
            }
        };
        if columns == 0 {
            return Ok(RetainedWithinStorage::Dense(DenseMatrix::from_row_major(
                rows,
                0,
                Vec::new(),
            )?));
        }
        let mut row_values = fallible_filled(
            columns,
            0.0,
            "unable to reserve weighted-linear retained cache row",
        )?;
        for row in 0..rows {
            let source_start = row
                .checked_mul(source_stride)
                .ok_or_else(|| allocation("weighted-linear retained cache offset overflows"))?;
            let root_weight = if scales.is_some() {
                self.root_solver_weight(row)?
            } else {
                1.0
            };
            for (target, source) in sources.iter().copied().enumerate() {
                let value = values[source_start + source];
                row_values[target] = scales
                    .as_ref()
                    .map_or(value, |scales| value * scales[target] / root_weight);
                if !row_values[target].is_finite() {
                    return Err(invalid_data(
                        "weighted-linear retained cache contains a nonfinite value",
                    ));
                }
            }
            let target_start = row
                .checked_mul(columns)
                .ok_or_else(|| allocation("weighted-linear compact cache offset overflows"))?;
            values[target_start..target_start + columns].copy_from_slice(&row_values);
        }
        values.truncate(elements);
        Ok(RetainedWithinStorage::Dense(DenseMatrix::from_row_major(
            rows, columns, values,
        )?))
    }

    fn fill_reusable_map_weights(&mut self) -> Result<bool> {
        if self
            .fe_projector
            .as_ref()
            .is_some_and(ReusableLinearMapProjector::borrows_raw_weights)
        {
            return Ok(true);
        }
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
    fn reproject_retained_within_in_place(&mut self) -> Result<bool> {
        let prior = self.within.take().ok_or_else(|| {
            invalid_argument("weighted-linear reusable MAP projection has no retained design")
        })?;
        let rows = prior.storage.rows();
        let columns = prior.storage.columns();
        let RetainedWithinDesign {
            storage: prior_storage,
            stored_generated_columns,
            active_positions,
            reference_norms,
            reference_means,
            mut within_norms,
            ..
        } = prior;
        if rows != self.response.len() {
            return Err(invalid_argument(
                "weighted-linear reusable MAP projection exceeds its fixed workspace",
            ));
        }
        let mut workspace_reused = true;
        if reference_norms.len() != columns
            || reference_means.len() != columns
            || within_norms.len() != columns
        {
            return Err(invalid_argument(
                "weighted-linear reusable norm workspaces are inconsistent",
            ));
        }
        within_norms.fill(0.0);
        let storage = match prior_storage {
            RetainedWithinStorage::Dense(values) => {
                if columns == 0 {
                    RetainedWithinStorage::Dense(values)
                } else {
                    if self.config.absorb.solver == FeSolverKind::Map
                        && columns > self.reusable_map_max_rhs()?
                    {
                        return Err(invalid_argument(
                            "weighted-linear reusable dense MAP projection exceeds its fixed workspace",
                        ));
                    }
                    let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
                        self.absorb_reusable_map(values)?
                    } else {
                        self.prepared.fit_absorb_matrix(
                            self.weights.borrowed(),
                            values,
                            self.config.absorb,
                        )?
                    };
                    let (values, report) = absorbed.into_parts();
                    if !report.certified() {
                        return Err(invalid_data(
                            "weighted-linear reusable dense projection did not certify",
                        ));
                    }
                    workspace_reused &=
                        self.fill_weighted_dense_column_norms_reusing(&values, &mut within_norms)?;
                    RetainedWithinStorage::Dense(values)
                }
            }
            RetainedWithinStorage::Scratch(mut scratch) => {
                let block_width = self.config.execution.poolsize.max(1).min(columns.max(1));
                if self.config.absorb.solver == FeSolverKind::Map
                    && block_width > self.reusable_map_max_rhs()?
                {
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
                    let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
                        self.absorb_reusable_map(raw)?
                    } else {
                        self.prepared.fit_absorb_matrix(
                            self.weights.borrowed(),
                            raw,
                            self.config.absorb,
                        )?
                    };
                    let (values, report) = absorbed.into_parts();
                    if !report.certified() {
                        return Err(invalid_data(
                            "weighted-linear reusable scratch projection did not certify",
                        ));
                    }
                    workspace_reused &= self.fill_weighted_dense_column_norms_reusing(
                        &values,
                        &mut within_norms[start..end],
                    )?;
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
            stored_generated_columns,
            active_positions,
            reference_norms,
            reference_means,
            within_norms,
            generations: WithinGenerations::from_session(self.generations),
            certificate: LinearWithinDesignCertificate {
                left_inverse_relative_infinity_norm: f64::INFINITY,
                right_inverse_relative_infinity_norm: f64::INFINITY,
                passed: false,
            },
        });
        Ok(workspace_reused)
    }

    /// Re-project the prior within controls and the exact response-delta warm
    /// start as one categorical MAP block. If `r_old = M_old z_old`, then
    /// `M_new(r_old + z_new - z_old) = M_new z_new`; no approximation enters
    /// the changing-weight solve. The retained dense allocation is expanded
    /// and compacted in place, so warmed PPML iterations need no second
    /// observation-sized projection matrix.
    fn reproject_retained_with_response_in_place(&mut self) -> Result<(bool, SolveReport)> {
        if !self
            .fe_projector
            .as_ref()
            .is_some_and(ReusableLinearMapProjector::is_ppml_categorical)
        {
            return Err(invalid_argument(
                "fused weighted-linear projection requires the PPML categorical MAP route",
            ));
        }
        let prior = self.within.take().ok_or_else(|| {
            invalid_argument("fused weighted-linear projection has no retained design")
        })?;
        let rows = prior.storage.rows();
        let columns = prior.storage.columns();
        let RetainedWithinDesign {
            storage,
            stored_generated_columns,
            active_positions,
            reference_norms,
            reference_means,
            within_norms,
            ..
        } = prior;
        let RetainedWithinStorage::Dense(matrix) = storage else {
            return Err(invalid_argument(
                "fused weighted-linear projection requires fast dense storage",
            ));
        };
        if rows != self.response.len()
            || self.frozen_workspace.response.len() != rows
            || self.map_weights.len() != rows
            || reference_norms.len() != columns
            || reference_means.len() != columns
            || within_norms.len() != columns
        {
            return Err(invalid_argument(
                "fused weighted-linear response/design state is not aligned",
            ));
        }
        let stride = columns
            .checked_add(1)
            .ok_or_else(|| allocation("fused weighted-linear stride overflows"))?;
        let elements = checked_elements(rows, stride)?;
        let mut values = matrix.into_row_major();
        let workspace_reused = values.capacity() >= elements;
        values
            .try_reserve(elements.saturating_sub(values.len()))
            .map_err(|_| allocation("unable to expand fused weighted-linear projection"))?;
        values.resize(elements, 0.0);
        for row in (0..rows).rev() {
            let source = row
                .checked_mul(columns)
                .ok_or_else(|| allocation("fused weighted-linear source offset overflows"))?;
            let target = row
                .checked_mul(stride)
                .ok_or_else(|| allocation("fused weighted-linear target offset overflows"))?;
            if columns > 0 {
                values.copy_within(source..source + columns, target + 1);
            }
            let response =
                self.frozen_workspace.response[row] + (self.response[row] - self.map_weights[row]);
            if !response.is_finite() {
                return Err(invalid_data(
                    "fused weighted-linear response warm start is nonfinite",
                ));
            }
            values[target] = response;
        }

        let combined = DenseMatrix::from_row_major(rows, stride, values)?;
        let absorbed = self.absorb_reusable_map(combined)?;
        if !absorbed.report().certified() {
            return Err(invalid_data(
                "fused weighted-linear response/design projection did not certify",
            ));
        }
        let (combined, report) = absorbed.into_parts();
        let mut values = combined.into_row_major();
        resize_zeroed(
            &mut self.frozen_workspace.response,
            rows,
            "unable to retain fused weighted-linear response",
        )?;
        for row in 0..rows {
            let source = row
                .checked_mul(stride)
                .ok_or_else(|| allocation("fused weighted-linear split offset overflows"))?;
            self.frozen_workspace.response[row] = values[source];
            if columns > 0 {
                let target = row
                    .checked_mul(columns)
                    .ok_or_else(|| allocation("fused weighted-linear compact offset overflows"))?;
                values.copy_within(source + 1..source + stride, target);
            }
        }
        values.truncate(checked_elements(rows, columns)?);
        self.within = Some(RetainedWithinDesign {
            storage: RetainedWithinStorage::Dense(DenseMatrix::from_row_major(
                rows, columns, values,
            )?),
            stored_generated_columns,
            active_positions,
            reference_norms,
            reference_means,
            within_norms,
            generations: WithinGenerations::from_session(self.generations),
            certificate: LinearWithinDesignCertificate {
                left_inverse_relative_infinity_norm: f64::INFINITY,
                right_inverse_relative_infinity_norm: f64::INFINITY,
                passed: false,
            },
        });
        Ok((workspace_reused, report))
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
        let mut certificate = LinearReconstructionCertificate::new();
        for row in 0..rows {
            self.absorbed_predictor[row] =
                (self.response[row] - fit.residuals()[row]) - self.finite_predictor[row];
            certificate.observe(
                self.response[row],
                self.finite_predictor[row],
                self.absorbed_predictor[row],
                fit.residuals()[row],
            )?;
        }
        Ok(certificate)
    }

    #[cfg(test)]
    fn root_solver_weight(&self, row: usize) -> Result<f64> {
        let weight =
            normalized_solver_weight(self.weights.borrowed(), self.bread_weight_scale, row)?;
        let root = weight.sqrt();
        if root.is_finite() && root > 0.0 {
            Ok(root)
        } else {
            Err(invalid_data(
                "weighted-linear normalized root weight is invalid",
            ))
        }
    }
}

struct FrozenSolveDraft {
    coefficients: Vec<f64>,
    bread: Vec<f64>,
    within_residuals: Vec<f64>,
    certificate: FrozenLinearSolveCertificate,
}

struct RetainedRankSolveDraft {
    fit: CertifiedPreparedOlsFit,
    active_positions: Vec<usize>,
    within_certificate: LinearWithinDesignCertificate,
    prior_basis_certificate: Option<LinearWithinDesignCertificate>,
}

struct RetainedSolveStage {
    candidate: RetainedRankSolveDraft,
    candidate_basis: Option<FrozenLinearBasis>,
    report: LinearSessionReuseReport,
    #[cfg(test)]
    response_fused_with_design: bool,
}

#[allow(clippy::too_many_arguments)]
fn stream_frozen_solve(
    storage: &RetainedWithinStorage,
    active_positions: &[usize],
    reference_norms: &[f64],
    within_response: &[f64],
    weights: BorrowedMapWeights<'_>,
    bread_weight_scale: super::PreparedOlsBreadWeightScale,
    config: PreparedOlsFitConfig,
    workspace: &mut FrozenLinearWorkspace,
) -> Result<FrozenSolveDraft> {
    let rows = storage.rows();
    let stored_columns = storage.columns();
    let rank = active_positions.len();
    if within_response.len() != rows
        || reference_norms.len() != stored_columns
        || active_positions.windows(2).any(|pair| pair[0] >= pair[1])
        || active_positions
            .last()
            .is_some_and(|position| *position >= stored_columns)
    {
        return Err(invalid_argument(
            "frozen weighted-linear response and design axes are inconsistent",
        ));
    }
    if rank == 0 {
        workspace.coefficients.clear();
        workspace.bread.clear();
        workspace.residuals.clear();
        workspace
            .residuals
            .try_reserve(rows)
            .map_err(|_| allocation("unable to reserve frozen weighted-linear residuals"))?;
        workspace.residuals.extend_from_slice(within_response);
        let mut weighted_residual_square = 0.0_f64;
        for (row, residual) in workspace.residuals.iter().copied().enumerate() {
            let weight = normalized_solver_weight(weights, bread_weight_scale, row)?;
            weighted_residual_square =
                (weight * residual).mul_add(residual, weighted_residual_square);
        }
        if !weighted_residual_square.is_finite()
            || workspace.residuals.iter().any(|value| !value.is_finite())
        {
            return Err(invalid_data(
                "frozen weighted-linear residual is not finite",
            ));
        }
        return Ok(FrozenSolveDraft {
            coefficients: std::mem::take(&mut workspace.coefficients),
            bread: std::mem::take(&mut workspace.bread),
            within_residuals: std::mem::take(&mut workspace.residuals),
            certificate: FrozenLinearSolveCertificate {
                score_relative_infinity_norm: 0.0,
                left_inverse_relative_infinity_norm: 0.0,
                right_inverse_relative_infinity_norm: 0.0,
                condition_proxy: 1.0,
                passed: true,
            },
        });
    }
    let square = checked_elements(rank, rank)?;
    workspace.candidate_scales.clear();
    workspace
        .candidate_scales
        .try_reserve(rank)
        .map_err(|_| allocation("unable to reserve frozen weighted-linear scales"))?;
    for position in active_positions.iter().copied() {
        let scale = reference_norms[position];
        if !scale.is_finite() || scale <= 0.0 {
            return Err(invalid_data(
                "frozen weighted-linear reference scale is invalid",
            ));
        }
        workspace.candidate_scales.push(scale);
    }
    let source = match storage {
        RetainedWithinStorage::Dense(values) => FixedBlockMomentSource::RetainedDense {
            values: values.as_slice(),
            rows,
            stored_columns,
            selected_columns: Some(active_positions),
            response: within_response,
            weights,
            solver_weight_divisor: bread_weight_scale.solver_weight_divisor,
            column_scales: Some(&workspace.candidate_scales),
        },
        RetainedWithinStorage::Scratch(values) => FixedBlockMomentSource::RetainedScratch {
            values,
            selected_columns: Some(active_positions),
            response: within_response,
            weights,
            solver_weight_divisor: bread_weight_scale.solver_weight_divisor,
            column_scales: Some(&workspace.candidate_scales),
        },
    };
    let moment_layout = finite_wls_moment_layout(source)?;
    resize_zeroed(
        &mut workspace.moment_partials,
        moment_layout.partial_elements,
        "unable to reserve frozen weighted-linear moment workspace",
    )?;
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
    fill_finite_wls_moments(
        source,
        moment_layout,
        &mut workspace.moment_partials,
        &mut workspace.rhs,
        &mut workspace.gram,
    )?;
    resize_zeroed(
        &mut workspace.row,
        stored_columns,
        "unable to reserve frozen weighted-linear row buffer",
    )?;
    resize_zeroed(
        &mut workspace.factor,
        square,
        "unable to reserve frozen weighted-linear Cholesky factor",
    )?;
    resize_zeroed(
        &mut workspace.inverse_lower,
        square,
        "unable to reserve frozen weighted-linear factor workspace",
    )?;
    workspace.inverse_lower.copy_from_slice(&workspace.gram);
    resize_zeroed(
        &mut workspace.solve,
        rank,
        "unable to reserve frozen weighted-linear factor RHS",
    )?;
    workspace.solve.copy_from_slice(&workspace.rhs);
    workspace.permutation.clear();
    workspace
        .permutation
        .try_reserve(rank)
        .map_err(|_| allocation("unable to reserve frozen weighted-linear permutation"))?;
    workspace.permutation.extend(0..rank);
    let maximum_diagonal = (0..rank)
        .map(|index| workspace.gram[index * rank + index])
        .fold(0.0_f64, f64::max);
    let relative_threshold =
        config.rank_tolerance_multiplier * f64::EPSILON * rows.max(stored_columns).max(1) as f64;
    let strong_pivot_guard = maximum_diagonal * relative_threshold.sqrt();
    let fast_factor = if maximum_diagonal.is_finite()
        && maximum_diagonal > 0.0
        && strong_pivot_guard.is_finite()
        && strong_pivot_guard > 0.0
    {
        factor_guarded_cholesky_in_place(
            &mut workspace.inverse_lower,
            &mut workspace.solve,
            &mut workspace.factor,
            &mut workspace.permutation,
            CholeskyPivotPolicy::MaximumDiagonal,
            strong_pivot_guard,
        )?
    } else {
        None
    };
    resize_zeroed(
        &mut workspace.bread,
        square,
        "unable to reserve frozen weighted-linear inverse",
    )?;
    resize_zeroed(
        &mut workspace.scores,
        rank,
        "unable to reserve frozen weighted-linear solve scratch",
    )?;
    resize_zeroed(
        &mut workspace.coefficients,
        rank,
        "unable to reserve frozen weighted-linear coefficients",
    )?;
    workspace.kept.clear();
    workspace
        .kept
        .try_reserve(rank)
        .map_err(|_| allocation("unable to reserve frozen weighted-linear identities"))?;
    workspace.kept.resize(rank, 0);
    let condition_proxy = if let Some(factor_report) = fast_factor {
        if !solve_guarded_cholesky(
            &workspace.factor,
            &workspace.solve,
            &mut workspace.scores,
            &mut workspace.coefficients,
        )? {
            return Err(invalid_data(
                "frozen weighted-linear Cholesky solution is not finite",
            ));
        }
        workspace.row[..rank].fill(0.0);
        for position in 0..rank {
            workspace.row[workspace.permutation[position]] = workspace.coefficients[position];
        }
        workspace
            .coefficients
            .copy_from_slice(&workspace.row[..rank]);
        if !invert_guarded_cholesky(
            &workspace.factor,
            &workspace.permutation,
            &mut workspace.inverse_lower,
            &mut workspace.bread,
        )? {
            return Err(invalid_data(
                "frozen weighted-linear Cholesky inverse is not finite",
            ));
        }
        factor_report.maximum_diagonal / factor_report.minimum_diagonal
    } else {
        #[cfg(test)]
        FROZEN_QR_FALLBACK_OBSERVED.store(true, Ordering::SeqCst);
        let report = factor_rank_revealing_retained_qr(
            source,
            active_positions,
            relative_threshold,
            &mut workspace.row,
            &mut workspace.factor,
            &mut workspace.solve,
            &mut workspace.scores,
            &mut workspace.permutation,
        )?;
        if report.ambiguous || report.rank != rank {
            return Err(invalid_data(
                "frozen weighted-linear basis lost certified full rank",
            ));
        }
        if !solve_rank_revealing_qr_leading(
            &workspace.factor,
            &workspace.solve,
            &workspace.permutation,
            active_positions,
            rank,
            &mut workspace.scores,
            &mut workspace.kept,
            &mut workspace.coefficients,
        )? {
            return Err(invalid_data(
                "frozen weighted-linear QR solution is not finite",
            ));
        }
        if !invert_rank_revealing_qr_leading(
            &workspace.factor,
            &workspace.permutation,
            active_positions,
            rank,
            &mut workspace.inverse_lower,
            &mut workspace.kept,
            &mut workspace.bread,
        )? {
            return Err(invalid_data(
                "frozen weighted-linear QR inverse is not finite",
            ));
        }
        report.condition_proxy
    };
    if !condition_proxy.is_finite() {
        return Err(invalid_data(
            "frozen weighted-linear condition proxy is not finite",
        ));
    }
    workspace.candidate_positions.clear();
    workspace
        .candidate_positions
        .try_reserve(rank)
        .map_err(|_| allocation("unable to reserve frozen inverse certificate positions"))?;
    workspace.candidate_positions.extend(0..rank);
    let left_inverse_error = inverse_relative_infinity_norm(
        &workspace.gram,
        rank,
        &workspace.candidate_positions,
        &workspace.bread,
        rank,
        false,
    )?;
    let right_inverse_error = inverse_relative_infinity_norm(
        &workspace.gram,
        rank,
        &workspace.candidate_positions,
        &workspace.bread,
        rank,
        true,
    )?;

    workspace.scores.copy_from_slice(&workspace.coefficients);
    for (coefficient, scale) in workspace
        .coefficients
        .iter_mut()
        .zip(workspace.candidate_scales.iter().copied())
    {
        *coefficient /= scale;
        if !coefficient.is_finite() {
            return Err(invalid_data(
                "frozen weighted-linear rescaled coefficient is not finite",
            ));
        }
    }

    workspace.residuals.clear();
    workspace
        .residuals
        .try_reserve(rows)
        .map_err(|_| allocation("unable to reserve frozen weighted-linear residuals"))?;
    workspace.residuals.extend_from_slice(within_response);
    let mut weighted_residual_square = 0.0_f64;
    for row in 0..rows {
        storage.fill_row(row, &mut workspace.row)?;
        let mut fitted = 0.0_f64;
        for (column, &active_position) in active_positions.iter().enumerate().take(rank) {
            fitted = workspace.coefficients[column].mul_add(workspace.row[active_position], fitted);
        }
        workspace.residuals[row] -= fitted;
        let weight = normalized_solver_weight(weights, bread_weight_scale, row)?;
        weighted_residual_square = (weight * workspace.residuals[row])
            .mul_add(workspace.residuals[row], weighted_residual_square);
    }
    let residual_norm = weighted_residual_square.max(0.0).sqrt();
    let score_certificate = certify_normal_equations(
        &workspace.gram,
        &workspace.rhs,
        NormalEquationScale::Frozen {
            coefficients: &workspace.scores,
            residual_norm,
        },
    )?;
    let maximum_score = score_certificate.relative_infinity_norm;
    if workspace.residuals.iter().any(|value| !value.is_finite())
        || !maximum_score.is_finite()
        || !score_certificate.relative_two_norm.is_finite()
    {
        return Err(invalid_data(
            "frozen weighted-linear residual or score is not finite",
        ));
    }
    let passed = maximum_score <= config.score_tolerance
        && left_inverse_error <= config.bread_tolerance
        && right_inverse_error <= config.bread_tolerance;
    let reference_multiplier = bread_weight_scale.reference_weight_multiplier;
    for left in 0..rank {
        for right in 0..rank {
            let value = &mut workspace.bread[left * rank + right];
            *value /= workspace.candidate_scales[left]
                * workspace.candidate_scales[right]
                * reference_multiplier;
            if !value.is_finite() {
                return Err(invalid_data("frozen weighted-linear bread is not finite"));
            }
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

fn weighted_mean_slice(
    values: &[f64],
    weights: BorrowedMapWeights<'_>,
    bread_weight_scale: super::PreparedOlsBreadWeightScale,
) -> Result<f64> {
    let mut numerator = 0.0_f64;
    let mut numerator_correction = 0.0_f64;
    let mut denominator = 0.0_f64;
    let mut denominator_correction = 0.0_f64;
    for (row, value) in values.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(invalid_data(
                "frozen weighted-linear mean input is not finite",
            ));
        }
        let weight = normalized_solver_weight(weights, bread_weight_scale, row)?;
        compensated_add(weight * value, &mut numerator, &mut numerator_correction)?;
        compensated_add(weight, &mut denominator, &mut denominator_correction)?;
    }
    let denominator = denominator + denominator_correction;
    let mean = (numerator + numerator_correction) / denominator;
    if denominator.is_finite() && denominator > 0.0 && mean.is_finite() {
        Ok(mean)
    } else {
        Err(invalid_data("frozen weighted-linear mean is not finite"))
    }
}

fn certify_frozen_combined_predictor(
    response: &[f64],
    within_residuals: &[f64],
) -> Result<LinearReconstructionCertificate> {
    if response.len() != within_residuals.len() {
        return Err(invalid_argument(
            "frozen PPML combined predictor dimensions are inconsistent",
        ));
    }
    let mut certificate = LinearReconstructionCertificate::new();
    for (response, residual) in response
        .iter()
        .copied()
        .zip(within_residuals.iter().copied())
    {
        certificate.observe(response, 0.0, response - residual, residual)?;
    }
    Ok(certificate)
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

fn stable_weighted_norm<F>(rows: usize, mut value: F) -> Result<f64>
where
    F: FnMut(usize) -> Result<(f64, f64)>,
{
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for row in 0..rows {
        let (value, weight) = value(row)?;
        let root = weight.sqrt();
        if !root.is_finite() || root <= 0.0 {
            return Err(invalid_data(
                "weighted-linear norm weight is not finite and positive",
            ));
        }
        update_scaled_square(root * value, &mut scale, &mut scaled_square_sum)?;
    }
    stable_norm(scale, scaled_square_sum)
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    if !value.is_finite() || !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "weighted-linear compensated sum input is not finite",
        ));
    }
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "weighted-linear compensated sum is not finite",
        ))
    }
}

fn update_scaled_square(value: f64, scale: &mut f64, sum: &mut f64) -> Result<()> {
    let absolute = value.abs();
    if !absolute.is_finite() {
        return Err(invalid_data("weighted-linear norm value is not finite"));
    }
    if absolute != 0.0 {
        if *scale < absolute {
            let ratio = *scale / absolute;
            *sum = 1.0 + *sum * ratio * ratio;
            *scale = absolute;
        } else {
            let ratio = absolute / *scale;
            *sum += ratio * ratio;
        }
    }
    if scale.is_finite() && sum.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "weighted-linear norm accumulator is not finite",
        ))
    }
}

fn stable_norm(scale: f64, scaled_square_sum: f64) -> Result<f64> {
    let norm = if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    };
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(invalid_data("weighted-linear norm is not finite"))
    }
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

#[cfg(test)]
mod reconstruction_tests {
    use super::{certify_frozen_combined_predictor, LinearReconstructionCertificate};

    #[test]
    fn reconstruction_matches_exact_rational_oracle_and_rejects_corruption() {
        let oracle = include_str!("../../../../tests/fixtures/ppml/reconstruction.csv");
        let mut cancellation_rejections = 0;
        for line in oracle.lines().skip(1) {
            let fields: Vec<_> = line.split(',').collect();
            let values: Vec<_> = fields[1..6]
                .iter()
                .map(|hex| f64::from_bits(u64::from_str_radix(hex, 16).unwrap()))
                .collect();
            let [z, f, a, r, exact_discrepancy] = values[..] else {
                panic!("oracle shape")
            };
            let expected = fields[6] == "1";
            let mut certificate = LinearReconstructionCertificate::new();
            certificate
                .observe(z, f, a, r)
                .expect("finite oracle input");
            assert_eq!(
                certificate.passed, expected,
                "{}: {certificate:?}, exact discrepancy={exact_discrepancy}",
                fields[0]
            );
            if expected && certificate.relative_infinity_norm > 64.0 * f64::EPSILON {
                cancellation_rejections += 1;
                assert!(exact_discrepancy > 0.0);
            }
            if expected {
                // Both existing subtraction orders must obey the same bound;
                // neither is used to construct the independent oracle above.
                for computed in [(z - f) - r, (z - r) - f] {
                    let mut constructed = LinearReconstructionCertificate::new();
                    constructed.observe(z, f, computed, r).unwrap();
                    assert!(constructed.passed, "{}: {constructed:?}", fields[0]);
                }
                assert!(
                    certify_frozen_combined_predictor(&[z], &[r])
                        .unwrap()
                        .passed
                );
            }
        }
        assert!(
            cancellation_rejections >= 3,
            "must exercise the former cancellation failure"
        );
    }

    #[test]
    fn reconstruction_rejects_nonfinite_and_overflowed_values() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for position in 0..4 {
                let mut values = [1.0, 0.25, 0.25, 0.5];
                values[position] = bad;
                let mut certificate = LinearReconstructionCertificate::new();
                assert!(certificate
                    .observe(values[0], values[1], values[2], values[3])
                    .is_err());
            }
        }
        let mut certificate = LinearReconstructionCertificate::new();
        assert!(certificate.observe(1.0, f64::MAX, f64::MAX, 0.0).is_err());
        assert!(certify_frozen_combined_predictor(&[f64::MAX], &[-f64::MAX]).is_err());
    }

    #[test]
    fn reconstruction_failure_cannot_be_erased_by_a_later_row() {
        let mut certificate = LinearReconstructionCertificate::new();
        certificate.observe(1.0, 0.0, 1.001, 0.0).unwrap();
        assert!(!certificate.passed);
        let maximum = certificate.relative_infinity_norm;
        certificate.observe(1.0, 0.0, 1.0, 0.0).unwrap();
        assert!(!certificate.passed);
        assert_eq!(certificate.relative_infinity_norm, maximum);
    }
}
