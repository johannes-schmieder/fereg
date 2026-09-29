//! Scalar dense operations ported from the independent Python Poisson oracle.
pub(super) fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

pub(super) fn max_abs(values: &[f64]) -> f64 {
    values.iter().map(|v| v.abs()).fold(0.0, f64::max)
}

pub(super) fn select_independent_columns(design: &[Vec<f64>]) -> (Vec<usize>, Vec<usize>) {
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let mut kept = Vec::new();
    let mut omitted = Vec::new();
    for j in 0..design[0].len() {
        let mut residual = design.iter().map(|row| row[j]).collect::<Vec<_>>();
        let original_norm = dot(&residual, &residual).sqrt();
        for _ in 0..2 {
            for direction in &basis {
                let projection = dot(&residual, direction);
                for (value, axis) in residual.iter_mut().zip(direction) {
                    *value -= projection * axis;
                }
            }
        }
        let norm = dot(&residual, &residual).sqrt();
        if norm <= 1e-11 * original_norm.max(1.0) {
            omitted.push(j);
        } else {
            for value in &mut residual {
                *value /= norm;
            }
            basis.push(residual);
            kept.push(j);
        }
    }
    (kept, omitted)
}

pub(super) fn invert(matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, String> {
    let n = matrix.len();
    if n == 0
        || matrix
            .iter()
            .any(|row| row.len() != n || row.iter().any(|v| !v.is_finite()))
    {
        return Err("invalid oracle inverse dimensions/values".into());
    }
    let mut augmented = matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .copied()
                .chain((0..n).map(|j| f64::from(i == j)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|&a, &b| {
                augmented[a][column]
                    .abs()
                    .total_cmp(&augmented[b][column].abs())
            })
            .unwrap();
        augmented.swap(column, pivot);
        let value = augmented[column][column];
        if value == 0.0 || !value.is_finite() {
            return Err("singular oracle inverse".into());
        }
        for cell in &mut augmented[column] {
            *cell /= value;
        }
        let pivot_row = augmented[column].clone();
        for (i, row) in augmented.iter_mut().enumerate() {
            if i == column {
                continue;
            }
            let factor = row[column];
            for (cell, axis) in row.iter_mut().zip(&pivot_row) {
                *cell -= factor * axis;
            }
        }
    }
    let result = augmented
        .into_iter()
        .map(|row| row[n..].to_vec())
        .collect::<Vec<_>>();
    if result.iter().flatten().any(|v| !v.is_finite()) {
        return Err("nonfinite oracle inverse".into());
    }
    Ok(result)
}

pub(super) fn cross_product(x: &[Vec<f64>], diagonal: &[f64]) -> Vec<Vec<f64>> {
    let k = x[0].len();
    let mut result = vec![vec![0.0; k]; k];
    for (row, &weight) in x.iter().zip(diagonal) {
        for (i, out) in result.iter_mut().enumerate() {
            for (j, value) in out.iter_mut().enumerate() {
                *value += weight * row[i] * row[j];
            }
        }
    }
    result
}

pub(super) fn score(x: &[Vec<f64>], y: &[f64], mu: &[f64], q: &[f64]) -> Vec<f64> {
    (0..x[0].len())
        .map(|j| {
            x.iter()
                .zip(y)
                .zip(mu)
                .zip(q)
                .map(|(((row, y), mu), q)| q * row[j] * (y - mu))
                .sum()
        })
        .collect()
}

pub(super) fn multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    a.iter()
        .map(|row| {
            (0..b[0].len())
                .map(|j| row.iter().enumerate().map(|(k, v)| v * b[k][j]).sum())
                .collect()
        })
        .collect()
}
