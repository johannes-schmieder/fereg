#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    fe::{Design, Weights},
    ppml::family::{Family, ScoreCertificate},
    spec::Estimator,
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};
use fereg_oracle::{
    designs::{generate, DesignSpec},
    explicit_ols::DummyEffect,
    explicit_ppml::{fit_explicit_ppml, PpmlProblem, PpmlWeights},
};

fn prepare(effects: &[DummyEffect], outcome: &[f64]) -> Prepared {
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    for (index, effect) in effects.iter().enumerate() {
        sources.push(effect.codes().iter().map(|&code| f64::from(code)).collect());
        let category = SourceId::new(sources.len() as u32).unwrap();
        let mut slopes = Vec::new();
        for slope in effect.slopes() {
            sources.push(slope.clone());
            slopes.push(SourceId::new(sources.len() as u32).unwrap());
        }
        terms.push(AbsorbedTerm {
            id: AbsorbTermId::new(index as u32 + 1).unwrap(),
            categorical_sources: vec![category],
            intercept: effect.has_intercept(),
            slope_sources: slopes,
            label: format!("g{index}"),
            saved_name: String::new(),
        });
    }
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..sources.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: terms,
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: outcome.len(),
            n_candidate: outcome.len(),
            original_row_by_active: (0..outcome.len()).collect(),
            outcome: outcome.to_vec(),
            regressors: sources,
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; outcome.len()],
        },
    )
    .unwrap()
}
fn explicit_columns(effects: &[DummyEffect], n: usize) -> Vec<Vec<f64>> {
    let mut columns = Vec::new();
    for effect in effects {
        for level in 0..effect.n_levels() {
            let bases = effect
                .has_intercept()
                .then(|| vec![1.; n])
                .into_iter()
                .chain(effect.slopes().iter().cloned());
            for basis in bases {
                columns.push(
                    (0..n)
                        .map(|i| {
                            if effect.codes()[i] as usize == level {
                                basis[i]
                            } else {
                                0.
                            }
                        })
                        .collect(),
                );
            }
        }
    }
    columns
}
fn compare_explicit(
    actual: ScoreCertificate,
    columns: &[Vec<f64>],
    finite: usize,
    y: &[f64],
    eta: &[f64],
    q: &[f64],
) {
    let mu: Vec<_> = eta.iter().map(|v| v.exp()).collect();
    let residual_scale: f64 = (0..y.len())
        .map(|i| q[i] * (y[i] - mu[i]).powi(2) / mu[i])
        .sum();
    let reference_scale: f64 = (0..y.len()).map(|i| q[i] * y[i].powi(2) / mu[i]).sum();
    let scale = residual_scale.max(reference_scale).sqrt();
    let mut relative = Vec::new();
    let mut absolute = Vec::new();
    for x in columns {
        let moment: f64 = (0..y.len()).map(|i| q[i] * (y[i] - mu[i]) * x[i]).sum();
        let information: f64 = (0..y.len()).map(|i| q[i] * mu[i] * x[i].powi(2)).sum();
        absolute.push(moment.abs());
        relative.push(if information == 0. {
            assert_eq!(moment, 0.);
            0.
        } else {
            moment.abs() / information.sqrt() / scale
        });
    }
    let max = |values: &[f64]| values.iter().copied().fold(0_f64, f64::max);
    let close = |a: f64, b: f64| {
        assert!(
            (a - b).abs() < 2e-12 * a.abs().max(b.abs()).max(1.),
            "{a:.17e} != {b:.17e}"
        )
    };
    close(actual.scale, scale);
    close(actual.finite.absolute_max, max(&absolute[..finite]));
    close(actual.fixed_effects.absolute_max, max(&absolute[finite..]));
    close(actual.finite.relative_max, max(&relative[..finite]));
    close(actual.fixed_effects.relative_max, max(&relative[finite..]));
    close(
        actual.relative_rms,
        (relative.iter().map(|v| v * v).sum::<f64>() / relative.len() as f64).sqrt(),
    );
    assert_eq!(actual.finite.directions, finite);
    assert_eq!(actual.fixed_effects.directions, columns.len() - finite);
    assert_eq!(actual.passed, max(&relative) <= 1e-8);
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let n = 61 + seed % 43;
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: n,
                levels: vec![3, 2, 4][..seed % 4].to_vec(),
                slopes: seed % 2 == 0,
                regressors: 2,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let mut effects = generated.effects;
        if seed % 4 == 2 {
            let e = &effects[0];
            effects[0] = DummyEffect::new(
                e.codes().to_vec(),
                e.n_levels(),
                false,
                vec![e.slopes()[0].clone(), vec![0.; n]],
            )
            .unwrap();
        }
        let eta: Vec<_> = (0..n)
            .map(|i| ((i * 7 + seed) % 13) as f64 * 0.07 - 0.4)
            .collect();
        let y: Vec<_> = (0..n)
            .map(|i| {
                if seed % 7 == 0 {
                    eta[i].exp()
                } else {
                    ((i * 11 + seed) % 17) as f64 / 2.
                }
            })
            .collect();
        let sample = prepare(&effects, &y);
        let fe = Design::new(&sample, Weights::Unit).unwrap();
        let mut columns = vec![vec![1.; n]];
        columns.extend(generated.regressors);
        columns.push((0..n).map(|i| f64::from(i % 3 == 1)).collect());
        columns.push(vec![0.; n]);
        let finite = columns.len();
        let raw = DenseMatrix::from_columns(&columns).unwrap();
        columns.extend(explicit_columns(&effects, n));
        let weights: Vec<_> = (0..n).map(|i| (1 + (i + seed) % 7) as f64).collect();
        let prepared = match seed % 3 {
            0 => PreparedWeights::None,
            1 => PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values: weights.clone(),
            },
            _ => PreparedWeights::Provided {
                kind: WeightKind::Probability,
                values: weights.clone(),
            },
        };
        let sum_weights: f64 = weights.iter().sum();
        let q: Vec<_> = weights
            .iter()
            .map(|w| match seed % 3 {
                0 => 1.,
                1 => *w,
                _ => w * n as f64 / sum_weights,
            })
            .collect();
        let reference = ReferenceWeights::new(&prepared, n, Estimator::Ppml).unwrap();
        let family = Family::new(&y, reference, None).unwrap();
        let actual = family
            .score_certificate(&eta, &raw, &fe, 1e-8, &one)
            .unwrap();
        compare_explicit(actual, &columns, finite, &y, &eta, &q);
        if seed % 20 == 0 {
            assert_eq!(
                actual,
                family
                    .score_certificate(&eta, &raw, &fe, 1e-8, &eight)
                    .unwrap()
            );
            check_newton(&columns, &raw, &fe, &y, &prepared, &effects, &eight);
        }
    }
}
fn check_newton(
    columns: &[Vec<f64>],
    raw: &DenseMatrix,
    fe: &Design<'_>,
    y: &[f64],
    weights: &PreparedWeights,
    effects: &[DummyEffect],
    team: &ThreadTeam,
) {
    let n = y.len();
    let offset: Vec<_> = (0..n).map(|i| (i % 5) as f64 * 0.1 - 0.2).collect();
    let problem = PpmlProblem {
        design: (0..n)
            .map(|i| columns.iter().map(|x| x[i]).collect())
            .collect(),
        outcome: y.to_vec(),
        offset: offset.clone(),
        weights: match weights {
            PreparedWeights::None => PpmlWeights::Unit,
            PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values,
            } => PpmlWeights::Frequency(values.iter().map(|w| *w as u32).collect()),
            PreparedWeights::Provided { values, .. } => PpmlWeights::Probability(values.clone()),
        },
        clusters: vec![],
    };
    let fit = fit_explicit_ppml(&problem).unwrap();
    let q = ReferenceWeights::new(weights, n, Estimator::Ppml).unwrap();
    let family = Family::new(y, q, Some(&offset)).unwrap();
    let certificate = family
        .score_certificate(&fit.eta, raw, fe, 1e-8, team)
        .unwrap();
    assert!(
        certificate.passed,
        "independent Newton state: {certificate:?}"
    );
    certify_newton_state(&family, raw, fe, &fit, effects, team);
    let eta: Vec<_> = fit.eta.iter().map(|v| v + 0.1).collect();
    assert!(
        !family
            .score_certificate(&eta, raw, fe, 1e-8, team)
            .unwrap()
            .passed
    );
}
fn certify_newton_state(
    family: &Family<'_>,
    raw: &DenseMatrix,
    fe: &Design<'_>,
    fit: &fereg_oracle::explicit_ppml::PpmlResult,
    effects: &[DummyEffect],
    team: &ThreadTeam,
) {
    use fereg_engine::ppml::step::Point;
    let mut absorbed = DenseMatrix::zeros(fe.layout().total, 1).unwrap();
    let mut base = raw.ncols();
    for (index, effect) in effects.iter().enumerate() {
        let layout = &fe.layout().effects[index];
        for level in 0..layout.levels {
            let representative = fe
                .topology()
                .effect_partition(index)
                .unwrap()
                .membership()
                .members(level)
                .unwrap()[0];
            let original_level = effect.codes()[representative] as usize;
            for channel in 0..layout.channels {
                absorbed[(layout.offset + channel * layout.levels + level, 0)] =
                    fit.coefficients[base + original_level * layout.channels + channel];
            }
        }
        base += effect.n_levels() * layout.channels;
    }
    let mut point = Point {
        finite: fit.coefficients[..raw.ncols()].to_vec(),
        absorbed,
        eta: fit.eta.clone(),
    };
    let evaluation = family.evaluate(&point.eta, team).unwrap();
    let certificate = family
        .certify_state(&point, &evaluation, raw, fe, 1e-12, team)
        .unwrap();
    assert!(certificate.predictor_relative_max < 2e-14);
    point.finite[0] += 0.01;
    assert!(family
        .certify_state(&point, &evaluation, raw, fe, 1e-12, team)
        .is_err());
}
#[test]
fn score_certificate_matches_independent_explicit_dummy_and_slope_moments() {
    campaign(0, 200);
}
#[test]
#[ignore = "long PPML score certificate campaign"]
fn two_thousand_original_frame_score_comparisons() {
    campaign(200, 2000);
}
