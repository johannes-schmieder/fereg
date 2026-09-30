//! Matrix-free Schur complement with independent, batched PCG recurrences.
use super::{
    blocks::Blocks, design::validate_matrix, monitor::Monitor, refinement::Limits, Attempt,
    Backend, Design, MonitorControl, Plan, StopReason,
};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    threads::{compensated_sum, ThreadTeam},
};

/// Recurrence completion is only a candidate; shared reconstruction/span checks
/// and independent accuracy qualification still own backend acceptance.
#[derive(Debug, PartialEq)]
pub struct SchurCandidate {
    pub coefficients: DenseMatrix,
    pub residuals: DenseMatrix,
    pub progress: Vec<Attempt>,
}
/// Numerical factors and warm coefficients cannot outlive or mutate their
/// borrowed design/weight frame. A new frame requires a new session.
pub struct SchurSession<'a, 'data> {
    design: &'a Design<'data>,
    blocks: Blocks,
    eliminated: usize,
    budget: usize,
    tolerance: f64,
    warm: Option<DenseMatrix>,
}
impl<'a, 'data> SchurSession<'a, 'data> {
    pub fn new(design: &'a Design<'data>, plan: &Plan, team: &ThreadTeam) -> Result<Self> {
        Self::new_polled(design, plan, team, None)
    }
    pub(super) fn new_polled(
        design: &'a Design<'data>,
        plan: &Plan,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<Self> {
        execution::check(poll)?;
        if plan.selected() != Backend::SchurPcg || design.layout().effects.len() < 2 {
            return Err(invalid("Schur session requires a multi-effect Schur plan"));
        }
        let eliminated = design
            .layout()
            .effects
            .iter()
            .enumerate()
            .max_by(|(a, left), (b, right)| {
                (left.levels * left.channels)
                    .cmp(&(right.levels * right.channels))
                    .then(b.cmp(a))
            })
            .map(|(index, _)| index)
            .expect("multiple effects");
        if plan.eliminated_effect() != Some(eliminated) {
            return Err(invalid("Schur plan belongs to a different design"));
        }
        Ok(Self {
            design,
            blocks: Blocks::new(design, team, poll)?,
            eliminated,
            budget: plan.linear_budget(),
            tolerance: plan.tolerance(),
            warm: None,
        })
    }
    pub fn clear_warm_start(&mut self) {
        self.warm = None;
    }
    /// Carry coefficient guesses only; all factors belong to this new weight
    /// frame. The remaining RHSs begin at zero and share no prior certificate.
    pub fn seed_response(&mut self, coefficients: &DenseMatrix, rhs_count: usize) -> Result<()> {
        validate_matrix(coefficients, self.design.layout().total)?;
        if coefficients.ncols() != 1 || rhs_count == 0 {
            return Err(invalid("Schur response seed requires one coefficient RHS"));
        }
        let mut warm = DenseMatrix::zeros(self.design.layout().total, rhs_count)?;
        for row in 0..warm.nrows() {
            warm[(row, 0)] = coefficients[(row, 0)];
        }
        self.warm = Some(warm);
        Ok(())
    }
    pub fn solve(&mut self, rhs: &DenseMatrix, team: &ThreadTeam) -> Result<SchurCandidate> {
        self.solve_monitored(rhs, MonitorControl::Disabled, team)
    }
    pub fn solve_monitored(
        &mut self,
        rhs: &DenseMatrix,
        control: MonitorControl,
        team: &ThreadTeam,
    ) -> Result<SchurCandidate> {
        self.solve_budget(
            rhs,
            control,
            true,
            Limits {
                budget: self.budget,
                tolerance: self.tolerance,
                poll: None,
            },
            team,
        )
    }
    pub(super) fn solve_budget(
        &mut self,
        rhs: &DenseMatrix,
        control: MonitorControl,
        warm_start: bool,
        limits: Limits<'_>,
        team: &ThreadTeam,
    ) -> Result<SchurCandidate> {
        let Limits {
            budget,
            tolerance,
            poll,
        } = limits;
        execution::check(poll)?;
        validate_matrix(rhs, self.design.nobs())?;
        if control == MonitorControl::StopAfter(0) {
            return Err(invalid(
                "a forced monitor checkpoint requires positive work",
            ));
        }
        let initial = self.project_eliminated(rhs, team, poll)?;
        let mut b = self.design.transpose_weighted(&initial, team)?;
        zero_effect(self.design, self.eliminated, &mut b);
        let mut x = match &self.warm {
            Some(warm) if warm_start && warm.ncols() == rhs.ncols() => copy_matrix(warm)?,
            _ => DenseMatrix::zeros(self.design.layout().total, rhs.ncols())?,
        };
        zero_effect(self.design, self.eliminated, &mut x);
        let applied = self.apply(&x, team, poll)?;
        let mut r = copy_matrix(&b)?;
        subtract(&mut r, &applied);
        let z = self.precondition(&r, team, poll)?;
        let mut targets = reserved(rhs.ncols())?;
        for column in 0..rhs.ncols() {
            let norm = (0..rhs.nrows()).fold(0_f64, |norm, row| {
                norm.hypot(self.design.weights().at(row).sqrt() * rhs[(row, column)])
            });
            if !norm.is_finite() {
                return Err(numerical("Schur RHS norm is not representable"));
            }
            targets.push(tolerance * norm);
        }
        let mut state = Recurrences::new(x, r, z, targets)?;
        let mut monitors = reserved(rhs.ncols())?;
        monitors.extend(state.rho.iter().map(|rho| Monitor::new(rho.sqrt())));
        for _ in 0..budget {
            execution::check(poll)?;
            if !state.active.iter().any(|&active| active) {
                break;
            }
            let applied = match self.apply(&state.p, team, poll) {
                Ok(applied) => applied,
                Err(error) if error.code == ErrorCode::NonConvergence => {
                    state.fail_active(true);
                    break;
                }
                Err(error) => return Err(error),
            };
            state.advance(&applied);
            let z = match self.precondition(&state.r, team, poll) {
                Ok(z) => z,
                Err(error) if error.code == ErrorCode::NonConvergence => {
                    state.fail_active(false);
                    break;
                }
                Err(error) => return Err(error),
            };
            state.redirect(z);
            for (column, monitor) in monitors.iter_mut().enumerate() {
                if state.active[column] {
                    if let Some(stop) = monitor.observe(
                        state.rho[column].sqrt(),
                        state.targets[column],
                        state.progress[column].iterations,
                        budget,
                        control,
                    ) {
                        state.finish(column, stop);
                    }
                }
            }
            state.mask_finished();
        }
        // Reconstruct eliminated coordinates from the original RHS and current
        // reduced coefficients; a recursive PCG residual never becomes output.
        let residuals = match self.recover(rhs, &mut state.x, team, poll) {
            Ok(residuals) => {
                if warm_start {
                    self.warm = Some(copy_matrix(&state.x)?);
                }
                residuals
            }
            Err(error) if error.code == ErrorCode::NonConvergence => {
                // Failed recovery is not a fresh zero-work failure. Keep every
                // completed recurrence's cost while discarding its unsafe candidate.
                for progress in &mut state.progress {
                    progress.stop = StopReason::NumericalBreakdown;
                }
                state.x.as_mut_slice().fill(0.);
                self.warm = None;
                copy_matrix(rhs)?
            }
            Err(error) => return Err(error),
        };
        Ok(SchurCandidate {
            coefficients: state.x,
            residuals,
            progress: state.progress,
        })
    }
    pub(super) fn refinement_controls(&self) -> (&'a Design<'data>, usize, f64) {
        (self.design, self.budget, self.tolerance)
    }
    pub(super) fn retain_refinement(
        &mut self,
        coefficients: &DenseMatrix,
        complete: bool,
    ) -> Result<()> {
        self.warm = if complete {
            Some(copy_matrix(coefficients)?)
        } else {
            None
        };
        Ok(())
    }
    fn recover(
        &self,
        rhs: &DenseMatrix,
        coefficients: &mut DenseMatrix,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<DenseMatrix> {
        let remaining = self.design.reconstruct(rhs, coefficients, team)?;
        let eliminated =
            self.blocks
                .observation_solve(self.design, self.eliminated, &remaining, team, poll)?;
        for (value, &addition) in coefficients
            .as_mut_slice()
            .iter_mut()
            .zip(eliminated.as_slice())
        {
            *value += addition;
        }
        if coefficients
            .as_slice()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(numerical(
                "Schur recovered coefficients are not representable",
            ));
        }
        self.design.reconstruct(rhs, coefficients, team)
    }
    fn project_eliminated(
        &self,
        rhs: &DenseMatrix,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<DenseMatrix> {
        let coefficients =
            self.blocks
                .observation_solve(self.design, self.eliminated, rhs, team, poll)?;
        self.design.reconstruct(rhs, &coefficients, team)
    }
    fn apply(
        &self,
        reduced: &DenseMatrix,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<DenseMatrix> {
        let fitted = self.design.multiply(reduced, team)?;
        let residual = self.project_eliminated(&fitted, team, poll)?;
        let mut result = self.design.transpose_weighted(&residual, team)?;
        zero_effect(self.design, self.eliminated, &mut result);
        Ok(result)
    }
    fn precondition(
        &self,
        rhs: &DenseMatrix,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<DenseMatrix> {
        self.blocks
            .diagonal_solve(self.design, self.eliminated, rhs, team, poll)
    }
}
struct Recurrences {
    x: DenseMatrix,
    r: DenseMatrix,
    p: DenseMatrix,
    rho: Vec<f64>,
    targets: Vec<f64>,
    active: Vec<bool>,
    progress: Vec<Attempt>,
}
impl Recurrences {
    fn new(x: DenseMatrix, r: DenseMatrix, p: DenseMatrix, targets: Vec<f64>) -> Result<Self> {
        let mut rho = reserved(x.ncols())?;
        let mut active = reserved(x.ncols())?;
        let mut progress = reserved(x.ncols())?;
        for (column, &target) in targets.iter().enumerate() {
            let value = dot_column(&r, &p, column);
            if !value.is_finite() || value < 0. {
                return Err(numerical(
                    "Schur preconditioned norm is not finite and nonnegative",
                ));
            }
            let converged = value.sqrt() <= target;
            rho.push(value);
            active.push(!converged);
            progress.push(Attempt {
                refinements: 0,
                backend: Backend::SchurPcg,
                iterations: 0,
                stop: if converged {
                    StopReason::Converged
                } else {
                    StopReason::BudgetExhausted
                },
            });
        }
        let mut result = Self {
            x,
            r,
            p,
            rho,
            targets,
            active,
            progress,
        };
        result.mask_finished();
        Ok(result)
    }
    fn advance(&mut self, applied: &DenseMatrix) {
        for column in 0..self.x.ncols() {
            if !self.active[column] {
                continue;
            }
            self.progress[column].iterations += 1;
            let curvature = dot_column(&self.p, applied, column);
            let alpha = self.rho[column] / curvature;
            if !curvature.is_finite() || curvature <= 0. || !alpha.is_finite() {
                self.finish(column, StopReason::NumericalBreakdown);
                continue;
            }
            for row in 0..self.x.nrows() {
                let x = self.x[(row, column)] + alpha * self.p[(row, column)];
                let r = self.r[(row, column)] - alpha * applied[(row, column)];
                if !x.is_finite() || !r.is_finite() {
                    self.finish(column, StopReason::NumericalBreakdown);
                    break;
                }
                self.x[(row, column)] = x;
                self.r[(row, column)] = r;
            }
        }
    }
    fn redirect(&mut self, z: DenseMatrix) {
        for column in 0..self.x.ncols() {
            if !self.active[column] {
                continue;
            }
            let next = dot_column(&self.r, &z, column);
            if !next.is_finite() || next < 0. {
                self.finish(column, StopReason::NumericalBreakdown);
                continue;
            }
            if next.sqrt() <= self.targets[column] {
                self.finish(column, StopReason::Converged);
                continue;
            }
            let beta = next / self.rho[column];
            if !beta.is_finite() {
                self.finish(column, StopReason::NumericalBreakdown);
                continue;
            }
            for row in 0..self.p.nrows() {
                let direction = z[(row, column)] + beta * self.p[(row, column)];
                if !direction.is_finite() {
                    self.finish(column, StopReason::NumericalBreakdown);
                    break;
                }
                self.p[(row, column)] = direction;
            }
            self.rho[column] = next;
        }
        self.mask_finished();
    }
    fn fail_active(&mut self, charge_iteration: bool) {
        for column in 0..self.active.len() {
            if self.active[column] {
                self.progress[column].iterations += usize::from(charge_iteration);
                self.finish(column, StopReason::NumericalBreakdown);
            }
        }
        self.mask_finished();
    }
    fn finish(&mut self, column: usize, stop: StopReason) {
        self.active[column] = false;
        self.progress[column].stop = stop;
    }
    fn mask_finished(&mut self) {
        for column in 0..self.p.ncols() {
            if !self.active[column] {
                for row in 0..self.p.nrows() {
                    self.p[(row, column)] = 0.;
                }
            }
        }
    }
}
fn dot_column(left: &DenseMatrix, right: &DenseMatrix, column: usize) -> f64 {
    compensated_sum((0..left.nrows()).map(|row| left[(row, column)] * right[(row, column)]))
}
fn zero_effect(design: &Design<'_>, effect: usize, values: &mut DenseMatrix) {
    let layout = &design.layout().effects[effect];
    for row in layout.offset..layout.offset + layout.levels * layout.channels {
        for column in 0..values.ncols() {
            values[(row, column)] = 0.;
        }
    }
}
fn copy_matrix(matrix: &DenseMatrix) -> Result<DenseMatrix> {
    let mut data = reserved(matrix.as_slice().len())?;
    data.extend_from_slice(matrix.as_slice());
    DenseMatrix::from_row_major(matrix.nrows(), matrix.ncols(), data)
}
fn subtract(target: &mut DenseMatrix, values: &DenseMatrix) {
    for (left, &right) in target.as_mut_slice().iter_mut().zip(values.as_slice()) {
        *left -= right;
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_nonrepresentable_update_retains_work_and_other_rhs_completion() {
        let matrix = |first| DenseMatrix::from_columns(&[vec![first], vec![0.]]).unwrap();
        let mut state = Recurrences::new(
            matrix(f64::MAX * 0.75),
            matrix(1.),
            matrix(1.),
            vec![1e-10; 2],
        )
        .unwrap();
        state.advance(&matrix(1e-308));
        assert_eq!(state.progress[0].stop, StopReason::NumericalBreakdown);
        assert_eq!(state.progress[0].iterations, 1);
        assert!(!state.active[0]);
        assert!(state.x.as_slice().iter().all(|x| x.is_finite()));
        assert_eq!(state.progress[1].stop, StopReason::Converged);
        assert_eq!(state.progress[1].iterations, 0);
    }
}
