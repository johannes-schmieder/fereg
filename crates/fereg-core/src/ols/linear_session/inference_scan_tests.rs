use super::*;
use crate::sandwich::{
    reduce_dense_score_meats, FixedBlockReductionPolicy, RowMajorSandwichDesign,
    SliceSandwichScoreMultiplier, SystemSandwichReservation,
};

#[test]
fn immutable_inference_scan_preserves_bits_boundaries_and_new_writes() {
    for columns in [1, 6, 1025] {
        let n = 37;
        let mut values: Vec<_> = (0..n * columns)
            .map(|i| {
                if i % 19 == 0 {
                    -0.0
                } else {
                    (i as f64 - 700.) / 17.
                }
            })
            .collect();
        let mut scratch = PreparedOlsScratch::create_row_major(n, columns).unwrap();
        scratch.write_linear(0, &values).unwrap();
        let mut storage = RetainedWithinStorage::Scratch(scratch);
        let positions: Vec<_> = (0..columns).collect();
        let design = CertifiedLinearWithinDesign {
            storage: &storage,
            active_positions: &positions,
        };
        let mut output = vec![0.; columns];
        let mut visits = 0;
        design
            .visit_range(1..n - 1, &mut output, |row, output| {
                assert!(output
                    .iter()
                    .zip(&values[row * columns..])
                    .all(|(a, b)| a.to_bits() == b.to_bits()));
                output.fill(f64::NAN);
                visits += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(visits, n - 2);
        let RetainedWithinStorage::Scratch(scratch) = &storage else {
            unreachable!()
        };
        assert_eq!(
            scratch.shared_read_chunks_for_test() as usize,
            ((n - 1) * columns).div_ceil(1024) - columns / 1024
        );
        design
            .visit_range(n..n, &mut output, |_, _| panic!("empty range"))
            .unwrap();
        assert!(design
            .visit_range(0..n + 1, &mut output, |_, _| Ok(()))
            .is_err());
        assert!(design.visit_range(0..1, &mut [], |_, _| Ok(())).is_err());
        let mut visits = 0;
        let error = design
            .visit_range(0..n, &mut output, |row, _| {
                visits += 1;
                if row == 3 {
                    Err(FeregError::new(
                        ErrorCode::Interrupted,
                        "scan callback failure",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(visits, 4);
        let RetainedWithinStorage::Scratch(scratch) = &mut storage else {
            unreachable!()
        };
        values[0] = 41.;
        scratch.write_linear(0, &[values[0]]).unwrap();
        CertifiedLinearWithinDesign {
            storage: &storage,
            active_positions: &positions,
        }
        .visit_range(0..1, &mut output, |_, row| {
            assert_eq!(row[0], 41.);
            Ok(())
        })
        .unwrap();
        let bad: Vec<_> = (0..columns).rev().collect();
        if columns > 1 {
            assert!(CertifiedLinearWithinDesign {
                storage: &storage,
                active_positions: &bad
            }
            .visit_range(0..1, &mut output, |_, _| Ok(()))
            .is_err());
        }
    }
}

#[test]
fn inference_scan_preserves_both_meats_across_weights_blocks_and_teams() {
    let n = 2035;
    for k in [1, 6, 17] {
        let values: Vec<_> = (0..n * k).map(|i| (i as f64 / 19.).sin()).collect();
        let mut scratch = PreparedOlsScratch::create_row_major(n, k).unwrap();
        scratch.write_linear(0, &values).unwrap();
        let storage = RetainedWithinStorage::Scratch(scratch);
        let positions: Vec<_> = (0..k).collect();
        let design = CertifiedLinearWithinDesign {
            storage: &storage,
            active_positions: &positions,
        };
        let dense = RowMajorSandwichDesign::new(&values, n, k).unwrap();
        let production: Vec<_> = (0..n)
            .map(|i| (0.2 + (i % 13) as f64) * (i as f64 / 17.).cos())
            .collect();
        let certificate: Vec<_> = production.iter().map(|x| x * 0.999).collect();
        let scores = SliceSandwichScoreMultiplier::new(&production, &certificate).unwrap();
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for block in [511, 16_384] {
                let policy = FixedBlockReductionPolicy {
                    rows_per_block: block,
                    maximum_blocks: 64,
                    partial_budget_bytes: 32 << 20,
                };
                let expected = pool
                    .install(|| {
                        reduce_dense_score_meats(
                            dense,
                            scores,
                            policy,
                            None,
                            &mut SystemSandwichReservation,
                        )
                    })
                    .unwrap();
                let RetainedWithinStorage::Scratch(scratch) = &storage else {
                    unreachable!()
                };
                let before = scratch.shared_read_chunks_for_test();
                let actual = pool
                    .install(|| {
                        reduce_dense_score_meats(
                            design,
                            scores,
                            policy,
                            None,
                            &mut SystemSandwichReservation,
                        )
                    })
                    .unwrap();
                assert_eq!(actual.production, expected.production);
                assert_eq!(actual.certificate, expected.certificate);
                assert!(
                    scratch.shared_read_chunks_for_test() - before
                        <= (n * k).div_ceil(1024) as u64 + n.div_ceil(block) as u64
                );
            }
        }
    }
}
