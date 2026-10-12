use super::*;
use crate::metadata::AbsorbTermId;
use crate::ols::prepared::tests::{
    column, request, two_way_singleton_input, two_way_singleton_request,
};
use crate::ols::*;

pub(crate) fn fixture(rows: usize) -> (OlsRequestV1, LoadedOlsInput) {
    let mut request = request(3, true);
    request.model.terms.truncate(1);
    request.absorbed_terms = [2, 3]
        .into_iter()
        .map(|source| crate::metadata::AbsorbedTerm {
            id: AbsorbTermId::new(source - 1).unwrap(),
            intercept: true,
            categorical_sources: vec![SourceId::new(source).unwrap()],
            slope_sources: Vec::new(),
            label: format!("g{source}"),
            saved_name: String::new(),
        })
        .collect();
    request.keep_singletons = true;
    let x: Vec<_> = (0..rows).map(|row| (row as f64 * 0.31).sin()).collect();
    let y: Vec<_> = (0..rows)
        .map(|row| 1.5 + 0.7 * x[row] + (row as f64 * 0.17).cos())
        .collect();
    let mut groups = [7, 11].map(|levels| {
        (0..rows)
            .map(|row| (row % levels) as f64)
            .collect::<Vec<_>>()
    });
    groups[0][0] = -0.0;
    let missing = vec![false; rows];
    let loaded = LoadedOlsInput {
        candidate: crate::data::UniformValues::Repeated {
            len: rows,
            value: true,
        },
        outcome: column(&y, &missing),
        numeric_sources: [x, groups[0].clone(), groups[1].clone()]
            .into_iter()
            .enumerate()
            .map(|(index, values)| {
                (
                    SourceId::new(index as u32 + 1).unwrap(),
                    column(&values, &missing),
                )
            })
            .collect(),
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    (request, loaded)
}

pub(crate) fn prepare(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
    ceiling: Option<u128>,
) -> PreparedOlsData {
    prepare_model_owned_with_load_columns_and_release(request, &[], loaded, false, None, 0, ceiling)
        .unwrap()
}

#[test]
fn fe_release_moves_raw_owners_and_accounts_actual_descriptor_overlap() {
    let (request, mut loaded) = fixture(513);
    loaded.numeric_sources[0].1.values.reserve(31);
    let pointer = loaded.numeric_sources[0].1.values.as_ptr();
    let capacity = loaded.numeric_sources[0].1.values.capacity();
    let expected = prepare(&request, loaded.clone(), None);
    let raw = prepare(&request, loaded, None);
    assert_eq!(raw.sample().regressors.get(0).unwrap().as_ptr(), pointer);
    assert_eq!(raw.sample().regressors.get(0).unwrap().capacity(), capacity);
    let original = raw.owned_buffer_bytes();
    let freed = raw.sample().regressors.get(1).unwrap().capacity() as u128 * 8
        + raw.sample().regressors.get(2).unwrap().capacity() as u128 * 8;
    let old_descriptors = match &raw.sample().regressors.0 {
        Storage::Raw(columns) => vec_capacity_bytes(columns),
        _ => unreachable!(),
    };
    let mut released = raw;
    released.release_fe_columns(&request, u128::MAX);
    assert_eq!(released.execution_admission_bytes(), original);
    assert_eq!(
        released.sample().regressors.get(0).unwrap().as_ptr(),
        pointer
    );
    assert_eq!(
        released.sample().regressors.get(0).unwrap().capacity(),
        capacity
    );
    assert!(released.sample().regressors.get(1).is_none());
    assert!(released.sample().regressors.get(2).is_none());
    let descriptors = match &released.sample().regressors.0 {
        Storage::Slots(slots) => vec_capacity_bytes(slots),
        _ => panic!("must release"),
    };
    assert_eq!(
        released.owned_buffer_bytes(),
        original - freed - old_descriptors + descriptors
    );
    assert_eq!(released.build_rhs().unwrap(), expected.build_rhs().unwrap());
    released.validate().unwrap();
}

#[test]
fn fe_release_optional_boundaries_and_failed_proof_preserve_raw_diagnostics() {
    let (request, loaded) = fixture(513);
    let baseline = prepare(&request, loaded.clone(), None);
    let requested =
        baseline.sample().regressors.len() as u128 * std::mem::size_of::<SourceSlot>() as u128;
    let ceiling = baseline.owned_buffer_bytes() + requested;
    let exact = prepare(&request, loaded.clone(), Some(ceiling));
    assert!(exact.sample().regressors.get(1).is_none());
    let short = prepare(&request, loaded.clone(), Some(ceiling - 1));
    assert_eq!(short, baseline);
    for fault in [1, 2] {
        RESERVATION_FAULT.set(fault);
        assert_eq!(prepare(&request, loaded.clone(), Some(ceiling)), baseline);
    }
    let mut malformed = prepare(&request, loaded, None);
    malformed.sample.regressors[1][0] = 92.0;
    let before = malformed.validate().unwrap_err();
    let bytes = malformed.owned_buffer_bytes();
    malformed.release_fe_columns(&request, u128::MAX);
    assert!(matches!(malformed.sample().regressors.0, Storage::Raw(_)));
    assert_eq!(malformed.owned_buffer_bytes(), bytes);
    assert_eq!(malformed.validate().unwrap_err(), before);
}

#[test]
fn fe_release_original_omitted_cluster_and_interaction_roles_remain_numeric() {
    let (mut request, loaded) = fixture(513);
    let mut omitted = crate::ols::prepared::tests::request(3, true).model.terms[1].clone();
    omitted.whole_column_omitted = true;
    request.model.terms.push(omitted);
    let prepared = prepare(&request, loaded.clone(), Some(u128::MAX));
    assert!(prepared.sample().regressors.get(1).is_some());
    assert!(prepared.sample().regressors.get(2).is_none());
    request.model.terms.truncate(1);
    let clusters = [OlsClusterTermV1 {
        label: "g2".into(),
        sources: vec![SourceId::new(2).unwrap()],
    }];
    let clustered = prepare_model_owned_with_load_columns_and_release(
        &request,
        &clusters,
        loaded.clone(),
        false,
        None,
        0,
        Some(u128::MAX),
    )
    .unwrap();
    assert!(clustered.sample().regressors.get(1).is_some());
    assert!(clustered.sample().regressors.get(2).is_none());
    let mut interaction = request.absorbed_terms[0].clone();
    interaction.id = AbsorbTermId::new(3).unwrap();
    interaction
        .categorical_sources
        .push(SourceId::new(3).unwrap());
    interaction.label = "g2#g3".into();
    request.absorbed_terms.push(interaction);
    let complex = prepare_model_owned_with_load_columns_and_release(
        &request,
        &[],
        loaded,
        true,
        None,
        0,
        Some(u128::MAX),
    )
    .unwrap();
    assert!(matches!(complex.sample().regressors.0, Storage::Raw(_)));
}

#[test]
fn fe_release_after_singleton_reencoding_matches_raw_final_topology() {
    let mut request = two_way_singleton_request(false);
    request.model.terms.truncate(1);
    let baseline = prepare(&request, two_way_singleton_input(), None);
    let released = prepare(&request, two_way_singleton_input(), Some(u128::MAX));
    assert!(released.sample().n_active() < 9);
    assert!(released.sample().regressors.get(1).is_none());
    assert_eq!(released.fe_topology(), baseline.fe_topology());
    assert_eq!(released.singletons, baseline.singletons);
    assert_eq!(released.build_rhs().unwrap(), baseline.build_rhs().unwrap());
    released.validate().unwrap();
}

#[test]
fn fe_release_slope_sharing_retains_raw_and_small_samples_have_no_fence() {
    let (mut request, loaded) = fixture(513);
    request.absorbed_terms[0]
        .slope_sources
        .push(SourceId::new(3).unwrap());
    let prepared = prepare_model_owned_with_load_columns_and_release(
        &request,
        &[],
        loaded,
        true,
        None,
        0,
        Some(u128::MAX),
    )
    .unwrap();
    assert!(prepared.sample().regressors.get(1).is_some());
    assert!(prepared.sample().regressors.get(2).is_some());
    let (request, loaded) = fixture(1);
    let baseline = prepare(&request, loaded.clone(), None);
    let small = prepare(&request, loaded, Some(u128::MAX));
    assert!(small.sample().regressors.get(1).is_none());
    assert!(small.owned_buffer_bytes() <= baseline.owned_buffer_bytes());
    assert_eq!(
        small.execution_admission_bytes(),
        baseline.owned_buffer_bytes()
    );
}

#[test]
fn fe_release_outcome_ownership_is_independent_of_equal_fe_source_values() {
    let (request, mut loaded) = fixture(513);
    loaded.outcome.values = loaded.numeric_sources[1].1.values.clone();
    loaded.outcome.values.reserve(19);
    let pointer = loaded.outcome.values.as_ptr();
    let capacity = loaded.outcome.values.capacity();
    let bits: Vec<_> = loaded
        .outcome
        .values
        .iter()
        .map(|value| value.to_bits())
        .collect();
    let prepared = prepare(&request, loaded, Some(u128::MAX));
    assert!(prepared.sample().regressors.get(1).is_none());
    assert_eq!(prepared.sample().outcome.as_ptr(), pointer);
    assert_eq!(prepared.sample().outcome.capacity(), capacity);
    assert_eq!(
        prepared
            .sample()
            .outcome
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        bits
    );
    assert_eq!(prepared.build_rhs().unwrap().values().nrows(), 513);
}

#[test]
fn fe_release_saved_effect_recovery_retains_exact_coefficients() {
    let (request, loaded) = fixture(513);
    let target: Vec<_> = (0..513)
        .map(|row| {
            2.0 * loaded.numeric_sources[1].1.values[row]
                - 0.5 * loaded.numeric_sources[2].1.values[row]
        })
        .collect();
    let baseline = prepare(&request, loaded.clone(), None);
    let released = prepare(&request, loaded, Some(u128::MAX));
    let run = |prepared: &PreparedOlsData| {
        prepared.recover_absorbed_effects_fast(
            &target,
            &[true, true],
            None,
            false,
            true,
            crate::fe::LsmrConfig::default(),
            crate::fe::MapConfig::default(),
            None,
        )
    };
    assert_eq!(run(&released).unwrap(), run(&baseline).unwrap());
}
