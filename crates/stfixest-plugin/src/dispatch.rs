use crate::output::OutputSink;
use stfixest_core::diagnostics::{BuildInfo, DoctorInput, DoctorReport};
use stfixest_core::error::{ErrorCode, Result, StFixestError};
use stfixest_core::options::ThreadConfig;
use stfixest_core::protocol::{
    parse_key_values, Handshake, ADO_VERSION, PLUGIN_VERSION, PROTOCOL_VERSION,
};
use stfixest_core::selftest::run_native_selftest;

pub const ACTION_VERSION: &str = "version";
pub const ACTION_DOCTOR: &str = "doctor";
pub const ACTION_SELFTEST: &str = "selftest";
const ACTION_PANIC_TEST: &str = "panic-test";

pub fn dispatch_args(args: &[String], output: &dyn OutputSink) -> Result<()> {
    let (action, tail) = args.split_first().ok_or_else(|| {
        StFixestError::new(
            ErrorCode::Syntax,
            "missing plugin action; expected version, doctor, or selftest",
        )
    })?;
    match action.to_ascii_lowercase().as_str() {
        ACTION_VERSION => version(tail, output),
        ACTION_DOCTOR => doctor(tail, output),
        ACTION_SELFTEST => selftest(tail, output),
        ACTION_PANIC_TEST => panic_test(tail),
        "estimate-ols" | "estimate-ppml" => Err(StFixestError::new(
            ErrorCode::Unsupported,
            "estimator actions are intentionally disabled in the Phase 0-1 architecture build",
        )),
        other => Err(StFixestError::new(
            ErrorCode::Syntax,
            format!("unknown StFixest plugin action {other:?}"),
        )),
    }
}

fn version(args: &[String], output: &dyn OutputSink) -> Result<()> {
    if !args.is_empty() {
        return Err(StFixestError::new(
            ErrorCode::Syntax,
            "version action does not accept arguments",
        ));
    }
    let build = BuildInfo::current();
    output.display(&format!(
        concat!(
            "stfixest.plugin_version={}\n",
            "stfixest.ado_contract={}\n",
            "stfixest.model_protocol={}.{}\n",
            "stfixest.plugin_abi={}.{}\n",
            "stfixest.git_commit={}\n",
            "stfixest.build_date={}\n"
        ),
        PLUGIN_VERSION,
        ADO_VERSION,
        PROTOCOL_VERSION.major,
        PROTOCOL_VERSION.minor,
        build.plugin_abi_major,
        build.plugin_abi_minor,
        build.git_commit,
        build.build_date,
    ));
    Ok(())
}

fn doctor(args: &[String], output: &dyn OutputSink) -> Result<()> {
    let values = parse_key_values(args)?;
    let handshake = Handshake::from_key_values(&values)?;
    handshake.validate()?;
    let configured_threads = requested_threads(&values)?;
    let report = run_native_selftest(configured_threads);
    report.require_pass()?;
    let doctor = DoctorReport {
        build: BuildInfo::current(),
        input: DoctorInput {
            stata_version: values
                .get("stata")
                .cloned()
                .unwrap_or_else(|| "unknown".to_owned()),
            plugin_filename: values
                .get("plugin_file")
                .cloned()
                .unwrap_or_else(|| "unknown".to_owned()),
            configured_threads,
            ado_plugin_compatible: true,
            native_selftest_passed: report.passed(),
            dependency_audit_status: values
                .get("dependency_audit")
                .cloned()
                .unwrap_or_else(|| "build-time-required".to_owned()),
        },
    };
    output.display(&doctor.render());
    Ok(())
}

fn selftest(args: &[String], output: &dyn OutputSink) -> Result<()> {
    let values = parse_key_values(args)?;
    if values.contains_key("ado") {
        Handshake::from_key_values(&values)?.validate()?;
    }
    let threads = requested_threads(&values)?;
    let report = run_native_selftest(threads);
    output.display(&report.render());
    report.require_pass()
}

fn requested_threads(values: &std::collections::BTreeMap<String, String>) -> Result<usize> {
    let requested = values
        .get("threads")
        .map(|value| {
            value.parse::<usize>().map_err(|_| {
                StFixestError::new(
                    ErrorCode::Syntax,
                    format!("threads must be a nonnegative integer; received {value:?}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(0);
    Ok(ThreadConfig::from_requested(requested, true)?.resolved())
}

fn panic_test(args: &[String]) -> Result<()> {
    let values = parse_key_values(args)?;
    if values.get("confirm").map(String::as_str) != Some("panic-boundary-test") {
        return Err(StFixestError::new(
            ErrorCode::Unsupported,
            "panic-test requires confirm=panic-boundary-test",
        ));
    }
    panic!("injected StFixest panic for ABI containment test");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BufferSink;

    #[test]
    fn version_is_machine_readable() {
        let sink = BufferSink::default();
        dispatch_args(&["version".to_owned()], &sink).expect("version");
        assert!(sink.displayed().contains("stfixest.model_protocol=1.0"));
    }

    #[test]
    fn estimator_actions_fail_closed() {
        let sink = BufferSink::default();
        let error = dispatch_args(&["estimate-ols".to_owned()], &sink)
            .expect_err("estimator must be disabled");
        assert_eq!(error.code, ErrorCode::Unsupported);
    }

    #[test]
    fn doctor_rejects_protocol_mismatch() {
        let sink = BufferSink::default();
        let args = [
            "doctor".to_owned(),
            "ado=0.1.0-phase01".to_owned(),
            "protocol_major=99".to_owned(),
            "protocol_minor=0".to_owned(),
            "stata=18.0".to_owned(),
        ];
        assert_eq!(
            dispatch_args(&args, &sink).expect_err("must reject").code,
            ErrorCode::ProtocolMismatch
        );
    }
}
