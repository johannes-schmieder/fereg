//! Preserve source/term identities while rebasing a committed row union once.
use super::{
    connected_components_all_polled, invalid, Effect, FeTopology, LevelIndex, Partition,
    SlopeChannel,
};
use crate::{
    error::{copied_string, reserved, Result},
    execution::{self, Poll},
};

impl FeTopology {
    pub(crate) fn subset_polled(&self, keep: &[bool], poll: Poll<'_>) -> Result<Self> {
        execution::check(poll)?;
        if keep.len() != self.nobs {
            return Err(invalid("topology subset mask is not sample aligned"));
        }
        let nobs = execution::fold(keep, 0, poll, |n, &keep| n + usize::from(keep))?;
        let mut partitions = reserved(self.partitions.len())?;
        for (index, partition) in self.partitions.iter().enumerate() {
            execution::check_chunk(poll, index)?;
            partitions.push(partition.subset(keep, nobs, poll)?);
        }
        let mut effects = reserved(self.effects.len())?;
        for (index, effect) in self.effects.iter().enumerate() {
            execution::check_chunk(poll, index)?;
            let mut slopes = reserved(effect.slopes.len())?;
            for (channel, slope) in effect.slopes.iter().enumerate() {
                execution::check_chunk(poll, channel)?;
                slopes.push(SlopeChannel {
                    source: slope.source,
                    label: copied_string(&slope.label)?,
                });
            }
            effects.push(Effect {
                term_id: effect.term_id,
                partition: effect.partition,
                categorical_sources: copied(&effect.categorical_sources, poll)?,
                intercept: effect.intercept,
                slopes,
                label: copied_string(&effect.label)?,
            });
        }
        let mut codes = reserved(partitions.len())?;
        let mut levels = reserved(partitions.len())?;
        for (index, partition) in partitions.iter().enumerate() {
            execution::check_chunk(poll, index)?;
            codes.push(partition.codes.as_slice());
            levels.push(partition.n_levels());
        }
        let components = connected_components_all_polled(&codes, &levels, nobs, poll)?;
        execution::check(poll)?;
        Ok(Self {
            nobs,
            partitions,
            effects,
            components,
        })
    }
}

impl Partition {
    fn subset(&self, keep: &[bool], n: usize, poll: Poll<'_>) -> Result<Self> {
        let mut remap = reserved(self.n_levels())?;
        remap.resize(self.n_levels(), u32::MAX);
        for (row, (&code, &retain)) in self.codes.iter().zip(keep).enumerate() {
            execution::check_chunk(poll, row)?;
            if retain {
                remap[code as usize] = 0;
            }
        }
        let used = execution::fold(&remap, 0, poll, |n, &code| {
            n + usize::from(code != u32::MAX)
        })?;
        let mut original_levels = reserved(used)?;
        for (level, code) in remap.iter_mut().enumerate() {
            execution::check_chunk(poll, level)?;
            if *code != u32::MAX {
                *code = original_levels.len() as u32;
                original_levels.push(self.original_levels[level]);
            }
        }
        let mut codes = reserved(n)?;
        for (row, (&code, &retain)) in self.codes.iter().zip(keep).enumerate() {
            execution::check_chunk(poll, row)?;
            if retain {
                codes.push(remap[code as usize]);
            }
        }
        let membership = LevelIndex::from_codes_polled(&codes, original_levels.len(), poll)?;
        Ok(Self {
            recipe: copied(&self.recipe, poll)?,
            codes,
            original_levels,
            membership,
        })
    }
}

fn copied<T: Copy>(source: &[T], poll: Poll<'_>) -> Result<Vec<T>> {
    let mut result = reserved(source.len())?;
    for (index, &value) in source.iter().enumerate() {
        execution::check_chunk(poll, index)?;
        result.push(value);
    }
    Ok(result)
}
