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
//! - The full formulation's preconditioner as a handle
//!   ([`Projector::fe_preconditioner`], [`FePreconditioner`]): Jacobi
//!   (channel blocks with slopes) or additive Schwarz built for one weight
//!   state, applied by the caller to FE coefficient vectors in the canonical
//!   layout, for example as the FE block of a block preconditioner of
//!   `W^1/2 [X D]` in the caller's own modified LSMR.
//!
//! Projections run on the caller's current rayon pool.
//!
//! # FE preconditioner handle
//! ```text
//! let m = projector.fe_preconditioner(Weights::Values(w), Preconditioner::Schwarz(spec),
//!                                     Some(limit), &control)?;   // Ok(Err(decline)) when refused
//! m.apply(&moments, &mut step)?;   // step = M^-1 moments, M ~ D'WD at the raw weights w
//! ```
//! - **Contract.** `M^-1` is fixed, symmetric positive definite and
//!   nonsingular on all of coefficient space (modified LSMR's requirement),
//!   although `D'WD` is singular. Its action is multiplied by `1 / max w`, so
//!   it approximates `(D'WD)^-1` for the caller's raw weights and composes
//!   with the caller's other blocks at their scale. It acts on one column
//!   in the canonical layout ([`CoefficientLayout`]) and the raw basis.
//! - **Lifetime.** The handle owns its action: it outlives the call and the
//!   projector's later weight states. Rebuild it for new weights between
//!   recurrences, or keep an earlier one (still a valid, fixed
//!   preconditioner of the current system); never change it within a
//!   recurrence.
//! - **Memory.** The build is admitted against `memory_limit` stage by
//!   stage, transients included; [`FePreconditioner::workspace_bytes`] is
//!   the peak it was admitted at (a limit of exactly that builds, one byte
//!   less declines an explicit choice), and
//!   [`FePreconditioner::retained_bytes`] what it keeps. Before weights
//!   exist, [`Projector::fe_preconditioner_bytes`] bounds that peak for any
//!   weights from the design alone (the build's charges without its
//!   factorizations).
//! - **Determinism and control.** Builds and applies do not depend on the
//!   worker count; builds poll the cancellation callback, applies are fixed
//!   work.

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
pub use full::FePreconditioner;
pub use projector::{
    BlockSpec, BlockUsed, Choice, CoefficientLayout, CoefficientsMut, ColumnReport, ColumnStatus,
    ColumnsMut, Control, Decline, Facts, Fallback, Gram, InputState, LocalSolver, LocalUsed,
    Method, MethodUsed, Outcome, Preconditioner, PreconditionerUsed, Projector, Reduction,
    ReductionUsed, Report, Request, Route, SchwarzLocal, SchwarzSolves, SchwarzSpec, Settings,
    Subdomains,
};
pub use weights::Weights;
