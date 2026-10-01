use super::*;
use crate::{execution::interruption_tests::every_checkpoint, threads::ThreadTeam};

#[test]
fn compensated_row_combinations_cancel_long_columns_without_changing_bits() {
    let width = 4099;
    let values: Vec<_> = (0..2)
        .flat_map(|row| {
            (0..width).map(move |column| {
                (row + 1) as f64
                    * if column + 1 == width {
                        -0.25
                    } else {
                        [1e16, 1., -1e16][column % 3]
                    }
            })
        })
        .collect();
    let matrix = DenseMatrix::from_row_major(2, width, values).unwrap();
    let coefficients = vec![1.; width];
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        for row in 0..2 {
            let expected = (row + 1) as f64 * 1365.75;
            assert_eq!(
                matrix.row_combination(row, &coefficients, 0.).to_bits(),
                expected.to_bits()
            );
            let count = team.install(|| {
                every_checkpoint(|poll| {
                    let value =
                        matrix.row_combination_polled(row, &coefficients, -expected, poll)?;
                    assert_eq!(value.to_bits(), 0_f64.to_bits());
                    Ok(value)
                })
            });
            assert_eq!(count, 4);
        }
    }
}

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

#[test]
fn finite_generation_cancels_every_checkpoint_in_rows_columns_and_interaction_components() {
    let values = vec![vec![1.; 2053], (0..2053).map(|i| (i % 7) as f64).collect()];
    let builder = ModelMatrixBuilder::new(&values, &[]).unwrap();
    let recipes = [
        ColumnRecipe::Intercept,
        ColumnRecipe::Continuous { source: 1 },
        ColumnRecipe::NumericIndicators {
            sources: vec![1],
            levels: vec![2.],
        },
    ];
    let short_values = vec![vec![0.5; 3]];
    let short = ModelMatrixBuilder::new(&short_values, &[]).unwrap();
    let long = [ColumnRecipe::ContinuousInteraction {
        sources: vec![0; 2053],
    }];
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let rows =
            team.install(|| every_checkpoint(|poll| builder.generate_polled(&recipes, poll)));
        let components =
            team.install(|| every_checkpoint(|poll| short.generate_polled(&long, poll)));
        let columns = team.install(|| {
            every_checkpoint(|poll| {
                short.generate_polled(&vec![ColumnRecipe::Intercept; 2053], poll)
            })
        });
        assert_eq!(
            *reference.get_or_insert((rows, components, columns)),
            (rows, components, columns)
        );
    }
    assert_eq!(short.generate(&long).unwrap().as_slice(), &[0.; 3]);
}
