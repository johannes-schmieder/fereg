    #[test]
    fn registered_nonretaining_cmg_correction_probe() {
        let rows = 48;
        let left = (0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let right = (0..rows).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
        let li = LevelIndex::from_codes(&left, 4).unwrap();
        let ri = LevelIndex::from_codes(&right, 3).unwrap();
        let design = || BorrowedInterceptFeDesign::new(rows,
            vec![effect(&left, 4, &li), effect(&right, 3, &ri)]).unwrap();
        let noise = (0..rows).map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 }).collect::<Vec<_>>();
        let input = DenseMatrix::from_columns(&[(0..rows).map(|row|
            noise[row] + 2e-9 * left[row] as f64 + 1e-9 * right[row] as f64).collect()]).unwrap();
        let mut failed = 0;
        for threads in [1, 8] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                for (frame, scale) in [None, Some(1e-100), Some(1.0), Some(1e100)].into_iter().enumerate() {
                    let raw = scale.map(|scale| (0..rows).map(|row| scale * (1 + row % 12) as f64).collect::<Vec<_>>());
                    let weights = raw.as_deref().map_or(BorrowedMapWeights::Unit, BorrowedMapWeights::Provided);
                    for (case, (requested, save, short)) in [(FeSolverKind::Auto, true, false),
                        (FeSolverKind::Auto, true, true), (FeSolverKind::Auto, false, false),
                        (FeSolverKind::Cmg, true, false)].into_iter().enumerate() {
                        let mut options = config(1);
                        options.threads = ThreadConfig::from_requested(threads, true).unwrap();
                        options.map.tolerance = 1e-8;
                        options.requested_solver = requested;
                        options.retain_coefficients = save;
                        let mut session = CmgAbsorber::build(design(), weights, options, false).unwrap().with_ols_final_reuse(true);
                        let estimate = estimate_fe_solver_memory(rows, 1, &[4, 3], threads, None).unwrap();
                        let required = estimate.cg_bytes + 4 * estimate.coefficient_batch_bytes + 8;
                        session.config.solver_memory_budget = Some(FeSolverMemoryBudget {
                            execution_bytes: 512, usable_bytes: 512 + required - u128::from(short),
                        });
                        let admitted = requested == FeSolverKind::Auto && !short;
                        assert_eq!(retained_cg_correction_fits(&design(), 1, session.config,
                            SolverWorkspacePeak::from_bytes(8).unwrap()).unwrap(), admitted);
                        // Match the existing isolated correction-budget test,
                        // but exercise its previously skipped nonretaining arms.
                        session.memory = CmgWorkspacePlan::EMPTY;
                        match session.absorb_final_residual(input.clone(), 1e-12) {
                            Ok(result) => {
                                let (within, report, coefficients) = result.into_full_parts();
                                let difference = within.as_slice().iter().zip(&noise)
                                    .map(|(&value, &expected)| (value - expected).abs()).fold(0.0_f64, f64::max);
                                let passed = report.certified() && difference < 1e-11
                                    && coefficients.is_some() == (save && admitted);
                                failed += usize::from(!passed);
                                println!("FEREG_CG_PROBE|cmg|{threads}|{frame}|{case}|{passed}|{difference:e}|{}|{:?}",
                                    report.iterations, report.solver_workspace_peak.bytes());
                                if let Some(coefficients) = coefficients {
                                    assert_coefficients_reconstruct(&input, &within, &[&left, &right], &coefficients, 1e-12);
                                }
                            }
                            Err(error) => {
                                failed += 1;
                                println!("FEREG_CG_PROBE|cmg|{threads}|{frame}|{case}|false|ERROR|{:?}|{}", error.code, error.message);
                            }
                        }
                    }
                }
            });
        }
        assert_eq!(failed, 0, "independent known-within CMG correction failures");
    }
