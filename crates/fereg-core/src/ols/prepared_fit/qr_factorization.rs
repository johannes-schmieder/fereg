use super::{
    apply_householder, apply_householder_slices, invalid_argument, invalid_data,
    stable_matrix_segment_norm, stable_slice_segment_norm, DenseMatrix, PreparedOlsRankCertificate,
    PreparedOlsScratch, Result, PARALLEL_QR_MIN_ROWS,
};
use rayon::prelude::*;

// Storage owns traversal and norm-refresh timing; rank policy is shared.
pub(super) trait QrStorage {
    const REFRESH_BEFORE_PIVOT: bool;
    const SCAN_FULL_RANK: bool;
    fn norms(&mut self, columns: &[usize], rank: usize, norms: &mut [f64]) -> Result<()>;
    fn pivot(&mut self, column: usize, rank: usize, cached_norm: f64) -> Result<(f64, f64)>;
    fn reflect(
        &mut self,
        columns: &[usize],
        rank: usize,
        norms: &mut [f64],
        beta: f64,
        denominator: f64,
        tau: f64,
    ) -> Result<()>;
}

pub(super) struct DenseQr<'a>(pub(super) &'a mut DenseMatrix);

impl QrStorage for DenseQr<'_> {
    const REFRESH_BEFORE_PIVOT: bool = true;
    const SCAN_FULL_RANK: bool = false;
    fn norms(&mut self, columns: &[usize], rank: usize, norms: &mut [f64]) -> Result<()> {
        if norms.len() != columns.len() || rank > self.0.nrows() {
            return Err(invalid_argument(
                "prepared OLS QR trailing-norm dimensions are inconsistent",
            ));
        }
        if self.0.nrows() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
            norms.par_iter_mut().enumerate().try_for_each(
                |(position, destination)| -> Result<()> {
                    *destination = if position < rank {
                        0.0
                    } else {
                        stable_matrix_segment_norm(self.0, columns[position] + 1, rank)?
                    };
                    Ok(())
                },
            )?;
        } else {
            for (position, generated_column) in columns.iter().copied().enumerate() {
                norms[position] = if position < rank {
                    0.0
                } else {
                    stable_matrix_segment_norm(self.0, generated_column + 1, rank)?
                };
            }
        }
        Ok(())
    }

    fn pivot(&mut self, column: usize, rank: usize, cached_norm: f64) -> Result<(f64, f64)> {
        // Dense norms were refreshed before selection; avoid rescanning the pivot.
        Ok((self.0[(rank, column + 1)], cached_norm))
    }

    fn reflect(
        &mut self,
        columns: &[usize],
        rank: usize,
        _norms: &mut [f64],
        beta: f64,
        denominator: f64,
        tau: f64,
    ) -> Result<()> {
        let column = columns[rank] + 1;
        let nrows = self.0.nrows();
        let width = self.0.ncols();
        self.0[(rank, column)] = beta;
        if nrows >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
            self.0
                .as_mut_slice()
                .par_chunks_mut(width)
                .skip(rank + 1)
                .try_for_each(|row| -> Result<()> {
                    row[column] /= denominator;
                    if row[column].is_finite() {
                        Ok(())
                    } else {
                        Err(invalid_data(
                            "prepared OLS QR Householder vector is not finite",
                        ))
                    }
                })?;
        } else {
            for row in (rank + 1)..nrows {
                self.0[(row, column)] /= denominator;
                if !self.0[(row, column)].is_finite() {
                    return Err(invalid_data(
                        "prepared OLS QR Householder vector is not finite",
                    ));
                }
            }
        }
        for &other in &columns[(rank + 1)..] {
            apply_householder(self.0, rank, column, other + 1, tau)?;
        }
        apply_householder(self.0, rank, column, 0, tau)
    }
}

pub(super) struct ScratchQr<'a> {
    pub(super) scratch: &'a mut PreparedOlsScratch,
    pub(super) reflector: &'a mut [f64],
    pub(super) target: &'a mut [f64],
    pub(super) outcome: &'a mut [f64],
}

impl QrStorage for ScratchQr<'_> {
    // Target writes refresh norms; the final independent norm pass remains.
    const REFRESH_BEFORE_PIVOT: bool = false;
    const SCAN_FULL_RANK: bool = true;
    fn norms(&mut self, columns: &[usize], rank: usize, norms: &mut [f64]) -> Result<()> {
        if norms.len() != columns.len()
            || rank > self.scratch.rows()
            || self.target.len() != self.scratch.rows()
        {
            return Err(invalid_argument(
                "prepared OLS low-memory trailing-norm dimensions are inconsistent",
            ));
        }
        for (position, generated) in columns.iter().copied().enumerate() {
            norms[position] = if position < rank {
                0.0
            } else {
                self.scratch.read_column(generated, self.target)?;
                stable_slice_segment_norm(self.target, rank)?
            };
        }
        Ok(())
    }

    fn pivot(&mut self, column: usize, rank: usize, _cached_norm: f64) -> Result<(f64, f64)> {
        self.scratch.read_column(column, self.reflector)?;
        Ok((
            self.reflector[rank],
            stable_slice_segment_norm(self.reflector, rank)?,
        ))
    }

    fn reflect(
        &mut self,
        columns: &[usize],
        rank: usize,
        norms: &mut [f64],
        beta: f64,
        denominator: f64,
        tau: f64,
    ) -> Result<()> {
        self.reflector[rank] = beta;
        for value in self.reflector.iter_mut().skip(rank + 1) {
            *value /= denominator;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS low-memory Householder vector is not finite",
                ));
            }
        }
        self.scratch.write_column(columns[rank], self.reflector)?;
        for (position, other) in columns.iter().copied().enumerate().skip(rank + 1) {
            self.scratch.read_column(other, self.target)?;
            apply_householder_slices(self.reflector, self.target, rank, tau)?;
            self.scratch.write_column(other, self.target)?;
            norms[position] = stable_slice_segment_norm(self.target, rank + 1)?;
        }
        apply_householder_slices(self.reflector, self.outcome, rank, tau)
    }
}

pub(super) struct QrFactorization {
    pub(super) rank: usize,
    pub(super) absolute_threshold: f64,
    certificate: PreparedOlsRankCertificate,
}

impl QrFactorization {
    pub(super) fn certificate(&self) -> Result<PreparedOlsRankCertificate> {
        // Validate at the existing post-triangle boundary to retain error precedence.
        if !self.certificate.condition_proxy.is_finite() {
            return Err(invalid_data(
                "prepared OLS QR condition proxy is not finite",
            ));
        }
        Ok(self.certificate)
    }
}

pub(super) fn factor_pivoted_qr<S: QrStorage>(
    mut storage: S,
    nrows: usize,
    columns: &mut [usize],
    norms: &mut [f64],
    relative_threshold: f64,
) -> Result<QrFactorization> {
    storage.norms(columns, 0, norms)?;
    let initial_norm = norms.iter().copied().fold(0.0_f64, f64::max);
    let absolute_threshold = initial_norm * relative_threshold;
    if !absolute_threshold.is_finite() {
        return Err(invalid_data("prepared OLS QR rank threshold is not finite"));
    }
    let mut rank = 0_usize;
    let mut largest_diagonal = 0.0_f64;
    let mut smallest_diagonal = f64::INFINITY;
    while rank < nrows.min(columns.len()) {
        if S::REFRESH_BEFORE_PIVOT && rank > 0 {
            storage.norms(columns, rank, norms)?;
        }
        let mut pivot_position = rank;
        let mut pivot_norm = norms[rank];
        for (candidate, candidate_norm) in norms.iter().copied().enumerate().skip(rank + 1) {
            let scale = candidate_norm.abs().max(pivot_norm.abs());
            let tied = (candidate_norm - pivot_norm).abs() <= 64.0 * f64::EPSILON * scale;
            if candidate_norm > pivot_norm && !tied
                || tied && columns[candidate] < columns[pivot_position]
            {
                pivot_position = candidate;
                pivot_norm = candidate_norm;
            }
        }
        if pivot_norm <= absolute_threshold {
            break;
        }
        columns.swap(rank, pivot_position);
        norms.swap(rank, pivot_position);
        let (first, norm) = storage.pivot(columns[rank], rank, pivot_norm)?;
        if norm <= absolute_threshold {
            break;
        }
        let beta = if first >= 0.0 { -norm } else { norm };
        let denominator = first - beta;
        if !beta.is_finite() || beta == 0.0 || !denominator.is_finite() || denominator == 0.0 {
            return Err(invalid_data(
                "prepared OLS QR Householder construction failed",
            ));
        }
        let tau = (beta - first) / beta;
        if !tau.is_finite() {
            return Err(invalid_data(
                "prepared OLS QR Householder coefficient is not finite",
            ));
        }
        storage.reflect(columns, rank, norms, beta, denominator, tau)?;
        let diagonal = beta.abs();
        largest_diagonal = largest_diagonal.max(diagonal);
        smallest_diagonal = smallest_diagonal.min(diagonal);
        // rank is below both dimensions, so the increment is representable.
        rank += 1;
    }
    if S::SCAN_FULL_RANK || rank < columns.len() {
        storage.norms(columns, rank, norms)?;
    }
    let maximum_dropped_trailing_norm = if rank == columns.len() || initial_norm == 0.0 {
        0.0
    } else {
        norms[rank..].iter().copied().fold(0.0_f64, f64::max) / initial_norm
    };
    let passed = maximum_dropped_trailing_norm <= relative_threshold * (1.0 + 64.0 * f64::EPSILON);
    if !maximum_dropped_trailing_norm.is_finite() || !passed {
        return Err(invalid_data(
            "prepared OLS QR dropped-column rank certificate did not pass",
        ));
    }
    // Positive rank has only finite, nonzero beta values; the minimum is positive.
    let condition_proxy = if rank == 0 {
        1.0
    } else {
        largest_diagonal / smallest_diagonal
    };
    Ok(QrFactorization {
        rank,
        absolute_threshold,
        certificate: PreparedOlsRankCertificate {
            relative_threshold,
            maximum_dropped_trailing_norm,
            condition_proxy,
            passed: true,
        },
    })
}
