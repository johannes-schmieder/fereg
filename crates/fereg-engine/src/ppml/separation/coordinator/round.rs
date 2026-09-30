use super::{Report, RunControls};
use crate::{
    data::{DenseMatrix, Weights},
    error::{ErrorCode, FeregError, Result},
    fe::{Design, Plan},
    ppml::{
        separation::{Ledger, Method, Model, SimplexReport, Status},
        Interrupt, InterruptPoint,
    },
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::{Role, Work},
};

pub(super) fn exact(ledger: &mut Ledger, report: &mut Report) -> Result<bool> {
    if !ledger.policy.methods.contains(Method::FixedEffects) {
        return Ok(false);
    }
    let proposal = ledger.view().exact_proposal()?;
    report.detector(
        Method::FixedEffects,
        proposal.status,
        proposal.iterations,
        Work::default(),
    )?;
    if let Some(commit) = ledger.accept_exact(&proposal)? {
        report.commit(Some(Method::FixedEffects), commit)?;
        return Ok(true);
    }
    report.terminal(Method::FixedEffects, proposal.status)?;
    Ok(false)
}

pub(super) struct Runtime<'a, 'callback> {
    pub options: Options,
    pub controls: RunControls,
    pub team: &'a ThreadTeam,
    pub probe: crate::execution::Probe<'a>,
    pub interrupt: &'a mut Interrupt<'callback>,
}
impl Runtime<'_, '_> {
    pub(super) fn run(
        &mut self,
        ledger: &mut Ledger,
        raw: &DenseMatrix,
        roles: &[Role],
        report: &mut Report,
    ) -> Result<bool> {
        let options = self.options;
        let controls = self.controls;
        let team = self.team;
        let model = Model {
            probe: self.probe,
            raw,
            roles,
            options,
            controls: controls.numerical,
        };
        model.validate(ledger.view())?;
        let design = Design::new(ledger.view().sample(), Weights::Unit)?;
        Plan::new(&design, Estimator::Ppml, &options)?;
        drop(design);
        let mut skipped = None;
        if ledger.policy.methods.contains(Method::Simplex) {
            let observed =
                ledger
                    .view()
                    .model_simplex_with(&model, controls.simplex, self.interrupt)?;
            report.detector(
                Method::Simplex,
                observed.status,
                observed.pivots,
                Work::default(),
            )?;
            if let Some(direction) = &observed.direction {
                (self.interrupt)(InterruptPoint::LinearFrame)?;
                accept(ledger, direction, &model, report, team)?;
                return Ok(true);
            }
            if observed.status == Status::SkippedCapacity {
                if !ledger.policy.methods.contains(Method::IteratedRectifier) {
                    return Err(FeregError::new(
                        ErrorCode::Allocation,
                        "simplex capacity exceeded; requested comprehensive IR is required",
                    ));
                }
                skipped = Some(observed);
            } else {
                let terminal = ledger.view().replay_simplex_with(
                    &observed,
                    &model,
                    controls.simplex,
                    self.interrupt,
                )?;
                report.replay(
                    Method::Simplex,
                    terminal.status(),
                    terminal.pivots(),
                    Work::default(),
                )?;
                report.terminal(Method::Simplex, terminal.status())?;
            }
        }
        if ledger.policy.methods.contains(Method::IteratedRectifier) {
            let observed = ledger
                .view()
                .iterated_rectifier_with(&model, controls.ir_iterations, team, self.interrupt)
                .map_err(|e| {
                    context(e, Method::IteratedRectifier, "detector", report.generation)
                })?;
            report.detector(
                Method::IteratedRectifier,
                observed.status,
                observed.iterations,
                observed.work,
            )?;
            if let Some(direction) = &observed.direction {
                (self.interrupt)(InterruptPoint::LinearFrame)?;
                accept(ledger, direction, &model, report, team)?;
                return Ok(true);
            }
            let terminal = match skipped.as_ref() {
                Some(simplex) => self.resolve_skip(ledger, simplex, &observed, &model, report)?,
                None => ledger.view().replay_ir_with(
                    &observed,
                    &model,
                    controls.ir_iterations,
                    team,
                    self.interrupt,
                )?,
            };
            report.replay(
                Method::IteratedRectifier,
                terminal.status(),
                terminal.iterations(),
                terminal.work(),
            )?;
            report.terminal(Method::IteratedRectifier, terminal.status())?;
        }
        if ledger.policy.methods.contains(Method::Mu) {
            let observed = ledger.view().fitted_mu_with(
                &model,
                controls.mu_tolerance,
                team,
                self.interrupt,
            )?;
            report.detector(
                Method::Mu,
                observed.status,
                observed.trajectory.accepted_iterations,
                observed.work,
            )?;
            if let Some(direction) = &observed.direction {
                (self.interrupt)(InterruptPoint::LinearFrame)?;
                accept(ledger, direction, &model, report, team)?;
                return Ok(true);
            }
            let terminal = ledger.view().replay_mu_with(
                &observed,
                &model,
                controls.mu_tolerance,
                team,
                self.interrupt,
            )?;
            report.replay(
                Method::Mu,
                observed.status,
                terminal.iterations(),
                terminal.work(),
            )?;
            report.terminal(Method::Mu, observed.status)?;
        }
        Ok(false)
    }
    fn resolve_skip(
        &mut self,
        ledger: &Ledger,
        simplex: &SimplexReport,
        ir: &crate::ppml::separation::IrReport,
        model: &Model<'_>,
        report: &mut Report,
    ) -> Result<crate::ppml::separation::IrTerminal> {
        let (terminal, ir) = ledger.resolve_simplex_skip_with(
            simplex,
            ir,
            model,
            self.controls,
            self.team,
            self.interrupt,
        )?;
        report.replay(
            Method::Simplex,
            terminal.status(),
            terminal.pivots(),
            Work::default(),
        )?;
        report.capacity_resolution()?;
        report.terminal(Method::Simplex, terminal.status())?;
        Ok(ir)
    }
}

fn accept(
    ledger: &mut Ledger,
    direction: &crate::ppml::separation::DirectionProposal,
    model: &Model<'_>,
    report: &mut Report,
    team: &ThreadTeam,
) -> Result<()> {
    let accepted = ledger
        .accept_direction(direction, model, team)
        .map_err(|e| context(e, direction.method, "acceptance", report.generation))?;
    report.work.merge(accepted.validation.work)?;
    report.commit(Some(direction.method), accepted.commit)
}
fn context(error: FeregError, method: Method, phase: &str, generation: usize) -> FeregError {
    FeregError::new(
        error.code,
        format!(
            "separation {method:?} {phase}, generation {generation}: {}",
            error.message
        ),
    )
}
