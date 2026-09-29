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
        let mut values = reserved(coefficients.as_slice().len())?;
        values.extend_from_slice(coefficients.as_slice());
        let mut normalized =
            DenseMatrix::from_row_major(coefficients.nrows(), coefficients.ncols(), values)?;
        for (local, &effect) in intercepts.iter().enumerate().skip(1) {
            let mut shifts = DenseMatrix::zeros(components.n_components, coefficients.ncols())?;
            for component in 0..components.n_components {
                let members = membership.members(component)?;
                for column in 0..coefficients.ncols() {
                    shifts[(component, column)] =
                        self.component_mean(effect, members, coefficients, column)?;
                }
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
        let amplitude = self.contribution_amplitudes(coefficients)?;
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
        effect: usize,
        members: &[usize],
        coefficients: &DenseMatrix,
        column: usize,
    ) -> Result<f64> {
        let layout = &self.layout().effects[effect];
        let codes = self.topology().effect_partition(effect)?.codes();
        let weight_scale = members
            .iter()
            .map(|&row| self.weights().at(row))
            .fold(0., f64::max);
        let value_scale = members
            .iter()
            .map(|&row| coefficients[(layout.offset + codes[row] as usize, column)].abs())
            .fold(0., f64::max);
        if value_scale == 0. || weight_scale == 0. {
            return Ok(0.);
        }
        let weight = compensated_sum(
            members
                .iter()
                .map(|&row| self.weights().at(row) / weight_scale),
        );
        let numerator = compensated_sum(members.iter().map(|&row| {
            (self.weights().at(row) / weight_scale)
                * (coefficients[(layout.offset + codes[row] as usize, column)] / value_scale)
        }));
        let mean = (numerator / weight) * value_scale;
        if !mean.is_finite() {
            return Err(numerical("FE normalization mean is not representable"));
        }
        Ok(mean)
    }
    fn contribution_amplitudes(&self, coefficients: &DenseMatrix) -> Result<DenseMatrix> {
        let mut amplitudes = DenseMatrix::zeros(self.nobs(), coefficients.ncols())?;
        for row in 0..self.nobs() {
            for column in 0..coefficients.ncols() {
                let mut scale = 0_f64;
                for (effect, layout) in self.layout().effects.iter().enumerate() {
                    let level = self.topology().effect_partition(effect)?.codes()[row] as usize;
                    for channel in 0..layout.channels {
                        scale = scale.max(
                            (self.basis_value(effect, channel, row)?
                                * coefficients
                                    [(self.coefficient_index(effect, channel, level)?, column)])
                                .abs(),
                        );
                    }
                }
                amplitudes[(row, column)] = scale;
            }
        }
        Ok(amplitudes)
    }
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
