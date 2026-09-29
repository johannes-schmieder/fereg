//! The pinned OLS no-absorb model still contains a hidden structural constant.
//! Represent it in the same FE engine, never a second centering implementation.
use super::{connected_components_all, invalid, Effect, FeTopology, LevelIndex, Partition};
use crate::{
    data::Prepared,
    error::{copied_string, reserved, ErrorCode, FeregError, Result},
};
pub fn prepare_implicit_constant(mut sample: Prepared) -> Result<Prepared> {
    sample.validate_alignment()?;
    let topology = sample
        .topology
        .as_ref()
        .ok_or_else(|| invalid("implicit constant requires prepared topology"))?;
    if !topology.effects().is_empty() || !topology.partitions().is_empty() {
        return Err(invalid(
            "implicit no-absorb constant cannot replace declared effects",
        ));
    }
    let n = sample.n_active();
    if n == 0 {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "implicit constant requires observations",
        ));
    }
    let mut codes = reserved(n)?;
    codes.resize(n, 0);
    let components = connected_components_all(&[&codes], &[1], n)?;
    let membership = LevelIndex::from_codes(&codes, 1)?;
    let mut original_levels = reserved(1)?;
    original_levels.push(0);
    let mut partitions = reserved(1)?;
    partitions.push(Partition {
        recipe: Vec::new(),
        codes,
        original_levels,
        membership,
    });
    let mut effects = reserved(1)?;
    effects.push(Effect {
        term_id: None,
        partition: 0,
        categorical_sources: Vec::new(),
        intercept: true,
        slopes: Vec::new(),
        label: copied_string("_cons")?,
    });
    let topology = FeTopology {
        nobs: n,
        partitions,
        effects,
        components,
    };
    topology.validate(&sample)?;
    sample.topology = Some(topology);
    Ok(sample)
}
