//! Accepted full-system information actions through the sole shared WLS pipeline.
use super::{anchor::Anchor, certificate, filled, invalid, numerical};
use crate::{
    data::{ColumnRecipe, CompiledColumns, DenseMatrix, Weights},
    error::{ErrorCode, FeregError, Result},
    fe::{Backend, Design, Engine},
    ppml::{control::Controls, irls},
    spec::{Estimator, Options},
    threads::{compensated_sum, ThreadTeam},
    wls::{self, AbsorbedControls, FrozenBasis, Phase, Policy, Work},
};
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Diagnostics {
    pub solves: usize,
    pub raw_recoveries: usize,
    pub maximum_score: f64,
    pub maximum_reconstruction: f64,
    pub minimum_level_information: f64,
    pub maximum_fe_leverage_lower_bound: f64,
    pub work: Work,
}
pub struct Information<'frame, 'data> {
    fit: &'frame irls::Fit<'data>,
    engine: Engine<'frame, 'frame>,
    team: &'frame ThreadTeam,
    basis: FrozenBasis,
    anchor: Anchor,
    tolerance: f64,
    raw: Vec<f64>,
    finite_slots: Vec<usize>,
    candidate: Vec<f64>,
    prediction: Vec<f64>,
    residual: Vec<f64>,
    diagnostics: Diagnostics,
}
/// The callback cannot transfer a numerical frame beyond its immutable accepted
/// Hessian. This is a linear-algebra component, not a full correction request.
pub fn with_information<T>(
    fit: &irls::Fit<'_>,
    columns: &CompiledColumns,
    team: &ThreadTeam,
    interrupt: &mut dyn FnMut() -> Result<()>,
    run: impl FnOnce(&mut Information<'_, '_>, &mut dyn FnMut() -> Result<()>) -> Result<T>,
) -> Result<T> {
    interrupt()?;
    let problem = fit.problem();
    wls::reporting::validate(columns, problem.raw, &fit.linear.within, &fit.linear.finite)?;
    if columns.roles != problem.roles
        || columns.implicit_constant
        || !columns.structural_intercept
        || columns
            .constant_slot
            .and_then(|slot| columns.slots[slot].generated)
            .and_then(|column| columns.recipes.get(column))
            != Some(&ColumnRecipe::Intercept)
        || !columns.matches(problem.sample, problem.raw)?
        || columns
            .recipes
            .iter()
            .any(|r| !matches!(r, ColumnRecipe::Continuous { .. } | ColumnRecipe::Intercept))
    {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "bias information requires matching ordinary numeric columns",
        ));
    }
    let design = Design::new(problem.sample, Weights::Provided(&fit.hessian_weights))?;
    let anchor = Anchor::new(design.topology())?;
    let controls = Controls::new(&fit.options())?;
    let options = Options {
        tolerance: controls.strict_tolerance(),
        ..fit.options()
    };
    let engine = Engine::new(&design, Estimator::Ppml, &options, team)?;
    let basis = FrozenBasis::from_fit(&fit.linear.finite)?;
    let dimension = anchor
        .levels()
        .checked_add(basis.kept().len())
        .ok_or_else(|| invalid("bias dimension overflows"))?;
    let mut finite_slots = filled(basis.kept().len(), 0)?;
    for (j, &column) in basis.kept().iter().enumerate() {
        finite_slots[j] = columns
            .slots
            .iter()
            .position(|s| s.generated == Some(column))
            .ok_or_else(|| invalid("bias finite slot is absent"))?;
    }
    let mut information = Information {
        fit,
        engine,
        team,
        basis,
        anchor,
        tolerance: fit.options().tolerance,
        raw: filled(dimension, 0.)?,
        finite_slots,
        candidate: filled(dimension, 0.)?,
        prediction: filled(problem.sample.n_active(), 0.)?,
        residual: filled(problem.sample.n_active(), 0.)?,
        diagnostics: Diagnostics::default(),
    };
    information.level_information()?;
    information
        .recover_raw(interrupt)
        .map_err(|e| super::context(e, "raw", None, None))?;
    run(&mut information, interrupt)
}
impl Information<'_, '_> {
    pub fn raw(&self) -> &[f64] {
        &self.raw
    }
    pub fn dimension(&self) -> usize {
        self.raw.len()
    }
    pub fn first_levels(&self) -> usize {
        self.anchor.first
    }
    pub fn second_levels(&self) -> usize {
        self.anchor.second
    }
    pub fn finite_slots(&self) -> &[usize] {
        &self.finite_slots
    }
    pub fn requested(&self) -> crate::spec::Solver {
        self.engine.plan().requested()
    }
    pub fn selected(&self) -> Backend {
        self.engine.plan().selected()
    }
    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics
    }
    /// A solve publishes both destinations only after fresh original-coordinate
    /// normal equations, frozen rank and reconstruction have all passed.
    pub fn solve(
        &mut self,
        response: &[f64],
        prediction: &mut [f64],
        coefficients: Option<&mut [f64]>,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        interrupt()?;
        if response.len() != self.prediction.len()
            || prediction.len() != response.len()
            || coefficients
                .as_ref()
                .is_some_and(|v| v.len() != self.dimension())
            || response.iter().any(|v| !v.is_finite())
        {
            return Err(invalid(
                "bias information solve dimensions or response are invalid",
            ));
        }
        self.diagnostics.solves = self
            .diagnostics
            .solves
            .checked_add(1)
            .ok_or_else(|| numerical("bias solve count overflows"))?;
        let problem = self.fit.problem();
        let linear = wls::solve_absorbed_recorded(
            &mut self.engine,
            problem.raw,
            response,
            problem.roles,
            AbsorbedControls {
                finite: Policy::default(),
                basis: Some(&self.basis),
            },
            &mut self.diagnostics.work,
            self.team,
        )?;
        if !self.basis.matches(&linear.finite) {
            return Err(numerical("bias information solve changed accepted rank"));
        }
        self.anchor
            .apply(&linear.absorbed_coefficients, &mut self.candidate)?;
        for (j, &column) in self.basis.kept().iter().enumerate() {
            self.candidate[self.anchor.levels() + j] = linear.finite.coefficients[column];
        }
        certificate::prediction(
            problem.sample,
            problem.raw,
            self.basis.kept(),
            self.anchor.first,
            self.anchor.levels(),
            &self.candidate,
            &mut self.prediction,
        )?;
        for (row, value) in self.residual.iter_mut().enumerate() {
            *value = response[row] - self.prediction[row];
        }
        let error = certificate::reconstruction(&linear.residuals, &self.residual, self.tolerance)?;
        let score = certificate::scores(
            problem.sample,
            problem.raw,
            self.basis.kept(),
            &self.fit.hessian_weights,
            response,
            &self.prediction,
            self.tolerance,
        )?;
        self.diagnostics.maximum_score = self.diagnostics.maximum_score.max(score);
        self.diagnostics.maximum_reconstruction =
            self.diagnostics.maximum_reconstruction.max(error);
        interrupt()?;
        prediction.copy_from_slice(&self.prediction);
        if let Some(destination) = coefficients {
            destination.copy_from_slice(&self.candidate);
        }
        Ok(())
    }
    /// A last-group contrast inferred by linearity still needs fresh original
    /// normal equations; subtraction must not hide an uncertified group.
    pub(super) fn certify_combination(
        &mut self,
        response: &[f64],
        coefficients: &[f64],
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        interrupt()?;
        let problem = self.fit.problem();
        certificate::prediction(
            problem.sample,
            problem.raw,
            self.basis.kept(),
            self.anchor.first,
            self.anchor.levels(),
            coefficients,
            &mut self.prediction,
        )?;
        let score = certificate::scores(
            problem.sample,
            problem.raw,
            self.basis.kept(),
            &self.fit.hessian_weights,
            response,
            &self.prediction,
            self.tolerance,
        )?;
        for (row, residual) in self.residual.iter_mut().enumerate() {
            *residual = self.prediction[row] + (response[row] - self.prediction[row]);
        }
        let error = certificate::reconstruction(response, &self.residual, self.tolerance)?;
        self.diagnostics.maximum_score = self.diagnostics.maximum_score.max(score);
        self.diagnostics.maximum_reconstruction =
            self.diagnostics.maximum_reconstruction.max(error);
        interrupt()
    }
    fn level_information(&mut self) -> Result<()> {
        let topology = self
            .fit
            .problem()
            .sample
            .topology
            .as_ref()
            .ok_or_else(|| invalid("bias topology is absent"))?;
        self.diagnostics.minimum_level_information = f64::INFINITY;
        for effect in 0..2 {
            let partition = topology.effect_partition(effect)?;
            for level in 0..partition.n_levels() {
                let rows = partition.membership().members(level)?;
                let information =
                    compensated_sum(rows.iter().map(|&r| self.fit.hessian_weights[r]));
                if !information.is_finite() || information <= 0. {
                    return Err(numerical("bias level information is not representable"));
                }
                self.diagnostics.minimum_level_information =
                    self.diagnostics.minimum_level_information.min(information);
                for &row in rows {
                    self.diagnostics.maximum_fe_leverage_lower_bound = self
                        .diagnostics
                        .maximum_fe_leverage_lower_bound
                        .max(self.fit.hessian_weights[row] / information);
                }
            }
        }
        Ok(())
    }
    fn recover_raw(&mut self, interrupt: &mut dyn FnMut() -> Result<()>) -> Result<()> {
        interrupt()?;
        let problem = self.fit.problem();
        let offset = irls::input::offset(problem.sample)?;
        let mut target = DenseMatrix::zeros(problem.sample.n_active(), 1)?;
        for row in 0..target.nrows() {
            target[(row, 0)] = self.fit.point.eta[row]
                - offset.as_deref().map_or(0., |v| v[row])
                - problem.raw.row_combination(row, &self.fit.point.finite, 0.);
        }
        let projection = wls::recorded_projection(
            &mut self.engine,
            &target,
            Phase::Recovery,
            &mut self.diagnostics.work,
            self.team,
        )?;
        self.diagnostics.raw_recoveries += 1;
        self.anchor.apply(&projection.coefficients, &mut self.raw)?;
        certificate::prediction(
            problem.sample,
            problem.raw,
            &[],
            self.anchor.first,
            self.anchor.levels(),
            &self.raw[..self.anchor.levels()],
            &mut self.prediction,
        )?;
        let error =
            certificate::reconstruction(target.as_slice(), &self.prediction, self.tolerance)?;
        let score = certificate::scores(
            problem.sample,
            problem.raw,
            &[],
            &self.fit.hessian_weights,
            target.as_slice(),
            &self.prediction,
            self.tolerance,
        )?;
        for (j, &column) in self.basis.kept().iter().enumerate() {
            self.raw[self.anchor.levels() + j] = self.fit.point.finite[column];
        }
        self.diagnostics.maximum_reconstruction = error;
        self.diagnostics.maximum_score = score;
        interrupt()
    }
}
