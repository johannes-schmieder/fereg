//! Independent per-level unit tables; no production grouping/weight/support helpers.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    ppml::{bias, irls},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    ErrorCode,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph},
    explicit_ols::DummyEffect,
};
use std::collections::BTreeMap;
#[path = "support/ppml_bias_calculation.rs"]
mod calculation;
#[path = "support/ppml_bias_information.rs"]
mod information;
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
        let source = sample.regressors.len();
        sample.regressors.push(g.regressors[0].clone());
        let columns = information::columns(source);
        let model = calculation::model(&sample, source);
        let raw = columns.generate(&sample).unwrap();
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            let problem = irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &columns.roles,
            };
            let options = Options {
                solver,
                ..Options::for_estimator(Estimator::Ppml)
            };
            let fit = irls::fit_final_sample(&problem, options, &one)
                .unwrap_or_else(|e| panic!("root seed={seed}/{solver:?}: {e}"));
            let actual = compare(&fit, &units, seed);
            let actions = information::compare(&fit, &columns, &g.effects, &one, seed);
            if let Some(actual) = actual {
                let correction = calculation::compare(&fit, &model, &units, &g.effects, &one, seed);
                accepted += 1;
                if seed % 41 == 0 {
                    let repeated = irls::fit_final_sample(&problem, options, &eight).unwrap();
                    assert_eq!(fit, repeated);
                    assert_eq!(
                        actions,
                        information::compare(&repeated, &columns, &g.effects, &eight, seed)
                    );
                    assert_eq!(actual, compare(&repeated, &units, seed).unwrap());
                    assert_eq!(
                        correction,
                        calculation::compare(&repeated, &model, &units, &g.effects, &eight, seed)
                    );
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
#[test]
fn both_exact_increments_retain_the_frozen_explicit_bias_fixture() {
    // Frozen prepared_estimator::bias_exact_full_system_matches_independent_explicit_design,
    // whose numbers were formed by scripts/ppml_bias_oracle.py independently of Rust.
    let effects = vec![
        DummyEffect::new((0..12).map(|i| (i / 2) as u32).collect(), 6, true, vec![]).unwrap(),
        DummyEffect::new((0..12).map(|i| (i % 2) as u32).collect(), 2, true, vec![]).unwrap(),
    ];
    let mut sample = inputs::prepare(&effects, &[1., 2., 2., 4., 3., 5., 4., 7., 5., 8., 6., 9.]);
    sample.regressors.push(vec![
        -1.5, -0.5, -1., 0.5, -0.5, 1., 0., 1.5, 0.5, 2., 1., 2.5,
    ]);
    sample.weights = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: vec![1., 2., 1.5, 0.5, 3., 2., 4., 1., 2.5, 3.5, 1., 2.],
    };
    sample.adjustment = PreparedAdjustment::Offset(vec![
        0., 0.05, 0.1, 0., -0.05, 0.1, 0., 0.05, -0.1, 0., 0.1, 0.05,
    ]);
    let model = calculation::model(&sample, 2);
    let columns = CompiledColumns::new(&model, Estimator::Ppml).unwrap();
    let raw = columns.generate(&sample).unwrap();
    let units: Vec<_> = (1..=12).map(f64::from).collect();
    let expected = [
        [
            -0.6300485701861334,
            0.30201008524001827,
            0.8998899114400561,
            1.4089593261745437,
            1.875388786600004,
            2.1284955000363808,
            0.,
            1.0631795606840344,
            -0.42003238012408883,
        ],
        [
            -0.013673707941541657,
            -0.03641436939328413,
            -0.0033508271882589646,
            -0.004078391875716949,
            0.004277300708386278,
            0.014184178816249116,
            0.,
            0.009115805294373391,
            -0.009115805294346302,
        ],
        [
            -0.0001930213526394886,
            0.006785593330410039,
            0.00038634085112329575,
            0.00034917161236207284,
            0.000364088184233044,
            0.00026489290492318074,
            0.,
            0.00012868090176021418,
            -0.0001286809017592705,
        ],
    ];
    let team = ThreadTeam::new(1).unwrap();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let fit = irls::fit_final_sample(
            &irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &columns.roles,
            },
            Options {
                solver,
                ..Options::for_estimator(Estimator::Ppml)
            },
            &team,
        )
        .unwrap();
        let result = bias::compute(
            &fit,
            &model,
            Some(bias::Request {
                method: bias::Method::Exact,
                save_effects: true,
            }),
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        for (channel, wanted) in [
            bias::Channel::Raw,
            bias::Channel::Derivative,
            bias::Channel::Curvature,
        ]
        .into_iter()
        .zip(expected)
        {
            for (j, reference) in wanted.into_iter().enumerate() {
                let value = result.coefficients.value(j, channel).unwrap();
                assert!(
                    (value - reference).abs() < 2e-8,
                    "{solver:?}/{channel:?}/{j}: {value} != {reference}"
                );
            }
        }
        assert_eq!(result.diagnostics.solves, 14);
    }
}
#[test]
fn tiny_mean_duration_raw_recovery_and_actions_match_explicit_full_design_qr() {
    // Preserve tests/stata/bias/final_recovery_precision.do's numerical trigger.
    let n = 4096;
    let effects = vec![
        DummyEffect::new((0..n).map(|i| (i % 8) as u32).collect(), 8, true, vec![]).unwrap(),
        DummyEffect::new(
            (0..n).map(|i| ((i / 8) % 8) as u32).collect(),
            8,
            true,
            vec![],
        )
        .unwrap(),
    ];
    let y: Vec<_> = (0..n).map(|i| 2. + (i % 5) as f64).collect();
    let mut sample = inputs::prepare(&effects, &y);
    sample
        .regressors
        .push((0..n).map(|i| (i as f64 * 0.37).sin()).collect());
    let mut exposure: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 8.).collect();
    exposure[37] = 5e-7;
    sample.adjustment = PreparedAdjustment::Exposure(exposure);
    let columns = information::columns(2);
    let raw = columns.generate(&sample).unwrap();
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let options = Options {
            solver,
            tolerance: 1e-9,
            ..Options::for_estimator(Estimator::Ppml)
        };
        let problem = irls::Problem {
            sample: &sample,
            raw: &raw,
            roles: &columns.roles,
        };
        let fit = irls::fit_final_sample(&problem, options, &one).unwrap();
        assert!(fit.evaluation.mean[37] < 1e-5);
        let expected = information::compare(&fit, &columns, &effects, &one, 3000);
        let repeated = irls::fit_final_sample(&problem, options, &eight).unwrap();
        assert_eq!(fit, repeated);
        assert_eq!(
            expected,
            information::compare(&repeated, &columns, &effects, &eight, 3000)
        );
    }
}

#[test]
fn weak_information_bridge_recovers_known_anchored_coordinates() {
    // A tiny bridge identifies an O(1) gauge transfer which small backward
    // moments alone cannot certify. Binary coefficients give an exact witness.
    let n = 33;
    let effects = vec![
        DummyEffect::new(
            (0..n).map(|i| u32::from((16..32).contains(&i))).collect(),
            2,
            true,
            vec![],
        )
        .unwrap(),
        DummyEffect::new(
            (0..n).map(|i| u32::from(i >= 16)).collect(),
            2,
            true,
            vec![],
        )
        .unwrap(),
    ];
    let response: Vec<_> = (0..n)
        .map(|i| {
            if i < 16 {
                0.25
            } else if i < 32 {
                1.25
            } else {
                2.25
            }
        })
        .collect();
    for bridge in [1e-8, 1e-18] {
        let mut sample = inputs::prepare(&effects, &vec![1.; n]);
        sample.regressors.push(vec![0.; n]);
        let mut weights = vec![1.; n];
        weights[n - 1] = bridge;
        sample.weights = PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values: weights,
        };
        let columns = information::columns(2);
        let raw = columns.generate(&sample).unwrap();
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            let options = Options {
                solver,
                ..Options::for_estimator(Estimator::Ppml)
            };
            let problem = irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &columns.roles,
            };
            let mut reference = None;
            for workers in [1, 8] {
                let team = ThreadTeam::new(workers).unwrap();
                let fit = irls::fit_final_sample(&problem, options, &team).unwrap();
                let actual = bias::with_information(
                    &fit,
                    &columns,
                    &team,
                    &mut || Ok(()),
                    |info, interrupt| {
                        assert!(info.finite_slots().is_empty());
                        let mut prediction = vec![0.; n];
                        let mut coefficients = vec![0.; info.dimension()];
                        info.solve(
                            &response,
                            &mut prediction,
                            Some(&mut coefficients),
                            interrupt,
                        )?;
                        for (&a, b) in coefficients.iter().zip([0.25, -0.75, 0., 2.]) {
                            assert!(
                                (a - b).abs() <= 2e-11,
                                "bridge {bridge:e}/{solver:?}: {coefficients:?}"
                            );
                        }
                        for (&a, &b) in prediction.iter().zip(&response) {
                            assert!(
                                (a - b).abs() <= 2e-11,
                                "bridge {bridge:e}/{solver:?}: {a} != {b}"
                            );
                        }
                        Ok((coefficients, prediction, info.diagnostics()))
                    },
                )
                .unwrap_or_else(|e| panic!("bridge {bridge:e}/{solver:?}: {e}"));
                if let Some(prior) = &reference {
                    assert_eq!(prior, &actual);
                } else {
                    reference = Some(actual);
                }
            }
        }
    }
}
