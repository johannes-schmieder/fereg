//! Trace-only rectifier laps (`FEREG_PHASE_TIMING`).
//!
//! Disabled, every ledger is a branch on the timing switch: no clock reads,
//! no allocation and no effect on any projection, decision or report.
use crate::diagnostics::phase_ledger::PhaseLedger;
use crate::fe::SolveReport;
use crate::ols::CertifiedPreparedLinearProjection;

/// One rectifier iteration: projection seconds, then the decision and
/// recurrence update. Emitted as `FEREG_PHASE|separation_ir_iteration`.
pub(super) struct IrIteration(PhaseLedger<2>);

/// Terminal outcome of one rectifier iteration.
pub(super) enum IrStep {
    /// Certified fixed point (no separation).
    Certified,
    /// Strict-negative boundary rows proposed for deletion.
    Proposed(usize),
    /// Recurrence update; the count is the rectified boundary support.
    Continued(usize),
}

impl IrIteration {
    pub(super) fn new(generation: usize, iteration: usize, boundary: usize) -> Self {
        let mut ledger = PhaseLedger::new("separation_ir_iteration", ["projection", "decision"]);
        ledger.facts(&[
            ("generation", generation as u64),
            ("iteration", iteration as u64),
            ("boundary", boundary as u64),
        ]);
        Self(ledger)
    }

    pub(super) fn projected(&self) {
        self.0.lap(0);
    }

    pub(super) fn decided(&mut self, step: IrStep) {
        self.0.lap(1);
        let (outcome, flagged) = match step {
            IrStep::Certified => (1, 0),
            IrStep::Proposed(rows) => (2, rows),
            IrStep::Continued(support) => (0, support),
        };
        self.0
            .facts(&[("outcome", outcome), ("flagged", flagged as u64)]);
    }
}

/// One weighted detector projection (rectifier, Mu or a direction's span
/// check). Emitted as `FEREG_PHASE|separation_projection`.
pub(super) struct Projection(PhaseLedger<1>);

impl Projection {
    pub(super) fn new(generation: usize, rows: usize, rhs: usize) -> Self {
        let mut ledger = PhaseLedger::new("separation_projection", ["fit"]);
        ledger.facts(&[
            ("generation", generation as u64),
            ("rows", rows as u64),
            ("rhs", rhs as u64),
        ]);
        Self(ledger)
    }

    pub(super) fn fitted(&mut self, fit: &CertifiedPreparedLinearProjection) {
        self.0.lap(0);
        let facts = |initial: &SolveReport, residual: &SolveReport| {
            [
                ("initial_iterations", initial.iterations as u64),
                ("initial_pilot", initial.pilot_iterations as u64),
                ("initial_solver", solver_code(initial.used_solver)),
                ("residual_iterations", residual.iterations as u64),
                ("residual_solver", solver_code(residual.used_solver)),
                ("certified", u64::from(fit.certified())),
            ]
        };
        self.0.facts(&facts(
            fit.absorption_report(),
            fit.residual_absorption_report(),
        ));
    }
}

const fn solver_code(solver: crate::fe::FeSolverKind) -> u64 {
    use crate::fe::FeSolverKind as S;
    match solver {
        S::Auto => 0,
        S::Map => 1,
        S::Lsmr => 2,
        S::Cmg => 3,
        S::CmgSchur => 4,
        S::Cg => 5,
        S::IronsTuck => 6,
        S::Squarem => 7,
    }
}

/// Rectified boundary support, counted only while timing is enabled.
pub(super) fn support(recurrence: &[f64]) -> usize {
    if crate::diagnostics::phase_timing_enabled() {
        recurrence.iter().filter(|value| **value > 0.0).count()
    } else {
        0
    }
}

/// One execution of the rectifier phase on a generation: detector (graph
/// proposal or projected recurrence), then either deletion validation and
/// commit or the terminal no-drop replay. `FEREG_PHASE|separation_ir_phase`.
pub(super) struct IrPhase(PhaseLedger<4>);

impl IrPhase {
    pub(super) fn new(generation: usize) -> Self {
        let mut ledger = PhaseLedger::new(
            "separation_ir_phase",
            ["detect", "validate", "commit", "replay"],
        );
        ledger.fact("generation", generation as u64);
        Self(ledger)
    }

    pub(super) fn detected(&mut self, proposal: &super::PreparedNumericalProposal) {
        self.0.lap(0);
        self.0.facts(&[
            ("graph", u64::from(proposal.exact_intercept_span.is_some())),
            ("iterations", proposal.iterations as u64),
            ("dropped", proposal.dropped_original_rows.len() as u64),
        ]);
    }

    pub(super) fn validated(&self) {
        self.0.lap(1);
    }

    pub(super) fn committed(&self) {
        self.0.lap(2);
    }

    pub(super) fn replayed(&self) {
        self.0.lap(3);
    }
}
