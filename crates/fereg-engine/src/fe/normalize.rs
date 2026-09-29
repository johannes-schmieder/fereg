//! SPEC saved-FE gauge, using connectivity of the intercept effects.
use super::{
    certificate::{check_tolerance, reconstruction_errors},
    Design,
};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    threads::{compensated_sum, ThreadTeam},
    topology::{connected_components_all, LevelIndex},
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
        check_tolerance(tolerance)?;
        let before = self.multiply(coefficients, team)?;
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
        let components = connected_components_all(&codes, &levels, self.nobs())?;
        let mut row_components = reserved(self.nobs())?;
        for &level in codes[0] {
            row_components.push(
                u32::try_from(components.component_by_effect_level[0][level as usize])
                    .map_err(|_| numerical("normalization component index overflows"))?,
            );
        }
        let membership = LevelIndex::from_codes(&row_components, components.n_components)?;
        let mut weights = reserved(components.n_components)?;
        for component in 0..components.n_components {
            let members = membership.members(component)?;
            let scale = members
                .iter()
                .map(|&row| self.weights().at(row))
                .fold(0., f64::max);
            let total = if scale == 0. {
                0.
            } else {
                compensated_sum(members.iter().map(|&row| self.weights().at(row) / scale))
            };
            weights.push((scale, total));
        }
        let mut values = reserved(coefficients.as_slice().len())?;
        values.extend_from_slice(coefficients.as_slice());
        let mut normalized =
            DenseMatrix::from_row_major(coefficients.nrows(), coefficients.ncols(), values)?;
        for (local, &effect) in intercepts.iter().enumerate().skip(1) {
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
                    .for_each(|(index, shift)| {
                        let component = index / coefficients.ncols();
                        let column = index % coefficients.ncols();
                        *shift = self.component_mean(
                            (offset, codes),
                            membership.members(component).expect("validated component"),
                            coefficients,
                            column,
                            weights[component],
                            scales[(component, column)],
                        );
                    });
            });
            if shifts.as_slice().iter().any(|value| !value.is_finite()) {
                return Err(numerical("FE normalization mean is not representable"));
            }
            for (local_effect, index) in [(local, effect), (0, intercepts[0])] {
                for level in 0..self.layout().effects[index].levels {
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
        let after = self.multiply(&normalized, team)?;
        let mut amplitude = self.contribution_amplitudes(coefficients, team)?;
        let normalized_amplitude = self.contribution_amplitudes(&normalized, team)?;
        // Centering can move a nonzero component mean onto a row whose old
        // contributions were all tiny. Its subtracting terms then round at
        // the new gauge's scale. Audit both representations symmetrically;
        // the final original-RHS/local-span certificate remains unchanged.
        for (scale, &other) in amplitude
            .as_mut_slice()
            .iter_mut()
            .zip(normalized_amplitude.as_slice())
        {
            *scale = scale.max(other);
        }
        if reconstruction_errors(&amplitude, &before, &after)?
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
        weights: (f64, f64),
        value_scale: f64,
    ) -> f64 {
        let (offset, codes) = effect;
        let (weight_scale, weight) = weights;
        if value_scale == 0. || weight_scale == 0. {
            return 0.;
        }
        let numerator = compensated_sum(members.iter().map(|&row| {
            (self.weights().at(row) / weight_scale)
                * (coefficients[(offset + codes[row] as usize, column)] / value_scale)
        }));
        (numerator / weight) * value_scale
    }
    fn contribution_amplitudes(
        &self,
        coefficients: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<DenseMatrix> {
        let mut amplitudes = DenseMatrix::zeros(self.nobs(), coefficients.ncols())?;
        // Both fitted passes validated the frame and coefficient shapes.
        // Each row retains the same effect/channel order on every worker.
        team.install(|| {
            amplitudes
                .as_mut_slice()
                .par_chunks_mut(coefficients.ncols().max(1))
                .enumerate()
                .for_each(|(row, scales)| {
                    for (column, scale) in scales.iter_mut().enumerate() {
                        for (effect, layout) in self.layout().effects.iter().enumerate() {
                            let level = self
                                .topology()
                                .effect_partition(effect)
                                .expect("validated effect")
                                .codes()[row] as usize;
                            for channel in 0..layout.channels {
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
                });
        });
        Ok(amplitudes)
    }
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
