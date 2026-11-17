use super::*;

#[test]
fn many_intercept_unit_policy_keeps_paired_finite_mle_across_workers() {
    use crate::ppml::{
        fit_prepared_ppml, LoadedPpmlInput, PpmlMemoryConfig, PpmlSolverFamily, PpmlVceRequest,
        PreparedPpmlAdjustmentKind, PreparedPpmlOutputRequest, PreparedPpmlRequestV1,
    };
    // Every complete design row occurs twice with outcomes (0,2): (1,-1)
    // is an exact full-span orthogonal dual and mu=1 is the finite MLE.
    // Repeated FE declarations have a large null coefficient space without
    // changing that model, exercising the many-effect unit projection policy.
    let (mut ols, _, mut loaded, _) = fixture();
    ols.keep_singletons = true;
    let template = ols.absorbed_terms[0].clone();
    ols.absorbed_terms.clear();
    for effect in 0..33 {
        let mut term = template.clone();
        term.id = AbsorbTermId::new(effect + 1).unwrap();
        term.categorical_sources =
            vec![SourceId::new(if effect % 2 == 0 { 2 } else { 3 }).unwrap()];
        term.label = format!("effect{}", effect + 1);
        ols.absorbed_terms.push(term);
    }
    let n = 480;
    loaded.candidate = vec![true; n].into();
    loaded.outcome = column(
        &(0..n)
            .map(|row| if row % 2 == 0 { 0. } else { 2. })
            .collect::<Vec<_>>(),
    );
    loaded.numeric_sources[0].1 = column(
        &(0..n)
            .map(|row| if (row / 60) % 2 == 0 { -1. } else { 1. })
            .collect::<Vec<_>>(),
    );
    loaded.numeric_sources[1].1 = column(
        &(0..n)
            .map(|row| ((row / 10) % 6) as f64)
            .collect::<Vec<_>>(),
    );
    loaded.numeric_sources[2].1 =
        column(&(0..n).map(|row| ((row / 2) % 5) as f64).collect::<Vec<_>>());
    let mut prior_rows = None;
    for workers in [1, 2, 4] {
        let threads = crate::ThreadConfig::from_requested(workers, true).unwrap();
        let mut ppml = PpmlConfig::default();
        ppml.absorb.lsmr.threads = threads;
        let linear = PreparedOlsFitConfig {
            execution: crate::ols::PreparedOlsExecutionConfig {
                threads,
                ..Default::default()
            },
            ..Default::default()
        };
        let request = PreparedPpmlRequestV1 {
            bias: None,
            ols: ols.clone(),
            clusters: Vec::new(),
            vce: PpmlVceRequest::Robust,
            ppml,
            linear,
            memory: PpmlMemoryConfig::default(),
            memory_preflight_probe: None,
            allocation_snapshot_probe: None,
            phase_allocation_probe: None,
            linear_phase_allocation_probe: None,
            action_allocation_snapshot_probe: None,
            action_allocation_start: None,
            action_allocation_previous_baseline_bytes: None,
            separation: rectifier_only(),
            output: PreparedPpmlOutputRequest {
                residual: None,
                xb: false,
                absorbed: false,
                eta: true,
                mean: true,
                requested_observation_columns: 2,
                publication_matrix_cells: 512,
            },
            solver: PpmlSolverFamily::Auto,
            lsmr_preconditioner: crate::fe::LsmrPreconditioner::Auto,
            inject_fe_recovery_allocation_failure: false,
            inject_topology_sharing: false,
            inject_separation_cg: false,
        };
        let result = fit_prepared_ppml(
            request,
            LoadedPpmlInput {
                bias_units: None,
                ols: loaded.clone(),
                adjustment: None,
                adjustment_kind: PreparedPpmlAdjustmentKind::None,
            },
        )
        .unwrap_or_else(|error| panic!("workers={workers}: {error:?}"));
        assert_eq!(result.sample().original_rows(), &(0..n).collect::<Vec<_>>());
        assert_eq!(result.separation().separated_count, 0);
        assert!(result
            .separation()
            .methods
            .iter()
            .any(
                |method| method.method == SeparationMethod::IteratedRectifier
                    && method.status == MethodStatus::NoSeparation
            ));
        assert_eq!(result.output().mean().len(), n);
        for (&mean, &eta) in result.output().mean().iter().zip(result.output().eta()) {
            assert!((mean - 1.).abs() < 1e-10 && eta.abs() < 1e-10);
        }
        if let Some(rows) = &prior_rows {
            assert_eq!(result.sample().original_rows(), rows);
        }
        prior_rows = Some(result.sample().original_rows().to_vec());
    }
}
