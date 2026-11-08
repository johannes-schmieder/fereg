//! The exact global elimination of the largest effect as the first route of
//! an Auto OLS session.

use super::FeSession;
use crate::error::Result;
use crate::fe::{BorrowedMapWeights, CertifiedBorrowedMap};

pub(super) fn block_schur_enabled() -> bool {
    crate::fe::borrowed_map::block_schur::enabled()
}

impl FeSession<'_> {
    /// Exact global elimination of the largest effect for Auto OLS with at
    /// least two categorical intercept effects; a decline returns the input
    /// for the session's established route.
    pub(super) fn try_block_schur(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: crate::ols::OlsProjectionInput,
        config: crate::fe::AbsorbConfig,
    ) -> Result<std::result::Result<CertifiedBorrowedMap, crate::ols::OlsProjectionInput>> {
        use crate::fe::borrowed_map::block_schur::Outcome;
        let default_routing = config.routing.is_none_or(|preset| {
            preset == crate::fe::RoutingPreset::geometry_many()
                || preset == crate::fe::RoutingPreset::gates_two_v2()
        });
        let eligible = block_schur_enabled()
            && config.solver == crate::fe::FeSolverKind::Auto
            && config.lsmr.preconditioner == crate::fe::LsmrPreconditioner::Auto
            && config.declined_recheck.is_none()
            && default_routing
            && self.cmg.is_none()
            && self.lsmr.is_none()
            && self.general.is_none()
            && self.owned.is_none();
        if !eligible {
            return Ok(Err(values));
        }
        let design = match (&self.categorical, &self.binding) {
            (Some(owner), _) => owner.borrowed_design(),
            (None, Some(design)) if design.effects().len() >= 2 => design,
            _ => return Ok(Err(values)),
        };
        let matrix = values.into_values();
        match self.block_schur.project(
            design,
            weights,
            matrix,
            config.map,
            config.solver_memory_budget,
            crate::fe::borrowed_map::block_schur::Request {
                retain_coefficients: config.retain_coefficients,
                // Weighted projections (PPML separation frames) also bound
                // coefficient errors; unit-weight OLS keeps the moment rule.
                coefficient_accuracy: !matches!(weights, BorrowedMapWeights::Unit),
            },
        )? {
            Outcome::Done(result) => {
                self.block_schur_used = true;
                Ok(Ok(result))
            }
            Outcome::Declined(matrix) => Ok(Err(matrix.into())),
        }
    }
}
