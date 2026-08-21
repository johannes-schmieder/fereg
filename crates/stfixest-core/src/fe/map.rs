use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::ProjectionWorkspace;
use crate::fe::{certify_residuals, project_effect_in_place, FeCertificate, FeDesign};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub check_every: usize,
    pub symmetric: bool,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 16_000,
            check_every: 4,
            symmetric: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveStopReason {
    NoFixedEffects,
    Certified,
    MaximumIterations,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    pub iterations: usize,
    pub stop_reason: SolveStopReason,
    pub certificate: FeCertificate,
}

/// Residualize one or more right-hand sides with weighted symmetric MAP.
pub fn absorb_map(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: MapConfig,
) -> Result<SolveReport> {
    validate_config(config)?;
    if design.nobs() != values.nrows() || weights.len() != values.nrows() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "MAP inputs have inconsistent observation counts",
        ));
    }
    if design.effects().is_empty() {
        let certificate = certify_residuals(design, weights, values, config.tolerance)?;
        return Ok(SolveReport {
            iterations: 0,
            stop_reason: SolveStopReason::NoFixedEffects,
            certificate,
        });
    }
    let mut workspace = ProjectionWorkspace::new(design.max_channels(), values.ncols())?;
    let mut final_certificate = certify_residuals(design, weights, values, config.tolerance)?;

    for iteration in 1..=config.max_iterations {
        for effect in design.effects() {
            project_effect_in_place(effect, weights, values, &mut workspace)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                project_effect_in_place(effect, weights, values, &mut workspace)?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            final_certificate = certify_residuals(design, weights, values, config.tolerance)?;
            if final_certificate.passed {
                return Ok(SolveReport {
                    iterations: iteration,
                    stop_reason: SolveStopReason::Certified,
                    certificate: final_certificate,
                });
            }
        }
    }

    Ok(SolveReport {
        iterations: config.max_iterations,
        stop_reason: SolveStopReason::MaximumIterations,
        certificate: final_certificate,
    })
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "MAP iteration and check budgets must be positive",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn absorbs_two_way_intercepts_and_certifies() {
        let worker = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("worker");
        let firm = EffectTerm::new(vec![0, 1, 0, 1, 0, 1], 2, true, vec![]).expect("firm");
        let design = FeDesign::new(6, vec![worker, firm]).expect("design");
        let weights = vec![1.0; 6];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 2.0, 3.0, 4.0, 5.0]]).expect("values");
        let report =
            absorb_map(&design, &weights, &mut values, MapConfig::default()).expect("absorb");
        assert_eq!(report.stop_reason, SolveStopReason::Certified);
        assert!(report.certificate.passed);
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-8));
    }
}
