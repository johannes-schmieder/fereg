use super::reduced::apply_incidence_schur;
use super::*;
use crate::metadata::*;
use crate::ols::{
    prepare_model, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
    OlsRequestV1, PreparedOlsData,
};

pub(super) fn prepared(y: &[f64], x: &[Vec<f64>], g: &[f64], h: &[f64]) -> PreparedOlsData {
    prepared_effects(y, x, &[g, h])
}

fn prepared_effects(y: &[f64], x: &[Vec<f64>], effects: &[&[f64]]) -> PreparedOlsData {
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    let mut numeric = Vec::new();
    let mut bindings = vec![
        OlsInputBinding {
            stata_var_position: 1,
            role: OlsInputRole::Candidate,
        },
        OlsInputBinding {
            stata_var_position: 2,
            role: OlsInputRole::Outcome,
        },
    ];
    for (j, values) in x
        .iter()
        .map(Vec::as_slice)
        .chain(effects.iter().copied())
        .enumerate()
    {
        let id = (j + 1) as u32;
        let source = SourceId::new(id).unwrap();
        sources.push(SourceEntry {
            id: source,
            canonical_name: format!("v{id}"),
        });
        bindings.push(OlsInputBinding {
            stata_var_position: id + 2,
            role: OlsInputRole::NumericSource(source),
        });
        numeric.push((
            source,
            LoadedNumericColumn {
                values: values.to_vec(),
                missing: vec![false; y.len()],
            },
        ));
        if j < x.len() {
            terms.push(ExpandedTerm {
                id: TermId::new(id).unwrap(),
                order: TermOrder::new(id).unwrap(),
                coefficient_slot: CoefficientSlot::new(id).unwrap(),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: format!("c.v{id}"),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).unwrap(),
                    source,
                    kind: ComponentKind::Continuous,
                    level_state: LevelState::Absent,
                    level_value: 0.,
                    base: MetadataFlag::False,
                    omitted: MetadataFlag::Unavailable,
                    raw_operator: "c".into(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                }],
            });
        }
    }
    prepare_model(
        &OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms: (0..effects.len() as u32)
                .map(|j| AbsorbedTerm {
                    id: AbsorbTermId::new(j + 1).unwrap(),
                    intercept: true,
                    categorical_sources: vec![SourceId::new(x.len() as u32 + j + 1).unwrap()],
                    slope_sources: Vec::new(),
                    label: format!("fe{j}"),
                    saved_name: String::new(),
                })
                .collect(),
            bindings,
            include_intercept: true,
            keep_singletons: true,
        },
        &[],
        &LoadedOlsInput {
            candidate: vec![true; y.len()].into(),
            outcome: LoadedNumericColumn {
                values: y.to_vec(),
                missing: vec![false; y.len()],
            },
            numeric_sources: numeric,
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        false,
    )
    .unwrap()
}

#[test]
fn implicit_canonical_frame_matches_weighted_contrasts_when_dense_storage_refuses() {
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for disconnected in [false, true] {
                    for frame in [0, 1] {
                        for mass in [1e-100, 1., 1e100] {
                            let (mut g, mut h, mut w) = (Vec::new(), Vec::new(), Vec::new());
                            let mut values = vec![Vec::new(); 7];
                            let mut truth = vec![Vec::new(); 7];
                            for a in 0..1024 {
                                for e in 0..8 {
                                    for repeat in 0..2 {
                                        let b = if disconnected {
                                            (a * 7 + e * 61) % 256 + (a / 512) * 256
                                        } else {
                                            (a * 7 + e * 61) % 512
                                        };
                                        g.push(a as f64);
                                        h.push(b as f64);
                                        w.push(
                                            mass * (0.2
                                                + ((a * 13 + e * 7 + frame * 3) % 17) as f64),
                                        );
                                        for j in 0..7 {
                                            let scale = if j == 1 {
                                                1e20
                                            } else if j == 2 {
                                                1e-20
                                            } else {
                                                1.
                                            };
                                            let r = if j == 3 {
                                                0.
                                            } else {
                                                (2. * repeat as f64 - 1.)
                                                    * ((a * (j + 1) + e * 3) % 11) as f64
                                            };
                                            truth[j].push(r * scale);
                                            values[j].push(
                                                (r + 0.01 * a as f64 - 0.02 * b as f64) * scale,
                                            );
                                        }
                                    }
                                }
                            }
                            let p = prepared(&values[0], &values[1..], &g, &h);
                            let config = MapConfig {
                                tolerance: 1e-12,
                                ..MapConfig::default()
                            };
                            let divisor = w.iter().copied().fold(0_f64, f64::max);
                            let allowance = 16 * g.len() as u128 * 6;
                            let cols = [0, 1, 2, 3, 4, 5];
                            let mut product = try_build(
                                &p,
                                &cols,
                                &values[0],
                                BorrowedMapWeights::Provided(&w),
                                divisor,
                                config,
                                allowance,
                                0,
                                &mut Vec::new(),
                            )
                            .unwrap()
                            .expect("implicit frame fits and certifies");
                            assert!(product.implicit && product.report.certified());
                            let mut dense = DenseMatrix::zeros(g.len(), 6).unwrap();
                            product.write_dense(&mut dense).unwrap();
                            let mut file =
                                PreparedOlsScratch::create_row_major(g.len(), 6).unwrap();
                            product.write_controls(&mut file).unwrap();
                            let mut row = [0.; 6];
                            for i in 0..g.len() {
                                file.read_linear_shared(i * 6, &mut row).unwrap();
                                assert!((product.response[i] - truth[0][i]).abs() < 1e-8);
                                for j in 1..7 {
                                    assert_eq!(row[j - 1], dense[(i, j - 1)]);
                                    let scale = if j == 1 {
                                        1e20
                                    } else if j == 2 {
                                        1e-20
                                    } else {
                                        1.
                                    };
                                    assert!((row[j - 1] - truth[j][i]).abs() / scale < 1e-8);
                                }
                            }
                            for j in 0..7 {
                                for k in 0..7 {
                                    let expected: f64 = (0..g.len())
                                        .map(|i| {
                                            (w[i] / divisor)
                                                * (truth[j][i] / nonzero(product.scales[j]))
                                                * (truth[k][i] / nonzero(product.scales[k]))
                                        })
                                        .sum();
                                    assert!(
                                        (product.moments.gram[j * 7 + k] - expected).abs() < 1e-10
                                    );
                                }
                            }
                            let bytes = product.minimum_workspace_bytes;
                            assert!(try_build(
                                &p,
                                &cols,
                                &values[0],
                                BorrowedMapWeights::Provided(&w),
                                divisor,
                                config,
                                bytes - 1,
                                0,
                                &mut Vec::new()
                            )
                            .unwrap()
                            .is_none());
                            assert!(try_build(
                                &p,
                                &cols,
                                &values[0],
                                BorrowedMapWeights::Provided(&w),
                                divisor,
                                config,
                                bytes,
                                0,
                                &mut Vec::new()
                            )
                            .unwrap()
                            .is_some());
                            assert!(try_build(
                                &p,
                                &cols,
                                &values[0],
                                BorrowedMapWeights::Provided(&w),
                                divisor,
                                MapConfig {
                                    max_iterations: 1,
                                    ..config
                                },
                                allowance,
                                0,
                                &mut Vec::new()
                            )
                            .unwrap()
                            .is_none());
                            // A genuinely affordable dense frame retains its old organization.
                            let dense = try_build(
                                &p,
                                &cols,
                                &values[0],
                                BorrowedMapWeights::Provided(&w),
                                divisor,
                                config,
                                u128::MAX,
                                0,
                                &mut Vec::new(),
                            )
                            .unwrap()
                            .unwrap();
                            assert!(!dense.implicit);
                        }
                    }
                }
            });
    }
}

#[test]
fn incidence_schur_matches_independent_dense_weighted_elimination() {
    let a = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let b = [0, 1, 1, 1, 2, 0, 3, 4, 4, 4, 5, 3];
    let groups = crate::fe::LevelIndex::from_codes(&a, 4).unwrap();
    let free = [usize::MAX, 0, 1, usize::MAX, 2, 3];
    let truth = [0.3, -0.7, 1.1, 0.2];
    for scale in [1e-200, 1., 1e200] {
        for frame in [0, 1] {
            let weights: Vec<f64> = (0..a.len())
                .map(|i| scale * (0.2 + ((i * 7 + frame * 3) % 11) as f64))
                .collect();
            let mut cross = [[0.; 6]; 4];
            let mut left_mass = [0.; 4];
            let mut right_mass = [0.; 6];
            for i in 0..a.len() {
                cross[a[i] as usize][b[i] as usize] += weights[i];
                left_mass[a[i] as usize] += weights[i];
                right_mass[b[i] as usize] += weights[i];
            }
            let mut schur = [[0_f64; 6]; 6];
            for h in 0..6 {
                schur[h][h] = right_mass[h];
                for j in 0..6 {
                    for g in 0..4 {
                        schur[h][j] -= (cross[g][h] / left_mass[g]) * cross[g][j];
                    }
                }
            }
            let roots: Vec<_> = (0..6).map(|h| schur[h][h].sqrt()).collect();
            let mut dense = [0.; 16];
            for h in 0..6 {
                if free[h] == usize::MAX {
                    continue;
                }
                for j in 0..6 {
                    if free[j] != usize::MAX {
                        dense[free[h] * 4 + free[j]] = (schur[h][j] / roots[h]) / roots[j];
                    }
                }
            }
            let mut incidence = Vec::new();
            let mut ids = Vec::new();
            let mut rows = Vec::new();
            // Keep the legacy matrix indices and numerical traversal order explicit.
            #[allow(clippy::needless_range_loop)]
            for g in 0..4 {
                let start = ids.len();
                for &i in groups.members(g) {
                    let h = b[i] as usize;
                    if free[h] == usize::MAX {
                        continue;
                    }
                    ids.push(free[h]);
                    incidence.push((weights[i] / left_mass[g].sqrt()) / roots[h]);
                }
                rows.push(IncidenceRow {
                    start,
                    end: ids.len(),
                    component: g / 2,
                });
            }
            let mut diagonal = [0.; 4];
            for h in 0..6 {
                if free[h] != usize::MAX {
                    diagonal[free[h]] = (right_mass[h] / roots[h]) / roots[h];
                }
            }
            let apply = |x: &[f64], y: &mut [f64]| {
                apply_incidence_schur(&incidence, &diagonal, &ids, &rows, x, y)
            };
            let rhs: Vec<_> = dense
                .chunks(4)
                .map(|row| row.iter().zip(truth).map(|(a, b)| a * b).sum::<f64>())
                .collect();
            let mut actual = [0.; 4];
            apply(&truth, &mut actual);
            for i in 0..4 {
                assert!((actual[i] - rhs[i]).abs() < 3e-15);
            }
            let mut solution = rhs.clone();
            let count =
                solve_reduced_cg_with_product(&mut solution, &mut [0.; 12], 4, 1e-13, apply)
                    .unwrap();
            assert!(count <= 4);
            for i in 0..4 {
                assert!((solution[i] - truth[i]).abs() < 1e-12);
            }
            assert!(solve_reduced_cg_with_product(
                &mut rhs.clone(),
                &mut [0.; 12],
                0,
                1e-13,
                apply
            )
            .is_err());
        }
    }
}

#[test]
fn reduced_cg_matches_dense_spd_products_and_refuses_unfinished_solves() {
    for n in [2_usize, 7, 31] {
        let mut matrix = vec![0.; n * n];
        for i in 0..n {
            matrix[i * n + i] = 2.;
            if i + 1 < n {
                matrix[i * n + i + 1] = -0.7;
                matrix[(i + 1) * n + i] = -0.7;
            }
        }
        for scale in [1e-200, 1., 1e200] {
            let truth: Vec<_> = (0..n)
                .map(|i| ((i as f64 + 0.2) * 0.37).sin() * scale)
                .collect();
            let rhs: Vec<_> = (0..n)
                .map(|i| (0..n).map(|j| matrix[i * n + j] * truth[j]).sum())
                .collect();
            let mut answer = rhs.clone();
            assert!(
                solve_reduced_cg(&matrix, &mut answer, &mut vec![0.; 3 * n], n, 1e-13).unwrap()
                    <= n
            );
            for (actual, expected) in answer.iter().zip(&truth) {
                assert!((actual / scale - expected / scale).abs() < 1e-12);
            }
            assert!(
                solve_reduced_cg(&matrix, &mut rhs.clone(), &mut vec![0.; 3 * n], 0, 1e-13)
                    .is_err()
            );
        }
    }
}

#[test]
fn reduced_cg_refuses_null_negative_and_nonfinite_curvature() {
    for matrix in [
        [1., -1., -1., 1.],
        [-1., 0., 0., -1.],
        [f64::NAN, 0., 0., 1.],
        [f64::INFINITY, 0., 0., 1.],
    ] {
        let mut rhs = [1., 1.];
        assert!(solve_reduced_cg(&matrix, &mut rhs, &mut [0.; 6], 2, 1e-13).is_err());
    }
}

#[test]
fn iterative_products_preserve_independent_weighted_nonzero_residuals() {
    for threads in [1, 4] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        team.install(|| {
            let mut g = Vec::new();
            let mut h = Vec::new();
            let mut x = Vec::new();
            let mut y = Vec::new();
            let mut q = Vec::new();
            let mut truth = Vec::new();
            for a in 0..640 {
                for slot in 0..6 {
                    let b = (a + 13 * slot) % 80;
                    let value = ((a * 6 + slot) as f64 * 0.23).sin();
                    let error = 0.03 * ((a * 6 + slot) as f64 * 0.71).cos();
                    for repeat in 0..2 {
                        let weight = if repeat == 0 { 0.7 } else { 2.3 };
                        let residual = if repeat == 0 {
                            error / weight
                        } else {
                            -error / weight
                        };
                        g.push(a as f64);
                        h.push(b as f64);
                        x.push(value);
                        q.push(weight);
                        truth.push(residual);
                        y.push(0.37 * value + 0.002 * a as f64 - 0.01 * b as f64 + residual);
                    }
                }
            }
            let prepared = prepared(&y, &[x.clone(), vec![0.; y.len()]], &g, &h);
            let design = prepared
                .fit_borrowed_intercept_map_design()
                .unwrap()
                .unwrap();
            let plan = Plan::new(&design, 3, 0, threads, 0).unwrap();
            assert!((plan.q as u128).pow(3) / 3 > plan.factor_work_budget);
            for frame in [0, 1] {
                let weights: Vec<_> = q
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        w * if frame == 0 {
                            1.
                        } else {
                            0.3 + (i / 2 % 13) as f64
                        }
                    })
                    .collect();
                let config = MapConfig {
                    tolerance: 1e-12,
                    ..MapConfig::default()
                };
                let products = try_build(
                    &prepared,
                    &[0, 1],
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    1.,
                    config,
                    8_000_000,
                    0,
                    &mut Vec::new(),
                )
                .unwrap()
                .expect("iterative product proposal certifies");
                assert!(products.report.iterations > 1 && products.report.certified());
                let minimum = products.minimum_workspace_bytes;
                let bounded = try_build(
                    &prepared,
                    &[0, 1],
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    1.,
                    config,
                    minimum,
                    0,
                    &mut Vec::new(),
                )
                .unwrap()
                .expect("complete Krylov phase fits its reported minimum");
                assert_eq!(products.response, bounded.response);
                assert_eq!(products.moments.gram, bounded.moments.gram);
                let mut panel = vec![42.; 1024];
                let capacity = panel.capacity();
                assert!(try_build(
                    &prepared,
                    &[0, 1],
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    1.,
                    config,
                    minimum - 1,
                    0,
                    &mut panel
                )
                .unwrap()
                .is_none());
                assert_eq!(panel.capacity(), capacity);
                assert!(panel.iter().all(|&x| x == 42.));
                let beta = products.moments.gram[3] / products.moments.gram[4] * products.scales[0]
                    / products.scales[1];
                assert!((beta - 0.37).abs() < 1e-10);
                // Keep the legacy matrix indices and numerical traversal order explicit.
                #[allow(clippy::needless_range_loop)]
                for i in 0..y.len() {
                    let residual =
                        products.residual(i, 0).unwrap() - beta * products.residual(i, 1).unwrap();
                    assert!((residual - truth[i]).abs() < 1e-10);
                }
                let stopped = MapConfig {
                    max_iterations: 1,
                    ..config
                };
                assert!(try_build(
                    &prepared,
                    &[0, 1],
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    1.,
                    stopped,
                    8_000_000,
                    0,
                    &mut Vec::new()
                )
                .unwrap()
                .is_none());
            }
        });
    }
}

#[test]
fn canonical_budget_credits_only_released_capacity() {
    let mut budget = Budget::new(64);
    let first = budget.reserve(6, 0_f64).unwrap();
    let second = budget.reserve(2, 0_u64).unwrap();
    assert_eq!(budget.remaining, 0);
    assert!(budget.set_limit(63).is_err());
    assert_eq!(budget.limit, 64);
    budget.set_limit(80).unwrap();
    assert_eq!(budget.remaining, 16);
    budget.set_limit(64).unwrap();
    assert!(budget.reserve(1, 0_u8).is_err());
    budget.release(first).unwrap();
    let third = budget.reserve(12, 0_u32).unwrap();
    assert_eq!(budget.peak, 64);
    assert_eq!(budget.remaining, 0);
    budget.release(second).unwrap();
    budget.release(third).unwrap();
    assert_eq!(budget.remaining, budget.limit);
}

#[test]
fn canonical_phase_peak_admits_wide_weighted_oracle_within_the_existing_pool() {
    use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
    use crate::ols::prepared_fit::LinearSolveFrame;
    use crate::ols::{PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig};
    let n = 6144;
    let k = 72;
    let workers = 4;
    let allowance = (n * 10 * 16) as u128;
    let g: Vec<_> = (0..n).map(|i| (i / 6) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| ((i / 6 + i % 6) % 24) as f64).collect();
    let x: Vec<Vec<_>> = (0..k)
        .map(|j| {
            (0..n)
                .map(|i| {
                    (i as f64 * (j as f64 + 1.618)).sin()
                        + 0.3 * ((i + j) as f64 / (j + 1) as f64).cos()
                })
                .collect()
        })
        .collect();
    let truth: Vec<_> = (0..k).map(|j| 0.005 * (j + 1) as f64).collect();
    let y: Vec<_> = (0..n)
        .map(|i| {
            0.01 * g[i] - 0.02 * h[i] + x.iter().zip(&truth).map(|(x, b)| x[i] * b).sum::<f64>()
        })
        .collect();
    let prepared = prepared(&y, &x, &g, &h);
    let design = prepared
        .fit_borrowed_intercept_map_design()
        .unwrap()
        .unwrap();
    let plan = Plan::new(&design, k + 1, 0, workers, 0).unwrap();
    let (p, q, w) = (plan.p, plan.q, plan.width);
    let previous_sum = 8
        * (2 * n
            + p
            + 3 * q
            + 2 * q * q
            + p * w
            + 3 * q * w
            + 3 * w
            + w * w
            + workers * plan.stride)
        + 4 * q * size_of::<usize>()
        + 32 * w
        + 2048;
    assert!(previous_sum as u128 > allowance);
    assert!(plan.bytes <= allowance);
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Map;
    config.execution.mode = PreparedOlsExecutionMode::Low;
    config.execution.poolsize = 10;
    config.execution.threads = crate::options::ThreadConfig::from_requested(workers, true).unwrap();
    let team = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .unwrap();
    for frame in [0, 1] {
        let weights: Vec<_> = (0..n)
            .map(|i| {
                if frame == 0 {
                    1.
                } else {
                    0.2 + (i % 17) as f64 / 7.
                }
            })
            .collect();
        let divisor = weights.iter().copied().fold(0_f64, f64::max);
        let scale = PreparedOlsBreadWeightScale {
            weight_kind: None,
            solver_weight_divisor: divisor,
            reference_weight_multiplier: divisor,
            reference_weight_sum: weights.iter().sum(),
        };
        team.install(|| {
            let products = try_build(
                &prepared,
                &(0..k).collect::<Vec<_>>(),
                &y,
                BorrowedMapWeights::Provided(&weights),
                divisor,
                config.absorb.map,
                allowance,
                0,
                &mut Vec::new(),
            )
            .unwrap()
            .unwrap();
            assert!(products.report.certified());
            assert!(try_build(
                &prepared,
                &(0..k).collect::<Vec<_>>(),
                &y,
                BorrowedMapWeights::Provided(&weights),
                divisor,
                config.absorb.map,
                products.minimum_workspace_bytes - 1,
                0,
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
        });
        let frame = LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Provided(&weights), scale)
            .unwrap();
        let mut session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            LinearSessionGenerations::default(),
            prepared.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        let fit = session.solve_fresh().unwrap();
        assert_eq!(fit.frame().absorption_report().iterations, 1);
        for (coefficient, expected) in fit.coefficients().zip(&truth) {
            assert!((coefficient.value.unwrap() - expected).abs() < 1e-10);
        }
        assert!(fit.frame().residuals().iter().all(|r| r.abs() < 1e-10));
    }
}

#[test]
fn streamed_canonical_groups_admit_wide_nonzero_weighted_contrasts() {
    weighted_streaming_case(8640, 100, 36, 1, &[1, 8], false);
}

#[test]
fn tiled_canonical_output_matches_independent_wls_under_changing_weights() {
    weighted_streaming_case(14400, 144, 32, 1, &[1, 8], true);
}

#[test]
fn column_owned_rhs_admits_disconnected_wls_below_the_old_reduction_peak() {
    weighted_streaming_case(16320, 100, 124, 4, &[4], true);
}

fn weighted_streaming_case(
    n: usize,
    k: usize,
    q_per_component: usize,
    components: usize,
    worker_counts: &[usize],
    tile_output: bool,
) {
    let q = q_per_component * components;
    use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
    use crate::ols::prepared_fit::LinearSolveFrame;
    use crate::ols::{PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig};
    let g: Vec<_> = (0..n).map(|i| (i / 6) as f64).collect();
    let h: Vec<_> = (0..n)
        .map(|i| {
            (((i / 6 / components + 11 * ((i % 6) / 2)) % q_per_component)
                + (i / 6 % components) * q_per_component) as f64
        })
        .collect();
    // Both rows in each pair share all FE and control coordinates. Their
    // reciprocal-weight errors are orthogonal to the complete explicit
    // design, giving a nonzero-residual WLS oracle without either solver.
    let x: Vec<Vec<_>> = (0..k)
        .map(|j| {
            (0..n)
                .map(|i| {
                    let row = (i / 2) as f64;
                    (row * (j as f64 + 1.618)).sin()
                        + 0.3 * ((row + j as f64) / (j + 1) as f64).cos()
                })
                .collect()
        })
        .collect();
    let truth: Vec<_> = (0..k).map(|j| 0.002 * (j + 1) as f64).collect();
    let predictor: Vec<_> = (0..n)
        .map(|i| {
            0.02 * g[i] - 0.03 * h[i] + x.iter().zip(&truth).map(|(x, b)| x[i] * b).sum::<f64>()
        })
        .collect();
    let prepared = prepared(&predictor, &x, &g, &h);
    let allowance = (n * 10 * 16) as u128;
    for &threads in worker_counts {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        team.install(|| {
            let design = prepared
                .fit_borrowed_intercept_map_design()
                .unwrap()
                .unwrap();
            let plan = Plan::new(&design, k + 1, 0, threads, 0).unwrap();
            let w = k + 1;
            if components > 1 {
                let matrix = components * q_per_component * q_per_component;
                let factor = components * (q_per_component - 1) * (q_per_component - 1);
                let old_solve = 8
                    * (n + plan.p + 2 * q + factor + q * w + 2 * w + threads * (q * w + 3 * w))
                    + 6 * q * size_of::<usize>()
                    + 32 * w
                    + 2048;
                assert!(old_solve as u128 > allowance);
                assert!(plan.minimum_storage_bytes(matrix, factor).unwrap() <= allowance);
            }
            let previous_certificate = 8
                * (n + n / 6
                    + q
                    + n / 6 * w
                    + q * w
                    + 3 * w
                    + w * w
                    + threads * (w * w + q * w + 5 * w))
                + 32 * w
                + 2048;
            assert!(previous_certificate as u128 > allowance);
            assert!(plan.bytes <= allowance);
            let mut config = PreparedOlsFitConfig::default();
            config.absorb.solver = FeSolverKind::Map;
            config.absorb.map.tolerance = 1e-12;
            config.execution.mode = PreparedOlsExecutionMode::Low;
            config.execution.poolsize = 10;
            config.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            for frame in [0, 1] {
                let ratio = 2. + frame as f64;
                let weights: Vec<_> = (0..n)
                    .map(|i| {
                        (0.3 + ((i / 2 + 7 * frame) % 31) as f64 * 0.02)
                            * if i % 2 == 0 { 1. } else { ratio }
                    })
                    .collect();
                let errors: Vec<_> = (0..n)
                    .map(|i| {
                        0.01 * ((i / 2) % 7 + 1) as f64 * if i % 2 == 0 { 1. } else { -1. / ratio }
                    })
                    .collect();
                let y: Vec<_> = predictor.iter().zip(&errors).map(|(a, b)| a + b).collect();
                let divisor = weights.iter().copied().fold(0_f64, f64::max);
                let scale = PreparedOlsBreadWeightScale {
                    weight_kind: None,
                    solver_weight_divisor: divisor,
                    reference_weight_multiplier: divisor,
                    reference_weight_sum: weights.iter().sum(),
                };
                let mut old_panel = vec![42.; n * 10];
                let old_capacity = old_panel.capacity();
                let topology = plan.topology_storage_bytes().unwrap();
                // The earlier preflight refused this live-panel frame by
                // charging reductions that do not exist during topology.
                if threads == 8 {
                    assert!(plan.bytes > allowance - old_capacity as u128 * 8);
                }
                assert!(topology <= allowance - old_capacity as u128 * 8);
                assert!(try_build(
                    &prepared,
                    &(0..k).collect::<Vec<_>>(),
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    divisor,
                    config.absorb.map,
                    old_capacity as u128 * 8 + topology - 1,
                    0,
                    &mut old_panel,
                )
                .unwrap()
                .is_none());
                assert_eq!(old_panel.capacity(), old_capacity);
                assert!(old_panel.iter().all(|&x| x == 42.));
                let mut products = try_build(
                    &prepared,
                    &(0..k).collect::<Vec<_>>(),
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    divisor,
                    config.absorb.map,
                    allowance,
                    0,
                    &mut old_panel,
                )
                .unwrap()
                .unwrap();
                assert!(products.report.certified());
                let selected_bytes = products.report.solver_workspace_peak.bytes().unwrap() as u128;
                if selected_bytes > allowance - old_capacity as u128 * 8 {
                    assert_eq!(old_panel.capacity(), 0);
                } else {
                    assert_eq!(old_panel.capacity(), old_capacity);
                    assert!(old_panel.iter().all(|&x| x == 42.));
                }
                assert!(!products.retained_centers);
                assert_eq!(products.tiled.is_some(), tile_output);
                let bytes = products.minimum_workspace_bytes;
                assert!(
                    products.report.solver_workspace_peak.bytes().unwrap() as u128 <= allowance
                );
                let cached = try_build(
                    &prepared,
                    &(0..k).collect::<Vec<_>>(),
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    divisor,
                    config.absorb.map,
                    u128::MAX,
                    0,
                    &mut Vec::new(),
                )
                .unwrap()
                .unwrap();
                assert!(cached.retained_centers);
                let mut dense = DenseMatrix::zeros(n, k).unwrap();
                products.write_dense(&mut dense).unwrap();
                for i in 0..n {
                    for j in 0..k {
                        assert_eq!(dense[(i, j)], cached.residual(i, j + 1).unwrap());
                    }
                }
                let mut misaligned = DenseMatrix::zeros(n - 1, k).unwrap();
                assert_eq!(
                    products.write_dense(&mut misaligned).unwrap_err().code,
                    ErrorCode::InvalidArgument
                );
                assert!(misaligned.as_slice().iter().all(|&x| x == 0.));
                if tile_output {
                    let mut scratch = PreparedOlsScratch::create_row_major(n, k).unwrap();
                    products.write_controls(&mut scratch).unwrap();
                    let mut row = vec![0.; k];
                    let mut reader = scratch.shared_reader();
                    for i in 0..n {
                        reader.read_linear(i * k, &mut row).unwrap();
                        for (j, &actual) in row.iter().enumerate() {
                            assert_eq!(actual, cached.residual(i, j + 1).unwrap());
                        }
                    }
                    assert!(products.left.is_empty());
                } else {
                    assert_eq!(products.left, cached.left);
                }
                assert_eq!(products.right, cached.right);
                assert_eq!(products.response, cached.response);
                assert_eq!(products.moments.gram, cached.moments.gram);
                assert_eq!(products.means, cached.means);
                drop(cached);
                let mut refused_panel = vec![42.; n * 10];
                let refused_capacity = refused_panel.capacity();
                assert!(try_build(
                    &prepared,
                    &(0..k).collect::<Vec<_>>(),
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    divisor,
                    config.absorb.map,
                    bytes - 1,
                    0,
                    &mut refused_panel
                )
                .unwrap()
                .is_none());
                assert_eq!(refused_panel.capacity(), refused_capacity);
                assert!(refused_panel.iter().all(|&x| x == 42.));
                // The bank rebuilt after certification must reproduce the
                // very residuals whose direct products supplied the Gram.
                for j in [0, 1, k] {
                    let norm = nonzero(products.scales[j]);
                    let direct: f64 = (0..n)
                        .map(|i| {
                            let value = products.residual(i, j).unwrap() / norm;
                            (weights[i] / divisor) * value * value
                        })
                        .sum();
                    assert!((direct - products.moments.gram[j * (k + 1) + j]).abs() < 1e-11);
                }
                drop(products);
                let frame =
                    LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Provided(&weights), scale)
                        .unwrap();
                let mut session = CertifiedLinearSession::new_with_fe_design(
                    &prepared,
                    frame,
                    config,
                    LinearSessionGenerations::default(),
                    prepared.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                let result = session.solve_fresh().unwrap();
                assert_eq!(result.frame().absorption_report().iterations, 1);
                assert!(result.frame().score_certificate().passed);
                for (actual, expected) in result.coefficients().take(k).zip(&truth) {
                    assert!((actual.value.unwrap() - expected).abs() < 1e-9);
                }
                for (actual, expected) in result.frame().residuals().iter().zip(&errors) {
                    assert!((actual - expected).abs() < 1e-9);
                }
            }
        });
    }
}

#[test]
fn canonical_work_history_admits_changed_weights_and_reclaims_only_after_preflight() {
    use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
    use crate::ols::prepared_fit::LinearSolveFrame;
    use crate::ols::{PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig};
    let n = 19200;
    // Four-member groups keep the complete implicit owner above this
    // allowance; Q158 keeps dense direct storage affordable only after
    // observed work permits eliminating its extra iterative workspace.
    let g: Vec<_> = (0..n).map(|i| (i / 4) as f64).collect();
    let h: Vec<_> = (0..n)
        .map(|i| ((i / 4 + 13 * (i % 4)) % 158) as f64)
        .collect();
    let x: Vec<Vec<_>> = (0..2)
        .map(|j| {
            (0..n)
                .map(|i| (i as f64 * (0.17 + j as f64 * 0.09)).sin())
                .collect()
        })
        .collect();
    let y: Vec<_> = (0..n)
        .map(|i| 0.25 * x[0][i] - 0.4 * x[1][i] + 0.02 * g[i] - 0.03 * h[i])
        .collect();
    let prepared = prepared(&y, &x, &g, &h);
    let allowance = (n * 2 * 16) as u128;
    for threads in [1, 4] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        team.install(|| {
            let mut config = PreparedOlsFitConfig::default();
            config.absorb.solver = FeSolverKind::Map;
            config.absorb.map.tolerance = 1e-12;
            config.execution.mode = PreparedOlsExecutionMode::Low;
            config.execution.poolsize = 2;
            config.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            let mut scale = PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: 1.,
                reference_weight_multiplier: 1.,
                reference_weight_sum: n as f64,
            };
            let frame = LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Unit, scale).unwrap();
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                frame,
                config,
                LinearSessionGenerations::default(),
                prepared.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            let cold = session.solve_fresh().unwrap();
            assert!(cold.frame().absorption_report().iterations > 1);
            for (coefficient, expected) in cold.coefficients().take(2).zip([0.25, -0.4]) {
                assert!((coefficient.value.unwrap() - expected).abs() < 1e-9);
            }
            let observed = session.canonical_projection_work_hint_for_test();
            assert!(observed > 0);
            let mut old_panel = vec![42.; n * 2];
            let old_capacity = old_panel.capacity();
            for (budget, work) in [(allowance, 0), (allowance, u128::MAX), (1, observed)] {
                assert!(try_build(
                    &prepared,
                    &[0, 1],
                    &y,
                    BorrowedMapWeights::Unit,
                    1.,
                    config.absorb.map,
                    budget,
                    work,
                    &mut old_panel
                )
                .unwrap()
                .is_none());
                assert_eq!(old_panel.capacity(), old_capacity);
                assert!(old_panel.iter().all(|&x| x == 42.));
            }
            let product = try_build(
                &prepared,
                &[0, 1],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                config.absorb.map,
                allowance,
                observed,
                &mut old_panel,
            )
            .unwrap()
            .unwrap();
            assert!(product.report.certified());
            assert_eq!(old_panel.capacity(), 0);
            assert!(product.report.solver_workspace_peak.bytes().unwrap() as u128 > allowance / 2);
            drop(product);
            let changed: Vec<_> = (0..n).map(|i| 0.3 + (i % 31) as f64 * 0.02).collect();
            scale.solver_weight_divisor = changed.iter().copied().fold(0_f64, f64::max);
            scale.reference_weight_multiplier = scale.solver_weight_divisor;
            scale.reference_weight_sum = changed.iter().sum();
            for commit in [false, true] {
                session
                    .refresh_weights(
                        BorrowedMapWeights::Provided(&changed),
                        scale,
                        1 + u64::from(commit),
                    )
                    .unwrap();
                let candidate = session.strict_candidate().unwrap();
                let fit = candidate.candidate();
                assert_eq!(fit.frame().absorption_report().iterations, 1);
                assert!(fit.frame().score_certificate().passed);
                for (coefficient, expected) in fit.coefficients().take(2).zip([0.25, -0.4]) {
                    assert!((coefficient.value.unwrap() - expected).abs() < 1e-9);
                }
                assert!(fit.frame().residuals().iter().all(|r| r.abs() < 1e-9));
                if commit {
                    candidate.commit();
                }
            }
            session.refresh_basis(2).unwrap();
            assert_eq!(session.canonical_projection_work_hint_for_test(), 0);
        });
    }
}

#[test]
fn canonical_products_session_keeps_rank_and_strict_rollback_across_weights() {
    use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
    use crate::ols::prepared_fit::LinearSolveFrame;
    use crate::ols::{PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig};
    let n = 4096;
    let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| ((i / 9) % 6) as f64).collect();
    for duplicate in [false, true] {
        let mut x: Vec<Vec<_>> = (0..8)
            .map(|j| {
                (0..n)
                    .map(|i| {
                        (i as f64 / (13 + 7 * j) as f64).sin()
                            + (i % (7 + j)) as f64 / (5 + j) as f64
                    })
                    .collect()
            })
            .collect();
        if duplicate {
            x[7] = x[2].clone();
        }
        let y: Vec<_> = (0..n)
            .map(|i| {
                0.2 * g[i] - 0.1 * h[i]
                    + x.iter()
                        .enumerate()
                        .map(|(j, x)| 0.03 * (j + 1) as f64 * x[i])
                        .sum::<f64>()
            })
            .collect();
        let prepared = prepared(&y, &x, &g, &h);
        for (threads, mode, admitted) in [
            (1, PreparedOlsExecutionMode::Low, true),
            (4, PreparedOlsExecutionMode::Low, true),
            (1, PreparedOlsExecutionMode::Fast, true),
            (4, PreparedOlsExecutionMode::Fast, true),
            (4, PreparedOlsExecutionMode::Fast, false),
        ] {
            let mut config = PreparedOlsFitConfig::default();
            config.absorb.solver = FeSolverKind::Map;
            config.execution.mode = mode;
            config.execution.poolsize = 8;
            config.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            let mut scale = PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: 1.,
                reference_weight_multiplier: 1.,
                reference_weight_sum: n as f64,
            };
            let frame = LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Unit, scale).unwrap();
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                frame,
                config,
                LinearSessionGenerations::default(),
                prepared.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            if mode == PreparedOlsExecutionMode::Fast {
                session
                    .enable_fast_canonical_products(if admitted { 16 * n as u128 * 8 } else { 1 })
                    .unwrap();
            }
            let original = session.solve_fresh().unwrap();
            // A direct certified solve reports one solve, proving that
            // this test reaches the fused handoff rather than its fallback.
            if admitted {
                assert_eq!(original.frame().absorption_report().iterations, 1);
            }
            assert!(original.frame().residuals().iter().all(|r| r.abs() < 1e-10));
            let statuses: Vec<_> = original.coefficients().map(|c| c.status).collect();
            let changed: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
            scale.solver_weight_divisor = changed.iter().copied().fold(0_f64, f64::max);
            scale.reference_weight_multiplier = scale.solver_weight_divisor;
            scale.reference_weight_sum = changed.iter().sum();
            let next_y: Vec<_> = y.iter().zip(&x[0]).map(|(y, x)| y + 0.4 * x).collect();
            session.refresh_response(&next_y, 1).unwrap();
            session
                .refresh_weights(BorrowedMapWeights::Provided(&changed), scale, 1)
                .unwrap();
            // A rank failure occurs after the direct writer changed the
            // physical dense bank. Retrying must use the matching response
            // anchor and never publish the failed candidate.
            session.inject_fresh_rank_failure(ErrorCode::Interrupted);
            assert!(session.strict_candidate().is_err());
            // A dropped speculative candidate cannot publish its basis.
            {
                let candidate = session.strict_candidate().unwrap();
                assert!(candidate
                    .candidate()
                    .frame()
                    .residuals()
                    .iter()
                    .all(|r| r.abs() < 1e-10));
            }
            let candidate = session.strict_candidate().unwrap();
            assert_eq!(
                candidate
                    .candidate()
                    .coefficients()
                    .map(|c| c.status)
                    .collect::<Vec<_>>(),
                statuses
            );
            candidate.commit();
            assert!(session.output().unwrap().frame().score_certificate().passed);
            // Same-weight response-only solving bypasses direct products;
            // this protects the handoff back to the existing projector.
            session.refresh_response(&y, 2).unwrap();
            let recovered = session.solve_fresh().unwrap();
            assert!(recovered.frame().score_certificate().passed);
            assert!(recovered.frame().residuals().iter().all(|r| r.abs() < 1e-9));
            assert_eq!(
                recovered
                    .coefficients()
                    .map(|c| c.status)
                    .collect::<Vec<_>>(),
                statuses
            );
        }
    }
}

#[test]
fn canonical_three_products_session_keeps_rank_and_strict_rollback_across_weights() {
    use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
    use crate::ols::prepared_fit::LinearSolveFrame;
    use crate::ols::{PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig};
    let n = 4096;
    let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| ((i / 9) % 6) as f64).collect();
    let z: Vec<_> = (0..n).map(|i| ((i / 31) % 5) as f64).collect();
    for duplicate in [false, true] {
        let mut x: Vec<Vec<_>> = (0..8)
            .map(|j| {
                (0..n)
                    .map(|i| {
                        (i as f64 / (13 + 7 * j) as f64).sin()
                            + (i % (7 + j)) as f64 / (5 + j) as f64
                    })
                    .collect()
            })
            .collect();
        if duplicate {
            x[7] = x[2].clone();
        }
        let y: Vec<_> = (0..n)
            .map(|i| {
                0.2 * g[i] - 0.1 * h[i]
                    + 0.07 * z[i]
                    + x.iter()
                        .enumerate()
                        .map(|(j, x)| 0.03 * (j + 1) as f64 * x[i])
                        .sum::<f64>()
            })
            .collect();
        let prepared = prepared_effects(&y, &x, &[&g, &h, &z]);
        for (threads, mode, admitted) in [
            (1, PreparedOlsExecutionMode::Low, true),
            (4, PreparedOlsExecutionMode::Low, true),
            (1, PreparedOlsExecutionMode::Fast, true),
            (4, PreparedOlsExecutionMode::Fast, true),
            (4, PreparedOlsExecutionMode::Fast, false),
        ] {
            let mut config = PreparedOlsFitConfig::default();
            config.absorb.solver = FeSolverKind::Map;
            config.execution.mode = mode;
            config.execution.poolsize = 8;
            config.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            let mut scale = PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: 1.,
                reference_weight_multiplier: 1.,
                reference_weight_sum: n as f64,
            };
            let frame = LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Unit, scale).unwrap();
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                frame,
                config,
                LinearSessionGenerations::default(),
                prepared.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            if mode == PreparedOlsExecutionMode::Fast {
                session
                    .enable_fast_canonical_products(if admitted { 16 * n as u128 * 8 } else { 1 })
                    .unwrap();
            }
            let original = session.solve_fresh().unwrap();
            // Independent product tests establish the direct route. This
            // test checks publication authority across speculative writes.
            assert!(original.frame().absorption_report().certified());
            assert!(original.frame().residuals().iter().all(|r| r.abs() < 1e-10));
            let statuses: Vec<_> = original.coefficients().map(|c| c.status).collect();
            let changed: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
            scale.solver_weight_divisor = changed.iter().copied().fold(0_f64, f64::max);
            scale.reference_weight_multiplier = scale.solver_weight_divisor;
            scale.reference_weight_sum = changed.iter().sum();
            let next_y: Vec<_> = y.iter().zip(&x[0]).map(|(y, x)| y + 0.4 * x).collect();
            session.refresh_response(&next_y, 1).unwrap();
            session
                .refresh_weights(BorrowedMapWeights::Provided(&changed), scale, 1)
                .unwrap();
            // A rank failure occurs after the direct writer changed the
            // physical dense bank. Retrying must use the matching response
            // anchor and never publish the failed candidate.
            session.inject_fresh_rank_failure(ErrorCode::Interrupted);
            assert!(session.strict_candidate().is_err());
            // A dropped speculative candidate cannot publish its basis.
            {
                let candidate = session.strict_candidate().unwrap();
                assert!(candidate
                    .candidate()
                    .frame()
                    .residuals()
                    .iter()
                    .all(|r| r.abs() < 1e-10));
            }
            let candidate = session.strict_candidate().unwrap();
            assert_eq!(
                candidate
                    .candidate()
                    .coefficients()
                    .map(|c| c.status)
                    .collect::<Vec<_>>(),
                statuses
            );
            candidate.commit();
            assert!(session.output().unwrap().frame().score_certificate().passed);
            // Same-weight response-only solving bypasses direct products;
            // this protects the handoff back to the existing projector.
            session.refresh_response(&y, 2).unwrap();
            let recovered = session.solve_fresh().unwrap();
            assert!(recovered.frame().score_certificate().passed);
            assert!(recovered.frame().residuals().iter().all(|r| r.abs() < 1e-9));
            assert_eq!(
                recovered
                    .coefficients()
                    .map(|c| c.status)
                    .collect::<Vec<_>>(),
                statuses
            );
        }
    }
}

#[test]
fn canonical_products_match_independent_weighted_contrasts_and_direct_gram() {
    for threads in [1, 4, 8] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        team.install(|| {
            for mass in [1., 1e-80, 1e80] {
                for shift in [0., 0.7] {
                    let wa: Vec<_> = (0..6).map(|a| 1. + a as f64 + shift).collect();
                    let wb: Vec<_> = (0..5).map(|b| 0.5 + b as f64 + shift).collect();
                    let ma = wa
                        .iter()
                        .enumerate()
                        .map(|(i, w)| i as f64 * w)
                        .sum::<f64>()
                        / wa.iter().sum::<f64>();
                    let mb = wb
                        .iter()
                        .enumerate()
                        .map(|(i, w)| i as f64 * w)
                        .sum::<f64>()
                        / wb.iter().sum::<f64>();
                    let mut g = Vec::new();
                    let mut h = Vec::new();
                    let mut weights = Vec::new();
                    let mut values = vec![Vec::new(); 4];
                    let mut truth = vec![Vec::new(); 4];
                    let scales = [1., 1e70, 1e-70, 1.];
                    for component in 0..2 {
                        // Keep the legacy matrix indices and numerical traversal order explicit.
                        #[allow(clippy::needless_range_loop)]
                        for a in 0..6 {
                            // Keep the legacy matrix indices and numerical traversal order explicit.
                            #[allow(clippy::needless_range_loop)]
                            for b in 0..5 {
                                for t in 0..3 {
                                    g.push((component * 6 + a) as f64);
                                    h.push((component * 5 + b) as f64);
                                    weights.push(
                                        wa[a]
                                            * wb[b]
                                            * if component == 0 { mass } else { 1. / mass },
                                    );
                                    let ab = (a as f64 - ma) * (b as f64 - mb);
                                    let noise = t as f64 - 1.;
                                    for (j, r) in [ab, noise, ab + noise * (b as f64 - mb), 0.]
                                        .into_iter()
                                        .enumerate()
                                    {
                                        truth[j].push(r * scales[j]);
                                        values[j].push(
                                            (r + 0.3 * a as f64 - 0.2 * b as f64
                                                + component as f64)
                                                * scales[j],
                                        );
                                    }
                                }
                            }
                        }
                    }
                    let p = prepared(&values[0], &values[1..], &g, &h);
                    let max = weights.iter().copied().fold(0_f64, f64::max);
                    let config = MapConfig {
                        tolerance: 1e-12,
                        ..MapConfig::default()
                    };
                    let mut result = try_build(
                        &p,
                        &[0, 1, 2],
                        &values[0],
                        BorrowedMapWeights::Provided(&weights),
                        max,
                        config,
                        u128::MAX,
                        0,
                        &mut Vec::new(),
                    )
                    .unwrap()
                    .expect("admitted exact contrasts");
                    assert!(result.report.certified());
                    let mut scratch = PreparedOlsScratch::create_row_major(g.len(), 3).unwrap();
                    result.write_controls(&mut scratch).unwrap();
                    let mut row = [0.; 3];
                    for i in 0..g.len() {
                        scratch.read_linear_shared(i * 3, &mut row).unwrap();
                        assert!((result.response[i] - truth[0][i]).abs() < 2e-12);
                        for j in 1..4 {
                            assert!((row[j - 1] - truth[j][i]).abs() / scales[j] < 2e-12);
                        }
                    }
                    for j in 0..4 {
                        for k in 0..4 {
                            let expected = (0..g.len())
                                .map(|i| {
                                    (weights[i] / max)
                                        * (truth[j][i] / nonzero(result.scales[j]))
                                        * (truth[k][i] / nonzero(result.scales[k]))
                                })
                                .sum::<f64>();
                            assert!((result.moments.gram[j * 4 + k] - expected).abs() < 2e-13);
                        }
                    }
                    assert!(result.norms[3] / result.scales[3] < 2e-14);
                    let bytes = result.minimum_workspace_bytes;
                    assert!(try_build(
                        &p,
                        &[0, 1, 2],
                        &values[0],
                        BorrowedMapWeights::Provided(&weights),
                        max,
                        config,
                        bytes - 1,
                        0,
                        &mut Vec::new()
                    )
                    .unwrap()
                    .is_none());
                    assert!(try_build(
                        &p,
                        &[0, 1, 2],
                        &values[0],
                        BorrowedMapWeights::Provided(&weights),
                        max,
                        config,
                        bytes,
                        0,
                        &mut Vec::new()
                    )
                    .unwrap()
                    .is_some());
                }
            }
        });
    }
}

#[test]
fn canonical_component_factors_admit_disconnected_weighted_contrasts() {
    for threads in [1, 4] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        team.install(|| {
            for transpose in [false, true] {
                for shift in [0., 0.7] {
                    let (mut g, mut h, mut weights) = (Vec::new(), Vec::new(), Vec::new());
                    let mut values = vec![Vec::new(); 3];
                    let mut truth = vec![Vec::new(); 3];
                    for component in 0..24 {
                        // Unequal component dimensions include one retained level
                        // (no factor) and noncontiguous labels across components.
                        let na = 4 + component % 3;
                        let nb = 1 + component % 4;
                        let wa: Vec<_> = (0..na).map(|a| 1. + a as f64 + shift).collect();
                        let wb: Vec<_> = (0..nb).map(|b| 0.5 + b as f64 + shift).collect();
                        let ma = wa
                            .iter()
                            .enumerate()
                            .map(|(a, w)| a as f64 * w)
                            .sum::<f64>()
                            / wa.iter().sum::<f64>();
                        let mb = wb
                            .iter()
                            .enumerate()
                            .map(|(b, w)| b as f64 * w)
                            .sum::<f64>()
                            / wb.iter().sum::<f64>();
                        // Keep the legacy matrix indices and numerical traversal order explicit.
                        #[allow(clippy::needless_range_loop)]
                        for a in 0..na {
                            // Keep the legacy matrix indices and numerical traversal order explicit.
                            #[allow(clippy::needless_range_loop)]
                            for b in 0..nb {
                                for t in 0..4 {
                                    g.push((a * 24 + component) as f64);
                                    h.push((b * 24 + component) as f64);
                                    weights.push(
                                        wa[a] * wb[b] * if component % 2 == 0 { 0.03 } else { 30. },
                                    );
                                    let ab = (a as f64 - ma) * (b as f64 - mb);
                                    for (j, r) in [ab, t as f64 - 1.5, ab + 0.3 * (t as f64 - 1.5)]
                                        .into_iter()
                                        .enumerate()
                                    {
                                        truth[j].push(r);
                                        values[j].push(
                                            r + 0.2 * a as f64 - 0.1 * b as f64 + component as f64,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    if transpose {
                        std::mem::swap(&mut g, &mut h);
                    }
                    let prepared = prepared(&values[0], &values[1..], &g, &h);
                    let design = prepared
                        .fit_borrowed_intercept_map_design()
                        .unwrap()
                        .unwrap();
                    let left = usize::from(
                        design.effects()[1].n_levels() > design.effects()[0].n_levels(),
                    );
                    let plan = Plan::new(&design, 3, left, threads, 0).unwrap();
                    assert!(
                        (plan.q as u128).pow(3) / 3 > plan.factor_work_budget,
                        "the global factor bound must reject this fixture"
                    );
                    let allowance = (g.len() * 2 * 16) as u128;
                    assert!(
                        plan.storage_bytes(plan.q * plan.q, plan.q * plan.q, false)
                            .unwrap()
                            > allowance
                    );
                    let divisor = weights.iter().copied().fold(0_f64, f64::max);
                    let result = try_build(
                        &prepared,
                        &[0, 1],
                        &values[0],
                        BorrowedMapWeights::Provided(&weights),
                        divisor,
                        MapConfig {
                            tolerance: 1e-12,
                            ..MapConfig::default()
                        },
                        allowance,
                        0,
                        &mut Vec::new(),
                    )
                    .unwrap()
                    .expect("component factors admitted");
                    assert!(result.report.certified());
                    // Keep the legacy matrix indices and numerical traversal order explicit.
                    #[allow(clippy::needless_range_loop)]
                    for j in 0..3 {
                        for i in 0..g.len() {
                            assert!((result.residual(i, j).unwrap() - truth[j][i]).abs() < 1e-11);
                        }
                    }
                    for j in 0..3 {
                        for k in 0..3 {
                            let expected = (0..g.len())
                                .map(|i| {
                                    weights[i] / divisor * truth[j][i] * truth[k][i]
                                        / result.scales[j]
                                        / result.scales[k]
                                })
                                .sum::<f64>();
                            assert!((result.moments.gram[j * 3 + k] - expected).abs() < 1e-12);
                        }
                    }
                    assert!(try_build(
                        &prepared,
                        &[0, 1],
                        &values[0],
                        BorrowedMapWeights::Provided(&weights),
                        divisor,
                        MapConfig::default(),
                        result.minimum_workspace_bytes - 1,
                        0,
                        &mut Vec::new()
                    )
                    .unwrap()
                    .is_none());
                }
            }
        });
    }
}

#[test]
fn canonical_products_refuse_work_and_invalid_frames_without_mutation() {
    let n = 240;
    let g: Vec<_> = (0..n).map(|i| (i % 12) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| ((i / 12) % 5) as f64).collect();
    let x: Vec<_> = (0..n).map(|i| (i as f64).sin()).collect();
    let y: Vec<_> = (0..n).map(|i| g[i] + h[i] + 1e-10 * x[i]).collect();
    let p = prepared(&y, &[x.clone(), vec![0.; n]], &g, &h);
    let team = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    team.install(|| {
        let result = try_build(
            &p,
            &[0, 1],
            &y,
            BorrowedMapWeights::Unit,
            1.,
            MapConfig::default(),
            u128::MAX,
            0,
            &mut Vec::new(),
        )
        .unwrap()
        .unwrap();
        assert!(result.moments.gram[0] >= 0. && result.moments.gram[0] < 1e-20);
        assert_eq!(result.scales[2], 0.);
        assert_eq!(result.norms[2], 0.);
        let mut bad = vec![1.; n];
        bad[4] = 0.;
        assert!(try_build(
            &p,
            &[0, 1],
            &y,
            BorrowedMapWeights::Provided(&bad),
            1.,
            MapConfig::default(),
            u128::MAX,
            0,
            &mut Vec::new()
        )
        .unwrap()
        .is_none());
        let mut invalid = y.clone();
        invalid[5] = f64::NAN;
        assert_eq!(
            try_build(
                &p,
                &[0, 1],
                &invalid,
                BorrowedMapWeights::Unit,
                1.,
                MapConfig::default(),
                u128::MAX,
                0,
                &mut Vec::new()
            )
            .err()
            .unwrap()
            .code,
            ErrorCode::InvalidData
        );
        let config = MapConfig {
            tolerance: 1e-30,
            ..MapConfig::default()
        };
        assert!(try_build(
            &p,
            &[0, 1],
            &y,
            BorrowedMapWeights::Unit,
            1.,
            config,
            u128::MAX,
            0,
            &mut Vec::new()
        )
        .unwrap()
        .is_none());
        let large: Vec<_> = (0..n).map(|i| (i % 60) as f64).collect();
        let other: Vec<_> = (0..n).map(|i| ((i / 4 + i) % 60) as f64).collect();
        // The old global-work refusal has multiple small components, so
        // the component engine now legitimately admits its same design.
        let disconnected = prepared(&y, &[x.clone()], &large, &other);
        assert!(try_build(
            &disconnected,
            &[0],
            &y,
            BorrowedMapWeights::Unit,
            1.,
            MapConfig::default(),
            u128::MAX,
            0,
            &mut Vec::new()
        )
        .unwrap()
        .is_some());
        // A connected cycle exceeds dense factor work but can now use a
        // certified iterative proposal. An unfinished proposal still refuses.
        let connected: Vec<_> = (0..n).map(|i| ((i % 60 + i / 60) % 60) as f64).collect();
        let wide = prepared(&y, &[x], &large, &connected);
        assert!(try_build(
            &wide,
            &[0],
            &y,
            BorrowedMapWeights::Unit,
            1.,
            MapConfig::default(),
            u128::MAX,
            0,
            &mut Vec::new()
        )
        .unwrap()
        .is_some_and(|products| products.report.certified() && products.report.iterations > 1));
        assert!(try_build(
            &wide,
            &[0],
            &y,
            BorrowedMapWeights::Unit,
            1.,
            MapConfig {
                max_iterations: 1,
                ..MapConfig::default()
            },
            u128::MAX,
            0,
            &mut Vec::new()
        )
        .unwrap()
        .is_none());
    });
}
#[test]
fn canonical_three_products_match_independent_weighted_contrasts() {
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for shift in [0., 0.7] {
                    for mass in [1., 1e-100, 1e100] {
                        for nested in [false, true] {
                            let wa: Vec<_> = (0..6).map(|i| 1. + i as f64 + shift).collect();
                            let wb: Vec<_> = (0..5).map(|i| 0.5 + i as f64 + shift).collect();
                            let wc: Vec<_> = (0..3).map(|i| 0.3 + i as f64 + shift).collect();
                            let mean = |v: &[f64]| {
                                v.iter().enumerate().map(|(i, w)| i as f64 * w).sum::<f64>()
                                    / v.iter().sum::<f64>()
                            };
                            let (ma, mb) = (mean(&wa), mean(&wb));
                            let (mut g, mut h, mut z, mut weights) =
                                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                            let mut values = vec![Vec::new(); 5];
                            let mut truth = vec![Vec::new(); 5];
                            let scales = [1., 1e30, 1e-30, 1., 1.];
                            // Keep the legacy matrix indices and numerical traversal order explicit.
                            #[allow(clippy::needless_range_loop)]
                            for a in 0..6 {
                                // Keep the legacy matrix indices and numerical traversal order explicit.
                                #[allow(clippy::needless_range_loop)]
                                for b in 0..5 {
                                    // Keep the legacy matrix indices and numerical traversal order explicit.
                                    #[allow(clippy::needless_range_loop)]
                                    for c in 0..3 {
                                        for t in 0..2 {
                                            g.push(a as f64);
                                            h.push(b as f64);
                                            z.push(if nested { (b % 3) as f64 } else { c as f64 });
                                            weights.push(wa[a] * wb[b] * wc[c] * mass);
                                            let ab = (a as f64 - ma) * (b as f64 - mb);
                                            let error = 2. * t as f64 - 1.;
                                            for (j, r) in [ab + error, ab, error, 0., ab]
                                                .into_iter()
                                                .enumerate()
                                            {
                                                truth[j].push(r * scales[j]);
                                                values[j].push(
                                                    (r + 0.2 * a as f64 - 0.1 * b as f64
                                                        + 0.3 * z[z.len() - 1])
                                                        * scales[j],
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            let p = prepared_effects(&values[0], &values[1..], &[&g, &h, &z]);
                            let divisor = weights.iter().copied().fold(0_f64, f64::max);
                            let cfg = MapConfig {
                                tolerance: 1e-12,
                                ..MapConfig::default()
                            };
                            let mut result = try_build(
                                &p,
                                &[0, 1, 2, 3],
                                &values[0],
                                BorrowedMapWeights::Provided(&weights),
                                divisor,
                                cfg,
                                u128::MAX,
                                0,
                                &mut Vec::new(),
                            )
                            .unwrap()
                            .expect("three-FE direct products admitted");
                            assert!(result.three.is_some() && result.report.certified());
                            let mut dense = DenseMatrix::zeros(g.len(), 4).unwrap();
                            result.write_dense(&mut dense).unwrap();
                            let mut scratch =
                                PreparedOlsScratch::create_row_major(g.len(), 4).unwrap();
                            result.write_controls(&mut scratch).unwrap();
                            let mut row = [0.; 4];
                            for i in 0..g.len() {
                                scratch.read_linear_shared(i * 4, &mut row).unwrap();
                                assert!((result.response[i] - truth[0][i]).abs() < 1e-10);
                                for j in 1..5 {
                                    assert_eq!(row[j - 1], dense[(i, j - 1)]);
                                    assert!((row[j - 1] - truth[j][i]).abs() / scales[j] < 1e-10);
                                }
                            }
                            for j in 0..5 {
                                for k in 0..5 {
                                    let expected: f64 = (0..g.len())
                                        .map(|i| {
                                            weights[i] / divisor
                                                * (truth[j][i] / nonzero(result.scales[j]))
                                                * (truth[k][i] / nonzero(result.scales[k]))
                                        })
                                        .sum();
                                    assert!(
                                        (result.moments.gram[j * 5 + k] - expected).abs() < 1e-11
                                    );
                                }
                            }
                            let bytes = result.minimum_workspace_bytes;
                            assert!(try_build(
                                &p,
                                &[0, 1, 2, 3],
                                &values[0],
                                BorrowedMapWeights::Provided(&weights),
                                divisor,
                                cfg,
                                bytes - 1,
                                0,
                                &mut Vec::new()
                            )
                            .unwrap()
                            .is_none());
                            assert!(try_build(
                                &p,
                                &[0, 1, 2, 3],
                                &values[0],
                                BorrowedMapWeights::Provided(&weights),
                                divisor,
                                cfg,
                                bytes,
                                0,
                                &mut Vec::new()
                            )
                            .unwrap()
                            .is_some());
                        }
                    }
                }
            });
    }
}

#[test]
fn canonical_three_products_preserve_disconnected_and_invalid_frame_fallback() {
    let n = 480;
    let g: Vec<_> = (0..n).map(|i| (i % 6) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i % 2 * 5 + (i / 6) % 5) as f64).collect();
    let z: Vec<_> = (0..n).map(|i| (i % 2 * 4 + (i / 30) % 4) as f64).collect();
    let x = vec![(0..n).map(|i| (i as f64).sin()).collect::<Vec<_>>()];
    let y = x[0].clone();
    let p = prepared_effects(&y, &x, &[&g, &h, &z]);
    let mut panel = vec![1.; 48];
    assert!(try_build(
        &p,
        &[0],
        &y,
        BorrowedMapWeights::Unit,
        1.,
        MapConfig::default(),
        u128::MAX,
        0,
        &mut panel
    )
    .unwrap()
    .is_none());
    assert_eq!(panel, vec![1.; 48]);
    let ch: Vec<_> = (0..n).map(|i| ((i / 6) % 5) as f64).collect();
    let cz: Vec<_> = (0..n).map(|i| ((i / 30) % 4) as f64).collect();
    let connected = prepared_effects(&y, &x, &[&g, &ch, &cz]);
    for weights in [vec![0.; n], vec![f64::NAN; n], vec![1.; n - 1]] {
        assert!(try_build(
            &connected,
            &[0],
            &y,
            BorrowedMapWeights::Provided(&weights),
            1.,
            MapConfig::default(),
            u128::MAX,
            0,
            &mut panel
        )
        .unwrap()
        .is_none());
    }
}

/// A 120-firm worker chain plus small firm components, with weights over
/// eight orders of magnitude: implicit solves at a strict target.
fn chain_with_small_components() -> (PreparedOlsData, Vec<f64>, Vec<Vec<f64>>) {
    let (mut g, mut h, mut w) = (Vec::new(), Vec::new(), Vec::new());
    let mut worker = 0;
    let mut link = |firms: &[usize], g: &mut Vec<f64>, h: &mut Vec<f64>| {
        for &firm in firms {
            g.push(worker as f64);
            h.push(firm as f64);
        }
        worker += 1;
    };
    for firm in 0..119 {
        for _ in 0..2 {
            link(&[firm, firm + 1, firm], &mut g, &mut h);
        }
    }
    // Components of one, two, three and four firms beside the chain.
    let mut next = 120;
    for size in 1..=4 {
        for _ in 0..3 {
            for firm in next..next + size {
                link(
                    &[firm, (firm + 1 - next) % size + next, firm],
                    &mut g,
                    &mut h,
                );
            }
            next += size;
        }
    }
    for row in 0..g.len() {
        w.push(0.5 + (row * 7 % 9) as f64);
    }
    let x: Vec<Vec<f64>> = (0..3)
        .map(|j| {
            (0..g.len())
                .map(|row| ((row * (j + 2)) as f64 * 0.37).sin() + h[row] * 0.01)
                .collect()
        })
        .collect();
    let p = prepared(&x[0], &x[1..], &g, &h);
    (p, w, x)
}

#[test]
fn small_implicit_components_meet_a_strict_target_and_a_shared_hierarchy_preconditions() {
    use crate::fe::slope_coefficients::joint::JointBlockPreconditioner;
    let (p, w, x) = chain_with_small_components();
    let n = w.len();
    let config = MapConfig {
        tolerance: 1e-12,
        ..MapConfig::default()
    };
    let divisor = w.iter().copied().fold(0_f64, f64::max);
    // Room for implicit banks and a shared hierarchy's scratch, not for a
    // dense reduced matrix.
    let allowance = 16 * n as u128 * 6 * 3;
    let build = |owner: Option<&mut crate::fe::ReusableCategoricalMapAbsorber<'_>>| {
        try_build_with_preconditioner(
            &p,
            &[0, 1],
            &x[0],
            BorrowedMapWeights::Provided(&w),
            divisor,
            config,
            allowance,
            0,
            &mut Vec::new(),
            &mut historical_preconditioner::Call::new(None, 0, 0),
            owner,
        )
        .unwrap()
        .expect("strict implicit products certify")
    };
    let plain = build(None);
    assert!(plain.implicit && plain.report.certified());
    let design = || p.fit_borrowed_intercept_map_design().unwrap().unwrap();
    let generation = crate::fe::MapGeometryGeneration {
        sample: 1,
        topology: 1,
        weights: 1,
    };
    let compare = |shared: &Products<'_, PreparedOlsData>| {
        assert!(shared.implicit && shared.report.certified());
        assert!(
            shared.report.iterations < plain.report.iterations,
            "{} preconditioned and {} plain steps",
            shared.report.iterations,
            plain.report.iterations
        );
        for (a, b) in shared.response.iter().zip(&plain.response) {
            assert!((a - b).abs() <= 1e-9 * (1. + b.abs()));
        }
        for (a, b) in shared.moments.gram.iter().zip(&plain.moments.gram) {
            assert!((a - b).abs() <= 1e-9 * (1. + b.abs()));
        }
    };
    // An installed hierarchy preconditions the large component.
    let mut absorber = crate::fe::ReusableCategoricalMapAbsorber::new(design(), 3, 2).unwrap();
    absorber.enable_ppml_cmg(None, u128::MAX).unwrap();
    absorber.begin_ppml_frame(generation).unwrap();
    assert!(
        crate::fe::JointCmgBlock::new(Some(&mut absorber), &w, Some(1))
            .unwrap()
            .install()
            .unwrap()
    );
    compare(&build(Some(&mut absorber)));
    assert_eq!(absorber.ppml_cmg_reach(), Some((1, 150, true)));
    // Without one, a probe beyond 64 plain steps builds it on these weights.
    let mut absorber = crate::fe::ReusableCategoricalMapAbsorber::new(design(), 3, 2).unwrap();
    absorber.enable_ppml_cmg(None, u128::MAX).unwrap();
    absorber.begin_ppml_frame(generation).unwrap();
    compare(&build(Some(&mut absorber)));
    assert_eq!(absorber.ppml_cmg_reach(), Some((1, 150, true)));
}

#[test]
fn an_unaffordable_dense_factor_uses_the_implicit_operator_without_assembly() {
    // A complete 300 x 300 bipartite design: assembling the reduced matrix
    // costs more than 64 implicit applications, and a prior operator
    // allowance admits the plan but not the cubic factorization.
    let (mut g, mut h) = (Vec::new(), Vec::new());
    for a in 0..300 {
        for b in 0..300 {
            g.push(a as f64);
            h.push(b as f64);
        }
    }
    let n = g.len();
    let w: Vec<f64> = (0..n).map(|i| 0.5 + (i * 13 % 7) as f64).collect();
    let y: Vec<f64> = (0..n)
        .map(|i| (i as f64 * 0.11).sin() + g[i] * 0.01)
        .collect();
    let x = vec![(0..n)
        .map(|i| (i as f64 * 0.29).cos() - h[i] * 0.02)
        .collect::<Vec<_>>()];
    let p = prepared(&y, &x, &g, &h);
    let config = MapConfig {
        tolerance: 1e-12,
        ..MapConfig::default()
    };
    let build = |prior_operator_work| {
        try_build(
            &p,
            &[0],
            &y,
            BorrowedMapWeights::Provided(&w),
            1.,
            config,
            u128::MAX >> 8,
            prior_operator_work,
            &mut Vec::new(),
        )
        .unwrap()
        .expect("certified products")
    };
    let implicit = build(20_000_000);
    assert!(implicit.implicit && implicit.report.certified());
    // A well-connected reduced graph finishes its probe: nothing is built and
    // later canonical solves of the estimation stop probing.
    let design = p.fit_borrowed_intercept_map_design().unwrap().unwrap();
    let mut absorber = crate::fe::ReusableCategoricalMapAbsorber::new(design, 2, 2).unwrap();
    absorber.enable_ppml_cmg(None, u128::MAX).unwrap();
    assert_eq!(absorber.ppml_cmg_reach(), Some((1, 300, false)));
    let probed = try_build_with_preconditioner(
        &p,
        &[0],
        &y,
        BorrowedMapWeights::Provided(&w),
        1.,
        config,
        u128::MAX >> 8,
        20_000_000,
        &mut Vec::new(),
        &mut historical_preconditioner::Call::new(None, 0, 0),
        Some(&mut absorber),
    )
    .unwrap()
    .unwrap();
    assert_eq!(probed.response, implicit.response);
    assert_eq!(absorber.ppml_cmg_reach(), None);
    // An allowance that affords the factor keeps the direct solve.
    let direct = build(1 << 40);
    assert!(!direct.implicit && direct.report.certified());
    for (a, b) in implicit.response.iter().zip(&direct.response) {
        assert!((a - b).abs() <= 1e-9 * (1. + b.abs()));
    }
    for (a, b) in implicit.moments.gram.iter().zip(&direct.moments.gram) {
        assert!((a - b).abs() <= 1e-9 * (1. + b.abs()));
    }
}
