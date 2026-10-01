use super::{
    connected_components_polled, invalid, prune_singletons_polled, Effect, FeTopology, LevelIndex,
    Partition, SingletonMultiplicity, SlopeChannel,
};
use crate::{
    data::{retain_rows_polled, DropReason, Prepared, PreparedWeights, WeightKind},
    error::{copied_string, reserved, Result},
    execution::{self, Poll},
    factor::{
        factorize_code_interaction_polled, factorize_f64_polled, CategoricalCodeView, EncodedFactor,
    },
    ModelSpec, SourceId,
};

struct PendingPartition {
    recipe: Vec<SourceId>,
    codes: Vec<u32>,
    n_levels: usize,
}

/// Rebuild after any sample generation change; all slope channels borrow the same source bank.
pub fn prepare_fixed_effects(spec: &ModelSpec, sample: Prepared) -> Result<Prepared> {
    prepare_fixed_effects_polled(spec, sample, None)
}
pub(crate) fn prepare_fixed_effects_polled(
    spec: &ModelSpec,
    sample: Prepared,
    poll: Poll<'_>,
) -> Result<Prepared> {
    prepare(spec, sample, spec.keep_singletons, poll)
}
/// PPML's ledger owns the first singleton generation and its diagnostics.
pub(crate) fn prepare_unpruned(
    spec: &ModelSpec,
    sample: Prepared,
    poll: Poll<'_>,
) -> Result<Prepared> {
    prepare(spec, sample, true, poll)
}
fn prepare(
    spec: &ModelSpec,
    mut sample: Prepared,
    keep_singletons: bool,
    poll: Poll<'_>,
) -> Result<Prepared> {
    execution::check(poll)?;
    spec.validate()?;
    sample.validate_alignment_polled(poll)?;
    if sample.regressors.len() != spec.model.sources.len() {
        return Err(invalid("prepared source bank and model registry differ"));
    }
    sample.topology = None;
    let (mut pending, effects) = encode_partitions(spec, &sample, poll)?;
    let mut code_views = reserved(pending.len())?;
    let mut levels = reserved(pending.len())?;
    for (index, partition) in pending.iter().enumerate() {
        execution::check_chunk(poll, index)?;
        code_views.push(partition.codes.as_slice());
        levels.push(partition.n_levels);
    }
    let mut active = reserved(sample.n_active())?;
    active.resize(sample.n_active(), true);
    let multiplicity = match &sample.weights {
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => SingletonMultiplicity::Frequency(values),
        _ => SingletonMultiplicity::Unit,
    };
    let pruned = prune_singletons_polled(
        &code_views,
        &levels,
        &active,
        multiplicity,
        keep_singletons,
        poll,
    )?;
    drop(code_views);
    sample = sample.compact_active_rows_polled(&pruned.active, DropReason::Singleton, poll)?;
    let mut partitions = reserved(pending.len())?;
    for partition in &mut pending {
        retain_rows_polled(&mut partition.codes, &pruned.active, poll)?;
    }
    for partition in pending {
        partitions.push(finalize_partition(partition, poll)?);
    }
    let mut codes = reserved(partitions.len())?;
    let mut levels = reserved(partitions.len())?;
    for (index, partition) in partitions.iter().enumerate() {
        execution::check_chunk(poll, index)?;
        codes.push(partition.codes.as_slice());
        levels.push(partition.n_levels());
    }
    active.truncate(sample.n_active());
    let components = connected_components_polled(&codes, &levels, &active, poll)?;
    let topology = FeTopology {
        nobs: sample.n_active(),
        partitions,
        effects,
        components,
    };
    topology.validate_polled(&sample, poll)?;
    sample.topology = Some(topology);
    execution::check(poll)?;
    Ok(sample)
}

fn encode_partitions(
    spec: &ModelSpec,
    sample: &Prepared,
    poll: Poll<'_>,
) -> Result<(Vec<PendingPartition>, Vec<Effect>)> {
    let mut sources: Vec<Option<EncodedFactor<f64>>> = reserved(sample.regressors.len())?;
    sources.resize_with(sample.regressors.len(), || None);
    let mut partitions: Vec<PendingPartition> = reserved(spec.absorbed_terms.len())?;
    let mut effects = reserved(spec.absorbed_terms.len())?;
    for (term_index, term) in spec.absorbed_terms.iter().enumerate() {
        execution::check_chunk(poll, term_index)?;
        let recipe = canonical_recipe(&term.categorical_sources, poll)?;
        let partition = if let Some(index) = find_partition(&partitions, &recipe, poll)? {
            index
        } else {
            for (position, source) in recipe.iter().enumerate() {
                execution::check_chunk(poll, position)?;
                let index = source.get() as usize - 1;
                if sources[index].is_none() {
                    sources[index] = Some(factorize_f64_polled(&sample.regressors[index], poll)?);
                }
            }
            let (codes, n_levels) = if recipe.len() == 1 {
                let source = sources[recipe[0].get() as usize - 1].as_ref().unwrap();
                // Single-source codes are transferred after all interactions have borrowed them.
                (Vec::new(), source.n_levels())
            } else {
                let mut views = reserved(recipe.len())?;
                for (position, id) in recipe.iter().enumerate() {
                    execution::check_chunk(poll, position)?;
                    let source = sources[id.get() as usize - 1].as_ref().unwrap();
                    views.push(CategoricalCodeView::new(&source.codes, source.n_levels()));
                }
                let encoded = factorize_code_interaction_polled(&views, poll)?;
                (encoded.codes, encoded.n_levels)
            };
            let index = partitions.len();
            partitions.push(PendingPartition {
                recipe,
                codes,
                n_levels,
            });
            index
        };
        let label = categorical_label(&term.categorical_sources, spec, poll)?;
        let mut slopes = reserved(term.slope_sources.len())?;
        for (position, &source) in term.slope_sources.iter().enumerate() {
            execution::check_chunk(poll, position)?;
            let name = &spec.model.sources[source.get() as usize - 1].canonical_name;
            let mut slope_label = copied_string(&label)?;
            slope_label.try_reserve(3 + name.len()).map_err(|_| {
                crate::FeregError::new(
                    crate::ErrorCode::Allocation,
                    "unable to reserve FE slope label",
                )
            })?;
            slope_label.push_str("#c.");
            slope_label.push_str(name);
            slopes.push(SlopeChannel {
                source,
                label: slope_label,
            });
        }
        let mut categorical_sources = reserved(term.categorical_sources.len())?;
        for (position, &source) in term.categorical_sources.iter().enumerate() {
            execution::check_chunk(poll, position)?;
            categorical_sources.push(source);
        }
        effects.push(Effect {
            term_id: Some(term.id),
            partition,
            categorical_sources,
            intercept: term.intercept,
            slopes,
            label,
        });
    }
    for (index, partition) in partitions.iter_mut().enumerate() {
        execution::check_chunk(poll, index)?;
        if partition.recipe.len() == 1 {
            partition.codes = sources[partition.recipe[0].get() as usize - 1]
                .take()
                .unwrap()
                .codes;
        }
    }
    Ok((partitions, effects))
}

fn finalize_partition(mut pending: PendingPartition, poll: Poll<'_>) -> Result<Partition> {
    execution::check(poll)?;
    let mut remap = reserved(pending.n_levels)?;
    remap.resize(pending.n_levels, u32::MAX);
    for (row, &code) in pending.codes.iter().enumerate() {
        execution::check_chunk(poll, row)?;
        remap[code as usize] = 0;
    }
    let used = execution::fold(&remap, 0usize, poll, |count, &code| {
        count + usize::from(code != u32::MAX)
    })?;
    let mut original_levels = reserved(used)?;
    for (old, mapping) in remap.iter_mut().enumerate() {
        execution::check_chunk(poll, old)?;
        if *mapping != u32::MAX {
            *mapping = original_levels.len() as u32;
            original_levels.push(old as u32);
        }
    }
    for (row, code) in pending.codes.iter_mut().enumerate() {
        execution::check_chunk(poll, row)?;
        *code = remap[*code as usize];
    }
    let membership = LevelIndex::from_codes_polled(&pending.codes, original_levels.len(), poll)?;
    execution::check(poll)?;
    Ok(Partition {
        recipe: pending.recipe,
        codes: pending.codes,
        original_levels,
        membership,
    })
}

fn categorical_label(sources: &[SourceId], spec: &ModelSpec, poll: Poll<'_>) -> Result<String> {
    let mut label = String::new();
    for (index, source) in sources.iter().enumerate() {
        execution::check_chunk(poll, index)?;
        let name = &spec.model.sources[source.get() as usize - 1].canonical_name;
        label
            .try_reserve(
                name.len()
                    .checked_add(1)
                    .ok_or_else(|| invalid("FE label length overflows"))?,
            )
            .map_err(|_| {
                crate::FeregError::new(crate::ErrorCode::Allocation, "unable to reserve FE label")
            })?;
        if index > 0 {
            label.push('#');
        }
        label.push_str(name);
    }
    Ok(label)
}

fn canonical_recipe(sources: &[SourceId], poll: Poll<'_>) -> Result<Vec<SourceId>> {
    let mut recipe = reserved(sources.len())?;
    for (position, &source) in sources.iter().enumerate() {
        execution::check_chunk(poll, position)?;
        recipe.push(source);
    }
    execution::sort_unstable_by(&mut recipe, Ord::cmp, poll)?;
    let mut written = 0;
    for read in 0..recipe.len() {
        execution::check_chunk(poll, read)?;
        if written == 0 || recipe[read] != recipe[written - 1] {
            recipe[written] = recipe[read];
            written += 1;
        }
    }
    recipe.truncate(written);
    Ok(recipe)
}

fn find_partition(
    partitions: &[PendingPartition],
    recipe: &[SourceId],
    poll: Poll<'_>,
) -> Result<Option<usize>> {
    for (index, entry) in partitions.iter().enumerate() {
        execution::check_chunk(poll, index)?;
        if entry.recipe.len() == recipe.len()
            && !execution::any(entry.recipe.iter().zip(recipe), |(a, b)| a != b, poll)?
        {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod interruption_tests;
