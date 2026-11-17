use crate::error::{ErrorCode, FeregError, Result};

#[must_use]
pub fn dot(left: &[f64], right: &[f64]) -> f64 {
    debug_assert_eq!(left.len(), right.len());
    left.iter()
        .zip(right)
        .fold(0.0, |sum, (left_value, right_value)| {
            left_value.mul_add(*right_value, sum)
        })
}

/// Solve `A X = B` in place using Gaussian elimination with partial pivoting.
///
/// `matrix` is row-major `dimension × dimension`; `right_hand_sides` is
/// row-major `dimension × rhs_count`. The return value is the numerical rank.
pub fn solve_linear_system(
    matrix: &mut [f64],
    right_hand_sides: &mut [f64],
    dimension: usize,
    rhs_count: usize,
    relative_tolerance: f64,
) -> Result<usize> {
    if matrix.len() != dimension.saturating_mul(dimension)
        || right_hand_sides.len() != dimension.saturating_mul(rhs_count)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "linear-system buffers have inconsistent dimensions",
        ));
    }
    if !relative_tolerance.is_finite() || relative_tolerance < 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "linear-system tolerance must be finite and nonnegative",
        ));
    }
    if dimension == 0 {
        return Ok(0);
    }

    let scale = matrix
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if scale == 0.0 {
        right_hand_sides.fill(0.0);
        return Ok(0);
    }
    let threshold = scale * relative_tolerance.max(f64::EPSILON);
    let mut rank = 0;

    for column in 0..dimension {
        crate::interrupt::checkpoint()?;
        let mut pivot_row = column;
        let mut pivot_value = matrix[column * dimension + column].abs();
        for row in (column + 1)..dimension {
            let candidate = matrix[row * dimension + column].abs();
            if candidate > pivot_value {
                pivot_row = row;
                pivot_value = candidate;
            }
        }
        if pivot_value <= threshold {
            continue;
        }
        if pivot_row != column {
            for other_column in 0..dimension {
                matrix.swap(
                    column * dimension + other_column,
                    pivot_row * dimension + other_column,
                );
            }
            for rhs in 0..rhs_count {
                right_hand_sides.swap(column * rhs_count + rhs, pivot_row * rhs_count + rhs);
            }
        }

        let pivot = matrix[column * dimension + column];
        for row in (column + 1)..dimension {
            let factor = matrix[row * dimension + column] / pivot;
            matrix[row * dimension + column] = 0.0;
            for other_column in (column + 1)..dimension {
                let pivot_entry = matrix[column * dimension + other_column];
                let index = row * dimension + other_column;
                matrix[index] = (-factor).mul_add(pivot_entry, matrix[index]);
            }
            for rhs in 0..rhs_count {
                let pivot_entry = right_hand_sides[column * rhs_count + rhs];
                let index = row * rhs_count + rhs;
                right_hand_sides[index] = (-factor).mul_add(pivot_entry, right_hand_sides[index]);
            }
        }
        rank += 1;
    }

    for row in (0..dimension).rev() {
        crate::interrupt::checkpoint()?;
        let diagonal = matrix[row * dimension + row];
        if diagonal.abs() <= threshold {
            for rhs in 0..rhs_count {
                right_hand_sides[row * rhs_count + rhs] = 0.0;
            }
            continue;
        }
        for rhs in 0..rhs_count {
            let mut value = right_hand_sides[row * rhs_count + rhs];
            for column in (row + 1)..dimension {
                value = (-matrix[row * dimension + column])
                    .mul_add(right_hand_sides[column * rhs_count + rhs], value);
            }
            right_hand_sides[row * rhs_count + rhs] = value / diagonal;
        }
    }
    Ok(rank)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_multiple_rhs_solve_observes_cancellation_during_both_stages() {
        const CHILD: &str = "FEREG_DENSE_SOLVE_BREAK_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args(["linalg::solve::tests::dense_multiple_rhs_solve_observes_cancellation_during_both_stages", "--exact", "--test-threads=1"])
                .status().unwrap();
            assert!(status.success());
            return;
        }
        let dimension = 96;
        for stop in [3, dimension + 3] {
            let mut matrix = vec![1.; dimension * dimension];
            let mut rhs = vec![0.; dimension * dimension];
            for i in 0..dimension {
                matrix[i * dimension + i] = 2.;
                rhs[i * dimension + i] = 1.;
            }
            let before = matrix.clone();
            crate::interrupt::test_polls::cancel_at(Some(stop));
            let result = solve_linear_system(&mut matrix, &mut rhs, dimension, dimension, 1e-12);
            crate::interrupt::test_polls::cancel_at(None);
            assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
            assert_ne!(
                matrix, before,
                "cancellation occurs after numerical work starts"
            );
        }
    }

    #[test]
    fn solves_multiple_right_hand_sides() {
        let mut matrix = vec![4.0, 2.0, 1.0, 3.0];
        let mut rhs = vec![6.0, 1.0, 4.0, 2.0];
        let rank = solve_linear_system(&mut matrix, &mut rhs, 2, 2, 1e-12).expect("solve");
        assert_eq!(rank, 2);
        assert!((rhs[0] - 1.0).abs() < 1e-12);
        assert!((rhs[2] - 1.0).abs() < 1e-12);
    }
}
