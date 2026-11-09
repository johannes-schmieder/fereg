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
//! - Categorical intercept designs with any number of effects and positive
//!   weights.
//! - Exact global elimination of the effect with the most levels (two to
//!   five effects), lockstep PCG over up to eight columns on the reduced
//!   (Schur) system with the Jacobi preconditioner (the default) or
//!   block-Jacobi over the retained effects
//!   ([`Preconditioner::BlockJacobi`]): exact grounded Cholesky solves of
//!   small blocks; one fixed CMG cycle on large ones (feature `cmg`,
//!   [`LocalSolver::Cmg`]), or Jacobi.
//! - [`Fallback::Escalate`]: an eliminated batch whose Jacobi recurrence
//!   stalls or reaches the iteration ceiling continues with block-Jacobi
//!   from its current iterate ([`Route::escalations`]).
//! - The full formulation ([`Reduction::Full`]; [`Reduction::Auto`] for one
//!   or six or more effects): lockstep PCG on the singular consistent normal
//!   equations, or modified LSMR (feature `schwarz`, [`Method::Lsmr`]), with
//!   Jacobi, no preconditioner or (feature `schwarz`, the full
//!   formulation's automatic choice) one-level additive Schwarz over
//!   factor-pair subdomains ([`Preconditioner::Schwarz`]); Jacobi escalating
//!   to Schwarz when it stalls or a pilot predicts it slow
//!   ([`Fallback::Escalate`]); warm starts
//!   ([`Projector::project_from`]).
//! - Varying slopes ([`Effect::with_slopes`]: an optional intercept and
//!   slope variables per effect, any number of channels: each level's
//!   `C x C` blocks are admitted by memory), certified per level
//!   on the span of its channels:
//!   - two effects: the elimination of the larger effect's level channel
//!     blocks with PCG on the Schur complement, Jacobi on its diagonal
//!     blocks ([`Reduction::EliminateLargest`], the automatic choice);
//!   - the full formulation with PCG or modified LSMR and Jacobi over each
//!     level's channel block (exact inverses; singular blocks act as
//!     pseudo-inverses on their range,
//!     [`PreconditionerUsed::ChannelBlocks`]), or additive Schwarz whose
//!     pairs with slopes are acted on by the same block elimination
//!     ([`SchwarzSolves::eliminated`]).
//! - Unsupported explicit combinations are [`Error::Unsupported`]; designs
//!   outside a route are declined ([`Outcome::Declined`]) for the caller's
//!   own solvers.
//!
//! Projections run on the caller's current rayon pool.

#![forbid(unsafe_code)]

mod certificate;
#[cfg(feature = "cmg")]
pub(crate) mod cmg_local;
mod design;
mod elimination;
mod error;
mod full;
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
    ReductionUsed, Report, Request, Route, SchwarzLocal, SchwarzSolves, SchwarzSpec, Settings,
    Subdomains,
};
pub use weights::Weights;
