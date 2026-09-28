    #[test]
    fn registered_nonretaining_cg_known_within_probe() {
        let rows = 48;
        let left = (0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let right = (0..rows).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
        let li = LevelIndex::from_codes(&left, 4).unwrap();
        let ri = LevelIndex::from_codes(&right, 3).unwrap();
        let ls = ValidatedBorrowedInterceptSource::new(&left, 4, &li).unwrap();
        let rs = ValidatedBorrowedInterceptSource::new(&right, 3, &ri).unwrap();
        let design = BorrowedInterceptFeDesign::new(rows, vec![ls.effect(), rs.effect()]).unwrap();
        // Four repetitions of each FE tuple have identical weights and signs
        // -,+,-,+. Thus noise is exactly W-orthogonal to the explicit FE design.
        let noise = (0..rows).map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 }).collect::<Vec<_>>();
        let mut failed = 0;
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                for (frame, scale) in [None, Some(1e-100), Some(1.0), Some(1e100)].into_iter().enumerate() {
                    let raw = scale.map(|scale| (0..rows).map(|row| scale * (1 + row % 12) as f64).collect::<Vec<_>>());
                    let weights = raw.as_deref().map_or(BorrowedMapWeights::Unit, BorrowedMapWeights::Provided);
                    for width in [1, 4, 64, 65, 129] {
                        let columns = (0..width).map(|rhs| {
                            let multiplier = 1.0 + rhs as f64 / 17.0;
                            (0..rows).map(|row| multiplier * noise[row]
                                + 2e-9 * left[row] as f64 + (1.0 + rhs as f64 / 5.0) * 1e-9 * right[row] as f64).collect()
                        }).collect::<Vec<_>>();
                        let input = DenseMatrix::from_columns(&columns).unwrap();
                        let mut options = config();
                        options.tolerance = 1e-12;
                        let result = absorb_borrowed_intercept_cg_certified(&design, weights, input, options);
                        match result {
                            Ok(result) => {
                                let mut difference = 0.0_f64;
                                for rhs in 0..width {
                                    let multiplier = 1.0 + rhs as f64 / 17.0;
                                    for row in 0..rows {
                                        difference = difference.max((result.residualized()[(row, rhs)] / multiplier - noise[row]).abs());
                                    }
                                }
                                let passed = result.report().certified() && difference < 1e-11;
                                failed += usize::from(!passed);
                                println!("FEREG_CG_PROBE|direct|{threads}|{frame}|{width}|{passed}|{difference:e}|{}|{:?}",
                                    result.report().iterations, result.report().solver_workspace_peak.bytes());
                            }
                            Err(error) => {
                                failed += 1;
                                println!("FEREG_CG_PROBE|direct|{threads}|{frame}|{width}|false|ERROR|{:?}|{}", error.code, error.message);
                            }
                        }
                    }
                }
            });
        }
        assert_eq!(failed, 0, "independent known-within direct CG failures");
    }
