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
//!
//! The categorical FE certificate (shared by this route, retained CG and
//! MAP) forms each level mean as the first member's value plus a compensated
//! sum of weighted differences from it. When that member is a zero-outcome
//! row with a residual of order one, the positive rows' residuals (about
//! 1e-18) vanish in their differences and the mean carries a rounding error
//! of order one ulp of that residual. Against the rectifier's weighted
//! residual norm (about 1e-8) this is of the order of the 1e-8 certificate
//! tolerance, so whether an accurate residual passes depends on its last bits
//! (seeded census seed 9: exact relative moments below 2.8e-10, computed
//! 1.06e-8; MAP corrections are below one ulp and leave every bit unchanged
//! for 16,000 sweeps). An Auto step that misses a certificate therefore takes
//! no MAP continuation and is re-executed as retained CG (`retry.rs`), the
//! public detector solver, whose own iterates certified that residual.

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
