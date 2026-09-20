mod error { pub use fereg_core::error::*; }
#[path = "baseline_factorize.rs"] mod baseline;
#[path = "candidate_factorize.rs"] mod candidate;
use fereg_core::{ThreadConfig, ThreadTeam};
use std::{hint::black_box, time::Instant};
fn main() {
    let n = 983_040;
    println!("case\tthreads\trepetition\tarm\tseconds");
    for threads in [1, 16] {
        let team = ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap());
        for case in ["ordered_dense", "ordered_fractional", "ordered_unique", "constant", "early_unordered", "late_unordered"] {
            let values: Vec<f64> = (0..n).map(|r| match case {
                "ordered_fractional" => (r / 5) as f64 * 0.25 - 20_000.0,
                "ordered_unique" => r as f64,
                "constant" => -0.0,
                "early_unordered" => ((r * 7_919) % (n / 5)) as f64,
                "late_unordered" if r == n - 1 => 0.0,
                _ => (r / 5) as f64,
            }).collect();
            team.install(|| {
                let expected = baseline::factorize_f64(&values).unwrap();
                let actual = candidate::factorize_f64(&values).unwrap();
                assert_eq!(expected.codes, actual.codes);
                assert_eq!(expected.levels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    actual.levels.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
            }).unwrap();
            for repetition in 1..=11 {
                let order = if repetition % 2 == 1 { ["baseline", "candidate"] } else { ["candidate", "baseline"] };
                for arm in order {
                    let elapsed = team.install(|| {
                        let started = Instant::now();
                        if arm == "baseline" {
                            let result = baseline::factorize_f64(black_box(&values)).unwrap();
                            let seconds = started.elapsed().as_secs_f64();
                            black_box(result);
                            seconds
                        } else {
                            let result = candidate::factorize_f64(black_box(&values)).unwrap();
                            let seconds = started.elapsed().as_secs_f64();
                            black_box(result);
                            seconds
                        }
                    }).unwrap();
                    println!("{case}\t{threads}\t{repetition}\t{arm}\t{elapsed:.9}");
                }
            }
        }
    }
}
