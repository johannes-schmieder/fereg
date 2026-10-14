use super::*;
use crate::fe::retained_effect_order_by_levels;
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder,
    ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent,
    TermId, TermOrder, TimeSeriesState,
};
use crate::ols::{
    inject_frozen_linear_failure, prepare_model, FrozenLinearFailureInjection, LoadedNumericColumn,
    LoadedOlsInput, OlsInputBinding, OlsInputRole, OlsRequestV1, PreparedOlsData,
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn source(id: u32, name: &str) -> SourceEntry {
    SourceEntry {
        id: SourceId::new(id).expect("source"),
        canonical_name: name.to_owned(),
    }
}

fn continuous_term(id: u32, source: SourceId) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("order"),
        coefficient_slot: CoefficientSlot::new(id).expect("slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("c.x{id}"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component"),
            source,
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn loaded(values: &[f64]) -> LoadedNumericColumn {
    LoadedNumericColumn {
        values: values.to_vec(),
        missing: vec![false; values.len()],
    }
}

fn prepared(outcome: &[f64], regressors: &[Vec<f64>]) -> PreparedOlsData {
    prepared_with_factor_sources(outcome, regressors, 0)
}

fn prepared_with_factor_sources(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    factor_count: usize,
) -> PreparedOlsData {
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    let mut numeric_sources = Vec::new();
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
    for (zero_based, values) in regressors.iter().enumerate() {
        let id = u32::try_from(zero_based + 1).expect("small fixture");
        let entry = source(id, &format!("x{id}"));
        terms.push(continuous_term(id, entry.id));
        bindings.push(OlsInputBinding {
            stata_var_position: id + 2,
            role: OlsInputRole::NumericSource(entry.id),
        });
        numeric_sources.push((entry.id, loaded(values)));
        sources.push(entry);
    }
    let mut request = OlsRequestV1 {
        model: CompiledTermGraph { sources, terms },
        absorbed_terms: Vec::new(),
        bindings,
        include_intercept: true,
        keep_singletons: true,
    };
    let original_model = request
        .validate_and_compile()
        .expect("numeric fixture model");
    // Use the real factor grammar to create immutable source handles. The
    // score witness then restores its original numeric recipe/slot axis.
    for term in request.model.terms.iter_mut().take(factor_count) {
        term.raw_kind = RawTermKind::Factor;
        term.canonical_token = format!("1.x{}", term.id.get());
        let component = &mut term.components[0];
        component.kind = ComponentKind::Categorical;
        component.level_state = LevelState::Present;
        component.level_value = 1.;
        component.omitted = MetadataFlag::False;
        component.raw_operator = "1".into();
    }
    let mut prepared = prepare_model(
        &request,
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(outcome),
            numeric_sources,
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        false,
    )
    .expect("prepared fixture");
    if factor_count != 0 {
        prepared.model = original_model;
    }
    prepared
}

fn prepared_no_fe_categorical(outcome: &[f64], regressors: &[Vec<f64>]) -> crate::PreparedOlsData {
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    let mut numeric_sources = Vec::new();
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
    for (zero_based, values) in regressors.iter().enumerate() {
        let id = u32::try_from(zero_based + 1).expect("small fixture");
        let entry = source(id, &format!("x{id}"));
        terms.push(continuous_term(id, entry.id));
        bindings.push(OlsInputBinding {
            stata_var_position: id + 2,
            role: OlsInputRole::NumericSource(entry.id),
        });
        numeric_sources.push((entry.id, loaded(values)));
        sources.push(entry);
    }
    crate::ols::prepared_categorical_data::prepare_ppml_categorical_ols_data_with_load_hint(
        &OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms: Vec::new(),
            bindings,
            include_intercept: true,
            keep_singletons: true,
        },
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(outcome),
            numeric_sources,
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        &[],
        None,
        0,
    )
    .expect("prepared fixture")
}

#[test]
fn original_profile_columns_match_cold_design_across_bindings_and_weight_frames() {
    use super::super::one_fe_profile::ProfiledDesign;
    for (rows, width) in [(80, 3), (4096, 3), (4096, 65)] {
        let y: Vec<_> = (0..rows).map(|r| 0.3 + (r % 13) as f64).collect();
        let x: Vec<Vec<_>> = (0..width)
            .map(|j| {
                (0..rows)
                    .map(|r| (r as f64 / (j + 3) as f64).sin() + (r % 17) as f64 * (j + 1) as f64)
                    .collect()
            })
            .collect();
        let prepared = prepared(&y, &x);
        let design = FeDesign::new(
            rows,
            vec![crate::fe::EffectTerm::new(
                (0..rows).map(|r| (r % 17) as u32).collect(),
                17,
                true,
                vec![],
            )
            .unwrap()],
        )
        .unwrap();
        let active = [2, 0];
        let columns = ProfiledRawColumns::new(&prepared, &active).unwrap();
        for row in 0..rows {
            for (position, &generated) in active.iter().enumerate() {
                assert_eq!(
                    (&columns).value(row, position).unwrap(),
                    prepared.fit_column_value(generated, row).unwrap()
                );
            }
        }
        assert!(ProfiledRawColumns::new(&prepared, &[usize::MAX]).is_err());
        let offset: Vec<_> = (0..rows).map(|r| (r as f64 / 19.).cos() * 0.1).collect();
        let coefficients = vec![0.03; prepared.fit_model().coefficients.len()];
        for threads in [1, 4] {
            let threads = crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            for frame in [1, 7] {
                let weights: Vec<_> = (0..rows).map(|r| 0.2 + ((r * frame) % 13) as f64).collect();
                let mut profile =
                    ProfiledOneFe::new(&prepared, &active, &design, &y, &weights, threads).unwrap();
                profile
                    .prepare_direction(
                        &columns,
                        &design,
                        &y,
                        &weights,
                        &offset,
                        &coefficients,
                        PpmlConfig::default(),
                    )
                    .unwrap();
                assert!(profile
                    .fresh_direction_equivalent(
                        &prepared,
                        &design,
                        &y,
                        &weights,
                        &offset,
                        &coefficients,
                        threads,
                        PpmlConfig::default(),
                        1e-12
                    )
                    .unwrap());
            }
        }
    }
}

#[path = "../../../../fereg-testkit/src/ppml_score_reference.rs"]
mod score_reference;

#[test]
fn score_fixed_leaves_are_exact_across_scratch_waves_and_workers() {
    for rows in [0, 1, 511, 512, 513, 4097, 65_537] {
        for width in [1, 17, 64] {
            let leaf = |column: usize, range: std::ops::Range<usize>| {
                let mut moment = CompensatedSum::default();
                let mut square = CompensatedSum::default();
                for row in range {
                    let value = match (row + column) % 5 {
                        0 => 1e30,
                        1 => -1e30,
                        2 => 1e-30,
                        _ => ((row + 7 * column) % 17) as f64 / 3.,
                    };
                    moment.add(value);
                    square.add(value * value);
                }
                Ok((moment, square))
            };
            let mut expected = None;
            for threads in [1, 4, 16] {
                let team = crate::parallel::ThreadTeam::new(
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
                )
                .acquire()
                .unwrap();
                for capacity in [0, 1, 2, 17, 64, 257, 8192] {
                    let mut workspace = PpmlScoreWorkspace {
                        team: team.clone(),
                        fe_moments: vec![f64::NAN; capacity],
                        fe_basis_squares: vec![f64::NAN; capacity],
                    };
                    let storage = (
                        workspace.fe_moments.as_ptr(),
                        workspace.fe_moments.capacity(),
                        workspace.fe_basis_squares.as_ptr(),
                        workspace.fe_basis_squares.capacity(),
                    );
                    let mut actual = vec![(0., 0.); width];
                    workspace.reduce_columns(rows, &mut actual, leaf).unwrap();
                    let bits: Vec<_> = actual
                        .iter()
                        .map(|(m, s)| (m.to_bits(), s.to_bits()))
                        .collect();
                    if let Some(expected) = &expected {
                        assert_eq!(
                            &bits, expected,
                            "rows={rows}, width={width}, threads={threads}, capacity={capacity}"
                        );
                    } else {
                        expected = Some(bits);
                    }
                    assert_eq!(
                        storage,
                        (
                            workspace.fe_moments.as_ptr(),
                            workspace.fe_moments.capacity(),
                            workspace.fe_basis_squares.as_ptr(),
                            workspace.fe_basis_squares.capacity(),
                        )
                    );
                    if rows <= 512 {
                        for (column, actual) in actual.iter().enumerate() {
                            let (moment, square) = leaf(column, 0..rows).unwrap();
                            assert_eq!(*actual, (moment.total(), square.total()));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn score_fixed_leaves_retain_small_components_across_large_cancellation() {
    for rows in [1027, 65_537] {
        for threads in [1, 4, 16] {
            let team = crate::parallel::ThreadTeam::new(
                crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
            )
            .acquire()
            .unwrap();
            for capacity in [0, 1, 2, 17, 64, 257, 8192] {
                let mut workspace = PpmlScoreWorkspace {
                    team: team.clone(),
                    fe_moments: vec![f64::NAN; capacity],
                    fe_basis_squares: vec![f64::NAN; capacity],
                };
                let mut result = [(0., 0.)];
                workspace
                    .reduce_columns(rows, &mut result, |_, range| {
                        let mut moment = CompensatedSum::default();
                        let mut square = CompensatedSum::default();
                        for row in range {
                            moment.add(match row {
                                0 => 1e30,
                                1 => 1.,
                                512 => -1e30,
                                _ => 0.,
                            });
                            square.add(match row {
                                0 | 512 => 2_f64.powi(54),
                                1 | 513 | 1024..=1026 => 1.,
                                _ => 0.,
                            });
                        }
                        Ok((moment, square))
                    })
                    .unwrap();
                // Independent exact sums: the equal large signed terms
                // cancel. The positive sum is 2^55+5, rounding to 2^55+8.
                assert_eq!(result[0].0.to_bits(), 1_f64.to_bits());
                assert_eq!(result[0].1.to_bits(), (2_f64.powi(55) + 8.).to_bits());
            }
        }
    }
    let rows = 65_537;
    let y = vec![2.; rows];
    let mut x = vec![0.; rows];
    x[0] = 1e30;
    x[1] = 1.;
    x[512] = -1e30;
    let mut prepared = prepared(&y, &[x]);
    // This score-only model has the single explicit signed basis above.
    prepared
        .model
        .columns
        .retain(|column| !matches!(column.kind, PreparedOlsColumnKind::Intercept));
    let design = FeDesign::new(rows, vec![]).unwrap();
    for threads in [1, 4, 16] {
        let mut workspace = PpmlScoreWorkspace::new(
            &design,
            crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
        )
        .unwrap();
        let certificate = prepared_score_certificate(
            &prepared,
            &design,
            &y,
            &vec![1.; rows],
            &vec![1.; rows],
            1e-40,
            &mut workspace,
        )
        .unwrap();
        assert_eq!(
            certificate.finite_absolute_infinity_norm.to_bits(),
            1_f64.to_bits()
        );
        assert!(
            !certificate.passed,
            "a canceled rounded leaf must not create a false zero score"
        );
    }
}

#[test]
fn independent_finite_scores_match_integer_moments_and_scale_on_large_rows() {
    let rows = 65_537;
    let y: Vec<_> = (0..rows).map(|r| (r % 5) as i64).collect();
    let q: Vec<_> = (0..rows).map(|r| (1 + r % 7) as i64).collect();
    for width in [1, 17, 65, 129] {
        let columns: Vec<Vec<_>> = (0..width)
            .map(|j| (0..rows).map(|r| ((r + j * 3) % 11) as i64 - 5).collect())
            .collect();
        let x: Vec<Vec<_>> = columns
            .iter()
            .map(|column| column.iter().map(|&v| v as f64).collect())
            .collect();
        let outcome: Vec<_> = y.iter().map(|&v| v as f64).collect();
        let prepared = prepared(&outcome, &x);
        let design = FeDesign::new(rows, vec![]).unwrap();
        for frame in [1, 2] {
            let mu: Vec<_> = (0..rows).map(|r| (1 + (r * frame) % 2) as i64).collect();
            let mean: Vec<_> = mu.iter().map(|&v| v as f64).collect();
            for exponent in [-400, 0, 400] {
                let scale = 2_f64.powi(exponent);
                let weights: Vec<_> = q.iter().map(|&v| v as f64 * scale).collect();
                let score_scale = score_reference::score_scale(&y, &mu, &q, scale);
                let mut moments: Vec<_> = columns
                    .iter()
                    .map(|column| score_reference::moments(&y, &mu, &q, column, scale))
                    .collect();
                // prepared() adds the explicit intercept after its controls.
                moments.push(score_reference::moments(&y, &mu, &q, &vec![1; rows], scale));
                let absolute = moments.iter().map(|(m, _)| m.abs()).fold(0., f64::max);
                let relative: Vec<_> = moments
                    .iter()
                    .map(|(m, s)| m.abs() / (s.sqrt() * score_scale))
                    .collect();
                let infinity = relative.iter().copied().fold(0., f64::max);
                let two =
                    (relative.iter().map(|v| v * v).sum::<f64>() / moments.len() as f64).sqrt();
                for threads in [1, 4, 16] {
                    let mut workspace = PpmlScoreWorkspace::new(
                        &design,
                        crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .unwrap();
                    let actual = prepared_score_certificate(
                        &prepared,
                        &design,
                        &outcome,
                        &mean,
                        &weights,
                        1e-8,
                        &mut workspace,
                    )
                    .unwrap();
                    assert_eq!(
                        actual.finite_absolute_infinity_norm.to_bits(),
                        absolute.to_bits()
                    );
                    assert_eq!(actual.fixed_effect_absolute_infinity_norm, 0.);
                    assert!(
                        (actual.relative_infinity_norm - infinity).abs()
                            <= 8. * f64::EPSILON * infinity
                    );
                    assert!((actual.relative_two_norm - two).abs() <= 8. * f64::EPSILON * two);
                    assert_eq!(actual.passed, infinity <= 1e-8 && two <= 1e-8);
                }
            }
        }
    }
}

#[test]
fn score_leaf_errors_preserve_scale_failure_before_column_validation() {
    let rows = 4097;
    let y = vec![1.; rows];
    let mut prepared = prepared(&y, &[vec![2.; rows]]);
    let crate::ols::PreparedOlsColumnKind::Continuous {
        prepared_source_index,
        ..
    } = &mut prepared.model.columns[0].kind
    else {
        panic!("continuous fixture")
    };
    *prepared_source_index = usize::MAX;
    let design = FeDesign::new(rows, vec![]).unwrap();
    for threads in [1, 4, 16] {
        let mut workspace = PpmlScoreWorkspace::new(
            &design,
            crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
        )
        .unwrap();
        let error = prepared_score_certificate(
            &prepared,
            &design,
            &y,
            &vec![0.; rows],
            &vec![1.; rows],
            1e-8,
            &mut workspace,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(error.message, "prepared PPML score scale is not finite");
        let error = prepared_score_certificate(
            &prepared,
            &design,
            &y,
            &vec![1.; rows],
            &vec![1.; rows],
            1e-8,
            &mut workspace,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(error.message, "numeric OLS column is not aligned");
    }
}

#[test]
fn numeric_outer_scores_preserve_every_certificate_bit_across_teams_and_weights() {
    for rows in [511, 512, 4096] {
        let y: Vec<_> = (0..rows).map(|r| (r % 13) as f64 / 3.0).collect();
        let mu: Vec<_> = (0..rows)
            .map(|r| (0.4 * (r as f64 / 91.0).sin()).exp())
            .collect();
        let outer = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        for width in [1, 17, 63, 64, 65, 129] {
            let x: Vec<Vec<_>> = (0..width)
                .map(|j| {
                    (0..rows)
                        .map(|r| (r as f64 / (j + 3) as f64).sin() + 0.1 * (r % 17) as f64)
                        .collect()
                })
                .collect();
            let prepared = prepared(&y, &x);
            let design = FeDesign::new(
                rows,
                vec![crate::fe::EffectTerm::new(
                    (0..rows).map(|r| (r % 17) as u32).collect(),
                    17,
                    true,
                    vec![x[0].clone()],
                )
                .unwrap()],
            )
            .unwrap();
            for scale in [1e-30, 1.0, 1e30] {
                let q: Vec<_> = (0..rows)
                    .map(|r| scale * (0.25 + (r % 11) as f64))
                    .collect();
                let mut expected = PpmlScoreWorkspace::new(
                    &design,
                    crate::options::ThreadConfig::from_requested(1, true).unwrap(),
                )
                .unwrap();
                FORCE_GENERIC_OUTER_SCORE.set(true);
                let baseline = prepared_score_certificate(
                    &prepared,
                    &design,
                    &y,
                    &mu,
                    &q,
                    1e-8,
                    &mut expected,
                );
                FORCE_GENERIC_OUTER_SCORE.set(false);
                let baseline = baseline.unwrap();
                for threads in [1, 4, 16] {
                    let mut workspace = PpmlScoreWorkspace::new(
                        &design,
                        crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .unwrap();
                    let actual = outer
                        .install(|| {
                            prepared_score_certificate(
                                &prepared,
                                &design,
                                &y,
                                &mu,
                                &q,
                                1e-8,
                                &mut workspace,
                            )
                        })
                        .unwrap();
                    assert_eq!(actual, baseline);
                    assert_eq!(workspace.fe_moments, expected.fe_moments);
                    assert_eq!(workspace.fe_basis_squares, expected.fe_basis_squares);
                }
            }
        }
    }
}

#[test]
fn generated_outer_scores_preserve_fresh_certificates_across_tiles_frames_and_teams() {
    use crate::ols::{PreparedOlsCategoricalComponent, PreparedOlsColumnKind};
    for rows in [511, 512, 4095, 4097] {
        for width in [8, 17, 65, 129] {
            let y: Vec<_> = (0..rows).map(|r| (r % 13) as f64 / 3.).collect();
            let x: Vec<Vec<_>> = (0..width)
                .map(|j| {
                    (0..rows)
                        .map(|r| {
                            if j < 2 {
                                ((r / (j + 1)) % (j + 3)) as f64
                            } else {
                                (r as f64 / (j + 3) as f64).sin()
                            }
                        })
                        .collect()
                })
                .collect();
            let mut prepared = prepared_with_factor_sources(&y, &x, 2);
            for j in 2..width {
                let component = |source_index: usize, code| PreparedOlsCategoricalComponent {
                    source: SourceId::new((source_index + 1) as u32).unwrap(),
                    categorical_source_index: source_index,
                    level_bits: 1f64.to_bits(),
                    level_code: code,
                };
                prepared.model.columns[j].kind = match j % 3 {
                    0 => PreparedOlsColumnKind::Categorical {
                        source: SourceId::new(1).unwrap(),
                        prepared_source_index: 0,
                        categorical_source_index: 0,
                        level_bits: 1f64.to_bits(),
                        level_code: Some(1),
                    },
                    1 => PreparedOlsColumnKind::CategoricalInteraction {
                        components: vec![
                            component(0, Some(1)),
                            component(1, Some(2)),
                            component(0, if j % 7 == 0 { None } else { Some(1) }),
                        ],
                    },
                    _ => prepared.model.columns[j].kind.clone(),
                };
            }
            let design = FeDesign::new(
                rows,
                vec![crate::fe::EffectTerm::new(
                    (0..rows).map(|r| (r % 17) as u32).collect(),
                    17,
                    true,
                    vec![x[2].clone()],
                )
                .unwrap()],
            )
            .unwrap();
            for frame in [1, 7] {
                let mu: Vec<_> = (0..rows)
                    .map(|r| (0.4 * (r as f64 / (91 * frame) as f64).sin()).exp())
                    .collect();
                let q: Vec<_> = (0..rows)
                    .map(|r| [1e-30, 1., 1e30][r % 3] * (0.25 + ((r * frame) % 11) as f64))
                    .collect();
                let mut reference = PpmlScoreWorkspace::new(
                    &design,
                    crate::options::ThreadConfig::from_requested(1, true).unwrap(),
                )
                .unwrap();
                FORCE_GENERIC_OUTER_SCORE.set(true);
                let expected = prepared_score_certificate(
                    &prepared,
                    &design,
                    &y,
                    &mu,
                    &q,
                    1e-8,
                    &mut reference,
                );
                FORCE_GENERIC_OUTER_SCORE.set(false);
                let expected = expected.unwrap();
                for threads in [1, 4, 16] {
                    let mut workspace = PpmlScoreWorkspace::new(
                        &design,
                        crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .unwrap();
                    let actual = prepared_score_certificate(
                        &prepared,
                        &design,
                        &y,
                        &mu,
                        &q,
                        1e-8,
                        &mut workspace,
                    )
                    .unwrap();
                    assert_eq!(actual, expected);
                    assert_eq!(workspace.fe_moments, reference.fe_moments);
                    assert_eq!(workspace.fe_basis_squares, reference.fe_basis_squares);
                }
            }
        }
    }
}

#[test]
fn independent_fe_scores_match_integer_design_across_frames_and_empty_directions() {
    use crate::fe::EffectTerm;
    for rows in [4097, 16385] {
        let y: Vec<_> = (0..rows)
            .map(|r| if r % 3 == 0 { 0. } else { 2. })
            .collect();
        let z: Vec<_> = (0..rows).map(|r| (r % 5) as f64 - 2.).collect();
        let w: Vec<_> = (0..rows).map(|r| (r % 7) as f64 - 3.).collect();
        let x: Vec<_> = (0..rows).map(|r| (r as f64 / 17.).sin()).collect();
        let prepared = prepared(&y, &[x]);
        let first: Vec<_> = (0..rows)
            .map(|r| (12 * (r % 2) + if r % 97 < 80 { 0 } else { 1 + (r / 2) % 7 }) as u32)
            .collect();
        let second: Vec<_> = (0..rows)
            .map(|r| (5 * (r % 2) + (r / 2) % 3) as u32)
            .collect();
        let third: Vec<_> = (0..rows).map(|r| (r % 2) as u32).collect();
        // Two disconnected components, a dominant group, unused labels,
        // duplicate slopes, a slope-only effect and an identically zero
        // direction exercise ownership without granting any rank authority.
        let specs = [
            (first, 24, true, vec![z.clone(), z]),
            (second, 10, false, vec![w, vec![0.; rows]]),
            (third, 3, true, vec![]),
        ];
        let mut explicit = Vec::new();
        let mut effects = Vec::new();
        for (codes, levels, intercept, slopes) in specs {
            let channels = usize::from(intercept) + slopes.len();
            for level in 0..levels {
                for channel in 0..channels {
                    explicit.push(
                        (0..rows)
                            .map(|r| {
                                if codes[r] as usize != level {
                                    0_i64
                                } else if intercept && channel == 0 {
                                    1
                                } else {
                                    slopes[channel - usize::from(intercept)][r] as i64
                                }
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            }
            effects.push(EffectTerm::new(codes, levels, intercept, slopes).unwrap());
        }
        let design = FeDesign::new(rows, effects).unwrap();
        for threads in [1, 4, 8] {
            let mut workspace = PpmlScoreWorkspace::new(
                &design,
                crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
            )
            .unwrap();
            let storage = (
                workspace.fe_moments.as_ptr(),
                workspace.fe_moments.capacity(),
                workspace.fe_basis_squares.as_ptr(),
                workspace.fe_basis_squares.capacity(),
            );
            for frame in [1, 7] {
                let mu: Vec<_> = (0..rows).map(|r| (1 + (r + frame) % 2) as f64).collect();
                let q: Vec<_> = (0..rows).map(|r| (1 + (r * frame) % 7) as i64).collect();
                // Integer explicit-design dot products are an independent
                // exact oracle; binary common scales remain exactly representable.
                let expected: Vec<_> = explicit
                    .iter()
                    .map(|column| {
                        let mut score = 0_i64;
                        let mut square = 0_i64;
                        for r in 0..rows {
                            score += q[r] * (y[r] as i64 - mu[r] as i64) * column[r];
                            square += q[r] * mu[r] as i64 * column[r] * column[r];
                        }
                        (score, square)
                    })
                    .collect();
                for exponent in [-400, 0, 400] {
                    let scale = 2_f64.powi(exponent);
                    let weights: Vec<_> = q.iter().map(|&value| value as f64 * scale).collect();
                    workspace.fe_moments.fill(f64::NAN);
                    workspace.fe_basis_squares.fill(f64::NAN);
                    let actual = prepared_score_certificate(
                        &prepared,
                        &design,
                        &y,
                        &mu,
                        &weights,
                        1e-8,
                        &mut workspace,
                    )
                    .unwrap();
                    assert!(!actual.passed);
                    for (direction, &(score, square)) in expected.iter().enumerate() {
                        assert_eq!(workspace.fe_moments[direction], score as f64 * scale);
                        assert_eq!(workspace.fe_basis_squares[direction], square as f64 * scale);
                    }
                    assert_eq!(
                        (
                            workspace.fe_moments.as_ptr(),
                            workspace.fe_moments.capacity(),
                            workspace.fe_basis_squares.as_ptr(),
                            workspace.fe_basis_squares.capacity()
                        ),
                        storage
                    );
                }
            }
        }
    }
}

fn prepared_two_fe(outcome: &[f64], regressor: &[f64]) -> PreparedOlsData {
    assert_eq!(outcome.len(), 8);
    prepared_two_fe_with_codes(
        outcome,
        regressor,
        &[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0],
        &[1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0],
    )
}

fn prepared_two_fe_with_codes(
    outcome: &[f64],
    regressor: &[f64],
    first: &[f64],
    second: &[f64],
) -> PreparedOlsData {
    assert_eq!(regressor.len(), outcome.len());
    let x = source(1, "x");
    let g = source(2, "g");
    let h = source(3, "h");
    prepare_model(
        &OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![x.clone(), g.clone(), h.clone()],
                terms: vec![continuous_term(1, x.id)],
            },
            absorbed_terms: vec![
                AbsorbedTerm {
                    id: AbsorbTermId::new(1).expect("g absorb id"),
                    intercept: true,
                    categorical_sources: vec![g.id],
                    slope_sources: Vec::new(),
                    label: "g".to_owned(),
                    saved_name: String::new(),
                },
                AbsorbedTerm {
                    id: AbsorbTermId::new(2).expect("h absorb id"),
                    intercept: true,
                    categorical_sources: vec![h.id],
                    slope_sources: Vec::new(),
                    label: "h".to_owned(),
                    saved_name: String::new(),
                },
            ],
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(x.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(g.id),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(h.id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        },
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(outcome),
            numeric_sources: vec![
                (x.id, loaded(regressor)),
                (g.id, loaded(first)),
                (h.id, loaded(second)),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        false,
    )
    .expect("prepared two-FE fixture")
}

fn prepared_joint_slopes(
    outcome: &[f64],
    regressor: &[f64],
    first: &[f64],
    second: &[f64],
) -> crate::PreparedOlsData {
    assert_eq!(regressor.len(), outcome.len());
    let z = source(4, "z");
    let w = source(5, "w");
    let slopes: Vec<f64> = (0..outcome.len()).map(|i| (i % 5) as f64 - 2.).collect();
    let x = source(1, "x");
    let g = source(2, "g");
    let h = source(3, "h");
    crate::prepare_model(
        &OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![x.clone(), g.clone(), h.clone(), z.clone(), w.clone()],
                terms: vec![continuous_term(1, x.id)],
            },
            absorbed_terms: vec![
                AbsorbedTerm {
                    id: AbsorbTermId::new(1).expect("g absorb id"),
                    intercept: true,
                    categorical_sources: vec![g.id],
                    slope_sources: vec![z.id],
                    label: "g".to_owned(),
                    saved_name: String::new(),
                },
                AbsorbedTerm {
                    id: AbsorbTermId::new(2).expect("h absorb id"),
                    intercept: true,
                    categorical_sources: vec![h.id],
                    slope_sources: vec![w.id],
                    label: "h".to_owned(),
                    saved_name: String::new(),
                },
            ],
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(x.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(g.id),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(h.id),
                },
                OlsInputBinding {
                    stata_var_position: 6,
                    role: OlsInputRole::NumericSource(z.id),
                },
                OlsInputBinding {
                    stata_var_position: 7,
                    role: OlsInputRole::NumericSource(w.id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        },
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(outcome),
            numeric_sources: vec![
                (x.id, loaded(regressor)),
                (g.id, loaded(first)),
                (h.id, loaded(second)),
                (z.id, loaded(&slopes)),
                (w.id, loaded(&slopes)),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        true,
    )
    .expect("prepared two-FE fixture")
}

#[test]
fn refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model() {
    for (hot, mode) in [
        (30.0_f64, PreparedOlsExecutionMode::Low),
        (30., PreparedOlsExecutionMode::Fast),
    ] {
        let mut x = Vec::new();
        let mut g = Vec::new();
        let mut h = Vec::new();
        let mut truth = Vec::new();
        let mut initial = Vec::new();
        for worker in 0..40 {
            for repeat in 0..4 {
                let firm = if worker < 8 {
                    (worker + repeat % 2) % 8
                } else {
                    8 + (worker - 8 + repeat % 2) % 32
                };
                let value = ((worker * 2 + repeat % 2) as f64 * 1.31).sin();
                x.push(value);
                g.push(worker as f64);
                h.push(firm as f64);
                let eta =
                    if worker < 8 { hot } else { 1.0 } + 0.2 * value + 0.3 * (firm as f64).cos();
                truth.push(eta);
                initial.push(
                    eta + if worker < 8 {
                        0.0
                    } else {
                        0.4 * (worker as f64).sin()
                    },
                );
            }
        }
        let y: Vec<_> = truth
            .iter()
            .enumerate()
            .map(|(row, eta)| eta.exp() * if row % 4 < 2 { 0.9 } else { 1.1 })
            .collect();
        let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
        let mut config = PreparedPpmlIrlsConfig::default();
        config.ppml.max_iterations = 6;
        config.memory.requested = if mode == PreparedOlsExecutionMode::Low {
            crate::options::MemoryMode::Low
        } else {
            crate::options::MemoryMode::Fast
        };
        config.memory.poolsize = 1;
        config.linear.execution.mode = mode;
        config.linear.execution.poolsize = 1;
        let start = PreparedPpmlStart {
            coefficient_values: vec![0.2, 0.0],
            eta: initial,
        };
        // Newer proposal paths can reach this known optimum without a
        // rejection. Exercise checkpoint refinement explicitly rather
        // than depending on an obsolete iteration trajectory.
        inject_strict_resume(true, false);
        let fitted = fit_prepared_ppml_irls(
            &prepared,
            &y,
            &vec![1.0; y.len()],
            &vec![0.0; y.len()],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
            Some(&start),
        );
        let fit = fitted.expect("refined ordinary solves must preserve strict progress");
        assert!(fit.diagnostics().converged);
        assert_eq!(fit.diagnostics().strict_profile_rejections, 1);
        assert!(
            relative_infinity_difference(fit.eta(), &truth) < 1e-10,
            "mode={mode:?} eta_error={:.17e}",
            relative_infinity_difference(fit.eta(), &truth)
        );
        assert!(fit.diagnostics().iterations <= 6);
    }
}
#[test]
fn canonical_final_preserves_accuracy_on_a_small_dynamic_range_model() {
    for (hot, mode) in [
        (30.0_f64, PreparedOlsExecutionMode::Low),
        (30., PreparedOlsExecutionMode::Fast),
    ] {
        let mut x = Vec::new();
        let mut g = Vec::new();
        let mut h = Vec::new();
        let mut truth = Vec::new();
        let mut initial = Vec::new();
        for worker in 0..40 {
            for repeat in 0..4 {
                let firm = if worker < 8 {
                    (worker + repeat % 2) % 8
                } else {
                    8 + (worker - 8 + repeat % 2) % 32
                };
                let value = ((worker * 2 + repeat % 2) as f64 * 1.31).sin();
                x.push(value);
                g.push(worker as f64);
                h.push(firm as f64);
                let eta =
                    if worker < 8 { hot } else { 1.0 } + 0.2 * value + 0.3 * (firm as f64).cos();
                truth.push(eta);
                initial.push(
                    eta + if worker < 8 {
                        0.0
                    } else {
                        0.4 * (worker as f64).sin()
                    },
                );
            }
        }
        let y: Vec<_> = truth
            .iter()
            .enumerate()
            .map(|(row, eta)| eta.exp() * if row % 4 < 2 { 0.9 } else { 1.1 })
            .collect();
        let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
        let mut config = PreparedPpmlIrlsConfig::default();
        config.ppml.max_iterations = 6;
        config.memory.requested = if mode == PreparedOlsExecutionMode::Low {
            crate::options::MemoryMode::Low
        } else {
            crate::options::MemoryMode::Fast
        };
        config.memory.poolsize = 1;
        config.linear.execution.mode = mode;
        config.linear.execution.poolsize = 1;
        let start = PreparedPpmlStart {
            coefficient_values: vec![0.2, 0.0],
            eta: initial,
        };
        let fitted = fit_prepared_ppml_irls(
            &prepared,
            &y,
            &vec![1.0; y.len()],
            &vec![0.0; y.len()],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
            Some(&start),
        );
        let fit = fitted.expect("refined ordinary solves must preserve strict progress");
        assert!(fit.diagnostics().converged);
        assert!(
            relative_infinity_difference(fit.eta(), &truth) < 1e-10,
            "mode={mode:?} eta_error={:.17e}",
            relative_infinity_difference(fit.eta(), &truth)
        );
        assert!(fit.diagnostics().iterations <= 6);
    }
}

fn prepared_three_fe(
    order: [usize; 3],
    zero_control: bool,
    nested_in: Option<usize>,
) -> (PreparedOlsData, Vec<f64>) {
    let rows = 240;
    let x_values = (0..rows)
        .map(|row| {
            if zero_control {
                0.0
            } else {
                (row as f64 * 0.173).sin() + (row as f64 * 0.071).cos()
            }
        })
        .collect::<Vec<_>>();
    let mut categorical = [
        (0..rows)
            .map(|row| (row % 24 + 1) as f64)
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((row * 5 + row / 24) % 9 + 1) as f64)
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((row * 7 + row / 11) % 4 + 1) as f64)
            .collect::<Vec<_>>(),
    ];
    if let Some(core) = nested_in {
        categorical[2] = categorical[core]
            .iter()
            .map(|value| 1.0 + ((*value - 1.0) / 3.0).floor())
            .collect();
    }
    let outcome = (0..rows)
        .map(|row| {
            1.0 + ((row * 17
                + categorical[0][row] as usize * 3
                + categorical[1][row] as usize * 5
                + categorical[2][row] as usize * 7)
                % 11) as f64
        })
        .collect::<Vec<_>>();
    let x = source(1, "x");
    let effects = [source(2, "large"), source(3, "medium"), source(4, "small")];
    let labels = ["large", "medium", "small"];
    let absorbed_terms = order
        .iter()
        .enumerate()
        .map(|(position, &effect)| AbsorbedTerm {
            id: AbsorbTermId::new(u32::try_from(position + 1).expect("small position"))
                .expect("absorb id"),
            intercept: true,
            categorical_sources: vec![effects[effect].id],
            slope_sources: Vec::new(),
            label: labels[effect].to_owned(),
            saved_name: format!("__fe_{}", labels[effect]),
        })
        .collect();
    let prepared = prepare_model(
        &OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![
                    x.clone(),
                    effects[0].clone(),
                    effects[1].clone(),
                    effects[2].clone(),
                ],
                terms: vec![continuous_term(1, x.id)],
            },
            absorbed_terms,
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(x.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(effects[0].id),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(effects[1].id),
                },
                OlsInputBinding {
                    stata_var_position: 6,
                    role: OlsInputRole::NumericSource(effects[2].id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        },
        &[],
        &LoadedOlsInput {
            candidate: vec![true; rows].into(),
            outcome: loaded(&outcome),
            numeric_sources: vec![
                (x.id, loaded(&x_values)),
                (effects[0].id, loaded(&categorical[0])),
                (effects[1].id, loaded(&categorical[1])),
                (effects[2].id, loaded(&categorical[2])),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        false,
    )
    .expect("prepared three-FE fixture");
    (prepared, outcome)
}

#[test]
fn explicit_three_fe_solver_roles_are_permutation_invariant_and_certified() {
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut reference_eta: Option<Vec<f64>> = None;
    let mut reference_mean: Option<Vec<f64>> = None;
    let mut reference_objective: Option<PoissonObjective> = None;

    for family in [
        super::super::PpmlSolverFamily::Lsmr,
        super::super::PpmlSolverFamily::Cmg,
    ] {
        for order in orders {
            let (prepared, outcome) = prepared_three_fe(order, false, None);
            let design = prepared.fit_owned_fe_design().expect("three-FE design");
            let selected = retained_effect_order_by_levels(design.effects().len(), |effect| {
                design.effects()[effect].n_levels()
            })
            .expect("effect role order");
            let selected_original = [order[selected[0]], order[selected[1]]];
            assert_eq!(selected_original, [0, 1]);

            let mut config = PreparedPpmlIrlsConfig {
                solver_plan: PpmlSolverPlan::new(family),
                ..PreparedPpmlIrlsConfig::default()
            };
            config.linear.execution.threads =
                crate::options::ThreadConfig::from_requested(2, true).expect("threads");
            config.ppml.absorb.lsmr.threads = config.linear.execution.threads;
            config.linear.absorb.lsmr.threads = config.linear.execution.threads;
            let prior_weights = vec![1.0; outcome.len()];
            let offset = vec![0.0; outcome.len()];
            let fit = fit_prepared_ppml_irls(
                &prepared,
                &outcome,
                &prior_weights,
                &offset,
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                    .expect("three-FE memory plan"),
                None,
            )
            .expect("permuted three-FE fit");
            assert!(fit.diagnostics().converged);
            assert!(fit.diagnostics().two_consecutive_convergence);
            assert!(fit.diagnostics().final_score.passed);
            assert_eq!(fit.diagnostics().solver.effective, family);
            assert_eq!(fit.diagnostics().solver.fallback_count, 0);

            if let (Some(eta), Some(mean), Some(objective)) =
                (&reference_eta, &reference_mean, reference_objective)
            {
                assert!(relative_infinity_difference(fit.eta(), eta) <= 2e-9);
                assert!(relative_infinity_difference(fit.mean(), mean) <= 2e-9);
                assert!(
                    (fit.objective().deviance - objective.deviance).abs()
                        <= 2e-9 * objective.deviance.abs().max(1.0)
                );
                assert!(
                    (fit.objective().log_likelihood - objective.log_likelihood).abs()
                        <= 2e-9 * objective.log_likelihood.abs().max(1.0)
                );
                assert!(fit.diagnostics().final_score.relative_infinity_norm <= 1e-8);
                assert!(fit.diagnostics().final_score.relative_two_norm <= 1e-8);
            } else {
                reference_eta = Some(fit.eta().to_vec());
                reference_mean = Some(fit.mean().to_vec());
                reference_objective = Some(fit.objective());
            }
        }
    }
}

#[test]
fn cmg_warm_seed_requires_complete_certification_and_survives_no_failed_frame() {
    use super::super::categorical_fwl_cmg as cmg;
    let (prepared, response) = prepared_three_fe([2, 0, 1], false, None);
    let design = prepared.fit_owned_fe_design().unwrap();
    let config = FusedFwlCmgConfig {
        map: crate::fe::MapConfig::default(),
        threads: crate::options::ThreadConfig::from_requested(2, true).unwrap(),
        batch_size: 2,
        solver_memory_budget: FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: u128::MAX,
        },
    };
    let mut engine = ReusableCategoricalFwlCmg::new(&prepared, &[0], &design, config).unwrap();
    let weights = vec![1.0; response.len()];
    for failure in 0..7 {
        assert_eq!(engine.warm_seed_ready(), (false, false));
        assert!(matches!(
            engine
                .solve(&prepared, &design, &response, &weights, 1e-8)
                .unwrap(),
            FusedFwlCmgSolveOutcome::Certified(_)
        ));
        assert_eq!(engine.warm_seed_ready(), (true, true));
        let expected = engine.expected_heap_peak(1024).unwrap();
        assert!(expected.expected_heap_bytes().unwrap() >= 1024);
        assert!(expected.expected_heap_bytes().unwrap() < 16 * 1024 * 1024);
        match failure {
            0 => {
                cmg::inject_projection_certificate_failure_once(0);
                let FusedFwlCmgSolveOutcome::Certified(report) = engine
                    .solve(&prepared, &design, &response, &weights, 1e-8)
                    .unwrap()
                else {
                    panic!("retry must certify")
                };
                assert_eq!(report.retried_right_hand_sides, 1);
            }
            1 => {
                cmg::inject_post_projection_numerical_failure_once();
                assert!(matches!(
                    engine
                        .solve(&prepared, &design, &response, &weights, 1e-8)
                        .unwrap(),
                    FusedFwlCmgSolveOutcome::NumericalFailure(_)
                ));
            }
            2..=4 => {
                let code = [
                    ErrorCode::Allocation,
                    ErrorCode::Interrupted,
                    ErrorCode::Internal,
                ][failure - 2];
                cmg::inject_fatal_failure_once(code);
                assert_eq!(
                    engine
                        .solve(&prepared, &design, &response, &weights, 1e-8)
                        .unwrap_err()
                        .code,
                    code
                );
            }
            5 => {
                let changed = prepared.fit_owned_fe_design().unwrap();
                assert_eq!(
                    engine
                        .solve(&prepared, &changed, &response, &weights, 1e-8)
                        .unwrap_err()
                        .code,
                    ErrorCode::InvalidArgument
                );
            }
            _ => engine.reset_warm_start(),
        }
        assert_eq!(
            engine.warm_seed_ready(),
            (false, false),
            "failed/retried/checkpoint frame cannot promote a seed"
        );
    }
    let replacement = ReusableCategoricalFwlCmg::new(&prepared, &[], &design, config).unwrap();
    assert_eq!(
        replacement.warm_seed_ready(),
        (false, false),
        "a replacement active basis starts cold"
    );
}

#[test]
fn cmg_nuisance_batches_preserve_weighted_fits_and_deterministic_certificates() {
    for zero_control in [false, true] {
        let (prepared, outcome) = prepared_three_fe([2, 0, 1], zero_control, None);
        let prior_weights = (0..outcome.len())
            .map(|row| 0.5 + (row % 7) as f64 / 3.0)
            .collect::<Vec<_>>();
        let offset = (0..outcome.len())
            .map(|row| (row as f64 * 0.03).sin() * 0.1)
            .collect::<Vec<_>>();
        let mut reference: Option<PreparedPpmlIrlsFit> = None;
        let mut deterministic: Option<PreparedPpmlIrlsFit> = None;
        for (poolsize, threads) in [(1, 1), (7, 1), (7, 2), (7, 7), (7, 7)] {
            let mut config = PreparedPpmlIrlsConfig {
                solver_plan: PpmlSolverPlan::new(super::super::PpmlSolverFamily::Cmg),
                ..PreparedPpmlIrlsConfig::default()
            };
            config.memory.poolsize = poolsize;
            config.linear.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            config.ppml.absorb.lsmr.threads = config.linear.execution.threads;
            config.linear.absorb.lsmr.threads = config.linear.execution.threads;
            let fit = fit_prepared_ppml_irls(
                &prepared,
                &outcome,
                &prior_weights,
                &offset,
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                None,
            )
            .expect("weighted CMG fit with a nuisance block wider than K+1");
            assert!(fit.diagnostics().converged);
            assert!(fit.diagnostics().two_consecutive_convergence);
            assert!(fit.diagnostics().final_score.passed);
            assert_eq!(fit.diagnostics().solver.fallback_count, 0);
            assert_eq!(
                fit.diagnostics().solver.effective,
                super::super::PpmlSolverFamily::Cmg
            );
            if let Some(reference) = &reference {
                assert!(relative_infinity_difference(fit.eta(), reference.eta()) <= 2e-9);
                assert!(relative_infinity_difference(fit.mean(), reference.mean()) <= 2e-9);
                assert!((fit.objective().deviance - reference.objective().deviance).abs() <= 1e-8);
            } else {
                reference = Some(fit);
                continue;
            }
            if threads == 7 {
                if let Some(previous) = &deterministic {
                    assert_eq!(fit.eta(), previous.eta());
                    assert_eq!(fit.mean(), previous.mean());
                    assert_eq!(fit.objective(), previous.objective());
                    assert_eq!(
                        fit.diagnostics().final_score,
                        previous.diagnostics().final_score
                    );
                    let report = &fit.diagnostics().solver.engines()[3];
                    let previous = &previous.diagnostics().solver.engines()[3];
                    assert_eq!(report.iterations, previous.iterations);
                    assert_eq!(report.retries, previous.retries);
                    assert_eq!(report.ordinary_frames, previous.ordinary_frames);
                }
                deterministic = Some(fit);
            }
        }
    }
}

#[test]
fn cmg_nested_nuisance_preserves_full_weighted_checkpoint_and_retry_certificates() {
    use super::super::PpmlSolverFamily;
    for core in [0, 1] {
        for zero_control in [false, true] {
            let mut reference: Option<PreparedPpmlIrlsFit> = None;
            for (order, family, retry) in [
                ([0, 1, 2], PpmlSolverFamily::Map, false),
                ([0, 1, 2], PpmlSolverFamily::Cmg, false),
                ([2, 0, 1], PpmlSolverFamily::Cmg, false),
                ([1, 2, 0], PpmlSolverFamily::Cmg, true),
            ] {
                let (prepared, outcome) = prepared_three_fe(order, zero_control, Some(core));
                let weights = (0..outcome.len())
                    .map(|row| 0.5 + (row % 7) as f64 / 3.0)
                    .collect::<Vec<_>>();
                let offset = (0..outcome.len())
                    .map(|row| 0.1 * (row as f64 / 17.0).sin())
                    .collect::<Vec<_>>();
                let mut config = PreparedPpmlIrlsConfig {
                    solver_plan: PpmlSolverPlan::new(family),
                    ..PreparedPpmlIrlsConfig::default()
                };
                config.linear.execution.threads =
                    crate::options::ThreadConfig::from_requested(2, true).unwrap();
                config.ppml.absorb.lsmr.threads = config.linear.execution.threads;
                config.linear.absorb.lsmr.threads = config.linear.execution.threads;
                if retry {
                    super::super::categorical_fwl_cmg::inject_projection_certificate_failure_once(
                        0,
                    );
                }
                let fit = fit_prepared_ppml_irls(
                    &prepared,
                    &outcome,
                    &weights,
                    &offset,
                    config,
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                    None,
                )
                .expect("nested CMG fit");
                assert!(fit.diagnostics().converged);
                assert!(fit.diagnostics().two_consecutive_convergence);
                assert!(fit.diagnostics().final_score.passed);
                assert_eq!(fit.diagnostics().solver.effective, family);
                assert_eq!(fit.diagnostics().solver.fallback_count, 0);
                if retry {
                    assert!(fit.diagnostics().solver.engines()[3].retries >= 1);
                }
                if let Some(reference) = &reference {
                    assert!(relative_infinity_difference(fit.eta(), reference.eta()) <= 2e-9);
                    assert!(relative_infinity_difference(fit.mean(), reference.mean()) <= 2e-9);
                    assert!(
                        (fit.objective().deviance - reference.objective().deviance).abs() <= 1e-8
                    );
                    assert!(
                        (fit.objective().log_likelihood - reference.objective().log_likelihood)
                            .abs()
                            <= 1e-8
                    );
                } else {
                    reference = Some(fit);
                }
            }
        }
    }
}

#[test]
fn cmg_nuisance_batch_memory_is_admitted_before_any_canonical_work() {
    let (prepared, outcome) = prepared_three_fe([2, 0, 1], true, None);
    let mut config = PreparedPpmlIrlsConfig {
        solver_plan: PpmlSolverPlan::new(super::super::PpmlSolverFamily::Cmg),
        ..PreparedPpmlIrlsConfig::default()
    };
    config.linear.execution.threads =
        crate::options::ThreadConfig::from_requested(7, true).unwrap();
    config.memory.poolsize = 1;
    let dimensions = memory_dimensions(&prepared, config);
    let rhs = dimensions.finite_columns + 1;
    let small_plan = plan_ppml_memory(dimensions, config.memory).unwrap();
    let small = public_solver_memory_admission(
        &prepared,
        small_plan,
        &config,
        rhs,
        PreparedPpmlIrlsPurpose::Publication,
    )
    .unwrap();
    config.memory.poolsize = 7;
    let mut plan = plan_ppml_memory(dimensions, config.memory).unwrap();
    let wide = public_solver_memory_admission(
        &prepared,
        plan,
        &config,
        rhs,
        PreparedPpmlIrlsPurpose::Publication,
    )
    .unwrap();
    assert!(wide.incremental_bytes > small.incremental_bytes);
    assert_eq!(small.cmg_batch_capacity, 1);
    assert_eq!(wide.cmg_batch_capacity, 3);
    let exact_peak = plan.selected_inventory().peak_bytes + wide.incremental_bytes;
    plan.usable_bytes = Some(exact_peak - 1);
    reset_canonical_frame_refresh_count();
    super::super::categorical_fwl_cmg::take_route_observation_preparations();
    let result = fit_prepared_ppml_irls(
        &prepared,
        &outcome,
        &vec![1.0; outcome.len()],
        &vec![0.0; outcome.len()],
        config,
        plan,
        None,
    );
    let error = match result {
        Ok(_) => panic!("enlarged nuisance pool escaped preflight"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("public PPML FE solver workspace"));
    assert_eq!(take_canonical_frame_refresh_count(), 0);
    assert_eq!(
        super::super::categorical_fwl_cmg::take_route_observation_preparations(),
        0
    );
    plan.usable_bytes = Some(exact_peak);
    assert!(plan.with_solver_workspace(wide.incremental_bytes).is_ok());
}

fn memory_dimensions<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    config: PreparedPpmlIrlsConfig,
) -> PpmlMemoryDimensions {
    let design = prepared.fit_owned_fe_design().expect("fixture FE design");
    let mut total_levels = 0_usize;
    let mut max_levels = 0_usize;
    let mut total_channels = 0_usize;
    let mut max_channels = 0_usize;
    let mut max_level_members = 0_usize;
    let mut directions = 0_usize;
    let mut max_directions = 0_usize;
    for effect in design.effects() {
        total_levels = total_levels
            .checked_add(effect.n_levels())
            .expect("small fixture levels");
        max_levels = max_levels.max(effect.n_levels());
        total_channels = total_channels
            .checked_add(effect.channels())
            .expect("small fixture channels");
        max_channels = max_channels.max(effect.channels());
        for level in 0..effect.n_levels() {
            max_level_members = max_level_members.max(effect.level_index().members(level).len());
        }
        let effect_directions = effect
            .n_levels()
            .checked_mul(effect.channels())
            .expect("small fixture directions");
        directions = directions
            .checked_add(effect_directions)
            .expect("small fixture directions");
        max_directions = max_directions.max(effect_directions);
    }
    PpmlMemoryDimensions {
        metadata: Default::default(),
        observations: prepared.fit_sample().n_active(),
        original_observations: prepared.fit_sample().n_original,
        candidate_observations: prepared.fit_sample().n_candidate,
        loaded_numeric_columns: prepared.fit_sample().regressors.len(),
        retained_base_adjustment_capacity_elements: 0,
        retained_raw_preseparation_probability_weight_elements: 0,
        finite_columns: prepared.fit_model().columns.len(),
        coefficient_slots: prepared.fit_model().coefficients.len(),
        reported_coefficient_slots: prepared.fit_model().coefficients.len(),
        fe_dimensions: design.effects().len(),
        fe_total_levels: total_levels,
        fe_max_levels: max_levels,
        fe_total_channels: total_channels,
        fe_max_channels: max_channels,
        fe_max_level_members: max_level_members,
        fe_coefficient_directions: directions,
        fe_max_effect_directions: max_directions,
        cluster_dimensions: 0,
        cluster_subset_groups: 0,
        separation_workspace_vectors: 0,
        separation_singleton_enabled: false,
        separation_method_count: 0,
        separation_generation_capacity: 1,
        separation_rebase_enabled: false,
        separation_exact_fe_enabled: false,
        separation_mu_enabled: false,
        simplex_tableau_cells: 0,
        requested_output_columns: 4,
        requested_mean_output: false,
        requested_saved_fe_channels: 0,
        publication_matrix_cells: prepared.fit_model().coefficients.len().saturating_pow(2),
        threads: config.linear.execution.threads.resolved(),
        allocation_audit_enabled: config.allocation_snapshot_probe.is_some(),
    }
}

#[test]
fn initial_checkpoint_is_structural_and_preserves_explicit_solver_purpose() {
    let mut config = PreparedPpmlIrlsConfig::default();
    config.linear.execution.mode = PreparedOlsExecutionMode::Fast;
    for family in [
        super::super::PpmlSolverFamily::Lsmr,
        super::super::PpmlSolverFamily::Cmg,
    ] {
        config.solver_plan = PpmlSolverPlan::new(family);
        assert!(use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::Publication,
            PARALLEL_PROJECTION_MIN_ROWS,
            2,
        ));
        assert!(use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::Publication,
            PARALLEL_PROJECTION_MIN_ROWS,
            3,
        ));
        assert!(use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::Publication,
            PARALLEL_PROJECTION_MIN_ROWS - 1,
            3,
        ));
        for observations in [1, 400, 8_000, 10_000] {
            assert!(use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                observations,
                3,
            ));
        }
        assert!(!use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::Publication,
            0,
            3,
        ));
        assert!(!use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::Publication,
            PARALLEL_PROJECTION_MIN_ROWS,
            4,
        ));
        assert!(!use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-8 },
            PARALLEL_PROJECTION_MIN_ROWS,
            3,
        ));
        config.linear.execution.mode = PreparedOlsExecutionMode::Low;
        assert!(use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::Publication,
            PARALLEL_PROJECTION_MIN_ROWS,
            3,
        ));
        config.linear.execution.mode = PreparedOlsExecutionMode::Fast;
    }
    for family in [
        super::super::PpmlSolverFamily::Auto,
        super::super::PpmlSolverFamily::Map,
    ] {
        config.solver_plan = PpmlSolverPlan::new(family);
        assert!(!use_initial_checkpoint_lsmr_accelerator(
            &config,
            PreparedPpmlIrlsPurpose::Publication,
            PARALLEL_PROJECTION_MIN_ROWS,
            3,
        ));
    }
}

static STABLE_AUDIT_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);
static CHURN_AUDIT_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);

fn stable_audit_probe() -> PpmlAllocatorSnapshot {
    STABLE_AUDIT_PROBE_CALLS.fetch_add(1, Ordering::SeqCst);
    PpmlAllocatorSnapshot {
        current_bytes: 16_384,
        peak_bytes: 32_768,
        allocation_events: 41,
    }
}

fn churn_audit_probe() -> PpmlAllocatorSnapshot {
    let call = CHURN_AUDIT_PROBE_CALLS.fetch_add(1, Ordering::SeqCst);
    if call == 0 {
        PpmlAllocatorSnapshot {
            current_bytes: 16_384,
            peak_bytes: 32_768,
            allocation_events: 41,
        }
    } else {
        // Simulate one successful allocate/free pair inside the measured
        // solves: current bytes return to baseline, while the monotone
        // event counter exposes the otherwise invisible churn.
        PpmlAllocatorSnapshot {
            current_bytes: 16_384,
            peak_bytes: 32_768,
            allocation_events: 42,
        }
    }
}

fn allocation_audited_fit(probe: PpmlAllocatorSnapshotProbe) -> PreparedPpmlIrlsFit {
    let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
    let eta = x
        .iter()
        .map(|value| 0.35_f64 + 0.42_f64 * value)
        .collect::<Vec<_>>();
    let outcome = eta
        .iter()
        .enumerate()
        .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
        .collect::<Vec<_>>();
    let prepared = prepared(&outcome, &[x]);
    let config = PreparedPpmlIrlsConfig {
        ppml: PpmlConfig {
            max_iterations: 100,
            ..PpmlConfig::default()
        },
        allocation_snapshot_probe: Some(probe),
        ..PreparedPpmlIrlsConfig::default()
    };
    fit_prepared_ppml_irls(
        &prepared,
        &outcome,
        &[1.0; 8],
        &[0.0; 8],
        config,
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
            .expect("allocation-audit memory plan"),
        None,
    )
    .expect("allocation-audited prepared PPML")
}

#[test]
fn complete_warmed_map_window_certifies_stable_fake_allocator() {
    STABLE_AUDIT_PROBE_CALLS.store(0, Ordering::SeqCst);
    let fitted = allocation_audited_fit(stable_audit_probe);
    let certificate = fitted
        .diagnostics()
        .memory
        .steady_state_allocation_certificate
        .expect("steady-state allocation certificate");
    assert_eq!(STABLE_AUDIT_PROBE_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(
        certificate.observation.measured_iterations,
        PPML_STEADY_STATE_AUDIT_ITERATIONS
    );
    assert_eq!(certificate.allocation_events, 0);
    assert!(certificate.workspace_capacity_stable);
    assert!(certificate.linear_workspace_capacity_stable);
    assert!(certificate.scratch_stable);
    assert!(certificate.observation.linear_session_allocation_free);
    assert!(certificate.observation.fresh_warm_equivalent);
    assert!(certificate.passed);
    assert!(fitted.diagnostics().memory.whole_iteration_allocation_free);
}

#[test]
fn allocator_event_inside_warmed_solve_fails_even_with_zero_net_growth() {
    CHURN_AUDIT_PROBE_CALLS.store(0, Ordering::SeqCst);
    let fitted = allocation_audited_fit(churn_audit_probe);
    let certificate = fitted
        .diagnostics()
        .memory
        .steady_state_allocation_certificate
        .expect("steady-state allocation certificate");
    assert_eq!(CHURN_AUDIT_PROBE_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(certificate.allocation_events, 1);
    assert_eq!(certificate.allocator_growth_bytes(), 0);
    assert_eq!(certificate.allocator_peak_growth_bytes, 0);
    assert!(!certificate.passed);
    assert!(!fitted.diagnostics().memory.whole_iteration_allocation_free);
}

#[test]
fn direct_first_partition_start_matches_independent_offset_poisson_truth() {
    for dimensions in [2, 3, 4] {
        let n = 2 * 2 * 3 * 2 * 2;
        let codes: Vec<Vec<u32>> = (0..dimensions)
            .map(|d| {
                (0..n)
                    .map(|r| ((r / [2, 4, 12, 24][d]) % [2, 3, 2, 2][d]) as u32)
                    .collect()
            })
            .collect();
        let offset: Vec<_> = (0..n)
            .map(|r| if codes[0][r] == 0 { 1000.0 } else { -1000.0 } + 0.05 * codes[1][r] as f64)
            .collect();
        let truth: Vec<_> = (0..n)
            .map(|r| {
                (0.05 * codes[1][r] as f64
                    + (0..dimensions)
                        .map(|d| 0.2 * (d + 1) as f64 * codes[d][r] as f64)
                        .sum::<f64>())
                .exp()
            })
            .collect();
        // Paired outcome deviations give zero score for every explicit
        // intercept and slope, independently specifying the Poisson optimum.
        let y: Vec<_> = truth
            .iter()
            .enumerate()
            .map(|(r, m)| m * if r % 2 == 0 { 0.8 } else { 1.2 })
            .collect();
        let prepared = prepared(&y, &[]);
        for slopes in [false, true] {
            let effects = codes
                .iter()
                .enumerate()
                .map(|(d, c)| {
                    crate::fe::EffectTerm::new(
                        c.clone(),
                        [2, 3, 2, 2][d],
                        true,
                        if slopes {
                            vec![
                                (0..n).map(|r| (0.37 * (r / 2) as f64).sin()).collect(),
                                vec![1.; n],
                            ]
                        } else {
                            vec![]
                        },
                    )
                    .unwrap()
                })
                .collect();
            let design = FeDesign::new(n, effects).unwrap();
            for scale in [1e-150, 1., 1e150] {
                let q: Vec<_> = (0..n)
                    .map(|r| {
                        scale
                            * (0..dimensions)
                                .map(|d| 1.0 + 0.25 * codes[d][r] as f64)
                                .product::<f64>()
                    })
                    .collect();
                let (beta, eta) = initialize(
                    &prepared,
                    &y,
                    &q,
                    &offset,
                    &design,
                    PreparedPpmlIrlsConfig::default(),
                    None,
                    PreparedPpmlIrlsPurpose::Publication,
                )
                .unwrap();
                assert!(
                    beta.iter().all(|b| *b == 0.),
                    "absorbed initial constant has a zero gauge"
                );
                for (value, expected) in eta.iter().zip(&truth) {
                    assert!((value.exp() - expected).abs() <= 1e-11 * (1.0 + expected));
                }
                for effect in design.effects() {
                    for level in 0..effect.n_levels() {
                        let score: f64 = effect
                            .level_index()
                            .members(level)
                            .iter()
                            .map(|&r| q[r] / scale * (y[r] - eta[r].exp()))
                            .sum();
                        assert!(score.abs() < 1e-9);
                    }
                }
            }
        }
    }
}

#[test]
fn direct_first_partition_refusal_restores_global_after_later_partition_failure() {
    let y = [1., 2., 0., 0., 3., 4., 0., 0.];
    let q = [1., 2., 3., 4., 5., 6., 7., 8.];
    let offset = [0.2, -0.3, 0.7, -0.1, 0.4, -0.5, 0.1, 0.9];
    let design = FeDesign::new(
        8,
        vec![
            crate::fe::EffectTerm::new(vec![0, 0, 0, 0, 1, 1, 1, 1], 2, true, vec![]).unwrap(),
            crate::fe::EffectTerm::new(vec![0, 0, 1, 1, 0, 0, 1, 1], 2, true, vec![]).unwrap(),
        ],
    )
    .unwrap();
    let mut eta = [0.; 8];
    assert!(seed_group_intercept_predictor(
        &y,
        &q,
        &offset,
        &design.effects()[0],
        PpmlConfig::default(),
        &mut eta
    ));
    assert!(!seed_multi_effect_predictor(
        &y,
        &q,
        &offset,
        &design,
        PpmlConfig::default(),
        &mut eta
    ));
    let prepared = prepared(&y, &[]);
    let run = |purpose| {
        initialize(
            &prepared,
            &y,
            &q,
            &offset,
            &design,
            PreparedPpmlIrlsConfig::default(),
            None,
            purpose,
        )
        .unwrap()
    };
    assert_eq!(
        run(PreparedPpmlIrlsPurpose::Publication),
        run(PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 })
    );
}

#[test]
fn stable_initialization_handles_large_offsets() {
    let outcome = [0.0, 2.0, 8.0];
    let weights = [1.0, 3.0, 0.5];
    let offset = [700.0, 699.0, 698.0];
    let shift = log_weighted_outcome_sum(&outcome, &weights).expect("outcome")
        - log_weighted_offset_sum(&offset, &weights).expect("offset");
    assert!(shift.is_finite());
    let eta = offset.map(|value| value + shift);
    let mut means = [0.0; 3];
    means_from_eta(&eta, super::super::PoissonBounds::default(), &mut means)
        .expect("representable initialized means");
    assert!(means.iter().all(|mean| mean.is_finite() && *mean > 0.0));
}

#[test]
fn one_effect_seed_is_group_optimal_weighted_stable_and_deterministic() {
    let design = FeDesign::new(
        6,
        vec![
            crate::fe::EffectTerm::new(vec![0, 0, 0, 1, 1, 1], 2, true, Vec::new())
                .expect("effect"),
        ],
    )
    .expect("design");
    let outcome = [0.0, 2.0, 5.0, 3.0, 0.5, 9.0];
    let weights = [0.25, 4.0, 2.0, 3.0, 1.0, 0.5];
    let offset = [1000.0, 1000.25, 999.8, -1000.0, -1000.1, -999.6];
    let mut eta = [0.0; 6];
    assert!(seed_one_effect_predictor(
        &outcome,
        &weights,
        &offset,
        &design,
        PpmlConfig::default(),
        &mut eta
    ));
    for group in [0..3, 3..6] {
        let score: f64 = group
            .clone()
            .map(|row| weights[row] * (outcome[row] - eta[row].exp()))
            .sum();
        assert!(score.abs() < 1e-11, "group score {score}");
        let alpha = eta[group.start] - offset[group.start];
        for row in group {
            assert!((eta[row] - offset[row] - alpha).abs() < 3e-13);
        }
    }
    let mut repeated = [0.0; 6];
    assert!(seed_one_effect_predictor(
        &outcome,
        &weights,
        &offset,
        &design,
        PpmlConfig::default(),
        &mut repeated
    ));
    assert_eq!(eta, repeated);
    let scaled = weights.map(|value| 7.5 * value);
    assert!(seed_one_effect_predictor(
        &outcome,
        &scaled,
        &offset,
        &design,
        PpmlConfig::default(),
        &mut repeated
    ));
    assert!(relative_infinity_difference(&eta, &repeated) < 1e-12);
}

#[test]
fn multi_effect_seed_matches_separable_poisson_truth_and_weight_scaling() {
    for dimensions in [2, 3] {
        let n = 4 * 5 * 3 * 2;
        let codes: Vec<Vec<u32>> = (0..dimensions)
            .map(|d| {
                (0..n)
                    .map(|r| match d {
                        0 => (r / 2) % 4,
                        1 => (r / 8) % 5,
                        _ => (r / 40) % 3,
                    } as u32)
                    .collect()
            })
            .collect();
        let effects = codes
            .iter()
            .enumerate()
            .map(|(d, c)| {
                crate::fe::EffectTerm::new(c.clone(), [4, 5, 3][d], true, Vec::new()).unwrap()
            })
            .collect();
        let design = FeDesign::new(n, effects).unwrap();
        let truth: Vec<f64> = (0..n)
            .map(|r| {
                (0..dimensions)
                    .map(|d| 0.2 * (d + 1) as f64 * codes[d][r] as f64)
                    .sum::<f64>()
                    .exp()
            })
            .collect();
        // Paired deviations have zero weighted score for every explicit
        // dummy. The independent optimum is the prescribed product mean.
        let outcome: Vec<_> = truth
            .iter()
            .enumerate()
            .map(|(r, m)| m * if r % 2 == 0 { 0.8 } else { 1.2 })
            .collect();
        for scale in [1e-50, 1.0, 1e50] {
            let weights: Vec<_> = (0..n)
                .map(|r| {
                    scale
                        * (0..dimensions)
                            .map(|d| 1.0 + 0.25 * codes[d][r] as f64)
                            .product::<f64>()
                })
                .collect();
            let total: f64 = outcome.iter().zip(&weights).map(|(y, w)| y * w).sum();
            let initial = (total / weights.iter().sum::<f64>()).ln();
            let mut eta = vec![initial; n];
            assert!(seed_multi_effect_predictor(
                &outcome,
                &weights,
                &vec![0.; n],
                &design,
                PpmlConfig::default(),
                &mut eta
            ));
            for (value, expected) in eta.iter().zip(&truth) {
                assert!((value.exp() - expected).abs() <= 1e-12 * (1.0 + expected));
            }
        }
    }
}

#[test]
fn profiled_slope_seed_matches_analytic_newton_step_and_scaling() {
    let members = [0, 1, 2, 3];
    let outcome = [1., 9., 1., 9.];
    for slope_scale in [1e-50, 1., 1e50] {
        for weight_scale in [1e-50, 1., 1e50] {
            let slope = [-slope_scale, slope_scale, -slope_scale, slope_scale];
            let weights = [weight_scale; 4];
            let mut eta = [5.0_f64.ln(); 4];
            assert!(seed_profiled_slope(
                &members,
                &slope,
                &outcome,
                &weights,
                20. * weight_scale,
                PpmlConfig::default(),
                &mut eta
            ));
            // At beta=0, the normalized slope score/curvature is 16/20.
            // Profiling the intercept sets the mean back to five.
            let b = 0.8_f64;
            let intercept = 5.0_f64.ln() - b.cosh().ln();
            // Keep the legacy matrix indices and numerical traversal order explicit.
            #[allow(clippy::needless_range_loop)]
            for r in 0..4 {
                let truth = intercept + if r % 2 == 0 { -b } else { b };
                assert!((eta[r] - truth).abs() < 1e-12);
            }
            let objective = |e: &[f64]| (0..4).map(|r| outcome[r] * e[r] - e[r].exp()).sum::<f64>();
            assert!(objective(&eta) > objective(&[5.0_f64.ln(); 4]));
        }
    }
}

#[test]
fn slope_intercept_seed_matches_independent_poisson_truth() {
    for dimensions in [2, 3] {
        let n = 4 * 5 * 3 * 2;
        let codes: Vec<Vec<u32>> = (0..dimensions)
            .map(|d| {
                (0..n)
                    .map(|r| match d {
                        0 => (r / 2) % 4,
                        1 => (r / 8) % 5,
                        _ => (r / 40) % 3,
                    } as u32)
                    .collect()
            })
            .collect();
        // Every explicit intercept/slope is constant within a replica pair.
        // Paired outcome deviations cancel its weighted score exactly.
        let slopes: Vec<_> = (0..n).map(|r| (0.37 * (r / 2) as f64).sin()).collect();
        let effects = codes
            .iter()
            .enumerate()
            .map(|(d, c)| {
                crate::fe::EffectTerm::new(
                    c.clone(),
                    [4, 5, 3][d],
                    true,
                    vec![
                        slopes.clone(),
                        slopes.iter().map(|z| z * 1e6).collect(),
                        vec![0.0; n],
                    ],
                )
                .unwrap()
            })
            .collect();
        let design = FeDesign::new(n, effects).unwrap();
        let truth: Vec<f64> = (0..n)
            .map(|r| {
                (0..dimensions)
                    .map(|d| 0.2 * (d + 1) as f64 * codes[d][r] as f64)
                    .sum::<f64>()
                    .exp()
            })
            .collect();
        let outcome: Vec<_> = truth
            .iter()
            .enumerate()
            .map(|(r, m)| m * if r % 2 == 0 { 0.8 } else { 1.2 })
            .collect();
        for scale in [1e-50, 1.0, 1e50] {
            let weights: Vec<_> = (0..n)
                .map(|r| {
                    scale
                        * (0..dimensions)
                            .map(|d| 1.0 + 0.25 * codes[d][r] as f64)
                            .product::<f64>()
                })
                .collect();
            let total: f64 = outcome.iter().zip(&weights).map(|(y, w)| y * w).sum();
            let initial = (total / weights.iter().sum::<f64>()).ln();
            let mut eta = vec![initial; n];
            assert!(seed_multi_effect_predictor(
                &outcome,
                &weights,
                &vec![0.; n],
                &design,
                PpmlConfig::default(),
                &mut eta
            ));
            for (value, expected) in eta.iter().zip(&truth) {
                assert!((value.exp() - expected).abs() <= 1e-12 * (1.0 + expected));
            }
        }
    }
}

#[test]
fn slope_intercept_seed_preserves_offsets_warm_starts_and_full_refusal() {
    let make_design = |intercept| {
        FeDesign::new(
            8,
            vec![
                crate::fe::EffectTerm::new(
                    vec![0, 0, 0, 0, 1, 1, 1, 1],
                    2,
                    true,
                    vec![vec![0., 0., 2., 2., 0., 0., 2., 2.]],
                )
                .unwrap(),
                crate::fe::EffectTerm::new(
                    vec![0, 0, 1, 1, 0, 0, 1, 1],
                    2,
                    intercept,
                    vec![vec![1., 1., 0., 0., 1., 1., 0., 0.], vec![0.; 8]],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    };
    let design = make_design(true);
    let weights = [1., 2., 3., 4., 5., 6., 7., 8.];
    let offset = [0.2, -0.3, 0.7, -0.1, 0.4, -0.5, 0.1, 0.9];
    let outcome = [1., 3., 4., 2., 7., 5., 8., 9.];
    let prepared = prepared(&outcome, &[]);
    let config = PreparedPpmlIrlsConfig::default();
    let initialize_as = |purpose, start| {
        initialize(
            &prepared, &outcome, &weights, &offset, &design, config, start, purpose,
        )
        .unwrap()
    };
    let mu = PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 };
    let (global_coefficients, global) = initialize_as(mu, None);
    let (_, publication) = initialize_as(PreparedPpmlIrlsPurpose::Publication, None);
    let objective = |e: &[f64]| {
        (0..8)
            .map(|r| weights[r] * (outcome[r] * e[r] - e[r].exp()))
            .sum::<f64>()
    };
    assert!(objective(&publication) > objective(&global));
    for group in [vec![0, 1, 4, 5], vec![2, 3, 6, 7]] {
        let score: f64 = group
            .iter()
            .map(|&r| weights[r] * (outcome[r] - publication[r].exp()))
            .sum();
        assert!(score.abs() < 1e-11);
    }
    for r in (0..8).step_by(2) {
        assert!(
            ((publication[r] - offset[r]) - (publication[r + 1] - offset[r + 1])).abs() < 1e-14
        );
    }
    let start = PreparedPpmlStart {
        coefficient_values: global_coefficients,
        eta: global.clone(),
    };
    let (coefficients, warm) = initialize_as(PreparedPpmlIrlsPurpose::Publication, Some(&start));
    assert_eq!(coefficients, start.coefficient_values);
    assert_eq!(warm, start.eta);
    let mut refused = global.clone();
    assert!(!seed_multi_effect_predictor(
        &outcome,
        &weights,
        &offset,
        &make_design(false),
        config.ppml,
        &mut refused
    ));
    assert_eq!(
        refused, global,
        "a slope-only partition retains the prior route"
    );
    for y in [[1., 2., 3., 4., 0., 0., 0., 0.], [2.; 8]] {
        let mut limited = config;
        if y[4] > 0.0 {
            limited.ppml.bounds.eta_max = 0.01;
        }
        let run = |purpose| {
            initialize(
                &prepared, &y, &weights, &offset, &design, limited, None, purpose,
            )
            .unwrap()
            .1
        };
        assert_eq!(run(PreparedPpmlIrlsPurpose::Publication), run(mu));
    }
    let mut eta = global;
    assert!(!seed_multi_effect_predictor(
        &[2.; 8],
        &[1e308; 8],
        &offset,
        &design,
        config.ppml,
        &mut eta
    ));
}

#[test]
fn multi_effect_seed_improves_objective_and_refusal_restores_global_start() {
    let design = FeDesign::new(
        8,
        vec![
            crate::fe::EffectTerm::new(vec![0, 0, 0, 0, 1, 1, 1, 1], 2, true, vec![]).unwrap(),
            crate::fe::EffectTerm::new(vec![0, 0, 1, 1, 0, 0, 1, 1], 2, true, vec![]).unwrap(),
        ],
    )
    .unwrap();
    let weights = [1., 2., 3., 4., 5., 6., 7., 8.];
    let offset = [0.2, -0.3, 0.7, -0.1, 0.4, -0.5, 0.1, 0.9];
    let outcome = [1., 3., 4., 2., 7., 5., 8., 9.];
    let mut eta = offset;
    let objective = |e: &[f64]| {
        (0..8)
            .map(|r| weights[r] * (outcome[r] * e[r] - e[r].exp()))
            .sum::<f64>()
    };
    let before = objective(&eta);
    assert!(seed_multi_effect_predictor(
        &outcome,
        &weights,
        &offset,
        &design,
        PpmlConfig::default(),
        &mut eta
    ));
    assert!(objective(&eta) > before);
    for group in [vec![0, 1, 4, 5], vec![2, 3, 6, 7]] {
        let score: f64 = group
            .iter()
            .map(|&r| weights[r] * (outcome[r] - eta[r].exp()))
            .sum();
        assert!(score.abs() < 1e-11);
    }
    // Each replica within a cell differs only by its original offset.
    for r in (0..8).step_by(2) {
        assert!(((eta[r] - offset[r]) - (eta[r + 1] - offset[r + 1])).abs() < 1e-14);
    }
    let outcome = [1., 2., 3., 4., 0., 0., 0., 0.];
    let prepared = prepared(&outcome, &[]);
    let config = PreparedPpmlIrlsConfig::default();
    let (_, global) = initialize(
        &prepared,
        &outcome,
        &weights,
        &offset,
        &design,
        config,
        None,
        PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 },
    )
    .unwrap();
    let (_, publication) = initialize(
        &prepared,
        &outcome,
        &weights,
        &offset,
        &design,
        config,
        None,
        PreparedPpmlIrlsPurpose::Publication,
    )
    .unwrap();
    assert_eq!(publication, global);
    let mut unusable = [0.; 8];
    assert!(!seed_multi_effect_predictor(
        &[2.; 8],
        &[1e308; 8],
        &[0.; 8],
        &design,
        config.ppml,
        &mut unusable
    ));
    let mut bounded = PpmlConfig::default();
    bounded.bounds.eta_max = 0.01;
    assert!(!seed_multi_effect_predictor(
        &[2.; 8],
        &[1.; 8],
        &[0.; 8],
        &design,
        bounded,
        &mut unusable
    ));
}

#[test]
fn unusable_one_effect_seed_restores_global_start_and_mu_keeps_global_start() {
    let design = FeDesign::new(
        4,
        vec![crate::fe::EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("effect")],
    )
    .expect("design");
    let offset = [0.0; 4];
    let weights = [1.0; 4];
    for outcome in [[2.0, 4.0, 0.0, 0.0], [2.0, 4.0, 1.0, 9.0]] {
        let prepared = prepared(&outcome, &[]);
        let config = PreparedPpmlIrlsConfig::default();
        let (_, global) = initialize(
            &prepared,
            &outcome,
            &weights,
            &offset,
            &design,
            config,
            None,
            PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 },
        )
        .expect("Mu start");
        let expected = (outcome.iter().sum::<f64>() / 4.0).ln();
        assert!(global.iter().all(|value| (*value - expected).abs() < 1e-14));
        let (_, publication) = initialize(
            &prepared,
            &outcome,
            &weights,
            &offset,
            &design,
            config,
            None,
            PreparedPpmlIrlsPurpose::Publication,
        )
        .expect("publication start");
        if outcome[2] == 0.0 {
            assert_eq!(
                publication, global,
                "zero-outcome group cannot produce a finite seed"
            );
        } else {
            assert_ne!(publication, global);
        }
    }
}

#[test]
fn constant_offset_seed_uses_weighted_means_and_falls_back_on_raw_overflow() {
    let members = [0, 1];
    assert_eq!(
        constant_offset_group_log_mean(&members, &[2.0, 4.0], &[3.0, 1.0], &[1000.0; 2]),
        Some(2.5_f64.ln()),
    );
    for (outcome, weights, offset) in [
        ([2.0, 4.0], [1e308; 2], [1000.0; 2]),
        ([1e-100; 2], [1e-300; 2], [0.0; 2]),
        ([2.0, 4.0], [1.0; 2], [0.0, 1.0]),
        ([0.0; 2], [1.0; 2], [0.0; 2]),
    ] {
        assert!(constant_offset_group_log_mean(&members, &outcome, &weights, &offset).is_none());
    }
    let design = FeDesign::new(
        2,
        vec![crate::fe::EffectTerm::new(vec![0, 0], 1, true, Vec::new()).unwrap()],
    )
    .unwrap();
    let mut eta = [0.0; 2];
    assert!(seed_one_effect_predictor(
        &[2.0, 4.0],
        &[1e308; 2],
        &[1000.0; 2],
        &design,
        PpmlConfig::default(),
        &mut eta,
    ));
    assert!(eta
        .iter()
        .all(|value| (*value - 3.0_f64.ln()).abs() < 1e-12));
}

#[test]
fn one_effect_seed_does_not_admit_slopes_or_multiple_effects() {
    let codes = vec![0, 0, 1, 1];
    let intercept = crate::fe::EffectTerm::new(codes.clone(), 2, true, Vec::new()).expect("effect");
    let slope =
        crate::fe::EffectTerm::new(codes, 2, true, vec![vec![0.0, 1.0, 2.0, 3.0]]).expect("slope");
    for effects in [Vec::new(), vec![slope], vec![intercept.clone(), intercept]] {
        let design = FeDesign::new(4, effects).expect("design");
        let mut eta = [9.0; 4];
        assert!(!seed_one_effect_predictor(
            &[1.0; 4],
            &[1.0; 4],
            &[0.0; 4],
            &design,
            PpmlConfig::default(),
            &mut eta
        ));
        assert_eq!(eta, [9.0; 4]);
    }
}

#[test]
fn line_search_reuses_buffers_and_rejects_overflow() {
    let dimensions = PpmlMemoryDimensions {
        metadata: Default::default(),
        observations: 2,
        original_observations: 2,
        candidate_observations: 2,
        loaded_numeric_columns: 0,
        retained_base_adjustment_capacity_elements: 0,
        retained_raw_preseparation_probability_weight_elements: 0,
        finite_columns: 1,
        coefficient_slots: 1,
        reported_coefficient_slots: 1,
        fe_dimensions: 0,
        fe_total_levels: 0,
        fe_max_levels: 0,
        fe_total_channels: 0,
        fe_max_channels: 0,
        fe_max_level_members: 0,
        fe_coefficient_directions: 0,
        fe_max_effect_directions: 0,
        cluster_dimensions: 0,
        cluster_subset_groups: 0,
        separation_workspace_vectors: 0,
        separation_singleton_enabled: false,
        separation_method_count: 0,
        separation_generation_capacity: 1,
        separation_rebase_enabled: false,
        separation_exact_fe_enabled: false,
        separation_mu_enabled: false,
        simplex_tableau_cells: 0,
        requested_output_columns: 0,
        requested_mean_output: false,
        requested_saved_fe_channels: 0,
        publication_matrix_cells: 1,
        threads: 1,
        allocation_audit_enabled: false,
    };
    let plan = plan_ppml_memory(dimensions, PpmlMemoryConfig::default()).expect("plan");
    let mut workspace = PpmlIterationWorkspace::from_initial_state(plan, vec![0.0, 0.0], vec![0.0])
        .expect("workspace");
    workspace
        .refresh_working_state(
            &[1.0, 1.0],
            &[1.0, 1.0],
            &[0.0, 0.0],
            PpmlConfig::default().bounds,
        )
        .expect("working state");
    workspace
        .set_candidate_eta(&[1_000.0, -1_000.0])
        .expect("candidate eta");
    workspace
        .candidate_coefficients_mut()
        .copy_from_slice(&[1.0]);
    let objective =
        poisson_objective_from_eta(&[1.0, 1.0], workspace.eta(), workspace.mean(), &[1.0, 1.0])
            .expect("objective");
    let accepted = workspace
        .accept_candidate(&[1.0, 1.0], &[1.0, 1.0], objective, PpmlConfig::default())
        .expect_err("no improving extreme direction");
    assert_eq!(accepted.code, ErrorCode::NonConvergence);
}

fn injected_warm_failure_fit(
    injection: Option<FrozenLinearFailureInjection>,
) -> Result<PreparedPpmlIrlsFit> {
    let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
    let eta = x
        .iter()
        .map(|value| 0.35_f64 + 0.42_f64 * value)
        .collect::<Vec<_>>();
    let outcome = eta
        .iter()
        .enumerate()
        .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
        .collect::<Vec<_>>();
    let prepared = prepared(&outcome, &[x]);
    let config = PreparedPpmlIrlsConfig::default();
    if let Some(injection) = injection {
        inject_frozen_linear_failure(injection);
    }
    fit_prepared_ppml_irls(
        &prepared,
        &outcome,
        &[1.0; 8],
        &[0.0; 8],
        config,
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)?,
        None,
    )
}

#[test]
fn invalid_warm_projection_falls_back_to_independently_certified_canonical_solve() {
    let fitted = injected_warm_failure_fit(Some(
        FrozenLinearFailureInjection::RecoverableWarmProjection,
    ))
    .expect("canonical fallback fit");
    let baseline = injected_warm_failure_fit(None).expect("baseline fit");
    assert!(fitted.diagnostics().converged);
    assert!(fitted.diagnostics().final_score.passed);
    assert_eq!(fitted.diagnostics().memory.fallback_count, 1);
    assert_eq!(
        fitted.diagnostics().memory.fallback_mask,
        1_u32 << PpmlFallbackKind::WarmProjection as u8
    );
    assert!(relative_infinity_difference(fitted.eta(), baseline.eta()) <= 1e-12);
    let fitted_coefficients = fitted
        .coefficients()
        .iter()
        .map(|coefficient| coefficient.value.unwrap_or(0.0))
        .collect::<Vec<_>>();
    let baseline_coefficients = baseline
        .coefficients()
        .iter()
        .map(|coefficient| coefficient.value.unwrap_or(0.0))
        .collect::<Vec<_>>();
    assert!(relative_infinity_difference(&fitted_coefficients, &baseline_coefficients) <= 1e-12);
}

#[test]
fn failed_canonical_retry_after_warm_invalidation_is_atomic() {
    let error = injected_warm_failure_fit(Some(
        FrozenLinearFailureInjection::RecoverableThenCanonicalAllocation,
    ))
    .err()
    .expect("canonical allocation failure must abort the fit");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(
        error.message,
        "injected canonical fallback allocation failure"
    );
}

#[test]
fn domain_and_interrupt_failures_never_enter_the_canonical_fallback() {
    let domain = injected_warm_failure_fit(Some(FrozenLinearFailureInjection::Domain))
        .err()
        .expect("domain failure must remain fail-closed");
    assert_eq!(domain.code, ErrorCode::InvalidData);
    assert_eq!(
        domain.message,
        "frozen weighted-linear normalized weight is invalid"
    );

    let interrupted = injected_warm_failure_fit(Some(FrozenLinearFailureInjection::Interrupted))
        .err()
        .expect("interruption must remain fail-closed");
    assert_eq!(interrupted.code, ErrorCode::Interrupted);
    assert_eq!(interrupted.message, "injected frozen solve interruption");
}

#[test]
fn strict_candidate_categorical_projection_preserves_scratch_and_fresh_parity() {
    check_strict_candidate_categorical_projection(false);
}

#[test]
fn rebuilt_strict_candidate_preserves_storage_and_fresh_science() {
    check_strict_candidate_categorical_projection(true);
}

fn check_strict_candidate_categorical_projection(rebuild: bool) {
    for (mode, poolsize) in [
        (PreparedOlsExecutionMode::Fast, 10),
        (PreparedOlsExecutionMode::Low, 1),
        (PreparedOlsExecutionMode::Low, 10_000),
    ] {
        let outcome = vec![1.2, 4.0, 0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
        let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
        let response = vec![1.7, 3.6, 0.1, 6.8, 2.1, 9.0, 6.2, 12.4];
        let initial_weights = vec![1.0; outcome.len()];
        let refreshed_weights = vec![1.5, 0.75, 2.0, 1.25, 3.5, 0.6, 1.8, 2.2];
        let prepared = prepared_two_fe(&outcome, &x);
        let mut config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-10)
            .expect("two-FE linear config");
        config.execution.mode = mode;
        config.execution.poolsize = poolsize;
        let initial_scale =
            ppml_bread_weight_scale(&initial_weights).expect("initial weight scale");
        let initial_frame = LinearSolveFrame::from_parts(
            &outcome,
            BorrowedMapWeights::Provided(&initial_weights),
            initial_scale,
        )
        .expect("initial two-FE frame");
        let design = prepared.fit_owned_fe_design().expect("two-FE design");
        let mut session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            initial_frame,
            config,
            LinearSessionGenerations::new(1, 1, 1, 1, 1),
            design,
        )
        .expect("two-FE PPML session");
        assert!(session.uses_ppml_categorical_map_for_test());
        assert!(session.solve_fresh().expect("initial solve").certified());
        let scratch = session
            .retained_scratch_identity()
            .map(std::path::Path::to_owned);
        assert_eq!(scratch.is_some(), mode == PreparedOlsExecutionMode::Low);
        if mode == PreparedOlsExecutionMode::Low {
            assert_eq!(
                session.workspace_capacities().fe.rhs,
                poolsize.min(prepared.fit_model().columns.len().max(1))
            );
            assert_eq!(session.workspace_capacities().within_resident_elements, 0);
        }
        session
            .refresh_response(&response, 2)
            .expect("strict response refresh");
        let refreshed_scale =
            ppml_bread_weight_scale(&refreshed_weights).expect("refreshed weight scale");
        session
            .refresh_weights(
                BorrowedMapWeights::Provided(&refreshed_weights),
                refreshed_scale,
                2,
            )
            .expect("strict weight refresh");

        let offset = vec![0.0; outcome.len()];
        if rebuild {
            let error = session
                .rebuild_retained_inputs_interruptible(&mut || {
                    Err(FeregError::new(
                        ErrorCode::Interrupted,
                        "rebuild interrupt test",
                    ))
                })
                .expect_err("rebuild must propagate caller interruption");
            assert_eq!(error.code, ErrorCode::Interrupted);
            let original_thread = std::thread::current().id();
            let capacities = session.workspace_capacities();
            let mut polls = 0;
            session
                .rebuild_retained_inputs_interruptible(&mut || {
                    assert_eq!(std::thread::current().id(), original_thread);
                    polls += 1;
                    Ok(())
                })
                .expect("canonical input rebuild");
            assert!(polls > 0);
            assert_eq!(session.retained_scratch_identity(), scratch.as_deref());
            let after_rebuild = session.workspace_capacities();
            let mut allowed = capacities;
            if mode == PreparedOlsExecutionMode::Low {
                allowed.projection_block_elements = allowed
                    .projection_block_elements
                    .max(outcome.len() * poolsize.min(1));
            }
            assert_eq!(after_rebuild, allowed);
            session
                .rebuild_retained_inputs_interruptible(&mut || Ok(()))
                .expect("warm rebuild");
            assert_eq!(session.workspace_capacities(), after_rebuild);
        }
        let transaction = session.strict_candidate().expect("fused strict candidate");
        assert_eq!(
            transaction.used_fused_projection_for_test(),
            mode == PreparedOlsExecutionMode::Fast
        );
        let actual = transaction.candidate();
        assert!(actual.certified());
        assert!(actual.reuse_report().warm_projection_attempted);
        let mut actual_eta = vec![0.0; outcome.len()];
        fill_candidate_eta_from_linear_output(&actual, &offset, &mut actual_eta)
            .expect("fused strict eta");
        let mut actual_coefficients = vec![0.0; prepared.fit_model().coefficients.len()];
        fill_candidate_coefficients_from_linear_output(&actual, &mut actual_coefficients)
            .expect("fused strict coefficients");
        let actual_bread = actual.frame().bread().to_vec();
        transaction.commit();
        assert_eq!(session.retained_scratch_identity(), scratch.as_deref());

        let expected_frame = LinearSolveFrame::from_parts(
            &response,
            BorrowedMapWeights::Provided(&refreshed_weights),
            refreshed_scale,
        )
        .expect("fresh comparison frame");
        let expected_design = prepared
            .fit_owned_fe_design()
            .expect("comparison FE design");
        let mut expected_session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            expected_frame,
            config,
            LinearSessionGenerations::new(1, 1, 1, 1, 1),
            expected_design,
        )
        .expect("fresh comparison session");
        let expected = expected_session
            .solve_fresh()
            .expect("fresh comparison solve");
        let mut expected_eta = vec![0.0; outcome.len()];
        fill_candidate_eta_from_linear_output(&expected, &offset, &mut expected_eta)
            .expect("fresh comparison eta");
        let mut expected_coefficients = vec![0.0; prepared.fit_model().coefficients.len()];
        fill_candidate_coefficients_from_linear_output(&expected, &mut expected_coefficients)
            .expect("fresh comparison coefficients");
        assert!(relative_infinity_difference(&actual_eta, &expected_eta) <= 1e-11);
        assert!(
            relative_infinity_difference(&actual_coefficients, &expected_coefficients) <= 1e-11
        );
        assert!(relative_infinity_difference(&actual_bread, expected.frame().bread()) <= 1e-10);
    }
}

#[test]
fn joint_strict_final_preserves_truth_current_hessian_and_canonical_retry() {
    let _curve = ProfileCurveTestGuard::disabled();
    let _canonical = CanonicalFinalTestGuard::disabled();
    let n = 240;
    let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
    let x: Vec<_> = (0..n)
        .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
        .collect();
    let eta: Vec<_> = (0..n)
        .map(|i| {
            0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05
                + (g[i] + h[i]) * ((i % 5) as f64 - 2.) * 0.02
        })
        .collect();
    let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
    let prepared = prepared_joint_slopes(&y, &x, &g, &h);
    let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
    for memory_mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for threads in [1, 4] {
            let mut config = PreparedPpmlIrlsConfig::default();
            config.linear.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            config.linear.execution.mode = memory_mode;
            config.linear.execution.poolsize = 1;
            config.memory.requested = match memory_mode {
                PreparedOlsExecutionMode::Fast => crate::options::MemoryMode::Fast,
                PreparedOlsExecutionMode::Low => crate::options::MemoryMode::Low,
            };
            config.memory.poolsize = 1;
            for mode in [0, 1, 2, 3] {
                JOINT_FINAL_TEST_MODE.set(mode);
                JOINT_FINAL_TEST_COUNTS.set((0, 0));
                let fit = fit_prepared_ppml_irls(
                    &prepared,
                    &y,
                    &weights,
                    &vec![0.; n],
                    config,
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                    None,
                )
                .unwrap();
                assert!(
                    fit.diagnostics().converged && fit.diagnostics().two_consecutive_convergence
                );
                assert!(fit.diagnostics().final_score.passed);
                assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
                assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
                if mode == 0 {
                    assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1);
                }
                if mode == 1 {
                    assert_eq!(JOINT_FINAL_TEST_COUNTS.get(), (0, 0));
                }
                if mode >= 2 {
                    assert!(JOINT_FINAL_TEST_COUNTS.get().0 >= 1);
                    assert_eq!(JOINT_FINAL_TEST_MODE.get(), 0);
                }
            }
        }
    }
    JOINT_FINAL_TEST_MODE.set(0);
}

#[test]
fn canonical_final_preserves_strict_disconnected_slope_truth() {
    let n = 480;
    let g: Vec<_> = (0..n).map(|i| (i / 5 % 4 + 4 * (i / 240)) as f64).collect();
    let h: Vec<_> = (0..n)
        .map(|i| (i / 20 % 3 + 3 * (i / 240)) as f64)
        .collect();
    let x: Vec<_> = (0..n)
        .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
        .collect();
    let eta: Vec<_> = (0..n)
        .map(|i| {
            0.3 + 0.2 * x[i] + 0.1 * g[i] - 0.05 * h[i]
                + (g[i] + h[i]) * ((i % 5) as f64 - 2.) * 0.02
        })
        .collect();
    let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
    let prepared = prepared_joint_slopes(&y, &x, &g, &h);
    for disparity in [1e13] {
        let weights: Vec<_> = (0..n)
            .map(|i| (0.2 + (i % 13) as f64 / 7.) * if i < 240 { 1. } else { disparity })
            .collect();
        for (execution, memory) in [(
            PreparedOlsExecutionMode::Fast,
            crate::options::MemoryMode::Fast,
        )] {
            for threads in [1, 4] {
                let mut config = PreparedPpmlIrlsConfig::default();
                config.ppml.tolerance = 1e-12;
                config.ppml.score_tolerance = 1e-12;
                config.linear.execution.mode = execution;
                config.memory.requested = memory;
                config.linear.execution.poolsize = 1;
                config.memory.poolsize = 1;
                config.linear.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                {
                    let _canonical = CanonicalFinalTestGuard::disabled();
                    let _curve = ProfileCurveTestGuard::disabled();
                    let baseline = fit_prepared_ppml_irls(
                        &prepared,
                        &y,
                        &weights,
                        &vec![0.; n],
                        config,
                        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                            .unwrap(),
                        None,
                    )
                    .unwrap();
                    assert!(relative_infinity_difference(baseline.eta(), &eta) < 1e-10);
                }
                for mode in [0, 1, 2, 3] {
                    profile_final_test_mode(mode);
                    let fit = fit_prepared_ppml_irls(
                        &prepared,
                        &y,
                        &weights,
                        &vec![0.; n],
                        config,
                        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                            .unwrap(),
                        None,
                    )
                    .unwrap();
                    let accepted = profile_final_test_mode(0);
                    assert_eq!(accepted, 0);
                    assert!(fit.diagnostics().strict_profile_rejections > 0);
                    assert!(
                        fit.diagnostics().converged
                            && fit.diagnostics().two_consecutive_convergence
                    );
                    assert!(fit.diagnostics().final_score.passed);
                    assert!(
                        relative_infinity_difference(fit.eta(), &eta) < 1e-10,
                        "disparity={disparity} mode={mode} storage={execution:?} threads={threads} eta_error={:.17e}",
                        relative_infinity_difference(fit.eta(), &eta)
                    );
                    assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                    validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean())
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn canonical_final_replays_cancellation_before_saved_effect_recovery() {
    let n = 4096;
    let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 8 % 8) as f64).collect();
    let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.37).sin()).collect();
    let y: Vec<_> = (0..n).map(|i| (2 + i % 5) as f64).collect();
    let mut offset: Vec<_> = (0..n).map(|i| (0.2 + (i % 17) as f64 / 8.0).ln()).collect();
    offset[37] = 5e-7_f64.ln();
    let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
    let config = PreparedPpmlIrlsConfig::default();
    profile_final_test_mode(0);
    let fit = fit_prepared_ppml_irls(
        &prepared,
        &y,
        &vec![1.0; n],
        &offset,
        config,
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
        None,
    )
    .unwrap();
    assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
    // This balanced Cartesian design has a separate closed-form additive
    // projection, independent of the production FE solver and its weights.
    let d = fit.linear().absorbed_predictor();
    let mut first = [0.0; 8];
    let mut second = [0.0; 8];
    let mut overall = 0.0;
    for i in 0..n {
        first[g[i] as usize] += d[i] / (n / 8) as f64;
        second[h[i] as usize] += d[i] / (n / 8) as f64;
        overall += d[i] / n as f64;
    }
    let error = (0..n)
        .map(|i| (d[i] - first[g[i] as usize] - second[h[i] as usize] + overall).abs())
        .fold(0.0_f64, f64::max);
    assert!(error < 1e-12, "absorbed span error={error:.17e}");
    assert_eq!(profile_final_test_mode(0), 0);
}

#[test]
fn canonical_final_preserves_slope_truth_current_hessian_and_reuse_refusal() {
    let n = 240;
    let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
    let x: Vec<_> = (0..n)
        .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
        .collect();
    let eta: Vec<_> = (0..n)
        .map(|i| {
            0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05
                + (g[i] + h[i]) * ((i % 5) as f64 - 2.) * 0.02
        })
        .collect();
    let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
    let prepared = prepared_joint_slopes(&y, &x, &g, &h);
    let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
    for memory_mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for threads in [1, 4] {
            let mut config = PreparedPpmlIrlsConfig::default();
            config.linear.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            config.linear.execution.mode = memory_mode;
            config.linear.execution.poolsize = 1;
            config.memory.requested = match memory_mode {
                PreparedOlsExecutionMode::Fast => crate::options::MemoryMode::Fast,
                PreparedOlsExecutionMode::Low => crate::options::MemoryMode::Low,
            };
            config.memory.poolsize = 1;
            for mode in [0, 1, 2, 3] {
                profile_final_test_mode(mode);
                let fit = fit_prepared_ppml_irls(
                    &prepared,
                    &y,
                    &weights,
                    &vec![0.; n],
                    config,
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                    None,
                )
                .unwrap();
                assert!(
                    fit.diagnostics().converged && fit.diagnostics().two_consecutive_convergence
                );
                assert!(fit.diagnostics().final_score.passed);
                assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
                assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
                assert_eq!(profile_final_test_mode(0), usize::from(mode == 0));
            }
        }
    }
}

#[test]
fn low_three_fe_audit_certifies_map_window_independently_of_checkpoint_engine() {
    fn stable_probe() -> PpmlAllocatorSnapshot {
        PpmlAllocatorSnapshot {
            current_bytes: 16384,
            peak_bytes: 32768,
            allocation_events: 41,
        }
    }
    let (prepared, y) = prepared_three_fe([0, 1, 2], false, None);
    let mut config = PreparedPpmlIrlsConfig::default();
    config.linear.execution.mode = PreparedOlsExecutionMode::Low;
    config.linear.execution.poolsize = 1;
    config.memory.requested = crate::options::MemoryMode::Low;
    config.memory.poolsize = 1;
    config.allocation_snapshot_probe = Some(stable_probe);
    let fit = fit_prepared_ppml_irls(
        &prepared,
        &y,
        &vec![1.; y.len()],
        &vec![0.; y.len()],
        config,
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
        None,
    )
    .unwrap();
    let memory = &fit.diagnostics().memory;
    memory.validate().unwrap();
    let audit = memory.steady_state_allocation_certificate.unwrap();
    assert!(audit.passed && audit.observation.fresh_warm_equivalent);
    assert_eq!(audit.observation.linear_route, FeSolverKind::Map);
    // The admitted centered Schur checkpoint is a coefficient-space MAP
    // accelerator. Its allocating canonical work is outside the audited
    // ordinary window; the separately certified LSMR alternative remains
    // an allowed final route if Schur declines.
    assert_eq!(memory.linear_route, Some(FeSolverKind::Lsmr));
    assert!(memory
        .plan
        .audit_route_matches_final(FeSolverKind::Map, FeSolverKind::Lsmr));
    for route in [FeSolverKind::Auto, FeSolverKind::Cg, FeSolverKind::Cmg] {
        assert!(!memory
            .plan
            .audit_route_matches_final(FeSolverKind::Map, route));
    }
    let mut invalid = memory.plan;
    invalid.solver_workspace_bytes = 0;
    assert!(!invalid.audit_route_matches_final(FeSolverKind::Map, FeSolverKind::Lsmr));
    invalid = memory.plan;
    invalid.dimensions.fe_total_channels += 1;
    assert!(!invalid.audit_route_matches_final(FeSolverKind::Map, FeSolverKind::Lsmr));
}

#[test]
fn low_three_fe_checkpoint_budget_refusal_preserves_joint_owner_and_science() {
    let (prepared, y) = prepared_three_fe([0, 1, 2], false, None);
    let weights: Vec<_> = (0..y.len()).map(|i| 0.2 + (i % 13) as f64).collect();
    let mut config = PreparedPpmlIrlsConfig::default();
    config.linear.execution.mode = PreparedOlsExecutionMode::Low;
    config.linear.execution.poolsize = 1;
    config.memory.requested = crate::options::MemoryMode::Low;
    config.memory.poolsize = 1;
    let plan = plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap();
    let fit = |plan| {
        fit_prepared_ppml_irls(
            &prepared,
            &y,
            &weights,
            &vec![0.; y.len()],
            config,
            plan,
            None,
        )
        .unwrap()
    };
    let admitted = fit(plan);
    assert_eq!(
        admitted.diagnostics().solver.checkpoint,
        super::super::PpmlSolverFamily::Lsmr
    );
    let mut constrained = plan;
    constrained.usable_bytes = Some(admitted.diagnostics().memory.plan.selected_bytes() - 1);
    let declined = fit(constrained);
    assert_eq!(
        declined.diagnostics().solver.checkpoint,
        super::super::PpmlSolverFamily::Map
    );
    assert_eq!(
        admitted.diagnostics().memory.plan.reusable_slope_bytes,
        declined.diagnostics().memory.plan.reusable_slope_bytes
    );
    assert!(declined.diagnostics().memory.plan.reusable_slope_bytes > 0);
    assert!(
        declined.diagnostics().memory.plan.selected_bytes() <= constrained.usable_bytes.unwrap()
    );
    assert!(relative_infinity_difference(admitted.eta(), declined.eta()) < 1e-10);
    assert!(admitted.diagnostics().final_score.passed && declined.diagnostics().final_score.passed);
}

#[test]
fn low_categorical_geometry_refusal_retains_canonical_projection() {
    let (prepared, y) = prepared_three_fe([0, 1, 2], false, None);
    let weights: Vec<_> = (0..y.len()).map(|i| 0.2 + (i % 13) as f64).collect();
    let frame = LinearSolveFrame::from_parts(
        &y,
        BorrowedMapWeights::Provided(&weights),
        ppml_bread_weight_scale(&weights).unwrap(),
    )
    .unwrap();
    let mut config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-12).unwrap();
    config.execution.mode = PreparedOlsExecutionMode::Low;
    config.execution.poolsize = 1;
    let generations = LinearSessionGenerations::new(1, 1, 1, 1, 1);
    let mut session = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        frame,
        config,
        generations,
        prepared.fit_owned_fe_design().unwrap(),
    )
    .unwrap();
    // Optional certificate-bank refusal must leave the established
    // categorical projector available without a second topology copy.
    session
        .enable_ppml_coefficient_solver(1, u128::MAX, Some(u128::MAX), Some(u128::MAX))
        .unwrap();
    assert!(session.uses_ppml_categorical_map_for_test());
    let actual = session.solve_fresh().unwrap();
    assert!(actual.certified());
    let mut baseline = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        frame,
        config,
        generations,
        prepared.fit_owned_fe_design().unwrap(),
    )
    .unwrap();
    let expected = baseline.solve_fresh().unwrap();
    assert_eq!(actual.frame().residuals(), expected.frame().residuals());
    assert_eq!(actual.frame().bread(), expected.frame().bread());
}

#[test]
fn fast_categorical_lsmr_strict_final_preserves_truth_and_canonical_retry() {
    // Preserve coverage of the fallback when the optional curve is absent.
    let _canonical = CanonicalFinalTestGuard::disabled();
    let _curve = ProfileCurveTestGuard::disabled();
    let n = 240;
    let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
    let x: Vec<_> = (0..n)
        .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
        .collect();
    let eta: Vec<_> = (0..n)
        .map(|i| 0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05)
        .collect();
    let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
    let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
    let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
    for threads in [1, 4] {
        let mut config = PreparedPpmlIrlsConfig::default();
        config.linear.execution.threads =
            crate::options::ThreadConfig::from_requested(threads, true).unwrap();
        config.solver_plan = PpmlSolverPlan::new(super::super::PpmlSolverFamily::Lsmr);
        for mode in [0, 1, 2, 3, 4] {
            JOINT_FINAL_TEST_MODE.set(mode);
            JOINT_FINAL_TEST_COUNTS.set((0, 0));
            let fit = fit_prepared_ppml_irls(
                &prepared,
                &y,
                &weights,
                &vec![0.; n],
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                None,
            )
            .unwrap();
            assert!(fit.diagnostics().converged && fit.diagnostics().two_consecutive_convergence);
            assert!(fit.diagnostics().final_score.passed);
            assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
            assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
            validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
            match mode {
                0 => {
                    assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1);
                    assert!(fit.diagnostics().solver.engines()[2].checkpoint_frames >= 1);
                }
                1 => assert_eq!(JOINT_FINAL_TEST_COUNTS.get(), (0, 0)),
                _ => {
                    assert!(JOINT_FINAL_TEST_COUNTS.get().0 >= 1);
                    assert_eq!(JOINT_FINAL_TEST_MODE.get(), 0);
                }
            }
        }
    }
    JOINT_FINAL_TEST_MODE.set(0);
}

#[test]
fn low_categorical_joint_strict_final_preserves_truth_and_canonical_retry() {
    // Preserve coverage of the fallback when the optional curve is absent.
    let _canonical = CanonicalFinalTestGuard::disabled();
    let _curve = ProfileCurveTestGuard::disabled();
    let n = 240;
    let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
    let x: Vec<_> = (0..n)
        .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
        .collect();
    let eta: Vec<_> = (0..n)
        .map(|i| 0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05)
        .collect();
    let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
    let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
    let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
    for threads in [1, 4] {
        let mut config = PreparedPpmlIrlsConfig::default();
        config.linear.execution.threads =
            crate::options::ThreadConfig::from_requested(threads, true).unwrap();
        config.linear.execution.mode = PreparedOlsExecutionMode::Low;
        config.linear.execution.poolsize = 1;
        config.memory.requested = crate::options::MemoryMode::Low;
        config.memory.poolsize = 1;
        for mode in [0, 1, 2, 3] {
            JOINT_FINAL_TEST_MODE.set(mode);
            JOINT_FINAL_TEST_COUNTS.set((0, 0));
            let fit = fit_prepared_ppml_irls(
                &prepared,
                &y,
                &weights,
                &vec![0.; n],
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                None,
            )
            .unwrap();
            assert!(fit.diagnostics().converged && fit.diagnostics().two_consecutive_convergence);
            assert!(fit.diagnostics().final_score.passed);
            assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
            assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
            validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
            match mode {
                0 => assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1),
                1 => assert_eq!(JOINT_FINAL_TEST_COUNTS.get(), (0, 0)),
                _ => {
                    assert!(JOINT_FINAL_TEST_COUNTS.get().0 >= 1);
                    assert_eq!(JOINT_FINAL_TEST_MODE.get(), 0);
                }
            }
        }
    }
    JOINT_FINAL_TEST_MODE.set(0);
}

#[test]
fn fast_categorical_lsmr_three_fe_preserves_canonical_science_nested_rank_and_zero_control() {
    let _canonical = CanonicalFinalTestGuard::disabled();
    for (order, nested, zero_control) in [
        ([0, 1, 2], None, false),
        ([2, 0, 1], Some(0), false),
        ([1, 2, 0], Some(1), true),
    ] {
        let (prepared, _) = prepared_three_fe(order, zero_control, nested);
        let design = prepared.fit_owned_fe_design().unwrap();
        let n = design.nobs();
        let offset: Vec<_> = (0..n).map(|i| (i as f64 / 17.).cos() * 0.1).collect();
        let eta: Vec<_> = (0..n)
            .map(|i| {
                let x = if zero_control {
                    0.
                } else {
                    (i as f64 * 0.173).sin() + (i as f64 * 0.071).cos()
                };
                offset[i]
                    + 0.3
                    + 0.2 * x
                    + design
                        .effects()
                        .iter()
                        .enumerate()
                        .map(|(j, e)| 0.02 * (j + 1) as f64 * e.codes()[i] as f64)
                        .sum::<f64>()
            })
            .collect();
        let y: Vec<_> = eta
            .iter()
            .enumerate()
            .map(|(i, v)| v.exp() + 0.1 * (i as f64 / 11.).sin())
            .collect();
        let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
        let mut config = PreparedPpmlIrlsConfig::default();
        let reference = fit_prepared_ppml_irls(
            &prepared,
            &y,
            &weights,
            &offset,
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
            None,
        )
        .unwrap();
        config.solver_plan = PpmlSolverPlan::new(super::super::PpmlSolverFamily::Lsmr);
        JOINT_FINAL_TEST_COUNTS.set((0, 0));
        let fit = fit_prepared_ppml_irls(
            &prepared,
            &y,
            &weights,
            &offset,
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
            None,
        )
        .unwrap();
        assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
        assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1);
        let engine = if zero_control { 1 } else { 2 };
        assert!(fit.diagnostics().solver.engines()[engine].checkpoint_frames >= 1);
        assert!(relative_infinity_difference(fit.eta(), reference.eta()) < 1e-9);
        if !zero_control {
            assert!(
                (fit.coefficients()[0].value.unwrap() - reference.coefficients()[0].value.unwrap())
                    .abs()
                    < 1e-10
            );
        }
        validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
    }
}
#[test]
fn higher_dimensional_categorical_frames_preserve_weighted_poisson_truth() {
    // Identical pairs have opposite response errors, so every weighted
    // explicit-design score vanishes at eta despite nonzero residuals.
    for (dimensions, nested, zero_control) in
        [(4, false, false), (5, false, false), (4, true, true)]
    {
        let n = 480;
        let x = source(1, "x");
        let effects: Vec<_> = (0..dimensions)
            .map(|j| source((j + 2) as u32, &format!("g{j}")))
            .collect();
        let codes: Vec<Vec<f64>> = (0..dimensions)
            .map(|j| {
                (0..n)
                    .map(|i| {
                        let row = i / 2;
                        (match j {
                            0 => row % 24,
                            1 => (row * 5 + row / 24) % 9,
                            2 => (row * 7 + row / 11) % 4,
                            3 if nested => (row % 24) / 3,
                            3 => (row * 11 + row / 7) % 5,
                            _ => (row / 3) % 3,
                        }) as f64
                    })
                    .collect()
            })
            .collect();
        let values: Vec<_> = (0..n)
            .map(|i| {
                if zero_control {
                    0.
                } else {
                    ((i / 2) as f64 * 0.173).sin()
                }
            })
            .collect();
        let offset: Vec<_> = (0..n).map(|i| ((i / 2) as f64 / 17.).cos() * 0.1).collect();
        let eta: Vec<_> = (0..n)
            .map(|i| {
                offset[i]
                    + 0.3
                    + 0.2 * values[i]
                    + codes
                        .iter()
                        .enumerate()
                        .map(|(j, c)| 0.02 * (j + 1) as f64 * c[i])
                        .sum::<f64>()
            })
            .collect();
        let y: Vec<_> = eta
            .iter()
            .enumerate()
            .map(|(i, v)| v.exp() * if i % 2 == 0 { 0.9 } else { 1.1 })
            .collect();
        let weights: Vec<_> = (0..n).map(|i| 0.2 + ((i / 2) % 17) as f64 / 7.).collect();
        let mut sources = vec![x.clone()];
        sources.extend(effects.clone());
        let mut numeric_sources = vec![(x.id, loaded(&values))];
        numeric_sources.extend(effects.iter().zip(&codes).map(|(e, c)| (e.id, loaded(c))));
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::NumericSource(x.id),
            },
        ];
        bindings.extend(effects.iter().enumerate().map(|(j, e)| OlsInputBinding {
            stata_var_position: (j + 4) as u32,
            role: OlsInputRole::NumericSource(e.id),
        }));
        let prepared = prepare_model(
            &OlsRequestV1 {
                model: CompiledTermGraph {
                    sources,
                    terms: vec![continuous_term(1, x.id)],
                },
                absorbed_terms: effects
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(j, e)| AbsorbedTerm {
                        id: AbsorbTermId::new((dimensions - j) as u32).unwrap(),
                        intercept: true,
                        categorical_sources: vec![e.id],
                        slope_sources: Vec::new(),
                        label: format!("g{j}"),
                        saved_name: String::new(),
                    })
                    .collect(),
                bindings,
                include_intercept: true,
                keep_singletons: true,
            },
            &[],
            &LoadedOlsInput {
                candidate: vec![true; n].into(),
                outcome: loaded(&y),
                numeric_sources,
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
            false,
        )
        .unwrap();
        for (mode, threads) in [
            (PreparedOlsExecutionMode::Fast, 1),
            (PreparedOlsExecutionMode::Fast, 4),
            (PreparedOlsExecutionMode::Low, 4),
        ] {
            let mut config = PreparedPpmlIrlsConfig::default();
            config.linear.execution.mode = mode;
            config.linear.execution.threads.requested = std::num::NonZeroUsize::new(threads);
            config.memory.requested = if mode == PreparedOlsExecutionMode::Low {
                crate::options::MemoryMode::Low
            } else {
                crate::options::MemoryMode::Fast
            };
            let plan =
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap();
            let fit = fit_prepared_ppml_irls(&prepared, &y, &weights, &offset, config, plan, None)
                .unwrap();
            assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
            assert!(fit.diagnostics().memory.plan.reusable_slope_bytes > 0);
            assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
            if !zero_control {
                assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
            }
            validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
            if dimensions == 4 && !nested && mode == PreparedOlsExecutionMode::Fast && threads == 1
            {
                let mut constrained = plan;
                constrained.usable_bytes = Some(plan.selected_bytes());
                let fallback = fit_prepared_ppml_irls(
                    &prepared,
                    &y,
                    &weights,
                    &offset,
                    config,
                    constrained,
                    None,
                )
                .unwrap();
                assert_eq!(fallback.diagnostics().memory.plan.reusable_slope_bytes, 0);
                assert!(
                    fallback.diagnostics().memory.plan.selected_bytes()
                        <= constrained.usable_bytes.unwrap()
                );
                // Refused optional workspace retains the generic engine;
                // check its published means against the scientific envelope.
                for (&actual, &truth) in fallback.mean().iter().zip(&eta) {
                    let expected = truth.exp();
                    assert!((actual - expected).abs() <= 1e-9 + 1e-7 * expected.abs());
                }
                validate_final_hessian_frame(
                    fallback.linear().weights(),
                    &weights,
                    fallback.mean(),
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn low_categorical_joint_three_fe_preserves_truth_nested_rank_and_zero_control() {
    for (order, nested, zero_control) in [
        ([0, 1, 2], None, false),
        ([2, 0, 1], Some(0), false),
        ([1, 2, 0], Some(1), true),
    ] {
        let (prepared, _) = prepared_three_fe(order, zero_control, nested);
        let design = prepared.fit_owned_fe_design().unwrap();
        let n = design.nobs();
        let offset: Vec<_> = (0..n).map(|i| (i as f64 / 17.).cos() * 0.1).collect();
        let eta: Vec<_> = (0..n)
            .map(|i| {
                let x = if zero_control {
                    0.
                } else {
                    (i as f64 * 0.173).sin() + (i as f64 * 0.071).cos()
                };
                offset[i]
                    + 0.3
                    + 0.2 * x
                    + design
                        .effects()
                        .iter()
                        .enumerate()
                        .map(|(j, e)| 0.02 * (j + 1) as f64 * e.codes()[i] as f64)
                        .sum::<f64>()
            })
            .collect();
        let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
        let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
        let mut config = PreparedPpmlIrlsConfig::default();
        config.linear.execution.mode = PreparedOlsExecutionMode::Low;
        config.linear.execution.poolsize = 1;
        config.memory.requested = crate::options::MemoryMode::Low;
        config.memory.poolsize = 1;
        let fit = fit_prepared_ppml_irls(
            &prepared,
            &y,
            &weights,
            &offset,
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
            None,
        )
        .unwrap();
        assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
        assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
        if !zero_control {
            assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
        }
        validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
    }
}
#[test]
fn joint_slope_session_preserves_changing_frames_rollback_and_fresh_rank() {
    let n = 240;
    let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
    let x: Vec<_> = (0..n)
        .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
        .collect();
    let y: Vec<_> = (0..n)
        .map(|i| 3. + 0.7 * x[i] + g[i] * 0.2 - h[i] * 0.1 + (i as f64 / 13.).cos())
        .collect();
    let prepared = prepared_joint_slopes(&y, &x, &g, &h);
    for (mode, poolsize) in [
        (PreparedOlsExecutionMode::Fast, 10),
        (PreparedOlsExecutionMode::Low, 1),
        (PreparedOlsExecutionMode::Low, 2),
    ] {
        for threads in [1, 4] {
            let mut config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-12).unwrap();
            config.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            config.execution.mode = mode;
            config.execution.poolsize = poolsize;
            let initial = vec![1.; n];
            let frame = LinearSolveFrame::from_parts(
                &y,
                BorrowedMapWeights::Provided(&initial),
                ppml_bread_weight_scale(&initial).unwrap(),
            )
            .unwrap();
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                frame,
                config,
                LinearSessionGenerations::new(1, 1, 1, 1, 1),
                prepared.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            session.enable_ppml_certificate_geometry(u128::MAX).unwrap();
            session
                .enable_ppml_coefficient_solver(
                    threads,
                    u128::MAX,
                    Some(u128::MAX),
                    Some(u128::MAX),
                )
                .unwrap();
            session.enable_ppml_joint_slopes(u128::MAX).unwrap();
            assert!(session.solve_fresh().unwrap().certified());
            for generation in 2..7 {
                let weights: Vec<_> = (0..n)
                    .map(|i| 0.2 + ((i * 13 + generation * 7) % 31) as f64 / 7.)
                    .collect();
                let response: Vec<_> = (0..n)
                    .map(|i| y[i] + (generation as f64) * 0.2 * (i as f64 / 11.).sin())
                    .collect();
                let scale = ppml_bread_weight_scale(&weights).unwrap();
                session
                    .refresh_response(&response, generation as u64)
                    .unwrap();
                session
                    .refresh_weights(
                        BorrowedMapWeights::Provided(&weights),
                        scale,
                        generation as u64,
                    )
                    .unwrap();
                let ordinary = session.solve_ordinary_ppml().unwrap();
                assert!(matches!(ordinary, PpmlOrdinaryLinearOutput::Joint(_)));
                assert!(ordinary.certified());
                let actual_beta = ordinary.coefficients().to_vec();
                let actual_residuals = ordinary.within_residuals().to_vec();
                let expected_frame = LinearSolveFrame::from_parts(
                    &response,
                    BorrowedMapWeights::Provided(&weights),
                    scale,
                )
                .unwrap();
                let mut expected_session = CertifiedLinearSession::new_with_fe_design(
                    &prepared,
                    expected_frame,
                    config,
                    LinearSessionGenerations::new(1, 1, 1, 1, 1),
                    prepared.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                let expected = expected_session.solve_fresh().unwrap();
                assert!(
                    relative_infinity_difference(
                        &actual_beta,
                        &expected
                            .coefficients()
                            .filter(|c| c.generated_column_index.is_some_and(|g| expected
                                .basis()
                                .kept_generated_columns()
                                .contains(&g)))
                            .map(|c| c.value.unwrap())
                            .collect::<Vec<_>>()
                    ) <= 1e-10
                );
                assert!(
                    relative_infinity_difference(&actual_residuals, expected.frame().residuals())
                        <= 1e-10
                );
                let expected_bread = expected.frame().bread().to_vec();
                // A speculative strict frame may rebuild the canonical within
                // design, but rollback must leave a valid representative for
                // another ordinary direction under this frame and the next.
                let transaction = session.strict_candidate().unwrap();
                assert!(transaction.candidate().certified());
                assert!(
                    relative_infinity_difference(
                        transaction.candidate().frame().bread(),
                        &expected_bread
                    ) <= 1e-10
                );
                if generation % 2 == 0 {
                    transaction.rollback();
                } else {
                    transaction.commit();
                }
                let repeated = session.solve_ordinary_ppml().unwrap();
                assert!(repeated.certified());
                assert!(
                    relative_infinity_difference(repeated.coefficients(), &actual_beta) <= 1e-10
                );
                assert!(
                    relative_infinity_difference(repeated.within_residuals(), &actual_residuals)
                        <= 1e-10
                );
                session.discard_invalid_frozen_state();
                assert!(session.solve_fresh().unwrap().certified());
            }
        }
    }
}

#[test]
fn bitwise_identical_weights_skip_final_frame_control_reprojection() {
    let outcome = vec![1.2, 4.0, 0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
    let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
    let response = vec![0.9, 3.8, 0.4, 5.9, 2.5, 8.1, 5.2, 11.4];
    let weights = vec![1.5, 0.75, 2.0, 1.25, 3.5, 0.6, 1.8, 2.2];
    let prepared = prepared_two_fe(&outcome, &x);
    let config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-10)
        .expect("identical-weight linear config");
    let weight_scale = ppml_bread_weight_scale(&weights).expect("weight scale");
    let frame = LinearSolveFrame::from_parts(
        &outcome,
        BorrowedMapWeights::Provided(&weights),
        weight_scale,
    )
    .expect("identical-weight initial frame");
    let design = prepared.fit_owned_fe_design().expect("two-FE design");
    let mut session = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        frame,
        config,
        LinearSessionGenerations::new(1, 1, 1, 1, 1),
        design,
    )
    .expect("identical-weight session");
    assert!(session.solve_fresh().expect("initial solve").certified());
    assert!(session
        .current_hessian_products_match(BorrowedMapWeights::Provided(&weights), weight_scale, 1)
        .unwrap());
    assert!(!session
        .current_hessian_products_match(BorrowedMapWeights::Provided(&weights), weight_scale, 2)
        .unwrap());
    let mut changed = weights.clone();
    changed[0] = f64::from_bits(changed[0].to_bits() + 1);
    assert!(!session
        .current_hessian_products_match(BorrowedMapWeights::Provided(&changed), weight_scale, 1)
        .unwrap());
    let mut changed_scale = weight_scale;
    changed_scale.reference_weight_multiplier *= 2.0;
    assert!(!session
        .current_hessian_products_match(BorrowedMapWeights::Provided(&weights), changed_scale, 1)
        .unwrap());
    assert!(session
        .refresh_frame(
            &response,
            BorrowedMapWeights::Provided(&weights),
            weight_scale,
            2,
        )
        .expect("identical weight refresh"));

    let transaction = session
        .strict_candidate()
        .expect("identical-weight strict candidate");
    assert!(!transaction.used_fused_projection_for_test());
    let candidate = transaction.candidate();
    assert!(candidate.certified());
    assert!(candidate.reuse_report().within_design_reused);
    assert!(!candidate.reuse_report().warm_projection_attempted);
    let offset = vec![0.0; outcome.len()];
    let mut actual_eta = vec![0.0; outcome.len()];
    fill_candidate_eta_from_linear_output(&candidate, &offset, &mut actual_eta)
        .expect("identical-weight candidate eta");
    transaction.commit();

    let expected_frame = LinearSolveFrame::from_parts(
        &response,
        BorrowedMapWeights::Provided(&weights),
        weight_scale,
    )
    .expect("identical-weight comparison frame");
    let expected_design = prepared
        .fit_owned_fe_design()
        .expect("comparison FE design");
    let mut expected_session = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        expected_frame,
        config,
        LinearSessionGenerations::new(1, 1, 1, 1, 1),
        expected_design,
    )
    .expect("comparison session");
    let expected = expected_session.solve_fresh().expect("comparison solve");
    let mut expected_eta = vec![0.0; outcome.len()];
    fill_candidate_eta_from_linear_output(&expected, &offset, &mut expected_eta)
        .expect("comparison eta");
    assert!(relative_infinity_difference(&actual_eta, &expected_eta) <= 1e-11);
}

#[test]
fn strict_no_fe_constant_outcome_matches_the_independent_optimum() {
    let rows = 16_384;
    let outcome = vec![2.0; rows];
    let mut columns: Vec<Vec<f64>> = (0..4)
        .map(|column| {
            (0..rows)
                .map(|row| ((row + 1) as f64 * (column + 1) as f64 * 1.123).sin())
                .collect()
        })
        .collect();
    for level in 1..7 {
        columns.push(
            (0..rows)
                .map(|row| f64::from((row + 1) % 7 == level))
                .collect(),
        );
    }
    let data = prepared_no_fe_categorical(&outcome, &columns);
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut config = PreparedPpmlIrlsConfig::default();
        config.ppml.tolerance = 1e-12;
        config.ppml.score_tolerance = 1e-12;
        config.linear.execution.mode = mode;
        config.linear.execution.poolsize = 1;
        config.memory.requested = if mode == PreparedOlsExecutionMode::Fast {
            crate::options::MemoryMode::Fast
        } else {
            crate::options::MemoryMode::Low
        };
        config.memory.poolsize = 1;
        let plan = plan_ppml_memory(memory_dimensions(&data, config), config.memory).unwrap();
        let fit = fit_prepared_ppml_irls(
            &data,
            &outcome,
            &vec![1.0; rows],
            &vec![0.0; rows],
            config,
            plan,
            None,
        )
        .expect("full-rank no-FE constant outcome has a unique finite optimum");
        assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
        assert!(fit.diagnostics().iterations < 40);
        assert_eq!(fit.linear().frame().finite_rank(), 11);
        for (slot, coefficient) in fit.coefficients().iter().enumerate() {
            let expected = if slot == 10 { 2.0_f64.ln() } else { 0.0 };
            assert!(
                (coefficient.value.expect("full-rank coefficient") - expected).abs() < 1e-13,
                "mode={mode:?} slot={slot} coefficient={coefficient:?}"
            );
        }
        assert!(fit
            .eta()
            .iter()
            .all(|eta| (eta - 2.0_f64.ln()).abs() < 1e-13));
        assert!(fit.mean().iter().all(|mean| (mean - 2.0).abs() < 2e-13));
    }
}

#[test]
fn persistent_strict_profile_failure_rebuilds_then_returns_bounded_nonconvergence() {
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
        let y: Vec<_> = x
            .iter()
            .enumerate()
            .map(|(row, x)| (0.35_f64 + 0.42 * x).exp() + (row % 3) as f64 * 0.03)
            .collect();
        let prepared = prepared(&y, &[x]);
        let mut config = PreparedPpmlIrlsConfig::default();
        config.linear.execution.mode = mode;
        config.linear.execution.poolsize = 1;
        config.memory.requested = if mode == PreparedOlsExecutionMode::Fast {
            crate::options::MemoryMode::Fast
        } else {
            crate::options::MemoryMode::Low
        };
        config.memory.poolsize = 1;
        let plan = plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap();
        REPEATED_STRICT_RESUME_INJECTION.set(100);
        let mut iterations = 0;
        let result = fit_prepared_ppml_irls_interruptible(
            &prepared,
            &y,
            &[1.0; 8],
            &[0.0; 8],
            config,
            plan,
            None,
            &mut |point| {
                if point == PpmlInterruptPoint::IrlsIteration {
                    iterations += 1;
                }
                Ok(())
            },
        );
        REPEATED_STRICT_RESUME_INJECTION.set(0);
        let error = result
            .err()
            .expect("persistent profile corruption must publish nothing");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert!(
            error
                .message
                .contains("canonical rebuild and two further strict attempts"),
            "{error:?}"
        );
        assert!(iterations < 40, "unbounded retry count: {iterations}");
    }
}

#[test]
fn profile_recovery_is_bounded_but_does_not_penalize_progress_or_changed_bases() {
    let mut recovery = ProfileRecovery::default();
    for difference in [1e-3, 5e-4, 2e-4, 1e-4, 3e-5, 1e-5] {
        assert_eq!(
            recovery.observe(true, true, difference),
            ProfileRecoveryAction::Continue
        );
    }
    recovery = ProfileRecovery::default();
    assert_eq!(
        recovery.observe(true, true, 1e-4),
        ProfileRecoveryAction::Continue
    );
    assert_eq!(
        recovery.observe(true, true, 2e-4),
        ProfileRecoveryAction::Continue
    );
    assert_eq!(
        recovery.observe(true, true, 1e-4),
        ProfileRecoveryAction::Rebuild
    );
    assert_eq!(
        recovery.observe(true, true, 1e-4),
        ProfileRecoveryAction::Continue
    );
    assert_eq!(
        recovery.observe(true, true, 1e-4),
        ProfileRecoveryAction::Continue
    );
    assert_eq!(
        recovery.observe(true, true, 1e-4),
        ProfileRecoveryAction::Exhausted
    );
    assert_eq!(
        recovery.observe(false, false, 1e-4),
        ProfileRecoveryAction::Continue
    );
    assert_eq!(
        recovery.observe(true, true, 1e-4),
        ProfileRecoveryAction::Continue
    );
    assert_eq!(
        recovery.observe(false, true, 1e-4),
        ProfileRecoveryAction::Continue
    );
    assert_eq!(
        recovery.observe(true, true, 1e-4),
        ProfileRecoveryAction::Continue
    );
}

#[test]
fn strict_projection_refinement_is_bounded_and_never_loosens_a_target() {
    for current in [1e-8, 1e-12, 1e-14, 1e-16] {
        let refined = refined_checkpoint_tolerance(current);
        assert!(refined > 0.0 && refined <= current);
    }
    assert_eq!(refined_checkpoint_tolerance(1e-12), 1e-14);
    assert_eq!(refined_checkpoint_tolerance(1e-14), 1e-14);
    assert_eq!(refined_checkpoint_tolerance(1e-16), 1e-16);
}

#[test]
fn strict_checkpoint_movement_resumes_bounded_irls_at_both_checkpoints() {
    for (first_strict, final_hessian) in [(true, false), (false, true)] {
        let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
        let eta = x
            .iter()
            .map(|value| 0.35_f64 + 0.42_f64 * value)
            .collect::<Vec<_>>();
        let outcome = eta
            .iter()
            .enumerate()
            .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
            .collect::<Vec<_>>();
        let prepared = prepared(&outcome, &[x]);
        let config = PreparedPpmlIrlsConfig::default();
        inject_strict_resume(first_strict, final_hessian);
        let fitted = fit_prepared_ppml_irls(
            &prepared,
            &outcome,
            &[1.0; 8],
            &[0.0; 8],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                .expect("strict-resume memory plan"),
            None,
        )
        .expect("strict movement resumes and converges");
        assert!(fitted.diagnostics().converged);
        assert!(
            fitted.diagnostics().strict_profile_rejections >= 1,
            "strict resume injection was not exercised: first={first_strict} final={final_hessian}"
        );
        assert!(fitted.diagnostics().iterations < config.ppml.max_iterations);
        assert!(fitted
            .diagnostics()
            .solver
            .engines()
            .iter()
            .any(|row| row.retries > 0));
        assert!(fitted.diagnostics().two_consecutive_convergence);
        assert!(fitted.diagnostics().final_score.passed);
    }
}

#[test]
fn strict_candidate_rank_predicate_is_shared_by_both_checkpoints() {
    let eta = [0.1, -0.2, 0.3];
    let coefficients = [0.4, -0.1];
    let objective = PoissonObjective {
        deviance: 2.5,
        log_likelihood: -1.25,
    };
    let config = PpmlConfig::default();
    assert_eq!(
        strict_candidate_decision(
            true,
            true,
            2,
            2,
            &eta,
            &coefficients,
            objective,
            &eta,
            &coefficients,
            Some(objective),
            config,
        ),
        FinalHessianSynchronization::Certified
    );
    assert_eq!(
        strict_candidate_decision(
            true,
            true,
            2,
            1,
            &eta,
            &coefficients,
            objective,
            &eta,
            &coefficients,
            Some(objective),
            config,
        ),
        FinalHessianSynchronization::ResumeWithCandidate
    );
    assert_eq!(
        strict_candidate_decision(
            false,
            false,
            2,
            1,
            &eta,
            &coefficients,
            objective,
            &eta,
            &coefficients,
            Some(objective),
            config,
        ),
        FinalHessianSynchronization::ResumeAcceptedState
    );
}

#[test]
fn experimental_strict_guard_ignores_only_nonestimable_coefficient_slots() {
    let eta = [0.1, -0.2, 0.3];
    let coefficients = [0.4, 2.0];
    let moved_inactive = [0.4, 3.0];
    let moved_active = [0.5, 2.0];
    let objective = PoissonObjective {
        deviance: 2.5,
        log_likelihood: -1.25,
    };
    let config = PpmlConfig::default();

    assert_eq!(
        strict_candidate_decision_with_coefficient_slots(
            true,
            true,
            1,
            1,
            &eta,
            &coefficients,
            objective,
            &eta,
            &moved_inactive,
            Some(objective),
            config,
            Some(&[0]),
        ),
        FinalHessianSynchronization::Certified
    );
    assert_eq!(
        strict_candidate_decision_with_coefficient_slots(
            true,
            true,
            1,
            1,
            &eta,
            &coefficients,
            objective,
            &eta,
            &moved_active,
            Some(objective),
            config,
            Some(&[0]),
        ),
        FinalHessianSynchronization::ResumeWithCandidate
    );
    assert_eq!(
        strict_candidate_decision(
            true,
            true,
            1,
            1,
            &eta,
            &coefficients,
            objective,
            &eta,
            &moved_inactive,
            Some(objective),
            config,
        ),
        FinalHessianSynchronization::ResumeWithCandidate
    );
}

#[test]
fn same_rank_basis_pivot_resumes_first_checkpoint_and_fails_final_hessian_closed() {
    let run = |first_strict: bool, final_hessian: bool| {
        let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
        let eta = x
            .iter()
            .map(|value| 0.35_f64 + 0.42_f64 * value)
            .collect::<Vec<_>>();
        let outcome = eta
            .iter()
            .enumerate()
            .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
            .collect::<Vec<_>>();
        let prepared = prepared(&outcome, &[x]);
        let config = PreparedPpmlIrlsConfig::default();
        inject_strict_basis_change(first_strict, final_hessian);
        let result = fit_prepared_ppml_irls(
            &prepared,
            &outcome,
            &[1.0; 8],
            &[0.0; 8],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                .expect("same-rank pivot memory plan"),
            None,
        );
        (result, config)
    };

    let (resumed, config) = run(true, false);
    let resumed = resumed.expect("same-rank first-checkpoint pivot must resume IRLS");
    assert!(resumed.diagnostics().converged);
    assert!(resumed.diagnostics().strict_profile_rejections >= 1);
    assert!(resumed.diagnostics().iterations < config.ppml.max_iterations);

    let (failed, _) = run(false, true);
    let error = failed
        .err()
        .expect("same-rank final-Hessian pivot must never publish");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(
        error.message,
        "prepared PPML final Hessian changed the canonical rank basis"
    );
}

#[test]
fn strict_profile_requires_small_coefficient_newton_accuracy() {
    let eta = [0.1, -0.2, 0.3];
    let objective = PoissonObjective {
        deviance: 2.5,
        log_likelihood: -1.25,
    };
    for beta in [0.0_f64, -2e-4, 3e-3, 0.5, 4.0] {
        let allowance = (1e-11 + 1e-8 * beta.abs()).min(1e-8 * beta.abs().max(1.0));
        for (factor, expected) in [
            (0.5, FinalHessianSynchronization::Certified),
            (2.0, FinalHessianSynchronization::ResumeWithCandidate),
        ] {
            assert_eq!(
                strict_candidate_decision(
                    true,
                    true,
                    1,
                    1,
                    &eta,
                    &[beta],
                    objective,
                    &eta,
                    &[beta + factor * allowance],
                    Some(objective),
                    PpmlConfig::default(),
                ),
                expected,
                "beta={beta}, factor={factor}"
            );
        }
    }
    // Excluded normalization coordinates have no Newton authority. An
    // invalid active coordinate or slot must still fail closed.
    assert_eq!(
        strict_coefficient_difference(&[0.0, 2.0], &[0.0, 3.0], Some(&[0])),
        0.0
    );
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(strict_coefficient_difference(&[invalid], &[0.0], None).is_infinite());
        assert!(strict_coefficient_difference(&[0.0], &[invalid], Some(&[0])).is_infinite());
    }
    assert!(strict_coefficient_difference(&[0.0], &[], None).is_infinite());
    assert!(strict_coefficient_difference(&[0.0], &[0.0], Some(&[1])).is_infinite());
}

#[test]
fn strict_profile_movement_never_weakens_a_tiny_user_tolerance() {
    let eta = [0.1, -0.2, 0.3];
    let moved_eta = [0.1 + 1e-12, -0.2, 0.3];
    let coefficients = [0.4, -0.1];
    let objective = PoissonObjective {
        deviance: 2.5,
        log_likelihood: -1.25,
    };
    let config = PpmlConfig {
        tolerance: 1e-14,
        score_tolerance: 1e-14,
        ..PpmlConfig::default()
    };
    assert_eq!(
        strict_candidate_decision(
            true,
            true,
            2,
            2,
            &eta,
            &coefficients,
            objective,
            &moved_eta,
            &coefficients,
            Some(objective),
            config,
        ),
        FinalHessianSynchronization::ResumeWithCandidate
    );
}

#[test]
fn prepared_driver_reports_nonconvergence_atomically() {
    let outcome = vec![1.0, 2.0, 1.0, 3.0, 2.0, 4.0];
    let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    let prepared = prepared(&outcome, &[x]);
    let config = PreparedPpmlIrlsConfig {
        ppml: PpmlConfig {
            max_iterations: 1,
            ..PpmlConfig::default()
        },
        ..PreparedPpmlIrlsConfig::default()
    };
    let result = fit_prepared_ppml_irls(
        &prepared,
        &outcome,
        &[1.0; 6],
        &[0.0; 6],
        config,
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
            .expect("nonconvergence memory plan"),
        None,
    );
    let error = match result {
        Ok(_) => panic!("one iteration cannot satisfy two-pass convergence"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert!(error.message.contains("after 1 iterations"));
}

#[test]
fn intercept_initialization_and_backtracking_move_beta_and_eta_together() {
    let x = vec![-2.0, -1.0, 1.0, 2.0];
    let outcome = vec![0.1, 0.2, 5.0, 10.0];
    let prepared = prepared(&outcome, std::slice::from_ref(&x));
    let config = PreparedPpmlIrlsConfig::default();
    let design = prepared.fit_owned_fe_design().expect("fixture FE design");
    let (coefficients, eta) = initialize(
        &prepared,
        &outcome,
        &[1.0; 4],
        &[0.0; 4],
        &design,
        config,
        None,
        PreparedPpmlIrlsPurpose::Publication,
    )
    .expect("initial state");
    let intercept = prepared
        .fit_model()
        .columns
        .iter()
        .position(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
        .expect("intercept generated column");
    let intercept_slot = prepared
        .fit_model()
        .coefficients
        .iter()
        .position(|coefficient| coefficient.generated_column_index == Some(intercept))
        .expect("intercept slot");
    assert!(coefficients[intercept_slot].abs() > 0.1);
    for value in &eta {
        assert!((*value - coefficients[intercept_slot]).abs() <= 64.0 * f64::EPSILON);
    }

    let plan =
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).expect("memory plan");
    let mut workspace =
        PpmlIterationWorkspace::from_initial_state(plan, eta, coefficients).expect("workspace");
    workspace
        .refresh_working_state(&outcome, &[1.0; 4], &[0.0; 4], config.ppml.bounds)
        .expect("working state");
    let objective =
        poisson_objective_from_eta(&outcome, workspace.eta(), workspace.mean(), &[1.0; 4])
            .expect("objective");
    let initial_intercept = workspace.coefficients()[intercept_slot];
    let mut candidate = workspace.coefficients().to_vec();
    candidate[0] = 20.0;
    workspace
        .candidate_coefficients_mut()
        .copy_from_slice(&candidate);
    let candidate_eta = x
        .iter()
        .map(|value| initial_intercept + 20.0 * value)
        .collect::<Vec<_>>();
    workspace
        .set_candidate_eta(&candidate_eta)
        .expect("candidate eta");
    let accepted = workspace
        .accept_candidate(&outcome, &[1.0; 4], objective, config.ppml)
        .expect("backtracked improving step");
    assert!(accepted.step < 1.0);
    for (row, value) in x.iter().enumerate() {
        let reconstructed =
            workspace.coefficients()[intercept_slot] + workspace.coefficients()[0] * value;
        assert!((workspace.eta()[row] - reconstructed).abs() <= 1e-12);
    }
}

#[test]
fn strict_caller_error_and_unwind_restore_a_usable_prior_session() {
    let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
    let outcome = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
    let prepared = prepared(&outcome, &[x.clone(), x]);
    let frame = LinearSolveFrame::from_parts(
        &outcome,
        BorrowedMapWeights::Unit,
        PreparedOlsBreadWeightScale {
            weight_kind: None,
            solver_weight_divisor: 1.0,
            reference_weight_multiplier: 1.0,
            reference_weight_sum: outcome.len() as f64,
        },
    )
    .expect("strict caller frame");

    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut linear_config = PreparedOlsFitConfig::default();
        linear_config.execution.mode = mode;
        linear_config.execution.poolsize = 1;
        linear_config.coefficient_tolerance = Some(1e-12);
        let mut session = CertifiedLinearSession::new(
            &prepared,
            frame,
            linear_config,
            LinearSessionGenerations::new(0, 0, 0, 0, 7),
        )
        .expect("strict caller session");
        let initial = session.solve_fresh().expect("strict caller initial fit");
        let original_kept = initial.basis().kept_generated_columns().to_vec();
        let original_dropped = initial.basis().dropped_generated_columns().to_vec();
        let original_dense_identity = session.retained_dense_identity();
        let original_scratch_identity = session
            .retained_scratch_identity()
            .map(std::path::Path::to_path_buf);

        let before_error_capacities = session.workspace_capacities();
        let mut candidate_eta = vec![0.0; outcome.len()];
        let error = (|| -> Result<()> {
            let transaction = session.strict_candidate()?;
            let candidate = transaction.candidate();
            // This is a real fallible caller edge after candidate
            // construction. `?` must drop the still-live transaction.
            fill_candidate_eta_from_linear_output(&candidate, &[], &mut candidate_eta)?;
            transaction.commit();
            Ok(())
        })()
        .expect_err("misaligned candidate fill must fail");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(session.basis_generation_for_test(), 7);
        assert_eq!(
            session.workspace_capacities(),
            before_error_capacities,
            "error rollback must not allocate or discard reusable capacity",
        );
        assert_eq!(session.retained_dense_identity(), original_dense_identity);
        assert_eq!(
            session.retained_scratch_identity(),
            original_scratch_identity.as_deref(),
        );
        let restored = session
            .output()
            .expect("error rollback restores prior output");
        assert_eq!(restored.basis().kept_generated_columns(), original_kept);
        assert_eq!(
            restored.basis().dropped_generated_columns(),
            original_dropped
        );
        assert!(session
            .solve_frozen()
            .expect("error rollback supports the next frozen solve")
            .certified());
        let fresh_prior = session
            .solve_fresh()
            .expect("establish a certified fresh state before unwind");
        assert_eq!(fresh_prior.basis().kept_generated_columns(), original_kept);
        assert_eq!(
            fresh_prior.basis().dropped_generated_columns(),
            original_dropped
        );

        let before_unwind_capacities = session.workspace_capacities();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut transaction = session.strict_candidate().expect("strict unwind candidate");
            assert!(
                transaction.inject_same_rank_basis_pivot(),
                "duplicate columns provide a speculative same-rank pivot",
            );
            let candidate = transaction.candidate();
            assert!(candidate.certified());
            panic!("injected strict caller unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(session.basis_generation_for_test(), 7);
        assert_eq!(
            session.workspace_capacities(),
            before_unwind_capacities,
            "unwind rollback must not allocate or discard reusable capacity",
        );
        assert_eq!(session.retained_dense_identity(), original_dense_identity);
        assert_eq!(
            session.retained_scratch_identity(),
            original_scratch_identity.as_deref(),
        );
        let restored = session
            .output()
            .expect("unwind rollback restores prior output");
        assert_eq!(restored.basis().kept_generated_columns(), original_kept);
        assert_eq!(
            restored.basis().dropped_generated_columns(),
            original_dropped
        );
        assert!(session
            .solve_frozen()
            .expect("unwind rollback supports the next frozen solve")
            .certified());
    }
}

#[test]
fn ppml_topology_publication_follows_strict_commit_and_preserves_accepted_on_rollback() {
    let (prepared, outcome) = prepared_three_fe([0, 1, 2], false, None);
    let n = outcome.len();
    let weights = vec![1.; n];
    let mut config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-12).unwrap();
    config.execution.threads = crate::ThreadConfig::from_requested(1, true).unwrap();
    let frame = LinearSolveFrame::from_parts(
        &outcome,
        BorrowedMapWeights::Provided(&weights),
        ppml_bread_weight_scale(&weights).unwrap(),
    )
    .unwrap();
    let mut session = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        frame,
        config,
        LinearSessionGenerations::new(1, 1, 1, 1, 1),
        prepared.fit_owned_fe_design().unwrap(),
    )
    .unwrap();
    session.enable_ppml_certificate_geometry(u128::MAX).unwrap();
    session
        .enable_ppml_coefficient_solver(1, u128::MAX, Some(u128::MAX), Some(u128::MAX))
        .unwrap();
    session.enable_checkpoint_lsmr(true);
    session
        .enable_component_checkpoint_scaling(u128::MAX, 1)
        .unwrap();
    session.enable_ppml_topology(u128::MAX).unwrap();
    assert!(session.solve_fresh().unwrap().certified());
    assert_eq!(session.ppml_topology_state_for_test(), Some((false, true)));
    // A new Hessian frame discards the unaccepted producer before validation.
    let mut invalid = weights.clone();
    invalid[0] = 0.;
    assert!(session
        .refresh_weights(
            BorrowedMapWeights::Provided(&invalid),
            ppml_bread_weight_scale(&weights).unwrap(),
            2
        )
        .is_err());
    assert_eq!(session.ppml_topology_state_for_test(), Some((false, false)));
    // Enter a fresh weighted strict projection without an accepted topology.
    // Its candidate owns pending labels; unwinding must drop them silently.
    session.refresh_response(&outcome, 2).unwrap();
    session
        .refresh_weights(
            BorrowedMapWeights::Provided(&weights),
            ppml_bread_weight_scale(&weights).unwrap(),
            2,
        )
        .unwrap();
    let mut reached_pending = false;
    let pending_unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let transaction = session.strict_candidate().unwrap();
        assert!(transaction.candidate().certified());
        assert_eq!(
            transaction.ppml_topology_state_for_test(),
            Some((false, true))
        );
        reached_pending = true;
        panic!("pending topology caller unwind");
    }));
    assert!(pending_unwind.is_err());
    assert!(reached_pending);
    assert_eq!(session.ppml_topology_state_for_test(), Some((false, false)));
    assert!(session.solve_fresh().unwrap().certified());
    session.accept_pending_preconditioner();
    assert_eq!(session.ppml_topology_state_for_test(), Some((true, false)));
    for (generation, rollback) in [(3, true), (4, false)] {
        let current: Vec<_> = (0..n)
            .map(|row| 0.5 + ((row * 13 + generation) % 31) as f64 / 7.)
            .collect();
        session
            .refresh_response(&outcome, generation as u64)
            .unwrap();
        session
            .refresh_weights(
                BorrowedMapWeights::Provided(&current),
                ppml_bread_weight_scale(&current).unwrap(),
                generation as u64,
            )
            .unwrap();
        let transaction = session.strict_candidate().unwrap();
        assert!(transaction.candidate().certified());
        if rollback {
            transaction.rollback();
        } else {
            transaction.commit();
        }
        assert_eq!(session.ppml_topology_state_for_test(), Some((true, false)));
    }
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let transaction = session.strict_candidate().unwrap();
        assert!(transaction.candidate().certified());
        panic!("speculative topology caller unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(session.ppml_topology_state_for_test(), Some((true, false)));
    session.invalidate_sample(2).unwrap();
    assert_eq!(session.ppml_topology_state_for_test(), Some((false, false)));
    assert!(session.solve_fresh().is_err());
}

#[test]
fn ppml_topology_complete_irls_preserves_science_outer_iterations_and_solver_signatures() {
    let (prepared, outcome) = prepared_three_fe([0, 1, 2], false, None);
    let rows = outcome.len();
    let weights: Vec<_> = (0..rows).map(|row| 0.5 + (row % 11) as f64 / 7.).collect();
    for memory in [
        crate::options::MemoryMode::Fast,
        crate::options::MemoryMode::Low,
    ] {
        let run = |topology_sharing| {
            let mut config = PreparedPpmlIrlsConfig {
                topology_sharing,
                ..PreparedPpmlIrlsConfig::default()
            };
            config.memory.requested = memory;
            config.memory.poolsize = 1;
            config.linear.execution.threads = crate::ThreadConfig::from_requested(1, true).unwrap();
            fit_prepared_ppml_irls(
                &prepared,
                &outcome,
                &weights,
                &vec![0.; rows],
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                None,
            )
            .unwrap()
        };
        let baseline = run(false);
        let shared = run(true);
        assert_eq!(shared.coefficients(), baseline.coefficients());
        for (actual, expected) in [
            (shared.eta(), baseline.eta()),
            (shared.mean(), baseline.mean()),
            (shared.score_residuals(), baseline.score_residuals()),
            (
                shared.linear().identified_bread(),
                baseline.linear().identified_bread(),
            ),
        ] {
            assert_eq!(
                actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
            );
        }
        assert_eq!(shared.objective(), baseline.objective());
        assert_eq!(shared.linear().basis(), baseline.linear().basis());
        assert_eq!(
            shared.linear().generations(),
            baseline.linear().generations()
        );
        assert_eq!(shared.linear().weights(), baseline.linear().weights());
        let (a, b) = (shared.diagnostics(), baseline.diagnostics());
        assert_eq!(
            (
                a.iterations,
                a.total_inner_iterations,
                a.total_step_halvings,
                a.strict_profile_rejections
            ),
            (
                b.iterations,
                b.total_inner_iterations,
                b.total_step_halvings,
                b.strict_profile_rejections
            )
        );
        assert_eq!(a.final_score, b.final_score);
        assert_eq!(
            (
                a.solver.requested,
                a.solver.selected,
                a.solver.effective,
                a.solver.effective_engine,
                a.solver.fallback_reason
            ),
            (
                b.solver.requested,
                b.solver.selected,
                b.solver.effective,
                b.solver.effective_engine,
                b.solver.fallback_reason
            )
        );
        for (actual, expected) in a.solver.engines().iter().zip(b.solver.engines()) {
            let mut actual = *actual;
            let mut expected = *expected;
            for engine in [&mut actual, &mut expected] {
                engine.setup_seconds = 0.;
                engine.solve_seconds = 0.;
                engine.certification_seconds = 0.;
            }
            assert_eq!(actual, expected);
        }
        assert!(a.memory.plan.solver_workspace_bytes > b.memory.plan.solver_workspace_bytes);
        validate_final_hessian_frame(shared.linear().weights(), &weights, shared.mean()).unwrap();
    }
}

#[test]
fn prepared_iteration_report_fits_registered_memory_slot() {
    assert!(
        std::mem::size_of::<PreparedPpmlIterationReport>() as u128 <= DIAGNOSTIC_RECORD_BYTES,
        "prepared iteration report uses {} bytes",
        std::mem::size_of::<PreparedPpmlIterationReport>(),
    );
}

#[test]
fn owned_intercept_fe_design_peak_covers_the_allocated_topology() {
    for codes in [
        vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]],
        vec![
            vec![0, 0, 1, 1, 2, 2],
            vec![0, 1, 0, 1, 0, 1],
            vec![0, 1, 2, 0, 1, 2],
        ],
    ] {
        let levels: Vec<usize> = codes
            .iter()
            .map(|effect| effect.iter().copied().max().expect("nonempty test effect") as usize + 1)
            .collect();
        let rows = codes[0].len();
        let effects = codes
            .into_iter()
            .zip(&levels)
            .map(|(effect, levels)| {
                crate::fe::EffectTerm::new(effect, *levels, true, Vec::new())
                    .expect("intercept-only test effect")
            })
            .collect();
        let design = FeDesign::new(rows, effects).expect("intercept-only test design");
        let bound = conservative_owned_intercept_fe_design_peak_bytes(rows, &levels)
            .expect("owned topology peak");
        assert!(bound >= design.storage_capacities().resident_bytes);
    }
}

fn preconditioner_prepared_many(
    y: &[f64],
    xs: &[Vec<f64>],
    g: &[f64],
    h: &[f64],
) -> PreparedOlsData {
    let sources: Vec<_> = (0..xs.len() + 2)
        .map(|j| source(j as u32 + 1, &format!("x{j}")))
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
    bindings.extend(sources.iter().enumerate().map(|(j, s)| OlsInputBinding {
        stata_var_position: j as u32 + 3,
        role: OlsInputRole::NumericSource(s.id),
    }));
    let numeric_sources = sources
        .iter()
        .zip(xs.iter().map(Vec::as_slice).chain([g, h]))
        .map(|(s, v)| (s.id, loaded(v)))
        .collect();
    let absorbed_terms = (0..2)
        .map(|j| AbsorbedTerm {
            id: AbsorbTermId::new(j as u32 + 1).unwrap(),
            intercept: true,
            categorical_sources: vec![sources[xs.len() + j].id],
            slope_sources: vec![],
            label: format!("g{j}"),
            saved_name: String::new(),
        })
        .collect();
    let terms = sources[..xs.len()]
        .iter()
        .enumerate()
        .map(|(j, s)| continuous_term(j as u32 + 1, s.id))
        .collect();
    prepare_model(
        &OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms,
            bindings,
            include_intercept: true,
            keep_singletons: true,
        },
        &[],
        &LoadedOlsInput {
            candidate: vec![true; y.len()].into(),
            outcome: loaded(y),
            numeric_sources,
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        false,
    )
    .unwrap()
}

#[test]
fn historical_preconditioner_session_acceptance_weight_frames_checkpoint_and_rollback() {
    let n = 4096;
    let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 8 % 8) as f64).collect();
    let xs: Vec<Vec<_>> = (0..8)
        .map(|j| {
            (0..n)
                .map(|i| ((i * (2 * j + 17) % 997) as f64 / 19.).sin())
                .collect()
        })
        .collect();
    let y: Vec<_> = (0..n)
        .map(|i| 0.7 * xs[0][i] + g[i] * 0.2 - h[i] * 0.3 + ((i * 11 % 97) as f64).cos())
        .collect();
    let prepared = preconditioner_prepared_many(&y, &xs, &g, &h);
    let w = vec![1.; n];
    let mut config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-12).unwrap();
    config.execution.mode = PreparedOlsExecutionMode::Fast;
    config.execution.poolsize = 8;
    config.execution.threads = crate::options::ThreadConfig::from_requested(1, true).unwrap();
    let frame = LinearSolveFrame::from_parts(
        &y,
        BorrowedMapWeights::Provided(&w),
        ppml_bread_weight_scale(&w).unwrap(),
    )
    .unwrap();
    let mut session = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        frame,
        config,
        LinearSessionGenerations::new(1, 1, 1, 1, 1),
        prepared.fit_owned_fe_design().unwrap(),
    )
    .unwrap();
    session
        .enable_fast_canonical_products(16 * n as u128 * 8)
        .unwrap();
    session.enable_fast_preconditioner_reuse();
    assert!(session.solve_fresh().unwrap().certified());
    assert_eq!(session.preconditioner_state_for_test(), (None, true, 0));
    // Provisional producers are discarded when the caller does not accept.
    session
        .refresh_weights(
            BorrowedMapWeights::Provided(&w),
            ppml_bread_weight_scale(&w).unwrap(),
            2,
        )
        .unwrap();
    assert_eq!(session.preconditioner_state_for_test(), (None, false, 0));
    assert!(session.solve_fresh().unwrap().certified());
    session.accept_pending_preconditioner();
    assert_eq!(session.preconditioner_state_for_test().0, Some(2));
    for generation in 3..6 {
        session
            .refresh_weights(
                BorrowedMapWeights::Provided(&w),
                ppml_bread_weight_scale(&w).unwrap(),
                generation,
            )
            .unwrap();
    }
    let transaction = session.strict_checkpoint_candidate().unwrap();
    assert!(transaction.candidate().certified());
    transaction.commit();
    assert!(session.preconditioner_state_for_test().2 > 0);
    assert_eq!(session.preconditioner_state_for_test().0, Some(2));
    let transaction = session.strict_checkpoint_candidate().unwrap();
    transaction.rollback();
    assert_eq!(session.preconditioner_state_for_test().0, None);
    assert!(!session.preconditioner_state_for_test().1);
    assert!(session.solve_fresh().unwrap().certified());
    session.accept_pending_preconditioner();
    let transaction = session.strict_candidate().unwrap();
    transaction.commit();
    assert_eq!(
        session.preconditioner_state_for_test().2,
        0,
        "accepted-Hessian/default strict stays cold"
    );
    for invalidation in 0..3 {
        let frame = LinearSolveFrame::from_parts(
            &y,
            BorrowedMapWeights::Provided(&w),
            ppml_bread_weight_scale(&w).unwrap(),
        )
        .unwrap();
        let mut next = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            LinearSessionGenerations::new(1, 1, 1, 1, 1),
            prepared.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        next.enable_fast_canonical_products(16 * n as u128 * 8)
            .unwrap();
        next.enable_fast_preconditioner_reuse();
        next.solve_fresh().unwrap();
        next.accept_pending_preconditioner();
        assert_eq!(next.preconditioner_state_for_test().0, Some(1));
        match invalidation {
            0 => next.invalidate_sample(2).unwrap(),
            1 => next.invalidate_topology(2).unwrap(),
            _ => next.refresh_basis(2).unwrap(),
        };
        assert_eq!(next.preconditioner_state_for_test().0, None);
        assert!(!next.preconditioner_state_for_test().1);
        assert!(next.strict_checkpoint_candidate().is_err());
    }
}

#[test]
fn historical_preconditioner_complete_ppml_known_optimum_hits_and_matches_cold() {
    let n = 8192;
    // Every pair has identical design and opposite Poisson disturbances.
    let g: Vec<_> = (0..n).map(|i| (i / 2 % 8) as f64).collect();
    let h: Vec<_> = (0..n).map(|i| (i / 16 % 8) as f64).collect();
    let xs: Vec<Vec<_>> = (0..8)
        .map(|j| {
            (0..n)
                .map(|i| ((i / 2 * (2 * j + 17) % 997) as f64 / 19.).sin())
                .collect()
        })
        .collect();
    let truth: Vec<_> = (0..n)
        .map(|i| {
            (0.2 + g[i] * 0.08 - h[i] * 0.03
                + (0..8)
                    .map(|j| 0.07 / (j + 1) as f64 * xs[j][i])
                    .sum::<f64>())
            .exp()
        })
        .collect();
    let y: Vec<_> = truth
        .iter()
        .enumerate()
        .map(|(i, m)| m * if i % 2 == 0 { 0.8 } else { 1.2 })
        .collect();
    let prepared = preconditioner_prepared_many(&y, &xs, &g, &h);
    let mut config = PreparedPpmlIrlsConfig::default();
    config.linear.execution.mode = PreparedOlsExecutionMode::Fast;
    config.linear.execution.poolsize = 8;
    config.memory.requested = crate::options::MemoryMode::Fast;
    config.memory.poolsize = 8;
    config.linear.execution.threads =
        crate::options::ThreadConfig::from_requested(1, true).unwrap();
    let before = CertifiedLinearSession::<PreparedOlsData>::preconditioner_test_hits();
    let fit = fit_prepared_ppml_irls(
        &prepared,
        &y,
        &vec![1.; n],
        &vec![0.; n],
        config,
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
        None,
    )
    .unwrap();
    assert!(
        CertifiedLinearSession::<PreparedOlsData>::preconditioner_test_hits() > before,
        "full fit must consume accepted producer"
    );
    assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
    assert!(relative_infinity_difference(fit.mean(), &truth) < 1e-10);
    for j in 0..8 {
        assert!((fit.coefficients()[j].value.unwrap() - 0.07 / (j + 1) as f64).abs() < 1e-10);
    }
    validate_final_hessian_frame(fit.linear().weights(), &vec![1.; n], fit.mean()).unwrap();
    config.solver_plan.requested = crate::PpmlSolverFamily::Map;
    let cold = fit_prepared_ppml_irls(
        &prepared,
        &y,
        &vec![1.; n],
        &vec![0.; n],
        config,
        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
        None,
    )
    .unwrap();
    assert!(relative_infinity_difference(fit.mean(), cold.mean()) < 1e-10);
    assert!(
        relative_infinity_difference(
            fit.linear().identified_bread(),
            cold.linear().identified_bread()
        ) < 1e-10
    );
}
