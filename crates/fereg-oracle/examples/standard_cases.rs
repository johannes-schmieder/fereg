//! Frozen standard timing workloads for `bench/time_exact.py`.
//!
//! Six fixed designs at one and four workers (seed 739391, tolerance 1e-8,
//! automatic solver). Their SHA-256 values are recorded in
//! `bench/standard_cases.json`; regenerate and compare before timing.
//! `standard_cases OUTPUT_DIR [SCALE]`; SCALE > 1 divides rows and levels for
//! quick local smoke runs (those inputs are not the frozen workloads).
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "replay/wire.rs"]
mod wire;

use fereg_oracle::designs::{generate, DesignSpec, WeightKind};
use fereg_oracle::explicit_ols::DummyEffect;
use std::{fs, path::PathBuf};

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

struct Spec {
    name: &'static str,
    rows: usize,
    regressors: usize,
    effects: usize,
    weights: WeightKind,
    vce: &'static str,
    poisson: bool,
}

const fn spec(
    name: &'static str,
    rows: usize,
    regressors: usize,
    effects: usize,
    weights: WeightKind,
    vce: &'static str,
) -> Spec {
    let poisson = name.as_bytes()[0] == b'p';
    Spec {
        name,
        rows,
        regressors,
        effects,
        weights,
        vce,
        poisson,
    }
}

const SPECS: [Spec; 6] = [
    spec("ols-two-unit-k5", 1_000_000, 5, 2, WeightKind::Unit, "iid"),
    spec(
        "ols-two-pw-k5",
        1_000_000,
        5,
        2,
        WeightKind::Probability,
        "robust",
    ),
    spec(
        "ols-two-unit-k20",
        1_000_000,
        20,
        2,
        WeightKind::Unit,
        "iid",
    ),
    spec(
        "ols-three-unit-k5",
        200_000,
        5,
        3,
        WeightKind::Unit,
        "cluster",
    ),
    spec(
        "ppml-two-unit-k5",
        200_000,
        5,
        2,
        WeightKind::Unit,
        "robust",
    ),
    spec("ols-no-fe-k5", 20_000, 5, 0, WeightKind::Unit, "iid"),
];

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next().map(PathBuf::from) else {
        eprintln!("usage: standard_cases OUTPUT_DIR [SCALE]");
        std::process::exit(2);
    };
    let scale: usize = args
        .next()
        .map_or(1, |text| text.parse().expect("SCALE is a positive integer"));
    assert!(scale > 0, "SCALE is a positive integer");
    fs::create_dir_all(&out).expect("create output directory");
    for Spec {
        name,
        rows,
        regressors,
        effects,
        weights,
        vce,
        poisson,
    } in SPECS
    {
        let rows = rows / scale;
        let mut design = generate(
            739_391,
            &DesignSpec {
                rows,
                regressors,
                levels: vec![10; effects],
                weights,
                poisson,
                clusters: usize::from(vce == "cluster"),
                ..DesignSpec::default()
            },
        )
        .expect("generate design");
        design.effects = (0..effects)
            .map(|effect| {
                let levels = ([10_000, 2_000, 100][effect] / scale).max(5);
                let codes = (0..rows)
                    .map(|row| {
                        let key = (row as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15)
                            ^ (effect as u64 * 739_391);
                        (mix(key) % levels as u64) as u32
                    })
                    .collect();
                DummyEffect::new(codes, levels, true, vec![]).expect("effect")
            })
            .collect();
        for threads in [1, 4] {
            let mut request = wire::make(&design, poisson, vce, "auto", false);
            request.replace("tolerance", "1e-8");
            request.replace("threads", &threads.to_string());
            request.write(&out.join(format!("{name}-t{threads}.case")));
        }
    }
}
