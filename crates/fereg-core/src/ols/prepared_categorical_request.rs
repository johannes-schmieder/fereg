//! Request planning for native numeric categorical absorbed interactions.
//!
//! Existing numeric OLS preparation already implements authoritative candidate
//! selection, missing-value accounting, model recipes, and weights, but its
//! absorbed-effect representation is deliberately single-source. This module
//! creates a metadata-only availability request containing one synthetic
//! single-source absorbed term for every unique categorical source used by the
//! requested effects. Running that request with `keep_singletons=true` preserves
//! the existing source-missingness boundary without applying the wrong
//! single-source singleton rule. The original effect identities and source
//! tuples are retained separately for the later interaction-aware topology pass.

use crate::error::{ErrorCode, Result, FeregError};
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ExpandedTerm, SourceEntry, SourceId,
    TermComponent,
};

use super::{
    OlsInputBinding, OlsInputRole, OlsRequestV1, PreparedCategoricalEffectRequest,
    PreparedCategoricalSource,
};

/// Original identity of one requested intercept-only categorical absorbed term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoricalAbsorbEffectIdentity {
    pub term_id: AbsorbTermId,
    pub label: String,
    /// Requested wire order is preserved exactly. Numerical topology code may
    /// canonicalize only a temporary index recipe.
    pub sources: Vec<SourceId>,
}

/// Metadata-only plan that separates source availability from true FE topology.
#[derive(Debug, PartialEq)]
pub struct CategoricalAbsorbRequestPlan {
    availability_request: OlsRequestV1,
    requested_effects: Vec<CategoricalAbsorbEffectIdentity>,
    required_sources: Vec<SourceId>,
    requested_keep_singletons: bool,
    model_source_count: usize,
}

impl CategoricalAbsorbRequestPlan {
    /// Request accepted by the existing numeric sample-preparation boundary.
    /// Its synthetic terms are source-availability markers only, and singleton
    /// pruning is always disabled there.
    #[must_use]
    pub const fn availability_request(&self) -> &OlsRequestV1 {
        &self.availability_request
    }

    #[must_use]
    pub fn requested_effects(&self) -> &[CategoricalAbsorbEffectIdentity] {
        &self.requested_effects
    }

    #[must_use]
    pub fn required_sources(&self) -> &[SourceId] {
        &self.required_sources
    }

    #[must_use]
    pub const fn requested_keep_singletons(&self) -> bool {
        self.requested_keep_singletons
    }

    /// Bind original requested source tuples to a prepared categorical-source
    /// table after authoritative numeric sample preparation.
    pub fn bind_effects(
        &self,
        prepared_sources: &[PreparedCategoricalSource],
    ) -> Result<Vec<PreparedCategoricalEffectRequest>> {
        let mut reservation = SystemRequestPlanReservation;
        self.bind_effects_with_reservation(prepared_sources, &mut reservation)
    }

    fn bind_effects_with_reservation<R: RequestPlanReservation>(
        &self,
        prepared_sources: &[PreparedCategoricalSource],
        reservation: &mut R,
    ) -> Result<Vec<PreparedCategoricalEffectRequest>> {
        let mut source_positions = reserved_filled(
            reservation,
            self.model_source_count,
            None,
            "unable to reserve prepared categorical source positions",
        )?;
        for (prepared_index, source) in prepared_sources.iter().enumerate() {
            let model_index = source_index(source.source, self.model_source_count)?;
            if source.prepared_source_index != model_index {
                return Err(invalid(
                    "prepared categorical source index does not match its model source id",
                ));
            }
            if source_positions[model_index].replace(prepared_index).is_some() {
                return Err(invalid(
                    "prepared categorical source table contains a duplicate source id",
                ));
            }
        }
        for source in &self.required_sources {
            if source_positions[source_index(*source, self.model_source_count)?].is_none() {
                return Err(invalid(
                    "prepared categorical source table is missing a required absorbed source",
                ));
            }
        }

        let mut output = Vec::new();
        reservation.reserve_vec(
            &mut output,
            self.requested_effects.len(),
            "unable to reserve bound categorical absorbed effects",
        )?;
        for effect in &self.requested_effects {
            let label = fallible_string_copy(
                &effect.label,
                reservation,
                "unable to reserve bound categorical absorbed label",
            )?;
            let sources = fallible_copy_slice(
                &effect.sources,
                reservation,
                "unable to reserve bound categorical absorbed sources",
            )?;
            let mut categorical_source_indices = Vec::new();
            reservation.reserve_vec(
                &mut categorical_source_indices,
                effect.sources.len(),
                "unable to reserve bound categorical absorbed source indices",
            )?;
            for source in &effect.sources {
                let position = source_positions[source_index(*source, self.model_source_count)?]
                    .ok_or_else(|| {
                        invalid(
                            "prepared categorical source table lost a required absorbed source",
                        )
                    })?;
                push_reserved(
                    &mut categorical_source_indices,
                    position,
                    "bound categorical absorbed source-index capacity exhausted",
                )?;
            }
            push_reserved(
                &mut output,
                PreparedCategoricalEffectRequest::new(
                    effect.term_id,
                    label,
                    sources,
                    categorical_source_indices,
                )?,
                "bound categorical absorbed effect capacity exhausted",
            )?;
        }
        Ok(output)
    }
}

/// Validate and split a numeric intercept-only categorical absorbed request.
pub fn plan_categorical_absorbed_request(
    request: &OlsRequestV1,
) -> Result<CategoricalAbsorbRequestPlan> {
    let mut reservation = SystemRequestPlanReservation;
    plan_categorical_absorbed_request_with_reservation(request, &mut reservation)
}

trait RequestPlanReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemRequestPlanReservation;

impl RequestPlanReservation for SystemRequestPlanReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

fn plan_categorical_absorbed_request_with_reservation<R: RequestPlanReservation>(
    request: &OlsRequestV1,
    reservation: &mut R,
) -> Result<CategoricalAbsorbRequestPlan> {
    request.model.validate_executable_metadata()?;
    validate_bindings(request, reservation)?;
    if request.absorbed_terms.is_empty() {
        return Err(unsupported(
            "categorical absorbed request planning requires explicit absorbed terms",
        ));
    }

    let source_count = request.model.sources.len();
    let mut required_marker = reserved_filled(
        reservation,
        source_count,
        false,
        "unable to reserve categorical absorbed source marker",
    )?;
    let mut requested_effects = Vec::new();
    reservation.reserve_vec(
        &mut requested_effects,
        request.absorbed_terms.len(),
        "unable to reserve categorical absorbed effect identities",
    )?;
    for (term_index, term) in request.absorbed_terms.iter().enumerate() {
        let expected_id = u32::try_from(term_index + 1)
            .map_err(|_| invalid("categorical absorbed-term count exceeds u32"))?;
        if term.id.get() != expected_id {
            return Err(invalid(
                "categorical absorbed-term ids must be contiguous and ordered",
            ));
        }
        if !term.intercept
            || term.categorical_sources.is_empty()
            || !term.slope_sources.is_empty()
            || !term.saved_name.is_empty()
        {
            return Err(unsupported(
                "categorical absorbed request planning supports only unnamed intercept-only terms without slopes or saved outputs",
            ));
        }
        if term.label.trim().is_empty() {
            return Err(invalid(
                "categorical absorbed effect label must be nonempty",
            ));
        }
        for source in &term.categorical_sources {
            required_marker[source_index(*source, source_count)?] = true;
        }
        push_reserved(
            &mut requested_effects,
            CategoricalAbsorbEffectIdentity {
                term_id: term.id,
                label: fallible_string_copy(
                    &term.label,
                    reservation,
                    "unable to reserve categorical absorbed effect label",
                )?,
                sources: fallible_copy_slice(
                    &term.categorical_sources,
                    reservation,
                    "unable to reserve categorical absorbed effect sources",
                )?,
            },
            "categorical absorbed effect-identity capacity exhausted",
        )?;
    }

    let required_count = required_marker.iter().filter(|required| **required).count();
    let mut required_sources = Vec::new();
    reservation.reserve_vec(
        &mut required_sources,
        required_count,
        "unable to reserve required categorical absorbed sources",
    )?;
    let mut availability_terms = Vec::new();
    reservation.reserve_vec(
        &mut availability_terms,
        required_count,
        "unable to reserve categorical source-availability terms",
    )?;
    for (model_index, source) in request.model.sources.iter().enumerate() {
        if !required_marker[model_index] {
            continue;
        }
        push_reserved(
            &mut required_sources,
            source.id,
            "required categorical absorbed source capacity exhausted",
        )?;
        let mut categorical_sources = Vec::new();
        reservation.reserve_vec(
            &mut categorical_sources,
            1,
            "unable to reserve source-availability categorical source",
        )?;
        push_reserved(
            &mut categorical_sources,
            source.id,
            "source-availability categorical source capacity exhausted",
        )?;
        let availability_id = u32::try_from(availability_terms.len() + 1)
            .map_err(|_| invalid("source-availability term count exceeds u32"))?;
        push_reserved(
            &mut availability_terms,
            AbsorbedTerm {
                id: AbsorbTermId::new(availability_id)?,
                intercept: true,
                categorical_sources,
                slope_sources: Vec::new(),
                label: fallible_string_copy(
                    &source.canonical_name,
                    reservation,
                    "unable to reserve source-availability label",
                )?,
                saved_name: String::new(),
            },
            "source-availability term capacity exhausted",
        )?;
    }
    if required_sources.is_empty() {
        return Err(invalid(
            "categorical absorbed request has no valid source identities",
        ));
    }

    let availability_request = OlsRequestV1 {
        model: clone_model(&request.model, reservation)?,
        absorbed_terms: availability_terms,
        bindings: fallible_copy_slice(
            &request.bindings,
            reservation,
            "unable to reserve source-availability input bindings",
        )?,
        include_intercept: request.include_intercept,
        // True here means the existing single-source preparation path validates
        // source availability but does not delete observations. The original
        // choice is applied later to the true interaction topology.
        keep_singletons: true,
    };

    Ok(CategoricalAbsorbRequestPlan {
        availability_request,
        requested_effects,
        required_sources,
        requested_keep_singletons: request.keep_singletons,
        model_source_count: source_count,
    })
}

fn validate_bindings<R: RequestPlanReservation>(
    request: &OlsRequestV1,
    reservation: &mut R,
) -> Result<()> {
    let mut positions = Vec::new();
    reservation.reserve_vec(
        &mut positions,
        request.bindings.len(),
        "unable to reserve categorical request binding positions",
    )?;
    let mut source_counts = reserved_filled(
        reservation,
        request.model.sources.len(),
        0_u8,
        "unable to reserve categorical request source binding counts",
    )?;
    let mut candidate_count = 0_u8;
    let mut outcome_count = 0_u8;
    let mut weight_count = 0_u8;
    for binding in &request.bindings {
        if binding.stata_var_position == 0 || positions.contains(&binding.stata_var_position) {
            return Err(invalid(
                "categorical request bindings require unique positive Stata positions",
            ));
        }
        push_reserved(
            &mut positions,
            binding.stata_var_position,
            "categorical request binding-position capacity exhausted",
        )?;
        match binding.role {
            OlsInputRole::Candidate => candidate_count = checked_increment(candidate_count)?,
            OlsInputRole::Outcome => outcome_count = checked_increment(outcome_count)?,
            OlsInputRole::NumericSource(source) => {
                let index = source_index(source, request.model.sources.len())?;
                source_counts[index] = checked_increment(source_counts[index])?;
            }
            OlsInputRole::AnalyticWeight
            | OlsInputRole::FrequencyWeight
            | OlsInputRole::ProbabilityWeight => weight_count = checked_increment(weight_count)?,
        }
    }
    if candidate_count != 1 || outcome_count != 1 || weight_count > 1 {
        return Err(invalid(
            "categorical request requires one candidate, one outcome, and at most one weight binding",
        ));
    }
    if source_counts.iter().any(|count| *count != 1) {
        return Err(invalid(
            "categorical request must bind every model source exactly once",
        ));
    }
    Ok(())
}

fn clone_model<R: RequestPlanReservation>(
    model: &CompiledTermGraph,
    reservation: &mut R,
) -> Result<CompiledTermGraph> {
    let mut sources = Vec::new();
    reservation.reserve_vec(
        &mut sources,
        model.sources.len(),
        "unable to reserve source-availability model sources",
    )?;
    for source in &model.sources {
        push_reserved(
            &mut sources,
            SourceEntry {
                id: source.id,
                canonical_name: fallible_string_copy(
                    &source.canonical_name,
                    reservation,
                    "unable to reserve source-availability source name",
                )?,
            },
            "source-availability model-source capacity exhausted",
        )?;
    }

    let mut terms = Vec::new();
    reservation.reserve_vec(
        &mut terms,
        model.terms.len(),
        "unable to reserve source-availability model terms",
    )?;
    for term in &model.terms {
        let mut components = Vec::new();
        reservation.reserve_vec(
            &mut components,
            term.components.len(),
            "unable to reserve source-availability term components",
        )?;
        for component in &term.components {
            push_reserved(
                &mut components,
                clone_component(component, reservation)?,
                "source-availability term-component capacity exhausted",
            )?;
        }
        push_reserved(
            &mut terms,
            ExpandedTerm {
                id: term.id,
                order: term.order,
                coefficient_slot: term.coefficient_slot,
                raw_kind: term.raw_kind,
                whole_column_omitted: term.whole_column_omitted,
                canonical_token: fallible_string_copy(
                    &term.canonical_token,
                    reservation,
                    "unable to reserve source-availability canonical token",
                )?,
                components,
            },
            "source-availability model-term capacity exhausted",
        )?;
    }
    Ok(CompiledTermGraph { sources, terms })
}

fn clone_component<R: RequestPlanReservation>(
    component: &TermComponent,
    reservation: &mut R,
) -> Result<TermComponent> {
    Ok(TermComponent {
        order: component.order,
        source: component.source,
        kind: component.kind,
        level_state: component.level_state,
        level_value: component.level_value,
        base: component.base,
        omitted: component.omitted,
        raw_operator: fallible_string_copy(
            &component.raw_operator,
            reservation,
            "unable to reserve source-availability component operator",
        )?,
        time_series_state: component.time_series_state,
        raw_time_series_operator: fallible_string_copy(
            &component.raw_time_series_operator,
            reservation,
            "unable to reserve source-availability time-series operator",
        )?,
    })
}

fn fallible_copy_slice<T: Copy, R: RequestPlanReservation>(
    input: &[T],
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, input.len(), label)?;
    extend_reserved(&mut output, input, label)?;
    Ok(output)
}

fn fallible_string_copy<R: RequestPlanReservation>(
    input: &str,
    reservation: &mut R,
    label: &'static str,
) -> Result<String> {
    let mut output = String::new();
    reservation.reserve_string(&mut output, input.len(), label)?;
    if output.capacity() < input.len() {
        return Err(allocation(label));
    }
    output.push_str(input);
    Ok(output)
}

fn reserved_filled<T: Clone, R: RequestPlanReservation>(
    reservation: &mut R,
    len: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, len, label)?;
    if output.capacity() < len {
        return Err(allocation(label));
    }
    output.resize(len, value);
    Ok(output)
}

fn extend_reserved<T: Copy>(target: &mut Vec<T>, values: &[T], label: &'static str) -> Result<()> {
    if target.capacity().saturating_sub(target.len()) < values.len() {
        return Err(allocation(label));
    }
    target.extend_from_slice(values);
    Ok(())
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn source_index(source: SourceId, source_count: usize) -> Result<usize> {
    let index = usize::try_from(source.get() - 1)
        .map_err(|_| invalid("categorical source id does not fit usize"))?;
    if index >= source_count {
        return Err(invalid(
            "categorical absorbed term references a source outside the model registry",
        ));
    }
    Ok(index)
}

fn checked_increment(value: u8) -> Result<u8> {
    value
        .checked_add(1)
        .ok_or_else(|| invalid("categorical request binding multiplicity exceeds u8"))
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn unsupported(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factorize::EncodedFactor;
    use crate::metadata::{SourceEntry, SourceId};

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        skip_at: Option<usize>,
    }

    impl RequestPlanReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected categorical request-plan allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical request-plan reservation failed"))
        }

        fn reserve_string(
            &mut self,
            target: &mut String,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected categorical request-plan string allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical request-plan string reservation failed"))
        }
    }

    fn source(value: u32) -> SourceId {
        SourceId::new(value).expect("source")
    }

    fn request() -> OlsRequestV1 {
        OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![
                    SourceEntry {
                        id: source(1),
                        canonical_name: "worker".to_owned(),
                    },
                    SourceEntry {
                        id: source(2),
                        canonical_name: "firm".to_owned(),
                    },
                    SourceEntry {
                        id: source(3),
                        canonical_name: "year".to_owned(),
                    },
                ],
                terms: Vec::new(),
            },
            absorbed_terms: vec![
                AbsorbedTerm {
                    id: AbsorbTermId::new(1).expect("term 1"),
                    intercept: true,
                    categorical_sources: vec![source(1)],
                    slope_sources: Vec::new(),
                    label: "worker".to_owned(),
                    saved_name: String::new(),
                },
                AbsorbedTerm {
                    id: AbsorbTermId::new(2).expect("term 2"),
                    intercept: true,
                    categorical_sources: vec![source(2), source(3)],
                    slope_sources: Vec::new(),
                    label: "firm#year".to_owned(),
                    saved_name: String::new(),
                },
            ],
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
                    role: OlsInputRole::NumericSource(source(1)),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(source(2)),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(source(3)),
                },
            ],
            include_intercept: true,
            keep_singletons: false,
        }
    }

    fn prepared(source_id: u32, prepared_source_index: usize) -> PreparedCategoricalSource {
        PreparedCategoricalSource {
            source: source(source_id),
            prepared_source_index,
            encoded: EncodedFactor {
                codes: vec![0, 1],
                levels: vec![0.0, 1.0],
            },
        }
    }

    #[test]
    fn availability_terms_follow_model_registry_and_preserve_original_effects() {
        let plan = plan_categorical_absorbed_request(&request()).expect("plan");
        let availability = plan.availability_request();
        assert!(availability.keep_singletons);
        assert!(availability.include_intercept);
        assert_eq!(
            availability
                .absorbed_terms
                .iter()
                .map(|term| term.categorical_sources.clone())
                .collect::<Vec<_>>(),
            vec![vec![source(1)], vec![source(2)], vec![source(3)]]
        );
        assert_eq!(
            availability
                .absorbed_terms
                .iter()
                .map(|term| term.id.get())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(plan.required_sources(), &[source(1), source(2), source(3)]);
        assert!(!plan.requested_keep_singletons());
        assert_eq!(plan.requested_effects()[1].label, "firm#year");
        assert_eq!(
            plan.requested_effects()[1].sources,
            vec![source(2), source(3)]
        );
    }

    #[test]
    fn binding_preserves_requested_tuple_order() {
        let mut request = request();
        request.absorbed_terms[1].categorical_sources = vec![source(3), source(2)];
        let plan = plan_categorical_absorbed_request(&request).expect("plan");
        let bound = plan
            .bind_effects(&[prepared(3, 2), prepared(1, 0), prepared(2, 1)])
            .expect("bound");
        assert_eq!(bound.len(), 2);
        assert_eq!(bound[1].sources, vec![source(3), source(2)]);
        assert_eq!(bound[1].categorical_source_indices, vec![0, 2]);
    }

    #[test]
    fn repeated_source_is_deduplicated_only_for_availability() {
        let mut request = request();
        request.absorbed_terms[1].categorical_sources =
            vec![source(2), source(3), source(2)];
        let plan = plan_categorical_absorbed_request(&request).expect("plan");
        assert_eq!(plan.availability_request().absorbed_terms.len(), 3);
        assert_eq!(
            plan.requested_effects()[1].sources,
            vec![source(2), source(3), source(2)]
        );
    }

    #[test]
    fn invalid_effect_surfaces_fail_closed() {
        let mut slope = request();
        slope.absorbed_terms[0].slope_sources.push(source(2));
        assert_eq!(
            plan_categorical_absorbed_request(&slope)
                .expect_err("slope")
                .code,
            ErrorCode::Unsupported
        );
        let mut saved = request();
        saved.absorbed_terms[0].saved_name = "alpha".to_owned();
        assert_eq!(
            plan_categorical_absorbed_request(&saved)
                .expect_err("saved")
                .code,
            ErrorCode::Unsupported
        );
        let mut empty_label = request();
        empty_label.absorbed_terms[0].label.clear();
        assert_eq!(
            plan_categorical_absorbed_request(&empty_label)
                .expect_err("empty label")
                .code,
            ErrorCode::InvalidArgument
        );
        let mut bad_source = request();
        bad_source.absorbed_terms[0].categorical_sources = vec![source(4)];
        assert_eq!(
            plan_categorical_absorbed_request(&bad_source)
                .expect_err("bad source")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn invalid_bindings_fail_before_plan_publication() {
        let mut duplicate = request();
        duplicate.bindings[4].role = OlsInputRole::NumericSource(source(2));
        assert_eq!(
            plan_categorical_absorbed_request(&duplicate)
                .expect_err("duplicate source")
                .code,
            ErrorCode::InvalidArgument
        );
        let mut duplicate_position = request();
        duplicate_position.bindings[4].stata_var_position = 4;
        assert_eq!(
            plan_categorical_absorbed_request(&duplicate_position)
                .expect_err("duplicate position")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn missing_duplicate_and_misaligned_prepared_sources_fail_closed() {
        let plan = plan_categorical_absorbed_request(&request()).expect("plan");
        assert_eq!(
            plan.bind_effects(&[prepared(1, 0), prepared(2, 1)])
                .expect_err("missing year")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            plan.bind_effects(&[prepared(1, 0), prepared(2, 1), prepared(2, 1)])
                .expect_err("duplicate firm")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            plan.bind_effects(&[prepared(1, 1), prepared(2, 1), prepared(3, 2)])
                .expect_err("misaligned worker")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn every_plan_reservation_and_capacity_guard_fails_closed() {
        let request = request();
        let mut successful = InjectedReservation::default();
        plan_categorical_absorbed_request_with_reservation(&request, &mut successful)
            .expect("successful allocation audit");
        assert!(successful.calls > 0);
        for call in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = plan_categorical_absorbed_request_with_reservation(
                &request,
                &mut injected,
            )
            .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
        for call in 0..successful.calls {
            let mut undersized = InjectedReservation {
                skip_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = plan_categorical_absorbed_request_with_reservation(
                &request,
                &mut undersized,
            )
            .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
    }

    #[test]
    fn every_binding_reservation_and_capacity_guard_fails_closed() {
        let plan = plan_categorical_absorbed_request(&request()).expect("plan");
        let prepared = [prepared(1, 0), prepared(2, 1), prepared(3, 2)];
        let mut successful = InjectedReservation::default();
        plan.bind_effects_with_reservation(&prepared, &mut successful)
            .expect("successful binding audit");
        assert!(successful.calls > 0);
        for call in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = plan
                .bind_effects_with_reservation(&prepared, &mut injected)
                .expect_err("injected binding allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
        for call in 0..successful.calls {
            let mut undersized = InjectedReservation {
                skip_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = plan
                .bind_effects_with_reservation(&prepared, &mut undersized)
                .expect_err("binding capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
    }
}
