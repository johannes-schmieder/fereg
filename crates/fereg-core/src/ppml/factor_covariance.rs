use super::{
    dense_score_reduction, matrix_relative_difference, ppml_sandwich_result, PpmlInferenceDesign,
    PpmlInferenceInput, PpmlSandwichDesign, PpmlScoreMode, PpmlScoreMultipliers, PpmlVceKind,
    PPML_SANDWICH_REDUCTION,
};
use crate::error::{ErrorCode, FeregError, Result};
use crate::options::ThreadConfig;
use crate::parallel::ThreadTeam;
use crate::sandwich::{
    reduce_clustered_score_meats, reduce_dense_score_meats, SandwichClusters,
    SystemSandwichReservation,
};

/// Evaluate the same strict sandwich from its score factors. Rounded,
/// rank-deficient meat can lose a null direction before B*M*B' is evaluated.
/// Raw meat banks must be retired before entering this admitted pass.
pub(super) fn reconstruct<D: PpmlInferenceDesign + Copy + Sync>(
    input: &PpmlInferenceInput<'_, D>,
    kind: PpmlVceKind,
    threads: ThreadConfig,
    correction: f64,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<(Vec<f64>, f64)> {
    let rank = input.within_design.dimension();
    let design = PpmlSandwichDesign {
        source: input.within_design,
    };
    let multipliers = PpmlScoreMultipliers {
        residuals: input.score_residuals,
        prior_weights: input.prior_weights,
        weight_kind: input.weight_report.kind,
        mode: if kind == PpmlVceKind::Robust {
            PpmlScoreMode::Robust
        } else {
            PpmlScoreMode::Clustered
        },
    };
    let mut reservation = SystemSandwichReservation;
    interrupt()?;
    let (mut production, mut certificate) = ThreadTeam::new(threads).install(|| match kind {
        PpmlVceKind::Robust => {
            let reduced = ppml_sandwich_result(reduce_dense_score_meats(
                design,
                multipliers,
                dense_score_reduction(rank),
                Some(input.identified_bread),
                &mut reservation,
            ))?;
            Ok((reduced.production, reduced.certificate))
        }
        PpmlVceKind::Clustered => {
            let clusters = SandwichClusters::new(input.clusters, design.source.nrows(), 1)?;
            let reduced = ppml_sandwich_result(reduce_clustered_score_meats(
                design,
                multipliers,
                clusters,
                PPML_SANDWICH_REDUCTION,
                Some(input.identified_bread),
                &mut reservation,
            ))?;
            Ok((reduced.production, reduced.certificate))
        }
    })??;
    interrupt()?;
    for matrix in [&mut production, &mut certificate] {
        for value in matrix.iter_mut() {
            *value *= correction;
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML influence covariance correction is nonfinite",
                ));
            }
        }
    }
    let error = matrix_relative_difference(&production, &certificate, rank)?;
    Ok((production, error))
}

/// Check a componentwise roundoff envelope before trusting rounded B*M*B'.
/// For a Gram meat, Cauchy bounds its absolute summands by sqrt(Mjj*Mkk).
/// The count covers the ordinary Gram sum, both rank-sized products, bounded
/// compensated merges, correction and symmetrization. It is a conservative
/// prompt for an independent factor check, never a relaxation of tolerance.
pub(super) fn needs_factor_check<D: PpmlInferenceDesign>(
    input: &PpmlInferenceInput<'_, D>,
    meat: &[f64],
    covariance: &[f64],
    workspace: &mut [f64],
    summands: usize,
    correction: f64,
) -> bool {
    let bread = input.identified_bread;
    let dimension = input.within_design.dimension();
    let tolerance = input.certificate_tolerance;
    let operations = summands as f64 + 2.0 * dimension as f64 + 32.0;
    let roundoff = operations * (f64::EPSILON / 2.0);
    if roundoff >= 1.0 {
        return true;
    }
    let gamma = roundoff / (1.0 - roundoff);
    if gamma >= 1.0 {
        return true;
    }
    // Stored Gram diagonals can themselves round downward.
    let gamma = gamma / (1.0 - gamma);
    for row in 0..dimension {
        let mut scale = 0.0;
        for column in 0..dimension {
            let diagonal = meat[column * dimension + column];
            if diagonal <= 0.0 || !diagonal.is_finite() || diagonal.is_subnormal() {
                return true;
            }
            scale += bread[row * dimension + column].abs() * diagonal.sqrt();
        }
        workspace[row] = scale;
    }
    let total: f64 = workspace[..dimension].iter().sum();
    for row in 0..dimension {
        let row_scale: f64 = covariance[row * dimension..(row + 1) * dimension]
            .iter()
            .map(|value| value.abs())
            .sum();
        let uncertainty = correction * gamma * workspace[row] * total;
        if !row_scale.is_finite()
            || !uncertainty.is_finite()
            || (workspace[row] > 0.0 && total > 0.0 && uncertainty == 0.0)
            || (uncertainty > 0.0 && row_scale < f64::MIN_POSITIVE)
            || uncertainty > tolerance * row_scale
            || covariance[row * dimension + row] < 0.0
        {
            return true;
        }
    }
    false
}
