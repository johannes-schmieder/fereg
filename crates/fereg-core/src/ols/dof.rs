use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{connected_components, FeDesign};

/// Policy for counting absorbed fixed-effect dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DofMethod {
    /// Exact for zero, one, and two intercept-only FE dimensions; conservative
    /// upper-rank accounting for higher-way and slope designs.
    Default,
    /// Use the conservative coefficient-slot upper bound for every FE design.
    Conservative,
    /// Do not subtract absorbed fixed-effect dimensions.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DofConfig {
    pub method: DofMethod,
}

impl Default for DofConfig {
    fn default() -> Self {
        Self {
            method: DofMethod::Default,
        }
    }
}

/// Centralized finite and absorbed degrees-of-freedom accounting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofReport {
    pub observations: usize,
    pub finite_rank: usize,
    pub absorbed_rank: usize,
    pub model_rank: usize,
    pub residual: usize,
    pub connected_components: usize,
    pub absorbed_rank_is_approximate: bool,
}

pub fn compute_dof(
    observations: usize,
    finite_rank: usize,
    fixed_effects: Option<&FeDesign>,
    config: DofConfig,
) -> Result<DofReport> {
    if finite_rank > observations {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "finite rank exceeds observation count",
        ));
    }
    let (absorbed_rank, components, approximate) = match (fixed_effects, config.method) {
        (_, DofMethod::None) | (None, _) => (0, 0, false),
        (Some(design), method) => absorbed_rank(design, method)?,
    };
    let model_rank = finite_rank
        .checked_add(absorbed_rank)
        .ok_or_else(|| FeregError::new(ErrorCode::InvalidData, "model rank overflowed usize"))?;
    if model_rank >= observations {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            format!(
                "model has no positive residual degrees of freedom: N={observations}, rank={model_rank}"
            ),
        ));
    }
    Ok(DofReport {
        observations,
        finite_rank,
        absorbed_rank,
        model_rank,
        residual: observations - model_rank,
        connected_components: components,
        absorbed_rank_is_approximate: approximate,
    })
}

fn absorbed_rank(design: &FeDesign, method: DofMethod) -> Result<(usize, usize, bool)> {
    if design.effects().is_empty() {
        return Ok((0, 0, false));
    }
    let mut total_slots = 0_usize;
    let mut intercept_effects = 0_usize;
    let mut has_slopes = false;
    for effect in design.effects() {
        let mut observed = vec![false; effect.n_levels()];
        for code in effect.codes() {
            observed[*code as usize] = true;
        }
        let count = observed.into_iter().filter(|value| *value).count();
        total_slots = total_slots
            .checked_add(count.saturating_mul(effect.channels()))
            .ok_or_else(|| {
                FeregError::new(ErrorCode::InvalidData, "absorbed rank overflowed usize")
            })?;
        intercept_effects += if effect.has_intercept() { 1 } else { 0 };
        has_slopes |= !effect.slopes().is_empty();
    }

    let codes = design
        .effects()
        .iter()
        .map(|effect| effect.codes())
        .collect::<Vec<_>>();
    let declared_levels = design
        .effects()
        .iter()
        .map(|effect| effect.n_levels())
        .collect::<Vec<_>>();
    let components =
        connected_components(&codes, &declared_levels, &vec![true; design.nobs()])?.n_components;
    let dependencies = intercept_effects
        .saturating_sub(1)
        .saturating_mul(components);
    let conservative_rank = total_slots.saturating_sub(dependencies);

    let exact_intercept_only = !has_slopes && intercept_effects == design.effects().len();
    let approximate = match method {
        DofMethod::None => false,
        DofMethod::Conservative => has_slopes || design.effects().len() > 2,
        DofMethod::Default => !exact_intercept_only || design.effects().len() > 2,
    };
    Ok((conservative_rank, components, approximate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn two_way_intercept_rank_uses_components() {
        let worker =
            EffectTerm::new(vec![0, 1, 2, 3, 0, 1, 2, 3], 4, true, vec![]).expect("worker");
        let firm = EffectTerm::new(vec![0, 0, 1, 1, 0, 0, 1, 1], 2, true, vec![]).expect("firm");
        let design = FeDesign::new(8, vec![worker, firm]).expect("design");
        let report = compute_dof(8, 1, Some(&design), DofConfig::default()).expect("dof");
        assert_eq!(report.connected_components, 2);
        assert_eq!(report.absorbed_rank, 4);
        assert_eq!(report.residual, 3);
        assert!(!report.absorbed_rank_is_approximate);
    }

    #[test]
    fn slope_rank_is_marked_approximate() {
        let effect = EffectTerm::new(
            vec![0, 0, 1, 1, 2, 2],
            3,
            true,
            vec![vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0]],
        )
        .expect("effect");
        let design = FeDesign::new(6, vec![effect]).expect("design");
        let report = compute_dof(10, 1, Some(&design), DofConfig::default()).expect("dof");
        assert_eq!(report.absorbed_rank, 6);
        assert!(report.absorbed_rank_is_approximate);
    }
}
