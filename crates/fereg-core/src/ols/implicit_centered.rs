//! One-intercept-FE rows backed by immutable prepared columns and current-weight
//! group centers. No observation-by-control bank is owned by this representation.
use rayon::prelude::*;
use std::time::Instant;

use super::linear_session::{compensated_add, stable_norm, update_scaled_square};
use super::{PreparedOlsColumnKind, PreparedOlsExecutionMode};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedInterceptEffect, BorrowedMapWeights, FeCertificate, FeSolverKind, SolveReport,
    SolveStopReason,
};
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::prepared_fit_view::{PreparedOlsFitView, SharedPreparedOlsData};

#[derive(Debug, Clone, Copy)]
pub(super) enum BoundColumn<'a> {
    Numeric(&'a [f64]),
    Generated(&'a PreparedCategoricalOlsData, usize),
}
impl<'a> BoundColumn<'a> {
    fn new(prepared: &'a PreparedCategoricalOlsData, column: usize) -> Result<Self> {
        let recipe = prepared.model().columns.get(column).ok_or_else(invalid)?;
        match recipe.kind {
            PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } => {
                let values = prepared
                    .sample()
                    .regressors
                    .get(prepared_source_index)
                    .ok_or_else(invalid)?;
                if values.len() != prepared.sample().n_active() {
                    return Err(invalid());
                }
                Ok(Self::Numeric(values))
            }
            _ => Ok(Self::Generated(prepared, column)),
        }
    }
    #[inline]
    pub(super) fn value(self, row: usize) -> Result<f64> {
        match self {
            Self::Numeric(values) => Ok(values[row]),
            Self::Generated(prepared, column) => prepared.fit_column_value(column, row),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ColumnStats {
    raw_norm: f64,
    mean: f64,
    within_norm: f64,
    absolute: f64,
    relative: f64,
    squares: f64,
}

#[derive(Debug)]
pub(crate) struct ImplicitCenteredDesign {
    source: SharedPreparedOlsData,
    generated: Vec<usize>,
    centers: Vec<f64>, // controls, then current response; column-major groups
    group_weights: Vec<f64>,
    stats: Vec<ColumnStats>,
    mode: PreparedOlsExecutionMode,
    valid: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ImplicitCenteredView<'a> {
    prepared: &'a PreparedCategoricalOlsData,
    generated: &'a [usize],
    centers: &'a [f64],
    effect: BorrowedInterceptEffect<'a>,
}
impl<'a> ImplicitCenteredView<'a> {
    pub(super) fn rows(self) -> usize {
        self.effect.codes().len()
    }
    pub(super) fn columns(self) -> usize {
        self.generated.len()
    }
    pub(super) fn groups(self) -> usize {
        self.effect.n_levels()
    }
    pub(super) fn codes(self) -> &'a [u32] {
        self.effect.codes()
    }
    pub(super) fn center(self, column: usize, group: usize) -> f64 {
        self.centers[column * self.groups() + group]
    }
    pub(super) fn bind_columns(self) -> Result<Vec<BoundColumn<'a>>> {
        let mut bound = reserve(self.columns())?;
        for &column in self.generated {
            bound.push(BoundColumn::new(self.prepared, column)?);
        }
        Ok(bound)
    }
    pub(super) fn fill_selected_row(
        self,
        row: usize,
        selected: &[usize],
        output: &mut [f64],
    ) -> Result<()> {
        if row >= self.rows() || selected.len() != output.len() {
            return Err(invalid());
        }
        let group = self.codes()[row] as usize;
        for (target, &column) in output.iter_mut().zip(selected) {
            let generated = *self.generated.get(column).ok_or_else(invalid)?;
            *target = BoundColumn::new(self.prepared, generated)?.value(row)?
                - self.center(column, group);
            if !target.is_finite() {
                return Err(numerical());
            }
        }
        Ok(())
    }
    pub(super) fn fill_row(self, row: usize, output: &mut [f64]) -> Result<()> {
        if row >= self.rows() || output.len() != self.columns() {
            return Err(invalid());
        }
        let group = self.codes()[row] as usize;
        for (column, target) in output.iter_mut().enumerate() {
            *target = BoundColumn::new(self.prepared, self.generated[column])?.value(row)?
                - self.center(column, group);
            if !target.is_finite() {
                return Err(numerical());
            }
        }
        Ok(())
    }
}

impl ImplicitCenteredDesign {
    /// Conservative extra lifetime charge, without crediting the replaced dense
    /// bank or scratch blocks. Includes finite-kernel bindings and worker rows.
    pub(crate) fn bytes(groups: usize, columns: usize, threads: usize) -> Result<u128> {
        let g = groups as u128;
        let k = columns as u128;
        let workers = k
            .checked_mul(threads.max(1) as u128)
            .and_then(|x| x.checked_mul((8 + std::mem::size_of::<BoundColumn<'_>>()) as u128))
            .ok_or_else(allocation)?;
        let result = (k + 1)
            .checked_mul(g)
            .and_then(|x| x.checked_mul(8))
            .and_then(|x| x.checked_add(g * 8))
            .and_then(|x| x.checked_add((k + 1) * std::mem::size_of::<ColumnStats>() as u128))
            .and_then(|x| x.checked_add(k * (8 + std::mem::size_of::<BoundColumn<'_>>() as u128)))
            .and_then(|x| x.checked_add(workers));
        result.ok_or_else(allocation)
    }
    pub(super) fn new(
        source: SharedPreparedOlsData,
        generated: &[usize],
        mode: PreparedOlsExecutionMode,
    ) -> Result<Self> {
        let prepared = source.as_ref();
        if !prepared.fit_has_one_ordinary_intercept_effect() {
            return Err(invalid());
        }
        let effect = prepared.validated_single_effect().ok_or_else(invalid)?;
        let groups = effect.n_levels();
        if groups == 0
            || effect.codes().len() != prepared.sample().n_active()
            || generated.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(invalid());
        }
        for &column in generated {
            BoundColumn::new(prepared, column)?;
        }
        let mut ids = reserve(generated.len())?;
        ids.extend_from_slice(generated);
        let width = generated.len().checked_add(1).ok_or_else(allocation)?;
        Ok(Self {
            source,
            generated: ids,
            centers: filled(groups.checked_mul(width).ok_or_else(allocation)?, 0.)?,
            group_weights: filled(groups, 0.)?,
            stats: filled(width, ColumnStats::default())?,
            mode,
            valid: false,
        })
    }
    pub(super) fn rows(&self) -> usize {
        self.source.as_ref().sample().n_active()
    }
    pub(super) fn columns(&self) -> usize {
        self.generated.len()
    }
    pub(super) fn mode(&self) -> PreparedOlsExecutionMode {
        self.mode
    }
    pub(super) fn resident_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.generated)
            + crate::memory::vec_capacity_bytes(&self.centers)
            + crate::memory::vec_capacity_bytes(&self.group_weights)
            + crate::memory::vec_capacity_bytes(&self.stats)
    }
    pub(super) fn view(&self) -> Result<ImplicitCenteredView<'_>> {
        if !self.valid {
            return Err(invalid());
        }
        let prepared = self.source.as_ref();
        let effect = prepared.validated_single_effect().ok_or_else(invalid)?;
        Ok(ImplicitCenteredView {
            prepared,
            generated: &self.generated,
            centers: &self.centers,
            effect,
        })
    }
    pub(super) fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Materialize the independently certified differences into the fast
    /// session's one retained bank. Geometry is temporary on this route;
    /// finite products and inference continue to read contiguous dense rows.
    pub(super) fn write_dense(&self, output: &mut DenseMatrix) -> Result<()> {
        let view = self.view()?;
        if output.nrows() != view.rows() || output.ncols() != view.columns() {
            return Err(invalid());
        }
        if view.columns() == 0 {
            return Ok(());
        }
        let columns = view.bind_columns()?;
        output
            .as_mut_slice()
            .par_chunks_mut(view.columns())
            .enumerate()
            .try_for_each(|(row, values)| -> Result<()> {
                let group = view.codes()[row] as usize;
                for (column, value) in values.iter_mut().enumerate() {
                    *value = columns[column].value(row)? - view.center(column, group);
                    if !value.is_finite() {
                        return Err(numerical());
                    }
                }
                Ok(())
            })
    }
    pub(super) fn compact(&mut self, positions: &[usize]) -> Result<()> {
        if !self.valid
            || positions.windows(2).any(|p| p[0] >= p[1])
            || positions.last().is_some_and(|&i| i >= self.columns())
        {
            return Err(invalid());
        }
        let groups = self.group_weights.len();
        let old = self.columns();
        for (target, &source) in positions.iter().enumerate() {
            self.generated[target] = self.generated[source];
            self.centers
                .copy_within(source * groups..(source + 1) * groups, target * groups);
            self.stats[target] = self.stats[source];
        }
        self.centers
            .copy_within(old * groups..(old + 1) * groups, positions.len() * groups);
        self.stats[positions.len()] = self.stats[old];
        self.generated.truncate(positions.len());
        self.centers.truncate((positions.len() + 1) * groups);
        self.stats.truncate(positions.len() + 1);
        Ok(())
    }
    /// All statistics and acceptance are recomputed under this exact frame.
    /// Publication authority is revoked before any fallible operation.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn refresh(
        &mut self,
        response: &[f64],
        weights: BorrowedMapWeights<'_>,
        divisor: f64,
        tolerance: f64,
        raw_norms: &mut [f64],
        means: &mut [f64],
        within_norms: &mut [f64],
        mut projected: Vec<f64>,
    ) -> Result<(Vec<f64>, SolveReport)> {
        self.valid = false;
        let rows = self.rows();
        let columns = self.columns();
        let groups = self.group_weights.len();
        if response.len() != rows
            || raw_norms.len() != columns
            || means.len() != columns
            || within_norms.len() != columns
            || !divisor.is_finite()
            || divisor <= 0.
            || !tolerance.is_finite()
            || tolerance <= 0.
            || response.iter().any(|v| !v.is_finite())
        {
            return Err(invalid());
        }
        if let BorrowedMapWeights::Provided(w) = weights {
            if w.len() != rows {
                return Err(invalid());
            }
        }
        let mut weight_sum = 0.;
        let mut weight_correction = 0.;
        for row in 0..rows {
            compensated_add(
                weight(weights, divisor, row)?,
                &mut weight_sum,
                &mut weight_correction,
            )?;
        }
        let total = weight_sum + weight_correction;
        if !total.is_finite() || total <= 0. {
            return Err(numerical());
        }
        let prepared = self.source.as_ref();
        let effect = prepared.validated_single_effect().ok_or_else(invalid)?;
        let start = Instant::now();
        self.group_weights.par_iter_mut().enumerate().try_for_each(
            |(group, target)| -> Result<()> {
                let (mut sum, mut correction) = (0., 0.);
                for &row in effect.level_index().members(group) {
                    compensated_add(weight(weights, divisor, row)?, &mut sum, &mut correction)?;
                }
                *target = sum + correction;
                if !target.is_finite() || *target <= 0. {
                    return Err(numerical());
                }
                Ok(())
            },
        )?;
        self.centers
            .par_chunks_mut(groups)
            .enumerate()
            .try_for_each(|(position, centers)| -> Result<()> {
                let column = if position == columns {
                    BoundColumn::Numeric(response)
                } else {
                    BoundColumn::new(prepared, self.generated[position])?
                };
                for (group, center) in centers.iter_mut().enumerate() {
                    *center = group_mean(
                        column,
                        effect.level_index().members(group),
                        0.,
                        weights,
                        divisor,
                        self.group_weights[group],
                    )?;
                }
                Ok(())
            })?;
        let setup_seconds = start.elapsed().as_secs_f64();
        let start = Instant::now();
        self.stats
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(position, stats)| -> Result<()> {
                let column = if position == columns {
                    BoundColumn::Numeric(response)
                } else {
                    BoundColumn::new(prepared, self.generated[position])?
                };
                let (mut raw_scale, mut raw_sum, mut within_scale, mut within_sum) =
                    (0., 1., 0., 1.);
                let (mut sum, mut correction) = (0., 0.);
                for row in 0..rows {
                    let w = weight(weights, divisor, row)?;
                    let value = column.value(row)?;
                    let centered =
                        value - self.centers[position * groups + effect.codes()[row] as usize];
                    update_scaled_square(w.sqrt() * value, &mut raw_scale, &mut raw_sum)?;
                    update_scaled_square(w.sqrt() * centered, &mut within_scale, &mut within_sum)?;
                    compensated_add(w * value, &mut sum, &mut correction)?;
                }
                *stats = ColumnStats {
                    raw_norm: stable_norm(raw_scale, raw_sum)?,
                    within_norm: stable_norm(within_scale, within_sum)?,
                    mean: (sum + correction) / total,
                    ..ColumnStats::default()
                };
                if !stats.mean.is_finite() {
                    return Err(numerical());
                }
                Ok(())
            })?;
        projected
            .try_reserve(rows.saturating_sub(projected.len()))
            .map_err(|_| allocation())?;
        projected.resize(rows, 0.);
        for (row, value) in projected.iter_mut().enumerate() {
            *value = response[row] - self.centers[columns * groups + effect.codes()[row] as usize];
        }
        let solve_seconds = start.elapsed().as_secs_f64();
        let start = Instant::now();
        // Replay original-minus-proposed effects. Never reuse the construction's
        // value sums or a recurrence residual as the acceptance witness.
        self.stats
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(position, stats)| -> Result<()> {
                let column = if position == columns {
                    BoundColumn::Numeric(response)
                } else {
                    BoundColumn::new(prepared, self.generated[position])?
                };
                let denominator = stats.raw_norm.max(stats.within_norm);
                for group in 0..groups {
                    let moment = group_mean(
                        column,
                        effect.level_index().members(group),
                        self.centers[position * groups + group],
                        weights,
                        divisor,
                        self.group_weights[group],
                    )?;
                    let ratio = if denominator == 0. {
                        if moment != 0. {
                            return Err(numerical());
                        }
                        0.
                    } else {
                        moment.abs() / denominator * self.group_weights[group].sqrt()
                    };
                    if !ratio.is_finite() {
                        return Err(numerical());
                    }
                    stats.relative = stats.relative.max(ratio);
                    stats.squares = ratio.mul_add(ratio, stats.squares);
                    let normalized = moment.abs() * self.group_weights[group];
                    stats.absolute = stats.absolute.max(if normalized == 0. {
                        0.
                    } else {
                        normalized * divisor
                    });
                }
                Ok(())
            })?;
        let mut certificate = FeCertificate {
            absolute_infinity_norm: 0.,
            relative_infinity_norm: 0.,
            relative_two_norm: 0.,
            passed: false,
        };
        let mut squares = 0.;
        for stats in &self.stats {
            certificate.absolute_infinity_norm =
                certificate.absolute_infinity_norm.max(stats.absolute);
            certificate.relative_infinity_norm =
                certificate.relative_infinity_norm.max(stats.relative);
            squares += stats.squares;
        }
        certificate.relative_two_norm =
            (squares / (groups as f64 * self.stats.len() as f64)).sqrt();
        certificate.passed = certificate.relative_infinity_norm <= tolerance
            && certificate.relative_two_norm.is_finite();
        if !certificate.passed {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "implicit centered projection did not certify",
            ));
        }
        for (position, stats) in self.stats[..columns].iter().enumerate() {
            raw_norms[position] = stats.raw_norm;
            means[position] = stats.mean;
            within_norms[position] = stats.within_norm;
        }
        self.valid = true;
        Ok((
            projected,
            SolveReport {
                requested_solver: FeSolverKind::Map,
                used_solver: FeSolverKind::Map,
                iterations: 1,
                checkpoint_retries: 0,
                solver_workspace_peak: crate::memory::SolverWorkspacePeak::ZERO,
                stop_reason: SolveStopReason::Certified,
                certificate,
                normal_equation_residual: 0.,
                setup_seconds,
                solve_seconds,
                certification_seconds: start.elapsed().as_secs_f64(),
                fallback_used: false,
                requested_preconditioner: None,
                used_preconditioner: None,
                pilot_iterations: 0,
                pilot_contraction: 0.,
                warnings: Vec::new(),
            },
        ))
    }
}
fn group_mean(
    column: BoundColumn<'_>,
    members: &[usize],
    center: f64,
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    total: f64,
) -> Result<f64> {
    let mut scale = 0_f64;
    for &row in members {
        let value = column.value(row)? - center;
        if !value.is_finite() {
            return Err(numerical());
        }
        scale = scale.max(value.abs());
    }
    if scale == 0. {
        return Ok(0.);
    }
    let (mut sum, mut correction) = (0., 0.);
    for &row in members {
        compensated_add(
            weight(weights, divisor, row)? * ((column.value(row)? - center) / scale),
            &mut sum,
            &mut correction,
        )?;
    }
    let mean = scale * ((sum + correction) / total);
    if mean.is_finite() {
        Ok(mean)
    } else {
        Err(numerical())
    }
}
fn weight(weights: BorrowedMapWeights<'_>, divisor: f64, row: usize) -> Result<f64> {
    let value = match weights {
        BorrowedMapWeights::Unit => 1.,
        BorrowedMapWeights::Provided(w) => w[row],
    } / divisor;
    if value.is_finite() && value > 0. {
        Ok(value)
    } else {
        Err(numerical())
    }
}
fn reserve<T>(n: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(n).map_err(|_| allocation())?;
    Ok(values)
}
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut values = reserve(n)?;
    values.resize(n, value);
    Ok(values)
}
fn invalid() -> FeregError {
    FeregError::new(
        ErrorCode::InvalidArgument,
        "implicit centered design is not aligned or current",
    )
}
fn numerical() -> FeregError {
    FeregError::new(
        ErrorCode::InvalidData,
        "implicit centered arithmetic is not finite and valid",
    )
}
fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "unable to admit implicit centered storage",
    )
}

#[cfg(test)]
#[path = "implicit_centered_tests.rs"]
mod tests;
