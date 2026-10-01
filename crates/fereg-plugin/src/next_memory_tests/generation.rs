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
    let g = [0., 0., 1., 1., 1., 2., 2., 2., 2., 2., 2.];
    let h = [0., 3., 0., 1., 2., 1., 1., 2., 2., 3., 3.];
    let y = [0., 0., 1., 0., 0., 1., 1., 1., 1., 1., 1.];
    let n = 11 * 373;
    let column = |values| NumericColumn {
        values,
        missing: None,
    };
    let numeric = |values| LoadedSource::Numeric(column(values));
    Loaded {
        candidate: vec![true; n],
        outcome: column((0..n).map(|row| y[row % 11]).collect()),
        sources: vec![
            numeric(
                (0..n)
                    .map(|row| g[row % 11] + 3. * (row / 11) as f64)
                    .collect(),
            ),
            numeric(
                (0..n)
                    .map(|row| h[row % 11] + 4. * (row / 11) as f64)
                    .collect(),
            ),
            numeric((0..n).map(|row| row as f64 / 8.).collect()),
            numeric((0..n).map(|row| (row % 7) as f64).collect()),
        ],
        weights: None,
        adjustment: LoadedAdjustment::None,
    }
}

#[test]
#[ignore = "process-global staged-generation allocator audit; run with --test-threads=1"]
fn ppml_generation_interruptions_release_every_staged_sample_before_final_fitting() {
    let _guard = crate::instrumentation::lock_action_observation();
    let mut model = fixture::model(false);
    model.keep_singletons = false;
    let loaded = loaded();
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let request = ppml::Request {
        separation: separation::Methods::only(separation::Method::FixedEffects),
        ..Default::default()
    };
    let count = Rc::new(Cell::new(0));
    let rounds = Rc::new(Cell::new(0));
    let caller = std::thread::current().id();
    let failure = FeregError::new(ErrorCode::NonConvergence, "generation host r(430)");
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
            make()
                .run_with_team(&loaded, None, &team, &mut |phase| {
                    assert_eq!(std::thread::current().id(), caller);
                    match phase {
                        Phase::Numerical(InterruptPoint::SeparationRound) => {
                            rounds.set(rounds.get() + 1);
                            // The next round can only begin after the exact/singleton union is committed.
                            if rounds.get() == 3 {
                                return Err(failure.clone());
                            }
                        }
                        Phase::Numerical(InterruptPoint::DenseLinearAlgebra) => {
                            count.set(count.get() + 1);
                            if count.get() == stop {
                                return Err(failure.clone());
                            }
                        }
                        Phase::Separated | Phase::Estimated => {
                            panic!("audit crossed into final estimation")
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
        assert_eq!(*reference.get_or_insert(total), total);
        for stop in 1..=total {
            let (before, _) = begin_ols_allocation_observation();
            let error = run(stop);
            assert_eq!(error, failure);
            assert_eq!(count.get(), stop);
            drop(error);
            assert!(
                allocation_usage().current_bytes <= before.current_bytes,
                "retained staged owner at {stop}/{total}"
            );
            let peak = allocation_usage()
                .peak_bytes
                .saturating_sub(before.current_bytes);
            assert!(
                peak as u128 <= estimate,
                "staged peak {peak} exceeds sole plan {estimate}"
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
    eprintln!("{tracked} tracked PPML preparation/generation aborts: original caller r(430), every staged owner released, exact fresh checkpoint counts at one/eight workers within the sole plan");
}
