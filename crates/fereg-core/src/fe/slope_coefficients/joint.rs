//! Joint ordinary WLS directions over the FE Schur and finite coordinates.
//! The canonical session owns rank and final bread. This owner retains capacity,
//! never finite products or acceptance across a changing response/weight frame.
use super::*;
use crate::fe::SolveReport;

/// A retained finite representative, in memory or on private row-major scratch.
/// Each scan owns its reader; parallel columns never share a mutable file cursor.
pub(crate) trait JointFiniteDesign: Sync {
    fn nrows(&self) -> usize;
    fn ncols(&self) -> usize;
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
        controls.visit_rows(&mut self.column_scratch, |row, values| {
            let mut residual = original[row] - observations[row];
            for (j, &column) in selected.iter().enumerate() {
                residual = (-self.coefficients[p + j])
                    .mul_add(values[column] * roots[row] / self.scales[j], residual);
            }
            observations[row] = residual;
            Ok(())
        })?;
        op.project_eliminated(observations, scratch);
        let mut maximum = op.transpose(observations, &mut self.residual[..p], true);
        self.residual[p..p + selected.len()].fill(0.);
        controls.visit_rows(&mut self.column_scratch, |row, values| {
            for (j, &column) in selected.iter().enumerate() {
                self.residual[p + j] = (values[column] * roots[row] / self.scales[j])
                    .mul_add(observations[row], self.residual[p + j]);
            }
            Ok(())
        })?;
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
        let worker = &mut owner.workers[0];
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
                for j in 0..k {
                    row[j] = dot(&z[i * n..(i + 1) * n], &z[j * n..(j + 1) * n]);
                }
            });
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
        config: MapConfig,
    ) -> Result<usize> {
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
                    &op,
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
                .and_then(|c| c.apply(&self.direction[..p], &mut self.product[..p], scratch))
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
                for i in 0..p {
                    self.product[i] = cross[i].mul_add(self.direction[p + j], self.product[i]);
                }
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
            for i in 0..dimension {
                self.coefficients[i] = step.mul_add(self.direction[i], self.coefficients[i]);
                self.residual[i] = (-step).mul_add(self.product[i], self.residual[i]);
            }
            self.precondition(p, k);
            let next = dot(
                &self.residual[..dimension],
                &self.preconditioned[..dimension],
            );
            let ratio = next / square;
            for i in 0..dimension {
                self.direction[i] = ratio.mul_add(self.direction[i], self.preconditioned[i]);
            }
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
        // Norm state and row scratch occupy existing banks before their solve
        // lifetimes. Visit each scratch row once for all current-weight scales.
        self.scales[..k].fill(0.);
        self.beta[..k].fill(1.);
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
        for j in 0..k {
            self.scales[j] *= self.beta[j].sqrt();
            if !self.scales[j].is_finite() || self.scales[j] <= 0. {
                return Err(failed());
            }
        }
        // A tile uses only the admitted per-worker observation buffers. Its
        // two complete-row scans load the columns and form their products;
        // no normalized N-by-K bank survives or is allocated for low mode.
        let width = owner.workers.len().min(k);
        self.hessian[..k * k].fill(0.);
        for first in (0..k).step_by(width) {
            let last = (first + width).min(k);
            let workers = &mut owner.workers[..last - first];
            controls.visit_rows(&mut self.factor[..controls.ncols()], |row, values| {
                for (local, worker) in workers.iter_mut().enumerate() {
                    let j = first + local;
                    worker.observations[row] = values[selected[j]] * roots[row] / self.scales[j];
                }
                Ok(())
            })?;
            workers
                .par_iter_mut()
                .zip(self.cross[first * p..last * p].par_chunks_mut(p))
                .zip(self.residual[p + first..p + last].par_iter_mut())
                .for_each(|((worker, cross), rhs)| {
                    let column = &mut worker.observations;
                    op.project_eliminated(column, &mut worker.eliminated_scratch);
                    op.transpose(column, cross, false);
                    *rhs = dot(column, original);
                });
            controls.visit_rows(&mut self.factor[..controls.ncols()], |row, values| {
                // All rows in this Hessian tile use the same normalized raw
                // finite row. The norm recurrence has finished with beta;
                // reuse that bank until iterate installs the solved beta.
                for (value, (&column, &scale)) in self.beta[..k]
                    .iter_mut()
                    .zip(selected.iter().zip(&self.scales[..k]))
                {
                    *value = values[column] * roots[row] / scale;
                }
                for (local, worker) in workers.iter().enumerate() {
                    let i = first + local;
                    let hessian_row = &mut self.hessian[i * k..(i + 1) * k];
                    for (entry, &value) in hessian_row.iter_mut().zip(&self.beta[..k]) {
                        *entry = value.mul_add(worker.observations[row], *entry);
                    }
                }
                Ok(())
            })?;
        }
        let worker = &mut owner.workers[0];
        let observations = &mut worker.observations;
        let scratch = &mut worker.eliminated_scratch;
        for i in 0..k {
            for j in 0..i {
                let mean = 0.5 * self.hessian[i * k + j] + 0.5 * self.hessian[j * k + i];
                self.hessian[i * k + j] = mean;
                self.hessian[j * k + i] = mean;
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
            config,
        )
    }
}
