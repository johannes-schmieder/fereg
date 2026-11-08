//! One immutable-row-order statistics operation for cold and refreshed banks.
use super::*;

struct ColumnStatistics<'a, const NORM: bool, const MEAN: bool> {
    /// Validated normalized weights and their roots; `None` is unit weight.
    normalized: Option<&'a [f64]>,
    roots: Option<&'a [f64]>,
    /// Exact scaled sum of squares (fallback); otherwise plain sums.
    exact: bool,
    scale: f64,
    squares: f64,
    largest: f64,
    mean: f64,
    correction: f64,
}

impl<const NORM: bool, const MEAN: bool> ColumnStatistics<'_, NORM, MEAN> {
    /// Plain-path values are not checked per row: a nonfinite value makes the
    /// chunk's square sum nonfinite (sending the column to the exact pass,
    /// which reports it) and its compensated mean nonfinite (rejected when the
    /// chunks merge). The arithmetic and its order are unchanged.
    #[inline(always)]
    fn add(&mut self, row: usize, value: f64) -> Result<()> {
        if NORM {
            let scaled = match self.roots {
                Some(roots) => roots[row] * value,
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
            let weight = match self.normalized {
                Some(weights) => weights[row],
                None => 1.0,
            };
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
        let roots = self.roots.map(|roots| &roots[first..first + count]);
        let weights = self
            .normalized
            .map(|weights| &weights[first..first + count]);
        let body = count / LANES * LANES;
        let mut squares = [0.0_f64; LANES];
        let mut largest = [0.0_f64; LANES];
        let mut sums = [0.0_f64; LANES];
        let mut corrections = [0.0_f64; LANES];
        for block in (0..body).step_by(LANES) {
            for lane in 0..LANES {
                let i = block + lane;
                let v = value(i);
                if NORM {
                    let scaled = match roots {
                        Some(roots) => roots[i] * v,
                        None => v,
                    };
                    squares[lane] += scaled * scaled;
                    largest[lane] = largest[lane].max(scaled.abs());
                }
                if MEAN {
                    let x = match weights {
                        Some(weights) => weights[i] * v,
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
    // Validate and normalize the weights once (and their roots for norms)
    // instead of once per column and row.
    let (normalized, roots) = match weights {
        BorrowedMapWeights::Unit => (None, None),
        BorrowedMapWeights::Provided(_) => {
            let mut normalized = crate::memory::fallible_filled(
                rows,
                0.0_f64,
                "unable to reserve weighted-linear normalized weights",
            )?;
            normalized
                .par_chunks_mut(STATISTIC_ROWS)
                .enumerate()
                .try_for_each(|(chunk, values)| -> Result<()> {
                    for (offset, value) in values.iter_mut().enumerate() {
                        *value = normalized_solver_weight(
                            weights,
                            weight_scale,
                            chunk * STATISTIC_ROWS + offset,
                        )?;
                    }
                    Ok(())
                })?;
            let roots = if NORM {
                let mut roots = crate::memory::fallible_filled(
                    rows,
                    0.0_f64,
                    "unable to reserve weighted-linear weight roots",
                )?;
                roots
                    .par_iter_mut()
                    .zip(normalized.par_iter())
                    .for_each(|(root, weight)| *root = weight.sqrt());
                Some(roots)
            } else {
                None
            };
            (Some(normalized), roots)
        }
    };
    let (normalized, roots) = (normalized.as_deref(), roots.as_deref());
    let mut weight_sum = 0.0;
    let mut weight_correction = 0.0;
    if MEAN {
        let parts: Vec<Result<(f64, f64)>> = (0..chunks)
            .into_par_iter()
            .map(|chunk| {
                // Normalized weights are validated finite and positive; the
                // merge below rejects a nonfinite chunk sum.
                let (mut sum, mut correction) = (0.0, 0.0);
                match normalized {
                    Some(weights) => {
                        for &weight in &weights[range(chunk)] {
                            crate::linalg::neumaier_add(weight, &mut sum, &mut correction);
                        }
                    }
                    None => {
                        for _ in range(chunk) {
                            crate::linalg::neumaier_add(1.0, &mut sum, &mut correction);
                        }
                    }
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
        normalized,
        roots,
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
