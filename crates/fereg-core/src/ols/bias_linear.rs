//! Response-only actions of the accepted full PPML information inverse.
//! The canonical fit, basis, within columns and predictor buffers stay immutable.

use super::*;
#[path = "bias_dense.rs"]
mod dense;

pub(crate) struct FixedPpmlInformation {
    dense: Option<dense::DenseInformation>,
    pub(crate) route: u8,
    pub(crate) minimum_level_information: f64,
    pub(crate) maximum_fe_leverage_lower_bound: f64,
    generations: LinearSessionGenerations,
    certificate_tolerance: f64,
    bread: Vec<f64>,
    projected: Vec<f64>,
    row: Vec<f64>,
    rhs: Vec<f64>,
    finite: Vec<f64>,
    target: Vec<f64>,
    queue: Vec<usize>,
    moments: Vec<f64>,
    scales: Vec<f64>,
    pub(crate) maximum_score: f64,
    pub(crate) maximum_reconstruction: f64,
}

impl<P: PreparedOlsFitView + Sync + ?Sized> CertifiedLinearSession<'_, P> {
    pub(crate) fn bias_weights(&self) -> BorrowedMapWeights<'_> {
        self.weights.borrowed()
    }

    pub(crate) fn bias_information(
        &self,
        certificate_tolerance: f64,
    ) -> Result<FixedPpmlInformation> {
        self.output()?;
        let within = self
            .within
            .as_ref()
            .ok_or_else(|| invalid_argument("bias within state absent"))?;
        let basis = self
            .frozen_basis
            .as_ref()
            .ok_or_else(|| invalid_argument("bias basis absent"))?;
        let design = self.fe_design();
        if design.effects().len() != 2
            || design
                .effects()
                .iter()
                .any(|e| !e.has_intercept() || !e.slopes().is_empty())
        {
            return Err(invalid_argument(
                "PPML bias correction requires two categorical intercept effects",
            ));
        }
        let rank = basis.kept_generated_columns().len();
        let levels = design.effects().iter().map(|e| e.n_levels()).sum::<usize>();
        let mut bread = Vec::new();
        fill_identified_bread(
            self.fit
                .as_ref()
                .ok_or_else(|| invalid_argument("bias accepted fit absent"))?,
            basis,
            &mut bread,
        )?;
        let dense = dense::DenseInformation::prepare(self)?;
        let mut fixed = FixedPpmlInformation {
            route: if dense.is_some() { 2 } else { 1 },
            minimum_level_information: f64::INFINITY,
            maximum_fe_leverage_lower_bound: 0.,
            dense,
            generations: self.generations,
            certificate_tolerance,
            bread,
            projected: fallible_filled(self.response.len(), 0., "bias projected response")?,
            row: fallible_filled(within.storage.columns(), 0., "bias within row")?,
            rhs: fallible_filled(rank, 0., "bias finite RHS")?,
            finite: fallible_filled(rank, 0., "bias finite coefficients")?,
            target: fallible_filled(self.response.len(), 0., "bias FE target")?,
            queue: fallible_empty(levels, "bias FE traversal")?,
            moments: fallible_filled(levels + rank, 0., "bias score certificate")?,
            scales: fallible_filled(levels + rank, 0., "bias score scales")?,
            maximum_score: 0.,
            maximum_reconstruction: 0.,
        };
        let first = design.effects()[0].n_levels();
        let h = |row: usize| match self.weights.borrowed() {
            BorrowedMapWeights::Unit => 1.,
            BorrowedMapWeights::Provided(w) => w[row],
        };
        for row in 0..self.response.len() {
            fixed.scales[design.effects()[0].codes()[row] as usize] += h(row);
            fixed.scales[first + design.effects()[1].codes()[row] as usize] += h(row);
        }
        fixed.minimum_level_information = fixed.scales[..levels]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        for row in 0..self.response.len() {
            // Each indicator belongs to the full regressor space, so its
            // one-column leverage is a lower bound for that row's full leverage.
            for level in [
                design.effects()[0].codes()[row] as usize,
                first + design.effects()[1].codes()[row] as usize,
            ] {
                fixed.maximum_fe_leverage_lower_bound = fixed
                    .maximum_fe_leverage_lower_bound
                    .max(h(row) / fixed.scales[level]);
            }
        }
        Ok(fixed)
    }

    pub(crate) fn bias_dimension(&self) -> usize {
        self.fe_design()
            .effects()
            .iter()
            .map(|e| e.n_levels())
            .sum::<usize>()
            + self
                .frozen_basis
                .as_ref()
                .map_or(0, |b| b.kept_generated_columns().len())
    }

    pub(crate) fn bias_finite_columns(&self) -> &[usize] {
        self.frozen_basis
            .as_ref()
            .expect("accepted bias basis")
            .kept_generated_columns()
    }

    pub(crate) fn bias_solve(
        &mut self,
        fixed: &mut FixedPpmlInformation,
        response: &[f64],
        prediction: &mut [f64],
        coefficients: Option<&mut [f64]>,
    ) -> Result<()> {
        if self.generations != fixed.generations
            || response.len() != self.response.len()
            || prediction.len() != response.len()
            || response.iter().any(|x| !x.is_finite())
        {
            return Err(invalid_argument(
                "bias solve changed the accepted information frame",
            ));
        }
        if let Some(dense) = &mut fixed.dense {
            let score = dense.solve(
                self,
                response,
                prediction,
                &mut fixed.moments,
                &mut fixed.scales,
                fixed.certificate_tolerance,
            )?;
            fixed.maximum_score = fixed.maximum_score.max(score);
            if let Some(output) = coefficients {
                output.copy_from_slice(&dense.coefficients);
            }
            return Ok(());
        }
        fixed.projected.copy_from_slice(response);
        let matrix =
            DenseMatrix::from_row_major(response.len(), 1, std::mem::take(&mut fixed.projected))?;
        let absorbed = if self.config.absorb.solver == FeSolverKind::Map {
            self.fill_reusable_map_weights()?;
            self.absorb_reusable_map(matrix)?
        } else {
            self.prepared
                .fit_absorb_matrix(self.weights.borrowed(), matrix, self.config.absorb)?
        };
        if !absorbed.report().certified() {
            return Err(invalid_data(
                "bias response projection failed certification",
            ));
        }
        fixed.projected = absorbed.into_parts().0.into_row_major();
        let within = self
            .within
            .as_ref()
            .ok_or_else(|| invalid_argument("bias within state disappeared"))?;
        let rank = fixed.rhs.len();
        let weight = |row: usize| match self.weights.borrowed() {
            BorrowedMapWeights::Unit => 1.,
            BorrowedMapWeights::Provided(w) => w[row],
        };
        fixed.rhs.fill(0.);
        let mut reader = within.storage.reader();
        for row in 0..response.len() {
            reader.fill_row(row, &mut fixed.row)?;
            for (j, &position) in within.active_positions.iter().enumerate() {
                fixed.rhs[j] =
                    (weight(row) * fixed.projected[row]).mul_add(fixed.row[position], fixed.rhs[j]);
            }
        }
        for j in 0..rank {
            fixed.finite[j] = (0..rank)
                .map(|k| fixed.bread[j * rank + k] * fixed.rhs[k])
                .sum();
        }
        fixed.moments.fill(0.);
        fixed.scales.fill(0.);
        let design = self.fe_design();
        let left = &design.effects()[0];
        let right = &design.effects()[1];
        let offset = left.n_levels();
        let levels = offset + right.n_levels();
        let kept = self.bias_finite_columns();
        for row in 0..response.len() {
            reader.fill_row(row, &mut fixed.row)?;
            let fitted_within = within
                .active_positions
                .iter()
                .enumerate()
                .fold(0., |sum, (j, &p)| {
                    fixed.finite[j].mul_add(fixed.row[p], sum)
                });
            let residual = fixed.projected[row] - fitted_within;
            prediction[row] = response[row] - residual;
            let h = weight(row);
            for position in [
                left.codes()[row] as usize,
                offset + right.codes()[row] as usize,
            ] {
                fixed.moments[position] += h * residual;
                fixed.scales[position] += h * (response[row].abs() + prediction[row].abs());
            }
            for (j, &column) in kept.iter().enumerate() {
                let x = self.prepared.fit_column_value(column, row)?;
                fixed.moments[levels + j] += h * x * residual;
                fixed.scales[levels + j] +=
                    h * x.abs() * (response[row].abs() + prediction[row].abs());
            }
        }
        let score = fixed
            .moments
            .iter()
            .zip(&fixed.scales)
            .map(|(s, n)| s.abs() / n.max(1.))
            .fold(0., f64::max);
        if !score.is_finite()
            || score > fixed.certificate_tolerance
            || prediction
                .iter()
                .chain(&fixed.finite)
                .any(|x| !x.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!("bias auxiliary normal equations failed: {score:.3e}"),
            ));
        }
        fixed.maximum_score = fixed.maximum_score.max(score);
        if let Some(output) = coefficients {
            fixed.target.copy_from_slice(prediction);
            for (j, &column) in kept.iter().enumerate() {
                self.prepared.fit_subtract_raw_column(
                    column,
                    fixed.finite[j],
                    &mut fixed.target,
                )?;
            }
            recover_anchored(
                design,
                &fixed.target,
                &mut output[..levels],
                &mut fixed.queue,
                fixed.certificate_tolerance,
                &mut fixed.maximum_reconstruction,
            )?;
            output[levels..].copy_from_slice(&fixed.finite);
        }
        Ok(())
    }

    pub(crate) fn bias_raw_coefficients(
        &self,
        fixed: &mut FixedPpmlInformation,
        eta: &[f64],
        offset: &[f64],
        output: &mut [f64],
    ) -> Result<()> {
        let kept = self.bias_finite_columns();
        let levels = self.bias_dimension() - kept.len();
        if eta.len() != fixed.target.len() || offset.len() != eta.len() {
            return Err(invalid_argument("bias accepted predictor is not row-aligned"));
        }
        // The canonical WLS response minus its residual can cancel when y/mu
        // is large. The accepted PPML predictor retains the accurate fitted
        // mean; remove its offset and unchanged finite contribution directly.
        for row in 0..eta.len() {
            fixed.target[row] = (eta[row] - offset[row]) - self.finite_predictor[row];
        }
        // The canonical reported constant is allocated into absorbed effects.
        let fit = self
            .fit
            .as_ref()
            .ok_or_else(|| invalid_argument("bias raw fit absent"))?;
        for (j, &column) in kept.iter().enumerate() {
            let coefficient = fit
                .coefficients()
                .iter()
                .find(|b| b.generated_column_index == Some(column))
                .and_then(|b| b.value)
                .ok_or_else(|| invalid_argument("bias raw finite coordinate absent"))?;
            output[levels + j] = coefficient;
        }
        recover_anchored(
            self.fe_design(),
            &fixed.target,
            &mut output[..levels],
            &mut fixed.queue,
            fixed.certificate_tolerance,
            &mut fixed.maximum_reconstruction,
        )
    }
}

fn recover_anchored(
    design: &FeDesign,
    target: &[f64],
    coefficients: &mut [f64],
    queue: &mut Vec<usize>,
    tolerance: f64,
    maximum: &mut f64,
) -> Result<()> {
    let left = &design.effects()[0];
    let right = &design.effects()[1];
    let offset = left.n_levels();
    coefficients.fill(f64::NAN);
    // Starting roots on the sorted second partition fixes its lowest level in
    // every component, independently of fitted weights and observation order.
    for root in offset..coefficients.len() {
        if coefficients[root].is_finite() {
            continue;
        }
        coefficients[root] = 0.;
        queue.clear();
        queue.push(root);
        let mut head = 0;
        while head < queue.len() {
            let node = queue[head];
            head += 1;
            let (effect, level) = if node < offset {
                (left, node)
            } else {
                (right, node - offset)
            };
            for &row in effect.level_index().checked_members(level)? {
                let other = if node < offset {
                    offset + right.codes()[row] as usize
                } else {
                    left.codes()[row] as usize
                };
                if coefficients[other].is_nan() {
                    coefficients[other] = target[row] - coefficients[node];
                    queue.push(other);
                }
            }
        }
    }
    for row in 0..target.len() {
        let value = coefficients[left.codes()[row] as usize]
            + coefficients[offset + right.codes()[row] as usize];
        let error = (value - target[row]).abs() / target[row].abs().max(value.abs()).max(1.);
        if !error.is_finite() || error > tolerance {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "bias anchored FE reconstruction failed",
            ));
        }
        *maximum = maximum.max(error);
    }
    Ok(())
}
