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
    // Continuous regressors only; the structural intercept is absorbed.
    let model = prepared.fit_model();
    let mut columns: Vec<&[f64]> = Vec::new();
    for coefficient in &model.coefficients {
        let Some(generated) = coefficient.generated_column_index else {
            continue;
        };
        match model.columns.get(generated).map(|column| &column.kind) {
            Some(PreparedOlsColumnKind::Intercept) => {}
            Some(PreparedOlsColumnKind::Continuous { .. }) => {
                let Some(values) = crate::ols::prepared_fit_view::prepared_continuous_range(
                    prepared,
                    generated,
                    0..rows,
                ) else {
                    return Ok(None);
                };
                columns.push(values);
            }
            _ => return Ok(None),
        }
    }
    let k = columns.len();
    if k == 0 || k > 32 {
        return Ok(None);
    }
    let width = k + 1;
    let threads = rayon::current_num_threads();
    let terms = inputs.row_terms();
    let mut partials = vec![0.0; super::super::family::objective_partial_cells(rows, threads)];
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
    // The first projection input: [z, X] from the raw design.
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
                for (target, column) in row[1..].iter_mut().zip(&columns) {
                    *target = column[i];
                }
            }
        });
    let mut input = Some(DenseMatrix::from_row_major(rows, width, data)?);
    let mut cache = block_schur::Cache::default();
    let mut change = f64::INFINITY;
    let mut steps = 0;
    let mut projection_iterations = 0_usize;
    let started = std::time::Instant::now();
    let bounds = config.bounds;
    while steps < MAX_STEPS {
        steps += 1;
        let tolerance = (0.01 * change).clamp(1e-11, 1e-6);
        let map = MapConfig {
            tolerance,
            ..MapConfig::default()
        };
        let mut projected = match cache.project(
            &design,
            BorrowedMapWeights::Provided(&weights),
            input.take().expect("pre-solve projection input"),
            map,
            None,
            block_schur::Request {
                retain_coefficients: false,
                coefficient_accuracy: true,
            },
        )? {
            block_schur::Outcome::Done(result) => {
                let (matrix, report) = result.into_parts();
                projection_iterations += report.iterations.max(1);
                matrix
            }
            block_schur::Outcome::Declined(_) => return Ok(None),
        };
        // Normal equations of the projected regressors (with the raw
        // weighted squares for the FE-collinearity check).
        let parts: Vec<(Vec<f64>, Vec<f64>, Vec<f64>)> = projected
            .as_slice()
            .par_chunks(ROWS * width)
            .zip(weights.par_chunks(ROWS))
            .enumerate()
            .map(|(chunk, (values, weights))| {
                let mut gram = vec![0.0; k * k];
                let mut rhs = vec![0.0; k];
                let mut raw = vec![0.0; k];
                for (local, (row, &w)) in values.chunks_exact(width).zip(weights).enumerate() {
                    let i = chunk * ROWS + local;
                    for a in 0..k {
                        let wa = w * row[1 + a];
                        rhs[a] += wa * row[0];
                        raw[a] += w * columns[a][i] * columns[a][i];
                        for b in 0..=a {
                            gram[a * k + b] += wa * row[1 + b];
                        }
                    }
                }
                (gram, rhs, raw)
            })
            .collect();
        let mut gram = vec![0.0; k * k];
        let mut rhs = vec![0.0; k];
        let mut raw = vec![0.0; k];
        for (g, r, q) in parts {
            for (target, value) in gram.iter_mut().zip(g) {
                *target += value;
            }
            for (target, value) in rhs.iter_mut().zip(r) {
                *target += value;
            }
            for (target, value) in raw.iter_mut().zip(q) {
                *target += value;
            }
        }
        // A regressor (nearly) collinear with the fixed effects is a rank
        // decision for the certified solve; leave such designs to it.
        let separated = |a: usize| gram[a * k + a] > COLLINEARITY_RATIO * raw[a];
        if !(0..k).all(separated) {
            return Ok(None);
        }
        let Some(beta) = scaled_cholesky_solve(&mut gram, &rhs, k) else {
            return Ok(None);
        };
        // One pass for the full step: eta = offset + z - z~ + X~ beta, its
        // mean and deviance terms, the next Hessian weights and working
        // response, and the next projection input (residuals plus the
        // working-response change: the same projection under new weights).
        let parts: Vec<Option<f64>> = projected
            .as_mut_slice()
            .par_chunks_mut(ROWS * width)
            .zip(current.par_chunks_mut(ROWS))
            .zip(mean.par_chunks_mut(ROWS))
            .zip(weights.par_chunks_mut(ROWS))
            .zip(response.par_chunks_mut(ROWS))
            .enumerate()
            .map(|(chunk, ((((values, eta), mean), weights), response))| {
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
                    let i = chunk * ROWS + local;
                    let mut fitted = *z - row[0];
                    for (b, x) in beta.iter().zip(&row[1..]) {
                        fitted += b * x;
                    }
                    let value = offset[i] + fitted;
                    if !(value.is_finite() && value >= bounds.eta_min && value <= bounds.eta_max) {
                        return None;
                    }
                    let m = value.exp();
                    if !(m.is_finite() && m > 0.0 && m >= bounds.mu_min && m <= bounds.mu_max) {
                        return None;
                    }
                    let y = outcome[i];
                    let q = prior_weights[i];
                    let half =
                        super::super::family::iteration_half_deviance(y, m, value, terms[i][0])
                            .ok()?;
                    run += 2.0 * q * half;
                    if local % 512 == 511 {
                        crate::linalg::neumaier_add(run, &mut sum, &mut correction);
                        run = 0.0;
                    }
                    let adjustment = if y == 0.0 {
                        -1.0
                    } else {
                        let ratio = y / m;
                        if y.is_normal() && m.is_normal() && ratio.is_normal() {
                            ratio - 1.0
                        } else {
                            (terms[i][0] - value).exp() - 1.0
                        }
                    };
                    let next = value - offset[i] + adjustment;
                    let weight = q * m;
                    if !(next.is_finite() && weight.is_finite() && weight > 0.0) {
                        return None;
                    }
                    row[0] += next - *z;
                    *z = next;
                    *w = weight;
                    *mu = m;
                    *eta = value;
                }
                crate::linalg::neumaier_add(run, &mut sum, &mut correction);
                Some(sum + correction)
            })
            .collect();
        let Some(next_deviance) = parts
            .into_iter()
            .try_fold(0.0_f64, |total, part| part.map(|part| total + part))
            .filter(|value| value.is_finite())
        else {
            return Ok(None);
        };
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

/// Solve `A x = b` for the symmetric positive definite `A` whose lower
/// triangle is stored row-major, after scaling to unit diagonal. `None` when a
/// scaled pivot is not clearly positive.
fn scaled_cholesky_solve(gram: &mut [f64], rhs: &[f64], k: usize) -> Option<Vec<f64>> {
    let scale: Vec<f64> = (0..k).map(|a| gram[a * k + a].sqrt()).collect();
    if scale.iter().any(|s| !s.is_finite() || *s <= 0.0) {
        return None;
    }
    for a in 0..k {
        for b in 0..=a {
            gram[a * k + b] /= scale[a] * scale[b];
        }
    }
    for a in 0..k {
        let mut pivot = gram[a * k + a];
        for c in 0..a {
            pivot -= gram[a * k + c] * gram[a * k + c];
        }
        if pivot.is_nan() || pivot <= 1e-10 {
            return None;
        }
        let pivot = pivot.sqrt();
        gram[a * k + a] = pivot;
        for b in a + 1..k {
            let mut value = gram[b * k + a];
            for c in 0..a {
                value -= gram[b * k + c] * gram[a * k + c];
            }
            gram[b * k + a] = value / pivot;
        }
    }
    let mut y: Vec<f64> = (0..k).map(|a| rhs[a] / scale[a]).collect();
    for a in 0..k {
        for c in 0..a {
            y[a] -= gram[a * k + c] * y[c];
        }
        y[a] /= gram[a * k + a];
    }
    for a in (0..k).rev() {
        for c in a + 1..k {
            y[a] -= gram[c * k + a] * y[c];
        }
        y[a] /= gram[a * k + a];
    }
    let solution: Vec<f64> = y.iter().zip(&scale).map(|(v, s)| v / s).collect();
    solution.iter().all(|v| v.is_finite()).then_some(solution)
}
