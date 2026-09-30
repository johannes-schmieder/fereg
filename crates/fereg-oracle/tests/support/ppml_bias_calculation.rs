//! Independent dense J_g/S_g/K/V contractions and two-pass group uncertainty.
use super::information::Explicit;
use fereg_engine::{
    data::{NumericColumnView, Prepared, PreparedWeights, WeightKind},
    ppml::{bias, irls},
    spec::metadata::*,
    threads::ThreadTeam,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec,
};
use fereg_oracle::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};
pub fn model(sample: &Prepared, source: usize) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (0..sample.regressors.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: ["x", "duplicate", "omitted"]
                .into_iter()
                .enumerate()
                .map(|(i, name)| ExpandedTerm {
                    id: TermId::new(i as u32 + 1).unwrap(),
                    order: TermOrder::new(i as u32 + 1).unwrap(),
                    coefficient_slot: CoefficientSlot::new(i as u32 + 1).unwrap(),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: i == 2,
                    canonical_token: name.into(),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).unwrap(),
                        source: SourceId::new(source as u32 + 1).unwrap(),
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::Unavailable,
                        raw_operator: "c".into(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    }],
                })
                .collect(),
        },
        absorbed_terms: (0..2)
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i + 1).unwrap(),
                categorical_sources: vec![SourceId::new(i + 1).unwrap()],
                intercept: true,
                slope_sources: vec![],
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    }
}
fn mv(matrix: &[Vec<f64>], value: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| row.iter().zip(value).map(|(a, b)| a * b).sum())
        .collect()
}
fn sum_vectors(vectors: &[Vec<f64>]) -> Vec<f64> {
    (0..vectors[0].len())
        .map(|j| vectors.iter().map(|v| v[j]).sum())
        .collect()
}
struct Dense {
    explicit: Explicit,
    inverse: Vec<Vec<f64>>,
    scores: Vec<Vec<f64>>,
    information: Vec<Vec<Vec<f64>>>,
    identities: Vec<f64>,
    h: Vec<f64>,
}
impl Dense {
    fn new(fit: &irls::Fit<'_>, effects: &[DummyEffect], units: &[f64]) -> Self {
        let sample = fit.problem().sample;
        let n = sample.n_active();
        let explicit = Explicit::new(effects, fit);
        let p = explicit.columns.len();
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
        let h: Vec<_> = q
            .iter()
            .zip(&fit.evaluation.mean)
            .map(|(a, b)| a * b)
            .collect();
        let oracle = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: vec![0.; n],
            regressors: explicit.columns.clone(),
            effects: vec![],
            weights: OracleWeights::Wls(h.clone()),
            clusters: vec![],
        })
        .unwrap();
        assert_eq!(oracle.kept_regressors.len(), p);
        let inverse: Vec<_> = (0..p)
            .map(|i| (0..p).map(|j| oracle.bread[(i, j)]).collect())
            .collect();
        let mut identities: Vec<_> = sample
            .retained_rows()
            .iter()
            .map(|&i| if units[i] == 0. { 0. } else { units[i] })
            .collect();
        identities.sort_by(f64::total_cmp);
        identities.dedup();
        let codes: Vec<_> = sample
            .retained_rows()
            .iter()
            .map(|&i| identities.iter().position(|&v| v == units[i]).unwrap())
            .collect();
        let mut scores = vec![vec![0.; p]; identities.len()];
        let mut information = vec![vec![vec![0.; p]; p]; identities.len()];
        for row in 0..n {
            for a in 0..p {
                scores[codes[row]][a] += q[row]
                    * explicit.columns[a][row]
                    * (sample.outcome[row] - fit.evaluation.mean[row]);
                for b in 0..p {
                    information[codes[row]][a][b] +=
                        h[row] * explicit.columns[a][row] * explicit.columns[b][row];
                }
            }
        }
        Self {
            explicit,
            inverse,
            scores,
            information,
            identities,
            h,
        }
    }
    fn exact(&self) -> (Vec<f64>, Vec<f64>) {
        let directions: Vec<_> = self.scores.iter().map(|s| mv(&self.inverse, s)).collect();
        let d = mv(
            &self.inverse,
            &sum_vectors(
                &directions
                    .iter()
                    .zip(&self.information)
                    .map(|(a, j)| mv(j, a))
                    .collect::<Vec<_>>(),
            ),
        );
        // Materialize V independently; production stores neither K nor V.
        let p = self.inverse.len();
        let v: Vec<Vec<_>> = (0..p)
            .map(|a| {
                (0..p)
                    .map(|b| directions.iter().map(|x| x[a] * x[b]).sum())
                    .collect()
            })
            .collect();
        let c = self.curvature(&v);
        (d, c)
    }
    fn curvature(&self, v: &[Vec<f64>]) -> Vec<f64> {
        let p = self.inverse.len();
        let mut rhs = vec![0.; p];
        for row in 0..self.h.len() {
            let x: Vec<_> = self
                .explicit
                .columns
                .iter()
                .map(|column| column[row])
                .collect();
            let variance: f64 = mv(v, &x).iter().zip(&x).map(|(a, b)| a * b).sum();
            for j in 0..p {
                rhs[j] += 0.5 * self.h[row] * x[j] * variance;
            }
        }
        mv(&self.inverse, &rhs)
    }
    fn randomized(&self, probes: usize) -> Vec<Vec<f64>> {
        let groups = probes.min(8);
        let mut blocks = vec![];
        let mut masses = vec![];
        let mut first = 0;
        for group in 0..groups {
            let count = probes / groups + usize::from(group < probes % groups);
            let p = self.inverse.len();
            let mut d_rhs = vec![0.; p];
            let mut v = vec![vec![0.; p]; p];
            for probe in first..first + count {
                let signs: Vec<_> = self.identities.iter().map(|&id| sign(id, probe)).collect();
                let score: Vec<_> = (0..p)
                    .map(|j| {
                        self.scores
                            .iter()
                            .zip(&signs)
                            .map(|(s, sign)| s[j] * sign)
                            .sum()
                    })
                    .collect();
                let direction = mv(&self.inverse, &score);
                for a in 0..p {
                    for b in 0..p {
                        v[a][b] += direction[a] * direction[b] / count as f64;
                    }
                }
                let signed_information: Vec<Vec<_>> = (0..p)
                    .map(|a| {
                        (0..p)
                            .map(|b| {
                                self.information
                                    .iter()
                                    .zip(&signs)
                                    .map(|(j, sign)| j[a][b] * sign)
                                    .sum()
                            })
                            .collect()
                    })
                    .collect();
                let term = mv(&signed_information, &direction);
                for j in 0..p {
                    d_rhs[j] += term[j] / count as f64;
                }
            }
            blocks.push((mv(&self.inverse, &d_rhs), self.curvature(&v)));
            masses.push(count as f64);
            first += count;
        }
        let p = self.inverse.len();
        let mean = |curvature: bool| {
            (0..p)
                .map(|j| {
                    blocks
                        .iter()
                        .zip(&masses)
                        .map(|((d, c), mass)| mass * if curvature { c[j] } else { d[j] })
                        .sum::<f64>()
                        / probes as f64
                })
                .collect::<Vec<_>>()
        };
        let d = mean(false);
        let c = mean(true);
        let mut variance = vec![vec![0.; p]; 4];
        for ((gd, gc), mass) in blocks.iter().zip(masses) {
            for j in 0..p {
                variance[0][j] += mass * (gd[j] - d[j]).powi(2) / (probes * (groups - 1)) as f64;
                variance[1][j] += mass * (gc[j] - c[j]).powi(2) / (probes * (groups - 1)) as f64;
                variance[2][j] +=
                    mass * (gd[j] - d[j]) * (gc[j] - c[j]) / (probes * (groups - 1)) as f64;
                variance[3][j] +=
                    mass * (gd[j] + gc[j] - d[j] - c[j]).powi(2) / (probes * (groups - 1)) as f64;
            }
        }
        vec![
            d,
            c,
            variance.remove(0),
            variance.remove(0),
            variance.remove(0),
            variance.remove(0),
        ]
    }
}
fn sign(unit: f64, probe: usize) -> f64 {
    // Independent scripts/ppml_bias_oracle.py vectorized Philox, seed=739391,
    // stream=0, probes 0..64. No production RNG is used by this contraction oracle.
    let masks = [
        0x4507f5287d971d9d_u64,
        0xed7aaff6cc8a4736,
        0x3171597bfcb70a4d,
        0xd69f2edeb3e0e744,
        0x35f3f3fd6f69d975,
        0x146af2352ff3da9f,
        0x95adae5d2438ec11,
    ];
    let integers = [
        0xabfad0f48b236a5e_u64,
        0x636d9e9c0508b162,
        0xcf54d042fc9858bf,
        0x44a4500a0abdb50b,
        0x8008fc1e4c6e2a15,
        0x81ee699d339f819e,
        0xea9eda05ab257b17,
        0x6e7c90ff8fdd7429,
        0x1095ee9415783c7f,
        0xad867ff5be01926b,
        0x044a4751c8fc6516,
        0xbf6be9c0ab19fa7f,
        0xb80ab5e5faa6fc96,
        0xbf0dc3e05d5f26cd,
        0x0b2e3ebd91695e50,
        0x6af30d27a8e32c7c,
        0xa072d787ff92f5c5,
        0xe4693b707c1748b1,
        0x7974b28336e61110,
        0xca8d05364406700a,
        0x1e7e2da3d0ddaa3a,
        0xd536f3589b9cdbf2,
        0x1253d6ee07ed6fff,
        0x893f71eccdec1da0,
        0xa279f6c13af5587c,
        0xc1a97be4a0724f39,
        0x47d52c462469b965,
        0x99af071400f7deef,
        0x65b9fd1b0acaacd9,
        0x7ef27d4a8abb02c0,
        0xcec6e28a49a67ffd,
        0x2af614d19d6c6bf3,
        0xea80a0fea740296f,
        0xcbcdddad8e7d983f,
        0x6f213a78d2340cb2,
        0xc7b259c530dcdcc5,
        0x6384a7889e49d9c4,
        0x8d5c2a7df5bd7c13,
        0xeb5e7980302ed21f,
        0xe065694ab62ec7f2,
        0x2f7834d0f2b0694d,
        0x52d238b7db37cfac,
        0x2e8d5df4859094cc,
        0xb8b8db6bf5e06b75,
        0xf6aeab0da27373d9,
        0xbca2541e50ecab9b,
        0x5d400ae903c3c212,
        0xb52184eb1389f6a1,
        0x0b6ffdd290d88fb6,
        0x865da4c542d6ae1d,
        0x29223458572fd332,
        0xe9fc103444531b88,
        0x89d22a2255276b6a,
        0xafb4c54296747c46,
        0xa1513c1692dd7159,
        0x1a835b6a08ff49de,
        0xb9c192188655095b,
        0xb17842149839f690,
        0x6fbc8b42f413fafc,
        0x23a512e20ee03670,
        0xd1ff80f247dd06b1,
        0x88b999a7fc69b1f0,
        0x11c4a9de53b49a7d,
        0x68889b79402dde24,
    ];
    assert!(probe < 64);
    let mask = if unit.fract() == 0. && (0. ..64.).contains(&unit) {
        integers[unit as usize]
    } else {
        assert!((unit + 3.5).fract() == 0. && (-3.5..=2.5).contains(&unit));
        masks[(unit + 3.5) as usize]
    };
    if mask & (1 << probe) == 0 {
        -1.
    } else {
        1.
    }
}
pub fn compare(
    fit: &irls::Fit<'_>,
    model: &ModelSpec,
    units: &[f64],
    effects: &[DummyEffect],
    team: &ThreadTeam,
    seed: usize,
) -> Vec<bias::Correction> {
    let dense = Dense::new(fit, effects, units);
    let mut results = vec![];
    for method in [
        bias::Method::Exact,
        bias::Method::Randomized {
            probes: 17,
            seed: 739391,
            stream: 0,
        },
    ] {
        let request = bias::Request {
            method,
            save_effects: seed % 2 == 0,
        };
        let mut result = bias::compute(
            fit,
            model,
            Some(request),
            Some(NumericColumnView::new(units)),
            team,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        let expected = if method == bias::Method::Exact {
            let (d, c) = dense.exact();
            vec![
                d,
                c,
                vec![0.; dense.inverse.len()],
                vec![0.; dense.inverse.len()],
                vec![0.; dense.inverse.len()],
                vec![0.; dense.inverse.len()],
            ]
        } else {
            dense.randomized(17)
        };
        for (index, channel) in [
            bias::Channel::Derivative,
            bias::Channel::Curvature,
            bias::Channel::SeDerivative,
            bias::Channel::SeCurvature,
            bias::Channel::Covariance,
        ]
        .into_iter()
        .enumerate()
        {
            let wanted: Vec<_> = expected[index]
                .iter()
                .map(|&v| {
                    if index == 2 || index == 3 {
                        v.sqrt()
                    } else {
                        v
                    }
                })
                .collect();
            let actual: Vec<_> = (0..result.coefficients.dimension())
                .map(|j| result.coefficients.value(j, channel).unwrap())
                .collect();
            dense
                .explicit
                .compare_coefficients(fit, &actual, &wanted, seed);
        }
        let wanted: Vec<_> = expected[5].iter().map(|v| v.sqrt()).collect();
        let actual: Vec<_> = (0..result.coefficients.dimension())
            .map(|j| {
                result
                    .coefficients
                    .value(j, bias::Channel::SeTotal)
                    .unwrap()
            })
            .collect();
        dense
            .explicit
            .compare_coefficients(fit, &actual, &wanted, seed);
        assert_eq!(result.diagnostics.solves, result.probes + 2 * result.groups);
        assert_eq!(
            result.saved.columns.len(),
            if request.save_effects { 18 } else { 0 }
        );
        result.elapsed_seconds = 0.;
        for column in &mut result.saved.columns {
            for value in &mut column.values {
                if value.is_nan() {
                    *value = -999.;
                }
            }
        }
        results.push(result);
    }
    results
}
