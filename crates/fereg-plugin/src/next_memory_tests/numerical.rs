use super::fixture;
use crate::ffi::{allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    data::{Loaded, LoadedAdjustment, LoadedSource, NumericColumn},
    memory::Inputs,
    ppml::{self, separation, InterruptPoint, Phase, Resources},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    ErrorCode, FeregError,
};
use std::{cell::Cell, rc::Rc};

fn loaded() -> Loaded {
    let n = 2053;
    let column = |values| NumericColumn {
        values,
        missing: None,
    };
    let numeric = |values| LoadedSource::Numeric(column(values));
    Loaded {
        candidate: vec![true; n],
        outcome: column(
            (0..n)
                .map(|row| if row % 2 == 0 { 1. } else { 0. })
                .collect(),
        ),
        sources: vec![
            numeric((0..n).map(|row| ((row / 2) % 5) as f64).collect()),
            numeric((0..n).map(|row| ((row / 10) % 7) as f64).collect()),
            numeric(
                (0..n)
                    .map(|row| if row % 2 == 0 { 0. } else { -1. })
                    .collect(),
            ),
            numeric((0..n).map(|row| ((row / 2) % 3) as f64).collect()),
        ],
        weights: None,
        adjustment: LoadedAdjustment::None,
    }
}

#[test]
#[ignore = "process-global numerical-separation allocator audit; run with --test-threads=1"]
fn numerical_separation_interruptions_release_span_rectifier_and_staged_owners() {
    let _guard = crate::instrumentation::lock_action_observation();
    let mut model = fixture::model(false);
    model.keep_singletons = false;
    let loaded = loaded();
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let request = ppml::Request {
        separation: separation::Methods::only(separation::Method::IteratedRectifier),
        ..Default::default()
    };
    let count = Rc::new(Cell::new(0));
    let rounds = Rc::new(Cell::new(0));
    let round_counts = Rc::new(Cell::new([0_usize; 3]));
    let caller = std::thread::current().id();
    let failure = FeregError::new(
        ErrorCode::NonConvergence,
        "numerical separation host r(430)",
    );
    let mut tracked = 0;
    let mut reference = None;
    for workers in [1, 8] {
        let options = Options {
            threads: workers,
            ..Options::for_estimator(Estimator::Ppml)
        };
        let team = ThreadTeam::new(workers).unwrap();
        let make = || {
            ppml::Call::new(
                &model,
                &options,
                &request,
                inputs,
                None,
                Resources::default(),
            )
            .unwrap()
        };
        let estimate = make().memory().peak_bytes();
        let run = |stop| {
            count.set(0);
            rounds.set(0);
            round_counts.set([0; 3]);
            make()
                .run_with_team(&loaded, None, &team, &mut |phase| {
                    assert_eq!(std::thread::current().id(), caller);
                    match phase {
                        Phase::Numerical(InterruptPoint::SeparationRound) => {
                            rounds.set(rounds.get() + 1);
                            // The next round requires a certified numerical drop and its atomic cascade.
                            if rounds.get() == 3 {
                                return Err(failure.clone());
                            }
                        }
                        Phase::Numerical(InterruptPoint::DenseLinearAlgebra) => {
                            count.set(count.get() + 1);
                            let mut counts = round_counts.get();
                            counts[rounds.get()] += 1;
                            round_counts.set(counts);
                            if count.get() == stop {
                                return Err(failure.clone());
                            }
                        }
                        Phase::Separated | Phase::Estimated => {
                            panic!("audit reached final fitting")
                        }
                        _ => (),
                    }
                    Ok(())
                })
                .unwrap_err()
        };
        assert_eq!(run(usize::MAX), failure);
        assert_eq!(rounds.get(), 3);
        let total = count.get();
        assert!(total > 100);
        let counts = round_counts.get();
        assert!(counts.iter().all(|&n| n > 0));
        assert_eq!(*reference.get_or_insert(counts), counts);
        eprintln!("numerical-separation census: {total} dense checkpoints at {workers} workers");
        let mut stops = Vec::new();
        let mut start = 0;
        for n in counts {
            stops.extend([start + 1, start + n.div_ceil(2), start + n]);
            start += n;
        }
        stops.sort_unstable();
        stops.dedup();
        for stop in stops {
            let (before, _) = begin_ols_allocation_observation();
            let error = run(stop);
            assert_eq!(error, failure);
            assert_eq!(count.get(), stop);
            drop(error);
            assert!(
                allocation_usage().current_bytes <= before.current_bytes,
                "retained numerical/staged owner at {stop}/{total}"
            );
            let peak = allocation_usage()
                .peak_bytes
                .saturating_sub(before.current_bytes);
            assert!(
                peak as u128 <= estimate,
                "numerical peak {peak} exceeds sole plan {estimate}"
            );
            let error = run(usize::MAX);
            assert_eq!(error, failure);
            assert_eq!(rounds.get(), 3);
            assert_eq!(count.get(), total);
            drop(error);
            assert!(allocation_usage().current_bytes <= before.current_bytes);
            tracked += 1;
        }
    }
    eprintln!("{tracked} tracked numerical preparation/separation aborts at first/middle/last checkpoints of each round: original caller r(430), released span/rectifier/staged owners, exact fresh counts at one/eight workers within the sole plan; exhaustive kernel checkpoints remain independently covered");
}
