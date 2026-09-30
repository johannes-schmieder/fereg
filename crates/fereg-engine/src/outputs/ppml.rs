//! Accepted-state PPML outputs; no exploratory state, re-solve or hidden prediction bank.
use super::{
    allocate, fixed, invalid, numerical, validate_scatter, Kind, OutputCertificate, Outputs,
    Request,
};
use crate::{
    data::{DenseMatrix, Weights},
    error::Result,
    fe::Design,
    ppml::{
        family::mean_from_eta,
        irls,
        residuals::{self, ResidualKind},
    },
    threads::ThreadTeam,
};
#[derive(Debug, Clone, Default)]
pub struct PpmlRequest {
    /// Xbd is the eta/xbd alias; all other shared names keep their SPEC meaning.
    pub common: Request,
    pub mean: bool,
    /// None selects response when a residual bank is requested.
    pub residual_type: Option<ResidualKind>,
}
pub fn ppml(
    fit: &irls::Fit<'_>,
    request: &PpmlRequest,
    tolerance: f64,
    team: &ThreadTeam,
) -> Result<Outputs> {
    if !tolerance.is_finite() || tolerance <= 0. || tolerance >= 1. {
        return Err(invalid("PPML output tolerance is invalid"));
    }
    if request.residual_type.is_some() && !request.common.residuals {
        return Err(invalid("PPML residual type requires a residual output"));
    }
    let problem = fit.problem();
    let sample = problem.sample;
    fixed::validate_channels(
        sample
            .topology
            .as_ref()
            .ok_or_else(|| invalid("PPML output topology is absent"))?,
        &request.common,
    )?;
    let kinds = request.common.kinds(request.mean)?;
    if kinds.iter().all(|k| *k == Kind::DropReason) {
        let output = allocate(sample, kinds)?;
        validate_scatter(sample, &output)?;
        return Ok(output);
    }
    let design = Design::new(sample, Weights::Provided(&fit.hessian_weights))?;
    // The sealed final-Hessian fit already owns the common qmu-normalized
    // gauge. Re-normalizing it could perturb a tiny original FE target.
    let alpha = &fit.point.absorbed;
    let absorbed = design.multiply(alpha, team)?;
    let offset = irls::input::offset(sample)?;
    let n = sample.n_active();
    let mut rhs = DenseMatrix::zeros(n, 1)?;
    let mut residual = DenseMatrix::zeros(n, 1)?;
    for row in 0..n {
        rhs[(row, 0)] = fit.point.eta[row]
            - offset.as_deref().map_or(0., |v| v[row])
            - problem.raw.row_combination(row, &fit.point.finite, 0.);
        residual[(row, 0)] = fit.linear.residuals[row];
    }
    let certificate = design.certify(&rhs, alpha, &residual, tolerance, team)?;
    let mut output = allocate(sample, kinds)?;
    let mut reconstruction = 0_f64;
    for (row, &original) in sample.retained_rows().iter().enumerate() {
        let adjustment = offset.as_deref().map_or(0., |v| v[row]);
        let finite = problem.raw.row_combination(row, &fit.point.finite, 0.);
        let xb = adjustment + finite;
        let d = absorbed[(row, 0)];
        let eta = fit.point.eta[row];
        let mu = fit.evaluation.mean[row];
        let replay = mean_from_eta(xb + d)?;
        let scale = eta
            .abs()
            .max(adjustment.abs())
            .max(finite.abs())
            .max(d.abs())
            .max(1.);
        let error = ((eta - (xb + d)).abs() / scale).max((mu - replay).abs() / mu.max(replay));
        if !error.is_finite() || error > tolerance {
            return Err(numerical(
                "PPML output predictor or mean reconstruction failed",
            ));
        }
        reconstruction = reconstruction.max(error);
        for column in &mut output.columns {
            let value = match column.kind {
                Kind::Xb => xb,
                Kind::Xbd => eta,
                Kind::Mean => mu,
                Kind::Absorbed => d,
                Kind::Residuals => residuals::value(
                    sample.outcome[row],
                    mu,
                    request.residual_type.unwrap_or_default(),
                )?,
                Kind::Saved { effect, channel } => {
                    fixed::contribution(&design, alpha, effect, channel, row)?
                }
                Kind::DropReason => continue,
            };
            if !value.is_finite() {
                return Err(numerical("PPML observation output is not representable"));
            }
            column.values[original] = value;
        }
    }
    validate_scatter(sample, &output)?;
    output.certificate = Some(OutputCertificate {
        fixed_effects: certificate,
        relative_reconstruction: reconstruction,
    });
    Ok(output)
}
