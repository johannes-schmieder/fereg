//! Modified LSMR on up to [`MAX_COLUMNS`] right-hand sides in lockstep.
//!
//! [`mlsmr_columns`] runs [`mlsmr`](super::mlsmr)'s recurrence for every column of an
//! interleaved block (entry `i` of column `j` at `i · k + j`) with one application of
//! the operator, its adjoint and the preconditioner per iteration for all running
//! columns ([`Operator::apply_columns`]). Each column keeps its own tolerance,
//! iteration budget, warm start, escalation handler, exit audits and restarts.
//!
//! The block's vector kernels keep `mlsmr`'s per-column arithmetic and summation
//! order (leaves of [`LSMR_UPDATE_CHUNK`] rows with [`LANES`] accumulators, combined
//! in a tree fixed by the length). Starts, stops, audits and restarts run per column
//! on one contiguous single-column stream with `mlsmr`'s own code. So when the
//! operator's and preconditioner's column applications give every column the bits of
//! a single application, every column returns `mlsmr`'s bits. A column that finishes
//! leaves the block, which narrows.
//
// Copyright (c) 2026 within contributors
// Copyright (c) 2026 appliedAI Institute for Europe gGmbH
// MIT License; see the notice in `crates/fedemean/src/lsmr.rs`.

use std::borrow::Cow;

use rayon::prelude::*;

use super::bidiag::{
    alpha_from_vp, axpby, normalize, BidiagStep, Bidiagonalization, ModifiedGolubKahan, LANES,
    LSMR_PAR_THRESHOLD, LSMR_UPDATE_CHUNK, RAW_U_NORMS,
};
use super::magnitude::{exponent, ldexp, Magnitude};
use super::recurrence::{
    update_coefficients, ConvergenceCriteria, ConvergenceState, LsmrRecurrenceState, RotationStep,
    Stop,
};
use super::{
    finite, invalid_input, mlsmr_counted_cancellable, vec_norm, EscalationHandler,
    EscalationPolicy, LsmrFailure, LsmrResult, LsmrStopReason, MlsmrOptions, NormalEqReference,
    Progress, MAX_RESTARTS,
};
use crate::lsmr::fold::fold;
use crate::lsmr::{Cancellation, Operator, SolveError};

/// Most columns [`mlsmr_columns`] runs in lockstep: eight doubles fill one cache line.
pub const MAX_COLUMNS: usize = 8;

/// One value per block column.
type Lanes = [f64; MAX_COLUMNS];
/// Which block columns a kernel touches.
type Mask = [bool; MAX_COLUMNS];

/// One column of [`mlsmr_columns`]: what [`mlsmr`](super::mlsmr) takes per call.
#[derive(Clone, Copy)]
pub struct ColumnSolve<'a> {
    /// Tolerance, relative to `‖b‖` as for `mlsmr`.
    pub tol: f64,
    /// Iteration budget.
    pub maxiter: usize,
    /// Residual-correction start; see [`MlsmrOptions::warm_start`].
    pub warm_start: Option<&'a [f64]>,
    /// Hands this column off mid-run; see [`MlsmrOptions::escalation`].
    pub escalation: Option<&'a dyn EscalationPolicy>,
}

/// Options shared by the columns of [`mlsmr_columns`].
#[derive(Clone, Copy, Default)]
pub struct MlsmrColumnsOptions {
    /// Local reorthogonalization window; with one, the columns run one at a time.
    pub local_size: Option<usize>,
    /// Return each certified column's recomputed residual, as `mlsmr` does (`k` more
    /// observation-sized vectors at once).
    pub true_residuals: bool,
}

/// [`mlsmr_counted_cancellable`] on the `k = columns.len()` interleaved columns of
/// `b` (`nrows · k` values) in lockstep, one result per column.
///
/// A column's result (and with [`MlsmrColumnsOptions::true_residuals`], its residual)
/// is the one `mlsmr` returns for that column alone, to its bits when the operator's
/// and the preconditioner's [`Operator::apply_columns`] give each column the bits of
/// a single application. Cancellation is checked once per lockstep iteration; it, an
/// invalid block shape or a failed application fails every column still running,
/// while a column's own numerical failure ends only that column.
pub fn mlsmr_columns<A: Operator + ?Sized, M: Operator + ?Sized>(
    operator: &A,
    b: &[f64],
    preconditioner: &M,
    columns: &[ColumnSolve<'_>],
    options: MlsmrColumnsOptions,
    cancellation: Cancellation<'_>,
) -> Vec<Result<LsmrResult, LsmrFailure>> {
    let k = columns.len();
    let failed = |source: SolveError| {
        columns
            .iter()
            .map(|_| {
                Err(LsmrFailure {
                    source: source.clone(),
                    iterations: 0,
                })
            })
            .collect()
    };
    let (rows, cols) = (operator.nrows(), operator.ncols());
    if k > MAX_COLUMNS {
        return failed(invalid_input(format!(
            "{k} columns exceed the lockstep limit of {MAX_COLUMNS}"
        )));
    }
    if b.len() != rows * k {
        return failed(invalid_input(format!(
            "rhs block length {} does not match operator row count {rows} times {k} columns",
            b.len()
        )));
    }
    if preconditioner.nrows() != cols || preconditioner.ncols() != cols {
        return failed(invalid_input(format!(
            "preconditioner shape {}x{} must match operator column count {cols}",
            preconditioner.nrows(),
            preconditioner.ncols(),
        )));
    }
    if k == 0 {
        return Vec::new();
    }
    if options.local_size.unwrap_or(0) > 0 {
        return one_at_a_time(operator, b, preconditioner, columns, options, cancellation);
    }
    if let Err(source) = cancellation.checkpoint() {
        return failed(source);
    }
    let mut batch = match Batch::new(operator, preconditioner, b, columns, options, cancellation) {
        Ok(batch) => batch,
        Err(source) => return failed(source),
    };
    batch.start();
    batch.run();
    let cancelled = cancellation.checkpoint().err();
    batch
        .columns
        .into_iter()
        .map(|column| match (&cancelled, column.outcome) {
            (Some(source), _) => Err(LsmrFailure {
                source: source.clone(),
                iterations: column.iterations,
            }),
            (None, Some(outcome)) => outcome,
            (None, None) => unreachable!("every column ends with an outcome"),
        })
        .collect()
}

/// With local reorthogonalization, `mlsmr` on each column in turn.
fn one_at_a_time<A: Operator + ?Sized, M: Operator + ?Sized>(
    operator: &A,
    b: &[f64],
    preconditioner: &M,
    columns: &[ColumnSolve<'_>],
    options: MlsmrColumnsOptions,
    cancellation: Cancellation<'_>,
) -> Vec<Result<LsmrResult, LsmrFailure>> {
    let k = columns.len();
    let mut rhs = match reserved(operator.nrows()) {
        Ok(rhs) => rhs,
        Err(source) => {
            return columns
                .iter()
                .map(|_| {
                    Err(LsmrFailure {
                        source: source.clone(),
                        iterations: 0,
                    })
                })
                .collect()
        }
    };
    columns
        .iter()
        .enumerate()
        .map(|(j, column)| {
            gather(b, k, j, &mut rhs);
            let column_options = MlsmrOptions {
                warm_start: column.warm_start,
                escalation: column.escalation,
                local_size: options.local_size,
            };
            let mut result = mlsmr_counted_cancellable(
                operator,
                &rhs,
                preconditioner,
                column.tol,
                column.maxiter,
                column_options,
                cancellation,
            )?;
            if !options.true_residuals {
                result.true_residual = None;
            }
            Ok(result)
        })
        .collect()
}

/// One pass of a column's recurrence: what `lsmr_from_bidiag` keeps per pass.
struct Pass {
    handler: Option<Box<dyn EscalationHandler>>,
    convergence: ConvergenceState,
    recurrence: LsmrRecurrenceState,
    /// `β₁` of the pass, which the solution recurrence restores.
    rhs_norm: f64,
    prev: RotationStep,
}

/// A column's state outside the block's vectors.
struct Column<'a> {
    spec: ColumnSolve<'a>,
    b_norm: f64,
    criteria: ConvergenceCriteria,
    reference: NormalEqReference,
    /// The stream's last `α` and `1/‖u‖` as stored.
    alpha: f64,
    u_norm_inv: f64,
    iterations: usize,
    restarts: usize,
    base: Option<Cow<'a, [f64]>>,
    pass: Option<Pass>,
    outcome: Option<Result<LsmrResult, LsmrFailure>>,
}

impl Column<'_> {
    fn running(&self) -> bool {
        self.outcome.is_none()
    }
}

/// The running columns' vectors, interleaved at the block's width, and one
/// contiguous single-column stream.
struct Batch<'a, A: Operator + ?Sized, M: Operator + ?Sized> {
    operator: &'a A,
    preconditioner: &'a M,
    /// The caller's right-hand sides, interleaved at the original width.
    b: &'a [f64],
    k0: usize,
    rows: usize,
    cols: usize,
    true_residuals: bool,
    cancellation: Cancellation<'a>,
    columns: Vec<Column<'a>>,
    /// The column of each block slot, in column order.
    slots: Vec<usize>,
    u: Vec<f64>,
    av: Vec<f64>,
    v: Vec<f64>,
    p: Vec<f64>,
    atu: Vec<f64>,
    x: Vec<f64>,
    h: Vec<f64>,
    hbar: Vec<f64>,
    spare: Vec<f64>,
    /// Audits, restarts and rare paths, with `mlsmr`'s own code.
    single: ModifiedGolubKahan<'a, A, M>,
    rhs: Vec<f64>,
    /// Contiguous copies of one column's `v` and `p̃` for `alpha_from_vp`'s rare paths.
    scratch: (Vec<f64>, Vec<f64>),
}

fn reserved(len: usize) -> Result<Vec<f64>, SolveError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| SolveError::Allocation {
            context: "mlsmr_columns",
        })?;
    values.resize(len, 0.0);
    Ok(values)
}

impl<'a, A: Operator + ?Sized, M: Operator + ?Sized> Batch<'a, A, M> {
    fn new(
        operator: &'a A,
        preconditioner: &'a M,
        b: &'a [f64],
        specs: &[ColumnSolve<'a>],
        options: MlsmrColumnsOptions,
        cancellation: Cancellation<'a>,
    ) -> Result<Self, SolveError> {
        let k = specs.len();
        let (rows, cols) = (operator.nrows(), operator.ncols());
        let columns = specs
            .iter()
            .map(|&spec| Column {
                spec,
                b_norm: 0.0,
                criteria: ConvergenceCriteria::new(0.0, 0.0),
                reference: NormalEqReference {
                    metric: Magnitude::from(0.0),
                    plain: Magnitude::from(0.0),
                },
                alpha: 0.0,
                u_norm_inv: 1.0,
                iterations: 0,
                restarts: 0,
                base: spec.warm_start.map(Cow::Borrowed),
                pass: None,
                outcome: None,
            })
            .collect();
        Ok(Self {
            operator,
            preconditioner,
            b,
            k0: k,
            rows,
            cols,
            true_residuals: options.true_residuals,
            cancellation,
            columns,
            slots: (0..k).collect(),
            u: reserved(rows * k)?,
            av: reserved(rows * k)?,
            v: reserved(cols * k)?,
            p: reserved(cols * k)?,
            atu: reserved(cols * k)?,
            x: reserved(cols * k)?,
            h: reserved(cols * k)?,
            hbar: reserved(cols * k)?,
            spare: reserved(cols * k)?,
            single: ModifiedGolubKahan::new(operator, preconditioner, 0),
            rhs: reserved(rows)?,
            scratch: (reserved(cols)?, reserved(cols)?),
        })
    }

    fn width(&self) -> usize {
        self.slots.len()
    }

    fn fail(&mut self, c: usize, source: SolveError) {
        let column = &mut self.columns[c];
        column.pass = None;
        column.outcome = Some(Err(LsmrFailure {
            source,
            iterations: column.iterations,
        }));
    }

    fn done(&mut self, c: usize, result: LsmrResult) {
        let column = &mut self.columns[c];
        column.pass = None;
        column.outcome = Some(Ok(result));
    }

    /// Fail every running column (a failed application or a cancellation).
    fn fail_running(&mut self, source: SolveError) {
        for c in self.slots.clone() {
            if self.columns[c].running() {
                self.fail(c, source.clone());
            }
        }
        self.slots.clear();
    }

    /// The mask of the block slots whose column satisfies `f`.
    fn mask(&self, f: impl Fn(&Column<'a>) -> bool) -> Mask {
        let mut mask = [false; MAX_COLUMNS];
        for (s, &c) in self.slots.iter().enumerate() {
            mask[s] = f(&self.columns[c]);
        }
        mask
    }

    /// Per-column input checks, in `mlsmr`'s order and words.
    fn validate(&self, c: usize) -> Result<(), SolveError> {
        let spec = &self.columns[c].spec;
        if !spec.tol.is_finite() || spec.tol < 0.0 {
            return Err(invalid_input(format!(
                "tolerance must be finite and nonnegative, got {}",
                spec.tol
            )));
        }
        let rhs = self.b.iter().skip(c).step_by(self.k0);
        if let Some((index, value)) = rhs.copied().enumerate().find(|(_, v)| !v.is_finite()) {
            return Err(invalid_input(format!(
                "rhs entry {index} must be finite, got {value}"
            )));
        }
        if let Some(x0) = spec.warm_start {
            if x0.len() != self.cols {
                return Err(invalid_input(format!(
                    "warm-start length {} does not match operator column count {}",
                    x0.len(),
                    self.cols
                )));
            }
            if let Some((index, value)) =
                x0.iter().copied().enumerate().find(|(_, v)| !v.is_finite())
            {
                return Err(invalid_input(format!(
                    "warm-start entry {index} must be finite, got {value}"
                )));
            }
        }
        Ok(())
    }

    /// `mlsmr_impl` for every column at once: the staged start `b − A x₀`, the warm
    /// gradient references, the first restart and the first pass. The block still has
    /// its original width, so slot `s` is column `s`.
    fn start(&mut self) {
        let (k, cols) = (self.k0, self.cols);
        for c in 0..k {
            if let Err(source) = self.validate(c) {
                self.fail(c, source);
                continue;
            }
            match finite(vec_norm_column(self.b, k, c), "rhs norm") {
                Ok(norm) => self.columns[c].b_norm = norm,
                Err(source) => self.fail(c, source),
            }
        }
        // A zero start vector needs no stream; a warm one is formed in the stream.
        let warm = self.mask(|c| c.running() && c.spec.warm_start.is_some());
        let mut rhs_norm = [0.0; MAX_COLUMNS];
        if warm.contains(&true) {
            for (c, column) in self.columns.iter().enumerate() {
                if let (true, Some(x0)) = (warm[c], column.spec.warm_start) {
                    scatter(x0, &mut self.x, k, c);
                }
            }
            if let Err(source) = self.operator.apply_columns(k, &self.x, &mut self.u) {
                return self.fail_running(source);
            }
            let sq = axpy_sq(&mut self.u, self.b, k, &[-1.0; MAX_COLUMNS], &warm);
            for c in (0..k).filter(|&c| warm[c]) {
                let norm = norm_from_sq_column(&self.u, k, c, sq[c]);
                // Unlike `b`, the residual is computed: an ∞ entry norms to NaN.
                match finite(norm, "warm-start residual norm") {
                    Ok(norm) => rhs_norm[c] = norm,
                    Err(source) => self.fail(c, source),
                }
            }
            self.x.fill(0.0);
        }
        for c in 0..k {
            if self.columns[c].running() && !warm[c] {
                copy_column(&mut self.u, self.b, k, c);
                rhs_norm[c] = self.columns[c].b_norm;
            }
        }
        for (c, &norm) in rhs_norm.iter().enumerate().take(k) {
            let column = &self.columns[c];
            if column.running() && norm == 0.0 {
                let (x, stop_reason) = match column.spec.warm_start {
                    Some(x0) => (x0.to_vec(), LsmrStopReason::WarmStartExact),
                    None => (vec![0.0; cols], LsmrStopReason::ZeroRhs),
                };
                self.done(
                    c,
                    LsmrResult {
                        x,
                        converged: true,
                        iterations: 0,
                        residual_norm: 0.0,
                        normal_eq_residual: 0.0,
                        stop_reason,
                        true_residual: None,
                    },
                );
            }
        }
        let references = self.references(&warm);
        let running = self.mask(Column::running);
        let Some(steps) = self.restart_block(&running, &rhs_norm) else {
            return;
        };
        let sq = dot_sq(&self.p, &self.p, k, &running);
        let started: Vec<usize> = (0..k)
            .filter(|&c| running[c] && self.columns[c].running())
            .collect();
        for c in started {
            let cold_plain =
                Magnitude::product(norm_from_sq_column(&self.p, k, c, sq[c]), rhs_norm[c]);
            let column = &mut self.columns[c];
            column.reference = match references[c] {
                Some((metric, plain)) => {
                    NormalEqReference::warm(metric, plain, steps[c], cold_plain)
                }
                None => NormalEqReference::cold(steps[c], cold_plain),
            };
            let reference_norm = if column.b_norm > 0.0 {
                column.b_norm
            } else {
                rhs_norm[c]
            };
            column.criteria = ConvergenceCriteria::new(reference_norm, column.spec.tol);
            self.begin_pass(c, c, steps[c]);
        }
        self.compact();
    }

    /// The warm columns' `StagedStart::gradient_references` against `b` (those with
    /// `‖b‖ > 0`), on the block's scratch; rare scalings take the single stream.
    fn references(&mut self, warm: &Mask) -> [Option<(Magnitude, Magnitude)>; MAX_COLUMNS] {
        let k = self.k0;
        let mut out = [None; MAX_COLUMNS];
        let positive = self.mask(|c| c.running() && c.b_norm > 0.0);
        let wanted: Mask = std::array::from_fn(|s| positive[s] && warm[s]);
        if !wanted.contains(&true) {
            return out;
        }
        if let Err(source) = self
            .operator
            .apply_adjoint_columns(k, self.b, &mut self.atu)
        {
            self.fail_running(source);
            return out;
        }
        let sq = dot_sq(&self.atu, &self.atu, k, &wanted);
        let mut scale = [0.0; MAX_COLUMNS];
        let mut metric = [false; MAX_COLUMNS];
        let mut plains = [Magnitude::from(0.0); MAX_COLUMNS];
        for c in (0..k).filter(|&c| wanted[c]) {
            let b_norm = self.columns[c].b_norm;
            let g_norm = norm_from_sq_column(&self.atu, k, c, sq[c]);
            // `adjoint_image` keeps the plain image unless it or `‖b‖` is not normal.
            if !(b_norm.is_normal() && (exponent(b_norm) == 0 || g_norm.is_normal())) {
                gather(self.b, k, c, &mut self.rhs);
                match self.single.gradient_references(&self.rhs, b_norm) {
                    Ok(references) => out[c] = Some(references),
                    Err(source) => self.fail(c, source),
                }
                continue;
            }
            let plain = Magnitude::product(g_norm, ldexp(1.0, 0));
            if !g_norm.is_finite() {
                out[c] = Some((Magnitude::from(g_norm), plain));
                continue;
            }
            scale[c] = if g_norm.is_normal() { g_norm } else { 1.0 };
            metric[c] = true;
            plains[c] = plain;
        }
        if !metric.contains(&true) {
            return out;
        }
        normalize_columns(&mut self.atu, k, &scale, &metric);
        if let Err(source) = self.preconditioner.apply_columns(k, &self.atu, &mut self.v) {
            self.fail_running(source);
            return out;
        }
        let vp = dot_sq(&self.v, &self.atu, k, &metric);
        for c in (0..k).filter(|&c| metric[c]) {
            match alpha_of((&self.v, &self.atu), k, c, vp[c], &mut self.scratch) {
                Ok(alpha) => {
                    let metric =
                        Magnitude::product(ldexp(1.0, 0), scale[c]) * Magnitude::from(alpha);
                    out[c] = Some((metric, plains[c]));
                }
                Err(source) => self.fail(c, source),
            }
        }
        out
    }

    /// The stream's `restart(β)` on the slots of `on` (the block at its original
    /// width): `u /= β`, `p̃ = Aᵀu`, `v = M⁻¹p̃`, `α = √⟨v, p̃⟩`, `v /= α`. `None` when an
    /// application failed (every running column has failed).
    fn restart_block(&mut self, on: &Mask, beta: &Lanes) -> Option<[BidiagStep; MAX_COLUMNS]> {
        let w = self.width();
        let mut steps = [BidiagStep {
            alpha: 0.0,
            beta: 0.0,
        }; MAX_COLUMNS];
        if !on.contains(&true) {
            return Some(steps);
        }
        normalize_columns(&mut self.u, w, beta, on);
        if let Err(source) = self.operator.apply_adjoint_columns(w, &self.u, &mut self.p) {
            self.fail_running(source);
            return None;
        }
        if let Err(source) = self.preconditioner.apply_columns(w, &self.p, &mut self.v) {
            self.fail_running(source);
            return None;
        }
        let vp = dot_sq(&self.v, &self.p, w, on);
        let mut alpha = [0.0; MAX_COLUMNS];
        let mut good = *on;
        for s in (0..w).filter(|&s| on[s]) {
            match alpha_of((&self.v, &self.p), w, s, vp[s], &mut self.scratch) {
                Ok(a) => alpha[s] = a,
                Err(source) => {
                    good[s] = false;
                    self.fail(self.slots[s], source);
                }
            }
        }
        normalize_columns(&mut self.v, w, &alpha, &good);
        for s in (0..w).filter(|&s| good[s]) {
            let column = &mut self.columns[self.slots[s]];
            column.alpha = alpha[s];
            column.u_norm_inv = 1.0;
            steps[s] = BidiagStep {
                alpha: alpha[s],
                beta: beta[s],
            };
        }
        Some(steps)
    }

    /// `lsmr_from_bidiag`'s pass start for column `c` in slot `s` from `step1`.
    fn begin_pass(&mut self, c: usize, s: usize, step1: BidiagStep) {
        let w = self.width();
        // A metric reporting no gradient at all may be hiding one outside itself.
        if step1.alpha == 0.0 {
            return self.hidden(c, s, step1);
        }
        let column = &mut self.columns[c];
        column.pass = Some(Pass {
            // A pass reports its drop from its own ζ̄₀, so a restarted one needs a handler that agrees.
            handler: column.spec.escalation.map(EscalationPolicy::handler),
            convergence: column.criteria.start(step1.alpha),
            recurrence: LsmrRecurrenceState::init(step1),
            rhs_norm: step1.beta,
            prev: RotationStep::initial(),
        });
        fill_column(&mut self.x, w, s, 0.0);
        fill_column(&mut self.hbar, w, s, 0.0);
        let (h, v) = (&mut self.h, &self.v);
        for (target, &value) in h
            .iter_mut()
            .skip(s)
            .step_by(w)
            .zip(v.iter().skip(s).step_by(w))
        {
            *target = value;
        }
    }

    /// `lsmr_from_bidiag`'s `α₁ = 0` exit for column `c` in slot `s`.
    fn hidden(&mut self, c: usize, s: usize, step1: BidiagStep) {
        let w = self.width();
        let cols = self.cols;
        gather(&self.p, w, s, &mut self.single.bufs.p_tilde);
        gather(self.b, self.k0, c, &mut self.rhs);
        let b_norm = vec_norm(&self.rhs);
        let criteria = self.columns[c].criteria;
        let (converged, normal_eq_residual) = match self.single.hidden_gradient(&self.rhs, b_norm) {
            Err(source) => return self.fail(c, source),
            Ok(None) => (true, 0.0),
            Ok(Some((per_unit_residual, b_image))) => {
                let normar = Magnitude::product(step1.beta, per_unit_residual);
                let plain = b_image.norm();
                (
                    criteria.corroborates(step1.beta, normar, b_image.per_unit()),
                    if plain.is_normal() {
                        (normar / plain).to_f64()
                    } else {
                        1.0
                    },
                )
            }
        };
        let column = &mut self.columns[c];
        let x = column
            .base
            .take()
            .map_or_else(|| vec![0.0; cols], Cow::into_owned);
        let iterations = column.iterations;
        self.done(
            c,
            LsmrResult {
                x,
                converged,
                iterations,
                residual_norm: step1.beta,
                normal_eq_residual,
                stop_reason: if converged {
                    LsmrStopReason::InitialNormalEquationResidualZero
                } else {
                    LsmrStopReason::FalseConvergence
                },
                true_residual: None,
            },
        );
    }

    /// Lockstep iterations until every column has finished.
    fn run(&mut self) {
        while !self.slots.is_empty() {
            for s in 0..self.width() {
                let c = self.slots[s];
                let column = &self.columns[c];
                if column.pass.is_some() && column.iterations >= column.spec.maxiter {
                    self.end_pass(c, s, LsmrStopReason::MaxIterations);
                }
            }
            self.compact();
            if self.slots.is_empty() {
                break;
            }
            if let Err(source) = self.cancellation.checkpoint() {
                return self.fail_running(source);
            }
            for &c in &self.slots {
                self.columns[c].iterations += 1;
            }
            let Some(steps) = self.step() else {
                return;
            };
            self.advance(&steps);
            self.compact();
        }
    }

    /// One bidiagonalization step of every running slot; `None` (every column failed)
    /// when an application failed. A slot whose column failed keeps `None`.
    fn step(&mut self) -> Option<[Option<BidiagStep>; MAX_COLUMNS]> {
        let w = self.width();
        let on = self.mask(Column::running);
        let mut steps = [None; MAX_COLUMNS];
        let mut scale = [0.0; MAX_COLUMNS];
        for (s, &c) in self.slots.iter().enumerate() {
            let column = &self.columns[c];
            scale[s] = -(column.alpha * column.u_norm_inv);
        }
        if let Err(source) = self.operator.apply_columns(w, &self.v, &mut self.av) {
            self.fail_running(source);
            return None;
        }
        let sq = axpy_sq(&mut self.u, &self.av, w, &scale, &on);
        let mut live = on;
        let mut beta = [0.0; MAX_COLUMNS];
        let mut broken = [false; MAX_COLUMNS];
        let mut inverse = [false; MAX_COLUMNS];
        for s in (0..w).filter(|&s| on[s]) {
            let c = self.slots[s];
            match finite(norm_from_sq_column(&self.u, w, s, sq[s]), "β") {
                Err(source) => {
                    live[s] = false;
                    self.fail(c, source);
                }
                Ok(b) if b == 0.0 => {
                    // Lucky breakdown: zero `v` and its paired `p̃` so the update contributes nothing.
                    live[s] = false;
                    broken[s] = true;
                    self.columns[c].alpha = 0.0;
                    steps[s] = Some(BidiagStep {
                        alpha: 0.0,
                        beta: b,
                    });
                }
                Ok(b) => {
                    beta[s] = b;
                    // `Aᵀu` scales with `‖A‖β`; only a `β` far from 1 carries it off `p̃`'s own `‖A‖` scale.
                    if RAW_U_NORMS.contains(&b) {
                        self.columns[c].u_norm_inv = 1.0 / b;
                    } else {
                        inverse[s] = true;
                        self.columns[c].u_norm_inv = 1.0;
                    }
                }
            }
        }
        for s in (0..w).filter(|&s| broken[s]) {
            fill_column(&mut self.p, w, s, 0.0);
        }
        normalize_columns(&mut self.u, w, &beta, &inverse);
        if let Err(source) = self
            .operator
            .apply_adjoint_columns(w, &self.u, &mut self.atu)
        {
            self.fail_running(source);
            return None;
        }
        // Scaling by `β / α_k` cancels the stored `α_k`.
        let mut p_alpha = [0.0; MAX_COLUMNS];
        let mut divide = [false; MAX_COLUMNS];
        let mut p_scale = [0.0; MAX_COLUMNS];
        let mut p_coeff = [0.0; MAX_COLUMNS];
        for s in (0..w).filter(|&s| live[s]) {
            let column = &self.columns[self.slots[s]];
            let mut coefficient = beta[s] / column.alpha;
            if !coefficient.is_normal() {
                // `β/α` can leave the range while `p̃ / α = M v` does not, so divide first.
                divide[s] = true;
                p_alpha[s] = column.alpha;
                coefficient = beta[s];
            }
            p_scale[s] = column.u_norm_inv;
            p_coeff[s] = -coefficient;
        }
        normalize_columns(&mut self.p, w, &p_alpha, &divide);
        axpby_columns(&mut self.p, &self.atu, w, (&p_scale, &p_coeff), &live);
        if let Err(source) = self.preconditioner.apply_columns(w, &self.p, &mut self.v) {
            self.fail_running(source);
            return None;
        }
        for s in (0..w).filter(|&s| broken[s]) {
            fill_column(&mut self.v, w, s, 0.0);
        }
        let vp = dot_sq(&self.v, &self.p, w, &live);
        let mut alpha = [0.0; MAX_COLUMNS];
        let stepped = live;
        for s in (0..w).filter(|&s| stepped[s]) {
            match alpha_of((&self.v, &self.p), w, s, vp[s], &mut self.scratch) {
                Ok(a) => alpha[s] = a,
                Err(source) => {
                    live[s] = false;
                    self.fail(self.slots[s], source);
                }
            }
        }
        normalize_columns(&mut self.v, w, &alpha, &live);
        for s in (0..w).filter(|&s| live[s]) {
            self.columns[self.slots[s]].alpha = alpha[s];
            steps[s] = Some(BidiagStep {
                alpha: alpha[s],
                beta: beta[s],
            });
        }
        Some(steps)
    }

    /// The recurrences and solution updates of the stepped slots, then their stopping
    /// tests and escalation handlers; a stopped column ends its pass.
    fn advance(&mut self, steps: &[Option<BidiagStep>; MAX_COLUMNS]) {
        let w = self.width();
        let mut coefficients = [[0.0; 3]; MAX_COLUMNS];
        let mut rotations = [RotationStep::initial(); MAX_COLUMNS];
        let mut on = [false; MAX_COLUMNS];
        for s in 0..w {
            let (Some(step), Some(pass)) = (steps[s], self.columns[self.slots[s]].pass.as_mut())
            else {
                continue;
            };
            pass.convergence.observe(step);
            let curr = pass.recurrence.step(step);
            coefficients[s] = update_coefficients(pass.rhs_norm, curr, pass.prev);
            rotations[s] = curr;
            on[s] = true;
        }
        update_columns(
            (&mut self.hbar, &mut self.h, &mut self.x),
            &self.v,
            w,
            &coefficients,
            &on,
        );
        let mut stops = Vec::new();
        for s in (0..w).filter(|&s| on[s]) {
            let c = self.slots[s];
            let column = &mut self.columns[c];
            let iterations = column.iterations;
            let pass = column.pass.as_mut().expect("a stepped column has a pass");
            // The tolerance test catches breakdown when the residual recurrences collapse.
            let stop = match pass.convergence.check(&pass.recurrence) {
                Stop::Continue => None,
                Stop::ResidualTolerance => Some(LsmrStopReason::ResidualTolerance),
                Stop::NormalEquationTolerance => Some(LsmrStopReason::NormalEquationTolerance),
            };
            let stop = stop.or_else(|| {
                let rule = pass.handler.as_deref_mut()?;
                let progress = Progress {
                    iteration: iterations,
                    normal_eq_residual: pass.recurrence.relative_normal_eq_residual(),
                };
                rule.should_escalate(progress)
                    .then_some(LsmrStopReason::Escalated)
            });
            match stop {
                Some(reason) => stops.push((c, s, reason)),
                None => pass.prev = rotations[s],
            }
        }
        for (c, s, reason) in stops {
            self.end_pass(c, s, reason);
        }
    }

    /// `lsmr_from_bidiag` after a pass of column `c` (slot `s`) stopped for `stop_reason`:
    /// the iterate, its exit audits on the single stream and either a restart into the
    /// slot or the column's result.
    fn end_pass(&mut self, c: usize, s: usize, stop_reason: LsmrStopReason) {
        let w = self.width();
        let mut x = vec![0.0; self.cols];
        gather(&self.x, w, s, &mut x);
        let column = &mut self.columns[c];
        let pass = column.pass.take().expect("a stopped column has a pass");
        if let Some(base) = &column.base {
            axpby(&mut x, base, 1.0, 1.0);
        }
        let (criteria, reference, iterations) =
            (column.criteria, column.reference, column.iterations);
        // Only tolerance stops measure `x`, and a non-finite entry never recovers.
        if let Some((index, value)) = x.iter().copied().enumerate().find(|(_, v)| !v.is_finite()) {
            return self.fail(
                c,
                invalid_input(format!("non-finite solution entry {index} ({value})")),
            );
        }
        let mut result = LsmrResult {
            x,
            converged: false,
            iterations,
            residual_norm: pass.recurrence.residual_estimate(),
            normal_eq_residual: reference.relative(pass.recurrence.normal_eq_residual_estimate()),
            stop_reason,
            true_residual: None,
        };
        // Only a tolerance stop claims convergence, so only it is worth a true-residual evaluation.
        if matches!(
            stop_reason,
            LsmrStopReason::ResidualTolerance | LsmrStopReason::NormalEquationTolerance
        ) {
            gather(self.b, self.k0, c, &mut self.rhs);
            let residual_norm = match self.single.residual_norm(&result.x, &self.rhs) {
                Ok(norm) => norm,
                Err(source) => return self.fail(c, source),
            };
            result.converged = criteria.corroborates_residual(residual_norm, result.residual_norm);
            if result.converged && stop_reason == LsmrStopReason::NormalEquationTolerance {
                match self.single.plain_gradient(residual_norm) {
                    Err(source) => return self.fail(c, source),
                    Ok(Some(current)) => {
                        result.converged = criteria.corroborates_normal_eq(
                            residual_norm,
                            current,
                            reference.plain,
                            result.normal_eq_residual,
                        );
                    }
                    Ok(None) => {}
                }
            }
            let column = &mut self.columns[c];
            if !result.converged
                && residual_norm.is_finite()
                && column.restarts < MAX_RESTARTS
                && column.iterations < column.spec.maxiter
            {
                column.restarts += 1;
                let step1 = match self.single.restart(residual_norm) {
                    Ok(step) => step,
                    Err(source) => return self.fail(c, source),
                };
                scatter(&self.single.bufs.u, &mut self.u, w, s);
                scatter(&self.single.bufs.v, &mut self.v, w, s);
                scatter(&self.single.bufs.p_tilde, &mut self.p, w, s);
                let column = &mut self.columns[c];
                column.alpha = self.single.alpha;
                column.u_norm_inv = 1.0;
                column.base = Some(Cow::Owned(result.x));
                return self.begin_pass(c, s, step1);
            }
            if !result.converged {
                result.stop_reason = LsmrStopReason::FalseConvergence;
                // The estimate is the refuted claim; a seed from the staged residual measures `x`.
                result.normal_eq_residual = if residual_norm.is_finite() {
                    match self.single.restart(residual_norm) {
                        Ok(step) => reference.relative(Magnitude::product(step.alpha, step.beta)),
                        Err(source) => return self.fail(c, source),
                    }
                } else {
                    residual_norm
                };
            }
            result.residual_norm = residual_norm;
            if result.converged && self.true_residuals {
                result.true_residual = Some(self.single.bufs.u.clone());
            }
        }
        self.done(c, result);
    }

    /// Drop the slots of finished columns, narrowing every carried vector (each
    /// buffer keeps exactly the block's width).
    fn compact(&mut self) {
        let w = self.width();
        let keep: Vec<usize> = (0..w)
            .filter(|&s| self.columns[self.slots[s]].pass.is_some())
            .collect();
        if keep.len() == w {
            return;
        }
        self.slots = keep.iter().map(|&s| self.slots[s]).collect();
        let nw = keep.len();
        if nw == 0 {
            return;
        }
        let (rows, cols) = (self.rows * nw, self.cols * nw);
        narrow(&self.u, &mut self.av, w, &keep);
        std::mem::swap(&mut self.u, &mut self.av);
        self.u.truncate(rows);
        self.av.truncate(rows);
        for buffer in [
            &mut self.v,
            &mut self.p,
            &mut self.x,
            &mut self.h,
            &mut self.hbar,
        ] {
            narrow(buffer, &mut self.spare, w, &keep);
            std::mem::swap(buffer, &mut self.spare);
            buffer.truncate(cols);
        }
        self.atu.truncate(cols);
    }
}

/// `α = √⟨v, p̃⟩` of column `j` from its block dot `vp`, by `alpha_from_vp` on
/// contiguous copies when `vp` is not a positive normal number.
fn alpha_of(
    (v, p): (&[f64], &[f64]),
    w: usize,
    j: usize,
    vp: f64,
    (column_v, column_p): &mut (Vec<f64>, Vec<f64>),
) -> Result<f64, SolveError> {
    if vp.is_normal() && vp > 0.0 {
        return Ok(vp.sqrt());
    }
    gather(v, w, j, column_v);
    gather(p, w, j, column_p);
    alpha_from_vp(column_v, column_p).map_err(SolveError::from)
}

/// `dst` takes the slots `keep` of the `w`-wide rows of `src`, `keep.len()` wide.
fn narrow(src: &[f64], dst: &mut [f64], w: usize, keep: &[usize]) {
    let nw = keep.len();
    let rows = src.len() / w;
    let body = |(dst, src): (&mut [f64], &[f64])| {
        for (d, s) in dst.chunks_exact_mut(nw).zip(src.chunks_exact(w)) {
            for (t, &k) in keep.iter().enumerate() {
                d[t] = s[k];
            }
        }
    };
    let dst = &mut dst[..rows * nw];
    if rows >= LSMR_PAR_THRESHOLD {
        dst.par_chunks_mut(LSMR_UPDATE_CHUNK * nw)
            .zip(src.par_chunks(LSMR_UPDATE_CHUNK * w))
            .for_each(body);
    } else {
        body((dst, src));
    }
}

/// Column `j` of the `w`-wide rows of `block` into `out`.
fn gather(block: &[f64], w: usize, j: usize, out: &mut [f64]) {
    for (o, &v) in out.iter_mut().zip(block.iter().skip(j).step_by(w)) {
        *o = v;
    }
}

/// `values` into column `j` of the `w`-wide rows of `block`.
fn scatter(values: &[f64], block: &mut [f64], w: usize, j: usize) {
    for (b, &v) in block.iter_mut().skip(j).step_by(w).zip(values) {
        *b = v;
    }
}

/// Column `j` of `src` into column `j` of `dst` (both `w` wide).
fn copy_column(dst: &mut [f64], src: &[f64], w: usize, j: usize) {
    for (d, &v) in dst
        .iter_mut()
        .skip(j)
        .step_by(w)
        .zip(src.iter().skip(j).step_by(w))
    {
        *d = v;
    }
}

fn fill_column(block: &mut [f64], w: usize, j: usize, value: f64) {
    for b in block.iter_mut().skip(j).step_by(w) {
        *b = value;
    }
}

/// `vec_norm` of column `j`.
fn vec_norm_column(block: &[f64], w: usize, j: usize) -> f64 {
    let column = || block.iter().skip(j).step_by(w);
    let scale = column().fold(0.0f64, |m, &x| m.max(x.abs()));
    if scale == 0.0 {
        return if column().any(|x| x.is_nan()) {
            f64::NAN
        } else {
            0.0
        };
    }
    scale * column().map(|&x| (x / scale).powi(2)).sum::<f64>().sqrt()
}

/// `norm_from_sq` of column `j`.
fn norm_from_sq_column(block: &[f64], w: usize, j: usize, sq: f64) -> f64 {
    if sq.is_normal() {
        sq.sqrt()
    } else {
        vec_norm_column(block, w, j)
    }
}

fn add_lanes(a: Lanes, b: Lanes) -> Lanes {
    std::array::from_fn(|j| a[j] + b[j])
}

/// Per column of `on`, `y = x + scale · y` returning `Σ y²` in `axpy_with_norm`'s order.
fn axpy_sq(y: &mut [f64], x: &[f64], w: usize, scale: &Lanes, on: &Mask) -> Lanes {
    let leaf = |(y, x): (&mut [f64], &[f64])| {
        let rows = y.len() / w;
        let full = rows - rows % LANES;
        let mut acc = [[0.0; LANES]; MAX_COLUMNS];
        for group in (0..full).step_by(LANES) {
            for (l, row) in (group..group + LANES).enumerate() {
                let i = row * w;
                for j in 0..w {
                    if on[j] {
                        let val = x[i + j] + scale[j] * y[i + j];
                        y[i + j] = val;
                        acc[j][l] += val * val;
                    }
                }
            }
        }
        let mut out = [0.0; MAX_COLUMNS];
        for j in (0..w).filter(|&j| on[j]) {
            let tail: f64 = (full..rows)
                .map(|row| {
                    let i = row * w + j;
                    let val = x[i] + scale[j] * y[i];
                    y[i] = val;
                    val * val
                })
                .sum();
            out[j] = acc[j].iter().sum::<f64>() + tail;
        }
        out
    };
    fold((y, x), LSMR_UPDATE_CHUNK * w, &leaf, &add_lanes)
}

/// Per column of `on`, `⟨a, b⟩` in `par_dot`'s order.
fn dot_sq(a: &[f64], b: &[f64], w: usize, on: &Mask) -> Lanes {
    let leaf = |(a, b): (&[f64], &[f64])| {
        let rows = a.len() / w;
        let full = rows - rows % LANES;
        let mut out = [0.0; MAX_COLUMNS];
        let mut tails = [0.0; MAX_COLUMNS];
        for j in (0..w).filter(|&j| on[j]) {
            tails[j] = (full..rows)
                .map(|row| a[row * w + j] * b[row * w + j])
                .sum();
        }
        let mut acc = [[0.0; LANES]; MAX_COLUMNS];
        for group in (0..full).step_by(LANES) {
            for (l, row) in (group..group + LANES).enumerate() {
                let i = row * w;
                for j in 0..w {
                    if on[j] {
                        acc[j][l] += a[i + j] * b[i + j];
                    }
                }
            }
        }
        for j in (0..w).filter(|&j| on[j]) {
            out[j] = acc[j].iter().sum::<f64>() + tails[j];
        }
        out
    };
    fold((a, b), LSMR_UPDATE_CHUNK * w, &leaf, &add_lanes)
}

/// `body` on consecutive row chunks of the `w`-wide `y` and `x`, in parallel above
/// the threshold (elementwise kernels: the schedule never changes a value).
fn rowwise(
    y: &mut [f64],
    x: Option<&[f64]>,
    w: usize,
    body: &(impl Fn(&mut [f64], Option<&[f64]>) + Sync),
) {
    let rows = y.len() / w;
    let chunk = LSMR_UPDATE_CHUNK * w;
    match (x, rows >= LSMR_PAR_THRESHOLD) {
        (Some(x), true) => y
            .par_chunks_mut(chunk)
            .zip(x.par_chunks(chunk))
            .for_each(|(y, x)| body(y, Some(x))),
        (None, true) => y.par_chunks_mut(chunk).for_each(|y| body(y, None)),
        (x, false) => body(y, x),
    }
}

/// Per column of `on` with `d > 0`, `y /= d` as `normalize` does.
fn normalize_columns(y: &mut [f64], w: usize, d: &Lanes, on: &Mask) {
    if w == 1 {
        if on[0] {
            normalize(y, d[0]);
        }
        return;
    }
    let mut multiply = [None; MAX_COLUMNS];
    let mut any = false;
    for j in (0..w).filter(|&j| on[j] && d[j] > 0.0) {
        let inv = 1.0 / d[j];
        multiply[j] = Some(inv.is_normal().then_some(inv));
        any = true;
    }
    if !any {
        return;
    }
    rowwise(y, None, w, &|y, _| {
        for row in y.chunks_exact_mut(w) {
            for (j, value) in row.iter_mut().enumerate() {
                match multiply[j] {
                    Some(Some(inv)) => *value *= inv,
                    Some(None) => *value /= d[j],
                    None => {}
                }
            }
        }
    });
}

/// Per column of `on`, `y = alpha x + beta y`.
fn axpby_columns(y: &mut [f64], x: &[f64], w: usize, (alpha, beta): (&Lanes, &Lanes), on: &Mask) {
    if !on.contains(&true) {
        return;
    }
    rowwise(y, Some(x), w, &|y, x| {
        let x = x.expect("an input");
        for (y, x) in y.chunks_exact_mut(w).zip(x.chunks_exact(w)) {
            for j in (0..w).filter(|&j| on[j]) {
                y[j] = alpha[j] * x[j] + beta[j] * y[j];
            }
        }
    });
}

/// Per column of `on`, the solution recurrence `h̄ = h − t_h̄ h̄`, `x += t_x h̄`,
/// `h = v − t_h h` with `t = (t_x, t_h̄, t_h)`.
fn update_columns(
    (hbar, h, x): (&mut [f64], &mut [f64], &mut [f64]),
    v: &[f64],
    w: usize,
    t: &[[f64; 3]; MAX_COLUMNS],
    on: &Mask,
) {
    if !on.contains(&true) {
        return;
    }
    let body = |hbar: &mut [f64], h: &mut [f64], x: &mut [f64], v: &[f64]| {
        for (((hb, h), x), v) in hbar
            .chunks_exact_mut(w)
            .zip(h.chunks_exact_mut(w))
            .zip(x.chunks_exact_mut(w))
            .zip(v.chunks_exact(w))
        {
            for j in (0..w).filter(|&j| on[j]) {
                let [t_x, t_hbar, t_h] = t[j];
                let h_old = h[j];
                let value = h_old - t_hbar * hb[j];
                hb[j] = value;
                x[j] += t_x * value;
                h[j] = v[j] - t_h * h_old;
            }
        }
    };
    let rows = v.len() / w;
    if rows >= LSMR_PAR_THRESHOLD {
        let chunk = LSMR_UPDATE_CHUNK * w;
        hbar.par_chunks_mut(chunk)
            .zip(h.par_chunks_mut(chunk))
            .zip(x.par_chunks_mut(chunk))
            .zip(v.par_chunks(chunk))
            .for_each(|(((hb, h), x), v)| body(hb, h, x, v));
    } else {
        body(hbar, h, x, v);
    }
}
