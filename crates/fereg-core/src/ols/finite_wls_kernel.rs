//! Static finite-WLS primitives shared by one-shot and reusable sessions.
//!
//! Estimator adapters own policy, rank/status publication, and storage
//! lifetime. This module owns only fixed-block moments, guarded Cholesky,
//! triangular solves, inverse construction, and independent numerical
//! certificates. All observation sources are concrete enum variants; there
//! are no trait objects or callbacks in the row loops.

mod axis;
mod direction;
mod workspace;
pub(super) use axis::{FiniteDenseRows, FiniteResponse};
pub(crate) use direction::{solve_finite_direction, FiniteDirectionWorkspace};
pub(super) use workspace::FiniteWorkspace;

use rayon::prelude::*;

#[cfg(test)]
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

#[cfg(test)]
use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, invalid_data, Result};
use crate::fe::BorrowedMapWeights;

use super::implicit_centered::ImplicitCenteredView;
use super::prepared_scratch::{PreparedOlsScratch, PreparedOlsScratchReader};

/// One bounded positional read per narrow row tile. Wider rows retain their
/// existing complete-row buffer and may span several scratch read chunks.
pub(crate) const RETAINED_MOMENT_TILE_ELEMENTS: usize = 1024;

pub(super) const FIXED_BLOCK_MOMENT_ROWS: usize = 16_384;
pub(super) const FIXED_BLOCK_MOMENT_MAX_COLUMNS: usize = 64;

#[derive(Debug, Clone, Copy)]
pub(super) enum FixedBlockMomentSource<'a> {
    /// Reusable dense representation: unscaled response and design with an
    /// explicit current solver-weight divisor.
    Dense {
        values: FiniteDenseRows<'a>,
        rows: usize,
        stored_columns: usize,
        selected_columns: Option<&'a [usize]>,
        response: FiniteResponse<'a>,
        weights: BorrowedMapWeights<'a>,
        solver_weight_divisor: f64,
        column_scales: Option<&'a [f64]>,
    },
    /// Row-major reusable scratch representation. Each fixed block owns one
    /// bounded complete-row tile inside its caller-provided partial workspace.
    Scratch {
        values: &'a PreparedOlsScratch,
        selected_columns: Option<&'a [usize]>,
        response: FiniteResponse<'a>,
        weights: BorrowedMapWeights<'a>,
        solver_weight_divisor: f64,
        column_scales: Option<&'a [f64]>,
    },
    Implicit {
        values: ImplicitCenteredView<'a>,
        selected_columns: Option<&'a [usize]>,
        response: FiniteResponse<'a>,
        weights: BorrowedMapWeights<'a>,
        solver_weight_divisor: f64,
        column_scales: Option<&'a [f64]>,
    },
}

impl FixedBlockMomentSource<'_> {
    pub(super) fn rows(self) -> usize {
        match self {
            Self::Dense { rows, .. } => rows,
            Self::Scratch { values, .. } => values.rows(),
            Self::Implicit { values, .. } => values.rows(),
        }
    }

    pub(super) fn dimension(self) -> usize {
        match self {
            Self::Dense {
                stored_columns,
                selected_columns,
                ..
            } => selected_columns.map_or(stored_columns, <[usize]>::len),
            Self::Scratch {
                values,
                selected_columns,
                ..
            } => selected_columns.map_or(values.columns(), <[usize]>::len),
            Self::Implicit {
                values,
                selected_columns,
                ..
            } => selected_columns.map_or(values.columns(), <[usize]>::len),
        }
    }

    fn row_workspace(self) -> usize {
        match self {
            Self::Scratch { values, .. } => {
                let columns = values.columns();
                if columns == 0 {
                    0
                } else {
                    columns
                        * values
                            .rows()
                            .max(1)
                            .min((RETAINED_MOMENT_TILE_ELEMENTS / columns).max(1))
                }
            }
            Self::Implicit { values, .. } => values.columns(),
            Self::Dense { .. } => 0,
        }
    }

    fn validate(self) -> Result<()> {
        let rows = self.rows();
        let dimension = self.dimension();
        if rows == 0 || dimension == 0 {
            return Err(invalid_argument(
                "finite-WLS fixed-block moments require nonempty dimensions",
            ));
        }
        match self {
            Self::Dense {
                values,
                rows,
                stored_columns,
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            } => {
                if !values.valid_for(rows, stored_columns) {
                    return Err(invalid_argument(
                        "finite-WLS dense physical axis is inconsistent",
                    ));
                }
                validate_retained_source(
                    values.len(),
                    rows,
                    stored_columns,
                    selected_columns,
                    response,
                    weights,
                    solver_weight_divisor,
                    column_scales,
                )?;
            }
            Self::Scratch {
                values,
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            } => validate_retained_source(
                values
                    .rows()
                    .checked_mul(values.columns())
                    .ok_or_else(|| allocation("retained finite-WLS scratch dimensions overflow"))?,
                values.rows(),
                values.columns(),
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            )?,
            Self::Implicit {
                values,
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            } => validate_retained_source(
                values
                    .rows()
                    .checked_mul(values.columns())
                    .ok_or_else(|| allocation("implicit moment dimensions overflow"))?,
                values.rows(),
                values.columns(),
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            )?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FiniteWlsMomentLayout {
    pub(super) partitions: usize,
    pub(super) columns_per_partition: usize,
    pub(super) partial_stride: usize,
    pub(super) partial_elements: usize,
}

pub(super) fn finite_wls_moment_layout(
    source: FixedBlockMomentSource<'_>,
) -> Result<FiniteWlsMomentLayout> {
    source.validate()?;
    FiniteWlsMomentLayout::plan(
        source.dimension(),
        source.row_workspace(),
        rayon::current_num_threads(),
    )
}

impl FiniteWlsMomentLayout {
    fn plan(dimension: usize, row_workspace: usize, threads: usize) -> Result<Self> {
        // Physical leaves preserve the retained scratch reduction order; the
        // general stage policy separately bounds how many leaves run at once.
        let columns_per_partition = dimension.div_ceil(threads.min(dimension).max(1));
        let partitions = dimension.div_ceil(columns_per_partition);
        Ok(Self {
            partitions,
            columns_per_partition,
            partial_stride: row_workspace,
            partial_elements: partitions
                .checked_mul(row_workspace)
                .ok_or_else(|| allocation("finite-WLS retained row workspace overflows"))?,
        })
    }
}

fn moment_lane_work(rows: usize, dimension: usize) -> u128 {
    (rows as u128)
        .saturating_mul(dimension as u128)
        .saturating_mul(dimension as u128 + 1)
        / 2
}

pub(crate) fn retained_scratch_moment_envelope(
    rows: u128,
    columns: u128,
    threads: u128,
) -> Result<u128> {
    // Selected rank can shrink without changing stored width; its actual
    // partition count is not monotone. Preserve the admission envelope across
    // every rank, rather than substituting concrete whole-row tile flooring.
    let tile = rows
        .checked_mul(columns)
        .ok_or_else(|| {
            allocation("PPML memory estimate overflow while sizing PPML moment row-tile cells")
        })?
        .min(columns.max(RETAINED_MOMENT_TILE_ELEMENTS as u128));
    threads.min(columns).checked_mul(tile)
        .ok_or_else(|| allocation("PPML memory estimate overflow while sizing PPML retained scratch moment tile workspaces"))
}

/// Fill deterministic Gram and RHS moments into caller-owned storage.
/// Narrow prepared designs retain fixed-block compensated merging. Wide and
/// retained designs partition Gram rows, with a thread-independent row order.
pub(super) fn fill_finite_wls_moments(
    source: FixedBlockMomentSource<'_>,
    layout: FiniteWlsMomentLayout,
    partials: &mut [f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    source.validate()?;
    let dimension = source.dimension();
    let square = checked_square(dimension, "finite-WLS Gram dimensions overflow")?;
    if layout != finite_wls_moment_layout(source)?
        || partials.len() != layout.partial_elements
        || rhs.len() != dimension
        || gram.len() != square
    {
        return Err(invalid_argument(
            "finite-WLS fixed-block workspace dimensions are inconsistent",
        ));
    }
    partials.fill(0.0);
    rhs.fill(0.0);
    gram.fill(0.0);

    fill_retained_partitioned(source, layout, partials, rhs, gram)
}

/// Stable retained-design rank revelation without a second N-by-K design.
///
/// Rows are inserted, in canonical observation order, into one caller-owned
/// upper-triangular factor with Givens rotations.  A complete column-pivoted
/// Householder QR of that K-by-K factor then applies the same rank policy as
/// the one-shot observation-space QR.  `transformed_rhs` is carried through
/// both orthogonal transformations, so the accepted solve never falls back to
/// normal equations.  The implementation is intentionally serial: retaining
/// one K-by-K factor is the bounded-memory contract, whereas deterministic
/// parallel row partitions would require one additional K-by-K factor per
/// partition.
#[allow(clippy::too_many_arguments)]
pub(super) fn factor_rank_revealing_retained_qr(
    source: FixedBlockMomentSource<'_>,
    canonical_generated_ids: &[usize],
    relative_threshold: f64,
    stored_row: &mut [f64],
    qr: &mut [f64],
    transformed_rhs: &mut [f64],
    trailing_norms: &mut [f64],
    permutation: &mut [usize],
) -> Result<RankRevealingQrReport> {
    source.validate()?;
    let dimension = source.dimension();
    let square = checked_square(dimension, "retained finite-WLS QR dimensions overflow")?;
    if canonical_generated_ids.len() != dimension
        || qr.len() != square
        || transformed_rhs.len() != dimension
        || trailing_norms.len() != dimension
        || permutation.len() != dimension
        || !relative_threshold.is_finite()
        || relative_threshold <= 0.0
        || relative_threshold >= 1.0
        || !identities_are_unique(canonical_generated_ids)
    {
        return Err(invalid_argument(
            "retained finite-WLS QR inputs are inconsistent",
        ));
    }
    let stored_columns = match source {
        FixedBlockMomentSource::Dense { stored_columns, .. } => stored_columns,
        FixedBlockMomentSource::Scratch { values, .. } => values.columns(),
        FixedBlockMomentSource::Implicit { values, .. } => values.columns(),
    };
    if stored_row.len() != stored_columns {
        return Err(invalid_argument(
            "retained finite-WLS QR row workspace has the wrong width",
        ));
    }

    qr.fill(0.0);
    transformed_rhs.fill(0.0);
    trailing_norms.fill(0.0);
    for (position, destination) in permutation.iter_mut().enumerate() {
        *destination = position;
    }

    // `trailing_norms` is the selected/standardized incoming row during the
    // streaming reduction. It is overwritten by actual trailing norms below.
    let mut scratch_reader = match source {
        FixedBlockMomentSource::Scratch { values, .. } => Some(values.shared_reader()),
        _ => None,
    };
    for observation in 0..source.rows() {
        let mut outcome = fill_retained_qr_row(
            source,
            observation,
            stored_row,
            trailing_norms,
            scratch_reader.as_mut(),
        )?;
        for step in 0..dimension {
            let diagonal = qr[step * dimension + step];
            let incoming = trailing_norms[step];
            if incoming == 0.0 {
                continue;
            }
            let rotated = diagonal.hypot(incoming);
            if !rotated.is_finite() || rotated <= 0.0 {
                return Err(invalid_data(
                    "retained finite-WLS streaming QR rotation is invalid",
                ));
            }
            let cosine = diagonal / rotated;
            let sine = incoming / rotated;
            qr[step * dimension + step] = rotated;
            trailing_norms[step] = 0.0;
            for column in (step + 1)..dimension {
                let upper = qr[step * dimension + column];
                let row_value = trailing_norms[column];
                qr[step * dimension + column] = cosine * upper + sine * row_value;
                trailing_norms[column] = -sine * upper + cosine * row_value;
            }
            let upper_outcome = transformed_rhs[step];
            transformed_rhs[step] = cosine * upper_outcome + sine * outcome;
            outcome = -sine * upper_outcome + cosine * outcome;
            if qr[step * dimension..(step + 1) * dimension]
                .iter()
                .chain(&transformed_rhs[step..=step])
                .chain(&trailing_norms[step..])
                .any(|value| !value.is_finite())
                || !outcome.is_finite()
            {
                return Err(invalid_data(
                    "retained finite-WLS streaming QR is not finite",
                ));
            }
        }
    }

    for (position, norm) in trailing_norms.iter_mut().enumerate() {
        *norm = stable_qr_column_norm(qr, dimension, position, 0)?;
    }
    let initial_norm = trailing_norms.iter().copied().fold(0.0_f64, f64::max);
    if !initial_norm.is_finite() {
        return Err(invalid_data(
            "retained finite-WLS QR initial scale is not finite",
        ));
    }
    if initial_norm == 0.0 {
        return Ok(RankRevealingQrReport {
            rank: 0,
            initial_norm: 0.0,
            maximum_dropped_relative_norm: 0.0,
            condition_proxy: 1.0,
            ambiguous: false,
        });
    }
    let absolute_threshold = initial_norm * relative_threshold;
    let relative_guard = RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON;
    let lower_boundary = absolute_threshold * (1.0 - relative_guard);
    let upper_boundary = absolute_threshold * (1.0 + relative_guard);
    let mut rank = 0_usize;
    let mut largest_diagonal = 0.0_f64;
    let mut smallest_diagonal = f64::INFINITY;
    let mut ambiguous = false;

    while rank < dimension {
        for position in rank..dimension {
            trailing_norms[position] =
                stable_qr_column_norm(qr, dimension, permutation[position], rank)?;
            ambiguous |= trailing_norms[position] >= lower_boundary
                && trailing_norms[position] <= upper_boundary;
        }
        let mut pivot_position = rank;
        let mut pivot_norm = trailing_norms[rank];
        for position in (rank + 1)..dimension {
            let candidate_norm = trailing_norms[position];
            let tie_scale = candidate_norm.abs().max(pivot_norm.abs());
            let tied = (candidate_norm - pivot_norm).abs()
                <= RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON * tie_scale;
            if candidate_norm > pivot_norm && !tied
                || tied
                    && canonical_generated_ids[permutation[position]]
                        < canonical_generated_ids[permutation[pivot_position]]
            {
                pivot_position = position;
                pivot_norm = candidate_norm;
            }
        }
        ambiguous |= pivot_norm >= lower_boundary && pivot_norm <= upper_boundary;
        if pivot_norm <= absolute_threshold {
            break;
        }
        permutation.swap(rank, pivot_position);
        trailing_norms.swap(rank, pivot_position);
        let pivot_column = permutation[rank];
        let first = qr[rank * dimension + pivot_column];
        let beta = if first >= 0.0 {
            -pivot_norm
        } else {
            pivot_norm
        };
        let denominator = first - beta;
        if !beta.is_finite() || beta == 0.0 || !denominator.is_finite() || denominator == 0.0 {
            return Err(invalid_data("retained finite-WLS QR reflector is invalid"));
        }
        let tau = (beta - first) / beta;
        qr[rank * dimension + pivot_column] = beta;
        for row in (rank + 1)..dimension {
            qr[row * dimension + pivot_column] /= denominator;
        }
        for &target_column in permutation.iter().skip(rank + 1) {
            apply_square_qr_householder(qr, dimension, rank, pivot_column, target_column, tau)?;
        }
        apply_qr_householder_to_vector(qr, dimension, rank, pivot_column, transformed_rhs, tau)?;
        let diagonal = beta.abs();
        largest_diagonal = largest_diagonal.max(diagonal);
        smallest_diagonal = smallest_diagonal.min(diagonal);
        rank += 1;
    }

    let maximum_dropped_norm = if rank == dimension {
        0.0
    } else {
        let mut maximum = 0.0_f64;
        for &dropped_column in permutation.iter().skip(rank) {
            let norm = stable_qr_column_norm(qr, dimension, dropped_column, rank)?;
            ambiguous |= norm >= lower_boundary && norm <= upper_boundary;
            maximum = maximum.max(norm);
        }
        maximum
    };
    let maximum_dropped_relative_norm = maximum_dropped_norm / initial_norm;
    if !maximum_dropped_relative_norm.is_finite()
        || maximum_dropped_relative_norm > relative_threshold * (1.0 + relative_guard)
    {
        return Err(invalid_data(
            "retained finite-WLS QR dropped-span certificate did not pass",
        ));
    }
    let condition_proxy = if rank == 0 {
        1.0
    } else {
        largest_diagonal / smallest_diagonal
    };
    if !condition_proxy.is_finite() || condition_proxy < 1.0 {
        return Err(invalid_data(
            "retained finite-WLS QR condition proxy is invalid",
        ));
    }
    Ok(RankRevealingQrReport {
        rank,
        initial_norm,
        maximum_dropped_relative_norm,
        condition_proxy,
        ambiguous,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RankRevealingQrReport {
    pub(super) rank: usize,
    pub(super) initial_norm: f64,
    pub(super) maximum_dropped_relative_norm: f64,
    pub(super) condition_proxy: f64,
    pub(super) ambiguous: bool,
}

/// Independently certify the CPQR dropped span by solving `R11 A = R12`
/// and rescanning the retained observation rows.  The certificate never uses
/// a Gram Schur complement, whose cancellation floor is O(sqrt(eps)) in
/// relative column-norm units.
#[allow(clippy::too_many_arguments)]
pub(super) fn certify_retained_qr_dropped_span(
    source: FixedBlockMomentSource<'_>,
    qr: &[f64],
    permutation: &[usize],
    rank: usize,
    initial_norm: f64,
    relative_threshold: f64,
    stored_row: &mut [f64],
    standardized_row: &mut [f64],
    projection_coefficients: &mut [f64],
    residual_norms: &mut [f64],
) -> Result<DroppedSpanCertificate> {
    source.validate()?;
    let dimension = source.dimension();
    let dropped = dimension.saturating_sub(rank);
    if rank > dimension
        || qr.len() != checked_square(dimension, "retained span QR dimensions overflow")?
        || !valid_permutation(permutation)
        || !initial_norm.is_finite()
        || initial_norm < 0.0
        || !relative_threshold.is_finite()
        || relative_threshold <= 0.0
        || relative_threshold >= 1.0
        || stored_row.len()
            != match source {
                FixedBlockMomentSource::Dense { stored_columns, .. } => stored_columns,
                FixedBlockMomentSource::Scratch { values, .. } => values.columns(),
                FixedBlockMomentSource::Implicit { values, .. } => values.columns(),
            }
        || standardized_row.len() != dimension
        || projection_coefficients.len()
            != rank
                .checked_mul(dropped)
                .ok_or_else(|| allocation("retained span projection dimensions overflow"))?
        || residual_norms.len() != dropped
    {
        return Err(invalid_argument(
            "retained finite-WLS dropped-span inputs are inconsistent",
        ));
    }
    if dropped == 0 {
        return Ok(DroppedSpanCertificate {
            maximum_dropped_relative_norm: 0.0,
            ambiguous: false,
            passed: true,
        });
    }
    if initial_norm == 0.0 {
        return Ok(DroppedSpanCertificate {
            maximum_dropped_relative_norm: f64::INFINITY,
            ambiguous: false,
            passed: false,
        });
    }

    projection_coefficients.fill(0.0);
    for dropped_position in 0..dropped {
        let pivot_position = rank + dropped_position;
        for row in (0..rank).rev() {
            let mut value = qr[row * dimension + permutation[pivot_position]];
            for later in (row + 1)..rank {
                value -= qr[row * dimension + permutation[later]]
                    * projection_coefficients[later * dropped + dropped_position];
            }
            let diagonal = qr[row * dimension + permutation[row]];
            let coefficient = value / diagonal;
            if !diagonal.is_finite() || diagonal == 0.0 || !coefficient.is_finite() {
                return Err(invalid_data(
                    "retained finite-WLS dropped-span projection is not finite",
                ));
            }
            projection_coefficients[row * dropped + dropped_position] = coefficient;
        }
    }

    residual_norms.fill(0.0);
    let mut scratch_reader = match source {
        FixedBlockMomentSource::Scratch { values, .. } => Some(values.shared_reader()),
        _ => None,
    };
    for observation in 0..source.rows() {
        let _ = fill_retained_qr_row(
            source,
            observation,
            stored_row,
            standardized_row,
            scratch_reader.as_mut(),
        )?;
        for dropped_position in 0..dropped {
            let mut residual = standardized_row[permutation[rank + dropped_position]];
            for selected in 0..rank {
                residual -= standardized_row[permutation[selected]]
                    * projection_coefficients[selected * dropped + dropped_position];
            }
            residual_norms[dropped_position] = residual_norms[dropped_position].hypot(residual);
            if !residual_norms[dropped_position].is_finite() {
                return Err(invalid_data(
                    "retained finite-WLS dropped-span residual is not finite",
                ));
            }
        }
    }
    let maximum_dropped_relative_norm =
        residual_norms.iter().copied().fold(0.0_f64, f64::max) / initial_norm;
    let guard = RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON;
    let lower = relative_threshold * (1.0 - guard);
    let upper = relative_threshold * (1.0 + guard);
    let ambiguous = residual_norms.iter().copied().any(|norm| {
        let relative = norm / initial_norm;
        relative >= lower && relative <= upper
    });
    Ok(DroppedSpanCertificate {
        maximum_dropped_relative_norm,
        ambiguous,
        passed: !ambiguous
            && maximum_dropped_relative_norm.is_finite()
            && maximum_dropped_relative_norm <= upper,
    })
}

/// Solve the accepted leading CPQR system and publish coefficients in
/// ascending generated-column identity. `pivot_solution` is caller-owned
/// scratch and the K-by-K factor keeps its original full stride.
#[allow(clippy::too_many_arguments)]
pub(super) fn solve_rank_revealing_qr_leading(
    qr: &[f64],
    transformed_rhs: &[f64],
    permutation: &[usize],
    canonical_generated_ids: &[usize],
    rank: usize,
    pivot_solution: &mut [f64],
    canonical_selected_ids: &mut [usize],
    canonical_solution: &mut [f64],
) -> Result<bool> {
    let dimension = permutation.len();
    if rank > dimension
        || qr.len() != checked_square(dimension, "retained QR solve dimensions overflow")?
        || transformed_rhs.len() != dimension
        || canonical_generated_ids.len() != dimension
        || pivot_solution.len() != rank
        || canonical_selected_ids.len() != rank
        || canonical_solution.len() != rank
        || !valid_permutation(permutation)
        || !identities_are_unique(canonical_generated_ids)
    {
        return Err(invalid_argument(
            "retained finite-WLS QR solve dimensions are inconsistent",
        ));
    }
    pivot_solution.fill(0.0);
    for row in (0..rank).rev() {
        let mut value = transformed_rhs[row];
        for later in (row + 1)..rank {
            value -= qr[row * dimension + permutation[later]] * pivot_solution[later];
        }
        let diagonal = qr[row * dimension + permutation[row]];
        let solved = value / diagonal;
        if !diagonal.is_finite() || diagonal == 0.0 || !solved.is_finite() {
            return Ok(false);
        }
        pivot_solution[row] = solved;
    }
    for position in 0..rank {
        canonical_selected_ids[position] = canonical_generated_ids[permutation[position]];
        canonical_solution[position] = pivot_solution[position];
    }
    sort_selected_solution(canonical_selected_ids, canonical_solution);
    Ok(true)
}

/// Form `(R'R)^-1` from the accepted leading triangular CPQR factor and
/// publish it in ascending generated-column identity.  This avoids the
/// condition-number squaring of refactoring a normal-equation Gram.
#[allow(clippy::too_many_arguments)]
pub(super) fn invert_rank_revealing_qr_leading(
    qr: &[f64],
    permutation: &[usize],
    canonical_generated_ids: &[usize],
    rank: usize,
    inverse_r: &mut [f64],
    canonical_selected_ids: &mut [usize],
    canonical_inverse: &mut [f64],
) -> Result<bool> {
    let dimension = permutation.len();
    let rank_square = checked_square(rank, "retained QR inverse dimensions overflow")?;
    if rank > dimension
        || qr.len() != checked_square(dimension, "retained QR factor dimensions overflow")?
        || canonical_generated_ids.len() != dimension
        || inverse_r.len() != rank_square
        || canonical_selected_ids.len() != rank
        || canonical_inverse.len() != rank_square
        || !valid_permutation(permutation)
        || !identities_are_unique(canonical_generated_ids)
    {
        return Err(invalid_argument(
            "retained finite-WLS QR inverse dimensions are inconsistent",
        ));
    }
    inverse_r.fill(0.0);
    for right_hand_side in 0..rank {
        for row in (0..rank).rev() {
            let mut value = f64::from(row == right_hand_side);
            let mut correction = 0.0_f64;
            for inner in (row + 1)..rank {
                compensated_add(
                    -qr[row * dimension + permutation[inner]]
                        * inverse_r[inner * rank + right_hand_side],
                    &mut value,
                    &mut correction,
                )?;
            }
            let diagonal = qr[row * dimension + permutation[row]];
            let solved = (value + correction) / diagonal;
            if !diagonal.is_finite() || diagonal == 0.0 || !solved.is_finite() {
                return Ok(false);
            }
            inverse_r[row * rank + right_hand_side] = solved;
        }
    }
    canonical_inverse.fill(0.0);
    for left in 0..rank {
        for right in left..rank {
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..rank {
                compensated_add(
                    inverse_r[left * rank + inner] * inverse_r[right * rank + inner],
                    &mut sum,
                    &mut correction,
                )?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Ok(false);
            }
            canonical_inverse[left * rank + right] = value;
            canonical_inverse[right * rank + left] = value;
        }
    }
    for position in 0..rank {
        canonical_selected_ids[position] = canonical_generated_ids[permutation[position]];
    }
    sort_selected_inverse(canonical_selected_ids, canonical_inverse);
    Ok(true)
}

fn fill_retained_qr_row(
    source: FixedBlockMomentSource<'_>,
    observation: usize,
    stored_row: &mut [f64],
    output: &mut [f64],
    scratch_reader: Option<&mut PreparedOlsScratchReader<'_>>,
) -> Result<f64> {
    let (selected_columns, response, weights, divisor, scales) = match source {
        FixedBlockMomentSource::Dense {
            values,
            stored_columns: _,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
            ..
        } => {
            stored_row.copy_from_slice(values.row(observation));
            (
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            )
        }
        FixedBlockMomentSource::Scratch {
            values,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
        } => {
            scratch_reader
                .expect("scratch QR scan owns its reader")
                .read_linear(observation * values.columns(), stored_row)?;
            (
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            )
        }
        FixedBlockMomentSource::Implicit {
            values,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
        } => {
            values.fill_row(observation, stored_row)?;
            (
                selected_columns,
                response,
                weights,
                solver_weight_divisor,
                column_scales,
            )
        }
    };
    let root_weight = normalized_weight(weights, divisor, observation)?.sqrt();
    if !root_weight.is_finite() {
        return Err(invalid_data(
            "retained finite-WLS QR root weight is not finite",
        ));
    }
    for (position, destination) in output.iter_mut().enumerate() {
        let source_position = selected_columns.map_or(position, |selected| selected[position]);
        *destination = root_weight * scaled_column(stored_row[source_position], scales, position)?;
        if !destination.is_finite() {
            return Err(invalid_data("retained finite-WLS QR row is not finite"));
        }
    }
    let outcome = root_weight * response[observation];
    if outcome.is_finite() {
        Ok(outcome)
    } else {
        Err(invalid_data("retained finite-WLS QR outcome is not finite"))
    }
}

fn stable_qr_column_norm(
    matrix: &[f64],
    dimension: usize,
    column: usize,
    start_row: usize,
) -> Result<f64> {
    if matrix.len() != checked_square(dimension, "retained QR norm dimensions overflow")?
        || column >= dimension
        || start_row > dimension
    {
        return Err(invalid_argument(
            "retained finite-WLS QR norm axes are inconsistent",
        ));
    }
    let mut norm = 0.0_f64;
    for row in start_row..dimension {
        norm = norm.hypot(matrix[row * dimension + column]);
    }
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(invalid_data("retained finite-WLS QR norm is not finite"))
    }
}

fn apply_square_qr_householder(
    matrix: &mut [f64],
    dimension: usize,
    start_row: usize,
    reflector_column: usize,
    target_column: usize,
    tau: f64,
) -> Result<()> {
    let mut projection = matrix[start_row * dimension + target_column];
    for row in (start_row + 1)..dimension {
        projection +=
            matrix[row * dimension + reflector_column] * matrix[row * dimension + target_column];
    }
    projection *= tau;
    matrix[start_row * dimension + target_column] -= projection;
    for row in (start_row + 1)..dimension {
        matrix[row * dimension + target_column] -=
            matrix[row * dimension + reflector_column] * projection;
    }
    if projection.is_finite()
        && (start_row..dimension).all(|row| matrix[row * dimension + target_column].is_finite())
    {
        Ok(())
    } else {
        Err(invalid_data(
            "retained finite-WLS QR transformed column is not finite",
        ))
    }
}

fn apply_qr_householder_to_vector(
    matrix: &[f64],
    dimension: usize,
    start_row: usize,
    reflector_column: usize,
    target: &mut [f64],
    tau: f64,
) -> Result<()> {
    let mut projection = target[start_row];
    for row in (start_row + 1)..dimension {
        projection += matrix[row * dimension + reflector_column] * target[row];
    }
    projection *= tau;
    target[start_row] -= projection;
    for row in (start_row + 1)..dimension {
        target[row] -= matrix[row * dimension + reflector_column] * projection;
    }
    if projection.is_finite() && target.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(invalid_data(
            "retained finite-WLS QR transformed outcome is not finite",
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CholeskyPivotPolicy {
    Canonical,
    MaximumDiagonal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GuardedCholeskyReport {
    pub(super) minimum_pivot: f64,
    pub(super) maximum_pivot: f64,
    pub(super) minimum_diagonal: f64,
    pub(super) maximum_diagonal: f64,
}

/// Rank decision and numerical diagnostics from complete diagonal-pivoted
/// Cholesky on a standardized Gram matrix.
///
/// `maximum_dropped_relative_norm` is the square root of the largest dropped
/// Schur diagonal divided by the largest original Gram diagonal. The
/// `condition_proxy` is the ratio of the largest to the smallest accepted
/// factor diagonal. A decision within the roundoff guard around the requested
/// relative column-norm threshold is reported as ambiguous so estimator policy
/// can fail closed.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RankRevealingCholeskyReport {
    pub(super) rank: usize,
    pub(super) maximum_dropped_relative_norm: f64,
    pub(super) condition_proxy: f64,
    pub(super) ambiguous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DroppedSpanCertificate {
    pub(super) maximum_dropped_relative_norm: f64,
    pub(super) ambiguous: bool,
    pub(super) passed: bool,
}

/// Return whether two Gram rows are bit-exact signed copies on the requested
/// trailing block. Retained columns that are exact scalar copies become
/// signed copies after reference-norm standardization. Recognizing that
/// identity before a square-root/subtraction round trip prevents an exact
/// zero Schur complement from being promoted to an O(eps) positive pivot.
#[cfg(test)]
fn exact_signed_gram_duplicate(
    gram: &[f64],
    dimension: usize,
    left: usize,
    right: usize,
    start: usize,
) -> bool {
    let left_diagonal = gram[left * dimension + left];
    let right_diagonal = gram[right * dimension + right];
    let cross = gram[left * dimension + right];
    if left_diagonal != right_diagonal || cross.abs() != left_diagonal {
        return false;
    }
    let opposite = cross.is_sign_negative();
    (start..dimension).all(|column| {
        let left_value = gram[left * dimension + column];
        let right_value = gram[right * dimension + column];
        if opposite {
            right_value == -left_value
        } else {
            right_value == left_value
        }
    })
}

/// Independently certify every unselected standardized column against the
/// selected span using the untouched Gram matrix and its selected inverse.
#[cfg(test)]
pub(super) fn certify_rank_revealing_dropped_span(
    original_standardized_gram: &[f64],
    dimension: usize,
    selected_positions: &[usize],
    selected_inverse: &[f64],
    relative_threshold: f64,
) -> Result<DroppedSpanCertificate> {
    let rank = selected_positions.len();
    if dimension.checked_mul(dimension) != Some(original_standardized_gram.len())
        || selected_inverse.len()
            != checked_square(rank, "rank-revealing dropped-span dimensions overflow")?
        || selected_positions.windows(2).any(|pair| pair[0] >= pair[1])
        || selected_positions
            .last()
            .is_some_and(|position| *position >= dimension)
        || !relative_threshold.is_finite()
        || relative_threshold <= 0.0
        || relative_threshold >= 1.0
        || original_standardized_gram
            .iter()
            .chain(selected_inverse)
            .any(|value| !value.is_finite())
    {
        return Err(invalid_argument(
            "rank-revealing dropped-span inputs are inconsistent",
        ));
    }
    for row in 0..dimension {
        if original_standardized_gram[row * dimension + row] < 0.0 {
            return Err(invalid_data(
                "rank-revealing dropped-span Gram has a negative diagonal",
            ));
        }
        for column in (row + 1)..dimension {
            if original_standardized_gram[row * dimension + column]
                != original_standardized_gram[column * dimension + row]
            {
                return Err(invalid_data(
                    "rank-revealing dropped-span Gram is not symmetric",
                ));
            }
        }
    }
    let initial_diagonal = (0..dimension)
        .map(|position| original_standardized_gram[position * dimension + position])
        .fold(0.0_f64, f64::max);
    if initial_diagonal < 0.0 || !initial_diagonal.is_finite() {
        return Err(invalid_data("rank-revealing dropped-span scale is invalid"));
    }
    if initial_diagonal == 0.0 {
        return Ok(DroppedSpanCertificate {
            maximum_dropped_relative_norm: 0.0,
            ambiguous: false,
            passed: selected_positions.is_empty()
                && original_standardized_gram.iter().all(|value| *value == 0.0),
        });
    }
    let guard = RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON;
    let lower = relative_threshold * (1.0 - guard);
    let upper = relative_threshold * (1.0 + guard);
    let negative_guard = guard * initial_diagonal;
    let mut maximum = 0.0_f64;
    let mut ambiguous = false;
    let mut passed = true;
    for dropped in 0..dimension {
        if selected_positions.binary_search(&dropped).is_ok() {
            continue;
        }
        if selected_positions.iter().copied().any(|selected| {
            exact_signed_gram_duplicate(original_standardized_gram, dimension, selected, dropped, 0)
        }) {
            continue;
        }
        let mut projection = 0.0_f64;
        let mut projection_correction = 0.0_f64;
        for (left, selected_left) in selected_positions.iter().copied().enumerate() {
            let mut inner = 0.0_f64;
            let mut inner_correction = 0.0_f64;
            for (right, selected_right) in selected_positions.iter().copied().enumerate() {
                compensated_add(
                    selected_inverse[left * rank + right]
                        * original_standardized_gram[selected_right * dimension + dropped],
                    &mut inner,
                    &mut inner_correction,
                )?;
            }
            compensated_add(
                original_standardized_gram[dropped * dimension + selected_left]
                    * (inner + inner_correction),
                &mut projection,
                &mut projection_correction,
            )?;
        }
        let schur = original_standardized_gram[dropped * dimension + dropped]
            - (projection + projection_correction);
        if !schur.is_finite() || schur < -negative_guard {
            return Err(invalid_data(
                "rank-revealing dropped-span Schur residual is invalid",
            ));
        }
        let relative = (schur.max(0.0) / initial_diagonal).sqrt();
        if !relative.is_finite() {
            return Err(invalid_data(
                "rank-revealing dropped-span relative norm is invalid",
            ));
        }
        maximum = maximum.max(relative);
        ambiguous |= relative >= lower && relative <= upper;
        passed &= relative <= upper;
    }
    Ok(DroppedSpanCertificate {
        maximum_dropped_relative_norm: maximum,
        ambiguous,
        passed: passed && !ambiguous,
    })
}

pub(super) const RANK_REVEALING_GUARD_EPSILONS: f64 = 64.0;

/// Guard normal-equation use before invoking the destructive physical kernel.
/// The unchanged stronger guard keeps weak designs under QR rank authority.
pub(super) fn factor_finite_cholesky(
    gram: &mut [f64],
    rhs: &mut [f64],
    factor: &mut [f64],
    permutation: &mut [usize],
    relative_threshold: f64,
) -> Result<Option<GuardedCholeskyReport>> {
    let dimension = rhs.len();
    if gram.len() != checked_square(dimension, "finite-WLS Cholesky dimensions overflow")? {
        return Err(invalid_argument(
            "finite-WLS Cholesky Gram dimensions are inconsistent",
        ));
    }
    let maximum = (0..dimension)
        .map(|index| gram[index * dimension + index])
        .fold(0.0_f64, f64::max);
    let guard = maximum * relative_threshold.sqrt();
    if !maximum.is_finite() || maximum <= 0.0 || !guard.is_finite() || guard <= 0.0 {
        return Ok(None);
    }
    factor_guarded_cholesky_in_place(
        gram,
        rhs,
        factor,
        permutation,
        CholeskyPivotPolicy::MaximumDiagonal,
        guard,
    )
}

/// Solve in pivot order and return canonical coefficients in a disjoint bank.
pub(super) fn solve_finite_cholesky(
    factor: &[f64],
    rhs: &[f64],
    permutation: &[usize],
    forward: &mut [f64],
    pivot: &mut [f64],
    canonical: &mut [f64],
) -> Result<bool> {
    if canonical.len() != rhs.len()
        || permutation.len() != rhs.len()
        || permutation.iter().any(|&position| position >= rhs.len())
    {
        return Err(invalid_argument(
            "finite-WLS canonical solution axis is inconsistent",
        ));
    }
    if !solve_guarded_cholesky(factor, rhs, forward, pivot)? {
        return Ok(false);
    }
    canonical.fill(0.0);
    for (position, &target) in permutation.iter().enumerate() {
        canonical[target] = pivot[position];
    }
    Ok(true)
}

/// Destructively factor a caller-owned Gram/RHS copy. A numerical guard miss
/// returns `Ok(None)` so one-shot policy can retain its canonical QR fallback.
pub(super) fn factor_guarded_cholesky_in_place(
    gram: &mut [f64],
    rhs: &mut [f64],
    factor: &mut [f64],
    permutation: &mut [usize],
    pivot_policy: CholeskyPivotPolicy,
    pivot_guard: f64,
) -> Result<Option<GuardedCholeskyReport>> {
    let dimension = rhs.len();
    let square = checked_square(dimension, "finite-WLS Cholesky dimensions overflow")?;
    if dimension == 0
        || gram.len() != square
        || factor.len() != square
        || permutation.len() != dimension
        || !pivot_guard.is_finite()
        || pivot_guard < 0.0
    {
        return Err(invalid_argument(
            "finite-WLS guarded Cholesky dimensions or threshold are invalid",
        ));
    }
    factor.fill(0.0);
    for (position, value) in permutation.iter_mut().enumerate() {
        *value = position;
    }
    let mut minimum_pivot = f64::INFINITY;
    let mut maximum_pivot = 0.0_f64;
    let mut minimum_diagonal = f64::INFINITY;
    let mut maximum_diagonal = 0.0_f64;
    for step in 0..dimension {
        let mut pivot_position = step;
        let mut pivot_value = gram[step * dimension + step];
        if pivot_policy == CholeskyPivotPolicy::MaximumDiagonal {
            for candidate in (step + 1)..dimension {
                let value = gram[candidate * dimension + candidate];
                if value > pivot_value {
                    pivot_position = candidate;
                    pivot_value = value;
                }
            }
        }
        if !pivot_value.is_finite() || pivot_value <= pivot_guard {
            return Ok(None);
        }
        if pivot_position != step {
            swap_symmetric_block(gram, dimension, step, pivot_position);
            rhs.swap(step, pivot_position);
            permutation.swap(step, pivot_position);
            for column in 0..step {
                factor.swap(
                    step * dimension + column,
                    pivot_position * dimension + column,
                );
            }
        }
        let diagonal = gram[step * dimension + step].sqrt();
        if !diagonal.is_finite() || diagonal <= 0.0 {
            return Ok(None);
        }
        factor[step * dimension + step] = diagonal;
        minimum_pivot = minimum_pivot.min(pivot_value);
        maximum_pivot = maximum_pivot.max(pivot_value);
        minimum_diagonal = minimum_diagonal.min(diagonal);
        maximum_diagonal = maximum_diagonal.max(diagonal);
        for row in (step + 1)..dimension {
            factor[row * dimension + step] = gram[row * dimension + step] / diagonal;
        }
        for row in (step + 1)..dimension {
            for column in row..dimension {
                let updated = gram[column * dimension + row]
                    - factor[row * dimension + step] * factor[column * dimension + step];
                if !updated.is_finite() {
                    return Ok(None);
                }
                gram[column * dimension + row] = updated;
                gram[row * dimension + column] = updated;
            }
        }
    }
    Ok(Some(GuardedCholeskyReport {
        minimum_pivot,
        maximum_pivot,
        minimum_diagonal,
        maximum_diagonal,
    }))
}

/// Copy and rank-factor original standardized moments into caller-owned
/// scratch without modifying the originals.
///
/// The returned `permutation` contains physical input positions in factor
/// order. Complete diagonal pivot ties within 64 machine epsilons are resolved
/// by the smaller canonical generated-column identity. `relative_threshold`
/// has column-norm units, so Schur diagonals are compared to its square.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn factor_rank_revealing_cholesky(
    original_standardized_gram: &[f64],
    original_standardized_rhs: &[f64],
    canonical_generated_ids: &[usize],
    relative_threshold: f64,
    gram_scratch: &mut [f64],
    rhs_scratch: &mut [f64],
    factor: &mut [f64],
    permutation: &mut [usize],
) -> Result<RankRevealingCholeskyReport> {
    let dimension = original_standardized_rhs.len();
    let square = checked_square(
        dimension,
        "rank-revealing finite-WLS Cholesky dimensions overflow",
    )?;
    if original_standardized_gram.len() != square
        || canonical_generated_ids.len() != dimension
        || gram_scratch.len() != square
        || rhs_scratch.len() != dimension
        || factor.len() != square
        || permutation.len() != dimension
        || !relative_threshold.is_finite()
        || relative_threshold <= 0.0
        || relative_threshold >= 1.0
        || !identities_are_unique(canonical_generated_ids)
    {
        return Err(invalid_argument(
            "rank-revealing finite-WLS Cholesky inputs are inconsistent",
        ));
    }
    if original_standardized_gram
        .iter()
        .chain(original_standardized_rhs)
        .any(|value| !value.is_finite())
    {
        return Err(invalid_data(
            "rank-revealing finite-WLS moments are not finite",
        ));
    }
    for row in 0..dimension {
        if original_standardized_gram[row * dimension + row] < 0.0 {
            return Err(invalid_data(
                "rank-revealing finite-WLS Gram has a negative diagonal",
            ));
        }
        for column in (row + 1)..dimension {
            if original_standardized_gram[row * dimension + column]
                != original_standardized_gram[column * dimension + row]
            {
                return Err(invalid_data(
                    "rank-revealing finite-WLS Gram is not symmetric",
                ));
            }
        }
    }

    gram_scratch.copy_from_slice(original_standardized_gram);
    rhs_scratch.copy_from_slice(original_standardized_rhs);
    factor.fill(0.0);
    for (position, destination) in permutation.iter_mut().enumerate() {
        *destination = position;
    }
    if dimension == 0 {
        return Ok(RankRevealingCholeskyReport {
            rank: 0,
            maximum_dropped_relative_norm: 0.0,
            condition_proxy: 1.0,
            ambiguous: false,
        });
    }

    let initial_diagonal = (0..dimension)
        .map(|position| original_standardized_gram[position * dimension + position])
        .fold(0.0_f64, f64::max);
    if initial_diagonal == 0.0 {
        if original_standardized_gram.iter().any(|value| *value != 0.0) {
            return Err(invalid_data(
                "rank-revealing finite-WLS zero-diagonal Gram is not zero",
            ));
        }
        return Ok(RankRevealingCholeskyReport {
            rank: 0,
            maximum_dropped_relative_norm: 0.0,
            condition_proxy: 1.0,
            ambiguous: false,
        });
    }

    let negative_roundoff_guard = RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON * initial_diagonal;
    let relative_guard = RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON;
    let lower_rank_boundary = relative_threshold * (1.0 - relative_guard);
    let upper_rank_boundary = relative_threshold * (1.0 + relative_guard);
    let mut rank = 0_usize;
    let mut minimum_pivot = f64::INFINITY;
    let mut maximum_pivot = 0.0_f64;
    let mut maximum_dropped_relative_norm = 0.0_f64;
    let mut ambiguous = false;

    for step in 0..dimension {
        let mut pivot_position = step;
        let mut pivot_value = gram_scratch[step * dimension + step];
        let mut maximum_schur_diagonal = pivot_value;
        for candidate in step..dimension {
            let value = gram_scratch[candidate * dimension + candidate];
            if !value.is_finite() {
                return Err(invalid_data(
                    "rank-revealing finite-WLS Schur diagonal is not finite",
                ));
            }
            if value < -negative_roundoff_guard {
                return Err(invalid_data(
                    "rank-revealing finite-WLS Gram is materially indefinite",
                ));
            }
            maximum_schur_diagonal = maximum_schur_diagonal.max(value);
            if candidate != step
                && prefer_rank_pivot(
                    value,
                    canonical_generated_ids[permutation[candidate]],
                    pivot_value,
                    canonical_generated_ids[permutation[pivot_position]],
                )
            {
                pivot_position = candidate;
                pivot_value = value;
            }
        }

        let pivot_relative_norm = (pivot_value.max(0.0) / initial_diagonal).sqrt();
        let maximum_relative_norm = (maximum_schur_diagonal.max(0.0) / initial_diagonal).sqrt();
        if !pivot_relative_norm.is_finite() || !maximum_relative_norm.is_finite() {
            return Err(invalid_data(
                "rank-revealing finite-WLS relative Schur norm is not finite",
            ));
        }
        if (pivot_relative_norm >= lower_rank_boundary
            && pivot_relative_norm <= upper_rank_boundary)
            || (maximum_relative_norm >= lower_rank_boundary
                && maximum_relative_norm <= upper_rank_boundary)
        {
            ambiguous = true;
        }
        if maximum_relative_norm <= relative_threshold {
            maximum_dropped_relative_norm = maximum_relative_norm;
            break;
        }

        if pivot_position != step {
            swap_symmetric_block(gram_scratch, dimension, step, pivot_position);
            rhs_scratch.swap(step, pivot_position);
            permutation.swap(step, pivot_position);
            for column in 0..step {
                factor.swap(
                    step * dimension + column,
                    pivot_position * dimension + column,
                );
            }
        }
        let accepted_pivot = gram_scratch[step * dimension + step];
        let diagonal = accepted_pivot.sqrt();
        if !diagonal.is_finite() || diagonal <= 0.0 {
            return Err(invalid_data(
                "rank-revealing finite-WLS factor diagonal is invalid",
            ));
        }
        factor[step * dimension + step] = diagonal;
        minimum_pivot = minimum_pivot.min(accepted_pivot);
        maximum_pivot = maximum_pivot.max(accepted_pivot);
        for row in (step + 1)..dimension {
            let value = gram_scratch[row * dimension + step] / diagonal;
            if !value.is_finite() {
                return Err(invalid_data(
                    "rank-revealing finite-WLS factor entry is not finite",
                ));
            }
            factor[row * dimension + step] = value;
        }
        for row in (step + 1)..dimension {
            let exact_duplicate =
                exact_signed_gram_duplicate(gram_scratch, dimension, step, row, row);
            for column in row..dimension {
                let updated = if exact_duplicate {
                    0.0
                } else {
                    gram_scratch[column * dimension + row]
                        - factor[row * dimension + step] * factor[column * dimension + step]
                };
                if !updated.is_finite() {
                    return Err(invalid_data(
                        "rank-revealing finite-WLS Schur update is not finite",
                    ));
                }
                gram_scratch[column * dimension + row] = updated;
                gram_scratch[row * dimension + column] = updated;
            }
        }
        rank += 1;
    }

    let condition_proxy = if rank == 0 {
        1.0
    } else {
        maximum_pivot.sqrt() / minimum_pivot.sqrt()
    };
    if !condition_proxy.is_finite() || condition_proxy < 1.0 {
        return Err(invalid_data(
            "rank-revealing finite-WLS condition proxy is invalid",
        ));
    }
    Ok(RankRevealingCholeskyReport {
        rank,
        maximum_dropped_relative_norm,
        condition_proxy,
        ambiguous,
    })
}

/// Solve the accepted leading factor and publish only selected coefficients in
/// ascending canonical generated-column order. The factor retains its full
/// original row stride even when `rank` is smaller than the input dimension.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn solve_rank_revealing_cholesky_leading(
    factor: &[f64],
    pivoted_rhs: &[f64],
    permutation: &[usize],
    canonical_generated_ids: &[usize],
    rank: usize,
    forward: &mut [f64],
    canonical_selected_ids: &mut [usize],
    canonical_selected_solution: &mut [f64],
) -> Result<bool> {
    let dimension = permutation.len();
    if rank > dimension
        || factor.len()
            != checked_square(
                dimension,
                "rank-revealing finite-WLS solve dimensions overflow",
            )?
        || pivoted_rhs.len() != dimension
        || canonical_generated_ids.len() != dimension
        || forward.len() != rank
        || canonical_selected_ids.len() != rank
        || canonical_selected_solution.len() != rank
        || !valid_permutation(permutation)
        || !identities_are_unique(canonical_generated_ids)
    {
        return Err(invalid_argument(
            "rank-revealing finite-WLS solve dimensions are inconsistent",
        ));
    }
    forward.fill(0.0);
    canonical_selected_solution.fill(0.0);
    for row in 0..rank {
        let mut value = pivoted_rhs[row];
        for column in 0..row {
            value = (-factor[row * dimension + column]).mul_add(forward[column], value);
        }
        forward[row] = value / factor[row * dimension + row];
        if !forward[row].is_finite() {
            return Ok(false);
        }
    }
    for row in (0..rank).rev() {
        let mut value = forward[row];
        for column in (row + 1)..rank {
            value = (-factor[column * dimension + row])
                .mul_add(canonical_selected_solution[column], value);
        }
        canonical_selected_solution[row] = value / factor[row * dimension + row];
        if !canonical_selected_solution[row].is_finite() {
            return Ok(false);
        }
    }
    for position in 0..rank {
        canonical_selected_ids[position] = canonical_generated_ids[permutation[position]];
    }
    sort_selected_solution(canonical_selected_ids, canonical_selected_solution);
    Ok(true)
}

/// Invert the accepted leading factor into a compact `rank * rank` bread in
/// ascending canonical generated-column order. The input factor is addressed
/// with its full original row stride.
#[cfg(test)]
pub(super) fn invert_rank_revealing_cholesky_leading(
    factor: &[f64],
    permutation: &[usize],
    canonical_generated_ids: &[usize],
    rank: usize,
    inverse_lower: &mut [f64],
    canonical_selected_ids: &mut [usize],
    canonical_inverse: &mut [f64],
) -> Result<bool> {
    let dimension = permutation.len();
    let factor_square = checked_square(
        dimension,
        "rank-revealing finite-WLS inverse factor dimensions overflow",
    )?;
    let rank_square = checked_square(
        rank,
        "rank-revealing finite-WLS inverse dimensions overflow",
    )?;
    if rank > dimension
        || factor.len() != factor_square
        || canonical_generated_ids.len() != dimension
        || inverse_lower.len() != rank_square
        || canonical_selected_ids.len() != rank
        || canonical_inverse.len() != rank_square
        || !valid_permutation(permutation)
        || !identities_are_unique(canonical_generated_ids)
    {
        return Err(invalid_argument(
            "rank-revealing finite-WLS inverse dimensions are inconsistent",
        ));
    }
    inverse_lower.fill(0.0);
    canonical_inverse.fill(0.0);
    for column in 0..rank {
        for row in 0..rank {
            let mut value = f64::from(row == column);
            for inner in 0..row {
                value = (-factor[row * dimension + inner])
                    .mul_add(inverse_lower[inner * rank + column], value);
            }
            let solved = value / factor[row * dimension + row];
            if !solved.is_finite() {
                return Ok(false);
            }
            inverse_lower[row * rank + column] = solved;
        }
    }
    for left in 0..rank {
        for right in left..rank {
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..rank {
                compensated_add(
                    inverse_lower[inner * rank + left] * inverse_lower[inner * rank + right],
                    &mut sum,
                    &mut correction,
                )?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Ok(false);
            }
            canonical_inverse[left * rank + right] = value;
            canonical_inverse[right * rank + left] = value;
        }
    }
    for position in 0..rank {
        canonical_selected_ids[position] = canonical_generated_ids[permutation[position]];
    }
    sort_selected_inverse(canonical_selected_ids, canonical_inverse);
    Ok(true)
}

/// Solve one complete Cholesky system into caller-owned forward/solution
/// vectors. Numerical failure returns `Ok(false)` for policy-owned fallback.
pub(super) fn solve_guarded_cholesky(
    factor: &[f64],
    rhs: &[f64],
    forward: &mut [f64],
    solution: &mut [f64],
) -> Result<bool> {
    let dimension = rhs.len();
    if factor.len() != checked_square(dimension, "finite-WLS factor dimensions overflow")?
        || forward.len() != dimension
        || solution.len() != dimension
    {
        return Err(invalid_argument(
            "finite-WLS Cholesky solve dimensions are inconsistent",
        ));
    }
    forward.fill(0.0);
    solution.fill(0.0);
    for row in 0..dimension {
        let mut value = rhs[row];
        for column in 0..row {
            value = (-factor[row * dimension + column]).mul_add(forward[column], value);
        }
        forward[row] = value / factor[row * dimension + row];
        if !forward[row].is_finite() {
            return Ok(false);
        }
    }
    for row in (0..dimension).rev() {
        let mut value = forward[row];
        for column in (row + 1)..dimension {
            value = (-factor[column * dimension + row]).mul_add(solution[column], value);
        }
        solution[row] = value / factor[row * dimension + row];
        if !solution[row].is_finite() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Build the canonical inverse from a pivoted Cholesky factor without heap
/// allocation. This is the accepted one-shot arithmetic and is also used by
/// the reusable frozen-basis route.
pub(super) fn invert_guarded_cholesky(
    factor: &[f64],
    permutation: &[usize],
    inverse_lower: &mut [f64],
    canonical_inverse: &mut [f64],
) -> Result<bool> {
    let dimension = permutation.len();
    let square = checked_square(dimension, "finite-WLS inverse dimensions overflow")?;
    if factor.len() != square
        || inverse_lower.len() != square
        || canonical_inverse.len() != square
        || permutation.iter().any(|position| *position >= dimension)
    {
        return Err(invalid_argument(
            "finite-WLS inverse workspace dimensions are inconsistent",
        ));
    }
    inverse_lower.fill(0.0);
    canonical_inverse.fill(0.0);
    for column in 0..dimension {
        for row in 0..dimension {
            let mut value = f64::from(row == column);
            for inner in 0..row {
                value = (-factor[row * dimension + inner])
                    .mul_add(inverse_lower[inner * dimension + column], value);
            }
            let solved = value / factor[row * dimension + row];
            if !solved.is_finite() {
                return Ok(false);
            }
            inverse_lower[row * dimension + column] = solved;
        }
    }
    for left in 0..dimension {
        for right in left..dimension {
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..dimension {
                compensated_add(
                    inverse_lower[inner * dimension + left]
                        * inverse_lower[inner * dimension + right],
                    &mut sum,
                    &mut correction,
                )?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Ok(false);
            }
            let canonical_left = permutation[left];
            let canonical_right = permutation[right];
            canonical_inverse[canonical_left * dimension + canonical_right] = value;
            canonical_inverse[canonical_right * dimension + canonical_left] = value;
        }
    }
    Ok(true)
}

#[derive(Clone, Copy)]
pub(super) struct FiniteInverseCertificate {
    pub left: f64,
    pub right: f64,
}
impl FiniteInverseCertificate {
    pub fn passed(self, tolerance: f64) -> bool {
        self.left <= tolerance && self.right <= tolerance
    }
}

pub(super) fn certify_finite_inverse(
    gram: &[f64],
    dimension: usize,
    positions: &[usize],
    inverse: &[f64],
    rank: usize,
) -> Result<FiniteInverseCertificate> {
    Ok(FiniteInverseCertificate {
        left: inverse_relative_infinity_norm(gram, dimension, positions, inverse, rank, false)?,
        right: inverse_relative_infinity_norm(gram, dimension, positions, inverse, rank, true)?,
    })
}

pub(super) fn inverse_relative_infinity_norm(
    gram: &[f64],
    gram_dimension: usize,
    gram_positions: &[usize],
    inverse: &[f64],
    rank: usize,
    reverse: bool,
) -> Result<f64> {
    let gram_square = checked_square(gram_dimension, "finite-WLS bread Gram dimensions overflow")?;
    let rank_square = checked_square(rank, "finite-WLS bread dimensions overflow")?;
    if gram.len() != gram_square
        || gram_positions.len() != rank
        || inverse.len() != rank_square
        || gram_positions
            .iter()
            .any(|position| *position >= gram_dimension)
    {
        return Err(invalid_argument(
            "finite-WLS inverse-certificate dimensions are inconsistent",
        ));
    }
    let mut maximum = 0.0_f64;
    for row in 0..rank {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        for column in 0..rank {
            let mut product = 0.0_f64;
            let mut correction = 0.0_f64;
            let mut absolute_products = 0.0_f64;
            for inner in 0..rank {
                let (left, right) = if reverse {
                    (
                        inverse[row * rank + inner],
                        gram[gram_positions[inner] * gram_dimension + gram_positions[column]],
                    )
                } else {
                    (
                        gram[gram_positions[row] * gram_dimension + gram_positions[inner]],
                        inverse[inner * rank + column],
                    )
                };
                let term = left * right;
                compensated_add(term, &mut product, &mut correction)?;
                absolute_products += term.abs();
                if !absolute_products.is_finite() {
                    return Err(invalid_data(
                        "finite-WLS inverse-certificate scale is not finite",
                    ));
                }
            }
            row_error += (product + correction - f64::from(row == column)).abs();
            row_scale += absolute_products;
            if !row_error.is_finite() || !row_scale.is_finite() {
                return Err(invalid_data(
                    "finite-WLS inverse-certificate accumulation is not finite",
                ));
            }
        }
        maximum = maximum.max(row_error / row_scale);
    }
    if maximum.is_finite() {
        Ok(maximum)
    } else {
        Err(invalid_data("finite-WLS inverse certificate is not finite"))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum NormalEquationScale<'a> {
    Prepared {
        generated_columns: &'a [usize],
        generated_coefficients: &'a [Option<f64>],
        reference_norms: &'a [f64],
        score_norms: &'a [f64],
    },
    Frozen {
        coefficients: &'a [f64],
        residual_norm: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NormalEquationCertificate {
    pub(super) relative_infinity_norm: f64,
    pub(super) relative_two_norm: f64,
    pub(super) relative_square_sum: f64,
    pub(super) moments: usize,
}

pub(super) fn certify_normal_equations(
    gram: &[f64],
    rhs: &[f64],
    scale: NormalEquationScale<'_>,
) -> Result<NormalEquationCertificate> {
    let dimension = rhs.len();
    if gram.len() != checked_square(dimension, "finite-WLS score dimensions overflow")? {
        return Err(invalid_argument(
            "finite-WLS score-certificate dimensions are inconsistent",
        ));
    }
    if let NormalEquationScale::Prepared {
        generated_columns,
        generated_coefficients,
        reference_norms,
        score_norms,
    } = scale
    {
        if generated_columns.len() != dimension
            || generated_coefficients.len() != reference_norms.len()
            || score_norms.len() != reference_norms.len()
            || generated_columns
                .iter()
                .any(|column| *column >= reference_norms.len() || *column >= score_norms.len())
        {
            return Err(invalid_argument(
                "prepared finite-WLS score scales are inconsistent",
            ));
        }
    } else if let NormalEquationScale::Frozen { coefficients, .. } = scale {
        if coefficients.len() != dimension {
            return Err(invalid_argument(
                "frozen finite-WLS score coefficients are inconsistent",
            ));
        }
    }
    let mut infinity = 0.0_f64;
    let mut square_sum = 0.0_f64;
    for row in 0..dimension {
        let mut fitted = 0.0_f64;
        let mut correction = 0.0_f64;
        for column in 0..dimension {
            let coefficient = match scale {
                NormalEquationScale::Prepared {
                    generated_columns,
                    generated_coefficients,
                    reference_norms,
                    ..
                } => {
                    let generated = generated_columns[column];
                    generated_coefficients[generated]
                        .map(|value| value * reference_norms[generated])
                        .unwrap_or(0.0)
                }
                NormalEquationScale::Frozen { coefficients, .. } => coefficients[column],
            };
            if !coefficient.is_finite() {
                return Err(invalid_data(
                    "finite-WLS standardized coefficient is not finite",
                ));
            }
            compensated_add(
                gram[row * dimension + column] * coefficient,
                &mut fitted,
                &mut correction,
            )?;
        }
        let score = (rhs[row] - (fitted + correction)).abs();
        let relative = match scale {
            NormalEquationScale::Prepared {
                generated_columns,
                reference_norms,
                score_norms,
                ..
            } => {
                let generated = generated_columns[row];
                let reference = reference_norms[generated];
                let score_norm = score_norms[generated];
                if !reference.is_finite()
                    || reference <= 0.0
                    || !score_norm.is_finite()
                    || score_norm <= 0.0
                {
                    return Err(invalid_data(
                        "prepared finite-WLS score scale is not finite and positive",
                    ));
                }
                score * reference / score_norm
            }
            NormalEquationScale::Frozen { residual_norm, .. } => {
                if !residual_norm.is_finite() || residual_norm < 0.0 {
                    return Err(invalid_data("frozen finite-WLS residual norm is invalid"));
                }
                let column_norm = gram[row * dimension + row].max(0.0).sqrt();
                score / (column_norm * residual_norm).max(rhs[row].abs()).max(1.0)
            }
        };
        if !relative.is_finite() {
            return Err(invalid_data(
                "finite-WLS normal-equation certificate is not finite",
            ));
        }
        infinity = infinity.max(relative);
        square_sum = relative.mul_add(relative, square_sum);
        if !square_sum.is_finite() {
            return Err(invalid_data(
                "finite-WLS normal-equation two-norm is not finite",
            ));
        }
    }
    Ok(NormalEquationCertificate {
        relative_infinity_norm: infinity,
        relative_two_norm: square_sum.sqrt(),
        relative_square_sum: square_sum,
        moments: dimension,
    })
}

fn merge_fixed_block_partials(
    layout: FiniteWlsMomentLayout,
    partials: &[f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    let dimension = rhs.len();
    for position in 0..dimension {
        let mut sum = 0.0_f64;
        let mut correction = 0.0_f64;
        for partial in partials.chunks_exact(layout.partial_stride) {
            compensated_add(partial[position], &mut sum, &mut correction)?;
        }
        rhs[position] = sum + correction;
    }
    for left in 0..dimension {
        for right in left..dimension {
            let position = left * dimension + right;
            let partial_position = dimension + position;
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for partial in partials.chunks_exact(layout.partial_stride) {
                compensated_add(partial[partial_position], &mut sum, &mut correction)?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Err(invalid_data("finite-WLS Gram entry is not finite"));
            }
            gram[position] = value;
            gram[right * dimension + left] = value;
        }
    }
    Ok(())
}

fn fill_retained_partitioned(
    source: FixedBlockMomentSource<'_>,
    layout: FiniteWlsMomentLayout,
    partials: &mut [f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    match source {
        FixedBlockMomentSource::Dense {
            values,
            stored_columns,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
            ..
        } => fill_retained_dense_partitioned(
            values,
            stored_columns,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
            layout,
            rhs,
            gram,
        ),
        FixedBlockMomentSource::Scratch {
            values,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
        } => fill_retained_scratch_partitioned(
            values,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
            layout,
            partials,
            rhs,
            gram,
        ),
        FixedBlockMomentSource::Implicit {
            values,
            selected_columns,
            response,
            weights,
            solver_weight_divisor,
            column_scales,
        } => {
            let bound = values.bind_columns()?;
            let dimension = rhs.len();
            crate::parallel::stage_try_for_each(
                gram.par_chunks_mut(dimension * layout.columns_per_partition)
                    .zip(rhs.par_chunks_mut(layout.columns_per_partition))
                    .zip(partials.par_chunks_mut(layout.partial_stride))
                    .enumerate(),
                moment_lane_work(values.rows(), dimension),
                |(part, ((gram_rows, rhs_rows), row))| -> Result<()> {
                    let first = part * layout.columns_per_partition;
                    let fill = |rhs: &mut [f64], gram: &mut [f64], row: &mut [f64]| -> Result<()> {
                        // Keep the legacy matrix indices and numerical traversal order explicit.
                        #[allow(clippy::needless_range_loop)]
                        for observation in 0..values.rows() {
                            let group = values.codes()[observation] as usize;
                            let weight =
                                normalized_weight(weights, solver_weight_divisor, observation)?;
                            for j in first..dimension {
                                let column = selected_columns.map_or(j, |ids| ids[j]);
                                row[j] = (bound[column].value(observation)?
                                    - values.center(column, group))
                                    / column_scales.map_or(1.0, |scales| scales[j]);
                            }
                            for (local, target) in rhs.iter_mut().enumerate() {
                                let j = first + local;
                                let left = weight * row[j];
                                *target = left.mul_add(response[observation], *target);
                                for (entry, &right) in gram
                                    [local * dimension + j..(local + 1) * dimension]
                                    .iter_mut()
                                    .zip(&row[j..dimension])
                                {
                                    *entry = left.mul_add(right, *entry);
                                }
                            }
                        }
                        Ok(())
                    };
                    if dimension <= FIXED_BLOCK_MOMENT_MAX_COLUMNS {
                        with_private_retained_moments(rhs_rows, gram_rows, fill)
                    } else {
                        fill(rhs_rows, gram_rows, row)
                    }
                },
            )
        }
    }?;
    let dimension = rhs.len();
    for left in 0..dimension {
        for right in (left + 1)..dimension {
            gram[right * dimension + left] = gram[left * dimension + right];
        }
    }
    if rhs.iter().chain(gram.iter()).all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(invalid_data("finite-WLS retained moments are not finite"))
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_retained_dense_partitioned(
    values: FiniteDenseRows<'_>,
    stored_columns: usize,
    selected_columns: Option<&[usize]>,
    response: FiniteResponse<'_>,
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    scales: Option<&[f64]>,
    layout: FiniteWlsMomentLayout,
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    let dimension = rhs.len();
    if let Some(scales) = scales {
        if dimension <= FIXED_BLOCK_MOMENT_MAX_COLUMNS {
            return fill_retained_dense_standardized_rows(
                values,
                stored_columns,
                selected_columns,
                response,
                weights,
                divisor,
                scales,
                layout,
                rhs,
                gram,
            );
        }
        if layout.columns_per_partition > 1 {
            return fill_retained_dense_standardized_tiles(
                values,
                stored_columns,
                selected_columns,
                response,
                weights,
                divisor,
                scales,
                layout,
                rhs,
                gram,
            );
        }
    }
    crate::parallel::stage_try_for_each(
        rhs.par_chunks_mut(layout.columns_per_partition)
            .zip(gram.par_chunks_mut(layout.columns_per_partition * dimension))
            .enumerate(),
        moment_lane_work(response.len(), dimension),
        |(partition, (rhs_rows, gram_rows))| -> Result<()> {
            record_retained_moment_worker();
            let left_start = partition * layout.columns_per_partition;
            for row in 0..response.len() {
                let source = values.row(row);
                let weight = normalized_weight(weights, divisor, row)?;
                for (local_left, destination) in rhs_rows.iter_mut().enumerate() {
                    let left = left_start + local_left;
                    let source_left = selected_columns.map_or(left, |selected| selected[left]);
                    let left_value = scaled_column(source[source_left], scales, left)?;
                    let weighted_left = weight * left_value;
                    *destination = weighted_left.mul_add(response[row], *destination);
                    let gram_row =
                        &mut gram_rows[local_left * dimension..(local_left + 1) * dimension];
                    for right in left..dimension {
                        let source_right =
                            selected_columns.map_or(right, |selected| selected[right]);
                        let right_value = scaled_column(source[source_right], scales, right)?;
                        gram_row[right] = weighted_left.mul_add(right_value, gram_row[right]);
                    }
                }
            }
            Ok(())
        },
    )
}

// Isolate the bounded stack frame from wide/scratch calls. The closure is
// statically dispatched once per partition, never through a row-loop callback.
#[inline(never)]
fn with_private_retained_moments(
    rhs: &mut [f64],
    gram: &mut [f64],
    fill: impl FnOnce(&mut [f64], &mut [f64], &mut [f64]) -> Result<()>,
) -> Result<()> {
    let mut row = [0.0_f64; FIXED_BLOCK_MOMENT_MAX_COLUMNS];
    let mut local_rhs = [0.0_f64; FIXED_BLOCK_MOMENT_MAX_COLUMNS];
    let mut local_gram = [0.0_f64; FIXED_BLOCK_MOMENT_MAX_COLUMNS * FIXED_BLOCK_MOMENT_MAX_COLUMNS];
    // Keep both the moments and row scratch private. Disjoint narrow slices of
    // either output or scratch banks can share cache lines across workers.
    let rhs_accumulators = &mut local_rhs[..rhs.len()];
    let gram_accumulators = &mut local_gram[..gram.len()];
    fill(rhs_accumulators, gram_accumulators, &mut row)?;
    rhs.copy_from_slice(rhs_accumulators);
    gram.copy_from_slice(gram_accumulators);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn fill_retained_dense_standardized_rows(
    values: FiniteDenseRows<'_>,
    _stored_columns: usize,
    selected_columns: Option<&[usize]>,
    response: FiniteResponse<'_>,
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    scales: &[f64],
    layout: FiniteWlsMomentLayout,
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    let dimension = rhs.len();
    crate::parallel::stage_try_for_each(
        rhs.par_chunks_mut(layout.columns_per_partition)
            .zip(gram.par_chunks_mut(layout.columns_per_partition * dimension))
            .enumerate(),
        moment_lane_work(response.len(), dimension),
        |(partition, (rhs_rows, gram_rows))| -> Result<()> {
            record_retained_moment_worker();
            let left_start = partition * layout.columns_per_partition;
            with_private_retained_moments(rhs_rows, gram_rows, |rhs, gram, standardized| {
                for (row, &response_value) in response.iter().enumerate() {
                    let source = values.row(row);
                    let weight = normalized_weight(weights, divisor, row)?;
                    for column in left_start..dimension {
                        let source_column =
                            selected_columns.map_or(column, |selected| selected[column]);
                        standardized[column] =
                            scaled_column(source[source_column], Some(scales), column)?;
                    }
                    for (local_left, destination) in rhs.iter_mut().enumerate() {
                        let left = left_start + local_left;
                        let weighted_left = weight * standardized[left];
                        *destination = weighted_left.mul_add(response_value, *destination);
                        let gram_row =
                            &mut gram[local_left * dimension..(local_left + 1) * dimension];
                        for (entry, &right_value) in gram_row[left..]
                            .iter_mut()
                            .zip(&standardized[left..dimension])
                        {
                            *entry = weighted_left.mul_add(right_value, *entry);
                        }
                    }
                }
                Ok(())
            })
        },
    )
}

// Two fixed column tiles bound local storage at every width. Each Gram entry
// receives exactly one update per observation, in the original row/FMA order.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn fill_retained_dense_standardized_tiles(
    values: FiniteDenseRows<'_>,
    _stored_columns: usize,
    selected_columns: Option<&[usize]>,
    response: FiniteResponse<'_>,
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    scales: &[f64],
    layout: FiniteWlsMomentLayout,
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    let dimension = rhs.len();
    crate::parallel::stage_try_for_each(
        rhs.par_chunks_mut(layout.columns_per_partition)
            .zip(gram.par_chunks_mut(layout.columns_per_partition * dimension))
            .enumerate(),
        moment_lane_work(response.len(), dimension),
        |(partition, (rhs_rows, gram_rows))| -> Result<()> {
            record_retained_moment_worker();
            let left_start = partition * layout.columns_per_partition;
            let mut weighted_left_tile = [0.0_f64; FIXED_BLOCK_MOMENT_MAX_COLUMNS];
            let mut right_tile = [0.0_f64; FIXED_BLOCK_MOMENT_MAX_COLUMNS];
            for (row, &response_value) in response.iter().enumerate() {
                let source = values.row(row);
                let weight = normalized_weight(weights, divisor, row)?;
                for local_start in (0..rhs_rows.len()).step_by(FIXED_BLOCK_MOMENT_MAX_COLUMNS) {
                    let left_base = left_start + local_start;
                    let left_count =
                        (rhs_rows.len() - local_start).min(FIXED_BLOCK_MOMENT_MAX_COLUMNS);
                    for (offset, weighted_left) in
                        weighted_left_tile[..left_count].iter_mut().enumerate()
                    {
                        let left = left_base + offset;
                        let source_left = selected_columns.map_or(left, |selected| selected[left]);
                        *weighted_left =
                            weight * scaled_column(source[source_left], Some(scales), left)?;
                        let destination = &mut rhs_rows[local_start + offset];
                        *destination = weighted_left.mul_add(response_value, *destination);
                    }
                    for right_start in
                        (left_base..dimension).step_by(FIXED_BLOCK_MOMENT_MAX_COLUMNS)
                    {
                        let right_count =
                            (dimension - right_start).min(FIXED_BLOCK_MOMENT_MAX_COLUMNS);
                        for (offset, value) in right_tile[..right_count].iter_mut().enumerate() {
                            let right = right_start + offset;
                            let source_right =
                                selected_columns.map_or(right, |selected| selected[right]);
                            *value = scaled_column(source[source_right], Some(scales), right)?;
                        }
                        for (offset, &weighted_left) in
                            weighted_left_tile[..left_count].iter().enumerate()
                        {
                            let left = left_base + offset;
                            let first_right = left.max(right_start);
                            let gram_row = &mut gram_rows[(local_start + offset) * dimension
                                ..(local_start + offset + 1) * dimension];
                            for (entry, &right_value) in gram_row
                                [first_right..right_start + right_count]
                                .iter_mut()
                                .zip(&right_tile[first_right - right_start..right_count])
                            {
                                *entry = weighted_left.mul_add(right_value, *entry);
                            }
                        }
                    }
                }
            }
            Ok(())
        },
    )
}

// Repartition only the already admitted row-tile bank. Each observation is
// read and standardized once, then contributes to one private complete Gram.
// Refuse when the bank or the conservative multiply-add work bound cannot
// support a shorter parallel critical path than disjoint Gram rows.
fn scratch_row_partition_plan(
    rows: usize,
    stored: usize,
    dimension: usize,
    layout: FiniteWlsMomentLayout,
) -> Option<(usize, usize)> {
    if dimension == 0
        || stored < dimension
        || rows == 0
        || layout.columns_per_partition == 0
        || layout.columns_per_partition > dimension
    {
        return None;
    }
    let square = dimension.checked_mul(dimension)?;
    let stride = square.checked_add(dimension)?.checked_add(stored)?;
    let partitions = (layout.partial_elements / stride)
        .min(layout.partitions)
        .min(rows);
    if partitions < 2 {
        return None;
    }
    let d = dimension as u128;
    let left = layout.columns_per_partition as u128;
    let total_work = d * (d + 1) / 2 + d;
    let old_critical_work = left * (2 * d - left + 1) / 2 + left;
    if rows.div_ceil(partitions) as u128 * total_work >= rows as u128 * old_critical_work {
        return None;
    }
    Some((partitions, stride))
}

#[allow(clippy::too_many_arguments)]
fn fill_retained_scratch_row_partitions(
    values: &PreparedOlsScratch,
    selected: Option<&[usize]>,
    response: FiniteResponse<'_>,
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    scales: Option<&[f64]>,
    layout: FiniteWlsMomentLayout,
    partials: &mut [f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<bool> {
    let dimension = rhs.len();
    let Some((partitions, stride)) =
        scratch_row_partition_plan(response.len(), values.columns(), dimension, layout)
    else {
        return Ok(false);
    };
    let used = &mut partials[..partitions * stride];
    crate::parallel::stage_try_for_each(
        used.par_chunks_mut(stride).enumerate(),
        moment_lane_work(response.len(), dimension),
        |(part, bank)| -> Result<()> {
            record_retained_moment_worker();
            let (out_rhs, rest) = bank.split_at_mut(dimension);
            let (out_gram, row) = rest.split_at_mut(dimension * dimension);
            let length = response.len() / partitions;
            let tail = response.len() % partitions;
            let start = part * length + part.min(tail);
            let end = start + length + usize::from(part < tail);
            let mut reader = values.shared_reader();
            // Keep the legacy matrix indices and numerical traversal order explicit.
            #[allow(clippy::needless_range_loop)]
            for observation in start..end {
                reader.read_linear(observation * values.columns(), row)?;
                let weight = normalized_weight(weights, divisor, observation)?;
                // Selection is increasing, so forward compaction cannot overwrite
                // a source that a later selected coordinate still needs.
                for j in 0..dimension {
                    row[j] = scaled_column(row[selected.map_or(j, |ids| ids[j])], scales, j)?;
                }
                for j in 0..dimension {
                    let left = weight * row[j];
                    out_rhs[j] = left.mul_add(response[observation], out_rhs[j]);
                    for (entry, &right) in out_gram[j * dimension + j..(j + 1) * dimension]
                        .iter_mut()
                        .zip(&row[j..dimension])
                    {
                        *entry = left.mul_add(right, *entry);
                    }
                }
            }
            if out_rhs.iter().chain(out_gram.iter()).all(|v| v.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "finite-WLS row-partition moment is not finite",
                ))
            }
        },
    )?;
    merge_fixed_block_partials(
        FiniteWlsMomentLayout {
            partitions,
            partial_stride: stride,
            partial_elements: used.len(),
            ..layout
        },
        used,
        rhs,
        gram,
    )?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn fill_retained_scratch_partitioned(
    values: &PreparedOlsScratch,
    selected_columns: Option<&[usize]>,
    response: FiniteResponse<'_>,
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    scales: Option<&[f64]>,
    layout: FiniteWlsMomentLayout,
    partials: &mut [f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    if fill_retained_scratch_row_partitions(
        values,
        selected_columns,
        response,
        weights,
        divisor,
        scales,
        layout,
        partials,
        rhs,
        gram,
    )? {
        return Ok(());
    }
    let dimension = rhs.len();
    let stored_columns = values.columns();
    crate::parallel::stage_try_for_each(
        partials
            .par_chunks_mut(layout.partial_stride)
            .zip(rhs.par_chunks_mut(layout.columns_per_partition))
            .zip(gram.par_chunks_mut(layout.columns_per_partition * dimension))
            .enumerate(),
        moment_lane_work(response.len(), dimension),
        |(partition, ((row_workspace, rhs_rows), gram_rows))| -> Result<()> {
            record_retained_moment_worker();
            let left_start = partition * layout.columns_per_partition;
            let tile_rows = row_workspace.len() / stored_columns;
            for start_row in (0..response.len()).step_by(tile_rows) {
                let rows = (response.len() - start_row).min(tile_rows);
                let tile = &mut row_workspace[..rows * stored_columns];
                values.read_linear_shared(start_row * stored_columns, tile)?;
                // Preserve every observation and FMA in canonical order;
                // only the positional I/O granularity changes.
                for (local_row, stored_row) in tile.chunks_exact_mut(stored_columns).enumerate() {
                    let row = start_row + local_row;
                    let response_value = response[row];
                    let weight = normalized_weight(weights, divisor, row)?;
                    // The validated selection is strictly increasing, so
                    // selected[j] >= j. Compact and standardize forward
                    // in this private tile without overwriting a later
                    // source. Every Gram coordinate keeps its original
                    // row/FMA order; scratch bytes are never changed.
                    for column in left_start..dimension {
                        let source = selected_columns.map_or(column, |selected| selected[column]);
                        stored_row[column] = scaled_column(stored_row[source], scales, column)?;
                    }
                    for (local_left, destination) in rhs_rows.iter_mut().enumerate() {
                        let left = left_start + local_left;
                        let weighted_left = weight * stored_row[left];
                        *destination = weighted_left.mul_add(response_value, *destination);
                        let gram_row =
                            &mut gram_rows[local_left * dimension..(local_left + 1) * dimension];
                        for (entry, &right_value) in gram_row[left..]
                            .iter_mut()
                            .zip(&stored_row[left..dimension])
                        {
                            *entry = weighted_left.mul_add(right_value, *entry);
                        }
                    }
                }
            }
            Ok(())
        },
    )
}

#[cfg(test)]
static RETAINED_MOMENT_OBSERVER_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static RETAINED_MOMENT_OBSERVED_WIDTH: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static RETAINED_MOMENT_OBSERVED_WORKERS: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(super) fn begin_retained_moment_worker_observation() {
    RETAINED_MOMENT_OBSERVED_WIDTH.store(0, Ordering::SeqCst);
    RETAINED_MOMENT_OBSERVED_WORKERS.store(0, Ordering::SeqCst);
    RETAINED_MOMENT_OBSERVER_ENABLED.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub(super) fn finish_retained_moment_worker_observation() -> (usize, u64) {
    RETAINED_MOMENT_OBSERVER_ENABLED.store(false, Ordering::SeqCst);
    (
        RETAINED_MOMENT_OBSERVED_WIDTH.load(Ordering::SeqCst),
        RETAINED_MOMENT_OBSERVED_WORKERS.load(Ordering::SeqCst),
    )
}

#[inline]
fn record_retained_moment_worker() {
    #[cfg(test)]
    if RETAINED_MOMENT_OBSERVER_ENABLED.load(Ordering::SeqCst) {
        RETAINED_MOMENT_OBSERVED_WIDTH.fetch_max(rayon::current_num_threads(), Ordering::SeqCst);
        if let Some(worker) = rayon::current_thread_index() {
            let bit = 1_u64.checked_shl(worker as u32).unwrap_or(0);
            if bit != 0 {
                RETAINED_MOMENT_OBSERVED_WORKERS.fetch_or(bit, Ordering::SeqCst);
            }
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn validate_retained_source(
    elements: usize,
    rows: usize,
    stored_columns: usize,
    selected_columns: Option<&[usize]>,
    response: FiniteResponse<'_>,
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    scales: Option<&[f64]>,
) -> Result<()> {
    let dimension = selected_columns.map_or(stored_columns, <[usize]>::len);
    if rows == 0
        || stored_columns == 0
        || dimension == 0
        || elements != rows.saturating_mul(stored_columns)
        || response.len() != rows
        || response.iter().any(|value| !value.is_finite())
        || !divisor.is_finite()
        || divisor <= 0.0
        || selected_columns.is_some_and(|selected| {
            selected.len() != dimension
                || selected.iter().any(|column| *column >= stored_columns)
                || selected.windows(2).any(|pair| pair[0] >= pair[1])
        })
        || scales.is_some_and(|values| {
            values.len() != dimension
                || values
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
        })
        || match weights {
            BorrowedMapWeights::Unit => divisor != 1.0,
            BorrowedMapWeights::Provided(values) => {
                values.len() != rows
                    || values
                        .iter()
                        .any(|value| !value.is_finite() || *value <= 0.0 || *value / divisor == 0.0)
            }
        }
    {
        return Err(invalid_argument(
            "retained finite-WLS moment source is inconsistent",
        ));
    }
    Ok(())
}

fn normalized_weight(weights: BorrowedMapWeights<'_>, divisor: f64, row: usize) -> Result<f64> {
    let value = match weights {
        BorrowedMapWeights::Unit => 1.0,
        BorrowedMapWeights::Provided(values) => {
            values
                .get(row)
                .copied()
                .ok_or_else(|| invalid_argument("finite-WLS weight row is out of range"))?
                / divisor
        }
    };
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid_data(
            "finite-WLS normalized weight is not finite and positive",
        ))
    }
}

fn scaled_column(value: f64, scales: Option<&[f64]>, column: usize) -> Result<f64> {
    let value = scales.map_or(value, |scales| value / scales[column]);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid_data("finite-WLS standardized column is not finite"))
    }
}

#[inline]
#[cfg(test)]
fn prefer_rank_pivot(
    candidate_value: f64,
    candidate_generated_id: usize,
    current_value: f64,
    current_generated_id: usize,
) -> bool {
    let candidate_norm = candidate_value.max(0.0).sqrt();
    let current_norm = current_value.max(0.0).sqrt();
    let scale = candidate_norm.max(current_norm);
    let tied = candidate_norm == current_norm
        || (candidate_norm - current_norm).abs()
            <= RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON * scale;
    if tied {
        candidate_generated_id < current_generated_id
    } else {
        candidate_norm > current_norm
    }
}

fn identities_are_unique(identities: &[usize]) -> bool {
    identities
        .iter()
        .enumerate()
        .all(|(position, identity)| !identities[..position].contains(identity))
}

fn valid_permutation(permutation: &[usize]) -> bool {
    permutation.iter().enumerate().all(|(position, value)| {
        *value < permutation.len() && !permutation[..position].contains(value)
    })
}

fn sort_selected_solution(generated_ids: &mut [usize], solution: &mut [f64]) {
    for current in 1..generated_ids.len() {
        let mut position = current;
        while position > 0 && generated_ids[position] < generated_ids[position - 1] {
            generated_ids.swap(position, position - 1);
            solution.swap(position, position - 1);
            position -= 1;
        }
    }
}

fn sort_selected_inverse(generated_ids: &mut [usize], inverse: &mut [f64]) {
    let rank = generated_ids.len();
    for current in 1..rank {
        let mut position = current;
        while position > 0 && generated_ids[position] < generated_ids[position - 1] {
            generated_ids.swap(position, position - 1);
            swap_symmetric_block(inverse, rank, position, position - 1);
            position -= 1;
        }
    }
}

fn swap_symmetric_block(matrix: &mut [f64], dimension: usize, left: usize, right: usize) {
    if left == right {
        return;
    }
    for column in 0..dimension {
        matrix.swap(left * dimension + column, right * dimension + column);
    }
    for row in 0..dimension {
        matrix.swap(row * dimension + left, row * dimension + right);
    }
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "finite-WLS compensated accumulator is not finite",
        ))
    }
}

fn checked_square(value: usize, message: &'static str) -> Result<usize> {
    value.checked_mul(value).ok_or_else(|| allocation(message))
}

#[cfg(test)]
mod tests {
    use crate::error::ErrorCode;
    #[test]
    fn retained_arithmetic_leaves_survive_serial_stage_admission() {
        rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| {
                let layout = super::FiniteWlsMomentLayout::plan(2, 12, 4).unwrap();
                assert_eq!(layout.partitions, 2);
                assert_eq!(
                    crate::parallel::StagePlan::current(
                        super::moment_lane_work(6, 2),
                        layout.partitions,
                    )
                    .lanes,
                    1,
                );
                assert!(super::scratch_row_partition_plan(6, 2, 2, layout).is_some());
            });
    }

    #[test]
    fn extreme_schedule_shapes_fail_capacity_without_panicking() {
        let error = super::FiniteWlsMomentLayout::plan(usize::MAX, usize::MAX, 16)
            .expect_err("unaddressable scratch must fail checked capacity");
        assert_eq!(error.code, crate::ErrorCode::Allocation);
    }

    use super::*;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "actual {actual:.17e}, expected {expected:.17e}, tolerance {tolerance:.3e}"
        );
    }

    fn assert_reordered_moments(
        rhs: &[f64],
        gram: &[f64],
        expected_rhs: &[f64],
        expected_gram: &[f64],
        rows: usize,
        response_norm: f64,
    ) {
        let k = rhs.len();
        // Both scalar and partitioned FMA sums have forward error bounded by
        // gamma_N times the absolute products; Cauchy bounds those products.
        // Four N*epsilon covers both sums and the compensated partial merge.
        let bound = 4. * rows as f64 * f64::EPSILON;
        for j in 0..k {
            let norm = expected_gram[j * k + j].sqrt();
            assert_close(rhs[j], expected_rhs[j], bound * norm * response_norm);
            for q in 0..k {
                assert_close(
                    gram[j * k + q],
                    expected_gram[j * k + q],
                    bound * norm * expected_gram[q * k + q].sqrt(),
                );
            }
        }
    }

    #[test]
    fn scratch_row_partition_admission_bounds_storage_and_critical_work() {
        for dimension in [1_usize, 7, 16, 28, 32, 33, 63, 64, 65, 128, 257] {
            for threads in [1_usize, 4, 8, 16] {
                let width = dimension.div_ceil(threads.min(dimension));
                let parts = dimension.div_ceil(width);
                let layout = FiniteWlsMomentLayout {
                    partitions: parts,
                    columns_per_partition: width,
                    partial_stride: 1024,
                    partial_elements: parts * 1024,
                };
                if let Some((count, stride)) =
                    scratch_row_partition_plan(16_385, dimension, dimension, layout)
                {
                    assert!(count >= 2 && count <= parts);
                    assert!(count * stride <= layout.partial_elements);
                    assert_eq!(stride, dimension * dimension + 2 * dimension);
                    assert!(
                        16_385_usize.div_ceil(count)
                            * (dimension * (dimension + 1) / 2 + dimension)
                            < 16_385 * (width * (2 * dimension - width + 1) / 2 + width)
                    );
                } else if dimension == 32 && threads == 8 {
                    panic!("the representative wide scratch bank admits seven complete partials");
                }
            }
        }
    }

    #[test]
    fn scratch_moment_tiles_match_scalar_oracle_and_bound_read_chunks() {
        for (stored, columns, rows) in [
            (1, 1, 2051),
            (5, 4, 419),
            (18, 8, 131),
            (129, 7, 23),
            (1025, 4, 3),
            (18, 8, 65_536),
            (18, 8, 131_072),
        ] {
            let selected = (0..columns)
                .map(|c| c * (stored - 1) / columns.max(1))
                .collect::<Vec<_>>();
            let scales = (0..columns)
                .map(|c| 2.0_f64.powi(c as i32 - 3))
                .collect::<Vec<_>>();
            let mut data = vec![0.0; rows * stored];
            for row in 0..rows {
                for c in 0..columns {
                    data[row * stored + selected[c]] =
                        (((row * 17 + c * 13) % 31) as f64 - 15.0) / 16.0 * scales[c];
                }
            }
            let response = (0..rows)
                .map(|row| ((row % 17) as f64 - 8.0) / 8.0)
                .collect::<Vec<_>>();
            let weights = (0..rows)
                .map(|row| 2.0_f64.powi((row % 5) as i32 * 100 - 200))
                .collect::<Vec<_>>();
            let mut scratch = PreparedOlsScratch::create_row_major(rows, stored).unwrap();
            scratch.write_linear(0, &data).unwrap();
            for provided in [false, true] {
                let divisor = if provided { 2.0_f64.powi(202) } else { 1.0 };
                let mut expected_rhs = vec![0.0; columns];
                let mut expected_gram = vec![0.0; columns * columns];
                for left in 0..columns {
                    for row in 0..rows {
                        let weight = if provided {
                            weights[row] / divisor
                        } else {
                            1.0
                        };
                        let left_value = data[row * stored + selected[left]] / scales[left];
                        let weighted_left = weight * left_value;
                        expected_rhs[left] =
                            weighted_left.mul_add(response[row], expected_rhs[left]);
                        for right in left..columns {
                            let position = left * columns + right;
                            expected_gram[position] = weighted_left.mul_add(
                                data[row * stored + selected[right]] / scales[right],
                                expected_gram[position],
                            );
                        }
                    }
                    for right in left..columns {
                        expected_gram[right * columns + left] =
                            expected_gram[left * columns + right];
                    }
                }
                for threads in [1, 4] {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| {
                            let source = FixedBlockMomentSource::Scratch {
                                values: &scratch,
                                selected_columns: Some(&selected),
                                response: (&response).into(),
                                weights: if provided {
                                    BorrowedMapWeights::Provided(&weights)
                                } else {
                                    BorrowedMapWeights::Unit
                                },
                                solver_weight_divisor: divisor,
                                column_scales: Some(&scales),
                            };
                            let layout = finite_wls_moment_layout(source).unwrap();
                            let mut partials = vec![0.0; layout.partial_elements];
                            let mut rhs = vec![0.0; columns];
                            let mut gram = vec![0.0; columns * columns];
                            let before = scratch.shared_read_chunks_for_test();
                            fill_finite_wls_moments(
                                source,
                                layout,
                                &mut partials,
                                &mut rhs,
                                &mut gram,
                            )
                            .unwrap();
                            let row_plan =
                                scratch_row_partition_plan(rows, stored, columns, layout);
                            if row_plan.is_some() {
                                let response_norm = response
                                    .iter()
                                    .enumerate()
                                    .map(|(i, y)| {
                                        (if provided { weights[i] / divisor } else { 1. }) * y * y
                                    })
                                    .sum::<f64>()
                                    .sqrt();
                                assert_reordered_moments(
                                    &rhs,
                                    &gram,
                                    &expected_rhs,
                                    &expected_gram,
                                    rows,
                                    response_norm,
                                );
                            } else {
                                assert_eq!(rhs, expected_rhs);
                                assert_eq!(gram, expected_gram);
                            }
                            let tile_rows = layout.partial_stride / stored;
                            let chunks_per_tile = layout.partial_stride.div_ceil(1024);
                            let full_tiles = rows / tile_rows;
                            let tail = rows % tile_rows;
                            let expected_chunks = if let Some((parts, _)) = row_plan {
                                (0..parts)
                                    .map(|part| {
                                        let start = part * (rows / parts) + part.min(rows % parts);
                                        let end =
                                            start + rows / parts + usize::from(part < rows % parts);
                                        (end * stored).div_ceil(1024) - start * stored / 1024
                                    })
                                    .sum()
                            } else {
                                layout.partitions
                                    * (full_tiles * chunks_per_tile
                                        + (tail * stored).div_ceil(1024))
                            };
                            assert_eq!(
                                scratch.shared_read_chunks_for_test() - before,
                                expected_chunks as u64
                            );
                            let bound = threads.min(stored)
                                * (rows * stored).min(stored.max(RETAINED_MOMENT_TILE_ELEMENTS));
                            assert!(layout.partial_elements <= bound);
                            if stored == 18 && rows >= 65_536 {
                                assert!(expected_chunks * 50 < layout.partitions * rows);
                            }
                        });
                }
            }
        }
    }

    #[test]
    fn retained_dense_moments_preserve_scalar_order_across_cache_boundaries() {
        for (columns, spaced, provided, scaled) in [
            (1, false, false, true),
            (6, false, true, true),
            (7, true, true, true),
            (16, false, false, true),
            (63, true, true, true),
            (64, false, true, true),
            (65, true, true, true),
            (128, false, false, true),
            (129, true, true, true),
            (257, true, true, true),
            (7, false, false, false),
            (64, true, true, false),
        ] {
            let rows = 37;
            let selected = (0..columns)
                .map(|column| if spaced { 2 * column } else { column })
                .collect::<Vec<_>>();
            let stored = selected[columns - 1] + 1;
            let scales = (0..columns)
                .map(|column| {
                    if scaled {
                        2.0_f64.powi((column % 3) as i32 * 400 - 400)
                    } else {
                        1.0
                    }
                })
                .collect::<Vec<_>>();
            let weights = (0..rows)
                .map(|row| 2.0_f64.powi((row % 5) as i32 * 100 - 200))
                .collect::<Vec<_>>();
            let divisor = if provided { 2.0_f64.powi(202) } else { 1.0 };
            let response = (0..rows)
                .map(|row| (row as f64 * 0.37).cos())
                .collect::<Vec<_>>();
            let mut values = vec![f64::NAN; rows * stored];
            for row in 0..rows {
                for column in 0..columns {
                    values[row * stored + selected[column]] =
                        (((row * 17 + column * 13) % 31) as f64 - 15.0) / 16.0 * scales[column];
                }
            }
            let source = FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::packed(&values, stored),
                rows,
                stored_columns: stored,
                selected_columns: if spaced { Some(&selected) } else { None },
                response: (&response).into(),
                weights: if provided {
                    BorrowedMapWeights::Provided(&weights)
                } else {
                    BorrowedMapWeights::Unit
                },
                solver_weight_divisor: divisor,
                column_scales: if scaled { Some(&scales) } else { None },
            };
            let mut expected_rhs = vec![0.0_f64; columns];
            let mut expected_gram = vec![0.0_f64; columns * columns];
            for left in 0..columns {
                for row in 0..rows {
                    let weight = if provided {
                        weights[row] / divisor
                    } else {
                        1.0
                    };
                    let left_value = values[row * stored + selected[left]] / scales[left];
                    let weighted_left = weight * left_value;
                    expected_rhs[left] = weighted_left.mul_add(response[row], expected_rhs[left]);
                    for right in left..columns {
                        let right_value = values[row * stored + selected[right]] / scales[right];
                        let position = left * columns + right;
                        expected_gram[position] =
                            weighted_left.mul_add(right_value, expected_gram[position]);
                    }
                }
                for right in left..columns {
                    expected_gram[right * columns + left] = expected_gram[left * columns + right];
                }
            }
            for threads in [1, 2, 4, 16] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let layout = finite_wls_moment_layout(source).unwrap();
                        assert_eq!(layout.partial_elements, 0);
                        let mut rhs = vec![0.0; columns];
                        let mut gram = vec![0.0; columns * columns];
                        fill_finite_wls_moments(source, layout, &mut [], &mut rhs, &mut gram)
                            .unwrap();
                        assert_eq!(rhs, expected_rhs);
                        assert_eq!(gram, expected_gram);
                    });
            }
            {
                let mut scratch = PreparedOlsScratch::create_row_major(rows, stored).unwrap();
                // Scratch validates every stored cell; only the dense source
                // can use nonfinite unselected cells to detect accidental reads.
                let scratch_values = values
                    .iter()
                    .map(|&value| if value.is_nan() { 0.0 } else { value })
                    .collect::<Vec<_>>();
                scratch.write_linear(0, &scratch_values).unwrap();
                let source = FixedBlockMomentSource::Scratch {
                    values: &scratch,
                    selected_columns: Some(&selected),
                    response: (&response).into(),
                    weights: if provided {
                        BorrowedMapWeights::Provided(&weights)
                    } else {
                        BorrowedMapWeights::Unit
                    },
                    solver_weight_divisor: divisor,
                    column_scales: if scaled { Some(&scales) } else { None },
                };
                for threads in [1, 2, 4, 16] {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| {
                            let layout = finite_wls_moment_layout(source).unwrap();
                            let mut partials = vec![0.0; layout.partial_elements];
                            let mut rhs = vec![0.0; columns];
                            let mut gram = vec![0.0; columns * columns];
                            fill_finite_wls_moments(
                                source,
                                layout,
                                &mut partials,
                                &mut rhs,
                                &mut gram,
                            )
                            .unwrap();
                            if scratch_row_partition_plan(rows, stored, columns, layout).is_some() {
                                let response_norm = response
                                    .iter()
                                    .enumerate()
                                    .map(|(i, y)| {
                                        (if provided { weights[i] / divisor } else { 1. }) * y * y
                                    })
                                    .sum::<f64>()
                                    .sqrt();
                                assert_reordered_moments(
                                    &rhs,
                                    &gram,
                                    &expected_rhs,
                                    &expected_gram,
                                    rows,
                                    response_norm,
                                );
                            } else {
                                assert_eq!(rhs, expected_rhs);
                                assert_eq!(gram, expected_gram);
                            }
                        });
                }
                let mut preserved = vec![0.; scratch_values.len()];
                scratch.read_linear_shared(0, &mut preserved).unwrap();
                assert_eq!(preserved, scratch_values);
            }
        }
    }

    #[test]
    fn retained_dense_moment_cache_rejects_nonfinite_selected_values() {
        let response = [1.0; 4];
        let scales = [1.0; 8];
        for bad in [f64::NAN, f64::INFINITY] {
            let mut values = [1.0; 32];
            values[17] = bad;
            let source = FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::packed(&values, 8),
                rows: 4,
                stored_columns: 8,
                selected_columns: None,
                response: (&response).into(),
                weights: BorrowedMapWeights::Unit,
                solver_weight_divisor: 1.0,
                column_scales: Some(&scales),
            };
            rayon::ThreadPoolBuilder::new()
                .num_threads(2)
                .build()
                .unwrap()
                .install(|| {
                    let layout = finite_wls_moment_layout(source).unwrap();
                    let error = fill_finite_wls_moments(
                        source,
                        layout,
                        &mut [],
                        &mut [0.0; 8],
                        &mut [0.0; 64],
                    )
                    .unwrap_err();
                    assert_eq!(error.code, ErrorCode::InvalidData);
                });
        }
    }

    #[test]
    fn wide_projected_moments_are_bounded_deterministic_and_match_direct_sums() {
        for columns in [65_usize, 128] {
            for spaced in [false, true] {
                let candidates = (0..columns)
                    .map(|j| if spaced { 2 * j } else { j })
                    .collect::<Vec<_>>();
                let stored = candidates[columns - 1] + 2;
                let rows = 137;
                let values = (0..rows * stored)
                    .map(|i| ((i * 17 % 127) as f64 - 63.0) / 64.0)
                    .collect();
                let within =
                    DenseMatrix::from_row_major(rows, stored, values).expect("wide matrix");
                let source = FixedBlockMomentSource::Dense {
                    values: FiniteDenseRows::projected(&within),
                    rows: within.nrows(),
                    stored_columns: within.ncols() - 1,
                    selected_columns: Some(&candidates),
                    response: FiniteResponse::Column(&within, 0),
                    weights: BorrowedMapWeights::Unit,
                    solver_weight_divisor: 1.0,
                    column_scales: None,
                };
                let mut reference = None;
                for threads in [1, 7] {
                    let actual = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .expect("test pool")
                        .install(|| {
                            let layout = finite_wls_moment_layout(source).expect("wide layout");
                            assert_eq!(layout.partial_elements, 0);
                            let mut rhs = vec![0.0; columns];
                            let mut gram = vec![0.0; columns * columns];
                            fill_finite_wls_moments(source, layout, &mut [], &mut rhs, &mut gram)
                                .expect("wide moments");
                            for left in 0..columns {
                                assert_eq!(
                                    rhs[left],
                                    (0..rows)
                                        .map(|r| within[(r, candidates[left] + 1)] * within[(r, 0)])
                                        .sum::<f64>()
                                );
                                for right in 0..columns {
                                    assert_eq!(
                                        gram[left * columns + right],
                                        (0..rows)
                                            .map(|r| within[(r, candidates[left] + 1)]
                                                * within[(r, candidates[right] + 1)])
                                            .sum::<f64>()
                                    );
                                }
                            }
                            (rhs, gram)
                        });
                    if let Some(expected) = &reference {
                        assert_eq!(&actual, expected);
                    }
                    reference = Some(actual);
                }
                let mut invalid = within.clone();
                invalid[(0, 1)] = f64::MAX;
                let invalid_source = FixedBlockMomentSource::Dense {
                    values: FiniteDenseRows::projected(&invalid),
                    rows: invalid.nrows(),
                    stored_columns: invalid.ncols() - 1,
                    selected_columns: Some(&candidates),
                    response: FiniteResponse::Column(&invalid, 0),
                    weights: BorrowedMapWeights::Unit,
                    solver_weight_divisor: 1.0,
                    column_scales: None,
                };
                let layout = finite_wls_moment_layout(invalid_source)
                    .expect("invalid values retain valid layout");
                assert_eq!(
                    fill_finite_wls_moments(
                        invalid_source,
                        layout,
                        &mut [],
                        &mut vec![0.0; columns],
                        &mut vec![0.0; columns * columns]
                    )
                    .expect_err("overflow must fail closed")
                    .code,
                    ErrorCode::InvalidData
                );
            }
        }
    }

    fn retained_qr_report(
        values: &[f64],
        rows: usize,
        columns: usize,
        scales: &[f64],
        relative_threshold: f64,
    ) -> Result<(RankRevealingQrReport, Vec<usize>)> {
        let response = vec![0.0_f64; rows];
        let ids = (0..columns).collect::<Vec<_>>();
        let source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::packed(values, columns),
            rows,
            stored_columns: columns,
            selected_columns: None,
            response: (&response).into(),
            weights: BorrowedMapWeights::Unit,
            solver_weight_divisor: 1.0,
            column_scales: Some(scales),
        };
        let mut stored_row = vec![0.0_f64; columns];
        let mut qr = vec![0.0_f64; columns * columns];
        let mut rhs = vec![0.0_f64; columns];
        let mut trailing = vec![0.0_f64; columns];
        let mut permutation = vec![usize::MAX; columns];
        let report = factor_rank_revealing_retained_qr(
            source,
            &ids,
            relative_threshold,
            &mut stored_row,
            &mut qr,
            &mut rhs,
            &mut trailing,
            &mut permutation,
        )?;
        Ok((report, permutation))
    }

    #[allow(clippy::too_many_arguments)]
    fn retained_qr_solution(
        values: &[f64],
        response: &[f64],
        rows: usize,
        columns: usize,
        scales: &[f64],
        ids: &[usize],
        relative_threshold: f64,
    ) -> Result<(RankRevealingQrReport, Vec<usize>, Vec<f64>)> {
        let source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::packed(values, columns),
            rows,
            stored_columns: columns,
            selected_columns: None,
            response: (response).into(),
            weights: BorrowedMapWeights::Unit,
            solver_weight_divisor: 1.0,
            column_scales: Some(scales),
        };
        let mut stored_row = vec![0.0_f64; columns];
        let mut qr = vec![0.0_f64; columns * columns];
        let mut rhs = vec![0.0_f64; columns];
        let mut trailing = vec![0.0_f64; columns];
        let mut permutation = vec![usize::MAX; columns];
        let report = factor_rank_revealing_retained_qr(
            source,
            ids,
            relative_threshold,
            &mut stored_row,
            &mut qr,
            &mut rhs,
            &mut trailing,
            &mut permutation,
        )?;
        let mut pivot_solution = vec![0.0_f64; report.rank];
        let mut selected_ids = vec![usize::MAX; report.rank];
        let mut solution = vec![0.0_f64; report.rank];
        if !solve_rank_revealing_qr_leading(
            &qr,
            &rhs,
            &permutation,
            ids,
            report.rank,
            &mut pivot_solution,
            &mut selected_ids,
            &mut solution,
        )? {
            return Err(invalid_data("test retained QR solution did not certify"));
        }
        Ok((report, selected_ids, solution))
    }

    fn retained_qr_direct_span_certificate(
        factor_values: &[f64],
        certificate_values: &[f64],
        rows: usize,
        columns: usize,
        scales: &[f64],
        relative_threshold: f64,
    ) -> Result<(RankRevealingQrReport, DroppedSpanCertificate)> {
        let response = vec![0.0_f64; rows];
        let ids = (0..columns).collect::<Vec<_>>();
        let factor_source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::packed(factor_values, columns),
            rows,
            stored_columns: columns,
            selected_columns: None,
            response: (&response).into(),
            weights: BorrowedMapWeights::Unit,
            solver_weight_divisor: 1.0,
            column_scales: Some(scales),
        };
        let mut stored_row = vec![0.0_f64; columns];
        let mut standardized_row = vec![0.0_f64; columns];
        let mut qr = vec![0.0_f64; columns * columns];
        let mut rhs = vec![0.0_f64; columns];
        let mut permutation = vec![usize::MAX; columns];
        let report = factor_rank_revealing_retained_qr(
            factor_source,
            &ids,
            relative_threshold,
            &mut stored_row,
            &mut qr,
            &mut rhs,
            &mut standardized_row,
            &mut permutation,
        )?;
        let dropped = columns - report.rank;
        let mut projection = vec![0.0_f64; report.rank * dropped];
        let mut residual_norms = vec![0.0_f64; dropped];
        let certificate_source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::packed(certificate_values, columns),
            rows,
            stored_columns: columns,
            selected_columns: None,
            response: (&response).into(),
            weights: BorrowedMapWeights::Unit,
            solver_weight_divisor: 1.0,
            column_scales: Some(scales),
        };
        let certificate = certify_retained_qr_dropped_span(
            certificate_source,
            &qr,
            &permutation,
            report.rank,
            report.initial_norm,
            relative_threshold,
            &mut stored_row,
            &mut standardized_row,
            &mut projection,
            &mut residual_norms,
        )?;
        Ok((report, certificate))
    }

    #[test]
    fn scratch_qr_scan_matches_dense_weighted_rank_and_bounds_reads() {
        let rows = 519;
        let stored = 6;
        let selected = [0, 2, 5];
        let ids = [7, 2, 11];
        let weights: Vec<_> = (0..rows).map(|r| 0.5 + (r % 13) as f64).collect();
        let scales = [0.125, 3.0, 8.0];
        for deficient in [false, true] {
            let mut values = Vec::new();
            let mut response = Vec::new();
            for r in 0..rows {
                let a = (r % 17) as f64 - 8.0;
                let b = (r % 23) as f64 - 11.0;
                let c = if deficient {
                    2.0 * a - b
                } else {
                    (r % 31) as f64
                };
                values.extend([a, 99.0, b, -11.0, 12.0, c]);
                response.push(a - 0.5 * b + 0.25 * c);
            }
            let mut scratch = PreparedOlsScratch::create_row_major(rows, stored).unwrap();
            scratch.write_linear(0, &values).unwrap();
            let sources = [
                FixedBlockMomentSource::Dense {
                    values: FiniteDenseRows::packed(&values, stored),
                    rows,
                    stored_columns: stored,
                    selected_columns: Some(&selected),
                    response: (&response).into(),
                    weights: BorrowedMapWeights::Provided(&weights),
                    solver_weight_divisor: 7.0,
                    column_scales: Some(&scales),
                },
                FixedBlockMomentSource::Scratch {
                    values: &scratch,
                    selected_columns: Some(&selected),
                    response: (&response).into(),
                    weights: BorrowedMapWeights::Provided(&weights),
                    solver_weight_divisor: 7.0,
                    column_scales: Some(&scales),
                },
            ];
            let mut results = Vec::new();
            for source in sources {
                let mut row = [0.0; 6];
                let mut qr = [0.0; 9];
                let mut rhs = [0.0; 3];
                let mut trailing = [0.0; 3];
                let mut permutation = [0; 3];
                let report = factor_rank_revealing_retained_qr(
                    source,
                    &ids,
                    1.0e-10,
                    &mut row,
                    &mut qr,
                    &mut rhs,
                    &mut trailing,
                    &mut permutation,
                )
                .unwrap();
                assert_eq!(report.rank, if deficient { 2 } else { 3 });
                results.push((report, qr, rhs, permutation));
            }
            assert_eq!(results[0], results[1]);
            assert_eq!(
                scratch.shared_read_chunks_for_test(),
                (rows * stored).div_ceil(1024) as u64
            );
        }
    }

    #[test]
    fn retained_qr_drops_general_exact_linear_combinations() {
        let values = [-5.0_f64, -1.0, -7.0, 7.0, 4.0, 2.0, 8.0, -10.0, 46.0];
        let mut scales = [0.0_f64; 3];
        for column in 0..3 {
            scales[column] = (0..3)
                .map(|row| values[row * 3 + column])
                .fold(0.0_f64, f64::hypot);
        }
        let threshold = 128.0 * f64::EPSILON * 3.0;
        let (report, permutation) =
            retained_qr_report(&values, 3, 3, &scales, threshold).expect("retained QR");
        assert_eq!(report.rank, 2);
        assert!(!report.ambiguous);
        assert!(report.maximum_dropped_relative_norm <= threshold);
        assert_eq!(&permutation[..2], &[0, 1]);
    }

    #[test]
    fn retained_qr_is_canonical_under_row_and_column_permutations() {
        let first = [3.0_f64, -2.0, 4.0, 1.0, -5.0];
        let second = [1.0_f64, 2.0, -1.0, 3.0, 4.0];
        let dependent = std::array::from_fn::<_, 5, _>(|row| 2.0 * first[row] - 3.0 * second[row]);
        let response = std::array::from_fn::<_, 5, _>(|row| 4.0 * first[row] + 0.5 * second[row]);
        let columns = [&first, &second, &dependent];
        let values = (0..first.len())
            .flat_map(|row| columns.iter().map(move |column| column[row]))
            .collect::<Vec<_>>();
        let scales = columns
            .iter()
            .map(|column| column.iter().copied().fold(0.0_f64, f64::hypot))
            .collect::<Vec<_>>();
        let ids = [20_usize, 10, 5];
        let threshold = 128.0 * f64::EPSILON * columns.len() as f64;
        let reference = retained_qr_solution(
            &values,
            &response,
            first.len(),
            columns.len(),
            &scales,
            &ids,
            threshold,
        )
        .expect("canonical retained QR");
        assert_eq!(reference.0.rank, 2);
        assert!(!reference.0.ambiguous);

        let physical_order = [2_usize, 0, 1];
        let permuted_values = (0..first.len())
            .flat_map(|row| {
                physical_order
                    .iter()
                    .map(move |column| columns[*column][row])
            })
            .collect::<Vec<_>>();
        let permuted_scales = physical_order
            .iter()
            .map(|column| scales[*column])
            .collect::<Vec<_>>();
        let permuted_ids = physical_order
            .iter()
            .map(|column| ids[*column])
            .collect::<Vec<_>>();
        let column_permuted = retained_qr_solution(
            &permuted_values,
            &response,
            first.len(),
            columns.len(),
            &permuted_scales,
            &permuted_ids,
            threshold,
        )
        .expect("column-permuted retained QR");

        let reversed_values = (0..first.len())
            .rev()
            .flat_map(|row| columns.iter().map(move |column| column[row]))
            .collect::<Vec<_>>();
        let reversed_response = response.iter().copied().rev().collect::<Vec<_>>();
        let row_permuted = retained_qr_solution(
            &reversed_values,
            &reversed_response,
            first.len(),
            columns.len(),
            &scales,
            &ids,
            threshold,
        )
        .expect("row-permuted retained QR");

        for candidate in [&column_permuted, &row_permuted] {
            assert_eq!(candidate.0.rank, reference.0.rank);
            assert!(!candidate.0.ambiguous);
            assert_eq!(candidate.1, reference.1);
            for (actual, expected) in candidate.2.iter().zip(&reference.2) {
                assert_close(*actual, *expected, 2.0e-12);
            }
        }
    }

    #[test]
    fn retained_qr_handles_extreme_finite_column_scales() {
        let first = [1.0_f64, -2.0, 3.0, -4.0];
        let second = [2.0_f64, 1.0, -1.0, 3.0];
        let huge = 1.0e150_f64;
        let tiny = 1.0e-150_f64;
        let values = (0..first.len())
            .flat_map(|row| [huge * first[row], tiny * second[row]])
            .collect::<Vec<_>>();
        let response = std::array::from_fn::<_, 4, _>(|row| first[row] - second[row]);
        let scales = [
            first.iter().copied().fold(0.0_f64, f64::hypot) * huge,
            second.iter().copied().fold(0.0_f64, f64::hypot) * tiny,
        ];
        let (report, selected, solution) = retained_qr_solution(
            &values,
            &response,
            first.len(),
            2,
            &scales,
            &[0, 1],
            128.0 * f64::EPSILON * first.len() as f64,
        )
        .expect("extreme-scale retained QR");
        assert_eq!(report.rank, 2);
        assert!(!report.ambiguous);
        assert_eq!(selected, [0, 1]);
        assert!(solution.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn retained_qr_covers_single_column_and_clear_threshold_sides() {
        let (single, _) = retained_qr_report(&[3.0_f64, -4.0], 2, 1, &[5.0], 1.0e-10)
            .expect("single-column retained QR");
        assert_eq!(single.rank, 1);
        assert!(!single.ambiguous);

        let relative_threshold = 0.25_f64;
        for (multiplier, expected_rank) in [
            (1.0 - 128.0 * f64::EPSILON, 1_usize),
            (1.0 + 128.0 * f64::EPSILON, 2_usize),
        ] {
            let values = [1.0_f64, 0.0, 0.0, relative_threshold * multiplier];
            let (report, _) = retained_qr_report(&values, 2, 2, &[1.0, 1.0], relative_threshold)
                .expect("clear threshold-side retained QR");
            assert_eq!(report.rank, expected_rank);
            assert!(!report.ambiguous);
        }
    }

    #[test]
    fn retained_qr_rank_decision_is_invariant_to_exact_column_scaling() {
        let first = [3.439_211_223_327_039_f64, -7.017_100_583_832_24];
        let multiplier = 6.285_119_033_872_249_f64;
        let values = [
            first[0],
            multiplier * first[0],
            first[1],
            multiplier * first[1],
        ];
        let scales = [
            first.into_iter().fold(0.0_f64, f64::hypot),
            values
                .iter()
                .copied()
                .skip(1)
                .step_by(2)
                .fold(0.0_f64, f64::hypot),
        ];
        let threshold = 128.0 * f64::EPSILON * 2.0;
        let (report, _) =
            retained_qr_report(&values, 2, 2, &scales, threshold).expect("scaled retained QR");
        assert_eq!(report.rank, 1);
        assert!(!report.ambiguous);
        assert!(report.maximum_dropped_relative_norm <= threshold);
    }

    #[test]
    fn retained_qr_marks_both_sides_of_rank_threshold_ambiguous() {
        let relative_threshold = 0.25_f64;
        for multiplier in [1.0 - 32.0 * f64::EPSILON, 1.0 + 32.0 * f64::EPSILON] {
            let values = [1.0_f64, 0.0, 0.0, relative_threshold * multiplier];
            let (report, _) = retained_qr_report(&values, 2, 2, &[1.0, 1.0], relative_threshold)
                .expect("guard-band retained QR");
            assert!(report.ambiguous);
        }
    }

    #[test]
    fn retained_qr_handles_exact_deficiency_above_sixty_four_columns() {
        let independent = 65_usize;
        let columns = independent + 1;
        let rows = independent;
        let mut values = vec![0.0_f64; rows * columns];
        for row in 0..rows {
            values[row * columns + row] = 1.0;
            values[row * columns + independent] = f64::from(row == 0);
        }
        let scales = vec![1.0_f64; columns];
        let threshold = 128.0 * f64::EPSILON * columns as f64;
        let (report, permutation) = retained_qr_report(&values, rows, columns, &scales, threshold)
            .expect("wide retained QR");
        assert_eq!(report.rank, independent);
        assert!(!report.ambiguous);
        assert_eq!(
            &permutation[..independent],
            &(0..independent).collect::<Vec<_>>()
        );
    }

    #[test]
    fn retained_qr_rejects_nonfinite_observation_space_inputs() {
        let result = retained_qr_report(&[1.0_f64, f64::NAN], 1, 2, &[1.0, 1.0], 1.0e-10);
        assert!(result.is_err());
    }

    #[test]
    fn pivoted_and_canonical_factorizations_share_the_same_primitives() {
        let source_gram = [4.0_f64, 1.0, 1.0, 3.0];
        let source_rhs = [2.0_f64, -1.0];
        for policy in [
            CholeskyPivotPolicy::Canonical,
            CholeskyPivotPolicy::MaximumDiagonal,
        ] {
            let mut gram = source_gram;
            let mut rhs = source_rhs;
            let mut factor = [0.0; 4];
            let mut permutation = [0_usize; 2];
            let report = factor_guarded_cholesky_in_place(
                &mut gram,
                &mut rhs,
                &mut factor,
                &mut permutation,
                policy,
                1e-14,
            )
            .expect("factor")
            .expect("positive definite");
            assert!(report.minimum_pivot > 0.0);
            let mut forward = [0.0; 2];
            let mut solution = [0.0; 2];
            assert!(
                solve_guarded_cholesky(&factor, &rhs, &mut forward, &mut solution).expect("solve")
            );
            let mut inverse_lower = [0.0; 4];
            let mut inverse = [0.0; 4];
            assert!(invert_guarded_cholesky(
                &factor,
                &permutation,
                &mut inverse_lower,
                &mut inverse,
            )
            .expect("inverse"));
            let positions = [0_usize, 1];
            assert!(
                inverse_relative_infinity_norm(&source_gram, 2, &positions, &inverse, 2, false,)
                    .expect("left certificate")
                    < 1e-14
            );
            assert!(
                inverse_relative_infinity_norm(&source_gram, 2, &positions, &inverse, 2, true,)
                    .expect("right certificate")
                    < 1e-14
            );
        }
    }

    #[test]
    fn rank_revealing_factor_is_deterministic_and_full_stride_above_64_columns() {
        let dimension = 65_usize;
        let square = dimension * dimension;
        let mut original_gram = vec![0.0_f64; square];
        let mut original_rhs = vec![0.0_f64; dimension];
        let generated_ids: Vec<usize> = (0..dimension).rev().collect();
        for position in 0..dimension {
            original_gram[position * dimension + position] = 1.0;
            original_rhs[position] = position as f64 + 0.5;
        }
        // The smallest generated identity has a slightly smaller diagonal but
        // remains inside the 64-epsilon pivot-tie band, so identity wins.
        let smallest_id_position = dimension - 1;
        let tied_norm = 1.0 - 32.0 * f64::EPSILON;
        original_gram[smallest_id_position * dimension + smallest_id_position] =
            tied_norm * tied_norm;
        let untouched_gram = original_gram.clone();
        let untouched_rhs = original_rhs.clone();
        let mut gram_scratch = vec![0.0_f64; square];
        let mut rhs_scratch = vec![0.0_f64; dimension];
        let mut factor = vec![0.0_f64; square];
        let mut permutation = vec![usize::MAX; dimension];

        let report = factor_rank_revealing_cholesky(
            &original_gram,
            &original_rhs,
            &generated_ids,
            1.0e-10,
            &mut gram_scratch,
            &mut rhs_scratch,
            &mut factor,
            &mut permutation,
        )
        .expect("rank factor");
        assert_eq!(original_gram, untouched_gram);
        assert_eq!(original_rhs, untouched_rhs);
        assert_eq!(report.rank, dimension);
        assert_eq!(report.maximum_dropped_relative_norm, 0.0);
        assert!(!report.ambiguous);
        assert!(report.condition_proxy >= 1.0);
        let pivoted_ids: Vec<usize> = permutation
            .iter()
            .map(|position| generated_ids[*position])
            .collect();
        assert_eq!(pivoted_ids, (0..dimension).collect::<Vec<_>>());

        let mut forward = vec![0.0_f64; dimension];
        let mut selected_ids = vec![usize::MAX; dimension];
        let mut solution = vec![0.0_f64; dimension];
        assert!(solve_rank_revealing_cholesky_leading(
            &factor,
            &rhs_scratch,
            &permutation,
            &generated_ids,
            report.rank,
            &mut forward,
            &mut selected_ids,
            &mut solution,
        )
        .expect("rank solve"));
        assert_eq!(selected_ids, (0..dimension).collect::<Vec<_>>());
        for (canonical_position, generated_id) in selected_ids.iter().copied().enumerate() {
            let original_position = dimension - 1 - generated_id;
            let diagonal = original_gram[original_position * dimension + original_position];
            assert_close(
                solution[canonical_position],
                original_rhs[original_position] / diagonal,
                16.0 * f64::EPSILON * original_rhs[original_position].abs().max(1.0),
            );
        }

        let mut inverse_lower = vec![0.0_f64; square];
        let mut inverse_ids = vec![usize::MAX; dimension];
        let mut inverse = vec![0.0_f64; square];
        assert!(invert_rank_revealing_cholesky_leading(
            &factor,
            &permutation,
            &generated_ids,
            report.rank,
            &mut inverse_lower,
            &mut inverse_ids,
            &mut inverse,
        )
        .expect("rank inverse"));
        assert_eq!(inverse_ids, selected_ids);
        for row in 0..dimension {
            for column in 0..dimension {
                let expected = if row == column {
                    let original_position = dimension - 1 - inverse_ids[row];
                    1.0 / original_gram[original_position * dimension + original_position]
                } else {
                    0.0
                };
                assert_close(
                    inverse[row * dimension + column],
                    expected,
                    16.0 * f64::EPSILON,
                );
            }
        }
    }

    #[test]
    fn retained_qr_direct_span_accepts_an_exact_dependency() {
        let values = [1.0_f64, 1.0, 0.0, 0.0];
        let threshold = 1.0e-8;
        let (report, certificate) =
            retained_qr_direct_span_certificate(&values, &values, 2, 2, &[1.0, 1.0], threshold)
                .expect("direct retained span certificate");
        assert_eq!(report.rank, 1);
        assert_eq!(certificate.maximum_dropped_relative_norm, 0.0);
        assert!(!certificate.ambiguous);
        assert!(certificate.passed);
    }

    #[test]
    fn retained_qr_direct_span_rejects_a_perturbed_omitted_direction() {
        let factor_values = [1.0_f64, 1.0, 0.0, 0.0];
        let certificate_values = [1.0_f64, 1.0, 0.0, 0.5];
        let (report, certificate) = retained_qr_direct_span_certificate(
            &factor_values,
            &certificate_values,
            2,
            2,
            &[1.0, 1.0],
            1.0e-8,
        )
        .expect("direct retained span certificate");
        assert_eq!(report.rank, 1);
        assert_eq!(certificate.maximum_dropped_relative_norm, 0.5);
        assert!(!certificate.ambiguous);
        assert!(!certificate.passed);
    }

    #[test]
    fn retained_qr_direct_span_fails_closed_in_the_threshold_guard() {
        let factor_values = [1.0_f64, 1.0, 0.0, 0.0];
        let relative_threshold = 0.25_f64;
        for multiplier in [1.0 - 32.0 * f64::EPSILON, 1.0 + 32.0 * f64::EPSILON] {
            let certificate_values = [1.0_f64, 1.0, 0.0, relative_threshold * multiplier];
            let (report, certificate) = retained_qr_direct_span_certificate(
                &factor_values,
                &certificate_values,
                2,
                2,
                &[1.0, 1.0],
                relative_threshold,
            )
            .expect("guard-band direct retained span certificate");
            assert_eq!(report.rank, 1);
            assert!(certificate.ambiguous);
            assert!(!certificate.passed);
        }
    }

    #[test]
    fn rank_revealing_factor_drops_an_exact_duplicate_and_compacts_outputs() {
        let dimension = 3_usize;
        let original_gram = vec![1.0_f64, 0.5, 1.0, 0.5, 1.0, 0.5, 1.0, 0.5, 1.0];
        let original_rhs = vec![1.0_f64, 2.0, 1.0];
        let generated_ids = vec![10_usize, 20, 5];
        let untouched_gram = original_gram.clone();
        let untouched_rhs = original_rhs.clone();
        let mut gram_scratch = vec![0.0_f64; dimension * dimension];
        let mut rhs_scratch = vec![0.0_f64; dimension];
        let mut factor = vec![0.0_f64; dimension * dimension];
        let mut permutation = vec![usize::MAX; dimension];

        let report = factor_rank_revealing_cholesky(
            &original_gram,
            &original_rhs,
            &generated_ids,
            1.0e-10,
            &mut gram_scratch,
            &mut rhs_scratch,
            &mut factor,
            &mut permutation,
        )
        .expect("rank factor");
        assert_eq!(original_gram, untouched_gram);
        assert_eq!(original_rhs, untouched_rhs);
        assert_eq!(report.rank, 2);
        assert_eq!(report.maximum_dropped_relative_norm, 0.0);
        assert!(!report.ambiguous);
        assert_close(
            report.condition_proxy,
            (4.0_f64 / 3.0).sqrt(),
            16.0 * f64::EPSILON,
        );
        assert_eq!(
            permutation[..report.rank]
                .iter()
                .map(|position| generated_ids[*position])
                .collect::<Vec<_>>(),
            vec![5, 20]
        );

        let mut forward = vec![0.0_f64; report.rank];
        let mut selected_ids = vec![usize::MAX; report.rank];
        let mut solution = vec![0.0_f64; report.rank];
        assert!(solve_rank_revealing_cholesky_leading(
            &factor,
            &rhs_scratch,
            &permutation,
            &generated_ids,
            report.rank,
            &mut forward,
            &mut selected_ids,
            &mut solution,
        )
        .expect("rank solve"));
        assert_eq!(selected_ids, vec![5, 20]);
        assert_close(solution[0], 0.0, 16.0 * f64::EPSILON);
        assert_close(solution[1], 2.0, 16.0 * f64::EPSILON);

        let rank_square = report.rank * report.rank;
        let mut inverse_lower = vec![0.0_f64; rank_square];
        let mut inverse_ids = vec![usize::MAX; report.rank];
        let mut inverse = vec![0.0_f64; rank_square];
        assert!(invert_rank_revealing_cholesky_leading(
            &factor,
            &permutation,
            &generated_ids,
            report.rank,
            &mut inverse_lower,
            &mut inverse_ids,
            &mut inverse,
        )
        .expect("rank inverse"));
        assert_eq!(inverse_ids, selected_ids);
        let expected = [4.0 / 3.0, -2.0 / 3.0, -2.0 / 3.0, 4.0 / 3.0];
        for (actual, expected) in inverse.iter().zip(expected) {
            assert_close(*actual, expected, 32.0 * f64::EPSILON);
        }
    }

    #[test]
    fn rank_revealing_factor_marks_both_sides_of_threshold_guard_ambiguous() {
        let relative_threshold = 0.25_f64;
        for multiplier in [1.0 - 32.0 * f64::EPSILON, 1.0 + 32.0 * f64::EPSILON] {
            let near_threshold_norm = relative_threshold * multiplier;
            let original_gram = [1.0_f64, 0.0, 0.0, near_threshold_norm.powi(2)];
            let original_rhs = [1.0_f64, 1.0];
            let generated_ids = [7_usize, 3];
            let mut gram_scratch = [0.0_f64; 4];
            let mut rhs_scratch = [0.0_f64; 2];
            let mut factor = [0.0_f64; 4];
            let mut permutation = [usize::MAX; 2];
            let report = factor_rank_revealing_cholesky(
                &original_gram,
                &original_rhs,
                &generated_ids,
                relative_threshold,
                &mut gram_scratch,
                &mut rhs_scratch,
                &mut factor,
                &mut permutation,
            )
            .expect("rank factor");
            assert!(report.ambiguous);
            let reconstructed_norm = original_gram[3].sqrt();
            if reconstructed_norm <= relative_threshold {
                assert_eq!(report.rank, 1);
                assert_close(
                    report.maximum_dropped_relative_norm,
                    reconstructed_norm,
                    2.0 * f64::EPSILON,
                );
            } else {
                assert_eq!(report.rank, 2);
                assert_eq!(report.maximum_dropped_relative_norm, 0.0);
            }
        }
    }

    #[test]
    fn dropped_span_certificate_accepts_an_exact_duplicate() {
        let gram = [1.0_f64, 1.0, 1.0, 1.0];
        let certificate = certify_rank_revealing_dropped_span(&gram, 2, &[0], &[1.0], 1.0e-10)
            .expect("dropped-span certificate");
        assert_eq!(certificate.maximum_dropped_relative_norm, 0.0);
        assert!(!certificate.ambiguous);
        assert!(certificate.passed);
    }

    #[test]
    fn dropped_span_certificate_rejects_an_independent_omitted_direction() {
        let gram = [1.0_f64, 0.0, 0.0, 1.0];
        let certificate = certify_rank_revealing_dropped_span(&gram, 2, &[0], &[1.0], 1.0e-10)
            .expect("dropped-span certificate");
        assert_eq!(certificate.maximum_dropped_relative_norm, 1.0);
        assert!(!certificate.ambiguous);
        assert!(!certificate.passed);
    }

    #[test]
    fn dropped_span_certificate_fails_closed_inside_the_threshold_guard() {
        let relative_threshold = 0.25_f64;
        for multiplier in [1.0 - 32.0 * f64::EPSILON, 1.0 + 32.0 * f64::EPSILON] {
            let omitted_norm = relative_threshold * multiplier;
            let gram = [1.0_f64, 0.0, 0.0, omitted_norm.powi(2)];
            let certificate =
                certify_rank_revealing_dropped_span(&gram, 2, &[0], &[1.0], relative_threshold)
                    .expect("dropped-span certificate");
            assert_close(
                certificate.maximum_dropped_relative_norm,
                omitted_norm,
                2.0 * f64::EPSILON,
            );
            assert!(certificate.ambiguous);
            assert!(!certificate.passed);
        }
    }

    #[test]
    fn dropped_span_certificate_rejects_asymmetric_and_negative_grams() {
        for gram in [[1.0_f64, 0.25, 0.5, 1.0], [1.0_f64, 0.0, 0.0, -1.0]] {
            let error = certify_rank_revealing_dropped_span(&gram, 2, &[0], &[1.0], 1.0e-10)
                .expect_err("malformed Gram must fail");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn dropped_span_certificate_accepts_a_zero_gram_only_at_rank_zero() {
        let gram = [0.0_f64; 4];
        let rank_zero = certify_rank_revealing_dropped_span(&gram, 2, &[], &[], 1.0e-10)
            .expect("rank-zero dropped-span certificate");
        assert_eq!(rank_zero.maximum_dropped_relative_norm, 0.0);
        assert!(!rank_zero.ambiguous);
        assert!(rank_zero.passed);

        let selected = certify_rank_revealing_dropped_span(&gram, 2, &[0], &[1.0], 1.0e-10)
            .expect("selected zero-Gram certificate");
        assert_eq!(selected.maximum_dropped_relative_norm, 0.0);
        assert!(!selected.ambiguous);
        assert!(!selected.passed);
    }
}
