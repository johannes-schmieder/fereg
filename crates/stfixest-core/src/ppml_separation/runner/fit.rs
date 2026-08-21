use super::detect::detect_separation;
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::FeDesign;
use crate::ppml::{fit_ppml_with_start, PpmlConfig, PpmlStart};
use crate::ppml_separation::{SeparatedPpmlResult, SeparationConfig};
use crate::ppml_separation::subset::{select_rows, subset_problem};

#[allow(clippy::too_many_arguments)]
pub fn fit_ppml_separated(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    clusters: &[Vec<u32>],
    ppml: PpmlConfig,
    separation: SeparationConfig,
) -> Result<SeparatedPpmlResult> {
    fit_ppml_separated_with_start(
        outcome,
        regressors,
        prior_weights,
        offset,
        fixed_effects,
        clusters,
        ppml,
        separation,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn fit_ppml_separated_with_start(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    clusters: &[Vec<u32>],
    ppml: PpmlConfig,
    separation: SeparationConfig,
    start: Option<&PpmlStart>,
) -> Result<SeparatedPpmlResult> {
    if prior_weights.len() != outcome.len()
        || offset.len() != outcome.len()
        || clusters.iter().any(|cluster| cluster.len() != outcome.len())
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "separated PPML inputs have inconsistent observation counts",
        ));
    }
    if let Some(start) = start {
        if start.linear_predictor.len() != outcome.len()
            || start.coefficients.len() != regressors.ncols()
        {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "separated PPML warm start has inconsistent dimensions",
            ));
        }
    }
    let report = detect_separation(outcome, regressors, fixed_effects, ppml, separation, start)?;
    let subset = subset_problem(outcome, regressors, fixed_effects, &report.active)?;
    let weights = select_rows(prior_weights, &subset.original_rows);
    let offset = select_rows(offset, &subset.original_rows);
    let clusters = clusters
        .iter()
        .map(|cluster| select_rows(cluster, &subset.original_rows))
        .collect::<Vec<_>>();
    let subset_start = start.map(|start| PpmlStart {
        coefficients: start.coefficients.clone(),
        linear_predictor: select_rows(&start.linear_predictor, &subset.original_rows),
    });
    let estimation = fit_ppml_with_start(
        &subset.outcome,
        &subset.regressors,
        &weights,
        &offset,
        subset.fixed_effects.as_ref(),
        &clusters,
        ppml,
        subset_start.as_ref(),
    )?;
    Ok(SeparatedPpmlResult {
        estimation,
        separation: report,
    })
}
