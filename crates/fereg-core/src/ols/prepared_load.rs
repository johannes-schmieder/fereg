//! Owned early columns keep optional encodings tied to their authoritative input.

use super::*;

/// Early numeric columns owned continuously from factorization through sample
/// preparation. Optional factorization never determines row availability.
#[derive(Debug)]
pub struct PreparedOlsLoadColumns {
    columns: Vec<(SourceId, LoadedNumericColumn)>,
    hint: Option<PreparedOlsLoadHint>,
}

impl PreparedOlsLoadColumns {
    /// `admitted` covers descriptors and optional factorization workspace;
    /// the moved raw buffers remain part of the caller's input admission.
    /// Refusal retains every raw column for authoritative preparation.
    pub fn prepare(
        columns: Vec<(SourceId, LoadedNumericColumn)>,
        rows: usize,
        admitted: u128,
    ) -> Self {
        let descriptor_bytes = crate::memory::vec_capacity_bytes(&columns);
        let hint = admitted
            .checked_sub(descriptor_bytes)
            .and_then(|remaining| {
                PreparedOlsLoadHint::prepare(
                    columns.iter().map(|(id, column)| {
                        (*id, column.values.as_slice(), column.missing.as_slice())
                    }),
                    rows,
                    remaining,
                )
            });
        let mut capsule = Self { columns, hint };
        // Completed encodings and their raw-column descriptors replace the
        // old speculative workspace; do not enlarge preparation/moment overlap.
        let retained_limit = PreparedOlsLoadHint::workspace_bytes(rows, capsule.columns.len())
            .unwrap_or(0)
            .min(admitted);
        if capsule
            .descriptor_bytes()
            .checked_add(capsule.encoding_bytes())
            .is_none_or(|bytes| bytes > retained_limit)
        {
            capsule.hint = None;
        }
        capsule
    }

    pub fn columns(&self) -> impl ExactSizeIterator<Item = (SourceId, &LoadedNumericColumn)> {
        self.columns.iter().map(|(id, column)| (*id, column))
    }

    pub fn descriptor_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.columns)
    }

    pub fn encoding_bytes(&self) -> u128 {
        self.hint
            .as_ref()
            .map_or(0, PreparedOlsLoadHint::owned_bytes)
    }

    fn restore(self, loaded: &mut LoadedOlsInput) -> Result<Option<PreparedOlsLoadHint>> {
        let mut previous = None;
        for (id, _) in &self.columns {
            let index = source_index(*id)?;
            let slot = loaded
                .numeric_sources
                .get(index)
                .ok_or_else(|| invalid("owned load column source slot is out of range"))?;
            if previous.is_some_and(|prior| prior >= index)
                || slot.0 != *id
                || !slot.1.values.is_empty()
                || !slot.1.missing.is_empty()
            {
                return Err(invalid(
                    "owned load columns require ordered matching vacant slots",
                ));
            }
            previous = Some(index);
        }
        for (id, column) in self.columns {
            loaded.numeric_sources[source_index(id)?].1 = column;
        }
        Ok(self.hint)
    }
}

pub(crate) enum PreparedSourceReuse {
    Checked(PreparedOlsLoadHint),
    Owned(Vec<PreparedCategoricalSource>),
}

impl PreparedSourceReuse {
    pub(super) fn into_sources(
        self,
        request: &OlsRequestV1,
        sample: &PreparedNumericSample,
    ) -> Vec<PreparedCategoricalSource> {
        match self {
            Self::Checked(hint) => hint.into_matching_sources(request, sample),
            Self::Owned(sources) => sources,
        }
    }
}

pub(crate) fn prepare_numeric_sample_owned_with_columns(
    request: &OlsRequestV1,
    mut loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
    categorical: bool,
    columns: Option<PreparedOlsLoadColumns>,
) -> Result<(
    PreparedOlsModel,
    PreparedNumericSample,
    Option<PreparedSourceReuse>,
)> {
    let model = request.compile_model(cluster_sources, categorical)?;
    let hint = match columns {
        Some(columns) => columns.restore(&mut loaded)?,
        None => None,
    };
    validate_loaded(request, &loaded)?;
    validate_candidate_weight_domain(&loaded)?;
    let complete = is_full_retained_unweighted_sample(&loaded);
    let sample = if complete {
        take_full_retained_unweighted_sample(loaded)?
    } else {
        take_classified_sample(request, loaded, cluster_sources)?
    };
    validate_available_categorical_domain(request, &sample)?;
    // Only the actual transfer branch preserves the exact owned columns. The
    // request may have changed since optional factorization, so IDs still bind
    // every encoding to its current role before the sources can be reused.
    let reuse = hint.map(|hint| {
        if complete
            && hint.rows == sample.n_active()
            && PreparedOlsLoadHint::categorical_ids(request)
                .eq(hint.sources.iter().map(|source| source.source))
        {
            PreparedSourceReuse::Owned(hint.sources)
        } else {
            PreparedSourceReuse::Checked(hint)
        }
    });
    Ok((model, sample, reuse))
}

pub(crate) fn prepare_owned_with_load_columns(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
    categorical: bool,
    columns: Option<PreparedOlsLoadColumns>,
    membership_scratch: u128,
) -> Result<PreparedOlsData> {
    if categorical {
        let _plan = crate::fe::plan_categorical_absorbed_request(request)?;
    }
    let (model, sample, reuse) = prepare_numeric_sample_owned_with_columns(
        request,
        loaded,
        cluster_sources,
        categorical,
        columns,
    )?;
    crate::ols::prepared_categorical_data::finish_prepared_model_with_hint(
        request,
        cluster_sources,
        model,
        sample,
        categorical,
        false,
        true,
        reuse,
        membership_scratch,
    )
}
