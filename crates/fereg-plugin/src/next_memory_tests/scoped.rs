use crate::ffi::allocation_usage;
use fereg_engine::{execution, threads::ThreadTeam, ErrorCode, FeregError};

fn run(team: &ThreadTeam, cancel: bool) {
    // An inline capture makes the runtime's job container independently visible
    // after the work has consumed every numerical owner.
    let payload = std::hint::black_box([23_u8; 16_384]);
    let failure = FeregError::new(ErrorCode::NonConvergence, "scoped host r(430)");
    let result = execution::run(
        team,
        move |poll| {
            let value = std::hint::black_box(payload)[0];
            if poll(()) {
                Ok(value)
            } else {
                Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "worker interrupted",
                ))
            }
        },
        &mut |()| {
            if cancel {
                Err(failure.clone())
            } else {
                Ok(())
            }
        },
    );
    if cancel {
        assert_eq!(result.unwrap_err(), failure);
    } else {
        assert_eq!(result.unwrap(), 23);
    }
}

#[test]
#[ignore = "process-global scoped-job allocator audit; run with --test-threads=1"]
fn completed_scoped_calls_release_job_containers_before_returning_to_the_host() {
    let _guard = crate::instrumentation::lock_action_observation();
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        for cancel in [false, true] {
            for _ in 0..100 {
                run(&team, cancel);
            }
            for call in 0..5000 {
                let before = allocation_usage();
                run(&team, cancel);
                let after = allocation_usage();
                assert!(
                    after.current_bytes <= before.current_bytes,
                    "completed job retained storage: workers={workers}, cancel={cancel}, call={call}, before={before:?}, after={after:?}"
                );
            }
        }
    }
}
