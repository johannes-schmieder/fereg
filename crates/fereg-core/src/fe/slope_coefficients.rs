//! Current-weight coefficient coordinates for multipartition slope absorption.
//!
//! Local orthonormal bases are prepared once by the rank-aware geometry owner.
//! Eliminate the largest partition Q0 exactly and solve the Schur system
//! B' (I - Q0 Q0') B a = B' (I - Q0 Q0') y with conjugate gradients.
//! Null coefficient directions are harmless: only the fitted span is identified.
//! Iteration
//! residuals only schedule reconstruction: acceptance uses freshly reconstructed
//! observation residuals, followed by the caller's independent full certificate.
use super::map::ReusableCertificateGeometry;
use super::{FeCertificate, FeDesign, MapConfig};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use rayon::prelude::*;

#[derive(Clone, Copy)]
struct Group {
    effect: usize,
    level: usize,
    basis_start: usize,
    coefficient_start: usize,
    channels: usize,
}

struct Worker {
    coefficients: Vec<f64>,
    residual: Vec<f64>,
    direction: Vec<f64>,
    product: Vec<f64>,
    observations: Vec<f64>,
    iterations: usize,
    checks: usize,
}

pub(super) struct SlopeCoefficientWorkspace {
    groups: Vec<Group>,
    workers: Vec<Worker>,
    columns: Vec<f64>,
    roots: Vec<f64>,
    certificate_groups: Vec<f64>,
    max_rhs: usize,
    coefficients: usize,
}

fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "slope coefficient workspace exceeds admission",
    )
}
fn invalid() -> FeregError {
    FeregError::new(
        ErrorCode::InvalidData,
        "slope coefficient solve failed current-weight certification",
    )
}
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut result = Vec::new();
    result.try_reserve_exact(n).map_err(|_| allocation())?;
    if result.capacity() > n {
        return Err(allocation());
    }
    result.resize(n, value);
    Ok(result)
}
fn dimensions(
    design: &FeDesign,
    rhs: usize,
    threads: usize,
) -> Result<(usize, usize, usize, usize)> {
    if rhs == 0 {
        return Err(allocation());
    }
    let mut groups = 0_usize;
    let mut coefficients = 0_usize;
    for effect in design.effects() {
        groups = groups
            .checked_add(effect.n_levels())
            .ok_or_else(allocation)?;
        coefficients = effect
            .n_levels()
            .checked_mul(effect.channels())
            .and_then(|n| coefficients.checked_add(n))
            .ok_or_else(allocation)?;
    }
    let columns = design.nobs().checked_mul(rhs).ok_or_else(allocation)?;
    Ok((groups, coefficients, columns, threads.max(1).min(rhs)))
}

fn certificate_elements(groups: usize, rhs: usize) -> Result<usize> {
    rhs.checked_add(1)
        .and_then(|width| groups.checked_mul(width))
        .ok_or_else(allocation)
}

impl SlopeCoefficientWorkspace {
    pub(super) fn bytes(design: &FeDesign, rhs: usize, threads: usize) -> Result<u128> {
        let (groups, coefficients, columns, workers) = dimensions(design, rhs, threads)?;
        Ok(std::mem::size_of::<Self>() as u128
            + std::mem::size_of::<usize>() as u128
            + groups as u128 * std::mem::size_of::<Group>() as u128
            + workers as u128 * std::mem::size_of::<Worker>() as u128
            + 8 * (columns as u128
                + design.nobs() as u128
                + workers as u128 * (4 * coefficients as u128 + design.nobs() as u128)))
    }

    pub(super) fn new(design: &FeDesign, max_rhs: usize, threads: usize) -> Result<Self> {
        let (group_count, coefficients, column_count, workers) =
            dimensions(design, max_rhs, threads)?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(group_count)
            .map_err(|_| allocation())?;
        if groups.capacity() > group_count {
            return Err(allocation());
        }
        let mut basis_start = 0;
        let mut coefficient_start = 0;
        for (effect_id, effect) in design.effects().iter().enumerate() {
            for level in 0..effect.n_levels() {
                groups.push(Group {
                    effect: effect_id,
                    level,
                    basis_start,
                    coefficient_start,
                    channels: effect.channels(),
                });
                basis_start += effect.channels() * effect.level_index().members(level).len();
                coefficient_start += effect.channels();
            }
        }
        let mut workspaces = Vec::new();
        workspaces
            .try_reserve_exact(workers)
            .map_err(|_| allocation())?;
        if workspaces.capacity() > workers {
            return Err(allocation());
        }
        for _ in 0..workers {
            workspaces.push(Worker {
                coefficients: filled(coefficients, 0.)?,
                residual: filled(coefficients, 0.)?,
                direction: filled(coefficients, 0.)?,
                product: filled(coefficients, 0.)?,
                observations: filled(design.nobs(), 0.)?,
                iterations: 0,
                checks: 0,
            });
        }
        Ok(Self {
            groups,
            workers: workspaces,
            columns: filled(column_count, 0.)?,
            roots: filled(design.nobs(), 0.)?,
            certificate_groups: Vec::new(),
            max_rhs,
            coefficients,
        })
    }

    pub(super) fn elements(&self) -> usize {
        self.columns.capacity()
            + self.roots.capacity()
            + self.certificate_groups.capacity()
            + self
                .workers
                .iter()
                .map(|w| {
                    w.coefficients.capacity()
                        + w.residual.capacity()
                        + w.direction.capacity()
                        + w.product.capacity()
                        + w.observations.capacity()
                })
                .sum::<usize>()
    }

    pub(super) fn certificate_bytes(design: &FeDesign, rhs: usize) -> Result<u128> {
        let (groups, _, _, _) = dimensions(design, rhs, 1)?;
        Ok(certificate_elements(groups, rhs)? as u128 * 8)
    }

    pub(super) fn enable_certificate_groups(&mut self) -> Result<()> {
        self.certificate_groups =
            filled(certificate_elements(self.groups.len(), self.max_rhs)?, 0.)?;
        Ok(())
    }

    pub(super) fn has_certificate_groups(&self) -> bool {
        !self.certificate_groups.is_empty()
    }

    /// Independent fresh residual checks share row loads within each group.
    /// Disjoint results are merged in the original group/RHS order so worker
    /// scheduling cannot change the certificate's RMS reduction.
    pub(super) fn certify(
        &mut self,
        design: &FeDesign,
        geometry: &ReusableCertificateGeometry,
        weights: &[f64],
        residuals: &DenseMatrix,
        scales: &[f64],
        tolerance: f64,
    ) -> FeCertificate {
        let stride = self.max_rhs + 1;
        let rhs = residuals.ncols();
        self.certificate_groups
            .par_chunks_mut(stride)
            .zip(self.groups.par_iter())
            .enumerate()
            .for_each(|(index, (result, group))| {
                let effect = &design.effects()[group.effect];
                let members = effect.level_index().members(group.level);
                let rank = geometry.ranks[index];
                let basis =
                    &geometry.basis[group.basis_start..group.basis_start + members.len() * rank];
                super::map::certify_reusable_group(
                    effect,
                    members,
                    weights,
                    residuals,
                    scales,
                    basis,
                    rank,
                    &mut result[..rhs + 1],
                );
            });
        let mut absolute = 0.0_f64;
        let mut maximum = 0.0_f64;
        let mut square = 0.0_f64;
        for result in self.certificate_groups.chunks(stride) {
            absolute = absolute.max(result[0]);
            for &relative in &result[1..rhs + 1] {
                maximum = maximum.max(relative);
                square = relative.mul_add(relative, square);
            }
        }
        let count = self.groups.len() * rhs;
        FeCertificate {
            absolute_infinity_norm: absolute,
            relative_infinity_norm: maximum,
            relative_two_norm: if count == 0 {
                0.0
            } else {
                (square / count as f64).sqrt()
            },
            passed: maximum <= tolerance,
        }
    }

    pub(super) fn solve(
        &mut self,
        design: &FeDesign,
        geometry: &ReusableCertificateGeometry,
        weights: &[f64],
        values: &mut DenseMatrix,
        norms: &[f64],
        config: MapConfig,
    ) -> Result<usize> {
        let n = design.nobs();
        let rhs = values.ncols();
        if rhs == 0
            || rhs > self.max_rhs
            || norms.len() < rhs
            || geometry.ranks.len() != self.groups.len()
            || n == 0
            || weights.len() != n
            || values.nrows() != n
        {
            return Err(invalid());
        }
        for (root, weight) in self.roots.iter_mut().zip(weights) {
            *root = weight.sqrt();
        }
        for column in 0..rhs {
            for row in 0..n {
                self.columns[column * n + row] = if norms[column] == 0. {
                    0.
                } else {
                    (values[(row, column)] * self.roots[row]) / norms[column]
                };
            }
        }
        let active_workers = rhs.min(self.workers.len());
        let width = rhs / active_workers;
        let longer_blocks = rhs % active_workers;
        let split = n * (width + 1) * longer_blocks;
        let eliminated = design
            .effects()
            .iter()
            .enumerate()
            .max_by_key(|(_, effect)| effect.n_levels() * effect.channels())
            .map_or(0, |(i, _)| i);
        let operator = Operator {
            design,
            geometry,
            groups: &self.groups,
            coefficients: self.coefficients,
            eliminated,
        };
        for worker in &mut self.workers {
            worker.iterations = 0;
            worker.checks = 0;
        }
        let (longer, shorter) = self.columns[..n * rhs].split_at_mut(split);
        // Unequal contiguous blocks keep every worker busy without allocating
        // a task list or changing any column's arithmetic or stopping rule.
        longer
            .par_chunks_mut(n * (width + 1))
            .chain(shorter.par_chunks_mut(n * width))
            .zip(self.workers[..active_workers].par_iter_mut())
            .try_for_each(|(block, worker)| -> Result<()> {
                for column in block.chunks_mut(n) {
                    worker.solve(&operator, column, config)?;
                }
                Ok(())
            })?;
        for column in 0..rhs {
            for row in 0..n {
                let value = (self.columns[column * n + row] * norms[column]) / self.roots[row];
                if !value.is_finite() {
                    return Err(invalid());
                }
                values[(row, column)] = value;
            }
        }
        Ok(self.workers.iter().map(|w| w.iterations).max().unwrap_or(0))
    }
}

struct Operator<'a> {
    design: &'a FeDesign,
    geometry: &'a ReusableCertificateGeometry,
    groups: &'a [Group],
    coefficients: usize,
    eliminated: usize,
}
impl Operator<'_> {
    fn forward(&self, coefficients: &[f64], output: &mut [f64]) {
        output.fill(0.);
        for (index, group) in self.groups.iter().enumerate() {
            if group.effect == self.eliminated {
                continue;
            }
            let rows = self.design.effects()[group.effect]
                .level_index()
                .members(group.level);
            let rank = self.geometry.ranks[index];
            for (local, &row) in rows.iter().enumerate() {
                let mut value = 0.;
                for basis in 0..rank {
                    value = self.geometry.basis[group.basis_start + basis * rows.len() + local]
                        .mul_add(coefficients[group.coefficient_start + basis], value);
                }
                output[row] += value;
            }
        }
    }
    fn transpose(&self, values: &[f64], output: &mut [f64], all: bool) {
        output.fill(0.);
        for (index, group) in self.groups.iter().enumerate() {
            if !all && group.effect == self.eliminated {
                continue;
            }
            let rows = self.design.effects()[group.effect]
                .level_index()
                .members(group.level);
            for basis in 0..self.geometry.ranks[index] {
                let q =
                    &self.geometry.basis[group.basis_start + basis * rows.len()..][..rows.len()];
                let mut value = 0.;
                for (&row, &q) in rows.iter().zip(q) {
                    value = q.mul_add(values[row], value);
                }
                output[group.coefficient_start + basis] = value;
            }
        }
    }
    fn project_eliminated(&self, values: &mut [f64], scratch: &mut [f64]) {
        for (index, group) in self.groups.iter().enumerate() {
            if group.effect != self.eliminated {
                continue;
            }
            let rows = self.design.effects()[group.effect]
                .level_index()
                .members(group.level);
            for basis in 0..self.geometry.ranks[index] {
                let q =
                    &self.geometry.basis[group.basis_start + basis * rows.len()..][..rows.len()];
                let mut value = 0.;
                for (&row, &q) in rows.iter().zip(q) {
                    value = q.mul_add(values[row], value);
                }
                scratch[group.coefficient_start + basis] = value;
            }
            for (local, &row) in rows.iter().enumerate() {
                let mut value = 0.;
                for basis in 0..self.geometry.ranks[index] {
                    value = self.geometry.basis[group.basis_start + basis * rows.len() + local]
                        .mul_add(scratch[group.coefficient_start + basis], value);
                }
                values[row] -= value;
            }
        }
    }
    fn zero_eliminated(&self, values: &mut [f64]) {
        for group in self.groups {
            if group.effect == self.eliminated {
                values[group.coefficient_start..group.coefficient_start + group.channels].fill(0.);
            }
        }
    }
    fn local_max(&self, coefficients: &[f64]) -> f64 {
        self.groups
            .iter()
            .map(|g| norm(&coefficients[g.coefficient_start..g.coefficient_start + g.channels]))
            .fold(0., f64::max)
    }
}
fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .fold(0., |sum, (&a, &b)| a.mul_add(b, sum))
}
fn norm(values: &[f64]) -> f64 {
    dot(values, values).sqrt()
}

impl Worker {
    fn reconstruct(&mut self, op: &Operator<'_>, original: &[f64]) -> bool {
        op.forward(&self.coefficients, &mut self.observations);
        for (residual, &original) in self.observations.iter_mut().zip(original) {
            *residual = original - *residual;
        }
        op.project_eliminated(&mut self.observations, &mut self.product);
        op.transpose(&self.observations, &mut self.residual, true);
        self.checks += 1;
        self.observations.iter().all(|x| x.is_finite())
            && self.residual.iter().all(|x| x.is_finite())
    }
    fn solve(&mut self, op: &Operator<'_>, original: &mut [f64], config: MapConfig) -> Result<()> {
        self.coefficients.fill(0.);
        self.observations.copy_from_slice(original);
        op.project_eliminated(&mut self.observations, &mut self.product);
        op.transpose(&self.observations, &mut self.residual, false);
        self.direction.copy_from_slice(&self.residual);
        let mut square = dot(&self.residual, &self.residual);
        for iteration in 0..=config.max_iterations {
            if square.sqrt() <= config.tolerance * 0.25 || iteration == config.max_iterations {
                if !self.reconstruct(op, original) {
                    return Err(invalid());
                }
                if op.local_max(&self.residual) <= config.tolerance * 0.5 {
                    original.copy_from_slice(&self.observations);
                    self.iterations = self.iterations.max(iteration);
                    return Ok(());
                }
                // A recurrence diagnostic never certifies. Replace drifted
                // moments with the fresh residual and restart the direction.
                op.zero_eliminated(&mut self.residual);
                self.direction.copy_from_slice(&self.residual);
                square = dot(&self.residual, &self.residual);
            }
            if iteration == config.max_iterations {
                break;
            }
            op.forward(&self.direction, &mut self.observations);
            op.project_eliminated(&mut self.observations, &mut self.product);
            let curvature = dot(&self.observations, &self.observations);
            if !curvature.is_finite() || curvature <= 0. || !square.is_finite() {
                return Err(invalid());
            }
            op.transpose(&self.observations, &mut self.product, false);
            let step = square / curvature;
            for j in 0..op.coefficients {
                self.coefficients[j] = step.mul_add(self.direction[j], self.coefficients[j]);
                self.residual[j] = (-step).mul_add(self.product[j], self.residual[j]);
            }
            let next = dot(&self.residual, &self.residual);
            let ratio = next / square;
            for j in 0..op.coefficients {
                self.direction[j] = ratio.mul_add(self.direction[j], self.residual[j]);
            }
            square = next;
        }
        Err(invalid())
    }
}
