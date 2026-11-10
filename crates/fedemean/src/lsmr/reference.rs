//! The standard (unpreconditioned) Golub-Kahan LSMR entry points of the
//! upstream engine, kept verbatim as a test reference: the tests run them
//! against the modified recurrence. fedemean's code calls only the modified
//! entry points.
//
// Copyright (c) 2026 within contributors
// Copyright (c) 2026 appliedAI Institute for Europe gGmbH
// MIT License; see the notice in `crates/fedemean/src/lsmr.rs`.

use super::bidiag::GolubKahan;
use super::magnitude::Magnitude;
use super::recurrence::ConvergenceCriteria;
use super::{
    finite, lsmr_from_bidiag, validate_lsmr_inputs, vec_norm, Cancellation, LsmrFailure,
    LsmrResult, LsmrStopReason, NormalEqReference, Operator, RunControl, SolveError, SolveStart,
};

/// Unpreconditioned LSMR.
///
/// Solves `min ‖b − A x‖₂` using the standard Golub-Kahan
/// bidiagonalization.
pub fn lsmr<A: Operator + ?Sized>(
    operator: &A,
    b: &[f64],
    tol: f64,
    maxiter: usize,
    local_size: Option<usize>,
) -> Result<LsmrResult, SolveError> {
    lsmr_counted(operator, b, tol, maxiter, local_size).map_err(|failure| failure.source)
}

/// Unpreconditioned LSMR with cooperative caller cancellation.
///
/// No partial coefficients are returned after cancellation. Checks occur before
/// setup, once per iteration, and after final validation; see [`Cancellation`].
pub fn lsmr_cancellable<A: Operator + ?Sized>(
    operator: &A,
    b: &[f64],
    tol: f64,
    maxiter: usize,
    local_size: Option<usize>,
    cancellation: Cancellation<'_>,
) -> Result<LsmrResult, SolveError> {
    lsmr_counted_cancellable(operator, b, tol, maxiter, local_size, cancellation)
        .map_err(|failure| failure.source)
}

/// Unpreconditioned [`lsmr`] with attempted-iteration accounting on errors.
pub fn lsmr_counted<A: Operator + ?Sized>(
    operator: &A,
    b: &[f64],
    tol: f64,
    maxiter: usize,
    local_size: Option<usize>,
) -> Result<LsmrResult, LsmrFailure> {
    lsmr_counted_cancellable(
        operator,
        b,
        tol,
        maxiter,
        local_size,
        Cancellation::default(),
    )
}

/// [`lsmr_cancellable`] with attempted-iteration accounting, including cancellation.
pub fn lsmr_counted_cancellable<A: Operator + ?Sized>(
    operator: &A,
    b: &[f64],
    tol: f64,
    maxiter: usize,
    local_size: Option<usize>,
    cancellation: Cancellation<'_>,
) -> Result<LsmrResult, LsmrFailure> {
    let mut iterations = 0;
    let result = (|| {
        cancellation.checkpoint()?;
        let result = lsmr_impl(
            operator,
            b,
            tol,
            maxiter,
            local_size,
            RunControl {
                attempted: &mut iterations,
                cancellation,
            },
        );
        cancellation.checkpoint()?;
        result
    })();
    result.map_err(|source| LsmrFailure { source, iterations })
}

fn lsmr_impl<A: Operator + ?Sized>(
    operator: &A,
    b: &[f64],
    tol: f64,
    maxiter: usize,
    local_size: Option<usize>,
    control: RunControl<'_>,
) -> Result<LsmrResult, SolveError> {
    let RunControl {
        attempted,
        cancellation,
    } = control;
    validate_lsmr_inputs(operator, b, tol)?;
    let n = operator.ncols();

    let b_norm = finite(vec_norm(b), "rhs norm")?;
    if b_norm == 0.0 {
        return Ok(LsmrResult {
            x: vec![0.0; n],
            converged: true,
            iterations: 0,
            residual_norm: 0.0,
            normal_eq_residual: 0.0,
            stop_reason: LsmrStopReason::ZeroRhs,
            true_residual: None,
        });
    }

    let local_size = local_size.unwrap_or(0);
    let (bidiag, step1) = GolubKahan::init(operator, b, b_norm, local_size)?;
    let criteria = ConvergenceCriteria::new(b_norm, tol);
    let start = SolveStart {
        cancellation,
        x0: None,
        reference: NormalEqReference::cold(step1, Magnitude::product(step1.alpha, step1.beta)),
        attempted,
    };
    lsmr_from_bidiag(bidiag, step1, b, start, criteria, maxiter, None)
}
