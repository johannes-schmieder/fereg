//! Fresh original-coordinate row reconstruction and full-system normal equations.
use super::{invalid, numerical};
use crate::{
    data::{DenseMatrix, Prepared},
    error::Result,
    threads::compensated_sum,
};
pub(super) fn prediction(
    sample: &Prepared,
    raw: &DenseMatrix,
    kept: &[usize],
    first: usize,
    levels: usize,
    coefficients: &[f64],
    destination: &mut [f64],
) -> Result<()> {
    if coefficients.len() != levels + kept.len() || destination.len() != sample.n_active() {
        return Err(invalid("bias prediction dimensions differ"));
    }
    let topology = sample
        .topology
        .as_ref()
        .ok_or_else(|| invalid("bias topology is absent"))?;
    let left = topology.effect_partition(0)?.codes();
    let right = topology.effect_partition(1)?.codes();
    for (row, value) in destination.iter_mut().enumerate() {
        *value = compensated_sum(
            [
                coefficients[left[row] as usize],
                coefficients[first + right[row] as usize],
            ]
            .into_iter()
            .chain(
                kept.iter()
                    .enumerate()
                    .map(|(j, &column)| raw[(row, column)] * coefficients[levels + j]),
            ),
        );
        if !value.is_finite() {
            return Err(numerical("bias prediction is not representable"));
        }
    }
    Ok(())
}
pub(super) fn reconstruction(target: &[f64], actual: &[f64], tolerance: f64) -> Result<f64> {
    if target.len() != actual.len() {
        return Err(invalid("bias reconstruction dimensions differ"));
    }
    let mut maximum = 0_f64;
    for (row, (&target, &actual)) in target.iter().zip(actual).enumerate() {
        let error = (target - actual).abs() / target.abs().max(actual.abs()).max(1.);
        if !error.is_finite() || error > tolerance {
            return Err(numerical(format!("bias anchored FE reconstruction failed at row {row}: {error:.17e} > {tolerance:.17e}")));
        }
        maximum = maximum.max(error);
    }
    Ok(maximum)
}
pub(super) fn scores(
    sample: &Prepared,
    raw: &DenseMatrix,
    kept: &[usize],
    h: &[f64],
    response: &[f64],
    prediction: &[f64],
    tolerance: f64,
) -> Result<f64> {
    let topology = sample
        .topology
        .as_ref()
        .ok_or_else(|| invalid("bias topology is absent"))?;
    let mut maximum = 0_f64;
    for effect in 0..2 {
        let partition = topology.effect_partition(effect)?;
        for level in 0..partition.n_levels() {
            let rows = partition.membership().members(level)?;
            maximum = maximum.max(moment(
                rows.iter().copied(),
                |_| 1.,
                h,
                response,
                prediction,
            )?);
        }
    }
    for &column in kept {
        maximum = maximum.max(moment(
            0..sample.n_active(),
            |row| raw[(row, column)],
            h,
            response,
            prediction,
        )?);
    }
    if maximum > tolerance {
        return Err(numerical(format!(
            "bias auxiliary normal equations failed: {maximum:.17e} > {tolerance:.17e}"
        )));
    }
    Ok(maximum)
}
fn moment(
    rows: impl Iterator<Item = usize> + Clone,
    x: impl Fn(usize) -> f64,
    h: &[f64],
    response: &[f64],
    prediction: &[f64],
) -> Result<f64> {
    let sum = compensated_sum(
        rows.clone()
            .map(|row| h[row] * x(row) * (response[row] - prediction[row])),
    );
    let scale = compensated_sum(
        rows.map(|row| h[row] * x(row).abs() * (response[row].abs() + prediction[row].abs())),
    );
    if !sum.is_finite() || !scale.is_finite() {
        return Err(numerical("bias normal equation is not representable"));
    }
    Ok(sum.abs() / scale.max(1.))
}
