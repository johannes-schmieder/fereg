use super::call::{Call, Phase, Request, Resources};
use crate::{
    data::{ClusterFrame, Loaded, Prepared, ReferenceWeights, Weights},
    dof::{compute_absorbed_dof_polled, ResidualDof},
    error::reserved,
    execution::{self, Poll, Probe},
    fe::{Design, Engine},
    memory::Inputs,
    outputs::{self, OlsFrame},
    result::{Diagnostics, FitData, OlsFit, Sample, Timings, WeightSummary},
    spec::{Estimator, Options},
    statistics,
    threads::ThreadTeam,
    vce,
    wls::{self, AbsorbedControls},
    ModelSpec, Result,
};
use std::time::Instant;

pub fn fit(model: &ModelSpec, loaded: &Loaded, options: &Options) -> Result<OlsFit> {
    fit_requested(
        model,
        loaded,
        options,
        &Request::default(),
        Resources::default(),
    )
}
/// Already-loaded Rust clients share the host's preflight and controller.
pub fn fit_requested(
    model: &ModelSpec,
    loaded: &Loaded,
    options: &Options,
    request: &Request,
    resources: Resources,
) -> Result<OlsFit> {
    let inputs = Inputs::from_loaded(loaded, 0, 0)?;
    Call::new(
        model,
        options,
        request,
        inputs,
        loaded.weights.as_ref().map(|(kind, _)| *kind),
        resources,
    )?
    .run(loaded)
}
pub(super) fn execute(
    call: Call<'_>,
    loaded: &Loaded,
    team: &ThreadTeam,
    interrupt: &mut impl FnMut(Phase) -> Result<()>,
) -> Result<OlsFit> {
    execution::run(
        team,
        |relay| {
            let fe_poll = || relay(Phase::FixedEffects);
            let dense_poll = || relay(Phase::DenseLinearAlgebra);
            execute_in(
                call,
                loaded,
                team,
                Probe::with_dense(&fe_poll, &dense_poll),
                &mut |phase| execution::check(Some(&|| relay(phase))),
            )
        },
        interrupt,
    )
}
pub(super) fn execute_in(
    call: Call<'_>,
    loaded: &Loaded,
    team: &ThreadTeam,
    probe: Probe<'_>,
    interrupt: &mut impl FnMut(Phase) -> Result<()>,
) -> Result<OlsFit> {
    // Executor setup must precede the first host poll; a caller may arm an
    // allocation injector here to qualify fallible numerical preparation.
    interrupt(Phase::Loaded)?;
    let mut timings = Timings::default();
    let mut clock = Instant::now();
    let sample = super::prepare::prepare_planned(call.model, loaded, &call.columns, &call.sources)?;
    let raw = call.columns.generate(&sample)?;
    let weights = ReferenceWeights::new(&sample.weights, sample.n_active(), Estimator::Ols)?;
    let summary = WeightSummary::from(weights);
    timings.preparation = clock.elapsed();
    interrupt(Phase::Prepared)?;
    clock = Instant::now();
    let design = Design::new(&sample, Weights::Reference(weights))?;
    let mut engine =
        Engine::new_polled(&design, Estimator::Ols, &call.options, team, probe.dense())?;
    let mut work = wls::Work::default();
    let linear = wls::solve_absorbed_recorded(
        &mut engine,
        &raw,
        &sample.outcome,
        &call.columns.roles,
        AbsorbedControls {
            probe,
            ..AbsorbedControls::default()
        },
        &mut work,
        team,
    )?;
    // Setup/solver owners end here; inference and outputs reuse coefficients.
    drop(engine);
    timings.estimation = clock.elapsed();
    interrupt(Phase::Estimated)?;
    clock = Instant::now();
    let reported = super::report(
        &call.columns,
        &raw,
        &linear.within,
        &sample.outcome,
        weights,
        &linear.finite,
    )?;
    let inference = infer(
        &call,
        &sample,
        &linear,
        weights,
        &reported,
        team,
        probe.dense(),
    )?;
    timings.inference = clock.elapsed();
    interrupt(Phase::Inferred)?;
    clock = Instant::now();
    let outputs = outputs::ols_polled(
        &OlsFrame {
            design: &design,
            finite_design: &raw,
            coefficients: &linear.finite.coefficients,
            absorbed_coefficients: &linear.absorbed_coefficients,
            residuals: &linear.residuals,
            reported_constant: reported.reported_constant,
        },
        &call.request.outputs,
        call.options.tolerance,
        team,
        probe.poll(),
    )?;
    timings.outputs = clock.elapsed();
    interrupt(Phase::Outputs)?;
    drop(design);
    let mut stripes = reserved(call.columns.slots.len())?;
    stripes.extend(call.columns.slots.into_iter().map(|slot| slot.name));
    let diagnostics = Diagnostics::new(linear, work, reported.constant_formula_error);
    // The complete observation ledger is mandatory; other row banks survive
    // only when requested explicitly. No prepared/score/solver cache escapes.
    let sample = Sample::from(sample);
    timings.total = call.started.elapsed();
    Ok(OlsFit(FitData {
        stripes,
        coefficients: reported.coefficients,
        statuses: reported.statuses,
        coordinates: reported.coordinates,
        covariance: inference.covariance,
        t_statistics: inference.t_statistics,
        wald: inference.wald,
        dof: inference.dof,
        absorbed_dof: inference.absorbed_dof,
        statistics: inference.statistics,
        sample,
        weights: summary,
        outputs,
        diagnostics,
        requested_solver: call.options.solver,
        vce: call.kind,
        workers: team.threads(),
        memory: call.admitted.estimate().clone(),
        timings,
    }))
}

struct Inference {
    covariance: vce::Covariance,
    t_statistics: Vec<f64>,
    wald: vce::Wald,
    dof: ResidualDof,
    absorbed_dof: crate::dof::DofReport,
    statistics: statistics::OlsStatistics,
}
fn infer(
    call: &Call<'_>,
    sample: &Prepared,
    linear: &wls::AbsorbedFit,
    weights: ReferenceWeights<'_>,
    reported: &super::Reported,
    team: &ThreadTeam,
    poll: Poll<'_>,
) -> Result<Inference> {
    execution::check(poll)?;
    let model = call.model;
    let columns = &call.columns;
    let request = call.request;
    let kind = call.kind;
    let frame = ClusterFrame::new_polled(model, sample, poll)?;
    let clusters = frame.views()?;
    let mut codes = reserved(clusters.len())?;
    let mut counts = reserved(clusters.len())?;
    for cluster in &clusters {
        codes.push(cluster.codes);
        counts.push(cluster.n_levels);
    }
    let absorbed_dof = compute_absorbed_dof_polled(
        sample,
        columns.structural_intercept,
        request.dof,
        &codes,
        poll,
    )?;
    let dof = ResidualDof::new(
        Estimator::Ols,
        weights.effective(),
        reported.finite_rank,
        absorbed_dof.absorbed_rank,
        absorbed_dof.nested_categories,
        &counts,
    )?;
    let tolerance = wls::Policy::default().bread_tolerance;
    let covariance = vce::estimate_polled(
        &vce::Problem {
            design: &reported.score_design,
            bread: &reported.bread,
            residual: &linear.residuals,
            weights,
            clusters: &clusters,
            dof,
        },
        kind,
        tolerance,
        team,
        poll,
    )?
    .scatter_polled(reported.coefficients.len(), &reported.coordinates, poll)?;
    let mut tested = reserved(reported.coordinates.len())?;
    tested.extend(
        reported
            .coordinates
            .iter()
            .copied()
            .filter(|&slot| Some(slot) != columns.constant_slot),
    );
    let wald = vce::wald_polled(
        &reported.coefficients,
        &covariance.values,
        &tested,
        Estimator::Ols,
        tolerance,
        poll,
    )?;
    let t_statistics = vce::coefficient_statistics_polled(
        &reported.coefficients,
        &covariance.standard_errors,
        &reported.coordinates,
        poll,
    )?;
    let statistics = statistics::ols(
        &sample.outcome,
        &linear.within_outcome,
        &linear.residuals,
        weights,
        columns.structural_intercept,
        dof,
    )?;
    Ok(Inference {
        covariance,
        t_statistics,
        wald,
        dof,
        absorbed_dof,
        statistics,
    })
}
