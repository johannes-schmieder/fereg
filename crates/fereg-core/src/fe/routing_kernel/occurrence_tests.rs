use super::*;
use crate::fe::{BorrowedInterceptEffect, LevelIndex};

fn with_design(action: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &[f64])) {
    let left = [0, 0, 1, 0, 1, 0, 2, 2];
    let right = [2, 0, 1, 2, 0, 1, 3, 3];
    let weights = [2., 3., 4., 6., 5., 7., 8., 1.];
    let a = LevelIndex::from_codes(&left, 3).unwrap();
    let b = LevelIndex::from_codes(&right, 4).unwrap();
    let design = BorrowedInterceptFeDesign::new(
        left.len(),
        vec![
            BorrowedInterceptEffect::new(&left, 3, &a).unwrap(),
            BorrowedInterceptEffect::new(&right, 4, &b).unwrap(),
        ],
    )
    .unwrap();
    action(&design, &weights);
}
fn limits() -> KernelLimits {
    KernelLimits {
        retained_live_bytes: 0,
        memory: None,
    }
}

#[test]
fn occurrence_order_duplicate_totals_and_independent_dense_schur() {
    with_design(|design, weights| {
        let first = build_occurrence(
            design,
            0,
            BorrowedMapWeights::Provided(weights),
            8.,
            limits(),
        )
        .unwrap();
        let kernel = first.kernel.unwrap();
        assert_eq!(
            kernel
                .cells
                .iter()
                .map(|c| (c.worker, c.firm, c.weight))
                .collect::<Vec<_>>(),
            vec![
                (0, 2, 1.),
                (0, 0, 3. / 8.),
                (0, 1, 7. / 8.),
                (1, 1, 4. / 8.),
                (1, 0, 5. / 8.),
                (2, 3, 9. / 8.)
            ]
        );
        assert_eq!(kernel.cells.capacity(), 6);
        assert_eq!(kernel.offsets, [0, 3, 5, 6]);
        assert!(kernel.retained_bytes() <= first.peak.bytes().unwrap() as u128);
        let left = design.effects()[0].codes();
        let right = design.effects()[1].codes();
        let mut dw = [0.; 3];
        let mut matrix = [[0.; 4]; 4];
        for row in 0..weights.len() {
            dw[left[row] as usize] += weights[row] / 8.;
            matrix[right[row] as usize][right[row] as usize] += weights[row] / 8.;
        }
        // Observation-pair expansion independently eliminates each left FE.
        for i in 0..weights.len() {
            for j in 0..weights.len() {
                if left[i] == left[j] {
                    matrix[right[i] as usize][right[j] as usize] -=
                        weights[i] * weights[j] / 64. / dw[left[i] as usize];
                }
            }
        }
        let lanes = 1100;
        let input = (0..4 * lanes)
            .map(|i| (i % 19) as f64 / 8. - 1.)
            .collect::<Vec<_>>();
        let mut output = vec![0.; input.len()];
        kernel
            .apply(&input, &mut output, &mut vec![0.; 3 * lanes], lanes, lanes)
            .unwrap();
        for firm in 0..4 {
            for lane in 0..lanes {
                let expected = (0..4)
                    .map(|j| matrix[firm][j] * input[j * lanes + lane])
                    .sum::<f64>();
                assert!((output[firm * lanes + lane] - expected).abs() < 4e-15);
            }
        }
        let repeated = build_occurrence(
            design,
            0,
            BorrowedMapWeights::Provided(weights),
            8.,
            limits(),
        )
        .unwrap()
        .kernel
        .unwrap();
        let mut again = vec![0.; input.len()];
        repeated
            .apply(&input, &mut again, &mut vec![0.; 3 * lanes], lanes, lanes)
            .unwrap();
        assert_eq!(
            output.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            again.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    });
}

#[test]
fn occurrence_admission_includes_retained_overlap_and_allocation_is_fatal() {
    with_design(|design, weights| {
        let peak = build_occurrence(
            design,
            0,
            BorrowedMapWeights::Provided(weights),
            8.,
            limits(),
        )
        .unwrap()
        .peak
        .bytes()
        .unwrap() as u128;
        for (available, expected) in [(peak + 99, false), (peak + 100, true)] {
            let attempt = build_occurrence(
                design,
                0,
                BorrowedMapWeights::Provided(weights),
                8.,
                KernelLimits {
                    retained_live_bytes: 100,
                    memory: Some(FeSolverMemoryBudget {
                        execution_bytes: 20,
                        usable_bytes: available + 20,
                    }),
                },
            )
            .unwrap();
            assert_eq!(attempt.kernel.is_some(), expected);
            assert_eq!(
                attempt.refusal,
                (!expected).then_some(KernelRefusal::MemoryBudget)
            );
        }
        CELL_ALLOCATION_FAILURE.set(true);
        assert_eq!(
            build_occurrence(
                design,
                0,
                BorrowedMapWeights::Provided(weights),
                8.,
                limits()
            )
            .err()
            .unwrap()
            .code,
            crate::error::ErrorCode::Allocation
        );
    });
}

#[test]
fn occurrence_and_wide_apply_break_after_real_inner_work() {
    const MARKER: &str = "FEREG_OCCURRENCE_WORK_CANCEL_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .env(MARKER, "1")
            .args(["fe::routing_kernel::occurrence_tests::occurrence_and_wide_apply_break_after_real_inner_work", "--exact", "--test-threads=1", "--nocapture"])
            .stdout(std::process::Stdio::inherit()).stderr(std::process::Stdio::inherit())
            .spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            if std::time::Instant::now() >= deadline {
                let killed = child.kill();
                let reaped = child.wait();
                panic!("cell cancellation child timed out: {killed:?}/{reaped:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    with_design(|design, weights| {
        CELL_CANCEL_PHASE.set(1);
        CELL_COMPLETED_WORK.set(0);
        let error = build_occurrence(
            design,
            0,
            BorrowedMapWeights::Provided(weights),
            8.,
            limits(),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
        // Rows poll once per POLL_UNITS block: all eight rows finish before
        // the next poll (compaction) observes the request.
        assert_eq!(CELL_COMPLETED_WORK.get(), 8 * 32);
        assert!(crate::interrupt::cancellation_requested());
        CELL_CANCEL_PHASE.set(0);
        crate::interrupt::reset_cancellation();
        let kernel = build_occurrence(
            design,
            0,
            BorrowedMapWeights::Provided(weights),
            8.,
            limits(),
        )
        .unwrap()
        .kernel
        .unwrap();
        CELL_COMPLETED_WORK.set(0);
        CELL_CANCEL_PHASE.set(3);
        let error = kernel
            .apply(
                &vec![1.; 4400],
                &mut vec![0.; 4400],
                &mut vec![0.; 3300],
                1100,
                1100,
            )
            .unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
        // Worker 0's first two wide cells (2 x 2200 units) cross one poll block.
        assert_eq!(CELL_COMPLETED_WORK.get(), 4400);
        assert!(crate::interrupt::cancellation_requested());
        CELL_CANCEL_PHASE.set(0);
        crate::interrupt::reset_cancellation();
    });
}

#[test]
fn occurrence_allocation_failure_propagates_through_real_ols_owner() {
    with_design(|design, weights| {
        let mut owner = crate::fe::borrowed_map::ReusableCategoricalMapAbsorber::new(
            BorrowedInterceptFeDesign::new(design.nobs(), design.effects().to_vec()).unwrap(),
            1,
            1,
        )
        .unwrap();
        owner.request_auto_routing(
            crate::fe::RoutingPreset::prefix(crate::fe::routing::CategoricalPrefix::None),
            None,
            0,
        );
        owner
            .request_eager_cells(BorrowedMapWeights::Provided(weights))
            .unwrap();
        CELL_ALLOCATION_FAILURE.set(true);
        let values =
            crate::data::DenseMatrix::from_columns(&[vec![1., 2., 3., 5., 7., 11., 13., 17.]])
                .unwrap();
        let error = owner
            .absorb_ols(
                BorrowedMapWeights::Provided(weights),
                values.into(),
                crate::fe::MapConfig {
                    tolerance: 1e-12,
                    max_iterations: 200,
                    check_every: 1,
                    symmetric: true,
                },
                false,
            )
            .unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Allocation);
        assert!(!CELL_ALLOCATION_FAILURE.get());
    });
}

// Narrow products poll once per block of cells, not per cell: a request made
// after the first gather (phase 3) or scatter (phase 4) cell is observed at
// the next block poll, inside that pass.
#[test]
fn narrow_apply_breaks_inside_gather_and_scatter_blocks() {
    const MARKER: &str = "FEREG_NARROW_APPLY_CANCEL_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(MARKER, "1")
            .args([
                "fe::routing_kernel::occurrence_tests::narrow_apply_breaks_inside_gather_and_scatter_blocks",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let (workers, firms) = (3000, 500);
    let left: Vec<u32> = (0..2 * workers).map(|row| (row / 2) as u32).collect();
    let right: Vec<u32> = (0..2 * workers)
        .map(|row| ((row / 2 + 7 * (row % 2)) % firms) as u32)
        .collect();
    let a = LevelIndex::from_codes(&left, workers).unwrap();
    let b = LevelIndex::from_codes(&right, firms).unwrap();
    let design = BorrowedInterceptFeDesign::new(
        left.len(),
        vec![
            BorrowedInterceptEffect::new(&left, workers, &a).unwrap(),
            BorrowedInterceptEffect::new(&right, firms, &b).unwrap(),
        ],
    )
    .unwrap();
    let kernel = build_occurrence(&design, 0, BorrowedMapWeights::Unit, 1., limits())
        .unwrap()
        .kernel
        .unwrap();
    let cells = kernel.cell_count();
    assert_eq!(cells, 2 * workers);
    let input: Vec<f64> = (0..firms).map(|firm| firm as f64 / 7.).collect();
    let mut expected = vec![0.; firms];
    kernel
        .apply(&input, &mut expected, &mut vec![0.; workers], 1, 1)
        .unwrap();
    for phase in [3, 4] {
        CELL_COMPLETED_WORK.set(0);
        CELL_CANCEL_PHASE.set(phase);
        let error = kernel
            .apply(&input, &mut vec![0.; firms], &mut vec![0.; workers], 1, 1)
            .unwrap_err();
        CELL_CANCEL_PHASE.set(0);
        assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
        assert!(crate::interrupt::cancellation_requested());
        crate::interrupt::reset_cancellation();
        let completed = CELL_COMPLETED_WORK.get();
        assert!(
            completed > 2 && completed < 2 * cells,
            "phase {phase}: {completed}"
        );
        assert!(completed <= POLL_UNITS, "phase {phase}: {completed}");
    }
    let mut again = vec![0.; firms];
    kernel
        .apply(&input, &mut again, &mut vec![0.; workers], 1, 1)
        .unwrap();
    assert_eq!(
        again.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}
