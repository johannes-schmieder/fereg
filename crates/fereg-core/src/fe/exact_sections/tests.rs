use super::*;
use crate::error::ErrorCode;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

static MODE: AtomicU8 = AtomicU8::new(0);
static LOCK: Mutex<()> = Mutex::new(());

pub(super) fn forced() -> Option<bool> {
    match MODE.load(Ordering::SeqCst) {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

/// Run `work` with every section forced to its serial or parallel form.
/// Tests that do not take the lock may observe the forced form; both forms
/// are bitwise identical by construction, which these tests witness.
pub(crate) fn with_form<R>(parallel: bool, work: impl FnOnce() -> R) -> R {
    let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
    MODE.store(if parallel { 2 } else { 1 }, Ordering::SeqCst);
    let result = work();
    MODE.store(0, Ordering::SeqCst);
    result
}

fn values(n: usize, seed: u64) -> Vec<f64> {
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    (0..n)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            // Mixed signs and magnitudes make every rounding order visible.
            let mantissa = (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
            mantissa * 2f64.powi((state % 41) as i32 - 20)
        })
        .collect()
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn pools() -> Vec<rayon::ThreadPool> {
    [1, 2, 4, 8]
        .into_iter()
        .map(|threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
        })
        .collect()
}

#[test]
fn helper_merge_matches_the_serial_worker_order_at_every_width() {
    for (p, helpers) in [(10_001, 7), (CHUNK * 3, 2), (17, 3)] {
        let output = values(p, 1);
        let partials: Vec<Vec<f64>> = (0..helpers).map(|h| values(p, 10 + h as u64)).collect();
        // The original merge: each helper's partial over the whole output.
        let mut expected = output.clone();
        for partial in &partials {
            for (value, addend) in expected.iter_mut().zip(partial) {
                *value += addend;
            }
        }
        for pool in pools() {
            for parallel in [false, true] {
                let mut merged = output.clone();
                with_form(parallel, || {
                    pool.install(|| add_partials(&mut merged, &partials, |v| v.as_slice()))
                })
                .unwrap();
                assert_eq!(bits(&merged), bits(&expected), "p={p} parallel={parallel}");
            }
        }
    }
}

#[test]
fn shorter_partials_leave_the_tail_unchanged_in_both_forms() {
    let output = values(CHUNK + 9, 2);
    let partials = vec![values(CHUNK + 3, 3)];
    for parallel in [false, true] {
        let mut merged = output.clone();
        with_form(parallel, || {
            add_partials(&mut merged, &partials, |v| v.as_slice())
        })
        .unwrap();
        assert_eq!(bits(&merged[CHUNK + 3..]), bits(&output[CHUNK + 3..]));
    }
}

#[test]
fn cross_block_matches_the_serial_column_order_at_every_width() {
    let hardware = crate::kernels::installed();
    for (p, k) in [(10_001, 16), (CHUNK * 2, 3), (5, 1)] {
        let cross = values(p * k, 4);
        let hessian = values(k * k, 5);
        let direction = values(p + k, 6);
        let start = values(p + k, 7);
        // The original loop of the joint recurrence.
        let mut expected = start.clone();
        for j in 0..k {
            let column = &cross[j * p..(j + 1) * p];
            expected[p + j] =
                crate::kernels::dispatch!(hardware, ordered_dot(column, &direction[..p]))
                    + crate::kernels::dispatch!(
                        hardware,
                        ordered_dot(&hessian[j * k..(j + 1) * k], &direction[p..p + k])
                    );
            crate::kernels::dispatch!(
                hardware,
                scaled_add(column, direction[p + j], &mut expected[..p])
            );
        }
        for pool in pools() {
            for parallel in [false, true] {
                let mut product = start.clone();
                with_form(parallel, || {
                    pool.install(|| cross_block(&cross, &hessian, &direction, &mut product, p, k))
                })
                .unwrap();
                assert_eq!(bits(&product), bits(&expected), "p={p} k={k}");
            }
        }
    }
}

#[test]
fn level_masses_match_the_serial_row_scatter_at_every_width() {
    let n = 50_000;
    let levels = 3_001;
    let draws = values(n, 8);
    // Level 0 stays empty; the others are interleaved in row order.
    let codes: Vec<u32> = draws
        .iter()
        .map(|value| 1 + (value.to_bits() % (levels as u64 - 1)) as u32)
        .collect();
    let weights: Vec<f64> = values(n, 9).iter().map(|value| value.abs()).collect();
    let index = LevelIndex::from_codes(&codes, levels).unwrap();
    for level in 0..levels {
        assert!(index
            .members(level)
            .windows(2)
            .all(|pair| pair[0] < pair[1]));
    }
    let mut expected = vec![0.0; levels];
    for (&code, &weight) in codes.iter().zip(&weights) {
        expected[code as usize] += weight;
    }
    for root in &mut expected {
        *root = root.sqrt();
    }
    for pool in pools() {
        for parallel in [false, true] {
            let mut roots = vec![f64::NAN; levels];
            with_form(parallel, || {
                pool.install(|| level_root_masses(&mut roots, &codes, &index, &weights))
            })
            .unwrap();
            assert_eq!(bits(&roots), bits(&expected), "parallel={parallel}");
        }
    }
}

/// The cancellation flag is process-wide, so the witness runs in a child.
#[test]
fn cancellation_inside_a_lane_stops_each_parallel_section() {
    const CHILD_ENV: &str = "FEREG_EXACT_SECTIONS_CANCELLATION_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD_ENV, "1")
            .args([
                "fe::exact_sections::tests::cancellation_inside_a_lane_stops_each_parallel_section",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success(), "isolated cancellation witness failed");
        return;
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let interrupted = |result: Result<()>| {
        assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
        crate::interrupt::test_polls::cancel_at(None);
    };
    let p = CHUNK * 8;
    let partials = vec![values(p, 1)];
    let mut output = values(p, 2);
    crate::interrupt::test_polls::cancel_at(Some(3));
    interrupted(with_form(true, || {
        pool.install(|| add_partials(&mut output, &partials, |v| v.as_slice()))
    }));
    let (k, cross, hessian, direction) = (2, values(p * 2, 3), values(4, 4), values(p + 2, 5));
    let mut product = values(p + 2, 6);
    crate::interrupt::test_polls::cancel_at(Some(4));
    interrupted(with_form(true, || {
        pool.install(|| cross_block(&cross, &hessian, &direction, &mut product, p, k))
    }));
    let codes: Vec<u32> = (0..p as u32).collect();
    let index = LevelIndex::from_codes(&codes, p).unwrap();
    let mut roots = vec![0.0; p];
    crate::interrupt::test_polls::cancel_at(Some(3));
    interrupted(with_form(true, || {
        pool.install(|| level_root_masses(&mut roots, &codes, &index, &partials[0]))
    }));
    // Without a request every form completes.
    assert!(
        with_form(true, || add_partials(&mut output, &partials, |v| v
            .as_slice()))
        .is_ok()
    );
}
