//! One immutable-row-order statistics operation for cold and refreshed banks.
use super::*;

struct ColumnStatistics<'a, const NORM: bool, const MEAN: bool> {
    weights: BorrowedMapWeights<'a>,
    weight_scale: super::super::PreparedOlsBreadWeightScale,
    /// Exact scaled sum of squares (fallback); otherwise plain sums.
    exact: bool,
    scale: f64,
    squares: f64,
    largest: f64,
    mean: f64,
    correction: f64,
}

impl<const NORM: bool, const MEAN: bool> ColumnStatistics<'_, NORM, MEAN> {
    fn add(&mut self, row: usize, value: f64) -> Result<()> {
        let weight = normalized_solver_weight(self.weights, self.weight_scale, row)?;
        if NORM {
            let scaled = weight.sqrt() * value;
            if self.exact {
                update_scaled_square(scaled, &mut self.scale, &mut self.squares)?;
            } else {
                if !scaled.is_finite() {
                    return Err(invalid_data("weighted-linear norm value is not finite"));
                }
                self.squares += scaled * scaled;
                self.largest = self.largest.max(scaled.abs());
            }
        }
        if MEAN {
            compensated_add(weight * value, &mut self.mean, &mut self.correction)?;
        }
        Ok(())
    }
}

/// Rows per fixed chunk; chunk boundaries depend only on the row count.
const STATISTIC_ROWS: usize = 1 << 15;

/// Column statistics over (column, fixed row chunk) tasks. Norms use plain
/// sums of squares merged in chunk order, with the exact scaled sum for a
/// column whose largest weighted magnitude lies outside `[1e-140, 1e140]`;
/// means keep compensated chunk sums merged in order.
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
    let chunks = rows.div_ceil(STATISTIC_ROWS).max(1);
    let range = |chunk: usize| chunk * STATISTIC_ROWS..((chunk + 1) * STATISTIC_ROWS).min(rows);
    let mut weight_sum = 0.0;
    let mut weight_correction = 0.0;
    if MEAN {
        let parts: Vec<Result<(f64, f64)>> = (0..chunks)
            .into_par_iter()
            .map(|chunk| {
                let (mut sum, mut correction) = (0.0, 0.0);
                for row in range(chunk) {
                    compensated_add(
                        normalized_solver_weight(weights, weight_scale, row)?,
                        &mut sum,
                        &mut correction,
                    )?;
                }
                Ok((sum, correction))
            })
            .collect();
        for part in parts {
            let (sum, correction) = part?;
            compensated_add(sum, &mut weight_sum, &mut weight_correction)?;
            compensated_add(correction, &mut weight_sum, &mut weight_correction)?;
        }
        if !(weight_sum + weight_correction).is_finite() || weight_sum + weight_correction <= 0.0 {
            return Err(invalid_data("weighted-linear mean denominator is invalid"));
        }
    }
    let state = |exact| ColumnStatistics::<NORM, MEAN> {
        weights,
        weight_scale,
        exact,
        scale: 0.0,
        squares: if exact { 1.0 } else { 0.0 },
        largest: 0.0,
        mean: 0.0,
        correction: 0.0,
    };
    let parts: Vec<Result<(f64, f64, f64, f64)>> = (0..columns * chunks)
        .into_par_iter()
        .map(|task| {
            let (column, chunk) = (task / chunks, task % chunks);
            let mut statistics = state(false);
            visit(column, range(chunk), &mut statistics)?;
            Ok((
                statistics.squares,
                statistics.largest,
                statistics.mean,
                statistics.correction,
            ))
        })
        .collect();
    let mut parts = parts.into_iter();
    for column in 0..columns {
        let (mut squares, mut largest) = (0.0_f64, 0.0_f64);
        let (mut mean, mut correction) = (0.0, 0.0);
        for _ in 0..chunks {
            let (s, l, m, c) = parts.next().expect("one task per column chunk")?;
            squares += s;
            largest = largest.max(l);
            compensated_add(m, &mut mean, &mut correction)?;
            compensated_add(c, &mut mean, &mut correction)?;
        }
        if NORM {
            norms[column] = if largest == 0.0 && squares == 0.0 {
                0.0
            } else if squares.is_finite() && (1e-140..=1e140).contains(&largest) {
                squares.sqrt()
            } else {
                let mut exact = state(true);
                visit(column, 0..rows, &mut exact)?;
                stable_norm(exact.scale, exact.squares)?
            };
        }
        if MEAN {
            let value = (mean + correction) / (weight_sum + weight_correction);
            if !value.is_finite() {
                return Err(invalid_data("weighted-linear mean is not finite"));
            }
            means[column] = value;
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
                for row in rows {
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
                    |column, range, state| {
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
