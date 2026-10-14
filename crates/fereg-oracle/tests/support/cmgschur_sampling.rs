//! Independent sampled-preconditioner witnesses. Reference fits and distribution
//! algebra do not call production numerical kernels; native comparisons use the
//! existing prepared-OLS test adapter.

use super::{cell_contrast, check_tiled_inference, config, support};
use fereg_core::{prepare_model, PreparedOlsExecutionMode};
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect};

#[derive(Default)]
struct Rows {
    worker: Vec<u32>,
    firm: Vec<u32>,
    weights: Vec<f64>,
    workers: u32,
}

impl Rows {
    fn worker(&mut self, cells: impl IntoIterator<Item = (u32, f64)>) {
        for (firm, weight) in cells {
            for repeat in 0..2 + (firm + self.workers) % 2 {
                self.worker.push(self.workers);
                self.firm.push(firm);
                self.weights.push(weight * (1. + f64::from(repeat) / 2.));
            }
        }
        self.workers += 1;
    }
}

fn sampled_design() -> Design {
    let mut rows = Rows::default();
    let mut offset = 0;
    for degree in [30, 60] {
        rows.worker((0..degree).map(|firm| (offset + firm, 0.5 + f64::from(firm % 7) / 4.)));
        for low_degree in [4, 5, 6] {
            rows.worker((0..low_degree).map(|firm| (offset + firm, 1. + f64::from(firm) / 4.)));
        }
        // This weak bridge cannot be bypassed by the high-degree worker.
        rows.worker([(offset, 1. / 4096.), (offset + degree, 1. / 4096.)]);
        rows.worker([(offset + degree, 1.), (offset + degree + 1, 2.)]);
        for firm in 0..degree + 2 {
            rows.worker([(offset + firm, 0.5)]);
            rows.worker([(offset + firm, 1.5)]);
        }
        offset += degree + 2;
    }
    rows.worker([(offset, 0.75)]);
    rows.worker([(offset, 1.25)]);
    assert!(rows.workers > offset + 1);
    let n = rows.worker.len();
    let mut design = Design {
        outcome: (0..n)
            .map(|r| {
                0.13 * f64::from(rows.worker[r]) - 0.21 * f64::from(rows.firm[r])
                    + 0.08 * (0.71 * (r % 73) as f64).sin()
            })
            .collect(),
        regressors: vec![],
        effects: vec![
            DummyEffect::new(rows.worker, rows.workers as usize, true, vec![]).unwrap(),
            DummyEffect::new(rows.firm, offset as usize + 1, true, vec![]).unwrap(),
        ],
        weight_kind: WeightKind::Analytic,
        weights: rows.weights,
        missing: vec![false; n],
        clusters: vec![],
    };
    for column in 0..9 {
        let values: Vec<_> = match column {
            0 => cell_contrast(&design, 0),
            2 => vec![0.; n],
            3 => design.effects[0]
                .codes()
                .iter()
                .map(|&worker| f64::from(worker))
                .collect(),
            _ => (0..n)
                .map(|r| {
                    (((r * (column * 11 + 23) + column * column * 7) % 167) as f64 * 0.19).sin()
                        + f64::from(design.effects[1].codes()[r]) / (column + 3) as f64
                })
                .collect(),
        };
        for (y, value) in design.outcome.iter_mut().zip(&values) {
            *y += value * (column + 1) as f64 / 8.;
        }
        design.regressors.push(values);
    }
    design
}

#[test]
fn sampled_degree_thirty_and_sixty_match_independent_dense_ols() {
    let design = sampled_design();
    let (problem, rows) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    assert_eq!(oracle.dropped_regressors, [2, 3]);
    assert_eq!(oracle.finite_regressor_rank, 7);
    let (request, loaded) = support::inputs(&design, false);
    let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
    assert_eq!(
        prepared
            .sample()
            .original_row_by_active
            .iter()
            .collect::<Vec<_>>(),
        rows
    );
    assert_eq!(rows.len(), design.outcome.len());
    // Every count has both seeds and both execution/thread policies. Cutoffs
    // are balanced without multiplying all scientifically redundant choices.
    let cases = [
        (1, 0, 4),
        (1, 37, 6),
        (2, 0, 5),
        (2, 37, 4),
        (3, 0, 6),
        (3, 37, 5),
        (4, 0, 4),
        (4, (1_u64 << 53) - 1, 6),
    ];
    for (samples, seed, cutoff) in cases {
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for workers in [1, 4] {
                let mut config = config(cutoff, mode, workers);
                config.execution.poolsize = 4;
                config.absorb.cmg_schur_samples = samples;
                config.absorb.cmg_schur_seed = seed;
                check_tiled_inference(&prepared, &design, &oracle, config);
            }
        }
    }
}

#[derive(Clone)]
struct Tree {
    probability_numerator: i128,
    /// Conductance numerator; all edges have denominator sum(weights).
    laplacian: Vec<i128>,
}

/// Enumerate the Cartesian product of all later-parent choices, with a common
/// probability denominator equal to the product of remaining weight sums.
fn weighted_trees(weights: &[i128]) -> (Vec<Tree>, i128) {
    fn visit(weights: &[i128], vertex: usize, tree: Tree, trees: &mut Vec<Tree>) {
        let n = weights.len();
        if vertex + 1 == n {
            trees.push(tree);
            return;
        }
        let remaining: i128 = weights[vertex + 1..].iter().sum();
        for parent in vertex + 1..n {
            let mut next = tree.clone();
            next.probability_numerator *= weights[parent];
            let conductance = weights[vertex] * remaining;
            next.laplacian[vertex * n + vertex] += conductance;
            next.laplacian[parent * n + parent] += conductance;
            next.laplacian[vertex * n + parent] -= conductance;
            next.laplacian[parent * n + vertex] -= conductance;
            visit(weights, vertex + 1, next, trees);
        }
    }
    assert!(weights.iter().all(|&weight| weight > 0));
    let denominator = (1..weights.len())
        .map(|start| weights[start..].iter().sum::<i128>())
        .product();
    let mut trees = Vec::new();
    visit(
        weights,
        0,
        Tree {
            probability_numerator: 1,
            laplacian: vec![0; weights.len().pow(2)],
        },
        &mut trees,
    );
    assert_eq!(
        trees
            .iter()
            .map(|tree| tree.probability_numerator)
            .sum::<i128>(),
        denominator
    );
    (trees, denominator)
}

fn check_tree_kernel(tree: &Tree, n: usize) {
    let mut edges = 0;
    let mut reached = vec![false; n];
    let mut stack = vec![0];
    while let Some(vertex) = stack.pop() {
        if reached[vertex] {
            continue;
        }
        reached[vertex] = true;
        for (other, &value) in tree.laplacian[vertex * n..(vertex + 1) * n]
            .iter()
            .enumerate()
        {
            if other != vertex && value < 0 {
                stack.push(other);
            }
        }
    }
    for row in 0..n {
        assert_eq!(
            tree.laplacian[row * n..(row + 1) * n].iter().sum::<i128>(),
            0
        );
        assert!(tree.laplacian[row * n + row] > 0);
        for column in row + 1..n {
            let value = tree.laplacian[row * n + column];
            assert_eq!(value, tree.laplacian[column * n + row]);
            assert!(value <= 0);
            edges += usize::from(value < 0);
        }
    }
    assert_eq!(edges, n - 1);
    // Connected positive-conductance trees have precisely the constant kernel.
    assert!(reached.into_iter().all(|value| value));
}

fn average_numerators(
    trees: &[Tree],
    remaining: usize,
    probability: i128,
    sum: &mut [i128],
    expectation: &mut [i128],
    mass: &mut i128,
) {
    if remaining == 0 {
        *mass += probability;
        for (expected, &value) in expectation.iter_mut().zip(sum.iter()) {
            *expected += probability * value;
        }
        return;
    }
    for tree in trees {
        for (value, edge) in sum.iter_mut().zip(&tree.laplacian) {
            *value += edge;
        }
        average_numerators(
            trees,
            remaining - 1,
            probability * tree.probability_numerator,
            sum,
            expectation,
            mass,
        );
        for (value, edge) in sum.iter_mut().zip(&tree.laplacian) {
            *value -= edge;
        }
    }
}

#[test]
fn exact_tree_distribution_and_averages_equal_the_weighted_worker_schur_clique() {
    for weights in [vec![1_i128, 2, 4], vec![1, 2, 3, 5], vec![1, 1, 4, 4]] {
        let n = weights.len();
        let total: i128 = weights.iter().sum();
        let (trees, denominator) = weighted_trees(&weights);
        for tree in &trees {
            check_tree_kernel(tree, n);
        }
        // Independent elimination identity, with the worker's harmonic value
        // sum(w*x)/sum(w); multiply through by sum(w) to stay in integers.
        let values: Vec<_> = (0..n).map(|i| (i * i + 2 * i) as i128 - 3).collect();
        let weighted_sum: i128 = weights.iter().zip(&values).map(|(w, x)| w * x).sum();
        let schur_energy = total
            * weights
                .iter()
                .zip(&values)
                .map(|(w, x)| w * x * x)
                .sum::<i128>()
            - weighted_sum * weighted_sum;
        let mut clique_energy = 0;
        for i in 0..n {
            for j in i + 1..n {
                clique_energy += weights[i] * weights[j] * (values[i] - values[j]).pow(2);
            }
        }
        assert_eq!(schur_energy, clique_energy);
        assert!(schur_energy > 0);
        for samples in 1..=4 {
            let mut expectation = vec![0; n * n];
            let mut mass = 0;
            average_numerators(
                &trees,
                samples,
                1,
                &mut vec![0; n * n],
                &mut expectation,
                &mut mass,
            );
            let probability_denominator = denominator.pow(samples as u32);
            assert_eq!(mass, probability_denominator);
            for i in 0..n {
                for j in 0..n {
                    let clique = if i == j {
                        weights[i] * (total - weights[i])
                    } else {
                        -weights[i] * weights[j]
                    };
                    // Average conductance denominator is total*samples. Expected
                    // clique conductance denominator is total, which cancels.
                    assert_eq!(
                        expectation[i * n + j],
                        clique * samples as i128 * probability_denominator
                    );
                }
            }
        }
    }
}
