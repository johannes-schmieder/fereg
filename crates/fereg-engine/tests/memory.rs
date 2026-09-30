//! Preflight boundaries, original-axis products and one optional workspace.
#![forbid(unsafe_code)]
use fereg_engine::{
    memory::{Effect, Inputs, Phase, Plan, Request},
    ppml::bias,
    spec::{Estimator, Solver},
    ErrorCode,
};

const EFFECTS: [Effect; 2] = [
    Effect {
        levels: 17,
        channels: 1,
    },
    Effect {
        levels: 29,
        channels: 1,
    },
];
fn request() -> Request<'static> {
    Request {
        inputs: Inputs {
            original_rows: 10_000,
            active_rows: 300,
            sources: 5,
            string_sources: 0,
            string_bytes: 0,
            metadata_bytes: 4096,
            weighted: true,
            adjustment: true,
            parent_bytes: 8192,
        },
        effects: &EFFECTS,
        finite_columns: 7,
        coefficient_slots: 9,
        estimator: Estimator::Ppml,
        solver: Solver::Auto,
        clusters: 2,
        worker_threads: 0,
        observation_outputs: 0,
        publication_matrix_cells: 81,
        bias: None,
        simplex_cells: 1000,
    }
}
#[test]
fn admission_distinguishes_required_storage_from_uncertain_estimates() {
    let base = request();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let requested = Request { solver, ..base };
        let plan = Plan::new(requested).unwrap();
        let minimum = plan.estimate().minimum_bytes();
        let peak = plan.estimate().peak_bytes();
        assert!(peak > minimum);
        assert_eq!(
            plan.admit(Some(minimum - 1)).unwrap_err().return_code(),
            909
        );
        // An uncertain fill estimate cannot silently veto an explicit solver.
        let admitted = Plan::new(requested).unwrap().admit(Some(minimum)).unwrap();
        assert_eq!(admitted.estimate().peak_bytes(), peak);
        Plan::new(requested).unwrap().admit(None).unwrap();
    }
}
#[test]
fn original_axis_and_declared_publication_owners_survive_compaction() {
    let base = request();
    let first = Plan::new(base).unwrap();
    let output = Request {
        observation_outputs: 3,
        ..base
    };
    let second = Plan::new(output).unwrap();
    let bytes = 3 * base.inputs.original_rows as u128 * 8;
    assert!(second.estimate().minimum_bytes() - first.estimate().minimum_bytes() >= bytes);
    assert!(
        second.estimate().phase_bytes(Phase::Publication)
            - first.estimate().phase_bytes(Phase::Publication)
            >= bytes
    );
    let compacted = Request {
        inputs: Inputs {
            active_rows: 10,
            ..base.inputs
        },
        ..output
    };
    let third = Plan::new(compacted).unwrap();
    assert_eq!(
        third.estimate().minimum_bytes(),
        second.estimate().minimum_bytes()
    );
    assert!(third.estimate().peak_bytes() < second.estimate().peak_bytes());
    let publication = Plan::new(Request {
        publication_matrix_cells: 10_081,
        ..base
    })
    .unwrap();
    assert_eq!(
        publication.estimate().phase_bytes(Phase::Publication)
            - first.estimate().phase_bytes(Phase::Publication),
        80_000
    );
}
#[test]
fn correction_has_no_probe_history_or_disabled_workspace() {
    let base = request();
    let ordinary = Plan::new(base).unwrap();
    assert_eq!(ordinary.estimate().bias_workspace_bytes(), 0);
    assert_eq!(ordinary.estimate().phase_bytes(Phase::BiasCorrection), 0);
    let mut estimates = vec![];
    for method in [
        bias::Method::Exact,
        bias::Method::Randomized {
            probes: 2,
            seed: 1,
            stream: 0,
        },
        bias::Method::Randomized {
            probes: u32::MAX,
            seed: u32::MAX,
            stream: u32::MAX,
        },
    ] {
        let plan = Plan::new(Request {
            bias: Some(bias::Request {
                method,
                save_effects: false,
            }),
            ..base
        })
        .unwrap();
        assert!(plan.estimate().bias_workspace_bytes() > 0);
        estimates.push(plan.estimate().clone());
    }
    assert!(estimates.windows(2).all(|v| v[0] == v[1]));
    let saved = Plan::new(Request {
        bias: Some(bias::Request {
            method: bias::Method::Exact,
            save_effects: true,
        }),
        ..base
    })
    .unwrap();
    let extra = saved.estimate().minimum_bytes() - estimates[0].minimum_bytes();
    assert!(extra >= 18 * base.inputs.original_rows as u128 * 8);
    assert_eq!(
        saved.estimate().bias_workspace_bytes() - estimates[0].bias_workspace_bytes(),
        extra
    );
}
#[test]
fn overflow_and_inconsistent_shapes_fail_before_storage() {
    let base = request();
    for huge in [
        Request {
            publication_matrix_cells: usize::MAX,
            ..base
        },
        Request {
            inputs: Inputs {
                original_rows: usize::MAX,
                ..base.inputs
            },
            ..base
        },
        Request {
            inputs: Inputs {
                parent_bytes: u128::MAX,
                ..base.inputs
            },
            ..base
        },
        Request {
            finite_columns: usize::MAX,
            coefficient_slots: usize::MAX,
            ..base
        },
    ] {
        assert_eq!(Plan::new(huge).unwrap_err().code, ErrorCode::Allocation);
    }
    let bad = Request {
        inputs: Inputs {
            active_rows: 10_001,
            ..base.inputs
        },
        ..base
    };
    assert_eq!(Plan::new(bad).unwrap_err().code, ErrorCode::InvalidArgument);
    let bad = Request {
        inputs: Inputs {
            string_sources: 6,
            ..base.inputs
        },
        ..base
    };
    assert_eq!(Plan::new(bad).unwrap_err().code, ErrorCode::InvalidArgument);
    let bad = Request {
        estimator: Estimator::Ols,
        ..base
    };
    assert_eq!(Plan::new(bad).unwrap_err().code, ErrorCode::InvalidArgument);
    let bad = Request {
        bias: Some(bias::Request {
            method: bias::Method::Randomized {
                probes: 1,
                seed: 1,
                stream: 1,
            },
            save_effects: false,
        }),
        ..base
    };
    assert_eq!(Plan::new(bad).unwrap_err().code, ErrorCode::InvalidArgument);
}
#[test]
fn payload_upper_bounds_do_not_become_false_capacity_proofs() {
    let base = request();
    let small = Plan::new(Request {
        inputs: Inputs {
            string_sources: 2,
            ..base.inputs
        },
        ..base
    })
    .unwrap();
    let large = Plan::new(Request {
        inputs: Inputs {
            string_sources: 2,
            string_bytes: 1_000_000_000,
            metadata_bytes: 16_000_000,
            ..base.inputs
        },
        ..base
    })
    .unwrap();
    assert_eq!(
        small.estimate().minimum_bytes(),
        large.estimate().minimum_bytes()
    );
    assert!(small.estimate().peak_bytes() < large.estimate().peak_bytes());
    large.admit(Some(small.estimate().minimum_bytes())).unwrap();
}
#[test]
fn executor_bookkeeping_changes_estimates_without_changing_admission() {
    let base = request();
    let first = Plan::new(base).unwrap();
    let larger = Plan::new(Request {
        worker_threads: 8,
        ..base
    })
    .unwrap();
    assert_eq!(
        first.estimate().minimum_bytes(),
        larger.estimate().minimum_bytes()
    );
    assert_eq!(
        larger.estimate().phase_bytes(Phase::Estimation)
            - first.estimate().phase_bytes(Phase::Estimation),
        8 * 512 * 1024
    );
    assert_eq!(
        Plan::new(Request {
            worker_threads: usize::MAX,
            ..base
        })
        .unwrap_err()
        .return_code(),
        909
    );
}

#[test]
fn model_layout_counts_implicit_constant_and_loaded_spare_capacities() {
    use fereg_engine::{data::*, memory::Layout, spec::metadata::*, ModelSpec};
    let mut model = ModelSpec {
        model: CompiledTermGraph {
            sources: vec![],
            terms: vec![],
        },
        absorbed_terms: vec![],
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let mut values = Vec::with_capacity(128);
    values.extend([1., 2.]);
    let mut loaded = Loaded {
        candidate: vec![true, false],
        outcome: NumericColumn {
            values,
            missing: None,
        },
        sources: vec![],
        weights: None,
        adjustment: LoadedAdjustment::None,
    };
    let layout = Layout::from_loaded(&model, &loaded, Estimator::Ols, 100, 7).unwrap();
    let request = layout.request(Solver::Auto);
    assert_eq!(request.inputs.active_rows, 1);
    assert_eq!(
        request.effects,
        &[Effect {
            levels: 1,
            channels: 1
        }]
    );
    assert_eq!(request.finite_columns, 1);
    assert!(request.inputs.parent_bytes >= 7 + 126 * 8);
    let ordinary = Plan::new(request).unwrap();
    model.model.sources.push(SourceEntry {
        id: SourceId::new(1).unwrap(),
        canonical_name: "group".into(),
    });
    model.absorbed_terms.push(AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        categorical_sources: vec![SourceId::new(1).unwrap()],
        intercept: true,
        slope_sources: vec![],
        label: "group".into(),
        saved_name: String::new(),
    });
    let mut text = String::with_capacity(4096);
    text.push_str("abc");
    loaded
        .sources
        .push(LoadedSource::Strings(vec![text, String::new()]));
    let layout = Layout::from_loaded(&model, &loaded, Estimator::Ols, 100, 7).unwrap();
    let request = layout.request(Solver::Auto);
    assert_eq!(request.inputs.string_sources, 1);
    assert_eq!(request.inputs.string_bytes, 0);
    assert!(request.inputs.parent_bytes >= 7 + 126 * 8 + 4096);
    assert!(
        Plan::new(request).unwrap().estimate().minimum_bytes()
            > ordinary.estimate().minimum_bytes()
    );
    // Alignment fails before sizing a request from a malformed native owner.
    loaded.outcome.values.pop();
    assert_eq!(
        Layout::from_loaded(&model, &loaded, Estimator::Ols, 100, 7)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}
