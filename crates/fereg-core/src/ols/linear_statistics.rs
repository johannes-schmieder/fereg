//! One immutable-row-order statistics operation for cold and refreshed banks.
use super::*;

struct ColumnStatistics<'a, const NORM: bool, const MEAN: bool> {
    /// Validated raw weights and their normalization divisor; `None` is unit
    /// weight. Rows use `weight / divisor` and its square root.
    weights: Option<&'a [f64]>,
    divisor: f64,
    /// Exact scaled sum of squares (fallback); otherwise plain sums.
    exact: bool,
    scale: f64,
    squares: f64,
    largest: f64,
    mean: f64,
    correction: f64,
}

impl<const NORM: bool, const MEAN: bool> ColumnStatistics<'_, NORM, MEAN> {
    #[inline(always)]
    fn weight(&self, row: usize) -> f64 {
        match self.weights {
            Some(weights) => weights[row] / self.divisor,
            None => 1.0,
        }
    }

    /// Plain-path values are not checked per row: a nonfinite value makes the
    /// chunk's square sum nonfinite (sending the column to the exact pass,
    /// which reports it) and its compensated mean nonfinite (rejected when the
    /// chunks merge).
    #[inline(always)]
    fn add(&mut self, row: usize, value: f64) -> Result<()> {
        let weight = self.weight(row);
        if NORM {
            let scaled = match self.weights {
                Some(_) => weight.sqrt() * value,
                None => value,
            };
            if self.exact {
                update_scaled_square(scaled, &mut self.scale, &mut self.squares)?;
            } else {
                self.squares += scaled * scaled;
                self.largest = self.largest.max(scaled.abs());
            }
        }
        if MEAN {
            crate::linalg::neumaier_add(weight * value, &mut self.mean, &mut self.correction);
        }
        Ok(())
    }

    /// Rows `first..first + count` with values `value(i)` (local index):
    /// `LANES` interleaved accumulators (plain squares, maxima, compensated
    /// means), merged in lane order, then the tail rows. The exact fallback
    /// keeps the row-by-row path.
    #[inline(always)]
    fn add_run(&mut self, first: usize, count: usize, value: impl Fn(usize) -> f64) -> Result<()> {
        if self.exact {
            for i in 0..count {
                self.add(first + i, value(i))?;
            }
            return Ok(());
        }
        let weights = self.weights.map(|weights| &weights[first..first + count]);
        let divisor = self.divisor;
        let body = count / LANES * LANES;
        let mut squares = [0.0_f64; LANES];
        let mut largest = [0.0_f64; LANES];
        let mut sums = [0.0_f64; LANES];
        let mut corrections = [0.0_f64; LANES];
        for block in (0..body).step_by(LANES) {
            for lane in 0..LANES {
                let i = block + lane;
                let v = value(i);
                let weight = weights.map_or(1.0, |weights| weights[i] / divisor);
                if NORM {
                    let scaled = match weights {
                        Some(_) => weight.sqrt() * v,
                        None => v,
                    };
                    squares[lane] += scaled * scaled;
                    largest[lane] = largest[lane].max(scaled.abs());
                }
                if MEAN {
                    let x = match weights {
                        Some(_) => weight * v,
                        None => v,
                    };
                    let t = sums[lane] + x;
                    corrections[lane] += if sums[lane].abs() >= x.abs() {
                        (sums[lane] - t) + x
                    } else {
                        (x - t) + sums[lane]
                    };
                    sums[lane] = t;
                }
            }
        }
        for lane in 0..LANES {
            if NORM {
                self.squares += squares[lane];
                self.largest = self.largest.max(largest[lane]);
            }
            if MEAN {
                crate::linalg::neumaier_add(sums[lane], &mut self.mean, &mut self.correction);
                crate::linalg::neumaier_add(
                    corrections[lane],
                    &mut self.mean,
                    &mut self.correction,
                );
            }
        }
        for i in body..count {
            self.add(first + i, value(i))?;
        }
        Ok(())
    }
}

/// Interleaved accumulators of the plain statistics path.
const LANES: usize = 4;

/// Rows per fixed chunk; chunk boundaries depend only on the row count.
const STATISTIC_ROWS: usize = 1 << 15;

/// Merge two compensated (sum, correction) partials, both components.
fn merge_compensated(left: (f64, f64), right: (f64, f64)) -> (f64, f64) {
    let (mut sum, mut correction) = left;
    crate::linalg::neumaier_add(right.0, &mut sum, &mut correction);
    crate::linalg::neumaier_add(right.1, &mut sum, &mut correction);
    (sum, correction)
}

/// Column statistics: per column, fixed row chunks over a fixed tree
/// (allocation free; the tree depends only on the row count). Norms use plain
/// sums of squares, with the exact scaled sum for a column whose largest
/// weighted magnitude lies outside `[1e-140, 1e140]`; means keep compensated
/// sums. Columns run in parallel inside a worker pool; the first failing
/// column (in order) reports.
fn column_statistics<const NORM: bool, const MEAN: bool>(
    rows: usize,
    weights: BorrowedMapWeights<'_>,
    weight_scale: super::super::PreparedOlsBreadWeightScale,
    norms: &mut [f64],
    means: &mut [f64],
    visit: impl Fn(usize, std::ops::Range<usize>, &mut ColumnStatistics<'_, NORM, MEAN>) -> Result<()>
        + Sync,
) -> Result<()> {
    let columns = if NORM { norms.len() } else { means.len() };
    if NORM && MEAN && norms.len() != means.len() {
        return Err(invalid_argument(
            "weighted-linear statistic columns are not aligned",
        ));
    }
    let raw = match weights {
        BorrowedMapWeights::Unit => None,
        BorrowedMapWeights::Provided(values) => Some(values),
    };
    // Validate the normalized weights once (first invalid row reports).
    if raw.is_some() {
        crate::parallel::fixed_tree_reduce(
            rows,
            STATISTIC_ROWS,
            &|range: std::ops::Range<usize>| -> Result<()> {
                for row in range {
                    normalized_solver_weight(weights, weight_scale, row)?;
                }
                Ok(())
            },
            &|(), ()| (),
        )?;
    }
    let divisor = weight_scale.solver_weight_divisor;
    let state = |exact| ColumnStatistics::<NORM, MEAN> {
        weights: raw,
        divisor,
        exact,
        scale: 0.0,
        squares: if exact { 1.0 } else { 0.0 },
        largest: 0.0,
        mean: 0.0,
        correction: 0.0,
    };
    let (weight_sum, weight_correction) = if MEAN {
        let (sum, correction) = crate::parallel::fixed_tree_reduce(
            rows,
            STATISTIC_ROWS,
            &|range: std::ops::Range<usize>| -> Result<(f64, f64)> {
                let statistics = state(false);
                let (mut sum, mut correction) = (0.0, 0.0);
                for row in range {
                    crate::linalg::neumaier_add(statistics.weight(row), &mut sum, &mut correction);
                }
                Ok((sum, correction))
            },
            &merge_compensated,
        )?;
        if !(sum + correction).is_finite() || sum + correction <= 0.0 {
            return Err(invalid_data("weighted-linear mean denominator is invalid"));
        }
        (sum, correction)
    } else {
        (0.0, 0.0)
    };
    let column_result = |column: usize| -> Result<(f64, f64)> {
        let (squares, largest, (mean, correction)) = crate::parallel::fixed_tree_reduce(
            rows,
            STATISTIC_ROWS,
            &|range: std::ops::Range<usize>| -> Result<(f64, f64, (f64, f64))> {
                let mut statistics = state(false);
                visit(column, range, &mut statistics)?;
                if !(statistics.mean.is_finite() && statistics.correction.is_finite()) {
                    return Err(invalid_data(
                        "weighted-linear compensated sum input is not finite",
                    ));
                }
                Ok((
                    statistics.squares,
                    statistics.largest,
                    (statistics.mean, statistics.correction),
                ))
            },
            &|(s, l, m), (s2, l2, m2)| (s + s2, l.max(l2), merge_compensated(m, m2)),
        )?;
        let norm = if !NORM || (largest == 0.0 && squares == 0.0) {
            0.0
        } else if squares.is_finite() && (1e-140..=1e140).contains(&largest) {
            squares.sqrt()
        } else {
            let mut exact = state(true);
            visit(column, 0..rows, &mut exact)?;
            stable_norm(exact.scale, exact.squares)?
        };
        let mean = if MEAN {
            let value = (mean + correction) / (weight_sum + weight_correction);
            if !value.is_finite() {
                return Err(invalid_data("weighted-linear mean is not finite"));
            }
            value
        } else {
            0.0
        };
        Ok((norm, mean))
    };
    let parallel = crate::parallel::in_pool()
        && match (NORM, MEAN) {
            (true, true) => norms
                .par_iter_mut()
                .zip(means.par_iter_mut())
                .enumerate()
                .try_for_each(|(column, (norm, mean))| {
                    (*norm, *mean) = column_result(column)?;
                    Ok::<(), FeregError>(())
                })
                .is_ok(),
            (true, false) => norms
                .par_iter_mut()
                .enumerate()
                .try_for_each(|(column, norm)| {
                    *norm = column_result(column)?.0;
                    Ok::<(), FeregError>(())
                })
                .is_ok(),
            (false, true) => means
                .par_iter_mut()
                .enumerate()
                .try_for_each(|(column, mean)| {
                    *mean = column_result(column)?.1;
                    Ok::<(), FeregError>(())
                })
                .is_ok(),
            (false, false) => true,
        };
    if !parallel {
        // Serial order also selects the first failing column's error.
        for column in 0..columns {
            let (norm, mean) = column_result(column)?;
            if NORM {
                norms[column] = norm;
            }
            if MEAN {
                means[column] = mean;
            }
        }
    }
    Ok(())
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
            |column, rows, state| {
                let source = position(column);
                let stride = values.ncols();
                let data = &values.as_slice()[rows.start * stride..rows.end * stride];
                state.add_run(rows.start, rows.len(), |i| data[i * stride + source])
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
                    |column, range, state| {
                        if let Some(values) =
                            crate::ols::prepared_fit_view::prepared_continuous_range(
                                self.prepared,
                                columns[column],
                                range.clone(),
                            )
                        {
                            return state.add_run(range.start, range.len(), |i| values[i]);
                        }
                        crate::ols::prepared_fit_view::visit_prepared_column_range(
                            self.prepared,
                            columns[column],
                            range,
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
