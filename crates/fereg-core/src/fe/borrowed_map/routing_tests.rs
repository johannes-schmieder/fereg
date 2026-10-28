use super::super::{cmg_schur::CmgSchurPreconditioner, routing::schur_iteration_work};
use super::*;
use crate::fe::{LevelIndex, RoutingPreset, ValidatedBorrowedInterceptSource};

#[path = "remaining_tests.rs"]
pub(super) mod remaining_tests;

/// Cached Rayon workers can execute another test's installed action while
/// joining work. Isolate worker-local observations, keeping complete witnesses
/// and child panic output; the deadline bounds a broken join, not solver work.
fn isolated_routing_witness(marker: &str, test: &str) -> bool {
    if std::env::var_os(marker).is_some() {
        return false;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .env(marker, "1")
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .unwrap_or_else(|error| panic!("cannot start isolated witness {test}: {error}"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(status.success(), "isolated witness {test} failed: {status}");
                return true;
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => {
                let killed = child.kill();
                let reaped = child.wait();
                panic!(
                    "isolated witness {test} timed out or failed to wait: \
                     poll={result:?}, kill={killed:?}, reap={reaped:?}"
                );
            }
        }
    }
}

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
        self.check_signals(result, positions, [Self::left, Self::right]);
    }

    fn check_signals(
        &self,
        result: &CertifiedBorrowedMap,
        positions: &[usize],
        signals: [fn(usize, usize) -> f64; 2],
    ) {
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
                    for (effect, truth) in signals.iter().enumerate() {
                        let start = base + coefficients.effect_offsets[effect];
                        let contrast = coefficients.values[start + level]
                            - coefficients.values[start + anchor];
                        assert!((contrast - truth(level, rhs) + truth(anchor, rhs)).abs() < 1e-8);
                    }
                }
            }
        }
    }

    fn independent_left(level: usize, rhs: usize) -> f64 {
        Self::left(level, 0)
            + if rhs == 0 {
                0.
            } else {
                (((level * (2 * rhs + 1) + 3 * rhs) % 17) as f64 - 8.) / 64.
            }
    }

    fn independent_right(level: usize, rhs: usize) -> f64 {
        Self::right(level, 0)
            + if rhs == 0 {
                0.
            } else {
                (((level * (2 * rhs + 3) + 5 * rhs) % 19) as f64 - 9.) / 128.
            }
    }

    fn independent_columns() -> Self {
        let mut witness = Self::new();
        let walsh = |rhs: usize, repeat: usize| {
            if (rhs & repeat).count_ones() % 2 == 0 {
                1.
            } else {
                -1.
            }
        };
        let mut columns = vec![Vec::new(); 16];
        let mut residuals = vec![Vec::new(); 16];
        for rhs in 0..16 {
            for row in 0..witness.input.nrows() {
                // Every cell keeps its original 32 opposite pairs and weight.
                // Walsh row zero is constant, so the first RHS is unchanged.
                let repeat = (row / 2) % 32;
                let residual = witness.residual[(row, 0)] * walsh(rhs, repeat);
                let input = residual
                    + Self::independent_left(witness.codes[0][row] as usize, rhs)
                    + Self::independent_right(witness.codes[1][row] as usize, rhs);
                if rhs == 0 {
                    assert_eq!(residual.to_bits(), witness.residual[(row, 0)].to_bits());
                    assert_eq!(input.to_bits(), witness.input[(row, 0)].to_bits());
                }
                residuals[rhs].push(residual);
                columns[rhs].push(input);
            }
            // The 16 Walsh vectors are independent over 32 repeats. Multiplying
            // each row by the original strictly positive dyadic magnitude is
            // invertible, so these residual columns remain independent.
            for other in 0..16 {
                let product: f64 = (0..32).map(|t| walsh(rhs, t) * walsh(other, t)).sum();
                assert_eq!(product, if rhs == other { 32. } else { 0. });
            }
            for pair in residuals[rhs].chunks_exact(2) {
                assert_eq!(pair[0] + pair[1], 0.);
            }
            // Each submitted column initially has nonzero FE moments. This
            // does not claim that every lane remains active at installation.
            for effect in 0..2 {
                let mut moments = [0.; Self::LEVELS];
                for (row, &value) in columns[rhs].iter().enumerate() {
                    moments[witness.codes[effect][row] as usize] += witness.weights[row] * value;
                }
                assert!(moments.iter().any(|&value| value != 0.));
            }
        }
        witness.input = DenseMatrix::from_columns(&columns).unwrap();
        witness.residual = DenseMatrix::from_columns(&residuals).unwrap();
        witness
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

/// A tall unit-weight rectangle admits the O(1) direct bound. Opposite row
/// pairs give its exact projection without another numerical solver.
struct PortfolioRectangle {
    codes: [Vec<u32>; 2],
    indices: [LevelIndex; 2],
    values: DenseMatrix,
    residuals: DenseMatrix,
}

impl PortfolioRectangle {
    fn new() -> Self {
        let mut codes = [Vec::new(), Vec::new()];
        let mut values = [Vec::new(), Vec::new()];
        let mut residuals = [Vec::new(), Vec::new()];
        for left in 0..4 {
            for right in 0..2 {
                for repeat in 0..64 {
                    for sign in [-1., 1.] {
                        codes[0].push(left as u32);
                        codes[1].push(right as u32);
                        for rhs in 0..2 {
                            let noise = sign * (1 + repeat % 7 + rhs) as f64 / 16.;
                            residuals[rhs].push(noise);
                            values[rhs].push(
                                noise
                                    + PairedGraph::left(left, rhs)
                                    + PairedGraph::right(right, rhs),
                            );
                        }
                    }
                }
            }
        }
        let indices = [
            LevelIndex::from_codes(&codes[0], 4).unwrap(),
            LevelIndex::from_codes(&codes[1], 2).unwrap(),
        ];
        Self {
            codes,
            indices,
            values: DenseMatrix::from_columns(&values).unwrap(),
            residuals: DenseMatrix::from_columns(&residuals).unwrap(),
        }
    }

    fn design(&self) -> BorrowedInterceptFeDesign<'_> {
        let sources = [
            ValidatedBorrowedInterceptSource::new(&self.codes[0], 4, &self.indices[0]).unwrap(),
            ValidatedBorrowedInterceptSource::new(&self.codes[1], 2, &self.indices[1]).unwrap(),
        ];
        BorrowedInterceptFeDesign::new(
            self.values.nrows(),
            sources.iter().map(|source| source.effect()).collect(),
        )
        .unwrap()
    }

    fn first(&self) -> DenseMatrix {
        DenseMatrix::from_columns(&[(0..self.values.nrows())
            .map(|row| self.values[(row, 0)])
            .collect()])
        .unwrap()
    }

    fn check(&self, fit: &CertifiedBorrowedMap) {
        assert!(fit.report().certified());
        let coefficients = fit.coefficients.as_ref().unwrap();
        for rhs in 0..fit.residualized().ncols() {
            let base = rhs * coefficients.coefficients_per_rhs;
            for row in 0..self.values.nrows() {
                assert!(
                    (fit.residualized()[(row, rhs)] - self.residuals[(row, rhs)]).abs() < 1e-12
                );
                let fitted = coefficients.values[base + self.codes[0][row] as usize]
                    + coefficients.values
                        [base + coefficients.effect_offsets[1] + self.codes[1][row] as usize];
                assert!(
                    (self.values[(row, rhs)] - fit.residualized()[(row, rhs)] - fitted).abs()
                        < 1e-12
                );
            }
        }
    }
}

#[test]
fn portfolio_unseeded_skips_the_geometry_pilot_and_keeps_preparation_across_rhs_widths() {
    let mut codes = [Vec::new(), Vec::new(), Vec::new()];
    let mut columns = [Vec::new(), Vec::new()];
    let mut expected = [Vec::new(), Vec::new()];
    for first in 0..2 {
        for second in 0..2 {
            for third in 0..2 {
                for sign in [-1., 1.] {
                    codes[0].push(first);
                    codes[1].push(second);
                    codes[2].push(third);
                    for rhs in 0..2 {
                        let noise = sign * (rhs + 1) as f64 / 8.;
                        expected[rhs].push(noise);
                        columns[rhs].push(
                            noise + first as f64 + second as f64 / 2. - third as f64 / 4.
                                + rhs as f64,
                        );
                    }
                }
            }
        }
    }
    let indices: Vec<_> = codes
        .iter()
        .map(|code| LevelIndex::from_codes(code, 2).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(&indices)
        .map(|(code, index)| ValidatedBorrowedInterceptSource::new(code, 2, index).unwrap())
        .collect();
    let design = BorrowedInterceptFeDesign::new(
        columns[0].len(),
        sources.iter().map(|s| s.effect()).collect(),
    )
    .unwrap();
    let mut route = routing_lsmr::GeneralRoutingState::new(RoutingPreset::portfolio_unseeded());
    for rhs in [1, 2] {
        let original = DenseMatrix::from_columns(&columns[..rhs]).unwrap();
        let lsmr = LsmrConfig {
            max_iterations: 8,
            preconditioner: LsmrPreconditioner::Off,
            ..LsmrConfig::default()
        };
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            rhs,
            &[2, 2, 2],
            rayon::current_num_threads(),
            lsmr.local_size,
        )
        .unwrap();
        let result = project_general_categorical(
            &design,
            BorrowedMapWeights::Unit,
            original,
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 16,
                ..config()
            },
            lsmr,
            None,
            true,
            estimate,
            false,
            Some(4),
            Some(&mut route),
            false,
        )
        .unwrap();
        assert_eq!(result.report().used_solver, FeSolverKind::Lsmr);
        assert_eq!(result.report().pilot_iterations, 0);
        assert!(!result.report().fallback_used);
        assert!(result.report().iterations <= 8);
        assert!(result.report().certificate.relative_infinity_norm <= 1e-14);
        assert!(route.has_prepared());
        assert!(!route.has_replacement());
        for (column, expected) in expected.iter().enumerate().take(rhs) {
            for (row, &expected) in expected.iter().enumerate() {
                assert!((result.residualized()[(row, column)] - expected).abs() < 1e-12);
            }
        }
        let coefficients = result.coefficients.as_ref().unwrap();
        assert_eq!(coefficients.level_counts, [2, 2, 2]);
        for (column, original) in columns.iter().enumerate().take(rhs) {
            for (row, &original) in original.iter().enumerate() {
                let base = column * coefficients.coefficients_per_rhs;
                let fitted: f64 = codes
                    .iter()
                    .enumerate()
                    .map(|(effect, code)| {
                        coefficients.values
                            [base + coefficients.effect_offsets[effect] + code[row] as usize]
                    })
                    .sum();
                assert!((original - result.residualized()[(row, column)] - fitted).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn geometry_many_runs_the_certified_pilot_without_preparing_lsmr() {
    let mut codes = [Vec::new(), Vec::new(), Vec::new()];
    let mut columns = [Vec::new(), Vec::new()];
    let mut expected = [Vec::new(), Vec::new()];
    for first in 0..2 {
        for second in 0..2 {
            for third in 0..2 {
                for sign in [-1., 1.] {
                    codes[0].push(first);
                    codes[1].push(second);
                    codes[2].push(third);
                    for rhs in 0..2 {
                        let noise = sign * (rhs + 1) as f64 / 8.;
                        expected[rhs].push(noise);
                        columns[rhs].push(
                            noise + first as f64 + second as f64 / 2. - third as f64 / 4.
                                + rhs as f64,
                        );
                    }
                }
            }
        }
    }
    let indices: Vec<_> = codes
        .iter()
        .map(|code| LevelIndex::from_codes(code, 2).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(&indices)
        .map(|(code, index)| ValidatedBorrowedInterceptSource::new(code, 2, index).unwrap())
        .collect();
    let design = BorrowedInterceptFeDesign::new(
        columns[0].len(),
        sources.iter().map(|s| s.effect()).collect(),
    )
    .unwrap();
    let mut route = routing_lsmr::GeneralRoutingState::new(RoutingPreset::geometry_many());
    for rhs in [1, 2] {
        let original = DenseMatrix::from_columns(&columns[..rhs]).unwrap();
        let lsmr = LsmrConfig {
            max_iterations: 8,
            preconditioner: LsmrPreconditioner::Off,
            ..LsmrConfig::default()
        };
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            rhs,
            &[2, 2, 2],
            rayon::current_num_threads(),
            lsmr.local_size,
        )
        .unwrap();
        let result = project_general_categorical(
            &design,
            BorrowedMapWeights::Unit,
            original,
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 16,
                ..config()
            },
            lsmr,
            None,
            true,
            estimate,
            false,
            Some(4),
            Some(&mut route),
            false,
        )
        .unwrap();
        // The orthogonal design certifies in the pilot at the requested
        // tolerance; no LSMR solver is built or retained.
        assert_eq!(result.report().used_solver, FeSolverKind::Map);
        assert_eq!(result.report().requested_solver, FeSolverKind::Auto);
        assert!(result.report().iterations <= 1);
        assert!(result.report().certified());
        assert!(result.report().certificate.relative_infinity_norm <= 1e-12);
        assert!(!route.has_prepared());
        assert!(!route.has_replacement());
        for (column, expected) in expected.iter().enumerate().take(rhs) {
            for (row, &expected) in expected.iter().enumerate() {
                assert!((result.residualized()[(row, column)] - expected).abs() < 1e-12);
            }
        }
        let coefficients = result.coefficients.as_ref().unwrap();
        assert_eq!(coefficients.level_counts, [2, 2, 2]);
        for (column, original) in columns.iter().enumerate().take(rhs) {
            for (row, &original) in original.iter().enumerate() {
                let base = column * coefficients.coefficients_per_rhs;
                let fitted: f64 = codes
                    .iter()
                    .enumerate()
                    .map(|(effect, code)| {
                        coefficients.values
                            [base + coefficients.effect_offsets[effect] + code[row] as usize]
                    })
                    .sum();
                assert!((original - result.residualized()[(row, column)] - fitted).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn portfolio_direct_numerical_refusal_keeps_incumbent_bits_without_a_cmg_retry() {
    let codes = [
        vec![0, 0, 0, 1, 1, 1, 2, 2, 2],
        vec![0, 1, 2, 0, 1, 2, 0, 1, 2],
    ];
    let indices = [
        LevelIndex::from_codes(&codes[0], 3).unwrap(),
        LevelIndex::from_codes(&codes[1], 3).unwrap(),
    ];
    let sources = [
        ValidatedBorrowedInterceptSource::new(&codes[0], 3, &indices[0]).unwrap(),
        ValidatedBorrowedInterceptSource::new(&codes[1], 3, &indices[1]).unwrap(),
    ];
    let make_design =
        || BorrowedInterceptFeDesign::new(9, sources.iter().map(|s| s.effect()).collect()).unwrap();
    let design = make_design();
    let weights = [1., 1e-200, 1e-200, 1., 1., 1., 1., 1., 1.];
    let columns: Vec<Vec<f64>> = (0..65)
        .map(|rhs| {
            (0..9)
                .map(|row| {
                    (rhs + 1) as f64 * (codes[0][row] as f64 + codes[1][row] as f64 / 2.)
                        + [0.25, -0.5, 0.75][row % 3]
                })
                .collect()
        })
        .collect();
    let values = DenseMatrix::from_columns(&columns).unwrap();
    let mut incumbent = ReusableCategoricalMapAbsorber::new(make_design(), 65, 1).unwrap();
    incumbent.retain_coefficients = true;
    let expected = incumbent
        .absorb_ols(
            BorrowedMapWeights::Provided(&weights),
            values.clone().into(),
            config(),
            false,
        )
        .unwrap();
    let mut owner = ReusableCategoricalMapAbsorber::new(make_design(), 65, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(RoutingPreset::portfolio_v1(), None, 0);
    let actual = owner
        .absorb_ols(
            BorrowedMapWeights::Provided(&weights),
            values.clone().into(),
            config(),
            false,
        )
        .unwrap();
    let route = owner.auto_route.as_ref().unwrap();
    assert_eq!(route.setup.portfolio_choice(), PortfolioChoice::Direct);
    assert!(route.setup.attempted);
    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
    assert_eq!(actual.residualized(), expected.residualized());
    assert_eq!(actual.coefficients, expected.coefficients);
    assert_eq!(actual.report().iterations, expected.report().iterations);
    assert_eq!(actual.report().certificate, expected.report().certificate);
    // The positive input weights pass caller validation; the eliminated pair
    // formed from two 1e-200 cells underflows inside the direct factor setup.
    let graph = owner.two_fe_pcg.as_ref().unwrap();
    let attempt = super::super::routing_direct::attempt_with_census(
        &design,
        BorrowedMapWeights::Provided(&weights),
        &values,
        super::super::routing_direct::DirectLimits {
            reference_iteration_work: schur_iteration_work(
                9,
                graph.left_weights.len(),
                graph.right_weights.len(),
                65,
                65,
            )
            .unwrap(),
            preflight_work: super::super::routing_direct::PORTFOLIO_PREFLIGHT_WORK,
            component_census_work: 0,
            certificate_work: super::super::routing_direct::original_certificate_work(9, 6, 2, 65)
                .unwrap(),
            certificate_bytes: 0,
            retained_live_bytes: 0,
            memory: None,
        },
    )
    .unwrap();
    assert_eq!(
        attempt.decline,
        Some(super::super::routing_direct::DirectDecline::Numerical)
    );
}

#[test]
fn portfolio_direct_selection_certifies_then_keeps_one_choice_across_low_chunks() {
    let fixture = PortfolioRectangle::new();
    for (capacity, preset) in [
        (2, RoutingPreset::portfolio_v1()),
        (64, RoutingPreset::portfolio_v1()),
        (2, RoutingPreset::portfolio_unseeded()),
        (64, RoutingPreset::portfolio_unseeded()),
    ] {
        let mut owner = ReusableCategoricalMapAbsorber::new(fixture.design(), capacity, 1).unwrap();
        owner.retain_coefficients = true;
        owner.request_auto_routing(preset, None, 0);
        let first = owner
            .absorb_ols(
                BorrowedMapWeights::Unit,
                fixture.first().into(),
                config(),
                false,
            )
            .unwrap();
        fixture.check(&first);
        assert_eq!(first.report().iterations, 0);
        let route = owner.auto_route.as_ref().unwrap();
        assert_eq!(route.setup.portfolio_choice(), PortfolioChoice::Direct);
        assert!(route.setup.attempted);
        let later = owner
            .absorb_ols(
                BorrowedMapWeights::Unit,
                fixture.values.clone().into(),
                config(),
                false,
            )
            .unwrap();
        fixture.check(&later);
        let route = owner.auto_route.as_ref().unwrap();
        assert_eq!(route.setup.portfolio_choice(), PortfolioChoice::Direct);
        assert!(route.setup.attempted);
        assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
        let final_fit = owner
            .absorb_ols(
                BorrowedMapWeights::Unit,
                fixture.first().into(),
                MapConfig {
                    tolerance: 1e-14,
                    ..config()
                },
                true,
            )
            .unwrap();
        fixture.check(&final_fit);
        assert_eq!(final_fit.report().iterations, 0);
        assert_eq!(
            owner.auto_route.as_ref().unwrap().setup.portfolio_choice(),
            PortfolioChoice::Direct
        );
    }
}

#[test]
fn portfolio_direct_certificate_refusal_preserves_incumbent_and_cannot_enable_cmg() {
    let fixture = PortfolioRectangle::new();
    let mut incumbent = ReusableCategoricalMapAbsorber::new(fixture.design(), 2, 1).unwrap();
    incumbent.retain_coefficients = true;
    let expected = incumbent
        .absorb_ols(
            BorrowedMapWeights::Unit,
            fixture.values.clone().into(),
            config(),
            false,
        )
        .unwrap();
    let mut owner = ReusableCategoricalMapAbsorber::new(fixture.design(), 2, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(RoutingPreset::portfolio_v1(), None, 0);
    TWO_FE_DIRECT_CERTIFICATE_FAILURE.set(true);
    let actual = owner
        .absorb_ols(
            BorrowedMapWeights::Unit,
            fixture.values.clone().into(),
            config(),
            false,
        )
        .unwrap();
    assert!(!TWO_FE_DIRECT_CERTIFICATE_FAILURE.get());
    fixture.check(&actual);
    assert_eq!(actual.residualized(), expected.residualized());
    assert_eq!(actual.coefficients, expected.coefficients);
    assert_eq!(actual.report().iterations, expected.report().iterations);
    assert_eq!(actual.report().certificate, expected.report().certificate);
    let route = owner.auto_route.as_ref().unwrap();
    assert_eq!(route.setup.portfolio_choice(), PortfolioChoice::Direct);
    assert!(route.setup.attempted);
    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
}

#[test]
fn portfolio_direct_break_after_selection_propagates_without_publication() {
    const CHILD: &str = "FEREG_PORTFOLIO_DIRECT_BREAK_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args(["fe::borrowed_map::routing_tests::portfolio_direct_break_after_selection_propagates_without_publication", "--exact", "--test-threads=1"])
            .status().unwrap();
        assert!(status.success());
        return;
    }
    let fixture = PortfolioRectangle::new();
    let mut owner = ReusableCategoricalMapAbsorber::new(fixture.design(), 2, 1).unwrap();
    owner.request_auto_routing(RoutingPreset::portfolio_v1(), None, 0);
    TWO_FE_DIRECT_BREAK_AFTER_SELECTION.set(true);
    let result = owner.absorb_ols(
        BorrowedMapWeights::Unit,
        fixture.values.clone().into(),
        config(),
        false,
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
    let route = owner.auto_route.as_ref().unwrap();
    assert_eq!(route.setup.portfolio_choice(), PortfolioChoice::Direct);
    assert!(route.setup.attempted);
    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
    crate::interrupt::reset_cancellation();
}

#[test]
fn finite_and_portfolio_checkpoints_install_the_same_bounded_hierarchy() {
    let fixture = PortfolioRectangle::new();
    let mut results = Vec::new();
    for preset in [
        RoutingPreset::new(16, 1).unwrap(),
        RoutingPreset::portfolio_v1(),
        RoutingPreset::portfolio_unseeded(),
    ] {
        let design = fixture.design();
        let mut owner = ReusableCategoricalMapAbsorber::new(fixture.design(), 2, 1).unwrap();
        owner.request_auto_routing(preset, None, 0);
        let route = owner.auto_route.as_mut().unwrap();
        route.begin(config().tolerance, false);
        let reference = CmgSchurPreconditioner::try_build_bounded(
            &design,
            BorrowedMapWeights::Unit,
            0,
            route.request.degree,
            route.request.sampling,
            2,
            None,
            false,
            crate::fe::cmg_schur::AutoApplication::Scalar,
            ::cmg::CmgOptions::default(),
        )
        .unwrap();
        assert_eq!(reference.refusal, None);
        let mut reference_preconditioner = reference.preconditioner.unwrap();
        let mut setup_seconds = 0.0;
        let mut solve_started = Instant::now();
        let mut actual = route
            .checkpoint(
                &design,
                validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap(),
                0,
                2,
                16,
                config().max_iterations,
                false,
                0,
                None,
                &mut setup_seconds,
                &mut solve_started,
            )
            .unwrap()
            .expect("small bounded candidate installs");
        assert!(route.setup.attempted);
        assert_eq!(route.peak, reference.peak);
        let mut expected = [0.0; 4];
        let mut result = [0.0; 4];
        let rhs = [1.0, 2.0, -1.0, -2.0];
        reference_preconditioner
            .apply_batch(&rhs, &mut expected, 2, 2, None, None)
            .unwrap();
        actual
            .apply_batch(&rhs, &mut result, 2, 2, None, None)
            .unwrap();
        assert_same_bits(&result, &expected);
        results.push(result);
    }
    // Without a factor screen, every preset installs the same hierarchy.
    assert_same_bits(&results[1], &results[0]);
    assert_same_bits(&results[2], &results[1]);
}

#[test]
fn rejected_original_certificate_preserves_the_checkpoint_h() {
    const CHILD: &str = "FEREG_ROUTING_CERTIFICATE_BOUNDARY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "fe::borrowed_map::routing_tests::rejected_original_certificate_preserves_the_checkpoint_h",
                "--exact", "--test-threads=1",
            ]).status().unwrap();
        assert!(status.success());
        return;
    }
    let witness = PairedGraph::new();
    for boundary in [4, 8] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(2, true).unwrap())
            .install(|| {
                TWO_FE_CERTIFICATE_AT.set(boundary);
                TWO_FE_CERTIFICATE_REJECTED_AT.set(0);
                TWO_FE_ROUTING_BUILD_AT.set(0);
                let mut owner =
                    ReusableCategoricalMapAbsorber::new(witness.design(), 3, 2).unwrap();
                owner.retain_coefficients = true;
                owner.request_auto_routing(RoutingPreset::new(8, 0).unwrap(), None, 0);
                let result = owner
                    .absorb_ols(
                        BorrowedMapWeights::Provided(&witness.weights),
                        witness.input.clone().into(),
                        config(),
                        false,
                    )
                    .unwrap();
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
                assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 8, "the build stays at h");
                assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
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
                    assert!(owner.auto_route.as_ref().unwrap().setup.attempted);

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
                    assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
                })
                .unwrap();
        }
    }
}

#[test]
fn routing_kernel_and_direct_live_memory_refusals_preserve_incumbent_bits() {
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
        {
            let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
            owner.retain_coefficients = true;
            owner.request_auto_routing(RoutingPreset::ablation(arm).unwrap(), None, 0);
            let route = owner.auto_route.as_mut().unwrap();
            route.request.budget = Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 0,
            });
            let fit = owner
                .absorb_ols(
                    BorrowedMapWeights::Provided(&witness.weights),
                    witness.input.clone().into(),
                    config(),
                    false,
                )
                .unwrap();
            assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
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
    assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
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
                    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_some());
                    assert!(fit.report().iterations > 4);
                    assert!(fit.report().iterations <= config().max_iterations);
                    (
                        fit.residualized().clone(),
                        fit.coefficients.unwrap(),
                        fit.report.iterations,
                        route.setup.attempted,
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
fn portfolio_bounded_hierarchy_certifies_and_reuses_one_setup_across_rhs_widths() {
    if isolated_routing_witness(
        "FEREG_PORTFOLIO_BOUNDED_RHS_CHILD",
        "fe::borrowed_map::routing_tests::portfolio_bounded_hierarchy_certifies_and_reuses_one_setup_across_rhs_widths",
    ) {
        return;
    }
    let witness = PairedGraph::new();
    let mut across_workers: Option<DenseMatrix> = None;
    for workers in [1, 4] {
        let run = |preset| {
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    TWO_FE_ROUTING_BUILD_AT.set(0);
                    let mut owner =
                        ReusableCategoricalMapAbsorber::new(witness.design(), 3, workers).unwrap();
                    owner.retain_coefficients = true;
                    owner.request_auto_routing(preset, None, 0);
                    let first = owner
                        .absorb_ols(
                            BorrowedMapWeights::Provided(&witness.weights),
                            witness.columns(&[0]).into(),
                            config(),
                            false,
                        )
                        .unwrap();
                    witness.check(&first, &[0]);
                    assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 16);
                    let route = owner.auto_route.as_ref().unwrap();
                    assert_eq!(route.request.degree, 5);
                    assert_eq!(route.request.sampling.samples, 1);
                    assert_eq!(route.request.sampling.seed, 0);
                    assert!(route.setup.attempted);
                    if preset.portfolio() {
                        assert_eq!(
                            route.setup.portfolio_choice(),
                            PortfolioChoice::IncumbentCmg
                        );
                    }
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
                    let wider = owner
                        .absorb_ols(
                            BorrowedMapWeights::Provided(&witness.weights),
                            witness.input.clone().into(),
                            config(),
                            false,
                        )
                        .unwrap();
                    witness.check(&wider, &[0, 1, 2]);
                    assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
                    assert_eq!(
                        std::ptr::from_ref(
                            owner
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
                    assert!(first.report.iterations > 16);
                    assert!(wider.report.iterations <= config().max_iterations);
                    (
                        first.residualized().clone(),
                        first.coefficients.unwrap(),
                        first.report.iterations,
                        wider.residualized().clone(),
                        wider.coefficients.unwrap(),
                        wider.report.iterations,
                    )
                })
                .unwrap()
        };
        let expected = run(RoutingPreset::new(16, 1).unwrap());
        let actual = run(RoutingPreset::portfolio_v1());
        // Both policies satisfy the independent analytic witnesses above and,
        // without a factor screen, install the same bounded hierarchy.
        assert_eq!(actual, expected);
        let replay = run(RoutingPreset::portfolio_v1());
        assert_eq!(actual, replay);
        assert_eq!(actual, run(RoutingPreset::portfolio_unseeded()));
        assert_same_bits(actual.0.as_slice(), replay.0.as_slice());
        assert_same_bits(&actual.1.values, &replay.1.values);
        assert_same_bits(actual.3.as_slice(), replay.3.as_slice());
        assert_same_bits(&actual.4.values, &replay.4.values);
        if let Some(previous) = across_workers.as_ref() {
            for (actual, expected) in actual.3.as_slice().iter().zip(previous.as_slice()) {
                assert!((actual - expected).abs() < 1e-9);
            }
        }
        across_workers = Some(actual.3);
    }
}

#[test]
fn incremental_two_injection_builds_installs_and_certifies_uncapped_cmg() {
    if isolated_routing_witness(
        "FEREG_INCREMENTAL_TWO_UNCAPPED_CHILD",
        "fe::borrowed_map::routing_tests::incremental_two_injection_builds_installs_and_certifies_uncapped_cmg",
    ) {
        return;
    }
    // The hidden `routing_incremental_two` injection on an exactly-two-FE
    // Auto/Auto fit resolves to the synchronous portfolio, which has no
    // construction-work cap after M3a.
    let preset = RoutingPreset::incremental_two()
        .resolve_ols(
            crate::fe::FeSolverKind::Auto,
            crate::fe::LsmrPreconditioner::Auto,
            Some(2),
        )
        .unwrap();
    assert_eq!(preset, RoutingPreset::portfolio_unseeded());
    let witness = PairedGraph::new();
    let budgets = [
        None,
        // Memory admission still applies; a roomy explicit budget admits.
        Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1 << 32,
        }),
    ];
    let mut reference: Option<DenseMatrix> = None;
    for workers in [1, 4] {
        for budget in budgets {
            let residual = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                    .install(|| {
                        TWO_FE_ROUTING_BUILD_AT.set(0);
                        let mut owner =
                            ReusableCategoricalMapAbsorber::new(witness.design(), 3, workers)
                                .unwrap();
                        owner.retain_coefficients = true;
                        owner.request_auto_routing(preset, budget, 0);
                        let result = owner
                            .absorb_ols(
                                BorrowedMapWeights::Provided(&witness.weights),
                                witness.columns(&[0]).into(),
                                config(),
                                false,
                            )
                            .unwrap();
                        // Built once, at the fixed iteration-16 checkpoint...
                        assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 16);
                        let route = owner.auto_route.as_ref().unwrap();
                        assert!(route.setup.attempted);
                        assert_eq!(
                            route.setup.portfolio_choice(),
                            PortfolioChoice::IncumbentCmg
                        );
                        assert_eq!(
                            (route.request.degree, route.request.sampling.samples),
                            (5, 1)
                        );
                        assert_eq!(route.request.sampling.seed, 0);
                        // ...installed (not refused) with admitted retained storage...
                        let installed = owner
                            .two_fe_pcg
                            .as_ref()
                            .unwrap()
                            .cmg_schur
                            .as_ref()
                            .expect("bounded CMG must be installed");
                        assert!(installed.retained_bytes().unwrap() > 0);
                        assert_ne!(installed.construction_peak(), SolverWorkspacePeak::ZERO);
                        // ...and the restarted recurrence certifies against the
                        // original design. The analytic witness checks the exact
                        // weighted projection and every within-component FE contrast.
                        assert!(result.report.iterations > 16);
                        assert!(result.report.iterations < config().max_iterations);
                        assert!(result.report().certified());
                        witness.check(&result, &[0]);
                        result.residualized().clone()
                    })
                    .unwrap()
            }))
            .unwrap_or_else(|failure| {
                // Re-emit on the test caller: a cached worker's panic capture
                // can belong to the test that originally constructed its pool.
                let message = failure
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| failure.downcast_ref::<&str>().copied())
                    .unwrap_or("non-string panic payload");
                panic!("h16 CMG witness workers={workers}, budget={budget:?}: {message}");
            });
            if let Some(reference) = &reference {
                for (actual, expected) in residual.as_slice().iter().zip(reference.as_slice()) {
                    assert!((actual - expected).abs() < 1e-9);
                }
            } else {
                reference = Some(residual);
            }
        }
    }
}

#[test]
fn routing_worker_local_observation_is_overwritten_by_nested_action() {
    if isolated_routing_witness(
        "FEREG_ROUTING_TLS_REENTRANCY_CHILD",
        "fe::borrowed_map::routing_tests::routing_worker_local_observation_is_overwritten_by_nested_action",
    ) {
        return;
    }
    let team = crate::ThreadTeam::new(crate::ThreadConfig::from_requested(1, true).unwrap());
    team.install(|| {
        let worker = std::thread::current().id();
        let previous = TWO_FE_ROUTING_BUILD_AT.replace(16);
        // W1 forces the scoped child to run on this worker at the scope join;
        // there is no sleep, race-to-arrive assumption or scheduler lottery.
        rayon::scope(|scope| {
            scope.spawn(|_| {
                team.install(|| {
                    assert_eq!(std::thread::current().id(), worker);
                    assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 16);
                    // A reused projection resets the observation without a new
                    // constructor. It leaves zero on the shared worker.
                    TWO_FE_ROUTING_BUILD_AT.set(0);
                })
                .unwrap();
            });
        });
        assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 0);
        TWO_FE_ROUTING_BUILD_AT.set(previous);
    })
    .unwrap();
}

#[test]
fn auto_route_trace_reports_installed_hierarchy_and_nothing_for_refusals_isolated() {
    const CHILD: &str = "FEREG_AUTO_ROUTE_TRACE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let trace =
            std::env::temp_dir().join(format!("fereg-auto-route-trace-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&trace);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .env("FEREG_SOLVER_TRACE", "1")
            .env("FEREG_SOLVER_TRACE_FILE", &trace)
            .args([
                "fe::borrowed_map::routing_tests::auto_route_trace_reports_installed_hierarchy_and_nothing_for_refusals_isolated",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        let _ = std::fs::remove_file(&trace);
        assert!(status.success());
        return;
    }
    let routes = || {
        std::fs::read_to_string(std::env::var_os("FEREG_SOLVER_TRACE_FILE").unwrap())
            .unwrap_or_default()
            .lines()
            .filter(|line| line.contains("|phase=auto_schur_route|"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let witness = PairedGraph::new();
    let fit = |owner: &mut ReusableCategoricalMapAbsorber<'_>| {
        owner
            .absorb_ols(
                BorrowedMapWeights::Provided(&witness.weights),
                witness.columns(&[0]).into(),
                config(),
                false,
            )
            .unwrap()
    };

    // Installed: the route record carries the stored hierarchy metadata.
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(RoutingPreset::portfolio_unseeded(), None, 0);
    witness.check(&fit(&mut owner), &[0]);
    let installed = owner
        .two_fe_pcg
        .as_ref()
        .unwrap()
        .cmg_schur
        .as_ref()
        .unwrap()
        .trace_hierarchy();
    let lines = routes();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        format!(
            "FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1|iterations=16|installed=1|refusal=None|{installed}"
        )
    );
    let fields: std::collections::HashMap<_, _> = lines[0]
        .split('|')
        .skip(1)
        .map(|part| part.split_once('=').unwrap())
        .collect();
    let series = |key: &str| -> Vec<usize> {
        fields[key]
            .split(';')
            .map(|value| value.parse().unwrap())
            .collect()
    };
    let levels: usize = fields["levels"].parse().unwrap();
    assert!(levels >= 1);
    for key in ["hierarchy_vertices", "hierarchy_nonzeros", "repeats"] {
        assert_eq!(series(key).len(), levels, "{key}");
    }
    assert_eq!(series("hierarchy_vertices")[0], PairedGraph::LEVELS);
    let factor = fields["terminal_factor"] == "1";
    assert_eq!(factor, fields["terminal"] == "Direct");
    let dimension: usize = fields["terminal_factor_dimension"].parse().unwrap();
    assert!(dimension <= series("hierarchy_vertices")[levels - 1]);
    assert_eq!(factor, dimension > 0);
    // Bounded Auto applies the scalar cycle one lane at a time.
    assert_eq!(
        (
            fields["application"],
            fields["parallel_operators"],
            fields["batch_columns"]
        ),
        ("scalar", "0", "1")
    );

    // Refused by memory admission: installation and metadata are both absent.
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(RoutingPreset::portfolio_unseeded(), None, 0);
    owner.auto_route.as_mut().unwrap().request.budget = Some(FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: 0,
    });
    witness.check(&fit(&mut owner), &[0]);
    assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
    let lines = routes();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1],
        "FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1|iterations=16|installed=0|refusal=Some(MemoryBudget)"
    );
}

#[test]
fn portfolio_matched_cold_reused_and_wide_rhs_preserve_science_and_repeats() {
    let witness = PairedGraph::new();
    // The first RHS is the same target in all three cases. Only its batching
    // and whether a different RHS has already paid for setup are changed.
    for (positions, primed) in [(&[0][..], false), (&[0][..], true), (&[0, 1, 2][..], false)] {
        let mut across_workers: Option<Vec<CertifiedBorrowedMap>> = None;
        for workers in [1, 4] {
            let run = || {
                crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                    .install(|| {
                        let mut owner =
                            ReusableCategoricalMapAbsorber::new(witness.design(), 3, workers)
                                .unwrap();
                        owner.retain_coefficients = true;
                        owner.request_auto_routing(RoutingPreset::portfolio_unseeded(), None, 0);
                        let mut results = Vec::new();
                        let mut prior_setup = None;
                        if primed {
                            let first = owner
                                .absorb_ols(
                                    BorrowedMapWeights::Provided(&witness.weights),
                                    witness.columns(&[1]).into(),
                                    config(),
                                    false,
                                )
                                .unwrap();
                            witness.check(&first, &[1]);
                            let route = owner.auto_route.as_ref().unwrap();
                            let hierarchy = owner
                                .two_fe_pcg
                                .as_ref()
                                .unwrap()
                                .cmg_schur
                                .as_ref()
                                .unwrap();
                            assert!(route.setup.attempted);
                            prior_setup = Some(std::ptr::from_ref(hierarchy.as_ref()));
                            results.push(first);
                        }
                        let target = owner
                            .absorb_ols(
                                BorrowedMapWeights::Provided(&witness.weights),
                                witness.columns(positions).into(),
                                config(),
                                false,
                            )
                            .unwrap();
                        witness.check(&target, positions);
                        let route = owner.auto_route.as_ref().unwrap();
                        let hierarchy = owner
                            .two_fe_pcg
                            .as_ref()
                            .unwrap()
                            .cmg_schur
                            .as_ref()
                            .unwrap();
                        assert!(route.setup.attempted);
                        assert_eq!(
                            route.setup.portfolio_choice(),
                            PortfolioChoice::IncumbentCmg
                        );
                        let setup = route.setup.attempted;
                        if let Some(pointer) = prior_setup {
                            assert_eq!(std::ptr::from_ref(hierarchy.as_ref()), pointer);
                        }
                        results.push(target);
                        for result in &mut results {
                            assert!(result.report.iterations <= config().max_iterations);
                            // Timings are not a repeatability target. Retain every
                            // scientific diagnostic and the original certificate.
                            result.report.setup_seconds = 0.;
                            result.report.solve_seconds = 0.;
                            result.report.certification_seconds = 0.;
                        }
                        (results, setup)
                    })
                    .unwrap()
            };
            let actual = run();
            let replay = run();
            assert_eq!(actual, replay);
            for (actual, replay) in actual.0.iter().zip(&replay.0) {
                assert_same_bits(
                    actual.residualized().as_slice(),
                    replay.residualized().as_slice(),
                );
                assert_same_bits(
                    &actual.coefficients.as_ref().unwrap().values,
                    &replay.coefficients.as_ref().unwrap().values,
                );
            }
            if let Some(previous) = across_workers.as_ref() {
                assert_eq!(actual.0.len(), previous.len());
                for (actual, previous) in actual.0.iter().zip(previous) {
                    let discrete = |report: &SolveReport| {
                        (
                            report.requested_solver,
                            report.used_solver,
                            report.stop_reason,
                            report.fallback_used,
                            report.requested_preconditioner,
                            report.used_preconditioner,
                            report.checkpoint_retries,
                            report.pilot_iterations,
                            report.warnings.clone(),
                        )
                    };
                    assert_eq!(discrete(actual.report()), discrete(previous.report()));
                    for (&actual, &expected) in actual
                        .residualized()
                        .as_slice()
                        .iter()
                        .zip(previous.residualized().as_slice())
                    {
                        assert!((actual - expected).abs() < 1e-9);
                    }
                }
            }
            // Across workers and across batch layouts, the analytic witness is
            // authoritative; neither iteration counts nor rounding must match.
            across_workers = Some(actual.0);
        }
    }
}

fn independent_rhs_sequence(
    witness: &PairedGraph,
    workers: usize,
    positions: &[usize],
    primed: bool,
) -> (Vec<CertifiedBorrowedMap>, bool) {
    crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
        .install(|| {
            let mut owner =
                ReusableCategoricalMapAbsorber::new(witness.design(), 16, workers).unwrap();
            owner.retain_coefficients = true;
            owner.request_auto_routing(RoutingPreset::portfolio_unseeded(), None, 0);
            let mut projections = Vec::new();
            if primed {
                projections.push(&[1][..]);
            }
            projections.push(positions);
            let mut results = Vec::new();
            let mut installed = None;
            for (projection, columns) in projections.into_iter().enumerate() {
                TWO_FE_ROUTING_BUILD_AT.set(0);
                let mut fit = owner
                    .absorb_ols(
                        BorrowedMapWeights::Provided(&witness.weights),
                        witness.columns(columns).into(),
                        config(),
                        false,
                    )
                    .unwrap();
                witness.check_signals(
                    &fit,
                    columns,
                    [
                        PairedGraph::independent_left,
                        PairedGraph::independent_right,
                    ],
                );
                let route = owner.auto_route.as_ref().unwrap();
                let hierarchy = owner.two_fe_pcg.as_ref().unwrap().cmg_schur.as_ref();
                assert!(
                    hierarchy.is_some(),
                    "workers={workers}, columns={columns:?}"
                );
                assert!(route.setup.attempted);
                assert_eq!(
                    route.setup.portfolio_choice(),
                    PortfolioChoice::IncumbentCmg
                );
                // Same capacity, intentionally different first submitted widths:
                // the single optional attempt is never renewed for this key.
                let setup = route.setup.attempted;
                let pointer = std::ptr::from_ref(hierarchy.unwrap().as_ref());
                if let Some(prior) = installed {
                    assert_eq!((setup, pointer), prior);
                    assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 0);
                } else {
                    assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 16);
                }
                assert!(!fit.report.fallback_used);
                // A retained hierarchy is applied at recurrence initialization.
                // Progress after cold installation (or on reuse) therefore
                // proves at least one CMG application, not sixteen active lanes.
                assert!(fit.report.iterations > if projection == 0 { 16 } else { 0 });
                assert!(fit.report.iterations <= config().max_iterations);
                installed = Some((setup, pointer));
                fit.report.setup_seconds = 0.;
                fit.report.solve_seconds = 0.;
                fit.report.certification_seconds = 0.;
                results.push(fit);
            }
            (results, installed.unwrap().0)
        })
        .unwrap()
}

fn assert_independent_rhs_workers(
    actual: &[CertifiedBorrowedMap],
    expected: &[CertifiedBorrowedMap],
) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        let discrete = |report: &SolveReport| {
            (
                report.requested_solver,
                report.used_solver,
                report.stop_reason,
                report.fallback_used,
                report.requested_preconditioner,
                report.used_preconditioner,
                report.checkpoint_retries,
                report.pilot_iterations,
                report.warnings.clone(),
            )
        };
        assert_eq!(discrete(actual.report()), discrete(expected.report()));
        let a = actual.coefficients.as_ref().unwrap();
        let b = expected.coefficients.as_ref().unwrap();
        assert_eq!(a.level_counts, b.level_counts);
        assert_eq!(a.effect_offsets, b.effect_offsets);
        assert_eq!(a.coefficients_per_rhs, b.coefficients_per_rhs);
        for (&a, &b) in actual
            .residualized()
            .as_slice()
            .iter()
            .zip(expected.residualized().as_slice())
        {
            assert!((a - b).abs() < 1e-9);
        }
        // The independent per-component contrast checks above remain the FE
        // witness; gauge choices need not be compared across worker counts.
    }
}

#[test]
fn portfolio_sixteen_independent_rhs_cover_matched_cold_and_reused_routes() {
    if isolated_routing_witness(
        "FEREG_PORTFOLIO_INDEPENDENT_RHS_CHILD",
        "fe::borrowed_map::routing_tests::portfolio_sixteen_independent_rhs_cover_matched_cold_and_reused_routes",
    ) {
        return;
    }
    let witness = PairedGraph::independent_columns();
    let sixteen: Vec<_> = (0..16).collect();
    let sequences = [
        (&[0][..], false),
        (&[0][..], true),
        (&[0, 1, 2, 3][..], false),
        (sixteen.as_slice(), false),
        (sixteen.as_slice(), true),
    ];
    let mut owners = 0;
    let mut projections = 0;
    for (positions, primed) in sequences {
        let mut across_workers: Option<Vec<CertifiedBorrowedMap>> = None;
        for workers in [1, 4] {
            let actual = independent_rhs_sequence(&witness, workers, positions, primed);
            let replay = independent_rhs_sequence(&witness, workers, positions, primed);
            owners += 2;
            projections += actual.0.len() + replay.0.len();
            assert_eq!(actual, replay);
            for (actual, replay) in actual.0.iter().zip(&replay.0) {
                assert_same_bits(
                    actual.residualized().as_slice(),
                    replay.residualized().as_slice(),
                );
                assert_same_bits(
                    &actual.coefficients.as_ref().unwrap().values,
                    &replay.coefficients.as_ref().unwrap().values,
                );
            }
            if let Some(previous) = across_workers.as_ref() {
                assert_independent_rhs_workers(&actual.0, previous);
            }
            across_workers = Some(actual.0);
        }
    }
    assert_eq!((owners, projections), (20, 28));
}

#[test]
fn routing_and_portfolio_memory_refusal_preserve_the_exact_incumbent() {
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
    assert_eq!(reference.report().used_solver, FeSolverKind::Cg);
    for preset in [
        RoutingPreset::new(4, 1).unwrap(),
        RoutingPreset::portfolio_v1(),
    ] {
        let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
        owner.retain_coefficients = true;
        owner.request_auto_routing(preset, None, 0);
        owner.auto_route.as_mut().unwrap().request.budget = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 0,
        });
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
        assert_eq!(fit.report().used_solver, FeSolverKind::Cg);
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
fn routing_and_portfolio_native_caps_around_checkpoint_never_extend_the_action() {
    let witness = PairedGraph::new();
    for preset in [
        RoutingPreset::new(4, 1).unwrap(),
        RoutingPreset::new(8, 1).unwrap(),
        RoutingPreset::new(16, 1).unwrap(),
        RoutingPreset::new(32, 1).unwrap(),
        RoutingPreset::portfolio_v1(),
    ] {
        let checkpoint = preset.checkpoint();
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
            owner.request_auto_routing(preset, None, 0);
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
    assert_eq!(fit.report().used_solver, FeSolverKind::Map);
    assert_eq!(fit.residualized(), &witness.residual);
    let route = owner.auto_route.as_ref().unwrap();
    assert!(!route.setup.attempted);
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
fn routing_low_rhs_chunks_share_preparation_but_final_accuracy_has_a_new_attempt() {
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
    assert_eq!(first.report().used_solver, FeSolverKind::CmgSchur);
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
    assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
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
        assert_eq!(chunk.report().used_solver, FeSolverKind::CmgSchur);
        assert!(owner.auto_route.as_ref().unwrap().setup.attempted);
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
    assert_eq!(final_fit.report().used_solver, FeSolverKind::CmgSchur);
    assert!(!owner.auto_route.as_ref().unwrap().setup.attempted);
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

#[path = "eager_cell_tests.rs"]
mod eager_cell_tests;

/// Worker/firm/third-factor panel with a ring that connects every firm. The
/// confounded panel has low worker mobility and a third factor that nearly
/// copies `firm % 20`; the mixing panel draws firms and the third factor
/// independently. Both have 680 total coefficients.
struct TrialPanel {
    codes: [Vec<u32>; 3],
    indices: Vec<LevelIndex>,
    values: DenseMatrix,
    levels: [usize; 3],
}

impl TrialPanel {
    fn new(confounded: bool) -> Self {
        Self::with_levels(confounded, [600, 60, 20])
    }

    fn with_levels(confounded: bool, levels: [usize; 3]) -> Self {
        let [workers, firms, thirds] = levels.map(|level| level as u64);
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state >> 11
        };
        let mut codes = [Vec::new(), Vec::new(), Vec::new()];
        let mut columns = [Vec::new(), Vec::new()];
        for worker in 0..workers {
            let degree = 1 + next() % 3;
            for period in 0..10_u64 {
                let firm = if worker < firms {
                    (worker + period % 2) % firms
                } else if confounded {
                    (worker * 7 + (period % degree) * 13 + next() % 2) % firms
                } else {
                    // Every observation moves: a well-mixed worker-firm graph.
                    next() % firms
                };
                let third = if confounded {
                    if next() % 200 == 0 {
                        next() % thirds
                    } else {
                        firm % thirds
                    }
                } else {
                    next() % thirds
                };
                codes[0].push(worker as u32);
                codes[1].push(firm as u32);
                codes[2].push(third as u32);
                let noise = (next() % 1024) as f64 / 512.0 - 1.0;
                columns[0].push(noise + (worker as f64 * 0.071).sin() + (firm as f64 * 0.13).cos());
                columns[1].push(noise * 0.5 + third as f64 / 20.0);
            }
        }
        let indices = codes
            .iter()
            .zip(levels)
            .map(|(code, levels)| LevelIndex::from_codes(code, levels).unwrap())
            .collect();
        Self {
            codes,
            indices,
            values: DenseMatrix::from_columns(&columns).unwrap(),
            levels,
        }
    }

    fn design(&self) -> BorrowedInterceptFeDesign<'_> {
        let sources: Vec<_> = self
            .codes
            .iter()
            .zip(self.levels)
            .zip(&self.indices)
            .map(|((code, levels), index)| {
                ValidatedBorrowedInterceptSource::new(code, levels, index).unwrap()
            })
            .collect();
        BorrowedInterceptFeDesign::new(
            self.values.nrows(),
            sources.iter().map(|source| source.effect()).collect(),
        )
        .unwrap()
    }

    fn project<'a>(
        &self,
        route: &mut routing_lsmr::GeneralRoutingState<'a>,
        design: &BorrowedInterceptFeDesign<'a>,
        deterministic: bool,
    ) -> CertifiedBorrowedMap {
        self.project_with(route, design, deterministic, true)
    }

    fn project_with<'a>(
        &self,
        route: &mut routing_lsmr::GeneralRoutingState<'a>,
        design: &BorrowedInterceptFeDesign<'a>,
        deterministic: bool,
        retain: bool,
    ) -> CertifiedBorrowedMap {
        let lsmr = LsmrConfig {
            max_iterations: 4_000,
            threads: crate::ThreadConfig {
                requested: None,
                deterministic,
            },
            ..LsmrConfig::default()
        };
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            2,
            &self.levels,
            rayon::current_num_threads(),
            lsmr.local_size,
        )
        .unwrap();
        project_general_categorical(
            design,
            BorrowedMapWeights::Unit,
            self.values.clone(),
            MapConfig {
                tolerance: 1e-8,
                max_iterations: 8_000,
                ..config()
            },
            lsmr,
            None,
            retain,
            estimate,
            false,
            None,
            Some(route),
            false,
        )
        .unwrap()
    }
}

#[test]
fn jacobi_trial_is_reserved_for_designs_resolved_to_the_additive_preconditioner() {
    // Below the slot rule's threshold Auto already resolves to the diagonal
    // preconditioner, so the geometry policy builds it once without a trial,
    // even where a trial would finish without escalating.
    let panel = TrialPanel::with_levels(false, [100, 20, 8]);
    let design = panel.design();
    assert!(design.total_term_slots() < 256);
    let mut route = routing_lsmr::GeneralRoutingState::new(RoutingPreset::geometry_many());
    let mut incumbent = routing_lsmr::GeneralRoutingState::new(RoutingPreset::portfolio_unseeded());
    let actual = panel.project(&mut route, &design, true);
    let reference = panel.project(&mut incumbent, &design, true);
    assert!(!route.jacobi_trial_active());
    for fit in [&actual, &reference] {
        assert!(fit.report().certified());
        assert_eq!(fit.report().used_solver, FeSolverKind::Lsmr);
        assert_eq!(
            fit.report().used_preconditioner,
            Some(LsmrPreconditioner::Diagonal)
        );
    }
    assert!(actual.report().iterations <= reference.report().iterations);
}

#[test]
fn jacobi_trial_finishes_mixing_designs_and_escalates_near_confounded_ones() {
    for (confounded, deterministic) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let panel = TrialPanel::new(confounded);
        let design = panel.design();
        assert!(design.total_term_slots() >= 256);
        let mut trial = routing_lsmr::GeneralRoutingState::new(RoutingPreset::geometry_many());
        let mut incumbent =
            routing_lsmr::GeneralRoutingState::new(RoutingPreset::portfolio_unseeded());
        let actual = panel.project(&mut trial, &design, deterministic);
        let reference = panel.project(&mut incumbent, &design, deterministic);
        for fit in [&actual, &reference] {
            assert!(fit.report().certified());
            assert_eq!(fit.report().used_solver, FeSolverKind::Lsmr);
            assert!(!fit.report().fallback_used);
        }
        assert_eq!(
            reference.report().used_preconditioner,
            Some(LsmrPreconditioner::Schwarz)
        );
        assert_eq!(
            actual.report().used_preconditioner,
            Some(if confounded {
                LsmrPreconditioner::Schwarz
            } else {
                LsmrPreconditioner::Diagonal
            })
        );
        assert_eq!(trial.jacobi_trial_active(), !confounded);
        assert!(actual.report().certificate.relative_infinity_norm <= 1e-8);
        // Both certified projections agree with each other on every row, and
        // their FE coefficients reconstruct the original values.
        for rhs in 0..2 {
            let scale = (0..design.nobs())
                .map(|row| panel.values[(row, rhs)].abs())
                .fold(0.0_f64, f64::max);
            let coefficients = actual.coefficients.as_ref().unwrap();
            let base = rhs * coefficients.coefficients_per_rhs;
            for row in 0..design.nobs() {
                assert!(
                    (actual.residualized()[(row, rhs)] - reference.residualized()[(row, rhs)])
                        .abs()
                        <= 1e-6 * scale,
                    "confounded={confounded} rhs={rhs} row={row}"
                );
                let fitted: f64 = panel
                    .codes
                    .iter()
                    .enumerate()
                    .map(|(effect, code)| {
                        coefficients.values
                            [base + coefficients.effect_offsets[effect] + code[row] as usize]
                    })
                    .sum();
                assert!(
                    (panel.values[(row, rhs)] - actual.residualized()[(row, rhs)] - fitted).abs()
                        <= 1e-10 * scale
                );
            }
        }
        // A later call on the binding reuses the retained solver: an escalated
        // binding keeps the additive preconditioner without another trial.
        let again = panel.project(&mut trial, &design, deterministic);
        assert!(again.report().certified());
        assert_eq!(
            again.report().used_preconditioner,
            actual.report().used_preconditioner
        );
        assert_eq!(trial.jacobi_trial_active(), !confounded);
        // A fresh binding repeats the same trial decision; reproducible solvers
        // repeat its bits exactly.
        let mut fresh = routing_lsmr::GeneralRoutingState::new(RoutingPreset::geometry_many());
        let repeat = panel.project(&mut fresh, &design, deterministic);
        assert_eq!(
            repeat.report().used_preconditioner,
            actual.report().used_preconditioner
        );
        assert_eq!(repeat.report().iterations, actual.report().iterations);
        if deterministic {
            assert_eq!(repeat.residualized(), actual.residualized());
        }
    }
}

#[test]
fn geometry_certificate_miss_takes_one_seeded_correction_before_map() {
    let panel = TrialPanel::new(false);
    let design = panel.design();
    for retain in [true, false] {
        let fresh = || routing_lsmr::GeneralRoutingState::new(RoutingPreset::geometry_many());
        let reference = panel.project_with(&mut fresh(), &design, true, retain);
        assert!(reference.report().certified());
        // One forced miss: the correction certifies on LSMR without fallback.
        GENERAL_LSMR_CERTIFICATE_FAILURES.set(1);
        GENERAL_LSMR_CORRECTIONS.set(0);
        let corrected = panel.project_with(&mut fresh(), &design, true, retain);
        assert_eq!(GENERAL_LSMR_CERTIFICATE_FAILURES.get(), 0);
        assert_eq!(GENERAL_LSMR_CORRECTIONS.get(), 1);
        let report = corrected.report();
        assert!(report.certified(), "{report:?}");
        assert_eq!(report.used_solver, FeSolverKind::Lsmr);
        assert!(!report.fallback_used);
        assert!(report.warnings.is_empty(), "{report:?}");
        assert!(report.iterations > reference.report().iterations);
        assert_eq!(corrected.coefficients.is_some(), retain);
        for rhs in 0..2 {
            let scale = (0..design.nobs())
                .map(|row| panel.values[(row, rhs)].abs())
                .fold(0.0_f64, f64::max);
            for row in 0..design.nobs() {
                let gap =
                    corrected.residualized()[(row, rhs)] - reference.residualized()[(row, rhs)];
                assert!(
                    gap.abs() <= 1e-6 * scale,
                    "retain={retain} rhs={rhs} row={row}"
                );
            }
        }
        if let Some(coefficients) = corrected.coefficients.as_ref() {
            // The seeded result already includes the seed: its coefficients
            // reconstruct the original values.
            for rhs in 0..2 {
                let base = rhs * coefficients.coefficients_per_rhs;
                for row in 0..design.nobs() {
                    let fitted: f64 = panel
                        .codes
                        .iter()
                        .enumerate()
                        .map(|(effect, code)| {
                            coefficients.values
                                [base + coefficients.effect_offsets[effect] + code[row] as usize]
                        })
                        .sum();
                    let total = corrected.residualized()[(row, rhs)] + fitted;
                    assert!((panel.values[(row, rhs)] - total).abs() <= 1e-9);
                }
            }
        }
        // A reproducible repeat of the forced path repeats its bits.
        GENERAL_LSMR_CERTIFICATE_FAILURES.set(1);
        let repeat = panel.project_with(&mut fresh(), &design, true, retain);
        assert_eq!(repeat.residualized(), corrected.residualized());
        assert_eq!(repeat.report().iterations, report.iterations);
        // A second miss leaves the original-input MAP fallback.
        GENERAL_LSMR_CERTIFICATE_FAILURES.set(2);
        GENERAL_LSMR_CORRECTIONS.set(0);
        let fallback = panel.project_with(&mut fresh(), &design, true, retain);
        assert_eq!(GENERAL_LSMR_CORRECTIONS.get(), 1);
        assert!(fallback.report().certified());
        assert_eq!(fallback.report().used_solver, FeSolverKind::Map);
        assert!(fallback.report().fallback_used);
    }
    // Presets without the geometry policy keep their direct MAP fallback.
    GENERAL_LSMR_CERTIFICATE_FAILURES.set(1);
    GENERAL_LSMR_CORRECTIONS.set(0);
    let mut incumbent = routing_lsmr::GeneralRoutingState::new(RoutingPreset::portfolio_unseeded());
    let strict = panel.project(&mut incumbent, &design, true);
    assert_eq!(GENERAL_LSMR_CORRECTIONS.get(), 0);
    assert_eq!(strict.report().used_solver, FeSolverKind::Map);
    assert!(strict.report().fallback_used);
}

#[test]
fn geometry_recurrence_stops_a_decade_below_the_request_and_corrects_at_the_hundredth() {
    use crate::fe::auto::{corrected_lsmr_tolerance, correction_tolerance};
    assert_eq!(corrected_lsmr_tolerance(1e-8), 1e-9);
    assert_eq!(correction_tolerance(1e-8, 1e-9), Some(1e-10));
    // The shared floor leaves no tighter correction at final-residual accuracy.
    assert_eq!(corrected_lsmr_tolerance(1e-13), 1e-14);
    assert_eq!(correction_tolerance(1e-13, 1e-14), None);
    assert_eq!(corrected_lsmr_tolerance(1e-14), 1e-14);
    assert_eq!(correction_tolerance(1e-14, 1e-14), None);
    assert_eq!(correction_tolerance(1e-8, 1e-10), None);
}

// The OLS session applies an installed hierarchy on the host team: row
// operators and interleaved lanes, with the scalar route's fits bit for bit
// (no coarse component reaches CMG's 131,072-entry team centering here).
#[test]
fn team_application_reproduces_scalar_route_fits_at_every_worker_count() {
    let witness = PairedGraph::new();
    let positions = [0, 1, 2];
    let run = |workers: usize, team: bool| {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                let mut owner =
                    ReusableCategoricalMapAbsorber::new(witness.design(), 3, workers).unwrap();
                owner.retain_coefficients = true;
                owner.request_auto_routing(RoutingPreset::portfolio_unseeded(), None, 0);
                if team {
                    owner.apply_auto_routing_on_team();
                }
                let mut result = owner
                    .absorb_ols(
                        BorrowedMapWeights::Provided(&witness.weights),
                        witness.columns(&positions).into(),
                        config(),
                        false,
                    )
                    .unwrap();
                witness.check(&result, &positions);
                let trace = owner
                    .two_fe_pcg
                    .as_ref()
                    .unwrap()
                    .cmg_schur
                    .as_ref()
                    .unwrap()
                    .trace_hierarchy();
                result.report.setup_seconds = 0.;
                result.report.solve_seconds = 0.;
                result.report.certification_seconds = 0.;
                (result, trace)
            })
            .unwrap()
    };
    let (reference, scalar_trace) = run(1, false);
    assert!(scalar_trace.ends_with("|application=scalar|parallel_operators=0|batch_columns=1"));
    for workers in [1, 4] {
        let (scalar, _) = run(workers, false);
        let (team, trace) = run(workers, true);
        assert!(trace.contains("|application=parallel|"), "{trace}");
        assert!(trace.ends_with("|batch_columns=3"), "{trace}");
        assert_eq!(scalar, reference);
        assert_eq!(team, reference);
    }
}

/// The campaign's sparse three-FE topology at a reduced size: workers with
/// five rows each, firms with 2% mobility per period and a 32-level third
/// effect `(firm + 17 worker + period) % 32`, all zero-based.
struct SparseThreePanel {
    codes: [Vec<u32>; 3],
    indices: Vec<LevelIndex>,
    values: DenseMatrix,
    levels: [usize; 3],
}

impl SparseThreePanel {
    fn new(rows: usize) -> Self {
        let periods = 5;
        let workers = rows.div_ceil(periods);
        let firms = (workers / 20).max(8);
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state >> 11
        };
        let mut codes = [Vec::new(), Vec::new(), Vec::new()];
        let mut columns = [Vec::new(), Vec::new()];
        for worker in 0..workers {
            let mut firm = worker % firms;
            for period in 0..periods {
                if codes[0].len() == rows {
                    break;
                }
                if period > 0 && next() % 50 == 0 {
                    firm = next() as usize % firms;
                }
                codes[0].push(worker as u32);
                codes[1].push(firm as u32);
                codes[2].push(((firm + 17 * worker + period) % 32) as u32);
                let noise = (next() % 1024) as f64 / 512.0 - 1.0;
                columns[0].push(noise + (worker as f64 * 0.071).sin() + (firm as f64 * 0.13).cos());
                columns[1].push(noise * 0.5 + ((firm + period) % 32) as f64 / 20.0);
            }
        }
        let levels = [workers, firms, 32];
        let indices = codes
            .iter()
            .zip(levels)
            .map(|(code, levels)| LevelIndex::from_codes(code, levels).unwrap())
            .collect();
        Self {
            codes,
            indices,
            values: DenseMatrix::from_columns(&columns).unwrap(),
            levels,
        }
    }

    fn design(&self) -> BorrowedInterceptFeDesign<'_> {
        let sources: Vec<_> = self
            .codes
            .iter()
            .zip(self.levels)
            .zip(&self.indices)
            .map(|((code, levels), index)| {
                ValidatedBorrowedInterceptSource::new(code, levels, index).unwrap()
            })
            .collect();
        BorrowedInterceptFeDesign::new(
            self.values.nrows(),
            sources.iter().map(|source| source.effect()).collect(),
        )
        .unwrap()
    }

    fn lsmr() -> LsmrConfig {
        LsmrConfig {
            max_iterations: 4_000,
            threads: crate::ThreadConfig {
                requested: None,
                deterministic: true,
            },
            ..LsmrConfig::default()
        }
    }

    fn project<'a>(
        &self,
        design: &BorrowedInterceptFeDesign<'a>,
        route: &mut routing_lsmr::GeneralRoutingState<'a>,
        budget: Option<FeSolverMemoryBudget>,
    ) -> CertifiedBorrowedMap {
        let lsmr = Self::lsmr();
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            2,
            &self.levels,
            rayon::current_num_threads(),
            lsmr.local_size,
        )
        .unwrap();
        project_general_categorical(
            design,
            BorrowedMapWeights::Unit,
            self.values.clone(),
            MapConfig {
                tolerance: 1e-8,
                max_iterations: 8_000,
                ..config()
            },
            lsmr,
            budget,
            true,
            estimate,
            false,
            None,
            Some(route),
            false,
        )
        .unwrap()
    }
}

#[test]
fn refused_level_bound_admits_additive_lsmr_by_exact_yielding_count() {
    // At campaign scale the level-count Schwarz bound grows quadratically in
    // the levels and refused LSMR, sending the fit to the MAP fallback. Here a
    // budget one byte below that bound must admit the same additive LSMR solve
    // through the exact yielding pair-component count, with the unbudgeted
    // solve's bits, at every worker count.
    let panel = SparseThreePanel::new(1_000_000);
    let lsmr = SparseThreePanel::lsmr();
    let mut fits = Vec::new();
    for workers in [1, 4] {
        let fit =
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let coarse = estimate_fe_solver_memory(
                        panel.values.nrows(),
                        2,
                        &panel.levels,
                        workers,
                        lsmr.local_size,
                    )
                    .unwrap();
                    let budget = FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: coarse.lsmr_schwarz_bytes - 1,
                    };
                    let design = panel.design();
                    let mut route =
                        routing_lsmr::GeneralRoutingState::new(RoutingPreset::geometry_many());
                    let reference = panel.project(&design, &mut route, None);
                    // An unbudgeted or admitted call never counts components.
                    assert_eq!(design.known_yielding_pair_components(), None);
                    let mut fits = Vec::new();
                    for _ in 0..2 {
                        let design = panel.design();
                        let mut route =
                            routing_lsmr::GeneralRoutingState::new(RoutingPreset::geometry_many());
                        let fit = panel.project(&design, &mut route, Some(budget));
                        assert_eq!(design.known_yielding_pair_components(), Some(3));
                        let refined = crate::memory::estimate_fe_solver_memory_with_yielding(
                            panel.values.nrows(),
                            2,
                            &panel.levels,
                            workers,
                            lsmr.local_size,
                            Some(3),
                        )
                        .unwrap();
                        assert!(budget.fits(refined.lsmr_schwarz_bytes));
                        fits.push(fit);
                    }
                    for fit in fits.iter().chain([&reference]) {
                        assert!(fit.report().certified());
                        assert_eq!(fit.report().used_solver, FeSolverKind::Lsmr);
                        assert!(!fit.report().fallback_used);
                        assert!(fit.report().certificate.relative_infinity_norm <= 1e-8);
                    }
                    for fit in &fits {
                        assert_eq!(fit.report().iterations, reference.report().iterations);
                        assert_eq!(fit.residualized(), reference.residualized());
                    }
                    fits.swap_remove(0)
                })
                .unwrap();
        fits.push(fit);
    }
    // Scientific agreement across worker counts, and the retained FE
    // coefficients reconstruct the original values on every row.
    for rhs in 0..2 {
        let scale = (0..panel.values.nrows())
            .map(|row| panel.values[(row, rhs)].abs())
            .fold(0.0_f64, f64::max);
        for row in 0..panel.values.nrows() {
            assert!(
                (fits[0].residualized()[(row, rhs)] - fits[1].residualized()[(row, rhs)]).abs()
                    <= 1e-6 * scale
            );
        }
        for fit in &fits {
            let coefficients = fit.coefficients.as_ref().unwrap();
            let base = rhs * coefficients.coefficients_per_rhs;
            for row in 0..panel.values.nrows() {
                let fitted: f64 = panel
                    .codes
                    .iter()
                    .enumerate()
                    .map(|(effect, code)| {
                        coefficients.values
                            [base + coefficients.effect_offsets[effect] + code[row] as usize]
                    })
                    .sum();
                assert!(
                    (panel.values[(row, rhs)] - fit.residualized()[(row, rhs)] - fitted).abs()
                        <= 1e-10 * scale
                );
            }
        }
    }
}

/// Agreement of two residual banks within a relative bound of the input scale.
fn assert_banks_agree(left: &DenseMatrix, right: &DenseMatrix, input: &DenseMatrix, bound: f64) {
    for rhs in 0..input.ncols() {
        let scale = (0..input.nrows())
            .map(|row| input[(row, rhs)].abs())
            .fold(0.0_f64, f64::max);
        for row in 0..input.nrows() {
            assert!((left[(row, rhs)] - right[(row, rhs)]).abs() <= bound * scale);
        }
    }
}

#[test]
fn refused_level_bound_admits_explicit_lsmr_by_exact_yielding_count() {
    // Explicit solver(lsmr) has no fallback: a refused level-count bound was an
    // allocation error. The exact count admits the unchanged explicit solve.
    let panel = SparseThreePanel::new(200_000);
    let mut fits = Vec::new();
    for workers in [1, 4] {
        let fit =
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let mut config = crate::fe::AbsorbConfig {
                        solver: FeSolverKind::Lsmr,
                        lsmr: SparseThreePanel::lsmr(),
                        ..crate::fe::AbsorbConfig::default()
                    };
                    let threads = config
                        .lsmr
                        .threads
                        .resolved()
                        .max(rayon::current_num_threads());
                    let coarse = estimate_fe_solver_memory(
                        panel.values.nrows(),
                        2,
                        &panel.levels,
                        threads,
                        config.lsmr.local_size,
                    )
                    .unwrap();
                    let refined = crate::memory::estimate_fe_solver_memory_with_yielding(
                        panel.values.nrows(),
                        2,
                        &panel.levels,
                        threads,
                        config.lsmr.local_size,
                        Some(2),
                    )
                    .unwrap();
                    assert!(refined.lsmr_schwarz_bytes < coarse.lsmr_schwarz_bytes);
                    let absorb = |config: crate::fe::AbsorbConfig| {
                        let mut session = BorrowedLsmrAbsorber::new(
                            panel.design(),
                            BorrowedMapWeights::Unit,
                            config,
                        )
                        .unwrap();
                        let fit = session.absorb(
                            BorrowedMapWeights::Unit,
                            panel.values.clone(),
                            super::super::MapGeometryGeneration {
                                sample: 0,
                                topology: 0,
                                weights: 0,
                            },
                            config,
                            false,
                        );
                        (fit, session.into_design().known_yielding_pair_components())
                    };
                    let (reference, counted) = absorb(config);
                    let reference = reference.unwrap();
                    assert_eq!(counted, None);
                    // Below the exact bound the refusal stays an allocation error.
                    config.solver_memory_budget = Some(FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: refined.lsmr_schwarz_bytes - 1,
                    });
                    let (refused, counted) = absorb(config);
                    assert_eq!(refused.unwrap_err().code, ErrorCode::Allocation);
                    assert_eq!(counted, Some(2));
                    config.solver_memory_budget = Some(FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: coarse.lsmr_schwarz_bytes - 1,
                    });
                    for _ in 0..2 {
                        let (fit, counted) = absorb(config);
                        let fit = fit.unwrap();
                        assert_eq!(counted, Some(2));
                        assert!(fit.report().certified());
                        assert_eq!(fit.report().used_solver, FeSolverKind::Lsmr);
                        assert_eq!(fit.report().iterations, reference.report().iterations);
                        assert_eq!(fit.residualized(), reference.residualized());
                    }
                    reference
                })
                .unwrap();
        fits.push(fit);
    }
    assert_banks_agree(
        fits[0].residualized(),
        fits[1].residualized(),
        &panel.values,
        1e-6,
    );
}

#[test]
fn refused_level_bound_admits_owned_lsmr_by_exact_yielding_count() {
    // Owned intercept-only designs: explicit LSMR errors and Auto falls back to
    // MAP when the level-count bound refuses. With the exact count both run
    // the unchanged LSMR solve with the unbudgeted solve's bits. A design with
    // slopes keeps the level-count bound.
    let panel = SparseThreePanel::new(200_000);
    let design = crate::fe::FeDesign::new(
        panel.values.nrows(),
        panel
            .codes
            .iter()
            .zip(panel.levels)
            .map(|(codes, levels)| {
                crate::fe::EffectTerm::new(codes.clone(), levels, true, Vec::new()).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let weights = vec![1.0; panel.values.nrows()];
    let mut fits = Vec::new();
    for workers in [1, 4] {
        let fit =
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let mut explicit = Vec::new();
                    for solver in [FeSolverKind::Lsmr, FeSolverKind::Auto] {
                        let mut config = crate::fe::AbsorbConfig {
                            solver,
                            lsmr: SparseThreePanel::lsmr(),
                            map: MapConfig {
                                tolerance: 1e-8,
                                max_iterations: 8_000,
                                ..config()
                            },
                            ..crate::fe::AbsorbConfig::default()
                        };
                        let preserved = if solver == FeSolverKind::Lsmr
                            && !config.certified_fallback
                        {
                            0
                        } else {
                            crate::memory::owned_fe_matrix_bytes(panel.values.nrows(), 2).unwrap()
                        };
                        let coarse = crate::memory::generic_fe_solver_bytes(
                            &design,
                            2,
                            FeSolverKind::Lsmr,
                            config.lsmr,
                            None,
                        )
                        .unwrap()
                            + preserved;
                        let mut reference = panel.values.clone();
                        let reference_report = crate::fe::absorb_fixed_effects(
                            &design,
                            &weights,
                            &mut reference,
                            config,
                        )
                        .unwrap();
                        assert_eq!(reference_report.used_solver, FeSolverKind::Lsmr);
                        config.solver_memory_budget = Some(FeSolverMemoryBudget {
                            execution_bytes: 0,
                            usable_bytes: coarse - 1,
                        });
                        for _ in 0..2 {
                            let mut values = panel.values.clone();
                            let report = crate::fe::absorb_fixed_effects(
                                &design,
                                &weights,
                                &mut values,
                                config,
                            )
                            .unwrap();
                            assert_eq!(report.used_solver, FeSolverKind::Lsmr);
                            assert!(!report.fallback_used);
                            assert!(report.certified());
                            assert_eq!(report.iterations, reference_report.iterations);
                            assert_eq!(values, reference);
                        }
                        if solver == FeSolverKind::Lsmr {
                            explicit.push(reference);
                        }
                    }
                    explicit.swap_remove(0)
                })
                .unwrap();
        fits.push(fit);
    }
    assert_banks_agree(&fits[0], &fits[1], &panel.values, 1e-6);
    // A slope channel keeps the level-count bound: still refused.
    let slope: Vec<f64> = (0..panel.values.nrows())
        .map(|row| 1.0 + (row % 7) as f64)
        .collect();
    let sloped = crate::fe::FeDesign::new(
        panel.values.nrows(),
        panel
            .codes
            .iter()
            .zip(panel.levels)
            .enumerate()
            .map(|(effect, (codes, levels))| {
                let slopes = if effect == 2 {
                    vec![slope.clone()]
                } else {
                    Vec::new()
                };
                crate::fe::EffectTerm::new(codes.clone(), levels, true, slopes).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let mut config = crate::fe::AbsorbConfig {
        solver: FeSolverKind::Lsmr,
        lsmr: SparseThreePanel::lsmr(),
        ..crate::fe::AbsorbConfig::default()
    };
    let coarse =
        crate::memory::generic_fe_solver_bytes(&sloped, 2, FeSolverKind::Lsmr, config.lsmr, None)
            .unwrap()
            + crate::memory::owned_fe_matrix_bytes(panel.values.nrows(), 2).unwrap();
    config.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: coarse - 1,
    });
    let mut values = panel.values.clone();
    let error =
        crate::fe::absorb_fixed_effects(&sloped, &weights, &mut values, config).unwrap_err();
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(sloped.yielding_pair_component_scratch_bytes(), None);
}
