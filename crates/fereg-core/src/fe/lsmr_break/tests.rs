use super::*;
use crate::data::DenseMatrix;
use crate::error::ErrorCode;
use crate::fe::borrowed_map::block_schur::{Cache, Outcome, Plan, Request};
use crate::fe::{
    BorrowedInterceptFeDesign, BorrowedMapWeights, LevelIndex, LsmrPreconditioner, MapConfig,
    ValidatedBorrowedInterceptSource,
};

/// The witness parks fedemean's explicit modified LSMR after a completed
/// iteration; the host's cancellation then stops the solve with the
/// upstream Cancelled result, the input is untouched, and a later unarmed
/// solve reproduces the reference bit for bit.
#[test]
fn iteration_break_observes_fedemean_cancellation_and_resets() {
    const CHILD: &str = "FEREG_LSMR_ITERATION_BREAK_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "fe::lsmr_break::tests::iteration_break_observes_fedemean_cancellation_and_resets",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let rows = 360;
    let levels = [12_usize, 9, 7];
    let codes: Vec<Vec<u32>> = levels
        .iter()
        .map(|&count| (0..rows).map(|row| (row % count) as u32).collect())
        .collect();
    let indices: Vec<_> = codes
        .iter()
        .zip(levels)
        .map(|(c, l)| LevelIndex::from_codes(c, l).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(levels)
        .zip(&indices)
        .map(|((c, l), i)| ValidatedBorrowedInterceptSource::new(c, l, i).unwrap())
        .collect();
    let design =
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap();
    let weights: Vec<_> = (0..rows).map(|row| 1.0 + (row % 11) as f64 / 7.0).collect();
    let columns: Vec<Vec<f64>> = (0..4)
        .map(|column| {
            (0..rows)
                .map(|row| ((row + column * 3) as f64).sin())
                .collect()
        })
        .collect();
    let input = DenseMatrix::from_columns(&columns).unwrap();
    let map = MapConfig {
        tolerance: 1e-12,
        ..MapConfig::default()
    };
    let plan = Plan::lsmr(LsmrPreconditioner::Diagonal);
    let project = |values: DenseMatrix| {
        Cache::with_plan(plan).project(
            &design,
            BorrowedMapWeights::Provided(&weights),
            values,
            map,
            None,
            Request::default(),
        )
    };
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let Outcome::Done(reference) = pool.install(|| project(input.clone())).unwrap() else {
            panic!("reference declined");
        };
        let guard = DeveloperLsmrBreak::claim(DeveloperLsmrBackend::Fedemean).unwrap();
        assert!(DeveloperLsmrBreak::claim(DeveloperLsmrBackend::PpmlJoint).is_err());
        assert!(policy(DeveloperLsmrBackend::PpmlJoint).is_none());
        record_cancelled(DeveloperLsmrBackend::Fedemean);
        assert_eq!(
            STATE.load(Ordering::Acquire),
            3,
            "no cancellation witness before progress"
        );
        let controller = std::thread::spawn(|| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !lsmr_break_ready() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let ready = lsmr_break_ready();
            crate::interrupt::request_cancellation();
            ready
        });
        let error = match pool.install(|| project(input.clone())) {
            Err(error) => error,
            Ok(_) => panic!("an armed solve must not publish a result"),
        };
        assert!(
            controller.join().unwrap(),
            "an iteration must complete before the witness"
        );
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(STATE.load(Ordering::Acquire), 3 | READY | CANCELLED);
        drop(guard);
        crate::interrupt::reset_cancellation();
        assert!(!lsmr_break_ready());
        assert!(!armed(DeveloperLsmrBackend::Fedemean));
        let Outcome::Done(reset) = pool.install(|| project(input.clone())).unwrap() else {
            panic!("reset declined");
        };
        assert_eq!(reset.residualized(), reference.residualized());
    }
}
