//! Caller-level witnesses. Equal-weight opposite pairs make the within result
//! known algebraically, independently of every iterative solver under test.
use super::*;
use crate::data::DenseMatrix;
use crate::fe::routing_observer::ObserverRef;
use crate::fe::session::FeSession;
use crate::fe::{BorrowedMapWeights, FeSolverKind, LsmrPreconditioner, RoutingPreset, SolveReport};
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder,
    ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent,
    TermId, TermOrder, TimeSeriesState,
};
use crate::ols::{
    CertifiedPreparedOlsFit, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
    OlsRequestV1, PreparedOlsExecutionMode, PreparedOlsFitConfig,
};
use crate::{
    fit_prepared_model_coefficients, prepare_model, PreparedOlsData, ThreadConfig, ThreadTeam,
};

struct PairedCells {
    y: Vec<f64>,
    x: [Vec<f64>; 2],
    effects: [Vec<f64>; 3],
    error: Vec<f64>,
}

impl PairedCells {
    fn new(rectangle: bool) -> Self {
        let mut result = Self {
            y: Vec::new(),
            x: [Vec::new(), Vec::new()],
            effects: [Vec::new(), Vec::new(), Vec::new()],
            error: Vec::new(),
        };
        let (components, levels, repeats) = if rectangle { (1, 4, 64) } else { (2, 128, 8) };
        for component in 0..components {
            for left in 0..levels {
                let degree = if rectangle {
                    2
                } else if left == 0 {
                    8
                } else {
                    2
                };
                for neighbor in 0..degree {
                    let first = component * levels + left;
                    let second = if rectangle {
                        neighbor
                    } else {
                        component * levels + (left + neighbor) % levels
                    };
                    for repeat in 0..repeats {
                        for sign in [-1., 1.] {
                            // The three Walsh contrasts are mutually orthogonal
                            // within each cell. Every FE moment is exactly zero.
                            let x1 = sign;
                            let x2 = sign * if repeat % 2 == 0 { -1. } else { 1. };
                            let error = sign * if repeat % 4 < 2 { -0.125 } else { 0.125 };
                            let fe = (((first * 17 + 3) % 37) as f64 - 18.) / 8.
                                + (((second * 13 + 5) % 43) as f64 - 21.) / 16.;
                            result.y.push(2. * x1 - 0.5 * x2 + error + fe);
                            result.x[0].push(x1);
                            result.x[1].push(x2);
                            result.error.push(error);
                            result.effects[0].push(first as f64);
                            result.effects[1].push(second as f64);
                            result.effects[2].push((repeat % 2) as f64);
                        }
                    }
                }
            }
        }
        result
    }

    fn prepared(&self, effects: usize) -> PreparedOlsData {
        let loaded = |values: &[f64]| LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        };
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
        for (index, values) in self.x.iter().chain(&self.effects[..effects]).enumerate() {
            let id = (index + 1) as u32;
            let source = SourceId::new(id).unwrap();
            sources.push(SourceEntry {
                id: source,
                canonical_name: format!("v{id}"),
            });
            numeric_sources.push((source, loaded(values)));
            bindings.push(OlsInputBinding {
                stata_var_position: (bindings.len() + 1) as u32,
                role: OlsInputRole::NumericSource(source),
            });
            if index < 2 {
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
            } else {
                absorbed_terms.push(AbsorbedTerm {
                    id: AbsorbTermId::new(id - 2).unwrap(),
                    intercept: true,
                    categorical_sources: vec![source],
                    slope_sources: Vec::new(),
                    label: format!("v{id}"),
                    saved_name: String::new(),
                });
            }
        }
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
                candidate: vec![true; self.y.len()].into(),
                outcome: loaded(&self.y),
                numeric_sources,
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
            true,
        )
        .unwrap()
    }

    fn packed(&self) -> DenseMatrix {
        DenseMatrix::from_columns(&[self.y.clone(), self.x[0].clone(), self.x[1].clone()]).unwrap()
    }

    fn check_projection(&self, values: &DenseMatrix) {
        assert_eq!((values.nrows(), values.ncols()), (self.y.len(), 3));
        for row in 0..values.nrows() {
            let expected = [
                2. * self.x[0][row] - 0.5 * self.x[1][row] + self.error[row],
                self.x[0][row],
                self.x[1][row],
            ];
            for column in 0..3 {
                assert!(
                    (values[(row, column)] - expected[column]).abs() <= 1e-10,
                    "analytic projection row={row} column={column}"
                );
            }
        }
    }

    fn check_fit(&self, fit: &CertifiedPreparedOlsFit) {
        assert!((fit.coefficients()[0].value.unwrap() - 2.).abs() <= 1e-10);
        assert!((fit.coefficients()[1].value.unwrap() + 0.5).abs() <= 1e-10);
        assert!(fit.absorption_report().certified());
        assert!(fit.residual_absorption_report().certified());
        for (&actual, &expected) in fit.residuals().iter().zip(&self.error) {
            assert!((actual - expected).abs() <= 1e-10);
        }
    }
}

fn config(preset: Option<RoutingPreset>, workers: usize) -> PreparedOlsFitConfig {
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.routing = preset;
    config.absorb.map.tolerance = 1e-12;
    config.absorb.map.max_iterations = 512;
    config.absorb.map.check_every = 1;
    config.execution.threads = ThreadConfig::from_requested(workers, true).unwrap();
    config
}

fn science(report: &SolveReport) -> SolveReport {
    let mut report = report.clone();
    report.setup_seconds = 0.;
    report.solve_seconds = 0.;
    report.certification_seconds = 0.;
    // The policy has additional admitted observation storage. Memory and clocks
    // are not scientific equality targets; every solver/certificate field is.
    report.solver_workspace_peak = crate::memory::SolverWorkspacePeak::ZERO;
    report
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn matrix_bits(values: &DenseMatrix) -> Vec<u64> {
    (0..values.nrows())
        .flat_map(|row| (0..values.ncols()).map(move |column| values[(row, column)].to_bits()))
        .collect()
}

fn assert_fit_exact(actual: &CertifiedPreparedOlsFit, expected: &CertifiedPreparedOlsFit) {
    assert_eq!(actual.coefficients(), expected.coefficients());
    assert_eq!(
        actual
            .coefficients()
            .iter()
            .map(|c| c.value.map(f64::to_bits))
            .collect::<Vec<_>>(),
        expected
            .coefficients()
            .iter()
            .map(|c| c.value.map(f64::to_bits))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        actual.kept_generated_columns(),
        expected.kept_generated_columns()
    );
    assert_eq!(
        actual.dropped_generated_columns(),
        expected.dropped_generated_columns()
    );
    assert_eq!(bits(actual.residuals()), bits(expected.residuals()));
    assert_eq!(actual.bread(), expected.bread());
    assert_eq!(actual.rank_certificate(), expected.rank_certificate());
    assert_eq!(actual.score_certificate(), expected.score_certificate());
    assert_eq!(
        science(actual.absorption_report()),
        science(expected.absorption_report())
    );
    assert_eq!(
        science(actual.residual_absorption_report()),
        science(expected.residual_absorption_report())
    );
}

#[test]
fn real_fast_and_low_callers_preserve_uncalibrated_arithmetic() {
    assert!(CALIBRATION.is_none());
    let fixture = PairedCells::new(false);
    let data = fixture.prepared(2);
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut options = config(None, 1);
        options.execution.mode = mode;
        options.execution.poolsize = 1; // Real Low outcome, two chunks and final action.
        let expected = fit_prepared_model_coefficients(&data, options).unwrap();
        fixture.check_fit(&expected);
        for preset in [
            RoutingPreset::remaining_two_control(None).unwrap(),
            RoutingPreset::remaining_two(),
        ] {
            options.absorb.routing = Some(preset);
            let actual = fit_prepared_model_coefficients(&data, options).unwrap();
            fixture.check_fit(&actual);
            assert_fit_exact(&actual, &expected);
        }
    }
}

#[test]
fn real_session_eligibility_requires_auto_auto_and_two_categorical_effects() {
    let fixture = PairedCells::new(true);
    for effects in [1, 2, 3] {
        let data = fixture.prepared(effects);
        for (solver, preconditioner) in [
            (FeSolverKind::Auto, LsmrPreconditioner::Auto),
            (FeSolverKind::Auto, LsmrPreconditioner::Off),
            (FeSolverKind::Map, LsmrPreconditioner::Auto),
        ] {
            let mut options = config(Some(RoutingPreset::remaining_two()), 1);
            options.absorb.solver = solver;
            options.absorb.lsmr.preconditioner = preconditioner;
            let result = FeSession::for_ols(&data, BorrowedMapWeights::Unit, options);
            if solver != FeSolverKind::Auto {
                assert_eq!(
                    result
                        .err()
                        .expect("explicit solver rejects hidden Auto control")
                        .code,
                    crate::error::ErrorCode::InvalidArgument
                );
                continue;
            }
            let session = result.unwrap();
            assert_eq!(
                session.remaining_state().is_some(),
                effects == 2
                    && solver == FeSolverKind::Auto
                    && preconditioner == LsmrPreconditioner::Auto
            );
        }
    }
}

fn trace() -> String {
    std::fs::read_to_string(std::env::var_os("FEREG_SOLVER_TRACE_FILE").unwrap())
        .unwrap_or_default()
}

#[test]
fn real_session_remaining_shell_has_an_exact_parent_admission_boundary() {
    let fixture = PairedCells::new(false);
    let data = fixture.prepared(2);
    let mut ordinary = config(None, 1);
    ordinary.absorb.retain_coefficients = false;
    // Obtain the same immutable binding, then independently ask its existing
    // owner plan for B. The successor adds one shell, before any observations.
    let design =
        crate::ols::prepared_fit_view::PreparedOlsFitView::fit_borrowed_intercept_map_design(&data)
            .unwrap()
            .unwrap();
    let base = crate::fe::ReusableCategoricalMapAbsorber::owner_bytes(&design, 3, 1).unwrap();
    let shell = State::shell_bytes();
    assert!(shell > 0);
    for (successor, usable, admitted) in [
        (false, base, true),
        (true, base + shell - 1, false),
        (true, base + shell, true),
    ] {
        let mut options = ordinary;
        options.absorb.routing =
            successor.then(|| RoutingPreset::remaining_two_control(None).unwrap());
        options.absorb.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 37,
            usable_bytes: usable + 37,
        });
        let session = FeSession::for_ols_rhs(&data, BorrowedMapWeights::Unit, options, 3).unwrap();
        assert_eq!(session.categorical.is_some(), admitted);
        assert_eq!(session.binding.is_some(), !admitted);
        assert_eq!(session.remaining_state().is_some(), successor && admitted);
        assert_eq!(
            session.peak.bytes().unwrap() as u128,
            if admitted {
                base + if successor { shell } else { 0 }
            } else {
                0
            }
        );
        if let Some(state) = session.remaining_state() {
            assert!(
                state.logs.is_empty(),
                "shell admission precedes optional banks"
            );
        }
    }
}

fn field<'a>(line: &'a str, key: &str) -> &'a str {
    line.split('|')
        .find_map(|part| {
            part.split_once('=')
                .filter(|(k, _)| *k == key)
                .map(|(_, value)| value)
        })
        .unwrap_or_else(|| panic!("missing {key}: {line}"))
}

#[test]
fn real_session_forced_routes_refusal_direct_and_cancellation_isolated() {
    const CHILD: &str = "FEREG_REMAINING_CALLER_WITNESS_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let path =
            std::env::temp_dir().join(format!("fereg-remaining-caller-{}.log", std::process::id()));
        for measured in [false, true] {
            let _ = std::fs::remove_file(&path);
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .env(CHILD, "1")
                .env("FEREG_SOLVER_TRACE", "1")
                .env("FEREG_SOLVER_TRACE_FILE", &path);
            if measured {
                command.env("FEREG_PHASE_TIMING", "1");
            } else {
                command.env_remove("FEREG_PHASE_TIMING");
            }
            let output = command
            .args(["fe::routing_remaining::integration_tests::real_session_forced_routes_refusal_direct_and_cancellation_isolated",
                "--exact", "--test-threads=1", "--nocapture"])
            .output().unwrap();
            let _ = std::fs::remove_file(&path);
            assert!(
                output.status.success(),
                "child failed: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    let fixture = PairedCells::new(false);
    let data = fixture.prepared(2);
    for workers in [1, 4] {
        for checkpoint in [16, 32] {
            let preset = RoutingPreset::remaining_two_control(Some(checkpoint)).unwrap();
            let options = config(Some(preset), workers);
            let run = || {
                ThreadTeam::new(options.execution.threads)
                    .install(|| {
                        let before = trace().len();
                        let mut session =
                            FeSession::for_ols(&data, BorrowedMapWeights::Unit, options).unwrap();
                        let mut state = session.remaining_state().unwrap();
                        let fit = session
                            .project_ols_with_remaining(
                                BorrowedMapWeights::Unit,
                                fixture.packed(),
                                options.absorb,
                                false,
                                false,
                                Some((
                                    ObserverRef::Remaining(&mut state),
                                    Demand::Packed { regressors: 2 },
                                )),
                            )
                            .unwrap();
                        fixture.check_projection(fit.residualized());
                        assert!(fit.report().certified());
                        let observed = trace();
                        let new = &observed[before..];
                        let routes: Vec<_> = new
                            .lines()
                            .filter(|line| line.contains("|phase=auto_schur_route|"))
                            .collect();
                        assert_eq!(routes.len(), 1, "{new}");
                        let route = routes[0];
                        assert_eq!(field(route, "iterations"), checkpoint.to_string());
                        assert_eq!(field(route, "checkpoint"), checkpoint.to_string());
                        assert_eq!(field(route, "installed"), "1");
                        assert_eq!(field(route, "refusal"), "None");
                        assert!(field(route, "levels").parse::<usize>().unwrap() > 0);
                        let grants: Vec<_> = new
                            .lines()
                            .filter(|line| {
                                line.contains("|phase=remaining_checkpoint|")
                                    && field(line, "build") == "1"
                            })
                            .collect();
                        assert_eq!(grants.len(), 1, "{new}");
                        assert_eq!(field(grants[0], "local"), checkpoint.to_string());
                        assert_eq!(field(grants[0], "r"), "3");
                        assert!(field(grants[0], "remaining").parse::<usize>().unwrap() > 0);
                        if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
                            let point = new
                                .lines()
                                .find(|line| {
                                    line.starts_with("FEREG_POLICY_LABEL|checkpoint|")
                                        && field(line, "build") == "1"
                                })
                                .unwrap();
                            let end = new
                                .lines()
                                .find(|line| line.starts_with("FEREG_POLICY_LABEL|action_end|"))
                                .unwrap();
                            let number = |line, name| field(line, name).parse::<u128>().unwrap();
                            assert_eq!(number(point, "workers"), workers as u128);
                            assert_eq!(
                                number(end, "elapsed_ns") - number(point, "stamp_ns"),
                                ["build_ns", "restart_ns", "startup_ns", "continuation_ns"]
                                    .iter()
                                    .map(|name| number(end, name))
                                    .sum::<u128>()
                            );
                            assert!(
                                !new.contains("FEREG_PHASE|"),
                                "ordinary timing emissions suppressed"
                            );
                        }
                        (
                            matrix_bits(fit.residualized()),
                            science(fit.report()),
                            route.to_owned(),
                        )
                    })
                    .unwrap()
            };
            assert_eq!(
                run(),
                run(),
                "fixed worker repeat W={workers}, h={checkpoint}"
            );
        }
    }

    let options = config(
        Some(RoutingPreset::remaining_two_control(Some(16)).unwrap()),
        1,
    );
    let mut session = FeSession::for_ols(&data, BorrowedMapWeights::Unit, options).unwrap();
    // This isolates the existing typed constructor-refusal boundary after normal
    // parent admission. It is not a claim about a public total-memory limit.
    session.categorical.as_mut().unwrap().request_auto_routing(
        options.absorb.routing.unwrap(),
        Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 48,
        }),
        0,
    );
    let mut state = session.remaining_state().unwrap();
    let before = trace().len();
    for _ in 0..2 {
        let fit = session
            .project_ols_with_remaining(
                BorrowedMapWeights::Unit,
                fixture.packed(),
                options.absorb,
                false,
                false,
                Some((
                    ObserverRef::Remaining(&mut state),
                    Demand::Packed { regressors: 2 },
                )),
            )
            .unwrap();
        fixture.check_projection(fit.residualized());
        assert!(fit.report().certified());
    }
    let observed = trace();
    let routes: Vec<_> = observed[before..]
        .lines()
        .filter(|line| line.contains("|phase=auto_schur_route|"))
        .collect();
    assert_eq!(
        routes.len(),
        1,
        "refusal consumes the attempt for the accuracy key"
    );
    assert_eq!(field(routes[0], "installed"), "0");
    assert_eq!(field(routes[0], "refusal"), "Some(MemoryBudget)");
    assert!(!routes[0].contains("hierarchy_vertices="));

    let rectangle = PairedCells::new(true);
    let direct_data = rectangle.prepared(2);
    let mut direct = FeSession::for_ols(&direct_data, BorrowedMapWeights::Unit, options).unwrap();
    let mut direct_state = direct.remaining_state().unwrap();
    let before = trace().len();
    let fit = direct
        .project_ols_with_remaining(
            BorrowedMapWeights::Unit,
            rectangle.packed(),
            options.absorb,
            false,
            false,
            Some((
                ObserverRef::Remaining(&mut direct_state),
                Demand::Packed { regressors: 2 },
            )),
        )
        .unwrap();
    rectangle.check_projection(fit.residualized());
    assert!(fit.report().certified());
    assert!(
        direct_state.logs.is_empty(),
        "direct selection precedes observation-bank allocation"
    );
    let observed = trace();
    let new = &observed[before..];
    if std::env::var_os("FEREG_PHASE_TIMING").is_none() {
        assert!(new.contains("|phase=auto_direct_certified|"), "{new}");
    } else {
        assert!(new.contains("FEREG_POLICY_LABEL|action_end|"));
        assert!(!new.contains("FEREG_POLICY_LABEL|checkpoint|"));
    }
    assert!(!new.contains("|phase=remaining_checkpoint|"));
    assert!(!new.contains("|phase=auto_schur_route|"));

    // Sticky cancellation at the real caller boundary, in this isolated child.
    // Inner-build cancellation and worker joining have separate production tests.
    let mut cancelled = FeSession::for_ols(&data, BorrowedMapWeights::Unit, options).unwrap();
    let mut state = cancelled.remaining_state().unwrap();
    let input = DenseMatrix::from_columns(&[fixture.y.clone()]).unwrap();
    crate::interrupt::request_cancellation();
    let result = cancelled.project_ols_with_remaining(
        BorrowedMapWeights::Unit,
        input,
        options.absorb,
        false,
        false,
        Some((
            ObserverRef::Remaining(&mut state),
            Demand::Regressors {
                start: 0,
                end: 1,
                total: 2,
                block: 1,
            },
        )),
    );
    crate::interrupt::reset_cancellation();
    assert_eq!(
        result.unwrap_err().code,
        crate::error::ErrorCode::Interrupted
    );
    assert!(
        state.history.is_none(),
        "failed caller action must not become history"
    );
}

#[test]
fn gates_shell_preserves_old_parent_boundary() {
    use crate::fe::routing_gates::GateState;
    let fixture = PairedCells::new(false);
    let data = fixture.prepared(2);
    let design =
        crate::ols::prepared_fit_view::PreparedOlsFitView::fit_borrowed_intercept_map_design(&data)
            .unwrap()
            .unwrap();
    let base = crate::fe::ReusableCategoricalMapAbsorber::owner_bytes(&design, 3, 1).unwrap();
    for (preset, extra, admitted) in [
        (None, 0, true),
        (
            Some(RoutingPreset::gates_two()),
            GateState::shell_bytes() - 1,
            false,
        ),
        (
            Some(RoutingPreset::gates_two()),
            GateState::shell_bytes(),
            true,
        ),
    ] {
        let mut options = config(preset, 1);
        options.absorb.retain_coefficients = false;
        options.absorb.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 37,
            usable_bytes: base + extra + 37,
        });
        let session = FeSession::for_ols_rhs(&data, BorrowedMapWeights::Unit, options, 3).unwrap();
        assert_eq!(session.categorical.is_some(), admitted);
        assert_eq!(
            session.gates_state().is_some(),
            preset.is_some() && admitted
        );
        assert!(session.remaining_state().is_none());
        assert_eq!(
            session.peak.bytes().unwrap() as u128,
            if admitted { base + extra } else { 0 }
        );
    }
}

#[test]
fn gates_real_callers_routes_refusal_direct_and_cancel() {
    use crate::fe::routing_gates::GateState;
    const CHILD: &str = "FEREG_GATES_REAL_CALLER_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let path =
            std::env::temp_dir().join(format!("fereg-gates-caller-{}.log", std::process::id()));
        let output_path = path.with_extension("output.log");
        let output = std::fs::File::create(&output_path).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD,"1").env("FEREG_SOLVER_TRACE","1").env("FEREG_PHASE_TIMING","1")
            .env("FEREG_SOLVER_TRACE_FILE",&path)
            .args(["fe::routing_remaining::integration_tests::gates_real_callers_routes_refusal_direct_and_cancel","--exact","--test-threads=1","--nocapture"])
            .stdout(output.try_clone().unwrap()).stderr(output).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let (status, timeout) = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break (s, false);
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                break (child.wait().unwrap(), true);
            }
            std::thread::yield_now();
        };
        let output = std::fs::read_to_string(&output_path).unwrap();
        let trace = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(output_path);
        assert!(
            !timeout && status.success(),
            "timeout={timeout}\n{output}\n{trace}"
        );
        return;
    }
    let fixture = PairedCells::new(false);
    let data = fixture.prepared(2);
    for workers in [1, 4] {
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let mut options = config(Some(RoutingPreset::gates_two()), workers);
            options.execution.mode = mode;
            options.execution.poolsize = 1;
            let mut previous = None;
            for _ in 0..2 {
                let before = trace().len();
                let fit = fit_prepared_model_coefficients(&data, options).unwrap();
                fixture.check_fit(&fit);
                if let Some(ref previous) = previous {
                    assert_fit_exact(&fit, previous);
                }
                let observed = trace();
                let new = &observed[before..];
                let routes: Vec<_> = new
                    .lines()
                    .filter(|s| s.contains("|phase=auto_schur_route|"))
                    .collect();
                assert_eq!(routes.len(), 1, "{new}");
                assert_eq!(field(routes[0], "iterations"), "16");
                assert_eq!(field(routes[0], "installed"), "1");
                let gates: Vec<_> = new
                    .lines()
                    .filter(|s| s.contains("|phase=two_fe_gates|event=decision|"))
                    .collect();
                assert_eq!(gates.len(), 1, "{new}");
                assert_eq!(field(gates[0], "grant"), "1");
                assert_eq!(
                    field(gates[0], "role"),
                    if mode == PreparedOlsExecutionMode::Fast {
                        "Packed"
                    } else {
                        "Outcome"
                    }
                );
                assert!(!new.contains("FEREG_POLICY_LABEL|"));
                assert!(!new.contains("|phase=remaining_checkpoint|"));
                previous = Some(fit);
            }
        }
    }
    let options = config(Some(RoutingPreset::gates_two()), 1);
    let mut session = FeSession::for_ols(&data, BorrowedMapWeights::Unit, options).unwrap();
    let mut state = session.gates_state().unwrap();
    // A short action cannot own the episode; the next actual packed action can.
    let short = DenseMatrix::from_columns(&fixture.x).unwrap();
    let before = trace().len();
    let fit = session
        .project_ols_with_remaining(
            BorrowedMapWeights::Unit,
            short,
            options.absorb,
            false,
            false,
            Some((
                ObserverRef::Gates(&mut state),
                Demand::Regressors {
                    start: 0,
                    end: 2,
                    total: 2,
                    block: 2,
                },
            )),
        )
        .unwrap();
    assert!(fit.report().certified());
    assert!(!trace()[before..].contains("|phase=two_fe_gates|"));
    session.categorical.as_mut().unwrap().request_auto_routing(
        options.absorb.routing.unwrap(),
        Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 48,
        }),
        0,
    );
    let before = trace().len();
    for final_phase in [false, true] {
        let values = if final_phase {
            DenseMatrix::from_columns(&[fixture.y.clone()]).unwrap()
        } else {
            fixture.packed()
        };
        let fit = session
            .project_ols_with_remaining(
                BorrowedMapWeights::Unit,
                values,
                options.absorb,
                final_phase,
                false,
                Some((
                    ObserverRef::Gates(&mut state),
                    if final_phase {
                        Demand::Final
                    } else {
                        Demand::Packed { regressors: 2 }
                    },
                )),
            )
            .unwrap();
        assert!(fit.report().certified());
    }
    let observed = trace();
    let new = &observed[before..];
    let routes: Vec<_> = new
        .lines()
        .filter(|s| s.contains("|phase=auto_schur_route|"))
        .collect();
    assert_eq!(
        routes.len(),
        1,
        "refusal cannot reopen at final accuracy: {new}"
    );
    assert_eq!(field(routes[0], "refusal"), "Some(MemoryBudget)");
    let rectangle = PairedCells::new(true);
    let direct_data = rectangle.prepared(2);
    let mut direct = FeSession::for_ols(&direct_data, BorrowedMapWeights::Unit, options).unwrap();
    let mut terminal = GateState::new();
    terminal.correction().unwrap();
    let before = trace().len();
    let fit = direct
        .project_ols_with_remaining(
            BorrowedMapWeights::Unit,
            rectangle.packed(),
            options.absorb,
            false,
            false,
            Some((
                ObserverRef::Gates(&mut terminal),
                Demand::Packed { regressors: 2 },
            )),
        )
        .unwrap();
    rectangle.check_projection(fit.residualized());
    let observed = trace();
    let new = &observed[before..];
    assert!(new.contains("|phase=auto_direct_certified|"), "{new}");
    assert!(!new.contains("|phase=auto_schur_route|"));
    // Unsupported first roles and a final-first episode never substitute a
    // later favourable packed action, even with a new accuracy key.
    for demand in [
        Demand::OneOff,
        Demand::Regressors {
            start: 0,
            end: 3,
            total: 3,
            block: 3,
        },
        Demand::Final,
    ] {
        let mut session = FeSession::for_ols(&data, BorrowedMapWeights::Unit, options).unwrap();
        let mut state = session.gates_state().unwrap();
        let before = trace().len();
        for first in [true, false] {
            let final_phase = first && demand == Demand::Final;
            let input = if final_phase {
                DenseMatrix::from_columns(&[fixture.y.clone()]).unwrap()
            } else {
                fixture.packed()
            };
            let fit = session
                .project_ols_with_remaining(
                    BorrowedMapWeights::Unit,
                    input,
                    options.absorb,
                    final_phase,
                    false,
                    Some((
                        ObserverRef::Gates(&mut state),
                        if first {
                            demand
                        } else {
                            Demand::Packed { regressors: 2 }
                        },
                    )),
                )
                .unwrap();
            assert!(fit.report().certified());
        }
        let observed = trace();
        let new = &observed[before..];
        assert!(!new.contains("|phase=auto_schur_route|"), "{new}");
        let gates: Vec<_> = new
            .lines()
            .filter(|s| s.contains("|phase=two_fe_gates|event=decision|"))
            .collect();
        assert_eq!(gates.len(), 1, "{new}");
        assert_eq!(field(gates[0], "grant"), "0");
    }
    crate::interrupt::request_cancellation();
    let failed = direct.project_ols_with_remaining(
        BorrowedMapWeights::Unit,
        rectangle.packed(),
        options.absorb,
        false,
        false,
        Some((
            ObserverRef::Gates(&mut terminal),
            Demand::Packed { regressors: 2 },
        )),
    );
    crate::interrupt::reset_cancellation();
    assert_eq!(
        failed.unwrap_err().code,
        crate::error::ErrorCode::Interrupted
    );
}

#[test]
fn gates_unaffected_auto_families_are_exact() {
    let fixture = PairedCells::new(true);
    for effects in [1, 2, 3] {
        let data = fixture.prepared(effects);
        for preconditioner in [LsmrPreconditioner::Auto, LsmrPreconditioner::Off] {
            let mut options = config(None, 1);
            options.absorb.lsmr.preconditioner = preconditioner;
            if effects == 2 && preconditioner == LsmrPreconditioner::Auto {
                continue;
            }
            let expected = fit_prepared_model_coefficients(&data, options).unwrap();
            options.absorb.routing = Some(RoutingPreset::gates_two());
            let session = FeSession::for_ols(&data, BorrowedMapWeights::Unit, options).unwrap();
            assert!(session.gates_state().is_none());
            assert!(session.remaining_state().is_none());
            let actual = fit_prepared_model_coefficients(&data, options).unwrap();
            assert_fit_exact(&actual, &expected);
        }
    }
}
