    #[test]
    fn cycle_route_admission_includes_descriptor_and_preserves_excluded_routes() {
        use crate::fe::LsmrPreconditioner;
        let levels = 256;
        let groups = levels * 24;
        let n = groups * 3;
        let effects = vec![(0..n).map(|r| (r / 3) as f64).collect::<Vec<_>>(),
            (0..n).map(|r| ((r / 3 + usize::from(r % 3 == 1)) % levels) as f64).collect()];
        let outcome: Vec<_> = (0..n).map(|r| (r as f64 * 0.17).sin()).collect();
        for k in [6, 50] {
            let columns: Vec<Vec<f64>> = (0..k).map(|c| (0..n)
                .map(|r| ((r * (c + 3)) as f64 * 0.07).cos()).collect()).collect();
            let data = prepared_multiway_categorical(&outcome, &columns, &effects);
            let rhs = data.fit_model().columns.len() + 1;
            let mut reservation = crate::memory::CycleProbeReservation::new(None);
            let descriptor = data.fit_cycle_probe_design(&mut reservation).unwrap();
            let held = descriptor.cycle_descriptor_bytes();
            assert!(reservation.peak.bytes().unwrap() as u128 > held);
            let mut denied = crate::memory::CycleProbeReservation::new(Some(crate::memory::FeSolverMemoryBudget {
                execution_bytes: 500, usable_bytes: 501,
            }));
            assert_eq!(data.fit_cycle_probe_design(&mut denied).unwrap_err().code, ErrorCode::Allocation);
            assert_eq!(denied.peak.bytes(), Some(0));
            for threads in [1, 8, 16] {
                let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
                pool.install(|| {
                    for retained in [false, true] {
                        for preconditioner in [LsmrPreconditioner::Auto, LsmrPreconditioner::Off, LsmrPreconditioner::Diagonal] {
                            let mut config = PreparedOlsFitConfig::default();
                            config.allow_final_fe_recovery = true;
                            config.absorb.retain_coefficients = retained;
                            config.execution.threads = ThreadConfig::from_requested(threads, false).unwrap();
                            config.absorb.lsmr.threads = config.execution.threads;
                            config.absorb.lsmr.preconditioner = preconditioner;
                            let workspace = crate::fe::cycle_direct_lsmr_workspace_bytes(&descriptor, rhs, config.absorb).unwrap();
                            let parent = 4096 + if retained { data.fit_retained_fe_parent_bytes().unwrap() } else { 0 };
                            for short in [false, true] {
                                config.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
                                    execution_bytes: parent, usable_bytes: parent + held + workspace - u128::from(short),
                                });
                                let absorber = build_fit_fe_absorber_with_one_fe_reuse(&data, BorrowedMapWeights::Unit, config, true, true).unwrap();
                                assert_eq!(absorber.cycle_design.is_some(), !short);
                                let (_, peak, selected) = absorber.cycle_probe_cost.unwrap();
                                assert_eq!(selected, !short);
                                assert!(peak.bytes().unwrap() as u128 >= held);
                                if !short { assert_eq!(absorber.auto_direct_solver, Some(FeSolverKind::Lsmr)); }
                            }
                        }
                    }
                    for excluded in 0..4 {
                        let mut config = PreparedOlsFitConfig::default();
                        config.allow_final_fe_recovery = true;
                        config.execution.threads = ThreadConfig::from_requested(threads, false).unwrap();
                        match excluded {
                            0 => config.allow_final_fe_recovery = false,
                            1 => config.absorb.solver = FeSolverKind::Map,
                            2 => config.execution.mode = PreparedOlsExecutionMode::Low,
                            _ => config.absorb.map.max_iterations = crate::fe::OLS_SCHUR_MAX_ITERATIONS + crate::fe::OLS_SCHUR_PROBE_ITERATIONS,
                        }
                        let absorber = build_fit_fe_absorber_with_one_fe_reuse(&data, BorrowedMapWeights::Unit, config, true, true).unwrap();
                        assert!(absorber.cycle_design.is_none());
                        assert!(absorber.cycle_probe_cost.is_none());
                    }
                });
            }
        }
    }
