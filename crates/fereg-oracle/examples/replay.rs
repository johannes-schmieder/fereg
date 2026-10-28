#![deny(clippy::too_many_lines)]
//! Generate bounded actual-binary cases and independently calculated targets.
#[path = "replay/bias.rs"]
mod bias;
#[allow(dead_code)]
#[path = "../tests/support/ols_complete_oracle.rs"]
mod dense;
#[path = "replay/derived.rs"]
mod derived;
#[path = "replay/expected.rs"]
mod expected;
#[allow(dead_code)]
#[path = "../tests/support/ols_complete.rs"]
mod fixture;
#[path = "replay/known.rs"]
mod known;
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[path = "replay/wide.rs"]
mod wide;
#[path = "replay/wire.rs"]
mod wire;
use fereg_oracle::designs::{generate, DesignSpec, Graph, WeightKind};
use std::{fs, path::PathBuf};

fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    fs::create_dir_all(&directory).unwrap();
    for poisson in [false, true] {
        for seed in 0..32 {
            let weight = if poisson {
                [
                    WeightKind::Unit,
                    WeightKind::Frequency,
                    WeightKind::Probability,
                ][seed % 3]
            } else {
                [
                    WeightKind::Unit,
                    WeightKind::Analytic,
                    WeightKind::Frequency,
                    WeightKind::Probability,
                ][seed % 4]
            };
            let effects = (seed / 4) % 4;
            let clusters = usize::from(seed % 4 == 3);
            let spec = DesignSpec {
                rows: 150,
                levels: match effects {
                    0 => vec![],
                    1 => vec![6],
                    2 => vec![6, 5],
                    _ => vec![6, 5, 4],
                },
                weights: weight,
                clusters,
                poisson,
                slopes: effects == 1 && seed >= 12,
                graph: if seed == 23 {
                    Graph::Disconnected
                } else {
                    Graph::DensePanel
                },
                missing_fraction: if seed % 5 == 0 { 0.04 } else { 0.0 },
                ..DesignSpec::default()
            };
            let mut design = generate(seed as u64, &spec).unwrap();
            // These dense VCE targets use full model rank. Nested-cluster DoF
            // is covered separately by the retained explicit fixtures.
            for cluster in &mut design.clusters {
                for (i, value) in cluster.iter_mut().enumerate() {
                    *value = ((i * 17 + i / 11) % 13) as u64 + (1u64 << 40);
                }
            }
            let name = format!("{}-{seed:02}", if poisson { "ppml" } else { "ols" });
            let vce = if clusters > 0 {
                "cluster"
            } else if poisson || seed % 2 != 0 || weight == WeightKind::Probability {
                "robust"
            } else {
                "iid"
            };
            let solver = if effects < 2 {
                "auto"
            } else {
                ["auto", "lsmr", "cmg"][seed % 3]
            };
            let mut request = wire::make(
                &design,
                poisson,
                vce,
                solver,
                seed >= 12 && (!poisson || solver == "auto"),
            );
            if seed % 4 == 0 {
                request.save_effects(&design);
            }
            request.write(&directory.join(format!("{name}.case")));
            expected::write(
                &directory.join(format!("{name}.json")),
                &design,
                poisson,
                vce,
                &request,
            );
        }
    }
    bias::generate(&directory);
    for method in ["fe", "simplex", "ir", "mu"] {
        let design = generate(
            954,
            &DesignSpec {
                rows: 120,
                poisson: true,
                zero_fraction: 0.25,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let mut request = wire::make(&design, true, "robust", "auto", false);
        request.separation(method);
        let name = format!("ppml-separation-{method}");
        request.write(&directory.join(format!("{name}.case")));
        expected::write(
            &directory.join(format!("{name}.json")),
            &design,
            true,
            "robust",
            &request,
        );
    }
    known::generate(&directory);
    wide::generate_case(&directory);
    derived::generate_cases(&directory);
}
