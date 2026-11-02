//! Frozen scalar arithmetic witness for structural-constant scheduling changes.
//! Scientific accuracy is checked separately by the dense OLS/FE oracles.

pub fn scalar_means(
    values: &[f64],
    columns: usize,
    weights: Option<&[f64]>,
) -> (f64, Vec<f64>, Vec<f64>, Vec<f64>) {
    fn add(value: f64, sum: &mut f64, correction: &mut f64) {
        let next = *sum + value;
        *correction += if sum.abs() >= value.abs() {
            (*sum - next) + value
        } else {
            (value - next) + *sum
        };
        *sum = next;
    }
    let scale = weights.map_or(1.0, |w| w.iter().copied().fold(0.0_f64, f64::max));
    let weight = |row: usize| weights.map_or(1.0, |w| w[row] / scale);
    let mut scales = vec![0.0_f64; columns];
    let mut means = vec![0.0; columns];
    let mut corrections = vec![0.0; columns];
    let mut total = 0.0;
    let mut correction = 0.0;
    for (row, values) in values.chunks_exact(columns).enumerate() {
        add(weight(row), &mut total, &mut correction);
        for column in 0..columns {
            scales[column] = scales[column].max(values[column].abs());
        }
    }
    total += correction;
    for (row, values) in values.chunks_exact(columns).enumerate() {
        for column in 0..columns {
            if scales[column] != 0.0 {
                add(
                    weight(row) * (values[column] / scales[column]),
                    &mut means[column],
                    &mut corrections[column],
                );
            }
        }
    }
    for column in 0..columns {
        means[column] = if scales[column] == 0.0 {
            0.0
        } else {
            scales[column] * ((means[column] + corrections[column]) / total)
        };
    }
    (total, scales, means, corrections)
}
