//! PPML explicit `solver(lsmr)`: each IRLS frame's weighted least-squares
//! Newton correction on `[X D]`.
//!
//! The frame's correction `z = (y - mu) / mu` (exact zero-outcome value -1)
//! is fitted on the frozen active controls `X` and the categorical intercept
//! design `D` under the current Hessian weights, by one of two formulations
//! with the same solution:
//!
//! - **Joint.** Modified LSMR on `W^1/2 [X D]` (schwarz-precond's `mlsmr`)
//!   with a block preconditioner: the controls' current-weight Gram factor
//!   and, on the FE block, the inverse level masses (Jacobi) or the identity.
//!   One Krylov recurrence solves the whole system, as fast as a single
//!   column's projection on designs the FE block preconditions well. The
//!   recurrence starts from the previous frame's solution scaled by the
//!   accepted step.
//! - **Projections** (the weighted Frisch-Waugh-Lovell route). `[X, z]` is
//!   projected onto `D` with fedemean's full-system modified LSMR
//!   (`Reduction::Full`, `Method::Lsmr`; Jacobi escalating to additive
//!   Schwarz, or Schwarz) and the small normal equations of the projected
//!   controls give the finite step; `r = z~ - X~ b` is the joint residual.
//!   Controls start from their previous frame's FE coefficients.
//!
//! `precondition(auto)` runs the joint recurrence with Jacobi; a recurrence
//! that stalls or misses its certificate after one tighter continuation
//! hands the frame to the projections, whose fedemean preconditioner
//! escalates to Schwarz, and later frames of the fit start there.
//! `precondition(jacobi)` and `precondition(none)` keep the joint recurrence;
//! `precondition(schwarz)` runs the projections with Schwarz.
//!
//! Either way the exact normal-equation moments of the formed residual on
//! the raw controls and on every FE level are recomputed after each solve,
//! at the frame's tolerance relative to `max(||z||, ||r||)`; recurrence
//! diagnostics and fedemean's per-column certificates never authorize an
//! IRLS step. Warm starts are guesses, never certificates.

use crate::linalg::neumaier_add;
use crate::memory::fallible_filled;
use std::time::Instant;

use rayon::prelude::*;
use schwarz_precond::{
    mlsmr_cancellable, LsmrStopReason, MlsmrOptions, Operator, SolveError, Staleness,
};

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::fe::block_schur::{design_view, fedemean_error, lsmr_choice, route};
use crate::fe::{FeDesign, FeMethod, FePreconditioner, FeReduction, FeRoute, LsmrPreconditioner};
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};

use super::topology_identity::FeTopologyIdentity;
use super::PpmlIterationWorkspace;

const ADJOINT_LEVEL_CHUNK: usize = 64;
const ADJOINT_COLUMN_LANES_PER_THREAD: usize = 2;
/// The tightest tolerance a frame's continuation or retry requests.
const NUMERICAL_FLOOR: f64 = 1e-14;
/// A missed joint certificate on the projections tightens later frames'
/// projection tolerance by this factor, down to [`MINIMUM_SCALE`] times the
/// frame tolerance.
const SCALE_STEP: f64 = 0.1;
const MINIMUM_SCALE: f64 = 1e-4;
/// The projections' retry tolerance relative to their first attempt's.
const RETRY_FACTOR: f64 = 0.01;

// Tests: count down projection attempts whose joint certificate is treated
// as missed, and joint recurrences treated as stalled.
crate::fault::fault_plan! {
    static FORCE_CERTIFICATE_MISSES: usize = 0;
    static FORCE_JOINT_STALL: bool = false;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct JointLsmrCertificate {
    pub(crate) finite_absolute_infinity_norm: f64,
    pub(crate) fixed_effect_absolute_infinity_norm: f64,
    pub(crate) relative_infinity_norm: f64,
    pub(crate) relative_two_norm: f64,
    pub(crate) passed: bool,
}

/// The formulation that solved a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Formulation {
    Joint,
    Projections,
}

/// One certified frame of the explicit LSMR engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct JointLsmrReport {
    /// LSMR iterations: the joint recurrence's, plus every projected
    /// column's when the frame reached the projections.
    pub(crate) iterations: usize,
    /// The tolerance of the solve that certified.
    pub(crate) effective_tolerance: f64,
    /// Continuations, hand-offs and reprojections after a stall or a missed
    /// certificate.
    pub(crate) retries: usize,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) certification_seconds: f64,
    pub(crate) warm_start_attempted: bool,
    pub(crate) formulation: Formulation,
    /// The route of the certifying solve: full system, modified LSMR, the
    /// FE preconditioner used and fedemean's escalations.
    pub(crate) route: FeRoute,
    /// The joint recurrence's last normal-equation estimate (joint frames).
    pub(crate) recurrence_residual: Option<f64>,
    pub(crate) certificate: JointLsmrCertificate,
    #[cfg(test)]
    pub(crate) executing_threads: usize,
}

/// One projection attempt's outcome.
enum Projection {
    Certified {
        route: FeRoute,
        iterations: usize,
    },
    /// fedemean declined numerically: the frame continues with its retry.
    Declined {
        iterations: usize,
    },
}

/// The work of a frame so far.
#[derive(Default)]
struct Tally {
    iterations: usize,
    retries: usize,
    solve_seconds: f64,
    certification_seconds: f64,
}

/// Fit-lifetime state of the explicit LSMR engine. A strict solve can witness
/// convergence, but the canonical projection still owns the final rank and
/// inference.
pub(crate) struct ReusableCategoricalJointLsmr<'p> {
    team: InstalledThreadTeam,
    preconditioner: LsmrPreconditioner,
    /// A frame of this fit needed the projections: later frames start there.
    projections_first: bool,
    active_design: DenseMatrix,
    active_slots: Vec<usize>,
    topology: FeTopologyIdentity,
    /// Each effect's first coefficient, with the coefficient count last.
    effect_offsets: Vec<usize>,
    level_counts: Vec<usize>,
    target: Vec<f64>,
    delta_eta: Vec<f64>,
    residual: Vec<f64>,
    /// The finite step of the last certified frame.
    step: Vec<f64>,
    finite_certificate: Vec<DirectionCertificate>,
    fe_certificate: Vec<DirectionCertificate>,
    joint: JointState,
    /// fedemean's view of the design, and the projections' state once a
    /// frame needs them.
    design: fedemean::Design<'p>,
    projections: Option<Projections<'p>>,
}

/// The joint recurrence's frame vectors, block preconditioner and warm
/// state.
struct JointState {
    sqrt_weights: Vec<f64>,
    rhs: Vec<f64>,
    warm_start: Vec<f64>,
    warm_start_valid: bool,
    finite_inverse: Vec<f64>,
    finite_factor: Vec<f64>,
    finite_factor_ready: bool,
    fe_inverse: Vec<f64>,
}

/// The projections' fedemean projector, projected matrix, FE coefficients
/// and finite normal-equation workspace.
struct Projections<'p> {
    projector: fedemean::Projector<'p>,
    /// fedemean's workspace limit.
    memory_limit: u128,
    /// Observation-major `[X, z]`: the projection's input and, after it,
    /// the projected columns.
    projected: DenseMatrix,
    /// Warm starts and solved coefficients, `(k + 1) * L` each.
    start: Vec<f64>,
    coefficients: Vec<f64>,
    warm_start_valid: bool,
    finite: FiniteBuffers,
    /// Projection tolerance relative to the frame's, learned from misses.
    tolerance_scale: f64,
}

/// The finite normal-equation workspace of the projected controls.
struct FiniteBuffers {
    gram: Vec<f64>,
    rhs: Vec<f64>,
    factor: Vec<f64>,
    forward: Vec<f64>,
    solution: Vec<f64>,
    scales: Vec<f64>,
    step: Vec<f64>,
    permutation: Vec<usize>,
}

impl FiniteBuffers {
    /// The `f64` values [`FiniteBuffers::new`] allocates for `columns`
    /// controls: two `k x k` matrices and five `k`-vectors (the permutation's
    /// `k` indices besides).
    fn values(columns: u128) -> Result<u128> {
        checked_add_memory(
            checked_mul_memory(
                checked_mul_memory(columns, columns, "PPML LSMR finite Gram")?,
                2,
                "PPML LSMR finite matrices",
            )?,
            checked_mul_memory(columns, 5, "PPML LSMR finite vectors")?,
            "PPML LSMR finite buffers",
        )
    }

    fn new(columns: usize) -> Result<Self> {
        let square = columns
            .checked_mul(columns)
            .ok_or_else(|| allocation("explicit PPML LSMR finite dimensions overflow"))?;
        let vector = |label| fallible_filled(columns, 0.0, label);
        Ok(Self {
            gram: fallible_filled(square, 0.0, "explicit PPML LSMR finite Gram")?,
            rhs: vector("explicit PPML LSMR finite moments")?,
            factor: fallible_filled(square, 0.0, "explicit PPML LSMR finite factor")?,
            forward: vector("explicit PPML LSMR finite forward solve")?,
            solution: vector("explicit PPML LSMR standardized step")?,
            scales: vector("explicit PPML LSMR finite scales")?,
            step: vector("explicit PPML LSMR finite step")?,
            permutation: fallible_filled(columns, 0, "explicit PPML LSMR finite permutation")?,
        })
    }
}

impl JointState {
    fn new(rows: usize, columns: usize, levels: usize) -> Result<Self> {
        let width = columns
            .checked_add(levels)
            .ok_or_else(|| allocation("explicit PPML LSMR augmented width overflows"))?;
        Ok(Self {
            sqrt_weights: fallible_filled(rows, 0.0, "explicit PPML LSMR square-root weights")?,
            rhs: fallible_filled(rows, 0.0, "explicit PPML LSMR right-hand side")?,
            warm_start: fallible_filled(width, 0.0, "explicit PPML LSMR warm state")?,
            warm_start_valid: false,
            finite_inverse: fallible_filled(
                columns,
                0.0,
                "explicit PPML LSMR finite diagonal preconditioner",
            )?,
            finite_factor: fallible_filled(
                columns
                    .checked_mul(columns)
                    .ok_or_else(|| allocation("explicit PPML LSMR finite factor overflows"))?,
                0.0,
                "explicit PPML LSMR finite factor",
            )?,
            finite_factor_ready: false,
            fe_inverse: fallible_filled(levels, 0.0, "explicit PPML LSMR FE preconditioner")?,
        })
    }
}

impl<'p> ReusableCategoricalJointLsmr<'p> {
    pub(crate) fn new_with_threads<P: PreparedOlsFitView + Sync + ?Sized>(
        prepared: &'p P,
        active_generated: &[usize],
        fe_design: &FeDesign,
        threads: ThreadConfig,
        preconditioner: LsmrPreconditioner,
    ) -> Result<Self> {
        let rows = fe_design.nobs();
        if rows != prepared.fit_sample().n_active()
            || fe_design.effects().is_empty()
            || fe_design
                .effects()
                .iter()
                .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
        {
            return Err(invalid_argument(
                "explicit PPML LSMR requires an aligned categorical intercept-only design",
            ));
        }
        let borrowed = prepared.fit_borrowed_cmg_design()?;
        if borrowed.nobs() != rows
            || borrowed.effects().len() != fe_design.effects().len()
            || borrowed
                .effects()
                .iter()
                .zip(fe_design.effects())
                .any(|(view, effect)| {
                    view.n_levels() != effect.n_levels() || view.codes() != effect.codes()
                })
        {
            return Err(invalid_argument(
                "explicit PPML LSMR projection design differs from its certificate design",
            ));
        }
        let team = ThreadTeam::new(threads).acquire()?;
        let active_design = team.install(|| build_active_design(prepared, active_generated))?;
        let mut active_slots = Vec::new();
        active_slots
            .try_reserve_exact(active_generated.len())
            .map_err(|_| allocation("unable to reserve explicit PPML LSMR coefficient slots"))?;
        for &generated in active_generated {
            let slot = prepared
                .fit_model()
                .coefficients
                .iter()
                .position(|coefficient| coefficient.generated_column_index == Some(generated))
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "explicit PPML LSMR active column has no coefficient slot",
                    )
                })?;
            active_slots.push(slot);
        }
        Self::assemble(
            team,
            fe_design,
            active_design,
            active_slots,
            design_view(&borrowed)?,
            preconditioner,
        )
    }

    /// The engine on `fe_design` (its topology identity and layout), with
    /// fedemean's view of the same design.
    fn assemble(
        team: InstalledThreadTeam,
        fe_design: &FeDesign,
        active_design: DenseMatrix,
        active_slots: Vec<usize>,
        design: fedemean::Design<'p>,
        preconditioner: LsmrPreconditioner,
    ) -> Result<Self> {
        let rows = fe_design.nobs();
        let effects = fe_design.effects().len();
        let mut effect_offsets =
            fallible_filled(effects + 1, 0_usize, "explicit PPML LSMR offsets")?;
        let mut level_counts = fallible_filled(effects, 0_usize, "explicit PPML LSMR levels")?;
        for (e, effect) in fe_design.effects().iter().enumerate() {
            level_counts[e] = effect.n_levels();
            effect_offsets[e + 1] = effect_offsets[e]
                .checked_add(effect.n_levels())
                .ok_or_else(|| allocation("explicit PPML LSMR FE coefficient count overflows"))?;
        }
        let total = effect_offsets[effects];
        let columns = active_design.ncols();
        Ok(Self {
            team,
            preconditioner,
            projections_first: preconditioner == LsmrPreconditioner::Schwarz,
            topology: FeTopologyIdentity::capture(fe_design),
            effect_offsets,
            level_counts,
            target: fallible_filled(rows, 0.0, "explicit PPML LSMR Newton target")?,
            delta_eta: fallible_filled(rows, 0.0, "explicit PPML LSMR predictor correction")?,
            residual: fallible_filled(rows, 0.0, "explicit PPML LSMR correction residual")?,
            step: fallible_filled(columns, 0.0, "explicit PPML LSMR finite step")?,
            finite_certificate: fallible_filled(
                columns,
                DirectionCertificate::default(),
                "explicit PPML LSMR finite certificate scratch",
            )?,
            fe_certificate: fallible_filled(
                total,
                DirectionCertificate::default(),
                "explicit PPML LSMR FE certificate scratch",
            )?,
            joint: JointState::new(rows, columns, total)?,
            active_design,
            active_slots,
            design,
            projections: None,
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
        // The fault plans live on the caller's thread.
        let faults = (
            FORCE_CERTIFICATE_MISSES.replace(0),
            FORCE_JOINT_STALL.replace(false),
        );
        let team = self.team.clone();
        team.install(|| {
            self.solve_installed(
                fe_design,
                (outcome, mean),
                (hessian_weights, tolerance),
                max_iterations,
                faults,
            )
        })
    }

    fn solve_installed(
        &mut self,
        fe_design: &FeDesign,
        (outcome, mean): (&[f64], &[f64]),
        (weights, tolerance): (&[f64], f64),
        max_iterations: usize,
        (misses, stall): (usize, bool),
    ) -> Result<JointLsmrReport> {
        let setup_started = Instant::now();
        let warm_start_attempted = self.joint.warm_start_valid
            || self
                .projections
                .as_ref()
                .is_some_and(|p| p.warm_start_valid);
        prepare_frame_and_finite_diagonal(
            &self.active_design,
            outcome,
            mean,
            weights,
            &mut self.joint.sqrt_weights,
            &mut self.target,
            &mut self.joint.rhs,
            &mut self.joint.finite_inverse,
            false,
        )?;
        let mut setup_seconds = setup_started.elapsed().as_secs_f64();
        let mut tally = Tally::default();
        if !self.projections_first {
            let started = Instant::now();
            self.joint.finite_factor_ready = prepare_finite_factor(
                &self.active_design,
                weights,
                &self.joint.finite_inverse,
                &mut self.joint.finite_factor,
            );
            match self.preconditioner {
                LsmrPreconditioner::Off => self.joint.fe_inverse.fill(1.0),
                _ => fill_fe_diagonal_inverse(fe_design, weights, &mut self.joint.fe_inverse)?,
            }
            setup_seconds += started.elapsed().as_secs_f64();
            if let Some(report) = self.solve_joint(
                fe_design,
                (weights, tolerance),
                max_iterations,
                stall,
                &mut tally,
            )? {
                let report = JointLsmrReport {
                    setup_seconds,
                    warm_start_attempted,
                    ..report
                };
                emit_frame_trace(tolerance, &report);
                return Ok(report);
            }
            if self.preconditioner != LsmrPreconditioner::Auto {
                return Err(nonconvergence(
                    "explicit PPML LSMR correction did not pass its exact normal-equation certificate",
                ));
            }
            // The joint recurrence's FE preconditioner is too weak for this
            // design; the projections escalate to Schwarz, here and on every
            // later frame of the fit.
            self.projections_first = true;
            self.joint.warm_start_valid = false;
            tally.retries += 1;
        }
        let report = self.solve_projections(
            fe_design,
            (weights, tolerance),
            max_iterations,
            misses,
            &mut tally,
        )?;
        let report = JointLsmrReport {
            setup_seconds,
            warm_start_attempted,
            ..report
        };
        emit_frame_trace(tolerance, &report);
        Ok(report)
    }

    /// The joint recurrence: one pass (with the automatic preconditioner, a
    /// staleness hand-off), and on a recurrence or certificate miss one
    /// tighter continuation from its iterate. `None` when the frame must
    /// move to the projections or fail.
    fn solve_joint(
        &mut self,
        fe_design: &FeDesign,
        (weights, tolerance): (&[f64], f64),
        max_iterations: usize,
        stall: bool,
        tally: &mut Tally,
    ) -> Result<Option<JointLsmrReport>> {
        #[cfg(test)]
        let executing_threads = rayon::current_num_threads();
        let staleness = Staleness::try_new(4, 0.75).map_err(|error| {
            FeregError::new(
                ErrorCode::Internal,
                format!("invalid explicit PPML LSMR staleness policy: {error}"),
            )
        })?;
        let escalate = self.preconditioner == LsmrPreconditioner::Auto;
        let started = Instant::now();
        let first = self.run_once(
            fe_design,
            tolerance,
            max_iterations,
            self.joint
                .warm_start_valid
                .then_some(self.joint.warm_start.as_slice()),
            escalate.then_some(&staleness as _),
            None,
        )?;
        tally.solve_seconds += started.elapsed().as_secs_f64();
        tally.iterations = tally.iterations.saturating_add(first.iterations);
        if stall || first.stop_reason == LsmrStopReason::Escalated {
            return Ok(None);
        }
        let started = Instant::now();
        let mut certificate = self.finish_joint(fe_design, weights, &first.x, tolerance)?;
        tally.certification_seconds += started.elapsed().as_secs_f64();
        let mut effective_tolerance = tolerance;
        let mut result = first;
        if !result.converged || !certificate.passed {
            // Recurrence termination and the independent certificate are
            // separate obligations; continue from the iterate when either
            // is missing.
            effective_tolerance =
                tolerance.min((tolerance * 0.01).max(NUMERICAL_FLOOR.min(tolerance)));
            tally.retries += 1;
            let started = Instant::now();
            let continued = self.run_once(
                fe_design,
                effective_tolerance,
                max_iterations,
                Some(result.x.as_slice()),
                None,
                Some(4),
            )?;
            tally.solve_seconds += started.elapsed().as_secs_f64();
            tally.iterations = tally.iterations.saturating_add(continued.iterations);
            result = continued;
            let started = Instant::now();
            certificate = self.finish_joint(fe_design, weights, &result.x, tolerance)?;
            tally.certification_seconds += started.elapsed().as_secs_f64();
        }
        if !result.converged || !certificate.passed {
            self.joint.warm_start_valid = false;
            return Ok(None);
        }
        let columns = self.active_design.ncols();
        self.step.copy_from_slice(&result.x[..columns]);
        self.joint.warm_start.copy_from_slice(&result.x);
        self.joint.warm_start_valid = true;
        let preconditioner = if self.preconditioner == LsmrPreconditioner::Off {
            FePreconditioner::None
        } else {
            FePreconditioner::Jacobi
        };
        Ok(Some(JointLsmrReport {
            iterations: tally.iterations,
            effective_tolerance,
            retries: tally.retries,
            setup_seconds: 0.0,
            solve_seconds: tally.solve_seconds,
            certification_seconds: tally.certification_seconds,
            warm_start_attempted: false,
            formulation: Formulation::Joint,
            route: FeRoute::new(FeReduction::Full, FeMethod::Lsmr, preconditioner),
            recurrence_residual: Some(result.normal_eq_residual),
            certificate,
            #[cfg(test)]
            executing_threads,
        }))
    }

    /// The projections: `[X, z]` projected at the learned tolerance and the
    /// finite step, certified exactly; a miss reprojects once from the
    /// solved coefficients at a tighter tolerance.
    fn solve_projections(
        &mut self,
        fe_design: &FeDesign,
        (weights, tolerance): (&[f64], f64),
        max_iterations: usize,
        mut misses: usize,
        tally: &mut Tally,
    ) -> Result<JointLsmrReport> {
        #[cfg(test)]
        let executing_threads = rayon::current_num_threads();
        self.ensure_projections()?;
        let floor = NUMERICAL_FLOOR.min(tolerance);
        let scale = self
            .projections
            .as_ref()
            .expect("projections")
            .tolerance_scale;
        let mut attempt_tolerance = (tolerance * scale).clamp(floor, tolerance);
        let mut retried = false;
        loop {
            let started = Instant::now();
            let warm = if retried {
                Some(Warm::Solved)
            } else {
                self.projections
                    .as_ref()
                    .filter(|p| p.warm_start_valid)
                    .map(|_| Warm::Controls)
            };
            let projection = self.project(weights, attempt_tolerance, max_iterations, warm)?;
            tally.solve_seconds += started.elapsed().as_secs_f64();
            let certified = match projection {
                Projection::Certified { route, iterations } => {
                    tally.iterations = tally.iterations.saturating_add(iterations);
                    let started = Instant::now();
                    let certificate = self.finish_projections(fe_design, weights, tolerance)?;
                    tally.certification_seconds += started.elapsed().as_secs_f64();
                    certificate.map(|certificate| (route, certificate))
                }
                Projection::Declined { iterations } => {
                    tally.iterations = tally.iterations.saturating_add(iterations);
                    None
                }
            };
            let forced = misses > 0;
            misses = misses.saturating_sub(1);
            let projections = self.projections.as_mut().expect("projections");
            if let Some((route, certificate)) = certified.filter(|(_, c)| c.passed && !forced) {
                let controls = self.active_design.ncols() * self.fe_certificate.len();
                projections.start[..controls]
                    .copy_from_slice(&projections.coefficients[..controls]);
                projections.warm_start_valid = true;
                self.step.copy_from_slice(&projections.finite.step);
                return Ok(JointLsmrReport {
                    iterations: tally.iterations,
                    effective_tolerance: attempt_tolerance,
                    retries: tally.retries,
                    setup_seconds: 0.0,
                    solve_seconds: tally.solve_seconds,
                    certification_seconds: tally.certification_seconds,
                    warm_start_attempted: false,
                    formulation: Formulation::Projections,
                    route,
                    recurrence_residual: None,
                    certificate,
                    #[cfg(test)]
                    executing_threads,
                });
            }
            // The independent joint certificate demonstrated that the
            // projection scale was insufficient for this frame's controls;
            // later frames start tighter.
            projections.tolerance_scale =
                (projections.tolerance_scale * SCALE_STEP).max(MINIMUM_SCALE);
            let tighter = (attempt_tolerance * RETRY_FACTOR).max(floor);
            if retried || tighter >= attempt_tolerance {
                self.reset_warm_start();
                return Err(nonconvergence(
                    "explicit PPML LSMR correction did not pass its exact normal-equation certificate",
                ));
            }
            retried = true;
            tally.retries += 1;
            attempt_tolerance = tighter;
        }
    }

    /// Build the projections' fedemean projector and buffers on first use.
    fn ensure_projections(&mut self) -> Result<()> {
        if self.projections.is_some() {
            return Ok(());
        }
        let rows = self.target.len();
        let columns = self.active_design.ncols();
        let width = columns + 1;
        let total = self.fe_certificate.len();
        let values = total
            .checked_mul(width)
            .ok_or_else(|| allocation("explicit PPML LSMR coefficient storage overflows"))?;
        let settings = fedemean::Settings {
            choice: lsmr_choice(self.preconditioner),
            ..fedemean::Settings::default()
        };
        self.projections = Some(Projections {
            projector: fedemean::Projector::new(self.design.clone(), settings),
            memory_limit: crate::fe::block_schur::lsmr_workspace_bytes(
                rows,
                &self.level_counts,
                rayon::current_num_threads(),
            )?,
            projected: DenseMatrix::zeros(rows, width)?,
            start: fallible_filled(values, 0.0, "explicit PPML LSMR warm starts")?,
            coefficients: fallible_filled(values, 0.0, "explicit PPML LSMR FE coefficients")?,
            warm_start_valid: false,
            finite: FiniteBuffers::new(columns)?,
            tolerance_scale: 1.0,
        });
        Ok(())
    }

    /// Project `[X, z]` (refilled from the originals) at `tolerance`: cold,
    /// from the controls' previous coefficients (the correction from zero),
    /// or from this frame's solved coefficients.
    fn project(
        &mut self,
        weights: &[f64],
        tolerance: f64,
        max_iterations: usize,
        warm: Option<Warm>,
    ) -> Result<Projection> {
        let columns = self.active_design.ncols();
        let width = columns + 1;
        let total = self.fe_certificate.len();
        let effects = self.level_counts.len();
        let projections = self.projections.as_mut().expect("projections");
        fill_projection_input(
            &self.active_design,
            &self.target,
            &mut projections.projected,
        );
        match warm {
            Some(Warm::Controls) => projections.start[columns * total..].fill(0.0),
            Some(Warm::Solved) => projections.start.copy_from_slice(&projections.coefficients),
            None => {}
        }
        projections.coefficients.fill(0.0);
        let mut request = fedemean::Request::new(tolerance, max_iterations);
        request.memory_limit = Some(projections.memory_limit);
        let host = crate::parallel::HostPool::current()?;
        // A developer LSMR Break witness observes the solve's polls only: the
        // structure is built first under the ordinary poll.
        let armed = crate::fe::lsmr_break::armed(crate::fe::DeveloperLsmrBackend::Fedemean);
        let polls = std::sync::atomic::AtomicUsize::new(0);
        let witness = || crate::fe::lsmr_break::fedemean_poll(&polls);
        let poll: &(dyn Fn() -> bool + Sync) = if armed {
            projections
                .projector
                .prepare(&fedemean::Control::with_cancel(
                    &crate::interrupt::cancellation_requested,
                ))
                .map_err(fedemean_error)?;
            &witness
        } else {
            &crate::interrupt::cancellation_requested
        };
        let mut control = fedemean::Control::with_cancel(poll);
        if let Some(host) = &host {
            control = control.with_pool(host.pool());
        }
        let rows = projections.projected.nrows();
        let values = fedemean::ColumnsMut::new(projections.projected.as_mut_slice(), rows, width)
            .map_err(fedemean_error)?;
        let layout = fedemean::CoefficientLayout::new(
            total,
            &self.effect_offsets[..effects],
            &self.level_counts,
        );
        let coefficients = fedemean::CoefficientsMut::new(&mut projections.coefficients, layout);
        let fe_weights = fedemean::Weights::Values(weights);
        let outcome = match warm {
            Some(_) => projections.projector.project_from(
                fe_weights,
                values,
                &projections.start,
                Some(coefficients),
                &request,
                &control,
            ),
            None => projections.projector.project(
                fe_weights,
                values,
                Some(coefficients),
                &request,
                &control,
            ),
        }
        .map_err(|error| {
            if armed && error == fedemean::Error::Cancelled {
                crate::fe::lsmr_break::record_cancelled(crate::fe::DeveloperLsmrBackend::Fedemean);
            }
            fedemean_error(error)
        })?;
        Ok(match outcome {
            fedemean::Outcome::Projected(report) => Projection::Certified {
                route: route(&report.route),
                iterations: report.facts.column_iterations,
            },
            fedemean::Outcome::Declined {
                reason: fedemean::Decline::MemoryLimit,
                ..
            } => {
                return Err(allocation(
                    "explicit PPML LSMR projection exceeds its admitted workspace",
                ))
            }
            fedemean::Outcome::Declined { facts, reason, .. } => {
                if crate::diagnostics::solver_trace_enabled() {
                    crate::diagnostics::emit_solver_trace(&format!(
                        "FEREG_SOLVER_TRACE|phase=ppml_lsmr_projection_decline|reason={reason:?}|iterations={}",
                        facts.column_iterations
                    ));
                }
                Projection::Declined {
                    iterations: facts.column_iterations,
                }
            }
            _ => Projection::Declined { iterations: 0 },
        })
    }

    /// Solve the projected controls' normal equations, form the joint
    /// residual and predictor correction, and certify them exactly. `None`
    /// when the finite solve or the formed values are not representable.
    fn finish_projections(
        &mut self,
        fe_design: &FeDesign,
        weights: &[f64],
        tolerance: f64,
    ) -> Result<Option<JointLsmrCertificate>> {
        let columns = self.active_design.ncols();
        let projections = self.projections.as_mut().expect("projections");
        if columns > 0 {
            let finite = &mut projections.finite;
            let solved = crate::ols::solve_finite_direction(
                &projections.projected,
                weights,
                tolerance,
                crate::ols::FiniteDirectionWorkspace {
                    gram: &mut finite.gram,
                    rhs: &mut finite.rhs,
                    factor: &mut finite.factor,
                    forward: &mut finite.forward,
                    solution: &mut finite.solution,
                    scales: &mut finite.scales,
                    coefficients: &mut finite.step,
                    permutation: &mut finite.permutation,
                },
            );
            match solved {
                Ok(()) => {}
                Err(error) if error.code == ErrorCode::InvalidData => return Ok(None),
                Err(error) => return Err(error),
            }
        }
        let step = &projections.finite.step;
        let width = columns + 1;
        self.residual
            .par_iter_mut()
            .zip(self.delta_eta.par_iter_mut())
            .zip(projections.projected.as_slice().par_chunks_exact(width))
            .zip(self.target.par_iter())
            .for_each(|(((residual, delta), values), &target)| {
                let mut value = values[columns];
                for (&projected, &coefficient) in values[..columns].iter().zip(step) {
                    value -= projected * coefficient;
                }
                *residual = value;
                *delta = target - value;
            });
        if self
            .residual
            .iter()
            .chain(&self.delta_eta)
            .any(|value| !value.is_finite())
        {
            return Ok(None);
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
        .map(Some)
    }

    pub(crate) fn fill_candidate(&self, workspace: &mut PpmlIterationWorkspace) -> Result<()> {
        self.team.install(|| {
            workspace.fill_incremental_candidate(&self.delta_eta, &self.active_slots, &self.step)
        })
    }

    pub(crate) fn active_rank(&self) -> usize {
        self.active_slots.len()
    }

    pub(crate) fn active_slots(&self) -> &[usize] {
        &self.active_slots
    }

    /// Scale the joint recurrence's warm state to the actually accepted IRLS
    /// step. A strict canonical synchronization discards it instead.
    pub(crate) fn accept_step(&mut self, step: f64) {
        if self.joint.warm_start_valid {
            for value in &mut self.joint.warm_start {
                *value *= step;
            }
        }
    }

    /// Forget every warm start (a sample, basis or failed frame).
    pub(crate) fn reset_warm_start(&mut self) {
        self.joint.warm_start.fill(0.0);
        self.joint.warm_start_valid = false;
        if let Some(projections) = self.projections.as_mut() {
            projections.warm_start_valid = false;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_once(
        &self,
        fe_design: &FeDesign,
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
            &self.joint.sqrt_weights,
        )?;
        let preconditioner = BlockPreconditioner {
            finite_inverse: &self.joint.finite_inverse,
            finite_factor: self
                .joint
                .finite_factor_ready
                .then_some(self.joint.finite_factor.as_slice()),
            fe_inverse: &self.joint.fe_inverse,
        };
        // A developer LSMR Break witness parks in the polls of the first
        // iterations, as fedemean's do.
        let armed = crate::fe::lsmr_break::armed(crate::fe::DeveloperLsmrBackend::Fedemean);
        let polls = std::sync::atomic::AtomicUsize::new(0);
        let witness = || crate::fe::lsmr_break::fedemean_poll(&polls);
        let poll: &(dyn Fn() -> bool + Send + Sync) = if armed {
            &witness
        } else {
            &crate::interrupt::cancellation_requested
        };
        mlsmr_cancellable(
            &operator,
            &self.joint.rhs,
            &preconditioner,
            tolerance,
            max_iterations,
            MlsmrOptions {
                warm_start,
                escalation,
                local_size,
            },
            schwarz_precond::Cancellation::new(poll),
        )
        .map_err(|error| {
            if armed && matches!(error, SolveError::Cancelled) {
                crate::fe::lsmr_break::record_cancelled(crate::fe::DeveloperLsmrBackend::Fedemean);
            }
            lsmr_error(error)
        })
    }

    /// Form the joint iterate's predictor correction and residual and
    /// certify them exactly.
    fn finish_joint(
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
            &self.joint.sqrt_weights,
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
            || self.target.len() != rows
            || [outcome.len(), mean.len(), hessian_weights.len()]
                .iter()
                .any(|length| *length != rows)
            || !tolerance.is_finite()
            || tolerance <= 0.0
            || max_iterations == 0
        {
            return Err(invalid_argument(
                "explicit PPML LSMR frame dimensions or controls are invalid",
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
                "explicit PPML LSMR frame contains an invalid outcome, mean, or weight",
            ));
        }
        Ok(())
    }
}

/// Where a projection starts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Warm {
    /// The controls at their previous frame's coefficients, the correction
    /// at zero.
    Controls,
    /// Every column at this frame's last solved coefficients (the retry).
    Solved,
}

fn emit_frame_trace(scheduled: f64, report: &JointLsmrReport) {
    if !crate::diagnostics::solver_trace_enabled() {
        return;
    }
    crate::diagnostics::emit_solver_trace(&format!(
        "FEREG_SOLVER_TRACE|phase=ppml_lsmr_frame|formulation={:?}|scheduled={scheduled:.17e}|effective={:.17e}|iterations={}|retries={}|warm={}|preconditioner={:?}|escalations={}|certificate={:.17e}|solve_seconds={:.17e}|certification_seconds={:.17e}",
        report.formulation,
        report.effective_tolerance,
        report.iterations,
        report.retries,
        u8::from(report.warm_start_attempted),
        report.route.preconditioner,
        report.route.escalations,
        report.certificate.relative_infinity_norm,
        report.solve_seconds,
        report.certification_seconds,
    ));
}

/// Row `i` of `projected` becomes `[X_i, z_i]`.
fn fill_projection_input(active: &DenseMatrix, target: &[f64], projected: &mut DenseMatrix) {
    let columns = active.ncols();
    let width = columns + 1;
    let source = active.as_slice();
    projected
        .as_mut_slice()
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(row, output)| {
            output[..columns].copy_from_slice(&source[row * columns..(row + 1) * columns]);
            output[columns] = target[row];
        });
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
            "explicit PPML LSMR frame preparation dimensions are inconsistent",
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
                    "explicit PPML LSMR prior Hessian weight state is invalid",
                ));
            }
            let log_change = 2.0 * (root_weight / prior_root_weight).ln().abs();
            if !log_change.is_finite() {
                return Err(nonconvergence(
                    "explicit PPML LSMR Hessian weight transition is nonfinite",
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
                "explicit PPML LSMR Newton target is not finitely representable",
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
                "explicit PPML LSMR finite diagonal preconditioner is nonfinite",
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
        .ok_or_else(|| allocation("explicit PPML LSMR active design dimensions overflow"))?;
    let mut values = fallible_filled(elements, 0.0, "explicit PPML LSMR active design")?;
    values
        .par_chunks_mut(columns.max(1))
        .enumerate()
        .try_for_each(|(row, destination)| -> Result<()> {
            if columns == 0 {
                return Ok(());
            }
            for (active, &generated) in active_generated.iter().enumerate() {
                let value = prepared.fit_column_value(generated, row)?;
                if !value.is_finite() {
                    return Err(crate::error::invalid_data(
                        "explicit PPML LSMR active control contains a nonfinite value",
                    ));
                }
                destination[active] = value;
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
                "explicit PPML LSMR augmented operator dimensions are inconsistent",
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
                "explicit PPML LSMR predictor dimensions are inconsistent",
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
                "explicit PPML LSMR augmented operator produced a nonfinite predictor",
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

/// The joint recurrence's preconditioner: the controls' current-weight
/// Gram factor (or diagonal) and the FE block's inverse level masses (or
/// the identity).
struct BlockPreconditioner<'a> {
    finite_inverse: &'a [f64],
    finite_factor: Option<&'a [f64]>,
    fe_inverse: &'a [f64],
}

impl Operator for BlockPreconditioner<'_> {
    fn nrows(&self) -> usize {
        self.finite_inverse.len() + self.fe_inverse.len()
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
        for ((output, input), scale) in y[finite..]
            .iter_mut()
            .zip(&x[finite..])
            .zip(self.fe_inverse)
        {
            *output = *scale * *input;
        }
        Ok(())
    }

    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        self.apply(x, y)
    }
}

fn fill_fe_diagonal_inverse(design: &FeDesign, weights: &[f64], values: &mut [f64]) -> Result<()> {
    if values.len() != design.total_coefficients() {
        return Err(invalid_argument(
            "explicit PPML LSMR FE diagonal dimensions are inconsistent",
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
            "explicit PPML LSMR FE diagonal preconditioner is nonfinite",
        ));
    }
    Ok(())
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

/// Conservative heap peak added by the explicit PPML LSMR engine: the
/// retained controls and frame vectors, the joint recurrence's warm state,
/// block preconditioner and `mlsmr` vectors (with its four-vector local
/// reorthogonalization window and a retained continuation iterate), and,
/// unless `preconditioner` keeps the joint recurrence (`Diagonal`, `Off`),
/// the projections' projected matrix, FE coefficients and fedemean
/// workspace (with its Schwarz grant).
pub(crate) fn estimate_joint_lsmr_workspace_bytes(
    rows: usize,
    active_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
    preconditioner: LsmrPreconditioner,
) -> Result<u128> {
    let projections = !matches!(
        preconditioner,
        LsmrPreconditioner::Diagonal | LsmrPreconditioner::Off
    );
    let fedemean =
        crate::fe::block_schur::lsmr_workspace_bytes(rows, effect_level_counts, threads)?;
    let n = rows as u128;
    let k = active_columns as u128;
    let levels = effect_level_counts
        .iter()
        .try_fold(0_u128, |total, &levels| {
            checked_add_memory(total, levels as u128, "PPML LSMR total levels")
        })?;
    let width = checked_add_memory(k, 1, "PPML LSMR projection width")?;
    let p = checked_add_memory(k, levels, "PPML LSMR augmented width")?;
    // Retained: controls, target/residual/predictor, square-root weights and
    // right-hand side, warm state, finite factor and diagonals, the FE
    // block, the step.
    let retained = checked_sum_memory(&[
        checked_mul_memory(n, k, "PPML LSMR retained controls")?,
        checked_mul_memory(n, 5, "PPML LSMR frame vectors")?,
        p,
        checked_mul_memory(k, k, "PPML LSMR finite factor")?,
        checked_mul_memory(k, 2, "PPML LSMR finite vectors")?,
        levels,
    ])?;
    // `mlsmr`: 2N + (6 + 2 * 4) P values with the local window, and two
    // retained P-vectors across a continuation.
    let recurrence = checked_sum_memory(&[
        checked_mul_memory(n, 2, "PPML LSMR recurrence observation vectors")?,
        checked_mul_memory(p, 16, "PPML LSMR recurrence coefficient vectors")?,
    ])?;
    let projected = if projections {
        checked_sum_memory(&[
            checked_mul_memory(n, width, "PPML LSMR projected columns")?,
            checked_mul_memory(
                checked_mul_memory(levels, width, "PPML LSMR coefficients")?,
                2,
                "PPML LSMR warm starts",
            )?,
            FiniteBuffers::values(k)?,
        ])?
    } else {
        0
    };
    let indices = checked_sum_memory(&[
        checked_mul_memory(k, 2, "PPML LSMR slots")?,
        if projections { k } else { 0 },
        checked_mul_memory(
            effect_level_counts.len() as u128 + 1,
            2,
            "PPML LSMR coefficient layout",
        )?,
    ])?;
    checked_sum_memory(&[
        bytes_for_memory::<f64>(
            checked_sum_memory(&[retained, recurrence, projected])?,
            "PPML LSMR retained values",
        )?,
        bytes_for_memory::<usize>(indices, "PPML LSMR retained indices")?,
        bytes_for_memory::<DirectionCertificate>(
            checked_add_memory(k, levels, "PPML LSMR certificate directions")?,
            "PPML LSMR certificate scratch",
        )?,
        if projections { fedemean } else { 0 },
    ])
}

/// Conservative transient peak for one canonical current-weight checkpoint.
///
/// The checkpoint batch builds fedemean's weighted state and its modified
/// LSMR workspace (and any additive Schwarz preconditioner) for the current
/// weights, so no ordinary-frame preconditioner or residual can become
/// scientific evidence. The estimate charges that solver frame, the
/// checkpoint's input and candidate matrices and the certificate workspace
/// in addition to the caller-owned canonical session.
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
    // fedemean's projection workspace, the projected copy of the
    // checkpoint's columns, the candidate it certifies and the certificate
    // workspace, on top of the retained session.
    let solver = crate::fe::block_schur::lsmr_workspace_bytes(rows, effect_level_counts, threads)?;
    let matrix = checked_mul_memory(rows as u128, rhs as u128, "checkpoint LSMR input matrix")?;
    checked_sum_memory(&[
        solver,
        bytes_for_memory::<f64>(
            checked_mul_memory(
                matrix,
                2,
                "checkpoint LSMR projected and candidate matrices",
            )?,
            "checkpoint LSMR matrix bytes",
        )?,
        bytes_for_memory::<f64>(rows as u128, "checkpoint LSMR certificate workspace")?,
    ])
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
        format!("explicit PPML LSMR failed: {error}"),
    )
}

fn solve_shape_error(context: &'static str) -> SolveError {
    SolveError::InvalidInput {
        context,
        message: "explicit PPML LSMR augmented operator dimensions are inconsistent".to_owned(),
    }
}

fn nonconvergence(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
mod tests;
