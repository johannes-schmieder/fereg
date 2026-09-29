use super::{cone::failed, DirectionProposal, Model, SpanValidation};
use crate::{
    data::Weights,
    error::{reserved, Result},
    fe::{Design, Engine},
    ppml::separation::{invalid, View},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls,
};

pub(super) fn certify(
    view: View<'_>,
    proposal: &DirectionProposal,
    model: &Model<'_>,
    team: &ThreadTeam,
) -> Result<SpanValidation> {
    model.options.validate()?;
    if model.raw.nrows() != view.sample.n_active() || model.raw.ncols() != model.roles.len() {
        return Err(invalid(
            "separation model is not aligned to the active generation",
        ));
    }
    // A positive rescaling preserves the cone and prevents magnitude alone
    // from overflowing or underflowing the fresh span solve and its norm ratio.
    let mut normalized = reserved(proposal.direction.len())?;
    normalized.extend(
        proposal
            .direction
            .iter()
            .map(|v| v / proposal.certificate.direction_scale),
    );
    let design = Design::new(view.sample, Weights::Unit)?;
    let mut options = Options {
        tolerance: model
            .options
            .tolerance
            .min(model.controls.projection_tolerance),
        ..model.options
    };
    // FE score and whole-model span residual use different scales. A detector
    // projection at the publication target can pass the former while missing
    // the latter; certification uses the shared strict PPML inner policy.
    options.tolerance = crate::ppml::control::Controls::new(&options)?.strict_tolerance();
    let mut engine = Engine::new(&design, Estimator::Ppml, &options, team)?;
    let policy = wls::Policy {
        score_tolerance: wls::Policy::default()
            .score_tolerance
            .min(model.controls.projection_tolerance),
        ..wls::Policy::default()
    };
    let fit = wls::solve_absorbed(
        &mut engine,
        model.raw,
        &normalized,
        model.roles,
        policy,
        team,
    )?;
    let norm = normalized.iter().fold(0_f64, |n, &v| n.hypot(v));
    let residual_norm = fit.residuals.iter().fold(0_f64, |n, &v| n.hypot(v));
    let span_residual_relative = residual_norm / norm;
    if !span_residual_relative.is_finite()
        || span_residual_relative > model.controls.projection_tolerance
    {
        return Err(failed(&format!(
            "separation direction failed finite-plus-FE span certification (relative residual={span_residual_relative:e}, required={:e}, original finite score={:e})",
            model.controls.projection_tolerance, fit.relative_original_finite_score
        )));
    }
    Ok(SpanValidation {
        certificate: proposal.certificate,
        span_residual_relative,
        projection_diagnostics: fit.projection_diagnostics,
        recovery_diagnostics: fit.recovery_diagnostics,
    })
}
