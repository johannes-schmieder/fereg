//! Full-design robust/cluster sandwich, independently grouped by exact tuples.
use super::{algebra::*, PpmlProblem, PpmlWeights, Prepared};
use std::collections::BTreeMap;

pub(super) fn covariance(
    problem: &PpmlProblem,
    prepared: &Prepared,
    mean: &[f64],
    bread: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>, String> {
    let k = bread.len();
    let scores = prepared
        .x
        .iter()
        .zip(&problem.outcome)
        .zip(mean)
        .zip(&prepared.q)
        .map(|(((row, y), mu), q)| row.iter().map(|v| q * (y - mu) * v).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut meat = vec![vec![0.0; k]; k];
    let correction = if problem.clusters.is_empty() {
        for (i, row) in scores.iter().enumerate() {
            let multiplier = if matches!(problem.weights, PpmlWeights::Frequency(_)) {
                prepared.q[i].recip()
            } else {
                1.0
            };
            add_outer(&mut meat, row, multiplier);
        }
        prepared.effective_n as f64 / (prepared.effective_n - 1) as f64
    } else {
        let mut minimum_groups = usize::MAX;
        for mask in 1usize..(1 << problem.clusters.len()) {
            let mut groups: BTreeMap<Vec<u64>, Vec<f64>> = BTreeMap::new();
            for (i, row) in scores.iter().enumerate() {
                let key = problem
                    .clusters
                    .iter()
                    .enumerate()
                    .filter_map(|(j, column)| ((mask & (1 << j)) != 0).then_some(column[i]))
                    .collect();
                let aggregate = groups.entry(key).or_insert_with(|| vec![0.0; k]);
                for (sum, value) in aggregate.iter_mut().zip(row) {
                    *sum += value;
                }
            }
            if mask.count_ones() == 1 {
                minimum_groups = minimum_groups.min(groups.len());
            }
            let sign = if mask.count_ones() % 2 == 1 {
                1.0
            } else {
                -1.0
            };
            for row in groups.values() {
                add_outer(&mut meat, row, sign);
            }
        }
        if minimum_groups < 2 {
            return Err("Poisson cluster dimension has fewer than two groups".into());
        }
        minimum_groups as f64 / (minimum_groups - 1) as f64
    };
    let mut result = multiply(&multiply(bread, &meat), bread);
    for value in result.iter_mut().flatten() {
        *value *= correction;
    }
    Ok(result)
}

fn add_outer(matrix: &mut [Vec<f64>], score: &[f64], multiplier: f64) {
    for (i, row) in matrix.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value += multiplier * score[i] * score[j];
        }
    }
}
