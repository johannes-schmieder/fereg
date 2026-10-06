//! Stateless finite rank, factor, solve and inverse authority over immutable rows.
use super::finite_wls_kernel::*;
use super::prepared_fit::FitReservation;
use crate::error::{ErrorCode, FeregError, Result};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RankRequest {
    OlsDiscover,
    RetainedDiscover,
    Frozen,
}
impl RankRequest {
    fn error(self, retained: &'static str, frozen: &'static str) -> FeregError {
        FeregError::new(
            ErrorCode::InvalidData,
            if self == Self::Frozen {
                frozen
            } else {
                retained
            },
        )
    }
}

pub(super) struct FactorBanks<'a> {
    pub gram: &'a [f64],
    pub rhs: &'a [f64],
    pub factor: &'a mut Vec<f64>,
    pub auxiliary: &'a mut Vec<f64>,
    pub solve: &'a mut Vec<f64>,
    pub scores: &'a mut Vec<f64>,
    pub row: &'a mut Vec<f64>,
    pub permutation: &'a mut Vec<usize>,
    pub coefficients: &'a mut Vec<f64>,
    pub inverse: &'a mut Vec<f64>,
    pub selected: &'a mut Vec<usize>,
}

pub(super) struct BasisRelations<'a> {
    pub candidates: &'a [usize],
    pub selected: &'a [usize],
    pub permutation: &'a [usize],
    pub dropped_projection: &'a [f64],
}

pub(super) fn resize<T: Clone, R: FitReservation>(
    values: &mut Vec<T>,
    count: usize,
    zero: T,
    reservation: &mut R,
) -> Result<()> {
    values.clear();
    reservation.reserve(
        values,
        count,
        "unable to reserve finite operation workspace",
    )?;
    values.resize(count, zero);
    Ok(())
}

pub(super) struct FactorResult {
    pub report: RankRevealingQrReport,
    pub used_qr: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_factor<R: FitReservation>(
    source: FixedBlockMomentSource<'_>,
    identities: &[usize],
    relative_threshold: f64,
    request: RankRequest,
    allow_cholesky: bool,
    mut banks: FactorBanks<'_>,
    reservation: &mut R,
    mut capture: impl FnMut(BasisRelations<'_>, usize, f64) -> Result<()>,
) -> Result<FactorResult> {
    let count = identities.len();
    let square = count.checked_mul(count).ok_or_else(|| {
        FeregError::new(ErrorCode::Allocation, "finite operation square overflows")
    })?;
    if count == 0
        || source.dimension() != count
        || banks.rhs.len() != count
        || banks.gram.len() != square
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "finite operation axes are inconsistent",
        ));
    }
    prepare_factor(source, count, square, &mut banks, reservation)?;
    let cholesky = if allow_cholesky {
        factor_finite_cholesky(
            banks.auxiliary,
            banks.solve,
            banks.factor,
            banks.permutation,
            relative_threshold,
        )?
    } else {
        None
    };
    if let Some(report) = cholesky {
        let failure = cholesky_solution(count, identities, &mut banks, reservation)?;
        let certified = failure.is_none();
        if certified {
            return Ok(FactorResult {
                report: RankRevealingQrReport {
                    rank: count,
                    initial_norm: 0.0,
                    maximum_dropped_relative_norm: 0.0,
                    condition_proxy: report.maximum_diagonal / report.minimum_diagonal,
                    ambiguous: false,
                },
                used_qr: false,
            });
        }
        if request != RankRequest::OlsDiscover {
            let inverse = failure == Some(CholeskyFailure::Inverse);
            return Err(request.error(
                if inverse {
                    "retained weighted-linear Cholesky inverse is not finite"
                } else {
                    "retained weighted-linear Cholesky solution is not finite"
                },
                if inverse {
                    "frozen weighted-linear Cholesky inverse is not finite"
                } else {
                    "frozen weighted-linear Cholesky solution is not finite"
                },
            ));
        }
    }
    let report = factor_rank_revealing_retained_qr(
        source,
        identities,
        relative_threshold,
        banks.row,
        banks.factor,
        banks.solve,
        banks.scores,
        banks.permutation,
    )?;
    if request == RankRequest::Frozen && (report.ambiguous || report.rank != count) {
        return Err(request.error(
            "retained weighted-linear rank decision is numerically ambiguous",
            "frozen weighted-linear basis lost certified full rank",
        ));
    }
    if request == RankRequest::RetainedDiscover && report.ambiguous {
        return Err(request.error(
            "retained weighted-linear rank decision is numerically ambiguous",
            "",
        ));
    }
    let mut report = qr_solution(
        source,
        identities,
        report,
        request,
        relative_threshold,
        &mut banks,
        reservation,
        &mut capture,
    )?;
    if !report.condition_proxy.is_finite() {
        return Err(request.error(
            "retained weighted-linear condition proxy is not finite",
            "frozen weighted-linear condition proxy is not finite",
        ));
    }
    report.ambiguous &= request != RankRequest::OlsDiscover;
    Ok(FactorResult {
        report,
        used_qr: true,
    })
}

fn prepare_factor<R: FitReservation>(
    source: FixedBlockMomentSource<'_>,
    count: usize,
    square: usize,
    banks: &mut FactorBanks<'_>,
    reservation: &mut R,
) -> Result<()> {
    let stored = match source {
        FixedBlockMomentSource::Dense { stored_columns, .. } => stored_columns,
        FixedBlockMomentSource::Scratch { values, .. } => values.columns(),
        FixedBlockMomentSource::Implicit { values, .. } => values.columns(),
    };
    resize(banks.factor, square, 0., reservation)?;
    resize(banks.solve, count, 0., reservation)?;
    resize(banks.scores, count, 0., reservation)?;
    resize(banks.row, stored, 0., reservation)?;
    resize(banks.permutation, count, 0, reservation)?;
    resize(banks.auxiliary, square, 0., reservation)?;
    banks.auxiliary.copy_from_slice(banks.gram);
    banks.solve.copy_from_slice(banks.rhs);
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CholeskyFailure {
    Solution,
    Inverse,
}

fn cholesky_solution<R: FitReservation>(
    count: usize,
    identities: &[usize],
    banks: &mut FactorBanks<'_>,
    reservation: &mut R,
) -> Result<Option<CholeskyFailure>> {
    resize(banks.scores, count, 0., reservation)?;
    resize(banks.coefficients, count, 0., reservation)?;
    if !solve_finite_cholesky(
        banks.factor,
        banks.solve,
        banks.permutation,
        banks.scores,
        banks.coefficients,
        &mut banks.row[..count],
    )? {
        return Ok(Some(CholeskyFailure::Solution));
    }
    banks.coefficients.copy_from_slice(&banks.row[..count]);
    resize(banks.selected, count, 0, reservation)?;
    banks.selected.copy_from_slice(identities);
    resize(banks.inverse, count * count, 0., reservation)?;
    let finite = invert_guarded_cholesky(
        banks.factor,
        banks.permutation,
        banks.auxiliary,
        banks.inverse,
    )?;
    Ok((!finite).then_some(CholeskyFailure::Inverse))
}

#[allow(clippy::too_many_arguments)]
fn qr_solution<R: FitReservation>(
    source: FixedBlockMomentSource<'_>,
    identities: &[usize],
    mut report: RankRevealingQrReport,
    request: RankRequest,
    threshold: f64,
    banks: &mut FactorBanks<'_>,
    reservation: &mut R,
    capture: &mut impl FnMut(BasisRelations<'_>, usize, f64) -> Result<()>,
) -> Result<RankRevealingQrReport> {
    let rank = report.rank;
    let count = identities.len();
    resize(banks.scores, rank, 0., reservation)?;
    resize(banks.coefficients, rank, 0., reservation)?;
    resize(banks.selected, rank, 0, reservation)?;
    if !solve_rank_revealing_qr_leading(
        banks.factor,
        banks.solve,
        banks.permutation,
        identities,
        rank,
        banks.scores,
        banks.selected,
        banks.coefficients,
    )? {
        return Err(request.error(
            "retained weighted-linear rank solution is not finite",
            "frozen weighted-linear QR solution is not finite",
        ));
    }
    if request != RankRequest::Frozen {
        let dropped = count - rank;
        resize(banks.auxiliary, rank * dropped, 0., reservation)?;
        resize(banks.scores, count, 0., reservation)?;
        resize(banks.solve, dropped, 0., reservation)?;
        let span = certify_retained_qr_dropped_span(
            source,
            banks.factor,
            banks.permutation,
            rank,
            report.initial_norm,
            threshold,
            banks.row,
            banks.scores,
            banks.auxiliary,
            banks.solve,
        )?;
        let ols_boundary = threshold * (1.0 + RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON);
        if !(span.passed
            || (request == RankRequest::OlsDiscover
                && span.maximum_dropped_relative_norm.is_finite()
                && span.maximum_dropped_relative_norm <= ols_boundary))
        {
            return Err(request.error(
                "retained weighted-linear direct dropped-span certificate did not pass",
                "",
            ));
        }
        report.maximum_dropped_relative_norm = report
            .maximum_dropped_relative_norm
            .max(span.maximum_dropped_relative_norm);
        capture(
            BasisRelations {
                candidates: identities,
                selected: banks.selected,
                permutation: banks.permutation,
                dropped_projection: banks.auxiliary,
            },
            rank,
            report.initial_norm,
        )?;
    }
    resize(banks.auxiliary, rank * rank, 0., reservation)?;
    resize(banks.inverse, rank * rank, 0., reservation)?;
    if !invert_rank_revealing_qr_leading(
        banks.factor,
        banks.permutation,
        identities,
        rank,
        banks.auxiliary,
        banks.selected,
        banks.inverse,
    )? {
        return Err(request.error(
            "retained weighted-linear rank inverse is not finite",
            "frozen weighted-linear QR inverse is not finite",
        ));
    }
    Ok(report)
}
