//! Translation changes intercept+slope coordinates, not their fitted span.
//! A binary slope has a direct oracle: weighted cell means within each
//! (level, binary value), with no numerical factorization of raw columns.
use fedemean::{
    Choice, ColumnsMut, Control, Design, Effect, Method, Outcome, Preconditioner, Projector,
    Reduction, Request, SchwarzSpec, Settings, Weights,
};

fn residual(
    codes: &[u32],
    slopes: &[&[f64]],
    intercept: bool,
    weights: &[f64],
    y: &[f64],
    (method, preconditioner): (Method, Preconditioner),
    extra_intercept: bool,
) -> Vec<f64> {
    let common = vec![0; y.len()];
    let mut effects = vec![Effect::new(codes, 4)
        .with_slopes(intercept, slopes.iter().copied())
        .unwrap()];
    if extra_intercept {
        effects.push(Effect::new(&common, 1));
    }
    let design = Design::new(y.len(), effects).unwrap();
    let settings = Settings {
        choice: Choice {
            reduction: Reduction::Full,
            method,
            preconditioner,
            ..Choice::default()
        },
        lanes: None,
    };
    let mut projector = Projector::new(design, settings);
    let mut r = y.to_vec();
    let out = projector
        .project(
            Weights::Values(weights),
            ColumnsMut::new(&mut r, y.len(), 1).unwrap(),
            None,
            &Request::new(1e-10, 5000),
            &Control::default(),
        )
        .unwrap();
    match out {
        Outcome::Projected(report) => assert!(report.certificate.passed),
        other => panic!("{method:?}/{preconditioner:?}: {other:?}"),
    }
    r
}

#[test]
fn translated_binary_span_matches_weighted_cell_means() {
    let n = 48;
    let codes: Vec<u32> = (0..n).map(|i| (i / 12) as u32).collect();
    let y: Vec<f64> = (0..n)
        .map(|i| {
            if i % 2 == 0 {
                1.0 + (i % 3) as f64
            } else {
                0.0
            }
        })
        .collect();
    for constant_level in [false, true] {
        let binary: Vec<f64> = (0..n)
            .map(|i| {
                if constant_level && i < 12 {
                    0.0
                } else {
                    (i % 2) as f64
                }
            })
            .collect();
        for weighted in [false, true] {
            let w: Vec<f64> = (0..n)
                .map(|i| if weighted { 1.0 + (i % 5) as f64 } else { 1.0 })
                .collect();
            let mut sums = [[0.0; 2]; 8];
            for i in 0..n {
                let cell = 2 * codes[i] as usize + binary[i] as usize;
                sums[cell][0] += w[i];
                sums[cell][1] += w[i] * y[i];
            }
            let oracle: Vec<f64> = (0..n)
                .map(|i| {
                    let cell = 2 * codes[i] as usize + binary[i] as usize;
                    y[i] - sums[cell][1] / sums[cell][0]
                })
                .collect();
            for (offset, scale) in [
                (0.0, 1.0),
                (1e15, 1.0),
                (-1e15, 1.0),
                (1e15, 65536.0),
                (1e15, 1.0 / 65536.0),
            ] {
                let z: Vec<f64> = binary.iter().map(|b| (offset + b) * scale).collect();
                // Exact dependent channel, including its own offset.
                let z2: Vec<f64> = z.iter().map(|z| -2.0 * z).collect();
                for dependent in [false, true] {
                    let slopes = if dependent {
                        vec![z.as_slice(), z2.as_slice()]
                    } else {
                        vec![z.as_slice()]
                    };
                    for method in [Method::Pcg, Method::Lsmr] {
                        let mut preconditioners = vec![Preconditioner::Jacobi];
                        if scale == 1.0 {
                            preconditioners.push(Preconditioner::Identity);
                        }
                        if cfg!(feature = "schwarz") {
                            preconditioners.push(Preconditioner::Schwarz(SchwarzSpec::default()));
                        }
                        for preconditioner in preconditioners {
                            let r = residual(
                                &codes,
                                &slopes,
                                true,
                                &w,
                                &y,
                                (method, preconditioner),
                                true,
                            );
                            for (i, (&actual, &expected)) in r.iter().zip(&oracle).enumerate() {
                                assert!((actual - expected).abs() < 1e-8,
                                    "offset {offset} scale {scale} weighted {weighted} dependent {dependent} constant {constant_level} {method:?}/{preconditioner:?} row {i}: {actual} vs {expected}");
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn pure_slope_projection_keeps_its_origin() {
    let n = 48;
    let codes: Vec<u32> = (0..n).map(|i| (i / 12) as u32).collect();
    let w: Vec<f64> = (0..n).map(|i| 1.0 + (i % 5) as f64).collect();
    let y: Vec<f64> = (0..n).map(|i| 1.0 + (i % 3) as f64).collect();
    for offset in [0.0, 8.0, 1e15] {
        let z: Vec<f64> = (0..n).map(|i| offset + (i % 2) as f64).collect();
        let mut sums = [[0.0; 2]; 4];
        for i in 0..n {
            sums[codes[i] as usize][0] += w[i] * z[i] * y[i];
            sums[codes[i] as usize][1] += w[i] * z[i] * z[i];
        }
        for method in [Method::Pcg, Method::Lsmr] {
            let r = residual(
                &codes,
                &[&z],
                false,
                &w,
                &y,
                (method, Preconditioner::Jacobi),
                false,
            );
            for i in 0..n {
                let level = codes[i] as usize;
                let expected = y[i] - z[i] * sums[level][0] / sums[level][1];
                assert!((r[i] - expected).abs() < 1e-8, "offset {offset} row {i}");
            }
        }
    }
}
