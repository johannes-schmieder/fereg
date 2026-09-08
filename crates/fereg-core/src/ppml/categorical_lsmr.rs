//! PPML augmented LSMR over finite controls and categorical intercepts.
//!
//! Exact normal-equation moments are recomputed after every recurrence stop;
//! LSMR diagnostics alone never authorize an IRLS step.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

use rayon::prelude::*;
use schwarz_precond::{mlsmr, LsmrStopReason, MlsmrOptions, Operator, SolveError, Staleness};
use within::{Effect, Preconditioner, PreconditionerConfig, Solver};

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{retained_effect_order_by_levels, FeDesign};
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use crate::prepared_fit_view::PreparedOlsFitView;

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
/// The retained Schwarz factor is effective for two categorical effects. With
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

fn largest_retained_core_pair(design: &FeDesign) -> Result<[usize; 2]> {
    let order = retained_effect_order_by_levels(design.effects().len(), |effect| {
        design.effects()[effect].n_levels()
    })?;
    if order.len() < 2 {
        return Err(invalid_argument(
            "joint PPML core-pair policy requires at least two effects",
        ));
    }
    Ok([order[0], order[1]])
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
    if std::env::var_os("FEREG_SOLVER_TRACE").is_none() {
        return;
    }
    let line = joint_lsmr_frame_trace_line(frame);
    emit_solver_trace_line(&line);
}

pub(super) fn emit_solver_trace_line(line: &str) {
    eprintln!("{line}");
    let Some(path) = std::env::var_os("FEREG_SOLVER_TRACE_FILE") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
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

/// Fit-lifetime state for the ordinary (non-strict) IRLS Newton correction.
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
        // Recurrence escalation is meaningful only for a factor carried from a
        // previous Hessian frame. A factor built from the current weights
        // cannot be made fresher by immediately rebuilding it unchanged.
        let use_escalation =
            self.policy != CategoricalLsmrPolicy::Diagonal && use_schwarz && !rebuilt;
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
                    Some(&result.x),
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
                    Some(&result.x),
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
        if self.policy == CategoricalLsmrPolicy::Diagonal {
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
        let parallel_column_limit =
            rayon::current_num_threads().saturating_mul(ADJOINT_COLUMN_LANES_PER_THREAD);
        if finite <= parallel_column_limit {
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
        } else {
            finite_output.fill(0.0);
            let active_values = self.active_design.as_slice();
            for (row, &input) in x.iter().enumerate() {
                let start = row * finite;
                for (column, &value) in active_values[start..start + finite].iter().enumerate() {
                    finite_output[column] =
                        (self.sqrt_weights[row] * value).mul_add(input, finite_output[column]);
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
                            sum = self.sqrt_weights[row].mul_add(x[row], sum);
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
    let mut bridge = scratch.lock().map_err(|_| SolveError::Synchronization {
        context: "joint PPML core-pair bridge scratch",
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
    let shape = CategoricalLsmrMemoryShape::new(
        rows,
        active_columns,
        effect_level_counts,
        threads,
        policy,
    )?;
    let n = shape.rows;
    let k = shape.active_columns;
    let p = checked_add_memory(k, shape.levels, "joint PPML augmented width")?;
    let core_pair_bridge_bytes = core_pair_bridge_scratch_bytes(effect_level_counts, policy)?;

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
        bytes_for_memory::<DirectionCertificate>(
            checked_add_memory(k, shape.levels, "joint PPML certificate direction count")?,
            "joint PPML certificate scratch",
        )?,
        shape.schwarz.retained_bytes,
        core_pair_bridge_bytes,
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
    if rhs == 0 {
        return Err(invalid_argument(
            "PPML checkpoint LSMR memory estimate requires a positive RHS width",
        ));
    }
    let shape = CategoricalLsmrMemoryShape::new(
        rows,
        0,
        effect_level_counts,
        threads,
        CategoricalLsmrPolicy::Reuse,
    )?;
    let concurrent_rhs = (rhs as u128).min(shape.threads);
    let rhs = rhs as u128;

    // The input matrix, column-major RHS copies, returned demeaned batch, and
    // returned coefficient batch can coexist until canonical recertification.
    let retained_batch_values = checked_sum_memory(&[
        checked_mul_memory(shape.rows, rhs, "checkpoint LSMR input matrix")?,
        checked_mul_memory(shape.rows, rhs, "checkpoint LSMR RHS copies")?,
        checked_mul_memory(shape.rows, rhs, "checkpoint LSMR demeaned outputs")?,
        checked_mul_memory(shape.levels, rhs, "checkpoint LSMR coefficient outputs")?,
        shape.rows,
    ])?;
    let retained_batch_bytes =
        bytes_for_memory::<f64>(retained_batch_values, "checkpoint LSMR retained batch")?;

    // Modified Golub--Kahan plus the solution recurrence and local history.
    // This deliberately over-bounds opaque upstream per-RHS allocations.
    let recurrence_per_rhs = checked_sum_memory(&[
        checked_mul_memory(shape.rows, 3, "checkpoint LSMR observation recurrence")?,
        checked_mul_memory(shape.levels, 14, "checkpoint LSMR coefficient recurrence")?,
    ])?;
    let recurrence_peak = bytes_for_memory::<f64>(
        checked_mul_memory(
            recurrence_per_rhs,
            concurrent_rhs,
            "checkpoint LSMR concurrent recurrences",
        )?,
        "checkpoint LSMR recurrence bytes",
    )?;
    let solve_peak = checked_sum_memory(&[
        retained_batch_bytes,
        shape.schwarz.retained_bytes,
        shape.schwarz.solver_frame_bytes,
        shape.schwarz.apply_peak_bytes,
        recurrence_peak,
    ])?;
    let build_peak = checked_sum_memory(&[retained_batch_bytes, shape.schwarz.build_peak_bytes])?;
    Ok(solve_peak.max(build_peak))
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
    fn new(
        rows: usize,
        active_columns: usize,
        effect_level_counts: &[usize],
        threads: usize,
        policy: CategoricalLsmrPolicy,
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

fn categorical_dense_cross_tab_bytes(rows: u128, level_counts: &[usize]) -> Result<u128> {
    // The pinned within cross_tab/accumulate.rs chooses dense whenever
    // L_i * L_j <= 5,000,000 OR its 8-byte table costs no more than 12N.
    // Pair construction is parallel; charge all simultaneously live tables.
    // The existing edge bound separately covers CSR output and sparse scratch.
    let sparse_bytes = checked_mul_memory(rows, 12, "PPML LSMR sparse cross-tab scratch")?;
    let mut dense_bytes = 0;
    for (index, &left) in level_counts.iter().enumerate() {
        for &right in &level_counts[index + 1..] {
            let cells =
                checked_mul_memory(left as u128, right as u128, "PPML LSMR cross-tab cells")?;
            let bytes = bytes_for_memory::<f64>(cells, "PPML LSMR dense cross-tab scratch")?;
            if cells <= 5_000_000 || bytes <= sparse_bytes {
                dense_bytes = checked_add_memory(dense_bytes, bytes, "PPML LSMR dense pair peak")?;
            }
        }
    }
    Ok(dense_bytes)
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
    dense_cross_tab_bytes: u128,
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
        dense_cross_tab_bytes,
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

    fn predictor_frame(
        scheduled_tolerance: f64,
        effective_tolerance: f64,
        first_certificate: f64,
        iterations: usize,
        continuation: JointLsmrContinuationCause,
    ) -> JointLsmrFrameDiagnostic {
        JointLsmrFrameDiagnostic {
            scheduled_tolerance,
            first_pass_tolerance: scheduled_tolerance,
            effective_tolerance,
            first_pass_iterations: iterations,
            continuation_iterations: 0,
            first_pass_recurrence_residual: 1e-6,
            recurrence_residual: 1e-7,
            first_certificate_relative_infinity_norm: first_certificate,
            finite_certificate_max: 1e-8,
            fixed_effect_certificate_max: 2e-8,
            max_abs_log_hessian_weight_change: 0.1,
            weight_change_valid: true,
            predictor_used: false,
            predictor_refreshed_preconditioner: false,
            continuation,
        }
    }

    #[test]
    fn first_pass_predictor_initializes_to_the_scheduled_tolerance() {
        let predictor = JointLsmrFirstPassPredictor::default();
        let policy = predictor.policy(1e-8, true, 0.1, CategoricalLsmrPolicy::CorePair);
        assert_eq!(policy.tolerance.to_bits(), 1e-8_f64.to_bits());
        assert!(!policy.used_predictor);
        assert!(!policy.refresh_preconditioner);
    }

    #[test]
    fn first_pass_predictor_clamps_prior_certificate_misses() {
        let mut predictor = JointLsmrFirstPassPredictor::default();
        predictor.record_certified(predictor_frame(
            1e-8,
            1e-10,
            2e-8,
            200,
            JointLsmrContinuationCause {
                recurrence: false,
                certificate: true,
                staleness: false,
            },
        ));
        let predicted = predictor.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::CorePair);
        assert!((predicted.tolerance - 1e-11).abs() <= 1e-26);
        assert!(predicted.used_predictor);
        assert!(predicted.refresh_preconditioner);

        let mut floor_predictor = JointLsmrFirstPassPredictor::default();
        floor_predictor.record_certified(predictor_frame(
            5e-12,
            5e-14,
            1e-11,
            10,
            JointLsmrContinuationCause {
                recurrence: true,
                certificate: false,
                staleness: false,
            },
        ));
        let floor = floor_predictor.policy(5e-14, true, 0.1, CategoricalLsmrPolicy::CorePair);
        assert_eq!(
            floor.tolerance.to_bits(),
            JOINT_LSMR_NUMERICAL_FLOOR.to_bits()
        );
        assert!(floor.tolerance <= 5e-14);

        let mut passing = JointLsmrFirstPassPredictor::default();
        passing.record_certified(predictor_frame(
            1e-8,
            1e-8,
            5e-9,
            10,
            JointLsmrContinuationCause::default(),
        ));
        let scheduled = passing.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::CorePair);
        assert_eq!(scheduled.tolerance.to_bits(), 1e-9_f64.to_bits());
        assert!(!scheduled.refresh_preconditioner);

        let two_effect = predictor.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::Reuse);
        assert_eq!(two_effect.tolerance.to_bits(), 1e-9_f64.to_bits());
        assert!(!two_effect.used_predictor);
        assert!(!two_effect.refresh_preconditioner);
    }

    #[test]
    fn first_pass_predictor_rejects_weight_and_schedule_jumps() {
        let mut predictor = JointLsmrFirstPassPredictor::default();
        predictor.record_certified(predictor_frame(
            1e-8,
            1e-10,
            2e-8,
            200,
            JointLsmrContinuationCause::default(),
        ));
        for policy in [
            predictor.policy(1e-9, false, 0.1, CategoricalLsmrPolicy::CorePair),
            predictor.policy(
                1e-9,
                true,
                JOINT_LSMR_PREDICTOR_MAX_LOG_WEIGHT_CHANGE + f64::EPSILON,
                CategoricalLsmrPolicy::CorePair,
            ),
            predictor.policy(1e-11, true, 0.1, CategoricalLsmrPolicy::CorePair),
            predictor.policy(2e-8, true, 0.1, CategoricalLsmrPolicy::CorePair),
        ] {
            assert!(!policy.used_predictor);
            assert!(!policy.refresh_preconditioner);
        }
    }

    #[test]
    fn first_pass_predictor_invalidation_is_atomic_and_matches_engine_replacement() {
        let frame = predictor_frame(1e-8, 1e-8, 5e-9, 200, JointLsmrContinuationCause::default());
        let mut predictor = JointLsmrFirstPassPredictor::default();
        predictor.record_certified(frame);
        let snapshot = predictor;
        let _speculative = predictor.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::CorePair);
        assert_eq!(predictor, snapshot);

        predictor.invalidate();
        assert_eq!(predictor, JointLsmrFirstPassPredictor::default());
        let replacement_for_changed_basis = JointLsmrFirstPassPredictor::default();
        assert_eq!(predictor, replacement_for_changed_basis);
    }

    #[test]
    fn joint_lsmr_frame_trace_is_deterministic_and_complete() {
        let mut frame = predictor_frame(
            1e-8,
            1e-9,
            2e-8,
            200,
            JointLsmrContinuationCause {
                recurrence: true,
                certificate: true,
                staleness: true,
            },
        );
        frame.predictor_used = true;
        frame.predictor_refreshed_preconditioner = true;
        let first = joint_lsmr_frame_trace_line(frame);
        let second = joint_lsmr_frame_trace_line(frame);
        assert_eq!(first, second);
        for field in [
            "scheduled=",
            "first_tolerance=",
            "effective=",
            "first_iterations=",
            "continuation_iterations=",
            "first_normal=",
            "final_normal=",
            "first_certificate=",
            "finite_max=",
            "fe_max=",
            "weight_change=",
            "predictor_used=1",
            "predictor_refresh=1",
            "recurrence_continuation=1",
            "certificate_continuation=1",
            "staleness_continuation=1",
        ] {
            assert!(first.contains(field), "trace omitted {field}");
        }
    }

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
    fn core_pair_bridge_matches_its_dense_symmetric_block_factor() {
        let first = vec![0, 0, 1, 1, 2, 2, 0, 1, 2, 0, 1, 2];
        let second = vec![0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 0];
        let third = vec![0, 1, 2, 0, 1, 2, 2, 0, 1, 1, 2, 0];
        let design = FeDesign::new(
            first.len(),
            vec![
                EffectTerm::new(first, 3, true, Vec::new()).expect("first"),
                EffectTerm::new(second, 2, true, Vec::new()).expect("second"),
                EffectTerm::new(third, 3, true, Vec::new()).expect("third"),
            ],
        )
        .expect("design");
        let weights = vec![
            0.75, 1.25, 0.9, 1.4, 0.65, 1.1, 1.6, 0.8, 1.35, 0.95, 1.2, 0.7,
        ];
        let offsets = vec![0, 3, 5, 8];
        let core_effects = [0, 2];
        let core_width = 6;
        let effects = within_effects_selected(&design, &core_effects).expect("within effects");
        let solver = Solver::new(
            effects,
            Some(weights.clone()),
            PreconditionerConfig::default(),
        )
        .expect("core solver");
        let core = solver
            .preconditioner()
            .cloned()
            .expect("core preconditioner");
        assert_eq!(core.nrows(), core_width);

        let mut diagonal = vec![0.0; design.total_coefficients()];
        fill_fe_diagonal_inverse(&design, &weights, &mut diagonal).expect("diagonal");
        let scratch = Mutex::new(CorePairBridgeScratch::new(core_width).expect("scratch"));
        let preconditioner = BlockPreconditioner {
            finite_inverse: &[],
            fe: FePreconditioner::CorePair {
                core: &core,
                core_effects,
                diagonal: &diagonal,
                design: &design,
                effect_offsets: &offsets,
                weights: &weights,
                scratch: &scratch,
            },
        };
        let input = vec![0.4, -0.7, 0.2, 0.9, -0.3, 0.6, -0.8, 0.5];
        let mut actual = vec![0.0; input.len()];
        preconditioner
            .apply(&input, &mut actual)
            .expect("bridge apply");

        let mut dense_core = vec![vec![0.0; core_width]; core_width];
        for column in 0..core_width {
            let mut basis = vec![0.0; core_width];
            let mut image = vec![0.0; core_width];
            basis[column] = 1.0;
            core.apply(&basis, &mut image).expect("core basis apply");
            for row in 0..core_width {
                dense_core[row][column] = image[row];
            }
        }
        let extra_effects = [1];
        let extra_width = design.effects()[extra_effects[0]].n_levels();
        let mut dense_cross = vec![vec![0.0; extra_width]; core_width];
        for (row, weight) in weights.iter().copied().enumerate() {
            let mut packed_core_start = 0_usize;
            for core_effect in core_effects {
                let core_index =
                    packed_core_start + design.effects()[core_effect].codes()[row] as usize;
                let extra_effect = extra_effects[0];
                let extra_index = design.effects()[extra_effect].codes()[row] as usize;
                dense_cross[core_index][extra_index] += weight;
                packed_core_start += design.effects()[core_effect].n_levels();
            }
        }
        let dense_apply = |matrix: &[Vec<f64>], vector: &[f64]| {
            matrix
                .iter()
                .map(|row| {
                    row.iter()
                        .zip(vector)
                        .map(|(left, right)| left * right)
                        .sum()
                })
                .collect::<Vec<f64>>()
        };
        let mut core_input = Vec::with_capacity(core_width);
        for core_effect in core_effects {
            core_input.extend_from_slice(&input[offsets[core_effect]..offsets[core_effect + 1]]);
        }
        let extra_effect = extra_effects[0];
        let extra_input = &input[offsets[extra_effect]..offsets[extra_effect + 1]];
        let core_image = dense_apply(&dense_core, &core_input);
        let mut extra_image = vec![0.0; extra_width];
        for extra in 0..extra_width {
            let cross = (0..core_width)
                .map(|core_index| dense_cross[core_index][extra] * core_image[core_index])
                .sum::<f64>();
            extra_image[extra] =
                diagonal[offsets[extra_effect] + extra] * (extra_input[extra] - cross);
        }
        let cross_image = dense_apply(&dense_cross, &extra_image);
        let core_correction = dense_apply(&dense_core, &cross_image);
        let mut expected = vec![0.0; input.len()];
        let mut packed_core_start = 0_usize;
        for core_effect in core_effects {
            let width = design.effects()[core_effect].n_levels();
            for level in 0..width {
                expected[offsets[core_effect] + level] = core_image[packed_core_start + level]
                    - core_correction[packed_core_start + level];
            }
            packed_core_start += width;
        }
        expected[offsets[extra_effect]..offsets[extra_effect + 1]].copy_from_slice(&extra_image);
        for (observed, reference) in actual.iter().zip(&expected) {
            assert!((observed - reference).abs() <= 2e-13);
        }

        let other = vec![-0.2, 0.3, 0.8, -0.4, 0.6, 0.1, -0.5, 0.7];
        let mut other_image = vec![0.0; other.len()];
        let mut adjoint_image = vec![0.0; input.len()];
        preconditioner
            .apply(&other, &mut other_image)
            .expect("second bridge apply");
        preconditioner
            .apply_adjoint(&input, &mut adjoint_image)
            .expect("bridge adjoint");
        let left = actual.iter().zip(&other).map(|(a, b)| a * b).sum::<f64>();
        let right = input
            .iter()
            .zip(&other_image)
            .map(|(a, b)| a * b)
            .sum::<f64>();
        assert!((left - right).abs() <= 2e-12);
        assert!(adjoint_image
            .iter()
            .zip(&actual)
            .all(|(adjoint, forward)| (adjoint - forward).abs() <= 2e-13));
        let quadratic = input.iter().zip(&actual).map(|(a, b)| a * b).sum::<f64>();
        assert!(quadratic >= -2e-12);
    }

    #[test]
    fn frame_preparation_preserves_column_order_diagonals() {
        let active = DenseMatrix::from_row_major(
            5,
            3,
            vec![
                1.0, -2.0, 0.5, 0.25, 3.0, -1.5, -0.75, 1.25, 2.0, 4.0, -0.5, 0.125, 1.5, 2.5, -3.0,
            ],
        )
        .expect("active design");
        let outcome = [0.0, 2.0, 1.0, 4.0, 0.5];
        let mean = [1.25, 1.5, 0.75, 2.0, 0.625];
        let weights = [0.75, 1.25, 2.0, 0.5, 3.0];
        let mut sqrt_weights = vec![0.0; 5];
        let mut target = vec![0.0; 5];
        let mut rhs = vec![0.0; 5];
        let mut finite_inverse = vec![0.0; 3];

        let weight_change = prepare_frame_and_finite_diagonal(
            &active,
            &outcome,
            &mean,
            &weights,
            &mut sqrt_weights,
            &mut target,
            &mut rhs,
            &mut finite_inverse,
            false,
        )
        .expect("prepared frame");
        assert_eq!(weight_change, 0.0);

        let mut expected_diagonal = vec![0.0_f64; active.ncols()];
        for (column, diagonal) in expected_diagonal.iter_mut().enumerate() {
            for (row, &weight) in weights.iter().enumerate() {
                let value = active[(row, column)];
                *diagonal = (weight * value).mul_add(value, *diagonal);
            }
            *diagonal = 1.0 / *diagonal;
        }
        let expected_target = outcome
            .iter()
            .zip(&mean)
            .map(|(&value, &mu)| if value == 0.0 { -1.0 } else { value / mu - 1.0 })
            .collect::<Vec<_>>();
        assert_eq!(finite_inverse, expected_diagonal);
        assert_eq!(target, expected_target);
        for row in 0..weights.len() {
            assert_eq!(sqrt_weights[row], weights[row].sqrt());
            assert_eq!(rhs[row], sqrt_weights[row] * target[row]);
        }
    }

    #[test]
    fn exact_certificate_is_thread_count_deterministic() {
        let rows = 2_048;
        let level_counts = [131, 137, 149];
        let effects = level_counts
            .iter()
            .enumerate()
            .map(|(effect, &levels)| {
                let codes = (0..rows)
                    .map(|row| ((row * (37 + effect * 12) + 5 * effect) % levels) as u32)
                    .collect::<Vec<_>>();
                EffectTerm::new(codes, levels, true, Vec::new()).expect("effect")
            })
            .collect::<Vec<_>>();
        let design = FeDesign::new(rows, effects).expect("design");
        let active_values = (0..rows)
            .flat_map(|row| {
                (0..4).map(move |column| ((row * (column + 3) + column * 11) % 97) as f64 / 53.0)
            })
            .collect::<Vec<_>>();
        let active = DenseMatrix::from_row_major(rows, 4, active_values).expect("active");
        let weights = (0..rows)
            .map(|row| 0.5 + (row % 29) as f64 / 17.0)
            .collect::<Vec<_>>();
        let target = (0..rows)
            .map(|row| (row % 41) as f64 / 19.0 - 0.75)
            .collect::<Vec<_>>();
        let residual = (0..rows)
            .map(|row| (row % 31) as f64 / 23.0 - 0.5)
            .collect::<Vec<_>>();

        let evaluate = |threads| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool");
            let mut finite_scratch = vec![DirectionCertificate::default(); active.ncols()];
            let mut fe_scratch = vec![DirectionCertificate::default(); design.total_coefficients()];
            let certificate = pool
                .install(|| {
                    certify_joint_with_scratch(
                        &design,
                        &active,
                        &weights,
                        &target,
                        &residual,
                        1e-8,
                        &mut finite_scratch,
                        &mut fe_scratch,
                    )
                })
                .expect("certificate");
            (certificate, finite_scratch, fe_scratch)
        };

        assert_eq!(evaluate(1), evaluate(4));
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
            core_effects: None,
            sqrt_weights: vec![0.0; 4],
            rhs: vec![0.0; 4],
            target: vec![0.0; 4],
            delta_eta: vec![0.0; 4],
            residual: vec![0.0; 4],
            warm_start: vec![0.0; 2],
            finite_inverse: Vec::new(),
            fe_inverse: vec![0.0; 2],
            finite_certificate: Vec::new(),
            fe_certificate: vec![DirectionCertificate::default(); 2],
            warm_start_valid: false,
            first_pass_predictor: JointLsmrFirstPassPredictor::default(),
            preconditioner: None,
            core_pair_bridge_scratch: Mutex::new(CorePairBridgeScratch::default()),
            policy: CategoricalLsmrPolicy::Diagonal,
            instrumentation: JointLsmrInstrumentation::default(),
        };
        solver
            .first_pass_predictor
            .record_certified(predictor_frame(
                1e-8,
                1e-8,
                5e-9,
                10,
                JointLsmrContinuationCause::default(),
            ));
        let predictor_before_failure = solver.first_pass_predictor;
        let error = solver
            .solve(&replacement, &[1.0; 4], &[1.0; 4], &[1.0; 4], 1e-8, 100)
            .expect_err("replacement topology must fail before operator use");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(solver.first_pass_predictor, predictor_before_failure);
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
            core_effects: None,
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
            finite_certificate: vec![DirectionCertificate::default(); 1],
            fe_certificate: vec![DirectionCertificate::default(); 6],
            warm_start_valid: false,
            first_pass_predictor: JointLsmrFirstPassPredictor::default(),
            preconditioner: None,
            core_pair_bridge_scratch: Mutex::new(CorePairBridgeScratch::default()),
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
        let bridge_scratch = solver
            .core_pair_bridge_scratch
            .lock()
            .expect("bridge scratch");
        assert!(bridge_scratch.core_rhs.is_empty());
        assert!(bridge_scratch.core_correction.is_empty());
        drop(bridge_scratch);
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
        assert!(!second.frame.predictor_used);
        assert!(!second.frame.predictor_refreshed_preconditioner);
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

        solver.reset_warm_start();
        assert!(!solver.warm_start_valid);
        assert_eq!(
            solver.first_pass_predictor,
            JointLsmrFirstPassPredictor::default()
        );
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
                core_effects: None,
                sqrt_weights: vec![0.0; 12],
                rhs: vec![0.0; 12],
                target: vec![0.0; 12],
                delta_eta: vec![0.0; 12],
                residual: vec![0.0; 12],
                warm_start: vec![0.0; 7],
                finite_inverse: vec![0.0; 1],
                fe_inverse: vec![0.0; 6],
                finite_certificate: vec![DirectionCertificate::default(); 1],
                fe_certificate: vec![DirectionCertificate::default(); 6],
                warm_start_valid: false,
                first_pass_predictor: JointLsmrFirstPassPredictor::default(),
                preconditioner: None,
                core_pair_bridge_scratch: Mutex::new(CorePairBridgeScratch::default()),
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
    fn joint_solver_supports_zero_finite_columns_with_three_effects() {
        let rows = 60;
        let first_codes = (0..rows).map(|row| (row % 5) as u32).collect::<Vec<_>>();
        let second_codes = (0..rows)
            .map(|row| ((row / 2) % 4) as u32)
            .collect::<Vec<_>>();
        let third_codes = (0..rows)
            .map(|row| ((row / 3) % 3) as u32)
            .collect::<Vec<_>>();
        let design = FeDesign::new(
            rows,
            vec![
                EffectTerm::new(first_codes.clone(), 5, true, Vec::new()).expect("first"),
                EffectTerm::new(second_codes.clone(), 4, true, Vec::new()).expect("second"),
                EffectTerm::new(third_codes.clone(), 3, true, Vec::new()).expect("third"),
            ],
        )
        .expect("design");
        let first_effect = [-0.04, -0.02, 0.0, 0.02, 0.04];
        let second_effect = [-0.03, -0.01, 0.01, 0.03];
        let third_effect = [-0.02, 0.0, 0.02];
        let target = (0..rows)
            .map(|row| {
                first_effect[first_codes[row] as usize]
                    + second_effect[second_codes[row] as usize]
                    + third_effect[third_codes[row] as usize]
            })
            .collect::<Vec<_>>();
        let outcome = target.iter().map(|value| 1.0 + value).collect::<Vec<_>>();
        let mut solver = ReusableCategoricalJointLsmr {
            team: test_team(4),
            active_design: DenseMatrix::from_row_major(rows, 0, Vec::new())
                .expect("empty active design"),
            active_slots: Vec::new(),
            topology: FeTopologyIdentity::capture(&design).expect("topology"),
            effect_offsets: vec![0, 5, 9, 12],
            core_effects: Some([0, 1]),
            sqrt_weights: vec![0.0; rows],
            rhs: vec![0.0; rows],
            target: vec![0.0; rows],
            delta_eta: vec![0.0; rows],
            residual: vec![0.0; rows],
            warm_start: vec![0.0; 12],
            finite_inverse: Vec::new(),
            fe_inverse: vec![0.0; 12],
            finite_certificate: Vec::new(),
            fe_certificate: vec![DirectionCertificate::default(); 12],
            warm_start_valid: false,
            first_pass_predictor: JointLsmrFirstPassPredictor::default(),
            preconditioner: None,
            core_pair_bridge_scratch: Mutex::new(
                CorePairBridgeScratch::new(9).expect("bridge scratch"),
            ),
            policy: CategoricalLsmrPolicy::CorePair,
            instrumentation: JointLsmrInstrumentation::default(),
        };

        let report = solver
            .solve(&design, &outcome, &[1.0; 60], &[1.0; 60], 1e-10, 1_000)
            .expect("FE-only correction");
        assert!(report.certificate.passed);
        assert!(report.requested_schwarz);
        assert!(report.used_schwarz);
        assert_eq!(solver.active_rank(), 0);
        assert!(solver.finite_inverse.is_empty());
        for (actual, expected) in solver.delta_eta.iter().zip(&target) {
            assert!((actual - expected).abs() <= 1e-9);
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
            &[2_001, 1_001],
            8,
            CategoricalLsmrPolicy::Reuse,
        )
        .expect("larger topology estimate");
        assert!(wider > base);
        assert!(parallel > wider);
        assert!(more_levels > parallel);
    }

    #[test]
    fn checkpoint_memory_charges_batch_outputs_and_concurrent_recurrences() {
        let one_thread =
            estimate_checkpoint_lsmr_workspace_bytes(50_000, 11, &[8_000, 7_000, 6_000], 1)
                .expect("one-thread checkpoint estimate");
        let seven_threads =
            estimate_checkpoint_lsmr_workspace_bytes(50_000, 11, &[8_000, 7_000, 6_000], 7)
                .expect("seven-thread checkpoint estimate");
        let wider = estimate_checkpoint_lsmr_workspace_bytes(50_000, 17, &[8_000, 7_000, 6_000], 7)
            .expect("wide checkpoint estimate");
        assert!(seven_threads > one_thread);
        assert!(wider > seven_threads);
        let three_dense_batches = 3_u128 * 50_000 * 11 * std::mem::size_of::<f64>() as u128;
        assert!(one_thread > three_dense_batches);
    }

    #[test]
    fn schwarz_memory_charges_dense_cross_tabs_at_both_dispatch_boundaries() {
        assert_eq!(
            categorical_dense_cross_tab_bytes(8_000, &[2_500, 2_000]).unwrap(),
            40_000_000
        );
        assert_eq!(
            categorical_dense_cross_tab_bytes(8_000, &[2_501, 2_000]).unwrap(),
            0
        );
        // Above the cap, dense is still used when no larger than sparse scratch.
        assert_eq!(
            categorical_dense_cross_tab_bytes(4_000_000, &[3_000, 2_000]).unwrap(),
            48_000_000
        );
        assert_eq!(
            categorical_dense_cross_tab_bytes(3_999_999, &[3_000, 2_000]).unwrap(),
            0
        );
        assert!(categorical_dense_cross_tab_bytes(u128::MAX, &[2, 3]).is_err());
    }

    #[test]
    fn sparse_checkpoint_memory_includes_concurrent_dense_nuisance_pairs() {
        let levels = [2_666, 2_000, 1_600];
        let dense = (2_666_u128 * 1_600 + 2_000 * 1_600) * 8;
        assert_eq!(
            categorical_dense_cross_tab_bytes(8_000, &levels).unwrap(),
            dense
        );
        let checkpoint =
            CategoricalLsmrMemoryShape::new(8_000, 0, &levels, 7, CategoricalLsmrPolicy::Reuse)
                .unwrap();
        let ordinary =
            CategoricalLsmrMemoryShape::new(8_000, 10, &levels, 7, CategoricalLsmrPolicy::CorePair)
                .unwrap();
        assert!(checkpoint.schwarz.build_peak_bytes > dense);
        assert!(ordinary.schwarz.build_peak_bytes < dense);
        assert!(estimate_checkpoint_lsmr_workspace_bytes(8_000, 11, &levels, 7).unwrap() > dense);
        let permuted = CategoricalLsmrMemoryShape::new(
            8_000,
            0,
            &[1_600, 2_666, 2_000],
            7,
            CategoricalLsmrPolicy::Reuse,
        )
        .unwrap();
        assert_eq!(checkpoint.schwarz, permuted.schwarz);
    }

    #[test]
    fn joint_policy_keeps_two_effect_schwarz_and_uses_one_core_pair_for_multiway() {
        assert_eq!(joint_lsmr_policy(1), CategoricalLsmrPolicy::Reuse);
        assert_eq!(joint_lsmr_policy(2), CategoricalLsmrPolicy::Reuse);
        assert_eq!(joint_lsmr_policy(3), CategoricalLsmrPolicy::CorePair);
        assert_eq!(joint_lsmr_policy(4), CategoricalLsmrPolicy::CorePair);
    }

    #[test]
    fn joint_core_pair_uses_the_same_two_largest_effects_under_every_permutation() {
        let rows = 120;
        let sources = [
            (
                (0..rows).map(|row| (row % 20) as u32).collect::<Vec<_>>(),
                20,
            ),
            ((0..rows).map(|row| (row % 9) as u32).collect::<Vec<_>>(), 9),
            ((0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>(), 4),
        ];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let effects = order
                .iter()
                .map(|&effect| {
                    EffectTerm::new(
                        sources[effect].0.clone(),
                        sources[effect].1,
                        true,
                        Vec::new(),
                    )
                    .expect("effect")
                })
                .collect();
            let design = FeDesign::new(rows, effects).expect("permuted design");
            let selected = largest_retained_core_pair(&design).expect("core pair");
            assert_eq!([order[selected[0]], order[selected[1]]], [0, 1]);
        }
    }

    #[test]
    fn joint_memory_charges_only_the_selected_schwarz_topology() {
        let levels = [500, 2_000, 1_000];
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
        let core_pair = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &levels,
            8,
            CategoricalLsmrPolicy::CorePair,
        )
        .expect("core-pair estimate");
        assert_eq!(diagonal_one, diagonal_eight);
        assert!(diagonal_eight < core_pair);
        assert!(core_pair < schwarz);
        let permuted = estimate_joint_lsmr_workspace_bytes(
            10_000,
            8,
            &[1_000, 500, 2_000],
            8,
            CategoricalLsmrPolicy::CorePair,
        )
        .expect("permuted core-pair estimate");
        assert_eq!(core_pair, permuted);
    }

    #[test]
    fn core_pair_memory_charges_exactly_two_core_width_bridge_buffers() {
        let levels = [500, 2_000, 1_000];
        let expected =
            2_u128 * (levels[1] + levels[2]) as u128 * std::mem::size_of::<f64>() as u128;
        assert_eq!(
            core_pair_bridge_scratch_bytes(&levels, CategoricalLsmrPolicy::CorePair)
                .expect("core-pair bridge bytes"),
            expected
        );
        assert_eq!(
            core_pair_bridge_scratch_bytes(&[1_000, 500, 2_000], CategoricalLsmrPolicy::CorePair)
                .expect("permuted core-pair bridge bytes"),
            expected
        );
        assert_eq!(
            core_pair_bridge_scratch_bytes(&levels, CategoricalLsmrPolicy::Reuse)
                .expect("reuse bridge bytes"),
            0
        );
        assert_eq!(
            core_pair_bridge_scratch_bytes(&levels, CategoricalLsmrPolicy::Diagonal)
                .expect("diagonal bridge bytes"),
            0
        );
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
