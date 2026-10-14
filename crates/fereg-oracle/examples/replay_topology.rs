//! Small disconnected three-FE PPML witnesses for structural topology reuse.
#[path = "replay/expected.rs"]
mod expected;
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "replay/wire.rs"]
mod wire;

use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::DummyEffect;
use std::{fs, path::PathBuf};

fn design(weighted: bool) -> Design {
    let mut codes = [Vec::new(), Vec::new(), Vec::new()];
    let (mut x, mut y, mut weights) = (vec![], vec![], vec![]);
    for component in 0..2 {
        for first in 0..2 {
            for second in 0..3 {
                for third in 0..4 {
                    for repeat in 0..2 {
                        let regressor = ((first * 17 + second * 11 + third * 7 + repeat * 19) % 29)
                            as f64
                            / 16.
                            - 0.875;
                        let eta = 0.2 * regressor
                            + (first as f64 - 0.5) * 0.1
                            + (second as f64 - 1.) * 0.1
                            + (third as f64 - 1.5) * 0.1
                            + component as f64 * 0.2;
                        let outcome = eta.exp() * if repeat == 0 { 0.75 } else { 1.3 }
                            + ((first + second * third) % 3) as f64 * 0.03;
                        codes[0].push((component * 2 + first) as u32);
                        codes[1].push((component * 3 + second) as u32);
                        codes[2].push((component * 4 + third) as u32);
                        weights.push(if weighted {
                            1. + ((y.len() * 13 + first * 7) % 11) as f64 / 8.
                        } else {
                            1.
                        });
                        x.push(regressor);
                        y.push(outcome);
                    }
                }
            }
        }
    }
    assert_eq!(y.len(), 96);
    Design {
        outcome: y,
        regressors: vec![x],
        effects: codes
            .into_iter()
            .zip([4, 6, 8])
            .map(|(codes, levels)| DummyEffect::new(codes, levels, true, vec![]).unwrap())
            .collect(),
        weights,
        weight_kind: if weighted {
            WeightKind::Probability
        } else {
            WeightKind::Unit
        },
        missing: vec![false; 96],
        clusters: vec![],
    }
}

fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    fs::create_dir_all(&directory).unwrap();
    for weighted in [false, true] {
        let design = design(weighted);
        for low in [false, true] {
            let name = format!(
                "topology-{}-{}",
                if weighted { "weighted" } else { "unit" },
                if low { "low" } else { "fast" }
            );
            let case = directory.join(format!("{name}.case"));
            assert!(!case.exists(), "refuse to replace frozen topology inputs");
            let mut request = wire::make(&design, true, "robust", "auto", low);
            request.replace("tolerance", "1e-12");
            request.save_effects(&design);
            request.write(&case);
            expected::write(
                &case.with_extension("json"),
                &design,
                true,
                "robust",
                &request,
            );
        }
    }
}
