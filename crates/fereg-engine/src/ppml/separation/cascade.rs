use super::exact;
use crate::{
    data::{DropReason, Prepared, PreparedWeights, WeightKind},
    error::{reserved, Result},
    execution::{self, Poll},
    topology::{prune_singletons_polled, SingletonMultiplicity},
};

pub(super) fn singletons(
    sample: &Prepared,
    active: &mut [bool],
    reasons: &mut [DropReason],
    poll: Poll<'_>,
) -> Result<()> {
    execution::check(poll)?;
    let Some(topology) = &sample.topology else {
        return Ok(());
    };
    let mut codes = reserved(topology.partitions().len())?;
    let mut levels = reserved(topology.partitions().len())?;
    // The synthetic one-level constant is not a declared absorbed partition.
    for (index, partition) in topology.partitions().iter().enumerate() {
        execution::check_chunk(poll, index)?;
        if execution::any(
            topology.effects(),
            |e| e.partition == index && e.term_id.is_some(),
            poll,
        )? {
            codes.push(partition.codes());
            levels.push(partition.n_levels());
        }
    }
    let multiplicity = match &sample.weights {
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => SingletonMultiplicity::Frequency(values),
        _ => SingletonMultiplicity::Unit,
    };
    let pruned = prune_singletons_polled(&codes, &levels, active, multiplicity, false, poll)?;
    for (index, row) in pruned.dropped_observations.into_iter().enumerate() {
        execution::check_chunk(poll, index)?;
        active[row] = false;
        reasons[row] = DropReason::Singleton;
    }
    execution::check(poll)
}

pub(super) fn complete(
    sample: &Prepared,
    active: &mut [bool],
    reasons: &mut [DropReason],
    drop_singletons: bool,
    exact_fe: bool,
    poll: Poll<'_>,
) -> Result<()> {
    loop {
        execution::check(poll)?;
        if drop_singletons {
            singletons(sample, active, reasons, poll)?;
        }
        if !exact_fe {
            break;
        }
        let (drops, _) = exact::detect(sample, active, poll)?;
        if !execution::any(&drops, |&drop| drop, poll)? {
            break;
        }
        for (row, &drop) in drops.iter().enumerate() {
            execution::check_chunk(poll, row)?;
            if drop {
                active[row] = false;
                reasons[row] = DropReason::FixedEffectSeparation;
            }
        }
    }
    execution::check(poll)
}
