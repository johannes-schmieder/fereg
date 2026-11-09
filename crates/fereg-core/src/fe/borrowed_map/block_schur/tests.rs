use super::*;
use crate::fe::ValidatedBorrowedInterceptSource;

/// Right-hand sides per lockstep batch (the Gram covers at most this many).
const BATCH: usize = 8;

/// Seeded codes with every level present and repeated row combinations.
fn codes(rows: usize, levels: &[usize], seed: u64) -> Vec<Vec<u32>> {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };
    levels
        .iter()
        .enumerate()
        .map(|(effect, &count)| {
            (0..rows)
                .map(|row| {
                    if row < count {
                        row as u32
                    } else if effect == 0 {
                        // A panel-like eliminated effect with repeated rows.
                        (row / 3 % count) as u32
                    } else {
                        (next() % count) as u32
                    }
                })
                .collect()
        })
        .collect()
}

fn values(rows: usize, columns: usize, seed: u64) -> DenseMatrix {
    let mut state = seed;
    let data = (0..rows * columns)
        .map(|_| {
            state = state
                .wrapping_mul(2_862_933_555_777_941_757)
                .wrapping_add(3_037_000_493);
            ((state >> 11) as f64 / (1_u64 << 53) as f64) * 4.0 - 2.0
        })
        .collect();
    DenseMatrix::from_row_major(rows, columns, data).unwrap()
}

fn with_design(
    rows: usize,
    levels: &[usize],
    seed: u64,
    action: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &[Vec<u32>]),
) {
    with_codes(rows, levels, codes(rows, levels, seed), action);
}

fn with_codes(
    rows: usize,
    levels: &[usize],
    codes: Vec<Vec<u32>>,
    action: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &[Vec<u32>]),
) {
    let indices: Vec<_> = codes
        .iter()
        .zip(levels)
        .map(|(c, &l)| LevelIndex::from_codes(c, l).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(levels)
        .zip(&indices)
        .map(|((c, &l), i)| ValidatedBorrowedInterceptSource::new(c, l, i).unwrap())
        .collect();
    let design =
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap();
    action(&design, &codes);
}

#[test]
fn elimination_matches_converged_map_and_returns_reproducing_coefficients() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let rows = 900;
    for levels in [
        vec![120, 17],
        vec![150, 23, 6],
        vec![150, 23, 6, 11],
        vec![150, 23, 6, 11, 4],
    ] {
        with_design(
            rows,
            &levels,
            20261107 + levels.len() as u64,
            |design, codes| {
                let weights: Vec<f64> = (0..rows).map(|r| 0.25 + (r * 7 % 11) as f64).collect();
                for weighted in [false, true] {
                    let borrowed = if weighted {
                        BorrowedMapWeights::Provided(&weights)
                    } else {
                        BorrowedMapWeights::Unit
                    };
                    for columns in [1, 3, 11] {
                        let input = values(rows, columns, 7 + columns as u64);
                        let map = MapConfig {
                            tolerance: 1e-10,
                            ..MapConfig::default()
                        };
                        let Outcome::Done(result) = Cache::default()
                            .project(
                                design,
                                borrowed,
                                input.clone(),
                                map,
                                None,
                                Request {
                                    retain_coefficients: true,
                                    coefficient_accuracy: weighted,
                                },
                            )
                            .unwrap()
                        else {
                            panic!("elimination declined: levels={levels:?} weighted={weighted}");
                        };
                        assert!(result.report().certified());
                        let oracle = absorb_borrowed_intercept_map(
                            design,
                            borrowed,
                            input.clone(),
                            MapConfig {
                                tolerance: 1e-13,
                                max_iterations: 200_000,
                                ..MapConfig::default()
                            },
                            false,
                            false,
                        )
                        .unwrap();
                        let (residual, _, coefficients) = result.into_full_parts();
                        let coefficients = coefficients.expect("retained coefficients");
                        for row in 0..rows {
                            for column in 0..columns {
                                let r = residual[(row, column)];
                                assert!(
                                (r - oracle.residualized()[(row, column)]).abs() < 1e-7,
                                "levels={levels:?} weighted={weighted} row={row} column={column}"
                            );
                                // v - D a reproduces the returned residual.
                                let mut fit = 0.0;
                                for (effect, effect_codes) in codes.iter().enumerate() {
                                    let base = column * coefficients.coefficients_per_rhs
                                        + coefficients.effect_offsets[effect];
                                    fit += coefficients.values[base + effect_codes[row] as usize];
                                }
                                assert!((input[(row, column)] - fit - r).abs() < 1e-9);
                            }
                        }
                    }
                }
            },
        );
    }
}

#[test]
fn residual_gram_matches_the_formed_residual() {
    let rows = 700;
    for levels in [vec![120, 17], vec![140, 23, 6], vec![140, 23, 6, 11, 4]] {
        with_design(rows, &levels, 41 + levels.len() as u64, |design, _| {
            let weights: Vec<f64> = (0..rows).map(|r| 0.5 + (r * 5 % 13) as f64).collect();
            for weighted in [false, true] {
                let borrowed = if weighted {
                    BorrowedMapWeights::Provided(&weights)
                } else {
                    BorrowedMapWeights::Unit
                };
                for columns in [1, 6, 9] {
                    let input = values(rows, columns, 3 + columns as u64);
                    let mut cache = Cache::default();
                    let request = Request {
                        retain_coefficients: false,
                        coefficient_accuracy: weighted,
                    };
                    // Not requested: no Gram; the norms are attached.
                    let Outcome::Done(mut first) = cache
                        .project(
                            design,
                            borrowed,
                            input.clone(),
                            MapConfig::default(),
                            None,
                            request,
                        )
                        .unwrap()
                    else {
                        panic!("declined");
                    };
                    assert!(cache.residual_gram().is_none());
                    let attached = first.take_residual_norms();
                    assert_eq!(attached.len(), columns);
                    assert_eq!(attached, cache.residual_norms());
                    // The retained workspaces repeat the projection bit for
                    // bit; detached norms stay in the cache.
                    cache.request_residual_gram(true);
                    cache.detach_residual_norms(true);
                    let Outcome::Done(mut result) = cache
                        .project(
                            design,
                            borrowed,
                            input.clone(),
                            MapConfig::default(),
                            None,
                            request,
                        )
                        .unwrap()
                    else {
                        panic!("declined");
                    };
                    assert!(result.take_residual_norms().is_empty());
                    assert_eq!(attached, cache.residual_norms());
                    let bits = |m: &DenseMatrix| -> Vec<u64> {
                        m.as_slice().iter().map(|v| v.to_bits()).collect()
                    };
                    assert_eq!(bits(first.residualized()), bits(result.residualized()));
                    let gram = cache.residual_gram();
                    if columns > BATCH {
                        assert!(gram.is_none());
                        continue;
                    }
                    let gram = gram.expect("requested Gram");
                    assert_eq!(gram.columns(), columns);
                    let residual = result.residualized();
                    for a in 0..columns {
                        for b in 0..columns {
                            let direct: f64 = (0..rows)
                                .map(|row| {
                                    let w = if weighted { weights[row] } else { 1.0 };
                                    w * residual[(row, a)] * residual[(row, b)]
                                })
                                .sum();
                            let value = gram.values()[a * columns + b];
                            assert_eq!(value, gram.values()[b * columns + a]);
                            assert!(
                                (value - direct).abs() <= 1e-12 * direct.abs().max(1.0),
                                "levels={levels:?} weighted={weighted} ({a},{b}): {value} vs {direct}"
                            );
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn declines_return_the_untouched_input() {
    with_design(400, &[80, 9, 5], 3, |design, _| {
        let input = values(400, 4, 11);
        // A one-iteration ceiling cannot reach the tolerance.
        let short = MapConfig {
            tolerance: 1e-14,
            max_iterations: 1,
            ..MapConfig::default()
        };
        match Cache::default()
            .project(
                design,
                BorrowedMapWeights::Unit,
                input.clone(),
                short,
                None,
                Request::default(),
            )
            .unwrap()
        {
            Outcome::Declined(values) => assert_eq!(values, input),
            Outcome::Done(_) => panic!("one iteration unexpectedly certified"),
        }
        // A budget below the admission bound declines before any work.
        let tiny = FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1,
        };
        match Cache::default()
            .project(
                design,
                BorrowedMapWeights::Unit,
                input.clone(),
                MapConfig::default(),
                Some(tiny),
                Request::default(),
            )
            .unwrap()
        {
            Outcome::Declined(values) => assert_eq!(values, input),
            Outcome::Done(_) => panic!("refused budget admitted"),
        }
    });
}

#[test]
fn designs_outside_two_to_five_effects_decline() {
    for count in [1_usize, 6] {
        let levels: Vec<usize> = (0..count).map(|e| 3 + e).collect();
        with_design(60, &levels, 5, |design, _| {
            let input = values(60, 2, 9);
            match Cache::default()
                .project(
                    design,
                    BorrowedMapWeights::Unit,
                    input.clone(),
                    MapConfig::default(),
                    None,
                    Request::default(),
                )
                .unwrap()
            {
                Outcome::Declined(values) => assert_eq!(values, input),
                Outcome::Done(_) => panic!("{count} effects unexpectedly projected"),
            }
        });
    }
}

/// A later batch that declines after the first batch's residuals were formed
/// in place (here: a column beyond the elimination's plain-sum range) hands
/// the matrix to the owner's established two-FE route, which retains
/// coefficients. They must still reproduce every residual from the original
/// columns.
#[test]
fn retained_coefficients_survive_a_late_decline() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let rows = 900;
    let levels = [150_usize, 23];
    let codes = codes(rows, &levels, 77);
    let indices: Vec<_> = codes
        .iter()
        .zip(&levels)
        .map(|(c, &l)| LevelIndex::from_codes(c, l).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(&levels)
        .zip(&indices)
        .map(|((c, &l), i)| ValidatedBorrowedInterceptSource::new(c, l, i).unwrap())
        .collect();
    let design =
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap();
    let columns = 10;
    let mut owner = ReusableCategoricalMapAbsorber::new(design, columns, 1).unwrap();
    owner.retain_coefficients = true;
    let mut input = values(rows, columns, 5);
    let huge = 1e149;
    for row in 0..rows {
        input[(row, columns - 1)] *= huge;
    }
    let config = MapConfig {
        tolerance: 1e-10,
        max_iterations: 100_000,
        ..MapConfig::default()
    };
    let result = owner
        .absorb_ppml(BorrowedMapWeights::Unit, input.clone(), config)
        .unwrap();
    let (residual, _, coefficients) = result.into_full_parts();
    let coefficients = coefficients.expect("retained coefficients");
    for column in 0..columns {
        let scale = if column + 1 == columns { huge } else { 1.0 };
        let base = column * coefficients.coefficients_per_rhs;
        let worst = (0..rows)
            .map(|row| {
                let fit: f64 = codes
                    .iter()
                    .enumerate()
                    .map(|(effect, effect_codes)| {
                        coefficients.values[base
                            + coefficients.effect_offsets[effect]
                            + effect_codes[row] as usize]
                    })
                    .sum();
                (input[(row, column)] - fit - residual[(row, column)]).abs() / scale
            })
            .fold(0.0_f64, f64::max);
        assert!(
            worst < 1e-8,
            "column {column}: v - D a differs from r by {worst:e}"
        );
    }
}

/// Movers on a chain of `firms` firms, two rows at each of two neighbouring
/// firms: the firms are eliminated and the Jacobi recurrence on the movers'
/// block stalls; block-Jacobi with the exact movers' block solves it.
fn chain(firms: usize) -> Vec<Vec<u32>> {
    let mut codes = vec![Vec::new(), Vec::new()];
    for mover in 0..firms - 1 {
        for t in 0..4 {
            codes[0].push(mover as u32);
            codes[1].push((mover + t / 2) as u32);
        }
    }
    codes
}

fn certified<'a>(
    cache: &mut Cache<'a>,
    design: &BorrowedInterceptFeDesign<'a>,
    input: &DenseMatrix,
    budget: Option<FeSolverMemoryBudget>,
) -> CertifiedBorrowedMap {
    let map = MapConfig {
        tolerance: 1e-10,
        ..MapConfig::default()
    };
    match cache
        .project(
            design,
            BorrowedMapWeights::Unit,
            input.clone(),
            map,
            budget,
            Request::default(),
        )
        .unwrap()
    {
        Outcome::Done(result) => {
            assert!(result.report().certified());
            result
        }
        Outcome::Declined(_) => panic!("elimination declined"),
    }
}

fn bits(m: &DenseMatrix) -> Vec<u64> {
    m.as_slice().iter().map(|v| v.to_bits()).collect()
}

fn preconditioner(cache: &Cache<'_>) -> fedemean::Preconditioner {
    cache
        .projector
        .as_ref()
        .expect("bound projector")
        .settings()
        .choice
        .preconditioner
}

/// The first cache of a hard design escalates and sets the shared hint; a
/// later cache of the design starts on the blocks: its route reports the
/// blocks without an escalation, in fewer iterations, and agrees.
#[test]
fn a_set_hint_starts_later_caches_on_the_blocks() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let firms = 256;
    let codes = chain(firms);
    let rows = codes[0].len();
    with_codes(rows, &[firms - 1, firms], codes, |design, _| {
        let input = values(rows, 3, 17);
        let hint = AtomicBool::new(false);
        let mut first = Cache::default();
        first.set_hint(Some(&hint));
        let escalated = certified(&mut first, design, &input, None);
        assert!(first.escalated && hint.load(Ordering::Relaxed));
        // The reported route names the escalation and the movers' exact
        // block; the eliminated firms have none.
        let block = crate::fe::FePreconditioner::Block([
            Some(crate::fe::FeLocalSolve::Dense),
            None,
            None,
            None,
            None,
        ]);
        let route = escalated.report().route();
        assert_eq!(
            (route.reduction, route.method, route.preconditioner),
            (
                crate::fe::FeReduction::Eliminate,
                crate::fe::FeMethod::Cg,
                block
            )
        );
        assert!(route.escalations >= 1);
        assert_eq!(preconditioner(&first), fedemean::Preconditioner::Auto);
        // The escalated projector is kept: fedemean starts it on the blocks.
        certified(&mut first, design, &input, None);
        assert_eq!(preconditioner(&first), fedemean::Preconditioner::Auto);

        let mut second = Cache::default();
        second.set_hint(Some(&hint));
        let hinted = certified(&mut second, design, &input, None);
        assert!(!second.escalated);
        let route = hinted.report().route();
        assert_eq!((route.preconditioner, route.escalations), (block, 0));
        assert!(matches!(
            preconditioner(&second),
            fedemean::Preconditioner::BlockJacobi(_)
        ));
        assert!(
            hinted.report().iterations < escalated.report().iterations,
            "{} vs {}",
            hinted.report().iterations,
            escalated.report().iterations
        );
        for (a, b) in hinted
            .residualized()
            .as_slice()
            .iter()
            .zip(escalated.residualized().as_slice())
        {
            assert!((a - b).abs() < 1e-7, "{a} vs {b}");
        }
        // The same projector's route: blocks, no escalation, same bits.
        let projector = second.projector.as_mut().expect("bound projector");
        let mut data = input.clone();
        let columns = fedemean::ColumnsMut::new(data.as_mut_slice(), rows, 3).unwrap();
        let request = fedemean::Request::new(1e-10, MapConfig::default().max_iterations);
        let fedemean::Outcome::Projected(report) = projector
            .project(
                fedemean::Weights::Unit,
                columns,
                None,
                &request,
                &fedemean::Control::default(),
            )
            .unwrap()
        else {
            panic!("hinted projection declined");
        };
        assert!(matches!(
            report.route.preconditioner,
            fedemean::PreconditionerUsed::Block(_)
        ));
        assert_eq!((report.route.escalations, report.facts.escalations), (0, 0));
        assert_eq!(bits(&data), bits(hinted.residualized()));
    });
}

/// Ordinary designs never escalate: a hint stays unset and the projection is
/// bit for bit the unhinted one. A set hint whose blocks exceed the memory
/// limit reruns the call on the automatic route, with the same bits.
#[test]
fn ordinary_designs_keep_their_bits_with_a_hint() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let rows = 900;
    with_design(rows, &[150, 23, 6], 11, |design, _| {
        let input = values(rows, 3, 5);
        let mut plain = Cache::default();
        let reference = certified(&mut plain, design, &input, None);
        let hint = AtomicBool::new(false);
        let mut hinted = Cache::default();
        hinted.set_hint(Some(&hint));
        let result = certified(&mut hinted, design, &input, None);
        assert_eq!(bits(result.residualized()), bits(reference.residualized()));
        assert!(!hint.load(Ordering::Relaxed) && !hinted.escalated);
        let route = result.report().route();
        assert_eq!(
            route,
            crate::fe::FeRoute::new(
                crate::fe::FeReduction::Eliminate,
                crate::fe::FeMethod::Cg,
                crate::fe::FePreconditioner::Jacobi,
            )
        );
        assert_eq!(preconditioner(&hinted), fedemean::Preconditioner::Auto);

        // The automatic route's own admission bound admits it, but not the
        // explicit blocks on top.
        let automatic = plain
            .projector
            .as_ref()
            .and_then(|p| p.admission_bytes(3))
            .expect("built structure");
        let budget = FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: automatic,
        };
        let hint = AtomicBool::new(true);
        let mut refused = Cache::default();
        refused.set_hint(Some(&hint));
        let result = certified(&mut refused, design, &input, Some(budget));
        assert!(refused.blocks_refused);
        assert_eq!(preconditioner(&refused), fedemean::Preconditioner::Auto);
        assert_eq!(bits(result.residualized()), bits(reference.residualized()));
    });
}
