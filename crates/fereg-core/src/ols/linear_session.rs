//! Reusable, crate-private state for certified prepared weighted-linear solves.
//!
//! Public OLS remains deliberately one-shot.  This session is the iterative
//! estimator boundary: it owns changing response/weight buffers, freezes one
//! canonical rank basis, and retains the certified within design needed by
//! PPML inference. Canonical fresh/strict and frozen operations borrow
//! the stateless finite service and its physical moment/factor kernels. Reuse is reported per route and never inferred.

use super::canonical_products::CanonicalMoments;
use super::implicit_centered::ImplicitCenteredDesign;
use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, invalid_data, ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedMapWeights, CertifiedBorrowedMap, FeDesign, FeSession, FeSolverKind,
    MapGeometryGeneration, ReusableCategoricalMapAbsorber, ReusableMapAbsorber,
    ReusableMapCapacities, SolveReport,
};
use crate::linalg::neumaier_add;
use crate::memory::{fallible_copy, fallible_empty, fallible_filled};
use crate::metadata::CoefficientSlot;
use crate::parallel::ThreadTeam;
use crate::prepared_fit_view::{PreparedOlsFitView, SharedPreparedOlsData};
use crate::sandwich::SandwichDesign;
use rayon::prelude::*;
#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[path = "linear_constant.rs"]
mod linear_constant;
pub(crate) use linear_constant::LinearConstantRepresentation;

#[path = "linear_frame.rs"]
mod linear_frame;
pub(crate) use linear_frame::{
    CanonicalLinearProof, CertifiedLinearFrame, LinearFrameProof, LinearPredictors,
};

#[path = "bias_linear.rs"]
mod bias_linear;

#[path = "retained_refinement.rs"]
mod retained_refinement;
pub(crate) use retained_refinement::coefficient_accuracy_scale;
use retained_refinement::{refine_workspace, RefinementFrame};

use super::finite_wls_kernel::{
    certify_finite_inverse, certify_normal_equations, factor_guarded_cholesky_in_place,
    fill_finite_wls_moments, finite_wls_moment_layout, inverse_relative_infinity_norm,
    invert_guarded_cholesky, solve_guarded_cholesky, CholeskyPivotPolicy, FixedBlockMomentSource,
    NormalEquationScale,
};
#[cfg(test)]
use super::prepared_fit::PreparedOlsFastScoreCache;
use super::prepared_fit::{
    assemble_retained_linear_frame, build_finite_score_draft, merge_solve_report,
    CurrentFrameColumnMeans, FiniteBreadDraft, FiniteFit, LinearSolveFrame, RetainedFitContext,
};
use super::prepared_scratch::{PreparedOlsScratch, PreparedOlsScratchReader};
use super::{
    PreparedOlsColumnKind, PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsFitStatus,
};

#[path = "linear_statistics.rs"]
mod linear_statistics;

/// Monotone identities for every input class that can invalidate a solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LinearSessionGenerations {
    pub(crate) sample: u64,
    pub(crate) topology: u64,
    pub(crate) weights: u64,
    pub(crate) response: u64,
    pub(crate) basis: u64,
}

/// Canonical projection banks belong to the base plan in either storage mode.
/// The coefficient operation adds its numerical workspace and the second
/// response/normalized-weight bank. Admission and installation share this bound.
pub(crate) fn ppml_coefficient_lifetime_bytes(
    design: &FeDesign,
    rhs: usize,
    threads: usize,
) -> Result<u128> {
    ReusableMapAbsorber::coefficient_solver_bytes(design, rhs, threads)?
        .checked_add(8 * design.nobs() as u128)
        .ok_or_else(|| allocation("PPML coefficient workspace overlap overflows"))
}

/// Component scaling and its three-effect topology overlap the LSMR frame.
pub(crate) fn ppml_component_checkpoint_bytes(
    design: &FeDesign,
    rhs: usize,
    threads: usize,
) -> Result<u128> {
    if design.effects().len() != 3 {
        return Err(invalid_argument(
            "component checkpoint requires three effects",
        ));
    }
    let levels: [usize; 3] = std::array::from_fn(|e| design.effects()[e].n_levels());
    let component =
        8 * design.nobs() as u128 + 32 * levels.iter().map(|&n| n as u128).sum::<u128>() + 512;
    crate::ppml::estimate_checkpoint_lsmr_workspace_bytes(design.nobs(), rhs, &levels, threads)?
        .checked_add(component)
        .ok_or_else(|| allocation("component checkpoint workspace overflows"))
}

impl LinearSessionGenerations {
    const fn map_geometry(self) -> MapGeometryGeneration {
        MapGeometryGeneration {
            sample: self.sample,
            topology: self.topology,
            weights: self.weights,
        }
    }

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
    pub(crate) fe: ReusableMapCapacities,
    pub(crate) constant_recipe_elements: usize,
    pub(crate) within_metadata: [usize; 5],
    pub(crate) basis_metadata: [usize; 3],
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
    pub(crate) frozen_basis_slot_elements: usize,
    pub(crate) map_weight_elements: usize,
    pub(crate) projection_block_elements: usize,
    pub(crate) within_resident_elements: usize,
    pub(crate) within_scratch_bytes: u64,
    pub(crate) within_io_buffer_elements: usize,
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
    frame: Option<CertifiedLinearFrame>,
    constant: Option<LinearConstantRepresentation>,
    reconstruction: Option<LinearReconstructionCertificate>,
    reuse: Option<LinearSessionReuseReport>,
    projection_refresh_required: bool,
    canonical_projection_work_hint: u128,
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

    #[cfg(test)]
    pub(crate) fn rollback_residual_capacity(&self) -> usize {
        self.rollback
            .as_ref()
            .and_then(|rollback| rollback.frame.as_ref())
            .map_or(0, |frame| frame.within_residuals.capacity())
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
    #[cfg(test)]
    pub(crate) fn dropped_generated_columns(&self) -> &[usize] {
        &self.dropped_generated_columns
    }

    pub(super) fn from_slots(
        coefficients: &[super::PreparedOlsFittedCoefficient],
        kept_generated_columns: Vec<usize>,
        dropped_generated_columns: Vec<usize>,
    ) -> Result<Self> {
        let mut slots = fallible_empty(
            coefficients.len(),
            "unable to reserve frozen weighted-linear basis slots",
        )?;
        slots.extend(
            coefficients
                .iter()
                .map(|coefficient| FrozenLinearBasisSlot {
                    coefficient_slot: coefficient.coefficient_slot,
                    generated_column_index: coefficient.generated_column_index,
                    status: coefficient.status,
                }),
        );
        Ok(Self {
            slots,
            kept_generated_columns,
            dropped_generated_columns,
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
    Implicit(ImplicitCenteredDesign),
    Dense(DenseMatrix),
    /// The scratch file is row-major so a complete inference row is one read.
    Scratch(PreparedOlsScratch),
}

/// Ordinary joint directions need an FE-equivalent finite representative, not
/// a current within design. The immutable originals avoid repeated scratch
/// scans; canonical rank and inference still consume the retained design.
struct JointOriginalColumns<'a, P: PreparedOlsFitView + ?Sized> {
    prepared: &'a P,
    generated: &'a [usize],
    numeric: Option<super::prepared_rhs::NumericOlsColumns<'a>>,
}

impl<P: PreparedOlsFitView + Sync + ?Sized> crate::fe::slope_coefficients::joint::JointFiniteDesign
    for JointOriginalColumns<'_, P>
{
    fn prefers_column_scans(&self) -> bool {
        self.numeric.is_none()
    }

    fn nrows(&self) -> usize {
        self.prepared.fit_sample().n_active()
    }

    fn ncols(&self) -> usize {
        self.generated.len()
    }

    fn column_slice(&self, column: usize) -> Option<&[f64]> {
        let generated = *self.generated.get(column)?;
        let super::PreparedOlsColumnKind::Continuous {
            prepared_source_index,
            ..
        } = self.prepared.fit_model().columns.get(generated)?.kind
        else {
            return None;
        };
        self.prepared
            .fit_sample()
            .regressors
            .get(prepared_source_index)
            .filter(|values| values.len() == self.nrows())
            .map(Vec::as_slice)
    }

    fn visit_column_range(
        &self,
        column: usize,
        range: std::ops::Range<usize>,
        visit: impl FnMut(usize, f64) -> Result<()>,
    ) -> Result<()> {
        let generated = *self
            .generated
            .get(column)
            .ok_or_else(|| invalid_argument("joint original column is out of range"))?;
        crate::prepared_fit_view::visit_prepared_column_range(
            self.prepared,
            generated,
            range,
            visit,
        )
    }

    fn visit_rows(
        &self,
        row_scratch: &mut [f64],
        mut visit: impl FnMut(usize, &[f64]) -> Result<()>,
    ) -> Result<()> {
        if row_scratch.len() < self.generated.len() {
            return Err(invalid_argument(
                "joint original row exceeds admitted width",
            ));
        }
        let values = &mut row_scratch[..self.generated.len()];
        for row in 0..self.prepared.fit_sample().n_active() {
            for (value, &generated) in values.iter_mut().zip(self.generated) {
                *value = match &self.numeric {
                    Some(numeric) => numeric.raw_columns()[generated].map_or(1., |v| v[row]),
                    None => self.prepared.fit_column_value(generated, row)?,
                };
            }
            visit(row, values)?;
        }
        Ok(())
    }
}

struct RetainedWithinReader<'a> {
    storage: &'a RetainedWithinStorage,
    scratch: Option<PreparedOlsScratchReader<'a>>,
}

impl RetainedWithinReader<'_> {
    fn fill_row(&mut self, row: usize, output: &mut [f64]) -> Result<()> {
        if let Some(reader) = self.scratch.as_mut() {
            if row >= self.storage.rows() || output.len() != self.storage.columns() {
                return Err(invalid_argument(
                    "weighted-linear within-design row dimensions are inconsistent",
                ));
            }
            reader.read_linear(row * self.storage.columns(), output)
        } else {
            self.storage.fill_row(row, output)
        }
    }
}

impl RetainedWithinStorage {
    fn reader(&self) -> RetainedWithinReader<'_> {
        RetainedWithinReader {
            storage: self,
            scratch: match self {
                Self::Scratch(values) => Some(values.shared_reader()),
                Self::Dense(_) | Self::Implicit(_) => None,
            },
        }
    }

    fn rows(&self) -> usize {
        match self {
            Self::Dense(values) => values.nrows(),
            Self::Scratch(values) => values.rows(),
            Self::Implicit(values) => values.rows(),
        }
    }

    fn moment_source<'a>(
        &'a self,
        selected: &'a [usize],
        response: &'a [f64],
        weights: BorrowedMapWeights<'a>,
        scale: super::PreparedOlsBreadWeightScale,
        norms: &'a [f64],
    ) -> Result<FixedBlockMomentSource<'a>> {
        let selected_columns = Some(selected);
        let column_scales = Some(norms);
        let solver_weight_divisor = scale.solver_weight_divisor;
        Ok(match self {
            Self::Implicit(values) => FixedBlockMomentSource::Implicit {
                values: values.view()?,
                selected_columns,
                response: (response).into(),
                weights,
                solver_weight_divisor,
                column_scales,
            },
            Self::Dense(values) => FixedBlockMomentSource::Dense {
                values: super::finite_wls_kernel::FiniteDenseRows::packed(
                    values.as_slice(),
                    self.columns(),
                ),
                rows: self.rows(),
                stored_columns: self.columns(),
                selected_columns,
                response: (response).into(),
                weights,
                solver_weight_divisor,
                column_scales,
            },
            Self::Scratch(values) => FixedBlockMomentSource::Scratch {
                values,
                selected_columns,
                response: response.into(),
                weights,
                solver_weight_divisor,
                column_scales,
            },
        })
    }

    fn columns(&self) -> usize {
        match self {
            Self::Dense(values) => values.ncols(),
            Self::Scratch(values) => values.columns(),
            Self::Implicit(values) => values.columns(),
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
            Self::Implicit(values) => values.view()?.fill_row(row, output),
            Self::Scratch(values) => {
                let start = row
                    .checked_mul(columns)
                    .ok_or_else(|| allocation("weighted-linear scratch row offset overflows"))?;
                values.read_linear_shared(start, output)
            }
        }
    }

    fn memory_report(&self) -> LinearWithinMemoryReport {
        let (resident_bytes, scratch_bytes, mode) = match self {
            // Compaction preserves the original strided allocation; report its
            // capacity, rather than logical length after rank reduction.
            Self::Dense(values) => (
                values.storage_capacity() as u128 * 8,
                0,
                PreparedOlsExecutionMode::Fast,
            ),
            Self::Implicit(values) => (values.resident_bytes(), 0, values.mode()),
            Self::Scratch(values) => (0, values.bytes(), PreparedOlsExecutionMode::Low),
        };
        LinearWithinMemoryReport {
            resident_bytes,
            scratch_bytes,
            rows: self.rows(),
            columns: self.columns(),
            mode,
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
            RetainedWithinStorage::Implicit(mut values) => {
                values.compact(&active_positions)?;
                RetainedWithinStorage::Implicit(values)
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
    fn prefers_sequential_scan(&self) -> bool {
        matches!(self.storage, RetainedWithinStorage::Scratch(_))
    }

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
            RetainedWithinStorage::Implicit(values) => {
                values
                    .view()?
                    .fill_selected_row(row, self.active_positions, output)
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

    fn visit_range(
        &self,
        rows: std::ops::Range<usize>,
        output: &mut [f64],
        mut visit: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<()> {
        match self.storage {
            RetainedWithinStorage::Dense(_) => {
                for row in rows {
                    self.fill_row(row, output)?;
                    visit(row, output)?;
                }
            }
            RetainedWithinStorage::Implicit(values) => {
                let view = values.view()?;
                if rows.start > rows.end
                    || rows.end > view.rows()
                    || output.len() != self.active_positions.len()
                {
                    return Err(invalid_argument("implicit inference range is invalid"));
                }
                let bound = view.bind_columns()?;
                for row in rows {
                    let group = view.codes()[row] as usize;
                    for (target, &column) in output.iter_mut().zip(self.active_positions) {
                        *target = bound[column].value(row)? - view.center(column, group);
                    }
                    visit(row, output)?;
                }
            }
            RetainedWithinStorage::Scratch(values) => {
                if rows.start > rows.end
                    || rows.end > values.rows()
                    || output.len() != self.active_positions.len()
                    || values.columns() != output.len()
                    || self
                        .active_positions
                        .iter()
                        .copied()
                        .enumerate()
                        .any(|(expected, position)| expected != position)
                {
                    return Err(invalid_argument(
                        "weighted-linear scratch inference range was not compacted or is invalid",
                    ));
                }
                let mut reader = values.shared_reader();
                for row in rows {
                    reader.read_linear(row * values.columns(), output)?;
                    visit(row, output)?;
                }
            }
        }
        Ok(())
    }
}

/// Borrowed certified products exposed to an iterative estimator and its
/// final inference stage.  Nothing here is exported from `fereg-core`.
pub(crate) struct CertifiedLinearSessionOutput<'a> {
    frame: &'a CertifiedLinearFrame,
    basis: &'a FrozenLinearBasis,
    response: &'a [f64],
    within: &'a RetainedWithinDesign,
    reconstruction: LinearReconstructionCertificate,
    reuse: LinearSessionReuseReport,
}

impl<'a> CertifiedLinearSessionOutput<'a> {
    pub(crate) const fn frame(&self) -> &'a CertifiedLinearFrame {
        self.frame
    }
    pub(crate) const fn basis(&self) -> &'a FrozenLinearBasis {
        self.basis
    }
    pub(crate) fn coefficients(
        &self,
    ) -> impl ExactSizeIterator<Item = super::PreparedOlsFittedCoefficient> + 'a {
        self.basis
            .slots
            .iter()
            .zip(&self.frame.reported_coefficients)
            .map(|(slot, &value)| super::PreparedOlsFittedCoefficient {
                coefficient_slot: slot.coefficient_slot,
                generated_column_index: slot.generated_column_index,
                status: slot.status,
                value,
            })
    }

    pub(crate) const fn response(&self) -> &'a [f64] {
        self.response
    }

    pub(crate) fn finite_predictor(&self) -> &'a [f64] {
        &self.frame.predictors().finite
    }

    pub(crate) fn absorbed_predictor(&self) -> &'a [f64] {
        &self.frame.predictors().absorbed
    }

    pub(crate) const fn reuse_report(&self) -> LinearSessionReuseReport {
        self.reuse
    }

    pub(crate) fn retained_within_memory(&self) -> LinearWithinMemoryReport {
        self.within.storage.memory_report()
    }

    pub(crate) fn certified(&self) -> bool {
        self.frame.canonical_proof().is_some()
            && self.frame.certified()
            && self.within.certificate.passed
            && self.reconstruction.passed
    }
}

/// Borrowed products from the no-N-by-K-rebuild iteration path. Coefficients
/// and bread follow `basis().kept_generated_columns()` order.
#[cfg(test)]
pub(crate) struct CertifiedFrozenLinearOutput<'a> {
    step: &'a CertifiedLinearFrame,
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
            && self
                .step
                .frozen_certificate()
                .is_some_and(|proof| proof.passed)
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
    step: &'a CertifiedLinearFrame,
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

    pub(crate) const fn basis(&self) -> &'a FrozenLinearBasis {
        self.basis
    }

    pub(crate) const fn response(&self) -> &'a [f64] {
        self.response
    }

    pub(crate) const fn reuse_report(&self) -> LinearSessionReuseReport {
        self.reuse
    }

    pub(crate) fn certified(&self) -> bool {
        self.step.response_absorption_report.certified()
            && self
                .step
                .frozen_certificate()
                .is_some_and(|proof| proof.passed)
            && self.within.certificate.passed
            && self.reconstruction.passed
    }
}

/// Ordinary PPML may certify a joint direction without constructing a bread.
/// Keeping this output distinct prevents final inference from consuming it.
pub(crate) struct CertifiedJointPpmlLinearOutput<'a> {
    joint: &'a crate::fe::slope_coefficients::joint::JointFiniteWorkspace,
    basis: &'a FrozenLinearBasis,
    response: &'a [f64],
    reported: &'a [Option<f64>],
    reconstruction: LinearReconstructionCertificate,
    reuse: LinearSessionReuseReport,
}

pub(crate) enum PpmlOrdinaryLinearOutput<'a> {
    Frozen(CertifiedFrozenPpmlLinearOutput<'a>),
    Joint(CertifiedJointPpmlLinearOutput<'a>),
}

impl<'a> PpmlOrdinaryLinearOutput<'a> {
    pub(crate) fn certified(&self) -> bool {
        match self {
            Self::Frozen(output) => output.certified(),
            Self::Joint(output) => {
                output
                    .joint
                    .report
                    .as_ref()
                    .is_some_and(SolveReport::certified)
                    && output.reconstruction.passed
            }
        }
    }
    pub(crate) fn coefficients(&self) -> &'a [f64] {
        match self {
            Self::Frozen(o) => o.coefficients(),
            Self::Joint(o) => o.joint.beta(),
        }
    }
    pub(crate) fn reported_coefficients(&self) -> &'a [Option<f64>] {
        match self {
            Self::Frozen(o) => o.reported_coefficients(),
            Self::Joint(o) => o.reported,
        }
    }
    pub(crate) fn within_residuals(&self) -> &'a [f64] {
        match self {
            Self::Frozen(o) => o.within_residuals(),
            Self::Joint(o) => o.joint.residuals.as_slice(),
        }
    }
    pub(crate) fn response_absorption_report(&self) -> &'a SolveReport {
        match self {
            Self::Frozen(o) => o.response_absorption_report(),
            Self::Joint(o) => o.joint.report.as_ref().expect("certified joint result"),
        }
    }
    pub(crate) fn basis(&self) -> &'a FrozenLinearBasis {
        match self {
            Self::Frozen(o) => o.basis(),
            Self::Joint(o) => o.basis,
        }
    }
    pub(crate) fn response(&self) -> &'a [f64] {
        match self {
            Self::Frozen(o) => o.response(),
            Self::Joint(o) => o.response,
        }
    }
    pub(crate) fn reuse_report(&self) -> LinearSessionReuseReport {
        match self {
            Self::Frozen(o) => o.reuse_report(),
            Self::Joint(o) => o.reuse,
        }
    }
}
type FrozenLinearWorkspace = super::finite_wls_kernel::FiniteWorkspace<FrozenLinearBasisSlot>;

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
    }
}

fn merge_canonical_projection_reports(
    target: &mut SolveReport,
    mut source: SolveReport,
) -> Result<()> {
    let no_work = |r: &SolveReport| {
        r.requested_solver == FeSolverKind::Map
            && r.used_solver == FeSolverKind::Map
            && r.iterations == 0
            && !r.fallback_used
    };
    let checkpoint = |r: &SolveReport| {
        r.requested_solver == FeSolverKind::Lsmr
            && matches!(r.used_solver, FeSolverKind::Lsmr | FeSolverKind::Map)
    };
    if target.certified()
        && source.certified()
        && (checkpoint(target) || checkpoint(&source))
        && (checkpoint(target) || no_work(target))
        && (checkpoint(&source) || no_work(&source))
    {
        // A zero-iteration input certificate is neutral to the aggregate
        // engine. Any actual coefficient-MAP solve or fallback governs the row.
        // A successful Schur accelerator is not a failed LSMR attempt. Preserve
        // a uniform preconditioner; a heterogeneous aggregate has no single one.
        let used = if (target.used_solver == FeSolverKind::Map && !no_work(target))
            || (source.used_solver == FeSolverKind::Map && !no_work(&source))
        {
            FeSolverKind::Map
        } else {
            FeSolverKind::Lsmr
        };
        let preconditioners = if no_work(target) {
            (source.requested_preconditioner, source.used_preconditioner)
        } else if no_work(&source)
            || (target.requested_preconditioner == source.requested_preconditioner
                && target.used_preconditioner == source.used_preconditioner)
        {
            (target.requested_preconditioner, target.used_preconditioner)
        } else {
            (None, None)
        };
        for report in [&mut *target, &mut source] {
            report.requested_solver = FeSolverKind::Lsmr;
            report.used_solver = used;
            report.requested_preconditioner = preconditioners.0;
            report.used_preconditioner = preconditioners.1;
        }
    }
    merge_solve_report(target, source)
}

/// Owned handoff after a final strict solve. This keeps dense or scratch
/// within-design row access alive after the iterative session is consumed.
pub(crate) struct CertifiedLinearSessionProducts {
    frame: CertifiedLinearFrame,
    constant: Option<LinearConstantRepresentation>,
    basis: FrozenLinearBasis,
    weights: OwnedLinearWeights,
    within: RetainedWithinDesign,
    reconstruction: LinearReconstructionCertificate,
}

impl CertifiedLinearSessionProducts {
    pub(crate) fn constant_representation(&self) -> Option<&LinearConstantRepresentation> {
        self.constant.as_ref()
    }

    pub(crate) const fn frame(&self) -> &CertifiedLinearFrame {
        &self.frame
    }

    /// Compact inverse cross product aligned exactly to
    /// `basis().kept_generated_columns()`. A normalized absorbed intercept is
    /// a reported coordinate and is deliberately excluded.
    pub(crate) fn identified_bread(&self) -> &[f64] {
        self.frame.bread()
    }

    pub(crate) const fn generations(&self) -> LinearSessionGenerations {
        self.frame.generations
    }

    pub(crate) const fn basis(&self) -> &FrozenLinearBasis {
        &self.basis
    }

    pub(crate) fn weights(&self) -> BorrowedMapWeights<'_> {
        self.weights.borrowed()
    }

    pub(crate) fn finite_predictor(&self) -> &[f64] {
        &self.frame.predictors().finite
    }

    pub(crate) fn absorbed_predictor(&self) -> &[f64] {
        &self.frame.predictors().absorbed
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
    group_centered_source: Option<SharedPreparedOlsData>,
    centered_projection: Option<ImplicitCenteredDesign>,
    fast_canonical_products_allowance: u128,
    config: PreparedOlsFitConfig,
    response: Vec<f64>,
    weights: OwnedLinearWeights,
    bread_weight_scale: super::PreparedOlsBreadWeightScale,
    generations: LinearSessionGenerations,
    prepared_alignment_valid: bool,
    frozen_basis: Option<FrozenLinearBasis>,
    within: Option<RetainedWithinDesign>,
    frame: Option<CertifiedLinearFrame>,
    predictors: LinearPredictors,
    constant: Option<LinearConstantRepresentation>,
    moment_workspace: Vec<f64>,
    row_workspace: Vec<f64>,
    frozen_workspace: FrozenLinearWorkspace,
    map_weights: Vec<f64>,
    previous_response: Vec<f64>,
    projection_block: Vec<f64>,
    fe_design: Option<FeDesign>,
    fe_projector: Option<FeSession<'a>>,
    reconstruction: Option<LinearReconstructionCertificate>,
    reuse: Option<LinearSessionReuseReport>,
    pending_response_buffer_reused: bool,
    pending_weight_buffer_reused: bool,
    projection_refresh_required: bool,
    canonical_projection_work_hint: u128,
    phase_allocator_probe: Option<LinearSessionPhaseAllocatorProbe>,
    #[cfg(test)]
    fresh_rank_failure_injection: u8,
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
        || message.starts_with("frozen weighted-linear joint slope failed;")
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
        Self::new_with_fe_design_mode(prepared, frame, config, generations, fe_design)
    }

    fn new_with_fe_design_mode(
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
            PreparedOlsExecutionMode::Fast => prepared
                .fit_model()
                .columns
                .len()
                .checked_add(1)
                .ok_or_else(|| allocation("weighted-linear projection width overflows"))?,
            PreparedOlsExecutionMode::Low => config
                .execution
                .poolsize
                .max(1)
                .min(prepared.fit_model().columns.len().max(1)),
        };
        if fe_design.nobs() != response.len() {
            return Err(invalid_argument(
                "weighted-linear owned FE design is not aligned on the prepared sample",
            ));
        }
        let (fe_design, fe_projector, map_weights) = if config.absorb.solver == FeSolverKind::Map {
            let categorical = prepared.fit_borrowed_intercept_map_design()?;
            if let Some(categorical) = categorical.filter(|design| {
                matches!(design.effects().len(), 1..=3) && design.nobs() == response.len()
            }) {
                (
                    None,
                    Some(FeSession::categorical(
                        ReusableCategoricalMapAbsorber::new(
                            categorical,
                            maximum_fe_rhs,
                            config.execution.threads.resolved(),
                        )?,
                        ReusableMapAbsorber::new(fe_design, maximum_fe_rhs)?,
                    )),
                    Vec::new(),
                )
            } else {
                (
                    None,
                    Some(FeSession::general(ReusableMapAbsorber::new(
                        fe_design,
                        maximum_fe_rhs,
                    )?)),
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
        let previous_response = if fe_projector
            .as_ref()
            .is_some_and(|p| p.categorical.is_some())
        {
            fallible_copy(&response, "unable to reserve linear response history")?
        } else {
            Vec::new()
        };
        Ok(Self {
            prepared,
            group_centered_source: None,
            centered_projection: None,
            fast_canonical_products_allowance: 0,
            config,
            response,
            weights,
            bread_weight_scale: frame.bread_weight_scale(),
            generations,
            prepared_alignment_valid: true,
            frozen_basis: None,
            within: None,
            frame: None,
            constant: None,
            predictors: LinearPredictors::default(),
            moment_workspace: Vec::new(),
            row_workspace: Vec::new(),
            frozen_workspace: FrozenLinearWorkspace::default(),
            previous_response,
            map_weights,
            projection_block: Vec::new(),
            fe_design,
            fe_projector,
            reconstruction: None,
            reuse: None,
            pending_response_buffer_reused: false,
            pending_weight_buffer_reused: false,
            projection_refresh_required: false,
            canonical_projection_work_hint: 0,
            phase_allocator_probe: None,
            #[cfg(test)]
            fresh_rank_failure_injection: 0,
        })
    }

    /// The caller admits all extra group/binding/worker storage before sharing
    /// the prepared owner. Object identity proves the immutable sample/model axis.
    pub(crate) fn enable_group_centered_one_fe(
        &mut self,
        source: SharedPreparedOlsData,
        allowance: u128,
    ) -> Result<()> {
        if self.within.is_some()
            || self.config.absorb.solver != FeSolverKind::Map
            || !self.prepared.fit_has_one_ordinary_intercept_effect()
            || !std::ptr::eq(self.prepared.fit_sample(), source.as_ref().sample())
            || !std::ptr::eq(self.prepared.fit_model(), source.as_ref().model())
        {
            return Err(invalid_argument(
                "implicit prepared owner does not match the linear session",
            ));
        }
        crate::memory::require_admitted_capacity(
            super::ImplicitCenteredDesign::bytes(
                self.fe_design().effects()[0].n_levels(),
                self.prepared.fit_model().columns.len(),
                self.config.execution.threads.resolved(),
            )?,
            allowance,
        )?;
        self.group_centered_source = Some(source);
        Ok(())
    }

    pub(crate) fn has_implicit_one_fe(&self) -> bool {
        self.group_centered_source.is_some()
            && self.config.execution.mode == PreparedOlsExecutionMode::Low
    }

    /// PPML admits this complete optional bank beside the existing fast plan;
    /// the direct builder may still decline work or current-weight geometry.
    pub(crate) fn enable_fast_canonical_products(&mut self, allowance: u128) -> Result<()> {
        if self.within.is_some()
            || self.config.execution.mode != PreparedOlsExecutionMode::Fast
            || self.config.absorb.solver != FeSolverKind::Map
            || !matches!(self.fe_design().effects().len(), 2 | 3)
            || !self
                .fe_design()
                .effects()
                .iter()
                .all(|e| e.has_intercept() && e.slopes().is_empty())
            || !matches!(
                self.fe_projector,
                Some(FeSession {
                    categorical: Some(_),
                    coefficients: false,
                    ..
                })
            )
        {
            return Err(invalid_argument(
                "fast canonical product admission does not match the session",
            ));
        }
        self.fast_canonical_products_allowance = allowance;
        Ok(())
    }

    pub(crate) fn fe_design(&self) -> &FeDesign {
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
        self.recycle_frame();
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
            .is_some_and(FeSession::is_ppml_categorical)
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
            | PreparedOlsFastScoreCache::Selected { within, .. } => within.as_slice().as_ptr(),
        };
        let storage = self.dense_from_fast_cache(cache, kept)?;
        let pointer_preserved = match &storage {
            RetainedWithinStorage::Dense(within) => within.as_slice().as_ptr() == source_pointer,
            RetainedWithinStorage::Scratch(_) | RetainedWithinStorage::Implicit(_) => false,
        };
        let retained_values = match &storage {
            RetainedWithinStorage::Dense(within) => within.as_slice().to_vec(),
            RetainedWithinStorage::Scratch(_) | RetainedWithinStorage::Implicit(_) => Vec::new(),
        };
        Ok((storage.memory_report(), pointer_preserved, retained_values))
    }

    #[cfg(test)]
    pub(crate) fn retained_scratch_identity(&self) -> Option<&std::path::Path> {
        match &self.within.as_ref()?.storage {
            RetainedWithinStorage::Dense(_) | RetainedWithinStorage::Implicit(_) => None,
            RetainedWithinStorage::Scratch(scratch) => Some(scratch.identity_path()),
        }
    }

    #[cfg(test)]
    pub(crate) fn retained_dense_identity(&self) -> Option<*const f64> {
        match &self.within.as_ref()?.storage {
            RetainedWithinStorage::Dense(values) => Some(values.as_slice().as_ptr()),
            RetainedWithinStorage::Scratch(_) | RetainedWithinStorage::Implicit(_) => None,
        }
    }

    pub(crate) fn workspace_capacities(&self) -> LinearSessionWorkspaceCapacities {
        let current_step = self.frame.as_ref();
        let mut fe = self
            .fe_projector
            .as_ref()
            .map(FeSession::capacities)
            .unwrap_or_default();
        fe.accelerator_state += self.previous_response.capacity();
        let (within_resident_elements, within_scratch_bytes, within_io_buffer_elements) = self
            .within
            .as_ref()
            .map_or((0, 0, 0), |within| match &within.storage {
                RetainedWithinStorage::Dense(values) => (values.storage_capacity(), 0, 0),
                RetainedWithinStorage::Implicit(values) => {
                    ((values.resident_bytes() / 8) as usize, 0, 0)
                }
                RetainedWithinStorage::Scratch(scratch) => {
                    (0, scratch.bytes(), scratch.io_buffer_capacity())
                }
            });
        LinearSessionWorkspaceCapacities {
            fe,
            constant_recipe_elements: self
                .constant
                .as_ref()
                .map_or(0, |constant| constant.capacity_elements()),
            within_metadata: self.within.as_ref().map_or([0; 5], |within| {
                [
                    within.stored_generated_columns.capacity(),
                    within.active_positions.capacity(),
                    within.reference_norms.capacity(),
                    within.reference_means.capacity(),
                    within.within_norms.capacity(),
                ]
            }),
            basis_metadata: self.frozen_basis.as_ref().map_or([0; 3], |basis| {
                [
                    basis.slots.capacity(),
                    basis.kept_generated_columns.capacity(),
                    basis.dropped_generated_columns.capacity(),
                ]
            }),
            response_elements: self.response.capacity(),
            weight_elements: self.weights.capacity(),
            finite_predictor_elements: self.predictors.finite.capacity()
                + current_step
                    .and_then(|step| step.decomposition.as_ref())
                    .map_or(0, |p| p.finite.capacity()),
            absorbed_predictor_elements: self.predictors.absorbed.capacity()
                + current_step
                    .and_then(|step| step.decomposition.as_ref())
                    .map_or(0, |p| p.absorbed.capacity()),
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
            frozen_coefficient_elements: self.frozen_workspace.coefficients.capacity()
                + current_step.map_or(0, |step| step.coefficients.capacity()),
            frozen_bread_elements: self.frozen_workspace.bread.capacity()
                + current_step.map_or(0, |step| step.bread.capacity()),
            frozen_residual_elements: self.frozen_workspace.residuals.capacity()
                + current_step.map_or(0, |step| step.within_residuals.capacity()),
            frozen_reported_coefficient_elements: self
                .frozen_workspace
                .reported_coefficients
                .capacity()
                + current_step.map_or(0, |step| step.reported_coefficients.capacity()),
            frozen_kept_elements: self.frozen_workspace.kept.capacity(),
            frozen_candidate_position_elements: self
                .frozen_workspace
                .candidate_positions
                .capacity(),
            frozen_candidate_identity_elements: self.frozen_workspace.candidate_ids.capacity(),
            frozen_candidate_scale_elements: self.frozen_workspace.candidate_scales.capacity(),
            frozen_basis_slot_elements: self.frozen_workspace.basis_slots.capacity(),
            map_weight_elements: self.map_weights.capacity(),
            projection_block_elements: self.projection_block.capacity(),
            within_resident_elements: within_resident_elements
                + self
                    .centered_projection
                    .as_ref()
                    .map_or(0, |p| (p.resident_bytes() / 8) as usize),
            within_scratch_bytes,
            within_io_buffer_elements,
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

    pub(crate) fn enable_ppml_parallel_projection(
        &mut self,
        plan: crate::fe::ParallelProjectionPlan,
    ) -> Result<()> {
        match self.fe_projector.as_mut() {
            Some(FeSession {
                general: Some(projector),
                coefficients: false,
                ..
            }) => projector.enable_parallel_projection(plan),
            _ => Err(invalid_argument(
                "parallel PPML projection requires the generic reusable MAP owner",
            )),
        }
    }

    pub(crate) fn enable_ppml_certificate_geometry(&mut self, allowance: u128) -> Result<()> {
        crate::memory::require_admitted_capacity(
            ReusableMapAbsorber::certificate_geometry_bytes(self.fe_design())?,
            allowance,
        )?;
        match self.fe_projector.as_mut() {
            Some(FeSession {
                general: Some(projector),
                coefficients: false,
                ..
            }) => match projector.enable_certificate_geometry() {
                Ok(()) => Ok(()),
                Err(error) if error.code == ErrorCode::Allocation => Ok(()),
                Err(error) => Err(error),
            },
            _ => Err(invalid_argument(
                "PPML certificate geometry requires the generic reusable MAP owner",
            )),
        }
    }

    pub(crate) fn enable_ppml_coefficient_solver(
        &mut self,
        threads: usize,
        allowance: u128,
        parallel_certificate: Option<u128>,
        prepared_coupling: Option<u128>,
    ) -> Result<()> {
        let rhs = self.fe_projector.as_ref().expect("MAP owner").max_rhs();
        let required = ppml_coefficient_lifetime_bytes(self.fe_design(), rhs, threads)?;
        crate::memory::require_admitted_capacity(required, allowance)?;
        // History must be available before enabling a numerical operation.
        // The canonical within projector remains installed independently.
        if self.previous_response.is_empty() {
            self.previous_response = fallible_copy(
                &self.response,
                "unable to reserve coefficient response history",
            )?;
        }
        if self.map_weights.is_empty() {
            self.map_weights = fallible_filled(
                self.response.len(),
                0.0,
                "unable to reserve coefficient frame weights",
            )?;
        }
        let Some(FeSession {
            general: Some(projector),
            coefficients: false,
            ..
        }) = self.fe_projector.as_mut()
        else {
            return Err(invalid_argument(
                "coefficient coordinates require the generic FE owner",
            ));
        };
        if !projector.has_certificate_geometry() {
            return Ok(());
        }
        match projector.enable_coefficient_solver(threads) {
            Ok(()) => {}
            Err(error) if error.code == ErrorCode::Allocation => return Ok(()),
            Err(error) => return Err(error),
        }
        if let Some(allowance) = parallel_certificate {
            crate::memory::require_admitted_capacity(
                ReusableMapAbsorber::certificate_workspace_bytes(projector.design(), rhs)?,
                allowance,
            )?;
            match projector.enable_certificate_workspace() {
                Ok(()) => {}
                Err(error) if error.code == ErrorCode::Allocation => {}
                Err(error) => return Err(error),
            }
        }
        if let Some(allowance) = prepared_coupling {
            crate::memory::require_admitted_capacity(
                ReusableMapAbsorber::coefficient_coupling_bytes(projector.design(), rhs)?
                    .unwrap_or(0),
                allowance,
            )?;
            match projector.enable_coefficient_coupling() {
                Ok(()) => {}
                Err(error) if error.code == ErrorCode::Allocation => {}
                Err(error) => return Err(error),
            }
        }
        let session = self.fe_projector.as_mut().expect("FE binding");
        session.coefficients = true;
        Ok(())
    }

    pub(crate) fn enable_ppml_joint_slopes(&mut self, allowance: u128) -> Result<()> {
        crate::memory::require_admitted_capacity(
            ReusableMapAbsorber::joint_slope_bytes(
                self.fe_design(),
                self.prepared.fit_model().columns.len(),
            )?,
            allowance,
        )?;
        if let Some(FeSession {
            general: Some(projector),
            coefficients: true,
            ..
        }) = self.fe_projector.as_mut()
        {
            match projector.enable_joint_slopes_for_columns(self.prepared.fit_model().columns.len())
            {
                Ok(()) => {}
                Err(error) if error.code == ErrorCode::Allocation => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    pub(crate) fn enable_checkpoint_lsmr(&mut self, deterministic: bool) {
        if let Some(projector) = self.fe_projector.as_mut() {
            projector.enable_checkpoint_lsmr(deterministic);
        }
    }

    pub(crate) fn enable_component_checkpoint_scaling(
        &mut self,
        allowance: u128,
        threads: usize,
    ) -> Result<()> {
        let rhs = self.fe_projector.as_ref().expect("MAP owner").max_rhs();
        let required = ppml_component_checkpoint_bytes(self.fe_design(), rhs, threads)?;
        crate::memory::require_admitted_capacity(required, allowance)?;
        if let Some(FeSession {
            coefficients: true,
            categorical: Some(p),
            ..
        }) = self.fe_projector.as_mut()
        {
            p.enable_component_checkpoint_scaling();
        }
        Ok(())
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

    pub(crate) fn supports_joint_ppml_direction(&self) -> bool {
        self.frozen_basis
            .as_ref()
            .is_some_and(|b| !b.kept_generated_columns().is_empty())
            && matches!(self.fe_projector.as_ref(), Some(FeSession { general: Some(projector), coefficients: true, .. }) if projector.joint_slopes().is_some())
    }

    pub(crate) fn solve_ordinary_ppml(&mut self) -> Result<PpmlOrdinaryLinearOutput<'_>> {
        let eligible = self.supports_joint_ppml_direction();
        if !eligible {
            return self
                .solve_frozen_ppml()
                .map(PpmlOrdinaryLinearOutput::Frozen);
        }
        self.check_frozen_failure_injection()?;
        let team = ThreadTeam::new(self.config.execution.threads);
        let result = team.install(|| self.solve_joint_ppml_internal())?;
        if let Err(error) = result {
            if let Some(FeSession {
                general: Some(projector),
                coefficients: true,
                ..
            }) = self.fe_projector.as_mut()
            {
                projector.invalidate_joint_slopes();
            }
            return Err(if error.code == ErrorCode::NonConvergence {
                FeregError::new(
                    error.code,
                    format!(
                        "frozen weighted-linear joint slope failed; {}",
                        error.message
                    ),
                )
            } else {
                error
            });
        }
        let Some(FeSession {
            general: Some(projector),
            coefficients: true,
            ..
        }) = self.fe_projector.as_ref()
        else {
            unreachable!()
        };
        Ok(PpmlOrdinaryLinearOutput::Joint(
            CertifiedJointPpmlLinearOutput {
                joint: projector.joint_slopes().expect("admitted joint owner"),
                basis: self.frozen_basis.as_ref().expect("frozen joint basis"),
                response: &self.response,
                reported: &self.frozen_workspace.reported_coefficients,
                reconstruction: self.reconstruction.expect("joint reconstruction"),
                reuse: self.reuse.expect("joint reuse accounting"),
            },
        ))
    }

    fn solve_joint_ppml_internal(&mut self) -> Result<()> {
        self.ensure_bound()?;
        self.recycle_frame();
        self.fill_reusable_map_weights()?;
        let basis = self
            .frozen_basis
            .as_ref()
            .ok_or_else(|| invalid_argument("joint slope basis is absent"))?;
        let within = self
            .within
            .as_ref()
            .ok_or_else(|| invalid_argument("joint slope retained controls are absent"))?;
        if within.generations.sample != self.generations.sample
            || within.generations.topology != self.generations.topology
            || within.active_positions.len() != basis.kept_generated_columns().len()
            || within
                .active_positions
                .iter()
                .zip(basis.kept_generated_columns())
                .any(|(&position, &generated)| {
                    within.stored_generated_columns.get(position) != Some(&generated)
                })
        {
            return Err(invalid_argument(
                "joint slope retained basis identity is invalid",
            ));
        }
        let Some(FeSession {
            coefficients: true,
            general: Some(projector),
            ..
        }) = self.fe_projector.as_mut()
        else {
            unreachable!()
        };
        let previous_response = &mut self.previous_response;
        let n = self.response.len();
        if previous_response.len() != n || self.frozen_workspace.response.len() != n {
            return Err(invalid_argument(
                "joint slope response representatives are misaligned",
            ));
        }
        // A representative differs from the current response only by FE terms.
        // Advance its anchor even on a later solver failure, so canonical fallback
        // cannot apply this same response difference for a second time.
        // Keep the legacy matrix indices and numerical traversal order explicit.
        #[allow(clippy::needless_range_loop)]
        for row in 0..n {
            self.frozen_workspace.response[row] += self.response[row] - previous_response[row];
        }
        previous_response.copy_from_slice(&self.response);
        let started = Instant::now();
        match &within.storage {
            RetainedWithinStorage::Dense(controls) => projector.solve_joint_slopes(
                &self.map_weights,
                controls,
                &within.active_positions,
                &self.frozen_workspace.response,
                self.config.absorb.map,
                self.generations.map_geometry(),
            )?,
            RetainedWithinStorage::Scratch(_) | RetainedWithinStorage::Implicit(_) => projector
                .solve_joint_slopes_streamed(
                    &self.map_weights,
                    &JointOriginalColumns {
                        prepared: self.prepared,
                        generated: &within.stored_generated_columns,
                        numeric: super::prepared_rhs::NumericOlsColumns::bind_score_means(
                            self.prepared.fit_model(),
                            self.prepared.fit_sample(),
                        )?,
                    },
                    &within.active_positions,
                    &self.frozen_workspace.response,
                    self.config.absorb.map,
                    self.generations.map_geometry(),
                )?,
        }
        let solving_seconds = started.elapsed().as_secs_f64();
        let checking = Instant::now();
        let joint = projector.joint_slopes().expect("joint direction");
        let residuals = joint.residuals.as_slice();
        // Only the streamed route supplied untouched original columns. These
        // current-call norms precede the recurrence and are never modified by it;
        // the score numerators below still rescan the original inputs freshly.
        let input_norms = match &within.storage {
            RetainedWithinStorage::Dense(_) => None,
            RetainedWithinStorage::Scratch(_) | RetainedWithinStorage::Implicit(_) => {
                Some(joint.input_norms().ok_or_else(|| {
                    invalid_argument("joint input norms lack a successful current-call certificate")
                })?)
            }
        };
        let rank = basis.kept_generated_columns().len();
        if input_norms.is_some_and(|norms| norms.len() != rank) {
            return Err(invalid_argument("joint input norm columns are misaligned"));
        }
        resize_zeroed(
            &mut self.frozen_workspace.scores,
            rank,
            "unable to reserve joint score checks",
        )?;
        resize_zeroed(
            &mut self.frozen_workspace.rhs,
            rank,
            "unable to reserve joint current means",
        )?;
        // The finite-solve scratch is dead during joint acceptance; it holds
        // per-column norms until the independent response scale is available.
        resize_zeroed(
            &mut self.frozen_workspace.solve,
            rank,
            "unable to reserve joint current norms",
        )?;
        let originals = JointOriginalColumns {
            prepared: self.prepared,
            generated: basis.kept_generated_columns(),
            numeric: None,
        };
        use crate::fe::slope_coefficients::joint::JointFiniteDesign;
        let weights = &self.map_weights;
        let response = &self.response;
        let needs_mean = basis
            .slots()
            .iter()
            .any(|slot| slot.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept);
        let (response_checks, column_checks) = rayon::join(
            || check_joint_response(response, residuals, weights, needs_mean),
            || {
                self.frozen_workspace
                    .scores
                    .par_iter_mut()
                    .zip(self.frozen_workspace.rhs.par_iter_mut())
                    .zip(self.frozen_workspace.solve.par_iter_mut())
                    .enumerate()
                    .try_for_each(|(column, ((score, mean), norm))| -> Result<()> {
                        let mut scale = 0.;
                        let mut square = 1.;
                        let mut sum = 0.;
                        let mut correction = 0.;
                        let mut mean_sum = 0.;
                        let mut mean_correction = 0.;
                        originals.visit_column(column, |row, value| {
                            if input_norms.is_none() {
                                update_scaled_square(
                                    weights[row].sqrt() * value,
                                    &mut scale,
                                    &mut square,
                                )?;
                            }
                            compensated_add(
                                weights[row] * value * residuals[row],
                                &mut sum,
                                &mut correction,
                            )?;
                            compensated_add(
                                weights[row] * value,
                                &mut mean_sum,
                                &mut mean_correction,
                            )?;
                            Ok(())
                        })?;
                        *norm = match input_norms {
                            Some(norms) => norms[column],
                            None => stable_norm(scale, square)?,
                        };
                        *score = (sum + correction).abs();
                        *mean = mean_sum + mean_correction;
                        Ok(())
                    })
            },
        );
        let response_checks = response_checks?;
        column_checks?;
        for ((score, mean), &norm) in self
            .frozen_workspace
            .scores
            .iter_mut()
            .zip(self.frozen_workspace.rhs.iter_mut())
            .zip(self.frozen_workspace.solve.iter())
        {
            if *score != 0. {
                *score = *score / norm / response_checks.reference_norm;
            }
            *mean /= response_checks.weight_sum;
            if !score.is_finite() || *score > self.config.absorb.map.tolerance || !mean.is_finite()
            {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "joint original-input finite score failed",
                ));
            }
        }
        let reconstruction = response_checks.reconstruction;
        let within = self.within.as_mut().expect("joint retained controls");
        for (&position, &mean) in within
            .active_positions
            .iter()
            .zip(&self.frozen_workspace.rhs)
        {
            within.reference_means[position] = mean;
        }
        let mut reported = std::mem::take(&mut self.frozen_workspace.reported_coefficients);
        let Some(FeSession {
            general: Some(projector),
            coefficients: true,
            ..
        }) = self.fe_projector.as_ref()
        else {
            unreachable!()
        };
        let joint = projector.joint_slopes().expect("joint solution");
        let basis = self.frozen_basis.as_ref().expect("joint basis");
        let reporting = self.build_frozen_ppml_reported_coefficients(
            basis.slots(),
            basis.kept_generated_columns(),
            joint.beta(),
            &mut reported,
            response_checks.response_mean,
        );
        self.frozen_workspace.reported_coefficients = reported;
        reporting?;
        self.frame = None;
        self.reconstruction = Some(reconstruction);
        let mut reuse = LinearSessionReuseReport::ppml_specialized(
            solving_seconds,
            checking.elapsed().as_secs_f64(),
        );
        // Every joint bank was admitted before the initial solve. The first call
        // warms the existing small score/report banks; allocator auditing decides
        // whether the complete subsequent iteration stays allocation-free.
        reuse.steady_state_allocation_free = true;
        self.reuse = Some(reuse);
        self.pending_response_buffer_reused = false;
        self.pending_weight_buffer_reused = false;
        // Retained controls remain FE-equivalent representatives, not a newly
        // certified current-weight within design. Their generations stay stale.
        Ok(())
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
        self.recycle_frame();
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

    /// Replace a stagnated retained projection with untouched generated
    /// inputs, without a second design allocation. Poll on the caller thread;
    /// the installed worker team only materializes Rust-owned numeric tiles.
    pub(crate) fn rebuild_retained_inputs_interruptible(
        &mut self,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.ensure_bound()?;
        let team = ThreadTeam::new(self.config.execution.threads).acquire()?;
        let prepared = self.prepared;
        let within = self.within.as_mut().ok_or_else(|| {
            invalid_argument("canonical profile rebuild requires a retained design")
        })?;
        if let RetainedWithinStorage::Implicit(values) = &mut within.storage {
            interrupt()?;
            values.invalidate();
            self.projection_refresh_required = true;
            return Ok(());
        }
        let generated = &within.stored_generated_columns;
        let columns = generated.len();
        let rows = self.response.len();
        if within.storage.rows() != rows || within.storage.columns() != columns {
            return Err(invalid_argument(
                "canonical profile rebuild dimensions disagree",
            ));
        }
        let fill = |values: &mut [f64], start_row: usize, ids: &[usize]| -> Result<()> {
            values
                .par_chunks_mut(ids.len())
                .enumerate()
                .try_for_each(|(row, target)| {
                    for (value, &column) in target.iter_mut().zip(ids) {
                        *value = prepared.fit_column_value(column, start_row + row)?;
                    }
                    Ok(())
                })
        };
        if columns > 0 {
            match &mut within.storage {
                RetainedWithinStorage::Implicit(_) => {
                    unreachable!("implicit originals require no materialization")
                }
                RetainedWithinStorage::Dense(values) => {
                    let tile_rows = (65_536 / columns).max(1);
                    for (tile, values) in values
                        .as_mut_slice()
                        .chunks_mut(tile_rows * columns)
                        .enumerate()
                    {
                        interrupt()?;
                        team.install(|| fill(values, tile * tile_rows, generated))?;
                    }
                }
                RetainedWithinStorage::Scratch(scratch) => {
                    let width = self.config.execution.poolsize.max(1).min(columns);
                    for start in (0..columns).step_by(width) {
                        let end = (start + width).min(columns);
                        let block = end - start;
                        resize_zeroed(
                            &mut self.projection_block,
                            checked_elements(rows, block)?,
                            "unable to reserve canonical profile rebuild block",
                        )?;
                        let tile_rows = (65_536 / block).max(1);
                        for (tile, values) in self
                            .projection_block
                            .chunks_mut(tile_rows * block)
                            .enumerate()
                        {
                            interrupt()?;
                            team.install(|| {
                                fill(values, tile * tile_rows, &generated[start..end])
                            })?;
                        }
                        interrupt()?;
                        scratch.patch_row_major_block(start, block, &self.projection_block)?;
                    }
                }
            }
        }
        // A fused response refresh now starts from z itself: z + (z-z).
        // This removes the prior residual from this independently rebuilt
        // checkpoint while retaining exactly the existing two vector banks.
        if self.config.execution.mode == PreparedOlsExecutionMode::Fast
            && self
                .fe_projector
                .as_ref()
                .is_some_and(FeSession::supports_fused_projection)
        {
            if self.frozen_workspace.response.len() != rows || self.previous_response.len() != rows
            {
                return Err(invalid_argument(
                    "canonical profile rebuild response banks disagree",
                ));
            }
            self.frozen_workspace
                .response
                .copy_from_slice(&self.response);
            self.previous_response.copy_from_slice(&self.response);
        }
        self.projection_refresh_required = true;
        Ok(())
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
            .eq(&stage.candidate.basis);

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
            frame: self.frame.take(),
            constant: self.constant.take(),
            reconstruction: self.reconstruction.take(),
            reuse: self.reuse.take(),
            projection_refresh_required: self.projection_refresh_required,
            canonical_projection_work_hint: self.canonical_projection_work_hint,
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
        transaction
            .session
            .frame
            .as_mut()
            .expect("validated candidate frame")
            .generations
            .basis = candidate_basis_generation;
        Ok(transaction)
    }

    fn restore_strict_rollback(&mut self, rollback: StrictLinearRollbackState) {
        // The speculative frame is discarded; recycling it would retain a second
        // bank beside the restored authority and grow every rollback footprint.
        self.frame = None;
        self.generations.basis = rollback.basis_generation;
        self.frozen_basis = Some(rollback.basis);
        if let Some(within) = self.within.as_mut() {
            within.active_positions = rollback.active_positions;
            within.certificate = rollback.within_certificate;
            within.generations = WithinGenerations::from_session(self.generations);
        }
        self.frame = rollback.frame;
        self.constant = rollback.constant;
        self.reconstruction = rollback.reconstruction;
        self.reuse = rollback.reuse;
        self.projection_refresh_required = rollback.projection_refresh_required;
        self.canonical_projection_work_hint = rollback.canonical_projection_work_hint;
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

    #[cfg(test)]
    pub(crate) fn bread_weight_scale_for_test(&self) -> super::PreparedOlsBreadWeightScale {
        self.bread_weight_scale
    }

    pub(crate) fn output(&self) -> Result<CertifiedLinearSessionOutput<'_>> {
        let fit = self.frame.as_ref().ok_or_else(|| {
            invalid_argument("weighted-linear session has no current certified solve")
        })?;
        if fit.canonical_proof().is_none()
            || fit.decomposition.is_none()
            || fit.generations != self.generations
        {
            return Err(invalid_argument(
                "weighted-linear session has no current canonical proof",
            ));
        }
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
            basis: self
                .frozen_basis
                .as_ref()
                .expect("canonical basis was validated"),
            frame: fit,
            response: &self.response,
            within,
            reconstruction: self.reconstruction.ok_or_else(|| {
                invalid_argument("weighted-linear session reconstruction is not certified")
            })?,
            reuse: self
                .reuse
                .ok_or_else(|| invalid_argument("weighted-linear session has no reuse report"))?,
        })
    }

    /// These are FE-equivalent representatives, not a current-weight within
    /// certificate. One-group Poisson profiling is invariant to the removed
    /// group constants; its original-input scores and canonical checks remain.
    pub(crate) fn one_fe_representatives(
        &self,
        active: &[usize],
    ) -> Result<(&DenseMatrix, &[usize])> {
        let within = self.validate_one_fe_profile_basis(active)?;
        let RetainedWithinStorage::Dense(values) = &within.storage else {
            return Err(invalid_argument(
                "profile representatives require dense storage",
            ));
        };
        Ok((values, &within.active_positions))
    }

    pub(crate) fn one_fe_original_columns(&self, active: &[usize]) -> Result<&[usize]> {
        self.validate_one_fe_profile_basis(active)?;
        Ok(self
            .frozen_basis
            .as_ref()
            .expect("validated profile basis")
            .kept_generated_columns())
    }

    fn validate_one_fe_profile_basis(&self, active: &[usize]) -> Result<&RetainedWithinDesign> {
        let design = self.fe_design();
        let within = self
            .within
            .as_ref()
            .ok_or_else(|| invalid_argument("profile design is absent"))?;
        if design.effects().len() != 1
            || !design.effects()[0].has_intercept()
            || !design.effects()[0].slopes().is_empty()
            || within.generations.sample != self.generations.sample
            || within.generations.topology != self.generations.topology
            || within.active_positions.len() != active.len()
            || within
                .active_positions
                .iter()
                .zip(active)
                .any(|(&position, &column)| {
                    within.stored_generated_columns.get(position) != Some(&column)
                })
            || self
                .frozen_basis
                .as_ref()
                .is_none_or(|basis| basis.kept_generated_columns() != active)
        {
            return Err(invalid_argument(
                "profile representatives have incompatible topology or basis",
            ));
        }
        Ok(within)
    }

    pub(crate) fn current_hessian_products_match(
        &self,
        weights: BorrowedMapWeights<'_>,
        scale: super::PreparedOlsBreadWeightScale,
        generation: u64,
    ) -> Result<bool> {
        Ok(self.generations.response == generation
            && self.generations.weights == generation
            && self.weights_exactly_match(weights, scale)
            && self.output()?.certified())
    }

    fn validated_output(&self) -> CertifiedLinearSessionOutput<'_> {
        CertifiedLinearSessionOutput {
            basis: self
                .frozen_basis
                .as_ref()
                .expect("canonical basis was validated"),
            frame: self
                .frame
                .as_ref()
                .expect("strict candidate output was prevalidated"),
            response: &self.response,
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
    pub(crate) fn into_current_products(
        mut self,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<CertifiedLinearSessionProducts> {
        self.output()?;
        if let Some(constant) = &mut self.constant {
            constant.certify(
                self.within.as_ref().expect("validated final within design"),
                self.frozen_basis.as_ref().expect("validated final basis"),
                (
                    self.weights.borrowed(),
                    self.bread_weight_scale.solver_weight_divisor,
                ),
                self.generations,
                &mut self.frozen_workspace.row,
                interrupt,
            )?;
        }
        let within = self
            .within
            .take()
            .ok_or_else(|| {
                invalid_argument("weighted-linear session has no final within design to hand off")
            })?
            .compact_for_inference()?;
        Ok(CertifiedLinearSessionProducts {
            constant: self.constant,
            frame: self.frame.ok_or_else(|| {
                invalid_argument("weighted-linear session has no final fit to hand off")
            })?,
            basis: self.frozen_basis.ok_or_else(|| {
                invalid_argument("weighted-linear session has no final basis to hand off")
            })?,
            weights: self.weights,
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
            .is_some_and(FeSession::supports_fused_projection)
        {
            reused &= self.previous_response.capacity() >= self.response.len();
            self.previous_response.clear();
            self.previous_response
                .try_reserve(self.response.len())
                .map_err(|_| allocation("unable to refresh linear response history"))?;
            self.previous_response.extend_from_slice(&self.response);
        }
        self.response.clear();
        self.response
            .try_reserve(response.len())
            .map_err(|_| allocation("unable to refresh weighted-linear session response"))?;
        self.response.extend_from_slice(response);
        self.generations.response = response_generation;
        self.recycle_frame();
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
        self.recycle_frame();
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
        let prior_geometry_generation = self.generations.map_geometry();
        self.refresh_response(response, generation)?;
        if identical_weights {
            self.generations.weights = generation;
            if let Some(FeSession {
                general: Some(projector),
                coefficients: true,
                ..
            }) = self.fe_projector.as_mut()
            {
                projector.rebind_identical_weight_generation(
                    prior_geometry_generation,
                    self.generations.map_geometry(),
                );
            }
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
        self.canonical_projection_work_hint = 0;
        self.recycle_frame();
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
        self.canonical_projection_work_hint = 0;
        self.frozen_basis = None;
        self.within = None;
        self.recycle_frame();
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

    fn recycle_frame(&mut self) {
        if let Some(step) = self.frame.take() {
            self.frozen_workspace.coefficients = step.coefficients;
            self.frozen_workspace.reported_coefficients = step.reported_coefficients;
            self.frozen_workspace.bread = step.bread;
            self.frozen_workspace.residuals = step.within_residuals;
            if let Some(predictors) = step.decomposition {
                self.predictors = predictors;
            }
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
        self.recycle_frame();
        let stage = self.prepare_retained_solve(kind, prior_active_positions)?;
        self.install_retained_stage(stage)
    }

    #[cfg(test)]
    pub(crate) fn canonical_projection_work_hint_for_test(&self) -> u128 {
        self.canonical_projection_work_hint
    }

    fn try_canonical_products(
        &mut self,
    ) -> Result<Option<(Vec<f64>, SolveReport, CanonicalMoments)>> {
        if (self.config.execution.mode != PreparedOlsExecutionMode::Low
            && self.fast_canonical_products_allowance == 0)
            || self.config.absorb.solver != FeSolverKind::Map
            || !self.fe_projector.as_ref().is_some_and(|projector| {
                matches!(
                    projector,
                    FeSession {
                        categorical: Some(_),
                        coefficients: false,
                        ..
                    } | FeSession {
                        coefficients: true,
                        categorical: Some(_),
                        ..
                    }
                )
            })
        {
            return Ok(None);
        }
        if let Some(within) = &self.within {
            if !matches!(
                within.storage,
                RetainedWithinStorage::Scratch(_) | RetainedWithinStorage::Dense(_)
            ) || within.generations.sample != self.generations.sample
                || within.generations.topology != self.generations.topology
                || (!self.projection_refresh_required
                    && within.generations.same_design(self.generations))
            {
                return Ok(None);
            }
        }
        let generated = self.full_retained_generated_columns()?;
        if generated.is_empty() {
            return Ok(None);
        }
        if let Some(within) = &self.within {
            if within.stored_generated_columns != generated {
                return Ok(None);
            }
        }
        // The old low projection owns a temporary raw/within block and a
        // separate reusable projection block. The optional current-call banks
        // replace those two reservations. The builder charges already-live
        // capacity until its complete plan permits releasing that obsolete
        // temporary panel. Nothing is added to the fit memory allowance.
        let pool = self.config.execution.poolsize.max(1).min(generated.len());
        let allowance = if self.config.execution.mode == PreparedOlsExecutionMode::Fast {
            self.fast_canonical_products_allowance
        } else {
            (self.response.len() as u128)
                .checked_mul(pool as u128)
                .and_then(|x| x.checked_mul(16))
                .ok_or_else(|| allocation("canonical product allowance overflows"))?
        };
        let Some(mut products) = super::canonical_products::try_build(
            self.prepared,
            &generated,
            &self.response,
            self.weights.borrowed(),
            self.bread_weight_scale.solver_weight_divisor,
            self.config.absorb.map,
            allowance,
            self.canonical_projection_work_hint,
            &mut self.projection_block,
        )?
        else {
            return Ok(None);
        };
        if let Some(within) = self.within.as_mut() {
            within.certificate.passed = false;
            match &mut within.storage {
                RetainedWithinStorage::Scratch(storage) => products.write_controls(storage)?,
                RetainedWithinStorage::Dense(storage) => products.write_dense(storage)?,
                RetainedWithinStorage::Implicit(_) => unreachable!(),
            }
            within
                .reference_norms
                .copy_from_slice(&products.scales[1..]);
            within.reference_means.copy_from_slice(&products.means[1..]);
            within.within_norms.copy_from_slice(&products.norms[1..]);
            within.generations = WithinGenerations::from_session(self.generations);
        } else {
            let storage = if self.config.execution.mode == PreparedOlsExecutionMode::Fast {
                let mut dense = DenseMatrix::zeros(self.response.len(), generated.len())?;
                products.write_dense(&mut dense)?;
                RetainedWithinStorage::Dense(dense)
            } else {
                let mut scratch =
                    PreparedOlsScratch::create_row_major(self.response.len(), generated.len())?;
                products.write_controls(&mut scratch)?;
                RetainedWithinStorage::Scratch(scratch)
            };
            self.within = Some(RetainedWithinDesign {
                storage,
                stored_generated_columns: generated,
                active_positions: Vec::new(),
                reference_norms: fallible_copy(
                    &products.scales[1..],
                    "unable to retain canonical product scales",
                )?,
                reference_means: fallible_copy(
                    &products.means[1..],
                    "unable to retain canonical product means",
                )?,
                within_norms: fallible_copy(
                    &products.norms[1..],
                    "unable to retain canonical product norms",
                )?,
                generations: WithinGenerations::from_session(self.generations),
                certificate: LinearWithinDesignCertificate {
                    left_inverse_relative_infinity_norm: f64::INFINITY,
                    right_inverse_relative_infinity_norm: f64::INFINITY,
                    passed: false,
                },
            });
        }
        // The dense residual bank and fused response anchor must describe the
        // same physical frame, including after a strict candidate is rolled
        // back. Otherwise the next ordinary refresh adds an obsolete z delta.
        if self.config.execution.mode == PreparedOlsExecutionMode::Fast {
            self.previous_response.copy_from_slice(&self.response);
        }
        Ok(Some((products.response, products.report, products.moments)))
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
        let products_reuse_storage = self.within.is_some();
        let operator_work_before = self
            .fe_projector
            .as_ref()
            .and_then(FeSession::ppml_operator_work);
        let product_attempt = self.try_canonical_products()?;
        let mut canonical_moments = None;
        let (within_response, absorption_report) =
            if let Some((response, projection, moments)) = product_attempt {
                canonical_moments = Some(moments);
                report.within_design_reused = products_reuse_storage;
                report.warm_projection_attempted = products_reuse_storage;
                report.warm_projection_certified = products_reuse_storage;
                (response, projection)
            } else if self.group_centered_source.is_some() && self.within.is_some() {
                report.warm_projection_attempted = true;
                let refreshed = self.refresh_group_centered_design()?;
                report.warm_projection_certified = true;
                report.within_design_reused = true;
                refreshed
            } else if self.within.is_none() {
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
                        && self.config.execution.mode == PreparedOlsExecutionMode::Fast
                        && self
                            .fe_projector
                            .as_ref()
                            .is_some_and(FeSession::supports_fused_projection)
                    {
                        report.fe_projection_workspace_reused = self.fill_reusable_map_weights()?;
                        let (reused, fused_report) = self.reproject_retained_in_place(true)?;
                        report.fe_projection_workspace_reused &= reused;
                        fused_response_report = fused_report;
                        #[cfg(test)]
                        {
                            response_fused_with_design = true;
                        }
                    } else if self.config.absorb.solver == FeSolverKind::Map {
                        report.fe_projection_workspace_reused = self.fill_reusable_map_weights()?;
                        report.fe_projection_workspace_reused &=
                            self.reproject_retained_in_place(false)?.0;
                    } else {
                        self.reproject_retained_in_place(false)?;
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
        let observed_projection_work = self
            .fe_projector
            .as_ref()
            .and_then(FeSession::ppml_operator_work)
            .zip(operator_work_before)
            .and_then(|(after, before)| after.checked_sub(before))
            .filter(|&work| work > 0);
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
                canonical_moments,
            ),
        };
        #[cfg(not(test))]
        let candidate = self.solve_full_retained_rank(
            &within_response,
            absorption_report,
            prior_active_positions,
            canonical_moments,
        );
        // `absorb_current_response` transfers the session-owned response
        // allocation into `within_response`.  Return that exact allocation to
        // the reusable workspace on both success and every downstream rank /
        // certificate error before propagating the result.
        self.frozen_workspace.response = within_response;
        if self
            .fe_projector
            .as_ref()
            .is_some_and(FeSession::supports_fused_projection)
        {
            // This residual and its raw-response anchor describe the same frame,
            // including after a fresh rank failure. A later weight refresh must
            // not add the already incorporated response delta a second time.
            self.previous_response.copy_from_slice(&self.response);
        }
        let candidate = candidate?;
        drop(finite_phase);
        let finite_solve_seconds = finite_started.elapsed().as_secs_f64();
        if kind != LinearSessionSolveKind::Strict {
            if let Some(basis) = &self.frozen_basis {
                if *basis != candidate.basis {
                    return Err(invalid_data(
                        "weighted-linear solve changed the frozen canonical rank basis",
                    ));
                }
            }
        }
        report.phase_timing = LinearSessionPhaseTiming {
            projection_seconds: Some(projection_seconds),
            finite_solve_seconds: Some(finite_solve_seconds),
        };
        report.phase_allocation = phase_allocation;
        // Only completed canonical projection/rank work updates the hint.
        // The next weight frame rebuilds and certifies its own geometry; this
        // scalar only compares its setup with work already paid on this model.
        if let Some(work) = observed_projection_work {
            self.canonical_projection_work_hint = work;
        }
        Ok(RetainedSolveStage {
            candidate,
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
            mut candidate,
            mut report,
            ..
        } = stage;
        if let Some(basis) = &self.frozen_basis {
            if *basis != candidate.basis {
                return Err(invalid_data(
                    "weighted-linear solve changed the frozen canonical rank basis",
                ));
            }
        }
        // The canonical draft owns the one original-input predictor used by
        // the intercept check. Move it through publication instead of scanning
        // every original column a second time. Strict rollback owns the prior
        // predictor separately; no speculative product mutates that state.
        self.predictors.finite = candidate.finite_predictor;
        let predictor_workspace_reused = false;
        let reconstruction = self.rebuild_predictors(&candidate.frame)?;
        let within = self
            .within
            .as_mut()
            .ok_or_else(|| invalid_argument("weighted-linear retained design disappeared"))?;
        within.active_positions = candidate.active_positions;
        within.certificate = candidate.within_certificate;
        report.predictor_workspace_reused = predictor_workspace_reused;
        report.certificate_workspace_reused = true;
        self.constant = candidate.constant;
        candidate.frame.decomposition = Some(std::mem::take(&mut self.predictors));
        self.frame = Some(candidate.frame);
        self.frozen_basis = Some(candidate.basis);
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
            self.generations.map_geometry(),
        )
    }

    /// Borrow norms from this call's fresh certificate only when its weight
    /// normalization matches the finite solve. Scratch tiles carry
    /// their own column axis; no cached norm survives a projection call.
    fn absorb_reusable_map_with_norms(
        &mut self,
        values: DenseMatrix,
        positions: std::ops::Range<usize>,
        norms: &mut [f64],
    ) -> Result<(CertifiedBorrowedMap, bool)> {
        match self.fe_projector.as_mut() {
            Some(FeSession {
                categorical: Some(projector),
                ..
            }) => projector.absorb_with_selected_norms(
                self.weights.borrowed(),
                values,
                self.config.absorb.map,
                self.bread_weight_scale.solver_weight_divisor,
                positions,
                norms,
            ),
            Some(FeSession {
                coefficients: true,
                general: Some(projector),
                categorical: None,
                ..
            }) => Ok((
                projector.absorb_coefficient_with_selected_norms(
                    &self.map_weights,
                    values,
                    self.config.absorb.map,
                    self.generations.map_geometry(),
                    positions,
                    None,
                    norms,
                )?,
                true,
            )),
            _ => Ok((self.absorb_reusable_map(values)?, false)),
        }
    }

    fn absorb_slope_with_selected_norms(
        &mut self,
        values: DenseMatrix,
        positions: impl ExactSizeIterator<Item = usize> + Clone,
        reference: Option<&mut [f64]>,
        residual: &mut [f64],
    ) -> Result<CertifiedBorrowedMap> {
        let Some(FeSession {
            coefficients: true,
            general: Some(projector),
            categorical: None,
            ..
        }) = self.fe_projector.as_mut()
        else {
            return Err(invalid_argument(
                "slope norm handoff requires its coefficient owner",
            ));
        };
        projector.absorb_coefficient_with_selected_norms(
            &self.map_weights,
            values,
            self.config.absorb.map,
            self.generations.map_geometry(),
            positions,
            reference,
            residual,
        )
    }

    fn reusable_map_max_rhs(&self) -> Result<usize> {
        self.fe_projector
            .as_ref()
            .map(FeSession::max_rhs)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "weighted-linear MAP route has no reusable projector",
                )
            })
    }

    fn solve_frozen_internal(&mut self, output_kind: FrozenLinearOutputKind) -> Result<()> {
        self.ensure_bound()?;
        self.recycle_frame();
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
            if matches!(
                within.storage,
                RetainedWithinStorage::Scratch(_) | RetainedWithinStorage::Implicit(_)
            ) {
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
                && self.config.execution.mode == PreparedOlsExecutionMode::Fast
                && self
                    .fe_projector
                    .as_ref()
                    .is_some_and(FeSession::supports_fused_projection)
            {
                let (reused, fused_report) = self
                    .reproject_retained_in_place(true)
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
                fused_response_report = fused_report;
            } else if reusable_map {
                projection_workspace_reused &=
                    self.reproject_retained_in_place(false).map(|result| result.0).map_err(|error| {
                        FeregError::new(
                            error.code,
                            format!(
                                "frozen weighted-linear reusable MAP projection failed; strict canonical solve required: {}",
                                error.message
                            ),
                        )
                    })?;
            } else {
                self.reproject_retained_in_place(false).map(|result| result.0).map_err(|error| {
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
                self.predictors.finite.capacity() >= self.response.len()
                    && self.predictors.absorbed.capacity() >= self.response.len(),
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
                    None,
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
        self.frame = Some(CertifiedLinearFrame {
            coefficients: frozen.coefficients,
            reported_coefficients,
            bread: frozen.bread,
            within_residuals: frozen.within_residuals,
            response_absorption_report,
            proof: LinearFrameProof::Frozen(frozen.certificate),
            generations: self.generations,
            decomposition: match output_kind {
                #[cfg(test)]
                FrozenLinearOutputKind::FullOlsDecomposition => {
                    Some(std::mem::take(&mut self.predictors))
                }
                FrozenLinearOutputKind::PpmlCombinedPredictor => None,
            },
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
        let step = self.frame.as_ref().ok_or_else(|| {
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
        let step = self.frame.as_ref().ok_or_else(|| {
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
            &mut self.predictors.finite,
            self.response.len(),
            "unable to reserve frozen weighted-linear finite predictor",
        )?;
        resize_zeroed(
            &mut self.predictors.absorbed,
            self.response.len(),
            "unable to reserve frozen weighted-linear absorbed predictor",
        )?;
        for (generated, coefficient) in kept.iter().copied().zip(coefficients.iter().copied()) {
            for row in 0..self.response.len() {
                self.predictors.finite[row] = coefficient.mul_add(
                    self.prepared.fit_column_value(generated, row)?,
                    self.predictors.finite[row],
                );
            }
        }
        let mut certificate = LinearReconstructionCertificate::new();
        for (row, within_residual) in within_residuals.iter().copied().enumerate() {
            self.predictors.absorbed[row] =
                (self.response[row] - within_residual) - self.predictors.finite[row];
            certificate.observe(
                self.response[row],
                self.predictors.finite[row],
                self.predictors.absorbed[row],
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
                    weight.mul_add(self.response[row] - self.predictors.finite[row], numerator);
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
        current_response_mean: Option<f64>,
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
            let outcome_mean = match current_response_mean {
                Some(mean) => mean,
                None => weighted_mean_slice(
                    &self.response,
                    self.weights.borrowed(),
                    self.bread_weight_scale,
                )?,
            };
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
        canonical_moments: Option<CanonicalMoments>,
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
        let mut constant = None;
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
            let source = retained.storage.moment_source(
                &workspace.candidate_positions,
                within_response,
                self.weights.borrowed(),
                self.bread_weight_scale,
                &workspace.candidate_scales,
            )?;
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
            if let Some(products) = canonical_moments {
                if products.width != stored_columns + 1
                    || products.gram.len() != products.width * products.width
                {
                    return Err(invalid_argument(
                        "canonical moment handoff has an incompatible column axis",
                    ));
                }
                for (j, &position) in workspace.candidate_positions.iter().enumerate() {
                    workspace.rhs[j] =
                        products.gram[(position + 1) * products.width] * products.response_scale;
                    for (k, &other) in workspace.candidate_positions.iter().enumerate() {
                        workspace.gram[j * candidate_count + k] =
                            products.gram[(position + 1) * products.width + other + 1];
                    }
                }
            } else {
                fill_finite_wls_moments(
                    source,
                    layout,
                    &mut workspace.moment_partials,
                    &mut workspace.rhs,
                    &mut workspace.gram,
                )?;
            }
            let result = super::finite_operations::solve_factor(
                source,
                &workspace.candidate_ids,
                relative_threshold,
                super::finite_operations::RankRequest::RetainedDiscover,
                true,
                super::finite_operations::FactorBanks {
                    gram: &workspace.gram,
                    rhs: &workspace.rhs,
                    factor: &mut workspace.factor,
                    auxiliary: &mut workspace.inverse_lower,
                    solve: &mut workspace.solve,
                    scores: &mut workspace.scores,
                    row: &mut workspace.row,
                    permutation: &mut workspace.permutation,
                    coefficients: &mut workspace.coefficients,
                    inverse: &mut workspace.bread,
                    selected: &mut workspace.kept,
                },
                &mut super::prepared_fit::SystemFitReservation,
                |relations, rank, initial_norm| {
                    constant = LinearConstantRepresentation::capture(
                        self.prepared.fit_model(),
                        retained,
                        relations,
                        rank,
                        initial_norm,
                        relative_threshold,
                        self.generations,
                    )?;
                    Ok(())
                },
            )?;
            (
                result.report.rank,
                result.report.maximum_dropped_relative_norm,
                result.report.condition_proxy,
            )
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
        if let Some(tolerance) = self.config.coefficient_tolerance {
            resize_zeroed(
                &mut workspace.scores,
                rank,
                "unable to reserve retained refinement scores",
            )?;
            resize_zeroed(
                &mut workspace.solve,
                rank,
                "unable to reserve retained refinement corrections",
            )?;
            refine_workspace(
                RefinementFrame {
                    storage: &retained.storage,
                    active_positions: &active_positions,
                    reference_norms: &retained.reference_norms,
                    response: within_response,
                    weights: self.weights.borrowed(),
                    weight_scale: self.bread_weight_scale,
                },
                workspace,
                tolerance,
            )?;
            for (selected, &generated) in workspace.kept.iter().enumerate() {
                generated_coefficients[generated] = Some(
                    workspace.coefficients[selected]
                        / retained.reference_norms[active_positions[selected]],
                );
            }
        }
        let inverse = certify_finite_inverse(
            &workspace.gram,
            candidate_count,
            &selected_candidate_positions,
            &workspace.bread,
            rank,
        )?;
        let left_inverse_error = inverse.left;
        let right_inverse_error = inverse.right;
        if !inverse.passed(self.config.bread_tolerance) {
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
        let weights = self.weights.borrowed();
        let scale = self.bread_weight_scale;
        let reference_norms = &retained.reference_norms;
        let coefficients = &workspace.coefficients;
        let residual_at =
            |row: usize, value: &mut dyn FnMut(usize) -> Result<f64>| -> Result<f64> {
                let within_value = within_response[row];
                let root_weight = normalized_solver_weight(weights, scale, row)?.sqrt();
                let mut weighted_residual = root_weight * within_value;
                for (selected, storage_position) in active_positions.iter().copied().enumerate() {
                    let standardized_value =
                        root_weight * value(storage_position)? / reference_norms[storage_position];
                    weighted_residual =
                        (-coefficients[selected]).mul_add(standardized_value, weighted_residual);
                }
                let residual = weighted_residual / root_weight;
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "retained weighted-linear residual is not finite",
                    ));
                }
                Ok(residual)
            };
        match &retained.storage {
            RetainedWithinStorage::Dense(values) => {
                workspace
                    .residuals
                    .par_iter_mut()
                    .enumerate()
                    .try_for_each(|(row, target)| -> Result<()> {
                        *target = residual_at(row, &mut |column| Ok(values[(row, column)]))?;
                        Ok(())
                    })?;
            }
            RetainedWithinStorage::Implicit(values) => {
                let view = values.view()?;
                let bound = view.bind_columns()?;
                workspace
                    .residuals
                    .par_iter_mut()
                    .enumerate()
                    .try_for_each(|(row, target)| -> Result<()> {
                        let group = view.codes()[row] as usize;
                        *target = residual_at(row, &mut |column| {
                            Ok(bound[column].value(row)? - view.center(column, group))
                        })?;
                        Ok(())
                    })?;
            }
            RetainedWithinStorage::Scratch(_) => {
                // Finite moments have been consumed. Reuse their admitted bank
                // for disjoint row buffers; each worker owns an immutable-read
                // cursor. Read every stored column, including omitted ones, so
                // scratch validation and each row's arithmetic remain unchanged.
                let workers = rayon::current_num_threads()
                    .min(rows)
                    .min(workspace.moment_partials.len() / stored_columns.max(1));
                if stored_columns != 0 && workers > 1 {
                    let chunk_rows = rows.div_ceil(workers);
                    // Every residual chunk needs its own row bank; a short bank
                    // would silently truncate the zip below.
                    if rows.div_ceil(chunk_rows) > workspace.moment_partials.len() / stored_columns
                    {
                        return Err(crate::error::allocation(
                            "retained residual reconstruction lacks one row bank per chunk",
                        ));
                    }
                    workspace
                        .residuals
                        .par_chunks_mut(chunk_rows)
                        .zip(
                            workspace
                                .moment_partials
                                .par_chunks_exact_mut(stored_columns),
                        )
                        .enumerate()
                        .try_for_each(|(chunk, (targets, values))| -> Result<()> {
                            let mut reader = retained.storage.reader();
                            for (local_row, target) in targets.iter_mut().enumerate() {
                                let row = chunk * chunk_rows + local_row;
                                reader.fill_row(row, values)?;
                                *target = residual_at(row, &mut |column| Ok(values[column]))?;
                            }
                            Ok(())
                        })?;
                } else {
                    let mut reader = retained.storage.reader();
                    for (row, target) in workspace.residuals.iter_mut().enumerate() {
                        reader.fill_row(row, &mut workspace.row)?;
                        *target = residual_at(row, &mut |column| Ok(workspace.row[column]))?;
                    }
                }
            }
        }
        let within_outcome_norm = stable_weighted_norm(rows, |row| {
            Ok((
                within_response[row],
                normalized_solver_weight(self.weights.borrowed(), self.bread_weight_scale, row)?,
            ))
        })?;
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
        let kept = fallible_copy(&workspace.kept, "unable to copy retained kept identities")?;
        let finite_bread_generated_columns =
            fallible_copy(&workspace.kept, "unable to copy retained bread identities")?;
        let residual_matrix =
            DenseMatrix::from_row_major(rows, 1, std::mem::take(&mut workspace.residuals))?;
        let residual_absorbed = if self.config.absorb.solver == FeSolverKind::Map {
            self.fill_reusable_map_weights()?;
            if let Some(FeSession {
                coefficients: true,
                general: Some(projector),
                categorical: None,
                ..
            }) = self.fe_projector.as_mut()
            {
                projector.absorb_coefficient_residual(
                    &self.map_weights,
                    residual_matrix,
                    self.config.absorb.map,
                    self.generations.map_geometry(),
                )?
            } else {
                self.absorb_reusable_map(residual_matrix)?
            }
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
        let retained = self
            .within
            .as_ref()
            .ok_or_else(|| invalid_argument("current-frame means are absent"))?;
        if !retained.generations.same_design(self.generations) {
            return Err(invalid_argument(
                "current-frame means have stale design generations",
            ));
        }
        let finite_score = build_finite_score_draft(
            &self.frozen_workspace.candidate_ids,
            &self.frozen_workspace.gram,
            &self.frozen_workspace.rhs,
            &generated_coefficients,
            &generated_statuses,
            &reference_norms,
            &score_norms,
        )?;
        let finite = FiniteFit {
            generated_coefficients,
            generated_statuses,
            rank: kept.len(),
            kept,
            dropped,
            certificate: super::PreparedOlsRankCertificate {
                relative_threshold,
                maximum_dropped_trailing_norm: maximum_dropped_relative_norm,
                condition_proxy,
                passed: true,
            },
            finite_score,
            bread: Some(FiniteBreadDraft {
                generated_columns: finite_bread_generated_columns,
                values: finite_bread,
                reference_crossproduct,
                weight_scale: self.bread_weight_scale,
                left_inverse_relative_infinity_norm: left_inverse_error,
                right_inverse_relative_infinity_norm: right_inverse_error,
            }),
            within_residuals: Some(residuals.into_row_major()),
        };
        let draft = RetainedFitContext {
            raw_column_means: CurrentFrameColumnMeans {
                generated_columns: &retained.stored_generated_columns,
                values: &retained.reference_means,
            },
            reference_norms,
            score_norms,
            within_outcome_norm,
            residual_absorption_report,
            absorption_report,
        };
        let (frame, basis, finite_predictor) = assemble_retained_linear_frame(
            self.prepared,
            LinearSolveFrame::from_parts(
                &self.response,
                self.weights.borrowed(),
                self.bread_weight_scale,
            )?,
            self.config,
            finite,
            draft,
            self.generations,
        )?;
        Ok(RetainedRankSolveDraft {
            constant,
            frame,
            basis,
            finite_predictor,
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

    /// Own one complete current-weight projected representation. Admitted
    /// one-FE sessions construct group centers, materializing fast dense rows
    /// or retaining low implicit rows; other sessions keep their prior storage.
    fn build_full_retained_design(
        &mut self,
    ) -> Result<(RetainedWithinDesign, Vec<f64>, SolveReport)> {
        if self.group_centered_source.is_some()
            && (self.config.absorb.map.max_iterations == 0
                || self.config.absorb.map.check_every == 0)
        {
            return Err(invalid_argument(
                "implicit MAP iteration and check budgets must be positive",
            ));
        }
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

        let slope_norms = self.config.absorb.solver == FeSolverKind::Map
            && matches!(
                self.fe_projector,
                Some(FeSession {
                    coefficients: true,
                    categorical: None,
                    ..
                })
            );

        let (storage, within_response, absorption_report) = if let Some(source) =
            &self.group_centered_source
        {
            let mut values = ImplicitCenteredDesign::new(
                source.clone(),
                &stored_generated_columns,
                self.config.execution.mode,
            )?;
            let (response, report) = values.refresh(
                &self.response,
                self.weights.borrowed(),
                self.bread_weight_scale.solver_weight_divisor,
                self.config.absorb.map.tolerance,
                &mut reference_norms,
                &mut reference_means,
                &mut within_norms,
                std::mem::take(&mut self.frozen_workspace.response),
            )?;
            let storage = if self.config.execution.mode == PreparedOlsExecutionMode::Fast {
                let mut dense = DenseMatrix::zeros(rows, stored_columns)?;
                values.write_dense(&mut dense)?;
                self.centered_projection = Some(values);
                RetainedWithinStorage::Dense(dense)
            } else {
                RetainedWithinStorage::Implicit(values)
            };
            (storage, response, report)
        } else {
            match self.config.execution.mode {
                PreparedOlsExecutionMode::Fast => {
                    let raw = self.prepared.fit_build_rhs_with_response(&self.response)?;
                    let expected_columns = self
                        .prepared
                        .fit_model()
                        .columns
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| {
                        allocation("weighted-linear combined width overflows")
                    })?;
                    if raw.nrows() != rows || raw.ncols() != expected_columns {
                        return Err(invalid_argument(
                            "weighted-linear combined retained design is inconsistent",
                        ));
                    }
                    if slope_norms {
                        self.fill_dense_statistics::<false, true>(
                            &raw,
                            |column| stored_generated_columns[column] + 1,
                            &mut [],
                            &mut reference_means,
                        )?;
                    } else {
                        self.fill_dense_statistics::<true, true>(
                            &raw,
                            |column| stored_generated_columns[column] + 1,
                            &mut reference_norms,
                            &mut reference_means,
                        )?;
                    }
                    let mut norms_reused = false;
                    let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
                        self.fill_reusable_map_weights()?;
                        if slope_norms {
                            norms_reused = true;
                            self.absorb_slope_with_selected_norms(
                                raw,
                                stored_generated_columns.iter().map(|column| column + 1),
                                Some(&mut reference_norms),
                                &mut within_norms,
                            )?
                        } else if let Some(FeSession {
                            categorical: Some(projector),
                            ..
                        }) = self.fe_projector.as_mut()
                        {
                            let (absorbed, reused) = projector.absorb_with_selected_norms(
                                self.weights.borrowed(),
                                raw,
                                self.config.absorb.map,
                                self.bread_weight_scale.solver_weight_divisor,
                                stored_generated_columns.iter().map(|column| column + 1),
                                &mut within_norms,
                            )?;
                            norms_reused = reused;
                            absorbed
                        } else {
                            self.absorb_reusable_map(raw)?
                        }
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
                    let mut response = fallible_filled(
                        rows,
                        0.0_f64,
                        "unable to reserve weighted-linear retained within response",
                    )?;
                    for (row, response_value) in response.iter_mut().enumerate().take(rows) {
                        let source_start = row.checked_mul(stride).ok_or_else(|| {
                            allocation("weighted-linear combined offset overflows")
                        })?;
                        *response_value = values[source_start];
                        let target_start = row.checked_mul(stored_columns).ok_or_else(|| {
                            allocation("weighted-linear retained compact offset overflows")
                        })?;
                        for (target, generated) in
                            stored_generated_columns.iter().copied().enumerate()
                        {
                            values[target_start + target] = values[source_start + generated + 1];
                        }
                    }
                    values.truncate(checked_elements(rows, stored_columns)?);
                    let compact = DenseMatrix::from_row_major(rows, stored_columns, values)?;
                    if !norms_reused {
                        self.fill_dense_statistics::<true, false>(
                            &compact,
                            |column| column,
                            &mut within_norms,
                            &mut [],
                        )?;
                    }
                    (RetainedWithinStorage::Dense(compact), response, report)
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
                        raw_values
                            .par_chunks_mut(128 * block)
                            .enumerate()
                            .try_for_each(|(tile, values)| -> Result<()> {
                                let start_row = tile * 128;
                                let end_row = start_row + values.len() / block;
                                for local in 0..block {
                                    crate::prepared_fit_view::visit_prepared_column_range(
                                        self.prepared,
                                        stored_generated_columns[start + local],
                                        start_row..end_row,
                                        |row, value| {
                                            values[(row - start_row) * block + local] = value;
                                            Ok(())
                                        },
                                    )?;
                                }
                                Ok(())
                            })?;
                        let raw = DenseMatrix::from_row_major(rows, block, raw_values)?;
                        if slope_norms {
                            self.fill_dense_statistics::<false, true>(
                                &raw,
                                |column| column,
                                &mut [],
                                &mut reference_means[start..end],
                            )?;
                        } else {
                            self.fill_dense_statistics::<true, true>(
                                &raw,
                                |column| column,
                                &mut reference_norms[start..end],
                                &mut reference_means[start..end],
                            )?;
                        }
                        let (absorbed, norms_reused) = if slope_norms {
                            (
                                self.absorb_slope_with_selected_norms(
                                    raw,
                                    0..block,
                                    Some(&mut reference_norms[start..end]),
                                    &mut within_norms[start..end],
                                )?,
                                true,
                            )
                        } else if self.config.absorb.solver == FeSolverKind::Map {
                            self.absorb_reusable_map_with_norms(
                                raw,
                                0..end - start,
                                &mut within_norms[start..end],
                            )?
                        } else {
                            (
                                self.prepared.fit_absorb_matrix(
                                    self.weights.borrowed(),
                                    raw,
                                    self.config.absorb,
                                )?,
                                false,
                            )
                        };
                        if !absorbed.report().certified() {
                            return Err(invalid_data(
                                "weighted-linear low-memory design absorption did not certify",
                            ));
                        }
                        let (within, block_report) = absorbed.into_parts();
                        merge_canonical_projection_reports(&mut report, block_report)?;
                        if !norms_reused {
                            self.fill_dense_statistics::<true, false>(
                                &within,
                                |column| column,
                                &mut within_norms[start..end],
                                &mut [],
                            )?;
                        }
                        scratch.patch_row_major_block(start, block, within.as_slice())?;
                    }
                    (
                        RetainedWithinStorage::Scratch(scratch),
                        response.into_row_major(),
                        report,
                    )
                }
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

    fn refresh_group_centered_design(&mut self) -> Result<(Vec<f64>, SolveReport)> {
        if self.config.absorb.map.max_iterations == 0 || self.config.absorb.map.check_every == 0 {
            return Err(invalid_argument(
                "implicit MAP iteration and check budgets must be positive",
            ));
        }
        let mut retained = self
            .within
            .take()
            .ok_or_else(|| invalid_argument("implicit within design is absent"))?;
        if retained.generations.sample != self.generations.sample
            || retained.generations.topology != self.generations.topology
        {
            return Err(invalid_argument(
                "implicit within sample or topology is stale",
            ));
        }
        let refreshed = match &mut retained.storage {
            RetainedWithinStorage::Implicit(values) => values.refresh(
                &self.response,
                self.weights.borrowed(),
                self.bread_weight_scale.solver_weight_divisor,
                self.config.absorb.map.tolerance,
                &mut retained.reference_norms,
                &mut retained.reference_means,
                &mut retained.within_norms,
                std::mem::take(&mut self.frozen_workspace.response),
            )?,
            RetainedWithinStorage::Dense(dense) => {
                let values = self
                    .centered_projection
                    .as_mut()
                    .ok_or_else(|| invalid_argument("centered projection owner is absent"))?;
                let refreshed = values.refresh(
                    &self.response,
                    self.weights.borrowed(),
                    self.bread_weight_scale.solver_weight_divisor,
                    self.config.absorb.map.tolerance,
                    &mut retained.reference_norms,
                    &mut retained.reference_means,
                    &mut retained.within_norms,
                    std::mem::take(&mut self.frozen_workspace.response),
                )?;
                values.write_dense(dense)?;
                refreshed
            }
            RetainedWithinStorage::Scratch(_) => {
                return Err(invalid_argument(
                    "centered refresh requires its owned representation",
                ))
            }
        };
        retained.generations = WithinGenerations::from_session(self.generations);
        retained.certificate.passed = false;
        self.within = Some(retained);
        Ok(refreshed)
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
        let (mut values, source_stride) = match cache {
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
                (within.into_row_major(), generated_columns.len())
            }
            PreparedOlsFastScoreCache::Selected {
                generated_columns,
                within,
                means,
            } => {
                if within.nrows() != rows
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
                for generated in kept.iter().copied() {
                    let source = generated_columns
                        .iter()
                        .position(|candidate| *candidate == generated)
                        .ok_or_else(|| {
                            invalid_argument(
                                "weighted-linear strided cache is missing a kept column",
                            )
                        })?;
                    let _ = source;
                    sources.push(generated.checked_add(1).ok_or_else(|| {
                        allocation("weighted-linear strided cache coordinate overflows")
                    })?);
                }
                let stride = within.ncols();
                (within.into_row_major(), stride)
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
            for (target, source) in sources.iter().copied().enumerate() {
                let value = values[source_start + source];
                row_values[target] = value;
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
            .is_some_and(FeSession::borrows_raw_weights)
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

    /// Weight refresh preserves each physical bank and its raw-response anchor.
    /// The optional leading RHS uses M_new(r_old + z_new - z_old) = M_new z_new.
    fn reproject_retained_in_place(
        &mut self,
        with_response: bool,
    ) -> Result<(bool, Option<SolveReport>)> {
        if self.group_centered_source.is_some() {
            let (response, report) = self.refresh_group_centered_design()?;
            if with_response {
                self.frozen_workspace.response = response;
            }
            return Ok((false, with_response.then_some(report)));
        }
        if with_response
            && !self
                .fe_projector
                .as_ref()
                .is_some_and(FeSession::supports_fused_projection)
        {
            return Err(invalid_argument(
                "fused weighted-linear projection requires the admitted PPML projector",
            ));
        }
        let mut prior = self
            .within
            .take()
            .ok_or_else(|| invalid_argument("weighted-linear projection has no retained design"))?;
        let rows = prior.storage.rows();
        let columns = prior.storage.columns();
        if rows != self.response.len()
            || prior.reference_norms.len() != columns
            || prior.reference_means.len() != columns
            || prior.within_norms.len() != columns
            || with_response
                && (self.frozen_workspace.response.len() != rows
                    || self.previous_response.len() != rows)
        {
            return Err(invalid_argument(
                "weighted-linear response/design state is not aligned",
            ));
        }
        prior.within_norms.fill(0.0);
        let mut workspace_reused = true;
        let mut response_report = None;
        prior.storage = match prior.storage {
            RetainedWithinStorage::Implicit(_) => unreachable!("implicit refresh handled above"),
            RetainedWithinStorage::Dense(matrix) => {
                if columns == 0 && !with_response {
                    RetainedWithinStorage::Dense(matrix)
                } else {
                    let matrix = if with_response {
                        let (matrix, reused) = self.expand_retained_response(matrix)?;
                        workspace_reused &= reused;
                        matrix
                    } else {
                        matrix
                    };
                    let first = usize::from(with_response);
                    if self.config.absorb.solver == FeSolverKind::Map
                        && matrix.ncols() > self.reusable_map_max_rhs()?
                    {
                        return Err(invalid_argument("weighted-linear reusable dense MAP projection exceeds its fixed workspace"));
                    }
                    let (absorbed, norms_reused) = if self.config.absorb.solver == FeSolverKind::Map
                    {
                        self.absorb_reusable_map_with_norms(
                            matrix,
                            first..first + columns,
                            &mut prior.within_norms,
                        )?
                    } else {
                        (
                            self.prepared.fit_absorb_matrix(
                                self.weights.borrowed(),
                                matrix,
                                self.config.absorb,
                            )?,
                            false,
                        )
                    };
                    let (matrix, report) = absorbed.into_parts();
                    if !report.certified() {
                        return Err(invalid_data(
                            "weighted-linear response/design projection did not certify",
                        ));
                    }
                    let matrix = if with_response {
                        response_report = Some(report);
                        self.compact_retained_response(matrix)?
                    } else {
                        matrix
                    };
                    if !norms_reused {
                        self.fill_dense_statistics::<true, false>(
                            &matrix,
                            |column| column,
                            &mut prior.within_norms,
                            &mut [],
                        )?;
                    }
                    RetainedWithinStorage::Dense(matrix)
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
                    scratch.read_row_major_block(start, block, &mut self.projection_block)?;
                    let raw = DenseMatrix::from_row_major(
                        rows,
                        block,
                        std::mem::take(&mut self.projection_block),
                    )?;
                    let (values, _, reused) =
                        self.project_retained_block(raw, &mut prior.within_norms[start..end])?;
                    workspace_reused &= reused;
                    self.projection_block = values.into_row_major();
                    scratch.patch_row_major_block(start, block, &self.projection_block)?;
                }
                RetainedWithinStorage::Scratch(scratch)
            }
        };
        prior.generations = WithinGenerations::from_session(self.generations);
        prior.certificate = LinearWithinDesignCertificate {
            left_inverse_relative_infinity_norm: f64::INFINITY,
            right_inverse_relative_infinity_norm: f64::INFINITY,
            passed: false,
        };
        self.within = Some(prior);
        Ok((workspace_reused, response_report))
    }

    fn project_retained_block(
        &mut self,
        matrix: DenseMatrix,
        norms: &mut [f64],
    ) -> Result<(DenseMatrix, SolveReport, bool)> {
        let (absorbed, norms_reused) = if self.config.absorb.solver == FeSolverKind::Map {
            self.absorb_reusable_map_with_norms(matrix, 0..norms.len(), norms)?
        } else {
            (
                self.prepared.fit_absorb_matrix(
                    self.weights.borrowed(),
                    matrix,
                    self.config.absorb,
                )?,
                false,
            )
        };
        let (matrix, report) = absorbed.into_parts();
        if !report.certified() {
            return Err(invalid_data(
                "weighted-linear reusable projection did not certify",
            ));
        }
        if !norms_reused {
            self.fill_dense_statistics::<true, false>(&matrix, |column| column, norms, &mut [])?;
        }
        Ok((matrix, report, true))
    }

    fn expand_retained_response(&self, matrix: DenseMatrix) -> Result<(DenseMatrix, bool)> {
        let rows = matrix.nrows();
        let columns = matrix.ncols();
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
        let previous_response = self.previous_response.as_slice();
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
                self.frozen_workspace.response[row] + (self.response[row] - previous_response[row]);
            if !response.is_finite() {
                return Err(invalid_data(
                    "fused weighted-linear response warm start is nonfinite",
                ));
            }
            values[target] = response;
        }

        Ok((
            DenseMatrix::from_row_major(rows, stride, values)?,
            workspace_reused,
        ))
    }

    fn compact_retained_response(&mut self, matrix: DenseMatrix) -> Result<DenseMatrix> {
        let rows = matrix.nrows();
        let stride = matrix.ncols();
        let columns = stride - 1;
        let mut values = matrix.into_row_major();
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
        DenseMatrix::from_row_major(rows, columns, values)
    }

    fn rebuild_predictors(
        &mut self,
        fit: &CertifiedLinearFrame,
    ) -> Result<LinearReconstructionCertificate> {
        let rows = self.response.len();
        if self.predictors.finite.len() != rows {
            return Err(invalid_argument("canonical finite predictor is misaligned"));
        }
        resize_zeroed(
            &mut self.predictors.absorbed,
            rows,
            "unable to reserve weighted-linear absorbed predictor",
        )?;
        let mut certificate = LinearReconstructionCertificate::new();
        for row in 0..rows {
            self.predictors.absorbed[row] =
                (self.response[row] - fit.residuals()[row]) - self.predictors.finite[row];
            certificate.observe(
                self.response[row],
                self.predictors.finite[row],
                self.predictors.absorbed[row],
                fit.residuals()[row],
            )?;
        }
        Ok(certificate)
    }
}

struct FrozenSolveDraft {
    coefficients: Vec<f64>,
    bread: Vec<f64>,
    within_residuals: Vec<f64>,
    certificate: FrozenLinearSolveCertificate,
}

struct RetainedRankSolveDraft {
    constant: Option<LinearConstantRepresentation>,
    frame: CertifiedLinearFrame,
    basis: FrozenLinearBasis,
    finite_predictor: Vec<f64>,
    active_positions: Vec<usize>,
    within_certificate: LinearWithinDesignCertificate,
    prior_basis_certificate: Option<LinearWithinDesignCertificate>,
}

struct RetainedSolveStage {
    candidate: RetainedRankSolveDraft,
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
    let source = storage.moment_source(
        active_positions,
        within_response,
        weights,
        bread_weight_scale,
        &workspace.candidate_scales,
    )?;
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
    let relative_threshold =
        config.rank_tolerance_multiplier * f64::EPSILON * rows.max(stored_columns).max(1) as f64;
    let result = super::finite_operations::solve_factor(
        source,
        active_positions,
        relative_threshold,
        super::finite_operations::RankRequest::Frozen,
        true,
        super::finite_operations::FactorBanks {
            gram: &workspace.gram,
            rhs: &workspace.rhs,
            factor: &mut workspace.factor,
            auxiliary: &mut workspace.inverse_lower,
            solve: &mut workspace.solve,
            scores: &mut workspace.scores,
            row: &mut workspace.row,
            permutation: &mut workspace.permutation,
            coefficients: &mut workspace.coefficients,
            inverse: &mut workspace.bread,
            selected: &mut workspace.candidate_positions,
        },
        &mut super::prepared_fit::SystemFitReservation,
        |_, _, _| Ok(()),
    )?;
    #[cfg(test)]
    if result.used_qr {
        FROZEN_QR_FALLBACK_OBSERVED.store(true, Ordering::SeqCst);
    }
    let condition_proxy = result.report.condition_proxy;
    workspace.candidate_positions.clear();
    workspace
        .candidate_positions
        .try_reserve(rank)
        .map_err(|_| allocation("unable to reserve frozen inverse certificate positions"))?;
    workspace.candidate_positions.extend(0..rank);
    let inverse = certify_finite_inverse(
        &workspace.gram,
        rank,
        &workspace.candidate_positions,
        &workspace.bread,
        rank,
    )?;
    let left_inverse_error = inverse.left;
    let right_inverse_error = inverse.right;

    if let Some(tolerance) = config.coefficient_tolerance {
        refine_workspace(
            RefinementFrame {
                storage,
                active_positions,
                reference_norms,
                response: within_response,
                weights,
                weight_scale: bread_weight_scale,
            },
            workspace,
            tolerance,
        )?;
    }
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
    let mut retained_reader = storage.reader();
    for row in 0..rows {
        retained_reader.fill_row(row, &mut workspace.row)?;
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

struct JointResponseChecks {
    reference_norm: f64,
    weight_sum: f64,
    response_mean: Option<f64>,
    reconstruction: LinearReconstructionCertificate,
}

fn check_joint_response(
    response: &[f64],
    residuals: &[f64],
    weights: &[f64],
    needs_mean: bool,
) -> Result<JointResponseChecks> {
    if response.len() != residuals.len() || response.len() != weights.len() {
        return Err(invalid_argument("joint response check dimensions differ"));
    }
    // Each norm and compensated sum preserves its original row order. Only
    // independent work overlaps; reconstruction combines maxima and predicates.
    let (moments, reconstruction) = rayon::join(
        || -> Result<(f64, f64, Option<f64>)> {
            let (mut response_scale, mut residual_scale) = (0., 0.);
            let (mut response_square, mut residual_square) = (1., 1.);
            let (mut weight_sum, mut weight_correction) = (0., 0.);
            let (mut response_sum, mut response_correction) = (0., 0.);
            for ((&value, &residual), &weight) in response.iter().zip(residuals).zip(weights) {
                let root = weight.sqrt();
                if !root.is_finite() || root <= 0. {
                    return Err(invalid_data(
                        "weighted-linear norm weight is not finite and positive",
                    ));
                }
                update_scaled_square(root * value, &mut response_scale, &mut response_square)?;
                update_scaled_square(root * residual, &mut residual_scale, &mut residual_square)?;
                compensated_add(weight, &mut weight_sum, &mut weight_correction)?;
                if needs_mean {
                    compensated_add(weight * value, &mut response_sum, &mut response_correction)?;
                }
            }
            let reference = stable_norm(response_scale, response_square)?
                .max(stable_norm(residual_scale, residual_square)?);
            let weight_sum = weight_sum + weight_correction;
            let mean = needs_mean.then(|| (response_sum + response_correction) / weight_sum);
            if !weight_sum.is_finite()
                || weight_sum <= 0.
                || mean.is_some_and(|value| !value.is_finite())
            {
                return Err(invalid_data("frozen weighted-linear mean is not finite"));
            }
            Ok((reference, weight_sum, mean))
        },
        || {
            response
                .par_chunks(4096)
                .zip(residuals.par_chunks(4096))
                .map(|(response, residuals)| certify_frozen_combined_predictor(response, residuals))
                .try_reduce(LinearReconstructionCertificate::new, |left, right| {
                    Ok(LinearReconstructionCertificate {
                        relative_infinity_norm: left
                            .relative_infinity_norm
                            .max(right.relative_infinity_norm),
                        passed: left.passed && right.passed,
                    })
                })
        },
    );
    let (reference_norm, weight_sum, response_mean) = moments?;
    Ok(JointResponseChecks {
        reference_norm,
        weight_sum,
        response_mean,
        reconstruction: reconstruction?,
    })
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
    crate::memory::Capacity::<f64>::matrix(
        rows as u128,
        columns as u128,
        "weighted-linear dimensions overflow",
    )?
    .len("weighted-linear dimensions overflow")
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

pub(super) fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    if !value.is_finite() || !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "weighted-linear compensated sum input is not finite",
        ));
    }
    neumaier_add(value, sum, correction);
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "weighted-linear compensated sum is not finite",
        ))
    }
}

pub(super) fn update_scaled_square(value: f64, scale: &mut f64, sum: &mut f64) -> Result<()> {
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

pub(super) fn stable_norm(scale: f64, scaled_square_sum: f64) -> Result<f64> {
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

#[cfg(test)]
mod reconstruction_tests {
    use super::{certify_frozen_combined_predictor, LinearReconstructionCertificate};

    #[test]
    fn joint_response_checks_match_separate_fresh_passes_and_reject_invalid_inputs() {
        use super::{check_joint_response, stable_weighted_norm, weighted_mean_slice};
        let n = 12_291;
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for response_scale in [0., 1e-150, 1., 1e150] {
                for weight_scale in [1e-150, 1., 1e150] {
                    let response: Vec<_> = (0..n)
                        .map(|i| ((i % 17) as f64 - 8.) * response_scale)
                        .collect();
                    let residuals: Vec<_> = (0..n)
                        .map(|i| ((i % 13) as f64 - 6.) * response_scale)
                        .collect();
                    let weights: Vec<_> = (0..n)
                        .map(|i| (0.5 + (i % 7) as f64) * weight_scale)
                        .collect();
                    let expected = stable_weighted_norm(n, |i| Ok((response[i], weights[i])))
                        .unwrap()
                        .max(stable_weighted_norm(n, |i| Ok((residuals[i], weights[i]))).unwrap());
                    let scale = super::super::PreparedOlsBreadWeightScale {
                        weight_kind: None,
                        solver_weight_divisor: 1.,
                        reference_weight_multiplier: 1.,
                        reference_weight_sum: n as f64,
                    };
                    for needs_mean in [false, true] {
                        let result = pool
                            .install(|| {
                                check_joint_response(&response, &residuals, &weights, needs_mean)
                            })
                            .unwrap();
                        assert_eq!(result.reference_norm.to_bits(), expected.to_bits());
                        assert_eq!(
                            result.reconstruction,
                            certify_frozen_combined_predictor(&response, &residuals).unwrap()
                        );
                        assert_eq!(
                            result.response_mean,
                            needs_mean.then(|| weighted_mean_slice(
                                &response,
                                crate::fe::BorrowedMapWeights::Provided(&weights),
                                scale,
                            )
                            .unwrap())
                        );
                    }
                }
            }
        }
        for bad in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(check_joint_response(&[1.], &[0.], &[bad], true).is_err());
        }
        assert!(check_joint_response(&[f64::NAN], &[0.], &[1.], false).is_err());
        assert!(check_joint_response(&[1.], &[f64::INFINITY], &[1.], false).is_err());
        assert!(check_joint_response(&[f64::MAX], &[0.], &[4.], true).is_err());
        assert!(check_joint_response(&[1.], &[], &[1.], false).is_err());
    }

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

#[cfg(test)]
mod inference_scan_tests {
    use super::*;
    use crate::sandwich::{
        reduce_dense_score_meats, FixedBlockReductionPolicy, RowMajorSandwichDesign,
        SliceSandwichScoreMultiplier, SystemSandwichReservation,
    };

    #[test]
    fn immutable_inference_scan_preserves_bits_boundaries_and_new_writes() {
        for columns in [1, 6, 1025] {
            let n = 37;
            let mut values: Vec<_> = (0..n * columns)
                .map(|i| {
                    if i % 19 == 0 {
                        -0.0
                    } else {
                        (i as f64 - 700.) / 17.
                    }
                })
                .collect();
            let mut scratch = PreparedOlsScratch::create_row_major(n, columns).unwrap();
            scratch.write_linear(0, &values).unwrap();
            let mut storage = RetainedWithinStorage::Scratch(scratch);
            let positions: Vec<_> = (0..columns).collect();
            let design = CertifiedLinearWithinDesign {
                storage: &storage,
                active_positions: &positions,
            };
            let mut output = vec![0.; columns];
            let mut visits = 0;
            design
                .visit_range(1..n - 1, &mut output, |row, output| {
                    assert!(output
                        .iter()
                        .zip(&values[row * columns..])
                        .all(|(a, b)| a.to_bits() == b.to_bits()));
                    output.fill(f64::NAN);
                    visits += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(visits, n - 2);
            let RetainedWithinStorage::Scratch(scratch) = &storage else {
                unreachable!()
            };
            assert_eq!(
                scratch.shared_read_chunks_for_test() as usize,
                ((n - 1) * columns).div_ceil(1024) - columns / 1024
            );
            design
                .visit_range(n..n, &mut output, |_, _| panic!("empty range"))
                .unwrap();
            assert!(design
                .visit_range(0..n + 1, &mut output, |_, _| Ok(()))
                .is_err());
            assert!(design.visit_range(0..1, &mut [], |_, _| Ok(())).is_err());
            let mut visits = 0;
            let error = design
                .visit_range(0..n, &mut output, |row, _| {
                    visits += 1;
                    if row == 3 {
                        Err(FeregError::new(
                            ErrorCode::Interrupted,
                            "scan callback failure",
                        ))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::Interrupted);
            assert_eq!(visits, 4);
            let RetainedWithinStorage::Scratch(scratch) = &mut storage else {
                unreachable!()
            };
            values[0] = 41.;
            scratch.write_linear(0, &[values[0]]).unwrap();
            CertifiedLinearWithinDesign {
                storage: &storage,
                active_positions: &positions,
            }
            .visit_range(0..1, &mut output, |_, row| {
                assert_eq!(row[0], 41.);
                Ok(())
            })
            .unwrap();
            let bad: Vec<_> = (0..columns).rev().collect();
            if columns > 1 {
                assert!(CertifiedLinearWithinDesign {
                    storage: &storage,
                    active_positions: &bad
                }
                .visit_range(0..1, &mut output, |_, _| Ok(()))
                .is_err());
            }
        }
    }

    #[test]
    fn inference_scan_preserves_both_meats_across_weights_blocks_and_teams() {
        let n = 2035;
        for k in [1, 6, 17] {
            let values: Vec<_> = (0..n * k).map(|i| (i as f64 / 19.).sin()).collect();
            let mut scratch = PreparedOlsScratch::create_row_major(n, k).unwrap();
            scratch.write_linear(0, &values).unwrap();
            let storage = RetainedWithinStorage::Scratch(scratch);
            let positions: Vec<_> = (0..k).collect();
            let design = CertifiedLinearWithinDesign {
                storage: &storage,
                active_positions: &positions,
            };
            let dense = RowMajorSandwichDesign::new(&values, n, k).unwrap();
            let production: Vec<_> = (0..n)
                .map(|i| (0.2 + (i % 13) as f64) * (i as f64 / 17.).cos())
                .collect();
            let certificate: Vec<_> = production.iter().map(|x| x * 0.999).collect();
            let scores = SliceSandwichScoreMultiplier::new(&production, &certificate).unwrap();
            for threads in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                for block in [511, 16_384] {
                    let policy = FixedBlockReductionPolicy {
                        rows_per_block: block,
                        maximum_blocks: 64,
                        partial_budget_bytes: 32 << 20,
                    };
                    let expected = pool
                        .install(|| {
                            reduce_dense_score_meats(
                                dense,
                                scores,
                                policy,
                                None,
                                &mut SystemSandwichReservation,
                            )
                        })
                        .unwrap();
                    let RetainedWithinStorage::Scratch(scratch) = &storage else {
                        unreachable!()
                    };
                    let before = scratch.shared_read_chunks_for_test();
                    let actual = pool
                        .install(|| {
                            reduce_dense_score_meats(
                                design,
                                scores,
                                policy,
                                None,
                                &mut SystemSandwichReservation,
                            )
                        })
                        .unwrap();
                    assert_eq!(actual.production, expected.production);
                    assert_eq!(actual.certificate, expected.certificate);
                    assert!(
                        scratch.shared_read_chunks_for_test() - before
                            <= (n * k).div_ceil(1024) as u64 + n.div_ceil(block) as u64
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod checkpoint_report_tests {
    use super::*;

    fn report(solver: FeSolverKind) -> SolveReport {
        SolveReport {
            requested_solver: solver,
            used_solver: solver,
            iterations: if solver == FeSolverKind::Map { 0 } else { 12 },
            checkpoint_retries: 0,
            solver_workspace_peak: crate::memory::SolverWorkspacePeak::ZERO,
            stop_reason: crate::fe::SolveStopReason::Certified,
            certificate: crate::fe::FeCertificate {
                absolute_infinity_norm: 1e-12,
                relative_infinity_norm: 1e-12,
                relative_two_norm: 1e-12,
                passed: true,
            },
            normal_equation_residual: 0.,
            setup_seconds: 1.,
            solve_seconds: 2.,
            certification_seconds: 3.,
            fallback_used: false,
            requested_preconditioner: None,
            used_preconditioner: None,
            pilot_iterations: 0,
            pilot_contraction: 0.,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn ppml_canonical_report_merge_preserves_work_and_fallback_in_any_order() {
        let zero = report(FeSolverKind::Map);
        let mut lsmr = report(FeSolverKind::Lsmr);
        lsmr.requested_preconditioner = Some(crate::fe::LsmrPreconditioner::Auto);
        lsmr.used_preconditioner = Some(crate::fe::LsmrPreconditioner::Schwarz);
        for (mut a, b) in [(zero.clone(), lsmr.clone()), (lsmr.clone(), zero.clone())] {
            merge_canonical_projection_reports(&mut a, b).unwrap();
            assert_eq!(a.used_solver, FeSolverKind::Lsmr);
            assert_eq!(a.used_preconditioner, lsmr.used_preconditioner);
            assert_eq!(a.certification_seconds, 6.);
            assert_eq!(a.iterations, 12);
        }
        let mut schur = report(FeSolverKind::Lsmr);
        schur.used_solver = FeSolverKind::Map;
        for reports in [
            [zero.clone(), lsmr.clone(), schur.clone()],
            [lsmr.clone(), schur.clone(), zero.clone()],
            [schur.clone(), zero.clone(), lsmr.clone()],
            [zero.clone(), schur.clone(), lsmr.clone()],
            [lsmr.clone(), zero.clone(), schur.clone()],
            [schur, lsmr.clone(), zero.clone()],
        ] {
            let mut merged = reports[0].clone();
            for source in &reports[1..] {
                merge_canonical_projection_reports(&mut merged, source.clone()).unwrap();
            }
            assert!(merged.certified() && !merged.fallback_used);
            assert_eq!(merged.requested_solver, FeSolverKind::Lsmr);
            assert_eq!(merged.used_solver, FeSolverKind::Map);
            assert_eq!(merged.used_preconditioner, None);
            assert_eq!(merged.checkpoint_retries, 0);
            assert_eq!(merged.certification_seconds, 9.);
            assert_eq!(merged.iterations, 12);
        }
        let mut fallback = report(FeSolverKind::Lsmr);
        fallback.used_solver = FeSolverKind::Map;
        fallback.fallback_used = true;
        fallback.checkpoint_retries = 1;
        fallback.warnings.push("certified fallback".to_owned());
        let reports = [zero, lsmr, fallback];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut merged = reports[order[0]].clone();
            for i in &order[1..] {
                merge_canonical_projection_reports(&mut merged, reports[*i].clone()).unwrap();
            }
            assert!(merged.certified() && merged.fallback_used);
            assert_eq!(merged.requested_solver, FeSolverKind::Lsmr);
            assert_eq!(merged.used_solver, FeSolverKind::Map);
            assert_eq!(merged.checkpoint_retries, 1);
            assert_eq!(merged.setup_seconds, 3.);
            assert_eq!(merged.solve_seconds, 6.);
            assert_eq!(merged.certification_seconds, 9.);
            assert_eq!(merged.warnings, vec!["certified fallback"]);
        }
        let mut invalid = report(FeSolverKind::Map);
        invalid.iterations = 1;
        assert!(merge_canonical_projection_reports(&mut invalid, reports[1].clone()).is_err());
        let mut invalid = report(FeSolverKind::Map);
        invalid.certificate.passed = false;
        assert!(merge_canonical_projection_reports(&mut invalid, reports[1].clone()).is_err());
    }
}
