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
