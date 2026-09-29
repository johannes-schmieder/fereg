//! Fresh local-span witness, harvested from legacy fe/certificate.rs.
//! Twice-reorthogonalized MGS is independent of the solver's Householder factors.
use super::Design;
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    threads::compensated_sum,
};
pub(super) fn level_basis(
    design: &Design<'_>,
    effect: usize,
    level: usize,
) -> Result<Vec<Vec<f64>>> {
    let members = design
        .topology()
        .effect_partition(effect)?
        .membership()
        .members(level)?;
    let channels = design.layout().effects[effect].channels;
    let threshold = 8. * f64::EPSILON * members.len().max(channels).max(1) as f64;
    let mut columns = reserved(channels)?;
    for channel in 0..channels {
        let mut column = reserved(members.len())?;
        let mut scale = 0_f64;
        for &row in members {
            let value =
                design.weights().at(row).sqrt() * design.basis_value(effect, channel, row)?;
            if !value.is_finite() {
                return Err(numerical("weighted certificate basis is not finite"));
            }
            scale = scale.max(value.abs());
            column.push(value);
        }
        if scale == 0. {
            continue;
        }
        for value in &mut column {
            *value /= scale;
        }
        let norm = norm(&column);
        for value in &mut column {
            *value /= norm;
        }
        columns.push((channel, column));
    }
    let mut basis: Vec<Vec<f64>> = reserved(channels.min(members.len()))?;
    while !columns.is_empty() && basis.len() < members.len() {
        let mut pivot = 0;
        let mut largest = norm(&columns[0].1);
        for (index, (canonical, column)) in columns.iter().enumerate().skip(1) {
            let size = norm(column);
            let tied = (size - largest).abs() <= 64. * f64::EPSILON * size.max(largest);
            if size > largest && !tied || tied && *canonical < columns[pivot].0 {
                pivot = index;
                largest = size;
            }
        }
        if largest <= threshold {
            break;
        }
        let (_, mut candidate) = columns.swap_remove(pivot);
        for _ in 0..2 {
            for direction in &basis {
                subtract_projection(&mut candidate, direction);
            }
        }
        let size = norm(&candidate);
        if size <= threshold {
            continue;
        }
        for value in &mut candidate {
            *value /= size;
        }
        for (_, column) in &mut columns {
            subtract_projection(column, &candidate);
            subtract_projection(column, &candidate);
        }
        basis.push(candidate);
    }
    Ok(basis)
}
fn norm(values: &[f64]) -> f64 {
    values.iter().fold(0_f64, |norm, &value| norm.hypot(value))
}
fn subtract_projection(values: &mut [f64], direction: &[f64]) {
    let projection = compensated_sum(
        values
            .iter()
            .zip(direction)
            .map(|(&value, &basis)| value * basis),
    );
    for (value, &basis) in values.iter_mut().zip(direction) {
        *value -= projection * basis;
    }
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
