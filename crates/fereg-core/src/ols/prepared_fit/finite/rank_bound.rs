//! Exceptional-path rank certificate. This verifies the existing QR basis.
//!
//! Let H be the exact Gram of the stored, standardized projected columns, G
//! its computed Gram, and C the computed inverse. If ||I-CG||_inf +
//! ||C||_inf ||G-H_true||_inf < 1, H_true is invertible and its smallest
//! eigenvalue is at least (1-total)/||C||_inf. H_true is a Gram, hence PSD.
//! The repaired-column error has Frobenius bound Delta; consequently its
//! Gram perturbation has spectral norm <= 2||X||_F Delta + Delta^2, and
//! infinity norm <= sqrt(K) times that quantity. Delta covers every candidate
//! column. Dropped directions additionally require a direct primal residual
//! bound; a cancellation-prone Gram difference never certifies omission.
use super::*;

fn unresolved() -> crate::error::FeregError {
    invalid_data("OLS projection repair left finite numerical rank unresolved")
}

// One representable step outward encloses a correctly rounded elementary
// operation, including gradual underflow. Nonfinite results fail at the
// certificate boundary. No ambient rounding-mode change or unsafe code.
fn up(value: f64) -> f64 {
    if !value.is_finite() {
        value
    } else if value == 0.0 {
        f64::from_bits(1)
    } else if value > 0.0 {
        f64::from_bits(value.to_bits() + 1)
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}
fn down(value: f64) -> f64 {
    -up(-value)
}
fn add(a: f64, b: f64) -> f64 {
    up(a + b)
}
fn mul(a: f64, b: f64) -> f64 {
    up(a * b)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_arithmetic<R: FitReservation>(
    storage: &ProjectedFiniteRows<'_>,
    weights: NormalizedWeights<'_>,
    weight_scale: PreparedOlsBreadWeightScale,
    selected: &[usize],
    scales: &[f64],
    row: &mut Vec<f64>,
    reservation: &mut R,
) -> Result<()> {
    if weight_scale.solver_weight_divisor != weights.raw_scale {
        return Err(unresolved());
    }
    let rows = match storage {
        ProjectedFiniteRows::Centered(_) => return Err(unresolved()),
        ProjectedFiniteRows::Dense(values) => values.nrows(),
        ProjectedFiniteRows::Scratch { scratch, .. } => {
            resize(row, scratch.columns(), 0.0, reservation)?;
            scratch.rows()
        }
    };
    let mut reader = match storage {
        ProjectedFiniteRows::Scratch { scratch, .. } => Some(scratch.shared_reader()),
        _ => None,
    };
    for observation in 0..rows {
        if observation % 256 == 0 {
            crate::interrupt::checkpoint()?;
        }
        let weight = weights.value(observation)?;
        if !weight.is_normal() || weight <= 0.0 {
            return Err(unresolved());
        }
        if let (ProjectedFiniteRows::Scratch { scratch, .. }, Some(reader)) =
            (storage, reader.as_mut())
        {
            reader.read_linear(observation * scratch.columns(), row)?;
        }
        for (position, &column) in selected.iter().enumerate() {
            let value = match storage {
                ProjectedFiniteRows::Dense(values) => values[(observation, column + 1)],
                ProjectedFiniteRows::Scratch { .. } => row[column],
                ProjectedFiniteRows::Centered(_) => unreachable!(),
            };
            validate_entry(value, scales[position], weight)?;
        }
    }
    Ok(())
}

fn validate_entry(value: f64, scale: f64, weight: f64) -> Result<()> {
    if !value.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return Err(unresolved());
    }
    if value != 0.0 {
        let standardized = value / scale;
        let weighted = weight * standardized;
        // This prevents loss before a later multiplication can magnify it.
        // Final product/addition underflow is covered by an absolute floor.
        if !standardized.is_normal() || !weighted.is_normal() {
            return Err(unresolved());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify<R: FitReservation>(
    storage: &ProjectedFiniteRows<'_>,
    weights: NormalizedWeights<'_>,
    workspace: &mut FiniteWorkspace,
    rank: usize,
    initial_norm: f64,
    threshold: f64,
    projection_error: f64,
    reservation: &mut R,
) -> Result<()> {
    if rank != workspace.kept.len() {
        return Err(unresolved());
    }
    // Factorization is finished. Its permutation/solve/row banks are dead
    // scratch until a possible fresh factorization and can be reused here.
    resize(&mut workspace.permutation, rank, 0, reservation)?;
    for (position, &id) in workspace.kept.iter().enumerate() {
        workspace.permutation[position] = workspace
            .candidate_ids
            .binary_search(&id)
            .map_err(|_| unresolved())?;
    }
    let rows = match storage {
        ProjectedFiniteRows::Dense(values) => values.nrows(),
        ProjectedFiniteRows::Scratch { scratch, .. } => scratch.rows(),
        ProjectedFiniteRows::Centered(_) => return Err(unresolved()),
    };
    let count = workspace.candidate_ids.len();
    let drop_cutoff = certify_basis(
        rows,
        &workspace.gram,
        &workspace.bread,
        count,
        &workspace.permutation,
        initial_norm,
        threshold,
        projection_error,
    )?;
    if rank == count {
        return Ok(());
    }
    resize(&mut workspace.solve, rank, 0.0, reservation)?;
    for dropped in 0..count {
        if workspace.permutation.binary_search(&dropped).is_ok() {
            continue;
        }
        // Any finite stored alpha is an admissible primal witness. Errors in
        // computing C*G affect its usefulness, never the validity of the scan.
        for i in 0..rank {
            crate::interrupt::checkpoint()?;
            let mut value = 0.0;
            let mut correction = 0.0;
            for j in 0..rank {
                compensated_add(
                    workspace.bread[i * rank + j]
                        * workspace.gram[workspace.permutation[j] * count + dropped],
                    &mut value,
                    &mut correction,
                )?;
            }
            workspace.solve[i] = value + correction;
        }
        let primal = dropped_norm(
            storage,
            weights,
            &workspace.candidate_ids,
            &workspace.candidate_scales,
            &workspace.permutation,
            dropped,
            &workspace.solve,
            &mut workspace.row,
        )?;
        check_dropped(primal, &workspace.solve, projection_error, drop_cutoff)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn certify_basis(
    rows: usize,
    gram: &[f64],
    inverse: &[f64],
    count: usize,
    positions: &[usize],
    initial_norm: f64,
    threshold: f64,
    projection_error: f64,
) -> Result<f64> {
    let rank = positions.len();
    if count == 0
        || rank == 0
        || rank > count
        || positions.iter().any(|&p| p >= count)
        || positions.windows(2).any(|p| p[0] >= p[1])
        || gram.len() != count.checked_mul(count).ok_or_else(unresolved)?
        || inverse.len() != rank.checked_mul(rank).ok_or_else(unresolved)?
        || !initial_norm.is_finite()
        || initial_norm < 0.0
        || !threshold.is_finite()
        || threshold <= 0.0
        || !projection_error.is_finite()
        || projection_error < 0.0
        || !gram.iter().chain(inverse).all(|x| x.is_finite())
    {
        return Err(unresolved());
    }
    // Dense/scratch kernels standardize and weight each row, accumulate each
    // product in row order, and optionally merge compensated partials. 16N+128
    // exceeds every elementary-operation path through those kernels. The
    // pre-scan excludes underflow in division and weight*column. Remaining
    // product/sum underflow is bounded additively, conservatively using the
    // minimum normal value (larger than the IEEE absolute rounding floor).
    let operations = add(mul(16.0, rows as f64), 128.0);
    let round = mul(operations, f64::EPSILON);
    if !round.is_finite() || round >= 0.25 {
        return Err(unresolved());
    }
    let gamma = up(round / down(1.0 - round));
    let floor = mul(operations, f64::MIN_POSITIVE);
    let mut max_diagonal = 0.0_f64;
    let mut frobenius_square = 0.0_f64;
    let mut all_frobenius_square = 0.0_f64;
    let mut largest_column_lower = 0.0_f64;
    for j in 0..count {
        let diagonal = gram[j * count + j];
        if diagonal <= 0.0 {
            return Err(unresolved());
        }
        let bound = up(add(diagonal, floor) / down(1.0 - gamma));
        max_diagonal = max_diagonal.max(bound);
        all_frobenius_square = add(all_frobenius_square, bound);
        if positions.binary_search(&j).is_ok() {
            frobenius_square = add(frobenius_square, bound);
        }
        let square_lower = down(down(diagonal - floor) / add(1.0, gamma)).max(0.0);
        largest_column_lower = largest_column_lower.max(down(square_lower.sqrt()));
    }
    // Cauchy-Schwarz bounds each absolute product sum by max diagonal.
    let gram_error = mul(rank as f64, add(mul(gamma, max_diagonal), floor));
    let frobenius = up(frobenius_square.sqrt());
    let perturbation = mul(
        up((rank as f64).sqrt()),
        add(
            mul(mul(2.0, frobenius), projection_error),
            mul(projection_error, projection_error),
        ),
    );
    let delta = add(gram_error, perturbation);
    let (eta, inverse_norm) = inverse_residual(gram, inverse, count, positions)?;
    let total = add(eta, mul(inverse_norm, delta));
    if !total.is_finite() || total >= 1.0 || inverse_norm <= 0.0 {
        return Err(unresolved());
    }
    let minimum_eigenvalue = down(down(1.0 - total) / inverse_norm);
    let minimum_singular = down(minimum_eigenvalue.sqrt());
    // The Frobenius bound also bounds the true largest column norm. Taking
    // max with the report's scale is conservative and avoids trusting its
    // norm-rounding error. Successful Cholesky reports initial_norm=0 because
    // it has no QR scale; the independent positive Frobenius bound owns that
    // case. This strengthens, never relaxes, the existing gate.
    let cutoff = mul(
        threshold,
        add(
            initial_norm.max(up(all_frobenius_square.sqrt())),
            projection_error,
        ),
    );
    if !minimum_singular.is_finite() || !cutoff.is_finite() || minimum_singular <= cutoff {
        return Err(unresolved());
    }
    // Omission needs the opposite scale direction: an upper norm scale would
    // silently loosen the existing per-column QR threshold. Each original
    // column norm is at least its stored norm minus the total error bound.
    let drop_cutoff = down(threshold * down(largest_column_lower - projection_error));
    if !drop_cutoff.is_finite() || drop_cutoff <= 0.0 {
        return Err(unresolved());
    }
    Ok(drop_cutoff)
}

// Absolute inverse residual, with outward elementary-operation enclosures.
// The existing relative inverse diagnostic is not a substitute for this norm.
fn inverse_residual(
    gram: &[f64],
    inverse: &[f64],
    count: usize,
    positions: &[usize],
) -> Result<(f64, f64)> {
    let rank = positions.len();
    let mut eta = 0.0_f64;
    let mut inverse_norm = 0.0_f64;
    for i in 0..rank {
        crate::interrupt::checkpoint()?;
        let mut residual_row = 0.0;
        let mut inverse_row = 0.0;
        for j in 0..rank {
            inverse_row = add(inverse_row, inverse[i * rank + j].abs());
            let mut lower = 0.0;
            let mut upper = 0.0;
            for k in 0..rank {
                let product = inverse[i * rank + k] * gram[positions[k] * count + positions[j]];
                lower = down(lower + down(product));
                upper = up(upper + up(product));
                if !product.is_finite() || !lower.is_finite() || !upper.is_finite() {
                    return Err(unresolved());
                }
            }
            let identity = f64::from(i == j);
            let residual = up((identity - lower).abs()).max(up((identity - upper).abs()));
            residual_row = add(residual_row, residual);
        }
        eta = eta.max(residual_row);
        inverse_norm = inverse_norm.max(inverse_row);
    }
    if eta.is_finite() && inverse_norm.is_finite() {
        Ok((eta, inverse_norm))
    } else {
        Err(unresolved())
    }
}

/// Upper Euclidean norm without overflowing intermediate squares. Every
/// division/product/sum rounds outward, including gradual underflow.
#[derive(Default)]
struct NormUpper {
    scale: f64,
    square_sum: f64,
}
impl NormUpper {
    fn push(&mut self, value: f64) -> Result<()> {
        let magnitude = value.abs();
        if !magnitude.is_finite() {
            return Err(unresolved());
        }
        if magnitude == 0.0 {
            return Ok(());
        }
        if magnitude > self.scale {
            let ratio = up(self.scale / magnitude);
            self.square_sum = add(1.0, mul(self.square_sum, mul(ratio, ratio)));
            self.scale = magnitude;
        } else {
            let ratio = up(magnitude / self.scale);
            self.square_sum = add(self.square_sum, mul(ratio, ratio));
        }
        Ok(())
    }
    fn finish(self) -> Result<f64> {
        if self.scale == 0.0 {
            return Ok(0.0);
        }
        let result = mul(self.scale, up(self.square_sum.sqrt()));
        if result.is_finite() {
            Ok(result)
        } else {
            Err(unresolved())
        }
    }
}

fn check_dropped(primal: f64, alpha: &[f64], error: f64, cutoff: f64) -> Result<()> {
    let mut norm = NormUpper::default();
    for &value in alpha {
        norm.push(value)?;
    }
    // ||E_j-E_kept alpha|| <= Delta sqrt(1+||alpha||^2)
    // <= Delta (1+||alpha||). All quantities here are upper bounds.
    let upper = add(primal, mul(error, add(1.0, norm.finish()?)));
    if !upper.is_finite() || upper > cutoff {
        Err(unresolved())
    } else {
        Ok(())
    }
}

/// Bound an exact standardized row residual for the stored witness alpha.
/// Even a precisely duplicated column gets a nonzero rounding allowance;
/// accepting it therefore still requires room below the rank threshold.
fn residual_bound(
    mut value: impl FnMut(usize) -> f64,
    scales: &[f64],
    kept: &[usize],
    dropped: usize,
    alpha: &[f64],
) -> Result<f64> {
    let mut lower = 0.0;
    let mut upper = 0.0;
    for (&position, &coefficient) in kept.iter().zip(alpha) {
        let standardized = value(position) / scales[position];
        if !standardized.is_finite() || !coefficient.is_finite() {
            return Err(unresolved());
        }
        let (left, right) = if coefficient >= 0.0 {
            (down(standardized), up(standardized))
        } else {
            (up(standardized), down(standardized))
        };
        lower = down(lower + down(coefficient * left));
        upper = up(upper + up(coefficient * right));
        if !lower.is_finite() || !upper.is_finite() {
            return Err(unresolved());
        }
    }
    let target = value(dropped) / scales[dropped];
    if !target.is_finite() {
        return Err(unresolved());
    }
    let result = down(down(target) - upper)
        .abs()
        .max(up(up(target) - lower).abs());
    if result.is_finite() {
        Ok(result)
    } else {
        Err(unresolved())
    }
}

#[allow(clippy::too_many_arguments)]
fn dropped_norm(
    storage: &ProjectedFiniteRows<'_>,
    weights: NormalizedWeights<'_>,
    candidates: &[usize],
    scales: &[f64],
    kept: &[usize],
    dropped: usize,
    alpha: &[f64],
    row: &mut [f64],
) -> Result<f64> {
    let rows = match storage {
        ProjectedFiniteRows::Dense(values) => values.nrows(),
        ProjectedFiniteRows::Scratch { scratch, .. } => scratch.rows(),
        ProjectedFiniteRows::Centered(_) => return Err(unresolved()),
    };
    let mut reader = match storage {
        ProjectedFiniteRows::Scratch { scratch, .. } => Some(scratch.shared_reader()),
        _ => None,
    };
    let mut norm = NormUpper::default();
    for observation in 0..rows {
        if observation % 256 == 0 {
            crate::interrupt::checkpoint()?;
        }
        if let (ProjectedFiniteRows::Scratch { scratch, .. }, Some(reader)) =
            (storage, reader.as_mut())
        {
            reader.read_linear(observation * scratch.columns(), row)?;
        }
        let residual = residual_bound(
            |position| match storage {
                ProjectedFiniteRows::Dense(values) => {
                    values[(observation, candidates[position] + 1)]
                }
                ProjectedFiniteRows::Scratch { .. } => row[candidates[position]],
                ProjectedFiniteRows::Centered(_) => unreachable!(),
            },
            scales,
            kept,
            dropped,
            alpha,
        )?;
        norm.push(mul(residual, up(weights.value(observation)?.sqrt())))?;
    }
    norm.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(gram: &[f64], inverse: &[f64], error: f64, threshold: f64) -> Result<f64> {
        certify_basis(800, gram, inverse, 2, &[0, 1], 1.0, threshold, error)
    }

    #[test]
    fn orthogonal_witness_clears_the_bound() {
        assert!(check(&[1.0, 0.0, 0.0, 1.0], &[1.0, 0.0, 0.0, 1.0], 1e-12, 1e-10).is_ok());
    }

    #[test]
    fn cholesky_zero_report_scale_uses_independent_gram_bound() {
        assert!(certify_basis(
            800,
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            2,
            &[0, 1],
            0.0,
            1e-10,
            1e-12
        )
        .is_ok());
        assert!(certify_basis(
            800,
            &[1.0, 0.0, 0.0, 1e-6],
            &[1.0, 0.0, 0.0, 1e6],
            2,
            &[0, 1],
            0.0,
            1e-3,
            1e-12
        )
        .is_err());
    }

    #[test]
    fn certified_singular_margin_rejects_a_near_boundary() {
        let gram = [1.0, 0.0, 0.0, 1e-6];
        let inverse = [1.0, 0.0, 0.0, 1e6];
        assert!(check(&gram, &inverse, 1e-12, 1e-4).is_ok());
        assert!(check(&gram, &inverse, 1e-12, 1e-3).is_err());
        assert!(check(&gram, &inverse, 1e-6, 1e-4).is_err());
    }

    #[test]
    fn wrong_inverse_and_nonfinite_inputs_fail() {
        assert!(check(&[1.0, 0.0, 0.0, 1.0], &[2.0, 0.0, 0.0, 2.0], 0.0, 1e-10).is_err());
        assert!(check(
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            f64::NAN,
            1e-10
        )
        .is_err());
        assert!(validate_entry(1e-200, 1e200, 1.0).is_err());
        assert!(validate_entry(1e-200, 1.0, 1e-200).is_err());
        assert!(validate_entry(0.0, 1.0, 1.0).is_ok());
    }

    #[test]
    fn inverse_residual_is_an_absolute_enclosure() {
        let (residual, norm) =
            inverse_residual(&[2.0, 0.0, 0.0, 4.0], &[0.5, 0.0, 0.0, 0.25], 2, &[0, 1]).unwrap();
        assert!(residual > 0.0 && residual < 1e-12);
        assert!((0.5..0.500000000001).contains(&norm));
        assert!(inverse_residual(
            &[1e300, 1e300, 1e300, 1e300],
            &[1e300, -1e300, -1e300, 1e300],
            2,
            &[0, 1]
        )
        .is_err());
    }

    #[test]
    fn duplicate_column_has_a_direct_primal_certificate() {
        let cutoff = certify_basis(
            2,
            &[2.0, 2.0, 2.0, 2.0],
            &[0.5],
            2,
            &[0],
            2.0_f64.sqrt(),
            1e-10,
            1e-14,
        )
        .unwrap();
        let mut norm = NormUpper::default();
        for x in [-1.0, 1.0] {
            norm.push(residual_bound(|_| x, &[1.0, 1.0], &[0], 1, &[1.0]).unwrap())
                .unwrap();
        }
        assert!(check_dropped(norm.finish().unwrap(), &[1.0], 1e-14, cutoff).is_ok());
        // Projection uncertainty that could hide a distinct direction blocks omission.
        assert!(check_dropped(0.0, &[1.0], 1e-9, cutoff).is_err());
        assert!(check_dropped(1e-8, &[1.0], 0.0, cutoff).is_err());
    }

    #[test]
    fn selected_subset_and_negative_duplicate_scan_use_original_axes() {
        let values = DenseMatrix::from_columns(&[
            vec![0.0, 0.0],
            vec![-1.0, 1.0],
            vec![2.0, -2.0],
            vec![1.0, 1.0],
        ])
        .unwrap();
        let root_two = 2.0_f64.sqrt();
        let mut workspace = FiniteWorkspace::<()> {
            candidate_ids: vec![0, 1, 2],
            candidate_scales: vec![root_two, 2.0 * root_two, root_two],
            kept: vec![0, 2],
            gram: vec![1.0, -1.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            bread: vec![1.0, 0.0, 0.0, 1.0],
            ..Default::default()
        };
        let weights = NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        };
        assert!(verify(
            &ProjectedFiniteRows::Dense(&values),
            weights,
            &mut workspace,
            2,
            1.0,
            1e-10,
            1e-14,
            &mut SystemFitReservation
        )
        .is_ok());
        assert_eq!(workspace.kept, [0, 2]);
        assert!(verify(
            &ProjectedFiniteRows::Dense(&values),
            weights,
            &mut workspace,
            2,
            1.0,
            1e-10,
            1e-9,
            &mut SystemFitReservation
        )
        .is_err());
    }

    #[test]
    fn omission_uses_a_lower_column_scale_not_frobenius() {
        let cutoff = certify_basis(
            800,
            &[1.0, 1.0, 1.0, 1.0],
            &[1.0],
            2,
            &[1],
            1.0,
            1e-10,
            1e-14,
        )
        .unwrap();
        assert!(cutoff < 1e-10);
        assert!(check_dropped(1.1e-10, &[1.0], 0.0, cutoff).is_err());
    }

    #[test]
    fn primal_scan_catches_cancellation_hidden_in_gram() {
        // Rounded Gram entries can all equal one while rows differ. The direct
        // primal check must see the independent difference, however Gram looks.
        let residual = residual_bound(
            |j| if j == 0 { 1.0 } else { 1.0 + 1e-8 },
            &[1.0, 1.0],
            &[0],
            1,
            &[1.0],
        )
        .unwrap();
        assert!(residual >= (1.0 + 1e-8) - 1.0);
        assert!(check_dropped(residual, &[1.0], 0.0, 1e-10).is_err());
    }
}
