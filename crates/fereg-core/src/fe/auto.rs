use crate::data::DenseMatrix;
use crate::error::Result;
use crate::fe::{
    absorb_lsmr, absorb_map, FeDesign, FeSolverKind, LsmrConfig, MapConfig, SolveReport,
};

/// Unified fixed-effect absorption configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsorbConfig {
    pub solver: FeSolverKind,
    pub map: MapConfig,
    pub lsmr: LsmrConfig,
    /// Whether `Auto` may retry the other certified solver after a failed certificate.
    pub certified_fallback: bool,
}

impl Default for AbsorbConfig {
    fn default() -> Self {
        Self {
            solver: FeSolverKind::Auto,
            map: MapConfig::default(),
            lsmr: LsmrConfig::default(),
            certified_fallback: true,
        }
    }
}

/// Residualize with explicit solver selection or deterministic automatic routing.
pub fn absorb_fixed_effects(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<SolveReport> {
    match config.solver {
        FeSolverKind::Map => absorb_map(design, weights, values, config.map),
        FeSolverKind::Lsmr => absorb_lsmr(design, weights, values, config.lsmr),
        FeSolverKind::Auto => absorb_auto(design, weights, values, config),
    }
}

fn absorb_auto(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<SolveReport> {
    let first = route(design);
    let original = config.certified_fallback.then(|| values.clone());
    let mut report = run(first, design, weights, values, config)?;
    report.requested_solver = FeSolverKind::Auto;
    if report.certified() || !config.certified_fallback {
        return Ok(report);
    }

    let second = match first {
        FeSolverKind::Map => FeSolverKind::Lsmr,
        FeSolverKind::Lsmr => FeSolverKind::Map,
        FeSolverKind::Auto => unreachable!("router never returns Auto"),
    };
    if let Some(original) = original {
        *values = original;
    }
    let first_summary = format!(
        "automatic {:?} path did not certify after {} iterations; retried {:?}",
        first, report.iterations, second
    );
    let mut fallback = run(second, design, weights, values, config)?;
    fallback.requested_solver = FeSolverKind::Auto;
    fallback.fallback_used = true;
    fallback.warnings.insert(0, first_summary);
    Ok(fallback)
}

fn run(
    solver: FeSolverKind,
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<SolveReport> {
    match solver {
        FeSolverKind::Map => absorb_map(design, weights, values, config.map),
        FeSolverKind::Lsmr => absorb_lsmr(design, weights, values, config.lsmr),
        FeSolverKind::Auto => unreachable!("Auto is resolved before execution"),
    }
}

fn route(design: &FeDesign) -> FeSolverKind {
    let has_slopes = design
        .effects()
        .iter()
        .any(|effect| !effect.slopes().is_empty());
    if design.effects().len() <= 1 {
        return FeSolverKind::Map;
    }
    if has_slopes || design.effects().len() >= 3 || design.total_coefficients() > 100_000 {
        FeSolverKind::Lsmr
    } else {
        FeSolverKind::Map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{EffectTerm, SolveStopReason};

    #[test]
    fn auto_routes_slope_design_to_lsmr() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![vec![0.0, 1.0, 0.0, 1.0]])
            .expect("effect");
        let other = EffectTerm::new(vec![0, 1, 0, 1], 2, true, vec![]).expect("other");
        let design = FeDesign::new(4, vec![effect, other]).expect("design");
        let mut values = DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("values");
        let report = absorb_fixed_effects(&design, &[1.0; 4], &mut values, AbsorbConfig::default())
            .expect("absorb");
        assert_eq!(report.requested_solver, FeSolverKind::Auto);
        assert_eq!(report.used_solver, FeSolverKind::Lsmr);
        assert!(matches!(report.stop_reason, SolveStopReason::Certified));
    }

    #[test]
    fn auto_falls_back_when_map_budget_cannot_certify() {
        let first = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("first");
        let second = EffectTerm::new(vec![0, 1, 1, 2, 2, 0], 3, true, vec![]).expect("second");
        let design = FeDesign::new(6, vec![first, second]).expect("design");
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0]]).expect("values");
        let report = absorb_fixed_effects(
            &design,
            &[1.0; 6],
            &mut values,
            AbsorbConfig {
                map: MapConfig {
                    max_iterations: 1,
                    check_every: 1,
                    tolerance: 1e-12,
                    symmetric: true,
                },
                lsmr: LsmrConfig {
                    tolerance: 1e-10,
                    ..LsmrConfig::default()
                },
                ..AbsorbConfig::default()
            },
        )
        .expect("absorb");
        assert!(report.fallback_used);
        assert_eq!(report.used_solver, FeSolverKind::Lsmr);
        assert!(report.certified());
    }
}
