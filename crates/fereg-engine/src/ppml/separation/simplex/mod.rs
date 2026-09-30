//! Full-model phase-I proposals, checked independently before sample authority.
mod capacity;
mod detect;
mod dual;
mod model;
mod nullspace;
mod tableau;
use super::{
    invalid, DirectionProposal, IrReport, IrTerminal, Ledger, Method, Model, RunControls, Status,
    View,
};
use crate::{
    error::{ErrorCode, FeregError, Result},
    ppml::{Interrupt, InterruptPoint},
    spec::Solver,
    threads::ThreadTeam,
};
pub use capacity::{simplex_kernel_cells, SimplexCapacity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimplexLimits {
    pub max_pivots: usize,
    pub max_cells: usize,
}
impl Default for SimplexLimits {
    fn default() -> Self {
        Self {
            max_pivots: 10_000,
            max_cells: 8_000_000,
        }
    }
}
impl SimplexLimits {
    pub(super) fn validate(self) -> Result<()> {
        if self.max_pivots == 0 || self.max_cells == 0 {
            return Err(invalid("simplex pivot and cell limits must be positive"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimplexDualCertificate {
    pub boundary_rows: usize,
    pub nullity: usize,
    pub stationarity_relative: f64,
    pub inequality_violation_relative: f64,
    pub infeasibility_margin_relative: f64,
    pub scale: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub enum SimplexEvidence {
    FullPositiveRank {
        columns: usize,
    },
    DualInfeasibility {
        dual: Vec<f64>,
        certificate: SimplexDualCertificate,
    },
}
#[derive(Debug, Clone, PartialEq)]
pub struct SimplexReport {
    pub generation: usize,
    pub sample_identity: u64,
    pub solver: Solver,
    pub controls: super::Controls,
    pub limits: SimplexLimits,
    pub capacity: SimplexCapacity,
    pub span_rank: Option<usize>,
    pub pivots: usize,
    pub status: Status,
    pub evidence: Option<SimplexEvidence>,
    pub direction: Option<DirectionProposal>,
}
/// A sealed terminal is constructed only by fresh replay or same-model IR
/// resolution. A capacity-skip report has no terminal authority by itself.
#[derive(Debug, PartialEq)]
pub struct SimplexTerminal {
    generation: usize,
    status: Status,
    capacity: SimplexCapacity,
    pivots: usize,
}
impl SimplexTerminal {
    #[must_use]
    pub const fn generation(&self) -> usize {
        self.generation
    }
    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }
    #[must_use]
    pub const fn capacity(&self) -> SimplexCapacity {
        self.capacity
    }
    #[must_use]
    pub const fn pivots(&self) -> usize {
        self.pivots
    }
}
impl View<'_> {
    pub fn simplex_capacity(self, model: &Model<'_>) -> Result<SimplexCapacity> {
        let design = model::design(self, model)?;
        capacity::estimate(self, &design, model.raw.ncols())
    }
    pub fn model_simplex(self, model: &Model<'_>, limits: SimplexLimits) -> Result<SimplexReport> {
        self.model_simplex_with(model, limits, &mut |_| Ok(()))
    }
    pub fn model_simplex_with(
        self,
        model: &Model<'_>,
        limits: SimplexLimits,
        interrupt: &mut Interrupt<'_>,
    ) -> Result<SimplexReport> {
        limits.validate()?;
        interrupt(InterruptPoint::SimplexModel)?;
        let design = model::design(self, model)?;
        let capacity = capacity::estimate(self, &design, model.raw.ncols())?;
        let mut report = SimplexReport {
            generation: self.generation(),
            sample_identity: self.numerical_identity(),
            solver: model.options.solver,
            controls: model.controls,
            limits,
            capacity,
            span_rank: None,
            pivots: 0,
            status: Status::NoCandidates,
            evidence: None,
            direction: None,
        };
        if capacity.explicit_width == 0 || !self.sample().outcome.contains(&0.) {
            return Ok(report);
        }
        if capacity.required_cells > limits.max_cells {
            report.status = Status::SkippedCapacity;
            return Ok(report);
        }
        let matrix = model::explicit(&design, model, interrupt)?;
        drop(design);
        let matrix = model::span(
            matrix,
            model.controls.projection_tolerance,
            interrupt,
            model.probe.dense(),
        )?;
        report.span_rank = Some(matrix.ncols());
        detect::run(self, model, &matrix, report, interrupt)
    }
    pub fn replay_simplex(
        self,
        observed: &SimplexReport,
        model: &Model<'_>,
        limits: SimplexLimits,
    ) -> Result<SimplexTerminal> {
        self.replay_simplex_with(observed, model, limits, &mut |_| Ok(()))
    }
    pub fn replay_simplex_with(
        self,
        observed: &SimplexReport,
        model: &Model<'_>,
        limits: SimplexLimits,
        interrupt: &mut Interrupt<'_>,
    ) -> Result<SimplexTerminal> {
        binding(self, observed, limits)?;
        if observed.direction.is_some()
            || !matches!(observed.status, Status::NoCandidates | Status::NoSeparation)
        {
            return Err(failed(
                "simplex replay requires a no-drop result, not a capacity skip",
            ));
        }
        let fresh = self.model_simplex_with(model, limits, interrupt)?;
        if &fresh != observed {
            return Err(failed("simplex terminal evidence failed fresh replay"));
        }
        Ok(seal(&fresh))
    }
}
impl Ledger {
    pub fn resolve_simplex_skip(
        &self,
        observed: &SimplexReport,
        ir: &IrReport,
        model: &Model<'_>,
        limits: SimplexLimits,
        ir_budget: usize,
        team: &ThreadTeam,
    ) -> Result<(SimplexTerminal, IrTerminal)> {
        self.resolve_simplex_skip_with(
            observed,
            ir,
            model,
            RunControls {
                simplex: limits,
                ir_iterations: ir_budget,
                ..RunControls::default()
            },
            team,
            &mut |_| Ok(()),
        )
    }
    pub fn resolve_simplex_skip_with(
        &self,
        observed: &SimplexReport,
        ir: &IrReport,
        model: &Model<'_>,
        controls: RunControls,
        team: &ThreadTeam,
        interrupt: &mut Interrupt<'_>,
    ) -> Result<(SimplexTerminal, IrTerminal)> {
        let limits = controls.simplex;
        let ir_budget = controls.ir_iterations;
        binding(self.view(), observed, limits)?;
        if !self.policy.methods.contains(Method::Simplex)
            || !self.policy.methods.contains(Method::IteratedRectifier)
            || observed.status != Status::SkippedCapacity
            || ir.status != Status::NoSeparation
        {
            return Err(FeregError::new(ErrorCode::Allocation,
                "simplex capacity skip requires requested comprehensive IR to close the same generation"));
        }
        let fresh = self.view().model_simplex_with(model, limits, interrupt)?;
        if &fresh != observed {
            return Err(invalid(
                "simplex capacity evidence differs from the current model",
            ));
        }
        // Replay the actual current model, rather than accepting a terminal
        // seal produced for another raw/FE span with the same row identities.
        let terminal = self
            .view()
            .replay_ir_with(ir, model, ir_budget, team, interrupt)?;
        Ok((seal(&fresh), terminal))
    }
}
fn binding(view: View<'_>, observed: &SimplexReport, limits: SimplexLimits) -> Result<()> {
    if observed.generation != view.generation()
        || observed.sample_identity != view.numerical_identity()
        || observed.limits != limits
    {
        return Err(invalid(
            "simplex report belongs to a different generation or budget",
        ));
    }
    Ok(())
}
fn seal(report: &SimplexReport) -> SimplexTerminal {
    SimplexTerminal {
        generation: report.generation,
        status: report.status,
        capacity: report.capacity,
        pivots: report.pivots,
    }
}
fn reserve<T>(target: &mut Vec<T>, additional: usize, label: &'static str) -> Result<()> {
    target
        .try_reserve_exact(additional)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, label))
}
fn filled<T: Clone>(length: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut result = Vec::new();
    reserve(&mut result, length, label)?;
    result.resize(length, value);
    Ok(result)
}
fn overflow() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "full-model simplex capacity dimensions overflow",
    )
}
fn failed(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
