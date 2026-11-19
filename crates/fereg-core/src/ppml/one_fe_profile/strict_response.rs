//! A projected Fisher response for a single categorical intercept effect.
use super::*;

impl ProfiledOneFe {
    /// At the exact accepted Hessian, eliminate the group dummies from
    /// H d = X' q(y-mu). Return eta-offset + X d only after verifying its
    /// weighted normal equations for every original control and group.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::ppml) fn strict_response<P: PreparedOlsFitView + ?Sized>(
        &mut self,
        prepared: &P,
        columns: &[usize],
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        offset: &[f64],
        workspace: &PpmlIterationWorkspace,
        tolerance: f64,
    ) -> Result<&[f64]> {
        let data = ProfiledRawColumns::new(prepared, columns)?;
        self.mean.copy_from_slice(workspace.mean());
        self.solve_centered_direction(&data, design, y, q)?;
        let effect = &design.effects()[0];
        let h = workspace.hessian_weights();
        let score = workspace.score_residuals();
        for group in 0..effect.n_levels() {
            crate::interrupt::row_checkpoint(group)?;
            let rows = effect.level_index().members(group);
            let first = *rows.first().ok_or_else(numerical)?;
            for (j, anchor) in self.base.iter_mut().enumerate() {
                *anchor = data.value(first, j)?;
            }
            // Group centering preserves the slope direction under arbitrary
            // regressor translations. Recover the group's coefficient from
            // its own score, with no division by an individual fitted mean.
            let mut mass = CompensatedSum::default();
            let mut group_score = CompensatedSum::default();
            for (index, &row) in rows.iter().enumerate() {
                crate::interrupt::row_checkpoint(index)?;
                let mut change = CompensatedSum::default();
                for (j, &beta) in self.direction.iter().enumerate() {
                    change.add((data.value(row, j)? - self.base[j]) * beta);
                }
                self.eta[row] = change.total();
                mass.add(h[row]);
                group_score.add((-h[row]).mul_add(self.eta[row], q[row] * score[row]));
            }
            let shift = group_score.total() / mass.total();
            for &row in rows {
                let current = workspace.eta()[row] - offset[row];
                self.eta[row] = current + (shift + self.eta[row]);
                // Verify the actual representable replacement. The removed
                // weighted response is q(y-mu) - h(replacement - current).
                self.mean[row] = (-h[row]).mul_add(self.eta[row] - current, q[row] * score[row]);
            }
            orthogonal(
                rows.iter().copied(),
                |_| Ok(1.),
                &self.mean,
                q,
                score,
                tolerance,
            )?;
        }
        for column in 0..prepared.fit_model().columns.len() {
            crate::interrupt::row_checkpoint(column)?;
            orthogonal(
                0..y.len(),
                |row| prepared.fit_column_value(column, row),
                &self.mean,
                q,
                score,
                tolerance,
            )?;
        }
        Ok(&self.eta)
    }
}

fn orthogonal(
    rows: impl Iterator<Item = usize>,
    basis: impl Fn(usize) -> Result<f64>,
    removed: &[f64],
    q: &[f64],
    score: &[f64],
    tolerance: f64,
) -> Result<()> {
    let mut moment = CompensatedSum::default();
    let mut scale = CompensatedSum::default();
    for (index, row) in rows.enumerate() {
        crate::interrupt::row_checkpoint(index)?;
        let x = basis(row)?;
        moment.add(x * removed[row]);
        scale.add(x.abs() * ((q[row] * score[row]).abs() + removed[row].abs()));
    }
    let error = moment.total().abs();
    let scale = scale.total();
    if error.is_finite() && scale.is_finite() && error <= tolerance * scale {
        Ok(())
    } else {
        Err(numerical())
    }
}
