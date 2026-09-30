#![allow(dead_code)]
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph},
    explicit_ols::DummyEffect,
};
pub struct Case {
    pub effects: Vec<DummyEffect>,
    pub columns: Vec<Vec<f64>>,
    pub outcome: Vec<f64>,
}
fn paired<T: Copy>(values: &[T]) -> Vec<T> {
    values.iter().flat_map(|&v| [v, v]).collect()
}
pub fn case(seed: usize, separated: bool) -> Case {
    let graphs = [
        Graph::DensePanel,
        Graph::AkmStrong,
        Graph::AkmWeak,
        Graph::Chain,
        Graph::Barbell,
        Graph::Disconnected,
    ];
    let n = 79 + seed % 31;
    let generated = generate(
        seed as u64,
        &DesignSpec {
            rows: n,
            levels: vec![6, 5, 3][..seed % 4].to_vec(),
            regressors: seed % 3,
            slopes: seed % 5 == 0,
            graph: graphs[(seed / 4) % graphs.len()],
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let mut effects = generated.effects;
    let mut columns = generated.regressors;
    let outcome = if separated {
        let y: Vec<_> = (0..n)
            .map(|i| {
                if i % 7 == seed % 7 {
                    0.
                } else {
                    1. + (i % 3) as f64 * 0.25
                }
            })
            .collect();
        // The recession direction -1 on every zero row is a known sum of the
        // new finite column and absorbed channels, including slopes. It proves
        // all zero rows can separate without relying on the detector's flags.
        let mut finite: Vec<_> = y.iter().map(|&y| if y == 0. { -1. } else { 0. }).collect();
        for (index, effect) in effects.iter().enumerate() {
            for (row, value) in finite.iter_mut().enumerate() {
                let alpha = (index + 1 + effect.codes()[row] as usize) as f64 * 0.0625;
                if effect.has_intercept() {
                    *value -= alpha;
                }
                for slope in effect.slopes() {
                    *value -= alpha * slope[row];
                }
            }
        }
        columns.push(finite);
        y
    } else {
        // Every zero row has an exactly identical positive row in the complete
        // explicit design. Equality on positives forces zero on every boundary
        // row, so no strictly negative recession direction can exist.
        effects = effects
            .into_iter()
            .map(|e| {
                DummyEffect::new(
                    paired(e.codes()),
                    e.n_levels(),
                    e.has_intercept(),
                    e.slopes().iter().map(|v| paired(v)).collect(),
                )
                .unwrap()
            })
            .collect();
        columns = columns.iter().map(|v| paired(v)).collect();
        (0..n)
            .flat_map(|i| [0., 1. + (i % 3) as f64 * 0.25])
            .collect()
    };
    if effects.is_empty() {
        columns.insert(0, vec![1.; outcome.len()]);
    }
    Case {
        effects,
        columns,
        outcome,
    }
}
