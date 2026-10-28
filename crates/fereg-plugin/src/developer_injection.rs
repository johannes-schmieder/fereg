//! Hidden-adapter developer injections shared by both estimators.
//!
//! These are documented test interfaces (docs/plugin_protocol.md), reachable
//! only from the hidden `_fereg_estimate_*` adapters, never from public
//! `fereg`/`feppml` syntax. They let licensed fixtures exercise the shipped
//! binary's Break, panic, allocation and experimental routing paths.
use crate::model_metadata::syntax;
use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::fe::{CategoricalPrefix, FeSolverKind, RoutingPreset};
use fereg_core::ols::{PreparedOlsCells, PreparedOlsOwnership};
use std::io::Write;

/// Displayed, then written to the marker file, when PPML is armed for Break.
pub(crate) const PPML_NATIVE_BREAK_MARKER: &str = "FEPPML_NATIVE_BREAK_ARMED\n";
/// Displayed, then written to the marker file, when OLS is armed for Break.
pub(crate) const OLS_NATIVE_BREAK_MARKER: &str = "FEREG_NATIVE_BREAK_ARMED\n";

pub(crate) struct OlsInjection {
    pub(crate) routing: Option<RoutingPreset>,
    pub(crate) ownership: PreparedOlsOwnership,
    pub(crate) cells: PreparedOlsCells,
    pub(crate) it_coefficients: bool,
    pub(crate) native_break_token: Option<String>,
    /// The selected hidden many-FE policy is public Auto's own preset; it keeps
    /// public two-FE activation so selected-hidden/public science is exact.
    pub(crate) public_activation: bool,
}

/// Resolve mechanism ownership independently of routing and marker controls.
fn ols_mechanisms(
    stage: &str,
    solver: FeSolverKind,
) -> Result<(PreparedOlsOwnership, PreparedOlsCells)> {
    let ownership = PreparedOlsOwnership {
        skip_unused_moments: matches!(
            stage,
            "ownership_moments"
                | "ownership_both"
                | "cells_eager_moments"
                | "cells_eager_both"
                | "cells_deferred_moments"
                | "cells_deferred_both"
        ),
        retain_projected_scores: matches!(
            stage,
            "ownership_scores"
                | "ownership_both"
                | "cells_eager_scores"
                | "cells_eager_both"
                | "cells_deferred_scores"
                | "cells_deferred_both"
        ),
    };
    let cells = if matches!(
        stage,
        "cells_eager" | "cells_eager_moments" | "cells_eager_scores" | "cells_eager_both"
    ) {
        if solver != FeSolverKind::Auto {
            return Err(syntax("hidden cell injection requires solver(auto)"));
        }
        PreparedOlsCells::Eager
    } else if matches!(
        stage,
        "cells_deferred"
            | "cells_deferred_moments"
            | "cells_deferred_scores"
            | "cells_deferred_both"
    ) {
        if solver != FeSolverKind::Auto {
            return Err(syntax("hidden cell injection requires solver(auto)"));
        }
        PreparedOlsCells::Deferred
    } else {
        PreparedOlsCells::Off
    };
    Ok((ownership, cells))
}

/// Decode the bounded OLS experiment presets without adding public syntax.
pub(crate) fn parse_ols(stage: &str, solver: FeSolverKind, token: &str) -> Result<OlsInjection> {
    let unknown = || syntax(format!("unknown hidden OLS injection stage {stage:?}"));
    let (ownership, cells) = ols_mechanisms(stage, solver)?;
    let (routing, break_native) = match stage {
        // A marker alone observes the public request; it must not select an
        // experimental preset or suppress the public ownership/cell policy.
        "" => (None, !token.is_empty()),
        "ownership_moments" | "ownership_scores" | "ownership_both" => (None, false),
        "cells_eager"
        | "cells_eager_moments"
        | "cells_eager_scores"
        | "cells_eager_both"
        | "cells_deferred"
        | "cells_deferred_moments"
        | "cells_deferred_scores"
        | "cells_deferred_both" => (None, false),
        "breaknative" => (None, true),
        "routing_builder_break" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            (Some(RoutingPreset::builder_break()), true)
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
        "routing_two_gates_v1"
        | "routing_two_gates_v2"
        | "routing_two_remaining_v1"
        | "routing_two_remaining_never"
        | "routing_two_remaining_force16"
        | "routing_two_remaining_force32" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            let preset = match stage {
                "routing_two_gates_v1" => RoutingPreset::gates_two(),
                "routing_two_gates_v2" => RoutingPreset::gates_two_v2(),
                "routing_two_remaining_v1" => RoutingPreset::remaining_two(),
                "routing_two_remaining_force16" => RoutingPreset::remaining_two_control(Some(16))?,
                "routing_two_remaining_force32" => RoutingPreset::remaining_two_control(Some(32))?,
                _ => RoutingPreset::remaining_two_control(None)?,
            };
            (Some(preset), false)
        }
        "routing_many_geometry_v1" | "routing_many_windows_v1" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            let preset = if stage == "routing_many_windows_v1" {
                RoutingPreset::geometry_many_windows()
            } else {
                RoutingPreset::geometry_many()
            };
            (Some(preset), false)
        }
        "routing_incremental_many" | "routing_incremental_two" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            (
                Some(if stage == "routing_incremental_many" {
                    RoutingPreset::incremental_many()
                } else {
                    RoutingPreset::incremental_two()
                }),
                false,
            )
        }
        "routing_portfolio_unseeded" => {
            if solver != FeSolverKind::Auto {
                return Err(syntax("hidden routing injection requires solver(auto)"));
            }
            (Some(RoutingPreset::portfolio_unseeded()), false)
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
            let (checkpoint, samples) = stage
                .strip_prefix("routing_h")
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
            (Some(RoutingPreset::new(checkpoint, samples)?), false)
        }
    };
    validate_break_marker_token(break_native, token)?;
    Ok(OlsInjection {
        routing,
        ownership,
        cells,
        it_coefficients: stage == "it_coefficients",
        native_break_token: break_native.then(|| token.to_owned()),
        // Selected public policies keep public two-FE activation, so the
        // hidden invocation equals uninjected Auto on every design.
        public_activation: PUBLIC_ACTIVATION.contains(&stage),
    })
}

/// Selected public policies keep public two-FE activation, so the hidden
/// invocation equals uninjected Auto on every design. The candidate many-FE
/// windows keep it too, so only many-FE windows differ from public Auto.
const PUBLIC_ACTIVATION: [&str; 3] = [
    "routing_many_geometry_v1",
    "routing_many_windows_v1",
    "routing_two_gates_v2",
];

/// `break_marker_token` is required exactly when a native Break is armed and
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
    fn cell_experiments_are_independent_of_routing_and_reject_unimplemented_modes() {
        for (stage, mode, moments, scores) in [
            ("cells_eager", PreparedOlsCells::Eager, false, false),
            ("cells_eager_moments", PreparedOlsCells::Eager, true, false),
            ("cells_eager_scores", PreparedOlsCells::Eager, false, true),
            ("cells_eager_both", PreparedOlsCells::Eager, true, true),
            ("cells_deferred", PreparedOlsCells::Deferred, false, false),
            (
                "cells_deferred_moments",
                PreparedOlsCells::Deferred,
                true,
                false,
            ),
            (
                "cells_deferred_scores",
                PreparedOlsCells::Deferred,
                false,
                true,
            ),
            (
                "cells_deferred_both",
                PreparedOlsCells::Deferred,
                true,
                true,
            ),
        ] {
            let parsed = parse_ols(stage, FeSolverKind::Auto, "").unwrap();
            assert_eq!(parsed.cells, mode);
            assert_eq!(parsed.ownership.skip_unused_moments, moments);
            assert_eq!(parsed.ownership.retain_projected_scores, scores);
            assert!(parsed.routing.is_none());
            assert!(parse_ols(stage, FeSolverKind::Lsmr, "").is_err());
            assert!(parse_ols(stage, FeSolverKind::Auto, "a").is_err());
        }
        for stage in ["cells_deferred_extra", "cells_eager_extra"] {
            assert!(parse_ols(stage, FeSolverKind::Auto, "").is_err());
        }
    }

    #[test]
    fn ownership_experiments_preserve_solver_and_routing_independence() {
        for solver in [
            FeSolverKind::Auto,
            FeSolverKind::Map,
            FeSolverKind::Lsmr,
            FeSolverKind::Cmg,
        ] {
            for (name, moments, scores) in [
                ("", false, false),
                ("ownership_moments", true, false),
                ("ownership_scores", false, true),
                ("ownership_both", true, true),
            ] {
                let parsed = parse_ols(name, solver, "").unwrap();
                assert_eq!(parsed.ownership.skip_unused_moments, moments);
                assert_eq!(parsed.ownership.retain_projected_scores, scores);
                assert!(parsed.routing.is_none());
                assert_eq!(parsed.cells, PreparedOlsCells::Off);
                assert!(!parsed.it_coefficients);
            }
        }
        assert!(parse_ols("ownership_scores_extra", FeSolverKind::Auto, "").is_err());
        assert!(parse_ols("ownership_both", FeSolverKind::Auto, "a").is_err());
    }

    #[test]
    fn incremental_routing_requires_exact_auto_names_without_break_token() {
        for preset in [
            RoutingPreset::geometry_many(),
            RoutingPreset::geometry_many_windows(),
            RoutingPreset::incremental_many(),
            RoutingPreset::incremental_two(),
            RoutingPreset::gates_two(),
            RoutingPreset::remaining_two(),
            RoutingPreset::remaining_two_control(None).unwrap(),
            RoutingPreset::remaining_two_control(Some(16)).unwrap(),
            RoutingPreset::remaining_two_control(Some(32)).unwrap(),
        ] {
            let name = preset.to_string();
            assert_eq!(
                parse_ols(&name, FeSolverKind::Auto, "").unwrap().routing,
                Some(preset)
            );
            for solver in [
                FeSolverKind::Map,
                FeSolverKind::Lsmr,
                FeSolverKind::Cmg,
                FeSolverKind::CmgSchur,
                FeSolverKind::Cg,
                FeSolverKind::IronsTuck,
                FeSolverKind::Squarem,
            ] {
                assert!(parse_ols(&name, solver, "").is_err());
            }
            assert!(parse_ols(
                &name,
                FeSolverKind::Auto,
                "0123456789abcdef0123456789abcdef"
            )
            .is_err());
            assert!(parse_ols(&format!("{name}_v1"), FeSolverKind::Auto, "").is_err());
        }
    }

    #[test]
    fn portfolio_unseeded_accepts_only_the_exact_private_auto_control() {
        let control = parse_ols("routing_portfolio_unseeded", FeSolverKind::Auto, "").unwrap();
        assert_eq!(control.routing, Some(RoutingPreset::portfolio_unseeded()));
        assert!(!control.it_coefficients);
        assert!(control.native_break_token.is_none());
        for solver in [
            FeSolverKind::Map,
            FeSolverKind::Lsmr,
            FeSolverKind::Cmg,
            FeSolverKind::CmgSchur,
            FeSolverKind::Cg,
            FeSolverKind::IronsTuck,
            FeSolverKind::Squarem,
        ] {
            assert!(parse_ols("routing_portfolio_unseeded", solver, "").is_err());
        }
        for name in [
            "routing_unseeded",
            "routing_portfolio_unseeded_v1",
            "routing_portfolio_unseeded_h16_k1",
            "ROUTING_PORTFOLIO_UNSEEDED",
        ] {
            assert!(parse_ols(name, FeSolverKind::Auto, "").is_err());
        }
        assert!(parse_ols(
            "routing_portfolio_unseeded",
            FeSolverKind::Auto,
            "0123456789abcdef0123456789abcdef"
        )
        .is_err());
    }

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

    #[test]
    fn marker_only_ols_does_not_select_an_injection() {
        let token = "0123456789abcdef0123456789abcdef";
        let plain = parse_ols("", FeSolverKind::Auto, "").unwrap();
        let marked = parse_ols("", FeSolverKind::Auto, token).unwrap();
        assert_eq!(marked.routing, plain.routing);
        assert_eq!(marked.cells, plain.cells);
        assert_eq!(marked.ownership, plain.ownership);
        assert_eq!(marked.it_coefficients, plain.it_coefficients);
        assert!(plain.native_break_token.is_none());
        assert_eq!(marked.native_break_token.as_deref(), Some(token));
        for invalid in ["invalid", "0123456789ABCDEF0123456789ABCDEF", "/tmp/marker"] {
            assert!(parse_ols("", FeSolverKind::Auto, invalid).is_err());
        }
        assert!(parse_ols("ownership_both", FeSolverKind::Auto, token).is_err());
        assert!(parse_ols("breaknative", FeSolverKind::Auto, "").is_err());
    }
}
