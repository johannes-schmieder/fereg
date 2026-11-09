//! fedemean's routes of an OLS session on its categorical intercept binding:
//! Auto's exact elimination of the largest effect and full system, and the
//! explicit modified LSMR and CG requests.

use super::FeSession;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::borrowed_map::block_schur::{Cache, Decline, Outcome, Plan, Request};
use crate::fe::{BorrowedMapWeights, CertifiedBorrowedMap, FeSolverKind, MapConfig};

pub(super) fn block_schur_enabled() -> bool {
    crate::fe::borrowed_map::block_schur::enabled()
}

/// The elimination's range of effect counts (fedemean's provisional limit).
const ELIMINATED_EFFECTS: std::ops::RangeInclusive<usize> = 2..=5;

// Tests: Auto's full system gets one iteration, so it declines.
crate::fault::fault_plan! {
    static STARVE_FULL_SYSTEM: bool = false;
}

#[cfg(test)]
pub(crate) fn starve_full_system_for_test(on: bool) {
    STARVE_FULL_SYSTEM.set(on);
}

/// The established route's input after fedemean declined, with the note the
/// fallback's report carries when fedemean's full system declined.
type Declined = (crate::ols::OlsProjectionInput, Option<String>);

impl<'a> FeSession<'a> {
    /// Auto OLS with at least two categorical intercept effects: the exact
    /// elimination of the largest effect (two to five effects), then, for
    /// other effect counts and the elimination's declines, fedemean's full
    /// system (Jacobi PCG escalating to additive Schwarz when slow or
    /// stalled) within half the iteration budget. A decline of both returns the input
    /// for the session's established route; a full-system decline is
    /// reported as its fallback.
    pub(super) fn try_block_schur(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: crate::ols::OlsProjectionInput,
        config: crate::fe::AbsorbConfig,
    ) -> Result<std::result::Result<CertifiedBorrowedMap, Declined>> {
        let default_routing = config.routing.is_none_or(|preset| {
            preset == crate::fe::RoutingPreset::geometry_many()
                || preset == crate::fe::RoutingPreset::gates_two_v2()
        });
        let eligible = block_schur_enabled()
            && config.solver == FeSolverKind::Auto
            && config.lsmr.preconditioner == crate::fe::LsmrPreconditioner::Auto
            && config.declined_recheck.is_none()
            && default_routing
            && self.cmg.is_none()
            && self.explicit.is_none()
            && self.general.is_none()
            && self.owned.is_none();
        if !eligible {
            return Ok(Err((values, None)));
        }
        let design = match (&self.categorical, &self.binding) {
            (Some(owner), _) => owner.borrowed_design(),
            (None, Some(design)) if design.effects().len() >= 2 => design,
            _ => return Ok(Err((values, None))),
        };
        let request = Request {
            retain_coefficients: config.retain_coefficients,
            // Weighted projections (PPML separation frames) also bound
            // coefficient errors; unit-weight OLS keeps the moment rule.
            coefficient_accuracy: !matches!(weights, BorrowedMapWeights::Unit),
        };
        let budget = config.solver_memory_budget;
        let mut matrix = values.into_values();
        if ELIMINATED_EFFECTS.contains(&design.effects().len()) {
            match self
                .block_schur
                .project(design, weights, matrix, config.map, budget, request)?
            {
                Outcome::Done(result) => {
                    self.block_schur_used = true;
                    return Ok(Ok(result));
                }
                Outcome::Declined(declined) => matrix = declined,
            }
        }
        // Half the budget, so a decline leaves the established route the rest.
        let map = MapConfig {
            max_iterations: if STARVE_FULL_SYSTEM.get() {
                1
            } else {
                (config.map.max_iterations / 2).max(1)
            },
            ..config.map
        };
        match self
            .full_system
            .project(design, weights, matrix, map, budget, request)?
        {
            Outcome::Done(result) => {
                self.full_system_used = true;
                self.block_schur
                    .add_partial_coefficients(Ok(result))
                    .map(Ok)
            }
            Outcome::Declined(declined) => {
                let note = self.full_system.decline().map(|decline| {
                    format!(
                        "the full-system CG declined after {} iterations ({}); the established route used the remaining budget",
                        decline.iterations,
                        decline.describe()
                    )
                });
                Ok(Err((declined.into(), note)))
            }
        }
    }

    /// PPML's canonical projections of a categorical intercept design that
    /// the generic owner serves (outside the elimination's range): fedemean's
    /// full system first (Jacobi PCG escalating to additive Schwarz when slow
    /// or stalled), within half the iteration budget and `budget`. The
    /// certified result keeps the MAP-family engine identity with the route
    /// it used; a decline returns the input for the generic owner.
    pub(crate) fn try_ppml_full_system(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: crate::data::DenseMatrix,
        map: MapConfig,
    ) -> Result<std::result::Result<CertifiedBorrowedMap, crate::data::DenseMatrix>> {
        let (Some(budget), Some(design)) = (self.ppml_full_system, self.binding.as_ref()) else {
            return Ok(Err(values));
        };
        if !block_schur_enabled() || self.categorical.is_some() || self.general.is_none() {
            return Ok(Err(values));
        }
        let request = Request {
            retain_coefficients: false,
            // PPML frames also bound the coefficient error (their fitted
            // predictors are checked row by row).
            coefficient_accuracy: true,
        };
        let map = MapConfig {
            max_iterations: if STARVE_FULL_SYSTEM.get() {
                1
            } else {
                (map.max_iterations / 2).max(1)
            },
            ..map
        };
        match self
            .full_system
            .project(design, weights, values, map, budget, request)?
        {
            Outcome::Done(result) => {
                self.full_system_used = true;
                Ok(Ok(result.into_map_family()))
            }
            Outcome::Declined(values) => Ok(Err(values)),
        }
    }

    /// PPML's coefficient-owner projection with a norm handoff: fedemean's
    /// full system first (its result leaves the norms to the caller, `false`),
    /// else the generic owner's (`true`).
    pub(crate) fn absorb_ppml_coefficients(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        (normalized, values): (&[f64], crate::data::DenseMatrix),
        map: MapConfig,
        generation: crate::fe::MapGeometryGeneration,
        (positions, norms): (std::ops::Range<usize>, &mut [f64]),
    ) -> Result<(CertifiedBorrowedMap, bool)> {
        let values = match self.try_ppml_full_system(weights, values, map)? {
            Ok(result) => return Ok((result, false)),
            Err(values) => values,
        };
        let result = self
            .general_owner()?
            .absorb_coefficient_with_selected_norms(
                normalized, values, map, generation, positions, None, norms,
            )?;
        Ok((result, true))
    }

    /// PPML's certification (and, when it misses, correction) of an assembled
    /// residual column: fedemean's full system first, else the generic owner.
    pub(crate) fn absorb_ppml_coefficient_residual(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        (normalized, values): (&[f64], crate::data::DenseMatrix),
        map: MapConfig,
        generation: crate::fe::MapGeometryGeneration,
    ) -> Result<CertifiedBorrowedMap> {
        let values = match self.try_ppml_full_system(weights, values, map)? {
            Ok(result) => return Ok(result),
            Err(values) => values,
        };
        self.general_owner()?
            .absorb_coefficient_residual(normalized, values, map, generation)
    }

    fn general_owner(&mut self) -> Result<&mut crate::fe::ReusableMapAbsorber> {
        self.general
            .as_mut()
            .ok_or_else(|| FeregError::new(ErrorCode::Internal, "coefficient owner is absent"))
    }

    /// Bind PPML's canonical generic owner to its categorical intercept
    /// design, so that fedemean's full system serves it first within
    /// `budget` ([`FeSession::try_ppml_full_system`]).
    pub(crate) fn bind_ppml_full_system(
        &mut self,
        design: crate::fe::BorrowedInterceptFeDesign<'a>,
        budget: Option<crate::memory::FeSolverMemoryBudget>,
    ) {
        self.binding = Some(design);
        self.ppml_full_system = Some(budget);
    }

    /// An explicit fedemean request on the binding. It has no fallback: a
    /// decline is the request's error.
    pub(super) fn project_explicit(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: crate::ols::OlsProjectionInput,
        config: crate::fe::AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let explicit = self.explicit.as_mut().expect("explicit fedemean request");
        let design = self
            .binding
            .as_ref()
            .ok_or_else(|| crate::error::invalid_argument("explicit FE binding is absent"))?;
        let lsmr = config.solver == FeSolverKind::Lsmr;
        // Explicit LSMR keeps its own tolerance and budget (the final
        // residual's tighter tolerance reaches both).
        let map = if lsmr {
            MapConfig {
                tolerance: config.lsmr.tolerance,
                max_iterations: config.lsmr.max_iterations,
                ..config.map
            }
        } else {
            config.map
        };
        let request = Request {
            retain_coefficients: config.retain_coefficients,
            coefficient_accuracy: false,
        };
        let matrix = values.into_values();
        match explicit.project(
            design,
            weights,
            matrix,
            map,
            config.solver_memory_budget,
            request,
        )? {
            Outcome::Done(result) => Ok(result),
            Outcome::Declined(_) => Err(explicit_decline(
                explicit.decline(),
                if lsmr { "solver(lsmr)" } else { "solver(cg)" },
                design.effects().len(),
            )),
        }
    }

    /// Drop an explicit request's retained projector (before the fit's QR
    /// and variance stages); a later projection rebuilds it.
    pub(crate) fn release_explicit(&mut self) {
        if let Some(explicit) = self.explicit.as_mut() {
            explicit.release();
        }
    }
}

/// The fedemean cache of an explicit `solver(lsmr)` or (non-CMG)
/// `solver(cg)` request.
pub(super) fn explicit_cache<'a>(config: crate::fe::AbsorbConfig) -> Result<Cache<'a>> {
    let plan = if config.solver == FeSolverKind::Lsmr {
        Plan::lsmr(config.lsmr.preconditioner)
    } else {
        Plan::pcg(config.pcg)?
    };
    Ok(Cache::with_plan(plan))
}

/// The error of an explicit request that fedemean declined.
fn explicit_decline(decline: Option<Decline>, request: &str, effects: usize) -> FeregError {
    let Some(decline) = decline else {
        return FeregError::new(
            ErrorCode::Internal,
            format!("{request} declined without a reason"),
        );
    };
    let (code, message) = match decline.reason {
        fedemean::Decline::MemoryLimit => (
            ErrorCode::Allocation,
            format!("{request}: the solver workspace exceeds the memory limit"),
        ),
        fedemean::Decline::OutsideRoute => (
            ErrorCode::Unsupported,
            format!(
                "{request}: reduce(eliminate) needs two to five categorical fixed effects (this design has {effects})"
            ),
        ),
        fedemean::Decline::Numerical(reason) => (
            ErrorCode::NonConvergence,
            format!(
                "{request} did not converge after {} iterations ({reason}); raise iterate() or choose another preconditioner",
                decline.iterations
            ),
        ),
        _ => (
            ErrorCode::InvalidData,
            format!("{request}: {}", decline.describe()),
        ),
    };
    FeregError::new(code, message)
}
