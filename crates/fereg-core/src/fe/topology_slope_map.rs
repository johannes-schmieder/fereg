//! Allocation-bounded certified MAP for categorical intercept and slope effects.
//!
//! The finalized categorical topology remains the sole owner of compact codes
//! and membership.  Continuous slope columns are borrowed from the prepared
//! numeric sample.  All numerical workspace is reserved before the first
//! projection; iterations and final certificates allocate nothing.

use std::time::Instant;

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};

use super::{
    absorb_borrowed_intercept_cg_recovery_certified, borrow_categorical_topology_design,
    connected_components_all, recover_borrowed_intercept_sd_lsmr_coefficients,
    BorrowedInterceptRecoveryMethod, BorrowedMapWeights, CertifiedBorrowedMap, FeCertificate,
    FeCoefficientBatch, FeSolverKind, FinalizedCategoricalTopology, LsmrConfig, LsmrPreconditioner,
    MapConfig, SolveReport, SolveStopReason,
};
use within::{Effect, LsmrOptions, PreconditionerConfig};

#[derive(Debug, Clone, Copy)]
struct ValidatedWeights<'a> {
    values: Option<&'a [f64]>,
    raw_scale: f64,
}

impl ValidatedWeights<'_> {
    fn value(self, row: usize) -> f64 {
        self.values
            .map_or(1.0, |values| values[row] / self.raw_scale)
    }
}

/// One borrowed absorbed-effect basis in requested term order.
///
/// Categorical codes and membership remain owned by the finalized topology;
/// continuous channels remain owned by the prepared numeric sample.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoricalAbsorbedEffectView<'a> {
    intercept: bool,
    slopes: Vec<&'a [f64]>,
}

/// Certified recovery of the absorbed contribution and any requested
/// term/channel contributions on the final compact observation axis.
///
/// Channels use absorbed-term order, with an intercept first when present and
/// slopes following in their declared order. Unrequested channels retain a
/// `None` slot so callers cannot accidentally lose wire identity.
#[derive(Debug, PartialEq)]
pub struct RecoveredCategoricalEffects {
    absorbed_sum: Vec<f64>,
    channels: Vec<Option<Vec<f64>>>,
    iterations: usize,
    reconstruction_relative_infinity: f64,
    recovery_method: CategoricalRecoveryMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoricalRecoveryMethod {
    RetainedCoefficients,
    Direct,
    SteepestDescent,
    HybridLsmr,
    Cg,
    Lsmr,
    Map,
}

impl RecoveredCategoricalEffects {
    #[must_use]
    pub fn absorbed_sum(&self) -> &[f64] {
        &self.absorbed_sum
    }

    #[must_use]
    pub fn channels(&self) -> &[Option<Vec<f64>>] {
        &self.channels
    }

    #[must_use]
    pub const fn iterations(&self) -> usize {
        self.iterations
    }

    #[must_use]
    pub const fn reconstruction_relative_infinity(&self) -> f64 {
        self.reconstruction_relative_infinity
    }

    #[cfg(test)]
    pub(crate) const fn recovery_method(&self) -> CategoricalRecoveryMethod {
        self.recovery_method
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<f64>, Vec<Option<Vec<f64>>>) {
        (self.absorbed_sum, self.channels)
    }
}

impl<'a> CategoricalAbsorbedEffectView<'a> {
    pub fn new(intercept: bool, slopes: Vec<&'a [f64]>) -> Result<Self> {
        if !intercept && slopes.is_empty() {
            return Err(invalid_argument(
                "categorical absorbed effect must contain an intercept or slope",
            ));
        }
        Ok(Self { intercept, slopes })
    }

    #[must_use]
    pub const fn has_intercept(&self) -> bool {
        self.intercept
    }

    #[must_use]
    pub fn slopes(&self) -> &[&'a [f64]] {
        &self.slopes
    }

    #[must_use]
    pub fn channels(&self) -> usize {
        usize::from(self.intercept) + self.slopes.len()
    }

    fn basis_value(&self, channel: usize, row: usize, scales: &[f64]) -> f64 {
        if self.intercept {
            if channel == 0 {
                1.0
            } else {
                self.slopes[channel - 1][row] / scales[channel]
            }
        } else {
            self.slopes[channel][row] / scales[channel]
        }
    }
}

trait SlopeWorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_usize(
        &mut self,
        target: &mut Vec<usize>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemSlopeWorkspaceReservation;

impl SlopeWorkspaceReservation for SystemSlopeWorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }

    fn reserve_usize(
        &mut self,
        target: &mut Vec<usize>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

struct SlopeWorkspace {
    max_channels: usize,
    reference_norms: Vec<f64>,
    residual_norms: Vec<f64>,
    rhs_scales: Vec<f64>,
    value_scales: Vec<f64>,
    channel_scales: Vec<f64>,
    gram: Vec<f64>,
    gram_corrections: Vec<f64>,
    factor: Vec<f64>,
    cross: Vec<f64>,
    cross_corrections: Vec<f64>,
    factor_cross: Vec<f64>,
    coefficients: Vec<f64>,
    permutation: Vec<usize>,
}

impl SlopeWorkspace {
    fn new<R: SlopeWorkspaceReservation>(
        max_channels: usize,
        rhs: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        if max_channels == 0 {
            return Err(invalid_argument(
                "categorical slope MAP requires at least one basis channel",
            ));
        }
        let gram_elements = max_channels
            .checked_mul(max_channels)
            .ok_or_else(|| allocation("categorical slope MAP Gram workspace overflows"))?;
        let rhs_elements = max_channels
            .checked_mul(rhs)
            .ok_or_else(|| allocation("categorical slope MAP RHS workspace overflows"))?;
        Ok(Self {
            max_channels,
            reference_norms: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP reference norms",
            )?,
            residual_norms: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP residual norms",
            )?,
            rhs_scales: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP RHS scales",
            )?,
            value_scales: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP value scales",
            )?,
            channel_scales: reserved_zeros(
                max_channels,
                reservation,
                "unable to reserve categorical slope MAP channel scales",
            )?,
            gram: reserved_zeros(
                gram_elements,
                reservation,
                "unable to reserve categorical slope MAP Gram matrix",
            )?,
            gram_corrections: reserved_zeros(
                gram_elements,
                reservation,
                "unable to reserve categorical slope MAP Gram corrections",
            )?,
            factor: reserved_zeros(
                gram_elements,
                reservation,
                "unable to reserve categorical slope MAP factor matrix",
            )?,
            cross: reserved_zeros(
                rhs_elements,
                reservation,
                "unable to reserve categorical slope MAP cross-products",
            )?,
            cross_corrections: reserved_zeros(
                rhs_elements,
                reservation,
                "unable to reserve categorical slope MAP cross-product corrections",
            )?,
            factor_cross: reserved_zeros(
                rhs_elements,
                reservation,
                "unable to reserve categorical slope MAP factored cross-products",
            )?,
            coefficients: reserved_zeros(
                rhs_elements,
                reservation,
                "unable to reserve categorical slope MAP coefficients",
            )?,
            permutation: reserved_indices(
                max_channels,
                reservation,
                "unable to reserve categorical slope MAP permutation",
            )?,
        })
    }
}

/// Compatibility wrapper for the previously qualified intercept-plus-one-slope
/// surface. New code should use `absorb_finalized_categorical_slope_map_certified`.
pub fn absorb_finalized_categorical_intercept_slope_map_certified(
    topology: &FinalizedCategoricalTopology,
    slopes: &[Option<&[f64]>],
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(slopes.len())
        .map_err(|_| allocation("unable to reserve compatibility absorbed effects"))?;
    for slope in slopes {
        let mut channels = Vec::new();
        channels
            .try_reserve_exact(usize::from(slope.is_some()))
            .map_err(|_| allocation("unable to reserve compatibility slope channels"))?;
        if let Some(values) = slope {
            channels.push(*values);
        }
        effects.push(CategoricalAbsorbedEffectView::new(true, channels)?);
    }
    absorb_finalized_categorical_slope_map_certified(topology, &effects, weights, values, config)
}

/// Residualize arbitrary categorical intercept, slope-only, and
/// intercept-plus-slope effects without copying observation-scale topology or
/// continuous source columns.
pub fn absorb_finalized_categorical_slope_map_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemSlopeWorkspaceReservation;
    absorb_with_reservation(topology, effects, weights, values, config, &mut reservation)
}

/// Recover one normalized MAP decomposition of an absorbed contribution.
///
/// Starting from zero and accumulating every ordered projection reproduces the
/// same weighted-zero-mean normalization used by the pinned reference for
/// intercept-only effects. The total absorbed contribution is always returned;
/// observation-scale channel vectors are allocated only when requested.
pub fn recover_finalized_categorical_effects_map_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested_channels: &[bool],
    config: MapConfig,
) -> Result<RecoveredCategoricalEffects> {
    let mut reservation = SystemSlopeWorkspaceReservation;
    recover_with_reservation(
        topology,
        effects,
        weights,
        target,
        requested_channels,
        config,
        &mut reservation,
    )
}

/// Recover normalized categorical effects from a coefficient product retained
/// by the estimator. This path performs no iterative solve: it normalizes the
/// solver's null-space representative, scatters requested channels, and
/// independently certifies reconstruction and FE orthogonality.
pub(crate) fn recover_finalized_categorical_effects_from_coefficients_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested_channels: &[bool],
    coefficients: &FeCoefficientBatch,
    tolerance: f64,
) -> Result<RecoveredCategoricalEffects> {
    validate_recovery_inputs(topology, effects, target, requested_channels)?;
    validate_coefficient_layout(topology, effects, coefficients)?;
    if coefficients.rhs_count() != 1 {
        return Err(invalid_argument(
            "categorical recovery requires exactly one combined coefficient RHS",
        ));
    }
    let weights = validate_weights(weights, target.len())?;
    let mut normalized = coefficients.rhs(0)?.to_vec();
    normalize_intercept_components(topology, effects, weights, &mut normalized)?;
    finish_channel_major_recovery(
        topology,
        effects,
        weights,
        target,
        requested_channels,
        &normalized,
        0,
        tolerance,
        CategoricalRecoveryMethod::RetainedCoefficients,
    )
}

/// Recover effects with one coefficient-producing modified-LSMR solve.
///
/// This is the bounded fallback for observation-space MAP/CG/extrapolation
/// routes and for low-memory fits that deliberately do not retain all initial
/// RHS coefficient vectors.
pub(crate) fn recover_finalized_categorical_effects_lsmr_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested_channels: &[bool],
    config: LsmrConfig,
    publication_tolerance: f64,
) -> Result<RecoveredCategoricalEffects> {
    validate_recovery_inputs(topology, effects, target, requested_channels)?;
    if !config.tolerance.is_finite()
        || config.tolerance <= 0.0
        || config.max_iterations == 0
        || config.local_size == Some(0)
        || !publication_tolerance.is_finite()
        || publication_tolerance <= 0.0
    {
        return Err(invalid_argument(
            "categorical recovery LSMR policy is invalid",
        ));
    }
    let source_table = topology_source_table(topology)?;
    let mut within_effects = Vec::new();
    within_effects
        .try_reserve_exact(effects.len())
        .map_err(|_| allocation("unable to reserve categorical recovery LSMR effects"))?;
    let mut effect_channels = Vec::new();
    let mut level_counts = Vec::new();
    effect_channels
        .try_reserve_exact(effects.len())
        .map_err(|_| allocation("unable to reserve categorical recovery channel layout"))?;
    level_counts
        .try_reserve_exact(effects.len())
        .map_err(|_| allocation("unable to reserve categorical recovery level layout"))?;
    for (effect_index, effect) in effects.iter().enumerate() {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?;
        let codes = source.codes(&source_table)?;
        within_effects.push(
            Effect::new(
                codes,
                effect.has_intercept(),
                effect.slopes().iter().copied(),
            )
            .map_err(|error| {
                invalid_data(format!(
                    "unable to construct categorical recovery LSMR effect: {error}"
                ))
            })?,
        );
        effect_channels.push(effect.channels());
        level_counts.push(source.n_levels());
    }
    let options = LsmrOptions {
        tol: config.tolerance,
        maxiter: config.max_iterations,
        local_size: config.local_size,
    };
    let coefficient_count = effect_channels.iter().zip(&level_counts).try_fold(
        0_usize,
        |total, (&channels, &levels)| {
            total
                .checked_add(channels.checked_mul(levels).ok_or_else(|| {
                    allocation("categorical recovery LSMR coefficient width overflows")
                })?)
                .ok_or_else(|| allocation("categorical recovery LSMR coefficient count overflows"))
        },
    )?;
    let preconditioner =
        recovery_preconditioner(config.preconditioner, effects.len(), coefficient_count);
    let weight_values = match weights {
        BorrowedMapWeights::Unit => None,
        BorrowedMapWeights::Provided(values) => Some(values),
    };
    let initial = within::solve(
        within_effects.clone(),
        target,
        weight_values,
        Some(&options),
        preconditioner,
    );
    let solved = match initial {
        Ok(solved) => solved,
        Err(error)
            if config.preconditioner == LsmrPreconditioner::Auto
                && recovery_preconditioner_breakdown(&error) =>
        {
            within::solve(
                within_effects,
                target,
                weight_values,
                Some(&options),
                PreconditionerConfig::Off,
            )
            .map_err(|fallback_error| {
                invalid_data(format!(
                    "categorical recovery LSMR failed after unpreconditioned fallback: {fallback_error}"
                ))
            })?
        }
        Err(error) => {
            return Err(invalid_data(format!(
                "categorical recovery LSMR failed: {error}"
            )))
        }
    };
    if !solved.converged {
        return Err(invalid_data(format!(
            "categorical recovery LSMR exhausted its iteration budget after {} iterations",
            solved.iterations
        )));
    }
    let residual = DenseMatrix::from_row_major(target.len(), 1, solved.demeaned)?;
    let validated_weights = validate_weights(weights, target.len())?;
    let mut reservation = SystemSlopeWorkspaceReservation;
    let mut workspace = SlopeWorkspace::new(
        effects
            .iter()
            .map(CategoricalAbsorbedEffectView::channels)
            .max()
            .unwrap_or(1),
        1,
        &mut reservation,
    )?;
    weighted_norms(
        &DenseMatrix::from_row_major(target.len(), 1, target.to_vec())?,
        validated_weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.coefficients[..1],
    )?;
    let certificate = certify(
        topology,
        effects,
        validated_weights,
        &residual,
        publication_tolerance,
        &mut workspace,
    )?;
    if !certificate.passed {
        return Err(invalid_data(
            "categorical recovery LSMR returned an uncertified residual",
        ));
    }
    let coefficients = FeCoefficientBatch::from_values(effect_channels, level_counts, 1, solved.x)?;
    let mut normalized = coefficients.rhs(0)?.to_vec();
    normalize_intercept_components(topology, effects, validated_weights, &mut normalized)?;
    finish_channel_major_recovery(
        topology,
        effects,
        validated_weights,
        target,
        requested_channels,
        &normalized,
        solved.iterations,
        publication_tolerance,
        CategoricalRecoveryMethod::Lsmr,
    )
}

/// Recover intercept-only effects through the production SD-Kaczmarz pilot
/// and remainder-LSMR hybrid. Component normalization and publication checks
/// remain topology-owned and are recomputed from the combined coefficients.
pub(crate) fn recover_finalized_categorical_effects_sd_lsmr_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested_channels: &[bool],
    lsmr: LsmrConfig,
    map: MapConfig,
) -> Result<RecoveredCategoricalEffects> {
    validate_recovery_inputs(topology, effects, target, requested_channels)?;
    if effects
        .iter()
        .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
    {
        return Err(invalid_argument(
            "SD-Kaczmarz fast recovery requires intercept-only effects",
        ));
    }
    let source_table = topology_source_table(topology)?;
    let design = borrow_categorical_topology_design(
        target.len(),
        topology.registry(),
        &source_table,
        topology.effect_to_topology(),
    )?;
    let recovered = recover_borrowed_intercept_sd_lsmr_coefficients(
        &design,
        weights,
        target,
        map,
        lsmr,
        recovery_reconstruction_threshold(map.tolerance),
    )?;
    let validated_weights = validate_weights(weights, target.len())?;
    let mut normalized = recovered.coefficients.rhs(0)?.to_vec();
    normalize_intercept_components(topology, effects, validated_weights, &mut normalized)?;
    let method = match recovered.method {
        BorrowedInterceptRecoveryMethod::Direct => CategoricalRecoveryMethod::Direct,
        BorrowedInterceptRecoveryMethod::SteepestDescent => {
            CategoricalRecoveryMethod::SteepestDescent
        }
        BorrowedInterceptRecoveryMethod::HybridLsmr => CategoricalRecoveryMethod::HybridLsmr,
    };
    finish_channel_major_recovery(
        topology,
        effects,
        validated_weights,
        target,
        requested_channels,
        &normalized,
        recovered.iterations,
        map.tolerance,
        method,
    )
}

/// Recover intercept-only effects with the optimized borrowed one-RHS CG
/// recurrence and retain the coefficient product it already computes.
pub(crate) fn recover_finalized_categorical_effects_cg_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested_channels: &[bool],
    map: MapConfig,
) -> Result<RecoveredCategoricalEffects> {
    validate_recovery_inputs(topology, effects, target, requested_channels)?;
    if effects
        .iter()
        .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
    {
        return Err(invalid_argument(
            "CG fast recovery requires intercept-only effects",
        ));
    }
    let source_table = topology_source_table(topology)?;
    let design = borrow_categorical_topology_design(
        target.len(),
        topology.registry(),
        &source_table,
        topology.effect_to_topology(),
    )?;
    let mut target_values = Vec::new();
    target_values
        .try_reserve_exact(target.len())
        .map_err(|_| allocation("unable to reserve CG recovery target"))?;
    target_values.extend_from_slice(target);
    let recovered = absorb_borrowed_intercept_cg_recovery_certified(
        &design,
        weights,
        DenseMatrix::from_row_major(target.len(), 1, target_values)?,
        map,
        recovery_reconstruction_threshold(map.tolerance),
    )?;
    let (_, report, coefficients) = recovered.into_full_parts();
    let coefficients = coefficients.ok_or_else(|| {
        invalid_data("CG recovery did not retain its certified coefficient product")
    })?;
    let validated_weights = validate_weights(weights, target.len())?;
    let mut normalized = coefficients.rhs(0)?.to_vec();
    normalize_intercept_components(topology, effects, validated_weights, &mut normalized)?;
    finish_channel_major_recovery(
        topology,
        effects,
        validated_weights,
        target,
        requested_channels,
        &normalized,
        report.iterations,
        map.tolerance,
        CategoricalRecoveryMethod::Cg,
    )
}

fn validate_recovery_inputs(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    target: &[f64],
    requested_channels: &[bool],
) -> Result<()> {
    topology.validate()?;
    if target.len() != topology.retained_rows().len()
        || effects.len() != topology.effect_to_topology().len()
        || target.is_empty()
        || target.iter().any(|value| !value.is_finite())
    {
        return Err(invalid_argument(
            "categorical recovery inputs are not finite and aligned",
        ));
    }
    let channels = effects.iter().try_fold(0_usize, |total, effect| {
        total
            .checked_add(effect.channels())
            .ok_or_else(|| allocation("categorical recovery channel count overflows"))
    })?;
    if channels == 0 || requested_channels.len() != channels {
        return Err(invalid_argument(
            "categorical recovery requested-channel layout differs",
        ));
    }
    Ok(())
}

fn validate_coefficient_layout(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    coefficients: &FeCoefficientBatch,
) -> Result<()> {
    let channels = effects
        .iter()
        .map(CategoricalAbsorbedEffectView::channels)
        .collect::<Vec<_>>();
    let levels = topology
        .effect_to_topology()
        .iter()
        .map(|source| {
            topology
                .registry()
                .source(*source)
                .map(|source| source.n_levels())
        })
        .collect::<Result<Vec<_>>>()?;
    if coefficients.effect_channels() != channels || coefficients.level_counts() != levels {
        return Err(invalid_argument(
            "categorical recovery coefficient layout differs from the finalized effects",
        ));
    }
    Ok(())
}

fn topology_source_table(
    topology: &FinalizedCategoricalTopology,
) -> Result<Vec<super::CategoricalCodeView<'_>>> {
    let mut source_table = Vec::new();
    source_table
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| allocation("unable to reserve categorical recovery source views"))?;
    for source in topology.compacted_sources() {
        source_table.push(source.view());
    }
    Ok(source_table)
}

fn normalize_intercept_components(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: ValidatedWeights<'_>,
    coefficients: &mut [f64],
) -> Result<()> {
    let intercept_effects = effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| effect.has_intercept().then_some(index))
        .collect::<Vec<_>>();
    if intercept_effects.len() <= 1 {
        return Ok(());
    }
    let source_table = topology_source_table(topology)?;
    let mut codes = Vec::with_capacity(intercept_effects.len());
    let mut levels = Vec::with_capacity(intercept_effects.len());
    for &effect_index in &intercept_effects {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?;
        codes.push(source.codes(&source_table)?);
        levels.push(source.n_levels());
    }
    let components = connected_components_all(&codes, &levels)?;
    let mut component_weights = vec![0.0_f64; components.n_components];
    let anchor_components = &components.component_by_effect_level[0];
    for (level, &component) in anchor_components.iter().enumerate() {
        if component == usize::MAX {
            continue;
        }
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[intercept_effects[0]])?;
        let members = source.members(level)?;
        if weights.values.is_none() {
            component_weights[component] += members.len() as f64;
        } else {
            for &row in members {
                component_weights[component] += weights.value(row);
            }
        }
    }
    let effect_offsets = channel_major_effect_offsets(topology, effects)?;
    let mut anchor_adjustments = vec![0.0_f64; components.n_components];
    for (local_effect, &effect_index) in intercept_effects.iter().enumerate().skip(1) {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?;
        let mut numerators = vec![0.0_f64; components.n_components];
        for level in 0..source.n_levels() {
            let component = components.component_by_effect_level[local_effect][level];
            if component == usize::MAX {
                continue;
            }
            let coefficient = coefficients[effect_offsets[effect_index] + level];
            let members = source.members(level)?;
            if weights.values.is_none() {
                numerators[component] += members.len() as f64 * coefficient;
            } else {
                for &row in members {
                    numerators[component] += weights.value(row) * coefficient;
                }
            }
        }
        let means = numerators
            .iter()
            .zip(&component_weights)
            .map(|(&numerator, &weight)| {
                if weight > 0.0 {
                    numerator / weight
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>();
        for level in 0..source.n_levels() {
            let component = components.component_by_effect_level[local_effect][level];
            if component != usize::MAX {
                coefficients[effect_offsets[effect_index] + level] -= means[component];
            }
        }
        for (adjustment, mean) in anchor_adjustments.iter_mut().zip(means) {
            *adjustment += mean;
        }
    }
    let anchor_effect = intercept_effects[0];
    let anchor_source = topology
        .registry()
        .source(topology.effect_to_topology()[anchor_effect])?;
    for level in 0..anchor_source.n_levels() {
        let component = components.component_by_effect_level[0][level];
        if component != usize::MAX {
            coefficients[effect_offsets[anchor_effect] + level] += anchor_adjustments[component];
        }
    }
    if coefficients.iter().any(|value| !value.is_finite()) {
        return Err(invalid_data(
            "categorical recovery normalization produced a nonfinite coefficient",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_channel_major_recovery(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: ValidatedWeights<'_>,
    target: &[f64],
    requested_channels: &[bool],
    coefficients: &[f64],
    iterations: usize,
    tolerance: f64,
    recovery_method: CategoricalRecoveryMethod,
) -> Result<RecoveredCategoricalEffects> {
    let effect_offsets = channel_major_effect_offsets(topology, effects)?;
    if coefficients.len() != effect_offsets.last().copied().unwrap_or(0) {
        return Err(invalid_argument(
            "categorical recovery coefficient count is inconsistent",
        ));
    }
    let mut absorbed_sum = vec![0.0_f64; target.len()];
    let mut channels = Vec::new();
    channels
        .try_reserve_exact(requested_channels.len())
        .map_err(|_| allocation("unable to reserve recovered channel registry"))?;
    let mut flat_channel = 0_usize;
    for (effect_index, effect) in effects.iter().enumerate() {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?;
        for channel in 0..effect.channels() {
            let mut contribution =
                requested_channels[flat_channel].then(|| vec![0.0_f64; target.len()]);
            for level in 0..source.n_levels() {
                let alpha = coefficients
                    [effect_offsets[effect_index] + channel * source.n_levels() + level];
                for &row in source.members(level)? {
                    let basis = if effect.has_intercept() && channel == 0 {
                        1.0
                    } else {
                        let slope_index = channel - usize::from(effect.has_intercept());
                        effect.slopes()[slope_index][row]
                    };
                    let value = alpha * basis;
                    absorbed_sum[row] += value;
                    if let Some(output) = contribution.as_mut() {
                        output[row] = value;
                    }
                }
            }
            channels.push(contribution);
            flat_channel += 1;
        }
    }
    let mut reservation = SystemSlopeWorkspaceReservation;
    let mut workspace = SlopeWorkspace::new(
        effects
            .iter()
            .map(CategoricalAbsorbedEffectView::channels)
            .max()
            .unwrap_or(1),
        1,
        &mut reservation,
    )?;
    workspace.value_scales[0] = 0.0;
    workspace.coefficients[0] = 1.0;
    let mut target_infinity = 0.0_f64;
    let mut residual_infinity = 0.0_f64;
    let mut residual_values = Vec::new();
    residual_values
        .try_reserve_exact(target.len())
        .map_err(|_| allocation("unable to reserve categorical recovery residual"))?;
    for (row, (&expected, &actual)) in target.iter().zip(&absorbed_sum).enumerate() {
        let remainder = expected - actual;
        if !remainder.is_finite() {
            return Err(invalid_data(
                "categorical recovery coefficient product produced a nonfinite residual",
            ));
        }
        residual_values.push(remainder);
        target_infinity = target_infinity.max(expected.abs());
        residual_infinity = residual_infinity.max(remainder.abs());
        let weighted_target = weights.value(row).sqrt() * expected;
        if !weighted_target.is_finite() {
            return Err(invalid_data(
                "categorical recovery weighted target is nonfinite",
            ));
        }
        update_scaled_sum(
            weighted_target,
            &mut workspace.value_scales[0],
            &mut workspace.coefficients[0],
        );
    }
    workspace.reference_norms[0] = if workspace.value_scales[0] == 0.0 {
        0.0
    } else {
        workspace.value_scales[0] * workspace.coefficients[0].sqrt()
    };
    if !workspace.reference_norms[0].is_finite() {
        return Err(invalid_data(
            "categorical recovery weighted target norm is nonfinite",
        ));
    }
    let residual = DenseMatrix::from_row_major(target.len(), 1, residual_values)?;
    let reconstruction_relative_infinity = if target_infinity == 0.0 {
        residual_infinity
    } else {
        residual_infinity / target_infinity
    };
    if !reconstruction_relative_infinity.is_finite() {
        return Err(invalid_data(
            "categorical recovery reconstruction ratio is not finite",
        ));
    }
    if reconstruction_relative_infinity > recovery_reconstruction_threshold(tolerance) {
        return Err(invalid_data(format!(
            "categorical recovery coefficient product failed reconstruction: relative infinity residual {reconstruction_relative_infinity:.6e}"
        )));
    }
    let certificate = certify(
        topology,
        effects,
        weights,
        &residual,
        tolerance,
        &mut workspace,
    )?;
    if !certificate.passed {
        return Err(invalid_data(
            "categorical recovery coefficient product failed its FE certificate",
        ));
    }
    Ok(RecoveredCategoricalEffects {
        absorbed_sum,
        channels,
        iterations,
        reconstruction_relative_infinity,
        recovery_method,
    })
}

fn channel_major_effect_offsets(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
) -> Result<Vec<usize>> {
    let mut offsets = Vec::with_capacity(effects.len() + 1);
    offsets.push(0);
    for (effect_index, effect) in effects.iter().enumerate() {
        let levels = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?
            .n_levels();
        let width = levels
            .checked_mul(effect.channels())
            .ok_or_else(|| allocation("categorical recovery coefficient width overflows"))?;
        offsets.push(
            offsets
                .last()
                .copied()
                .unwrap_or(0_usize)
                .checked_add(width)
                .ok_or_else(|| allocation("categorical recovery coefficient offset overflows"))?,
        );
    }
    Ok(offsets)
}

fn recovery_preconditioner(
    requested: LsmrPreconditioner,
    effects: usize,
    coefficients: usize,
) -> PreconditionerConfig {
    match requested {
        LsmrPreconditioner::Off => PreconditionerConfig::Off,
        LsmrPreconditioner::Diagonal => PreconditionerConfig::Diagonal,
        LsmrPreconditioner::Schwarz => PreconditionerConfig::default(),
        LsmrPreconditioner::Auto if effects >= 2 && coefficients >= 256 => {
            PreconditionerConfig::default()
        }
        LsmrPreconditioner::Auto => PreconditionerConfig::Diagonal,
    }
}

fn recovery_preconditioner_breakdown(error: &within::WithinError) -> bool {
    matches!(
        error,
        within::WithinError::Solve(within::SolveError::InvalidInput { context, message })
            if *context == "mlsmr" && message.contains("preconditioner not positive definite")
    )
}

fn recover_with_reservation<R: SlopeWorkspaceReservation>(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested_channels: &[bool],
    config: MapConfig,
    reservation: &mut R,
) -> Result<RecoveredCategoricalEffects> {
    validate_config(config)?;
    topology.validate()?;
    let nobs = topology.retained_rows().len();
    if target.len() != nobs || effects.len() != topology.effect_to_topology().len() {
        return Err(invalid_argument(
            "categorical effect recovery topology, basis, and target dimensions differ",
        ));
    }
    if target.is_empty() || target.iter().any(|value| !value.is_finite()) {
        return Err(invalid_data(
            "categorical effect recovery requires a nonempty finite target",
        ));
    }
    let channel_count = effects.iter().try_fold(0_usize, |total, effect| {
        total
            .checked_add(effect.channels())
            .ok_or_else(|| allocation("categorical recovery channel count overflows"))
    })?;
    if requested_channels.len() != channel_count || channel_count == 0 {
        return Err(invalid_argument(
            "categorical effect recovery requested-channel layout differs",
        ));
    }
    let mut effect_offsets = Vec::new();
    reservation.reserve_usize(
        &mut effect_offsets,
        effects.len() + 1,
        "unable to reserve categorical recovery effect offsets",
    )?;
    if effect_offsets.capacity() < effects.len() + 1 {
        return Err(allocation(
            "categorical recovery effect-offset reservation was undersized",
        ));
    }
    effect_offsets.push(0);
    let mut coefficient_count = 0_usize;
    let mut max_channels = 0_usize;
    for (effect_index, effect) in effects.iter().enumerate() {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?;
        max_channels = max_channels.max(effect.channels());
        coefficient_count = coefficient_count
            .checked_add(
                source
                    .n_levels()
                    .checked_mul(effect.channels())
                    .ok_or_else(|| {
                        allocation("categorical recovery coefficient block overflows")
                    })?,
            )
            .ok_or_else(|| allocation("categorical recovery coefficient count overflows"))?;
        effect_offsets.push(coefficient_count);
    }
    let mut alphas = reserved_zeros(
        coefficient_count,
        reservation,
        "unable to reserve categorical recovery coefficients",
    )?;
    let mut residual_values = Vec::new();
    reservation.reserve_f64(
        &mut residual_values,
        nobs,
        "unable to reserve categorical recovery residual",
    )?;
    if residual_values.capacity() < nobs {
        return Err(allocation(
            "categorical recovery residual reservation was undersized",
        ));
    }
    residual_values.extend_from_slice(target);
    let mut residual = DenseMatrix::from_row_major(nobs, 1, residual_values)?;
    let weights = validate_weights(weights, nobs)?;
    let mut workspace = SlopeWorkspace::new(max_channels, 1, reservation)?;
    weighted_norms(
        &residual,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.coefficients[..1],
    )?;

    let initial = certify(
        topology,
        effects,
        weights,
        &residual,
        config.tolerance,
        &mut workspace,
    )?;
    // A small FE moment only shows that the remainder is nearly orthogonal to
    // each effect. Weak graphs can reach that condition before the remainder
    // is small enough to certify that the target lies in the absorbed span.
    if initial.passed && recovery_reconstruction_passed(target, &residual, config.tolerance)? {
        return finish_recovery(
            topology,
            effects,
            target,
            requested_channels,
            &effect_offsets,
            &alphas,
            &residual,
            0,
            config.tolerance,
            reservation,
        );
    }

    for iteration in 1..=config.max_iterations {
        for (effect_index, effect) in effects.iter().enumerate() {
            project_effect_and_accumulate(
                topology,
                effect_index,
                effect,
                weights,
                &mut residual,
                &mut workspace,
                &mut alphas,
                effect_offsets[effect_index],
            )?;
        }
        if config.symmetric && effects.len() > 1 {
            for effect_index in (0..effects.len() - 1).rev() {
                project_effect_and_accumulate(
                    topology,
                    effect_index,
                    &effects[effect_index],
                    weights,
                    &mut residual,
                    &mut workspace,
                    &mut alphas,
                    effect_offsets[effect_index],
                )?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            let certificate = certify(
                topology,
                effects,
                weights,
                &residual,
                config.tolerance,
                &mut workspace,
            )?;
            if certificate.passed
                && recovery_reconstruction_passed(target, &residual, config.tolerance)?
            {
                return finish_recovery(
                    topology,
                    effects,
                    target,
                    requested_channels,
                    &effect_offsets,
                    &alphas,
                    &residual,
                    iteration,
                    config.tolerance,
                    reservation,
                );
            }
            if certificate.passed && iteration == config.max_iterations {
                return finish_recovery(
                    topology,
                    effects,
                    target,
                    requested_channels,
                    &effect_offsets,
                    &alphas,
                    &residual,
                    iteration,
                    config.tolerance,
                    reservation,
                );
            }
        }
    }
    Err(invalid_data(
        "categorical effect recovery exhausted its iteration budget without certification",
    ))
}

#[allow(clippy::too_many_arguments)]
fn project_effect_and_accumulate(
    topology: &FinalizedCategoricalTopology,
    effect_index: usize,
    effect: &CategoricalAbsorbedEffectView<'_>,
    weights: ValidatedWeights<'_>,
    residual: &mut DenseMatrix,
    workspace: &mut SlopeWorkspace,
    alphas: &mut [f64],
    effect_offset: usize,
) -> Result<()> {
    let source = topology.registry().source(
        *topology
            .effect_to_topology()
            .get(effect_index)
            .ok_or_else(|| invalid_argument("categorical recovery effect index is out of range"))?,
    )?;
    let channels = effect.channels();
    for level in 0..source.n_levels() {
        let members = source.members(level)?;
        group_moments(members, effect, weights, residual, workspace)?;
        solve_group(channels, 1, workspace)?;
        let level_offset = effect_offset
            .checked_add(
                level
                    .checked_mul(channels)
                    .ok_or_else(|| allocation("categorical recovery level offset overflows"))?,
            )
            .ok_or_else(|| allocation("categorical recovery coefficient offset overflows"))?;
        for channel in 0..channels {
            let raw = workspace.coefficients[channel] / workspace.channel_scales[channel];
            let destination = alphas.get_mut(level_offset + channel).ok_or_else(|| {
                invalid_argument("categorical recovery coefficient is out of range")
            })?;
            *destination += raw;
            if !destination.is_finite() {
                return Err(invalid_data(
                    "categorical recovery coefficient accumulation is not finite",
                ));
            }
        }
        for &row in members {
            let mut fitted = 0.0_f64;
            for channel in 0..channels {
                fitted = effect
                    .basis_value(channel, row, &workspace.channel_scales)
                    .mul_add(workspace.coefficients[channel], fitted);
            }
            residual[(row, 0)] -= fitted;
            if !residual[(row, 0)].is_finite() {
                return Err(invalid_data(
                    "categorical recovery projection produced a nonfinite residual",
                ));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_recovery<R: SlopeWorkspaceReservation>(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    target: &[f64],
    requested_channels: &[bool],
    effect_offsets: &[usize],
    alphas: &[f64],
    residual: &DenseMatrix,
    iterations: usize,
    tolerance: f64,
    reservation: &mut R,
) -> Result<RecoveredCategoricalEffects> {
    let reconstruction_relative_infinity =
        recovery_reconstruction_relative_infinity(target, residual)?;
    if reconstruction_relative_infinity > recovery_reconstruction_threshold(tolerance) {
        return Err(invalid_data(format!(
            "categorical recovery target is not certified to lie in the absorbed span: relative infinity residual {reconstruction_relative_infinity:.6e}",
        )));
    }

    let mut absorbed_sum = reserved_zeros(
        target.len(),
        reservation,
        "unable to reserve recovered absorbed sum",
    )?;
    let mut channels = Vec::new();
    channels
        .try_reserve_exact(requested_channels.len())
        .map_err(|_| allocation("unable to reserve recovered channel registry"))?;
    let mut flat_channel = 0_usize;
    for (effect_index, effect) in effects.iter().enumerate() {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?;
        let channel_count = effect.channels();
        for channel in 0..channel_count {
            let requested = requested_channels[flat_channel];
            let mut contribution = if requested {
                Some(reserved_zeros(
                    target.len(),
                    reservation,
                    "unable to reserve recovered channel contribution",
                )?)
            } else {
                None
            };
            for level in 0..source.n_levels() {
                let alpha_index = effect_offsets[effect_index] + level * channel_count + channel;
                let alpha = *alphas.get(alpha_index).ok_or_else(|| {
                    invalid_argument("categorical recovery alpha index is out of range")
                })?;
                for &row in source.members(level)? {
                    let basis = if effect.intercept && channel == 0 {
                        1.0
                    } else {
                        let slope_index = channel - usize::from(effect.intercept);
                        effect.slopes[slope_index][row]
                    };
                    let value = alpha * basis;
                    absorbed_sum[row] += value;
                    if let Some(values) = contribution.as_mut() {
                        values[row] = value;
                    }
                }
            }
            channels.push(contribution);
            flat_channel += 1;
        }
    }
    for ((recovered, expected), remainder) in
        absorbed_sum.iter().zip(target).zip(residual.as_slice())
    {
        let error = (*recovered + *remainder - *expected).abs();
        if !error.is_finite() || error > (tolerance * expected.abs().max(1.0) * 256.0).max(1e-10) {
            return Err(invalid_data(
                "categorical recovery decomposition failed its reconstruction identity",
            ));
        }
    }
    Ok(RecoveredCategoricalEffects {
        absorbed_sum,
        channels,
        iterations,
        reconstruction_relative_infinity,
        recovery_method: CategoricalRecoveryMethod::Map,
    })
}

fn recovery_reconstruction_passed(
    target: &[f64],
    residual: &DenseMatrix,
    tolerance: f64,
) -> Result<bool> {
    Ok(recovery_reconstruction_relative_infinity(target, residual)?
        <= recovery_reconstruction_threshold(tolerance))
}

fn recovery_reconstruction_relative_infinity(
    target: &[f64],
    residual: &DenseMatrix,
) -> Result<f64> {
    if residual.ncols() != 1 || residual.nrows() != target.len() {
        return Err(invalid_argument(
            "categorical recovery reconstruction dimensions differ",
        ));
    }
    let scale = target
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let residual_max = residual
        .as_slice()
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let relative = if scale == 0.0 {
        residual_max
    } else {
        residual_max / scale
    };
    if !relative.is_finite() {
        return Err(invalid_data(
            "categorical recovery reconstruction ratio is not finite",
        ));
    }
    Ok(relative)
}

fn recovery_reconstruction_threshold(tolerance: f64) -> f64 {
    (tolerance * 100.0).max(1e-10)
}

fn absorb_with_reservation<R: SlopeWorkspaceReservation>(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    topology.validate()?;
    let nobs = topology.retained_rows().len();
    if values.nrows() != nobs || effects.len() != topology.effect_to_topology().len() {
        return Err(invalid_argument(
            "categorical slope MAP topology, basis, and value dimensions differ",
        ));
    }
    if nobs == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "categorical slope MAP requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("categorical slope MAP values must be finite"));
    }
    let mut max_channels = 0_usize;
    for effect in effects {
        if !effect.intercept && effect.slopes.is_empty() {
            return Err(invalid_argument(
                "categorical slope MAP effect has no basis channels",
            ));
        }
        max_channels = max_channels.max(effect.channels());
        for slope in &effect.slopes {
            if slope.len() != nobs || slope.iter().any(|value| !value.is_finite()) {
                return Err(invalid_data(
                    "categorical slope MAP values are not finite and aligned",
                ));
            }
        }
    }
    if max_channels == 0 {
        return Err(invalid_argument(
            "categorical slope MAP request has no basis channels",
        ));
    }
    for effect in effects {
        if effect.channels() > nobs {
            return Err(invalid_data(
                "categorical slope MAP effect has more channels than observations",
            ));
        }
    }
    let weights = validate_weights(weights, nobs)?;

    let setup_started = Instant::now();
    let mut workspace = SlopeWorkspace::new(max_channels, values.ncols(), reservation)?;
    weighted_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.coefficients[..values.ncols()],
    )?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();

    let initial = certify(
        topology,
        effects,
        weights,
        &values,
        config.tolerance,
        &mut workspace,
    )?;
    if initial.passed {
        return finish(
            values,
            0,
            initial,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    for iteration in 1..=config.max_iterations {
        for (effect_index, effect) in effects.iter().enumerate() {
            project_effect(
                topology,
                effect_index,
                effect,
                weights,
                &mut values,
                &mut workspace,
            )?;
        }
        if config.symmetric && effects.len() > 1 {
            for effect_index in (0..effects.len() - 1).rev() {
                project_effect(
                    topology,
                    effect_index,
                    &effects[effect_index],
                    weights,
                    &mut values,
                    &mut workspace,
                )?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            let certificate = certify(
                topology,
                effects,
                weights,
                &values,
                config.tolerance,
                &mut workspace,
            )?;
            if certificate.passed {
                return finish(
                    values,
                    iteration,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                );
            }
        }
    }
    Err(invalid_data(
        "intercept-slope MAP exhausted its iteration budget without certification",
    ))
}

fn project_effect(
    topology: &FinalizedCategoricalTopology,
    effect_index: usize,
    effect: &CategoricalAbsorbedEffectView<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut SlopeWorkspace,
) -> Result<()> {
    let source = topology.registry().source(
        *topology
            .effect_to_topology()
            .get(effect_index)
            .ok_or_else(|| {
                invalid_argument("categorical slope MAP effect index is out of range")
            })?,
    )?;
    for level in 0..source.n_levels() {
        let members = source.members(level)?;
        group_moments(members, effect, weights, values, workspace)?;
        solve_group(effect.channels(), values.ncols(), workspace)?;
        for &row in members {
            for rhs in 0..values.ncols() {
                let mut fitted = 0.0_f64;
                for channel in 0..effect.channels() {
                    fitted = effect
                        .basis_value(channel, row, &workspace.channel_scales)
                        .mul_add(
                            workspace.coefficients[channel * values.ncols() + rhs],
                            fitted,
                        );
                }
                let residual = values[(row, rhs)] - fitted;
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "categorical slope MAP projection produced a nonfinite residual",
                    ));
                }
                values[(row, rhs)] = residual;
            }
        }
    }
    Ok(())
}

fn group_moments(
    members: &[usize],
    effect: &CategoricalAbsorbedEffectView<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut SlopeWorkspace,
) -> Result<()> {
    if members.is_empty() {
        return Err(invalid_argument(
            "categorical slope MAP contains an empty final level",
        ));
    }
    let channels = effect.channels();
    let rhs_count = values.ncols();
    let gram_elements = channels * channels;
    let cross_elements = channels * rhs_count;
    workspace.gram[..gram_elements].fill(0.0);
    workspace.gram_corrections[..gram_elements].fill(0.0);
    workspace.cross[..cross_elements].fill(0.0);
    workspace.cross_corrections[..cross_elements].fill(0.0);
    workspace.value_scales.fill(0.0);
    workspace.channel_scales[..channels].fill(0.0);
    let slope_offset = usize::from(effect.intercept);
    if effect.intercept {
        workspace.channel_scales[0] = 1.0;
    }
    for (slope_index, slope) in effect.slopes.iter().enumerate() {
        let channel = slope_offset + slope_index;
        let scale = members
            .iter()
            .map(|row| slope[*row].abs())
            .fold(0.0_f64, f64::max);
        // A zero group-specific slope contributes no local span. Keeping unit
        // scale lets the rank-revealing solve drop it without division by zero.
        workspace.channel_scales[channel] = if scale == 0.0 { 1.0 } else { scale };
    }
    for &row in members {
        if row >= values.nrows() {
            return Err(invalid_argument(
                "categorical slope MAP membership exceeds the observation axis",
            ));
        }
        for rhs in 0..values.ncols() {
            workspace.value_scales[rhs] = workspace.value_scales[rhs].max(values[(row, rhs)].abs());
        }
    }
    for &row in members {
        let weight = weights.value(row);
        for left in 0..channels {
            let left_basis = effect.basis_value(left, row, &workspace.channel_scales);
            for right in 0..channels {
                compensated_add(
                    weight * left_basis * effect.basis_value(right, row, &workspace.channel_scales),
                    &mut workspace.gram[left * channels + right],
                    &mut workspace.gram_corrections[left * channels + right],
                )?;
            }
            for rhs in 0..rhs_count {
                let scale = workspace.value_scales[rhs];
                if scale != 0.0 {
                    compensated_add(
                        weight * left_basis * (values[(row, rhs)] / scale),
                        &mut workspace.cross[left * rhs_count + rhs],
                        &mut workspace.cross_corrections[left * rhs_count + rhs],
                    )?;
                }
            }
        }
    }
    for index in 0..gram_elements {
        workspace.gram[index] += workspace.gram_corrections[index];
    }
    for index in 0..cross_elements {
        workspace.cross[index] += workspace.cross_corrections[index];
    }
    Ok(())
}

fn solve_group(channels: usize, rhs_count: usize, workspace: &mut SlopeWorkspace) -> Result<usize> {
    if channels == 0 || channels > workspace.max_channels {
        return Err(invalid_argument(
            "categorical slope MAP channel count exceeds its workspace",
        ));
    }
    let gram_elements = channels * channels;
    let cross_elements = channels * rhs_count;
    workspace.factor[..gram_elements].copy_from_slice(&workspace.gram[..gram_elements]);
    workspace.factor_cross[..cross_elements].copy_from_slice(&workspace.cross[..cross_elements]);
    workspace.coefficients[..cross_elements].fill(0.0);
    for (index, destination) in workspace.permutation[..channels].iter_mut().enumerate() {
        *destination = index;
    }
    let diagonal_scale = (0..channels)
        .map(|index| workspace.factor[index * channels + index].abs())
        .fold(0.0_f64, f64::max);
    if !diagonal_scale.is_finite() {
        return Err(invalid_data(
            "categorical slope MAP Gram scale is not finite",
        ));
    }
    let tolerance = f64::EPSILON * diagonal_scale.max(1.0) * (channels.max(1) as f64) * 128.0;
    let mut rank = 0_usize;
    for pivot_position in 0..channels {
        let mut pivot_index = pivot_position;
        let mut pivot_value = workspace.factor[pivot_position * channels + pivot_position].abs();
        for candidate in pivot_position + 1..channels {
            let value = workspace.factor[candidate * channels + candidate].abs();
            if value > pivot_value {
                pivot_index = candidate;
                pivot_value = value;
            }
        }
        if pivot_value <= tolerance {
            break;
        }
        if pivot_index != pivot_position {
            swap_symmetric(&mut workspace.factor, channels, pivot_position, pivot_index);
            swap_rhs_rows(
                &mut workspace.factor_cross,
                rhs_count,
                pivot_position,
                pivot_index,
            );
            workspace.permutation.swap(pivot_position, pivot_index);
        }
        let pivot = workspace.factor[pivot_position * channels + pivot_position];
        if !pivot.is_finite() || pivot.abs() <= tolerance {
            break;
        }
        for row in pivot_position + 1..channels {
            let multiplier = workspace.factor[row * channels + pivot_position] / pivot;
            if !multiplier.is_finite() {
                return Err(invalid_data(
                    "categorical slope MAP elimination multiplier is not finite",
                ));
            }
            workspace.factor[row * channels + pivot_position] = multiplier;
            for column in pivot_position + 1..channels {
                let updated = workspace.factor[row * channels + column]
                    - multiplier * workspace.factor[pivot_position * channels + column];
                if !updated.is_finite() {
                    return Err(invalid_data(
                        "categorical slope MAP factorization is not finite",
                    ));
                }
                workspace.factor[row * channels + column] = updated;
            }
            for rhs in 0..rhs_count {
                let updated = workspace.factor_cross[row * rhs_count + rhs]
                    - multiplier * workspace.factor_cross[pivot_position * rhs_count + rhs];
                if !updated.is_finite() {
                    return Err(invalid_data(
                        "categorical slope MAP transformed cross-product is not finite",
                    ));
                }
                workspace.factor_cross[row * rhs_count + rhs] = updated;
            }
        }
        rank += 1;
    }
    for rhs in 0..rhs_count {
        for row in (0..rank).rev() {
            let mut value = workspace.factor_cross[row * rhs_count + rhs];
            for column in row + 1..rank {
                value -= workspace.factor[row * channels + column]
                    * workspace.factor_cross[column * rhs_count + rhs];
            }
            let pivot = workspace.factor[row * channels + row];
            let solution = value / pivot;
            if !solution.is_finite() {
                return Err(invalid_data(
                    "categorical slope MAP group solution is not finite",
                ));
            }
            workspace.factor_cross[row * rhs_count + rhs] = solution;
        }
        for position in 0..rank {
            let channel = workspace.permutation[position];
            workspace.coefficients[channel * rhs_count + rhs] =
                workspace.value_scales[rhs] * workspace.factor_cross[position * rhs_count + rhs];
        }
    }
    Ok(rank)
}

fn swap_symmetric(matrix: &mut [f64], dimension: usize, left: usize, right: usize) {
    for column in 0..dimension {
        matrix.swap(left * dimension + column, right * dimension + column);
    }
    for row in 0..dimension {
        matrix.swap(row * dimension + left, row * dimension + right);
    }
}

fn swap_rhs_rows(values: &mut [f64], rhs_count: usize, left: usize, right: usize) {
    for rhs in 0..rhs_count {
        values.swap(left * rhs_count + rhs, right * rhs_count + rhs);
    }
}

fn certify(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut SlopeWorkspace,
) -> Result<FeCertificate> {
    weighted_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.coefficients[..residuals.ncols()],
    )?;
    for rhs in 0..residuals.ncols() {
        workspace.rhs_scales[rhs] =
            workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
    }
    let mut absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_squares = 0.0_f64;
    let mut count = 0_usize;
    for (effect_index, effect) in effects.iter().enumerate() {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect_index])?;
        for level in 0..source.n_levels() {
            group_moments(
                source.members(level)?,
                effect,
                weights,
                residuals,
                workspace,
            )?;
            solve_group(effect.channels(), residuals.ncols(), workspace)?;
            let channels = effect.channels();
            for rhs in 0..residuals.ncols() {
                let mut projected_square = 0.0_f64;
                for left in 0..channels {
                    for right in 0..channels {
                        projected_square = workspace.coefficients[left * residuals.ncols() + rhs]
                            .mul_add(
                                workspace.gram[left * channels + right]
                                    * workspace.coefficients[right * residuals.ncols() + rhs],
                                projected_square,
                            );
                    }
                }
                let projected = projected_square.max(0.0).sqrt();
                let denominator = workspace.rhs_scales[rhs];
                let relative = if denominator == 0.0 {
                    if projected == 0.0 {
                        0.0
                    } else {
                        f64::INFINITY
                    }
                } else {
                    projected / denominator
                };
                if !relative.is_finite() {
                    return Err(invalid_data(
                        "intercept-slope MAP certificate ratio is not finite",
                    ));
                }
                relative_infinity = relative_infinity.max(relative);
                relative_squares = relative.mul_add(relative, relative_squares);
                count = count
                    .checked_add(1)
                    .ok_or_else(|| allocation("intercept-slope MAP certificate count overflows"))?;
                for channel in 0..channels {
                    let raw = workspace.cross[channel * residuals.ncols() + rhs].abs()
                        * workspace.value_scales[rhs]
                        * workspace.channel_scales[channel]
                        * weights.raw_scale;
                    absolute = absolute.max(if raw.is_finite() { raw } else { f64::INFINITY });
                }
            }
        }
    }
    let relative_two = if count == 0 {
        0.0
    } else {
        (relative_squares / count as f64).sqrt()
    };
    Ok(FeCertificate {
        absolute_infinity_norm: absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance,
    })
}

fn weighted_norms(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    scales: &mut [f64],
    sums: &mut [f64],
) -> Result<()> {
    if output.len() != values.ncols()
        || scales.len() != values.ncols()
        || sums.len() != values.ncols()
    {
        return Err(invalid_argument(
            "intercept-slope MAP norm workspace dimensions differ",
        ));
    }
    scales.fill(0.0);
    sums.fill(1.0);
    for row in 0..values.nrows() {
        let root_weight = weights.value(row).sqrt();
        for rhs in 0..values.ncols() {
            let value = root_weight * values[(row, rhs)];
            if !value.is_finite() {
                return Err(invalid_data(
                    "intercept-slope MAP weighted value is not finite",
                ));
            }
            update_scaled_sum(value, &mut scales[rhs], &mut sums[rhs]);
        }
    }
    for rhs in 0..values.ncols() {
        output[rhs] = if scales[rhs] == 0.0 {
            0.0
        } else {
            scales[rhs] * sums[rhs].sqrt()
        };
        if !output[rhs].is_finite() {
            return Err(invalid_data(
                "intercept-slope MAP weighted norm is not finite",
            ));
        }
    }
    Ok(())
}

fn validate_weights(weights: BorrowedMapWeights<'_>, nobs: usize) -> Result<ValidatedWeights<'_>> {
    match weights {
        BorrowedMapWeights::Unit => Ok(ValidatedWeights {
            values: None,
            raw_scale: 1.0,
        }),
        BorrowedMapWeights::Provided(values) => {
            if values.len() != nobs
                || values
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
            {
                return Err(invalid_data(
                    "intercept-slope MAP weights must be positive, finite, and aligned",
                ));
            }
            let raw_scale = values.iter().copied().fold(0.0_f64, f64::max);
            if values.iter().any(|value| *value / raw_scale == 0.0) {
                return Err(invalid_data(
                    "intercept-slope MAP normalized weight underflowed to zero",
                ));
            }
            Ok(ValidatedWeights {
                values: Some(values),
                raw_scale,
            })
        }
    }
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(invalid_argument(
            "intercept-slope MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(invalid_argument(
            "intercept-slope MAP iteration budgets must be positive",
        ));
    }
    Ok(())
}

fn finish(
    values: DenseMatrix,
    iterations: usize,
    certificate: FeCertificate,
    setup_seconds: f64,
    solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    CertifiedBorrowedMap::from_certified_parts(
        values,
        SolveReport {
            requested_solver: FeSolverKind::Map,
            used_solver: FeSolverKind::Map,
            iterations,
            stop_reason: SolveStopReason::Certified,
            normal_equation_residual: certificate.relative_two_norm,
            certificate,
            setup_seconds,
            solve_seconds,
            fallback_used: false,
            requested_preconditioner: None,
            used_preconditioner: None,
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
        },
    )
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let updated = *sum + value;
    let adjustment = if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *sum = updated;
    *correction += adjustment;
    if !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "intercept-slope MAP compensated accumulation is not finite",
        ));
    }
    Ok(())
}

fn update_scaled_sum(value: f64, scale: &mut f64, sum: &mut f64) {
    let absolute = value.abs();
    if absolute == 0.0 {
        return;
    }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *sum = 1.0 + *sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *sum += ratio * ratio;
    }
}

fn reserved_zeros<R: SlopeWorkspaceReservation>(
    elements: usize,
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<f64>> {
    let mut output = Vec::new();
    reservation.reserve_f64(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "intercept-slope MAP workspace reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, 0.0);
    Ok(output)
}

fn reserved_indices<R: SlopeWorkspaceReservation>(
    elements: usize,
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<usize>> {
    let mut output = Vec::new();
    reservation.reserve_usize(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "categorical slope MAP permutation reservation returned insufficient capacity",
        ));
    }
    output.extend(0..elements);
    Ok(output)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{finalize_categorical_topology, CategoricalCodeView};

    fn config() -> MapConfig {
        MapConfig {
            tolerance: 1e-11,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        }
    }

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize_at: Option<usize>,
    }

    impl SlopeWorkspaceReservation for InjectedReservation {
        fn reserve_f64(
            &mut self,
            target: &mut Vec<f64>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected intercept-slope MAP allocation failure",
                ));
            }
            if self.undersize_at != Some(call) {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test intercept-slope MAP reservation failed"))?;
            }
            Ok(())
        }

        fn reserve_usize(
            &mut self,
            target: &mut Vec<usize>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected categorical slope MAP allocation failure",
                ));
            }
            if self.undersize_at != Some(call) {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test categorical slope MAP reservation failed"))?;
            }
            Ok(())
        }
    }

    #[test]
    fn intercept_plus_slope_and_second_intercept_effect_match_explicit_span() {
        let group = [0, 0, 0, 1, 1, 1, 2, 2, 2];
        let second = [0, 1, 2, 0, 1, 2, 0, 1, 2];
        let sources = [
            CategoricalCodeView::new(&group, 3),
            CategoricalCodeView::new(&second, 3),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &[true; 9])
                .expect("topology");
        let slope = [0.0, 1.0, 2.0, 0.0, 2.0, 3.0, 1.0, 2.0, 4.0];
        let values = DenseMatrix::from_columns(&[
            (0..9)
                .map(|row| {
                    3.0 + 0.5 * group[row] as f64
                        + (group[row] as f64 + 1.0) * slope[row]
                        + 0.25 * second[row] as f64
                })
                .collect::<Vec<_>>(),
            (0..9).map(|row| slope[row] + second[row] as f64).collect(),
        ])
        .expect("values");
        let result = absorb_finalized_categorical_intercept_slope_map_certified(
            &topology,
            &[Some(&slope), None],
            BorrowedMapWeights::Unit,
            values,
            config(),
        )
        .expect("certified MAP");
        assert!(result.report().certified());
        assert!(result
            .residualized()
            .as_slice()
            .iter()
            .all(|value| value.abs() < 1e-9));
    }

    #[test]
    fn slope_only_multiple_channels_match_their_explicit_local_span() {
        let firm = [0, 0, 0, 1, 1, 1];
        let worker = [0, 0, 0, 1, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&worker, 2),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &[true; 6])
                .expect("topology");
        let time = [1.0, 2.0, 3.0, 1.0, 2.0, 3.0];
        let x2 = [1.0, 4.0, 2.0, 2.0, 1.0, 5.0];
        let effects = vec![
            CategoricalAbsorbedEffectView::new(false, vec![&time]).expect("firm slope"),
            CategoricalAbsorbedEffectView::new(false, vec![&time, &x2]).expect("worker slopes"),
        ];
        let values =
            DenseMatrix::from_columns(&[vec![3.0, 8.0, 8.0, 4.0, 5.0, 11.0]]).expect("values");
        let result = absorb_finalized_categorical_slope_map_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            values,
            config(),
        )
        .expect("multiple slope-only MAP");
        assert!(result.report().certified());
        assert!(result
            .residualized()
            .as_slice()
            .iter()
            .all(|value| value.abs() < 1e-9));
    }

    #[test]
    fn three_nonorthogonal_slope_channels_preserve_the_known_complement() {
        let group = [0_u32, 0, 0, 0];
        let sources = [CategoricalCodeView::new(&group, 1)];
        let topology = finalize_categorical_topology(&sources, &[&[0_usize][..]], &[true; 4])
            .expect("topology");
        let first = [1.0, 1.0, 0.0, 0.0];
        let second = [0.0, 1.0, 1.0, 0.0];
        let third = [0.0, 0.0, 1.0, 1.0];
        let complement = [-1.0, 1.0, -1.0, 1.0];
        let values = (0..4)
            .map(|row| 2.0 * first[row] - 3.0 * second[row] + 0.5 * third[row] + complement[row])
            .collect::<Vec<_>>();
        let effects = [
            CategoricalAbsorbedEffectView::new(false, vec![&first, &second, &third])
                .expect("three slopes"),
        ];
        let result = absorb_finalized_categorical_slope_map_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[values]).expect("values"),
            config(),
        )
        .expect("three-slope MAP");
        assert!(result.report().certified());
        for (actual, expected) in result.residualized().as_slice().iter().zip(complement) {
            assert!((actual - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn recovery_returns_centered_crossed_effects_and_requested_channels() {
        let worker = [0_u32, 0, 1, 1, 2, 2];
        let firm = [0_u32, 1, 0, 1, 0, 1];
        let topology = finalize_categorical_topology(
            &[
                CategoricalCodeView::new(&worker, 3),
                CategoricalCodeView::new(&firm, 2),
            ],
            &[&[0], &[1]],
            &[true; 6],
        )
        .expect("topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
        ];
        let worker_effect = [-1.0, -1.0, 0.25, 0.25, 0.75, 0.75];
        let firm_effect = [-0.5, 0.5, -0.5, 0.5, -0.5, 0.5];
        let target = worker_effect
            .iter()
            .zip(firm_effect)
            .map(|(left, right)| left + right)
            .collect::<Vec<_>>();
        let recovered = recover_finalized_categorical_effects_map_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, false],
            config(),
        )
        .expect("recovery");
        assert!(recovered.iterations() > 0);
        assert!(recovered.reconstruction_relative_infinity() < 1e-10);
        for (actual, expected) in recovered.absorbed_sum().iter().zip(&target) {
            assert!((actual - expected).abs() < 1e-10);
        }
        let saved_worker = recovered.channels()[0].as_ref().expect("worker output");
        for (actual, expected) in saved_worker.iter().zip(worker_effect) {
            assert!((actual - expected).abs() < 1e-10);
        }
        assert!(recovered.channels()[1].is_none());
    }

    #[test]
    fn coefficient_lsmr_recovery_matches_map_normalization() {
        let worker = [0_u32, 0, 1, 1, 2, 2];
        let firm = [0_u32, 1, 0, 1, 0, 1];
        let topology = finalize_categorical_topology(
            &[
                CategoricalCodeView::new(&worker, 3),
                CategoricalCodeView::new(&firm, 2),
            ],
            &[&[0], &[1]],
            &[true; 6],
        )
        .expect("topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
        ];
        let target = [-1.5, -0.5, -0.25, 0.75, 0.25, 1.25];
        let map = recover_finalized_categorical_effects_map_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            config(),
        )
        .expect("MAP recovery");
        let lsmr = recover_finalized_categorical_effects_lsmr_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            LsmrConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                ..LsmrConfig::default()
            },
            1e-12,
        )
        .expect("LSMR recovery");
        for (left, right) in lsmr.absorbed_sum().iter().zip(map.absorbed_sum()) {
            assert!((left - right).abs() < 1e-10);
        }
        for channel in 0..2 {
            for (left, right) in lsmr.channels()[channel]
                .as_ref()
                .expect("LSMR channel")
                .iter()
                .zip(map.channels()[channel].as_ref().expect("MAP channel"))
            {
                assert!((left - right).abs() < 1e-10, "{left} != {right}");
            }
        }
    }

    #[test]
    fn four_sweep_sd_pilot_then_lsmr_matches_strict_lsmr() {
        let worker = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let firm = [0_u32, 1, 1, 2, 2, 3, 3, 0];
        let topology = finalize_categorical_topology(
            &[
                CategoricalCodeView::new(&worker, 4),
                CategoricalCodeView::new(&firm, 4),
            ],
            &[&[0], &[1]],
            &[true; 8],
        )
        .expect("topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
        ];
        let target = [-1.5, -0.5, -0.25, 0.75, 0.25, 1.25, 1.5, -1.5];
        let lsmr_config = LsmrConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            ..LsmrConfig::default()
        };
        let hybrid = recover_finalized_categorical_effects_sd_lsmr_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            lsmr_config,
            MapConfig {
                tolerance: 1e-12,
                ..config()
            },
        )
        .expect("SD recovery");
        let lsmr = recover_finalized_categorical_effects_lsmr_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            lsmr_config,
            1e-12,
        )
        .expect("LSMR recovery");
        assert_eq!(
            hybrid.recovery_method(),
            CategoricalRecoveryMethod::HybridLsmr
        );
        assert!(hybrid.reconstruction_relative_infinity() <= 1e-10);
        for (actual, expected) in hybrid.absorbed_sum().iter().zip(lsmr.absorbed_sum()) {
            assert!((actual - expected).abs() <= 1e-10);
        }
        for channel in 0..2 {
            for (actual, expected) in hybrid.channels()[channel]
                .as_ref()
                .expect("hybrid channel")
                .iter()
                .zip(lsmr.channels()[channel].as_ref().expect("LSMR channel"))
            {
                assert!((actual - expected).abs() <= 1e-9);
            }
        }
    }

    #[test]
    fn borrowed_cg_recovery_matches_strict_lsmr_coefficients() {
        let worker = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let firm = [0_u32, 1, 1, 2, 2, 3, 3, 0];
        let topology = finalize_categorical_topology(
            &[
                CategoricalCodeView::new(&worker, 4),
                CategoricalCodeView::new(&firm, 4),
            ],
            &[&[0], &[1]],
            &[true; 8],
        )
        .expect("topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
        ];
        let target = [-1.5, -0.5, -0.25, 0.75, 0.25, 1.25, 1.5, -1.5];
        let cg = recover_finalized_categorical_effects_cg_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                check_every: 1,
                symmetric: true,
            },
        )
        .expect("CG recovery");
        let lsmr = recover_finalized_categorical_effects_lsmr_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            LsmrConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                ..LsmrConfig::default()
            },
            1e-12,
        )
        .expect("LSMR recovery");
        assert_eq!(cg.recovery_method(), CategoricalRecoveryMethod::Cg);
        for (actual, expected) in cg.absorbed_sum().iter().zip(lsmr.absorbed_sum()) {
            assert!((actual - expected).abs() <= 1e-10);
        }
        for channel in 0..2 {
            for (actual, expected) in cg.channels()[channel]
                .as_ref()
                .expect("CG channel")
                .iter()
                .zip(lsmr.channels()[channel].as_ref().expect("LSMR channel"))
            {
                assert!((actual - expected).abs() <= 1e-9);
            }
        }
    }

    #[test]
    fn sd_lsmr_recovery_uses_the_direct_one_effect_path_with_weights() {
        let group = [0_u32, 0, 1, 1, 2, 2];
        let topology = finalize_categorical_topology(
            &[CategoricalCodeView::new(&group, 3)],
            &[&[0]],
            &[true; 6],
        )
        .expect("topology");
        let effects = [CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("group")];
        let target = [-2.0, -2.0, 0.5, 0.5, 1.5, 1.5];
        let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0];
        let recovered = recover_finalized_categorical_effects_sd_lsmr_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Provided(&weights),
            &target,
            &[true],
            LsmrConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                ..LsmrConfig::default()
            },
            MapConfig {
                tolerance: 1e-12,
                ..config()
            },
        )
        .expect("direct recovery");
        assert_eq!(
            recovered.recovery_method(),
            CategoricalRecoveryMethod::Direct
        );
        assert_eq!(recovered.iterations(), 1);
        for (actual, expected) in recovered.absorbed_sum().iter().zip(target) {
            assert!((actual - expected).abs() <= 1e-12);
        }
    }

    #[test]
    fn coefficient_lsmr_recovery_normalizes_disconnected_components() {
        let worker = [0_u32, 1, 2, 3];
        let firm = [0_u32, 0, 1, 1];
        let topology = finalize_categorical_topology(
            &[
                CategoricalCodeView::new(&worker, 4),
                CategoricalCodeView::new(&firm, 2),
            ],
            &[&[0], &[1]],
            &[true; 4],
        )
        .expect("disconnected topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
        ];
        let target = [1.0, 3.0, 10.0, 14.0];
        let map = recover_finalized_categorical_effects_map_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            config(),
        )
        .expect("disconnected MAP recovery");
        let lsmr = recover_finalized_categorical_effects_lsmr_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            LsmrConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                ..LsmrConfig::default()
            },
            1e-12,
        )
        .expect("disconnected LSMR recovery");
        for channel in 0..2 {
            for (left, right) in lsmr.channels()[channel]
                .as_ref()
                .expect("LSMR channel")
                .iter()
                .zip(map.channels()[channel].as_ref().expect("MAP channel"))
            {
                assert!((left - right).abs() < 1e-10, "{left} != {right}");
            }
        }
    }

    #[test]
    fn recovery_waits_for_reconstruction_after_an_early_moment_certificate() {
        const GROUPS: usize = 72;
        let observations = GROUPS * 2;
        let mut worker_codes = Vec::with_capacity(observations);
        let mut firm_codes = Vec::with_capacity(observations);
        let mut target = Vec::with_capacity(observations);
        for firm in 0..GROUPS {
            for worker in [firm, (firm + 1) % GROUPS] {
                worker_codes.push(worker as u32);
                firm_codes.push(firm as u32);
                let worker_effect =
                    0.35 * (std::f64::consts::TAU * worker as f64 / GROUPS as f64).cos();
                let firm_effect = (std::f64::consts::TAU * firm as f64 / GROUPS as f64).sin();
                target.push(2.0 + worker_effect + firm_effect);
            }
        }
        let sources = [
            CategoricalCodeView::new(&worker_codes, GROUPS),
            CategoricalCodeView::new(&firm_codes, GROUPS),
        ];
        let retained = vec![true; observations];
        let topology =
            finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &retained)
                .expect("weak AKM-cycle topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker effect"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm effect"),
        ];
        let recovered = recover_finalized_categorical_effects_map_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                check_every: 1,
                symmetric: true,
            },
        )
        .expect("weak AKM-cycle recovery");
        assert!(recovered.reconstruction_relative_infinity() <= 1e-10);
        for (actual, expected) in recovered.absorbed_sum().iter().zip(target) {
            assert!((actual - expected).abs() <= 1e-9);
        }
    }

    #[test]
    fn weak_cycle_uses_the_hybrid_remainder_solve_and_certifies() {
        const GROUPS: usize = 72;
        let observations = GROUPS * 2;
        let mut worker_codes = Vec::with_capacity(observations);
        let mut firm_codes = Vec::with_capacity(observations);
        let mut target = Vec::with_capacity(observations);
        for firm in 0..GROUPS {
            for worker in [firm, (firm + 1) % GROUPS] {
                worker_codes.push(worker as u32);
                firm_codes.push(firm as u32);
                target.push(
                    0.35 * (std::f64::consts::TAU * worker as f64 / GROUPS as f64).cos()
                        + (std::f64::consts::TAU * firm as f64 / GROUPS as f64).sin(),
                );
            }
        }
        let topology = finalize_categorical_topology(
            &[
                CategoricalCodeView::new(&worker_codes, GROUPS),
                CategoricalCodeView::new(&firm_codes, GROUPS),
            ],
            &[&[0], &[1]],
            &vec![true; observations],
        )
        .expect("weak topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
        ];
        let recovered = recover_finalized_categorical_effects_sd_lsmr_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            LsmrConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                ..LsmrConfig::default()
            },
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                check_every: 1,
                symmetric: true,
            },
        )
        .expect("hybrid weak-cycle recovery");
        assert_eq!(
            recovered.recovery_method(),
            CategoricalRecoveryMethod::HybridLsmr
        );
        assert!(recovered.reconstruction_relative_infinity() <= 1e-10);
    }

    #[test]
    fn weak_cycle_cg_continues_to_the_reconstruction_certificate() {
        const GROUPS: usize = 72;
        let observations = GROUPS * 2;
        let mut worker_codes = Vec::with_capacity(observations);
        let mut firm_codes = Vec::with_capacity(observations);
        let mut target = Vec::with_capacity(observations);
        for firm in 0..GROUPS {
            for worker in [firm, (firm + 1) % GROUPS] {
                worker_codes.push(worker as u32);
                firm_codes.push(firm as u32);
                target.push(
                    0.35 * (std::f64::consts::TAU * worker as f64 / GROUPS as f64).cos()
                        + (std::f64::consts::TAU * firm as f64 / GROUPS as f64).sin(),
                );
            }
        }
        let topology = finalize_categorical_topology(
            &[
                CategoricalCodeView::new(&worker_codes, GROUPS),
                CategoricalCodeView::new(&firm_codes, GROUPS),
            ],
            &[&[0], &[1]],
            &vec![true; observations],
        )
        .expect("weak topology");
        let effects = [
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
            CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
        ];
        let recovered = recover_finalized_categorical_effects_cg_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            &target,
            &[true, true],
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                check_every: 1,
                symmetric: true,
            },
        )
        .expect("strict CG recovery");
        assert_eq!(recovered.recovery_method(), CategoricalRecoveryMethod::Cg);
        assert!(recovered.reconstruction_relative_infinity() <= 1e-10);
        for (actual, expected) in recovered.absorbed_sum().iter().zip(target) {
            assert!((actual - expected).abs() <= 1e-9);
        }
    }

    #[test]
    fn every_workspace_reservation_and_capacity_guard_fails_closed() {
        let group = [0, 0, 0, 1, 1, 1];
        let topology = finalize_categorical_topology(
            &[CategoricalCodeView::new(&group, 2)],
            &[&[0]],
            &[true; 6],
        )
        .expect("topology");
        let slope = [0.0, 1.0, 2.0, 0.0, 1.5, 3.0];
        let effects =
            [CategoricalAbsorbedEffectView::new(true, vec![&slope]).expect("slope effect")];
        let values =
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 4.0, 3.0, 5.0, 8.0]]).expect("values");
        let mut successful = InjectedReservation::default();
        absorb_with_reservation(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            values.clone(),
            config(),
            &mut successful,
        )
        .expect("allocation audit baseline");
        assert!(successful.calls > 0);
        for call in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &topology,
                &effects,
                BorrowedMapWeights::Unit,
                values.clone(),
                config(),
                &mut injected,
            )
            .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");

            let mut undersized = InjectedReservation {
                undersize_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &topology,
                &effects,
                BorrowedMapWeights::Unit,
                values.clone(),
                config(),
                &mut undersized,
            )
            .expect_err("undersized allocation");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
    }
}
