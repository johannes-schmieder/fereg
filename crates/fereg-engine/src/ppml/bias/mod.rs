//! Optional full-system empirical coefficient mean correction on a sealed fit.
mod anchor;
mod calculation;
mod certificate;
mod diagnostics;
mod information;
mod model;
pub(crate) use model::validate_model_polled;
mod moments;
pub mod philox;
mod publication;
mod request;
mod support;
mod units;
use crate::error::{reserved, ErrorCode, FeregError, Result};
use crate::{
    data::{DenseMatrix, NumericColumnView},
    outputs::Outputs,
    ppml::irls,
    spec::ModelSpec,
    threads::ThreadTeam,
};
pub use diagnostics::DiagnosticTable;
pub use information::{with_information, Diagnostics, Information};
pub use moments::Coefficients;
pub use request::{Channel, Method, Request};
pub use support::{screen, Summary, Support};

#[derive(Debug, PartialEq)]
pub struct Correction {
    pub request: Request,
    pub coefficients: Coefficients,
    pub stripes: Vec<String>,
    /// Channel-major ordinary-stripe banks; the host names each row under e().
    pub ordinary: DenseMatrix,
    pub saved: Outputs,
    pub support: [Summary; 2],
    pub first_levels: usize,
    pub second_levels: usize,
    pub finite_slots: Vec<usize>,
    pub units: usize,
    pub probes: usize,
    pub groups: usize,
    pub minimum_unit_rows: usize,
    pub maximum_unit_rows: usize,
    pub minimum_mean: f64,
    pub maximum_mean: f64,
    pub diagnostics: Diagnostics,
    pub elapsed_seconds: f64,
    pub certificate_tolerance: f64,
}
/// None bypasses metadata, units, callbacks, allocations and numerical work.
/// Typed exact requests carry no randomization controls; the syntax binder must
/// reject explicitly supplied ancillary controls before forming this request.
pub fn compute(
    fit: &irls::Fit<'_>,
    model: &ModelSpec,
    request: Option<Request>,
    units: Option<NumericColumnView<'_>>,
    team: &ThreadTeam,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Correction>> {
    compute_polled(
        fit,
        model,
        request,
        units,
        team,
        interrupt,
        crate::execution::Probe::default(),
    )
}
pub(crate) fn compute_polled(
    fit: &irls::Fit<'_>,
    model: &ModelSpec,
    request: Option<Request>,
    units: Option<NumericColumnView<'_>>,
    team: &ThreadTeam,
    interrupt: &mut dyn FnMut() -> Result<()>,
    probe: crate::execution::Probe<'_>,
) -> Result<Option<Correction>> {
    let Some(request) = request else {
        if units.is_some() {
            return Err(invalid("biasunit requires bias correction"));
        }
        return Ok(None);
    };
    let started = std::time::Instant::now();
    interrupt().map_err(|e| context(e, "request", None, None))?;
    request
        .method
        .validate()
        .map_err(|e| context(e, "request", None, None))?;
    let columns = model::bind(fit, model).map_err(|e| context(e, "model", None, None))?;
    let support = screen(
        fit,
        units.ok_or_else(|| invalid("bias correction requires an explicit numeric biasunit"))?,
        interrupt,
    )
    .map_err(|e| context(e, "support", None, None))?;
    request
        .method
        .dimensions(fit.problem().sample.n_active(), support.identities().len())?;
    information::with_information_polled(
        fit,
        &columns,
        team,
        interrupt,
        probe,
        |info, interrupt| {
            let calculated =
                calculation::calculate(fit, &support, request.method, info, interrupt)?;
            let (stripes, ordinary) =
                publication::ordinary(&columns, info, &calculated.coefficients)?;
            let saved = publication::saved(
                fit.problem().sample,
                info.first_levels(),
                &calculated.coefficients,
                request.save_effects,
                interrupt,
            )
            .map_err(|e| context(e, "publication", None, None))?;
            let mut finite_slots = reserved(info.finite_slots().len())?;
            finite_slots.extend_from_slice(info.finite_slots());
            interrupt()?;
            Ok(Some(Correction {
                request,
                coefficients: calculated.coefficients,
                stripes,
                ordinary,
                saved,
                support: support.effects,
                first_levels: info.first_levels(),
                second_levels: info.second_levels(),
                finite_slots,
                units: support.identities().len(),
                probes: calculated.probes,
                groups: calculated.groups,
                minimum_unit_rows: support.minimum_unit_rows(),
                maximum_unit_rows: support.maximum_unit_rows(),
                minimum_mean: fit
                    .evaluation
                    .mean
                    .iter()
                    .copied()
                    .fold(f64::INFINITY, f64::min),
                maximum_mean: fit.evaluation.mean.iter().copied().fold(0., f64::max),
                diagnostics: info.diagnostics(),
                elapsed_seconds: started.elapsed().as_secs_f64(),
                certificate_tolerance: fit.options().tolerance,
            }))
        },
    )
    .map_err(|e| context(e, "correction", None, None))
}
fn context(
    mut error: FeregError,
    channel: &str,
    block: Option<usize>,
    probe: Option<usize>,
) -> FeregError {
    error.message = format!(
        "bias_channel={channel} bias_block={block:?} bias_probe={probe:?}: {}",
        error.message
    );
    error
}
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut values = reserved(n)?;
    values.resize(n, value);
    Ok(values)
}
fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
