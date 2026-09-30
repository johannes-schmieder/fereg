//! Fresh local-span witness, harvested from legacy fe/certificate.rs.
//! Twice-reorthogonalized MGS is independent of the solver's Householder factors.
use super::Design;
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
};
pub(super) fn level_basis(
    design: &Design<'_>,
    effect: usize,
    level: usize,
    poll: Poll<'_>,
) -> Result<Vec<Vec<f64>>> {
    execution::check(poll)?;
    let members = design
        .topology()
        .effect_partition(effect)?
        .membership()
        .members(level)?;
    let channels = design.layout().effects[effect].channels;
    let threshold = 8. * f64::EPSILON * members.len().max(channels).max(1) as f64;
    let mut columns = reserved(channels)?;
    for channel in 0..channels {
        execution::check(poll)?;
        let mut column = reserved(members.len())?;
        let mut scale = 0_f64;
        for (local, &row) in members.iter().enumerate() {
            execution::check_chunk(poll, local)?;
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
        for (local, value) in column.iter_mut().enumerate() {
            execution::check_chunk(poll, local)?;
            *value /= scale;
        }
        let norm = norm(&column, poll)?;
        for (local, value) in column.iter_mut().enumerate() {
            execution::check_chunk(poll, local)?;
            *value /= norm;
        }
        columns.push((channel, column));
    }
    let mut basis: Vec<Vec<f64>> = reserved(channels.min(members.len()))?;
    while !columns.is_empty() && basis.len() < members.len() {
        execution::check(poll)?;
        let mut pivot = 0;
        let mut largest = norm(&columns[0].1, poll)?;
        for (index, (canonical, column)) in columns.iter().enumerate().skip(1) {
            let size = norm(column, poll)?;
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
                subtract_projection(&mut candidate, direction, poll)?;
            }
        }
        let size = norm(&candidate, poll)?;
        if size <= threshold {
            continue;
        }
        for (local, value) in candidate.iter_mut().enumerate() {
            execution::check_chunk(poll, local)?;
            *value /= size;
        }
        for (_, column) in &mut columns {
            subtract_projection(column, &candidate, poll)?;
            subtract_projection(column, &candidate, poll)?;
        }
        basis.push(candidate);
    }
    Ok(basis)
}
fn norm(values: &[f64], poll: Poll<'_>) -> Result<f64> {
    execution::fold(values, 0_f64, poll, |norm, &value| norm.hypot(value))
}
fn subtract_projection(values: &mut [f64], direction: &[f64], poll: Poll<'_>) -> Result<()> {
    let projection = execution::sum(
        values
            .iter()
            .zip(direction)
            .map(|(&value, &basis)| value * basis),
        poll,
    )?;
    for (local, (value, &basis)) in values.iter_mut().zip(direction).enumerate() {
        execution::check_chunk(poll, local)?;
        *value -= projection * basis;
    }
    Ok(())
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
