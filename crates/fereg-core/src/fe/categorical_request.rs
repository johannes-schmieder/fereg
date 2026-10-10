//! Source-role planning borrows the original request. No fabricated model,
//! input bindings or intercept partitions are constructed for availability.

use crate::error::{allocation, invalid_argument as invalid, unsupported, Result};
use crate::memory::push_reserved;
use crate::memory::SystemVecReservation as SystemRequestPlanReservation;
use crate::memory::VecReservation as RequestPlanReservation;
use crate::metadata::SourceId;
use crate::ols::{OlsInputRole, OlsRequestV1, PreparedCategoricalSource};

/// Metadata-only plan that separates source availability from true FE topology.
#[derive(Debug, PartialEq)]
pub struct CategoricalAbsorbRequestPlan<'a> {
    request: &'a OlsRequestV1,
    required_sources: Vec<SourceId>,
}

impl CategoricalAbsorbRequestPlan<'_> {
    #[must_use]
    pub fn required_sources(&self) -> &[SourceId] {
        &self.required_sources
    }
}

pub(crate) fn bind_categorical_effect_recipes(
    request: &OlsRequestV1,
    prepared_sources: &[PreparedCategoricalSource],
) -> Result<Vec<Vec<usize>>> {
    bind_effect_recipes_with_reservation(
        request,
        prepared_sources,
        &mut SystemRequestPlanReservation,
    )
}

fn bind_effect_recipes_with_reservation<R: RequestPlanReservation>(
    request: &OlsRequestV1,
    prepared_sources: &[PreparedCategoricalSource],
    reservation: &mut R,
) -> Result<Vec<Vec<usize>>> {
    let mut source_positions = reserved_filled(
        reservation,
        request.model.sources.len(),
        None,
        "unable to reserve prepared categorical source positions",
    )?;
    for (prepared_index, source) in prepared_sources.iter().enumerate() {
        let model_index = source_index(source.source, request.model.sources.len())?;
        if source.prepared_source_index != model_index {
            return Err(invalid(
                "prepared categorical source index does not match its model source id",
            ));
        }
        if source_positions[model_index]
            .replace(prepared_index)
            .is_some()
        {
            return Err(invalid(
                "prepared categorical source table contains a duplicate source id",
            ));
        }
    }
    let mut output = Vec::new();
    reserve_vec(
        reservation,
        &mut output,
        request.absorbed_terms.len(),
        "unable to reserve bound categorical absorbed effects",
    )?;
    for effect in &request.absorbed_terms {
        let mut categorical_source_indices = Vec::new();
        reserve_vec(
            reservation,
            &mut categorical_source_indices,
            effect.categorical_sources.len(),
            "unable to reserve bound categorical absorbed source indices",
        )?;
        for source in &effect.categorical_sources {
            let position = source_positions[source_index(*source, request.model.sources.len())?]
                .ok_or_else(|| {
                    invalid("prepared categorical source table lost a required absorbed source")
                })?;
            push_reserved(
                &mut categorical_source_indices,
                position,
                "bound categorical absorbed source-index capacity exhausted",
            )?;
        }
        push_reserved(
            &mut output,
            categorical_source_indices,
            "bound categorical absorbed effect capacity exhausted",
        )?;
    }
    Ok(output)
}

/// Validate and split a numeric categorical absorbed request.
///
/// Intercept-only, intercept-plus-slope, and slope-only terms share the same
/// categorical topology. Slope columns remain borrowed numeric sources.
pub fn plan_categorical_absorbed_request(
    request: &OlsRequestV1,
) -> Result<CategoricalAbsorbRequestPlan<'_>> {
    let mut reservation = SystemRequestPlanReservation;
    plan_categorical_absorbed_request_with_reservation(request, &mut reservation)
}

fn plan_categorical_absorbed_request_with_reservation<'a, R: RequestPlanReservation>(
    request: &'a OlsRequestV1,
    reservation: &mut R,
) -> Result<CategoricalAbsorbRequestPlan<'a>> {
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
    for (term_index, term) in request.absorbed_terms.iter().enumerate() {
        let expected_id = u32::try_from(term_index + 1)
            .map_err(|_| invalid("categorical absorbed-term count exceeds u32"))?;
        if term.id.get() != expected_id {
            return Err(invalid(
                "categorical absorbed-term ids must be contiguous and ordered",
            ));
        }
        if term.categorical_sources.is_empty() || (!term.intercept && term.slope_sources.is_empty())
        {
            return Err(unsupported(
                "categorical absorbed request planning requires categorical terms containing an intercept or slope",
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
    }

    let required_count = required_marker.iter().filter(|required| **required).count();
    let mut required_sources = Vec::new();
    reserve_vec(
        reservation,
        &mut required_sources,
        required_count,
        "unable to reserve required categorical absorbed sources",
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
    }
    if required_sources.is_empty() {
        return Err(invalid(
            "categorical absorbed request has no valid source identities",
        ));
    }

    Ok(CategoricalAbsorbRequestPlan {
        request,
        required_sources,
    })
}

fn validate_bindings<R: RequestPlanReservation>(
    request: &OlsRequestV1,
    reservation: &mut R,
) -> Result<()> {
    let mut positions = Vec::new();
    reserve_vec(
        reservation,
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

fn reserve_vec<T, R: RequestPlanReservation>(
    reservation: &mut R,
    target: &mut Vec<T>,
    additional: usize,
    label: &'static str,
) -> Result<()> {
    if additional == 0 {
        return Ok(());
    }
    reservation.reserve_vec(target, additional, label)
}

fn reserved_filled<T: Clone, R: RequestPlanReservation>(
    reservation: &mut R,
    len: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    reserve_vec(reservation, &mut output, len, label)?;
    if output.capacity() < len {
        return Err(allocation(label));
    }
    output.resize(len, value);
    Ok(output)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::factorize::EncodedFactor;
    use crate::metadata::AbsorbedTerm;
    use crate::metadata::{AbsorbTermId, CompiledTermGraph, SourceEntry, SourceId};
    use crate::ols::OlsInputBinding;

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
    fn source_plan_borrows_original_effects_and_orders_required_sources() {
        let request = request();
        let plan = plan_categorical_absorbed_request(&request).expect("plan");
        assert!(std::ptr::eq(plan.request, &request));
        assert_eq!(plan.required_sources(), &[source(1), source(2), source(3)]);
        assert!(!plan.request.keep_singletons);
        assert_eq!(plan.request.absorbed_terms[1].label, "firm#year");
        assert_eq!(
            plan.request.absorbed_terms[1].categorical_sources,
            vec![source(2), source(3)]
        );
    }

    #[test]
    fn binding_preserves_requested_tuple_order() {
        let mut request = request();
        request.absorbed_terms[1].categorical_sources = vec![source(3), source(2)];
        let plan = plan_categorical_absorbed_request(&request).expect("plan");
        let bound = bind_categorical_effect_recipes(
            plan.request,
            &[prepared(3, 2), prepared(1, 0), prepared(2, 1)],
        )
        .expect("bound");
        assert_eq!(bound.len(), 2);
        assert_eq!(
            plan.request.absorbed_terms[1].categorical_sources,
            vec![source(3), source(2)]
        );
        assert_eq!(bound[1], vec![0, 2]);
    }

    #[test]
    fn repeated_source_is_deduplicated_only_for_availability() {
        let mut request = request();
        request.absorbed_terms[1].categorical_sources = vec![source(2), source(3), source(2)];
        let plan = plan_categorical_absorbed_request(&request).expect("plan");
        assert_eq!(plan.required_sources().len(), 3);
        assert_eq!(
            plan.request.absorbed_terms[1].categorical_sources,
            vec![source(2), source(3), source(2)]
        );
    }

    #[test]
    fn invalid_effect_surfaces_fail_closed() {
        let mut slope = request();
        slope.absorbed_terms[0].slope_sources.push(source(2));
        let slope_plan = plan_categorical_absorbed_request(&slope).expect("single guarded slope");
        assert_eq!(
            slope_plan.request.absorbed_terms[0].slope_sources,
            vec![source(2)]
        );
        let mut multiple_slopes = slope.clone();
        multiple_slopes.absorbed_terms[1].intercept = false;
        multiple_slopes.absorbed_terms[1].slope_sources = vec![source(1), source(2)];
        let multiple_plan =
            plan_categorical_absorbed_request(&multiple_slopes).expect("multiple slope-only terms");
        assert_eq!(
            multiple_plan.request.absorbed_terms[1].slope_sources,
            vec![source(1), source(2)]
        );
        assert!(!multiple_plan.request.absorbed_terms[1].intercept);
        let mut empty = request();
        empty.absorbed_terms[0].intercept = false;
        assert_eq!(
            plan_categorical_absorbed_request(&empty)
                .expect_err("empty effect")
                .code,
            ErrorCode::Unsupported
        );
        let mut saved = request();
        saved.absorbed_terms[0].saved_name = "alpha".to_owned();
        let saved_plan = plan_categorical_absorbed_request(&saved).expect("saved metadata");
        assert_eq!(saved_plan.request.absorbed_terms[0].id.get(), 1);
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
        let original = request();
        let plan = plan_categorical_absorbed_request(&original).expect("plan");
        assert_eq!(
            bind_categorical_effect_recipes(plan.request, &[prepared(1, 0), prepared(2, 1)])
                .expect_err("missing year")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            bind_categorical_effect_recipes(
                plan.request,
                &[prepared(1, 0), prepared(2, 1), prepared(2, 1)]
            )
            .expect_err("duplicate firm")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            bind_categorical_effect_recipes(
                plan.request,
                &[prepared(1, 1), prepared(2, 1), prepared(3, 2)]
            )
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
            let error = plan_categorical_absorbed_request_with_reservation(&request, &mut injected)
                .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
        for call in 0..successful.calls {
            let mut undersized = InjectedReservation {
                skip_at: Some(call),
                ..InjectedReservation::default()
            };
            let error =
                plan_categorical_absorbed_request_with_reservation(&request, &mut undersized)
                    .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
    }

    #[test]
    fn every_binding_reservation_and_capacity_guard_fails_closed() {
        let original = request();
        let plan = plan_categorical_absorbed_request(&original).expect("plan");
        let prepared = [prepared(1, 0), prepared(2, 1), prepared(3, 2)];
        let mut successful = InjectedReservation::default();
        bind_effect_recipes_with_reservation(plan.request, &prepared, &mut successful)
            .expect("successful binding audit");
        assert!(successful.calls > 0);
        for call in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(call),
                ..InjectedReservation::default()
            };
            let error =
                bind_effect_recipes_with_reservation(plan.request, &prepared, &mut injected)
                    .expect_err("injected binding allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
        for call in 0..successful.calls {
            let mut undersized = InjectedReservation {
                skip_at: Some(call),
                ..InjectedReservation::default()
            };
            let error =
                bind_effect_recipes_with_reservation(plan.request, &prepared, &mut undersized)
                    .expect_err("binding capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
    }
}
