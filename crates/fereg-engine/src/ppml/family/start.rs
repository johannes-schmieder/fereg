//! Stable global/one-effect starts and the offset-free reference null likelihood.
use super::{invalid, numerical, Family};
use crate::{
    error::{reserved, Result},
    threads::{compensated_sum, CompensatedSum, ThreadTeam},
    topology::FeTopology,
};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NullObjective {
    pub mean: f64,
    pub log_likelihood: f64,
}
impl Family<'_> {
    pub fn global_start(&self) -> Result<f64> {
        self.start_for(0..self.outcome.len())?
            .ok_or_else(|| invalid("global start has no positive outcome"))
    }
    /// An optional proposal only: zero-outcome groups decline the analytic start.
    /// The controller retains its finite global state until a proposal is accepted.
    pub fn one_fe_start(
        &self,
        topology: &FeTopology,
        team: &ThreadTeam,
    ) -> Result<Option<Vec<f64>>> {
        if topology.nobs() != self.outcome.len() {
            return Err(invalid("start topology and sample differ"));
        }
        if topology.effects().len() != 1
            || !topology.effects()[0].intercept
            || !topology.effects()[0].slopes.is_empty()
        {
            return Ok(None);
        }
        let partition = topology.effect_partition(0)?;
        let proposed = team.map_ranges(partition.n_levels(), 1, |range| {
            Some(
                self.start_for(
                    partition
                        .membership()
                        .members(range.start)
                        .expect("validated level")
                        .iter()
                        .copied(),
                ),
            )
        })?;
        let mut coefficients = reserved(partition.n_levels())?;
        for value in proposed {
            match value.expect("visited start level")? {
                Some(value) => coefficients.push(value),
                None => return Ok(None),
            }
        }
        Ok(Some(coefficients))
    }
    fn start_for(&self, rows: impl Iterator<Item = usize> + Clone) -> Result<Option<f64>> {
        let numerator = self.log_outcome_sum(rows.clone());
        if numerator == f64::NEG_INFINITY {
            return Ok(None);
        }
        let denominator = log_sum(rows, |row| {
            self.prior.value(row).ln() + self.offset.map_or(0., |offset| offset[row])
        });
        let value = numerator - denominator;
        if value.is_finite() {
            Ok(Some(value))
        } else {
            Err(numerical("Poisson start is not representable"))
        }
    }
    fn log_outcome_sum(&self, rows: impl Iterator<Item = usize> + Clone) -> f64 {
        log_sum(rows, |row| {
            if self.outcome[row] > 0. {
                self.prior.value(row).ln() + self.terms[row][0]
            } else {
                f64::NEG_INFINITY
            }
        })
    }
    pub fn null_objective(&self) -> Result<NullObjective> {
        let (mean, log_mean) = if let Some(mean) = self.scaled_null_mean() {
            (mean, mean.ln())
        } else {
            let log_mean = self.log_outcome_sum(0..self.outcome.len())
                - log_sum(0..self.outcome.len(), |row| self.prior.value(row).ln());
            (log_mean.exp(), log_mean)
        };
        if !mean.is_finite() || mean <= 0. || !log_mean.is_finite() {
            return Err(numerical("Poisson null mean is not representable"));
        }
        let mut likelihood = CompensatedSum::default();
        for row in 0..self.outcome.len() {
            let term =
                self.prior.value(row) * (self.outcome[row] * log_mean - mean - self.terms[row][1]);
            if !term.is_finite() {
                return Err(numerical("Poisson null likelihood is not representable"));
            }
            likelihood.add(term);
        }
        let log_likelihood = likelihood.total();
        if !log_likelihood.is_finite() {
            return Err(numerical("Poisson null likelihood is not representable"));
        }
        Ok(NullObjective {
            mean,
            log_likelihood,
        })
    }
    // Harvested positive-mean safeguard: do not lose a subnormal intermediate
    // whose magnitude can matter again after restoring the outcome scale.
    fn scaled_null_mean(&self) -> Option<f64> {
        let max_q = (0..self.outcome.len())
            .map(|row| self.prior.value(row))
            .fold(0., f64::max);
        let max_y = self.outcome.iter().copied().fold(0., f64::max);
        let mut weights = CompensatedSum::default();
        let mut outcomes = CompensatedSum::default();
        for (row, &y) in self.outcome.iter().enumerate() {
            let q = self.prior.value(row) / max_q;
            if !q.is_normal() {
                return None;
            }
            weights.add(q);
            if y > 0. {
                let scaled = y / max_y;
                let product = q * scaled;
                if !scaled.is_normal() || !product.is_normal() {
                    return None;
                }
                outcomes.add(product);
            }
        }
        let ratio = outcomes.total() / weights.total();
        let mean = max_y * ratio;
        (ratio.is_normal() && mean.is_normal()).then_some(mean)
    }
}
fn log_sum(rows: impl Iterator<Item = usize> + Clone, value: impl Fn(usize) -> f64) -> f64 {
    let maximum = rows.clone().map(&value).fold(f64::NEG_INFINITY, f64::max);
    if maximum == f64::NEG_INFINITY {
        return maximum;
    }
    maximum + compensated_sum(rows.map(|row| (value(row) - maximum).exp())).ln()
}
