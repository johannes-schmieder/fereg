use super::accounting::{
    append_exact_fe_followup, apply_detection, apply_exact_fe_fixed_point, apply_singletons,
    has_active_boundary, map_detection_to_original, validate_detection_inputs,
};
use crate::data::DenseMatrix;
use crate::error::{Result, StFixestError};
use crate::fe::FeDesign;
use crate::ppml::{PpmlConfig, PpmlStart};
use crate::ppml_separation::{
    DetectionResult, MethodStatus, SeparationConfig, SeparationDropReason, SeparationMethod,
    SeparationMethodReport, SeparationReport,
};
use crate::ppml_separation::fe::zero_only_fixed_effect_levels;
use crate::ppml_separation::ir::iterated_rectifier;
use crate::ppml_separation::mu::detect_mu;
use crate::ppml_separation::projection::ProjectionConfig;
use crate::ppml_separation::simplex::finite_simplex;
use crate::ppml_separation::subset::subset_problem;

pub fn detect_separation(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    ppml: PpmlConfig,
    config: SeparationConfig,
    start: Option<&PpmlStart>,
) -> Result<SeparationReport> {
    validate_detection_inputs(outcome, regressors, fixed_effects, config)?;
    let mut active = vec![true; outcome.len()];
    let mut reasons = vec![None; outcome.len()];
    let mut reports = Vec::new();

    let initial_singletons = apply_singletons(
        &mut active,
        &mut reasons,
        fixed_effects,
        config.drop_singletons,
        SeparationDropReason::InitialSingleton,
    )?;
    if !initial_singletons.is_empty() {
        reports.push(SeparationMethodReport {
            method: SeparationMethod::Singletons,
            status: MethodStatus::Certified,
            iterations: 0,
            dropped_observations: Vec::new(),
            singleton_drops: initial_singletons,
            certificate: None,
            note: "initial singleton fixed point".to_owned(),
        });
    }

    if config.methods.fixed_effects {
        reports.push(apply_exact_fe_fixed_point(
            outcome,
            fixed_effects,
            &mut active,
            &mut reasons,
            config.drop_singletons,
        )?);
    }

    if config.methods.simplex && has_active_boundary(outcome, &active) {
        let subset = subset_problem(outcome, regressors, fixed_effects, &active)?;
        let detected = finite_simplex(
            &subset.outcome,
            &subset.regressors,
            subset.fixed_effects.as_ref(),
            config.tolerance,
            config.certificate_tolerance,
            config.simplex_max_iterations,
            config.simplex_max_tableau_cells,
        )?;
        let detected = map_detection_to_original(detected, &subset.original_rows)?;
        reports.push(apply_detection(
            SeparationMethod::Simplex,
            detected,
            SeparationDropReason::Simplex,
            &mut active,
            &mut reasons,
            fixed_effects,
            config.drop_singletons,
        )?);
        append_exact_fe_followup(
            outcome,
            fixed_effects,
            &mut active,
            &mut reasons,
            config,
            &mut reports,
        )?;
    }

    if config.methods.iterated_rectifier && has_active_boundary(outcome, &active) {
        let subset = subset_problem(outcome, regressors, fixed_effects, &active)?;
        let projection = ProjectionConfig {
            absorb: ppml.absorb,
            finite_solver: ppml.finite_solver,
            rank_tolerance: ppml.rank_tolerance,
            collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            certificate_tolerance: config.projection_tolerance,
        };
        let detected = iterated_rectifier(
            &subset.outcome,
            &subset.regressors,
            subset.fixed_effects.as_ref(),
            projection,
            config.tolerance,
            config.certificate_tolerance,
            config.max_ir_iterations,
            config.strict,
        )?;
        let detected = map_detection_to_original(detected, &subset.original_rows)?;
        reports.push(apply_detection(
            SeparationMethod::IteratedRectifier,
            detected,
            SeparationDropReason::IteratedRectifier,
            &mut active,
            &mut reasons,
            fixed_effects,
            config.drop_singletons,
        )?);
        append_exact_fe_followup(
            outcome,
            fixed_effects,
            &mut active,
            &mut reasons,
            config,
            &mut reports,
        )?;
    }

    if config.methods.mu && has_active_boundary(outcome, &active) {
        let subset = subset_problem(outcome, regressors, fixed_effects, &active)?;
        let detected = detect_mu(
            &subset,
            ppml,
            config,
            start,
            &active,
        )?;
        let detected = map_detection_to_original(detected, &subset.original_rows)?;
        reports.push(apply_detection(
            SeparationMethod::Mu,
            detected,
            SeparationDropReason::Mu,
            &mut active,
            &mut reasons,
            fixed_effects,
            config.drop_singletons,
        )?);
        append_exact_fe_followup(
            outcome,
            fixed_effects,
            &mut active,
            &mut reasons,
            config,
            &mut reports,
        )?;
    }

    let retained_observations = active
        .iter()
        .enumerate()
        .filter_map(|(row, keep)| (*keep).then_some(row))
        .collect::<Vec<_>>();
    if retained_observations.is_empty() {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "separation and singleton removal deleted the full sample",
        ));
    }
    let dropped_observations = active
        .iter()
        .enumerate()
        .filter_map(|(row, keep)| (!*keep).then_some(row))
        .collect();
    Ok(SeparationReport {
        active,
        retained_observations,
        dropped_observations,
        drop_reasons: reasons,
        methods: reports,
    })
}

