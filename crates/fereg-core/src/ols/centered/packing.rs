//! Exact bulk preparation: retain scalar high/low arithmetic, check shapes
//! once per bounded panel, and expose contiguous independent column values.
use super::*;

impl CenteredRows<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pack_panel(
        self,
        first: usize,
        rows: usize,
        selected: Option<&[usize]>,
        scales: Option<&[f64]>,
        response_scale: f64,
        panel: &mut [f64],
        row: &mut [f64],
    ) -> Result<()> {
        let columns = selected.map_or(self.columns(), <[usize]>::len);
        let width = self.raw.ncols();
        if rows == 0
            || first.checked_add(rows).is_none_or(|end| end > self.rows())
            || row.len() != width
            || (columns + 1)
                .checked_mul(rows)
                .is_none_or(|n| panel.len() < n)
            || selected.is_some_and(|ids| ids.iter().any(|&id| id >= self.columns()))
            || scales.is_some_and(|s| {
                s.len() != columns || s.iter().any(|v| !v.is_finite() || *v <= 0.0)
            })
            || !response_scale.is_finite()
            || response_scale <= 0.0
        {
            return Err(invalid_argument("centered panel shape or scale is invalid"));
        }
        crate::interrupt::checkpoint()?;
        // The original extrema enclose every immutable source value. Finite
        // subtraction at both endpoints proves the first two_sum sum finite
        // throughout. The remaining operations retain the original order.
        // Rare extreme-range data use the original scalar scaled fallback.
        if !self.summary.direct_centering {
            return self.pack_scalar(first, rows, selected, scales, response_scale, panel, row);
        }
        let mut finite_values = true;
        let raw = &self.raw.as_slice()[first * width..(first + rows) * width];
        for (offset, source) in raw.chunks_exact(width).enumerate() {
            for (((&value, target), &high), &low) in source
                .iter()
                .zip(row.iter_mut())
                .zip(&self.summary.centers_hi)
                .zip(&self.summary.centers_lo)
            {
                let (difference, error) = two_sum(value, -high);
                let (high, low) = two_sum(difference, -low);
                *target = high + (error + low);
                finite_values &= target.is_finite();
            }
            for column in 0..columns {
                let source = selected.map_or(column, |ids| ids[column]) + 1;
                let value = scales.map_or(row[source], |scales| row[source] / scales[column]);
                panel[column * rows + offset] = value;
                finite_values &= value.is_finite();
            }
            let value = row[0] / response_scale;
            panel[columns * rows + offset] = value;
            finite_values &= value.is_finite();
        }
        if finite_values {
            Ok(())
        } else {
            // Replay only a failed bounded tile in the former row/column
            // order, preserving its exact numerical error and precedence.
            self.pack_scalar(first, rows, selected, scales, response_scale, panel, row)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn pack_scalar(
        self,
        first: usize,
        rows: usize,
        selected: Option<&[usize]>,
        scales: Option<&[f64]>,
        response_scale: f64,
        panel: &mut [f64],
        row: &mut [f64],
    ) -> Result<()> {
        let columns = selected.map_or(self.columns(), <[usize]>::len);
        for offset in 0..rows {
            self.fill_row(first + offset, &mut row[1..])?;
            for column in 0..columns {
                let source = selected.map_or(column, |ids| ids[column]) + 1;
                let value = scales.map_or(row[source], |scales| row[source] / scales[column]);
                panel[column * rows + offset] =
                    finite(value, "finite-WLS standardized column is not finite")?;
            }
            panel[columns * rows + offset] = self.response(first + offset)? / response_scale;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "packing_tests.rs"]
mod tests;
