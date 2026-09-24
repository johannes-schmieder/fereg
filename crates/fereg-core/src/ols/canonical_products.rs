//! Current-call two-partition projection and finite products for canonical PPML.
//! All weighted state dies with this call; only certified rows and one-use
//! moments cross into the canonical rank owner.
use super::prepared_scratch::PreparedOlsScratch;
use super::PreparedOlsColumnKind;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedInterceptFeDesign, BorrowedMapWeights, FeCertificate, FeSolverKind, MapConfig,
    SolveReport, SolveStopReason,
};
use crate::memory::SolverWorkspacePeak;
use crate::prepared_fit_view::PreparedOlsFitView;
use rayon::prelude::*;
use std::mem::size_of;
use std::time::Instant;

fn declined() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "canonical Schur products declined current-weight geometry",
    )
}
fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "unable to reserve admitted canonical Schur products",
    )
}

struct Budget {
    remaining: u128,
}
impl Budget {
    fn reserve<T: Clone>(&mut self, n: usize, value: T) -> Result<Vec<T>> {
        let mut result = Vec::new();
        let bytes = (n as u128)
            .checked_mul(size_of::<T>() as u128)
            .ok_or_else(allocation)?;
        if bytes > self.remaining {
            return Err(allocation());
        }
        result.try_reserve_exact(n).map_err(|_| allocation())?;
        self.remaining = self
            .remaining
            .checked_sub(result.capacity() as u128 * size_of::<T>() as u128)
            .ok_or_else(allocation)?;
        result.resize(n, value);
        Ok(result)
    }
}

#[derive(Clone, Copy)]
struct Plan {
    n: usize,
    p: usize,
    q: usize,
    width: usize,
    workers: usize,
    stride: usize,
    bytes: u128,
}
impl Plan {
    fn new(
        design: &BorrowedInterceptFeDesign<'_>,
        width: usize,
        left: usize,
        threads: usize,
    ) -> Option<Self> {
        let n = design.nobs();
        let p = design.effects()[left].n_levels();
        let q = design.effects()[1 - left].n_levels();
        if n == 0 || p == 0 || q == 0 || width < 2 || threads == 0 {
            return None;
        }
        let workers = threads.min(p);
        let square = width.checked_mul(width)?;
        let cross = q.checked_mul(width)?;
        let factor = q.checked_mul(q)?;
        let stride = square
            .checked_add(cross)?
            .checked_add(width.checked_mul(5)?)?;
        // Preparation must fit within one complete finite Gram's arithmetic.
        // Distinct neighbours cannot exceed either membership or q; this
        // upper bound needs no allocated topology or benchmark-specific rule.
        let mut work = (q as u128).checked_mul(q as u128)?.checked_mul(q as u128)? / 3;
        for a in 0..p {
            let degree = design.effects()[left].level_index().members(a).len().min(q) as u128;
            work = work.checked_add(degree.checked_mul(degree.saturating_sub(1))? / 2)?;
        }
        let gram_work = (n as u128)
            .checked_mul(width as u128)?
            .checked_mul(width as u128 + 1)?
            / 2;
        if work > gram_work {
            return None;
        }
        let doubles = [
            n,
            n,
            p,
            q,
            q,
            q,
            factor,
            factor,
            p.checked_mul(width)?,
            cross,
            cross,
            cross,
            width,
            width,
            width,
            square,
            workers.checked_mul(stride)?,
        ]
        .into_iter()
        .try_fold(0_u128, |sum, x| sum.checked_add(x as u128))?;
        let indices = q.checked_mul(4)?;
        // Includes the small borrowed design, source bindings, vector headers,
        // result descriptors and scalar scratch, with no lifetime subtraction.
        let bytes = doubles
            .checked_mul(8)?
            .checked_add((indices as u128).checked_mul(size_of::<usize>() as u128)?)?
            .checked_add((width as u128).checked_mul(32)?)?
            .checked_add(2048)?;
        Some(Self {
            n,
            p,
            q,
            width,
            workers,
            stride,
            bytes,
        })
    }
}

enum Column<'a, P: ?Sized> {
    Numeric(&'a [f64]),
    Generated(&'a P, usize),
}
impl<P: PreparedOlsFitView + ?Sized> Column<'_, P> {
    #[inline]
    fn value(&self, row: usize) -> Result<f64> {
        match self {
            Self::Numeric(x) => Ok(x[row]),
            Self::Generated(p, j) => p.fit_column_value(*j, row),
        }
    }
}

/// Gram of [response, all stored controls], each divided by its original norm.
/// This value is consumed by the same call's canonical rank solve, never cached.
pub(super) struct CanonicalMoments {
    pub width: usize,
    pub gram: Vec<f64>,
    pub response_scale: f64,
}

pub(super) struct Products<'a, P: PreparedOlsFitView + ?Sized> {
    columns: Vec<Column<'a, P>>,
    design: BorrowedInterceptFeDesign<'a>,
    left_effect: usize,
    left: Vec<f64>,
    right: Vec<f64>,
    pub scales: Vec<f64>,
    pub means: Vec<f64>,
    pub norms: Vec<f64>,
    pub response: Vec<f64>,
    pub moments: CanonicalMoments,
    pub report: SolveReport,
}

impl<P: PreparedOlsFitView + Sync + ?Sized> Products<'_, P> {
    fn residual(&self, row: usize, j: usize) -> Result<f64> {
        let a = &self.design.effects()[self.left_effect];
        let b = &self.design.effects()[1 - self.left_effect];
        let scale = nonzero(self.scales[j]);
        let value = (self.columns[j].value(row)? / scale
            - self.left[j * a.n_levels() + a.codes()[row] as usize]
            - self.right[j * b.n_levels() + b.codes()[row] as usize])
            * scale;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(declined())
        }
    }

    pub fn write_controls(&self, storage: &mut PreparedOlsScratch) -> Result<()> {
        if storage.rows() != self.design.nobs() || storage.columns() + 1 != self.columns.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "canonical product storage is misaligned",
            ));
        }
        storage.rewrite_row_major_with(|row, column| self.residual(row, column + 1))
    }
}

fn nonzero(x: f64) -> f64 {
    if x == 0. {
        1.
    } else {
        x
    }
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

pub(super) fn try_build<'a, P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &'a P,
    generated: &[usize],
    response: &'a [f64],
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    config: MapConfig,
    allowance: u128,
) -> Result<Option<Products<'a, P>>> {
    if config.max_iterations == 0
        || config.check_every == 0
        || !config.tolerance.is_finite()
        || config.tolerance <= 0.
        || config.tolerance >= 1.
        || !divisor.is_finite()
        || divisor <= 0.
    {
        return Ok(None);
    }
    let Some(design) = prepared.fit_borrowed_intercept_map_design()? else {
        return Ok(None);
    };
    if design.effects().len() != 2 || design.nobs() != response.len() {
        return Ok(None);
    }
    let left = usize::from(design.effects()[1].n_levels() > design.effects()[0].n_levels());
    let Some(plan) = Plan::new(
        &design,
        generated.len() + 1,
        left,
        rayon::current_num_threads(),
    ) else {
        return Ok(None);
    };
    if plan.bytes > allowance {
        return Ok(None);
    }
    match build(
        prepared, generated, response, weights, divisor, config, design, left, plan,
    ) {
        Ok(result) => Ok(Some(result)),
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
    left_effect: usize,
    plan: Plan,
) -> Result<Products<'a, P>> {
    let started = Instant::now();
    let Plan {
        n,
        p,
        q,
        width,
        workers,
        stride,
        bytes,
    } = plan;
    let a = &design.effects()[left_effect];
    let b = &design.effects()[1 - left_effect];
    let mut budget = Budget {
        remaining: bytes - 2048 - (width as u128) * 32,
    };
    let mut columns = Vec::new();
    columns.try_reserve_exact(width).map_err(|_| allocation())?;
    if columns.capacity() as u128 * size_of::<Column<'_, P>>() as u128 > width as u128 * 32 {
        return Err(allocation());
    }
    columns.push(Column::Numeric(response));
    for &j in generated {
        let recipe = prepared.fit_model().columns.get(j).ok_or_else(declined)?;
        if let PreparedOlsColumnKind::Continuous {
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
            columns.push(Column::Numeric(values));
        } else {
            columns.push(Column::Generated(prepared, j));
        }
    }
    let mut w = budget.reserve(n, 0.)?;
    let mut mass_a = budget.reserve(p, 0.)?;
    let mut mass_b = budget.reserve(q, 0.)?;
    if let BorrowedMapWeights::Provided(values) = weights {
        if values.len() != n {
            return Err(declined());
        }
    }
    for i in 0..n {
        w[i] = match weights {
            BorrowedMapWeights::Unit => 1. / divisor,
            BorrowedMapWeights::Provided(v) => v[i] / divisor,
        };
        if !w[i].is_finite() || w[i] <= 0. {
            return Err(declined());
        }
        mass_a[a.codes()[i] as usize] += w[i];
        mass_b[b.codes()[i] as usize] += w[i];
    }
    if mass_a
        .iter()
        .chain(&mass_b)
        .any(|x| !x.is_finite() || *x <= 0.)
    {
        return Err(declined());
    }
    let mut matrix = budget.reserve(q * q, 0.)?;
    let mut parent = budget.reserve(q, 0_usize)?;
    for (i, x) in parent.iter_mut().enumerate() {
        *x = i;
    }
    let mut totals = budget.reserve(q, 0.)?;
    let mut touched = budget.reserve(q, 0_usize)?;
    for g in 0..p {
        let mut count = 0;
        for &i in a.level_index().members(g) {
            let h = b.codes()[i] as usize;
            if totals[h] == 0. {
                touched[count] = h;
                count += 1;
            }
            totals[h] += w[i];
        }
        touched[..count].sort_unstable();
        if count == 0 {
            return Err(declined());
        }
        for (position, &h) in touched[..count].iter().enumerate() {
            let rh = root(&mut parent, h);
            let rf = root(&mut parent, touched[0]);
            parent[rh] = rf;
            for &j in &touched[position + 1..count] {
                let value = (totals[h].max(totals[j]) / mass_a[g]) * totals[h].min(totals[j]);
                if !value.is_finite() || value <= 0. {
                    return Err(declined());
                }
                matrix[h * q + j] -= value;
                matrix[j * q + h] -= value;
                matrix[h * q + h] += value;
                matrix[j * q + j] += value;
            }
        }
        for &h in &touched[..count] {
            totals[h] = 0.;
        }
    }
    let mut anchors = budget.reserve(q, usize::MAX)?;
    for h in 0..q {
        let r = root(&mut parent, h);
        let old = anchors[r];
        if old == usize::MAX || mass_b[h] > mass_b[old] {
            anchors[r] = h;
        }
    }
    let mut free = budget.reserve(q, usize::MAX)?;
    let mut d = 0;
    let mut roots = budget.reserve(q, 0.)?;
    for h in 0..q {
        roots[h] = matrix[h * q + h].sqrt();
        if anchors[root(&mut parent, h)] != h {
            if !roots[h].is_finite() || roots[h] <= 0. {
                return Err(declined());
            }
            free[h] = d;
            d += 1;
        }
    }
    let mut factor = budget.reserve(d * d, 0.)?;
    for h in 0..q {
        if free[h] != usize::MAX {
            for j in 0..q {
                if free[j] != usize::MAX {
                    factor[free[h] * d + free[j]] = (matrix[h * q + j] / roots[h]) / roots[j];
                }
            }
        }
    }
    let guard = 128. * f64::EPSILON * d.max(1) as f64;
    for j in 0..d {
        let mut diagonal = factor[j * d + j];
        for k in 0..j {
            diagonal -= factor[j * d + k].powi(2);
        }
        if !diagonal.is_finite() || diagonal <= guard {
            return Err(declined());
        }
        factor[j * d + j] = diagonal.sqrt();
        for i in j + 1..d {
            let mut value = factor[i * d + j];
            for k in 0..j {
                value -= factor[i * d + k] * factor[j * d + k];
            }
            factor[i * d + j] = value / factor[j * d + j];
        }
    }
    let setup_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let mut scales = budget.reserve(width, 0.)?;
    let mut means = budget.reserve(width, 0.)?;
    let mut left = budget.reserve(p * width, 0.)?;
    let total: f64 = mass_a.iter().sum();
    if !total.is_finite() || total <= 0. {
        return Err(declined());
    }
    left.par_chunks_mut(p)
        .zip(scales.par_iter_mut())
        .zip(means.par_iter_mut())
        .enumerate()
        .try_for_each(|(j, ((values, norm), mean))| -> Result<()> {
            let mut scale = 0_f64;
            let mut square = 1.;
            for i in 0..n {
                let value = columns[j].value(i)?;
                if !value.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "canonical original column is nonfinite",
                    ));
                }
                let absolute = (value * w[i].sqrt()).abs();
                if absolute > scale {
                    square = 1. + square * (scale / absolute).powi(2);
                    scale = absolute;
                } else if absolute > 0. {
                    square += (absolute / scale).powi(2);
                }
            }
            *norm = scale * square.sqrt();
            if !norm.is_finite() {
                return Err(declined());
            }
            let norm = nonzero(*norm);
            let mut mean_sum = 0.;
            let mut mean_correction = 0.;
            for g in 0..p {
                let mut sum = 0.;
                let mut correction = 0.;
                for &i in a.level_index().members(g) {
                    let value = (w[i] / mass_a[g]) * (columns[j].value(i)? / norm);
                    let term = value - correction;
                    let next = sum + term;
                    correction = (next - sum) - term;
                    sum = next;
                }
                values[g] = sum;
                let term = (mass_a[g] / total) * sum - mean_correction;
                let next = mean_sum + term;
                mean_correction = (next - mean_sum) - term;
                mean_sum = next;
            }
            *mean = mean_sum * norm;
            if !mean.is_finite() || values.iter().any(|x| !x.is_finite()) {
                return Err(declined());
            }
            Ok(())
        })?;
    let mut partials = budget.reserve(workers * stride, 0.)?;
    let rows_per_worker = n.div_ceil(workers);
    partials.par_chunks_mut(stride).enumerate().try_for_each(
        |(worker, partial)| -> Result<()> {
            for i in worker * rows_per_worker..((worker + 1) * rows_per_worker).min(n) {
                let g = a.codes()[i] as usize;
                let h = b.codes()[i] as usize;
                for j in 0..width {
                    let value = columns[j].value(i)? / nonzero(scales[j]) - left[j * p + g];
                    partial[h * width + j] = w[i].mul_add(value, partial[h * width + j]);
                }
            }
            Ok(())
        },
    )?;
    let mut cross = budget.reserve(q * width, 0.)?;
    for slot in 0..cross.len() {
        cross[slot] = merge(&partials, stride, slot);
    }
    let mut solved = budget.reserve(d * width, 0.)?;
    if d > 0 {
        solved
            .par_chunks_mut(d)
            .enumerate()
            .try_for_each(|(j, values)| -> Result<()> {
                for h in 0..q {
                    if free[h] != usize::MAX {
                        values[free[h]] = cross[h * width + j] / roots[h];
                    }
                }
                for i in 0..d {
                    for k in 0..i {
                        values[i] -= factor[i * d + k] * values[k];
                    }
                    values[i] /= factor[i * d + i];
                }
                for i in (0..d).rev() {
                    for k in i + 1..d {
                        values[i] -= factor[k * d + i] * values[k];
                    }
                    values[i] /= factor[i * d + i];
                }
                if values.iter().any(|x| !x.is_finite()) {
                    return Err(declined());
                }
                Ok(())
            })?;
    }
    let mut right = budget.reserve(q * width, 0.)?;
    right.par_chunks_mut(q).enumerate().for_each(|(j, values)| {
        for h in 0..q {
            if free[h] != usize::MAX {
                values[h] = solved[j * d + free[h]] / roots[h];
            }
        }
    });
    left.par_chunks_mut(p).enumerate().for_each(|(j, values)| {
        for g in 0..p {
            let mut sum = 0.;
            for &i in a.level_index().members(g) {
                sum += (w[i] / mass_a[g]) * right[j * q + b.codes()[i] as usize];
            }
            values[g] -= sum;
        }
    });
    let solve_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    partials.fill(0.);
    let groups_per_worker = p.div_ceil(workers);
    partials.par_chunks_mut(stride).enumerate().try_for_each(
        |(worker, partial)| -> Result<()> {
            let (gram, rest) = partial.split_at_mut(width * width);
            let (scores, rest) = rest.split_at_mut(q * width);
            let (row, rest) = rest.split_at_mut(width);
            let (sums, rest) = rest.split_at_mut(width);
            let (maximum, rest) = rest.split_at_mut(width);
            let (squares, absolute) = rest.split_at_mut(width);
            for g in worker * groups_per_worker..((worker + 1) * groups_per_worker).min(p) {
                sums.fill(0.);
                for &i in a.level_index().members(g) {
                    let h = b.codes()[i] as usize;
                    for j in 0..width {
                        row[j] = columns[j].value(i)? / nonzero(scales[j])
                            - left[j * p + g]
                            - right[j * q + h];
                        if !row[j].is_finite() {
                            return Err(declined());
                        }
                        let score = w[i] * row[j];
                        sums[j] += score;
                        scores[h * width + j] += score;
                    }
                    for j in 0..width {
                        let value = w[i] * row[j];
                        for k in j..width {
                            gram[j * width + k] = value.mul_add(row[k], gram[j * width + k]);
                        }
                    }
                }
                for j in 0..width {
                    let value = sums[j].abs() / mass_a[g].sqrt();
                    if !value.is_finite() {
                        return Err(declined());
                    }
                    maximum[j] = maximum[j].max(value);
                    squares[j] = value.mul_add(value, squares[j]);
                    absolute[j] = absolute[j].max(sums[j].abs());
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
    let mut maximum = 0_f64;
    let mut absolute = 0_f64;
    let mut squares = 0.;
    for j in 0..width {
        let norm = gram[j * width + j].sqrt();
        let denominator = norm.max(1.);
        norms[j] = norm * nonzero(scales[j]);
        if !norms[j].is_finite() {
            return Err(declined());
        }
        let mut column_max = 0_f64;
        let mut column_abs = 0_f64;
        let mut column_square = 0.;
        for partial in partials.chunks(stride) {
            column_max = column_max.max(partial[stride - 3 * width + j]);
            column_square += partial[stride - 2 * width + j];
            column_abs = column_abs.max(partial[stride - width + j]);
        }
        for h in 0..q {
            let score = merge(&partials, stride, width * width + h * width + j).abs();
            let value = score / mass_b[h].sqrt();
            if !value.is_finite() {
                return Err(declined());
            }
            column_max = column_max.max(value);
            column_square = value.mul_add(value, column_square);
            column_abs = column_abs.max(score);
        }
        maximum = maximum.max(column_max / denominator);
        squares += column_square / (denominator * denominator);
        absolute = absolute.max((column_abs * nonzero(scales[j])) * divisor);
    }
    if !maximum.is_finite()
        || maximum > config.tolerance
        || !squares.is_finite()
        || !absolute.is_finite()
    {
        return Err(declined());
    }
    let mut within_response = budget.reserve(n, 0.)?;
    within_response
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(i, value)| -> Result<()> {
            *value = (response[i] / nonzero(scales[0])
                - left[a.codes()[i] as usize]
                - right[b.codes()[i] as usize])
                * nonzero(scales[0]);
            if !value.is_finite() {
                return Err(declined());
            }
            Ok(())
        })?;
    let report = SolveReport {
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations: 1,
        checkpoint_retries: 0,
        solver_workspace_peak: SolverWorkspacePeak::from_bytes(bytes)?,
        stop_reason: SolveStopReason::Certified,
        certificate: FeCertificate {
            absolute_infinity_norm: absolute,
            relative_infinity_norm: maximum,
            relative_two_norm: (squares / ((p + q) * width) as f64).sqrt(),
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
        left_effect,
        left,
        right,
        scales,
        means,
        norms,
        response: within_response,
        moments,
        report,
    })
}

fn merge(partials: &[f64], stride: usize, slot: usize) -> f64 {
    let mut sum = 0.;
    let mut correction = 0.;
    for partial in partials.chunks(stride) {
        let term = partial[slot] - correction;
        let next = sum + term;
        correction = (next - sum) - term;
        sum = next;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::*;
    use crate::ols::{
        prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
        OlsRequestV1, PreparedOlsData,
    };

    fn prepared(y: &[f64], x: &[Vec<f64>], g: &[f64], h: &[f64]) -> PreparedOlsData {
        let mut sources = Vec::new();
        let mut terms = Vec::new();
        let mut numeric = Vec::new();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        for (j, values) in x.iter().map(Vec::as_slice).chain([g, h]).enumerate() {
            let id = (j + 1) as u32;
            let source = SourceId::new(id).unwrap();
            sources.push(SourceEntry {
                id: source,
                canonical_name: format!("v{id}"),
            });
            bindings.push(OlsInputBinding {
                stata_var_position: id + 2,
                role: OlsInputRole::NumericSource(source),
            });
            numeric.push((
                source,
                LoadedNumericColumn {
                    values: values.to_vec(),
                    missing: vec![false; y.len()],
                },
            ));
            if j < x.len() {
                terms.push(ExpandedTerm {
                    id: TermId::new(id).unwrap(),
                    order: TermOrder::new(id).unwrap(),
                    coefficient_slot: CoefficientSlot::new(id).unwrap(),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: false,
                    canonical_token: format!("c.v{id}"),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).unwrap(),
                        source,
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::Unavailable,
                        raw_operator: "c".into(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    }],
                });
            }
        }
        prepare_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph { sources, terms },
                absorbed_terms: (0..2)
                    .map(|j| AbsorbedTerm {
                        id: AbsorbTermId::new(j + 1).unwrap(),
                        intercept: true,
                        categorical_sources: vec![SourceId::new(x.len() as u32 + j + 1).unwrap()],
                        slope_sources: Vec::new(),
                        label: format!("fe{j}"),
                        saved_name: String::new(),
                    })
                    .collect(),
                bindings,
                include_intercept: true,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; y.len()],
                outcome: LoadedNumericColumn {
                    values: y.to_vec(),
                    missing: vec![false; y.len()],
                },
                numeric_sources: numeric,
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn canonical_products_session_keeps_rank_and_strict_rollback_across_weights() {
        use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
        use crate::ols::prepared_fit::LinearSolveFrame;
        use crate::ols::{
            PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig,
        };
        let n = 4096;
        let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| ((i / 9) % 6) as f64).collect();
        for duplicate in [false, true] {
            let mut x: Vec<Vec<_>> = (0..8)
                .map(|j| {
                    (0..n)
                        .map(|i| {
                            (i as f64 / (13 + 7 * j) as f64).sin()
                                + (i % (7 + j)) as f64 / (5 + j) as f64
                        })
                        .collect()
                })
                .collect();
            if duplicate {
                x[7] = x[2].clone();
            }
            let y: Vec<_> = (0..n)
                .map(|i| {
                    0.2 * g[i] - 0.1 * h[i]
                        + x.iter()
                            .enumerate()
                            .map(|(j, x)| 0.03 * (j + 1) as f64 * x[i])
                            .sum::<f64>()
                })
                .collect();
            let prepared = prepared(&y, &x, &g, &h);
            for threads in [1, 4] {
                let mut config = PreparedOlsFitConfig::default();
                config.absorb.solver = FeSolverKind::Map;
                config.execution.mode = PreparedOlsExecutionMode::Low;
                config.execution.poolsize = 8;
                config.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                let mut scale = PreparedOlsBreadWeightScale {
                    weight_kind: None,
                    solver_weight_divisor: 1.,
                    reference_weight_multiplier: 1.,
                    reference_weight_sum: n as f64,
                };
                let frame =
                    LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Unit, scale).unwrap();
                let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
                    &prepared,
                    frame,
                    config,
                    LinearSessionGenerations::default(),
                    prepared.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                let original = session.solve_fresh().unwrap();
                // A direct certified solve reports one solve, proving that
                // this test reaches the fused handoff rather than its fallback.
                assert_eq!(original.fit().absorption_report().iterations, 1);
                assert!(original
                    .fit()
                    .residuals()
                    .iter()
                    .all(|r| r.abs() < 1e-10));
                let statuses: Vec<_> = original
                    .fit()
                    .coefficients()
                    .iter()
                    .map(|c| c.status)
                    .collect();
                let changed: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
                scale.solver_weight_divisor = changed.iter().copied().fold(0_f64, f64::max);
                scale.reference_weight_multiplier = scale.solver_weight_divisor;
                scale.reference_weight_sum = changed.iter().sum();
                session
                    .refresh_weights(BorrowedMapWeights::Provided(&changed), scale, 1)
                    .unwrap();
                // A dropped speculative candidate cannot publish its basis.
                {
                    let candidate = session.strict_candidate().unwrap();
                    assert!(candidate
                        .candidate()
                        .fit()
                        .residuals()
                        .iter()
                        .all(|r| r.abs() < 1e-10));
                }
                let candidate = session.strict_candidate().unwrap();
                assert_eq!(
                    candidate
                        .candidate()
                        .fit()
                        .coefficients()
                        .iter()
                        .map(|c| c.status)
                        .collect::<Vec<_>>(),
                    statuses
                );
                candidate.commit();
                assert!(session.output().unwrap().fit().score_certificate().passed);
            }
        }
    }

    #[test]
    fn canonical_products_match_independent_weighted_contrasts_and_direct_gram() {
        for threads in [1, 4, 8] {
            let team = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            team.install(|| {
                for mass in [1., 1e-80, 1e80] {
                    for shift in [0., 0.7] {
                        let wa: Vec<_> = (0..6).map(|a| 1. + a as f64 + shift).collect();
                        let wb: Vec<_> = (0..5).map(|b| 0.5 + b as f64 + shift).collect();
                        let ma = wa
                            .iter()
                            .enumerate()
                            .map(|(i, w)| i as f64 * w)
                            .sum::<f64>()
                            / wa.iter().sum::<f64>();
                        let mb = wb
                            .iter()
                            .enumerate()
                            .map(|(i, w)| i as f64 * w)
                            .sum::<f64>()
                            / wb.iter().sum::<f64>();
                        let mut g = Vec::new();
                        let mut h = Vec::new();
                        let mut weights = Vec::new();
                        let mut values = vec![Vec::new(); 4];
                        let mut truth = vec![Vec::new(); 4];
                        let scales = [1., 1e70, 1e-70, 1.];
                        for component in 0..2 {
                            for a in 0..6 {
                                for b in 0..5 {
                                    for t in 0..3 {
                                        g.push((component * 6 + a) as f64);
                                        h.push((component * 5 + b) as f64);
                                        weights.push(
                                            wa[a]
                                                * wb[b]
                                                * if component == 0 { mass } else { 1. / mass },
                                        );
                                        let ab = (a as f64 - ma) * (b as f64 - mb);
                                        let noise = t as f64 - 1.;
                                        for (j, r) in [ab, noise, ab + noise * (b as f64 - mb), 0.]
                                            .into_iter()
                                            .enumerate()
                                        {
                                            truth[j].push(r * scales[j]);
                                            values[j].push(
                                                (r + 0.3 * a as f64 - 0.2 * b as f64
                                                    + component as f64)
                                                    * scales[j],
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        let p = prepared(&values[0], &values[1..], &g, &h);
                        let max = weights.iter().copied().fold(0_f64, f64::max);
                        let config = MapConfig {
                            tolerance: 1e-12,
                            ..MapConfig::default()
                        };
                        let result = try_build(
                            &p,
                            &[0, 1, 2],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            max,
                            config,
                            u128::MAX,
                        )
                        .unwrap()
                        .expect("admitted exact contrasts");
                        assert!(result.report.certified());
                        let mut scratch = PreparedOlsScratch::create_row_major(g.len(), 3).unwrap();
                        result.write_controls(&mut scratch).unwrap();
                        let mut row = [0.; 3];
                        for i in 0..g.len() {
                            scratch.read_linear_shared(i * 3, &mut row).unwrap();
                            assert!((result.response[i] - truth[0][i]).abs() < 2e-12);
                            for j in 1..4 {
                                assert!((row[j - 1] - truth[j][i]).abs() / scales[j] < 2e-12);
                            }
                        }
                        for j in 0..4 {
                            for k in 0..4 {
                                let expected = (0..g.len())
                                    .map(|i| {
                                        (weights[i] / max)
                                            * (truth[j][i] / nonzero(result.scales[j]))
                                            * (truth[k][i] / nonzero(result.scales[k]))
                                    })
                                    .sum::<f64>();
                                assert!((result.moments.gram[j * 4 + k] - expected).abs() < 2e-13);
                            }
                        }
                        assert!(result.norms[3] / result.scales[3] < 2e-14);
                        let design = p.fit_borrowed_intercept_map_design().unwrap().unwrap();
                        let bytes = Plan::new(&design, 4, 0, threads).unwrap().bytes;
                        assert!(try_build(
                            &p,
                            &[0, 1, 2],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            max,
                            config,
                            bytes - 1
                        )
                        .unwrap()
                        .is_none());
                        assert!(try_build(
                            &p,
                            &[0, 1, 2],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            max,
                            config,
                            bytes
                        )
                        .unwrap()
                        .is_some());
                    }
                }
            });
        }
    }

    #[test]
    fn canonical_products_refuse_work_and_invalid_frames_without_mutation() {
        let n = 240;
        let g: Vec<_> = (0..n).map(|i| (i % 12) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| ((i / 12) % 5) as f64).collect();
        let x: Vec<_> = (0..n).map(|i| (i as f64).sin()).collect();
        let y: Vec<_> = (0..n).map(|i| g[i] + h[i] + 1e-10 * x[i]).collect();
        let p = prepared(&y, &[x.clone(), vec![0.; n]], &g, &h);
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        team.install(|| {
            let result = try_build(
                &p,
                &[0, 1],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                MapConfig::default(),
                u128::MAX,
            )
            .unwrap()
            .unwrap();
            assert!(result.moments.gram[0] >= 0. && result.moments.gram[0] < 1e-20);
            assert_eq!(result.scales[2], 0.);
            assert_eq!(result.norms[2], 0.);
            let mut bad = vec![1.; n];
            bad[4] = 0.;
            assert!(try_build(
                &p,
                &[0, 1],
                &y,
                BorrowedMapWeights::Provided(&bad),
                1.,
                MapConfig::default(),
                u128::MAX
            )
            .unwrap()
            .is_none());
            let mut invalid = y.clone();
            invalid[5] = f64::NAN;
            assert_eq!(
                try_build(
                    &p,
                    &[0, 1],
                    &invalid,
                    BorrowedMapWeights::Unit,
                    1.,
                    MapConfig::default(),
                    u128::MAX
                )
                .err()
                .unwrap()
                .code,
                ErrorCode::InvalidData
            );
            let config = MapConfig {
                tolerance: 1e-30,
                ..MapConfig::default()
            };
            assert!(try_build(
                &p,
                &[0, 1],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                config,
                u128::MAX
            )
            .unwrap()
            .is_none());
            let large: Vec<_> = (0..n).map(|i| (i % 60) as f64).collect();
            let other: Vec<_> = (0..n).map(|i| ((i / 4 + i) % 60) as f64).collect();
            let wide = prepared(&y, &[x], &large, &other);
            assert!(try_build(
                &wide,
                &[0],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                MapConfig::default(),
                u128::MAX
            )
            .unwrap()
            .is_none());
        });
    }
}
