use crate::options::ThreadConfig;
use crate::protocol::{
    ACTION_PROTOCOL_VERSION, ADO_VERSION, MINIMUM_STATA_MAJOR, MODEL_WIRE_VERSION,
    OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION, PPML_REQUEST_WIRE_VERSION, PPML_RESULT_WIRE_VERSION,
    SPI_MAJOR, SPI_MINOR,
};
use std::fs::OpenOptions;
use std::io::Write;

/// Developer solver tracing (`FEREG_SOLVER_TRACE`), read at each use.
#[must_use]
pub fn solver_trace_enabled() -> bool {
    std::env::var_os("FEREG_SOLVER_TRACE").is_some()
}

/// Developer phase timing (`FEREG_PHASE_TIMING`), read at each use.
#[must_use]
pub fn phase_timing_enabled() -> bool {
    std::env::var_os("FEREG_PHASE_TIMING").is_some()
}

/// Write one solver-trace line to standard error and, when
/// `FEREG_SOLVER_TRACE_FILE` names a file, append it there as well.
pub(crate) fn emit_solver_trace(line: &str) {
    eprintln!("{line}");
    let Some(path) = std::env::var_os("FEREG_SOLVER_TRACE_FILE") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub ado_version: &'static str,
    pub plugin_version: &'static str,
    pub action_protocol_major: u16,
    pub action_protocol_minor: u16,
    pub model_wire_major: u16,
    pub model_wire_minor: u16,
    pub ols_result_wire_major: u16,
    pub ols_result_wire_minor: u16,
    pub git_commit: &'static str,
    pub build_date: &'static str,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub plugin_abi_major: u16,
    pub plugin_abi_minor: u16,
    pub cpu_dispatch_path: &'static str,
    pub available_threads: usize,
}

impl BuildInfo {
    #[must_use]
    pub fn current() -> Self {
        Self {
            ado_version: ADO_VERSION,
            plugin_version: PLUGIN_VERSION,
            action_protocol_major: ACTION_PROTOCOL_VERSION.major,
            action_protocol_minor: ACTION_PROTOCOL_VERSION.minor,
            model_wire_major: MODEL_WIRE_VERSION.major,
            model_wire_minor: MODEL_WIRE_VERSION.minor,
            ols_result_wire_major: OLS_RESULT_WIRE_VERSION.major,
            ols_result_wire_minor: OLS_RESULT_WIRE_VERSION.minor,
            git_commit: env!("FEREG_GIT_COMMIT"),
            build_date: env!("FEREG_BUILD_DATE"),
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            plugin_abi_major: SPI_MAJOR,
            plugin_abi_minor: SPI_MINOR,
            cpu_dispatch_path: crate::kernels::active_name(),
            available_threads: ThreadConfig::available(),
        }
    }

    /// Plugin identity is available even when a subsequent installation check fails.
    #[must_use]
    pub fn render(&self, verbose: bool) -> String {
        let mut output = format!("fereg.plugin_version={}\n", self.plugin_version);
        if verbose {
            output.push_str(&format!(
                concat!(
                    "fereg.ado_contract={}\n",
                    "fereg.action_protocol={}.{}\n",
                    "fereg.model_wire={}.{}\n",
                    "fereg.ols_result_wire={}.{}\n",
                    "fereg.ppml_request_wire={}.{}\n",
                    "fereg.ppml_result_wire={}.{}\n",
                    "fereg.plugin_abi={}.{}\n",
                    "fereg.git_commit={}\n",
                    "fereg.build_date={}\n",
                    "fereg.os={}\n",
                    "fereg.arch={}\n",
                    "fereg.cpu_dispatch={}\n",
                    "fereg.available_threads={}\n",
                    "fereg.minimum_stata={}\n",
                    "fereg.dependency_audit=build-time-required\n"
                ),
                self.ado_version,
                self.action_protocol_major,
                self.action_protocol_minor,
                self.model_wire_major,
                self.model_wire_minor,
                self.ols_result_wire_major,
                self.ols_result_wire_minor,
                PPML_REQUEST_WIRE_VERSION.major,
                PPML_REQUEST_WIRE_VERSION.minor,
                PPML_RESULT_WIRE_VERSION.major,
                PPML_RESULT_WIRE_VERSION.minor,
                self.plugin_abi_major,
                self.plugin_abi_minor,
                self.git_commit,
                self.build_date,
                self.operating_system,
                self.architecture,
                self.cpu_dispatch_path,
                self.available_threads,
                MINIMUM_STATA_MAJOR,
            ));
        }
        output
    }
}
