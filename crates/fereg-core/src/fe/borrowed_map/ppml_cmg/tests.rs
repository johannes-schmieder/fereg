use super::*;
use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

/// Workers bridge adjacent firms of one long chain. Each worker appears
/// twice at its first firm and once at the next, so firms are retained.
struct Chain {
    workers: Vec<u32>,
    firms: Vec<u32>,
    worker_index: LevelIndex,
    firm_index: LevelIndex,
}

impl Chain {
    fn new(firms: usize, per_link: usize) -> Self {
        let (mut worker_codes, mut firm_codes) = (Vec::new(), Vec::new());
        for link in 0..firms - 1 {
            for slot in 0..per_link {
                let worker = (link * per_link + slot) as u32;
                for firm in [link, link + 1, link] {
                    worker_codes.push(worker);
                    firm_codes.push(firm as u32);
                }
            }
        }
        let workers = (firms - 1) * per_link;
        Self {
            worker_index: LevelIndex::from_codes(&worker_codes, workers).unwrap(),
            firm_index: LevelIndex::from_codes(&firm_codes, firms).unwrap(),
            workers: worker_codes,
            firms: firm_codes,
        }
    }

    fn rows(&self) -> usize {
        self.workers.len()
    }

    fn absorber(&self) -> ReusableCategoricalMapAbsorber<'_> {
        let sources = [
            ValidatedBorrowedInterceptSource::new(
                &self.workers,
                self.worker_index.n_levels(),
                &self.worker_index,
            )
            .unwrap(),
            ValidatedBorrowedInterceptSource::new(
                &self.firms,
                self.firm_index.n_levels(),
                &self.firm_index,
            )
            .unwrap(),
        ];
        let design = BorrowedInterceptFeDesign::new(
            self.rows(),
            sources.iter().map(|s| s.effect()).collect(),
        )
        .unwrap();
        ReusableCategoricalMapAbsorber::new(design, 2, 2).unwrap()
    }

    fn weights(&self, frame: u64) -> Vec<f64> {
        (0..self.rows())
            .map(|row| (0.4 + ((row as u64 * 7 + frame * 3) % 11) as f64 / 5.0).exp())
            .collect()
    }

    fn values(&self) -> DenseMatrix {
        let rows = self.rows();
        let data = (0..rows * 2)
            .map(|i| ((i / 2) as f64 * if i % 2 == 0 { 0.37 } else { 0.11 }).sin() + 0.2)
            .collect();
        DenseMatrix::from_row_major(rows, 2, data).unwrap()
    }
}

fn generation(weights: u64) -> MapGeometryGeneration {
    MapGeometryGeneration {
        sample: 1,
        topology: 1,
        weights,
    }
}

fn state<'s>(absorber: &'s ReusableCategoricalMapAbsorber<'_>) -> &'s PpmlCmg {
    absorber.ppml_cmg.as_ref().unwrap()
}

fn installed(absorber: &ReusableCategoricalMapAbsorber<'_>) -> bool {
    absorber.two_fe_pcg.as_ref().unwrap().cmg_schur.is_some()
}

const CONFIG: MapConfig = MapConfig {
    tolerance: 1e-12,
    max_iterations: 4000,
    check_every: 1,
    symmetric: true,
};

#[test]
fn joint_block_applies_the_retained_hierarchy_in_joint_coordinates() {
    let chain = Chain::new(90, 2);
    let mut absorber = chain.absorber();
    absorber.enable_ppml_cmg(None, u128::MAX).unwrap();
    absorber.begin_ppml_frame(generation(1)).unwrap();
    let weights = chain.weights(1);
    let firms = chain.firm_index.n_levels();
    // The other orientation declines rather than misapplying the hierarchy.
    assert!(JointCmgBlock::new(Some(&mut absorber), &weights, Some(0)).is_none());
    let residual: Vec<f64> = (0..firms).map(|f| (f as f64 * 0.61).cos()).collect();
    let mut actual = vec![0.0; firms];
    let mut transposed = vec![0.0; firms];
    let other: Vec<f64> = (0..firms).map(|f| (f as f64 * 0.23).sin()).collect();
    {
        let mut block = JointCmgBlock::new(Some(&mut absorber), &weights, Some(1)).unwrap();
        assert!(!block.installed());
        assert!(block.install().unwrap());
        assert!(
            block.install().unwrap(),
            "a frame attempts construction once"
        );
        block.apply(&residual, &mut actual).unwrap();
        block.apply(&other, &mut transposed).unwrap();
    }
    assert_eq!(state(&absorber).attempts, 1);
    assert_eq!(state(&absorber).built, Some(generation(1)));
    let mut roots = vec![0.0; firms];
    for (&firm, &weight) in chain.firms.iter().zip(&weights) {
        roots[firm as usize] += weight;
    }
    let roots: Vec<f64> = roots.into_iter().map(f64::sqrt).collect();
    let raw: Vec<f64> = residual.iter().zip(&roots).map(|(r, d)| r * d).collect();
    let mut solved = vec![0.0; firms];
    absorber
        .two_fe_pcg
        .as_mut()
        .unwrap()
        .cmg_schur
        .as_mut()
        .unwrap()
        .apply_batch(&raw, &mut solved, 1, 1, None, None)
        .unwrap();
    let expected: Vec<f64> = solved.iter().zip(&roots).map(|(z, d)| z * d).collect();
    assert_eq!(
        actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    // A fixed symmetric cycle keeps the scaled block symmetric and positive.
    let forward: f64 = other.iter().zip(&actual).map(|(a, b)| a * b).sum();
    let backward: f64 = residual.iter().zip(&transposed).map(|(a, b)| a * b).sum();
    assert!((forward - backward).abs() <= 1e-10 * forward.abs().max(backward.abs()));
    assert!(
        residual
            .iter()
            .zip(&actual)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            > 0.0
    );
}

#[test]
fn estimation_builds_at_most_three_times_and_refreshes_only_stale_strict_hierarchies() {
    let chain = Chain::new(120, 2);
    let mut absorber = chain.absorber();
    absorber.enable_ppml_cmg(None, u128::MAX).unwrap();
    let mut reference = chain.absorber();
    // Joint construction on frame one.
    absorber.begin_ppml_frame(generation(1)).unwrap();
    let first = chain.weights(1);
    {
        let mut block = JointCmgBlock::new(Some(&mut absorber), &first, Some(1)).unwrap();
        assert!(block.install().unwrap());
    }
    // A projection finishing before the route's checkpoint reinstates the
    // set-aside hierarchy and keeps the attempt.
    absorber.begin_ppml_frame(generation(2)).unwrap();
    let loose = MapConfig {
        tolerance: 1e-2,
        ..CONFIG
    };
    let quick = absorber
        .absorb_ppml(
            BorrowedMapWeights::Provided(&chain.weights(2)),
            chain.values(),
            loose,
        )
        .unwrap();
    assert!(quick.report().certified() && quick.report().iterations < 16);
    assert_eq!(state(&absorber).attempts, 1);
    assert_eq!(state(&absorber).built, Some(generation(1)));
    assert!(installed(&absorber) && state(&absorber).aside.is_none());
    // A strict projection on a later frame rebuilds once on current weights.
    for (frame, attempts, built) in [(3, 2, 3), (5, 3, 5), (6, 3, 5)] {
        absorber.begin_ppml_frame(generation(frame)).unwrap();
        reference.begin_ppml_frame(generation(frame)).unwrap();
        let weights = chain.weights(frame);
        let fit = absorber
            .absorb_ppml(
                BorrowedMapWeights::Provided(&weights),
                chain.values(),
                CONFIG,
            )
            .unwrap();
        let expected = reference
            .absorb_ppml(
                BorrowedMapWeights::Provided(&weights),
                chain.values(),
                CONFIG,
            )
            .unwrap();
        assert!(fit.report().certified());
        assert!(fit.report().iterations < expected.report().iterations);
        for (a, b) in fit
            .residualized()
            .as_slice()
            .iter()
            .zip(expected.residualized().as_slice())
        {
            assert!((a - b).abs() <= 1e-9 * (1.0 + b.abs()), "{a} != {b}");
        }
        assert_eq!(state(&absorber).attempts, attempts);
        assert_eq!(state(&absorber).built, Some(generation(built)));
        assert!(installed(&absorber));
        // A withheld route is restored for later projections.
        assert!(absorber.auto_route.is_some());
    }
    // With all attempts used, a missing hierarchy is not rebuilt.
    absorber.two_fe_pcg.as_mut().unwrap().cmg_schur = None;
    let weights = chain.weights(7);
    absorber.begin_ppml_frame(generation(7)).unwrap();
    assert!(!JointCmgBlock::new(Some(&mut absorber), &weights, Some(1))
        .unwrap()
        .install()
        .unwrap());
    absorber
        .absorb_ppml(
            BorrowedMapWeights::Provided(&weights),
            chain.values(),
            CONFIG,
        )
        .unwrap();
    assert!(!installed(&absorber));
    assert_eq!(state(&absorber).attempts, 3);
}

#[test]
fn a_refused_construction_consumes_its_attempt_and_installs_nothing() {
    let chain = Chain::new(60, 2);
    let mut absorber = chain.absorber();
    let budget = FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: 64,
    };
    absorber.enable_ppml_cmg(Some(budget), u128::MAX).unwrap();
    absorber.begin_ppml_frame(generation(1)).unwrap();
    let weights = chain.weights(1);
    {
        let mut block = JointCmgBlock::new(Some(&mut absorber), &weights, Some(1)).unwrap();
        assert!(!block.install().unwrap());
        assert!(!block.installed());
    }
    assert_eq!(state(&absorber).attempts, 1);
    assert_eq!(state(&absorber).built, None);
    // The application banks are charged against their admitted allowance.
    let mut other = chain.absorber();
    let bytes = PpmlCmg::bytes(chain.firm_index.n_levels()).unwrap();
    assert!(other.enable_ppml_cmg(None, bytes - 1).is_err());
    other.enable_ppml_cmg(None, bytes).unwrap();
}

// Host cancellation inside the joint construction fails the frame as an
// interrupted estimation instead of resuming the diagonal recurrence.
#[test]
fn cancellation_during_joint_construction_propagates() {
    const CHILD: &str = "FEREG_PPML_CMG_CANCEL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "fe::borrowed_map::ppml_cmg::tests::cancellation_during_joint_construction_propagates",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let chain = Chain::new(60, 2);
    let mut absorber = chain.absorber();
    absorber.enable_ppml_cmg(None, u128::MAX).unwrap();
    absorber.begin_ppml_frame(generation(1)).unwrap();
    let weights = chain.weights(1);
    let mut block = JointCmgBlock::new(Some(&mut absorber), &weights, Some(1)).unwrap();
    crate::interrupt::request_cancellation();
    let error = block.install().unwrap_err();
    crate::interrupt::reset_cancellation();
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert!(!block.installed());
}

#[test]
fn a_canonical_refresh_rebuilds_only_a_stale_hierarchy_within_the_attempts() {
    let chain = Chain::new(90, 2);
    let mut absorber = chain.absorber();
    absorber.enable_ppml_cmg(None, u128::MAX).unwrap();
    // Nothing installed: a refresh neither builds nor spends an attempt.
    absorber.begin_ppml_frame(generation(1)).unwrap();
    assert!(!absorber
        .refresh_ppml_cmg(BorrowedMapWeights::Provided(&chain.weights(1)), "test")
        .unwrap());
    assert_eq!(state(&absorber).attempts, 0);
    assert!(absorber.ppml_cmg_view().is_none());
    assert_eq!(absorber.ppml_cmg_reach(), Some((1, 90, false)));
    let weights = chain.weights(1);
    let mut block = JointCmgBlock::new(Some(&mut absorber), &weights, Some(1)).unwrap();
    assert!(block.install().unwrap());
    assert_eq!(state(&absorber).attempts, 1);
    // Current hierarchies are kept.
    assert!(!absorber
        .refresh_ppml_cmg(BorrowedMapWeights::Provided(&weights), "test")
        .unwrap());
    assert_eq!(state(&absorber).attempts, 1);
    // Stale ones are rebuilt on current weights while attempts remain.
    for frame in [2, 3] {
        absorber.begin_ppml_frame(generation(frame)).unwrap();
        assert!(absorber
            .refresh_ppml_cmg(BorrowedMapWeights::Provided(&chain.weights(frame)), "test")
            .unwrap());
        assert_eq!(state(&absorber).attempts, frame as u8);
        assert_eq!(state(&absorber).built, Some(generation(frame)));
    }
    // Without an attempt the stale hierarchy still serves.
    absorber.begin_ppml_frame(generation(4)).unwrap();
    assert!(!absorber
        .refresh_ppml_cmg(BorrowedMapWeights::Provided(&chain.weights(4)), "test")
        .unwrap());
    assert_eq!(state(&absorber).attempts, 3);
    assert_eq!(state(&absorber).built, Some(generation(3)));
    assert_eq!(absorber.ppml_cmg_reach(), Some((1, 90, true)));
    let view = absorber.ppml_cmg_view().unwrap();
    // The shared application is the installed cycle on a compatible input.
    let firms = chain.firm_index.n_levels();
    let mut input: Vec<f64> = (0..firms).map(|f| (f as f64 * 0.43).sin()).collect();
    let mean = input.iter().sum::<f64>() / firms as f64;
    input.iter_mut().for_each(|value| *value -= mean);
    let (mut shared, mut batched) = (vec![0.0; firms], vec![0.0; firms]);
    let mut scratch = view.scratch().unwrap();
    view.apply(&input, &mut shared, &mut scratch).unwrap();
    absorber
        .two_fe_pcg
        .as_mut()
        .unwrap()
        .cmg_schur
        .as_mut()
        .unwrap()
        .apply_batch(&input, &mut batched, 1, 1, None, None)
        .unwrap();
    let shift = shared[0] - batched[0];
    for (shared, batched) in shared.iter().zip(&batched) {
        assert!((shared - shift - batched).abs() <= 1e-12 * (1. + batched.abs()));
    }
}
