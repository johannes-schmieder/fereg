//! Final-model recovery at original-response scales, independent of backend stops.
use super::{
    absorbed::{numerical, original_score, recorded_projection},
    work::Phase,
    Fit, Policy, Work,
};
use crate::{
    data::DenseMatrix,
    error::{ErrorCode, FeregError, Result},
    execution::Probe,
    fe::{Certificate, Engine, Projection, RhsDiagnostics, SetupDiagnostics},
    threads::ThreadTeam,
};

pub(super) struct Model<'a> {
    pub raw: &'a DenseMatrix,
    pub outcome: &'a [f64],
    pub finite: &'a Fit,
    pub policy: Policy,
    pub probe: Probe<'a>,
}
pub(super) struct Recovered {
    pub coefficients: DenseMatrix,
    pub residual: DenseMatrix,
    pub certificate: Certificate,
    pub diagnostics: Vec<RhsDiagnostics>,
    pub setup: Vec<SetupDiagnostics>,
    pub refinements: usize,
    pub original_score: f64,
}
pub(super) fn finish(
    engine: &mut Engine<'_, '_>,
    model: Model<'_>,
    target: DenseMatrix,
    mut coefficients: DenseMatrix,
    work: &mut Work,
    team: &ThreadTeam,
) -> Result<Recovered> {
    let design = engine.design();
    let tolerance = engine.plan().tolerance();
    let mut residual = design.reconstruct(&target, &coefficients, team)?;
    let mut diagnostics = Vec::new();
    let mut setup = Vec::new();
    let mut certificate = match design.certify(&target, &coefficients, &residual, tolerance, team) {
        Ok(certificate) => certificate,
        Err(error) if error.code == ErrorCode::NonConvergence => {
            // Cancellation can make the combined RHS much smaller than its
            // separate projections. Its own scale remains authoritative.
            let mut recovered =
                recorded_projection(engine, &target, Phase::Recovery, work, model.probe, team)?;
            append(&mut diagnostics, &mut setup, &mut recovered)?;
            coefficients = recovered.coefficients;
            residual = recovered.residuals;
            recovered.certificate
        }
        Err(error) => return Err(error),
    };
    let mut original_score = original_score_fn(engine, &model, &residual)?;
    let acceptance = tolerance.max(model.policy.score_tolerance);
    let mut refinements = 0;
    // The original response can be far smaller than y-Xb. A certificate at
    // that larger FE-target scale does not bound the original finite score.
    // Correct actual residuals at their own scale; never relax acceptance.
    while original_score > acceptance && refinements < 3 && design.layout().total != 0 {
        let mut correction =
            recorded_projection(engine, &residual, Phase::Recovery, work, model.probe, team)?;
        append(&mut diagnostics, &mut setup, &mut correction)?;
        for (alpha, delta) in coefficients
            .as_mut_slice()
            .iter_mut()
            .zip(correction.coefficients.as_slice())
        {
            *alpha += delta;
        }
        design.normalize_coefficients(&mut coefficients, tolerance, team)?;
        residual = design.reconstruct(&target, &coefficients, team)?;
        certificate = design.certify(&target, &coefficients, &residual, tolerance, team)?;
        let next = original_score_fn(engine, &model, &residual)?;
        refinements += 1;
        if next >= original_score {
            original_score = next;
            break;
        }
        original_score = next;
    }
    if original_score > acceptance {
        return Err(numerical(
            "absorbed WLS original finite score did not certify",
        ));
    }
    Ok(Recovered {
        coefficients,
        residual,
        certificate,
        diagnostics,
        setup,
        refinements,
        original_score,
    })
}
fn original_score_fn(
    engine: &Engine<'_, '_>,
    model: &Model<'_>,
    residual: &DenseMatrix,
) -> Result<f64> {
    original_score(
        engine,
        model.raw,
        model.outcome,
        &model.finite.kept,
        residual.as_slice(),
        model.probe.dense(),
    )
}
fn append(
    reports: &mut Vec<RhsDiagnostics>,
    setup: &mut Vec<SetupDiagnostics>,
    projection: &mut Projection,
) -> Result<()> {
    reports
        .try_reserve(projection.diagnostics.len())
        .map_err(|_| allocation())?;
    setup
        .try_reserve(projection.setup_diagnostics.len())
        .map_err(|_| allocation())?;
    reports.append(&mut projection.diagnostics);
    setup.append(&mut projection.setup_diagnostics);
    Ok(())
}
fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "unable to reserve model recovery diagnostics",
    )
}
