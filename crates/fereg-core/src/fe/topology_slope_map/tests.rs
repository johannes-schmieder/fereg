fn shared_recovery_with_map_controls(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested: &[bool],
    map: MapConfig,
) -> Result<RecoveredCategoricalEffects> {
    crate::fe::FeSession::recover(crate::fe::session::RecoveryRequest {
        topology,
        effects,
        weights,
        target,
        requested_channels: requested,
        retained: None,
        known_connected: false,
        materialize_absorbed_sum: true,
        lsmr: crate::fe::LsmrConfig::default(),
        map,
        memory_budget: None,
        reconstruction_tolerance: None,
    })
}

use crate::fe::LsmrConfig;
fn shared_recovery_with_solve_controls(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested: &[bool],
    lsmr: LsmrConfig,
    tolerance: f64,
) -> Result<RecoveredCategoricalEffects> {
    crate::fe::FeSession::recover(crate::fe::session::RecoveryRequest {
        topology,
        effects,
        weights,
        target,
        requested_channels: requested,
        retained: None,
        known_connected: false,
        materialize_absorbed_sum: true,
        lsmr,
        map: MapConfig {
            tolerance,
            max_iterations: lsmr.max_iterations,
            ..MapConfig::default()
        },
        memory_budget: None,
        reconstruction_tolerance: None,
    })
}
fn shared_recovery_with_controls(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    requested: &[bool],
    lsmr: LsmrConfig,
    map: MapConfig,
) -> Result<RecoveredCategoricalEffects> {
    crate::fe::FeSession::recover(crate::fe::session::RecoveryRequest {
        topology,
        effects,
        weights,
        target,
        requested_channels: requested,
        retained: None,
        known_connected: false,
        materialize_absorbed_sum: true,
        lsmr,
        map,
        memory_budget: None,
        reconstruction_tolerance: None,
    })
}

use super::*;
use crate::fe::{finalize_categorical_topology, CategoricalCodeView};

fn config() -> MapConfig {
    MapConfig {
        tolerance: 1e-11,
        max_iterations: 20_000,
        check_every: 1,
        symmetric: true,
    }
}

#[derive(Default)]
struct InjectedReservation {
    calls: usize,
    fail_at: Option<usize>,
    undersize_at: Option<usize>,
    excess: usize,
}

impl SlopeWorkspaceReservation for InjectedReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        elements: usize,
        _label: &'static str,
    ) -> Result<()> {
        let call = self.calls;
        self.calls += 1;
        if self.fail_at == Some(call) {
            return Err(allocation(
                "injected intercept-slope MAP allocation failure",
            ));
        }
        if self.undersize_at != Some(call) {
            target
                .try_reserve_exact(elements + self.excess)
                .map_err(|_| allocation("test intercept-slope MAP reservation failed"))?;
        }
        Ok(())
    }

    fn reserve_usize(
        &mut self,
        target: &mut Vec<usize>,
        elements: usize,
        _label: &'static str,
    ) -> Result<()> {
        let call = self.calls;
        self.calls += 1;
        if self.fail_at == Some(call) {
            return Err(allocation(
                "injected categorical slope MAP allocation failure",
            ));
        }
        if self.undersize_at != Some(call) {
            target
                .try_reserve_exact(elements + self.excess)
                .map_err(|_| allocation("test categorical slope MAP reservation failed"))?;
        }
        Ok(())
    }
}

#[test]
fn slope_memory_model_covers_every_numeric_capacity_and_overflow() {
    for channels in [1, 2, 7] {
        for rhs in [1, 3, 19] {
            let workspace =
                SlopeWorkspace::new(channels, rhs, &mut SystemSlopeWorkspaceReservation).unwrap();
            assert_eq!(
                workspace.payload_bytes().unwrap(),
                SlopeWorkspace::modeled_bytes(channels, rhs).unwrap()
            );
        }
    }
    assert_eq!(
        SlopeWorkspace::modeled_bytes(usize::MAX, usize::MAX)
            .unwrap_err()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn slope_memory_exact_budget_shortfall_and_capacity_growth_fail_closed() {
    let group = [0, 0, 0, 1, 1, 1];
    let topology =
        finalize_categorical_topology(&[CategoricalCodeView::new(&group, 2)], &[&[0]], &[true; 6])
            .unwrap();
    let slope = [0.0, 1.0, 2.0, 0.0, 1.5, 3.0];
    let effects = [CategoricalAbsorbedEffectView::new(true, vec![&slope]).unwrap()];
    let values = DenseMatrix::from_columns(&[vec![1.0, 2.0, 4.0, 3.0, 5.0, 8.0]]).unwrap();
    let required = SlopeWorkspace::modeled_bytes(2, 1)
        .unwrap()
        .max(topology.validation_workspace_bytes().unwrap());
    let base = 1234;
    for weights in [
        BorrowedMapWeights::Unit,
        BorrowedMapWeights::Provided(&[1.0, 3.0, 2.0, 5.0, 1.0, 2.0]),
    ] {
        let expected = absorb_finalized_categorical_slope_map_certified(
            &topology,
            &effects,
            weights,
            values.clone(),
            config(),
        )
        .unwrap();
        let exact = FeSolverMemoryBudget {
            execution_bytes: base,
            usable_bytes: base + required,
        };
        let actual = absorb_finalized_categorical_slope_map_with_budget_certified(
            &topology,
            &effects,
            weights,
            values.clone(),
            config(),
            Some(exact),
        )
        .unwrap();
        assert_eq!(actual.residualized(), expected.residualized());
        assert_eq!(
            actual.report().solver_workspace_peak.bytes(),
            Some(required as u64)
        );
        let mut denied = InjectedReservation::default();
        let error = absorb_with_reservation(
            &topology,
            &effects,
            weights,
            values.clone(),
            config(),
            Some(FeSolverMemoryBudget {
                usable_bytes: exact.usable_bytes - 1,
                ..exact
            }),
            &mut denied,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(denied.calls, 0);
        let mut oversized = InjectedReservation {
            excess: 1,
            ..InjectedReservation::default()
        };
        assert_eq!(
            absorb_with_reservation(
                &topology,
                &effects,
                weights,
                values.clone(),
                config(),
                Some(exact),
                &mut oversized,
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
    }
}

#[test]
fn slope_memory_charges_interaction_validation_as_a_sequential_peak() {
    let left: Vec<u32> = (0..100).map(|row| row % 5).collect();
    let right: Vec<u32> = (0..100).map(|row| row % 7).collect();
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&left, 5),
            CategoricalCodeView::new(&right, 7),
        ],
        &[&[0, 1]],
        &[true; 100],
    )
    .unwrap();
    let validation = topology.validation_workspace_bytes().unwrap();
    assert_eq!(
        validation,
        (4 * std::mem::size_of::<super::super::CategoricalCodeView<'_>>()
            + 100 * (std::mem::size_of::<usize>() + std::mem::size_of::<u32>())) as u128
    );
    assert!(validation > SlopeWorkspace::modeled_bytes(2, 1).unwrap());
    let slope: Vec<f64> = (0..100).map(|row| row as f64).collect();
    let effects = [CategoricalAbsorbedEffectView::new(true, vec![&slope]).unwrap()];
    let values = DenseMatrix::from_columns(&[slope.clone()]).unwrap();
    let result = absorb_finalized_categorical_slope_map_with_budget_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        values,
        config(),
        Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: validation,
        }),
    )
    .unwrap();
    assert_eq!(
        result.report().solver_workspace_peak.bytes(),
        Some(validation as u64)
    );
}

#[test]
fn shared_multi_slope_recovery_certifies_known_target_at_public_tolerance() {
    let rows = 144;
    let g: Vec<u32> = (0..rows).map(|r| (r % 6) as u32).collect();
    let h: Vec<u32> = (0..rows).map(|r| ((r / 3) % 8) as u32).collect();
    let z: Vec<f64> = (1..=rows)
        .map(|r| 0.35 + (5 * r % 23) as f64 / 17.)
        .collect();
    let w: Vec<f64> = (1..=rows)
        .map(|r| 0.45 + (7 * r % 29) as f64 / 19.)
        .collect();
    let sources = [
        CategoricalCodeView::new(&g, 6),
        CategoricalCodeView::new(&h, 8),
    ];
    let topology = finalize_categorical_topology(
        &sources,
        &[&[0_usize][..], &[1_usize][..]],
        &vec![true; rows],
    )
    .unwrap();
    let effects = [
        CategoricalAbsorbedEffectView::new(false, vec![&z]).unwrap(),
        CategoricalAbsorbedEffectView::new(false, vec![&w]).unwrap(),
    ];
    // This exact FE-span construction is independent of the estimator and
    // the recovery iteration: each channel has known group coefficients.
    let target: Vec<f64> = (0..rows)
        .map(|r| 0.025 * (g[r] as f64 - 2.) * z[r] + 0.022 * (h[r] as f64 - 3.) * w[r])
        .collect();
    let weights: Vec<f64> = (0..rows)
        .map(|r| (target[r] + 0.18 * ((11 * (r + 1) % 31) as f64 / 15. - 1.)).exp())
        .collect();
    for tolerance in [1e-8, 1e-10] {
        let recovered = shared_recovery_with_map_controls(
            &topology,
            &effects,
            BorrowedMapWeights::Provided(&weights),
            &target,
            &[true, true],
            MapConfig {
                tolerance,
                check_every: 4,
                ..config()
            },
        )
        .unwrap();
        assert!(recovered.reconstruction_relative_infinity() <= tolerance);
        let (sum, channels) = recovered.into_parts();
        for r in 0..rows {
            assert!((sum[r] - target[r]).abs() <= tolerance * target[r].abs().max(1.));
            assert!(
                (channels[0].as_ref().unwrap()[r] + channels[1].as_ref().unwrap()[r] - target[r])
                    .abs()
                    <= tolerance * target[r].abs().max(1.)
            );
        }
    }
}

#[test]
fn intercept_plus_slope_and_second_intercept_effect_match_explicit_span() {
    let group = [0, 0, 0, 1, 1, 1, 2, 2, 2];
    let second = [0, 1, 2, 0, 1, 2, 0, 1, 2];
    let sources = [
        CategoricalCodeView::new(&group, 3),
        CategoricalCodeView::new(&second, 3),
    ];
    let topology =
        finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &[true; 9])
            .expect("topology");
    let slope = [0.0, 1.0, 2.0, 0.0, 2.0, 3.0, 1.0, 2.0, 4.0];
    let values = DenseMatrix::from_columns(&[
        (0..9)
            .map(|row| {
                3.0 + 0.5 * group[row] as f64
                    + (group[row] as f64 + 1.0) * slope[row]
                    + 0.25 * second[row] as f64
            })
            .collect::<Vec<_>>(),
        (0..9).map(|row| slope[row] + second[row] as f64).collect(),
    ])
    .expect("values");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, vec![&slope[..]]).expect("slope effect"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("intercept effect"),
    ];
    let result = absorb_finalized_categorical_slope_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        values,
        config(),
    )
    .expect("certified MAP");
    assert!(result.report().certified());
    assert!(result
        .residualized()
        .as_slice()
        .iter()
        .all(|value| value.abs() < 1e-9));
}

#[test]
fn slope_only_multiple_channels_match_their_explicit_local_span() {
    let firm = [0, 0, 0, 1, 1, 1];
    let worker = [0, 0, 0, 1, 1, 1];
    let sources = [
        CategoricalCodeView::new(&firm, 2),
        CategoricalCodeView::new(&worker, 2),
    ];
    let topology =
        finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &[true; 6])
            .expect("topology");
    let time = [1.0, 2.0, 3.0, 1.0, 2.0, 3.0];
    let x2 = [1.0, 4.0, 2.0, 2.0, 1.0, 5.0];
    let effects = vec![
        CategoricalAbsorbedEffectView::new(false, vec![&time]).expect("firm slope"),
        CategoricalAbsorbedEffectView::new(false, vec![&time, &x2]).expect("worker slopes"),
    ];
    let values = DenseMatrix::from_columns(&[vec![3.0, 8.0, 8.0, 4.0, 5.0, 11.0]]).expect("values");
    let result = absorb_finalized_categorical_slope_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        values,
        config(),
    )
    .expect("multiple slope-only MAP");
    assert!(result.report().certified());
    assert!(result
        .residualized()
        .as_slice()
        .iter()
        .all(|value| value.abs() < 1e-9));
}

#[test]
fn three_nonorthogonal_slope_channels_preserve_the_known_complement() {
    let group = [0_u32, 0, 0, 0];
    let sources = [CategoricalCodeView::new(&group, 1)];
    let topology =
        finalize_categorical_topology(&sources, &[&[0_usize][..]], &[true; 4]).expect("topology");
    let first = [1.0, 1.0, 0.0, 0.0];
    let second = [0.0, 1.0, 1.0, 0.0];
    let third = [0.0, 0.0, 1.0, 1.0];
    let complement = [-1.0, 1.0, -1.0, 1.0];
    let values = (0..4)
        .map(|row| 2.0 * first[row] - 3.0 * second[row] + 0.5 * third[row] + complement[row])
        .collect::<Vec<_>>();
    let effects = [
        CategoricalAbsorbedEffectView::new(false, vec![&first, &second, &third])
            .expect("three slopes"),
    ];
    let result = absorb_finalized_categorical_slope_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        DenseMatrix::from_columns(&[values]).expect("values"),
        config(),
    )
    .expect("three-slope MAP");
    assert!(result.report().certified());
    for (actual, expected) in result.residualized().as_slice().iter().zip(complement) {
        assert!((actual - expected).abs() < 1e-10);
    }
}

#[test]
fn recovery_returns_centered_crossed_effects_and_requested_channels() {
    let worker = [0_u32, 0, 1, 1, 2, 2];
    let firm = [0_u32, 1, 0, 1, 0, 1];
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker, 3),
            CategoricalCodeView::new(&firm, 2),
        ],
        &[&[0], &[1]],
        &[true; 6],
    )
    .expect("topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let worker_effect = [-1.0, -1.0, 0.25, 0.25, 0.75, 0.75];
    let firm_effect = [-0.5, 0.5, -0.5, 0.5, -0.5, 0.5];
    let target = worker_effect
        .iter()
        .zip(firm_effect)
        .map(|(left, right)| left + right)
        .collect::<Vec<_>>();
    let recovered = recover_finalized_categorical_effects_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, false],
        config(),
    )
    .expect("recovery");
    assert!(recovered.iterations() > 0);
    assert!(recovered.reconstruction_relative_infinity() < 1e-10);
    for (actual, expected) in recovered.absorbed_sum().iter().zip(&target) {
        assert!((actual - expected).abs() < 1e-10);
    }
    let saved_worker = recovered.channels()[0].as_ref().expect("worker output");
    for (actual, expected) in saved_worker.iter().zip(worker_effect) {
        assert!((actual - expected).abs() < 1e-10);
    }
    assert!(recovered.channels()[1].is_none());
}

#[test]
fn direct_slope_geometry_size_is_checked_before_allocation() {
    for channels in 1..=4 {
        assert_eq!(
            single_slope_recovery_geometry_bytes(257, channels).unwrap(),
            257 * (channels * (channels + 1)) as u128 * 8
        );
    }
    for (levels, channels) in [(0, 2), (1, 0), (1, 5)] {
        assert_eq!(
            single_slope_recovery_geometry_bytes(levels, channels)
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
    }
    assert_eq!(
        single_slope_recovery_geometry_bytes(usize::MAX, 4)
            .unwrap_err()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn direct_recovery_row_cache_preserves_bounded_moments() {
    let n = 31;
    let members: Vec<_> = (0..n).map(|r| (r * 7) % n).collect();
    let raw_weights: Vec<_> = (0..n).map(|r| (r % 7 + 1) as f64 * 1e80).collect();
    let slopes: Vec<Vec<_>> = (0..4)
        .map(|c| {
            (0..n)
                .map(|r| {
                    if c == 1 {
                        0.0
                    } else {
                        ((r * (c + 1)) as f64).sin() * 10.0_f64.powi(c as i32 * 80 - 120)
                    }
                })
                .collect()
        })
        .collect();
    for intercept in [false, true] {
        for channels in 1..=4 {
            let effect = CategoricalAbsorbedEffectView::new(
                intercept,
                slopes[..channels - usize::from(intercept)]
                    .iter()
                    .map(Vec::as_slice)
                    .collect(),
            )
            .unwrap();
            for zero_rhs in [false, true] {
                let values = DenseMatrix::from_row_major(
                    n,
                    1,
                    (0..n)
                        .map(|r| {
                            if zero_rhs {
                                0.0
                            } else {
                                (r as f64).cos() * 1e-120
                            }
                        })
                        .collect(),
                )
                .unwrap();
                for weights in [
                    BorrowedMapWeights::Unit,
                    BorrowedMapWeights::Provided(&raw_weights),
                ] {
                    let validated = validate_weights(weights, n).unwrap();
                    let mut prior =
                        SlopeWorkspace::new(channels, 1, &mut SystemSlopeWorkspaceReservation)
                            .unwrap();
                    let mut cached =
                        SlopeWorkspace::new(channels, 1, &mut SystemSlopeWorkspaceReservation)
                            .unwrap();
                    group_moments(&members, &effect, validated, &values, &mut prior).unwrap();
                    group_moments_with_row_cache(
                        &members,
                        &effect,
                        validated,
                        &values,
                        &mut cached,
                        true,
                    )
                    .unwrap();
                    for (a, b) in [
                        (&prior.gram, &cached.gram),
                        (&prior.cross, &cached.cross),
                        (&prior.gram_corrections, &cached.gram_corrections),
                        (&prior.cross_corrections, &cached.cross_corrections),
                        (&prior.channel_scales, &cached.channel_scales),
                        (&prior.value_scales, &cached.value_scales),
                    ] {
                        assert!(a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()));
                    }
                }
            }
        }
    }
    let effect =
        CategoricalAbsorbedEffectView::new(true, slopes.iter().map(Vec::as_slice).collect())
            .unwrap();
    let mut workspace = SlopeWorkspace::new(5, 2, &mut SystemSlopeWorkspaceReservation).unwrap();
    for rhs in [1, 2] {
        let values = DenseMatrix::from_row_major(n, rhs, vec![0.0; n * rhs]).unwrap();
        assert_eq!(
            group_moments_with_row_cache(
                &members,
                &effect,
                validate_weights(BorrowedMapWeights::Unit, n).unwrap(),
                &values,
                &mut workspace,
                true
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidArgument
        );
    }
}

#[test]
fn direct_single_slope_recovery_preserves_rank_weights_and_rejection() {
    let n = 4099;
    let codes: Vec<_> = (0..n).map(|r| (r % 257) as u32).collect();
    let topology = finalize_categorical_topology(
        &[CategoricalCodeView::new(&codes, 257)],
        &[&[0]],
        &vec![true; n],
    )
    .unwrap();
    for intercept in [false, true] {
        for rank_case in 0..4 {
            for scale in [1e-8, 1.0, 1e8] {
                let z: Vec<_> = (0..n).map(|r| ((r * 7) as f64).sin() * scale).collect();
                let z2: Vec<_> = (0..n)
                    .map(|r| match rank_case {
                        1 => 0.0,
                        2 => z[r],
                        3 => z[r] + scale * 1e-15 * (r as f64).cos(),
                        _ => ((r * 11) as f64).cos() * scale,
                    })
                    .collect();
                let effects =
                    [CategoricalAbsorbedEffectView::new(intercept, vec![&z, &z2]).unwrap()];
                let target: Vec<_> = (0..n)
                    .map(|r| {
                        let g = codes[r] as f64;
                        (if intercept { g.cos() } else { 0.0 })
                            + g.sin() * z[r] / scale
                            + (g * 0.7).cos() * z2[r] / scale
                    })
                    .collect();
                for weight_scale in [0.0, 1e-100, 1.0, 1e100] {
                    let raw: Vec<_> = (0..n).map(|r| weight_scale * (1 + r % 7) as f64).collect();
                    let weights = if weight_scale == 0.0 {
                        BorrowedMapWeights::Unit
                    } else {
                        BorrowedMapWeights::Provided(&raw)
                    };
                    for threads in [1, 4] {
                        crate::ThreadTeam::new(
                            crate::ThreadConfig::from_requested(threads, false).unwrap(),
                        )
                        .install(|| {
                            let requested = vec![true; effects[0].channels()];
                            let actual = recover_finalized_single_slope_direct_certified(
                                &topology, &effects, weights, &target, &requested, 1e-12, true,
                                false,
                            )
                            .unwrap();
                            let expected = recover_finalized_categorical_effects_map_certified(
                                &topology,
                                &effects,
                                weights,
                                &target,
                                &requested,
                                config(),
                            )
                            .unwrap();
                            assert_eq!(actual.recovery_method(), CategoricalRecoveryMethod::Direct);
                            let residual = DenseMatrix::from_row_major(
                                n,
                                1,
                                (0..n)
                                    .map(|r| target[r] * 0.123 + 0.1 * (r as f64).sin())
                                    .collect(),
                            )
                            .unwrap();
                            let validated = validate_weights(weights, n).unwrap();
                            let mut workspace = SlopeWorkspace::new(
                                effects[0].channels(),
                                1,
                                &mut SystemSlopeWorkspaceReservation,
                            )
                            .unwrap();
                            workspace.reference_norms[0] = 100.0;
                            let serial = certify(
                                &topology,
                                &effects,
                                validated,
                                &residual,
                                1e-12,
                                &mut workspace,
                            )
                            .unwrap();
                            let parallel = certify_single_slope_parallel(
                                &topology,
                                &effects[0],
                                validated,
                                &residual,
                                1e-12,
                                &mut workspace,
                            )
                            .unwrap();
                            assert_eq!(serial, parallel);
                            let channels = effects[0].channels();
                            let stride = channels * (channels + 1);
                            let source = topology
                                .registry()
                                .source(topology.effect_to_topology()[0])
                                .unwrap();
                            let mut geometry = vec![0.0; source.n_levels() * stride];
                            let target_matrix =
                                DenseMatrix::from_row_major(n, 1, target.clone()).unwrap();
                            let mut row_cached = SlopeWorkspace::new(
                                effects[0].channels(),
                                1,
                                &mut SystemSlopeWorkspaceReservation,
                            )
                            .unwrap();
                            for (level, cached) in geometry.chunks_mut(stride).enumerate() {
                                group_moments(
                                    source.members(level).unwrap(),
                                    &effects[0],
                                    validated,
                                    &target_matrix,
                                    &mut workspace,
                                )
                                .unwrap();
                                group_moments_with_row_cache(
                                    source.members(level).unwrap(),
                                    &effects[0],
                                    validated,
                                    &target_matrix,
                                    &mut row_cached,
                                    true,
                                )
                                .unwrap();
                                assert_eq!(workspace.gram, row_cached.gram);
                                assert_eq!(workspace.cross, row_cached.cross);
                                assert_eq!(workspace.gram_corrections, row_cached.gram_corrections);
                                assert_eq!(
                                    workspace.cross_corrections,
                                    row_cached.cross_corrections
                                );
                                assert_eq!(workspace.channel_scales, row_cached.channel_scales);
                                assert_eq!(workspace.value_scales, row_cached.value_scales);
                                cached[..channels * channels]
                                    .copy_from_slice(&workspace.gram[..channels * channels]);
                                cached[channels * channels..]
                                    .copy_from_slice(&workspace.channel_scales[..channels]);
                            }
                            let cached = certify_with_geometry(
                                &topology,
                                &effects,
                                validated,
                                &residual,
                                1e-12,
                                &mut workspace,
                                Some(&geometry),
                            )
                            .unwrap();
                            assert_eq!(
                                serial, cached,
                                "fresh residual certificate changed with geometry reuse"
                            );
                            geometry[0] = f64::NAN;
                            assert!(certify_with_geometry(
                                &topology,
                                &effects,
                                validated,
                                &residual,
                                1e-12,
                                &mut workspace,
                                Some(&geometry)
                            )
                            .is_err());
                            if threads == 1 {
                                let cached = recover_finalized_single_slope_direct_certified(
                                    &topology, &effects, weights, &target, &requested, 1e-12, true,
                                    true,
                                )
                                .unwrap();
                                assert_eq!(actual.channels(), cached.channels());
                                assert_eq!(actual.absorbed_sum(), cached.absorbed_sum());
                                let mut inconsistent = target.clone();
                                inconsistent[0] += 0.001;
                                assert!(recover_finalized_single_slope_direct_certified(
                                    &topology,
                                    &effects,
                                    weights,
                                    &inconsistent,
                                    &requested,
                                    1e-12,
                                    true,
                                    true
                                )
                                .is_err());
                            } else {
                                assert_eq!(
                                    recover_finalized_single_slope_direct_certified(
                                        &topology, &effects, weights, &target, &requested, 1e-12,
                                        true, true
                                    )
                                    .unwrap_err()
                                    .code,
                                    ErrorCode::InvalidArgument
                                );
                            }

                            for (a, b) in actual.absorbed_sum().iter().zip(&target) {
                                assert!((a - b).abs() < 1e-10);
                            }
                            for (a, b) in actual.channels().iter().zip(expected.channels()) {
                                for (a, b) in a.as_ref().unwrap().iter().zip(b.as_ref().unwrap()) {
                                    assert!(
                                        (a - b).abs() < 1e-9,
                                        "{intercept} {rank_case} {scale}: {a} {b}"
                                    );
                                }
                            }
                            let partial: Vec<_> =
                                (0..effects[0].channels()).map(|c| c == 1).collect();
                            let selected = recover_finalized_single_slope_direct_certified(
                                &topology, &effects, weights, &target, &partial, 1e-12, false,
                                false,
                            )
                            .unwrap();
                            for (c, values) in selected.channels().iter().enumerate() {
                                assert_eq!(values.is_some(), c == 1);
                            }
                            let mut inconsistent = target.clone();
                            inconsistent[0] += 0.001;
                            assert_eq!(
                                recover_finalized_single_slope_direct_certified(
                                    &topology,
                                    &effects,
                                    weights,
                                    &inconsistent,
                                    &requested,
                                    1e-12,
                                    true,
                                    false
                                )
                                .unwrap_err()
                                .code,
                                ErrorCode::InvalidData
                            );
                        })
                        .unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn two_effect_graph_recovery_certifies_disconnected_and_inconsistent_targets() {
    for disconnected in [false, true] {
        for reversed in [false, true] {
            let mut left = vec![0_u32, 0, 1, 1, 2, 2, 3, 3];
            let mut right = if disconnected {
                vec![0_u32, 1, 0, 1, 2, 3, 2, 3]
            } else {
                vec![0_u32, 1, 1, 2, 2, 3, 3, 0]
            };
            if reversed {
                left.reverse();
                right.reverse();
            }
            let topology = finalize_categorical_topology(
                &[
                    CategoricalCodeView::new(&left, 4),
                    CategoricalCodeView::new(&right, 4),
                ],
                &[&[0], &[1]],
                &[true; 8],
            )
            .unwrap();
            let effects = [
                CategoricalAbsorbedEffectView::new(true, Vec::new()).unwrap(),
                CategoricalAbsorbedEffectView::new(true, Vec::new()).unwrap(),
            ];
            let target = left
                .iter()
                .zip(&right)
                .map(|(&a, &b)| (a as f64 * 0.7).sin() + (b as f64 * 0.9).cos())
                .collect::<Vec<_>>();
            for requested in [[true, true], [true, false], [false, true]] {
                for materialize in [false, true] {
                    let actual = recover_finalized_two_effects_graph_certified(
                        &topology,
                        &effects,
                        BorrowedMapWeights::Unit,
                        &target,
                        &requested,
                        1e-12,
                        !disconnected,
                        materialize,
                    )
                    .unwrap();
                    let expected = recover_finalized_categorical_effects_map_certified(
                        &topology,
                        &effects,
                        BorrowedMapWeights::Unit,
                        &target,
                        &requested,
                        config(),
                    )
                    .unwrap();
                    assert_eq!(actual.recovery_method(), CategoricalRecoveryMethod::Direct);
                    assert!(actual.reconstruction_relative_infinity() < 1e-12);
                    for (a, b) in actual.channels().iter().zip(expected.channels()) {
                        assert_eq!(a.is_some(), b.is_some());
                        if let (Some(a), Some(b)) = (a, b) {
                            for (&a, &b) in a.iter().zip(b) {
                                assert!((a - b).abs() < 1e-10);
                            }
                        }
                    }
                    assert_eq!(actual.absorbed_sum.is_some(), materialize);
                }
            }
            let mut inconsistent = target.clone();
            inconsistent[0] += 0.001;
            assert_eq!(
                recover_finalized_two_effects_graph_certified(
                    &topology,
                    &effects,
                    BorrowedMapWeights::Unit,
                    &inconsistent,
                    &[true, true],
                    1e-12,
                    !disconnected,
                    true
                )
                .unwrap_err()
                .code,
                ErrorCode::InvalidData
            );
        }
    }
}

#[test]
fn weighted_graph_recovery_preserves_components_scale_and_pointwise_rejection() {
    for disconnected in [false, true] {
        for reverse in [false, true] {
            let mut left = vec![0_u32, 0, 1, 1, 2, 2, 3, 3];
            let mut right = if disconnected {
                vec![0_u32, 1, 0, 1, 2, 3, 2, 3]
            } else {
                vec![0_u32, 1, 1, 2, 2, 3, 3, 0]
            };
            if reverse {
                left.reverse();
                right.reverse();
            }
            let topology = finalize_categorical_topology(
                &[
                    CategoricalCodeView::new(&left, 4),
                    CategoricalCodeView::new(&right, 4),
                ],
                &[&[0], &[1]],
                &[true; 8],
            )
            .unwrap();
            let effects = [
                CategoricalAbsorbedEffectView::new(true, Vec::new()).unwrap(),
                CategoricalAbsorbedEffectView::new(true, Vec::new()).unwrap(),
            ];
            let a = [-0.3, 1.7, 0.4, -1.1];
            let b = [0.2, -0.7, 0.9, 1.3];
            let target = left
                .iter()
                .zip(&right)
                .map(|(&l, &r)| a[l as usize] + b[r as usize])
                .collect::<Vec<_>>();
            for raw in [
                vec![4.0; 8],
                vec![1.0, 3.0, 5.0, 2.0, 7.0, 4.0, 2.0, 6.0],
                vec![1e-18, 1.0, 1e-12, 1.0, 1e-8, 1.0, 1e-4, 1.0],
            ] {
                let component = |row: usize| {
                    if disconnected {
                        (left[row] / 2) as usize
                    } else {
                        0
                    }
                };
                let mut mass = [0.0; 2];
                let mut centers = [0.0; 2];
                for row in 0..8 {
                    mass[component(row)] += raw[row];
                    centers[component(row)] += raw[row] * b[right[row] as usize];
                }
                for c in 0..=usize::from(disconnected) {
                    centers[c] /= mass[c];
                }
                for scale in [1e-100, 1.0, 1e100] {
                    let weights = raw.iter().map(|w| w * scale).collect::<Vec<_>>();
                    for requested in [[true, true], [true, false], [false, true]] {
                        for materialize in [false, true] {
                            let recovered = recover_finalized_two_effects_graph_certified(
                                &topology,
                                &effects,
                                BorrowedMapWeights::Provided(&weights),
                                &target,
                                &requested,
                                1e-12,
                                !disconnected,
                                materialize,
                            )
                            .unwrap();
                            assert!(recovered.reconstruction_relative_infinity() < 1e-12);
                            for row in 0..8 {
                                let expected = [
                                    a[left[row] as usize] + centers[component(row)],
                                    b[right[row] as usize] - centers[component(row)],
                                ];
                                for channel in 0..2 {
                                    assert_eq!(
                                        recovered.channels()[channel].is_some(),
                                        requested[channel]
                                    );
                                    if let Some(values) = &recovered.channels()[channel] {
                                        assert!((values[row] - expected[channel]).abs() < 1e-12);
                                    }
                                }
                            }
                            assert_eq!(recovered.absorbed_sum.is_some(), materialize);
                        }
                    }
                    // The first edge can have negligible weight. Its cycle inconsistency
                    // must still fail the unweighted pointwise reconstruction certificate.
                    let mut inconsistent = target.clone();
                    inconsistent[0] += 0.001;
                    assert_eq!(
                        recover_finalized_two_effects_graph_certified(
                            &topology,
                            &effects,
                            BorrowedMapWeights::Provided(&weights),
                            &inconsistent,
                            &[true, true],
                            1e-12,
                            !disconnected,
                            true
                        )
                        .unwrap_err()
                        .code,
                        ErrorCode::InvalidData
                    );
                }
            }
        }
    }
}

#[test]
fn coefficient_lsmr_recovery_matches_map_normalization() {
    let worker = [0_u32, 0, 1, 1, 2, 2];
    let firm = [0_u32, 1, 0, 1, 0, 1];
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker, 3),
            CategoricalCodeView::new(&firm, 2),
        ],
        &[&[0], &[1]],
        &[true; 6],
    )
    .expect("topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let target = [-1.5, -0.5, -0.25, 0.75, 0.25, 1.25];
    let map = recover_finalized_categorical_effects_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        config(),
    )
    .expect("MAP recovery");
    let lsmr = shared_recovery_with_solve_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        LsmrConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            ..LsmrConfig::default()
        },
        1e-12,
    )
    .expect("LSMR recovery");
    for (left, right) in lsmr.absorbed_sum().iter().zip(map.absorbed_sum()) {
        assert!((left - right).abs() < 1e-10);
    }
    for channel in 0..2 {
        for (left, right) in lsmr.channels()[channel]
            .as_ref()
            .expect("LSMR channel")
            .iter()
            .zip(map.channels()[channel].as_ref().expect("MAP channel"))
        {
            assert!((left - right).abs() < 1e-10, "{left} != {right}");
        }
    }
}

#[test]
fn four_sweep_sd_pilot_then_lsmr_matches_strict_lsmr() {
    let worker = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let firm = [0_u32, 1, 1, 2, 2, 3, 3, 0];
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker, 4),
            CategoricalCodeView::new(&firm, 4),
        ],
        &[&[0], &[1]],
        &[true; 8],
    )
    .expect("topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let target = [-1.5, -0.5, -0.25, 0.75, 0.25, 1.25, 1.5, -1.5];
    let lsmr_config = LsmrConfig {
        tolerance: 1e-12,
        max_iterations: 20_000,
        ..LsmrConfig::default()
    };
    let hybrid = shared_recovery_with_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        lsmr_config,
        MapConfig {
            tolerance: 1e-12,
            ..config()
        },
    )
    .expect("SD recovery");
    let lsmr = shared_recovery_with_solve_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        lsmr_config,
        1e-12,
    )
    .expect("LSMR recovery");
    assert_eq!(hybrid.recovery_method(), CategoricalRecoveryMethod::Direct);
    assert!(hybrid.reconstruction_relative_infinity() <= 1e-10);
    for (actual, expected) in hybrid.absorbed_sum().iter().zip(lsmr.absorbed_sum()) {
        assert!((actual - expected).abs() <= 1e-10);
    }
    for channel in 0..2 {
        for (actual, expected) in hybrid.channels()[channel]
            .as_ref()
            .expect("hybrid channel")
            .iter()
            .zip(lsmr.channels()[channel].as_ref().expect("LSMR channel"))
        {
            assert!((actual - expected).abs() <= 1e-9);
        }
    }
}

#[test]
fn shared_two_fe_graph_matches_fallible_map_coefficients() {
    let worker = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let firm = [0_u32, 1, 1, 2, 2, 3, 3, 0];
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker, 4),
            CategoricalCodeView::new(&firm, 4),
        ],
        &[&[0], &[1]],
        &[true; 8],
    )
    .expect("topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let target = [-1.5, -0.5, -0.25, 0.75, 0.25, 1.25, 1.5, -1.5];
    let cg = shared_recovery_with_map_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        MapConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        },
    )
    .expect("CG recovery");
    let lsmr = recover_finalized_categorical_effects_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        MapConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        },
    )
    .expect("LSMR recovery");
    assert_eq!(cg.recovery_method(), CategoricalRecoveryMethod::Direct);
    for (actual, expected) in cg.absorbed_sum().iter().zip(lsmr.absorbed_sum()) {
        assert!((actual - expected).abs() <= 1e-10);
    }
    for channel in 0..2 {
        for (actual, expected) in cg.channels()[channel]
            .as_ref()
            .expect("CG channel")
            .iter()
            .zip(lsmr.channels()[channel].as_ref().expect("LSMR channel"))
        {
            assert!((actual - expected).abs() <= 1e-9);
        }
    }
}

#[test]
fn sd_lsmr_recovery_preserves_raw_and_owned_topology_results() {
    let group = [0_u32, 0, 1, 1, 2, 2];
    let topology =
        finalize_categorical_topology(&[CategoricalCodeView::new(&group, 3)], &[&[0]], &[true; 6])
            .unwrap();
    let effects = [CategoricalAbsorbedEffectView::new(true, Vec::new()).unwrap()];
    let supplied_weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0];
    let owned = crate::fe::ValidatedFinalizedTopology::new(topology.clone()).unwrap();
    for weights in [
        BorrowedMapWeights::Unit,
        BorrowedMapWeights::Provided(&supplied_weights),
    ] {
        for perturbation in 0..4 {
            let mut target = [-2.0, -2.0, 0.5, 0.5, 1.5, 1.5];
            match perturbation {
                1 => target[0] += 1e-13,
                2 => target[0] += 1.0,
                3 => target[0] = f64::NAN,
                _ => {}
            }
            let run = |topology| {
                shared_recovery_with_controls(
                    topology,
                    &effects,
                    weights,
                    &target,
                    &[true],
                    LsmrConfig::default(),
                    config(),
                )
            };
            assert_eq!(run(&topology), run(owned.get()));
        }
    }
}

#[test]
fn shared_recovery_removes_one_weighted_effect_in_one_pass() {
    let group = [0_u32, 0, 1, 1, 2, 2];
    let topology =
        finalize_categorical_topology(&[CategoricalCodeView::new(&group, 3)], &[&[0]], &[true; 6])
            .expect("topology");
    let effects = [CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("group")];
    let target = [-2.0, -2.0, 0.5, 0.5, 1.5, 1.5];
    let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0];
    let recovered = shared_recovery_with_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Provided(&weights),
        &target,
        &[true],
        LsmrConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            ..LsmrConfig::default()
        },
        MapConfig {
            tolerance: 1e-12,
            ..config()
        },
    )
    .expect("direct recovery");
    assert_eq!(recovered.recovery_method(), CategoricalRecoveryMethod::Map);
    assert_eq!(recovered.iterations(), 1);
    for (actual, expected) in recovered.absorbed_sum().iter().zip(target) {
        assert!((actual - expected).abs() <= 1e-12);
    }
}

#[test]
fn coefficient_lsmr_recovery_normalizes_disconnected_components() {
    let worker = [0_u32, 1, 2, 3];
    let firm = [0_u32, 0, 1, 1];
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker, 4),
            CategoricalCodeView::new(&firm, 2),
        ],
        &[&[0], &[1]],
        &[true; 4],
    )
    .expect("disconnected topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let target = [1.0, 3.0, 10.0, 14.0];
    let map = recover_finalized_categorical_effects_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        config(),
    )
    .expect("disconnected MAP recovery");
    let lsmr = shared_recovery_with_solve_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        LsmrConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            ..LsmrConfig::default()
        },
        1e-12,
    )
    .expect("disconnected LSMR recovery");
    for channel in 0..2 {
        for (left, right) in lsmr.channels()[channel]
            .as_ref()
            .expect("LSMR channel")
            .iter()
            .zip(map.channels()[channel].as_ref().expect("MAP channel"))
        {
            assert!((left - right).abs() < 1e-10, "{left} != {right}");
        }
    }
}

#[test]
fn retained_coefficient_connected_hint_matches_component_normalization() {
    let worker = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let firm = [0_u32, 1, 1, 2, 2, 3, 3, 0];
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker, 4),
            CategoricalCodeView::new(&firm, 4),
        ],
        &[&[0], &[1]],
        &[true; 8],
    )
    .expect("connected topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let coefficient_values = vec![1.0, -2.0, 0.5, 3.0, 4.0, -1.0, 2.0, -3.0];
    let coefficients =
        FeCoefficientBatch::from_values(vec![1, 1], vec![4, 4], 1, coefficient_values.clone())
            .expect("coefficient batch");
    let target = worker
        .iter()
        .zip(&firm)
        .map(|(&worker, &firm)| {
            coefficient_values[worker as usize] + coefficient_values[4 + firm as usize]
        })
        .collect::<Vec<_>>();
    let general = recover_finalized_categorical_effects_from_coefficients_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        &coefficients,
        1e-12,
        false,
    )
    .expect("general normalization");
    let connected = recover_finalized_categorical_effects_from_coefficients_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        &coefficients,
        1e-12,
        true,
    )
    .expect("known-connected normalization");
    assert_eq!(general.absorbed_sum(), connected.absorbed_sum());
    assert_eq!(general.channels(), connected.channels());
}

#[test]
fn retained_coefficient_saved_only_plan_omits_combined_observation_vector() {
    let worker = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let firm = [0_u32, 1, 1, 2, 2, 3, 3, 0];
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker, 4),
            CategoricalCodeView::new(&firm, 4),
        ],
        &[&[0], &[1]],
        &[true; 8],
    )
    .expect("connected topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let coefficient_values = vec![1.0, -2.0, 0.5, 3.0, 4.0, -1.0, 2.0, -3.0];
    let coefficients =
        FeCoefficientBatch::from_values(vec![1, 1], vec![4, 4], 1, coefficient_values.clone())
            .expect("coefficient batch");
    let target = worker
        .iter()
        .zip(&firm)
        .map(|(&worker, &firm)| {
            coefficient_values[worker as usize] + coefficient_values[4 + firm as usize]
        })
        .collect::<Vec<_>>();

    let materialized = recover_finalized_categorical_effects_from_coefficients_selected_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        &coefficients,
        1e-12,
        true,
        true,
    )
    .expect("materialized retained-coefficient recovery");
    let saved_only = recover_finalized_categorical_effects_from_coefficients_selected_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        &coefficients,
        1e-12,
        true,
        false,
    )
    .expect("saved-only retained-coefficient recovery");
    assert_eq!(materialized.channels(), saved_only.channels());
    let (materialized_sum, materialized_channels) = materialized.into_optional_parts();
    let (saved_only_sum, saved_only_channels) = saved_only.into_optional_parts();
    assert!(materialized_sum.is_some());
    assert!(saved_only_sum.is_none());
    assert_eq!(materialized_channels, saved_only_channels);
}

#[test]
fn publication_recovery_continues_to_requested_reconstruction_or_refuses() {
    const GROUPS: usize = 24;
    let mut worker_codes = Vec::new();
    let mut firm_codes = Vec::new();
    let mut component_codes = Vec::new();
    let mut target = Vec::new();
    let mut weights = Vec::new();
    for component in 0..2 {
        for firm in 0..GROUPS {
            for worker in [firm, (firm + 1) % GROUPS] {
                worker_codes.push((component * GROUPS + worker) as u32);
                firm_codes.push((component * GROUPS + firm) as u32);
                component_codes.push(component as u32);
                target.push(
                    2.0 + component as f64
                        + (std::f64::consts::TAU * worker as f64 / GROUPS as f64).cos()
                        + (std::f64::consts::TAU * firm as f64 / GROUPS as f64).sin(),
                );
                weights.push(if worker == firm { 0.5 } else { 2.0 });
            }
        }
    }
    let sources = [
        CategoricalCodeView::new(&worker_codes, 2 * GROUPS),
        CategoricalCodeView::new(&firm_codes, 2 * GROUPS),
        CategoricalCodeView::new(&component_codes, 2),
    ];
    let topology = finalize_categorical_topology(
        &sources,
        &[&[0][..], &[1][..], &[2][..]],
        &vec![true; target.len()],
    )
    .unwrap();
    let effects = (0..3)
        .map(|_| CategoricalAbsorbedEffectView::new(true, Vec::new()).unwrap())
        .collect::<Vec<_>>();
    let config = MapConfig {
        tolerance: 1e-12,
        max_iterations: 20_000,
        check_every: 1,
        symmetric: true,
    };
    let recover = |config, tolerance| {
        recover_finalized_categorical_effects_map_selected_certified(
            &topology,
            &effects,
            BorrowedMapWeights::Provided(&weights),
            &target,
            &[true; 3],
            config,
            true,
            tolerance,
        )
    };
    let relaxed = recover(config, None).unwrap();
    assert!(relaxed.reconstruction_relative_infinity() > 1e-12);
    assert!(relaxed.reconstruction_relative_infinity() <= 1e-10);
    assert_eq!(recover(config, Some(1e-8)).unwrap(), relaxed);
    let strict = recover(config, Some(1e-12)).unwrap();
    assert!(strict.iterations() > relaxed.iterations());
    assert!(strict.reconstruction_relative_infinity() <= 1e-12);
    let scale = target.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
    for (actual, expected) in strict.absorbed_sum().iter().zip(&target) {
        assert!((actual - expected).abs() / scale <= 1e-12);
        assert!((actual - expected).abs() / actual.abs().max(expected.abs()).max(1.0) <= 1e-12);
    }
    let refused = recover(
        MapConfig {
            max_iterations: relaxed.iterations(),
            ..config
        },
        Some(1e-12),
    );
    let refused = refused.unwrap_err();
    assert_eq!(refused.code, ErrorCode::NonConvergence);
    assert!(refused.message.contains("exhausted its iteration budget"));
}

#[test]
fn recovery_waits_for_reconstruction_after_an_early_moment_certificate() {
    const GROUPS: usize = 72;
    let observations = GROUPS * 2;
    let mut worker_codes = Vec::with_capacity(observations);
    let mut firm_codes = Vec::with_capacity(observations);
    let mut target = Vec::with_capacity(observations);
    for firm in 0..GROUPS {
        for worker in [firm, (firm + 1) % GROUPS] {
            worker_codes.push(worker as u32);
            firm_codes.push(firm as u32);
            let worker_effect =
                0.35 * (std::f64::consts::TAU * worker as f64 / GROUPS as f64).cos();
            let firm_effect = (std::f64::consts::TAU * firm as f64 / GROUPS as f64).sin();
            target.push(2.0 + worker_effect + firm_effect);
        }
    }
    let sources = [
        CategoricalCodeView::new(&worker_codes, GROUPS),
        CategoricalCodeView::new(&firm_codes, GROUPS),
    ];
    let retained = vec![true; observations];
    let topology =
        finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &retained)
            .expect("weak AKM-cycle topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker effect"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm effect"),
    ];
    let recovered = recover_finalized_categorical_effects_map_certified(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        MapConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        },
    )
    .expect("weak AKM-cycle recovery");
    assert!(recovered.reconstruction_relative_infinity() <= 1e-10);
    for (actual, expected) in recovered.absorbed_sum().iter().zip(target) {
        assert!((actual - expected).abs() <= 1e-9);
    }
}

#[test]
fn weak_cycle_uses_the_hybrid_remainder_solve_and_certifies() {
    const GROUPS: usize = 72;
    let observations = GROUPS * 2;
    let mut worker_codes = Vec::with_capacity(observations);
    let mut firm_codes = Vec::with_capacity(observations);
    let mut target = Vec::with_capacity(observations);
    for firm in 0..GROUPS {
        for worker in [firm, (firm + 1) % GROUPS] {
            worker_codes.push(worker as u32);
            firm_codes.push(firm as u32);
            target.push(
                0.35 * (std::f64::consts::TAU * worker as f64 / GROUPS as f64).cos()
                    + (std::f64::consts::TAU * firm as f64 / GROUPS as f64).sin(),
            );
        }
    }
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker_codes, GROUPS),
            CategoricalCodeView::new(&firm_codes, GROUPS),
        ],
        &[&[0], &[1]],
        &vec![true; observations],
    )
    .expect("weak topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let recovered = shared_recovery_with_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        LsmrConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            ..LsmrConfig::default()
        },
        MapConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        },
    )
    .expect("hybrid weak-cycle recovery");
    assert_eq!(
        recovered.recovery_method(),
        CategoricalRecoveryMethod::Direct
    );
    assert!(recovered.reconstruction_relative_infinity() <= 1e-10);
}

#[test]
fn weak_cycle_shared_graph_requires_reconstruction_certificate() {
    const GROUPS: usize = 72;
    let observations = GROUPS * 2;
    let mut worker_codes = Vec::with_capacity(observations);
    let mut firm_codes = Vec::with_capacity(observations);
    let mut target = Vec::with_capacity(observations);
    for firm in 0..GROUPS {
        for worker in [firm, (firm + 1) % GROUPS] {
            worker_codes.push(worker as u32);
            firm_codes.push(firm as u32);
            target.push(
                0.35 * (std::f64::consts::TAU * worker as f64 / GROUPS as f64).cos()
                    + (std::f64::consts::TAU * firm as f64 / GROUPS as f64).sin(),
            );
        }
    }
    let topology = finalize_categorical_topology(
        &[
            CategoricalCodeView::new(&worker_codes, GROUPS),
            CategoricalCodeView::new(&firm_codes, GROUPS),
        ],
        &[&[0], &[1]],
        &vec![true; observations],
    )
    .expect("weak topology");
    let effects = [
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("worker"),
        CategoricalAbsorbedEffectView::new(true, Vec::new()).expect("firm"),
    ];
    let recovered = shared_recovery_with_map_controls(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        &target,
        &[true, true],
        MapConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        },
    )
    .expect("strict CG recovery");
    assert_eq!(
        recovered.recovery_method(),
        CategoricalRecoveryMethod::Direct
    );
    assert!(recovered.reconstruction_relative_infinity() <= 1e-10);
    for (actual, expected) in recovered.absorbed_sum().iter().zip(target) {
        assert!((actual - expected).abs() <= 1e-9);
    }
}

#[test]
fn every_workspace_reservation_and_capacity_guard_fails_closed() {
    let group = [0, 0, 0, 1, 1, 1];
    let topology =
        finalize_categorical_topology(&[CategoricalCodeView::new(&group, 2)], &[&[0]], &[true; 6])
            .expect("topology");
    let slope = [0.0, 1.0, 2.0, 0.0, 1.5, 3.0];
    let effects = [CategoricalAbsorbedEffectView::new(true, vec![&slope]).expect("slope effect")];
    let values = DenseMatrix::from_columns(&[vec![1.0, 2.0, 4.0, 3.0, 5.0, 8.0]]).expect("values");
    let mut successful = InjectedReservation::default();
    absorb_with_reservation(
        &topology,
        &effects,
        BorrowedMapWeights::Unit,
        values.clone(),
        config(),
        None,
        &mut successful,
    )
    .expect("allocation audit baseline");
    assert!(successful.calls > 0);
    for call in 0..successful.calls {
        let mut injected = InjectedReservation {
            fail_at: Some(call),
            ..InjectedReservation::default()
        };
        let error = absorb_with_reservation(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            values.clone(),
            config(),
            None,
            &mut injected,
        )
        .expect_err("injected allocation failure");
        assert_eq!(error.code, ErrorCode::Allocation, "call {call}");

        let mut undersized = InjectedReservation {
            undersize_at: Some(call),
            ..InjectedReservation::default()
        };
        let error = absorb_with_reservation(
            &topology,
            &effects,
            BorrowedMapWeights::Unit,
            values.clone(),
            config(),
            None,
            &mut undersized,
        )
        .expect_err("undersized allocation");
        assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
    }
}

#[test]
fn recovery_output_capacity_overflow_is_an_allocation_error() {
    let error = fallible_filled(
        usize::MAX,
        0.0_f64,
        "injected categorical recovery output overflow",
    )
    .expect_err("oversized recovery output");
    assert_eq!(error.code, ErrorCode::Allocation);
}
