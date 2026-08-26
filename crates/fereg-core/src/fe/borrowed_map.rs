use std::time::Instant;

use super::auto::auto_selects_lsmr;
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_lsmr, BorrowedInterceptFeDesign, EffectTerm, FeCertificate, FeDesign, FeSolverKind,
    LsmrConfig, MapConfig, SolveReport, SolveStopReason,
};
use rayon::prelude::*;

const PARALLEL_PROJECTION_MIN_ROWS: usize = 65_536;
const FUSED_PARALLEL_MAX_RHS: usize = 8;
const PARALLEL_DOT_CHUNK_ROWS: usize = 32_768;

/// Objective weights for the strict borrowed-intercept MAP path.
///
/// Unit weights remain structural: the solver never materializes an
/// observation-scale vector of ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorrowedMapWeights<'a> {
    Unit,
    Provided(&'a [f64]),
}

/// A borrowed-intercept MAP result whose construction certifies the returned
/// residualized values.
#[derive(Debug, PartialEq)]
pub struct CertifiedBorrowedMap {
    residualized: DenseMatrix,
    report: SolveReport,
}

impl CertifiedBorrowedMap {
    #[must_use]
    pub const fn residualized(&self) -> &DenseMatrix {
        &self.residualized
    }

    #[must_use]
    pub const fn report(&self) -> &SolveReport {
        &self.report
    }

    #[must_use]
    pub fn into_parts(self) -> (DenseMatrix, SolveReport) {
        (self.residualized, self.report)
    }

    pub(crate) fn from_certified_parts(
        residualized: DenseMatrix,
        report: SolveReport,
    ) -> Result<Self> {
        if !report.certified() {
            return Err(invalid_data(
                "borrowed MAP attempted to construct an uncertified result",
            ));
        }
        Ok(Self {
            residualized,
            report,
        })
    }
}

trait WorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemWorkspaceReservation;

impl WorkspaceReservation for SystemWorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

struct BorrowedMapWorkspace {
    reference_norms: Vec<f64>,
    residual_norms: Vec<f64>,
    rhs_scales: Vec<f64>,
    rhs_relative_infinity_norms: Vec<f64>,
    value_scales: Vec<f64>,
    group_sums: Vec<f64>,
    group_corrections: Vec<f64>,
    parallel_group_means: Vec<f64>,
    parallel_group_weights: Vec<f64>,
    parallel_dot_sums: Vec<f64>,
}

impl BorrowedMapWorkspace {
    fn new<R: WorkspaceReservation>(
        design: &BorrowedInterceptFeDesign<'_>,
        observation_count: usize,
        rhs_count: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        let parallel_group_levels = if observation_count >= PARALLEL_PROJECTION_MIN_ROWS
            && rayon::current_num_threads() > 1
        {
            design
                .effects()
                .iter()
                .map(crate::fe::BorrowedInterceptEffect::n_levels)
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        let parallel_group_mean_count = if rhs_count > 1 || parallel_group_levels > 0 {
            parallel_group_levels
                .checked_mul(rhs_count)
                .ok_or_else(|| allocation("borrowed MAP parallel group-mean dimensions overflow"))?
        } else {
            0
        };
        let parallel_dot_sum_count = if observation_count >= PARALLEL_PROJECTION_MIN_ROWS
            && rhs_count <= FUSED_PARALLEL_MAX_RHS
            && rayon::current_num_threads() > 1
        {
            observation_count
                .div_ceil(PARALLEL_DOT_CHUNK_ROWS)
                .checked_mul(rhs_count)
                .ok_or_else(|| allocation("borrowed CG parallel dot dimensions overflow"))?
        } else {
            0
        };
        Ok(Self {
            reference_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP reference norms",
            )?,
            residual_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP residual norms",
            )?,
            rhs_scales: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP RHS scales",
            )?,
            rhs_relative_infinity_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP per-RHS certificate norms",
            )?,
            value_scales: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP value scales",
            )?,
            group_sums: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP group sums",
            )?,
            group_corrections: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP group corrections",
            )?,
            parallel_group_means: if parallel_group_mean_count == 0 {
                Vec::new()
            } else {
                reserved_zeros(
                    parallel_group_mean_count,
                    reservation,
                    "unable to reserve borrowed MAP parallel group means",
                )?
            },
            parallel_group_weights: if parallel_group_levels == 0 {
                Vec::new()
            } else {
                reserved_zeros(
                    parallel_group_levels,
                    reservation,
                    "unable to reserve borrowed MAP parallel group weights",
                )?
            },
            parallel_dot_sums: if parallel_dot_sum_count == 0 {
                Vec::new()
            } else {
                reserved_zeros(
                    parallel_dot_sum_count,
                    reservation,
                    "unable to reserve borrowed CG parallel dot sums",
                )?
            },
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ValidatedWeights<'a> {
    values: Option<&'a [f64]>,
    raw_scale: f64,
}

impl ValidatedWeights<'_> {
    #[must_use]
    fn normalized(self, observation: usize) -> f64 {
        self.values
            .map_or(1.0, |values| values[observation] / self.raw_scale)
    }
}

/// Residualize a nonempty dense matrix against a validated borrowed,
/// intercept-only FE design.
///
/// The matrix is consumed so a failure cannot expose partially residualized
/// caller state. Every numerical workspace is fallibly allocated before the
/// first projection. A successful return always has a passing independently
/// recomputed fixed-effect certificate.
pub fn absorb_borrowed_intercept_map_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation(design, weights, values, config, false, &mut reservation)
}

/// Residualize with batched weighted conjugate gradients over the borrowed
/// symmetric-Kaczmarz projection operator. No observation-scale categorical
/// codes or unit-weight vector are copied.
pub fn absorb_borrowed_intercept_cg_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        false,
        None,
        &mut reservation,
    )
}

/// Automatic borrowed-intercept policy: direct one-pass MAP for one effect;
/// otherwise run at most four symmetric MAP pilot sweeps and continue with
/// genuine CG unless the pilot already certifies.
pub fn absorb_borrowed_intercept_auto_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_auto_with_reservation(
        design,
        weights,
        values,
        config,
        None,
        false,
        &mut reservation,
    )
}

/// Prepared-data automatic policy. Categorical codes stay borrowed for the
/// MAP pilot and CG continuation; they are copied fallibly only when the
/// frozen pilot rule selects LSMR.
pub(crate) fn absorb_borrowed_intercept_auto_routed_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_auto_with_reservation(
        design,
        weights,
        values,
        config,
        Some(lsmr),
        false,
        &mut reservation,
    )
}

/// Prepared OLS compatibility path: an empty explicit FE design still owns a
/// structural one-level constant. The global projection uses only the existing
/// dense RHS plus O(K) workspace; it never materializes N constant codes.
pub(crate) fn absorb_borrowed_intercept_map_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation(design, weights, values, config, true, &mut reservation)
}

/// Prepared OLS compatibility path for CG when the empty explicit design owns
/// a structural one-level constant.
pub(crate) fn absorb_borrowed_intercept_cg_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        true,
        None,
        &mut reservation,
    )
}

pub(crate) fn absorb_borrowed_intercept_auto_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_auto_with_reservation(
        design,
        weights,
        values,
        config,
        Some(lsmr),
        true,
        &mut reservation,
    )
}

/// Prepared OLS initial-RHS path. A one-effect projection is exact, so the raw
/// input is projected once and independently certified once. Residual reuse
/// continues to use the ordinary auto entry point, whose initial certificate
/// can return an already-within residual without another projection.
pub(crate) fn absorb_borrowed_intercept_auto_initial_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    if design.effects().len() <= 1 {
        let direct = absorb_initial_with_reservation(
            design,
            weights,
            values,
            config,
            true,
            &mut reservation,
        )?;
        mark_auto(direct, 0, 0.0, 0.0, 0.0)
    } else {
        absorb_auto_with_reservation(
            design,
            weights,
            values,
            config,
            Some(lsmr),
            true,
            &mut reservation,
        )
    }
}

fn absorb_auto_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    lsmr_config: Option<LsmrConfig>,
    empty_design_is_structural_constant: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    if design.nobs() != values.nrows() {
        return Err(invalid_argument(
            "borrowed auto design and matrix observation counts differ",
        ));
    }
    if values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "borrowed auto requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed auto values must be finite"));
    }

    if design.effects().len() <= 1 {
        let direct = absorb_with_reservation(
            design,
            weights,
            values,
            config,
            empty_design_is_structural_constant,
            reservation,
        )?;
        return mark_auto(direct, 0, 0.0, 0.0, 0.0);
    }

    let weights = validate_weights(weights, values.nrows())?;
    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let initial_certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
    let pilot_setup_seconds = setup_started.elapsed().as_secs_f64();
    if initial_certificate.passed {
        let result = certified_solver_result(
            values,
            FeSolverKind::Map,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            0.0,
            pilot_setup_seconds,
            0.0,
        )?;
        return mark_auto(result, 0, 0.0, 0.0, 0.0);
    }

    let initial_moment = initial_certificate.relative_two_norm;
    let pilot_budget = config.max_iterations.min(4);
    let pilot_started = Instant::now();
    let mut final_pilot_certificate = initial_certificate;
    for pilot_iteration in 1..=pilot_budget {
        for effect in design.effects() {
            project_effect(effect, weights, &mut values, &mut workspace)?;
        }
        for effect in design.effects().iter().rev().skip(1) {
            project_effect(effect, weights, &mut values, &mut workspace)?;
        }
        final_pilot_certificate =
            certify(design, weights, &values, config.tolerance, &mut workspace)?;
        let contraction =
            pilot_contraction(initial_moment, final_pilot_certificate.relative_two_norm)?;
        if final_pilot_certificate.passed {
            let pilot_seconds = pilot_started.elapsed().as_secs_f64();
            let result = certified_solver_result(
                values,
                FeSolverKind::Map,
                SolveStopReason::Certified,
                pilot_iteration,
                final_pilot_certificate,
                contraction,
                pilot_setup_seconds,
                pilot_seconds,
            )?;
            return mark_auto(result, pilot_iteration, contraction, 0.0, 0.0);
        }
    }
    let pilot_seconds = pilot_started.elapsed().as_secs_f64();
    let contraction = pilot_contraction(initial_moment, final_pilot_certificate.relative_two_norm)?;
    if pilot_budget == config.max_iterations {
        return Err(invalid_data(
            "borrowed auto exhausted its iteration budget during the MAP pilot",
        ));
    }

    // CG continues the original absorption problem. Keep the raw RHS norms
    // used by the pilot certificate: recomputing the reference scale from the
    // already-residualized pilot state makes the same requested tolerance
    // artificially stricter and can erase the pilot's benefit on slow graphs.
    let original_reference_norms = std::mem::take(&mut workspace.reference_norms);
    drop(workspace);
    let remaining_iterations = config.max_iterations - pilot_budget;
    let mut lsmr_failure = None;
    let total_fe_coefficients = design.effects().iter().try_fold(0_usize, |total, effect| {
        total
            .checked_add(effect.n_levels())
            .ok_or_else(|| invalid_data("borrowed auto FE coefficient count overflows usize"))
    })?;
    if let Some(lsmr_config) =
        lsmr_config.filter(|_| auto_selects_lsmr(contraction, total_fe_coefficients))
    {
        let lsmr_setup_started = Instant::now();
        let owned_design = own_borrowed_intercept_design(design)?;
        let unit_weights;
        let lsmr_weights = match weights.values {
            Some(values) => values,
            None => {
                unit_weights = fallible_filled(
                    values.nrows(),
                    1.0,
                    "unable to reserve automatic LSMR unit weights",
                )?;
                unit_weights.as_slice()
            }
        };
        let mut lsmr_values =
            fallible_matrix_copy(&values, "unable to reserve automatic LSMR fallback matrix")?;
        let owned_setup_seconds = lsmr_setup_started.elapsed().as_secs_f64();
        let lsmr_config = LsmrConfig {
            tolerance: config.tolerance,
            max_iterations: lsmr_config.max_iterations.min(remaining_iterations),
            ..lsmr_config
        };
        match absorb_lsmr(&owned_design, lsmr_weights, &mut lsmr_values, lsmr_config) {
            Ok(mut report) if report.certified() => {
                report.requested_solver = FeSolverKind::Auto;
                report.iterations = report.iterations.saturating_add(pilot_budget);
                report.pilot_iterations = pilot_budget;
                report.pilot_contraction = contraction;
                report.setup_seconds += pilot_setup_seconds + owned_setup_seconds;
                report.solve_seconds += pilot_seconds;
                return CertifiedBorrowedMap::from_certified_parts(lsmr_values, report);
            }
            Ok(report) => {
                lsmr_failure = Some(format!(
                    "automatic LSMR path did not certify after {} iterations; retried CG",
                    report.iterations
                ));
            }
            Err(error) => {
                lsmr_failure = Some(format!("automatic LSMR path failed ({error}); retried CG"));
            }
        }
    }
    let cg_config = MapConfig {
        max_iterations: remaining_iterations,
        ..config
    };
    let cg = absorb_cg_with_reservation(
        design,
        match weights.values {
            Some(values) => BorrowedMapWeights::Provided(values),
            None => BorrowedMapWeights::Unit,
        },
        values,
        cg_config,
        false,
        Some(&original_reference_norms),
        reservation,
    )?;
    let marked = mark_auto(
        cg,
        pilot_budget,
        contraction,
        pilot_setup_seconds,
        pilot_seconds,
    )?;
    if let Some(warning) = lsmr_failure {
        let (residualized, mut report) = marked.into_parts();
        report.fallback_used = true;
        report.warnings.insert(0, warning);
        return CertifiedBorrowedMap::from_certified_parts(residualized, report);
    }
    Ok(marked)
}

fn own_borrowed_intercept_design(design: &BorrowedInterceptFeDesign<'_>) -> Result<FeDesign> {
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(design.effects().len())
        .map_err(|_| allocation("unable to reserve automatic LSMR FE effects"))?;
    for effect in design.effects() {
        effects.push(EffectTerm::new(
            fallible_copy(effect.codes(), "unable to reserve automatic LSMR FE codes")?,
            effect.n_levels(),
            true,
            Vec::new(),
        )?);
    }
    FeDesign::new(design.nobs(), effects)
}

fn pilot_contraction(initial: f64, current: f64) -> Result<f64> {
    if !initial.is_finite() || initial < 0.0 || !current.is_finite() || current < 0.0 {
        return Err(invalid_data("borrowed auto pilot moment is invalid"));
    }
    let contraction = if initial == 0.0 {
        if current == 0.0 {
            0.0
        } else {
            return Err(invalid_data(
                "borrowed auto pilot acquired a moment from a zero initial norm",
            ));
        }
    } else {
        current / initial
    };
    if !contraction.is_finite() || contraction < 0.0 {
        return Err(invalid_data("borrowed auto pilot contraction is invalid"));
    }
    Ok(contraction)
}

fn mark_auto(
    result: CertifiedBorrowedMap,
    pilot_iterations: usize,
    contraction: f64,
    additional_setup_seconds: f64,
    additional_solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    let (residualized, mut report) = result.into_parts();
    report.requested_solver = FeSolverKind::Auto;
    report.pilot_iterations = pilot_iterations;
    report.pilot_contraction = contraction;
    report.setup_seconds += additional_setup_seconds;
    report.solve_seconds += additional_solve_seconds;
    CertifiedBorrowedMap::from_certified_parts(residualized, report)
}

fn absorb_cg_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    continuation_reference_norms: Option<&[f64]>,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    if design.nobs() != values.nrows() {
        return Err(invalid_argument(
            "borrowed CG design and matrix observation counts differ",
        ));
    }
    if values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "borrowed CG requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed CG values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;

    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let rhs_count = values.ncols();
    let mut improvement_potential = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG Hestenes improvement potential",
    )?;
    for (potential, norm) in improvement_potential
        .iter_mut()
        .zip(&workspace.reference_norms)
    {
        *potential = norm * norm;
        if !potential.is_finite() || *potential < 0.0 {
            return Err(invalid_data(
                "borrowed CG Hestenes improvement potential is invalid",
            ));
        }
    }
    if let Some(reference_norms) = continuation_reference_norms {
        if reference_norms.len() != values.ncols()
            || reference_norms
                .iter()
                .any(|norm| !norm.is_finite() || *norm < 0.0)
        {
            return Err(invalid_argument(
                "borrowed CG continuation reference norms are invalid",
            ));
        }
        workspace.reference_norms.copy_from_slice(reference_norms);
    }
    let mut residual_square = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG residual norms",
    )?;
    let mut initial_residual_square = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG initial residual norms",
    )?;
    let mut direction_projection = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG direction products",
    )?;
    let mut next_residual_square = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG updated residual norms",
    )?;
    let mut alpha = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG alpha coefficients",
    )?;
    let mut beta = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG beta coefficients",
    )?;
    let mut dot_corrections = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG dot corrections",
    )?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();

    if design.effects().is_empty() && !empty_design_is_structural_constant {
        return certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::NoFixedEffects,
            0,
            zero_certificate(),
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    let initial_certificate = if design.effects().is_empty() {
        certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?
    } else {
        certify(design, weights, &values, config.tolerance, &mut workspace)?
    };
    if initial_certificate.passed {
        return certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    if design.effects().is_empty() {
        project_structural_constant(weights, &mut values, &mut workspace)?;
        let certificate =
            certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?;
        if !certificate.passed {
            return Err(invalid_data(
                "borrowed CG structural-constant projection failed certification",
            ));
        }
        return certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::Certified,
            1,
            certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    let mut residual = fallible_matrix_zeros(
        values.nrows(),
        values.ncols(),
        "unable to reserve borrowed CG residual matrix",
    )?;
    borrowed_symmetric_projection_into(design, weights, &values, &mut residual, &mut workspace)?;
    let mut direction =
        fallible_matrix_copy(&residual, "unable to reserve borrowed CG direction matrix")?;
    let mut projection = fallible_matrix_zeros(
        values.nrows(),
        values.ncols(),
        "unable to reserve borrowed CG projection matrix",
    )?;
    weighted_dot_columns_into(
        &residual,
        &residual,
        weights,
        &mut residual_square,
        &mut dot_corrections,
        &mut workspace.parallel_dot_sums,
    )?;
    initial_residual_square.copy_from_slice(&residual_square);
    let mut recurrence_residual = 1.0_f64;
    let mut hestenes_error = 1.0_f64;

    for iteration in 1..=config.max_iterations {
        borrowed_symmetric_projection_into(
            design,
            weights,
            &direction,
            &mut projection,
            &mut workspace,
        )?;
        weighted_dot_columns_into(
            &direction,
            &projection,
            weights,
            &mut direction_projection,
            &mut dot_corrections,
            &mut workspace.parallel_dot_sums,
        )?;
        let alpha_result = cg_ratios_into(
            &residual_square,
            &direction_projection,
            &mut alpha,
            "borrowed CG direction denominator is nonfinite or nonpositive",
        );
        if let Err(error) = alpha_result {
            // A semidefinite projection operator can lose its remaining
            // direction to roundoff exactly at convergence. Batched RHSs can
            // reach that boundary at different iterations, so independently
            // certify the current iterate and deactivate only broken columns
            // that already satisfy their own certificate. Other columns keep
            // iterating, and publication still requires the final joint
            // certificate.
            let certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if certificate.passed {
                return certified_solver_result(
                    values,
                    FeSolverKind::Cg,
                    SolveStopReason::Certified,
                    iteration - 1,
                    certificate,
                    recurrence_residual,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                );
            }
            let mut recovered = false;
            for rhs in 0..residual_square.len() {
                let numerator = residual_square[rhs];
                let denominator = direction_projection[rhs];
                if !numerator.is_finite() || numerator < 0.0 {
                    return Err(error);
                }
                if numerator == 0.0 {
                    alpha[rhs] = 0.0;
                } else if denominator.is_finite() && denominator > 0.0 {
                    let ratio = numerator / denominator;
                    if !ratio.is_finite() || ratio < 0.0 {
                        return Err(error);
                    }
                    alpha[rhs] = ratio;
                } else if denominator.is_finite() {
                    if workspace.rhs_relative_infinity_norms[rhs] > config.tolerance {
                        // Finite nonpositive curvature for this PSD operator is
                        // a roundoff loss of conjugacy. Setting p=0 with
                        // alpha=0 makes the Fletcher--Reeves update below
                        // restart this RHS from its unchanged residual.
                        for row in 0..values.nrows() {
                            direction[(row, rhs)] = 0.0;
                        }
                        alpha[rhs] = 0.0;
                        recovered = true;
                        continue;
                    }
                    for row in 0..values.nrows() {
                        residual[(row, rhs)] = 0.0;
                        direction[(row, rhs)] = 0.0;
                    }
                    residual_square[rhs] = 0.0;
                    alpha[rhs] = 0.0;
                    recovered = true;
                } else {
                    return Err(error);
                }
            }
            if !recovered {
                return Err(error);
            }
        }
        hestenes_error =
            cg_hestenes_error_and_update(&alpha, &residual_square, &mut improvement_potential)?;
        cg_update_solution_and_residual(
            &mut values,
            &mut residual,
            &direction,
            &projection,
            &alpha,
        )?;
        weighted_dot_columns_into(
            &residual,
            &residual,
            weights,
            &mut next_residual_square,
            &mut dot_corrections,
            &mut workspace.parallel_dot_sums,
        )?;
        recurrence_residual =
            cg_largest_relative_residual(&next_residual_square, &initial_residual_square)?;

        // Match reghdfe's Hestenes stopping rule, then require the independent
        // FE-moment certificate as a second condition before publication.
        // This avoids accepting a weak-graph iterate merely because its FE
        // moments are small while keeping certification out of the hot loop.
        if hestenes_error <= config.tolerance || iteration == config.max_iterations {
            let certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if certificate.passed && hestenes_error <= config.tolerance {
                return certified_solver_result(
                    values,
                    FeSolverKind::Cg,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    recurrence_residual,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                );
            }
        }

        cg_ratios_into(
            &next_residual_square,
            &residual_square,
            &mut beta,
            "borrowed CG residual denominator is nonfinite or nonpositive",
        )?;
        cg_update_direction(&mut direction, &residual, &beta)?;
        residual_square.copy_from_slice(&next_residual_square);
    }

    Err(invalid_data(format!(
        "borrowed CG exhausted {} iterations without comparator-equivalent recurrence convergence and a passing final certificate (Hestenes error {hestenes_error:e}, recurrence residual {recurrence_residual:e})",
        config.max_iterations,
    )))
}

fn absorb_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    absorb_with_reservation_mode(
        design,
        weights,
        values,
        config,
        empty_design_is_structural_constant,
        false,
        reservation,
    )
}

fn absorb_initial_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    absorb_with_reservation_mode(
        design,
        weights,
        values,
        config,
        empty_design_is_structural_constant,
        true,
        reservation,
    )
}

#[allow(clippy::too_many_arguments)]
fn absorb_with_reservation_mode<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    skip_initial_one_effect_certificate: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    if design.nobs() != values.nrows() {
        return Err(invalid_argument(
            "borrowed MAP design and matrix observation counts differ",
        ));
    }
    if values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "borrowed MAP requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed MAP values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;

    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();

    if design.effects().is_empty() && !empty_design_is_structural_constant {
        return certified_result(
            values,
            SolveStopReason::NoFixedEffects,
            0,
            zero_certificate(),
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    if !skip_initial_one_effect_certificate {
        let initial_certificate = if design.effects().is_empty() {
            certify_initial_structural_constant(weights, &values, config.tolerance, &mut workspace)?
        } else {
            certify_initial(design, weights, &values, config.tolerance, &mut workspace)?
        };
        if initial_certificate.passed {
            return certified_result(
                values,
                SolveStopReason::Certified,
                0,
                initial_certificate,
                setup_seconds,
                solve_started.elapsed().as_secs_f64(),
            );
        }
    }

    if design.effects().is_empty() {
        project_structural_constant(weights, &mut values, &mut workspace)?;
        let certificate =
            certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?;
        if !certificate.passed {
            return Err(invalid_data(
                "borrowed MAP structural-constant projection failed certification",
            ));
        }
        return certified_result(
            values,
            SolveStopReason::Certified,
            1,
            certificate,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    for iteration in 1..=config.max_iterations {
        for effect in design.effects() {
            project_effect(effect, weights, &mut values, &mut workspace)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                project_effect(effect, weights, &mut values, &mut workspace)?;
            }
        }
        // One intercept effect is removed exactly by its first projection, so
        // certify immediately instead of waiting for the periodic multiway
        // cadence. Multiway designs retain periodic plus final certification.
        if design.effects().len() <= 1
            || iteration % config.check_every == 0
            || iteration == config.max_iterations
        {
            let certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if certificate.passed {
                return certified_result(
                    values,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                );
            }
        }
    }

    Err(invalid_data(
        "borrowed MAP exhausted its iteration budget without a passing final certificate",
    ))
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(invalid_argument(
            "borrowed MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(invalid_argument(
            "borrowed MAP iteration and check budgets must be positive",
        ));
    }
    Ok(())
}

fn validate_weights(weights: BorrowedMapWeights<'_>, nrows: usize) -> Result<ValidatedWeights<'_>> {
    match weights {
        BorrowedMapWeights::Unit => Ok(ValidatedWeights {
            values: None,
            raw_scale: 1.0,
        }),
        BorrowedMapWeights::Provided(values) => {
            if values.len() != nrows {
                return Err(invalid_argument(
                    "borrowed MAP weight and observation counts differ",
                ));
            }
            if values
                .iter()
                .any(|weight| !weight.is_finite() || *weight <= 0.0)
            {
                return Err(invalid_data(
                    "borrowed MAP weights must be finite and positive",
                ));
            }
            let raw_scale = values.iter().copied().fold(0.0_f64, f64::max);
            if values.iter().any(|weight| *weight / raw_scale == 0.0) {
                return Err(invalid_data(
                    "borrowed MAP normalized weight underflowed to zero",
                ));
            }
            Ok(ValidatedWeights {
                values: Some(values),
                raw_scale,
            })
        }
    }
}

fn project_structural_constant(
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    structural_constant_means(weights, values, workspace)?;
    for observation in 0..values.nrows() {
        for rhs in 0..values.ncols() {
            let residual = values[(observation, rhs)] - workspace.group_sums[rhs];
            if !residual.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP structural-constant projection produced a nonfinite residual",
                ));
            }
            values[(observation, rhs)] = residual;
        }
    }
    Ok(())
}

fn structural_constant_means(
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<f64> {
    workspace.value_scales.fill(0.0);
    workspace.group_sums.fill(0.0);
    workspace.group_corrections.fill(0.0);
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for observation in 0..values.nrows() {
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
        for rhs in 0..values.ncols() {
            workspace.value_scales[rhs] =
                workspace.value_scales[rhs].max(values[(observation, rhs)].abs());
        }
    }
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP structural-constant weight sum is invalid",
        ));
    }
    for observation in 0..values.nrows() {
        let weight = weights.normalized(observation);
        for rhs in 0..values.ncols() {
            let scale = workspace.value_scales[rhs];
            if scale != 0.0 {
                compensated_add(
                    weight * (values[(observation, rhs)] / scale),
                    &mut workspace.group_sums[rhs],
                    &mut workspace.group_corrections[rhs],
                )?;
            }
        }
    }
    for rhs in 0..values.ncols() {
        let scale = workspace.value_scales[rhs];
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * ((workspace.group_sums[rhs] + workspace.group_corrections[rhs]) / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data(
                "borrowed MAP structural-constant mean is not finite",
            ));
        }
        workspace.group_sums[rhs] = mean;
    }
    Ok(total_weight)
}

fn certify_structural_constant(
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    weighted_column_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    certify_structural_constant_with_current_norms(weights, residuals, tolerance, workspace)
}

fn certify_initial_structural_constant(
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    workspace
        .residual_norms
        .copy_from_slice(&workspace.reference_norms);
    certify_structural_constant_with_current_norms(weights, residuals, tolerance, workspace)
}

fn certify_structural_constant_with_current_norms(
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    for rhs in 0..residuals.ncols() {
        let scale = workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
        if !scale.is_finite() || scale < 0.0 {
            return Err(invalid_data(
                "borrowed MAP structural-constant RHS scale is invalid",
            ));
        }
        workspace.rhs_scales[rhs] = scale;
    }
    let group_weight = structural_constant_means(weights, residuals, workspace)?;
    let root_group_weight = group_weight.sqrt();
    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    for rhs in 0..residuals.ncols() {
        let mean = workspace.group_sums[rhs];
        let denominator = workspace.rhs_scales[rhs];
        let relative = if denominator == 0.0 {
            if mean == 0.0 {
                0.0
            } else {
                return Err(invalid_data(
                    "borrowed MAP structural-constant zero-scale certificate failed",
                ));
            }
        } else {
            (mean.abs() / denominator) * root_group_weight
        };
        if !relative.is_finite() {
            return Err(invalid_data(
                "borrowed MAP structural-constant certificate ratio is not finite",
            ));
        }
        relative_infinity_norm = relative_infinity_norm.max(relative);
        relative_square_sum = relative.mul_add(relative, relative_square_sum);
        let normalized_moment = mean.abs() * group_weight;
        let raw_moment = normalized_moment * weights.raw_scale;
        absolute_infinity_norm = absolute_infinity_norm.max(if raw_moment.is_finite() {
            raw_moment
        } else if normalized_moment == 0.0 {
            0.0
        } else {
            f64::INFINITY
        });
    }
    let relative_two_norm = (relative_square_sum / residuals.ncols() as f64).sqrt();
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP structural-constant certificate norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn project_effect(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    if effect.codes().len() != values.nrows() {
        return Err(invalid_argument(
            "borrowed MAP effect and matrix observation counts differ",
        ));
    }
    let parallel_mean_count = effect
        .n_levels()
        .checked_mul(values.ncols())
        .ok_or_else(|| allocation("borrowed MAP parallel group-mean dimensions overflow"))?;
    if values.ncols() > 1
        && rayon::current_num_threads() > 1
        && workspace.parallel_group_means.len() >= parallel_mean_count
    {
        return project_effect_parallel(effect, weights, values, workspace, parallel_mean_count);
    }
    project_effect_serial(effect, weights, values, workspace)
}

fn project_effect_serial(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    for level in 0..effect.n_levels() {
        let members = effect.level_index().checked_members(level)?;
        if members.is_empty() {
            return Err(invalid_argument(
                "borrowed MAP effect contains an empty final level",
            ));
        }
        group_means(members, weights, values, workspace)?;
        for &observation in members {
            if observation >= values.nrows() {
                return Err(invalid_argument(
                    "borrowed MAP membership exceeds the observation axis",
                ));
            }
            for rhs in 0..values.ncols() {
                let residual = values[(observation, rhs)] - workspace.group_sums[rhs];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                values[(observation, rhs)] = residual;
            }
        }
    }
    Ok(())
}

fn project_effect_parallel(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel_mean_count: usize,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    if columns <= FUSED_PARALLEL_MAX_RHS {
        return project_effect_parallel_fused(
            effect,
            weights,
            values,
            workspace,
            parallel_mean_count,
        );
    }
    workspace.parallel_group_weights[..levels]
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(level, group_weight)| -> Result<()> {
            let members = effect.level_index().checked_members(level)?;
            *group_weight = group_weight_sum(members, weights, values.nrows())?;
            Ok(())
        })?;
    let group_weights = &workspace.parallel_group_weights[..levels];
    let values_read = &*values;
    workspace.parallel_group_means[..parallel_mean_count]
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(slot, mean)| -> Result<()> {
            let level = slot / columns;
            let rhs = slot % columns;
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP effect contains an empty final level",
                ));
            }
            *mean = group_mean_for_column_with_weight(
                members,
                rhs,
                weights,
                group_weights[level],
                values_read,
            )?;
            Ok(())
        })?;

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    let codes = effect.codes();
    values
        .as_mut_slice()
        .par_chunks_mut(columns)
        .enumerate()
        .try_for_each(|(observation, row)| -> Result<()> {
            let level = codes[observation] as usize;
            if level >= levels {
                return Err(invalid_argument(
                    "borrowed MAP code exceeds the finalized level count",
                ));
            }
            for (rhs, value) in row.iter_mut().enumerate() {
                let residual = *value - means[level * columns + rhs];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                *value = residual;
            }
            Ok(())
        })?;
    Ok(())
}

fn project_effect_parallel_fused(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel_mean_count: usize,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    let values_read = &*values;
    workspace.parallel_group_means[..parallel_mean_count]
        .par_chunks_mut(columns)
        .enumerate()
        .try_for_each(|(level, means)| -> Result<()> {
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP effect contains an empty final level",
                ));
            }
            let mut scales = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
            let mut sums = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
            let mut corrections = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
            let mut weight_sum = 0.0_f64;
            let mut weight_correction = 0.0_f64;
            for &observation in members {
                if observation >= values_read.nrows() {
                    return Err(invalid_argument(
                        "borrowed MAP membership exceeds the observation axis",
                    ));
                }
                if weights.values.is_some() {
                    compensated_add(
                        weights.normalized(observation),
                        &mut weight_sum,
                        &mut weight_correction,
                    )?;
                }
                for rhs in 0..columns {
                    scales[rhs] = scales[rhs].max(values_read[(observation, rhs)].abs());
                }
            }
            let total_weight = if weights.values.is_some() {
                weight_sum + weight_correction
            } else {
                members.len() as f64
            };
            if !total_weight.is_finite() || total_weight <= 0.0 {
                return Err(invalid_data(
                    "borrowed MAP group weight sum is not finite and positive",
                ));
            }
            if let Some(weight_values) = weights.values {
                for &observation in members {
                    let weight = weight_values[observation] / weights.raw_scale;
                    for rhs in 0..columns {
                        if scales[rhs] != 0.0 {
                            compensated_add(
                                weight * (values_read[(observation, rhs)] / scales[rhs]),
                                &mut sums[rhs],
                                &mut corrections[rhs],
                            )?;
                        }
                    }
                }
            } else {
                for &observation in members {
                    for rhs in 0..columns {
                        if scales[rhs] != 0.0 {
                            compensated_add(
                                values_read[(observation, rhs)] / scales[rhs],
                                &mut sums[rhs],
                                &mut corrections[rhs],
                            )?;
                        }
                    }
                }
            }
            for rhs in 0..columns {
                let mean = if scales[rhs] == 0.0 {
                    0.0
                } else {
                    scales[rhs] * ((sums[rhs] + corrections[rhs]) / total_weight)
                };
                if !mean.is_finite() {
                    return Err(invalid_data("borrowed MAP group mean is not finite"));
                }
                means[rhs] = mean;
            }
            Ok(())
        })?;

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    let codes = effect.codes();
    values
        .as_mut_slice()
        .par_chunks_mut(columns)
        .enumerate()
        .try_for_each(|(observation, row)| -> Result<()> {
            let level = codes[observation] as usize;
            if level >= levels {
                return Err(invalid_argument(
                    "borrowed MAP code exceeds the finalized level count",
                ));
            }
            for (rhs, value) in row.iter_mut().enumerate() {
                let residual = *value - means[level * columns + rhs];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                *value = residual;
            }
            Ok(())
        })?;
    Ok(())
}

fn group_weight_sum(members: &[usize], weights: ValidatedWeights<'_>, nrows: usize) -> Result<f64> {
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= nrows {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
    }
    let total_weight = weight_sum + weight_correction;
    if total_weight.is_finite() && total_weight > 0.0 {
        Ok(total_weight)
    } else {
        Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ))
    }
}

fn group_mean_for_column_with_weight(
    members: &[usize],
    rhs: usize,
    weights: ValidatedWeights<'_>,
    total_weight: f64,
    values: &DenseMatrix,
) -> Result<f64> {
    let mut scale = 0.0_f64;
    for &observation in members {
        if observation >= values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        scale = scale.max(values[(observation, rhs)].abs());
    }

    let mut normalized_sum = 0.0_f64;
    let mut correction = 0.0_f64;
    if scale != 0.0 {
        for &observation in members {
            compensated_add(
                weights.normalized(observation) * (values[(observation, rhs)] / scale),
                &mut normalized_sum,
                &mut correction,
            )?;
        }
    }
    let mean = if scale == 0.0 {
        0.0
    } else {
        scale * ((normalized_sum + correction) / total_weight)
    };
    if !mean.is_finite() {
        return Err(invalid_data("borrowed MAP group mean is not finite"));
    }
    Ok(mean)
}

fn group_means(
    members: &[usize],
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<f64> {
    workspace.value_scales.fill(0.0);
    workspace.group_sums.fill(0.0);
    workspace.group_corrections.fill(0.0);

    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
        for rhs in 0..values.ncols() {
            workspace.value_scales[rhs] =
                workspace.value_scales[rhs].max(values[(observation, rhs)].abs());
        }
    }
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ));
    }

    for &observation in members {
        let weight = weights.normalized(observation);
        for rhs in 0..values.ncols() {
            let scale = workspace.value_scales[rhs];
            if scale == 0.0 {
                continue;
            }
            let term = weight * (values[(observation, rhs)] / scale);
            compensated_add(
                term,
                &mut workspace.group_sums[rhs],
                &mut workspace.group_corrections[rhs],
            )?;
        }
    }

    for rhs in 0..values.ncols() {
        let scale = workspace.value_scales[rhs];
        let normalized_sum = workspace.group_sums[rhs] + workspace.group_corrections[rhs];
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * (normalized_sum / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data("borrowed MAP group mean is not finite"));
        }
        workspace.group_sums[rhs] = mean;
    }
    Ok(total_weight)
}

fn certify(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    weighted_column_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    certify_with_current_norms(design, weights, residuals, tolerance, workspace)
}

fn certify_initial(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    workspace
        .residual_norms
        .copy_from_slice(&workspace.reference_norms);
    certify_with_current_norms(design, weights, residuals, tolerance, workspace)
}

fn certify_with_current_norms(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    workspace.rhs_relative_infinity_norms.fill(0.0);
    for rhs in 0..residuals.ncols() {
        let scale = workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
        if !scale.is_finite() || scale < 0.0 {
            return Err(invalid_data("borrowed MAP RHS scale is invalid"));
        }
        workspace.rhs_scales[rhs] = scale;
    }

    if design.effects().len() == 1
        && residuals.nrows() >= PARALLEL_PROJECTION_MIN_ROWS
        && rayon::current_num_threads() > 1
        && workspace.parallel_group_weights.len() >= design.effects()[0].n_levels()
        && workspace.parallel_group_means.len()
            >= design.effects()[0]
                .n_levels()
                .checked_mul(residuals.ncols())
                .ok_or_else(|| allocation("borrowed MAP certificate dimensions overflow"))?
    {
        return certify_one_effect_parallel(
            &design.effects()[0],
            weights,
            residuals,
            tolerance,
            workspace,
        );
    }

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP certificate contains an empty final level",
                ));
            }
            let group_weight = group_means(members, weights, residuals, workspace)?;
            let root_group_weight = group_weight.sqrt();
            if !root_group_weight.is_finite() || root_group_weight <= 0.0 {
                return Err(invalid_data(
                    "borrowed MAP certificate group weight is invalid",
                ));
            }
            for rhs in 0..residuals.ncols() {
                let mean = workspace.group_sums[rhs];
                let denominator = workspace.rhs_scales[rhs];
                let relative = if denominator == 0.0 {
                    if mean == 0.0 {
                        0.0
                    } else {
                        return Err(invalid_data(
                            "borrowed MAP zero-scale certificate has a nonzero moment",
                        ));
                    }
                } else {
                    (mean.abs() / denominator) * root_group_weight
                };
                if !relative.is_finite() {
                    return Err(invalid_data("borrowed MAP certificate ratio is not finite"));
                }
                relative_infinity_norm = relative_infinity_norm.max(relative);
                workspace.rhs_relative_infinity_norms[rhs] =
                    workspace.rhs_relative_infinity_norms[rhs].max(relative);
                relative_square_sum = relative.mul_add(relative, relative_square_sum);
                if !relative_square_sum.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP certificate two-norm accumulation is not finite",
                    ));
                }
                relative_count = relative_count
                    .checked_add(1)
                    .ok_or_else(|| allocation("borrowed MAP certificate moment count overflows"))?;

                let normalized_moment = mean.abs() * group_weight;
                let raw_moment = normalized_moment * weights.raw_scale;
                absolute_infinity_norm = absolute_infinity_norm.max(if raw_moment.is_finite() {
                    raw_moment
                } else if normalized_moment == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                });
            }
        }
    }

    let relative_two_norm = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP certificate two norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn certify_one_effect_parallel(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    let levels = effect.n_levels();
    let columns = residuals.ncols();
    workspace.parallel_group_weights[..levels]
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(level, group_weight)| -> Result<()> {
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP certificate contains an empty final level",
                ));
            }
            *group_weight = group_weight_sum(members, weights, residuals.nrows())?;
            Ok(())
        })?;
    let group_weights = &workspace.parallel_group_weights[..levels];
    let means = &mut workspace.parallel_group_means[..levels * columns];
    means
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(slot, mean)| -> Result<()> {
            let level = slot / columns;
            let rhs = slot % columns;
            let members = effect.level_index().checked_members(level)?;
            *mean = group_mean_for_column_with_weight(
                members,
                rhs,
                weights,
                group_weights[level],
                residuals,
            )?;
            Ok(())
        })?;

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for level in 0..levels {
        let group_weight = group_weights[level];
        let root_group_weight = group_weight.sqrt();
        if !root_group_weight.is_finite() || root_group_weight <= 0.0 {
            return Err(invalid_data(
                "borrowed MAP certificate group weight is invalid",
            ));
        }
        for rhs in 0..columns {
            let mean = means[level * columns + rhs];
            let denominator = workspace.rhs_scales[rhs];
            let relative = if denominator == 0.0 {
                if mean == 0.0 {
                    0.0
                } else {
                    return Err(invalid_data(
                        "borrowed MAP zero-scale certificate has a nonzero moment",
                    ));
                }
            } else {
                (mean.abs() / denominator) * root_group_weight
            };
            if !relative.is_finite() {
                return Err(invalid_data("borrowed MAP certificate ratio is not finite"));
            }
            relative_infinity_norm = relative_infinity_norm.max(relative);
            workspace.rhs_relative_infinity_norms[rhs] =
                workspace.rhs_relative_infinity_norms[rhs].max(relative);
            relative_square_sum = relative.mul_add(relative, relative_square_sum);
            if !relative_square_sum.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP certificate two-norm accumulation is not finite",
                ));
            }
            relative_count = relative_count
                .checked_add(1)
                .ok_or_else(|| allocation("borrowed MAP certificate moment count overflows"))?;
            let normalized_moment = mean.abs() * group_weight;
            let raw_moment = normalized_moment * weights.raw_scale;
            absolute_infinity_norm = absolute_infinity_norm.max(if raw_moment.is_finite() {
                raw_moment
            } else if normalized_moment == 0.0 {
                0.0
            } else {
                f64::INFINITY
            });
        }
    }
    let relative_two_norm = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP certificate two norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn borrowed_symmetric_projection_into(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    input: &DenseMatrix,
    residual: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    if residual.nrows() != input.nrows() || residual.ncols() != input.ncols() {
        return Err(invalid_argument(
            "borrowed CG projection buffer dimensions differ",
        ));
    }
    residual.as_mut_slice().copy_from_slice(input.as_slice());
    for effect in design.effects() {
        project_effect(effect, weights, residual, workspace)?;
    }
    if design.effects().len() > 1 {
        for effect in design.effects().iter().rev().skip(1) {
            project_effect(effect, weights, residual, workspace)?;
        }
    }
    for (output, original) in residual.as_mut_slice().iter_mut().zip(input.as_slice()) {
        *output = *original - *output;
        if !output.is_finite() {
            return Err(invalid_data(
                "borrowed CG symmetric projection produced a nonfinite value",
            ));
        }
    }
    Ok(())
}

fn weighted_dot_columns_into(
    left: &DenseMatrix,
    right: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    corrections: &mut [f64],
    parallel_sums: &mut [f64],
) -> Result<()> {
    if left.nrows() != right.nrows()
        || left.ncols() != right.ncols()
        || output.len() != left.ncols()
        || corrections.len() != left.ncols()
    {
        return Err(invalid_argument(
            "borrowed CG weighted-dot dimensions differ",
        ));
    }
    output.fill(0.0);
    corrections.fill(0.0);
    let chunks = left.nrows().div_ceil(PARALLEL_DOT_CHUNK_ROWS);
    let parallel_elements = chunks
        .checked_mul(left.ncols())
        .ok_or_else(|| allocation("borrowed CG parallel dot dimensions overflow"))?;
    if left.nrows() >= PARALLEL_PROJECTION_MIN_ROWS
        && left.ncols() <= FUSED_PARALLEL_MAX_RHS
        && rayon::current_num_threads() > 1
        && parallel_sums.len() >= parallel_elements
    {
        parallel_sums[..parallel_elements]
            .par_chunks_mut(left.ncols())
            .enumerate()
            .try_for_each(|(chunk, sums)| -> Result<()> {
                sums.fill(0.0);
                let mut chunk_corrections = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
                let start = chunk * PARALLEL_DOT_CHUNK_ROWS;
                let end = (start + PARALLEL_DOT_CHUNK_ROWS).min(left.nrows());
                if let Some(weight_values) = weights.values {
                    for row in start..end {
                        let weight = weight_values[row] / weights.raw_scale;
                        for rhs in 0..left.ncols() {
                            let term = weight * left[(row, rhs)] * right[(row, rhs)];
                            if !term.is_finite() {
                                return Err(invalid_data(
                                    "borrowed CG weighted inner product produced a nonfinite term",
                                ));
                            }
                            compensated_add(term, &mut sums[rhs], &mut chunk_corrections[rhs])?;
                        }
                    }
                } else {
                    for row in start..end {
                        for rhs in 0..left.ncols() {
                            let term = left[(row, rhs)] * right[(row, rhs)];
                            if !term.is_finite() {
                                return Err(invalid_data(
                                    "borrowed CG weighted inner product produced a nonfinite term",
                                ));
                            }
                            compensated_add(term, &mut sums[rhs], &mut chunk_corrections[rhs])?;
                        }
                    }
                }
                for rhs in 0..left.ncols() {
                    sums[rhs] += chunk_corrections[rhs];
                    if !sums[rhs].is_finite() {
                        return Err(invalid_data(
                            "borrowed CG weighted inner product is nonfinite",
                        ));
                    }
                }
                Ok(())
            })?;
        for chunk in 0..chunks {
            for rhs in 0..left.ncols() {
                compensated_add(
                    parallel_sums[chunk * left.ncols() + rhs],
                    &mut output[rhs],
                    &mut corrections[rhs],
                )?;
            }
        }
        for rhs in 0..output.len() {
            output[rhs] += corrections[rhs];
            if !output[rhs].is_finite() {
                return Err(invalid_data(
                    "borrowed CG weighted inner product is nonfinite",
                ));
            }
        }
        return Ok(());
    }
    for row in 0..left.nrows() {
        let weight = weights.normalized(row);
        for rhs in 0..left.ncols() {
            let term = weight * left[(row, rhs)] * right[(row, rhs)];
            if !term.is_finite() {
                return Err(invalid_data(
                    "borrowed CG weighted inner product produced a nonfinite term",
                ));
            }
            compensated_add(term, &mut output[rhs], &mut corrections[rhs])?;
        }
    }
    for rhs in 0..output.len() {
        output[rhs] += corrections[rhs];
        if !output[rhs].is_finite() {
            return Err(invalid_data(
                "borrowed CG weighted inner product is nonfinite",
            ));
        }
    }
    Ok(())
}

fn cg_ratios_into(
    numerators: &[f64],
    denominators: &[f64],
    output: &mut [f64],
    message: &'static str,
) -> Result<()> {
    if numerators.len() != denominators.len() || output.len() != numerators.len() {
        return Err(invalid_argument("borrowed CG recurrence dimensions differ"));
    }
    for rhs in 0..output.len() {
        let numerator = numerators[rhs];
        let denominator = denominators[rhs];
        if !numerator.is_finite() || numerator < 0.0 {
            return Err(invalid_data("borrowed CG recurrence numerator is invalid"));
        }
        if numerator == 0.0 {
            output[rhs] = 0.0;
            continue;
        }
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(invalid_data(message));
        }
        let ratio = numerator / denominator;
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(invalid_data("borrowed CG recurrence ratio is invalid"));
        }
        output[rhs] = ratio;
    }
    Ok(())
}

fn cg_hestenes_error_and_update(
    alpha: &[f64],
    residual_square: &[f64],
    improvement_potential: &mut [f64],
) -> Result<f64> {
    if alpha.len() != residual_square.len() || alpha.len() != improvement_potential.len() {
        return Err(invalid_argument(
            "borrowed CG Hestenes recurrence dimensions differ",
        ));
    }
    let mut largest_error = 0.0_f64;
    for rhs in 0..alpha.len() {
        let decrease = alpha[rhs] * residual_square[rhs];
        let previous = improvement_potential[rhs];
        if !decrease.is_finite() || decrease < 0.0 || !previous.is_finite() || previous < 0.0 {
            return Err(invalid_data(
                "borrowed CG Hestenes improvement values are invalid",
            ));
        }
        let remaining = previous - decrease;
        let roundoff_floor = f64::EPSILON * previous.max(1.0);
        if remaining < -roundoff_floor {
            return Err(invalid_data(
                "borrowed CG Hestenes improvement potential became negative",
            ));
        }
        improvement_potential[rhs] = remaining.max(f64::EPSILON);
        let numerator = if decrease < 1e-15 { 0.0 } else { decrease };
        let error = (numerator / improvement_potential[rhs]).sqrt();
        if !error.is_finite() {
            return Err(invalid_data(
                "borrowed CG Hestenes recurrence error is nonfinite",
            ));
        }
        largest_error = largest_error.max(error);
    }
    Ok(largest_error)
}

fn cg_update_solution_and_residual(
    values: &mut DenseMatrix,
    residual: &mut DenseMatrix,
    direction: &DenseMatrix,
    projection: &DenseMatrix,
    alpha: &[f64],
) -> Result<()> {
    if alpha.len() != values.ncols()
        || residual.nrows() != values.nrows()
        || residual.ncols() != values.ncols()
        || direction.nrows() != values.nrows()
        || direction.ncols() != values.ncols()
        || projection.nrows() != values.nrows()
        || projection.ncols() != values.ncols()
    {
        return Err(invalid_argument("borrowed CG update dimensions differ"));
    }
    let columns = values.ncols();
    if values.nrows() >= PARALLEL_PROJECTION_MIN_ROWS && rayon::current_num_threads() > 1 {
        values
            .as_mut_slice()
            .par_chunks_mut(columns)
            .zip(residual.as_mut_slice().par_chunks_mut(columns))
            .enumerate()
            .try_for_each(|(row, (values, residuals))| -> Result<()> {
                for rhs in 0..columns {
                    values[rhs] -= alpha[rhs] * direction[(row, rhs)];
                    residuals[rhs] -= alpha[rhs] * projection[(row, rhs)];
                    if !values[rhs].is_finite() || !residuals[rhs].is_finite() {
                        return Err(invalid_data(
                            "borrowed CG update produced a nonfinite value",
                        ));
                    }
                }
                Ok(())
            })?;
        return Ok(());
    }
    for row in 0..values.nrows() {
        for rhs in 0..columns {
            values[(row, rhs)] -= alpha[rhs] * direction[(row, rhs)];
            residual[(row, rhs)] -= alpha[rhs] * projection[(row, rhs)];
            if !values[(row, rhs)].is_finite() || !residual[(row, rhs)].is_finite() {
                return Err(invalid_data(
                    "borrowed CG update produced a nonfinite value",
                ));
            }
        }
    }
    Ok(())
}

fn cg_update_direction(
    direction: &mut DenseMatrix,
    residual: &DenseMatrix,
    beta: &[f64],
) -> Result<()> {
    if direction.nrows() != residual.nrows()
        || direction.ncols() != residual.ncols()
        || beta.len() != direction.ncols()
    {
        return Err(invalid_argument("borrowed CG direction dimensions differ"));
    }
    if direction.nrows() >= PARALLEL_PROJECTION_MIN_ROWS && rayon::current_num_threads() > 1 {
        let columns = direction.ncols();
        direction
            .as_mut_slice()
            .par_chunks_mut(columns)
            .zip(residual.as_slice().par_chunks(columns))
            .try_for_each(|(direction, residual)| -> Result<()> {
                for rhs in 0..columns {
                    direction[rhs] = residual[rhs] + beta[rhs] * direction[rhs];
                    if !direction[rhs].is_finite() {
                        return Err(invalid_data(
                            "borrowed CG direction update produced a nonfinite value",
                        ));
                    }
                }
                Ok(())
            })?;
        return Ok(());
    }
    for row in 0..direction.nrows() {
        for rhs in 0..direction.ncols() {
            direction[(row, rhs)] = residual[(row, rhs)] + beta[rhs] * direction[(row, rhs)];
            if !direction[(row, rhs)].is_finite() {
                return Err(invalid_data(
                    "borrowed CG direction update produced a nonfinite value",
                ));
            }
        }
    }
    Ok(())
}

fn cg_largest_relative_residual(current: &[f64], initial: &[f64]) -> Result<f64> {
    if current.len() != initial.len() {
        return Err(invalid_argument(
            "borrowed CG residual-norm dimensions differ",
        ));
    }
    let mut largest = 0.0_f64;
    for (current, initial) in current.iter().zip(initial) {
        if !current.is_finite() || *current < 0.0 || !initial.is_finite() || *initial < 0.0 {
            return Err(invalid_data("borrowed CG residual norm is invalid"));
        }
        let relative = if *initial == 0.0 {
            if *current == 0.0 {
                0.0
            } else {
                return Err(invalid_data(
                    "borrowed CG acquired residual from a zero initial norm",
                ));
            }
        } else {
            (current / initial).sqrt()
        };
        if !relative.is_finite() {
            return Err(invalid_data("borrowed CG relative residual is nonfinite"));
        }
        largest = largest.max(relative);
    }
    Ok(largest)
}

fn fallible_matrix_copy(input: &DenseMatrix, label: &'static str) -> Result<DenseMatrix> {
    let mut output = fallible_matrix_zeros(input.nrows(), input.ncols(), label)?;
    output.as_mut_slice().copy_from_slice(input.as_slice());
    Ok(output)
}

fn fallible_matrix_zeros(nrows: usize, ncols: usize, label: &'static str) -> Result<DenseMatrix> {
    DenseMatrix::zeros(nrows, ncols).map_err(|error| {
        if error.code == ErrorCode::Allocation {
            allocation(label)
        } else {
            error
        }
    })
}

fn fallible_copy<T: Copy>(input: &[T], label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| allocation(label))?;
    output.extend_from_slice(input);
    Ok(output)
}

fn fallible_filled<T: Clone>(elements: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(label))?;
    output.resize(elements, value);
    Ok(output)
}

fn weighted_column_norms(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    scales: &mut [f64],
    scaled_square_sums: &mut [f64],
) -> Result<()> {
    if output.len() != values.ncols()
        || scales.len() != values.ncols()
        || scaled_square_sums.len() != values.ncols()
    {
        return Err(invalid_argument(
            "borrowed MAP norm workspace dimensions differ",
        ));
    }
    scales.fill(0.0);
    scaled_square_sums.fill(1.0);
    for row in 0..values.nrows() {
        let root_weight = weights.normalized(row).sqrt();
        if !root_weight.is_finite() || root_weight <= 0.0 {
            return Err(invalid_data(
                "borrowed MAP normalized weight is not finite and positive",
            ));
        }
        for rhs in 0..values.ncols() {
            let value = root_weight * values[(row, rhs)];
            if !value.is_finite() {
                return Err(invalid_data("borrowed MAP weighted value is not finite"));
            }
            update_scaled_sum_of_squares(value, &mut scales[rhs], &mut scaled_square_sums[rhs]);
        }
    }
    for rhs in 0..values.ncols() {
        let norm = if scales[rhs] == 0.0 {
            0.0
        } else {
            scales[rhs] * scaled_square_sums[rhs].sqrt()
        };
        if !norm.is_finite() {
            return Err(invalid_data(
                "borrowed MAP weighted column norm is not finite",
            ));
        }
        output[rhs] = norm;
    }
    Ok(())
}

fn update_scaled_sum_of_squares(value: f64, scale: &mut f64, sum: &mut f64) {
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

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let updated = *sum + value;
    let adjustment = if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *correction += adjustment;
    *sum = updated;
    if !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "borrowed MAP compensated accumulation is not finite",
        ));
    }
    Ok(())
}

fn certified_result(
    residualized: DenseMatrix,
    stop_reason: SolveStopReason,
    iterations: usize,
    certificate: FeCertificate,
    setup_seconds: f64,
    solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    let normal_equation_residual = certificate.relative_two_norm;
    certified_solver_result(
        residualized,
        FeSolverKind::Map,
        stop_reason,
        iterations,
        certificate,
        normal_equation_residual,
        setup_seconds,
        solve_seconds,
    )
}

#[allow(clippy::too_many_arguments)]
fn certified_solver_result(
    residualized: DenseMatrix,
    solver: FeSolverKind,
    stop_reason: SolveStopReason,
    iterations: usize,
    certificate: FeCertificate,
    normal_equation_residual: f64,
    setup_seconds: f64,
    solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    let report = SolveReport {
        requested_solver: solver,
        used_solver: solver,
        iterations,
        stop_reason,
        normal_equation_residual,
        certificate,
        setup_seconds,
        solve_seconds,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    };
    if !report.certified() {
        return Err(invalid_data(
            "borrowed MAP attempted to return an uncertified result",
        ));
    }
    Ok(CertifiedBorrowedMap {
        residualized,
        report,
    })
}

const fn zero_certificate() -> FeCertificate {
    FeCertificate {
        absolute_infinity_norm: 0.0,
        relative_infinity_norm: 0.0,
        relative_two_norm: 0.0,
        passed: true,
    }
}

fn reserved_zeros<R: WorkspaceReservation>(
    elements: usize,
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<f64>> {
    let mut output = Vec::new();
    reservation.reserve_f64(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "borrowed MAP workspace reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, 0.0);
    Ok(output)
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    #[derive(Default)]
    struct InjectedReservation {
        fail_at: Option<usize>,
        skip_at: Option<usize>,
        calls: usize,
        records: Vec<(usize, &'static str)>,
    }

    impl WorkspaceReservation for InjectedReservation {
        fn reserve_f64(
            &mut self,
            target: &mut Vec<f64>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls = self.calls.checked_add(1).expect("small reservation count");
            self.records.push((additional, label));
            if self.fail_at == Some(call) {
                return Err(allocation("injected borrowed MAP allocation failure"));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test borrowed MAP reservation failed"))
        }
    }

    fn config() -> MapConfig {
        MapConfig {
            tolerance: 1e-11,
            max_iterations: 10_000,
            check_every: 1,
            symmetric: true,
        }
    }

    #[test]
    fn every_workspace_reservation_and_capacity_guard_fails_closed() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let columns = [vec![1.0, 2.0, 4.0, 8.0], vec![2.0, 3.0, 1.0, -1.0]];

        let mut successful = InjectedReservation::default();
        absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
            false,
            &mut successful,
        )
        .expect("successful run");
        assert_eq!(successful.calls, 7);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut injected,
            )
            .expect_err("injected allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skip_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut injected,
            )
            .expect_err("undersized allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }

    #[test]
    fn structural_constant_empty_design_allocations_are_fallible_and_only_rhs_scale() {
        let observations = 4096;
        let rhs_count = 3;
        let design =
            BorrowedInterceptFeDesign::new(observations, Vec::new()).expect("empty design");
        let columns = (0..rhs_count)
            .map(|column| {
                (0..observations)
                    .map(|row| (row as f64).sin() + column as f64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let labels = [
            "unable to reserve borrowed MAP reference norms",
            "unable to reserve borrowed MAP residual norms",
            "unable to reserve borrowed MAP RHS scales",
            "unable to reserve borrowed MAP per-RHS certificate norms",
            "unable to reserve borrowed MAP value scales",
            "unable to reserve borrowed MAP group sums",
            "unable to reserve borrowed MAP group corrections",
        ];

        let mut successful = InjectedReservation::default();
        let result = absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("structural RHS"),
            config(),
            true,
            &mut successful,
        )
        .expect("structural constant projection");
        assert!(result.report().certified());
        assert_eq!(result.report().iterations, 1);
        assert_eq!(successful.calls, labels.len());
        assert_eq!(
            successful.records,
            labels
                .into_iter()
                .map(|label| (rhs_count, label))
                .collect::<Vec<_>>()
        );
        assert!(successful
            .records
            .iter()
            .all(|(additional, _)| *additional < observations));

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("structural RHS"),
                config(),
                true,
                &mut injected,
            )
            .expect_err("structural constant allocation must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }
        for skip_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("structural RHS"),
                config(),
                true,
                &mut injected,
            )
            .expect_err("undersized structural workspace must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }

    #[test]
    fn large_parallel_projection_is_bit_stable_and_its_workspace_fails_closed() {
        let observations = PARALLEL_PROJECTION_MIN_ROWS;
        let levels = 4_096;
        let codes = (0..observations)
            .map(|row| (row % levels) as u32)
            .collect::<Vec<_>>();
        let index = LevelIndex::from_codes(&codes, levels).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, levels, &index).expect("source");
        let design =
            BorrowedInterceptFeDesign::new(observations, vec![source.effect()]).expect("design");
        let columns = (0..3)
            .map(|column| {
                (0..observations)
                    .map(|row| {
                        let centered = (row % 97) as f64 - 48.0;
                        centered.mul_add(column as f64 + 0.5, (row % levels) as f64 / 17.0)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let weights = (0..observations)
            .map(|row| 1.0 + (row % 11) as f64 / 13.0)
            .collect::<Vec<_>>();

        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool")
                .install(|| {
                    absorb_borrowed_intercept_map_certified(
                        &design,
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(&columns).expect("matrix"),
                        config(),
                    )
                    .expect("certified projection")
                })
        };
        let serial = run(1);
        let parallel = run(4);
        assert_eq!(serial.residualized(), parallel.residualized());
        assert_eq!(serial.report().iterations, parallel.report().iterations);
        assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
        assert_eq!(serial.report().certificate, parallel.report().certificate);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("pool");
        let mut successful = InjectedReservation::default();
        pool.install(|| {
            absorb_with_reservation(
                &design,
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut successful,
            )
            .expect("parallel reservation succeeds");
        });
        assert_eq!(successful.calls, 10);
        assert_eq!(
            successful.records.last(),
            Some(&(
                observations.div_ceil(PARALLEL_DOT_CHUNK_ROWS) * columns.len(),
                "unable to reserve borrowed CG parallel dot sums",
            )),
        );

        for reservation in 7..successful.calls {
            for skip in [false, true] {
                let mut injected = InjectedReservation {
                    fail_at: (!skip).then_some(reservation),
                    skip_at: skip.then_some(reservation),
                    ..InjectedReservation::default()
                };
                let error = pool
                    .install(|| {
                        absorb_with_reservation(
                            &design,
                            BorrowedMapWeights::Provided(&weights),
                            DenseMatrix::from_columns(&columns).expect("matrix"),
                            config(),
                            false,
                            &mut injected,
                        )
                    })
                    .expect_err("parallel workspace allocation must fail closed");
                assert_eq!(error.code, ErrorCode::Allocation);
            }
        }
    }

    #[test]
    fn success_is_always_certified_and_nonconvergence_is_fatal() {
        let first_codes = [0_u32, 0, 0, 1, 1, 1];
        let second_codes = [0_u32, 0, 1, 0, 1, 1];
        let first_index = LevelIndex::from_codes(&first_codes, 2).expect("first index");
        let second_index = LevelIndex::from_codes(&second_codes, 2).expect("second index");
        let first =
            ValidatedBorrowedInterceptSource::new(&first_codes, 2, &first_index).expect("first");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).expect("second");
        let design = BorrowedInterceptFeDesign::new(6, vec![first.effect(), second.effect()])
            .expect("design");
        let columns = [vec![1.0, -3.0, 2.0, 5.0, -4.0, 0.5]];

        let success = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
        )
        .expect("certified");
        assert!(success.report().certified());

        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            MapConfig {
                tolerance: 1e-16,
                max_iterations: 1,
                check_every: 1,
                symmetric: false,
            },
        )
        .expect_err("uncertified solve must be fatal");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn empty_nonfinite_and_invalid_weight_inputs_fail_before_workspace_allocation() {
        let design = BorrowedInterceptFeDesign::new(0, Vec::new()).expect("empty design");
        let mut reservation = InjectedReservation::default();
        let error = absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::zeros(0, 1).expect("matrix"),
            config(),
            false,
            &mut reservation,
        )
        .expect_err("empty rows");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(reservation.calls, 0);

        let codes = [0_u32, 0];
        let index = LevelIndex::from_codes(&codes, 1).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let error = absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Provided(&[1.0, bad]),
                DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
                config(),
            )
            .expect_err("invalid weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[f64::MAX, f64::MIN_POSITIVE]),
            DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
            config(),
        )
        .expect_err("normalized underflow");
        assert_eq!(error.code, ErrorCode::InvalidData);

        for bad in [f64::NAN, f64::INFINITY] {
            let error = absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&[vec![1.0, bad]]).expect("matrix"),
                config(),
            )
            .expect_err("invalid value");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn no_effects_and_already_certified_inputs_return_without_iteration() {
        let design = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0]]).expect("matrix"),
            config(),
        )
        .expect("no effects");
        assert_eq!(result.report().stop_reason, SolveStopReason::NoFixedEffects);
        assert_eq!(result.report().iterations, 0);

        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![-1.0, 1.0, -2.0, 2.0]]).expect("matrix"),
            config(),
        )
        .expect("already certified");
        assert_eq!(result.report().iterations, 0);
        assert!(result.report().certified());

        let zero = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![0.0; 4]]).expect("zero matrix"),
            config(),
        )
        .expect("zero RHS");
        assert_eq!(zero.report().iterations, 0);
        assert_eq!(zero.report().certificate.relative_infinity_norm, 0.0);
    }

    #[test]
    fn one_way_projection_is_exact_and_invalid_shapes_and_configurations_fail() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 3.0, 2.0, 2.0]),
            DenseMatrix::from_columns(&[vec![1.0, 5.0, 4.0, 8.0]]).expect("matrix"),
            config(),
        )
        .expect("one-way projection");
        assert_eq!(result.report().iterations, 1);
        assert_close(
            result.residualized().as_slice(),
            &[-3.0, 1.0, -2.0, 2.0],
            1e-14,
        );

        let short = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("short design");
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &short,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                config(),
            )
            .expect_err("dimension mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Provided(&[1.0; 3]),
                DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                config(),
            )
            .expect_err("weight mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::zeros(4, 0).expect("empty RHS matrix"),
                config(),
            )
            .expect_err("empty RHS")
            .code,
            ErrorCode::InvalidData
        );

        for bad in [
            MapConfig {
                tolerance: 0.0,
                ..config()
            },
            MapConfig {
                tolerance: f64::NAN,
                ..config()
            },
            MapConfig {
                max_iterations: 0,
                ..config()
            },
            MapConfig {
                check_every: 0,
                ..config()
            },
        ] {
            assert_eq!(
                absorb_borrowed_intercept_map_certified(
                    &design,
                    BorrowedMapWeights::Unit,
                    DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                    bad,
                )
                .expect_err("invalid config")
                .code,
                ErrorCode::InvalidArgument
            );
        }
    }

    #[test]
    fn nonrepresentable_weighted_norm_fails_closed() {
        let codes = [0_u32, 0];
        let index = LevelIndex::from_codes(&codes, 1).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![f64::MAX, f64::MAX]]).expect("matrix"),
            config(),
        )
        .expect_err("overflowing weighted norm");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn borrowed_cg_is_weighted_batched_certified_and_deterministic() {
        let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
        let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
        let worker =
            ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
        let firm =
            ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
        let design = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("design");
        let weights = [1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25];
        let columns = [
            vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0],
            vec![2.0, -1.0, 4.0, 5.0, 0.0, 6.0, 3.0, 10.0],
        ];
        let run = || {
            absorb_borrowed_intercept_cg_certified(
                &design,
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
            )
            .expect("CG")
        };
        let first = run();
        let second = run();
        assert!(first.report().certified());
        assert_eq!(first.report().used_solver, FeSolverKind::Cg);
        assert_eq!(first.residualized(), second.residualized());
        assert_eq!(first.report().iterations, second.report().iterations);

        let map = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
        )
        .expect("MAP");
        assert_close(
            first.residualized().as_slice(),
            map.residualized().as_slice(),
            2e-10,
        );
    }

    #[test]
    fn borrowed_cg_breakdown_and_exhaustion_fail_closed() {
        let mut output = [0.0, 0.0];
        let error = cg_ratios_into(
            &[1.0, 0.0],
            &[0.0, 0.0],
            &mut output,
            "injected denominator breakdown",
        )
        .expect_err("breakdown");
        assert_eq!(error.code, ErrorCode::InvalidData);

        let mut improvement = [10.0, 20.0];
        let error = cg_hestenes_error_and_update(&[0.5, 0.25], &[4.0, 8.0], &mut improvement)
            .expect("Hestenes update");
        assert!((error - 0.5).abs() <= f64::EPSILON);
        assert_eq!(improvement, [8.0, 18.0]);
        assert_eq!(
            cg_hestenes_error_and_update(&[2.0], &[1.0], &mut [1.0])
                .expect_err("negative improvement potential")
                .code,
            ErrorCode::InvalidData
        );

        let first_codes = [0_u32, 0, 0, 1, 1, 1];
        let second_codes = [0_u32, 0, 1, 0, 1, 1];
        let first_index = LevelIndex::from_codes(&first_codes, 2).expect("first index");
        let second_index = LevelIndex::from_codes(&second_codes, 2).expect("second index");
        let first =
            ValidatedBorrowedInterceptSource::new(&first_codes, 2, &first_index).expect("first");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).expect("second");
        let design = BorrowedInterceptFeDesign::new(
            first_codes.len(),
            vec![first.effect(), second.effect()],
        )
        .expect("design");
        let error = absorb_borrowed_intercept_cg_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0]]).expect("matrix"),
            MapConfig {
                tolerance: 1e-16,
                max_iterations: 1,
                check_every: 1,
                symmetric: true,
            },
        )
        .expect_err("iteration exhaustion");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn borrowed_auto_uses_direct_one_fe_early_map_and_cg_routes() {
        let one_codes = [0_u32, 0, 1, 1];
        let one_index = LevelIndex::from_codes(&one_codes, 2).expect("one index");
        let one = ValidatedBorrowedInterceptSource::new(&one_codes, 2, &one_index).expect("one");
        let one_design = BorrowedInterceptFeDesign::new(4, vec![one.effect()]).expect("one design");
        let direct = absorb_borrowed_intercept_auto_certified(
            &one_design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 3.0, 4.0, 10.0]]).expect("one matrix"),
            config(),
        )
        .expect("direct auto");
        assert_eq!(direct.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(direct.report().used_solver, FeSolverKind::Map);
        assert_eq!(direct.report().pilot_iterations, 0);

        let row_codes = [0_u32, 0, 1, 1];
        let column_codes = [0_u32, 1, 0, 1];
        let row_index = LevelIndex::from_codes(&row_codes, 2).expect("row index");
        let column_index = LevelIndex::from_codes(&column_codes, 2).expect("column index");
        let row = ValidatedBorrowedInterceptSource::new(&row_codes, 2, &row_index).expect("row");
        let column =
            ValidatedBorrowedInterceptSource::new(&column_codes, 2, &column_index).expect("column");
        let orthogonal = BorrowedInterceptFeDesign::new(4, vec![row.effect(), column.effect()])
            .expect("orthogonal design");
        let early = absorb_borrowed_intercept_auto_certified(
            &orthogonal,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 4.0, 8.0]]).expect("early matrix"),
            config(),
        )
        .expect("early MAP");
        assert_eq!(early.report().used_solver, FeSolverKind::Map);
        assert_eq!(early.report().pilot_iterations, 1);

        let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
        let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
        let worker =
            ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
        let firm =
            ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
        let hard = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("hard design");
        let cg = absorb_borrowed_intercept_auto_certified(
            &hard,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25]),
            DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0]])
                .expect("CG matrix"),
            MapConfig {
                tolerance: 1e-14,
                ..config()
            },
        )
        .expect("CG auto");
        assert_eq!(cg.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(cg.report().used_solver, FeSolverKind::Cg);
        assert_eq!(cg.report().pilot_iterations, 4);
        assert!(cg.report().pilot_contraction.is_finite());
        assert!(cg.report().certified());
    }

    #[test]
    fn routed_auto_selects_lsmr_for_a_slow_chain_graph() {
        let workers = 160_usize;
        let mut worker_codes = Vec::with_capacity(workers * 2);
        let mut firm_codes = Vec::with_capacity(workers * 2);
        let mut outcome = Vec::with_capacity(workers * 2);
        for worker in 0..workers {
            worker_codes.extend([worker as u32, worker as u32]);
            firm_codes.extend([worker as u32, (worker + 1) as u32]);
            outcome.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
        }
        let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
        let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
            .expect("worker");
        let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index)
            .expect("firm");
        let design = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("design");
        let result = absorb_borrowed_intercept_auto_routed_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[outcome]).expect("matrix"),
            config(),
            LsmrConfig::default(),
        )
        .expect("routed auto");
        assert_eq!(result.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(result.report().used_solver, FeSolverKind::Lsmr);
        assert_eq!(result.report().pilot_iterations, 4);
        assert!(result.report().pilot_contraction >= 0.008);
        assert!(result.report().certified());
    }

    fn assert_close(left: &[f64], right: &[f64], tolerance: f64) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right) {
            assert!((left - right).abs() <= tolerance, "{left} != {right}");
        }
    }
}
