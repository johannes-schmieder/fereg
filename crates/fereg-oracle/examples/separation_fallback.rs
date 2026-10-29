//! Two actual-plugin inputs for the pinned retained-CG to LSMR separation case.
//! Uses the maintained generator, wire serializer and independent dense oracle.
#[path = "replay/expected.rs"]
mod expected;
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "replay/wire.rs"]
mod wire;

use fereg_oracle::designs::{generate, DesignSpec, Graph, WeightKind};
use std::{fs, path::PathBuf};

// Row identities from the retained seed-17 census, checked against its
// independent HiGHS LP witness below. These are zero-based ORIGINAL rows.
// A new plugin's sample is never used to construct the dense fit target.
const DROPPED: [usize; 72] = [
    4, 9, 15, 18, 19, 21, 27, 29, 32, 40, 41, 44, 45, 50, 52, 58, 64, 70, 78, 79, 85, 87, 89, 92,
    99, 101, 104, 110, 116, 124, 126, 134, 138, 139, 145, 147, 149, 150, 157, 161, 164, 170, 178,
    184, 186, 195, 198, 199, 205, 207, 209, 211, 219, 221, 224, 225, 230, 241, 244, 249, 258, 259,
    263, 267, 269, 277, 281, 283, 284, 289, 290, 294,
];

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let output = PathBuf::from(arguments.next().expect("new output directory"));
    assert!(
        arguments.next().is_none(),
        "one output directory is required"
    );
    let task_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp")
        .canonicalize()
        .expect("existing task output root");
    let parent = output
        .parent()
        .expect("output parent")
        .canonicalize()
        .unwrap();
    assert!(
        parent.starts_with(task_root),
        "output must be under target/tmp"
    );
    assert!(!output.exists(), "refuse to replace an existing fixture");
    fs::create_dir(&output).unwrap();

    // Exact seed-17 configuration from tests/ppml_separation_routing.rs.
    let mut design = generate(
        17,
        &DesignSpec {
            rows: 300,
            levels: vec![30, 20],
            regressors: 2,
            graph: Graph::Chain,
            weights: WeightKind::Probability,
            poisson: true,
            zero_fraction: 0.7,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let mut zeros = 0;
    for row in 0..design.outcome.len() {
        design.regressors[1][row] = 0.0;
        if design.outcome[row] == 0.0 {
            zeros += 1;
            if zeros % 5 == 0 {
                design.regressors[1][row] = 1.0 + (row % 3) as f64;
            }
        }
    }

    let mut oracle = design.clone();
    for row in DROPPED {
        oracle.missing[row] = true;
    }
    let (_, retained) = oracle.poisson_problem(false).unwrap();
    let hash = retained
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, &row| {
            (hash ^ row as u64).wrapping_mul(0x0100_0000_01b3)
        });
    assert_eq!((retained.len(), hash), (228, 0xaf35_96f7_b415_6b10));

    for (name, injected) in [("public-auto", false), ("retained-cg", true)] {
        let mut request = wire::make(&design, true, "robust", "auto", false);
        request.separation("fe ir");
        request.replace("tolerance", "1e-8");
        request.replace("iterate", "10000");
        request.replace("separation_tolerance", "1e-8");
        request.replace("separation_max_iterations", "10000");
        if injected {
            request.arg("inject", "ppml_separation_cg");
        }
        request.write(&output.join(format!("{name}.case")));
        if !injected {
            expected::write(
                &output.join("expected.json"),
                &oracle,
                true,
                "robust",
                &request,
            );
        }
    }
}
