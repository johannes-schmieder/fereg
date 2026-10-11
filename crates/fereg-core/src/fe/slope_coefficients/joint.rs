//! Joint ordinary WLS directions over the FE Schur and finite coordinates.
//! The canonical session owns rank and final bread. This owner retains capacity,
//! never finite products or acceptance across a changing response/weight frame.
use super::*;
use crate::fe::SolveReport;

/// An immutable finite representative. Row and column access let preparation
/// follow the source layout without materializing another normalized design.
pub(crate) trait JointFiniteDesign: Sync {
    // Direct row storage can fuse independent accumulators; recipe-backed
    // rows benefit from resolving one column before scanning observations.
    fn prefers_column_scans(&self) -> bool {
        false
    }
    fn nrows(&self) -> usize;
    fn ncols(&self) -> usize;
    fn column_slice(&self, _column: usize) -> Option<&[f64]> {
        None
    }
    fn visit_column(
        &self,
        column: usize,
        visit: impl FnMut(usize, f64) -> Result<()>,
    ) -> Result<()> {
        self.visit_column_range(column, 0..self.nrows(), visit)
    }
    fn visit_column_range(
        &self,
        column: usize,
        range: std::ops::Range<usize>,
        visit: impl FnMut(usize, f64) -> Result<()>,
    ) -> Result<()>;
    fn visit_rows(
        &self,
        row_scratch: &mut [f64],
        visit: impl FnMut(usize, &[f64]) -> Result<()>,
    ) -> Result<()>;
}

impl JointFiniteDesign for DenseMatrix {
    fn nrows(&self) -> usize {
        self.nrows()
    }
    fn ncols(&self) -> usize {
        self.ncols()
    }
    fn visit_column_range(
        &self,
        column: usize,
        range: std::ops::Range<usize>,
        mut visit: impl FnMut(usize, f64) -> Result<()>,
    ) -> Result<()> {
        for row in range {
            visit(row, self[(row, column)])?;
        }
        Ok(())
    }

    fn visit_rows(
        &self,
        _: &mut [f64],
        mut visit: impl FnMut(usize, &[f64]) -> Result<()>,
    ) -> Result<()> {
        for (row, values) in self.as_slice().chunks(self.ncols()).enumerate() {
            visit(row, values)?;
        }
        Ok(())
    }
}

pub(crate) struct JointFiniteWorkspace {
    cross: Vec<f64>,
    hessian: Vec<f64>,
    factor: Vec<f64>,
    scales: Vec<f64>,
    column_scratch: Vec<f64>,
    coefficients: Vec<f64>,
    residual: Vec<f64>,
    direction: Vec<f64>,
    product: Vec<f64>,
    preconditioned: Vec<f64>,
    beta: Vec<f64>,
    pub(crate) residuals: DenseMatrix,
    pub(crate) report: Option<SolveReport>,
    max_controls: usize,
    active: usize,
}

fn failed() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "joint slope direction failed current-weight certification",
    )
}

// Independent accumulation chains remove the dense Gram's serial dependency.
// The reduction order is independent of worker scheduling.
pub(crate) fn normalized_product(left: &[f64], right: &[f64]) -> f64 {
    crate::kernels::dispatch!(crate::kernels::installed(), normalized_product(left, right))
}

fn finite_product<C: JointFiniteDesign>(
    controls: &C,
    selected: usize,
    roots: &[f64],
    projected: &[f64],
    scale: f64,
) -> Result<f64> {
    // The current weighted column has a finite, already checked norm.
    // Normalize the dot product once, rather than dividing every observation.
    if let Some(values) = controls.column_slice(selected) {
        Ok(crate::kernels::dispatch!(
            crate::kernels::installed(),
            weighted_product(values, roots, projected)
        ) / scale)
    } else {
        let mut sums = [0.0_f64; 8];
        controls.visit_column(selected, |row, value| {
            let lane = row % sums.len();
            sums[lane] = (value * roots[row]).mul_add(projected[row], sums[lane]);
            Ok(())
        })?;
        Ok(sums.iter().sum::<f64>() / scale)
    }
}

// Independent row panels borrow contiguous original columns. Each residual
// keeps its selected-column FMA order, while the inner loop follows the source
// layout. No normalized N-by-K bank or worker scratch is needed.
fn reconstruct_direct_columns<C: JointFiniteDesign>(
    controls: &C,
    selected: &[usize],
    coefficients: &[f64],
    scales: &[f64],
    roots: &[f64],
    original: &[f64],
    observations: &mut [f64],
) -> bool {
    if !selected.iter().all(|&column| {
        controls
            .column_slice(column)
            .is_some_and(|values| values.len() == observations.len())
    }) {
        return false;
    }
    const ROWS: usize = 1024;
    let hardware = crate::kernels::installed();
    observations
        .par_chunks_mut(ROWS)
        .enumerate()
        .for_each(|(panel, target)| {
            let start = panel * ROWS;
            let end = start + target.len();
            for (value, &input) in target.iter_mut().zip(&original[start..end]) {
                *value = input - *value;
            }
            for (j, &column) in selected.iter().enumerate() {
                let values = controls
                    .column_slice(column)
                    .expect("validated immutable column");
                crate::kernels::dispatch!(
                    hardware,
                    normalized_add(
                        -coefficients[j],
                        &values[start..end],
                        &roots[start..end],
                        scales[j],
                        target
                    )
                );
            }
        });
    true
}

impl JointFiniteWorkspace {
    pub(crate) fn bytes(design: &FeDesign, controls: usize) -> Result<u128> {
        let (_, p, _, _) = dimensions(design, 1, 1)?;
        let q = design.effects()[eliminated_effect(design)].channels();
        let k = controls as u128;
        let square = k
            .checked_mul(k)
            .and_then(|n| n.checked_mul(2))
            .ok_or_else(allocation)?;
        let small = (q as u128)
            .checked_add(2)
            .and_then(|q| q.checked_mul(k))
            .ok_or_else(allocation)?;
        let vectors = (p as u128)
            .checked_add(k)
            .and_then(|n| n.checked_mul(5))
            .ok_or_else(allocation)?;
        let values = (p as u128)
            .checked_mul(k)
            .and_then(|n| n.checked_add(square))
            .and_then(|n| n.checked_add(small))
            .and_then(|n| n.checked_add(vectors))
            .and_then(|n| n.checked_add(design.nobs() as u128))
            .ok_or_else(allocation)?;
        values
            .checked_mul(8)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() as u128))
            .ok_or_else(allocation)
    }

    pub(super) fn new(design: &FeDesign, controls: usize, p: usize) -> Result<Self> {
        let square = controls.checked_mul(controls).ok_or_else(allocation)?;
        let size = p.checked_add(controls).ok_or_else(allocation)?;
        let q = design.effects()[eliminated_effect(design)].channels();
        Ok(Self {
            cross: filled(p.checked_mul(controls).ok_or_else(allocation)?, 0.)?,
            hessian: filled(square, 0.)?,
            factor: filled(square, 0.)?,
            scales: filled(controls, 0.)?,
            column_scratch: filled(q.checked_mul(controls).ok_or_else(allocation)?, 0.)?,
            coefficients: filled(size, 0.)?,
            residual: filled(size, 0.)?,
            direction: filled(size, 0.)?,
            product: filled(size, 0.)?,
            preconditioned: filled(size, 0.)?,
            beta: filled(controls, 0.)?,
            residuals: DenseMatrix::from_row_major(design.nobs(), 1, filled(design.nobs(), 0.)?)?,
            report: None,
            max_controls: controls,
            active: 0,
        })
    }

    pub(in crate::fe) fn elements(&self) -> usize {
        [
            &self.cross,
            &self.hessian,
            &self.factor,
            &self.scales,
            &self.column_scratch,
            &self.coefficients,
            &self.residual,
            &self.direction,
            &self.product,
            &self.preconditioned,
            &self.beta,
        ]
        .iter()
        .map(|v| v.capacity())
        .sum::<usize>()
            + self.residuals.as_slice().len()
    }

    pub(crate) fn beta(&self) -> &[f64] {
        &self.beta[..self.active]
    }

    /// Norms of this successful call's input columns, in selected-column order.
    /// Dense representatives and streamed originals have different meanings;
    /// the caller must retain that input identity. No failed call exposes norms.
    pub(crate) fn input_norms(&self) -> Option<&[f64]> {
        self.report
            .as_ref()
            .filter(|report| report.certified())
            .map(|_| &self.scales[..self.active])
    }

    fn precondition(&mut self, p: usize, k: usize) {
        self.preconditioned[..p + k].copy_from_slice(&self.residual[..p + k]);
        for i in 0..k {
            for j in 0..i {
                self.preconditioned[p + i] -= self.factor[i * k + j] * self.preconditioned[p + j];
            }
            self.preconditioned[p + i] /= self.factor[i * k + i];
        }
        for i in (0..k).rev() {
            for j in i + 1..k {
                self.preconditioned[p + i] -= self.factor[j * k + i] * self.preconditioned[p + j];
            }
            self.preconditioned[p + i] /= self.factor[i * k + i];
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn reconstruct<C: JointFiniteDesign>(
        &mut self,
        op: &Operator<'_>,
        controls: &C,
        selected: &[usize],
        roots: &[f64],
        original: &[f64],
        observations: &mut [f64],
        scratch: &mut [f64],
    ) -> Result<f64> {
        let p = op.coefficients;
        op.forward(&self.coefficients[..p], observations);
        let direct = reconstruct_direct_columns(
            controls,
            selected,
            &self.coefficients[p..p + selected.len()],
            &self.scales[..selected.len()],
            roots,
            original,
            observations,
        );
        if !direct && controls.prefers_column_scans() {
            // Each worker owns disjoint residual rows. Recipe tiles preserve
            // finite-coordinate FMA order without serial full-vector passes.
            observations.par_chunks_mut(1024).enumerate().try_for_each(
                |(panel, target)| -> Result<()> {
                    let start = panel * 1024;
                    let end = start + target.len();
                    for (value, &input) in target.iter_mut().zip(&original[start..end]) {
                        *value = input - *value;
                    }
                    for (j, &column) in selected.iter().enumerate() {
                        controls.visit_column_range(column, start..end, |row, value| {
                            target[row - start] = (-self.coefficients[p + j])
                                .mul_add(value * roots[row] / self.scales[j], target[row - start]);
                            Ok(())
                        })?;
                    }
                    Ok(())
                },
            )?;
        } else if !direct {
            controls.visit_rows(&mut self.column_scratch, |row, values| {
                observations[row] = crate::kernels::dispatch!(
                    crate::kernels::installed(),
                    normalized_selected_residual(
                        original[row] - observations[row],
                        values,
                        selected,
                        &self.scales[..selected.len()],
                        &self.coefficients[p..p + selected.len()],
                        roots[row]
                    )
                );
                Ok(())
            })?;
        }
        op.project_eliminated(observations, scratch);
        let mut maximum = op.transpose(observations, &mut self.residual[..p], true);
        self.residual[p..p + selected.len()].fill(0.);
        if direct {
            // Scores are fresh original-column scans after eliminated-effect
            // projection. Columns are independent; each keeps its row order.
            self.residual[p..p + selected.len()]
                .par_iter_mut()
                .zip(selected.par_iter())
                .zip(self.scales[..selected.len()].par_iter())
                .for_each(|((sum, &column), &scale)| {
                    let values = controls
                        .column_slice(column)
                        .expect("validated immutable column");
                    *sum = crate::kernels::dispatch!(
                        crate::kernels::installed(),
                        normalized_dot(values, roots, scale, observations)
                    );
                });
        } else if controls.prefers_column_scans() {
            self.residual[p..p + selected.len()]
                .par_iter_mut()
                .zip(selected.par_iter())
                .zip(self.scales[..selected.len()].par_iter())
                .try_for_each(|((sum, &column), &scale)| -> Result<()> {
                    controls.visit_column(column, |row, value| {
                        *sum = (value * roots[row] / scale).mul_add(observations[row], *sum);
                        Ok(())
                    })
                })?;
        } else {
            controls.visit_rows(&mut self.column_scratch, |row, values| {
                crate::kernels::dispatch!(
                    crate::kernels::installed(),
                    normalized_selected_scores(
                        values,
                        selected,
                        &self.scales[..selected.len()],
                        roots[row],
                        observations[row],
                        &mut self.residual[p..p + selected.len()]
                    )
                );
                Ok(())
            })?;
        }
        for &sum in &self.residual[p..p + selected.len()] {
            if !sum.is_finite() {
                return Ok(f64::INFINITY);
            }
            maximum = maximum.max(sum.abs());
        }
        Ok(if observations.iter().any(|v| !v.is_finite()) {
            f64::INFINITY
        } else {
            maximum
        })
    }

    /// Reuse the existing normalized RHS bank for y and projected finite columns.
    /// The original retained representatives are borrowed and never overwritten.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::fe) fn solve(
        &mut self,
        owner: &mut SlopeCoefficientWorkspace,
        design: &FeDesign,
        geometry: &ReusableCertificateGeometry,
        weights: &[f64],
        controls: &DenseMatrix,
        selected: &[usize],
        response: &[f64],
        config: MapConfig,
        reusable_geometry: bool,
    ) -> Result<usize> {
        self.report = None;
        self.active = 0;
        let n = design.nobs();
        let k = selected.len();
        let p = owner.coefficients;
        if k == 0
            || k > self.max_controls
            || k >= owner.max_rhs
            || response.len() != n
            || weights.len() != n
            || controls.nrows() != n
            || selected.iter().any(|&c| c >= controls.ncols())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "joint slope inputs exceed their admitted dimensions",
            ));
        }
        if !reusable_geometry {
            if let Some(coupling) = owner.coupling.as_mut() {
                coupling.prepare(design, geometry, &owner.groups);
            }
        }
        let op = Operator {
            coupling: owner.coupling.as_deref(),
            design,
            geometry,
            groups: &owner.groups,
            coefficients: p,
            eliminated: eliminated_effect(design),
        };
        let q = design.effects()[op.eliminated].channels();
        let (original, bank) = owner.columns.split_at_mut(n);
        let z = &mut bank[..n * k];
        let roots = &mut owner.roots;
        let (worker, helpers) = owner.workers.split_first_mut().expect("admitted worker");
        let observations = &mut worker.observations;
        let scratch = &mut worker.eliminated_scratch;
        for row in 0..n {
            roots[row] = weights[row].sqrt();
            original[row] = response[row] * roots[row];
        }
        let ynorm = super::super::map::reusable_norm(original);
        if !ynorm.is_finite() {
            return Err(failed());
        }
        let scale_y = if ynorm == 0. { 1. } else { ynorm };
        for value in original.iter_mut() {
            *value /= scale_y;
        }
        let roots = roots.as_slice();
        z.par_chunks_mut(n)
            .zip(self.scales[..k].par_iter_mut())
            .zip(self.column_scratch[..q * k].par_chunks_mut(q))
            .zip(self.cross[..p * k].par_chunks_mut(p))
            .enumerate()
            .try_for_each(|(j, (((column, scale), work), cross))| -> Result<()> {
                for row in 0..n {
                    column[row] = controls[(row, selected[j])] * roots[row];
                }
                *scale = super::super::map::reusable_norm(column);
                if !scale.is_finite() || *scale <= 0. {
                    return Err(failed());
                }
                for value in column.iter_mut() {
                    *value /= *scale;
                }
                op.project_eliminated(column, work);
                op.transpose(column, cross, false);
                Ok(())
            })?;
        self.hessian[..k * k]
            .par_chunks_mut(k)
            .enumerate()
            .for_each(|(i, row)| {
                for j in i..k {
                    row[j] = normalized_product(&z[i * n..(i + 1) * n], &z[j * n..(j + 1) * n]);
                }
            });
        for i in 0..k {
            for j in 0..i {
                self.hessian[i * k + j] = self.hessian[j * k + i];
            }
        }
        self.iterate(
            &op,
            controls,
            selected,
            roots,
            original,
            Some(z),
            scale_y,
            observations,
            scratch,
            helpers,
            config,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn iterate<C: JointFiniteDesign>(
        &mut self,
        op: &Operator<'_>,
        controls: &C,
        selected: &[usize],
        roots: &[f64],
        original: &[f64],
        z: Option<&[f64]>,
        scale_y: f64,
        observations: &mut [f64],
        scratch: &mut [f64],
        helpers: &mut [Worker],
        config: MapConfig,
    ) -> Result<usize> {
        let hardware = crate::kernels::installed();
        let n = original.len();
        let k = selected.len();
        let p = op.coefficients;
        self.factor[..k * k].copy_from_slice(&self.hessian[..k * k]);
        for i in 0..k {
            for j in 0..=i {
                let mut value = self.factor[i * k + j];
                for a in 0..j {
                    value -= self.factor[i * k + a] * self.factor[j * k + a];
                }
                if !value.is_finite() {
                    return Err(failed());
                }
                if i == j {
                    if value <= 32. * f64::EPSILON {
                        return Err(failed());
                    }
                    self.factor[i * k + j] = value.sqrt();
                } else {
                    self.factor[i * k + j] = value / self.factor[j * k + j];
                }
            }
        }
        let dimension = p + k;
        self.coefficients[..dimension].fill(0.);
        observations.copy_from_slice(original);
        op.project_eliminated(observations, scratch);
        op.transpose(observations, &mut self.residual[..p], false);
        if let Some(z) = z {
            for j in 0..k {
                self.residual[p + j] = dot(&z[j * n..(j + 1) * n], original);
            }
        }
        self.precondition(p, k);
        self.direction[..dimension].copy_from_slice(&self.preconditioned[..dimension]);
        let mut square = dot(
            &self.residual[..dimension],
            &self.preconditioned[..dimension],
        );
        for iteration in 0..=config.max_iterations {
            if dot(&self.residual[..dimension], &self.residual[..dimension]).sqrt()
                <= config.tolerance * 0.25
                || iteration == config.max_iterations
            {
                let maximum = self.reconstruct(
                    op,
                    controls,
                    selected,
                    roots,
                    original,
                    observations,
                    scratch,
                )?;
                if maximum <= config.tolerance * 0.5 {
                    for j in 0..k {
                        self.beta[j] = self.coefficients[p + j] * scale_y / self.scales[j];
                        if !self.beta[j].is_finite() {
                            return Err(failed());
                        }
                    }
                    for row in 0..n {
                        let value = observations[row] * scale_y / roots[row];
                        if !value.is_finite() {
                            return Err(failed());
                        }
                        self.residuals[(row, 0)] = value;
                    }
                    self.active = k;
                    return Ok(iteration);
                }
                self.precondition(p, k);
                self.direction[..dimension].copy_from_slice(&self.preconditioned[..dimension]);
                square = dot(
                    &self.residual[..dimension],
                    &self.preconditioned[..dimension],
                );
            }
            if iteration == config.max_iterations {
                break;
            }
            if op
                .coupling
                .and_then(|c| {
                    c.apply_with_helpers(
                        &self.direction[..p],
                        &mut self.product[..p],
                        scratch,
                        helpers,
                    )
                })
                .is_none()
            {
                op.forward(&self.direction[..p], observations);
                op.project_eliminated(observations, scratch);
                op.transpose(observations, &mut self.product[..p], false);
            }
            for j in 0..k {
                let cross = &self.cross[j * p..(j + 1) * p];
                self.product[p + j] = dot(cross, &self.direction[..p])
                    + dot(
                        &self.hessian[j * k..(j + 1) * k],
                        &self.direction[p..dimension],
                    );
                crate::kernels::dispatch!(
                    hardware,
                    scaled_add(cross, self.direction[p + j], &mut self.product[..p])
                );
            }
            let mut curvature = dot(&self.direction[..dimension], &self.product[..dimension]);
            let magnitude = self.direction[..dimension]
                .iter()
                .zip(&self.product[..dimension])
                .fold(0., |s, (&a, &b)| s + (a * b).abs());
            if !curvature.is_finite() || curvature <= 128. * f64::EPSILON * magnitude {
                // Cross-block cancellation must not turn a positive operator
                // into a negative-curvature step. Reapply in observation space.
                op.forward(&self.direction[..p], observations);
                if let Some(z) = z {
                    for row in 0..n {
                        for j in 0..k {
                            observations[row] =
                                z[j * n + row].mul_add(self.direction[p + j], observations[row]);
                        }
                    }
                } else {
                    controls.visit_rows(&mut self.column_scratch, |row, values| {
                        for (j, &column) in selected.iter().enumerate() {
                            observations[row] = (values[column] * roots[row] / self.scales[j])
                                .mul_add(self.direction[p + j], observations[row]);
                        }
                        Ok(())
                    })?;
                }
                op.project_eliminated(observations, scratch);
                curvature = dot(observations, observations);
                op.transpose(observations, &mut self.product[..p], false);
                if let Some(z) = z {
                    for j in 0..k {
                        self.product[p + j] = dot(&z[j * n..(j + 1) * n], observations);
                    }
                } else {
                    self.product[p..dimension].fill(0.);
                    controls.visit_rows(&mut self.column_scratch, |row, values| {
                        for (j, &column) in selected.iter().enumerate() {
                            self.product[p + j] = (values[column] * roots[row] / self.scales[j])
                                .mul_add(observations[row], self.product[p + j]);
                        }
                        Ok(())
                    })?;
                }
            }
            if !curvature.is_finite() || curvature <= 0. || !square.is_finite() || square < 0. {
                return Err(failed());
            }
            let step = square / curvature;
            crate::kernels::dispatch!(
                hardware,
                joint_step(
                    step,
                    &self.direction[..dimension],
                    &self.product[..dimension],
                    &mut self.coefficients[..dimension],
                    &mut self.residual[..dimension]
                )
            );
            self.precondition(p, k);
            let next = dot(
                &self.residual[..dimension],
                &self.preconditioned[..dimension],
            );
            let ratio = next / square;
            crate::kernels::dispatch!(
                hardware,
                affine_direction(
                    ratio,
                    &self.preconditioned[..dimension],
                    &mut self.direction[..dimension]
                )
            );
            square = next;
        }
        Err(failed())
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::fe) fn solve_streamed<C: JointFiniteDesign>(
        &mut self,
        owner: &mut SlopeCoefficientWorkspace,
        design: &FeDesign,
        geometry: &ReusableCertificateGeometry,
        weights: &[f64],
        controls: &C,
        selected: &[usize],
        response: &[f64],
        config: MapConfig,
        reusable_geometry: bool,
    ) -> Result<usize> {
        self.report = None;
        self.active = 0;
        let n = design.nobs();
        let k = selected.len();
        let p = owner.coefficients;
        if k == 0
            || k > self.max_controls
            || response.len() != n
            || weights.len() != n
            || controls.ncols() > self.max_controls
            || controls.nrows() != n
            || selected.iter().any(|&c| c >= controls.ncols())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "joint slope inputs exceed their admitted dimensions",
            ));
        }
        if !reusable_geometry {
            if let Some(coupling) = owner.coupling.as_mut() {
                coupling.prepare(design, geometry, &owner.groups);
            }
        }
        let op = Operator {
            coupling: owner.coupling.as_deref(),
            design,
            geometry,
            groups: &owner.groups,
            coefficients: p,
            eliminated: eliminated_effect(design),
        };
        let original = &mut owner.columns[..n];
        let roots = &mut owner.roots;
        for row in 0..n {
            roots[row] = weights[row].sqrt();
            original[row] = response[row] * roots[row];
        }
        let ynorm = super::super::map::reusable_norm(original);
        if !ynorm.is_finite() {
            return Err(failed());
        }
        let scale_y = if ynorm == 0. { 1. } else { ynorm };
        for value in original.iter_mut() {
            *value /= scale_y;
        }
        let roots = roots.as_slice();
        // Norm state occupies existing banks before their solve lifetimes.
        // Each column retains its stable row-order reduction; source-oriented
        // inputs let independent columns use the admitted worker team.
        self.scales[..k].fill(0.);
        self.beta[..k].fill(1.);
        if controls.prefers_column_scans() {
            self.scales[..k]
                .par_iter_mut()
                .zip(self.beta[..k].par_iter_mut())
                .zip(selected.par_iter())
                .try_for_each(|((scale, square), &column)| {
                    controls.visit_column(column, |row, value| {
                        let absolute = (value * roots[row]).abs();
                        if absolute != 0. {
                            if *scale < absolute {
                                let ratio = *scale / absolute;
                                *square = 1. + *square * ratio * ratio;
                                *scale = absolute;
                            } else {
                                let ratio = absolute / *scale;
                                *square += ratio * ratio;
                            }
                        }
                        Ok(())
                    })
                })?;
        } else {
            controls.visit_rows(&mut self.factor[..controls.ncols()], |row, values| {
                for (j, &column) in selected.iter().enumerate() {
                    let absolute = (values[column] * roots[row]).abs();
                    if absolute == 0. {
                        continue;
                    }
                    if self.scales[j] < absolute {
                        let ratio = self.scales[j] / absolute;
                        self.beta[j] = 1. + self.beta[j] * ratio * ratio;
                        self.scales[j] = absolute;
                    } else {
                        let ratio = absolute / self.scales[j];
                        self.beta[j] += ratio * ratio;
                    }
                }
                Ok(())
            })?;
        }
        for j in 0..k {
            self.scales[j] *= self.beta[j].sqrt();
            if !self.scales[j].is_finite() || self.scales[j] <= 0. {
                return Err(failed());
            }
        }
        // Only the admitted worker observation buffers hold projected columns.
        // Each Hessian row reads resolved source columns and uses the same
        // fixed product reduction, independent of the admitted team width.
        let width = owner.workers.len().min(k);
        self.hessian[..k * k].fill(0.);
        for first in (0..k).step_by(width) {
            let last = (first + width).min(k);
            owner.workers[..last - first]
                .par_iter_mut()
                .zip(self.cross[first * p..last * p].par_chunks_mut(p))
                .zip(self.residual[p + first..p + last].par_iter_mut())
                .zip(self.hessian[first * k..last * k].par_chunks_mut(k))
                .enumerate()
                .try_for_each(|(local, (((worker, cross), rhs), hessian_row))| {
                    let j = first + local;
                    let column = &mut worker.observations;
                    controls.visit_column(selected[j], |row, value| {
                        column[row] = value * roots[row] / self.scales[j];
                        Ok(())
                    })?;
                    op.project_eliminated(column, &mut worker.eliminated_scratch);
                    op.transpose(column, cross, false);
                    *rhs = dot(column, original);
                    for (entry, (&selected, &scale)) in hessian_row[j..]
                        .iter_mut()
                        .zip(selected[j..].iter().zip(&self.scales[j..k]))
                    {
                        *entry = finite_product(controls, selected, roots, column, scale)?;
                    }
                    Ok(())
                })?;
        }
        let (worker, helpers) = owner.workers.split_first_mut().expect("admitted worker");
        let observations = &mut worker.observations;
        let scratch = &mut worker.eliminated_scratch;
        for i in 0..k {
            for j in 0..i {
                self.hessian[i * k + j] = self.hessian[j * k + i];
            }
        }
        self.iterate(
            &op,
            controls,
            selected,
            roots,
            original,
            None,
            scale_y,
            observations,
            scratch,
            helpers,
            config,
        )
    }
}

#[cfg(test)]
mod product_tests {
    use super::*;

    struct Column {
        values: Vec<f64>,
        contiguous: bool,
    }

    impl JointFiniteDesign for Column {
        fn nrows(&self) -> usize {
            self.values.len()
        }
        fn ncols(&self) -> usize {
            1
        }
        fn column_slice(&self, _: usize) -> Option<&[f64]> {
            self.contiguous.then_some(self.values.as_slice())
        }
        fn visit_column_range(
            &self,
            _: usize,
            range: std::ops::Range<usize>,
            mut visit: impl FnMut(usize, f64) -> Result<()>,
        ) -> Result<()> {
            for row in range {
                visit(row, self.values[row])?;
            }
            Ok(())
        }
        fn visit_rows(
            &self,
            _: &mut [f64],
            mut visit: impl FnMut(usize, &[f64]) -> Result<()>,
        ) -> Result<()> {
            for (row, value) in self.values.iter().enumerate() {
                visit(row, std::slice::from_ref(value))?;
            }
            Ok(())
        }
    }

    struct Columns {
        values: Vec<Vec<f64>>,
        missing: Option<usize>,
    }

    impl JointFiniteDesign for Columns {
        fn nrows(&self) -> usize {
            self.values[0].len()
        }
        fn ncols(&self) -> usize {
            self.values.len()
        }
        fn column_slice(&self, column: usize) -> Option<&[f64]> {
            (self.missing != Some(column)).then(|| self.values[column].as_slice())
        }
        fn visit_column_range(
            &self,
            _: usize,
            _: std::ops::Range<usize>,
            _: impl FnMut(usize, f64) -> Result<()>,
        ) -> Result<()> {
            unreachable!()
        }
        fn visit_rows(
            &self,
            _: &mut [f64],
            _: impl FnMut(usize, &[f64]) -> Result<()>,
        ) -> Result<()> {
            unreachable!()
        }
    }

    #[test]
    fn direct_reconstruction_preserves_order_panels_scales_and_refusal() {
        for threads in [1, 4, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                for n in [1, 1023, 1024, 1025, 2083] {
                    for k in [1, 7, 33, 65] {
                        let scales: Vec<_> = (0..k).map(|j| [1e-200, 1., 1e200][j % 3]).collect();
                        let roots: Vec<_> = (0..n).map(|i| [0.125, 0.5, 1., 4.][i % 4]).collect();
                        let original: Vec<_> = (0..n).map(|i| (i as f64 / 13.).sin()).collect();
                        let coefficients: Vec<_> = (0..k).map(|j| (j as f64 + 1.).cos()).collect();
                        let selected: Vec<_> = (0..k).map(|j| 2 * j).collect();
                        let mut controls = Columns {
                            values: (0..2 * k)
                                .map(|j| {
                                    (0..n)
                                        .map(|i| ((i + j) as f64 / 17.).sin() * scales[j / 2])
                                        .collect()
                                })
                                .collect(),
                            missing: None,
                        };
                        let initial: Vec<_> = (0..n).map(|i| (i % 7) as f64 / 3.).collect();
                        let expected: Vec<_> = (0..n)
                            .map(|i| {
                                selected.iter().enumerate().fold(
                                    original[i] - initial[i],
                                    |r, (j, &c)| {
                                        (-coefficients[j]).mul_add(
                                            controls.values[c][i] * roots[i] / scales[j],
                                            r,
                                        )
                                    },
                                )
                            })
                            .collect();
                        let mut actual = initial.clone();
                        assert!(reconstruct_direct_columns(
                            &controls,
                            &selected,
                            &coefficients,
                            &scales,
                            &roots,
                            &original,
                            &mut actual
                        ));
                        assert_eq!(
                            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                        );
                        controls.missing = Some(selected[k - 1]);
                        actual.copy_from_slice(&initial);
                        assert!(!reconstruct_direct_columns(
                            &controls,
                            &selected,
                            &coefficients,
                            &scales,
                            &roots,
                            &original,
                            &mut actual
                        ));
                        assert_eq!(actual, initial);
                    }
                }
            });
        }
    }

    #[test]
    fn normalized_products_preserve_independent_signed_inner_products_and_tails() {
        for n in [1, 7, 8, 9, 17, 8193] {
            let left: Vec<_> = (0..n)
                .map(|i| (i as f64 * 1.731).sin() / (n as f64).sqrt())
                .collect();
            let right: Vec<_> = (0..n)
                .map(|i| ((i % 13) as f64 - 6.) / (7. * (n as f64).sqrt()))
                .collect();
            let expected: f64 = left.iter().zip(&right).map(|(a, b)| a * b).sum();
            assert!((normalized_product(&left, &right) - expected).abs() < 1e-13);
            assert!((normalized_product(&right, &left) - expected).abs() < 1e-13);
        }
    }

    #[test]
    fn finite_products_preserve_scaled_weighted_inner_products_across_layouts() {
        for n in [1, 7, 8, 17, 8192] {
            for scale in [1e-300, 1., 1e300] {
                // Powers of two give a separately evaluable normalized column
                // even when its unscaled magnitude is near floating-point limits.
                let roots: Vec<_> = (0..n).map(|r| [0.25, 0.5, 1., 2.][r % 4]).collect();
                let projected: Vec<_> = (0..n)
                    .map(|r| ((r * 7) as f64).cos() / (n as f64).sqrt())
                    .collect();
                let normalized: Vec<_> = (0..n)
                    .map(|r| ((r % 5) as f64 - 2.) / (4. * (n as f64).sqrt()))
                    .collect();
                let expected: f64 = normalized.iter().zip(&projected).map(|(a, b)| a * b).sum();
                for contiguous in [false, true] {
                    let controls = Column {
                        values: normalized
                            .iter()
                            .zip(&roots)
                            .map(|(x, w)| x * scale / w)
                            .collect(),
                        contiguous,
                    };
                    let actual = finite_product(&controls, 0, &roots, &projected, scale).unwrap();
                    assert!(
                        (actual - expected).abs() < 1e-13,
                        "n={n} scale={scale} contiguous={contiguous}: {actual} != {expected}"
                    );
                }
            }
        }
    }
}
