//! PPML augmented LSMR over finite controls and categorical intercepts.
//!
//! Exact normal-equation moments are recomputed after every recurrence stop;
//! LSMR diagnostics alone never authorize an IRLS step.

use crate::linalg::neumaier_add;
use crate::memory::fallible_filled;
use std::sync::Mutex;
use std::time::Instant;

use rayon::prelude::*;
use schwarz_precond::{
    mlsmr_cancellable, LsmrStopReason, MlsmrOptions, Operator, SolveError, Staleness,
};
use within::{Effect, Preconditioner, Solver};

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::fe::{retained_effect_order_by_levels, FeDesign};
use crate::memory::{
    categorical_dense_cross_tab_bytes, estimate_categorical_schwarz_memory,
    CategoricalSchwarzMemoryEstimate,
};
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};

use super::PpmlIterationWorkspace;

const ADJOINT_LEVEL_CHUNK: usize = 64;
const ADJOINT_COLUMN_LANES_PER_THREAD: usize = 2;
const JOINT_LSMR_NUMERICAL_FLOOR: f64 = 1e-14;
const JOINT_LSMR_PREDICTOR_MAX_LOG_WEIGHT_CHANGE: f64 = 0.5;
const JOINT_LSMR_PREDICTOR_MIN_SCHEDULE_RATIO: f64 = 0.01;
const JOINT_LSMR_PREDICTOR_CERTIFICATE_TARGET: f64 = 0.5;
const JOINT_LSMR_CORE_PAIR_REFRESH_ITERATIONS: usize = 128;

/// Numeric preconditioner lifecycle for categorical PPML LSMR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoricalLsmrPolicy {
    Diagonal,
    CorePair,
    Reuse,
}

/// Select the joint PPML preconditioner without a data-dependent threshold.
///
/// Two categorical effects start with the current diagonal and promote to a
/// retained Schwarz factor on stalled progress or failed certification. With
/// three or more effects, the former all-pairs factor was slower than the
/// diagonal block. Retain only one two-effect graph core and leave every
/// additional effect on the diagonal; this does not restore all-pairs
/// Schwarz.
pub(crate) const fn joint_lsmr_policy(effect_count: usize) -> CategoricalLsmrPolicy {
    if effect_count >= 3 {
        CategoricalLsmrPolicy::CorePair
    } else {
        CategoricalLsmrPolicy::Reuse
    }
}

/// The effects whose factor pairs the policy's Schwarz factor covers, in the
/// order of `effect_level_counts`: the core pair, every effect, or none.
pub(crate) fn schwarz_effects(
    effect_level_counts: &[usize],
    policy: CategoricalLsmrPolicy,
) -> Result<Vec<usize>> {
    match policy {
        CategoricalLsmrPolicy::Diagonal => Ok(Vec::new()),
        CategoricalLsmrPolicy::Reuse => Ok((0..effect_level_counts.len()).collect()),
        CategoricalLsmrPolicy::CorePair => {
            Ok(core_pair_by_levels(effect_level_counts.len(), |effect| {
                effect_level_counts[effect]
            })?
            .to_vec())
        }
    }
}

fn largest_retained_core_pair(design: &FeDesign) -> Result<[usize; 2]> {
    core_pair_by_levels(design.effects().len(), |effect| {
        design.effects()[effect].n_levels()
    })
}

fn core_pair_by_levels(effects: usize, levels: impl Fn(usize) -> usize) -> Result<[usize; 2]> {
    let order = retained_effect_order_by_levels(effects, levels)?;
    if order.len() < 2 {
        return Err(invalid_argument(
            "joint PPML core-pair policy requires at least two effects",
        ));
    }
    Ok([order[0], order[1]])
}

use super::topology_identity::FeTopologyIdentity;

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
    pub(crate) frame: JointLsmrFrameDiagnostic,
    #[cfg(test)]
    pub(crate) executing_threads: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct JointLsmrContinuationCause {
    pub(crate) recurrence: bool,
    pub(crate) certificate: bool,
    pub(crate) staleness: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct JointLsmrFrameDiagnostic {
    pub(crate) scheduled_tolerance: f64,
    pub(crate) first_pass_tolerance: f64,
    pub(crate) effective_tolerance: f64,
    pub(crate) first_pass_iterations: usize,
    pub(crate) continuation_iterations: usize,
    pub(crate) first_pass_recurrence_residual: f64,
    pub(crate) recurrence_residual: f64,
    pub(crate) first_certificate_relative_infinity_norm: f64,
    pub(crate) finite_certificate_max: f64,
    pub(crate) fixed_effect_certificate_max: f64,
    pub(crate) max_abs_log_hessian_weight_change: f64,
    pub(crate) weight_change_valid: bool,
    pub(crate) predictor_used: bool,
    pub(crate) predictor_refreshed_preconditioner: bool,
    pub(crate) continuation: JointLsmrContinuationCause,
}

fn joint_lsmr_frame_trace_line(frame: JointLsmrFrameDiagnostic) -> String {
    format!(
        "FEREG_SOLVER_TRACE|phase=joint_lsmr_frame|scheduled={:.17e}|first_tolerance={:.17e}|effective={:.17e}|first_iterations={}|continuation_iterations={}|first_normal={:.17e}|final_normal={:.17e}|first_certificate={:.17e}|finite_max={:.17e}|fe_max={:.17e}|weight_change={:.17e}|weight_change_valid={}|predictor_used={}|predictor_refresh={}|recurrence_continuation={}|certificate_continuation={}|staleness_continuation={}",
        frame.scheduled_tolerance,
        frame.first_pass_tolerance,
        frame.effective_tolerance,
        frame.first_pass_iterations,
        frame.continuation_iterations,
        frame.first_pass_recurrence_residual,
        frame.recurrence_residual,
        frame.first_certificate_relative_infinity_norm,
        frame.finite_certificate_max,
        frame.fixed_effect_certificate_max,
        frame.max_abs_log_hessian_weight_change,
        u8::from(frame.weight_change_valid),
        u8::from(frame.predictor_used),
        u8::from(frame.predictor_refreshed_preconditioner),
        u8::from(frame.continuation.recurrence),
        u8::from(frame.continuation.certificate),
        u8::from(frame.continuation.staleness),
    )
}

fn emit_joint_lsmr_frame_trace(frame: JointLsmrFrameDiagnostic) {
    if !crate::diagnostics::solver_trace_enabled() {
        return;
    }
    let line = joint_lsmr_frame_trace_line(frame);
    crate::diagnostics::emit_solver_trace(&line);
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct JointLsmrFirstPassPredictor {
    prior_certified_frame: Option<JointLsmrFrameDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct JointLsmrFirstPassPolicy {
    tolerance: f64,
    used_predictor: bool,
    refresh_preconditioner: bool,
}

impl JointLsmrFirstPassPredictor {
    fn policy(
        &self,
        scheduled_tolerance: f64,
        weight_change_valid: bool,
        max_abs_log_weight_change: f64,
        preconditioner_policy: CategoricalLsmrPolicy,
    ) -> JointLsmrFirstPassPolicy {
        let fallback = JointLsmrFirstPassPolicy {
            tolerance: scheduled_tolerance,
            used_predictor: false,
            refresh_preconditioner: false,
        };
        if preconditioner_policy != CategoricalLsmrPolicy::CorePair {
            return fallback;
        }
        let Some(prior) = self.prior_certified_frame else {
            return fallback;
        };
        let schedule_ratio = scheduled_tolerance / prior.scheduled_tolerance;
        if !weight_change_valid
            || !max_abs_log_weight_change.is_finite()
            || max_abs_log_weight_change > JOINT_LSMR_PREDICTOR_MAX_LOG_WEIGHT_CHANGE
            || !prior.scheduled_tolerance.is_finite()
            || prior.scheduled_tolerance <= 0.0
            || !prior.effective_tolerance.is_finite()
            || prior.effective_tolerance <= 0.0
            || prior.effective_tolerance > prior.scheduled_tolerance
            || !prior.first_certificate_relative_infinity_norm.is_finite()
            || prior.first_certificate_relative_infinity_norm < 0.0
            || !schedule_ratio.is_finite()
            || !(JOINT_LSMR_PREDICTOR_MIN_SCHEDULE_RATIO..=1.0).contains(&schedule_ratio)
        {
            return fallback;
        }

        // Only an independently checked prior miss can tighten the recurrence.
        // Passing recurrence diagnostics never substitute for that certificate.
        let mut scale = 1.0_f64;
        if prior.continuation.recurrence || prior.continuation.certificate {
            scale = scale.min(prior.effective_tolerance / prior.scheduled_tolerance);
            if prior.first_certificate_relative_infinity_norm > 0.0 {
                scale = scale.min(
                    JOINT_LSMR_PREDICTOR_CERTIFICATE_TARGET * prior.scheduled_tolerance
                        / prior.first_certificate_relative_infinity_norm,
                );
            }
        }
        let lower = JOINT_LSMR_NUMERICAL_FLOOR.min(scheduled_tolerance);
        let tolerance = (scheduled_tolerance * scale).clamp(lower, scheduled_tolerance);
        let prior_iterations = prior
            .first_pass_iterations
            .saturating_add(prior.continuation_iterations);
        JointLsmrFirstPassPolicy {
            tolerance,
            used_predictor: true,
            // In the measured sparse-three-FE regime, a long certified frame
            // predicts that the retained two-way core will trip staleness on
            // the next nearby Hessian. Build the same factor from current
            // weights before recurrence instead of discarding a short pass.
            refresh_preconditioner: prior.continuation.staleness
                || prior_iterations >= JOINT_LSMR_CORE_PAIR_REFRESH_ITERATIONS,
        }
    }

    fn record_certified(&mut self, frame: JointLsmrFrameDiagnostic) {
        self.prior_certified_frame = Some(frame);
    }

    fn invalidate(&mut self) {
        self.prior_certified_frame = None;
    }
}

/// Fit-lifetime state for an IRLS Newton correction. A strict solve can witness
/// convergence, but canonical projection still owns final rank and inference.
pub(crate) struct ReusableCategoricalJointLsmr {
    team: InstalledThreadTeam,
    active_design: DenseMatrix,
    active_slots: Vec<usize>,
    topology: FeTopologyIdentity,
    effect_offsets: Vec<usize>,
    core_effects: Option<[usize; 2]>,
    sqrt_weights: Vec<f64>,
    rhs: Vec<f64>,
    target: Vec<f64>,
    delta_eta: Vec<f64>,
    residual: Vec<f64>,
    warm_start: Vec<f64>,
    finite_inverse: Vec<f64>,
    finite_factor: Vec<f64>,
    finite_factor_ready: bool,
    fe_inverse: Vec<f64>,
    finite_certificate: Vec<DirectionCertificate>,
    fe_certificate: Vec<DirectionCertificate>,
    warm_start_valid: bool,
    first_pass_predictor: JointLsmrFirstPassPredictor,
    preconditioner: Option<Preconditioner>,
    core_pair_bridge_scratch: Mutex<CorePairBridgeScratch>,
    policy: CategoricalLsmrPolicy,
    #[cfg(test)]
    instrumentation: JointLsmrInstrumentation,
}

#[derive(Default)]
struct CorePairBridgeScratch {
    core_rhs: Vec<f64>,
    core_correction: Vec<f64>,
}

impl CorePairBridgeScratch {
    fn new(core_width: usize) -> Result<Self> {
        Ok(Self {
            core_rhs: fallible_filled(
                core_width,
                0.0,
                "joint PPML core-pair bridge right-hand side",
            )?,
            core_correction: fallible_filled(
                core_width,
                0.0,
                "joint PPML core-pair bridge correction",
            )?,
        })
    }
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

        let topology = FeTopologyIdentity::capture(fe_design);
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
        let core_effects = if policy == CategoricalLsmrPolicy::CorePair {
            Some(largest_retained_core_pair(fe_design)?)
        } else {
            None
        };
        let core_pair_bridge_scratch = if let Some(core_effects) = core_effects {
            let core_width = core_effects.iter().try_fold(0_usize, |total, &effect| {
                total
                    .checked_add(fe_design.effects()[effect].n_levels())
                    .ok_or_else(|| allocation("joint PPML core-pair width overflows"))
            })?;
            CorePairBridgeScratch::new(core_width)?
        } else {
            CorePairBridgeScratch::default()
        };
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
            core_effects,
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
            finite_factor: fallible_filled(
                active_generated
                    .len()
                    .checked_mul(active_generated.len())
                    .ok_or_else(|| allocation("joint PPML finite factor dimensions overflow"))?,
                0.0,
                "joint PPML finite factor",
            )?,
            finite_factor_ready: false,
            fe_inverse: fallible_filled(
                fe_coefficients,
                0.0,
                "joint PPML FE diagonal preconditioner",
            )?,
            finite_certificate: fallible_filled(
                active_generated.len(),
                DirectionCertificate::default(),
                "joint PPML finite certificate scratch",
            )?,
            fe_certificate: fallible_filled(
                fe_coefficients,
                DirectionCertificate::default(),
                "joint PPML FE certificate scratch",
            )?,
            warm_start_valid: false,
            first_pass_predictor: JointLsmrFirstPassPredictor::default(),
            preconditioner: None,
            core_pair_bridge_scratch: Mutex::new(core_pair_bridge_scratch),
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
        let setup_started = Instant::now();
        let weight_change_valid = self.warm_start_valid;
        let max_abs_log_hessian_weight_change = prepare_frame_and_finite_diagonal(
            &self.active_design,
            outcome,
            mean,
            hessian_weights,
            &mut self.sqrt_weights,
            &mut self.target,
            &mut self.rhs,
            &mut self.finite_inverse,
            weight_change_valid,
        )?;
        self.finite_factor_ready = prepare_finite_factor(
            &self.active_design,
            hessian_weights,
            &self.finite_inverse,
            &mut self.finite_factor,
        );
        let warm_start_attempted = self.warm_start_valid;
        let first_pass_policy = self.first_pass_predictor.policy(
            tolerance,
            weight_change_valid,
            max_abs_log_hessian_weight_change,
            self.policy,
        );
        let (mut use_schwarz, mut rebuilt) =
            self.prepare_preconditioner(fe_design, hessian_weights)?;
        if first_pass_policy.refresh_preconditioner
            && !rebuilt
            && self.policy != CategoricalLsmrPolicy::Diagonal
        {
            use_schwarz = self.rebuild_schwarz(fe_design, hessian_weights)?;
            rebuilt = true;
            if !use_schwarz {
                fill_fe_diagonal_inverse(fe_design, hessian_weights, &mut self.fe_inverse)?;
            }
        }
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
        // An unfactored two-effect frame starts with the current diagonal.
        // Stalled progress promotes it to Schwarz using the same continuation
        // as a stale factor. A newly built factor cannot be refreshed again.
        let use_escalation = self.policy != CategoricalLsmrPolicy::Diagonal && !rebuilt;
        #[cfg(test)]
        if use_escalation {
            self.instrumentation.escalation_armed =
                self.instrumentation.escalation_armed.saturating_add(1);
        }
        let first_tolerance = first_pass_policy.tolerance;
        let solve_started = Instant::now();
        let first = {
            let block_preconditioner =
                self.block_preconditioner(fe_design, hessian_weights, use_schwarz)?;
            self.run_once(
                fe_design,
                &block_preconditioner,
                first_tolerance,
                max_iterations,
                self.warm_start_valid.then_some(self.warm_start.as_slice()),
                use_escalation.then_some(&staleness as _),
                None,
            )?
        };
        solve_seconds += solve_started.elapsed().as_secs_f64();
        let first_pass_iterations = first.iterations;
        let first_pass_recurrence_residual = first.normal_eq_residual;
        let mut result = first;
        let mut iterations = result.iterations;
        let mut continuation_iterations = 0_usize;
        let mut effective_tolerance = first_tolerance;
        let mut continuation = JointLsmrContinuationCause::default();
        if result.stop_reason == LsmrStopReason::Escalated {
            continuation.staleness = true;
            // A diagonal pilot has no certified state in the new metric.
            // Carrying its numerical null-space drift into a stronger factor
            // can defeat even a tight recurrence and fresh certificate.
            let diagonal_promotion = !use_schwarz;
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
                let block_preconditioner =
                    self.block_preconditioner(fe_design, hessian_weights, use_schwarz)?;
                self.run_once(
                    fe_design,
                    &block_preconditioner,
                    first_tolerance,
                    max_iterations,
                    (!diagonal_promotion).then_some(result.x.as_slice()),
                    None,
                    Some(4),
                )?
            };
            solve_seconds += solve_started.elapsed().as_secs_f64();
            continuation_iterations = continuation_iterations
                .checked_add(continued.iterations)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "joint PPML LSMR continuation iteration count overflowed",
                    )
                })?;
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
        let first_certificate_relative_infinity_norm = certificate.relative_infinity_norm;
        // Recurrence termination and the independent normal-equation
        // certificate are separate obligations. Continue from the first
        // iterate when either one is missing; a certificate that happens to
        // pass cannot turn an exhausted recurrence into convergence.
        if !result.converged || !certificate.passed {
            continuation.recurrence = !result.converged;
            continuation.certificate = !certificate.passed;
            let diagonal_promotion =
                !use_schwarz && !rebuilt && self.policy != CategoricalLsmrPolicy::Diagonal;
            if !rebuilt && self.policy != CategoricalLsmrPolicy::Diagonal {
                let rebuild_started = Instant::now();
                use_schwarz = self.rebuild_schwarz(fe_design, hessian_weights)?;
                rebuilt = true;
                if !use_schwarz {
                    fill_fe_diagonal_inverse(fe_design, hessian_weights, &mut self.fe_inverse)?;
                }
                setup_seconds += rebuild_started.elapsed().as_secs_f64();
            }
            let numerical_floor = JOINT_LSMR_NUMERICAL_FLOOR.min(tolerance);
            effective_tolerance = first_tolerance.min((tolerance * 0.01).max(numerical_floor));
            retries = retries.checked_add(1).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "joint PPML LSMR retry count overflowed",
                )
            })?;
            let solve_started = Instant::now();
            let continued = {
                let block_preconditioner =
                    self.block_preconditioner(fe_design, hessian_weights, use_schwarz)?;
                self.run_once(
                    fe_design,
                    &block_preconditioner,
                    effective_tolerance,
                    max_iterations,
                    (!diagonal_promotion).then_some(result.x.as_slice()),
                    None,
                    Some(4),
                )?
            };
            solve_seconds += solve_started.elapsed().as_secs_f64();
            continuation_iterations = continuation_iterations
                .checked_add(continued.iterations)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "joint PPML LSMR continuation iteration count overflowed",
                    )
                })?;
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

        let frame = JointLsmrFrameDiagnostic {
            scheduled_tolerance: tolerance,
            first_pass_tolerance: first_tolerance,
            effective_tolerance,
            first_pass_iterations,
            continuation_iterations,
            first_pass_recurrence_residual,
            recurrence_residual: result.normal_eq_residual,
            first_certificate_relative_infinity_norm,
            finite_certificate_max: certificate.finite_absolute_infinity_norm,
            fixed_effect_certificate_max: certificate.fixed_effect_absolute_infinity_norm,
            max_abs_log_hessian_weight_change,
            weight_change_valid,
            predictor_used: first_pass_policy.used_predictor,
            predictor_refreshed_preconditioner: first_pass_policy.refresh_preconditioner,
            continuation,
        };
        self.first_pass_predictor.record_certified(frame);
        emit_joint_lsmr_frame_trace(frame);

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
            frame,
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
        self.first_pass_predictor.invalidate();
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
        mlsmr_cancellable(
            &operator,
            &self.rhs,
            preconditioner,
            tolerance,
            max_iterations,
            MlsmrOptions {
                warm_start,
                escalation: crate::fe::lsmr_break::policy(
                    crate::fe::DeveloperLsmrBackend::PpmlJoint,
                )
                .or(escalation),
                local_size,
            },
            schwarz_precond::Cancellation::new(&crate::interrupt::cancellation_requested),
        )
        .map_err(|error| {
            if matches!(error, SolveError::Cancelled) {
                crate::fe::lsmr_break::record_cancelled(crate::fe::DeveloperLsmrBackend::PpmlJoint);
            }
            lsmr_error(error)
        })
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
        if self.policy == CategoricalLsmrPolicy::Diagonal
            || (self.policy == CategoricalLsmrPolicy::Reuse && self.preconditioner.is_none())
        {
            fill_fe_diagonal_inverse(fe_design, weights, &mut self.fe_inverse)?;
            return Ok((false, false));
        }
        if self.policy == CategoricalLsmrPolicy::CorePair {
            // The graph core covers only the selected two effect blocks. Every
            // additional coordinate retains a current-weight positive
            // diagonal action, so the combined preconditioner spans the full
            // augmented FE axis without an uncovered null direction.
            fill_fe_diagonal_inverse(fe_design, weights, &mut self.fe_inverse)?;
        }
        // Retain the factor by default. The fit-owned first-pass predictor may
        // refresh a core-pair factor before recurrence, but only after a
        // certified nearby frame supplies the measured conditioning signal.
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
        let effects = if self.policy == CategoricalLsmrPolicy::CorePair {
            let core_effects = self.core_effects.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "joint PPML core-pair route has no selected effects",
                )
            })?;
            within_effects_selected(fe_design, &core_effects)?
        } else {
            within_effects(fe_design)?
        };
        // Explicit additive policy and repeatable owner reduction in every mode.
        let config = crate::fe::additive_schwarz();
        let solver = Solver::new_cancellable(
            effects,
            Some(weights),
            config,
            within::Cancellation::new(&crate::interrupt::cancellation_requested),
        )
        .map_err(|error| {
            if let Err(error) = crate::interrupt::checkpoint() {
                return error;
            }
            FeregError::new(
                if matches!(error, within::BuildError::Cancelled) {
                    ErrorCode::Interrupted
                } else {
                    ErrorCode::InvalidData
                },
                format!("unable to build joint PPML Schwarz preconditioner: {error}"),
            )
        })?;
        self.preconditioner = solver.preconditioner().cloned();
        Ok(self.preconditioner.is_some())
    }

    fn block_preconditioner<'a>(
        &'a self,
        fe_design: &'a FeDesign,
        weights: &'a [f64],
        use_schwarz: bool,
    ) -> Result<BlockPreconditioner<'a>> {
        let fe = if use_schwarz {
            let preconditioner = self.preconditioner.as_ref().ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "joint PPML Schwarz route has no retained preconditioner",
                )
            })?;
            if self.policy == CategoricalLsmrPolicy::CorePair {
                FePreconditioner::CorePair {
                    core: preconditioner,
                    core_effects: self.core_effects.ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::Internal,
                            "joint PPML core-pair route has no selected effects",
                        )
                    })?,
                    diagonal: &self.fe_inverse,
                    design: fe_design,
                    effect_offsets: &self.effect_offsets,
                    weights,
                    scratch: &self.core_pair_bridge_scratch,
                }
            } else {
                FePreconditioner::Schwarz(preconditioner)
            }
        } else {
            FePreconditioner::Diagonal(&self.fe_inverse)
        };
        Ok(BlockPreconditioner {
            finite_inverse: &self.finite_inverse,
            finite_factor: self
                .finite_factor_ready
                .then_some(self.finite_factor.as_slice()),
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
        certify_joint_with_scratch(
            fe_design,
            &self.active_design,
            weights,
            &self.target,
            &self.residual,
            tolerance,
            &mut self.finite_certificate,
            &mut self.fe_certificate,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_frame_and_finite_diagonal(
    active: &DenseMatrix,
    outcome: &[f64],
    mean: &[f64],
    weights: &[f64],
    sqrt_weights: &mut [f64],
    target: &mut [f64],
    rhs: &mut [f64],
    finite_inverse: &mut [f64],
    prior_hessian_available: bool,
) -> Result<f64> {
    let rows = active.nrows();
    let columns = active.ncols();
    if [
        outcome.len(),
        mean.len(),
        weights.len(),
        sqrt_weights.len(),
        target.len(),
        rhs.len(),
    ]
    .iter()
    .any(|length| *length != rows)
        || finite_inverse.len() != columns
    {
        return Err(invalid_argument(
            "joint PPML frame preparation dimensions are inconsistent",
        ));
    }

    finite_inverse.fill(0.0);
    let mut max_abs_log_hessian_weight_change = 0.0_f64;
    let active_values = active.as_slice();
    for row in 0..rows {
        let root_weight = weights[row].sqrt();
        if prior_hessian_available {
            let prior_root_weight = sqrt_weights[row];
            if !prior_root_weight.is_finite() || prior_root_weight <= 0.0 {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "joint PPML prior Hessian weight state is invalid",
                ));
            }
            let log_change = 2.0 * (root_weight / prior_root_weight).ln().abs();
            if !log_change.is_finite() {
                return Err(nonconvergence(
                    "joint PPML Hessian weight transition is nonfinite",
                ));
            }
            max_abs_log_hessian_weight_change = max_abs_log_hessian_weight_change.max(log_change);
        }
        let correction = if outcome[row] == 0.0 {
            -1.0
        } else {
            outcome[row] / mean[row] - 1.0
        };
        let weighted_correction = root_weight * correction;
        if !root_weight.is_finite() || !correction.is_finite() || !weighted_correction.is_finite() {
            return Err(nonconvergence(
                "joint PPML Newton target is not finitely representable",
            ));
        }
        sqrt_weights[row] = root_weight;
        target[row] = correction;
        rhs[row] = weighted_correction;

        let start = row * columns;
        for (column, &value) in active_values[start..start + columns].iter().enumerate() {
            finite_inverse[column] = (weights[row] * value).mul_add(value, finite_inverse[column]);
        }
    }
    for inverse in finite_inverse {
        *inverse = if *inverse > 0.0 { 1.0 / *inverse } else { 0.0 };
        if !inverse.is_finite() {
            return Err(nonconvergence(
                "joint PPML finite diagonal preconditioner is nonfinite",
            ));
        }
    }
    Ok(max_abs_log_hessian_weight_change)
}

// The optional finite preconditioner is rebuilt for every current-weight call.
// A nonfinite or nearly singular standardized Gram declines to the original
// diagonal; it cannot drop a control or change the canonical kept basis.
fn prepare_finite_factor(
    active: &DenseMatrix,
    weights: &[f64],
    inverse_diagonal: &[f64],
    factor: &mut [f64],
) -> bool {
    let k = active.ncols();
    if k < 2
        || factor.len() != k.saturating_mul(k)
        || inverse_diagonal.len() != k
        || weights.len() != active.nrows()
        || inverse_diagonal
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return false;
    }
    factor
        .par_chunks_mut(k)
        .enumerate()
        .for_each(|(i, destination)| {
            destination.fill(0.0);
            let scale_i = inverse_diagonal[i].sqrt();
            for (row, &weight) in weights.iter().enumerate() {
                let values = &active.as_slice()[row * k..(row + 1) * k];
                let left = weight * values[i] * scale_i;
                for j in 0..i {
                    destination[j] = left.mul_add(values[j], destination[j]);
                }
            }
            for j in 0..i {
                destination[j] *= inverse_diagonal[j].sqrt();
            }
            destination[i] = 1.0;
        });
    for i in 0..k {
        for j in 0..=i {
            let mut value = factor[i * k + j];
            for a in 0..j {
                value -= factor[i * k + a] * factor[j * k + a];
            }
            if !value.is_finite() {
                return false;
            }
            if i == j {
                if value <= 32.0 * f64::EPSILON {
                    return false;
                }
                factor[i * k + j] = value.sqrt();
            } else {
                factor[i * k + j] = value / factor[j * k + j];
            }
        }
    }
    true
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

// These products belong only to speculative LSMR applications. Reconstruction
// and original-input certification retain their separate arithmetic. Portable
// builds avoid an out-of-line libm call per entry; hardware FMA targets keep it.
#[inline]
fn proposal_product_add(left: f64, right: f64, sum: f64) -> f64 {
    #[cfg(any(target_arch = "aarch64", target_feature = "fma"))]
    {
        left.mul_add(right, sum)
    }
    #[cfg(not(any(target_arch = "aarch64", target_feature = "fma")))]
    {
        left * right + sum
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
            let values = &self.active_design.as_slice()[row * finite..(row + 1) * finite];
            for (&value, &coefficient) in values.iter().zip(&x[..finite]) {
                fitted = proposal_product_add(value, coefficient, fitted);
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
        let parallel_column_limit =
            rayon::current_num_threads().saturating_mul(ADJOINT_COLUMN_LANES_PER_THREAD);
        if finite <= parallel_column_limit {
            finite_output
                .par_iter_mut()
                .enumerate()
                .for_each(|(column, output)| {
                    let mut sums = [0.0; 8];
                    let values = self.active_design.as_slice();
                    let full = x.len() / 8 * 8;
                    for row in (0..full).step_by(8) {
                        // Keep the legacy matrix indices and numerical traversal order explicit.
                        #[allow(clippy::needless_range_loop)]
                        for lane in 0..8 {
                            let r = row + lane;
                            sums[lane] = proposal_product_add(
                                self.sqrt_weights[r] * values[r * finite + column],
                                x[r],
                                sums[lane],
                            );
                        }
                    }
                    let mut sum = sums.into_iter().sum();
                    for row in full..x.len() {
                        sum = proposal_product_add(
                            self.sqrt_weights[row] * values[row * finite + column],
                            x[row],
                            sum,
                        );
                    }
                    *output = sum;
                });
        } else {
            finite_output.fill(0.0);
            let active_values = self.active_design.as_slice();
            for (row, &input) in x.iter().enumerate() {
                let start = row * finite;
                for (column, &value) in active_values[start..start + finite].iter().enumerate() {
                    finite_output[column] = proposal_product_add(
                        self.sqrt_weights[row] * value,
                        input,
                        finite_output[column],
                    );
                }
            }
        }
        for (effect_index, effect) in self.fe_design.effects().iter().enumerate() {
            let start = self.effect_offsets[effect_index];
            let end = self.effect_offsets[effect_index + 1];
            fe_output[start..end]
                .par_chunks_mut(ADJOINT_LEVEL_CHUNK)
                .enumerate()
                .for_each(|(chunk_index, outputs)| {
                    let first_level = chunk_index * ADJOINT_LEVEL_CHUNK;
                    for (within_chunk, output) in outputs.iter_mut().enumerate() {
                        let level = first_level + within_chunk;
                        let mut sum = 0.0_f64;
                        for &row in effect.level_index().members(level) {
                            sum = proposal_product_add(self.sqrt_weights[row], x[row], sum);
                        }
                        *output = sum;
                    }
                });
        }
        Ok(())
    }
}

enum FePreconditioner<'a> {
    Diagonal(&'a [f64]),
    Schwarz(&'a Preconditioner),
    CorePair {
        core: &'a Preconditioner,
        core_effects: [usize; 2],
        diagonal: &'a [f64],
        design: &'a FeDesign,
        effect_offsets: &'a [usize],
        weights: &'a [f64],
        scratch: &'a Mutex<CorePairBridgeScratch>,
    },
}

struct BlockPreconditioner<'a> {
    finite_inverse: &'a [f64],
    finite_factor: Option<&'a [f64]>,
    fe: FePreconditioner<'a>,
}

impl Operator for BlockPreconditioner<'_> {
    fn nrows(&self) -> usize {
        self.finite_inverse.len()
            + match &self.fe {
                FePreconditioner::Diagonal(values) => values.len(),
                FePreconditioner::Schwarz(value) => value.nrows(),
                FePreconditioner::CorePair { diagonal, .. } => diagonal.len(),
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
        if let Some(factor) = self.finite_factor {
            // H = S^-1 L L' S^-1. Applying S L'^-1 L^-1 S is symmetric
            // positive definite; no finite-rank or inference authority is given
            // to this ordinary-direction preconditioner.
            for i in 0..finite {
                let mut value = x[i] * self.finite_inverse[i].sqrt();
                for j in 0..i {
                    value -= factor[i * finite + j] * y[j];
                }
                y[i] = value / factor[i * finite + i];
            }
            for i in (0..finite).rev() {
                let mut value = y[i];
                for j in i + 1..finite {
                    value -= factor[j * finite + i] * y[j];
                }
                y[i] = value / factor[i * finite + i];
            }
            // Keep the legacy matrix indices and numerical traversal order explicit.
            #[allow(clippy::needless_range_loop)]
            for i in 0..finite {
                y[i] *= self.finite_inverse[i].sqrt();
            }
        } else {
            for column in 0..finite {
                y[column] = self.finite_inverse[column] * x[column];
            }
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
            FePreconditioner::CorePair {
                core,
                core_effects,
                diagonal,
                design,
                effect_offsets,
                weights,
                scratch,
            } => {
                let core_width = core.nrows();
                if core_width > diagonal.len() {
                    return Err(solve_shape_error("core-pair preconditioner apply"));
                }
                apply_core_pair_bridge(
                    core,
                    *core_effects,
                    diagonal,
                    design,
                    effect_offsets,
                    weights,
                    scratch,
                    &x[finite..],
                    &mut y[finite..],
                )
            }
        }
    }

    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        self.apply(x, y)
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_core_pair_bridge(
    core: &Preconditioner,
    core_effects: [usize; 2],
    diagonal: &[f64],
    design: &FeDesign,
    effect_offsets: &[usize],
    weights: &[f64],
    scratch: &Mutex<CorePairBridgeScratch>,
    x: &[f64],
    y: &mut [f64],
) -> std::result::Result<(), SolveError> {
    let core_width = core.nrows();
    let [first_core, second_core] = core_effects;
    if core.ncols() != core_width
        || design.effects().len() < 2
        || first_core == second_core
        || first_core >= design.effects().len()
        || second_core >= design.effects().len()
        || effect_offsets.len() != design.effects().len() + 1
        || design.effects()[first_core]
            .n_levels()
            .checked_add(design.effects()[second_core].n_levels())
            != Some(core_width)
        || effect_offsets.last().copied() != Some(diagonal.len())
        || weights.len() != design.nobs()
        || x.len() != diagonal.len()
        || y.len() != diagonal.len()
    {
        return Err(solve_shape_error("core-pair bridge apply"));
    }

    let effects = design.effects();
    let mut bridge = scratch.lock().map_err(|_| SolveError::InvalidInput {
        context: "joint PPML core-pair bridge scratch",
        message: "synchronization failure: scratch mutex is poisoned".to_owned(),
    })?;
    if bridge.core_rhs.len() != core_width || bridge.core_correction.len() != core_width {
        return Err(solve_shape_error("core-pair bridge scratch"));
    }
    let mut packed_start = 0_usize;
    for effect_index in core_effects {
        let global_start = effect_offsets[effect_index];
        let width = effects[effect_index].n_levels();
        bridge.core_rhs[packed_start..packed_start + width]
            .copy_from_slice(&x[global_start..global_start + width]);
        packed_start += width;
    }
    let CorePairBridgeScratch {
        core_rhs,
        core_correction,
    } = &mut *bridge;
    Operator::apply(core, core_rhs, core_correction)?;
    packed_start = 0;
    for effect_index in core_effects {
        let global_start = effect_offsets[effect_index];
        let width = effects[effect_index].n_levels();
        y[global_start..global_start + width]
            .copy_from_slice(&core_correction[packed_start..packed_start + width]);
        packed_start += width;
    }

    if effects.len() == 2 {
        return Ok(());
    }

    // This is the symmetric block factor T diag(C, D) T', where C is the
    // retained core action and T's off-diagonal block is -C B. Apply
    // D(x_e - B' C x_c) first. Each output level owns its canonical member
    // order, so the exact cross-block correction remains deterministic across
    // thread counts without retaining another pairwise graph.
    let second_packed_start = effects[first_core].n_levels();
    for effect_index in 0..effects.len() {
        if core_effects.contains(&effect_index) {
            continue;
        }
        let global_start = effect_offsets[effect_index];
        let global_end = effect_offsets[effect_index + 1];
        y[global_start..global_end]
            .par_chunks_mut(ADJOINT_LEVEL_CHUNK)
            .enumerate()
            .for_each(|(chunk_index, outputs)| {
                let first_level = chunk_index * ADJOINT_LEVEL_CHUNK;
                for (within_chunk, output) in outputs.iter_mut().enumerate() {
                    let level = first_level + within_chunk;
                    let mut cross = 0.0_f64;
                    for &row in effects[effect_index].level_index().members(level) {
                        let core_predictor = core_correction
                            [effects[first_core].codes()[row] as usize]
                            + core_correction
                                [second_packed_start + effects[second_core].codes()[row] as usize];
                        cross = weights[row].mul_add(core_predictor, cross);
                    }
                    *output = diagonal[global_start + level] * (x[global_start + level] - cross);
                }
            });
    }

    // Form B e directly from the borrowed memberships and current frame
    // weights. The retained core remains the sole graph preconditioner.
    packed_start = 0;
    for effect_index in core_effects {
        let width = effects[effect_index].n_levels();
        core_rhs[packed_start..packed_start + width]
            .par_chunks_mut(ADJOINT_LEVEL_CHUNK)
            .enumerate()
            .for_each(|(chunk_index, outputs)| {
                let first_level = chunk_index * ADJOINT_LEVEL_CHUNK;
                for (within_chunk, output) in outputs.iter_mut().enumerate() {
                    let level = first_level + within_chunk;
                    let mut cross = 0.0_f64;
                    for &row in effects[effect_index].level_index().members(level) {
                        let mut extra_predictor = 0.0_f64;
                        for extra_effect in 0..effects.len() {
                            if core_effects.contains(&extra_effect) {
                                continue;
                            }
                            let extra_index = effect_offsets[extra_effect]
                                + effects[extra_effect].codes()[row] as usize;
                            extra_predictor += y[extra_index];
                        }
                        cross = weights[row].mul_add(extra_predictor, cross);
                    }
                    *output = cross;
                }
            });
        packed_start += width;
    }
    Operator::apply(core, core_rhs, core_correction)?;
    packed_start = 0;
    for effect_index in core_effects {
        let global_start = effect_offsets[effect_index];
        let width = effects[effect_index].n_levels();
        for (output, correction) in y[global_start..global_start + width]
            .iter_mut()
            .zip(&core_correction[packed_start..packed_start + width])
        {
            *output -= *correction;
        }
        packed_start += width;
    }
    Ok(())
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
        block
            .par_chunks_mut(ADJOINT_LEVEL_CHUNK)
            .enumerate()
            .for_each(|(chunk_index, outputs)| {
                let first_level = chunk_index * ADJOINT_LEVEL_CHUNK;
                for (within_chunk, value) in outputs.iter_mut().enumerate() {
                    let level = first_level + within_chunk;
                    let mut diagonal = 0.0_f64;
                    for &row in effect.level_index().members(level) {
                        diagonal += weights[row];
                    }
                    *value = if diagonal > 0.0 { 1.0 / diagonal } else { 0.0 };
                }
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

fn within_effects_selected<'a>(
    design: &'a FeDesign,
    effect_indices: &[usize],
) -> Result<Vec<Effect<'a>>> {
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(effect_indices.len())
        .map_err(|_| allocation("unable to reserve selected joint PPML within effects"))?;
    for &effect_index in effect_indices {
        let effect = design
            .effects()
            .get(effect_index)
            .ok_or_else(|| invalid_argument("joint PPML selected effect index is out of range"))?;
        effects.push(
            Effect::new(effect.codes(), true, std::iter::empty::<&[f64]>()).map_err(|error| {
                FeregError::new(
                    ErrorCode::InvalidData,
                    format!("unable to build selected joint PPML categorical effect: {error}"),
                )
            })?,
        );
    }
    Ok(effects)
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct DirectionCertificate {
    moment: f64,
    basis_norm: f64,
}

#[cfg(test)]
fn certify_joint(
    design: &FeDesign,
    active: &DenseMatrix,
    weights: &[f64],
    target: &[f64],
    residual: &[f64],
    tolerance: f64,
) -> Result<JointLsmrCertificate> {
    let mut finite_scratch = fallible_filled(
        active.ncols(),
        DirectionCertificate::default(),
        "joint PPML finite certificate test scratch",
    )?;
    let mut fe_scratch = fallible_filled(
        design.total_coefficients(),
        DirectionCertificate::default(),
        "joint PPML FE certificate test scratch",
    )?;
    certify_joint_with_scratch(
        design,
        active,
        weights,
        target,
        residual,
        tolerance,
        &mut finite_scratch,
        &mut fe_scratch,
    )
}

#[allow(clippy::too_many_arguments)]
fn certify_joint_with_scratch(
    design: &FeDesign,
    active: &DenseMatrix,
    weights: &[f64],
    target: &[f64],
    residual: &[f64],
    tolerance: f64,
    finite_scratch: &mut [DirectionCertificate],
    fe_scratch: &mut [DirectionCertificate],
) -> Result<JointLsmrCertificate> {
    if active.nrows() != design.nobs()
        || [weights.len(), target.len(), residual.len()]
            .iter()
            .any(|length| *length != design.nobs())
        || finite_scratch.len() != active.ncols()
        || fe_scratch.len() != design.total_coefficients()
    {
        return Err(invalid_argument(
            "joint PPML LSMR certificate dimensions are inconsistent",
        ));
    }
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
    finite_scratch.par_iter_mut().enumerate().try_for_each(
        |(column, direction)| -> Result<()> {
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
            *direction = DirectionCertificate {
                moment: moment.total()?,
                basis_norm: basis_square.norm()?,
            };
            Ok(())
        },
    )?;
    for direction in finite_scratch {
        let moment = direction.moment;
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative(
            moment,
            direction.basis_norm,
            reference,
            &mut relative_infinity,
            &mut relative_square,
            &mut count,
        )?;
    }
    let mut offset = 0_usize;
    for effect in design.effects() {
        let block = &mut fe_scratch[offset..offset + effect.n_levels()];
        block
            .par_chunks_mut(ADJOINT_LEVEL_CHUNK)
            .enumerate()
            .try_for_each(|(chunk_index, directions)| -> Result<()> {
                let first_level = chunk_index * ADJOINT_LEVEL_CHUNK;
                for (within_chunk, direction) in directions.iter_mut().enumerate() {
                    let level = first_level + within_chunk;
                    let mut moment = FiniteCompensatedSum::default();
                    let mut basis_square = ScaledSquareSum::default();
                    for &row in effect.level_index().members(level) {
                        let root_weight = weights[row].sqrt();
                        let (moment_term, weighted_basis) =
                            finite_weighted_moment(weights[row], residual[row], 1.0, root_weight)?;
                        moment.add(moment_term)?;
                        basis_square.add(weighted_basis)?;
                    }
                    *direction = DirectionCertificate {
                        moment: moment.total()?,
                        basis_norm: basis_square.norm()?,
                    };
                }
                Ok(())
            })?;
        offset += effect.n_levels();
    }
    for direction in fe_scratch {
        let moment = direction.moment;
        fe_absolute = fe_absolute.max(moment.abs());
        add_relative(
            moment,
            direction.basis_norm,
            reference,
            &mut relative_infinity,
            &mut relative_square,
            &mut count,
        )?;
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
    let relative = super::certificate::relative_moment(moment, basis_norm, reference);
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
        neumaier_add(value, &mut self.sum, &mut self.correction);
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
/// only for a policy that can build or reuse it. `yielding` is an optional
/// exact count of the yielding pair components among the effects the policy's
/// Schwarz factor covers ([`schwarz_effects`]); `None` keeps the level-count
/// bound.
pub(crate) fn estimate_joint_lsmr_workspace_bytes(
    rows: usize,
    active_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
    policy: CategoricalLsmrPolicy,
    yielding: Option<usize>,
) -> Result<u128> {
    let shape = CategoricalLsmrMemoryShape::new(
        rows,
        active_columns,
        effect_level_counts,
        threads,
        policy,
        yielding,
    )?;
    let n = shape.rows;
    let k = shape.active_columns;
    let p = checked_add_memory(k, shape.levels, "joint PPML augmented width")?;
    let core_pair_bridge_bytes = core_pair_bridge_scratch_bytes(effect_level_counts, policy)?;

    // Fit-lifetime allocations owned directly by ReusableCategoricalJointLsmr.
    let retained_values = checked_sum_memory(&[
        checked_mul_memory(n, k, "joint PPML retained active design")?,
        checked_mul_memory(k, k, "joint PPML finite factor")?,
        checked_mul_memory(n, 5, "joint PPML retained observation buffers")?,
        p,
        k,
        shape.levels,
    ])?;
    let retained_indices = checked_sum_memory(&[
        k,
        checked_add_memory(shape.effects, 1, "joint PPML retained effect-offset count")?,
    ])?;
    let retained_engine = checked_sum_memory(&[
        bytes_for_memory::<f64>(retained_values, "joint PPML retained values")?,
        bytes_for_memory::<usize>(retained_indices, "joint PPML retained indices")?,
        bytes_for_memory::<DirectionCertificate>(
            checked_add_memory(k, shape.levels, "joint PPML certificate direction count")?,
            "joint PPML certificate scratch",
        )?,
        shape.schwarz.retained_bytes,
        core_pair_bridge_bytes,
    ])?;

    // The continuation path uses a four-vector local reorthogonalization
    // window. Modified Golub-Kahan and the solution recurrence own
    // 2N + (6 + 2w)P values; warm residuals reuse the observation workspace.
    // Keep the caller's prior P-vector and a second P-vector for an upstream
    // false-convergence restart while the corrected recurrence runs.
    const CONTINUATION_LOCAL_SIZE: u128 = 4;
    let recurrence_vectors = checked_add_memory(
        6,
        checked_mul_memory(CONTINUATION_LOCAL_SIZE, 2, "joint PPML local history lanes")?,
        "joint PPML recurrence vector count",
    )?;
    let recurrence_values = checked_sum_memory(&[
        checked_mul_memory(n, 2, "joint PPML recurrence observation vectors")?,
        checked_mul_memory(
            p,
            recurrence_vectors,
            "joint PPML recurrence coefficient vectors",
        )?,
        checked_mul_memory(p, 2, "joint PPML retained continuation results")?,
    ])?;
    let recurrence_peak = checked_sum_memory(&[
        bytes_for_memory::<f64>(recurrence_values, "joint PPML recurrence values")?,
        shape.schwarz.apply_peak_bytes,
    ])?;

    // A stale/certificate-triggered rebuild retains the previous solution
    // while `within::Solver::new` constructs square-root weights and may
    // locality-sort all categorical columns.
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

fn core_pair_bridge_scratch_bytes(
    effect_level_counts: &[usize],
    policy: CategoricalLsmrPolicy,
) -> Result<u128> {
    if policy != CategoricalLsmrPolicy::CorePair {
        return Ok(0);
    }
    if effect_level_counts.len() < 2 {
        return Err(invalid_argument(
            "PPML LSMR core-pair bridge memory estimate requires two effects",
        ));
    }
    let order = retained_effect_order_by_levels(effect_level_counts.len(), |effect| {
        effect_level_counts[effect]
    })?;
    let core_width = checked_add_memory(
        effect_level_counts[order[0]] as u128,
        effect_level_counts[order[1]] as u128,
        "PPML LSMR core-pair bridge width",
    )?;
    bytes_for_memory::<f64>(
        checked_mul_memory(core_width, 2, "PPML LSMR core-pair bridge scratch")?,
        "PPML LSMR core-pair bridge scratch bytes",
    )
}

/// Conservative transient peak for one canonical current-weight checkpoint.
///
/// The checkpoint batch intentionally rebuilds the weighted Schwarz solver so
/// no ordinary-frame preconditioner or residual can become scientific
/// evidence. The estimate therefore charges the full upstream solver frame,
/// all retained batch outputs, and one recurrence workspace per concurrent
/// RHS in addition to the caller-owned canonical session.
pub(crate) fn estimate_checkpoint_lsmr_workspace_bytes(
    rows: usize,
    rhs: usize,
    effect_level_counts: &[usize],
    threads: usize,
) -> Result<u128> {
    estimate_checkpoint_lsmr_workspace_bytes_with_yielding(
        rows,
        rhs,
        effect_level_counts,
        threads,
        None,
    )
}

/// As [`estimate_checkpoint_lsmr_workspace_bytes`], with an optional exact
/// yielding pair-component count over every effect pair.
pub(crate) fn estimate_checkpoint_lsmr_workspace_bytes_with_yielding(
    rows: usize,
    rhs: usize,
    effect_level_counts: &[usize],
    threads: usize,
    yielding: Option<usize>,
) -> Result<u128> {
    if rhs == 0 {
        return Err(invalid_argument(
            "PPML checkpoint LSMR memory estimate requires a positive RHS width",
        ));
    }
    // This is the same borrowed categorical batch adapter used by OLS. Share
    // its guarded recurrence windows, scatter, output overlap and Schwarz
    // estimates; nested Rayon alone does not bound live frames by workers.
    let solver = crate::memory::estimate_fe_solver_memory_with_yielding(
        rows,
        rhs,
        effect_level_counts,
        threads,
        Some(4),
        yielding,
    )?;
    checked_sum_memory(&[
        solver.lsmr_schwarz_bytes,
        // OLS charges its caller-owned input in the execution model. A PPML
        // checkpoint constructs this matrix on top of the retained session.
        bytes_for_memory::<f64>(
            checked_mul_memory(rows as u128, rhs as u128, "checkpoint LSMR input matrix")?,
            "checkpoint LSMR input matrix bytes",
        )?,
        bytes_for_memory::<f64>(rows as u128, "checkpoint LSMR certificate workspace")?,
    ])
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
    fn new(
        rows: usize,
        active_columns: usize,
        effect_level_counts: &[usize],
        threads: usize,
        policy: CategoricalLsmrPolicy,
        yielding: Option<usize>,
    ) -> Result<Self> {
        let yielding = yielding.map(|count| count as u128);
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
        let schwarz = match policy {
            CategoricalLsmrPolicy::Diagonal => CategoricalSchwarzMemoryEstimate::default(),
            CategoricalLsmrPolicy::Reuse => estimate_categorical_schwarz_memory(
                rows,
                effects,
                levels,
                maximum_levels,
                threads,
                checked_add_memory(
                    categorical_dense_cross_tab_bytes(rows, effect_level_counts)?,
                    crate::memory::schwarz_weighted_correction_bytes(rows, effect_level_counts)?,
                    "PPML LSMR cross-tab and compensation scratch",
                )?,
                yielding,
            )?,
            CategoricalLsmrPolicy::CorePair => {
                if effect_level_counts.len() < 2 {
                    return Err(invalid_argument(
                        "PPML LSMR core-pair memory estimate requires two effects",
                    ));
                }
                let order = retained_effect_order_by_levels(effect_level_counts.len(), |effect| {
                    effect_level_counts[effect]
                })?;
                let core_levels =
                    [order[0], order[1]]
                        .iter()
                        .try_fold(0_u128, |total, &effect| {
                            checked_add_memory(
                                total,
                                effect_level_counts[effect] as u128,
                                "PPML LSMR core-pair levels",
                            )
                        })?;
                let core_maximum_levels = effect_level_counts[order[0]] as u128;
                estimate_categorical_schwarz_memory(
                    rows,
                    2,
                    core_levels,
                    core_maximum_levels,
                    threads,
                    {
                        let core = [effect_level_counts[order[0]], effect_level_counts[order[1]]];
                        checked_add_memory(
                            categorical_dense_cross_tab_bytes(rows, &core)?,
                            crate::memory::schwarz_weighted_correction_bytes(rows, &core)?,
                            "PPML LSMR core cross-tab and compensation scratch",
                        )?
                    },
                    yielding,
                )?
            }
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

fn lsmr_error(error: SolveError) -> FeregError {
    if let Err(error) = crate::interrupt::checkpoint() {
        return error;
    }
    FeregError::new(
        if matches!(error, SolveError::Cancelled) {
            ErrorCode::Interrupted
        } else if matches!(error, SolveError::Allocation { .. }) {
            ErrorCode::Allocation
        } else {
            ErrorCode::NonConvergence
        },
        format!("joint PPML modified LSMR failed: {error}"),
    )
}

fn solve_shape_error(context: &'static str) -> SolveError {
    SolveError::InvalidInput {
        context,
        message: "joint PPML augmented operator dimensions are inconsistent".to_owned(),
    }
}

fn nonconvergence(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
mod tests;
