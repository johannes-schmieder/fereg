use super::*;
use crate::metadata::*;
use crate::ols::*;
use std::sync::Arc;

fn fixture(rank_deficient: bool) -> SharedPreparedOlsData {
    fixture_partition(rank_deficient, false)
}

fn fixture_partition(rank_deficient: bool, interaction: bool) -> SharedPreparedOlsData {
    fixture_extra_columns(rank_deficient, interaction, 0)
}

fn fixture_extra_columns(
    rank_deficient: bool,
    interaction: bool,
    extra: usize,
) -> SharedPreparedOlsData {
    let n = 96;
    let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.71).sin()).collect();
    let z: Vec<_> = (0..n).map(|i| (i as f64 * 0.47).cos()).collect();
    let groups: Vec<_> = (0..n).map(|i| ((i * 7 + i / 8) % 8) as f64).collect();
    let y: Vec<_> = (0..n)
        .map(|i| 0.7 * x[i] - 0.2 * z[i] + groups[i] / 3.)
        .collect();
    let mut columns = vec![x.clone(), z];
    if rank_deficient {
        columns.extend([
            x.iter().map(|v| 2. * v).collect(),
            groups.clone(),
            vec![0.; n],
        ]);
    }
    columns.extend((0..extra).map(|j| {
        (0..n)
            .map(|i| ((i * (j + 3) + 1) as f64 * 0.39).sin())
            .collect::<Vec<_>>()
    }));
    let k = columns.len();
    columns.push(groups);
    if interaction {
        columns.push((0..n).map(|i| (i % 3) as f64).collect());
    }
    let sources: Vec<_> = (0..columns.len())
        .map(|j| SourceEntry {
            id: SourceId::new(j as u32 + 1).unwrap(),
            canonical_name: format!("x{j}"),
        })
        .collect();
    let terms = (0..k)
        .map(|j| ExpandedTerm {
            id: TermId::new(j as u32 + 1).unwrap(),
            order: TermOrder::new(j as u32 + 1).unwrap(),
            coefficient_slot: CoefficientSlot::new(j as u32 + 1).unwrap(),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: format!("c.x{j}"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).unwrap(),
                source: sources[j].id,
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".into(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        })
        .collect();
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
    for (j, s) in sources.iter().enumerate() {
        bindings.push(OlsInputBinding {
            stata_var_position: j as u32 + 3,
            role: OlsInputRole::NumericSource(s.id),
        });
    }
    let loaded = |values: Vec<f64>| LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    };
    let input = LoadedOlsInput {
        candidate: vec![true; n],
        outcome: loaded(y),
        numeric_sources: sources
            .iter()
            .zip(columns)
            .map(|(s, c)| (s.id, loaded(c)))
            .collect(),
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: sources.clone(),
            terms,
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept: true,
            categorical_sources: sources[k..].iter().map(|s| s.id).collect(),
            slope_sources: Vec::new(),
            label: "g".into(),
            saved_name: String::new(),
        }],
        bindings,
        include_intercept: true,
        keep_singletons: true,
    };
    Arc::new(crate::prepare_categorical_ols_data(&request, &input).unwrap())
}

#[test]
fn implicit_refresh_matches_independent_group_sums_and_revokes_failed_frames() {
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let source = fixture(true);
                let prepared = source.as_ref();
                let ids: Vec<_> = (0..prepared.model().columns.len() - 1).collect();
                let response = prepared.sample().outcome.clone();
                let mut frame = ImplicitCenteredDesign::new(
                    source.clone(),
                    &ids,
                    PreparedOlsExecutionMode::Low,
                )
                .unwrap();
                let mut norms = vec![0.; ids.len()];
                let mut means = norms.clone();
                let mut within = norms.clone();
                assert!(frame.view().is_err());
                for generation in 0..3 {
                    for scale in [1e-200, 1., 1e200] {
                        let w: Vec<_> = (0..response.len())
                            .map(|i| scale * (0.25 + ((i * 13 + generation * 11) % 19) as f64))
                            .collect();
                        let divisor = w.iter().copied().fold(0., f64::max);
                        let (_, report) = frame
                            .refresh(
                                &response,
                                BorrowedMapWeights::Provided(&w),
                                divisor,
                                1e-14,
                                &mut norms,
                                &mut means,
                                &mut within,
                                Vec::new(),
                            )
                            .unwrap();
                        assert!(report.certified());
                        let mut dense =
                            crate::data::DenseMatrix::zeros(response.len(), ids.len()).unwrap();
                        frame.write_dense(&mut dense).unwrap();
                        let view = frame.view().unwrap();
                        let mut row = vec![0.; ids.len()];
                        for observation in 0..response.len() {
                            view.fill_row(observation, &mut row).unwrap();
                            let members = view
                                .effect
                                .level_index()
                                .members(view.codes()[observation] as usize);
                            let total: f64 = members.iter().map(|&i| w[i] / divisor).sum();
                            for (column, &value) in row.iter().enumerate() {
                                let mean: f64 = members
                                    .iter()
                                    .map(|&i| {
                                        w[i] / divisor
                                            * prepared.fit_column_value(ids[column], i).unwrap()
                                    })
                                    .sum::<f64>()
                                    / total;
                                let expected =
                                    prepared.fit_column_value(ids[column], observation).unwrap()
                                        - mean;
                                assert!((value - expected).abs() < 1e-12);
                                assert!((dense[(observation, column)] - expected).abs() < 1e-12);
                                assert!(value.is_finite());
                            }
                        }
                    }
                }
                let mut bad = vec![1.; response.len()];
                bad[3] = f64::NAN;
                assert!(frame
                    .refresh(
                        &response,
                        BorrowedMapWeights::Provided(&bad),
                        1.,
                        1e-14,
                        &mut norms,
                        &mut means,
                        &mut within,
                        Vec::new(),
                    )
                    .is_err());
                assert!(frame.view().is_err());
                let mut unpublished =
                    crate::data::DenseMatrix::zeros(response.len(), ids.len()).unwrap();
                assert!(frame.write_dense(&mut unpublished).is_err());
                assert!(unpublished.as_slice().iter().all(|&v| v == 0.0));
                frame
                    .refresh(
                        &response,
                        BorrowedMapWeights::Unit,
                        1.,
                        1e-14,
                        &mut norms,
                        &mut means,
                        &mut within,
                        Vec::new(),
                    )
                    .unwrap();
                let expected = {
                    let mut row = vec![0.; ids.len()];
                    frame.view().unwrap().fill_row(7, &mut row).unwrap();
                    row
                };
                frame.compact(&[0, 2]).unwrap();
                let mut actual = vec![0.; 2];
                frame.view().unwrap().fill_row(7, &mut actual).unwrap();
                assert_eq!(actual, vec![expected[0], expected[2]]);
                assert!(
                    frame.resident_bytes()
                        <= ImplicitCenteredDesign::bytes(8, ids.len(), threads).unwrap()
                );
            });
    }
}

#[test]
fn implicit_rank_fallback_and_owned_final_inference_preserve_the_linear_solution() {
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let source = fixture(true);
        let prepared = source.as_ref();
        let y = &prepared.sample().outcome;
        let w: Vec<_> = (0..y.len()).map(|i| 0.25 + (i % 11) as f64).collect();
        let divisor = w.iter().copied().fold(0., f64::max);
        let frame = LinearSolveFrame::from_parts(
            y,
            BorrowedMapWeights::Provided(&w),
            PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: divisor,
                reference_weight_multiplier: divisor,
                reference_weight_sum: w.iter().sum(),
            },
        )
        .unwrap();
        let mut config = PreparedOlsFitConfig::default();
        config.absorb.solver = FeSolverKind::Map;
        config.absorb.map.tolerance = 1e-12;
        config.execution.mode = mode;
        config.execution.poolsize = 1;
        config.coefficient_tolerance = Some(1e-12);
        let mut session = CertifiedLinearSession::new_with_fe_design(
            prepared,
            frame,
            config,
            LinearSessionGenerations::new(1, 1, 1, 1, 1),
            prepared.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        let other = fixture(true);
        assert!(session
            .enable_group_centered_one_fe(other, u128::MAX)
            .is_err());
        let admitted = ImplicitCenteredDesign::bytes(
            session.fe_design().effects()[0].n_levels(),
            prepared.fit_model().columns.len(),
            config.execution.threads.resolved(),
        )
        .unwrap();
        assert_eq!(
            session
                .enable_group_centered_one_fe(source.clone(), admitted - 1)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        session
            .enable_group_centered_one_fe(source.clone(), admitted)
            .unwrap();
        let output = session.solve_fresh().unwrap();
        assert!(output.certified());
        assert_eq!(output.basis().kept_generated_columns().len(), 2);
        assert_eq!(
            output
                .coefficients()
                .filter(|c| {
                    c.generated_column_index.is_some_and(|i| i < 3)
                        && c.status == PreparedOlsFitStatus::NumericallyRankDeficient
                })
                .count(),
            1,
            "exact duplicate loses one finite rank direction"
        );
        for generation in 2..=4 {
            let next_weights: Vec<_> = (0..y.len())
                .map(|i| 0.5 + ((i * 7 + generation as usize) % 13) as f64)
                .collect();
            let maximum = next_weights.iter().copied().fold(0.0_f64, f64::max);
            session
                .refresh_weights(
                    BorrowedMapWeights::Provided(&next_weights),
                    PreparedOlsBreadWeightScale {
                        weight_kind: None,
                        solver_weight_divisor: maximum,
                        reference_weight_multiplier: maximum,
                        reference_weight_sum: next_weights.iter().sum(),
                    },
                    generation,
                )
                .unwrap();
            let strict = session.strictify().unwrap();
            assert!(strict.certified());
            assert_eq!(strict.basis().kept_generated_columns().len(), 2);
            assert!(strict.frame().residuals().iter().all(|r| r.abs() < 1e-10));
        }
        let products = session.into_current_products(&mut || Ok(())).unwrap();
        assert!(products.frame().residuals().iter().all(|r| r.abs() < 1e-10));
        let weak = Arc::downgrade(&source);
        drop(source);
        assert_eq!(
            weak.upgrade().is_some(),
            mode == PreparedOlsExecutionMode::Low,
            "implicit products retain the input owner; dense products own every projected value"
        );
        let design = products.finite_within_design();
        use crate::sandwich::SandwichDesign;
        let mut row = vec![0.; design.dimension()];
        let mut count = 0;
        design
            .visit_range(0..design.nrows(), &mut row, |_, values| {
                assert!(values.iter().all(|x| x.is_finite()));
                count += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(count, 96);
        drop(products);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn implicit_interaction_partition_and_zero_controls_match_closed_form_projection() {
    let source = fixture_partition(false, true);
    let prepared = source.as_ref();
    assert!(prepared.validated_single_topology().unwrap().is_none());
    assert!(prepared.fit_has_one_ordinary_intercept_effect());
    let y = &prepared.sample().outcome;
    let weights: Vec<_> = (0..y.len()).map(|i| 0.5 + (i % 7) as f64).collect();
    for ids in [vec![0, 1], vec![]] {
        let mut frame =
            ImplicitCenteredDesign::new(source.clone(), &ids, PreparedOlsExecutionMode::Low)
                .unwrap();
        let mut raw = vec![0.; ids.len()];
        let mut means = raw.clone();
        let mut within = raw.clone();
        let (residual, report) = frame
            .refresh(
                y,
                BorrowedMapWeights::Provided(&weights),
                6.5,
                1e-14,
                &mut raw,
                &mut means,
                &mut within,
                Vec::new(),
            )
            .unwrap();
        assert!(report.certified());
        let view = frame.view().unwrap();
        let groups: std::collections::BTreeSet<_> =
            (0..y.len()).map(|i| ((i * 7 + i / 8) % 8, i % 3)).collect();
        assert_eq!(view.groups(), groups.len());
        let mut row = vec![0.; ids.len()];
        for i in 0..y.len() {
            // Independently rebuild interaction membership from the input design,
            // without using the retained codes or membership index.
            let same = |j: usize| (i * 7 + i / 8) % 8 == (j * 7 + j / 8) % 8 && i % 3 == j % 3;
            let total: f64 = (0..y.len()).filter(|&j| same(j)).map(|j| weights[j]).sum();
            let mean: f64 = (0..y.len())
                .filter(|&j| same(j))
                .map(|j| weights[j] * y[j])
                .sum::<f64>()
                / total;
            assert!((residual[i] - (y[i] - mean)).abs() < 1e-12);
            view.fill_row(i, &mut row).unwrap();
            for (column, value) in row.iter().enumerate() {
                let mean: f64 = (0..y.len())
                    .filter(|&j| same(j))
                    .map(|j| weights[j] * prepared.fit_column_value(ids[column], j).unwrap())
                    .sum::<f64>()
                    / total;
                assert!(
                    (*value - (prepared.fit_column_value(ids[column], i).unwrap() - mean)).abs()
                        < 1e-12
                );
            }
        }
        assert!(
            frame.resident_bytes()
                <= ImplicitCenteredDesign::bytes(view.groups(), ids.len(), 4).unwrap()
        );
    }
    assert!(ImplicitCenteredDesign::bytes(usize::MAX, usize::MAX, usize::MAX).is_err());
}

#[test]
fn implicit_moments_match_ordered_scalar_oracle_across_private_workspace_boundary() {
    use crate::ols::finite_wls_kernel::{
        fill_finite_wls_moments, finite_wls_moment_layout, FixedBlockMomentSource,
    };
    for (stored, active) in [(2, 1), (6, 6), (7, 3), (64, 64), (65, 64), (65, 65)] {
        let source = fixture_extra_columns(false, false, stored - 2);
        let prepared = source.as_ref();
        let ids: Vec<_> = (0..stored).collect();
        let selected: Vec<_> = (0..active)
            .map(|j| {
                if active == 1 {
                    stored - 1
                } else {
                    j * (stored - 1) / (active - 1)
                }
            })
            .collect();
        let scales: Vec<_> = (0..active)
            .map(|j| 2.0_f64.powi((j % 3) as i32 * 100 - 100))
            .collect();
        let y = &prepared.sample().outcome;
        let mut frame =
            ImplicitCenteredDesign::new(source.clone(), &ids, PreparedOlsExecutionMode::Low)
                .unwrap();
        let mut raw = vec![0.; stored];
        let mut means = raw.clone();
        let mut norms = raw.clone();
        for (generation, magnitude) in [1e-200, 1., 1e200].into_iter().enumerate() {
            let weights: Vec<_> = (0..y.len())
                .map(|i| magnitude * (0.25 + ((i * 13 + generation * 11) % 19) as f64))
                .collect();
            let divisor = weights.iter().copied().fold(0.0_f64, f64::max);
            let (response, report) = frame
                .refresh(
                    y,
                    BorrowedMapWeights::Provided(&weights),
                    divisor,
                    1e-14,
                    &mut raw,
                    &mut means,
                    &mut norms,
                    Vec::new(),
                )
                .unwrap();
            assert!(report.certified());
            let view = frame.view().unwrap();
            for scaled in [false, true] {
                let mut expected_rhs = vec![0.0_f64; active];
                let mut expected_gram = vec![0.0_f64; active * active];
                // Separate scalar per-entry sums replay originals and the
                // already independently tested group centers, not kernel rows.
                for left in 0..active {
                    for i in 0..y.len() {
                        let group = view.codes()[i] as usize;
                        let value = |j: usize| {
                            (prepared.fit_column_value(ids[selected[j]], i).unwrap()
                                - view.center(selected[j], group))
                                / if scaled { scales[j] } else { 1. }
                        };
                        let weighted_left = weights[i] / divisor * value(left);
                        expected_rhs[left] = weighted_left.mul_add(response[i], expected_rhs[left]);
                        for right in left..active {
                            let position = left * active + right;
                            expected_gram[position] =
                                weighted_left.mul_add(value(right), expected_gram[position]);
                        }
                    }
                    for right in left..active {
                        expected_gram[right * active + left] = expected_gram[left * active + right];
                    }
                }
                for threads in [1, 2, 4, 16] {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| {
                            let source = FixedBlockMomentSource::Implicit {
                                values: view,
                                selected_columns: Some(&selected),
                                response: (&response).into(),
                                weights: BorrowedMapWeights::Provided(&weights),
                                solver_weight_divisor: divisor,
                                column_scales: scaled.then_some(scales.as_slice()),
                            };
                            let layout = finite_wls_moment_layout(source).unwrap();
                            let mut partials = vec![0.; layout.partial_elements];
                            let mut rhs = vec![0.; active];
                            let mut gram = vec![0.; active * active];
                            fill_finite_wls_moments(
                                source,
                                layout,
                                &mut partials,
                                &mut rhs,
                                &mut gram,
                            )
                            .unwrap();
                            assert_eq!(rhs, expected_rhs);
                            assert_eq!(gram, expected_gram);
                        });
                }
            }
        }
    }
}
