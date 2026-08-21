use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{prune_singletons, FeDesign};
use crate::ppml_separation::{
    DetectionResult, MethodStatus, SeparationConfig, SeparationDropReason, SeparationMethod,
    SeparationMethodReport,
};
use crate::ppml_separation::fe::zero_only_fixed_effect_levels;

pub(super) fn validate_detection_inputs(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    config: SeparationConfig,
) -> Result<()> {
    if outcome.len() != regressors.nrows()
        || fixed_effects.is_some_and(|design| design.nobs() != outcome.len())
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "separation inputs have inconsistent observation counts",
        ));
    }
    if outcome.iter().any(|value| !value.is_finite() || *value < 0.0)
        || regressors.as_slice().iter().any(|value| !value.is_finite())
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "separation requires finite nonnegative outcomes and finite regressors",
        ));
    }
    for value in [
        config.tolerance,
        config.certificate_tolerance,
        config.projection_tolerance,
        config.mu_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 || value >= 1.0 {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "separation tolerances must lie strictly between zero and one",
            ));
        }
    }
    if config.max_ir_iterations == 0
        || config.simplex_max_iterations == 0
        || config.simplex_max_tableau_cells == 0
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "separation iteration and memory budgets must be positive",
        ));
    }
    Ok(())
}

pub(super) fn apply_exact_fe_fixed_point(
    outcome: &[f64],
    fixed_effects: Option<&FeDesign>,
    active: &mut [bool],
    reasons: &mut [Option<SeparationDropReason>],
    drop_singletons: bool,
) -> Result<SeparationMethodReport> {
    let mut direct_drops = Vec::new();
    let mut singleton_drops = Vec::new();
    let mut iterations = 0_usize;
    loop {
        let dropped = zero_only_fixed_effect_levels(outcome, fixed_effects, active);
        if dropped.is_empty() {
            break;
        }
        iterations = iterations.checked_add(1).ok_or_else(|| {
            StFixestError::new(ErrorCode::Internal, "FE separation iteration count overflowed")
        })?;
        let report = apply_detection(
            SeparationMethod::FixedEffects,
            DetectionResult {
                status: MethodStatus::Certified,
                dropped,
                iterations: 1,
                certificate: None,
                note: "exact zero-only intercept fixed-effect levels".to_owned(),
            },
            SeparationDropReason::FixedEffectZeroLevel,
            active,
            reasons,
            fixed_effects,
            drop_singletons,
        )?;
        if report.dropped_observations.is_empty() {
            return Err(StFixestError::new(
                ErrorCode::Internal,
                "FE separation fixed point made no progress",
            ));
        }
        direct_drops.extend(report.dropped_observations);
        singleton_drops.extend(report.singleton_drops);
    }
    Ok(SeparationMethodReport {
        method: SeparationMethod::FixedEffects,
        status: if direct_drops.is_empty() {
            MethodStatus::NoSeparation
        } else {
            MethodStatus::Certified
        },
        iterations,
        dropped_observations: direct_drops,
        singleton_drops,
        certificate: None,
        note: "exact zero-only intercept FE levels iterated with singleton cascades to a fixed point"
            .to_owned(),
    })
}

pub(super) fn append_exact_fe_followup(
    outcome: &[f64],
    fixed_effects: Option<&FeDesign>,
    active: &mut [bool],
    reasons: &mut [Option<SeparationDropReason>],
    config: SeparationConfig,
    reports: &mut Vec<SeparationMethodReport>,
) -> Result<()> {
    if !config.methods.fixed_effects {
        return Ok(());
    }
    let report = apply_exact_fe_fixed_point(
        outcome,
        fixed_effects,
        active,
        reasons,
        config.drop_singletons,
    )?;
    if !report.dropped_observations.is_empty() || !report.singleton_drops.is_empty() {
        reports.push(report);
    }
    Ok(())
}

pub(super) fn apply_detection(
    method: SeparationMethod,
    detection: DetectionResult,
    reason: SeparationDropReason,
    active: &mut [bool],
    reasons: &mut [Option<SeparationDropReason>],
    fixed_effects: Option<&FeDesign>,
    drop_singletons: bool,
) -> Result<SeparationMethodReport> {
    let mut actually_dropped = Vec::new();
    for row in detection.dropped {
        if row >= active.len() {
            return Err(StFixestError::new(
                ErrorCode::Internal,
                "separation detector returned an out-of-range observation",
            ));
        }
        if active[row] {
            active[row] = false;
            reasons[row] = Some(reason);
            actually_dropped.push(row);
        }
    }
    let singleton_drops = if actually_dropped.is_empty() {
        Vec::new()
    } else {
        apply_singletons(
            active,
            reasons,
            fixed_effects,
            drop_singletons,
            SeparationDropReason::SingletonCascade,
        )?
    };
    Ok(SeparationMethodReport {
        method,
        status: detection.status,
        iterations: detection.iterations,
        dropped_observations: actually_dropped,
        singleton_drops,
        certificate: detection.certificate,
        note: detection.note,
    })
}

pub(super) fn apply_singletons(
    active: &mut [bool],
    reasons: &mut [Option<SeparationDropReason>],
    fixed_effects: Option<&FeDesign>,
    drop_singletons: bool,
    reason: SeparationDropReason,
) -> Result<Vec<usize>> {
    let Some(design) = fixed_effects else {
        return Ok(Vec::new());
    };
    if !drop_singletons || design.effects().is_empty() {
        return Ok(Vec::new());
    }
    let codes = design
        .effects()
        .iter()
        .map(|effect| effect.codes())
        .collect::<Vec<_>>();
    let levels = design
        .effects()
        .iter()
        .map(|effect| effect.n_levels())
        .collect::<Vec<_>>();
    let pruned = prune_singletons(&codes, &levels, active, false)?;
    for &row in &pruned.dropped_observations {
        if active[row] {
            reasons[row] = Some(reason);
        }
    }
    active.copy_from_slice(&pruned.active);
    Ok(pruned.dropped_observations)
}

pub(super) fn map_detection_to_original(
    mut detection: DetectionResult,
    original_rows: &[usize],
) -> Result<DetectionResult> {
    detection.dropped = detection
        .dropped
        .into_iter()
        .map(|row| {
            original_rows.get(row).copied().ok_or_else(|| {
                StFixestError::new(
                    ErrorCode::Internal,
                    "separation detector returned an out-of-range subset observation",
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(detection)
}

pub(super) fn has_active_boundary(outcome: &[f64], active: &[bool]) -> bool {
    outcome
        .iter()
        .zip(active)
        .any(|(outcome, active)| *active && *outcome == 0.0)
}
