//! Bounded cached information factor for small grouped PPML corrections.
use super::*;

pub(super) struct DenseInformation {
    factor: Vec<f64>,
    permutation: Vec<usize>,
    full_to_free: Vec<usize>,
    free_to_full: Vec<usize>,
    scale: Vec<f64>,
    rhs: Vec<f64>,
    ordered_rhs: Vec<f64>,
    forward: Vec<f64>,
    solution: Vec<f64>,
    entries: Vec<(usize, f64)>,
    pub(super) coefficients: Vec<f64>,
}

impl DenseInformation {
    pub(super) fn prepare<P: PreparedOlsFitView + Sync + ?Sized>(
        session: &CertifiedLinearSession<'_, P>,
    ) -> Result<Option<Self>> {
        let dimension = session.bias_dimension();
        // Explicit low memory keeps the established sparse operator. Admission
        // reserves the dense envelope before the final support is known.
        if dimension > 256 || session.config.execution.mode == PreparedOlsExecutionMode::Low {
            return Ok(None);
        }
        let design = session.fe_design();
        let left = &design.effects()[0];
        let right = &design.effects()[1];
        let offset = left.n_levels();
        let components = crate::fe::connected_components_all(
            &[left.codes(), right.codes()],
            &[offset, right.n_levels()],
        )?;
        let mut anchors = fallible_filled(
            components.n_components,
            usize::MAX,
            "bias component anchors",
        )?;
        for (level, &component) in components.component_by_effect_level[1].iter().enumerate() {
            anchors[component] = anchors[component].min(offset + level);
        }
        let rank = dimension - anchors.len();
        let mut full_to_free = fallible_filled(dimension, usize::MAX, "bias dense coordinate map")?;
        let mut free_to_full = fallible_empty(rank, "bias dense free coordinates")?;
        for (j, free) in full_to_free.iter_mut().enumerate() {
            if !anchors.contains(&j) {
                *free = free_to_full.len();
                free_to_full.push(j);
            }
        }
        let mut result = Self {
            factor: fallible_filled(rank * rank, 0., "bias dense factor")?,
            permutation: fallible_filled(rank, 0, "bias dense permutation")?,
            full_to_free,
            free_to_full,
            scale: fallible_filled(rank, 0., "bias dense scales")?,
            rhs: fallible_filled(rank, 0., "bias dense RHS")?,
            ordered_rhs: fallible_filled(rank, 0., "bias dense permuted RHS")?,
            forward: fallible_filled(rank, 0., "bias dense forward solve")?,
            solution: fallible_filled(rank, 0., "bias dense solution")?,
            entries: fallible_empty(
                session.bias_finite_columns().len() + 2,
                "bias sparse row entries",
            )?,
            coefficients: fallible_filled(dimension, 0., "bias dense normalized solution")?,
        };
        let mut gram = fallible_filled(rank * rank, 0., "bias bounded information matrix")?;
        for row in 0..session.response.len() {
            result.fill_row(session, row)?;
            let h = match session.weights.borrowed() {
                BorrowedMapWeights::Unit => 1.,
                BorrowedMapWeights::Provided(w) => w[row],
            };
            for &(j, x) in &result.entries {
                for &(k, z) in &result.entries {
                    gram[j * rank + k] = (h * x).mul_add(z, gram[j * rank + k]);
                }
            }
        }
        for j in 0..rank {
            result.scale[j] = gram[j * rank + j].sqrt();
            if !result.scale[j].is_finite() || result.scale[j] <= 0. {
                return Ok(None);
            }
        }
        for j in 0..rank {
            for k in 0..rank {
                gram[j * rank + k] /= result.scale[j] * result.scale[k];
            }
        }
        let guard = (session.config.rank_tolerance_multiplier
            * f64::EPSILON
            * session.response.len().max(rank) as f64)
            .sqrt();
        if factor_guarded_cholesky_in_place(
            &mut gram,
            &mut result.rhs,
            &mut result.factor,
            &mut result.permutation,
            CholeskyPivotPolicy::MaximumDiagonal,
            guard,
        )?
        .is_none()
        {
            return Ok(None);
        }
        Ok(Some(result))
    }

    fn fill_row<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        session: &CertifiedLinearSession<'_, P>,
        row: usize,
    ) -> Result<()> {
        let design = session.fe_design();
        let offset = design.effects()[0].n_levels();
        let levels = offset + design.effects()[1].n_levels();
        self.entries.clear();
        for full in [
            design.effects()[0].codes()[row] as usize,
            offset + design.effects()[1].codes()[row] as usize,
        ] {
            let free = self.full_to_free[full];
            if free != usize::MAX {
                self.entries.push((free, 1.));
            }
        }
        for (j, &column) in session.bias_finite_columns().iter().enumerate() {
            self.entries.push((
                self.full_to_free[levels + j],
                session.prepared.fit_column_value(column, row)?,
            ));
        }
        Ok(())
    }

    pub(super) fn solve<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        session: &CertifiedLinearSession<'_, P>,
        response: &[f64],
        prediction: &mut [f64],
        moments: &mut [f64],
        scales: &mut [f64],
        tolerance: f64,
    ) -> Result<f64> {
        self.rhs.fill(0.);
        let weight = |row: usize| match session.weights.borrowed() {
            BorrowedMapWeights::Unit => 1.,
            BorrowedMapWeights::Provided(w) => w[row],
        };
        for (row, &value) in response.iter().enumerate() {
            self.fill_row(session, row)?;
            for &(j, x) in &self.entries {
                self.rhs[j] = (weight(row) * value).mul_add(x, self.rhs[j]);
            }
        }
        for (j, &original) in self.permutation.iter().enumerate() {
            self.ordered_rhs[j] = self.rhs[original] / self.scale[original];
        }
        if !solve_guarded_cholesky(
            &self.factor,
            &self.ordered_rhs,
            &mut self.forward,
            &mut self.solution,
        )? {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "bias cached information solve is nonfinite",
            ));
        }
        for (j, &original) in self.permutation.iter().enumerate() {
            self.coefficients[self.free_to_full[original]] =
                self.solution[j] / self.scale[original];
        }
        moments.fill(0.);
        scales.fill(0.);
        for row in 0..response.len() {
            self.fill_row(session, row)?;
            prediction[row] = self.entries.iter().fold(0., |s, &(j, x)| {
                self.coefficients[self.free_to_full[j]].mul_add(x, s)
            });
            let residual = response[row] - prediction[row];
            for &(j, x) in &self.entries {
                let full = self.free_to_full[j];
                moments[full] += weight(row) * x * residual;
                scales[full] +=
                    weight(row) * x.abs() * (response[row].abs() + prediction[row].abs());
            }
            let full = session.fe_design().effects()[0].n_levels()
                + session.fe_design().effects()[1].codes()[row] as usize;
            if self.full_to_free[full] == usize::MAX {
                moments[full] += weight(row) * residual;
                scales[full] += weight(row) * (response[row].abs() + prediction[row].abs());
            }
        }
        let score = moments
            .iter()
            .zip(scales.iter())
            .map(|(m, s)| m.abs() / s.max(1.))
            .fold(0., f64::max);
        if !score.is_finite() || score > tolerance || prediction.iter().any(|x| !x.is_finite()) {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!("bias dense normal equations failed: {score:.3e}"),
            ));
        }
        Ok(score)
    }
}
