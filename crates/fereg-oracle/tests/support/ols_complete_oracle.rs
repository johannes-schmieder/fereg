//! Independent recipe materialization, sample ledger and explicit dummy design.
use super::fixture::Case;
use fereg_core as old;
use fereg_oracle::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, ExplicitOlsResult, OracleWeights,
};
use std::collections::BTreeMap;

pub struct Independent {
    pub rows: Vec<usize>,
    pub drops: Vec<u8>,
    pub slots: Vec<usize>,
    pub result: ExplicitOlsResult,
    pub means: Vec<f64>,
    pub weight_sum: f64,
    pub constant: Option<f64>,
    pub subsets: Vec<usize>,
}
fn available(column: &old::LoadedNumericColumn, row: usize) -> bool {
    !column.missing[row] && column.values[row].is_finite()
}
fn value(case: &Case, source: old::SourceId, row: usize) -> f64 {
    case.loaded.numeric_sources[source.get() as usize - 1]
        .1
        .values[row]
}
fn weights(case: &Case) -> Option<&old::LoadedNumericColumn> {
    case.loaded
        .analytic_weight
        .as_ref()
        .or(case.loaded.frequency_weight.as_ref())
        .or(case.loaded.probability_weight.as_ref())
}
fn tuple(case: &Case, sources: &[old::SourceId], row: usize) -> Vec<u64> {
    sources
        .iter()
        .map(|&s| value(case, s, row).to_bits())
        .collect()
}
fn sample(case: &Case) -> (Vec<usize>, Vec<u8>) {
    let n = case.loaded.outcome.values.len();
    let ordinary = case
        .request
        .model
        .terms
        .iter()
        .flat_map(|t| &t.components)
        .map(|c| c.source)
        .collect::<Vec<_>>();
    let effects = case
        .request
        .absorbed_terms
        .iter()
        .flat_map(|t| t.categorical_sources.iter().chain(&t.slope_sources))
        .copied()
        .collect::<Vec<_>>();
    let clusters = case
        .clusters
        .iter()
        .flat_map(|t| &t.sources)
        .copied()
        .collect::<Vec<_>>();
    let missing = |sources: &[old::SourceId], row| {
        sources
            .iter()
            .any(|s| !available(&case.loaded.numeric_sources[s.get() as usize - 1].1, row))
    };
    let mut drops = (0..n)
        .map(|row| {
            if !case.loaded.candidate[row] {
                1
            } else if !available(&case.loaded.outcome, row) {
                2
            } else if missing(&ordinary, row) {
                3
            } else if weights(case).is_some_and(|w| !available(w, row) || w.values[row] == 0.) {
                4
            } else if missing(&effects, row) {
                5
            } else if missing(&clusters, row) {
                6
            } else {
                0
            }
        })
        .collect::<Vec<_>>();
    let mut rows = (0..n).filter(|&row| drops[row] == 0).collect::<Vec<_>>();
    if !case.request.keep_singletons {
        loop {
            let before = rows.len();
            for effect in &case.request.absorbed_terms {
                let mut counts = BTreeMap::<Vec<u64>, u64>::new();
                for &row in &rows {
                    *counts
                        .entry(tuple(case, &effect.categorical_sources, row))
                        .or_default() += case
                        .loaded
                        .frequency_weight
                        .as_ref()
                        .map_or(1, |w| w.values[row] as u64);
                }
                rows.retain(|&row| {
                    let keep = counts[&tuple(case, &effect.categorical_sources, row)] > 1;
                    if !keep {
                        drops[row] = 8;
                    }
                    keep
                });
            }
            if rows.len() == before {
                break;
            }
        }
    }
    (rows, drops)
}
fn codes(case: &Case, sources: &[old::SourceId], rows: &[usize]) -> (Vec<u32>, usize) {
    let mut labels = rows
        .iter()
        .map(|&row| (tuple(case, sources, row), 0))
        .collect::<BTreeMap<_, _>>();
    for (index, code) in labels.values_mut().enumerate() {
        *code = index as u32;
    }
    (
        rows.iter()
            .map(|&row| labels[&tuple(case, sources, row)])
            .collect(),
        labels.len(),
    )
}
pub fn independent(case: &Case) -> Independent {
    let (rows, drops) = sample(case);
    let mut slots = vec![];
    let mut regressors = vec![];
    for (slot, term) in case.request.model.terms.iter().enumerate() {
        // The fixture's exact duplicate has a known earlier representative.
        // Removing it here leaves independent QR in charge of all other ranks.
        if term.whole_column_omitted || term.canonical_token == "duplicate.x0" {
            continue;
        }
        slots.push(slot);
        regressors.push(
            rows.iter()
                .map(|&row| {
                    if term.components[0].kind == old::ComponentKind::Continuous {
                        value(case, term.components[0].source, row)
                    } else {
                        f64::from(u8::from(
                            term.components
                                .iter()
                                .all(|c| value(case, c.source, row) == c.level_value),
                        ))
                    }
                })
                .collect::<Vec<_>>(),
        );
    }
    let mut effects = case
        .request
        .absorbed_terms
        .iter()
        .map(|effect| {
            let (codes, levels) = codes(case, &effect.categorical_sources, &rows);
            DummyEffect::new(
                codes,
                levels,
                effect.intercept,
                effect
                    .slope_sources
                    .iter()
                    .map(|&s| rows.iter().map(|&row| value(case, s, row)).collect())
                    .collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    if effects.is_empty() {
        effects.push(DummyEffect::new(vec![0; rows.len()], 1, true, vec![]).unwrap());
    }
    let supplied = weights(case).map(|w| rows.iter().map(|&r| w.values[r]).collect::<Vec<_>>());
    let reference = match supplied {
        None => vec![1.; rows.len()],
        Some(raw) if case.loaded.frequency_weight.is_some() => raw,
        Some(raw) => {
            let scale = rows.len() as f64 / raw.iter().sum::<f64>();
            raw.iter().map(|w| w * scale).collect()
        }
    };
    let weight_sum = reference.iter().sum::<f64>();
    let means = regressors
        .iter()
        .map(|column| {
            column
                .iter()
                .zip(&reference)
                .map(|(x, w)| x * w)
                .sum::<f64>()
                / weight_sum
        })
        .collect::<Vec<_>>();
    let outcome = rows
        .iter()
        .map(|&r| case.loaded.outcome.values[r])
        .collect::<Vec<_>>();
    let result = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors,
        effects,
        weights: if case.loaded.frequency_weight.is_some() {
            OracleWeights::Frequency(reference.iter().map(|w| *w as u32).collect())
        } else {
            OracleWeights::Wls(reference.clone())
        },
        clusters: case
            .clusters
            .iter()
            .map(|cluster| {
                codes(case, &cluster.sources, &rows)
                    .0
                    .into_iter()
                    .map(u64::from)
                    .collect()
            })
            .collect(),
    })
    .unwrap();
    let constant = if case.request.include_intercept
        && (case.request.absorbed_terms.is_empty()
            || case.request.absorbed_terms.iter().any(|e| e.intercept))
    {
        Some(
            outcome
                .iter()
                .zip(&reference)
                .map(|(y, w)| y * w)
                .sum::<f64>()
                / weight_sum
                - means
                    .iter()
                    .zip(&result.coefficients)
                    .map(|(x, b)| x * b.unwrap_or(0.))
                    .sum::<f64>(),
        )
    } else {
        None
    };
    let subsets = (1..1usize << case.clusters.len())
        .map(|mask| {
            let sources = case
                .clusters
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .flat_map(|(_, c)| &c.sources)
                .copied()
                .collect::<Vec<_>>();
            rows.iter()
                .map(|&row| tuple(case, &sources, row))
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        })
        .collect();
    Independent {
        rows,
        drops,
        slots,
        result,
        means,
        weight_sum,
        constant,
        subsets,
    }
}
