//! Real successor callsite witnesses, sharing the independent paired-cell oracle.
use super::*;
use crate::fe::routing_remaining::{Demand, Mode};
use std::cell::Cell;

thread_local! {
    static SCAN: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

/// Counts completed readiness entries, not checkpoint ordinals. This hook and
/// its callsite disappear from non-test builds and allocate nothing.
pub(in crate::fe::borrowed_map) fn observer_entry() {
    SCAN.with(|scan| {
        if let Some((completed, trigger)) = scan.get() {
            let completed = completed + 1;
            scan.set(Some((completed, trigger)));
            if completed == trigger {
                crate::interrupt::request_cancellation();
            }
        }
    });
}

struct ScanGuard;
impl ScanGuard {
    fn new(trigger: usize) -> Self {
        SCAN.with(|scan| assert!(scan.replace(Some((0, trigger))).is_none()));
        Self
    }
    fn completed(&self) -> usize {
        SCAN.with(|scan| scan.get().unwrap().0)
    }
}
impl Drop for ScanGuard {
    fn drop(&mut self) {
        SCAN.with(|scan| scan.set(None));
        crate::interrupt::reset_cancellation();
    }
}

fn child(name: &str) -> bool {
    const MARKER: &str = "FEREG_REMAINING_CALLSITE_CHILD";
    if std::env::var(MARKER).as_deref() == Ok(name) {
        return true;
    }
    let path = std::env::temp_dir().join(format!("fereg-{name}-{}.log", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let output_path = path.with_extension("output.log");
    let output = std::fs::File::create(&output_path).unwrap();
    let mut process = std::process::Command::new(std::env::current_exe().unwrap())
        .env(MARKER, name)
        .env("FEREG_SOLVER_TRACE", "1")
        .env("FEREG_PHASE_TIMING", "1")
        .env("FEREG_SOLVER_TRACE_FILE", &path)
        .args([
            &format!("fe::borrowed_map::routing_tests::remaining_tests::{name}"),
            "--exact",
            "--test-threads=1",
            "--nocapture",
        ])
        .stdout(output.try_clone().unwrap())
        .stderr(output)
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let (status, timed_out) = loop {
        if let Some(status) = process.try_wait().unwrap() {
            break (status, false);
        }
        if std::time::Instant::now() >= deadline {
            let _ = process.kill();
            break (process.wait().unwrap(), true);
        }
        std::thread::yield_now();
    };
    let trace = std::fs::read_to_string(&path).unwrap_or_default();
    let output = std::fs::read_to_string(&output_path).unwrap();
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(output_path);
    assert!(
        !timed_out && status.success(),
        "child failed (timeout={timed_out}):\n{output}\n{trace}"
    );
    false
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
fn remaining_correction_restarts_observations_not_work() {
    if !child("remaining_correction_restarts_observations_not_work") {
        return;
    }
    let witness = PairedGraph::new();
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(
        RoutingPreset::remaining_two_control(Some(16)).unwrap(),
        None,
        0,
    );
    let mut state = RemainingState::new(Mode::Force(16));
    state.begin(
        Demand::Packed { regressors: 2 },
        3,
        3,
        config().tolerance,
        false,
    );
    TWO_FE_CERTIFICATE_AT.set(8);
    TWO_FE_CERTIFICATE_REJECTED_AT.set(0);
    TWO_FE_CORRECTIONS.set(0);
    TWO_FE_ROUTING_BUILD_AT.set(0);
    let fit = owner
        .absorb_ols_with_remaining(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.input.clone().into(),
            config(),
            false,
            Some(&mut state),
        )
        .unwrap();
    witness.check(&fit, &[0, 1, 2]);
    state.complete();
    assert_eq!(TWO_FE_CERTIFICATE_AT.get(), 0);
    assert_eq!(TWO_FE_CERTIFICATE_REJECTED_AT.get(), 8);
    assert_eq!(TWO_FE_CORRECTIONS.get(), 1);
    assert_eq!(
        TWO_FE_ROUTING_BUILD_AT.get(),
        24,
        "fresh local age 16 after rejection at 8"
    );
    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_some());
    let trace =
        std::fs::read_to_string(std::env::var_os("FEREG_SOLVER_TRACE_FILE").unwrap()).unwrap();
    let grant = trace
        .lines()
        .find(|line| {
            line.starts_with("FEREG_POLICY_LABEL|checkpoint|") && field(line, "build") == "1"
        })
        .unwrap();
    assert_eq!(field(grant, "epoch"), "1");
    assert_eq!(field(grant, "local"), "16");
    assert_ne!(field(grant, "forecast_h"), "none", "three fresh samples");
    let end = trace
        .lines()
        .find(|line| line.starts_with("FEREG_POLICY_LABEL|action_end|"))
        .unwrap();
    assert_eq!(
        field(end, "steps").parse::<usize>().unwrap(),
        fit.report().iterations,
        "correction keeps the whole action's work history"
    );
}

#[test]
fn remaining_restart_keeps_native_iteration_cap() {
    if !child("remaining_restart_keeps_native_iteration_cap") {
        return;
    }
    let witness = PairedGraph::new();
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(
        RoutingPreset::remaining_two_control(Some(16)).unwrap(),
        None,
        0,
    );
    let limited = MapConfig {
        max_iterations: 22,
        ..config()
    };
    let mut state = RemainingState::new(Mode::Force(16));
    state.begin(
        Demand::Packed { regressors: 2 },
        3,
        3,
        limited.tolerance,
        false,
    );
    TWO_FE_ROUTING_BUILD_AT.set(0);
    let fit = owner.absorb_ols_with_remaining(
        BorrowedMapWeights::Provided(&witness.weights),
        witness.input.clone().into(),
        limited,
        false,
        Some(&mut state),
    );
    assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 16);
    let graph = owner.two_fe_pcg.as_ref().unwrap();
    assert!(graph.cmg_schur.is_some());
    // Twenty-two total native iterations reserve five for MAP. Installation
    // at sixteen leaves exactly one PCG step, never a fresh seventeen.
    assert_eq!(graph.ppml_operator_rhs_count, 17 * 3);
    match fit {
        Ok(fit) => {
            witness.check(&fit, &[0, 1, 2]);
            assert!(fit.report().iterations <= 22);
        }
        Err(error) => assert_eq!(error.code, ErrorCode::InvalidData),
    }
}

#[test]
fn remaining_observer_scan_honors_sticky_inner_break() {
    if !child("remaining_observer_scan_honors_sticky_inner_break") {
        return;
    }
    const LEVELS: usize = 3073;
    let codes: Vec<_> = (0..LEVELS as u32).collect();
    let index = LevelIndex::from_codes(&codes, LEVELS).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, LEVELS, &index).unwrap();
    let design =
        BorrowedInterceptFeDesign::new(LEVELS, vec![source.effect(), source.effect()]).unwrap();
    let mut owner = ReusableCategoricalMapAbsorber::new(design, 1, 1).unwrap();
    let graph = owner.two_fe_pcg.as_mut().unwrap();
    graph.left_weights.fill(1.);
    graph.right_weights.fill(1.);
    graph.residual.fill(1.);
    owner.workspace.reference_norms.fill(1.);
    let mut state = RemainingState::new(Mode::Never);
    state.begin(Demand::OneOff, 1, 1, 1e-12, false);
    state.prepare_observations(None).unwrap();
    let mut remaining = (&mut state, 1);
    let guard = ScanGuard::new(1025);
    let result = observe_remaining(
        Some(&mut remaining),
        0,
        &[1.],
        1e-12,
        100,
        &owner.design,
        owner.two_fe_pcg.as_ref().unwrap(),
        &owner.workspace,
        1,
        false,
        false,
        &[],
    );
    let completed = guard.completed();
    drop(guard);
    assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
    assert!(
        completed >= 1025,
        "sticky trigger followed actual inner work"
    );
    assert!(
        completed - 1025 <= 1024,
        "observer exceeded bounded entry work: {completed}"
    );
    assert_eq!(
        completed, 2048,
        "next real row checkpoint must see the sticky break"
    );
    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
    // A second unmodified pass proves guard cleanup and ordinary observation.
    assert!(!observe_remaining(
        Some(&mut remaining),
        0,
        &[1.],
        1e-12,
        100,
        &owner.design,
        owner.two_fe_pcg.as_ref().unwrap(),
        &owner.workspace,
        1,
        false,
        false,
        &[]
    )
    .unwrap());
}
