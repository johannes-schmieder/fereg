//! The general owned designs: OLS Auto on slope designs (fedemean's full
//! system first, the generic MAP owner after a decline), explicit requests,
//! and the general saved-effect recovery tiers (fedemean's full system for
//! slope designs, then MAP).

use super::{FeSession, RecoveryRequest};
use crate::data::DenseMatrix;
use crate::error::{allocation, ErrorCode, FeregError, Result};
use crate::fe::{BorrowedMapWeights, CertifiedBorrowedMap, MapConfig, MapGeometryGeneration};

impl FeSession<'_> {
    /// An explicit request on the session's owned design. Explicit modified
    /// LSMR runs fedemean's full system (and returns its coefficients when
    /// retained), with the established within and MAP routes for a fit it
    /// does not certify; the other explicit requests keep their routes.
    pub(super) fn project_owned(
        &mut self,
        mut values: DenseMatrix,
        config: crate::fe::AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let design = self.owned.as_ref().expect("owned FE binding");
        let (mut report, coefficients) = if config.solver == crate::fe::FeSolverKind::Lsmr {
            crate::fe::auto::absorb_explicit_lsmr(design, &self.normalized, &mut values, config)?
        } else {
            (
                crate::fe::absorb_fixed_effects(design, &self.normalized, &mut values, config)?,
                None,
            )
        };
        report.solver_workspace_peak = report
            .solver_workspace_peak
            .with_retained(self.resident_bytes)?
            .max(self.peak);
        match coefficients {
            Some(coefficients) => CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                values,
                report,
                coefficients,
            ),
            None => CertifiedBorrowedMap::from_certified_parts(values, report),
        }
    }

    /// OLS Auto on an owned slope design: fedemean's full system (PCG with
    /// the levels' channel blocks, escalating to the hybrid Schwarz when slow
    /// or stalled) within half the iteration budget, certified by the
    /// generic owner's independent certificate; a decline (or a residual
    /// that misses after one correction) continues with the owner's MAP,
    /// reported as the fallback. Retained coefficients come from fedemean's
    /// certified projections only.
    pub(super) fn project_full_general(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: crate::fe::AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let generation = MapGeometryGeneration {
            sample: 0,
            topology: 0,
            weights: 0,
        };
        let peak = self.peak;
        let p = self
            .general
            .as_mut()
            .ok_or_else(|| FeregError::new(ErrorCode::Internal, "generic FE owner is absent"))?;
        let layout = config
            .retain_coefficients
            .then(|| crate::fe::fedemean_slope::Layout::new(p.design()))
            .transpose()?;
        let mut coefficients = layout
            .as_ref()
            .map(|layout| layout.zeros(values.ncols()))
            .transpose()?;
        let rhs = values.ncols();
        let normalized = &self.normalized;
        let unit = matches!(weights, BorrowedMapWeights::Unit);
        let budget = (config.map.max_iterations / 2).max(1);
        // The coefficient bank coexists with fedemean's workspace.
        let held = layout.as_ref().map_or(0, |layout| layout.bytes(rhs));
        let memory_limit = config.solver_memory_budget.map(|b| {
            b.usable_bytes
                .saturating_sub(b.execution_bytes)
                .saturating_sub(peak.bytes().map_or(0, u128::from))
                .saturating_sub(held)
        });
        let mut workspace = crate::memory::SolverWorkspacePeak::ZERO;
        let mut solve = |design: &crate::fe::FeDesign,
                         values: &mut DenseMatrix,
                         tolerance: f64,
                         spent: usize|
         -> Result<crate::fe::map::ExternalProjection> {
            let attempt = crate::fe::fedemean_slope::Attempt::auto(
                tolerance,
                budget.saturating_sub(spent).max(1),
                memory_limit,
            );
            let retained = layout.as_ref().zip(coefficients.as_deref_mut());
            Ok(
                match crate::fe::fedemean_slope::project(
                    design,
                    (normalized, unit),
                    values,
                    &attempt,
                    retained,
                )? {
                    crate::fe::fedemean_slope::Outcome::Projected(report) => {
                        workspace = workspace.max(report.solver_workspace_peak);
                        crate::fe::map::ExternalProjection::Projected(report)
                    }
                    crate::fe::fedemean_slope::Outcome::Declined {
                        reason, iterations, ..
                    } => crate::fe::map::ExternalProjection::Declined {
                        iterations,
                        reason: crate::fe::fedemean_slope::describe(reason),
                    },
                    crate::fe::fedemean_slope::Outcome::Ineligible(_) => {
                        crate::fe::map::ExternalProjection::Ineligible
                    }
                },
            )
        };
        let (result, fallback) = match p.absorb_external(
            normalized,
            values,
            config.map,
            Some(generation),
            &mut solve,
        )? {
            crate::fe::map::External::Done(result, fallback) => (result, fallback),
            // fedemean does not take this design (for example an effect
            // with more channels than it supports): the owner's MAP, as
            // before fedemean served slopes, for this and later calls.
            crate::fe::map::External::Ineligible(values) => {
                self.full_general = false;
                return p.absorb_in_generation(
                    normalized,
                    values,
                    config.map,
                    false,
                    Some(generation),
                );
            }
        };
        let (values, mut report) = result.into_parts();
        report.solver_workspace_peak = workspace.with_retained(held)?.with_retained_peak(peak)?;
        match (fallback, coefficients, layout) {
            (false, Some(coefficients), Some(layout)) => {
                CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                    values,
                    report,
                    layout.batch(coefficients, rhs)?,
                )
            }
            _ => CertifiedBorrowedMap::from_certified_parts(values, report),
        }
    }
}

/// The general tiers: fedemean's full system for slope designs, then MAP at
/// `min(tol, 1e-12)` within the recovery memory plan.
pub(super) fn recover_general(
    r: &RecoveryRequest<'_>,
    intercepts: bool,
    memory: Option<crate::memory::InterceptRecoveryMemoryEstimate>,
    mut peak: crate::memory::SolverWorkspacePeak,
) -> Result<crate::fe::RecoveredCategoricalEffects> {
    use crate::fe::topology_slope_map as recover;
    let fits = |n| r.memory_budget.is_none_or(|b| b.fits(n));
    // Slope designs: fedemean's full system; a miss continues with MAP.
    if let Some(recovered) = recover_slopes(r, intercepts, memory, &mut peak)? {
        return Ok(recovered);
    }
    // General reconstruction uses repository-owned fallible coefficient
    // banks. Upstream LSMR recovery cannot provide the allocation-failure
    // contract required by the shared OLS/PPML publication transaction.
    if memory.is_some_and(|m| !fits(m.map_peak_bytes)) {
        return Err(allocation(
            "saved-FE recovery does not fit the joint execution/recovery memory plan",
        ));
    }
    super::include_recovery_peak(&mut peak, memory.map(|m| m.map_peak_bytes))?;
    recover::recover_finalized_categorical_effects_map_selected_certified(
        r.topology,
        r.effects,
        r.weights,
        r.target,
        r.requested_channels,
        // Small FE moments alone do not certify a complete slope decomposition.
        // Preserve the internal recovery target used before this shared owner;
        // publication still independently checks the requested public tolerance.
        MapConfig {
            tolerance: r.map.tolerance.min(1e-12),
            ..r.map
        },
        r.materialize_absorbed_sum,
        r.reconstruction_tolerance,
    )
    .map(|v| v.with_solver_workspace_peak(peak))
}
/// Recover a slope design's saved effects with fedemean's full system from
/// the fit's retained product (a warm start) or zero, within the memory left
/// beside the retained recovery envelope. `None`: not a slope design, the
/// envelope does not fit, or a numerical miss (the caller's MAP follows).
pub(super) fn recover_slopes(
    r: &RecoveryRequest<'_>,
    intercepts: bool,
    memory: Option<crate::memory::InterceptRecoveryMemoryEstimate>,
    peak: &mut crate::memory::SolverWorkspacePeak,
) -> Result<Option<crate::fe::RecoveredCategoricalEffects>> {
    use crate::fe::topology_slope_map as recover;
    let fits = |n| r.memory_budget.is_none_or(|b| b.fits(n));
    let Some(m) = memory.filter(|m| !intercepts && fits(m.retained_peak_bytes)) else {
        return Ok(None);
    };
    let options = recover::FullRecovery {
        start: r.retained,
        config: MapConfig {
            tolerance: r.map.tolerance.min(1e-12),
            ..r.map
        },
        memory_limit: r.memory_budget.map(|b| {
            b.usable_bytes
                .saturating_sub(b.execution_bytes)
                .saturating_sub(m.retained_peak_bytes)
        }),
        reconstruction_tolerance: r.reconstruction_tolerance,
        known_connected: r.known_connected,
        materialize_absorbed_sum: r.materialize_absorbed_sum,
    };
    match recover::recover_finalized_full_certified(
        r.topology,
        r.effects,
        r.weights,
        (r.target, r.requested_channels),
        options,
    ) {
        Ok((recovered, bytes)) => {
            let bytes = m
                .retained_peak_bytes
                .checked_add(bytes)
                .ok_or_else(|| allocation("full-system recovery peak overflows"))?;
            super::include_recovery_peak(peak, Some(bytes))?;
            Ok(Some(recovered.with_solver_workspace_peak(*peak)))
        }
        Err(e) if super::numerical_recovery_miss(&e) => Ok(None),
        Err(e) => Err(e),
    }
}
