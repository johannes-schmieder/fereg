//! Independent per-level unit tables; no production grouping/weight/support helpers.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    ppml::{bias, irls},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::Role,
    ErrorCode,
};
use fereg_oracle::designs::{generate, DesignSpec, Graph};
use std::collections::BTreeMap;
#[path = "support/engine.rs"]
mod inputs;
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "support seed={seed}: {a} != {b}"
    );
}
fn expected(sample: &Prepared, mu: &[f64], units: &[f64], effect: usize) -> [f64; 11] {
    let n = sample.n_active();
    let q = match &sample.weights {
        PreparedWeights::None => vec![1.; n],
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => values.clone(),
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values,
        } => {
            let sum: f64 = values.iter().sum();
            values.iter().map(|v| v * n as f64 / sum).collect()
        }
        _ => unreachable!(),
    };
    let partition = sample
        .topology
        .as_ref()
        .unwrap()
        .effect_partition(effect)
        .unwrap();
    let mut levels = vec![BTreeMap::<u64, f64>::new(); partition.n_levels()];
    for (row, &original) in sample.retained_rows().iter().enumerate() {
        let bits = if units[original] == 0. {
            0
        } else {
            units[original].to_bits()
        };
        *levels[partition.codes()[row] as usize]
            .entry(bits)
            .or_default() += q[row] * mu[row];
    }
    let mut counts: Vec<_> = levels.iter().map(BTreeMap::len).collect();
    let mut info = vec![];
    let mut largest = 0_f64;
    for table in levels {
        let sum: f64 = table.values().sum();
        let shares: Vec<_> = table.values().map(|v| v / sum).collect();
        info.push(1. / shares.iter().map(|v| v * v).sum::<f64>());
        largest = largest.max(shares.iter().copied().fold(0., f64::max));
    }
    counts.sort_unstable();
    info.sort_by(f64::total_cmp);
    let l = counts.len();
    let rank = |fraction: f64| (fraction * l as f64).ceil() as usize - 1;
    let singles = counts.iter().filter(|&&v| v == 1).count();
    [
        l as f64,
        counts[0] as f64,
        counts[rank(0.25)] as f64,
        counts[rank(0.5)] as f64,
        counts[rank(0.75)] as f64,
        counts[l - 1] as f64,
        singles as f64,
        singles as f64 / l as f64,
        info[0],
        info[rank(0.5)],
        largest,
    ]
}
fn compare(fit: &irls::Fit<'_>, units: &[f64], seed: usize) -> Option<bias::Support> {
    let sample = fit.problem().sample;
    let wanted = [
        expected(sample, &fit.evaluation.mean, units, 0),
        expected(sample, &fit.evaluation.mean, units, 1),
    ];
    let actual = bias::screen(fit, NumericColumnView::new(units), &mut || Ok(()));
    if wanted.iter().any(|summary| summary[6] != 0.) {
        assert_eq!(actual.unwrap_err().code, ErrorCode::InvalidArgument);
        return None;
    }
    let actual = actual.unwrap_or_else(|e| panic!("support seed={seed}: {e}"));
    for (summary, wanted) in actual.effects.iter().zip(wanted) {
        for (a, b) in summary.values().into_iter().zip(wanted) {
            close(a, b, seed);
        }
    }
    let mut identities: Vec<_> = sample
        .retained_rows()
        .iter()
        .map(|&i| if units[i] == 0. { 0. } else { units[i] })
        .collect();
    identities.sort_by(f64::total_cmp);
    identities.dedup();
    assert_eq!(actual.identities(), identities);
    let mut counts = vec![0usize; identities.len()];
    for (row, &original) in sample.retained_rows().iter().enumerate() {
        let code = identities
            .iter()
            .position(|v| *v == units[original])
            .unwrap();
        assert_eq!(actual.codes()[row], code);
        counts[code] += 1;
    }
    assert_eq!(actual.minimum_unit_rows(), *counts.iter().min().unwrap());
    assert_eq!(actual.maximum_unit_rows(), *counts.iter().max().unwrap());
    Some(actual)
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    let graphs = [
        Graph::DensePanel,
        Graph::AkmStrong,
        Graph::AkmWeak,
        Graph::Chain,
        Graph::Barbell,
        Graph::Disconnected,
    ];
    let (mut accepted, mut refused) = (0, 0);
    for seed in start..start + count {
        let g = generate(
            seed as u64,
            &DesignSpec {
                rows: 79 + seed % 31,
                levels: vec![6, 5],
                regressors: 1,
                graph: graphs[seed % 6],
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let y: Vec<_> = (0..g.outcome.len())
            .map(|i| 0.5 + ((i * 17 + seed * 3) % 11) as f64 * 0.15)
            .collect();
        let mut sample = inputs::prepare(&g.effects, &y);
        let n = sample.n_active();
        let values: Vec<_> = (0..n).map(|i| (1 + (i + seed) % 5) as f64).collect();
        sample.weights = match seed % 3 {
            0 => PreparedWeights::None,
            v => PreparedWeights::Provided {
                kind: if v == 1 {
                    WeightKind::Frequency
                } else {
                    WeightKind::Probability
                },
                values,
            },
        };
        let offsets: Vec<_> = (0..n)
            .map(|i| ((i + seed) % 7) as f64 * 0.06 - 0.18)
            .collect();
        sample.adjustment = match (seed / 3) % 3 {
            0 => PreparedAdjustment::None,
            1 => PreparedAdjustment::Offset(offsets),
            _ => PreparedAdjustment::Exposure(offsets.iter().map(|v| v.exp()).collect()),
        };
        sample.n_original = 2 * n + 3;
        sample.n_candidate = 2 * n + 2;
        sample.original_row_by_active = (0..n).map(|i| 2 * i + 1).collect();
        sample.drop_reasons = vec![DropReason::MissingOutcome; sample.n_original];
        sample.drop_reasons[0] = DropReason::ExcludedByIfIn;
        let mut units = vec![f64::NAN; sample.n_original];
        for (i, &row) in sample.original_row_by_active.iter().enumerate() {
            sample.drop_reasons[row] = DropReason::Retained;
            units[row] = if seed % 5 == 0 {
                g.effects[0].codes()[i] as f64
            } else if seed % 11 == 0 {
                (i / 2) as f64
            } else {
                (i % 7) as f64 - 3.5
            };
        }
        let raw = DenseMatrix::from_columns(&[g.regressors[0].clone(), vec![1.; n]]).unwrap();
        let roles = [Role::Ordinary, Role::AbsorbedIntercept];
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            let problem = irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &roles,
            };
            let options = Options {
                solver,
                ..Options::for_estimator(Estimator::Ppml)
            };
            let fit = irls::fit_final_sample(&problem, options, &one)
                .unwrap_or_else(|e| panic!("root seed={seed}/{solver:?}: {e}"));
            let actual = compare(&fit, &units, seed);
            if let Some(actual) = actual {
                accepted += 1;
                if seed % 41 == 0 {
                    let repeated = irls::fit_final_sample(&problem, options, &eight).unwrap();
                    assert_eq!(fit, repeated);
                    assert_eq!(actual, compare(&repeated, &units, seed).unwrap());
                }
            } else {
                refused += 1;
            }
        }
    }
    assert!(accepted > 0 && refused > 0);
}
#[test]
fn bias_support_matches_independent_tables_across_weights_graphs_and_backends() {
    campaign(0, 200);
}
#[test]
#[ignore = "long accepted-information unit-support campaign"]
fn two_thousand_bias_support_designs() {
    campaign(200, 2000);
}
