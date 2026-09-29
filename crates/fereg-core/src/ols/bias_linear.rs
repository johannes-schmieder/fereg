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
    queue: Vec<(usize, usize)>,
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
                "bias response projection failed certification: stage=auxiliary_projection",
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
                format!("bias auxiliary normal equations failed: stage=auxiliary_normal_equations max_scaled={score:.17e} tolerance={:.17e}", fixed.certificate_tolerance),
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
            self.bias_recover_coefficients(fixed, &mut output[..levels])?;
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
        self.bias_recover_coefficients(fixed, &mut output[..levels])
    }

    fn bias_recover_coefficients(
        &self,
        fixed: &mut FixedPpmlInformation,
        coefficients: &mut [f64],
    ) -> Result<()> {
        let mut config = self.config.absorb;
        config.retain_coefficients = true;
        config.map.tolerance = config.map.tolerance.min(fixed.certificate_tolerance);
        config.lsmr.tolerance = config.lsmr.tolerance.min(fixed.certificate_tolerance);
        recover_with_refinement(
            self.fe_design(),
            &fixed.target,
            coefficients,
            &mut fixed.queue,
            &mut fixed.projected,
            fixed.certificate_tolerance,
            &mut fixed.maximum_reconstruction,
            |remainder| self.prepared.fit_absorb_matrix(self.weights.borrowed(), remainder, config),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn recover_with_refinement(
    design: &FeDesign,
    target: &[f64],
    coefficients: &mut [f64],
    queue: &mut Vec<(usize, usize)>,
    scratch: &mut Vec<f64>,
    tolerance: f64,
    maximum: &mut f64,
    refine: impl FnOnce(DenseMatrix) -> Result<CertifiedBorrowedMap>,
) -> Result<()> {
    match recover_anchored(design, target, coefficients, queue, tolerance, maximum) {
        Ok(()) => return Ok(()),
        Err(error) if error.code != ErrorCode::NonConvergence
            || target.iter().chain(coefficients.iter()).any(|x| !x.is_finite()) => return Err(error),
        Err(_) => {}
    }
    let left = &design.effects()[0];
    let right = &design.effects()[1];
    let offset = left.n_levels();
    if scratch.len() != target.len() {
        return Err(invalid_argument("bias FE refinement scratch is not row-aligned"));
    }
    // First-edge traversal can amplify the small non-additive remainder of
    // an accepted projection. Solve that remainder under the same information
    // weights; solving the large original target can hide the needed precision.
    for row in 0..target.len() {
        scratch[row] = target[row] - (coefficients[left.codes()[row] as usize]
            + coefficients[offset + right.codes()[row] as usize]);
    }
    let matrix = DenseMatrix::from_row_major(target.len(), 1, std::mem::take(scratch))?;
    let refined = refine(matrix).map_err(|mut error| {
        error.message = format!("stage=fe_recovery_refinement: {}", error.message);
        error
    })?;
    let (residual, _, increment) = refined.into_full_parts();
    *scratch = residual.into_row_major();
    let increment = increment.ok_or_else(|| invalid_data("bias FE refinement omitted coefficients"))?;
    let increment = increment.rhs(0)?;
    if increment.len() != coefficients.len() {
        return Err(invalid_data("bias FE refinement coefficient layout differs"));
    }
    // Traversal records each component's lowest right-hand anchor. Apply that
    // same linear normalization to the solver increment, including disconnected
    // components, before adding it to the original anchored coefficients.
    for &(node, anchor) in queue.iter() {
        coefficients[node] += increment[node]
            + if node < offset { increment[anchor] } else { -increment[anchor] };
    }
    certify_anchored(design, target, coefficients, tolerance, maximum)
        .map_err(|mut error| {
            error.message = format!("bias_recovery=refined {}", error.message);
            error
        })
}

fn recover_anchored(
    design: &FeDesign,
    target: &[f64],
    coefficients: &mut [f64],
    queue: &mut Vec<(usize, usize)>,
    tolerance: f64,
    maximum: &mut f64,
) -> Result<()> {
    if design.effects().len() != 2
        || design.effects().iter().any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
        || target.len() != design.nobs()
        || coefficients.len() != design.effects().iter().map(|e| e.n_levels()).sum::<usize>()
        || !tolerance.is_finite()
        || tolerance <= 0.
    {
        return Err(invalid_argument("bias anchored recovery inputs are inconsistent"));
    }
    let nonfinite_targets = target.iter().filter(|x| !x.is_finite()).count();
    if nonfinite_targets != 0 {
        return Err(FeregError::new(ErrorCode::NonConvergence,
            format!("bias anchored FE reconstruction failed: stage=fe_recovery nonfinite_target_rows={nonfinite_targets}")));
    }
    let left = &design.effects()[0];
    let right = &design.effects()[1];
    let offset = left.n_levels();
    coefficients.fill(f64::NAN);
    queue.clear();
    // Starting roots on the sorted second partition fixes its lowest level in
    // every component, independently of fitted weights and observation order.
    for root in offset..coefficients.len() {
        if coefficients[root].is_finite() {
            continue;
        }
        coefficients[root] = 0.;
        let mut head = queue.len();
        queue.push((root, root));
        while head < queue.len() {
            let node = queue[head].0;
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
                    let value = target[row] - coefficients[node];
                    // NaN marks an unvisited node; assigning nonfinite values
                    // could enqueue it repeatedly instead of terminating.
                    if !value.is_finite() {
                        return Err(FeregError::new(ErrorCode::NonConvergence,
                            format!("bias anchored FE reconstruction failed: stage=fe_recovery nonfinite_coefficient_row={row}")));
                    }
                    coefficients[other] = value;
                    queue.push((other, root));
                }
            }
        }
    }
    certify_anchored(design, target, coefficients, tolerance, maximum)
}

fn certify_anchored(
    design: &FeDesign,
    target: &[f64],
    coefficients: &[f64],
    tolerance: f64,
    maximum: &mut f64,
) -> Result<()> {
    let left = &design.effects()[0];
    let right = &design.effects()[1];
    let offset = left.n_levels();
    let mut worst_row = 0;
    let mut worst_scaled = 0.0_f64;
    let mut worst_absolute = 0.0_f64;
    let mut target_scale = 0.0_f64;
    let mut reconstructed_scale = 0.0_f64;
    let mut nonfinite = 0_usize;
    for row in 0..target.len() {
        let value = coefficients[left.codes()[row] as usize]
            + coefficients[offset + right.codes()[row] as usize];
        let absolute = (value - target[row]).abs();
        let error = absolute / target[row].abs().max(value.abs()).max(1.);
        if !error.is_finite() {
            nonfinite += 1;
        } else if error > worst_scaled {
            worst_scaled = error;
            worst_row = row;
        }
        worst_absolute = worst_absolute.max(absolute);
        target_scale = target_scale.max(target[row].abs());
        reconstructed_scale = reconstructed_scale.max(value.abs());
    }
    if nonfinite != 0 || worst_scaled > tolerance {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!("bias anchored FE reconstruction failed: stage=fe_recovery row={worst_row} max_scaled={worst_scaled:.17e} max_absolute={worst_absolute:.17e} tolerance={tolerance:.17e} target_scale={target_scale:.17e} reconstructed_scale={reconstructed_scale:.17e} nonfinite_rows={nonfinite}"),
        ));
    }
    *maximum = maximum.max(worst_scaled);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    fn design(left: &[u32], right: &[u32]) -> FeDesign {
        FeDesign::new(left.len(), vec![
            EffectTerm::new(left.to_vec(), *left.iter().max().unwrap() as usize + 1, true, vec![]).unwrap(),
            EffectTerm::new(right.to_vec(), *right.iter().max().unwrap() as usize + 1, true, vec![]).unwrap(),
        ]).unwrap()
    }

    fn native_refinement(design: &FeDesign, target: &[f64], coefficients: &mut [f64], weights: BorrowedMapWeights<'_>) -> Result<()> {
        let borrowed = crate::fe::BorrowedInterceptFeDesign::new(design.nobs(), design.effects().iter()
            .map(|e| crate::fe::BorrowedInterceptEffect::new(e.codes(), e.n_levels(), e.level_index()).unwrap())
            .collect()).unwrap();
        recover_with_refinement(
            design, target, coefficients, &mut vec![], &mut vec![0.; target.len()], 1e-9, &mut 0.,
            |remainder| crate::fe::absorb_borrowed_intercept_lsmr_certified(
                &borrowed, weights, remainder,
                crate::fe::LsmrConfig { tolerance: 1e-12,
                    ..crate::fe::LsmrConfig::default() }, true,
            ),
        )
    }

    #[test]
    fn anchored_recovery_cycles_repeated_pairs_and_disconnected_anchors() {
        let left = [0, 0, 1, 1, 1, 2, 2, 3, 3];
        let right = [0, 1, 0, 1, 1, 2, 3, 2, 3];
        let a = [2., -3., 7., 11.];
        let b = [0., 5., 0., -13.];
        for reverse in [false, true] {
            let mut order: Vec<_> = (0..left.len()).collect();
            if reverse { order.reverse(); }
            let l: Vec<_> = order.iter().map(|&i| left[i]).collect();
            let r: Vec<_> = order.iter().map(|&i| right[i]).collect();
            let y: Vec<_> = order.iter().map(|&i| a[left[i] as usize] + b[right[i] as usize]).collect();
            let design = design(&l, &r);
            let mut coefficients = vec![0.; 8];
            recover_anchored(&design, &y, &mut coefficients, &mut vec![], 1e-9, &mut 0.).unwrap();
            assert_eq!(coefficients, [a, b].concat());
        }
    }

    #[test]
    fn anchored_recovery_failure_reports_worst_row_and_rejects_nonfinite_targets() {
        let design = design(&[0, 0, 0], &[0, 0, 0]);
        let error = recover_anchored(&design, &[0., 1e-8, 2e-8], &mut [0.; 2], &mut vec![], 1e-9, &mut 0.).unwrap_err();
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert!(error.message.contains("row=2"), "{error}");
        assert!(error.message.contains("max_scaled="), "{error}");
        assert!(error.message.contains("tolerance="), "{error}");
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = recover_anchored(&design, &[0., bad, 0.], &mut [0.; 2], &mut vec![], 1e-9, &mut 0.).unwrap_err();
            assert!(error.message.contains("nonfinite_target_rows=1"), "{error}");
        }
    }

    #[test]
    fn anchored_recovery_refuses_arithmetic_overflow_and_invalid_shapes() {
        let design = design(&[0, 0, 1], &[0, 1, 1]);
        let error = recover_anchored(&design, &[1e308, -1e308, 0.], &mut [0.; 4], &mut vec![], 1e-9, &mut 0.).unwrap_err();
        assert!(error.message.contains("nonfinite_coefficient_row=1"), "{error}");
        let error = recover_anchored(&design, &[0.], &mut [0.; 4], &mut vec![], 1e-9, &mut 0.).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn anchored_recovery_first_edge_can_amplify_admissible_target_noise() {
        let design = design(&[0, 0, 1, 1, 0, 0, 1, 1], &[0, 1, 0, 1, 0, 1, 0, 1]);
        let exact = [0.125, 0.25, 0., 0.375];
        let mut target = vec![];
        for row in 0..design.nobs() {
            target.push(exact[design.effects()[0].codes()[row] as usize]
                + exact[2 + design.effects()[1].codes()[row] as usize]
                + if row < 4 { 0.75e-9 } else { -0.75e-9 });
        }
        let mut coefficients = [0.; 4];
        let error = recover_anchored(&design, &target, &mut coefficients, &mut vec![], 1e-9, &mut 0.).unwrap_err();
        assert_eq!(error.code, ErrorCode::NonConvergence);

        // Paired perturbations cancel exactly in every weighted normal
        // equation. The rational vector above is the independent LS oracle.
        native_refinement(&design, &target, &mut coefficients, BorrowedMapWeights::Unit).unwrap();
        for j in 0..4 {
            assert!((coefficients[j] - exact[j]).abs() < 1e-14);
        }
        for (row, y) in target.iter().enumerate() {
            let prediction = coefficients[design.effects()[0].codes()[row] as usize]
                + coefficients[2 + design.effects()[1].codes()[row] as usize];
            assert!((prediction-y).abs() <= 1e-9);
        }
        let borrowed = crate::fe::BorrowedInterceptFeDesign::new(design.nobs(), design.effects().iter()
            .map(|e| crate::fe::BorrowedInterceptEffect::new(e.codes(), e.n_levels(), e.level_index()).unwrap())
            .collect()).unwrap();
        recover_with_refinement(&design, &target, &mut coefficients, &mut vec![],
            &mut vec![0.; target.len()], 1e-9, &mut 0.,
            |remainder| crate::fe::absorb_borrowed_intercept_map_retaining_certified(
                &borrowed, BorrowedMapWeights::Unit, remainder,
                crate::fe::MapConfig { tolerance: 1e-12, ..crate::fe::MapConfig::default() },
            )).unwrap();
        for (actual, reference) in coefficients.iter().zip(exact) {
            assert!((actual-reference).abs() < 1e-14);
        }
    }

    #[test]
    fn anchored_recovery_preserves_large_cancelling_effects() {
        let design = design(&[0, 0, 1, 1], &[0, 1, 0, 1]);
        let scale = 2_f64.powi(30);
        let exact = [scale + 0.125, scale + 0.25, 0., -scale + 0.375];
        let target = [exact[0], 0.5, exact[1], 0.625];
        let mut coefficients = [0.; 4];
        recover_with_refinement(&design, &target, &mut coefficients,
            &mut vec![], &mut vec![0.; 4], 1e-9, &mut 0.,
            |_| panic!("consistent binary fractions must use traversal")).unwrap();
        assert_eq!(coefficients, exact);
    }

    #[test]
    fn anchored_recovery_rejects_a_genuinely_inconsistent_cycle() {
        let design = design(&[0, 0, 1, 1], &[0, 1, 0, 1]);
        let target = [0., 0., 0., 1.];
        let mut coefficients = [0.; 4];
        let failure = recover_anchored(
            &design, &target, &mut coefficients, &mut vec![], 1e-9, &mut 0.,
        ).unwrap_err();
        assert_eq!(failure.code, ErrorCode::NonConvergence);

        // An additive two-way target has zero alternating sum around a
        // four-cycle. This target's sum is one, so every coefficient vector
        // leaves an absolute residual of at least one quarter on some edge.
        let failure = native_refinement(&design, &target, &mut coefficients, BorrowedMapWeights::Unit).unwrap_err();
        assert!(failure.message.contains("bias_recovery=refined"), "{failure}");
        let residuals: Vec<_> = target.iter().enumerate().map(|(row, y)| {
            coefficients[design.effects()[0].codes()[row] as usize]
                + coefficients[2 + design.effects()[1].codes()[row] as usize] - y
        }).collect();
        for residual in residuals {
            assert!((residual.abs() - 0.25).abs() < 1e-14);
        }
    }

    #[test]
    fn anchored_refinement_preserves_disconnected_anchors_and_row_order() {
        let mut left = vec![0, 0, 1, 1, 2, 2, 3, 3];
        let mut right = vec![0, 1, 0, 1, 2, 3, 2, 3];
        left.extend_from_within(..);
        right.extend_from_within(..);
        let exact = [0.125, 0.25, 1.125, 1.25, 0., 0.375, 0., -0.75];
        let mut target: Vec<_> = (0..left.len()).map(|row| {
            exact[left[row] as usize] + exact[4 + right[row] as usize]
                + if row < 8 { 0.75e-9 } else { -0.75e-9 }
        }).collect();
        for reverse in [false, true] {
            if reverse { left.reverse(); right.reverse(); target.reverse(); }
            let design = design(&left, &right);
            let mut coefficients = [0.; 8];
            assert!(recover_anchored(&design, &target, &mut coefficients,
                &mut vec![], 1e-9, &mut 0.).is_err());
            native_refinement(&design, &target, &mut coefficients, BorrowedMapWeights::Unit).unwrap();
            for (actual, reference) in coefficients.iter().zip(exact) {
                assert!((actual-reference).abs() < 1e-13);
            }
            assert_eq!(coefficients[4], 0.);
            assert_eq!(coefficients[6], 0.);
        }
    }

    #[test]
    fn anchored_refinement_is_lazy_and_propagates_allocation_failure() {
        let design = design(&[0, 0, 1, 1], &[0, 1, 0, 1]);
        recover_with_refinement(&design, &[1., 2., 3., 4.], &mut [0.; 4],
            &mut vec![], &mut vec![0.; 4], 1e-9, &mut 0.,
            |_| panic!("exact traversal must not allocate a solver")).unwrap();
        let failure = recover_with_refinement(&design, &[0., 0., 0., 1.], &mut [0.; 4],
            &mut vec![], &mut vec![0.; 4], 1e-9, &mut 0.,
            |_| Err(FeregError::new(ErrorCode::Allocation, "injected refinement allocation"))).unwrap_err();
        assert_eq!(failure.code, ErrorCode::Allocation);
        assert!(failure.message.contains("stage=fe_recovery_refinement"));
        for bad in [f64::NAN, f64::INFINITY] {
            assert!(recover_with_refinement(&design, &[0., 0., bad, 1.], &mut [0.; 4],
                &mut vec![], &mut vec![0.; 4], 1e-9, &mut 0.,
                |_| panic!("nonfinite targets must not enter refinement")).is_err());
        }
    }

    #[test]
    fn anchored_recovery_refinement_preserves_weak_chain_effect_accuracy() {
        let levels = 129;
        let mut left = vec![];
        let mut right = vec![];
        for j in 0..levels {
            left.push(j as u32);
            right.push(j as u32);
            if j + 1 < levels {
                left.push(j as u32);
                right.push(j as u32 + 1);
            }
        }
        let edges = left.len();
        left.extend_from_within(..);
        right.extend_from_within(..);
        let design = design(&left, &right);
        let exact: Vec<_> = (0..levels).map(|j| 0.125 + j as f64 / 1024.)
            .chain((0..levels).map(|j| -(j as f64) / 1024.)).collect();
        for uneven in [false, true] {
            let target: Vec<_> = (0..left.len()).map(|row| {
                let sign = if row % edges % 2 == 0 { 1. } else { -1. };
                exact[left[row] as usize] + exact[levels + right[row] as usize]
                    + sign * if row < edges { 0.75e-9 } else { -0.75e-9 }
            }).collect();
            let weights: Vec<_> = (0..left.len()).map(|row|
                if uneven && row % edges == edges / 2 { 1e-6 } else { 1. }).collect();
            let mut coefficients = vec![0.; 2 * levels];
            assert!(recover_anchored(&design, &target, &mut coefficients, &mut vec![], 1e-9, &mut 0.).is_err());
            native_refinement(&design, &target, &mut coefficients, BorrowedMapWeights::Provided(&weights)).unwrap();
            let error = coefficients.iter().zip(&exact).map(|(a,b)| (a-b).abs()).fold(0., f64::max);
            assert!(error < 2e-11, "uneven={uneven} coefficient error={error:.17e}");
        }
    }

    #[test]
    #[ignore = "private diagnostic capture; requires FEREG_RECOVERY_INPUT and FEREG_RECOVERY_OUTPUT"]
    fn anchored_recovery_external_capture() {
        use std::io::Write;
        let input = std::fs::read(std::env::var_os("FEREG_RECOVERY_INPUT").unwrap()).unwrap();
        let mut cursor = 0;
        let mut take = |n: usize| {
            let start = cursor;
            cursor += n;
            &input[start..cursor]
        };
        let n = u64::from_le_bytes(take(8).try_into().unwrap()) as usize;
        let nl = u64::from_le_bytes(take(8).try_into().unwrap()) as usize;
        let nr = u64::from_le_bytes(take(8).try_into().unwrap()) as usize;
        let original: Vec<_> = (0..nl+nr).map(|_| f64::from_le_bytes(take(8).try_into().unwrap())).collect();
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        let mut weights = Vec::with_capacity(n);
        let mut target = Vec::with_capacity(n);
        for _ in 0..n {
            left.push(u32::from_le_bytes(take(4).try_into().unwrap()));
            right.push(u32::from_le_bytes(take(4).try_into().unwrap()));
            weights.push(f64::from_le_bytes(take(8).try_into().unwrap()));
            target.push(f64::from_le_bytes(take(8).try_into().unwrap()));
        }
        assert_eq!(cursor, input.len());
        let design = design(&left, &right);
        assert_eq!(design.effects()[0].n_levels(), nl);
        assert_eq!(design.effects()[1].n_levels(), nr);
        let mut coefficients = vec![0.; nl+nr];
        assert!(recover_anchored(&design, &target, &mut coefficients,
            &mut vec![], 1e-9, &mut 0.).is_err());
        assert_eq!(coefficients, original);
        let started = std::time::Instant::now();
        native_refinement(&design, &target, &mut coefficients,
            BorrowedMapWeights::Provided(&weights)).unwrap();
        let seconds = started.elapsed().as_secs_f64();
        let mut maximum = 0.;
        certify_anchored(&design, &target, &coefficients, 1e-9, &mut maximum).unwrap();
        // Keep coefficient arrays on the data host; only aggregate certificate
        // and timing values go to the test log.
        let mut output = std::fs::OpenOptions::new().write(true).create_new(true)
            .open(std::env::var_os("FEREG_RECOVERY_OUTPUT").unwrap()).unwrap();
        for coefficient in coefficients { output.write_all(&coefficient.to_le_bytes()).unwrap(); }
        println!("NATIVE_CAPTURE_RECOVERY_PASS max_scaled={maximum:.17e} seconds={seconds:.9}");
    }
}
