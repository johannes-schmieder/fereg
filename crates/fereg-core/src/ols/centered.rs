//! Bounded, deterministic summaries and an immutable view of centered raw rows.
//!
//! This is numerical preparation, not a rank or estimator policy. Callers retain
//! the original matrix and must use the compatible path when construction fails
//! or its independently measured residual-mean certificate is insufficient.
//! There are three scans: raw norms/extrema, anchored means, then actual centered
//! norms/means. The partial bank is reused; no centered matrix is allocated.

use super::prepared_fit::FitReservation;
use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, invalid_data, Result};
use rayon::prelude::*;

const LEAF_ROWS: usize = 16_384;
const MAX_LEAVES: usize = 64;
const RESULT_VECTORS: usize = 11;

#[derive(Clone, Copy, Debug)]
struct Partial {
    scale: f64,
    squares: f64,
    sum: f64,
    correction: f64,
}

impl Default for Partial {
    fn default() -> Self {
        Self {
            scale: 0.0,
            squares: 1.0,
            sum: 0.0,
            correction: 0.0,
        }
    }
}

impl Partial {
    fn add(&mut self, value: f64) {
        crate::linalg::neumaier_add(value, &mut self.sum, &mut self.correction);
    }

    fn norm_add(&mut self, value: f64) {
        let magnitude = value.abs();
        if magnitude == 0.0 {
            return;
        }
        if self.scale < magnitude {
            let ratio = self.scale / magnitude;
            self.squares = 1.0 + self.squares * ratio * ratio;
            self.scale = magnitude;
        } else {
            let ratio = magnitude / self.scale;
            self.squares += ratio * ratio;
        }
    }

    fn norm_merge(&mut self, other: Self) {
        if other.scale == 0.0 {
            return;
        }
        if self.scale < other.scale {
            let ratio = self.scale / other.scale;
            self.squares = other.squares + self.squares * ratio * ratio;
            self.scale = other.scale;
        } else {
            let ratio = other.scale / self.scale;
            self.squares += other.squares * ratio * ratio;
        }
    }

    fn norm(self) -> Result<f64> {
        finite(
            self.scale * self.squares.sqrt(),
            "centered summary norm is not finite",
        )
    }
}

/// Bound to one immutable original row-major allocation, including response in
/// column zero. Centers are unevaluated high/low pairs in original units.
#[derive(Debug)]
pub(crate) struct CenteredSummary {
    rows: usize,
    source: usize,
    raw_norms: Vec<f64>,
    maxima: Vec<f64>,
    minimum_values: Vec<f64>,
    maximum_values: Vec<f64>,
    means: Vec<f64>,
    centers_hi: Vec<f64>,
    centers_lo: Vec<f64>,
    within_norms: Vec<f64>,
    centered_means: Vec<f64>,
    centered_relative_moments: Vec<f64>,
    units: Vec<f64>,
    direct_centering: bool,
}

impl CenteredSummary {
    /// Peak owned payload: retained result plus the simultaneously live partial
    /// bank. Excludes the caller's original matrix and allocator bookkeeping.
    pub(crate) fn workspace_bytes(rows: usize, columns: usize) -> Result<u128> {
        shape(rows, columns)?;
        let partials = (leaves(rows) as u128)
            .checked_mul(columns as u128)
            .and_then(|n| n.checked_mul(std::mem::size_of::<Partial>() as u128));
        let retained =
            (columns as u128).checked_mul((RESULT_VECTORS * std::mem::size_of::<f64>()) as u128);
        partials
            .and_then(|n| retained.and_then(|r| n.checked_add(r)))
            .ok_or_else(|| allocation("centered summary workspace size overflow"))
    }

    pub(super) fn build(raw: &DenseMatrix, reservation: &mut impl FitReservation) -> Result<Self> {
        let columns = raw.ncols();
        Self::workspace_bytes(raw.nrows(), columns)?;
        crate::interrupt::checkpoint()?;
        let mut result = Self {
            rows: raw.nrows(),
            source: raw.as_slice().as_ptr() as usize,
            raw_norms: reserved(reservation, columns, 0.0)?,
            maxima: reserved(reservation, columns, 0.0)?,
            minimum_values: reserved(reservation, columns, 0.0)?,
            maximum_values: reserved(reservation, columns, 0.0)?,
            means: reserved(reservation, columns, 0.0)?,
            centers_hi: reserved(reservation, columns, 0.0)?,
            centers_lo: reserved(reservation, columns, 0.0)?,
            within_norms: reserved(reservation, columns, 0.0)?,
            centered_means: reserved(reservation, columns, 0.0)?,
            centered_relative_moments: reserved(reservation, columns, 0.0)?,
            units: reserved(reservation, columns, 1.0)?,
            direct_centering: true,
        };
        let count = leaves(raw.nrows())
            .checked_mul(columns)
            .ok_or_else(|| allocation("centered summary partial size overflow"))?;
        // Reuse the compensated-sum fields as extrema during the first scan.
        // Every physical leaf is nonempty; sentinels cannot survive its fold.
        let initial = Partial {
            sum: f64::INFINITY,
            correction: f64::NEG_INFINITY,
            ..Partial::default()
        };
        let mut partials = reserved(reservation, count, initial)?;
        scan(raw, &mut partials, |value, _, state| {
            finite(value, "centered summary raw value is not finite")?;
            state.norm_add(value);
            if value < state.sum {
                state.sum = value;
            }
            if value > state.correction {
                state.correction = value;
            }
            Ok(())
        })?;
        for column in 0..columns {
            let norm = fold_norm(&partials, columns, column);
            result.maxima[column] = norm.scale;
            result.raw_norms[column] = norm.norm()?;
            result.units[column] = power_unit(norm.scale);
            let mut minimum = f64::INFINITY;
            let mut maximum = f64::NEG_INFINITY;
            for leaf in partials.chunks_exact(columns) {
                if leaf[column].sum < minimum {
                    minimum = leaf[column].sum;
                }
                if leaf[column].correction > maximum {
                    maximum = leaf[column].correction;
                }
            }
            result.minimum_values[column] = minimum;
            result.maximum_values[column] = maximum;
        }
        partials.fill(Partial::default());
        scan(raw, &mut partials, |value, column, state| {
            let unit = result.units[column];
            let scaled = value / unit;
            if scaled * unit != value {
                return Err(invalid_data(
                    "centered summary scaling lost a nonzero component",
                ));
            }
            let anchor = raw[(0, column)] / unit;
            let (difference, error) = two_sum(scaled, -anchor);
            state.add(difference);
            state.add(error);
            Ok(())
        })?;
        for column in 0..columns {
            let sum = fold_sum(&partials, columns, column);
            let rows = raw.nrows() as f64;
            let quotient = sum.sum / rows;
            let remainder = (-quotient).mul_add(rows, sum.sum) + sum.correction;
            let unit = result.units[column];
            let (high, low) = two_sum(raw[(0, column)] / unit, quotient);
            let (high, low) = two_sum(high, low + remainder / rows);
            let (high, low) = two_sum(high * unit, low * unit);
            result.centers_hi[column] = finite(high, "centered summary center is not finite")?;
            result.centers_lo[column] = finite(low, "centered summary center is not finite")?;
            result.means[column] = finite(high + low, "centered summary mean is not finite")?;
            result.direct_centering &= (result.minimum_values[column] - high).is_finite()
                && (result.maximum_values[column] - high).is_finite();
        }
        partials.fill(Partial::default());
        scan(raw, &mut partials, |value, column, state| {
            let centered = result.center_value(value, column)?;
            state.norm_add(centered);
            let scaled = centered / result.units[column];
            if scaled * result.units[column] != centered {
                return Err(invalid_data(
                    "centered summary residual scaling lost a nonzero component",
                ));
            }
            state.add(scaled);
            Ok(())
        })?;
        for column in 0..columns {
            result.within_norms[column] = fold_norm(&partials, columns, column).norm()?;
            let sum = fold_sum(&partials, columns, column);
            let scaled_mean = (sum.sum + sum.correction) / raw.nrows() as f64;
            result.centered_means[column] = finite(
                scaled_mean * result.units[column],
                "centered summary residual mean is not finite",
            )?;
            let denominator =
                result.raw_norms[column].max(result.within_norms[column]) / result.units[column];
            let relative = if denominator == 0.0 {
                0.0
            } else {
                scaled_mean.abs() / denominator * (raw.nrows() as f64).sqrt()
            };
            result.centered_relative_moments[column] = finite(
                relative,
                "centered summary residual certificate is not finite",
            )?;
        }
        crate::interrupt::checkpoint()?;
        Ok(result)
    }

    pub(crate) fn raw_norms(&self) -> &[f64] {
        &self.raw_norms
    }
    pub(crate) fn maxima(&self) -> &[f64] {
        &self.maxima
    }
    /// Minimum observed signed value, including response in column zero.
    pub(crate) fn minimum_values(&self) -> &[f64] {
        &self.minimum_values
    }
    /// Maximum observed signed value; `maxima` instead contains magnitudes.
    pub(crate) fn maximum_values(&self) -> &[f64] {
        &self.maximum_values
    }
    pub(crate) fn means(&self) -> &[f64] {
        &self.means
    }
    pub(crate) fn within_norms(&self) -> &[f64] {
        &self.within_norms
    }
    pub(crate) fn centered_means(&self) -> &[f64] {
        &self.centered_means
    }

    /// Absolute mean of the actual evaluated centered values, divided by
    /// max(raw norm, centered norm), times sqrt(N). Computed in scaled units
    /// without first rounding the physical mean (which can underflow to zero).
    pub(crate) fn centered_relative_moments(&self) -> &[f64] {
        &self.centered_relative_moments
    }

    pub(crate) fn resident_bytes(&self) -> u128 {
        [
            &self.raw_norms,
            &self.maxima,
            &self.minimum_values,
            &self.maximum_values,
            &self.means,
            &self.centers_hi,
            &self.centers_lo,
            &self.within_norms,
            &self.centered_means,
            &self.centered_relative_moments,
            &self.units,
        ]
        .iter()
        .map(|values| values.capacity() as u128 * 8)
        .sum()
    }

    #[inline(always)]
    fn center_value(&self, value: f64, column: usize) -> Result<f64> {
        let (difference, error) = two_sum(value, -self.centers_hi[column]);
        let centered = if difference.is_finite() {
            let (high, low) = two_sum(difference, -self.centers_lo[column]);
            high + (error + low)
        } else {
            let unit = self.units[column];
            let (high, low) = two_sum(value / unit, -self.centers_hi[column] / unit);
            (high + (low - self.centers_lo[column] / unit)) * unit
        };
        finite(centered, "centered row value is not finite")
    }
}

/// Read-only view; the owner must not change raw values after summary creation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CenteredRows<'a> {
    raw: &'a DenseMatrix,
    summary: &'a CenteredSummary,
}

impl<'a> CenteredRows<'a> {
    pub(crate) fn new(raw: &'a DenseMatrix, summary: &'a CenteredSummary) -> Result<Self> {
        if raw.nrows() != summary.rows
            || raw.ncols() != summary.means.len()
            || raw.as_slice().as_ptr() as usize != summary.source
        {
            return Err(invalid_argument(
                "centered rows do not match their summary owner",
            ));
        }
        Ok(Self { raw, summary })
    }

    pub(crate) fn rows(self) -> usize {
        self.raw.nrows()
    }
    pub(crate) fn columns(self) -> usize {
        self.raw.ncols() - 1
    }
    pub(crate) fn raw(self) -> &'a DenseMatrix {
        self.raw
    }
    pub(crate) fn summary(self) -> &'a CenteredSummary {
        self.summary
    }
    pub(crate) fn response(self, row: usize) -> Result<f64> {
        self.at(row, 0)
    }

    pub(crate) fn value(self, row: usize, generated_column: usize) -> Result<f64> {
        if generated_column >= self.columns() {
            return Err(invalid_argument("centered row column is out of bounds"));
        }
        self.at(row, generated_column + 1)
    }

    pub(crate) fn fill_row(self, row: usize, output: &mut [f64]) -> Result<()> {
        if row >= self.rows() || output.len() != self.columns() {
            return Err(invalid_argument("centered row output shape is invalid"));
        }
        for (column, target) in output.iter_mut().enumerate() {
            *target = self
                .summary
                .center_value(self.raw[(row, column + 1)], column + 1)?;
        }
        Ok(())
    }

    fn at(self, row: usize, column: usize) -> Result<f64> {
        if row >= self.rows() {
            return Err(invalid_argument("centered row index is out of bounds"));
        }
        self.summary.center_value(self.raw[(row, column)], column)
    }
}

fn reserved<T: Clone>(
    reservation: &mut impl FitReservation,
    count: usize,
    value: T,
) -> Result<Vec<T>> {
    let mut result = Vec::new();
    reservation.reserve(
        &mut result,
        count,
        "unable to reserve centered summary workspace",
    )?;
    if result.capacity() < count {
        return Err(allocation("centered summary reservation is undersized"));
    }
    result.resize(count, value);
    Ok(result)
}

fn shape(rows: usize, columns: usize) -> Result<()> {
    if rows == 0 || columns == 0 {
        return Err(invalid_argument(
            "centered summary requires nonempty raw rows",
        ));
    }
    rows.checked_mul(columns)
        .filter(|&n| n <= isize::MAX as usize / 8)
        .ok_or_else(|| allocation("centered summary matrix size overflow"))?;
    Ok(())
}

fn leaves(rows: usize) -> usize {
    rows.div_ceil(LEAF_ROWS).clamp(1, MAX_LEAVES)
}

fn scan<F>(raw: &DenseMatrix, partials: &mut [Partial], operation: F) -> Result<()>
where
    F: Fn(f64, usize, &mut Partial) -> Result<()> + Sync + Send,
{
    let columns = raw.ncols();
    let count = leaves(raw.nrows());
    crate::parallel::stage_try_for_each(
        partials.par_chunks_mut(columns).enumerate(),
        raw.as_slice().len() as u128,
        |(leaf, states)| {
            let base = raw.nrows() / count;
            let tail = raw.nrows() % count;
            let start = leaf * base + leaf.min(tail);
            let end = start + base + usize::from(leaf < tail);
            crate::interrupt::checkpoint()?;
            for row in start..end {
                crate::interrupt::row_checkpoint(row)?;
                for (column, state) in states.iter_mut().enumerate() {
                    operation(raw[(row, column)], column, state)?;
                }
            }
            Ok(())
        },
    )
}

fn fold_norm(partials: &[Partial], columns: usize, column: usize) -> Partial {
    let mut result = Partial::default();
    for leaf in partials.chunks_exact(columns) {
        result.norm_merge(leaf[column]);
    }
    result
}

fn fold_sum(partials: &[Partial], columns: usize, column: usize) -> Partial {
    let mut result = Partial::default();
    for leaf in partials.chunks_exact(columns) {
        result.add(leaf[column].sum);
        result.add(leaf[column].correction);
    }
    result
}

fn power_unit(maximum: f64) -> f64 {
    let exponent = maximum.to_bits() & 0x7ff0_0000_0000_0000;
    if exponent == 0 {
        f64::MIN_POSITIVE
    } else {
        f64::from_bits(exponent)
    }
}

#[inline(always)]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let part = sum - a;
    (sum, (a - (sum - part)) + (b - part))
}

fn finite(value: f64, message: &'static str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid_data(message))
    }
}

#[path = "centered/packing.rs"]
mod packing;

#[cfg(test)]
#[path = "centered/tests.rs"]
mod tests;
