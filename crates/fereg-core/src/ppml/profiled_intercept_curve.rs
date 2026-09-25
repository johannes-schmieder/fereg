//! Analytic optimization of one intercept partition along a certified Newton
//! direction. Only immutable prior-weight/outcome totals survive a trial.
use super::family::{CompensatedSum, PreparedPoissonInputs};
use super::{PoissonObjective, PpmlConfig, PpmlIterationWorkspace, PpmlLineSearchAcceptance};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use crate::prepared_fit_view::PreparedOlsFitView;
use rayon::prelude::*;

pub(super) struct ProfiledInterceptCurve {
    team: InstalledThreadTeam,
    effect: usize,
    slots: Vec<usize>,
    log_weights: Vec<f64>,
    log_outcomes: Vec<f64>,
    shifts: Vec<f64>,
}
fn numerical() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "profiled intercept curve is not representable",
    )
}
fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "profiled intercept curve exceeds admission",
    )
}
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| allocation())?;
    if v.capacity() != n {
        return Err(allocation());
    }
    v.resize(n, value);
    Ok(v)
}
impl ProfiledInterceptCurve {
    // The active-column identity temporarily coexists with the retained slots
    // during construction. Neither bank is credited to the existing solver.
    pub(super) fn bytes(n: usize, k: usize, g: usize) -> u128 {
        std::mem::size_of::<Self>() as u128
            + std::mem::size_of::<usize>() as u128 * 2 * k as u128
            + 8 * (n as u128 + 2 * g as u128)
    }
    fn from_inputs(
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        threads: ThreadConfig,
        k: usize,
    ) -> Result<Self> {
        if design.effects().len() != 2
            || y.len() != design.nobs()
            || q.len() != y.len()
            || design
                .effects()
                .iter()
                .any(|e| !e.has_intercept() || !e.slopes().is_empty())
        {
            return Err(numerical());
        }
        let effect = usize::from(design.effects()[1].n_levels() > design.effects()[0].n_levels());
        let g = design.effects()[effect].n_levels();
        let mut result = Self {
            team: ThreadTeam::new(threads).acquire()?,
            effect,
            slots: filled(k, 0)?,
            log_weights: filled(y.len(), 0.)?,
            log_outcomes: filled(g, 0.)?,
            shifts: filled(g, 0.)?,
        };
        for (target, &weight) in result.log_weights.iter_mut().zip(q) {
            if !weight.is_finite() || weight <= 0. {
                return Err(numerical());
            }
            *target = weight.ln();
        }
        for (g, target) in result.log_outcomes.iter_mut().enumerate() {
            let rows = design.effects()[effect].level_index().members(g);
            let mut maximum = f64::NEG_INFINITY;
            for &r in rows {
                if !y[r].is_finite() || y[r] < 0. {
                    return Err(numerical());
                }
                if y[r] > 0. {
                    maximum = maximum.max(result.log_weights[r] + y[r].ln());
                }
            }
            if !maximum.is_finite() {
                return Err(numerical());
            }
            let mut sum = CompensatedSum::default();
            for &r in rows {
                if y[r] > 0. {
                    sum.add((result.log_weights[r] + y[r].ln() - maximum).exp());
                }
            }
            *target = maximum + sum.total().ln();
            if !target.is_finite() {
                return Err(numerical());
            }
        }
        Ok(result)
    }
    pub(super) fn new<P: PreparedOlsFitView + ?Sized>(
        prepared: &P,
        active: &[usize],
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        threads: ThreadConfig,
    ) -> Result<Self> {
        let mut result = Self::from_inputs(design, y, q, threads, active.len())?;
        for (j, &column) in active.iter().enumerate() {
            result.slots[j] = prepared
                .fit_model()
                .coefficients
                .iter()
                .position(|c| c.generated_column_index == Some(column))
                .ok_or_else(numerical)?;
        }
        Ok(result)
    }
    pub(super) fn capacities_unchanged(&self) -> bool {
        self.slots.capacity() == self.slots.len()
            && self.log_weights.capacity() == self.log_weights.len()
            && self.log_outcomes.capacity() == self.log_outcomes.len()
            && self.shifts.capacity() == self.shifts.len()
            && self.shifts.len() == self.log_outcomes.len()
    }
    pub(super) fn active_slots(&self) -> &[usize] {
        &self.slots
    }
    fn project(&mut self, design: &FeDesign, eta: &mut [f64]) -> Result<()> {
        if eta.len() != self.log_weights.len() || design.nobs() != eta.len() {
            return Err(numerical());
        }
        if design.effects().len() != 2
            || design.effects()[self.effect].n_levels() != self.shifts.len()
            || !self.capacities_unchanged()
        {
            return Err(numerical());
        }
        let effect = &design.effects()[self.effect];
        let log_weights = &self.log_weights;
        let log_outcomes = &self.log_outcomes;
        self.team.install(|| {
            self.shifts
                .par_iter_mut()
                .enumerate()
                .try_for_each(|(g, shift)| -> Result<()> {
                    let rows = effect.level_index().members(g);
                    let mut maximum = f64::NEG_INFINITY;
                    for &r in rows {
                        let value = log_weights[r] + eta[r];
                        if !value.is_finite() {
                            return Err(numerical());
                        }
                        maximum = maximum.max(value);
                    }
                    if !maximum.is_finite() {
                        return Err(numerical());
                    }
                    let mut sum = CompensatedSum::default();
                    for &r in rows {
                        sum.add((log_weights[r] + eta[r] - maximum).exp());
                    }
                    *shift = log_outcomes[g] - (maximum + sum.total().ln());
                    if !shift.is_finite() {
                        return Err(numerical());
                    }
                    Ok(())
                })?;
            eta.par_iter_mut()
                .zip(effect.codes().par_iter())
                .try_for_each(|(value, &g)| {
                    *value += self.shifts[g as usize];
                    if value.is_finite() {
                        Ok(())
                    } else {
                        Err(numerical())
                    }
                })
        })
    }
    pub(super) fn accept(
        &mut self,
        design: &FeDesign,
        inputs: &PreparedPoissonInputs<'_>,
        workspace: &mut PpmlIterationWorkspace,
        current: PoissonObjective,
        config: PpmlConfig,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<PpmlLineSearchAcceptance> {
        let allowed = current.deviance + config.objective_slack * current.deviance.abs().max(1.);
        let mut step = 1.;
        for step_halvings in 0..=config.max_step_halvings {
            interrupt()?;
            if step < config.minimum_step {
                break;
            }
            match workspace
                .fill_prepared_line_search_trial_with(step, config.bounds, inputs, |eta| self.project(design, eta))
            {
                Ok(()) => {}
                Err(e) if e.code == ErrorCode::NonConvergence => {
                    step *= 0.5;
                    continue;
                }
                Err(e) => return Err(e),
            }
            match workspace.evaluate_trial_objective(inputs) {
                Ok(objective) if objective.deviance <= allowed => {
                    workspace.accept_trial()?;
                    return Ok(PpmlLineSearchAcceptance {
                        objective,
                        step,
                        step_halvings,
                    });
                }
                Ok(_) => {}
                Err(e) if e.code == ErrorCode::NonConvergence => {}
                Err(e) => return Err(e),
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
    fn analytic_group_optimum_is_invariant_to_group_shift_and_threads() {
        let n = 96;
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new((0..n).map(|r| (r % 12) as u32).collect(), 12, true, vec![])
                    .unwrap(),
                EffectTerm::new(
                    (0..n).map(|r| ((r / 12) % 4) as u32).collect(),
                    4,
                    true,
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let y: Vec<_> = (0..n).map(|r| (r % 7) as f64 + 0.25).collect();
        for exponent in [0., 60., -60.] {
            let q: Vec<_> = (0..n)
                .map(|r| 10_f64.powf(exponent) * (1. + (r % 3) as f64))
                .collect();
            let base: Vec<_> = (0..n).map(|r| (r as f64 / 11.).sin()).collect();
            for t in [1, 4] {
                let mut owner = ProfiledInterceptCurve::from_inputs(
                    &design,
                    &y,
                    &q,
                    ThreadConfig::from_requested(t, true).unwrap(),
                    0,
                )
                .unwrap();
                let mut eta = base.clone();
                owner.project(&design, &mut eta).unwrap();
                let mut shifted: Vec<_> = base
                    .iter()
                    .enumerate()
                    .map(|(r, &v)| v + (r % 12) as f64 * 0.7 - 3.)
                    .collect();
                owner.project(&design, &mut shifted).unwrap();
                for r in 0..n {
                    assert!((eta[r] - shifted[r]).abs() < 1e-12);
                }
                for g in 0..12 {
                    let rows = design.effects()[0].level_index().members(g);
                    let sy: f64 = rows.iter().map(|&r| q[r] * y[r]).sum();
                    let sm: f64 = rows.iter().map(|&r| q[r] * base[r].exp()).sum();
                    let actual: f64 = rows.iter().map(|&r| q[r] * eta[r].exp()).sum();
                    assert!((actual / sy - 1.).abs() < 1e-12);
                    for &r in rows {
                        assert!((eta[r] - (base[r] + (sy / sm).ln())).abs() < 1e-12);
                    }
                }
            }
        }
        assert!(ProfiledInterceptCurve::from_inputs(
            &design,
            &vec![0.; n],
            &vec![1.; n],
            ThreadConfig::default(),
            0
        )
        .is_err());
    }
}
