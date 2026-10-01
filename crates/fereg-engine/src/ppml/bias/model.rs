//! Bind the complete declared grammar, including syntactically omitted factors.
use super::invalid;
use crate::{
    data::CompiledColumns,
    error::{ErrorCode, FeregError, Result},
    execution,
    ppml::irls,
    spec::{
        metadata::{ComponentKind, LevelState, RawTermKind},
        Estimator, ModelSpec,
    },
};
pub(super) fn bind(fit: &irls::Fit<'_>, model: &ModelSpec) -> Result<CompiledColumns> {
    validate_model(model)?;
    let topology = fit
        .problem()
        .sample
        .topology
        .as_ref()
        .ok_or_else(|| invalid("bias topology is absent"))?;
    if topology.effects().len() != 2
        || topology
            .effects()
            .iter()
            .zip(&model.absorbed_terms)
            .any(|(a, b)| {
                a.term_id != Some(b.id)
                    || a.intercept != b.intercept
                    || a.categorical_sources != b.categorical_sources
                    || a.slopes.len() != b.slope_sources.len()
            })
    {
        return Err(invalid(
            "bias model does not match the accepted FE declaration",
        ));
    }
    CompiledColumns::new(model, Estimator::Ppml)
}
pub(crate) fn validate_model(model: &ModelSpec) -> Result<()> {
    validate_model_polled(model, &mut || Ok(()))
}
pub(crate) fn validate_model_polled(
    model: &ModelSpec,
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    model.validate_polled(poll)?;
    if model.absorbed_terms.len() != 2
        || execution::any_with(
            &model.absorbed_terms,
            |t| !t.intercept || t.categorical_sources.is_empty() || !t.slope_sources.is_empty(),
            poll,
        )?
        || execution::any_with(
            &model.model.terms,
            |t| {
                t.raw_kind != RawTermKind::Variable
                    || t.components.len() != 1
                    || t.components[0].kind != ComponentKind::Continuous
                    || t.components[0].level_state != LevelState::Absent
            },
            poll,
        )?
    {
        return Err(FeregError::new(ErrorCode::Unsupported,
            "bias correction requires two categorical intercept FE sets and ordinary numeric controls, including omitted terms"));
    }
    poll()
}
