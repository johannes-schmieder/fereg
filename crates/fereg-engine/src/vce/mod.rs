//! One score/bread covariance path with explicit OLS and PPML policies.
pub mod psd;
mod reduce;
mod wald;
use crate::{
    data::{DenseMatrix, ReferenceWeights, WeightKind},
    dof::ResidualDof,
    error::{reserved, ErrorCode, FeregError, Result},
    factor::{factorize_code_interaction, CategoricalCodeView},
    spec::Estimator,
    threads::{compensated_sum, ThreadTeam},
    topology::LevelIndex,
};
use rayon::prelude::*;
pub use wald::{wald, Wald};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Iid,
    Robust,
    Cluster,
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
pub fn estimate(
    problem: &Problem<'_>,
    kind: Kind,
    tolerance: f64,
    team: &ThreadTeam,
) -> Result<Covariance> {
    validate(problem, kind, tolerance)?;
    let p = problem.design.ncols();
    let mut cluster_counts = reserved(problem.clusters.len())?;
    for cluster in problem.clusters {
        cluster_counts.push(factorize_code_interaction(&[*cluster])?.n_levels);
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
            let rss = crate::statistics::squares(problem.residual, 0., problem.weights)?;
            let mut copy = DenseMatrix::zeros(p, p)?;
            copy.as_mut_slice()
                .copy_from_slice(problem.bread.as_slice());
            let mut replay = DenseMatrix::zeros(p, p)?;
            for i in 0..p {
                for j in 0..p {
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
            let meat = reduce::meat(problem.design.nrows(), p, scores, team)?;
            (
                reduce::sandwich(problem.bread, &meat)?,
                reduce::influence_meat(problem.design.nrows(), problem.bread, scores, team)?,
                dof.robust_multiplier()?,
            )
        }
        Kind::Cluster => {
            let (meat, replay) = clustered(problem, &mut subsets, team)?;
            (
                reduce::sandwich(problem.bread, &meat)?,
                replay,
                dof.cluster_multiplier()?,
            )
        }
    };
    for matrix in [&mut raw, &mut replay] {
        for value in matrix.as_mut_slice() {
            *value *= correction;
        }
        reduce::finite(matrix)?;
    }
    let reconstruction_error = reduce::difference(&raw, &replay);
    if !reconstruction_error.is_finite() || reconstruction_error > tolerance {
        return Err(numerical(format!(
            "covariance replay failed: {reconstruction_error:.3e}"
        )));
    }
    let (values, psd) =
        psd::apply_psd_policy(raw.as_slice(), p, problem.clusters.len(), tolerance)?;
    let values = DenseMatrix::from_row_major(p, p, values)?;
    let mut standard_errors = reserved(p)?;
    for i in 0..p {
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
) -> Result<(DenseMatrix, DenseMatrix)> {
    let p = problem.design.ncols();
    let mut meat = DenseMatrix::zeros(p, p)?;
    let mut correction = DenseMatrix::zeros(p, p)?;
    let mut replay = DenseMatrix::zeros(p, p)?;
    let mut replay_correction = DenseMatrix::zeros(p, p)?;
    let mut components = reserved(problem.clusters.len())?;
    for mask in 1usize..1usize << problem.clusters.len() {
        components.clear();
        components.extend(
            problem
                .clusters
                .iter()
                .enumerate()
                .filter_map(|(j, c)| ((mask >> j) & 1 == 1).then_some(*c)),
        );
        let encoded = factorize_code_interaction(&components)?;
        let membership = LevelIndex::from_codes(&encoded.codes, encoded.n_levels)?;
        counts.push(encoded.n_levels);
        let mut scores = DenseMatrix::zeros(encoded.n_levels, p)?;
        if p > 0 {
            team.install(|| {
                scores
                    .as_mut_slice()
                    .par_chunks_mut(p)
                    .enumerate()
                    .for_each(|(g, values)| {
                        let rows = membership.members(g).expect("canonical cluster level");
                        for (j, value) in values.iter_mut().enumerate() {
                            *value = compensated_sum(rows.iter().map(|&i| {
                                problem.weights.value(i)
                                    * problem.residual[i]
                                    * problem.design[(i, j)]
                            }));
                        }
                    })
            });
        }
        reduce::finite(&scores)?;
        let sign = if mask.count_ones() % 2 == 1 { 1. } else { -1. };
        let subset = reduce::meat(encoded.n_levels, p, |i, j| scores[(i, j)], team)?;
        let independent =
            reduce::influence_meat(
                encoded.n_levels,
                problem.bread,
                |i, j| {
                    // Replay the underlying observations too, rather than trust stored group sums.
                    compensated_sum(membership.members(i).expect("cluster").iter().rev().map(
                        |&row| {
                            problem.design[(row, j)]
                                * (problem.residual[row] * problem.weights.value(row))
                        },
                    ))
                },
                team,
            )?;
        reduce::add(&mut meat, &mut correction, &subset, sign)?;
        reduce::add(&mut replay, &mut replay_correction, &independent, sign)?;
    }
    reduce::finish(&mut meat, &correction)?;
    reduce::finish(&mut replay, &replay_correction)?;
    Ok((meat, replay))
}
fn validate(problem: &Problem<'_>, kind: Kind, tolerance: f64) -> Result<()> {
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
        || problem.residual.iter().any(|v| !v.is_finite())
        || !tolerance.is_finite()
        || tolerance <= 0.
        || tolerance >= 1.
    {
        return Err(invalid("invalid covariance inputs"));
    }
    reduce::finite(problem.design)?;
    reduce::finite(problem.bread)?;
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
        for j in 0..i {
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
