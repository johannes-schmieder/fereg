
#[cfg(test)]
mod generic_budget_regression {
    use super::*;
    use crate::fe::EffectTerm;

    fn check_budget(solver: FeSolverKind) {
        let n = 48;
        let left = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let right = (0..n).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
        let slope = (0..n).map(|row| (row / 12) as f64 - 1.5).collect::<Vec<_>>();
        // The four equal-weight observations in every crossed cell have
        // residuals [1,-1,-1,1], orthogonal to both intercept and slope.
        let noise = (0..n).map(|row| [1.0, -1.0, -1.0, 1.0][row / 12]).collect::<Vec<_>>();
        let outcome = (0..n).map(|row| 2.0 * left[row] as f64
            + 3.0 * right[row] as f64 * slope[row] + noise[row]).collect::<Vec<_>>();
        let design = FeDesign::new(n, vec![
            EffectTerm::new(left, 4, true, vec![]).unwrap(),
            EffectTerm::new(right, 3, false, vec![slope]).unwrap(),
        ]).unwrap();
        let weights = vec![1.0; n];
        let mut config = AbsorbConfig { solver, ..AbsorbConfig::default() };
        config.map.tolerance = 1e-12;
        config.lsmr.tolerance = 1e-12;
        let mut values = DenseMatrix::from_columns(&[outcome.clone()]).unwrap();
        let report = absorb_fixed_effects(&design, &weights, &mut values, config).unwrap();
        assert!(report.certified());
        assert!(values.as_slice().iter().zip(&noise).all(|(a,b)| (a-b).abs()<1e-10));
        config.solver_memory_budget = Some(FeSolverMemoryBudget { execution_bytes: 0, usable_bytes: 1 });
        let mut values = DenseMatrix::from_columns(&[outcome]).unwrap();
        let error = absorb_fixed_effects(&design, &weights, &mut values, config)
            .expect_err("generic slope projection must enforce its joint workspace budget");
        assert_eq!(error.code, ErrorCode::Allocation);
    }
    #[test] fn generic_auto_enforces_joint_budget() { check_budget(FeSolverKind::Auto); }
    #[test] fn generic_map_enforces_joint_budget() { check_budget(FeSolverKind::Map); }
    #[test] fn generic_lsmr_enforces_joint_budget() { check_budget(FeSolverKind::Lsmr); }
    #[test] fn generic_cg_enforces_joint_budget() { check_budget(FeSolverKind::Cg); }
}
