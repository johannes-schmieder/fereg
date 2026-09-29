//! Preserve source/term identities while rebasing a committed row union once.
use super::{
    connected_components_all, invalid, Effect, FeTopology, LevelIndex, Partition, SlopeChannel,
};
use crate::error::{copied_string, reserved, Result};

impl FeTopology {
    pub(crate) fn subset(&self, keep: &[bool]) -> Result<Self> {
        if keep.len() != self.nobs {
            return Err(invalid("topology subset mask is not sample aligned"));
        }
        let nobs = keep.iter().filter(|&&k| k).count();
        let mut partitions = reserved(self.partitions.len())?;
        for partition in &self.partitions {
            partitions.push(partition.subset(keep, nobs)?);
        }
        let mut effects = reserved(self.effects.len())?;
        for effect in &self.effects {
            let mut slopes = reserved(effect.slopes.len())?;
            for slope in &effect.slopes {
                slopes.push(SlopeChannel {
                    source: slope.source,
                    label: copied_string(&slope.label)?,
                });
            }
            effects.push(Effect {
                term_id: effect.term_id,
                partition: effect.partition,
                categorical_sources: copied(&effect.categorical_sources)?,
                intercept: effect.intercept,
                slopes,
                label: copied_string(&effect.label)?,
            });
        }
        let mut codes = reserved(partitions.len())?;
        let mut levels = reserved(partitions.len())?;
        for partition in &partitions {
            codes.push(partition.codes.as_slice());
            levels.push(partition.n_levels());
        }
        let components = connected_components_all(&codes, &levels, nobs)?;
        Ok(Self {
            nobs,
            partitions,
            effects,
            components,
        })
    }
}

impl Partition {
    fn subset(&self, keep: &[bool], n: usize) -> Result<Self> {
        let mut remap = reserved(self.n_levels())?;
        remap.resize(self.n_levels(), u32::MAX);
        for (&code, &retain) in self.codes.iter().zip(keep) {
            if retain {
                remap[code as usize] = 0;
            }
        }
        let used = remap.iter().filter(|&&code| code != u32::MAX).count();
        let mut original_levels = reserved(used)?;
        for (level, code) in remap.iter_mut().enumerate() {
            if *code != u32::MAX {
                *code = original_levels.len() as u32;
                original_levels.push(self.original_levels[level]);
            }
        }
        let mut codes = reserved(n)?;
        codes.extend(
            self.codes
                .iter()
                .zip(keep)
                .filter_map(|(&code, &retain)| retain.then_some(remap[code as usize])),
        );
        let membership = LevelIndex::from_codes(&codes, original_levels.len())?;
        Ok(Self {
            recipe: copied(&self.recipe)?,
            codes,
            original_levels,
            membership,
        })
    }
}

fn copied<T: Copy>(source: &[T]) -> Result<Vec<T>> {
    let mut result = reserved(source.len())?;
    result.extend_from_slice(source);
    Ok(result)
}
