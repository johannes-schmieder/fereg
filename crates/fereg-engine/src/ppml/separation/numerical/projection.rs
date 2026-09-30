use super::{cone::failed, DirectionProposal, Model, SpanValidation};
use crate::{
    data::Weights,
    error::{reserved, Result},
    fe::{Design, Engine},
    ppml::separation::View,
    spec::Estimator,
    threads::ThreadTeam,
    wls,
};

pub(super) fn certify(
    view: View<'_>,
    proposal: &DirectionProposal,
    model: &Model<'_>,
    team: &ThreadTeam,
) -> Result<SpanValidation> {
    model.validate(view)?;
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
    let options = model.span_options()?;
    let mut engine = Engine::new(&design, Estimator::Ppml, &options, team)?;
    let mut work = wls::Work::default();
    let fit = wls::solve_absorbed_recorded(
        &mut engine,
        model.raw,
        &normalized,
        model.roles,
        wls::AbsorbedControls {
            probe: model.probe,
            finite: model.linear_policy(),
            basis: None,
        },
        &mut work,
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
        setup_diagnostics: fit.setup_diagnostics,
        work,
    })
}
