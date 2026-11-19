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
        // A tiny raw moment is not evidence that an omitted column adds no
        // score direction. Its scale may be dominated by a group constant
        // or by a large multiple of a kept column. Only the explicit
        // intercept is known to belong to the absorbed group span here.
        if prepared
            .fit_model()
            .columns
            .iter()
            .enumerate()
            .any(|(j, column)| {
                column.kind != crate::ols::PreparedOlsColumnKind::Intercept && !columns.contains(&j)
            })
        {
            return Err(numerical());
        }
        let data = ProfiledRawColumns::new(prepared, columns)?;
        self.mean.copy_from_slice(workspace.mean());
        self.solve_centered_direction(&data, design, y, q)?;
        let effect = &design.effects()[0];
        let h = workspace.hessian_weights();
        let score = workspace.score_residuals();
        for group in 0..effect.n_levels() {
            crate::interrupt::row_checkpoint(group)?;
            let rows = effect.level_index().members(group);
            weighted_group_center(&data, rows, h, &mut self.base)?;
            // Group centering preserves the slope direction under arbitrary
            // regressor translations. Recover the group's coefficient from
            // its own score, with no division by an individual fitted mean.
            let mut mass = CompensatedSum::default();
            let mut group_score = CompensatedSum::default();
            for (index, &row) in rows.iter().enumerate() {
                crate::interrupt::row_checkpoint(index)?;
                let mut change = CompensatedSum::default();
                for (j, &beta) in self.direction.iter().enumerate() {
                    if j > 0 {
                        crate::interrupt::row_checkpoint(j)?;
                    }
                    change.add((data.value(row, j)? - self.base[j]) * beta);
                }
                self.eta[row] = change.total();
                mass.add(h[row]);
                group_score.add((-h[row]).mul_add(self.eta[row], q[row] * score[row]));
            }
            let shift = group_score.total() / mass.total();
            for (index, &row) in rows.iter().enumerate() {
                crate::interrupt::row_checkpoint(index)?;
                let mut base = CompensatedSum::default();
                base.add(workspace.eta()[row]);
                base.add(-offset[row]);
                let [current, tail] = base.components();
                self.eta[row] = current + (shift + self.eta[row]);
                // Verify the actual representable replacement. The removed
                // weighted response is q(y-mu) - h(replacement - current).
                let raw_score = q[row] * score[row];
                let removed = (-h[row]).mul_add(self.eta[row] - current, raw_score);
                self.mean[row] = h[row].mul_add(tail, removed);
                if (h[row] * tail).abs() > raw_score.abs() + self.mean[row].abs() {
                    return Err(numerical());
                }
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
        self.verify_fisher_correction(
            &data,
            effect,
            h,
            score,
            q,
            workspace.eta(),
            workspace.coefficients(),
            tolerance,
        )?;
        Ok(&self.eta)
    }
}

/// A representable group constant. Centering about the smallest-magnitude
/// observation before summing retains within-group variation after translation
/// and makes the anchor independent of row order.
fn weighted_group_center(
    data: impl ProfiledDesign,
    rows: &[usize],
    h: &[f64],
    centers: &mut [f64],
) -> Result<()> {
    let mut mass = CompensatedSum::default();
    for (index, &i) in rows.iter().enumerate() {
        crate::interrupt::row_checkpoint(index)?;
        mass.add(h[i]);
    }
    let mass = mass.total();
    if !(mass.is_normal() && mass > 0.) {
        return Err(numerical());
    }
    for (j, center) in centers.iter_mut().enumerate() {
        let mut anchor = f64::INFINITY;
        for (index, &i) in rows.iter().enumerate() {
            crate::interrupt::row_checkpoint(index)?;
            let x = data.value(i, j)?;
            if x.abs() < anchor.abs() || x.abs() == anchor.abs() && x < anchor {
                anchor = x;
            }
        }
        let mut difference = CompensatedSum::default();
        for (index, &i) in rows.iter().enumerate() {
            crate::interrupt::row_checkpoint(index)?;
            difference.add((h[i] / mass) * (data.value(i, j)? - anchor));
        }
        *center = anchor + difference.total();
        if !center.is_finite() {
            return Err(numerical());
        }
    }
    Ok(())
}

impl ProfiledOneFe {
    /// For the scaled design Z (all kept within columns and unit-norm group
    /// dummies), a strictly diagonally dominant Gram has inverse infinity
    /// norm at most 1/min_j(G_jj - sum_{l!=j}|G_jl|). Thus the score of the
    /// removed response bounds the missing Newton movement in every row and
    /// coefficient. A nearly aliased direction or an inconclusive bound is a
    /// refusal. The computed group cross terms also cover a rounded center.
    #[allow(clippy::too_many_arguments)]
    fn verify_fisher_correction(
        &mut self,
        data: impl ProfiledDesign,
        effect: &crate::fe::EffectTerm,
        h: &[f64],
        score: &[f64],
        q: &[f64],
        eta: &[f64],
        coefficients: &[f64],
        tolerance: f64,
    ) -> Result<()> {
        let k = self.columns;
        let mut hmax = 0.0_f64;
        for (i, &weight) in h.iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            hmax = hmax.max(weight);
        }
        let scratch = self.partials.first_mut().ok_or_else(numerical)?;
        self.factor.fill(0.); // normalized finite Gram
        self.base.fill(0.); // absolute group cross terms of each finite column
        self.gradient.fill(0.);
        self.hessian[..k].fill(0.); // compensation for each finite score
        self.direction.fill(0.); // scales for the score's rounding error
        let mut margin = 1.0_f64;
        let mut max_diagonal = 1.0_f64;
        let mut max_score = 0.0_f64;
        let mut row_amplification = 0.0_f64;
        let n = h.len() as f64;
        let tiny = (f64::MIN_POSITIVE / f64::EPSILON) / hmax + f64::MIN_POSITIVE / f64::EPSILON;
        // Compensated scores have first-order error independent of the row
        // count. This allowance also covers forming and scaling the row scores.
        let score_roundoff = (16. + 4. * n * f64::EPSILON) * f64::EPSILON;
        for group in 0..effect.n_levels() {
            let rows = effect.level_index().members(group);
            weighted_group_center(data, rows, h, &mut scratch.anchor)?;
            let mut mass = CompensatedSum::default();
            for (index, &i) in rows.iter().enumerate() {
                crate::interrupt::row_checkpoint(index)?;
                mass.add(h[i] / hmax);
            }
            let group_scale = mass.total().sqrt();
            if !group_scale.is_normal() {
                return Err(numerical());
            }
            scratch.mean.fill(0.); // weighted cross moments with this dummy
            let mut group_score = CompensatedSum::default();
            let mut group_score_scale = 0.;
            for (index, &i) in rows.iter().enumerate() {
                crate::interrupt::row_checkpoint(index)?;
                let w = h[i] / hmax;
                let removed = self.mean[i] / hmax;
                let raw_score = q[i] * score[i];
                let score_scale = (removed.abs() + (raw_score / hmax).abs()).max(tiny);
                if !w.is_normal()
                    || !score_scale.is_finite()
                    || (score[i] != 0. && !raw_score.is_normal())
                    || (self.mean[i] != 0. && !removed.is_normal())
                {
                    return Err(numerical());
                }
                group_score.add(removed / group_scale);
                group_score_scale += score_scale / group_scale;
                let mut row_norm = 1. / group_scale;
                for j in 0..k {
                    if j > 0 {
                        crate::interrupt::row_checkpoint(j)?;
                    }
                    let z = (data.value(i, j)? - scratch.anchor[j]) / self.scales[j];
                    if !z.is_finite() {
                        return Err(numerical());
                    }
                    scratch.centered[j] = z;
                    row_norm += z.abs();
                    scratch.mean[j] += (w / group_scale) * z;
                    crate::linalg::neumaier_add(
                        z * removed,
                        &mut self.gradient[j],
                        &mut self.hessian[j],
                    );
                    self.direction[j] += z.abs() * score_scale;
                    for l in 0..=j {
                        if l > 0 {
                            crate::interrupt::row_checkpoint(l)?;
                        }
                        self.factor[j * k + l] += (w * z) * scratch.centered[l];
                    }
                }
                row_amplification = row_amplification.max(row_norm / eta[i].abs().max(1.));
            }
            let mut cross = 0.;
            for (total, &value) in self.base.iter_mut().zip(&scratch.mean) {
                cross += value.abs();
                *total += value.abs();
            }
            margin = margin.min(1. - cross);
            let bound = group_score.total().abs() + score_roundoff * group_score_scale;
            if !bound.is_finite() || !cross.is_finite() {
                return Err(numerical());
            }
            max_score = max_score.max(bound);
        }
        for j in 0..k {
            crate::interrupt::row_checkpoint(j)?;
            let diagonal = self.factor[j * k + j];
            let mut cross = self.base[j];
            for l in 0..k {
                if l > 0 {
                    crate::interrupt::row_checkpoint(l)?;
                }
                if j != l {
                    cross += self.factor[j.max(l) * k + j.min(l)].abs();
                }
            }
            let bound =
                (self.gradient[j] + self.hessian[j]).abs() + score_roundoff * self.direction[j];
            if !diagonal.is_finite() || !cross.is_finite() || !bound.is_finite() {
                return Err(numerical());
            }
            max_diagonal = max_diagonal.max(diagonal);
            margin = margin.min(diagonal - cross);
            max_score = max_score.max(bound);
            row_amplification = row_amplification
                .max(1. / self.scales[j] / coefficients[self.slots[j]].abs().max(1.));
        }
        // A Gram entry's absolute-product sum is bounded by the largest
        // diagonal (Cauchy-Schwarz). Allow for all entries in a row, including
        // its cross terms with the group dummies, and the ordered dot products.
        let width = k as f64 + effect.n_levels() as f64;
        let rounding = 8. * f64::EPSILON * (n + width + 8.) * width;
        if rounding >= 0.5 {
            return Err(numerical());
        }
        margin -= rounding * max_diagonal;
        let movement = row_amplification * (max_score / margin);
        if margin <= 0. || !movement.is_finite() || movement > tolerance {
            return Err(numerical());
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_kept_contrast_refuses_a_false_zero_newton_step_in_both_orders() {
        let alias = 2_f64.powi(42);
        for order in [[0, 1, 2, 3], [3, 2, 1, 0]] {
            let s = order.map(|i| [0.5, -1., 0.5, 0.][i]);
            let x = order.map(|i| [0., 1., 2., 3.][i]);
            let z = order.map(|i| [0., 0., 1., 0.][i]);
            let x2: [f64; 4] = std::array::from_fn(|i| alias * x[i] + z[i]);
            let weights = [1.; 4];
            let tolerance = 1e-12;
            // The raw x2 moment passes despite a score of 1/2 along x2-alias*x.
            assert!(orthogonal(0..4, |i| Ok(x2[i]), &s, &weights, &s, tolerance).is_ok());
            let matrix = DenseMatrix::from_row_major(
                4,
                2,
                x.into_iter()
                    .zip(x2)
                    .flat_map(|(left, right)| [left, right])
                    .collect(),
            )
            .unwrap();
            let data = ProfiledColumns::selected(&matrix, &[0, 1]).unwrap();
            let effect = crate::fe::EffectTerm::new(vec![0; 4], 1, true, vec![]).unwrap();
            let mut p =
                ProfiledOneFe::reserve(4, 2, 1, ThreadConfig::from_requested(1, true).unwrap())
                    .unwrap();
            p.mean.copy_from_slice(&s);
            p.scales
                .copy_from_slice(&[5_f64.sqrt(), alias * 5_f64.sqrt()]);
            assert_eq!(
                p.verify_fisher_correction(
                    data, &effect, &weights, &s, &weights, &[0.; 4], &[0.; 2], tolerance
                )
                .unwrap_err()
                .code,
                ErrorCode::NonConvergence
            );
        }
    }

    #[test]
    fn weighted_center_certifies_orthogonal_leverage_rows_in_both_orders() {
        for order in [[0, 1, 2, 3], [3, 2, 1, 0]] {
            let x = order.map(|i| [2_f64.powi(40), 0., 1., 2.][i]);
            let s = order.map(|i| [0., 0.5, -1., 0.5][i]);
            let matrix = DenseMatrix::from_row_major(4, 1, x.to_vec()).unwrap();
            let data = ProfiledColumns::selected(&matrix, &[0]).unwrap();
            let effect = crate::fe::EffectTerm::new(vec![0; 4], 1, true, vec![]).unwrap();
            let mut p =
                ProfiledOneFe::reserve(4, 1, 1, ThreadConfig::from_requested(1, true).unwrap())
                    .unwrap();
            p.mean.copy_from_slice(&s);
            p.scales[0] = 2_f64.powi(40);
            p.verify_fisher_correction(
                data,
                &effect,
                &[1.; 4],
                &s,
                &[1.; 4],
                &[0.; 4],
                &[0.],
                1e-12,
            )
            .unwrap();
        }
    }
}
