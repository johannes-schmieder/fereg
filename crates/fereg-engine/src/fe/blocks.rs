//! One numerical factor per declared level, bound to an immutable weight frame.
use super::Design;
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    linalg::WeightedQr,
    threads::ThreadTeam,
};
use rayon::prelude::*;

pub(super) struct Blocks {
    factors: Vec<Vec<WeightedQr>>,
}
impl Blocks {
    pub(super) fn new(design: &Design<'_>, team: &ThreadTeam) -> Result<Self> {
        let mut factors = reserved(design.layout().effects.len())?;
        for effect in 0..design.layout().effects.len() {
            let levels = design.layout().effects[effect].levels;
            let mut results = reserved(levels)?;
            results.resize_with(levels, || None);
            team.install(|| {
                results
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(level, result)| {
                        *result = Some(factor_level(design, effect, level));
                    })
            });
            let mut blocks = reserved(levels)?;
            for result in results {
                blocks.push(result.expect("visited level")?);
            }
            factors.push(blocks);
        }
        Ok(Self { factors })
    }
    pub(super) fn observation_solve(
        &self,
        design: &Design<'_>,
        effect: usize,
        rhs: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<DenseMatrix> {
        self.apply(design, rhs.ncols(), team, |e, level, factor| {
            if e != effect {
                return Ok(None);
            }
            Ok(Some(solve_level(design, e, level, factor, rhs)?))
        })
    }
    pub(super) fn diagonal_solve(
        &self,
        design: &Design<'_>,
        excluded: usize,
        rhs: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<DenseMatrix> {
        self.apply(design, rhs.ncols(), team, |effect, level, factor| {
            if effect == excluded {
                return Ok(None);
            }
            let mut block =
                DenseMatrix::zeros(design.layout().effects[effect].channels, rhs.ncols())?;
            for channel in 0..block.nrows() {
                let coefficient = design.coefficient_index(effect, channel, level)?;
                for column in 0..rhs.ncols() {
                    block[(channel, column)] = rhs[(coefficient, column)];
                }
            }
            Ok(Some(factor.solve_normal(&block)?))
        })
    }
    fn apply<F>(
        &self,
        design: &Design<'_>,
        rhs_count: usize,
        team: &ThreadTeam,
        operation: F,
    ) -> Result<DenseMatrix>
    where
        F: Fn(usize, usize, &WeightedQr) -> Result<Option<DenseMatrix>> + Sync,
    {
        let mut output = DenseMatrix::zeros(design.layout().total, rhs_count)?;
        for (effect, factors) in self.factors.iter().enumerate() {
            let mut results = reserved(factors.len())?;
            results.resize_with(factors.len(), || None);
            team.install(|| {
                results.par_iter_mut().zip(factors).enumerate().for_each(
                    |(level, (result, factor))| {
                        *result = Some(operation(effect, level, factor));
                    },
                )
            });
            for (level, result) in results.into_iter().enumerate() {
                if let Some(block) = result.expect("visited level")? {
                    for channel in 0..block.nrows() {
                        let coefficient = design.coefficient_index(effect, channel, level)?;
                        for column in 0..rhs_count {
                            output[(coefficient, column)] = block[(channel, column)];
                        }
                    }
                }
            }
        }
        Ok(output)
    }
}
pub(super) fn factor_level(design: &Design<'_>, effect: usize, level: usize) -> Result<WeightedQr> {
    let members = design
        .topology()
        .effect_partition(effect)?
        .membership()
        .members(level)?;
    let channels = design.layout().effects[effect].channels;
    let mut matrix = DenseMatrix::zeros(members.len(), channels)?;
    let mut weights = reserved(members.len())?;
    for (local, &row) in members.iter().enumerate() {
        weights.push(design.weights().at(row));
        for channel in 0..channels {
            matrix[(local, channel)] = design.basis_value(effect, channel, row)?;
        }
    }
    let threshold = 8. * f64::EPSILON * members.len().max(channels).max(1) as f64;
    WeightedQr::factor(matrix, &weights, threshold)
}
pub(super) fn solve_level(
    design: &Design<'_>,
    effect: usize,
    level: usize,
    factor: &WeightedQr,
    rhs: &DenseMatrix,
) -> Result<DenseMatrix> {
    let members = design
        .topology()
        .effect_partition(effect)?
        .membership()
        .members(level)?;
    let mut block = DenseMatrix::zeros(members.len(), rhs.ncols())?;
    for (local, &row) in members.iter().enumerate() {
        for column in 0..rhs.ncols() {
            block[(local, column)] = rhs[(row, column)];
        }
    }
    factor.solve(block)
}
