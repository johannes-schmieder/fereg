//! PPML-only augmented LSMR over finite controls and categorical intercepts.
//!
//! This module deliberately bypasses the one-shot weighted-linear projection
//! route only for an explicitly admitted development PPML fit.  It never
//! participates in OLS or public PPML routing.  Exact normal-equation moments
//! are recomputed after every recurrence stop; LSMR diagnostics alone never
//! authorize an IRLS step.

use std::time::Instant;

use rayon::prelude::*;
use schwarz_precond::{mlsmr, LsmrStopReason, MlsmrOptions, Operator, SolveError, Staleness};
use within::{Effect, Preconditioner, PreconditionerConfig, Solver};

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::prepared_fit_view::PreparedOlsFitView;

use super::PpmlIterationWorkspace;

/// Numeric preconditioner lifecycle selected by the private PPML experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoricalLsmrPolicy {
    Diagonal,
    Reuse,
    Rebuild,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct JointLsmrCertificate {
    pub(crate) finite_absolute_infinity_norm: f64,
    pub(crate) fixed_effect_absolute_infinity_norm: f64,
    pub(crate) relative_infinity_norm: f64,
    pub(crate) relative_two_norm: f64,
    pub(crate) passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct JointLsmrReport {
    pub(crate) iterations: usize,
    pub(crate) effective_tolerance: f64,
    pub(crate) recurrence_residual: f64,
    pub(crate) stop_reason: LsmrStopReason,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) rebuilt_preconditioner: bool,
    pub(crate) continued_after_certificate: bool,
    pub(crate) requested_schwarz: bool,
    pub(crate) used_schwarz: bool,
    pub(crate) warm_start_attempted: bool,
    pub(crate) certificate: JointLsmrCertificate,
}

/// Fit-lifetime state for the ordinary (non-strict) IRLS Newton correction.
pub(crate) struct ReusableCategoricalJointLsmr {
    active_design: DenseMatrix,
    active_slots: Vec<usize>,
    effect_offsets: Vec<usize>,
    coefficient_effect: Vec<usize>,
    coefficient_level: Vec<usize>,
    sqrt_weights: Vec<f64>,
    factor_weights: Vec<f64>,
    rhs: Vec<f64>,
    target: Vec<f64>,
    delta_eta: Vec<f64>,
    residual: Vec<f64>,
    warm_start: Vec<f64>,
    finite_inverse: Vec<f64>,
    fe_inverse: Vec<f64>,
    warm_start_valid: bool,
    preconditioner: Option<Preconditioner>,
    previous_iterations: usize,
    policy: CategoricalLsmrPolicy,
}

impl ReusableCategoricalJointLsmr {
    pub(crate) fn new<P: PreparedOlsFitView + Sync + ?Sized>(
        prepared: &P,
        active_generated: &[usize],
        fe_design: &FeDesign,
        policy: CategoricalLsmrPolicy,
    ) -> Result<Self> {
        let rows = fe_design.nobs();
        if rows != prepared.fit_sample().n_active()
            || fe_design
                .effects()
                .iter()
                .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
        {
            return Err(invalid_argument(
                "joint PPML LSMR requires an aligned categorical intercept-only design",
            ));
        }

        let active_design = build_active_design(prepared, active_generated)?;
        let mut active_slots = Vec::new();
        active_slots
            .try_reserve_exact(active_generated.len())
            .map_err(|_| allocation("unable to reserve joint PPML active coefficient slots"))?;
        for &generated in active_generated {
            let slot = prepared
                .fit_model()
                .coefficients
                .iter()
                .position(|coefficient| coefficient.generated_column_index == Some(generated))
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "joint PPML active column has no coefficient slot",
                    )
                })?;
            active_slots.push(slot);
        }

        let mut effect_offsets = Vec::new();
        effect_offsets
            .try_reserve_exact(fe_design.effects().len() + 1)
            .map_err(|_| allocation("unable to reserve joint PPML FE offsets"))?;
        effect_offsets.push(0_usize);
        for effect in fe_design.effects() {
            effect_offsets.push(
                effect_offsets
                    .last()
                    .copied()
                    .unwrap_or(0)
                    .checked_add(effect.n_levels())
                    .ok_or_else(|| allocation("joint PPML FE coefficient count overflows"))?,
            );
        }
        let fe_coefficients = effect_offsets.last().copied().unwrap_or(0);
        let mut coefficient_effect = Vec::new();
        let mut coefficient_level = Vec::new();
        coefficient_effect
            .try_reserve_exact(fe_coefficients)
            .map_err(|_| allocation("unable to reserve joint PPML FE effect map"))?;
        coefficient_level
            .try_reserve_exact(fe_coefficients)
            .map_err(|_| allocation("unable to reserve joint PPML FE level map"))?;
        for (effect_index, effect) in fe_design.effects().iter().enumerate() {
            for level in 0..effect.n_levels() {
                coefficient_effect.push(effect_index);
                coefficient_level.push(level);
            }
        }
        let columns = active_generated
            .len()
            .checked_add(fe_coefficients)
            .ok_or_else(|| allocation("joint PPML augmented width overflows"))?;
        Ok(Self {
            active_design,
            active_slots,
            effect_offsets,
            coefficient_effect,
            coefficient_level,
            sqrt_weights: fallible_filled(rows, 0.0, "joint PPML square-root weights")?,
            factor_weights: fallible_filled(rows, 0.0, "joint PPML factor weights")?,
            rhs: fallible_filled(rows, 0.0, "joint PPML LSMR right-hand side")?,
            target: fallible_filled(rows, 0.0, "joint PPML Newton target")?,
            delta_eta: fallible_filled(rows, 0.0, "joint PPML predictor correction")?,
            residual: fallible_filled(rows, 0.0, "joint PPML correction residual")?,
            warm_start: fallible_filled(columns, 0.0, "joint PPML LSMR warm state")?,
            finite_inverse: fallible_filled(
                active_generated.len(),
                0.0,
                "joint PPML finite diagonal preconditioner",
            )?,
            fe_inverse: fallible_filled(
                fe_coefficients,
                0.0,
                "joint PPML FE diagonal preconditioner",
            )?,
            warm_start_valid: false,
            preconditioner: None,
            previous_iterations: 0,
            policy,
        })
    }

    pub(crate) fn solve(
        &mut self,
        fe_design: &FeDesign,
        outcome: &[f64],
        mean: &[f64],
        hessian_weights: &[f64],
        tolerance: f64,
        max_iterations: usize,
    ) -> Result<JointLsmrReport> {
        self.validate_frame(
            fe_design,
            outcome,
            mean,
            hessian_weights,
            tolerance,
            max_iterations,
        )?;

        for row in 0..outcome.len() {
            self.sqrt_weights[row] = hessian_weights[row].sqrt();
            self.target[row] = if outcome[row] == 0.0 {
                -1.0
            } else {
                outcome[row] / mean[row] - 1.0
            };
            self.rhs[row] = self.sqrt_weights[row] * self.target[row];
            if !self.sqrt_weights[row].is_finite()
                || !self.target[row].is_finite()
                || !self.rhs[row].is_finite()
            {
                return Err(nonconvergence(
                    "joint PPML Newton target is not finitely representable",
                ));
            }
        }

        let setup_started = Instant::now();
        let warm_start_attempted = self.warm_start_valid;
        let (mut use_schwarz, mut rebuilt) =
            self.prepare_preconditioner(fe_design, hessian_weights)?;
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        let solve_started = Instant::now();
        let staleness = Staleness::try_new(4, 0.75).map_err(|error| {
            FeregError::new(
                ErrorCode::Internal,
                format!("invalid joint PPML LSMR staleness policy: {error}"),
            )
        })?;
        let use_escalation = self.policy == CategoricalLsmrPolicy::Reuse && use_schwarz;
        let first = {
            let block_preconditioner = self.block_preconditioner(use_schwarz)?;
            self.run_once(
                fe_design,
                &block_preconditioner,
                tolerance,
                max_iterations,
                self.warm_start_valid.then_some(self.warm_start.as_slice()),
                use_escalation.then_some(&staleness as _),
                None,
            )?
        };
        let mut result = first;
        let mut iterations = result.iterations;
        let mut effective_tolerance = tolerance;
        if result.stop_reason == LsmrStopReason::Escalated {
            self.rebuild_schwarz(fe_design, hessian_weights)?;
            rebuilt = true;
            use_schwarz = true;
            let continued = {
                let block_preconditioner = self.block_preconditioner(use_schwarz)?;
                self.run_once(
                    fe_design,
                    &block_preconditioner,
                    tolerance,
                    max_iterations,
                    Some(&result.x),
                    None,
                    Some(4),
                )?
            };
            iterations = iterations
                .checked_add(continued.iterations)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "joint PPML LSMR iteration count overflowed",
                    )
                })?;
            result = continued;
        }

        let mut continued_after_certificate = false;
        let mut certificate =
            self.finish_and_certify(fe_design, hessian_weights, &result.x, tolerance)?;
        if !certificate.passed {
            continued_after_certificate = true;
            if !rebuilt && self.policy != CategoricalLsmrPolicy::Diagonal {
                self.rebuild_schwarz(fe_design, hessian_weights)?;
                rebuilt = true;
                use_schwarz = true;
            }
            effective_tolerance = (tolerance * 0.01).max(1e-14);
            let continued = {
                let block_preconditioner = self.block_preconditioner(use_schwarz)?;
                self.run_once(
                    fe_design,
                    &block_preconditioner,
                    effective_tolerance,
                    max_iterations,
                    Some(&result.x),
                    None,
                    Some(4),
                )?
            };
            iterations = iterations
                .checked_add(continued.iterations)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "joint PPML LSMR iteration count overflowed",
                    )
                })?;
            result = continued;
            certificate =
                self.finish_and_certify(fe_design, hessian_weights, &result.x, tolerance)?;
        }
        let solve_seconds = solve_started.elapsed().as_secs_f64();
        if !result.converged || !certificate.passed {
            return Err(nonconvergence(
                "joint PPML LSMR correction did not pass its exact normal-equation certificate",
            ));
        }

        self.warm_start.copy_from_slice(&result.x);
        self.warm_start_valid = true;
        self.previous_iterations = result.iterations;

        Ok(JointLsmrReport {
            iterations,
            effective_tolerance,
            recurrence_residual: result.normal_eq_residual,
            stop_reason: result.stop_reason,
            setup_seconds,
            solve_seconds,
            rebuilt_preconditioner: rebuilt,
            continued_after_certificate,
            requested_schwarz: self.policy != CategoricalLsmrPolicy::Diagonal,
            used_schwarz: use_schwarz,
            warm_start_attempted,
            certificate,
        })
    }

    pub(crate) fn fill_candidate(&self, workspace: &mut PpmlIterationWorkspace) -> Result<()> {
        workspace.fill_incremental_candidate(
            &self.delta_eta,
            &self.active_slots,
            &self.warm_start[..self.active_slots.len()],
        )
    }

    pub(crate) fn active_rank(&self) -> usize {
        self.active_slots.len()
    }

    pub(crate) fn active_slots(&self) -> &[usize] {
        &self.active_slots
    }

    /// Scale the speculative recurrence state to the actually accepted IRLS
    /// step. A strict canonical synchronization discards it instead.
    pub(crate) fn accept_step(&mut self, step: f64) {
        if self.warm_start_valid {
            for value in &mut self.warm_start {
                *value *= step;
            }
        }
    }

    pub(crate) fn reset_warm_start(&mut self) {
        self.warm_start.fill(0.0);
        self.warm_start_valid = false;
    }

    #[allow(clippy::too_many_arguments)]
    fn run_once(
        &self,
        fe_design: &FeDesign,
        preconditioner: &BlockPreconditioner,
        tolerance: f64,
        max_iterations: usize,
        warm_start: Option<&[f64]>,
        escalation: Option<&dyn schwarz_precond::EscalationPolicy>,
        local_size: Option<usize>,
    ) -> Result<schwarz_precond::LsmrResult> {
        let operator = AugmentedCategoricalOperator::new(
            fe_design,
            &self.active_design,
            &self.effect_offsets,
            &self.coefficient_effect,
            &self.coefficient_level,
            &self.sqrt_weights,
        )?;
        mlsmr(
            &operator,
            &self.rhs,
            preconditioner,
            tolerance,
            max_iterations,
            MlsmrOptions {
                warm_start,
                escalation,
                local_size,
            },
        )
        .map_err(lsmr_error)
    }

    fn validate_frame(
        &self,
        fe_design: &FeDesign,
        outcome: &[f64],
        mean: &[f64],
        hessian_weights: &[f64],
        tolerance: f64,
        max_iterations: usize,
    ) -> Result<()> {
        let rows = fe_design.nobs();
        if rows == 0
            || self.active_design.nrows() != rows
            || self.sqrt_weights.len() != rows
            || [outcome.len(), mean.len(), hessian_weights.len()]
                .iter()
                .any(|length| *length != rows)
            || !tolerance.is_finite()
            || tolerance <= 0.0
            || max_iterations == 0
        {
            return Err(invalid_argument(
                "joint PPML LSMR frame dimensions or controls are invalid",
            ));
        }
        if outcome
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
            || mean.iter().any(|value| !value.is_finite() || *value <= 0.0)
            || hessian_weights
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(nonconvergence(
                "joint PPML LSMR frame contains an invalid outcome, mean, or weight",
            ));
        }
        Ok(())
    }

    fn prepare_preconditioner(
        &mut self,
        fe_design: &FeDesign,
        weights: &[f64],
    ) -> Result<(bool, bool)> {
        self.refresh_diagonal_preconditioner(fe_design, weights)?;
        let use_schwarz =
            self.policy != CategoricalLsmrPolicy::Diagonal && fe_design.total_coefficients() >= 256;
        if !use_schwarz {
            return Ok((false, false));
        }
        let weight_change = maximum_weight_ratio(weights, &self.factor_weights);
        let rebuild = self.preconditioner.is_none()
            || self.policy == CategoricalLsmrPolicy::Rebuild
            || (weight_change > 4.0 && self.previous_iterations > 8);
        if rebuild {
            self.rebuild_schwarz(fe_design, weights)?;
        }
        Ok((true, rebuild))
    }

    fn rebuild_schwarz(&mut self, fe_design: &FeDesign, weights: &[f64]) -> Result<()> {
        let effects = within_effects(fe_design)?;
        let solver = Solver::new(
            effects,
            Some(fallible_copy(weights, "joint PPML Schwarz weights")?),
            PreconditionerConfig::default(),
        )
        .map_err(|error| {
            FeregError::new(
                ErrorCode::InvalidData,
                format!("unable to build joint PPML Schwarz preconditioner: {error}"),
            )
        })?;
        self.preconditioner = solver.preconditioner().cloned();
        self.factor_weights.copy_from_slice(weights);
        Ok(())
    }

    fn refresh_diagonal_preconditioner(
        &mut self,
        fe_design: &FeDesign,
        weights: &[f64],
    ) -> Result<()> {
        self.finite_inverse
            .par_iter_mut()
            .enumerate()
            .for_each(|(column, inverse)| {
                let mut diagonal = 0.0_f64;
                for row in 0..self.active_design.nrows() {
                    let value = self.active_design[(row, column)];
                    diagonal = (weights[row] * value).mul_add(value, diagonal);
                }
                *inverse = if diagonal > 0.0 { 1.0 / diagonal } else { 0.0 };
            });
        if self.finite_inverse.iter().any(|value| !value.is_finite()) {
            return Err(nonconvergence(
                "joint PPML finite diagonal preconditioner is nonfinite",
            ));
        }
        fill_fe_diagonal_inverse(fe_design, weights, &mut self.fe_inverse)
    }

    fn block_preconditioner(&self, use_schwarz: bool) -> Result<BlockPreconditioner<'_>> {
        let fe = if use_schwarz {
            FePreconditioner::Schwarz(self.preconditioner.as_ref().ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "joint PPML Schwarz route has no retained preconditioner",
                )
            })?)
        } else {
            FePreconditioner::Diagonal(&self.fe_inverse)
        };
        Ok(BlockPreconditioner {
            finite_inverse: &self.finite_inverse,
            fe,
        })
    }

    fn finish_and_certify(
        &mut self,
        fe_design: &FeDesign,
        weights: &[f64],
        solution: &[f64],
        tolerance: f64,
    ) -> Result<JointLsmrCertificate> {
        let operator = AugmentedCategoricalOperator::new(
            fe_design,
            &self.active_design,
            &self.effect_offsets,
            &self.coefficient_effect,
            &self.coefficient_level,
            &self.sqrt_weights,
        )?;
        operator.fill_unweighted_predictor(solution, &mut self.delta_eta)?;
        for row in 0..self.residual.len() {
            self.residual[row] = self.target[row] - self.delta_eta[row];
        }
        certify_joint(
            fe_design,
            &self.active_design,
            weights,
            &self.target,
            &self.residual,
            tolerance,
        )
    }
}

fn build_active_design<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    active_generated: &[usize],
) -> Result<DenseMatrix> {
    let rows = prepared.fit_sample().n_active();
    let columns = active_generated.len();
    let elements = rows
        .checked_mul(columns)
        .ok_or_else(|| allocation("joint PPML active design dimensions overflow"))?;
    let mut values = fallible_filled(elements, 0.0, "joint PPML active design")?;
    values
        .par_chunks_mut(columns.max(1))
        .enumerate()
        .try_for_each(|(row, destination)| -> Result<()> {
            if columns == 0 {
                return Ok(());
            }
            for (active, &generated) in active_generated.iter().enumerate() {
                destination[active] = prepared.fit_column_value(generated, row)?;
            }
            Ok(())
        })?;
    DenseMatrix::from_row_major(rows, columns, values)
}

struct AugmentedCategoricalOperator<'a> {
    fe_design: &'a FeDesign,
    active_design: &'a DenseMatrix,
    effect_offsets: &'a [usize],
    coefficient_effect: &'a [usize],
    coefficient_level: &'a [usize],
    sqrt_weights: &'a [f64],
}

impl<'a> AugmentedCategoricalOperator<'a> {
    fn new(
        fe_design: &'a FeDesign,
        active_design: &'a DenseMatrix,
        effect_offsets: &'a [usize],
        coefficient_effect: &'a [usize],
        coefficient_level: &'a [usize],
        sqrt_weights: &'a [f64],
    ) -> Result<Self> {
        if fe_design.nobs() != active_design.nrows()
            || sqrt_weights.len() != fe_design.nobs()
            || effect_offsets.len() != fe_design.effects().len() + 1
            || coefficient_effect.len() != fe_design.total_coefficients()
            || coefficient_level.len() != fe_design.total_coefficients()
        {
            return Err(invalid_argument(
                "joint PPML augmented operator dimensions are inconsistent",
            ));
        }
        Ok(Self {
            fe_design,
            active_design,
            effect_offsets,
            coefficient_effect,
            coefficient_level,
            sqrt_weights,
        })
    }

    fn fill_unweighted_predictor(&self, x: &[f64], output: &mut [f64]) -> Result<()> {
        if x.len() != self.ncols() || output.len() != self.nrows() {
            return Err(invalid_argument(
                "joint PPML predictor dimensions are inconsistent",
            ));
        }
        let finite = self.active_design.ncols();
        output.par_iter_mut().enumerate().for_each(|(row, value)| {
            let mut fitted = 0.0_f64;
            for column in 0..finite {
                fitted = self.active_design[(row, column)].mul_add(x[column], fitted);
            }
            for (effect_index, effect) in self.fe_design.effects().iter().enumerate() {
                let level = effect.codes()[row] as usize;
                fitted += x[finite + self.effect_offsets[effect_index] + level];
            }
            *value = fitted;
        });
        if output.iter().any(|value| !value.is_finite()) {
            return Err(nonconvergence(
                "joint PPML augmented operator produced a nonfinite predictor",
            ));
        }
        Ok(())
    }
}

impl Operator for AugmentedCategoricalOperator<'_> {
    fn nrows(&self) -> usize {
        self.fe_design.nobs()
    }

    fn ncols(&self) -> usize {
        self.active_design
            .ncols()
            .saturating_add(self.fe_design.total_coefficients())
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        if x.len() != self.ncols() || y.len() != self.nrows() {
            return Err(solve_shape_error("apply"));
        }
        let finite = self.active_design.ncols();
        y.par_iter_mut().enumerate().for_each(|(row, value)| {
            let mut fitted = 0.0_f64;
            for column in 0..finite {
                fitted = self.active_design[(row, column)].mul_add(x[column], fitted);
            }
            for (effect_index, effect) in self.fe_design.effects().iter().enumerate() {
                let level = effect.codes()[row] as usize;
                fitted += x[finite + self.effect_offsets[effect_index] + level];
            }
            *value = self.sqrt_weights[row] * fitted;
        });
        Ok(())
    }

    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        if x.len() != self.nrows() || y.len() != self.ncols() {
            return Err(solve_shape_error("apply_adjoint"));
        }
        let finite = self.active_design.ncols();
        let (finite_output, fe_output) = y.split_at_mut(finite);
        finite_output
            .par_iter_mut()
            .enumerate()
            .for_each(|(column, output)| {
                let mut sum = 0.0_f64;
                for row in 0..self.nrows() {
                    sum = (self.sqrt_weights[row] * self.active_design[(row, column)])
                        .mul_add(x[row], sum);
                }
                *output = sum;
            });
        fe_output
            .par_iter_mut()
            .enumerate()
            .for_each(|(coefficient, output)| {
                let effect = &self.fe_design.effects()[self.coefficient_effect[coefficient]];
                let level = self.coefficient_level[coefficient];
                let mut sum = 0.0_f64;
                for &row in effect.level_index().members(level) {
                    sum = self.sqrt_weights[row].mul_add(x[row], sum);
                }
                *output = sum;
            });
        Ok(())
    }
}

enum FePreconditioner<'a> {
    Diagonal(&'a [f64]),
    Schwarz(&'a Preconditioner),
}

struct BlockPreconditioner<'a> {
    finite_inverse: &'a [f64],
    fe: FePreconditioner<'a>,
}

impl Operator for BlockPreconditioner<'_> {
    fn nrows(&self) -> usize {
        self.finite_inverse.len()
            + match &self.fe {
                FePreconditioner::Diagonal(values) => values.len(),
                FePreconditioner::Schwarz(value) => value.nrows(),
            }
    }

    fn ncols(&self) -> usize {
        self.nrows()
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        if x.len() != self.nrows() || y.len() != self.nrows() {
            return Err(solve_shape_error("preconditioner apply"));
        }
        let finite = self.finite_inverse.len();
        for column in 0..finite {
            y[column] = self.finite_inverse[column] * x[column];
        }
        match &self.fe {
            FePreconditioner::Diagonal(inverse) => {
                for ((output, input), scale) in
                    y[finite..].iter_mut().zip(&x[finite..]).zip(*inverse)
                {
                    *output = *scale * *input;
                }
                Ok(())
            }
            FePreconditioner::Schwarz(preconditioner) => {
                Operator::apply(*preconditioner, &x[finite..], &mut y[finite..])
            }
        }
    }

    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        self.apply(x, y)
    }
}

fn fill_fe_diagonal_inverse(design: &FeDesign, weights: &[f64], values: &mut [f64]) -> Result<()> {
    if values.len() != design.total_coefficients() {
        return Err(invalid_argument(
            "joint PPML FE diagonal dimensions are inconsistent",
        ));
    }
    let mut offset = 0_usize;
    for effect in design.effects() {
        let block = &mut values[offset..offset + effect.n_levels()];
        block.par_iter_mut().enumerate().for_each(|(level, value)| {
            let mut diagonal = 0.0_f64;
            for &row in effect.level_index().members(level) {
                diagonal += weights[row];
            }
            *value = if diagonal > 0.0 { 1.0 / diagonal } else { 0.0 };
        });
        offset += effect.n_levels();
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(nonconvergence(
            "joint PPML FE diagonal preconditioner is nonfinite",
        ));
    }
    Ok(())
}

fn within_effects(design: &FeDesign) -> Result<Vec<Effect<'_>>> {
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(design.effects().len())
        .map_err(|_| allocation("unable to reserve joint PPML within effects"))?;
    for effect in design.effects() {
        effects.push(
            Effect::new(effect.codes(), true, std::iter::empty::<&[f64]>()).map_err(|error| {
                FeregError::new(
                    ErrorCode::InvalidData,
                    format!("unable to build joint PPML categorical effect: {error}"),
                )
            })?,
        );
    }
    Ok(effects)
}

fn certify_joint(
    design: &FeDesign,
    active: &DenseMatrix,
    weights: &[f64],
    target: &[f64],
    residual: &[f64],
    tolerance: f64,
) -> Result<JointLsmrCertificate> {
    let mut target_square = 0.0_f64;
    let mut residual_square = 0.0_f64;
    for row in 0..design.nobs() {
        target_square = (weights[row] * target[row]).mul_add(target[row], target_square);
        residual_square = (weights[row] * residual[row]).mul_add(residual[row], residual_square);
    }
    let reference = target_square.sqrt().max(residual_square.sqrt());
    let mut finite_absolute = 0.0_f64;
    let mut fe_absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_square = 0.0_f64;
    let mut count = 0_usize;
    for column in 0..active.ncols() {
        let mut moment = 0.0_f64;
        let mut basis_square = 0.0_f64;
        for row in 0..active.nrows() {
            let basis = active[(row, column)];
            moment = (weights[row] * residual[row]).mul_add(basis, moment);
            basis_square = (weights[row] * basis).mul_add(basis, basis_square);
        }
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative(
            moment,
            basis_square.sqrt(),
            reference,
            &mut relative_infinity,
            &mut relative_square,
            &mut count,
        );
    }
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let mut moment = 0.0_f64;
            let mut basis_square = 0.0_f64;
            for &row in effect.level_index().members(level) {
                moment += weights[row] * residual[row];
                basis_square += weights[row];
            }
            fe_absolute = fe_absolute.max(moment.abs());
            add_relative(
                moment,
                basis_square.sqrt(),
                reference,
                &mut relative_infinity,
                &mut relative_square,
                &mut count,
            );
        }
    }
    let relative_two = if count == 0 {
        0.0
    } else {
        (relative_square / count as f64).sqrt()
    };
    if !finite_absolute.is_finite()
        || !fe_absolute.is_finite()
        || !relative_infinity.is_finite()
        || !relative_two.is_finite()
    {
        return Err(nonconvergence(
            "joint PPML LSMR exact certificate is nonfinite",
        ));
    }
    Ok(JointLsmrCertificate {
        finite_absolute_infinity_norm: finite_absolute,
        fixed_effect_absolute_infinity_norm: fe_absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance && relative_two <= tolerance,
    })
}

fn add_relative(
    moment: f64,
    basis_norm: f64,
    reference: f64,
    infinity: &mut f64,
    square: &mut f64,
    count: &mut usize,
) {
    let denominator = basis_norm * reference;
    let relative = if denominator > 0.0 {
        moment.abs() / denominator
    } else if moment == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    *infinity = (*infinity).max(relative);
    *square = relative.mul_add(relative, *square);
    *count += 1;
}

fn maximum_weight_ratio(current: &[f64], factor: &[f64]) -> f64 {
    if current.len() != factor.len() || factor.iter().all(|value| *value == 0.0) {
        return f64::INFINITY;
    }
    current
        .iter()
        .zip(factor)
        .map(|(left, right)| {
            if *left > 0.0 && *right > 0.0 {
                (left / right).max(right / left)
            } else {
                f64::INFINITY
            }
        })
        .fold(1.0_f64, f64::max)
}

fn fallible_filled<T: Clone>(elements: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(message))?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_copy<T: Clone>(values: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation(message))?;
    output.extend_from_slice(values);
    Ok(output)
}

fn lsmr_error(error: SolveError) -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        format!("joint PPML modified LSMR failed: {error}"),
    )
}

fn solve_shape_error(context: &'static str) -> SolveError {
    SolveError::InvalidInput {
        context,
        message: "joint PPML augmented operator dimensions are inconsistent".to_owned(),
    }
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn nonconvergence(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn augmented_operator_obeys_adjoint_identity() {
        let left = vec![0, 0, 1, 1];
        let right = vec![0, 1, 0, 1];
        let design = FeDesign::new(
            4,
            vec![
                EffectTerm::new(left, 2, true, Vec::new()).expect("left"),
                EffectTerm::new(right, 2, true, Vec::new()).expect("right"),
            ],
        )
        .expect("design");
        let active = DenseMatrix::from_row_major(4, 1, vec![1.0, 2.0, -1.0, 0.5]).expect("active");
        let offsets = vec![0, 2, 4];
        let effects = vec![0, 0, 1, 1];
        let levels = vec![0, 1, 0, 1];
        let weights = vec![1.0, 2.0_f64.sqrt(), 0.5_f64.sqrt(), 1.5_f64.sqrt()];
        let operator = AugmentedCategoricalOperator::new(
            &design, &active, &offsets, &effects, &levels, &weights,
        )
        .expect("operator");
        let x = vec![0.25, 1.0, -0.5, 0.75, -1.25];
        let y = vec![0.3, -0.2, 0.8, 1.1];
        let mut ax = vec![0.0; operator.nrows()];
        let mut aty = vec![0.0; operator.ncols()];
        operator.apply(&x, &mut ax).expect("apply");
        operator.apply_adjoint(&y, &mut aty).expect("adjoint");
        let left_product = ax.iter().zip(&y).map(|(a, b)| a * b).sum::<f64>();
        let right_product = x.iter().zip(&aty).map(|(a, b)| a * b).sum::<f64>();
        assert!((left_product - right_product).abs() <= 1e-13);
    }

    #[test]
    fn exact_certificate_rejects_an_uncorrected_target() {
        let codes = vec![0, 0, 1, 1];
        let design = FeDesign::new(
            4,
            vec![EffectTerm::new(codes, 2, true, Vec::new()).expect("effect")],
        )
        .expect("design");
        let active = DenseMatrix::from_row_major(4, 0, Vec::new()).expect("active");
        let certificate = certify_joint(
            &design,
            &active,
            &[1.0; 4],
            &[1.0, 1.0, -1.0, -1.0],
            &[1.0, 1.0, -1.0, -1.0],
            1e-8,
        )
        .expect("certificate");
        assert!(!certificate.passed);
        assert!(certificate.relative_infinity_norm > 0.7);
    }

    #[test]
    fn joint_solver_reuses_a_certified_incremental_state_across_weight_frames() {
        let first_codes = vec![0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let second_codes = vec![0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0];
        let design = FeDesign::new(
            12,
            vec![
                EffectTerm::new(first_codes.clone(), 4, true, Vec::new()).expect("first"),
                EffectTerm::new(second_codes.clone(), 2, true, Vec::new()).expect("second"),
            ],
        )
        .expect("design");
        let x = vec![0.0, 1.0, 1.5, 0.5, 2.0, 3.0, 2.5, 4.0, 1.2, 2.2, 3.2, 4.2];
        let active_design = DenseMatrix::from_row_major(12, 1, x.clone()).expect("active design");
        let first_effect = [0.12, -0.08, 0.05, -0.03];
        let second_effect = [0.07, -0.06];
        let target = x
            .iter()
            .enumerate()
            .map(|(row, value)| {
                0.08 * value
                    + first_effect[first_codes[row] as usize]
                    + second_effect[second_codes[row] as usize]
            })
            .collect::<Vec<_>>();
        let outcome = target.iter().map(|value| 1.0 + value).collect::<Vec<_>>();
        let mut solver = ReusableCategoricalJointLsmr {
            active_design,
            active_slots: vec![0],
            effect_offsets: vec![0, 4, 6],
            coefficient_effect: vec![0, 0, 0, 0, 1, 1],
            coefficient_level: vec![0, 1, 2, 3, 0, 1],
            sqrt_weights: vec![0.0; 12],
            factor_weights: vec![0.0; 12],
            rhs: vec![0.0; 12],
            target: vec![0.0; 12],
            delta_eta: vec![0.0; 12],
            residual: vec![0.0; 12],
            warm_start: vec![0.0; 7],
            finite_inverse: vec![0.0; 1],
            fe_inverse: vec![0.0; 6],
            warm_start_valid: false,
            preconditioner: None,
            previous_iterations: 0,
            policy: CategoricalLsmrPolicy::Diagonal,
        };
        let first = solver
            .solve(&design, &outcome, &[1.0; 12], &[1.0; 12], 1e-10, 1_000)
            .expect("first correction");
        assert!(first.certificate.passed);
        for (actual, expected) in solver.delta_eta.iter().zip(&target) {
            assert!((actual - expected).abs() <= 1e-9);
        }

        solver.accept_step(0.5);
        let mean = target
            .iter()
            .map(|value| 1.0 + 0.5 * value)
            .collect::<Vec<_>>();
        let weights = mean.clone();
        let second = solver
            .solve(&design, &outcome, &mean, &weights, 1e-10, 1_000)
            .expect("second correction");
        assert!(second.certificate.passed);
        assert!(solver.warm_start_valid);
        assert!(solver.delta_eta.iter().all(|value| value.is_finite()));
    }
}
