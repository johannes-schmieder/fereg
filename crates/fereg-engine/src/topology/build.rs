use super::{
    connected_components, invalid, prune_singletons_with_multiplicity, Effect, FeTopology,
    LevelIndex, Partition, SingletonMultiplicity, SlopeChannel,
};
use crate::{
    data::{retain_rows, DropReason, Prepared, PreparedWeights, WeightKind},
    error::{copied_string, reserved, Result},
    factor::{factorize_code_interaction, factorize_f64, CategoricalCodeView, EncodedFactor},
    ModelSpec, SourceId,
};

struct PendingPartition {
    recipe: Vec<SourceId>,
    codes: Vec<u32>,
    n_levels: usize,
}

/// Rebuild after any sample generation change; all slope channels borrow the same source bank.
pub fn prepare_fixed_effects(spec: &ModelSpec, mut sample: Prepared) -> Result<Prepared> {
    spec.validate()?;
    sample.validate_alignment()?;
    if sample.regressors.len() != spec.model.sources.len() {
        return Err(invalid("prepared source bank and model registry differ"));
    }
    sample.topology = None;
    let (mut pending, effects) = encode_partitions(spec, &sample)?;
    let mut code_views = reserved(pending.len())?;
    let mut levels = reserved(pending.len())?;
    for partition in &pending {
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
    let pruned = prune_singletons_with_multiplicity(
        &code_views,
        &levels,
        &active,
        multiplicity,
        spec.keep_singletons,
    )?;
    drop(code_views);
    sample = sample.compact_active_rows(&pruned.active, DropReason::Singleton)?;
    let mut partitions = reserved(pending.len())?;
    for partition in &mut pending {
        retain_rows(&mut partition.codes, &pruned.active);
    }
    for partition in pending {
        partitions.push(finalize_partition(partition)?);
    }
    let mut codes = reserved(partitions.len())?;
    let mut levels = reserved(partitions.len())?;
    for partition in &partitions {
        codes.push(partition.codes.as_slice());
        levels.push(partition.n_levels());
    }
    active.truncate(sample.n_active());
    let components = connected_components(&codes, &levels, &active)?;
    let topology = FeTopology {
        nobs: sample.n_active(),
        partitions,
        effects,
        components,
    };
    topology.validate(&sample)?;
    sample.topology = Some(topology);
    Ok(sample)
}

fn encode_partitions(
    spec: &ModelSpec,
    sample: &Prepared,
) -> Result<(Vec<PendingPartition>, Vec<Effect>)> {
    let mut sources: Vec<Option<EncodedFactor<f64>>> = reserved(sample.regressors.len())?;
    sources.resize_with(sample.regressors.len(), || None);
    let mut partitions: Vec<PendingPartition> = reserved(spec.absorbed_terms.len())?;
    let mut effects = reserved(spec.absorbed_terms.len())?;
    for term in &spec.absorbed_terms {
        let mut recipe = reserved(term.categorical_sources.len())?;
        recipe.extend_from_slice(&term.categorical_sources);
        recipe.sort_unstable();
        recipe.dedup();
        let partition =
            if let Some(index) = partitions.iter().position(|entry| entry.recipe == recipe) {
                index
            } else {
                for source in &recipe {
                    let index = source.get() as usize - 1;
                    if sources[index].is_none() {
                        sources[index] = Some(factorize_f64(&sample.regressors[index])?);
                    }
                }
                let (codes, n_levels) = if recipe.len() == 1 {
                    let source = sources[recipe[0].get() as usize - 1].as_ref().unwrap();
                    // Single-source codes are transferred after all interactions have borrowed them.
                    (Vec::new(), source.n_levels())
                } else {
                    let mut views = reserved(recipe.len())?;
                    for id in &recipe {
                        let source = sources[id.get() as usize - 1].as_ref().unwrap();
                        views.push(CategoricalCodeView::new(&source.codes, source.n_levels()));
                    }
                    let encoded = factorize_code_interaction(&views)?;
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
        let label = categorical_label(&term.categorical_sources, spec)?;
        let mut slopes = reserved(term.slope_sources.len())?;
        for &source in &term.slope_sources {
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
        categorical_sources.extend_from_slice(&term.categorical_sources);
        effects.push(Effect {
            term_id: Some(term.id),
            partition,
            categorical_sources,
            intercept: term.intercept,
            slopes,
            label,
        });
    }
    for partition in &mut partitions {
        if partition.recipe.len() == 1 {
            partition.codes = sources[partition.recipe[0].get() as usize - 1]
                .take()
                .unwrap()
                .codes;
        }
    }
    Ok((partitions, effects))
}

fn finalize_partition(mut pending: PendingPartition) -> Result<Partition> {
    let mut remap = reserved(pending.n_levels)?;
    remap.resize(pending.n_levels, u32::MAX);
    for &code in &pending.codes {
        remap[code as usize] = 0;
    }
    let mut original_levels = reserved(remap.iter().filter(|&&v| v != u32::MAX).count())?;
    for (old, mapping) in remap.iter_mut().enumerate() {
        if *mapping != u32::MAX {
            *mapping = original_levels.len() as u32;
            original_levels.push(old as u32);
        }
    }
    for code in &mut pending.codes {
        *code = remap[*code as usize];
    }
    let membership = LevelIndex::from_codes(&pending.codes, original_levels.len())?;
    Ok(Partition {
        recipe: pending.recipe,
        codes: pending.codes,
        original_levels,
        membership,
    })
}

fn categorical_label(sources: &[SourceId], spec: &ModelSpec) -> Result<String> {
    let mut label = String::new();
    for (index, source) in sources.iter().enumerate() {
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
