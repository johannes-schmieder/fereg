use crate::options::ThreadConfig;
use crate::protocol::{
    ADO_VERSION, MINIMUM_STATA_MAJOR, PLUGIN_VERSION, PROTOCOL_VERSION, SPI_MAJOR, SPI_MINOR,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub ado_version: &'static str,
    pub plugin_version: &'static str,
    pub protocol_major: u16,
    pub protocol_minor: u16,
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
            protocol_major: PROTOCOL_VERSION.major,
            protocol_minor: PROTOCOL_VERSION.minor,
            git_commit: env!("STFIXEST_GIT_COMMIT"),
            build_date: env!("STFIXEST_BUILD_DATE"),
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
            format!("stfixest.ado_version={}", build.ado_version),
            format!("stfixest.plugin_version={}", build.plugin_version),
            format!(
                "stfixest.model_protocol={}.{}",
                build.protocol_major, build.protocol_minor
            ),
            format!("stfixest.git_commit={}", build.git_commit),
            format!("stfixest.build_date={}", build.build_date),
            format!("stfixest.os={}", build.operating_system),
            format!("stfixest.arch={}", build.architecture),
            format!("stfixest.stata_version={}", input.stata_version),
            format!(
                "stfixest.plugin_abi={}.{}",
                build.plugin_abi_major, build.plugin_abi_minor
            ),
            format!("stfixest.cpu_dispatch={}", build.cpu_dispatch_path),
            format!("stfixest.available_threads={}", build.available_threads),
            format!("stfixest.configured_threads={}", input.configured_threads),
            format!("stfixest.plugin_filename={}", input.plugin_filename),
            format!(
                "stfixest.ado_plugin_compatible={}",
                yes_no(input.ado_plugin_compatible)
            ),
            format!(
                "stfixest.native_selftest={}",
                if input.native_selftest_passed { "pass" } else { "fail" }
            ),
            format!(
                "stfixest.dependency_audit={}",
                input.dependency_audit_status
            ),
            format!("stfixest.minimum_stata={MINIMUM_STATA_MAJOR}"),
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
                plugin_filename: "stfixest_linux.plugin".to_owned(),
                configured_threads: 8,
                ado_plugin_compatible: true,
                native_selftest_passed: true,
                dependency_audit_status: "not-run".to_owned(),
            },
        }
        .render();
        assert!(report.contains("stfixest.model_protocol=1.0"));
        assert!(report.contains("stfixest.native_selftest=pass"));
    }
}
