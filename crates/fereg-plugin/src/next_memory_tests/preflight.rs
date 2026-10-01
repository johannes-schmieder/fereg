use super::fixture;
use crate::ffi::{allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    memory::{Inputs, Resources},
    ols, ppml,
    spec::{metadata::*, Estimator, Options},
    ErrorCode, FeregError, ModelSpec, Result,
};
use std::{cell::Cell, rc::Rc};

struct Preflight<'a> {
    wire: &'a ModelWireV2,
    inputs: Inputs,
    options: Options,
    ols: ols::Request,
    ppml: ppml::Request,
}
impl Preflight<'_> {
    fn run(&self, estimator: Estimator, poll: &mut dyn FnMut() -> Result<()>) -> Result<u128> {
        let model = ModelSpec::from_wire_with(self.wire, &mut || poll())?;
        let resources = Resources::default();
        match estimator {
            Estimator::Ols => ols::Call::new_with(
                &model,
                &self.options,
                &self.ols,
                self.inputs,
                None,
                resources,
                &mut || poll(),
            )
            .map(|call| call.memory().peak_bytes()),
            Estimator::Ppml => ppml::Call::new_with(
                &model,
                &self.options,
                &self.ppml,
                self.inputs,
                None,
                resources,
                &mut || poll(),
            )
            .map(|call| call.memory().peak_bytes()),
        }
    }
}
#[test]
#[ignore = "process-global preflight allocator audit; run with --test-threads=1"]
fn caller_preflight_interruptions_release_wire_and_compiled_metadata_before_loading() {
    let _guard = crate::instrumentation::lock_action_observation();
    let mut model = fixture::model(true);
    let templates = model.model.terms.clone();
    model.model.terms = (0..1027)
        .map(|index| ExpandedTerm {
            id: TermId::new(index + 1).unwrap(),
            order: TermOrder::new(index + 1).unwrap(),
            coefficient_slot: CoefficientSlot::new(index + 1).unwrap(),
            canonical_token: format!("slot{index}"),
            ..templates[index as usize % templates.len()].clone()
        })
        .collect();
    let wire = ModelWireV2::from_specification(&CompiledModelSpecification {
        model: model.model,
        absorbed_terms: model.absorbed_terms,
    })
    .unwrap();
    let inputs = Inputs {
        original_rows: 64,
        active_rows: 64,
        sources: 4,
        auxiliary_numeric_sources: 0,
        string_sources: 0,
        string_bytes: 0,
        metadata_bytes: 0,
        weighted: false,
        adjustment: false,
        parent_bytes: 0,
    };
    let caller = std::thread::current().id();
    let count = Rc::new(Cell::new(0));
    let failure = FeregError::new(ErrorCode::NonConvergence, "preloading host r(430)");
    let mut aborted = 0;
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        let mut reference = None;
        for workers in [1, 8] {
            let context = Preflight {
                wire: &wire,
                inputs,
                options: Options {
                    threads: workers,
                    ..Options::for_estimator(estimator)
                },
                ols: ols::Request::default(),
                ppml: ppml::Request::default(),
            };
            count.set(0);
            let estimate = context
                .run(estimator, &mut || {
                    assert_eq!(std::thread::current().id(), caller);
                    count.set(count.get() + 1);
                    Ok(())
                })
                .unwrap();
            let total = count.get();
            assert!(total > 20);
            assert_eq!(*reference.get_or_insert(total), total);
            for stop in 1..=total {
                count.set(0);
                let (before, _) = begin_ols_allocation_observation();
                let error = context
                    .run(estimator, &mut || {
                        assert_eq!(std::thread::current().id(), caller);
                        count.set(count.get() + 1);
                        if count.get() == stop {
                            Err(failure.clone())
                        } else {
                            Ok(())
                        }
                    })
                    .unwrap_err();
                assert_eq!(error, failure);
                assert_eq!(count.get(), stop);
                drop(error);
                assert!(
                    allocation_usage().current_bytes <= before.current_bytes,
                    "{estimator:?} preflight retained metadata at checkpoint {stop}/{total}"
                );
                let peak = allocation_usage()
                    .peak_bytes
                    .saturating_sub(before.current_bytes);
                assert!(
                    peak as u128 <= estimate,
                    "preflight {peak} exceeds sole plan {estimate}"
                );
                assert_eq!(context.run(estimator, &mut || Ok(())).unwrap(), estimate);
                assert!(allocation_usage().current_bytes <= before.current_bytes);
                aborted += 1;
            }
        }
    }
    eprintln!("{aborted} tracked wire/constructor preflight aborts: no loaded observation banks, original caller errors, all metadata owners released and exact fresh admissions at 1/8 declared workers");
}
