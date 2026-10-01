use super::{fixture, hash};
use crate::ffi::{allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    memory::Inputs,
    ppml::{self, InterruptPoint, Phase, Resources},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    ErrorCode, FeregError,
};
use std::cell::Cell;

fn audit(solver: Solver, workers: usize) -> (usize, u64) {
    let model = fixture::model(false);
    let loaded = fixture::loaded(2053, false, true);
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let options = Options {
        solver,
        threads: workers,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let request = ppml::Request::default();
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
    let caller = std::thread::current().id();
    let in_trial = Cell::new(false);
    let count = Cell::new(0_usize);
    let estimated = Cell::new(false);
    let failure = FeregError::new(ErrorCode::NonConvergence, "objective host r(430)");
    let run = |stop| {
        in_trial.set(false);
        count.set(0);
        estimated.set(false);
        make().run_with_team(&loaded, None, &team, &mut |phase| {
            assert_eq!(std::thread::current().id(), caller);
            match phase {
                Phase::Numerical(InterruptPoint::LineSearch) => in_trial.set(true),
                Phase::Numerical(InterruptPoint::DenseLinearAlgebra) if in_trial.get() => {
                    count.set(count.get() + 1);
                    if stop == Some(count.get()) {
                        return Err(failure.clone());
                    }
                }
                Phase::Numerical(
                    InterruptPoint::FixedEffects | InterruptPoint::DenseLinearAlgebra,
                ) => (),
                Phase::Estimated => {
                    in_trial.set(false);
                    estimated.set(true);
                }
                _ => in_trial.set(false),
            }
            Ok(())
        })
    };
    let fingerprint = |fit: &ppml::PpmlFit| {
        let mut value = 0;
        hash(&fit.coefficients, &mut value);
        hash(fit.covariance.values.as_slice(), &mut value);
        value
    };
    let plain = make()
        .run_with_team(&loaded, None, &team, &mut |_| Ok(()))
        .unwrap();
    let expected = fingerprint(&plain);
    drop(plain);
    let baseline = run(None).unwrap();
    assert_eq!(fingerprint(&baseline), expected);
    let census = count.get();
    assert!(census > 20 && estimated.get());
    drop(baseline);
    for stop in [1, census.div_ceil(2), census] {
        let (before, _) = begin_ols_allocation_observation();
        let error = run(Some(stop)).unwrap_err();
        assert_eq!(error, failure);
        assert_eq!(count.get(), stop);
        assert!(!estimated.get());
        drop(error);
        assert!(allocation_usage().current_bytes <= before.current_bytes);
        let peak = allocation_usage()
            .peak_bytes
            .saturating_sub(before.current_bytes);
        assert!(
            peak as u128 <= estimate,
            "objective peak {peak} > sole plan {estimate}"
        );
        let fit = run(None).unwrap();
        assert_eq!(fingerprint(&fit), expected);
        assert_eq!(count.get(), census);
        drop(fit);
        assert!(allocation_usage().current_bytes <= before.current_bytes);
    }
    eprintln!("objective {solver:?}/{workers}: {census} trial dense callbacks; three caller r(430) aborts, released owners and sole-plan peak");
    (census, expected)
}

#[test]
#[ignore = "process-global objective allocator audit; run with --test-threads=1"]
fn trial_objective_interruptions_release_point_mean_and_partial_owners_and_restart() {
    let _guard = crate::instrumentation::lock_action_observation();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        assert_eq!(audit(solver, 1), audit(solver, 8));
    }
}
