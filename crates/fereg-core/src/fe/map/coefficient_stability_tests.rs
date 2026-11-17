use super::*;
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder,
    ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent,
    TermId, TermOrder, TimeSeriesState,
};
use crate::ols::{
    LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedOlsFitConfig,
};
use crate::options::{MemoryMode, ThreadConfig};
use crate::ppml::*;

struct Fixture {
    columns: Vec<Vec<f64>>,
    outcome: Vec<f64>,
    weights: Vec<f64>,
    half_source: usize,
    truth: Vec<f64>,
}

fn source_id(index: usize) -> SourceId {
    SourceId::new((index + 1) as u32).unwrap()
}
fn loaded(values: &[f64]) -> LoadedNumericColumn {
    LoadedNumericColumn {
        values: values.to_vec(),
        missing: vec![false; values.len()],
    }
}
fn continuous(slot: usize, source: usize) -> ExpandedTerm {
    let id = slot as u32 + 1;
    ExpandedTerm {
        id: TermId::new(id).unwrap(),
        order: TermOrder::new(id).unwrap(),
        coefficient_slot: CoefficientSlot::new(id).unwrap(),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("c.x{source}"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).unwrap(),
            source: source_id(source),
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".into(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn model(
    fixture: &Fixture,
    finite: &[usize],
    intercept_slope: bool,
) -> (OlsRequestV1, LoadedOlsInput) {
    let used: Vec<_> = (0..fixture.columns.len())
        .filter(|i| finite.contains(i) || (8..=11).contains(i) || *i == fixture.half_source)
        .collect();
    let used_id = |source: usize| source_id(used.iter().position(|&i| i == source).unwrap());
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
    bindings.extend(
        used.iter()
            .enumerate()
            .map(|(position, &i)| OlsInputBinding {
                stata_var_position: (position + 3) as u32,
                role: OlsInputRole::NumericSource(used_id(i)),
            }),
    );
    bindings.push(OlsInputBinding {
        stata_var_position: (used.len() + 3) as u32,
        role: OlsInputRole::ProbabilityWeight,
    });
    let mut absorbed_terms: Vec<_> = (0..3)
        .map(|i| AbsorbedTerm {
            id: AbsorbTermId::new(i + 1).unwrap(),
            intercept: true,
            categorical_sources: vec![used_id(i as usize + 8), used_id(fixture.half_source)],
            slope_sources: vec![],
            label: format!("fe{i}"),
            saved_name: String::new(),
        })
        .collect();
    absorbed_terms.push(AbsorbedTerm {
        id: AbsorbTermId::new(4).unwrap(),
        intercept: intercept_slope,
        categorical_sources: vec![used_id(8)],
        slope_sources: vec![used_id(11)],
        label: "period#c.z".into(),
        saved_name: String::new(),
    });
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: used
                .iter()
                .copied()
                .map(|i| SourceEntry {
                    id: used_id(i),
                    canonical_name: format!("x{i}"),
                })
                .collect(),
            terms: finite
                .iter()
                .enumerate()
                .map(|(slot, &source)| {
                    let mut term = continuous(slot, source);
                    term.components[0].source = used_id(source);
                    term
                })
                .collect(),
        },
        absorbed_terms,
        bindings,
        include_intercept: true,
        keep_singletons: false,
    };
    let input = LoadedOlsInput {
        candidate: vec![true; fixture.outcome.len()].into(),
        outcome: loaded(&fixture.outcome),
        numeric_sources: used
            .iter()
            .map(|&i| (used_id(i), loaded(&fixture.columns[i])))
            .collect(),
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: Some(loaded(&fixture.weights)),
    };
    (request, input)
}

fn fit(
    fixture: &Fixture,
    finite: &[usize],
    intercept_slope: bool,
    threads: usize,
    mode: MemoryMode,
) -> CertifiedPpmlResult {
    let (ols, input) = model(fixture, finite, intercept_slope);
    let mut linear = PreparedOlsFitConfig::default();
    linear.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
    let request = PreparedPpmlRequestV1 {
        bias: None,
        ols,
        clusters: vec![],
        vce: PpmlVceRequest::Robust,
        ppml: PpmlConfig::default(),
        linear,
        memory: PpmlMemoryConfig {
            requested: mode,
            poolsize: 3,
            ..PpmlMemoryConfig::default()
        },
        memory_preflight_probe: None,
        allocation_snapshot_probe: None,
        phase_allocation_probe: None,
        linear_phase_allocation_probe: None,
        action_allocation_snapshot_probe: None,
        action_allocation_start: None,
        action_allocation_previous_baseline_bytes: None,
        separation: crate::ppml_separation::SeparationConfig::default(),
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
    fit_prepared_ppml(
        request,
        LoadedPpmlInput {
            bias_units: None,
            ols: input,
            adjustment: None,
            adjustment_kind: PreparedPpmlAdjustmentKind::None,
        },
    )
    .unwrap_or_else(|e| {
        panic!("intercept_slope={intercept_slope} threads={threads} mode={mode:?}: {e:?}")
    })
}

fn generated_fixture() -> Fixture {
    let n = 1728;
    let half: Vec<_> = (0..n).map(|i| (i % 2) as f64).collect();
    let x: Vec<_> = (0..n)
        .map(|i| ((i + 1) as f64 * 0.61).sin() + (i / 216) as f64 / 19.)
        .collect();
    let outcome = (0..n)
        .map(|i| {
            let h = half[i];
            let c1 = 1 + (i / 2) % 3;
            let c2 = 1 + (i / 6) % 2;
            (0.2 * x[i] + 0.3 * h * f64::from(c1 == 3) - 0.15 * (1. - h) * f64::from(c1 == 3)
                + 0.1 * h * f64::from(c2 == 2)
                + 0.04 * (1 + (i / 12) % 3) as f64
                - 0.03 * (1 + (i / 36) % 3) as f64
                + 0.02 * (1 + (i / 108) % 2) as f64)
                .exp()
                * (0.8 + 0.1 * ((i / 216) % 3) as f64 + 0.05 * ((i + 1) as f64 / 7.).sin())
        })
        .collect();
    let mut columns = vec![x, half];
    for (which, level) in [(1, 3), (2, 2), (1, 1)] {
        for h in 0..2 {
            columns.push(
                (0..n)
                    .map(|i| {
                        f64::from(
                            i % 2 == h
                                && if which == 1 {
                                    1 + (i / 2) % 3 == level
                                } else {
                                    1 + (i / 6) % 2 == level
                                },
                        )
                    })
                    .collect(),
            );
        }
    }
    for (divisor, levels) in [(12, 3), (36, 3), (108, 2)] {
        columns.push(
            (0..n)
                .map(|i| (1 + (i / divisor) % levels) as f64)
                .collect(),
        );
    }
    columns.push((0..n).map(|i| ((i + 1) as f64 * 0.23).cos()).collect());
    Fixture {
        columns,
        outcome,
        weights: (0..n).map(|i| 0.7 + ((i + 1) % 7) as f64 / 5.).collect(),
        half_source: 1,
        truth: vec![],
    }
}

#[test]
fn exact_redundant_half_preserves_public_ppml_fit_across_coordinates_and_workers() {
    let fixture = generated_fixture();
    let reduced = [0, 2, 3, 4, 5, 6, 7];
    let reference = fit(&fixture, &reduced, false, 1, MemoryMode::Fast);
    for intercept_slope in [false, true] {
        for threads in [1, 4] {
            let result = fit(
                &fixture,
                &[0, 1, 2, 3, 4, 5, 6, 7],
                intercept_slope,
                threads,
                MemoryMode::Fast,
            );
            assert_eq!(
                result.sample().original_rows(),
                reference.sample().original_rows()
            );
            assert_eq!(result.coefficients().kept_full_slots().len(), 7);
            assert_eq!(
                result.coefficients().statuses()[1],
                PpmlCoefficientStatus::CollinearWithFixedEffects
            );
            for (&actual, &expected) in result.output().eta().iter().zip(reference.output().eta()) {
                assert!((actual - expected).abs() < 1e-8);
            }
            for (slot, &source) in reduced.iter().enumerate() {
                assert!(
                    (result.coefficients().values()[source]
                        - reference.coefficients().values()[slot])
                        .abs()
                        < 1e-8
                );
            }
            let repeated = fit(
                &fixture,
                &[0, 1, 2, 3, 4, 5, 6, 7],
                intercept_slope,
                threads,
                MemoryMode::Fast,
            );
            assert_eq!(result.coefficients(), repeated.coefficients());
            assert_eq!(result.output().eta(), repeated.output().eta());
        }
    }
    let low = fit(
        &fixture,
        &[0, 1, 2, 3, 4, 5, 6, 7],
        false,
        1,
        MemoryMode::Low,
    );
    assert_eq!(
        low.sample().original_rows(),
        reference.sample().original_rows()
    );
    for (&actual, &expected) in low.output().eta().iter().zip(reference.output().eta()) {
        assert!((actual - expected).abs() < 1e-8);
    }
}

fn walsh_fixture(delta: f64, weight_scale: f64) -> Fixture {
    let mut fixture = Fixture {
        columns: vec![Vec::new(); 13],
        outcome: vec![],
        weights: vec![],
        half_source: 12,
        truth: vec![],
    };
    let sign = |row: usize, mask| if row & mask == 0 { 1. } else { -1. };
    for cell in 0..36 {
        let (half, period, entry, family) = (
            (cell % 2) as f64,
            (1 + (cell / 2) % 3) as f64,
            (1 + (cell / 6) % 3) as f64,
            (1 + (cell / 18) % 2) as f64,
        );
        for row in 0..16 {
            let (z, v, x, error) = (sign(row, 1), sign(row, 2), sign(row, 4), sign(row, 8));
            let weak = half + delta * v;
            let eta = 0.3 * x
                + if delta == 0. {
                    0.
                } else {
                    (0.2 / delta) * (weak - half)
                }
                + 0.05 * period * z
                + 0.02 * entry
                - 0.03 * family
                + 0.1 * half;
            for (column, value) in fixture.columns.iter_mut().zip([
                x, weak, 0., 0., 0., 0., 0., 0., period, entry, family, z, half,
            ]) {
                column.push(value);
            }
            fixture.outcome.push(eta.exp() * (1. + 0.1 * error));
            fixture.weights.push(weight_scale * (1 + cell % 5) as f64);
            fixture.truth.push(eta);
        }
    }
    fixture
}

#[test]
fn weak_contrast_retains_rank_and_analytic_ppml_coefficients() {
    // The error Walsh sign varies independently of every design column and
    // eta. Its paired scores cancel, so exp(truth) is the exact finite MLE.
    // The nonzero contrast is above the existing FE-collinearity threshold.
    for delta in [0., 1e-3] {
        for scale in [1., 1e6] {
            let fixture = walsh_fixture(delta, scale);
            for intercept_slope in [false, true] {
                for threads in [1, 4] {
                    let result = fit(
                        &fixture,
                        &[0, 1],
                        intercept_slope,
                        threads,
                        MemoryMode::Fast,
                    );
                    assert_eq!(
                        result.coefficients().kept_full_slots().len(),
                        if delta == 0. { 1 } else { 2 }
                    );
                    assert!((result.coefficients().values()[0] - 0.3).abs() < 1e-8);
                    if delta == 0. {
                        assert_eq!(
                            result.coefficients().statuses()[1],
                            PpmlCoefficientStatus::CollinearWithFixedEffects
                        );
                    } else {
                        assert!((result.coefficients().values()[1] - 0.2 / delta).abs() < 1e-5);
                    }
                    for (&actual, &expected) in result.output().eta().iter().zip(&fixture.truth) {
                        assert!(
                            (actual - expected).abs() < 1e-8,
                            "delta={delta} scale={scale} actual={actual} expected={expected}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn repeated_slope_projection_preserves_a_weak_orthogonal_contrast() {
    for delta in [0., 1e-3] {
        for scale in [1., 1e6] {
            let fixture = walsh_fixture(delta, scale);
            let n = fixture.outcome.len();
            for intercept_slope in [false, true] {
                let mut effects: Vec<_> = [3, 3, 2]
                    .into_iter()
                    .enumerate()
                    .map(|(which, levels)| {
                        EffectTerm::new(
                            (0..n)
                                .map(|row| {
                                    (fixture.columns[8 + which][row] as u32 - 1) * 2
                                        + fixture.columns[fixture.half_source][row] as u32
                                })
                                .collect(),
                            levels * 2,
                            true,
                            vec![],
                        )
                        .unwrap()
                    })
                    .collect();
                effects.push(
                    EffectTerm::new(
                        fixture.columns[8].iter().map(|v| *v as u32 - 1).collect(),
                        3,
                        intercept_slope,
                        vec![fixture.columns[11].clone()],
                    )
                    .unwrap(),
                );
                let design = FeDesign::new(n, effects).unwrap();
                for threads in [1, 4] {
                    let mut owner = ReusableMapAbsorber::new(design.clone(), 2).unwrap();
                    owner.enable_certificate_geometry().unwrap();
                    owner.enable_coefficient_solver(threads).unwrap();
                    owner.enable_certificate_workspace().unwrap();
                    owner.enable_coefficient_coupling().unwrap();
                    let capacity = owner.capacities();
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();
                    let mut values = DenseMatrix::from_columns(&[
                        fixture.columns[1].clone(),
                        fixture.columns[0].clone(),
                    ])
                    .unwrap();
                    for round in 0..4 {
                        let result = pool
                            .install(|| {
                                owner.absorb(
                                    &fixture.weights,
                                    values,
                                    MapConfig {
                                        tolerance: if round == 0 { 1e-6 } else { 1e-14 },
                                        ..MapConfig::default()
                                    },
                                    false,
                                )
                            })
                            .unwrap();
                        assert!(result.report().certified());
                        values = result.residualized().clone();
                        for row in 0..n {
                            let expected =
                                fixture.columns[1][row] - fixture.columns[fixture.half_source][row];
                            assert!(
                                (values[(row, 0)] - expected).abs() < if round == 0 { 1e-5 } else { 1e-12 },
                                "delta={delta} scale={scale} round={round} actual={} expected={expected}", values[(row, 0)]
                            );
                            assert!(
                                (values[(row, 1)] - fixture.columns[0][row]).abs()
                                    < if round == 0 { 1e-5 } else { 1e-12 }
                            );
                        }
                    }
                    assert_eq!(capacity, owner.capacities());
                }
            }
        }
    }
}
