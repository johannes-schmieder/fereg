//! Final-sample cluster views, borrowing existing FE partitions whenever possible.
use super::Prepared;
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    factor::{factorize_code_interaction, factorize_f64, CategoricalCodeView, EncodedFactor},
    topology::FeTopology,
    ModelSpec, SourceId,
};
use std::borrow::Cow;

struct Partition<'a> {
    recipe: Vec<SourceId>,
    codes: Cow<'a, [u32]>,
    n_levels: usize,
}
/// Borrowing the prepared owner prevents sample mutation while cluster views
/// are alive. Repeated declared dimensions share storage but retain their slots.
pub struct ClusterFrame<'a> {
    partitions: Vec<Partition<'a>>,
    dimensions: Vec<usize>,
}
impl<'a> ClusterFrame<'a> {
    pub fn new(spec: &ModelSpec, sample: &'a Prepared) -> Result<Self> {
        spec.validate()?;
        sample.validate_alignment()?;
        if spec.model.sources.len() != sample.regressors.len() {
            return Err(invalid(
                "cluster model registry and final source bank differ",
            ));
        }
        let topology = sample
            .topology
            .as_ref()
            .ok_or_else(|| invalid("clusters require final topology"))?;
        topology.validate(sample)?;
        let mut partitions: Vec<Partition<'a>> = reserved(spec.clusters.len())?;
        let mut dimensions = reserved(spec.clusters.len())?;
        let mut sources: Vec<Option<EncodedFactor<f64>>> = reserved(sample.regressors.len())?;
        sources.resize_with(sample.regressors.len(), || None);
        for term in &spec.clusters {
            let mut recipe = reserved(term.sources.len())?;
            recipe.extend_from_slice(&term.sources);
            recipe.sort_unstable();
            let index = if let Some(index) = partitions.iter().position(|p| p.recipe == recipe) {
                index
            } else {
                let (codes, n_levels) = if let Some(partition) =
                    topology.partitions().iter().find(|p| p.recipe() == recipe)
                {
                    (Cow::Borrowed(partition.codes()), partition.n_levels())
                } else {
                    encode(&recipe, sample, topology, &mut sources)?
                };
                let index = partitions.len();
                partitions.push(Partition {
                    recipe,
                    codes,
                    n_levels,
                });
                index
            };
            dimensions.push(index);
        }
        // Interactions have now borrowed their components. Transfer each
        // requested single-source bank once instead of retaining a second copy.
        for partition in &mut partitions {
            if partition.recipe.len() == 1
                && matches!(&partition.codes, Cow::Owned(values) if values.is_empty())
            {
                partition.codes = Cow::Owned(
                    sources[partition.recipe[0].get() as usize - 1]
                        .take()
                        .expect("pending single-source encoding")
                        .codes,
                );
            }
        }
        Ok(Self {
            partitions,
            dimensions,
        })
    }
    pub fn views(&self) -> Result<Vec<CategoricalCodeView<'_>>> {
        let mut views = reserved(self.dimensions.len())?;
        for &index in &self.dimensions {
            let partition = &self.partitions[index];
            views.push(CategoricalCodeView::new(
                &partition.codes,
                partition.n_levels,
            ));
        }
        Ok(views)
    }
}
fn encode<'a>(
    recipe: &[SourceId],
    sample: &'a Prepared,
    topology: &FeTopology,
    sources: &mut [Option<EncodedFactor<f64>>],
) -> Result<(Cow<'a, [u32]>, usize)> {
    for &source in recipe {
        let index = source.get() as usize - 1;
        if topology.partitions().iter().all(|p| p.recipe() != [source]) && sources[index].is_none()
        {
            sources[index] = Some(factorize_f64(&sample.regressors[index])?);
        }
    }
    if recipe.len() == 1 {
        let encoded = sources[recipe[0].get() as usize - 1]
            .as_ref()
            .expect("unshared source encoded");
        return Ok((Cow::Owned(Vec::new()), encoded.n_levels()));
    }
    let mut components = reserved(recipe.len())?;
    for &source in recipe {
        let view = if let Some(partition) = topology
            .partitions()
            .iter()
            .find(|p| p.recipe() == [source])
        {
            CategoricalCodeView::new(partition.codes(), partition.n_levels())
        } else {
            let encoded = sources[source.get() as usize - 1]
                .as_ref()
                .expect("component encoded");
            CategoricalCodeView::new(&encoded.codes, encoded.n_levels())
        };
        components.push(view);
    }
    let encoded = factorize_code_interaction(&components)?;
    Ok((Cow::Owned(encoded.codes), encoded.n_levels))
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
