use super::{Report, RunControls};
use crate::{
    data::{DenseMatrix, Weights},
    error::{ErrorCode, FeregError, Result},
    fe::{Design, Plan},
    ppml::separation::{Ledger, Method, Model, SimplexReport, Status},
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

pub(super) fn numerical(
    ledger: &mut Ledger,
    raw: &DenseMatrix,
    roles: &[Role],
    options: Options,
    controls: RunControls,
    report: &mut Report,
    team: &ThreadTeam,
) -> Result<bool> {
    let model = Model {
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
        let observed = ledger.view().model_simplex(&model, controls.simplex)?;
        report.detector(
            Method::Simplex,
            observed.status,
            observed.pivots,
            Work::default(),
        )?;
        if let Some(direction) = &observed.direction {
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
            let terminal = ledger
                .view()
                .replay_simplex(&observed, &model, controls.simplex)?;
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
            .iterated_rectifier(&model, controls.ir_iterations, team)
            .map_err(|e| context(e, Method::IteratedRectifier, "detector", report.generation))?;
        report.detector(
            Method::IteratedRectifier,
            observed.status,
            observed.iterations,
            observed.work,
        )?;
        if let Some(direction) = &observed.direction {
            accept(ledger, direction, &model, report, team)?;
            return Ok(true);
        }
        let terminal = match skipped.as_ref() {
            Some(simplex) => {
                resolve_skip(ledger, simplex, &observed, &model, controls, report, team)?
            }
            None => ledger
                .view()
                .replay_ir(&observed, &model, controls.ir_iterations, team)?,
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
        let observed = ledger
            .view()
            .fitted_mu(&model, controls.mu_tolerance, team)?;
        report.detector(
            Method::Mu,
            observed.status,
            observed.trajectory.accepted_iterations,
            observed.work,
        )?;
        if let Some(direction) = &observed.direction {
            accept(ledger, direction, &model, report, team)?;
            return Ok(true);
        }
        let terminal = ledger
            .view()
            .replay_mu(&observed, &model, controls.mu_tolerance, team)?;
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
fn resolve_skip(
    ledger: &Ledger,
    simplex: &SimplexReport,
    ir: &crate::ppml::separation::IrReport,
    model: &Model<'_>,
    controls: RunControls,
    report: &mut Report,
    team: &ThreadTeam,
) -> Result<crate::ppml::separation::IrTerminal> {
    let (terminal, ir) = ledger.resolve_simplex_skip(
        simplex,
        ir,
        model,
        controls.simplex,
        controls.ir_iterations,
        team,
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
