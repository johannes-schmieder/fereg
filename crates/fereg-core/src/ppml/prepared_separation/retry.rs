//! Re-execution of a detector step whose admitted projection failed.
//!
//! A step admitted on the two-FE Auto route ([`super::routing`]) that fails a
//! numerical certificate first runs again with retained-product CG, the
//! detector's established solver, under the same weights, tolerances,
//! iteration limit, rules and certificates; from there it continues exactly
//! as a retained-CG step (below). The Auto attempt takes no MAP continuation
//! (its `AbsorbConfig::certified_fallback` is off), so a two-FE PCG
//! certificate miss reaches retained CG at once. The
//! re-executed step reproduces the retained-CG step bit for bit, and when it
//! fails too, its error (not the Auto attempt's) is reported.
//!
//! The public detector solves its weighted finite-plus-FE projections with
//! retained-product CG on the normal equations. The rectifier weights zero
//! outcomes by `tolerance^2 / zeros` (about 1e-18 at `septolerance(1e-8)`), so
//! the normal operator's spectrum spans the square of that ratio. On weakly
//! connected or heavily zero designs the components that only zero rows
//! determine are then lost to rounding: the returned fitted values on zero
//! rows leave the model span, and the deletion certificate's independent
//! unit-weight span check rejects the proposed direction. A span check of an
//! exact (simplex) direction can also fail its finite score certificate when
//! the direction lies in the FE span, because that certificate is normalized
//! by the FE solver's own residual noise.
//!
//! When such a step fails a numerical certificate, it is executed once more
//! with matrix-free LSMR (the public `lsmr` FE solver family and its
//! preconditioner policy), which works on the weighted least-squares operator
//! itself instead of its square. The detector's weights, tolerances,
//! iteration limit, sign and threshold rules and every certificate are
//! unchanged, and the re-executed step must pass the same certificates. A step
//! that certifies on its first attempt is never re-executed, so fits that
//! complete without this path are unchanged bit for bit. If the LSMR workspace
//! does not fit the memory plan, or the re-execution fails a numerical
//! certificate as well, the first error is reported unchanged; Break and
//! allocation failures of the re-execution propagate.
use std::cell::Cell;

use crate::error::{allocation, ErrorCode, FeregError, Result};
use crate::fe::{FeSolverKind, LsmrPreconditioner};
use crate::memory::{FeSolverMemoryBudget, FeSolverMemoryEstimate};
use crate::ols::PreparedOlsFitConfig;
use crate::ppml::{PpmlConfig, PpmlMemoryPlan};
use crate::ppml_separation::{MethodStatus, SeparationConfig};

use super::super::prepared_estimator::PpmlInterruptPoint;
use super::super::prepared_irls::PreparedPpmlMuExploration;
use super::super::prepared_model::PreparedPpmlModelGeneration;
use super::{
    bind_numerical_proposal, certify_numerical_proposal, fail_closed_comprehensive,
    numerical_drop_reason, prepared_bound_iterated_rectifier, prepared_mu_from_mean, trace,
    validate_numerical_proposal, PreparedNumericalProposal, ValidatedNumericalProposal,
};

/// LSMR projection workspace resolved on generation zero beside the
/// retained-CG charge; later generations only shrink. It is charged to the
/// memory plan only when a re-execution is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LsmrAdmission {
    /// Execution bytes below the detector projection (the plan before its
    /// separation projection charge).
    base: u128,
    bytes: u128,
}

/// The re-execution charge for an admitted retained-CG detector (or an Auto
/// detector, whose re-execution is retained CG): the CG charge with its CG
/// workspace replaced by LSMR's, and at least the graph proposal workspace
/// the CG budget admitted.
pub(super) fn lsmr_admission(
    solver: FeSolverKind,
    estimate: &FeSolverMemoryEstimate,
    preconditioner: LsmrPreconditioner,
    rows: usize,
    graph_bytes: u128,
    base: u128,
) -> Result<Option<LsmrAdmission>> {
    if !matches!(solver, FeSolverKind::Cg | FeSolverKind::Auto) {
        return Ok(None);
    }
    let bytes = estimate
        .coefficient_batch_bytes
        .checked_mul(5)
        .and_then(|n| n.checked_add(estimate.lsmr_bytes(preconditioner)))
        .and_then(|n| {
            estimate
                .rhs_copy_bytes
                .checked_mul(2)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            (rows as u128)
                .checked_mul(16)
                .and_then(|m| n.checked_add(m))
        })
        .ok_or_else(|| allocation("separation LSMR projection memory overflows"))?
        .max(graph_bytes);
    Ok(Some(LsmrAdmission { base, bytes }))
}

/// The admitted detector projection policy and its optional re-executions.
pub(super) struct DetectorPolicy<'a> {
    ppml: PpmlConfig,
    linear: PreparedOlsFitConfig,
    /// The retained-CG policy of an admitted Auto detector, under the same
    /// admitted budget; `None` when the admitted policy is retained CG.
    retained_cg: Option<(PpmlConfig, PreparedOlsFitConfig)>,
    /// The LSMR charge and the fit's live plan it is admitted into.
    lsmr: Option<(LsmrAdmission, &'a Cell<PpmlMemoryPlan>)>,
}

impl<'a> DetectorPolicy<'a> {
    pub(super) const fn new(
        ppml: PpmlConfig,
        linear: PreparedOlsFitConfig,
        retained_cg: Option<(PpmlConfig, PreparedOlsFitConfig)>,
        lsmr: Option<(LsmrAdmission, &'a Cell<PpmlMemoryPlan>)>,
    ) -> Self {
        Self {
            ppml,
            linear,
            retained_cg,
            lsmr,
        }
    }

    /// Run one detector step with the admitted policy and, after a numerical
    /// certificate failure, once more with retained CG when the admitted
    /// policy is Auto, then once more with LSMR. Returns the step's result and
    /// the policy that produced it (a terminal result's replay must re-execute
    /// with that same policy).
    pub(super) fn run<T>(
        &self,
        generation: usize,
        mut step: impl FnMut(PpmlConfig, PreparedOlsFitConfig) -> Result<T>,
    ) -> Result<(T, PpmlConfig, PreparedOlsFitConfig)> {
        let mut first = match step(self.ppml, self.linear) {
            Ok(value) => return Ok((value, self.ppml, self.linear)),
            Err(error) => error,
        };
        if !numerical(&first) {
            return Err(first);
        }
        if let Some((ppml, linear)) = self.retained_cg {
            let retried = step(ppml, linear);
            if crate::diagnostics::phase_timing_enabled() {
                eprintln!(
                    "FEREG_PHASE|separation_projection_retry|generation={generation}|solver=cg|first={:?}|succeeded={}",
                    first.code,
                    u8::from(retried.is_ok()),
                );
            }
            // From here the step is exactly a retained-CG step.
            first = match retried {
                Ok(value) => return Ok((value, ppml, linear)),
                Err(error) if numerical(&error) => error,
                Err(error) => return Err(error),
            };
        }
        let Some((ppml, linear)) = self.admit_lsmr()? else {
            return Err(first);
        };
        let retried = step(ppml, linear);
        if crate::diagnostics::phase_timing_enabled() {
            eprintln!(
                "FEREG_PHASE|separation_projection_retry|generation={generation}|solver=lsmr|first={:?}|succeeded={}",
                first.code,
                u8::from(retried.is_ok()),
            );
        }
        match retried {
            Ok(value) => Ok((value, ppml, linear)),
            Err(error) if numerical(&error) => Err(first),
            Err(error) => Err(error),
        }
    }

    /// Charge the LSMR workspace to the plan and return its policy, or `None`
    /// when no re-execution was resolved or it does not fit the plan.
    fn admit_lsmr(&self) -> Result<Option<(PpmlConfig, PreparedOlsFitConfig)>> {
        let Some((admission, live)) = self.lsmr else {
            return Ok(None);
        };
        let mut plan = live.get();
        if !plan.admit_separation_projection(plan.separation_projection_bytes.max(admission.bytes))
        {
            return Ok(None);
        }
        live.set(plan);
        let mut linear = self.retained_cg.map_or(self.linear, |(_, linear)| linear);
        linear.absorb.solver = FeSolverKind::Lsmr;
        linear.absorb.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: admission.base,
            usable_bytes: admission
                .base
                .checked_add(admission.bytes)
                .ok_or_else(|| allocation("separation LSMR projection budget overflows"))?,
        });
        let mut ppml = self.retained_cg.map_or(self.ppml, |(ppml, _)| ppml);
        ppml.absorb = linear.absorb;
        Ok(Some((ppml, linear)))
    }
}

impl DetectorPolicy<'_> {
    /// The admitted first-attempt policy.
    pub(super) const fn admitted(&self) -> (PpmlConfig, PreparedOlsFitConfig) {
        (self.ppml, self.linear)
    }

    /// Certify a deletion proposal, re-executing a failed certification with
    /// LSMR (the proposal itself is borrowed by both attempts).
    pub(super) fn validate(
        &self,
        generation: &PreparedPpmlModelGeneration,
        proposal: PreparedNumericalProposal,
        config: SeparationConfig,
    ) -> Result<ValidatedNumericalProposal> {
        let (certificate, _, _) = self.run(generation.generation(), |ppml, linear| {
            certify_numerical_proposal(generation, &proposal, ppml, config, linear)
        })?;
        Ok(ValidatedNumericalProposal {
            reason: numerical_drop_reason(proposal.method)?,
            proposal,
            certificate,
        })
    }
}

/// One re-executable numerical detector step: a validated deletion, or the
/// terminal no-drop proposal that the caller replays.
pub(super) enum Detected {
    Deletion(ValidatedNumericalProposal),
    Terminal(PreparedNumericalProposal),
}

/// The rectifier step: detect (graph proposal or projected recurrence),
/// require a comprehensive result, and certify a deletion.
pub(super) fn ir_step(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    graph_admitted: bool,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    lap: &mut trace::IrPhase,
) -> Result<Detected> {
    let proposal = prepared_bound_iterated_rectifier(
        generation,
        ppml,
        config,
        linear,
        graph_admitted,
        interrupt,
    )?;
    lap.detected(&proposal);
    fail_closed_comprehensive(&proposal)?;
    if proposal.dropped_original_rows.is_empty() {
        return Ok(Detected::Terminal(proposal));
    }
    let certified = validate_numerical_proposal(generation, proposal, ppml, config, linear)?;
    lap.validated();
    Ok(Detected::Deletion(certified))
}

/// The Mu step on a fitted exploration: propose from the fitted mean,
/// require a certified result, and certify a deletion.
pub(super) fn mu_step(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    exploration: &PreparedPpmlMuExploration,
) -> Result<Detected> {
    let proposal = bind_numerical_proposal(
        generation,
        prepared_mu_from_mean(
            generation,
            ppml,
            config,
            linear,
            exploration.mean(),
            exploration.certificate().accepted_iterations,
        )?,
    )?;
    if matches!(
        proposal.status,
        MethodStatus::MaximumIterations | MethodStatus::Uncertified
    ) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "prepared mu separation did not produce a certified result",
        ));
    }
    if proposal.dropped_original_rows.is_empty() {
        return Ok(Detected::Terminal(proposal));
    }
    Ok(Detected::Deletion(validate_numerical_proposal(
        generation, proposal, ppml, config, linear,
    )?))
}

/// A failed projection, score, FE or sign certificate, or a solver that did
/// not converge. Break, allocation and internal errors are not retried.
fn numerical(error: &FeregError) -> bool {
    matches!(
        error.code,
        ErrorCode::NonConvergence | ErrorCode::InvalidData
    )
}

#[cfg(test)]
mod tests;
