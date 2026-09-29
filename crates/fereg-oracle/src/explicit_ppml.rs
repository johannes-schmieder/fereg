//! Dense damped-Newton Poisson oracle, ported from
//! tests/oracles/ppml_explicit_dummy_oracle.py at legacy-v0.5.0-beta.
//! This materializes the full design and never calls production IRLS or WLS.
mod algebra;
mod inference;
use algebra::*;

#[derive(Debug, Clone, PartialEq)]
pub enum PpmlWeights {
    Unit,
    Frequency(Vec<u32>),
    Probability(Vec<f64>),
}

#[derive(Debug, Clone)]
pub struct PpmlProblem {
    /// Row-major, including every finite and explicit dummy/slope column.
    pub design: Vec<Vec<f64>>,
    pub outcome: Vec<f64>,
    /// Empty means zero; exposure is supplied here after taking its logarithm.
    pub offset: Vec<f64>,
    pub weights: PpmlWeights,
    /// Empty selects robust inference; otherwise uncorrected subset meats are combined.
    pub clusters: Vec<Vec<u64>>,
}

#[derive(Debug, Clone, Copy)]
pub struct PpmlConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
}

impl Default for PpmlConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-11,
            max_iterations: 200,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PpmlResult {
    pub coefficients: Vec<f64>,
    /// Full-column covariance before any multiway PSD repair, with omitted slots zero.
    pub covariance: Vec<Vec<f64>>,
    pub eta: Vec<f64>,
    pub mean: Vec<f64>,
    pub score: Vec<f64>,
    pub hessian: Vec<Vec<f64>>,
    pub deviance: f64,
    /// Omits the coefficient-independent log-factorial term, as in the Python oracle.
    pub log_likelihood_kernel: f64,
    pub iterations: usize,
    pub kept_columns: Vec<usize>,
    pub omitted_columns: Vec<usize>,
}

struct Prepared {
    x: Vec<Vec<f64>>,
    q: Vec<f64>,
    effective_n: usize,
    kept: Vec<usize>,
    omitted: Vec<usize>,
}

struct State {
    eta: Vec<f64>,
    mean: Vec<f64>,
    objective: f64,
}

pub fn fit_explicit_ppml(problem: &PpmlProblem) -> Result<PpmlResult, String> {
    fit_explicit_ppml_with_config(problem, PpmlConfig::default())
}

pub fn fit_explicit_ppml_with_config(
    problem: &PpmlProblem,
    config: PpmlConfig,
) -> Result<PpmlResult, String> {
    let prepared = prepare(problem, config)?;
    let (beta, state, iterations) = iterate(problem, &prepared, config)?;
    let score = score(&prepared.x, &problem.outcome, &state.mean, &prepared.q);
    let hessian = cross_product(
        &prepared.x,
        &prepared
            .q
            .iter()
            .zip(&state.mean)
            .map(|(q, mu)| q * mu)
            .collect::<Vec<_>>(),
    );
    let covariance = inference::covariance(problem, &prepared, &state.mean, &invert(&hessian)?)?;
    let width = problem.design[0].len();
    let mut coefficients = vec![0.0; width];
    let mut full_covariance = vec![vec![0.0; width]; width];
    for (i, &column) in prepared.kept.iter().enumerate() {
        coefficients[column] = beta[i];
        for (j, &other) in prepared.kept.iter().enumerate() {
            full_covariance[column][other] = covariance[i][j];
        }
    }
    let deviance = poisson_deviance(&problem.outcome, &state.mean, &prepared.q);
    if !deviance.is_finite() || full_covariance.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Poisson oracle produced nonfinite final inference".into());
    }
    Ok(PpmlResult {
        coefficients,
        covariance: full_covariance,
        eta: state.eta,
        mean: state.mean,
        score,
        hessian,
        deviance,
        log_likelihood_kernel: state.objective,
        iterations,
        kept_columns: prepared.kept,
        omitted_columns: prepared.omitted,
    })
}

fn prepare(problem: &PpmlProblem, config: PpmlConfig) -> Result<Prepared, String> {
    let n = problem.outcome.len();
    let width = problem.design.first().map_or(0, Vec::len);
    if n == 0
        || width == 0
        || problem.design.len() != n
        || problem
            .design
            .iter()
            .any(|row| row.len() != width || row.iter().any(|v| !v.is_finite()))
        || problem.outcome.iter().any(|v| !v.is_finite() || *v < 0.0)
        || !problem.outcome.iter().any(|v| *v > 0.0)
        || (!problem.offset.is_empty() && problem.offset.len() != n)
        || problem.offset.iter().any(|v| !v.is_finite())
        || problem.clusters.len() > 5
        || problem.clusters.iter().any(|c| c.len() != n)
        || !config.tolerance.is_finite()
        || config.tolerance <= 0.0
        || config.tolerance >= 1.0
        || config.max_iterations == 0
    {
        return Err("invalid Poisson oracle data or configuration".into());
    }
    let (q, effective_n) = match &problem.weights {
        PpmlWeights::Unit => (vec![1.0; n], n),
        PpmlWeights::Frequency(f) => {
            if f.len() != n || f.contains(&0) {
                return Err("invalid Poisson frequencies".into());
            }
            let count = f
                .iter()
                .try_fold(0usize, |n, &v| n.checked_add(v as usize))
                .ok_or("frequency sum overflow")?;
            (f.iter().map(|&v| f64::from(v)).collect(), count)
        }
        PpmlWeights::Probability(w) => {
            if w.len() != n || w.iter().any(|v| !v.is_finite() || *v <= 0.0) {
                return Err("invalid Poisson probability weights".into());
            }
            let maximum = max_abs(w);
            let scaled = w.iter().map(|v| v / maximum).collect::<Vec<_>>();
            let sum: f64 = scaled.iter().sum();
            (scaled.iter().map(|v| v * (n as f64 / sum)).collect(), n)
        }
    };
    if q.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err("unrepresentable Poisson weights".into());
    }
    let (kept, omitted) = select_independent_columns(&problem.design);
    if kept.is_empty() || effective_n <= kept.len() {
        return Err("insufficient Poisson oracle rank/DoF".into());
    }
    let x = problem
        .design
        .iter()
        .map(|row| kept.iter().map(|&j| row[j]).collect())
        .collect();
    Ok(Prepared {
        x,
        q,
        effective_n,
        kept,
        omitted,
    })
}

fn evaluate(problem: &PpmlProblem, prepared: &Prepared, beta: &[f64]) -> State {
    let eta = prepared
        .x
        .iter()
        .enumerate()
        .map(|(i, row)| dot(row, beta) + problem.offset.get(i).copied().unwrap_or(0.0))
        .collect::<Vec<_>>();
    let mean = eta.iter().map(|v| v.exp()).collect::<Vec<_>>();
    let objective = if mean.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        f64::NEG_INFINITY
    } else {
        problem
            .outcome
            .iter()
            .zip(&eta)
            .zip(&mean)
            .zip(&prepared.q)
            .map(|(((y, eta), mu), q)| q * (y * eta - mu))
            .sum()
    };
    State {
        eta,
        mean,
        objective,
    }
}

fn iterate(
    problem: &PpmlProblem,
    prepared: &Prepared,
    config: PpmlConfig,
) -> Result<(Vec<f64>, State, usize), String> {
    let mut beta = vec![0.0; prepared.kept.len()];
    let mut current = evaluate(problem, prepared, &beta);
    if !current.objective.is_finite() {
        return Err("nonfinite Poisson oracle initial state".into());
    }
    for iteration in 1..=config.max_iterations {
        let gradient = score(&prepared.x, &problem.outcome, &current.mean, &prepared.q);
        let h = prepared
            .q
            .iter()
            .zip(&current.mean)
            .map(|(q, mu)| q * mu)
            .collect::<Vec<_>>();
        let step = invert(&cross_product(&prepared.x, &h))?
            .iter()
            .map(|row| dot(row, &gradient))
            .collect::<Vec<_>>();
        let gain = dot(&gradient, &step);
        let mut scale = 1.0;
        let mut accepted = None;
        while scale >= 2.0_f64.powi(-40) {
            let candidate = beta
                .iter()
                .zip(&step)
                .map(|(b, d)| b + scale * d)
                .collect::<Vec<_>>();
            let state = evaluate(problem, prepared, &candidate);
            if state.objective.is_finite()
                && state.objective >= current.objective + 1e-4 * scale * gain
            {
                accepted = Some((candidate, state));
                break;
            }
            scale *= 0.5;
        }
        let (candidate, state) = accepted.ok_or("Poisson oracle could not improve objective")?;
        beta = candidate;
        current = state;
        let score_scale = (0..beta.len())
            .map(|j| {
                prepared
                    .x
                    .iter()
                    .zip(&current.mean)
                    .zip(&prepared.q)
                    .map(|((row, mu), q)| q * mu * row[j].abs())
                    .sum::<f64>()
            })
            .fold(1.0, f64::max);
        let new_score = score(&prepared.x, &problem.outcome, &current.mean, &prepared.q);
        if scale * max_abs(&step) <= config.tolerance * (1.0 + max_abs(&beta))
            && max_abs(&new_score) <= config.tolerance.sqrt() * score_scale
        {
            return Ok((beta, current, iteration));
        }
    }
    Err("Poisson oracle did not converge".into())
}

pub fn poisson_deviance(y: &[f64], mu: &[f64], q: &[f64]) -> f64 {
    y.iter()
        .zip(mu)
        .zip(q)
        .map(|((&y, &mu), &q)| {
            2.0 * q
                * if y == 0.0 {
                    mu
                } else {
                    mu - y + y * (y / mu).ln()
                }
        })
        .sum()
}
