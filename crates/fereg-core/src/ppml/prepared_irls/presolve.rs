//! Elimination pre-solve for categorical-intercept PPML publication fits.
//!
//! Before the certified iteration starts, improve its seed with plain IRLS
//! steps whose weighted projections use the exact block elimination: each
//! step projects `[z, X]` under the current Hessian weights (reprojecting the
//! previous residuals plus the working-response change, which has the same
//! projection), solves the small normal equations of the continuous
//! regressors, and accepts the step under the deviance line search. Only the
//! predictor is handed over (coefficients stay at their seed; the certified
//! solves decide rank and slots). The certified iteration then starts near
//! its fixed point; every published quantity and certificate still comes
//! from it. Any unsupported design,
//! decline, rank problem or nonfinite value leaves the seed untouched.

use super::super::family::PreparedPoissonInputs;
use super::super::PpmlConfig;
use crate::data::DenseMatrix;
use crate::error::Result;
use crate::fe::block_schur;
use crate::fe::{BorrowedMapWeights, MapConfig};
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::ols::PreparedOlsColumnKind;
use rayon::prelude::*;

/// Largest number of continuous regressors handled by the pre-solve.
const MAX_COLUMNS: usize = 8;
/// Normal-equation partials: lower Gram triangle, right side, raw squares.
type Moments = (
    [f64; MAX_COLUMNS * MAX_COLUMNS],
    [f64; MAX_COLUMNS],
    [f64; MAX_COLUMNS],
);
/// Largest number of pre-solve steps.
const MAX_STEPS: usize = 12;
/// Smallest within-to-raw weighted square ratio of a pre-solved regressor
/// (well above the certified collinearity tolerances).
const COLLINEARITY_RATIO: f64 = 1e-8;
/// Rows per fixed chunk of the moment and predictor passes.
const ROWS: usize = 1 << 14;

/// Steps of an accepted pre-solve and whether its last step met the
/// deviance convergence criterion of the certified iteration.
#[derive(Debug, Clone, Copy)]
pub(super) struct PresolveReport {
    pub(super) steps: usize,
    /// Projection iterations (at least one per step).
    pub(super) projection_iterations: usize,
    pub(super) converged: bool,
    /// Relative deviance change of the last step.
    pub(super) change: f64,
}

/// Improve `(coefficients, eta)` in place; `None` leaves both untouched.
#[allow(clippy::too_many_arguments)]
pub(super) fn presolve<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    inputs: &PreparedPoissonInputs<'_>,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PpmlConfig,
    eta: &mut [f64],
) -> Result<Option<PresolveReport>> {
    if !block_schur::enabled() {
        return Ok(None);
    }
    let Some(design) = prepared.fit_borrowed_intercept_map_design()? else {
        return Ok(None);
    };
    let rows = outcome.len();
    if design.effects().len() < 2 || design.nobs() != rows || eta.len() != rows {
        return Ok(None);
    }
    let Some((columns, k)) = continuous_columns(prepared, rows) else {
        return Ok(None);
    };
    let columns = &columns[..k];
    let threads = rayon::current_num_threads();
    let mut partials = crate::memory::fallible_filled(
        super::super::family::objective_partial_cells(rows, threads),
        0.0,
        "unable to reserve PPML pre-solve objective partials",
    )?;
    // Per-chunk deviance partials of the fused step pass (NaN: invalid).
    let mut chunk_deviance = crate::memory::fallible_filled(
        rows.div_ceil(ROWS),
        0.0,
        "unable to reserve PPML pre-solve chunk partials",
    )?;
    let mut mean =
        crate::memory::fallible_filled(rows, 0.0, "unable to reserve PPML pre-solve means")?;
    let mut weights =
        crate::memory::fallible_filled(rows, 0.0, "unable to reserve PPML pre-solve weights")?;
    let mut response =
        crate::memory::fallible_filled(rows, 0.0, "unable to reserve PPML pre-solve response")?;
    let mut scores =
        crate::memory::fallible_filled(rows, 0.0, "unable to reserve PPML pre-solve scores")?;
    let mut current =
        crate::memory::fallible_copy(eta, "unable to reserve PPML pre-solve predictor")?;
    if inputs
        .fill_means(&current, config.bounds, &mut mean)
        .is_err()
    {
        return Ok(None);
    }
    let Ok(initial) = inputs.evaluate_tiled(&current, &mean, &mut partials) else {
        return Ok(None);
    };
    let mut deviance = initial.deviance;
    if inputs
        .fill_working_state_and_scores(&mean, &current, &mut weights, &mut response, &mut scores)
        .is_err()
    {
        return Ok(None);
    }
    drop(scores);
    let mut input = Some(first_projection_input(&response, columns)?);
    let step_rows = StepRows {
        outcome,
        prior_weights,
        offset,
        terms: inputs.row_terms(),
        bounds: config.bounds,
    };
    let mut cache = block_schur::Cache::default();
    // The elimination accumulates the residual Gram inside its residual pass
    // (up to eight columns), which supplies the normal equations directly.
    cache.request_residual_gram(true);
    let mut change = f64::INFINITY;
    let mut steps = 0;
    let mut projection_iterations = 0_usize;
    let started = std::time::Instant::now();
    // Pre-solve steps are outer iterations of the same budget; the certified
    // iteration keeps at least two of them.
    let max_steps = MAX_STEPS.min(config.max_iterations.saturating_sub(2));
    if max_steps == 0 {
        return Ok(None);
    }
    while steps < max_steps {
        // Break is honoured before any state is handed over.
        crate::interrupt::checkpoint()?;
        steps += 1;
        let next = input.take().expect("pre-solve projection input");
        let Some((mut projected, iterations)) =
            project(&mut cache, &design, &weights, next, change)?
        else {
            return Ok(None);
        };
        projection_iterations += iterations;
        let Some((mut gram, rhs)) =
            step_normal_equations(&cache, projected.as_slice(), &weights, columns, steps == 1)?
        else {
            return Ok(None);
        };
        let Some(beta) = scaled_cholesky_solve(&mut gram, &rhs, k) else {
            return Ok(None);
        };
        step_rows.full_step(
            &beta[..k],
            projected.as_mut_slice(),
            StepState {
                eta: &mut current,
                mean: &mut mean,
                weights: &mut weights,
                response: &mut response,
                chunk_deviance: &mut chunk_deviance,
            },
        );
        let next_deviance: f64 = chunk_deviance.iter().sum();
        if !next_deviance.is_finite() {
            return Ok(None);
        }
        // Only full Newton steps that do not increase the deviance: a step
        // needing damping means the seed is not where plain steps contract,
        // and a small damped change is no evidence of convergence.
        if next_deviance > deviance + config.objective_slack * deviance.abs().max(1.0) {
            return Ok(None);
        }
        change = (deviance - next_deviance).abs() / deviance.min(next_deviance).max(0.1);
        deviance = next_deviance;
        input = Some(projected);
        // Stop well inside the criterion: the start's score certificate must
        // also pass for the first pass to be credited.
        if change <= 0.01 * config.tolerance {
            break;
        }
    }
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=ppml_presolve|steps={steps}|deviance={deviance:.17e}|change={change:.3e}|seconds={:.6}",
            started.elapsed().as_secs_f64()
        ));
    }
    eta.copy_from_slice(&current);
    Ok(Some(PresolveReport {
        steps,
        projection_iterations,
        converged: change <= config.tolerance,
        change,
    }))
}

/// Project `[z, X]` under the Hessian `weights` with the exact elimination at
/// a hundredth of the last deviance change; `None` on a decline. Also returns
/// the projection's iteration count (at least one).
fn project(
    cache: &mut block_schur::Cache,
    design: &crate::fe::BorrowedInterceptFeDesign<'_>,
    weights: &[f64],
    input: DenseMatrix,
    change: f64,
) -> Result<Option<(DenseMatrix, usize)>> {
    let map = MapConfig {
        tolerance: (0.01 * change).clamp(1e-11, 1e-6),
        ..MapConfig::default()
    };
    let request = block_schur::Request {
        retain_coefficients: false,
        coefficient_accuracy: true,
    };
    let weights = BorrowedMapWeights::Provided(weights);
    Ok(
        match cache.project(design, weights, input, map, None, request)? {
            block_schur::Outcome::Done(result) => {
                let (matrix, report) = result.into_parts();
                Some((matrix, report.iterations.max(1)))
            }
            block_schur::Outcome::Declined(_) => None,
        },
    )
}

/// The continuous regressor columns (at most `MAX_COLUMNS`) and their count;
/// the structural intercept is absorbed. `None` for any other column kind,
/// a non-contiguous column, more columns or none.
fn continuous_columns<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    rows: usize,
) -> Option<([&[f64]; MAX_COLUMNS], usize)> {
    let model = prepared.fit_model();
    let mut columns: [&[f64]; MAX_COLUMNS] = [&[]; MAX_COLUMNS];
    let mut k = 0_usize;
    for coefficient in &model.coefficients {
        let Some(generated) = coefficient.generated_column_index else {
            continue;
        };
        match model.columns.get(generated).map(|column| &column.kind) {
            Some(PreparedOlsColumnKind::Intercept) => {}
            Some(PreparedOlsColumnKind::Continuous { .. }) => {
                let values = crate::ols::prepared_fit_view::prepared_continuous_range(
                    prepared,
                    generated,
                    0..rows,
                )?;
                if k == MAX_COLUMNS {
                    return None;
                }
                columns[k] = values;
                k += 1;
            }
            _ => return None,
        }
    }
    (k > 0).then_some((columns, k))
}

/// The first projection input: `[z, X]` row-major from the raw design.
fn first_projection_input(response: &[f64], columns: &[&[f64]]) -> Result<DenseMatrix> {
    let rows = response.len();
    let width = columns.len() + 1;
    let mut data = crate::memory::fallible_filled(
        rows * width,
        0.0,
        "unable to reserve PPML pre-solve design",
    )?;
    data.par_chunks_mut(ROWS * width)
        .enumerate()
        .for_each(|(chunk, values)| {
            for (local, row) in values.chunks_exact_mut(width).enumerate() {
                let i = chunk * ROWS + local;
                row[0] = response[i];
                for (target, column) in row[1..].iter_mut().zip(columns) {
                    *target = column[i];
                }
            }
        });
    DenseMatrix::from_row_major(rows, width, data)
}

/// Normal equations of the projected regressors `data = [z~, X~]` (with the
/// raw weighted squares for the FE-collinearity check), over a fixed tree of
/// row chunks without allocation.
fn normal_equations(data: &[f64], weights: &[f64], columns: &[&[f64]]) -> Result<Moments> {
    let k = columns.len();
    let width = k + 1;
    crate::parallel::fixed_tree_reduce(
        weights.len(),
        ROWS,
        &|range: std::ops::Range<usize>| -> Result<Moments> {
            let mut gram = [0.0; MAX_COLUMNS * MAX_COLUMNS];
            let mut rhs = [0.0; MAX_COLUMNS];
            let mut raw = [0.0; MAX_COLUMNS];
            for i in range {
                let row = &data[i * width..(i + 1) * width];
                let w = weights[i];
                for a in 0..k {
                    let wa = w * row[1 + a];
                    rhs[a] += wa * row[0];
                    raw[a] += w * columns[a][i] * columns[a][i];
                    for b in 0..=a {
                        gram[a * MAX_COLUMNS + b] += wa * row[1 + b];
                    }
                }
            }
            Ok((gram, rhs, raw))
        },
        &|(mut gram, mut rhs, mut raw): Moments, (g, r, q): Moments| {
            for (target, value) in gram.iter_mut().zip(g) {
                *target += value;
            }
            for (target, value) in rhs.iter_mut().zip(r) {
                *target += value;
            }
            for (target, value) in raw.iter_mut().zip(q) {
                *target += value;
            }
            (gram, rhs, raw)
        },
    )
}

/// The step's normal equations: from the elimination's residual Gram when it
/// covers `[z~, X~]`, otherwise by a row pass. A regressor (nearly) collinear
/// with the fixed effects is a rank decision for the certified solve, so
/// `None` leaves such designs to it; the collinearity is structural, so the
/// first step's weights decide it (every step keeps the Cholesky pivot guard).
fn step_normal_equations(
    cache: &block_schur::Cache,
    projected: &[f64],
    weights: &[f64],
    columns: &[&[f64]],
    first: bool,
) -> Result<Option<([f64; MAX_COLUMNS * MAX_COLUMNS], [f64; MAX_COLUMNS])>> {
    let k = columns.len();
    let (gram, rhs) = match cache.residual_gram() {
        Some(residual_gram) if residual_gram.columns == k + 1 => {
            elimination_normal_equations(residual_gram, k)
        }
        _ => {
            let (gram, rhs, _) = normal_equations(projected, weights, columns)?;
            (gram, rhs)
        }
    };
    if first {
        let raw = raw_weighted_squares(weights, columns)?;
        if !(0..k).all(|a| gram[a * MAX_COLUMNS + a] > COLLINEARITY_RATIO * raw[a]) {
            return Ok(None);
        }
    }
    Ok(Some((gram, rhs)))
}

/// Normal equations of the regressors from the elimination's residual Gram
/// of `[z~, X~]` (raw weights): `X~' W X~` (lower triangle, stride
/// `MAX_COLUMNS`) and `X~' W z~`.
fn elimination_normal_equations(
    residual_gram: &block_schur::ResidualGram,
    k: usize,
) -> ([f64; MAX_COLUMNS * MAX_COLUMNS], [f64; MAX_COLUMNS]) {
    let width = residual_gram.columns;
    let values = &residual_gram.values;
    let mut gram = [0.0; MAX_COLUMNS * MAX_COLUMNS];
    let mut rhs = [0.0; MAX_COLUMNS];
    for a in 0..k {
        rhs[a] = values[(1 + a) * width];
        for b in 0..=a {
            gram[a * MAX_COLUMNS + b] = values[(1 + a) * width + 1 + b];
        }
    }
    (gram, rhs)
}

/// Raw weighted squares `sum w x_a^2` of the regressors over a fixed tree of
/// row chunks (allocation free).
fn raw_weighted_squares(weights: &[f64], columns: &[&[f64]]) -> Result<[f64; MAX_COLUMNS]> {
    crate::parallel::fixed_tree_reduce(
        weights.len(),
        ROWS,
        &|range: std::ops::Range<usize>| -> Result<[f64; MAX_COLUMNS]> {
            let mut raw = [0.0; MAX_COLUMNS];
            for (square, column) in raw.iter_mut().zip(columns) {
                for i in range.clone() {
                    *square += weights[i] * column[i] * column[i];
                }
            }
            Ok(raw)
        },
        &|mut left: [f64; MAX_COLUMNS], right: [f64; MAX_COLUMNS]| {
            for (target, value) in left.iter_mut().zip(right) {
                *target += value;
            }
            left
        },
    )
}

/// Fixed per-row inputs of a pre-solve step.
struct StepRows<'a> {
    outcome: &'a [f64],
    prior_weights: &'a [f64],
    offset: &'a [f64],
    terms: &'a [[f64; 2]],
    bounds: super::super::PoissonBounds,
}

/// Row state a step rewrites, and its per-chunk deviance partials.
struct StepState<'a> {
    eta: &'a mut [f64],
    mean: &'a mut [f64],
    weights: &'a mut [f64],
    response: &'a mut [f64],
    chunk_deviance: &'a mut [f64],
}

/// One evaluated row of a full step.
struct StepRow {
    eta: f64,
    mean: f64,
    weight: f64,
    response: f64,
    deviance: f64,
}

impl StepRows<'_> {
    /// One pass for the full step: eta = offset + z - z~ + X~ beta, its mean
    /// and deviance terms, the next Hessian weights and working response, and
    /// the next projection input (residuals plus the working-response change:
    /// the same projection under new weights). A chunk with an invalid row
    /// leaves NaN as its deviance partial.
    fn full_step(&self, beta: &[f64], projected: &mut [f64], state: StepState<'_>) {
        let width = beta.len() + 1;
        projected
            .par_chunks_mut(ROWS * width)
            .zip(state.eta.par_chunks_mut(ROWS))
            .zip(state.mean.par_chunks_mut(ROWS))
            .zip(state.weights.par_chunks_mut(ROWS))
            .zip(state.response.par_chunks_mut(ROWS))
            .zip(state.chunk_deviance.par_iter_mut())
            .enumerate()
            .for_each(
                |(chunk, (((((values, eta), mean), weights), response), out))| {
                    *out = f64::NAN;
                    let mut step = || -> Option<f64> {
                        let mut sum = 0.0_f64;
                        let mut correction = 0.0_f64;
                        let mut run = 0.0_f64;
                        for (local, ((((row, eta), mu), w), z)) in values
                            .chunks_exact_mut(width)
                            .zip(eta.iter_mut())
                            .zip(mean.iter_mut())
                            .zip(weights.iter_mut())
                            .zip(response.iter_mut())
                            .enumerate()
                        {
                            let next = self.row(chunk * ROWS + local, beta, row, *z)?;
                            run += next.deviance;
                            if local % 512 == 511 {
                                crate::linalg::neumaier_add(run, &mut sum, &mut correction);
                                run = 0.0;
                            }
                            row[0] += next.response - *z;
                            *z = next.response;
                            *w = next.weight;
                            *mu = next.mean;
                            *eta = next.eta;
                        }
                        crate::linalg::neumaier_add(run, &mut sum, &mut correction);
                        Some(sum + correction)
                    };
                    if let Some(value) = step() {
                        *out = value;
                    }
                },
            );
    }

    /// Row `i` of the full step from its projected row `[z~, X~]` and current
    /// working response `z`; `None` when any value is invalid.
    #[inline(always)]
    fn row(&self, i: usize, beta: &[f64], projected: &[f64], z: f64) -> Option<StepRow> {
        let bounds = self.bounds;
        let mut fitted = z - projected[0];
        for (b, x) in beta.iter().zip(&projected[1..]) {
            fitted += b * x;
        }
        let value = self.offset[i] + fitted;
        if !(value.is_finite() && value >= bounds.eta_min && value <= bounds.eta_max) {
            return None;
        }
        let m = value.exp();
        if !(m.is_finite() && m > 0.0 && m >= bounds.mu_min && m <= bounds.mu_max) {
            return None;
        }
        let y = self.outcome[i];
        let q = self.prior_weights[i];
        let log_y = self.terms[i][0];
        let half = super::super::family::iteration_half_deviance(y, m, value, log_y).ok()?;
        let adjustment = if y == 0.0 {
            -1.0
        } else {
            let ratio = y / m;
            if y.is_normal() && m.is_normal() && ratio.is_normal() {
                ratio - 1.0
            } else {
                (log_y - value).exp() - 1.0
            }
        };
        let next = value - self.offset[i] + adjustment;
        let weight = q * m;
        if !(next.is_finite() && weight.is_finite() && weight > 0.0) {
            return None;
        }
        Some(StepRow {
            eta: value,
            mean: m,
            weight,
            response: next,
            deviance: 2.0 * q * half,
        })
    }
}

/// Solve `A x = b` for the symmetric positive definite `A` whose lower
/// triangle is stored row-major with stride `MAX_COLUMNS`, after scaling to
/// unit diagonal. `None` when a scaled pivot is not clearly positive.
fn scaled_cholesky_solve(
    gram: &mut [f64; MAX_COLUMNS * MAX_COLUMNS],
    rhs: &[f64; MAX_COLUMNS],
    k: usize,
) -> Option<[f64; MAX_COLUMNS]> {
    const S: usize = MAX_COLUMNS;
    let mut scale = [0.0_f64; MAX_COLUMNS];
    for a in 0..k {
        scale[a] = gram[a * S + a].sqrt();
        if !scale[a].is_finite() || scale[a] <= 0.0 {
            return None;
        }
    }
    for a in 0..k {
        for b in 0..=a {
            gram[a * S + b] /= scale[a] * scale[b];
        }
    }
    for a in 0..k {
        let mut pivot = gram[a * S + a];
        for c in 0..a {
            pivot -= gram[a * S + c] * gram[a * S + c];
        }
        if pivot.is_nan() || pivot <= 1e-10 {
            return None;
        }
        let pivot = pivot.sqrt();
        gram[a * S + a] = pivot;
        for b in a + 1..k {
            let mut value = gram[b * S + a];
            for c in 0..a {
                value -= gram[b * S + c] * gram[a * S + c];
            }
            gram[b * S + a] = value / pivot;
        }
    }
    let mut y = [0.0_f64; MAX_COLUMNS];
    for a in 0..k {
        y[a] = rhs[a] / scale[a];
        for c in 0..a {
            y[a] -= gram[a * S + c] * y[c];
        }
        y[a] /= gram[a * S + a];
    }
    for a in (0..k).rev() {
        for c in a + 1..k {
            y[a] -= gram[c * S + a] * y[c];
        }
        y[a] /= gram[a * S + a];
    }
    let mut solution = [0.0_f64; MAX_COLUMNS];
    for a in 0..k {
        solution[a] = y[a] / scale[a];
        if !solution[a].is_finite() {
            return None;
        }
    }
    Some(solution)
}
