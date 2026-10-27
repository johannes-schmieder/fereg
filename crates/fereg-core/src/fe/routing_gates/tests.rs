use super::*;

#[test]
fn selected_wire_and_scalar_predicate_agree() {
    let record = include_str!("../../../../../bench/results/two-fe-policy-gates-fit-20261022.json");
    let hex = record
        .split("\"encoded_hex\": \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let wire: Vec<_> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(wire.len(), 128);
    assert_eq!(&wire[..8], b"2FEGATE2");
    assert_eq!(&wire[64..67], &[2, 16, 1]);
    assert!(wire[67..].iter().all(|&v| v == 0));
    for (demand, width, final_phase, expected) in [
        (Demand::Packed { regressors: 3 }, 4, false, true),
        (
            Demand::Outcome {
                regressors: 17,
                block: 10,
            },
            1,
            false,
            true,
        ),
        (Demand::Final, 1, true, false),
    ] {
        let mut state = GateState::new(GateRule::Structural16);
        state.begin(demand, width, 10, final_phase);
        assert_eq!(state.checkpoint(16, 100, true).unwrap(), expected);
        assert!(state.terminal);
        assert!(!state.checkpoint(32, 100, true).unwrap());
    }
}

#[test]
fn owner_episode_eligibility_correction_and_unknowns() {
    let mut state = GateState::new(GateRule::Structural16);
    state.begin(
        Demand::Outcome {
            regressors: 17,
            block: 10,
        },
        1,
        10,
        false,
    );
    assert!(!state.checkpoint(8, 100, true).unwrap());
    assert!(!state.checkpoint(16, 100, false).unwrap());
    assert!(!state.terminal);
    state.begin(Demand::Packed { regressors: 3 }, 4, 4, false);
    assert!(state.checkpoint(16, 100, true).unwrap());
    state.begin(
        Demand::Outcome {
            regressors: 17,
            block: 10,
        },
        1,
        10,
        false,
    );
    assert!(!state.checkpoint(16, 100, true).unwrap());
    for demand in [
        Demand::Final,
        Demand::OneOff,
        Demand::Regressors {
            start: 0,
            end: 10,
            total: 17,
            block: 10,
        },
        Demand::Packed {
            regressors: usize::MAX,
        },
    ] {
        let mut state = GateState::new(GateRule::Structural16);
        state.begin(demand, 1, 10, demand == Demand::Final);
        assert!(!state.checkpoint(16, 100, true).unwrap());
        assert!(state.terminal);
    }
    let mut state = GateState::new(GateRule::Structural16);
    state.correction().unwrap();
    state.begin(Demand::Packed { regressors: 3 }, 4, 4, false);
    assert!(!state.checkpoint(16, 100, true).unwrap());
    assert_eq!(
        GateState::shell_bytes(),
        std::mem::size_of::<GateState>() as u128
    );
}

#[test]
fn stall_rule_grants_early_only_for_stalled_packed_or_outcome_episodes() {
    let rule = GateRule::Stall {
        threshold: STALL_THRESHOLD,
    };
    let packed = |state: &mut GateState| state.begin(Demand::Packed { regressors: 1 }, 2, 2, false);
    let bound = |age: usize| STALL_THRESHOLD.powi(2 * age as i32);
    // A lane at the bound stalls; a faster lane alone would not.
    let mut state = GateState::new(rule);
    packed(&mut state);
    let initial = [8.0, 4.0];
    assert!(!state
        .observe(
            4,
            100,
            &initial,
            &[8.0 * bound(4) * 0.5, 4.0 * bound(4) * 0.5]
        )
        .unwrap());
    assert!(!state.terminal);
    assert!(state
        .observe(
            8,
            100,
            &initial,
            &[8.0 * bound(8) * 0.5, 4.0 * bound(8) * 1.5]
        )
        .unwrap());
    assert!(state.terminal);
    assert!(!state.observe(12, 100, &initial, &initial).unwrap());
    assert!(!state.observe(16, 100, &initial, &initial).unwrap());

    // Completed lanes are ignored; underflowed history leaves the age open.
    let mut state = GateState::new(rule);
    packed(&mut state);
    assert!(!state.observe(4, 100, &[-1.0, 4.0], &[1.0, 4.0]).unwrap());
    assert!(!state.terminal);
    assert!(state.observe(4, 100, &[0.0, 4.0], &[9.0, 4.0]).unwrap());

    // Converging pilots reach local16 and keep the v1 decision there.
    let mut state = GateState::new(rule);
    state.begin(
        Demand::Outcome {
            regressors: 3,
            block: 2,
        },
        1,
        2,
        false,
    );
    for age in [4, 8, 12] {
        assert!(!state
            .observe(age, 100, &[1.0], &[bound(age) * 0.9])
            .unwrap());
    }
    assert!(state.observe(16, 100, &[1.0], &[1e-6]).unwrap());

    // Final and unknown roles, exhausted budgets and other ages never grant.
    for (demand, width, final_phase) in [(Demand::Final, 1, true), (Demand::OneOff, 1, false)] {
        let mut state = GateState::new(rule);
        state.begin(demand, width, 2, final_phase);
        assert!(!state.observe(4, 100, &[1.0], &[1.0]).unwrap());
        assert!(!state.terminal);
    }
    let mut state = GateState::new(rule);
    packed(&mut state);
    assert!(!state.observe(4, 0, &initial, &initial).unwrap());
    assert!(!state.observe(5, 100, &initial, &initial).unwrap());
    assert!(!state.terminal);
}

#[test]
fn structural_rule_observes_only_local16() {
    let mut state = GateState::new(GateRule::Structural16);
    state.begin(Demand::Packed { regressors: 1 }, 2, 2, false);
    for age in [4, 8, 12] {
        assert!(!state.observe(age, 100, &[1.0, 1.0], &[1.0, 1.0]).unwrap());
    }
    assert!(!state.terminal);
    assert!(state.observe(16, 100, &[1.0, 1.0], &[1.0, 1.0]).unwrap());
}
