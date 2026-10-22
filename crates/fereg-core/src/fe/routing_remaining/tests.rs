use super::*;

const TOLERANCE: f64 = 1e-8;

fn state(mode: Mode, demand: Demand, width: usize, stride: usize) -> State {
    let mut value = State::new(mode);
    value.trace = false;
    value.begin(demand, width, stride, TOLERANCE, false);
    value
}

fn schedule(value: &State, expected: &[(usize, u128, Option<Work>)], history_mode: usize) {
    let (groups, len, mode) = value.future_groups().unwrap();
    assert_eq!(len, expected.len());
    assert_eq!(mode, history_mode);
    for (group, &(width, actions, work)) in groups[..len].iter().zip(expected) {
        assert_eq!(
            (group.width, group.actions, group.work),
            (width, actions, work)
        );
        assert_eq!(group.stride, value.stride);
    }
}

fn observe(
    value: &mut State,
    age: usize,
    active: &[f64],
    remaining: usize,
    mut proxy: impl FnMut(usize) -> Result<f64>,
) -> Result<bool> {
    value.observe(
        age,
        active,
        TOLERANCE,
        remaining,
        [100, 10, 10, 10],
        4,
        false,
        false,
        &mut proxy,
        || Ok(0),
    )
}

#[test]
fn packed_and_low_schedules_count_actual_future_actions() {
    let cases = [
        (Demand::Packed { regressors: 7 }, 8, 8, vec![(1, 1, None)]),
        (Demand::Packed { regressors: 0 }, 1, 1, vec![(1, 1, None)]),
        (
            Demand::Outcome {
                regressors: 23,
                block: 10,
            },
            1,
            10,
            vec![(10, 2, None), (3, 1, None), (1, 1, None)],
        ),
        (
            Demand::Outcome {
                regressors: 0,
                block: 10,
            },
            1,
            10,
            vec![(1, 1, None)],
        ),
        (
            Demand::Regressors {
                start: 0,
                end: 10,
                total: 23,
                block: 10,
            },
            10,
            10,
            vec![(10, 1, None), (3, 1, None), (1, 1, None)],
        ),
        (
            Demand::Regressors {
                start: 10,
                end: 20,
                total: 23,
                block: 10,
            },
            10,
            10,
            vec![(3, 1, None), (1, 1, None)],
        ),
        (
            Demand::Regressors {
                start: 20,
                end: 23,
                total: 23,
                block: 10,
            },
            3,
            10,
            vec![(1, 1, None)],
        ),
        (Demand::Final, 1, 10, vec![]),
        (Demand::OneOff, 4, 10, vec![]),
    ];
    for (demand, width, stride, expected) in cases {
        let value = state(Mode::Never, demand, width, stride);
        schedule(&value, &expected, 0);
    }
}

#[test]
fn zero_step_completion_is_history_but_current_action_is_not() {
    let mut value = state(
        Mode::Never,
        Demand::Regressors {
            start: 0,
            end: 3,
            total: 11,
            block: 3,
        },
        3,
        3,
    );
    assert!(value.history.is_none());
    value.complete(); // Initially certified: three original columns, zero executed steps.
    let first = value.history.unwrap();
    assert_eq!((first.actions, first.original_width), (1, 3));
    assert_eq!(first.work(), Ok(Work { h: 0, d: 0 }));
    value.begin(
        Demand::Regressors {
            start: 3,
            end: 6,
            total: 11,
            block: 3,
        },
        3,
        3,
        TOLERANCE,
        false,
    );
    value.step(3);
    value.step(1);
    // The running action must not improve its own prediction of the next block.
    schedule(
        &value,
        &[
            (3, 1, Some(Work { h: 0, d: 0 })),
            (2, 1, None),
            (1, 1, None),
        ],
        2,
    );
    assert_eq!(value.history.unwrap().actions, 1);
    value.complete();
    let completed = value.history.unwrap();
    assert_eq!((completed.actions, completed.original_width), (2, 6));
    assert_eq!((completed.counts.steps, completed.counts.live), (2, 4));
    assert_eq!(completed.work(), Ok(Work { h: 1, d: 2 }));
}

#[test]
fn compatible_full_blocks_use_history_while_final_has_no_positive_credit() {
    let mut value = state(
        Mode::Never,
        Demand::Regressors {
            start: 0,
            end: 3,
            total: 9,
            block: 3,
        },
        3,
        5,
    );
    for live in [3, 1, 1] {
        value.step(live);
    }
    value.complete();
    value.begin(
        Demand::Regressors {
            start: 3,
            end: 6,
            total: 9,
            block: 3,
        },
        3,
        5,
        TOLERANCE,
        false,
    );
    schedule(
        &value,
        &[(3, 1, Some(Work { h: 3, d: 5 })), (1, 1, None)],
        1,
    );
    // Caller failure omits complete: the next begin discards pending counts.
    value.step(3);
    value.begin(
        Demand::Regressors {
            start: 6,
            end: 9,
            total: 9,
            block: 3,
        },
        3,
        5,
        TOLERANCE,
        false,
    );
    assert_eq!(value.history.unwrap().actions, 1);
    assert_eq!(value.history.unwrap().work(), Ok(Work { h: 3, d: 5 }));
    assert_eq!(value.counts.unwrap().steps, 0);
    schedule(&value, &[(1, 1, None)], 0);
}

#[test]
fn accuracy_keys_and_completed_width_changes_reset_compatible_history() {
    let demand = Demand::Regressors {
        start: 0,
        end: 3,
        total: 5,
        block: 3,
    };
    let mut value = state(Mode::Never, demand, 3, 3);
    value.step(3);
    value.complete();
    value.begin(
        Demand::Regressors {
            start: 3,
            end: 5,
            total: 5,
            block: 3,
        },
        2,
        3,
        TOLERANCE,
        false,
    );
    value.complete();
    let short = value.history.unwrap();
    assert_eq!(
        (short.width, short.actions, short.original_width),
        (2, 1, 2)
    );
    assert_eq!(short.work(), Ok(Work { h: 0, d: 0 }));
    value.begin(demand, 3, 3, TOLERANCE / 2.0, false);
    assert!(value.history.is_none());
    value.complete();
    assert!(value.history.is_some());
    value.begin(Demand::Final, 1, 3, TOLERANCE / 2.0, true);
    assert!(value.history.is_none());
    schedule(&value, &[], 0);
    value.complete();
    assert!(value.history.is_none());
}

#[test]
fn invalid_schedules_and_unrepresentable_history_fail_closed() {
    for demand in [
        Demand::Outcome {
            regressors: 5,
            block: 0,
        },
        Demand::Regressors {
            start: 0,
            end: 1,
            total: 5,
            block: 0,
        },
        Demand::Regressors {
            start: 2,
            end: 2,
            total: 5,
            block: 3,
        },
        Demand::Regressors {
            start: 3,
            end: 2,
            total: 5,
            block: 3,
        },
        Demand::Regressors {
            start: 0,
            end: 6,
            total: 5,
            block: 6,
        },
        Demand::Regressors {
            start: 0,
            end: 4,
            total: 5,
            block: 3,
        },
    ] {
        let value = state(Mode::Never, demand, 1, 5);
        assert!(matches!(value.future_groups(), Err(Unknown::InvalidInput)));
    }
    let mut value = state(
        Mode::Never,
        Demand::Regressors {
            start: 0,
            end: 1,
            total: 3,
            block: 1,
        },
        1,
        1,
    );
    value.complete();
    value.counts = Some(Counts {
        steps: u128::MAX,
        live: u128::MAX,
    });
    value.step(1);
    assert!(value.counts.is_none());
    value.complete();
    assert!(value.history.is_none());
    let mut history = History {
        width: 1,
        actions: 1,
        original_width: 1,
        counts: Counts {
            steps: u128::MAX,
            live: u128::MAX,
        },
    };
    assert!(history.add(Counts { steps: 1, live: 1 }).is_none());
    assert_eq!(
        (
            history.actions,
            history.original_width,
            history.counts.steps
        ),
        (1, 1, u128::MAX)
    );
    // Invalid raw exposure must not become compatible credit through averaging.
    value.history = Some(History {
        width: 1,
        actions: 1,
        original_width: 1,
        counts: Counts { steps: 2, live: 1 },
    });
    assert!(matches!(value.future_groups(), Err(Unknown::InvalidInput)));
}

#[test]
fn optional_bank_admission_refuses_before_allocation_and_charges_actual_capacity() {
    let mut value = state(Mode::Never, Demand::Packed { regressors: 2 }, 3, 3);
    assert_eq!(
        value
            .prepare_observations(Some(FeSolverMemoryBudget {
                execution_bytes: 100,
                usable_bytes: 147,
            }))
            .unwrap(),
        0
    );
    assert_eq!(value.logs.capacity(), 0);
    assert!(!value.observes(0));
    let bytes = value
        .prepare_observations(Some(FeSolverMemoryBudget {
            execution_bytes: 100,
            usable_bytes: 4096,
        }))
        .unwrap();
    assert_eq!(bytes, value.logs.capacity() as u128 * 16);
    assert!((48..=3996).contains(&bytes));
    assert_eq!(value.logs.len(), 3);
    assert!(value.logs.iter().flatten().all(|sample| sample.is_nan()));
    assert!(value.observes(0));
    value.begin(Demand::Final, 1, 3, TOLERANCE / 2.0, true);
    assert_eq!(value.logs.capacity(), 0);
    assert_eq!(value.last_age, None);
    assert_eq!(
        value
            .prepare_observations(Some(FeSolverMemoryBudget {
                execution_bytes: u128::MAX,
                usable_bytes: u128::MAX,
            }))
            .unwrap(),
        0
    );
}

#[test]
fn observation_cadence_reads_only_live_proxies_and_preserves_bank_chronology() {
    let mut value = state(Mode::Never, Demand::Packed { regressors: 1 }, 2, 2);
    value.prepare_observations(None).unwrap();
    let mut visits = Vec::new();
    for (age, proxy) in [(0, 64.0), (1, 32.0), (8, 16.0), (16, 4.0), (24, 1.0)] {
        assert!(!observe(&mut value, age, &[1.0, 0.0], 100, |rhs| {
            visits.push((age, rhs));
            Ok(proxy)
        })
        .unwrap());
    }
    assert_eq!(visits, vec![(0, 0), (8, 0), (16, 0), (24, 0)]);
    assert_eq!(value.logs[0], [4.0_f64.ln(), 0.0]);
    assert!(value.logs[1].iter().all(|sample| sample.is_nan()));
    assert_eq!(value.last_age, Some(24));
}

#[test]
fn correction_requires_fresh_samples_without_resetting_counted_work() {
    let mut value = state(Mode::Never, Demand::Packed { regressors: 0 }, 1, 1);
    value.prepare_observations(None).unwrap();
    for (age, proxy) in [(0, 64.0), (8, 16.0), (16, 4.0)] {
        observe(&mut value, age, &[1.0], 100, |_| Ok(proxy)).unwrap();
    }
    value.step(1);
    value.correction().unwrap();
    assert_eq!(value.epoch, 1);
    assert_eq!(value.last_age, None);
    assert!(value.logs[0].iter().all(|sample| sample.is_nan()));
    assert_eq!(
        (value.counts.unwrap().steps, value.counts.unwrap().live),
        (1, 1)
    );
    observe(&mut value, 0, &[1.0], 100, |_| Ok(9.0)).unwrap();
    assert!(value.logs[0][0].is_nan());
    assert_eq!(value.logs[0][1], 9.0_f64.ln());
    observe(&mut value, 8, &[1.0], 100, |_| Ok(3.0)).unwrap();
    assert_eq!(value.logs[0], [9.0_f64.ln(), 3.0_f64.ln()]);
    observe(&mut value, 16, &[1.0], 100, |_| Ok(1.0)).unwrap();
    assert_eq!(value.logs[0], [3.0_f64.ln(), 0.0]);
    assert_eq!(value.counts.unwrap().steps, 1);
}

#[test]
fn force_controls_intervene_at_exact_local_age_and_never_controls_do_not() {
    for (mode, expected_age) in [
        (Mode::Never, None),
        (Mode::Force(16), Some(16)),
        (Mode::Force(32), Some(32)),
    ] {
        let mut value = state(mode, Demand::Packed { regressors: 0 }, 1, 1);
        value.prepare_observations(None).unwrap();
        let mut built = Vec::new();
        for age in [0, 8, 16, 24, 32, 40] {
            if observe(&mut value, age, &[1.0], 100, |_| Ok(2.0)).unwrap() {
                built.push(age);
            }
        }
        assert_eq!(built, expected_age.into_iter().collect::<Vec<_>>());
        if expected_age.is_some() {
            assert_eq!(value.logs.capacity(), 0);
            assert_eq!(value.last_age, None);
        }
    }
}

#[test]
fn unknown_progress_does_not_gain_predictive_credit_but_can_be_a_forced_label() {
    for mode in [Mode::Predict, Mode::Never, Mode::Force(16)] {
        let mut value = state(mode, Demand::Packed { regressors: 0 }, 1, 1);
        value.prepare_observations(None).unwrap();
        // The negative underflow sentinel is live but has no valid log history.
        let build = observe(&mut value, 16, &[-1.0], 100, |_| {
            panic!("negative sentinel must not be read as a positive proxy")
        })
        .unwrap();
        assert_eq!(build, mode == Mode::Force(16));
    }
    let mut forced = state(Mode::Force(16), Demand::Packed { regressors: 0 }, 1, 1);
    forced.prepare_observations(None).unwrap();
    // Missing/nonfinite forecast labels remain diagnostic observations, not support.
    assert!(observe(&mut forced, 16, &[1.0], 100, |_| Ok(f64::NAN)).unwrap());
}

#[test]
fn forced_controls_still_require_live_activity_and_remaining_work() {
    for (activity, remaining) in [(0.0, 100), (f64::NAN, 100), (f64::INFINITY, 100), (1.0, 0)] {
        let mut value = state(Mode::Force(16), Demand::Packed { regressors: 0 }, 1, 1);
        value.prepare_observations(None).unwrap();
        assert!(!observe(&mut value, 16, &[activity], remaining, |_| Ok(2.0)).unwrap());
    }
}

#[test]
fn proxy_failures_propagate_instead_of_becoming_a_forced_install() {
    let mut value = state(Mode::Force(16), Demand::Packed { regressors: 0 }, 1, 1);
    value.prepare_observations(None).unwrap();
    let failure = observe(&mut value, 16, &[1.0], 100, |_| {
        Err(allocation("synthetic proxy failure"))
    });
    assert!(failure.is_err());
}
