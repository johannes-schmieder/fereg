//! Coordinate changes on a prepared separation generation. The original row
//! ledger and the finite, absorbed and offset parts remain separately checked.
mod exchange;
use super::prepared_irls::PreparedPpmlIrlsFit;
use super::prepared_model::PreparedPpmlModelGeneration;
use super::{PpmlCoefficientOutput, PpmlCoefficientStatus, PreparedPpmlRequestV1};
use crate::error::{ErrorCode, FeregError, Result};
use crate::ols::PreparedOlsData;
pub(super) use crate::slope_recovery::RecoveredSlope as GenerationRecovery;
use crate::slope_recovery::{absorbed::AbsorbedMap, mapping::PairMap};

#[derive(Clone, Copy)]
pub(super) enum ReportMap {
    Pair(PairMap),
    Intercept(PairMap),
    Absorbed(AbsorbedMap),
}

pub(super) struct OriginalCoordinates {
    pub(super) map: ReportMap,
    pub(super) predictors: Option<(Vec<f64>, Vec<f64>)>,
}

pub(super) fn preflight(request: &PreparedPpmlRequestV1) -> usize {
    // Multiway PSD repair uses original-unit standardized covariance before
    // clipping. Bias-corrected models have additional derivative outputs.
    if request.bias.is_some() || request.clusters.len() > 1 {
        return 0;
    }
    let slopes = request
        .ols
        .model
        .terms
        .iter()
        .filter(|term| {
            term.raw_kind == crate::metadata::RawTermKind::Variable && !term.whole_column_omitted
        })
        .take(2)
        .count();
    let intercepts = !request.ols.absorbed_terms.is_empty()
        && request
            .ols
            .absorbed_terms
            .iter()
            .all(|e| e.intercept && e.slope_sources.is_empty());
    if slopes >= 1 && intercepts {
        4
    } else {
        usize::from(slopes >= 2)
    }
}

impl GenerationRecovery {
    pub(super) fn original_coordinates(
        &self,
        generation: &PreparedPpmlModelGeneration,
        fit: &PreparedPpmlIrlsFit,
        coefficients: &PpmlCoefficientOutput,
        tolerance: f64,
    ) -> Result<OriginalCoordinates> {
        let identified = |slot| {
            if coefficients.statuses().get(slot) == Some(&PpmlCoefficientStatus::Identified) {
                Ok(())
            } else {
                Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "a reparameterized PPML slope is not identified on the final sample",
                ))
            }
        };
        match self {
            Self::Pair(pair) => {
                identified(pair.map.source)?;
                identified(pair.map.target)?;
                let check = pair.map.prediction_check(coefficients.values())?;
                pair.visit_full_predictions(generation.fit_view(), &check, coefficients.values(),|row,xb|{
                    // feppml_p first adds offset (or log exposure) to _predict, xb
                    // and then adds the saved absorbed contribution.
                    let eta = (xb + generation.offset()[row]) + fit.linear().absorbed_predictor()[row];
                    let mu = eta.exp();
                    let relative = (mu - fit.mean()[row]).abs() / fit.mean()[row];
                    if mu <= 0.0 || !relative.is_finite() || relative > tolerance.min(1e-7) {
                        return Err(FeregError::new(ErrorCode::NonConvergence,
                            "the posted original slopes cannot reproduce the certified Poisson mean"));
                    }
                    Ok(())
                })?;
                Ok(OriginalCoordinates {
                    map: if matches!(
                        generation.model().columns[pair.source_column].kind,
                        crate::ols::PreparedOlsColumnKind::Intercept
                    ) {
                        ReportMap::Intercept(pair.map)
                    } else {
                        ReportMap::Pair(pair.map)
                    },
                    predictors: None,
                })
            }
            Self::Absorbed(fe) => {
                identified(fe.map.slot)?;
                let predictors = fe.predictions(
                    generation.fit_view(),
                    coefficients.values(),
                    fit.linear().finite_predictor(),
                    fit.linear().absorbed_predictor(),
                    generation.offset(),
                    fit.eta(),
                    tolerance,
                )?;
                Ok(OriginalCoordinates {
                    map: ReportMap::Absorbed(fe.map),
                    predictors: Some(predictors),
                })
            }
        }
    }
}

fn numerically_omitted(status: crate::ols::PreparedOlsFitStatus) -> bool {
    use crate::ols::PreparedOlsFitStatus::{CollinearWithFixedEffects, NumericallyRankDeficient};
    matches!(status, CollinearWithFixedEffects | NumericallyRankDeficient)
}

fn verify_translated_omissions(
    generation: &PreparedPpmlModelGeneration,
    fit: &PreparedPpmlIrlsFit,
) -> Result<()> {
    let data = generation.fit_view();
    for (position, coefficient) in fit.coefficients().iter().enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        let Some(column) = coefficient
            .generated_column_index
            .filter(|_| numerically_omitted(coefficient.status))
        else {
            continue;
        };
        let base = data.column_value(column, 0)?;
        for row in 1..fit.mean().len() {
            crate::interrupt::row_checkpoint(row)?;
            if data.column_value(column, row)? != base {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "translated PPML fit cannot certify an original nonconstant omitted column",
                ));
            }
        }
    }
    Ok(())
}

// A slope can exchange places with a different retained column after changing
// units. An unrelated omission on the final sample supplies no such evidence.
fn exact_alternate(
    generation: &PreparedPpmlModelGeneration,
    fit: &PreparedPpmlIrlsFit,
    target: usize,
    target_omitted: bool,
) -> Result<Option<usize>> {
    let view = generation.fit_view();
    for c in fit.coefficients() {
        let Some(other) = c.generated_column_index else {
            continue;
        };
        if other == target
            || numerically_omitted(c.status) == target_omitted
            || (!numerically_omitted(c.status) && c.value.is_none())
        {
            continue;
        }
        let mut ratio = None;
        let mut exact = true;
        for r in 0..fit.mean().len() {
            crate::interrupt::row_checkpoint(r)?;
            let a = view.column_value(target, r)?;
            let b = view.column_value(other, r)?;
            if b == 0.0 {
                exact &= a == 0.0;
            } else {
                let m = *ratio.get_or_insert(a / b);
                exact &= m.is_finite() && m != 0.0 && (-m).mul_add(b, a) == 0.0;
            }
            if !exact {
                break;
            }
        }
        if exact && ratio.is_some() {
            return Ok(Some(other));
        }
    }
    Ok(None)
}

/// A genuinely weak slope with a zero conditional effect can be omitted by
/// the original rank policy. Verify both fits on the exact final sample before
/// retaining that representation; a fully identified raw fit must still pass
/// the original-unit publication certificate.
pub(super) fn replay_original_omission(
    generation: &mut PreparedPpmlModelGeneration,
    stable: PreparedPpmlIrlsFit,
    interrupt: &mut dyn FnMut(super::PpmlInterruptPoint) -> Result<()>,
    mut refit: impl FnMut(
        &PreparedPpmlModelGeneration,
        &mut dyn FnMut(super::PpmlInterruptPoint) -> Result<()>,
    ) -> Result<PreparedPpmlIrlsFit>,
    mut certify: impl FnMut(
        &PreparedPpmlModelGeneration,
        &PreparedPpmlIrlsFit,
        &mut dyn FnMut(super::PpmlInterruptPoint) -> Result<()>,
    ) -> Result<super::PpmlInferenceResult>,
) -> Result<(PreparedPpmlIrlsFit, Option<super::PpmlInferenceResult>)> {
    let Some(recovery) = &generation.slope_recovery else {
        return Ok((stable, None));
    };
    if matches!(recovery, GenerationRecovery::Pair(pair) if matches!(generation.model().columns[pair.source_column].kind, crate::ols::PreparedOlsColumnKind::Intercept))
    {
        // Inverse translation preserves the independently certified retained
        // span and the two participating coefficients must both be identified.
        // Every other omitted direction must belong to that span exactly. A
        // constant belongs to the explicit intercept on all original rows.
        verify_translated_omissions(generation, &stable)?;
        return Ok((stable, None));
    }
    let target = match recovery {
        GenerationRecovery::Pair(pair) => pair.target_column,
        GenerationRecovery::Absorbed(fe) => fe.target_column,
    };
    let fitted_target = stable
        .coefficients()
        .iter()
        .find(|c| c.generated_column_index == Some(target));
    let Some(fitted_target) = fitted_target else {
        return Ok((stable, None));
    };
    let target_omitted = numerically_omitted(fitted_target.status);
    let fe_recovery = matches!(recovery, GenerationRecovery::Absorbed(_));
    let alternate = exact_alternate(generation, &stable, target, target_omitted)?;
    let stable_rank = stable.linear().basis().kept_generated_columns().len();
    let beta = fitted_target.value.unwrap_or(0.);
    let mut negligible = true;
    for (r, eta) in stable.eta().iter().enumerate() {
        crate::interrupt::row_checkpoint(r)?;
        if (generation.fit_view().column_value(target, r)? * beta).abs()
            > 64.0 * f64::EPSILON * (1.0 + eta.abs())
        {
            negligible = false;
            break;
        }
    }
    let exchange = if !fe_recovery {
        exchange::Exchange::candidate(generation, &stable, target)?
    } else {
        None
    };
    if !negligible && alternate.is_none() && exchange.is_none() {
        return Ok((stable, None));
    }
    let reference = stable.into_mean();
    let recovery = generation.slope_recovery.take().expect("checked presence");
    let pair = match recovery {
        GenerationRecovery::Pair(p) => {
            let columns = [p.source_column, p.target_column];
            p.restore_design(generation.unique_data()?)?;
            columns
        }
        GenerationRecovery::Absorbed(fe) => {
            let column = fe.target_column;
            fe.restore_design(generation.unique_data()?)?;
            [column, column]
        }
    };
    match refit(generation, interrupt) {
        Ok(raw) => {
            let omitted = |column| {
                raw.coefficients().iter().any(|c| {
                    c.generated_column_index == Some(column) && numerically_omitted(c.status)
                })
            };
            let member_omitted = pair.iter().copied().any(omitted);
            let rank = raw.linear().basis().kept_generated_columns().len();
            // For the finite pair, keeping both original columns requires their
            // joint covariance certificate even when a third column is omitted.
            // An exchanged FE alias must be exact, omit its alternate, preserve
            // rank and pass independent covariance replay on the original frame.
            let exchanged = fe_recovery
                && !member_omitted
                && target_omitted
                && rank == stable_rank
                && alternate.is_some_and(omitted);
            let swapped = match exchange.as_ref() {
                Some(eligible) => eligible.verified(generation, &raw)?,
                None => false,
            };
            let legacy = (negligible || alternate.is_some()) && (member_omitted || exchanged);
            if (legacy || swapped) && raw.mean().len() == reference.len() {
                let mut agrees = true;
                for (row, (raw, stable)) in raw.mean().iter().zip(&reference).enumerate() {
                    crate::interrupt::row_checkpoint(row)?;
                    agrees &= *raw > 0.0 && (raw - stable).abs() / stable <= 1e-8;
                }
                if agrees && swapped {
                    match certify(generation, &raw, interrupt) {
                        Ok(inference) => return Ok((raw, Some(inference))),
                        Err(e)
                            if matches!(
                                e.code,
                                ErrorCode::NonConvergence | ErrorCode::InvalidData
                            ) => {}
                        Err(e) => return Err(e),
                    }
                } else if agrees && legacy {
                    generation.replayed_original_omission = true;
                    return Ok((raw, None));
                }
            }
        }
        Err(e)
            if e.code == ErrorCode::NonConvergence
                || exchange.is_some() && e.code == ErrorCode::InvalidData => {}
        Err(e) => return Err(e),
    }
    drop(reference);
    // The original bank and final sample are identical to the ones that already
    // admitted this replacement. Reuse its prior memory admission.
    generation.slope_recovery = Some(
        GenerationRecovery::prepare(generation.unique_data()?, u128::MAX)?.ok_or_else(|| {
            crate::error::invalid_argument("PPML final slope basis did not replay")
        })?,
    );
    refit(generation, interrupt).map(|fit| (fit, None))
}

/// A generation can first expose a weak pair after deletions. Share the active
/// admission with the detector, so candidate storage is admitted before its
/// allocation alongside all earlier detector reservations.
pub(super) struct RecoveryAdmission {
    columns: usize,
    config: super::PpmlMemoryConfig,
    charged: bool,
    plan: Option<std::sync::Arc<super::memory::LiveMemoryPlan>>,
}

impl RecoveryAdmission {
    pub(super) fn new(columns: usize, config: super::PpmlMemoryConfig) -> Self {
        Self {
            columns,
            config,
            charged: false,
            plan: None,
        }
    }
    pub(super) fn bind(&mut self, plan: std::sync::Arc<super::memory::LiveMemoryPlan>) {
        self.plan = Some(plan);
    }
    fn upgrade(&self, base: super::PpmlMemoryPlan) -> Result<super::PpmlMemoryPlan> {
        let mut dimensions = base.preflight_dimensions;
        dimensions.metadata.generation_bytes_per_row +=
            (self.columns * std::mem::size_of::<f64>()) as u128;
        // Fisher replay retains the returned inference while it evaluates
        // the compensated Gram and inverse correction, or a compact bank
        // of one-way cluster influences. Width and group count may shrink
        // after separation. The charge is never applied to an ordinary fit.
        let k = dimensions.reported_coefficient_slots as u128;
        let groups = dimensions.cluster_subset_groups as u128;
        let overflow = || crate::error::allocation("PPML slope inference capacity overflow");
        let cells = k
            .checked_mul(k)
            .and_then(|v| v.checked_mul(4))
            .and_then(|v| v.checked_add(k.checked_mul(8)?))
            .and_then(|v| v.checked_add(k.checked_mul(groups)?.checked_mul(2)?))
            .ok_or_else(overflow)?;
        dimensions.metadata.result_bytes = dimensions
            .metadata
            .result_bytes
            .checked_add(
                cells
                    .checked_mul(std::mem::size_of::<f64>() as u128)
                    .ok_or_else(overflow)?,
            )
            .ok_or_else(overflow)?;
        {
            let bytes = super::absorbed_information::workspace_bytes(
                dimensions.reported_coefficient_slots,
                dimensions.fe_total_levels,
                dimensions.cluster_subset_groups,
            );
            dimensions.metadata.result_bytes = dimensions
                .metadata
                .result_bytes
                .checked_add(bytes)
                .ok_or_else(overflow)?;
        }
        let mut config = self.config;
        config.require_fast_route = base.selected == super::PpmlMemoryRoute::Fast;
        let mut candidate = super::plan_ppml_memory(dimensions, config)?
            .with_solver_workspace(base.solver_workspace_bytes)?;
        if candidate.selected != base.selected
            || !candidate.admit_separation_projection(base.separation_projection_bytes)
            || !candidate.admit_reusable_projection(base.reusable_projection_bytes)
            || !candidate.admit_reusable_certificate(base.reusable_certificate_bytes)
            || !candidate.admit_reusable_slope(base.reusable_slope_bytes)
            || !candidate.restore_sparse_peaks(base.sparse_proof_peaks)
        {
            return Err(crate::error::allocation(
                "PPML memory limit cannot admit the recovered slopes beside the selected solver",
            ));
        }
        Ok(candidate)
    }

    pub(super) fn translate(
        mut plan: super::PpmlMemoryPlan,
        memory: super::PpmlMemoryConfig,
        generation: &mut PreparedPpmlModelGeneration,
    ) -> Result<Option<super::PpmlMemoryPlan>> {
        use crate::slope_recovery::intercept_translation;
        if generation.slope_recovery.is_some()
            || generation
                .cluster_design()
                .is_some_and(|c| c.dimensions().len() > 1)
        {
            return Ok(None);
        }
        let overhead = generation.owned_buffer_bytes() - generation.fit_view().owned_buffer_bytes();
        let Some(data) = generation.try_unique_data() else {
            return Ok(None);
        };
        let recovery = intercept_translation(data, |required| {
            plan = Self::new(1, memory).upgrade(plan)?;
            if required + overhead > plan.selected_inventory().prepared_generation_bytes {
                return Err(crate::error::allocation(
                    "unable to admit translated PPML slope",
                ));
            }
            Ok(())
        });
        // This is an optional alternate to the original numerical failure.
        // An unadmitted alternate has no independent allocation prerequisite.
        let recovery = match recovery {
            Err(error) if error.code == ErrorCode::Allocation => return Ok(None),
            other => other?,
        };
        generation.slope_recovery = recovery.map(GenerationRecovery::Pair);
        Ok(generation.slope_recovery.as_ref().map(|_| plan))
    }

    pub(super) fn prepare(
        &mut self,
        data: &mut PreparedOlsData,
        overhead: u128,
    ) -> Result<(Option<GenerationRecovery>, u128)> {
        let shared = self.plan.as_ref().ok_or_else(|| {
            crate::error::invalid_argument("PPML slope admission lacks the current fit plan")
        })?;
        let base = shared.get();
        let base_bound = base.selected_inventory().prepared_generation_bytes;
        if self.charged {
            return Ok((
                GenerationRecovery::prepare(data, base_bound.saturating_sub(overhead))?,
                base_bound,
            ));
        }
        // Zero capacity is an allocation-free screening probe. Only the
        // original-data geometry can request storage, independently of y.
        match GenerationRecovery::prepare(data, 0) {
            Ok(None) => return Ok((None, base_bound)),
            Err(e) if e.code == ErrorCode::Allocation => {}
            Err(e) => return Err(e),
            Ok(Some(_)) => {
                return Err(crate::error::invalid_argument(
                    "PPML slope probe allocated before admission",
                ))
            }
        }
        let candidate = self.upgrade(base)?;
        let bound = candidate.selected_inventory().prepared_generation_bytes;
        let recovery = GenerationRecovery::prepare(data, bound.saturating_sub(overhead))?;
        if recovery.is_some() {
            shared.set(candidate);
            self.charged = true;
            Ok((recovery, bound))
        } else {
            Ok((None, base_bound))
        }
    }
}
