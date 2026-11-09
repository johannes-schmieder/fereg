use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    certify_residuals_with_reference_norms, weighted_column_norms, FeDesign, FeSolverKind,
    LsmrPreconditioner, SolveReport, SolveStopReason,
};
use crate::options::ThreadConfig;
use within::{Effect, LsmrOptions, PreconditionerConfig};

/// Modified-LSMR tuning for fixed-effect absorption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LsmrConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    /// Windowed modified Gram-Schmidt history of the LSMR memory estimates
    /// (fedemean's modified LSMR runs without reorthogonalization). `None`
    /// disables it.
    pub local_size: Option<usize>,
    pub preconditioner: LsmrPreconditioner,
    pub threads: ThreadConfig,
}

impl Default for LsmrConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 16_000,
            local_size: Some(4),
            preconditioner: LsmrPreconditioner::Auto,
            threads: ThreadConfig::default(),
        }
    }
}

fn reserved_empty<T>(elements: usize, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    Ok(output)
}

/// Residualize one or more right-hand sides with fedemean's modified LSMR on
/// the full system (any supported FE grammar, including slopes).
///
/// All input materialization and all output writes occur on the caller thread.
/// Numerical work uses the caller's current Rayon pool, which is the isolated
/// action-local `ThreadTeam` in public estimation. It never has access to
/// Stata's SPI.
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
        Some((report, _)) => Ok(report),
        None => absorb_within(design, weights, values, config, certificate_tolerance),
    }
}

/// Which implementation produced [`absorb_lsmr_with`]'s report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LsmrRoute {
    /// fedemean's full system (its coefficients, when requested, are filled).
    Fedemean,
    /// within's modified LSMR (no coefficients), or no fixed effects.
    Within,
}

/// [`absorb_lsmr_certified_at`] with fedemean's `memory_limit` and, with
/// `coefficients` (the design's layout, `values.ncols()` columns), the fitted
/// FE coefficients added in place when fedemean ran. `None`: fedemean is not
/// eligible for the design (for example an effect with more channels than it
/// takes); the values are untouched for the caller's established route.
pub(crate) fn absorb_lsmr_with(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
    (certificate_tolerance, memory_limit): (f64, Option<u128>),
    coefficients: Option<(&super::fedemean_slope::Layout, &mut [f64])>,
) -> Result<Option<(SolveReport, LsmrRoute)>> {
    crate::interrupt::checkpoint()?;
    validate_config(config)?;
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
            requested_preconditioner: Some(config.preconditioner),
            used_preconditioner: Some(resolve_preconditioner_kind(design, config.preconditioner)),
            ..SolveReport::single_route(
                FeSolverKind::Lsmr,
                SolveStopReason::NoFixedEffects,
                certificate,
            )
        };
        return Ok(Some((report, LsmrRoute::Within)));
    }
    if within_schwarz(design, config.preconditioner) {
        return absorb_within(design, weights, values, config, certificate_tolerance)
            .map(|report| Some((report, LsmrRoute::Within)));
    }
    let unit = weights.iter().all(|&weight| weight == 1.0);
    let attempt = super::fedemean_slope::Attempt::lsmr(
        config.preconditioner,
        FeSolverKind::Lsmr,
        certificate_tolerance,
        config.max_iterations,
        memory_limit,
    );
    let Some(solved) = super::fedemean_slope::absorb_certified(
        design,
        (weights, unit),
        values,
        &attempt,
        coefficients,
    )?
    else {
        return Ok(None);
    };
    let mut report = solved.report;
    if solved.declined.is_none() && !report.certified() {
        report.warnings.push(
            "LSMR recurrence claimed convergence but the recomputed FE certificate failed"
                .to_owned(),
        );
    }
    Ok(Some((report, LsmrRoute::Fedemean)))
}

/// Explicit additive Schwarz on a slope design with fewer than two effects
/// with intercepts: fedemean's hybrid Schwarz needs two (its factor-pair
/// subdomains live on intercept channels), so this one explicit combination
/// keeps within's additive Schwarz, whose subdomains include slope channels.
fn within_schwarz(design: &FeDesign, preconditioner: LsmrPreconditioner) -> bool {
    preconditioner == LsmrPreconditioner::Schwarz
        && super::fedemean_slope::sloped(design)
        && design
            .effects()
            .iter()
            .filter(|effect| effect.has_intercept())
            .count()
            < 2
}

fn build_effects(design: &FeDesign) -> Result<Vec<Effect<'_>>> {
    let mut effects = reserved_empty(
        design.effects().len(),
        "unable to reserve LSMR fixed-effect views",
    )?;
    for effect in design.effects() {
        effects.push(
            Effect::new(
                effect.codes(),
                effect.has_intercept(),
                effect.slopes().iter().map(Vec::as_slice),
            )
            .map_err(|error| {
                FeregError::new(
                    ErrorCode::InvalidData,
                    format!("unable to construct LSMR fixed-effect term: {error}"),
                )
            })?,
        );
    }
    Ok(effects)
}

/// within's modified LSMR with the requested preconditioner (the automatic
/// one resolved as before), its recurrence at a hundredth of the request
/// (floored at 1e-15, never looser) and fereg's independent certificate at
/// the request: explicit additive Schwarz with fewer than two intercept
/// effects (see [`within_schwarz`]), and the established route for a slope
/// fit fedemean does not certify.
pub(crate) fn absorb_within(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
    certificate_tolerance: f64,
) -> Result<SolveReport> {
    let reference_norms = weighted_column_norms(values, weights)?;
    let rhs = super::lsmr_batch::rhs_columns(values)?;
    let mut rhs_views = reserved_empty(
        rhs.len(),
        "unable to reserve borrowed LSMR right-hand-side views",
    )?;
    for column in &rhs {
        rhs_views.push(column.as_slice());
    }
    let options = LsmrOptions {
        tol: (certificate_tolerance * 1e-2)
            .max(1e-15)
            .min(certificate_tolerance),
        maxiter: config.max_iterations,
        local_size: config.local_size,
    };
    let resolved = resolve_preconditioner_kind(design, config.preconditioner);
    let solved = super::lsmr_batch::solve_batch(
        build_effects(design)?,
        &rhs_views,
        Some(weights),
        &options,
        match resolved {
            LsmrPreconditioner::Off => PreconditionerConfig::Off,
            LsmrPreconditioner::Diagonal => PreconditionerConfig::Diagonal,
            _ => additive_schwarz(),
        },
        config.threads.deterministic,
    )
    .map_err(|error| error.into_fereg("modified LSMR failed"))?;
    super::lsmr_batch::copy_demeaned(&solved.demeaned, values);
    let certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        certificate_tolerance,
    )?;
    let converged = solved.converged.iter().all(|&value| value);
    let stop_reason = if certificate.passed {
        SolveStopReason::Certified
    } else if converged {
        SolveStopReason::Uncertified
    } else {
        SolveStopReason::MaximumIterations
    };
    let mut warnings =
        reserved_empty(solved.warnings.len() + 1, "unable to reserve LSMR warnings")?;
    for warning in &solved.warnings {
        warnings.push(format!("within preconditioner: {warning:?}"));
    }
    if converged && !certificate.passed {
        warnings.push(
            "LSMR recurrence claimed convergence but the recomputed FE certificate failed"
                .to_owned(),
        );
    }
    Ok(SolveReport {
        iterations: solved.iterations.iter().copied().max().unwrap_or(0),
        normal_equation_residual: solved.residual.iter().copied().fold(0.0_f64, f64::max),
        setup_seconds: solved.time_setup,
        solve_seconds: solved.time_solve.iter().copied().sum(),
        requested_preconditioner: Some(config.preconditioner),
        used_preconditioner: Some(resolved),
        warnings,
        ..SolveReport::single_route(FeSolverKind::Lsmr, stop_reason, certificate)
    })
}

pub(crate) fn resolve_preconditioner_kind(
    design: &FeDesign,
    requested: LsmrPreconditioner,
) -> LsmrPreconditioner {
    match requested {
        LsmrPreconditioner::Off => LsmrPreconditioner::Off,
        LsmrPreconditioner::Diagonal => LsmrPreconditioner::Diagonal,
        LsmrPreconditioner::Schwarz => LsmrPreconditioner::Schwarz,
        LsmrPreconditioner::Auto => {
            if design.effects().len() >= 2 && design.total_coefficients() >= 256 {
                LsmrPreconditioner::Schwarz
            } else {
                LsmrPreconditioner::Diagonal
            }
        }
    }
}

/// Explicit additive Schwarz for fereg's own routing policy. Upstream's default
/// is adaptive; inheriting it would add a second, independent escalation policy.
/// The upstream owner reduction is repeatable at a fixed worker count.
pub(crate) fn additive_schwarz() -> PreconditionerConfig {
    PreconditionerConfig::Additive {
        local_solver: within::LocalSolverConfig::default(),
    }
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
    if config.local_size == Some(0) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR reorthogonalization window must be positive when supplied",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn schwarz_configuration_never_inherits_upstream_adaptive_policy() {
        assert!(matches!(
            additive_schwarz(),
            PreconditionerConfig::Additive { .. }
        ));
    }

    #[test]
    fn lsmr_setup_capacity_overflow_is_an_allocation_error() {
        let error = reserved_empty::<u8>(usize::MAX, "injected LSMR allocation")
            .expect_err("capacity overflow");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
    }

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
