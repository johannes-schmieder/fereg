use super::*;
use crate::fe::{LevelIndex, RoutingPreset, ValidatedBorrowedInterceptSource};

/// Every occupied cell contains equal-weight opposite pairs. Its exact weighted
/// projection is therefore known without running another iterative solver.
struct PairedGraph {
    codes: [Vec<u32>; 2],
    indexes: [LevelIndex; 2],
    weights: Vec<f64>,
    input: DenseMatrix,
    residual: DenseMatrix,
}

impl PairedGraph {
    const LEVELS: usize = 64;
    const COMPONENT_LEVELS: usize = 32;

    fn new() -> Self {
        let mut codes = [Vec::new(), Vec::new()];
        let mut weights = Vec::new();
        let mut columns = vec![Vec::new(); 3];
        let mut expected = vec![Vec::new(); 3];
        for component in 0..2 {
            for worker in 0..Self::COMPONENT_LEVELS {
                // A local high-degree star exercises sampled construction,
                // while the remaining long ring cannot finish in four steps.
                let degree = if worker == 0 { 8 } else { 2 };
                for neighbor in 0..degree {
                    let first = component * Self::COMPONENT_LEVELS + worker;
                    let second = component * Self::COMPONENT_LEVELS
                        + (worker + neighbor) % Self::COMPONENT_LEVELS;
                    let weight = (1 + (first * 3 + second) % 7) as f64 / 8.;
                    for repeat in 0..32 {
                        for sign in [-1., 1.] {
                            let noise = sign * (1 + (first + second + repeat) % 11) as f64 / 16.;
                            codes[0].push(first as u32);
                            codes[1].push(second as u32);
                            weights.push(weight);
                            for (rhs, (column, target)) in
                                columns.iter_mut().zip(&mut expected).enumerate()
                            {
                                let value = noise * (rhs + 1) as f64;
                                target.push(value);
                                column.push(
                                    value + Self::left(first, rhs) + Self::right(second, rhs),
                                );
                            }
                        }
                    }
                }
            }
        }
        let indexes = [
            LevelIndex::from_codes(&codes[0], Self::LEVELS).unwrap(),
            LevelIndex::from_codes(&codes[1], Self::LEVELS).unwrap(),
        ];
        Self {
            codes,
            indexes,
            weights,
            input: DenseMatrix::from_columns(&columns).unwrap(),
            residual: DenseMatrix::from_columns(&expected).unwrap(),
        }
    }

    fn left(level: usize, rhs: usize) -> f64 {
        let scale = [1., 2., 0.][rhs];
        scale * (((level * 17 + 3) % 37) as f64 - 18.) / 8.
    }

    fn right(level: usize, rhs: usize) -> f64 {
        let scale = [1., -3., 0.][rhs];
        scale * (((level * 13 + 5) % 43) as f64 - 21.) / 16.
    }

    fn design(&self) -> BorrowedInterceptFeDesign<'_> {
        let sources = [
            ValidatedBorrowedInterceptSource::new(&self.codes[0], Self::LEVELS, &self.indexes[0])
                .unwrap(),
            ValidatedBorrowedInterceptSource::new(&self.codes[1], Self::LEVELS, &self.indexes[1])
                .unwrap(),
        ];
        BorrowedInterceptFeDesign::new(
            self.weights.len(),
            sources.iter().map(|source| source.effect()).collect(),
        )
        .unwrap()
    }

    fn columns(&self, positions: &[usize]) -> DenseMatrix {
        DenseMatrix::from_columns(
            &positions
                .iter()
                .map(|&rhs| {
                    (0..self.input.nrows())
                        .map(|row| self.input[(row, rhs)])
                        .collect()
                })
                .collect::<Vec<Vec<f64>>>(),
        )
        .unwrap()
    }

    fn check(&self, result: &CertifiedBorrowedMap, positions: &[usize]) {
        assert!(result.report().certified());
        assert_eq!(result.residualized().nrows(), self.weights.len());
        assert_eq!(result.residualized().ncols(), positions.len());
        let coefficients = result.coefficients.as_ref().unwrap();
        assert_eq!(coefficients.level_counts, [Self::LEVELS, Self::LEVELS]);
        for (column, &rhs) in positions.iter().enumerate() {
            let base = column * coefficients.coefficients_per_rhs;
            for row in 0..self.weights.len() {
                let residual = result.residualized()[(row, column)];
                assert!((residual - self.residual[(row, rhs)]).abs() < 1e-9);
                let fitted = coefficients.values
                    [base + coefficients.effect_offsets[0] + self.codes[0][row] as usize]
                    + coefficients.values
                        [base + coefficients.effect_offsets[1] + self.codes[1][row] as usize];
                assert!((self.input[(row, rhs)] - residual - fitted).abs() < 2e-12);
            }
            // Differences within each component identify the saved FE gauge;
            // their target is the generating additive signal, not another fit.
            for component in 0..2 {
                let anchor = component * Self::COMPONENT_LEVELS;
                for level in anchor..anchor + Self::COMPONENT_LEVELS {
                    for (effect, truth) in [
                        (0, Self::left as fn(usize, usize) -> f64),
                        (1, Self::right as fn(usize, usize) -> f64),
                    ] {
                        let start = base + coefficients.effect_offsets[effect];
                        let contrast = coefficients.values[start + level]
                            - coefficients.values[start + anchor];
                        assert!((contrast - truth(level, rhs) + truth(anchor, rhs)).abs() < 1e-8);
                    }
                }
            }
        }
    }
}

fn config() -> MapConfig {
    MapConfig {
        tolerance: 1e-12,
        max_iterations: 200,
        check_every: 1,
        symmetric: true,
    }
}

fn request(owner: &mut ReusableCategoricalMapAbsorber<'_>, samples: u32) {
    owner.retain_coefficients = true;
    owner.request_auto_routing(RoutingPreset::new(4, samples).unwrap(), None, 0);
}

fn assert_same_bits(actual: &[f64], expected: &[f64]) {
    assert_eq!(
        actual
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn rejected_original_certificate_preserves_overlap_epoch_and_h() {
    const CHILD: &str = "FEREG_ROUTING_CERTIFICATE_BOUNDARY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "fe::borrowed_map::routing_tests::rejected_original_certificate_preserves_overlap_epoch_and_h",
                "--exact", "--test-threads=1",
            ]).status().unwrap();
        assert!(status.success());
        return;
    }
    let witness = PairedGraph::new();
    for boundary in [4, 8] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(2, true).unwrap())
            .with_exclusive_overlap()
            .install(|| {
                TWO_FE_CERTIFICATE_AT.set(boundary);
                TWO_FE_CERTIFICATE_REJECTED_AT.set(0);
                TWO_FE_ROUTING_LAUNCH_AT.set(0);
                TWO_FE_ROUTING_BUILD_AT.set(0);
                let mut owner =
                    ReusableCategoricalMapAbsorber::new(witness.design(), 3, 2).unwrap();
                owner.retain_coefficients = true;
                owner.request_auto_routing(RoutingPreset::overlap(8, 0).unwrap(), None, 0);
                let result = owner
                    .absorb_ols(
                        BorrowedMapWeights::Provided(&witness.weights),
                        witness.input.clone().into(),
                        config(),
                        false,
                    )
                    .unwrap();
                owner.finish_routing_preparation().unwrap();
                witness.check(&result, &[0, 1, 2]);
                assert_eq!(
                    TWO_FE_CERTIFICATE_AT.get(),
                    0,
                    "checkpoint actually executed"
                );
                assert_eq!(
                    TWO_FE_CERTIFICATE_REJECTED_AT.get(),
                    boundary,
                    "original certificate rejected the frozen iterate"
                );
                assert_eq!(
                    TWO_FE_ROUTING_LAUNCH_AT.get(),
                    4,
                    "first grant stays at iteration four"
                );
                assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 8, "full grant stays at h");
                let setup = &owner.auto_route.as_ref().unwrap().setup;
                assert!(setup.used <= setup.limit);
                assert!(result.report().iterations <= config().max_iterations);
            })
            .unwrap();
    }
}

#[test]
fn routing_diagonal_and_cells_live_callers_certify_saved_effects_and_reuse_setup() {
    let witness = PairedGraph::new();
    for arm in ["diagonal", "cells"] {
        for workers in [1, 4] {
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let mut owner =
                        ReusableCategoricalMapAbsorber::new(witness.design(), 3, workers).unwrap();
                    owner.retain_coefficients = true;
                    owner.request_auto_routing(RoutingPreset::ablation(arm).unwrap(), None, 0);
                    let first = owner
                        .absorb_ols(
                            BorrowedMapWeights::Provided(&witness.weights),
                            witness.input.clone().into(),
                            config(),
                            false,
                        )
                        .unwrap();
                    witness.check(&first, &[0, 1, 2]);
                    let graph = owner.two_fe_pcg.as_ref().unwrap();
                    assert!(graph.cmg_schur.is_none());
                    assert_eq!(graph.use_cell_operator, arm == "cells");
                    assert!(graph.ppml_operator_rhs_count > 0);
                    let kernel = std::ptr::from_ref(graph.kernel.as_ref().unwrap());
                    let setup = &owner.auto_route.as_ref().unwrap().setup;
                    assert!(setup.attempted);
                    assert!(setup.used > 0 && setup.used <= setup.limit);
                    let charged = (setup.limit, setup.used);

                    // Exercise an installed caller with inactive RHS lanes and
                    // saved effects, rather than calling the kernel directly.
                    let second = owner
                        .absorb_ols(
                            BorrowedMapWeights::Provided(&witness.weights),
                            witness.columns(&[1]).into(),
                            config(),
                            false,
                        )
                        .unwrap();
                    witness.check(&second, &[1]);
                    let graph = owner.two_fe_pcg.as_ref().unwrap();
                    assert_eq!(std::ptr::from_ref(graph.kernel.as_ref().unwrap()), kernel);
                    assert_eq!(graph.use_cell_operator, arm == "cells");
                    let setup = &owner.auto_route.as_ref().unwrap().setup;
                    assert_eq!((setup.limit, setup.used), charged);
                })
                .unwrap();
        }
    }
}

#[test]
fn routing_kernel_and_direct_live_refusals_preserve_incumbent_bits_and_budget() {
    let witness = PairedGraph::new();
    let mut incumbent = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    incumbent.retain_coefficients = true;
    let reference = incumbent
        .absorb_ols(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.input.clone().into(),
            config(),
            false,
        )
        .unwrap();
    for arm in ["diagonal", "cells", "direct"] {
        for memory_refusal in [false, true] {
            let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
            owner.retain_coefficients = true;
            owner.request_auto_routing(RoutingPreset::ablation(arm).unwrap(), None, 0);
            let route = owner.auto_route.as_mut().unwrap();
            if memory_refusal {
                route.request.budget = Some(FeSolverMemoryBudget {
                    execution_bytes: 0,
                    usable_bytes: 0,
                });
            } else {
                route.setup.begin(config().tolerance, false, Some(0));
            }
            let fit = owner
                .absorb_ols(
                    BorrowedMapWeights::Provided(&witness.weights),
                    witness.input.clone().into(),
                    config(),
                    false,
                )
                .unwrap();
            let setup = &owner.auto_route.as_ref().unwrap().setup;
            assert!(setup.attempted);
            assert!(setup.used <= setup.limit);
            let graph = owner.two_fe_pcg.as_ref().unwrap();
            assert!(graph.kernel.is_none());
            assert!(graph.cmg_schur.is_none());
            assert_same_bits(
                fit.residualized().as_slice(),
                reference.residualized().as_slice(),
            );
            assert_eq!(fit.coefficients, reference.coefficients);
            assert_same_bits(
                &fit.coefficients.as_ref().unwrap().values,
                &reference.coefficients.as_ref().unwrap().values,
            );
            assert_eq!(fit.report().certificate, reference.report().certificate);
            assert_eq!(fit.report().iterations, reference.report().iterations);
            assert_eq!(
                graph.ppml_operator_rhs_count,
                incumbent
                    .two_fe_pcg
                    .as_ref()
                    .unwrap()
                    .ppml_operator_rhs_count,
            );
        }
    }
}

#[test]
fn routing_direct_live_caller_certifies_weighted_disconnected_coefficients() {
    const COMPONENTS: usize = 16;
    let levels = 2 * COMPONENTS;
    let mut codes = [Vec::new(), Vec::new()];
    let mut weights = Vec::new();
    let mut input = Vec::new();
    let mut residual = Vec::new();
    for component in 0..COMPONENTS {
        for cell in 0..4 {
            let left = 2 * component + cell / 2;
            let right = 2 * component + cell % 2;
            let weight = [1., 2., 4., 8.][cell] * [1., 4.][component % 2];
            // A weighted rectangle contrast is orthogonal to both FE sets.
            let noise = [1., -1., -1., 1.][cell] / weight;
            codes[0].push(left as u32);
            codes[1].push(right as u32);
            weights.push(weight);
            residual.push(noise);
            input.push(noise + PairedGraph::left(left, 0) + PairedGraph::right(right, 0));
        }
    }
    let indexes = [
        LevelIndex::from_codes(&codes[0], levels).unwrap(),
        LevelIndex::from_codes(&codes[1], levels).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&codes[0], levels, &indexes[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[1], levels, &indexes[1]).unwrap(),
    ];
    let design = BorrowedInterceptFeDesign::new(
        weights.len(),
        sources.iter().map(|source| source.effect()).collect(),
    )
    .unwrap();
    // A one-RHS final action can retain an earlier 64-RHS workspace. The actual
    // incumbent's bank clearing cost makes these tiny components admissible;
    // neither the live setup quota nor its W0 is overridden by the test.
    let mut owner = ReusableCategoricalMapAbsorber::new(design, 64, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(RoutingPreset::ablation("direct").unwrap(), None, 0);
    let fit = owner
        .absorb_ols(
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&[input.clone()]).unwrap().into(),
            config(),
            true,
        )
        .unwrap();
    assert!(fit.report().certified());
    assert_eq!(fit.report().iterations, 0);
    let graph = owner.two_fe_pcg.as_ref().unwrap();
    assert_eq!(graph.ppml_operator_rhs_count, 0);
    assert!(graph.kernel.is_none() && graph.cmg_schur.is_none());
    let setup = &owner.auto_route.as_ref().unwrap().setup;
    assert!(setup.attempted);
    assert!(setup.used > 0 && setup.used <= setup.limit / 8);
    let coefficients = fit.coefficients.as_ref().unwrap();
    assert_eq!(coefficients.level_counts, [levels, levels]);
    for row in 0..weights.len() {
        let actual = fit.residualized()[(row, 0)];
        assert!((actual - residual[row]).abs() < 1e-12);
        let fitted = coefficients.values[coefficients.effect_offsets[0] + codes[0][row] as usize]
            + coefficients.values[coefficients.effect_offsets[1] + codes[1][row] as usize];
        assert!((input[row] - actual - fitted).abs() < 2e-12);
    }
    for component in 0..COMPONENTS {
        let anchor = 2 * component;
        for (effect, truth) in [
            (0, PairedGraph::left as fn(usize, usize) -> f64),
            (1, PairedGraph::right as fn(usize, usize) -> f64),
        ] {
            let offset = coefficients.effect_offsets[effect] + anchor;
            let actual = coefficients.values[offset + 1] - coefficients.values[offset];
            assert!((actual - truth(anchor + 1, 0) + truth(anchor, 0)).abs() < 1e-12);
        }
    }
}

#[test]
fn routing_restart_has_independent_weighted_disconnected_science_and_repeats() {
    let witness = PairedGraph::new();
    for samples in 0..=2 {
        let mut across_workers = None;
        for workers in [1, 4] {
            let mut repeated = None;
            for _ in 0..2 {
                let snapshot = crate::ThreadTeam::new(
                    crate::ThreadConfig::from_requested(workers, true).unwrap(),
                )
                .install(|| {
                    let mut owner =
                        ReusableCategoricalMapAbsorber::new(witness.design(), 3, workers).unwrap();
                    request(&mut owner, samples);
                    let fit = owner
                        .absorb_ols(
                            BorrowedMapWeights::Provided(&witness.weights),
                            witness.input.clone().into(),
                            config(),
                            false,
                        )
                        .unwrap();
                    witness.check(&fit, &[0, 1, 2]);
                    let mut original = BorrowedMapWorkspace::new(
                        &owner.design,
                        witness.input.nrows(),
                        3,
                        &mut SystemWorkspaceReservation,
                    )
                    .unwrap();
                    update_workspace_norms(
                        &witness.input,
                        validate_weights(
                            BorrowedMapWeights::Provided(&witness.weights),
                            witness.weights.len(),
                        )
                        .unwrap(),
                        &mut original,
                        true,
                    )
                    .unwrap();
                    assert_eq!(owner.workspace.reference_norms, original.reference_norms);
                    let route = owner.auto_route.as_ref().unwrap();
                    assert!(route.setup.attempted);
                    assert!(route.setup.used <= route.setup.limit);
                    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_some());
                    assert!(fit.report().iterations > 4);
                    assert!(fit.report().iterations <= config().max_iterations);
                    (
                        fit.residualized().clone(),
                        fit.coefficients.unwrap(),
                        fit.report.iterations,
                        route.setup.used,
                    )
                })
                .unwrap();
                if let Some(previous) = repeated.as_ref() {
                    assert_eq!(&snapshot, previous);
                    assert_same_bits(snapshot.0.as_slice(), previous.0.as_slice());
                    assert_same_bits(&snapshot.1.values, &previous.1.values);
                }
                repeated = Some(snapshot);
            }
            let snapshot = repeated.unwrap();
            if let Some((residuals, coefficients, _, _)) = across_workers.as_ref() {
                let residuals: &DenseMatrix = residuals;
                let coefficients: &FeCoefficientBatch = coefficients;
                assert_eq!(coefficients.level_counts, snapshot.1.level_counts);
                for (&a, &b) in residuals.as_slice().iter().zip(snapshot.0.as_slice()) {
                    assert!((a - b).abs() < 1e-9);
                }
            }
            across_workers = Some(snapshot);
        }
    }
}

#[test]
fn routing_quota_or_memory_refusal_preserves_the_exact_incumbent() {
    let witness = PairedGraph::new();
    let mut incumbent = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    incumbent.retain_coefficients = true;
    let reference = incumbent
        .absorb_ols(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.input.clone().into(),
            config(),
            false,
        )
        .unwrap();
    for memory_refusal in [false, true] {
        let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
        request(&mut owner, 1);
        let route = owner.auto_route.as_mut().unwrap();
        if memory_refusal {
            route.request.budget = Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 0,
            });
        } else {
            // Bind the real session before restricting its optional setup quota.
            // No recurrence state is injected or changed.
            route.setup.begin(config().tolerance, false, Some(0));
        }
        let fit = owner
            .absorb_ols(
                BorrowedMapWeights::Provided(&witness.weights),
                witness.input.clone().into(),
                config(),
                false,
            )
            .unwrap();
        assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
        assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
        assert_eq!(fit.residualized(), reference.residualized());
        assert_same_bits(
            fit.residualized().as_slice(),
            reference.residualized().as_slice(),
        );
        assert_eq!(fit.coefficients, reference.coefficients);
        assert_same_bits(
            &fit.coefficients.as_ref().unwrap().values,
            &reference.coefficients.as_ref().unwrap().values,
        );
        assert_eq!(fit.report().iterations, reference.report().iterations);
        assert_eq!(fit.report().certificate, reference.report().certificate);
        assert_eq!(
            owner.two_fe_pcg.as_ref().unwrap().ppml_operator_rhs_count,
            incumbent
                .two_fe_pcg
                .as_ref()
                .unwrap()
                .ppml_operator_rhs_count,
        );
    }
}

#[test]
fn routing_native_caps_around_checkpoint_never_extend_the_action() {
    let witness = PairedGraph::new();
    for checkpoint in [4, 8, 16, 32] {
        for cap in [checkpoint - 1, checkpoint, checkpoint + 1] {
            let limited = MapConfig {
                max_iterations: cap,
                ..config()
            };
            let mut incumbent =
                ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
            let reference = incumbent.absorb_ols(
                BorrowedMapWeights::Provided(&witness.weights),
                witness.input.clone().into(),
                limited,
                false,
            );
            let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
            owner.request_auto_routing(RoutingPreset::new(checkpoint, 1).unwrap(), None, 0);
            let routed = owner.absorb_ols(
                BorrowedMapWeights::Provided(&witness.weights),
                witness.input.clone().into(),
                limited,
                false,
            );
            assert!(!owner.auto_route.as_ref().unwrap().setup.attempted);
            assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
            let operators = owner.two_fe_pcg.as_ref().unwrap().ppml_operator_rhs_count;
            assert!(operators <= cap as u128 * 3);
            assert_eq!(
                operators,
                incumbent
                    .two_fe_pcg
                    .as_ref()
                    .unwrap()
                    .ppml_operator_rhs_count
            );
            match (reference, routed) {
                (Ok(reference), Ok(routed)) => {
                    assert_eq!(routed.residualized(), reference.residualized());
                    assert_same_bits(
                        routed.residualized().as_slice(),
                        reference.residualized().as_slice(),
                    );
                    assert_eq!(routed.report().iterations, reference.report().iterations);
                    assert!(routed.report().iterations <= cap);
                }
                (Err(reference), Err(routed)) => assert_eq!(routed.code, reference.code),
                _ => panic!("routing changed the success decision before its checkpoint"),
            }
        }
    }
}

#[test]
fn routing_initial_certificate_does_not_spend_or_prepare() {
    let witness = PairedGraph::new();
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    request(&mut owner, 1);
    let fit = owner
        .absorb_ols(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.residual.clone().into(),
            config(),
            false,
        )
        .unwrap();
    assert!(fit.report().certified());
    assert_eq!(fit.report().iterations, 0);
    assert_eq!(fit.residualized(), &witness.residual);
    let route = owner.auto_route.as_ref().unwrap();
    assert!(!route.setup.attempted);
    assert_eq!(route.setup.used, 0);
    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
}

#[test]
fn routing_installed_restart_keeps_the_last_native_iteration_limit() {
    let witness = PairedGraph::new();
    // The incumbent reserves one of these six native iterations for MAP. A
    // hierarchy installed after iteration four gets exactly one PCG iteration,
    // not a refreshed five-iteration budget.
    let limited = MapConfig {
        max_iterations: 6,
        ..config()
    };
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    request(&mut owner, 1);
    let result = owner.absorb_ols(
        BorrowedMapWeights::Provided(&witness.weights),
        witness.input.clone().into(),
        limited,
        false,
    );
    assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
    let graph = owner.two_fe_pcg.as_ref().unwrap();
    assert!(graph.cmg_schur.is_some());
    assert_eq!(graph.ppml_operator_rhs_count, 5 * 3);
    match result {
        Ok(fit) => {
            witness.check(&fit, &[0, 1, 2]);
            assert!(fit.report().iterations <= limited.max_iterations);
        }
        Err(error) => assert_eq!(error.code, ErrorCode::InvalidData),
    }
}

#[test]
fn routing_low_rhs_chunks_share_preparation_but_final_accuracy_has_a_new_ledger() {
    let witness = PairedGraph::new();
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    request(&mut owner, 1);
    let first = owner
        .absorb_ols(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.columns(&[0, 1, 2]).into(),
            config(),
            false,
        )
        .unwrap();
    witness.check(&first, &[0, 1, 2]);
    let hierarchy = std::ptr::from_ref(
        owner
            .two_fe_pcg
            .as_ref()
            .unwrap()
            .cmg_schur
            .as_ref()
            .unwrap()
            .as_ref(),
    );
    let ledger = owner.auto_route.as_ref().unwrap();
    assert!(ledger.setup.attempted);
    let work = (ledger.setup.limit, ledger.setup.used);
    let references = owner.workspace.reference_norms.clone();
    assert!(references.iter().all(|value| *value > 0.));
    for positions in [&[1][..], &[0, 1][..]] {
        let chunk = owner
            .absorb_ols(
                BorrowedMapWeights::Provided(&witness.weights),
                witness.columns(positions).into(),
                config(),
                false,
            )
            .unwrap();
        witness.check(&chunk, positions);
        let ledger = owner.auto_route.as_ref().unwrap();
        assert_eq!((ledger.setup.limit, ledger.setup.used), work);
        assert_eq!(
            std::ptr::from_ref(
                owner
                    .two_fe_pcg
                    .as_ref()
                    .unwrap()
                    .cmg_schur
                    .as_ref()
                    .unwrap()
                    .as_ref()
            ),
            hierarchy,
        );
        assert!(chunk.report().iterations <= config().max_iterations);
    }
    let final_config = MapConfig {
        tolerance: 1e-14,
        ..config()
    };
    let final_fit = owner
        .absorb_ols(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.columns(&[0]).into(),
            final_config,
            true,
        )
        .unwrap();
    witness.check(&final_fit, &[0]);
    let ledger = owner.auto_route.as_ref().unwrap();
    assert!(!ledger.setup.attempted);
    assert_eq!(ledger.setup.used, 0);
    assert!(ledger.setup.limit > 0);
    assert_eq!(
        std::ptr::from_ref(
            owner
                .two_fe_pcg
                .as_ref()
                .unwrap()
                .cmg_schur
                .as_ref()
                .unwrap()
                .as_ref()
        ),
        hierarchy,
    );
}
