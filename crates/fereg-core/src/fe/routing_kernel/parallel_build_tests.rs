use super::super::*;
use crate::fe::{BorrowedInterceptEffect, LevelIndex};
use crate::memory::FeSolverMemoryBudget;

/// Seeded AKM-like codes: workers visit a few firms with repeated rows (so
/// cells aggregate), every seventh worker also visits one large firm (long
/// membership lists on the firm side), and weights vary.
fn design_codes(workers: usize, firms: usize, seed: u64) -> (Vec<u32>, Vec<u32>, Vec<f64>) {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };
    let (mut left, mut right, mut weights) = (Vec::new(), Vec::new(), Vec::new());
    for worker in 0..workers {
        let visits = 1 + next() % 4;
        for visit in 0..visits + usize::from(worker % 7 == 0) {
            let firm = if visit == visits {
                0
            } else if worker < firms && visit == 0 {
                worker
            } else {
                next() % firms
            };
            for _ in 0..1 + next() % 3 {
                left.push(worker as u32);
                right.push(firm as u32);
                weights.push(0.25 + (next() % 1000) as f64 / 97.0);
            }
        }
    }
    (left, right, weights)
}

fn with_design(
    left: &[u32],
    right: &[u32],
    p: usize,
    q: usize,
    action: impl FnOnce(&BorrowedInterceptFeDesign<'_>),
) {
    let li = LevelIndex::from_codes(left, p).unwrap();
    let ri = LevelIndex::from_codes(right, q).unwrap();
    let design = BorrowedInterceptFeDesign::new(
        left.len(),
        vec![
            BorrowedInterceptEffect::new(left, p, &li).unwrap(),
            BorrowedInterceptEffect::new(right, q, &ri).unwrap(),
        ],
    )
    .unwrap();
    action(&design);
}

fn unlimited() -> KernelLimits {
    KernelLimits {
        retained_live_bytes: 0,
        memory: None,
    }
}

fn on_team<T: Send>(threads: usize, action: impl FnOnce() -> T + Send) -> T {
    crate::ThreadTeam::new(crate::ThreadConfig::from_requested(threads, true).unwrap())
        .install(action)
        .unwrap()
}

type Constructor = fn(
    &BorrowedInterceptFeDesign<'_>,
    usize,
    BorrowedMapWeights<'_>,
    f64,
    KernelLimits,
) -> Result<OptionalSetup>;
const CONSTRUCTORS: [(&str, Constructor); 2] = [("radix", build), ("occurrence", build_occurrence)];

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn assert_same(actual: &CellSchur, expected: &CellSchur, label: &str) {
    let cell_bits = |kernel: &CellSchur| {
        kernel
            .cells
            .iter()
            .map(|c| (c.worker, c.firm, c.weight.to_bits()))
            .collect::<Vec<_>>()
    };
    let firm_bits = |kernel: &CellSchur| {
        kernel
            .firm_cells
            .iter()
            .map(|c| (c.worker, c.weight.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(cell_bits(actual), cell_bits(expected), "{label} cells");
    assert_eq!(actual.offsets, expected.offsets, "{label} offsets");
    assert_eq!(
        actual.firm_offsets, expected.firm_offsets,
        "{label} firm offsets"
    );
    assert_eq!(firm_bits(actual), firm_bits(expected), "{label} firm index");
    assert_eq!(
        bits(&actual.worker_weights),
        bits(&expected.worker_weights),
        "{label} worker weights"
    );
    assert_eq!(
        bits(&actual.firm_weights),
        bits(&expected.firm_weights),
        "{label} firm weights"
    );
    assert_eq!(
        bits(&actual.diagonal),
        bits(&expected.diagonal),
        "{label} diagonal"
    );
    // The serial radix cell bank keeps its observation-sized capacity; the
    // occurrence and parallel banks are exact.
    assert!(
        actual.retained_bytes() <= expected.retained_bytes(),
        "{label}"
    );
    if label.starts_with("occurrence") {
        assert_eq!(
            actual.retained_bytes(),
            expected.retained_bytes(),
            "{label}"
        );
    }
    assert_eq!(
        actual.firm_cells.capacity(),
        actual.firm_cells.len(),
        "{label}"
    );
}

// Both constructors, both eliminated sides, weighted and unit: the team
// constructor reproduces the serial constructor's every bank at 1, 2, 4 and 8
// workers, and the serial path stays the one-lane schedule.
#[test]
fn parallel_construction_reproduces_serial_bits_at_every_worker_count() {
    let (workers, firms) = (20_000, 2_500);
    let (left, right, weights) = design_codes(workers, firms, 20261027);
    with_design(&left, &right, workers, firms, |design| {
        for side in [0, 1] {
            for weighted in [true, false] {
                let weights = if weighted {
                    BorrowedMapWeights::Provided(&weights)
                } else {
                    BorrowedMapWeights::Unit
                };
                for (name, constructor) in CONSTRUCTORS {
                    let label = format!("{name} side {side} weighted {weighted}");
                    let serial =
                        on_team(1, || constructor(design, side, weights, 3.0, unlimited()))
                            .unwrap();
                    let serial_peak = serial.peak.bytes().unwrap();
                    let serial = serial.kernel.unwrap();
                    for threads in [1, 2, 4, 8] {
                        let built = on_team(threads, || {
                            constructor(design, side, weights, 3.0, unlimited())
                        })
                        .unwrap();
                        let peak = built.peak.bytes().unwrap();
                        let kernel = built.kernel.unwrap();
                        assert_same(&kernel, &serial, &format!("{label} t{threads}"));
                        assert!(kernel.retained_bytes() <= u128::from(peak));
                        // A parallel schedule admits its own (larger) peak.
                        assert_eq!(threads > 1, peak > serial_peak, "{label} t{threads}");
                    }
                }
            }
        }
    });
}

// Invalid rows and overflowing cells fail exactly as the serial constructors
// do: radix validates every row before refusing an overflow, occurrence order
// reports the first failure in worker order.
#[test]
fn parallel_construction_reproduces_serial_failures() {
    let (workers, firms) = (12_000, 1_500);
    let (left, right, base) = design_codes(workers, firms, 99);
    with_design(&left, &right, workers, firms, |design| {
        let overflow_rows = (0..left.len() - 1)
            .find(|&row| row > 100 && left[row] == left[row + 1] && right[row] == right[row + 1])
            .unwrap();
        let invalid_row = left.len() - 5;
        assert!(left[invalid_row] > left[overflow_rows]);
        for (overflow, invalid) in [(true, false), (false, true), (true, true)] {
            let mut weights = base.clone();
            if overflow {
                weights[overflow_rows] = 1.7e308;
                weights[overflow_rows + 1] = 1.7e308;
            }
            if invalid {
                weights[invalid_row] = 0.0;
            }
            for (name, constructor) in CONSTRUCTORS {
                let outcome = |threads| {
                    on_team(threads, || {
                        match constructor(
                            design,
                            0,
                            BorrowedMapWeights::Provided(&weights),
                            1.0,
                            unlimited(),
                        ) {
                            Ok(setup) => {
                                assert!(setup.kernel.is_none());
                                Ok(setup.refusal)
                            }
                            Err(error) => Err(error.code),
                        }
                    })
                };
                let serial = outcome(1);
                let expected = if invalid && (name == "radix" || !overflow) {
                    Err(crate::error::ErrorCode::InvalidData)
                } else {
                    Ok(Some(KernelRefusal::Numerical))
                };
                assert_eq!(serial, expected, "{name} {overflow} {invalid}");
                for threads in [2, 4, 8] {
                    assert_eq!(
                        outcome(threads),
                        serial,
                        "{name} {overflow} {invalid} t{threads}"
                    );
                }
            }
        }
    });
}

// The parallel scratch is optional: one byte below its peak the serial
// constructor runs with its own peak and bits; one byte below the serial
// peak (beside retained live bytes) both refuse without a kernel.
#[test]
fn parallel_construction_admission_falls_back_then_refuses() {
    let (workers, firms) = (16_000, 2_000);
    let (left, right, weights) = design_codes(workers, firms, 11);
    with_design(&left, &right, workers, firms, |design| {
        let weights = BorrowedMapWeights::Provided(&weights);
        let budget = |usable_bytes| KernelLimits {
            retained_live_bytes: 300,
            memory: Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: usable_bytes + 300,
            }),
        };
        for (name, constructor) in CONSTRUCTORS {
            let serial = on_team(1, || constructor(design, 0, weights, 1.0, unlimited())).unwrap();
            let serial_peak = u128::from(serial.peak.bytes().unwrap());
            let serial = serial.kernel.unwrap();
            on_team(4, || {
                let parallel = constructor(design, 0, weights, 1.0, unlimited()).unwrap();
                let peak = u128::from(parallel.peak.bytes().unwrap());
                assert!(peak > serial_peak, "{name}");
                let exact = constructor(design, 0, weights, 1.0, budget(peak)).unwrap();
                assert_eq!(u128::from(exact.peak.bytes().unwrap()), peak, "{name}");
                assert_same(&exact.kernel.unwrap(), &serial, name);
                let fallback = constructor(design, 0, weights, 1.0, budget(peak - 1)).unwrap();
                assert_eq!(
                    u128::from(fallback.peak.bytes().unwrap()),
                    serial_peak,
                    "{name}"
                );
                assert_same(&fallback.kernel.unwrap(), &serial, name);
                let refused =
                    constructor(design, 0, weights, 1.0, budget(serial_peak - 1)).unwrap();
                assert!(refused.kernel.is_none(), "{name}");
                assert_eq!(refused.refusal, Some(KernelRefusal::MemoryBudget));
                assert_eq!(refused.peak.bytes(), Some(0));
            });
        }
    });
}

// Cancellation is process-wide, so the check runs in a child process. The
// last lane requests cancellation after real rows; the build stops with
// Interrupted, and caller-thread allocation faults stay fatal.
#[test]
fn parallel_construction_cancels_inside_a_lane_and_keeps_allocation_faults() {
    const CHILD: &str = "FEREG_CELL_PARALLEL_BUILD_CANCEL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "fe::routing_kernel::parallel_build::tests::parallel_construction_cancels_inside_a_lane_and_keeps_allocation_faults",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let (workers, firms) = (16_000, 2_000);
    let (left, right, weights) = design_codes(workers, firms, 5);
    with_design(&left, &right, workers, firms, |design| {
        let weights = BorrowedMapWeights::Provided(&weights);
        for (name, constructor) in CONSTRUCTORS {
            let serial = on_team(1, || constructor(design, 0, weights, 1.0, unlimited()))
                .unwrap()
                .kernel
                .unwrap();
            on_team(4, || {
                let (cancelled, completed) = construction_fault(true, || {
                    let result = constructor(design, 0, weights, 1.0, unlimited());
                    (result, crate::interrupt::cancellation_requested())
                });
                assert_eq!(
                    cancelled.0.err().unwrap().code,
                    crate::error::ErrorCode::Interrupted,
                    "{name}"
                );
                assert!(cancelled.1, "{name}");
                assert!(completed >= 32 && completed % 32 == 0, "{name}");
                assert!(!crate::interrupt::cancellation_requested());
                let (failed, _) =
                    construction_fault(false, || constructor(design, 0, weights, 1.0, unlimited()));
                assert_eq!(
                    failed.err().unwrap().code,
                    crate::error::ErrorCode::Allocation,
                    "{name}"
                );
                let kernel = constructor(design, 0, weights, 1.0, unlimited())
                    .unwrap()
                    .kernel
                    .unwrap();
                assert_same(&kernel, &serial, name);
            });
        }
    });
}
