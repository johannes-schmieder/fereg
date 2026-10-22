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
        let mut state = GateState::new();
        state.begin(demand, width, 10, final_phase);
        assert_eq!(state.checkpoint(16, 100, true).unwrap(), expected);
        assert!(state.terminal);
        assert!(!state.checkpoint(32, 100, true).unwrap());
    }
}

#[test]
fn owner_episode_eligibility_correction_and_unknowns() {
    let mut state = GateState::new();
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
        let mut state = GateState::new();
        state.begin(demand, 1, 10, demand == Demand::Final);
        assert!(!state.checkpoint(16, 100, true).unwrap());
        assert!(state.terminal);
    }
    let mut state = GateState::new();
    state.correction().unwrap();
    state.begin(Demand::Packed { regressors: 3 }, 4, 4, false);
    assert!(!state.checkpoint(16, 100, true).unwrap());
    assert_eq!(
        GateState::shell_bytes(),
        std::mem::size_of::<GateState>() as u128
    );
}
