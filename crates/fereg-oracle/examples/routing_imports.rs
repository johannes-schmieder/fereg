//! Fresh, independently witnessed import families for sequential routing stages.
//! Usage: routing_imports OUTPUT_DIRECTORY SEED. The output must be new.
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

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(arguments.len(), 2, "routing_imports OUTPUT_DIRECTORY SEED");
    let directory = PathBuf::from(&arguments[0]);
    let seed: u64 = arguments[1].parse().expect("unsigned seed");
    let cases = [
        ("no-fe", vec![], false, false),
        ("one-fe", vec![6], false, false),
        ("slopes", vec![6], true, false),
        ("low-saved", vec![6, 5], false, true),
    ];
    fs::create_dir(&directory).expect("new output directory with existing parent");
    for (index, (name, levels, slopes, saved)) in cases.into_iter().enumerate() {
        let design = generate(
            seed.checked_add(index as u64).expect("seed overflow"),
            &DesignSpec {
                rows: 150,
                levels,
                slopes,
                weights: WeightKind::Unit,
                graph: Graph::DensePanel,
                missing_fraction: 0.,
                ..DesignSpec::default()
            },
        )
        .expect("bounded routing design");
        let mut request = wire::make(&design, false, "robust", "auto", false);
        if saved {
            request.save_effects(&design);
            request.replace("memory", "low");
        }
        request.write(&directory.join(format!("{name}.case")));
        expected::write(
            &directory.join(format!("{name}.json")),
            &design,
            false,
            "robust",
            &request,
        );
    }
}
