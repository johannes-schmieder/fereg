//! SPEC/legacy ADR 0016: accepted-Hessian PPML scores, rank and z/Wald laws.
use super::irls;
use crate::{
    data::{ColumnRecipe, CompiledColumns, ReferenceWeights},
    dof::{compute_absorbed_dof, DofPolicy, DofReport, ResidualDof},
    error::{reserved, ErrorCode, FeregError, Result},
    factor::{factorize_code_interaction, CategoricalCodeView},
    spec::Estimator,
    threads::ThreadTeam,
    vce::{self, Covariance, Kind, Wald},
    wls::{reporting, Status},
};
#[derive(Debug, PartialEq)]
pub struct Inference {
    pub coefficients: Vec<f64>,
    pub statuses: Vec<Status>,
    pub coordinates: Vec<usize>,
    pub covariance: Covariance,
    /// Undefined for omissions, structural normalization or zero variance.
    pub z_statistics: Vec<f64>,
    pub wald: Wald,
    pub dof: ResidualDof,
    pub absorbed_dof: DofReport,
}
pub fn estimate(
    columns: &CompiledColumns,
    fit: &irls::Fit<'_>,
    clusters: &[CategoricalCodeView<'_>],
    kind: Kind,
    tolerance: f64,
    team: &ThreadTeam,
) -> Result<Inference> {
    let problem = fit.problem();
    if columns.implicit_constant || !columns.structural_intercept || columns.roles != problem.roles
    {
        return Err(invalid(
            "PPML report roles or structural constant disagree with the fit",
        ));
    }
    if kind == Kind::Iid {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "PPML IID covariance is unsupported",
        ));
    }
    if clusters.len() > 5 || (kind == Kind::Cluster) == clusters.is_empty() {
        return Err(invalid("PPML VCE and cluster declarations disagree"));
    }
    let q = ReferenceWeights::new(
        &problem.sample.weights,
        problem.sample.n_active(),
        Estimator::Ppml,
    )?;
    let mut reported = reporting::mapped(
        columns,
        problem.raw,
        &fit.linear.within,
        &fit.linear.finite,
        false,
    )?;
    let constant = columns
        .constant_slot
        .ok_or_else(|| invalid("PPML reported constant is missing"))?;
    if columns.recipes[columns.slots[constant].generated.unwrap()] != ColumnRecipe::Intercept
        || !columns.matches(problem.sample, problem.raw)?
    {
        return Err(invalid(
            "PPML report recipes disagree with the fitted raw columns",
        ));
    }
    let normalized = reporting::deferred_slot(columns);
    if let Some(slot) = normalized {
        reported.statuses[slot] = Status::NormalizedAbsorbedIntercept;
    }
    let mut cluster_counts = reserved(clusters.len())?;
    let mut cluster_codes = reserved(clusters.len())?;
    for &view in clusters {
        cluster_counts.push(factorize_code_interaction(&[view])?.n_levels);
        cluster_codes.push(view.codes);
    }
    let mut absorbed_dof =
        compute_absorbed_dof(problem.sample, false, DofPolicy::Pairwise, &cluster_codes)?;
    if normalized.is_some() {
        transfer_constant(&mut absorbed_dof)?;
    }
    let dof = ResidualDof::new(
        Estimator::Ppml,
        q.effective(),
        reported
            .finite_rank
            .checked_add(usize::from(normalized.is_some()))
            .ok_or_else(|| invalid("PPML finite rank overflows"))?,
        absorbed_dof.absorbed_rank,
        absorbed_dof.nested_categories,
        &cluster_counts,
    )?;
    let mut residual = reserved(q.nobs())?;
    residual.extend(
        problem
            .sample
            .outcome
            .iter()
            .zip(&fit.evaluation.mean)
            .map(|(y, mu)| y - mu),
    );
    let covariance = vce::estimate(
        &vce::Problem {
            design: &reported.score_design,
            bread: &reported.bread,
            residual: &residual,
            weights: q,
            clusters,
            dof,
        },
        kind,
        tolerance,
        team,
    )?
    .scatter(reported.coefficients.len(), &reported.coordinates)?;
    let z_statistics = vce::coefficient_statistics(
        &reported.coefficients,
        &covariance.standard_errors,
        &reported.coordinates,
    )?;
    let mut tested = reserved(reported.coordinates.len())?;
    tested.extend(
        reported
            .coordinates
            .iter()
            .copied()
            .filter(|&slot| Some(slot) != columns.constant_slot),
    );
    let wald = vce::wald(
        &reported.coefficients,
        &covariance.values,
        &tested,
        Estimator::Ppml,
        tolerance,
    )?;
    Ok(Inference {
        coefficients: reported.coefficients,
        statuses: reported.statuses,
        coordinates: reported.coordinates,
        covariance,
        z_statistics,
        wald,
        dof,
        absorbed_dof,
    })
}
fn transfer_constant(report: &mut DofReport) -> Result<()> {
    // This is the pinned PPML allocation of the normalization direction,
    // not an OLS covariance correction. Nested intercepts are already removed.
    if let Some(effect) = report
        .effects
        .iter_mut()
        .find(|e| e.slope_source.is_none() && !e.nested && e.coefficients > 0)
    {
        effect.redundant = effect
            .redundant
            .checked_add(1)
            .ok_or_else(|| invalid("PPML redundancy count overflows"))?;
        effect.coefficients -= 1;
        report.redundant_categories = report
            .redundant_categories
            .checked_add(1)
            .ok_or_else(|| invalid("PPML redundancy total overflows"))?;
        report.absorbed_rank = report
            .absorbed_rank
            .checked_sub(1)
            .ok_or_else(|| invalid("PPML normalization exceeds absorbed rank"))?;
    }
    Ok(())
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
