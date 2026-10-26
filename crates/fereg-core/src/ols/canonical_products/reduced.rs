//! Reduced Schur solves of canonical products: dense small components and
//! the implicit incidence operator's recurrence.
use super::*;

/// Implicit applications a dense iterative assembly must undercut.
pub(super) const ASSEMBLY_APPLICATIONS: u128 = 64;
/// Implicit components this small are factored from their applied columns.
pub(super) const DIRECT_COMPONENT: usize = 16;

/// CG terminates within `size` exact steps on a component of `size` free
/// coordinates; small components get slack for a recurrence target near
/// rounding. The certificate is unchanged.
pub(super) fn component_cap(maximum: usize, size: usize) -> usize {
    let slack = if size < 64 { size + 8 } else { 2 };
    maximum.min(size.saturating_add(slack))
}

/// A small implicit component as a dense Cholesky solve of its applied unit
/// columns, with the dense path's pivot guard.
pub(super) fn solve_small_component(
    values: &mut [f64],
    apply: impl Fn(&[f64], &mut [f64]),
) -> Result<usize> {
    let size = values.len();
    if size > DIRECT_COMPONENT {
        return Err(declined());
    }
    let mut matrix = [0.0_f64; DIRECT_COMPONENT * DIRECT_COMPONENT];
    let mut unit = [0.0_f64; DIRECT_COMPONENT];
    let mut column = [0.0_f64; DIRECT_COMPONENT];
    for j in 0..size {
        unit[j] = 1.;
        apply(&unit[..size], &mut column[..size]);
        unit[j] = 0.;
        for i in j..size {
            matrix[i * size + j] = column[i];
        }
    }
    let guard = 128. * f64::EPSILON * size as f64;
    for j in 0..size {
        let mut diagonal = matrix[j * size + j];
        for k in 0..j {
            diagonal -= matrix[j * size + k].powi(2);
        }
        if !diagonal.is_finite() || diagonal <= guard {
            return Err(declined());
        }
        matrix[j * size + j] = diagonal.sqrt();
        for i in j + 1..size {
            let mut value = matrix[i * size + j];
            for k in 0..j {
                value -= matrix[i * size + k] * matrix[j * size + k];
            }
            matrix[i * size + j] = value / matrix[j * size + j];
        }
    }
    for i in 0..size {
        for k in 0..i {
            values[i] -= matrix[i * size + k] * values[k];
        }
        values[i] /= matrix[i * size + i];
    }
    for i in (0..size).rev() {
        for k in i + 1..size {
            values[i] -= matrix[k * size + i] * values[k];
        }
        values[i] /= matrix[i * size + i];
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(declined());
    }
    Ok(1)
}

pub(super) fn solve_reduced_cg(
    matrix: &[f64],
    values: &mut [f64],
    scratch: &mut [f64],
    max_iterations: usize,
    tolerance: f64,
) -> Result<usize> {
    let n = values.len();
    if matrix.len() != n * n {
        return Err(declined());
    }
    solve_reduced_cg_with_product(
        values,
        scratch,
        max_iterations,
        tolerance,
        |direction, product| {
            for (row, out) in matrix.chunks(n).zip(product.iter_mut()) {
                *out = reduced_matrix_product(row, direction);
            }
        },
    )
}

// diag(m_b / root^2) - C'C. Group-pair weights are combined once;
// grounding and component-local coordinates are resolved before any iteration.
pub(super) fn apply_incidence_schur(
    incidence: &[f64],
    diagonal: &[f64],
    ids: &[usize],
    rows: &[IncidenceRow],
    direction: &[f64],
    product: &mut [f64],
) {
    for ((out, d), x) in product.iter_mut().zip(diagonal).zip(direction) {
        *out = d * x;
    }
    let hardware = crate::kernels::installed();
    for row in rows {
        let coefficients = &incidence[row.start..row.end];
        let columns = &ids[row.start..row.end];
        crate::kernels::dispatch!(
            hardware,
            incidence_schur_row(coefficients, columns, direction, product)
        );
    }
}

pub(super) fn solve_reduced_cg_with_product(
    values: &mut [f64],
    scratch: &mut [f64],
    max_iterations: usize,
    tolerance: f64,
    apply: impl Fn(&[f64], &mut [f64]),
) -> Result<usize> {
    let n = values.len();
    if scratch.len() < 3 * n {
        return Err(declined());
    }
    let scale = values.iter().map(|x| x.abs()).fold(0_f64, f64::max);
    if !scale.is_finite() {
        return Err(declined());
    }
    if scale == 0. {
        return Ok(0);
    }
    let (residual, rest) = scratch.split_at_mut(n);
    let (direction, product) = rest.split_at_mut(n);
    let product = &mut product[..n];
    for i in 0..n {
        residual[i] = values[i] / scale;
        direction[i] = residual[i];
    }
    values.fill(0.);
    let mut square: f64 = residual.iter().map(|x| x * x).sum();
    let target = tolerance * square.sqrt();
    let hardware = crate::kernels::installed();
    for iteration in 1..=max_iterations {
        apply(direction, product);
        let curvature: f64 = direction
            .iter()
            .zip(product.iter())
            .map(|(x, y)| x * y)
            .sum();
        if !curvature.is_finite() || curvature <= 0. || !square.is_finite() {
            return Err(declined());
        }
        let alpha = square / curvature;
        let next = crate::kernels::dispatch!(
            hardware,
            joint_step_energy(alpha, direction, product, values, residual)
        );
        if !next.is_finite() {
            return Err(declined());
        }
        if next.sqrt() <= target {
            for value in values {
                *value *= scale;
                if !value.is_finite() {
                    return Err(declined());
                }
            }
            return Ok(iteration);
        }
        let beta = next / square;
        crate::kernels::dispatch!(hardware, affine_direction(beta, residual, direction));
        square = next;
    }
    Err(declined())
}
