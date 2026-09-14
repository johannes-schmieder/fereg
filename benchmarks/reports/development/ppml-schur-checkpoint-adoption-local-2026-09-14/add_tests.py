import os
from pathlib import Path
p=Path(os.path.join(os.environ['REPOSITORY_ROOT'], 'crates/fereg-core/src/fe/borrowed_map.rs'));s=p.read_text();idx=s.index('    #[test]\n    fn ols_schur_probe_solve_budget_and_order_match_independent_map()')
s=s[:idx]+'''    #[test]
    fn checkpoint_schur_copy_respects_exact_transient_admission() {
        let values = DenseMatrix::from_columns(&[vec![1.0, -2.0], vec![3.0, 4.0]]).unwrap();
        let bytes = values.as_slice().len() as u128 * 8;
        assert_eq!(checkpoint_schur_copy(&values, bytes - 1).unwrap_err().code, ErrorCode::Allocation);
        let copy = checkpoint_schur_copy(&values, bytes).unwrap();
        assert_eq!(copy, values);
        assert_ne!(copy.as_slice().as_ptr(), values.as_slice().as_ptr());
        for rows in [16384, 122880, 983040] {
            for rhs in [16, 17, 51, 64] {
                for threads in [1, 4, 16] {
                    let estimate = estimate_fe_solver_memory(rows, rhs, &[rows / 4, 128], threads, Some(4)).unwrap();
                    assert!(estimate.lsmr_schwarz_bytes >= rows as u128 * rhs as u128 * 8);
                }
            }
        }
    }

    #[test]
    fn checkpoint_schur_reuses_probe_and_falls_back_on_untouched_inputs() {
        let rows = 16384;
        let first_codes = (0..rows).map(|r| (r % 257) as u32).collect::<Vec<_>>();
        let second_codes = (0..rows).map(|r| ((r * 17 + r / 257) % 127) as u32).collect::<Vec<_>>();
        let first_index = LevelIndex::from_codes(&first_codes, 257).unwrap();
        let second_index = LevelIndex::from_codes(&second_codes, 127).unwrap();
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, 257, &first_index).unwrap();
        let second = ValidatedBorrowedInterceptSource::new(&second_codes, 127, &second_index).unwrap();
        let design = || BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()]).unwrap();
        let columns = (0..17).map(|c| (0..rows).map(|r| {
            ((r * (c + 17) % 997) as f64 / 17.0).sin() + first_codes[r] as f64 * 0.001 + second_codes[r] as f64 * 0.01
        }).collect::<Vec<_>>()).collect::<Vec<_>>();
        let values = DenseMatrix::from_columns(&columns).unwrap();
        let original = values.clone();
        let supplied = (0..rows).map(|r| 1.0 + (r % 7) as f64).collect::<Vec<_>>();
        let scaled = supplied.iter().map(|w| w * 1024.0).collect::<Vec<_>>();
        let config = MapConfig { tolerance: 1e-12, ..config() };
        for threads in [1, 4, 16] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                let mut reusable = ReusableCategoricalMapAbsorber::new(design(), 64, threads).unwrap();
                reusable.enable_checkpoint_lsmr();
                let capacities = reusable.capacities();
                for raw in [BorrowedMapWeights::Unit, BorrowedMapWeights::Provided(&supplied), BorrowedMapWeights::Provided(&scaled)] {
                    let weights = validate_weights(raw, rows).unwrap();
                    let expected_probe = schur_ols_probe(&design(), raw, &values).unwrap();
                    let graph = reusable.two_fe_pcg.as_mut().unwrap();
                    for storage in [&mut graph.left_rhs, &mut graph.left_work, &mut graph.residual, &mut graph.direction, &mut graph.operator, &mut graph.solution] {
                        storage.fill(123.0);
                    }
                    for _ in 0..2 {
                        assert_eq!(schur_probe_reusing(&design(), weights, &values, graph, 64).unwrap(), expected_probe);
                    }
                    let expected = absorb_borrowed_intercept_map_certified(&design(), raw, values.clone(), MapConfig { tolerance: 1e-13, ..config }).unwrap();
                    let solved = reusable.absorb(raw, values.clone(), config).unwrap();
                    assert_eq!(solved.report.used_solver, FeSolverKind::Map);
                    assert_eq!(solved.report.requested_solver, FeSolverKind::Lsmr);
                    assert_eq!(solved.report.pilot_iterations, 8);
                    assert!(!solved.report.fallback_used);
                    assert_eq!(solved.report.checkpoint_retries, 0);
                    for (&a, &b) in solved.residualized().as_slice().iter().zip(expected.residualized().as_slice()) {
                        assert!((a - b).abs() < 2e-10, "{a} != {b}");
                    }
                    // A width-65 owner has no Schur graph and supplies the unchanged
                    // checkpoint LSMR result for exactly these 17 authoritative RHSs.
                    let mut baseline = ReusableCategoricalMapAbsorber::new(design(), 65, threads).unwrap();
                    baseline.enable_checkpoint_lsmr();
                    let original_lsmr = baseline.absorb(raw, values.clone(), config).unwrap();
                    for reject_certificate in [false, true] {
                        if reject_certificate { reusable.checkpoint_schur_certificate_failure = true; }
                        else { reusable.checkpoint_schur_iteration_cap = Some(1); }
                        let fallback = reusable.absorb(raw, values.clone(), config).unwrap();
                        assert_eq!(fallback.report.used_solver, FeSolverKind::Lsmr);
                        assert_eq!(fallback.residualized(), original_lsmr.residualized());
                        assert_eq!(fallback.report.checkpoint_retries, original_lsmr.report.checkpoint_retries + 1);
                        assert!(fallback.report.iterations > original_lsmr.report.iterations);
                        assert!(fallback.report.warnings.iter().any(|w| w.contains("untouched authoritative inputs")));
                    }
                    assert_eq!(values, original);
                    assert_eq!(reusable.capacities(), capacities);
                }
                for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
                    reusable.inject_checkpoint_lsmr_fatal_failure(code);
                    assert_eq!(reusable.absorb(BorrowedMapWeights::Unit, values.clone(), config).unwrap_err().code, code);
                    assert!(reusable.absorb(BorrowedMapWeights::Unit, values.clone(), config).unwrap().report.certified());
                    assert_eq!(reusable.capacities(), capacities);
                }
            });
        }
    }

    #[test]
    fn checkpoint_schur_weighted_cut_rejects_before_copying() {
        let rows = 16384;
        let left_codes = (0..rows).map(|r| (r / 4) as u32).collect::<Vec<_>>();
        let right_codes = (0..rows).map(|r| (match r % 4 { 0 => r / 4 % 128, 1 => (r / 4 + 1) % 128, _ => (r * 37 + r / 512 * 19) % 128 }) as u32).collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, rows / 4).unwrap();
        let right_index = LevelIndex::from_codes(&right_codes, 128).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, rows / 4, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, 128, &right_index).unwrap();
        let design = BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        let values = DenseMatrix::from_columns(&(0..17).map(|c| (0..rows).map(|r| ((r * (c + 17) % 997) as f64 / 17.0).sin() + (right_codes[r] as f64 * 0.049).sin()).collect::<Vec<_>>()).collect::<Vec<_>>()).unwrap();
        let weights = (0..rows).map(|r| if r % 4 < 2 { 1.0 } else { 1e-10 }).collect::<Vec<_>>();
        let mut reusable = ReusableCategoricalMapAbsorber::new(design, 17, 4).unwrap();
        reusable.enable_checkpoint_lsmr();
        let capacities = reusable.capacities();
        let (result, cost) = reusable.try_checkpoint_schur(validate_weights(BorrowedMapWeights::Provided(&weights), rows).unwrap(), &values, config()).unwrap();
        assert!(result.is_none());
        assert!(cost.contraction > OLS_SCHUR_MAX_CONTRACTION);
        assert_eq!(cost.iterations, 0);
        assert_eq!(cost.solve_seconds, 0.0);
        assert!(!cost.rejected);
        assert_eq!(reusable.capacities(), capacities);
        for width in [1, 15] {
            let narrow = DenseMatrix::zeros(rows, width).unwrap();
            let (result, cost) = reusable.try_checkpoint_schur(validate_weights(BorrowedMapWeights::Unit, rows).unwrap(), &narrow, config()).unwrap();
            assert!(result.is_none());
            assert_eq!(cost.probe_iterations, 0);
        }
    }

'''+s[idx:];p.write_text(s)
