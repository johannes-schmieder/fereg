//! Action-local geometry for a single, fixed-weight slope partition.
//!
//! Projection retains the raw Gram rank rule. Certification independently uses
//! the normalized, pivoted and twice-reorthogonalized basis of `certificate`.
//! A rejected projection returns the untouched input to the established solver.
use super::certificate::{euclidean_norm, update_scaled_sum_of_squares};
use super::{
    BorrowedInterceptEffect, BorrowedMapWeights, CategoricalAbsorbedEffectView,
    CertifiedBorrowedMap, FeCertificate, FeSolverKind, SolveReport, SolveStopReason,
};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::linalg::solve_linear_system;
use crate::memory::{FeSolverMemoryBudget, SolverWorkspacePeak};
use rayon::prelude::*;
use std::time::Instant;

const MAX_CHANNELS: usize = 4;
const MAX_RHS: usize = 64;

#[derive(Clone, Default)]
struct Group {
    start: usize,
    rank: usize,
    gram: [f64; MAX_CHANNELS * MAX_CHANNELS],
    raw_scales: [f64; MAX_CHANNELS],
}

#[derive(Clone, Default)]
struct Check {
    absolute: f64,
    relative: f64,
    squares: f64,
}

pub(crate) struct SingleSlopeAbsorber<'a> {
    partition: BorrowedInterceptEffect<'a>,
    effect: CategoricalAbsorbedEffectView<'a>,
    weights: BorrowedMapWeights<'a>,
    roots: Vec<f64>,
    groups: Vec<Group>,
    basis: Vec<f64>,
    max_rhs: usize,
    setup_seconds: f64,
    peak: SolverWorkspacePeak,
    absorption_bytes: u128,
}

pub(crate) enum SingleSlopeAttempt {
    Certified(CertifiedBorrowedMap),
    Rejected {
        original: DenseMatrix,
        peak: SolverWorkspacePeak,
        seconds: f64,
        iterations: usize,
    },
}

fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "single-partition slope workspace exceeds its admitted capacity",
    )
}
fn invalid() -> FeregError {
    FeregError::new(
        ErrorCode::InvalidData,
        "single-partition slope inputs or weighted geometry are invalid",
    )
}
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| allocation())?;
    v.resize(n, value);
    Ok(v)
}
fn bytes<T>(v: &Vec<T>) -> u128 {
    v.capacity() as u128 * std::mem::size_of::<T>() as u128
}
fn weight(weights: BorrowedMapWeights<'_>, row: usize) -> f64 {
    match weights {
        BorrowedMapWeights::Unit => 1.0,
        BorrowedMapWeights::Provided(w) => w[row],
    }
}
fn basis_value(effect: &CategoricalAbsorbedEffectView<'_>, channel: usize, row: usize) -> f64 {
    if effect.has_intercept() && channel == 0 {
        1.0
    } else {
        effect.slopes()[channel - usize::from(effect.has_intercept())][row]
    }
}

impl<'a> SingleSlopeAbsorber<'a> {
    pub(crate) fn build(
        partition: BorrowedInterceptEffect<'a>,
        effect: CategoricalAbsorbedEffectView<'a>,
        weights: BorrowedMapWeights<'a>,
        max_rhs: usize,
        budget: Option<FeSolverMemoryBudget>,
        attempted_peak: &mut SolverWorkspacePeak,
    ) -> Result<Option<Self>> {
        let started = Instant::now();
        let n = partition.codes().len();
        let levels = partition.n_levels();
        let channels = effect.channels();
        if !(1..=MAX_CHANNELS).contains(&channels)
            || effect.slopes().is_empty()
            || !(1..=MAX_RHS).contains(&max_rhs)
            || levels == 0
        {
            return Ok(None);
        }
        if effect
            .slopes()
            .iter()
            .any(|s| s.len() != n || s.iter().any(|x| !x.is_finite()))
            || matches!(weights, BorrowedMapWeights::Provided(w) if w.len()!=n || w.iter().any(|x| !x.is_finite() || *x<=0.0))
        {
            return Err(invalid());
        }
        let basis_len = n.checked_mul(channels).ok_or_else(allocation)?;
        let coefs = levels
            .checked_mul(channels)
            .and_then(|n| n.checked_mul(max_rhs))
            .ok_or_else(allocation)?;
        let output = n.checked_mul(max_rhs).ok_or_else(allocation)?;
        let retained = 8 * (basis_len as u128 + n as u128)
            + levels as u128 * std::mem::size_of::<Group>() as u128
            + effect.slope_storage_bytes();
        let construction = retained + 8 * basis_len as u128;
        let solve = retained
            + 8 * (coefs as u128 + output as u128 + 6 * max_rhs as u128)
            + levels as u128 * std::mem::size_of::<Check>() as u128;
        if budget.is_some_and(|b| !b.fits(construction.max(solve))) {
            return Ok(None);
        }
        *attempted_peak = attempted_peak.max(SolverWorkspacePeak::from_bytes(construction)?);
        let mut roots = filled(n, 0.0)?;
        roots
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, v)| *v = weight(weights, i).sqrt());
        let mut groups = filled(levels, Group::default())?;
        let mut start = 0;
        for (level, group) in groups.iter_mut().enumerate() {
            group.start = start;
            start += partition.level_index().members(level).len() * channels;
        }
        let mut basis = filled(basis_len, 0.0)?;
        let mut scratch = filled(basis_len, 0.0)?;
        let retained =
            bytes(&roots) + bytes(&groups) + bytes(&basis) + effect.slope_storage_bytes();
        let actual = retained + bytes(&scratch);
        let absorption_bytes = retained
            + 8 * (coefs as u128 + output as u128 + 6 * max_rhs as u128)
            + levels as u128 * std::mem::size_of::<Check>() as u128;
        *attempted_peak = attempted_peak.max(SolverWorkspacePeak::from_bytes(actual)?);
        if budget.is_some_and(|b| !b.fits(actual.max(absorption_bytes))) {
            return Ok(None);
        }
        build_groups(
            partition,
            &effect,
            weights,
            &roots,
            0,
            &mut groups,
            &mut basis,
            &mut scratch,
        )?;
        drop(scratch);
        Ok(Some(Self {
            partition,
            effect,
            weights,
            roots,
            groups,
            basis,
            max_rhs,
            setup_seconds: started.elapsed().as_secs_f64(),
            peak: *attempted_peak,
            absorption_bytes,
        }))
    }

    pub(crate) fn absorb(
        &mut self,
        original: DenseMatrix,
        tolerance: f64,
        budget: Option<FeSolverMemoryBudget>,
        original_norms: Option<&[f64]>,
    ) -> Result<SingleSlopeAttempt> {
        let started = Instant::now();
        let rhs = original.ncols();
        let n = original.nrows();
        let channels = self.effect.channels();
        if n != self.roots.len()
            || rhs == 0
            || rhs > self.max_rhs
            || !tolerance.is_finite()
            || tolerance <= 0.0
        {
            return Err(invalid());
        }
        if budget.is_some_and(|b| !b.fits(self.absorption_bytes)) {
            return Ok(SingleSlopeAttempt::Rejected {
                original,
                peak: self.peak,
                seconds: started.elapsed().as_secs_f64() + std::mem::take(&mut self.setup_seconds),
                iterations: 0,
            });
        }
        // This projector retains the generic slope solver's raw-weight
        // arithmetic. OLS's norm cache uses weights divided by their maximum;
        // the two scales coincide only for the unit-weight handoff.
        let original_norms = if matches!(self.weights, BorrowedMapWeights::Unit) {
            original_norms
        } else {
            None
        };
        let references = if let Some(norms) = original_norms {
            if norms.len() != rhs || norms.iter().any(|v| !v.is_finite() || *v < 0.0) {
                return Err(invalid());
            }
            let mut references = filled(rhs, 0.0)?;
            references.copy_from_slice(norms);
            references
        } else {
            self.norms(&original)?
        };
        let mut coefficients = filled(self.groups.len() * channels * rhs, 0.0)?;
        let mut result = DenseMatrix::zeros(n, rhs)?;
        let mut checks = filled(self.groups.len(), Check::default())?;
        let resident = bytes(&self.roots)
            + bytes(&self.groups)
            + bytes(&self.basis)
            + self.effect.slope_storage_bytes();
        let actual = resident
            + bytes(&coefficients)
            + bytes(&checks)
            + result.storage_capacity() as u128 * 8
            + bytes(&references)
            + 5 * rhs as u128 * 8;
        let peak = self.peak.max(SolverWorkspacePeak::from_bytes(actual)?);
        if budget.is_some_and(|b| !b.fits(actual)) {
            return Ok(SingleSlopeAttempt::Rejected {
                original,
                peak,
                seconds: started.elapsed().as_secs_f64() + std::mem::take(&mut self.setup_seconds),
                iterations: 0,
            });
        }
        coefficients
            .par_chunks_mut(channels * rhs)
            .enumerate()
            .try_for_each(|(level, coefs)| -> Result<()> {
                let members = self.partition.level_index().members(level);
                for &row in members {
                    let values = &original.as_slice()[row * rhs..(row + 1) * rhs];
                    for channel in 0..channels {
                        let left =
                            weight(self.weights, row) * basis_value(&self.effect, channel, row);
                        for (sum, &value) in coefs[channel * rhs..(channel + 1) * rhs]
                            .iter_mut()
                            .zip(values)
                        {
                            *sum = left.mul_add(value, *sum);
                        }
                    }
                }
                let mut gram = self.groups[level].gram;
                solve_linear_system(
                    &mut gram[..channels * channels],
                    coefs,
                    channels,
                    rhs,
                    1e-12,
                )?;
                Ok(())
            })?;
        result
            .as_mut_slice()
            .par_chunks_mut(rhs)
            .enumerate()
            .for_each(|(row, target)| {
                let level = self.partition.codes()[row] as usize;
                let coefs = &coefficients[level * channels * rhs..(level + 1) * channels * rhs];
                for channel in 0..channels {
                    let basis = basis_value(&self.effect, channel, row);
                    for (fitted, &coefficient) in target
                        .iter_mut()
                        .zip(&coefs[channel * rhs..(channel + 1) * rhs])
                    {
                        *fitted = basis.mul_add(coefficient, *fitted);
                    }
                }
                for (fitted, &value) in target
                    .iter_mut()
                    .zip(&original.as_slice()[row * rhs..(row + 1) * rhs])
                {
                    *fitted = value - *fitted;
                }
            });
        let certificate_started = Instant::now();
        let residual_norms = self.norms(&result)?;
        let mut scales = references;
        for (scale, norm) in scales.iter_mut().zip(&residual_norms) {
            *scale = scale.max(*norm);
        }
        checks
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(level, check)| -> Result<()> {
                *check = if rhs == 1 {
                    self.check_group::<1, 4>(level, &result, &scales)?
                } else if rhs <= 16 {
                    self.check_group::<16, 64>(level, &result, &scales)?
                } else {
                    self.check_group::<MAX_RHS, { MAX_CHANNELS * MAX_RHS }>(
                        level, &result, &scales,
                    )?
                };
                Ok(())
            })?;
        let mut certificate = FeCertificate {
            absolute_infinity_norm: 0.0,
            relative_infinity_norm: 0.0,
            relative_two_norm: 0.0,
            passed: false,
        };
        for check in checks {
            certificate.absolute_infinity_norm =
                certificate.absolute_infinity_norm.max(check.absolute);
            certificate.relative_infinity_norm =
                certificate.relative_infinity_norm.max(check.relative);
            certificate.relative_two_norm += check.squares;
        }
        certificate.relative_two_norm =
            (certificate.relative_two_norm / (self.groups.len() * rhs) as f64).sqrt();
        certificate.passed = certificate.relative_infinity_norm <= tolerance;
        let certification_seconds = certificate_started.elapsed().as_secs_f64();
        if !certificate.passed {
            return Ok(SingleSlopeAttempt::Rejected {
                original,
                peak,
                seconds: started.elapsed().as_secs_f64() + std::mem::take(&mut self.setup_seconds),
                iterations: 1,
            });
        }
        let report = SolveReport {
            checkpoint_retries: 0,
            solver_workspace_peak: peak,
            requested_solver: FeSolverKind::Auto,
            used_solver: FeSolverKind::Map,
            iterations: 1,
            stop_reason: SolveStopReason::Certified,
            normal_equation_residual: certificate.relative_two_norm,
            certificate,
            setup_seconds: std::mem::take(&mut self.setup_seconds),
            solve_seconds: started.elapsed().as_secs_f64() - certification_seconds,
            certification_seconds,
            fallback_used: false,
            requested_preconditioner: None,
            used_preconditioner: None,
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
        };
        let certified = CertifiedBorrowedMap::from_certified_parts(result, report)?;
        let certified = if matches!(self.weights, BorrowedMapWeights::Unit) {
            certified.with_residual_norms(residual_norms)?
        } else {
            certified
        };
        Ok(SingleSlopeAttempt::Certified(certified))
    }

    fn check_group<const WIDTH: usize, const CELLS: usize>(
        &self,
        level: usize,
        result: &DenseMatrix,
        scales: &[f64],
    ) -> Result<Check> {
        let rhs = result.ncols();
        let channels = self.effect.channels();
        debug_assert!(rhs <= WIDTH && channels * rhs <= CELLS);
        let mut check = Check::default();
        let group = &self.groups[level];
        let members = self.partition.level_index().members(level);
        let mut right_scales = [0.0_f64; WIDTH];
        for &row in members {
            for col in 0..rhs {
                right_scales[col] =
                    right_scales[col].max((self.roots[row] * result[(row, col)]).abs());
            }
        }
        let mut projections = [0.0_f64; CELLS];
        let mut moments = [0.0_f64; CELLS];
        let mut corrections = [0.0_f64; CELLS];
        let mut normalized = [0.0_f64; WIDTH];
        let mut raw = [0.0_f64; WIDTH];
        // Fuse columns while preserving every within-column reduction order.
        // The gate still uses normalized QR directions, independently of
        // the raw Gram projection and the raw-moment diagnostics.
        for (local, &row) in members.iter().enumerate() {
            for col in 0..rhs {
                let value = self.roots[row] * result[(row, col)];
                normalized[col] = if scales[col] > 0.0 {
                    value / scales[col]
                } else {
                    0.0
                };
                raw[col] = if right_scales[col] > 0.0 {
                    value / right_scales[col]
                } else {
                    0.0
                };
            }
            for channel in 0..group.rank {
                let direction = self.basis[group.start + channel * members.len() + local];
                let values = &mut projections[channel * rhs..(channel + 1) * rhs];
                for col in 0..rhs {
                    values[col] = direction.mul_add(normalized[col], values[col]);
                }
            }
            for channel in 0..channels {
                let left = if group.raw_scales[channel] > 0.0 {
                    self.roots[row] * basis_value(&self.effect, channel, row)
                        / group.raw_scales[channel]
                } else {
                    0.0
                };
                let sums = &mut moments[channel * rhs..(channel + 1) * rhs];
                let compensation = &mut corrections[channel * rhs..(channel + 1) * rhs];
                for col in 0..rhs {
                    let product = left * raw[col];
                    let updated = sums[col] + product;
                    compensation[col] += if sums[col].abs() >= product.abs() {
                        (sums[col] - updated) + product
                    } else {
                        (product - updated) + sums[col]
                    };
                    sums[col] = updated;
                }
            }
        }
        for col in 0..rhs {
            let mut square = 0.0_f64;
            for channel in 0..group.rank {
                let coefficient = projections[channel * rhs + col];
                square = coefficient.mul_add(coefficient, square);
            }
            let relative = square.sqrt();
            if !relative.is_finite() {
                return Err(invalid());
            }
            check.relative = check.relative.max(relative);
            check.squares = relative.mul_add(relative, check.squares);
            for channel in 0..channels {
                let slot = channel * rhs + col;
                let normalized = moments[slot] + corrections[slot];
                let left = group.raw_scales[channel];
                let right = right_scales[col];
                let moment = if normalized == 0.0 {
                    0.0
                } else {
                    (normalized * left.min(right)) * left.max(right)
                };
                let absolute = if moment.is_finite() {
                    moment.abs()
                } else {
                    f64::INFINITY
                };
                check.absolute = check.absolute.max(absolute);
            }
        }
        Ok(check)
    }

    fn norms(&self, values: &DenseMatrix) -> Result<Vec<f64>> {
        let mut result = filled(values.ncols(), 0.0)?;
        let width = if rayon::current_num_threads() == 1 {
            MAX_RHS
        } else if values.ncols() < 32 {
            1
        } else {
            8
        };
        result
            .par_chunks_mut(width)
            .enumerate()
            .try_for_each(|(block, output)| -> Result<()> {
                let mut scales = [0.0; MAX_RHS];
                let mut sums = [1.0; MAX_RHS];
                for row in 0..values.nrows() {
                    for col in 0..output.len() {
                        let value = self.roots[row] * values[(row, block * width + col)];
                        if !value.is_finite() {
                            return Err(invalid());
                        }
                        update_scaled_sum_of_squares(value, &mut scales[col], &mut sums[col]);
                    }
                }
                for col in 0..output.len() {
                    output[col] = if scales[col] == 0.0 {
                        0.0
                    } else {
                        scales[col] * sums[col].sqrt()
                    };
                    if !output[col].is_finite() {
                        return Err(invalid());
                    }
                }
                Ok(())
            })?;
        Ok(result)
    }
}

fn build_groups(
    partition: BorrowedInterceptEffect<'_>,
    effect: &CategoricalAbsorbedEffectView<'_>,
    weights: BorrowedMapWeights<'_>,
    roots: &[f64],
    first: usize,
    groups: &mut [Group],
    basis: &mut [f64],
    scratch: &mut [f64],
) -> Result<()> {
    if groups.len() > 256 && rayon::current_num_threads() > 1 {
        let middle = groups.len() / 2;
        let split = groups[middle].start - groups[0].start;
        let (gl, gr) = groups.split_at_mut(middle);
        let (bl, br) = basis.split_at_mut(split);
        let (sl, sr) = scratch.split_at_mut(split);
        let (l, r) = rayon::join(
            || build_groups(partition, effect, weights, roots, first, gl, bl, sl),
            || {
                build_groups(
                    partition,
                    effect,
                    weights,
                    roots,
                    first + middle,
                    gr,
                    br,
                    sr,
                )
            },
        );
        l?;
        return r;
    }
    let channels = effect.channels();
    let start = groups[0].start;
    for (index, group) in groups.iter_mut().enumerate() {
        let members = partition.level_index().members(first + index);
        let count = members.len();
        let offset = group.start - start;
        let columns = &mut scratch[offset..offset + count * channels];
        let output = &mut basis[offset..offset + count * channels];
        for &row in members {
            for left in 0..channels {
                for right in 0..channels {
                    let slot = left * channels + right;
                    group.gram[slot] = (weight(weights, row) * basis_value(effect, left, row))
                        .mul_add(basis_value(effect, right, row), group.gram[slot]);
                }
            }
        }
        let mut active = 0;
        for channel in 0..channels {
            let column = &mut columns[active * count..(active + 1) * count];
            let mut scale = 0.0_f64;
            for (local, &row) in members.iter().enumerate() {
                column[local] = roots[row] * basis_value(effect, channel, row);
                if !column[local].is_finite() {
                    return Err(invalid());
                }
                scale = scale.max(column[local].abs());
            }
            group.raw_scales[channel] = scale;
            if scale == 0.0 {
                continue;
            }
            for v in column.iter_mut() {
                *v /= scale;
            }
            let norm = euclidean_norm(column);
            if norm == 0.0 || !norm.is_finite() {
                return Err(invalid());
            }
            for v in column {
                *v /= norm;
            }
            active += 1;
        }
        let threshold = f64::EPSILON * count.max(active).max(1) as f64 * 8.0;
        while active > 0 && group.rank < count {
            // Iterator max_by chooses the last tied pivot in the reference QR.
            let (pivot, pivot_norm) = (0..active)
                .map(|i| (i, euclidean_norm(&columns[i * count..(i + 1) * count])))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .expect("nonempty slope columns");
            if pivot_norm <= threshold {
                break;
            }
            for local in 0..count {
                columns.swap(pivot * count + local, (active - 1) * count + local);
            }
            let candidate = &mut columns[(active - 1) * count..active * count];
            for _ in 0..2 {
                for prior in 0..group.rank {
                    subtract_projection(candidate, &output[prior * count..(prior + 1) * count]);
                }
            }
            let norm = euclidean_norm(candidate);
            active -= 1;
            if norm <= threshold {
                continue;
            }
            let direction = &mut output[group.rank * count..(group.rank + 1) * count];
            for (a, b) in direction.iter_mut().zip(candidate) {
                *a = *b / norm;
            }
            for column in columns[..active * count].chunks_mut(count) {
                subtract_projection(column, direction);
                subtract_projection(column, direction);
            }
            group.rank += 1;
        }
    }
    Ok(())
}
fn subtract_projection(values: &mut [f64], direction: &[f64]) {
    let coefficient = values
        .iter()
        .zip(direction)
        .fold(0.0_f64, |sum, (&v, &b)| v.mul_add(b, sum));
    for (v, &b) in values.iter_mut().zip(direction) {
        *v = (-coefficient).mul_add(b, *v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::certificate::{certify_residuals_with_reference_norms, weighted_column_norms};
    use crate::fe::{project_effect_in_place, EffectTerm, FeDesign, ProjectionWorkspace};

    #[test]
    fn cached_basis_matches_independent_reference_and_reuses_fresh_residual_checks() {
        for threads in [1, 4] {
            for intercept in [false, true] {
                for count in 1..=3 {
                    for scale in [1e-8, 1.0, 1e8] {
                        for rank_case in 0..4 {
                            let n = 2593;
                            let levels = 257;
                            let codes = (0..n).map(|i| (i % levels) as u32).collect();
                            let slopes: Vec<Vec<f64>> = (0..count)
                                .map(|channel| {
                                    (0..n)
                                        .map(|r| {
                                            if channel == count - 1 && rank_case > 0 {
                                                match rank_case {
                                                    1 => 0.0,
                                                    2 => {
                                                        scale * ((r + r / levels * 13) as f64).sin()
                                                    }
                                                    _ => {
                                                        scale
                                                            * (((r + r / levels * 13) as f64).sin()
                                                                + 1e-15 * (r as f64).cos())
                                                    }
                                                }
                                            } else {
                                                scale
                                                    * ((r * (channel + 1) + r / levels * 13) as f64)
                                                        .sin()
                                            }
                                        })
                                        .collect()
                                })
                                .collect();
                            let weights: Vec<f64> = (0..n)
                                .map(|r| {
                                    if threads == 1 {
                                        1.0
                                    } else {
                                        0.1 + (r % 7) as f64
                                    }
                                })
                                .collect();
                            let effect = EffectTerm::new(codes, levels, intercept, slopes).unwrap();
                            let design = FeDesign::new(n, vec![effect]).unwrap();
                            let effect = &design.effects()[0];
                            let partition = BorrowedInterceptEffect::new(
                                effect.codes(),
                                levels,
                                effect.level_index(),
                            )
                            .unwrap();
                            let view = CategoricalAbsorbedEffectView::new(
                                intercept,
                                effect.slopes().iter().map(Vec::as_slice).collect(),
                            )
                            .unwrap();
                            let pool = rayon::ThreadPoolBuilder::new()
                                .num_threads(threads)
                                .build()
                                .unwrap();
                            pool.install(|| {
                                let mut attempted_peak = SolverWorkspacePeak::ZERO;
                                let mut absorber = SingleSlopeAbsorber::build(
                                    partition,
                                    view,
                                    if threads == 1 {
                                        BorrowedMapWeights::Unit
                                    } else {
                                        BorrowedMapWeights::Provided(&weights)
                                    },
                                    3,
                                    None,
                                    &mut attempted_peak,
                                )
                                .unwrap()
                                .unwrap();
                                for pass in 0..2 {
                                    let input = DenseMatrix::from_columns(
                                        &(0..3)
                                            .map(|c| {
                                                (0..n)
                                                    .map(|r| {
                                                        ((r * (c + 3) + pass * 17) as f64).cos()
                                                    })
                                                    .collect()
                                            })
                                            .collect::<Vec<Vec<f64>>>(),
                                    )
                                    .unwrap();
                                    let refs = weighted_column_norms(&input, &weights).unwrap();
                                    let mut expected = input.clone();
                                    let mut workspace =
                                        ProjectionWorkspace::new(effect.channels(), 3).unwrap();
                                    project_effect_in_place(
                                        effect,
                                        &weights,
                                        &mut expected,
                                        &mut workspace,
                                    )
                                    .unwrap();
                                    let certificate = certify_residuals_with_reference_norms(
                                        &design, &weights, &expected, &refs, 1e-8,
                                    )
                                    .unwrap();
                                    match absorber
                                        .absorb(
                                            input.clone(),
                                            1e-8,
                                            None,
                                            if pass == 0 { None } else { Some(&refs) },
                                        )
                                        .unwrap()
                                    {
                                        SingleSlopeAttempt::Certified(mut result) => {
                                            assert!(certificate.passed);
                                            let norms = result.take_residual_norms();
                                            if threads == 1 {
                                                assert_eq!(norms, weighted_column_norms(&expected, &weights).unwrap());
                                            } else {
                                                assert!(norms.is_empty(), "raw-weight norms must not enter OLS's normalized frame");
                                            }
                                            let (actual, report) = result.into_parts();
                                            assert_eq!(actual, expected);
                                            assert_eq!(
                                                report.certificate.relative_infinity_norm,
                                                certificate.relative_infinity_norm
                                            );
                                            assert_eq!(
                                                report.certificate.absolute_infinity_norm,
                                                certificate.absolute_infinity_norm
                                            );
                                            assert!(
                                                (report.certificate.relative_two_norm
                                                    - certificate.relative_two_norm)
                                                    .abs()
                                                    < 1e-16
                                            );
                                        }
                                        SingleSlopeAttempt::Rejected { original, .. } => {
                                            assert!(!certificate.passed);
                                            assert_eq!(original, input);
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn budget_and_strict_certificate_rejections_preserve_original_input() {
        let effect = EffectTerm::new(
            vec![0; 7],
            1,
            true,
            vec![vec![0.3, 0.4, 0.9, -0.8, -0.2, 0.1, 0.6]],
        )
        .unwrap();
        let partition =
            BorrowedInterceptEffect::new(effect.codes(), 1, effect.level_index()).unwrap();
        let view = || {
            CategoricalAbsorbedEffectView::new(true, vec![effect.slopes()[0].as_slice()]).unwrap()
        };
        let tiny = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1,
        });
        let mut attempted_peak = SolverWorkspacePeak::ZERO;
        assert!(SingleSlopeAbsorber::build(
            partition,
            view(),
            BorrowedMapWeights::Unit,
            1,
            tiny,
            &mut attempted_peak
        )
        .unwrap()
        .is_none());
        let mut absorber = SingleSlopeAbsorber::build(
            partition,
            view(),
            BorrowedMapWeights::Unit,
            1,
            None,
            &mut attempted_peak,
        )
        .unwrap()
        .unwrap();
        let original =
            DenseMatrix::from_columns(&[vec![1.2, 2.3, 0.2, -0.1, -0.5, 0.7, 0.9]]).unwrap();
        for (tolerance, budget) in [(1e-8, tiny), (1e-30, None)] {
            let SingleSlopeAttempt::Rejected {
                original: returned, ..
            } = absorber
                .absorb(original.clone(), tolerance, budget, None)
                .unwrap()
            else {
                panic!("must reject")
            };
            assert_eq!(returned, original);
        }
    }
}
