use super::exact;
use crate::{
    data::{DropReason, Prepared, PreparedWeights, WeightKind},
    error::{reserved, Result},
    topology::{prune_singletons_with_multiplicity, SingletonMultiplicity},
};

pub(super) fn singletons(
    sample: &Prepared,
    active: &mut [bool],
    reasons: &mut [DropReason],
) -> Result<()> {
    let Some(topology) = &sample.topology else {
        return Ok(());
    };
    let mut codes = reserved(topology.partitions().len())?;
    let mut levels = reserved(topology.partitions().len())?;
    // The synthetic one-level constant is not a declared absorbed partition.
    for (index, partition) in topology.partitions().iter().enumerate() {
        if topology
            .effects()
            .iter()
            .any(|e| e.partition == index && e.term_id.is_some())
        {
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
    let pruned = prune_singletons_with_multiplicity(&codes, &levels, active, multiplicity, false)?;
    for row in pruned.dropped_observations {
        active[row] = false;
        reasons[row] = DropReason::Singleton;
    }
    Ok(())
}

pub(super) fn complete(
    sample: &Prepared,
    active: &mut [bool],
    reasons: &mut [DropReason],
    drop_singletons: bool,
    exact_fe: bool,
) -> Result<()> {
    loop {
        if drop_singletons {
            singletons(sample, active, reasons)?;
        }
        if !exact_fe {
            break;
        }
        let (drops, _) = exact::detect(sample, active)?;
        if !drops.iter().any(|&drop| drop) {
            break;
        }
        for (row, &drop) in drops.iter().enumerate() {
            if drop {
                active[row] = false;
                reasons[row] = DropReason::FixedEffectSeparation;
            }
        }
    }
    Ok(())
}
