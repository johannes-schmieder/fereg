//! Hidden-adapter developer injections shared by both estimators.
//!
//! These are documented test interfaces (docs/plugin_protocol.md), reachable
//! only from the hidden `_fereg_estimate_*` adapters, never from public
//! `fereg`/`feppml` syntax. They let licensed fixtures exercise the shipped
//! binary's Break, panic, allocation and experimental routing paths.
use crate::model_metadata::syntax;
use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::fe::{CategoricalPrefix, FeSolverKind, RoutingPreset};
use std::io::Write;

/// Displayed, then written to the marker file, when PPML is armed for Break.
pub(crate) const PPML_NATIVE_BREAK_MARKER: &str = "FEPPML_NATIVE_BREAK_ARMED\n";
/// Displayed, then written to the marker file, when OLS is armed for Break.
pub(crate) const OLS_NATIVE_BREAK_MARKER: &str = "FEREG_NATIVE_BREAK_ARMED\n";

pub(crate) struct OlsInjection {
    pub(crate) routing: Option<RoutingPreset>,
    pub(crate) it_coefficients: bool,
    pub(crate) native_break_token: Option<String>,
}

/// Decode the bounded OLS experiment presets without adding public syntax.
pub(crate) fn parse_ols(stage: &str, solver: FeSolverKind, token: &str) -> Result<OlsInjection> {
    let unknown = || syntax(format!("unknown hidden OLS injection stage {stage:?}"));
    let (routing, break_native) = match stage {
        "" => (None, false),
        "breaknative" => (None, true),
        "routing_overlap_break" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            (Some(RoutingPreset::overlap_break()), true)
        }
        "it_coefficients" => {
            if solver != FeSolverKind::IronsTuck {
                return Err(syntax(
                    "hidden coefficient IT injection requires solver(it)",
                ));
            }
            (None, false)
        }
        "routing_portfolio_v1" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            (Some(RoutingPreset::portfolio_v1()), false)
        }
        "routing_prefix_none" | "routing_prefix_map4" | "routing_prefix_it4" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            let prefix = match stage {
                "routing_prefix_none" => CategoricalPrefix::None,
                "routing_prefix_map4" => CategoricalPrefix::Map4,
                _ => CategoricalPrefix::CompactIt4,
            };
            (Some(RoutingPreset::prefix(prefix)), false)
        }
        "routing_diagonal" | "routing_cells" | "routing_direct" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            (
                Some(RoutingPreset::ablation(
                    stage.strip_prefix("routing_").unwrap(),
                )?),
                false,
            )
        }
        _ => {
            let overlap = stage.starts_with("routing_overlap_h");
            let (checkpoint, samples) = stage
                .strip_prefix(if overlap {
                    "routing_overlap_h"
                } else {
                    "routing_h"
                })
                .and_then(|preset| preset.split_once("_k"))
                .ok_or_else(unknown)?;
            let checkpoint = match checkpoint {
                "4" => 4,
                "8" => 8,
                "16" => 16,
                "32" => 32,
                _ => return Err(unknown()),
            };
            let samples = match samples {
                "0" => 0,
                "1" => 1,
                "2" => 2,
                _ => return Err(unknown()),
            };
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            if overlap && checkpoint == 4 {
                return Err(unknown());
            }
            let preset = if overlap {
                RoutingPreset::overlap(checkpoint, samples)?
            } else {
                RoutingPreset::new(checkpoint, samples)?
            };
            (Some(preset), false)
        }
    };
    validate_break_marker_token(break_native, token)?;
    Ok(OlsInjection {
        routing,
        it_coefficients: stage == "it_coefficients",
        native_break_token: break_native.then(|| token.to_owned()),
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portfolio_routing_accepts_only_the_finite_auto_arm_without_break_token() {
        let parsed = parse_ols("routing_portfolio_v1", FeSolverKind::Auto, "").unwrap();
        assert_eq!(parsed.routing, Some(RoutingPreset::portfolio_v1()));
        assert!(!parsed.it_coefficients);
        assert!(parsed.native_break_token.is_none());
        for solver in [
            FeSolverKind::Map,
            FeSolverKind::Lsmr,
            FeSolverKind::Cmg,
            FeSolverKind::CmgSchur,
            FeSolverKind::Cg,
            FeSolverKind::IronsTuck,
            FeSolverKind::Squarem,
        ] {
            assert!(parse_ols("routing_portfolio_v1", solver, "").is_err());
        }
        for stage in [
            "routing_composed_map4",
            "routing_portfolio_v2",
            "routing_portfolio_v1_h8_k1",
            "routing_portfolio_v01",
            "routing_portfolio_map4",
            "ROUTING_PORTFOLIO_V1",
        ] {
            assert!(parse_ols(stage, FeSolverKind::Auto, "").is_err());
        }
        assert!(parse_ols(
            "routing_portfolio_v1",
            FeSolverKind::Auto,
            "0123456789abcdef0123456789abcdef"
        )
        .is_err());
        assert_eq!(
            parse_ols("routing_prefix_map4", FeSolverKind::Auto, "")
                .unwrap()
                .routing,
            Some(RoutingPreset::prefix(CategoricalPrefix::Map4))
        );
    }
}
