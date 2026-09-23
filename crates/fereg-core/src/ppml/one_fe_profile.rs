//! Concentrated Poisson steps after the canonical one-intercept-FE rank decision.
//! Group effects are optimized analytically at every beta trial; the PPML owner
//! still supplies independent outer and canonical strict/final certificates.
use super::family::{means_from_eta, poisson_objective_from_eta, CompensatedSum};
use super::{PoissonObjective, PpmlConfig, PpmlIterationWorkspace, PpmlLineSearchAcceptance};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::linalg::solve_linear_system;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use crate::prepared_fit_view::PreparedOlsFitView;
use rayon::prelude::*;

const GROUPS_PER_BLOCK: usize = 64;

struct Moments {
    mean: Vec<f64>,
    centered: Vec<f64>,
    gradient: Vec<f64>,
    hessian: Vec<f64>,
}

pub(super) struct ProfiledOneFe {
    team: InstalledThreadTeam,
    columns: usize,
    data: Vec<f64>,
    active: Vec<usize>,
    slots: Vec<usize>,
    log_weights: Vec<f64>,
    log_outcomes: Vec<f64>,
    shifts: Vec<f64>,
    eta: Vec<f64>,
    mean: Vec<f64>,
    base: Vec<f64>,
    direction: Vec<f64>,
    scales: Vec<f64>,
    gradient: Vec<f64>,
    hessian: Vec<f64>,
    factor: Vec<f64>,
    partials: Vec<Moments>,
}

fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "one-FE profile workspace exceeds admission",
    )
}
fn numerical() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "one-FE profiled Newton step did not certify",
    )
}
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| allocation())?;
    if v.capacity() > n {
        return Err(allocation());
    }
    v.resize(n, value);
    Ok(v)
}

impl ProfiledOneFe {
    pub(super) fn bytes(n: usize, k: usize, g: usize) -> Result<u128> {
        let square = k.checked_mul(k).ok_or_else(allocation)?;
        let data = n.checked_mul(k).ok_or_else(allocation)?;
        let blocks = g.checked_add(GROUPS_PER_BLOCK - 1).ok_or_else(allocation)? / GROUPS_PER_BLOCK;
        Ok(std::mem::size_of::<Self>() as u128
            + 2 * std::mem::size_of::<usize>() as u128 * k as u128
            + blocks as u128 * std::mem::size_of::<Moments>() as u128
            + 8 * (data as u128
                + 3 * n as u128
                + 2 * g as u128
                + 4 * k as u128
                + 2 * square as u128
                + blocks as u128 * (3 * k as u128 + square as u128)))
    }

    pub(super) fn new<P: PreparedOlsFitView + ?Sized>(
        prepared: &P,
        active: &[usize],
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        threads: ThreadConfig,
    ) -> Result<Self> {
        if design.effects().len() != 1
            || !design.effects()[0].has_intercept()
            || !design.effects()[0].slopes().is_empty()
            || y.len() != design.nobs()
            || q.len() != y.len()
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "one-FE profile design is inconsistent",
            ));
        }
        let n = y.len();
        let k = active.len();
        let g = design.effects()[0].n_levels();
        let mut result = Self::reserve(n, k, g, threads)?;
        result.active.copy_from_slice(active);
        result.team.install(|| {
            result
                .data
                .par_chunks_mut(k.max(1))
                .enumerate()
                .try_for_each(|(row, values)| -> Result<()> {
                    for (j, &column) in active.iter().enumerate() {
                        values[j] = prepared.fit_column_value(column, row)?;
                    }
                    Ok(())
                })
        })?;
        for (j, &column) in active.iter().enumerate() {
            result.slots[j] = prepared
                .fit_model()
                .coefficients
                .iter()
                .position(|c| c.generated_column_index == Some(column))
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "profiled column has no coefficient slot",
                    )
                })?;
        }
        result.initialize_groups(design, y, q)?;
        Ok(result)
    }

    fn reserve(n: usize, k: usize, g: usize, threads: ThreadConfig) -> Result<Self> {
        let _ = Self::bytes(n, k, g)?;
        let square = k * k;
        let blocks = g.div_ceil(GROUPS_PER_BLOCK);
        let mut partials = Vec::new();
        partials
            .try_reserve_exact(blocks)
            .map_err(|_| allocation())?;
        if partials.capacity() > blocks {
            return Err(allocation());
        }
        for _ in 0..blocks {
            partials.push(Moments {
                mean: filled(k, 0.)?,
                centered: filled(k, 0.)?,
                gradient: filled(k, 0.)?,
                hessian: filled(square, 0.)?,
            });
        }
        Ok(Self {
            team: ThreadTeam::new(threads).acquire()?,
            columns: k,
            data: filled(n * k, 0.)?,
            active: filled(k, 0)?,
            slots: filled(k, 0)?,
            log_weights: filled(n, 0.)?,
            log_outcomes: filled(g, 0.)?,
            shifts: filled(g, 0.)?,
            eta: filled(n, 0.)?,
            mean: filled(n, 0.)?,
            base: filled(k, 0.)?,
            direction: filled(k, 0.)?,
            scales: filled(k, 0.)?,
            gradient: filled(k, 0.)?,
            hessian: filled(square, 0.)?,
            factor: filled(square, 0.)?,
            partials,
        })
    }

    fn initialize_groups(&mut self, design: &FeDesign, y: &[f64], q: &[f64]) -> Result<()> {
        for (target, &weight) in self.log_weights.iter_mut().zip(q) {
            if !weight.is_finite() || weight <= 0. {
                return Err(numerical());
            }
            *target = weight.ln();
        }
        for (group, log_y) in self.log_outcomes.iter_mut().enumerate() {
            let rows = design.effects()[0].level_index().members(group);
            let mut maximum = f64::NEG_INFINITY;
            for &row in rows {
                if y[row] > 0. {
                    maximum = maximum.max(self.log_weights[row] + y[row].ln());
                }
            }
            if !maximum.is_finite() {
                return Err(numerical());
            }
            let mut sum = CompensatedSum::default();
            for &row in rows {
                if y[row] > 0. {
                    sum.add((self.log_weights[row] + y[row].ln() - maximum).exp());
                }
            }
            *log_y = maximum + sum.total().ln();
            if !log_y.is_finite() {
                return Err(numerical());
            }
        }
        Ok(())
    }

    pub(super) fn active_slots(&self) -> &[usize] {
        &self.slots
    }
    pub(super) fn active_rank(&self) -> usize {
        self.columns
    }

    /// Construction requires exact capacities, and all iteration operations
    /// overwrite fixed-length buffers. The allocator probe independently
    /// detects allocate/free churn that capacity equality cannot detect.
    pub(super) fn capacities_unchanged(&self) -> bool {
        let k = self.columns;
        let n = self.eta.len();
        let g = self.shifts.len();
        let exact = |v: &Vec<f64>, length| v.len() == length && v.capacity() == length;
        exact(&self.data, n * k)
            && self.active.len() == k
            && self.active.capacity() == k
            && self.slots.len() == k
            && self.slots.capacity() == k
            && exact(&self.log_weights, n)
            && exact(&self.log_outcomes, g)
            && exact(&self.shifts, g)
            && exact(&self.eta, n)
            && exact(&self.mean, n)
            && exact(&self.base, k)
            && exact(&self.direction, k)
            && exact(&self.scales, k)
            && exact(&self.gradient, k)
            && exact(&self.hessian, k * k)
            && exact(&self.factor, k * k)
            && self.partials.len() == g.div_ceil(GROUPS_PER_BLOCK)
            && self.partials.capacity() == self.partials.len()
            && self.partials.iter().all(|p| {
                exact(&p.mean, k)
                    && exact(&p.centered, k)
                    && exact(&p.gradient, k)
                    && exact(&p.hessian, k * k)
            })
    }

    /// Outside the allocation window, reconstruct all immutable inputs and
    /// recompute the current concentrated step in a separately owned engine.
    /// This is a storage-reuse witness, not the mathematical acceptance oracle.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fresh_direction_equivalent<P: PreparedOlsFitView + ?Sized>(
        &self,
        prepared: &P,
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        offset: &[f64],
        coefficients: &[f64],
        threads: ThreadConfig,
        config: PpmlConfig,
        tolerance: f64,
    ) -> Result<bool> {
        let mut fresh = Self::new(prepared, &self.active, design, y, q, threads)?;
        fresh.prepare_direction(design, y, q, offset, coefficients, config)?;
        Ok(self.capacities_unchanged()
            && self.slots == fresh.slots
            && [
                &self.eta,
                &self.mean,
                &self.gradient,
                &self.hessian,
                &self.direction,
            ]
            .into_iter()
            .zip([
                &fresh.eta,
                &fresh.mean,
                &fresh.gradient,
                &fresh.hessian,
                &fresh.direction,
            ])
            .all(|(a, b)| {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(&x, &y)| {
                        x.is_finite()
                            && y.is_finite()
                            && (x - y).abs() <= tolerance * x.abs().max(y.abs()).max(1.)
                    })
            }))
    }

    fn profile(
        &mut self,
        design: &FeDesign,
        offset: &[f64],
        step: f64,
        config: PpmlConfig,
    ) -> Result<()> {
        let team = self.team.clone();
        team.install(|| self.profile_in_team(design, offset, step, config))
    }

    fn profile_in_team(
        &mut self,
        design: &FeDesign,
        offset: &[f64],
        step: f64,
        config: PpmlConfig,
    ) -> Result<()> {
        let k = self.columns;
        let data = &self.data;
        let base = &self.base;
        let direction = &self.direction;
        self.team.install(|| {
            self.eta.par_iter_mut().enumerate().for_each(|(row, eta)| {
                let mut value = offset[row];
                for j in 0..k {
                    value = (step.mul_add(direction[j], base[j])).mul_add(data[row * k + j], value);
                }
                *eta = value;
            })
        });
        let eta = &self.eta;
        let log_q = &self.log_weights;
        let log_y = &self.log_outcomes;
        self.team.install(|| {
            self.shifts
                .par_iter_mut()
                .enumerate()
                .try_for_each(|(group, shift)| -> Result<()> {
                    let rows = design.effects()[0].level_index().members(group);
                    let maximum = rows
                        .iter()
                        .map(|&r| eta[r] + log_q[r])
                        .fold(f64::NEG_INFINITY, f64::max);
                    let mut sum = CompensatedSum::default();
                    for &row in rows {
                        sum.add((eta[row] + log_q[row] - maximum).exp());
                    }
                    *shift = log_y[group] - maximum - sum.total().ln();
                    if !shift.is_finite() {
                        return Err(numerical());
                    }
                    Ok(())
                })
        })?;
        let shifts = &self.shifts;
        let codes = design.effects()[0].codes();
        self.team.install(|| {
            self.eta
                .par_iter_mut()
                .enumerate()
                .for_each(|(row, eta)| *eta += shifts[codes[row] as usize])
        });
        means_from_eta(&self.eta, config.bounds, &mut self.mean)
    }

    /// A centered Hessian is assembled directly, avoiding subtraction of two
    /// large raw cross-products. Partitioned reductions have fixed boundaries.
    pub(super) fn prepare_direction(
        &mut self,
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        offset: &[f64],
        accepted_coefficients: &[f64],
        config: PpmlConfig,
    ) -> Result<f64> {
        // One submitted job owns the dependent numerical phases. Nested
        // installs use that same worker team without re-entering its external
        // injection queue. Interrupt/SPI polling stays on the caller thread.
        let team = self.team.clone();
        team.install(|| {
            self.prepare_direction_in_team(design, y, q, offset, accepted_coefficients, config)
        })
    }

    fn prepare_direction_in_team(
        &mut self,
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        offset: &[f64],
        accepted_coefficients: &[f64],
        config: PpmlConfig,
    ) -> Result<f64> {
        for (j, &slot) in self.slots.iter().enumerate() {
            self.base[j] = accepted_coefficients[slot];
        }
        self.direction.fill(0.);
        self.profile(design, offset, 0., config)?;
        let maximum = q
            .iter()
            .zip(&self.mean)
            .map(|(&q, &mu)| q * mu)
            .fold(0.0_f64, f64::max);
        if !maximum.is_finite() || maximum <= 0. {
            return Err(numerical());
        }
        let k = self.columns;
        let data = &self.data;
        let mu = &self.mean;
        self.team.install(|| {
            self.partials.par_iter_mut().enumerate().try_for_each(
                |(block, partial)| -> Result<()> {
                    partial.gradient.fill(0.);
                    partial.hessian.fill(0.);
                    let end = ((block + 1) * GROUPS_PER_BLOCK).min(design.effects()[0].n_levels());
                    for group in block * GROUPS_PER_BLOCK..end {
                        let rows = design.effects()[0].level_index().members(group);
                        let first = *rows.first().ok_or_else(numerical)?;
                        partial.mean.fill(0.);
                        let mut total = CompensatedSum::default();
                        for &row in rows {
                            let w = (q[row] * mu[row]) / maximum;
                            if !w.is_finite() || w <= 0. {
                                return Err(numerical());
                            }
                            total.add(w);
                            for j in 0..k {
                                partial.mean[j] = w.mul_add(
                                    data[row * k + j] - data[first * k + j],
                                    partial.mean[j],
                                );
                            }
                        }
                        for value in &mut partial.mean {
                            *value /= total.total();
                        }
                        for &row in rows {
                            let w = (q[row] * mu[row]) / maximum;
                            let score = (q[row] / maximum) * (y[row] - mu[row]);
                            for j in 0..k {
                                partial.centered[j] =
                                    (data[row * k + j] - data[first * k + j]) - partial.mean[j];
                            }
                            for j in 0..k {
                                let x = partial.centered[j];
                                partial.gradient[j] = x.mul_add(score, partial.gradient[j]);
                                for l in 0..=j {
                                    partial.hessian[j * k + l] = (w * x)
                                        .mul_add(partial.centered[l], partial.hessian[j * k + l]);
                                }
                            }
                        }
                    }
                    Ok(())
                },
            )
        })?;
        for j in 0..k {
            let mut score = CompensatedSum::default();
            for p in &self.partials {
                score.add(p.gradient[j]);
            }
            self.gradient[j] = score.total();
            for l in 0..=j {
                let mut sum = CompensatedSum::default();
                for p in &self.partials {
                    sum.add(p.hessian[j * k + l]);
                }
                self.hessian[j * k + l] = sum.total();
                self.hessian[l * k + j] = sum.total();
            }
            self.scales[j] = self.hessian[j * k + j].sqrt();
            if !self.scales[j].is_finite() || self.scales[j] <= 0. {
                return Err(numerical());
            }
        }
        for j in 0..k {
            self.direction[j] = self.gradient[j] / self.scales[j];
            for l in 0..k {
                self.factor[j * k + l] =
                    (self.hessian[j * k + l] / self.scales[j]) / self.scales[l];
            }
        }
        if solve_linear_system(&mut self.factor, &mut self.direction, k, 1, 1e-12)? != k {
            return Err(numerical());
        }
        for j in 0..k {
            self.direction[j] /= self.scales[j];
        }
        // Independently multiply the unfactored system. This is only a step
        // certificate; the outer score and full final Hessian remain separate.
        let mut error = 0.0_f64;
        for j in 0..k {
            let mut sum = CompensatedSum::default();
            let mut scale = self.gradient[j].abs();
            for l in 0..k {
                let product = self.hessian[j * k + l] * self.direction[l];
                sum.add(product);
                scale += product.abs();
            }
            let relative = if scale == 0. {
                0.
            } else {
                (sum.total() - self.gradient[j]).abs() / scale
            };
            if !relative.is_finite() || !self.direction[j].is_finite() {
                return Err(numerical());
            }
            error = error.max(relative);
        }
        if error > 1e-12 {
            return Err(numerical());
        }
        Ok(error)
    }

    pub(super) fn accept(
        &mut self,
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        offset: &[f64],
        workspace: &mut PpmlIterationWorkspace,
        current: PoissonObjective,
        config: PpmlConfig,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<PpmlLineSearchAcceptance> {
        let allowed = current.deviance + config.objective_slack * current.deviance.abs().max(1.);
        let mut step = 1.;
        for halvings in 0..=config.max_step_halvings {
            interrupt()?;
            if step < config.minimum_step {
                break;
            }
            match self.profile(design, offset, step, config) {
                Ok(()) => {}
                Err(e) if e.code == ErrorCode::NonConvergence => {
                    step *= 0.5;
                    continue;
                }
                Err(e) => return Err(e),
            }
            let objective = match poisson_objective_from_eta(y, &self.eta, &self.mean, q) {
                Ok(value) => value,
                Err(e) if e.code == ErrorCode::NonConvergence => {
                    step *= 0.5;
                    continue;
                }
                Err(e) => return Err(e),
            };
            if objective.deviance <= allowed {
                workspace.set_candidate_eta(&self.eta)?;
                for i in 0..workspace.coefficients().len() {
                    let value = workspace.coefficients()[i];
                    workspace.candidate_coefficients_mut()[i] = value;
                }
                for (j, &slot) in self.slots.iter().enumerate() {
                    workspace.candidate_coefficients_mut()[slot] =
                        step.mul_add(self.direction[j], self.base[j]);
                }
                workspace.fill_line_search_trial(1., config.bounds)?;
                workspace.accept_trial()?;
                return Ok(PpmlLineSearchAcceptance {
                    objective,
                    step,
                    step_halvings: halvings,
                });
            }
            step *= 0.5;
        }
        Err(numerical())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn concentrated_direction_matches_full_dummy_weighted_normal_equations() {
        let groups = 129;
        let n = groups * 8;
        let k = 2;
        let codes: Vec<_> = (0..n).map(|r| (r / 8) as u32).collect();
        let design = FeDesign::new(
            n,
            vec![EffectTerm::new(codes, groups, true, vec![]).unwrap()],
        )
        .unwrap();
        let mut reference: Option<(Vec<f64>, Vec<f64>)> = None;
        for common in [1., 1e40, 1e-40] {
            for threads in [1, 4] {
                let mut p = ProfiledOneFe::reserve(
                    n,
                    k,
                    groups,
                    ThreadConfig::from_requested(threads, true).unwrap(),
                )
                .unwrap();
                p.slots.copy_from_slice(&[0, 1]);
                let q: Vec<_> = (0..n).map(|r| common * (0.5 + (r % 5) as f64)).collect();
                let y: Vec<_> = (0..n)
                    .map(|r| {
                        if r % 7 == 0 {
                            0.
                        } else {
                            0.7 + (r % 11) as f64 / 3.
                        }
                    })
                    .collect();
                let offset: Vec<_> = (0..n).map(|r| 0.1 * (r as f64).sin()).collect();
                for r in 0..n {
                    p.data[r * k] = (r % 8) as f64 / 4. - 0.7;
                    p.data[r * k + 1] = ((r % 8) as f64).cos() + 0.2 * (r / 8) as f64;
                }
                p.initialize_groups(&design, &y, &q).unwrap();
                let capacities = (
                    p.data.capacity(),
                    p.eta.capacity(),
                    p.mean.capacity(),
                    p.factor.capacity(),
                    p.partials.capacity(),
                );
                p.prepare_direction(
                    &design,
                    &y,
                    &q,
                    &offset,
                    &[0.3, -0.4],
                    PpmlConfig::default(),
                )
                .unwrap();
                // Independent full dummy system; there is no within projection
                // or group-block elimination in this oracle construction.
                let width = k + groups;
                let mut a = vec![0.; width * width];
                let mut b = vec![0.; width];
                let mut d = vec![0.; width];
                for r in 0..n {
                    d.fill(0.);
                    d[..k].copy_from_slice(&p.data[r * k..(r + 1) * k]);
                    d[k + r / 8] = 1.;
                    let weight = q[r] / common;
                    for j in 0..width {
                        b[j] += weight * (y[r] - p.mean[r]) * d[j];
                        for l in 0..width {
                            a[j * width + l] += weight * p.mean[r] * d[j] * d[l];
                        }
                    }
                }
                assert_eq!(
                    solve_linear_system(&mut a, &mut b, width, 1, 1e-14).unwrap(),
                    width
                );
                for j in 0..k {
                    assert!(
                        (p.direction[j] - b[j]).abs() < 3e-10,
                        "{} != {}",
                        p.direction[j],
                        b[j]
                    );
                }
                for group in 0..groups {
                    let mut target = 0.;
                    let mut actual = 0.;
                    for &r in design.effects()[0].level_index().members(group) {
                        target += (q[r] / common) * y[r];
                        actual += (q[r] / common) * p.mean[r];
                    }
                    assert!((target - actual).abs() < 2e-12 * target);
                }
                assert_eq!(
                    capacities,
                    (
                        p.data.capacity(),
                        p.eta.capacity(),
                        p.mean.capacity(),
                        p.factor.capacity(),
                        p.partials.capacity()
                    )
                );
                if let Some((mean, direction)) = &reference {
                    for (a, b) in mean.iter().zip(&p.mean) {
                        assert!((a - b).abs() < 2e-12);
                    }
                    for (a, b) in direction.iter().zip(&p.direction) {
                        assert!((a - b).abs() < 2e-12);
                    }
                } else {
                    reference = Some((p.mean.clone(), p.direction.clone()));
                }
            }
        }
    }

    #[test]
    fn profiled_direction_refuses_a_lost_finite_rank_and_zero_outcome_group() {
        let design = FeDesign::new(
            8,
            vec![EffectTerm::new(vec![0, 0, 0, 0, 1, 1, 1, 1], 2, true, vec![]).unwrap()],
        )
        .unwrap();
        let mut p = ProfiledOneFe::reserve(8, 2, 2, ThreadConfig::from_requested(1, true).unwrap())
            .unwrap();
        p.slots.copy_from_slice(&[0, 1]);
        for r in 0..8 {
            p.data[r * 2] = (r % 4) as f64;
            p.data[r * 2 + 1] = p.data[r * 2];
        }
        p.initialize_groups(&design, &[1.; 8], &[1.; 8]).unwrap();
        assert_eq!(
            p.prepare_direction(
                &design,
                &[1.; 8],
                &[1.; 8],
                &[0.; 8],
                &[0.; 2],
                PpmlConfig::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::NonConvergence
        );
        assert_eq!(
            p.initialize_groups(&design, &[0., 0., 0., 0., 1., 1., 1., 1.], &[1.; 8])
                .unwrap_err()
                .code,
            ErrorCode::NonConvergence
        );
        assert!(ProfiledOneFe::bytes(usize::MAX, usize::MAX, 2).is_err());
    }
}
