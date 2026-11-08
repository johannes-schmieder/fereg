//! fedemean: weighted projection of numerical columns onto high-dimensional
//! fixed-effect designs ("demeaning" in the general sense of FE
//! residualization).
//!
//! Given a categorical design, positive observation weights and columns,
//! a [`Projector`] returns each column's residual from the weighted
//! least-squares projection onto the design's span, certified by the weighted
//! FE moments of the residual ([`Certificate`]), and optionally the FE
//! coefficients of the removed fit. A projector is prepared once per design
//! and reused across columns and weight changes.
//!
//! The library separates three choices: the reduction of the problem
//! ([`Reduction`]), the iterative method ([`Method`]) and its preconditioner
//! ([`Preconditioner`]). Only combinations this build implements are
//! expressible; the enums grow as routes are added.
//!
//! # Capability of this release
//! - Categorical intercept designs with two to five effects and positive
//!   weights.
//! - Exact global elimination of the effect with the most levels, lockstep
//!   PCG over up to eight columns on the reduced (Schur) system with the
//!   Jacobi preconditioner (the default) or block-Jacobi over the retained
//!   effects ([`Preconditioner::BlockJacobi`]): exact grounded Cholesky
//!   solves of small blocks and Jacobi on large ones.
//! - [`Fallback::Escalate`]: a batch whose Jacobi recurrence stalls or
//!   reaches the iteration ceiling continues with block-Jacobi from its
//!   current iterate ([`Route::escalations`]).
//! - Not yet: CMG local solves ([`LocalSolver::Cmg`]), the full formulation
//!   without elimination, LSMR, Schwarz, varying slopes, more than five
//!   effects. Designs outside the route are declined ([`Outcome::Declined`])
//!   for the caller's own solvers.
//!
//! Projections run on the caller's current rayon pool.

#![forbid(unsafe_code)]

mod certificate;
#[cfg(feature = "cmg")]
pub(crate) mod cmg_local;
mod design;
mod elimination;
mod error;
mod projector;
mod reduce;
mod weights;

pub use certificate::Certificate;
pub use design::{Design, Effect, Members};
pub use error::Error;
pub use projector::{
    BlockSpec, BlockUsed, Choice, CoefficientLayout, CoefficientsMut, ColumnReport, ColumnStatus,
    ColumnsMut, Control, Decline, Facts, Fallback, Gram, InputState, LocalSolver, LocalUsed,
    Method, MethodUsed, Outcome, Preconditioner, PreconditionerUsed, Projector, Reduction,
    ReductionUsed, Report, Request, Route, Settings,
};
pub use weights::Weights;
