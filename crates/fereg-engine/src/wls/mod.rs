//! One finite weighted least-squares solve and canonical coefficient-slot map.
mod certificate;
mod cholesky;
mod moments;
use crate::{
    data::{DenseMatrix, Weights},
    error::{reserved, ErrorCode, FeregError, Result},
    linalg::{ColumnScaling, ScaledDesign, WeightedQr},
    threads::ThreadTeam,
};
use cholesky::Cholesky;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Ordinary,
    Base,
    ExplicitlyOmitted,
    AbsorbedIntercept,
}
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub role: Role,
    /// Weighted norm before FE projection, in this same sample and weight frame.
    pub reference_norm: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Kept,
    Base,
    ExplicitlyOmitted,
    AbsorbedInterceptDeferred,
    CollinearWithFixedEffects,
    NumericallyRankDeficient,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Empty,
    GuardedCholesky,
    ColumnPivotedQr,
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SolveOverride {
    #[default]
    Automatic,
    Qr,
    Cholesky,
}
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub rank_multiplier: f64,
    pub fe_collinearity: f64,
    pub score_tolerance: f64,
    pub bread_tolerance: f64,
    pub override_method: SolveOverride,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            rank_multiplier: 128.,
            fe_collinearity: 1e-9,
            score_tolerance: 5e-10,
            bread_tolerance: 5e-10,
            override_method: SolveOverride::Automatic,
        }
    }
}
pub struct Problem<'a> {
    pub design: &'a DenseMatrix,
    pub outcome: &'a [f64],
    pub weights: Weights<'a>,
    pub slots: &'a [Slot],
    pub has_absorbed_intercept: bool,
}
#[derive(Debug, PartialEq)]
pub struct Fit {
    pub coefficients: Vec<f64>,
    pub bread: DenseMatrix,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub statuses: Vec<Status>,
    pub kept: Vec<usize>,
    pub method: Method,
    pub relative_score: f64,
    pub inverse_error: f64,
    pub refinements: usize,
}
struct Candidate {
    coefficients: DenseMatrix,
    scaled_bread: DenseMatrix,
    kept: Vec<usize>,
    method: Method,
    refinements: usize,
}
struct Products {
    coefficients: Vec<f64>,
    bread: DenseMatrix,
    fitted: Vec<f64>,
    residuals: Vec<f64>,
    score: f64,
    inverse_error: f64,
}

pub fn solve(problem: &Problem<'_>, policy: Policy, team: &ThreadTeam) -> Result<Fit> {
    let threshold = validate(problem, policy)?;
    let scaled = ScaledDesign::new(copy_matrix(problem.design)?, problem.weights)?;
    let (columns, mut statuses) = select(problem, &scaled.scaling, policy)?;
    let mut scaled = scaled.select_columns(&columns)?;
    let mut references = reserved(columns.len())?;
    references.extend(
        columns
            .iter()
            .map(|&column| problem.slots[column].reference_norm),
    );
    scaled.reference_scales(&references)?;
    let rhs = outcome_matrix(problem.outcome)?;
    let (weighted_rhs, rhs_scales) = scaled.scaling.rhs(copy_matrix(&rhs)?)?;
    let (gram, cross) = moments::moments(&scaled.matrix, &weighted_rhs, team)?;
    if policy.override_method != SolveOverride::Qr {
        if let Some(factor) = Cholesky::factor(&gram, threshold)? {
            let mut coefficients = factor.solve(&cross)?;
            for column in 0..columns.len() {
                coefficients[(column, 0)] =
                    scaled
                        .scaling
                        .coefficient(coefficients[(column, 0)], column, rhs_scales[0]);
            }
            let mut kept = reserved(columns.len())?;
            kept.extend(0..columns.len());
            let candidate = Candidate {
                coefficients,
                scaled_bread: factor.solve(&identity(columns.len())?)?,
                kept,
                refinements: 0,
                method: if columns.is_empty() {
                    Method::Empty
                } else {
                    Method::GuardedCholesky
                },
            };
            if let certificate::CandidateCheck::Pass(products) = certificate::assess(
                problem,
                &columns,
                &scaled.scaling,
                &gram,
                &candidate,
                policy,
            )? {
                return finish(products, candidate, columns, statuses);
            }
        }
        if policy.override_method == SolveOverride::Cholesky {
            return Err(numerical(
                "forced Cholesky did not pass its rank or scientific guard",
            ));
        }
    }
    let factor = WeightedQr::from_scaled(scaled, threshold)?;
    let mut kept = reserved(factor.rank())?;
    kept.extend_from_slice(factor.kept());
    let mut candidate = Candidate {
        coefficients: factor.solve(rhs)?,
        scaled_bread: factor.scaled_inverse()?,
        kept,
        refinements: 0,
        method: if columns.is_empty() {
            Method::Empty
        } else {
            Method::ColumnPivotedQr
        },
    };
    let products = certify_qr(problem, &columns, &gram, &factor, &mut candidate, policy)?;
    for &column in &columns {
        statuses[column] = Status::NumericallyRankDeficient;
    }
    finish(products, candidate, columns, statuses)
}
fn certify_qr(
    problem: &Problem<'_>,
    columns: &[usize],
    gram: &DenseMatrix,
    factor: &WeightedQr,
    candidate: &mut Candidate,
    policy: Policy,
) -> Result<Products> {
    let mut previous = f64::INFINITY;
    for refinement in 0..=2 {
        candidate.refinements = refinement;
        match certificate::assess(problem, columns, factor.scaling(), gram, candidate, policy)? {
            certificate::CandidateCheck::Pass(products) => return Ok(products),
            certificate::CandidateCheck::Fail { score, inverse } => {
                if refinement == 2
                    || !score.is_finite()
                    || score >= previous
                    || inverse > policy.bread_tolerance
                {
                    return Err(FeregError::new(ErrorCode::NonConvergence, format!("finite QR certificate failed (score={score:e}, inverse={inverse:e}, refinements={refinement})")));
                }
                previous = score;
            }
        }
        // Residual refinement reuses the same rank and factor; it cannot alter
        // omission decisions or replace the independent acceptance thresholds.
        let (_, residuals) = certificate::reconstruct(problem, columns, &candidate.coefficients)?;
        let correction = factor.solve(outcome_matrix(&residuals)?)?;
        for (value, &correction) in candidate
            .coefficients
            .as_mut_slice()
            .iter_mut()
            .zip(correction.as_slice())
        {
            *value += correction;
        }
    }
    unreachable!("bounded refinement always returns an accepted fit or error")
}

fn finish(
    products: Products,
    candidate: Candidate,
    columns: Vec<usize>,
    mut statuses: Vec<Status>,
) -> Result<Fit> {
    let mut kept = reserved(candidate.kept.len())?;
    for local in candidate.kept {
        statuses[columns[local]] = Status::Kept;
        kept.push(columns[local]);
    }
    kept.sort_unstable();
    Ok(Fit {
        coefficients: products.coefficients,
        bread: products.bread,
        fitted: products.fitted,
        residuals: products.residuals,
        statuses,
        kept,
        method: candidate.method,
        relative_score: products.score,
        inverse_error: products.inverse_error,
        refinements: candidate.refinements,
    })
}
fn select(
    problem: &Problem<'_>,
    scaling: &ColumnScaling,
    policy: Policy,
) -> Result<(Vec<usize>, Vec<Status>)> {
    let mut columns = reserved(problem.slots.len())?;
    let mut statuses = reserved(problem.slots.len())?;
    for (column, slot) in problem.slots.iter().enumerate() {
        let mut status = match slot.role {
            Role::Ordinary => Status::NumericallyRankDeficient,
            Role::Base => Status::Base,
            Role::ExplicitlyOmitted => Status::ExplicitlyOmitted,
            Role::AbsorbedIntercept => Status::AbsorbedInterceptDeferred,
        };
        if slot.role == Role::Ordinary && slot.reference_norm == 0. && scaling.norms[column] != 0. {
            return Err(invalid(
                "a zero reference norm has a nonzero finite regressor",
            ));
        }
        if slot.role == Role::Ordinary && slot.reference_norm > 0. {
            let norm = (scaling.maxima[column] * scaling.norms[column]) * scaling.root_scale;
            let ratio = norm / slot.reference_norm;
            let squared_ratio = ratio * ratio;
            if !squared_ratio.is_finite() {
                return Err(numerical(
                    "finite regressor reference ratio is not representable",
                ));
            }
            if problem.has_absorbed_intercept && squared_ratio <= policy.fe_collinearity {
                status = Status::CollinearWithFixedEffects;
            } else {
                columns.push(column);
            }
        }
        statuses.push(status);
    }
    Ok((columns, statuses))
}
fn validate(problem: &Problem<'_>, policy: Policy) -> Result<f64> {
    let n = problem.design.nrows();
    let p = problem.design.ncols();
    if problem.outcome.len() != n || problem.slots.len() != p {
        return Err(invalid("finite WLS dimensions differ"));
    }
    problem.weights.validate(n)?;
    if n == 0 {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "finite WLS requires observations",
        ));
    }
    if problem.outcome.iter().any(|v| !v.is_finite())
        || problem
            .slots
            .iter()
            .any(|s| !s.reference_norm.is_finite() || s.reference_norm < 0.)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "invalid finite WLS outcome or reference norm",
        ));
    }
    if !policy.rank_multiplier.is_finite()
        || policy.rank_multiplier <= 0.
        || [
            policy.fe_collinearity,
            policy.score_tolerance,
            policy.bread_tolerance,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0. || *v >= 1.)
    {
        return Err(invalid("invalid finite WLS policy"));
    }
    let threshold = policy.rank_multiplier * f64::EPSILON * n.max(p).max(1) as f64;
    if !threshold.is_finite() || threshold >= 1. {
        return Err(invalid("finite WLS rank threshold is invalid"));
    }
    Ok(threshold)
}
fn identity(p: usize) -> Result<DenseMatrix> {
    let mut matrix = DenseMatrix::zeros(p, p)?;
    for i in 0..p {
        matrix[(i, i)] = 1.;
    }
    Ok(matrix)
}
fn copy_matrix(matrix: &DenseMatrix) -> Result<DenseMatrix> {
    let mut data = reserved(matrix.as_slice().len())?;
    data.extend_from_slice(matrix.as_slice());
    DenseMatrix::from_row_major(matrix.nrows(), matrix.ncols(), data)
}
fn outcome_matrix(outcome: &[f64]) -> Result<DenseMatrix> {
    let mut data = reserved(outcome.len())?;
    data.extend_from_slice(outcome);
    DenseMatrix::from_row_major(outcome.len(), 1, data)
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
