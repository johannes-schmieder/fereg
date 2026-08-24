#![forbid(unsafe_code)]

pub mod explicit_dummy;
pub mod explicit_ols;

use fereg_plugin::{dispatch_args, BufferSink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchCapture {
    pub return_code: i32,
    pub display: String,
    pub error: String,
}

#[must_use]
pub fn run_action(args: &[&str]) -> DispatchCapture {
    let sink = BufferSink::default();
    let owned: Vec<String> = args.iter().map(|value| (*value).to_owned()).collect();
    let result = dispatch_args(&owned, &sink);
    DispatchCapture {
        return_code: result
            .as_ref()
            .map_or_else(|error| error.return_code(), |_| 0),
        display: sink.displayed(),
        error: result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_else(|| sink.errors()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_action_is_available_without_stata() {
        let capture = run_action(&["version"]);
        assert_eq!(capture.return_code, 0);
        assert!(capture.display.contains("fereg.plugin_version="));
    }

    #[test]
    fn doctor_runs_with_32_requested_threads() {
        let capture = run_action(&[
            "doctor",
            "ado=0.9.0",
            "protocol_major=1",
            "protocol_minor=0",
            "stata=18.0",
            "threads=32",
            "plugin_file=mock.plugin",
            "dependency_audit=mock-pass",
        ]);
        assert_eq!(capture.return_code, 0, "{}", capture.error);
        assert!(capture.display.contains("fereg.native_selftest=pass"));
    }

    #[test]
    fn unknown_action_returns_syntax_error() {
        let capture = run_action(&["unknown-action"]);
        assert_eq!(capture.return_code, 198);
    }
}
