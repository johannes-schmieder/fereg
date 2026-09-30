//! One score/bread covariance path with explicit OLS and PPML policies.
#[cfg(test)]
mod interruption_tests;
pub mod psd;
mod reduce;
mod wald;
use crate::{
    data::{DenseMatrix, ReferenceWeights, WeightKind},
    dof::ResidualDof,
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    factor::{factorize_code_interaction_polled, CategoricalCodeView},
    spec::Estimator,
    threads::ThreadTeam,
    topology::LevelIndex,
};
use rayon::prelude::*;
pub(crate) use wald::{coefficient_statistics_polled, wald_polled};
pub use wald::{wald, Wald};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Iid,
    Robust,
    Cluster,
}
/// Resolve OLS metadata before loading; numerical covariance stays on estimate.
pub(crate) fn ols_kind(
    requested: Option<Kind>,
    weight: Option<WeightKind>,
    clusters: usize,
) -> Result<Kind> {
    let kind = requested.unwrap_or(if clusters > 0 {
        Kind::Cluster
    } else if weight == Some(WeightKind::Probability) {
        Kind::Robust
    } else {
        Kind::Iid
    });
    if (kind == Kind::Cluster) == (clusters == 0) {
        return Err(invalid("OLS VCE and cluster declarations disagree"));
    }
    if kind == Kind::Iid && weight == Some(WeightKind::Probability) {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "probability-weight IID covariance is unsupported",
        ));
    }
    Ok(kind)
}
#[derive(Debug)]
pub struct Problem<'a> {
    /// Reported coordinates, including a reconstructed constant where requested.
    pub design: &'a DenseMatrix,
    pub bread: &'a DenseMatrix,
    /// OLS residual or PPML y-mu, recomputed at the accepted state.
    pub residual: &'a [f64],
    pub weights: ReferenceWeights<'a>,
    pub clusters: &'a [CategoricalCodeView<'a>],
    pub dof: ResidualDof,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Covariance {
    pub values: DenseMatrix,
    pub standard_errors: Vec<f64>,
    pub correction: f64,
    pub cluster_counts: Vec<usize>,
    pub subset_counts: Vec<usize>,
    pub reconstruction_error: f64,
    pub psd: psd::PsdReport,
}
impl Covariance {
    /// Scatter identified coordinates only. Omitted covariance cells stay
    /// zero and their undefined standard errors remain IEEE NaN for the wire.
    pub fn scatter(self, slots: usize, coordinates: &[usize]) -> Result<Self> {
        self.scatter_polled(slots, coordinates, None)
    }
    pub(crate) fn scatter_polled(
        mut self,
        slots: usize,
        coordinates: &[usize],
        poll: Poll<'_>,
    ) -> Result<Self> {
        execution::check(poll)?;
        let k = coordinates.len();
        if self.values.nrows() != k
            || self.values.ncols() != k
            || self.standard_errors.len() != k
            || coordinates.iter().any(|&slot| slot >= slots)
            || coordinates.windows(2).any(|w| w[0] >= w[1])
        {
            return Err(invalid("covariance scatter coordinates disagree"));
        }
        let mut values = DenseMatrix::zeros(slots, slots)?;
        let mut errors = reserved(slots)?;
        errors.resize(slots, f64::NAN);
        for (i, &left) in coordinates.iter().enumerate() {
            execution::check(poll)?;
            errors[left] = self.standard_errors[i];
            for (j, &right) in coordinates.iter().enumerate() {
                execution::check_chunk(poll, j)?;
                values[(left, right)] = self.values[(i, j)];
            }
        }
        self.values = values;
        self.standard_errors = errors;
        Ok(self)
    }
}
pub fn estimate(
    problem: &Problem<'_>,
    kind: Kind,
    tolerance: f64,
    team: &ThreadTeam,
) -> Result<Covariance> {
    estimate_polled(problem, kind, tolerance, team, None)
}
pub(crate) fn estimate_polled(
    problem: &Problem<'_>,
    kind: Kind,
    tolerance: f64,
    team: &ThreadTeam,
    poll: Poll<'_>,
) -> Result<Covariance> {
    execution::check(poll)?;
    validate(problem, kind, tolerance, poll)?;
    let p = problem.design.ncols();
    let mut cluster_counts = reserved(problem.clusters.len())?;
    for cluster in problem.clusters {
        execution::check(poll)?;
        cluster_counts.push(factorize_code_interaction_polled(&[*cluster], poll)?.n_levels);
        execution::check(poll)?;
    }
    let dof = ResidualDof::new(
        problem.dof.estimator,
        problem.weights.effective(),
        problem.dof.finite,
        problem.dof.absorbed,
        problem.dof.nested,
        &cluster_counts,
    )?;
    if dof != problem.dof {
        return Err(invalid(
            "VCE degrees of freedom disagree with the final sample",
        ));
    }
    let mut subsets = reserved((1usize << problem.clusters.len()) - 1)?;
    let (mut raw, mut replay, correction) = match kind {
        Kind::Iid => {
            let rss =
                crate::statistics::squares_polled(problem.residual, 0., problem.weights, poll)?;
            let mut copy = DenseMatrix::zeros(p, p)?;
            copy.as_mut_slice()
                .copy_from_slice(problem.bread.as_slice());
            let mut replay = DenseMatrix::zeros(p, p)?;
            for i in 0..p {
                execution::check(poll)?;
                for j in 0..p {
                    execution::check_chunk(poll, j)?;
                    replay[(i, j)] = problem.bread[(j, i)];
                }
            }
            (copy, replay, rss / dof.structural as f64)
        }
        Kind::Robust => {
            let scores = |row: usize, col: usize| {
                problem.weights.robust_root(row)
                    * problem.residual[row]
                    * problem.design[(row, col)]
            };
            let meat = reduce::meat(problem.design.nrows(), p, scores, team, poll)?;
            (
                reduce::sandwich(problem.bread, &meat, poll)?,
                reduce::influence_meat(
                    problem.design.nrows(),
                    problem.bread,
                    |i, j| Ok(scores(i, j)),
                    team,
                    poll,
                )?,
                dof.robust_multiplier()?,
            )
        }
        Kind::Cluster => {
            let (meat, replay) = clustered(problem, &mut subsets, team, poll)?;
            (
                reduce::sandwich(problem.bread, &meat, poll)?,
                replay,
                dof.cluster_multiplier()?,
            )
        }
    };
    for matrix in [&mut raw, &mut replay] {
        for (position, value) in matrix.as_mut_slice().iter_mut().enumerate() {
            execution::check_chunk(poll, position)?;
            *value *= correction;
        }
        reduce::finite(matrix, poll)?;
    }
    let reconstruction_error = reduce::difference(&raw, &replay, poll)?;
    if !reconstruction_error.is_finite() || reconstruction_error > tolerance {
        return Err(numerical(format!(
            "covariance replay failed: {reconstruction_error:.3e}"
        )));
    }
    let (values, psd) =
        psd::apply_psd_policy_polled(raw.as_slice(), p, problem.clusters.len(), tolerance, poll)?;
    let values = DenseMatrix::from_row_major(p, p, values)?;
    let mut standard_errors = reserved(p)?;
    for i in 0..p {
        execution::check_chunk(poll, i)?;
        let diagonal = values[(i, i)];
        if diagonal < 0. || !diagonal.is_finite() {
            return Err(numerical("covariance has a negative/nonfinite diagonal"));
        }
        standard_errors.push(diagonal.sqrt());
    }
    Ok(Covariance {
        values,
        standard_errors,
        correction,
        cluster_counts,
        subset_counts: subsets,
        reconstruction_error,
        psd,
    })
}
fn clustered(
    problem: &Problem<'_>,
    counts: &mut Vec<usize>,
    team: &ThreadTeam,
    poll: Poll<'_>,
) -> Result<(DenseMatrix, DenseMatrix)> {
    let p = problem.design.ncols();
    let mut meat = DenseMatrix::zeros(p, p)?;
    let mut correction = DenseMatrix::zeros(p, p)?;
    let mut replay = DenseMatrix::zeros(p, p)?;
    let mut replay_correction = DenseMatrix::zeros(p, p)?;
    let mut components = reserved(problem.clusters.len())?;
    for mask in 1usize..1usize << problem.clusters.len() {
        execution::check(poll)?;
        components.clear();
        components.extend(
            problem
                .clusters
                .iter()
                .enumerate()
                .filter_map(|(j, c)| ((mask >> j) & 1 == 1).then_some(*c)),
        );
        let encoded = factorize_code_interaction_polled(&components, poll)?;
        execution::check(poll)?;
        let membership = LevelIndex::from_codes_polled(&encoded.codes, encoded.n_levels, poll)?;
        counts.push(encoded.n_levels);
        let mut scores = DenseMatrix::zeros(encoded.n_levels, p)?;
        if p > 0 {
            team.install(|| {
                scores
                    .as_mut_slice()
                    .par_chunks_mut(p)
                    .enumerate()
                    .try_for_each(|(g, values)| -> Result<()> {
                        execution::check(poll)?;
                        let rows = membership.members(g).expect("canonical cluster level");
                        for (j, value) in values.iter_mut().enumerate() {
                            *value = execution::sum(
                                rows.iter().map(|&i| {
                                    problem.weights.value(i)
                                        * problem.residual[i]
                                        * problem.design[(i, j)]
                                }),
                                poll,
                            )?;
                        }
                        Ok(())
                    })
            })?;
        }
        reduce::finite(&scores, poll)?;
        let sign = if mask.count_ones() % 2 == 1 { 1. } else { -1. };
        let subset = reduce::meat(encoded.n_levels, p, |i, j| scores[(i, j)], team, poll)?;
        let independent = reduce::influence_meat(
            encoded.n_levels,
            problem.bread,
            |i, j| {
                // Replay the underlying observations too, rather than trust stored group sums.
                execution::sum(
                    membership
                        .members(i)
                        .expect("cluster")
                        .iter()
                        .rev()
                        .map(|&row| {
                            problem.design[(row, j)]
                                * (problem.residual[row] * problem.weights.value(row))
                        }),
                    poll,
                )
            },
            team,
            poll,
        )?;
        reduce::add(&mut meat, &mut correction, &subset, sign, poll)?;
        reduce::add(
            &mut replay,
            &mut replay_correction,
            &independent,
            sign,
            poll,
        )?;
    }
    reduce::finish(&mut meat, &correction, poll)?;
    reduce::finish(&mut replay, &replay_correction, poll)?;
    Ok((meat, replay))
}
fn validate(problem: &Problem<'_>, kind: Kind, tolerance: f64, poll: Poll<'_>) -> Result<()> {
    let n = problem.design.nrows();
    let p = problem.design.ncols();
    let estimator = problem.dof.estimator;
    let limit = if estimator == Estimator::Ols { 10 } else { 5 };
    if n == 0
        || n != problem.weights.nobs()
        || problem.residual.len() != n
        || problem.bread.nrows() != p
        || problem.bread.ncols() != p
        || problem.clusters.len() > limit
        || problem.clusters.iter().any(|c| c.codes.len() != n)
        || !tolerance.is_finite()
        || tolerance <= 0.
        || tolerance >= 1.
    {
        return Err(invalid("invalid covariance inputs"));
    }
    for (row, value) in problem.residual.iter().enumerate() {
        execution::check_chunk(poll, row)?;
        if !value.is_finite() {
            return Err(invalid("invalid covariance inputs"));
        }
    }
    reduce::finite(problem.design, poll)?;
    reduce::finite(problem.bread, poll)?;
    if (kind == Kind::Cluster) == problem.clusters.is_empty() {
        return Err(invalid("VCE and cluster declarations disagree"));
    }
    if kind == Kind::Iid
        && (estimator == Estimator::Ppml || problem.weights.kind() == Some(WeightKind::Probability))
    {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "IID covariance is unsupported for this estimator/weight",
        ));
    }
    for i in 0..p {
        execution::check(poll)?;
        for j in 0..i {
            execution::check_chunk(poll, j)?;
            let scale = problem.bread[(i, j)]
                .abs()
                .max(problem.bread[(j, i)].abs())
                .max(f64::MIN_POSITIVE);
            if (problem.bread[(i, j)] - problem.bread[(j, i)]).abs() > tolerance * scale {
                return Err(numerical("bread is not symmetric"));
            }
        }
    }
    Ok(())
}
fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
