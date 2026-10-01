//! The pinned OLS no-absorb model still contains a hidden structural constant.
//! Represent it in the same FE engine, never a second centering implementation.
use super::{invalid, Effect, FeTopology, LevelIndex, Partition};
use crate::{
    data::Prepared,
    error::{copied_string, reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
};
pub fn prepare_implicit_constant(sample: Prepared) -> Result<Prepared> {
    prepare_implicit_constant_polled(sample, None)
}
pub(crate) fn prepare_implicit_constant_polled(
    mut sample: Prepared,
    poll: Poll<'_>,
) -> Result<Prepared> {
    sample.validate_alignment_polled(poll)?;
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
    let components = super::connected_components_all_polled(&[&codes], &[1], n, poll)?;
    let membership = LevelIndex::from_codes_polled(&codes, 1, poll)?;
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
    topology.validate_polled(&sample, poll)?;
    sample.topology = Some(topology);
    execution::check(poll)?;
    Ok(sample)
}
