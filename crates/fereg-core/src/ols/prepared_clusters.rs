use std::cmp::Ordering;

use crate::error::{allocation, invalid_argument, invalid_data, Result};
use crate::memory::push_reserved;
use crate::metadata::SourceId;

use super::prepared::{
    prepare_ols_data_owned_with_cluster_sources, prepare_ols_data_with_cluster_sources,
};
use super::{LoadedOlsInput, OlsRequestV1, PreparedOlsData};

/// One declared cluster dimension. Multiple numeric sources describe one
/// categorical interaction such as `firm#year`, not multiple-way clustering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlsClusterTermV1 {
    pub label: String,
    pub sources: Vec<SourceId>,
}

/// Sample-bound prepared-OLS request with one to ten declared cluster
/// dimensions. Cluster sources use the same numeric source registry as OLS.
#[derive(Debug, Clone, PartialEq)]
pub struct OlsClusteredRequestV1 {
    pub ols: OlsRequestV1,
    pub clusters: Vec<OlsClusterTermV1>,
}

struct ClusterRequest<'a> {
    ols: &'a super::OlsRequestV1,
    clusters: &'a [OlsClusterTermV1],
}

impl OlsClusteredRequestV1 {
    fn borrowed(&self) -> ClusterRequest<'_> {
        ClusterRequest {
            ols: &self.ols,
            clusters: &self.clusters,
        }
    }
}

impl OlsClusteredRequestV1 {
    /// Validate the complete OLS-plus-cluster source graph before observation
    /// loading and return its executable finite-dimensional model.
    pub fn validate_and_compile(&self) -> Result<super::PreparedOlsModel> {
        let cluster_sources = validate_request(self)?;
        self.ols
            .validate_and_compile_with_cluster_sources(&cluster_sources)
    }

    /// PPML metadata validation admits the categorical absorbed slopes owned
    /// by its prepared topology while preserving exact cluster-source checks.
    pub fn validate_ppml_and_compile(&self) -> Result<super::PreparedOlsModel> {
        let cluster_sources = validate_request(self)?;
        self.ols
            .validate_ppml_and_compile_with_cluster_sources(&cluster_sources)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PreparedOlsClusterCodes {
    /// Reuse the authoritative final categorical codes owned by OLS.
    CategoricalSource { categorical_source_index: usize },
    /// One collision-free interaction code per final active observation.
    Interaction(Vec<u32>),
}

/// One final-sample cluster dimension in declared order.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedOlsClusterDimension {
    pub(super) label: String,
    pub(super) sources: Vec<SourceId>,
    pub(super) codes: PreparedOlsClusterCodes,
    pub(super) groups: usize,
}

impl PreparedOlsClusterDimension {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn sources(&self) -> &[SourceId] {
        &self.sources
    }

    #[must_use]
    pub const fn groups(&self) -> usize {
        self.groups
    }

    #[must_use]
    pub const fn is_interaction(&self) -> bool {
        matches!(self.codes, PreparedOlsClusterCodes::Interaction(_))
    }
}

/// Compact cluster metadata and owned interaction codes. Simple dimensions
/// contain only a categorical-source index and never copy observation codes.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedOlsClusterDesign {
    pub(super) dimensions: Vec<PreparedOlsClusterDimension>,
}

impl PreparedOlsClusterDesign {
    /// Owned cluster buffers only; simple dimensions borrow OLS source codes.
    #[must_use]
    pub fn owned_buffer_bytes(&self) -> u128 {
        self.metadata_capacity_bytes()
            + self
                .dimensions
                .iter()
                .map(|dimension| match &dimension.codes {
                    PreparedOlsClusterCodes::CategoricalSource { .. } => 0,
                    PreparedOlsClusterCodes::Interaction(codes) => {
                        crate::memory::vec_capacity_bytes(codes)
                    }
                })
                .sum::<u128>()
    }

    pub(crate) fn metadata_capacity_bytes(&self) -> u128 {
        self.dimensions.capacity() as u128
            * std::mem::size_of::<PreparedOlsClusterDimension>() as u128
            + self
                .dimensions
                .iter()
                .map(|dimension| {
                    dimension.label.capacity() as u128
                        + dimension.sources.capacity() as u128
                            * std::mem::size_of::<SourceId>() as u128
                })
                .sum::<u128>()
    }

    #[must_use]
    pub fn dimensions(&self) -> &[PreparedOlsClusterDimension] {
        &self.dimensions
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.dimensions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dimensions.is_empty()
    }
}

impl PreparedOlsData {
    #[must_use]
    pub const fn ols(&self) -> &Self {
        self
    }

    #[must_use]
    pub const fn cluster_design(&self) -> Option<&PreparedOlsClusterDesign> {
        self.clusters.as_ref()
    }

    /// Empty metadata makes unsupported clustered fitting fail its existing
    /// validation boundary; no independent wrapper can bind another sample.
    #[must_use]
    pub fn clusters(&self) -> &PreparedOlsClusterDesign {
        static EMPTY: PreparedOlsClusterDesign = PreparedOlsClusterDesign {
            dimensions: Vec::new(),
        };
        self.clusters.as_ref().unwrap_or(&EMPTY)
    }

    pub fn cluster_codes(&self, dimension: usize) -> Result<&[u32]> {
        let dimension = self
            .clusters()
            .dimensions
            .get(dimension)
            .ok_or_else(|| invalid_argument("prepared cluster dimension is out of range"))?;
        let codes = match &dimension.codes {
            PreparedOlsClusterCodes::CategoricalSource {
                categorical_source_index,
            } => self.categorical_codes(*categorical_source_index)?,
            PreparedOlsClusterCodes::Interaction(codes) => codes,
        };
        if codes.len() != self.sample().n_active() {
            return Err(invalid_argument(
                "prepared cluster codes are not aligned with the final sample",
            ));
        }
        Ok(codes)
    }
}

trait ClusterReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_string(
        &mut self,
        target: &mut String,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemClusterReservation;

impl ClusterReservation for SystemClusterReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }

    fn reserve_string(
        &mut self,
        target: &mut String,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

pub(super) fn finish_prepare_ols_clusters(
    request: &OlsClusteredRequestV1,
    mut ols: PreparedOlsData,
) -> Result<PreparedOlsData> {
    let mut reservation = SystemClusterReservation;
    let clusters = build_cluster_design(&request.borrowed(), &ols, &mut reservation)?;
    ols.clusters = Some(clusters);
    certify_prepared_clusters(&ols)?;
    Ok(ols)
}

pub(super) fn validate_request(request: &OlsClusteredRequestV1) -> Result<Vec<SourceId>> {
    validate_cluster_request(&request.borrowed())
}

fn validate_cluster_request(request: &ClusterRequest<'_>) -> Result<Vec<SourceId>> {
    if !(1..=10).contains(&request.clusters.len()) {
        return Err(invalid_argument(
            "prepared clustered OLS requires between one and ten declared cluster dimensions",
        ));
    }
    let mut all_sources = Vec::new();
    all_sources
        .try_reserve_exact(request.ols.model.sources.len())
        .map_err(|_| allocation("unable to reserve prepared cluster source set"))?;
    for term in request.clusters {
        if term.label.is_empty() {
            return Err(invalid_argument("prepared cluster label cannot be empty"));
        }
        if term.sources.is_empty() {
            return Err(invalid_argument(
                "prepared cluster dimension requires at least one numeric source",
            ));
        }
        for (position, source) in term.sources.iter().enumerate() {
            if !request
                .ols
                .model
                .sources
                .iter()
                .any(|entry| entry.id == *source)
            {
                return Err(invalid_argument(
                    "prepared cluster source is absent from the numeric source registry",
                ));
            }
            if term.sources[..position].contains(source) {
                return Err(invalid_argument(
                    "prepared cluster interaction repeats a numeric source",
                ));
            }
            if !all_sources.contains(source) {
                if all_sources.len() == all_sources.capacity() {
                    return Err(allocation("prepared cluster source-set capacity exhausted"));
                }
                all_sources.push(*source);
            }
        }
    }
    Ok(all_sources)
}

fn build_cluster_design<R: ClusterReservation>(
    request: &ClusterRequest<'_>,
    ols: &PreparedOlsData,
    reservation: &mut R,
) -> Result<PreparedOlsClusterDesign> {
    let mut dimensions = reserved_empty(
        reservation,
        request.clusters.len(),
        "unable to reserve prepared cluster dimension table",
    )?;
    for term in request.clusters {
        let mut categorical_indices = reserved_empty(
            reservation,
            term.sources.len(),
            "unable to reserve prepared cluster source-index recipe",
        )?;
        for source in &term.sources {
            let index = ols
                .categorical_sources()
                .iter()
                .position(|categorical| categorical.source() == *source)
                .ok_or_else(|| {
                    invalid_argument("prepared cluster categorical source disappeared")
                })?;
            push_reserved(
                &mut categorical_indices,
                index,
                "prepared cluster source-index capacity exhausted",
            )?;
        }
        let (codes, groups) = if let [index] = categorical_indices.as_slice() {
            let groups = ols
                .categorical_sources()
                .get(*index)
                .ok_or_else(|| invalid_argument("prepared simple cluster source disappeared"))?
                .n_levels(ols.fe_topology())?;
            (
                PreparedOlsClusterCodes::CategoricalSource {
                    categorical_source_index: *index,
                },
                groups,
            )
        } else {
            let (codes, groups) = factorize_interaction(ols, &categorical_indices, reservation)?;
            (PreparedOlsClusterCodes::Interaction(codes), groups)
        };
        if groups < 2 {
            return Err(invalid_data(
                "each declared cluster dimension must contain at least two final-sample groups",
            ));
        }
        let label = fallible_string_copy(
            &term.label,
            reservation,
            "unable to reserve prepared cluster label",
        )?;
        let mut sources = reserved_empty(
            reservation,
            term.sources.len(),
            "unable to reserve prepared cluster source identities",
        )?;
        for source in &term.sources {
            push_reserved(
                &mut sources,
                *source,
                "prepared cluster source-identity capacity exhausted",
            )?;
        }
        push_reserved(
            &mut dimensions,
            PreparedOlsClusterDimension {
                label,
                sources,
                codes,
                groups,
            },
            "prepared cluster dimension capacity exhausted",
        )?;
    }
    Ok(PreparedOlsClusterDesign { dimensions })
}

/// Collision-free tuple factorization without concatenated strings or an
/// observation-by-arity tuple table. Codes are assigned in lexicographic tuple
/// order, so they are invariant to observation order.
fn factorize_interaction<R: ClusterReservation>(
    ols: &PreparedOlsData,
    categorical_indices: &[usize],
    reservation: &mut R,
) -> Result<(Vec<u32>, usize)> {
    let nobs = ols.sample.n_active();
    if categorical_indices.len() < 2 {
        return Err(invalid_argument(
            "prepared cluster interaction requires at least two sources",
        ));
    }
    let mut components = reserved_empty(
        reservation,
        categorical_indices.len(),
        "unable to reserve prepared cluster component views",
    )?;
    for index in categorical_indices {
        let source = ols
            .categorical_sources()
            .get(*index)
            .ok_or_else(|| invalid_argument("prepared cluster categorical source disappeared"))?;
        push_reserved(
            &mut components,
            crate::fe::CategoricalCodeView::new(
                source.codes(ols.fe_topology())?,
                source.n_levels(ols.fe_topology())?,
            ),
            "prepared cluster component-view capacity exhausted",
        )?;
    }
    if let Some(encoded) = crate::fe::try_factorize_dense_code_interaction(
        nobs,
        components.iter().copied(),
        &mut ClusterInteractionReservation(reservation),
    )? {
        let groups = encoded.n_levels();
        return Ok((encoded.into_codes(), groups));
    }
    let mut order = reserved_empty(
        reservation,
        nobs,
        "unable to reserve prepared cluster interaction ordering",
    )?;
    for row in 0..nobs {
        push_reserved(
            &mut order,
            row,
            "prepared cluster interaction ordering capacity exhausted",
        )?;
    }
    order.sort_unstable_by(|left, right| {
        compare_rows(&components, *left, *right).then_with(|| left.cmp(right))
    });
    let mut codes = reserved_filled(
        reservation,
        nobs,
        0_u32,
        "unable to reserve prepared cluster interaction codes",
    )?;
    let mut groups = 0_usize;
    let mut previous = None;
    for row in order {
        if previous.is_none_or(|prior| compare_rows(&components, prior, row) != Ordering::Equal) {
            groups = groups
                .checked_add(1)
                .ok_or_else(|| allocation("prepared cluster interaction group count overflows"))?;
            if groups > u32::MAX as usize {
                return Err(allocation(
                    "prepared cluster interaction group count exceeds u32",
                ));
            }
            previous = Some(row);
        }
        codes[row] = u32::try_from(groups - 1)
            .map_err(|_| allocation("prepared cluster interaction code exceeds u32"))?;
    }
    Ok((codes, groups))
}

struct ClusterInteractionReservation<'a, R>(&'a mut R);

impl<R: ClusterReservation> crate::fe::InteractionReservation
    for ClusterInteractionReservation<'_, R>
{
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        self.0.reserve_vec(target, elements, label)
    }
}

fn compare_rows(
    components: &[crate::fe::CategoricalCodeView<'_>],
    left: usize,
    right: usize,
) -> Ordering {
    for source in components {
        let codes = source.codes();
        let ordering = codes[left].cmp(&codes[right]);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn certify_prepared_clusters(prepared: &PreparedOlsData) -> Result<()> {
    if prepared.uses_categorical_preparation() {
        return certify_categorical_clusters(prepared);
    }
    let nobs = prepared.sample.n_active();
    if !(1..=10).contains(&prepared.clusters().len()) {
        return Err(invalid_argument(
            "prepared cluster dimension count is outside the certified range",
        ));
    }
    for dimension in 0..prepared.clusters().len() {
        let metadata = &prepared.clusters().dimensions[dimension];
        let codes = prepared.cluster_codes(dimension)?;
        if metadata.groups < 2
            || codes.len() != nobs
            || codes.iter().any(|code| *code as usize >= metadata.groups)
        {
            return Err(invalid_data(
                "prepared cluster codes or group count failed certification",
            ));
        }
        let mut seen = Vec::new();
        seen.try_reserve_exact(metadata.groups)
            .map_err(|_| allocation("unable to reserve prepared cluster certificate groups"))?;
        seen.resize(metadata.groups, false);
        for code in codes {
            seen[*code as usize] = true;
        }
        if seen.iter().any(|present| !present) {
            return Err(invalid_data(
                "prepared cluster codes are not contiguous on the final sample",
            ));
        }
    }
    Ok(())
}

fn reserved_empty<T, R: ClusterReservation>(
    reservation: &mut R,
    elements: usize,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "prepared cluster reservation returned insufficient capacity",
        ));
    }
    Ok(output)
}

fn reserved_filled<T: Clone, R: ClusterReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = reserved_empty(reservation, elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_string_copy<R: ClusterReservation>(
    value: &str,
    reservation: &mut R,
    label: &'static str,
) -> Result<String> {
    let mut output = String::new();
    reservation.reserve_string(&mut output, value.len(), label)?;
    if output.capacity() < value.len() {
        return Err(allocation(
            "prepared cluster string reservation returned insufficient capacity",
        ));
    }
    output.push_str(value);
    Ok(output)
}

/// Direct host construction from borrowed grammar and optional cluster terms.
/// The source set participates in availability before singleton finalization.
pub fn prepare_model_owned(
    request: &super::OlsRequestV1,
    clusters: &[OlsClusterTermV1],
    loaded: LoadedOlsInput,
    categorical: bool,
) -> Result<PreparedOlsData> {
    prepare_model_with(request, clusters, |sources| {
        if categorical {
            crate::prepared_categorical_data::prepare_categorical_ols_data_owned_with_cluster_sources(
                request, loaded, sources,
            )
        } else {
            prepare_ols_data_owned_with_cluster_sources(request, loaded, sources)
        }
    })
}

/// Borrowed counterpart of [`prepare_model_owned`]. The loaded columns stay
/// with the caller; PPML's preparation uses the same borrowed kernel to
/// re-prepare every separation generation from one retained input.
pub fn prepare_model(
    request: &super::OlsRequestV1,
    clusters: &[OlsClusterTermV1],
    loaded: &LoadedOlsInput,
    categorical: bool,
) -> Result<PreparedOlsData> {
    prepare_model_with(request, clusters, |sources| {
        if categorical {
            crate::prepared_categorical_data::prepare_categorical_ols_data_with_cluster_sources(
                request, loaded, sources,
            )
        } else {
            prepare_ols_data_with_cluster_sources(request, loaded, sources)
        }
    })
}

fn prepare_model_with(
    request: &super::OlsRequestV1,
    clusters: &[OlsClusterTermV1],
    prepare: impl FnOnce(&[SourceId]) -> Result<PreparedOlsData>,
) -> Result<PreparedOlsData> {
    let cluster_request = ClusterRequest {
        ols: request,
        clusters,
    };
    let sources = if clusters.is_empty() {
        Vec::new()
    } else {
        validate_cluster_request(&cluster_request)?
    };
    let mut prepared = prepare(&sources)?;
    if !clusters.is_empty() {
        prepared.clusters = Some(build_cluster_design(
            &cluster_request,
            &prepared,
            &mut SystemClusterReservation,
        )?);
        certify_prepared_clusters(&prepared)?;
    }
    Ok(prepared)
}

/// Preflight the same original grammar/source roles without copied requests.
pub fn compile_prepared_model(
    request: &super::OlsRequestV1,
    clusters: &[OlsClusterTermV1],
    categorical: bool,
) -> Result<super::PreparedOlsModel> {
    if categorical {
        let _plan = crate::fe::plan_categorical_absorbed_request(request)?;
    }
    let sources = if clusters.is_empty() {
        Vec::new()
    } else {
        validate_cluster_request(&ClusterRequest {
            ols: request,
            clusters,
        })?
    };
    request.compile_model(&sources, categorical)
}

fn certify_categorical_clusters(prepared: &PreparedOlsData) -> Result<()> {
    prepared.validate()?;
    let nobs = prepared.sample().n_active();
    if !(1..=10).contains(&prepared.clusters().len()) {
        return Err(invalid_argument(
            "categorical cluster dimension count is outside the certified range",
        ));
    }
    let mut reservation = SystemClusterReservation;
    for (dimension_index, dimension) in prepared.clusters().dimensions.iter().enumerate() {
        if dimension.label.is_empty() || dimension.sources.is_empty() || dimension.groups < 2 {
            return Err(invalid_argument(
                "categorical cluster metadata is structurally incomplete",
            ));
        }
        let source_indices =
            resolve_source_indices(prepared, &dimension.sources, &mut reservation)?;
        let codes = prepared.cluster_codes(dimension_index)?;
        if codes.len() != nobs || codes.iter().any(|code| *code as usize >= dimension.groups) {
            return Err(invalid_data(
                "categorical cluster codes or group count failed certification",
            ));
        }
        match (&dimension.codes, source_indices.as_slice()) {
            (
                PreparedOlsClusterCodes::CategoricalSource {
                    categorical_source_index,
                },
                [expected],
            ) if categorical_source_index == expected => {
                let source = prepared
                    .ols()
                    .categorical_sources()
                    .get(*expected)
                    .ok_or_else(|| {
                        invalid_argument("certified simple cluster source disappeared")
                    })?;
                if source.n_levels(prepared.fe_topology())? != dimension.groups
                    || !std::ptr::eq(
                        codes.as_ptr(),
                        source.codes(prepared.fe_topology())?.as_ptr(),
                    )
                {
                    return Err(invalid_argument(
                        "categorical simple cluster is not the authoritative zero-copy source",
                    ));
                }
            }
            (PreparedOlsClusterCodes::Interaction(_), indices) if indices.len() >= 2 => {
                let (expected_codes, expected_groups) =
                    factorize_interaction(prepared, indices, &mut reservation)?;
                if expected_groups != dimension.groups || expected_codes != codes {
                    return Err(invalid_data(
                        "categorical cluster interaction differs from its source tuple partition",
                    ));
                }
            }
            _ => {
                return Err(invalid_argument(
                    "categorical cluster storage kind and source recipe disagree",
                ));
            }
        }
        let mut seen = reserved_filled(
            &mut reservation,
            dimension.groups,
            false,
            "unable to reserve categorical cluster certificate groups",
        )?;
        for code in codes {
            seen[*code as usize] = true;
        }
        if seen.iter().any(|present| !present) {
            return Err(invalid_data(
                "categorical cluster codes are not contiguous on the final sample",
            ));
        }
    }
    Ok(())
}

fn resolve_source_indices<R: ClusterReservation>(
    ols: &PreparedOlsData,
    sources: &[SourceId],
    reservation: &mut R,
) -> Result<Vec<usize>> {
    let mut indices = reserved_empty(
        reservation,
        sources.len(),
        "unable to reserve prepared cluster source-index recipe",
    )?;
    for source in sources {
        let index = ols
            .categorical_sources()
            .iter()
            .position(|entry| entry.source() == *source)
            .ok_or_else(|| invalid_argument("prepared cluster source disappeared"))?;
        push_reserved(
            &mut indices,
            index,
            "prepared cluster source-index capacity exhausted",
        )?;
    }
    Ok(indices)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use crate::ols::{LoadedNumericColumn, OlsInputBinding, OlsInputRole};

    struct CountingReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize: bool,
    }

    impl CountingReservation {
        const fn succeeds() -> Self {
            Self {
                calls: 0,
                fail_at: None,
                undersize: false,
            }
        }

        const fn fails_at(call: usize) -> Self {
            Self {
                calls: 0,
                fail_at: Some(call),
                undersize: false,
            }
        }

        const fn undersized() -> Self {
            Self {
                calls: 0,
                fail_at: None,
                undersize: true,
            }
        }

        fn requested(&mut self) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                Err(allocation("injected prepared cluster reservation failure"))
            } else {
                Ok(())
            }
        }
    }

    impl ClusterReservation for CountingReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.requested()?;
            let requested = if self.undersize && elements > 0 {
                elements - 1
            } else {
                elements
            };
            target
                .try_reserve_exact(requested)
                .map_err(|_| allocation("test prepared cluster reservation failed"))
        }

        fn reserve_string(
            &mut self,
            target: &mut String,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.requested()?;
            let requested = if self.undersize && elements > 0 {
                elements - 1
            } else {
                elements
            };
            target
                .try_reserve_exact(requested)
                .map_err(|_| allocation("test prepared cluster string reservation failed"))
        }
    }

    #[test]
    fn owned_cluster_weights_preserve_missingness_singletons_and_final_codes() {
        for role in [
            None,
            Some(OlsInputRole::AnalyticWeight),
            Some(OlsInputRole::FrequencyWeight),
            Some(OlsInputRole::ProbabilityWeight),
        ] {
            let (mut request, mut loaded) = fixture();
            request.ols.keep_singletons = false;
            request.ols.absorbed_terms = vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(2).unwrap()],
                slope_sources: vec![],
                label: "a".into(),
                saved_name: String::new(),
            }];
            loaded.numeric_sources[2].1.missing[0] = true;
            loaded.numeric_sources[2].1.missing[6] = true;
            loaded.candidate[7] = false;
            if let Some(role) = role {
                request.ols.bindings.push(OlsInputBinding {
                    stata_var_position: 6,
                    role,
                });
                let weight = column(vec![2.0; 8]);
                match role {
                    OlsInputRole::AnalyticWeight => loaded.analytic_weight = Some(weight),
                    OlsInputRole::FrequencyWeight => loaded.frequency_weight = Some(weight),
                    OlsInputRole::ProbabilityWeight => loaded.probability_weight = Some(weight),
                    _ => unreachable!(),
                }
            }
            let expected = prepare_model(&request.ols, &request.clusters, &loaded, false).unwrap();
            let actual =
                prepare_model_owned(&request.ols, &request.clusters, loaded, false).unwrap();
            assert_eq!(expected, actual);
            assert_eq!(
                actual.sample.drop_reason(0),
                Some(crate::DropReason::MissingCluster)
            );
            assert_eq!(
                actual.sample.drop_reason(6),
                Some(crate::DropReason::MissingCluster)
            );
            assert_eq!(
                actual.sample.drop_reason(1),
                Some(if role == Some(OlsInputRole::FrequencyWeight) {
                    crate::DropReason::Retained
                } else {
                    crate::DropReason::Singleton
                })
            );
        }
    }

    #[test]
    fn interaction_factorization_is_row_order_invariant_and_collision_free() {
        let (request, loaded) = fixture();
        let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
            .expect("prepared interaction");
        assert_eq!(prepared.clusters().dimensions.len(), 1);
        assert_eq!(prepared.clusters().dimensions[0].groups, 8);
        let codes = prepared.cluster_codes(0).expect("interaction codes");
        let mut mapping = (0..codes.len())
            .map(|row| {
                (
                    loaded.numeric_sources[1].1.values[row].to_bits(),
                    loaded.numeric_sources[2].1.values[row].to_bits(),
                    codes[row],
                )
            })
            .collect::<Vec<_>>();
        mapping.sort_unstable();

        let permutation = [7_usize, 0, 5, 2, 6, 1, 4, 3];
        let mut permuted = loaded.clone();
        permuted.candidate = permutation
            .iter()
            .map(|row| loaded.candidate[*row])
            .collect();
        permuted.outcome.values = permutation
            .iter()
            .map(|row| loaded.outcome.values[*row])
            .collect();
        permuted.outcome.missing = permutation
            .iter()
            .map(|row| loaded.outcome.missing[*row])
            .collect();
        for (target, (_, source)) in permuted
            .numeric_sources
            .iter_mut()
            .zip(&loaded.numeric_sources)
        {
            target.1.values = permutation.iter().map(|row| source.values[*row]).collect();
            target.1.missing = permutation.iter().map(|row| source.missing[*row]).collect();
        }
        let permuted_prepared = prepare_model(&request.ols, &request.clusters, &permuted, false)
            .expect("permuted interaction");
        let permuted_codes = permuted_prepared
            .cluster_codes(0)
            .expect("permuted interaction codes");
        let mut permuted_mapping = (0..permuted_codes.len())
            .map(|row| {
                (
                    permuted.numeric_sources[1].1.values[row].to_bits(),
                    permuted.numeric_sources[2].1.values[row].to_bits(),
                    permuted_codes[row],
                )
            })
            .collect::<Vec<_>>();
        permuted_mapping.sort_unstable();
        assert_eq!(mapping, permuted_mapping);
    }

    #[test]
    fn every_cluster_design_reservation_failure_and_undersize_fails_closed() {
        let (request, loaded) = fixture();
        let sources = validate_request(&request).expect("cluster sources");
        let ols = prepare_ols_data_with_cluster_sources(&request.ols, &loaded, &sources)
            .expect("prepared OLS");
        let mut count = CountingReservation::succeeds();
        build_cluster_design(&request.borrowed(), &ols, &mut count).expect("count reservations");
        assert!(count.calls >= 6);
        for fail_at in 0..count.calls {
            let ols = prepare_ols_data_with_cluster_sources(&request.ols, &loaded, &sources)
                .expect("prepared OLS");
            let error = build_cluster_design(
                &request.borrowed(),
                &ols,
                &mut CountingReservation::fails_at(fail_at),
            )
            .expect_err("injected cluster allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation);
        }
        let ols = prepare_ols_data_with_cluster_sources(&request.ols, &loaded, &sources)
            .expect("prepared OLS");
        let error = build_cluster_design(
            &request.borrowed(),
            &ols,
            &mut CountingReservation::undersized(),
        )
        .expect_err("undersized cluster allocation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn corrupted_interaction_codes_and_group_metadata_fail_closed() {
        let (request, loaded) = fixture();
        let mut prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
            .expect("prepared clusters");
        let groups = prepared.clusters().dimensions[0].groups;
        let codes = match &mut prepared.clusters.as_mut().unwrap().dimensions[0].codes {
            PreparedOlsClusterCodes::Interaction(codes) => codes,
            PreparedOlsClusterCodes::CategoricalSource { .. } => {
                panic!("fixture must use interaction codes")
            }
        };
        let original = codes[0];
        codes[0] = u32::try_from(groups).expect("small group count");
        let error = certify_prepared_clusters(&prepared)
            .expect_err("out-of-domain interaction code must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);

        let codes = match &mut prepared.clusters.as_mut().unwrap().dimensions[0].codes {
            PreparedOlsClusterCodes::Interaction(codes) => codes,
            PreparedOlsClusterCodes::CategoricalSource { .. } => unreachable!(),
        };
        codes[0] = original;
        prepared.clusters.as_mut().unwrap().dimensions[0].groups += 1;
        let error = certify_prepared_clusters(&prepared)
            .expect_err("noncontiguous group metadata must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn ppml_metadata_validation_admits_intercept_slope_plus_intercept() {
        let (mut request, _) = fixture();
        let x = SourceId::new(1).expect("x");
        let first = SourceId::new(2).expect("first FE");
        let second = SourceId::new(3).expect("second FE");
        request.ols.include_intercept = true;
        request.ols.absorbed_terms = vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("first absorb term"),
                intercept: true,
                categorical_sources: vec![first],
                slope_sources: vec![x],
                label: "first##c.x".to_owned(),
                saved_name: "saved_first".to_owned(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).expect("second absorb term"),
                intercept: true,
                categorical_sources: vec![second],
                slope_sources: Vec::new(),
                label: "second".to_owned(),
                saved_name: String::new(),
            },
        ];
        let error = request
            .validate_and_compile()
            .expect_err("developer OLS validator must retain its slope restriction");
        assert_eq!(error.code, ErrorCode::Unsupported);
        let model = request
            .validate_ppml_and_compile()
            .expect("PPML validator must admit declared absorbed slopes");
        assert_eq!(model.coefficients.len(), 2);
    }

    pub(crate) fn fixture() -> (OlsClusteredRequestV1, LoadedOlsInput) {
        let x = SourceId::new(1).expect("x");
        let first = SourceId::new(2).expect("first");
        let second = SourceId::new(3).expect("second");
        let term = ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.x".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source: x,
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        };
        let request = OlsClusteredRequestV1 {
            ols: OlsRequestV1 {
                model: CompiledTermGraph {
                    sources: vec![
                        SourceEntry {
                            id: x,
                            canonical_name: "x".to_owned(),
                        },
                        SourceEntry {
                            id: first,
                            canonical_name: "a".to_owned(),
                        },
                        SourceEntry {
                            id: second,
                            canonical_name: "b".to_owned(),
                        },
                    ],
                    terms: vec![term],
                },
                absorbed_terms: Vec::new(),
                bindings: vec![
                    OlsInputBinding {
                        stata_var_position: 1,
                        role: OlsInputRole::Candidate,
                    },
                    OlsInputBinding {
                        stata_var_position: 2,
                        role: OlsInputRole::Outcome,
                    },
                    OlsInputBinding {
                        stata_var_position: 3,
                        role: OlsInputRole::NumericSource(x),
                    },
                    OlsInputBinding {
                        stata_var_position: 4,
                        role: OlsInputRole::NumericSource(first),
                    },
                    OlsInputBinding {
                        stata_var_position: 5,
                        role: OlsInputRole::NumericSource(second),
                    },
                ],
                include_intercept: false,
                keep_singletons: true,
            },
            clusters: vec![OlsClusterTermV1 {
                label: "a#b".to_owned(),
                sources: vec![first, second],
            }],
        };
        let loaded = LoadedOlsInput {
            candidate: vec![true; 8],
            outcome: column(vec![1.0, 2.0, 2.5, 4.0, 5.0, 6.5, 7.0, 8.0]),
            numeric_sources: vec![
                (x, column(vec![-2.0, -1.0, 0.0, 1.0, -1.5, 0.5, 2.0, 3.0])),
                (
                    first,
                    column(vec![-2.5, -2.5, 1.25, 1.25, 4.5, 4.5, -2.5, 1.25]),
                ),
                (second, column(vec![0.1, 0.2, 0.1, 0.3, 0.2, 0.4, 0.3, 0.4])),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        (request, loaded)
    }

    fn column(values: Vec<f64>) -> LoadedNumericColumn {
        LoadedNumericColumn {
            missing: vec![false; values.len()],
            values,
        }
    }
}
