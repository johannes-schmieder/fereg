//! Q=1 coefficient candidate; acceptance belongs to the shared FE certifier.
use super::Design;
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    linalg::WeightedQr,
    threads::ThreadTeam,
};
use rayon::prelude::*;

/// An explicitly unaccepted numerical candidate, never an estimation result.
#[derive(Debug, PartialEq)]
pub struct DirectCandidate {
    pub coefficients: DenseMatrix,
    pub residuals: DenseMatrix,
    pub level_ranks: Vec<usize>,
}
impl Design<'_> {
    /// Solve disjoint weighted level spans. No ridge or rank is manufactured for
    /// duplicate/constant/zero slope channels. All RHSs share each factor.
    pub fn direct_candidate(
        &self,
        rhs: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<DirectCandidate> {
        super::design::validate_matrix(rhs, self.nobs())?;
        if self.layout().effects.len() > 1 {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "direct FE projection requires at most one effect",
            ));
        }
        let mut coefficients = DenseMatrix::zeros(self.layout().total, rhs.ncols())?;
        let levels = self
            .layout()
            .effects
            .first()
            .map_or(0, |effect| effect.levels);
        let mut results = reserved(levels)?;
        results.resize_with(levels, || None);
        team.install(|| {
            results
                .par_iter_mut()
                .enumerate()
                .for_each(|(level, result)| {
                    *result = Some(self.solve_level(rhs, level));
                })
        });
        let mut level_ranks = reserved(levels)?;
        for (level, result) in results.into_iter().enumerate() {
            let (block, rank) = result.expect("every level is visited")?;
            level_ranks.push(rank);
            for channel in 0..block.nrows() {
                let coefficient = self.coefficient_index(0, channel, level)?;
                for column in 0..rhs.ncols() {
                    coefficients[(coefficient, column)] = block[(channel, column)];
                }
            }
        }
        let residuals = self.reconstruct(rhs, &coefficients, team)?;
        Ok(DirectCandidate {
            coefficients,
            residuals,
            level_ranks,
        })
    }
    fn solve_level(&self, rhs: &DenseMatrix, level: usize) -> Result<(DenseMatrix, usize)> {
        let members = self
            .topology()
            .effect_partition(0)?
            .membership()
            .members(level)?;
        let channels = self.layout().effects[0].channels;
        let mut matrix = DenseMatrix::zeros(members.len(), channels)?;
        let mut local_rhs = DenseMatrix::zeros(members.len(), rhs.ncols())?;
        let mut weights = reserved(members.len())?;
        for (local, &row) in members.iter().enumerate() {
            weights.push(self.weights().at(row));
            for channel in 0..channels {
                matrix[(local, channel)] = self.basis_value(0, channel, row)?;
            }
            for column in 0..rhs.ncols() {
                local_rhs[(local, column)] = rhs[(row, column)];
            }
        }
        // Matches the local-span rank scale; independent of solver convergence tolerance.
        let threshold = 8. * f64::EPSILON * members.len().max(channels).max(1) as f64;
        let factor = WeightedQr::factor(matrix, &weights, threshold)?;
        Ok((factor.solve(local_rhs)?, factor.rank()))
    }
}
