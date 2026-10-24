use super::*;
use crate::fe::auto::direct_lsmr_tolerance;
use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

fn assert_initial_screen_matches_first_certificate_group(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel: bool,
) -> Vec<f64> {
    assert_eq!(
        certificate_parallel_workspaces_fit(design, values, workspace).unwrap(),
        parallel
    );
    assert!(initial_group_rejects(design, weights, values, 1e-11, workspace).unwrap());
    let screened_means = workspace.group_sums.clone();
    let screened_maxima = workspace.rhs_relative_infinity_norms.clone();
    for (reference, (residual, scale)) in workspace
        .reference_norms
        .iter()
        .zip(workspace.residual_norms.iter().zip(&workspace.rhs_scales))
    {
        assert_eq!(reference.to_bits(), residual.to_bits());
        assert_eq!(reference.to_bits(), scale.to_bits());
    }

    // Invoke the actual complete-certificate gather, independently of the
    // screen's route branches, then fold only its canonical first group.
    let effect = &design.effects()[0];
    let columns = values.ncols();
    let (weight, means) = if parallel {
        let levels = effect.n_levels();
        fill_parallel_group_statistics(
            effect,
            weights,
            values,
            &mut workspace.parallel_group_weights[..levels],
            &mut workspace.parallel_group_means[..levels * columns],
            GroupMeanRoute::OnePass,
        )
        .unwrap();
        (
            workspace.parallel_group_weights[0],
            workspace.parallel_group_means[..columns].to_vec(),
        )
    } else {
        let weight = group_means(
            effect.level_index().checked_members(0).unwrap(),
            weights,
            values,
            workspace,
            GroupMeanRoute::OnePass,
        )
        .unwrap();
        (weight, workspace.group_sums.clone())
    };
    let mut maxima = vec![0.0; columns];
    CertificateFold::default()
        .level(
            weight,
            &means,
            &workspace.rhs_scales,
            &mut maxima,
            weights.raw_scale,
        )
        .unwrap();
    for rhs in 0..columns {
        assert_eq!(
            screened_means[rhs].to_bits(),
            means[rhs].to_bits(),
            "mean RHS {rhs}"
        );
        assert_eq!(
            screened_maxima[rhs].to_bits(),
            maxima[rhs].to_bits(),
            "moment RHS {rhs}"
        );
    }
    screened_maxima
}

#[test]
fn initial_unit_rejection_is_a_true_full_certificate_failure_across_group_routes() {
    for workers in [1, 4, 16] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                for rows in [257, 16_385] {
                    let codes: Vec<_> = (0..rows).map(|r| (r % 17) as u32).collect();
                    let index = LevelIndex::from_codes(&codes, 17).unwrap();
                    let source = ValidatedBorrowedInterceptSource::new(&codes, 17, &index).unwrap();
                    let design =
                        BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
                    for columns in [1, 17, 64, 65, 129] {
                        // The constant has a nonzero group mean. A tiny RHS in
                        // the first group forces the whole OnePass group onto
                        // scaled arithmetic; other groups keep its norm moderate.
                        let values = DenseMatrix::from_row_major(
                            rows,
                            columns,
                            (0..rows * columns)
                                .map(|i| match i % columns {
                                    0 => 1.0,
                                    1 if codes[i / columns] == 0 => 1e-200,
                                    2 => -0.0,
                                    _ => (i as f64 * 0.17).sin(),
                                })
                                .collect(),
                        )
                        .unwrap();
                        let weights = validate_weights(BorrowedMapWeights::Unit, rows).unwrap();
                        let mut workspace = BorrowedMapWorkspace::new(
                            &design,
                            rows,
                            columns,
                            &mut SystemWorkspaceReservation,
                        )
                        .unwrap();
                        update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
                        let partial_maxima = assert_initial_screen_matches_first_certificate_group(
                            &design,
                            weights,
                            &values,
                            &mut workspace,
                            workers > 1 && (rows == 16_385 || columns >= 64),
                        );
                        let full =
                            certify_initial(&design, weights, &values, 1e-11, &mut workspace)
                                .unwrap();
                        assert!(!full.passed);
                        assert!(partial_maxima
                            .iter()
                            .zip(&workspace.rhs_relative_infinity_norms)
                            .all(|(part, whole)| part <= whole));
                        // The constant's exact mathematical relative moment is
                        // sqrt(group_size / rows), well clear of the tolerance.
                        let expected = ((rows.div_ceil(17) as f64) / rows as f64).sqrt();
                        assert!((partial_maxima[0] - expected).abs() < 1e-15);
                    }
                }
            })
            .unwrap();
    }
}

#[test]
fn initial_unit_screen_uses_exact_group_route_during_pilot_scratch_loan() {
    let rows = 2049;
    let left: Vec<_> = (0..rows).map(|row| (row % 17) as u32).collect();
    let right: Vec<_> = (0..rows).map(|row| ((row / 7) % 13) as u32).collect();
    let left_index = LevelIndex::from_codes(&left, 17).unwrap();
    let right_index = LevelIndex::from_codes(&right, 13).unwrap();
    let left_source = ValidatedBorrowedInterceptSource::new(&left, 17, &left_index).unwrap();
    let right_source = ValidatedBorrowedInterceptSource::new(&right, 13, &right_index).unwrap();
    for workers in [4, 16] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                let design = BorrowedInterceptFeDesign::new(
                    rows,
                    vec![left_source.effect(), right_source.effect()],
                )
                .unwrap();
                // One-lane admission leaves both parallel banks empty. The
                // installed team must select its route after the real loan.
                let mut owner = ReusableCategoricalMapAbsorber::new(design, 129, 1).unwrap();
                let capacities = owner.capacities();
                let graph = owner.two_fe_pcg.as_ref().unwrap();
                let pointers = (graph.left_work.as_ptr(), graph.left_weights.as_ptr());
                for columns in [17, 64, 65, 129] {
                    for tiny_rhs in [false, true] {
                        let values = DenseMatrix::from_row_major(
                            rows,
                            columns,
                            (0..rows * columns)
                                .map(|i| match i % columns {
                                    0 => 1.0,
                                    1 if tiny_rhs && left[i / columns] == 0 => 1e-200,
                                    2 => -0.0,
                                    _ => (i as f64 * 0.17).sin(),
                                })
                                .collect(),
                        )
                        .unwrap();
                        let weights = validate_weights(BorrowedMapWeights::Unit, rows).unwrap();
                        owner.workspace.set_rhs_count(columns).unwrap();
                        update_workspace_norms(&values, weights, &mut owner.workspace, true)
                            .unwrap();
                        assert!(!certificate_parallel_workspaces_fit(
                            &owner.design,
                            &values,
                            &owner.workspace
                        )
                        .unwrap());
                        owner
                            .with_pilot_group_scratch(columns, |design, workspace| {
                                assert_eq!(workspace.parallel_group_means.as_ptr(), pointers.0);
                                assert_eq!(workspace.parallel_group_weights.as_ptr(), pointers.1);
                                let _ = assert_initial_screen_matches_first_certificate_group(
                                    design, weights, &values, workspace, true,
                                );
                                assert!(
                                    !certify_initial(design, weights, &values, 1e-11, workspace)?
                                        .passed
                                );
                                Ok(())
                            })
                            .unwrap();
                        assert_eq!(owner.capacities(), capacities);
                        assert!(owner.workspace.parallel_group_means.is_empty());
                        assert!(owner.workspace.parallel_group_weights.is_empty());
                        let graph = owner.two_fe_pcg.as_ref().unwrap();
                        assert_eq!(
                            (graph.left_work.as_ptr(), graph.left_weights.as_ptr()),
                            pointers
                        );
                    }
                }
            })
            .unwrap();
    }
}

#[test]
fn initial_rejection_declines_unsupported_norms_and_accepts_supplied_unit_weights() {
    let codes = [0, 0, 1, 1];
    let index = LevelIndex::from_codes(&codes, 2).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).unwrap();
    for weights in [
        BorrowedMapWeights::Unit,
        BorrowedMapWeights::Provided(&[1.0; 4]),
    ] {
        let weights = validate_weights(weights, 4).unwrap();
        for scale in [0.0, 1e-100, 1e100] {
            let values = DenseMatrix::from_row_major(4, 1, vec![scale; 4]).unwrap();
            let mut workspace =
                BorrowedMapWorkspace::new(&design, 4, 1, &mut SystemWorkspaceReservation).unwrap();
            update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
            assert!(
                !initial_group_rejects(&design, weights, &values, 1e-11, &mut workspace).unwrap()
            );
        }
    }
    let weights = validate_weights(BorrowedMapWeights::Provided(&[1.0; 4]), 4).unwrap();
    let values = DenseMatrix::from_row_major(4, 1, vec![1.0; 4]).unwrap();
    let mut workspace =
        BorrowedMapWorkspace::new(&design, 4, 1, &mut SystemWorkspaceReservation).unwrap();
    update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
    assert!(initial_group_rejects(&design, weights, &values, 1e-11, &mut workspace).unwrap());
    assert!(
        !certify_initial(&design, weights, &values, 1e-11, &mut workspace)
            .unwrap()
            .passed
    );
}

#[test]
fn bounded_weight_screen_matches_exact_certificate_routes_and_pilot_loans() {
    let rows = 257;
    let left: Vec<_> = (0..rows).map(|row| (row % 17) as u32).collect();
    let right: Vec<_> = (0..rows).map(|row| (row % 13) as u32).collect();
    let li = LevelIndex::from_codes(&left, 17).unwrap();
    let ri = LevelIndex::from_codes(&right, 13).unwrap();
    let ls = ValidatedBorrowedInterceptSource::new(&left, 17, &li).unwrap();
    let rs = ValidatedBorrowedInterceptSource::new(&right, 13, &ri).unwrap();
    for workers in [1, 4, 16] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                let design =
                    BorrowedInterceptFeDesign::new(rows, vec![ls.effect(), rs.effect()]).unwrap();
                let mut owner = ReusableCategoricalMapAbsorber::new(design, 129, 1).unwrap();
                let capacities = owner.capacities();
                for columns in [1, 17, 64, 65, 129] {
                    for scale in [1.0 / 4_294_967_296.0, 1.0, 4_294_967_296.0] {
                        let raw: Vec<_> = (0..rows)
                            .map(|row| scale * [1.0, 0.25, 1.0 / 4_294_967_296.0][row % 3])
                            .collect();
                        let weights =
                            validate_weights(BorrowedMapWeights::Provided(&raw), rows).unwrap();
                        for tiny_rhs in [false, true] {
                            let values = DenseMatrix::from_row_major(
                                rows,
                                columns,
                                (0..rows * columns)
                                    .map(|i| match i % columns {
                                        0 => 1.0,
                                        1 if tiny_rhs && left[i / columns] == 0 => 1e-200,
                                        _ => (i as f64 * 0.17).sin(),
                                    })
                                    .collect(),
                            )
                            .unwrap();
                            owner.workspace.set_rhs_count(columns).unwrap();
                            update_workspace_norms(&values, weights, &mut owner.workspace, true)
                                .unwrap();
                            owner
                                .with_pilot_group_scratch(columns, |design, workspace| {
                                    let partial =
                                        assert_initial_screen_matches_first_certificate_group(
                                            design,
                                            weights,
                                            &values,
                                            workspace,
                                            workers > 1 && columns >= 64,
                                        );
                                    assert!(
                                        !certify_initial(
                                            design, weights, &values, 1e-11, workspace
                                        )?
                                        .passed
                                    );
                                    assert!(partial
                                        .iter()
                                        .zip(&workspace.rhs_relative_infinity_norms)
                                        .all(|(part, full)| part <= full));
                                    Ok(())
                                })
                                .unwrap();
                            assert_eq!(owner.capacities(), capacities);
                            assert!(owner.workspace.parallel_group_means.is_empty());
                            assert!(owner.workspace.parallel_group_weights.is_empty());
                        }
                    }
                }
            })
            .unwrap();
    }
}

#[test]
fn weighted_initial_screen_respects_inclusive_weight_and_norm_boundaries() {
    let codes = [0, 1];
    let index = LevelIndex::from_codes(&codes, 2).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).unwrap();
    let low = 1.0_f64 / 4_294_967_296.0;
    let high = 4_294_967_296.0_f64;
    for (scale, ratio, eligible) in [
        (low, 1.0, true),
        (f64::from_bits(low.to_bits() - 1), 1.0, false),
        (high, 1.0, true),
        (f64::from_bits(high.to_bits() + 1), 1.0, false),
        (1.0, low, true),
        (1.0, f64::from_bits(low.to_bits() - 1), false),
        (1.0, f64::from_bits(low.to_bits() + 1), true),
    ] {
        let raw = [scale, scale * ratio];
        let weights = validate_weights(BorrowedMapWeights::Provided(&raw), 2).unwrap();
        let values = DenseMatrix::from_row_major(2, 1, vec![1.0, 0.0]).unwrap();
        let mut workspace =
            BorrowedMapWorkspace::new(&design, 2, 1, &mut SystemWorkspaceReservation).unwrap();
        update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
        assert_eq!(
            initial_group_rejects(&design, weights, &values, 1e-11, &mut workspace).unwrap(),
            eligible
        );
        assert!(
            !certify_initial(&design, weights, &values, 1e-11, &mut workspace)
                .unwrap()
                .passed
        );
    }
    for (norm, eligible) in [
        (f64::from_bits(1e-20_f64.to_bits() - 1), false),
        (1e-20, true),
        (1e20, true),
        (f64::from_bits(1e20_f64.to_bits() + 1), false),
    ] {
        let weights = validate_weights(BorrowedMapWeights::Provided(&[1.0; 2]), 2).unwrap();
        let values = DenseMatrix::from_row_major(2, 1, vec![norm, 0.0]).unwrap();
        let mut workspace =
            BorrowedMapWorkspace::new(&design, 2, 1, &mut SystemWorkspaceReservation).unwrap();
        update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
        assert_eq!(workspace.reference_norms[0].to_bits(), norm.to_bits());
        assert_eq!(
            initial_group_rejects(&design, weights, &values, 1e-11, &mut workspace).unwrap(),
            eligible
        );
    }
}

#[test]
fn failing_initial_group_does_not_authorize_skipping_later_certificate_errors() {
    let codes = [0, 1];
    let index = LevelIndex::from_codes(&codes, 2).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).unwrap();
    // The second case stays inside the bounded weight domain: the zero-norm
    // guard must still preserve its later nonzero subnormal group-mean error.
    for (tiny, raw_weights) in [
        (1e-200, [1.0, f64::from_bits(1)]),
        (f64::from_bits(1), [1.0, 0.25]),
    ] {
        let values = DenseMatrix::from_row_major(2, 2, vec![1.0, 0.0, 0.0, tiny]).unwrap();
        let weights = validate_weights(BorrowedMapWeights::Provided(&raw_weights), 2).unwrap();
        let mut workspace =
            BorrowedMapWorkspace::new(&design, 2, 2, &mut SystemWorkspaceReservation).unwrap();
        update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
        assert_eq!(workspace.reference_norms, [1.0, 0.0]);
        assert!(!initial_group_rejects(&design, weights, &values, 1e-8, &mut workspace).unwrap());
        let total = group_means(
            &[0],
            weights,
            &values,
            &mut workspace,
            GroupMeanRoute::OnePass,
        )
        .unwrap();
        let first_relative =
            workspace.group_sums[0].abs() / workspace.reference_norms[0] * total.sqrt();
        assert!(first_relative > 1e-8);
        // The second RHS underflows in the weighted norm but has a nonzero mean
        // in the second group. A first-group rejection screen would hide this
        // existing error instead of preserving the initial certificate boundary.
        let error = certify_initial(&design, weights, &values, 1e-8, &mut workspace).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert!(error.to_string().contains("zero-scale certificate"));
    }
}

#[test]
fn automatic_initial_norm_handoff_preserves_fresh_certificate_and_workspace() {
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for rows in [257, 16_385] {
                    let left: Vec<_> = (0..rows).map(|r| (r % 17) as u32).collect();
                    let right: Vec<_> = (0..rows).map(|r| ((r / 7) % 13) as u32).collect();
                    let li = LevelIndex::from_codes(&left, 17).unwrap();
                    let ri = LevelIndex::from_codes(&right, 13).unwrap();
                    let l = ValidatedBorrowedInterceptSource::new(&left, 17, &li).unwrap();
                    let r = ValidatedBorrowedInterceptSource::new(&right, 13, &ri).unwrap();
                    let design =
                        BorrowedInterceptFeDesign::new(rows, vec![l.effect(), r.effect()]).unwrap();
                    for width in [1, 7, 52] {
                        let values = DenseMatrix::from_row_major(
                            rows,
                            width,
                            (0..rows * width)
                                .map(|i| match (i % width) % 5 {
                                    0 => {
                                        if i % 2 == 0 {
                                            -0.0
                                        } else {
                                            0.0
                                        }
                                    }
                                    1 => 0.25,
                                    2 => (i as f64 * 0.17).sin() * 1e-200,
                                    3 => (i as f64 * 0.17).sin() * 1e200,
                                    _ => (i as f64 * 0.17).sin(),
                                })
                                .collect(),
                        )
                        .unwrap();
                        for weight_scale in [None, Some(1e-100), Some(1.0), Some(1e100)] {
                            let raw = weight_scale.map(|scale| {
                                (0..rows)
                                    .map(|r| scale * 10.0_f64.powi((r % 9) as i32 - 4))
                                    .collect::<Vec<_>>()
                            });
                            let weights = validate_weights(
                                raw.as_deref()
                                    .map_or(BorrowedMapWeights::Unit, BorrowedMapWeights::Provided),
                                rows,
                            )
                            .unwrap();
                            let mut full = BorrowedMapWorkspace::new(
                                &design,
                                rows,
                                width,
                                &mut SystemWorkspaceReservation,
                            )
                            .unwrap();
                            let mut reused = BorrowedMapWorkspace::new(
                                &design,
                                rows,
                                width,
                                &mut SystemWorkspaceReservation,
                            )
                            .unwrap();
                            for workspace in [&mut full, &mut reused] {
                                weighted_column_norms(
                                    &values,
                                    weights,
                                    &mut workspace.reference_norms,
                                    &mut workspace.value_scales,
                                    &mut workspace.group_sums,
                                    &mut [],
                                )
                                .unwrap();
                            }
                            let expected =
                                certify(&design, weights, &values, 1e-10, &mut full).unwrap();
                            let actual =
                                certify_initial(&design, weights, &values, 1e-10, &mut reused)
                                    .unwrap();
                            assert_eq!(actual, expected);
                            assert_eq!(reused.residual_norms, full.residual_norms);
                            assert_eq!(reused.value_scales, full.value_scales);
                            assert_eq!(reused.rhs_scales, full.rhs_scales);
                            assert_eq!(
                                reused.rhs_relative_infinity_norms,
                                full.rhs_relative_infinity_norms
                            );
                            assert_eq!(reused.group_sums, full.group_sums);
                            assert_eq!(reused.group_corrections, full.group_corrections);
                            // A fresh moment check still rejects a non-orthogonal
                            // input; the norm handoff cannot grant a certificate.
                            assert_eq!(actual.passed, width == 1);
                        }
                    }
                }
            });
    }
}

#[derive(Default)]
struct InjectedReservation {
    fail_at: Option<usize>,
    skip_at: Option<usize>,
    calls: usize,
    records: Vec<(usize, &'static str)>,
}

impl WorkspaceReservation for InjectedReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        let call = self.calls;
        self.calls = self.calls.checked_add(1).expect("small reservation count");
        self.records.push((additional, label));
        if self.fail_at == Some(call) {
            return Err(allocation("injected borrowed MAP allocation failure"));
        }
        if self.skip_at == Some(call) {
            return Ok(());
        }
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation("test borrowed MAP reservation failed"))
    }
}

fn config() -> MapConfig {
    MapConfig {
        tolerance: 1e-11,
        max_iterations: 10_000,
        check_every: 1,
        symmetric: true,
    }
}

#[test]
fn checkpoint_schur_copy_respects_exact_transient_admission() {
    let values = DenseMatrix::from_columns(&[vec![1.0, -2.0], vec![3.0, 4.0]]).unwrap();
    let bytes = values.as_slice().len() as u128 * 8;
    assert_eq!(
        checkpoint_schur_copy(&values, bytes - 1).unwrap_err().code,
        ErrorCode::Allocation
    );
    let copy = checkpoint_schur_copy(&values, bytes).unwrap();
    assert_eq!(copy, values);
    assert_ne!(copy.as_slice().as_ptr(), values.as_slice().as_ptr());
    for rows in [16384, 122880, 983040] {
        for rhs in [1, 2, 3, 8, 15, 16, 17, 51, 64] {
            for threads in [1, 4, 16] {
                let estimate =
                    estimate_fe_solver_memory(rows, rhs, &[rows / 4, 128], threads, Some(4))
                        .unwrap();
                assert!(estimate.lsmr_schwarz_bytes >= rows as u128 * rhs as u128 * 8);
            }
        }
    }
}

#[test]
fn checkpoint_schur_reuses_full_progress_and_falls_back_on_untouched_inputs() {
    for width in [1, 2, 3, 8, 15, 16, 17] {
        check_checkpoint_schur_width(width);
    }
}

fn check_checkpoint_schur_width(width: usize) {
    let rows = 16384;
    let first_codes = (0..rows).map(|r| (r % 257) as u32).collect::<Vec<_>>();
    let second_codes = (0..rows)
        .map(|r| ((r * 17 + r / 257) % 127) as u32)
        .collect::<Vec<_>>();
    let first_index = LevelIndex::from_codes(&first_codes, 257).unwrap();
    let second_index = LevelIndex::from_codes(&second_codes, 127).unwrap();
    let first = ValidatedBorrowedInterceptSource::new(&first_codes, 257, &first_index).unwrap();
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, 127, &second_index).unwrap();
    let design =
        || BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()]).unwrap();
    let columns = (0..17)
        .map(|c| {
            (0..rows)
                .map(|r| {
                    ((r * (c + 17) % 997) as f64 / 17.0).sin()
                        + first_codes[r] as f64 * 0.001
                        + second_codes[r] as f64 * 0.01
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let values = DenseMatrix::from_columns(&columns[..width]).unwrap();
    let original = values.clone();
    let supplied = (0..rows).map(|r| 1.0 + (r % 7) as f64).collect::<Vec<_>>();
    let scaled = supplied.iter().map(|w| w * 1024.0).collect::<Vec<_>>();
    let config = MapConfig {
        tolerance: 1e-12,
        ..config()
    };
    for threads in [1, 4, 16] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let mut reusable =
                    ReusableCategoricalMapAbsorber::new(design(), 64, threads).unwrap();
                reusable.enable_checkpoint_lsmr(true);
                let capacities = reusable.capacities();
                for raw in [
                    BorrowedMapWeights::Unit,
                    BorrowedMapWeights::Provided(&supplied),
                    BorrowedMapWeights::Provided(&scaled),
                ] {
                    let expected = absorb_borrowed_intercept_map_certified(
                        &design(),
                        raw,
                        values.clone(),
                        MapConfig {
                            tolerance: 1e-13,
                            ..config
                        },
                    )
                    .unwrap();
                    let solved = reusable.absorb_ppml(raw, values.clone(), config).unwrap();
                    assert_eq!(solved.report.used_solver, FeSolverKind::Map);
                    assert_eq!(solved.report.requested_solver, FeSolverKind::Lsmr);
                    assert_eq!(solved.report.pilot_iterations, 0);
                    assert!(!solved.report.fallback_used);
                    assert_eq!(solved.report.checkpoint_retries, 0);
                    for (&a, &b) in solved
                        .residualized()
                        .as_slice()
                        .iter()
                        .zip(expected.residualized().as_slice())
                    {
                        assert!((a - b).abs() < 2e-10, "{a} != {b}");
                    }
                    // A width-65 owner has no Schur graph and supplies the unchanged
                    // checkpoint LSMR result for the same authoritative RHSs.
                    let mut baseline =
                        ReusableCategoricalMapAbsorber::new(design(), 65, threads).unwrap();
                    baseline.two_fe_pcg = None; // Independent LSMR witness, without the graph attempt.
                    baseline.enable_checkpoint_lsmr(true);
                    let original_lsmr = baseline.absorb_ppml(raw, values.clone(), config).unwrap();
                    for reject_certificate in [false, true] {
                        if reject_certificate {
                            reusable.checkpoint_schur_certificate_failure = true;
                        } else {
                            reusable.checkpoint_schur_iteration_cap = Some(1);
                        }
                        let fallback = reusable.absorb_ppml(raw, values.clone(), config).unwrap();
                        assert_eq!(fallback.report.used_solver, FeSolverKind::Lsmr);
                        assert_eq!(fallback.residualized(), original_lsmr.residualized());
                        assert_eq!(
                            fallback.report.checkpoint_retries,
                            original_lsmr.report.checkpoint_retries + 1
                        );
                        assert!(fallback.report.iterations > original_lsmr.report.iterations);
                        assert!(fallback
                            .report
                            .warnings
                            .iter()
                            .any(|w| w.contains("untouched authoritative inputs")));
                    }
                    if threads == 4 && raw == BorrowedMapWeights::Provided(&supplied) {
                        reusable.checkpoint_schur_iteration_cap = Some(1);
                        reusable.inject_checkpoint_lsmr_certificate_failure();
                        let final_map = reusable.absorb_ppml(raw, values.clone(), config).unwrap();
                        assert_eq!(final_map.report.used_solver, FeSolverKind::Map);
                        assert!(final_map.report.fallback_used);
                        assert_eq!(final_map.report.checkpoint_retries, 2);
                        for message in ["bounded current-weight Schur", "checkpoint LSMR"] {
                            assert!(final_map
                                .report
                                .warnings
                                .iter()
                                .any(|w| w.contains(message)));
                        }
                        for (&a, &b) in final_map
                            .residualized()
                            .as_slice()
                            .iter()
                            .zip(expected.residualized().as_slice())
                        {
                            assert!((a - b).abs() < 2e-10);
                        }
                    }
                    assert_eq!(values, original);
                    assert_eq!(reusable.capacities(), capacities);
                }
                for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
                    reusable.inject_checkpoint_lsmr_fatal_failure(code);
                    assert_eq!(
                        reusable
                            .absorb_ppml(BorrowedMapWeights::Unit, values.clone(), config)
                            .unwrap_err()
                            .code,
                        code
                    );
                    assert!(reusable
                        .absorb_ppml(BorrowedMapWeights::Unit, values.clone(), config)
                        .unwrap()
                        .report
                        .certified());
                    assert_eq!(reusable.capacities(), capacities);
                }
            });
    }
}

#[test]
fn checkpoint_schur_weighted_cut_rejects_after_bounded_progress() {
    let rows = 16384;
    let left_codes = (0..rows).map(|r| (r / 4) as u32).collect::<Vec<_>>();
    let right_codes = (0..rows)
        .map(|r| {
            (match r % 4 {
                0 => r / 4 % 128,
                1 => (r / 4 + 1) % 128,
                _ => (r * 37 + r / 512 * 19) % 128,
            }) as u32
        })
        .collect::<Vec<_>>();
    let left_index = LevelIndex::from_codes(&left_codes, rows / 4).unwrap();
    let right_index = LevelIndex::from_codes(&right_codes, 128).unwrap();
    let left = ValidatedBorrowedInterceptSource::new(&left_codes, rows / 4, &left_index).unwrap();
    let right = ValidatedBorrowedInterceptSource::new(&right_codes, 128, &right_index).unwrap();
    let design = BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
    let values = DenseMatrix::from_columns(
        &(0..17)
            .map(|c| {
                (0..rows)
                    .map(|r| {
                        ((r * (c + 17) % 997) as f64 / 17.0).sin()
                            + (right_codes[r] as f64 * 0.049).sin()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let weights = (0..rows)
        .map(|r| if r % 4 < 2 { 1.0 } else { 1e-10 })
        .collect::<Vec<_>>();
    let mut reusable = ReusableCategoricalMapAbsorber::new(design, 17, 4).unwrap();
    reusable.enable_checkpoint_lsmr(true);
    let capacities = reusable.capacities();
    let (result, cost) = reusable
        .try_checkpoint_schur(
            validate_weights(BorrowedMapWeights::Provided(&weights), rows).unwrap(),
            &values,
            config(),
        )
        .unwrap();
    assert!(result.is_none());
    assert!(cost.contraction > GRAPH_MAX_PROGRESS_CONTRACTION);
    assert_eq!(cost.iterations, GRAPH_PROGRESS_CHECK_ITERATIONS);
    assert!(cost.solve_seconds > 0.0);
    assert!(cost.rejected);
    assert_eq!(reusable.capacities(), capacities);
    for width in [1, 2, 3, 8, 15] {
        let narrow = DenseMatrix::from_columns(
            &(0..width)
                .map(|column| {
                    (0..rows)
                        .map(|row| values[(row, column)])
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let (result, cost) = reusable
            .try_checkpoint_schur(
                validate_weights(BorrowedMapWeights::Provided(&weights), rows).unwrap(),
                &narrow,
                config(),
            )
            .unwrap();
        assert!(result.is_none());
        assert!(cost.contraction > GRAPH_MAX_PROGRESS_CONTRACTION);
        assert_eq!(cost.iterations, GRAPH_PROGRESS_CHECK_ITERATIONS);
        assert!(cost.solve_seconds > 0.0);
        assert_eq!(reusable.capacities(), capacities);
    }
    for width in [0, 18] {
        let narrow = DenseMatrix::zeros(rows, width).unwrap();
        let (result, cost) = reusable
            .try_checkpoint_schur(
                validate_weights(BorrowedMapWeights::Unit, rows).unwrap(),
                &narrow,
                config(),
            )
            .unwrap();
        assert!(result.is_none());
        assert_eq!(cost.iterations, 0);
        assert!(!cost.rejected);
    }
}

#[test]
fn ols_parallel_certificate_matches_canonical_moments_and_errors() {
    let rows = 65_536;
    let codes = (0..rows).map(|r| (r % 8193) as u32).collect::<Vec<_>>();
    let index = LevelIndex::from_codes(&codes, 8193).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 8193, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
    for width in [8, 12, 17] {
        let columns = (0..width)
            .map(|c| {
                (0..rows)
                    .map(|r| {
                        if c == 0 {
                            0.0
                        } else {
                            ((r * (c + 17) % 997) as f64 / 17.0).sin()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let values = DenseMatrix::from_columns(&columns).unwrap();
        let raw_weights = (0..rows)
            .map(|r| 0.01 + (r % 19) as f64 / 7.0)
            .collect::<Vec<_>>();
        for threads in [4, 16] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for weights in [
                        BorrowedMapWeights::Unit,
                        BorrowedMapWeights::Provided(&raw_weights),
                    ] {
                        let weights = validate_weights(weights, rows).unwrap();
                        let mut reservation = SystemWorkspaceReservation;
                        let mut work =
                            BorrowedMapWorkspace::new(&design, rows, width, &mut reservation)
                                .unwrap();
                        let mut serial_work = BorrowedMapWorkspace::new_with_parallelism(
                            &design,
                            rows,
                            width,
                            1,
                            &mut reservation,
                        )
                        .unwrap();
                        for scale in [1000.0, 0.0, 1e-200] {
                            work.reference_norms.fill(scale);
                            work.residual_norms.fill(scale);
                            serial_work.reference_norms.fill(scale);
                            serial_work.residual_norms.fill(scale);
                            let canonical = certify_with_current_norms(
                                &design,
                                weights,
                                &values,
                                1e-8,
                                &mut serial_work,
                            );
                            let maxima = serial_work.rhs_relative_infinity_norms.clone();
                            let parallel = certify_with_current_norms(
                                &design, weights, &values, 1e-8, &mut work,
                            );
                            match (canonical, parallel) {
                                (Ok(a), Ok(b)) => {
                                    assert_eq!(a.passed, b.passed);
                                    assert_eq!(a.absolute_infinity_norm, b.absolute_infinity_norm);
                                    assert_eq!(a.relative_infinity_norm, b.relative_infinity_norm);
                                    assert!(
                                        (a.relative_two_norm / b.relative_two_norm - 1.0).abs()
                                            < 1e-12
                                    );
                                    assert_eq!(maxima, work.rhs_relative_infinity_norms);
                                }
                                (Err(a), Err(b)) => {
                                    assert_eq!(a.code, b.code);
                                    assert_eq!(a.message, b.message);
                                }
                                other => {
                                    panic!("parallel certificate changed outcome: {other:?}")
                                }
                            }
                        }
                    }
                });
        }
    }
}

#[test]
fn ols_schur_readiness_includes_group_weight_certificate_scale() {
    for (request, target) in [
        (1e-8, 1e-10),
        (1e-12, 1e-14),
        (1e-13, 1e-14),
        (1e-14, 1e-14),
        (1e-15, 1e-15),
    ] {
        assert_eq!(direct_lsmr_tolerance(request), target);
        assert!(target <= request);
    }
    let codes = [0_u32; 100];
    let index = LevelIndex::from_codes(&codes, 1).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
    let design =
        BorrowedInterceptFeDesign::new(100, vec![source.effect(), source.effect()]).unwrap();
    let mut reservation = SystemWorkspaceReservation;
    let mut graph = TwoFePcgWorkspace::new(&design, 1, &mut reservation).unwrap();
    let mut workspace = BorrowedMapWorkspace::new(&design, 100, 1, &mut reservation).unwrap();
    graph.right_weights[0] = 100.0;
    graph.residual[0] = 2e-7;
    workspace.reference_norms[0] = 1.0;
    // The mean-only rule would accept 2e-9 at tolerance 1e-8.
    assert!(!two_fe_pcg_rhs_ready(&graph, &workspace, 0, 1, 5e-9, false).unwrap());
    graph.residual[0] = 2e-8;
    assert!(two_fe_pcg_rhs_ready(&graph, &workspace, 0, 1, 5e-9, false).unwrap());
    assert!(
        !two_fe_pcg_rhs_ready(&graph, &workspace, 0, 1, direct_lsmr_tolerance(1e-8), false,)
            .unwrap()
    );
    // Powers of two keep this boundary exactly representable in the proxy.
    graph.right_weights[0] = 64.0;
    graph.residual[0] = 8e-10;
    assert!(two_fe_pcg_rhs_ready(&graph, &workspace, 0, 1, 1e-10, false).unwrap());
}

#[test]
fn ols_schur_accepts_a_passing_certificate_after_recurrence_budget() {
    let left_codes = [0, 0, 0, 0, 1, 1, 1, 1, 2, 2];
    let right_codes = [0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let left_index = LevelIndex::from_codes(&left_codes, 3).unwrap();
    let right_index = LevelIndex::from_codes(&right_codes, 3).unwrap();
    let left = ValidatedBorrowedInterceptSource::new(&left_codes, 3, &left_index).unwrap();
    let right = ValidatedBorrowedInterceptSource::new(&right_codes, 3, &right_index).unwrap();
    let design = BorrowedInterceptFeDesign::new(10, vec![left.effect(), right.effect()]).unwrap();
    let mut owner = ReusableCategoricalMapAbsorber::new(design, 1, 1).unwrap();
    let values = DenseMatrix::from_columns(&[vec![
        11.0, -9.0, 10.0, -10.0, 10.0, -10.0, 10.0, -10.0, 10.0, -10.0,
    ]])
    .unwrap();
    let result = owner
        .absorb_ols(
            BorrowedMapWeights::Unit,
            values.clone().into(),
            MapConfig {
                tolerance: 7e-3,
                max_iterations: 1,
                ..config()
            },
            false,
        )
        .unwrap();
    assert_eq!(result.report().iterations, 1);
    assert!(owner.cg_initial_residual_square[0] != 0.0);
    assert!(result.report().certified());
    assert_eq!(result.report().stop_reason, SolveStopReason::Certified);
    assert_eq!(result.report().used_solver, FeSolverKind::Cg);
    assert!(!result.report().fallback_used);
    assert!(!two_fe_pcg_rhs_ready(
        owner.two_fe_pcg.as_ref().unwrap(),
        &owner.workspace,
        0,
        1,
        direct_lsmr_tolerance(7e-3),
        false,
    )
    .unwrap());
    // The Schur matrix is the three-vertex path Laplacian. With masses
    // [2,4,4], one PCG step has alpha=1.2 and solution [0.6,-0.3,0].
    // The duplicated-edge +/-10 component has zero moments in both effects.
    for (row, fitted) in [0.95, 0.95, 0.05, 0.05, -0.15, -0.15, 0.15, 0.15, 0.0, 0.0]
        .into_iter()
        .enumerate()
    {
        assert!((values[(row, 0)] - result.residualized()[(row, 0)] - fitted).abs() < 1e-14);
    }
}

#[test]
fn ols_norms_are_scaled_and_bit_stable_across_worker_counts() {
    let rows = 16_391;
    let design = BorrowedInterceptFeDesign::new(rows, Vec::new()).unwrap();
    let columns = [
        vec![0.0; rows],
        (0..rows)
            .map(|r| if r % 2 == 0 { 1e200 } else { -1e-200 })
            .collect(),
        (0..rows).map(|r| (r as f64).sin() * 1e-150).collect(),
    ];
    let values = DenseMatrix::from_columns(&columns).unwrap();
    let raw_weights = (0..rows).map(|r| 1.0 + (r % 17) as f64).collect::<Vec<_>>();
    let weights = validate_weights(BorrowedMapWeights::Provided(&raw_weights), rows).unwrap();
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let mut reservation = SystemWorkspaceReservation;
                let mut workspace =
                    BorrowedMapWorkspace::new(&design, rows, 3, &mut reservation).unwrap();
                update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
                workspace.reference_norms
            })
    };
    let serial = run(1);
    for threads in [4, 16] {
        assert_eq!(
            serial.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            run(threads).iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
    }
    for c in 0..3 {
        let scale = columns[c].iter().map(|x| x.abs()).fold(0.0, f64::max);
        let expected = if scale == 0.0 {
            0.0
        } else {
            scale
                * (0..rows)
                    .map(|r| weights.normalized(r) * (columns[c][r] / scale).powi(2))
                    .sum::<f64>()
                    .sqrt()
        };
        assert!((serial[c] - expected).abs() <= expected * 1e-12);
    }
    let mut reservation = SystemWorkspaceReservation;
    let mut workspace = BorrowedMapWorkspace::new(&design, rows, 3, &mut reservation).unwrap();
    let invalid =
        DenseMatrix::from_columns(&[vec![f64::MAX; rows], vec![0.0; rows], vec![0.0; rows]])
            .unwrap();
    assert!(update_workspace_norms(&invalid, weights, &mut workspace, false).is_err());
}

#[test]
fn explicit_lsmr_recurrence_is_direct_and_certificate_stays_requested() {
    let rows = 240;
    let codes = [
        (0..rows).map(|r| (r % 8) as u32).collect::<Vec<_>>(),
        (0..rows).map(|r| ((r / 8) % 7) as u32).collect::<Vec<_>>(),
        (0..rows)
            .map(|r| ((3 * r + r / 5) % 5) as u32)
            .collect::<Vec<_>>(),
    ];
    let indexes = [
        LevelIndex::from_codes(&codes[0], 8).unwrap(),
        LevelIndex::from_codes(&codes[1], 7).unwrap(),
        LevelIndex::from_codes(&codes[2], 5).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&codes[0], 8, &indexes[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[1], 7, &indexes[1]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[2], 5, &indexes[2]).unwrap(),
    ];
    let design = || {
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap()
    };
    let input = DenseMatrix::from_columns(&[
        (0..rows)
            .map(|r| (r as f64 * 0.13).sin())
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|r| (r as f64 * 0.07).cos() + (r % 3) as f64)
            .collect::<Vec<_>>(),
    ])
    .unwrap();
    let raw = (0..rows).map(|r| 1. + (r % 5) as f64).collect::<Vec<_>>();
    let lsmr = LsmrConfig {
        tolerance: 1e-8,
        ..LsmrConfig::default()
    };
    let config = super::super::AbsorbConfig {
        solver: FeSolverKind::Lsmr,
        lsmr,
        ..super::super::AbsorbConfig::default()
    };
    let validated = validate_weights(BorrowedMapWeights::Provided(&raw), rows).unwrap();
    let recurrence = super::super::auto::explicit_lsmr_recurrence(lsmr);
    assert_eq!(recurrence.tolerance, direct_lsmr_tolerance(1e-8));
    // The explicit rule keeps a margin at the 1e-14 final-residual projection
    // and is never looser than the certificate.
    for (requested, expected) in [
        (1e-8, 1e-10),
        (1e-12, 1e-14),
        (5e-13, 5e-15),
        (1e-14, 1e-15),
        (1e-15, 1e-15),
        (1e-16, 1e-16),
    ] {
        let tolerance = super::super::auto::explicit_lsmr_recurrence(LsmrConfig {
            tolerance: requested,
            ..lsmr
        })
        .tolerance;
        assert_eq!(tolerance, expected, "{requested:e}");
        assert!(tolerance <= requested);
    }
    let (rule, rule_report, _, _) = run_borrowed_lsmr(
        &design(),
        validated,
        &input,
        lsmr.tolerance,
        lsmr.max_iterations,
        recurrence,
        None,
        false,
    )
    .unwrap();
    let (_, equal_report, _, _) = run_borrowed_lsmr(
        &design(),
        validated,
        &input,
        lsmr.tolerance,
        lsmr.max_iterations,
        lsmr,
        None,
        false,
    )
    .unwrap();
    // The tighter recurrence never stops earlier, and the certificate keeps
    // the requested threshold rather than the recurrence tolerance.
    assert!(rule_report.iterations >= equal_report.iterations);
    assert!(rule_report.certified());
    assert_eq!(
        rule_report.certificate.passed,
        rule_report.certificate.relative_infinity_norm <= lsmr.tolerance
    );
    let one_shot = absorb_borrowed_intercept_lsmr(
        &design(),
        BorrowedMapWeights::Provided(&raw),
        input.clone(),
        lsmr,
        false,
    )
    .unwrap();
    let mut session =
        BorrowedLsmrAbsorber::new(design(), BorrowedMapWeights::Provided(&raw), config).unwrap();
    let retained = session
        .absorb(
            BorrowedMapWeights::Provided(&raw),
            input.clone(),
            super::super::MapGeometryGeneration {
                sample: 1,
                topology: 1,
                weights: 1,
            },
            config,
            false,
        )
        .unwrap();
    for (residualized, report) in [one_shot.into_parts(), retained.into_parts()] {
        assert_eq!(residualized, rule);
        assert_eq!(report.certificate, rule_report.certificate);
        assert_eq!(report.iterations, rule_report.iterations);
    }
    let error = run_borrowed_lsmr(
        &design(),
        validated,
        &input,
        lsmr.tolerance,
        lsmr.max_iterations,
        LsmrConfig {
            tolerance: f64::NAN,
            ..lsmr
        },
        None,
        false,
    )
    .expect_err("nonfinite recurrence tolerance");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}

#[test]
fn retained_lsmr_weight_generation_revokes_failed_and_changed_frames() {
    let rows = 128;
    let codes = [
        (0..rows).map(|r| (r % 8) as u32).collect::<Vec<_>>(),
        (0..rows).map(|r| ((r / 8) % 7) as u32).collect::<Vec<_>>(),
    ];
    let indexes = [
        LevelIndex::from_codes(&codes[0], 8).unwrap(),
        LevelIndex::from_codes(&codes[1], 7).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&codes[0], 8, &indexes[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[1], 7, &indexes[1]).unwrap(),
    ];
    let design = || {
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap()
    };
    let input = DenseMatrix::from_columns(&[(0..rows)
        .map(|r| (r as f64 * 0.13).sin())
        .collect::<Vec<_>>()])
    .unwrap();
    let config = super::super::AbsorbConfig {
        solver: FeSolverKind::Lsmr,
        lsmr: LsmrConfig {
            tolerance: 1e-12,
            ..LsmrConfig::default()
        },
        ..super::super::AbsorbConfig::default()
    };
    let mut raw = (0..rows).map(|r| 1. + (r % 5) as f64).collect::<Vec<_>>();
    let mut session =
        BorrowedLsmrAbsorber::new(design(), BorrowedMapWeights::Provided(&raw), config).unwrap();
    let generation = |weights| super::super::MapGeometryGeneration {
        sample: 1,
        topology: 1,
        weights,
    };
    let old = session
        .absorb(
            BorrowedMapWeights::Provided(&raw),
            input.clone(),
            generation(1),
            config,
            false,
        )
        .unwrap();
    let warm = session
        .absorb(
            BorrowedMapWeights::Provided(&raw),
            input.clone(),
            generation(1),
            config,
            false,
        )
        .unwrap();
    assert_eq!(old.residualized(), warm.residualized());
    assert_eq!(session.prepared.builds, 1);
    // The storage address is unchanged; only the enclosing generation
    // grants authority to the rebuilt current-weight operator.
    for (row, w) in raw.iter_mut().enumerate() {
        *w = 0.5 + (row * 17 % 11) as f64;
    }
    let expected = absorb_borrowed_intercept_lsmr(
        &design(),
        BorrowedMapWeights::Provided(&raw),
        input.clone(),
        config.lsmr,
        false,
    )
    .unwrap();
    let changed = session
        .absorb(
            BorrowedMapWeights::Provided(&raw),
            input.clone(),
            generation(2),
            config,
            false,
        )
        .unwrap();
    assert_eq!(changed.residualized(), expected.residualized());
    assert_eq!(changed.report().certificate, expected.report().certificate);
    assert_ne!(changed.residualized(), old.residualized());
    raw[0] = f64::NAN;
    assert_eq!(
        session
            .absorb(
                BorrowedMapWeights::Provided(&raw),
                input.clone(),
                generation(3),
                config,
                false,
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
    assert!(session.generation.is_none());
    raw[0] = 0.5;
    let restored = session
        .absorb(
            BorrowedMapWeights::Provided(&raw),
            input.clone(),
            generation(3),
            config,
            false,
        )
        .unwrap();
    assert_eq!(restored.residualized(), expected.residualized());
    let mut refused = config;
    refused.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: 0,
    });
    assert_eq!(
        session
            .absorb(
                BorrowedMapWeights::Provided(&raw),
                input.clone(),
                generation(3),
                refused,
                false,
            )
            .unwrap_err()
            .code,
        ErrorCode::Allocation
    );
    assert!(session.generation.is_none());
    let retry = session
        .absorb(
            BorrowedMapWeights::Provided(&raw),
            input,
            generation(3),
            config,
            false,
        )
        .unwrap();
    assert_eq!(retry.residualized(), expected.residualized());
}

#[test]
fn retained_lsmr_reports_reused_operator_overlap_and_preserves_results() {
    let rows = 256;
    let first = (0..rows).map(|row| (row % 8) as u32).collect::<Vec<_>>();
    let second = (0..rows)
        .map(|row| ((row / 8) % 7) as u32)
        .collect::<Vec<_>>();
    let first_index = LevelIndex::from_codes(&first, 8).unwrap();
    let second_index = LevelIndex::from_codes(&second, 7).unwrap();
    let first_source = ValidatedBorrowedInterceptSource::new(&first, 8, &first_index).unwrap();
    let second_source = ValidatedBorrowedInterceptSource::new(&second, 7, &second_index).unwrap();
    {
        let solver = FeSolverKind::Lsmr;
        let design = BorrowedInterceptFeDesign::new(
            rows,
            vec![first_source.effect(), second_source.effect()],
        )
        .unwrap();
        let mut config = crate::fe::AbsorbConfig {
            solver,
            solver_memory_budget: Some(FeSolverMemoryBudget {
                execution_bytes: 137,
                usable_bytes: 1 << 30,
            }),
            ..crate::fe::AbsorbConfig::default()
        };
        let input = DenseMatrix::from_columns(&[(0..rows)
            .map(|row| (row as f64 * 0.17).sin())
            .collect::<Vec<_>>()])
        .unwrap();
        let mut session =
            BorrowedLsmrAbsorber::new(design, BorrowedMapWeights::Unit, config).unwrap();
        if solver == FeSolverKind::Lsmr {
            let estimate = estimate_fe_solver_memory(
                rows,
                1,
                &[8, 7],
                config
                    .lsmr
                    .threads
                    .resolved()
                    .max(rayon::current_num_threads()),
                config.lsmr.local_size,
            )
            .unwrap();
            let required = estimate.lsmr_bytes(LsmrPreconditioner::Diagonal);
            config.solver_memory_budget = Some(FeSolverMemoryBudget {
                execution_bytes: 137,
                usable_bytes: 136 + required,
            });
            assert_eq!(
                session
                    .absorb(
                        BorrowedMapWeights::Unit,
                        input.clone(),
                        super::super::MapGeometryGeneration {
                            sample: 0,
                            topology: 0,
                            weights: 0
                        },
                        config,
                        false,
                    )
                    .unwrap_err()
                    .code,
                ErrorCode::Allocation
            );
            assert_eq!(session.prepared.builds, 0);
            config.solver_memory_budget.as_mut().unwrap().usable_bytes += 1;
        }
        let cold = session
            .absorb(
                BorrowedMapWeights::Unit,
                input.clone(),
                super::super::MapGeometryGeneration {
                    sample: 0,
                    topology: 0,
                    weights: 0,
                },
                config,
                false,
            )
            .unwrap();
        assert_eq!(session.prepared.builds, 1);
        assert!(cold.report().solver_workspace_peak.bytes().is_some());
        let warm = session
            .absorb(
                BorrowedMapWeights::Unit,
                input,
                super::super::MapGeometryGeneration {
                    sample: 0,
                    topology: 0,
                    weights: 0,
                },
                config,
                false,
            )
            .unwrap();
        assert_eq!(session.prepared.builds, 1);
        assert!(warm.report().solver_workspace_peak.bytes().is_some());
        assert!(session.prepared.retained_bytes > 0);
        let pilot = BorrowedMapWorkspace::planned_payload_bytes(
            &session.design,
            rows,
            1,
            rayon::current_num_threads(),
        )
        .unwrap();
        assert!(
            warm.report().solver_workspace_peak.bytes().unwrap() as u128
                >= pilot + session.prepared.retained_bytes
        );
        assert_eq!(cold.residualized(), warm.residualized());
        assert_eq!(cold.report().certificate, warm.report().certificate);
    }
}

#[test]
fn lsmr_postprocess_admission_preserves_attempt_peak_and_checks_parent_overlap() {
    let estimate = estimate_fe_solver_memory(128, 3, &[16, 8], 1, Some(4)).unwrap();
    let required = estimate.lsmr_bytes(LsmrPreconditioner::Off);
    let parent = 137;
    let base = 211;
    let budget = FeSolverMemoryBudget {
        execution_bytes: base,
        usable_bytes: base + parent + required,
    };
    let mut memory =
        BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Off, Some(budget), parent)
            .unwrap();
    assert_eq!(memory.peak.bytes(), Some((parent + required) as u64));
    memory.include(required).unwrap();
    let prior = memory.peak;
    assert_eq!(
        memory.include(required + 1).unwrap_err().code,
        ErrorCode::Allocation
    );
    assert_eq!(memory.peak, prior);
    assert_eq!(
        memory.include(u128::MAX).unwrap_err().code,
        ErrorCode::Allocation
    );
    assert_eq!(memory.peak, prior);
    let short = FeSolverMemoryBudget {
        usable_bytes: budget.usable_bytes - 1,
        ..budget
    };
    assert!(
        BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Off, Some(short), parent)
            .is_err()
    );
    memory.budget = None;
    memory.include_payload(123, required).unwrap();
    assert_eq!(
        memory.peak.bytes(),
        Some((parent + required + 123 + estimate.lsmr_result_metadata_bytes) as u64)
    );
}

#[test]
fn memory_plan_includes_coefficient_layouts_and_all_map_banks() {
    let codes = [0, 0, 1, 1];
    let index = LevelIndex::from_codes(&codes, 2).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
    for effects in [1, 3, 512] {
        let design =
            BorrowedInterceptFeDesign::new(4, (0..effects).map(|_| source.effect()).collect())
                .unwrap();
        for rhs in [1, 19] {
            let batch = FeCoefficientBatch::intercept_zeros(&design, rhs).unwrap();
            let estimate =
                estimate_fe_solver_memory(4, rhs, &vec![2; effects], 1, Some(4)).unwrap();
            assert_eq!(estimate.coefficient_batch_bytes, batch.retained_bytes());
            assert!(estimate.coefficient_batch_bytes > (effects * 2 * rhs * 8) as u128);
        }
    }
    let rows = 16_384;
    let codes: Vec<u32> = (0..rows).map(|r| (r % 127) as u32).collect();
    let index = LevelIndex::from_codes(&codes, 127).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 127, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
    for threads in [1, 4] {
        for rhs in [1, 19, 65] {
            let workspace = BorrowedMapWorkspace::new_with_parallelism(
                &design,
                rows,
                rhs,
                threads,
                &mut SystemWorkspaceReservation,
            )
            .unwrap();
            assert_eq!(
                workspace.payload_peak().unwrap().bytes().unwrap() as u128,
                BorrowedMapWorkspace::planned_payload_bytes(&design, rows, rhs, threads).unwrap()
            );
            if rhs == 1 {
                let matrix = DenseMatrix::zeros(rows, 1).unwrap();
                let coefficient = FeCoefficientBatch::intercept_zeros(&design, 1).unwrap();
                assert_eq!(
                    borrowed_cg_recovery_workspace_bytes(rows, 1, 127, 127, threads).unwrap(),
                    workspace.payload_peak().unwrap().bytes().unwrap() as u128
                        + 4 * matrix.storage_capacity() as u128 * 8
                        + 4 * coefficient.retained_bytes()
                        + 8 * 8,
                );
            }
        }
    }
}

#[test]
fn retained_lsmr_reuses_preparation_and_preserves_each_batch() {
    let rows = 2048;
    let codes = [97_usize, 89, 9].map(|levels| {
        (0..rows)
            .map(|r| ((r * 37 + r / 11) % levels) as u32)
            .collect::<Vec<_>>()
    });
    let indexes = codes
        .iter()
        .zip([97, 89, 9])
        .map(|(c, levels)| LevelIndex::from_codes(c, levels).unwrap())
        .collect::<Vec<_>>();
    let sources = codes
        .iter()
        .zip(&indexes)
        .zip([97, 89, 9])
        .map(|((c, index), levels)| {
            ValidatedBorrowedInterceptSource::new(c, levels, index).unwrap()
        })
        .collect::<Vec<_>>();
    let make_design = || {
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap()
    };
    let raw_weights = (0..rows).map(|r| 0.5 + (r % 7) as f64).collect::<Vec<_>>();
    for threads in [1, 7] {
        let team = crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
        );
        team.install(|| {
            for weights in [
                BorrowedMapWeights::Unit,
                BorrowedMapWeights::Provided(&raw_weights),
            ] {
                for solver in [FeSolverKind::Lsmr] {
                    let config = super::super::AbsorbConfig {
                        solver,
                        map: config(),
                        lsmr: LsmrConfig {
                            tolerance: 1e-12,
                            preconditioner: LsmrPreconditioner::Schwarz,
                            ..LsmrConfig::default()
                        },
                        retain_coefficients: true,
                        ..super::super::AbsorbConfig::default()
                    };
                    let mut session =
                        BorrowedLsmrAbsorber::new(make_design(), weights, config).unwrap();
                    for (batch, columns) in [1, 3, 2].into_iter().enumerate() {
                        let rhs = DenseMatrix::from_columns(
                            &(0..columns)
                                .map(|j| {
                                    (0..rows)
                                        .map(|r| {
                                            (r as f64 * (j + batch + 3) as f64 * 0.137).sin()
                                                + codes[0][r] as f64 * 0.01
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                        let expected = if solver == FeSolverKind::Auto {
                            project_borrowed_intercept_structural(
                                &make_design(),
                                weights,
                                rhs.clone().into(),
                                config.map,
                                config.lsmr,
                                None,
                                false,
                                true,
                                false,
                                false,
                                None,
                                None,
                            )
                        } else {
                            absorb_borrowed_intercept_lsmr(
                                &make_design(),
                                weights,
                                rhs.clone(),
                                config.lsmr,
                                true,
                            )
                        }
                        .unwrap();
                        let actual = session
                            .absorb(
                                weights,
                                rhs,
                                super::super::MapGeometryGeneration {
                                    sample: 0,
                                    topology: 0,
                                    weights: 0,
                                },
                                config,
                                false,
                            )
                            .unwrap();
                        assert_eq!(actual.residualized(), expected.residualized());
                        assert_eq!(actual.report().certificate, expected.report().certificate);
                        assert_eq!(actual.report().iterations, expected.report().iterations);
                        assert_eq!(actual.into_full_parts().2, expected.into_full_parts().2);
                        assert_eq!(session.prepared.builds, 1);
                        assert_eq!(session.prepared.batches, batch + 1);
                    }
                    let mut constrained = config;
                    constrained.solver_memory_budget = Some(FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: 0,
                    });
                    let mut rejected =
                        BorrowedLsmrAbsorber::new(make_design(), weights, constrained).unwrap();
                    let rhs = DenseMatrix::from_columns(&[(0..rows)
                        .map(|r| (r as f64 * 0.137).sin())
                        .collect::<Vec<_>>()])
                    .unwrap();
                    assert_eq!(
                        rejected
                            .absorb(
                                weights,
                                rhs,
                                super::super::MapGeometryGeneration {
                                    sample: 0,
                                    topology: 0,
                                    weights: 0
                                },
                                constrained,
                                false,
                            )
                            .unwrap_err()
                            .code,
                        ErrorCode::Allocation
                    );
                    assert_eq!(rejected.prepared.builds, 0);
                }
            }
        })
        .unwrap();
    }
}

#[test]
fn reusable_one_fe_matches_weighted_group_means_and_reuses_warm_residual() {
    for threads in [1, 4, 16] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let n = 16_384;
            let codes: Vec<u32> = (0..n).map(|row| (row / 8) as u32).collect();
            let index = LevelIndex::from_codes(&codes, n / 8).unwrap();
            let source = ValidatedBorrowedInterceptSource::new(&codes, n / 8, &index).unwrap();
            let design = BorrowedInterceptFeDesign::new(n, vec![source.effect()]).unwrap();
            let narrow_design = BorrowedInterceptFeDesign::new(n, vec![source.effect()]).unwrap();
            let narrow = ReusableCategoricalMapAbsorber::new(narrow_design, 3, threads).unwrap();
            assert!(narrow.two_fe_pcg.is_none());
            drop(narrow);
            let mut projector = ReusableCategoricalMapAbsorber::new(design, 17, threads).unwrap();
            assert!(projector.two_fe_pcg.is_none());

            let capacities = projector.capacities();
            for (frame, width) in [(0, 17), (1, 1), (2, 17)] {
                let weights: Vec<f64> = (0..n).map(|r| 1.0 + ((r + frame) % 7) as f64).collect();
                let columns: Vec<Vec<f64>> = (0..width)
                    .map(|j| {
                        (0..n)
                            .map(|r| {
                                if j == 0 {
                                    (r / 8) as f64
                                } else {
                                    ((r * (j + 1) + frame) % 19) as f64
                                }
                            })
                            .collect()
                    })
                    .collect();
                let raw = DenseMatrix::from_columns(&columns).unwrap();
                let actual = projector
                    .absorb_ppml(BorrowedMapWeights::Provided(&weights), raw, config())
                    .unwrap();
                assert!(actual.report().certified());
                assert_eq!(actual.report().iterations, 1);
                // Keep the legacy matrix indices and numerical traversal order explicit.
                #[allow(clippy::needless_range_loop)]
                for column in 0..width {
                    for group in 0..n / 8 {
                        let rows = group * 8..group * 8 + 8;
                        let denominator: f64 = rows.clone().map(|r| weights[r]).sum();
                        let mean = rows
                            .clone()
                            .map(|r| weights[r] * columns[column][r])
                            .sum::<f64>()
                            / denominator;
                        for row in rows {
                            assert!(
                                (actual.residualized()[(row, column)]
                                    - (columns[column][row] - mean))
                                    .abs()
                                    < 1e-11
                            );
                        }
                    }
                }
                let within = actual.residualized().clone();
                let warm = projector
                    .absorb_ppml(
                        BorrowedMapWeights::Provided(&weights),
                        within.clone(),
                        config(),
                    )
                    .unwrap();
                assert_eq!(warm.report().iterations, 0);
                assert_eq!(warm.residualized(), &within);
                assert_eq!(projector.capacities(), capacities);
            }
        });
    }
}

#[test]
fn reusable_selected_norms_preserve_mapping_and_reject_invalid_selections() {
    let codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let index = LevelIndex::from_codes(&codes, 4).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(codes.len(), vec![source.effect()]).unwrap();
    let mut projector = ReusableCategoricalMapAbsorber::new(design, 5, 1).unwrap();
    let capacities = projector.capacities();
    let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0];
    let matrix = || {
        DenseMatrix::from_columns(&[
            vec![1.0, 3.0, 2.0, -1.0, 5.0, 4.0, 6.0, 8.0],
            vec![1.0; 8],
            vec![1e140, 2e140, -3e140, 1e140, 2e140, 5e140, -1e140, 3e140],
            vec![0.0; 8],
            vec![1e-140, 2e-140, 5e-140, 3e-140, 0.0, -1e-140, 2e-140, 4e-140],
        ])
        .unwrap()
    };
    let mut norms = [-1.0; 3];
    for selection in [[4, 0, 2], [2, 2, 0]] {
        let (actual, reused) = projector
            .absorb_with_selected_norms(
                BorrowedMapWeights::Provided(&weights),
                matrix(),
                config(),
                8.0,
                selection.into_iter(),
                &mut norms,
            )
            .unwrap();
        assert!(reused);
        for (position, &column) in selection.iter().enumerate() {
            let expected = (0..codes.len()).fold(0.0_f64, |norm, row| {
                norm.hypot((weights[row] / 8.0).sqrt() * actual.residualized()[(row, column)])
            });
            assert!((norms[position] / expected - 1.0).abs() < 2e-15);
        }
    }
    let saved = norms;
    assert!(projector
        .absorb_with_selected_norms(
            BorrowedMapWeights::Provided(&weights),
            matrix(),
            config(),
            8.0,
            [0, 5, 2].into_iter(),
            &mut norms,
        )
        .is_err());
    assert_eq!(norms, saved);
    assert!(projector
        .absorb_with_selected_norms(
            BorrowedMapWeights::Provided(&weights),
            matrix(),
            config(),
            8.0,
            0..2,
            &mut norms,
        )
        .is_err());
    assert_eq!(norms, saved);
    let (_, reused) = projector
        .absorb_with_selected_norms(
            BorrowedMapWeights::Provided(&weights),
            matrix(),
            config(),
            4.0,
            0..3,
            &mut norms,
        )
        .unwrap();
    assert!(!reused);
    assert_eq!(norms, saved);
    assert_eq!(projector.capacities(), capacities);
}

#[test]
fn reusable_one_fe_trailing_norms_follow_current_values_weights_and_width() {
    let codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let index = LevelIndex::from_codes(&codes, 4).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(codes.len(), vec![source.effect()]).unwrap();
    let mut reusable = ReusableCategoricalMapAbsorber::new(design, 3, 1).unwrap();
    let capacities = reusable.capacities();
    let columns = [
        vec![1.0, 2.0, 3.0, 5.0, -1.0, 4.0, 6.0, 3.0],
        vec![1e140, 2e140, -3e140, 1e140, 2e140, 5e140, -1e140, 3e140],
        vec![1e-140, 2e-140, 5e-140, 3e-140, 0.0, -1e-140, 2e-140, 4e-140],
    ];
    let first = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0];
    let next = [8e100, 1e100, 7e100, 2e100, 6e100, 3e100, 5e100, 4e100];
    let mut output = [0.0; 2];
    for (weights, scale) in [
        (BorrowedMapWeights::Unit, 1.0),
        (BorrowedMapWeights::Provided(&first), 8.0),
        (BorrowedMapWeights::Provided(&next), 8e100),
    ] {
        for width in [3, 1, 2, 3] {
            let norms = &mut output[..width - 1];
            let (actual, reused) = reusable
                .absorb_with_selected_norms(
                    weights,
                    DenseMatrix::from_columns(&columns[..width]).unwrap(),
                    config(),
                    scale,
                    1..width,
                    norms,
                )
                .unwrap();
            assert!(reused);
            assert!(actual.report().iterations > 0);
            for (column, &norm) in norms.iter().enumerate() {
                // Hypot is independent of the scaled-square accumulator
                // and remains finite at both extreme column scales.
                let expected = (0..codes.len()).fold(0.0_f64, |sum, row| {
                    let w = match weights {
                        BorrowedMapWeights::Unit => 1.0,
                        BorrowedMapWeights::Provided(w) => w[row] / scale,
                    };
                    sum.hypot(w.sqrt() * actual.residualized()[(row, column + 1)])
                });
                assert!((norm / expected - 1.0).abs() < 2e-15);
            }
            let (values, _) = actual.into_parts();
            let saved = norms.to_vec();
            norms.fill(-1.0);
            let (warm, reused) = reusable
                .absorb_with_selected_norms(weights, values, config(), scale, 1..width, norms)
                .unwrap();
            assert!(reused);
            assert_eq!(warm.report().iterations, 0);
            assert_eq!(norms, saved.as_slice());
            assert_eq!(reusable.capacities(), capacities);
        }
    }
}

#[test]
fn reusable_trailing_norms_do_not_publish_on_mismatch_or_failure() {
    let codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let index = LevelIndex::from_codes(&codes, 4).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
    let second_codes = [0_u32, 1, 0, 1, 0, 1, 0, 1];
    let second_index = LevelIndex::from_codes(&second_codes, 2).unwrap();
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).unwrap();
    let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0];
    let values = || {
        DenseMatrix::from_columns(&[
            vec![1.0, 3.0, 2.0, -1.0, 5.0, 4.0, 6.0, 8.0],
            vec![2.0, -1.0, 5.0, 3.0, 4.0, 6.0, -2.0, 7.0],
        ])
        .unwrap()
    };
    for multi_fe in [false, true] {
        let effects = if multi_fe {
            vec![source.effect(), second.effect()]
        } else {
            vec![source.effect()]
        };
        let design = BorrowedInterceptFeDesign::new(codes.len(), effects).unwrap();
        let mut reusable = ReusableCategoricalMapAbsorber::new(design, 2, 1).unwrap();
        reusable.enable_checkpoint_lsmr(true);
        let mut norms = [-1.0];
        for scale in [8.0, 4.0] {
            let saved = norms;
            let (_, reused) = reusable
                .absorb_with_selected_norms(
                    BorrowedMapWeights::Provided(&weights),
                    values(),
                    config(),
                    scale,
                    1..2,
                    &mut norms,
                )
                .unwrap();
            assert_eq!(reused, !multi_fe && scale == 8.0);
            if !reused {
                assert_eq!(norms, saved);
            }
        }
        let saved = norms;
        let mut invalid = values();
        invalid[(0, 0)] = f64::NAN;
        assert!(reusable
            .absorb_with_selected_norms(
                BorrowedMapWeights::Unit,
                invalid,
                config(),
                1.0,
                1..2,
                &mut norms,
            )
            .is_err());
        assert_eq!(norms, saved);
        let mut oversized = [-1.0; 3];
        assert!(reusable
            .absorb_with_selected_norms(
                BorrowedMapWeights::Unit,
                values(),
                config(),
                1.0,
                0..3,
                &mut oversized,
            )
            .is_err());
        assert_eq!(oversized, [-1.0; 3]);
    }
}

#[test]
fn pilot_group_scratch_matches_weighted_ring_oracle_and_restores_on_failure() {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut cells = Vec::new();
    for component in 0..2 {
        for a in 0..16 {
            for b in [a, (a + 1) % 16] {
                for replicate in 0..64 {
                    left.push((component * 16 + a) as u32);
                    right.push((component * 16 + b) as u32);
                    cells.push((component, a, b, replicate));
                }
            }
        }
    }
    let n = cells.len();
    let rhs = 8;
    assert!(n < PARALLEL_PROJECTION_MIN_ROWS && n * rhs >= PARALLEL_PROJECTION_MIN_ROWS);
    let left_index = LevelIndex::from_codes(&left, 32).unwrap();
    let right_index = LevelIndex::from_codes(&right, 32).unwrap();
    let left_source = ValidatedBorrowedInterceptSource::new(&left, 32, &left_index).unwrap();
    let right_source = ValidatedBorrowedInterceptSource::new(&right, 32, &right_index).unwrap();
    let design = || {
        BorrowedInterceptFeDesign::new(n, vec![left_source.effect(), right_source.effect()])
            .unwrap()
    };
    let input = DenseMatrix::from_row_major(
        n,
        rhs,
        cells
            .iter()
            .flat_map(|&(_, a, b, r)| {
                (0..rhs).map(move |j| {
                    if j == rhs - 1 {
                        return 0.;
                    }
                    let residual = if r % 2 == 0 { 1. } else { -1. }
                        * 0.001
                        * (j + 1) as f64
                        * (r / 2 + 1) as f64;
                    ((a * (j + 1)) as f64 / 5.).sin() + ((b * (j + 2)) as f64 / 7.).cos() + residual
                })
            })
            .collect(),
    )
    .unwrap();
    let serial_team = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let parallel_team = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let mut serial = ReusableCategoricalMapAbsorber::new(design(), rhs, 1).unwrap();
    // A one-lane admission has no parallel banks. Exercise the existing
    // PCG bank loan on a wider installed team; ordinary common-plan
    // admission now owns parallel banks even below the old row boundary.
    let mut parallel = ReusableCategoricalMapAbsorber::new(design(), rhs, 1).unwrap();
    let capacities = parallel.capacities();
    assert!(parallel.workspace.parallel_group_means.is_empty());
    assert!(parallel.workspace.parallel_group_weights.is_empty());
    let pointers = {
        let graph = parallel.two_fe_pcg.as_ref().unwrap();
        (graph.left_work.as_ptr(), graph.left_weights.as_ptr())
    };
    for panic in [false, true] {
        let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            parallel_team.install(|| {
                parallel.with_pilot_group_scratch(rhs, |_, workspace| {
                    assert_eq!(workspace.parallel_group_means.len(), 32 * rhs);
                    assert_eq!(workspace.parallel_group_weights.len(), 32);
                    workspace.parallel_group_means.fill(f64::NAN);
                    workspace.parallel_group_weights.fill(f64::NAN);
                    if panic {
                        panic!("injected pilot scratch unwind");
                    }
                    Err::<(), _>(invalid_data("injected pilot scratch error"))
                })
            })
        }));
        if panic {
            assert!(attempt.is_err());
        } else {
            assert_eq!(attempt.unwrap().unwrap_err().code, ErrorCode::InvalidData);
        }
        assert_eq!(parallel.capacities(), capacities);
        assert!(parallel.workspace.parallel_group_means.is_empty());
        assert!(parallel.workspace.parallel_group_weights.is_empty());
        let graph = parallel.two_fe_pcg.as_ref().unwrap();
        assert_eq!(
            (graph.left_work.as_ptr(), graph.left_weights.as_ptr()),
            pointers
        );
    }
    let config = MapConfig {
        tolerance: 1e-12,
        ..config()
    };
    for frame in 0..3 {
        let weights: Vec<_> = cells
            .iter()
            .map(|&(component, a, b, r)| {
                let mass = [[1., 1.], [0.03, 30.], [3., 0.2]][frame][component];
                mass * (1. + a as f64 / 10.)
                    * (1. + b as f64 / 17.)
                    * (1. + (r / 2) as f64 / 13. + frame as f64 / 7.)
            })
            .collect();
        let solve = |owner: &mut ReusableCategoricalMapAbsorber<'_>| {
            owner
                .absorb_ppml(
                    BorrowedMapWeights::Provided(&weights),
                    input.clone(),
                    config,
                )
                .unwrap()
        };
        let a = serial_team.install(|| solve(&mut serial));
        let b = parallel_team.install(|| solve(&mut parallel));
        assert!(b.report().certified());
        assert!(b.report().iterations > 0);
        assert_eq!(a.residualized(), b.residualized());
        assert_eq!(a.report().certificate, b.report().certificate);
        for (row, &(_, _, _, r)) in cells.iter().enumerate() {
            for j in 0..rhs {
                let expected = if j == rhs - 1 {
                    0.
                } else {
                    (if r % 2 == 0 { 1. } else { -1. })
                        * 0.001
                        * (j + 1) as f64
                        * (r / 2 + 1) as f64
                };
                assert!((b.residualized()[(row, j)] - expected).abs() < 1e-10);
            }
        }
        assert_eq!(parallel.capacities(), capacities);
    }
}

#[test]
fn direct_two_fe_schur_matches_weighted_contrasts_across_frames_and_widths() {
    check_direct_two_fe_weighted_contrasts(false, false);
}

#[test]
fn checkpoint_schur_preserves_component_contrasts_across_frames_and_widths() {
    check_direct_two_fe_weighted_contrasts(true, false);
}

fn check_direct_two_fe_weighted_contrasts(checkpoint: bool, ols: bool) {
    // Paired rows share both FE labels and have reciprocal weighted
    // contrasts. The exact WLS residual is known without either solver.
    let mut cells = Vec::new();
    for component in 0..2 {
        for a in 0..8 {
            for b in [a, (a + 1) % 8] {
                for replicate in 0..2 {
                    cells.push((component, a, b, replicate));
                }
            }
        }
    }
    let left: Vec<_> = cells
        .iter()
        .map(|&(c, a, _, _)| (8 * c + a) as u32)
        .collect();
    let right: Vec<_> = cells
        .iter()
        .map(|&(c, _, b, _)| (8 * c + b) as u32)
        .collect();
    let indices = [
        LevelIndex::from_codes(&left, 16).unwrap(),
        LevelIndex::from_codes(&right, 16).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&left, 16, &indices[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&right, 16, &indices[1]).unwrap(),
    ];
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let design = || {
                    BorrowedInterceptFeDesign::new(
                        cells.len(),
                        sources.iter().map(|s| s.effect()).collect(),
                    )
                    .unwrap()
                };
                let ordinary = ReusableCategoricalMapAbsorber::new(design(), 67, threads).unwrap();
                assert!(!ordinary
                    .two_fe_pcg
                    .as_ref()
                    .unwrap()
                    .component_roots
                    .is_empty());
                assert_eq!(ordinary.ppml_operator_work(), Some(0));
                let mut owner = ReusableCategoricalMapAbsorber::new(design(), 67, threads).unwrap();
                assert_eq!(
                    owner.capacities().accelerator_state,
                    ordinary.capacities().accelerator_state
                );
                assert_eq!(owner.ppml_operator_work(), Some(0));
                if checkpoint {
                    owner.enable_checkpoint_lsmr(true);
                }
                let capacities = owner.capacities();
                for (frame, width) in [65, 3, 1, 67].into_iter().enumerate() {
                    let pair_ratio = 2. + frame as f64;
                    let masses = [[1., 1.], [1e-8, 1.], [1., 1e-10], [3., 0.2]][frame];
                    let weights: Vec<_> = cells
                        .iter()
                        .map(|&(c, a, b, r)| {
                            masses[c]
                                * (1. + a as f64 / 3.)
                                * (1. + b as f64 / 7.)
                                * (1. + (r / 2) as f64)
                                * if r % 2 == 0 { 1. } else { pair_ratio }
                        })
                        .collect();
                    let expected = |r: usize, j: usize| {
                        if j == 7 {
                            return 0.;
                        }
                        let scale = if r % 2 == 0 { 1. } else { -1. / pair_ratio };
                        scale * 0.01 * (j + 1) as f64 * (r / 2 + 1) as f64
                    };
                    let mut input = DenseMatrix::zeros(cells.len(), width).unwrap();
                    for (row, &(_, a, b, r)) in cells.iter().enumerate() {
                        for j in 0..width {
                            let scale = if j == 1 { 1000. } else { 1. };
                            input[(row, j)] = if j == 7 {
                                0.
                            } else {
                                scale
                                    * (((a * (j + 1)) as f64).sin() + ((b * (j + 2)) as f64).cos())
                                    + expected(r, j)
                            };
                        }
                    }
                    if frame == 0 && !checkpoint {
                        let attempt = owner.absorb_ppml(
                            BorrowedMapWeights::Provided(&weights),
                            input.clone(),
                            MapConfig {
                                tolerance: 1e-12,
                                max_iterations: 1,
                                ..config()
                            },
                        );
                        assert_eq!(attempt.unwrap_err().code, ErrorCode::InvalidData);
                        assert_eq!(owner.capacities(), capacities);
                    }
                    let map = MapConfig {
                        tolerance: 1e-12,
                        ..config()
                    };
                    let result = if ols {
                        owner.absorb_ols(
                            BorrowedMapWeights::Provided(&weights),
                            input.into(),
                            map,
                            false,
                        )
                    } else {
                        owner.absorb_ppml(BorrowedMapWeights::Provided(&weights), input, map)
                    }
                    .unwrap();
                    assert!(result.report().certified());
                    if checkpoint {
                        assert_eq!(result.report().used_solver, FeSolverKind::Map);
                    }
                    assert!(owner.ppml_operator_work().unwrap() > 0);
                    assert_eq!(result.report().pilot_iterations, 0);
                    for (row, &(_, _, _, r)) in cells.iter().enumerate() {
                        for j in 0..width {
                            assert!(
                                (result.residualized()[(row, j)] - expected(r, j)).abs() < 1e-8,
                                "frame={frame} row={row} rhs={j}"
                            );
                        }
                    }
                    assert_eq!(owner.capacities(), capacities);
                }
            });
    }
}

#[test]
fn reusable_categorical_map_matches_one_shot_across_weight_frames_and_rhs_widths() {
    let first_codes = [0_u32, 0, 1, 1, 2, 2, 0, 1, 2];
    let second_codes = [0_u32, 1, 0, 1, 0, 1, 2, 2, 2];
    let first_index = LevelIndex::from_codes(&first_codes, 3).expect("first index");
    let second_index = LevelIndex::from_codes(&second_codes, 3).expect("second index");
    let first =
        ValidatedBorrowedInterceptSource::new(&first_codes, 3, &first_index).expect("first source");
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, 3, &second_index)
        .expect("second source");
    let columns = [
        vec![1.0, 2.0, 4.0, 8.0, 3.0, 5.0, 9.0, 7.0, 6.0],
        vec![2.0, -1.0, 3.0, 0.5, 4.0, 1.0, -2.0, 8.0, 6.0],
        vec![0.0, 1.0, 0.5, 2.0, -1.0, 3.0, 4.0, -2.0, 5.0],
    ];
    let first_weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0, 9.0];
    let second_weights = [9.0, 2.0, 8.0, 3.0, 7.0, 4.0, 6.0, 5.0, 1.0];

    let expected_design =
        BorrowedInterceptFeDesign::new(first_codes.len(), vec![first.effect(), second.effect()])
            .expect("expected design");
    let oracle_config = MapConfig {
        tolerance: 1e-14,
        ..config()
    };
    let expected = absorb_borrowed_intercept_map_certified(
        &expected_design,
        BorrowedMapWeights::Provided(&first_weights),
        DenseMatrix::from_columns(&columns).expect("expected matrix"),
        oracle_config,
    )
    .expect("one-shot map");

    let reusable_design =
        BorrowedInterceptFeDesign::new(first_codes.len(), vec![first.effect(), second.effect()])
            .expect("reusable design");
    let mut reusable = ReusableCategoricalMapAbsorber::new(reusable_design, columns.len(), 1)
        .expect("reusable map");
    let initial_capacities = reusable.capacities();
    let actual = reusable
        .absorb_ppml(
            BorrowedMapWeights::Provided(&first_weights),
            DenseMatrix::from_columns(&columns).expect("actual matrix"),
            config(),
        )
        .expect("reusable first frame");
    assert!(actual.report().certified());
    for (&left, &right) in actual
        .residualized()
        .as_slice()
        .iter()
        .zip(expected.residualized().as_slice())
    {
        assert!(
            (left - right).abs() <= 1e-12,
            "{left} != {right}; report={:?}",
            actual.report()
        );
    }

    let narrower = [columns[0].clone(), columns[2].clone()];
    let expected_design =
        BorrowedInterceptFeDesign::new(first_codes.len(), vec![first.effect(), second.effect()])
            .expect("second expected design");
    let expected = absorb_borrowed_intercept_map_certified(
        &expected_design,
        BorrowedMapWeights::Provided(&second_weights),
        DenseMatrix::from_columns(&narrower).expect("second expected matrix"),
        oracle_config,
    )
    .expect("second one-shot map");
    let actual = reusable
        .absorb_ppml(
            BorrowedMapWeights::Provided(&second_weights),
            DenseMatrix::from_columns(&narrower).expect("second actual matrix"),
            config(),
        )
        .expect("reusable second frame");
    assert_eq!(reusable.capacities(), initial_capacities);
    for (&left, &right) in actual
        .residualized()
        .as_slice()
        .iter()
        .zip(expected.residualized().as_slice())
    {
        assert!((left - right).abs() <= 1e-12, "{left} != {right}");
    }
}

#[test]
fn component_checkpoint_rechecks_small_drift_before_accepting_warm_residuals() {
    // A repeated-row contrast is exactly orthogonal to the explicit design.
    // A tiny FE term in the low-weight component passes a global norm check
    // despite a material predictor error; component scaling must also govern
    // the zero-iteration shortcut, not just the iterative solve.
    let codes: Vec<Vec<u32>> = (0..3)
        .map(|effect| {
            (0..16)
                .map(|row| {
                    let component = row / 8;
                    let level = if effect == 1 {
                        row / 2 % 2
                    } else {
                        row / 4 % 2
                    };
                    (2 * component + level) as u32
                })
                .collect()
        })
        .collect();
    let indices: Vec<_> = codes
        .iter()
        .map(|c| LevelIndex::from_codes(c, 4).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(&indices)
        .map(|(c, index)| ValidatedBorrowedInterceptSource::new(c, 4, index).unwrap())
        .collect();
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for dimensions in [2, 3] {
                    let design = BorrowedInterceptFeDesign::new(
                        16,
                        sources[..dimensions].iter().map(|s| s.effect()).collect(),
                    )
                    .unwrap();
                    let mut projector =
                        ReusableCategoricalMapAbsorber::new(design, 2, threads).unwrap();
                    projector.enable_checkpoint_lsmr(true);
                    projector.enable_component_checkpoint_scaling();
                    for hot in [0, 1, 0] {
                        let weights: Vec<_> = (0..16)
                            .map(|row| if row / 8 == hot { 1e13 } else { 1. })
                            .collect();
                        let contrast: Vec<_> = (0..16)
                            .map(|row| if row % 2 == 0 { 1. } else { -1. })
                            .collect();
                        for drift in [1e-8, 0.] {
                            let response: Vec<_> = (0..16)
                                .map(|row| {
                                    contrast[row]
                                        + if row / 8 == hot {
                                            0.
                                        } else {
                                            drift * (1. + codes[0][row] as f64)
                                        }
                                })
                                .collect();
                            let fit = projector
                                .absorb_ppml(
                                    BorrowedMapWeights::Provided(&weights),
                                    DenseMatrix::from_columns(&[response, vec![0.; 16]]).unwrap(),
                                    config(),
                                )
                                .unwrap();
                            assert!(fit.report().certified());
                            assert_eq!(fit.report().iterations == 0, drift == 0.);
                            // Keep the legacy matrix indices and numerical traversal order explicit.
                            #[allow(clippy::needless_range_loop)]
                            for row in 0..16 {
                                assert!(
                                    (fit.residualized()[(row, 0)] - contrast[row]).abs() < 1e-12
                                );
                                assert_eq!(fit.residualized()[(row, 1)], 0.);
                            }
                        }
                    }
                }
            });
    }
}

#[test]
fn ppml_component_checkpoint_preserves_exact_projection_across_weight_frames() {
    // The repeated-row contrast is orthogonal to every explicit FE column.
    // Thus its residual is known analytically, independently of either solver.
    let mut codes = [Vec::new(), Vec::new(), Vec::new()];
    for component in 0..2_u32 {
        for a in 0..2_u32 {
            for b in 0..3_u32 {
                for c in 0..4_u32 {
                    for _ in 0..2 {
                        codes[0].push(a + 2 * component);
                        codes[1].push(b + 3 * component);
                        codes[2].push(c + 4 * component);
                    }
                }
            }
        }
    }
    let indices = [
        LevelIndex::from_codes(&codes[0], 4).unwrap(),
        LevelIndex::from_codes(&codes[1], 6).unwrap(),
        LevelIndex::from_codes(&codes[2], 8).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&codes[0], 4, &indices[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[1], 6, &indices[1]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[2], 8, &indices[2]).unwrap(),
    ];
    let rows = codes[0].len();
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let design = BorrowedInterceptFeDesign::new(
                    rows,
                    sources.iter().map(|s| s.effect()).collect(),
                )
                .unwrap();
                let mut projector =
                    ReusableCategoricalMapAbsorber::new(design, 2, threads).unwrap();
                projector.enable_checkpoint_lsmr(true);
                projector.enable_component_checkpoint_scaling();
                for frame in 0..3 {
                    let weights: Vec<_> = (0..rows)
                        .map(|r| {
                            let mass = if (r < rows / 2) == (frame % 2 == 0) {
                                1e13
                            } else {
                                1.
                            };
                            mass * (1. + (r / 2 % 7) as f64)
                                * if r % 2 == 0 { 1. } else { 2. + frame as f64 }
                        })
                        .collect();
                    let expected: Vec<_> = (0..rows)
                        .map(|r| {
                            if r % 2 == 0 {
                                0.5
                            } else {
                                -0.5 / (2. + frame as f64)
                            }
                        })
                        .collect();
                    let response: Vec<_> = (0..rows)
                        .map(|r| {
                            expected[r] + 0.3 * codes[0][r] as f64 - 0.2 * codes[1][r] as f64
                                + 0.1 * codes[2][r] as f64
                        })
                        .collect();
                    for fallback in [false, true] {
                        if fallback {
                            projector.inject_checkpoint_lsmr_certificate_failure();
                        }
                        let fit = projector
                            .absorb_ppml(
                                BorrowedMapWeights::Provided(&weights),
                                DenseMatrix::from_columns(&[response.clone(), vec![0.; rows]])
                                    .unwrap(),
                                config(),
                            )
                            .unwrap();
                        assert!(fit.report().certified());
                        assert_eq!(fit.report().fallback_used, fallback);
                        // Keep the legacy matrix indices and numerical traversal order explicit.
                        #[allow(clippy::needless_range_loop)]
                        for r in 0..rows {
                            assert!((fit.residualized()[(r, 0)] - expected[r]).abs() < 1e-10);
                            assert_eq!(fit.residualized()[(r, 1)], 0.);
                        }
                    }
                }
            });
    }
}

#[test]
fn selective_checkpoint_retry_preserves_weighted_oracle_and_bounded_full_fallback() {
    let mut codes = [Vec::new(), Vec::new(), Vec::new()];
    for component in 0..2_u32 {
        for a in 0..2_u32 {
            for b in 0..3_u32 {
                for c in 0..4_u32 {
                    for _ in 0..2 {
                        codes[0].push(a + 2 * component);
                        codes[1].push(b + 3 * component);
                        codes[2].push(c + 4 * component);
                    }
                }
            }
        }
    }
    let indices = [
        LevelIndex::from_codes(&codes[0], 4).unwrap(),
        LevelIndex::from_codes(&codes[1], 6).unwrap(),
        LevelIndex::from_codes(&codes[2], 8).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&codes[0], 4, &indices[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[1], 6, &indices[1]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[2], 8, &indices[2]).unwrap(),
    ];
    let rows = codes[0].len();
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for dimensions in [2, 3] {
                    let design = BorrowedInterceptFeDesign::new(
                        rows,
                        sources[..dimensions].iter().map(|s| s.effect()).collect(),
                    )
                    .unwrap();
                    for frame in 0..3 {
                        let weights: Vec<_> = (0..rows)
                            .map(|r| {
                                (1. + (r / 2 % 7) as f64)
                                    * if r % 2 == 0 { 1. } else { 2. + frame as f64 }
                            })
                            .collect();
                        // Each paired contrast is orthogonal to every explicit
                        // FE column under these weights, independent of LSMR.
                        let contrast: Vec<_> = (0..rows)
                            .map(|r| {
                                if r % 2 == 0 {
                                    0.5
                                } else {
                                    -0.5 / (2. + frame as f64)
                                }
                            })
                            .collect();
                        let columns: Vec<Vec<_>> = (0..8)
                            .map(|k| {
                                (0..rows)
                                    .map(|r| {
                                        if k == 7 {
                                            0.
                                        } else {
                                            contrast[r] * (k + 1) as f64 + 0.3 * codes[0][r] as f64
                                                - 0.2 * codes[1][r] as f64
                                        }
                                    })
                                    .collect()
                            })
                            .collect();
                        let original = DenseMatrix::from_columns(&columns).unwrap();
                        let weight_frame =
                            validate_weights(BorrowedMapWeights::Provided(&weights), rows).unwrap();
                        for refuse in [false, true] {
                            let mut workspace = BorrowedMapWorkspace::new(
                                &design,
                                rows,
                                8,
                                &mut SystemWorkspaceReservation,
                            )
                            .unwrap();
                            weighted_column_norms(
                                &original,
                                weight_frame,
                                &mut workspace.reference_norms,
                                &mut workspace.value_scales,
                                &mut workspace.group_sums,
                                &mut [],
                            )
                            .unwrap();
                            let mut batch = CheckpointLsmrBatch::new(
                                &design,
                                weight_frame,
                                &original,
                                LsmrConfig::default(),
                            )
                            .unwrap();
                            batch.solve_attempt(1e-12, None, 1e-10).unwrap();
                            let builds = batch.prepared.builds;
                            assert!(builds > 0);
                            let before = batch.candidate.as_ref().unwrap().as_slice().to_vec();
                            for row in 0..rows {
                                batch.candidate.as_mut().unwrap()[(row, 3)] += 0.125;
                            }
                            let failed = certify(
                                &design,
                                weight_frame,
                                batch.candidate.as_ref().unwrap(),
                                1e-10,
                                &mut workspace,
                            )
                            .unwrap();
                            assert!(!failed.passed);
                            assert_eq!(
                                workspace
                                    .rhs_relative_infinity_norms
                                    .iter()
                                    .filter(|&&v| v > 1e-10)
                                    .count(),
                                1
                            );
                            if refuse {
                                batch.selective_allowance = Some(0);
                            }
                            batch
                                .solve_attempt(
                                    1e-14,
                                    Some(&workspace.rhs_relative_infinity_norms),
                                    1e-10,
                                )
                                .unwrap();
                            assert_eq!(batch.prepared.builds, builds);
                            assert_eq!(
                                &batch.attempted_rhs[..batch.attempt_count],
                                &[8, if refuse { 8 } else { 1 }]
                            );
                            let candidate = batch.candidate.as_ref().unwrap();
                            assert!(
                                certify(&design, weight_frame, candidate, 1e-10, &mut workspace)
                                    .unwrap()
                                    .passed
                            );
                            for row in 0..rows {
                                for k in 0..8 {
                                    let expected = if k == 7 {
                                        0.
                                    } else {
                                        contrast[row] * (k + 1) as f64
                                    };
                                    assert!((candidate[(row, k)] - expected).abs() < 1e-10);
                                    if !refuse && k != 3 {
                                        assert_eq!(
                                            candidate[(row, k)].to_bits(),
                                            before[row * 8 + k].to_bits()
                                        );
                                    }
                                }
                            }
                            assert_eq!(
                                original.as_slice(),
                                DenseMatrix::from_columns(&columns).unwrap().as_slice()
                            );
                            assert!(batch.memory.peak.bytes().unwrap() as u128 <= batch.allowance);
                        }
                    }
                }
            });
    }
}

#[test]
fn checkpoint_lsmr_recertifies_and_numerical_failure_falls_back_transactionally() {
    let first_codes = [0_u32, 0, 1, 1, 2, 2, 0, 1, 2];
    let second_codes = [0_u32, 1, 0, 1, 0, 1, 2, 2, 2];
    let first_index = LevelIndex::from_codes(&first_codes, 3).expect("first index");
    let second_index = LevelIndex::from_codes(&second_codes, 3).expect("second index");
    let first =
        ValidatedBorrowedInterceptSource::new(&first_codes, 3, &first_index).expect("first source");
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, 3, &second_index)
        .expect("second source");
    let design =
        BorrowedInterceptFeDesign::new(first_codes.len(), vec![first.effect(), second.effect()])
            .expect("design");
    let columns = [
        vec![1.0, 2.0, 4.0, 8.0, 3.0, 5.0, 9.0, 7.0, 6.0],
        vec![2.0, -1.0, 3.0, 0.5, 4.0, 1.0, -2.0, 8.0, 6.0],
    ];
    let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0, 9.0];
    let mut reusable =
        ReusableCategoricalMapAbsorber::new(design, columns.len(), 1).expect("reusable projector");
    reusable.two_fe_pcg = None; // Isolate LSMR recertification; graph progress has its own witness.
    reusable.enable_checkpoint_lsmr(true);

    reusable.inject_checkpoint_lsmr_certificate_failure();
    let fallback = reusable
        .absorb_ppml(
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("fallback values"),
            config(),
        )
        .expect("certified MAP fallback");
    assert_eq!(fallback.report().requested_solver, FeSolverKind::Lsmr);
    assert_eq!(fallback.report().used_solver, FeSolverKind::Map);
    assert!(fallback.report().fallback_used);
    assert_eq!(fallback.report().checkpoint_retries, 1);
    assert!(fallback.report().certified());

    let accelerated = reusable
        .absorb_ppml(
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("accelerated values"),
            config(),
        )
        .expect("recertified LSMR checkpoint");
    assert_eq!(accelerated.report().requested_solver, FeSolverKind::Lsmr);
    assert_eq!(accelerated.report().used_solver, FeSolverKind::Lsmr);
    assert!(!accelerated.report().fallback_used);
    assert_eq!(accelerated.report().checkpoint_retries, 0);
    assert!(accelerated.report().certification_seconds >= 0.0);
    for (&left, &right) in accelerated
        .residualized()
        .as_slice()
        .iter()
        .zip(fallback.residualized().as_slice())
    {
        assert!((left - right).abs() <= 5e-10, "{left} != {right}");
    }

    for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
        reusable.inject_checkpoint_lsmr_certificate_failure();
        reusable.checkpoint_lsmr_retry_error = Some(code);
        let error = reusable
            .absorb_ppml(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("failure inputs"),
                config(),
            )
            .expect_err("retry failures remain fatal");
        assert_eq!(error.code, code);
        let recovered = reusable
            .absorb_ppml(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("recovery inputs"),
                config(),
            )
            .expect("failed retry leaves reusable canonical state usable");
        assert_eq!(recovered.residualized(), accelerated.residualized());
        assert_eq!(recovered.report().checkpoint_retries, 0);
    }
}

#[test]
fn sparse_two_fe_pcg_acceleration_matches_certified_map() {
    let rows = 256_usize;
    let first_levels = 80_usize;
    let second_levels = 64_usize;
    let first_codes = (0..rows)
        .map(|row| (row % first_levels) as u32)
        .collect::<Vec<_>>();
    let second_codes = (0..rows)
        .map(|row| ((row / first_levels + 7 * row) % second_levels) as u32)
        .collect::<Vec<_>>();
    let first_index =
        LevelIndex::from_codes(&first_codes, first_levels).expect("first sparse index");
    let second_index =
        LevelIndex::from_codes(&second_codes, second_levels).expect("second sparse index");
    let first = ValidatedBorrowedInterceptSource::new(&first_codes, first_levels, &first_index)
        .expect("first sparse source");
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, second_levels, &second_index)
        .expect("second sparse source");
    let columns = [
        (0..rows)
            .map(|row| ((row * 13 % 71) as f64).sin() + row as f64 / rows as f64)
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((row * 5 % 43) as f64).cos() - 0.25 * (row % 9) as f64)
            .collect::<Vec<_>>(),
        vec![1.0; rows],
    ];
    let weights = (0..rows)
        .map(|row| 1.0 + (row % 17) as f64 / 11.0)
        .collect::<Vec<_>>();
    let oracle_design = BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
        .expect("sparse oracle design");
    let oracle = absorb_borrowed_intercept_map_certified(
        &oracle_design,
        BorrowedMapWeights::Provided(&weights),
        DenseMatrix::from_columns(&columns).expect("sparse oracle values"),
        MapConfig {
            tolerance: 1e-13,
            max_iterations: 20_000,
            check_every: 4,
            symmetric: true,
        },
    )
    .expect("sparse MAP oracle");
    let design = BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
        .expect("sparse reusable design");
    let mut reusable = ReusableCategoricalMapAbsorber::new(design, columns.len(), 1)
        .expect("sparse reusable absorber");
    assert!(reusable.two_fe_pcg.is_some());
    let capacities = reusable.capacities();
    let actual = reusable
        .absorb_ppml(
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("sparse reusable values"),
            config(),
        )
        .expect("sparse reusable PCG absorption");
    assert!(actual.report().certified());
    assert_eq!(reusable.capacities(), capacities);
    for (&left, &right) in actual
        .residualized()
        .as_slice()
        .iter()
        .zip(oracle.residualized().as_slice())
    {
        assert!((left - right).abs() <= 5e-11, "{left} != {right}");
    }
    for tolerance in [1e-8, 1e-14] {
        let map = MapConfig {
            tolerance,
            ..config()
        };
        let legacy = reusable
            .absorb_ppml(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).unwrap(),
                map,
            )
            .unwrap();
        let final_residual = reusable
            .absorb_ols(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).unwrap().into(),
                map,
                true,
            )
            .unwrap();
        assert_eq!(legacy.residualized(), final_residual.residualized());
        assert_eq!(
            legacy.report().iterations,
            final_residual.report().iterations
        );
        let ordinary = reusable
            .absorb_ols(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).unwrap().into(),
                map,
                false,
            )
            .unwrap();
        assert!(ordinary.report().certified());
        let structural = project_borrowed_intercept_structural(
            &oracle_design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).unwrap().into(),
            map,
            LsmrConfig::default(),
            None,
            false,
            false,
            false,
            false,
            None,
            None,
        )
        .unwrap();
        assert_eq!(ordinary.residualized(), structural.residualized());
        assert_eq!(ordinary.report().iterations, structural.report().iterations);
        eprintln!(
            "adaptive sparse witness: tolerance={tolerance} maximum_absolute_error={:.17e}",
            ordinary
                .residualized()
                .as_slice()
                .iter()
                .zip(oracle.residualized().as_slice())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max)
        );
        // Johannes accepted the adaptive default's projection tradeoff on
        // 2026-10-13. Retain a quantitative default witness and the original
        // H8 bound at the strict request; the publication certificate is unchanged.
        let projection_bound = if tolerance == 1e-8 { 1e-8 } else { 5e-9 };
        for (&left, &right) in ordinary
            .residualized()
            .as_slice()
            .iter()
            .zip(oracle.residualized().as_slice())
        {
            assert!(
                (left - right).abs() <= projection_bound,
                "{left} != {right}"
            );
        }
        if tolerance == 1e-8 {
            assert_eq!(ordinary.residualized(), legacy.residualized());
            assert_eq!(ordinary.report().iterations, legacy.report().iterations);
        }
    }
}

#[test]
fn weighted_graph_operator_and_projection_match_independent_paths() {
    let rows = 20_000;
    for disconnected in [false, true] {
        let left_codes = (0..rows)
            .map(|r| ((r * 8191 % rows) / 5) as u32)
            .collect::<Vec<_>>();
        let right_codes = (0..rows)
            .map(|r| {
                let row = r * 8191 % rows;
                let firm = if row % 5 < 3 {
                    row / 5 * 17
                } else {
                    row / 5 * 389 + row % 5 * 97
                };
                if disconnected {
                    (firm % 499 + usize::from(row / 5 >= 2000) * 499) as u32
                } else {
                    (firm % 997) as u32
                }
            })
            .collect::<Vec<_>>();
        let right_levels = if disconnected { 998 } else { 997 };
        let left_index = LevelIndex::from_codes(&left_codes, 4000).unwrap();
        let right_index = LevelIndex::from_codes(&right_codes, right_levels).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, 4000, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, right_levels, &right_index)
            .unwrap();
        let design =
            || BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        for scale in [1e-100, 1.0, 1e100] {
            let raw = (0..rows)
                .map(|r| scale * (0.125 + (r % 17) as f64 / 11.0))
                .collect::<Vec<_>>();
            let weights = validate_weights(BorrowedMapWeights::Provided(&raw), rows).unwrap();
            for rhs_count in [3, 5, 8, 52] {
                let stride = rhs_count + 1;
                let mut graph =
                    TwoFePcgWorkspace::new(&design(), stride, &mut SystemWorkspaceReservation)
                        .unwrap();
                for row in 0..rows {
                    graph.left_weights[left_codes[row] as usize] += weights.normalized(row);
                    graph.right_weights[right_codes[row] as usize] += weights.normalized(row);
                }
                for (index, value) in graph.direction.iter_mut().enumerate() {
                    *value = ((index * 17 % 101) as f64 - 50.0) / 37.0;
                }
                rayon::ThreadPoolBuilder::new()
                    .num_threads(1)
                    .build()
                    .unwrap()
                    .install(|| {
                        graph
                            .apply_schur(&design(), weights, rhs_count, stride)
                            .unwrap();
                    });
                let expected_left = graph.left_work.clone();
                let expected_operator = graph.operator.clone();
                let capacity = graph.capacity_elements();
                for threads in [8, 16] {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| {
                            graph
                                .apply_schur_parallel(&design(), weights, rhs_count, stride)
                                .unwrap();
                        });
                    for (actual, expected) in [
                        (&graph.left_work, &expected_left),
                        (&graph.operator, &expected_operator),
                    ] {
                        for (&actual, &expected) in actual.iter().zip(expected) {
                            assert!((actual - expected).abs() < 2e-12, "{actual} != {expected}");
                        }
                    }
                }
                assert_eq!(graph.capacity_elements(), capacity);
            }
        }
        let raw = (0..rows)
            .map(|r| 0.125 + (r % 17) as f64 / 11.0)
            .collect::<Vec<_>>();
        let weights = BorrowedMapWeights::Provided(&raw);
        let values = DenseMatrix::from_columns(
            &(0..8)
                .map(|column| {
                    (0..rows)
                        .map(|row| {
                            ((row * (column + 13) % 1009) as f64).sin()
                                + f64::from(left_codes[row]) * 0.01
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let expected = absorb_borrowed_intercept_map_certified(
            &design(),
            weights,
            values.clone(),
            MapConfig {
                tolerance: 1e-13,
                ..config()
            },
        )
        .unwrap();
        for threads in [4, 8, 16] {
            let actual = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    ReusableCategoricalMapAbsorber::new(design(), values.ncols(), threads)
                        .unwrap()
                        .absorb_ppml(
                            weights,
                            values.clone(),
                            MapConfig {
                                tolerance: 1e-14,
                                ..config()
                            },
                        )
                        .unwrap()
                });
            assert!(actual.report().certified());
            let distance = actual
                .residualized()
                .as_slice()
                .iter()
                .zip(expected.residualized().as_slice())
                .map(|(actual, expected)| (actual - expected).abs())
                .fold(0.0_f64, f64::max);
            eprintln!("weighted graph projection: disconnected={disconnected}, threads={threads}, distance={distance}");
            assert!(
                distance < 2e-9,
                "weighted graph projection distance {distance}"
            );
        }
    }
}

#[test]
fn unit_graph_projection_matches_independent_map() {
    let rows = 20_000;
    for disconnected in [false, true] {
        let left_codes = (0..rows)
            .map(|r| ((r * 8191 % rows) / 5) as u32)
            .collect::<Vec<_>>();
        let right_codes = (0..rows)
            .map(|r| {
                let row = r * 8191 % rows;
                let firm = if row % 5 < 3 {
                    row / 5 * 17
                } else {
                    row / 5 * 389 + row % 5 * 97
                };
                if disconnected {
                    (firm % 499 + usize::from(row / 5 >= 2000) * 499) as u32
                } else {
                    (firm % 997) as u32
                }
            })
            .collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, 4000).unwrap();
        let right_levels = if disconnected { 998 } else { 997 };
        let right_index = LevelIndex::from_codes(&right_codes, right_levels).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, 4000, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, right_levels, &right_index)
            .unwrap();
        let design =
            || BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        for rhs_count in [5, 7, 12] {
            let columns = (0..rhs_count)
                .map(|column| {
                    (0..rows)
                        .map(|row| {
                            ((row * (column + 13) % 1009) as f64).sin()
                                + f64::from(left_codes[row]) * 0.01
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let values = DenseMatrix::from_columns(&columns).unwrap();
            let expected = absorb_borrowed_intercept_map_certified(
                &design(),
                BorrowedMapWeights::Unit,
                values.clone(),
                MapConfig {
                    tolerance: 1e-13,
                    ..config()
                },
            )
            .unwrap();
            let solve = |threads, tolerance| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        ReusableCategoricalMapAbsorber::new(design(), values.ncols(), threads)
                            .unwrap()
                            .absorb_ppml(
                                BorrowedMapWeights::Unit,
                                values.clone(),
                                MapConfig {
                                    tolerance,
                                    ..config()
                                },
                            )
                            .unwrap()
                    })
            };
            let four_workers = solve(4, 1e-8);
            let eight_workers = solve(8, 1e-8);
            let distance = |a: &CertifiedBorrowedMap, b: &CertifiedBorrowedMap| {
                a.residualized()
                    .as_slice()
                    .iter()
                    .zip(b.residualized().as_slice())
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f64, f64::max)
            };
            assert!(distance(&eight_workers, &four_workers) < 2e-9);
            eprintln!("counted Schur default tolerance: four_workers/MAP={}, eight_workers/MAP={}, eight_workers/four_workers={}",
        distance(&four_workers, &expected), distance(&eight_workers, &expected), distance(&eight_workers, &four_workers));
            // The absolute oracle bound here is tighter than the default norm-scaled
            // stopping rule on the deliberately large raw FE contribution.
            for threads in [4, 8] {
                let continued = solve(threads, 1e-14);
                assert!(continued.report().certified());
                assert!(distance(&continued, &expected) < 2e-9);
            }
            let mut previous = None;
            for threads in [8, 16] {
                let actual = solve(threads, 1e-12);
                assert!(actual.report().certified());
                for (&actual, &expected) in actual
                    .residualized()
                    .as_slice()
                    .iter()
                    .zip(expected.residualized().as_slice())
                {
                    assert!((actual - expected).abs() < 2e-9, "{actual} != {expected}");
                }
                if let Some(previous) = previous {
                    assert_eq!(actual.residualized(), &previous);
                }
                previous = Some(actual.residualized().clone());
            }
        }
    }
}

#[test]
fn large_two_fe_schur_operator_is_bit_stable_across_threads() {
    let rows = 16_384;
    let first_levels = 1_024;
    let second_levels = 997;
    let first_codes = (0..rows)
        .map(|row| (row % first_levels) as u32)
        .collect::<Vec<_>>();
    let second_codes = (0..rows)
        .map(|row| ((7 * row + row / first_levels) % second_levels) as u32)
        .collect::<Vec<_>>();
    let first_index = LevelIndex::from_codes(&first_codes, first_levels).expect("first index");
    let second_index = LevelIndex::from_codes(&second_codes, second_levels).expect("second index");
    let first = ValidatedBorrowedInterceptSource::new(&first_codes, first_levels, &first_index)
        .expect("first source");
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, second_levels, &second_index)
        .expect("second source");
    let design = BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
        .expect("design");
    let raw_weights = (0..rows)
        .map(|row| 1.0 + (row % 17) as f64 / 11.0)
        .collect::<Vec<_>>();
    let weights =
        validate_weights(BorrowedMapWeights::Provided(&raw_weights), rows).expect("weights");
    let build = |threads, rhs, stride| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
            .install(|| {
                let mut reservation = SystemWorkspaceReservation;
                let mut graph =
                    TwoFePcgWorkspace::new(&design, stride, &mut reservation).expect("graph");
                let left_codes = design.effects()[graph.left_effect].codes();
                let right_codes = design.effects()[graph.right_effect].codes();
                for row in 0..rows {
                    let weight = weights.normalized(row);
                    graph.left_weights[left_codes[row] as usize] += weight;
                    graph.right_weights[right_codes[row] as usize] += weight;
                }
                for (index, direction) in graph.direction.iter_mut().enumerate() {
                    *direction = ((17 * index % 101) as f64 - 50.0) / 37.0;
                }
                graph
                    .apply_schur(&design, weights, rhs, stride)
                    .expect("Schur operator");
                graph
            })
    };
    for (rhs, stride) in [
        (1, 1),
        (7, 8),
        (8, 8),
        (9, 12),
        (17, 20),
        (52, 52),
        (51, 64),
        (65, 68),
        (129, 132),
    ] {
        let serial = build(1, rhs, stride);
        for threads in [2, 4, 8, 16] {
            let parallel = build(threads, rhs, stride);
            for (expected, actual) in [
                (&serial.left_work, &parallel.left_work),
                (&serial.operator, &parallel.operator),
            ] {
                assert_eq!(
                    expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                    actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                );
            }
        }
    }
}

#[test]
fn schur_rhs_tiles_match_exact_weighted_bipartite_operator() {
    for repetitions in [1, 4096] {
        let rows = 4 * repetitions;
        let left_codes: Vec<_> = (0..rows).map(|r| ((r % 4) / 2) as u32).collect();
        let right_codes: Vec<_> = (0..rows).map(|r| (r % 2) as u32).collect();
        let left_index = LevelIndex::from_codes(&left_codes, 2).unwrap();
        let right_index = LevelIndex::from_codes(&right_codes, 2).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, 2, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, 2, &right_index).unwrap();
        let design =
            BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        let raw_weights: Vec<_> = (0..rows).map(|r| [1.0, 2.0, 2.0, 4.0][r % 4]).collect();
        for weighted in [false, true] {
            let weights = validate_weights(
                if weighted {
                    BorrowedMapWeights::Provided(&raw_weights)
                } else {
                    BorrowedMapWeights::Unit
                },
                rows,
            )
            .unwrap();
            for threads in [1, 4, 16] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        for width in [1, 3, 4, 7, 8, 9, 17, 51, 64, 65, 129] {
                            let stride = width + 3;
                            let mut graph = TwoFePcgWorkspace::new(
                                &design,
                                stride,
                                &mut SystemWorkspaceReservation,
                            )
                            .unwrap();
                            let mass = repetitions as f64;
                            let group_weights = if weighted {
                                [0.75 * mass, 1.5 * mass]
                            } else {
                                [2.0 * mass; 2]
                            };
                            graph.left_weights.copy_from_slice(&group_weights);
                            graph.right_weights.copy_from_slice(&group_weights);
                            graph.direction.fill(0.0);
                            graph.left_work.fill(99.0);
                            graph.operator.fill(99.0);
                            for rhs in 0..width {
                                let k = ((rhs % 7) as f64 - 3.0) / 8.0;
                                graph.direction[rhs] = if weighted { 9.0 * k } else { 6.0 * k };
                            }
                            graph.apply_schur(&design, weights, width, stride).unwrap();
                            // Complete 2x2 incidence has a rank-one Schur block.
                            // The chosen dyadic directions give left means 3*k
                            // and exact S*d = [a*k, -a*k], independently of code.
                            let amplitude = if weighted { 4.5 * mass } else { 6.0 * mass };
                            for level in 0..2 {
                                for rhs in 0..stride {
                                    let k = ((rhs % 7) as f64 - 3.0) / 8.0;
                                    let mean = if rhs < width { 3.0 * k } else { 0.0 };
                                    let product = if rhs >= width || k == 0.0 {
                                        0.0
                                    } else if level == 0 {
                                        amplitude * k
                                    } else {
                                        -amplitude * k
                                    };
                                    assert_eq!(
                                        graph.left_work[level * stride + rhs].to_bits(),
                                        mean.to_bits()
                                    );
                                    assert_eq!(
                                        graph.operator[level * stride + rhs].to_bits(),
                                        product.to_bits()
                                    );
                                }
                            }
                        }
                    });
            }
        }
    }
}

#[test]
fn large_two_fe_pcg_recurrence_is_bit_stable_across_threads() {
    let rows = 16_384;
    let first_levels = 1_024;
    let second_levels = 997;
    let first_codes = (0..rows)
        .map(|row| (row % first_levels) as u32)
        .collect::<Vec<_>>();
    let second_codes = (0..rows)
        .map(|row| ((7 * row + row / first_levels) % second_levels) as u32)
        .collect::<Vec<_>>();
    let first_index = LevelIndex::from_codes(&first_codes, first_levels).expect("first index");
    let second_index = LevelIndex::from_codes(&second_codes, second_levels).expect("second index");
    let first = ValidatedBorrowedInterceptSource::new(&first_codes, first_levels, &first_index)
        .expect("first source");
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, second_levels, &second_index)
        .expect("second source");
    let columns = (0..3)
        .map(|column| {
            (0..rows)
                .map(|row| {
                    ((row * (column + 3) % 103) as f64 - 51.0)
                        .mul_add(column as f64 + 0.5, (row % 29) as f64 / 31.0)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let weights = (0..rows)
        .map(|row| 1.0 + (row % 17) as f64 / 11.0)
        .collect::<Vec<_>>();
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
            .install(|| {
                let design =
                    BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
                        .expect("design");
                let mut reusable =
                    ReusableCategoricalMapAbsorber::new(design, columns.len(), threads)
                        .expect("reusable map");
                assert!(reusable.two_fe_pcg.is_some());
                reusable
                    .absorb_ppml(
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(&columns).expect("matrix"),
                        config(),
                    )
                    .expect("certified PCG")
            })
    };
    let serial = run(1);
    assert!(serial.report().iterations > 0);
    for threads in [7, 14] {
        let parallel = run(threads);
        assert_eq!(serial.residualized(), parallel.residualized());
        assert_eq!(serial.report().iterations, parallel.report().iterations);
        assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
        assert_eq!(serial.report().certificate, parallel.report().certificate);
    }
}

#[test]
fn active_two_fe_pcg_recurrence_is_bit_stable_across_budget_boundaries() {
    let rows = 65_536;
    let first_levels = 8_192;
    let second_levels = 8_191;
    let first_codes = (0..rows)
        .map(|row| (row % first_levels) as u32)
        .collect::<Vec<_>>();
    let second_codes = (0..rows)
        .map(|row| ((7 * row + row / first_levels) % second_levels) as u32)
        .collect::<Vec<_>>();
    let first_index = LevelIndex::from_codes(&first_codes, first_levels).expect("first index");
    let second_index = LevelIndex::from_codes(&second_codes, second_levels).expect("second index");
    let first = ValidatedBorrowedInterceptSource::new(&first_codes, first_levels, &first_index)
        .expect("first source");
    let second = ValidatedBorrowedInterceptSource::new(&second_codes, second_levels, &second_index)
        .expect("second source");
    let columns = (0..3)
        .map(|column| {
            (0..rows)
                .map(|row| {
                    ((row * (column + 3) % 103) as f64 - 51.0)
                        .mul_add(column as f64 + 0.5, (row % 29) as f64 / 31.0)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let weights = (0..rows)
        .map(|row| 1.0 + (row % 17) as f64 / 11.0)
        .collect::<Vec<_>>();
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
            .install(|| {
                let design =
                    BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
                        .expect("design");
                let mut reusable =
                    ReusableCategoricalMapAbsorber::new(design, columns.len(), threads)
                        .expect("reusable map");
                assert!(reusable.two_fe_pcg.is_some());
                reusable
                    .absorb_ppml(
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(&columns).expect("matrix"),
                        config(),
                    )
                    .expect("certified PCG")
            })
    };
    let serial = run(1);
    assert!(serial.report().iterations > 0);
    assert!(crate::parallel::StagePlan::new(second_levels as u128 * 3, second_levels, 7).lanes > 1);
    for threads in [7, 9, 15] {
        let parallel = run(threads);
        assert_eq!(serial.residualized(), parallel.residualized());
        assert_eq!(serial.report().iterations, parallel.report().iterations);
        assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
        assert_eq!(serial.report().certificate, parallel.report().certificate);
    }
}

#[test]
fn map_workspace_report_counts_reserved_capacity_before_norm_handoff() {
    struct RecordingReservation {
        bytes: u64,
    }
    impl WorkspaceReservation for RecordingReservation {
        fn reserve_f64(
            &mut self,
            values: &mut Vec<f64>,
            elements: usize,
            label: &'static str,
        ) -> Result<()> {
            SystemWorkspaceReservation.reserve_f64(values, elements + 7, label)?;
            self.bytes += (values.capacity() * std::mem::size_of::<f64>()) as u64;
            Ok(())
        }
    }
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for rows in [32, 16_384] {
                    let codes = (0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>();
                    let index = LevelIndex::from_codes(&codes, 4).unwrap();
                    let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
                    for effects in [vec![], vec![source.effect()]] {
                        let design = BorrowedInterceptFeDesign::new(rows, effects).unwrap();
                        let input = (0..rows)
                            .map(|row| {
                                2.0 * (row % 4) as f64 + if (row / 4) % 2 == 0 { -1.0 } else { 1.0 }
                            })
                            .collect::<Vec<_>>();
                        let mut reservation = RecordingReservation { bytes: 0 };
                        let result = absorb_with_reservation_mode(
                            &design,
                            BorrowedMapWeights::Unit,
                            DenseMatrix::from_columns(&[input]).unwrap(),
                            config(),
                            false,
                            false,
                            &mut reservation,
                        )
                        .unwrap();
                        assert_eq!(
                            result.report().solver_workspace_peak.bytes(),
                            Some(reservation.bytes)
                        );
                        assert!(reservation.bytes > 7 * 8);
                        assert!(result.report().certified());
                        if !design.effects().is_empty() {
                            for (row, residual) in
                                result.residualized().as_slice().iter().enumerate()
                            {
                                assert_eq!(*residual, if (row / 4) % 2 == 0 { -1.0 } else { 1.0 });
                            }
                        }
                    }
                }
            });
    }
}

#[test]
fn cg_workspace_report_counts_capacity_early_exit_and_retained_coefficients() {
    struct RecordingReservation {
        bytes: u64,
    }
    impl WorkspaceReservation for RecordingReservation {
        fn reserve_f64(
            &mut self,
            values: &mut Vec<f64>,
            elements: usize,
            label: &'static str,
        ) -> Result<()> {
            SystemWorkspaceReservation.reserve_f64(values, elements + 7, label)?;
            self.bytes += (values.capacity() * 8) as u64;
            Ok(())
        }
    }
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let rows = 1024;
                let codes = (0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>();
                let index = LevelIndex::from_codes(&codes, 4).unwrap();
                let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
                let design = BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
                for already_within in [false, true] {
                    for retain_coefficients in [false, true] {
                        let columns = (0..2)
                            .map(|rhs| {
                                (0..rows)
                                    .map(|row| {
                                        let noise = if (row / 4) % 2 == 0 { -1.0 } else { 1.0 };
                                        (rhs + 1) as f64
                                            * (noise
                                                + if already_within {
                                                    0.0
                                                } else {
                                                    2.0 * codes[row] as f64
                                                })
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>();
                        let mut reservation = RecordingReservation { bytes: 0 };
                        let result = absorb_cg_with_reservation(
                            &design,
                            BorrowedMapWeights::Unit,
                            DenseMatrix::from_columns(&columns).unwrap(),
                            config(),
                            false,
                            CgContinuation {
                                retain_coefficients,
                                ..CgContinuation::ORDINARY
                            },
                            &mut reservation,
                        )
                        .unwrap();
                        assert!(result.report().certified());
                        assert_eq!(result.report().iterations == 0, already_within);
                        let matrices = if already_within {
                            0
                        } else {
                            3 * rows as u64 * 2 * 8
                        };
                        // Four levels times two RHSs, plus channel/count/offset metadata.
                        let one_coefficient_bank =
                            4 * 2 * 8 + (1 + 1 + 2) * std::mem::size_of::<usize>() as u64;
                        let coefficients = if !retain_coefficients {
                            0
                        } else if already_within {
                            one_coefficient_bank
                        } else {
                            4 * one_coefficient_bank
                        };
                        assert_eq!(
                            result.report().solver_workspace_peak.bytes(),
                            Some(reservation.bytes + matrices + coefficients)
                        );
                        for row in 0..rows {
                            let noise = if (row / 4) % 2 == 0 { -1.0 } else { 1.0 };
                            for rhs in 0..2 {
                                assert!(
                                    (result.residualized()[(row, rhs)] - (rhs + 1) as f64 * noise)
                                        .abs()
                                        < 1e-12
                                );
                            }
                        }
                    }
                }
            });
    }
}

#[test]
fn every_workspace_reservation_and_capacity_guard_fails_closed() {
    let codes = [0_u32, 0, 1, 1];
    let index = LevelIndex::from_codes(&codes, 2).expect("index");
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
    let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
    let columns = [vec![1.0, 2.0, 4.0, 8.0], vec![2.0, 3.0, 1.0, -1.0]];

    let mut successful = InjectedReservation::default();
    absorb_with_reservation_mode(
        &design,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&columns).expect("matrix"),
        config(),
        false,
        false,
        &mut successful,
    )
    .expect("successful run");
    assert_eq!(successful.calls, 7);

    for fail_at in 0..successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        let error = absorb_with_reservation_mode(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
            false,
            false,
            &mut injected,
        )
        .expect_err("injected allocation must fail");
        assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
    }

    for skip_at in 0..successful.calls {
        let mut injected = InjectedReservation {
            skip_at: Some(skip_at),
            ..InjectedReservation::default()
        };
        let error = absorb_with_reservation_mode(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
            false,
            false,
            &mut injected,
        )
        .expect_err("undersized allocation must fail");
        assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
    }
}

#[test]
fn lane_initialized_banks_retain_reserved_storage_and_failure_contract() {
    #[derive(Default)]
    struct PointerReservation {
        address: usize,
        capacity: usize,
    }
    impl WorkspaceReservation for PointerReservation {
        fn reserve_f64(
            &mut self,
            target: &mut Vec<f64>,
            additional: usize,
            _: &'static str,
        ) -> Result<()> {
            target.try_reserve_exact(additional).unwrap();
            self.address = target.as_ptr() as usize;
            self.capacity = target.capacity();
            Ok(())
        }
    }
    for workers in [1, 4, 16] {
        let team =
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap());
        team.install(|| {
            for elements in [0, 1, 16_385, 262_147] {
                let mut reservation = PointerReservation::default();
                let values = reserved_zeros(elements, &mut reservation, "test bank").unwrap();
                assert_eq!(values.as_ptr() as usize, reservation.address);
                assert_eq!(values.capacity(), reservation.capacity);
                assert_eq!(values.len(), elements);
                assert!(values.iter().all(|value| value.to_bits() == 0));
            }
            for skip in [false, true] {
                let mut reservation = InjectedReservation {
                    fail_at: (!skip).then_some(0),
                    skip_at: skip.then_some(0),
                    ..InjectedReservation::default()
                };
                let error = reserved_zeros(262_147, &mut reservation, "test bank").unwrap_err();
                assert_eq!(error.code, ErrorCode::Allocation);
                assert_eq!(reservation.records, [(262_147, "test bank")]);
            }
        })
        .unwrap();
    }
}

#[test]
fn structural_constant_empty_design_allocations_are_fallible_and_only_rhs_scale() {
    let observations = 4096;
    let rhs_count = 3;
    let design = BorrowedInterceptFeDesign::new(observations, Vec::new()).expect("empty design");
    let columns = (0..rhs_count)
        .map(|column| {
            (0..observations)
                .map(|row| (row as f64).sin() + column as f64)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let labels = [
        "unable to reserve borrowed MAP reference norms",
        "unable to reserve borrowed MAP residual norms",
        "unable to reserve borrowed MAP RHS scales",
        "unable to reserve borrowed MAP per-RHS certificate norms",
        "unable to reserve borrowed MAP value scales",
        "unable to reserve borrowed MAP group sums",
        "unable to reserve borrowed MAP group corrections",
    ];

    let mut successful = InjectedReservation::default();
    let result = absorb_with_reservation_mode(
        &design,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&columns).expect("structural RHS"),
        config(),
        true,
        false,
        &mut successful,
    )
    .expect("structural constant projection");
    assert!(result.report().certified());
    assert_eq!(result.report().iterations, 1);
    assert_eq!(successful.calls, labels.len());
    assert_eq!(
        successful.records,
        labels
            .into_iter()
            .map(|label| (rhs_count, label))
            .collect::<Vec<_>>()
    );
    assert!(successful
        .records
        .iter()
        .all(|(additional, _)| *additional < observations));

    for fail_at in 0..successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(fail_at),
            ..InjectedReservation::default()
        };
        let error = absorb_with_reservation_mode(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("structural RHS"),
            config(),
            true,
            false,
            &mut injected,
        )
        .expect_err("structural constant allocation must fail closed");
        assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
    }
    for skip_at in 0..successful.calls {
        let mut injected = InjectedReservation {
            skip_at: Some(skip_at),
            ..InjectedReservation::default()
        };
        let error = absorb_with_reservation_mode(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("structural RHS"),
            config(),
            true,
            false,
            &mut injected,
        )
        .expect_err("undersized structural workspace must fail closed");
        assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
    }
}

#[test]
fn large_parallel_projection_is_bit_stable_and_its_workspace_fails_closed() {
    let observations = PARALLEL_PROJECTION_MIN_ROWS;
    let levels = 4_096;
    let codes = (0..observations)
        .map(|row| (row % levels) as u32)
        .collect::<Vec<_>>();
    let index = LevelIndex::from_codes(&codes, levels).expect("index");
    let source = ValidatedBorrowedInterceptSource::new(&codes, levels, &index).expect("source");
    let design =
        BorrowedInterceptFeDesign::new(observations, vec![source.effect()]).expect("design");
    let columns = (0..3)
        .map(|column| {
            (0..observations)
                .map(|row| {
                    let centered = (row % 97) as f64 - 48.0;
                    centered.mul_add(column as f64 + 0.5, (row % levels) as f64 / 17.0)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let weights = (0..observations)
        .map(|row| 1.0 + (row % 11) as f64 / 13.0)
        .collect::<Vec<_>>();

    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
            .install(|| {
                absorb_borrowed_intercept_map_certified(
                    &design,
                    BorrowedMapWeights::Provided(&weights),
                    DenseMatrix::from_columns(&columns).expect("matrix"),
                    config(),
                )
                .expect("certified projection")
            })
    };
    let serial = run(1);
    let parallel = run(4);
    assert_eq!(serial.residualized(), parallel.residualized());
    assert_eq!(serial.report().iterations, parallel.report().iterations);
    assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
    assert_eq!(serial.report().certificate, parallel.report().certificate);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("pool");
    let mut successful = InjectedReservation::default();
    pool.install(|| {
        absorb_with_reservation_mode(
            &design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
            false,
            false,
            &mut successful,
        )
        .expect("parallel reservation succeeds");
    });
    assert_eq!(successful.calls, 10);
    assert_eq!(
        successful.records.last(),
        Some(&(
            observations.div_ceil(PARALLEL_DOT_CHUNK_ROWS) * columns.len(),
            "unable to reserve borrowed CG parallel dot sums",
        )),
    );

    for reservation in 7..successful.calls {
        for skip in [false, true] {
            let mut injected = InjectedReservation {
                fail_at: (!skip).then_some(reservation),
                skip_at: skip.then_some(reservation),
                ..InjectedReservation::default()
            };
            let error = pool
                .install(|| {
                    absorb_with_reservation_mode(
                        &design,
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(&columns).expect("matrix"),
                        config(),
                        false,
                        false,
                        &mut injected,
                    )
                })
                .expect_err("parallel workspace allocation must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation);
        }
    }
}

#[test]
fn large_one_rhs_one_fe_projection_is_bit_stable_through_sixteen_threads() {
    let observations = PARALLEL_PROJECTION_MIN_ROWS;
    let levels = 4_096;
    let codes = (0..observations)
        .map(|row| (row % levels) as u32)
        .collect::<Vec<_>>();
    let index = LevelIndex::from_codes(&codes, levels).expect("index");
    let source = ValidatedBorrowedInterceptSource::new(&codes, levels, &index).expect("source");
    let design =
        BorrowedInterceptFeDesign::new(observations, vec![source.effect()]).expect("design");
    let column = (0..observations)
        .map(|row| {
            let centered = (row % 97) as f64 - 48.0;
            centered.mul_add(0.5, (row % levels) as f64 / 17.0)
        })
        .collect::<Vec<_>>();
    let weights = (0..observations)
        .map(|row| 1.0 + (row % 11) as f64 / 13.0)
        .collect::<Vec<_>>();

    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
            .install(|| {
                absorb_borrowed_intercept_map_certified(
                    &design,
                    BorrowedMapWeights::Provided(&weights),
                    DenseMatrix::from_columns(std::slice::from_ref(&column)).expect("matrix"),
                    config(),
                )
                .expect("certified one-RHS projection")
            })
    };
    let serial = run(1);
    for threads in [2, 4, 8, 16] {
        let parallel = run(threads);
        assert_eq!(serial.residualized(), parallel.residualized());
        assert_eq!(
            serial.report().requested_solver,
            parallel.report().requested_solver
        );
        assert_eq!(serial.report().used_solver, parallel.report().used_solver);
        assert_eq!(serial.report().iterations, parallel.report().iterations);
        assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
        assert_eq!(serial.report().certificate, parallel.report().certificate);
        assert_eq!(
            serial.report().normal_equation_residual,
            parallel.report().normal_equation_residual
        );
        assert_eq!(
            serial.report().fallback_used,
            parallel.report().fallback_used
        );
    }
}

#[test]
fn ppml_cg_action_preserves_fe_space_under_cancellation() {
    let observations = 16;
    let codes = (0..3)
        .map(|axis| {
            (0..observations)
                .map(|row| (((row / 2) >> axis) & 1) as u32)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let indices = codes
        .iter()
        .map(|c| LevelIndex::from_codes(c, 2).unwrap())
        .collect::<Vec<_>>();
    let sources = codes
        .iter()
        .zip(&indices)
        .map(|(c, i)| ValidatedBorrowedInterceptSource::new(c, 2, i).unwrap())
        .collect::<Vec<_>>();
    let design = BorrowedInterceptFeDesign::new(
        observations,
        sources
            .iter()
            .map(ValidatedBorrowedInterceptSource::effect)
            .collect(),
    )
    .unwrap();
    let input = DenseMatrix::from_columns(&[
        (0..observations)
            .map(|row| {
                if row % 2 == 0 {
                    1e10 * (row + 1) as f64
                } else {
                    0.001 * row as f64
                }
            })
            .collect(),
        (0..observations)
            .map(|row| (row as f64 * 0.71).sin())
            .collect(),
    ])
    .unwrap();
    let weights = (0..observations)
        .map(|row| 1.0 + (row % 5) as f64)
        .collect::<Vec<_>>();
    for raw_weights in [
        BorrowedMapWeights::Unit,
        BorrowedMapWeights::Provided(&weights),
    ] {
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let mut workspace = BorrowedMapWorkspace::new_with_parallelism(
                        &design,
                        observations,
                        2,
                        threads,
                        &mut SystemWorkspaceReservation,
                    )
                    .unwrap();
                    let mut action = DenseMatrix::zeros(observations, 2).unwrap();
                    borrowed_symmetric_projection_from_group_means_recording(
                        &design,
                        validate_weights(raw_weights, observations).unwrap(),
                        &input,
                        &mut action,
                        &mut workspace,
                        None,
                        false,
                    )
                    .unwrap();
                    // Identical FE tuples must receive identical contributions,
                    // independently of the magnitude of the input's off-space part.
                    for row in (0..observations).step_by(2) {
                        for rhs in 0..2 {
                            assert_eq!(action[(row, rhs)], action[(row + 1, rhs)]);
                        }
                    }
                    for rhs in 0..2 {
                        let scale = (0..observations)
                            .map(|row| action[(row, rhs)].abs())
                            .fold(1.0_f64, f64::max);
                        for tuple in 0..8 {
                            let additive = action[(0, rhs)]
                                + (0..3)
                                    .filter(|axis| tuple & (1 << axis) != 0)
                                    .map(|axis| action[((1 << axis) * 2, rhs)] - action[(0, rhs)])
                                    .sum::<f64>();
                            assert!((action[(tuple * 2, rhs)] - additive).abs() < 1e-14 * scale);
                        }
                    }
                    let mut subtractive = DenseMatrix::zeros(observations, 2).unwrap();
                    borrowed_symmetric_projection_into_recording(
                        &design,
                        validate_weights(raw_weights, observations).unwrap(),
                        &input,
                        &mut subtractive,
                        &mut workspace,
                        None,
                    )
                    .unwrap();
                    for rhs in 0..2 {
                        let scale = (0..observations)
                            .map(|row| input[(row, rhs)].abs())
                            .fold(1.0_f64, f64::max);
                        for row in 0..observations {
                            assert!(
                                (action[(row, rhs)] - subtractive[(row, rhs)]).abs()
                                    < 1e-14 * scale
                            );
                        }
                    }
                    action
                })
        };
        assert_eq!(run(1), run(3));
    }
}

#[test]
fn fused_symmetric_complement_and_cg_update_match_separate_passes() {
    let observations = 20_000;
    let level_counts = [257, 263, 269];
    let code_sets = level_counts
        .iter()
        .enumerate()
        .map(|(effect, &levels)| {
            (0..observations)
                .map(|row| ((row * (2 * effect + 1) + row / 17) % levels) as u32)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let indexes = code_sets
        .iter()
        .zip(level_counts)
        .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
        .collect::<Vec<_>>();
    let sources = code_sets
        .iter()
        .zip(indexes.iter())
        .zip(level_counts)
        .map(|((codes, index), levels)| {
            ValidatedBorrowedInterceptSource::new(codes, levels, index).expect("source")
        })
        .collect::<Vec<_>>();
    let design = BorrowedInterceptFeDesign::new(
        observations,
        sources.iter().map(|source| source.effect()).collect(),
    )
    .expect("design");
    let columns = (0..11)
        .map(|column| {
            (0..observations)
                .map(|row| {
                    ((row % 101) as f64 - 50.0).mul_add(
                        column as f64 + 0.25,
                        ((row * (column + 5)) % 4_103) as f64 / 19.0,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let input = DenseMatrix::from_columns(&columns).expect("matrix");
    let weight_values = (0..observations)
        .map(|row| 1.0 + (row % 13) as f64 / 17.0)
        .collect::<Vec<_>>();
    let weights = validate_weights(BorrowedMapWeights::Provided(&weight_values), observations)
        .expect("weights");

    for threads in [1, 7] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
            .install(|| {
                let mut reservation = SystemWorkspaceReservation;
                let mut workspace = BorrowedMapWorkspace::new_with_parallelism(
                    &design,
                    observations,
                    columns.len(),
                    threads,
                    &mut reservation,
                )
                .expect("workspace");
                let mut expected_initial = input.clone();
                let mut expected_initial_coefficients =
                    FeCoefficientBatch::intercept_zeros(&design, columns.len())
                        .expect("coefficients");
                project_effect_recording(
                    &design.effects()[0],
                    weights,
                    &mut expected_initial,
                    &mut workspace,
                    Some((&mut expected_initial_coefficients, 0)),
                )
                .expect("separate initial effect projection");
                let mut actual_initial =
                    DenseMatrix::zeros(observations, columns.len()).expect("initial matrix");
                let mut actual_initial_coefficients =
                    FeCoefficientBatch::intercept_zeros(&design, columns.len())
                        .expect("coefficients");
                project_effect_recording_from_input(
                    &design.effects()[0],
                    weights,
                    &input,
                    &mut actual_initial,
                    &mut workspace,
                    Some((&mut actual_initial_coefficients, 0)),
                )
                .expect("fused initial effect projection");
                assert_eq!(expected_initial, actual_initial);
                assert_eq!(expected_initial_coefficients, actual_initial_coefficients);

                let mut expected = input.clone();
                let mut expected_coefficients =
                    FeCoefficientBatch::intercept_zeros(&design, columns.len())
                        .expect("coefficients");
                borrowed_residual_sweep_recording(
                    &design,
                    weights,
                    &mut expected,
                    &mut workspace,
                    Some(&mut expected_coefficients),
                    true,
                )
                .expect("separate symmetric projection");
                for (projected, original) in
                    expected.as_mut_slice().iter_mut().zip(input.as_slice())
                {
                    *projected = *original - *projected;
                }

                let mut actual =
                    DenseMatrix::zeros(observations, columns.len()).expect("actual matrix");
                let mut actual_coefficients =
                    FeCoefficientBatch::intercept_zeros(&design, columns.len())
                        .expect("coefficients");
                borrowed_symmetric_projection_into_recording(
                    &design,
                    weights,
                    &input,
                    &mut actual,
                    &mut workspace,
                    Some(&mut actual_coefficients),
                )
                .expect("fused symmetric projection");

                assert_eq!(expected, actual);
                assert_eq!(expected_coefficients, actual_coefficients);
            });
    }
}

#[test]
fn large_wide_three_fe_projection_and_certificate_are_bit_stable() {
    let observations = 50_000;
    assert!(observations >= PARALLEL_PROJECTION_MIN_ROWS);
    let level_counts = [4_096, 2_048, 1_024];
    let code_sets = level_counts
        .iter()
        .map(|&levels| {
            (0..observations)
                .map(|row| (row % levels) as u32)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let indexes = code_sets
        .iter()
        .zip(level_counts)
        .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
        .collect::<Vec<_>>();
    let sources = code_sets
        .iter()
        .zip(indexes.iter())
        .zip(level_counts)
        .map(|((codes, index), levels)| {
            ValidatedBorrowedInterceptSource::new(codes, levels, index).expect("source")
        })
        .collect::<Vec<_>>();
    let design = BorrowedInterceptFeDesign::new(
        observations,
        sources.iter().map(|source| source.effect()).collect(),
    )
    .expect("design");
    let columns = (0..11)
        .map(|column| {
            (0..observations)
                .map(|row| {
                    let centered = (row % 97) as f64 - 48.0;
                    centered.mul_add(
                        column as f64 + 0.5,
                        ((row * (column + 3)) % 4_099) as f64 / 17.0,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let weights = (0..observations)
        .map(|row| 1.0 + (row % 11) as f64 / 13.0)
        .collect::<Vec<_>>();

    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
            .install(|| {
                absorb_borrowed_intercept_map_certified(
                    &design,
                    BorrowedMapWeights::Provided(&weights),
                    DenseMatrix::from_columns(&columns).expect("matrix"),
                    config(),
                )
                .expect("certified wide projection")
            })
    };
    let serial = run(1);
    for threads in [2, 4, 8, 16] {
        let parallel = run(threads);
        assert_eq!(serial.residualized(), parallel.residualized());
        assert_eq!(serial.report().iterations, parallel.report().iterations);
        assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
        assert_eq!(serial.report().certificate, parallel.report().certificate);
    }
}

#[test]
fn success_is_always_certified_and_nonconvergence_is_fatal() {
    let first_codes = [0_u32, 0, 0, 1, 1, 1];
    let second_codes = [0_u32, 0, 1, 0, 1, 1];
    let first_index = LevelIndex::from_codes(&first_codes, 2).expect("first index");
    let second_index = LevelIndex::from_codes(&second_codes, 2).expect("second index");
    let first =
        ValidatedBorrowedInterceptSource::new(&first_codes, 2, &first_index).expect("first");
    let second =
        ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).expect("second");
    let design =
        BorrowedInterceptFeDesign::new(6, vec![first.effect(), second.effect()]).expect("design");
    let columns = [vec![1.0, -3.0, 2.0, 5.0, -4.0, 0.5]];

    let success = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
        DenseMatrix::from_columns(&columns).expect("matrix"),
        config(),
    )
    .expect("certified");
    assert!(success.report().certified());

    let error = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
        DenseMatrix::from_columns(&columns).expect("matrix"),
        MapConfig {
            tolerance: 1e-16,
            max_iterations: 1,
            check_every: 1,
            symmetric: false,
        },
    )
    .expect_err("uncertified solve must be fatal");
    assert_eq!(error.code, ErrorCode::InvalidData);
}

#[test]
fn empty_nonfinite_and_invalid_weight_inputs_fail_before_workspace_allocation() {
    let design = BorrowedInterceptFeDesign::new(0, Vec::new()).expect("empty design");
    let mut reservation = InjectedReservation::default();
    let error = absorb_with_reservation_mode(
        &design,
        BorrowedMapWeights::Unit,
        DenseMatrix::zeros(0, 1).expect("matrix"),
        config(),
        false,
        false,
        &mut reservation,
    )
    .expect_err("empty rows");
    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(reservation.calls, 0);

    let codes = [0_u32, 0];
    let index = LevelIndex::from_codes(&codes, 1).expect("index");
    let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
    let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, bad]),
            DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
            config(),
        )
        .expect_err("invalid weight");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }
    let error = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Provided(&[f64::MAX, f64::MIN_POSITIVE]),
        DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
        config(),
    )
    .expect_err("normalized underflow");
    assert_eq!(error.code, ErrorCode::InvalidData);

    for bad in [f64::NAN, f64::INFINITY] {
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, bad]]).expect("matrix"),
            config(),
        )
        .expect_err("invalid value");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }
}

#[test]
fn no_effects_and_already_certified_inputs_return_without_iteration() {
    let design = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("design");
    let result = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0]]).expect("matrix"),
        config(),
    )
    .expect("no effects");
    assert_eq!(result.report().stop_reason, SolveStopReason::NoFixedEffects);
    assert_eq!(result.report().iterations, 0);

    let codes = [0_u32, 0, 1, 1];
    let index = LevelIndex::from_codes(&codes, 2).expect("index");
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
    let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
    let result = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&[vec![-1.0, 1.0, -2.0, 2.0]]).expect("matrix"),
        config(),
    )
    .expect("already certified");
    assert_eq!(result.report().iterations, 0);
    assert!(result.report().certified());

    let zero = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&[vec![0.0; 4]]).expect("zero matrix"),
        config(),
    )
    .expect("zero RHS");
    assert_eq!(zero.report().iterations, 0);
    assert_eq!(zero.report().certificate.relative_infinity_norm, 0.0);
}

#[test]
fn fresh_one_effect_nonfinite_validation_precedes_weights_and_allocation() {
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            for rows in [16_383, 16_384] {
                let codes = vec![0_u32; rows];
                let index = LevelIndex::from_codes(&codes, 1).unwrap();
                let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
                let design = BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
                for columns in [7, 8] {
                    for position in [0, rows * columns / 2, rows * columns - 1] {
                        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                            let mut data = vec![1.0; rows * columns];
                            data[position] = bad;
                            let values = DenseMatrix::from_row_major(rows, columns, data).unwrap();
                            let mut reservation = InjectedReservation {
                                fail_at: Some(0),
                                ..InjectedReservation::default()
                            };
                            // Both a bad weight shape and an allocation failure
                            // would mask this diagnostic if validation moved later.
                            let error = absorb_with_reservation_mode(
                                &design,
                                BorrowedMapWeights::Provided(&[]),
                                values,
                                config(),
                                true,
                                false,
                                &mut reservation,
                            )
                            .unwrap_err();
                            assert_eq!(error.code, ErrorCode::InvalidData);
                            assert_eq!(error.message, "borrowed MAP values must be finite");
                            assert_eq!(reservation.calls, 0);
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn unit_single_column_norm_matches_generic_unit_weights_bitwise() {
    for n in [0, 1, 7, 127, 16_383, 16_384, 32_771] {
        for case in 0..7 {
            let mut values: Vec<_> = (0..n).map(|r| (r as f64).sin()).collect();
            if n > 0 {
                match case {
                    1 => values.fill(-0.0),
                    2 => values[n / 2] = f64::NAN,
                    3 => values[n - 1] = f64::INFINITY,
                    4 => {
                        for value in &mut values {
                            *value *= 1e-300;
                        }
                    }
                    5 => {
                        for value in &mut values {
                            *value *= 1e150;
                        }
                    }
                    6 => {
                        values[0] = f64::MAX;
                        values[n / 2] = -f64::MAX;
                    }
                    _ => (),
                }
            }
            let matrix = DenseMatrix::from_row_major(n, 1, values).unwrap();
            let weights = vec![1.0; n];
            let run = |weights| {
                let mut output = [0.0];
                let mut scale = [0.0];
                let mut sum = [0.0];
                weighted_column_norms(&matrix, weights, &mut output, &mut scale, &mut sum, &mut [])
                    .map(|()| (output[0].to_bits(), scale[0].to_bits(), sum[0].to_bits()))
            };
            assert_eq!(
                run(ValidatedWeights {
                    values: None,
                    raw_scale: 1.0
                }),
                run(ValidatedWeights {
                    values: Some(&weights),
                    raw_scale: 1.0
                }),
                "N={n}, case={case}"
            );
        }
    }
}

#[test]
fn one_way_projection_is_exact_and_invalid_shapes_and_configurations_fail() {
    let codes = [0_u32, 0, 1, 1];
    let index = LevelIndex::from_codes(&codes, 2).expect("index");
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
    let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
    let result = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Provided(&[1.0, 3.0, 2.0, 2.0]),
        DenseMatrix::from_columns(&[vec![1.0, 5.0, 4.0, 8.0]]).expect("matrix"),
        config(),
    )
    .expect("one-way projection");
    assert_eq!(result.report().iterations, 1);
    assert_close(
        result.residualized().as_slice(),
        &[-3.0, 1.0, -2.0, 2.0],
        1e-14,
    );

    let short = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("short design");
    assert_eq!(
        absorb_borrowed_intercept_map_certified(
            &short,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
            config(),
        )
        .expect_err("dimension mismatch")
        .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0; 3]),
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
            config(),
        )
        .expect_err("weight mismatch")
        .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::zeros(4, 0).expect("empty RHS matrix"),
            config(),
        )
        .expect_err("empty RHS")
        .code,
        ErrorCode::InvalidData
    );

    for bad in [
        MapConfig {
            tolerance: 0.0,
            ..config()
        },
        MapConfig {
            tolerance: f64::NAN,
            ..config()
        },
        MapConfig {
            max_iterations: 0,
            ..config()
        },
        MapConfig {
            check_every: 0,
            ..config()
        },
    ] {
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                bad,
            )
            .expect_err("invalid config")
            .code,
            ErrorCode::InvalidArgument
        );
    }
}

#[test]
fn nonrepresentable_weighted_norm_fails_closed() {
    let codes = [0_u32, 0];
    let index = LevelIndex::from_codes(&codes, 1).expect("index");
    let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
    let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
    let error = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&[vec![f64::MAX, f64::MAX]]).expect("matrix"),
        config(),
    )
    .expect_err("overflowing weighted norm");
    assert_eq!(error.code, ErrorCode::InvalidData);
}

#[test]
fn cmg_polish_accurate_means_preserve_extremes_and_reject_lost_range() {
    let tiny = f64::from_bits(1);
    for (input, projection, raw, expected) in [
        (
            vec![f64::MAX; 3],
            vec![0.0; 3],
            vec![0.1, 0.2, 0.7],
            f64::MAX,
        ),
        (vec![f64::MAX, -f64::MAX], vec![0.0; 2], vec![1.0; 2], 0.0),
        (
            vec![f64::MAX; 2],
            vec![f64::MAX / 2.0; 2],
            vec![1.0; 2],
            f64::MAX / 2.0,
        ),
        (
            vec![4.0 * tiny, 8.0 * tiny],
            vec![tiny; 2],
            vec![1.0; 2],
            5.0 * tiny,
        ),
        (
            vec![4.0 * tiny, -2.0 * tiny],
            vec![tiny; 2],
            vec![1.0; 2],
            0.0,
        ),
        (vec![1.0, -1.0], vec![-2.0; 2], vec![1.0; 2], 2.0),
    ] {
        let codes = vec![0_u32; input.len()];
        let index = LevelIndex::from_codes(&codes, 1).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
        let mut means = [0.0];
        fused_group_means_by_value(
            &source.effect(),
            0,
            validate_weights(BorrowedMapWeights::Provided(&raw), input.len()).unwrap(),
            input.len(),
            &mut means,
            true,
            |row, _| cg_polish_difference(input[row], projection[row]),
        )
        .unwrap();
        assert_eq!(means[0], expected);
    }
    assert!(cg_polish_difference(f64::MAX, -f64::MAX).is_err());
    let codes = [0_u32, 0];
    let index = LevelIndex::from_codes(&codes, 1).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
    let error = fused_group_means_by_value(
        &source.effect(),
        0,
        validate_weights(BorrowedMapWeights::Unit, 2).unwrap(),
        2,
        &mut [0.0],
        true,
        |_, _| cg_polish_difference(f64::MAX, tiny),
    )
    .unwrap_err();
    assert!(error.message.contains("lost range"));
    // General TwoDiff must also preserve a low part without |a| >= |b|.
    assert_eq!(
        cg_polish_difference(2.0_f64.powi(-54), 1.0).unwrap(),
        (-1.0, 2.0_f64.powi(-54))
    );
}

#[test]
fn cmg_polish_mixed_columns_reject_energy_underflow_without_deactivating_live_rhs() {
    let codes = [0_u32, 0];
    let index = LevelIndex::from_codes(&codes, 1).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).unwrap();
    let input = DenseMatrix::from_columns(&[vec![0.0; 2], vec![1.0, 3.0]]).unwrap();
    let result = absorb_borrowed_intercept_cg(
        &design,
        BorrowedMapWeights::Unit,
        input,
        config(),
        BorrowedCgOptions::polish(&[0.0, 10.0_f64.sqrt()]),
    )
    .unwrap();
    assert_eq!(result.residualized().as_slice(), &[0.0, -1.0, 0.0, 1.0]);
    let input = DenseMatrix::from_columns(&[vec![0.0; 2], vec![1e-170; 2]]).unwrap();
    let error = absorb_borrowed_intercept_cg(
        &design,
        BorrowedMapWeights::Unit,
        input.clone(),
        config(),
        BorrowedCgOptions::polish(&[0.0, 2.0_f64.sqrt() * 1e-170]),
    )
    .unwrap_err();
    assert!(error.message.contains("energy underflowed"));
    // The same guard used after each update checks columns independently.
    check_cg_energy_resolution(&DenseMatrix::zeros(2, 2).unwrap(), &[0.0, 0.0]).unwrap();
    assert!(check_cg_energy_resolution(&input, &[0.0, 0.0]).is_err());
}

#[test]
fn cmg_polish_resolves_small_fe_components_on_weighted_cycles() {
    let failures = weighted_cycle_polish_failures(1e-6);
    assert!(
        failures.is_empty(),
        "analytic weighted-cycle projection failures: {failures:?}"
    );
}

#[test]
fn cmg_polish_initial_certificate_handles_smaller_fe_components() {
    let failures = weighted_cycle_polish_failures(1e-8);
    assert!(
        failures.is_empty(),
        "smaller FE component projection failures: {failures:?}"
    );
}

fn weighted_cycle_polish_failures(amplitude: f64) -> Vec<(usize, f64, f64, usize, f64)> {
    // Every graph edge has two equal-weight observations with opposite
    // contrasts. Their exact projection is therefore +/-1, independently of
    // the graph's spectrum. The added worker/firm potential cancels on heavy
    // edges and leaves a small, slow component on light edges.
    let levels = 32;
    let rows = 4 * levels;
    let worker: Vec<_> = (0..rows).map(|row| (row / 4) as u32).collect();
    let firm: Vec<_> = (0..rows)
        .map(|row| ((row / 4 + (row / 2) % 2) % levels) as u32)
        .collect();
    let worker_index = LevelIndex::from_codes(&worker, levels).unwrap();
    let firm_index = LevelIndex::from_codes(&firm, levels).unwrap();
    let worker_source =
        ValidatedBorrowedInterceptSource::new(&worker, levels, &worker_index).unwrap();
    let firm_source = ValidatedBorrowedInterceptSource::new(&firm, levels, &firm_index).unwrap();
    let design =
        BorrowedInterceptFeDesign::new(rows, vec![worker_source.effect(), firm_source.effect()])
            .unwrap();
    let potential: Vec<_> = (0..levels)
        .map(|level| amplitude * ((0.73 * level as f64).sin() + (0.31 * level as f64).cos()))
        .collect();
    let truth: Vec<_> = (0..rows)
        .map(|row| if row % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let input = DenseMatrix::from_columns(&[truth
        .iter()
        .enumerate()
        .map(|(row, value)| value + potential[firm[row] as usize] - potential[worker[row] as usize])
        .collect()])
    .unwrap();
    let mut failures = Vec::new();
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for weakness in [1e-3, 1e-6] {
                    for scale in [1e-100, 1.0, 1e100] {
                        let raw: Vec<_> = (0..rows)
                            .map(|row| scale * if (row / 2) % 2 == 0 { 1.0 } else { weakness })
                            .collect();
                        let weights = BorrowedMapWeights::Provided(&raw);
                        let mut norms = [0.0];
                        weighted_column_norms(
                            &input,
                            validate_weights(weights, rows).unwrap(),
                            &mut norms,
                            &mut [0.0],
                            &mut [0.0],
                            &mut [],
                        )
                        .unwrap();
                        let result = absorb_borrowed_intercept_cg(
                            &design,
                            weights,
                            input.clone(),
                            MapConfig {
                                tolerance: 1e-14,
                                max_iterations: 256,
                                ..config()
                            },
                            BorrowedCgOptions::polish(&norms),
                        )
                        .unwrap();
                        assert!(result.report().certified());
                        let error = truth
                            .iter()
                            .enumerate()
                            .map(|(row, expected)| {
                                (result.residualized()[(row, 0)] - expected).abs()
                            })
                            .fold(0.0_f64, f64::max);
                        if error > 1e-10 {
                            failures.push((
                                threads,
                                weakness,
                                scale,
                                result.report().iterations,
                                error,
                            ));
                        }
                    }
                }
            });
    }
    failures
}

#[test]
fn stable_cg_polish_preserves_near_within_hadamard_products_at_every_width() {
    let rows = 48;
    let left: Vec<_> = (0..rows).map(|row| (row % 4) as u32).collect();
    let right: Vec<_> = (0..rows).map(|row| ((row / 4) % 3) as u32).collect();
    let li = LevelIndex::from_codes(&left, 4).unwrap();
    let ri = LevelIndex::from_codes(&right, 3).unwrap();
    let ls = ValidatedBorrowedInterceptSource::new(&left, 4, &li).unwrap();
    let rs = ValidatedBorrowedInterceptSource::new(&right, 3, &ri).unwrap();
    let design = BorrowedInterceptFeDesign::new(rows, vec![ls.effect(), rs.effect()]).unwrap();
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
            for width in [4, PARALLEL_GROUP_MAX_RHS+1,65] {
                let truth: Vec<Vec<_>> = (0..width).map(|column| (0..rows).map(|row| {
                    let u = if row/12<2 {1.0} else {-1.0};
                    let v = if (row/12)%2==0 {1.0} else {-1.0};
                    match column%4 { 0 => 0.7*u-0.4*v+4.0*u*v, 1 => u, 2 => v, _ => 0.0 }
                }).collect()).collect();
                let columns: Vec<Vec<_>> = truth.iter().enumerate().map(|(column, values)| values.iter().enumerate().map(|(row,&value)| {
                    value + if column%4==3 {1.0} else {2e-9*(left[row]+1) as f64+1e-9*(right[row]+1) as f64}
                }).collect()).collect();
                for weight_scale in [None,Some(1.0),Some(1e-100),Some(1e100)] {
                    let raw = weight_scale.map(|scale| (0..rows).map(|row| scale*(0.25+(left[row]+1) as f64/4.0+(right[row]+1) as f64/8.0)).collect::<Vec<_>>());
                    let weights = raw.as_deref().map_or(BorrowedMapWeights::Unit,BorrowedMapWeights::Provided);
                    let input = DenseMatrix::from_columns(&columns).unwrap();
                    let mut norms = vec![0.0;width];
                    let mut scales = vec![0.0;width];
                    let mut sums = vec![0.0;width];
                    weighted_column_norms(&input,validate_weights(weights,rows).unwrap(),&mut norms,&mut scales,&mut sums, &mut []).unwrap();
                    let result = absorb_borrowed_intercept_cg(&design, weights, input.clone(), MapConfig {tolerance:1e-12,..config()}, BorrowedCgOptions { retain_coefficients: true, ..BorrowedCgOptions::polish(&norms) }).unwrap();
                    let (within, report, coefficients) = result.into_full_parts();
                    assert!(report.certified());
                    let coefficients = coefficients.unwrap();
                    for (column, expected) in truth.iter().enumerate() {
                        let product = coefficients.rhs(column).unwrap();
                        for row in 0..rows {
                            let fitted = product[coefficients.effect_offsets()[0]+left[row] as usize] + product[coefficients.effect_offsets()[1]+right[row] as usize];
                            assert!((within[(row,column)]-expected[row]).abs()<1e-12,"threads={threads} width={width} scale={weight_scale:?} row={row} col={column}");
                            assert!((input[(row,column)]-within[(row,column)]-fitted).abs()<1e-12);
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn cmg_near_within_hadamard_single_rhs_matches_analytic_projection() {
    use crate::fe::{CmgAbsorber, CmgConfig};
    let rows = 48;
    let left: Vec<_> = (0..rows).map(|row| (row % 4) as u32).collect();
    let right: Vec<_> = (0..rows).map(|row| ((row / 4) % 3) as u32).collect();
    let li = LevelIndex::from_codes(&left, 4).unwrap();
    let ri = LevelIndex::from_codes(&right, 3).unwrap();
    let ls = ValidatedBorrowedInterceptSource::new(&left, 4, &li).unwrap();
    let rs = ValidatedBorrowedInterceptSource::new(&right, 3, &ri).unwrap();
    let design = BorrowedInterceptFeDesign::new(rows, vec![ls.effect(), rs.effect()]).unwrap();
    let u: Vec<_> = (0..rows)
        .map(|row| if row / 12 < 2 { 1.0 } else { -1.0 })
        .collect();
    let v: Vec<_> = (0..rows)
        .map(|row| if (row / 12) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let x1: Vec<_> = (0..rows)
        .map(|row| u[row] + 2e-9 * (left[row] + 1) as f64)
        .collect();
    let x2: Vec<_> = (0..rows)
        .map(|row| v[row] + 1e-9 * (right[row] + 1) as f64)
        .collect();
    let y: Vec<_> = (0..rows)
        .map(|row| {
            0.7 * x1[row] - 0.4 * x2[row]
                + 2e-9 * (left[row] + 1) as f64
                + 1e-9 * (right[row] + 1) as f64
                + 4.0 * u[row] * v[row]
        })
        .collect();
    let raw: Vec<_> = (0..rows)
        .map(|row| 0.25 + (left[row] + 1) as f64 / 4.0 + (right[row] + 1) as f64 / 8.0)
        .collect();
    let weights = BorrowedMapWeights::Provided(&raw);
    let raw_residual: Vec<_> = (0..rows)
        .map(|row| y[row] - 0.7 * x1[row] - (-0.4) * x2[row])
        .collect();
    for retain_coefficients in [false, true] {
        let options = CmgConfig {
            map: MapConfig {
                tolerance: 1e-12,
                ..config()
            },
            threads: crate::ThreadConfig::from_requested(1, true).unwrap(),
            batch_size: 1,
            retain_coefficients,
            requested_solver: FeSolverKind::Cmg,
            components: true,
            solver_memory_budget: None,
        };
        let session_design =
            BorrowedInterceptFeDesign::new(rows, vec![ls.effect(), rs.effect()]).unwrap();
        let mut session = CmgAbsorber::build(session_design, weights, options, false).unwrap();
        for (rhs, column) in [y.clone(), x1.clone(), x2.clone(), vec![1.0; rows]]
            .into_iter()
            .enumerate()
        {
            let result = session
                .absorb(DenseMatrix::from_columns(&[column]).unwrap())
                .unwrap_or_else(|error| panic!("RHS {rhs}: {error}"));
            assert!(result.report().certified());
            for row in 0..rows {
                let expected = match rhs {
                    0 => 0.7 * u[row] - 0.4 * v[row] + 4.0 * u[row] * v[row],
                    1 => u[row],
                    2 => v[row],
                    _ => 0.0,
                };
                assert!((result.residualized()[(row, 0)] - expected).abs() < 1e-12);
            }
        }
        let result = session
            .absorb_raw_final_residual(
                DenseMatrix::from_columns(&[raw_residual.clone()]).unwrap(),
                1e-14,
            )
            .expect("raw final residual");
        assert!(result.report().certified());
        for row in 0..rows {
            assert!((result.residualized()[(row, 0)] - 4.0 * u[row] * v[row]).abs() < 1e-12);
        }
        // Bulk failure may omit its coefficient product. Exercise strict polishing
        // directly as well so retained correction reconstruction is mandatory.
        let mut signatures = std::collections::BTreeMap::new();
        for workers in [1, 1, 4] {
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    for width in [1, 4, 17, 65] {
                        let input =
                            DenseMatrix::from_columns(&vec![raw_residual.clone(); width]).unwrap();
                        let mut norms = vec![0.0; width];
                        weighted_column_norms(
                            &input,
                            validate_weights(weights, rows).unwrap(),
                            &mut norms,
                            &mut vec![0.0; width],
                            &mut vec![0.0; width],
                            &mut [],
                        )
                        .unwrap();
                        let result = absorb_borrowed_intercept_cg(
                            &design,
                            weights,
                            input.clone(),
                            MapConfig {
                                tolerance: 1e-14,
                                ..config()
                            },
                            BorrowedCgOptions {
                                retain_coefficients,
                                ..BorrowedCgOptions::polish(&norms)
                            },
                        )
                        .unwrap();
                        let (within, report, coefficients) = result.into_full_parts();
                        assert!(report.certified());
                        assert_eq!(coefficients.is_some(), retain_coefficients);
                        let signature = (
                            within
                                .as_slice()
                                .iter()
                                .map(|x| x.to_bits())
                                .collect::<Vec<_>>(),
                            report.iterations,
                            report.used_solver,
                            report.stop_reason,
                        );
                        if let Some(prior) = signatures.insert(width, signature.clone()) {
                            assert_eq!(prior, signature, "workers={workers}, width={width}");
                        }
                        for rhs in 0..width {
                            for row in 0..rows {
                                assert!((within[(row, rhs)] - 4.0 * u[row] * v[row]).abs() < 1e-12);
                                if let Some(coefficients) = coefficients.as_ref() {
                                    let product = coefficients.rhs(rhs).unwrap();
                                    let fitted = product
                                        [coefficients.effect_offsets()[0] + left[row] as usize]
                                        + product[coefficients.effect_offsets()[1]
                                            + right[row] as usize];
                                    assert!(
                                        (input[(row, rhs)] - within[(row, rhs)] - fitted).abs()
                                            < 1e-14
                                    );
                                }
                            }
                        }
                    }
                })
                .unwrap();
        }
    }
}

#[test]
fn borrowed_cg_constant_energy_roundoff_preserves_weighted_projection() {
    let n = 45;
    let codes = vec![0_u32; n];
    let index = LevelIndex::from_codes(&codes, 1).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(n, vec![source.effect()]).unwrap();
    let weights = (0..n).map(|r| (1 + r % 7) as f64).collect::<Vec<_>>();
    let x = (0..n).map(|r| (r % 5) as f64 - 2.0).collect::<Vec<_>>();
    let mean =
        x.iter().zip(&weights).map(|(x, w)| x * w).sum::<f64>() / weights.iter().sum::<f64>();
    let input = DenseMatrix::from_columns(&[vec![1.0; n], x.clone()]).unwrap();
    let ordinary = absorb_borrowed_intercept_cg(
        &design,
        BorrowedMapWeights::Provided(&weights),
        DenseMatrix::from_columns(&[x.clone()]).unwrap(),
        config(),
        BorrowedCgOptions {
            retain_coefficients: true,
            ..BorrowedCgOptions::default()
        },
    )
    .unwrap();
    let ordinary_error = (0..n)
        .map(|row| (ordinary.residualized()[(row, 0)] - (x[row] - mean)).abs())
        .fold(0.0_f64, f64::max);
    // The unaffected variable RHS already has cancellation error above
    // 1e-13; use the existing borrowed-CG oracle bound for that column.
    eprintln!(
        "DEBUG ordinary_error={ordinary_error:e} iterations={}",
        ordinary.report().iterations
    );
    assert!(ordinary_error <= 2e-10);
    let recovered = absorb_borrowed_intercept_cg(
        &design,
        BorrowedMapWeights::Provided(&weights),
        input,
        config(),
        BorrowedCgOptions {
            retain_coefficients: true,
            ..BorrowedCgOptions::default()
        },
    )
    .expect("constant RHS energy must not fail from a two-ulp subtraction overshoot");
    let (residual, report, coefficients) = recovered.into_full_parts();
    assert!(report.certified());
    let coefficients = coefficients.unwrap();
    for row in 0..n {
        assert!(residual[(row, 0)].abs() <= 1e-13);
        assert!((residual[(row, 1)] - (x[row] - mean)).abs() <= 2e-10);
    }
    assert!((coefficients.rhs(0).unwrap()[0] - 1.0).abs() <= 1e-13);
    assert!((coefficients.rhs(1).unwrap()[0] - mean).abs() <= 2e-10);
}

#[test]
fn hestenes_energy_audit_recomputes_remaining_energy_and_rejects_invalid_steps() {
    let unit = ValidatedWeights {
        values: None,
        raw_scale: 1.0,
    };
    let call = |values: &DenseMatrix, direction: &DenseMatrix, previous: f64, decrease: f64| {
        let mut potential = [previous];
        let error = cg_hestenes_error_and_update(
            &[1.0],
            &[decrease],
            &mut potential,
            values,
            direction,
            unit,
            false,
        )?;
        Ok::<_, FeregError>((potential[0], error))
    };
    let one = DenseMatrix::from_row_major(1, 1, vec![1.0]).unwrap();
    let two_ulps = f64::from_bits(1.0_f64.to_bits() + 2);
    let (energy, error) = call(&one, &one, 1.0, two_ulps).unwrap();
    assert_eq!(energy, f64::EPSILON);
    assert!(error > 1.0);

    let delta = 1e-7;
    let values = DenseMatrix::from_row_major(2, 1, vec![1.0 + delta, 1.0 - delta]).unwrap();
    let direction = DenseMatrix::from_row_major(2, 1, vec![1.0, 1.0]).unwrap();
    let previous = values[(0, 0)].powi(2) + values[(1, 0)].powi(2);
    let decrease = f64::from_bits(previous.to_bits() + 2);
    let expected = (values[(0, 0)] - 1.0).powi(2) + (values[(1, 0)] - 1.0).powi(2);
    let (energy, error) = call(&values, &direction, previous, decrease).unwrap();
    assert!((energy - expected).abs() <= 4.0 * f64::EPSILON * expected);
    assert!(energy > f64::EPSILON && error > 1.0);

    let large = DenseMatrix::from_row_major(1, 1, vec![1e150]).unwrap();
    let previous = 1e150_f64 * 1e150;
    let decrease = f64::from_bits(previous.to_bits() + 2);
    assert_eq!(
        cg_audit_remaining_energy(&large, &large, unit, 0, 1.0, previous, decrease).unwrap(),
        0.0
    );
    // The energy is representable, but the unchanged Hestenes quotient
    // overflows at this scale and must still fail closed.
    assert_eq!(
        call(&large, &large, previous, decrease).unwrap_err().code,
        ErrorCode::InvalidData
    );
    for (value, step) in [
        (2.0, 2.0),
        (1.0, 0.5),
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
    ] {
        let value = DenseMatrix::from_row_major(1, 1, vec![value]).unwrap();
        let step = DenseMatrix::from_row_major(1, 1, vec![step]).unwrap();
        assert_eq!(
            call(&value, &step, 1.0, two_ulps).unwrap_err().code,
            ErrorCode::InvalidData
        );
    }
    assert_eq!(
        call(&one, &one, 1.0, 2.0).unwrap_err().code,
        ErrorCode::InvalidData
    );
    assert_eq!(
        call(&one, &one, 1.0, f64::NAN).unwrap_err().code,
        ErrorCode::InvalidData
    );
}

#[test]
fn borrowed_cg_is_weighted_batched_certified_and_deterministic() {
    let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
    let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
    let worker =
        ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
    let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
    let design =
        BorrowedInterceptFeDesign::new(worker_codes.len(), vec![worker.effect(), firm.effect()])
            .expect("design");
    let weights = [1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25];
    let columns = [
        vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0],
        vec![2.0, -1.0, 4.0, 5.0, 0.0, 6.0, 3.0, 10.0],
    ];
    let run = || {
        absorb_borrowed_intercept_cg(
            &design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
            BorrowedCgOptions::default(),
        )
        .expect("CG")
    };
    let first = run();
    let second = run();
    assert!(first.report().certified());
    assert_eq!(first.report().used_solver, FeSolverKind::Cg);
    assert_eq!(first.residualized(), second.residualized());
    assert_eq!(first.report().iterations, second.report().iterations);

    let map = absorb_borrowed_intercept_map_certified(
        &design,
        BorrowedMapWeights::Provided(&weights),
        DenseMatrix::from_columns(&columns).expect("matrix"),
        config(),
    )
    .expect("MAP");
    assert_close(
        first.residualized().as_slice(),
        map.residualized().as_slice(),
        2e-10,
    );

    let input = DenseMatrix::from_columns(&columns).expect("matrix");
    let retained = absorb_borrowed_intercept_cg(
        &design,
        BorrowedMapWeights::Provided(&weights),
        input.clone(),
        config(),
        BorrowedCgOptions {
            retain_coefficients: true,
            ..BorrowedCgOptions::default()
        },
    )
    .expect("retaining CG");
    let (residualized, report, coefficients) = retained.into_full_parts();
    assert!(report.certified());
    let coefficients = coefficients.expect("CG coefficient product");
    for rhs_index in 0..input.ncols() {
        let rhs = coefficients.rhs(rhs_index).expect("coefficient RHS");
        for row in 0..input.nrows() {
            let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
                + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
            assert!(
                (fitted - (input[(row, rhs_index)] - residualized[(row, rhs_index)])).abs()
                    <= 2e-10
            );
        }
    }
}

#[test]
fn map_conversion_peak_preserves_overreservation_and_moved_layout() {
    let codes = [0_u32, 0, 1, 1];
    let index = LevelIndex::from_codes(&codes, 2).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).unwrap();
    let mut accumulator = MapCoefficientAccumulator::zeros(&design, 3).unwrap();
    accumulator.values.try_reserve_exact(17).unwrap();
    accumulator.effect_offsets.try_reserve_exact(11).unwrap();
    accumulator.level_counts.try_reserve_exact(7).unwrap();
    accumulator
        .values
        .copy_from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let source_bytes = accumulator.values.capacity() as u128 * 8
        + accumulator.effect_offsets.capacity() as u128 * std::mem::size_of::<usize>() as u128;
    let level_capacity = accumulator.level_counts.capacity();
    let (batch, conversion_peak) = accumulator.into_batch().unwrap();
    assert_eq!(batch.rhs(0).unwrap(), &[1.0, 4.0]);
    assert_eq!(batch.rhs(1).unwrap(), &[2.0, 5.0]);
    assert_eq!(batch.rhs(2).unwrap(), &[3.0, 6.0]);
    assert_eq!(batch.level_counts.capacity(), level_capacity);
    assert_eq!(conversion_peak, source_bytes + batch.retained_bytes());
    assert!(conversion_peak > 2 * 6 * 8);
}

#[test]
fn retained_map_level_major_accumulator_reconstructs_every_rhs() {
    let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
    let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
    let worker =
        ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
    let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
    let design =
        BorrowedInterceptFeDesign::new(worker_codes.len(), vec![worker.effect(), firm.effect()])
            .expect("design");
    let weights = [1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25];
    let columns = [
        vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0],
        vec![2.0, -1.0, 4.0, 5.0, 0.0, 6.0, 3.0, 10.0],
        vec![-3.0, 0.5, 1.0, 2.5, 7.0, -4.0, 6.0, 8.0],
    ];
    let input = DenseMatrix::from_columns(&columns).expect("matrix");
    let retained = absorb_borrowed_intercept_map(
        &design,
        BorrowedMapWeights::Provided(&weights),
        input.clone(),
        config(),
        false,
        true,
    )
    .expect("retaining MAP");
    let (residualized, report, coefficients) = retained.into_full_parts();
    assert!(report.certified());
    let coefficients = coefficients.expect("MAP coefficient product");
    assert!(
        report.solver_workspace_peak.bytes().unwrap() as u128
            >= coefficients.retained_bytes() + 2 * 7 * 3 * 8
    );
    for rhs_index in 0..input.ncols() {
        let rhs = coefficients.rhs(rhs_index).expect("coefficient RHS");
        for row in 0..input.nrows() {
            let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
                + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
            assert!(
                (fitted - (input[(row, rhs_index)] - residualized[(row, rhs_index)])).abs()
                    <= 2e-10
            );
        }
    }
}

#[test]
fn borrowed_cg_breakdown_and_exhaustion_fail_closed() {
    let mut output = [0.0, 0.0];
    for denominator in [0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let error = cg_ratios_into(
            &[1.0, 0.0],
            &[denominator, 0.0],
            &mut output,
            "injected denominator breakdown",
        )
        .expect_err("breakdown");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    let mut improvement = [10.0, 20.0];
    let values = DenseMatrix::from_row_major(1, 2, vec![1.0, 1.0]).unwrap();
    let error = cg_hestenes_error_and_update(
        &[0.5, 0.25],
        &[4.0, 8.0],
        &mut improvement,
        &values,
        &values,
        ValidatedWeights {
            values: None,
            raw_scale: 1.0,
        },
        false,
    )
    .expect("Hestenes update");
    assert!((error - 0.5).abs() <= f64::EPSILON);
    assert_eq!(improvement, [8.0, 18.0]);
    assert_eq!(
        cg_hestenes_error_and_update(
            &[2.0, 0.0],
            &[1.0, 0.0],
            &mut [1.0, 1.0],
            &values,
            &values,
            ValidatedWeights {
                values: None,
                raw_scale: 1.0
            },
            false,
        )
        .expect_err("negative improvement potential")
        .code,
        ErrorCode::InvalidData
    );

    let first_codes = [0_u32, 0, 0, 1, 1, 1];
    let second_codes = [0_u32, 0, 1, 0, 1, 1];
    let first_index = LevelIndex::from_codes(&first_codes, 2).expect("first index");
    let second_index = LevelIndex::from_codes(&second_codes, 2).expect("second index");
    let first =
        ValidatedBorrowedInterceptSource::new(&first_codes, 2, &first_index).expect("first");
    let second =
        ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).expect("second");
    let design =
        BorrowedInterceptFeDesign::new(first_codes.len(), vec![first.effect(), second.effect()])
            .expect("design");
    let input = DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0]]).expect("matrix");
    // Independent Euclidean norm, not the solver's projection recurrence.
    let norms = [160.0_f64.sqrt()];
    for options in [
        BorrowedCgOptions::default(),
        BorrowedCgOptions::polish(&norms),
        BorrowedCgOptions {
            retain_coefficients: true,
            ..BorrowedCgOptions::polish(&norms)
        },
    ] {
        let error = absorb_borrowed_intercept_cg(
            &design,
            BorrowedMapWeights::Unit,
            input.clone(),
            MapConfig {
                tolerance: 1e-16,
                max_iterations: 1,
                check_every: 1,
                symmetric: true,
            },
            options,
        )
        .expect_err("iteration exhaustion");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert!(error.message.contains("exhausted 1 iterations"));
    }
}

#[test]
fn borrowed_irons_tuck_retains_a_reconstructing_coefficient_product() {
    let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
    let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2, 2, 0, 1, 2];
    let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
    let worker =
        ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
    let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
    let design =
        BorrowedInterceptFeDesign::new(worker_codes.len(), vec![worker.effect(), firm.effect()])
            .expect("design");
    let input = DenseMatrix::from_columns(&[
        (0..worker_codes.len())
            .map(|row| (row as f64 * 0.37).sin())
            .collect(),
        (0..worker_codes.len())
            .map(|row| (row as f64 * 0.19).cos())
            .collect(),
    ])
    .expect("matrix");
    let result = absorb_borrowed_intercept_irons_tuck_certified(
        &design,
        BorrowedMapWeights::Unit,
        input.clone(),
        config(),
        true,
        None,
        false,
    )
    .expect("retaining Irons-Tuck");
    let (residualized, report, coefficients) = result.into_full_parts();
    assert!(report.certified());
    assert_eq!(report.used_solver, FeSolverKind::IronsTuck);
    let coefficients = coefficients.expect("Irons-Tuck coefficient product");
    for rhs_index in 0..input.ncols() {
        let rhs = coefficients.rhs(rhs_index).expect("coefficient RHS");
        for row in 0..input.nrows() {
            let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
                + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
            assert!(
                (fitted - (input[(row, rhs_index)] - residualized[(row, rhs_index)])).abs()
                    <= 2e-10
            );
        }
    }
}

#[test]
fn structural_auto_uses_direct_groups_and_one_two_fe_graph() {
    let one_codes = [0_u32, 0, 1, 1];
    let one_index = LevelIndex::from_codes(&one_codes, 2).expect("one index");
    let one = ValidatedBorrowedInterceptSource::new(&one_codes, 2, &one_index).expect("one");
    let one_design = BorrowedInterceptFeDesign::new(4, vec![one.effect()]).expect("one design");
    let direct = project_borrowed_intercept_structural(
        &one_design,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&[vec![1.0, 3.0, 4.0, 10.0]])
            .expect("one matrix")
            .into(),
        config(),
        LsmrConfig::default(),
        None,
        false,
        false,
        false,
        false,
        None,
        None,
    )
    .expect("direct auto");
    assert_eq!(direct.report().requested_solver, FeSolverKind::Auto);
    assert_eq!(direct.report().used_solver, FeSolverKind::Map);
    assert_eq!(direct.report().pilot_iterations, 0);

    let row_codes = [0_u32, 0, 1, 1];
    let column_codes = [0_u32, 1, 0, 1];
    let row_index = LevelIndex::from_codes(&row_codes, 2).expect("row index");
    let column_index = LevelIndex::from_codes(&column_codes, 2).expect("column index");
    let row = ValidatedBorrowedInterceptSource::new(&row_codes, 2, &row_index).expect("row");
    let column =
        ValidatedBorrowedInterceptSource::new(&column_codes, 2, &column_index).expect("column");
    let orthogonal = BorrowedInterceptFeDesign::new(4, vec![row.effect(), column.effect()])
        .expect("orthogonal design");
    let early = project_borrowed_intercept_structural(
        &orthogonal,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&[vec![1.0, 2.0, 4.0, 8.0]])
            .expect("early matrix")
            .into(),
        config(),
        LsmrConfig::default(),
        None,
        false,
        false,
        false,
        false,
        None,
        None,
    )
    .expect("early Schur CG");
    assert_eq!(early.report().used_solver, FeSolverKind::Cg);
    assert_eq!(early.report().pilot_iterations, 0);

    let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
    let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
    let worker =
        ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
    let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
    let hard =
        BorrowedInterceptFeDesign::new(worker_codes.len(), vec![worker.effect(), firm.effect()])
            .expect("hard design");
    let cg = project_borrowed_intercept_structural(
        &hard,
        BorrowedMapWeights::Provided(&[1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25]),
        DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0]])
            .expect("CG matrix")
            .into(),
        MapConfig {
            tolerance: 1e-14,
            ..config()
        },
        LsmrConfig::default(),
        None,
        false,
        false,
        false,
        false,
        None,
        None,
    )
    .expect("CG auto");
    assert_eq!(cg.report().requested_solver, FeSolverKind::Auto);
    assert_eq!(cg.report().used_solver, FeSolverKind::Cg);
    assert_eq!(cg.report().pilot_iterations, 0);
    assert!(cg.report().pilot_contraction.is_finite());
    assert!(cg.report().certified());
}

#[test]
fn structural_graph_preserves_slow_chain_and_retained_coefficients() {
    let workers = 160_usize;
    let mut worker_codes = Vec::with_capacity(workers * 2);
    let mut firm_codes = Vec::with_capacity(workers * 2);
    let mut outcome = Vec::with_capacity(workers * 2);
    for worker in 0..workers {
        worker_codes.extend([worker as u32, worker as u32]);
        firm_codes.extend([worker as u32, (worker + 1) as u32]);
        outcome.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
    }
    let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
    let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
        .expect("worker");
    let firm =
        ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index).expect("firm");
    let design =
        BorrowedInterceptFeDesign::new(worker_codes.len(), vec![worker.effect(), firm.effect()])
            .expect("design");
    let input = DenseMatrix::from_columns(&[outcome]).expect("matrix");
    let result = project_borrowed_intercept_structural(
        &design,
        BorrowedMapWeights::Unit,
        input.clone().into(),
        config(),
        LsmrConfig::default(),
        None,
        false,
        true,
        false,
        false,
        None,
        None,
    )
    .expect("routed auto");
    assert_eq!(result.report().requested_solver, FeSolverKind::Auto);
    assert_eq!(result.report().used_solver, FeSolverKind::Cg);
    assert_eq!(result.report().pilot_iterations, 0);
    assert!(result.report().pilot_contraction.is_finite());
    assert!(result.report().certified());
    let direct = project_borrowed_intercept_structural(
        &design,
        BorrowedMapWeights::Unit,
        input.clone().into(),
        config(),
        LsmrConfig::default(),
        None,
        false,
        false,
        false,
        false,
        None,
        None,
    )
    .expect("direct LSMR auto");
    assert_eq!(direct.report().requested_solver, FeSolverKind::Auto);
    assert_eq!(direct.report().used_solver, FeSolverKind::Cg);
    assert_eq!(direct.report().pilot_iterations, 0);
    assert!(direct.report().pilot_contraction.is_finite());
    assert!(direct.report().certified());
    let (residualized, _, coefficients) = result.into_full_parts();
    let coefficients = coefficients.expect("retained AUTO coefficient product");
    let rhs = coefficients.rhs(0).expect("coefficient RHS");
    for row in 0..input.nrows() {
        let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
            + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
        assert!((fitted - (input[(row, 0)] - residualized[(row, 0)])).abs() <= 1e-9);
    }
}

#[test]
fn lsmr_continuation_certificate_uses_the_supplied_original_rhs_scale() {
    let workers = 160_usize;
    let observations = workers * 2;
    let mut worker_codes = Vec::with_capacity(observations);
    let mut firm_codes = Vec::with_capacity(observations);
    let mut outcome = Vec::with_capacity(observations);
    for worker in 0..workers {
        worker_codes.extend([worker as u32, worker as u32]);
        firm_codes.extend([worker as u32, (worker + 1) as u32]);
        outcome.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
    }
    let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
    let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
        .expect("worker");
    let firm =
        ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index).expect("firm");
    let design = BorrowedInterceptFeDesign::new(observations, vec![worker.effect(), firm.effect()])
        .expect("design");
    let values = DenseMatrix::from_columns(&[outcome]).expect("matrix");
    let weights = validate_weights(BorrowedMapWeights::Unit, observations).expect("weights");
    let mut reference_norms = vec![0.0; values.ncols()];
    let mut scales = vec![0.0; values.ncols()];
    let mut square_sums = vec![0.0; values.ncols()];
    weighted_column_norms(
        &values,
        weights,
        &mut reference_norms,
        &mut scales,
        &mut square_sums,
        &mut [],
    )
    .expect("reference norms");
    for norm in &mut reference_norms {
        *norm *= 1_000.0;
    }
    let lsmr = LsmrConfig {
        tolerance: 1e-14,
        max_iterations: 1,
        preconditioner: LsmrPreconditioner::Schwarz,
        ..LsmrConfig::default()
    };
    let (ordinary, ordinary_report, _, _) = run_borrowed_lsmr(
        &design,
        weights,
        &values,
        lsmr.tolerance,
        lsmr.max_iterations,
        lsmr,
        None,
        false,
    )
    .expect("ordinary LSMR certificate");
    let (continued, continued_report, _, _) = run_borrowed_lsmr(
        &design,
        weights,
        &values,
        lsmr.tolerance,
        lsmr.max_iterations,
        lsmr,
        Some(&reference_norms),
        false,
    )
    .expect("continuation LSMR certificate");
    assert_eq!(ordinary, continued);
    assert!(ordinary_report.certificate.relative_infinity_norm > 0.0);
    assert!(
        continued_report.certificate.relative_infinity_norm
            < ordinary_report.certificate.relative_infinity_norm / 100.0
    );
}

#[test]
fn uncertified_lsmr_candidate_is_polished_and_fully_accounted() {
    let workers = 160_usize;
    let observations = workers * 2;
    let mut worker_codes = Vec::with_capacity(observations);
    let mut firm_codes = Vec::with_capacity(observations);
    let mut outcome = Vec::with_capacity(observations);
    for worker in 0..workers {
        worker_codes.extend([worker as u32, worker as u32]);
        firm_codes.extend([worker as u32, (worker + 1) as u32]);
        outcome.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
    }
    let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
    let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
        .expect("worker");
    let firm =
        ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index).expect("firm");
    let design = BorrowedInterceptFeDesign::new(observations, vec![worker.effect(), firm.effect()])
        .expect("design");
    let mut strict = config();
    strict.tolerance = 1e-14;
    let input = DenseMatrix::from_columns(&[outcome]).expect("matrix");
    let result = project_borrowed_intercept_structural(
        &design,
        BorrowedMapWeights::Unit,
        input.clone().into(),
        strict,
        LsmrConfig {
            tolerance: 1e-14,
            max_iterations: 1,
            preconditioner: LsmrPreconditioner::Off,
            ..LsmrConfig::default()
        },
        None,
        false,
        true,
        false,
        false,
        None,
        None,
    )
    .expect("polished auto result");
    let report = result.report();
    assert!(report.certified());
    assert!(report.iterations <= strict.max_iterations);
    let (residualized, _, coefficients) = result.into_full_parts();
    let coefficients = coefficients.expect("combined pilot, LSMR, and CG coefficients");
    let rhs = coefficients.rhs(0).expect("coefficient RHS");
    for row in 0..observations {
        let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
            + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
        assert!((fitted - (input[(row, 0)] - residualized[(row, 0)])).abs() <= 1e-9);
    }
}

#[test]
fn constrained_joint_memory_routes_lsmr_to_cg_and_fails_cleanly_when_neither_fits() {
    let workers = 160_usize;
    let observations = workers * 2;
    let mut worker_codes = Vec::with_capacity(observations);
    let mut firm_codes = Vec::with_capacity(observations);
    let mut outcome = Vec::with_capacity(observations);
    for worker in 0..workers {
        worker_codes.extend([worker as u32, worker as u32]);
        firm_codes.extend([worker as u32, (worker + 1) as u32]);
        outcome.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
    }
    let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
    let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
        .expect("worker");
    let firm =
        ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index).expect("firm");
    let design = BorrowedInterceptFeDesign::new(observations, vec![worker.effect(), firm.effect()])
        .expect("design");
    let lsmr = LsmrConfig {
        threads: crate::options::ThreadConfig::from_requested(1, true).expect("threads"),
        ..LsmrConfig::default()
    };
    let memory = estimate_fe_solver_memory(
        observations,
        1,
        &[workers, workers + 1],
        rayon::current_num_threads(),
        lsmr.local_size,
    )
    .expect("solver memory");
    assert!(memory.cg_bytes < memory.lsmr_schwarz_bytes);
    let weights: Vec<f64> = (0..observations)
        .map(|row| 0.5 + (row % 19) as f64 / 19.0)
        .collect();
    let run = |usable_bytes| {
        project_borrowed_intercept_structural(
            &design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(std::slice::from_ref(&outcome))
                .expect("matrix")
                .into(),
            config(),
            lsmr,
            Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes,
            }),
            false,
            false,
            false,
            false,
            None,
            None,
        )
    };
    // The original certificate norm remains live beside the CG continuation.
    let cg_required = memory.cg_bytes;
    let cg = run(cg_required).expect("memory-routed CG");
    assert_eq!(cg.report().used_solver, FeSolverKind::Cg);
    assert!(!cg.report().fallback_used);
    assert_eq!(
        cg.report().solver_workspace_peak.bytes(),
        Some(cg_required as u64)
    );

    // The old model charged the compensation table but omitted the equally
    // sized primary table. Check rejection before any LSMR numerical work.
    let bounded = run(327_680).expect("small budget must use certified CG");
    assert!(bounded.report().certified());
    assert!(!bounded.report().fallback_used);
    for (&a, &b) in bounded
        .residualized()
        .as_slice()
        .iter()
        .zip(cg.residualized().as_slice())
    {
        assert!((a - b).abs() < 1e-9);
    }
    assert!(bounded.report().solver_workspace_peak.bytes().unwrap() <= 327_680);

    let error = run(0).expect_err("neither route may allocate");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("workspace"));
}

#[test]
fn qualification_matrix_exercises_direct_tolerances_preconditioners_and_forced_cg() {
    let workers = 48_usize;
    let observations = workers * 3;
    let mut worker_codes = Vec::with_capacity(observations);
    let mut firm_codes = Vec::with_capacity(observations);
    let mut first = Vec::with_capacity(observations);
    let mut second = Vec::with_capacity(observations);
    let mut raw_weights = Vec::with_capacity(observations);
    for worker in 0..workers {
        worker_codes.extend([worker as u32, worker as u32, worker as u32]);
        firm_codes.extend([
            worker as u32,
            worker as u32,
            ((worker + 1) % workers) as u32,
        ]);
        first.extend([
            (worker as f64 * 0.17).sin(),
            (worker as f64 * 0.11).cos(),
            (worker as f64 * 0.07).sin(),
        ]);
        second.extend([
            (worker as f64 * 0.13).cos(),
            (worker as f64 * 0.19).sin(),
            (worker as f64 * 0.23).cos(),
        ]);
        raw_weights.extend([0.25, 1.0 + (worker % 5) as f64, 3.5]);
    }
    let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm_codes, workers).expect("firm index");
    let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
        .expect("worker");
    let firm =
        ValidatedBorrowedInterceptSource::new(&firm_codes, workers, &firm_index).expect("firm");
    let design = BorrowedInterceptFeDesign::new(observations, vec![worker.effect(), firm.effect()])
        .expect("design");
    let raw = DenseMatrix::from_columns(&[first, second]).expect("matrix");
    let weights = validate_weights(BorrowedMapWeights::Provided(&raw_weights), observations)
        .expect("weights");
    let requested_tolerance = 1e-8;
    assert_eq!(direct_lsmr_tolerance(requested_tolerance), 1e-10);
    assert_eq!(direct_lsmr_tolerance(1e-15), 1e-15);

    let reference = absorb_borrowed_intercept_cg(
        &design,
        BorrowedMapWeights::Provided(&raw_weights),
        raw.clone(),
        MapConfig {
            tolerance: requested_tolerance,
            ..config()
        },
        BorrowedCgOptions::default(),
    )
    .expect("CG reference");

    for preconditioner in [
        crate::fe::LsmrPreconditioner::Off,
        crate::fe::LsmrPreconditioner::Diagonal,
        crate::fe::LsmrPreconditioner::Schwarz,
    ] {
        for tolerance in [
            requested_tolerance,
            direct_lsmr_tolerance(requested_tolerance),
        ] {
            let (candidate, report, _, _) = run_borrowed_lsmr(
                &design,
                weights,
                &raw,
                tolerance,
                16_000,
                LsmrConfig {
                    tolerance,
                    preconditioner,
                    ..LsmrConfig::default()
                },
                None,
                false,
            )
            .expect("qualification direct LSMR arm");
            assert!(
                report.certified(),
                "{preconditioner:?} {tolerance:e}: {report:?}"
            );
            assert_close(
                candidate.as_slice(),
                reference.residualized().as_slice(),
                2e-7,
            );
        }
    }

    let (direct, direct_report, _, _) = run_borrowed_lsmr(
        &design,
        weights,
        &raw,
        direct_lsmr_tolerance(requested_tolerance),
        16_000,
        LsmrConfig {
            tolerance: direct_lsmr_tolerance(requested_tolerance),
            preconditioner: crate::fe::LsmrPreconditioner::Schwarz,
            ..LsmrConfig::default()
        },
        None,
        false,
    )
    .expect("direct Schwarz LSMR");
    assert!(direct_report.certified());
    for forced_iterations in [4_usize, 8, 16] {
        let mut reservation = SystemWorkspaceReservation;
        let refined = absorb_cg_with_reservation(
            &design,
            BorrowedMapWeights::Provided(&raw_weights),
            direct.clone(),
            MapConfig {
                tolerance: requested_tolerance,
                max_iterations: forced_iterations,
                ..config()
            },
            false,
            CgContinuation {
                reference_norms: None,
                refinement: CgRefinementPolicy::forced(forced_iterations),
                retain_coefficients: false,
                stable_projection: false,
                strict_polish: false,
                reconstruction_tolerance: None,
            },
            &mut reservation,
        )
        .expect("forced restarted CG arm");
        assert_eq!(refined.report().iterations, forced_iterations);
        assert!(refined.report().certified());
        assert_close(
            refined.residualized().as_slice(),
            reference.residualized().as_slice(),
            2e-7,
        );
    }
}

fn assert_close(left: &[f64], right: &[f64], tolerance: f64) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        assert!((left - right).abs() <= tolerance, "{left} != {right}");
    }
}

#[test]
fn cg_relative_residual_rejects_residual_on_an_exactly_solved_column() {
    assert_eq!(
        cg_largest_relative_residual(&[0.25, 0.0], &[1.0, 0.0]).expect("solved column stays zero"),
        0.5
    );
    let error = cg_largest_relative_residual(&[0.25, 1e-300], &[1.0, 0.0])
        .expect_err("residual acquired from a zero initial norm");
    assert_eq!(error.code, ErrorCode::InvalidData);
    let error = cg_largest_relative_residual(&[0.25], &[1.0, 0.0]).expect_err("dimensions");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}

#[test]
fn fixed_leaf_norm_loans_preserve_borrowed_certificate_storage() {
    let rows = 16_385;
    let codes: Vec<_> = (0..rows).map(|r| (r % 17) as u32).collect();
    let index = LevelIndex::from_codes(&codes, 17).unwrap();
    let source = ValidatedBorrowedInterceptSource::new(&codes, 17, &index).unwrap();
    let design = BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
    for workers in [1, 4, 16] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        team.install(|| {
            for width in [1, 17, 65, 129] {
                let values = DenseMatrix::from_row_major(
                    rows,
                    width,
                    (0..rows * width)
                        .map(|i| ((i * 7 % 67) as f64 - 33.0) / 17.0)
                        .collect(),
                )
                .unwrap();
                let mut loaned = BorrowedMapWorkspace::new_with_parallelism(
                    &design,
                    rows,
                    width,
                    workers,
                    &mut SystemWorkspaceReservation,
                )
                .unwrap();
                let mut baseline = BorrowedMapWorkspace::new_with_parallelism(
                    &design,
                    rows,
                    width,
                    workers,
                    &mut SystemWorkspaceReservation,
                )
                .unwrap();
                let peak = loaned.payload_peak().unwrap();
                let pointers = (
                    loaned.parallel_dot_sums.as_ptr(),
                    loaned.parallel_group_means.as_ptr(),
                );
                for frame in 0..2 {
                    let raw: Vec<_> = (0..rows).map(|r| 0.5 + ((r + frame) % 7) as f64).collect();
                    let weights =
                        validate_weights(BorrowedMapWeights::Provided(&raw), rows).unwrap();
                    loaned.parallel_dot_sums.fill(f64::NAN);
                    loaned.parallel_group_means.fill(f64::NAN);
                    update_workspace_norms(&values, weights, &mut loaned, true).unwrap();
                    weighted_column_norms(
                        &values,
                        weights,
                        &mut baseline.reference_norms,
                        &mut baseline.value_scales,
                        &mut baseline.group_sums,
                        &mut [],
                    )
                    .unwrap();
                    assert_eq!(loaned.reference_norms, baseline.reference_norms);
                    let actual = certify(&design, weights, &values, 1e-11, &mut loaned).unwrap();
                    let expected =
                        certify(&design, weights, &values, 1e-11, &mut baseline).unwrap();
                    assert_eq!(actual, expected);
                    assert_eq!(loaned.payload_peak().unwrap(), peak);
                    assert_eq!(
                        (
                            loaned.parallel_dot_sums.as_ptr(),
                            loaned.parallel_group_means.as_ptr()
                        ),
                        pointers
                    );
                }
            }
        });
    }
}

#[test]
fn norm_handoff_lsmr_routes_preserve_certificates_coefficients_and_memory() {
    let rows = 513;
    let codes = [7, 11, 13].map(|levels| {
        (0..rows)
            .map(|r| ((r * 17 + r / 5) % levels) as u32)
            .collect::<Vec<_>>()
    });
    let indexes = codes
        .iter()
        .zip([7, 11, 13])
        .map(|(c, n)| LevelIndex::from_codes(c, n).unwrap())
        .collect::<Vec<_>>();
    let sources = codes
        .iter()
        .zip(&indexes)
        .zip([7, 11, 13])
        .map(|((c, i), n)| ValidatedBorrowedInterceptSource::new(c, n, i).unwrap())
        .collect::<Vec<_>>();
    let design = || {
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap()
    };
    let input = DenseMatrix::from_row_major(
        rows,
        3,
        (0..rows * 3)
            .map(|i| (i as f64 * 0.13).sin() + (i % 7) as f64)
            .collect(),
    )
    .unwrap();
    let raw_weights = (0..rows)
        .map(|r| [0.25, 1.0, 7.0][r % 3])
        .collect::<Vec<_>>();
    for workers in [1, 4] {
        let team = crate::parallel::ThreadTeam::new(
            crate::ThreadConfig::from_requested(workers, true).unwrap(),
        );
        team.install(|| {
            let mut saw_auto_lsmr = false;
            let mut saw_retained_lsmr = false;
            for automatic in [false, true] {
                for retain in [false, true] {
                    for weights in [
                        BorrowedMapWeights::Unit,
                        BorrowedMapWeights::Provided(&raw_weights),
                    ] {
                        let config = super::super::AbsorbConfig {
                            solver: FeSolverKind::Lsmr,
                            retain_coefficients: retain,
                            lsmr: LsmrConfig {
                                preconditioner: LsmrPreconditioner::Off,
                                ..LsmrConfig::default()
                            },
                            ..super::super::AbsorbConfig::default()
                        };
                        let solve = |cache, budget| {
                            let config = super::super::AbsorbConfig {
                                solver_memory_budget: budget,
                                ..config
                            };
                            if automatic {
                                return project_borrowed_intercept_structural(
                                    &design(),
                                    weights,
                                    input.clone().into(),
                                    config.map,
                                    config.lsmr,
                                    budget,
                                    false,
                                    retain,
                                    false,
                                    cache,
                                    None,
                                    None,
                                );
                            }
                            let mut session = BorrowedLsmrAbsorber::new(design(), weights, config)?;
                            session.absorb(
                                weights,
                                input.clone(),
                                super::super::MapGeometryGeneration {
                                    sample: 0,
                                    topology: 0,
                                    weights: 0,
                                },
                                config,
                                cache,
                            )
                        };
                        let normalize = |mut r: SolveReport| {
                            r.setup_seconds = 0.0;
                            r.solve_seconds = 0.0;
                            r.certification_seconds = 0.0;
                            r
                        };
                        let expected = solve(false, None).unwrap();
                        let peak = expected.report().solver_workspace_peak.bytes().unwrap() as u128;
                        for budget in [
                            None,
                            Some(FeSolverMemoryBudget {
                                execution_bytes: 0,
                                usable_bytes: peak,
                            }),
                            Some(FeSolverMemoryBudget {
                                execution_bytes: 0,
                                usable_bytes: peak - 1,
                            }),
                        ] {
                            match (solve(true, budget), solve(false, budget)) {
                                (Err(actual), Err(expected)) => {
                                    assert!(budget.is_some());
                                    assert_eq!(actual, expected);
                                }
                                (Ok(mut actual), Ok(mut expected)) => {
                                    assert!(actual.report().certified());
                                    let norms = actual.take_residual_norms();
                                    assert_eq!(norms.len(), input.ncols());
                                    if actual.report().used_solver == FeSolverKind::Lsmr {
                                        if automatic {
                                            saw_auto_lsmr = true;
                                        } else {
                                            saw_retained_lsmr = true;
                                        }
                                        let w = validate_weights(weights, rows).unwrap();
                                        for (c, norm) in norms.iter().enumerate() {
                                            let expected_norm =
                                                crate::fe::norms::reference::legacy_scalar_norm_state(
                                                    (0..rows).map(|r| {
                                                        w.normalized(r).sqrt()
                                                            * actual.residualized()[(r, c)]
                                                    }),
                                                )
                                                .0;
                                            assert_eq!(norm.to_bits(), expected_norm.to_bits());
                                        }
                                    } else {
                                        assert!(automatic);
                                        assert_eq!(actual.report().used_solver, FeSolverKind::Map);
                                        assert_eq!(norms, expected.take_residual_norms());
                                    }
                                    assert_eq!(actual.residualized(), expected.residualized());
                                    assert_eq!(actual.coefficients, expected.coefficients);
                                    assert_eq!(
                                        normalize(actual.report().clone()),
                                        normalize(expected.report().clone())
                                    );
                                }
                                _ => panic!("norm handoff changed route outcome"),
                            }
                        }
                    }
                }
            }
            assert!(saw_auto_lsmr && saw_retained_lsmr);
        })
        .unwrap();
    }
}

#[test]
fn norm_handoff_failed_lsmr_certificate_and_auto_continuation_discard_cache() {
    let rows = 513;
    let codes = [7, 11, 13].map(|n| {
        (0..rows)
            .map(|r| ((r * 17 + r / 5) % n) as u32)
            .collect::<Vec<_>>()
    });
    let indexes = codes
        .iter()
        .zip([7, 11, 13])
        .map(|(c, n)| LevelIndex::from_codes(c, n).unwrap())
        .collect::<Vec<_>>();
    let sources = codes
        .iter()
        .zip(&indexes)
        .zip([7, 11, 13])
        .map(|((c, i), n)| ValidatedBorrowedInterceptSource::new(c, n, i).unwrap())
        .collect::<Vec<_>>();
    let design = || {
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap()
    };
    let input = DenseMatrix::from_row_major(
        rows,
        2,
        (0..rows * 2).map(|i| (i as f64 * 0.17).sin()).collect(),
    )
    .unwrap();
    let config = super::super::AbsorbConfig {
        solver: FeSolverKind::Lsmr,
        lsmr: LsmrConfig {
            max_iterations: 1,
            preconditioner: LsmrPreconditioner::Off,
            ..LsmrConfig::default()
        },
        ..super::super::AbsorbConfig::default()
    };
    let generation = super::super::MapGeometryGeneration {
        sample: 0,
        topology: 0,
        weights: 0,
    };
    let mut session =
        BorrowedLsmrAbsorber::new(design(), BorrowedMapWeights::Unit, config).unwrap();
    let error = session
        .absorb(
            BorrowedMapWeights::Unit,
            input.clone(),
            generation,
            config,
            true,
        )
        .unwrap_err();
    let mut ordinary =
        BorrowedLsmrAbsorber::new(design(), BorrowedMapWeights::Unit, config).unwrap();
    assert_eq!(
        error,
        ordinary
            .absorb(
                BorrowedMapWeights::Unit,
                input.clone(),
                generation,
                config,
                false
            )
            .unwrap_err()
    );
    let estimate = estimate_fe_solver_memory(
        rows,
        input.ncols(),
        &[7, 11, 13],
        rayon::current_num_threads(),
        config.lsmr.local_size,
    )
    .unwrap();
    let mut memory =
        BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Off, None, 0).unwrap();
    let mut norms = Vec::new();
    let (_, report, _, _) = run_borrowed_lsmr_with_prepared(
        &design(),
        validate_weights(BorrowedMapWeights::Unit, rows).unwrap(),
        &input,
        config.lsmr.tolerance,
        1,
        config.lsmr,
        None,
        false,
        None,
        &mut memory,
        Some(&mut norms),
        None,
        None,
    )
    .unwrap();
    assert!(!report.certified());
    assert!(norms.is_empty());
    let solve = |cache| {
        project_borrowed_intercept_structural(
            &design(),
            BorrowedMapWeights::Unit,
            input.clone().into(),
            config.map,
            config.lsmr,
            None,
            false,
            false,
            false,
            cache,
            None,
            None,
        )
        .unwrap()
    };
    let mut expected = solve(false);
    let mut actual = solve(true);
    assert!(actual.report().certified() && actual.report().fallback_used);
    assert_eq!(actual.take_residual_norms(), expected.take_residual_norms());
    assert_eq!(actual.residualized(), expected.residualized());
    assert_eq!(actual.report().certificate, expected.report().certificate);
    assert_eq!(actual.report().iterations, expected.report().iterations);
    assert_eq!(
        actual.report().solver_workspace_peak,
        expected.report().solver_workspace_peak
    );
}

#[test]
fn adaptive_ols_schur_preserves_weighted_contrasts_across_frames_and_widths() {
    check_direct_two_fe_weighted_contrasts(false, true);
}

#[test]
fn adaptive_ols_schur_correction_preserves_scales_coefficients_and_budget() {
    check_adaptive_schur_correction(false);
}

#[test]
fn cmg_schur_correction_preserves_scales_coefficients_and_budget() {
    check_adaptive_schur_correction(true);
}

fn check_adaptive_schur_correction(cmg_schur: bool) {
    let left_codes = [0, 0, 0, 0, 1, 1, 1, 1, 2, 2];
    let right_codes = [0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let left_index = LevelIndex::from_codes(&left_codes, 3).unwrap();
    let right_index = LevelIndex::from_codes(&right_codes, 3).unwrap();
    let left = ValidatedBorrowedInterceptSource::new(&left_codes, 3, &left_index).unwrap();
    let right = ValidatedBorrowedInterceptSource::new(&right_codes, 3, &right_index).unwrap();
    let input = DenseMatrix::from_columns(&[
        vec![11., -9., 10., -10., 10., -10., 10., -10., 10., -10.],
        vec![0.; 10],
        vec![10., -10., 10., -10., 10., -10., 10., -10., 10., -10.],
    ])
    .unwrap();
    let mut reference = None;
    for workers in [1, 4, 16] {
        let result =
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let design =
                        BorrowedInterceptFeDesign::new(10, vec![left.effect(), right.effect()])
                            .unwrap();
                    let mut owner =
                        ReusableCategoricalMapAbsorber::new(design, 3, workers).unwrap();
                    owner.retain_coefficients = true;
                    if cmg_schur {
                        owner
                            .request_cmg_schur(4, Default::default(), workers, None, 0)
                            .unwrap();
                    }
                    let capacities = owner.capacities();
                    let mut repeated = None;
                    for _ in 0..2 {
                        // Simulate a recurrence/certificate discrepancy without weakening
                        // the actual independent certificate on the correction result.
                        TWO_FE_CERTIFICATE_FAILURES.set(1);
                        TWO_FE_CORRECTIONS.set(0);
                        let fit = owner
                            .absorb_ols(
                                BorrowedMapWeights::Unit,
                                input.clone().into(),
                                MapConfig {
                                    tolerance: 1e-2,
                                    max_iterations: 4,
                                    ..config()
                                },
                                false,
                            )
                            .unwrap();
                        assert_eq!(TWO_FE_CORRECTIONS.get(), 1);
                        assert_eq!(TWO_FE_CERTIFICATE_FAILURES.get(), 0);
                        let (residual, report, coefficients) = fit.into_full_parts();
                        assert!(report.certified());
                        if cmg_schur {
                            assert!(report.iterations > 0 && report.iterations <= 4);
                        } else {
                            assert_eq!(report.iterations, 3);
                        }
                        assert!(!report.fallback_used);
                        assert_eq!(owner.capacities(), capacities);
                        let coefficients = coefficients.unwrap();
                        // The exact projection on this duplicated-edge tree averages
                        // each row pair. Its first pair is [10, -10], all others likewise.
                        for row in 0..10 {
                            for rhs in 0..3 {
                                let expected = if rhs == 1 {
                                    0.
                                } else if row % 2 == 0 {
                                    10.
                                } else {
                                    -10.
                                };
                                assert!((residual[(row, rhs)] - expected).abs() < 1e-13);
                                let base = rhs * coefficients.coefficients_per_rhs;
                                let fitted = coefficients.values[base
                                    + coefficients.effect_offsets[0]
                                    + left_codes[row] as usize]
                                    + coefficients.values[base
                                        + coefficients.effect_offsets[1]
                                        + right_codes[row] as usize];
                                assert!(
                                    (input[(row, rhs)] - residual[(row, rhs)] - fitted).abs()
                                        < 1e-13
                                );
                            }
                        }
                        let normalized = validate_weights(BorrowedMapWeights::Unit, 10).unwrap();
                        let mut workspace = BorrowedMapWorkspace::new(
                            &owner.design,
                            10,
                            3,
                            &mut SystemWorkspaceReservation,
                        )
                        .unwrap();
                        update_workspace_norms(&input, normalized, &mut workspace, true).unwrap();
                        assert_eq!(owner.workspace.reference_norms, workspace.reference_norms);
                        assert!(
                            certify(&owner.design, normalized, &residual, 1e-2, &mut workspace)
                                .unwrap()
                                .passed
                        );
                        let snapshot = (residual.as_slice().to_vec(), coefficients);
                        if let Some(previous) = repeated.as_ref() {
                            assert_eq!(&snapshot, previous);
                        }
                        repeated = Some(snapshot);
                    }
                    repeated.unwrap()
                });
        if let Some(previous) = reference.as_ref() {
            assert_eq!(&result, previous);
        }
        reference = Some(result);
    }
}

#[test]
fn adaptive_ols_schur_retry_is_bounded_and_preserves_other_callers() {
    let first = [0, 0, 0, 0, 1, 1, 1, 1, 2, 2];
    let second = [0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let indices = [
        LevelIndex::from_codes(&first, 3).unwrap(),
        LevelIndex::from_codes(&second, 3).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&first, 3, &indices[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&second, 3, &indices[1]).unwrap(),
    ];
    let input =
        DenseMatrix::from_columns(&[vec![11., -9., 10., -10., 10., -10., 10., -10., 10., -10.]])
            .unwrap();
    // caller: 0 ordinary OLS, 1 final residual, 2 PPML, 3/4 CMG ordinary/final.
    for (caller, tolerance, cap, failures, breakdown, corrections, succeeds) in [
        (0, 1e-2, 4, 0, false, 0, true),
        (0, 1e-2, 4, 2, false, 1, true),
        (0, 1e-2, 1, 1, false, 0, false),
        (0, 1e-2, 4, 0, true, 0, true),
        (0, 1.5e-14, 20, 1, false, 0, true),
        (0, 1e-15, 20, 1, false, 0, true),
        (1, 1e-2, 4, 1, false, 0, true),
        (2, 1e-2, 4, 1, false, 0, true),
        (3, 1e-2, 4, 0, false, 0, true),
        (3, 1e-2, 4, 1, false, 1, true),
        (3, 1e-2, 4, 2, false, 1, false),
        (3, 1e-2, 1, 1, false, 0, false),
        (3, 1e-2, 4, 0, true, 0, false),
        (3, 1e-15, 20, 1, false, 0, false),
        (4, 1e-2, 4, 1, false, 1, true),
        (4, 1e-2, 4, 2, false, 1, false),
    ] {
        let design =
            BorrowedInterceptFeDesign::new(10, sources.iter().map(|s| s.effect()).collect())
                .unwrap();
        let mut owner = ReusableCategoricalMapAbsorber::new(design, 1, 1).unwrap();
        if caller >= 3 {
            owner
                .request_cmg_schur(4, Default::default(), 1, None, 0)
                .unwrap();
        }
        let map = MapConfig {
            tolerance,
            max_iterations: cap,
            ..config()
        };
        TWO_FE_CERTIFICATE_FAILURES.set(failures);
        TWO_FE_CORRECTIONS.set(0);
        TWO_FE_BREAKDOWN.set(breakdown);
        let result = if caller == 2 {
            owner.absorb_ppml(BorrowedMapWeights::Unit, input.clone(), map)
        } else {
            owner.absorb_ols(
                BorrowedMapWeights::Unit,
                input.clone().into(),
                map,
                caller == 1 || caller == 4,
            )
        };
        TWO_FE_CERTIFICATE_FAILURES.set(0);
        TWO_FE_BREAKDOWN.set(false);
        assert_eq!(
            result.is_ok(),
            succeeds,
            "caller={caller} tol={tolerance} cap={cap}"
        );
        assert_eq!(TWO_FE_CORRECTIONS.get(), corrections);
        if let Ok(result) = result {
            assert!(result.report().iterations <= cap);
            let map_fallback = caller <= 1 && (breakdown || failures > corrections);
            assert_eq!(result.report().fallback_used, map_fallback);
            assert_eq!(
                result.report().used_solver,
                if caller == 2 || map_fallback {
                    FeSolverKind::Map
                } else if caller >= 3 {
                    FeSolverKind::CmgSchur
                } else {
                    FeSolverKind::Cg
                }
            );
        } else if caller >= 3 {
            assert_eq!(result.unwrap_err().code, ErrorCode::NonConvergence);
        }
    }
}

#[test]
fn cmg_schur_direction_parallel_update_matches_separate_scalar_operations() {
    for workers in [1, 4] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                for (levels, stride, rhs_count) in [(17, 5, 3), (65_537, 9, 7)] {
                    let ratio: Vec<_> = (0..rhs_count).map(|r| (r + 1) as f64 / 7.0).collect();
                    let active: Vec<_> = (0..rhs_count)
                        .map(|r| if r % 3 == 0 { 0.0 } else { -1.0 })
                        .collect();
                    let direction: Vec<_> = (0..levels * stride)
                        .map(|i| (i as f64 * 0.27).sin())
                        .collect();
                    let preconditioned: Vec<_> = (0..levels * stride)
                        .map(|i| (i as f64 * 0.49).cos())
                        .collect();
                    let mut expected = direction.clone();
                    for level in 0..levels {
                        for rhs in 0..rhs_count {
                            let i = level * stride + rhs;
                            expected[i] = if active[rhs] == 0.0 {
                                0.0
                            } else {
                                let product = ratio[rhs] * direction[i];
                                preconditioned[i] + product
                            };
                        }
                    }
                    let plan = crate::parallel::StagePlan::current(
                        levels as u128 * rhs_count as u128,
                        levels,
                    );
                    assert_eq!(plan.lanes > 1, workers > 1 && levels > 17);
                    for _ in 0..2 {
                        let mut actual = direction.clone();
                        cmg_pcg_direction(
                            stride,
                            &ratio,
                            &active,
                            &preconditioned,
                            &mut actual,
                            plan,
                        )
                        .unwrap();
                        assert_eq!(
                            actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                            expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
                        );
                    }
                }
            })
            .unwrap();
    }
}

#[test]
fn cmg_schur_hierarchy_is_lazy_admitted_and_reused() {
    let first = [0, 0, 0, 0, 1, 1, 1, 1, 2, 2];
    let second = [0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let indices = [
        LevelIndex::from_codes(&first, 3).unwrap(),
        LevelIndex::from_codes(&second, 3).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&first, 3, &indices[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&second, 3, &indices[1]).unwrap(),
    ];
    let design = || {
        BorrowedInterceptFeDesign::new(10, sources.iter().map(|s| s.effect()).collect()).unwrap()
    };
    // Opposite values within every duplicated worker/firm cell give exact
    // zero FE moments while keeping both RHS norms positive.
    let orthogonal = DenseMatrix::from_columns(&[
        vec![1., -1., 2., -2., 3., -3., 4., -4., 5., -5.],
        vec![4., -4., 3., -3., 2., -2., 1., -1., 6., -6.],
    ])
    .unwrap();
    let mut nonorthogonal = orthogonal.clone();
    nonorthogonal[(0, 0)] += 1.0;
    nonorthogonal[(0, 1)] -= 2.0;
    let mut refused = ReusableCategoricalMapAbsorber::new(design(), 2, 1).unwrap();
    for sampling in [
        super::super::cmg_schur::Sampling {
            samples: 0,
            seed: 1,
        },
        super::super::cmg_schur::Sampling {
            samples: 1,
            seed: 1_u64 << 53,
        },
    ] {
        let error = refused
            .request_cmg_schur(4, sampling, 1, None, 0)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(refused.cmg_schur_request.is_none());
        assert_eq!(refused.cmg_schur_bytes().unwrap(), 0);
    }
    refused
        .request_cmg_schur(
            4,
            Default::default(),
            1,
            Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 0,
            }),
            0,
        )
        .unwrap();
    let certified = refused
        .absorb_ols(
            BorrowedMapWeights::Unit,
            orthogonal.clone().into(),
            config(),
            false,
        )
        .unwrap();
    assert_eq!(certified.residualized(), &orthogonal);
    assert_eq!(certified.report().iterations, 0);
    assert!(certified.report().certified());
    assert_eq!(refused.cmg_schur_bytes().unwrap(), 0);
    assert_eq!(refused.cmg_schur_peak().unwrap(), None);
    let error = refused
        .absorb_ols(
            BorrowedMapWeights::Unit,
            nonorthogonal.clone().into(),
            config(),
            false,
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(refused.cmg_schur_bytes().unwrap(), 0);
    assert_eq!(refused.cmg_schur_peak().unwrap(), None);

    let mut admitted = ReusableCategoricalMapAbsorber::new(design(), 2, 1).unwrap();
    admitted
        .request_cmg_schur(
            4,
            Default::default(),
            1,
            Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 1 << 30,
            }),
            0,
        )
        .unwrap();
    let first = admitted
        .absorb_ols(
            BorrowedMapWeights::Unit,
            nonorthogonal.clone().into(),
            config(),
            false,
        )
        .unwrap();
    assert!(first.report().certified());
    let bytes = admitted.cmg_schur_bytes().unwrap();
    let peak = admitted.cmg_schur_peak().unwrap();
    assert!(bytes > 0 && peak.is_some());
    let hierarchy = std::ptr::from_ref(
        admitted
            .two_fe_pcg
            .as_ref()
            .unwrap()
            .cmg_schur
            .as_ref()
            .unwrap()
            .as_ref(),
    );
    for input in [orthogonal, nonorthogonal] {
        let repeated = admitted
            .absorb_ols(BorrowedMapWeights::Unit, input.into(), config(), false)
            .unwrap();
        assert!(repeated.report().certified());
        assert_eq!(admitted.cmg_schur_bytes().unwrap(), bytes);
        assert_eq!(admitted.cmg_schur_peak().unwrap(), peak);
        assert_eq!(
            std::ptr::from_ref(
                admitted
                    .two_fe_pcg
                    .as_ref()
                    .unwrap()
                    .cmg_schur
                    .as_ref()
                    .unwrap()
                    .as_ref(),
            ),
            hierarchy,
        );
    }
}

#[test]
fn sampled_cmg_schur_preserves_exact_paired_contrasts_and_reuses_seeded_hierarchy() {
    let mut first = Vec::new();
    let mut second = Vec::new();
    let mut weights = Vec::new();
    let mut columns = vec![Vec::new(), Vec::new(), Vec::new()];
    let mut expected = vec![Vec::new(), Vec::new(), Vec::new()];
    for worker in 0..33_u32 {
        let firms: Vec<_> = if worker == 0 {
            (0..16).collect()
        } else {
            vec![worker % 16, (worker + 1) % 16]
        };
        for firm in firms {
            for sign in [1.0, -1.0] {
                first.push(worker);
                second.push(firm);
                weights.push(1.0 + ((worker + firm) % 7) as f64);
                for rhs in 0..3 {
                    let noise = if rhs == 1 {
                        0.0
                    } else {
                        sign * (rhs + 1) as f64
                    };
                    let fitted = if rhs == 1 {
                        0.0
                    } else {
                        (rhs + 1) as f64 * (worker as f64 * 0.125 - firm as f64 * 0.25)
                    };
                    columns[rhs].push(fitted + noise);
                    expected[rhs].push(noise);
                }
            }
        }
    }
    let indexes = [
        LevelIndex::from_codes(&first, 33).unwrap(),
        LevelIndex::from_codes(&second, 16).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&first, 33, &indexes[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&second, 16, &indexes[1]).unwrap(),
    ];
    let input = DenseMatrix::from_columns(&columns).unwrap();
    let expected = DenseMatrix::from_columns(&expected).unwrap();
    for workers in [1, 4] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                for seed in [0, 17, (1_u64 << 53) - 1] {
                    let design = BorrowedInterceptFeDesign::new(
                        first.len(),
                        sources.iter().map(|s| s.effect()).collect(),
                    )
                    .unwrap();
                    let mut owner =
                        ReusableCategoricalMapAbsorber::new(design, 3, workers).unwrap();
                    owner.retain_coefficients = true;
                    owner
                        .request_cmg_schur(
                            4,
                            super::super::cmg_schur::Sampling { samples: 2, seed },
                            workers,
                            None,
                            0,
                        )
                        .unwrap();
                    let mut previous = None;
                    let mut peak = None;
                    for _ in 0..2 {
                        let fit = owner
                            .absorb_ols(
                                BorrowedMapWeights::Provided(&weights),
                                input.clone().into(),
                                config(),
                                false,
                            )
                            .unwrap();
                        assert!(fit.report().certified());
                        assert!(!fit.report().fallback_used);
                        let (residual, _, coefficients) = fit.into_full_parts();
                        let coefficients = coefficients.unwrap();
                        for row in 0..first.len() {
                            for rhs in 0..3 {
                                assert!((residual[(row, rhs)] - expected[(row, rhs)]).abs() < 1e-9);
                                let base = rhs * coefficients.coefficients_per_rhs;
                                let fitted = coefficients.values
                                    [base + coefficients.effect_offsets[0] + first[row] as usize]
                                    + coefficients.values[base
                                        + coefficients.effect_offsets[1]
                                        + second[row] as usize];
                                assert!(
                                    (input[(row, rhs)] - residual[(row, rhs)] - fitted).abs()
                                        < 1e-12
                                );
                            }
                        }
                        let snapshot = (residual, coefficients);
                        if let Some(previous) = previous.as_ref() {
                            assert_eq!(&snapshot, previous);
                            assert_eq!(owner.cmg_schur_peak().unwrap(), peak);
                        }
                        previous = Some(snapshot);
                        peak = owner.cmg_schur_peak().unwrap();
                    }
                }
            })
            .unwrap();
    }
}
