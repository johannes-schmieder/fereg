//! One immutable-row-order statistics operation for cold and refreshed banks.
use super::*;

struct ColumnStatistics<'a, const NORM: bool, const MEAN: bool> {
    weights: BorrowedMapWeights<'a>,
    weight_scale: super::super::PreparedOlsBreadWeightScale,
    scale: f64,
    squares: f64,
    mean: f64,
    correction: f64,
}

impl<const NORM: bool, const MEAN: bool> ColumnStatistics<'_, NORM, MEAN> {
    fn add(&mut self, row: usize, value: f64) -> Result<()> {
        let weight = normalized_solver_weight(self.weights, self.weight_scale, row)?;
        if NORM {
            update_scaled_square(weight.sqrt() * value, &mut self.scale, &mut self.squares)?;
        }
        if MEAN {
            compensated_add(weight * value, &mut self.mean, &mut self.correction)?;
        }
        Ok(())
    }
}

/// Columns are independent tasks; every column keeps the original row order.
/// Scalar accumulators live on each lane's stack, replacing K-sized norm and
/// correction banks. Constant flags remove unrequested statistics from leaves.
fn column_statistics<const NORM: bool, const MEAN: bool>(
    rows: usize,
    weights: BorrowedMapWeights<'_>,
    weight_scale: super::super::PreparedOlsBreadWeightScale,
    norms: &mut [f64],
    means: &mut [f64],
    visit: impl Fn(usize, &mut ColumnStatistics<'_, NORM, MEAN>) -> Result<()> + Sync,
) -> Result<()> {
    let columns = if NORM { norms.len() } else { means.len() };
    if NORM && MEAN && norms.len() != means.len() {
        return Err(invalid_argument(
            "weighted-linear statistic columns are not aligned",
        ));
    }
    let mut weight_sum = 0.0;
    let mut weight_correction = 0.0;
    if MEAN {
        for row in 0..rows {
            compensated_add(
                normalized_solver_weight(weights, weight_scale, row)?,
                &mut weight_sum,
                &mut weight_correction,
            )?;
        }
        if !(weight_sum + weight_correction).is_finite() || weight_sum + weight_correction <= 0.0 {
            return Err(invalid_data("weighted-linear mean denominator is invalid"));
        }
    }
    let evaluate = |column| -> Result<(f64, f64)> {
        let mut state = ColumnStatistics {
            weights,
            weight_scale,
            scale: 0.0,
            squares: 1.0,
            mean: 0.0,
            correction: 0.0,
        };
        visit(column, &mut state)?;
        let norm = if NORM {
            stable_norm(state.scale, state.squares)?
        } else {
            0.0
        };
        let mean = if MEAN {
            (state.mean + state.correction) / (weight_sum + weight_correction)
        } else {
            0.0
        };
        if !mean.is_finite() {
            return Err(invalid_data("weighted-linear mean is not finite"));
        }
        Ok((norm, mean))
    };
    let work = rows as u128 * columns as u128 * 3;
    if NORM && MEAN {
        crate::parallel::stage_try_for_each(
            norms.par_iter_mut().zip(means.par_iter_mut()).enumerate(),
            work,
            |(column, (norm, mean))| {
                (*norm, *mean) = evaluate(column)?;
                Ok(())
            },
        )
    } else if NORM {
        crate::parallel::stage_try_for_each(
            norms.par_iter_mut().enumerate(),
            work,
            |(column, norm)| {
                *norm = evaluate(column)?.0;
                Ok(())
            },
        )
    } else {
        crate::parallel::stage_try_for_each(
            means.par_iter_mut().enumerate(),
            work,
            |(column, mean)| {
                *mean = evaluate(column)?.1;
                Ok(())
            },
        )
    }
}

impl<P: PreparedOlsFitView + Sync + ?Sized> CertifiedLinearSession<'_, P> {
    pub(super) fn fill_dense_statistics<const NORM: bool, const MEAN: bool>(
        &self,
        values: &DenseMatrix,
        position: impl Fn(usize) -> usize + Sync,
        norms: &mut [f64],
        means: &mut [f64],
    ) -> Result<()> {
        let columns = if NORM { norms.len() } else { means.len() };
        if values.nrows() != self.response.len()
            || (0..columns).any(|column| position(column) >= values.ncols())
        {
            return Err(invalid_argument(
                "weighted-linear statistic matrix is not aligned",
            ));
        }
        column_statistics::<NORM, MEAN>(
            values.nrows(),
            self.weights.borrowed(),
            self.bread_weight_scale,
            norms,
            means,
            |column, state| {
                let source = position(column);
                for row in 0..values.nrows() {
                    state.add(row, values[(row, source)])?;
                }
                Ok(())
            },
        )
    }

    pub(super) fn refresh_retained_norms(&mut self) -> Result<()> {
        let mut retained = self
            .within
            .take()
            .ok_or_else(|| invalid_argument("weighted-linear retained design disappeared"))?;
        let result = if self.group_centered_source.is_some() {
            if retained.generations.same_design(self.generations) {
                Ok(())
            } else {
                Err(invalid_argument("implicit norms are stale"))
            }
        } else {
            let columns = &retained.stored_generated_columns;
            if retained.reference_norms.len() != columns.len()
                || retained.reference_means.len() != columns.len()
                || retained.within_norms.len() != columns.len()
            {
                Err(invalid_argument(
                    "weighted-linear reference workspaces are absent",
                ))
            } else {
                let rows = self.response.len();
                column_statistics::<true, true>(
                    rows,
                    self.weights.borrowed(),
                    self.bread_weight_scale,
                    &mut retained.reference_norms,
                    &mut retained.reference_means,
                    |column, state| {
                        crate::ols::prepared_fit_view::visit_prepared_column_range(
                            self.prepared,
                            columns[column],
                            0..rows,
                            |row, value| state.add(row, value),
                        )
                    },
                )
            }
        };
        if result.is_ok() {
            retained.generations = WithinGenerations::from_session(self.generations);
        }
        self.within = Some(retained);
        result
    }
}
