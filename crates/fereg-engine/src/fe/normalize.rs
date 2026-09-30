//! SPEC saved-FE gauge, using connectivity of the intercept effects.
use super::{
    certificate::{check_tolerance, reconstruction_errors},
    Design,
};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    threads::ThreadTeam,
    topology::{connected_components_all_polled, LevelIndex},
};
use rayon::prelude::*;
impl Design<'_> {
    /// Center later intercepts within each intercept-connected component and
    /// transfer each shift to the first intercept. Commit only if the fitted
    /// sum is preserved; slopes and rank-deficient representatives are retained.
    pub fn normalize_coefficients(
        &self,
        coefficients: &mut DenseMatrix,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<()> {
        self.normalize_coefficients_polled(coefficients, tolerance, team, None)
    }
    pub(crate) fn normalize_coefficients_polled(
        &self,
        coefficients: &mut DenseMatrix,
        tolerance: f64,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<()> {
        execution::check(poll)?;
        check_tolerance(tolerance)?;
        let before = self.multiply_polled(coefficients, team, poll)?;
        let mut intercepts = reserved(self.layout().effects.len())?;
        intercepts.extend(
            self.topology()
                .effects()
                .iter()
                .enumerate()
                .filter_map(|(i, e)| e.intercept.then_some(i)),
        );
        if intercepts.len() <= 1 {
            return Ok(());
        }
        let mut codes = reserved(intercepts.len())?;
        let mut levels = reserved(intercepts.len())?;
        for &effect in &intercepts {
            let partition = self.topology().effect_partition(effect)?;
            codes.push(partition.codes());
            levels.push(partition.n_levels());
        }
        let components = connected_components_all_polled(&codes, &levels, self.nobs(), poll)?;
        let mut row_components = reserved(self.nobs())?;
        for (row, &level) in codes[0].iter().enumerate() {
            execution::check_chunk(poll, row)?;
            row_components.push(
                u32::try_from(components.component_by_effect_level[0][level as usize])
                    .map_err(|_| numerical("normalization component index overflows"))?,
            );
        }
        let membership =
            LevelIndex::from_codes_polled(&row_components, components.n_components, poll)?;
        let mut weights = reserved(components.n_components)?;
        for component in 0..components.n_components {
            execution::check(poll)?;
            let members = membership.members(component)?;
            let scale = execution::fold(
                members.iter().map(|&row| self.weights().at(row)),
                0.,
                poll,
                f64::max,
            )?;
            let total = if scale == 0. {
                0.
            } else {
                execution::sum(
                    members.iter().map(|&row| self.weights().at(row) / scale),
                    poll,
                )?
            };
            weights.push((scale, total));
        }
        let mut values = reserved(coefficients.as_slice().len())?;
        values.extend_from_slice(coefficients.as_slice());
        let mut normalized =
            DenseMatrix::from_row_major(coefficients.nrows(), coefficients.ncols(), values)?;
        for (local, &effect) in intercepts.iter().enumerate().skip(1) {
            execution::check(poll)?;
            let mut shifts = DenseMatrix::zeros(components.n_components, coefficients.ncols())?;
            let mut scales = DenseMatrix::zeros(components.n_components, coefficients.ncols())?;
            let offset = self.layout().effects[effect].offset;
            let codes = self.topology().effect_partition(effect)?.codes();
            // Every declared final-sample level occurs. Its repeated row
            // values have the same maximum as this single level traversal.
            for (level, &component) in components.component_by_effect_level[local]
                .iter()
                .enumerate()
            {
                execution::check_chunk(poll, level)?;
                for column in 0..coefficients.ncols() {
                    scales[(component, column)] = scales[(component, column)]
                        .max(coefficients[(offset + level, column)].abs());
                }
            }
            team.install(|| {
                shifts
                    .as_mut_slice()
                    .par_iter_mut()
                    .enumerate()
                    .try_for_each(|(index, shift)| -> Result<()> {
                        let component = index / coefficients.ncols();
                        let column = index % coefficients.ncols();
                        *shift = self.component_mean(
                            (offset, codes),
                            membership.members(component).expect("validated component"),
                            coefficients,
                            column,
                            (
                                weights[component].0,
                                weights[component].1,
                                scales[(component, column)],
                            ),
                            poll,
                        )?;
                        Ok(())
                    })
            })?;
            if shifts.as_slice().iter().any(|value| !value.is_finite()) {
                return Err(numerical("FE normalization mean is not representable"));
            }
            for (local_effect, index) in [(local, effect), (0, intercepts[0])] {
                for level in 0..self.layout().effects[index].levels {
                    execution::check_chunk(poll, level)?;
                    let component = components.component_by_effect_level[local_effect][level];
                    let coefficient = self.coefficient_index(index, 0, level)?;
                    for column in 0..coefficients.ncols() {
                        let shift = shifts[(component, column)];
                        normalized[(coefficient, column)] +=
                            if index == effect { -shift } else { shift };
                    }
                }
            }
        }
        let after = self.multiply_polled(&normalized, team, poll)?;
        let mut amplitude = self.contribution_amplitudes(coefficients, team, poll)?;
        let normalized_amplitude = self.contribution_amplitudes(&normalized, team, poll)?;
        // Centering can move a nonzero component mean onto a row whose old
        // contributions were all tiny. Its subtracting terms then round at
        // the new gauge's scale. Audit both representations symmetrically;
        // the final original-RHS/local-span certificate remains unchanged.
        for (position, (scale, &other)) in amplitude
            .as_mut_slice()
            .iter_mut()
            .zip(normalized_amplitude.as_slice())
            .enumerate()
        {
            execution::check_chunk(poll, position)?;
            *scale = scale.max(other);
        }
        if reconstruction_errors(&amplitude, &before, &after, poll)?
            .iter()
            .any(|&error| !error.is_finite() || error > tolerance)
        {
            return Err(numerical("FE normalization changed the fitted sum"));
        }
        *coefficients = normalized;
        Ok(())
    }
    fn component_mean(
        &self,
        effect: (usize, &[u32]),
        members: &[usize],
        coefficients: &DenseMatrix,
        column: usize,
        weights: (f64, f64, f64),
        poll: Poll<'_>,
    ) -> Result<f64> {
        execution::check(poll)?;
        let (offset, codes) = effect;
        let (weight_scale, weight, value_scale) = weights;
        if value_scale == 0. || weight_scale == 0. {
            return Ok(0.);
        }
        let numerator = execution::sum(
            members.iter().map(|&row| {
                (self.weights().at(row) / weight_scale)
                    * (coefficients[(offset + codes[row] as usize, column)] / value_scale)
            }),
            poll,
        )?;
        Ok((numerator / weight) * value_scale)
    }
    fn contribution_amplitudes(
        &self,
        coefficients: &DenseMatrix,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<DenseMatrix> {
        let mut amplitudes = DenseMatrix::zeros(self.nobs(), coefficients.ncols())?;
        // Both fitted passes validated the frame and coefficient shapes.
        // Each row retains the same effect/channel order on every worker.
        team.install(|| {
            amplitudes
                .as_mut_slice()
                .par_chunks_mut(coefficients.ncols().max(1))
                .enumerate()
                .try_for_each(|(row, scales)| -> Result<()> {
                    execution::check_chunk(poll, row)?;
                    for (column, scale) in scales.iter_mut().enumerate() {
                        for (effect, layout) in self.layout().effects.iter().enumerate() {
                            let level = self
                                .topology()
                                .effect_partition(effect)
                                .expect("validated effect")
                                .codes()[row] as usize;
                            for channel in 0..layout.channels {
                                execution::check_chunk(poll, channel)?;
                                *scale = scale.max(
                                    (self
                                        .basis_value(effect, channel, row)
                                        .expect("validated channel")
                                        * coefficients[(
                                            layout.offset + channel * layout.levels + level,
                                            column,
                                        )])
                                        .abs(),
                                );
                            }
                        }
                    }
                    Ok(())
                })
        })?;
        Ok(amplitudes)
    }
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
