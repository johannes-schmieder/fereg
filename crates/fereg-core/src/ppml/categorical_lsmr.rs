//! PPML augmented LSMR over finite controls and categorical intercepts.
//!
//! Exact normal-equation moments are recomputed after every recurrence stop;
//! LSMR diagnostics alone never authorize an IRLS step.

use std::time::Instant;

use rayon::prelude::*;
use schwarz_precond::{mlsmr, LsmrStopReason, MlsmrOptions, Operator, SolveError, Staleness};
use within::{Effect, Preconditioner, PreconditionerConfig, Solver};

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use crate::prepared_fit_view::PreparedOlsFitView;

use super::PpmlIterationWorkspace;

/// Numeric preconditioner lifecycle for categorical PPML LSMR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoricalLsmrPolicy {
    Diagonal,
    Reuse,
}

/// Select the joint PPML preconditioner without a data-dependent threshold.
///
/// The retained Schwarz factor is effective for two categorical effects. With
/// three or more effects, its pairwise factors can make the augmented
/// recurrence dramatically slower than the diagonal block while adding a
/// large topology-dependent memory peak.
pub(crate) const fn joint_lsmr_policy(effect_count: usize) -> CategoricalLsmrPolicy {
    if effect_count >= 3 {
        CategoricalLsmrPolicy::Diagonal
    } else {
        CategoricalLsmrPolicy::Reuse
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EffectTopologyIdentity {
    codes_address: usize,
    membership_address: usize,
    observations: usize,
    levels: usize,
}

/// Identity of the immutable categorical topology captured by a reusable
/// ordinary-frame engine. Pointer identity is intentional: accepting an equal-
/// shaped replacement would let cached offsets or factors operate on different
/// memberships. `FeDesign` exposes no mutation of these backing allocations.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct FeTopologyIdentity {
    observations: usize,
    total_coefficients: usize,
    effects: Vec<EffectTopologyIdentity>,
}

impl FeTopologyIdentity {
    pub(super) fn capture(design: &FeDesign) -> Result<Self> {
        let mut effects = Vec::new();
        effects
            .try_reserve_exact(design.effects().len())
            .map_err(|_| allocation("unable to reserve PPML LSMR topology identity"))?;
        for effect in design.effects() {
            effects.push(EffectTopologyIdentity {
                codes_address: effect.codes().as_ptr() as usize,
                membership_address: effect.level_index() as *const _ as usize,
                observations: effect.codes().len(),
                levels: effect.n_levels(),
            });
        }
        Ok(Self {
            observations: design.nobs(),
            total_coefficients: design.total_coefficients(),
            effects,
        })
    }

    pub(super) fn matches(&self, design: &FeDesign) -> bool {
        self.observations == design.nobs()
            && self.total_coefficients == design.total_coefficients()
            && self.effects.len() == design.effects().len()
            && self
                .effects
                .iter()
                .zip(design.effects())
                .all(|(identity, effect)| {
                    identity.codes_address == effect.codes().as_ptr() as usize
                        && identity.membership_address == effect.level_index() as *const _ as usize
                        && identity.observations == effect.codes().len()
                        && identity.levels == effect.n_levels()
                })
    }
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
    pub(crate) retries: usize,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) certification_seconds: f64,
    pub(crate) rebuilt_preconditioner: bool,
    pub(crate) requested_schwarz: bool,
    pub(crate) used_schwarz: bool,
    pub(crate) warm_start_attempted: bool,
    pub(crate) certificate: JointLsmrCertificate,
    #[cfg(test)]
    pub(crate) executing_threads: usize,
}

/// Fit-lifetime state for the ordinary (non-strict) IRLS Newton correction.
pub(crate) struct ReusableCategoricalJointLsmr {
    team: InstalledThreadTeam,
    active_design: DenseMatrix,
    active_slots: Vec<usize>,
    topology: FeTopologyIdentity,
    effect_offsets: Vec<usize>,
    sqrt_weights: Vec<f64>,
    rhs: Vec<f64>,
    target: Vec<f64>,
    delta_eta: Vec<f64>,
    residual: Vec<f64>,
    warm_start: Vec<f64>,
    finite_inverse: Vec<f64>,
    fe_inverse: Vec<f64>,
    warm_start_valid: bool,
    preconditioner: Option<Preconditioner>,
    policy: CategoricalLsmrPolicy,
    #[cfg(test)]
    instrumentation: JointLsmrInstrumentation,
}

#[cfg(test)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct JointLsmrInstrumentation {
    schwarz_rebuilds: usize,
    escalation_armed: usize,
}

impl ReusableCategoricalJointLsmr {
    pub(crate) fn new_with_threads<P: PreparedOlsFitView + Sync + ?Sized>(
        prepared: &P,
        active_generated: &[usize],
        fe_design: &FeDesign,
        threads: ThreadConfig,
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

        let topology = FeTopologyIdentity::capture(fe_design)?;
        let team = ThreadTeam::new(threads).acquire()?;
        let active_design = team.install(|| build_active_design(prepared, active_generated))?;
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
        let columns = active_generated
            .len()
            .checked_add(fe_coefficients)
            .ok_or_else(|| allocation("joint PPML augmented width overflows"))?;
        Ok(Self {
            team,
            active_design,
            active_slots,
            topology,
            effect_offsets,
            sqrt_weights: fallible_filled(rows, 0.0, "joint PPML square-root weights")?,
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
            policy,
            #[cfg(test)]
            instrumentation: JointLsmrInstrumentation::default(),
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

        let team = self.team.clone();
        team.install(|| {
            self.solve_installed(
                fe_design,
                outcome,
                mean,
                hessian_weights,
                tolerance,
                max_iterations,
            )
        })
    }

    fn solve_installed(
        &mut self,
        fe_design: &FeDesign,
        outcome: &[f64],
        mean: &[f64],
        hessian_weights: &[f64],
        tolerance: f64,
        max_iterations: usize,
    ) -> Result<JointLsmrReport> {
        #[cfg(test)]
        let executing_threads = rayon::current_num_threads();
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
        let mut setup_seconds = setup_started.elapsed().as_secs_f64();
        let mut solve_seconds = 0.0_f64;
        let mut certification_seconds = 0.0_f64;
        let mut retries = 0_usize;
        let staleness = Staleness::try_new(4, 0.75).map_err(|error| {
            FeregError::new(
                ErrorCode::Internal,
                format!("invalid joint PPML LSMR staleness policy: {error}"),
            )
        })?;
        // Recurrence escalation is meaningful only for a factor carried from a
        // previous Hessian frame. A factor built from the current weights
        // cannot be made fresher by immediately rebuilding it unchanged.
        let use_escalation = self.policy == CategoricalLsmrPolicy::Reuse && use_schwarz && !rebuilt;
        #[cfg(test)]
        if use_escalation {
            self.instrumentation.escalation_armed =
                self.instrumentation.escalation_armed.saturating_add(1);
        }
        let solve_started = Instant::now();
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
        solve_seconds += solve_started.elapsed().as_secs_f64();
        let mut result = first;
        let mut iterations = result.iterations;
        let mut effective_tolerance = tolerance;
        if result.stop_reason == LsmrStopReason::Escalated {
            let rebuild_started = Instant::now();
            use_schwarz = self.rebuild_schwarz(fe_design, hessian_weights)?;
            rebuilt = true;
            if !use_schwarz {
                fill_fe_diagonal_inverse(fe_design, hessian_weights, &mut self.fe_inverse)?;
            }
            setup_seconds += rebuild_started.elapsed().as_secs_f64();
            retries = retries.checked_add(1).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "joint PPML LSMR retry count overflowed",
                )
            })?;
            let solve_started = Instant::now();
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
            solve_seconds += solve_started.elapsed().as_secs_f64();
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

        let certification_started = Instant::now();
        let mut certificate =
            self.finish_and_certify(fe_design, hessian_weights, &result.x, tolerance)?;
        certification_seconds += certification_started.elapsed().as_secs_f64();
        if !certificate.passed {
            if !rebuilt && self.policy != CategoricalLsmrPolicy::Diagonal {
                let rebuild_started = Instant::now();
                use_schwarz = self.rebuild_schwarz(fe_design, hessian_weights)?;
                rebuilt = true;
                if !use_schwarz {
                    fill_fe_diagonal_inverse(fe_design, hessian_weights, &mut self.fe_inverse)?;
                }
                setup_seconds += rebuild_started.elapsed().as_secs_f64();
            }
            effective_tolerance = (tolerance * 0.01).max(1e-14);
            retries = retries.checked_add(1).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "joint PPML LSMR retry count overflowed",
                )
            })?;
            let solve_started = Instant::now();
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
            solve_seconds += solve_started.elapsed().as_secs_f64();
            iterations = iterations
                .checked_add(continued.iterations)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "joint PPML LSMR iteration count overflowed",
                    )
                })?;
            result = continued;
            let certification_started = Instant::now();
            certificate =
                self.finish_and_certify(fe_design, hessian_weights, &result.x, tolerance)?;
            certification_seconds += certification_started.elapsed().as_secs_f64();
        }
        if !result.converged || !certificate.passed {
            return Err(nonconvergence(
                "joint PPML LSMR correction did not pass its exact normal-equation certificate",
            ));
        }

        self.warm_start.copy_from_slice(&result.x);
        self.warm_start_valid = true;

        Ok(JointLsmrReport {
            iterations,
            effective_tolerance,
            recurrence_residual: result.normal_eq_residual,
            retries,
            setup_seconds,
            solve_seconds,
            certification_seconds,
            rebuilt_preconditioner: rebuilt,
            requested_schwarz: self.policy != CategoricalLsmrPolicy::Diagonal,
            used_schwarz: use_schwarz,
            warm_start_attempted,
            certificate,
            #[cfg(test)]
            executing_threads,
        })
    }

    pub(crate) fn fill_candidate(&self, workspace: &mut PpmlIterationWorkspace) -> Result<()> {
        self.team.install(|| {
            workspace.fill_incremental_candidate(
                &self.delta_eta,
                &self.active_slots,
                &self.warm_start[..self.active_slots.len()],
            )
        })
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
            || !self.topology.matches(fe_design)
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
        self.refresh_finite_diagonal_preconditioner(weights)?;
        if self.policy == CategoricalLsmrPolicy::Diagonal {
            fill_fe_diagonal_inverse(fe_design, weights, &mut self.fe_inverse)?;
            return Ok((false, false));
        }
        // Reuse until the recurrence or the exact certificate demonstrates
        // that the retained factor is inadequate. This avoids an unmeasured
        // FE-size or weight-ratio routing threshold.
        let rebuild = self.preconditioner.is_none();
        if rebuild {
            let use_schwarz = self.rebuild_schwarz(fe_design, weights)?;
            if !use_schwarz {
                fill_fe_diagonal_inverse(fe_design, weights, &mut self.fe_inverse)?;
            }
            return Ok((use_schwarz, true));
        }
        Ok((true, false))
    }

    fn rebuild_schwarz(&mut self, fe_design: &FeDesign, weights: &[f64]) -> Result<bool> {
        #[cfg(test)]
        {
            self.instrumentation.schwarz_rebuilds =
                self.instrumentation.schwarz_rebuilds.saturating_add(1);
        }
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
        Ok(self.preconditioner.is_some())
    }

    fn refresh_finite_diagonal_preconditioner(&mut self, weights: &[f64]) -> Result<()> {
        self.finite_inverse
            .par_iter_mut()
            .enumerate()
            .for_each(|(column, inverse)| {
                let mut diagonal = 0.0_f64;
                for (row, &weight) in weights.iter().enumerate() {
                    let value = self.active_design[(row, column)];
                    diagonal = (weight * value).mul_add(value, diagonal);
                }
                *inverse = if diagonal > 0.0 { 1.0 / diagonal } else { 0.0 };
            });
        if self.finite_inverse.iter().any(|value| !value.is_finite()) {
            return Err(nonconvergence(
                "joint PPML finite diagonal preconditioner is nonfinite",
            ));
        }
        Ok(())
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
    sqrt_weights: &'a [f64],
}

impl<'a> AugmentedCategoricalOperator<'a> {
    fn new(
        fe_design: &'a FeDesign,
        active_design: &'a DenseMatrix,
        effect_offsets: &'a [usize],
        sqrt_weights: &'a [f64],
    ) -> Result<Self> {
        if fe_design.nobs() != active_design.nrows()
            || sqrt_weights.len() != fe_design.nobs()
            || effect_offsets.len() != fe_design.effects().len() + 1
            || effect_offsets.first().copied() != Some(0)
            || effect_offsets.last().copied() != Some(fe_design.total_coefficients())
            || fe_design
                .effects()
                .iter()
                .zip(effect_offsets.windows(2))
                .any(|(effect, offsets)| offsets[1].saturating_sub(offsets[0]) != effect.n_levels())
        {
            return Err(invalid_argument(
                "joint PPML augmented operator dimensions are inconsistent",
            ));
        }
        Ok(Self {
            fe_design,
            active_design,
            effect_offsets,
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
            for (column, &coefficient) in x[..finite].iter().enumerate() {
                fitted = self.active_design[(row, column)].mul_add(coefficient, fitted);
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
            for (column, &coefficient) in x[..finite].iter().enumerate() {
                fitted = self.active_design[(row, column)].mul_add(coefficient, fitted);
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
                for (row, &input) in x.iter().enumerate() {
                    sum = (self.sqrt_weights[row] * self.active_design[(row, column)])
                        .mul_add(input, sum);
                }
                *output = sum;
            });
        for (effect_index, effect) in self.fe_design.effects().iter().enumerate() {
            let start = self.effect_offsets[effect_index];
            let end = self.effect_offsets[effect_index + 1];
            fe_output[start..end]
                .par_iter_mut()
                .enumerate()
                .for_each(|(level, output)| {
                    let mut sum = 0.0_f64;
                    for &row in effect.level_index().members(level) {
                        sum = self.sqrt_weights[row].mul_add(x[row], sum);
                    }
                    *output = sum;
                });
        }
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
    let mut target_square = ScaledSquareSum::default();
    let mut residual_square = ScaledSquareSum::default();
    for row in 0..design.nobs() {
        let root_weight = weights[row].sqrt();
        target_square.add(root_weight * target[row])?;
        residual_square.add(root_weight * residual[row])?;
    }
    let reference = target_square.norm()?.max(residual_square.norm()?);
    let mut finite_absolute = 0.0_f64;
    let mut fe_absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_square = FiniteCompensatedSum::default();
    let mut count = 0_usize;
    for column in 0..active.ncols() {
        let mut moment = FiniteCompensatedSum::default();
        let mut basis_square = ScaledSquareSum::default();
        for row in 0..active.nrows() {
            let basis = active[(row, column)];
            let root_weight = weights[row].sqrt();
            let (moment_term, weighted_basis) =
                finite_weighted_moment(weights[row], residual[row], basis, root_weight)?;
            moment.add(moment_term)?;
            basis_square.add(weighted_basis)?;
        }
        let moment = moment.total()?;
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative(
            moment,
            basis_square.norm()?,
            reference,
            &mut relative_infinity,
            &mut relative_square,
            &mut count,
        )?;
    }
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let mut moment = FiniteCompensatedSum::default();
            let mut basis_square = ScaledSquareSum::default();
            for &row in effect.level_index().members(level) {
                let root_weight = weights[row].sqrt();
                let (moment_term, weighted_basis) =
                    finite_weighted_moment(weights[row], residual[row], 1.0, root_weight)?;
                moment.add(moment_term)?;
                basis_square.add(weighted_basis)?;
            }
            let moment = moment.total()?;
            fe_absolute = fe_absolute.max(moment.abs());
            add_relative(
                moment,
                basis_square.norm()?,
                reference,
                &mut relative_infinity,
                &mut relative_square,
                &mut count,
            )?;
        }
    }
    let relative_two = if count == 0 {
        0.0
    } else {
        (relative_square.total()?.max(0.0) / count as f64).sqrt()
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

#[inline]
pub(super) fn finite_weighted_moment(
    weight: f64,
    residual: f64,
    basis: f64,
    root_weight: f64,
) -> Result<(f64, f64)> {
    let weighted_residual = root_weight * residual;
    let weighted_basis = root_weight * basis;
    let split_product = weighted_residual * weighted_basis;
    if split_product.is_finite()
        && !weighted_residual.is_subnormal()
        && !weighted_basis.is_subnormal()
        && !split_product.is_subnormal()
        && (split_product != 0.0 || weight == 0.0 || residual == 0.0 || basis == 0.0)
    {
        return Ok((split_product, weighted_basis));
    }
    Ok((finite_product3(weight, residual, basis)?, weighted_basis))
}

#[cold]
#[inline(never)]
fn finite_product3(mut first: f64, mut second: f64, mut third: f64) -> Result<f64> {
    if !first.is_finite() || !second.is_finite() || !third.is_finite() {
        return Err(nonconvergence("PPML LSMR certificate product is nonfinite"));
    }
    // Pair the smallest and largest magnitudes first. If the exact three-
    // factor product is representable, this order cannot overflow or
    // underflow merely because the two poorly scaled factors were adjacent.
    if first.abs() > second.abs() {
        std::mem::swap(&mut first, &mut second);
    }
    if second.abs() > third.abs() {
        std::mem::swap(&mut second, &mut third);
    }
    if first.abs() > second.abs() {
        std::mem::swap(&mut first, &mut second);
    }
    let product = (first * third) * second;
    if product.is_finite() {
        Ok(product)
    } else {
        Err(nonconvergence("PPML LSMR certificate product is nonfinite"))
    }
}

pub(super) fn add_relative(
    moment: f64,
    basis_norm: f64,
    reference: f64,
    infinity: &mut f64,
    square: &mut FiniteCompensatedSum,
    count: &mut usize,
) -> Result<()> {
    if !moment.is_finite() || !basis_norm.is_finite() || !reference.is_finite() {
        return Err(nonconvergence(
            "PPML LSMR certificate scale or moment is nonfinite",
        ));
    }
    let relative = if basis_norm > 0.0 && reference > 0.0 {
        // Divide by the larger scale first so the product of two finite norms
        // cannot overflow and turn a material moment into a false zero.
        if basis_norm >= reference {
            (moment.abs() / basis_norm) / reference
        } else {
            (moment.abs() / reference) / basis_norm
        }
    } else if moment == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    *infinity = (*infinity).max(relative);
    square.add(relative * relative)?;
    *count = count
        .checked_add(1)
        .ok_or_else(|| allocation("PPML LSMR certificate direction count overflows"))?;
    Ok(())
}

#[derive(Debug, Default)]
pub(super) struct FiniteCompensatedSum {
    sum: f64,
    correction: f64,
}

impl FiniteCompensatedSum {
    pub(super) fn add(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(nonconvergence(
                "PPML LSMR certificate accumulation is nonfinite",
            ));
        }
        let updated = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - updated) + value
        } else {
            (value - updated) + self.sum
        };
        self.sum = updated;
        if self.sum.is_finite() && self.correction.is_finite() {
            Ok(())
        } else {
            Err(nonconvergence(
                "PPML LSMR certificate accumulation is nonfinite",
            ))
        }
    }

    pub(super) fn total(&self) -> Result<f64> {
        let total = self.sum + self.correction;
        if total.is_finite() {
            Ok(total)
        } else {
            Err(nonconvergence(
                "PPML LSMR certificate accumulation is nonfinite",
            ))
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ScaledSquareSum {
    scale: f64,
    square_sum: f64,
}

impl ScaledSquareSum {
    pub(super) fn add(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(nonconvergence("PPML LSMR weighted norm is nonfinite"));
        }
        let absolute = value.abs();
        if absolute == 0.0 {
            return Ok(());
        }
        if self.scale < absolute {
            let ratio = if self.scale == 0.0 {
                0.0
            } else {
                self.scale / absolute
            };
            self.square_sum = 1.0 + self.square_sum * ratio * ratio;
            self.scale = absolute;
        } else {
            let ratio = absolute / self.scale;
            self.square_sum += ratio * ratio;
        }
        if self.scale.is_finite() && self.square_sum.is_finite() {
            Ok(())
        } else {
            Err(nonconvergence("PPML LSMR weighted norm is nonfinite"))
        }
    }

    pub(super) fn norm(&self) -> Result<f64> {
        let norm = if self.scale == 0.0 {
            0.0
        } else {
            self.scale * self.square_sum.sqrt()
        };
        if norm.is_finite() {
            Ok(norm)
        } else {
            Err(nonconvergence("PPML LSMR weighted norm is nonfinite"))
        }
    }
}

/// Conservative heap peak added by the PPML joint-LSMR engine.
///
/// This is deliberately route- and policy-specific. The generic FE estimate
/// assumes borrowed topology and a shared recurrence, while this engine
/// retains an active finite design and PPML frame buffers and can overlap an
/// earlier solution with a certified continuation. Schwarz state is charged
/// only for a policy that can build or reuse it.
pub(crate) fn estimate_joint_lsmr_workspace_bytes(
    rows: usize,
    active_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
    policy: CategoricalLsmrPolicy,
) -> Result<u128> {
    let shape = CategoricalLsmrMemoryShape::new_with_schwarz(
        rows,
        active_columns,
        effect_level_counts,
        threads,
        policy != CategoricalLsmrPolicy::Diagonal,
    )?;
    let n = shape.rows;
    let k = shape.active_columns;
    let p = checked_add_memory(k, shape.levels, "joint PPML augmented width")?;

    // Fit-lifetime allocations owned directly by ReusableCategoricalJointLsmr.
    let retained_values = checked_sum_memory(&[
        checked_mul_memory(n, k, "joint PPML retained active design")?,
        checked_mul_memory(n, 5, "joint PPML retained observation buffers")?,
        p,
        k,
        shape.levels,
    ])?;
    let retained_indices = checked_sum_memory(&[
        k,
        checked_add_memory(shape.effects, 1, "joint PPML retained effect-offset count")?,
        checked_mul_memory(shape.effects, 4, "joint PPML topology identity words")?,
    ])?;
    let retained_engine = checked_sum_memory(&[
        bytes_for_memory::<f64>(retained_values, "joint PPML retained values")?,
        bytes_for_memory::<usize>(retained_indices, "joint PPML retained indices")?,
        shape.schwarz.retained_bytes,
    ])?;

    // The continuation path uses a four-vector local reorthogonalization
    // window. Besides Modified Golub-Kahan's 2N + (3 + 2w)P vectors, the
    // LSMR solution recurrence owns another 3P vectors and a warm start owns
    // an N-vector residual. Keep one prior P-vector result live because both
    // recurrence escalation and exact-certificate continuation do so.
    const CONTINUATION_LOCAL_SIZE: u128 = 4;
    let recurrence_vectors = checked_add_memory(
        6,
        checked_mul_memory(CONTINUATION_LOCAL_SIZE, 2, "joint PPML local history lanes")?,
        "joint PPML recurrence vector count",
    )?;
    let recurrence_values = checked_sum_memory(&[
        checked_mul_memory(n, 3, "joint PPML recurrence observation vectors")?,
        checked_mul_memory(
            p,
            recurrence_vectors,
            "joint PPML recurrence coefficient vectors",
        )?,
        p,
    ])?;
    let recurrence_peak = checked_sum_memory(&[
        bytes_for_memory::<f64>(recurrence_values, "joint PPML recurrence values")?,
        shape.schwarz.apply_peak_bytes,
    ])?;

    // A stale/certificate-triggered rebuild retains the previous solution
    // while `within::Solver::new` owns copied weights and may locality-sort all
    // categorical columns.
    let solve_peak = if policy == CategoricalLsmrPolicy::Diagonal {
        recurrence_peak
    } else {
        let rebuild_peak = checked_add_memory(
            shape.schwarz.build_peak_bytes,
            bytes_for_memory::<f64>(p, "joint PPML retained continuation result")?,
            "joint PPML rebuild peak",
        )?;
        recurrence_peak.max(rebuild_peak)
    };
    checked_add_memory(
        retained_engine,
        solve_peak,
        "joint PPML LSMR workspace peak",
    )
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct CategoricalSchwarzMemoryEstimate {
    pub(super) retained_bytes: u128,
    pub(super) build_peak_bytes: u128,
    pub(super) apply_peak_bytes: u128,
    pub(super) solver_frame_bytes: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CategoricalLsmrMemoryShape {
    pub(super) rows: u128,
    pub(super) active_columns: u128,
    pub(super) effects: u128,
    pub(super) levels: u128,
    pub(super) maximum_levels: u128,
    pub(super) threads: u128,
    pub(super) schwarz: CategoricalSchwarzMemoryEstimate,
}

impl CategoricalLsmrMemoryShape {
    pub(super) fn new(
        rows: usize,
        active_columns: usize,
        effect_level_counts: &[usize],
        threads: usize,
    ) -> Result<Self> {
        Self::new_with_schwarz(rows, active_columns, effect_level_counts, threads, true)
    }

    fn new_with_schwarz(
        rows: usize,
        active_columns: usize,
        effect_level_counts: &[usize],
        threads: usize,
        include_schwarz: bool,
    ) -> Result<Self> {
        if rows == 0 || threads == 0 {
            return Err(invalid_argument(
                "PPML categorical LSMR memory estimate requires positive rows and threads",
            ));
        }
        if effect_level_counts.is_empty() || effect_level_counts.contains(&0) {
            return Err(invalid_argument(
                "PPML categorical LSMR memory estimate requires positive effect level counts",
            ));
        }
        let levels = effect_level_counts
            .iter()
            .try_fold(0_u128, |total, &levels| {
                checked_add_memory(total, levels as u128, "PPML LSMR total levels")
            })?;
        let maximum_levels = effect_level_counts.iter().copied().max().unwrap_or(0) as u128;
        let rows = rows as u128;
        let effects = effect_level_counts.len() as u128;
        let threads = threads as u128;
        let schwarz = if include_schwarz {
            estimate_categorical_schwarz_memory(rows, effects, levels, maximum_levels, threads)?
        } else {
            CategoricalSchwarzMemoryEstimate::default()
        };
        Ok(Self {
            rows,
            active_columns: active_columns as u128,
            effects,
            levels,
            maximum_levels,
            threads,
            schwarz,
        })
    }
}

/// Upper-bound the owned `within::Solver` categorical/Schwarz state used by
/// both PPML LSMR routes. Every factor-pair can contain N distinct edges; the
/// accounting charges both CSR orientations, construction copies, per-level
/// maps/factors, locality-sort copies, and concurrent Schwarz apply buffers.
fn estimate_categorical_schwarz_memory(
    rows: u128,
    effects: u128,
    levels: u128,
    maximum_levels: u128,
    threads: u128,
) -> Result<CategoricalSchwarzMemoryEstimate> {
    let pairs = checked_mul_memory(
        effects,
        effects.saturating_sub(1),
        "PPML LSMR factor-pair count",
    )? / 2;
    let edges = checked_mul_memory(rows, pairs, "PPML LSMR factor-pair edges")?;
    let pair_level_incidence = checked_mul_memory(
        levels,
        effects.saturating_sub(1),
        "PPML LSMR pair-level incidence",
    )?;

    // Constants include payload slack for opaque upstream vector and factor
    // records, not an empirically selected routing threshold.
    let retained_edge_bytes = checked_mul_memory(edges, 80, "PPML LSMR retained edge bytes")?;
    let retained_level_bytes = checked_mul_memory(
        pair_level_incidence,
        80,
        "PPML LSMR retained pair-level bytes",
    )?;
    let diagonal_bytes = bytes_for_memory::<f64>(levels, "PPML LSMR retained diagonal")?;

    // The additive executor pools at most four buffers. During an apply,
    // nested Rayon work cannot have more live global accumulators than the
    // retained team width. Each buffer also has two local-solve scratches,
    // conservatively sized to two maximum FE blocks.
    let local_scratch_values =
        checked_mul_memory(maximum_levels, 4, "PPML LSMR Schwarz local scratch values")?;
    let global_apply_values = checked_add_memory(
        levels,
        local_scratch_values,
        "PPML LSMR Schwarz apply values",
    )?;
    let retained_pool_bytes = bytes_for_memory::<f64>(
        checked_mul_memory(
            global_apply_values,
            threads.min(4),
            "PPML LSMR retained Schwarz pools",
        )?,
        "PPML LSMR retained Schwarz pool bytes",
    )?;
    let apply_peak_bytes = bytes_for_memory::<f64>(
        checked_mul_memory(
            global_apply_values,
            threads,
            "PPML LSMR concurrent Schwarz buffers",
        )?,
        "PPML LSMR concurrent Schwarz bytes",
    )?;
    let retained_bytes = checked_sum_memory(&[
        retained_edge_bytes,
        retained_level_bytes,
        diagonal_bytes,
        retained_pool_bytes,
    ])?;

    let categorical_copy_bytes = checked_sum_memory(&[
        bytes_for_memory::<u32>(
            checked_mul_memory(rows, effects, "PPML LSMR sorted categorical cells")?,
            "PPML LSMR sorted categorical bytes",
        )?,
        bytes_for_memory::<u32>(rows, "PPML LSMR observation permutation")?,
    ])?;
    let solver_frame_bytes = checked_sum_memory(&[
        categorical_copy_bytes,
        bytes_for_memory::<f64>(rows, "PPML LSMR square-root weights")?,
        bytes_for_memory::<usize>(
            checked_add_memory(
                pair_level_incidence,
                effects,
                "PPML LSMR categorical metadata count",
            )?,
            "PPML LSMR categorical metadata bytes",
        )?,
    ])?;

    // Construction may simultaneously hold caller-order and sorted weights,
    // stable-sort scratch, active maps, both transient cross tabs, the newly
    // built retained factor, and the solver frame that takes ownership.
    let sort_scratch_bytes = checked_sum_memory(&[
        checked_mul_memory(rows, 16, "PPML LSMR stable-sort scratch")?,
        bytes_for_memory::<usize>(
            checked_add_memory(maximum_levels, 1, "PPML LSMR sort cursor count")?,
            "PPML LSMR sort cursor bytes",
        )?,
    ])?;
    let build_edge_bytes = checked_mul_memory(edges, 64, "PPML LSMR transient edge bytes")?;
    let build_level_bytes = checked_mul_memory(
        pair_level_incidence,
        64,
        "PPML LSMR transient pair-level bytes",
    )?;
    let build_peak_bytes = checked_sum_memory(&[
        solver_frame_bytes,
        bytes_for_memory::<f64>(rows, "PPML LSMR source weight copy")?,
        sort_scratch_bytes,
        build_edge_bytes,
        build_level_bytes,
        retained_bytes,
    ])?;
    Ok(CategoricalSchwarzMemoryEstimate {
        retained_bytes,
        build_peak_bytes,
        apply_peak_bytes,
        solver_frame_bytes,
    })
}

pub(super) fn checked_add_memory(left: u128, right: u128, context: &'static str) -> Result<u128> {
    left.checked_add(right).ok_or_else(|| allocation(context))
}

pub(super) fn checked_mul_memory(left: u128, right: u128, context: &'static str) -> Result<u128> {
    left.checked_mul(right).ok_or_else(|| allocation(context))
}

pub(super) fn checked_sum_memory(values: &[u128]) -> Result<u128> {
    values.iter().try_fold(0_u128, |total, &value| {
        checked_add_memory(total, value, "PPML LSMR workspace sum overflowed")
    })
}

pub(super) fn bytes_for_memory<T>(elements: u128, context: &'static str) -> Result<u128> {
    checked_mul_memory(elements, std::mem::size_of::<T>() as u128, context)
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
        let weights = vec![1.0, 2.0_f64.sqrt(), 0.5_f64.sqrt(), 1.5_f64.sqrt()];
        let operator = AugmentedCategoricalOperator::new(&design, &active, &offsets, &weights)
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
    fn exact_certificate_does_not_turn_overflowing_squares_into_a_false_pass() {
        let design = FeDesign::new(
            2,
            vec![EffectTerm::new(vec![0, 0], 1, true, Vec::new()).expect("effect")],
        )
        .expect("design");
        let active = DenseMatrix::from_row_major(2, 0, Vec::new()).expect("active");
        let values = [1e200, 1e200];
        let certificate = certify_joint(&design, &active, &[1.0; 2], &values, &values, 1e-8)
            .expect("finite scaled certificate");
        assert!(!certificate.passed);
        assert!((certificate.relative_infinity_norm - 1.0).abs() <= 1e-15);
    }

    #[test]
    fn exact_certificate_does_not_erase_a_finite_moment_through_split_underflow() {
        let design = FeDesign::new(
            2,
            vec![EffectTerm::new(vec![0, 0], 1, true, Vec::new()).expect("effect")],
        )
        .expect("design");
        let active = DenseMatrix::from_row_major(2, 1, vec![1e300, 1e300]).expect("active");
        let values = [1e-200, 1e-200];
        let error = certify_joint(&design, &active, &[1e-300; 2], &values, &values, 1e-8)
            .expect_err("unrepresentable reference norm must fail closed");
        assert_eq!(error.code, ErrorCode::NonConvergence);
    }

    #[test]
    fn weighted_moment_reorders_a_nonzero_subnormal_split_factor() {
        let weight = 1e-300_f64;
        let residual = 6e-174_f64;
        let basis = 1e308_f64;
        let root_weight = weight.sqrt();
        let split = (root_weight * residual) * (root_weight * basis);
        let expected = finite_product3(weight, residual, basis).expect("finite reordered product");
        assert!(root_weight.mul_add(residual, 0.0).is_subnormal());
        assert!((split - expected).abs() / expected.abs() > 0.1);

        let (moment, weighted_basis) = finite_weighted_moment(weight, residual, basis, root_weight)
            .expect("robust weighted moment");
        assert_eq!(moment, expected);
        assert_eq!(weighted_basis, root_weight * basis);
    }

    #[test]
    fn topology_identity_rejects_an_equal_shaped_replacement() {
        let first = FeDesign::new(
            4,
            vec![EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("first")],
        )
        .expect("first design");
        let replacement = FeDesign::new(
            4,
            vec![EffectTerm::new(vec![0, 1, 0, 1], 2, true, Vec::new()).expect("replacement")],
        )
        .expect("replacement design");
        let identity = FeTopologyIdentity::capture(&first).expect("identity");
        assert!(identity.matches(&first));
        assert!(!identity.matches(&replacement));

        let mut solver = ReusableCategoricalJointLsmr {
            team: test_team(1),
            active_design: DenseMatrix::from_row_major(4, 0, Vec::new()).expect("active"),
            active_slots: Vec::new(),
            topology: identity,
            effect_offsets: vec![0, 2],
            sqrt_weights: vec![0.0; 4],
            rhs: vec![0.0; 4],
            target: vec![0.0; 4],
            delta_eta: vec![0.0; 4],
            residual: vec![0.0; 4],
            warm_start: vec![0.0; 2],
            finite_inverse: Vec::new(),
            fe_inverse: vec![0.0; 2],
            warm_start_valid: false,
            preconditioner: None,
            policy: CategoricalLsmrPolicy::Diagonal,
            instrumentation: JointLsmrInstrumentation::default(),
        };
        let error = solver
            .solve(&replacement, &[1.0; 4], &[1.0; 4], &[1.0; 4], 1e-8, 100)
            .expect_err("replacement topology must fail before operator use");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn topology_identity_rejects_a_different_effect_partition_with_the_same_total_levels() {
        let first = FeDesign::new(
            4,
            vec![
                EffectTerm::new(vec![0, 0, 0, 0], 1, true, Vec::new()).expect("first effect"),
                EffectTerm::new(vec![0, 1, 2, 0], 3, true, Vec::new()).expect("second effect"),
            ],
        )
        .expect("first design");
        let replacement = FeDesign::new(
            4,
            vec![
                EffectTerm::new(vec![0, 1, 0, 1], 2, true, Vec::new()).expect("first effect"),
                EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("second effect"),
            ],
        )
        .expect("replacement design");
        assert_eq!(first.total_coefficients(), replacement.total_coefficients());
        let identity = FeTopologyIdentity::capture(&first).expect("identity");
        assert!(!identity.matches(&replacement));
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
            team: test_team(2),
            active_design,
            active_slots: vec![0],
            topology: FeTopologyIdentity::capture(&design).expect("topology"),
            effect_offsets: vec![0, 4, 6],
            sqrt_weights: vec![0.0; 12],
            rhs: vec![0.0; 12],
            target: vec![0.0; 12],
            delta_eta: vec![0.0; 12],
            residual: vec![0.0; 12],
            warm_start: vec![0.0; 7],
            finite_inverse: vec![0.0; 1],
            // A Schwarz frame must never inspect or overwrite the unused FE
            // diagonal. NaNs make an accidental read fail the solve, while an
            // accidental refresh is visible below.
            fe_inverse: vec![f64::NAN; 6],
            warm_start_valid: false,
            preconditioner: None,
            policy: CategoricalLsmrPolicy::Reuse,
            instrumentation: JointLsmrInstrumentation::default(),
        };
        let unrelated_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("unrelated pool");
        let first = unrelated_pool
            .install(|| solver.solve(&design, &outcome, &[1.0; 12], &[1.0; 12], 1e-10, 1_000))
            .expect("first correction");
        assert_eq!(first.executing_threads, 2);
        assert_eq!(first.retries, 0);
        assert!(first.certificate.passed);
        assert!(first.certification_seconds > 0.0);
        assert!(first.requested_schwarz);
        assert!(first.used_schwarz);
        assert!(first.rebuilt_preconditioner);
        assert_eq!(solver.instrumentation.schwarz_rebuilds, 1);
        assert_eq!(solver.instrumentation.escalation_armed, 0);
        let expected_finite_inverse = 1.0 / x.iter().map(|value| value * value).sum::<f64>();
        assert!((solver.finite_inverse[0] - expected_finite_inverse).abs() <= 1e-15);
        assert!(solver.fe_inverse.iter().all(|value| value.is_nan()));
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
        assert_eq!(second.retries, 0);
        assert!(second.certificate.passed);
        assert!(second.certification_seconds > 0.0);
        assert!(second.used_schwarz);
        assert!(!second.rebuilt_preconditioner);
        assert_eq!(solver.instrumentation.schwarz_rebuilds, 1);
        assert_eq!(solver.instrumentation.escalation_armed, 1);
        let expected_finite_inverse = 1.0
            / x.iter()
                .zip(&weights)
                .map(|(value, weight)| weight * value * value)
                .sum::<f64>();
        assert!((solver.finite_inverse[0] - expected_finite_inverse).abs() <= 1e-15);
        assert!(solver.fe_inverse.iter().all(|value| value.is_nan()));
        assert!(solver.warm_start_valid);
        assert!(solver.delta_eta.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn joint_solver_tracks_retained_team_across_one_four_one() {
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
        let active_values = vec![0.0, 1.0, 1.5, 0.5, 2.0, 3.0, 2.5, 4.0, 1.2, 2.2, 3.2, 4.2];
        let first_effect = [0.12, -0.08, 0.05, -0.03];
        let second_effect = [0.07, -0.06];
        let target = active_values
            .iter()
            .enumerate()
            .map(|(row, value)| {
                0.08 * value
                    + first_effect[first_codes[row] as usize]
                    + second_effect[second_codes[row] as usize]
            })
            .collect::<Vec<_>>();
        let outcome = target.iter().map(|value| 1.0 + value).collect::<Vec<_>>();
        let ambient = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("ambient pool");

        for requested in [1, 4, 1] {
            let mut solver = ReusableCategoricalJointLsmr {
                team: test_team(requested),
                active_design: DenseMatrix::from_row_major(12, 1, active_values.clone())
                    .expect("active design"),
                active_slots: vec![0],
                topology: FeTopologyIdentity::capture(&design).expect("topology"),
                effect_offsets: vec![0, 4, 6],
                sqrt_weights: vec![0.0; 12],
                rhs: vec![0.0; 12],
                target: vec![0.0; 12],
                delta_eta: vec![0.0; 12],
                residual: vec![0.0; 12],
                warm_start: vec![0.0; 7],
                finite_inverse: vec![0.0; 1],
                fe_inverse: vec![0.0; 6],
                warm_start_valid: false,
                preconditioner: None,
                policy: CategoricalLsmrPolicy::Diagonal,
                instrumentation: JointLsmrInstrumentation::default(),
            };
            let report = ambient
                .install(|| solver.solve(&design, &outcome, &[1.0; 12], &[1.0; 12], 1e-10, 1_000))
                .expect("joint correction");
            assert_eq!(report.executing_threads, requested);
            assert!(report.certificate.passed);
        }
    }

    #[test]
    fn joint_memory_estimate_scales_with_retained_design_and_thread_team() {
        let base = estimate_joint_lsmr_workspace_bytes(
            10_000,
            2,
            &[2_000, 1_000],
            1,
            CategoricalLsmrPolicy::Reuse,
        )
        .expect("base estimate");
        let wider = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &[2_000, 1_000],
            1,
            CategoricalLsmrPolicy::Reuse,
        )
        .expect("wide estimate");
        let parallel = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &[2_000, 1_000],
            8,
            CategoricalLsmrPolicy::Reuse,
        )
        .expect("parallel estimate");
        let more_levels = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &[4_000, 2_000],
            8,
            CategoricalLsmrPolicy::Reuse,
        )
        .expect("larger topology estimate");
        assert!(wider > base);
        assert!(parallel > wider);
        assert!(more_levels > parallel);
    }

    #[test]
    fn joint_policy_keeps_two_effect_schwarz_and_diagonalizes_three_or_more() {
        assert_eq!(joint_lsmr_policy(1), CategoricalLsmrPolicy::Reuse);
        assert_eq!(joint_lsmr_policy(2), CategoricalLsmrPolicy::Reuse);
        assert_eq!(joint_lsmr_policy(3), CategoricalLsmrPolicy::Diagonal);
        assert_eq!(joint_lsmr_policy(4), CategoricalLsmrPolicy::Diagonal);
    }

    #[test]
    fn diagonal_joint_memory_excludes_schwarz_state_and_apply_scratch() {
        let levels = [2_000, 1_000, 500];
        let diagonal_one = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &levels,
            1,
            CategoricalLsmrPolicy::Diagonal,
        )
        .expect("serial diagonal estimate");
        let diagonal_eight = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &levels,
            8,
            CategoricalLsmrPolicy::Diagonal,
        )
        .expect("parallel diagonal estimate");
        let schwarz = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &levels,
            8,
            CategoricalLsmrPolicy::Reuse,
        )
        .expect("Schwarz estimate");
        assert_eq!(diagonal_one, diagonal_eight);
        assert!(diagonal_eight < schwarz);
    }

    #[test]
    fn joint_memory_estimate_rejects_invalid_and_overflowing_shapes() {
        let invalid = estimate_joint_lsmr_workspace_bytes(
            100,
            2,
            &[20, 0],
            4,
            CategoricalLsmrPolicy::Diagonal,
        )
        .expect_err("zero levels must fail");
        assert_eq!(invalid.code, ErrorCode::InvalidArgument);

        let overflow = estimate_joint_lsmr_workspace_bytes(
            usize::MAX,
            usize::MAX,
            &[usize::MAX, usize::MAX],
            usize::MAX,
            CategoricalLsmrPolicy::Reuse,
        )
        .expect_err("adversarial dimensions must fail closed");
        assert_eq!(overflow.code, ErrorCode::Allocation);
    }

    fn test_team(threads: usize) -> InstalledThreadTeam {
        ThreadTeam::new(ThreadConfig::from_requested(threads, true).expect("thread config"))
            .acquire()
            .expect("installed thread team")
    }
}
