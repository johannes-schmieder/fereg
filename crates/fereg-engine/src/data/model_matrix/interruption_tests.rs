use super::*;
use crate::{execution::interruption_tests::every_checkpoint, threads::ThreadTeam};

#[test]
fn reported_recipe_binding_cancels_rows_columns_and_long_interactions_without_regeneration() {
    let n = 2053;
    let continuous = vec![(0..n).map(|i| (i % 3) as f64).collect(), vec![1.; n]];
    let categorical = vec![(0..n).map(|i| (i % 5) as u32).collect()];
    let builder = ModelMatrixBuilder::new(&continuous, &categorical).unwrap();
    let recipes = vec![
        ColumnRecipe::Intercept,
        ColumnRecipe::Continuous { source: 0 },
        ColumnRecipe::NumericIndicators {
            sources: vec![1; 2053],
            levels: vec![1.; 2053],
        },
        ColumnRecipe::Categorical {
            source: 0,
            level: 1,
        },
        ColumnRecipe::CategoricalInteraction {
            sources: vec![0, 0],
            levels: vec![1, 1],
        },
        ColumnRecipe::CategoricalByContinuous {
            categorical_source: 0,
            level: 1,
            continuous_source: 1,
        },
        ColumnRecipe::ContinuousInteraction {
            sources: vec![0, 1],
        },
    ];
    // The long recipe uses only three rows so exhaustive cancellation stays bounded.
    let short_sources = vec![vec![1.; 3]];
    let short = ModelMatrixBuilder::with_nrows(3, &short_sources, &[]).unwrap();
    let long = [ColumnRecipe::ContinuousInteraction {
        sources: vec![0; 4099],
    }];
    let short_matrix = short.generate(&long).unwrap();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let count = team
            .install(|| every_checkpoint(|poll| short.matches_polled(&long, &short_matrix, poll)));
        assert!(count > 10);
        let ordinary: Vec<_> = recipes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 2)
            .map(|(_, v)| v.clone())
            .collect();
        let matrix = builder.generate(&ordinary).unwrap();
        let rows = team
            .install(|| every_checkpoint(|poll| builder.matches_polled(&ordinary, &matrix, poll)));
        let sources = vec![vec![1.; 3]; 2];
        let indicators = ModelMatrixBuilder::new(&sources, &[]).unwrap();
        let numeric = indicators.generate(&recipes[2..3]).unwrap();
        let arity = team.install(|| {
            every_checkpoint(|poll| indicators.matches_polled(&recipes[2..3], &numeric, poll))
        });
        assert_eq!(
            *reference.get_or_insert((count, rows, arity)),
            (count, rows, arity)
        );
        let mut changed = matrix.clone();
        changed[(1025, 0)] = 2.;
        assert!(!builder.matches_polled(&ordinary, &changed, None).unwrap());
    }
    let observe = || true;
    let invalid = [ColumnRecipe::NumericIndicators {
        sources: vec![usize::MAX],
        levels: vec![1.],
    }];
    let zero = DenseMatrix::zeros(n, 1).unwrap();
    assert_eq!(
        builder.matches_polled(&invalid, &zero, None).unwrap_err(),
        builder
            .matches_polled(&invalid, &zero, Some(&observe))
            .unwrap_err()
    );
}
