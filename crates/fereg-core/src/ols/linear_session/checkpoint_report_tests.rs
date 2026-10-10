use super::*;

fn report(solver: FeSolverKind) -> SolveReport {
    SolveReport {
        requested_solver: solver,
        used_solver: solver,
        iterations: if solver == FeSolverKind::Map { 0 } else { 12 },
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::ZERO,
        stop_reason: crate::fe::SolveStopReason::Certified,
        certificate: crate::fe::FeCertificate {
            absolute_infinity_norm: 1e-12,
            relative_infinity_norm: 1e-12,
            relative_two_norm: 1e-12,
            passed: true,
        },
        normal_equation_residual: 0.,
        setup_seconds: 1.,
        solve_seconds: 2.,
        certification_seconds: 3.,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.,
        warnings: Vec::new(),
    }
}

#[test]
fn ppml_canonical_report_merge_preserves_work_and_fallback_in_any_order() {
    let zero = report(FeSolverKind::Map);
    let mut lsmr = report(FeSolverKind::Lsmr);
    lsmr.requested_preconditioner = Some(crate::fe::LsmrPreconditioner::Auto);
    lsmr.used_preconditioner = Some(crate::fe::LsmrPreconditioner::Schwarz);
    for (mut a, b) in [(zero.clone(), lsmr.clone()), (lsmr.clone(), zero.clone())] {
        merge_canonical_projection_reports(&mut a, b).unwrap();
        assert_eq!(a.used_solver, FeSolverKind::Lsmr);
        assert_eq!(a.used_preconditioner, lsmr.used_preconditioner);
        assert_eq!(a.certification_seconds, 6.);
        assert_eq!(a.iterations, 12);
    }
    let mut schur = report(FeSolverKind::Lsmr);
    schur.used_solver = FeSolverKind::Map;
    for reports in [
        [zero.clone(), lsmr.clone(), schur.clone()],
        [lsmr.clone(), schur.clone(), zero.clone()],
        [schur.clone(), zero.clone(), lsmr.clone()],
        [zero.clone(), schur.clone(), lsmr.clone()],
        [lsmr.clone(), zero.clone(), schur.clone()],
        [schur, lsmr.clone(), zero.clone()],
    ] {
        let mut merged = reports[0].clone();
        for source in &reports[1..] {
            merge_canonical_projection_reports(&mut merged, source.clone()).unwrap();
        }
        assert!(merged.certified() && !merged.fallback_used);
        assert_eq!(merged.requested_solver, FeSolverKind::Lsmr);
        assert_eq!(merged.used_solver, FeSolverKind::Map);
        assert_eq!(merged.used_preconditioner, None);
        assert_eq!(merged.checkpoint_retries, 0);
        assert_eq!(merged.certification_seconds, 9.);
        assert_eq!(merged.iterations, 12);
    }
    let mut fallback = report(FeSolverKind::Lsmr);
    fallback.used_solver = FeSolverKind::Map;
    fallback.fallback_used = true;
    fallback.checkpoint_retries = 1;
    fallback.warnings.push("certified fallback".to_owned());
    let reports = [zero, lsmr, fallback];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut merged = reports[order[0]].clone();
        for i in &order[1..] {
            merge_canonical_projection_reports(&mut merged, reports[*i].clone()).unwrap();
        }
        assert!(merged.certified() && merged.fallback_used);
        assert_eq!(merged.requested_solver, FeSolverKind::Lsmr);
        assert_eq!(merged.used_solver, FeSolverKind::Map);
        assert_eq!(merged.checkpoint_retries, 1);
        assert_eq!(merged.setup_seconds, 3.);
        assert_eq!(merged.solve_seconds, 6.);
        assert_eq!(merged.certification_seconds, 9.);
        assert_eq!(merged.warnings, vec!["certified fallback"]);
    }
    let mut invalid = report(FeSolverKind::Map);
    invalid.iterations = 1;
    assert!(merge_canonical_projection_reports(&mut invalid, reports[1].clone()).is_err());
    let mut invalid = report(FeSolverKind::Map);
    invalid.certificate.passed = false;
    assert!(merge_canonical_projection_reports(&mut invalid, reports[1].clone()).is_err());
}
