use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    certify_residuals_with_reference_norms, weighted_column_norms, FeDesign, FeFallbackReason,
    FeSolverKind, LsmrPreconditioner, SolveReport, SolveStopReason,
};
use crate::options::ThreadConfig;

/// Modified-LSMR tuning for fixed-effect absorption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LsmrConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub preconditioner: LsmrPreconditioner,
    pub threads: ThreadConfig,
}

impl Default for LsmrConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 16_000,
            preconditioner: LsmrPreconditioner::Auto,
            threads: ThreadConfig::default(),
        }
    }
}

/// Residualize one or more right-hand sides with fedemean's modified LSMR on
/// the full system (any supported FE grammar, including slopes).
///
/// All input materialization and all output writes occur on the caller thread.
/// Numerical work uses the caller's current Rayon pool, which is the isolated
/// action-local `ThreadTeam` in public estimation. It never has access to
/// Stata's SPI. A design fedemean does not take is `Unsupported`.
pub fn absorb_lsmr(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
) -> Result<SolveReport> {
    absorb_lsmr_certified_at(design, weights, values, config, config.tolerance)
}

/// [`absorb_lsmr`] certified at `certificate_tolerance`. fedemean's
/// recurrence stops on its own certificate at that tolerance (its measure
/// equals SPEC §16's on nonsingular level blocks); fereg's independent
/// certificate then decides, after at most one correction (see
/// [`super::fedemean_slope::absorb_certified`]). `config.tolerance` is not a
/// separate recurrence target.
pub(crate) fn absorb_lsmr_certified_at(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
    certificate_tolerance: f64,
) -> Result<SolveReport> {
    match absorb_lsmr_with(
        design,
        weights,
        values,
        config,
        (certificate_tolerance, None),
        None,
    )? {
        Some((report, ..)) => Ok(report),
        None => Err(ineligible()),
    }
}

/// The error of a modified-LSMR request on a design fedemean does not take.
pub(crate) fn ineligible() -> FeregError {
    FeregError::new(
        ErrorCode::Unsupported,
        "solver(lsmr): fedemean's full system does not take this fixed-effect design",
    )
}

/// Which case produced [`absorb_lsmr_with`]'s report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LsmrRoute {
    /// fedemean's full system (its coefficients, when requested, are filled).
    Fedemean,
    /// No fixed effects or no columns: the certified input, no solve.
    NoFixedEffects,
}

/// [`absorb_lsmr_certified_at`] with fedemean's `memory_limit` and, with
/// `coefficients` (the design's layout, `values.ncols()` columns), the fitted
/// FE coefficients added in place when fedemean ran. `None`: fedemean does
/// not take the design (it refuses before reading data); the values are
/// untouched for the caller's fallback. An uncertified report comes with the
/// reason a fallback reports.
pub(crate) fn absorb_lsmr_with(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
    (certificate_tolerance, memory_limit): (f64, Option<u128>),
    coefficients: Option<(&super::fedemean_slope::Layout, &mut [f64])>,
) -> Result<Option<(SolveReport, LsmrRoute, FeFallbackReason)>> {
    validate_config(config)?;
    let attempt = super::fedemean_slope::Attempt::lsmr(
        config.preconditioner,
        FeSolverKind::Lsmr,
        certificate_tolerance,
        config.max_iterations,
        memory_limit,
    );
    absorb_attempt(design, weights, values, &attempt, coefficients)
}

/// One fedemean full-system `attempt` (modified LSMR or PCG), certified
/// independently at its tolerance after at most one correction (see
/// [`super::fedemean_slope::absorb_certified`]). `None`: fedemean does not
/// take the design or choice (refused before any data is read; the values
/// are untouched). An uncertified report comes with the reason a fallback
/// reports: fedemean's decline, or a missed certificate.
pub(crate) fn absorb_attempt(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    attempt: &super::fedemean_slope::Attempt,
    coefficients: Option<(&super::fedemean_slope::Layout, &mut [f64])>,
) -> Result<Option<(SolveReport, LsmrRoute, FeFallbackReason)>> {
    crate::interrupt::checkpoint()?;
    let certificate_tolerance = attempt.tolerance;
    if !certificate_tolerance.is_finite() || certificate_tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR tolerance must be finite and positive",
        ));
    }
    if design.nobs() != values.nrows() || weights.len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR inputs have inconsistent observation counts",
        ));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "LSMR weights must be finite and positive",
        ));
    }
    let lsmr = attempt.choice.method == fedemean::Method::Lsmr;
    if design.effects().is_empty() || values.ncols() == 0 {
        let reference_norms = weighted_column_norms(values, weights)?;
        let certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            values,
            &reference_norms,
            certificate_tolerance,
        )?;
        let report = SolveReport {
            requested_preconditioner: attempt.requested_preconditioner,
            used_preconditioner: attempt.requested_preconditioner.map(|p| match p {
                LsmrPreconditioner::Auto => LsmrPreconditioner::Diagonal,
                other => other,
            }),
            ..SolveReport::single_route(
                if lsmr {
                    FeSolverKind::Lsmr
                } else {
                    FeSolverKind::Cg
                },
                SolveStopReason::NoFixedEffects,
                certificate,
            )
        };
        return Ok(Some((
            SolveReport {
                requested_solver: attempt.requested,
                ..report
            },
            LsmrRoute::NoFixedEffects,
            FeFallbackReason::None,
        )));
    }
    let unit = weights.iter().all(|&weight| weight == 1.0);
    let Some(solved) = super::fedemean_slope::absorb_certified(
        design,
        (weights, unit),
        values,
        attempt,
        coefficients,
    )?
    else {
        return Ok(None);
    };
    let mut report = solved.report;
    let reason = match solved.declined {
        Some((decline, _)) => FeFallbackReason::from_decline(decline),
        None if report.certified() => FeFallbackReason::None,
        None => {
            report.warnings.push(
                "the recurrence claimed convergence but the recomputed FE certificate failed"
                    .to_owned(),
            );
            FeFallbackReason::Uncertified
        }
    };
    Ok(Some((report, LsmrRoute::Fedemean, reason)))
}

fn validate_config(config: LsmrConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR iteration budget must be positive",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn lsmr_matches_two_way_additive_projection() {
        let worker = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("worker");
        let firm = EffectTerm::new(vec![0, 1, 0, 1, 0, 1], 2, true, vec![]).expect("firm");
        let design = FeDesign::new(6, vec![worker, firm]).expect("design");
        let weights = vec![1.0; 6];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 2.0, 3.0, 4.0, 5.0]]).expect("values");
        let report = absorb_lsmr(
            &design,
            &weights,
            &mut values,
            LsmrConfig {
                preconditioner: LsmrPreconditioner::Diagonal,
                ..LsmrConfig::default()
            },
        )
        .expect("absorb");
        assert!(report.certified());
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-7));
    }

    #[test]
    fn lsmr_absorbs_group_specific_slopes() {
        let effect = EffectTerm::new(
            vec![0, 0, 0, 1, 1, 1],
            2,
            true,
            vec![vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0]],
        )
        .expect("effect");
        let design = FeDesign::new(6, vec![effect]).expect("design");
        let weights = vec![1.0, 2.0, 1.0, 1.0, 1.0, 2.0];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 3.0, 5.0, 2.0, 1.0, 0.0]]).expect("values");
        let report =
            absorb_lsmr(&design, &weights, &mut values, LsmrConfig::default()).expect("absorb");
        assert!(report.certified());
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-7));
    }

    fn extreme_weight_tree_fixture() -> (FeDesign, Vec<f64>, DenseMatrix) {
        let first = EffectTerm::new(vec![0, 0, 1, 1, 1], 2, true, vec![]).expect("first");
        let second = EffectTerm::new(vec![0, 0, 0, 1, 1], 2, true, vec![]).expect("second");
        let design = FeDesign::new(5, vec![first, second]).expect("design");
        let tiny = 1e-5_f64 * 1e-5_f64 / 3.0;
        let weights = vec![tiny, 1.0, tiny, tiny, 1.0];
        let values = DenseMatrix::from_columns(&[vec![1.0, 0.0, 1.0, 1.0, 0.0]]).expect("values");
        (design, weights, values)
    }

    #[test]
    fn extreme_weight_tree_matches_analytic_projection_with_auto_and_off() {
        // Preserve the historical roundoff fixture as a scientific witness.
        // The three observed cells form an acyclic bipartite design, so their
        // weighted means give its exact independent FE projection. Whether a
        // particular arithmetic implementation needs a positivity retry is
        // incidental; both returned projections must satisfy this certificate.
        let (design, weights, original) = extreme_weight_tree_fixture();
        let mean = weights[0] / (weights[0] + weights[1]);
        let analytic = [1.0 - mean, -mean, 0.0, 1.0 - mean, -mean];
        for preconditioner in [LsmrPreconditioner::Auto, LsmrPreconditioner::Off] {
            let mut values = original.clone();
            let report = absorb_lsmr(
                &design,
                &weights,
                &mut values,
                LsmrConfig {
                    tolerance: 1e-10,
                    preconditioner,
                    ..LsmrConfig::default()
                },
            )
            .expect("certified weighted projection");
            assert!(report.certified());
            assert!(values
                .as_slice()
                .iter()
                .zip(analytic)
                .all(|(actual, expected)| (actual - expected).abs() < 1e-14));
        }
    }
}
