//! Parallel cell Schur product with the serial operator's bits.
//!
//! Workers own disjoint rows of the left scratch and firms own disjoint rows
//! of the output. Each worker sums its cells in the serial order; each firm
//! starts from its diagonal term and subtracts its cells in increasing worker
//! order, which is the order the serial scatter reaches that firm. Every lane
//! count therefore reproduces [`CellSchur::apply`] exactly.

use super::*;
use crate::parallel::StagePlan;

impl CellSchur {
    /// Whether the parallel product has more than one lane for this width.
    pub(crate) fn parallel_lanes(&self, rhs_count: usize) -> bool {
        StagePlan::current(
            self.cells.len() as u128 * rhs_count as u128,
            self.worker_weights.len().min(self.firm_weights.len()),
        )
        .lanes
            > 1
    }

    /// [`Self::apply`] on the installed host team. Borrowed buffers only; the
    /// left scratch and output rows beyond `rhs_count` are untouched.
    pub fn apply_parallel(
        &self,
        input: &[f64],
        output: &mut [f64],
        left_scratch: &mut [f64],
        rhs_count: usize,
        stride: usize,
    ) -> Result<()> {
        let (p, q) = (self.worker_weights.len(), self.firm_weights.len());
        let right_len = q
            .checked_mul(stride)
            .ok_or_else(|| allocation("cell Schur right workspace size overflows"))?;
        let left_len = p
            .checked_mul(stride)
            .ok_or_else(|| allocation("cell Schur left workspace size overflows"))?;
        if rhs_count > stride
            || input.len() != right_len
            || output.len() != right_len
            || left_scratch.len() != left_len
        {
            return Err(invalid_argument(
                "cell Schur input/workspace dimensions differ",
            ));
        }
        if CELL_RECORD_ACTIVITY.get() {
            CELL_APPLY_COUNT.set(CELL_APPLY_COUNT.get() + 1);
        }
        let work = self.cells.len() as u128 * rhs_count as u128;
        StagePlan::current(work, p).groups_mut(
            left_scratch,
            stride,
            &self.offsets,
            |first, rows| {
                for (local, row) in rows.chunks_exact_mut(stride).enumerate() {
                    crate::interrupt::checkpoint()?;
                    let worker = first + local;
                    let row = &mut row[..rhs_count];
                    row.fill(0.0);
                    for cell in &self.cells[self.offsets[worker]..self.offsets[worker + 1]] {
                        let firm = cell.firm as usize * stride;
                        for (sum, &value) in row.iter_mut().zip(&input[firm..firm + rhs_count]) {
                            *sum += cell.weight * value;
                        }
                    }
                    let total = self.worker_weights[worker];
                    if total > 0.0 {
                        for value in row.iter_mut() {
                            *value /= total;
                        }
                    }
                }
                Ok(())
            },
        )?;
        let left_scratch = &*left_scratch;
        StagePlan::current(work, q).groups_mut(
            output,
            stride,
            &self.firm_offsets,
            |first, rows| {
                for (local, row) in rows.chunks_exact_mut(stride).enumerate() {
                    crate::interrupt::checkpoint()?;
                    let firm = first + local;
                    let row = &mut row[..rhs_count];
                    let total = self.firm_weights[firm];
                    let source = &input[firm * stride..firm * stride + rhs_count];
                    for (value, &term) in row.iter_mut().zip(source) {
                        *value = total * term;
                    }
                    let cells =
                        &self.firm_cells[self.firm_offsets[firm]..self.firm_offsets[firm + 1]];
                    for cell in cells {
                        let worker = cell.worker as usize * stride;
                        for (value, &term) in row
                            .iter_mut()
                            .zip(&left_scratch[worker..worker + rhs_count])
                        {
                            *value -= cell.weight * term;
                        }
                    }
                    if row.iter().any(|value| !value.is_finite()) {
                        return Err(invalid_data("cell Schur product is nonfinite"));
                    }
                }
                Ok(())
            },
        )?;
        crate::interrupt::checkpoint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{BorrowedInterceptEffect, LevelIndex};

    /// A seeded AKM-like design: workers visit a few random firms, with
    /// repeated observations so cells aggregate and weights vary.
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
            for visit in 0..visits {
                let firm = if worker < firms && visit == 0 {
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

    fn limits() -> KernelLimits {
        KernelLimits {
            retained_live_bytes: 0,
            memory: None,
        }
    }

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|value| value.to_bits()).collect()
    }

    // Both constructors, both eliminated sides, narrow and wide RHS with
    // unused stride lanes: the parallel product equals the serial bits at
    // 1, 2, 4 and 8 workers and leaves rows beyond `rhs_count` untouched.
    #[test]
    fn parallel_product_reproduces_serial_bits_at_every_worker_count() {
        let (workers, firms) = (6000, 700);
        let (left, right, weights) = design_codes(workers, firms, 20261027);
        with_design(&left, &right, workers, firms, |design| {
            for side in [0, 1] {
                let (p, q) = if side == 0 {
                    (workers, firms)
                } else {
                    (firms, workers)
                };
                let kernels = [
                    build(
                        design,
                        side,
                        BorrowedMapWeights::Provided(&weights),
                        3.0,
                        limits(),
                    ),
                    build_occurrence(
                        design,
                        side,
                        BorrowedMapWeights::Provided(&weights),
                        3.0,
                        limits(),
                    ),
                ];
                for setup in kernels {
                    let setup = setup.unwrap();
                    let peak = setup.peak.bytes().unwrap();
                    let kernel = setup.kernel.unwrap();
                    assert!(kernel.retained_bytes() <= u128::from(peak));
                    for (rhs_count, stride) in [(1, 1), (3, 5), (8, 8)] {
                        let input: Vec<f64> = (0..q * stride)
                            .map(|index| ((index * 37 % 257) as f64 - 128.0) / 64.0)
                            .collect();
                        let mut expected = vec![7.5; q * stride];
                        let mut scratch = vec![-3.0; p * stride];
                        kernel
                            .apply(&input, &mut expected, &mut scratch, rhs_count, stride)
                            .unwrap();
                        let expected_scratch = scratch.clone();
                        for threads in [1, 2, 4, 8] {
                            crate::ThreadTeam::new(
                                crate::ThreadConfig::from_requested(threads, true).unwrap(),
                            )
                            .install(|| {
                                let mut output = vec![7.5; q * stride];
                                let mut scratch = vec![-3.0; p * stride];
                                kernel
                                    .apply_parallel(
                                        &input,
                                        &mut output,
                                        &mut scratch,
                                        rhs_count,
                                        stride,
                                    )
                                    .unwrap();
                                assert_eq!(bits(&output), bits(&expected), "t{threads}");
                                assert_eq!(bits(&scratch), bits(&expected_scratch));
                                assert_eq!(
                                    kernel.parallel_lanes(rhs_count),
                                    threads > 1 && kernel.cell_count() * rhs_count > 16_384
                                );
                            })
                            .unwrap();
                        }
                    }
                }
            }
        });
    }

    // Cancellation is process-wide, so the check runs in a child process.
    #[test]
    fn parallel_product_rejects_bad_dimensions_nonfinite_input_and_cancellation() {
        const CHILD: &str = "FEREG_CELL_PARALLEL_CANCEL_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::routing_kernel::parallel::tests::parallel_product_rejects_bad_dimensions_nonfinite_input_and_cancellation",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let (left, right, weights) = design_codes(3000, 300, 7);
        with_design(&left, &right, 3000, 300, |design| {
            let kernel = build(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.0,
                limits(),
            )
            .unwrap()
            .kernel
            .unwrap();
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(4, true).unwrap())
                .install(|| {
                    let mut output = vec![0.0; 300 * 2];
                    let mut scratch = vec![0.0; 3000 * 2];
                    let mut input = vec![1.0; 300 * 2];
                    assert!(kernel
                        .apply_parallel(&input[1..], &mut output[1..], &mut scratch, 2, 2)
                        .is_err());
                    assert!(kernel
                        .apply_parallel(&input, &mut output, &mut scratch, 3, 2)
                        .is_err());
                    input[17] = f64::INFINITY;
                    assert!(kernel
                        .apply_parallel(&input, &mut output, &mut scratch, 2, 2)
                        .is_err());
                    input[17] = 1.0;
                    crate::interrupt::request_cancellation();
                    let cancelled = kernel.apply_parallel(&input, &mut output, &mut scratch, 2, 2);
                    crate::interrupt::reset_cancellation();
                    assert_eq!(
                        cancelled.unwrap_err().code,
                        crate::error::ErrorCode::Interrupted
                    );
                    kernel
                        .apply_parallel(&input, &mut output, &mut scratch, 2, 2)
                        .unwrap();
                })
                .unwrap();
        });
    }

    // The firm index is part of the admitted kernel: one byte below the
    // admitted peak refuses construction without touching the inputs.
    #[test]
    fn firm_index_is_admitted_with_the_kernel() {
        let (left, right, weights) = design_codes(2000, 200, 11);
        with_design(&left, &right, 2000, 200, |design| {
            let peak = build(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.0,
                limits(),
            )
            .unwrap()
            .peak
            .bytes()
            .unwrap() as u128;
            let budget = |usable_bytes| KernelLimits {
                retained_live_bytes: 0,
                memory: Some(crate::memory::FeSolverMemoryBudget {
                    execution_bytes: 0,
                    usable_bytes,
                }),
            };
            let n = left.len() as u128;
            assert!(peak >= firm_index_bytes(n, 200));
            assert!(build(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.0,
                budget(peak)
            )
            .unwrap()
            .kernel
            .is_some());
            let refused = build(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.0,
                budget(peak - 1),
            )
            .unwrap();
            assert!(refused.kernel.is_none());
            assert_eq!(refused.refusal, Some(KernelRefusal::MemoryBudget));
        });
    }
}
