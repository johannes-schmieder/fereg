//! Coordinate changes on a prepared separation generation. The original row
//! ledger and the finite, absorbed and offset parts remain separately checked.
use super::prepared_irls::PreparedPpmlIrlsFit;
use super::prepared_model::PreparedPpmlModelGeneration;
use super::{PpmlCoefficientOutput, PpmlCoefficientStatus, PreparedPpmlRequestV1};
use crate::error::{ErrorCode, FeregError, Result};
use crate::ols::PreparedOlsData;
use crate::slope_recovery::{
    self,
    absorbed::{AbsorbedMap, AbsorbedRecovery},
    mapping::PairMap,
    Recovery,
};

#[derive(Debug, PartialEq)]
pub(super) enum GenerationRecovery {
    Pair(Recovery),
    Absorbed(AbsorbedRecovery),
}

#[derive(Clone, Copy)]
pub(super) enum ReportMap {
    Pair(PairMap),
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
    pub(super) fn prepare(data: &mut PreparedOlsData, ceiling: u128) -> Result<Option<Self>> {
        if let Some(pair) = slope_recovery::prepare(data, ceiling)? {
            return Ok(Some(Self::Pair(pair)));
        }
        Ok(slope_recovery::absorbed::prepare_absorbed(data, ceiling)?.map(Self::Absorbed))
    }

    pub(super) fn owned_bytes(&self) -> u128 {
        match self {
            Self::Pair(pair) => pair.owned_bytes(),
            Self::Absorbed(fe) => fe.owned_bytes(),
        }
    }

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
                    map: ReportMap::Pair(pair.map),
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

/// A genuinely weak slope with a zero conditional effect can be omitted by
/// the original rank policy. Verify both fits on the exact final sample before
/// retaining that representation; a fully identified raw fit must still pass
/// the original-unit publication certificate.
pub(super) fn replay_original_omission(
    generation: &mut PreparedPpmlModelGeneration,
    stable: PreparedPpmlIrlsFit,
    mut refit: impl FnMut(&PreparedPpmlModelGeneration) -> Result<PreparedPpmlIrlsFit>,
) -> Result<PreparedPpmlIrlsFit> {
    let Some(recovery) = &generation.slope_recovery else {
        return Ok(stable);
    };
    let target = match recovery {
        GenerationRecovery::Pair(pair) => pair.target_column,
        GenerationRecovery::Absorbed(fe) => fe.target_column,
    };
    let fitted_target = stable
        .coefficients()
        .iter()
        .find(|c| c.generated_column_index == Some(target));
    let Some(fitted_target) = fitted_target else {
        return Ok(stable);
    };
    use crate::ols::PreparedOlsFitStatus::{CollinearWithFixedEffects, NumericallyRankDeficient};
    let aliased = stable.coefficients().iter().any(|c| {
        matches!(
            c.status,
            CollinearWithFixedEffects | NumericallyRankDeficient
        )
    });
    let stable_rank = stable.linear().basis().kept_generated_columns().len();
    if !aliased {
        let beta = fitted_target.value.unwrap_or(0.0);
        for (row, eta) in stable.eta().iter().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            if (generation.fit_view().column_value(target, row)? * beta).abs()
                > 64.0 * f64::EPSILON * (1.0 + eta.abs())
            {
                return Ok(stable);
            }
        }
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
    match refit(generation) {
        Ok(raw) => {
            use crate::ols::PreparedOlsFitStatus::{
                CollinearWithFixedEffects, NumericallyRankDeficient,
            };
            let omitted = raw.coefficients().iter().any(|c| {
                matches!(
                    c.status,
                    CollinearWithFixedEffects | NumericallyRankDeficient
                ) && c.generated_column_index.is_some_and(|i| pair.contains(&i))
            });
            let rank = raw.linear().basis().kept_generated_columns().len();
            if (omitted || aliased && rank == stable_rank) && raw.mean().len() == reference.len() {
                let mut agrees = true;
                for (row, (raw, stable)) in raw.mean().iter().zip(&reference).enumerate() {
                    crate::interrupt::row_checkpoint(row)?;
                    agrees &= *raw > 0.0 && (raw - stable).abs() / stable <= 1e-8;
                }
                if agrees {
                    return Ok(raw);
                }
            }
        }
        Err(e) if e.code == ErrorCode::NonConvergence => {}
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
    refit(generation)
}

/// A generation can first expose a weak pair after deletions. Share the active
/// admission with the detector, so candidate storage is admitted before its
/// allocation alongside all earlier detector reservations.
pub(super) struct RecoveryAdmission {
    columns: usize,
    config: super::PpmlMemoryConfig,
    charged: bool,
    plan: Option<std::rc::Rc<std::cell::Cell<super::PpmlMemoryPlan>>>,
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
    pub(super) fn bind(
        &mut self,
        plan: super::PpmlMemoryPlan,
    ) -> std::rc::Rc<std::cell::Cell<super::PpmlMemoryPlan>> {
        let shared = std::rc::Rc::new(std::cell::Cell::new(plan));
        self.plan = Some(shared.clone());
        shared
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
        let mut config = self.config;
        config.require_fast_route = base.selected == super::PpmlMemoryRoute::Fast;
        let mut candidate = super::plan_ppml_memory(dimensions, config)?
            .with_solver_workspace(base.solver_workspace_bytes)?;
        if candidate.selected != base.selected
            || !candidate.admit_separation_projection(base.separation_projection_bytes)
            || !candidate.admit_reusable_projection(base.reusable_projection_bytes)
            || !candidate.admit_reusable_certificate(base.reusable_certificate_bytes)
            || !candidate.admit_reusable_slope(base.reusable_slope_bytes)
        {
            return Err(crate::error::allocation(
                "PPML memory limit cannot admit the recovered slopes beside the selected solver",
            ));
        }
        Ok(candidate)
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
