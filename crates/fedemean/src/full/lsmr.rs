//! Modified LSMR on `A = W^1/2 D` with right-hand side `W^1/2 y`
//! (schwarz-precond's `mlsmr`), one column at a time.
//!
//! `mlsmr` needs `M ~ A'A = D'WD` as a nonsingular SPD action `M^-1`; see
//! the module contract in [`super`]. Its stopping tests (the residual and
//! normal-equation estimates and their exit audits) are proxies: each pass
//! is followed by the moment certificate on the formed residual over all
//! levels. A pass that does not certify restarts from its iterate (the true
//! residual is recomputed from the columns) at a tolerance tightened by the
//! certificate's shortfall, within the column's iteration budget. A column
//! is written only once it certifies.

use super::kernels::{adjoint, forward, load, residualize, Rows};
use super::structure::Structure;
use super::{
    add_coefficients, certify, filled, form_gram, numerical, refill, report, reserve,
    same_projection, targets, Action, Job, Outputs,
};
use crate::certificate::MomentFold;
use crate::design::Effect;
use crate::elimination::pcg::Targets;
use crate::elimination::STOP_FRACTION;
use crate::error::{Error, Result};
use crate::projector::{
    CoefficientsMut, ColumnReport, ColumnStatus, ColumnsMut, Facts, InputState, Outcome,
};
use rayon::prelude::*;
use schwarz_precond::{
    mlsmr_counted_cancellable, Cancellation, MlsmrOptions, Operator, SolveError,
};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

/// Passes per column: the first and its certified restarts.
const MAX_PASSES: usize = 4;

/// `W^1/2 D` and its adjoint `D' W^1/2`.
struct Design<'a, 'd> {
    st: &'a Structure,
    effects: &'a [Effect<'d>],
    roots: Option<&'a [f64]>,
    /// The scatter accumulators of the adjoint (applies run one at a time).
    scatter: Mutex<Vec<f64>>,
}

impl Operator for Design<'_, '_> {
    fn nrows(&self) -> usize {
        self.st.nobs
    }

    fn ncols(&self) -> usize {
        self.st.coefficients()
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        forward::<1>(self.st, self.effects, x, self.roots, y);
        Ok(())
    }

    fn apply_adjoint(&self, u: &[f64], v: &mut [f64]) -> std::result::Result<(), SolveError> {
        let mut scatter = self.scatter.lock().unwrap_or_else(PoisonError::into_inner);
        let rows = Rows {
            data: u,
            stride: 1,
            first: 0,
        };
        adjoint::<1>(self.st, self.effects, rows, self.roots, v, &mut scatter);
        Ok(())
    }
}

/// `M^-1`: Jacobi `diag(D'WD)^-1` (the inverse level masses) or the
/// identity; both symmetric positive definite on all of coefficient space.
struct Inverse<'a> {
    inverse_mass: Option<&'a [f64]>,
    size: usize,
}

impl Operator for Inverse<'_> {
    fn nrows(&self) -> usize {
        self.size
    }

    fn ncols(&self) -> usize {
        self.size
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        match self.inverse_mass {
            Some(inverse) => {
                for ((y, x), d) in y.iter_mut().zip(x).zip(inverse) {
                    *y = x * d;
                }
            }
            None => y.copy_from_slice(x),
        }
        Ok(())
    }

    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        self.apply(x, y)
    }
}

/// One column's certified solve.
struct Solved {
    x: Vec<f64>,
    /// The original column's weighted norm (its reference scale).
    reference: f64,
    report: ColumnReport,
    fold: MomentFold,
}

/// Why a column's solve stopped short of a certificate.
enum Short {
    Declined(&'static str),
    Failed(Error),
}

/// Modified LSMR over the columns one at a time; see the module docs. Also
/// reports whether the residual Gram was formed.
pub(super) fn project(
    job: &Job<'_, '_>,
    values: &mut ColumnsMut<'_>,
    start: Option<&[f64]>,
    mut coefficients: Option<CoefficientsMut<'_>>,
    outputs: Outputs<'_>,
    mut facts: Facts,
) -> Result<(Outcome, bool)> {
    let st = job.st;
    let ncols = values.ncols();
    let Outputs {
        work,
        gram,
        norms,
        columns,
        reference: formed,
    } = outputs;
    let started = Instant::now();
    work.size(st, 1)?;
    if work.b.len() != st.nobs {
        refill(&mut work.b, st.nobs, 0.0)?;
    }
    for buffer in [&mut *norms, &mut *formed] {
        buffer.clear();
        reserve(buffer, ncols)?;
    }
    columns.clear();
    reserve(columns, ncols)?;
    formed.resize(ncols, f64::NAN);
    facts.columns = ncols;
    let m = st.coefficients();
    let mut fold = MomentFold::default();
    for j in 0..ncols {
        let warm = start.map(|start| &start[j * m..(j + 1) * m]);
        let solved = match solve_column(job, values, j, warm, work, &mut facts) {
            Ok(solved) => solved,
            Err(Short::Failed(error)) => return Err(error),
            Err(Short::Declined(reason)) => {
                let input = if j > 0 {
                    same_projection(formed)?
                } else {
                    InputState::Untouched
                };
                return Ok((numerical(reason, input, facts, job), false));
            }
        };
        let began = Instant::now();
        let target = (values.as_mut_slice(), ncols, j);
        let weights = job.state.weights();
        residualize::<1>(
            st,
            job.design.effects(),
            &solved.x,
            target,
            weights,
            &mut work.t,
            &mut work.rows,
        );
        if let Some(coefficients) = coefficients.as_mut() {
            add_coefficients(st, &solved.x, (1, 0), coefficients, j);
        }
        facts.residual_seconds += began.elapsed().as_secs_f64();
        formed[j] = solved.reference;
        fold.merge(&solved.fold);
        norms.push(solved.report.residual_norm);
        columns.push(solved.report);
    }
    let certificate = fold.finish(job.request.tolerance)?;
    if !certificate.passed {
        let input = same_projection(formed)?;
        return Ok((
            numerical("combined certificate failed", input, facts, job),
            false,
        ));
    }
    let gram_formed = form_gram(job, values, gram, work)?;
    Ok((
        Outcome::Projected(report(job, certificate, facts, started)),
        gram_formed,
    ))
}

/// The certificate of `x` on column `j` (without writing the column):
/// `t = W (y - D x)`, its moments over all levels in `work.q`, the fold at
/// `max(scale, ||y - D x||)` and the stopping measures `max |m_g| /
/// sqrt(mass_g)` and `max |m_g| / mass_g`.
fn check(
    job: &Job<'_, '_>,
    operator: &Design<'_, '_>,
    (src, x, scale): (Rows<'_>, &[f64], f64),
    work: &mut super::Work,
) -> (super::Certified<1>, f64, f64) {
    let (st, state) = (job.st, job.state);
    let effects = job.design.effects();
    let weights = state.weights();
    let stats = load::<1>(
        st,
        effects,
        src,
        Some(x),
        weights,
        &mut work.t,
        &mut work.rows,
    );
    let t = Rows {
        data: &work.t,
        stride: 1,
        first: 0,
    };
    let mut scatter = operator
        .scatter
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    adjoint::<1>(st, effects, t, None, &mut work.q, &mut scatter);
    drop(scatter);
    let tolerance = job.request.tolerance;
    let certified = certify::<1>(state, &work.q, [stats[2][0]], [scale], tolerance);
    let (mut inf, mut zinf) = (0.0_f64, 0.0_f64);
    for ((m, ir), im) in work
        .q
        .iter()
        .zip(&state.inverse_root_mass)
        .zip(&state.inverse_mass)
    {
        inf = inf.max(m.abs() * ir);
        zinf = zinf.max(m.abs() * im);
    }
    (certified, inf, zinf)
}

/// Solve and certify column `j` from `warm` or cold, as PCG does: from the
/// largest effect's level means, with the goal at a tenth of the tolerance
/// on the starting residual's norm.
fn solve_column(
    job: &Job<'_, '_>,
    values: &ColumnsMut<'_>,
    j: usize,
    warm: Option<&[f64]>,
    work: &mut super::Work,
    facts: &mut Facts,
) -> std::result::Result<Solved, Short> {
    let (st, state) = (job.st, job.state);
    let effects = job.design.effects();
    let started = Instant::now();
    let src = Rows {
        data: values.as_slice(),
        stride: values.ncols(),
        first: j,
    };
    let weights = state.weights();
    load::<1>(st, effects, src, None, weights, &mut work.t, &mut work.rows);
    let mut start = filled(st.coefficients(), 0.0).map_err(Short::Failed)?;
    match warm {
        Some(warm) => start.copy_from_slice(warm),
        None => {
            let t = Rows {
                data: &work.t,
                stride: 1,
                first: 0,
            };
            adjoint::<1>(st, effects, t, None, &mut work.q, &mut work.scatter);
            let levels = st.offsets[st.largest]..st.offsets[st.largest + 1];
            let inverse = &state.inverse_mass[levels.clone()];
            for ((x, q), d) in start[levels.clone()]
                .iter_mut()
                .zip(&work.q[levels])
                .zip(inverse)
            {
                *x = q * d;
            }
        }
    }
    let stats = load::<1>(
        st,
        effects,
        src,
        Some(&start),
        weights,
        &mut work.t,
        &mut work.rows,
    );
    let Some((scale, targets)) = targets::<1>(job, &stats, None) else {
        return Err(Short::Declined("RHS scale outside the plain-sum range"));
    };
    let roots = state.roots();
    work.b.par_iter_mut().enumerate().for_each(|(i, b)| {
        let y = src.data[i * src.stride + j];
        *b = roots.map_or(y, |roots| roots[i] * y);
    });
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let scatter = std::mem::take(&mut work.scatter);
    let operator = Design {
        st,
        effects,
        roots,
        scatter: Mutex::new(scatter),
    };
    let inverse = Inverse {
        inverse_mass: (job.action == Action::Jacobi).then_some(&state.inverse_mass[..]),
        size: st.coefficients(),
    };
    let column = (src, scale[0], &targets);
    let solved = passes(job, (&operator, &inverse), column, start, work, facts);
    work.scatter = operator
        .scatter
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner);
    solved
}

/// The passes of one column (see the module docs): a pass that meets the
/// goal and the certificate succeeds; one that misses the goal restarts
/// from its iterate at a tolerance tightened by the shortfall; the last
/// pass (or an exhausted budget) also succeeds at the acceptance rule.
fn passes(
    job: &Job<'_, '_>,
    (operator, inverse): (&Design<'_, '_>, &Inverse<'_>),
    (src, scale, targets): (Rows<'_>, f64, &Targets<1>),
    mut start: Vec<f64>,
    work: &mut super::Work,
    facts: &mut Facts,
) -> std::result::Result<Solved, Short> {
    let poll = || job.cancel.0.is_some_and(|cancel| cancel());
    let cancellation = match job.cancel.0 {
        Some(_) => Cancellation::new(&poll),
        None => Cancellation::default(),
    };
    let mut tolerance = STOP_FRACTION * job.request.tolerance;
    let mut budget = job.cap;
    let mut iterations = 0;
    for pass in 0..MAX_PASSES {
        let started = Instant::now();
        let options = MlsmrOptions {
            warm_start: Some(&start),
            escalation: None,
            local_size: None,
        };
        let b = &work.b[..];
        let result = mlsmr_counted_cancellable(
            operator,
            b,
            inverse,
            tolerance,
            budget,
            options,
            cancellation,
        );
        facts.pcg_seconds += started.elapsed().as_secs_f64();
        let result = match result {
            Ok(result) => result,
            Err(failure) => {
                facts.iterations = facts.iterations.max(iterations + failure.iterations);
                return Err(match failure.source {
                    SolveError::Cancelled => Short::Failed(Error::Cancelled),
                    SolveError::Allocation { .. } => Short::Failed(Error::OutOfMemory(
                        "unable to reserve the modified LSMR workspace",
                    )),
                    _ => Short::Declined("modified LSMR broke down"),
                });
            }
        };
        iterations += result.iterations;
        budget = budget.saturating_sub(result.iterations);
        facts.iterations = facts.iterations.max(iterations);
        let started = Instant::now();
        let (certified, inf, zinf) = check(job, operator, (src, &result.x, scale), work);
        facts.residual_seconds += started.elapsed().as_secs_f64();
        let measure = targets.goal(inf, zinf, 0);
        let last = budget == 0 || pass + 1 == MAX_PASSES;
        let status = if measure <= 1.0 {
            Some(ColumnStatus::Converged)
        } else if last && targets.accepted(inf, zinf, 0) {
            Some(ColumnStatus::AcceptedAtFloor)
        } else {
            None
        };
        if let (Some(fold), Some(status)) = (certified.fold, status) {
            facts.column_iterations += iterations;
            facts.accepted += usize::from(status == ColumnStatus::AcceptedAtFloor);
            return Ok(Solved {
                x: result.x,
                reference: scale,
                report: ColumnReport {
                    relative_moment: certified.relative[0],
                    scale: certified.scales[0],
                    residual_norm: certified.norms[0],
                    iterations,
                    status,
                },
                fold,
            });
        }
        if last {
            break;
        }
        // Tighten by the goal's shortfall, at least tenfold.
        let factor = if measure.is_finite() && measure > 1.0 {
            (0.5 / measure).clamp(1e-4, 0.1)
        } else {
            0.1
        };
        tolerance = (tolerance * factor).max(f64::EPSILON);
        start = result.x;
    }
    Err(Short::Declined(
        "modified LSMR did not certify within its budget",
    ))
}
