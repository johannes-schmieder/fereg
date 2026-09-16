use super::prepared_model::PreparedPpmlModel;
use super::prepared_separation::candidate_ledger_hash_candidate_axis;
use super::prepared_separation::CertifiedSeparationSeal;
use super::psd::inspect_published_ppml_covariance;
#[cfg(test)]
use super::psd::apply_ppml_psd_policy;
use super::residuals::{ppml_residuals, PpmlResidualKind};
use super::vce::{compute_ppml_wald, validate_weight_report};
#[cfg(test)]
use super::PpmlIterationWorkspace;
use super::{
    poisson_null_objective, poisson_objective, PoissonObjective, PpmlAllocationTelemetry,
    PpmlAllocatorSnapshot, PpmlAllocatorSnapshotProbe, PpmlCertificate, PpmlInferenceResult,
    PpmlMemoryTelemetry, PpmlPhase, PpmlPhaseClock, PpmlPsdState, PpmlSolverReport,
    PpmlTestDistribution, PpmlVceKind, PpmlVceRequest, PreparedPpmlAdjustmentKind,
    PreparedPpmlAdjustmentReport, PreparedPpmlWeightReport,
};
use crate::data::{DropReason, WeightKind};
use crate::error::{ErrorCode, FeregError, Result};
#[cfg(test)]
use crate::fe::FeSolverKind;
use crate::fe::{
    connected_components_all_fallible, BorrowedMapWeights, FeCoefficientBatch, LsmrConfig,
    MapConfig,
};
use crate::ppml_separation::{
    separation_method_count, separation_workspace_requirements, MethodStatus,
    SeparationCertificate, SeparationConfig, SeparationDropReason, SeparationMethod,
    SeparationMethodReport, SeparationMethods, SeparationNote, SeparationReport,
};
use crate::topology_dof::{CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofPolicy};
use std::sync::Arc;

fn ppml_fe_recovery_mask_length(length: usize, inject_allocation_failure: bool) -> usize {
    if inject_allocation_failure {
        usize::MAX
    } else {
        length
    }
}

/// PPML likelihood statistics; OLS fit statistics are intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlLikelihoodStatistics {
    pub deviance: f64,
    pub log_likelihood: f64,
    pub null_mean: f64,
    pub null_log_likelihood: f64,
    pub pseudo_r_squared: f64,
}

impl PpmlLikelihoodStatistics {
    pub fn from_final_state(
        objective: PoissonObjective,
        outcome: &[f64],
        prior_weights: &[f64],
    ) -> Result<Self> {
        if !objective.deviance.is_finite()
            || objective.deviance < 0.0
            || !objective.log_likelihood.is_finite()
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML final objective is not publishable",
            ));
        }
        let null = poisson_null_objective(outcome, prior_weights)?;
        if null.log_likelihood == 0.0 {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML null likelihood is zero, so pseudo R-squared is undefined",
            ));
        }
        let pseudo_r_squared = 1.0 - objective.log_likelihood / null.log_likelihood;
        if !pseudo_r_squared.is_finite() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML pseudo R-squared is not finite",
            ));
        }
        Ok(Self {
            deviance: objective.deviance,
            log_likelihood: objective.log_likelihood,
            null_mean: null.mean,
            null_log_likelihood: null.log_likelihood,
            pseudo_r_squared,
        })
    }
}

/// Prediction and residual requests supported by the initial PPML surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlPredictionKind {
    Mu,
    Xb,
    Absorbed,
    Eta,
    Response,
    Score,
    Pearson,
    Working,
    Deviance,
    Anscombe,
}

const fn prediction_residual_kind(kind: PpmlPredictionKind) -> Option<PpmlResidualKind> {
    match kind {
        PpmlPredictionKind::Response => Some(PpmlResidualKind::Response),
        PpmlPredictionKind::Score => Some(PpmlResidualKind::Score),
        PpmlPredictionKind::Pearson => Some(PpmlResidualKind::Pearson),
        PpmlPredictionKind::Working => Some(PpmlResidualKind::Working),
        PpmlPredictionKind::Deviance => Some(PpmlResidualKind::Deviance),
        PpmlPredictionKind::Anscombe => Some(PpmlResidualKind::Anscombe),
        PpmlPredictionKind::Mu
        | PpmlPredictionKind::Xb
        | PpmlPredictionKind::Absorbed
        | PpmlPredictionKind::Eta => None,
    }
}

/// Optional fixed-effect products retained for publication.
///
/// The authoritative observation-space total is
/// [`PpmlOutputState::absorbed_predictor`]. Keeping it there avoids a second
/// O(N) copy solely to certify or publish saved channels. Every entry of
/// `channels` is one normalized saved FE channel on the same final sample.
#[derive(Debug, Clone, PartialEq)]
pub struct PpmlSavedEffectOutput {
    /// Requested channel vectors only, in increasing declared-channel order.
    pub channels: Vec<Vec<f64>>,
    pub declared_channels: usize,
    /// Declared channel identity for each retained output vector.
    pub declared_channel_indices: Vec<usize>,
    /// Certificate from recovery over the complete normalized coefficient
    /// decomposition, including channels not retained for publication.
    pub total_reconstruction_relative_infinity_norm: f64,
    pub full_decomposition_relative_infinity_norm: f64,
    pub full_decomposition_tolerance: f64,
    pub full_decomposition_certified: bool,
}

/// Independent reconstruction checks over the final accepted strict state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlReconstructionCertificate {
    pub predictor_absolute_infinity_norm: f64,
    pub predictor_relative_infinity_norm: f64,
    pub mean_relative_infinity_norm: f64,
    pub saved_total_relative_infinity_norm: f64,
    pub saved_channels_relative_infinity_norm: f64,
    pub saved_channels: usize,
    pub saved_declared_channels: usize,
    pub saved_complete_decomposition: bool,
    pub tolerance: f64,
    pub passed: bool,
}

/// Validated final-sample predictor products and stable original-row mapping.
///
/// Construction is the result boundary: every row-scoped vector must use the
/// same compact axis, retained original rows must be strictly increasing, and
/// all requested saved products must reconstruct before the value can be
/// handed to a publication transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct PpmlOutputState {
    original_observations: usize,
    original_rows: Vec<usize>,
    /// Retained only by the general-purpose constructor. Production PPML
    /// stores the one requested xb vector directly instead of retaining two
    /// inputs for one output.
    offset: Vec<f64>,
    finite_predictor: Vec<f64>,
    xb: Vec<f64>,
    absorbed_predictor: Vec<f64>,
    eta: Vec<f64>,
    /// The final mean is copied once for composite objective replay. It is
    /// cleared immediately after certification unless explicitly requested.
    mean: Vec<f64>,
    mean_requested: bool,
    residual_kind: Option<PpmlResidualKind>,
    residual: Vec<f64>,
    saved_effects: Option<PpmlSavedEffectOutput>,
    certificate: PpmlReconstructionCertificate,
}

/// PPML-owned scientific status on the full canonical coefficient-slot axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlCoefficientStatus {
    Identified,
    BaseLevel,
    SyntaxOmitted,
    NormalizedStructuralConstant,
    CollinearWithFixedEffects,
    CollinearAfterResidualization,
    NumericallyRankDeficient,
}

/// Full reported coefficients plus the explicit generated/compact mappings.
/// The within design remains compact; this metadata never expands it to the
/// full coefficient-slot dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct PpmlCoefficientOutput {
    values: Vec<f64>,
    statuses: Vec<PpmlCoefficientStatus>,
    generated_columns: Vec<Option<usize>>,
    kept_full_slots: Vec<usize>,
}

impl PpmlCoefficientOutput {
    pub fn new(
        values: Vec<f64>,
        statuses: Vec<PpmlCoefficientStatus>,
        generated_columns: Vec<Option<usize>>,
        kept_full_slots: Vec<usize>,
    ) -> Result<Self> {
        let dimension = values.len();
        if dimension == 0
            || statuses.len() != dimension
            || generated_columns.len() != dimension
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML coefficient products are empty, nonfinite, or misaligned",
            ));
        }
        let mut identified = fallible_filled(
            dimension,
            false,
            "unable to reserve PPML coefficient-slot validation state",
        )?;
        for &slot in &kept_full_slots {
            if slot >= dimension || identified[slot] {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML kept full slots are duplicated or out of range",
                ));
            }
            identified[slot] = true;
        }
        for slot in 0..dimension {
            match statuses[slot] {
                PpmlCoefficientStatus::Identified if !identified[slot] => {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML identified status is absent from the compact inference mapping",
                    ));
                }
                PpmlCoefficientStatus::Identified => {}
                PpmlCoefficientStatus::NormalizedStructuralConstant if identified[slot] => {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML normalized structural constant cannot add finite within rank",
                    ));
                }
                PpmlCoefficientStatus::NormalizedStructuralConstant => {
                    if values[slot] != 0.0 {
                        return Err(FeregError::new(
                            ErrorCode::Internal,
                            "PPML normalized structural constant must be exactly zero",
                        ));
                    }
                }
                PpmlCoefficientStatus::BaseLevel
                | PpmlCoefficientStatus::SyntaxOmitted
                | PpmlCoefficientStatus::CollinearWithFixedEffects
                | PpmlCoefficientStatus::CollinearAfterResidualization
                | PpmlCoefficientStatus::NumericallyRankDeficient => {
                    if identified[slot] || values[slot] != 0.0 {
                        return Err(FeregError::new(
                            ErrorCode::Internal,
                            "PPML omitted or redundant coefficient must be zero and outside the compact mapping",
                        ));
                    }
                }
            }
        }
        if identified.iter().filter(|value| **value).count() != kept_full_slots.len() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML kept-slot validation count is inconsistent",
            ));
        }
        Ok(Self {
            values,
            statuses,
            generated_columns,
            kept_full_slots,
        })
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    #[must_use]
    pub fn statuses(&self) -> &[PpmlCoefficientStatus] {
        &self.statuses
    }

    #[must_use]
    pub fn generated_columns(&self) -> &[Option<usize>] {
        &self.generated_columns
    }

    #[must_use]
    pub fn kept_full_slots(&self) -> &[usize] {
        &self.kept_full_slots
    }
}

/// One authoritative original/compact row ledger for every result product.
#[derive(Debug, Clone, PartialEq)]
pub struct PpmlFinalSample {
    original_observations: usize,
    candidate_observations: usize,
    prepared_original_rows: Arc<Vec<usize>>,
    original_rows: Vec<usize>,
    drop_reasons: Vec<DropReason>,
    outcome: Vec<f64>,
    prior_weights: Vec<f64>,
}

impl PpmlFinalSample {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        original_observations: usize,
        candidate_observations: usize,
        prepared_original_rows: Arc<Vec<usize>>,
        original_rows: Vec<usize>,
        drop_reasons: Vec<DropReason>,
        outcome: Vec<f64>,
        prior_weights: Vec<f64>,
    ) -> Result<Self> {
        if original_observations == 0
            || candidate_observations == 0
            || candidate_observations > original_observations
            || drop_reasons.len() != original_observations
            || original_rows.is_empty()
            || outcome.len() != original_rows.len()
            || prior_weights.len() != original_rows.len()
            || original_rows.iter().enumerate().any(|(position, row)| {
                *row >= original_observations
                    || (position > 0 && original_rows[position - 1] >= *row)
            })
            || prepared_original_rows
                .iter()
                .enumerate()
                .any(|(position, row)| {
                    *row >= original_observations
                        || (position > 0 && prepared_original_rows[position - 1] >= *row)
                })
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML final sample ledger is empty, misaligned, or not in stable original order",
            ));
        }
        let observed_candidates = drop_reasons
            .iter()
            .filter(|reason| **reason != DropReason::ExcludedByIfIn)
            .count();
        if observed_candidates != candidate_observations {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML candidate count disagrees with the original-axis drop ledger",
            ));
        }
        let mut retained = fallible_filled(
            original_observations,
            false,
            "unable to reserve PPML final-row validation state",
        )?;
        for &row in &original_rows {
            retained[row] = true;
        }
        if drop_reasons.iter().enumerate().any(|(row, reason)| {
            (retained[row] && *reason != DropReason::Retained)
                || (!retained[row] && *reason == DropReason::Retained)
        }) {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML retained rows disagree with original-axis drop reasons",
            ));
        }
        if outcome
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
            || prior_weights
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML final outcome/weight experiment is outside the supported domain",
            ));
        }
        if !outcome.iter().any(|value| *value > 0.0) {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "PPML final sample has no positive weighted outcome",
            ));
        }
        Ok(Self {
            original_observations,
            candidate_observations,
            prepared_original_rows,
            original_rows,
            drop_reasons,
            outcome,
            prior_weights,
        })
    }

    #[must_use]
    pub const fn original_observations(&self) -> usize {
        self.original_observations
    }

    #[must_use]
    pub const fn candidate_observations(&self) -> usize {
        self.candidate_observations
    }

    #[must_use]
    pub fn prepared_original_rows(&self) -> &[usize] {
        &self.prepared_original_rows
    }

    #[must_use]
    pub fn original_rows(&self) -> &[usize] {
        &self.original_rows
    }

    #[must_use]
    pub fn drop_reasons(&self) -> &[DropReason] {
        &self.drop_reasons
    }

    #[must_use]
    pub fn outcome(&self) -> &[f64] {
        &self.outcome
    }

    #[must_use]
    pub fn prior_weights(&self) -> &[f64] {
        &self.prior_weights
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlConvergenceReport {
    pub converged: bool,
    /// Two consecutive IRLS states met both objective and score tolerances
    /// before the strict final solve was accepted.
    pub two_consecutive_convergence: bool,
    /// Every post-initial weighted solve and the strict final solve preserved
    /// the first certified finite-rank basis.
    pub rank_frozen: bool,
    pub outer_iterations: usize,
    pub inner_iterations: usize,
    pub step_halvings: usize,
    pub strict_profile_rejections: usize,
    pub strict_final_solve_passed: bool,
    pub linear_within_relative_infinity_norm: f64,
    pub linear_reconstruction_relative_infinity_norm: f64,
    pub final_score: PpmlCertificate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlFixedEffectChannelKind {
    Intercept,
    Slope,
}

/// One fixed-effect result row in declared channel order. Term/source IDs are
/// the caller's stable one-based metadata identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpmlFixedEffectResult {
    pub declared_channel: usize,
    pub term_id: u32,
    pub source_ids: Vec<u32>,
    pub kind: PpmlFixedEffectChannelKind,
    /// One-based continuous source identity for a slope channel. Intercept
    /// channels carry `None`; categorical interaction sources stay in
    /// `source_ids` for both kinds.
    pub slope_source_id: Option<u32>,
    pub levels: usize,
    pub categories: usize,
    pub redundant_parameters: usize,
    pub absorbed_rank: usize,
    /// Whether the per-term redundancy count is exact rather than a
    /// conservative graph/metadata approximation.
    pub redundancy_exact: bool,
    /// Whether this term is nested in another absorbed or cluster structure.
    /// This is explicit result metadata; publication layers must not infer it
    /// from the aggregate absorbed rank.
    pub nested: bool,
    pub saved_channel: Option<usize>,
    pub normalization_applied: bool,
    pub normalization_certified: bool,
    pub reconstruction_certified: bool,
}

/// One cluster dimension in declared user order. Full subset topology lives
/// in `PpmlInferenceResult`; this table binds its singleton masks to labels and
/// source recipes without asking the plugin to reconstruct identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpmlClusterDimensionResult {
    pub declared_dimension: usize,
    pub label: String,
    pub source_ids: Vec<u32>,
    pub groups: usize,
}

/// Exact PPML-owned conversion of the shared prepared categorical DoF report.
/// The OLS-only virtual no-absorb direction is deliberately excluded: PPML
/// classifies an ordinary no-FE structural constant as identified finite rank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedPpmlFixedEffectReport {
    pub absorbed_rank: usize,
    pub connected_components: usize,
    pub is_approximate: bool,
    pub rows: Vec<PpmlFixedEffectResult>,
}

/// Final-Hessian FE recovery handoff for the composite PPML result builder.
/// Only explicitly requested O(N) channels survive this boundary, while the
/// complete declared decomposition is recovered and certified before the
/// unrequested channel buffers are released.
#[derive(Debug, PartialEq)]
pub(crate) struct PreparedPpmlRecoveredFixedEffects {
    pub report: PreparedPpmlFixedEffectReport,
    pub saved_effects: Option<PpmlSavedEffectOutput>,
    pub recovery_iterations: usize,
    pub reconstruction_relative_infinity_norm: f64,
}

/// Compute final-sample absorbed DoF and declared FE rows without duplicating
/// the shared topology algorithms. `requested_saved_channels` is in the same
/// intercept-then-slopes order as the prepared DoF rows.
pub(crate) fn prepare_ppml_fixed_effect_report(
    model: &PreparedPpmlModel,
    requested_saved_channels: &[bool],
) -> Result<PreparedPpmlFixedEffectReport> {
    let prepared = match model {
        PreparedPpmlModel::Categorical(prepared) => prepared,
        PreparedPpmlModel::CategoricalClustered(prepared) => prepared.ols(),
    };
    if prepared.effect_specs().is_empty() {
        if !requested_saved_channels.is_empty() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML saved fixed effects were requested for a model without absorb terms",
            ));
        }
        return Ok(PreparedPpmlFixedEffectReport {
            absorbed_rank: 0,
            connected_components: 0,
            is_approximate: false,
            rows: Vec::new(),
        });
    }

    let mut cluster_dimensions = Vec::new();
    if let Some(clusters) = model.clusters() {
        cluster_dimensions
            .try_reserve_exact(clusters.len())
            .map_err(|_| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "unable to reserve PPML cluster nesting views",
                )
            })?;
        for dimension in 0..clusters.len() {
            cluster_dimensions.push(model.cluster_codes(dimension)?);
        }
    }
    // ADR 0016 freezes PPML to the shared pairwise topology calculation. The
    // OLS policy adapter is deliberately absent from this boundary: PPML owns
    // the subsequent structural-normalization and inference corrections.
    let mut report =
        prepared.absorbed_dof(CategoricalTopologyDofPolicy::Pairwise, &cluster_dimensions)?;
    // In PPML the structural normalization is a reported finite coordinate,
    // even though the shared linear solve obtains it from the absorbed span.
    // Transfer exactly one estimable intercept direction from df_a into that
    // coordinate. This leaves total model rank unchanged and does not affect
    // the compact sandwich dimension.
    if prepared.structural_intercept() {
        if let Some(intercept) = report.effects.iter_mut().find(|effect| {
            effect.slope_source.is_none() && !effect.nested && effect.coefficients > 0
        }) {
            intercept.redundant = intercept.redundant.checked_add(1).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "PPML structural-normalization redundancy overflows",
                )
            })?;
            intercept.coefficients -= 1;
            report.redundant_categories =
                report.redundant_categories.checked_add(1).ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Allocation,
                        "PPML structural-normalization redundancy total overflows",
                    )
                })?;
            report.absorbed_rank = report.absorbed_rank.checked_sub(1).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "PPML structural normalization exceeds absorbed rank",
                )
            })?;
        }
    }
    if report.effects.len() != requested_saved_channels.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML saved-channel request does not match declared FE channel order",
        ));
    }
    let connected_components = categorical_connected_components(prepared)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(report.effects.len()).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve PPML fixed-effect result rows",
        )
    })?;
    let mut saved_channel = 0_usize;
    for (declared_channel, (effect, requested)) in report
        .effects
        .iter()
        .zip(requested_saved_channels)
        .enumerate()
    {
        let (term_id, source_ids) = match &effect.identity {
            CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources } => {
                let mut source_ids = Vec::new();
                source_ids.try_reserve_exact(sources.len()).map_err(|_| {
                    FeregError::new(
                        ErrorCode::Allocation,
                        "unable to reserve PPML fixed-effect source identities",
                    )
                })?;
                for source in sources {
                    source_ids.push(source.get());
                }
                (term_id.get(), source_ids)
            }
            CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb => {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "explicit PPML absorbed topology produced a virtual no-absorb row",
                ));
            }
        };
        let assigned_saved_channel = requested.then(|| {
            let channel = saved_channel;
            saved_channel += 1;
            channel
        });
        rows.push(PpmlFixedEffectResult {
            declared_channel,
            term_id,
            source_ids,
            kind: if effect.slope_source.is_some() {
                PpmlFixedEffectChannelKind::Slope
            } else {
                PpmlFixedEffectChannelKind::Intercept
            },
            slope_source_id: effect.slope_source.map(|source| source.get()),
            levels: effect.categories,
            categories: effect.categories,
            redundant_parameters: effect.redundant,
            absorbed_rank: effect.coefficients,
            redundancy_exact: effect.redundancy_exact,
            nested: effect.nested,
            saved_channel: assigned_saved_channel,
            // Numerical recovery owns these evidence flags. Metadata/DoF
            // conversion alone must never claim a recovery certificate.
            normalization_applied: false,
            normalization_certified: false,
            reconstruction_certified: false,
        });
    }
    Ok(PreparedPpmlFixedEffectReport {
        absorbed_rank: report.absorbed_rank,
        connected_components,
        is_approximate: report.is_approximate,
        rows,
    })
}

/// Recover normalized PPML FE outputs under the accepted strict-state Hessian
/// weights `q * mu`. Passing preparation weights `q` is rejected whenever
/// `mu` is nonconstant, so OLS's preparation-weight recovery cannot silently
/// leak into PPML.
#[allow(clippy::too_many_arguments)]
pub(crate) fn recover_ppml_fixed_effects(
    model: &PreparedPpmlModel,
    mut report: PreparedPpmlFixedEffectReport,
    final_hessian_weights: BorrowedMapWeights<'_>,
    prior_weights: &[f64],
    mean: &[f64],
    absorbed_predictor: &[f64],
    retained: Option<&FeCoefficientBatch>,
    lsmr: LsmrConfig,
    map: MapConfig,
    inject_allocation_failure: bool,
) -> Result<PreparedPpmlRecoveredFixedEffects> {
    let requested = report
        .rows
        .iter()
        .filter(|effect| effect.saved_channel.is_some())
        .count();
    if requested == 0 {
        return Ok(PreparedPpmlRecoveredFixedEffects {
            report,
            saved_effects: None,
            recovery_iterations: 0,
            reconstruction_relative_infinity_norm: 0.0,
        });
    }
    let prepared = match model {
        PreparedPpmlModel::Categorical(prepared) => prepared,
        PreparedPpmlModel::CategoricalClustered(prepared) => prepared.ols(),
    };
    let rows = absorbed_predictor.len();
    if rows == 0
        || prior_weights.len() != rows
        || mean.len() != rows
        || report
            .rows
            .iter()
            .enumerate()
            .any(|(declared, effect)| effect.declared_channel != declared)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML final-Hessian FE recovery inputs are empty or misaligned",
        ));
    }
    validate_ppml_hessian_weights(final_hessian_weights, prior_weights, mean)?;

    // Recover every declared channel when any save was requested. This makes
    // the decomposition certificate independent of the publication mask.
    let all_channels = fallible_filled(
        ppml_fe_recovery_mask_length(report.rows.len(), inject_allocation_failure),
        true,
        "unable to reserve PPML complete FE recovery mask",
    )?;
    let recovered = prepared.recover_ppml_absorbed_effects_fast_with_weights(
        final_hessian_weights,
        absorbed_predictor,
        &all_channels,
        retained,
        report.connected_components == 1,
        true,
        lsmr,
        map,
    )?;
    let recovery_iterations = recovered.iterations();
    let reconstruction_relative_infinity_norm = recovered.reconstruction_relative_infinity();
    let (total, all_recovered_channels) = recovered.into_parts();
    if all_recovered_channels.len() != report.rows.len()
        || all_recovered_channels.iter().any(Option::is_none)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML complete FE recovery omitted a declared channel",
        ));
    }
    let complete_relative = complete_saved_decomposition_error(&total, &all_recovered_channels)?;
    if !reconstruction_relative_infinity_norm.is_finite()
        || reconstruction_relative_infinity_norm > map.tolerance
        || complete_relative > map.tolerance
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML final-Hessian FE recovery did not certify the complete decomposition",
        ));
    }

    // `absorbed_predictor` is already the authoritative O(N) total retained
    // by the final strict fit. The recovered total exists only long enough to
    // certify the complete decomposition; it must not cross the recovery
    // boundary and duplicate that predictor during publication.
    drop(total);

    let mut channels = Vec::new();
    let mut declared_channel_indices = Vec::new();
    channels.try_reserve_exact(requested).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to retain requested PPML saved FE channels",
        )
    })?;
    declared_channel_indices
        .try_reserve_exact(requested)
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to retain PPML saved FE channel identities",
            )
        })?;
    for (declared, (effect, channel)) in report
        .rows
        .iter_mut()
        .zip(all_recovered_channels)
        .enumerate()
    {
        if effect.saved_channel.is_some() {
            let expected_saved = channels.len();
            if effect.saved_channel != Some(expected_saved) {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML saved FE metadata is not in compact declared order",
                ));
            }
            channels.push(channel.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "PPML requested FE channel disappeared after certified recovery",
                )
            })?);
            declared_channel_indices.push(declared);
            effect.normalization_applied = true;
            effect.normalization_certified = true;
            effect.reconstruction_certified = true;
        }
    }
    Ok(PreparedPpmlRecoveredFixedEffects {
        report,
        saved_effects: Some(PpmlSavedEffectOutput {
            channels,
            declared_channels: all_channels.len(),
            declared_channel_indices,
            total_reconstruction_relative_infinity_norm: reconstruction_relative_infinity_norm,
            full_decomposition_relative_infinity_norm: complete_relative,
            full_decomposition_tolerance: map.tolerance,
            full_decomposition_certified: true,
        }),
        recovery_iterations,
        reconstruction_relative_infinity_norm,
    })
}

fn validate_ppml_hessian_weights(
    weights: BorrowedMapWeights<'_>,
    prior_weights: &[f64],
    mean: &[f64],
) -> Result<()> {
    if prior_weights.len() != mean.len() || prior_weights.is_empty() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML FE recovery prior weights and means are empty or misaligned",
        ));
    }
    if prior_weights
        .iter()
        .any(|prior| !prior.is_finite() || *prior <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML FE recovery received an invalid prepared prior weight",
        ));
    }
    if mean.iter().any(|mean| !mean.is_finite() || *mean <= 0.0) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML FE recovery received a nonpublishable final mean",
        ));
    }
    let supplied = match weights {
        BorrowedMapWeights::Unit => None,
        BorrowedMapWeights::Provided(values) if values.len() == mean.len() => Some(values),
        BorrowedMapWeights::Provided(_) => {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML final Hessian weights are not aligned to the accepted sample",
            ));
        }
    };
    for row in 0..mean.len() {
        let expected = prior_weights[row] * mean[row];
        let actual = supplied.map_or(1.0, |values| values[row]);
        if !expected.is_finite() || expected <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML final Hessian weight q*mu is not finitely representable",
            ));
        }
        if !actual.is_finite()
            || actual <= 0.0
            || relative_scalar_error(expected, actual) > 128.0 * f64::EPSILON
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML FE recovery weights are not the accepted final Hessian weights q*mu",
            ));
        }
    }
    Ok(())
}

fn complete_saved_decomposition_error(total: &[f64], channels: &[Option<Vec<f64>>]) -> Result<f64> {
    if channels.iter().any(|channel| {
        channel
            .as_ref()
            .is_none_or(|values| values.len() != total.len())
    }) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML complete FE decomposition channels are misaligned",
        ));
    }
    let mut error = 0.0_f64;
    for (row, &total_value) in total.iter().enumerate() {
        let mut sum = 0.0_f64;
        for channel in channels {
            sum += channel.as_ref().expect("validated complete channel")[row];
        }
        if !sum.is_finite() || !total_value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML complete FE decomposition is not finite",
            ));
        }
        error = error.max((sum - total_value).abs() / sum.abs().max(total_value.abs()).max(1.0));
    }
    Ok(error)
}

fn categorical_connected_components(
    prepared: &crate::prepared_categorical_data::PreparedCategoricalOlsData,
) -> Result<usize> {
    let topology = prepared.fe_topology();
    if topology.effect_to_topology().is_empty() {
        return Ok(0);
    }
    let mut source_table = Vec::new();
    source_table
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve PPML connected-component source views",
            )
        })?;
    source_table.extend(
        topology
            .compacted_sources()
            .iter()
            .map(|source| source.view()),
    );
    let mut codes = Vec::new();
    let mut levels = Vec::new();
    codes
        .try_reserve_exact(topology.effect_to_topology().len())
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve PPML connected-component code views",
            )
        })?;
    levels
        .try_reserve_exact(topology.effect_to_topology().len())
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve PPML connected-component level counts",
            )
        })?;
    for &source in topology.effect_to_topology() {
        let source = topology.registry().source(source)?;
        codes.push(source.codes(&source_table)?);
        levels.push(source.n_levels());
    }
    Ok(connected_components_all_fallible(&codes, &levels)?.n_components)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlResultCertificate {
    pub objective_deviance_relative_error: f64,
    pub objective_log_likelihood_relative_error: f64,
    pub score_passed: bool,
    pub inference_passed: bool,
    pub reconstruction_passed: bool,
    pub separation_passed: bool,
    pub generation_aligned: bool,
    pub metadata_aligned: bool,
    pub tolerance: f64,
    pub passed: bool,
}

/// Owned inputs to the single certified PPML publication boundary.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CertifiedPpmlResultInput {
    pub(crate) coefficients: PpmlCoefficientOutput,
    pub(crate) sample: PpmlFinalSample,
    pub(crate) weights: PreparedPpmlWeightReport,
    pub(crate) adjustment: PreparedPpmlAdjustmentReport,
    pub(crate) objective: PoissonObjective,
    pub(crate) inference: PpmlInferenceResult,
    pub(crate) output: PpmlOutputState,
    pub(crate) convergence: PpmlConvergenceReport,
    pub(crate) fixed_effects: Vec<PpmlFixedEffectResult>,
    pub(crate) clusters: Vec<PpmlClusterDimensionResult>,
    pub(crate) separation: SeparationReport,
    pub(crate) separation_config: SeparationConfig,
    pub(crate) separation_seal: CertifiedSeparationSeal,
    pub(crate) memory: PpmlMemoryTelemetry,
    pub(crate) solver: PpmlSolverReport,
    pub(crate) certificate_tolerance: f64,
}

/// Complete PPML result from one final sample generation. Construction is
/// atomic: no coefficient, likelihood, inference, prediction, separation, or
/// runtime product escapes unless every mandatory scientific certificate and
/// cross-generation mapping agrees.
#[derive(Debug, Clone, PartialEq)]
pub struct CertifiedPpmlResult {
    coefficients: PpmlCoefficientOutput,
    sample: PpmlFinalSample,
    weights: PreparedPpmlWeightReport,
    adjustment: PreparedPpmlAdjustmentReport,
    likelihood: PpmlLikelihoodStatistics,
    inference: PpmlInferenceResult,
    output: PpmlOutputState,
    convergence: PpmlConvergenceReport,
    fixed_effects: Vec<PpmlFixedEffectResult>,
    clusters: Vec<PpmlClusterDimensionResult>,
    separation: SeparationReport,
    separation_config: SeparationConfig,
    memory: PpmlMemoryTelemetry,
    solver: PpmlSolverReport,
    certificate: PpmlResultCertificate,
}

impl PpmlOutputState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        original_observations: usize,
        original_rows: Vec<usize>,
        offset: Vec<f64>,
        finite_predictor: Vec<f64>,
        absorbed_predictor: Vec<f64>,
        eta: Vec<f64>,
        mean: Vec<f64>,
        saved_effects: Option<PpmlSavedEffectOutput>,
        tolerance: f64,
    ) -> Result<Self> {
        let certificate = certify_ppml_reconstruction(
            original_observations,
            &original_rows,
            &offset,
            &finite_predictor,
            &absorbed_predictor,
            &eta,
            &mean,
            saved_effects.as_ref(),
            tolerance,
        )
        .map_err(|error| composed_result_error("PPML output-state construction failed", error))?;
        if !certificate.passed {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!(
                    "PPML final predictor reconstruction did not certify: predictor={:.3e}, mean={:.3e}, saved_total={:.3e}, saved_channels={:.3e}",
                    certificate.predictor_relative_infinity_norm,
                    certificate.mean_relative_infinity_norm,
                    certificate.saved_total_relative_infinity_norm,
                    certificate.saved_channels_relative_infinity_norm,
                ),
            ));
        }
        Ok(Self {
            original_observations,
            original_rows,
            offset,
            finite_predictor,
            xb: Vec::new(),
            absorbed_predictor,
            eta,
            mean,
            mean_requested: true,
            residual_kind: None,
            residual: Vec::new(),
            saved_effects,
            certificate,
        })
    }

    /// Certify against the complete strict state while retaining only the
    /// exact observation products requested by the embedding transaction.
    ///
    /// The final mean remains live only through the composite objective
    /// replay. [`CertifiedPpmlResult::retain_requested_outputs`] releases it
    /// before returning when neither `mu` nor a residual was requested.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_requested(
        original_observations: usize,
        original_rows: Vec<usize>,
        offset: &[f64],
        finite_predictor: &[f64],
        absorbed_predictor: &[f64],
        eta: &[f64],
        mean: &[f64],
        outcome: &[f64],
        saved_effects: Option<PpmlSavedEffectOutput>,
        request: super::PreparedPpmlOutputRequest,
        tolerance: f64,
    ) -> Result<Self> {
        let certificate = certify_ppml_reconstruction(
            original_observations,
            &original_rows,
            offset,
            finite_predictor,
            absorbed_predictor,
            eta,
            mean,
            saved_effects.as_ref(),
            tolerance,
        )
        .map_err(|error| composed_result_error("PPML output-state construction failed", error))?;
        if !certificate.passed {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!(
                    "PPML final predictor reconstruction did not certify: predictor={:.3e}, mean={:.3e}, saved_total={:.3e}, saved_channels={:.3e}",
                    certificate.predictor_relative_infinity_norm,
                    certificate.mean_relative_infinity_norm,
                    certificate.saved_total_relative_infinity_norm,
                    certificate.saved_channels_relative_infinity_norm,
                ),
            ));
        }

        let xb = if request.xb {
            let mut values = fallible_filled(
                offset.len(),
                0.0_f64,
                "unable to reserve requested PPML xb output",
            )?;
            for row in 0..values.len() {
                values[row] = offset[row] + finite_predictor[row];
                if !values[row].is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        "PPML requested xb output is not finite",
                    ));
                }
            }
            values
        } else {
            Vec::new()
        };
        let residual = request
            .residual
            .map(|kind| ppml_residuals(outcome, mean, kind))
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            original_observations,
            original_rows,
            offset: Vec::new(),
            finite_predictor: Vec::new(),
            xb,
            absorbed_predictor: if request.absorbed {
                fallible_copy(
                    absorbed_predictor,
                    "unable to retain requested PPML absorbed predictor",
                )?
            } else {
                Vec::new()
            },
            eta: if request.eta {
                fallible_copy(eta, "unable to retain requested PPML eta")?
            } else {
                Vec::new()
            },
            // One mean copy is the only publication-certificate O(N)
            // transient. It doubles as the requested mu vector when needed.
            mean: fallible_copy(mean, "unable to retain PPML certification mean")?,
            mean_requested: request.mean,
            residual_kind: request.residual,
            residual,
            saved_effects,
            certificate,
        })
    }

    #[must_use]
    pub const fn original_observations(&self) -> usize {
        self.original_observations
    }

    #[must_use]
    pub fn original_rows(&self) -> &[usize] {
        &self.original_rows
    }

    #[must_use]
    pub fn offset(&self) -> &[f64] {
        &self.offset
    }

    #[must_use]
    pub fn finite_predictor(&self) -> &[f64] {
        &self.finite_predictor
    }

    #[must_use]
    pub fn absorbed_predictor(&self) -> &[f64] {
        &self.absorbed_predictor
    }

    #[must_use]
    pub fn eta(&self) -> &[f64] {
        &self.eta
    }

    #[must_use]
    pub fn mean(&self) -> &[f64] {
        &self.mean
    }

    #[must_use]
    pub fn xb(&self) -> &[f64] {
        &self.xb
    }

    /// Borrow an already retained publication product without allocating a
    /// second observation vector in the plugin shadow.
    pub fn retained_prediction(&self, kind: PpmlPredictionKind) -> Result<&[f64]> {
        let values: &[f64] = match kind {
            PpmlPredictionKind::Mu if self.mean_requested => self.mean.as_slice(),
            PpmlPredictionKind::Xb => self.xb.as_slice(),
            PpmlPredictionKind::Absorbed => self.absorbed_predictor.as_slice(),
            PpmlPredictionKind::Eta => self.eta.as_slice(),
            PpmlPredictionKind::Response
            | PpmlPredictionKind::Score
            | PpmlPredictionKind::Pearson
            | PpmlPredictionKind::Working
            | PpmlPredictionKind::Deviance
            | PpmlPredictionKind::Anscombe
                if self.residual_kind == prediction_residual_kind(kind) =>
            {
                self.residual.as_slice()
            }
            _ => &[],
        };
        if values.len() != self.original_rows.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "requested PPML observation product was not retained by this transaction",
            ));
        }
        Ok(values)
    }

    #[must_use]
    pub const fn saved_effects(&self) -> Option<&PpmlSavedEffectOutput> {
        self.saved_effects.as_ref()
    }

    #[must_use]
    pub const fn certificate(&self) -> PpmlReconstructionCertificate {
        self.certificate
    }

    /// Compute a requested prediction on the compact final sample.
    ///
    /// Residual requests require the final compact outcome. Non-residual
    /// predictions ignore `outcome`, so callers need not retain it merely to
    /// publish `mu`, `eta`, `xb`, or the absorbed contribution.
    pub fn prediction(
        &self,
        kind: PpmlPredictionKind,
        outcome: Option<&[f64]>,
    ) -> Result<Vec<f64>> {
        match kind {
            PpmlPredictionKind::Mu if self.mean_requested => {
                fallible_copy(&self.mean, "unable to reserve PPML mean prediction")
            }
            PpmlPredictionKind::Absorbed if !self.absorbed_predictor.is_empty() => fallible_copy(
                &self.absorbed_predictor,
                "unable to reserve PPML absorbed prediction",
            ),
            PpmlPredictionKind::Eta if !self.eta.is_empty() => {
                fallible_copy(&self.eta, "unable to reserve PPML eta prediction")
            }
            PpmlPredictionKind::Xb => {
                if !self.xb.is_empty() {
                    return fallible_copy(&self.xb, "unable to reserve PPML xb prediction");
                }
                if self.offset.len() != self.original_rows.len()
                    || self.finite_predictor.len() != self.original_rows.len()
                {
                    return Err(FeregError::new(
                        ErrorCode::InvalidArgument,
                        "requested PPML xb product was not retained by this transaction",
                    ));
                }
                let mut output = fallible_filled(
                    self.offset.len(),
                    0.0_f64,
                    "unable to reserve PPML xb prediction",
                )?;
                for (row, value) in output.iter_mut().enumerate() {
                    *value = self.offset[row] + self.finite_predictor[row];
                    if !value.is_finite() {
                        return Err(FeregError::new(
                            ErrorCode::NonConvergence,
                            "PPML xb prediction is not finite",
                        ));
                    }
                }
                Ok(output)
            }
            PpmlPredictionKind::Response
            | PpmlPredictionKind::Score
            | PpmlPredictionKind::Pearson
            | PpmlPredictionKind::Working
            | PpmlPredictionKind::Deviance
            | PpmlPredictionKind::Anscombe => {
                if self.residual_kind == prediction_residual_kind(kind)
                    && self.residual.len() == self.original_rows.len()
                {
                    return fallible_copy(
                        &self.residual,
                        "unable to reserve retained PPML residual prediction",
                    );
                }
                let outcome = outcome.ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::InvalidArgument,
                        "a PPML residual prediction requires the final compact outcome",
                    )
                })?;
                let residual_kind = match kind {
                    PpmlPredictionKind::Response => PpmlResidualKind::Response,
                    PpmlPredictionKind::Score => PpmlResidualKind::Score,
                    PpmlPredictionKind::Pearson => PpmlResidualKind::Pearson,
                    PpmlPredictionKind::Working => PpmlResidualKind::Working,
                    PpmlPredictionKind::Deviance => PpmlResidualKind::Deviance,
                    PpmlPredictionKind::Anscombe => PpmlResidualKind::Anscombe,
                    PpmlPredictionKind::Mu
                    | PpmlPredictionKind::Xb
                    | PpmlPredictionKind::Absorbed
                    | PpmlPredictionKind::Eta => unreachable!("handled above"),
                };
                ppml_residuals(outcome, &self.mean, residual_kind)
            }
            PpmlPredictionKind::Mu | PpmlPredictionKind::Absorbed | PpmlPredictionKind::Eta => {
                Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "requested PPML observation product was not retained by this transaction",
                ))
            }
        }
    }

    /// Scatter a compact prediction back to the original observation axis.
    /// Excluded rows retain `missing_value`.
    pub fn scatter_to_original(&self, compact: &[f64], missing_value: f64) -> Result<Vec<f64>> {
        if compact.len() != self.original_rows.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML prediction is not aligned on the final compact sample",
            ));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.original_observations)
            .map_err(|_| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "unable to reserve the original-axis PPML output",
                )
            })?;
        output.resize(self.original_observations, missing_value);
        for (compact_row, &original_row) in self.original_rows.iter().enumerate() {
            output[original_row] = compact[compact_row];
        }
        Ok(output)
    }
}

impl CertifiedPpmlResult {
    pub(crate) fn new(input: CertifiedPpmlResultInput) -> Result<Self> {
        let CertifiedPpmlResultInput {
            coefficients,
            sample,
            weights,
            adjustment,
            objective,
            inference,
            output,
            convergence,
            fixed_effects,
            clusters,
            separation,
            separation_config,
            separation_seal,
            memory,
            solver,
            certificate_tolerance,
        } = input;
        if !certificate_tolerance.is_finite()
            || certificate_tolerance <= 0.0
            || certificate_tolerance >= 1.0
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML result tolerance must be finite and lie in (0, 1)",
            ));
        }
        validate_adjustment_report(adjustment)?;
        validate_weight_report(
            sample.prior_weights(),
            weights,
            sample.original_rows().len(),
        )
        .map_err(|error| composed_result_error("PPML final weight report is invalid", error))?;
        memory
            .validate()
            .map_err(|error| composed_result_error("PPML final memory report is invalid", error))?;
        solver
            .validate()
            .map_err(|error| composed_result_error("PPML final solver report is invalid", error))?;
        validate_memory_sample_axes(&sample, weights, adjustment, separation_config, &memory)?;
        separation_seal.validate(
            separation_config,
            &separation,
            sample.original_rows(),
            sample.outcome(),
        )?;
        let generation_aligned = validate_result_generation(&sample, &output, &separation)?;
        let metadata_aligned = validate_result_metadata(ResultMetadataValidation {
            coefficients: &coefficients,
            inference: &inference,
            output: &output,
            fixed_effects: &fixed_effects,
            clusters: &clusters,
            memory: &memory,
            weights,
            tolerance: certificate_tolerance,
        })?;
        validate_convergence(&convergence, certificate_tolerance)?;
        let separation_passed = validate_separation_report(
            &separation,
            separation_config,
            sample.prepared_original_rows(),
            memory
                .plan
                .dimensions
                .finite_columns
                .checked_add(memory.plan.dimensions.fe_coefficient_directions)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "PPML full separation dimension overflowed",
                    )
                })?,
            &fixed_effects,
            memory.plan.preflight_dimensions.fe_total_levels,
        )?;

        let recomputed_objective =
            poisson_objective(sample.outcome(), output.mean(), sample.prior_weights()).map_err(
                |error| composed_result_error("PPML final objective replay failed", error),
            )?;
        let objective_deviance_relative_error =
            relative_scalar_error(objective.deviance, recomputed_objective.deviance);
        let objective_log_likelihood_relative_error = relative_scalar_error(
            objective.log_likelihood,
            recomputed_objective.log_likelihood,
        );
        let likelihood = PpmlLikelihoodStatistics::from_final_state(
            objective,
            sample.outcome(),
            sample.prior_weights(),
        )
        .map_err(|error| {
            composed_result_error("PPML final likelihood publication failed", error)
        })?;
        let score_passed = convergence.final_score.passed;
        let inference_passed = inference.vce.certificate.passed && inference.vce.psd.passed;
        let reconstruction_passed = output.certificate().passed;
        let passed = objective_deviance_relative_error <= certificate_tolerance
            && objective_log_likelihood_relative_error <= certificate_tolerance
            && score_passed
            && inference_passed
            && reconstruction_passed
            && separation_passed
            && generation_aligned
            && metadata_aligned;
        if !passed {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!(
                    "PPML composite result did not certify: deviance={objective_deviance_relative_error:.3e}, likelihood={objective_log_likelihood_relative_error:.3e}, score={score_passed}, inference={inference_passed}, reconstruction={reconstruction_passed}, separation={separation_passed}, generation={generation_aligned}, metadata={metadata_aligned}",
                ),
            ));
        }
        Ok(Self {
            coefficients,
            sample,
            weights,
            adjustment,
            likelihood,
            inference,
            output,
            convergence,
            fixed_effects,
            clusters,
            separation,
            separation_config,
            memory,
            solver,
            certificate: PpmlResultCertificate {
                objective_deviance_relative_error,
                objective_log_likelihood_relative_error,
                score_passed,
                inference_passed,
                reconstruction_passed,
                separation_passed,
                generation_aligned,
                metadata_aligned,
                tolerance: certificate_tolerance,
                passed,
            },
        })
    }

    /// Release the one full mean vector retained solely for objective replay
    /// once the composite certificate has closed. Requested mu is preserved;
    /// residual publication already owns its exact final vector.
    pub(crate) fn retain_requested_outputs(mut self) -> Self {
        if !self.output.mean_requested {
            self.output.mean = Vec::new();
        }
        self
    }

    /// Attach the publication duration only after the complete atomic result
    /// constructor has certified every cross-product invariant.  This keeps
    /// the constructor inside the measured Publication phase without running
    /// any scientific validator twice.
    pub(crate) fn with_publication_observations(
        mut self,
        clock: PpmlPhaseClock,
        allocation_start: Option<PpmlAllocatorSnapshot>,
        allocation_probe: Option<PpmlAllocatorSnapshotProbe>,
        previous_baseline_bytes: Option<u64>,
        phase_allocation_probe: Option<super::PpmlPhaseAllocatorProbe>,
    ) -> Result<Self> {
        if self.memory.timings.get(PpmlPhase::Publication).is_some() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML publication timing was already recorded",
            ));
        }
        match (allocation_start, allocation_probe, previous_baseline_bytes) {
            (None, None, None) => {}
            (Some(start), Some(probe), previous_baseline_bytes) => {
                let end = probe();
                self.memory.observe_allocator(PpmlAllocationTelemetry {
                    start_bytes: Some(start.current_bytes),
                    current_bytes: Some(end.current_bytes),
                    peak_bytes: Some(end.peak_bytes),
                    previous_baseline_bytes,
                    allocation_events_start: Some(start.allocation_events),
                    allocation_events_end: Some(end.allocation_events),
                });
            }
            _ => {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML action allocation observation is incomplete",
                ));
            }
        }
        self.memory
            .record_phase(PpmlPhase::Publication, clock.elapsed());
        if let Some(bytes) =
            super::memory::end_phase_allocation(phase_allocation_probe, PpmlPhase::Publication)
        {
            self.memory
                .observe_phase_peak(PpmlPhase::Publication, bytes);
        }
        self.memory.validate().map_err(|error| {
            composed_result_error("PPML publication memory report is invalid", error)
        })?;
        Ok(self)
    }

    #[must_use]
    pub const fn coefficients(&self) -> &PpmlCoefficientOutput {
        &self.coefficients
    }

    #[must_use]
    pub const fn sample(&self) -> &PpmlFinalSample {
        &self.sample
    }

    #[must_use]
    pub const fn weights(&self) -> PreparedPpmlWeightReport {
        self.weights
    }

    #[must_use]
    pub const fn adjustment(&self) -> PreparedPpmlAdjustmentReport {
        self.adjustment
    }

    #[must_use]
    pub const fn likelihood(&self) -> PpmlLikelihoodStatistics {
        self.likelihood
    }

    #[must_use]
    pub const fn inference(&self) -> &PpmlInferenceResult {
        &self.inference
    }

    #[must_use]
    pub const fn output(&self) -> &PpmlOutputState {
        &self.output
    }

    #[must_use]
    pub const fn convergence(&self) -> &PpmlConvergenceReport {
        &self.convergence
    }

    #[must_use]
    pub fn fixed_effects(&self) -> &[PpmlFixedEffectResult] {
        &self.fixed_effects
    }

    #[must_use]
    pub fn clusters(&self) -> &[PpmlClusterDimensionResult] {
        &self.clusters
    }

    #[must_use]
    pub const fn separation(&self) -> &SeparationReport {
        &self.separation
    }

    /// Exact detector policy certified for this result.  In particular an
    /// explicitly disabled `separation(none)` remains distinguishable from
    /// the default FE/simplex/IR chain even when neither path drops a row.
    #[must_use]
    pub const fn separation_config(&self) -> SeparationConfig {
        self.separation_config
    }

    #[must_use]
    pub const fn memory(&self) -> &PpmlMemoryTelemetry {
        &self.memory
    }

    #[must_use]
    pub const fn solver(&self) -> PpmlSolverReport {
        self.solver
    }

    #[must_use]
    pub const fn certificate(&self) -> PpmlResultCertificate {
        self.certificate
    }

    /// Compute any requested prediction/residual from this exact final
    /// generation; the publication layer never needs to rejoin row products.
    pub fn prediction(&self, kind: PpmlPredictionKind) -> Result<Vec<f64>> {
        self.output.prediction(kind, Some(self.sample.outcome()))
    }

    pub fn scatter_prediction(
        &self,
        kind: PpmlPredictionKind,
        missing_value: f64,
    ) -> Result<Vec<f64>> {
        let compact = self.prediction(kind)?;
        self.output.scatter_to_original(&compact, missing_value)
    }
}

fn validate_memory_sample_axes(
    sample: &PpmlFinalSample,
    weights: PreparedPpmlWeightReport,
    adjustment: PreparedPpmlAdjustmentReport,
    separation: SeparationConfig,
    memory: &PpmlMemoryTelemetry,
) -> Result<()> {
    let dimensions = memory.plan.dimensions;
    let retained_base_adjustment = if adjustment.input_kind == PreparedPpmlAdjustmentKind::None {
        0
    } else {
        sample.candidate_observations()
    };
    let retained_raw_probability_weights = if weights.kind == Some(WeightKind::Probability) {
        sample.prepared_original_rows().len()
    } else {
        0
    };
    let workspace = separation_workspace_requirements(separation);
    let rebase_enabled = separation.drop_singletons
        || separation.methods.fixed_effects
        || separation.methods.simplex
        || separation.methods.iterated_rectifier
        || separation.methods.mu;
    let policy_matches = |dimensions: crate::ppml::PpmlMemoryDimensions| {
        dimensions.separation_workspace_vectors == workspace.observation_vectors
            && dimensions.separation_singleton_enabled == separation.drop_singletons
            && dimensions.separation_method_count == separation_method_count(separation)
            && dimensions.separation_rebase_enabled == rebase_enabled
            && dimensions.separation_exact_fe_enabled == separation.methods.fixed_effects
            && dimensions.separation_mu_enabled == separation.methods.mu
            && dimensions.simplex_tableau_cells == workspace.simplex_tableau_cells
    };
    if dimensions.original_observations != sample.original_observations()
        || dimensions.candidate_observations != sample.candidate_observations()
        || dimensions.observations != sample.original_rows().len()
        || memory.plan.preflight_dimensions.observations != sample.prepared_original_rows().len()
        || dimensions.retained_base_adjustment_capacity_elements != retained_base_adjustment
        || memory
            .plan
            .preflight_dimensions
            .retained_base_adjustment_capacity_elements
            != retained_base_adjustment
        || dimensions.retained_raw_preseparation_probability_weight_elements
            != retained_raw_probability_weights
        || memory
            .plan
            .preflight_dimensions
            .retained_raw_preseparation_probability_weight_elements
            != retained_raw_probability_weights
        || !policy_matches(dimensions)
        || !policy_matches(memory.plan.preflight_dimensions)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML frozen memory-route axes or retained pre-separation adjustment/weight capacity disagree with the certified final sample",
        ));
    }
    Ok(())
}

/// Reclassify failures after raw-domain preparation has completed. At this
/// boundary, caller data have already been validated and every product is
/// estimator-owned. Numerical nonpublication, allocation, and interruption
/// keep their registered public codes; malformed composed artifacts are an
/// internal invariant failure rather than a second user-domain error.
fn composed_result_error(context: &'static str, error: FeregError) -> FeregError {
    match error.code {
        ErrorCode::Allocation | ErrorCode::Interrupted | ErrorCode::NonConvergence => error,
        ErrorCode::Internal | ErrorCode::Panic => error,
        _ => FeregError::new(ErrorCode::Internal, format!("{context}: {}", error.message)),
    }
}

fn validate_adjustment_report(report: PreparedPpmlAdjustmentReport) -> Result<()> {
    let valid = match report.input_kind {
        PreparedPpmlAdjustmentKind::None => {
            !report.prepared_as_offset && !report.exposure_log_transformed
        }
        PreparedPpmlAdjustmentKind::Offset => {
            report.prepared_as_offset && !report.exposure_log_transformed
        }
        PreparedPpmlAdjustmentKind::Exposure => {
            report.prepared_as_offset && report.exposure_log_transformed
        }
    };
    if !valid {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML adjustment report is inconsistent with its canonical preparation",
        ));
    }
    Ok(())
}

fn validate_convergence(report: &PpmlConvergenceReport, tolerance: f64) -> Result<()> {
    let score = &report.final_score;
    let score_values = [
        score.finite_absolute_infinity_norm,
        score.fixed_effect_absolute_infinity_norm,
        score.relative_infinity_norm,
        score.relative_two_norm,
    ];
    let score_within_tolerance =
        score.relative_infinity_norm <= tolerance && score.relative_two_norm <= tolerance;
    if !report.converged
        || !report.two_consecutive_convergence
        || !report.rank_frozen
        || report.outer_iterations < 2
        || report.inner_iterations < report.outer_iterations
        || !report.strict_final_solve_passed
        || !report.linear_within_relative_infinity_norm.is_finite()
        || !report
            .linear_reconstruction_relative_infinity_norm
            .is_finite()
        || report.linear_within_relative_infinity_norm < 0.0
        || report.linear_reconstruction_relative_infinity_norm < 0.0
        || report.linear_within_relative_infinity_norm > tolerance
        || report.linear_reconstruction_relative_infinity_norm > tolerance
        || score_values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        || score.relative_two_norm > score.relative_infinity_norm + 16.0 * f64::EPSILON
        || score.passed != score_within_tolerance
        || !score_within_tolerance
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML convergence/strict-final certificate is not publishable",
        ));
    }
    Ok(())
}

fn validate_result_generation(
    sample: &PpmlFinalSample,
    output: &PpmlOutputState,
    separation: &SeparationReport,
) -> Result<bool> {
    if output.original_observations() != sample.original_observations()
        || output.original_rows() != sample.original_rows()
        || separation.candidate_observations != sample.prepared_original_rows().len()
        || separation.active.len() != separation.candidate_observations
        || separation.drop_reasons.len() != separation.candidate_observations
        || separation.retained_observations.len() != sample.original_rows().len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML result products do not share one final sample generation",
        ));
    }
    if separation
        .retained_observations
        .iter()
        .zip(sample.original_rows())
        .any(|(prepared_row, original_row)| {
            sample.prepared_original_rows().get(*prepared_row) != Some(original_row)
        })
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation and output rows refer to different original observations",
        ));
    }
    let mut prepared = fallible_filled(
        sample.original_observations(),
        false,
        "unable to reserve PPML prepared-row validation state",
    )?;
    for (candidate_row, &original_row) in sample.prepared_original_rows().iter().enumerate() {
        prepared[original_row] = true;
        let expected = match separation.drop_reasons[candidate_row] {
            None => DropReason::Retained,
            Some(reason) => result_drop_reason(reason),
        };
        if sample.drop_reasons()[original_row] != expected {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML separation reasons do not map to the final original-axis drop ledger",
            ));
        }
    }
    for (original_row, reason) in sample.drop_reasons().iter().enumerate() {
        if !prepared[original_row]
            && matches!(
                reason,
                DropReason::Retained
                    | DropReason::Singleton
                    | DropReason::FixedEffectSeparation
                    | DropReason::SimplexSeparation
                    | DropReason::IteratedRectifierSeparation
                    | DropReason::MuSeparation
            )
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML preprocessing row carries a final separation/retention reason",
            ));
        }
    }
    Ok(true)
}

const fn result_drop_reason(reason: crate::ppml_separation::SeparationDropReason) -> DropReason {
    match reason {
        crate::ppml_separation::SeparationDropReason::InitialSingleton
        | crate::ppml_separation::SeparationDropReason::SingletonCascade => DropReason::Singleton,
        crate::ppml_separation::SeparationDropReason::FixedEffectZeroLevel => {
            DropReason::FixedEffectSeparation
        }
        crate::ppml_separation::SeparationDropReason::Simplex => DropReason::SimplexSeparation,
        crate::ppml_separation::SeparationDropReason::IteratedRectifier => {
            DropReason::IteratedRectifierSeparation
        }
        crate::ppml_separation::SeparationDropReason::Mu => DropReason::MuSeparation,
    }
}

struct ResultMetadataValidation<'a> {
    coefficients: &'a PpmlCoefficientOutput,
    inference: &'a PpmlInferenceResult,
    output: &'a PpmlOutputState,
    fixed_effects: &'a [PpmlFixedEffectResult],
    clusters: &'a [PpmlClusterDimensionResult],
    memory: &'a PpmlMemoryTelemetry,
    weights: PreparedPpmlWeightReport,
    tolerance: f64,
}

fn validate_result_metadata(validation: ResultMetadataValidation<'_>) -> Result<bool> {
    let ResultMetadataValidation {
        coefficients,
        inference,
        output,
        fixed_effects,
        clusters,
        memory,
        weights,
        tolerance,
    } = validation;
    let dimension = coefficients.values().len();
    let square = dimension.checked_mul(dimension).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            "PPML result covariance dimensions overflow",
        )
    })?;
    let normalized_constants = coefficients
        .statuses()
        .iter()
        .filter(|status| **status == PpmlCoefficientStatus::NormalizedStructuralConstant)
        .count();
    let expected_finite_rank = coefficients
        .kept_full_slots()
        .len()
        .checked_add(normalized_constants)
        .ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "PPML reported finite rank overflows")
        })?;
    let saved_channels = output
        .saved_effects()
        .map_or(0, |saved| saved.channels.len());
    if inference.vce.covariance.len() != square
        || inference.vce.standard_errors.len() != dimension
        || inference.vce.z_statistics.len() != dimension
        || normalized_constants > 1
        || inference.dof.sandwich_rank != coefficients.kept_full_slots().len()
        || inference.dof.finite_within_rank != expected_finite_rank
        || memory.plan.dimensions.reported_coefficient_slots != dimension
        || memory.plan.dimensions.observations != output.original_rows().len()
        || memory.plan.dimensions.observations != weights.physical_observations
        || memory.plan.dimensions.fe_total_channels != fixed_effects.len()
        || memory.plan.dimensions.requested_saved_fe_channels != saved_channels
        || memory.plan.dimensions.cluster_dimensions != clusters.len()
        || memory.plan.dimensions.threads != memory.effective_threads
        || memory.effective_threads == 0
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML coefficient, inference, and memory metadata are dimensionally inconsistent",
        ));
    }
    validate_generated_column_mapping(coefficients, memory.plan.dimensions.finite_columns)?;
    validate_dof_and_vce_topology(coefficients, inference, clusters, weights, tolerance)?;
    validate_covariance_and_certificates(coefficients, inference, tolerance)?;
    validate_wald(coefficients, inference, tolerance)?;
    let mut identified = fallible_filled(
        dimension,
        false,
        "unable to reserve PPML result inference-slot validation state",
    )?;
    for &slot in coefficients.kept_full_slots() {
        identified[slot] = true;
    }
    for (slot, (&standard_error, &z)) in inference
        .vce
        .standard_errors
        .iter()
        .zip(&inference.vce.z_statistics)
        .enumerate()
    {
        if identified[slot] {
            if !standard_error.is_finite() || standard_error < 0.0 {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML identified coefficient has an invalid standard error",
                ));
            }
            if standard_error == 0.0 {
                if !z.is_nan() {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML zero-variance identified coefficient must have missing z inference",
                    ));
                }
            } else if !z.is_finite()
                || relative_scalar_error(coefficients.values()[slot] / standard_error, z)
                    > tolerance
            {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML z statistic does not match the certified coefficient and standard error",
                ));
            }
        } else if !standard_error.is_nan() || !z.is_nan() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML omitted coefficient inference must use explicit missing values",
            ));
        }
        if !identified[slot] {
            for other in 0..dimension {
                if inference.vce.covariance[slot * dimension + other] != 0.0
                    || inference.vce.covariance[other * dimension + slot] != 0.0
                {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML omitted covariance rows and columns must be exactly zero",
                    ));
                }
            }
        }
    }
    validate_fixed_effect_metadata(
        fixed_effects,
        output,
        inference.dof.absorbed_rank,
        coefficients,
    )?;
    let expected_approximation = fixed_effects.iter().any(|effect| !effect.redundancy_exact);
    if inference.dof.absorbed_rank_is_approximate != expected_approximation
        || (fixed_effects.is_empty()
            && (inference.dof.absorbed_rank != 0 || inference.dof.connected_components != 0))
        || (!fixed_effects.is_empty() && inference.dof.connected_components == 0)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML absorbed-rank approximation/topology metadata is inconsistent",
        ));
    }
    validate_cluster_metadata(clusters, inference)?;
    Ok(true)
}

fn validate_generated_column_mapping(
    coefficients: &PpmlCoefficientOutput,
    finite_columns: usize,
) -> Result<()> {
    let mut seen = fallible_filled(
        finite_columns,
        false,
        "unable to reserve PPML generated-column validation state",
    )?;
    for generated in coefficients.generated_columns().iter().copied().flatten() {
        if generated >= finite_columns || seen[generated] {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML generated-column identities are duplicated or out of range",
            ));
        }
        seen[generated] = true;
    }
    if seen.iter().any(|present| !present) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML generated-column identities do not cover the prepared finite design",
        ));
    }
    let mut previous = None;
    for &slot in coefficients.kept_full_slots() {
        let generated = coefficients.generated_columns()[slot].ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML identified coefficient lacks a generated-column identity",
            )
        })?;
        if previous.is_some_and(|prior| prior >= generated) {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML compact inference slots are not in frozen generated-column order",
            ));
        }
        previous = Some(generated);
    }
    Ok(())
}

fn validate_dof_and_vce_topology(
    coefficients: &PpmlCoefficientOutput,
    inference: &PpmlInferenceResult,
    clusters: &[PpmlClusterDimensionResult],
    weights: PreparedPpmlWeightReport,
    tolerance: f64,
) -> Result<()> {
    let dof = &inference.dof;
    let vce = &inference.vce;
    let model_rank = dof
        .finite_within_rank
        .checked_add(dof.absorbed_rank)
        .ok_or_else(|| FeregError::new(ErrorCode::Internal, "PPML result rank overflows"))?;
    if dof.effective_observations != weights.effective_observations
        || dof.sandwich_rank != coefficients.kept_full_slots().len()
        || dof.model_rank != model_rank
        || model_rank >= dof.effective_observations
        || dof.structural_residual != dof.effective_observations - model_rank
        || dof.reported == 0
        || vce.coefficient_distribution != PpmlTestDistribution::StandardNormal
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML DoF/rank/test-distribution metadata is internally inconsistent",
        ));
    }

    let robust = matches!(
        vce.requested,
        PpmlVceRequest::Unadjusted | PpmlVceRequest::Robust
    );
    if robust {
        if vce.effective != PpmlVceKind::Robust
            || !clusters.is_empty()
            || !vce.cluster_dimension_groups.is_empty()
            || !vce.cluster_subsets.is_empty()
            || dof.minimum_cluster_groups.is_some()
            || dof.reported != dof.structural_residual
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "robust PPML inference contains clustered topology or wrong reported DoF",
            ));
        }
        let expected = dof.effective_observations as f64 / (dof.effective_observations - 1) as f64;
        if relative_scalar_error(vce.correction, expected) > 16.0 * f64::EPSILON {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "robust PPML covariance has the wrong asymptotic correction",
            ));
        }
        return Ok(());
    }

    let dimensions = vce.cluster_dimension_groups.len();
    if vce.requested != PpmlVceRequest::Clustered
        || vce.effective != PpmlVceKind::Clustered
        || !(1..=5).contains(&dimensions)
        || clusters.len() != dimensions
        || vce
            .cluster_dimension_groups
            .iter()
            .any(|groups| *groups < 2)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "clustered PPML covariance has an invalid declared topology",
        ));
    }
    let expected_subsets = (1_usize << dimensions) - 1;
    if vce.cluster_subsets.len() != expected_subsets {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "clustered PPML covariance lacks the complete subset lattice",
        ));
    }
    let mut position = 0_usize;
    for cardinality in 1..=dimensions {
        for mask in 1..=expected_subsets {
            if mask.count_ones() as usize != cardinality {
                continue;
            }
            let report = &vce.cluster_subsets[position];
            let expected_mask = u16::try_from(mask).map_err(|_| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "PPML cluster subset mask overflows u16",
                )
            })?;
            let expected_sign = if cardinality % 2 == 1 { 1 } else { -1 };
            if report.mask != expected_mask
                || report.dimensions.len() != cardinality
                || !report
                    .dimensions
                    .iter()
                    .copied()
                    .eq((0..dimensions).filter(|dimension| mask & (1_usize << dimension) != 0))
                || report.sign != expected_sign
                || report.groups < 2
            {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML cluster subset identity/order/sign/group count is invalid",
                ));
            }
            if cardinality == 1 {
                let dimension = report.dimensions[0];
                if report.groups != vce.cluster_dimension_groups[dimension]
                    || report.groups != clusters[dimension].groups
                {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML singleton cluster subset disagrees with its declared dimension",
                    ));
                }
            }
            position += 1;
        }
    }
    let minimum = *vce
        .cluster_dimension_groups
        .iter()
        .min()
        .expect("validated nonempty PPML cluster topology");
    let expected_reported = dof.structural_residual.min(minimum - 1);
    let expected_correction = minimum as f64 / (minimum - 1) as f64;
    if dof.minimum_cluster_groups != Some(minimum)
        || dof.reported != expected_reported
        || relative_scalar_error(vce.correction, expected_correction) > 16.0 * f64::EPSILON
        || !tolerance.is_finite()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "clustered PPML DoF or common Gmin correction is inconsistent",
        ));
    }
    Ok(())
}

fn validate_covariance_and_certificates(
    coefficients: &PpmlCoefficientOutput,
    inference: &PpmlInferenceResult,
    tolerance: f64,
) -> Result<()> {
    let dimension = coefficients.values().len();
    let rank = coefficients.kept_full_slots().len();
    let covariance = &inference.vce.covariance;
    let scale = covariance
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    if covariance.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML published covariance contains a nonfinite entry",
        ));
    }
    let mut symmetry = 0.0_f64;
    for row in 0..dimension {
        for column in 0..dimension {
            symmetry = symmetry.max(
                (covariance[row * dimension + column] - covariance[column * dimension + row]).abs()
                    / scale,
            );
        }
    }
    if symmetry > tolerance {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML published covariance is not symmetric",
        ));
    }
    for (compact, &slot) in coefficients.kept_full_slots().iter().enumerate() {
        let variance = covariance[slot * dimension + slot];
        let standard_error = inference.vce.standard_errors[slot];
        if variance < 0.0 || relative_scalar_error(variance.sqrt(), standard_error) > tolerance {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!(
                    "PPML standard error does not match covariance diagonal at identified coordinate {compact}"
                ),
            ));
        }
    }

    let mut identified = fallible_filled(
        rank.checked_mul(rank).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "PPML identified covariance overflows",
            )
        })?,
        0.0_f64,
        "unable to reserve PPML identified covariance validation matrix",
    )?;
    for (row, &source_row) in coefficients.kept_full_slots().iter().enumerate() {
        for (column, &source_column) in coefficients.kept_full_slots().iter().enumerate() {
            identified[row * rank + column] = covariance[source_row * dimension + source_column];
        }
    }
    let cluster_dimensions = inference.vce.cluster_dimension_groups.len();
    let (repaired, recomputed_psd) =
        inspect_published_ppml_covariance(&identified, rank, cluster_dimensions, tolerance)?;
    if matrix_relative_error(&identified, &repaired)? > tolerance {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML published covariance still requires PSD repair",
        ));
    }
    let psd = inference.vce.psd;
    let state_valid = matches!(
        (cluster_dimensions, psd.state),
        (0 | 1, PpmlPsdState::Strict)
            | (
                2..,
                PpmlPsdState::MultiwayRaw | PpmlPsdState::MultiwayClipped
            )
    );
    let report_values_valid = [
        psd.raw_minimum_eigenvalue,
        psd.final_minimum_eigenvalue,
        psd.eigensystem_reconstruction_relative_error,
        psd.orthogonality_relative_error,
        psd.clipped_reconstruction_relative_error,
    ]
    .iter()
    .all(|value| value.is_finite());
    if !state_valid
        || !report_values_valid
        || !psd.passed
        || psd.final_rank != recomputed_psd.rank
        || psd.final_rank > rank
        || psd.clipped_eigenvalues > rank
        || psd.eigensystem_reconstruction_relative_error < 0.0
        || psd.orthogonality_relative_error < 0.0
        || psd.clipped_reconstruction_relative_error < 0.0
        || psd.eigensystem_reconstruction_relative_error > tolerance
        || psd.orthogonality_relative_error > tolerance
        || psd.clipped_reconstruction_relative_error > tolerance
        || psd.final_minimum_eigenvalue < -tolerance * scale
        || psd.adjusted != (psd.state == PpmlPsdState::MultiwayClipped)
        || psd.adjusted != (psd.clipped_eigenvalues > 0)
        || (psd.state == PpmlPsdState::MultiwayClipped
            && (psd.raw_minimum_eigenvalue >= 0.0
                || psd.clipped_eigenvalues == 0
                || psd.final_rank > rank - psd.clipped_eigenvalues))
        || (psd.state == PpmlPsdState::MultiwayRaw
            && (psd.raw_minimum_eigenvalue < 0.0 || psd.clipped_eigenvalues != 0))
        || (psd.state == PpmlPsdState::Strict && psd.clipped_eigenvalues != 0)
        || psd.eigensystem_reconstruction_relative_error + 16.0 * f64::EPSILON
            < recomputed_psd.reconstruction_relative_error
        || psd.orthogonality_relative_error + 16.0 * f64::EPSILON
            < recomputed_psd.orthogonality_relative_error
        || (psd.state != PpmlPsdState::MultiwayClipped
            && relative_scalar_error(
                psd.raw_minimum_eigenvalue,
                recomputed_psd.minimum_eigenvalue,
            ) > tolerance)
        || relative_scalar_error(
            psd.final_minimum_eigenvalue,
            recomputed_psd.minimum_eigenvalue,
        ) > tolerance
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML covariance PSD report is inconsistent with the published matrix",
        ));
    }

    let certificate = inference.vce.certificate;
    let certificate_errors = [
        certificate.bread_symmetry_relative_infinity_norm,
        certificate.meat_relative_infinity_norm,
        certificate.covariance_relative_infinity_norm,
        certificate.covariance_symmetry_relative_infinity_norm,
        certificate.wald_solve_relative_infinity_norm,
    ];
    if certificate.tolerance != tolerance
        || !certificate.passed
        || certificate_errors
            .iter()
            .any(|error| !error.is_finite() || *error < 0.0 || *error > certificate.tolerance)
        || relative_scalar_error(
            certificate.covariance_symmetry_relative_infinity_norm,
            symmetry,
        ) > tolerance
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML inference certificate fields do not certify the published covariance",
        ));
    }
    Ok(())
}

fn validate_wald(
    coefficients: &PpmlCoefficientOutput,
    inference: &PpmlInferenceResult,
    tolerance: f64,
) -> Result<()> {
    let wald = &inference.wald;
    let dimension = coefficients.values().len();
    let mut identified = fallible_filled(
        dimension,
        false,
        "unable to reserve PPML Wald identified-slot validation state",
    )?;
    for &slot in coefficients.kept_full_slots() {
        identified[slot] = true;
    }
    if wald.distribution != PpmlTestDistribution::ChiSquared
        || wald.degrees_of_freedom != wald.tested_columns.len()
        || wald
            .tested_columns
            .iter()
            .enumerate()
            .any(|(position, column)| {
                *column >= dimension
                    || !identified[*column]
                    || (position > 0 && wald.tested_columns[position - 1] >= *column)
            })
        || (wald.degrees_of_freedom == 0 && wald.statistic.is_some())
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML Wald chi-squared identity metadata is inconsistent",
        ));
    }
    if !wald.solve_relative_infinity_norm.is_finite()
        || wald.solve_relative_infinity_norm < 0.0
        || wald.solve_relative_infinity_norm > tolerance
        || relative_scalar_error(
            wald.solve_relative_infinity_norm,
            inference.vce.certificate.wald_solve_relative_infinity_norm,
        ) > tolerance
        || wald
            .statistic
            .is_some_and(|statistic| !statistic.is_finite() || statistic < 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML Wald chi-squared numerical evidence is not publishable",
        ));
    }
    let recomputed = compute_ppml_wald(
        coefficients.values(),
        &inference.vce.covariance,
        dimension,
        &wald.tested_columns,
        tolerance,
    )
    .map_err(|error| composed_result_error("PPML Wald replay failed", error))?;
    let statistic_matches = match (wald.statistic, recomputed.statistic) {
        (None, None) => true,
        (Some(left), Some(right)) => relative_scalar_error(left, right) <= tolerance,
        _ => false,
    };
    if !statistic_matches
        || recomputed.degrees_of_freedom != wald.degrees_of_freedom
        || relative_scalar_error(
            recomputed.solve_relative_infinity_norm,
            wald.solve_relative_infinity_norm,
        ) > tolerance
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML Wald statistic does not match coefficients and covariance",
        ));
    }
    Ok(())
}

fn matrix_relative_error(left: &[f64], right: &[f64]) -> Result<f64> {
    if left.len() != right.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML matrix certificate dimensions are inconsistent",
        ));
    }
    let scale = left
        .iter()
        .chain(right)
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    Ok(left
        .iter()
        .zip(right)
        .map(|(left, right)| (left - right).abs() / scale)
        .fold(0.0_f64, f64::max))
}

fn validate_fixed_effect_metadata(
    fixed_effects: &[PpmlFixedEffectResult],
    output: &PpmlOutputState,
    absorbed_rank: usize,
    coefficients: &PpmlCoefficientOutput,
) -> Result<()> {
    let normalized_constants = coefficients
        .statuses()
        .iter()
        .filter(|status| **status == PpmlCoefficientStatus::NormalizedStructuralConstant)
        .count();
    if normalized_constants > 1 {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML result contains multiple normalized structural constants",
        ));
    }
    let saved_effects = output.saved_effects();
    let saved_channels = saved_effects.map_or(0, |saved| saved.channels.len());
    if saved_effects.is_some_and(|saved| saved.declared_channels != fixed_effects.len()) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML saved-effect declaration count does not match fixed-effect metadata",
        ));
    }
    let mut seen_saved = fallible_filled(
        saved_channels,
        false,
        "unable to reserve PPML saved-effect metadata validation state",
    )?;
    let mut reported_absorbed_rank = 0_usize;
    for (position, effect) in fixed_effects.iter().enumerate() {
        if effect.declared_channel != position
            || effect.term_id == 0
            || effect.source_ids.is_empty()
            || effect.source_ids.contains(&0)
            || match effect.kind {
                PpmlFixedEffectChannelKind::Intercept => effect.slope_source_id.is_some(),
                PpmlFixedEffectChannelKind::Slope => {
                    effect.slope_source_id.is_none_or(|source| source == 0)
                }
            }
            || effect.levels == 0
            || effect.categories == 0
            || effect.levels != effect.categories
            || effect.redundant_parameters > effect.categories
            || effect.absorbed_rank != effect.categories - effect.redundant_parameters
            || (effect.nested
                && (!effect.redundancy_exact
                    || effect.redundant_parameters != effect.categories
                    || effect.absorbed_rank != 0))
            || effect.saved_channel.is_some() != effect.normalization_applied
            || effect.saved_channel.is_some()
                && (!effect.normalization_certified || !effect.reconstruction_certified)
            || effect.saved_channel.is_none()
                && (effect.normalization_applied
                    || effect.normalization_certified
                    || effect.reconstruction_certified)
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML fixed-effect metadata is incomplete or uncertified",
            ));
        }
        reported_absorbed_rank = reported_absorbed_rank
            .checked_add(effect.absorbed_rank)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "PPML per-channel absorbed rank overflows",
                )
            })?;
        if let Some(saved) = effect.saved_channel {
            if saved >= saved_channels || seen_saved[saved] {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML saved fixed-effect channel mapping is duplicated or out of range",
                ));
            }
            if saved_effects.and_then(|effects| effects.declared_channel_indices.get(saved))
                != Some(&effect.declared_channel)
            {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML saved fixed-effect channel does not retain its declared identity",
                ));
            }
            seen_saved[saved] = true;
        }
    }
    if seen_saved.iter().any(|seen| !seen) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML saved fixed-effect output lacks declared channel metadata",
        ));
    }
    if reported_absorbed_rank != absorbed_rank {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML per-channel absorbed ranks do not equal the inference rank",
        ));
    }
    Ok(())
}

fn validate_cluster_metadata(
    clusters: &[PpmlClusterDimensionResult],
    inference: &PpmlInferenceResult,
) -> Result<()> {
    if clusters.len() != inference.vce.cluster_dimension_groups.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML cluster dimension metadata does not match covariance topology",
        ));
    }
    for (dimension, cluster) in clusters.iter().enumerate() {
        if cluster.declared_dimension != dimension
            || cluster.label.is_empty()
            || cluster.source_ids.is_empty()
            || cluster.source_ids.contains(&0)
            || cluster.groups != inference.vce.cluster_dimension_groups[dimension]
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML declared cluster metadata is incomplete or has the wrong group count",
            ));
        }
    }
    Ok(())
}

fn validate_separation_report(
    report: &SeparationReport,
    config: SeparationConfig,
    candidate_original_rows: &[usize],
    full_model_columns: usize,
    fixed_effects: &[PpmlFixedEffectResult],
    exact_level_capacity: usize,
) -> Result<bool> {
    validate_separation_config(config)?;
    let method_capacity = report
        .candidate_observations
        .checked_mul(separation_method_count(config))
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML separation method-report capacity overflows",
            )
        })?;
    if report.active.len() != report.candidate_observations
        || report.drop_reasons.len() != report.candidate_observations
        || candidate_original_rows.len() != report.candidate_observations
        || candidate_original_rows
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
        || report.generations >= report.candidate_observations
        || report.generation_hashes.len() != report.generations.saturating_add(1)
        || report.generation_hashes.len() > report.candidate_observations
        || report.methods.len() > method_capacity
        || report.retained_observations.len() + report.dropped_observations.len()
            != report.candidate_observations
        || report.singleton_count + report.separated_count != report.dropped_observations.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation report has an inconsistent final ledger",
        ));
    }
    let valid_stable_rows = |rows: &[usize]| {
        rows.iter().enumerate().all(|(position, row)| {
            *row < report.candidate_observations && (position == 0 || rows[position - 1] < *row)
        })
    };
    if !valid_stable_rows(&report.retained_observations)
        || !valid_stable_rows(&report.dropped_observations)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation retained/dropped rows are not unique stable-order ledgers",
        ));
    }
    let observed_singletons = report
        .drop_reasons
        .iter()
        .filter(|reason| {
            matches!(
                reason,
                Some(
                    SeparationDropReason::InitialSingleton | SeparationDropReason::SingletonCascade
                )
            )
        })
        .count();
    let observed_separated = report
        .drop_reasons
        .iter()
        .filter(|reason| {
            matches!(
                reason,
                Some(
                    SeparationDropReason::FixedEffectZeroLevel
                        | SeparationDropReason::Simplex
                        | SeparationDropReason::IteratedRectifier
                        | SeparationDropReason::Mu
                )
            )
        })
        .count();
    if observed_singletons != report.singleton_count || observed_separated != report.separated_count
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation reason counts do not equal singleton/separated totals",
        ));
    }
    for row in 0..report.candidate_observations {
        let retained = report.retained_observations.binary_search(&row).is_ok();
        let dropped = report.dropped_observations.binary_search(&row).is_ok();
        if retained == dropped
            || report.active[row] != retained
            || report.drop_reasons[row].is_some() != dropped
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML separation active mask, row lists, and reasons disagree",
            ));
        }
    }
    let mut globally_reported = fallible_filled(
        report.candidate_observations,
        false,
        "unable to reserve PPML separation method-ledger validation state",
    )?;
    validate_requested_separation_chain(report, config)?;
    let mut previous_generation = 0_usize;
    let mut exact_level_events = 0_usize;
    for method in &report.methods {
        if method.generation > report.generations || method.generation < previous_generation {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML separation method generations are out of order or range",
            ));
        }
        previous_generation = method.generation;
        exact_level_events = exact_level_events
            .checked_add(validate_separation_method(
                method,
                candidate_original_rows,
                full_model_columns,
                fixed_effects,
            )?)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "PPML exact-FE certificate event count overflows",
                )
            })?;
        if exact_level_events > exact_level_capacity {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML exact-FE certificates exceed the generation-zero level envelope",
            ));
        }
        for &row in method
            .dropped_observations
            .iter()
            .chain(&method.singleton_drops)
        {
            if globally_reported[row] {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML separation row is attributed to multiple method reports",
                ));
            }
            globally_reported[row] = true;
        }
        for &row in &method.dropped_observations {
            if report.drop_reasons[row] != Some(method_primary_reason(method.method)) {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML separation method row has the wrong final drop reason",
                ));
            }
        }
        let singleton_reason = if method.method == SeparationMethod::Singletons {
            SeparationDropReason::InitialSingleton
        } else {
            SeparationDropReason::SingletonCascade
        };
        if method
            .singleton_drops
            .iter()
            .any(|row| report.drop_reasons[*row] != Some(singleton_reason))
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML method singleton cascade has the wrong final drop reason",
            ));
        }
    }
    if report
        .dropped_observations
        .iter()
        .any(|row| !globally_reported[*row])
        || globally_reported
            .iter()
            .enumerate()
            .any(|(row, reported)| *reported == report.active[row])
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation methods do not partition the final dropped ledger",
        ));
    }

    replay_separation_generation_hashes(report, candidate_original_rows)?;
    Ok(true)
}

fn validate_separation_config(config: SeparationConfig) -> Result<()> {
    if [
        config.tolerance,
        config.certificate_tolerance,
        config.projection_tolerance,
        config.mu_tolerance,
    ]
    .iter()
    .any(|value| !value.is_finite() || *value <= 0.0 || *value >= 1.0)
        || config.max_ir_iterations == 0
        || config.simplex_max_iterations == 0
        || config.simplex_max_tableau_cells == 0
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML certified separation request has an invalid tolerance, iteration, or memory policy",
        ));
    }
    Ok(())
}

/// Validate the exact detector state machine requested by the caller.
///
/// A report carrying accepted drops is stamped with the committed generation,
/// so its inspected generation is one smaller.  No-drop reports are stamped
/// with the generation they inspected.  Making that distinction explicit is
/// what prevents a later-generation IR result from retroactively closing an
/// unresolved simplex result.  An IR deletion may make the old attempt
/// nonpublishing, but the restarted generation must execute the entire chain
/// again and obtain its own terminal same-generation closure.
fn validate_requested_separation_chain(
    report: &SeparationReport,
    config: SeparationConfig,
) -> Result<()> {
    let mut position = 0_usize;
    for inspected_generation in 0..=report.generations {
        let mut simplex_unresolved = false;

        if config.drop_singletons {
            let execution = take_primary_execution(
                report,
                &mut position,
                inspected_generation,
                SeparationMethod::Singletons,
            )?;
            if execution_commits(execution) {
                validate_primary_commit(execution, inspected_generation)?;
                finish_committed_attempt(
                    report,
                    &mut position,
                    inspected_generation,
                    config.methods,
                )?;
                continue;
            }
            if execution.status != MethodStatus::NoSeparation {
                return Err(chain_error(
                    "singleton policy did not report its stable fixed point",
                ));
            }
        }

        if config.methods.fixed_effects {
            let execution = take_primary_execution(
                report,
                &mut position,
                inspected_generation,
                SeparationMethod::FixedEffects,
            )?;
            if execution_commits(execution) {
                validate_primary_commit(execution, inspected_generation)?;
                finish_committed_attempt(
                    report,
                    &mut position,
                    inspected_generation,
                    config.methods,
                )?;
                continue;
            }
            if execution.status != MethodStatus::NoSeparation {
                return Err(chain_error(
                    "requested exact-FE detector did not report no separation",
                ));
            }
        }

        if config.methods.simplex {
            let execution = take_primary_execution(
                report,
                &mut position,
                inspected_generation,
                SeparationMethod::Simplex,
            )?;
            if execution_commits(execution) {
                validate_primary_commit(execution, inspected_generation)?;
                finish_committed_attempt(
                    report,
                    &mut position,
                    inspected_generation,
                    config.methods,
                )?;
                continue;
            }
            simplex_unresolved = matches!(
                execution.status,
                MethodStatus::SkippedMemory
                    | MethodStatus::SkippedUnsupported
                    | MethodStatus::Uncertified
            );
            if !matches!(
                execution.status,
                MethodStatus::NoCandidates
                    | MethodStatus::NoSeparation
                    | MethodStatus::SkippedMemory
                    | MethodStatus::SkippedUnsupported
                    | MethodStatus::Uncertified
            ) {
                return Err(chain_error(
                    "requested simplex detector has a nonterminal no-drop status",
                ));
            }
        }

        if config.methods.iterated_rectifier {
            let execution = take_primary_execution(
                report,
                &mut position,
                inspected_generation,
                SeparationMethod::IteratedRectifier,
            )?;
            if execution_commits(execution) {
                validate_primary_commit(execution, inspected_generation)?;
                // This deletion restarts the model; it is not a
                // no-separation certificate for an unresolved simplex run.
                finish_committed_attempt(
                    report,
                    &mut position,
                    inspected_generation,
                    config.methods,
                )?;
                continue;
            }
            if !matches!(
                execution.status,
                MethodStatus::NoCandidates | MethodStatus::NoSeparation
            ) {
                return Err(chain_error(
                    "requested comprehensive IR detector did not certify its inspected generation",
                ));
            }
            if simplex_unresolved && execution.status != MethodStatus::NoSeparation {
                return Err(chain_error(
                    "unresolved simplex requires an IR no-separation certificate from the same stable generation",
                ));
            }
        } else if simplex_unresolved {
            return Err(chain_error(
                "unresolved simplex has no requested same-generation comprehensive IR closure",
            ));
        }

        if config.methods.mu {
            let execution = take_primary_execution(
                report,
                &mut position,
                inspected_generation,
                SeparationMethod::Mu,
            )?;
            if execution_commits(execution) {
                validate_primary_commit(execution, inspected_generation)?;
                finish_committed_attempt(
                    report,
                    &mut position,
                    inspected_generation,
                    config.methods,
                )?;
                continue;
            }
            // Mu is a one-sided heuristic: it may certify a concrete
            // deletion, but an empty candidate screen is not evidence that
            // separation is absent. Its sole no-drop grammar is therefore
            // the proof-free `NoCandidates` status.
            if execution.status != MethodStatus::NoCandidates {
                return Err(chain_error(
                    "requested Mu detector did not produce a terminal no-drop status",
                ));
            }
        }

        if inspected_generation != report.generations {
            return Err(chain_error(
                "separation chain stopped without committing the next generation",
            ));
        }
        if position != report.methods.len() {
            return Err(chain_error(
                "separation report contains executions after the stable terminal chain",
            ));
        }
        return Ok(());
    }
    Err(chain_error(
        "separation chain did not reach its declared final generation",
    ))
}

fn take_primary_execution<'a>(
    report: &'a SeparationReport,
    position: &mut usize,
    inspected_generation: usize,
    expected_method: SeparationMethod,
) -> Result<&'a SeparationMethodReport> {
    let execution = report.methods.get(*position).ok_or_else(|| {
        chain_error("requested separation detector is missing from an inspected generation")
    })?;
    if separation_inspected_generation(execution)? != inspected_generation
        || execution.method != expected_method
    {
        return Err(chain_error(
            "separation detector order or inspected generation does not match the exact request",
        ));
    }
    *position += 1;
    Ok(execution)
}

fn execution_commits(execution: &SeparationMethodReport) -> bool {
    !execution.dropped_observations.is_empty() || !execution.singleton_drops.is_empty()
}

fn separation_inspected_generation(execution: &SeparationMethodReport) -> Result<usize> {
    if execution_commits(execution) {
        if execution.status != MethodStatus::Certified || execution.generation == 0 {
            return Err(chain_error(
                "separation deletion is not attached to a valid committed generation",
            ));
        }
        Ok(execution.generation - 1)
    } else {
        Ok(execution.generation)
    }
}

fn validate_primary_commit(
    execution: &SeparationMethodReport,
    inspected_generation: usize,
) -> Result<()> {
    if execution.status != MethodStatus::Certified
        || execution.generation != inspected_generation.saturating_add(1)
    {
        return Err(chain_error(
            "separation deletion does not commit exactly the next generation",
        ));
    }
    Ok(())
}

fn finish_committed_attempt(
    report: &SeparationReport,
    position: &mut usize,
    inspected_generation: usize,
    requested: SeparationMethods,
) -> Result<()> {
    // A transaction may append one exact-FE fixed-point follow-up after its
    // primary detector.  It belongs to the same atomic commit and cannot be
    // mistaken for the primary FE execution of the restarted generation.
    if requested.fixed_effects {
        if let Some(followup) = report.methods.get(*position) {
            if separation_inspected_generation(followup)? == inspected_generation
                && followup.method == SeparationMethod::FixedEffects
            {
                if followup.status != MethodStatus::Certified
                    || followup.generation != inspected_generation.saturating_add(1)
                    || followup.dropped_observations.is_empty()
                    || !followup.singleton_drops.is_empty()
                {
                    return Err(chain_error(
                        "same-transaction exact-FE follow-up is malformed",
                    ));
                }
                *position += 1;
            }
        }
    }
    if report.methods.get(*position).is_some_and(|execution| {
        separation_inspected_generation(execution).ok() == Some(inspected_generation)
    }) {
        return Err(chain_error(
            "committed separation attempt contains an execution after its atomic restart",
        ));
    }
    Ok(())
}

fn chain_error(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

fn validate_separation_method(
    method: &SeparationMethodReport,
    candidate_original_rows: &[usize],
    full_model_columns: usize,
    fixed_effects: &[PpmlFixedEffectResult],
) -> Result<usize> {
    let candidate_observations = candidate_original_rows.len();
    let stable_unique = |rows: &[usize]| {
        rows.iter().enumerate().all(|(position, row)| {
            *row < candidate_observations && (position == 0 || rows[position - 1] < *row)
        })
    };
    if !method.elapsed_seconds.is_finite()
        || method.elapsed_seconds < 0.0
        || method.note.is_empty()
        || !stable_unique(&method.dropped_observations)
        || !stable_unique(&method.singleton_drops)
        || method
            .dropped_observations
            .iter()
            .any(|row| method.singleton_drops.binary_search(row).is_ok())
        || method.workspace_observation_vectors != separation_workspace_vectors(method.method)
        || (method.method == SeparationMethod::Simplex) != (method.simplex_tableau_cells > 0)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation method metadata is malformed",
        ));
    }
    if let Some(certificate) = method.certificate.as_ref() {
        use crate::ppml_separation::SeparationCertificateProof;
        let proof_matches_method = match (method.method, method.status, certificate.proof) {
            (
                SeparationMethod::Simplex,
                MethodStatus::NoSeparation,
                SeparationCertificateProof::SimplexFullPositiveRank { columns },
            ) => columns == full_model_columns && columns > 0,
            (
                SeparationMethod::Simplex,
                MethodStatus::NoSeparation,
                SeparationCertificateProof::SimplexDualInfeasibility {
                    boundary_rows,
                    nullity,
                },
            ) => boundary_rows > 0 && nullity > 0 && nullity <= full_model_columns,
            (
                SeparationMethod::IteratedRectifier,
                MethodStatus::NoSeparation,
                SeparationCertificateProof::IteratedRectifierFixedPoint,
            ) => true,
            (
                SeparationMethod::Simplex
                | SeparationMethod::IteratedRectifier
                | SeparationMethod::Mu,
                MethodStatus::Certified,
                SeparationCertificateProof::RecessionDirection,
            ) => true,
            _ => false,
        };
        if !proof_matches_method {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML separation certificate proof identity does not match its method and status",
            ));
        }
        validate_separation_certificate(certificate, method.status == MethodStatus::NoSeparation)?;
    }
    let exact_level_events = if let Some(certificate) = method.exact_fe_certificate.as_ref() {
        let checked_intercept_effects = fixed_effects
            .iter()
            .filter(|effect| effect.kind == PpmlFixedEffectChannelKind::Intercept)
            .count();
        let invalid_identity = certificate.zero_only_levels.iter().any(|identity| {
            let valid_effect = fixed_effects.iter().any(|effect| {
                effect.kind == PpmlFixedEffectChannelKind::Intercept
                    && effect.term_id == identity.effect.get()
            });
            let valid_anchor = candidate_original_rows
                .binary_search(&identity.representative_original_row)
                .ok()
                .is_some_and(|candidate_row| {
                    method
                        .dropped_observations
                        .binary_search(&candidate_row)
                        .is_ok()
                });
            !valid_effect || !valid_anchor
        });
        if method.method != SeparationMethod::FixedEffects
            || !certificate.passed
            || certificate.checked_intercept_effects != checked_intercept_effects
            || certificate
                .zero_only_levels
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || invalid_identity
            || (method.status == MethodStatus::NoSeparation
                && !certificate.zero_only_levels.is_empty())
            || (method.status == MethodStatus::Certified && certificate.zero_only_levels.is_empty())
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML exact-FE separation certificate is malformed or failed",
            ));
        }
        certificate.zero_only_levels.len()
    } else {
        0
    };
    let any_drops = !method.dropped_observations.is_empty() || !method.singleton_drops.is_empty();
    let expected_certificate_shape = match method.status {
        MethodStatus::NoCandidates => {
            !any_drops
                && method.iterations == 0
                && method.certificate.is_none()
                && method.exact_fe_certificate.is_none()
        }
        MethodStatus::NoSeparation => {
            !any_drops
                && match method.method {
                    SeparationMethod::Singletons => {
                        method.certificate.is_none() && method.exact_fe_certificate.is_none()
                    }
                    SeparationMethod::FixedEffects => {
                        method.certificate.is_none() && method.exact_fe_certificate.is_some()
                    }
                    SeparationMethod::Simplex
                    | SeparationMethod::IteratedRectifier
                    | SeparationMethod::Mu => {
                        method.certificate.is_some() && method.exact_fe_certificate.is_none()
                    }
                }
        }
        MethodStatus::Certified => {
            any_drops
                && match method.method {
                    SeparationMethod::Singletons => {
                        method.certificate.is_none() && method.exact_fe_certificate.is_none()
                    }
                    SeparationMethod::FixedEffects => {
                        method.certificate.is_none() && method.exact_fe_certificate.is_some()
                    }
                    SeparationMethod::Simplex
                    | SeparationMethod::IteratedRectifier
                    | SeparationMethod::Mu => {
                        method.certificate.is_some() && method.exact_fe_certificate.is_none()
                    }
                }
        }
        MethodStatus::SkippedMemory | MethodStatus::SkippedUnsupported => {
            method.method == SeparationMethod::Simplex
                && !any_drops
                && method.certificate.is_none()
                && method.exact_fe_certificate.is_none()
        }
        MethodStatus::Uncertified => {
            method.method == SeparationMethod::Simplex
                && !any_drops
                && method.certificate.is_none()
                && method.exact_fe_certificate.is_none()
        }
        MethodStatus::MaximumIterations => false,
    };
    if !expected_certificate_shape {
        let numerical_certificate_failure = matches!(
            method.method,
            SeparationMethod::Simplex | SeparationMethod::IteratedRectifier | SeparationMethod::Mu
        ) && matches!(
            method.status,
            MethodStatus::NoSeparation
                | MethodStatus::Certified
                | MethodStatus::SkippedMemory
                | MethodStatus::SkippedUnsupported
                | MethodStatus::MaximumIterations
                | MethodStatus::Uncertified
        );
        return Err(FeregError::new(
            if numerical_certificate_failure {
                ErrorCode::NonConvergence
            } else {
                ErrorCode::Internal
            },
            "PPML separation method status, drops, and certificate shape disagree",
        ));
    }
    if !separation_note_compatible(method.method, method.status, method.note) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation note code does not match its method and status",
        ));
    }
    Ok(exact_level_events)
}

/// Accept only the allocation-free note codes emitted by the certified
/// prepared detector for this exact method/status pair. Diagnostic prose is
/// deliberately not part of a digest, but an arbitrary static string must not
/// be able to masquerade as a registered execution state.
fn separation_note_compatible(
    method: SeparationMethod,
    status: MethodStatus,
    note: SeparationNote,
) -> bool {
    let code_matches = match (method, status, note.code) {
        (
            SeparationMethod::Singletons,
            MethodStatus::Certified,
            "prepared singleton pruning reached its fixed point",
        )
        | (
            SeparationMethod::Singletons,
            MethodStatus::NoSeparation,
            "prepared singleton pruning independently certified a stable fixed point",
        )
        | (
            SeparationMethod::FixedEffects,
            MethodStatus::Certified,
            "prepared exact zero-only FE certificate passed",
        )
        | (
            SeparationMethod::FixedEffects,
            MethodStatus::Certified,
            "prepared exact-FE follow-up in the same atomic generation",
        )
        | (
            SeparationMethod::FixedEffects,
            MethodStatus::NoSeparation,
            "prepared exact FE scan found no zero-only intercept level",
        )
        | (
            SeparationMethod::Simplex | SeparationMethod::IteratedRectifier | SeparationMethod::Mu,
            MethodStatus::NoCandidates,
            "no zero-outcome observations",
        )
        | (
            SeparationMethod::Simplex,
            MethodStatus::NoCandidates,
            "model simplex found no zero-outcome observations",
        )
        | (
            SeparationMethod::Simplex,
            MethodStatus::NoSeparation,
            "finite regressors have full column rank on the positive-outcome sample",
        )
        | (
            SeparationMethod::Simplex,
            MethodStatus::NoSeparation,
            "model simplex phase-I infeasibility passed an independent Farkas certificate",
        )
        | (
            SeparationMethod::Simplex,
            MethodStatus::Certified,
            "single null-space direction has a common sign on boundary observations",
        )
        | (
            SeparationMethod::Simplex,
            MethodStatus::Certified,
            "model simplex phase-I certificate passed",
        )
        | (
            SeparationMethod::Simplex,
            MethodStatus::Uncertified,
            "model simplex phase-I dual failed independent Farkas verification",
        )
        | (
            SeparationMethod::IteratedRectifier,
            MethodStatus::NoSeparation,
            "prepared iterated rectifier converged to zero",
        )
        | (
            SeparationMethod::IteratedRectifier,
            MethodStatus::NoSeparation,
            "prepared iterated rectifier certified no separation",
        )
        | (
            SeparationMethod::IteratedRectifier,
            MethodStatus::Certified,
            "prepared iterated-rectifier candidate direction",
        )
        | (
            SeparationMethod::Mu,
            MethodStatus::NoCandidates,
            "prepared mu found no small zero-outcome means",
        )
        | (SeparationMethod::Mu, MethodStatus::Certified, "prepared mu candidate direction") => {
            true
        }
        (
            SeparationMethod::Simplex,
            MethodStatus::SkippedMemory,
            "prepared full-model simplex exceeded its configured cell limit"
            | "model simplex exceeded its configured cell limit",
        ) => note.detail > 0,
        _ => false,
    };
    code_matches
        && (note.detail == 0
            || matches!(
                (method, status),
                (SeparationMethod::Simplex, MethodStatus::SkippedMemory)
            ))
}

fn validate_separation_certificate(
    certificate: &SeparationCertificate,
    allow_zero_direction: bool,
) -> Result<()> {
    use crate::ppml_separation::SeparationCertificateProof;
    let nonnegative = [
        certificate.interior_zero_relative,
        certificate.boundary_violation_relative,
        certificate.span_residual_relative,
        certificate.negative_margin_relative,
        certificate.positive_maximum,
    ];
    let valid_zero_direction = allow_zero_direction
        && match certificate.proof {
            SeparationCertificateProof::RecessionDirection => {
                certificate.direction_scale == 0.0 && nonnegative.iter().all(|value| *value == 0.0)
            }
            SeparationCertificateProof::SimplexFullPositiveRank { columns } => {
                columns > 0
                    && certificate.direction_scale == 0.0
                    && nonnegative.iter().all(|value| *value == 0.0)
            }
            SeparationCertificateProof::SimplexDualInfeasibility {
                boundary_rows,
                nullity,
            } => {
                boundary_rows > 0
                    && nullity > 0
                    && certificate.direction_scale > 0.0
                    && certificate.negative_margin_relative > 0.0
            }
            SeparationCertificateProof::IteratedRectifierFixedPoint => true,
        };
    let valid_nonzero_direction = certificate.direction_scale > 0.0
        && matches!(
            certificate.proof,
            SeparationCertificateProof::RecessionDirection
                | SeparationCertificateProof::SimplexDualInfeasibility { .. }
        );
    if !certificate.passed
        || !certificate.direction_scale.is_finite()
        || certificate.direction_scale < 0.0
        || !(valid_zero_direction || valid_nonzero_direction)
        || nonnegative
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML numerical separation certificate is not finite and passed",
        ));
    }
    Ok(())
}

const fn separation_workspace_vectors(method: SeparationMethod) -> usize {
    match method {
        SeparationMethod::Singletons => 0,
        SeparationMethod::FixedEffects => 1,
        SeparationMethod::Simplex => 0,
        SeparationMethod::IteratedRectifier | SeparationMethod::Mu => 8,
    }
}

const fn method_primary_reason(method: SeparationMethod) -> SeparationDropReason {
    match method {
        SeparationMethod::Singletons => SeparationDropReason::InitialSingleton,
        SeparationMethod::FixedEffects => SeparationDropReason::FixedEffectZeroLevel,
        SeparationMethod::Simplex => SeparationDropReason::Simplex,
        SeparationMethod::IteratedRectifier => SeparationDropReason::IteratedRectifier,
        SeparationMethod::Mu => SeparationDropReason::Mu,
    }
}

fn replay_separation_generation_hashes(
    report: &SeparationReport,
    candidate_original_rows: &[usize],
) -> Result<()> {
    let mut active = fallible_filled(
        report.candidate_observations,
        true,
        "unable to reserve PPML separation hash replay active mask",
    )?;
    let mut reasons = fallible_filled(
        report.candidate_observations,
        None,
        "unable to reserve PPML separation hash replay reasons",
    )?;
    if report.generation_hashes.first().copied()
        != Some(candidate_ledger_hash_candidate_axis(
            candidate_original_rows,
            &active,
            &reasons,
        )?)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML initial separation generation hash is invalid",
        ));
    }
    for generation in 1..=report.generations {
        let mut changed = false;
        for method in report
            .methods
            .iter()
            .filter(|method| method.generation == generation)
        {
            for &row in &method.dropped_observations {
                if !active[row] {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML separation hash replay drops an inactive row",
                    ));
                }
                active[row] = false;
                reasons[row] = Some(method_primary_reason(method.method));
                changed = true;
            }
            let singleton_reason = if method.method == SeparationMethod::Singletons {
                SeparationDropReason::InitialSingleton
            } else {
                SeparationDropReason::SingletonCascade
            };
            for &row in &method.singleton_drops {
                if !active[row] {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML separation hash replay singleton-drops an inactive row",
                    ));
                }
                active[row] = false;
                reasons[row] = Some(singleton_reason);
                changed = true;
            }
        }
        if !changed
            || report.generation_hashes.get(generation).copied()
                != Some(candidate_ledger_hash_candidate_axis(
                    candidate_original_rows,
                    &active,
                    &reasons,
                )?)
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML committed separation generation hash cannot be replayed",
            ));
        }
    }
    if active != report.active || reasons != report.drop_reasons {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML separation generation replay does not reach the published ledger",
        ));
    }
    Ok(())
}

fn relative_scalar_error(left: f64, right: f64) -> f64 {
    if !left.is_finite() || !right.is_finite() {
        f64::INFINITY
    } else {
        (left - right).abs() / left.abs().max(right.abs()).max(1.0)
    }
}

fn fallible_filled<T: Clone>(length: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    output.resize(length, value);
    Ok(output)
}

fn fallible_copy<T: Clone>(source: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    output.extend_from_slice(source);
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
pub fn certify_ppml_reconstruction(
    original_observations: usize,
    original_rows: &[usize],
    offset: &[f64],
    finite_predictor: &[f64],
    absorbed_predictor: &[f64],
    eta: &[f64],
    mean: &[f64],
    saved_effects: Option<&PpmlSavedEffectOutput>,
    tolerance: f64,
) -> Result<PpmlReconstructionCertificate> {
    let rows = original_rows.len();
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML reconstruction tolerance must be finite and lie in (0, 1)",
        ));
    }
    if original_observations < rows
        || offset.len() != rows
        || finite_predictor.len() != rows
        || absorbed_predictor.len() != rows
        || eta.len() != rows
        || mean.len() != rows
        || original_rows.iter().enumerate().any(|(position, row)| {
            *row >= original_observations || (position > 0 && original_rows[position - 1] >= *row)
        })
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML output products do not share a valid stable row mapping",
        ));
    }
    if offset
        .iter()
        .chain(finite_predictor)
        .chain(absorbed_predictor)
        .chain(eta)
        .chain(mean)
        .any(|value| !value.is_finite())
        || mean.iter().any(|value| *value <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML final output products must be finite and means strictly positive",
        ));
    }
    if let Some(saved) = saved_effects {
        let indices_are_strict = saved
            .declared_channel_indices
            .windows(2)
            .all(|pair| pair[0] < pair[1]);
        if saved.declared_channels == 0
            || saved.channels.is_empty()
            || saved.channels.len() != saved.declared_channel_indices.len()
            || saved.channels.len() > saved.declared_channels
            || !indices_are_strict
            || saved
                .declared_channel_indices
                .iter()
                .any(|index| *index >= saved.declared_channels)
            || saved.channels.iter().any(|channel| channel.len() != rows)
            || saved
                .channels
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
            || !saved
                .total_reconstruction_relative_infinity_norm
                .is_finite()
            || saved.total_reconstruction_relative_infinity_norm
                > saved.full_decomposition_tolerance
            || !saved.full_decomposition_relative_infinity_norm.is_finite()
            || !saved.full_decomposition_tolerance.is_finite()
            || saved.full_decomposition_tolerance <= 0.0
            || saved.full_decomposition_tolerance >= 1.0
            || saved.full_decomposition_relative_infinity_norm > saved.full_decomposition_tolerance
            || !saved.full_decomposition_certified
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML saved fixed-effect products are not aligned or lack a complete-decomposition certificate",
            ));
        }
    }

    let mut predictor_absolute = 0.0_f64;
    let mut predictor_relative = 0.0_f64;
    let mut mean_relative = 0.0_f64;
    // Recovery compares its transient reconstructed total directly with the
    // authoritative absorbed predictor before releasing that O(N) vector.
    // Retain the scalar error as evidence instead of retaining the vector.
    let saved_total_relative = saved_effects.map_or(0.0, |saved| {
        saved.total_reconstruction_relative_infinity_norm
    });
    let mut saved_channels_relative =
        saved_effects.map_or(0.0, |saved| saved.full_decomposition_relative_infinity_norm);
    let saved_complete_decomposition =
        saved_effects.is_some_and(|saved| saved.channels.len() == saved.declared_channels);
    for row in 0..rows {
        let reconstructed = offset[row] + finite_predictor[row] + absorbed_predictor[row];
        if !reconstructed.is_finite() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!("PPML predictor reconstruction overflowed at compact row {row}"),
            ));
        }
        let predictor_error = (eta[row] - reconstructed).abs();
        let predictor_scale = eta[row]
            .abs()
            .max(offset[row].abs())
            .max(finite_predictor[row].abs())
            .max(absorbed_predictor[row].abs())
            .max(1.0);
        predictor_absolute = predictor_absolute.max(predictor_error);
        predictor_relative = predictor_relative.max(predictor_error / predictor_scale);

        let reconstructed_mean = eta[row].exp();
        if !reconstructed_mean.is_finite() || reconstructed_mean <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!("PPML final eta has no representable positive mean at compact row {row}"),
            ));
        }
        mean_relative = mean_relative.max(
            (mean[row] - reconstructed_mean).abs()
                / mean[row]
                    .abs()
                    .max(reconstructed_mean.abs())
                    .max(f64::MIN_POSITIVE),
        );

        if let Some(saved) = saved_effects {
            // A partial public mask deliberately omits unrequested O(N)
            // channels. Its complete-decomposition evidence comes from the
            // final-Hessian recovery pass above. Recompute the public sum only
            // when every declared channel was retained.
            if saved_complete_decomposition {
                let sum = saved
                    .channels
                    .iter()
                    .fold(0.0_f64, |total, channel| total + channel[row]);
                if !sum.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        format!(
                            "PPML saved fixed-effect channel sum overflowed at compact row {row}"
                        ),
                    ));
                }
                saved_channels_relative = saved_channels_relative.max(
                    (sum - absorbed_predictor[row]).abs()
                        / sum.abs().max(absorbed_predictor[row].abs()).max(1.0),
                );
            }
        }
    }
    let saved_channels = saved_effects.map_or(0, |saved| saved.channels.len());
    let saved_declared_channels = saved_effects.map_or(0, |saved| saved.declared_channels);
    let saved_recovery_certified =
        saved_effects.is_none_or(|saved| saved.full_decomposition_certified);
    let passed = predictor_relative <= tolerance
        && mean_relative <= tolerance
        && saved_total_relative <= tolerance
        && saved_channels_relative <= tolerance
        && saved_recovery_certified;
    Ok(PpmlReconstructionCertificate {
        predictor_absolute_infinity_norm: predictor_absolute,
        predictor_relative_infinity_norm: predictor_relative,
        mean_relative_infinity_norm: mean_relative,
        saved_total_relative_infinity_norm: saved_total_relative,
        saved_channels_relative_infinity_norm: saved_channels_relative,
        saved_channels,
        saved_declared_channels,
        saved_complete_decomposition,
        tolerance,
        passed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DenseMatrix;
    use crate::options::MemoryMode;
    use crate::ppml::{
        compute_ppml_inference, plan_ppml_memory, PpmlInferenceInput, PpmlMemoryConfig,
        PpmlMemoryDimensions, PpmlSolverEngine, PpmlSolverFamily, PpmlSolverPlan, PpmlVceRequest,
    };

    fn exact_state(saved: bool) -> PpmlOutputState {
        let offset = vec![0.1_f64, -0.2, 0.3];
        let finite = vec![0.2_f64, 0.4, -0.1];
        let first = vec![0.3_f64, -0.1, 0.2];
        let second = vec![-0.05_f64, 0.2, 0.1];
        let absorbed = first
            .iter()
            .zip(&second)
            .map(|(left, right)| left + right)
            .collect::<Vec<_>>();
        let eta = offset
            .iter()
            .zip(&finite)
            .zip(&absorbed)
            .map(|((offset, finite), absorbed)| offset + finite + absorbed)
            .collect::<Vec<_>>();
        let mean = eta.iter().map(|value| value.exp()).collect::<Vec<_>>();
        let effects = saved.then_some(PpmlSavedEffectOutput {
            channels: vec![first, second],
            declared_channels: 2,
            declared_channel_indices: vec![0, 1],
            total_reconstruction_relative_infinity_norm: 0.0,
            full_decomposition_relative_infinity_norm: 0.0,
            full_decomposition_tolerance: 1e-12,
            full_decomposition_certified: true,
        });
        PpmlOutputState::new(
            5,
            vec![0, 2, 4],
            offset,
            finite,
            absorbed,
            eta,
            mean,
            effects,
            1e-12,
        )
        .expect("certified output state")
    }

    fn certified_composite_input() -> CertifiedPpmlResultInput {
        let outcome = vec![1.0_f64, 3.0, 2.0, 4.0];
        let prior_weights = vec![1.0_f64; outcome.len()];
        let mean_value = 2.5_f64;
        let eta_value = mean_value.ln();
        let mean = vec![eta_value.exp(); outcome.len()];
        let eta = vec![eta_value; outcome.len()];
        let coefficients = PpmlCoefficientOutput::new(
            vec![eta_value],
            vec![PpmlCoefficientStatus::Identified],
            vec![Some(0)],
            vec![0],
        )
        .expect("coefficient result");
        let sample = PpmlFinalSample::new(
            outcome.len(),
            outcome.len(),
            Arc::new((0..outcome.len()).collect::<Vec<_>>()),
            (0..outcome.len()).collect(),
            vec![DropReason::Retained; outcome.len()],
            outcome.clone(),
            prior_weights.clone(),
        )
        .expect("final sample");
        let weight_report = PreparedPpmlWeightReport {
            kind: None,
            physical_observations: outcome.len(),
            effective_observations: outcome.len(),
            raw_weight_sum: None,
            log_raw_weight_sum: None,
            prepared_weight_sum: outcome.len() as f64,
            raw_to_prepared_multiplier: Some(1.0),
            log_raw_to_prepared_multiplier: 0.0,
        };
        let design =
            DenseMatrix::from_columns(&[vec![1.0_f64; outcome.len()]]).expect("constant design");
        let score_residuals = outcome
            .iter()
            .zip(&mean)
            .map(|(outcome, mean)| outcome - mean)
            .collect::<Vec<_>>();
        let inference = compute_ppml_inference(PpmlInferenceInput {
            coefficients: coefficients.values(),
            full_dimension: coefficients.values().len(),
            within_design: &design,
            reported_finite_rank: 1,
            score_residuals: &score_residuals,
            prior_weights: &prior_weights,
            weight_report,
            identified_bread: &[0.1],
            kept_full_slots: coefficients.kept_full_slots(),
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Robust,
            wald_columns: &[],
            certificate_tolerance: 1e-10,
        })
        .expect("certified inference");
        let output = PpmlOutputState::new(
            outcome.len(),
            (0..outcome.len()).collect(),
            vec![0.0; outcome.len()],
            eta.clone(),
            vec![0.0; outcome.len()],
            eta,
            mean.clone(),
            None,
            1e-12,
        )
        .expect("output state");
        let objective = poisson_objective(&outcome, &mean, &prior_weights).expect("objective");
        let separation = SeparationReport {
            candidate_observations: outcome.len(),
            active: vec![true; outcome.len()],
            retained_observations: (0..outcome.len()).collect(),
            dropped_observations: Vec::new(),
            drop_reasons: vec![None; outcome.len()],
            singleton_count: 0,
            separated_count: 0,
            generations: 0,
            generation_hashes: vec![candidate_ledger_hash_candidate_axis(
                &(0..outcome.len()).collect::<Vec<_>>(),
                &vec![true; outcome.len()],
                &vec![None; outcome.len()],
            )
            .expect("identity candidate ledger hash")],
            methods: Vec::new(),
        };
        let separation_config = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let separation_seal =
            super::super::prepared_separation::certify_disabled_separation_for_test(
                separation_config,
                &separation,
                sample.original_rows(),
                sample.outcome(),
            )
            .expect("disabled separation seal");
        let dimensions = PpmlMemoryDimensions {
            metadata: Default::default(),
            observations: outcome.len(),
            original_observations: outcome.len(),
            candidate_observations: outcome.len(),
            loaded_numeric_columns: 1,
            retained_base_adjustment_capacity_elements: 0,
            retained_raw_preseparation_probability_weight_elements: 0,
            finite_columns: 1,
            coefficient_slots: 1,
            reported_coefficient_slots: 1,
            fe_dimensions: 0,
            fe_total_levels: 0,
            fe_max_levels: 0,
            fe_total_channels: 0,
            fe_max_channels: 0,
            fe_max_level_members: 0,
            fe_coefficient_directions: 0,
            fe_max_effect_directions: 0,
            cluster_dimensions: 0,
            cluster_subset_groups: 0,
            separation_workspace_vectors: 0,
            separation_singleton_enabled: false,
            separation_method_count: 0,
            separation_generation_capacity: 1,
            separation_rebase_enabled: false,
            separation_exact_fe_enabled: false,
            separation_mu_enabled: false,
            simplex_tableau_cells: 0,
            requested_output_columns: 0,
            requested_mean_output: false,
            requested_saved_fe_channels: 0,
            publication_matrix_cells: 64,
            threads: 1,
            allocation_audit_enabled: false,
        };
        let memory_plan = plan_ppml_memory(
            dimensions,
            PpmlMemoryConfig {
                requested: MemoryMode::Fast,
                ..PpmlMemoryConfig::default()
            },
        )
        .expect("memory plan");
        let workspace = PpmlIterationWorkspace::from_initial_state(
            memory_plan,
            vec![0.0; outcome.len()],
            vec![0.0; memory_plan.dimensions.coefficient_slots],
        )
        .expect("memory fixture workspace");
        let mut memory = PpmlMemoryTelemetry::new(memory_plan, 0, 1).expect("memory telemetry");
        memory.observe_linear_route(FeSolverKind::Map);
        memory.observe_scratch(0);
        memory
            .observe_diagnostic_records(0)
            .expect("memory diagnostic telemetry");
        memory
            .certify_workspace(&workspace)
            .expect("memory workspace telemetry");
        CertifiedPpmlResultInput {
            coefficients,
            sample,
            weights: weight_report,
            adjustment: PreparedPpmlAdjustmentReport {
                input_kind: super::super::PreparedPpmlAdjustmentKind::None,
                prepared_as_offset: false,
                exposure_log_transformed: false,
            },
            objective,
            inference,
            output,
            convergence: PpmlConvergenceReport {
                converged: true,
                two_consecutive_convergence: true,
                rank_frozen: true,
                outer_iterations: 2,
                inner_iterations: 2,
                step_halvings: 0,
                strict_profile_rejections: 0,
                strict_final_solve_passed: true,
                linear_within_relative_infinity_norm: 0.0,
                linear_reconstruction_relative_infinity_norm: 0.0,
                final_score: PpmlCertificate {
                    finite_absolute_infinity_norm: 0.0,
                    fixed_effect_absolute_infinity_norm: 0.0,
                    relative_infinity_norm: 0.0,
                    relative_two_norm: 0.0,
                    passed: true,
                },
            },
            fixed_effects: Vec::new(),
            clusters: Vec::new(),
            separation,
            separation_config,
            separation_seal,
            memory,
            solver: PpmlSolverReport::new(
                PpmlSolverPlan::new(PpmlSolverFamily::Auto),
                PpmlSolverEngine::Map,
            ),
            certificate_tolerance: 1e-10,
        }
    }

    fn certified_clustered_composite_input() -> CertifiedPpmlResultInput {
        let mut input = certified_composite_input();
        let design = DenseMatrix::from_columns(&[vec![1.0_f64; input.sample.outcome().len()]])
            .expect("constant design");
        let score_residuals = input
            .sample
            .outcome()
            .iter()
            .zip(input.output.mean())
            .map(|(outcome, mean)| outcome - mean)
            .collect::<Vec<_>>();
        let cluster = [0_u32, 0, 1, 1];
        input.inference = compute_ppml_inference(PpmlInferenceInput {
            coefficients: input.coefficients.values(),
            full_dimension: input.coefficients.values().len(),
            within_design: &design,
            reported_finite_rank: 1,
            score_residuals: &score_residuals,
            prior_weights: input.sample.prior_weights(),
            weight_report: input.weights,
            identified_bread: &[0.1],
            kept_full_slots: input.coefficients.kept_full_slots(),
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[&cluster],
            requested_vce: PpmlVceRequest::Clustered,
            wald_columns: &[],
            certificate_tolerance: input.certificate_tolerance,
        })
        .expect("clustered inference");
        input.clusters = vec![PpmlClusterDimensionResult {
            declared_dimension: 0,
            label: "g".to_owned(),
            source_ids: vec![1],
            groups: 2,
        }];
        let mut dimensions = input.memory.plan.dimensions;
        dimensions.cluster_dimensions = 1;
        dimensions.cluster_subset_groups = 2;
        let memory_plan = plan_ppml_memory(
            dimensions,
            PpmlMemoryConfig {
                requested: MemoryMode::Fast,
                ..PpmlMemoryConfig::default()
            },
        )
        .expect("clustered memory plan");
        let workspace = PpmlIterationWorkspace::from_initial_state(
            memory_plan,
            vec![0.0; dimensions.observations],
            vec![0.0; memory_plan.dimensions.coefficient_slots],
        )
        .expect("clustered memory fixture workspace");
        input.memory =
            PpmlMemoryTelemetry::new(memory_plan, 0, 1).expect("clustered memory telemetry");
        input.memory.observe_linear_route(FeSolverKind::Map);
        input.memory.observe_scratch(0);
        input
            .memory
            .observe_diagnostic_records(0)
            .expect("clustered memory diagnostic telemetry");
        input
            .memory
            .certify_workspace(&workspace)
            .expect("clustered memory workspace telemetry");
        input
    }

    #[test]
    fn exact_predictor_and_saved_channels_certify() {
        let state = exact_state(true);
        assert!(state.certificate().passed);
        assert_eq!(state.certificate().saved_channels, 2);
        assert_eq!(
            state
                .saved_effects()
                .expect("saved")
                .channels
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            vec![3, 3]
        );
    }

    #[test]
    fn xb_includes_offset_but_not_absorbed_contribution() {
        let state = exact_state(false);
        let xb = state.prediction(PpmlPredictionKind::Xb, None).expect("xb");
        for (actual, expected) in xb.iter().zip([0.3_f64, 0.2, 0.2]) {
            assert!((actual - expected).abs() <= 2e-16);
        }
        assert_eq!(
            state
                .prediction(PpmlPredictionKind::Absorbed, None)
                .expect("absorbed"),
            state.absorbed_predictor()
        );
        assert_eq!(
            state
                .prediction(PpmlPredictionKind::Eta, None)
                .expect("eta"),
            state.eta()
        );
    }

    #[test]
    fn residual_predictions_require_outcome_and_remain_unweighted() {
        let state = exact_state(false);
        assert_eq!(
            state
                .prediction(PpmlPredictionKind::Response, None)
                .expect_err("missing outcome")
                .code,
            ErrorCode::InvalidArgument
        );
        let outcome = [1.0_f64, 2.0, 3.0];
        let response = state
            .prediction(PpmlPredictionKind::Response, Some(&outcome))
            .expect("response");
        assert_eq!(
            response,
            outcome
                .iter()
                .zip(state.mean())
                .map(|(outcome, mean)| outcome - mean)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn scatter_uses_stable_original_rows_and_missing_elsewhere() {
        let state = exact_state(false);
        let scattered = state
            .scatter_to_original(&[10.0, 20.0, 30.0], f64::NAN)
            .expect("scatter");
        assert_eq!(scattered[0], 10.0);
        assert!(scattered[1].is_nan());
        assert_eq!(scattered[2], 20.0);
        assert!(scattered[3].is_nan());
        assert_eq!(scattered[4], 30.0);
    }

    #[test]
    fn nonmonotone_mapping_and_inconsistent_saved_effects_fail_closed() {
        let state = exact_state(true);
        let error = PpmlOutputState::new(
            5,
            vec![0, 0, 4],
            state.offset.clone(),
            state.finite_predictor.clone(),
            state.absorbed_predictor.clone(),
            state.eta.clone(),
            state.mean.clone(),
            state.saved_effects.clone(),
            1e-12,
        )
        .expect_err("mapping must be unique and ordered");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.return_code(), 3499);

        let mut saved = state.saved_effects.clone().expect("saved");
        saved.channels[0][1] += 0.25;
        let error = PpmlOutputState::new(
            5,
            vec![0, 2, 4],
            state.offset,
            state.finite_predictor,
            state.absorbed_predictor,
            state.eta,
            state.mean,
            Some(saved),
            1e-12,
        )
        .expect_err("saved channels must reconstruct");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(error.return_code(), 430);
    }

    #[test]
    fn partial_saved_mask_uses_full_decomposition_evidence_without_public_sum() {
        let state = exact_state(true);
        let complete = state.saved_effects.clone().expect("complete saved effects");
        let partial = PpmlSavedEffectOutput {
            channels: vec![complete.channels[0].clone()],
            declared_channels: 2,
            declared_channel_indices: vec![0],
            total_reconstruction_relative_infinity_norm: 0.0,
            full_decomposition_relative_infinity_norm: 0.0,
            full_decomposition_tolerance: 1e-12,
            full_decomposition_certified: true,
        };
        let partial_state = PpmlOutputState::new(
            state.original_observations,
            state.original_rows,
            state.offset,
            state.finite_predictor,
            state.absorbed_predictor,
            state.eta,
            state.mean,
            Some(partial),
            1e-12,
        )
        .expect("partial saved mask uses complete recovery evidence");
        assert_eq!(partial_state.certificate().saved_channels, 1);
        assert_eq!(partial_state.certificate().saved_declared_channels, 2);
        assert!(!partial_state.certificate().saved_complete_decomposition);
    }

    #[test]
    fn duplicate_or_out_of_range_saved_declared_indices_fail_closed() {
        let state = exact_state(true);
        let mut duplicate = state.saved_effects.clone().expect("saved effects");
        duplicate.declared_channel_indices = vec![0, 0];
        assert_eq!(
            PpmlOutputState::new(
                state.original_observations,
                state.original_rows.clone(),
                state.offset.clone(),
                state.finite_predictor.clone(),
                state.absorbed_predictor.clone(),
                state.eta.clone(),
                state.mean.clone(),
                Some(duplicate),
                1e-12,
            )
            .expect_err("duplicate saved declared identity")
            .code,
            ErrorCode::Internal
        );

        let mut out_of_range = state.saved_effects.clone().expect("saved effects");
        out_of_range.declared_channel_indices = vec![0, 2];
        assert_eq!(
            PpmlOutputState::new(
                state.original_observations,
                state.original_rows,
                state.offset,
                state.finite_predictor,
                state.absorbed_predictor,
                state.eta,
                state.mean,
                Some(out_of_range),
                1e-12,
            )
            .expect_err("out-of-range saved declared identity")
            .code,
            ErrorCode::Internal
        );
    }

    #[test]
    fn nonconstant_mu_requires_final_hessian_not_preparation_weights() {
        let prior = [1.0_f64, 1.0, 2.0];
        let mean = [1.0_f64, 3.0, 0.5];
        let q_only = [1.0_f64, 1.0, 2.0];
        assert_eq!(
            validate_ppml_hessian_weights(BorrowedMapWeights::Provided(&q_only), &prior, &mean)
                .expect_err("q alone is not the PPML recovery metric")
                .code,
            ErrorCode::Internal
        );
        let final_hessian = [1.0_f64, 3.0, 1.0];
        validate_ppml_hessian_weights(BorrowedMapWeights::Provided(&final_hessian), &prior, &mean)
            .expect("q*mu is the certified recovery metric");
    }

    #[test]
    fn null_likelihood_is_constant_only_and_pseudo_r_squared_uses_it() {
        let outcome = [1.0_f64, 3.0, 2.0, 6.0];
        let weights = [1.0_f64; 4];
        let fitted_mean = [2.0_f64, 2.0, 4.0, 4.0];
        let objective = super::super::poisson_objective(&outcome, &fitted_mean, &weights)
            .expect("fitted objective");
        let statistics = PpmlLikelihoodStatistics::from_final_state(objective, &outcome, &weights)
            .expect("statistics");
        assert!((statistics.null_mean - 3.0).abs() <= 8.0 * f64::EPSILON);
        let null = super::super::poisson_objective(&outcome, &[3.0; 4], &weights)
            .expect("explicit null objective");
        assert!(
            (statistics.null_log_likelihood - null.log_likelihood).abs()
                <= 8.0 * f64::EPSILON * null.log_likelihood.abs().max(1.0)
        );
        let expected = 1.0 - objective.log_likelihood / null.log_likelihood;
        assert!((statistics.pseudo_r_squared - expected).abs() <= 8.0 * f64::EPSILON);
    }

    #[test]
    fn null_likelihood_does_not_accept_an_offset_or_fixed_effect_input() {
        // The API has only outcome and prior weights by construction. This
        // fixture also verifies literal fweight replication.
        let compact_y = [1.0_f64, 4.0];
        let compact_weights = [2.0_f64, 3.0];
        let compact = super::super::poisson_null_objective(&compact_y, &compact_weights)
            .expect("compact null");
        let expanded_y = [1.0_f64, 1.0, 4.0, 4.0, 4.0];
        let expanded =
            super::super::poisson_null_objective(&expanded_y, &[1.0; 5]).expect("expanded null");
        assert!((compact.mean - expanded.mean).abs() < 1e-15);
        assert!((compact.log_likelihood - expanded.log_likelihood).abs() < 1e-13);
    }

    #[test]
    fn composite_result_is_one_certified_prediction_and_inference_authority() {
        let result =
            CertifiedPpmlResult::new(certified_composite_input()).expect("composite result");
        assert!(result.certificate().passed);
        assert_eq!(result.inference().dof.structural_residual, 3);
        assert_eq!(result.inference().dof.reported, 3);
        assert_eq!(
            result.inference().vce.coefficient_distribution,
            super::super::PpmlTestDistribution::StandardNormal
        );
        assert_eq!(
            result.inference().wald.distribution,
            super::super::PpmlTestDistribution::ChiSquared
        );
        assert!(result.inference().vce.z_statistics[0].is_finite());
        assert_eq!(
            result.prediction(PpmlPredictionKind::Mu).expect("mean"),
            result.output().mean()
        );
        assert_eq!(
            result
                .prediction(PpmlPredictionKind::Response)
                .expect("response residual"),
            result
                .sample()
                .outcome()
                .iter()
                .zip(result.output().mean())
                .map(|(outcome, mean)| outcome - mean)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn composite_result_rejects_cross_generation_rows_and_objective_drift() {
        let mut generation = certified_composite_input();
        generation.separation.retained_observations.swap(1, 2);
        assert_eq!(
            CertifiedPpmlResult::new(generation)
                .expect_err("cross-generation row order")
                .code,
            ErrorCode::Internal
        );

        let mut objective = certified_composite_input();
        objective.objective.deviance += 0.25;
        assert_eq!(
            CertifiedPpmlResult::new(objective)
                .expect_err("objective drift")
                .code,
            ErrorCode::NonConvergence
        );
    }

    #[test]
    fn frozen_memory_route_axes_remain_bound_to_the_certified_final_sample() {
        let mut original = certified_composite_input();
        original.sample.original_observations += 1;
        let error = CertifiedPpmlResult::new(original)
            .expect_err("memory route must retain the certified original axis");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("frozen memory-route axes"));

        let mut candidate = certified_composite_input();
        candidate.sample.candidate_observations -= 1;
        let error = CertifiedPpmlResult::new(candidate)
            .expect_err("memory route must retain the certified if/in candidate axis");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("frozen memory-route axes"));

        let mut generation_zero = certified_composite_input();
        Arc::make_mut(&mut generation_zero.sample.prepared_original_rows).pop();
        let error = CertifiedPpmlResult::new(generation_zero)
            .expect_err("memory route must retain the certified generation-zero active axis");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("frozen memory-route axes"));

        let mut uncharged_probability_weights = certified_composite_input();
        let prepared_rows = uncharged_probability_weights
            .sample
            .prepared_original_rows()
            .len();
        uncharged_probability_weights.weights.kind = Some(WeightKind::Probability);
        uncharged_probability_weights.weights.raw_weight_sum = Some(prepared_rows as f64);
        uncharged_probability_weights.weights.log_raw_weight_sum =
            Some((prepared_rows as f64).ln());
        let error = CertifiedPpmlResult::new(uncharged_probability_weights)
            .expect_err("pweights require one retained raw value per generation-zero active row");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("retained pre-separation"));

        let mut spurious_unweighted_capacity = certified_composite_input();
        spurious_unweighted_capacity
            .memory
            .plan
            .preflight_dimensions
            .retained_raw_preseparation_probability_weight_elements = prepared_rows;
        let error = validate_memory_sample_axes(
            &spurious_unweighted_capacity.sample,
            spurious_unweighted_capacity.weights,
            spurious_unweighted_capacity.adjustment,
            spurious_unweighted_capacity.separation_config,
            &spurious_unweighted_capacity.memory,
        )
        .expect_err("unweighted fits cannot claim retained raw-pweight capacity");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("retained pre-separation"));

        let mut uncharged_offset = certified_composite_input();
        uncharged_offset.adjustment = PreparedPpmlAdjustmentReport {
            input_kind: PreparedPpmlAdjustmentKind::Offset,
            prepared_as_offset: true,
            exposure_log_transformed: false,
        };
        let error = CertifiedPpmlResult::new(uncharged_offset)
            .expect_err("offset fits require immutable candidate-capacity owner storage");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("retained pre-separation"));

        let mut spurious_adjustment_capacity = certified_composite_input();
        let candidate_rows = spurious_adjustment_capacity.sample.candidate_observations();
        spurious_adjustment_capacity
            .memory
            .plan
            .dimensions
            .retained_base_adjustment_capacity_elements = candidate_rows;
        spurious_adjustment_capacity
            .memory
            .plan
            .preflight_dimensions
            .retained_base_adjustment_capacity_elements = candidate_rows;
        let error = validate_memory_sample_axes(
            &spurious_adjustment_capacity.sample,
            spurious_adjustment_capacity.weights,
            spurious_adjustment_capacity.adjustment,
            spurious_adjustment_capacity.separation_config,
            &spurious_adjustment_capacity.memory,
        )
        .expect_err("unadjusted fits cannot claim retained base-adjustment capacity");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("retained pre-separation"));

        let mut active = certified_composite_input();
        active.sample.original_rows.pop();
        active.sample.outcome.pop();
        active.sample.prior_weights.pop();
        active.weights.physical_observations -= 1;
        active.weights.effective_observations -= 1;
        active.weights.prepared_weight_sum -= 1.0;
        let error = CertifiedPpmlResult::new(active)
            .expect_err("memory route must retain the certified active axis");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error.message.contains("frozen memory-route axes"));
    }

    #[test]
    fn composite_result_rejects_mutated_dof_vce_wald_certificate_and_memory_families() {
        let mut dof = certified_composite_input();
        dof.inference.dof.reported -= 1;
        assert_eq!(
            CertifiedPpmlResult::new(dof)
                .expect_err("DoF identity")
                .code,
            ErrorCode::Internal
        );

        let mut vce = certified_composite_input();
        vce.inference.vce.effective = PpmlVceKind::Clustered;
        assert_eq!(
            CertifiedPpmlResult::new(vce)
                .expect_err("VCE identity")
                .code,
            ErrorCode::Internal
        );

        let mut wald = certified_composite_input();
        wald.inference.wald.distribution = PpmlTestDistribution::StandardNormal;
        assert_eq!(
            CertifiedPpmlResult::new(wald)
                .expect_err("Wald distribution identity")
                .code,
            ErrorCode::Internal
        );

        let mut covariance = certified_composite_input();
        covariance.inference.vce.standard_errors[0] *= 2.0;
        assert_eq!(
            CertifiedPpmlResult::new(covariance)
                .expect_err("covariance/SE certificate")
                .code,
            ErrorCode::NonConvergence
        );

        let mut certificate = certified_composite_input();
        certificate.inference.vce.certificate.passed = false;
        assert_eq!(
            CertifiedPpmlResult::new(certificate)
                .expect_err("VCE certificate")
                .code,
            ErrorCode::NonConvergence
        );

        let mut convergence = certified_composite_input();
        convergence.convergence.linear_within_relative_infinity_norm = -1.0;
        assert_eq!(
            CertifiedPpmlResult::new(convergence)
                .expect_err("convergence certificate")
                .code,
            ErrorCode::NonConvergence
        );

        let mut adjustment = certified_composite_input();
        adjustment.adjustment.prepared_as_offset = true;
        assert_eq!(
            CertifiedPpmlResult::new(adjustment)
                .expect_err("adjustment identity")
                .code,
            ErrorCode::Internal
        );

        let mut memory = certified_composite_input();
        memory.memory.plan.dimensions.observations += 1;
        assert_eq!(
            CertifiedPpmlResult::new(memory)
                .expect_err("memory identity")
                .code,
            ErrorCode::Internal
        );

        let mut generated = certified_composite_input();
        generated.coefficients.generated_columns[0] = None;
        assert_eq!(
            CertifiedPpmlResult::new(generated)
                .expect_err("generated-column identity")
                .code,
            ErrorCode::Internal
        );
    }

    #[test]
    fn clustered_subset_identity_and_declared_metadata_fail_closed() {
        CertifiedPpmlResult::new(certified_clustered_composite_input())
            .expect("valid one-way clustered result");

        let mut subset = certified_clustered_composite_input();
        subset.inference.vce.cluster_subsets[0].sign = -1;
        assert_eq!(
            CertifiedPpmlResult::new(subset)
                .expect_err("cluster subset identity")
                .code,
            ErrorCode::Internal
        );

        let mut metadata = certified_clustered_composite_input();
        metadata.clusters[0].label.clear();
        assert_eq!(
            CertifiedPpmlResult::new(metadata)
                .expect_err("cluster declaration identity")
                .code,
            ErrorCode::Internal
        );
    }

    #[test]
    fn separation_generation_sample_mapping_and_method_ranges_fail_closed() {
        let mut hash = certified_composite_input();
        hash.separation.generation_hashes[0] ^= 1;
        assert_eq!(
            CertifiedPpmlResult::new(hash)
                .expect_err("generation hash")
                .code,
            ErrorCode::Internal
        );

        let mut sample_reason = certified_composite_input();
        sample_reason.sample.drop_reasons[0] = DropReason::SimplexSeparation;
        assert_eq!(
            CertifiedPpmlResult::new(sample_reason)
                .expect_err("sample reason identity")
                .code,
            ErrorCode::Internal
        );

        let mut out_of_range = certified_composite_input().separation;
        out_of_range.methods.push(SeparationMethodReport {
            generation: 0,
            method: SeparationMethod::Singletons,
            status: MethodStatus::Certified,
            iterations: 1,
            dropped_observations: vec![out_of_range.candidate_observations],
            singleton_drops: Vec::new(),
            certificate: None,
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 0,
            simplex_tableau_cells: 0,
            note: "adversarial out-of-range row".into(),
        });
        let candidate_original_rows = (0..out_of_range.candidate_observations).collect::<Vec<_>>();
        let disabled = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        assert!(validate_separation_report(
            &out_of_range,
            disabled,
            &candidate_original_rows,
            1,
            &[],
            0,
        )
        .is_err());
    }

    #[test]
    fn certified_result_binds_separation_none_to_its_exact_request_and_seal() {
        let mut default_substitution = certified_composite_input();
        assert!(default_substitution.separation.methods.is_empty());
        assert!(!default_substitution.separation_config.drop_singletons);
        assert_eq!(
            default_substitution.separation_config.methods,
            SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            }
        );
        default_substitution.separation_config = SeparationConfig::default();
        assert_eq!(
            CertifiedPpmlResult::new(default_substitution)
                .expect_err("an empty separation(none) report cannot impersonate the default chain")
                .code,
            ErrorCode::Internal
        );

        let mut report_mutation = certified_composite_input();
        report_mutation
            .separation
            .methods
            .push(SeparationMethodReport {
            generation: 0,
            method: SeparationMethod::IteratedRectifier,
            status: MethodStatus::NoSeparation,
            iterations: 1,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: Some(SeparationCertificate {
                proof:
                    crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 0.0,
                direction_scale: 0.0,
                positive_maximum: 0.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 8,
            simplex_tableau_cells: 0,
            note: "self-minted fixed-point certificate".into(),
        });
        assert_eq!(
            CertifiedPpmlResult::new(report_mutation)
                .expect_err(
                    "a self-minted no-separation certificate lacks an independent replay seal"
                )
                .code,
            ErrorCode::Internal
        );

        let mut generation_substitution = certified_composite_input();
        generation_substitution.sample.outcome[0] += 1.0;
        assert_eq!(
            CertifiedPpmlResult::new(generation_substitution)
                .expect_err("a replay seal cannot be transplanted onto another final outcome")
                .code,
            ErrorCode::Internal
        );
    }

    #[test]
    fn raw_domain_and_positive_support_keep_their_public_return_codes() {
        let sample = |outcome: Vec<f64>, prior_weights: Vec<f64>| {
            PpmlFinalSample::new(
                outcome.len(),
                outcome.len(),
                Arc::new((0..outcome.len()).collect::<Vec<_>>()),
                (0..outcome.len()).collect(),
                vec![DropReason::Retained; outcome.len()],
                outcome,
                prior_weights,
            )
        };

        let negative = sample(vec![-f64::MIN_POSITIVE], vec![1.0])
            .expect_err("negative outcome is outside the PPML domain");
        assert_eq!(negative.code, ErrorCode::InvalidData);
        assert_eq!(negative.return_code(), 459);

        let zero_weight = sample(vec![0.25], vec![0.0])
            .expect_err("nonpositive prior weight is outside the PPML domain");
        assert_eq!(zero_weight.code, ErrorCode::InvalidData);
        assert_eq!(zero_weight.return_code(), 459);

        let all_zero = sample(vec![0.0, 0.0], vec![1.0, 1.0])
            .expect_err("all-zero outcome has no estimable PPML support");
        assert_eq!(all_zero.code, ErrorCode::InsufficientObservations);
        assert_eq!(all_zero.return_code(), 2001);

        sample(vec![0.0, 0.25], vec![1.0, 1.0])
            .expect("finite nonnegative fractional outcomes are in-domain");
    }

    #[test]
    fn separation_report_envelope_is_exact_for_all_policy_subsets() {
        let candidate_original_rows = vec![7_usize];
        let identity_hash =
            candidate_ledger_hash_candidate_axis(&candidate_original_rows, &[true], &[None])
                .expect("identity candidate ledger hash");
        for policy_bits in 0_u8..32 {
            let config = SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: policy_bits & 0b0_0010 != 0,
                    simplex: policy_bits & 0b0_0100 != 0,
                    iterated_rectifier: policy_bits & 0b0_1000 != 0,
                    mu: policy_bits & 0b1_0000 != 0,
                },
                drop_singletons: policy_bits & 0b0_0001 != 0,
                ..SeparationConfig::default()
            };
            let expected_policies = policy_bits.count_ones() as usize;
            assert_eq!(separation_method_count(config), expected_policies);
            let mut report = SeparationReport {
                candidate_observations: 1,
                active: vec![true],
                retained_observations: vec![0],
                dropped_observations: Vec::new(),
                drop_reasons: vec![None],
                singleton_count: 0,
                separated_count: 0,
                generations: 0,
                generation_hashes: vec![identity_hash],
                methods: Vec::new(),
            };
            if expected_policies == 0 {
                validate_separation_report(&report, config, &candidate_original_rows, 1, &[], 0)
                    .expect("keepsingletons plus separation(none) has a zero-report envelope");
                continue;
            }
            report
                .methods
                .resize_with(expected_policies + 1, || SeparationMethodReport {
                    generation: 0,
                    method: SeparationMethod::Singletons,
                    status: MethodStatus::NoSeparation,
                    iterations: 1,
                    dropped_observations: Vec::new(),
                    singleton_drops: Vec::new(),
                    certificate: None,
                    exact_fe_certificate: None,
                    elapsed_seconds: 0.0,
                    workspace_observation_vectors: 0,
                    simplex_tableau_cells: 0,
                    note: "prepared singleton pruning independently certified a stable fixed point"
                        .into(),
                });
            assert!(validate_separation_report(
                &report,
                config,
                &candidate_original_rows,
                1,
                &[],
                0,
            )
            .is_err());
        }

        let mut impossible_generation = SeparationReport {
            candidate_observations: 1,
            active: vec![true],
            retained_observations: vec![0],
            dropped_observations: Vec::new(),
            drop_reasons: vec![None],
            singleton_count: 0,
            separated_count: 0,
            generations: 1,
            generation_hashes: vec![identity_hash, identity_hash],
            methods: Vec::new(),
        };
        let disabled = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        assert!(validate_separation_report(
            &impossible_generation,
            disabled,
            &candidate_original_rows,
            1,
            &[],
            0,
        )
        .is_err());
        impossible_generation.generations = 0;
        assert!(validate_separation_report(
            &impossible_generation,
            disabled,
            &candidate_original_rows,
            1,
            &[],
            0,
        )
        .is_err());
    }

    #[test]
    fn exact_fe_identities_bind_intercept_terms_and_direct_drop_anchors() {
        use crate::metadata::AbsorbTermId;
        use crate::ppml_separation::{ExactFeLevelIdentity, ExactFeSeparationCertificate};

        let fixed_effect = |declared_channel, term_id, kind| PpmlFixedEffectResult {
            declared_channel,
            term_id,
            source_ids: vec![term_id],
            kind,
            slope_source_id: (kind == PpmlFixedEffectChannelKind::Slope).then_some(term_id),
            levels: 3,
            categories: 3,
            redundant_parameters: 0,
            absorbed_rank: 2,
            redundancy_exact: true,
            nested: false,
            saved_channel: None,
            normalization_applied: true,
            normalization_certified: true,
            reconstruction_certified: true,
        };
        let fixed_effects = vec![
            fixed_effect(0, 1, PpmlFixedEffectChannelKind::Intercept),
            fixed_effect(1, 1, PpmlFixedEffectChannelKind::Slope),
            fixed_effect(2, 2, PpmlFixedEffectChannelKind::Intercept),
            fixed_effect(3, 3, PpmlFixedEffectChannelKind::Slope),
        ];
        let candidate_original_rows = vec![10_usize, 20, 30];
        let mut method = SeparationMethodReport {
            generation: 1,
            method: SeparationMethod::FixedEffects,
            status: MethodStatus::Certified,
            iterations: 1,
            dropped_observations: vec![0, 1],
            singleton_drops: Vec::new(),
            certificate: None,
            exact_fe_certificate: Some(ExactFeSeparationCertificate {
                checked_intercept_effects: 2,
                zero_only_levels: vec![
                    ExactFeLevelIdentity {
                        effect: AbsorbTermId::new(1).expect("first intercept term"),
                        representative_original_row: 10,
                    },
                    ExactFeLevelIdentity {
                        effect: AbsorbTermId::new(2).expect("second intercept term"),
                        representative_original_row: 20,
                    },
                ],
                passed: true,
            }),
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 1,
            simplex_tableau_cells: 0,
            note: "prepared exact zero-only FE certificate passed".into(),
        };
        assert_eq!(
            validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
                .expect("stable exact-FE identities"),
            2
        );

        let certificate = method
            .exact_fe_certificate
            .as_mut()
            .expect("exact-FE certificate");
        certificate.zero_only_levels[1].representative_original_row = 30;
        assert!(
            validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
                .is_err()
        );
        method
            .exact_fe_certificate
            .as_mut()
            .expect("exact-FE certificate")
            .zero_only_levels[1] = ExactFeLevelIdentity {
            effect: AbsorbTermId::new(3).expect("slope-only term"),
            representative_original_row: 20,
        };
        assert!(
            validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
                .is_err()
        );
        let first = method
            .exact_fe_certificate
            .as_ref()
            .expect("exact-FE certificate")
            .zero_only_levels[0];
        method
            .exact_fe_certificate
            .as_mut()
            .expect("exact-FE certificate")
            .zero_only_levels[1] = first;
        assert!(
            validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
                .is_err()
        );

        method.status = MethodStatus::NoSeparation;
        method.dropped_observations.clear();
        method.note = "prepared exact FE scan found no zero-only intercept level".into();
        method
            .exact_fe_certificate
            .as_mut()
            .expect("exact-FE certificate")
            .zero_only_levels
            .clear();
        assert_eq!(
            validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
                .expect("empty exact-FE set proves no separation"),
            0
        );
        method
            .exact_fe_certificate
            .as_mut()
            .expect("exact-FE certificate")
            .zero_only_levels
            .push(first);
        assert!(
            validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
                .is_err()
        );
    }

    #[test]
    fn simplex_full_positive_rank_proof_is_bound_to_the_final_finite_dimension() {
        use crate::ppml_separation::SeparationCertificateProof;

        let mut report = certified_composite_input().separation;
        report.methods.push(SeparationMethodReport {
            generation: 0,
            method: SeparationMethod::Simplex,
            status: MethodStatus::NoSeparation,
            iterations: 1,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: Some(SeparationCertificate {
                proof: SeparationCertificateProof::SimplexFullPositiveRank { columns: 1 },
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 0.0,
                direction_scale: 0.0,
                positive_maximum: 0.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 0,
            simplex_tableau_cells: 1,
            note: "finite regressors have full column rank on the positive-outcome sample".into(),
        });
        let candidate_original_rows = (0..report.candidate_observations).collect::<Vec<_>>();
        let simplex_only = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        validate_separation_report(&report, simplex_only, &candidate_original_rows, 1, &[], 0)
            .expect("full-positive-rank proof closes simplex exactly");

        let mut wrong_dimension = report.clone();
        wrong_dimension.methods[0]
            .certificate
            .as_mut()
            .expect("simplex certificate")
            .proof = SeparationCertificateProof::SimplexFullPositiveRank { columns: 2 };
        let error = validate_separation_report(
            &wrong_dimension,
            simplex_only,
            &candidate_original_rows,
            1,
            &[],
            0,
        )
        .expect_err("proof must bind the final finite dimension");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.return_code(), 3499);

        let mut failed = report;
        failed.methods[0]
            .certificate
            .as_mut()
            .expect("simplex certificate")
            .passed = false;
        let error =
            validate_separation_report(&failed, simplex_only, &candidate_original_rows, 1, &[], 0)
                .expect_err("failed exact proof is not publishable");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(error.return_code(), 430);
    }

    #[test]
    fn unresolved_simplex_requires_a_certified_comprehensive_ir_closure() {
        use crate::ppml_separation::SeparationCertificateProof;

        let mut unresolved = certified_composite_input().separation;
        unresolved.methods.push(SeparationMethodReport {
            generation: 0,
            method: SeparationMethod::Simplex,
            status: MethodStatus::Uncertified,
            iterations: 1,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: None,
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 0,
            simplex_tableau_cells: 1,
            note: "model simplex phase-I dual failed independent Farkas verification".into(),
        });
        let candidate_original_rows = (0..unresolved.candidate_observations).collect::<Vec<_>>();
        let simplex_only = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let error = validate_separation_report(
            &unresolved,
            simplex_only,
            &candidate_original_rows,
            1,
            &[],
            0,
        )
        .expect_err("terminal simplex uncertainty must fail closed");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(error.return_code(), 430);

        unresolved.methods.push(SeparationMethodReport {
            generation: 0,
            method: SeparationMethod::IteratedRectifier,
            status: MethodStatus::NoSeparation,
            iterations: 1,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: Some(SeparationCertificate {
                proof: SeparationCertificateProof::IteratedRectifierFixedPoint,
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 0.0,
                direction_scale: 1.0,
                positive_maximum: 0.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 8,
            simplex_tableau_cells: 0,
            note: "prepared iterated rectifier certified no separation".into(),
        });
        let simplex_ir = SeparationConfig {
            methods: SeparationMethods {
                iterated_rectifier: true,
                ..simplex_only.methods
            },
            ..simplex_only
        };
        validate_separation_report(&unresolved, simplex_ir, &candidate_original_rows, 1, &[], 0)
            .expect("same-generation comprehensive IR closes simplex uncertainty");
    }

    #[test]
    fn ir_commit_does_not_retroactively_close_simplex_across_generations() {
        use crate::ppml_separation::SeparationCertificateProof;

        let candidate_original_rows = vec![0_usize, 1, 2, 3];
        let initial_active = vec![true; 4];
        let initial_reasons = vec![None; 4];
        let committed_active = vec![false, true, true, true];
        let committed_reasons = vec![
            Some(SeparationDropReason::IteratedRectifier),
            None,
            None,
            None,
        ];
        let mut report = SeparationReport {
            candidate_observations: 4,
            active: committed_active.clone(),
            retained_observations: vec![1, 2, 3],
            dropped_observations: vec![0],
            drop_reasons: committed_reasons.clone(),
            singleton_count: 0,
            separated_count: 1,
            generations: 1,
            generation_hashes: vec![
                candidate_ledger_hash_candidate_axis(
                    &candidate_original_rows,
                    &initial_active,
                    &initial_reasons,
                )
                .expect("initial ledger hash"),
                candidate_ledger_hash_candidate_axis(
                    &candidate_original_rows,
                    &committed_active,
                    &committed_reasons,
                )
                .expect("committed ledger hash"),
            ],
            methods: vec![
                SeparationMethodReport {
                    generation: 0,
                    method: SeparationMethod::Simplex,
                    status: MethodStatus::Uncertified,
                    iterations: 1,
                    dropped_observations: Vec::new(),
                    singleton_drops: Vec::new(),
                    certificate: None,
                    exact_fe_certificate: None,
                    elapsed_seconds: 0.0,
                    workspace_observation_vectors: 0,
                    simplex_tableau_cells: 1,
                    note: "model simplex phase-I dual failed independent Farkas verification"
                        .into(),
                },
                SeparationMethodReport {
                    generation: 1,
                    method: SeparationMethod::IteratedRectifier,
                    status: MethodStatus::Certified,
                    iterations: 1,
                    dropped_observations: vec![0],
                    singleton_drops: Vec::new(),
                    certificate: Some(SeparationCertificate {
                        proof: SeparationCertificateProof::RecessionDirection,
                        interior_zero_relative: 0.0,
                        boundary_violation_relative: 0.0,
                        span_residual_relative: 0.0,
                        negative_margin_relative: 1.0,
                        direction_scale: 1.0,
                        positive_maximum: 0.0,
                        passed: true,
                    }),
                    exact_fe_certificate: None,
                    elapsed_seconds: 0.0,
                    workspace_observation_vectors: 8,
                    simplex_tableau_cells: 0,
                    note: "prepared iterated-rectifier candidate direction".into(),
                },
            ],
        };
        let simplex_ir = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let error =
            validate_separation_report(&report, simplex_ir, &candidate_original_rows, 1, &[], 0)
                .expect_err("an IR deletion cannot certify the restarted generation");
        assert_eq!(error.code, ErrorCode::NonConvergence);

        report.methods.extend([
            SeparationMethodReport {
                generation: 1,
                method: SeparationMethod::Simplex,
                status: MethodStatus::NoSeparation,
                iterations: 0,
                dropped_observations: Vec::new(),
                singleton_drops: Vec::new(),
                certificate: Some(SeparationCertificate {
                    proof: SeparationCertificateProof::SimplexFullPositiveRank { columns: 1 },
                    interior_zero_relative: 0.0,
                    boundary_violation_relative: 0.0,
                    span_residual_relative: 0.0,
                    negative_margin_relative: 0.0,
                    direction_scale: 0.0,
                    positive_maximum: 0.0,
                    passed: true,
                }),
                exact_fe_certificate: None,
                elapsed_seconds: 0.0,
                workspace_observation_vectors: 0,
                simplex_tableau_cells: 1,
                note: "finite regressors have full column rank on the positive-outcome sample"
                    .into(),
            },
            SeparationMethodReport {
                generation: 1,
                method: SeparationMethod::IteratedRectifier,
                status: MethodStatus::NoSeparation,
                iterations: 1,
                dropped_observations: Vec::new(),
                singleton_drops: Vec::new(),
                certificate: Some(SeparationCertificate {
                    proof: SeparationCertificateProof::IteratedRectifierFixedPoint,
                    interior_zero_relative: 0.0,
                    boundary_violation_relative: 0.0,
                    span_residual_relative: 0.0,
                    negative_margin_relative: 0.0,
                    direction_scale: 0.0,
                    positive_maximum: 0.0,
                    passed: true,
                }),
                exact_fe_certificate: None,
                elapsed_seconds: 0.0,
                workspace_observation_vectors: 8,
                simplex_tableau_cells: 0,
                note: "prepared iterated rectifier certified no separation".into(),
            },
        ]);
        validate_separation_report(&report, simplex_ir, &candidate_original_rows, 1, &[], 0)
            .expect("the restarted generation has its own complete detector chain");

        let mut missing_commit = report;
        missing_commit.generation_hashes.pop();
        let error = validate_separation_report(
            &missing_commit,
            simplex_ir,
            &candidate_original_rows,
            1,
            &[],
            0,
        )
        .expect_err("IR closure requires the committed generation receipt");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.return_code(), 3499);
    }

    #[test]
    fn zero_direction_is_only_valid_for_exact_zero_no_separation_certificate() {
        let certificate = SeparationCertificate {
            proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
            interior_zero_relative: 0.0,
            boundary_violation_relative: 0.0,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale: 0.0,
            positive_maximum: 0.0,
            passed: true,
        };
        validate_separation_certificate(&certificate, true)
            .expect("zero numerical direction is valid no-separation evidence");
        assert!(validate_separation_certificate(&certificate, false).is_err());

        let mut nonzero_diagnostic = certificate;
        nonzero_diagnostic.span_residual_relative = f64::EPSILON;
        assert!(validate_separation_certificate(&nonzero_diagnostic, true).is_err());
    }

    #[test]
    fn mu_no_drop_grammar_is_proof_free_no_candidates_only() {
        use crate::ppml_separation::SeparationCertificateProof;

        let mut report = certified_composite_input().separation;
        report.methods.push(SeparationMethodReport {
            generation: 0,
            method: SeparationMethod::Mu,
            status: MethodStatus::NoSeparation,
            iterations: 1,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: Some(SeparationCertificate {
                proof: SeparationCertificateProof::RecessionDirection,
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 0.0,
                direction_scale: 0.0,
                positive_maximum: 0.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 8,
            simplex_tableau_cells: 0,
            note: "forged Mu no-separation claim".into(),
        });
        let candidate_original_rows = (0..report.candidate_observations).collect::<Vec<_>>();
        let mu_only = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: true,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let error =
            validate_separation_report(&report, mu_only, &candidate_original_rows, 1, &[], 0)
                .expect_err("Mu cannot certify absence of separation");
        assert_eq!(error.code, ErrorCode::NonConvergence);

        let terminal_mu = report.methods.last_mut().expect("Mu report");
        terminal_mu.status = MethodStatus::NoCandidates;
        terminal_mu.iterations = 0;
        terminal_mu.certificate = None;
        terminal_mu.note = "prepared mu found no small zero-outcome means".into();
        validate_separation_report(&report, mu_only, &candidate_original_rows, 1, &[], 0)
            .expect("proof-free Mu no-candidate result is the sole no-drop grammar");
    }

    #[test]
    fn clipped_covariance_validation_certifies_the_published_matrix() {
        // Q is an independently constructed Householder reflection. The raw
        // spectrum has 22 positive and 43 negative eigenvalues, so its exact
        // PSD projection is Q diag(max(lambda, 0)) Q'.
        let dimension = 65;
        let tolerance = 1e-10;
        let vector: Vec<f64> = (0..dimension)
            .map(|i| (((i + 1) * 137 + 11 * 53) % 251) as f64 - 125.0)
            .collect();
        let squared_norm: f64 = vector.iter().map(|value| value * value).sum();
        let orthogonal: Vec<f64> = (0..dimension * dimension)
            .map(|i| {
                f64::from(u8::from(i / dimension == i % dimension))
                    - 2.0 * vector[i / dimension] * vector[i % dimension] / squared_norm
            })
            .collect();
        let eigenvalues: Vec<f64> = (0..dimension)
            .map(|j| {
                if j % 3 == 0 {
                    1.0 + j as f64
                } else {
                    -(1.0 + j as f64) / 7.0
                }
            })
            .collect();
        let mut raw = vec![0.0; dimension * dimension];
        let mut expected = vec![0.0; dimension * dimension];
        for row in 0..dimension {
            for column in row..dimension {
                let mut value = 0.0;
                let mut clipped = 0.0;
                for eigen in 0..dimension {
                    value += orthogonal[row * dimension + eigen]
                        * eigenvalues[eigen]
                        * orthogonal[column * dimension + eigen];
                    clipped += orthogonal[row * dimension + eigen]
                        * eigenvalues[eigen].max(0.0)
                        * orthogonal[column * dimension + eigen];
                }
                raw[row * dimension + column] = value;
                raw[column * dimension + row] = value;
                expected[row * dimension + column] = clipped;
                expected[column * dimension + row] = clipped;
            }
        }
        let (published, report) = apply_ppml_psd_policy(&raw, dimension, 2, tolerance).unwrap();
        assert_eq!(report.final_rank, 22);
        assert_eq!(report.clipped_eigenvalues, 43);
        assert!(matrix_relative_error(&published, &expected).unwrap() < tolerance);
        let coefficients = PpmlCoefficientOutput::new(
            vec![0.0; dimension],
            vec![PpmlCoefficientStatus::Identified; dimension],
            (0..dimension).map(Some).collect(),
            (0..dimension).collect(),
        )
        .unwrap();
        let mut inference = certified_composite_input().inference;
        inference.vce.covariance = published.clone();
        inference.vce.standard_errors = (0..dimension)
            .map(|index| published[index * dimension + index].sqrt())
            .collect();
        inference.vce.cluster_dimension_groups = vec![97, 3];
        inference.vce.psd = report;
        validate_covariance_and_certificates(&coefficients, &inference, tolerance)
            .expect("the certificate must describe the actual published covariance");
        assert_eq!(inference.vce.covariance, published);
        for mutation in 0..8 {
            inference.vce.psd = report;
            match mutation {
                0 => inference.vce.psd.final_rank += 1,
                1 => inference.vce.psd.clipped_eigenvalues = 0,
                2 => inference.vce.psd.adjusted = false,
                3 => inference.vce.psd.raw_minimum_eigenvalue = 0.0,
                4 => inference.vce.psd.final_minimum_eigenvalue = 1.0,
                5 => inference.vce.psd.orthogonality_relative_error = 0.0,
                6 => inference.vce.psd.eigensystem_reconstruction_relative_error = 0.0,
                _ => inference.vce.psd.passed = false,
            }
            assert_eq!(
                validate_covariance_and_certificates(&coefficients, &inference, tolerance)
                    .expect_err("an inconsistent report must still be rejected")
                    .code,
                ErrorCode::NonConvergence,
            );
        }
        inference.vce.psd = report;
        inference.vce.covariance[1] = 1000.0;
        inference.vce.covariance[dimension] = 1000.0;
        assert_eq!(
            validate_covariance_and_certificates(&coefficients, &inference, tolerance)
                .expect_err("a materially indefinite matrix cannot reuse a valid report")
                .code,
            ErrorCode::NonConvergence,
        );
    }

    #[test]
    fn identified_zero_variance_uses_zero_se_and_missing_z() {
        let mut input = certified_composite_input();
        input.inference.vce.covariance[0] = 0.0;
        input.inference.vce.standard_errors[0] = 0.0;
        input.inference.vce.z_statistics[0] = f64::NAN;
        input.inference.vce.psd = apply_ppml_psd_policy(&[0.0], 1, 0, 1e-10)
            .expect("zero covariance PSD certificate")
            .1;
        CertifiedPpmlResult::new(input).expect("PSD-singular identified inference");

        let mut mismatch = certified_composite_input();
        mismatch.inference.vce.covariance[0] = 0.0;
        mismatch.inference.vce.standard_errors[0] = 0.0;
        mismatch.inference.vce.z_statistics[0] = 0.0;
        mismatch.inference.vce.psd = apply_ppml_psd_policy(&[0.0], 1, 0, 1e-10)
            .expect("zero covariance PSD certificate")
            .1;
        assert_eq!(
            CertifiedPpmlResult::new(mismatch)
                .expect_err("zero SE must not report a finite z")
                .code,
            ErrorCode::Internal
        );
    }

    #[test]
    fn normalized_structural_constant_must_be_exactly_zero() {
        let error = PpmlCoefficientOutput::new(
            vec![1.0],
            vec![PpmlCoefficientStatus::NormalizedStructuralConstant],
            vec![Some(0)],
            Vec::new(),
        )
        .expect_err("nonzero normalized constant must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
    }

    #[test]
    fn fixed_effect_rows_require_explicit_channel_identity_and_exact_rank_sum() {
        let output = exact_state(true);
        let effects = vec![
            PpmlFixedEffectResult {
                declared_channel: 0,
                term_id: 1,
                source_ids: vec![1],
                kind: PpmlFixedEffectChannelKind::Intercept,
                slope_source_id: None,
                levels: 2,
                categories: 2,
                redundant_parameters: 1,
                absorbed_rank: 1,
                redundancy_exact: true,
                nested: false,
                saved_channel: Some(0),
                normalization_applied: true,
                normalization_certified: true,
                reconstruction_certified: true,
            },
            PpmlFixedEffectResult {
                declared_channel: 1,
                term_id: 1,
                source_ids: vec![1],
                kind: PpmlFixedEffectChannelKind::Slope,
                slope_source_id: Some(2),
                levels: 2,
                categories: 2,
                redundant_parameters: 0,
                absorbed_rank: 2,
                redundancy_exact: false,
                nested: false,
                saved_channel: Some(1),
                normalization_applied: true,
                normalization_certified: true,
                reconstruction_certified: true,
            },
        ];
        let coefficients = PpmlCoefficientOutput::new(
            vec![0.0],
            vec![PpmlCoefficientStatus::NormalizedStructuralConstant],
            vec![Some(0)],
            Vec::new(),
        )
        .expect("normalized constant");
        validate_fixed_effect_metadata(&effects, &output, 3, &coefficients).expect("valid FE rows");

        let mut missing_slope_identity = effects.clone();
        missing_slope_identity[1].slope_source_id = None;
        assert_eq!(
            validate_fixed_effect_metadata(&missing_slope_identity, &output, 3, &coefficients)
                .expect_err("slope identity")
                .code,
            ErrorCode::Internal
        );
        assert_eq!(
            validate_fixed_effect_metadata(&effects, &output, 2, &coefficients)
                .expect_err("rank mismatch")
                .code,
            ErrorCode::Internal
        );
        let mut inconsistent_levels = effects.clone();
        inconsistent_levels[0].levels = 3;
        assert_eq!(
            validate_fixed_effect_metadata(&inconsistent_levels, &output, 3, &coefficients)
                .expect_err("level/category identity")
                .code,
            ErrorCode::Internal
        );
        let mut unapplied_saved_normalization = effects.clone();
        unapplied_saved_normalization[0].normalization_applied = false;
        assert_eq!(
            validate_fixed_effect_metadata(
                &unapplied_saved_normalization,
                &output,
                3,
                &coefficients,
            )
            .expect_err("saved normalization evidence")
            .code,
            ErrorCode::Internal
        );
    }
}
