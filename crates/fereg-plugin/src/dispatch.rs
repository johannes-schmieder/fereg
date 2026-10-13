use crate::output::OutputSink;
use fereg_core::diagnostics::BuildInfo;
use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::options::ThreadConfig;
use fereg_core::protocol::{parse_key_values, Handshake};
use fereg_core::selftest::run_native_selftest;

pub const ACTION_SELFTEST: &str = "selftest";
const ACTION_PANIC_TEST: &str = "panic-test";

pub fn dispatch_args(args: &[String], output: &dyn OutputSink) -> Result<()> {
    let (action, tail) = args.split_first().ok_or_else(|| {
        FeregError::new(
            ErrorCode::Syntax,
            "missing plugin action; expected selftest",
        )
    })?;
    match action.to_ascii_lowercase().as_str() {
        ACTION_SELFTEST => selftest(tail, output),
        ACTION_PANIC_TEST => panic_test(tail),
        crate::input::ACTION_PREPARE_OLS_TEST => Err(FeregError::new(
            ErrorCode::Unsupported,
            "prepare-ols-test is developer-only and requires a live Stata SPI session",
        )),
        crate::estimate::ACTION_ESTIMATE_OLS => Err(FeregError::new(
            ErrorCode::Unsupported,
            "estimate-ols requires a live Stata SPI session",
        )),
        crate::ppml_estimate::ACTION_ESTIMATE_PPML => Err(FeregError::new(
            ErrorCode::Unsupported,
            "estimate-ppml requires a live Stata SPI session",
        )),
        other => Err(FeregError::new(
            ErrorCode::Syntax,
            format!("unknown fereg plugin action {other:?}"),
        )),
    }
}

fn selftest(args: &[String], output: &dyn OutputSink) -> Result<()> {
    let values = parse_key_values(args)?;
    let verbose = match values.get("verbose").map(String::as_str) {
        None | Some("0") => false,
        Some("1") => true,
        _ => return Err(FeregError::new(ErrorCode::Syntax, "verbose must be 0 or 1")),
    };
    output.display(&BuildInfo::current().render(verbose));
    if verbose {
        for (label, key) in [
            ("stata_version", "stata"),
            ("plugin_filename", "plugin_file"),
        ] {
            if let Some(value) = values.get(key) {
                output.display(&format!("fereg.{label}={value}\n"));
            }
        }
    }
    let compatibility =
        Handshake::from_key_values(&values).and_then(|handshake| handshake.validate());
    output.display(if compatibility.is_ok() {
        "fereg.ado_plugin_compatible=yes\n"
    } else {
        "fereg.ado_plugin_compatible=no\n"
    });
    compatibility?;
    let threads = requested_threads(&values)?;
    output.display(&format!("fereg.configured_threads={threads}\n"));
    let report = run_native_selftest(threads);
    output.display(&report.render(verbose));
    report.require_pass()
}

fn requested_threads(values: &std::collections::BTreeMap<String, String>) -> Result<usize> {
    let requested = values
        .get("threads")
        .map(|value| {
            value.parse::<usize>().map_err(|_| {
                FeregError::new(
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
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "panic-test requires confirm=panic-boundary-test",
        ));
    }
    panic!("injected fereg panic for ABI containment test");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BufferSink;

    fn diagnostic_args(extra: &[&str]) -> Vec<String> {
        [
            "selftest",
            "ado=0.5.0",
            "protocol_major=1",
            "protocol_minor=6",
            "stata=18.0",
            "threads=2",
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn selftest_combines_identity_compatibility_and_checks() {
        let sink = BufferSink::default();
        dispatch_args(&diagnostic_args(&[]), &sink).expect("selftest");
        let output = sink.displayed();
        assert!(output.contains("fereg.plugin_version=0.5.0"));
        assert!(output.contains("fereg.ado_plugin_compatible=yes"));
        assert!(output.contains("fereg.configured_threads=2"));
        assert!(output.contains("fereg.selftest.overall=pass"));
        assert!(!output.contains("fereg.selftest.threads="));
        assert!(!output.contains("fereg.git_commit="));
    }

    #[test]
    fn verbose_selftest_retains_build_and_individual_check_details() {
        let sink = BufferSink::default();
        dispatch_args(
            &diagnostic_args(&["verbose=1", "plugin_file=mock.plugin"]),
            &sink,
        )
        .expect("selftest");
        for field in [
            "action_protocol=1.6",
            "model_wire=2.1",
            "ols_result_wire=1.5",
            "ppml_request_wire=1.8",
            "ppml_result_wire=1.5",
            "plugin_filename=mock.plugin",
            "stata_version=18.0",
            "selftest.threads=pass",
            "git_commit=",
            "cpu_dispatch=",
        ] {
            assert!(
                sink.displayed().contains(&format!("fereg.{field}")),
                "missing {field}"
            );
        }
    }

    #[test]
    fn retired_actions_and_invalid_verbosity_fail() {
        for action in ["version", "doctor"] {
            let sink = BufferSink::default();
            assert_eq!(
                dispatch_args(&[action.to_owned()], &sink).unwrap_err().code,
                ErrorCode::Syntax
            );
            assert!(sink.displayed().is_empty());
        }
        assert_eq!(
            dispatch_args(&diagnostic_args(&["verbose=2"]), &BufferSink::default())
                .unwrap_err()
                .code,
            ErrorCode::Syntax
        );
    }

    #[test]
    fn selftest_requires_a_complete_handshake() {
        let sink = BufferSink::default();
        assert_eq!(
            dispatch_args(&["selftest".to_owned()], &sink)
                .unwrap_err()
                .code,
            ErrorCode::Syntax
        );
        assert!(sink.displayed().contains("fereg.plugin_version="));
        assert!(!sink.displayed().contains("fereg.selftest.overall="));
    }

    #[test]
    fn estimator_actions_require_the_live_spi_boundary() {
        let sink = BufferSink::default();
        for (action, message) in [
            (
                "estimate-ols",
                "estimate-ols requires a live Stata SPI session",
            ),
            (
                "estimate-ppml",
                "estimate-ppml requires a live Stata SPI session",
            ),
        ] {
            let error = dispatch_args(&[action.to_owned()], &sink)
                .expect_err("estimator must require live SPI");
            assert_eq!(error.code, ErrorCode::Unsupported);
            assert_eq!(error.message, message);
        }
    }

    #[test]
    fn selftest_reports_identity_before_rejecting_protocol_mismatch() {
        let sink = BufferSink::default();
        let args = [
            "selftest".to_owned(),
            "ado=0.5.0".to_owned(),
            "protocol_major=99".to_owned(),
            "protocol_minor=0".to_owned(),
            "stata=18.0".to_owned(),
        ];
        assert_eq!(
            dispatch_args(&args, &sink).expect_err("must reject").code,
            ErrorCode::ProtocolMismatch
        );
        assert!(sink.displayed().contains("fereg.plugin_version=0.5.0"));
        assert!(sink.displayed().contains("fereg.ado_plugin_compatible=no"));
        assert!(!sink.displayed().contains("fereg.selftest.overall="));
    }
}
