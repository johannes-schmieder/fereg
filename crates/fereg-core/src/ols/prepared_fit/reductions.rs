//! Allocation-free ordered chunk reductions and the compensated weighted
//! means built on them (fixed row chunks, chunk-order merges).
use super::*;

/// Whether `predicate` holds for any fixed chunk of `values`: on the installed
/// team inside a worker pool, serially (the same predicate) outside one, so a
/// caller outside the estimation pool never injects into a global pool.
pub(crate) fn any_chunk<T: Sync>(
    values: &[T],
    chunk: usize,
    predicate: impl Fn(&[T]) -> bool + Sync + Send,
) -> bool {
    let chunk = chunk.max(1);
    if rayon::current_thread_index().is_some() {
        values.par_chunks(chunk).any(predicate)
    } else {
        values.chunks(chunk).any(predicate)
    }
}

/// Chunk results held on the stack by one round of `ordered_chunk_reduce`.
const ORDERED_ROUND_CHUNKS: usize = 64;

/// Evaluate `leaf(chunk)` for every chunk in `0..chunks` and hand each result
/// to `merge` in chunk order: the leaves, their order and the merge sequence
/// of collecting every result into a vector first, without any heap
/// allocation. Rounds of up to 64 chunks run on the installed team (serially,
/// with the identical leaves, outside a worker pool); `merge` stops at its
/// first error, which is the first error in chunk order.
pub(crate) fn ordered_chunk_reduce<T, L, M>(chunks: usize, leaf: L, mut merge: M) -> Result<()>
where
    T: Send,
    L: Fn(usize) -> T + Sync,
    M: FnMut(T) -> Result<()>,
{
    let parallel = rayon::current_thread_index().is_some();
    let mut start = 0;
    while start < chunks {
        let count = (chunks - start).min(ORDERED_ROUND_CHUNKS);
        let mut slots: [Option<T>; ORDERED_ROUND_CHUNKS] = std::array::from_fn(|_| None);
        if parallel && count > 1 {
            slots[..count]
                .par_iter_mut()
                .with_max_len(1)
                .enumerate()
                .for_each(|(offset, slot)| *slot = Some(leaf(start + offset)));
        } else {
            for (offset, slot) in slots[..count].iter_mut().enumerate() {
                *slot = Some(leaf(start + offset));
            }
        }
        for slot in &mut slots[..count] {
            merge(
                slot.take().ok_or_else(|| {
                    invalid_argument("ordered chunk reduction lost a leaf result")
                })?,
            )?;
        }
        start += count;
    }
    Ok(())
}

/// Rows per fixed chunk of the parallel weighted means; chunk boundaries
/// depend only on the row count.
pub(super) const MEAN_ROWS: usize = 1 << 14;

pub(super) fn weighted_mean_by<F>(
    nrows: usize,
    weights: NormalizedWeights<'_>,
    value: F,
) -> Result<f64>
where
    F: Fn(usize) -> Result<f64> + Sync,
{
    if nrows == 0 {
        return Err(invalid_argument(
            "prepared OLS weighted mean requires a nonempty sample",
        ));
    }
    let mut maximum = MeanMaximum::default();
    ordered_chunk_reduce(
        nrows.div_ceil(MEAN_ROWS),
        |chunk| mean_chunk_maximum(mean_chunk_rows(chunk, nrows), &value),
        |part| {
            maximum.push(part);
            Ok(())
        },
    )?;
    let value_scale = maximum.finish()?;
    weighted_mean_with_scale(nrows, weights, &value, value_scale, None)
}

/// `weighted_mean_by` for two value sequences on the same rows and weights,
/// in one maximum traversal and one sum traversal. Each result is the
/// identical value or error its own `weighted_mean_by` returns; callers keep
/// their original error order by inspecting the results in that order.
pub(super) fn weighted_mean_by_pair<F, G>(
    nrows: usize,
    weights: NormalizedWeights<'_>,
    first: F,
    second: G,
) -> (Result<f64>, Result<f64>)
where
    F: Fn(usize) -> Result<f64> + Sync,
    G: Fn(usize) -> Result<f64> + Sync,
{
    if nrows == 0 {
        let error = || {
            Err(invalid_argument(
                "prepared OLS weighted mean requires a nonempty sample",
            ))
        };
        return (error(), error());
    }
    let (mut first_maximum, mut second_maximum) = (MeanMaximum::default(), MeanMaximum::default());
    let reduced = ordered_chunk_reduce(
        nrows.div_ceil(MEAN_ROWS),
        |chunk| {
            let rows = mean_chunk_rows(chunk, nrows);
            (
                mean_chunk_maximum(rows.clone(), &first),
                mean_chunk_maximum(rows, &second),
            )
        },
        |(first_part, second_part)| {
            first_maximum.push(first_part);
            second_maximum.push(second_part);
            Ok(())
        },
    );
    if let Err(error) = reduced {
        return (Err(error.clone()), Err(error));
    }
    let scales = (first_maximum.finish(), second_maximum.finish());
    match scales {
        (Ok(first_scale), Ok(second_scale)) => weighted_mean_pair_with_scales(
            nrows,
            weights,
            (&first, first_scale, None),
            (&second, second_scale),
        ),
        (first_scale, second_scale) => (
            first_scale
                .and_then(|scale| weighted_mean_with_scale(nrows, weights, &first, scale, None)),
            second_scale
                .and_then(|scale| weighted_mean_with_scale(nrows, weights, &second, scale, None)),
        ),
    }
}

pub(super) const fn mean_chunk_rows(chunk: usize, nrows: usize) -> std::ops::Range<usize> {
    let start = chunk * MEAN_ROWS;
    let end = start + MEAN_ROWS;
    start..if end < nrows { end } else { nrows }
}

pub(super) fn mean_chunk_maximum<F>(rows: std::ops::Range<usize>, value: &F) -> Result<f64>
where
    F: Fn(usize) -> Result<f64>,
{
    let mut scale = 0.0_f64;
    for row in rows {
        let current = value(row)?;
        if !current.is_finite() {
            return Err(invalid_data(
                "prepared OLS weighted-mean value is not finite",
            ));
        }
        scale = scale.max(current.abs());
    }
    Ok(scale)
}

/// The ordered maximum merge of `weighted_mean_by`: the largest chunk
/// maximum, or the first chunk error in chunk order.
#[derive(Default)]
pub(super) struct MeanMaximum {
    value_scale: f64,
    error: Option<FeregError>,
}

impl MeanMaximum {
    pub(super) fn push(&mut self, part: Result<f64>) {
        if self.error.is_none() {
            match part {
                Ok(maximum) => self.value_scale = self.value_scale.max(maximum),
                Err(error) => self.error = Some(error),
            }
        }
    }

    pub(super) fn finish(self) -> Result<f64> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.value_scale),
        }
    }
}

pub(super) fn weighted_mean_with_bound<F>(
    nrows: usize,
    weights: NormalizedWeights<'_>,
    value: F,
    bound: f64,
) -> Result<f64>
where
    F: Fn(usize) -> Result<f64> + Sync,
{
    let mut needs_rescale = false;
    let mean = weighted_mean_with_scale(nrows, weights, &value, bound, Some(&mut needs_rescale))?;
    // Cancellation can make a valid upper bound much larger than the actual
    // residual. Recover the original maximum scan if scaling loses precision
    // to subnormal arithmetic; the optimization never just discards that tail.
    if needs_rescale {
        weighted_mean_by(nrows, weights, value)
    } else {
        Ok(mean)
    }
}

/// One chunk's numerator, numerator correction, denominator, denominator
/// correction and underflow flag.
type MeanPartial = (f64, f64, f64, f64, bool);

/// Compensated scaled weighted mean: each fixed row chunk keeps its own
/// Neumaier sums, merged in chunk order with the same compensation.
pub(super) fn weighted_mean_with_scale<F>(
    nrows: usize,
    weights: NormalizedWeights<'_>,
    value: F,
    value_scale: f64,
    underflow: Option<&mut bool>,
) -> Result<f64>
where
    F: Fn(usize) -> Result<f64> + Sync,
{
    if value_scale == 0.0 {
        return Ok(0.0);
    }
    // Unit mass sums exactly to N through 2^53.
    let unit = weights.values.is_none() && (nrows as u128) <= (1_u128 << 53);
    let mut merged = MeanMerge::default();
    ordered_chunk_reduce(
        nrows.div_ceil(MEAN_ROWS),
        |chunk| {
            let rows = mean_chunk_rows(chunk, nrows);
            unchecked_mean_chunk(rows.clone(), unit, weights, &value, value_scale).map_or_else(
                || checked_mean_chunk(rows, unit, weights, &value, value_scale),
                Ok,
            )
        },
        |part| {
            merged.push(part);
            Ok(())
        },
    )?;
    merged.finish(nrows, unit, value_scale, underflow)
}

/// Two `weighted_mean_with_scale` results from one traversal of the rows:
/// the values, terms and Neumaier updates of each are those of its own call,
/// and the weight denominator (identical in both) is accumulated once. A chunk
/// that would replay in its own call replays the same checked loop here.
pub(super) fn weighted_mean_pair_with_scales<F, G>(
    nrows: usize,
    weights: NormalizedWeights<'_>,
    first: (&F, f64, Option<&mut bool>),
    second: (&G, f64),
) -> (Result<f64>, Result<f64>)
where
    F: Fn(usize) -> Result<f64> + Sync,
    G: Fn(usize) -> Result<f64> + Sync,
{
    let (first, first_scale, underflow) = first;
    let (second, second_scale) = second;
    if first_scale == 0.0 || second_scale == 0.0 {
        return (
            weighted_mean_with_scale(nrows, weights, first, first_scale, underflow),
            weighted_mean_with_scale(nrows, weights, second, second_scale, None),
        );
    }
    let unit = weights.values.is_none() && (nrows as u128) <= (1_u128 << 53);
    let (mut first_merged, mut second_merged) = (MeanMerge::default(), MeanMerge::default());
    let reduced = ordered_chunk_reduce(
        nrows.div_ceil(MEAN_ROWS),
        |chunk| {
            let rows = mean_chunk_rows(chunk, nrows);
            let (first_part, second_part) = unchecked_mean_chunk_pair(
                rows.clone(),
                unit,
                weights,
                (first, first_scale),
                (second, second_scale),
            );
            (
                first_part.map_or_else(
                    || checked_mean_chunk(rows.clone(), unit, weights, first, first_scale),
                    Ok,
                ),
                second_part.map_or_else(
                    || checked_mean_chunk(rows, unit, weights, second, second_scale),
                    Ok,
                ),
            )
        },
        |(first_part, second_part)| {
            first_merged.push(first_part);
            second_merged.push(second_part);
            Ok(())
        },
    );
    if let Err(error) = reduced {
        return (Err(error.clone()), Err(error));
    }
    (
        first_merged.finish(nrows, unit, first_scale, underflow),
        second_merged.finish(nrows, unit, second_scale, None),
    )
}

/// Unchecked compensated sums first (value and weight errors are still
/// reported in row order by the checked replay); `None` when a term, weight
/// or sum is not finite, in which case the caller replays the checked loop.
fn unchecked_mean_chunk<F>(
    rows: std::ops::Range<usize>,
    unit: bool,
    weights: NormalizedWeights<'_>,
    value: &F,
    value_scale: f64,
) -> Option<MeanPartial>
where
    F: Fn(usize) -> Result<f64>,
{
    let (mut numerator, mut numerator_correction) = (0.0_f64, 0.0_f64);
    let (mut denominator, mut denominator_correction) = (0.0_f64, 0.0_f64);
    let mut flag = false;
    let mut finite = true;
    for row in rows {
        let Ok(current) = value(row) else {
            return None;
        };
        if unit {
            let scaled = current / value_scale;
            flag |= current != 0.0 && scaled.abs() < f64::MIN_POSITIVE;
            finite &= scaled.is_finite();
            neumaier_add(scaled, &mut numerator, &mut numerator_correction);
        } else {
            let Ok(weight) = weights.value(row) else {
                return None;
            };
            let scaled = weight * (current / value_scale);
            flag |= current != 0.0 && scaled.abs() < f64::MIN_POSITIVE;
            finite &= scaled.is_finite() && weight.is_finite();
            neumaier_add(scaled, &mut numerator, &mut numerator_correction);
            neumaier_add(weight, &mut denominator, &mut denominator_correction);
        }
    }
    (finite
        && [
            numerator,
            numerator_correction,
            denominator,
            denominator_correction,
        ]
        .iter()
        .all(|value| value.is_finite()))
    .then_some((
        numerator,
        numerator_correction,
        denominator,
        denominator_correction,
        flag,
    ))
}

/// `unchecked_mean_chunk` for two sequences sharing one weight lookup and one
/// denominator accumulation per row; each side is `None` exactly when its own
/// unchecked chunk would be.
fn unchecked_mean_chunk_pair<F, G>(
    rows: std::ops::Range<usize>,
    unit: bool,
    weights: NormalizedWeights<'_>,
    first: (&F, f64),
    second: (&G, f64),
) -> (Option<MeanPartial>, Option<MeanPartial>)
where
    F: Fn(usize) -> Result<f64>,
    G: Fn(usize) -> Result<f64>,
{
    let ((first, first_scale), (second, second_scale)) = (first, second);
    let mut sums = [(0.0_f64, 0.0_f64); 2];
    let (mut denominator, mut denominator_correction) = (0.0_f64, 0.0_f64);
    let mut flags = [false; 2];
    let mut finite = [true; 2];
    for row in rows {
        let values = [first(row), second(row)];
        let weight = if unit {
            1.0
        } else {
            match weights.value(row) {
                Ok(weight) => weight,
                Err(_) => return (None, None),
            }
        };
        for (side, (current, scale)) in values
            .into_iter()
            .zip([first_scale, second_scale])
            .enumerate()
        {
            let Ok(current) = current else {
                finite[side] = false;
                continue;
            };
            if !finite[side] {
                continue;
            }
            let scaled = if unit {
                current / scale
            } else {
                weight * (current / scale)
            };
            flags[side] |= current != 0.0 && scaled.abs() < f64::MIN_POSITIVE;
            finite[side] &= scaled.is_finite() && weight.is_finite();
            let (numerator, correction) = &mut sums[side];
            neumaier_add(scaled, numerator, correction);
        }
        if !unit {
            neumaier_add(weight, &mut denominator, &mut denominator_correction);
        }
    }
    let side = |index: usize| {
        let (numerator, correction) = sums[index];
        (finite[index]
            && [numerator, correction, denominator, denominator_correction]
                .iter()
                .all(|value| value.is_finite()))
        .then_some((
            numerator,
            correction,
            denominator,
            denominator_correction,
            flags[index],
        ))
    };
    (side(0), side(1))
}

fn checked_mean_chunk<F>(
    rows: std::ops::Range<usize>,
    unit: bool,
    weights: NormalizedWeights<'_>,
    value: &F,
    value_scale: f64,
) -> Result<MeanPartial>
where
    F: Fn(usize) -> Result<f64>,
{
    let (mut numerator, mut numerator_correction) = (0.0_f64, 0.0_f64);
    let (mut denominator, mut denominator_correction) = (0.0_f64, 0.0_f64);
    let mut flag = false;
    for row in rows {
        let current = value(row)?;
        if unit {
            let scaled = current / value_scale;
            flag |= current != 0.0 && scaled.abs() < f64::MIN_POSITIVE;
            compensated_add(scaled, &mut numerator, &mut numerator_correction)?;
        } else {
            let weight = weights.value(row)?;
            let scaled = weight * (current / value_scale);
            flag |= current != 0.0 && scaled.abs() < f64::MIN_POSITIVE;
            compensated_add(scaled, &mut numerator, &mut numerator_correction)?;
            compensated_add(weight, &mut denominator, &mut denominator_correction)?;
        }
    }
    Ok((
        numerator,
        numerator_correction,
        denominator,
        denominator_correction,
        flag,
    ))
}

/// The ordered compensated merge of mean chunk partials; the first error in
/// chunk order (a chunk's own error or a nonfinite merge) is kept.
#[derive(Default)]
struct MeanMerge {
    numerator: f64,
    numerator_correction: f64,
    denominator: f64,
    denominator_correction: f64,
    flag: bool,
    error: Option<FeregError>,
}

impl MeanMerge {
    fn push(&mut self, part: Result<MeanPartial>) {
        if self.error.is_some() {
            return;
        }
        let merged = part.and_then(|(n, nc, d, dc, f)| {
            compensated_add(n, &mut self.numerator, &mut self.numerator_correction)?;
            compensated_add(nc, &mut self.numerator, &mut self.numerator_correction)?;
            compensated_add(d, &mut self.denominator, &mut self.denominator_correction)?;
            compensated_add(dc, &mut self.denominator, &mut self.denominator_correction)?;
            self.flag |= f;
            Ok(())
        });
        if let Err(error) = merged {
            self.error = Some(error);
        }
    }

    fn finish(
        self,
        nrows: usize,
        unit: bool,
        value_scale: f64,
        underflow: Option<&mut bool>,
    ) -> Result<f64> {
        if let Some(error) = self.error {
            return Err(error);
        }
        merge_mean_totals(self, nrows, unit, value_scale, underflow)
    }
}

fn merge_mean_totals(
    merged: MeanMerge,
    nrows: usize,
    unit: bool,
    value_scale: f64,
    underflow: Option<&mut bool>,
) -> Result<f64> {
    let MeanMerge {
        numerator,
        numerator_correction,
        denominator,
        denominator_correction,
        flag,
        ..
    } = merged;
    let denominator = if unit {
        nrows as f64
    } else {
        denominator + denominator_correction
    };
    let numerator = numerator + numerator_correction;
    let scaled_mean = numerator / denominator;
    if let Some(underflow) = underflow {
        *underflow |= flag || (numerator != 0.0 && scaled_mean.abs() < f64::MIN_POSITIVE);
    }
    let mean = value_scale * scaled_mean;
    if mean.is_finite() && denominator.is_finite() && denominator > 0.0 {
        Ok(mean)
    } else {
        Err(invalid_data("prepared OLS weighted mean is not finite"))
    }
}
