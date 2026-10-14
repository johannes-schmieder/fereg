//! Paired-cell weak graphs with analytic projection and regression targets.
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::DummyEffect;

pub const BETA: f64 = 0.75;
pub const COMPONENT_LEVELS: usize = 32;
pub const LEVELS: usize = 2 * COMPONENT_LEVELS;

fn character(repeat: usize, mask: usize) -> f64 {
    if (repeat & mask).count_ones() % 2 == 0 {
        1.
    } else {
        -1.
    }
}

fn worker_signal(level: usize) -> f64 {
    (((level * 17 + 3) % 37) as f64 - 18.) / 8.
}

fn firm_signal(level: usize) -> f64 {
    (((level * 13 + 5) % 43) as f64 - 21.) / 16.
}

pub struct Witness {
    pub design: Design,
    pub beta: Vec<f64>,
    pub residuals: Vec<f64>,
    pub constant: f64,
    pub saved_channels: [Vec<f64>; 2],
}

/// The signs cancel within each cell. Distinct Walsh characters additionally
/// make the finite regressor and residual exactly weighted-orthogonal. This
/// supplies beta and residual truth without invoking any iterative solver.
pub fn witness(weighted: bool) -> Witness {
    let mut codes = [Vec::new(), Vec::new()];
    let (mut x, mut y, mut weights, mut residuals) = (vec![], vec![], vec![], vec![]);
    for component in 0..2 {
        for worker in 0..COMPONENT_LEVELS {
            let degree = if worker == 0 { 8 } else { 2 };
            for neighbor in 0..degree {
                let first = component * COMPONENT_LEVELS + worker;
                let second = component * COMPONENT_LEVELS + (worker + neighbor) % COMPONENT_LEVELS;
                let weight = if weighted {
                    (1 + (first * 3 + second) % 7) as f64 / 8.
                } else {
                    1.
                };
                for repeat in 0..32 {
                    for sign in [-1., 1.] {
                        let within_x =
                            sign * character(repeat, 1) * (1 + (first + second) % 3) as f64 / 8.;
                        let residual =
                            sign * character(repeat, 2) * (1 + (2 * first + second) % 5) as f64
                                / 16.;
                        let regressor =
                            within_x + 2. * worker_signal(first) - 3. * firm_signal(second);
                        codes[0].push(first as u32);
                        codes[1].push(second as u32);
                        weights.push(weight);
                        x.push(regressor);
                        y.push(
                            BETA * regressor
                                + worker_signal(first)
                                + firm_signal(second)
                                + residual,
                        );
                        residuals.push(residual);
                    }
                }
            }
        }
    }
    let constant = weights
        .iter()
        .enumerate()
        .map(|(row, weight)| {
            weight * (worker_signal(codes[0][row] as usize) + firm_signal(codes[1][row] as usize))
        })
        .sum::<f64>()
        / weights.iter().sum::<f64>();
    let mut component_mass = [0.; 2];
    let mut firm_mean = [0.; 2];
    for (row, weight) in weights.iter().enumerate() {
        let component = codes[0][row] as usize / COMPONENT_LEVELS;
        component_mass[component] += weight;
        firm_mean[component] += weight * firm_signal(codes[1][row] as usize);
    }
    for component in 0..2 {
        firm_mean[component] /= component_mass[component];
    }
    let saved_channels = [
        codes[0]
            .iter()
            .map(|&worker| {
                worker_signal(worker as usize) + firm_mean[worker as usize / COMPONENT_LEVELS]
                    - constant
            })
            .collect(),
        codes[1]
            .iter()
            .map(|&firm| firm_signal(firm as usize) - firm_mean[firm as usize / COMPONENT_LEVELS])
            .collect(),
    ];
    let rows = y.len();
    Witness {
        beta: vec![BETA],
        design: Design {
            outcome: y,
            regressors: vec![x],
            effects: codes
                .into_iter()
                .map(|codes| DummyEffect::new(codes, LEVELS, true, vec![]).unwrap())
                .collect(),
            weights,
            weight_kind: if weighted {
                WeightKind::Analytic
            } else {
                WeightKind::Unit
            },
            missing: vec![false; rows],
            clusters: vec![],
        },
        residuals,
        constant,
        saved_channels,
    }
}

/// A genuine three-regressor Low block follows a zero-work outcome certificate.
/// The same graph, row count and weights retain the original setup tariff;
/// only the declared finite model changes. Distinct Walsh characters make its
/// within Gram diagonal and positive, with beta=(1,1,-1) and FE signal u+v.
pub fn three_rhs_low_witness(weighted: bool) -> Witness {
    let mut witness = witness(weighted);
    let rows = witness.design.outcome.len();
    let mut regressors = [
        Vec::with_capacity(rows),
        Vec::with_capacity(rows),
        Vec::with_capacity(rows),
    ];
    let mut outcome = Vec::with_capacity(rows);
    let mut residuals = Vec::with_capacity(rows);
    for row in 0..rows {
        let first = witness.design.effects[0].codes()[row] as usize;
        let second = witness.design.effects[1].codes()[row] as usize;
        let repeat = (row / 2) % 32;
        let sign = if row % 2 == 0 { -1. } else { 1. };
        let a = sign * character(repeat, 1) * (1 + (first + second) % 3) as f64 / 8.;
        let b = sign * character(repeat, 2) * (1 + (2 * first + second) % 5) as f64 / 16.;
        let c = sign * character(repeat, 3) * (1 + (3 * first + second) % 7) as f64 / 32.;
        let residual = sign * character(repeat, 4) * (1 + (first + 3 * second) % 5) as f64 / 16.;
        let (u, v) = (worker_signal(first), firm_signal(second));
        regressors[0].push(a + 2. * u);
        regressors[1].push(b + 2. * v);
        regressors[2].push(c + 3. * u + 3. * v);
        // Dyadic, opposite-sign row pairs keep raw y exactly cell-centered.
        outcome.push(a + b - c + residual);
        residuals.push(residual);
    }
    assert!(outcome.chunks_exact(2).all(|pair| pair[0] + pair[1] == 0.));
    witness.design.outcome = outcome;
    witness.design.regressors = regressors.into();
    witness.beta = vec![1., 1., -1.];
    witness.residuals = residuals;
    witness
}
