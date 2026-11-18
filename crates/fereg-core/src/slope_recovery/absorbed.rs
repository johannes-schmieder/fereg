//! Center an ordinary slope within a declared categorical intercept partition.
//! Only the first declared FE carries the component means in saved outputs.
//! Transferring a level function to this anchor preserves every normalization.
use super::{mapping::PairMap, Column};
use crate::data::PreparedWeights;
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::fe::CategoricalTopologySource;
use crate::interrupt::row_checkpoint;
use crate::linalg::neumaier_add;
use crate::memory::fallible_filled;
use crate::ols::PreparedOlsData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsorbedMap {
    pub slot: usize,
    pub scale: f64,
    pub mean_anchor: f64,
}

#[derive(Debug, PartialEq)]
pub struct AbsorbedRecovery {
    pub target_column: usize,
    pub map: AbsorbedMap,
    storage: usize,
    original: Vec<f64>,
    anchors: Vec<f64>,
}

fn inaccurate() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "original-unit slope and absorbed effects cannot reproduce the certified fit",
    )
}

impl AbsorbedMap {
    /// A normalized OLS constant absorbs the preparation-weighted level mean.
    pub fn normalized_ols_map(self, constant: usize) -> PairMap {
        PairMap {
            source: constant,
            target: self.slot,
            multiple: self.mean_anchor,
            scale: self.scale,
        }
    }

    /// PPML reports its structural constant as zero. Only one public slope
    /// changes units. Require *exact* inverse scaling on the b and V doubles.
    pub fn restore(self, b: &mut [f64], v: &mut [f64]) -> Result<()> {
        let n = b.len();
        if self.slot >= n || n.checked_mul(n) != Some(v.len()) {
            return Err(invalid_argument("invalid absorbed-slope result dimensions"));
        }
        let scale = self.scale;
        let checked = |value| {
            let mapped: f64 = value * scale;
            if mapped.is_finite() && mapped / scale == value {
                Ok(mapped)
            } else {
                Err(inaccurate())
            }
        };
        let z = self.slot;
        let beta = checked(b[z])?;
        let variance = checked(checked(v[z * n + z])?)?;
        for i in 0..n {
            checked(v[z * n + i])?;
            if v[z * n + i] != v[i * n + z] {
                return Err(inaccurate());
            }
        }
        b[z] = beta;
        for i in 0..n {
            if i != z {
                v[z * n + i] *= scale;
                v[i * n + z] = v[z * n + i];
            }
        }
        v[z * n + z] = variance;
        Ok(())
    }
}

impl AbsorbedRecovery {
    pub fn owned_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.original)
            + crate::memory::vec_capacity_bytes(&self.anchors)
    }
    pub fn original_target(&self, row: usize) -> f64 {
        self.original[row]
    }

    pub fn row_adjustment(&self, row: usize, raw_beta: f64, center: bool) -> f64 {
        let mean = if center { self.map.mean_anchor } else { 0.0 };
        (self.anchors[row] - mean) * raw_beta
    }

    pub fn restore_design(mut self, prepared: &mut PreparedOlsData) -> Result<()> {
        prepared
            .sample
            .swap_recovery_column(self.storage, &mut self.original)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn predictions(
        &self,
        data: &PreparedOlsData,
        stable_b: &[f64],
        stable_xb: &[f64],
        stable_d: &[f64],
        offset: &[f64],
        eta: &[f64],
        tolerance: f64,
    ) -> Result<(Vec<f64>, Vec<f64>)> {
        let n = self.original.len();
        if [
            stable_xb.len(),
            stable_d.len(),
            eta.len(),
            data.sample().n_active(),
        ]
        .iter()
        .any(|&size| size != n)
            || (!offset.is_empty() && offset.len() != n)
            || stable_b.len() != data.report_plan().original_coefficient_count()
        {
            return Err(invalid_argument(
                "invalid absorbed-slope prediction dimensions",
            ));
        }
        let mut xb = fallible_filled(n, 0.0, "unable to reserve original finite predictor")?;
        let mut d = fallible_filled(n, 0.0, "unable to reserve original absorbed predictor")?;
        let raw_beta = stable_b[self.map.slot] * self.map.scale;
        // Compute the posted dot product in column order, including the original
        // weak regressor. The producer and Stata can parenthesize the offset
        // differently; certify both sums on the log-mean scale.
        for row in 0..n {
            row_checkpoint(row)?;
            let mut fused = 0.0;
            for (j, column) in data.model().columns.iter().enumerate() {
                let (value, beta) = if j == self.target_column {
                    (self.original[row], raw_beta)
                } else {
                    let slot = data
                        .report_plan()
                        .original_slot(column.coefficient_slot)?
                        .get() as usize
                        - 1;
                    (data.column_value(j, row)?, stable_b[slot])
                };
                xb[row] += value * beta;
                fused = value.mul_add(beta, fused);
            }
            let shift = self.row_adjustment(row, raw_beta, false);
            d[row] = stable_d[row] - shift;
            let o = offset.get(row).copied().unwrap_or(0.);
            for error in [
                (xb[row] - shift) - stable_xb[row],
                (fused - shift) - stable_xb[row],
                (xb[row] + o + d[row]) - eta[row],
                (xb[row] + d[row] + o) - eta[row],
                (fused + o + d[row]) - eta[row],
                (fused + d[row] + o) - eta[row],
            ] {
                if !error.is_finite() || error.exp_m1().abs() > tolerance.min(1e-8) {
                    return Err(inaccurate());
                }
            }
        }
        Ok((xb, d))
    }

    /// Apply the exact level-function change to the anchor after recovering
    /// *all* conventionally normalized channels under the strict Hessian.
    /// Recheck their binary64 sum against the original D in log-mean units.
    pub(crate) fn saved_channels(
        &self,
        channels: &mut [Option<Vec<f64>>],
        stable_beta: f64,
        raw_d: &[f64],
        tolerance: f64,
    ) -> Result<f64> {
        if raw_d.len() != self.anchors.len()
            || channels.is_empty()
            || channels
                .iter()
                .any(|c| c.as_ref().is_none_or(|v| v.len() != raw_d.len()))
        {
            return Err(invalid_argument(
                "invalid absorbed-slope saved FE dimensions",
            ));
        }
        let raw_beta = stable_beta * self.map.scale;
        let mut maximum = 0.0_f64;
        for (row, &target) in raw_d.iter().enumerate() {
            row_checkpoint(row)?;
            let anchor_channel = channels[0].as_mut().unwrap();
            let previous = anchor_channel[row];
            let shift = self.row_adjustment(row, raw_beta, false);
            anchor_channel[row] -= shift;
            let inverse_error = (anchor_channel[row] + shift) - previous;
            if !inverse_error.is_finite() || inverse_error.abs().exp_m1() > tolerance.min(1e-8) {
                return Err(inaccurate());
            }
            let total: f64 = channels.iter().map(|c| c.as_ref().unwrap()[row]).sum();
            let error = (total - target).abs();
            if !error.is_finite() || error.exp_m1() > tolerance.min(1e-8) {
                return Err(inaccurate());
            }
            maximum = maximum.max(error);
        }
        Ok(maximum)
    }
}

// A sparse bounded screen rejects ordinary regressors without constructing any
// scratch. The threshold only proposes coordinates. Every accepted subtraction
// and power-of-two scaling is checked on every observation.
fn plausible(values: &[f64], effect: &CategoricalTopologySource, boundary: f64) -> Result<bool> {
    let mut departure = 0.0_f64;
    for i in 0..effect.n_levels().min(12) {
        let group = i * (effect.n_levels() - 1) / (effect.n_levels().min(12) - 1).max(1);
        let rows = effect.members(group)?;
        let Some(&first_row) = rows.first() else {
            continue;
        };
        let first = values[first_row];
        for &r in [rows[0], rows[rows.len() / 2], rows[rows.len() - 1]].iter() {
            departure = departure.max((values[r] - first).abs());
        }
    }
    Ok(departure <= boundary)
}

fn geometry(
    values: &[f64],
    effect: &CategoricalTopologySource,
    boundary: f64,
) -> Result<Option<f64>> {
    let mut departure = 0.0_f64;
    let mut visited = 0;
    for level in 0..effect.n_levels() {
        let rows = effect.members(level)?;
        let Some(&first_row) = rows.first() else {
            continue;
        };
        let mut min = values[first_row];
        let mut max = min;
        for &row in rows {
            row_checkpoint(visited)?;
            visited += 1;
            min = min.min(values[row]);
            max = max.max(values[row]);
        }
        departure = departure.max(max - min);
        if departure > boundary {
            return Ok(None);
        }
    }
    if !departure.is_normal() {
        return Ok(None);
    }
    let exponent = (departure.to_bits() >> 52) as i32 - 1023;
    Ok(Some(2.0_f64.powi(-exponent)))
}

pub fn prepare_absorbed(
    data: &mut PreparedOlsData,
    ceiling: u128,
) -> Result<Option<AbsorbedRecovery>> {
    prepare_with_sketches(data, ceiling, &[])
}

pub(super) fn prepare_with_sketches(
    data: &mut PreparedOlsData,
    ceiling: u128,
    pair_columns: &[Column],
) -> Result<Option<AbsorbedRecovery>> {
    let n = data.sample().n_active();
    if n < 2
        || data.effect_specs().is_empty()
        || data
            .effect_specs()
            .iter()
            .any(|e| !e.intercept || !e.slopes.is_empty())
    {
        return Ok(None);
    }
    let topology = data.fe_topology();
    let effect = topology
        .registry()
        .source(topology.effect_to_topology()[0])?;
    if effect.n_levels() < 2 {
        return Ok(None);
    }
    // Charge both possible scans for every eligible column, regardless of
    // the probes. Row order then cannot exhaust a later candidate's budget.
    let candidates_allowed = ((32_usize << 20) / n / 2).min(super::SEARCH_WIDTH);
    let mut candidates = 0;
    for j in 0..data.model().columns.len() {
        let Some(column) = Column::identify(data, j) else {
            continue;
        };
        if !column.mutable {
            continue;
        }
        candidates += 1;
        if candidates > candidates_allowed {
            return Ok(None);
        }
        let values = data
            .sample()
            .regressors
            .get(column.storage)
            .ok_or_else(|| invalid_argument("missing absorbed-slope source"))?;
        let extrema = match pair_columns.iter().find(|c| c.generated == j) {
            Some(c) => c.sketch,
            None => super::screen::Sketch::new(values)?,
        };
        let boundary = (extrema.maximum - extrema.minimum) / 8192.;
        if !boundary.is_finite() || boundary == 0. || !plausible(values, effect, boundary)? {
            continue;
        }
        let Some(scale) = geometry(values, effect, boundary)? else {
            continue;
        };
        if data.owned_buffer_bytes() + n as u128 * 16 > ceiling {
            return Err(allocation(
                "insufficient memory to certify a slope near the fixed effects",
            ));
        }
        let mut anchors = fallible_filled(n, 0.0, "unable to reserve fixed-effect slope anchors")?;
        let mut stable = fallible_filled(n, 0.0, "unable to reserve scaled within-group slope")?;
        let mut visits = 0;
        for level in 0..effect.n_levels() {
            let members = effect.members(level)?;
            let Some(&first_row) = members.first() else {
                continue;
            };
            let mut anchor = values[first_row];
            for &row in members {
                row_checkpoint(visits)?;
                visits += 1;
                anchor = anchor.min(values[row]);
            }
            for &row in members {
                row_checkpoint(visits)?;
                visits += 1;
                let value = values[row];
                let difference = value - anchor;
                // Error-free TwoSum subtraction includes cases outside Sterbenz's interval.
                let virtual_origin = difference - value;
                let remainder =
                    (value - (difference - virtual_origin)) + (-anchor - virtual_origin);
                let scaled = difference * scale;
                if remainder != 0.0 || !scaled.is_finite() || scaled / scale != difference {
                    return Ok(None);
                }
                anchors[row] = anchor;
                stable[row] = scaled;
            }
        }
        let weights = match &data.sample().weights {
            PreparedWeights::None => None,
            PreparedWeights::Provided { values, .. } => Some(values.as_slice()),
        };
        let wmax = weights.map_or(1., |w| w.iter().copied().fold(0., f64::max));
        let amax = anchors.iter().copied().map(f64::abs).fold(0.0, f64::max);
        let (mut sw, mut cw, mut sa, mut ca) = (0., 0., 0., 0.);
        for (r, &a) in anchors.iter().enumerate() {
            row_checkpoint(r)?;
            let w = weights.map_or(1., |v| v[r] / wmax);
            neumaier_add(w, &mut sw, &mut cw);
            neumaier_add(w * (a / amax), &mut sa, &mut ca);
        }
        let mean_anchor = ((sa + ca) / (sw + cw)) * amax;
        if !mean_anchor.is_finite() {
            return Ok(None);
        }
        let slot = data
            .report_plan()
            .original_slot(data.model().columns[j].coefficient_slot)?
            .get() as usize
            - 1;
        data.sample
            .swap_recovery_column(column.storage, &mut stable)?;
        return Ok(Some(AbsorbedRecovery {
            target_column: j,
            map: AbsorbedMap {
                slot,
                scale,
                mean_anchor,
            },
            storage: column.storage,
            original: stable,
            anchors,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::CategoricalCodeView;

    #[test]
    fn rare_large_group_outside_probes_does_not_hide_an_identified_coordinate() {
        let mut rows: Vec<_> = (0..16_u32).flat_map(|g| [(g, 0_f64), (g, 1.)]).collect();
        // The twelve probe groups skip group 3. Every other group's scale
        // is tiny compared with the actual between-group range.
        for (group, value) in &mut rows {
            *value += if *group == 3 { 2f64.powi(26) } else { 0. };
        }
        for _ in 0..2 {
            let codes: Vec<_> = rows.iter().map(|(g, _)| *g).collect();
            let x: Vec<_> = rows.iter().map(|(_, x)| *x).collect();
            let source =
                CategoricalTopologySource::single(0, &[CategoricalCodeView::new(&codes, 16)])
                    .unwrap();
            let extrema = crate::slope_recovery::screen::Sketch::new(&x).unwrap();
            let boundary = (extrema.maximum - extrema.minimum) / 8192.;
            assert!(plausible(&x, &source, boundary).unwrap());
            assert_eq!(geometry(&x, &source, boundary).unwrap(), Some(1.));
            rows.reverse();
        }
    }
}
