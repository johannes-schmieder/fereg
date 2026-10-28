//! Concentrated Poisson steps after the canonical one-intercept-FE rank decision.
//! Group effects are optimized analytically at every beta trial; the PPML owner
//! still supplies independent outer and canonical strict/final certificates.
use super::family::{means_from_eta, CompensatedSum, PreparedPoissonInputs};
use super::{PoissonObjective, PpmlConfig, PpmlIterationWorkspace, PpmlLineSearchAcceptance};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::linalg::solve_linear_system;
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::ols::NumericOlsColumns;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use rayon::prelude::*;

const GROUPS_PER_BLOCK: usize = 64;

pub(super) trait ProfiledDesign: Copy + Sync {
    fn rows(self) -> usize;
    fn columns(self) -> usize;
    fn value(self, row: usize, column: usize) -> Result<f64>;
}

/// Low storage already owns the original loaded columns. Borrow their recipes
/// instead of seeking retained scratch by group or materializing another N*K
/// matrix. Original-input outer scores and canonical WLS remain independent.
pub(super) struct ProfiledRawColumns<'a, P: PreparedOlsFitView + ?Sized> {
    prepared: &'a P,
    active: &'a [usize],
    numeric: Option<NumericOlsColumns<'a>>,
}

impl<'a, P: PreparedOlsFitView + ?Sized> ProfiledRawColumns<'a, P> {
    pub(super) fn new(prepared: &'a P, active: &'a [usize]) -> Result<Self> {
        if active
            .iter()
            .any(|&j| j >= prepared.fit_model().columns.len())
        {
            return Err(numerical());
        }
        Ok(Self {
            prepared,
            active,
            numeric: NumericOlsColumns::bind_score_means(
                prepared.fit_model(),
                prepared.fit_sample(),
            )?,
        })
    }
}

impl<P: PreparedOlsFitView + ?Sized> ProfiledDesign for &ProfiledRawColumns<'_, P> {
    fn rows(self) -> usize {
        self.prepared.fit_sample().n_active()
    }
    fn columns(self) -> usize {
        self.active.len()
    }
    fn value(self, row: usize, column: usize) -> Result<f64> {
        let generated = self.active[column];
        match &self.numeric {
            Some(numeric) => Ok(numeric.raw_columns()[generated].map_or(1., |v| v[row])),
            None => self.prepared.fit_column_value(generated, row),
        }
    }
}

struct Moments {
    anchor: Vec<f64>,
    mean: Vec<f64>,
    centered: Vec<f64>,
    gradient: Vec<f64>,
    hessian: Vec<f64>,
}

/// A short immutable borrow; no weight-frame or final-Hessian authority.
#[derive(Clone, Copy)]
pub(super) struct ProfiledColumns<'a> {
    values: &'a DenseMatrix,
    positions: Option<&'a [usize]>,
}

impl<'a> ProfiledColumns<'a> {
    pub(super) fn selected(values: &'a DenseMatrix, positions: &'a [usize]) -> Result<Self> {
        if positions.iter().any(|&j| j >= values.ncols()) {
            return Err(numerical());
        }
        let all =
            positions.len() == values.ncols() && positions.iter().enumerate().all(|(j, &p)| j == p);
        Ok(Self {
            values,
            positions: if all { None } else { Some(positions) },
        })
    }
    fn value(self, row: usize, column: usize) -> f64 {
        self.values[(row, self.positions.map_or(column, |p| p[column]))]
    }
    fn columns(self) -> usize {
        self.positions.map_or(self.values.ncols(), |p| p.len())
    }
}

impl ProfiledDesign for ProfiledColumns<'_> {
    fn rows(self) -> usize {
        self.values.nrows()
    }
    fn columns(self) -> usize {
        self.columns()
    }
    fn value(self, row: usize, column: usize) -> Result<f64> {
        Ok(self.value(row, column))
    }
}

#[derive(Clone, Copy)]
pub(super) struct OneFeProfileLayout {
    rows: usize,
    columns: usize,
    groups: usize,
    square: usize,
    blocks: usize,
}
impl OneFeProfileLayout {
    pub(super) fn new(n: usize, k: usize, g: usize) -> Result<Self> {
        Ok(Self {
            rows: n,
            columns: k,
            groups: g,
            square: k.checked_mul(k).ok_or_else(allocation)?,
            blocks: g.checked_add(GROUPS_PER_BLOCK - 1).ok_or_else(allocation)? / GROUPS_PER_BLOCK,
        })
    }
    pub(super) fn bytes(self, audit: bool) -> Result<u128> {
        let Self {
            rows: n,
            columns: k,
            groups: g,
            square,
            blocks,
        } = self;
        let base = std::mem::size_of::<ProfiledOneFe>() as u128
            + 2 * std::mem::size_of::<usize>() as u128 * k as u128
            + blocks as u128 * std::mem::size_of::<Moments>() as u128
            + 8 * (3 * n as u128
                + 2 * g as u128
                + 4 * k as u128
                + 2 * square as u128
                + blocks as u128 * (4 * k as u128 + square as u128));
        if !audit {
            return Ok(base);
        }
        let data = n.checked_mul(k).ok_or_else(allocation)?;
        base.checked_add(base)
            .and_then(|v| v.checked_add(std::mem::size_of::<DenseMatrix>() as u128))
            .and_then(|v| v.checked_add(8 * data as u128))
            .ok_or_else(allocation)
    }
    pub(super) fn construct<P: PreparedOlsFitView + ?Sized>(
        self,
        prepared: &P,
        active: &[usize],
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        threads: ThreadConfig,
    ) -> Result<ProfiledOneFe> {
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
        if self.rows != y.len()
            || self.groups != design.effects()[0].n_levels()
            || active.len() > self.columns
        {
            return Err(allocation());
        }
        let layout = Self::new(self.rows, active.len(), self.groups)?;
        let mut result = ProfiledOneFe::reserve_layout(layout, threads)?;
        result.active.copy_from_slice(active);
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
}

pub(super) struct ProfiledOneFe {
    team: InstalledThreadTeam,
    columns: usize,
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
    pub(super) fn active_columns(&self) -> &[usize] {
        &self.active
    }

    #[cfg(test)]
    pub(super) fn new<P: PreparedOlsFitView + ?Sized>(
        prepared: &P,
        active: &[usize],
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        threads: ThreadConfig,
    ) -> Result<Self> {
        OneFeProfileLayout::new(
            y.len(),
            active.len(),
            design.effects().first().map_or(0, |e| e.n_levels()),
        )?
        .construct(prepared, active, design, y, q, threads)
    }

    #[cfg(test)]
    fn reserve(n: usize, k: usize, g: usize, threads: ThreadConfig) -> Result<Self> {
        Self::reserve_layout(OneFeProfileLayout::new(n, k, g)?, threads)
    }

    fn reserve_layout(layout: OneFeProfileLayout, threads: ThreadConfig) -> Result<Self> {
        let OneFeProfileLayout {
            rows: n,
            columns: k,
            groups: g,
            square,
            blocks,
        } = layout;
        let mut partials = Vec::new();
        partials
            .try_reserve_exact(blocks)
            .map_err(|_| allocation())?;
        if partials.capacity() > blocks {
            return Err(allocation());
        }
        for _ in 0..blocks {
            partials.push(Moments {
                anchor: filled(k, 0.)?,
                mean: filled(k, 0.)?,
                centered: filled(k, 0.)?,
                gradient: filled(k, 0.)?,
                hessian: filled(square, 0.)?,
            });
        }
        Ok(Self {
            team: ThreadTeam::new(threads).acquire()?,
            columns: k,
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
        self.active.len() == k
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
                exact(&p.anchor, k)
                    && exact(&p.mean, k)
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
        let mut fresh = OneFeProfileLayout::new(y.len(), self.active.len(), self.shifts.len())?
            .construct(prepared, &self.active, design, y, q, threads)?;
        let mut data = DenseMatrix::zeros(y.len(), self.columns)?;
        for row in 0..y.len() {
            for (j, &column) in self.active.iter().enumerate() {
                data[(row, j)] = prepared.fit_column_value(column, row)?;
            }
        }
        fresh.prepare_direction(
            ProfiledColumns {
                values: &data,
                positions: None,
            },
            design,
            y,
            q,
            offset,
            coefficients,
            config,
        )?;
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
        data: impl ProfiledDesign,
        design: &FeDesign,
        offset: &[f64],
        step: f64,
        config: PpmlConfig,
    ) -> Result<()> {
        let team = self.team.clone();
        team.install(|| self.profile_in_team(data, design, offset, step, config))
    }

    fn profile_in_team(
        &mut self,
        data: impl ProfiledDesign,
        design: &FeDesign,
        offset: &[f64],
        step: f64,
        config: PpmlConfig,
    ) -> Result<()> {
        // Trace-only laps (FEREG_PHASE_TIMING): no arithmetic or decision effect.
        let trace = crate::diagnostics::phase_ledger::PhaseLedger::new(
            "ppml_one_fe_profile",
            ["eta", "shifts", "apply", "means"],
        );
        let k = self.columns;
        let base = &self.base;
        let direction = &self.direction;
        self.team.install(|| {
            self.eta
                .par_iter_mut()
                .enumerate()
                .try_for_each(|(row, eta)| -> Result<()> {
                    let mut value = offset[row];
                    for j in 0..k {
                        value = (step.mul_add(direction[j], base[j]))
                            .mul_add(data.value(row, j)?, value);
                    }
                    *eta = value;
                    Ok(())
                })
        })?;
        trace.lap(0);
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
        trace.lap(1);
        let shifts = &self.shifts;
        let codes = design.effects()[0].codes();
        self.team.install(|| {
            self.eta
                .par_iter_mut()
                .enumerate()
                .for_each(|(row, eta)| *eta += shifts[codes[row] as usize])
        });
        trace.lap(2);
        let result = means_from_eta(&self.eta, config.bounds, &mut self.mean);
        trace.lap(3);
        result
    }

    /// A centered Hessian is assembled directly, avoiding subtraction of two
    /// large raw cross-products. Partitioned reductions have fixed boundaries.
    // Keep the legacy workspace and ownership boundaries explicit during migration.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_direction(
        &mut self,
        data: impl ProfiledDesign,
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
            self.prepare_direction_in_team(
                data,
                design,
                y,
                q,
                offset,
                accepted_coefficients,
                config,
            )
        })
    }

    // Keep the legacy workspace and ownership boundaries explicit during migration.
    #[allow(clippy::too_many_arguments)]
    fn prepare_direction_in_team(
        &mut self,
        data: impl ProfiledDesign,
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        offset: &[f64],
        accepted_coefficients: &[f64],
        config: PpmlConfig,
    ) -> Result<f64> {
        if data.rows() != y.len() || data.columns() != self.columns {
            return Err(numerical());
        }
        for (j, &slot) in self.slots.iter().enumerate() {
            self.base[j] = accepted_coefficients[slot];
        }
        self.direction.fill(0.);
        self.profile(data, design, offset, 0., config)?;
        // Trace-only lap (FEREG_PHASE_TIMING): no arithmetic or decision effect.
        let trace =
            crate::diagnostics::phase_ledger::PhaseLedger::new("ppml_one_fe_direction", ["max"]);
        let maximum = q
            .iter()
            .zip(&self.mean)
            .map(|(&q, &mu)| q * mu)
            .fold(0.0_f64, f64::max);
        trace.lap(0);
        drop(trace);
        if !maximum.is_finite() || maximum <= 0. {
            return Err(numerical());
        }
        let k = self.columns;
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
                        // This immutable group row anchors both passes. Bind it
                        // once instead of reevaluating its recipes for every row.
                        for (j, value) in partial.anchor.iter_mut().enumerate() {
                            *value = data.value(first, j)?;
                        }
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
                                    data.value(row, j)? - partial.anchor[j],
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
                                    (data.value(row, j)? - partial.anchor[j]) - partial.mean[j];
                            }
                            for j in 0..k {
                                let x = partial.centered[j];
                                partial.gradient[j] = x.mul_add(score, partial.gradient[j]);
                                let weighted = w * x;
                                for (entry, &right) in partial.hessian[j * k..j * k + j + 1]
                                    .iter_mut()
                                    .zip(&partial.centered[..=j])
                                {
                                    *entry = weighted.mul_add(right, *entry);
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

    // Keep the legacy workspace and ownership boundaries explicit during migration.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn accept(
        &mut self,
        data: impl ProfiledDesign,
        design: &FeDesign,
        inputs: &PreparedPoissonInputs<'_>,
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
            match self.profile(data, design, offset, step, config) {
                Ok(()) => {}
                Err(e) if e.code == ErrorCode::NonConvergence => {
                    step *= 0.5;
                    continue;
                }
                Err(e) => return Err(e),
            }
            let objective =
                match workspace.evaluate_external_objective(inputs, &self.eta, &self.mean) {
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
                workspace.fill_prepared_line_search_trial_with(
                    1.,
                    config.bounds,
                    inputs,
                    |_| Ok(()),
                )?;
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
    fn borrowed_group_representatives_preserve_profile_across_weight_frames() {
        let n = 80;
        let groups = 8;
        let design = FeDesign::new(
            n,
            vec![EffectTerm::new(
                (0..n).map(|r| (r % groups) as u32).collect(),
                groups,
                true,
                vec![],
            )
            .unwrap()],
        )
        .unwrap();
        let mut raw = DenseMatrix::zeros(n, 2).unwrap();
        let mut shifted = DenseMatrix::zeros(n, 3).unwrap();
        for row in 0..n {
            raw[(row, 0)] = (row as f64 / 7.).sin();
            raw[(row, 1)] = (row as f64 / 13.).cos();
            shifted[(row, 2)] = raw[(row, 0)] - (row % groups) as f64 * 0.3;
            shifted[(row, 0)] = raw[(row, 1)] + (row % groups) as f64 * 0.7;
            shifted[(row, 1)] = 42.;
        }
        let y: Vec<_> = (0..n).map(|r| 0.3 + (r % 11) as f64).collect();
        let offset: Vec<_> = (0..n).map(|r| (r as f64 / 19.).sin()).collect();
        for threads in [1, 4] {
            let threads = ThreadConfig::from_requested(threads, true).unwrap();
            let mut original = ProfiledOneFe::reserve(n, 2, groups, threads).unwrap();
            let mut borrowed = ProfiledOneFe::reserve(n, 2, groups, threads).unwrap();
            original.slots.copy_from_slice(&[0, 1]);
            borrowed.slots.copy_from_slice(&[0, 1]);
            for frame in 0..4 {
                let q: Vec<_> = (0..n)
                    .map(|r| 0.2 + ((r * (frame + 1)) % 17) as f64 / 5.)
                    .collect();
                original.initialize_groups(&design, &y, &q).unwrap();
                borrowed.initialize_groups(&design, &y, &q).unwrap();
                original
                    .prepare_direction(
                        ProfiledColumns {
                            values: &raw,
                            positions: None,
                        },
                        &design,
                        &y,
                        &q,
                        &offset,
                        &[0.3, -0.2],
                        PpmlConfig::default(),
                    )
                    .unwrap();
                borrowed
                    .prepare_direction(
                        ProfiledColumns::selected(&shifted, &[2, 0]).unwrap(),
                        &design,
                        &y,
                        &q,
                        &offset,
                        &[0.3, -0.2],
                        PpmlConfig::default(),
                    )
                    .unwrap();
                for (a, b) in [
                    &original.eta,
                    &original.mean,
                    &original.gradient,
                    &original.hessian,
                    &original.direction,
                ]
                .into_iter()
                .zip([
                    &borrowed.eta,
                    &borrowed.mean,
                    &borrowed.gradient,
                    &borrowed.hessian,
                    &borrowed.direction,
                ]) {
                    assert!(a
                        .iter()
                        .zip(b)
                        .all(|(x, y)| (x - y).abs() < 1e-11 * x.abs().max(y.abs()).max(1.)));
                }
                assert!(original.capacities_unchanged() && borrowed.capacities_unchanged());
            }
        }
        assert!(ProfiledColumns::selected(&shifted, &[3]).is_err());
        assert_eq!(
            OneFeProfileLayout::new(n, 2, groups)
                .unwrap()
                .bytes(true)
                .unwrap()
                - 2 * OneFeProfileLayout::new(n, 2, groups)
                    .unwrap()
                    .bytes(false)
                    .unwrap(),
            (n * 2 * 8 + std::mem::size_of::<DenseMatrix>()) as u128
        );
    }

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
                let mut data = DenseMatrix::zeros(n, k).unwrap();
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
                    data[(r, 0)] = (r % 8) as f64 / 4. - 0.7;
                    data[(r, 1)] = ((r % 8) as f64).cos() + 0.2 * (r / 8) as f64;
                }
                p.initialize_groups(&design, &y, &q).unwrap();
                let capacities = (
                    p.eta.capacity(),
                    p.mean.capacity(),
                    p.factor.capacity(),
                    p.partials.capacity(),
                );
                p.prepare_direction(
                    ProfiledColumns {
                        values: &data,
                        positions: None,
                    },
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
                    d[..k].copy_from_slice(&data.as_slice()[r * k..(r + 1) * k]);
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
                // Keep the legacy matrix indices and numerical traversal order explicit.
                #[allow(clippy::needless_range_loop)]
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
        let mut data = DenseMatrix::zeros(8, 2).unwrap();
        p.slots.copy_from_slice(&[0, 1]);
        for r in 0..8 {
            data[(r, 0)] = (r % 4) as f64;
            data[(r, 1)] = data[(r, 0)];
        }
        p.initialize_groups(&design, &[1.; 8], &[1.; 8]).unwrap();
        assert_eq!(
            p.prepare_direction(
                ProfiledColumns {
                    values: &data,
                    positions: None
                },
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
        assert!(OneFeProfileLayout::new(usize::MAX, usize::MAX, 2).is_err());
    }
}
