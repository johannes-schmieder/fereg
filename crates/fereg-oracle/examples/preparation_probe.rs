//! Bounded preparation-only observations. Input generation and checks are untimed.
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;

use fereg_core::{
    ols::prepare_model_owned_with_load_hint, PreparedOlsData, ThreadConfig, ThreadTeam,
};
use fereg_oracle::designs::{generate, DesignSpec, Graph, WeightKind};
use std::fmt::Write;
use std::time::Instant;

struct Fingerprint(u64);
impl Write for Fingerprint {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        for byte in text.bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        Ok(())
    }
}

fn fingerprint(prepared: &PreparedOlsData) -> u64 {
    // Stream the complete public preparation result, without a large String.
    // Exact fingerprints supplement, not replace, independent test witnesses.
    let mut hash = Fingerprint(0xcbf29ce484222325);
    write!(hash, "{prepared:?}").unwrap();
    hash.0
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        (5..=6).contains(&args.len()),
        "preparation_probe ROWS WORKERS REPEATS akm|dense unit|aw|missing [fast]"
    );
    assert!(
        args.get(5).is_none_or(|mode| mode == "fast"),
        "unknown preparation mode"
    );
    let fast = args.get(5).is_some_and(|mode| mode == "fast");
    let rows: usize = args[0].parse().unwrap();
    let workers: usize = args[1].parse().unwrap();
    let repeats: usize = args[2].parse().unwrap();
    assert!((1..=16_000_000).contains(&rows));
    assert!((1..=32).contains(&workers));
    assert!((1..=7).contains(&repeats));
    let graph = match args[3].as_str() {
        "akm" => Graph::AkmCampaign,
        "dense" => Graph::DensePanel,
        _ => panic!("unknown shape"),
    };
    let (weights, missing_fraction) = match args[4].as_str() {
        "unit" => (WeightKind::Unit, 0.0),
        "aw" => (WeightKind::Analytic, 0.0),
        "missing" => (WeightKind::Unit, 0.01),
        _ => panic!("unknown sample"),
    };
    let design = generate(
        739391,
        &DesignSpec {
            rows,
            levels: if graph == Graph::AkmCampaign {
                vec![rows.div_ceil(5), (rows / 100).max(2)]
            } else {
                vec![100, 100]
            },
            regressors: 4,
            graph,
            weights,
            missing_fraction,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let (request, loaded) = support::inputs(&design, false);
    drop(design);
    let team = ThreadTeam::new(ThreadConfig::from_requested(workers, false).unwrap());
    let mut reference = None;
    let mut hash = 0;
    for repetition in 0..=repeats {
        let input = loaded.clone();
        let started = Instant::now();
        let result = team
            .install(|| {
                prepare_model_owned_with_load_hint(
                    &request,
                    &[],
                    input,
                    true,
                    None,
                    if fast {
                        fereg_core::fe::LevelIndex::parallel_scratch_bytes(rows)
                    } else {
                        0
                    },
                )
            })
            .unwrap()
            .unwrap();
        let seconds = started.elapsed().as_secs_f64();
        if let Some(expected) = reference.as_ref() {
            assert_eq!(&result, expected, "preparation repeat changed");
        } else {
            hash = fingerprint(&result);
        }
        println!("{{\"rows\":{rows},\"workers\":{workers},\"fast\":{fast},\"repetition\":{repetition},\"shape\":\"{}\",\"sample\":\"{}\",\"seconds\":{seconds:.9},\"fingerprint\":\"{hash:016x}\",\"owned_bytes\":{}}}", args[3], args[4], result.owned_buffer_bytes());
        if reference.is_none() {
            reference = Some(result);
        }
    }
}
