use crate::options::ThreadConfig;
use crate::protocol::{
    ACTION_PROTOCOL_VERSION, ADO_VERSION, MINIMUM_STATA_MAJOR, MODEL_WIRE_VERSION,
    OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION, SPI_MAJOR, SPI_MINOR,
};

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
    pub cpu_dispatch_path: String,
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
            cpu_dispatch_path: cpu_dispatch_path(),
            available_threads: ThreadConfig::available(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorInput {
    pub stata_version: String,
    pub plugin_filename: String,
    pub configured_threads: usize,
    pub ado_plugin_compatible: bool,
    pub native_selftest_passed: bool,
    pub dependency_audit_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub build: BuildInfo,
    pub input: DoctorInput,
}

impl DoctorReport {
    #[must_use]
    pub fn render(&self) -> String {
        let build = &self.build;
        let input = &self.input;
        let mut rendered = [
            format!("fereg.ado_version={}", build.ado_version),
            format!("fereg.plugin_version={}", build.plugin_version),
            format!(
                "fereg.action_protocol={}.{}",
                build.action_protocol_major, build.action_protocol_minor
            ),
            format!(
                "fereg.model_wire={}.{}",
                build.model_wire_major, build.model_wire_minor
            ),
            format!(
                "fereg.ols_result_wire={}.{}",
                build.ols_result_wire_major, build.ols_result_wire_minor
            ),
            format!("fereg.git_commit={}", build.git_commit),
            format!("fereg.build_date={}", build.build_date),
            format!("fereg.os={}", build.operating_system),
            format!("fereg.arch={}", build.architecture),
            format!("fereg.stata_version={}", input.stata_version),
            format!(
                "fereg.plugin_abi={}.{}",
                build.plugin_abi_major, build.plugin_abi_minor
            ),
            format!("fereg.cpu_dispatch={}", build.cpu_dispatch_path),
            format!("fereg.available_threads={}", build.available_threads),
            format!("fereg.configured_threads={}", input.configured_threads),
            format!("fereg.plugin_filename={}", input.plugin_filename),
            format!(
                "fereg.ado_plugin_compatible={}",
                yes_no(input.ado_plugin_compatible)
            ),
            format!(
                "fereg.native_selftest={}",
                if input.native_selftest_passed {
                    "pass"
                } else {
                    "fail"
                }
            ),
            format!("fereg.dependency_audit={}", input.dependency_audit_status),
            format!("fereg.minimum_stata={MINIMUM_STATA_MAJOR}"),
        ]
        .join("\n");
        rendered.push('\n');
        rendered
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn cpu_dispatch_path() -> String {
    if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") {
        "x86-avx2-fma".to_owned()
    } else if std::is_x86_feature_detected!("sse2") {
        "x86-sse2".to_owned()
    } else {
        "x86-baseline".to_owned()
    }
}

#[cfg(target_arch = "aarch64")]
fn cpu_dispatch_path() -> String {
    "aarch64-neon-baseline".to_owned()
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
fn cpu_dispatch_path() -> String {
    "portable-baseline".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_contains_machine_readable_fields() {
        let report = DoctorReport {
            build: BuildInfo::current(),
            input: DoctorInput {
                stata_version: "18.0".to_owned(),
                plugin_filename: "fereg_linux.plugin".to_owned(),
                configured_threads: 8,
                ado_plugin_compatible: true,
                native_selftest_passed: true,
                dependency_audit_status: "not-run".to_owned(),
            },
        }
        .render();
        assert!(report.contains("fereg.action_protocol=1.1"));
        assert!(report.contains("fereg.model_wire=2.1"));
        assert!(report.contains("fereg.ols_result_wire=1.1"));
        assert!(!report.contains("fereg.model_protocol="));
        assert!(report.contains("fereg.native_selftest=pass"));
    }
}
