use super::*;

#[test]
fn in_span_offsets_preserve_analytic_poisson_fit() {
    let n = 48;
    let group: Vec<_> = (0..n).map(|i| (1 + i / 12) as f64).collect();
    let x: Vec<_> = (0..n)
        .map(|i| if (i / 6) % 2 == 0 { -1.0 } else { 1.0 })
        .collect();
    let y: Vec<_> = (0..n)
        .map(|i| {
            (1.0 + group[i])
                * if x[i] < 0.0 {
                    (1 + i % 6 % 2) as f64
                } else {
                    (2 + i % 6 % 3) as f64
                }
        })
        .collect();
    // Each group's exact cell means have ratio two, so beta=log(2)/2.
    // Groupwise nuisance parameters do not change that ratio; neither does
    // the aggregate cell mean when the FE is absent.
    for absorbed in [false, true] {
        let model = if absorbed {
            prepared_many_fe(&y, &x, &[group.clone()])
        } else {
            prepared(&y, &[x.clone()])
        };
        for mode in [
            crate::options::MemoryMode::Fast,
            crate::options::MemoryMode::Low,
        ] {
            for shift in [1.0, 20.0, 100.0, 500.0] {
                let offset: Vec<_> = x.iter().map(|x| shift * x).collect();
                let mut config = PreparedPpmlIrlsConfig::default();
                config.memory.requested = mode;
                config.ppml.tolerance = 1e-10;
                config.ppml.score_tolerance = 1e-10;
                let plan =
                    plan_ppml_memory(memory_dimensions(&model, config), config.memory).unwrap();
                let fit =
                    fit_prepared_ppml_irls(&model, &y, &[1.; 48], &offset, config, plan, None)
                        .unwrap_or_else(|error| {
                            panic!("absorbed={absorbed} mode={mode:?} shift={shift}: {error:?}")
                        });
                let b = fit.coefficients()[0].value.unwrap();
                assert!(
                    (b + shift - 2.0_f64.ln() / 2.0).abs() < 2e-8,
                    "beta {b} shift {shift}"
                );
                for row in 0..n {
                    let base = if absorbed { 1.0 + group[row] } else { 3.5 };
                    let expected = (base * if x[row] < 0.0 { 1.5 } else { 3.0 }).ln();
                    assert!((fit.eta()[row] - expected).abs() < 2e-8);
                }
            }
        }
    }
}

#[test]
fn offset_seed_keeps_out_of_span_component_and_zero_rows() {
    let x = vec![-1., -1., 1., 1., -1., -1., 1., 1.];
    let y = vec![0., 2., 3., 5., 2., 4., 6., 8.];
    let model = prepared(&y, &[x.clone()]);
    let offset: Vec<_> = (0..8)
        .map(|i| 100.0 * x[i] + if i % 2 == 0 { -0.2 } else { 0.2 })
        .collect();
    let mut config = PreparedPpmlIrlsConfig::default();
    config.ppml.absorb.solver = FeSolverKind::Map;
    config.linear.absorb.solver = FeSolverKind::Map;
    let mut plan = plan_ppml_memory(memory_dimensions(&model, config), config.memory).unwrap();
    let (coefficients, eta) =
        offset_start::propose(&model, &y, &[1.; 8], &offset, config, &mut plan)
            .unwrap()
            .expect("certified start");
    // Independent dense projection: intercept plus binary x spans exactly
    // the two cell means of log-proxy minus offset.
    let zero_proxy = (y.iter().sum::<f64>() / 8.0).ln();
    for row in 0..8 {
        let mean = (0..8)
            .filter(|&i| x[i] == x[row])
            .map(|i| (if y[i] > 0.0 { y[i].ln() } else { zero_proxy }) - offset[i])
            .sum::<f64>()
            / 4.0;
        assert!((eta[row] - offset[row] - mean).abs() < 2e-12);
    }
    assert!((eta[1] - eta[0] - 0.4).abs() < 2e-12);
    validate_start(
        &model,
        &PreparedPpmlStart {
            coefficient_values: coefficients,
            eta,
        },
        &[1.; 8],
        &offset,
        &model.fit_owned_fe_design().unwrap(),
        config,
    )
    .unwrap();
}

#[test]
fn offset_seed_budget_decline_preserves_original_plan() {
    let y = vec![1., 2., 3., 4., 2., 3., 4., 5.];
    let x = vec![-1., 1., -1., 1., -1., 1., -1., 1.];
    let model = prepared_many_fe(&y, &x, &[vec![1., 1., 1., 1., 2., 2., 2., 2.]]);
    let config = PreparedPpmlIrlsConfig::default();
    let mut plan = plan_ppml_memory(memory_dimensions(&model, config), config.memory).unwrap();
    plan.usable_bytes = Some(plan.solver_workspace_peak());
    let original = plan.solver_workspace_bytes;
    assert!(
        offset_start::propose(&model, &y, &[1.; 8], &x, config, &mut plan)
            .unwrap()
            .is_none()
    );
    assert_eq!(plan.solver_workspace_bytes, original);
}

#[test]
fn offset_seed_does_not_swallow_break() {
    const CHILD: &str = "FEREG_OFFSET_START_BREAK_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ppml::prepared_irls::tests::offset_start_tests::offset_seed_does_not_swallow_break", "--nocapture"])
            .env(CHILD, "1").status().unwrap();
        assert!(status.success());
        return;
    }
    let y = vec![1., 2., 3., 4.];
    let x = vec![-1., 1., -1., 1.];
    let model = prepared(&y, &[x.clone()]);
    let config = PreparedPpmlIrlsConfig::default();
    let mut plan = plan_ppml_memory(memory_dimensions(&model, config), config.memory).unwrap();
    crate::interrupt::test_polls::cancel_at(Some(1));
    let error = offset_start::propose(&model, &y, &[1.; 4], &x, config, &mut plan).unwrap_err();
    crate::interrupt::test_polls::cancel_at(None);
    assert_eq!(error.code, ErrorCode::Interrupted);
}

#[test]
fn offset_seed_dynamic_workspace_respects_the_duplicate_topology() {
    let y = vec![1., 2., 3., 4., 2., 3., 4., 5.];
    let x = vec![-1., 1., -1., 1., -1., 1., -1., 1.];
    let model = prepared_many_fe(&y, &x, &[vec![1., 1., 1., 1., 2., 2., 2., 2.]]);
    let mut config = PreparedPpmlIrlsConfig::default();
    let mut plan = plan_ppml_memory(memory_dimensions(&model, config), config.memory).unwrap();
    let topology = plan.selected_inventory().fe_solver_topology_bytes;
    assert!(topology > 0);
    let execution = plan.solver_workspace_peak();
    let original = FeSolverMemoryBudget {
        execution_bytes: execution,
        usable_bytes: execution + topology + 128,
    };
    config.ppml.absorb.solver_memory_budget = Some(original);
    config.linear.absorb.solver_memory_budget = Some(original);
    plan.usable_bytes = Some(original.usable_bytes);
    let seed = offset_start::seed_solver_budget(config, plan, topology).unwrap();
    assert_eq!(seed.execution_bytes, execution + topology);
    assert_eq!(seed.usable_bytes, original.usable_bytes);
    assert!(original.fits(topology + 128));
    assert!(seed.fits(128));
    assert!(!seed.fits(129));
    assert_eq!(config.ppml.absorb.solver_memory_budget, Some(original));
    plan.usable_bytes = Some(execution + topology - 1);
    assert!(offset_start::seed_solver_budget(config, plan, topology).is_none());
}
