use super::fixture;
use crate::ffi::{allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    data::{Loaded, LoadedAdjustment, LoadedSource, NumericColumn},
    memory::Inputs,
    ppml::{self, separation, InterruptPoint, Phase, Resources},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    ErrorCode, FeregError, SourceId,
};
use std::cell::Cell;

fn loaded(infeasible: bool) -> Loaded {
    let n = 201;
    let column = |values| NumericColumn {
        values,
        missing: None,
    };
    let numeric = |values| LoadedSource::Numeric(column(values));
    let x: Vec<_> = (0..n)
        .map(|row| {
            if infeasible {
                [0., 1., -1.][row % 3]
            } else {
                [1., 3., 0.][row % 3]
            }
        })
        .collect();
    let z = (0..n)
        .map(|row| {
            if infeasible {
                2. * x[row]
            } else {
                [1., 0., 3.][row % 3]
            }
        })
        .collect();
    Loaded {
        candidate: vec![true; n],
        outcome: column((0..n).map(|row| f64::from(row % 3 == 0)).collect()),
        sources: vec![
            numeric(vec![0.; n]),
            numeric(vec![0.; n]),
            numeric(x),
            numeric(z),
        ],
        weights: None,
        adjustment: LoadedAdjustment::None,
    }
}
fn audit(infeasible: bool, workers: usize) -> ([usize; 4], usize) {
    let mut model = fixture::model(false);
    model.model.terms[1].components[0].source = SourceId::new(4).unwrap();
    let loaded = loaded(infeasible);
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let options = Options {
        threads: workers,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let request = ppml::Request {
        separation: separation::Methods::only(separation::Method::Simplex),
        ..Default::default()
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
    let counts = Cell::new([0_usize; 4]);
    let stage = Cell::new(0);
    let separated = Cell::new(false);
    let caller = std::thread::current().id();
    let failure = FeregError::new(ErrorCode::NonConvergence, "simplex host r(430)");
    let run = |stop| {
        counts.set([0; 4]);
        stage.set(0);
        separated.set(false);
        make()
            .run_with_team(&loaded, None, &team, &mut |phase| {
                assert_eq!(std::thread::current().id(), caller);
                match phase {
                    Phase::Numerical(InterruptPoint::SimplexModel) => stage.set(1),
                    Phase::Numerical(InterruptPoint::SimplexReduction) => stage.set(2),
                    Phase::Numerical(InterruptPoint::SimplexPivot) => stage.set(3),
                    Phase::Numerical(InterruptPoint::DenseLinearAlgebra) => {
                        let mut observed = counts.get();
                        observed[stage.get()] += 1;
                        counts.set(observed);
                        if stop == Some((stage.get(), observed[stage.get()])) {
                            return Err(failure.clone());
                        }
                    }
                    Phase::Separated => {
                        separated.set(true);
                        return Err(failure.clone());
                    }
                    Phase::Estimated => panic!("simplex audit reached final fitting"),
                    _ => (),
                }
                Ok(())
            })
            .unwrap_err()
    };
    assert_eq!(run(None), failure);
    assert!(separated.get());
    let census = counts.get();
    assert!(census[1..].iter().all(|&n| n > 0));
    let mut tracked = 0;
    for (part, &total) in census.iter().enumerate() {
        if total == 0 {
            continue;
        }
        let mut stops = [1, total.div_ceil(2), total];
        stops.sort_unstable();
        for (index, &ordinal) in stops.iter().enumerate() {
            if index > 0 && ordinal == stops[index - 1] {
                continue;
            }
            let (before, _) = begin_ols_allocation_observation();
            let error = run(Some((part, ordinal)));
            assert_eq!(error, failure);
            assert_eq!(stage.get(), part);
            assert_eq!(counts.get()[part], ordinal);
            assert!(!separated.get());
            drop(error);
            assert!(
                allocation_usage().current_bytes <= before.current_bytes,
                "retained simplex owner at {part}/{ordinal}/{total}"
            );
            let peak = allocation_usage()
                .peak_bytes
                .saturating_sub(before.current_bytes);
            assert!(
                peak as u128 <= estimate,
                "simplex peak {peak} exceeds sole plan {estimate}"
            );
            let error = run(None);
            assert_eq!(error, failure);
            assert!(separated.get());
            assert_eq!(counts.get(), census);
            drop(error);
            assert!(allocation_usage().current_bytes <= before.current_bytes);
            tracked += 1;
        }
    }
    eprintln!("simplex infeasible={infeasible}, workers={workers}: {census:?}, {tracked} first/middle/last tracked aborts; caller r(430), complete cleanup and sole-plan peak");
    (census, tracked)
}
#[test]
#[ignore = "process-global simplex allocator audit; run with --test-threads=1"]
fn simplex_primal_and_dual_interruptions_release_dense_owners_and_restart_within_one_plan() {
    let _guard = crate::instrumentation::lock_action_observation();
    for infeasible in [false, true] {
        assert_eq!(audit(infeasible, 1), audit(infeasible, 8));
    }
}
