use super::{Design, WeightKind};
use crate::explicit_ols::{DummyEffect, ExplicitOlsProblem, OracleWeights};
use std::collections::BTreeMap;

pub(super) fn prepare(
    design: &Design,
    keep_singletons: bool,
) -> Result<(ExplicitOlsProblem, Vec<usize>), String> {
    let n = design.outcome.len();
    if design.missing.len() != n
        || design.weights.len() != n
        || design.regressors.iter().any(|x| x.len() != n)
        || design.effects.iter().any(|e| e.codes().len() != n)
        || design.clusters.iter().any(|c| c.len() != n)
    {
        return Err("random design has inconsistent lengths".into());
    }
    let mut rows = (0..n)
        .filter(|&i| {
            !design.missing[i]
                && design.outcome[i].is_finite()
                && design.regressors.iter().all(|x| x[i].is_finite())
                && design.weights[i].is_finite()
                && design.weights[i] > 0.0
        })
        .collect::<Vec<_>>();
    if design.weight_kind == WeightKind::Frequency
        && rows
            .iter()
            .any(|&i| design.weights[i].fract() != 0.0 || design.weights[i] > f64::from(u32::MAX))
    {
        return Err("invalid generated frequency".into());
    }
    if !keep_singletons {
        loop {
            let before = rows.len();
            for effect in &design.effects {
                let mut counts = BTreeMap::<u32, u64>::new();
                for &i in &rows {
                    *counts.entry(effect.codes()[i]).or_default() +=
                        if design.weight_kind == WeightKind::Frequency {
                            design.weights[i] as u64
                        } else {
                            1
                        };
                }
                rows.retain(|&i| counts[&effect.codes()[i]] > 1);
            }
            if rows.len() == before {
                break;
            }
        }
    }
    if rows.is_empty() {
        return Err("empty independently prepared sample".into());
    }
    let take = |values: &[f64]| rows.iter().map(|&i| values[i]).collect::<Vec<_>>();
    let mut effects = Vec::new();
    for effect in &design.effects {
        let mut labels = rows
            .iter()
            .map(|&i| (effect.codes()[i], 0u32))
            .collect::<BTreeMap<_, _>>();
        for (i, label) in labels.values_mut().enumerate() {
            *label = i as u32;
        }
        effects.push(
            DummyEffect::new(
                rows.iter().map(|&i| labels[&effect.codes()[i]]).collect(),
                labels.len(),
                effect.has_intercept(),
                effect.slopes().iter().map(|x| take(x)).collect(),
            )
            .map_err(|e| e.to_string())?,
        );
    }
    // The public OLS model has an intercept even when no absorb() term is supplied.
    if effects.is_empty() {
        effects.push(DummyEffect::new(vec![0; rows.len()], 1, true, vec![]).unwrap());
    }
    let weights = match design.weight_kind {
        WeightKind::Unit => OracleWeights::Unit,
        WeightKind::Frequency => {
            OracleWeights::Frequency(rows.iter().map(|&i| design.weights[i] as u32).collect())
        }
        WeightKind::Analytic | WeightKind::Probability => {
            let raw = take(&design.weights);
            let scale = raw.len() as f64 / raw.iter().sum::<f64>();
            OracleWeights::Wls(raw.iter().map(|w| w * scale).collect())
        }
    };
    Ok((
        ExplicitOlsProblem {
            outcome: take(&design.outcome),
            regressors: design.regressors.iter().map(|x| take(x)).collect(),
            effects,
            weights,
            clusters: design
                .clusters
                .iter()
                .map(|c| rows.iter().map(|&i| c[i]).collect())
                .collect(),
        },
        rows,
    ))
}
