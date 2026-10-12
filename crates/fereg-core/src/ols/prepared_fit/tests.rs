/// Fit a prepared OLS model around an explicitly borrowed frame through
/// the production Fast/Low frame entries (test comparator for sessions).
fn fit_prepared_weighted_linear_once<'a, P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    ThreadTeam::new(config.execution.threads).install(|| {
        validate_config(prepared, config)?;
        frame.validate_for(prepared)?;
        let mut reservation = SystemFitReservation;
        match config.execution.mode {
            PreparedOlsExecutionMode::Fast => fit_with_controls_internal_mode_with_frame(
                prepared,
                frame,
                config,
                &mut reservation,
                FitPerturbations::default(),
                false,
                MAX_FAST_SCORE_CACHE_BYTES,
                None,
                None,
                None,
            ),
            PreparedOlsExecutionMode::Low => fit_with_controls_low_internal_with_frame(
                prepared,
                frame,
                config,
                &mut reservation,
                FitPerturbations::default(),
                None,
                None,
                None,
            ),
        }
    })?
}

use super::*;
use crate::fe::{recover_finalized_categorical_effects_map_certified, MapConfig};
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
    LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent, TermId, TermOrder,
    TimeSeriesState,
};
use crate::ols::finite_wls_kernel::{
    begin_retained_moment_worker_observation, finish_retained_moment_worker_observation,
};
use crate::ols::{
    begin_frozen_qr_fallback_observation, finish_frozen_qr_fallback_observation, prepare_model,
    CertifiedLinearSession, CertifiedLinearSessionOutput, CertifiedLinearSessionProducts,
    CertifiedLinearWithinDesign, LinearSessionAllocationPhase, LinearSessionGenerations,
    LinearSessionPhaseAllocationEvent, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding,
    OlsInputRole, OlsRequestV1,
};
use crate::sandwich::SandwichDesign;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The deleted categorical recovery wrapper tightened the MAP tolerance to
/// this ceiling before recovering slope channels; the recovery test keeps
/// that exact configuration.
const CATEGORICAL_SLOPE_MAP_TOLERANCE_CEILING: f64 = 1e-12;

fn categorical_slope_map_config(mut config: MapConfig) -> MapConfig {
    // Group-specific slopes can amplify the final raw finite-score moment even
    // after every FE moment has reached the generic MAP tolerance. Keep the
    // registered finite-score gate unchanged and require a tighter upstream
    // projection certificate for this path.
    config.tolerance = config
        .tolerance
        .min(CATEGORICAL_SLOPE_MAP_TOLERANCE_CEILING);
    config
}

#[test]
fn qr_classification_preserves_norm_errors_and_deferred_intercept_order() {
    let y = [0.0; 6];
    let groups = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let data = prepared(&y, &[y.to_vec()], &[false], None, false, TestWeights::Unit);
    let intercept = prepared(&y, &[], &[], Some(&groups), true, TestWeights::Unit);
    let run = |data: &PreparedOlsData, reference, score, low| {
        let frame = LinearSolveFrame::from_prepared_ols(data).unwrap();
        let config = PreparedOlsFitConfig::default();
        if low {
            finite::solve_one_shot(
                data,
                finite::ProjectedFiniteRows::Scratch {
                    scratch: &PreparedOlsScratch::create_row_major(6, 1).unwrap(),
                    outcome: &[0.0; 6],
                },
                frame.weights(),
                frame.bread_weight_scale(),
                &[reference],
                &[score],
                config,
                &mut SystemFitReservation,
                false,
                false,
                true,
            )
        } else {
            finite::solve_one_shot(
                data,
                finite::ProjectedFiniteRows::Dense(&DenseMatrix::zeros(6, 2).unwrap()),
                frame.weights(),
                frame.bread_weight_scale(),
                &[reference],
                &[score],
                config,
                &mut SystemFitReservation,
                false,
                false,
                false,
            )
        }
    };
    let reference_error = "prepared OLS reference regressor norm is invalid";
    let ratio_error = "prepared OLS fixed-effect collinearity ratio is not finite";
    let square_error = "prepared OLS fixed-effect squared collinearity ratio is not finite";
    let low_norm_error = "prepared OLS low-memory regressor norm is invalid";
    let low_ratio_error = "prepared OLS low-memory collinearity ratio is invalid";
    // A zero reference short-circuits fast validation, but low validates
    // both norms first. These are caller boundaries, not memory-mode rules
    // to introduce into the shared classification implementation.
    for (reference, score, fast, low) in [
        (-1.0, f64::NAN, Some(reference_error), Some(low_norm_error)),
        (f64::NAN, 0.0, Some(reference_error), Some(low_norm_error)),
        (0.0, f64::NAN, None, Some(low_norm_error)),
        (1.0, f64::NAN, Some(ratio_error), Some(low_norm_error)),
        (1e-300, 1e300, Some(ratio_error), Some(low_ratio_error)),
        (1.0, 1e200, Some(square_error), Some(low_ratio_error)),
        (0.0, 0.0, None, None),
    ] {
        for (low_mode, expected) in [(false, fast), (true, low)] {
            let result = run(&data, reference, score, low_mode);
            if let Some(message) = expected {
                let error = result.unwrap_err();
                assert_eq!(error.code, ErrorCode::InvalidData);
                assert_eq!(error.message, message);
            } else {
                let qr = result.unwrap();
                assert_eq!(qr.rank, 0);
                assert_eq!(
                    qr.generated_statuses,
                    [PreparedOlsFitStatus::NumericallyRankDeficient]
                );
            }
        }
    }
    for low in [false, true] {
        let qr = run(&intercept, f64::NAN, f64::NAN, low).unwrap();
        assert_eq!(qr.rank, 0);
        assert_eq!(
            qr.generated_statuses,
            [PreparedOlsFitStatus::AbsorbedInterceptDeferred]
        );
    }
}

#[test]
fn current_frame_means_select_canonical_coordinates_and_reject_invalid_views() {
    let known = CurrentFrameColumnMeans {
        generated_columns: &[0, 2, 5],
        values: &[1.0, -3.0, 7.0],
    };
    let mut output = [0.0; 2];
    known.fill_selected(&[5, 0], &mut output).unwrap();
    assert_eq!(output, [7.0, 1.0]);
    assert!(known.fill_selected(&[1, 0], &mut output).is_err());
    assert!(known.fill_selected(&[0], &mut output).is_err());
    for bad in [
        CurrentFrameColumnMeans {
            generated_columns: &[1, 1],
            values: &[1.0, 2.0],
        },
        CurrentFrameColumnMeans {
            generated_columns: &[2, 1],
            values: &[1.0, 2.0],
        },
        CurrentFrameColumnMeans {
            generated_columns: &[0, 1],
            values: &[1.0],
        },
        CurrentFrameColumnMeans {
            generated_columns: &[0, 1],
            values: &[1.0, f64::NAN],
        },
    ] {
        assert!(bad.fill_selected(&[0, 1], &mut output).is_err());
    }
}

static ACTIVE_LINEAR_ALLOCATION_PHASE: AtomicUsize = AtomicUsize::new(0);
static LINEAR_PROJECTION_ENDS: AtomicUsize = AtomicUsize::new(0);
static LINEAR_FINITE_ENDS: AtomicUsize = AtomicUsize::new(0);

fn assert_retained_moment_team(observed: (usize, u64), expected: usize) {
    assert_eq!(
        observed.0,
        expected,
        "retained kernel escaped configured pool; observed participants={}",
        observed.1.count_ones(),
    );
}

fn observed_linear_allocation_phase(
    phase: LinearSessionAllocationPhase,
    event: LinearSessionPhaseAllocationEvent,
) -> u64 {
    let phase_id = match phase {
        LinearSessionAllocationPhase::Projection => 1,
        LinearSessionAllocationPhase::FiniteSolve => 2,
    };
    match event {
        LinearSessionPhaseAllocationEvent::Begin => {
            assert_eq!(
                ACTIVE_LINEAR_ALLOCATION_PHASE.swap(phase_id, Ordering::SeqCst),
                0,
                "linear allocation phases must never overlap"
            );
            0
        }
        LinearSessionPhaseAllocationEvent::End => {
            assert_eq!(
                ACTIVE_LINEAR_ALLOCATION_PHASE.swap(0, Ordering::SeqCst),
                phase_id,
                "linear allocation phase end must match its begin"
            );
            match phase {
                LinearSessionAllocationPhase::Projection => {
                    LINEAR_PROJECTION_ENDS.fetch_add(1, Ordering::SeqCst);
                    11
                }
                LinearSessionAllocationPhase::FiniteSolve => {
                    LINEAR_FINITE_ENDS.fetch_add(1, Ordering::SeqCst);
                    23
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum TestWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [f64]),
}

#[derive(Default)]
struct InjectedReservation {
    calls: usize,
    fail_at: Option<usize>,
    undersize: bool,
    records: Vec<(usize, &'static str)>,
}

impl FitReservation for InjectedReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        self.calls += 1;
        self.records.push((elements, label));
        if self.fail_at == Some(self.calls) {
            return Err(allocation("injected prepared OLS fit reservation failure"));
        }
        if !self.undersize {
            target
                .try_reserve_exact(elements)
                .map_err(|_| allocation("test prepared OLS fit reservation failure"))?;
        }
        Ok(())
    }
}

fn source(id: u32, name: &str) -> SourceEntry {
    SourceEntry {
        id: SourceId::new(id).expect("source"),
        canonical_name: name.to_owned(),
    }
}

fn continuous_term(id: u32, source: SourceId, omitted: bool) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("order"),
        coefficient_slot: CoefficientSlot::new(id).expect("slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: omitted,
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

fn categorical_term(id: u32, source: SourceId, level: f64, omitted: bool) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("order"),
        coefficient_slot: CoefficientSlot::new(id).expect("slot"),
        raw_kind: RawTermKind::Factor,
        whole_column_omitted: omitted,
        canonical_token: format!("{level}.g"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component"),
            source,
            kind: ComponentKind::Categorical,
            level_state: LevelState::Present,
            level_value: level,
            base: if omitted {
                MetadataFlag::True
            } else {
                MetadataFlag::False
            },
            omitted: if omitted {
                MetadataFlag::True
            } else {
                MetadataFlag::False
            },
            raw_operator: level.to_string(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn categorical_interaction_term(id: u32, components: &[(SourceId, f64)]) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("order"),
        coefficient_slot: CoefficientSlot::new(id).expect("slot"),
        raw_kind: RawTermKind::Interaction,
        whole_column_omitted: false,
        canonical_token: "categorical-interaction".to_owned(),
        components: components
            .iter()
            .copied()
            .enumerate()
            .map(|(index, (source, level))| TermComponent {
                order: ComponentOrder::new(u32::try_from(index + 1).expect("small component"))
                    .expect("component"),
                source,
                kind: ComponentKind::Categorical,
                level_state: LevelState::Present,
                level_value: level,
                base: MetadataFlag::False,
                omitted: MetadataFlag::False,
                raw_operator: level.to_string(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            })
            .collect(),
    }
}

fn loaded(values: &[f64]) -> LoadedNumericColumn {
    LoadedNumericColumn {
        values: values.to_vec(),
        missing: vec![false; values.len()],
    }
}

fn prepared(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    omitted: &[bool],
    fixed_effect: Option<&[f64]>,
    include_intercept: bool,
    weights: TestWeights<'_>,
) -> PreparedOlsData {
    assert_eq!(regressors.len(), omitted.len());
    let mut sources = Vec::new();
    let mut terms = Vec::new();
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
    let mut numeric_sources = Vec::new();
    for (index, values) in regressors.iter().enumerate() {
        let one_based = u32::try_from(index + 1).expect("small source");
        let entry = source(one_based, &format!("x{one_based}"));
        terms.push(continuous_term(one_based, entry.id, omitted[index]));
        bindings.push(OlsInputBinding {
            stata_var_position: one_based + 2,
            role: OlsInputRole::NumericSource(entry.id),
        });
        numeric_sources.push((entry.id, loaded(values)));
        sources.push(entry);
    }
    let absorbed_terms = fixed_effect.map_or_else(Vec::new, |codes| {
        let one_based = u32::try_from(sources.len() + 1).expect("small FE source");
        let entry = source(one_based, "g");
        bindings.push(OlsInputBinding {
            stata_var_position: one_based + 2,
            role: OlsInputRole::NumericSource(entry.id),
        });
        numeric_sources.push((entry.id, loaded(codes)));
        sources.push(entry.clone());
        vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("absorbed term"),
            intercept: true,
            categorical_sources: vec![entry.id],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: String::new(),
        }]
    });
    let weight_position = u32::try_from(bindings.len() + 1).expect("small binding");
    let (analytic_weight, probability_weight, frequency_weight) = match weights {
        TestWeights::Unit => (None, None, None),
        TestWeights::Analytic(values) => {
            bindings.push(OlsInputBinding {
                stata_var_position: weight_position,
                role: OlsInputRole::AnalyticWeight,
            });
            (Some(loaded(values)), None, None)
        }
        TestWeights::Probability(values) => {
            bindings.push(OlsInputBinding {
                stata_var_position: weight_position,
                role: OlsInputRole::ProbabilityWeight,
            });
            (None, Some(loaded(values)), None)
        }
        TestWeights::Frequency(values) => {
            bindings.push(OlsInputBinding {
                stata_var_position: weight_position,
                role: OlsInputRole::FrequencyWeight,
            });
            (None, None, Some(loaded(values)))
        }
    };
    prepare_model(
        &OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms,
            bindings,
            include_intercept,
            keep_singletons: true,
        },
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(outcome),
            numeric_sources,
            analytic_weight,
            frequency_weight,
            probability_weight,
        },
        false,
    )
    .expect("prepared data")
}

#[test]
fn discarded_workspace_is_merged_once_even_without_a_setup_timer_or_warning() {
    use crate::memory::SolverWorkspacePeak;
    let y = [1.0, -1.0, 2.0, -2.0];
    let g = [0.0, 0.0, 1.0, 1.0];
    let data = prepared(&y, &[], &[], Some(&g), false, TestWeights::Unit);
    let config = PreparedOlsFitConfig::default();
    for discarded in [
        SolverWorkspacePeak::from_bytes(1 << 28).unwrap(),
        SolverWorkspacePeak::UNMODELED,
    ] {
        let mut session =
            crate::fe::FeSession::for_ols(&data, BorrowedMapWeights::Unit, config).unwrap();
        session.peak = discarded;
        for _ in 0..2 {
            let result = session
                .project_ols(
                    BorrowedMapWeights::Unit,
                    DenseMatrix::from_columns(&[y.to_vec()]).unwrap(),
                    config.absorb,
                    false,
                    false,
                )
                .unwrap();
            assert!(result.report().certified());
            assert_eq!(result.residualized().as_slice(), &y);
            assert_eq!(result.report().solver_workspace_peak, discarded);
            assert_eq!(session.peak, discarded);
        }
    }
}

#[test]
fn retained_fe_capacity_handoff_counts_spare_value_and_layout_buffers() {
    // Logical axes match a two-effect/two-RHS result. Spare storage must
    // remain charged when the result is moved out of its solver.
    for (value_capacity, channel_capacity, level_capacity) in
        [(14, 2, 2), (8192, 2, 2), (14, 64, 2), (14, 2, 64)]
    {
        let mut values = Vec::with_capacity(value_capacity);
        values.resize(14, 0.0);
        let mut channels = Vec::with_capacity(channel_capacity);
        channels.extend_from_slice(&[1, 1]);
        let mut levels = Vec::with_capacity(level_capacity);
        levels.extend_from_slice(&[4, 3]);
        let initial = FeCoefficientBatch::from_values(channels, levels, 2, values).unwrap();
        let combined =
            FeCoefficientBatch::from_values(vec![1, 1], vec![4, 3], 1, vec![0.0; 7]).unwrap();
        let exact = initial.retained_bytes() + combined.retained_bytes();
        check_retained_fe_initial_capacity(Some(&initial), exact).unwrap();
        let error = check_retained_fe_initial_capacity(Some(&initial), exact - 1).unwrap_err();
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("capacity exceeds"));
        // Returning an error cannot change either coefficient owner.
        assert_eq!(initial.rhs(0).unwrap(), &[0.0; 7]);
        assert_eq!(initial.rhs(1).unwrap(), &[0.0; 7]);
    }
    check_retained_fe_initial_capacity(None, 0).unwrap();
}

#[test]
fn retained_fe_capacity_handoff_rejects_logical_size_only_reservation() {
    let mut values = Vec::with_capacity(16_384);
    values.resize(14, 0.0);
    let initial = FeCoefficientBatch::from_values(vec![1, 1], vec![4, 3], 2, values).unwrap();
    let logical = 7 * 3 * 8 + 2 * 7 * std::mem::size_of::<usize>() as u128;
    assert_eq!(
        check_retained_fe_initial_capacity(Some(&initial), logical)
            .unwrap_err()
            .code,
        ErrorCode::Allocation,
    );
}

#[test]
fn retained_fe_combination_admits_before_allocation_and_preserves_arithmetic() {
    let initial =
        || FeCoefficientBatch::from_values(vec![1], vec![2], 2, vec![5.0, 9.0, 2.0, 3.0]).unwrap();
    let residual =
        || FeCoefficientBatch::from_values(vec![1], vec![2], 1, vec![0.25, -0.5]).unwrap();
    let parent = initial().retained_bytes() + residual().retained_bytes();
    let mut reservation = InjectedReservation::default();
    assert_eq!(
        combine_recovered_fe_coefficients(
            Some(initial()),
            Some(residual()),
            1,
            &[Some(2.0)],
            parent - 1,
            &mut reservation,
        )
        .unwrap_err()
        .code,
        ErrorCode::Allocation
    );
    assert_eq!(
        reservation.calls, 0,
        "reject before the new coefficient bank"
    );
    for fail_at in 1..=3 {
        let mut reservation = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        assert_eq!(
            combine_recovered_fe_coefficients(
                Some(initial()),
                Some(residual()),
                1,
                &[Some(2.0)],
                parent,
                &mut reservation,
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
        assert_eq!(reservation.calls, fail_at);
    }
    for _ in 0..3 {
        let combined = combine_recovered_fe_coefficients(
            Some(initial()),
            Some(residual()),
            1,
            &[Some(2.0)],
            parent,
            &mut SystemFitReservation,
        )
        .unwrap()
        .unwrap();
        assert_eq!(combined.rhs(0).unwrap(), &[1.25, 2.5]);
    }
    struct SpareReservation;
    impl FitReservation for SpareReservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            _: &'static str,
        ) -> Result<()> {
            target
                .try_reserve_exact(elements + 64)
                .map_err(|_| allocation("test spare reservation failed"))
        }
    }
    let error = combine_recovered_fe_coefficients(
        Some(initial()),
        Some(residual()),
        1,
        &[Some(2.0)],
        parent,
        &mut SpareReservation,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("combined FE product capacity"));
}

#[test]
fn retained_fe_parent_is_admitted_before_session_and_survives_peak_reporting() {
    let x = (0..96)
        .map(|r| ((r * 19 % 37) as f64).sin())
        .collect::<Vec<_>>();
    let g = (0..96).map(|r| (r % 4) as f64).collect::<Vec<_>>();
    let y = (0..96)
        .map(|r| 2.0 * x[r] + g[r] + ((r * 7 % 31) as f64).cos())
        .collect::<Vec<_>>();
    let h = (0..96).map(|r| ((r / 4) % 3) as f64).collect::<Vec<_>>();
    let view = prepared_multiway_categorical(&y, &[x], &[g, h]);
    let parent = view.fit_retained_fe_parent_bytes().unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Cg;
    config.absorb.retain_coefficients = true;
    config.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
        execution_bytes: 1234,
        usable_bytes: 1 << 30,
    });
    let mut reservation = SystemFitReservation;
    let reference =
        fit_with_controls_internal(&view, config, &mut reservation, FitPerturbations::default())
            .unwrap();
    assert!(reference.recovered_fe_coefficients.is_some());
    assert!(u128::from(reference.solver_workspace_peak().bytes().unwrap()) >= parent);
    config.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
        execution_bytes: 1234,
        usable_bytes: 1234 + parent - 1,
    });
    let error =
        fit_with_controls_internal(&view, config, &mut reservation, FitPerturbations::default())
            .unwrap_err();
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("retained FE products"));
    // The returned solver envelope includes the retained parent exactly
    // once relative to the caller's unchanged execution base.
    config
        .absorb
        .solver_memory_budget
        .as_mut()
        .unwrap()
        .usable_bytes = 1234 + u128::from(reference.solver_workspace_peak().bytes().unwrap());
    let exact =
        fit_with_controls_internal(&view, config, &mut reservation, FitPerturbations::default())
            .unwrap();
    assert_eq!(exact.coefficients(), reference.coefficients());
    assert_eq!(exact.residuals(), reference.residuals());
    assert_eq!(
        exact.recovered_fe_coefficients,
        reference.recovered_fe_coefficients
    );
    assert_eq!(
        exact.solver_workspace_peak(),
        reference.solver_workspace_peak()
    );
    config
        .absorb
        .solver_memory_budget
        .as_mut()
        .unwrap()
        .usable_bytes -= 1;
    assert_eq!(
        fit_with_controls_internal(&view, config, &mut reservation, FitPerturbations::default())
            .unwrap_err()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn direct_prepared_cg_checks_joint_budget_with_retained_coefficients() {
    let n = 96;
    let effects = [
        (0..n).map(|row| (row % 4) as f64).collect::<Vec<_>>(),
        (0..n).map(|row| ((row / 4) % 3) as f64).collect::<Vec<_>>(),
    ];
    let noise = (0..n)
        .map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 })
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|row| 2.0 * effects[0][row] + 3.0 * effects[1][row] + noise[row])
        .collect::<Vec<_>>();
    let simple_y = (0..n)
        .map(|row| 2.0 * effects[0][row] + noise[row])
        .collect::<Vec<_>>();
    let simple = prepared(
        &simple_y,
        &[],
        &[],
        Some(&effects[0]),
        false,
        TestWeights::Unit,
    );
    let categorical = prepared_multiway_categorical(&y, &[], &effects);
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for (view, outcome, levels, retain) in [
                    (
                        &simple as &dyn PreparedOlsFitView,
                        &simple_y,
                        vec![4],
                        false,
                    ),
                    (
                        &categorical as &dyn PreparedOlsFitView,
                        &y,
                        vec![4, 3],
                        false,
                    ),
                    (
                        &categorical as &dyn PreparedOlsFitView,
                        &y,
                        vec![4, 3],
                        true,
                    ),
                ] {
                    let model =
                        crate::memory::estimate_fe_solver_memory(n, 1, &levels, threads, None)
                            .unwrap();
                    let incremental = model.cg_bytes
                        + if retain {
                            5 * model.coefficient_batch_bytes
                        } else {
                            0
                        };
                    let execution = 137;
                    let supplied = (0..n).map(|row| 1.0 + (row % 4) as f64).collect::<Vec<_>>();
                    for weights in [
                        BorrowedMapWeights::Unit,
                        BorrowedMapWeights::Provided(&supplied),
                    ] {
                        for dispatch in 0..3 {
                            let run = |usable| {
                                let config = AbsorbConfig {
                                    solver: FeSolverKind::Cg,
                                    retain_coefficients: retain,
                                    solver_memory_budget: Some(
                                        crate::memory::FeSolverMemoryBudget {
                                            execution_bytes: execution,
                                            usable_bytes: usable,
                                        },
                                    ),
                                    ..AbsorbConfig::default()
                                };
                                let values =
                                    DenseMatrix::from_row_major(n, 1, outcome.to_vec()).unwrap();
                                match dispatch {
                                    0 => view.fit_absorb_matrix(weights, values, config),
                                    1 => view.fit_absorb_matrix(weights, values, config),
                                    _ => {
                                        let mut session = crate::fe::FeSession::for_ols(
                                            view,
                                            weights,
                                            PreparedOlsFitConfig {
                                                absorb: config,
                                                ..PreparedOlsFitConfig::default()
                                            },
                                        )?;
                                        session.project_ols(weights, values, config, false, false)
                                    }
                                }
                            };
                            let error = run(execution + incremental - 1)
                                .expect_err("CG must reject before its workspace");
                            assert_eq!(error.code, ErrorCode::Allocation);
                            assert!(error.message.contains("joint execution/CG memory plan"));
                            let fit = run(execution + incremental).unwrap();
                            assert!(fit.report().certified());
                            assert_eq!(
                                fit.report().solver_workspace_peak.bytes(),
                                Some(incremental as u64)
                            );
                            let (residual, _, coefficients) = fit.into_full_parts();
                            assert_eq!(coefficients.is_some(), retain);
                            for (&actual, &expected) in residual.as_slice().iter().zip(&noise) {
                                assert!((actual - expected).abs() < 1e-10);
                            }
                            assert_eq!(run(0).unwrap_err().code, ErrorCode::Allocation);
                        }
                    }
                }
            });
    }
}

#[test]
fn ppml_slope_availability_preserves_classification_and_final_design() {
    use crate::ols::prepared_categorical_data::{
        prepare_categorical_ols_data_with_cluster_sources,
        prepare_ppml_categorical_ols_data_with_load_hint,
    };
    for swapped in [false, true] {
        for kind in 0..3 {
            for case in 0..12 {
                let n = 97;
                let x = source(1, "x");
                let g = source(if swapped { 3 } else { 2 }, "g");
                let z = source(if swapped { 2 } else { 3 }, "z");
                let h = source(4, "h");
                let t = source(5, "t");
                let mut sources = vec![x.clone(), g.clone(), z.clone(), h.clone(), t.clone()];
                sources.sort_by_key(|source| source.id);
                let mut request = OlsRequestV1 {
                    model: CompiledTermGraph {
                        sources,
                        terms: vec![continuous_term(1, x.id, false)],
                    },
                    absorbed_terms: vec![
                        AbsorbedTerm {
                            id: AbsorbTermId::new(1).unwrap(),
                            intercept: case != 5,
                            categorical_sources: vec![g.id],
                            slope_sources: vec![z.id, x.id],
                            label: "g##c.(z x)".into(),
                            saved_name: String::new(),
                        },
                        AbsorbedTerm {
                            id: AbsorbTermId::new(2).unwrap(),
                            intercept: true,
                            categorical_sources: if case == 8 {
                                vec![g.id, h.id]
                            } else {
                                vec![h.id]
                            },
                            slope_sources: vec![t.id],
                            label: "h##c.t".into(),
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
                    ],
                    include_intercept: true,
                    keep_singletons: case != 7,
                };
                for source in &request.model.sources {
                    request.bindings.push(OlsInputBinding {
                        stata_var_position: source.id.get() + 2,
                        role: OlsInputRole::NumericSource(source.id),
                    });
                }
                let mut input = LoadedOlsInput {
                    candidate: vec![true; n].into(),
                    outcome: loaded(&vec![1.0; n]),
                    numeric_sources: request
                        .model
                        .sources
                        .iter()
                        .map(|source| {
                            let values: Vec<_> = (0..n)
                                .map(|r| {
                                    if source.id == g.id {
                                        (r % 7) as f64
                                    } else if source.id == h.id {
                                        (r % 5) as f64
                                    } else {
                                        ((r * (source.id.get() as usize + 3)) as f64).sin()
                                    }
                                })
                                .collect();
                            (source.id, loaded(&values))
                        })
                        .collect(),
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                };
                let zi = (z.id.get() - 1) as usize;
                let gi = (g.id.get() - 1) as usize;
                if case == 1 {
                    input.numeric_sources[zi].1.missing[3] = true;
                }
                if case == 2 {
                    input.numeric_sources[0].1.missing[4] = true;
                    input.numeric_sources[zi].1.missing[4] = true;
                    input.outcome.missing[5] = true;
                }
                if case == 3 {
                    input.numeric_sources[zi].1.values[6] = f64::INFINITY;
                }
                if case == 4 {
                    input.candidate.explicit_mut().unwrap()[3] = false;
                    input.numeric_sources[zi].1.values[3] = f64::NAN;
                }
                if case == 6 {
                    input.numeric_sources[zi].1.missing[8] = true;
                }
                if case == 7 {
                    input.numeric_sources[gi].1.values[0] = 1000.0;
                }
                if case == 9 {
                    // A slope source may also define another partition. Its codes
                    // must survive compaction even with deficient slope channels.
                    request.absorbed_terms[1].categorical_sources = vec![z.id];
                    input.numeric_sources[zi].1.values.fill(1.0);
                    input.numeric_sources[4].1.values.fill(0.0);
                }
                if case >= 10 {
                    // Factor recipes bind indices in the complete availability
                    // table, so their existing path is deliberately preserved.
                    input.numeric_sources[zi].1.values = (0..n).map(|r| (r % 3) as f64).collect();
                    request
                        .model
                        .terms
                        .push(categorical_term(2, z.id, 1.0, false));
                    request
                        .model
                        .terms
                        .push(categorical_interaction_term(3, &[(g.id, 2.0), (h.id, 1.0)]));
                    if case == 11 {
                        request
                            .model
                            .terms
                            .push(categorical_term(4, z.id, 9.0, false));
                        request
                            .model
                            .terms
                            .push(categorical_term(5, z.id, 0.0, true));
                    }
                }
                if kind > 0 {
                    let role = if kind == 1 {
                        OlsInputRole::FrequencyWeight
                    } else {
                        OlsInputRole::ProbabilityWeight
                    };
                    request.bindings.push(OlsInputBinding {
                        stata_var_position: 8,
                        role,
                    });
                    let mut weight =
                        loaded(&(0..n).map(|r| 1.0 + (r % 3) as f64).collect::<Vec<_>>());
                    if case == 2 {
                        weight.missing[7] = true;
                    }
                    if kind == 1 {
                        input.frequency_weight = Some(weight);
                    } else {
                        input.probability_weight = Some(weight);
                    }
                }
                let clusters = if case == 6 { vec![z.id] } else { Vec::new() };
                let expected =
                    prepare_categorical_ols_data_with_cluster_sources(&request, &input, &clusters);
                let actual = prepare_ppml_categorical_ols_data_with_load_hint(
                    &request, &input, &clusters, None, 0,
                );
                match (expected, actual) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(a.sample(), b.sample());
                        assert_eq!(a.model(), b.model());
                        assert_eq!(a.effect_specs(), b.effect_specs());
                        assert_eq!(a.report_plan(), b.report_plan());
                        assert_eq!(a.singletons(), b.singletons());
                        assert_eq!(
                            a.fit_owned_fe_design().unwrap(),
                            b.fit_owned_fe_design().unwrap()
                        );
                        if case == 1 {
                            assert!(!b.sample().original_row_by_active.iter().any(|row| row == 3));
                            assert_eq!(b.sample().n_active(), n - 1);
                        }
                        if case == 4 {
                            assert!(!b.sample().original_row_by_active.iter().any(|row| row == 3));
                        }
                    }
                    (Err(a), Err(b)) => assert_eq!(a, b),
                    _ => {
                        panic!("PPML slope preparation changed acceptance: {swapped}/{kind}/{case}")
                    }
                }
            }
        }
    }
}

#[test]
fn compact_slope_availability_preserves_legacy_samples_models_and_topology() {
    use crate::ols::prepared_categorical_data::{
        prepare_categorical_ols_data_owned_with_cluster_sources,
        prepare_categorical_ols_data_with_cluster_sources,
    };
    for swapped in [false, true] {
        for kind in 0..4 {
            for case in 0..7 {
                let n = 16_387;
                let x = source(1, "x");
                let group = source(if swapped { 3 } else { 2 }, "group");
                let slope = source(if swapped { 2 } else { 3 }, "slope");
                let mut sources = vec![x.clone(), group.clone(), slope.clone()];
                sources.sort_by_key(|s| s.id);
                let mut request = OlsRequestV1 {
                    model: CompiledTermGraph {
                        sources,
                        terms: vec![continuous_term(1, x.id, false)],
                    },
                    absorbed_terms: vec![AbsorbedTerm {
                        id: AbsorbTermId::new(1).unwrap(),
                        intercept: case != 5,
                        categorical_sources: vec![group.id],
                        slope_sources: vec![slope.id],
                        label: "group##c.slope".into(),
                        saved_name: String::new(),
                    }],
                    bindings: vec![
                        OlsInputBinding {
                            stata_var_position: 1,
                            role: OlsInputRole::Candidate,
                        },
                        OlsInputBinding {
                            stata_var_position: 2,
                            role: OlsInputRole::Outcome,
                        },
                    ],
                    include_intercept: true,
                    keep_singletons: false,
                };
                for source in &request.model.sources {
                    request.bindings.push(OlsInputBinding {
                        stata_var_position: source.id.get() + 2,
                        role: OlsInputRole::NumericSource(source.id),
                    });
                }
                if case == 6 {
                    request
                        .model
                        .terms
                        .push(continuous_term(2, slope.id, false));
                }
                let mut numeric_sources: Vec<_> = request
                    .model
                    .sources
                    .iter()
                    .map(|source| {
                        let values: Vec<_> = (0..n)
                            .map(|r| {
                                if source.id == group.id {
                                    (r / 5) as f64
                                } else {
                                    ((r * (source.id.get() as usize + 3)) as f64).sin()
                                }
                            })
                            .collect();
                        (source.id, loaded(&values))
                    })
                    .collect();
                let si = (slope.id.get() - 1) as usize;
                let gi = (group.id.get() - 1) as usize;
                if case == 1 {
                    numeric_sources[si].1.missing[3] = true;
                }
                if case == 2 {
                    numeric_sources[gi].1.missing[4] = true;
                    numeric_sources[si].1.missing[4] = true;
                }
                if case == 3 {
                    numeric_sources[gi].1.values[0] = 100_000.0;
                }
                let mut input = LoadedOlsInput {
                    candidate: vec![true; n].into(),
                    outcome: loaded(&vec![1.0; n]),
                    numeric_sources,
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                };
                if case == 4 {
                    input.candidate.explicit_mut().unwrap()[3] = false;
                    input.numeric_sources[si].1.values[3] = f64::NAN;
                }
                if kind > 0 {
                    let role = match kind {
                        1 => OlsInputRole::AnalyticWeight,
                        2 => OlsInputRole::ProbabilityWeight,
                        _ => OlsInputRole::FrequencyWeight,
                    };
                    request.bindings.push(OlsInputBinding {
                        stata_var_position: 6,
                        role,
                    });
                    let mut weight =
                        loaded(&(0..n).map(|r| 1.0 + (r % 3) as f64).collect::<Vec<_>>());
                    if case == 2 {
                        weight.missing[7] = true;
                    }
                    match kind {
                        1 => input.analytic_weight = Some(weight),
                        2 => input.probability_weight = Some(weight),
                        _ => input.frequency_weight = Some(weight),
                    }
                }
                let clusters = if case == 6 { vec![x.id] } else { Vec::new() };
                let expected =
                    prepare_categorical_ols_data_with_cluster_sources(&request, &input, &clusters);
                let actual = prepare_categorical_ols_data_owned_with_cluster_sources(
                    &request, input, &clusters,
                );
                match (expected, actual) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(a.sample(), b.sample());
                        assert_eq!(a.model(), b.model());
                        assert_eq!(a.effect_specs(), b.effect_specs());
                        assert_eq!(a.report_plan(), b.report_plan());
                        assert_eq!(a.singletons(), b.singletons());
                        assert_eq!(
                            a.fit_owned_fe_design().unwrap(),
                            b.fit_owned_fe_design().unwrap()
                        );
                    }
                    (Err(a), Err(b)) => assert_eq!(a, b),
                    _ => panic!("slope preparation changed acceptance"),
                }
            }
        }
    }
}

#[test]
fn single_slope_cache_is_ols_only_and_preserves_full_fit() {
    for n in [16_383, 16_384] {
        for intercept in [false, true] {
            let x = source(1, "x");
            let group = source(2, "g");
            let slope = source(3, "z");
            let request = OlsRequestV1 {
                model: CompiledTermGraph {
                    sources: vec![x.clone(), group.clone(), slope.clone()],
                    terms: vec![continuous_term(1, x.id, false)],
                },
                absorbed_terms: vec![AbsorbedTerm {
                    id: AbsorbTermId::new(1).unwrap(),
                    intercept,
                    categorical_sources: vec![group.id],
                    slope_sources: vec![slope.id],
                    label: "g#c.z".into(),
                    saved_name: String::new(),
                }],
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
                        role: OlsInputRole::NumericSource(group.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 5,
                        role: OlsInputRole::NumericSource(slope.id),
                    },
                ],
                include_intercept: true,
                keep_singletons: false,
            };
            let columns: Vec<Vec<f64>> = (0..3)
                .map(|c| {
                    (0..n)
                        .map(|r| {
                            if c == 1 {
                                (r % 397) as f64
                            } else {
                                ((r * (c + 3)) as f64).sin()
                            }
                        })
                        .collect()
                })
                .collect();
            let y: Vec<_> = (0..n)
                .map(|r| {
                    0.7 * columns[0][r] + columns[2][r] * columns[1][r].cos() + (r as f64).cos()
                })
                .collect();
            let input = LoadedOlsInput {
                candidate: vec![true; n].into(),
                outcome: loaded(&y),
                numeric_sources: vec![
                    (x.id, loaded(&columns[0])),
                    (group.id, loaded(&columns[1])),
                    (slope.id, loaded(&columns[2])),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            };
            let data = crate::ols::prepare_model_owned(&request, &[], input, true).unwrap();
            let config = PreparedOlsFitConfig::default();

            let admitted =
                crate::fe::FeSession::for_ols(&data, BorrowedMapWeights::Unit, config).unwrap();

            drop(admitted);

            let mut legacy = config;
            legacy.absorb.solver = FeSolverKind::Map;
            let expected = fit_with_controls_internal(
                &data,
                legacy,
                &mut SystemFitReservation,
                FitPerturbations::default(),
            )
            .unwrap();
            for saved in [false, true] {
                let mut current = config;
                current.absorb.retain_coefficients = saved;
                current.allow_final_fe_recovery = saved;
                let actual = fit_with_controls_internal(
                    &data,
                    current,
                    &mut SystemFitReservation,
                    FitPerturbations::default(),
                )
                .unwrap();
                assert_eq!(actual.finite_rank(), expected.finite_rank());
                for (a, b) in actual.coefficients().iter().zip(expected.coefficients()) {
                    assert_eq!(a.status, b.status);
                    match (a.value, b.value) {
                        (Some(a), Some(b)) => assert_close(a, b, 1e-10),
                        (None, None) => {}
                        _ => panic!("coefficient presence changed"),
                    }
                }
                for (&a, &b) in actual.residuals().iter().zip(expected.residuals()) {
                    assert_close(a, b, 1e-10);
                }
                assert!(actual.score_certificate().passed);
            }
        }
    }
}

#[test]
fn multipartition_slope_owner_preserves_span_and_admission_boundaries() {
    let g = source(1, "g");
    let h = source(2, "h");
    let z = source(3, "z");
    let n = 280;
    let groups: Vec<_> = (0..n).map(|r| (r % 5) as f64).collect();
    let other: Vec<_> = (0..n).map(|r| (r % 7) as f64).collect();
    let slope: Vec<_> = (0..n).map(|r| (r as f64 / 11.).sin()).collect();
    let outcome: Vec<_> = (0..n)
        .map(|r| 0.3 * groups[r] + groups[r].cos() * slope[r] - 0.2 * other[r])
        .collect();
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![g.clone(), h.clone(), z.clone()],
            terms: Vec::new(),
        },
        absorbed_terms: vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![g.id],
                slope_sources: vec![z.id],
                label: "g##c.z".into(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).unwrap(),
                intercept: true,
                categorical_sources: vec![h.id],
                slope_sources: Vec::new(),
                label: "h".into(),
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
                role: OlsInputRole::NumericSource(g.id),
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(h.id),
            },
            OlsInputBinding {
                stata_var_position: 5,
                role: OlsInputRole::NumericSource(z.id),
            },
        ],
        include_intercept: false,
        keep_singletons: true,
    };
    let input = LoadedOlsInput {
        candidate: vec![true; n].into(),
        outcome: loaded(&outcome),
        numeric_sources: vec![
            (g.id, loaded(&groups)),
            (h.id, loaded(&other)),
            (z.id, loaded(&slope)),
        ],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    let data = crate::ols::prepare_model_owned(&request, &[], input, true).unwrap();
    let supplied: Vec<_> = (0..n).map(|r| 0.1 + (r % 13) as f64).collect();
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for threads in [1, 4] {
            for weights in [
                BorrowedMapWeights::Unit,
                BorrowedMapWeights::Provided(&supplied),
            ] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.mode = mode;
                config.execution.poolsize = 1;
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                ThreadTeam::new(config.execution.threads)
                    .install(|| {
                        let mut session =
                            crate::fe::FeSession::for_ols(&data, weights, config).unwrap();

                        let raw = DenseMatrix::from_row_major(n, 1, outcome.clone()).unwrap();
                        let actual = session
                            .project_ols(weights, raw, config.absorb, false, false)
                            .unwrap();
                        assert!(actual.report().certified());
                        assert!(actual
                            .residualized()
                            .as_slice()
                            .iter()
                            .all(|v| v.abs() < 1e-9));
                        let peak = actual.report().solver_workspace_peak.bytes().unwrap() as u128;
                        let mut refused = config;
                        refused.absorb.solver_memory_budget =
                            Some(crate::memory::FeSolverMemoryBudget {
                                execution_bytes: 100,
                                usable_bytes: 100,
                            });
                        assert_eq!(
                            crate::fe::FeSession::for_ols(&data, weights, refused)
                                .err()
                                .unwrap()
                                .code,
                            ErrorCode::Allocation
                        );
                        let mut bounded = config;
                        bounded.absorb.solver_memory_budget =
                            Some(crate::memory::FeSolverMemoryBudget {
                                execution_bytes: 100,
                                usable_bytes: 100 + peak,
                            });

                        let residual = DenseMatrix::from_row_major(n, 1, vec![0.; n]).unwrap();
                        let actual = session
                            .project_ols(weights, residual, config.absorb, true, false)
                            .unwrap();
                        assert_eq!(actual.report().iterations, 0);
                    })
                    .unwrap();
            }
        }
    }
}

#[test]
fn generic_prepared_slope_admits_owned_design_unit_weights_and_solver_together() {
    let group = source(1, "group");
    let slope = source(2, "slope");
    let n = 12;
    let groups = (0..n).map(|row| (row / 6) as f64).collect::<Vec<_>>();
    let slopes = (0..n).map(|row| (row % 6) as f64 - 2.0).collect::<Vec<_>>();
    let noise = (0..n)
        .map(|row| [1.0, -2.0, 1.0][row % 3])
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|row| 3.0 * groups[row] + 2.0 * slopes[row] + noise[row])
        .collect::<Vec<_>>();
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![group.clone(), slope.clone()],
            terms: Vec::new(),
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept: true,
            categorical_sources: vec![group.id],
            slope_sources: vec![slope.id],
            label: "group##c.slope".into(),
            saved_name: String::new(),
        }],
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
                role: OlsInputRole::NumericSource(group.id),
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(slope.id),
            },
        ],
        include_intercept: false,
        keep_singletons: true,
    };
    let input = LoadedOlsInput {
        candidate: vec![true; n].into(),
        outcome: loaded(&y),
        numeric_sources: vec![(group.id, loaded(&groups)), (slope.id, loaded(&slopes))],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    let prepared = crate::ols::prepare_model(&request, &[], &input, true).unwrap();
    let memory = prepared.fit_owned_fe_design_memory().unwrap();
    let design = prepared.fit_owned_fe_design().unwrap();
    assert_eq!(
        memory.resident_bytes,
        design.storage_capacities().resident_bytes
    );
    let supplied = (0..n).map(|row| 1.0 + (row / 6) as f64).collect::<Vec<_>>();
    for weights in [
        BorrowedMapWeights::Unit,
        BorrowedMapWeights::Provided(&supplied),
    ] {
        let map_config = AbsorbConfig {
            solver: FeSolverKind::Map,
            ..AbsorbConfig::default()
        };
        let map = prepared
            .fit_absorb_matrix(
                weights,
                DenseMatrix::from_columns(&[y.clone()]).unwrap(),
                map_config,
            )
            .unwrap();
        let map_bytes = map.report().solver_workspace_peak.bytes().unwrap() as u128;
        for missing in [0, 1] {
            let mut bounded = map_config;
            bounded.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
                execution_bytes: 137,
                usable_bytes: 137 + map_bytes - missing,
            });
            let result = prepared.fit_absorb_matrix(
                weights,
                DenseMatrix::from_columns(&[y.clone()]).unwrap(),
                bounded,
            );
            if missing == 0 {
                assert_eq!(result.unwrap().residualized(), map.residualized());
            } else {
                assert_eq!(result.unwrap_err().code, ErrorCode::Allocation);
            }
        }
        for solver in [
            FeSolverKind::Cg,
            FeSolverKind::Lsmr,
            FeSolverKind::IronsTuck,
        ] {
            let mut config = AbsorbConfig {
                solver,
                ..AbsorbConfig::default()
            };
            let solver_bytes =
                crate::memory::generic_fe_solver_bytes(&design, 1, solver, config.lsmr).unwrap()
                    + if solver == FeSolverKind::Lsmr {
                        (8 * n) as u128
                    } else {
                        0
                    };
            let required = memory
                .construction_bytes
                .max(memory.resident_bytes + solver_bytes)
                + (32 * n) as u128;
            for usable in [1, 137 + required - 1, 137 + required] {
                config.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
                    execution_bytes: 137,
                    usable_bytes: usable,
                });
                let result = prepared.fit_absorb_matrix(
                    weights,
                    DenseMatrix::from_columns(&[y.clone()]).unwrap(),
                    config,
                );
                if usable < 137 + required {
                    assert_eq!(result.unwrap_err().code, ErrorCode::Allocation);
                } else {
                    let output = result.unwrap();
                    assert!(output.report().certified());
                    assert_eq!(
                        output.report().solver_workspace_peak.bytes(),
                        Some(required as u64)
                    );
                    assert!(output
                        .residualized()
                        .as_slice()
                        .iter()
                        .zip(&noise)
                        .all(|(a, b)| (a - b).abs() < 1e-10));
                }
            }
        }
    }
}

#[test]
fn saved_final_residual_removes_small_fe_components_across_solver_families() {
    let n = 512 * 16 * 4;
    let effects = [
        (0..n).map(|row| (row % 512) as f64).collect::<Vec<_>>(),
        (0..n)
            .map(|row| ((row / 512) % 16) as f64)
            .collect::<Vec<_>>(),
    ];
    // Each crossed FE cell has two +1 and two -1 observations with
    // identical weights: its exact projected residual is known directly.
    let residuals = (0..n)
        .map(|row| {
            if (row / (512 * 16)) % 2 == 0 {
                -1.0
            } else {
                1.0
            }
        })
        .collect::<Vec<_>>();
    let raw_weights = (0..n)
        .map(|row| 1.0 + ((row % (512 * 16)) % 3) as f64)
        .collect::<Vec<_>>();
    let prepared = prepared_multiway_categorical(&residuals, &[], &effects);
    for weights in [
        BorrowedMapWeights::Unit,
        BorrowedMapWeights::Provided(&raw_weights),
    ] {
        for retain in [false, true] {
            for solver in [
                FeSolverKind::Auto,
                FeSolverKind::Cg,
                FeSolverKind::Map,
                FeSolverKind::Lsmr,
                FeSolverKind::Cmg,
            ] {
                let mut config = PreparedOlsFitConfig::default();
                config.absorb.solver = solver;
                config.absorb.retain_coefficients = retain;
                config.final_residual_tolerance = Some(1e-12);
                config.absorb.map.tolerance = 1e-8;
                let mut session =
                    crate::fe::FeSession::for_ols(&prepared, weights, config).unwrap();
                let pending = (0..n)
                    .map(|row| residuals[row] + if row % 512 == 0 { 2e-9 } else { 0.0 })
                    .collect();
                let projected = session
                    .project_ols(
                        weights,
                        DenseMatrix::from_row_major(n, 1, pending).unwrap(),
                        config.final_absorb_config(),
                        true,
                        false,
                    )
                    .unwrap();
                assert!(projected.report().certified());
                let (actual, _, _) = projected.into_full_parts();
                let error = actual
                    .as_slice()
                    .iter()
                    .zip(&residuals)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f64, f64::max);
                assert!(
                    error <= 1e-11,
                    "{solver:?} final residual retains an FE component: {error}"
                );
            }
        }
    }
}

#[test]
fn saved_slope_recovery_admits_each_attempt_and_keeps_map_fallback() {
    let group = source(1, "group");
    let slope = source(2, "slope");
    for n in [12, 16_386] {
        let groups = (0..n).map(|row| (row / (n / 2)) as f64).collect::<Vec<_>>();
        let slopes = (0..n).map(|row| (row % 6) as f64 - 2.0).collect::<Vec<_>>();
        let noise = (0..n)
            .map(|row| [1.0, -2.0, 1.0][row % 3])
            .collect::<Vec<_>>();
        let y = (0..n)
            .map(|row| 3.0 * groups[row] + 2.0 * slopes[row] + noise[row])
            .collect::<Vec<_>>();
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![group.clone(), slope.clone()],
                terms: Vec::new(),
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![group.id],
                slope_sources: vec![slope.id],
                label: "group##c.slope".into(),
                saved_name: String::new(),
            }],
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
                    role: OlsInputRole::NumericSource(group.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(slope.id),
                },
            ],
            include_intercept: false,
            keep_singletons: true,
        };
        let input = LoadedOlsInput {
            candidate: vec![true; n].into(),
            outcome: loaded(&y),
            numeric_sources: vec![(group.id, loaded(&groups)), (slope.id, loaded(&slopes))],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = crate::ols::prepare_model(&request, &[], &input, true).unwrap();
        let target = y.iter().zip(&noise).map(|(y, e)| y - e).collect::<Vec<_>>();
        for budget in [
            None,
            Some(crate::memory::FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 1,
            }),
        ] {
            let error = prepared
                .recover_model_effects(
                    &target,
                    &noise,
                    10.0,
                    &[true, true],
                    false,
                    true,
                    crate::fe::MapConfig::default(),
                    budget,
                )
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidArgument);
            assert_eq!(
                error.message,
                "model FE recovery requires intercept-only effects"
            );
        }
        let weights = (0..n).map(|row| 1.0 + (row % 3) as f64).collect::<Vec<_>>();
        for workers in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    for weights in [
                        BorrowedMapWeights::Unit,
                        BorrowedMapWeights::Provided(&weights),
                    ] {
                        let lsmr = crate::fe::LsmrConfig::default();
                        let map = crate::fe::MapConfig {
                            tolerance: 1e-12,
                            ..Default::default()
                        };
                        let memory = crate::memory::estimate_slope_recovery_memory(
                            n,
                            &[(2, true, 1)],
                            2,
                            prepared.fe_topology().compacted_sources().len(),
                            prepared.fe_topology().validation_workspace_bytes().unwrap(),
                            workers,
                            lsmr.local_size,
                        )
                        .unwrap();
                        let run = |workspace| {
                            prepared.recover_absorbed_effects_fast_with_weights(
                                weights,
                                &target,
                                &[true, true],
                                None,
                                false,
                                true,
                                lsmr,
                                map,
                                Some(crate::memory::FeSolverMemoryBudget {
                                    execution_bytes: 137,
                                    usable_bytes: 137 + workspace,
                                }),
                            )
                        };
                        assert_eq!(
                            run(memory.map_peak_bytes - 1).unwrap_err().code,
                            ErrorCode::Allocation
                        );
                        assert_eq!(run(0).unwrap_err().code, ErrorCode::Allocation);
                        let tight = run(memory.map_peak_bytes).unwrap();
                        assert_eq!(
                            tight.recovery_method(),
                            crate::fe::CategoricalRecoveryMethod::Map
                        );
                        assert_eq!(
                            tight.solver_workspace_peak().bytes(),
                            Some(memory.map_peak_bytes as u64)
                        );
                        let direct_bytes = memory.retained_peak_bytes
                            + crate::fe::single_slope_recovery_extra_bytes(n, 2, 2, workers)
                                .unwrap();
                        let ample = run(memory.lsmr_peak_bytes.max(direct_bytes)).unwrap();
                        if true {
                            assert_eq!(
                                ample.recovery_method(),
                                crate::fe::CategoricalRecoveryMethod::Direct
                            );
                            let cached_bytes = direct_bytes
                                + crate::fe::single_slope_recovery_geometry_bytes(2, 2).unwrap();
                            let expected = direct_bytes;
                            assert_eq!(
                                ample.solver_workspace_peak().bytes(),
                                Some(expected as u64)
                            );
                            let uncached = run(direct_bytes).unwrap();
                            assert_eq!(
                                uncached.recovery_method(),
                                crate::fe::CategoricalRecoveryMethod::Direct
                            );
                            assert_eq!(
                                uncached.solver_workspace_peak().bytes(),
                                Some(direct_bytes as u64)
                            );
                            if workers == 1 {
                                let cached = run(cached_bytes).unwrap();
                                assert_eq!(
                                    cached.solver_workspace_peak().bytes(),
                                    Some(direct_bytes as u64)
                                );
                                assert_eq!(cached.channels(), uncached.channels());
                                assert_eq!(
                                    run(cached_bytes - 1)
                                        .unwrap()
                                        .solver_workspace_peak()
                                        .bytes(),
                                    Some(direct_bytes as u64)
                                );
                            }
                            assert_eq!(
                                run(direct_bytes - 1).unwrap().recovery_method(),
                                crate::fe::CategoricalRecoveryMethod::Map
                            );
                        } else {
                            assert_eq!(
                                ample.solver_workspace_peak().bytes(),
                                Some(memory.lsmr_peak_bytes as u64)
                            );
                        }
                        let coefficients = crate::fe::FeCoefficientBatch::from_values(
                            vec![2],
                            vec![2],
                            1,
                            vec![0.0, 3.0, 2.0, 2.0],
                        )
                        .unwrap();
                        let retained = prepared
                            .recover_absorbed_effects_fast_with_weights(
                                weights,
                                &target,
                                &[true, true],
                                Some(&coefficients),
                                false,
                                true,
                                lsmr,
                                map,
                                Some(crate::memory::FeSolverMemoryBudget {
                                    execution_bytes: 137,
                                    usable_bytes: 137 + memory.retained_peak_bytes,
                                }),
                            )
                            .unwrap();
                        assert_eq!(
                            retained.recovery_method(),
                            crate::fe::CategoricalRecoveryMethod::RetainedCoefficients
                        );
                        assert_eq!(
                            retained.solver_workspace_peak().bytes(),
                            Some(memory.retained_peak_bytes as u64)
                        );
                        for recovered in [tight, ample, retained] {
                            for (row, expected) in target.iter().enumerate() {
                                assert!((recovered.absorbed_sum()[row] - expected).abs() < 1e-10);
                                let sum = recovered
                                    .channels()
                                    .iter()
                                    .flatten()
                                    .map(|channel| channel[row])
                                    .sum::<f64>();
                                assert!((sum - expected).abs() < 1e-10);
                            }
                        }
                    }
                });
        }
    }
}

#[test]
fn saved_intercept_recovery_enforces_joint_budget_before_materialization() {
    let n = 96;
    let effects = [
        (0..n).map(|row| (row % 4) as f64).collect::<Vec<_>>(),
        (0..n).map(|row| ((row / 4) % 3) as f64).collect::<Vec<_>>(),
    ];
    let target = (0..n)
        .map(|row| 2.0 * effects[0][row] + 3.0 * effects[1][row])
        .collect::<Vec<_>>();
    let prepared = prepared_multiway_categorical(&target, &[], &effects);
    for workers in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let lsmr = crate::fe::LsmrConfig::default();
                let map = crate::fe::MapConfig {
                    tolerance: 1e-12,
                    ..crate::fe::MapConfig::default()
                };
                let memory = crate::memory::estimate_intercept_recovery_memory(
                    n,
                    &[4, 3],
                    2,
                    workers,
                    lsmr.local_size,
                )
                .unwrap();
                let workspace = memory.cg_peak_bytes;
                let execution = 137;
                let run = |usable_bytes| {
                    prepared.recover_absorbed_effects_fast(
                        &target,
                        &[true, true],
                        None,
                        false,
                        true,
                        lsmr,
                        map,
                        Some(crate::memory::FeSolverMemoryBudget {
                            execution_bytes: execution,
                            usable_bytes,
                        }),
                    )
                };
                let error =
                    run(execution + memory.map_peak_bytes.min(memory.retained_peak_bytes) - 1)
                        .unwrap_err();
                assert_eq!(error.code, ErrorCode::Allocation);
                assert!(error
                    .message
                    .contains("joint execution/recovery memory plan"));
                let recovered = run(execution + workspace).unwrap();
                assert_eq!(
                    recovered.recovery_method(),
                    crate::fe::CategoricalRecoveryMethod::Direct
                );
                assert_eq!(
                    recovered.solver_workspace_peak().bytes(),
                    Some(memory.retained_peak_bytes as u64)
                );
                let tight = run(execution + memory.map_peak_bytes).unwrap();
                assert_eq!(
                    tight.recovery_method(),
                    if memory.retained_peak_bytes <= memory.map_peak_bytes {
                        crate::fe::CategoricalRecoveryMethod::Direct
                    } else {
                        crate::fe::CategoricalRecoveryMethod::Map
                    }
                );
                assert_eq!(
                    tight.solver_workspace_peak().bytes(),
                    Some(memory.map_peak_bytes.min(memory.retained_peak_bytes) as u64)
                );
                assert!(tight
                    .absorbed_sum()
                    .iter()
                    .zip(&target)
                    .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
                // Keep the legacy matrix indices and numerical traversal order explicit.
                #[allow(clippy::needless_range_loop)]
                for row in 0..n {
                    assert!((recovered.absorbed_sum()[row] - target[row]).abs() <= 1e-10);
                    assert!(
                        (recovered.channels()[0].as_ref().unwrap()[row]
                            - (2.0 * effects[0][row] + 3.0))
                            .abs()
                            <= 1e-10
                    );
                    assert!(
                        (recovered.channels()[1].as_ref().unwrap()[row]
                            - (3.0 * effects[1][row] - 3.0))
                            .abs()
                            <= 1e-10
                    );
                }
                let retained = crate::fe::FeCoefficientBatch::from_values(
                    vec![1, 1],
                    vec![4, 3],
                    1,
                    vec![0.0, 2.0, 4.0, 6.0, 0.0, 3.0, 6.0],
                )
                .unwrap();
                let retained_run = |coefficients, bytes| {
                    prepared.recover_absorbed_effects_fast(
                        &target,
                        &[true, true],
                        Some(coefficients),
                        false,
                        true,
                        lsmr,
                        map,
                        Some(crate::memory::FeSolverMemoryBudget {
                            execution_bytes: execution,
                            usable_bytes: execution + bytes,
                        }),
                    )
                };
                assert!(memory.retained_peak_bytes < workspace);
                let normalized = retained_run(&retained, memory.retained_peak_bytes).unwrap();
                assert_eq!(
                    normalized.recovery_method(),
                    crate::fe::CategoricalRecoveryMethod::RetainedCoefficients
                );
                assert_eq!(
                    normalized.solver_workspace_peak().bytes(),
                    Some(memory.retained_peak_bytes as u64)
                );
                assert!(normalized
                    .absorbed_sum()
                    .iter()
                    .zip(&target)
                    .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
                assert_eq!(
                    retained_run(&retained, memory.retained_peak_bytes - 1)
                        .unwrap_err()
                        .code,
                    ErrorCode::Allocation
                );
                let incorrect = crate::fe::FeCoefficientBatch::from_values(
                    vec![1, 1],
                    vec![4, 3],
                    1,
                    vec![0.0; 7],
                )
                .unwrap();
                let fallback = retained_run(&incorrect, memory.retained_peak_bytes).unwrap();
                assert_eq!(
                    fallback.recovery_method(),
                    crate::fe::CategoricalRecoveryMethod::Direct
                );
                assert_eq!(
                    fallback.solver_workspace_peak().bytes(),
                    Some(memory.map_peak_bytes.min(memory.retained_peak_bytes) as u64)
                );
                assert!(fallback
                    .absorbed_sum()
                    .iter()
                    .zip(&target)
                    .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
                let corrected = retained_run(&incorrect, workspace).unwrap();
                assert_eq!(
                    corrected.solver_workspace_peak().bytes(),
                    Some(memory.retained_peak_bytes as u64)
                );
                assert!(corrected
                    .absorbed_sum()
                    .iter()
                    .zip(&target)
                    .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
            });
    }
}

fn prepared_multiway_categorical(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effects: &[Vec<f64>],
) -> crate::PreparedOlsData {
    prepared_multiway_categorical_weights(outcome, regressors, effects, TestWeights::Unit)
}

fn prepared_multiway_categorical_weights(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effects: &[Vec<f64>],
    weights: TestWeights<'_>,
) -> crate::PreparedOlsData {
    assert!((1..=3).contains(&effects.len()));
    assert!(regressors
        .iter()
        .chain(effects)
        .all(|values| values.len() == outcome.len()));
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    let mut absorbed_terms = Vec::new();
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
    for (index, values) in regressors.iter().enumerate() {
        let one_based = u32::try_from(index + 1).expect("small control identity");
        let entry = source(one_based, &format!("x{one_based}"));
        terms.push(continuous_term(one_based, entry.id, false));
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(bindings.len() + 1).expect("small control binding"),
            role: OlsInputRole::NumericSource(entry.id),
        });
        numeric_sources.push((entry.id, loaded(values)));
        sources.push(entry);
    }
    for (effect, values) in effects.iter().enumerate() {
        let source_id =
            u32::try_from(regressors.len() + effect + 1).expect("small fixed-effect identity");
        let entry = source(source_id, &format!("g{}", effect + 1));
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(bindings.len() + 1)
                .expect("small fixed-effect binding"),
            role: OlsInputRole::NumericSource(entry.id),
        });
        numeric_sources.push((entry.id, loaded(values)));
        absorbed_terms.push(AbsorbedTerm {
            id: AbsorbTermId::new(u32::try_from(effect + 1).expect("small absorb identity"))
                .expect("absorb identity"),
            intercept: true,
            categorical_sources: vec![entry.id],
            slope_sources: Vec::new(),
            label: format!("g{}", effect + 1),
            saved_name: String::new(),
        });
        sources.push(entry);
    }
    let (role, analytic_weight, probability_weight, frequency_weight) = match weights {
        TestWeights::Unit => (None, None, None, None),
        TestWeights::Analytic(values) => (
            Some(OlsInputRole::AnalyticWeight),
            Some(loaded(values)),
            None,
            None,
        ),
        TestWeights::Probability(values) => (
            Some(OlsInputRole::ProbabilityWeight),
            None,
            Some(loaded(values)),
            None,
        ),
        TestWeights::Frequency(values) => (
            Some(OlsInputRole::FrequencyWeight),
            None,
            None,
            Some(loaded(values)),
        ),
    };
    if let Some(role) = role {
        bindings.push(OlsInputBinding {
            stata_var_position: (bindings.len() + 1) as u32,
            role,
        });
    }
    crate::prepare_model(
        &OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms,
            bindings,
            include_intercept: true,
            keep_singletons: true,
        },
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(outcome),
            numeric_sources,
            analytic_weight,
            frequency_weight,
            probability_weight,
        },
        true,
    )
    .expect("prepared multiway categorical data")
}

fn assert_initial_checkpoint_lsmr_matches_map(
    prepared: &crate::PreparedOlsData,
    threads: usize,
) -> FeSolverKind {
    let mut config = PreparedOlsFitConfig {
        execution: PreparedOlsExecutionConfig {
            mode: PreparedOlsExecutionMode::Fast,
            poolsize: 8,
            threads: ThreadConfig::from_requested(threads, true).expect("test thread config"),
        },
        ..PreparedOlsFitConfig::default()
    };
    config.absorb.solver = FeSolverKind::Map;
    config.absorb.map.tolerance = 1e-6;
    config.absorb.lsmr.tolerance = 1e-6;
    let frame = LinearSolveFrame::from_prepared_ols(prepared).expect("initial frame");
    let generations = LinearSessionGenerations::new(1, 1, 1, 1, 1);
    let mut map = CertifiedLinearSession::new_with_fe_design(
        prepared,
        frame,
        config,
        generations,
        prepared.fit_owned_fe_design().expect("MAP FE design"),
    )
    .expect("MAP initial session");
    let mut lsmr = CertifiedLinearSession::new_with_fe_design(
        prepared,
        frame,
        config,
        generations,
        prepared.fit_owned_fe_design().expect("LSMR FE design"),
    )
    .expect("LSMR initial session");
    lsmr.enable_checkpoint_lsmr(true);
    let expected = map.solve_fresh().expect("MAP initial solve");
    let actual = lsmr.solve_fresh().expect("LSMR initial solve");
    assert!(actual.certified());
    assert_eq!(
        actual.frame().absorption_report().requested_solver,
        FeSolverKind::Lsmr,
        "initial projection report: {:?}",
        actual.frame().absorption_report(),
    );
    let used_solver = actual.frame().absorption_report().used_solver;
    assert!(matches!(
        used_solver,
        FeSolverKind::Lsmr | FeSolverKind::Map
    ));
    // Bounded two/three-FE Schur checkpoints report Map on success without
    // a failed LSMR attempt. Only a true fallback must report Map.
    if actual.frame().absorption_report().fallback_used {
        assert_eq!(used_solver, FeSolverKind::Map);
    } else if used_solver == FeSolverKind::Map {
        assert!(matches!(
            prepared.fit_borrowed_cmg_design().unwrap().effects().len(),
            2 | 3
        ));
    }
    assert_eq!(actual.frame().finite_rank(), expected.frame().finite_rank());
    assert_eq!(
        actual.basis().kept_generated_columns(),
        expected.basis().kept_generated_columns()
    );
    assert_eq!(
        actual.basis().dropped_generated_columns(),
        expected.basis().dropped_generated_columns()
    );
    assert_eq!(
        actual
            .coefficients()
            .map(|coefficient| coefficient.status)
            .collect::<Vec<_>>(),
        expected
            .coefficients()
            .map(|coefficient| coefficient.status)
            .collect::<Vec<_>>()
    );
    for (actual, expected) in actual.coefficients().zip(expected.coefficients()) {
        match (actual.value, expected.value) {
            (Some(actual), Some(expected)) => assert_close(actual, expected, 2e-6),
            (None, None) => {}
            _ => panic!("initial checkpoint coefficient presence differs"),
        }
    }
    for (&actual, &expected) in actual.frame().bread().iter().zip(expected.frame().bread()) {
        assert_close(actual, expected, 2e-6);
    }
    assert!(actual.frame().rank_certificate().passed);
    assert!(actual.frame().score_certificate().passed);
    assert!(actual.frame().canonical_proof().unwrap().bread.passed);
    used_solver
}

#[test]
fn bound_score_means_preserve_cache_bits_and_reservations() {
    for (rows, regressors, intercept, absorbed) in [
        (4095, 6, false, false),
        (4096, 1, true, false),
        (32771, 10, true, true),
        (4103, 63, true, false),
        (4103, 64, true, true),
        (32771, 6, false, true),
        (4096, 0, true, true),
    ] {
        let x = (0..regressors)
            .map(|column| {
                (0..rows)
                    .map(|row| match column % 7 {
                        0 => -0.0,
                        1 => ((row * 37 % 101) as f64 - 50.0) * 1e150,
                        2 => ((row * 19 % 97) as f64 - 48.0) * 1e-150,
                        3 => [1e16, 1.0, -1e16][row % 3],
                        4 => 1.0,
                        _ => (row as f64 * 0.173).sin(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let groups = (0..rows).map(|row| (row % 17) as f64).collect::<Vec<_>>();
        let raw_weights = (0..rows)
            .map(|row| (row % 13 + 1) as f64)
            .collect::<Vec<_>>();
        for weight in [
            TestWeights::Unit,
            TestWeights::Analytic(&raw_weights),
            TestWeights::Probability(&raw_weights),
            TestWeights::Frequency(&raw_weights),
        ] {
            let data = prepared(
                &vec![1.0; rows],
                &x,
                &vec![false; regressors],
                absorbed.then_some(groups.as_slice()),
                intercept,
                weight,
            );
            let columns = data.model.columns.len();
            let binding = super::super::prepared_rhs::NumericOlsColumns::bind_score_means(
                &data.model,
                &data.sample,
            )
            .unwrap();
            assert_eq!(
                binding.is_some(),
                rows >= 4096 && (1..=64).contains(&columns)
            );
            let mut within = DenseMatrix::zeros(rows, columns + 1).unwrap();
            let scales = (0..columns)
                .map(|column| {
                    for row in 0..rows {
                        within[(row, column + 1)] = (row as f64 - 13.0) / (column + 1) as f64;
                    }
                    (0..rows)
                        .map(|row| data.column_value(column, row).unwrap().abs())
                        .fold(0.0_f64, f64::max)
                })
                .collect::<Vec<_>>();
            let weights = normalized_weights(&data).unwrap();
            for threads in [1, 4] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let run = |bound| {
                            let mut reservation = InjectedReservation::default();
                            let cache = build_fast_score_cache(
                                &data,
                                &within,
                                &scales,
                                weights,
                                &mut reservation,
                                usize::MAX,
                                bound,
                            )
                            .unwrap()
                            .unwrap();
                            let PreparedOlsFastScoreCache::Packed {
                                generated_columns,
                                within,
                                means,
                            } = cache
                            else {
                                panic!("expected packed score cache")
                            };
                            (
                                generated_columns,
                                within
                                    .as_slice()
                                    .iter()
                                    .map(|v| v.to_bits())
                                    .collect::<Vec<_>>(),
                                means.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                                reservation.records,
                            )
                        };
                        assert_eq!(
                            run(true),
                            run(false),
                            "rows={rows} columns={columns} absorbed={absorbed} threads={threads}"
                        );
                    });
            }
        }
    }
}

#[test]
fn bound_score_means_preserve_budget_and_failure_checks() {
    let rows = 4096;
    let data = prepared(
        &vec![1.0; rows],
        &[vec![2.0; rows]],
        &[false],
        None,
        true,
        TestWeights::Unit,
    );
    let weights = normalized_weights(&data).unwrap();
    let mut within = DenseMatrix::zeros(rows, 3).unwrap();
    for bound in [false, true] {
        let mut reservation = InjectedReservation::default();
        assert!(build_fast_score_cache(
            &data,
            &within,
            &[2.0, 1.0],
            weights,
            &mut reservation,
            0,
            bound
        )
        .unwrap()
        .is_none());
        assert_eq!(reservation.calls, 1);
        assert!(build_fast_score_cache(
            &data,
            &within,
            &[1.0],
            weights,
            &mut SystemFitReservation,
            usize::MAX,
            bound
        )
        .is_err());
        for fail_at in 1..=4 {
            let mut reservation = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            assert!(build_fast_score_cache(
                &data,
                &within,
                &[2.0, 1.0],
                weights,
                &mut reservation,
                usize::MAX,
                bound
            )
            .is_err());
        }
        within[(rows - 1, 1)] = f64::INFINITY;
        assert!(build_fast_score_cache(
            &data,
            &within,
            &[2.0, 1.0],
            weights,
            &mut SystemFitReservation,
            usize::MAX,
            bound
        )
        .is_err());
        within[(rows - 1, 1)] = 0.0;
    }
}

fn test_projected_moments<R: FitReservation>(
    within: &DenseMatrix,
    candidates: &[usize],
    reservation: &mut R,
) -> Result<(Vec<f64>, Vec<f64>)> {
    use crate::ols::finite_wls_kernel::{fill_finite_wls_moments, FiniteDenseRows, FiniteResponse};
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::projected(within),
        rows: within.nrows(),
        stored_columns: within.ncols() - 1,
        selected_columns: Some(candidates),
        response: FiniteResponse::Column(within, 0),
        weights: BorrowedMapWeights::Unit,
        solver_weight_divisor: 1.0,
        column_scales: None,
    };
    let layout_source = validate_moment_source(source)?;
    let layout = layout_source.layout();
    let mut partials = reserved_filled(
        reservation,
        layout.partial_elements,
        0.0,
        "test finite partials",
    )?;
    let mut rhs = reserved_filled(reservation, candidates.len(), 0.0, "test finite RHS")?;
    let mut gram = reserved_filled(
        reservation,
        candidates.len() * candidates.len(),
        0.0,
        "test finite Gram",
    )?;
    fill_finite_wls_moments(layout_source, &mut partials, &mut rhs, &mut gram)?;
    Ok((rhs, gram))
}

fn assert_packed_and_strided_score_caches_match(prepared: &PreparedOlsData) {
    let mut packed_reservation = InjectedReservation::default();
    let packed_fit = fit_with_controls_internal_mode(
        prepared,
        PreparedOlsFitConfig::default(),
        &mut packed_reservation,
        FitPerturbations::default(),
        true,
        MAX_FAST_SCORE_CACHE_BYTES,
    )
    .expect("packed score cache fit");
    let mut strided_reservation = InjectedReservation::default();
    let strided_fit = fit_with_controls_internal_mode(
        prepared,
        PreparedOlsFitConfig::default(),
        &mut strided_reservation,
        FitPerturbations::default(),
        true,
        0,
    )
    .expect("strided score cache fit");
    assert_eq!(packed_fit.coefficients(), strided_fit.coefficients());
    assert_eq!(packed_fit.bread(), strided_fit.bread());
    assert_eq!(packed_fit.residuals(), strided_fit.residuals());
    let PreparedOlsFastScoreCache::Packed {
        generated_columns: packed_columns,
        within: packed,
        means: packed_means,
    } = packed_fit.fast_score_cache.expect("packed score cache")
    else {
        panic!("expected packed score cache");
    };
    let PreparedOlsFastScoreCache::Selected {
        generated_columns: strided_columns,
        within: strided,
        means: strided_means,
    } = strided_fit.fast_score_cache.expect("strided score cache")
    else {
        panic!("expected strided score cache");
    };
    assert_eq!(packed_columns, strided_columns);
    assert_eq!(packed_means.len(), strided_means.len());
    for (actual, expected) in strided_means.iter().zip(&packed_means) {
        assert_close(*actual, *expected, 2e-12);
    }
    for row in 0..packed.nrows() {
        for (finite, generated) in strided_columns.iter().copied().enumerate() {
            let actual = strided[(row, generated + 1)];
            assert_close(actual, packed[(row, finite)], 2e-12);
        }
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance * scale,
        "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
    );
}

fn without_solve_timings(mut fit: CertifiedPreparedOlsFit) -> CertifiedPreparedOlsFit {
    fit.absorption_report.setup_seconds = 0.0;
    fit.absorption_report.solve_seconds = 0.0;
    fit.residual_absorption_report.setup_seconds = 0.0;
    fit.residual_absorption_report.solve_seconds = 0.0;

    fit
}

#[test]
fn loaded_moments_preserve_fit_bits_fallback_and_admission() {
    for (n, width) in [
        (16_383, 9),
        (16_384, 9),
        (32_771, 9),
        (16_384, 1),
        (16_384, 3),
        (16_384, 5),
    ] {
        let columns: Vec<Vec<f64>> = (0..width)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| {
                groups[r]
                    + columns[0][r] * 1.3
                    + columns[width - 1][r] * 0.7
                    + (r as f64).cos() * 0.1
            })
            .collect();
        let data = prepared(
            &response,
            &columns,
            &vec![false; width],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        let raw = data
            .fit_build_rhs_with_response(&data.fit_sample().outcome)
            .unwrap();
        let required = LoadedOlsRows::moment_workspace_bytes(n, raw.ncols());
        for threads in [4, 8, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
            let expected =
                without_solve_timings(fit_prepared_model_coefficients(&data, config).unwrap());
            for short in [false, true] {
                for stale in [false, true] {
                    let mut rows = raw.as_slice().to_vec();
                    if stale {
                        rows[1] += 0.25;
                    }
                    let budget = required.map_or(0, |b| b - usize::from(short));
                    let loaded = ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            LoadedOlsRows::from(rows).with_unit_moments(n, raw.ncols(), budget)
                        })
                        .unwrap();
                    assert_eq!(
                        loaded.moments.is_some(),
                        required.is_some()
                            && !short
                            && crate::parallel::StagePlan::new(
                                n as u128 * raw.ncols() as u128,
                                2,
                                threads
                            )
                            .lanes
                                > 1
                    );
                    let actual = without_solve_timings(
                        fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded)
                            .unwrap(),
                    );
                    assert!(
                        expected == actual,
                        "N={n}, width={width}, threads={threads}, short={short}, stale={stale}"
                    );
                }
            }
        }
    }
    let mut reservation = LoadedMomentReservation { remaining: 7 };
    let mut values: Vec<f64> = Vec::new();
    assert!(reservation.reserve(&mut values, 1, "test").is_err());
    assert_eq!(values.capacity(), 0);
}

#[test]
fn loaded_moment_errors_remain_after_validation_and_stale_rows_are_discarded() {
    for (n, width, workers) in [(257, 1, 1), (4095, 3, 4), (16_384, 8, 8)] {
        let columns: Vec<Vec<f64>> = (0..width)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| groups[r] + columns[0][r] + (r as f64).cos())
            .collect();
        let mut data = prepared(
            &response,
            &columns,
            &vec![false; width],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        let raw = data
            .fit_build_rhs_with_response(&data.fit_sample().outcome)
            .unwrap();
        let mut config = PreparedOlsFitConfig::default();
        config.execution.threads = ThreadConfig::from_requested(workers, true).unwrap();
        let error = invalid_data("deferred raw moment error");
        let loaded = |stale| {
            let mut rows = raw.as_slice().to_vec();
            if stale {
                rows[1] += 0.5;
            }
            LoadedOlsRows {
                rows,
                moments: Some(Err(error.clone())),
            }
        };
        assert_eq!(
            fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded(false))
                .unwrap_err(),
            error
        );
        let expected =
            without_solve_timings(fit_prepared_model_coefficients(&data, config).unwrap());
        let actual = without_solve_timings(
            fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded(true)).unwrap(),
        );
        assert!(actual == expected);
        data.sample.outcome[0] = f64::NAN;
        assert_eq!(
            fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded(false))
                .unwrap_err(),
            fit_prepared_model_coefficients(&data, config).unwrap_err()
        );
    }
}
#[test]
fn one_fe_borrow_reuse_preserves_complete_fits_and_error_order() {
    for n in [16_383, 16_384, 32_771] {
        let columns: Vec<Vec<f64>> = (0..9)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1)
            .collect();
        let mut data = prepared(
            &response,
            &columns,
            &[false; 9],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        for threads in [1, 4, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
            let run = |data: &PreparedOlsData, _disabled| {
                ThreadTeam::new(config.execution.threads)
                    .install(|| {
                        fit_with_controls_internal(
                            data,
                            config,
                            &mut SystemFitReservation,
                            FitPerturbations {
                                ..FitPerturbations::default()
                            },
                        )
                    })
                    .unwrap()
                    .map(without_solve_timings)
            };
            let expected = run(&data, true);
            let actual = run(&data, false);
            assert!(expected == actual, "N={n}, threads={threads}");
        }
        let source = data.categorical_sources()[0].prepared_source_index();
        data.sample.regressors[source][0] += 100.0;
        data.sample.outcome[0] = f64::NAN;
        let run = |_disabled| {
            fit_with_controls_internal(
                &data,
                PreparedOlsFitConfig::default(),
                &mut SystemFitReservation,
                FitPerturbations {
                    ..FitPerturbations::default()
                },
            )
        };
        assert_eq!(run(true).unwrap_err(), run(false).unwrap_err());
    }
}

#[test]
fn one_fe_final_target_recovery_preserves_saved_results() {
    for n in [360, 16_385] {
        let mut columns: Vec<Vec<f64>> = (0..9)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        if n == 360 {
            columns.push(columns[0].clone());
        }
        let groups: Vec<_> = (0..n).map(|r| (r / 5) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| {
                (groups[r] / 19.0).sin()
                    + columns[0][r] * 1.3
                    + columns[1][r] * 0.7
                    + (r as f64).cos() * 0.1
            })
            .collect();
        let data = prepared_multiway_categorical(&response, &columns, &[groups]);
        for threads in [1, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
            config.absorb.retain_coefficients = true;
            config.allow_final_fe_recovery = true;
            config.final_residual_tolerance = Some(1e-14);
            let run = |_disabled| {
                ThreadTeam::new(config.execution.threads)
                    .install(|| {
                        fit_with_controls_internal(
                            &data,
                            config,
                            &mut SystemFitReservation,
                            FitPerturbations {
                                one_fe_final_recovery_disabled: _disabled,
                                ..FitPerturbations::default()
                            },
                        )
                    })
                    .unwrap()
                    .unwrap()
            };
            let expected = run(true);
            let actual = run(false);
            assert!(expected.recovered_fe_coefficients().is_none());
            assert!(actual.recovered_fe_coefficients().is_none());
            assert_eq!(actual.finite_rank(), expected.finite_rank());
            for (a, b) in actual.coefficients().iter().zip(expected.coefficients()) {
                assert_eq!(a.status, b.status);
                match (a.value, b.value) {
                    (Some(a), Some(b)) => assert_close(a, b, 1e-10),
                    (None, None) => {}
                    _ => panic!("coefficient presence changed"),
                }
            }
            for (&a, &b) in actual.residuals().iter().zip(expected.residuals()) {
                assert_close(a, b, 1e-10);
            }
            let recover = |fit: &CertifiedPreparedOlsFit| {
                let target: Vec<_> = (0..n)
                    .map(|r| {
                        let mut xb = 0.0_f64;
                        for (column, coefficient) in columns.iter().zip(fit.coefficients()) {
                            if let Some(value) = coefficient.value {
                                xb = value.mul_add(column[r], xb);
                            }
                        }
                        xb += fit.coefficients().last().unwrap().value.unwrap();
                        response[r] - xb - fit.residuals()[r]
                    })
                    .collect();
                ThreadTeam::new(config.execution.threads)
                    .install(|| {
                        data.recover_absorbed_effects_fast(
                            &target,
                            &[true],
                            fit.recovered_fe_coefficients(),
                            false,
                            false,
                            config.absorb.lsmr,
                            crate::fe::MapConfig {
                                tolerance: 1e-12,
                                ..config.absorb.map
                            },
                            None,
                        )
                    })
                    .unwrap()
                    .unwrap()
                    .into_optional_parts()
                    .1
                    .remove(0)
                    .unwrap()
            };
            for (a, b) in recover(&actual).into_iter().zip(recover(&expected)) {
                assert_close(a, b, 1e-9);
            }
            config.allow_final_fe_recovery = false;
            let required = fit_prepared_ols_coefficients_view_map(&data, config).unwrap();
            assert_eq!(
                required.recovered_fe_coefficients(),
                expected.recovered_fe_coefficients()
            );
        }
    }
}

#[test]
fn residual_sums_stages_repeat_complete_fits_and_error_order() {
    for n in [16_383, 16_384, 32_771] {
        let columns: Vec<Vec<f64>> = (0..9)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1)
            .collect();
        let mut data = prepared(
            &response,
            &columns,
            &[false; 9],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        for threads in [1, 4, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
            let run = |data: &PreparedOlsData, _repeat: bool| {
                ThreadTeam::new(config.execution.threads)
                    .install(|| {
                        fit_with_controls_internal(
                            data,
                            config,
                            &mut SystemFitReservation,
                            FitPerturbations::default(),
                        )
                    })
                    .unwrap()
                    .map(without_solve_timings)
            };
            let expected = run(&data, true);
            let actual = run(&data, false);
            assert!(expected == actual, "N={n}, threads={threads}");
        }
        let source = data.categorical_sources()[0].prepared_source_index();
        data.sample.regressors[source][0] += 100.0;
        data.sample.outcome[0] = f64::NAN;
        let run = |_repeat: bool| {
            fit_with_controls_internal(
                &data,
                PreparedOlsFitConfig::default(),
                &mut SystemFitReservation,
                FitPerturbations::default(),
            )
        };
        assert_eq!(run(true).unwrap_err(), run(false).unwrap_err());
    }
}

#[test]
fn score_stages_repeat_complete_fits_and_error_order() {
    for n in [16_383, 16_384, 32_771] {
        let columns: Vec<Vec<f64>> = (0..9)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1)
            .collect();
        let mut data = prepared(
            &response,
            &columns,
            &[false; 9],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        for threads in [1, 4, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
            let run = |data: &PreparedOlsData, _repeat: bool, _raw_repeat: bool| {
                ThreadTeam::new(config.execution.threads)
                    .install(|| {
                        fit_with_controls_internal(
                            data,
                            config,
                            &mut SystemFitReservation,
                            FitPerturbations::default(),
                        )
                    })
                    .unwrap()
                    .map(without_solve_timings)
            };
            let expected = run(&data, true, true);
            let actual = run(&data, false, false);
            assert_eq!(
                run(&data, false, true),
                actual,
                "raw residual repeat N={n}, threads={threads}"
            );
            assert!(expected == actual, "N={n}, threads={threads}");
        }
        let source = data.categorical_sources()[0].prepared_source_index();
        data.sample.regressors[source][0] += 100.0;
        data.sample.outcome[0] = f64::NAN;
        let run = |_repeat: bool| {
            fit_with_controls_internal(
                &data,
                PreparedOlsFitConfig::default(),
                &mut SystemFitReservation,
                FitPerturbations::default(),
            )
        };
        assert_eq!(run(true).unwrap_err(), run(false).unwrap_err());
    }
}

#[test]
fn unit_intercept_score_matches_generic_weights_and_errors() {
    for n in [1, 127, 16_384, 16_387] {
        let y: Vec<_> = (0..n).map(|r| (r as f64).sin()).collect();
        let groups = vec![0.0; n];
        let data = prepared(&y, &[], &[], Some(&groups), true, TestWeights::Unit);
        let ones = vec![1.0; n];
        let unit = NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        };
        let generic = NormalizedWeights {
            values: Some(&ones),
            raw_scale: 1.0,
            kind: Some(WeightKind::Frequency),
        };
        for scale in [0.0, 1e-200, 1.0, 1e150] {
            let mut residuals: Vec<_> = y.iter().map(|v| v * scale).collect();
            let call = |residuals: &[f64], weights| {
                certify_scores(
                    &data,
                    residuals,
                    weights,
                    &[(n as f64).sqrt()],
                    &[0.0],
                    FiniteScoreDraft::default(),
                    &[PreparedOlsFitStatus::NormalizedAbsorbedIntercept],
                    scale,
                    1e-10,
                )
            };
            assert_eq!(call(&residuals, unit), call(&residuals, generic));
            residuals[n / 2] = f64::NAN;
            assert_eq!(call(&residuals, unit), call(&residuals, generic));
        }
    }
}

#[test]
fn two_fe_schur_final_stages_repeat_results_caches_and_error_order() {
    let n = 16_385;
    for width in [5, 6, 10, 17] {
        let columns = (0..width)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let effects = [
            (0..n).map(|r| (r % 41) as f64).collect::<Vec<_>>(),
            (0..n).map(|r| ((r / 41) % 37) as f64).collect::<Vec<_>>(),
        ];
        let response = (0..n)
            .map(|r| columns[0][r] * 1.3 + effects[0][r] + effects[1][r] + 0.1 * (r as f64).cos())
            .collect::<Vec<_>>();
        let data = prepared_multiway_categorical(&response, &columns, &effects);
        let mut config = PreparedOlsFitConfig {
            allow_final_fe_recovery: true,
            ..PreparedOlsFitConfig::default()
        };
        config.absorb.retain_coefficients = true;
        config.execution.threads = ThreadConfig::from_requested(8, true).unwrap();
        for (cache, limit) in [(false, 0), (true, 0), (true, MAX_FAST_SCORE_CACHE_BYTES)] {
            for perturbation in [
                FitPerturbations::default(),
                FitPerturbations {
                    reported_bread_extension: true,
                    reconstructed_within_residual_constant: true,
                    ..FitPerturbations::default()
                },
                FitPerturbations {
                    final_residual_constant: true,
                    ..FitPerturbations::default()
                },
            ] {
                let run = |_repeat: bool| {
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            fit_with_controls_internal_mode(
                                &data,
                                config,
                                &mut SystemFitReservation,
                                perturbation,
                                cache,
                                limit,
                            )
                        })
                        .unwrap()
                        .map(without_solve_timings)
                };
                assert_eq!(
                    run(false),
                    run(true),
                    "width={width} cache={cache} limit={limit} perturbation={perturbation:?}"
                );
            }
        }
    }
}

#[test]
fn narrow_weighted_final_stages_repeat_fits_and_error_order() {
    let n = 16_385;
    let weights = (0..n).map(|r| (1 + r % 7) as f64).collect::<Vec<_>>();
    for width in [5, 6] {
        let columns = (0..width)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let effects = [
            (0..n).map(|r| (r % 41) as f64).collect::<Vec<_>>(),
            (0..n).map(|r| ((r / 41) % 37) as f64).collect::<Vec<_>>(),
        ];
        let response = (0..n)
            .map(|r| columns[0][r] * 1.3 + effects[0][r] + effects[1][r] + 0.1 * (r as f64).cos())
            .collect::<Vec<_>>();
        for effects in [&effects[..1], &effects[..]] {
            for weights in [
                TestWeights::Analytic(&weights),
                TestWeights::Probability(&weights),
                TestWeights::Frequency(&weights),
            ] {
                let data =
                    prepared_multiway_categorical_weights(&response, &columns, effects, weights);
                let mut config = PreparedOlsFitConfig {
                    allow_final_fe_recovery: true,
                    ..PreparedOlsFitConfig::default()
                };
                config.absorb.retain_coefficients = true;
                config.execution.threads = ThreadConfig::from_requested(8, true).unwrap();
                for (cache, limit) in [(false, 0), (true, 0), (true, MAX_FAST_SCORE_CACHE_BYTES)] {
                    for perturbation in [
                        FitPerturbations::default(),
                        FitPerturbations {
                            reported_bread_extension: true,
                            reconstructed_within_residual_constant: true,
                            ..FitPerturbations::default()
                        },
                        FitPerturbations {
                            final_residual_constant: true,
                            ..FitPerturbations::default()
                        },
                    ] {
                        let run = |_repeat: bool| {
                            ThreadTeam::new(config.execution.threads)
                                .install(|| {
                                    fit_with_controls_internal_mode(
                                        &data,
                                        config,
                                        &mut SystemFitReservation,
                                        perturbation,
                                        cache,
                                        limit,
                                    )
                                })
                                .unwrap()
                                .map(without_solve_timings)
                        };
                        assert_eq!(run(false), run(true), "width={width} effects={} cache={cache} limit={limit} perturbation={perturbation:?}", effects.len());
                    }
                }
            }
        }
    }
}

#[test]
fn final_residual_stages_repeat_bits_and_rejection_order() {
    for n in [16_383, 16_384, 32_771] {
        let columns: Vec<Vec<f64>> = (0..9)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1)
            .collect();
        let data = prepared(
            &response,
            &columns,
            &[false; 9],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        for threads in [4, 8, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
            for perturbation in [
                FitPerturbations::default(),
                FitPerturbations {
                    reported_intercept: true,
                    ..FitPerturbations::default()
                },
                FitPerturbations {
                    reported_bread_extension: true,
                    ..FitPerturbations::default()
                },
                FitPerturbations {
                    reconstructed_within_residual_constant: true,
                    ..FitPerturbations::default()
                },
                FitPerturbations {
                    final_residual_constant: true,
                    ..FitPerturbations::default()
                },
            ] {
                let run = |_repeat: bool| {
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            fit_with_controls_internal(
                                &data,
                                config,
                                &mut SystemFitReservation,
                                perturbation,
                            )
                        })
                        .unwrap()
                        .map(without_solve_timings)
                };
                let expected = run(true);
                let actual = run(false);
                assert!(
                    expected == actual,
                    "N={n}, threads={threads}, perturbation={perturbation:?}, errors={:?}/{:?}",
                    expected.as_ref().err(),
                    actual.as_ref().err()
                );
            }
        }
    }
}

#[test]
fn wide_residual_stages_repeat_fits_caches_and_error_order() {
    let n = 16_385;
    for width in [17, 50] {
        let mut columns: Vec<Vec<f64>> = (0..width)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        columns[width - 1] = columns[0].clone();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1)
            .collect();
        let data = prepared(
            &response,
            &columns,
            &vec![false; width],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        let mut config = PreparedOlsFitConfig::default();
        config.execution.threads = ThreadConfig::from_requested(8, true).unwrap();
        for (cache, limit) in [(false, 0), (true, 0), (true, MAX_FAST_SCORE_CACHE_BYTES)] {
            for perturbation in [
                FitPerturbations::default(),
                FitPerturbations {
                    reported_bread_extension: true,
                    reconstructed_within_residual_constant: true,
                    ..FitPerturbations::default()
                },
                FitPerturbations {
                    final_residual_constant: true,
                    ..FitPerturbations::default()
                },
            ] {
                let run = |_repeat: bool| {
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            fit_with_controls_internal_mode(
                                &data,
                                config,
                                &mut SystemFitReservation,
                                perturbation,
                                cache,
                                limit,
                            )
                        })
                        .unwrap()
                        .map(without_solve_timings)
                };
                assert_eq!(
                    run(false),
                    run(true),
                    "width={width} cache={cache} limit={limit} perturbation={perturbation:?}"
                );
            }
        }
    }
}

#[test]
fn retained_score_cache_residual_stages_repeat_fits_and_errors() {
    for n in [16_383, 16_384, 32_771] {
        let columns: Vec<Vec<f64>> = (0..9)
            .map(|c| {
                (0..n)
                    .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                    .collect()
            })
            .collect();
        let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
        let response: Vec<_> = (0..n)
            .map(|r| groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1)
            .collect();
        let data = prepared(
            &response,
            &columns,
            &[false; 9],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        for threads in [1, 8, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
            for cache_limit in [0, MAX_FAST_SCORE_CACHE_BYTES] {
                for perturbation in [
                    FitPerturbations::default(),
                    FitPerturbations {
                        reported_intercept: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        reported_bread_extension: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        reconstructed_within_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        final_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                ] {
                    let run = |_repeat: bool| {
                        ThreadTeam::new(config.execution.threads)
                            .install(|| {
                                fit_with_controls_internal_mode(
                                    &data,
                                    config,
                                    &mut SystemFitReservation,
                                    perturbation,
                                    true,
                                    cache_limit,
                                )
                            })
                            .unwrap()
                            .map(|fit| {
                                assert!(fit.fast_score_cache.is_some());
                                without_solve_timings(fit)
                            })
                    };
                    assert_eq!(run(false), run(true),
                            "n={n} threads={threads} cache_limit={cache_limit} perturbation={perturbation:?}");
                }
            }
        }
    }
}

#[test]
fn numeric_tiles_preserve_rhs_and_raw_residual_bits_across_threads() {
    let rows = 4_103;
    for width in [1, 5, 6, 7, 15, 50, 63, 64] {
        let columns = (0..width)
            .map(|c| {
                (0..rows)
                    .map(|r| {
                        if r % 17 == 0 {
                            -0.0
                        } else {
                            ((r * (c + 3) % 997) as f64).sin()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let response = (0..rows)
            .map(|r| ((r * 37 % 991) as f64).cos())
            .collect::<Vec<_>>();
        let prepared = prepared(
            &response,
            &columns,
            &vec![false; width],
            None,
            true,
            TestWeights::Unit,
        );
        let make_expected = || {
            let mut values = Vec::new();
            let mut residuals = response.clone();
            let coefficients = (0..prepared.model.columns.len())
                .map(|c| {
                    if c % 5 == 0 {
                        None
                    } else {
                        Some((c as f64 - 13.0) / 31.0)
                    }
                })
                .collect::<Vec<_>>();
            for r in 0..rows {
                values.push(response[r]);
                // Keep the legacy matrix indices and numerical traversal order explicit.
                #[allow(clippy::needless_range_loop)]
                for c in 0..prepared.model.columns.len() {
                    let value = prepared.column_value(c, r).unwrap();
                    values.push(value);
                    if let Some(coefficient) = coefficients[c] {
                        residuals[r] = (-coefficient).mul_add(value, residuals[r]);
                    }
                }
            }
            (values, residuals, coefficients)
        };
        let (expected_values, expected_residuals, coefficients) = make_expected();
        for threads in [1, 4, 16] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let actual = prepared
                        .fit_build_rhs_with_response(&prepared.fit_sample().outcome)
                        .unwrap();
                    assert_eq!(
                        actual
                            .as_slice()
                            .iter()
                            .map(|v| v.to_bits())
                            .collect::<Vec<_>>(),
                        expected_values
                            .iter()
                            .map(|v| v.to_bits())
                            .collect::<Vec<_>>()
                    );
                    if let Some(bound) = crate::ols::NumericOlsColumns::bind_residual_mean(
                        &prepared.model,
                        &prepared.sample,
                    )
                    .unwrap()
                    {
                        for row in 0..rows {
                            assert_eq!(
                                bound
                                    .residual_at(&coefficients, row, response[row])
                                    .unwrap()
                                    .to_bits(),
                                expected_residuals[row].to_bits()
                            );
                        }
                        assert!(bound
                            .residual_at(&coefficients[..1], 0, response[0])
                            .is_err());
                        assert!(bound.residual_at(&coefficients, rows, response[0]).is_err());
                        assert!(bound.residual_at(&coefficients, 0, f64::NAN).is_err());
                    } else {
                        assert!(width >= 64);
                    }
                    let bound =
                        crate::ols::NumericOlsColumns::bind(&prepared.model, &prepared.sample)
                            .unwrap();
                    if let Some(bound) = bound {
                        let mut residuals = response.clone();
                        bound
                            .subtract_fitted(&coefficients, &mut residuals)
                            .unwrap();
                        assert_eq!(
                            residuals.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                            expected_residuals
                                .iter()
                                .map(|v| v.to_bits())
                                .collect::<Vec<_>>()
                        );
                        assert!(bound
                            .subtract_fitted(&coefficients[..1], &mut residuals)
                            .is_err());
                        assert!(bound
                            .fill_reserved_rhs(
                                &response[..1],
                                &mut vec![0.0; expected_values.len()]
                            )
                            .is_err());
                        let mut invalid = coefficients.clone();
                        invalid[0] = Some(f64::INFINITY);
                        assert!(bound.subtract_fitted(&invalid, &mut residuals).is_err());
                    } else {
                        assert!(!(7..64).contains(&width));
                    }
                });
        }
    }
}

#[test]
fn numeric_tiles_support_omissions_working_responses_and_categorical_topology() {
    let rows = 4_103;
    let x = (0..12)
        .map(|c| {
            (0..rows)
                .map(|r| ((r + c * 3) % 11) as f64)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let outcome = vec![1.0; rows];
    let omitted = (0..12).map(|c| c % 4 == 0).collect::<Vec<_>>();
    let mut numeric = prepared(&outcome, &x, &omitted, None, false, TestWeights::Unit);
    let effects = vec![
        (0..rows).map(|r| (r % 17) as f64).collect(),
        (0..rows).map(|r| (r % 23) as f64).collect(),
    ];
    let categorical = prepared_multiway_categorical(&outcome, &x, &effects);
    let working = (0..rows).map(|r| (r % 7) as f64 - 2.5).collect::<Vec<_>>();
    for view in [&numeric as &dyn PreparedOlsFitView, &categorical] {
        let actual = view.fit_build_rhs_with_response(&working).unwrap();
        let generated = view.fit_model().columns.len();
        let coefficients = (0..generated)
            .map(|c| {
                if c % 3 == 0 {
                    None
                } else {
                    Some(c as f64 / 7.0)
                }
            })
            .collect::<Vec<_>>();
        let mut expected_residual = working.clone();
        for (r, &response) in working.iter().enumerate() {
            assert_eq!(actual[(r, 0)].to_bits(), response.to_bits());
            for c in 0..generated {
                assert_eq!(
                    actual[(r, c + 1)].to_bits(),
                    view.fit_column_value(c, r).unwrap().to_bits()
                );
            }
        }
        for (c, coefficient) in coefficients.iter().enumerate() {
            if let Some(coefficient) = coefficient {
                view.fit_subtract_raw_column(c, *coefficient, &mut expected_residual)
                    .unwrap();
            }
        }
        let bound = crate::ols::NumericOlsColumns::bind(view.fit_model(), view.fit_sample())
            .unwrap()
            .unwrap();
        let mut residual = working.clone();
        bound.subtract_fitted(&coefficients, &mut residual).unwrap();
        assert_eq!(residual, expected_residual);
        let mut matrix = Vec::new();
        matrix.try_reserve_exact(actual.as_slice().len()).unwrap();
        let address = matrix.as_ptr();
        let capacity = matrix.capacity();
        bound.fill_reserved_rhs(&working, &mut matrix).unwrap();
        assert_eq!(address, matrix.as_ptr());
        assert_eq!(capacity, matrix.capacity());
        assert_eq!(matrix, actual.as_slice());
        assert_eq!(
            bound
                .fill_reserved_rhs(&working, &mut Vec::new())
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        // Finite operands can overflow on an intermediate FMA. Later
        // cancellation must not turn this into a successful residual.
        let coefficients = vec![Some(f64::MAX); generated];
        assert_eq!(
            bound
                .subtract_fitted(&coefficients, &mut residual)
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
    }
    numeric.sample.regressors[1][200] = f64::NAN;
    assert_eq!(
        numeric
            .fit_build_rhs_with_response(&numeric.fit_sample().outcome)
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
    numeric.sample.regressors[1][200] = 0.0;
    numeric.sample.regressors[1].pop();
    assert!(crate::ols::NumericOlsColumns::bind(&numeric.model, &numeric.sample).is_err());
}

#[test]
fn two_effect_graph_recovery_admits_exact_finishing_budget() {
    let n = 16_512;
    let effects = [
        (0..n).map(|r| (r % 17) as f64).collect::<Vec<_>>(),
        (0..n).map(|r| ((r / 17) % 13) as f64).collect::<Vec<_>>(),
    ];
    let target = (0..n)
        .map(|r| effects[0][r] * 0.1 - effects[1][r] * 0.07)
        .collect::<Vec<_>>();
    let x = vec![(0..n).map(|r| (r as f64).sin()).collect::<Vec<_>>()];
    let prepared = prepared_multiway_categorical(&target, &x, &effects);
    let planned = crate::memory::estimate_intercept_recovery_memory(n, &[17, 13], 2, 8, Some(4))
        .unwrap()
        .retained_peak_bytes;
    let threads = ThreadConfig::from_requested(8, false).unwrap();
    let supplied = (0..n).map(|r| (1 + r % 7) as f64).collect::<Vec<_>>();
    crate::ThreadTeam::new(threads)
        .install(|| {
            for weights in [
                BorrowedMapWeights::Unit,
                BorrowedMapWeights::Provided(&supplied),
            ] {
                for usable in [planned, planned - 1] {
                    let recovered = prepared.recover_absorbed_effects_fast_with_weights(
                        weights,
                        &target,
                        &[true, true],
                        None,
                        true,
                        false,
                        crate::fe::LsmrConfig::default(),
                        crate::fe::MapConfig {
                            tolerance: 1e-12,
                            ..crate::fe::MapConfig::default()
                        },
                        Some(crate::memory::FeSolverMemoryBudget {
                            execution_bytes: 1234,
                            usable_bytes: 1234 + usable,
                        }),
                    );
                    if usable == planned {
                        let recovered = recovered.unwrap();
                        assert_eq!(
                            recovered.recovery_method(),
                            crate::fe::CategoricalRecoveryMethod::Direct
                        );
                        assert!(recovered.reconstruction_relative_infinity() < 1e-12);
                    } else {
                        assert_eq!(recovered.unwrap_err().code, ErrorCode::Allocation);
                    }
                }
            }
        })
        .unwrap();
}

#[test]
fn fused_standardized_moments_match_compensated_scalar_reference() {
    let rows = 40_003_usize;
    let columns = 5_usize;
    let mut values = Vec::with_capacity(rows * columns);
    for row in 0..rows {
        let axis = row as f64;
        values.extend_from_slice(&[
            (axis * 0.013).sin() + 0.4 * (axis * 0.007).cos(),
            (axis * 0.011).sin(),
            (axis * 0.017).cos(),
            ((row % 29) as f64 - 14.0) / 17.0,
            ((row * 37 % 101) as f64 - 50.0) / 31.0,
        ]);
    }
    let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("moment matrix");
    let candidates = [0_usize, 1, 2, 3];
    let mut reservation = SystemFitReservation;
    let (rhs, gram) = test_projected_moments(&matrix, &candidates, &mut reservation)
        .expect("fused standardized moments");
    for (left, generated) in candidates.iter().copied().enumerate() {
        let mut expected_rhs = 0.0_f64;
        let mut rhs_correction = 0.0_f64;
        for row in 0..rows {
            compensated_add(
                matrix[(row, generated + 1)] * matrix[(row, 0)],
                &mut expected_rhs,
                &mut rhs_correction,
            )
            .expect("reference RHS");
        }
        assert_close(rhs[left], expected_rhs + rhs_correction, 2e-12);
        for (right, other_generated) in candidates.iter().copied().enumerate() {
            let mut expected_gram = 0.0_f64;
            let mut gram_correction = 0.0_f64;
            for row in 0..rows {
                compensated_add(
                    matrix[(row, generated + 1)] * matrix[(row, other_generated + 1)],
                    &mut expected_gram,
                    &mut gram_correction,
                )
                .expect("reference Gram");
            }
            assert_close(
                gram[left * candidates.len() + right],
                expected_gram + gram_correction,
                2e-12,
            );
        }
    }
}

#[test]
fn fused_weighted_norms_match_columnwise_reference_exactly() {
    let rows = 40_007_usize;
    let columns = 7_usize;
    let mut values = Vec::with_capacity(rows * columns);
    let mut weights = Vec::with_capacity(rows);
    for row in 0..rows {
        let axis = row as f64;
        for column in 0..columns {
            let frequency = (column + 3) as f64 * 0.000_017;
            values.push((axis * frequency).sin() + (column as f64 - 2.0) * 0.125);
        }
        weights.push(0.25 + ((row * 37 % 97) as f64 + 1.0) / 101.0);
    }
    let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("norm matrix");
    let raw_scale = weights.iter().copied().fold(0.0_f64, f64::max);
    let normalized = NormalizedWeights {
        values: Some(&weights),
        raw_scale,
        kind: Some(WeightKind::Analytic),
    };
    let expected = (0..columns)
        .map(|column| {
            weighted_matrix_column_norm(&matrix, column, normalized)
                .expect("columnwise weighted norm")
        })
        .collect::<Vec<_>>();
    let mut actual = vec![0.0_f64; columns];
    let mut reservation = SystemFitReservation;
    fill_weighted_matrix_norms_fused(&matrix, normalized, &mut actual, &mut reservation)
        .expect("fused weighted norms");
    assert_eq!(actual, expected);
}

#[test]
fn blocked_weighted_norms_preserve_scalar_bits_at_all_widths() {
    for threads in [1, 7] {
        let team = ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap());
        team.install(|| {
            for columns in [1, 2, 3, 4, 5, 7, 8, 9, 13, 51, 65] {
                let rows = 1031;
                let mut matrix = DenseMatrix::zeros(rows, columns).unwrap();
                let weights = (0..rows)
                    .map(|row| 0.25 + (row % 17) as f64 / 19.0)
                    .collect::<Vec<_>>();
                for row in 0..rows {
                    for col in 0..columns {
                        matrix[(row, col)] = match col % 5 {
                            0 => 0.0,
                            1 => -0.0,
                            2 => ((row * 37 % 101) as f64 - 50.0) * 1e150,
                            3 => ((row * 19 % 97) as f64 - 48.0) * 1e-150,
                            _ => (row as f64 * 0.173).sin(),
                        };
                    }
                }
                for values in [None, Some(weights.as_slice())] {
                    let normalized = NormalizedWeights {
                        values,
                        raw_scale: 2.0,
                        kind: values.map(|_| WeightKind::Analytic),
                    };
                    let expected = (0..columns)
                        .map(|col| {
                            weighted_matrix_column_norm(&matrix, col, normalized)
                                .unwrap()
                                .to_bits()
                        })
                        .collect::<Vec<_>>();
                    let mut actual = vec![0.0; columns];
                    for block in [2, 4, 8] {
                        match block {
                            2 => fill_weighted_matrix_norms_blocked::<2>(
                                &matrix,
                                0,
                                normalized,
                                &mut actual,
                            ),
                            4 => fill_weighted_matrix_norms_blocked::<4>(
                                &matrix,
                                0,
                                normalized,
                                &mut actual,
                            ),
                            _ => fill_weighted_matrix_norms_blocked::<8>(
                                &matrix,
                                0,
                                normalized,
                                &mut actual,
                            ),
                        }
                        .unwrap();
                        assert_eq!(
                            actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                            expected
                        );
                    }
                }
            }
        })
        .unwrap();
    }
}

#[test]
fn low_norm_preserves_frozen_large_row_reduction_bits() {
    // Independent former Low recurrence and ordered 16,384-row merge.
    fn accumulate(values: &[f64], raw: Option<&[f64]>, divisor: f64) -> (f64, f64) {
        let (mut scale, mut sum) = (0.0_f64, 1.0_f64);
        for (row, &value) in values.iter().enumerate() {
            let root = raw.map_or(1.0, |w| (w[row] / divisor).sqrt());
            let absolute = (root * value).abs();
            if absolute == 0.0 {
                continue;
            }
            if scale < absolute {
                let ratio = scale / absolute;
                sum = 1.0 + sum * ratio * ratio;
                scale = absolute;
            } else {
                let ratio = absolute / scale;
                sum += ratio * ratio;
            }
        }
        (scale, sum)
    }
    for rows in [16_383, 16_384, 16_385, 32_771] {
        let values: Vec<_> = (0..rows)
            .map(|r| match r % 5 {
                0 => -0.0,
                1 => 1e-150,
                2 => -1e120,
                _ => ((r * 37 % 173) as f64 - 86.0) / 91.0,
            })
            .collect();
        let raw: Vec<_> = (0..rows).map(|r| [8.0, 0.25, 1e-120][r % 3]).collect();
        for weighted in [false, true] {
            let weights = NormalizedWeights {
                values: weighted.then_some(raw.as_slice()),
                raw_scale: if weighted { 8.0 } else { 1.0 },
                kind: weighted.then_some(WeightKind::Analytic),
            };
            for threads in [1, 4, 16] {
                let (scale, sum) = if threads == 1 || rows <= 16_384 {
                    accumulate(&values, weights.values, weights.raw_scale)
                } else {
                    let (mut scale, mut sum) = (0.0_f64, 1.0_f64);
                    for start in (0..rows).step_by(16_384) {
                        let end = (start + 16_384).min(rows);
                        let (other_scale, other_sum) = accumulate(
                            &values[start..end],
                            weights.values.map(|w| &w[start..end]),
                            weights.raw_scale,
                        );
                        if other_scale == 0.0 {
                            continue;
                        }
                        if scale < other_scale {
                            let ratio = scale / other_scale;
                            sum = other_sum + sum * ratio * ratio;
                            scale = other_scale;
                        } else {
                            let ratio = other_scale / scale;
                            sum += other_sum * ratio * ratio;
                        }
                    }
                    (scale, sum)
                };
                let expected = scale * sum.sqrt();
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let actual = pool
                    .install(|| weighted_slice_norm(&values, weights))
                    .unwrap();
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "rows={rows}, weighted={weighted}, threads={threads}"
                );
            }
        }
    }
}

#[test]
fn blocked_column_maxima_match_columnwise_scans() {
    let rows = 37;
    for columns in [1, 2, 9, 17] {
        let data = (0..rows * columns)
            .map(|i| ((i * 7919) % 211) as f64 - 105.5)
            .collect();
        let matrix = DenseMatrix::from_row_major(rows, columns, data).unwrap();
        for first in 0..columns.min(2) {
            let start = [0.0_f64, 50.0, 3.25, 400.0];
            let width = columns - first;
            let initial = (0..width).map(|j| start[j % 4]).collect::<Vec<_>>();
            let expected = (0..width)
                .map(|j| (0..rows).fold(initial[j], |m, row| m.max(matrix[(row, first + j)].abs())))
                .collect::<Vec<_>>();
            for threads in [1, 3] {
                let mut actual = initial.clone();
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        fill_column_maxima_blocked::<4>(&matrix, first, &mut actual).unwrap();
                    });
                assert_eq!(actual, expected);
            }
        }
    }
    let matrix = DenseMatrix::zeros(2, 2).unwrap();
    assert!(fill_column_maxima_blocked::<4>(&matrix, 1, &mut [0.0; 2]).is_err());
    assert!(fill_column_maxima_blocked::<4>(&matrix, 3, &mut []).is_err());
}

#[test]
fn blocked_weighted_norms_fail_closed() {
    let mut matrix = DenseMatrix::zeros(3, 2).unwrap();
    let weights = NormalizedWeights {
        values: None,
        raw_scale: 1.0,
        kind: None,
    };
    assert!(fill_weighted_matrix_norms_blocked::<4>(&matrix, 0, weights, &mut [0.0]).is_err());
    assert!(fill_weighted_matrix_norms_blocked::<0>(&matrix, 0, weights, &mut [0.0; 2]).is_err());
    matrix[(1, 1)] = f64::INFINITY;
    assert!(fill_weighted_matrix_norms_blocked::<4>(&matrix, 0, weights, &mut [0.0; 2]).is_err());
    matrix[(1, 1)] = 1.0;
    let bad_weights = [1.0, 0.0, 1.0];
    let weights = NormalizedWeights {
        values: Some(&bad_weights),
        raw_scale: 1.0,
        kind: Some(WeightKind::Analytic),
    };
    assert!(fill_weighted_matrix_norms_blocked::<4>(&matrix, 0, weights, &mut [0.0; 2]).is_err());
}

#[test]
fn fused_standardized_moments_preserve_noncontiguous_candidate_fallback() {
    let rows = 40_019_usize;
    let columns = 6_usize;
    let mut values = Vec::with_capacity(rows * columns);
    for row in 0..rows {
        let axis = row as f64;
        values.extend_from_slice(&[
            (axis * 0.019).sin(),
            (axis * 0.023).cos(),
            ((row % 31) as f64 - 15.0) / 19.0,
            (axis * 0.029).sin(),
            ((row * 41 % 109) as f64 - 54.0) / 37.0,
            (axis * 0.031).cos(),
        ]);
    }
    let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("moment matrix");
    let candidates = [0_usize, 2, 4];
    let mut fused_reservation = SystemFitReservation;
    let (rhs, gram) = test_projected_moments(&matrix, &candidates, &mut fused_reservation)
        .expect("noncontiguous fused standardized moments");
    let mut reference_reservation = SystemFitReservation;
    let (reference_rhs, reference_gram) =
        build_standardized_moments_columnwise(&matrix, &candidates, &mut reference_reservation)
            .expect("noncontiguous scalar standardized moments");
    for (actual, expected) in rhs.iter().zip(reference_rhs) {
        assert_close(*actual, expected, 2e-12);
    }
    for (actual, expected) in gram.iter().zip(reference_gram) {
        assert_close(*actual, expected, 2e-12);
    }
}

#[test]
fn bound_weighted_column_means_preserve_current_weight_bits() {
    for (rows, width) in [
        (1, 1),
        (4096, 9),
        (16383, 33),
        (16384, 32),
        (32771, 17),
        (65537, 8),
    ] {
        let columns: Vec<Vec<f64>> = (0..width)
            .map(|column| {
                (0..rows)
                    .map(|row| match column % 6 {
                        0 => [1e300, 1e-300, -1e300][row % 3],
                        1 => ((row % 17) as f64 - 8.0) * 1e-300,
                        2 => -0.0,
                        3 => [1e16, 1.0, -1e16][row % 3],
                        _ => (row as f64 / 17.0).sin(),
                    })
                    .collect()
            })
            .collect();
        let raw: Vec<_> = columns
            .iter()
            .map(|v| Some(v.as_slice()))
            .chain([None])
            .collect();
        let generated: Vec<_> = (0..=width).rev().filter(|c| *c != 2).collect();
        let first: Vec<_> = (0..rows).map(|row| (row % 5 + 1) as f64).collect();
        let second: Vec<_> = (0..rows)
            .map(|row| {
                if row % 7 == 0 {
                    1e-250
                } else {
                    (row % 13 + 1) as f64
                }
            })
            .collect();
        for weights in [
            NormalizedWeights {
                values: None,
                kind: None,
                raw_scale: 1.0,
            },
            NormalizedWeights {
                values: Some(&first),
                kind: Some(WeightKind::Frequency),
                raw_scale: 1.0,
            },
            NormalizedWeights {
                values: Some(&second),
                kind: Some(WeightKind::Probability),
                raw_scale: 32.0,
            },
        ] {
            let expected: Vec<_> = generated
                .iter()
                .map(|&column| {
                    weighted_mean_by(rows, weights, |row| Ok(raw[column].map_or(1.0, |v| v[row])))
                        .unwrap()
                        .to_bits()
                })
                .collect();
            for threads in [1, 4] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let mut means = vec![0.0; generated.len()];
                        super::fill_bound_weighted_column_means(
                            rows, &raw, &generated, weights, &mut means,
                        )
                        .unwrap();
                        assert_eq!(
                            means.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                            expected,
                            "rows={rows}, width={width}, threads={threads}"
                        );
                    });
            }
        }
    }
}

#[test]
fn bound_weighted_column_means_reject_bad_axes_and_nonfinite_inputs() {
    let values = [1.0, 2.0];
    let unit = NormalizedWeights {
        values: None,
        kind: None,
        raw_scale: 1.0,
    };
    let mut means = [19.0];
    for (rows, generated) in [(1, 0), (2, 1)] {
        assert!(super::fill_bound_weighted_column_means(
            rows,
            &[Some(&values)],
            &[generated],
            unit,
            &mut means
        )
        .is_err());
        assert_eq!(means, [19.0]);
    }
    assert!(
        super::fill_bound_weighted_column_means(2, &[Some(&values)], &[], unit, &mut means)
            .is_err()
    );
    assert!(super::fill_bound_weighted_column_means(0, &[None], &[0], unit, &mut means).is_err());
    super::fill_bound_weighted_column_means(0, &[], &[], unit, &mut []).unwrap();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let bad = [1.0, invalid];
        assert!(
            super::fill_bound_weighted_column_means(2, &[Some(&bad)], &[0], unit, &mut means)
                .is_err()
        );
    }
    let invalid_weights = [1.0, 0.0];
    let weights = NormalizedWeights {
        values: Some(&invalid_weights),
        kind: Some(WeightKind::Probability),
        raw_scale: 1.0,
    };
    assert!(super::fill_bound_weighted_column_means(
        2,
        &[Some(&values)],
        &[0],
        weights,
        &mut means
    )
    .is_err());
}

#[test]
fn unit_mean_by_shortcut_matches_explicit_unit_weights_bits() {
    for n in [0, 1, 127, 16384, 16387] {
        let ones = vec![1.0; n];
        let unit = NormalizedWeights {
            values: None,
            kind: None,
            raw_scale: 1.0,
        };
        let supplied = NormalizedWeights {
            values: Some(&ones),
            kind: Some(WeightKind::Frequency),
            raw_scale: 1.0,
        };
        for scale in [1e-200, 1.0, 1e200] {
            let values: Vec<_> = (0..n).map(|r| ((r % 107) as f64 - 51.0) * scale).collect();
            assert_eq!(
                weighted_mean_by(n, unit, |r| Ok(values[r])).map(f64::to_bits),
                weighted_mean_by(n, supplied, |r| Ok(values[r])).map(f64::to_bits)
            );
            for fail in [0, n / 2, n] {
                let get = |r| {
                    if r == fail {
                        Err(invalid_data("injected mean input error"))
                    } else {
                        Ok(values[r])
                    }
                };
                assert_eq!(
                    weighted_mean_by(n, unit, get),
                    weighted_mean_by(n, supplied, get)
                );
            }
        }
    }
}

#[test]
fn residual_mean_bound_retains_cancellation_and_extreme_scale_tails() {
    let supplied = [1.0, 0.25, 0.5, 0.125];
    let tiny = [1e-300, 1.0, 1.0, 1.0];
    for values in [
        [1.0, 0.0, 0.0, 0.0],
        [1.0, -1.0, 2.0, -2.0],
        [1e-300, -1e-300, 2e-300, 4e-300],
        [1e300, -1e300, 2e299, 4e299],
        [
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            2.0 * f64::MIN_POSITIVE,
            0.0,
        ],
    ] {
        for weights in [
            NormalizedWeights {
                values: None,
                raw_scale: 1.0,
                kind: None,
            },
            NormalizedWeights {
                values: Some(&supplied),
                raw_scale: 1.0,
                kind: Some(WeightKind::Analytic),
            },
            NormalizedWeights {
                values: Some(&tiny),
                raw_scale: 1.0,
                kind: Some(WeightKind::Analytic),
            },
        ] {
            let expected = weighted_mean_by(4, weights, |r| Ok(values[r])).unwrap();
            let maximum = values.iter().copied().map(f64::abs).fold(0.0, f64::max);
            for bound in [maximum, maximum * 16.0, 1e308] {
                let actual =
                    weighted_mean_with_bound(4, weights, |r| Ok(values[r]), bound).unwrap();
                assert!(
                    (actual - expected).abs() <= 1e-14 * expected.abs(),
                    "values={values:?}, bound={bound}, actual={actual}, expected={expected}"
                );
                if maximum < bound * f64::MIN_POSITIVE {
                    assert_eq!(actual.to_bits(), expected.to_bits());
                }
            }
        }
    }
}

#[test]
fn residual_bound_requires_weighted_underflow_fallback() {
    let raw_weights = [1e-300, 1.0, 1.0, 1.0];
    let values = [1.0, 0.0, 0.0, 0.0];
    let weights = NormalizedWeights {
        values: Some(&raw_weights),
        raw_scale: 1.0,
        kind: Some(WeightKind::Analytic),
    };
    let expected = weighted_mean_by(4, weights, |r| Ok(values[r])).unwrap();
    assert_eq!(expected, 1e-300 / 3.0);
    assert_eq!(
        weighted_mean_with_scale(4, weights, |r| Ok(values[r]), 1e300, None).unwrap(),
        0.0
    );
    assert_eq!(
        weighted_mean_with_bound(4, weights, |r| Ok(values[r]), 1e300)
            .unwrap()
            .to_bits(),
        expected.to_bits()
    );
    let values = [1e-299, 0.0, 0.0, 0.0];
    let weights = NormalizedWeights {
        values: None,
        raw_scale: 1.0,
        kind: None,
    };
    let mut underflow = false;
    assert!(values[0] / 2e8 >= f64::MIN_POSITIVE);
    weighted_mean_with_scale(4, weights, |r| Ok(values[r]), 2e8, Some(&mut underflow)).unwrap();
    assert!(
        underflow,
        "division by total mass can also underflow the bounded mean"
    );
    let expected = weighted_mean_by(4, weights, |r| Ok(values[r])).unwrap();
    assert_eq!(
        weighted_mean_with_bound(4, weights, |r| Ok(values[r]), 2e8)
            .unwrap()
            .to_bits(),
        expected.to_bits()
    );
}

#[test]
fn blocked_raw_moments_preserve_columnwise_bits_and_dispatch() {
    for threads in [1, 7, 16] {
        let team = ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap());
        team.install(|| {
            for columns in [1, 7, 8, 9, 63, 64, 65, 66, 129] {
                let rows = 1031;
                let mut matrix = DenseMatrix::zeros(rows, columns).unwrap();
                let weights = (0..rows)
                    .map(|row| 1.0 + (row % 17) as f64)
                    .collect::<Vec<_>>();
                for row in 0..rows {
                    for column in 0..columns {
                        matrix[(row, column)] = match column % 7 {
                            0 => 0.0,
                            1 => -0.0,
                            2 => ((row * 37 % 101) as f64 - 50.0) * 1e150,
                            3 => ((row * 19 % 97) as f64 - 48.0) * 1e-150,
                            4 => 1.0,
                            5 => [1e16, 1.0, -1e16][row % 3],
                            _ => (row as f64 * 0.173).sin(),
                        };
                    }
                }
                for kind in [
                    None,
                    Some(WeightKind::Analytic),
                    Some(WeightKind::Frequency),
                    Some(WeightKind::Probability),
                ] {
                    let normalized = NormalizedWeights {
                        values: kind.map(|_| weights.as_slice()),
                        raw_scale: 17.0,
                        kind,
                    };
                    let mut reservation = SystemFitReservation;
                    let cache = build_raw_moment_cache(&matrix, normalized, &mut reservation)
                        .expect("dispatched raw moments");
                    let mut expected_scales = vec![0.0; columns];
                    let mut expected_means = vec![0.0; columns];
                    let mut expected_norms = vec![0.0; columns];
                    fill_raw_moment_cache_columnwise(
                        &matrix,
                        normalized,
                        cache.weight_sum,
                        &mut expected_scales,
                        &mut expected_means,
                        &mut expected_norms,
                    )
                    .unwrap();
                    for fill in [
                        fill_raw_moment_cache_blocked::<8>,
                        fill_raw_moment_cache_blocked::<1>,
                    ] {
                        let mut scales = vec![0.0; columns];
                        let mut means = vec![0.0; columns];
                        let mut norms = vec![0.0; columns];
                        fill(
                            &matrix,
                            normalized,
                            cache.weight_sum,
                            &mut scales,
                            &mut means,
                            &mut norms,
                        )
                        .unwrap();
                        for (actual, expected) in [
                            (&scales, &expected_scales),
                            (&means, &expected_means),
                            (&norms, &expected_norms),
                        ] {
                            assert_eq!(
                                actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                                expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
                            );
                        }
                    }
                    if columns > FUSED_MOMENT_MAX_COLUMNS {
                        for (actual, expected) in [
                            (&cache.scales, &expected_scales),
                            (&cache.means, &expected_means),
                            (&cache.norms, &expected_norms),
                        ] {
                            assert_eq!(
                                actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                                expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        })
        .unwrap();
    }
}

#[test]
fn blocked_raw_moments_fail_closed() {
    let mut matrix = DenseMatrix::zeros(3, 2).unwrap();
    let weights = NormalizedWeights {
        values: None,
        raw_scale: 1.0,
        kind: None,
    };
    assert!(fill_raw_moment_cache_blocked::<8>(
        &matrix,
        weights,
        3.0,
        &mut [0.0],
        &mut [0.0; 2],
        &mut [0.0; 2],
    )
    .is_err());
    assert!(fill_raw_moment_cache_blocked::<0>(
        &matrix,
        weights,
        3.0,
        &mut [0.0; 2],
        &mut [0.0; 2],
        &mut [0.0; 2],
    )
    .is_err());
    for weight_sum in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        assert!(fill_raw_moment_cache_blocked::<8>(
            &matrix,
            weights,
            weight_sum,
            &mut [0.0; 2],
            &mut [0.0; 2],
            &mut [0.0; 2],
        )
        .is_err());
    }
    for value in [f64::NAN, f64::INFINITY, f64::MAX] {
        matrix[(0, 1)] = value;
        matrix[(1, 1)] = value;
        let blocked = fill_raw_moment_cache_blocked::<8>(
            &matrix,
            weights,
            3.0,
            &mut [0.0; 2],
            &mut [0.0; 2],
            &mut [0.0; 2],
        )
        .unwrap_err();
        let reference = fill_raw_moment_cache_columnwise(
            &matrix,
            weights,
            3.0,
            &mut [0.0; 2],
            &mut [0.0; 2],
            &mut [0.0; 2],
        )
        .unwrap_err();
        assert_eq!(blocked.to_string(), reference.to_string());
    }
    matrix.as_mut_slice().fill(1.0);
    for invalid in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        let values = [1.0, invalid, 1.0];
        let weights = NormalizedWeights {
            values: Some(&values),
            raw_scale: 1.0,
            kind: Some(WeightKind::Analytic),
        };
        assert!(fill_raw_moment_cache_blocked::<8>(
            &matrix,
            weights,
            3.0,
            &mut [0.0; 2],
            &mut [0.0; 2],
            &mut [0.0; 2],
        )
        .is_err());
    }
}

#[test]
fn fused_raw_moment_cache_matches_columnwise_reference() {
    let rows = 40_021_usize;
    let columns = 7_usize;
    let mut values = Vec::with_capacity(rows * columns);
    let mut raw_weights = Vec::with_capacity(rows);
    for row in 0..rows {
        let axis = row as f64;
        raw_weights.push(0.25 + ((row * 17 % 43) + 1) as f64 / 11.0);
        values.extend_from_slice(&[
            (axis * 0.013).sin(),
            (axis * 0.017).cos(),
            ((row % 29) as f64 - 14.0) / 17.0,
            ((row * 37 % 101) as f64 - 50.0) / 31.0,
            0.0,
            1.0e120 * (axis * 0.007).sin(),
            1.0e-120 * (axis * 0.011).cos(),
        ]);
    }
    let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("raw matrix");
    let raw_scale = raw_weights.iter().copied().fold(0.0_f64, f64::max);
    for weights in [
        NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        },
        NormalizedWeights {
            values: Some(&raw_weights),
            raw_scale,
            kind: Some(WeightKind::Analytic),
        },
    ] {
        let mut reservation = SystemFitReservation;
        let cache =
            build_raw_moment_cache(&matrix, weights, &mut reservation).expect("fused raw moments");
        let mut scales = vec![0.0_f64; columns];
        let mut means = vec![0.0_f64; columns];
        let mut norms = vec![0.0_f64; columns];
        fill_raw_moment_cache_columnwise(
            &matrix,
            weights,
            cache.weight_sum,
            &mut scales,
            &mut means,
            &mut norms,
        )
        .expect("columnwise raw moments");
        assert_eq!(cache.scales, scales);
        for (actual, expected) in cache.means.iter().zip(means) {
            assert_close(*actual, expected, 3e-12);
        }
        for (actual, expected) in cache.norms.iter().zip(norms) {
            assert_close(*actual, expected, 3e-12);
        }
    }
}

#[test]
fn fused_raw_moment_block_cap_covers_tail_rows() {
    let rows = FUSED_MOMENT_MAX_BLOCKS * PARALLEL_REDUCTION_CHUNK_ROWS + 17;
    let mut values = vec![0.0_f64; rows * 2];
    for row in rows - 17..rows {
        values[row * 2] = 2.0;
        values[row * 2 + 1] = 3.0;
    }
    let matrix = DenseMatrix::from_row_major(rows, 2, values).expect("raw matrix");
    let mut reservation = SystemFitReservation;
    let cache = build_raw_moment_cache(
        &matrix,
        NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        },
        &mut reservation,
    )
    .expect("capped fused raw moments");
    assert_eq!(cache.scales, vec![2.0, 3.0]);
    assert_close(cache.means[0], 34.0 / rows as f64, 1e-15);
    assert_close(cache.means[1], 51.0 / rows as f64, 1e-15);
    assert_close(cache.norms[0], 2.0 * 17.0_f64.sqrt(), 1e-15);
    assert_close(cache.norms[1], 3.0 * 17.0_f64.sqrt(), 1e-15);
}

#[test]
fn fused_standardized_moment_block_cap_covers_tail_rows() {
    let rows = FUSED_MOMENT_MAX_BLOCKS * PARALLEL_REDUCTION_CHUNK_ROWS + 17;
    let mut values = vec![0.0_f64; rows * 2];
    for row in rows - 17..rows {
        values[row * 2] = 2.0;
        values[row * 2 + 1] = 3.0;
    }
    let matrix = DenseMatrix::from_row_major(rows, 2, values).expect("moment matrix");
    let mut reservation = SystemFitReservation;
    let (rhs, gram) = test_projected_moments(&matrix, &[0], &mut reservation)
        .expect("capped fused standardized moments");
    assert_eq!(rhs, vec![102.0]);
    assert_eq!(gram, vec![153.0]);
}

#[test]
fn shared_finite_accepts_known_wide_design() {
    for (rows, columns) in [(32_768_usize, 12_usize), (1_024, 65), (1_024, 128)] {
        let mut regressors = vec![vec![0.0_f64; rows]; columns];
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for row in 0..rows {
            for (column, values) in regressors.iter_mut().enumerate() {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let uniform = ((state >> 11) as f64) / ((1_u64 << 53) as f64) - 0.5;
                values[row] = uniform + 0.03 * ((row + 3 * column) as f64 * 0.17).sin();
            }
        }
        let outcome = (0..rows)
            .map(|row| {
                regressors
                    .iter()
                    .enumerate()
                    .fold(0.0_f64, |value, (column, regressor)| {
                        ((column + 1) as f64 / columns as f64).mul_add(regressor[row], value)
                    })
            })
            .collect::<Vec<_>>();
        let prepared = prepared(
            &outcome,
            &regressors,
            &vec![false; columns],
            None,
            false,
            TestWeights::Unit,
        );
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let fit = fit_prepared_model_coefficients(
                &prepared,
                PreparedOlsFitConfig {
                    execution: PreparedOlsExecutionConfig {
                        mode,
                        poolsize: 8,
                        ..PreparedOlsExecutionConfig::default()
                    },
                    ..PreparedOlsFitConfig::default()
                },
            )
            .expect("shared finite known wide fit");
            assert_eq!(fit.finite_rank(), columns);
            assert!(fit.rank_certificate().passed);
            assert!(fit.score_certificate().passed);
            for (column, coefficient) in fit.coefficients().iter().enumerate() {
                assert_close(
                    coefficient.value.expect("known wide coefficient"),
                    (column + 1) as f64 / columns as f64,
                    1e-10,
                );
            }
            assert!(fit.residuals().iter().all(|value| value.abs() < 1e-10));
        }
    }
}

#[test]
fn no_fe_intercept_and_syntax_omission_preserve_canonical_slots() {
    let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0];
    let omitted = vec![9.0, 8.0, 7.0, 6.0, 5.0, 4.0];
    let y = x.iter().map(|value| 3.0 + 2.5 * value).collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        &[x, omitted],
        &[false, true],
        None,
        true,
        TestWeights::Unit,
    );
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("certified fit");
    assert_eq!(fit.finite_rank(), 1);
    assert_eq!(fit.coefficients().len(), 3);
    assert_eq!(fit.coefficients()[0].status, PreparedOlsFitStatus::Kept);
    assert_close(fit.coefficients()[0].value.expect("slope"), 2.5, 1e-11);
    assert_eq!(
        fit.coefficients()[1].status,
        PreparedOlsFitStatus::SyntaxOmitted
    );
    assert_eq!(fit.coefficients()[1].generated_column_index, None);
    assert_eq!(
        fit.coefficients()[2].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_close(fit.coefficients()[2].value.expect("intercept"), 3.0, 1e-11);
    assert!(fit.residuals().iter().all(|value| value.abs() < 1e-11));
    assert!(fit.absorption_report().certified());
    assert!(fit.residual_absorption_report().certified());
    assert!(fit.rank_certificate().passed);
    assert!(fit.score_certificate().passed);
}

#[test]
fn prepared_lsmr_path_matches_certified_map_fit() {
    let group = vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
    let x = vec![-2.0, 1.0, -1.0, 2.0, 0.5, 3.0, -0.5, 4.0];
    let y = x
        .iter()
        .zip(&group)
        .map(|(x, group)| 1.75 * x + 2.0 * group - 0.5)
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&group),
        true,
        TestWeights::Unit,
    );
    let map = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("certified MAP fit");
    let lsmr = fit_prepared_model_coefficients(
        &prepared,
        PreparedOlsFitConfig {
            absorb: AbsorbConfig {
                solver: crate::fe::FeSolverKind::Lsmr,
                lsmr: crate::fe::LsmrConfig {
                    tolerance: 1e-10,
                    preconditioner: crate::fe::LsmrPreconditioner::Diagonal,
                    ..crate::fe::LsmrConfig::default()
                },
                ..PreparedOlsFitConfig::default().absorb
            },
            ..PreparedOlsFitConfig::default()
        },
    )
    .expect("certified LSMR fit");
    assert_eq!(
        lsmr.absorption_report().used_solver,
        crate::fe::FeSolverKind::Lsmr
    );
    assert_eq!(
        lsmr.residual_absorption_report().used_solver,
        crate::fe::FeSolverKind::Lsmr
    );
    for (map, lsmr) in map.coefficients().iter().zip(lsmr.coefficients()) {
        assert_eq!(map.status, lsmr.status);
        match (map.value, lsmr.value) {
            (Some(map), Some(lsmr)) => assert_close(lsmr, map, 1e-8),
            (None, None) => {}
            _ => panic!("MAP and LSMR coefficient availability differs"),
        }
    }
    for (map, lsmr) in map.residuals().iter().zip(lsmr.residuals()) {
        assert_close(*lsmr, *map, 1e-8);
    }
}

#[test]
fn reconstructed_within_residual_is_reused_and_refined_only_when_needed() {
    let group = (0..48).map(|row| (row / 6) as f64).collect::<Vec<_>>();
    let x = (0..48)
        .map(|row| (row % 11) as f64 - 5.0)
        .collect::<Vec<_>>();
    let y = x
        .iter()
        .zip(&group)
        .enumerate()
        .map(|(row, (x, group))| 1.75 * x + 0.6 * group + 0.03 * (row as f64).sin())
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&group),
        true,
        TestWeights::Unit,
    );

    let ordinary = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("ordinary reconstructed-residual fit");
    assert!(ordinary.residual_absorption_report().certified());
    assert_eq!(ordinary.residual_absorption_report().iterations, 0);

    let mut reservation = InjectedReservation::default();
    let refined = fit_with_controls_internal(
        &prepared,
        PreparedOlsFitConfig::default(),
        &mut reservation,
        FitPerturbations {
            reconstructed_within_residual_constant: true,
            ..FitPerturbations::default()
        },
    )
    .expect("perturbed reconstructed residual is refined");
    assert!(refined.residual_absorption_report().certified());
    assert!(refined.residual_absorption_report().iterations > 0);
    for (ordinary, refined) in ordinary.residuals().iter().zip(refined.residuals()) {
        assert_close(*refined, *ordinary, 1e-10);
    }
}

fn automatic_lsmr_report_for_merge() -> SolveReport {
    SolveReport {
        requested_solver: FeSolverKind::Auto,
        used_solver: FeSolverKind::Lsmr,
        iterations: 12,
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
        stop_reason: SolveStopReason::Certified,
        certificate: FeCertificate {
            absolute_infinity_norm: 2e-8,
            relative_infinity_norm: 3e-11,
            relative_two_norm: 4e-12,
            passed: true,
        },
        normal_equation_residual: 5e-11,
        setup_seconds: 1.0,
        solve_seconds: 2.0,
        certification_seconds: 0.25,
        fallback_used: false,
        requested_preconditioner: Some(crate::fe::LsmrPreconditioner::Auto),
        used_preconditioner: Some(crate::fe::LsmrPreconditioner::Schwarz),
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    }
}

#[test]
fn merged_solver_memory_preserves_largest_attempt_and_unmodeled_scope() {
    use crate::memory::SolverWorkspacePeak;
    let mut first = automatic_lsmr_report_for_merge();
    first.solver_workspace_peak = SolverWorkspacePeak::from_bytes(400).unwrap();
    let mut second = first.clone();
    second.solver_workspace_peak = SolverWorkspacePeak::from_bytes(200).unwrap();
    for (mut target, source) in [(first.clone(), second.clone()), (second, first)] {
        merge_solve_report(&mut target, source).unwrap();
        assert_eq!(target.solver_workspace_peak.bytes(), Some(400));
        merge_solve_report(&mut target, automatic_lsmr_report_for_merge()).unwrap();
        assert_eq!(target.solver_workspace_peak.bytes(), None);
    }
}

#[test]
fn low_report_merge_preserves_certified_lsmr_cg_continuation_in_any_order() {
    let lsmr = automatic_lsmr_report_for_merge();
    let mut cg = lsmr.clone();
    cg.used_solver = FeSolverKind::Cg;
    cg.fallback_used = true;
    cg.iterations = 17;
    cg.checkpoint_retries = 1;
    cg.certificate.absolute_infinity_norm = 9e-8;
    cg.certificate.relative_infinity_norm = 2e-10;
    cg.normal_equation_residual = 0.0;
    cg.setup_seconds = 3.0;
    cg.solve_seconds = 4.0;
    cg.certification_seconds = 0.5;
    cg.warnings
        .push("continued CG from the LSMR candidate".to_owned());
    let mut last = lsmr.clone();
    last.pilot_iterations = 2;
    last.pilot_contraction = 0.1;
    last.warnings.push("later LSMR batch".to_owned());
    let reports = [lsmr.clone(), cg.clone(), last];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut merged = reports[order[0]].clone();
        let expected_warnings = order
            .iter()
            .flat_map(|i| reports[*i].warnings.clone())
            .collect::<Vec<_>>();
        for i in &order[1..] {
            merge_solve_report(&mut merged, reports[*i].clone()).unwrap();
        }
        assert!(merged.certified());
        assert_eq!(merged.requested_solver, FeSolverKind::Auto);
        assert_eq!(merged.used_solver, FeSolverKind::Cg);
        assert!(merged.fallback_used);
        assert_eq!(
            merged.requested_preconditioner,
            lsmr.requested_preconditioner
        );
        assert_eq!(merged.used_preconditioner, lsmr.used_preconditioner);
        assert_eq!(merged.iterations, 17);
        assert_eq!(merged.checkpoint_retries, 1);
        assert_eq!(
            merged.certificate.absolute_infinity_norm,
            cg.certificate.absolute_infinity_norm
        );
        assert_eq!(
            merged.certificate.relative_infinity_norm,
            cg.certificate.relative_infinity_norm
        );
        assert_eq!(
            merged.certificate.relative_two_norm,
            lsmr.certificate.relative_two_norm
        );
        assert_eq!(
            merged.normal_equation_residual,
            lsmr.normal_equation_residual
        );
        assert_eq!(merged.setup_seconds, 5.0);
        assert_eq!(merged.solve_seconds, 8.0);
        assert_eq!(merged.certification_seconds, 1.0);
        assert_eq!(merged.pilot_iterations, 2);
        assert_eq!(merged.pilot_contraction, 0.1);
        assert_eq!(merged.warnings, expected_warnings);
    }
}

#[test]
fn low_report_merge_accepts_independent_auto_pilots_in_any_order() {
    let lsmr = automatic_lsmr_report_for_merge();
    let mut map = lsmr.clone();
    map.used_solver = FeSolverKind::Map;
    map.requested_preconditioner = None;
    map.used_preconditioner = None;
    map.iterations = 4;
    let mut direct_cg = map.clone();
    direct_cg.used_solver = FeSolverKind::Cg;
    direct_cg.iterations = 9;
    for fallback in [false, true] {
        let mut last = lsmr.clone();
        if fallback {
            last.used_solver = FeSolverKind::Cg;
            last.fallback_used = true;
        }
        let reports = [map.clone(), direct_cg.clone(), lsmr.clone(), last];
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let order = [a, b, c, d];
                        if (0..4).any(|i| (i + 1..4).any(|j| order[i] == order[j])) {
                            continue;
                        }
                        let mut merged = reports[a].clone();
                        for index in [b, c, d] {
                            merge_solve_report(&mut merged, reports[index].clone()).unwrap();
                        }
                        assert!(merged.certified());
                        assert_eq!(
                            merged.used_solver,
                            if fallback {
                                FeSolverKind::Cg
                            } else {
                                FeSolverKind::Lsmr
                            }
                        );
                        assert_eq!(merged.fallback_used, fallback);
                        assert_eq!(
                            merged.requested_preconditioner,
                            lsmr.requested_preconditioner
                        );
                        assert_eq!(merged.used_preconditioner, lsmr.used_preconditioner);
                        assert_eq!(merged.iterations, 12);
                        assert_eq!(merged.setup_seconds, 4.0);
                        assert_eq!(merged.solve_seconds, 8.0);
                    }
                }
            }
        }
    }
    // A direct-CG/early-MAP mixture did not attempt or fall back from LSMR.
    for (mut merged, source) in [(map.clone(), direct_cg.clone()), (direct_cg, map)] {
        merge_solve_report(&mut merged, source).unwrap();
        assert_eq!(merged.used_solver, FeSolverKind::Cg);
        assert!(!merged.fallback_used);
        assert_eq!(merged.used_preconditioner, None);
    }
}

#[test]
fn low_report_merge_preserves_general_map_fallback_in_any_order() {
    let lsmr = automatic_lsmr_report_for_merge();
    let mut fallback = lsmr.clone();
    fallback.used_solver = FeSolverKind::Map;
    fallback.fallback_used = true;
    fallback.iterations = 40;
    let mut direct = lsmr.clone();
    direct.used_solver = FeSolverKind::Map;
    direct.requested_preconditioner = None;
    direct.used_preconditioner = None;
    let reports = [lsmr, fallback, direct];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut merged = reports[order[0]].clone();
        for i in &order[1..] {
            merge_solve_report(&mut merged, reports[*i].clone()).unwrap();
        }
        assert!(merged.certified());
        assert_eq!(merged.used_solver, FeSolverKind::Map);
        assert!(merged.fallback_used);
        assert_eq!(merged.iterations, 40);
        assert_eq!(
            merged.requested_preconditioner,
            reports[0].requested_preconditioner
        );
        assert_eq!(merged.used_preconditioner, reports[0].used_preconditioner);
    }
}

#[test]
fn low_report_merge_rejects_uncertified_or_incompatible_continuations() {
    let lsmr = automatic_lsmr_report_for_merge();
    let mut cg = lsmr.clone();
    cg.used_solver = FeSolverKind::Cg;
    cg.fallback_used = true;
    for variant in 0..8 {
        let mut bad = cg.clone();
        match variant {
            0 => bad.certificate.passed = false,
            1 => bad.stop_reason = SolveStopReason::Uncertified,
            2 => bad.requested_solver = FeSolverKind::Lsmr,
            3 => bad.fallback_used = false,
            4 => bad.used_solver = FeSolverKind::Squarem,
            5 => bad.requested_preconditioner = Some(crate::fe::LsmrPreconditioner::Schwarz),
            6 => bad.used_preconditioner = Some(crate::fe::LsmrPreconditioner::Off),
            7 => bad.requested_preconditioner = None,
            _ => unreachable!(),
        }
        for (target, source) in [(lsmr.clone(), bad.clone()), (bad, lsmr.clone())] {
            let mut merged = target.clone();
            let error = merge_solve_report(&mut merged, source).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert_eq!(merged, target, "invalid input must not partially merge");
        }
    }
    let mut missing = lsmr;
    missing.requested_preconditioner = None;
    missing.used_preconditioner = None;
    cg.requested_preconditioner = None;
    cg.used_preconditioner = None;
    assert!(merge_solve_report(&mut missing, cg).is_err());
}

#[test]
fn low_lsmr_batches_match_fast_across_pool_sizes() {
    let rows = 2048;
    let effects = [97, 89, 9].map(|levels| {
        (0..rows)
            .map(|r| ((r * 37 + r / 11) % levels) as f64)
            .collect::<Vec<_>>()
    });
    let x = (0..4)
        .map(|j| {
            (0..rows)
                .map(|r| (r as f64 * (j + 3) as f64 * 0.137).sin())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let y = (0..rows)
        .map(|r| 0.7 * x[0][r] - 0.2 * x[1][r] + effects[0][r] * 0.01 + (r as f64 * 0.131).cos())
        .collect::<Vec<_>>();
    let prepared = prepared_multiway_categorical(&y, &x, &effects);
    let mut config = PreparedOlsFitConfig::default();
    config.execution.threads = ThreadConfig::from_requested(1, true).unwrap();
    let fast = fit_prepared_model_coefficients(&prepared, config).unwrap();
    config.execution.mode = PreparedOlsExecutionMode::Low;
    for poolsize in [1, 2, 4, 5] {
        config.execution.poolsize = poolsize;
        let low = fit_prepared_model_coefficients(&prepared, config).unwrap();
        assert_eq!(low.finite_rank(), fast.finite_rank());
        assert_eq!(low.absorption_report().used_solver, FeSolverKind::Lsmr);
        for (a, b) in low.residuals().iter().zip(fast.residuals()) {
            assert_close(*a, *b, 1e-10);
        }
        assert!(low.rank_certificate().passed);
    }
}

#[test]
fn low_fresh_blocks_preserve_explicit_weight_divisors_and_rank() {
    let rows = 257;
    let groups = (0..rows).map(|r| (r % 7) as f64).collect::<Vec<_>>();
    let weights = (0..rows).map(|r| (1 + r % 5) as f64).collect::<Vec<_>>();
    let x1 = (0..rows)
        .map(|r| 1e3 * groups[r] + (r as f64 * 0.37).sin())
        .collect::<Vec<_>>();
    let x2 = (0..rows)
        .map(|r| (r as f64 * 0.17).cos())
        .collect::<Vec<_>>();
    let x3 = x1.iter().map(|value| 2.0 * value).collect::<Vec<_>>();
    let y = (0..rows)
        .map(|r| 0.7 * x1[r] - 0.3 * x2[r] + groups[r] + (r as f64 * 0.11).sin())
        .collect::<Vec<_>>();
    for kind in [
        TestWeights::Unit,
        TestWeights::Analytic(&weights),
        TestWeights::Frequency(&weights),
        TestWeights::Probability(&weights),
    ] {
        let data = prepared(
            &y,
            &[x1.clone(), x2.clone(), x3.clone(), groups.clone()],
            &[false; 4],
            Some(&groups),
            true,
            kind,
        );
        let canonical = LinearSolveFrame::from_prepared_ols(&data).unwrap();
        let fast = fit_prepared_model_coefficients(&data, PreparedOlsFitConfig::default()).unwrap();
        assert_eq!(fast.finite_rank(), 2);
        for divisor_factor in [1.0, 8.0] {
            if canonical.weights().values.is_none() && divisor_factor != 1.0 {
                continue;
            }
            // Rescale the numerical solve without changing reference bread.
            let mut scale = canonical.bread_weight_scale();
            scale.solver_weight_divisor *= divisor_factor;
            scale.reference_weight_multiplier *= divisor_factor;
            let frame =
                LinearSolveFrame::from_parts(canonical.response(), canonical.raw_weights(), scale)
                    .unwrap();
            for (poolsize, threads) in [(1, 1), (3, 7), (5, 1)] {
                let config = PreparedOlsFitConfig {
                    execution: PreparedOlsExecutionConfig {
                        mode: PreparedOlsExecutionMode::Low,
                        poolsize,
                        threads: ThreadConfig::from_requested(threads, true).unwrap(),
                    },
                    ..PreparedOlsFitConfig::default()
                };
                let low = fit_prepared_weighted_linear_once(&data, frame, config).unwrap();
                assert_eq!(low.kept_generated_columns(), fast.kept_generated_columns());
                assert_eq!(
                    low.dropped_generated_columns(),
                    fast.dropped_generated_columns()
                );
                for (a, b) in low.coefficients().iter().zip(fast.coefficients()) {
                    assert_eq!(a.status, b.status);
                    match (a.value, b.value) {
                        (Some(a), Some(b)) => assert_close(a, b, 1e-9),
                        (a, b) => assert_eq!(a, b),
                    }
                }
                for (a, b) in low.bread().values().iter().zip(fast.bread().values()) {
                    assert_close(*a, *b, 1e-9);
                }
                for (a, b) in low.residuals().iter().zip(fast.residuals()) {
                    assert_close(*a, *b, 1e-9);
                }
                assert!(low.rank_certificate().passed);
                assert!(low.score_certificate().passed);
            }
        }
    }
}

#[test]
fn low_memory_scratch_fit_matches_fast_fit_across_multiple_blocks() {
    let rows = 40_usize;
    let g = (0..rows).map(|row| (row % 5) as f64).collect::<Vec<_>>();
    let x1 = (0..rows)
        .map(|row| row as f64 / 7.0 - 2.0)
        .collect::<Vec<_>>();
    let x2 = (0..rows)
        .map(|row| ((row * 3) % 17) as f64 / 5.0 - 1.0)
        .collect::<Vec<_>>();
    let x3 = (0..rows)
        .map(|row| ((row * row + 3) % 23) as f64 / 6.0)
        .collect::<Vec<_>>();
    let x4 = (0..rows)
        .map(|row| ((row * 11 + 5) % 29) as f64 / 8.0 - 1.5)
        .collect::<Vec<_>>();
    let y = (0..rows)
        .map(|row| {
            0.75 * x1[row] - 1.25 * x2[row]
                + 0.4 * x3[row]
                + 2.0 * x4[row]
                + [0.2, -0.4, 0.7, 1.1, -0.3][row % 5]
                + (row % 7) as f64 * 0.013
        })
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        &[x1, x2, x3, x4],
        &[false; 4],
        Some(&g),
        true,
        TestWeights::Unit,
    );
    let fast = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("fast fit");
    let low = fit_prepared_model_coefficients(
        &prepared,
        PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode: PreparedOlsExecutionMode::Low,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        },
    )
    .expect("low fit");

    assert_eq!(fast.finite_rank(), low.finite_rank());
    assert_eq!(fast.coefficients().len(), low.coefficients().len());
    for (fast, low) in fast.coefficients().iter().zip(low.coefficients()) {
        assert_eq!(fast.status, low.status);
        assert_eq!(fast.generated_column_index, low.generated_column_index);
        match (fast.value, low.value) {
            (Some(fast), Some(low)) => assert_close(low, fast, 5e-11),
            (None, None) => {}
            pair => panic!("coefficient presence differs: {pair:?}"),
        }
    }
    for (fast, low) in fast.residuals().iter().zip(low.residuals()) {
        assert_close(*low, *fast, 5e-11);
    }
    for (fast, low) in fast.bread().values().iter().zip(low.bread().values()) {
        assert_close(*low, *fast, 5e-10);
    }
    assert_eq!(fast.execution().mode, PreparedOlsExecutionMode::Fast);
    assert_eq!(fast.execution().scratch_bytes, 0);
    assert_eq!(low.execution().mode, PreparedOlsExecutionMode::Low);
    assert_eq!(low.execution().poolsize, 1);
    assert_eq!(low.execution().scratch_bytes, (rows * 5 * 8) as u64);
}

#[test]
fn absorbed_intercept_is_normalized_and_fe_collinear_column_is_separate() {
    let g = vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0];
    let x = vec![-1.0, 0.5, 2.0, -2.0, 1.0, 3.0, -0.5, 2.5, 4.0];
    let y = x
        .iter()
        .zip(&g)
        .map(|(x, group)| 1.75 * x + [0.0, 3.0, -2.0, 4.5][*group as usize])
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        &[x, g.clone()],
        &[false, false],
        Some(&g),
        true,
        TestWeights::Unit,
    );
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("FE fit");
    assert_eq!(fit.finite_rank(), 1);
    assert_close(
        fit.coefficients()[0].value.expect("identified slope"),
        1.75,
        1e-9,
    );
    assert_eq!(
        fit.coefficients()[1].status,
        PreparedOlsFitStatus::CollinearWithFixedEffects
    );
    assert_eq!(
        fit.coefficients()[2].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_close(
        fit.coefficients()[2].value.expect("normalized intercept"),
        11.0 / 6.0,
        1e-9,
    );
    assert_eq!(fit.kept_generated_columns(), &[0]);
    assert_eq!(fit.dropped_generated_columns(), &[1]);
    assert_eq!(fit.bread().dimension(), 2);
    assert_eq!(fit.bread().coordinates()[0].generated_column_index, 0);
    assert_eq!(fit.bread().coordinates()[1].generated_column_index, 2);
    assert_eq!(
        fit.bread().coordinates()[1].kind,
        PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
    );
    assert!(fit.bread().certificate().passed);
    assert!(fit.residuals().iter().all(|value| value.abs() < 2e-9));
}

#[test]
fn exact_scaled_duplicate_keeps_earlier_generated_column() {
    let g = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
    let x = vec![100.0, 101.0, 99.0, 100.5, -100.0, -99.0, -101.0, -100.5];
    let duplicate = x.iter().map(|value| 7.0 * value).collect::<Vec<_>>();
    let distractor = vec![-2.0, 1.0, 2.0, -1.0, 1.0, -2.0, -1.0, 2.0];
    let y = x
        .iter()
        .zip(&distractor)
        .zip(&g)
        .map(|((x, z), group)| 4.0 * x + 2.0 * z + 3.0 * group)
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        &[x, duplicate, distractor],
        &[false, false, false],
        Some(&g),
        false,
        TestWeights::Unit,
    );
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("rank-deficient fit");
    assert_eq!(fit.finite_rank(), 2);
    assert_eq!(fit.kept_generated_columns(), &[0, 2]);
    assert_eq!(fit.dropped_generated_columns(), &[1]);
    assert_eq!(
        fit.bread()
            .coordinates()
            .iter()
            .map(|coordinate| coordinate.generated_column_index)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert_close(
        fit.coefficients()[0].value.expect("first column"),
        4.0,
        1e-10,
    );
    assert_eq!(
        fit.coefficients()[1].status,
        PreparedOlsFitStatus::NumericallyRankDeficient
    );
    assert!(fit.rank_certificate().passed);
}

#[test]
fn pinned_full_ibn_span_keeps_cons_and_earlier_indicator() {
    // Pinned Stata 18/reghdfe contract for `ibn.foreign` with the default
    // `_cons`: retain the structural intercept and one indicator, rather
    // than allowing the preceding full indicator span to evict `_cons`.
    let foreign = source(1, "foreign");
    let foreign_values = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let outcome = vec![10.0, 10.0, 10.0, 13.0, 13.0, 13.0];
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![foreign.clone()],
            terms: vec![
                categorical_term(1, foreign.id, 0.0, false),
                categorical_term(2, foreign.id, 1.0, false),
            ],
        },
        absorbed_terms: Vec::new(),
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
                role: OlsInputRole::NumericSource(foreign.id),
            },
        ],
        include_intercept: true,
        keep_singletons: true,
    };
    let prepared = prepare_model(
        &request,
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(&outcome),
            numeric_sources: vec![(foreign.id, loaded(&foreign_values))],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        false,
    )
    .expect("full ibn preparation");
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("full ibn fit");
    assert_eq!(fit.finite_rank(), 1);
    assert_eq!(fit.kept_generated_columns(), &[0]);
    assert_eq!(fit.dropped_generated_columns(), &[1]);
    assert_eq!(fit.coefficients()[0].status, PreparedOlsFitStatus::Kept);
    assert_eq!(
        fit.coefficients()[1].status,
        PreparedOlsFitStatus::NumericallyRankDeficient
    );
    assert_eq!(
        fit.coefficients()[2].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_close(fit.coefficients()[0].value.expect("foreign=0"), -3.0, 1e-11);
    assert_close(fit.coefficients()[2].value.expect("intercept"), 13.0, 1e-11);
    assert!(fit.rank_certificate().passed);
    assert!(fit.score_certificate().passed);
}

#[test]
fn factor_interaction_omission_and_vanished_level_keep_exact_slot_semantics() {
    let g = source(1, "g");
    let h = source(2, "h");
    let g_values = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
    let h_values = vec![1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0];
    let outcome = (0..g_values.len())
        .map(|row| {
            1.0 + 2.0 * f64::from(g_values[row] == 2.0)
                + 3.0 * f64::from(g_values[row] == 2.0 && h_values[row] == 2.0)
        })
        .collect::<Vec<_>>();
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![g.clone(), h.clone()],
            terms: vec![
                categorical_term(1, g.id, 1.0, true),
                categorical_term(2, g.id, 2.0, false),
                categorical_interaction_term(3, &[(g.id, 2.0), (h.id, 2.0)]),
                categorical_term(4, g.id, 3.0, false),
            ],
        },
        absorbed_terms: Vec::new(),
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
                role: OlsInputRole::NumericSource(g.id),
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(h.id),
            },
        ],
        include_intercept: true,
        keep_singletons: false,
    };
    let prepared = prepare_model(
        &request,
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(&outcome),
            numeric_sources: vec![(g.id, loaded(&g_values)), (h.id, loaded(&h_values))],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        false,
    )
    .expect("factor preparation");
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("factor fit");
    assert_eq!(fit.coefficients().len(), 5);
    assert_eq!(
        fit.coefficients()[0].status,
        PreparedOlsFitStatus::SyntaxOmitted
    );
    assert_close(fit.coefficients()[1].value.expect("g=2"), 2.0, 1e-10);
    assert_close(fit.coefficients()[2].value.expect("g=2,h=2"), 3.0, 1e-10);
    assert_eq!(
        fit.coefficients()[3].status,
        PreparedOlsFitStatus::NumericallyRankDeficient
    );
    assert_close(fit.coefficients()[4].value.expect("intercept"), 1.0, 1e-10);
    assert_eq!(fit.finite_rank(), 2);
}

#[test]
fn rank_threshold_separates_clear_near_collinearity_cases() {
    let x = vec![-3.0, -2.0, -1.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    let z = vec![1.0, -1.0, 0.5, -0.5, 1.5, -1.5, 2.0, -2.0];
    let y = x
        .iter()
        .zip(&z)
        .map(|(x, z)| 2.0 * x + 0.3 * z)
        .collect::<Vec<_>>();
    let config = PreparedOlsFitConfig {
        rank_tolerance_multiplier: 1.0e8,
        ..PreparedOlsFitConfig::default()
    };
    let dropped_column = x
        .iter()
        .zip(&z)
        .map(|(x, z)| x + 1e-10 * z)
        .collect::<Vec<_>>();
    let dropped = prepared(
        &y,
        &[x.clone(), dropped_column],
        &[false, false],
        None,
        false,
        TestWeights::Unit,
    );
    let dropped =
        fit_prepared_model_coefficients(&dropped, config).expect("clearly below-threshold fit");
    assert_eq!(dropped.finite_rank(), 1);

    let kept_column = x
        .iter()
        .zip(&z)
        .map(|(x, z)| x + 1e-3 * z)
        .collect::<Vec<_>>();
    let kept = prepared(
        &y,
        &[x, kept_column],
        &[false, false],
        None,
        false,
        TestWeights::Unit,
    );
    let kept = fit_prepared_model_coefficients(&kept, config).expect("clearly above-threshold fit");
    assert_eq!(kept.finite_rank(), 2);
}

#[test]
fn all_weight_kinds_and_common_scaling_preserve_coefficients() {
    let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 5.0, 7.0];
    let y = vec![2.2, 0.9, 1.5, 4.4, 4.8, 8.2, 11.1, 15.8];
    let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
    let frequencies = vec![1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 3.0, 2.0];
    let mut coefficients = Vec::new();
    for case in [
        TestWeights::Analytic(&weights),
        TestWeights::Probability(&weights),
        TestWeights::Frequency(&frequencies),
    ] {
        let prepared = prepared(&y, std::slice::from_ref(&x), &[false], None, true, case);
        let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
            .expect("weighted fit");
        coefficients.push((
            fit.coefficients()[0].value.expect("slope"),
            fit.coefficients()[1].value.expect("intercept"),
        ));
    }
    assert_close(coefficients[0].0, coefficients[1].0, 1e-12);
    assert_close(coefficients[0].1, coefficients[1].1, 1e-12);

    for scale in [1e-200, 1e200] {
        let scaled = weights
            .iter()
            .map(|value| scale * value)
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            true,
            TestWeights::Analytic(&scaled),
        );
        let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
            .expect("scaled analytic fit");
        assert_close(
            fit.coefficients()[0].value.expect("scaled slope"),
            coefficients[0].0,
            2e-11,
        );
        assert_close(
            fit.coefficients()[1].value.expect("scaled intercept"),
            coefficients[0].1,
            2e-11,
        );
    }
}

#[test]
fn ols_adapter_and_borrowed_frame_are_exact_for_all_weight_kinds_and_modes() {
    let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
    let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
    let y = vec![1.2, 4.0, -0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
    let continuous_weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
    let frequency_weights = vec![1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 3.0, 2.0];
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for case in [
            TestWeights::Unit,
            TestWeights::Analytic(&continuous_weights),
            TestWeights::Probability(&continuous_weights),
            TestWeights::Frequency(&frequency_weights),
        ] {
            let prepared = prepared(&y, std::slice::from_ref(&x), &[false], Some(&g), true, case);
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 1,
                    ..PreparedOlsExecutionConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            };
            let adapter =
                fit_prepared_model_coefficients(&prepared, config).expect("OLS adapter fit");
            let derived =
                LinearSolveFrame::from_prepared_ols(&prepared).expect("derived OLS frame");
            let explicit = LinearSolveFrame::from_parts(
                derived.response(),
                derived.raw_weights(),
                derived.bread_weight_scale(),
            )
            .expect("explicit borrowed frame");
            let framed = fit_prepared_weighted_linear_once(&prepared, explicit, config)
                .expect("explicit-frame fit");
            assert_eq!(
                without_solve_timings(framed),
                without_solve_timings(adapter)
            );
        }
    }
}

#[test]
fn categorical_one_fe_initial_projection_preserves_warm_and_retained_paths() {
    let rows = 16_384;
    let x = source(1, "x");
    let g = source(2, "g");
    let h = source(3, "h");
    let outcome = (0..rows).map(|r| (r % 53) as f64).collect::<Vec<_>>();
    let g_values = (0..rows).map(|r| (r % 97) as f64).collect::<Vec<_>>();
    let h_values = (0..rows).map(|r| ((r / 97) % 3) as f64).collect::<Vec<_>>();
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![x.clone(), g.clone(), h.clone()],
            terms: vec![continuous_term(1, x.id, false)],
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept: true,
            categorical_sources: vec![g.id, h.id],
            slope_sources: Vec::new(),
            label: "g#h".to_owned(),
            saved_name: String::new(),
        }],
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
    };
    let prepared = crate::prepare_model(
        &request,
        &[],
        &LoadedOlsInput {
            candidate: vec![true; rows].into(),
            outcome: loaded(&outcome),
            numeric_sources: vec![
                (x.id, loaded(&outcome)),
                (g.id, loaded(&g_values)),
                (h.id, loaded(&h_values)),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        true,
    )
    .unwrap();
    let config = AbsorbConfig::default();
    assert_eq!(config.solver, FeSolverKind::Auto);
    for width in [1, 17, 65] {
        let columns = (0..width)
            .map(|c| {
                (0..rows)
                    .map(|r| {
                        let between = if c == width - 1 { 1e6 } else { 3.0 };
                        between * (g_values[r] + h_values[r]) + ((r * (c + 7) % 997) as f64).sin()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let raw = DenseMatrix::from_columns(&columns).unwrap();
        for weight_case in 0..4 {
            let supplied = (0..rows)
                .map(|r| match weight_case {
                    1 => 128.0,
                    2 => (1 + r % 7) as f64,
                    _ => {
                        if r % 5 == 0 {
                            1_048_576.0
                        } else {
                            1.0 / 1_048_576.0
                        }
                    }
                })
                .collect::<Vec<_>>();
            let weights = if weight_case == 0 {
                BorrowedMapWeights::Unit
            } else {
                BorrowedMapWeights::Provided(&supplied)
            };
            let mut across_threads = None;
            for threads in [1, 7] {
                crate::parallel::ThreadTeam::new(
                    ThreadConfig::from_requested(threads, true).unwrap(),
                )
                .install(|| {
                    let direct = prepared
                        .fit_absorb_matrix(weights, raw.clone(), config)
                        .unwrap();
                    assert_eq!(direct.report().iterations, 1);
                    assert!(!direct.report().fallback_used);
                    let residuals = direct.residualized();
                    // Independent anchored group means on the original two-key frame.
                    for c in 0..width {
                        for group in 0..291 {
                            let group_rows = (group..rows).step_by(291);
                            let anchor = columns[c][group];
                            let mut sum = 0.0;
                            let mut mass = 0.0;
                            for r in group_rows.clone() {
                                let w = if weight_case == 0 { 1.0 } else { supplied[r] };
                                sum += (columns[c][r] - anchor) * w;
                                mass += w;
                            }
                            for r in group_rows {
                                let expected = (columns[c][r] - anchor) - sum / mass;
                                assert!(
                                    (residuals[(r, c)] - expected).abs() < 4e-8,
                                    "width={width} weights={weight_case} row={r} column={c}"
                                );
                            }
                        }
                    }
                    if let Some(previous) = &across_threads {
                        assert_eq!(residuals, previous);
                    } else {
                        across_threads = Some(residuals.clone());
                    }
                    let warm = prepared
                        .fit_absorb_matrix(weights, residuals.clone(), config)
                        .unwrap();
                    assert_eq!(warm.report().iterations, 0);
                    assert_eq!(warm.residualized(), residuals);
                    for (solver, retain) in [(FeSolverKind::Auto, true), (FeSolverKind::Map, false)]
                    {
                        let mut unchanged = config;
                        unchanged.solver = solver;
                        unchanged.retain_coefficients = retain;
                        let initial = prepared
                            .fit_absorb_matrix(weights, raw.clone(), unchanged)
                            .unwrap();
                        let ordinary = prepared
                            .fit_absorb_matrix(weights, raw.clone(), unchanged)
                            .unwrap();
                        assert_eq!(initial.residualized(), ordinary.residualized());
                        assert_eq!(initial.report().iterations, ordinary.report().iterations);
                    }
                })
                .unwrap();
            }
        }
    }
}

#[test]
fn categorical_ols_adapter_and_borrowed_frame_are_exact() {
    let x = source(1, "x");
    let g = source(2, "g");
    let outcome = vec![1.0, 3.5, 2.0, 7.0, 4.5, 10.0, 8.0, 13.0];
    let x_values = vec![-1.0, 0.5, -2.0, 2.0, -0.5, 3.0, 1.0, 4.0];
    let g_values = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![x.clone(), g.clone()],
            terms: vec![continuous_term(1, x.id, false)],
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("absorbed term"),
            intercept: true,
            categorical_sources: vec![g.id],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: String::new(),
        }],
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
        ],
        include_intercept: true,
        keep_singletons: true,
    };
    let prepared = crate::prepare_model(
        &request,
        &[],
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(&outcome),
            numeric_sources: vec![(x.id, loaded(&x_values)), (g.id, loaded(&g_values))],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        true,
    )
    .expect("categorical prepared data");
    let config = PreparedOlsFitConfig::default();
    let adapter =
        fit_prepared_model_coefficients(&prepared, config).expect("categorical OLS adapter fit");
    let derived = LinearSolveFrame::from_prepared_ols(&prepared).expect("categorical OLS frame");
    let explicit = LinearSolveFrame::from_parts(
        derived.response(),
        derived.raw_weights(),
        derived.bread_weight_scale(),
    )
    .expect("explicit categorical frame");
    let framed = fit_prepared_weighted_linear_once(&prepared, explicit, config)
        .expect("categorical explicit-frame fit");
    assert_eq!(
        without_solve_timings(framed),
        without_solve_timings(adapter)
    );
}

#[test]
fn initial_full_width_lsmr_preserves_canonical_multiway_rank_policy() {
    let rows = 384_usize;
    let effects = [
        (0..rows)
            .map(|row| (row % 17 + 1) as f64)
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((7 * row + row / 17) % 13 + 1) as f64)
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((11 * row + row / 13) % 9 + 1) as f64)
            .collect::<Vec<_>>(),
    ];
    let axis = (0..rows)
        .map(|row| row as f64 - (rows - 1) as f64 / 2.0)
        .collect::<Vec<_>>();
    let independent = (0..8_usize)
        .map(|column| {
            axis.iter()
                .enumerate()
                .map(|(row, value)| {
                    (value * (column + 3) as f64 / 97.0).sin()
                        + ((row * (column + 5) % 23) as f64 - 11.0) / 29.0
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let perturbation = axis
        .iter()
        .enumerate()
        .map(|(row, value)| (value / 31.0).cos() + (row % 7) as f64 / 19.0)
        .collect::<Vec<_>>();
    let outcome = (0..rows)
        .map(|row| {
            0.4 * independent[0][row] - 0.25 * independent[1][row]
                + 0.15 * independent[2][row]
                + 0.03 * effects[0][row]
                - 0.02 * effects[1][row]
                + (row % 5) as f64 / 41.0
        })
        .collect::<Vec<_>>();

    let mut rank_edges = independent.clone();
    rank_edges.push(effects[0].clone());
    rank_edges.push(independent[0].clone());
    rank_edges.push(
        independent[1]
            .iter()
            .zip(&perturbation)
            .map(|(value, perturbation)| value + 1e-13 * perturbation)
            .collect(),
    );
    rank_edges.push(
        independent[2]
            .iter()
            .zip(&perturbation)
            .map(|(value, perturbation)| value + 1e-5 * perturbation)
            .collect(),
    );
    let mut reordered = rank_edges.clone();
    reordered.reverse();
    let wide = (1..=50_usize)
        .map(|mask| {
            (0..rows)
                .map(|row| {
                    if (row & mask).count_ones() % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut certified_schur_cases = 0_usize;
    for effect_count in [2_usize, 3] {
        for threads in [1_usize, 7] {
            for regressors in [
                Vec::new(),
                rank_edges.clone(),
                reordered.clone(),
                wide.clone(),
            ] {
                let prepared =
                    prepared_multiway_categorical(&outcome, &regressors, &effects[..effect_count]);
                certified_schur_cases += usize::from(
                    assert_initial_checkpoint_lsmr_matches_map(&prepared, threads)
                        == FeSolverKind::Map,
                );
            }
        }
    }
    assert!(
        certified_schur_cases > 0,
        "the rank matrix must exercise a certified Schur checkpoint"
    );
}

#[test]
fn initial_checkpoint_lsmr_falls_back_only_after_numerical_noncertification() {
    let rows = 192_usize;
    let effects = vec![
        (0..rows)
            .map(|row| (row % 17 + 1) as f64)
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((5 * row + row / 17) % 13 + 1) as f64)
            .collect::<Vec<_>>(),
    ];
    let regressors = vec![
        (0..rows)
            .map(|row| (row as f64 / 17.0).sin())
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| (row as f64 / 23.0).cos())
            .collect::<Vec<_>>(),
    ];
    let outcome = (0..rows)
        .map(|row| {
            0.5 * regressors[0][row] - 0.2 * regressors[1][row]
                + 0.03 * effects[0][row]
                + (row % 7) as f64 / 31.0
        })
        .collect::<Vec<_>>();
    let prepared = prepared_multiway_categorical(&outcome, &regressors, &effects);
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Map;
    config.absorb.map.tolerance = 1e-6;
    config.absorb.lsmr.tolerance = 1e-6;
    let frame = LinearSolveFrame::from_prepared_ols(&prepared).expect("initial frame");
    let generations = LinearSessionGenerations::new(1, 1, 1, 1, 1);

    let mut expected_session = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        frame,
        config,
        generations,
        prepared.fit_owned_fe_design().expect("MAP FE design"),
    )
    .expect("MAP initial session");
    let expected = expected_session.solve_fresh().expect("MAP initial solve");

    let mut fallback_session = CertifiedLinearSession::new_with_fe_design(
        &prepared,
        frame,
        config,
        generations,
        prepared.fit_owned_fe_design().expect("fallback FE design"),
    )
    .expect("fallback initial session");
    fallback_session.enable_checkpoint_lsmr(true);
    fallback_session.inject_checkpoint_lsmr_certificate_failure();
    let fallback = fallback_session
        .solve_fresh()
        .expect("independently certified MAP fallback");
    assert_eq!(
        fallback.frame().absorption_report().requested_solver,
        FeSolverKind::Lsmr
    );
    assert_eq!(
        fallback.frame().absorption_report().used_solver,
        FeSolverKind::Map,
        "design={:?}, residual={:?}",
        fallback.frame().absorption_report(),
        fallback.frame().residual_absorption_report(),
    );
    assert!(fallback.frame().absorption_report().fallback_used);
    assert_eq!(
        fallback.frame().finite_rank(),
        expected.frame().finite_rank()
    );
    assert_eq!(
        fallback.basis().kept_generated_columns(),
        expected.basis().kept_generated_columns()
    );
    assert_eq!(
        fallback.basis().dropped_generated_columns(),
        expected.basis().dropped_generated_columns()
    );
    for (fallback, expected) in fallback.coefficients().zip(expected.coefficients()) {
        assert_eq!(fallback.status, expected.status);
        match (fallback.value, expected.value) {
            (Some(fallback), Some(expected)) => assert_close(fallback, expected, 2e-12),
            (None, None) => {}
            _ => panic!("fallback coefficient presence differs"),
        }
    }

    for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
        let mut session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().expect("fatal FE design"),
        )
        .expect("fatal initial session");
        session.enable_checkpoint_lsmr(true);
        session.inject_checkpoint_lsmr_fatal_failure(code);
        let error = match session.solve_fresh() {
            Ok(_) => panic!("fatal LSMR failure must not fall back"),
            Err(error) => error,
        };
        assert_eq!(error.code, code);
        assert!(session.output().is_err());
        let retry = session
            .solve_fresh()
            .expect("fatal injection must leave the session reusable");
        assert_eq!(
            retry.frame().absorption_report().requested_solver,
            FeSolverKind::Lsmr
        );
        // With the fault consumed, the admitted two-FE Schur accelerator
        // can again certify the requested checkpoint before LSMR runs.
        assert_eq!(
            retry.frame().absorption_report().used_solver,
            FeSolverKind::Map
        );
        assert!(!retry.frame().absorption_report().fallback_used);
        assert!(retry.certified());
    }

    for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
        let mut session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().expect("rank FE design"),
        )
        .expect("rank initial session");
        session.enable_checkpoint_lsmr(true);
        session.inject_fresh_rank_failure(code);
        let error = match session.solve_fresh() {
            Ok(_) => panic!("canonical rank failure must remain fatal"),
            Err(error) => error,
        };
        assert_eq!(error.code, code);
        assert!(session.output().is_err());
        let retry = session
            .solve_fresh()
            .expect("rank failure must leave the private session reusable");
        assert_eq!(retry.frame().finite_rank(), expected.frame().finite_rank());
        assert!(retry.certified());
    }
}

#[test]
fn categorical_recovery_uses_explicit_final_hessian_weights_for_normalization() {
    let worker = source(1, "worker");
    let firm = source(2, "firm");
    let worker_codes = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
    let firm_codes = vec![1.0, 2.0, 1.0, 2.0, 1.0, 2.0];
    let worker_effect = [-1.0, -1.0, 0.25, 0.25, 0.75, 0.75];
    let firm_effect = [-0.5, 0.5, -0.5, 0.5, -0.5, 0.5];
    let target = worker_effect
        .iter()
        .zip(firm_effect)
        .map(|(left, right)| left + right)
        .collect::<Vec<_>>();
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![worker.clone(), firm.clone()],
            terms: Vec::new(),
        },
        absorbed_terms: vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("worker absorb"),
                intercept: true,
                categorical_sources: vec![worker.id],
                slope_sources: Vec::new(),
                label: "worker".to_owned(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).expect("firm absorb"),
                intercept: true,
                categorical_sources: vec![firm.id],
                slope_sources: Vec::new(),
                label: "firm".to_owned(),
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
                role: OlsInputRole::NumericSource(worker.id),
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(firm.id),
            },
        ],
        include_intercept: true,
        keep_singletons: true,
    };
    let prepared = crate::prepare_model(
        &request,
        &[],
        &LoadedOlsInput {
            candidate: vec![true; target.len()].into(),
            outcome: loaded(&target),
            numeric_sources: vec![
                (worker.id, loaded(&worker_codes)),
                (firm.id, loaded(&firm_codes)),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        true,
    )
    .expect("categorical recovery preparation");
    let requested = [true, true];
    let map = PreparedOlsFitConfig::default().absorb.map;
    let effects = prepared.effect_views().expect("effect views");
    let unit = recover_finalized_categorical_effects_map_certified(
        prepared.fe_topology(),
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &requested,
        categorical_slope_map_config(map),
    )
    .expect("unit-weight recovery");
    // A nonconstant mean changes the final PPML Hessian from q to q*mu.
    // The saved FE decomposition must therefore be normalized on this
    // explicit final curvature measure, even though its absorbed sum is
    // unchanged.
    let final_hessian = [1.0, 3.0, 2.0, 9.0, 4.0, 27.0];
    let hessian = recover_finalized_categorical_effects_map_certified(
        prepared.fe_topology(),
        &effects,
        BorrowedMapWeights::Provided(&final_hessian),
        &target,
        &requested,
        categorical_slope_map_config(map),
    )
    .expect("final-Hessian recovery");
    for recovered in [&unit, &hessian] {
        for (actual, expected) in recovered.absorbed_sum().iter().zip(&target) {
            assert_close(*actual, *expected, 2e-10);
        }
    }
    let unit_firm = unit.channels()[1].as_ref().expect("unit firm channel");
    let hessian_firm = hessian.channels()[1]
        .as_ref()
        .expect("Hessian firm channel");
    let unit_mean = unit_firm.iter().sum::<f64>() / unit_firm.len() as f64;
    assert_close(unit_mean, 0.0, 2e-10);
    let hessian_sum = final_hessian.iter().sum::<f64>();
    let hessian_mean = hessian_firm
        .iter()
        .zip(final_hessian)
        .map(|(value, weight)| value * weight)
        .sum::<f64>()
        / hessian_sum;
    assert_close(hessian_mean, 0.0, 2e-10);
    assert!(
        unit_firm
            .iter()
            .zip(hessian_firm)
            .any(|(left, right)| (left - right).abs() > 1e-3),
        "nonconstant q*mu must change the normalized saved-FE channels"
    );
}

#[test]
fn empty_categorical_topology_preserves_finite_intercept_across_session_routes() {
    let x = source(1, "x");
    let y = vec![1.0, 2.5, 2.0, 5.0, 4.5, 8.0, 7.0, 11.5];
    let refreshed_y = vec![1.4, 2.1, 2.7, 4.6, 5.2, 7.4, 7.8, 10.9];
    let x_values = vec![-2.0, -1.0, -0.5, 0.5, 1.0, 2.0, 3.0, 4.0];
    let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
    let refreshed_weights = vec![2.0, 0.75, 1.5, 0.6, 3.5, 1.25, 2.2, 1.8];
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![x.clone()],
            terms: vec![continuous_term(1, x.id, false)],
        },
        absorbed_terms: Vec::new(),
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
                role: OlsInputRole::AnalyticWeight,
            },
        ],
        include_intercept: true,
        keep_singletons: true,
    };
    let loaded_with = |outcome: &[f64], analytic_weight: &[f64]| LoadedOlsInput {
        candidate: vec![true; outcome.len()].into(),
        outcome: loaded(outcome),
        numeric_sources: vec![(x.id, loaded(&x_values))],
        analytic_weight: Some(loaded(analytic_weight)),
        frequency_weight: None,
        probability_weight: None,
    };
    let prepared =
        crate::ols::prepared_categorical_data::prepare_ppml_categorical_ols_data_with_load_hint(
            &request,
            &loaded_with(&y, &weights),
            &[],
            None,
            0,
        )
        .expect("empty categorical topology");
    let refreshed_prepared =
        crate::ols::prepared_categorical_data::prepare_ppml_categorical_ols_data_with_load_hint(
            &request,
            &loaded_with(&refreshed_y, &refreshed_weights),
            &[],
            None,
            0,
        )
        .expect("refreshed empty categorical topology");
    assert!(!prepared.has_absorbed_intercept());
    assert!(prepared.effect_specs().is_empty());

    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            absorb: AbsorbConfig {
                solver: FeSolverKind::Map,
                ..AbsorbConfig::default()
            },
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let initial_frame = LinearSolveFrame::from_prepared_ols(&prepared).expect("initial frame");
        let mut session = CertifiedLinearSession::new(
            &prepared,
            initial_frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .expect("empty-topology session");
        ACTIVE_LINEAR_ALLOCATION_PHASE.store(0, Ordering::SeqCst);
        LINEAR_PROJECTION_ENDS.store(0, Ordering::SeqCst);
        LINEAR_FINITE_ENDS.store(0, Ordering::SeqCst);
        session.set_phase_allocator_probe(Some(observed_linear_allocation_phase));
        let fresh = session.solve_fresh().expect("empty-topology fresh solve");
        assert_eq!(
            fresh.reuse_report().phase_allocation.projection_peak_bytes,
            Some(11)
        );
        assert_eq!(
            fresh
                .reuse_report()
                .phase_allocation
                .finite_solve_peak_bytes,
            Some(23)
        );
        assert_eq!(LINEAR_PROJECTION_ENDS.load(Ordering::SeqCst), 1);
        assert_eq!(LINEAR_FINITE_ENDS.load(Ordering::SeqCst), 1);
        assert_eq!(
            fresh.frame().finite_rank(),
            2,
            "retained coefficients: {:?}",
            fresh.coefficients().collect::<Vec<_>>()
        );
        assert!(fresh.coefficients().any(|coefficient| {
            coefficient.status == PreparedOlsFitStatus::Kept
                && coefficient.generated_column_index.is_some_and(|generated| {
                    matches!(
                        prepared.model().columns[generated].kind,
                        PreparedOlsColumnKind::Intercept
                    )
                })
        }));
        let refreshed_frame =
            LinearSolveFrame::from_prepared_ols(&refreshed_prepared).expect("refreshed frame");
        session
            .refresh_response(&refreshed_y, 1)
            .expect("response refresh");
        session
            .refresh_weights(
                refreshed_frame.raw_weights(),
                refreshed_frame.bread_weight_scale(),
                1,
            )
            .expect("weight refresh");
        let expected = fit_prepared_weighted_linear_once(
            &prepared,
            LinearSolveFrame::from_parts(
                &refreshed_y,
                refreshed_frame.raw_weights(),
                refreshed_frame.bread_weight_scale(),
            )
            .expect("canonical refreshed frame"),
            config,
        )
        .expect("canonical refreshed fit");
        let frozen = session.solve_frozen().expect("empty-topology frozen solve");
        assert_eq!(
            frozen.reuse_report().phase_allocation.projection_peak_bytes,
            Some(11)
        );
        assert_eq!(
            frozen
                .reuse_report()
                .phase_allocation
                .finite_solve_peak_bytes,
            Some(23)
        );
        assert_eq!(LINEAR_PROJECTION_ENDS.load(Ordering::SeqCst), 2);
        assert_eq!(LINEAR_FINITE_ENDS.load(Ordering::SeqCst), 2);
        assert_frozen_matches_fit(&frozen, &expected);
        assert_eq!(frozen.basis().kept_generated_columns().len(), 2);
        let strict = session.strictify().expect("empty-topology strict solve");
        assert_eq!(
            strict.reuse_report().phase_allocation.projection_peak_bytes,
            Some(11)
        );
        assert_eq!(
            strict
                .reuse_report()
                .phase_allocation
                .finite_solve_peak_bytes,
            Some(23)
        );
        assert_eq!(LINEAR_PROJECTION_ENDS.load(Ordering::SeqCst), 3);
        assert_eq!(LINEAR_FINITE_ENDS.load(Ordering::SeqCst), 3);
        assert_eq!(ACTIVE_LINEAR_ALLOCATION_PHASE.load(Ordering::SeqCst), 0);
        assert_fit_science_equal(&strict, &expected);
        assert_eq!(strict.frame().finite_rank(), 2);
    }
}

#[test]
fn ppml_slope_only_topology_preserves_structural_constant() {
    let group = source(1, "group");
    let slope = source(2, "slope");
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![group.clone(), slope.clone()],
            terms: Vec::new(),
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("absorb term"),
            intercept: false,
            categorical_sources: vec![group.id],
            slope_sources: vec![slope.id],
            label: "group#c.slope".to_owned(),
            saved_name: "saved_slope".to_owned(),
        }],
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
                role: OlsInputRole::NumericSource(group.id),
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(slope.id),
            },
        ],
        include_intercept: true,
        keep_singletons: true,
    };
    let loaded_input = LoadedOlsInput {
        candidate: vec![true; 6].into(),
        outcome: loaded(&[1.0, 2.0, 1.5, 3.0, 2.5, 4.0]),
        numeric_sources: vec![
            (group.id, loaded(&[1.0, 1.0, 1.0, 2.0, 2.0, 2.0])),
            (slope.id, loaded(&[-1.0, 0.0, 1.0, -1.0, 0.0, 1.0])),
        ],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };

    let prepared =
        crate::ols::prepared_categorical_data::prepare_ppml_categorical_ols_data_with_load_hint(
            &request,
            &loaded_input,
            &[],
            None,
            0,
        )
        .expect("PPML slope-only topology");
    assert!(prepared.has_absorbed_slope());
    assert!(!prepared.has_absorbed_intercept());
    assert!(prepared.structural_intercept());
    assert!(prepared.report_intercept());
    let frame = LinearSolveFrame::from_prepared_ols(&prepared).expect("slope-only frame");
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 2,
                threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
            },
            ..PreparedOlsFitConfig::default()
        };
        let expected = fit_prepared_weighted_linear_once(&prepared, frame, config)
            .expect("slope-only one-shot fit");
        let mut session = CertifiedLinearSession::new(
            &prepared,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .expect("slope-only retained session");
        let fresh = session.solve_fresh().expect("slope-only fresh");
        assert_fit_science_equal(&fresh, &expected);
        assert_fit_science_equal(&session.strictify().expect("slope-only strict"), &expected);
    }
}

#[path = "../../../../fereg-testkit/src/rounded_square_norm_reference.rs"]
mod handoff_norm_reference;

#[test]
fn projection_norm_handoff_preserves_fixed_leaf_scope_and_explicit_divisor_fallback() {
    for rows in [511, 1031] {
        let x: Vec<_> = (0..rows).map(|r| (r as f64 / 19.).sin()).collect();
        let y: Vec<_> = (0..rows)
            .map(|r| 1. + 0.3 * x[r] + (r as f64 / 13.).cos())
            .collect();
        let groups: Vec<_> = (0..rows).map(|r| (r % 17) as f64).collect();
        let firms: Vec<_> = (0..rows).map(|r| ((r / 17) % 13) as f64).collect();
        let weights: Vec<_> = (0..rows).map(|r| [0.001, 0.5, 1., 1000.][r % 4]).collect();
        let data = prepared_multiway_categorical_weights(
            &y,
            &[x],
            &[groups, firms],
            TestWeights::Analytic(&weights),
        );
        let canonical = LinearSolveFrame::from_prepared_ols(&data).unwrap();
        for workers in [1, 4, 16] {
            let mut config = PreparedOlsFitConfig::default();
            config.absorb.solver = FeSolverKind::Map;
            config.execution.threads =
                crate::options::ThreadConfig::from_requested(workers, true).unwrap();
            ThreadTeam::new(config.execution.threads)
            .install(|| {
                for divisor_factor in [1., 8.] {
                    let mut scale = canonical.bread_weight_scale();
                    scale.solver_weight_divisor *= divisor_factor;
                    scale.reference_weight_multiplier *= divisor_factor;
                    let frame =
                        LinearSolveFrame::from_parts(&y, canonical.raw_weights(), scale).unwrap();
                    assert_eq!(
                        frame.weights().matches_fe_norm_scale(),
                        divisor_factor == 1.
                    );
                    let mut session =
                        crate::fe::FeSession::for_ols(&data, frame.raw_weights(), config).unwrap();
                    let raw = data.fit_build_rhs_with_response(&y).unwrap();
                    let mut projected = session
                        .project_ols(frame.raw_weights(), raw, config.absorb, false, false)
                        .unwrap();
                    let norms = projected.take_residual_norms();
                    assert_eq!(norms.len(), projected.residualized().ncols());
                    let mut repeated_session =
                        crate::fe::FeSession::for_ols(&data, frame.raw_weights(), config).unwrap();
                    let mut repeated = repeated_session.project_ols(
                        frame.raw_weights(), data.fit_build_rhs_with_response(&y).unwrap(),
                        config.absorb, false, false,
                    ).unwrap();
                    let repeated_norms = repeated.take_residual_norms();
                    assert_eq!(norms.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        repeated_norms.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
                    assert_eq!(projected.residualized().as_slice().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        repeated.residualized().as_slice().iter().map(|v| v.to_bits()).collect::<Vec<_>>());
                    if divisor_factor == 1. {
                        for (column, norm) in norms.iter().enumerate() {
                            if rows <= 512 {
                                // A single E1 leaf retains the legacy recurrence exactly.
                                let recomputed = weighted_matrix_column_norm(
                                    projected.residualized(), column, frame.weights(),
                                ).unwrap();
                                assert_eq!(norm.to_bits(), recomputed.to_bits(),
                                    "rows={rows}, workers={workers}, column={column}");
                            }
                            // Above one leaf, E's declared reassociation supersedes
                            // legacy recurrence identity, not numerical accuracy.
                            // The reference squares the rounded weighted elements
                            // into twofold products without the production recurrence.
                            let accurate = handoff_norm_reference::rounded_element_norm(
                                (0..rows).map(|row| frame.weights().root(row).unwrap()
                                    * projected.residualized()[(row, column)]));
                            if accurate == 0.0 {
                                assert_eq!(norm.to_bits(), 0.0_f64.to_bits());
                            } else {
                                assert!((norm / accurate - 1.0).abs() <= 64.0 * f64::EPSILON,
                                    "rows={rows}, workers={workers}, column={column}: {norm} != {accurate}");
                            }
                        }
                    } else {
                        assert!(norms.iter().enumerate().any(|(column, norm)| {
                            norm.to_bits()
                                != weighted_matrix_column_norm(
                                    projected.residualized(),
                                    column,
                                    frame.weights(),
                                )
                                .unwrap()
                                .to_bits()
                        }));
                    }
                    let mut reservation = InjectedReservation::default();
                    let projection =
                        fit_projection_only_internal(&data, frame, config, &mut reservation)
                            .unwrap();
                    assert!(projection.certified());
                    assert_eq!(
                        reservation.records.iter().any(|(_, label)| *label
                            == "unable to reserve prepared projection within norms"),
                        divisor_factor != 1.
                    );
                }
            })
            .unwrap();
        }
    }
}

#[test]
fn projection_only_matches_full_fit_without_constructing_reported_bread() {
    let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0, 5.0, 7.0];
    let y = vec![1.2, 0.9, 1.5, 4.4, 4.8, 8.2, 11.1, 15.8];
    let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
    let prepared_data = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        None,
        true,
        TestWeights::Analytic(&weights),
    );
    let config = PreparedOlsFitConfig::default();
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
    let full = fit_prepared_weighted_linear_once(&prepared_data, frame, config).expect("full fit");
    let projection = fit_prepared_weighted_linear_projection_once(&prepared_data, frame, config)
        .expect("projection-only fit");
    assert!(projection.certified());
    assert_eq!(projection.finite_rank(), full.finite_rank());
    assert_eq!(
        projection.kept_generated_columns(),
        full.kept_generated_columns()
    );
    assert_eq!(
        projection.dropped_generated_columns(),
        full.dropped_generated_columns()
    );
    assert_eq!(projection.coefficients().len(), full.coefficients().len());
    for (projected, reported) in projection.coefficients().iter().zip(full.coefficients()) {
        if reported.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept {
            assert_eq!(
                projected.status,
                PreparedOlsFitStatus::AbsorbedInterceptDeferred
            );
            assert_eq!(projected.value, None);
            assert_eq!(projected.coefficient_slot, reported.coefficient_slot);
            assert_eq!(
                projected.generated_column_index,
                reported.generated_column_index
            );
        } else {
            assert_eq!(projected, reported);
        }
    }
    assert_eq!(projection.rank_certificate(), full.rank_certificate());
    assert_eq!(projection.score_certificate(), full.score_certificate());
    for (actual, expected) in projection.residuals().iter().zip(full.residuals()) {
        assert_close(*actual, *expected, 2e-12);
    }
    for ((fitted, residual), response) in projection
        .fitted()
        .iter()
        .zip(projection.residuals())
        .zip(&y)
    {
        assert_close(*fitted + *residual, *response, 2e-15);
    }
    assert!(projection.reconstruction_certificate().passed);
    assert!(projection.absorption_report().certified());
    assert!(projection.residual_absorption_report().certified());
}

fn assert_frozen_matches_fit(
    frozen: &crate::ols::CertifiedFrozenLinearOutput<'_>,
    expected: &CertifiedPreparedOlsFit,
) {
    assert!(frozen.certified());
    assert_eq!(
        frozen.basis().kept_generated_columns(),
        expected.kept_generated_columns()
    );
    for (position, generated) in frozen
        .basis()
        .kept_generated_columns()
        .iter()
        .copied()
        .enumerate()
    {
        let expected_coefficient = expected
            .coefficients()
            .iter()
            .find(|coefficient| {
                coefficient.generated_column_index == Some(generated)
                    && coefficient.status == PreparedOlsFitStatus::Kept
            })
            .and_then(|coefficient| coefficient.value)
            .expect("expected kept coefficient");
        assert_close(frozen.coefficients()[position], expected_coefficient, 5e-10);
    }
    assert_eq!(
        frozen.reported_coefficients().len(),
        expected.coefficients().len()
    );
    for (actual, expected) in frozen
        .reported_coefficients()
        .iter()
        .zip(expected.coefficients())
    {
        match (*actual, expected.value) {
            (Some(actual), Some(expected)) => assert_close(actual, expected, 5e-10),
            (None, None) => {}
            _ => panic!("frozen reported coefficient presence differs"),
        }
    }
    let rank = frozen.coefficients().len();
    assert_eq!(frozen.bread().len(), rank * rank);
    for (left, left_generated) in frozen
        .basis()
        .kept_generated_columns()
        .iter()
        .copied()
        .enumerate()
    {
        let left_position = expected
            .bread()
            .coordinates()
            .iter()
            .position(|coordinate| {
                coordinate.generated_column_index == left_generated
                    && coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
            })
            .expect("expected left bread coordinate");
        for (right, right_generated) in frozen
            .basis()
            .kept_generated_columns()
            .iter()
            .copied()
            .enumerate()
        {
            let right_position = expected
                .bread()
                .coordinates()
                .iter()
                .position(|coordinate| {
                    coordinate.generated_column_index == right_generated
                        && coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                })
                .expect("expected right bread coordinate");
            let dimension = expected.bread().dimension();
            assert_close(
                frozen.bread()[left * rank + right],
                expected.bread().values()[left_position * dimension + right_position],
                2e-9,
            );
        }
    }
    assert_eq!(frozen.finite_within_rows(), expected.residuals().len());
    assert_eq!(frozen.finite_within_columns(), rank);
    assert!(frozen.reconstruction_certificate().passed);
}

trait LinearScienceWitness {
    fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient>;
    fn kept(&self) -> &[usize];
    fn dropped(&self) -> &[usize];
    fn rank(&self) -> PreparedOlsRankCertificate;
    fn score(&self) -> PreparedOlsScoreCertificate;
    fn bread(&self) -> Vec<f64>;
    fn bread_passed(&self) -> bool;
    fn residuals(&self) -> &[f64];
    fn absorption(&self) -> &SolveReport;
    fn residual_absorption(&self) -> &SolveReport;
}

impl LinearScienceWitness for CertifiedPreparedOlsFit {
    fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient> {
        self.coefficients().to_vec()
    }
    fn kept(&self) -> &[usize] {
        self.kept_generated_columns()
    }
    fn dropped(&self) -> &[usize] {
        self.dropped_generated_columns()
    }
    fn rank(&self) -> PreparedOlsRankCertificate {
        self.rank_certificate()
    }
    fn score(&self) -> PreparedOlsScoreCertificate {
        self.score_certificate()
    }
    fn bread(&self) -> Vec<f64> {
        let dimension = self.bread().coordinates().len();
        self.kept_generated_columns()
            .iter()
            .flat_map(|left| {
                let i = self
                    .bread()
                    .coordinates()
                    .iter()
                    .position(|c| {
                        c.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                            && c.generated_column_index == *left
                    })
                    .unwrap();
                self.kept_generated_columns().iter().map(move |right| {
                    let j = self
                        .bread()
                        .coordinates()
                        .iter()
                        .position(|c| {
                            c.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                                && c.generated_column_index == *right
                        })
                        .unwrap();
                    self.bread().values()[i * dimension + j]
                })
            })
            .collect()
    }
    fn bread_passed(&self) -> bool {
        self.bread().certificate().passed
    }
    fn residuals(&self) -> &[f64] {
        self.residuals()
    }
    fn absorption(&self) -> &SolveReport {
        self.absorption_report()
    }
    fn residual_absorption(&self) -> &SolveReport {
        self.residual_absorption_report()
    }
}

impl LinearScienceWitness for CertifiedLinearSessionOutput<'_> {
    fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient> {
        self.coefficients().collect()
    }
    fn kept(&self) -> &[usize] {
        self.basis().kept_generated_columns()
    }
    fn dropped(&self) -> &[usize] {
        self.basis().dropped_generated_columns()
    }
    fn rank(&self) -> PreparedOlsRankCertificate {
        *self.frame().rank_certificate()
    }
    fn score(&self) -> PreparedOlsScoreCertificate {
        *self.frame().score_certificate()
    }
    fn bread(&self) -> Vec<f64> {
        self.frame().bread().to_vec()
    }
    fn bread_passed(&self) -> bool {
        self.frame().canonical_proof().unwrap().bread.passed
    }
    fn residuals(&self) -> &[f64] {
        self.frame().residuals()
    }
    fn absorption(&self) -> &SolveReport {
        self.frame().absorption_report()
    }
    fn residual_absorption(&self) -> &SolveReport {
        self.frame().residual_absorption_report()
    }
}

impl LinearScienceWitness for CertifiedLinearSessionProducts {
    fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient> {
        self.basis()
            .slots()
            .iter()
            .zip(&self.frame().reported_coefficients)
            .map(|(slot, &value)| PreparedOlsFittedCoefficient {
                coefficient_slot: slot.coefficient_slot,
                generated_column_index: slot.generated_column_index,
                status: slot.status,
                value,
            })
            .collect()
    }
    fn kept(&self) -> &[usize] {
        self.basis().kept_generated_columns()
    }
    fn dropped(&self) -> &[usize] {
        self.basis().dropped_generated_columns()
    }
    fn rank(&self) -> PreparedOlsRankCertificate {
        *self.frame().rank_certificate()
    }
    fn score(&self) -> PreparedOlsScoreCertificate {
        *self.frame().score_certificate()
    }
    fn bread(&self) -> Vec<f64> {
        self.frame().bread().to_vec()
    }
    fn bread_passed(&self) -> bool {
        self.frame().canonical_proof().unwrap().bread.passed
    }
    fn residuals(&self) -> &[f64] {
        self.frame().residuals()
    }
    fn absorption(&self) -> &SolveReport {
        self.frame().absorption_report()
    }
    fn residual_absorption(&self) -> &SolveReport {
        self.frame().residual_absorption_report()
    }
}

fn assert_fit_science_equal(
    actual: &impl LinearScienceWitness,
    expected: &impl LinearScienceWitness,
) {
    assert_eq!(actual.coefficients().len(), expected.coefficients().len());
    for (actual, expected) in actual.coefficients().iter().zip(expected.coefficients()) {
        assert_eq!(actual.coefficient_slot, expected.coefficient_slot);
        assert_eq!(
            actual.generated_column_index,
            expected.generated_column_index
        );
        assert_eq!(actual.status, expected.status);
        match (actual.value, expected.value) {
            (Some(actual), Some(expected)) => assert_close(actual, expected, 2e-12),
            (None, None) => {}
            _ => panic!("prepared fit coefficient presence differs"),
        }
    }
    assert_eq!(actual.kept(), expected.kept());
    assert_eq!(actual.dropped(), expected.dropped());
    assert_eq!(actual.kept().len(), expected.kept().len());
    assert_eq!(
        actual.rank().relative_threshold,
        expected.rank().relative_threshold
    );
    // The retained observation-space reduction compresses rows through a
    // streaming Givens factor before CPQR; one-shot fallback applies CPQR
    // directly to N rows.  The selected basis/rank and certificate pass
    // remain exact, while this descriptive trailing-residual magnitude can
    // differ by final-rounding noise.
    assert_close(
        actual.rank().maximum_dropped_trailing_norm,
        expected.rank().maximum_dropped_trailing_norm,
        512.0 * f64::EPSILON,
    );
    assert_eq!(actual.rank().passed, expected.rank().passed);
    // Retained rank revelation may compress rows through Givens before
    // CPQR while the one-shot fallback factors N rows directly. Basis and
    // rank decisions remain exact above. The descriptive diagonal-ratio
    // condition proxy is perturbation-sensitive on deliberately weak
    // full-rank designs, so its machine-precision comparison scales by
    // sqrt(kappa); no certificate threshold or publication value does.
    let condition_scale = actual
        .rank()
        .condition_proxy
        .max(expected.rank().condition_proxy)
        .max(1.0)
        .sqrt();
    assert_close(
        actual.rank().condition_proxy,
        expected.rank().condition_proxy,
        512.0 * f64::EPSILON * condition_scale,
    );
    assert_eq!(actual.score().passed, expected.score().passed);
    assert_eq!(actual.score().moments, expected.score().moments);
    assert_close(
        actual.score().relative_infinity_norm,
        expected.score().relative_infinity_norm,
        2e-11,
    );
    assert_close(
        actual.score().relative_two_norm,
        expected.score().relative_two_norm,
        2e-11,
    );
    let actual_bread = actual.bread();
    let expected_bread = expected.bread();
    assert_eq!(actual_bread.len(), expected_bread.len());
    for (&actual, &expected) in actual_bread.iter().zip(&expected_bread) {
        assert_close(actual, expected, 2e-12);
    }
    assert!(actual.bread_passed() && expected.bread_passed());
    assert_solve_report_science_equal(actual.absorption(), expected.absorption());
    assert_solve_report_science_equal(actual.residual_absorption(), expected.residual_absorption());
    assert_eq!(actual.residuals().len(), expected.residuals().len());
    for (&actual, &expected) in actual.residuals().iter().zip(expected.residuals()) {
        assert_close(actual, expected, 2e-12);
    }
}

fn assert_solve_report_science_equal(actual: &SolveReport, expected: &SolveReport) {
    assert_eq!(actual.requested_solver, expected.requested_solver);
    assert_eq!(actual.used_solver, expected.used_solver);
    // Iterations and the stop label describe work actually performed on
    // different, independently certified projection inputs.  Fresh
    // one-shot absorption owns a combined matrix and its QR routes may
    // reverse destructive Householder storage; retained warm/strict work
    // projects the response and preserves the sole row-stream design.
    // Their scientific equivalence is established below by both passing
    // FE certificates and by the exact fit/residual checks at the caller.
    assert!(actual.certified());
    assert!(expected.certified());
    assert_eq!(actual.fallback_used, expected.fallback_used);
    assert_eq!(
        actual.requested_preconditioner,
        expected.requested_preconditioner
    );
    assert_eq!(actual.used_preconditioner, expected.used_preconditioner);
    assert_eq!(actual.pilot_iterations, expected.pilot_iterations);
    // Pilot iteration counts and routing are exact above.  Once both
    // independently certified FE residuals are at the roundoff floor,
    // the reported contraction is a ratio of two vanishing diagnostics:
    // a one-shot Householder route (always in Low mode and for wide or
    // rank-deficient Fast designs) and the retained row stream may report
    // any finite ratio in [0, 1] even when their published residuals are
    // bit-identical. Outside that explicitly degenerate regime the
    // contraction remains a machine-precision comparison.
    let pilot_floor = 64.0 * f64::EPSILON;
    if actual.certificate.relative_two_norm <= pilot_floor
        && expected.certificate.relative_two_norm <= pilot_floor
        && actual.normal_equation_residual <= pilot_floor
        && expected.normal_equation_residual <= pilot_floor
    {
        assert!(actual.pilot_contraction.is_finite());
        assert!((0.0..=1.0).contains(&actual.pilot_contraction));
        assert!(expected.pilot_contraction.is_finite());
        assert!((0.0..=1.0).contains(&expected.pilot_contraction));
    } else {
        assert_close(
            actual.pilot_contraction,
            expected.pilot_contraction,
            512.0 * f64::EPSILON,
        );
    }
    assert_eq!(actual.warnings, expected.warnings);
    assert_eq!(actual.certificate.passed, expected.certificate.passed);
    // The retained route uses its owned reusable MAP workspace while the
    // one-shot reference uses the borrowed MAP adapter. They execute the
    // same solver and stop policy, but their independent certificate
    // reductions can differ in the final rounding bits. Keep this at a
    // machine-precision bound; neither publication tolerance is relaxed.
    let diagnostic_tolerance = 512.0 * f64::EPSILON;
    assert_close(
        actual.certificate.absolute_infinity_norm,
        expected.certificate.absolute_infinity_norm,
        diagnostic_tolerance,
    );
    assert_close(
        actual.certificate.relative_infinity_norm,
        expected.certificate.relative_infinity_norm,
        diagnostic_tolerance,
    );
    assert_close(
        actual.certificate.relative_two_norm,
        expected.certificate.relative_two_norm,
        diagnostic_tolerance,
    );
    assert_close(
        actual.normal_equation_residual,
        expected.normal_equation_residual,
        diagnostic_tolerance,
    );
}

fn assert_retained_report_has_no_warning_storage(report: &SolveReport) {
    assert!(report.warnings.is_empty());
    assert_eq!(report.warnings.capacity(), 0);
}

fn assert_retained_fit_has_no_warning_storage(
    fit: &crate::ols::linear_session::CertifiedLinearFrame,
) {
    assert_retained_report_has_no_warning_storage(fit.absorption_report());
    assert_retained_report_has_no_warning_storage(fit.residual_absorption_report());
}

#[test]
fn retained_fresh_frozen_and_strict_kernels_use_requested_team_against_global_pool() {
    const CHILD_ENV: &str = "FEREG_RETAINED_THREAD_TEAM_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let executable = std::env::current_exe().expect("current Rust test executable");
        let output = std::process::Command::new(executable)
            .arg("retained_fresh_frozen_and_strict_kernels_use_requested_team_against_global_pool")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .output()
            .expect("spawn isolated Rayon-global child");
        assert!(
            output.status.success(),
            "isolated retained-team child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(7)
        .build_global()
        .expect("install mismatched seven-thread global pool");
    assert_eq!(rayon::current_num_threads(), 7);

    let rows = 512_usize;
    let regressors = (0..4)
        .map(|column| {
            (0..rows)
                .map(|row| (((row >> column) & 1) as f64) * 2.0 - 1.0)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let response = (0..rows)
        .map(|row| {
            regressors
                .iter()
                .enumerate()
                .map(|(column, values)| (column + 1) as f64 * values[row])
                .sum::<f64>()
                + (row % 11) as f64 / 17.0
        })
        .collect::<Vec<_>>();
    let prepared_data = prepared(
        &response,
        &regressors,
        &[false; 4],
        None,
        false,
        TestWeights::Unit,
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("session frame");

    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for requested in [1_usize, 4] {
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 2,
                    threads: ThreadConfig::from_requested(requested, true).expect("thread config"),
                },
                ..PreparedOlsFitConfig::default()
            };
            let mut session = CertifiedLinearSession::new(
                &prepared_data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .expect("retained session");

            begin_retained_moment_worker_observation();
            session.solve_fresh().expect("observed fresh solve");
            assert_retained_moment_team(finish_retained_moment_worker_observation(), requested);

            session
                .refresh_response(&response, 1)
                .expect("response generation refresh");
            begin_retained_moment_worker_observation();
            session.solve_frozen().expect("observed frozen solve");
            assert_retained_moment_team(finish_retained_moment_worker_observation(), requested);

            begin_retained_moment_worker_observation();
            let candidate = session
                .strict_candidate()
                .expect("observed strict candidate");
            assert!(candidate.candidate().frame().rank_certificate().passed);
            let observed = finish_retained_moment_worker_observation();
            assert_retained_moment_team(observed, requested);
            candidate.commit();
        }
    }
}

#[test]
fn wide_fast_and_low_agree_for_weighted_full_weak_and_deficient_designs() {
    let rows = 512_usize;
    let weights = (0..rows)
        .map(|row| 1.0 + (row % 7) as f64)
        .collect::<Vec<_>>();
    for columns in [65_usize, 128] {
        for gap in [1.0_f64, 1e-4, 0.0] {
            let mut x = (1..=columns)
                .map(|mask| {
                    (0..rows)
                        .map(|row| {
                            if (row & mask).count_ones() % 2 == 0 {
                                1.0
                            } else {
                                -1.0
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            if gap != 1.0 {
                for row in 0..rows {
                    x[columns - 1][row] = x[0][row] + gap * x[columns - 1][row];
                }
            }
            let y = (0..rows)
                .map(|row| {
                    x.iter()
                        .enumerate()
                        .map(|(j, values)| values[row] * (j + 1) as f64 / columns as f64)
                        .sum::<f64>()
                        + if (row & 256).count_ones() % 2 == 0 {
                            0.01
                        } else {
                            -0.01
                        }
                })
                .collect::<Vec<_>>();
            let data = prepared(
                &y,
                &x,
                &vec![false; columns],
                None,
                false,
                TestWeights::Analytic(&weights),
            );
            let fast = fit_prepared_model_coefficients(&data, PreparedOlsFitConfig::default())
                .expect("certified wide fast fit");
            let low_config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode: PreparedOlsExecutionMode::Low,
                    poolsize: 8,
                    ..PreparedOlsExecutionConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            };
            let low =
                fit_prepared_model_coefficients(&data, low_config).expect("certified wide low fit");
            assert_eq!(fast.finite_rank(), columns - usize::from(gap == 0.0));
            assert_eq!(fast.kept_generated_columns(), low.kept_generated_columns());
            assert_eq!(
                fast.dropped_generated_columns(),
                low.dropped_generated_columns()
            );
            for (a, b) in fast.coefficients().iter().zip(low.coefficients()) {
                assert_eq!(a.status, b.status);
                assert_eq!(a.coefficient_slot, b.coefficient_slot);
                if let (Some(a), Some(b)) = (a.value, b.value) {
                    assert!((a - b).abs() <= 1e-10 + 1e-7 * a.abs().max(b.abs()));
                } else {
                    assert_eq!(a.value, b.value);
                }
            }
            for (a, b) in fast.bread().values().iter().zip(low.bread().values()) {
                assert!((a - b).abs() <= 1e-10 + 1e-6 * a.abs().max(b.abs()));
            }
            for (a, b) in fast.residuals().iter().zip(low.residuals()) {
                assert_close(*a, *b, 1e-9);
            }
            for fit in [&fast, &low] {
                assert!(fit.rank_certificate().passed);
                assert!(fit.score_certificate().passed);
            }
        }
    }
}

#[test]
fn retained_scratch_parallel_residuals_match_weighted_orthogonal_oracle() {
    // Complete Walsh blocks remain orthogonal under either block-constant
    // weight frame. The unused channel is an independent residual oracle;
    // the duplicate checks rank selection and validation of omitted storage.
    let rows = 2048;
    let channel = |row: usize, mask: usize| {
        if (row & mask).count_ones() % 2 == 0 {
            1.0
        } else {
            -1.0
        }
    };
    for width in [6, 65] {
        let mut regressors = (1..=width)
            .map(|mask| (0..rows).map(|row| channel(row, mask)).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        regressors.push(regressors[0].clone());
        let response = (0..rows)
            .map(|row| {
                (0..width)
                    .map(|column| (column + 1) as f64 / 67.0 * regressors[column][row])
                    .sum::<f64>()
                    + channel(row, 256) / 16.0
            })
            .collect::<Vec<_>>();
        let initial_weights = (0..rows)
            .map(|row| (1 + row / 512) as f64)
            .collect::<Vec<_>>();
        let next_weights = (0..rows)
            .map(|row| (7 - row / 512) as f64)
            .collect::<Vec<_>>();
        let data = prepared(
            &response,
            &regressors,
            &vec![false; width + 1],
            None,
            false,
            TestWeights::Frequency(&initial_weights),
        );
        let next = prepared(
            &response,
            &regressors,
            &vec![false; width + 1],
            None,
            false,
            TestWeights::Frequency(&next_weights),
        );
        let frame = LinearSolveFrame::from_prepared_ols(&data).unwrap();
        let next_frame = LinearSolveFrame::from_prepared_ols(&next).unwrap();
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            // Three workers leave a short last row partition.
            for threads in [1, 3, 8] {
                let config = PreparedOlsFitConfig {
                    execution: PreparedOlsExecutionConfig {
                        mode,
                        poolsize: 8,
                        threads: ThreadConfig::from_requested(threads, true).unwrap(),
                    },
                    ..PreparedOlsFitConfig::default()
                };
                let mut session = CertifiedLinearSession::new(
                    &data,
                    frame,
                    config,
                    LinearSessionGenerations::new(0, 0, 0, 0, 0),
                )
                .unwrap();
                let check = |output: CertifiedLinearSessionOutput<'_>| {
                    let fit = output.frame();
                    assert_eq!(fit.finite_rank(), width);
                    assert_eq!(output.basis().dropped_generated_columns(), &[width]);
                    for coefficient in output.coefficients().filter(|c| c.value.is_some()) {
                        assert_close(
                            coefficient.value.unwrap(),
                            (coefficient.generated_column_index.unwrap() + 1) as f64 / 67.0,
                            2e-12,
                        );
                    }
                    for (row, &residual) in fit.residuals().iter().enumerate() {
                        assert_close(residual, channel(row, 256) / 16.0, 2e-12);
                    }
                    assert!(fit.rank_certificate().passed);
                    assert!(fit.score_certificate().passed);
                };
                check(session.solve_fresh().unwrap());
                session
                    .refresh_weights(next_frame.raw_weights(), next_frame.bread_weight_scale(), 1)
                    .unwrap();
                let strict = session.strict_candidate().unwrap();
                check(strict.candidate());
                strict.commit();
            }
        }
    }
}

#[test]
fn retained_session_matches_one_shot_above_sixty_four_columns_with_exact_deficiency() {
    let rows = 128_usize;
    let mut regressors = (1..=65_usize)
        .map(|mask| {
            (0..rows)
                .map(|row| {
                    if (row & mask).count_ones() % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    regressors.push(regressors[0].clone());
    let response = (0..rows)
        .map(|row| {
            regressors[..65]
                .iter()
                .enumerate()
                .map(|(column, values)| (column + 1) as f64 / 67.0 * values[row])
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let prepared_data = prepared(
        &response,
        &regressors,
        &vec![false; regressors.len()],
        None,
        false,
        TestWeights::Unit,
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("wide frame");

    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 8,
                threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
            },
            ..PreparedOlsFitConfig::default()
        };
        let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
            .expect("wide one-shot reference");
        assert_eq!(expected.finite_rank(), 65);
        assert_eq!(expected.dropped_generated_columns(), &[65]);

        let mut session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .expect("wide retained session");
        let fresh = session.solve_fresh().expect("wide retained fresh solve");
        assert_fit_science_equal(&fresh, &expected);
        session
            .refresh_response(&response, 1)
            .expect("wide response refresh");
        let frozen = session.solve_frozen().expect("wide retained frozen solve");
        assert_frozen_matches_fit(&frozen, &expected);
        let strict = session
            .strict_candidate()
            .expect("wide retained strict solve");
        assert!(strict.basis_unchanged());
        assert_fit_science_equal(&strict.candidate(), &expected);
        strict.commit();
    }
}

#[test]
fn retained_session_drops_general_and_scaled_exact_linear_combinations() {
    let first = vec![-5.0_f64, 7.0, 8.0, -3.0, 4.0, 9.0, -6.0, 2.0];
    let second = vec![-1.0_f64, 4.0, -10.0, 6.0, 2.0, -7.0, 5.0, 3.0];
    let combination = first
        .iter()
        .copied()
        .zip(second.iter().copied())
        .map(|(left, right)| 2.0 * left - 3.0 * right)
        .collect::<Vec<_>>();
    let scaled = first
        .iter()
        .copied()
        .map(|value| 6.285_119_033_872_249_f64 * value)
        .collect::<Vec<_>>();
    let response = first
        .iter()
        .copied()
        .zip(second.iter().copied())
        .map(|(left, right)| 2.0 * left - 3.0 * right)
        .collect::<Vec<_>>();
    let regressors = vec![first, second, combination, scaled];
    let prepared_data = prepared(
        &response,
        &regressors,
        &[false; 4],
        None,
        false,
        TestWeights::Unit,
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("exact frame");
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 3,
                threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
            },
            ..PreparedOlsFitConfig::default()
        };
        let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
            .expect("exact-combination one-shot fit");
        assert_eq!(expected.finite_rank(), 2);
        let mut session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .expect("exact-combination retained session");
        assert_fit_science_equal(
            &session.solve_fresh().expect("exact-combination fresh"),
            &expected,
        );
        assert_fit_science_equal(
            &session.strictify().expect("exact-combination strict"),
            &expected,
        );
    }
}

#[test]
fn retained_session_covers_zero_and_single_finite_rank_through_final_handoff() {
    let response = vec![1.0_f64, -2.0, 3.0, 0.5, 4.0, -1.5];
    let regressor = vec![-3.0_f64, -1.0, 0.5, 2.0, 4.0, 6.0];
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for finite_columns in 0..=1_usize {
            let regressors = if finite_columns == 0 {
                Vec::new()
            } else {
                vec![regressor.clone()]
            };
            let prepared_data = prepared(
                &response,
                &regressors,
                &vec![false; finite_columns],
                None,
                false,
                TestWeights::Unit,
            );
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 2,
                    threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
                },
                ..PreparedOlsFitConfig::default()
            };
            let frame =
                LinearSolveFrame::from_prepared_ols(&prepared_data).expect("rank edge frame");
            let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
                .expect("rank edge one-shot");
            assert_eq!(expected.finite_rank(), finite_columns);
            let mut session = CertifiedLinearSession::new(
                &prepared_data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .expect("rank edge retained session");
            assert_fit_science_equal(&session.solve_fresh().expect("rank edge fresh"), &expected);
            session
                .refresh_response(&response, 1)
                .expect("rank edge response refresh");
            assert_frozen_matches_fit(
                &session.solve_frozen().expect("rank edge frozen"),
                &expected,
            );
            assert_fit_science_equal(&session.strictify().expect("rank edge strict"), &expected);
            let products = session
                .into_current_products(&mut || Ok(()))
                .expect("rank edge final products");
            assert_eq!(products.frame().finite_rank(), finite_columns);
            assert_eq!(products.finite_within_design().dimension(), finite_columns);
        }
    }
}

#[test]
fn frozen_session_uses_standardized_qr_fallback_for_clear_weak_full_rank() {
    let rows = 64_usize;
    let first = (0..rows)
        .map(|row| row as f64 - (rows - 1) as f64 / 2.0)
        .collect::<Vec<_>>();
    let second = first
        .iter()
        .copied()
        .enumerate()
        .map(|(row, value)| value + 5.0e-4 * if row % 2 == 0 { 1.0 } else { -1.0 })
        .collect::<Vec<_>>();
    let response = first
        .iter()
        .copied()
        .zip(second.iter().copied())
        .map(|(left, right)| 1.25 * left - 0.75 * right)
        .collect::<Vec<_>>();
    let prepared_data = prepared(
        &response,
        &[first, second],
        &[false, false],
        None,
        false,
        TestWeights::Unit,
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("weak-rank frame");
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 4,
                threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
            },
            ..PreparedOlsFitConfig::default()
        };
        let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
            .expect("weak-rank one-shot");
        assert_eq!(expected.finite_rank(), 2);
        let mut session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .expect("weak-rank retained session");
        assert_fit_science_equal(&session.solve_fresh().expect("weak-rank fresh"), &expected);
        session
            .strict_candidate()
            .expect("weak-rank initial strict")
            .commit();
        session
            .refresh_response(&response, 1)
            .expect("weak-rank response refresh");
        begin_frozen_qr_fallback_observation();
        let frozen = session.solve_frozen().expect("weak-rank frozen fallback");
        assert!(
            finish_frozen_qr_fallback_observation(),
            "weak but clearly full-rank frozen basis must use QR fallback"
        );
        assert_frozen_matches_fit(&frozen, &expected);
        assert_fit_science_equal(
            &session.strictify().expect("weak-rank final strict"),
            &expected,
        );
        let products = session
            .into_current_products(&mut || Ok(()))
            .expect("weak-rank final products");
        assert_eq!(products.frame().finite_rank(), 2);
    }
}

#[test]
fn ppml_fresh_norm_selection_preserves_rank_and_explicit_weight_divisors() {
    let rows = 257;
    let groups = (0..rows).map(|r| (r % 7) as f64).collect::<Vec<_>>();
    let weights = (0..rows).map(|r| (1 + r % 5) as f64).collect::<Vec<_>>();
    let x1 = (0..rows)
        .map(|r| 100.0 * groups[r] + (r as f64 * 0.37).sin())
        .collect::<Vec<_>>();
    let x2 = (0..rows)
        .map(|r| (r as f64 * 0.17).cos())
        .collect::<Vec<_>>();
    let x3 = x1.iter().map(|value| 2.0 * value).collect::<Vec<_>>();
    let y = (0..rows)
        .map(|r| 0.7 * x1[r] - 0.3 * x2[r] + groups[r] + (r as f64 * 0.11).sin())
        .collect::<Vec<_>>();
    for kind in [
        TestWeights::Unit,
        TestWeights::Frequency(&weights),
        TestWeights::Probability(&weights),
    ] {
        let data = prepared(
            &y,
            &[x1.clone(), x2.clone(), x3.clone(), groups.clone()],
            &[false; 4],
            Some(&groups),
            true,
            kind,
        );
        let canonical = LinearSolveFrame::from_prepared_ols(&data).unwrap();
        let config = PreparedOlsFitConfig {
            absorb: AbsorbConfig {
                solver: FeSolverKind::Map,
                ..AbsorbConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let mut generic = CertifiedLinearSession::new(
            &data,
            canonical,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .unwrap();
        let expected_output = generic.solve_fresh().unwrap();
        let expected = &expected_output;
        assert_eq!(expected.frame().finite_rank(), 2);
        for factor in [1.0, 8.0] {
            if canonical.weights().values.is_none() && factor != 1.0 {
                continue;
            }
            let mut scale = canonical.bread_weight_scale();
            scale.solver_weight_divisor *= factor;
            scale.reference_weight_multiplier *= factor;
            let frame =
                LinearSolveFrame::from_parts(canonical.response(), canonical.raw_weights(), scale)
                    .unwrap();
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
                data.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            assert_eq!(session.bread_weight_scale_for_test(), scale);
            let actual_output = session.solve_fresh().unwrap();
            let actual = &actual_output;
            // The generic and categorical projectors have different raw
            // FE-certificate rounding, also on unchanged production code.
            // Compare scientific products and each current-frame certificate;
            // an explicit divisor need not preserve diagnostic magnitudes.
            assert_eq!(actual.frame().finite_rank(), expected.frame().finite_rank());
            assert_eq!(
                actual.basis().kept_generated_columns(),
                expected.basis().kept_generated_columns()
            );
            assert_eq!(
                actual.basis().dropped_generated_columns(),
                expected.basis().dropped_generated_columns()
            );
            assert_eq!(actual.coefficients().len(), expected.coefficients().len());
            for (actual, expected) in actual.coefficients().zip(expected.coefficients()) {
                assert_eq!(actual.coefficient_slot, expected.coefficient_slot);
                assert_eq!(
                    actual.generated_column_index,
                    expected.generated_column_index
                );
                assert_eq!(actual.status, expected.status);
                match (actual.value, expected.value) {
                    (Some(actual), Some(expected)) => assert_close(actual, expected, 2e-12),
                    (actual, expected) => assert_eq!(actual, expected),
                }
            }
            assert_eq!(actual.frame().bread().len(), expected.frame().bread().len());
            for (&actual, &expected) in actual.frame().bread().iter().zip(expected.frame().bread())
            {
                assert_close(actual, expected, 2e-12);
            }
            assert_eq!(
                actual.frame().residuals().len(),
                expected.frame().residuals().len()
            );
            for (actual, expected) in actual
                .frame()
                .residuals()
                .iter()
                .zip(expected.frame().residuals())
            {
                assert_close(*actual, *expected, 2e-12);
            }
            assert!(actual.frame().rank_certificate().passed);
            assert!(actual.frame().score_certificate().passed);
            assert!(actual.frame().canonical_proof().unwrap().bread.passed);
            assert!(actual.frame().absorption_report().certified());
            assert!(actual.frame().residual_absorption_report().certified());
        }
    }
}

#[test]
fn ppml_fused_weight_refresh_rejects_stale_fe_collinearity_basis() {
    let groups = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
    let x = groups
        .iter()
        .enumerate()
        .map(|(r, g)| g + if r % 2 == 0 { -0.001 } else { 0.001 })
        .collect::<Vec<_>>();
    let y = groups
        .iter()
        .enumerate()
        .map(|(r, g)| g + if r % 2 == 0 { -0.1 } else { 0.1 })
        .collect::<Vec<_>>();
    let initial_weights = vec![1.0; groups.len()];
    let next_weights = (0..groups.len())
        .map(|r| if r % 2 == 0 { 1.0 } else { 1e-8 })
        .collect::<Vec<_>>();
    let initial = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&groups),
        true,
        TestWeights::Analytic(&initial_weights),
    );
    let next = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&groups),
        true,
        TestWeights::Analytic(&next_weights),
    );
    let config = PreparedOlsFitConfig {
        absorb: AbsorbConfig {
            solver: FeSolverKind::Map,
            ..AbsorbConfig::default()
        },
        ..PreparedOlsFitConfig::default()
    };
    let canonical = LinearSolveFrame::from_prepared_ols(&next).unwrap();
    for divisor_factor in [1.0, 8.0] {
        let mut scale = canonical.bread_weight_scale();
        scale.solver_weight_divisor *= divisor_factor;
        scale.reference_weight_multiplier *= divisor_factor;
        let fresh_frame =
            LinearSolveFrame::from_parts(canonical.response(), canonical.raw_weights(), scale)
                .unwrap();
        let mut fresh = CertifiedLinearSession::new_with_fe_design(
            &initial,
            fresh_frame,
            config,
            LinearSessionGenerations::new(0, 0, 1, 1, 0),
            initial.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        let fresh_output = fresh.solve_fresh().unwrap();
        let expected = &fresh_output;
        assert_eq!(expected.frame().finite_rank(), 0);
        let mut reused = CertifiedLinearSession::new_with_fe_design(
            &initial,
            LinearSolveFrame::from_prepared_ols(&initial).unwrap(),
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
            initial.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        assert_eq!(reused.solve_fresh().unwrap().frame().finite_rank(), 1);
        reused.refresh_response(&y, 1).unwrap();
        reused
            .refresh_weights(
                fresh_frame.raw_weights(),
                fresh_frame.bread_weight_scale(),
                1,
            )
            .unwrap();
        // Each two-row group's weighted within sum of squares is
        // w0*w1/(w0+w1) * (x1-x0)^2. The current ratio is below the unchanged
        // FE-collinearity cutoff; using the old numerator instead is above it.
        let within_square = 4.0 * (1e-8 / (1.0 + 1e-8)) * 0.002_f64.powi(2);
        let raw_square = x
            .iter()
            .zip(&next_weights)
            .map(|(x, w)| w * x * x)
            .sum::<f64>();
        assert!(within_square / raw_square < config.fixed_effect_collinearity_tolerance);
        assert!(8e-6 / raw_square > config.fixed_effect_collinearity_tolerance);
        // Strict staging must recertify the prior basis before a transaction
        // can publish. Here that basis became FE-collinear under new weights.
        let error = reused
            .strictify()
            .err()
            .expect("stale prior basis must fail closed");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(
            error.message,
            "strict weighted-linear prior basis could not be recertified"
        );
        assert_eq!(reused.basis_generation_for_test(), 0);
        assert!(reused.output().is_err());
        reused.refresh_basis(1).unwrap();
        let actual = reused.solve_fresh().unwrap();
        assert_fit_science_equal(&actual, expected);
    }
}

#[test]
fn certified_linear_session_fresh_frozen_and_strict_match_across_routes() {
    let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
    let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
    let z = vec![1.0, -1.0, 0.5, 2.5, -2.0, 1.5, 3.0, -0.5];
    let y = vec![1.2, 4.0, -0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
    let refreshed_y = vec![1.7, 3.6, 0.1, 6.8, 2.1, 9.0, 6.2, 12.4];
    let analytic = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
    let analytic_refresh = vec![1.5, 0.75, 2.0, 1.25, 3.5, 0.6, 1.8, 2.2];
    let frequency = vec![1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 3.0, 2.0];
    let frequency_refresh = vec![2.0, 1.0, 1.0, 3.0, 2.0, 4.0, 1.0, 2.0];
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for has_fe in [false, true] {
            for weight_case in 0..3 {
                let initial_weights = match weight_case {
                    0 => TestWeights::Unit,
                    1 => TestWeights::Analytic(&analytic),
                    _ => TestWeights::Frequency(&frequency),
                };
                let refreshed_weights = match weight_case {
                    0 | 1 => TestWeights::Analytic(&analytic_refresh),
                    _ => TestWeights::Frequency(&frequency_refresh),
                };
                let fixed_effect = has_fe.then_some(g.as_slice());
                let prepared_data = prepared(
                    &y,
                    &[x.clone(), z.clone()],
                    &[false, false],
                    fixed_effect,
                    true,
                    initial_weights,
                );
                let config = PreparedOlsFitConfig {
                    absorb: AbsorbConfig {
                        solver: FeSolverKind::Map,
                        ..AbsorbConfig::default()
                    },
                    execution: PreparedOlsExecutionConfig {
                        mode,
                        poolsize: 1,
                        ..PreparedOlsExecutionConfig::default()
                    },
                    ..PreparedOlsFitConfig::default()
                };
                let initial_frame =
                    LinearSolveFrame::from_prepared_ols(&prepared_data).expect("initial frame");
                let initial_expected =
                    fit_prepared_model_coefficients(&prepared_data, config).expect("initial fit");
                if !has_fe {
                    assert!(prepared_data.fit_has_absorbed_intercept());
                    assert!(initial_expected.coefficients().iter().any(|coefficient| {
                        coefficient.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept
                    }));
                }
                let mut session = CertifiedLinearSession::new(
                    &prepared_data,
                    initial_frame,
                    config,
                    LinearSessionGenerations::new(0, 0, 0, 0, 0),
                )
                .expect("session");
                let initial = session.solve_fresh().expect("fresh session fit");
                assert_fit_science_equal(&initial, &initial_expected);
                assert_retained_fit_has_no_warning_storage(initial.frame());

                session
                    .refresh_response(&refreshed_y, 1)
                    .expect("response refresh");
                let response_frame = LinearSolveFrame::from_parts(
                    &refreshed_y,
                    initial_frame.raw_weights(),
                    initial_frame.bread_weight_scale(),
                )
                .expect("response frame");
                let response_expected =
                    fit_prepared_weighted_linear_once(&prepared_data, response_frame, config)
                        .expect("response reference");
                let response_warm = session.solve_warm().expect("response warm solve");
                assert_fit_science_equal(&response_warm, &response_expected);
                let response_frozen = session.solve_frozen().expect("response frozen solve");
                assert_frozen_matches_fit(&response_frozen, &response_expected);
                assert_retained_report_has_no_warning_storage(
                    response_frozen.response_absorption_report(),
                );

                let refreshed_prepared = prepared(
                    &refreshed_y,
                    &[x.clone(), z.clone()],
                    &[false, false],
                    fixed_effect,
                    true,
                    refreshed_weights,
                );
                let refreshed_frame = LinearSolveFrame::from_prepared_ols(&refreshed_prepared)
                    .expect("refreshed weight frame");
                session
                    .refresh_weights(
                        refreshed_frame.raw_weights(),
                        refreshed_frame.bread_weight_scale(),
                        1,
                    )
                    .expect("weight refresh");
                let weighted_frame = LinearSolveFrame::from_parts(
                    &refreshed_y,
                    refreshed_frame.raw_weights(),
                    refreshed_frame.bread_weight_scale(),
                )
                .expect("weighted reference frame");
                let weighted_expected =
                    fit_prepared_weighted_linear_once(&prepared_data, weighted_frame, config)
                        .expect("weighted reference");
                let weighted_frozen = session.solve_frozen().expect("weighted frozen solve");
                assert_frozen_matches_fit(&weighted_frozen, &weighted_expected);
                let warmed_capacities = session.workspace_capacities();
                assert!(
                    session.output().is_err(),
                    "frozen proof cannot publish canonical products"
                );
                assert!(warmed_capacities.frozen_residual_elements >= y.len());
                assert!(warmed_capacities.within_metadata[2] > 0);
                assert!(warmed_capacities.basis_metadata[0] > 0);

                session
                    .refresh_response(&refreshed_y, 2)
                    .expect("same-value response generation refresh");
                let reused_frozen = session.solve_frozen().expect("reused frozen solve");
                assert_frozen_matches_fit(&reused_frozen, &weighted_expected);
                assert!(reused_frozen.reuse_report().within_design_reused);
                assert!(reused_frozen.reuse_report().response_buffer_reused);
                assert!(reused_frozen.reuse_report().kernel_moment_workspace_reused);
                assert!(reused_frozen.reuse_report().kernel_solver_workspace_reused);
                assert!(reused_frozen.reuse_report().fe_projection_workspace_reused);
                assert!(reused_frozen.reuse_report().steady_state_allocation_free);
                assert_eq!(session.workspace_capacities(), warmed_capacities);
                let scratch_identity = session
                    .retained_scratch_identity()
                    .map(std::path::Path::to_path_buf);

                for iteration in 0..16_u64 {
                    let response = if iteration % 2 == 0 { &y } else { &refreshed_y };
                    let loop_weights = match (weight_case, iteration % 2) {
                        (2, 0) => TestWeights::Frequency(&frequency),
                        (2, _) => TestWeights::Frequency(&frequency_refresh),
                        (_, 0) => TestWeights::Analytic(&analytic),
                        (_, _) => TestWeights::Analytic(&analytic_refresh),
                    };
                    let loop_prepared = prepared(
                        response,
                        &[x.clone(), z.clone()],
                        &[false, false],
                        fixed_effect,
                        true,
                        loop_weights,
                    );
                    let loop_frame = LinearSolveFrame::from_prepared_ols(&loop_prepared)
                        .expect("alternating frame");
                    session
                        .refresh_response(response, 3 + iteration)
                        .expect("alternating response refresh");
                    session
                        .refresh_weights(
                            loop_frame.raw_weights(),
                            loop_frame.bread_weight_scale(),
                            2 + iteration,
                        )
                        .expect("alternating weight refresh");
                    let expected = fit_prepared_weighted_linear_once(
                        &prepared_data,
                        LinearSolveFrame::from_parts(
                            response,
                            loop_frame.raw_weights(),
                            loop_frame.bread_weight_scale(),
                        )
                        .expect("alternating reference frame"),
                        config,
                    )
                    .expect("alternating canonical reference");
                    let frozen = session.solve_frozen().expect("alternating frozen solve");
                    assert_frozen_matches_fit(&frozen, &expected);
                    assert!(frozen.reuse_report().fe_projection_workspace_reused);
                    assert!(frozen.reuse_report().kernel_moment_workspace_reused);
                    assert!(frozen.reuse_report().kernel_solver_workspace_reused);
                    assert!(frozen.reuse_report().steady_state_allocation_free);
                    assert_eq!(session.workspace_capacities(), warmed_capacities);
                    assert_eq!(
                        session.retained_scratch_identity(),
                        scratch_identity.as_deref()
                    );
                }

                let strict_transaction = session.strict_candidate().expect("strict candidate");
                assert!(
                    strict_transaction
                        .workspace_capacities()
                        .frozen_residual_elements
                        >= y.len()
                );
                assert!(
                    strict_transaction.rollback_residual_capacity() >= y.len(),
                    "the candidate residual overlaps the rollback residual until commit"
                );
                strict_transaction.commit();
                let strict = session.output().expect("strict session fit");
                assert_retained_fit_has_no_warning_storage(strict.frame());
                let final_weights = if weight_case == 2 {
                    TestWeights::Frequency(&frequency_refresh)
                } else {
                    TestWeights::Analytic(&analytic_refresh)
                };
                let final_prepared = prepared(
                    &refreshed_y,
                    &[x.clone(), z.clone()],
                    &[false, false],
                    fixed_effect,
                    true,
                    final_weights,
                );
                let final_frame = LinearSolveFrame::from_prepared_ols(&final_prepared)
                    .expect("final strict frame");
                let final_expected = fit_prepared_weighted_linear_once(
                    &prepared_data,
                    LinearSolveFrame::from_parts(
                        &refreshed_y,
                        final_frame.raw_weights(),
                        final_frame.bread_weight_scale(),
                    )
                    .expect("final strict reference frame"),
                    config,
                )
                .expect("final strict reference");
                assert_fit_science_equal(&strict, &final_expected);
                let memory = strict.retained_within_memory();
                assert_eq!(memory.rows, y.len());
                assert_eq!(memory.columns, strict.frame().finite_rank());
                assert_eq!(memory.mode, mode);
                let final_capacities = session.workspace_capacities();
                assert!(final_capacities.frozen_residual_elements >= y.len());
                assert_eq!(
                    final_capacities.frozen_residual_elements,
                    y.len(),
                    "canonical frame owns the one residual allocation"
                );
                let products = session
                    .into_current_products(&mut || Ok(()))
                    .expect("owned final products");
                assert_fit_science_equal(&products, &final_expected);
                let design = products.finite_within_design();
                assert_eq!(design.nrows(), y.len());
                assert_eq!(design.dimension(), products.frame().finite_rank());
                assert_eq!(
                    products.identified_bread().len(),
                    products.frame().finite_rank() * products.frame().finite_rank()
                );
                let mut within_row = vec![0.0; design.dimension()];
                design
                    .fill_row(0, &mut within_row)
                    .expect("owned within row");
                assert!(within_row.iter().all(|value| value.is_finite()));
                fn assert_copy_sync<T: Copy + Sync>() {}
                assert_copy_sync::<CertifiedLinearWithinDesign<'_>>();
                let mut adapter_row = vec![0.0; design.dimension()];
                design
                    .fill_row(0, &mut adapter_row)
                    .expect("thread-safe within row");
                assert_eq!(adapter_row, within_row);
            }
        }
    }
}

#[test]
fn retained_low_residual_report_preserves_actual_work_without_changing_one_shot() {
    let group = [1.0_f64, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
    let x = [-2.0_f64, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
    let z = [1.0_f64, -1.0, 0.5, 2.5, -2.0, 1.5, 3.0, -0.5];
    let outcome = [1.2_f64, 4.0, -0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
    let prepared_data = prepared(
        &outcome,
        &[x.to_vec(), z.to_vec()],
        &[false, false],
        Some(&group),
        true,
        TestWeights::Unit,
    );
    let config = PreparedOlsFitConfig {
        absorb: AbsorbConfig {
            solver: FeSolverKind::Map,
            ..AbsorbConfig::default()
        },
        execution: PreparedOlsExecutionConfig {
            mode: PreparedOlsExecutionMode::Low,
            poolsize: 1,
            threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
        },
        ..PreparedOlsFitConfig::default()
    };
    let expected =
        fit_prepared_model_coefficients(&prepared_data, config).expect("one-shot Low reference");
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("session frame");
    let mut session = CertifiedLinearSession::new(
        &prepared_data,
        frame,
        config,
        LinearSessionGenerations::new(0, 0, 0, 0, 0),
    )
    .expect("retained Low session");
    let retained = session.solve_fresh().expect("retained Low fit");

    // The public fereg Low route destroys and reverses its one-shot
    // Householder scratch, leaving a real one-sweep residual cleanup on
    // this fixture.  The retained PPML row stream must preserve its sole
    // design; direct subtraction is already FE-certified and truthfully
    // reports zero cleanup iterations.  Both are independent, passing
    // certificates at the roundoff floor.
    assert_eq!(expected.residual_absorption_report().iterations, 1);
    assert_eq!(retained.frame().residual_absorption_report().iterations, 0);
    for report in [
        expected.residual_absorption_report(),
        retained.frame().residual_absorption_report(),
    ] {
        assert!(report.certified());
        assert!(report.certificate.relative_two_norm <= 64.0 * f64::EPSILON);
        assert!(report.normal_equation_residual <= 64.0 * f64::EPSILON);
    }
    assert_fit_science_equal(&retained, &expected);
}

#[test]
fn certified_linear_session_generation_invalidation_is_fail_closed() {
    let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
    let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
    let prepared_data = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        None,
        true,
        TestWeights::Unit,
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
    let mut session = CertifiedLinearSession::new(
        &prepared_data,
        frame,
        PreparedOlsFitConfig::default(),
        LinearSessionGenerations::new(10, 20, 30, 40, 50),
    )
    .expect("session");
    session.solve_fresh().expect("fresh fit");
    assert_eq!(
        session
            .refresh_response(&y, 40)
            .expect_err("response generation must increase")
            .code,
        ErrorCode::InvalidArgument
    );
    session.refresh_basis(51).expect("basis generation");
    assert!(session.output().is_err());
    session.solve_fresh().expect("refreshed basis fit");
    session.invalidate_sample(11).expect("sample invalidation");
    session
        .invalidate_topology(21)
        .expect("topology invalidation");
    assert!(session.requires_rebind());
    let error = match session.strictify() {
        Ok(_) => panic!("invalidated session must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}

#[test]
fn rejected_uncertified_strict_candidate_restores_exact_basis_transaction() {
    let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
    let duplicate_x = x.clone();
    let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
    let prepared_data = prepared(
        &y,
        &[x, duplicate_x],
        &[false, false],
        None,
        true,
        TestWeights::Unit,
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let mut session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 7),
        )
        .expect("session");
        let initial = session.solve_fresh().expect("initial canonical fit");
        assert!(
            initial.frame().canonical_proof().is_some(),
            "session solves must not retain a duplicate N-by-K score cache"
        );
        let original_kept = initial.basis().kept_generated_columns().to_vec();
        let original_dropped = initial.basis().dropped_generated_columns().to_vec();
        assert!(!original_dropped.is_empty());

        let mut strict = session.strict_candidate().expect("speculative strict fit");
        // Model the caller's uncertified strict-output branch: candidate
        // values are inspected but are not accepted as estimator state.
        assert!(strict.candidate().frame().finite_rank() > 0);
        assert!(strict.candidate().frame().canonical_proof().is_some());
        assert!(strict.inject_same_rank_basis_pivot());
        assert_eq!(strict.basis_generation_for_test(), 8);
        strict.rollback();
        assert_eq!(session.basis_generation_for_test(), 7);

        let recovered = session
            .solve_frozen()
            .expect("rollback reconstruction must support the next frozen solve");
        assert!(recovered.certified());
        assert_eq!(recovered.basis().kept_generated_columns(), original_kept);
        let rebuilt = session
            .solve_fresh()
            .expect("canonical solve after rejected strict candidate");
        assert_eq!(rebuilt.basis().kept_generated_columns(), original_kept);
        assert_eq!(
            rebuilt.basis().dropped_generated_columns(),
            original_dropped
        );
    }
}

#[test]
fn strict_rollback_is_allocation_free_and_preserves_retained_storage() {
    let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
    let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
    let prepared_data = prepared(
        &y,
        &[x.clone(), x],
        &[false, false],
        None,
        true,
        TestWeights::Unit,
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let mut session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 7),
        )
        .expect("session");
        session.solve_fresh().expect("initial canonical fit");
        let original_dense_identity = session.retained_dense_identity();
        let original_scratch_identity = session
            .retained_scratch_identity()
            .map(std::path::Path::to_path_buf);
        let mut strict = session.strict_candidate().expect("speculative strict fit");
        assert!(strict.candidate().frame().finite_rank() > 0);
        assert!(strict.inject_same_rank_basis_pivot());
        let before_rollback = strict.workspace_capacities();
        strict.rollback();
        assert_eq!(
            session.workspace_capacities(),
            before_rollback,
            "rollback must not grow any session workspace"
        );
        assert_eq!(session.retained_dense_identity(), original_dense_identity);
        assert_eq!(
            session.retained_scratch_identity(),
            original_scratch_identity.as_deref()
        );
        assert!(session.output().is_ok());
        assert!(session
            .solve_frozen()
            .expect("rolled-back state must remain usable")
            .certified());
    }
}

#[test]
fn strided_session_cache_compacts_in_place_and_reports_retained_capacity() {
    let x1 = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
    let x2 = vec![3.0, -4.0, 1.5, 2.0, -0.5, 5.0];
    let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
    let weights = vec![1.0, 4.0, 2.0, 9.0, 3.0, 16.0];
    let prepared_data = prepared(
        &y,
        &[x1, x2],
        &[false, false],
        None,
        false,
        TestWeights::Analytic(&weights),
    );
    let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
    let session = CertifiedLinearSession::new(
        &prepared_data,
        frame,
        PreparedOlsFitConfig::default(),
        LinearSessionGenerations::new(0, 0, 0, 0, 0),
    )
    .expect("session");
    assert!(!session.uses_ppml_categorical_map_for_test());
    let rows = prepared_data.fit_sample().n_active();
    let stride = prepared_data.fit_model().columns.len() + 1;
    let logical_elements = rows * stride;
    let expected = vec![1.25, -2.5, 3.75, -4.0, 5.5, 6.25];
    let mut values = vec![0.0; logical_elements];
    for (row, expected) in expected.iter().copied().enumerate() {
        values[row * stride] = row as f64 + 1.0;
        values[row * stride + 1] = 100.0 + row as f64;
        values[row * stride + 2] = expected;
    }
    let source_capacity = values.capacity();
    assert!(source_capacity >= logical_elements);
    let cache = PreparedOlsFastScoreCache::Selected {
        generated_columns: vec![0, 1],
        within: DenseMatrix::from_row_major(rows, stride, values).expect("strided cache"),
        means: vec![0.0, 0.0],
    };
    let (memory, pointer_preserved, compacted) = session
        .compact_fast_cache_memory_for_test(cache, &[1])
        .expect("in-place strided compaction");
    assert!(
        pointer_preserved,
        "strided compaction allocated a second design"
    );
    assert_eq!(memory.rows, rows);
    assert_eq!(memory.columns, 1);
    assert_eq!(compacted, expected);
    assert_eq!(memory.resident_bytes, source_capacity as u128 * 8);
    assert!(
        memory.resident_bytes > (rows * memory.columns * 8) as u128,
        "telemetry must report retained capacity, not truncated logical length"
    );
}

#[test]
fn no_fe_bread_is_the_exact_canonical_inverse_crossproduct() {
    let x = vec![-1.0, 0.0, 1.0];
    let y = x.iter().map(|value| 2.0 + 3.0 * value).collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        None,
        true,
        TestWeights::Unit,
    );
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("exact no-FE bread");
    assert_eq!(fit.bread().dimension(), 2);
    assert_eq!(
        fit.bread().coordinates()[0].kind,
        PreparedOlsBreadCoordinateKind::FiniteRegressor
    );
    assert_eq!(
        fit.bread().coordinates()[1].kind,
        PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
    );
    assert_close(fit.bread().values()[0], 0.5, 1e-12);
    assert_close(fit.bread().values()[1], 0.0, 1e-12);
    assert_close(fit.bread().values()[2], 0.0, 1e-12);
    assert_close(fit.bread().values()[3], 1.0 / 3.0, 1e-12);
    assert_eq!(fit.bread().weight_scale().weight_kind, None);
    assert_close(fit.bread().weight_scale().reference_weight_sum, 3.0, 0.0);
    assert!(fit.bread().certificate().passed);
}

#[test]
fn absorbed_intercept_and_bread_follow_reference_weight_normalization() {
    let g = vec![1.0, 1.0, 2.0, 2.0];
    let x = vec![-1.0, 1.0, -2.0, 2.0];
    let y = x
        .iter()
        .zip(&g)
        .map(|(value, group)| 2.0 * value + if *group == 1.0 { 5.0 } else { -1.0 })
        .collect::<Vec<_>>();
    let raw_weights = vec![1.0, 3.0, 2.0, 4.0];
    let analytic = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        true,
        TestWeights::Analytic(&raw_weights),
    );
    let analytic = fit_prepared_model_coefficients(&analytic, PreparedOlsFitConfig::default())
        .expect("analytic normalized intercept and bread");
    assert_eq!(analytic.finite_rank(), 1);
    assert_close(analytic.coefficients()[0].value.expect("slope"), 2.0, 1e-11);
    assert_eq!(
        analytic.coefficients()[1].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_close(
        analytic.coefficients()[1].value.expect("intercept"),
        1.4,
        1e-11,
    );
    assert_close(analytic.bread().values()[0], 15.0 / 146.0, 1e-11);
    assert_close(analytic.bread().values()[1], -9.0 / 146.0, 1e-11);
    assert_close(analytic.bread().values()[2], -9.0 / 146.0, 1e-11);
    assert_close(analytic.bread().values()[3], 419.0 / 1460.0, 1e-11);
    let analytic_scale = analytic.bread().weight_scale();
    assert_eq!(analytic_scale.weight_kind, Some(WeightKind::Analytic));
    assert_close(analytic_scale.solver_weight_divisor, 4.0, 0.0);
    assert_close(analytic_scale.reference_weight_multiplier, 1.6, 1e-12);
    assert_close(analytic_scale.reference_weight_sum, 4.0, 0.0);

    let scaled_weights = raw_weights
        .iter()
        .map(|weight| weight * 1e200)
        .collect::<Vec<_>>();
    let scaled = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        true,
        TestWeights::Analytic(&scaled_weights),
    );
    let scaled = fit_prepared_model_coefficients(&scaled, PreparedOlsFitConfig::default())
        .expect("rescaled analytic normalized intercept and bread");
    for (left, right) in analytic
        .bread()
        .values()
        .iter()
        .zip(scaled.bread().values())
    {
        assert_close(*left, *right, 2e-11);
    }

    let probability = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        true,
        TestWeights::Probability(&raw_weights),
    );
    let probability =
        fit_prepared_model_coefficients(&probability, PreparedOlsFitConfig::default())
            .expect("probability normalized intercept and bread");
    assert_eq!(
        probability.bread().weight_scale().weight_kind,
        Some(WeightKind::Probability)
    );
    for (analytic, probability) in analytic
        .bread()
        .values()
        .iter()
        .zip(probability.bread().values())
    {
        assert_close(*analytic, *probability, 2e-11);
    }

    let frequencies = vec![1.0, 3.0, 2.0, 4.0];
    let frequency = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        true,
        TestWeights::Frequency(&frequencies),
    );
    let frequency = fit_prepared_model_coefficients(&frequency, PreparedOlsFitConfig::default())
        .expect("frequency normalized intercept and bread");
    assert_close(frequency.bread().values()[0], 3.0 / 73.0, 1e-11);
    assert_close(frequency.bread().values()[1], -9.0 / 365.0, 1e-11);
    assert_close(frequency.bread().values()[3], 419.0 / 3650.0, 1e-11);
    assert_close(
        frequency.bread().weight_scale().reference_weight_sum,
        10.0,
        0.0,
    );
    let doubled = frequencies
        .iter()
        .map(|value| 2.0 * value)
        .collect::<Vec<_>>();
    let doubled = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        true,
        TestWeights::Frequency(&doubled),
    );
    let doubled = fit_prepared_model_coefficients(&doubled, PreparedOlsFitConfig::default())
        .expect("doubled frequency bread");
    for (baseline, doubled) in frequency
        .bread()
        .values()
        .iter()
        .zip(doubled.bread().values())
    {
        assert_close(*doubled, *baseline / 2.0, 2e-11);
    }
}

#[test]
fn normalized_intercept_does_not_add_finite_or_absorbed_rank() {
    let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
    let y = vec![2.0, 4.0, -1.0, 1.0, 7.0, 9.0];
    let prepared = prepared(&y, &[], &[], Some(&g), true, TestWeights::Unit);
    let absorbed = super::super::compute_prepared_ols_absorbed_dof(
        &prepared,
        super::super::PreparedOlsAbsorbedDofPolicy::Pairwise,
    )
    .expect("absorbed rank");
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("FE-only fit");
    assert_eq!(fit.finite_rank(), 0);
    assert_eq!(absorbed.absorbed_rank, 3);
    assert_eq!(fit.bread().dimension(), 1);
    assert_close(fit.bread().values()[0], 1.0 / 6.0, 1e-12);
    assert_eq!(
        fit.coefficients()[0].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_close(
        fit.coefficients()[0].value.expect("normalized intercept"),
        3.0 + 2.0 / 3.0,
        1e-12,
    );
    assert!(fit.kept_generated_columns().is_empty());
    assert!(fit.dropped_generated_columns().is_empty());
    assert_eq!(
        fit.bread().coordinates()[0].generated_column_index,
        fit.coefficients()[0]
            .generated_column_index
            .expect("reported intercept generated column")
    );
}

#[test]
fn rank_zero_without_finite_columns_is_certified() {
    let y = vec![1.0, -2.0, 3.0, 0.5];
    let prepared = prepared(&y, &[], &[], None, false, TestWeights::Unit);
    let fit = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("rank-zero fit");
    assert_eq!(fit.finite_rank(), 0);
    assert!(fit.coefficients().is_empty());
    assert!(fit.kept_generated_columns().is_empty());
    assert!(fit.dropped_generated_columns().is_empty());
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    assert_eq!(
        fit.residuals(),
        y.iter().map(|value| value - mean).collect::<Vec<_>>()
    );
    assert_eq!(fit.score_certificate().moments, 0);
}

#[test]
fn rank_zero_fast_path_matches_low_memory_weighted_fe_fit() {
    let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 3.0];
    let y = vec![2.0, 4.0, -1.0, 1.0, 7.0, 9.0, 8.0];
    let weights = vec![0.5, 2.0, 1.5, 0.75, 3.0, 1.0, 2.5];
    let prepared = prepared(
        &y,
        &[],
        &[],
        Some(&g),
        true,
        TestWeights::Analytic(&weights),
    );
    let fast = fit_prepared_model_coefficients(&prepared, PreparedOlsFitConfig::default())
        .expect("fast weighted FE-only fit");
    let low = fit_prepared_model_coefficients(
        &prepared,
        PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode: PreparedOlsExecutionMode::Low,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        },
    )
    .expect("low-memory weighted FE-only fit");
    assert_eq!(fast.finite_rank(), 0);
    assert_eq!(fast.coefficients(), low.coefficients());
    for (actual, expected) in fast.residuals().iter().zip(low.residuals()) {
        assert_close(*actual, *expected, 2e-12);
    }
    for (actual, expected) in fast.bread().values().iter().zip(low.bread().values()) {
        assert_close(*actual, *expected, 2e-12);
    }
    assert!(fast.rank_certificate().passed);
    assert!(fast.score_certificate().passed);
}

#[test]
fn strided_score_cache_matches_packed_cache_across_weight_semantics() {
    let group = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
    let x1 = vec![-2.0, 0.5, 1.0, 3.0, -1.5, 2.0, 0.25, 4.0];
    let x2 = vec![1.0, -3.0, 0.75, 2.5, 4.0, -0.5, 3.0, 1.5];
    let weights = vec![1.0, 2.0, 3.0, 4.0, 2.0, 1.0, 3.0, 2.0];
    let y = x1
        .iter()
        .zip(&x2)
        .zip(&group)
        .map(|((left, right), group)| {
            1.75 * left - 0.4 * right + if *group <= 2.0 { 2.0 } else { -1.0 }
        })
        .collect::<Vec<_>>();
    for test_weights in [
        TestWeights::Unit,
        TestWeights::Analytic(&weights),
        TestWeights::Probability(&weights),
        TestWeights::Frequency(&weights),
    ] {
        let prepared = prepared(
            &y,
            &[x1.clone(), x2.clone()],
            &[false, false],
            Some(&group),
            true,
            test_weights,
        );
        assert_packed_and_strided_score_caches_match(&prepared);
    }
    let no_intercept = prepared(
        &y,
        &[x1, x2],
        &[false, false],
        None,
        false,
        TestWeights::Unit,
    );
    assert_packed_and_strided_score_caches_match(&no_intercept);
}

#[test]
fn packed_score_handoff_selects_arbitrary_basis_without_reallocating_rows() {
    let make_cache = || {
        let mut values = Vec::with_capacity(40);
        values.extend([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        PreparedOlsFastScoreCache::Packed {
            generated_columns: vec![0, 2, 5, 9],
            within: DenseMatrix::from_row_major(2, 4, values).expect("matrix"),
            means: vec![-0.0, 0.25, -2.0, 3.0],
        }
    };
    let original = make_cache();
    let PreparedOlsFastScoreCache::Packed { within, .. } = &original else {
        unreachable!();
    };
    let pointer = within.as_slice().as_ptr();
    let capacity = within.storage_capacity();
    let mut reservation = InjectedReservation::default();
    let selected = original
        .into_kept(&[9, 0, 5], &mut reservation)
        .expect("selected packed bank");
    let PreparedOlsFastScoreCache::Packed {
        generated_columns,
        within,
        means,
    } = selected
    else {
        unreachable!();
    };
    assert_eq!(generated_columns, [9, 0, 5]);
    assert_eq!(within.as_slice(), [4.0, 1.0, 3.0, 8.0, 5.0, 7.0]);
    assert_eq!(within.as_slice().as_ptr(), pointer);
    assert_eq!(within.storage_capacity(), capacity);
    assert_eq!(
        means.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        [
            3.0_f64.to_bits(),
            (-0.0_f64).to_bits(),
            (-2.0_f64).to_bits()
        ]
    );
    assert_eq!(
        reservation.records,
        [(3, "unable to reserve accepted packed score selection")]
    );
    let mut no_alloc = InjectedReservation {
        fail_at: Some(1),
        ..Default::default()
    };
    make_cache()
        .into_kept(&[0, 2, 5, 9], &mut no_alloc)
        .expect("unchanged bank");
    assert_eq!(no_alloc.calls, 0);
    for kept in [&[2, 2][..], &[2, 7][..]] {
        assert_eq!(
            make_cache()
                .into_kept(kept, &mut SystemFitReservation)
                .expect_err("invalid selection")
                .code,
            ErrorCode::InvalidData
        );
    }
    for mut refusal in [
        InjectedReservation {
            fail_at: Some(1),
            ..Default::default()
        },
        InjectedReservation {
            undersize: true,
            ..Default::default()
        },
    ] {
        assert_eq!(
            make_cache()
                .into_kept(&[9, 0, 5], &mut refusal)
                .expect_err("selection allocation refusal")
                .code,
            ErrorCode::Allocation
        );
    }
}

#[test]
fn allocation_capacity_and_score_failures_are_fatal() {
    let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    let y = vec![1.2, -0.5, 0.4, 3.1, 4.0, 7.2];
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        None,
        true,
        TestWeights::Unit,
    );
    let mut successful = InjectedReservation::default();
    fit_with_controls(
        &prepared,
        PreparedOlsFitConfig::default(),
        &mut successful,
        false,
        false,
    )
    .expect("control fit");
    assert!(successful.calls >= 9);
    for fail_at in 1..=successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        let error = fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut injected,
            false,
            false,
        )
        .expect_err("injected allocation failure");
        assert_eq!(error.code, ErrorCode::Allocation, "failure {fail_at}");
    }
    let mut cached_successful = InjectedReservation::default();
    let cached = fit_with_controls_internal_mode(
        &prepared,
        PreparedOlsFitConfig::default(),
        &mut cached_successful,
        FitPerturbations::default(),
        true,
        MAX_FAST_SCORE_CACHE_BYTES,
    )
    .expect("fast inference-cache control fit");
    assert!(cached.fast_score_cache.is_some());
    assert!(cached_successful.calls >= 9);
    for fail_at in 1..=cached_successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        let error = fit_with_controls_internal_mode(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut injected,
            FitPerturbations::default(),
            true,
            MAX_FAST_SCORE_CACHE_BYTES,
        )
        .expect_err("injected fast inference-cache allocation failure");
        assert_eq!(
            error.code,
            ErrorCode::Allocation,
            "cached failure {fail_at}"
        );
    }
    let mut strided_successful = InjectedReservation::default();
    let strided = fit_with_controls_internal_mode(
        &prepared,
        PreparedOlsFitConfig::default(),
        &mut strided_successful,
        FitPerturbations::default(),
        true,
        0,
    )
    .expect("strided inference-cache control fit");
    assert!(matches!(
        strided.fast_score_cache,
        Some(PreparedOlsFastScoreCache::Selected { .. })
    ));
    for fail_at in 1..=strided_successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        let error = fit_with_controls_internal_mode(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut injected,
            FitPerturbations::default(),
            true,
            0,
        )
        .expect_err("injected strided inference-cache allocation failure");
        assert_eq!(
            error.code,
            ErrorCode::Allocation,
            "strided failure {fail_at}"
        );
    }
    let mut undersized = InjectedReservation {
        undersize: true,
        ..InjectedReservation::default()
    };
    assert_eq!(
        fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut undersized,
            false,
            false,
        )
        .expect_err("undersized reservation")
        .code,
        ErrorCode::Allocation
    );
    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            true,
            false,
        )
        .expect_err("perturbed coefficient must fail score certificate")
        .code,
        ErrorCode::InvalidData
    );
    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls_internal(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            FitPerturbations {
                finite_bread: true,
                ..FitPerturbations::default()
            },
        )
        .expect_err("perturbed bread must fail its inverse certificate")
        .code,
        ErrorCode::InvalidData
    );
    assert_eq!(
        checked_square(
            usize::MAX,
            "injected prepared OLS square-dimension overflow"
        )
        .expect_err("square dimensions must use checked arithmetic")
        .code,
        ErrorCode::Allocation
    );
}

#[test]
fn low_guarded_fit_allocation_and_certificate_failures_are_fatal() {
    let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    let y = vec![1.2, -0.5, 0.4, 3.1, 4.0, 7.2];
    let data = prepared(&y, &[x], &[false], None, true, TestWeights::Unit);
    let config = PreparedOlsFitConfig {
        execution: PreparedOlsExecutionConfig {
            mode: PreparedOlsExecutionMode::Low,
            poolsize: 1,
            ..PreparedOlsExecutionConfig::default()
        },
        ..PreparedOlsFitConfig::default()
    };
    let mut successful = InjectedReservation::default();
    fit_with_controls(&data, config, &mut successful, false, false).expect("low guarded control");
    for fail_at in 1..=successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        let error = fit_with_controls(&data, config, &mut injected, false, false)
            .expect_err("low guarded allocation failure");
        assert_eq!(error.code, ErrorCode::Allocation, "failure {fail_at}");
    }
    for perturbations in [
        FitPerturbations {
            first_coefficient: true,
            ..FitPerturbations::default()
        },
        FitPerturbations {
            finite_bread: true,
            ..FitPerturbations::default()
        },
    ] {
        let error =
            fit_with_controls_internal(&data, config, &mut SystemFitReservation, perturbations)
                .expect_err("low guarded corruption must fail certification");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }
}

#[test]
fn reported_intercept_certificate_fails_closed_before_fit_publication() {
    let g = (0..64).map(|row| (row / 8) as f64).collect::<Vec<_>>();
    let x = (0..64)
        .map(|row| (row % 8) as f64 - 3.5)
        .collect::<Vec<_>>();
    let y = x
        .iter()
        .zip(&g)
        .map(|(value, group)| 2.0 * value + 0.75 * group - 1.0)
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        true,
        TestWeights::Unit,
    );
    let mut successful = InjectedReservation::default();
    fit_with_controls_internal(
        &prepared,
        PreparedOlsFitConfig::default(),
        &mut successful,
        FitPerturbations::default(),
    )
    .expect("absorbed-intercept allocation control");
    let covariance_bound = checked_square(
        prepared.model.columns.len(),
        "test prepared OLS covariance bound overflow",
    )
    .expect("small covariance bound");
    assert!(prepared.sample.n_active() > covariance_bound);
    let covariance_records = successful
        .records
        .iter()
        .filter(|(_, label)| {
            label.contains("Gram")
                || label.contains("bread")
                || label.contains("inverse R")
                || label.contains("reported-intercept")
        })
        .collect::<Vec<_>>();
    assert!(!covariance_records.is_empty());
    assert!(covariance_records
        .iter()
        .all(|(elements, _)| *elements <= covariance_bound));
    for (label, elements) in [
        ("unable to reserve prepared OLS reported bread", 4),
        ("unable to reserve prepared OLS reported-intercept means", 1),
        (
            "unable to reserve prepared OLS reported-intercept bread side",
            1,
        ),
    ] {
        assert!(successful.records.contains(&(elements, label)));
    }
    for fail_at in 1..=successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        let error = fit_with_controls_internal(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut injected,
            FitPerturbations::default(),
        )
        .expect_err("absorbed-intercept injected allocation failure");
        assert_eq!(error.code, ErrorCode::Allocation, "failure {fail_at}");
    }

    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls_internal(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            FitPerturbations {
                reported_intercept: true,
                ..FitPerturbations::default()
            },
        )
        .expect_err("perturbed reported intercept must not publish a partial fit")
        .code,
        ErrorCode::InvalidData
    );

    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls_internal(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            FitPerturbations {
                reported_bread_extension: true,
                ..FitPerturbations::default()
            },
        )
        .expect_err("perturbed reported-bread extension must fail its inverse certificate")
        .code,
        ErrorCode::InvalidData
    );
}

#[test]
fn fixed_effect_only_outcome_scale_cannot_mask_score_failure() {
    let g = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
    let x = vec![-2.0, -0.5, 1.0, 2.5, -1.5, 0.0, 0.5, 1.0];
    let y = x
        .iter()
        .zip(&g)
        .map(|(x, group)| {
            let fixed_effect = if *group == 1.0 { 1e10 } else { -1e10 };
            fixed_effect + 2.5 * x
        })
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        false,
        TestWeights::Unit,
    );
    let mut control_reservation = InjectedReservation::default();
    let control = fit_with_controls(
        &prepared,
        PreparedOlsFitConfig::default(),
        &mut control_reservation,
        false,
        false,
    )
    .expect("unperturbed large-FE-outcome fit");
    assert_close(
        control.coefficients()[0].value.expect("control slope"),
        2.5,
        2e-6,
    );
    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            true,
            false,
        )
        .expect_err("FE-only outcome scale must not hide a perturbed coefficient")
        .code,
        ErrorCode::InvalidData
    );
}

#[test]
fn fixed_effect_only_regressor_scale_cannot_mask_score_failure() {
    let g = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
    let within_x = [-2.0, -0.5, 1.0, 2.5, -1.5, 0.0, 0.5, 1.0];
    let x = within_x
        .iter()
        .zip(&g)
        .map(|(within, group)| {
            // Large enough to exercise scale separation, but outside the
            // pinned squared 1e-9 FE-collinearity cutoff so this remains
            // an identified-coordinate score-certificate mutation test.
            let fixed_effect = if *group == 1.0 { 1e4 } else { -1e4 };
            fixed_effect + within
        })
        .collect::<Vec<_>>();
    let y = within_x
        .iter()
        .map(|within| 100.0 * within)
        .collect::<Vec<_>>();
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        Some(&g),
        false,
        TestWeights::Unit,
    );
    let mut control_reservation = InjectedReservation::default();
    let control = fit_with_controls(
        &prepared,
        PreparedOlsFitConfig::default(),
        &mut control_reservation,
        false,
        false,
    )
    .expect("unperturbed large-FE-regressor fit");
    assert_close(
        control.coefficients()[0].value.expect("control slope"),
        100.0,
        2e-8,
    );
    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            true,
            false,
        )
        .expect_err("FE-only regressor scale must not hide a perturbed coefficient")
        .code,
        ErrorCode::InvalidData
    );
}

#[test]
fn zero_within_norm_column_still_checks_its_raw_score_moment() {
    let g = vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
    let within_y = [-1.0, 0.0, 1.0, -2.0, 0.5, 1.5];
    let y = within_y
        .iter()
        .zip(&g)
        .map(|(within, group)| 10.0 * group + within)
        .collect::<Vec<_>>();
    let fe_prepared = prepared(&y, &[], &[], Some(&g), true, TestWeights::Unit);
    let mut control_reservation = InjectedReservation::default();
    let control = fit_with_controls(
        &fe_prepared,
        PreparedOlsFitConfig::default(),
        &mut control_reservation,
        false,
        false,
    )
    .expect("unperturbed deferred-intercept fit");
    assert_eq!(control.score_certificate().moments, 1);
    assert_eq!(
        control.coefficients()[0].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_close(
        control.coefficients()[0]
            .value
            .expect("normalized intercept"),
        weighted_mean_slice(&y, normalized_weights(&fe_prepared).expect("weights")).expect("mean"),
        1e-10,
    );

    // A structurally absorbed intercept can acquire a tiny nonzero within
    // norm from iterative MAP roundoff. Its score remains a raw intercept
    // moment, not a moment normalized by that numerical noise.
    let weights = normalized_weights(&fe_prepared).expect("weights");
    let rhs = fe_prepared
        .fit_build_rhs_with_response(&fe_prepared.fit_sample().outcome)
        .expect("rhs");
    let reference_norm =
        weighted_matrix_column_norm(&rhs, 1, weights).expect("reference intercept norm");
    let mut roundoff_residuals = control.residuals().to_vec();
    for residual in &mut roundoff_residuals {
        *residual += 1e-14;
    }
    let roundoff = certify_scores(
        &fe_prepared,
        &roundoff_residuals,
        weights,
        &[reference_norm],
        &[reference_norm * 1e-14],
        FiniteScoreDraft::default(),
        &[PreparedOlsFitStatus::NormalizedAbsorbedIntercept],
        1.0,
        PreparedOlsFitConfig::default().score_tolerance,
    )
    .expect("roundoff score certificate");
    assert!(roundoff.passed);
    assert!(roundoff.relative_infinity_norm < 1e-12);
    // Projection-only fits (separation detectors) leave the absorbed
    // intercept deferred; it is the same structural coordinate and keeps the
    // same raw-norm moment.
    let deferred = certify_scores(
        &fe_prepared,
        &roundoff_residuals,
        weights,
        &[reference_norm],
        &[reference_norm * 1e-14],
        FiniteScoreDraft::default(),
        &[PreparedOlsFitStatus::AbsorbedInterceptDeferred],
        1.0,
        PreparedOlsFitConfig::default().score_tolerance,
    )
    .expect("deferred-intercept roundoff score certificate");
    assert!(deferred.passed);
    assert_eq!(
        deferred.relative_infinity_norm.to_bits(),
        roundoff.relative_infinity_norm.to_bits()
    );

    // Exact-zero outcomes and residuals have a mathematically exact zero
    // score even though no positive response normalization exists. A
    // nonzero finite-coordinate score under the same response must remain
    // fatal instead of being hidden by a numerical floor.
    let zero_outcome = vec![0.0; 6];
    let varying = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    let zero_response = prepared(
        &zero_outcome,
        &[varying],
        &[false],
        None,
        false,
        TestWeights::Unit,
    );
    let zero_weights = normalized_weights(&zero_response).expect("zero-response weights");
    let exact_zero = certify_scores(
        &zero_response,
        &zero_outcome,
        zero_weights,
        &[1.0],
        &[1.0],
        FiniteScoreDraft {
            infinity_scale: 0.0,
            square_sum_scale: 0.0,
            moments: 1,
        },
        &[PreparedOlsFitStatus::Kept],
        0.0,
        PreparedOlsFitConfig::default().score_tolerance,
    )
    .expect("exact-zero response score certificate");
    assert!(exact_zero.passed);
    assert_eq!(exact_zero.relative_infinity_norm, 0.0);
    assert_eq!(exact_zero.relative_two_norm, 0.0);

    let nonzero_error = certify_scores(
        &zero_response,
        &zero_outcome,
        zero_weights,
        &[1.0],
        &[1.0],
        FiniteScoreDraft {
            infinity_scale: f64::MIN_POSITIVE,
            square_sum_scale: 0.0,
            moments: 1,
        },
        &[PreparedOlsFitStatus::Kept],
        0.0,
        PreparedOlsFitConfig::default().score_tolerance,
    )
    .expect_err("nonzero score under an exact-zero response must fail");
    assert_eq!(nonzero_error.code, ErrorCode::InvalidData);

    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls(
            &fe_prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            false,
            true,
        )
        .expect_err("a deferred intercept must retain its raw score moment")
        .code,
        ErrorCode::InvalidData
    );
}

#[test]
fn empty_sample_and_invalid_policy_fail_closed() {
    let empty = prepared(&[], &[], &[], None, false, TestWeights::Unit);
    assert_eq!(
        fit_prepared_model_coefficients(&empty, PreparedOlsFitConfig::default())
            .expect_err("empty fit")
            .code,
        ErrorCode::InsufficientObservations
    );
    let x = vec![0.0, 1.0, 2.0];
    let y = vec![1.0, 2.0, 3.0];
    let prepared = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        None,
        true,
        TestWeights::Unit,
    );
    let default = PreparedOlsFitConfig::default();
    let invalid = PreparedOlsFitConfig {
        score_tolerance: 1.0,
        ..default
    };
    assert_eq!(
        fit_prepared_model_coefficients(&prepared, invalid)
            .expect_err("invalid policy")
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn corrupted_structural_intercept_metadata_fails_before_map_and_dof() {
    let x = vec![0.0, 1.0, 2.0];
    let y = vec![1.0, 2.0, 3.0];
    let mut fit_corrupted = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        None,
        true,
        TestWeights::Unit,
    );
    assert!(fit_corrupted.structural_intercept);
    assert!(fit_corrupted.report_intercept);
    fit_corrupted.structural_intercept = false;

    let mut reservation = InjectedReservation::default();
    assert_eq!(
        fit_with_controls(
            &fit_corrupted,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            false,
            false,
        )
        .expect_err("corrupted structural-intercept metadata must fail")
        .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        reservation.calls, 0,
        "structural-intercept corruption must fail before MAP or fit allocation"
    );

    // A hidden no-absorb constant is mandatory for the ordinary prepared
    // path.  Pure slope-only categorical models use their separate
    // topology-aware path and legitimately carry neither structural nor
    // reported intercept metadata.
    let mut dof_corrupted = prepared(
        &y,
        std::slice::from_ref(&x),
        &[false],
        None,
        false,
        TestWeights::Unit,
    );
    assert!(dof_corrupted.structural_intercept);
    assert!(!dof_corrupted.report_intercept);
    dof_corrupted.structural_intercept = false;
    assert_eq!(
        super::super::compute_prepared_ols_absorbed_dof(
            &dof_corrupted,
            super::super::PreparedOlsAbsorbedDofPolicy::Pairwise,
        )
        .expect_err("corrupted structural-intercept metadata must fail DoF")
        .code,
        ErrorCode::InvalidArgument
    );
}

/// Independent per-column reference for the blocked raw-moment kernel.
fn fill_raw_moment_cache_columnwise(
    raw: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    weight_sum: f64,
    scales: &mut [f64],
    means: &mut [f64],
    norms: &mut [f64],
) -> Result<()> {
    if scales.len() != raw.ncols()
        || means.len() != raw.ncols()
        || norms.len() != raw.ncols()
        || !weight_sum.is_finite()
        || weight_sum <= 0.0
    {
        return Err(invalid_argument(
            "prepared OLS columnwise raw-moment dimensions are inconsistent",
        ));
    }
    crate::parallel::stage_try_for_each(
        scales
            .par_iter_mut()
            .zip(means.par_iter_mut())
            .zip(norms.par_iter_mut())
            .enumerate(),
        raw.nrows() as u128 * raw.ncols() as u128,
        |(column, ((scale, mean), norm))| -> Result<()> {
            let mut norm_scale = 0.0_f64;
            let mut norm_sum = 1.0_f64;
            for row in 0..raw.nrows() {
                let value = raw[(row, column)];
                if !value.is_finite() {
                    return Err(invalid_data("prepared OLS raw-moment value is not finite"));
                }
                *scale = scale.max(value.abs());
                update_scaled_sum_of_squares(
                    weights.root(row)? * value,
                    &mut norm_scale,
                    &mut norm_sum,
                )?;
            }
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            if *scale != 0.0 {
                for row in 0..raw.nrows() {
                    compensated_add(
                        weights.value(row)? * (raw[(row, column)] / *scale),
                        &mut sum,
                        &mut correction,
                    )?;
                }
            }
            *mean = if *scale == 0.0 {
                0.0
            } else {
                *scale * ((sum + correction) / weight_sum)
            };
            if !mean.is_finite() {
                return Err(invalid_data("prepared OLS raw-moment mean is not finite"));
            }
            *norm = checked_norm(norm_scale, norm_sum)?;
            Ok(())
        },
    )
}
