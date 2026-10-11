use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::linalg::neumaier_add;
use crate::memory::{fallible_empty, fallible_filled};

/// Final weighted fixed-effect orthogonality certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct FeCertificate {
    /// Largest raw element of `D' W r` over all effects and right-hand sides.
    /// Saturates to infinity when a nonzero diagnostic is not representable.
    pub absolute_infinity_norm: f64,
    /// Largest basis-invariant local projection ratio over all FE levels and RHSs.
    pub relative_infinity_norm: f64,
    /// Root-mean-square of the basis-invariant local projection ratios.
    pub relative_two_norm: f64,
    pub passed: bool,
}

/// Weighted Euclidean norm of each matrix column.
pub(crate) fn weighted_column_norms(values: &DenseMatrix, weights: &[f64]) -> Result<Vec<f64>> {
    validate_common(values, weights)?;
    let mut scales = fallible_filled(
        values.ncols(),
        0.0_f64,
        "unable to reserve weighted certificate scales",
    )?;
    let mut scaled_square_sums = fallible_filled(
        values.ncols(),
        1.0_f64,
        "unable to reserve weighted certificate square sums",
    )?;
    for row in 0..values.nrows() {
        let square_root_weight = weights[row].sqrt();
        for column in 0..values.ncols() {
            let value = square_root_weight * values[(row, column)];
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "weighted certificate value is not finite",
                ));
            }
            update_scaled_sum_of_squares(
                value,
                &mut scales[column],
                &mut scaled_square_sums[column],
            );
        }
    }
    for column in 0..scales.len() {
        let scale = scales[column];
        let norm = if scale == 0.0 {
            0.0
        } else {
            scale * scaled_square_sums[column].sqrt()
        };
        if !norm.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "weighted certificate norm is not finite",
            ));
        }
        scales[column] = norm;
    }
    Ok(scales)
}

/// Recompute weighted fixed-effect span projections from returned residuals.
///
/// This convenience entry point uses the residual norm as the reference scale.
/// Estimation solvers should call the internal reference-norm entry point with
/// norms of the original right-hand sides so that an almost-exact fit does not
/// turn roundoff into an order-one relative violation.
pub fn certify_residuals(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    tolerance: f64,
) -> Result<FeCertificate> {
    let reference_norms = weighted_column_norms(residuals, weights)?;
    certify_residuals_with_reference_norms(design, weights, residuals, &reference_norms, tolerance)
}

/// Recompute weighted FE-span projections using pre-absorption RHS norms.
pub(crate) fn certify_residuals_with_reference_norms(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    reference_norms: &[f64],
    tolerance: f64,
) -> Result<FeCertificate> {
    certify_with_bases(
        design,
        weights,
        residuals,
        reference_norms,
        tolerance,
        None,
        false,
    )
}

fn certify_with_bases(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    reference_norms: &[f64],
    tolerance: f64,
    cached_bases: Option<&[Vec<Vec<f64>>]>,
    parallel: bool,
) -> Result<FeCertificate> {
    if residuals.nrows() != design.nobs() || weights.len() != design.nobs() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "certificate inputs have inconsistent observation counts",
        ));
    }
    validate_common(residuals, weights)?;
    if reference_norms.len() != residuals.ncols()
        || reference_norms
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "certificate reference norms are invalid",
        ));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "certificate tolerance must be finite and positive",
        ));
    }
    if design.effects().is_empty() || residuals.ncols() == 0 {
        return Ok(FeCertificate {
            absolute_infinity_norm: 0.0,
            relative_infinity_norm: 0.0,
            relative_two_norm: 0.0,
            passed: true,
        });
    }

    let residual_norms = weighted_column_norms(residuals, weights)?;
    let mut rhs_scales = fallible_empty(
        reference_norms.len(),
        "unable to reserve fixed-effect certificate RHS scales",
    )?;
    for (&reference, &residual) in reference_norms.iter().zip(&residual_norms) {
        rhs_scales.push(reference.max(residual));
    }

    if parallel {
        if let Some(bases) = cached_bases {
            let groups = design
                .effects()
                .iter()
                .map(|effect| effect.n_levels())
                .max()
                .unwrap_or(0);
            let cells = residuals
                .ncols()
                .checked_add(1)
                .and_then(|width| groups.checked_mul(width));
            if let Some(cells) = cells {
                // Parallelism is optional. A failed scratch reservation retains
                // the already admitted serial certificate and its check order.
                if let Ok(mut scratch) =
                    fallible_filled(cells, 0.0, "unable to reserve parallel certificate results")
                {
                    return certify_parallel_groups(
                        design,
                        weights,
                        residuals,
                        &rhs_scales,
                        tolerance,
                        bases,
                        &mut scratch,
                    );
                }
            }
        }
    }

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    let mut group = 0;
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let observations = effect.level_index().members(level);
            let fresh_basis;
            let orthonormal_basis = if let Some(bases) = cached_bases {
                &bases[group]
            } else {
                fresh_basis = level_basis(effect, weights, level)?;
                &fresh_basis
            };
            group += 1;
            // Keep the legacy matrix indices and numerical traversal order explicit.
            #[allow(clippy::needless_range_loop)]
            for rhs in 0..residuals.ncols() {
                let relative = relative_projection(
                    observations,
                    orthonormal_basis,
                    weights,
                    residuals,
                    rhs,
                    rhs_scales[rhs],
                );
                if !relative.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "fixed-effect projection certificate is not finite",
                    ));
                }
                relative_infinity_norm = relative_infinity_norm.max(relative);
                relative_square_sum = relative.mul_add(relative, relative_square_sum);
                relative_count += 1;
            }

            absolute_infinity_norm = absolute_infinity_norm.max(raw_moment_maximum(
                observations,
                effect,
                weights,
                residuals,
            ));
        }
    }

    let relative_two_norm = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn relative_projection(
    observations: &[usize],
    orthonormal_basis: &[Vec<f64>],
    weights: &[f64],
    residuals: &DenseMatrix,
    rhs: usize,
    denominator: f64,
) -> f64 {
    crate::kernels::dispatch!(
        crate::kernels::installed(),
        basis_projection(
            observations,
            orthonormal_basis,
            weights,
            residuals.as_slice(),
            residuals.ncols(),
            rhs,
            denominator
        )
    )
}

/// Independent groups write disjoint slots; the caller merges every RHS in
/// the original effect/level/RHS order, including the RMS FMA recurrence.
fn certify_parallel_groups(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    rhs_scales: &[f64],
    tolerance: f64,
    bases: &[Vec<Vec<f64>>],
    scratch: &mut [f64],
) -> Result<FeCertificate> {
    let rhs_count = residuals.ncols();
    let width = rhs_count + 1;
    let mut group = 0;
    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0;
    for effect in design.effects() {
        let results = &mut scratch[..effect.n_levels() * width];
        let plan = crate::parallel::StagePlan::current(
            design.nobs() as u128 * width as u128 * effect.channels() as u128,
            effect.n_levels(),
        );
        // Zero-sized lane contexts carry no allocation or retained bank.
        let mut lanes = vec![(); plan.lanes];
        crate::parallel::group_blocks(
            crate::parallel::GroupBuffers {
                first: results,
                first_stride: width,
                second: &mut [] as &mut [f64],
                second_stride: 0,
                scratch: &mut lanes,
            },
            effect.level_index().membership_offsets(),
            0,
            &|first, output, _, _| {
                for (local, output) in output.chunks_mut(width).enumerate() {
                    let level = first + local;
                    let observations = effect.level_index().members(level);
                    for rhs in 0..rhs_count {
                        output[rhs] = relative_projection(
                            observations,
                            &bases[group + level],
                            weights,
                            residuals,
                            rhs,
                            rhs_scales[rhs],
                        );
                    }
                    output[rhs_count] =
                        raw_moment_maximum(observations, effect, weights, residuals);
                }
                Ok(())
            },
        )?;
        for output in results.chunks_exact(width) {
            for &relative in &output[..rhs_count] {
                if !relative.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "fixed-effect projection certificate is not finite",
                    ));
                }
                relative_infinity_norm = relative_infinity_norm.max(relative);
                relative_square_sum = relative.mul_add(relative, relative_square_sum);
                relative_count += 1;
            }
            absolute_infinity_norm = absolute_infinity_norm.max(output[rhs_count]);
        }
        group += effect.n_levels();
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm: if relative_count == 0 {
            0.0
        } else {
            (relative_square_sum / relative_count as f64).sqrt()
        },
        passed: relative_infinity_norm <= tolerance,
    })
}

/// One absorption call's immutable design and weight frame. Only geometry is
/// retained; every check still reads the current residuals and reference norms.
/// A new context is required after changing weights, sample or generation.
pub(super) struct CertificateContext<'a> {
    design: &'a FeDesign,
    weights: &'a [f64],
    bases: Option<Vec<Vec<Vec<f64>>>>,
    parallel_rhs: usize,
}

impl<'a> CertificateContext<'a> {
    pub(super) fn new(design: &'a FeDesign, weights: &'a [f64]) -> Self {
        Self {
            design,
            weights,
            bases: None,
            parallel_rhs: 0,
        }
    }

    pub(super) fn inputs(&self) -> (&FeDesign, &[f64]) {
        (self.design, self.weights)
    }

    /// Upper bound for retained numeric banks and both levels of descriptors.
    /// Construction scratch is already in the generic certificate workspace;
    /// numeric columns move from QR into this bank without a second copy.
    pub(super) fn cache_bytes(&self) -> Result<u128> {
        let mut bytes = 0_u128;
        for effect in self.design.effects() {
            let cells = (self.design.nobs() as u128).checked_mul(effect.channels() as u128);
            let descriptors = (effect.n_levels() as u128).checked_mul(
                (std::mem::size_of::<Vec<Vec<f64>>>() as u128)
                    + (effect.channels() as u128) * std::mem::size_of::<Vec<f64>>() as u128,
            );
            bytes = cells
                .and_then(|n| n.checked_mul(8))
                .and_then(|n| n.checked_add(descriptors?))
                .and_then(|n| n.checked_add(bytes))
                .ok_or_else(|| {
                    FeregError::new(ErrorCode::Allocation, "certificate cache size overflows")
                })?;
        }
        Ok(bytes)
    }

    /// Caller must admit cache_bytes plus the existing certificate workspace.
    /// Allocation failure leaves this context uncached and its input untouched.
    pub(super) fn cache(&mut self) -> Result<()> {
        let groups = self
            .design
            .effects()
            .iter()
            .try_fold(0_usize, |n, effect| {
                n.checked_add(effect.n_levels()).ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Allocation,
                        "certificate cache group count overflows",
                    )
                })
            })?;
        let mut bases = fallible_empty(groups, "unable to reserve certificate cache groups")?;
        for effect in self.design.effects() {
            for level in 0..effect.n_levels() {
                bases.push(level_basis(effect, self.weights, level)?);
            }
        }
        self.bases = Some(bases);
        Ok(())
    }

    /// Additional scratch for ordered results from one effect at a time.
    /// Only an already installed team may evaluate immutable native inputs.
    pub(super) fn parallel_workspace_bytes(&self, rhs: usize) -> Result<u128> {
        if self.bases.is_none()
            || rhs == 0
            || rayon::current_thread_index().is_none()
            || !self.design.effects().iter().any(|effect| {
                crate::parallel::StagePlan::current(
                    self.design.nobs() as u128 * (rhs as u128 + 1) * effect.channels() as u128,
                    effect.n_levels(),
                )
                .lanes
                    > 1
            })
        {
            return Ok(0);
        }
        let levels = self
            .design
            .effects()
            .iter()
            .map(|e| e.n_levels())
            .max()
            .unwrap_or(0);
        (rhs as u128)
            .checked_add(1)
            .and_then(|r| r.checked_mul(levels as u128))
            .and_then(|cells| cells.checked_mul(8))
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "parallel certificate workspace overflows",
                )
            })
    }

    /// Caller admits the workspace before enabling any parallel certificate.
    pub(super) fn set_parallel_rhs(&mut self, rhs: usize) {
        self.parallel_rhs = rhs;
    }

    pub(super) fn certify(
        &self,
        residuals: &DenseMatrix,
        reference_norms: &[f64],
        tolerance: f64,
    ) -> Result<FeCertificate> {
        certify_with_bases(
            self.design,
            self.weights,
            residuals,
            reference_norms,
            tolerance,
            self.bases.as_deref(),
            residuals.ncols() > 0 && residuals.ncols() <= self.parallel_rhs,
        )
    }
}

fn level_basis(
    effect: &crate::fe::EffectTerm,
    weights: &[f64],
    level: usize,
) -> Result<Vec<Vec<f64>>> {
    let channels = effect.channels();
    let observations = effect.level_index().members(level);
    let mut normalized_columns = fallible_empty(
        channels,
        "unable to reserve fixed-effect certificate normalized columns",
    )?;
    for channel in 0..channels {
        let mut column = fallible_empty(
            observations.len(),
            "unable to reserve fixed-effect certificate level column",
        )?;
        let mut column_scale = 0.0_f64;
        for &observation in observations {
            let value = weights[observation].sqrt() * effect.basis_value(channel, observation);
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "weighted fixed-effect basis is not finite",
                ));
            }
            column_scale = column_scale.max(value.abs());
            column.push(value);
        }
        if column_scale == 0.0 {
            continue;
        }
        for value in &mut column {
            *value /= column_scale;
        }
        let norm = euclidean_norm(&column);
        if norm == 0.0 || !norm.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "fixed-effect basis normalization failed",
            ));
        }
        for value in &mut column {
            *value /= norm;
        }
        normalized_columns.push(column);
    }
    pivoted_reorthogonalized_qr(normalized_columns)
}

/// Column-pivoted, twice-reorthogonalized modified Gram-Schmidt.
///
/// Input columns are individually normalized before this routine. That makes
/// rank selection insensitive to FE channel units. The cutoff is tied only to
/// machine precision and block dimensions, never to the user's convergence
/// tolerance.
fn pivoted_reorthogonalized_qr(mut columns: Vec<Vec<f64>>) -> Result<Vec<Vec<f64>>> {
    let nrows = columns.first().map_or(0, Vec::len);
    let ncols = columns.len();
    let rank_threshold = f64::EPSILON * nrows.max(ncols).max(1) as f64 * 8.0;
    let mut basis = fallible_empty(
        nrows.min(ncols),
        "unable to reserve fixed-effect certificate orthonormal basis",
    )?;

    while !columns.is_empty() && basis.len() < nrows {
        let (pivot, pivot_norm) = columns
            .iter()
            .enumerate()
            .map(|(index, column)| (index, euclidean_norm(column)))
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .expect("nonempty QR column set");
        if pivot_norm <= rank_threshold {
            break;
        }

        let mut candidate = columns.swap_remove(pivot);
        reorthogonalize(&mut candidate, &basis);
        let norm = euclidean_norm(&candidate);
        if norm <= rank_threshold {
            continue;
        }
        for value in &mut candidate {
            *value /= norm;
        }

        for column in &mut columns {
            subtract_projection(column, &candidate);
            subtract_projection(column, &candidate);
        }
        basis.push(candidate);
    }
    Ok(basis)
}

fn reorthogonalize(candidate: &mut [f64], basis: &[Vec<f64>]) {
    for _ in 0..2 {
        for direction in basis {
            subtract_projection(candidate, direction);
        }
    }
}

fn subtract_projection(values: &mut [f64], direction: &[f64]) {
    crate::kernels::dispatch!(
        crate::kernels::installed(),
        subtract_projection(values, direction)
    );
}

pub(super) fn euclidean_norm(values: &[f64]) -> f64 {
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for &value in values {
        update_scaled_sum_of_squares(value, &mut scale, &mut scaled_square_sum);
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    }
}

pub(super) fn update_scaled_sum_of_squares(
    value: f64,
    scale: &mut f64,
    scaled_square_sum: &mut f64,
) {
    let absolute = value.abs();
    if absolute == 0.0 {
        return;
    }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *scaled_square_sum = 1.0 + *scaled_square_sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *scaled_square_sum += ratio * ratio;
    }
}

/// Accumulate a diagnostic dot product after separately scaling both vectors.
///
/// The certificate gate never consumes this value. A true nonzero result that
/// exceeds the `f64` range saturates to infinity instead of turning a valid
/// basis-invariant projection certificate into an error.
#[cfg(test)]
fn scaled_raw_moment(
    observations: &[usize],
    left: impl Fn(usize) -> f64,
    right: impl Fn(usize) -> f64,
) -> f64 {
    let mut left_scale = 0.0_f64;
    let mut right_scale = 0.0_f64;
    for &observation in observations {
        left_scale = left_scale.max(left(observation).abs());
        right_scale = right_scale.max(right(observation).abs());
    }
    if left_scale == 0.0 || right_scale == 0.0 {
        return 0.0;
    }

    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for &observation in observations {
        let product = (left(observation) / left_scale) * (right(observation) / right_scale);
        neumaier_add(product, &mut sum, &mut correction);
    }
    rescale_raw_moment(sum + correction, left_scale, right_scale)
}

fn rescale_raw_moment(normalized: f64, left_scale: f64, right_scale: f64) -> f64 {
    if normalized == 0.0 {
        return 0.0;
    }

    let smaller_scale = left_scale.min(right_scale);
    let larger_scale = left_scale.max(right_scale);
    let moment = (normalized * smaller_scale) * larger_scale;
    if moment.is_finite() {
        moment
    } else {
        normalized.signum() * f64::INFINITY
    }
}

/// Fresh raw diagnostics, with independent RHS entries in bounded row tiles.
/// Each entry retains its observation order, two divisions, compensated sum
/// and final scale order. Geometry and residuals are never cached across calls.
fn raw_moment_maximum(
    observations: &[usize],
    effect: &crate::fe::EffectTerm,
    weights: &[f64],
    residuals: &DenseMatrix,
) -> f64 {
    const TILE: usize = 32;
    let rhs_count = residuals.ncols();
    let mut maximum = 0.0_f64;
    for first in (0..rhs_count).step_by(TILE) {
        let width = (rhs_count - first).min(TILE);
        let mut right_scales = [0.0_f64; TILE];
        for &observation in observations {
            let root = weights[observation].sqrt();
            let offset = observation * rhs_count + first;
            let row = &residuals.as_slice()[offset..offset + width];
            for (scale, &value) in right_scales[..width].iter_mut().zip(row) {
                *scale = scale.max((root * value).abs());
            }
        }
        for channel in 0..effect.channels() {
            let mut left_scale = 0.0_f64;
            for &observation in observations {
                let left = weights[observation].sqrt() * effect.basis_value(channel, observation);
                left_scale = left_scale.max(left.abs());
            }
            if left_scale == 0.0 {
                continue;
            }
            let mut sums = [0.0_f64; TILE];
            let mut corrections = [0.0_f64; TILE];
            for &observation in observations {
                let root = weights[observation].sqrt();
                let left = root * effect.basis_value(channel, observation) / left_scale;
                let offset = observation * rhs_count + first;
                let row = &residuals.as_slice()[offset..offset + width];
                for rhs in 0..width {
                    if right_scales[rhs] == 0.0 {
                        continue;
                    }
                    let product = left * ((root * row[rhs]) / right_scales[rhs]);
                    neumaier_add(product, &mut sums[rhs], &mut corrections[rhs]);
                }
            }
            for rhs in 0..width {
                let moment =
                    rescale_raw_moment(sums[rhs] + corrections[rhs], left_scale, right_scales[rhs]);
                maximum = maximum.max(moment.abs());
            }
        }
    }
    maximum
}

fn validate_common(values: &DenseMatrix, weights: &[f64]) -> Result<()> {
    if values.nrows() != weights.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "weighted matrix and weight lengths differ",
        ));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "certificate weights must be finite and positive",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "certificate values must be finite",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn parallel_groups_match_serial_bits_and_independent_weighted_two_column_projection() {
        let n = 84;
        let z: Vec<_> = (0..n).map(|r| (r / 7) as f64 - 5.5).collect();
        let design = FeDesign::new(
            n,
            vec![EffectTerm::new(
                (0..n).map(|r| (r % 7) as u32).collect(),
                8,
                true,
                vec![z.clone(), z.clone(), vec![0.0; n]],
            )
            .unwrap()],
        )
        .unwrap();
        for weights in [vec![1.0; n], (0..n).map(|r| 0.5 + (r % 5) as f64).collect()] {
            let mut values = DenseMatrix::zeros(n, 7).unwrap();
            for r in 0..n {
                for c in 0..7 {
                    values[(r, c)] = ((r * 13 + c * 7) as f64).sin();
                }
            }
            let reference = weighted_column_norms(&values, &weights).unwrap();
            let serial = certify_residuals_with_reference_norms(
                &design, &weights, &values, &reference, 1e-8,
            )
            .unwrap();
            let mut expected_inf = 0.0_f64;
            let mut expected_square_sum = 0.0;
            for level in 0..8 {
                for rhs in 0..7 {
                    let mut sw = 0.0;
                    let mut sz = 0.0;
                    let mut szz = 0.0;
                    let mut sr = 0.0;
                    let mut szr = 0.0;
                    for r in (level..n).step_by(7).filter(|_| level < 7) {
                        sw += weights[r];
                        sz += weights[r] * z[r];
                        szz += weights[r] * z[r] * z[r];
                        sr += weights[r] * values[(r, rhs)];
                        szr += weights[r] * z[r] * values[(r, rhs)];
                    }
                    let ratio = if sw == 0.0 {
                        0.0
                    } else {
                        ((szz * sr * sr - 2.0 * sz * sr * szr + sw * szr * szr)
                            / (sw * szz - sz * sz))
                            .sqrt()
                            / reference[rhs]
                    };
                    expected_inf = expected_inf.max(ratio);
                    expected_square_sum += ratio * ratio;
                }
            }
            assert!((serial.relative_infinity_norm - expected_inf).abs() < 1e-14);
            assert!((serial.relative_two_norm - (expected_square_sum / 56.0).sqrt()).abs() < 1e-14);
            for threads in [1, 2, 4, 8] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let mut context = CertificateContext::new(&design, &weights);
                        context.cache().unwrap();
                        // Exercise the kernel below its production scheduling floor.
                        context.set_parallel_rhs(7);
                        for _ in 0..3 {
                            assert_eq!(context.certify(&values, &reference, 1e-8).unwrap(), serial);
                        }
                        context = CertificateContext::new(context.design, context.weights);
                        assert_eq!(context.parallel_rhs, 0);
                    });
            }
        }
    }

    #[test]
    fn scoped_geometry_is_exact_across_residuals_weights_rank_and_units() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        for scale in [1e-100, 1.0, 1e100] {
            let n = 72;
            let z: Vec<_> = (0..n).map(|r| scale * ((r % 7) as f64 - 3.0)).collect();
            let design = FeDesign::new(
                n,
                vec![
                    EffectTerm::new(
                        (0..n).map(|r| (r % 5) as u32).collect(),
                        6,
                        true,
                        vec![z.clone(), z, vec![0.0; n]],
                    )
                    .unwrap(),
                    EffectTerm::new(
                        (0..n).map(|r| (r % 3) as u32).collect(),
                        3,
                        false,
                        vec![vec![scale; n]],
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
            for weights in [
                vec![1.0; n],
                (0..n).map(|r| 0.25 + (r % 11) as f64).collect(),
            ] {
                let mut context = CertificateContext::new(&design, &weights);
                context.cache().unwrap();
                let bases = context.bases.as_ref().unwrap();
                let actual = bases.capacity() * std::mem::size_of::<Vec<Vec<f64>>>()
                    + bases
                        .iter()
                        .map(|group| {
                            group.capacity() * std::mem::size_of::<Vec<f64>>()
                                + group.iter().map(|v| v.capacity() * 8).sum::<usize>()
                        })
                        .sum::<usize>();
                assert!(actual as u128 <= context.cache_bytes().unwrap());
                for rhs in [0, 1, 7, 33] {
                    context.set_parallel_rhs(rhs);
                    let mut values = DenseMatrix::zeros(n, rhs).unwrap();
                    for state in [0.0, 1.0, 1e-14, -3.0] {
                        for row in 0..n {
                            for col in 0..rhs {
                                values[(row, col)] =
                                    state * (((row * 13 + col * 7) % 37) as f64 - 18.0);
                            }
                        }
                        let norms = vec![100.0; rhs];
                        let expected = certify_residuals_with_reference_norms(
                            &design, &weights, &values, &norms, 1e-8,
                        )
                        .unwrap();
                        assert_eq!(
                            pool.install(|| context.certify(&values, &norms, 1e-8))
                                .unwrap(),
                            expected
                        );
                    }
                }
                let invalid = DenseMatrix::from_columns(&[vec![f64::NAN; n]]).unwrap();
                assert!(context.certify(&invalid, &[1.0], 1e-8).is_err());
                context = CertificateContext::new(context.design, context.weights);
                assert!(context.bases.is_none());
            }
        }
    }

    #[test]
    fn raw_moment_tiles_match_scalar_entries_across_widths_and_scales() {
        let observations = [6, 0, 8, 3, 2, 5, 1, 7, 4];
        let weights: [f64; 9] = [0.25, 1.0, 4.0, 0.5, 2.0, 16.0, 0.0625, 8.0, 1.0];
        for width in [1, 2, 7, 31, 32, 33, 63, 64, 65, 129] {
            for scale in [0.0, 1e-200, 1.0, 1e200] {
                let slope: Vec<_> = (0..9).map(|i| scale * ((i as f64) - 4.0)).collect();
                let effect = EffectTerm::new(vec![0; 9], 1, false, vec![slope]).unwrap();
                for selected in 0..width {
                    let mut residuals = DenseMatrix::zeros(9, width).unwrap();
                    for row in 0..9 {
                        residuals[(row, selected)] = scale * ((row * 7 % 11) as f64 - 5.0);
                    }
                    // Isolate each entry, so a larger neighboring moment cannot
                    // hide a mistake in a full or partial tile.
                    let expected = scaled_raw_moment(
                        &observations,
                        |row| weights[row].sqrt() * effect.basis_value(0, row),
                        |row| weights[row].sqrt() * residuals[(row, selected)],
                    )
                    .abs();
                    let actual = raw_moment_maximum(&observations, &effect, &weights, &residuals);
                    assert_eq!(
                        actual.to_bits(),
                        expected.to_bits(),
                        "width={width}, rhs={selected}, scale={scale}"
                    );
                }
            }
        }
    }

    #[test]
    fn raw_moment_tiles_preserve_cancellation_and_multiple_channels() {
        let weights: Vec<f64> = (0..73).map(|i| 0.1 + (i % 11) as f64).collect();
        let effect = EffectTerm::new(
            vec![0; 73],
            1,
            true,
            vec![(0..73).map(|i| (i as f64).sin()).collect(), vec![0.0; 73]],
        )
        .unwrap();
        let observations: Vec<_> = (0..73).rev().collect();
        let mut values = DenseMatrix::zeros(73, 67).unwrap();
        for row in 0..73 {
            for rhs in 0..67 {
                values[(row, rhs)] = if rhs == 0 {
                    -0.0
                } else {
                    (((row * 23 + rhs * 17) % 137) as f64 - 68.0) * 1e70
                };
            }
        }
        let mut expected = 0.0_f64;
        for channel in 0..effect.channels() {
            for rhs in 0..values.ncols() {
                expected = expected.max(
                    scaled_raw_moment(
                        &observations,
                        |row| weights[row].sqrt() * effect.basis_value(channel, row),
                        |row| weights[row].sqrt() * values[(row, rhs)],
                    )
                    .abs(),
                );
            }
        }
        assert_eq!(
            raw_moment_maximum(&observations, &effect, &weights, &values).to_bits(),
            expected.to_bits()
        );
        assert_eq!(raw_moment_maximum(&[], &effect, &weights, &values), 0.0);
    }

    #[test]
    fn certificate_capacity_overflow_is_an_allocation_error() {
        let error = fallible_empty::<f64>(usize::MAX, "injected certificate allocation")
            .expect_err("capacity overflow");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
    }

    #[test]
    fn certificate_is_invariant_to_common_weight_scaling() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let residuals =
            DenseMatrix::from_columns(&[vec![-1.0, 1.0, -2.0, 2.0]]).expect("residuals");
        let first = certify_residuals(&design, &[1.0; 4], &residuals, 1e-12).expect("certificate");
        let second =
            certify_residuals(&design, &[100.0; 4], &residuals, 1e-12).expect("certificate");
        assert!((first.relative_infinity_norm - second.relative_infinity_norm).abs() < 1e-14);
        assert!(first.passed);
        assert!(second.passed);
    }

    #[test]
    fn reference_scale_certifies_near_exact_fit_roundoff() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let residuals =
            DenseMatrix::from_columns(&[vec![1e-15, -1e-15, 2e-15, -2e-15]]).expect("residuals");
        let certificate =
            certify_residuals_with_reference_norms(&design, &[1.0; 4], &residuals, &[10.0], 1e-12)
                .expect("certificate");
        assert!(certificate.passed);
    }

    #[test]
    fn certificate_rejects_nonorthogonal_residuals() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let residuals =
            DenseMatrix::from_columns(&[vec![1.0, 1.0, -1.0, -1.0]]).expect("residuals");
        let certificate =
            certify_residuals(&design, &[1.0; 4], &residuals, 1e-8).expect("certificate");
        assert!(!certificate.passed);
    }
}
