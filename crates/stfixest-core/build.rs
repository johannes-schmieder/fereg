use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=STFIXEST_BUILD_DATE");
    println!("cargo:rerun-if-env-changed=STFIXEST_GIT_COMMIT");

    let commit = env::var("STFIXEST_GIT_COMMIT").unwrap_or_else(|_| {
        Command::new("git")
            .args(["rev-parse", "--short=12", "HEAD"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    });

    let build_date = env::var("STFIXEST_BUILD_DATE")
        .or_else(|_| env::var("SOURCE_DATE_EPOCH").map(|epoch| format!("epoch:{epoch}")))
        .unwrap_or_else(|_| "unknown".to_owned());

    println!("cargo:rustc-env=STFIXEST_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=STFIXEST_BUILD_DATE={build_date}");
}
