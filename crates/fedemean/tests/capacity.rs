//! Reused full-projector workspaces remain subject to each public memory cap.
use fedemean::{
    Choice, ColumnsMut, Control, Decline, Design, Effect, InputState, Method, Outcome,
    Preconditioner, Projector, Reduction, Request, Settings, Weights,
};

fn settings(method: Method) -> Settings {
    Settings {
        choice: Choice {
            reduction: Reduction::Full,
            method,
            preconditioner: Preconditioner::Jacobi,
            ..Choice::default()
        },
        ..Settings::default()
    }
}

// A balanced 64 x 32 panel repeated twice. The interaction has zero mean in
// every level, so it is the exact residual, independently of either solver.
fn input(n: usize, width: usize) -> Vec<f64> {
    (0..n * width)
        .map(|i| {
            let (row, j) = (i / width, i % width);
            let (a, b) = (row % 64, (row / 64) % 32);
            let residual = if (a + b) % 2 == 0 { 1.0 } else { -1.0 };
            (j + 1) as f64 * (a as f64 / 16.0 + b as f64 / 8.0 + residual)
        })
        .collect()
}

fn check_residual(values: &[f64], width: usize) {
    for (i, &value) in values.iter().enumerate() {
        let (row, j) = (i / width, i % width);
        let expected = (j + 1) as f64
            * if (row % 64 + (row / 64) % 32) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
        assert!(
            (value - expected).abs() < 1e-8,
            "{i}: {value} != {expected}"
        );
    }
}

#[test]
fn wide_banks_and_weight_gram_state_remain_charged_on_narrow_reuse() {
    let n = 4096;
    let a: Vec<u32> = (0..n).map(|i| (i % 64) as u32).collect();
    let b: Vec<u32> = (0..n).map(|i| ((i / 64) % 32) as u32).collect();
    let weights = vec![2.0; n];
    for method in [Method::Pcg, Method::Lsmr] {
        for weighted in [false, true] {
            for gram in [false, true] {
                let design =
                    || Design::new(n, vec![Effect::new(&a, 64), Effect::new(&b, 32)]).unwrap();
                let mut projector = Projector::new(design(), settings(method));
                projector.prepare(&Control::default()).unwrap();
                let cold = projector.admission_bytes(1).unwrap();
                let wide = projector.admission_bytes(8).unwrap();
                let mut request = Request::new(1e-10, 1000);
                request.memory_limit = Some(wide);
                request.residual_gram = gram;
                let mut data = input(n, 8);
                let outcome = projector
                    .project(
                        if weighted {
                            Weights::Values(&weights)
                        } else {
                            Weights::Unit
                        },
                        ColumnsMut::new(&mut data, n, 8).unwrap(),
                        None,
                        &request,
                        &Control::default(),
                    )
                    .unwrap();
                let Outcome::Projected(report) = outcome else {
                    panic!("wide: {outcome:?}");
                };
                assert!(report.workspace_bytes <= wide);
                check_residual(&data, 8);
                let retained = projector.admission_bytes(1).unwrap();
                assert!(
                    retained > cold,
                    "{method:?}, weighted={weighted}, gram={gram}"
                );

                let original = input(n, 1);
                let mut narrow = original.clone();
                request.residual_gram = false;
                request.memory_limit = Some(cold);
                let outcome = projector
                    .project(
                        Weights::Unit,
                        ColumnsMut::new(&mut narrow, n, 1).unwrap(),
                        None,
                        &request,
                        &Control::default(),
                    )
                    .unwrap();
                assert!(matches!(
                    outcome,
                    Outcome::Declined {
                        reason: Decline::MemoryLimit,
                        input: InputState::Untouched,
                        ..
                    }
                ));
                assert_eq!(narrow, original);

                request.memory_limit = Some(wide.max(retained));
                let outcome = projector
                    .project(
                        Weights::Unit,
                        ColumnsMut::new(&mut narrow, n, 1).unwrap(),
                        None,
                        &request,
                        &Control::default(),
                    )
                    .unwrap();
                let Outcome::Projected(report) = outcome else {
                    panic!("reuse: {outcome:?}");
                };
                assert!(report.workspace_bytes > cold);
                assert!(report.workspace_bytes <= request.memory_limit.unwrap());
                check_residual(&narrow, 1);
                let repeated = narrow.clone();
                narrow.copy_from_slice(&original);
                let outcome = projector
                    .project(
                        Weights::Unit,
                        ColumnsMut::new(&mut narrow, n, 1).unwrap(),
                        None,
                        &request,
                        &Control::default(),
                    )
                    .unwrap();
                assert!(matches!(outcome, Outcome::Projected(_)));
                assert_eq!(narrow, repeated);

                let mut fresh = Projector::new(design(), settings(method));
                narrow.copy_from_slice(&original);
                request.memory_limit = Some(cold);
                let outcome = fresh
                    .project(
                        Weights::Unit,
                        ColumnsMut::new(&mut narrow, n, 1).unwrap(),
                        None,
                        &request,
                        &Control::default(),
                    )
                    .unwrap();
                let Outcome::Projected(report) = outcome else {
                    panic!("fresh: {outcome:?}");
                };
                assert!(report.workspace_bytes <= cold);
                check_residual(&narrow, 1);
            }
        }
    }
}
