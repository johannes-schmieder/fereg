use super::*;
use crate::data::DenseMatrix;
use crate::error::ErrorCode;
use crate::fe::{absorb_lsmr, EffectTerm, FeDesign, LsmrConfig, LsmrPreconditioner};
use crate::options::ThreadConfig;

#[test]
fn iteration_break_observes_upstream_cancellation_and_resets() {
    const CHILD: &str = "FEREG_LSMR_ITERATION_BREAK_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "fe::lsmr_break::tests::iteration_break_observes_upstream_cancellation_and_resets",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let rows = 360;
    let effects = [12, 9, 7]
        .into_iter()
        .map(|levels| {
            EffectTerm::new(
                (0..rows).map(|row| (row % levels) as u32).collect(),
                levels,
                true,
                vec![],
            )
            .unwrap()
        })
        .collect();
    let design = FeDesign::new(rows, effects).unwrap();
    let weights: Vec<_> = (0..rows).map(|row| 1.0 + (row % 11) as f64 / 7.0).collect();
    let columns: Vec<Vec<f64>> = (0..4)
        .map(|column| {
            (0..rows)
                .map(|row| ((row + column * 3) as f64).sin())
                .collect()
        })
        .collect();
    let input = DenseMatrix::from_columns(&columns).unwrap();
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let config = LsmrConfig {
            tolerance: 1e-12,
            preconditioner: LsmrPreconditioner::Diagonal,
            threads: ThreadConfig::from_requested(threads, true).unwrap(),
            ..LsmrConfig::default()
        };
        let mut reference = input.clone();
        pool.install(|| absorb_lsmr(&design, &weights, &mut reference, config))
            .unwrap();
        let guard = DeveloperLsmrBreak::claim(DeveloperLsmrBackend::Within).unwrap();
        assert!(DeveloperLsmrBreak::claim(DeveloperLsmrBackend::PpmlJoint).is_err());
        assert!(policy(DeveloperLsmrBackend::PpmlJoint).is_none());
        record_cancelled(DeveloperLsmrBackend::Within);
        assert_eq!(
            STATE.load(Ordering::Acquire),
            1,
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
        let mut interrupted = input.clone();
        let error = pool
            .install(|| absorb_lsmr(&design, &weights, &mut interrupted, config))
            .unwrap_err();
        assert!(
            controller.join().unwrap(),
            "an upstream iteration must complete"
        );
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(STATE.load(Ordering::Acquire), 1 | READY | CANCELLED);
        assert_eq!(
            interrupted, input,
            "failed absorption must not copy partial results"
        );
        drop(guard);
        crate::interrupt::reset_cancellation();
        assert!(!lsmr_break_ready());
        assert!(policy(DeveloperLsmrBackend::Within).is_none());
        let mut reset = input.clone();
        pool.install(|| absorb_lsmr(&design, &weights, &mut reset, config))
            .unwrap();
        assert_eq!(reset, reference);
    }
}
