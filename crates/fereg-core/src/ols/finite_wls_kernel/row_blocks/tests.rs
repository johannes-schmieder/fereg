use super::*;
use crate::ols::centered::CenteredSummary;
use crate::ols::prepared_fit::SystemFitReservation;

#[test]
fn complete_leaf_products_have_independent_orthogonal_witness() {
    for (rows, columns) in [(65_536, 1), (65_536, 3), (1024, 65)] {
        let sign = |row: usize, mask: usize| {
            if (row & mask).count_ones() % 2 == 0 {
                1.0
            } else {
                -1.0
            }
        };
        let mut raw = Vec::with_capacity(rows * (columns + 2));
        for row in 0..rows {
            raw.push(1.0 + 2.0 * sign(row, 1) - 0.5 * sign(row, columns) + 0.125 * sign(row, 128));
            for column in 0..columns {
                raw.push(sign(row, column + 1));
            }
            raw.push(1.0);
        }
        let raw = DenseMatrix::from_row_major(rows, columns + 2, raw).unwrap();
        let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
        let view = CenteredRows::new(&raw, &summary).unwrap();
        let selected: Vec<_> = (0..columns).collect();
        let scales = &summary.raw_norms()[1..columns + 1];
        let source = FixedBlockMomentSource::Centered {
            values: view,
            selected_columns: Some(&selected),
            column_scales: Some(scales),
        };
        let mut first = None;
        for workers in [1, 4, 16] {
            let result = crate::parallel::ThreadTeam::new(
                crate::options::ThreadConfig::from_requested(workers, false).unwrap(),
            )
            .install(|| {
                let checked = validate_moment_source(source).unwrap();
                let layout = checked.layout();
                let mut partials = vec![0.0; layout.partial_elements];
                let mut rhs = vec![0.0; columns];
                let mut gram = vec![0.0; columns * columns];
                fill_finite_wls_moments(checked, &mut partials, &mut rhs, &mut gram).unwrap();
                for left in 0..columns {
                    for right in 0..columns {
                        let expected = if left == right { 1.0 } else { 0.0 };
                        assert!((gram[left * columns + right] - expected).abs() < 2e-14);
                    }
                    let coefficient = if left == 0 { 2.0 } else { 0.0 }
                        - if left == columns - 1 { 0.5 } else { 0.0 };
                    assert!((rhs[left] - coefficient * (rows as f64).sqrt()).abs() < 2e-11);
                }
                (gram, rhs)
            })
            .unwrap();
            if let Some(first) = &first {
                assert_eq!(first, &result);
            } else {
                first = Some(result);
            }
        }
    }
}
