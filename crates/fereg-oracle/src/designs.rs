//! Reproducible small designs, owned entirely by the independent oracle crate.
mod sample;
mod topology;
use crate::explicit_ols::{DummyEffect, ExplicitOlsProblem};
use crate::explicit_ppml::{PpmlProblem, PpmlWeights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graph {
    DensePanel,
    AkmStrong,
    AkmWeak,
    Chain,
    Barbell,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightKind {
    Unit,
    Analytic,
    Frequency,
    Probability,
}

#[derive(Debug, Clone)]
pub struct DesignSpec {
    pub rows: usize,
    pub levels: Vec<usize>,
    pub regressors: usize,
    pub graph: Graph,
    pub slopes: bool,
    pub weights: WeightKind,
    pub missing_fraction: f64,
    pub singleton: bool,
    pub collinear: bool,
    pub clusters: usize,
    pub poisson: bool,
    pub zero_fraction: f64,
}

impl Default for DesignSpec {
    fn default() -> Self {
        Self {
            rows: 120,
            levels: vec![6, 5],
            regressors: 2,
            graph: Graph::DensePanel,
            slopes: false,
            weights: WeightKind::Unit,
            missing_fraction: 0.0,
            singleton: false,
            collinear: false,
            clusters: 0,
            poisson: false,
            zero_fraction: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Design {
    pub outcome: Vec<f64>,
    pub regressors: Vec<Vec<f64>>,
    pub effects: Vec<DummyEffect>,
    pub weight_kind: WeightKind,
    pub weights: Vec<f64>,
    /// Missing in a model input; values are retained to test masking separately.
    pub missing: Vec<bool>,
    pub clusters: Vec<Vec<u64>>,
}

/// SplitMix64 is specified here rather than using a version-dependent RNG crate.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

pub fn generate(seed: u64, spec: &DesignSpec) -> Result<Design, String> {
    if spec.rows == 0
        || spec
            .levels
            .iter()
            .any(|&l| l == 0 || l >= u32::MAX as usize)
        || spec.clusters > 5
        || !spec.missing_fraction.is_finite()
        || !(0.0..1.0).contains(&spec.missing_fraction)
        || !spec.zero_fraction.is_finite()
        || !(0.0..1.0).contains(&spec.zero_fraction)
    {
        return Err("invalid random-design dimensions or fractions".into());
    }
    let mut rng = Rng(seed);
    let mut codes = topology::codes(spec, &mut rng);
    if spec.singleton && !codes.is_empty() {
        codes[0][spec.rows - 1] = spec.levels[0] as u32;
    }
    let effects = codes
        .into_iter()
        .enumerate()
        .map(|(j, codes)| {
            let slopes = if spec.slopes {
                vec![(0..spec.rows).map(|_| 2.0 * rng.uniform() - 1.0).collect()]
            } else {
                vec![]
            };
            DummyEffect::new(
                codes,
                spec.levels[j] + usize::from(j == 0 && spec.singleton),
                true,
                slopes,
            )
            .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut regressors = (0..spec.regressors)
        .map(|_| {
            (0..spec.rows)
                .map(|_| 2.0 * rng.uniform() - 1.0)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if spec.collinear && !regressors.is_empty() {
        regressors.push(regressors[0].clone());
    }
    let outcome = (0..spec.rows)
        .map(|i| {
            let finite = regressors
                .iter()
                .enumerate()
                .map(|(j, x)| x[i] * 0.4 / (j + 1) as f64)
                .sum::<f64>();
            let absorbed = effects
                .iter()
                .enumerate()
                .map(|(j, e)| {
                    let intercept = (f64::from(e.codes()[i]) * (j + 1) as f64 * 0.3).sin() * 0.4;
                    intercept + e.slopes().iter().map(|s| s[i] * 0.15).sum::<f64>()
                })
                .sum::<f64>();
            let noise = (rng.uniform() - 0.5) * 0.2;
            if spec.poisson {
                if rng.uniform() < spec.zero_fraction {
                    0.0
                } else {
                    (finite + absorbed + noise).exp()
                }
            } else {
                finite + absorbed + noise
            }
        })
        .collect();
    let weights = (0..spec.rows)
        .map(|_| match spec.weights {
            WeightKind::Unit => 1.0,
            WeightKind::Frequency => (1 + rng.next() % 4) as f64,
            _ => (rng.uniform() * 4.0 - 2.0).exp(),
        })
        .collect();
    let missing = (0..spec.rows)
        .map(|_| rng.uniform() < spec.missing_fraction)
        .collect();
    let clusters = (0..spec.clusters)
        .map(|j| {
            (0..spec.rows)
                .map(|i| ((i / (j + 1)) % (5 + j)) as u64 + (1u64 << 40))
                .collect()
        })
        .collect();
    Ok(Design {
        outcome,
        regressors,
        effects,
        weight_kind: spec.weights,
        weights,
        missing,
        clusters,
    })
}

impl Design {
    /// Independent complete-case filtering and recursive singleton removal.
    pub fn oracle_problem(
        &self,
        keep_singletons: bool,
    ) -> Result<(ExplicitOlsProblem, Vec<usize>), String> {
        sample::prepare(self, keep_singletons)
    }

    pub fn poisson_problem(
        &self,
        keep_singletons: bool,
    ) -> Result<(PpmlProblem, Vec<usize>), String> {
        let (ols, rows) = self.oracle_problem(keep_singletons)?;
        let mut columns = ols.regressors;
        for effect in &ols.effects {
            for level in 0..effect.n_levels() {
                if effect.has_intercept() {
                    columns.push(
                        effect
                            .codes()
                            .iter()
                            .map(|&c| f64::from(c as usize == level))
                            .collect(),
                    );
                }
                for slope in effect.slopes() {
                    columns.push(
                        effect
                            .codes()
                            .iter()
                            .zip(slope)
                            .map(|(&c, &s)| if c as usize == level { s } else { 0.0 })
                            .collect(),
                    );
                }
            }
        }
        let weights = match self.weight_kind {
            WeightKind::Unit => PpmlWeights::Unit,
            WeightKind::Frequency => {
                PpmlWeights::Frequency(rows.iter().map(|&i| self.weights[i] as u32).collect())
            }
            WeightKind::Probability => {
                PpmlWeights::Probability(rows.iter().map(|&i| self.weights[i]).collect())
            }
            WeightKind::Analytic => return Err("PPML does not support analytic weights".into()),
        };
        let design = (0..rows.len())
            .map(|i| columns.iter().map(|c| c[i]).collect())
            .collect();
        Ok((
            PpmlProblem {
                design,
                outcome: ols.outcome,
                offset: vec![],
                weights,
                clusters: ols.clusters,
            },
            rows,
        ))
    }
}
