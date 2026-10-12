//! Bounded comparison of serial and caller-admitted stable membership.
use fereg_core::fe::LevelIndex;
use fereg_core::{ThreadConfig, ThreadTeam};
use std::time::Instant;

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        4,
        "membership_probe ROWS WORKERS LEVELS uniform|skew"
    );
    assert!(
        ["uniform", "skew"].contains(&args[3].as_str()),
        "unknown shape"
    );
    let rows: usize = args[0].parse().unwrap();
    let workers: usize = args[1].parse().unwrap();
    let levels: usize = args[2].parse().unwrap();
    assert!((1..=16_000_000).contains(&rows));
    assert!((1..=32).contains(&workers));
    assert!(levels > 0 && levels <= rows);
    let codes: Vec<u32> = (0..rows)
        .map(|row| {
            if args[3] == "skew" && row % 10 != 0 {
                0
            } else {
                (mix(row as u64 + 739391) % levels as u64) as u32
            }
        })
        .collect();
    let reference = LevelIndex::from_codes(&codes, levels).unwrap();
    let team = ThreadTeam::new(ThreadConfig::from_requested(workers, false).unwrap());
    let budget = LevelIndex::parallel_scratch_bytes(rows);
    for repetition in 0..=4 {
        for candidate in if repetition % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let started = Instant::now();
            let result = team
                .install(|| {
                    if candidate {
                        LevelIndex::from_codes_with_scratch(&codes, levels, budget)
                    } else {
                        LevelIndex::from_codes(&codes, levels)
                    }
                })
                .unwrap()
                .unwrap();
            let seconds = started.elapsed().as_secs_f64();
            assert_eq!(result, reference);
            println!("{{\"rows\":{rows},\"workers\":{workers},\"levels\":{levels},\"candidate\":{candidate},\"repetition\":{repetition},\"seconds\":{seconds:.9},\"scratch_budget\":{budget}}}");
        }
    }
}
