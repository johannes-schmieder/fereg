use super::*;
use crate::memory::estimate_fe_solver_memory;
use crate::ppml::{plan_ppml_memory, PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryPlan};
use crate::MemoryMode;

/// The memory module's minimum headroom (512 MiB) below `available_bytes`.
const HEADROOM: u128 = 512 * 1024 * 1024;

fn plan(available_bytes: Option<u128>) -> PpmlMemoryPlan {
    let shape = PpmlMemoryDimensions {
        metadata: Default::default(),
        observations: 10_000,
        original_observations: 10_000,
        candidate_observations: 10_000,
        loaded_numeric_columns: 4,
        retained_base_adjustment_capacity_elements: 0,
        retained_raw_preseparation_probability_weight_elements: 0,
        finite_columns: 3,
        coefficient_slots: 3,
        reported_coefficient_slots: 3,
        fe_dimensions: 2,
        fe_total_levels: 3_000,
        fe_max_levels: 2_000,
        fe_total_channels: 2,
        fe_max_channels: 1,
        fe_max_level_members: 20,
        fe_coefficient_directions: 3_000,
        fe_max_effect_directions: 2_000,
        cluster_dimensions: 0,
        cluster_subset_groups: 0,
        separation_workspace_vectors: 8,
        separation_singleton_enabled: true,
        separation_method_count: 3,
        separation_generation_capacity: 10_000,
        separation_rebase_enabled: true,
        separation_exact_fe_enabled: true,
        separation_mu_enabled: false,
        simplex_tableau_cells: 100_000,
        requested_output_columns: 0,
        requested_mean_output: false,
        requested_saved_fe_channels: 0,
        publication_matrix_cells: 100,
        threads: 1,
        allocation_audit_enabled: false,
    };
    plan_ppml_memory(
        shape,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            available_bytes,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("memory plan")
}

/// An admitted retained-CG detector policy on `plan`, with its LSMR charge.
fn retained_cg(plan: &Cell<PpmlMemoryPlan>) -> (DetectorPolicy<'_>, LsmrAdmission) {
    let estimate = estimate_fe_solver_memory(10_000, 4, &[2_000, 1_000], 1, Some(4)).unwrap();
    let mut linear = PreparedOlsFitConfig::default();
    linear.absorb.solver = FeSolverKind::Cg;
    linear.absorb.retain_coefficients = true;
    let ppml = PpmlConfig {
        absorb: linear.absorb,
        ..PpmlConfig::default()
    };
    let base = plan.get().selected_bytes();
    let admission = lsmr_admission(
        FeSolverKind::Cg,
        &estimate,
        linear.absorb.lsmr.preconditioner,
        10_000,
        0,
        base,
    )
    .unwrap()
    .expect("retained CG resolves an LSMR re-execution");
    (
        DetectorPolicy::new(ppml, linear, None, Some((admission, plan))),
        admission,
    )
}

fn failure(code: ErrorCode, message: &'static str) -> FeregError {
    FeregError::new(code, message)
}

#[test]
fn certified_first_attempt_is_never_reexecuted_or_charged() {
    let cell = Cell::new(plan(None));
    let before = cell.get();
    let (policy, _) = retained_cg(&cell);
    let mut solvers = Vec::new();
    let (value, ppml, linear) = policy
        .run(0, |ppml, linear| {
            assert_eq!(ppml.absorb, linear.absorb);
            solvers.push(linear.absorb.solver);
            Ok(7)
        })
        .unwrap();
    assert_eq!(value, 7);
    assert_eq!(solvers, [FeSolverKind::Cg]);
    assert_eq!((ppml, linear), policy.admitted());
    assert_eq!(cell.get(), before);
}

#[test]
fn numerical_failure_reexecutes_once_with_charged_lsmr() {
    for code in [ErrorCode::NonConvergence, ErrorCode::InvalidData] {
        let cell = Cell::new(plan(None));
        let (policy, admission) = retained_cg(&cell);
        let mut calls = Vec::new();
        let (value, ppml, linear) = policy
            .run(3, |ppml, linear| {
                calls.push((linear.absorb.solver, linear.absorb.solver_memory_budget));
                assert_eq!(ppml.absorb, linear.absorb);
                if linear.absorb.solver == FeSolverKind::Cg {
                    Err(failure(
                        code,
                        "retained CG projection failed its certificate",
                    ))
                } else {
                    Ok(11)
                }
            })
            .unwrap();
        assert_eq!(value, 11);
        let budget = FeSolverMemoryBudget {
            execution_bytes: admission.base,
            usable_bytes: admission.base + admission.bytes,
        };
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1], (FeSolverKind::Lsmr, Some(budget)));
        // The returned policy is the one a terminal replay must re-execute.
        assert_eq!(linear.absorb.solver, FeSolverKind::Lsmr);
        assert_eq!(ppml.absorb, linear.absorb);
        assert_eq!(
            linear.absorb.retain_coefficients,
            policy.admitted().1.absorb.retain_coefficients
        );
        assert_eq!(cell.get().separation_projection_bytes, admission.bytes);
    }
}

#[test]
fn failed_reexecution_reports_the_first_error_but_propagates_break_and_memory() {
    let cell = Cell::new(plan(None));
    let (policy, _) = retained_cg(&cell);
    let first = "retained CG direction failed certification";
    let error = policy
        .run(0, |_, linear| {
            Err::<(), _>(if linear.absorb.solver == FeSolverKind::Cg {
                failure(ErrorCode::NonConvergence, first)
            } else {
                failure(ErrorCode::InvalidData, "LSMR did not certify")
            })
        })
        .unwrap_err();
    assert_eq!(
        (error.code, error.message.as_str()),
        (ErrorCode::NonConvergence, first)
    );
    for code in [
        ErrorCode::Interrupted,
        ErrorCode::Allocation,
        ErrorCode::Internal,
    ] {
        let error = policy
            .run(0, |_, linear| {
                Err::<(), _>(if linear.absorb.solver == FeSolverKind::Cg {
                    failure(ErrorCode::NonConvergence, first)
                } else {
                    failure(code, "re-execution stopped")
                })
            })
            .unwrap_err();
        assert_eq!(error.code, code);
    }
}

#[test]
fn only_numerical_failures_of_admitted_retained_cg_are_reexecuted() {
    let cell = Cell::new(plan(None));
    let (policy, _) = retained_cg(&cell);
    for code in [
        ErrorCode::Interrupted,
        ErrorCode::Allocation,
        ErrorCode::Internal,
    ] {
        let mut calls = 0;
        let error = policy
            .run(0, |_, _| {
                calls += 1;
                Err::<(), _>(failure(code, "not a certificate failure"))
            })
            .unwrap_err();
        assert_eq!((calls, error.code), (1, code));
    }
    // Other admitted detector solvers resolve none; Auto re-executes as
    // retained CG and so resolves the same charge.
    let estimate = estimate_fe_solver_memory(100, 2, &[10, 10], 1, Some(4)).unwrap();
    assert_eq!(
        lsmr_admission(
            FeSolverKind::Auto,
            &estimate,
            LsmrPreconditioner::Auto,
            100,
            0,
            0
        ),
        lsmr_admission(
            FeSolverKind::Cg,
            &estimate,
            LsmrPreconditioner::Auto,
            100,
            0,
            0
        ),
    );
    for solver in [FeSolverKind::Map, FeSolverKind::Lsmr] {
        let admission =
            lsmr_admission(solver, &estimate, LsmrPreconditioner::Auto, 100, 0, 0).unwrap();
        assert_eq!(admission, None);
    }
    let unadmitted = DetectorPolicy::new(
        PpmlConfig::default(),
        PreparedOlsFitConfig::default(),
        None,
        None,
    );
    let mut calls = 0;
    let error = unadmitted
        .run(0, |_, _| {
            calls += 1;
            Err::<(), _>(failure(ErrorCode::NonConvergence, "uncertified"))
        })
        .unwrap_err();
    assert_eq!((calls, error.code), (1, ErrorCode::NonConvergence));
}

#[test]
fn reexecution_that_does_not_fit_the_plan_reports_the_first_error() {
    let unlimited = plan(None);
    let estimate = estimate_fe_solver_memory(10_000, 4, &[2_000, 1_000], 1, Some(4)).unwrap();
    let admission = lsmr_admission(
        FeSolverKind::Cg,
        &estimate,
        LsmrPreconditioner::Auto,
        10_000,
        0,
        unlimited.selected_bytes(),
    )
    .unwrap()
    .unwrap();
    // The LSMR charge includes the CG term's coefficient and RHS banks.
    assert!(admission.bytes >= estimate.lsmr_bytes(LsmrPreconditioner::Auto));
    // A usable limit just below the re-execution's peak.
    let usable = unlimited.required_peak_bytes + admission.bytes - 1;
    let tight = plan(Some(usable + HEADROOM.max((usable - 1) / 4)));
    assert_eq!(tight.usable_bytes, Some(usable));
    let cell = Cell::new(tight);
    let mut linear = PreparedOlsFitConfig::default();
    linear.absorb.solver = FeSolverKind::Cg;
    let policy = DetectorPolicy::new(
        PpmlConfig::default(),
        linear,
        None,
        Some((admission, &cell)),
    );
    let mut calls = 0;
    let error = policy
        .run(0, |_, _| {
            calls += 1;
            Err::<(), _>(failure(ErrorCode::NonConvergence, "uncertified"))
        })
        .unwrap_err();
    assert_eq!((calls, error.code), (1, ErrorCode::NonConvergence));
    assert_eq!(cell.get(), tight);
}

/// An admitted Auto detector policy on `plan`: its retained-CG re-execution
/// under the same budget and the retained-CG LSMR charge.
fn auto_routed(plan: &Cell<PpmlMemoryPlan>) -> (DetectorPolicy<'_>, LsmrAdmission) {
    let (cg, admission) = retained_cg(plan);
    let (cg_ppml, cg_linear) = cg.admitted();
    let mut linear = cg_linear;
    linear.absorb.solver = FeSolverKind::Auto;
    linear.absorb.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 5,
        usable_bytes: 50,
    });
    let mut cg_linear = cg_linear;
    cg_linear.absorb.solver_memory_budget = linear.absorb.solver_memory_budget;
    let cg_ppml = PpmlConfig {
        absorb: cg_linear.absorb,
        ..cg_ppml
    };
    let ppml = PpmlConfig {
        absorb: linear.absorb,
        ..cg_ppml
    };
    (
        DetectorPolicy::new(
            ppml,
            linear,
            Some((cg_ppml, cg_linear)),
            Some((admission, plan)),
        ),
        admission,
    )
}

#[test]
fn certified_auto_attempt_is_never_reexecuted_or_charged() {
    let cell = Cell::new(plan(None));
    let before = cell.get();
    let (policy, _) = auto_routed(&cell);
    let mut solvers = Vec::new();
    let (value, ppml, linear) = policy
        .run(0, |ppml, linear| {
            assert_eq!(ppml.absorb, linear.absorb);
            solvers.push(linear.absorb.solver);
            Ok(5)
        })
        .unwrap();
    assert_eq!((value, solvers), (5, vec![FeSolverKind::Auto]));
    assert_eq!((ppml, linear), policy.admitted());
    assert_eq!(cell.get(), before);
}

#[test]
fn failed_auto_step_reexecutes_as_retained_cg_under_the_same_budget() {
    for code in [ErrorCode::NonConvergence, ErrorCode::InvalidData] {
        let cell = Cell::new(plan(None));
        let before = cell.get();
        let (policy, _) = auto_routed(&cell);
        let admitted = policy.admitted().1;
        let mut calls = Vec::new();
        let (value, ppml, linear) = policy
            .run(2, |ppml, linear| {
                assert_eq!(ppml.absorb, linear.absorb);
                calls.push(linear.absorb.solver);
                if linear.absorb.solver == FeSolverKind::Auto {
                    Err(failure(code, "Auto projection failed its certificate"))
                } else {
                    Ok(13)
                }
            })
            .unwrap();
        assert_eq!(value, 13);
        assert_eq!(calls, [FeSolverKind::Auto, FeSolverKind::Cg]);
        // Only the solver differs; a terminal replay re-executes this policy.
        let mut expected = admitted;
        expected.absorb.solver = FeSolverKind::Cg;
        assert_eq!(linear, expected);
        assert_eq!(ppml.absorb, expected.absorb);
        // Retained CG runs within the admitted budget: nothing is charged.
        assert_eq!(cell.get(), before);
    }
}

#[test]
fn failed_auto_and_cg_steps_continue_with_lsmr_and_report_the_cg_error() {
    let cell = Cell::new(plan(None));
    let (policy, admission) = auto_routed(&cell);
    let mut calls = Vec::new();
    let (value, ppml, linear) = policy
        .run(1, |_, linear| {
            calls.push(linear.absorb.solver);
            match linear.absorb.solver {
                FeSolverKind::Lsmr => Ok(17),
                _ => Err(failure(ErrorCode::NonConvergence, "uncertified")),
            }
        })
        .unwrap();
    assert_eq!(value, 17);
    assert_eq!(
        calls,
        [FeSolverKind::Auto, FeSolverKind::Cg, FeSolverKind::Lsmr]
    );
    // The LSMR policy is the retained-CG step's re-execution.
    assert_eq!(
        linear.absorb.solver_memory_budget,
        Some(FeSolverMemoryBudget {
            execution_bytes: admission.base,
            usable_bytes: admission.base + admission.bytes,
        })
    );
    assert_eq!(ppml.absorb, linear.absorb);
    assert_eq!(cell.get().separation_projection_bytes, admission.bytes);

    let cell = Cell::new(plan(None));
    let (policy, _) = auto_routed(&cell);
    let cg = "retained CG direction failed certification";
    let error = policy
        .run(0, |_, linear| {
            Err::<(), _>(match linear.absorb.solver {
                FeSolverKind::Auto => failure(ErrorCode::InvalidData, "Auto did not certify"),
                FeSolverKind::Cg => failure(ErrorCode::NonConvergence, cg),
                _ => failure(ErrorCode::InvalidData, "LSMR did not certify"),
            })
        })
        .unwrap_err();
    assert_eq!(
        (error.code, error.message.as_str()),
        (ErrorCode::NonConvergence, cg)
    );
}

#[test]
fn auto_step_stops_on_break_memory_or_internal_errors() {
    for code in [
        ErrorCode::Interrupted,
        ErrorCode::Allocation,
        ErrorCode::Internal,
    ] {
        // Of the Auto attempt itself.
        let cell = Cell::new(plan(None));
        let (policy, _) = auto_routed(&cell);
        let mut calls = 0;
        let error = policy
            .run(0, |_, _| {
                calls += 1;
                Err::<(), _>(failure(code, "not a certificate failure"))
            })
            .unwrap_err();
        assert_eq!((calls, error.code), (1, code));
        // Of its retained-CG re-execution.
        let mut solvers = Vec::new();
        let error = policy
            .run(0, |_, linear| {
                solvers.push(linear.absorb.solver);
                Err::<(), _>(if linear.absorb.solver == FeSolverKind::Auto {
                    failure(ErrorCode::NonConvergence, "uncertified")
                } else {
                    failure(code, "re-execution stopped")
                })
            })
            .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(solvers, [FeSolverKind::Auto, FeSolverKind::Cg]);
        assert_eq!(cell.get(), plan(None));
    }
}
