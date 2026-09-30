//! Q=1 coefficient candidate; acceptance belongs to the shared FE certifier.
use super::Design;
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
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
        self.direct_candidate_polled(rhs, team, None)
    }
    pub(super) fn direct_candidate_polled(
        &self,
        rhs: &DenseMatrix,
        team: &ThreadTeam,
        poll: Poll<'_>,
    ) -> Result<DirectCandidate> {
        execution::check(poll)?;
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
                    *result = Some(self.solve_level(rhs, level, poll));
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
    fn solve_level(
        &self,
        rhs: &DenseMatrix,
        level: usize,
        poll: Poll<'_>,
    ) -> Result<(DenseMatrix, usize)> {
        let factor = super::blocks::factor_level(self, 0, level, poll)?;
        let block = super::blocks::solve_level(self, 0, level, &factor, rhs, poll)?;
        Ok((block, factor.rank()))
    }
}
