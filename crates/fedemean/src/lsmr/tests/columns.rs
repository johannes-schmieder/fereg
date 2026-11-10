//! `mlsmr_columns` against `mlsmr` column by column: the same bits for every column.
//
// Copyright (c) 2026 within contributors
// Copyright (c) 2026 appliedAI Institute for Europe gGmbH
// MIT License; see the notice in `crates/fedemean/src/lsmr.rs`.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::super::*;
use crate::lsmr::fixtures::*;
use crate::lsmr::fold::assert_pool_independent;
use crate::lsmr::{Cancellation, Operator, SolveError};

/// `A x` with `y[i] = x[i mod n] + ½ x[(7i + 3) mod n]`, large enough that every reduction splits.
struct Wrapped {
    m: usize,
    n: usize,
}

impl Wrapped {
    fn cols(&self, i: usize) -> (usize, usize) {
        (i % self.n, (7 * i + 3) % self.n)
    }
}

impl Operator for Wrapped {
    fn nrows(&self) -> usize {
        self.m
    }
    fn ncols(&self) -> usize {
        self.n
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        for (i, yi) in y.iter_mut().enumerate() {
            let (a, b) = self.cols(i);
            *yi = x[a] + 0.5 * x[b];
        }
        Ok(())
    }
    fn apply_adjoint(&self, u: &[f64], x: &mut [f64]) -> Result<(), SolveError> {
        x.fill(0.0);
        for (i, &ui) in u.iter().enumerate() {
            let (a, b) = self.cols(i);
            x[a] += ui;
            x[b] += 0.5 * ui;
        }
        Ok(())
    }
}

/// [`Wrapped`] with native interleaved column applications (each column's bits as `apply`).
struct Interleaved(Wrapped);

impl Operator for Interleaved {
    fn nrows(&self) -> usize {
        self.0.m
    }
    fn ncols(&self) -> usize {
        self.0.n
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        self.0.apply(x, y)
    }
    fn apply_adjoint(&self, u: &[f64], x: &mut [f64]) -> Result<(), SolveError> {
        self.0.apply_adjoint(u, x)
    }
    fn apply_columns(&self, k: usize, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        for (i, row) in y.chunks_exact_mut(k).enumerate() {
            let (a, b) = self.0.cols(i);
            for (j, yi) in row.iter_mut().enumerate() {
                *yi = x[a * k + j] + 0.5 * x[b * k + j];
            }
        }
        Ok(())
    }
    fn apply_adjoint_columns(&self, k: usize, u: &[f64], x: &mut [f64]) -> Result<(), SolveError> {
        x.fill(0.0);
        for (i, row) in u.chunks_exact(k).enumerate() {
            let (a, b) = self.0.cols(i);
            for (j, &ui) in row.iter().enumerate() {
                x[a * k + j] += ui;
                x[b * k + j] += 0.5 * ui;
            }
        }
        Ok(())
    }
}

/// Deterministic values in `[-scale/2, scale/2)`, a different sequence per `seed`.
fn pattern(len: usize, seed: u64, scale: f64) -> Vec<f64> {
    (0..len as u64)
        .map(|i| {
            let h = (i.wrapping_add(seed).wrapping_mul(2_654_435_761)) % 1_000;
            h as f64 * scale / 1_000.0 - 0.5 * scale
        })
        .collect()
}

fn interleave(columns: &[Vec<f64>]) -> Vec<f64> {
    let rows = columns.first().map_or(0, Vec::len);
    let mut out = vec![0.0; rows * columns.len()];
    for (j, column) in columns.iter().enumerate() {
        for (i, &v) in column.iter().enumerate() {
            out[i * columns.len() + j] = v;
        }
    }
    out
}

type Fingerprint = Result<
    (
        Vec<u64>,
        bool,
        usize,
        u64,
        u64,
        LsmrStopReason,
        Option<Vec<u64>>,
    ),
    (String, usize),
>;

/// Everything a caller can read off a result, as bits.
fn fingerprint(result: &Result<LsmrResult, LsmrFailure>) -> Fingerprint {
    match result {
        Ok(r) => Ok((
            r.x.iter().map(|v| v.to_bits()).collect(),
            r.converged,
            r.iterations,
            r.residual_norm.to_bits(),
            r.normal_eq_residual.to_bits(),
            r.stop_reason,
            r.true_residual
                .as_ref()
                .map(|t| t.iter().map(|v| v.to_bits()).collect()),
        )),
        Err(failure) => Err((format!("{}", failure.source), failure.iterations)),
    }
}

/// `mlsmr` per column, with the true residual kept only where `keep`.
fn one_by_one<A: Operator + ?Sized, M: Operator + ?Sized>(
    op: &A,
    rhs: &[Vec<f64>],
    m: &M,
    specs: &[ColumnSolve<'_>],
    options: MlsmrColumnsOptions,
) -> Vec<Fingerprint> {
    rhs.iter()
        .zip(specs)
        .map(|(b, spec)| {
            let result = mlsmr_counted_cancellable(
                op,
                b,
                m,
                spec.tol,
                spec.maxiter,
                MlsmrOptions {
                    warm_start: spec.warm_start,
                    escalation: spec.escalation,
                    local_size: options.local_size,
                },
                Cancellation::default(),
            )
            .map(|mut r| {
                if !options.true_residuals {
                    r.true_residual = None;
                }
                r
            });
            fingerprint(&result)
        })
        .collect()
}

fn lockstep<A: Operator + ?Sized, M: Operator + ?Sized>(
    op: &A,
    rhs: &[Vec<f64>],
    m: &M,
    specs: &[ColumnSolve<'_>],
    options: MlsmrColumnsOptions,
) -> Vec<Fingerprint> {
    let block = interleave(rhs);
    mlsmr_columns(op, &block, m, specs, options, Cancellation::default())
        .iter()
        .map(fingerprint)
        .collect()
}

fn assert_columns_match(expected: &[Fingerprint], got: &[Fingerprint]) {
    assert_eq!(expected.len(), got.len());
    for (j, (e, g)) in expected.iter().zip(got).enumerate() {
        // Not `assert_eq!`: a printed solution vector would bury the column.
        assert!(
            e == g,
            "column {j} differs: {:?} vs {:?}",
            summary(e),
            summary(g)
        );
    }
}

fn summary(f: &Fingerprint) -> String {
    match f {
        Ok((_, converged, iterations, residual, normal_eq, stop, _)) => format!(
            "{stop:?} converged={converged} iterations={iterations} residual={} normal_eq={}",
            f64::from_bits(*residual),
            f64::from_bits(*normal_eq)
        ),
        Err((message, iterations)) => format!("error {message} after {iterations}"),
    }
}

fn wrapped() -> Wrapped {
    Wrapped {
        m: 300_000,
        n: 80_000,
    }
}

/// Entries over three decades, so `⟨v, M⁻¹p̃⟩` is not a rescaled norm.
fn spread_preconditioner(n: usize) -> DiagOp {
    DiagOp(
        pattern(n, 7, 1.0)
            .iter()
            .map(|v| 10f64.powf(3.0 * (v + 0.5)))
            .collect(),
    )
}

/// Columns differing in scale, start, tolerance, budget and escalation, at every width.
#[test]
fn mixed_columns_match_mlsmr_at_every_width() {
    mixed_columns_match_mlsmr_at_every_width_case(1, false);
    mixed_columns_match_mlsmr_at_every_width_case(1, true);
    mixed_columns_match_mlsmr_at_every_width_case(2, false);
    mixed_columns_match_mlsmr_at_every_width_case(2, true);
    mixed_columns_match_mlsmr_at_every_width_case(3, false);
    mixed_columns_match_mlsmr_at_every_width_case(3, true);
    mixed_columns_match_mlsmr_at_every_width_case(5, false);
    mixed_columns_match_mlsmr_at_every_width_case(5, true);
    mixed_columns_match_mlsmr_at_every_width_case(8, false);
    mixed_columns_match_mlsmr_at_every_width_case(8, true);
}

fn mixed_columns_match_mlsmr_at_every_width_case(width: usize, residuals: bool) {
    let op = wrapped();
    let m = spread_preconditioner(op.n);
    let rhs: Vec<Vec<f64>> = (0..width)
        .map(|j| pattern(op.m, 11 * j as u64 + 1, 10f64.powi(j as i32 - 3)))
        .collect();
    let warm: Vec<Vec<f64>> = (0..width)
        .map(|j| pattern(op.n, 5 * j as u64 + 2, 1e-3))
        .collect();
    let escalate = FixedIterations(9);
    let specs: Vec<ColumnSolve<'_>> = (0..width)
        .map(|j| ColumnSolve {
            tol: [1e-14, 1e-6, 1e-3, 1e-10][j % 4],
            maxiter: [40, 7, 60, 0, 25][j % 5],
            warm_start: (j % 3 == 1).then_some(&warm[j][..]),
            escalation: (j % 4 == 2).then_some(&escalate as &dyn EscalationPolicy),
        })
        .collect();
    let options = MlsmrColumnsOptions {
        true_residuals: residuals,
        ..Default::default()
    };
    let expected = one_by_one(&op, &rhs, &m, &specs, options);
    assert_columns_match(&expected, &lockstep(&op, &rhs, &m, &specs, options));
    // Native interleaved applications give the same bits as the default.
    let native = Interleaved(wrapped());
    assert_columns_match(&expected, &lockstep(&native, &rhs, &m, &specs, options));
}

/// An ill-conditioned problem whose tight stops the exit audit refutes (restarts and
/// refused stops), beside a looser column that certifies.
#[test]
fn audited_and_restarted_columns_match_mlsmr() {
    audited_and_restarted_columns_match_mlsmr_case(false);
    audited_and_restarted_columns_match_mlsmr_case(true);
}

fn audited_and_restarted_columns_match_mlsmr_case(jacobi_m: bool) {
    let (op, b) = vandermonde_ls();
    let m: Box<dyn Operator> = if jacobi_m {
        Box::new(jacobi(&op))
    } else {
        Box::new(IdentityOp { n: op.cols })
    };
    let scaled: Vec<f64> = b.iter().map(|v| 3e5 * v).collect();
    let rhs = vec![b.clone(), scaled, b.clone(), b];
    let x0 = vec![0.25; op.cols];
    let specs = [1e-15, 1e-12, 1e-6, 0.0].map(|tol| ColumnSolve {
        tol,
        maxiter: 400,
        warm_start: None,
        escalation: None,
    });
    let mut specs = specs.to_vec();
    specs[2].warm_start = Some(&x0);
    let options = MlsmrColumnsOptions {
        true_residuals: true,
        ..Default::default()
    };
    let expected = one_by_one(&op, &rhs, m.as_ref(), &specs, options);
    assert_columns_match(&expected, &lockstep(&op, &rhs, m.as_ref(), &specs, options));
}

/// `M⁻¹ x` with `y_i = d_i (x_i + s x_{i+1})`: not symmetric, so the recurrence's
/// estimates drift from the true residual and the exit audit refutes stops; with a
/// negative `d_i`, an indefinite metric.
struct Skewed {
    d: Vec<f64>,
    s: f64,
}

impl Operator for Skewed {
    fn nrows(&self) -> usize {
        self.d.len()
    }
    fn ncols(&self) -> usize {
        self.d.len()
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        let n = x.len();
        for i in 0..n {
            let next = if i + 1 < n { x[i + 1] } else { 0.0 };
            y[i] = self.d[i] * (x[i] + self.s * next);
        }
        Ok(())
    }
    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        self.apply(x, y)
    }
}

/// Counts the passes (handlers) of all columns, and never escalates.
struct Passes(AtomicUsize);

impl EscalationPolicy for Passes {
    fn handler(&self) -> Box<dyn EscalationHandler> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Box::new(FixedIterations(usize::MAX))
    }
}

/// Refuted stops restart (twice, then the stop is refused) in lockstep with columns an
/// indefinite metric fails, and certified stops beside them: `mlsmr`'s bits, iteration
/// counts and passes.
#[test]
fn refuted_stops_and_failures_match_mlsmr() {
    refuted_stops_and_failures_match_mlsmr_case(0.0, 1.0); // certified
    refuted_stops_and_failures_match_mlsmr_case(0.45, 1.0); // stalled
    refuted_stops_and_failures_match_mlsmr_case(0.45, 0.2); // indefinite
    refuted_stops_and_failures_match_mlsmr_case(0.9, 0.2); // refuted
}

fn refuted_stops_and_failures_match_mlsmr_case(skew: f64, shift: f64) {
    let op = Wrapped { m: 3_000, n: 900 };
    let m = Skewed {
        d: pattern(op.n, 3, 1.0).iter().map(|v| shift + v).collect(),
        s: skew,
    };
    let rhs: Vec<Vec<f64>> = (0..6).map(|j| pattern(op.m, 13 * j + 5, 1.0)).collect();
    let warm = pattern(op.n, 17, 0.1);
    let shape = [
        (1e-12, 300, false),
        (1e-9, 300, true),
        (1e-14, 300, false),
        (1e-6, 40, true),
        (1e-12, 12, false),
        (1e-10, 300, true),
    ];
    fn with<'a>(
        shape: &[(f64, usize, bool)],
        warm: &'a [f64],
        policy: &'a Passes,
    ) -> Vec<ColumnSolve<'a>> {
        shape
            .iter()
            .map(|&(tol, maxiter, warm_start)| ColumnSolve {
                tol,
                maxiter,
                warm_start: warm_start.then_some(warm),
                escalation: Some(policy),
            })
            .collect()
    }
    let columns = |policy| with(&shape, &warm, policy);
    let (counted, batched) = (Passes(AtomicUsize::new(0)), Passes(AtomicUsize::new(0)));
    let options = MlsmrColumnsOptions {
        true_residuals: true,
        ..Default::default()
    };
    let expected = one_by_one(&op, &rhs, &m, &columns(&counted), options);
    assert_columns_match(
        &expected,
        &lockstep(&op, &rhs, &m, &columns(&batched), options),
    );
    let passes = counted.0.load(Ordering::Relaxed);
    assert_eq!(passes, batched.0.load(Ordering::Relaxed));
    if skew == 0.9 {
        assert_eq!(passes, shape.len() + MAX_RESTARTS);
        assert!(matches!(
            expected[0],
            Ok((_, false, _, _, _, LsmrStopReason::FalseConvergence, _))
        ));
    }
    if shift < 1.0 {
        assert!(expected[1..].iter().all(Result::is_err));
    }
}

/// Zero right-hand sides, an exact warm start, a mid-stream `β = 0` and an `α₁ = 0`
/// early exit, in one block.
#[test]
fn breakdowns_and_early_exits_match_mlsmr() {
    let identity = IdentityOp { n: 2 };
    let rhs = vec![
        vec![5.0, 0.0],
        vec![0.0, 0.0],
        vec![5.0, 0.0],
        vec![0.0, 3.0],
        vec![2.0, -1.0],
    ];
    let exact = [5.0, 0.0];
    let near = [4.0, 0.0];
    let specs: Vec<ColumnSolve<'_>> = [None, None, Some(&exact[..]), None, Some(&near[..])]
        .into_iter()
        .map(|warm_start| ColumnSolve {
            tol: 1e-12,
            maxiter: 100,
            warm_start,
            escalation: None,
        })
        .collect();
    let options = MlsmrColumnsOptions {
        true_residuals: true,
        ..Default::default()
    };
    let expected = one_by_one(&ZeroSecondRow, &rhs, &identity, &specs, options);
    assert_columns_match(
        &expected,
        &lockstep(&ZeroSecondRow, &rhs, &identity, &specs, options),
    );
}

/// A column's invalid input fails that column alone; the others keep `mlsmr`'s bits.
#[test]
fn invalid_columns_fail_alone() {
    let op = OverdeterminedOp;
    let identity = IdentityOp { n: 3 };
    let rhs = vec![
        vec![1.0, 2.0, 3.0, 4.0],
        vec![1.0, f64::NAN, 3.0, 4.0],
        vec![1.0, 2.0, 3.0, 5.0],
        vec![0.5, 2.0, 3.0, 4.0],
        vec![1.0, 2.0, 3.0, 4.0],
    ];
    let short = [1.0, 2.0];
    let infinite = [1.0, f64::INFINITY, 0.0];
    let mut specs = vec![
        ColumnSolve {
            tol: 1e-12,
            maxiter: 50,
            warm_start: None,
            escalation: None,
        };
        5
    ];
    specs[2].tol = -1.0;
    specs[3].warm_start = Some(&short);
    specs[4].warm_start = Some(&infinite);
    let options = MlsmrColumnsOptions::default();
    let expected = one_by_one(&op, &rhs, &identity, &specs, options);
    assert!(expected[1..].iter().all(Result::is_err));
    assert_columns_match(&expected, &lockstep(&op, &rhs, &identity, &specs, options));
}

#[test]
fn local_reorthogonalization_runs_each_column_through_mlsmr() {
    let (op, b) = vandermonde_ls();
    let m = jacobi(&op);
    let rhs = vec![b.clone(), b.iter().map(|v| -2.0 * v).collect()];
    let specs = vec![
        ColumnSolve {
            tol: 1e-10,
            maxiter: 200,
            warm_start: None,
            escalation: None,
        };
        2
    ];
    let options = MlsmrColumnsOptions {
        local_size: Some(6),
        true_residuals: false,
    };
    let expected = one_by_one(&op, &rhs, &m, &specs, options);
    assert_columns_match(&expected, &lockstep(&op, &rhs, &m, &specs, options));
}

#[test]
fn the_block_returns_the_same_bits_at_any_thread_count() {
    let op = wrapped();
    let m = spread_preconditioner(op.n);
    let rhs: Vec<Vec<f64>> = (0..4).map(|j| pattern(op.m, j + 3, 1.0)).collect();
    let block = interleave(&rhs);
    let specs = vec![
        ColumnSolve {
            tol: 1e-14,
            maxiter: 30,
            warm_start: None,
            escalation: None,
        };
        4
    ];
    assert_pool_independent(|| {
        mlsmr_columns(
            &op,
            &block,
            &m,
            &specs,
            MlsmrColumnsOptions::default(),
            Cancellation::default(),
        )
        .iter()
        .map(fingerprint)
        .collect::<Vec<_>>()
    });
}

/// Cancellation, polled once per lockstep iteration, fails every column, with no
/// partial coefficients.
#[test]
fn cancellation_fails_every_column() {
    cancellation_fails_every_column_case(0);
    cancellation_fails_every_column_case(1);
    cancellation_fails_every_column_case(5);
}

fn cancellation_fails_every_column_case(polls: usize) {
    let op = wrapped();
    let m = spread_preconditioner(op.n);
    let rhs: Vec<Vec<f64>> = (0..3).map(|j| pattern(op.m, j + 9, 1.0)).collect();
    let block = interleave(&rhs);
    let specs = vec![
        ColumnSolve {
            tol: 1e-14,
            maxiter: 30,
            warm_start: None,
            escalation: None,
        };
        3
    ];
    let seen = AtomicUsize::new(0);
    let query = || seen.fetch_add(1, Ordering::Relaxed) >= polls;
    let results = mlsmr_columns(
        &op,
        &block,
        &m,
        &specs,
        MlsmrColumnsOptions::default(),
        Cancellation::new(&query),
    );
    assert_eq!(results.len(), 3);
    for result in results {
        let failure = result.err().expect("cancelled");
        assert_eq!(failure.source, SolveError::Cancelled);
        assert!(failure.iterations <= polls);
    }
}

#[test]
fn block_shape_errors_fail_every_column() {
    let op = OverdeterminedOp;
    let identity = IdentityOp { n: 3 };
    let spec = ColumnSolve {
        tol: 1e-12,
        maxiter: 10,
        warm_start: None,
        escalation: None,
    };
    let short = mlsmr_columns(
        &op,
        &[1.0; 7],
        &identity,
        &[spec, spec],
        MlsmrColumnsOptions::default(),
        Cancellation::default(),
    );
    assert!(short.iter().all(Result::is_err));
    let wide = mlsmr_columns(
        &op,
        &[1.0; 4 * (MAX_COLUMNS + 1)],
        &identity,
        &[spec; MAX_COLUMNS + 1],
        MlsmrColumnsOptions::default(),
        Cancellation::default(),
    );
    assert!(wide.iter().all(Result::is_err));
    assert!(mlsmr_columns(
        &op,
        &[],
        &identity,
        &[],
        MlsmrColumnsOptions::default(),
        Cancellation::default(),
    )
    .is_empty());
}

/// The default column applications are the column-by-column `apply`.
#[test]
fn default_column_applications_apply_each_column() {
    let op = OverdeterminedOp;
    let x = [1.0, 10.0, 2.0, 20.0, 3.0, 30.0];
    let mut y = [0.0; 8];
    op.apply_columns(2, &x, &mut y).expect("apply");
    assert_eq!(y, [1.0, 10.0, 2.0, 20.0, 3.0, 30.0, 3.0, 30.0]);
    let mut back = [0.0; 6];
    op.apply_adjoint_columns(2, &y, &mut back).expect("adjoint");
    assert_eq!(back, [4.0, 40.0, 5.0, 50.0, 3.0, 30.0]);
    assert!(op.apply_columns(3, &x, &mut y).is_err());
}
