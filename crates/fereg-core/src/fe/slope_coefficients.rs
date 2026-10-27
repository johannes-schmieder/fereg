//! Current-weight coefficient coordinates for multipartition slope absorption.
//!
//! Local orthonormal bases are prepared once by the rank-aware geometry owner.
//! Eliminate the largest partition Q0 exactly and solve the Schur system
//! B' (I - Q0 Q0') B a = B' (I - Q0 Q0') y with conjugate gradients.
//! Null coefficient directions are harmless: only the fitted span is identified.
//! Iteration
//! residuals only schedule reconstruction: acceptance uses freshly reconstructed
//! observation residuals, followed by the caller's independent full certificate.
mod coupling;
pub(crate) mod joint;
use super::map::ReusableCertificateGeometry;
use super::{FeDesign, MapConfig};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use coupling::PreparedCoupling;
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
    eliminated_scratch: Vec<f64>,
    projection_max: f64,
    iterations: usize,
    checks: usize,
}

impl AsMut<[f64]> for Worker {
    fn as_mut(&mut self) -> &mut [f64] {
        &mut self.observations
    }
}

pub(super) struct SlopeCoefficientWorkspace {
    groups: Vec<Group>,
    workers: Vec<Worker>,
    columns: Vec<f64>,
    roots: Vec<f64>,
    coupling: Option<Box<PreparedCoupling>>,
    max_rhs: usize,
    coefficients: usize,
    pub(super) joint: Option<Box<joint::JointFiniteWorkspace>>,
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

fn observation_bank<R: crate::memory::VecReservation>(
    rows: usize,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let mut result = Vec::new();
    reservation
        .reserve_vec(
            &mut result,
            rows,
            "slope coefficient workspace exceeds admission",
        )
        .map_err(|_| allocation())?;
    if result.capacity() != rows {
        return Err(allocation());
    }
    crate::parallel::initialize_reserved(&mut result, rows, 0.0_f64);
    Ok(result)
}

pub(super) fn eliminated_effect(design: &FeDesign) -> usize {
    design
        .effects()
        .iter()
        .enumerate()
        .max_by_key(|(_, effect)| effect.n_levels().saturating_mul(effect.channels()))
        .map_or(0, |(i, _)| i)
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
    let eliminated = eliminated_effect(design);
    for (index, effect) in design.effects().iter().enumerate() {
        groups = groups
            .checked_add(effect.n_levels())
            .ok_or_else(allocation)?;
        if index != eliminated {
            coefficients = effect
                .n_levels()
                .checked_mul(effect.channels())
                .and_then(|n| coefficients.checked_add(n))
                .ok_or_else(allocation)?;
        }
    }
    let columns = design.nobs().checked_mul(rhs).ok_or_else(allocation)?;
    Ok((groups, coefficients, columns, threads.max(1).min(rhs)))
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
                + workers as u128
                    * (4 * coefficients as u128
                        + design.nobs() as u128
                        + design
                            .effects()
                            .get(eliminated_effect(design))
                            .map_or(0, |e| e.channels()) as u128)))
    }

    pub(super) fn new(design: &FeDesign, max_rhs: usize, threads: usize) -> Result<Self> {
        Self::new_with_observation_reservation(
            design,
            max_rhs,
            threads,
            &mut crate::memory::SystemVecReservation,
        )
    }

    fn new_with_observation_reservation<R: crate::memory::VecReservation>(
        design: &FeDesign,
        max_rhs: usize,
        threads: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        let started = crate::diagnostics::phase_timing_enabled().then(std::time::Instant::now);
        let (group_count, coefficients, column_count, workers) =
            dimensions(design, max_rhs, threads)?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(group_count)
            .map_err(|_| allocation())?;
        if groups.capacity() > group_count {
            return Err(allocation());
        }
        let eliminated = eliminated_effect(design);
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
                if effect_id != eliminated {
                    coefficient_start += effect.channels();
                }
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
                observations: observation_bank(design.nobs(), reservation)?,
                eliminated_scratch: filled(
                    design.effects().get(eliminated).map_or(0, |e| e.channels()),
                    0.,
                )?,
                projection_max: 0.,
                iterations: 0,
                checks: 0,
            });
        }
        let workspace = Self {
            groups,
            workers: workspaces,
            columns: filled(column_count, 0.)?,
            roots: filled(design.nobs(), 0.)?,
            coupling: None,
            max_rhs,
            coefficients,
            joint: None,
        };
        if let Some(started) = started {
            eprintln!(
                "FEREG_PHASE|coefficient_workspace_install|seconds={:.9}|rows={}|rhs={max_rhs}|workers={workers}|observation_bytes={}",
                started.elapsed().as_secs_f64(),
                design.nobs(),
                design.nobs() as u128 * workers as u128 * 8,
            );
        }
        Ok(workspace)
    }

    pub(super) fn elements(&self) -> usize {
        self.joint.as_ref().map_or(0, |j| j.elements())
            + self.columns.capacity()
            + self.roots.capacity()
            + self.coupling.as_ref().map_or(0, |c| c.elements())
            + self
                .workers
                .iter()
                .map(|w| {
                    w.coefficients.capacity()
                        + w.residual.capacity()
                        + w.direction.capacity()
                        + w.product.capacity()
                        + w.observations.capacity()
                        + w.eliminated_scratch.capacity()
                })
                .sum::<usize>()
    }

    pub(super) fn enable_joint(&mut self, design: &FeDesign, controls: usize) -> Result<()> {
        if controls > 0 {
            self.joint = Some(Box::new(joint::JointFiniteWorkspace::new(
                design,
                controls,
                self.coefficients,
            )?));
        }
        Ok(())
    }

    /// Geometry precedes column solving, so observation workspaces have no live
    /// numerical state here. Borrow them for QR scratch and write each group's
    /// basis directly into its disjoint part of the admitted geometry bank.
    pub(super) fn prepare_geometry(
        &mut self,
        design: &FeDesign,
        weights: &[f64],
        geometry: &mut ReusableCertificateGeometry,
    ) -> Result<bool> {
        let workers = self.workers.len().min(self.groups.len());
        if workers < 2 {
            return Ok(false);
        }
        super::map::prepare_geometry_with_scratch(
            design,
            weights,
            geometry,
            &mut self.workers[..workers],
        )
    }

    #[cfg(test)]
    pub(super) fn force_compact_coupling_for_test(&mut self) {
        self.coupling
            .as_mut()
            .expect("prepared coupling")
            .force_compact_for_test();
    }

    #[cfg(test)]
    pub(super) fn coupling_preparations(&self) -> Option<usize> {
        self.coupling.as_ref().map(|c| c.preparations)
    }

    #[cfg(test)]
    pub(super) fn check_coupling_products(
        &self,
        design: &FeDesign,
        geometry: &ReusableCertificateGeometry,
    ) {
        let coupling = self.coupling.as_ref().unwrap();
        let eliminated = design
            .effects()
            .iter()
            .enumerate()
            .max_by_key(|(_, e)| e.n_levels() * e.channels())
            .unwrap()
            .0;
        let operator = Operator {
            design,
            geometry,
            groups: &self.groups,
            coefficients: self.coefficients,
            eliminated,
            coupling: None,
        };
        let mut values = vec![0.; design.nobs()];
        let mut expected = vec![0.; self.coefficients];
        let mut actual = vec![0.; self.coefficients];
        let mut work = vec![0.; design.effects()[eliminated].channels()];
        let mut used = 0;
        for phase in 0..4 {
            let direction: Vec<_> = (0..self.coefficients)
                .map(|i| ((i * 19 + phase * 7) as f64).sin())
                .collect();
            operator.forward(&direction, &mut values);
            operator.project_eliminated(&mut values, &mut work);
            let curvature = dot(&values, &values);
            operator.transpose(&values, &mut expected, false);
            if let Some(candidate) = coupling.apply(&direction, &mut actual, &mut work) {
                used += 1;
                assert!((candidate - curvature).abs() < 1e-10 * curvature.max(1.));
                let scale = norm(&expected).max(1.);
                for (a, b) in actual.iter().zip(&expected) {
                    assert!((a - b).abs() < 1e-11 * scale, "prepared product {a} != {b}");
                }
            }
        }
        assert!(used > 0, "test must exercise a prepared numerical operator");
        let mut direction = vec![0.; self.coefficients];
        assert!(coupling.apply(&direction, &mut actual, &mut work).is_none());
        for invalid in [f64::NAN, f64::INFINITY] {
            direction[0] = invalid;
            assert!(coupling.apply(&direction, &mut actual, &mut work).is_none());
        }
        if design.effects().iter().all(|effect| effect.has_intercept()) {
            // Each remaining partition represents the weighted constant,
            // which the eliminated intercept must annihilate. Test this
            // independently in observation space before requiring fallback
            // from the cancellation-prone prepared product.
            let scale = norm(&self.roots);
            let constant: Vec<_> = self.roots.iter().map(|root| root / scale).collect();
            operator.transpose(&constant, &mut direction, false);
            for perturbation in [0., 1e-13] {
                let near_null: Vec<_> = direction
                    .iter()
                    .enumerate()
                    .map(|(i, value)| value + perturbation * ((i * 19) as f64).sin())
                    .collect();
                operator.forward(&near_null, &mut values);
                operator.project_eliminated(&mut values, &mut work);
                assert!(dot(&values, &values) <= 1e-20 * dot(&near_null, &near_null));
                assert!(coupling.apply(&near_null, &mut actual, &mut work).is_none());
            }
        }
        let extra = Self::coupling_bytes(design, self.max_rhs).unwrap().unwrap();
        assert!(extra >= coupling.allocated_bytes());
    }

    pub(super) fn coupling_bytes(design: &FeDesign, rhs: usize) -> Result<Option<u128>> {
        PreparedCoupling::bytes(design, rhs)
    }

    pub(super) fn enable_coupling(&mut self, design: &FeDesign) -> Result<()> {
        if let Some(coupling) = PreparedCoupling::new(design, &self.groups, self.max_rhs)? {
            self.coupling = Some(Box::new(coupling));
        }
        Ok(())
    }

    // Keep the legacy workspace and ownership boundaries explicit during migration.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn solve(
        &mut self,
        design: &FeDesign,
        geometry: &ReusableCertificateGeometry,
        weights: &[f64],
        values: &mut DenseMatrix,
        norms: &[f64],
        config: MapConfig,
        reusable_geometry: bool,
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
        // Row-major reads with one sequential write stream per column.
        for (row, entries) in values.as_slice().chunks_exact(rhs).enumerate() {
            let root = self.roots[row];
            for (column, (&value, &norm)) in entries.iter().zip(norms).enumerate() {
                self.columns[column * n + row] = if norm == 0. {
                    0.
                } else {
                    (value * root) / norm
                };
            }
        }
        let active_workers = rhs.min(self.workers.len());
        let width = rhs / active_workers;
        let longer_blocks = rhs % active_workers;
        let split = n * (width + 1) * longer_blocks;
        let eliminated = eliminated_effect(design);
        if !reusable_geometry {
            if let Some(coupling) = self.coupling.as_mut() {
                coupling.prepare(design, geometry, &self.groups);
            }
        }
        let operator = Operator {
            coupling: self.coupling.as_deref(),
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
        if rhs == 1 && self.workers.len() > 1 {
            // A response-only call leaves the other admitted column buffers
            // idle. They can share compact operator products, just as in the
            // joint finite/FE solve, without another workspace or recurrence.
            let (worker, helpers) = self.workers.split_first_mut().expect("admitted worker");
            worker.solve::<true>(&operator, &mut self.columns[..n], config, helpers)?;
        } else {
            let (longer, shorter) = self.columns[..n * rhs].split_at_mut(split);
            // Unequal contiguous blocks keep every worker busy without allocating
            // a task list or changing any column's arithmetic or stopping rule.
            longer
                .par_chunks_mut(n * (width + 1))
                .chain(shorter.par_chunks_mut(n * width))
                .zip(self.workers[..active_workers].par_iter_mut())
                .try_for_each(|(block, worker)| -> Result<()> {
                    for column in block.chunks_mut(n) {
                        worker.solve::<false>(&operator, column, config, &mut [])?;
                    }
                    Ok(())
                })?;
        }
        for (row, entries) in values.as_mut_slice().chunks_exact_mut(rhs).enumerate() {
            let root = self.roots[row];
            for (column, (entry, &norm)) in entries.iter_mut().zip(norms).enumerate() {
                let value = (self.columns[column * n + row] * norm) / root;
                if !value.is_finite() {
                    return Err(invalid());
                }
                *entry = value;
            }
        }
        Ok(self.workers.iter().map(|w| w.iterations).max().unwrap_or(0))
    }
}

struct Operator<'a> {
    coupling: Option<&'a PreparedCoupling>,
    design: &'a FeDesign,
    geometry: &'a ReusableCertificateGeometry,
    groups: &'a [Group],
    coefficients: usize,
    eliminated: usize,
}
impl Operator<'_> {
    fn forward(&self, coefficients: &[f64], output: &mut [f64]) {
        output.fill(0.);
        let hardware = crate::kernels::installed();
        for (index, group) in self.groups.iter().enumerate() {
            if group.effect == self.eliminated {
                continue;
            }
            let rows = self.design.effects()[group.effect]
                .level_index()
                .members(group.level);
            let rank = self.geometry.ranks[index];
            crate::kernels::dispatch!(
                hardware,
                basis_forward(
                    rows,
                    &self.geometry.basis[group.basis_start..],
                    &coefficients[group.coefficient_start..group.coefficient_start + rank],
                    output
                )
            );
        }
    }
    fn transpose(&self, values: &[f64], output: &mut [f64], all: bool) -> f64 {
        output.fill(0.);
        let mut maximum = 0.0_f64;
        let hardware = crate::kernels::installed();
        for (index, group) in self.groups.iter().enumerate() {
            if !all && group.effect == self.eliminated {
                continue;
            }
            let rows = self.design.effects()[group.effect]
                .level_index()
                .members(group.level);
            let rank = self.geometry.ranks[index];
            let destination = (group.effect != self.eliminated)
                .then(|| &mut output[group.coefficient_start..group.coefficient_start + rank]);
            let square = crate::kernels::dispatch!(
                hardware,
                basis_transpose(
                    rows,
                    &self.geometry.basis[group.basis_start..],
                    rank,
                    values,
                    destination,
                    all
                )
            );
            if all {
                // Nonfinite eliminated moments must fail even though they no
                // longer occupy the remaining-coordinate residual vector.
                maximum = maximum.max(if square.is_finite() {
                    square.sqrt()
                } else {
                    f64::INFINITY
                });
            }
        }
        maximum
    }
    fn project_eliminated(&self, values: &mut [f64], scratch: &mut [f64]) {
        let hardware = crate::kernels::installed();
        for (index, group) in self.groups.iter().enumerate() {
            if group.effect != self.eliminated {
                continue;
            }
            let rows = self.design.effects()[group.effect]
                .level_index()
                .members(group.level);
            crate::kernels::dispatch!(
                hardware,
                basis_project(
                    rows,
                    &self.geometry.basis[group.basis_start..],
                    values,
                    &mut scratch[..self.geometry.ranks[index]]
                )
            );
        }
    }
}
fn dot(left: &[f64], right: &[f64]) -> f64 {
    crate::kernels::dispatch!(crate::kernels::installed(), ordered_dot(left, right))
}
#[cfg(test)]
fn norm(values: &[f64]) -> f64 {
    dot(values, values).sqrt()
}

impl Worker {
    fn reconstruct(&mut self, op: &Operator<'_>, original: &[f64]) -> bool {
        op.forward(&self.coefficients, &mut self.observations);
        for (residual, &original) in self.observations.iter_mut().zip(original) {
            *residual = original - *residual;
        }
        op.project_eliminated(&mut self.observations, &mut self.eliminated_scratch);
        self.projection_max = op.transpose(&self.observations, &mut self.residual, true);
        self.checks += 1;
        self.observations.iter().all(|x| x.is_finite())
            && self.residual.iter().all(|x| x.is_finite())
            && self.projection_max.is_finite()
    }
    fn solve<const SHARE_PRODUCT: bool>(
        &mut self,
        op: &Operator<'_>,
        original: &mut [f64],
        config: MapConfig,
        helpers: &mut [Worker],
    ) -> Result<()> {
        self.coefficients.fill(0.);
        self.observations.copy_from_slice(original);
        op.project_eliminated(&mut self.observations, &mut self.eliminated_scratch);
        op.transpose(&self.observations, &mut self.residual, false);
        self.direction.copy_from_slice(&self.residual);
        let mut square = dot(&self.residual, &self.residual);
        for iteration in 0..=config.max_iterations {
            if square.sqrt() <= config.tolerance * 0.25 || iteration == config.max_iterations {
                if !self.reconstruct(op, original) {
                    return Err(invalid());
                }
                if self.projection_max <= config.tolerance * 0.5 {
                    original.copy_from_slice(&self.observations);
                    self.iterations = self.iterations.max(iteration);
                    return Ok(());
                }
                // A recurrence diagnostic never certifies. Replace drifted
                // moments with the fresh residual and restart the direction.
                self.direction.copy_from_slice(&self.residual);
                square = dot(&self.residual, &self.residual);
            }
            if iteration == config.max_iterations {
                break;
            }
            let curvature = match op.coupling.and_then(|c| {
                // Specialize at dispatch: routing every column through the
                // helper-capable product penalizes the ordinary compact kernel
                // even when no worker is available to share its work.
                if SHARE_PRODUCT {
                    c.apply_with_helpers(
                        &self.direction,
                        &mut self.product,
                        &mut self.eliminated_scratch,
                        helpers,
                        None,
                    )
                } else {
                    c.apply(
                        &self.direction,
                        &mut self.product,
                        &mut self.eliminated_scratch,
                    )
                }
            }) {
                Some(curvature) => curvature,
                None => {
                    op.forward(&self.direction, &mut self.observations);
                    op.project_eliminated(&mut self.observations, &mut self.eliminated_scratch);
                    let curvature = dot(&self.observations, &self.observations);
                    op.transpose(&self.observations, &mut self.product, false);
                    curvature
                }
            };
            if !curvature.is_finite() || curvature <= 0. || !square.is_finite() {
                return Err(invalid());
            }
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

#[cfg(test)]
mod initialization_tests {
    use super::*;
    use crate::memory::VecReservation;

    #[derive(Clone, Copy)]
    enum Fault {
        Refuse,
        Undersize,
        Oversize,
    }

    #[derive(Default)]
    struct Reservation {
        records: Vec<(usize, usize, usize, std::thread::ThreadId)>,
        fault: Option<(usize, Fault)>,
    }

    impl VecReservation for Reservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            let call = self.records.len();
            self.records
                .push((additional, 0, 0, std::thread::current().id()));
            let fault = self.fault.filter(|(index, _)| *index == call);
            match fault.map(|(_, fault)| fault) {
                Some(Fault::Refuse) => return Err(FeregError::new(ErrorCode::Allocation, label)),
                Some(Fault::Undersize) => return Ok(()),
                Some(Fault::Oversize) => target.try_reserve_exact(additional + 1),
                None => target.try_reserve_exact(additional),
            }
            .map_err(|_| allocation())?;
            self.records[call].1 = target.as_ptr() as usize;
            self.records[call].2 = target.capacity();
            Ok(())
        }
    }

    #[test]
    fn observation_first_touch_preserves_admission_storage_and_reservation_sequence() {
        let rows = 65_539;
        let effects = [17, 13]
            .into_iter()
            .map(|levels| {
                super::super::EffectTerm::new(
                    (0..rows).map(|row| (row % levels) as u32).collect(),
                    levels,
                    true,
                    Vec::new(),
                )
                .unwrap()
            })
            .collect();
        let design = FeDesign::new(rows, effects).unwrap();
        for threads in [1, 4, 16] {
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(threads, true).unwrap())
                .install(|| {
                    let coordinator = std::thread::current().id();
                    for rhs in [1, 3] {
                        let expected_workers = threads.min(rhs);
                        let mut reservation = Reservation::default();
                        let workspace =
                            SlopeCoefficientWorkspace::new_with_observation_reservation(
                                &design,
                                rhs,
                                threads,
                                &mut reservation,
                            )
                            .unwrap();
                        assert_eq!(workspace.workers.len(), expected_workers);
                        assert_eq!(workspace.groups.len(), 30);
                        assert_eq!(workspace.coefficients, 13);
                        assert_eq!(workspace.columns.len(), rows * rhs);
                        assert_eq!(workspace.roots.len(), rows);
                        assert_eq!(reservation.records.len(), expected_workers);
                        for (worker, &(count, pointer, capacity, thread)) in
                            workspace.workers.iter().zip(&reservation.records)
                        {
                            assert_eq!(count, rows);
                            assert_eq!(thread, coordinator);
                            assert_eq!(worker.observations.as_ptr() as usize, pointer);
                            assert_eq!(worker.observations.capacity(), capacity);
                            assert_eq!(worker.observations.len(), rows);
                            assert_eq!(capacity, rows);
                            assert!(worker.observations.iter().all(|value| value.to_bits() == 0));
                            assert_eq!(worker.coefficients.len(), 13);
                            assert_eq!(worker.eliminated_scratch.len(), 1);
                        }
                        drop(workspace);
                        for failed_call in 0..expected_workers {
                            for fault in [Fault::Refuse, Fault::Undersize, Fault::Oversize] {
                                let mut reservation = Reservation {
                                    fault: Some((failed_call, fault)),
                                    ..Reservation::default()
                                };
                                let error =
                                    SlopeCoefficientWorkspace::new_with_observation_reservation(
                                        &design,
                                        rhs,
                                        threads,
                                        &mut reservation,
                                    )
                                    .err()
                                    .expect("observation reservation must fail closed");
                                assert_eq!(error.code, ErrorCode::Allocation);
                                assert_eq!(error.message, allocation().message);
                                assert_eq!(reservation.records.len(), failed_call + 1);
                                assert!(reservation.records.iter().all(
                                    |&(count, _, _, thread)| {
                                        count == rows && thread == coordinator
                                    }
                                ));
                            }
                        }
                    }
                })
                .unwrap();
        }
    }
}
