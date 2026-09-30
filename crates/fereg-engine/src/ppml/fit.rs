use super::{
    bias,
    call::{Call, Phase, Request, Resources},
    inference,
    result::{BiasResult, Diagnostics, FitData, PpmlFit, Timings},
    separation,
};
use crate::{
    data::{ClusterFrame, Loaded, NumericColumn, ReferenceWeights},
    error::reserved,
    execution::{self, Probe},
    memory::Inputs,
    outputs,
    result::{Sample, WeightSummary},
    spec::{Estimator, Options},
    statistics,
    threads::ThreadTeam,
    wls, ModelSpec, Result,
};
use std::time::Instant;

pub fn fit(model: &ModelSpec, loaded: &Loaded, options: &Options) -> Result<PpmlFit> {
    fit_requested(
        model,
        loaded,
        options,
        &Request::default(),
        None,
        Resources::default(),
    )
}
/// Loaded Rust clients share the same single preflight and consumed controller.
pub fn fit_requested(
    model: &ModelSpec,
    loaded: &Loaded,
    options: &Options,
    request: &Request,
    units: Option<&NumericColumn>,
    resources: Resources,
) -> Result<PpmlFit> {
    let mut inputs = Inputs::from_loaded(loaded, 0, 0)?;
    if let Some(column) = units {
        inputs = inputs.with_numeric_auxiliary(column)?;
    }
    Call::new(
        model,
        options,
        request,
        inputs,
        loaded.weights.as_ref().map(|(kind, _)| *kind),
        resources,
    )?
    .run(loaded, units)
}
pub(super) fn execute(
    call: Call<'_>,
    loaded: &Loaded,
    units: Option<&NumericColumn>,
    team: &ThreadTeam,
    interrupt: &mut impl FnMut(Phase) -> Result<()>,
) -> Result<PpmlFit> {
    execution::run(
        team,
        |relay| {
            let fe_poll = || relay(Phase::Numerical(super::InterruptPoint::FixedEffects));
            let dense_poll = || relay(Phase::Numerical(super::InterruptPoint::DenseLinearAlgebra));
            execute_in(
                call,
                loaded,
                units,
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
    units: Option<&NumericColumn>,
    team: &ThreadTeam,
    probe: Probe<'_>,
    interrupt: &mut impl FnMut(Phase) -> Result<()>,
) -> Result<PpmlFit> {
    // Executor setup precedes host cancellation and allocation-failure probes.
    interrupt(Phase::Loaded)?;
    let units = call.units(loaded, units)?;
    let mut timings = Timings::default();
    let mut clock = Instant::now();
    let sample = super::prepare::prepare(call.model, loaded, &call.sources)?;
    timings.preparation = clock.elapsed();
    interrupt(Phase::Prepared)?;
    clock = Instant::now();
    let final_sample = separation::run_polled(
        sample,
        &call.columns,
        call.options,
        separation::Policy {
            drop_singletons: !call.model.keep_singletons,
            methods: call.request.separation,
        },
        call.request.separation_controls,
        team,
        super::interrupt::Flow {
            interrupt: &mut |point| interrupt(Phase::Numerical(point)),
            probe,
        },
    )?;
    timings.separation = clock.elapsed();
    interrupt(Phase::Separated)?;
    clock = Instant::now();
    // Final rank, mean and Hessian authority are cold on this exact generation.
    let accepted = final_sample.fit_polled(
        team,
        super::interrupt::Flow {
            interrupt: &mut |point| interrupt(Phase::Numerical(point)),
            probe,
        },
    )?;
    timings.estimation = clock.elapsed();
    interrupt(Phase::Estimated)?;
    clock = Instant::now();
    let inferred = {
        let frame = ClusterFrame::new_polled(call.model, final_sample.sample(), probe.dense())?;
        inference::estimate_polled(
            &call.columns,
            &accepted,
            &frame.views()?,
            call.kind,
            wls::Policy::default().bread_tolerance,
            team,
            probe.dense(),
        )?
    };
    let statistics = statistics::ppml(&accepted)?;
    let weights = WeightSummary::from(ReferenceWeights::new(
        &final_sample.sample().weights,
        final_sample.sample().n_active(),
        Estimator::Ppml,
    )?);
    timings.inference = clock.elapsed();
    interrupt(Phase::Inferred)?;
    clock = Instant::now();
    let outputs = outputs::ppml_polled(
        &accepted,
        &call.request.outputs,
        call.options.tolerance,
        team,
        probe.poll(),
    )?;
    timings.outputs = clock.elapsed();
    interrupt(Phase::Outputs)?;
    clock = Instant::now();
    let bias = bias::compute_polled(
        &accepted,
        call.model,
        call.request.bias,
        units.map(NumericColumn::view),
        team,
        &mut || interrupt(Phase::BiasCorrection),
        probe,
    )?
    .map(|correction| BiasResult::new(correction, call.admitted.estimate().bias_workspace_bytes()))
    .transpose()?;
    timings.bias = clock.elapsed();
    interrupt(Phase::Corrected)?;
    let diagnostics = Diagnostics::from(accepted.into_state());
    // No accepted means, raw/within design, alpha, bread cache or detector
    // session survives beyond requested outputs and the original-row ledger.
    let (sample, _, _, separation) = final_sample.into_parts();
    let sample = Sample::from(sample);
    let mut stripes = reserved(call.columns.slots.len())?;
    stripes.extend(call.columns.slots.into_iter().map(|slot| slot.name));
    timings.total = call.started.elapsed();
    Ok(PpmlFit(FitData {
        stripes,
        coefficients: inferred.coefficients,
        statuses: inferred.statuses,
        coordinates: inferred.coordinates,
        covariance: inferred.covariance,
        z_statistics: inferred.z_statistics,
        wald: inferred.wald,
        dof: inferred.dof,
        absorbed_dof: inferred.absorbed_dof,
        statistics,
        sample,
        weights,
        outputs,
        diagnostics,
        separation,
        bias,
        requested_solver: call.options.solver,
        vce: call.kind,
        workers: team.threads(),
        memory: call.admitted.estimate().clone(),
        timings,
    }))
}
