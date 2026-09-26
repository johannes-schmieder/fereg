//! Full-frame three-FE canonical products, with one weighted Schur preparation.
use super::*;
use crate::fe::three_fe_schur::{AllocationBudget, Geometry, Plan as SchurPlan};
use crate::ols::linear_session::{compensated_add, update_scaled_square};

pub(super) struct Coefficients {
    order: [usize; 3],
    eliminated: usize,
    split: usize,
    stride: usize,
    values: Vec<f64>,
}

impl Coefficients {
    fn fitted(&self, design: &BorrowedInterceptFeDesign<'_>, row: usize, column: usize) -> f64 {
        let [a, b, c] = self.order.map(|e| &design.effects()[e]);
        let v = &self.values[column * self.stride..(column + 1) * self.stride];
        v[a.codes()[row] as usize]
            + v[self.eliminated + b.codes()[row] as usize]
            + v[self.eliminated + self.split + c.codes()[row] as usize]
    }

    pub(super) fn residual<P: PreparedOlsFitView + ?Sized>(
        &self,
        design: &BorrowedInterceptFeDesign<'_>,
        columns: &[Column<'_, P>],
        scales: &[f64],
        row: usize,
        column: usize,
    ) -> Result<f64> {
        let value = columns[column].value(row)? - scales[column] * self.fitted(design, row, column);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(declined())
        }
    }
}

// Shared by admission and construction: an impossible geometry must not
// inflate the public optional-workspace forecast before falling back.
pub(super) fn workspace_bytes(n: usize, width: usize, levels: [usize; 3], threads: usize) -> Option<u128> {
    let plan = SchurPlan::new(n, width, levels, threads)?;
    let groups = levels.iter().try_fold(0_usize, |sum, &v| sum.checked_add(v))?;
    let workers = threads.min(width).max(1);
    let square = width.checked_mul(width)?;
    let stride = groups.checked_mul(width)?.checked_add(square)?.checked_add(width)?;
    let coefficient_stride = plan.eliminated.checked_add(plan.reduced)?;
    coefficient_stride.checked_mul(width)?;
    workers.checked_mul(stride)?;
    plan.reduced.checked_mul(4)?.checked_mul(workers)?;
    // Cumulative charge retains construction scratch after its physical death.
    let doubles = 2 * n as u128 + groups as u128 + 7 * width as u128
        + coefficient_stride as u128 * width as u128
        + workers as u128 * (4 * plan.reduced as u128 + stride as u128)
        + square as u128;
    plan.geometry_bytes()?.checked_add(doubles.checked_mul(8)?)?
        .checked_add((groups as u128 + width as u128).checked_mul(size_of::<usize>() as u128)?)?
        .checked_add(width as u128 * 32)?
        .checked_add(4096)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_build<'a, P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &'a P,
    generated: &[usize],
    response: &'a [f64],
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    config: MapConfig,
    allowance: u128,
    obsolete_projection: &mut Vec<f64>,
    design: BorrowedInterceptFeDesign<'a>,
) -> Result<Option<Products<'a, P>>> {
    let n = response.len();
    let width = generated.len().checked_add(1).ok_or_else(allocation)?;
    let levels: [usize; 3] = std::array::from_fn(|e| design.effects()[e].n_levels());
    let threads = rayon::current_num_threads();
    let Some(plan) = SchurPlan::new(n, width, levels, threads) else {
        return Ok(None);
    };
    let Some(groups) = levels
        .iter()
        .try_fold(0_usize, |sum, &v| sum.checked_add(v))
    else {
        return Ok(None);
    };
    let workers = threads.min(width).max(1);
    let Some(stride) = groups
        .checked_mul(width)
        .and_then(|v| v.checked_add(width.checked_mul(width)?))
        .and_then(|v| v.checked_add(width))
    else {
        return Ok(None);
    };
    let Some(coefficient_stride) = plan.eliminated.checked_add(plan.reduced) else {
        return Ok(None);
    };
    if coefficient_stride.checked_mul(width).is_none()
        || workers.checked_mul(stride).is_none()
        || plan.reduced.checked_mul(4).and_then(|v| v.checked_mul(workers)).is_none()
    {
        return Ok(None);
    }
    let bytes = workspace_bytes(n, width, levels, threads);
    let Some(bytes) = bytes.filter(|&v| v <= allowance) else {
        return Ok(None);
    };
    if bytes > allowance.saturating_sub(obsolete_projection.capacity() as u128 * 8) {
        drop(std::mem::take(obsolete_projection));
    }
    match build(
        prepared,
        generated,
        response,
        weights,
        divisor,
        config,
        design,
        plan,
        groups,
        workers,
        stride,
        coefficient_stride,
        bytes,
    ) {
        Ok(products) => Ok(Some(products)),
        Err(error) if error.code == ErrorCode::NonConvergence => Ok(None),
        Err(error) => Err(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn build<'a, P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &'a P,
    generated: &[usize],
    response: &'a [f64],
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    config: MapConfig,
    design: BorrowedInterceptFeDesign<'a>,
    plan: SchurPlan,
    groups: usize,
    workers: usize,
    stride: usize,
    coefficient_stride: usize,
    bytes: u128,
) -> Result<Products<'a, P>> {
    let started = Instant::now();
    let n = response.len();
    let width = generated.len() + 1;
    let mut budget = AllocationBudget::new(bytes - 4096);
    let mut columns = budget.reserve(width, Column::Numeric(response))?;
    for (j, &generated) in generated.iter().enumerate() {
        let recipe = prepared
            .fit_model()
            .columns
            .get(generated)
            .ok_or_else(declined)?;
        columns[j + 1] = if let PreparedOlsColumnKind::Continuous {
            prepared_source_index,
            ..
        } = recipe.kind
        {
            let values = prepared
                .fit_sample()
                .regressors
                .get(prepared_source_index)
                .ok_or_else(declined)?;
            if values.len() != n {
                return Err(declined());
            }
            Column::Numeric(values)
        } else {
            Column::Generated(prepared, generated)
        };
    }
    let offsets = [
        0,
        design.effects()[0].n_levels(),
        design.effects()[0].n_levels() + design.effects()[1].n_levels(),
    ];
    let mut parent = budget.reserve(groups, 0_usize)?;
    for (j, value) in parent.iter_mut().enumerate() {
        *value = j;
    }
    for row in 0..n {
        let a = design.effects()[0].codes()[row] as usize;
        for (e, &offset) in offsets.iter().enumerate().skip(1) {
            let left = root(&mut parent, a);
            let right = root(
                &mut parent,
                offset + design.effects()[e].codes()[row] as usize,
            );
            parent[left.max(right)] = left.min(right);
        }
    }
    // Disconnected canonical checkpoints retain their existing component-scaled
    // engine. A global original-weight check alone cannot replace that policy.
    if (0..groups).filter(|&j| root(&mut parent, j) == j).count() != 1 {
        return Err(declined());
    }
    drop(parent);
    let mut w = budget.reserve(n, 0.)?;
    let mut masses = budget.reserve(groups, 0.)?;
    if let BorrowedMapWeights::Provided(values) = weights {
        if values.len() != n {
            return Err(declined());
        }
    }
    let mut total = 0.;
    let mut correction = 0.;
    for (row, value) in w.iter_mut().enumerate() {
        *value = match weights {
            BorrowedMapWeights::Unit => 1. / divisor,
            BorrowedMapWeights::Provided(v) => v[row] / divisor,
        };
        if !value.is_finite() || *value <= 0. {
            return Err(declined());
        }
        compensated_add(*value, &mut total, &mut correction)?;
        for (e, &offset) in offsets.iter().enumerate() {
            masses[offset + design.effects()[e].codes()[row] as usize] += *value;
        }
    }
    let total = total + correction;
    if !total.is_finite() || total <= 0. || masses.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(declined());
    }
    let Some(geometry) = Geometry::prepare(&design, &w, plan, &mut budget)? else {
        return Err(declined());
    };
    let setup_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let mut scales = budget.reserve(width, 0.)?;
    let mut means = budget.reserve(width, 0.)?;
    scales
        .par_iter_mut()
        .zip(means.par_iter_mut())
        .enumerate()
        .try_for_each(|(j, (norm, mean))| -> Result<()> {
            let (mut scale, mut square, mut sum, mut correction) = (0., 1., 0., 0.);
            for (row, &weight) in w.iter().enumerate() {
                let value = columns[j].value(row)?;
                update_scaled_square(value * weight.sqrt(), &mut scale, &mut square)?;
                compensated_add(weight * value, &mut sum, &mut correction)?;
            }
            *norm = scale * square.sqrt();
            *mean = (sum + correction) / total;
            if !norm.is_finite() || !mean.is_finite() {
                return Err(declined());
            }
            Ok(())
        })?;
    let mut coefficients = Coefficients {
        order: plan.order,
        eliminated: plan.eliminated,
        split: plan.split,
        stride: coefficient_stride,
        values: budget.reserve(width * coefficient_stride, 0.)?,
    };
    let mut scratch = budget.reserve(workers * 4 * plan.reduced, 0.)?;
    let mut steps = budget.reserve(width, 0_usize)?;
    let per_worker = width.div_ceil(workers);
    coefficients
        .values
        .par_chunks_mut(per_worker * coefficient_stride)
        .zip(scratch.par_chunks_mut(4 * plan.reduced))
        .zip(steps.par_chunks_mut(per_worker))
        .enumerate()
        .try_for_each(|(block, ((values, scratch), steps))| -> Result<()> {
            for (local, values) in values.chunks_mut(coefficient_stride).enumerate() {
                let j = block * per_worker + local;
                steps[local] = geometry.solve_column(
                    &design,
                    &w,
                    |row| columns[j].value(row),
                    scales[j],
                    config.max_iterations.min(plan.reduced.saturating_mul(2)),
                    config.tolerance,
                    values,
                    scratch,
                )?;
                if values.iter().any(|v| !v.is_finite()) {
                    return Err(declined());
                }
            }
            Ok(())
        })?;
    let iterations = steps.iter().copied().max().unwrap_or(0);
    drop(geometry);
    drop(scratch);
    drop(steps);
    let solve_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let mut partials = budget.reserve(workers * stride, 0.)?;
    let rows_per_worker = n.div_ceil(workers);
    partials.par_chunks_mut(stride).enumerate().try_for_each(
        |(worker, partial)| -> Result<()> {
            let (gram, rest) = partial.split_at_mut(width * width);
            let (scores, row_values) = rest.split_at_mut(groups * width);
            for row in worker * rows_per_worker..((worker + 1) * rows_per_worker).min(n) {
                for j in 0..width {
                    // The checked value is exactly the later materialized residual,
                    // independently reconstructed from originals, not a recurrence.
                    row_values[j] = coefficients.residual(&design, &columns, &scales, row, j)?
                        / nonzero(scales[j]);
                    for (e, &offset) in offsets.iter().enumerate() {
                        let group = offset + design.effects()[e].codes()[row] as usize;
                        scores[group * width + j] =
                            w[row].mul_add(row_values[j], scores[group * width + j]);
                    }
                }
                for j in 0..width {
                    let value = w[row] * row_values[j];
                    for k in j..width {
                        gram[j * width + k] = value.mul_add(row_values[k], gram[j * width + k]);
                    }
                }
            }
            Ok(())
        },
    )?;
    let mut gram = budget.reserve(width * width, 0.)?;
    for j in 0..width {
        for k in j..width {
            let value = merge(&partials, stride, j * width + k);
            if !value.is_finite() {
                return Err(declined());
            }
            gram[j * width + k] = value;
            gram[k * width + j] = value;
        }
    }
    let mut norms = budget.reserve(width, 0.)?;
    let (mut maximum, mut absolute, mut squares) = (0_f64, 0_f64, 0.);
    for j in 0..width {
        let norm = gram[j * width + j].sqrt();
        norms[j] = norm * nonzero(scales[j]);
        if !norms[j].is_finite() {
            return Err(declined());
        }
        for (g, &mass) in masses.iter().enumerate() {
            let score = merge(&partials, stride, width * width + g * width + j).abs();
            let relative = score / mass.sqrt() / norm.max(1.);
            if !relative.is_finite() {
                return Err(declined());
            }
            maximum = maximum.max(relative);
            squares = relative.mul_add(relative, squares);
            absolute = absolute.max(score * nonzero(scales[j]) * divisor);
        }
    }
    if !maximum.is_finite()
        || maximum > config.tolerance
        || !absolute.is_finite()
        || !squares.is_finite()
    {
        return Err(declined());
    }
    drop(partials);
    drop(masses);
    // Geometry and certificate state have no cross-call authority. Transfer the
    // dead weight bank into the response; only reconstruction coefficients live
    // until retained controls have been written once.
    let mut within_response = w;
    within_response
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(row, out)| -> Result<()> {
            *out = coefficients.residual(&design, &columns, &scales, row, 0)?;
            Ok(())
        })?;
    if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
        eprintln!("FEREG_SOLVER_TRACE|phase=canonical_three_products|rows={n}|rhs={width}|iterations={iterations}|workspace={bytes}");
    }
    let report = SolveReport {
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations,
        checkpoint_retries: 0,
        solver_workspace_peak: SolverWorkspacePeak::from_bytes(bytes)?,
        stop_reason: SolveStopReason::Certified,
        certificate: FeCertificate {
            absolute_infinity_norm: absolute,
            relative_infinity_norm: maximum,
            relative_two_norm: (squares / (groups * width) as f64).sqrt(),
            passed: true,
        },
        normal_equation_residual: maximum,
        setup_seconds,
        solve_seconds,
        certification_seconds: started.elapsed().as_secs_f64(),
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.,
        warnings: Vec::new(),
    };
    let moments = CanonicalMoments {
        width,
        gram,
        response_scale: nonzero(scales[0]),
    };
    Ok(Products {
        columns,
        design,
        left_effect: 0,
        left: Vec::new(),
        tiled: None,
        right: Vec::new(),
        three: Some(coefficients),
        scales,
        means,
        norms,
        response: within_response,
        moments,
        report,
        #[cfg(test)]
        minimum_workspace_bytes: bytes,
        #[cfg(test)]
        retained_centers: true,
    })
}
