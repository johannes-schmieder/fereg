//! Hidden-adapter developer injections shared by both estimators.
//!
//! These are documented test interfaces (docs/plugin_protocol.md), reachable
//! only from the hidden `_fereg_estimate_*` adapters, never from public
//! `fereg`/`feppml` syntax. They never change arithmetic; they let licensed
//! fixtures exercise the shipped binary's Break, panic and allocation paths.
use crate::model_metadata::syntax;
use fereg_core::error::{ErrorCode, FeregError, Result};
use std::io::Write;

/// Displayed, then written to the marker file, when PPML is armed for Break.
pub(crate) const PPML_NATIVE_BREAK_MARKER: &str = "FEPPML_NATIVE_BREAK_ARMED\n";
/// Displayed, then written to the marker file, when OLS is armed for Break.
pub(crate) const OLS_NATIVE_BREAK_MARKER: &str = "FEREG_NATIVE_BREAK_ARMED\n";

/// `break_marker_token` is required exactly when native Break is injected and
/// must be 32 lowercase hexadecimal characters, so it cannot name a path.
pub(crate) fn validate_break_marker_token(expects_marker: bool, token: &str) -> Result<()> {
    let has_marker = !token.is_empty();
    if expects_marker != has_marker {
        return Err(syntax(
            "breaknative requires exactly one hidden break_marker_token",
        ));
    }
    if !token.is_empty()
        && (token.len() != 32
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        return Err(syntax(
            "hidden break_marker_token must be exactly 32 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

pub(crate) fn native_break_marker_path(prefix: &str, token: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-native-break-{token}.marker"))
}

/// Create the marker file the interactive Break controller waits for. It is
/// created exclusively and synced before the controller may send Break.
pub(crate) fn create_native_break_marker(prefix: &str, marker: &str, token: &str) -> Result<()> {
    let path = native_break_marker_path(prefix, token);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| internal(format!("unable to create hidden Break marker: {error}")))?;
    file.write_all(marker.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| internal(format!("unable to commit hidden Break marker: {error}")))
}

fn internal(message: String) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}
