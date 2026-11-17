use super::*;

#[test]
fn disconnected_offset_truth_survives_component_mass_disparity() {
    let n = 480;
    let pair: Vec<_> = (0..n).map(|row| row / 2).collect();
    let g: Vec<_> = pair
        .iter()
        .map(|&p| (1 + p % 5 + 5 * (p / 120)) as f64)
        .collect();
    let h: Vec<_> = pair
        .iter()
        .map(|&p| (1 + (p / 5) % 4 + 4 * (p / 120)) as f64)
        .collect();
    let x: Vec<_> = pair
        .iter()
        .map(|&p| (p as f64 / 17.).sin() + (p % 7) as f64 / 5.)
        .collect();
    let offset: Vec<_> = pair.iter().map(|&p| 0.1 * (p as f64 / 19.).cos()).collect();
    let truth: Vec<f64> = (0..n)
        .map(|i| 0.3 + 0.2 * x[i] + 0.1 * g[i] - 0.07 * h[i] + offset[i])
        .collect();
    // Equal-weight pairs share every design value. Their outcomes sum to
    // twice exp(truth), proving every full-model Poisson score is zero.
    let y: Vec<_> = truth
        .iter()
        .enumerate()
        .map(|(i, eta)| eta.exp() * if i % 2 == 0 { 0.9 } else { 1.1 })
        .collect();
    let model = prepared_many_fe(&y, &x, &[g, h]);
    for disparity in [1., 1e13] {
        let mut weights: Vec<_> = pair
            .iter()
            .map(|&p| (1. + (p % 11) as f64 / 7.) * if p < 120 { 1. } else { disparity })
            .collect();
        let average = weights.iter().sum::<f64>() / n as f64;
        for weight in &mut weights {
            *weight /= average;
        }
        for (mode, execution) in [
            (
                crate::options::MemoryMode::Low,
                PreparedOlsExecutionMode::Low,
            ),
            (
                crate::options::MemoryMode::Fast,
                PreparedOlsExecutionMode::Fast,
            ),
        ] {
            for workers in [1, 4] {
                for tolerance in [1e-8, 1e-12] {
                    let mut config = PreparedPpmlIrlsConfig::default();
                    config.ppml.tolerance = tolerance;
                    config.ppml.score_tolerance = tolerance;
                    config.memory.requested = mode;
                    config.linear.execution.mode = execution;
                    config.linear.execution.threads =
                        crate::ThreadConfig::from_requested(workers, true).unwrap();
                    let plan =
                        plan_ppml_memory(memory_dimensions(&model, config), config.memory).unwrap();
                    let fit =
                        fit_prepared_ppml_irls(&model, &y, &weights, &offset, config, plan, None)
                            .unwrap();
                    let error = fit
                        .eta()
                        .iter()
                        .zip(&truth)
                        .map(|(&a, &b)| (a - b).abs() / (1. + b.abs()))
                        .fold(0.0_f64, f64::max);
                    eprintln!("component truth disparity={disparity} mode={mode:?} workers={workers} tolerance={tolerance} error={error:.17e}");
                    assert_eq!(fit.eta().len(), n);
                    for (&mean, &eta) in fit.mean().iter().zip(&truth) {
                        assert!((mean - eta.exp()).abs() < 1e-10 * (1. + eta.exp()));
                    }
                    assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                    assert!(error < 1e-10, "disparity={disparity} mode={mode:?} workers={workers} tolerance={tolerance} error={error}");
                }
            }
        }
    }
}

#[test]
fn disconnected_offset_component_checkpoint_cap_audit() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let n = 480;
    let pair: Vec<_> = (0..n).map(|row| row / 2).collect();
    let g: Vec<_> = pair
        .iter()
        .map(|&p| (1 + p % 5 + 5 * (p / 120)) as f64)
        .collect();
    let h: Vec<_> = pair
        .iter()
        .map(|&p| (1 + (p / 5) % 4 + 4 * (p / 120)) as f64)
        .collect();
    let x: Vec<_> = pair
        .iter()
        .map(|&p| (p as f64 / 17.).sin() + (p % 7) as f64 / 5.)
        .collect();
    let offset: Vec<_> = pair.iter().map(|&p| 0.1 * (p as f64 / 19.).cos()).collect();
    let truth: Vec<f64> = (0..n)
        .map(|i| 0.3 + 0.2 * x[i] + 0.1 * g[i] - 0.07 * h[i] + offset[i])
        .collect();
    // Equal-weight pairs share every design value. Their outcomes sum to
    // twice exp(truth), proving every full-model Poisson score is zero.
    let y: Vec<_> = truth
        .iter()
        .enumerate()
        .map(|(i, eta)| eta.exp() * if i % 2 == 0 { 0.9 } else { 1.1 })
        .collect();
    let model = prepared_many_fe(&y, &x, &[g.clone(), h.clone()]);

    let mut weights: Vec<_> = pair
        .iter()
        .map(|&p| (1. + (p % 11) as f64 / 7.) * if p < 120 { 1. } else { 1e13 })
        .collect();
    let average = weights.iter().sum::<f64>() / n as f64;
    for weight in &mut weights {
        *weight /= average;
    }
    for (mode, execution) in [
        (
            crate::options::MemoryMode::Low,
            PreparedOlsExecutionMode::Low,
        ),
        (
            crate::options::MemoryMode::Fast,
            PreparedOlsExecutionMode::Fast,
        ),
    ] {
        let mut config = PreparedPpmlIrlsConfig::default();
        config.ppml.tolerance = 1e-12;
        config.ppml.score_tolerance = 1e-12;
        config.memory.requested = mode;
        config.linear.execution.mode = execution;
        config.linear.execution.threads = crate::ThreadConfig::from_requested(1, true).unwrap();
        let plan = plan_ppml_memory(memory_dimensions(&model, config), config.memory).unwrap();
        let admitted =
            fit_prepared_ppml_irls(&model, &y, &weights, &offset, config, plan, None).unwrap();
        let admitted_plan = admitted.diagnostics().memory.plan;
        let seed_topology = admitted_plan.selected_inventory().fe_solver_topology_bytes;
        let cap = admitted_plan.selected_bytes() - seed_topology - 1;
        let mut boundary = plan;
        boundary.usable_bytes = Some(cap + 1);
        let boundary_fit =
            fit_prepared_ppml_irls(&model, &y, &weights, &offset, config, boundary, None).unwrap();
        eprintln!(
            "component checkpoint exact boundary mode={mode:?} cap={} actual={} checkpoint={:?}",
            cap + 1,
            boundary_fit.diagnostics().memory.plan.selected_bytes(),
            boundary_fit.diagnostics().solver.checkpoint
        );
        assert_eq!(
            boundary_fit.diagnostics().solver.checkpoint,
            super::super::super::PpmlSolverFamily::Lsmr
        );
        assert!(boundary_fit.diagnostics().memory.plan.selected_bytes() <= cap + 1);
        assert!(relative_infinity_difference(boundary_fit.eta(), &truth) < 1e-10);
        let mut constrained = plan;
        constrained.usable_bytes = Some(cap);
        let fitted =
            fit_prepared_ppml_irls(&model, &y, &weights, &offset, config, constrained, None)
                .unwrap();
        let report = fitted.diagnostics();
        let eta_error = fitted
            .eta()
            .iter()
            .zip(&truth)
            .map(|(a, b)| (a - b).abs() / (1. + b.abs()))
            .fold(0.0_f64, f64::max);
        let mean_error = fitted
            .mean()
            .iter()
            .zip(&truth)
            .map(|(a, b)| (a - b.exp()).abs() / (1. + b.exp()))
            .fold(0.0_f64, f64::max);
        let local_score = (1..=4)
            .map(|level| {
                let mut score = 0.;
                let mut mass = 0.;
                for row in 0..n {
                    if h[row] == level as f64 {
                        score += weights[row] * (y[row] - fitted.mean()[row]);
                        mass += weights[row] * fitted.mean()[row];
                    }
                }
                (score / mass).abs()
            })
            .fold(0.0_f64, f64::max);
        eprintln!("component cap audit mode={mode:?} base={} admitted={} seed_topology={seed_topology} cap={cap} actual={} checkpoint={:?} eta_error={eta_error:.17e} mean_error={mean_error:.17e} local_score={local_score:.17e} global_score={:.17e}", plan.selected_bytes(), admitted_plan.selected_bytes(), report.memory.plan.selected_bytes(), report.solver.checkpoint, report.final_score.relative_infinity_norm);
        assert!(report.memory.plan.selected_bytes() <= cap);
        assert_eq!(
            report.solver.checkpoint,
            super::super::super::PpmlSolverFamily::Map
        );
        assert_eq!(fitted.eta().len(), n);
        assert!(report.final_score.passed);
        // The optional reserve's refusal must retain SPEC §20 accuracy.
        // This is deliberately distinct from the admitted route's stronger
        // 1e-10 fixture oracle above: a global weighted certificate does not
        // promise forward error below the requested outer tolerance on each
        // low-mass component (SPEC §§13,16).
        for (row, &target) in truth.iter().enumerate() {
            assert!((fitted.eta()[row] - target).abs() <= 1e-9 + 1e-7 * target.abs());
            assert!((fitted.mean()[row] - target.exp()).abs() <= 1e-9 + 1e-7 * target.exp());
        }
        assert!((fitted.coefficients()[0].value.unwrap() - 0.2).abs() <= 1e-10 + 1e-7 * 0.2);
    }
}
