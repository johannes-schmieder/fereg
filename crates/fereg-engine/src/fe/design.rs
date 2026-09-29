//! Matrix-free D and D'W over the one prepared topology and current weight frame.
use crate::{
    data::{DenseMatrix, Prepared},
    error::{reserved, ErrorCode, FeregError, Result},
    threads::{compensated_sum, ThreadTeam},
    topology::FeTopology,
};
use rayon::prelude::*;
#[derive(Debug, Clone, Copy)]
pub enum Weights<'a> {
    Unit,
    Provided(&'a [f64]),
}
impl Weights<'_> {
    #[must_use]
    pub fn at(self, row: usize) -> f64 {
        match self {
            Self::Unit => 1.,
            Self::Provided(values) => values[row],
        }
    }
    fn validate(self, nobs: usize) -> Result<()> {
        if let Self::Provided(values) = self {
            if values.len() != nobs {
                return Err(invalid("FE weights and observations differ"));
            }
            if values
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.)
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "FE weights must be positive and finite",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectLayout {
    pub offset: usize,
    pub levels: usize,
    pub channels: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoefficientLayout {
    pub effects: Vec<EffectLayout>,
    pub total: usize,
}
#[derive(Debug)]
pub struct Design<'a> {
    sample: &'a Prepared,
    topology: &'a FeTopology,
    weights: Weights<'a>,
    layout: CoefficientLayout,
}
impl<'a> Design<'a> {
    pub fn new(sample: &'a Prepared, weights: Weights<'a>) -> Result<Self> {
        sample.validate_alignment()?;
        let topology = sample
            .topology
            .as_ref()
            .ok_or_else(|| invalid("FE design requires prepared topology"))?;
        topology.validate(sample)?;
        weights.validate(sample.n_active())?;
        let mut effects = reserved(topology.effects().len())?;
        let mut total = 0usize;
        for (index, effect) in topology.effects().iter().enumerate() {
            let levels = topology.effect_partition(index)?.n_levels();
            let channels = usize::from(effect.intercept)
                .checked_add(effect.slopes.len())
                .ok_or_else(allocation)?;
            effects.push(EffectLayout {
                offset: total,
                levels,
                channels,
            });
            total = total
                .checked_add(levels.checked_mul(channels).ok_or_else(allocation)?)
                .ok_or_else(allocation)?;
        }
        Ok(Self {
            sample,
            topology,
            weights,
            layout: CoefficientLayout { effects, total },
        })
    }
    #[must_use]
    pub fn sample(&self) -> &'a Prepared {
        self.sample
    }
    #[must_use]
    pub fn nobs(&self) -> usize {
        self.sample.n_active()
    }
    #[must_use]
    pub fn topology(&self) -> &FeTopology {
        self.topology
    }
    #[must_use]
    pub fn weights(&self) -> Weights<'a> {
        self.weights
    }
    #[must_use]
    pub fn layout(&self) -> &CoefficientLayout {
        &self.layout
    }
    pub fn coefficient_index(&self, effect: usize, channel: usize, level: usize) -> Result<usize> {
        let layout = self
            .layout
            .effects
            .get(effect)
            .ok_or_else(|| invalid("FE coefficient effect is out of range"))?;
        if channel >= layout.channels || level >= layout.levels {
            return Err(invalid("FE coefficient channel or level is out of range"));
        }
        Ok(layout.offset + channel * layout.levels + level)
    }
    pub fn basis_value(&self, effect: usize, channel: usize, row: usize) -> Result<f64> {
        if row >= self.nobs()
            || self
                .layout
                .effects
                .get(effect)
                .is_none_or(|layout| channel >= layout.channels)
        {
            return Err(invalid("FE basis index is out of range"));
        }
        Ok(self.basis(effect, channel, row))
    }
    fn basis(&self, effect: usize, channel: usize, row: usize) -> f64 {
        let term = &self.topology.effects()[effect];
        if term.intercept && channel == 0 {
            1.
        } else {
            let source = term.slopes[channel - usize::from(term.intercept)]
                .source
                .get() as usize
                - 1;
            self.sample.regressors[source][row]
        }
    }
    /// Each output row has a fixed accumulation order independent of worker count.
    pub fn multiply(&self, coefficients: &DenseMatrix, team: &ThreadTeam) -> Result<DenseMatrix> {
        validate_matrix(coefficients, self.layout.total)?;
        let rhs = coefficients.ncols();
        let mut result = DenseMatrix::zeros(self.nobs(), rhs)?;
        team.install(|| {
            result
                .as_mut_slice()
                .par_chunks_mut(rhs.max(1))
                .enumerate()
                .for_each(|(row, values)| {
                    for (column, value) in values.iter_mut().enumerate() {
                        *value = compensated_sum(self.layout.effects.iter().enumerate().flat_map(
                            |(effect, layout)| {
                                let level =
                                    self.topology
                                        .effect_partition(effect)
                                        .expect("validated effect")
                                        .codes()[row] as usize;
                                (0..layout.channels).map(move |channel| {
                                    self.basis(effect, channel, row)
                                        * coefficients[(
                                            layout.offset + channel * layout.levels + level,
                                            column,
                                        )]
                                })
                            },
                        ));
                    }
                })
        });
        finite_result(result)
    }
    /// CSR rows are canonical fixed partitions; workers never change a coefficient sum's order.
    pub fn transpose_weighted(
        &self,
        values: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<DenseMatrix> {
        validate_matrix(values, self.nobs())?;
        let rhs = values.ncols();
        let mut result = DenseMatrix::zeros(self.layout.total, rhs)?;
        team.install(|| {
            result
                .as_mut_slice()
                .par_chunks_mut(rhs.max(1))
                .enumerate()
                .for_each(|(coefficient, output)| {
                    let effect = self
                        .layout
                        .effects
                        .partition_point(|layout| layout.offset <= coefficient)
                        - 1;
                    let layout = &self.layout.effects[effect];
                    let local = coefficient - layout.offset;
                    let channel = local / layout.levels;
                    let level = local % layout.levels;
                    let members = self
                        .topology
                        .effect_partition(effect)
                        .expect("validated effect")
                        .membership()
                        .members(level)
                        .expect("validated level");
                    for (column, value) in output.iter_mut().enumerate() {
                        *value = compensated_sum(members.iter().map(|&row| {
                            self.weights.at(row)
                                * self.basis(effect, channel, row)
                                * values[(row, column)]
                        }));
                    }
                })
        });
        finite_result(result)
    }
    pub fn level_gram(&self, effect: usize, level: usize) -> Result<DenseMatrix> {
        let layout = self
            .layout
            .effects
            .get(effect)
            .ok_or_else(|| invalid("FE block effect is out of range"))?;
        let members = self
            .topology
            .effect_partition(effect)?
            .membership()
            .members(level)?;
        let mut block = DenseMatrix::zeros(layout.channels, layout.channels)?;
        for left in 0..layout.channels {
            for right in 0..=left {
                let value = compensated_sum(members.iter().map(|&row| {
                    self.weights.at(row)
                        * self.basis(effect, left, row)
                        * self.basis(effect, right, row)
                }));
                block[(left, right)] = value;
                block[(right, left)] = value;
            }
        }
        finite_result(block)
    }
    pub fn reconstruct(
        &self,
        rhs: &DenseMatrix,
        coefficients: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<DenseMatrix> {
        validate_matrix(rhs, self.nobs())?;
        if rhs.ncols() != coefficients.ncols() {
            return Err(invalid("FE coefficient and observation RHS counts differ"));
        }
        let mut residual = self.multiply(coefficients, team)?;
        for (value, &original) in residual.as_mut_slice().iter_mut().zip(rhs.as_slice()) {
            *value = original - *value;
        }
        finite_result(residual)
    }
}
pub(super) fn validate_matrix(matrix: &DenseMatrix, rows: usize) -> Result<()> {
    if matrix.nrows() != rows {
        return Err(invalid("FE matrix has an incompatible row count"));
    }
    if matrix.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "FE matrix contains nonfinite values",
        ));
    }
    Ok(())
}
fn finite_result(matrix: DenseMatrix) -> Result<DenseMatrix> {
    if matrix.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "FE operator result is not representable",
        ));
    }
    Ok(matrix)
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn allocation() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "FE coefficient dimensions overflow")
}
