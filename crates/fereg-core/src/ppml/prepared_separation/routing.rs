//! Solver family for admitted weighted separation-detector projections.
//!
//! An admitted detector projection (rectifier recurrence, Mu candidate or a
//! numerical direction's span check) is one certified weighted
//! finite-plus-FE least-squares projection. The public route solves its FE
//! systems with retained-product CG. The hidden `inject=ppml_separation_auto`
//! arm solves them with the OLS two-FE Auto route (diagonal pilot, stall
//! gate, CMG Schur preconditioner on the worker team, aggregated cells) on
//! the detector's own weights, for exactly two categorical intercept effects.
//!
//! Only the solver that reaches the projection's certificates changes: the
//! FE tolerance, the finite score tolerance (`projection_tolerance`), the
//! rank, FE-moment, score and reconstruction certificates, the memory budget,
//! the rectifier's weights, recurrence, iteration limit and every sign and
//! threshold rule are the caller's, unchanged. Projected values may differ
//! from CG's in their low bits within those certificates.
use crate::fe::{BorrowedInterceptFeDesign, FeSolverKind};

/// The FE solver of an admitted detector projection.
pub(super) fn projection_solver(
    auto_projection: bool,
    design: &BorrowedInterceptFeDesign<'_>,
) -> FeSolverKind {
    if auto_projection && design.effects().len() == 2 {
        FeSolverKind::Auto
    } else {
        FeSolverKind::Cg
    }
}
