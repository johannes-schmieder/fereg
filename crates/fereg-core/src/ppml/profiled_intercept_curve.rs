//! Conditional Poisson correction of one partition along a certified Newton
//! direction. Only immutable prior-weight/outcome moments survive a trial.
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
    slope_steps: Vec<f64>,
    slope_scales: Vec<f64>,
    slope_targets: Vec<f64>,
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
    pub(super) fn selected_effect(design: &FeDesign) -> Option<usize> {
        let effects = design.effects();
        if effects.len() != 2 || effects.iter().any(|e| !e.has_intercept()) {
            return None;
        }
        let has_slopes = effects.iter().any(|e| !e.slopes().is_empty());
        // Preserve the categorical choice. For slopes, correct the eligible
        // slope partition with most groups, breaking ties in design order.
        let mut selected = None;
        for (index, effect) in effects.iter().enumerate() {
            if has_slopes && effect.slopes().is_empty() {
                continue;
            }
            if selected.map_or(true, |old: usize| {
                effect.n_levels() > effects[old].n_levels()
            }) {
                selected = Some(index);
            }
        }
        selected
    }
    pub(super) fn bytes(n: usize, k: usize, g: usize, slopes: usize) -> u128 {
        std::mem::size_of::<Self>() as u128
            + std::mem::size_of::<usize>() as u128 * 2 * k as u128
            + 8 * (n as u128 + (2 + 2 * slopes as u128 + u128::from(slopes > 0)) * g as u128)
    }
    fn from_inputs(
        design: &FeDesign,
        y: &[f64],
        q: &[f64],
        threads: ThreadConfig,
        k: usize,
    ) -> Result<Self> {
        if y.len() != design.nobs() || q.len() != y.len() {
            return Err(numerical());
        }
        let effect = Self::selected_effect(design).ok_or_else(numerical)?;
        let g = design.effects()[effect].n_levels();
        let slopes = design.effects()[effect].slopes().len();
        let moments = g.checked_mul(slopes).ok_or_else(allocation)?;
        let mut result = Self {
            team: ThreadTeam::new(threads).acquire()?,
            effect,
            slots: filled(k, 0)?,
            log_weights: filled(y.len(), 0.)?,
            log_outcomes: filled(g, 0.)?,
            shifts: filled(g, 0.)?,
            slope_steps: filled(if slopes == 0 { 0 } else { g }, 0.)?,
            slope_scales: filled(moments, 0.)?,
            slope_targets: filled(moments, 0.)?,
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
        for (channel, slope) in design.effects()[effect].slopes().iter().enumerate() {
            for group in 0..g {
                let rows = design.effects()[effect].level_index().members(group);
                let first = slope[rows[0]];
                if rows.iter().all(|&r| slope[r] == first) {
                    continue;
                }
                let scale = rows.iter().map(|&r| slope[r].abs()).fold(0.0_f64, f64::max);
                if !scale.is_finite() || scale <= 0.0 {
                    return Err(numerical());
                }
                let mut target = CompensatedSum::default();
                for &r in rows {
                    if y[r] > 0.0 {
                        target.add(
                            (result.log_weights[r] + y[r].ln() - result.log_outcomes[group]).exp()
                                * (slope[r] / scale),
                        );
                    }
                }
                let index = channel * g + group;
                result.slope_scales[index] = scale;
                result.slope_targets[index] = target.total();
                if !target.total().is_finite() {
                    return Err(numerical());
                }
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
            && self.slope_steps.capacity() == self.slope_steps.len()
            && self.slope_scales.capacity() == self.slope_scales.len()
            && self.slope_targets.capacity() == self.slope_targets.len()
            && self.slope_scales.len() == self.slope_targets.len()
    }
    pub(super) fn active_slots(&self) -> &[usize] {
        &self.slots
    }
    fn project(&mut self, design: &FeDesign, eta: &mut [f64], config: PpmlConfig) -> Result<()> {
        if eta.len() != self.log_weights.len() || design.nobs() != eta.len() {
            return Err(numerical());
        }
        if Self::selected_effect(design) != Some(self.effect)
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
                })?;
            let groups = effect.n_levels();
            for (channel, slope) in effect.slopes().iter().enumerate() {
                let scales = &self.slope_scales[channel * groups..(channel + 1) * groups];
                let targets = &self.slope_targets[channel * groups..(channel + 1) * groups];
                self.shifts
                    .par_iter_mut()
                    .zip(self.slope_steps.par_iter_mut())
                    .enumerate()
                    .for_each(|(g, (shift, step))| {
                        (*step, *shift) = slope_proposal(
                            effect.level_index().members(g),
                            slope,
                            scales[g],
                            targets[g],
                            log_weights,
                            log_outcomes[g],
                            eta,
                            config,
                        );
                    });
                eta.par_iter_mut()
                    .zip(effect.codes().par_iter())
                    .enumerate()
                    .for_each(|(r, (value, &g))| {
                        let g = g as usize;
                        if scales[g] > 0.0 {
                            *value += self.slope_steps[g] * (slope[r] / scales[g]) + self.shifts[g];
                        }
                    });
            }
            Ok(())
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
            match workspace.fill_prepared_line_search_trial_with(
                step,
                config.bounds,
                inputs,
                |eta| self.project(design, eta, config),
            ) {
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

// Normalize all moments by the immutable group outcome mass: this cancels the
// arbitrary prior-weight scale without retaining changing Hessian geometry.
#[allow(clippy::too_many_arguments)]
fn slope_proposal(
    rows: &[usize],
    slope: &[f64],
    scale: f64,
    target: f64,
    log_weights: &[f64],
    log_outcome: f64,
    eta: &[f64],
    config: PpmlConfig,
) -> (f64, f64) {
    if scale == 0.0 {
        return (0.0, 0.0);
    }
    let mut mass = CompensatedSum::default();
    let mut first = CompensatedSum::default();
    for &r in rows {
        let w = (log_weights[r] + eta[r] - log_outcome).exp();
        mass.add(w);
        first.add(w * (slope[r] / scale));
    }
    let center = first.total() / mass.total();
    let mut curvature = CompensatedSum::default();
    for &r in rows {
        let z = slope[r] / scale - center;
        curvature.add((log_weights[r] + eta[r] - log_outcome).exp() * z * z);
    }
    let curvature = curvature.total() / mass.total();
    if !curvature.is_finite() || curvature <= 0.0 {
        return (0.0, 0.0);
    }
    let mut step = (target - center) / curvature;
    if !step.is_finite() {
        return (0.0, 0.0);
    }
    for _ in 0..=config.max_step_halvings {
        let maximum = rows
            .iter()
            .map(|&r| log_weights[r] + eta[r] + step * (slope[r] / scale))
            .fold(f64::NEG_INFINITY, f64::max);
        let mut sum = CompensatedSum::default();
        for &r in rows {
            sum.add((log_weights[r] + eta[r] + step * (slope[r] / scale) - maximum).exp());
        }
        let shift = log_outcome - (maximum + sum.total().ln());
        let change = step * target + shift - (1.0 - mass.total());
        if change.is_finite()
            && change >= 0.0
            && rows.iter().all(|&r| {
                let value = eta[r] + step * (slope[r] / scale) + shift;
                let mu = value.exp();
                value.is_finite()
                    && value >= config.bounds.eta_min
                    && value <= config.bounds.eta_max
                    && mu.is_finite()
                    && mu >= config.bounds.mu_min
                    && mu <= config.bounds.mu_max
            })
        {
            return (step, shift);
        }
        step *= 0.5;
    }
    // The exact intercept correction remains usable when an optional slope
    // coordinate is numerically unrepresentable or fails its local safeguard.
    (0.0, 0.0)
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
                owner
                    .project(&design, &mut eta, PpmlConfig::default())
                    .unwrap();
                let mut shifted: Vec<_> = base
                    .iter()
                    .enumerate()
                    .map(|(r, &v)| v + (r % 12) as f64 * 0.7 - 3.)
                    .collect();
                owner
                    .project(&design, &mut shifted, PpmlConfig::default())
                    .unwrap();
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
    #[test]
    fn slope_trial_matches_independent_profiled_newton_and_preserves_group_mass() {
        let n = 96;
        let base: Vec<f64> = (0..n).map(|r| (r as f64 / 11.).sin() * 0.3).collect();
        let y: Vec<f64> = (0..n).map(|r| 1.0 + (r % 7) as f64 / 8.0).collect();
        for slope_scale in [1e-80, 1.0, 1e80] {
            let z: Vec<f64> = (0..n)
                .map(|r| ((r / 12) as f64 - 3.5) * slope_scale)
                .collect();
            let design = FeDesign::new(
                n,
                vec![
                    EffectTerm::new(
                        (0..n).map(|r| (r % 12) as u32).collect(),
                        12,
                        true,
                        vec![z.clone()],
                    )
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
            for weight_scale in [1e-200, 1.0, 1e200] {
                let q: Vec<f64> = (0..n)
                    .map(|r| weight_scale * (1.0 + (r % 3) as f64))
                    .collect();
                for threads in [1, 4] {
                    let mut owner = ProfiledInterceptCurve::from_inputs(
                        &design,
                        &y,
                        &q,
                        ThreadConfig::from_requested(threads, true).unwrap(),
                        0,
                    )
                    .unwrap();
                    let mut eta = base.clone();
                    owner
                        .project(&design, &mut eta, PpmlConfig::default())
                        .unwrap();
                    for group in 0..12 {
                        let rows = design.effects()[0].level_index().members(group);
                        let mass: f64 = rows
                            .iter()
                            .map(|&r| q[r] / weight_scale * base[r].exp())
                            .sum();
                        let observed: f64 = rows.iter().map(|&r| q[r] / weight_scale * y[r]).sum();
                        let first: f64 = rows
                            .iter()
                            .map(|&r| q[r] / weight_scale * base[r].exp() * (z[r] / slope_scale))
                            .sum();
                        let center = first / mass;
                        let variance: f64 = rows
                            .iter()
                            .map(|&r| {
                                q[r] / weight_scale
                                    * base[r].exp()
                                    * (z[r] / slope_scale - center).powi(2)
                            })
                            .sum::<f64>()
                            / mass;
                        let target: f64 = rows
                            .iter()
                            .map(|&r| q[r] / weight_scale * y[r] * (z[r] / slope_scale))
                            .sum::<f64>()
                            / observed;
                        let step = (target - center) / variance;
                        let trial_mass: f64 = rows
                            .iter()
                            .map(|&r| {
                                q[r] / weight_scale * (base[r] + step * z[r] / slope_scale).exp()
                            })
                            .sum();
                        let shift = (observed / trial_mass).ln();
                        for &r in rows {
                            assert!(
                                (eta[r] - base[r] - step * z[r] / slope_scale - shift).abs()
                                    < 2e-12
                            );
                        }
                        let actual: f64 = rows
                            .iter()
                            .map(|&r| q[r] / weight_scale * eta[r].exp())
                            .sum();
                        assert!((actual / observed - 1.0).abs() < 2e-12);
                    }
                    assert!(owner.capacities_unchanged());
                }
            }
        }
    }

    #[test]
    fn constant_duplicate_and_refused_slope_trials_remain_finite() {
        let n = 96;
        let z: Vec<f64> = (0..n).map(|r| ((r / 12) as f64 - 3.5) / 8.).collect();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    (0..n).map(|r| (r % 12) as u32).collect(),
                    12,
                    true,
                    vec![vec![0.; n], vec![7.; n], z.clone(), z],
                )
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
        let y: Vec<f64> = (0..n)
            .map(|r| (0.2 + (r / 12) as f64 / 16.).exp())
            .collect();
        let q = vec![1.; n];
        let mut owner =
            ProfiledInterceptCurve::from_inputs(&design, &y, &q, ThreadConfig::default(), 0)
                .unwrap();
        let mut eta = vec![0.; n];
        let objective = |eta: &[f64]| {
            eta.iter()
                .zip(&y)
                .map(|(&v, &y)| y * v - v.exp())
                .sum::<f64>()
        };
        for _ in 0..6 {
            let old = objective(&eta);
            owner
                .project(&design, &mut eta, PpmlConfig::default())
                .unwrap();
            assert!(eta.iter().all(|v| v.is_finite()));
            assert!(objective(&eta) >= old - 1e-12);
        }
        for (&value, &outcome) in eta.iter().zip(&y) {
            assert!((value - outcome.ln()).abs() < 1e-10);
        }
        let rows = design.effects()[0].level_index().members(0);
        let mut config = PpmlConfig::default();
        config.bounds.eta_max = -10.;
        let proposal = slope_proposal(
            rows,
            &design.effects()[0].slopes()[2],
            1.,
            1e200,
            &owner.log_weights,
            owner.log_outcomes[0],
            &eta,
            config,
        );
        assert_eq!(proposal, (0., 0.));
        assert!(owner.capacities_unchanged());
    }
}
