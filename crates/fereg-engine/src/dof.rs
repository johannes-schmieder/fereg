//! SPEC §9 policy harvested from legacy topology_dof.rs; one topology for every channel.
mod residual;
use crate::{
    data::Prepared,
    error::{copied_string, reserved, ErrorCode, FeregError, Result},
    topology::{connected_components_all, Effect, FeTopology, Partition},
    AbsorbTermId, SourceId,
};
pub use residual::ResidualDof;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DofPolicy {
    None,
    FirstPair,
    #[default]
    Pairwise,
    Continuous,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectIdentity {
    Explicit {
        term_id: AbsorbTermId,
        sources: Vec<SourceId>,
    },
    ImplicitNoAbsorb,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofEffect {
    pub identity: EffectIdentity,
    pub label: String,
    pub categories: usize,
    pub redundant: usize,
    pub coefficients: usize,
    pub redundancy_exact: bool,
    pub nested: bool,
    pub slope_source: Option<SourceId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofReport {
    pub policy: DofPolicy,
    pub effects: Vec<DofEffect>,
    pub initial_categories: usize,
    pub redundant_categories: usize,
    pub absorbed_rank: usize,
    pub nested_categories: usize,
    pub is_approximate: bool,
}
struct Adjustments {
    redundant: Vec<usize>,
    exact: Vec<bool>,
    nested: Vec<bool>,
    nested_categories: usize,
}

pub fn compute_absorbed_dof(
    sample: &Prepared,
    structural_intercept: bool,
    policy: DofPolicy,
    clusters: &[&[u32]],
) -> Result<DofReport> {
    sample.validate_alignment()?;
    let topology = sample
        .topology
        .as_ref()
        .ok_or_else(|| invalid("absorbed DoF requires prepared FE topology"))?;
    topology.validate(sample)?;
    validate_clusters(clusters, sample.n_active())?;
    let adjustments = adjustments(topology, policy, clusters)?;
    build_report(sample, topology, structural_intercept, policy, &adjustments)
}
fn adjustments(
    topology: &FeTopology,
    policy: DofPolicy,
    clusters: &[&[u32]],
) -> Result<Adjustments> {
    let count = topology.effects().len();
    let mut redundant = reserved(count)?;
    redundant.resize(count, 0);
    let mut exact = reserved(count)?;
    exact.resize(count, false);
    let mut nested = reserved(count)?;
    nested.resize(count, false);
    let mut eligible = reserved(count)?;
    let mut nested_categories = 0usize;
    for (index, effect) in topology.effects().iter().enumerate() {
        if !effect.intercept {
            continue;
        }
        let partition = topology.effect_partition(index)?;
        if is_nested(partition, clusters)? {
            redundant[index] = partition.n_levels();
            exact[index] = true;
            nested[index] = true;
            nested_categories = nested_categories
                .checked_add(partition.n_levels())
                .ok_or_else(allocation)?;
        } else {
            eligible.push(index);
        }
    }
    if let Some(&first) = eligible.first() {
        exact[first] = true;
    }
    for (position, &index) in eligible.iter().enumerate() {
        if position == 0 && nested_categories == 0 {
            continue;
        }
        redundant[index] = usize::from(topology.effect_partition(index)?.n_levels() > 0);
    }
    if matches!(policy, DofPolicy::FirstPair | DofPolicy::Pairwise) {
        for later_position in 1..eligible.len() {
            if policy == DofPolicy::FirstPair && later_position > 1 {
                break;
            }
            let later = eligible[later_position];
            for (earlier_position, &earlier) in eligible[..later_position].iter().enumerate() {
                let a = topology.effect_partition(earlier)?;
                let b = topology.effect_partition(later)?;
                let components = connected_components_all(
                    &[a.codes(), b.codes()],
                    &[a.n_levels(), b.n_levels()],
                    topology.nobs(),
                )?
                .n_components;
                redundant[later] = redundant[later].max(components);
                if earlier_position == 0 && later_position == 1 {
                    exact[later] = true;
                }
            }
        }
    }
    Ok(Adjustments {
        redundant,
        exact,
        nested,
        nested_categories,
    })
}
fn is_nested(partition: &Partition, clusters: &[&[u32]]) -> Result<bool> {
    for cluster in clusters {
        let mut groups = reserved(partition.n_levels())?;
        groups.resize(partition.n_levels(), u32::MAX);
        let mut nested = true;
        for (&code, &group) in partition.codes().iter().zip(*cluster) {
            let mapped = &mut groups[code as usize];
            if *mapped == u32::MAX {
                *mapped = group;
            } else if *mapped != group {
                nested = false;
                break;
            }
        }
        if nested {
            return Ok(true);
        }
    }
    Ok(false)
}
fn build_report(
    sample: &Prepared,
    topology: &FeTopology,
    structural_intercept: bool,
    policy: DofPolicy,
    adjustments: &Adjustments,
) -> Result<DofReport> {
    let implicit = structural_intercept && topology.effects().is_empty() && topology.nobs() > 0;
    let count = topology
        .effects()
        .iter()
        .try_fold(usize::from(implicit), |sum, effect| {
            sum.checked_add(usize::from(effect.intercept))
                .and_then(|sum| sum.checked_add(effect.slopes.len()))
                .ok_or_else(allocation)
        })?;
    let mut effects = reserved(count)?;
    if implicit {
        effects.push(DofEffect {
            identity: EffectIdentity::ImplicitNoAbsorb,
            label: copied_string("_cons")?,
            categories: 1,
            redundant: 0,
            coefficients: 1,
            redundancy_exact: true,
            nested: false,
            slope_source: None,
        });
    }
    for (index, effect) in topology.effects().iter().enumerate() {
        let partition = topology.effect_partition(index)?;
        let categories = partition.n_levels();
        if effect.intercept {
            effects.push(dof_row(
                effect,
                &effect.label,
                categories,
                adjustments.redundant[index],
                adjustments.exact[index],
                adjustments.nested[index],
                None,
            )?);
        }
        for slope in &effect.slopes {
            let redundant = if policy == DofPolicy::Continuous {
                continuous_redundancy(
                    partition,
                    &sample.regressors[slope.source.get() as usize - 1],
                    effect.intercept,
                )?
            } else {
                0
            };
            effects.push(dof_row(
                effect,
                &slope.label,
                categories,
                redundant,
                false,
                false,
                Some(slope.source),
            )?);
        }
    }
    let mut initial_categories = 0usize;
    let mut redundant_categories = 0usize;
    let mut absorbed_rank = 0usize;
    for effect in &effects {
        initial_categories = initial_categories
            .checked_add(effect.categories)
            .ok_or_else(allocation)?;
        redundant_categories = redundant_categories
            .checked_add(effect.redundant)
            .ok_or_else(allocation)?;
        absorbed_rank = absorbed_rank
            .checked_add(effect.coefficients)
            .ok_or_else(allocation)?;
    }
    if initial_categories.checked_sub(redundant_categories) != Some(absorbed_rank) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "absorbed DoF rank certificate did not pass",
        ));
    }
    let is_approximate = effects.iter().any(|effect| !effect.redundancy_exact);
    Ok(DofReport {
        policy,
        effects,
        initial_categories,
        redundant_categories,
        absorbed_rank,
        nested_categories: adjustments.nested_categories,
        is_approximate,
    })
}
fn dof_row(
    effect: &Effect,
    label: &str,
    categories: usize,
    redundant: usize,
    exact: bool,
    nested: bool,
    slope_source: Option<SourceId>,
) -> Result<DofEffect> {
    let coefficients = categories.checked_sub(redundant).ok_or_else(|| {
        FeregError::new(
            ErrorCode::InvalidData,
            "absorbed redundancy exceeds category count",
        )
    })?;
    let mut sources = reserved(effect.categorical_sources.len())?;
    sources.extend_from_slice(&effect.categorical_sources);
    Ok(DofEffect {
        identity: EffectIdentity::Explicit {
            term_id: effect.term_id,
            sources,
        },
        label: copied_string(label)?,
        categories,
        redundant,
        coefficients,
        redundancy_exact: exact,
        nested,
        slope_source,
    })
}
fn continuous_redundancy(partition: &Partition, values: &[f64], intercept: bool) -> Result<usize> {
    // Pinned continuous-DoF convention; distinct from numerical rank tolerances.
    const ZERO_TOLERANCE: f64 = 1e-6;
    let mut redundant = 0;
    for level in 0..partition.n_levels() {
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        for &row in partition.membership().members(level)? {
            minimum = minimum.min(values[row]);
            maximum = maximum.max(values[row]);
        }
        let is_redundant = if intercept {
            maximum - minimum <= ZERO_TOLERANCE
        } else {
            minimum.abs() + maximum.abs() <= ZERO_TOLERANCE
        };
        redundant += usize::from(is_redundant);
    }
    Ok(redundant)
}
fn validate_clusters(clusters: &[&[u32]], nobs: usize) -> Result<()> {
    if clusters.len() > 10 || clusters.iter().any(|codes| codes.len() != nobs) {
        return Err(invalid(
            "DoF cluster dimensions are invalid or not sample aligned",
        ));
    }
    if clusters.iter().any(|codes| codes.contains(&u32::MAX)) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "cluster codes cannot use u32::MAX",
        ));
    }
    Ok(())
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "absorbed DoF dimensions overflow or allocation failed",
    )
}
