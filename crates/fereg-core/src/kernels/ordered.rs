//! Ordered recurrence and projection bodies shared by portable and FMA hosts.
//! Callers retain shape validation, allocation, interruption and certification.

/// One fused accumulation chain, in slice order.
#[inline(always)]
pub fn ordered_dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .fold(0.0_f64, |sum, (&a, &b)| a.mul_add(b, sum))
}

/// Eight chains with the entire incomplete block added to chain zero.
#[inline(always)]
pub fn normalized_product(left: &[f64], right: &[f64]) -> f64 {
    debug_assert_eq!(left.len(), right.len());
    let mut sums = [0.0_f64; 8];
    let blocks = left.len() / sums.len();
    for block in 0..blocks {
        let start = block * sums.len();
        for lane in 0..sums.len() {
            sums[lane] = left[start + lane].mul_add(right[start + lane], sums[lane]);
        }
    }
    for row in blocks * sums.len()..left.len() {
        sums[0] = left[row].mul_add(right[row], sums[0]);
    }
    sums.iter().sum()
}

#[inline(always)]
pub fn scaled_add(source: &[f64], scale: f64, destination: &mut [f64]) {
    for (value, &source) in destination.iter_mut().zip(source) {
        *value = source.mul_add(scale, *value);
    }
}

/// Add a numeric column or the implicit unit column, keeping coefficient first.
#[inline(always)]
pub fn coefficient_add(coefficient: f64, source: Option<&[f64]>, destination: &mut [f64]) {
    if let Some(source) = source {
        for (target, &value) in destination.iter_mut().zip(source) {
            *target = coefficient.mul_add(value, *target);
        }
    } else {
        for target in destination.iter_mut() {
            *target = coefficient.mul_add(1.0, *target);
        }
    }
}

/// Scatter one group's centered numeric or unit column in membership order.
#[inline(always)]
pub fn grouped_cross(
    members: &[usize],
    weights: &[f64],
    source: Option<&[f64]>,
    scale: f64,
    center: f64,
    codes: &[u32],
    cross: &mut [f64],
) {
    for &row in members {
        let level = codes[row] as usize;
        let value = source.map_or(1.0, |source| source[row]) / scale - center;
        cross[level] = weights[row].mul_add(value, cross[level]);
    }
}

#[inline(always)]
pub fn affine_direction(scale: f64, source: &[f64], destination: &mut [f64]) {
    for (value, &source) in destination.iter_mut().zip(source) {
        *value = scale.mul_add(*value, source);
    }
}

#[inline(always)]
pub fn joint_step(
    step: f64,
    direction: &[f64],
    product: &[f64],
    solution: &mut [f64],
    residual: &mut [f64],
) {
    for i in 0..solution.len() {
        solution[i] = step.mul_add(direction[i], solution[i]);
        residual[i] = (-step).mul_add(product[i], residual[i]);
    }
}

#[inline(always)]
pub fn subtract_projection(values: &mut [f64], direction: &[f64]) {
    let coefficient = values
        .iter()
        .zip(direction)
        .fold(0.0_f64, |sum, (&value, &basis)| value.mul_add(basis, sum));
    for (value, &basis) in values.iter_mut().zip(direction) {
        *value = (-coefficient).mul_add(basis, *value);
    }
}

#[inline(always)]
pub fn weighted_square_sum(response: &[f64], weights: &[f64], reference: f64) -> f64 {
    response.iter().zip(weights).fold(0.0_f64, |sum, (&y, &w)| {
        let value = y * w.sqrt() / reference;
        value.mul_add(value, sum)
    })
}

/// Eight weighted chains, retaining the contiguous-column tail in chain zero.
#[inline(always)]
pub fn weighted_product(left: &[f64], roots: &[f64], right: &[f64]) -> f64 {
    let mut sums = [0.0_f64; 8];
    let blocks = left.len() / sums.len();
    for block in 0..blocks {
        for (lane, sum) in sums.iter_mut().enumerate() {
            let row = block * 8 + lane;
            *sum = (left[row] * roots[row]).mul_add(right[row], *sum);
        }
    }
    for row in blocks * 8..left.len() {
        sums[0] = (left[row] * roots[row]).mul_add(right[row], sums[0]);
    }
    sums.iter().sum()
}

#[inline(always)]
pub fn normalized_add(
    coefficient: f64,
    source: &[f64],
    roots: &[f64],
    scale: f64,
    destination: &mut [f64],
) {
    for ((value, &source), &root) in destination.iter_mut().zip(source).zip(roots) {
        *value = coefficient.mul_add(source * root / scale, *value);
    }
}

/// The original-column certificate uses one ordered chain, with per-row scaling.
#[inline(always)]
pub fn normalized_dot(source: &[f64], roots: &[f64], scale: f64, residual: &[f64]) -> f64 {
    let mut sum = 0.0_f64;
    for ((&value, &root), &residual) in source.iter().zip(roots).zip(residual) {
        sum = (value * root / scale).mul_add(residual, sum);
    }
    sum
}

/// Retained-row reconstruction, with the caller's optional square-root weighting.
/// Weighting keeps its multiplication before normalization and its final division.
#[inline(always)]
pub fn standardized_residual(
    response: f64,
    values: &[f64],
    positions: &[usize],
    norms: &[f64],
    coefficients: &[f64],
    root: Option<f64>,
) -> f64 {
    let mut residual = root.map_or(response, |root| root * response);
    for (column, &position) in positions.iter().enumerate() {
        let value = root.map_or(values[position], |root| root * values[position]) / norms[position];
        residual = (-coefficients[column]).mul_add(value, residual);
    }
    root.map_or(residual, |root| residual / root)
}

#[inline(always)]
pub fn basis_forward(rows: &[usize], basis: &[f64], coefficients: &[f64], output: &mut [f64]) {
    for (local, &row) in rows.iter().enumerate() {
        let mut value = 0.0_f64;
        for (channel, &coefficient) in coefficients.iter().enumerate() {
            value = basis[channel * rows.len() + local].mul_add(coefficient, value);
        }
        output[row] += value;
    }
}

/// Form group moments; an eliminated group has no output coordinate.
#[inline(always)]
pub fn basis_transpose(
    rows: &[usize],
    basis: &[f64],
    rank: usize,
    values: &[f64],
    mut output: Option<&mut [f64]>,
    certify: bool,
) -> f64 {
    let mut square = 0.0_f64;
    for channel in 0..rank {
        let q = &basis[channel * rows.len()..][..rows.len()];
        let mut value = 0.0_f64;
        for (&row, &q) in rows.iter().zip(q) {
            value = q.mul_add(values[row], value);
        }
        if let Some(output) = output.as_deref_mut() {
            output[channel] = value;
        }
        if certify {
            square = value.mul_add(value, square);
        }
    }
    square
}

#[inline(always)]
pub fn basis_project(rows: &[usize], basis: &[f64], values: &mut [f64], scratch: &mut [f64]) {
    for (channel, target) in scratch.iter_mut().enumerate() {
        let q = &basis[channel * rows.len()..][..rows.len()];
        let mut value = 0.0_f64;
        for (&row, &q) in rows.iter().zip(q) {
            value = q.mul_add(values[row], value);
        }
        *target = value;
    }
    for (local, &row) in rows.iter().enumerate() {
        let mut value = 0.0_f64;
        for (channel, &coefficient) in scratch.iter().enumerate() {
            value = basis[channel * rows.len() + local].mul_add(coefficient, value);
        }
        values[row] -= value;
    }
}

#[inline(always)]
pub fn basis_projection(
    observations: &[usize],
    bases: &[Vec<f64>],
    weights: &[f64],
    residuals: &[f64],
    stride: usize,
    rhs: usize,
    denominator: f64,
) -> f64 {
    let mut square = 0.0_f64;
    if denominator > 0.0 {
        for basis in bases {
            let mut coefficient = 0.0_f64;
            for (local, &observation) in observations.iter().enumerate() {
                let scaled = weights[observation].sqrt() * residuals[observation * stride + rhs]
                    / denominator;
                coefficient = basis[local].mul_add(scaled, coefficient);
            }
            square = coefficient.mul_add(coefficient, square);
        }
    }
    square.sqrt()
}

#[inline(always)]
pub fn weighted_upper_gram(weight: f64, row: &[f64], gram: &mut [f64]) {
    let width = row.len();
    for j in 0..width {
        let value = weight * row[j];
        for k in j..width {
            gram[j * width + k] = value.mul_add(row[k], gram[j * width + k]);
        }
    }
}

#[inline(always)]
pub fn column_products(left: &[f64], right: &[f64], stride: usize, sums: &mut [f64]) {
    for level in 0..left.len() / stride {
        for (rhs, sum) in sums.iter_mut().enumerate() {
            let index = level * stride + rhs;
            *sum = left[index].mul_add(right[index], *sum);
        }
    }
}

#[inline(always)]
fn denominator(weights: &[f64], scales: Option<&[f64]>, level: usize) -> f64 {
    weights[level] / scales.map_or(1.0, |values| values[level])
}

#[inline(always)]
pub fn pcg_initialize(
    weights: &[f64],
    scales: Option<&[f64]>,
    stride: usize,
    residual: &[f64],
    direction: &mut [f64],
    energy: &mut [f64],
) {
    for level in 0..weights.len() {
        let denominator = denominator(weights, scales, level);
        for (rhs, sum) in energy.iter_mut().enumerate() {
            let index = level * stride + rhs;
            let value = residual[index];
            direction[index] = value / denominator;
            *sum = value.mul_add(direction[index], *sum);
        }
    }
}

#[inline(always)]
pub fn pcg_step(
    stride: usize,
    steps: &[f64],
    direction: &[f64],
    product: &[f64],
    solution: &mut [f64],
    residual: &mut [f64],
) {
    for level in 0..solution.len() / stride {
        for (rhs, &step) in steps.iter().enumerate() {
            let index = level * stride + rhs;
            solution[index] = step.mul_add(direction[index], solution[index]);
            residual[index] = (-step).mul_add(product[index], residual[index]);
        }
    }
}

#[inline(always)]
pub fn pcg_energy(
    weights: &[f64],
    scales: Option<&[f64]>,
    stride: usize,
    residual: &[f64],
    energy: &mut [f64],
) {
    for level in 0..weights.len() {
        let denominator = denominator(weights, scales, level);
        for (rhs, sum) in energy.iter_mut().enumerate() {
            let index = level * stride + rhs;
            let preconditioned = residual[index] / denominator;
            *sum = residual[index].mul_add(preconditioned, *sum);
        }
    }
}

// The serial path retains its single traversal and per-RHS reduction order.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn pcg_step_energy(
    weights: &[f64],
    scales: Option<&[f64]>,
    stride: usize,
    steps: &[f64],
    direction: &[f64],
    product: &[f64],
    solution: &mut [f64],
    residual: &mut [f64],
    energy: &mut [f64],
) {
    for level in 0..weights.len() {
        let denominator = denominator(weights, scales, level);
        for (rhs, &step) in steps.iter().enumerate() {
            let index = level * stride + rhs;
            solution[index] = step.mul_add(direction[index], solution[index]);
            residual[index] = (-step).mul_add(product[index], residual[index]);
            let preconditioned = residual[index] / denominator;
            energy[rhs] = residual[index].mul_add(preconditioned, energy[rhs]);
        }
    }
}

#[inline(always)]
pub fn pcg_direction(
    weights: &[f64],
    scales: Option<&[f64]>,
    stride: usize,
    ratios: &[f64],
    active: &[f64],
    residual: &[f64],
    direction: &mut [f64],
) {
    for level in 0..weights.len() {
        let denominator = denominator(weights, scales, level);
        for rhs in 0..ratios.len() {
            if active[rhs] == 0.0 {
                continue;
            }
            let index = level * stride + rhs;
            let preconditioned = residual[index] / denominator;
            // Deliberately different from affine_direction: ratio*direction
            // rounds before the addition. Do not contract that multiplication.
            direction[index] = preconditioned.mul_add(1.0, ratios[rhs] * direction[index]);
        }
    }
}

/// A fused scalar update for small certificate folds whose validation remains
/// interleaved in the owner. Hot traversals use the bulk entries instead.
#[inline(always)]
pub fn multiply_add(left: f64, right: f64, sum: f64) -> f64 {
    left.mul_add(right, sum)
}

/// Ordered triangular products, including the initial RHS and operand sign.
#[inline(always)]
pub fn subtract_products(
    left: &[f64],
    left_stride: usize,
    right: &[f64],
    right_stride: usize,
    count: usize,
    mut value: f64,
) -> f64 {
    for index in 0..count {
        value = (-left[index * left_stride]).mul_add(right[index * right_stride], value);
    }
    value
}

/// Project a bounded RHS tile onto a flat, optionally padded local basis.
/// Each coefficient and square sum retains its observation/basis order.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn basis_projection_tile(
    members: &[usize],
    basis: &[f64],
    stride: usize,
    weights: &[f64],
    residuals: &[f64],
    width: usize,
    scales: &[f64],
    first: usize,
    projected: &mut [f64],
) {
    debug_assert!(projected.len() <= 16);
    projected.fill(0.0);
    if basis.is_empty() {
        return;
    }
    for direction in basis.chunks_exact(stride) {
        let mut coefficients = [0.0_f64; 16];
        for (local, &row) in members.iter().enumerate() {
            let root = weights[row].sqrt();
            for column in 0..projected.len() {
                let denominator = scales[first + column];
                if denominator > 0.0 {
                    let scaled = root * residuals[row * width + first + column] / denominator;
                    coefficients[column] = direction[local].mul_add(scaled, coefficients[column]);
                }
            }
        }
        for column in 0..projected.len() {
            projected[column] =
                coefficients[column].mul_add(coefficients[column], projected[column]);
        }
    }
}

/// Reconstruct one immutable projected row in retained identity order.
#[inline(always)]
pub fn selected_residual(
    mut response: f64,
    values: &[f64],
    selected: &[usize],
    coefficients: &[Option<f64>],
) -> f64 {
    for &column in selected {
        response = (-coefficients[column].expect("selected finite coefficient"))
            .mul_add(values[column], response);
    }
    response
}

/// Reconstruct a joint residual from a row-major finite representative.
/// Scales and coefficients use selected-coordinate order, not source columns.
#[inline(always)]
pub fn normalized_selected_residual(
    mut residual: f64,
    values: &[f64],
    selected: &[usize],
    scales: &[f64],
    coefficients: &[f64],
    root: f64,
) -> f64 {
    for (j, &column) in selected.iter().enumerate() {
        residual = (-coefficients[j]).mul_add(values[column] * root / scales[j], residual);
    }
    residual
}

/// Update fresh joint scores in source-row and selected-coordinate order.
#[inline(always)]
pub fn normalized_selected_scores(
    values: &[f64],
    selected: &[usize],
    scales: &[f64],
    root: f64,
    observation: f64,
    scores: &mut [f64],
) {
    for (j, &column) in selected.iter().enumerate() {
        scores[j] = (values[column] * root / scales[j]).mul_add(observation, scores[j]);
    }
}

/// Apply one incidence row, completing its center before ordered scatter.
#[inline(always)]
pub fn incidence_schur_row(
    coefficients: &[f64],
    columns: &[usize],
    direction: &[f64],
    product: &mut [f64],
) {
    let mut center = 0.;
    for (&coefficient, &column) in coefficients.iter().zip(columns) {
        center = coefficient.mul_add(direction[column], center);
    }
    for (&coefficient, &column) in coefficients.iter().zip(columns) {
        product[column] = (-coefficient).mul_add(center, product[column]);
    }
}

/// Advance reduced CG and accumulate its next energy in coordinate order.
#[inline(always)]
pub fn joint_step_energy(
    alpha: f64,
    direction: &[f64],
    product: &[f64],
    values: &mut [f64],
    residual: &mut [f64],
) -> f64 {
    let mut next = 0.;
    for i in 0..values.len() {
        values[i] = alpha.mul_add(direction[i], values[i]);
        residual[i] = (-alpha).mul_add(product[i], residual[i]);
        next = residual[i].mul_add(residual[i], next);
    }
    next
}
