use super::*;

/// Every (g,h) cell contains the complete 2^3 sign cube. Consequently u, v,
/// and w have zero sum within every FE level, are mutually orthogonal, and
/// have squared norms 800. This gives an analytical oracle without another
/// numerical projection or least-squares implementation.
struct CrossedSignCube {
    outcome: Vec<f64>,
    effects: [Vec<f64>; 2],
    u: Vec<f64>,
    v: Vec<f64>,
    w: Vec<f64>,
}

impl CrossedSignCube {
    fn new() -> Self {
        let mut data = Self {
            outcome: Vec::new(),
            effects: [Vec::new(), Vec::new()],
            u: Vec::new(),
            v: Vec::new(),
            w: Vec::new(),
        };
        for g in 1..=10 {
            for h in 1..=10 {
                for signs in 0..8 {
                    let u = if signs & 1 == 0 { -1.0 } else { 1.0 };
                    let v = if signs & 2 == 0 { -1.0 } else { 1.0 };
                    let w = if signs & 4 == 0 { -1.0 } else { 1.0 };
                    data.effects[0].push(f64::from(g));
                    data.effects[1].push(f64::from(h));
                    data.u.push(u);
                    data.v.push(v);
                    data.w.push(w);
                    data.outcome
                        .push(5.0 * f64::from(g) + 7.0 * f64::from(h) + 2.0 * u - 3.0 * v + w);
                }
            }
        }
        data
    }

    fn regressors(&self, shift: f64, unit: f64, include_signal: bool) -> [Vec<f64>; 2] {
        let x = self.effects[0]
            .iter()
            .zip(&self.u)
            .map(|(&g, &u)| unit * (shift * g + if include_signal { u } else { 0.0 }))
            .collect::<Vec<_>>();
        let z = self.v.iter().map(|v| v / unit).collect::<Vec<_>>();
        [x, z]
    }

    fn prepare(&self, shift: f64, unit: f64, include_signal: bool) -> PreparedOlsData {
        prepared_multiway_categorical(
            &self.outcome,
            &self.regressors(shift, unit, include_signal),
            &self.effects,
        )
    }
}

fn configs() -> impl Iterator<Item = PreparedOlsFitConfig> {
    [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ]
    .into_iter()
    .flat_map(|mode| {
        [FeSolverKind::Auto, FeSolverKind::Map, FeSolverKind::Lsmr]
            .into_iter()
            .map(move |solver| {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.mode = mode;
                config.absorb.solver = solver;
                config
            })
    })
}

fn check_identified_fit(fit: &CertifiedPreparedOlsFit, data: &CrossedSignCube, unit: f64) {
    let rows = data.outcome.len() as f64;
    assert_eq!(fit.finite_rank(), 2, "{:?}", fit.coefficients());
    for (column, (rescale, expected)) in [(unit, 2.0), (1.0 / unit, -3.0)].into_iter().enumerate() {
        assert_eq!(
            fit.coefficients()[column].status,
            PreparedOlsFitStatus::Kept
        );
        assert_close(
            fit.coefficients()[column].value.expect("identified slope") * rescale,
            expected,
            1e-8,
        );
        let bread = fit.bread();
        let coordinate = bread
            .coordinates()
            .iter()
            .position(|coordinate| coordinate.generated_column_index == column)
            .expect("retained slope bread coordinate");
        // Rescaling the inverse cross-product back to u,v coordinates must
        // produce diag(1/N,1/N), including after a large absorbed shift.
        assert_close(
            bread.values()[coordinate * bread.dimension() + coordinate] * rescale * rescale,
            1.0 / rows,
            1e-10,
        );
    }
    // The exact fitted residual is w. Bounds allow normal double-precision
    // accumulation while excluding the omitted-x model (RSS 4000).
    for (&actual, &expected) in fit.residuals().iter().zip(&data.w) {
        assert_close(actual, expected, 1e-6);
    }
    assert_close(
        fit.residuals().iter().map(|value| value * value).sum(),
        rows,
        1e-6,
    );
    assert!(fit.rank_certificate().passed);
    assert!(fit.score_certificate().passed);
    assert!(fit.bread().certificate().passed);
}

#[test]
fn absorbed_shifts_preserve_resolved_within_rank() {
    let data = CrossedSignCube::new();
    for shift in [0.0, 1e4, 1e8] {
        // These x values are exact integers below 2^53. The signal u remains
        // exactly represented even when its raw/within squared norm ratio
        // falls below the historical fixed 1e-9 omission threshold.
        let prepared = data.prepare(shift, 1.0, true);
        for config in configs() {
            let fit = fit_prepared_model_coefficients(&prepared, config).unwrap_or_else(|error| {
                panic!(
                    "shift={shift}, mode={:?}, solver={:?}: {error}",
                    config.execution.mode, config.absorb.solver
                )
            });
            check_identified_fit(&fit, &data, 1.0);
        }
    }
}

#[test]
fn opposite_rhs_units_preserve_coefficients_and_inverse_cross_product() {
    let data = CrossedSignCube::new();
    for unit in [1e-8, 1.0, 1e8] {
        let prepared = data.prepare(0.0, unit, true);
        for config in configs() {
            let fit = fit_prepared_model_coefficients(&prepared, config).unwrap_or_else(|error| {
                panic!(
                    "unit={unit}, mode={:?}, solver={:?}: {error}",
                    config.execution.mode, config.absorb.solver
                )
            });
            check_identified_fit(&fit, &data, unit);
        }
    }
}

#[test]
fn exactly_absorbed_columns_remain_omitted_at_large_scales() {
    let data = CrossedSignCube::new();
    for shift in [1.0, 1e8] {
        let prepared = data.prepare(shift, 1.0, false);
        for config in configs() {
            let fit = fit_prepared_model_coefficients(&prepared, config).unwrap_or_else(|error| {
                panic!(
                    "exact absorbed shift={shift}, mode={:?}, solver={:?}: {error}",
                    config.execution.mode, config.absorb.solver
                )
            });
            assert_eq!(fit.finite_rank(), 1);
            assert_eq!(
                fit.coefficients()[0].status,
                PreparedOlsFitStatus::CollinearWithFixedEffects
            );
            assert_eq!(fit.coefficients()[1].status, PreparedOlsFitStatus::Kept);
            assert_close(fit.coefficients()[1].value.expect("v slope"), -3.0, 1e-8);
            for ((&actual, &u), &w) in fit.residuals().iter().zip(&data.u).zip(&data.w) {
                assert_close(actual, 2.0 * u + w, 1e-6);
            }
            assert_close(
                fit.residuals().iter().map(|value| value * value).sum(),
                4000.0,
                1e-6,
            );
            assert!(fit.rank_certificate().passed);
            assert!(fit.score_certificate().passed);
        }
    }
}

#[test]
fn cell_constant_weights_preserve_shifted_within_signal() {
    let data = CrossedSignCube::new();
    // Weights vary over both effects but are constant over each sign cube.
    // Weighted within moments of each sign and all sign cross-products are
    // therefore exactly zero; weighted RSS is exactly the raw weight sum.
    let weights = data.effects[0]
        .iter()
        .zip(&data.effects[1])
        .map(|(&g, &h)| 1.0 + (g + 2.0 * h) % 7.0)
        .collect::<Vec<_>>();
    let prepared = prepared_multiway_categorical_weights(
        &data.outcome,
        &data.regressors(1e8, 1.0, true),
        &data.effects,
        TestWeights::Analytic(&weights),
    );
    for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
        let fit = fit_prepared_model_coefficients(&prepared, config).expect("weighted shifted fit");
        // Analytic weights are normalized to sum N for the reported bread.
        check_identified_fit(&fit, &data, 1.0);
        assert_close(
            fit.residuals()
                .iter()
                .zip(&weights)
                .map(|(&residual, &weight)| weight * residual * residual)
                .sum(),
            weights.iter().sum(),
            1e-6,
        );
    }
}

#[test]
fn unique_joint_cells_preserve_cycle_identification() {
    // One observation per (g,h) cell: identification comes entirely from
    // cycles in the two-effect graph, never from within-cell replication.
    // Products of the two orthogonal, zero-sum four-level contrasts give
    // three mutually orthogonal FE-orthogonal vectors with squared norm 16.
    let a = [-1.0, -1.0, 1.0, 1.0];
    let b = [-1.0, 1.0, -1.0, 1.0];
    let mut data = CrossedSignCube {
        outcome: Vec::new(),
        effects: [Vec::new(), Vec::new()],
        u: Vec::new(),
        v: Vec::new(),
        w: Vec::new(),
    };
    for g in 0..4 {
        for h in 0..4 {
            let u = a[g] * a[h];
            let v = a[g] * b[h];
            let w = b[g] * a[h];
            let g_value = (g + 1) as f64;
            let h_value = (h + 1) as f64;
            data.effects[0].push(g_value);
            data.effects[1].push(h_value);
            data.u.push(u);
            data.v.push(v);
            data.w.push(w);
            data.outcome
                .push(5.0 * g_value + 7.0 * h_value + 2.0 * u - 3.0 * v + w);
        }
    }
    let prepared = data.prepare(1e8, 1.0, true);
    for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
        let fit = fit_prepared_model_coefficients(&prepared, config).expect("unique-cell fit");
        check_identified_fit(&fit, &data, 1.0);
    }
}

fn global_intercept_cube() -> CrossedSignCube {
    let mut data = CrossedSignCube::new();
    data.u.truncate(8);
    data.v.truncate(8);
    data.w.truncate(8);
    data.effects[0].clear();
    data.effects[1].clear();
    data.outcome = (0..8)
        .map(|row| 7.0 + 2.0 * data.u[row] - 3.0 * data.v[row] + data.w[row])
        .collect();
    data
}

#[test]
fn global_intercept_shifts_preserve_finite_signal() {
    let data = global_intercept_cube();
    let shift = 1e8;
    let x = data.u.iter().map(|u| shift + u).collect();
    let prepared = prepared(
        &data.outcome,
        &[x, data.v.clone()],
        &[false, false],
        None,
        true,
        TestWeights::Unit,
    );
    for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
        let fit = fit_prepared_model_coefficients(&prepared, config).expect("shifted no-FE fit");
        check_identified_fit(&fit, &data, 1.0);
        assert_close(
            fit.coefficients()[2].value.expect("global intercept") + 2.0 * shift,
            7.0,
            1e-6,
        );
    }
}

#[test]
fn global_intercept_exact_constants_remain_omitted() {
    let data = global_intercept_cube();
    let prepared = prepared(
        &data.outcome,
        &[vec![1e8; 8], data.v.clone()],
        &[false, false],
        None,
        true,
        TestWeights::Unit,
    );
    for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
        let fit = fit_prepared_model_coefficients(&prepared, config).expect("constant no-FE fit");
        assert_eq!(fit.finite_rank(), 1);
        assert_eq!(
            fit.coefficients()[0].status,
            PreparedOlsFitStatus::CollinearWithFixedEffects
        );
        assert_close(fit.coefficients()[1].value.expect("v slope"), -3.0, 1e-8);
        assert_close(
            fit.coefficients()[2].value.expect("global intercept"),
            7.0,
            1e-8,
        );
        for ((&actual, &u), &w) in fit.residuals().iter().zip(&data.u).zip(&data.w) {
            assert_close(actual, 2.0 * u + w, 1e-8);
        }
    }
}

#[test]
fn saved_effects_reconstruct_original_shifted_model() {
    let data = CrossedSignCube::new();
    let shift = 1e8;
    let regressors = data.regressors(shift, 1.0, true);
    let prepared = data.prepare(shift, 1.0, true);
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.retain_coefficients = true;
    config.allow_final_fe_recovery = true;
    let fit = fit_prepared_model_coefficients(&prepared, config).expect("saved-FE shifted fit");
    check_identified_fit(&fit, &data, 1.0);
    let coefficients = fit
        .coefficients()
        .iter()
        .map(|coefficient| coefficient.value.expect("identified coefficient"))
        .collect::<Vec<_>>();
    let target = (0..data.outcome.len())
        .map(|row| {
            data.outcome[row]
                - coefficients[0] * regressors[0][row]
                - coefficients[1] * regressors[1][row]
                - coefficients[2]
                - fit.residuals()[row]
        })
        .collect::<Vec<_>>();
    let recovered = ThreadTeam::new(config.execution.threads)
        .install(|| {
            prepared.recover_absorbed_effects_fast(
                &target,
                &[true, true],
                fit.recovered_fe_coefficients(),
                true,
                true,
                config.absorb.lsmr,
                MapConfig {
                    tolerance: 1e-12,
                    ..config.absorb.map
                },
                None,
            )
        })
        .unwrap()
        .expect("saved effect recovery");
    for row in 0..data.outcome.len() {
        let saved_sum = recovered.channels()[0].as_ref().unwrap()[row]
            + recovered.channels()[1].as_ref().unwrap()[row];
        // Compare in the outcome's original small units, avoiding a relative
        // tolerance dominated by the large cancelling FE and x contributions.
        let reconstructed = coefficients[0] * regressors[0][row]
            + coefficients[1] * regressors[1][row]
            + coefficients[2]
            + saved_sum
            + fit.residuals()[row];
        assert!((reconstructed - data.outcome[row]).abs() <= 2e-6);
        assert!((saved_sum - recovered.absorbed_sum()[row]).abs() <= 2e-6);
    }
}

#[test]
fn one_categorical_effect_preserves_shifted_signal() {
    let mut data = CrossedSignCube::new();
    data.outcome = (0..data.outcome.len())
        .map(|row| 5.0 * data.effects[0][row] + 2.0 * data.u[row] - 3.0 * data.v[row] + data.w[row])
        .collect();
    let prepared = prepared_multiway_categorical(
        &data.outcome,
        &data.regressors(1e8, 1.0, true),
        &data.effects[..1],
    );
    for config in configs().filter(|config| {
        config.absorb.solver == FeSolverKind::Auto
            || (config.execution.mode == PreparedOlsExecutionMode::Fast
                && config.absorb.solver == FeSolverKind::Lsmr)
    }) {
        let fit =
            fit_prepared_model_coefficients(&prepared, config).expect("one-effect shifted fit");
        check_identified_fit(&fit, &data, 1.0);
    }
}

#[test]
fn explicit_accelerated_solvers_preserve_shifted_signal() {
    let data = CrossedSignCube::new();
    let prepared = data.prepare(1e8, 1.0, true);
    // This connected two-way categorical-intercept graph satisfies the
    // explicit CMG contract. One Fast case per family avoids an unnecessary
    // Cartesian product with the separate memory-mode and worker tests.
    for solver in [
        FeSolverKind::Cmg,
        FeSolverKind::Squarem,
        FeSolverKind::IronsTuck,
    ] {
        let mut config = PreparedOlsFitConfig::default();
        config.absorb.solver = solver;
        let fit = fit_prepared_model_coefficients(&prepared, config)
            .unwrap_or_else(|error| panic!("explicit {solver:?}: {error}"));
        assert_eq!(fit.absorption_report().requested_solver, solver);
        check_identified_fit(&fit, &data, 1.0);
    }
}

#[test]
fn repaired_columns_keep_canonical_duplicate_omissions() {
    let data = CrossedSignCube::new();
    let [x, z] = data.regressors(1e8, 1.0, true);
    let prepared = prepared_multiway_categorical(&data.outcome, &[x.clone(), x, z], &data.effects);
    for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
        let fit = fit_prepared_model_coefficients(&prepared, config)
            .expect("shifted duplicate-column fit");
        assert_eq!(fit.finite_rank(), 2);
        assert_eq!(fit.kept_generated_columns(), &[0, 2]);
        assert_eq!(fit.dropped_generated_columns(), &[1]);
        assert_eq!(fit.coefficients()[0].status, PreparedOlsFitStatus::Kept);
        assert_eq!(
            fit.coefficients()[1].status,
            PreparedOlsFitStatus::NumericallyRankDeficient
        );
        assert_eq!(fit.coefficients()[1].value, None);
        assert_eq!(fit.coefficients()[2].status, PreparedOlsFitStatus::Kept);
        assert_close(fit.coefficients()[0].value.unwrap(), 2.0, 1e-8);
        assert_close(fit.coefficients()[2].value.unwrap(), -3.0, 1e-8);
        for (&actual, &expected) in fit.residuals().iter().zip(&data.w) {
            assert_close(actual, expected, 1e-6);
        }
        assert_close(
            fit.residuals()
                .iter()
                .map(|residual| residual * residual)
                .sum(),
            data.outcome.len() as f64,
            1e-6,
        );
        assert!(fit.rank_certificate().passed);
        assert!(fit.score_certificate().passed);
    }
}

#[test]
fn repaired_columns_repeat_at_fixed_workers_and_agree_across_workers() {
    let source = CrossedSignCube::new();
    // 25,600 rows exceed the parallel reduction boundary. Replication
    // preserves the exact orthogonality oracle and changes squared norms
    // and RSS only by the known factor 32.
    let data = CrossedSignCube {
        outcome: source.outcome.repeat(32),
        effects: [source.effects[0].repeat(32), source.effects[1].repeat(32)],
        u: source.u.repeat(32),
        v: source.v.repeat(32),
        w: source.w.repeat(32),
    };
    let prepared = data.prepare(1e8, 1.0, true);
    let mut previous: Option<CertifiedPreparedOlsFit> = None;
    for workers in [1, 4] {
        let mut config = PreparedOlsFitConfig::default();
        config.execution.threads = ThreadConfig::from_requested(workers, true).unwrap();
        let fit = fit_prepared_model_coefficients(&prepared, config).expect("shifted worker fit");
        let repeated = fit_prepared_model_coefficients(&prepared, config)
            .expect("repeated shifted worker fit");
        check_identified_fit(&fit, &data, 1.0);
        check_identified_fit(&repeated, &data, 1.0);
        assert_incremental_fit_exact(&repeated, &fit);
        if let Some(previous) = previous {
            assert_eq!(
                fit.kept_generated_columns(),
                previous.kept_generated_columns()
            );
            assert_eq!(
                fit.dropped_generated_columns(),
                previous.dropped_generated_columns()
            );
            for (actual, previous) in fit.coefficients().iter().zip(previous.coefficients()) {
                assert_eq!(actual.status, previous.status);
                match (actual.value, previous.value) {
                    (Some(actual), Some(previous)) => assert_close(actual, previous, 1e-8),
                    (None, None) => {}
                    _ => panic!("coefficient presence changed across worker counts"),
                }
            }
        }
        previous = Some(fit);
    }
}

#[test]
fn auto_rank_repair_admits_live_workspace_with_a_public_style_budget() {
    let data = CrossedSignCube::new();
    let prepared = data.prepare(1e4, 1.0, true);
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut config = PreparedOlsFitConfig::default();
        config.execution.mode = mode;
        config.allow_final_fe_recovery = true;
        config.final_residual_tolerance = Some(1e-14);
        let reference = fit_prepared_model_coefficients(&prepared, config)
            .expect("unbounded Auto shifted-model reference");
        check_identified_fit(&reference, &data, 1.0);
        let historical_peak = reference.absorption_report().solver_workspace_peak;
        // This Auto route has an unmodeled historical scope. A finite memory
        // budget must not turn that history into a claim of zero allocation,
        // or treat the released original FE owner as a live unknown parent.
        assert_eq!(historical_peak.bytes(), None, "mode={mode:?}");
        let execution_bytes = 1 << 20;
        let usable_bytes = execution_bytes + (1 << 20);
        config.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
            execution_bytes,
            usable_bytes,
        });
        let bounded = fit_prepared_model_coefficients(&prepared, config)
            .unwrap_or_else(|error| panic!("bounded Auto mode={mode:?}: {error}"));
        check_identified_fit(&bounded, &data, 1.0);
        assert_eq!(bounded.coefficients(), reference.coefficients());
        assert_eq!(
            bounded.kept_generated_columns(),
            reference.kept_generated_columns()
        );
        assert_eq!(
            bounded.dropped_generated_columns(),
            reference.dropped_generated_columns()
        );
        assert_eq!(bounded.residuals(), reference.residuals());
        assert_eq!(bounded.bread(), reference.bread());
        assert_eq!(bounded.rank_certificate(), reference.rank_certificate());
        assert_eq!(bounded.score_certificate(), reference.score_certificate());
        assert_eq!(
            bounded.absorption_report().solver_workspace_peak,
            historical_peak
        );
        assert_eq!(bounded.solver_workspace_peak().bytes(), None);
        // Modeled sub-operations still report a peak within the caller's
        // remaining budget; unknown historical coverage remains explicit.
        if let Some(bytes) = bounded
            .residual_absorption_report()
            .solver_workspace_peak
            .bytes()
        {
            assert!(execution_bytes + u128::from(bytes) <= usable_bytes);
        }
    }
}

#[test]
fn rank_repair_honors_the_explicit_memory_limit() {
    let data = CrossedSignCube::new();
    let ordinary = data.prepare(0.0, 1.0, true);
    let suspicious = data.prepare(1e8, 1.0, true);
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Map;
    let ordinary_fit =
        fit_prepared_model_coefficients(&ordinary, config).expect("ordinary workspace reference");
    let initial = ordinary_fit
        .absorption_report()
        .solver_workspace_peak
        .bytes()
        .expect("known initial MAP workspace");
    let final_residual = ordinary_fit
        .residual_absorption_report()
        .solver_workspace_peak
        .bytes()
        .expect("known final MAP workspace");
    config.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: u128::from(initial.max(final_residual)),
    });
    // The same model shape and solver complete under this limit before any
    // repair is needed. Only the large nuisance shift needs extra workspace.
    let bounded = fit_prepared_model_coefficients(&ordinary, config)
        .expect("ordinary model fits the explicit limit");
    check_identified_fit(&bounded, &data, 1.0);
    let error = fit_prepared_model_coefficients(&suspicious, config)
        .expect_err("repair must not exceed the explicit memory limit");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("rank verification"), "{error}");
}

#[test]
fn nonsuspicious_columns_keep_the_unrepaired_scientific_result() {
    let data = CrossedSignCube::new();
    let prepared = data.prepare(0.0, 1.0, true);
    let ordinary = LinearSolveFrame::from_prepared_ols(&prepared).unwrap();
    // An explicitly borrowed weighted-linear frame is outside the OLS rank
    // repair scope. It supplies the unchanged production arithmetic as a
    // comparator when the public OLS columns need no verification.
    let unmodified = LinearSolveFrame::from_parts(
        ordinary.response(),
        ordinary.raw_weights(),
        ordinary.bread_weight_scale(),
    )
    .unwrap();
    for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
        let expected = fit_prepared_weighted_linear_once(&prepared, unmodified, config)
            .expect("ordinary unrepaired frame");
        let actual = fit_prepared_weighted_linear_once(&prepared, ordinary, config)
            .expect("ordinary public OLS frame");
        check_identified_fit(&actual, &data, 1.0);
        assert_incremental_fit_exact(&actual, &expected);
    }
}

#[test]
fn repaired_rank_is_identified_with_an_exact_zero_outcome() {
    let mut data = CrossedSignCube::new();
    data.outcome.fill(0.0);
    let prepared = data.prepare(1e8, 1.0, true);
    for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
        let fit = fit_prepared_model_coefficients(&prepared, config)
            .expect("zero-outcome shifted design fit");
        assert_eq!(fit.finite_rank(), 2);
        assert_eq!(fit.kept_generated_columns(), &[0, 1]);
        assert!(fit
            .coefficients()
            .iter()
            .all(|coefficient| coefficient.value == Some(0.0)));
        assert!(fit.residuals().iter().all(|&residual| residual == 0.0));
        assert!(fit.rank_certificate().passed);
        assert!(fit.score_certificate().passed);
        assert!(fit.bread().certificate().passed);
        for column in 0..2 {
            assert_close(
                fit.bread().values()[column * fit.bread().dimension() + column],
                1.0 / 800.0,
                1e-10,
            );
        }
    }
}

#[test]
fn repaired_columns_reject_coefficient_and_residual_corruption() {
    for zero_outcome in [false, true] {
        let mut data = CrossedSignCube::new();
        if zero_outcome {
            data.outcome.fill(0.0);
        }
        let prepared = data.prepare(1e8, 1.0, true);
        for config in configs().filter(|config| config.absorb.solver == FeSolverKind::Auto) {
            for (coefficient, residual) in [(true, false), (false, true)] {
                let error = ThreadTeam::new(config.execution.threads)
                    .install(|| {
                        fit_with_controls(
                            &prepared,
                            config,
                            &mut SystemFitReservation,
                            coefficient,
                            residual,
                        )
                    })
                    .unwrap()
                    .expect_err("repaired rank must not bypass independent certificates");
                assert_eq!(error.code, ErrorCode::InvalidData);
            }
        }
    }
}
