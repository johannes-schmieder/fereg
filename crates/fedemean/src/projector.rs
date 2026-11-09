//! The prepared projection problem and its public inputs and outputs.

use crate::certificate::Certificate;
use crate::design::{Design, Members, OwnedMembers};
use crate::elimination::structure::{eliminated_effect, lane_width};
use crate::elimination::{self, Call, Cancel, Engine, MAX_BLOCKS};
use crate::error::{Error, Result};
use crate::full;
use crate::weights::Weights;

/// Observation-major columns, residualized in place: observation `i` holds
/// its `ncols` values at `data[i * ncols..(i + 1) * ncols]`.
pub struct ColumnsMut<'a> {
    data: &'a mut [f64],
    nobs: usize,
    ncols: usize,
}

impl<'a> ColumnsMut<'a> {
    pub fn new(data: &'a mut [f64], nobs: usize, ncols: usize) -> Result<Self> {
        if nobs.checked_mul(ncols) != Some(data.len()) {
            return Err(Error::InvalidInput(
                "column storage does not match its dimensions",
            ));
        }
        Ok(Self { data, nobs, ncols })
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub const fn ncols(&self) -> usize {
        self.ncols
    }

    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &*self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut *self.data
    }
}

/// Coefficient storage: per column `per_rhs` values; within a column,
/// effect `e` occupies `channels[e] * level_counts[e]` values from
/// `effect_offsets[e]`, channel-major: channel `c` of level `l` at
/// `effect_offsets[e] + c * level_counts[e] + l` (one channel per effect
/// unless [`CoefficientLayout::with_channels`]).
#[derive(Debug, Clone, Copy)]
pub struct CoefficientLayout<'a> {
    pub(crate) per_rhs: usize,
    pub(crate) effect_offsets: &'a [usize],
    pub(crate) level_counts: &'a [usize],
    pub(crate) channels: Option<&'a [usize]>,
}

impl<'a> CoefficientLayout<'a> {
    #[must_use]
    pub const fn new(
        per_rhs: usize,
        effect_offsets: &'a [usize],
        level_counts: &'a [usize],
    ) -> Self {
        Self {
            per_rhs,
            effect_offsets,
            level_counts,
            channels: None,
        }
    }

    /// The effects' channel counts ([`crate::Effect::channels`]), required
    /// for a design whose effects have more than one channel.
    #[must_use]
    pub const fn with_channels(self, channels: &'a [usize]) -> Self {
        Self {
            channels: Some(channels),
            ..self
        }
    }
}

/// Caller-owned FE coefficients. A projection *adds* the coefficients whose
/// fitted contribution it removed, so increments survive a decline (see
/// [`InputState`]); after a warm start ([`Projector::project_from`]) these
/// are the start plus the solve's increments. Coefficients are
/// unnormalized: eliminated-effect levels follow by exact back-substitution
/// and the retained blocks are the PCG iterate from zero (the full
/// formulation: the method's iterate from its start), so the fitted
/// contribution is unique while individual coefficients are determined only
/// up to the design's null space (with slopes, also within a level whose
/// channels are dependent on its rows; see [`Preconditioner::Jacobi`]).
pub struct CoefficientsMut<'a> {
    pub(crate) values: &'a mut [f64],
    pub(crate) layout: CoefficientLayout<'a>,
}

impl<'a> CoefficientsMut<'a> {
    #[must_use]
    pub fn new(values: &'a mut [f64], layout: CoefficientLayout<'a>) -> Self {
        Self { values, layout }
    }

    fn check(&self, design: &Design<'_>, ncols: usize) -> Result<()> {
        let layout = &self.layout;
        let effects = design.effects();
        let channels = |e: usize| layout.channels.map_or(1, |channels| channels[e]);
        let fits = layout.effect_offsets.len() == effects.len()
            && layout.level_counts.len() == effects.len()
            && layout.channels.is_none_or(|c| c.len() == effects.len())
            && effects.iter().enumerate().all(|(e, effect)| {
                layout.level_counts[e] == effect.levels()
                    && channels(e) == effect.channels()
                    && effect
                        .levels()
                        .checked_mul(channels(e))
                        .and_then(|span| layout.effect_offsets[e].checked_add(span))
                        .is_some_and(|end| end <= layout.per_rhs)
            })
            && layout.per_rhs.checked_mul(ncols) == Some(self.values.len());
        if fits {
            Ok(())
        } else {
            Err(Error::InvalidInput(
                "coefficient layout does not match the design and columns",
            ))
        }
    }
}

/// Outer problem. Explicit choices are honored; [`Reduction::Auto`] picks a
/// formulation that supports the other choices.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Reduction {
    /// [`Reduction::EliminateLargest`] for two to five intercept effects (the
    /// elimination's current, provisional range), or two effects with
    /// slopes, with [`Method::Auto`] or [`Method::Pcg`] and an elimination
    /// preconditioner (`Auto`, `Jacobi`, `BlockJacobi`); [`Reduction::Full`]
    /// for any other design, for [`Method::Lsmr`] and for
    /// [`Preconditioner::Identity`] or [`Preconditioner::Schwarz`].
    /// `BlockJacobi` always eliminates.
    #[default]
    Auto,
    /// Eliminate the effect with the most levels exactly (global Schur
    /// complement) and iterate on the remaining effects. Designs with fewer
    /// than two or more than five effects decline
    /// ([`Decline::OutsideRoute`]).
    ///
    /// With slopes (two effects, [`Method::Pcg`], `Auto` or `Jacobi`;
    /// otherwise [`Error::Unsupported`]): the larger effect's level channel
    /// blocks are eliminated exactly, and PCG runs on the Schur complement
    /// over the other effect's coefficients with Jacobi on its diagonal
    /// blocks (exact inverses; a block that cancels to rounding relative to
    /// its level's Gram is null, [`PreconditionerUsed::ChannelBlocks`]). The
    /// eliminated coefficients follow by back-substitution, and every level
    /// of both effects is certified. No warm starts.
    EliminateLargest,
    /// The full system of every effect's coefficients, without elimination,
    /// for any number of effects (at least one), with or without slopes.
    Full,
}

/// Iterative method on the outer problem.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Method {
    /// [`Method::Pcg`] (provisional for the full formulation).
    #[default]
    Auto,
    /// Conjugate gradients on the normal equations (reduced, or the full
    /// system's singular consistent `D'WD x = D'W y`), in lockstep over up
    /// to eight columns.
    Pcg,
    /// Modified LSMR on `W^1/2 D` with `M ~ D'WD`, one column at a time
    /// (the full formulation only; feature `schwarz`, otherwise
    /// [`Error::Unsupported`]).
    Lsmr,
}

/// Preconditioner of the iterative method.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Preconditioner {
    /// The elimination: Jacobi. With [`Fallback::Escalate`], an eliminated
    /// column batch whose Jacobi recurrence stalls or reaches the iteration
    /// ceiling continues from its current iterate with
    /// [`Preconditioner::BlockJacobi`] at the default [`BlockSpec`].
    ///
    /// The full formulation (feature `schwarz`), two or more effects: with
    /// [`Fallback::Escalate`], Jacobi escalating to
    /// [`Preconditioner::Schwarz`] at the default [`SchwarzSpec`] as for
    /// [`Preconditioner::Jacobi`]; otherwise Schwarz when it builds within
    /// [`Request::memory_limit`], else Jacobi. One effect: Jacobi, which is
    /// exact. Without the feature: Jacobi. A design with slopes counts its
    /// effects with intercepts: with two or more, as above with the hybrid
    /// Schwarz of [`Preconditioner::Schwarz`]; otherwise Jacobi (with the
    /// levels' channel blocks) without escalation.
    #[default]
    Auto,
    /// Inverse diagonal of the reduced matrix, or of `D'WD` (the level
    /// masses) for the full formulation. On the full formulation with
    /// [`Fallback::Escalate`] (feature `schwarz`), a recurrence that stalls,
    /// reaches the iteration ceiling or whose rate after a short pilot
    /// predicts many iterations continues from its current iterate with
    /// [`Preconditioner::Schwarz`] at the default [`SchwarzSpec`] (a slow one
    /// whose Schwarz exceeds the memory limit continues with Jacobi).
    ///
    /// With slopes, the diagonal of an effect with several channels (or a
    /// slope without intercept) is its levels' `channels x channels` blocks
    /// of `D'WD`, each inverted exactly ([`PreconditionerUsed::ChannelBlocks`]):
    /// a block that is singular (a slope constant or zero on the level, or
    /// fewer distinct rows than channels: null directions of `D`) acts as its
    /// pseudo-inverse on its range and gives its numerically null
    /// directions (scaled eigenvalues at most `1e-10` of the largest) the
    /// block's largest scaled curvature, so the action stays symmetric
    /// positive definite. Consistent residuals have no component along those
    /// directions, so they shape neither iterates nor results.
    Jacobi,
    /// Block-Jacobi over the retained effects: a local solve of each
    /// retained effect's diagonal block of the reduced matrix (a weighted
    /// graph Laplacian), from the first iteration. Elimination only.
    BlockJacobi(BlockSpec),
    /// No preconditioner (`M = I`; the full formulation only).
    Identity,
    /// One-level additive Schwarz over factor-pair subdomains (the full
    /// formulation; feature `schwarz`, otherwise [`Error::Unsupported`]).
    /// With slopes, a pair in which either effect has several channels (or a
    /// slope without intercept) is one subdomain on both effects' channels:
    /// the block elimination of its cells, exact on the larger effect's
    /// level channel blocks and Jacobi on the Schur complement's diagonal
    /// blocks ([`SchwarzSolves::eliminated`]; [`SchwarzLocal`] applies to
    /// the pairs of intercept effects, and [`SchwarzLocal::Exact`] with
    /// slopes is [`Error::Unsupported`]).
    /// A subdomain is one connected component of the bipartite level graph
    /// of two effects, and its local operator is that component's block of
    /// `D'WD` (a signed graph Laplacian). The local actions are summed
    /// without partition of unity or coarse correction; levels in no
    /// subdomain (one effect) get Jacobi. The action is symmetric positive
    /// definite on all of coefficient space (see the full formulation's
    /// contract), so both methods accept it. A Schwarz preconditioner that
    /// exceeds [`Request::memory_limit`] declines ([`Decline::MemoryLimit`]).
    Schwarz(SchwarzSpec),
}

/// Additive Schwarz preconditioning of the full system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchwarzSpec {
    pub subdomains: Subdomains,
    pub local: SchwarzLocal,
}

/// The subdomains of [`Preconditioner::Schwarz`]: the connected components
/// of chosen pairs of effects.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Subdomains {
    /// The pairs of the effect with the most levels (first of equals) with
    /// every other effect: a star over the largest effect, `k - 1` pairs
    /// for `k` effects. The default.
    #[default]
    Star,
    /// Every pair of effects, `k (k - 1) / 2` pairs (more overlap, usually
    /// more iterations than the star).
    AllPairs,
}

/// The local solve of a Schwarz subdomain. Each eliminates the diagonal of
/// the subdomain's larger side exactly and solves the Schur complement on
/// its smaller side (a weighted graph Laplacian, formed exactly); they
/// differ in that solve.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SchwarzLocal {
    /// Exact where cheap, approximate elsewhere: a dense grounded Cholesky
    /// factorization of a small or dense Schur complement, an exact
    /// envelope (skyline) factorization in reverse Cuthill-McKee order of a
    /// banded one, and [`SchwarzLocal::Approximate`] otherwise.
    #[default]
    Auto,
    /// Exact: dense or envelope Cholesky factorizations of every Schur
    /// complement, whatever their size (bounded by the memory limit).
    Exact,
    /// Approximate Cholesky (approx-chol's AC(2) with a fixed seed) of every
    /// Schur complement with two or more levels.
    Approximate,
}

/// Local solves of [`Preconditioner::BlockJacobi`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockSpec {
    /// The local solve of blocks above the dense limit.
    pub large: LocalSolver,
    /// Blocks with at most this many levels are solved exactly (a grounded
    /// Cholesky factorization per connected component). `None`: automatic,
    /// blocks of at most 256 levels, and of at most 1024 levels when the
    /// dense solve costs no more than the reduced operator.
    pub dense_max_levels: Option<usize>,
}

/// Local solve of a block above the dense limit.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LocalSolver {
    /// The best local solve this build has: with feature `cmg`, a CMG cycle
    /// where CMG builds a hierarchy of the block's graph within the memory
    /// limit, else [`LocalSolver::Jacobi`]; without it, Jacobi.
    #[default]
    Auto,
    /// One fixed combinatorial multigrid cycle on the block's graph
    /// (feature `cmg`; otherwise [`Error::Unsupported`]). A block whose graph
    /// CMG refuses numerically (fewer than two connected levels, or weights
    /// it cannot represent) keeps Jacobi and reports [`LocalUsed::Jacobi`];
    /// a hierarchy beyond [`Request::memory_limit`] declines the projection
    /// ([`Decline::MemoryLimit`]).
    Cmg,
    /// The block's inverse diagonal (Jacobi on that block).
    Jacobi,
}

/// What happens when the chosen route cannot finish.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Fallback {
    /// Hand the columns back to the caller ([`Outcome::Declined`]).
    #[default]
    Decline,
    /// Escalate a Jacobi recurrence that stalls or reaches the iteration
    /// ceiling to a stronger preconditioner from its current iterate, within
    /// the remaining iteration budget: the elimination's
    /// [`Preconditioner::Auto`] to block-Jacobi, the full formulation's
    /// [`Preconditioner::Auto`] and [`Preconditioner::Jacobi`] to Schwarz
    /// (feature `schwarz`; also when a pilot predicts a slow recurrence).
    /// Decline when the escalated recurrence cannot finish either, or when
    /// the stronger preconditioner exceeds [`Request::memory_limit`].
    Escalate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Choice {
    pub reduction: Reduction,
    pub method: Method,
    pub preconditioner: Preconditioner,
    pub fallback: Fallback,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Settings {
    pub choice: Choice,
    /// Parallel lanes of the structure (1..=64). Results are deterministic
    /// for a fixed value, whatever the pool width. `None`: the current
    /// rayon pool's width when the structure is built.
    pub lanes: Option<usize>,
}

/// One projection's accuracy, resource and output requests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    /// Certified tolerance of the relative FE moments (see [`Certificate`]).
    pub tolerance: f64,
    /// Iteration ceiling: per column batch for PCG, per column over all
    /// passes for modified LSMR. The elimination clamps it to 1..=1500 (its
    /// recurrences yield to escalation or the caller beyond that); the full
    /// formulation honors it up to 1,000,000.
    pub max_iterations: usize,
    /// Also stop each column on the Jacobi-scaled residual `D^-1 r` within
    /// `tolerance` of the column's root-mean-square (modified LSMR: checked
    /// with the certificate after each pass). A stopping heuristic for
    /// callers that refit predictors row by row, not a bound on the
    /// coefficient error.
    pub coefficient_stopping: bool,
    /// Form the raw-weighted residual Gram `sum w r r'` (at most 8 columns).
    pub residual_gram: bool,
    /// Decline when the solver workspace would exceed this many bytes.
    pub memory_limit: Option<u128>,
}

impl Request {
    #[must_use]
    pub const fn new(tolerance: f64, max_iterations: usize) -> Self {
        Self {
            tolerance,
            max_iterations,
            coefficient_stopping: false,
            residual_gram: false,
            memory_limit: None,
        }
    }
}

/// Execution control. Projections run on the caller's current rayon pool.
#[derive(Clone, Copy, Default)]
pub struct Control<'a> {
    cancel: Option<&'a (dyn Fn() -> bool + Sync)>,
    pool: Option<&'a rayon::ThreadPool>,
}

impl<'a> Control<'a> {
    /// `cancel` is polled at iteration boundaries; `true` stops the
    /// projection with [`Error::Cancelled`].
    #[must_use]
    pub fn with_cancel(cancel: &'a (dyn Fn() -> bool + Sync)) -> Self {
        Self {
            cancel: Some(cancel),
            pool: None,
        }
    }

    /// Lend `pool`, the pool the projection runs on, to CMG local solves
    /// (feature `cmg`): CMG then builds and applies large hierarchies on its
    /// workers as a borrowed team (never a pool of its own). The pool
    /// changes only where rows run, never results, which repeat bitwise at
    /// any worker count; without it CMG runs serially.
    #[must_use]
    pub fn with_pool(self, pool: &'a rayon::ThreadPool) -> Self {
        Self {
            pool: Some(pool),
            ..self
        }
    }
}

/// Why a projection handed the columns back.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decline {
    /// The design is outside the route (effect count, sizes, empty segments).
    OutsideRoute,
    /// The columns do not match the design, or there are none.
    Shape,
    /// The solver workspace exceeds [`Request::memory_limit`].
    MemoryLimit,
    /// A level has no positive weight.
    Weights,
    /// The iteration stalled, broke down or failed its certificate.
    Numerical(&'static str),
}

/// State of the borrowed columns (and coefficient increments) after a
/// decline.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum InputState {
    /// Columns and coefficients are unchanged.
    Untouched,
    /// Some columns were partially residualized; their projection is the
    /// requested one. The coefficient buffer holds exactly the increments so
    /// far (original = current + fitted increments). `reference_scales` are
    /// the certificate scales of the original columns (NaN where untouched).
    SameProjection { reference_scales: Vec<f64> },
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReductionUsed {
    Eliminated {
        effect: usize,
    },
    /// The full system of every effect's coefficients.
    Full,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodUsed {
    Pcg,
    /// Modified LSMR.
    Lsmr,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreconditionerUsed {
    Jacobi,
    /// Block-Jacobi, one entry per retained effect in effect order.
    Block([Option<BlockUsed>; MAX_BLOCKS]),
    /// None (`M = I`).
    Identity,
    /// Additive Schwarz over `subdomains` factor-pair subdomains, with the
    /// local solves counted by kind.
    Schwarz {
        subdomains: u32,
        local: SchwarzSolves,
    },
    /// Jacobi on a design with slopes ([`Preconditioner::Jacobi`]): inverse
    /// level masses of the intercept effects and the exact inverses of the
    /// other effects' level channel blocks, `singular` of which (saturating)
    /// had a null direction.
    ChannelBlocks {
        singular: u32,
    },
}

/// The local Schur-complement solves of a Schwarz preconditioner, counted
/// by kind (saturating).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchwarzSolves {
    /// One level on the smaller side: nothing to solve.
    pub trivial: u32,
    /// Dense grounded Cholesky factorization.
    pub dense: u32,
    /// Exact envelope (skyline) Cholesky factorization.
    pub skyline: u32,
    /// Approximate Cholesky factorization.
    pub approximate: u32,
    /// Pairs with slopes: one subdomain on both effects' channels, exact on
    /// the larger effect's level blocks and Jacobi on the Schur
    /// complement's diagonal blocks.
    pub eliminated: u32,
}

/// One retained effect's block of a block-Jacobi preconditioner.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockUsed {
    /// The design effect.
    pub effect: usize,
    pub levels: usize,
    pub local: LocalUsed,
}

/// The local solve of one block.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalUsed {
    Cmg,
    /// Exact: grounded Cholesky factorization per connected component.
    Dense,
    /// The block's inverse diagonal.
    Jacobi,
}

/// The route actually used.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    pub reduction: ReductionUsed,
    pub method: MethodUsed,
    /// Block-Jacobi or Schwarz when any batch used it (explicitly or
    /// escalated).
    pub preconditioner: PreconditionerUsed,
    /// Column batches (modified LSMR: columns) that escalated (saturating).
    pub escalations: u8,
}

/// Cost and size facts of one projection (diagnostics; not scientific).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Facts {
    /// Largest iteration count over the column batches (an escalated batch
    /// counts both of its recurrences).
    pub iterations: usize,
    pub column_iterations: usize,
    /// Columns accepted at their rounding floor.
    pub accepted: usize,
    pub corrections: usize,
    pub columns: usize,
    /// Elimination cells (0 for the full formulation).
    pub cells: usize,
    pub active_segments: usize,
    /// Unknowns of the iterated system (the full formulation: every level).
    pub reduced: usize,
    /// Elimination lanes (0 for the full formulation, whose results do not
    /// depend on the worker count).
    pub lanes: usize,
    pub structure_seconds: f64,
    pub weights_seconds: f64,
    pub rhs_seconds: f64,
    /// Iterations (PCG or modified LSMR).
    pub pcg_seconds: f64,
    pub residual_seconds: f64,
    pub solve_seconds: f64,
    /// Column batches (modified LSMR: columns) that escalated to
    /// block-Jacobi or Schwarz.
    pub escalations: usize,
    /// Building local solves: block-Jacobi blocks or Schwarz subdomains.
    pub block_seconds: f64,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnStatus {
    Converged,
    /// Returned to an iterate meeting the acceptance rule (half the
    /// tolerance at the certificate scale) after its recurrence reached the
    /// rounding floor.
    AcceptedAtFloor,
}

/// Achieved accuracy of one column.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnReport {
    /// Largest relative remaining-level moment (the certified quantity).
    pub relative_moment: f64,
    /// Certificate scale: `max(reference norm, ||r||)`.
    pub scale: f64,
    /// `sqrt(sum (w / max w) r^2)`.
    pub residual_norm: f64,
    pub iterations: usize,
    pub status: ColumnStatus,
}

/// A successful projection.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Report {
    pub route: Route,
    pub facts: Facts,
    /// Certificate of all columns together; per-column values are in
    /// [`Projector::columns`].
    pub certificate: Certificate,
    /// The maximum weight that normalizes weights for norms and
    /// certificates (1 for unit weights).
    pub weight_scale: f64,
    /// Bytes of retained structure, weight state and solve workspaces.
    pub workspace_bytes: u128,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Projected(Report),
    Declined {
        reason: Decline,
        input: InputState,
        facts: Facts,
    },
}

/// Raw-weighted residual Gram `sum_i w_i r_i r_i'`, row-major, symmetric.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gram {
    pub(crate) columns: usize,
    pub(crate) values: Vec<f64>,
}

impl Gram {
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

/// The formulation a projector's choices resolve to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Formulation {
    Eliminate,
    Full(full::Method, full::Action),
}

/// Resolve the choices for `design` (see [`Reduction::Auto`]); unsupported
/// explicit combinations are errors.
fn formulation(design: &Design<'_>, choice: Choice) -> Result<Formulation> {
    let effects = design.effects().len();
    let sloped = design.sloped();
    if design
        .effects()
        .iter()
        .any(|effect| effect.channels() > full::MAX_CHANNELS)
    {
        return Err(Error::Unsupported("an effect has more than 16 channels"));
    }
    let eliminable = !sloped && (2..=MAX_BLOCKS + 1).contains(&effects);
    let reduction = match choice.reduction {
        Reduction::Auto => match (choice.method, choice.preconditioner) {
            (Method::Lsmr, _) | (_, Preconditioner::Identity | Preconditioner::Schwarz(_)) => {
                Reduction::Full
            }
            (_, Preconditioner::BlockJacobi(_)) => Reduction::EliminateLargest,
            _ if eliminable => Reduction::EliminateLargest,
            // Two effects with slopes: the slope elimination.
            _ if sloped && effects == 2 => Reduction::EliminateLargest,
            _ => Reduction::Full,
        },
        explicit => explicit,
    };
    if reduction == Reduction::EliminateLargest {
        if sloped {
            return match (effects, choice.method, choice.preconditioner) {
                (2, Method::Auto | Method::Pcg, Preconditioner::Auto | Preconditioner::Jacobi) => {
                    Ok(Formulation::Full(
                        full::Method::Pcg,
                        full::Action::Eliminate,
                    ))
                }
                (2, _, _) => Err(Error::Unsupported(
                    "the elimination with slopes runs PCG with Jacobi on its Schur blocks",
                )),
                _ => Err(Error::Unsupported(
                    "the elimination with slopes supports two effects (use Reduction::Full)",
                )),
            };
        }
        return match (choice.method, choice.preconditioner) {
            (Method::Lsmr, _) => Err(Error::Unsupported(
                "modified LSMR is not implemented on the eliminated system (use Reduction::Full)",
            )),
            (_, Preconditioner::Identity) => Err(Error::Unsupported(
                "the eliminated system has no unpreconditioned route",
            )),
            (_, Preconditioner::Schwarz(_)) => Err(Error::Unsupported(
                "additive Schwarz preconditions the full formulation (Reduction::Full)",
            )),
            _ => Ok(Formulation::Eliminate),
        };
    }
    // Pairs with slopes have one local solve, which is not exact.
    if let Preconditioner::Schwarz(spec) = choice.preconditioner {
        if sloped && effects >= 2 && spec.local == SchwarzLocal::Exact {
            return Err(Error::Unsupported(
                "exact Schwarz local solves need intercept effects",
            ));
        }
    }
    let schwarz = cfg!(feature = "schwarz");
    let escalate = choice.fallback == Fallback::Escalate;
    let action =
        match choice.preconditioner {
            // Jacobi first, Schwarz when Jacobi is slow or stalls.
            Preconditioner::Auto if schwarz && effects >= 2 && escalate => full::Action::Escalate,
            Preconditioner::Auto if schwarz && effects >= 2 => full::Action::Auto,
            Preconditioner::Jacobi if schwarz && escalate => full::Action::Escalate,
            Preconditioner::Auto | Preconditioner::Jacobi => full::Action::Jacobi,
            Preconditioner::Identity => full::Action::Identity,
            Preconditioner::BlockJacobi(_) => return Err(Error::Unsupported(
                "block-Jacobi preconditions the eliminated system (Reduction::EliminateLargest)",
            )),
            Preconditioner::Schwarz(spec) if schwarz => full::Action::Schwarz(spec),
            Preconditioner::Schwarz(_) => {
                return Err(Error::Unsupported("additive Schwarz needs feature schwarz"))
            }
        };
    let method = match choice.method {
        Method::Auto | Method::Pcg => full::Method::Pcg,
        #[cfg(feature = "schwarz")]
        Method::Lsmr => full::Method::Lsmr,
        #[cfg(not(feature = "schwarz"))]
        Method::Lsmr => return Err(Error::Unsupported("modified LSMR needs feature schwarz")),
    };
    Ok(Formulation::Full(method, action))
}

/// One prepared projection problem: a fixed design generation, its retained
/// structure, current-weight state and workspaces. Weight changes refresh the
/// exact weighted state; repeated PCG projections of one width allocate
/// nothing.
pub struct Projector<'a> {
    design: Design<'a>,
    settings: Settings,
    /// The resolved formulation; an unsupported combination is the error of
    /// every projection.
    formulation: Result<Formulation>,
    owned: Option<OwnedMembers>,
    engine: Engine,
    full: full::Engine,
}

impl<'a> Projector<'a> {
    #[must_use]
    pub fn new(design: Design<'a>, settings: Settings) -> Self {
        Self {
            formulation: formulation(&design, settings.choice),
            design,
            settings,
            owned: None,
            engine: Engine::default(),
            full: full::Engine::default(),
        }
    }

    #[must_use]
    pub const fn design(&self) -> &Design<'a> {
        &self.design
    }

    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Whether the choices resolve to the full formulation.
    fn is_full(&self) -> bool {
        matches!(self.formulation, Ok(Formulation::Full(..)))
    }

    /// Build the retained structure now; `false` when the design is outside
    /// the route (remembered: later projections decline at once).
    pub fn prepare(&mut self, control: &Control<'_>) -> Result<bool> {
        if let Formulation::Full(method, action) = self.formulation? {
            let call = full::Call {
                design: &self.design,
                method,
                action,
                cancel: Cancel(control.cancel),
            };
            return self.full.prepare(&call);
        }
        let Self {
            design,
            settings,
            owned,
            engine,
            ..
        } = self;
        let Some(members) = eliminated_members(design, owned)? else {
            return Ok(false);
        };
        engine.prepare(&Call {
            design,
            members: Some(members),
            width: lane_width(settings.lanes),
            choice: settings.choice,
            cancel: Cancel(control.cancel),
            pool: control.pool,
        })
    }

    /// Residualize `columns` in place. With `coefficients`, also add the FE
    /// coefficients of the removed fit.
    pub fn project(
        &mut self,
        weights: Weights<'_>,
        columns: ColumnsMut<'_>,
        coefficients: Option<CoefficientsMut<'_>>,
        request: &Request,
        control: &Control<'_>,
    ) -> Result<Outcome> {
        let formulation = self.check(&columns, coefficients.as_ref(), request)?;
        if let Formulation::Full(method, action) = formulation {
            let call = full::Call {
                design: &self.design,
                method,
                action,
                cancel: Cancel(control.cancel),
            };
            return self
                .full
                .project(&call, weights, columns, None, coefficients, request);
        }
        let Self {
            design,
            settings,
            owned,
            engine,
            ..
        } = self;
        let members = eliminated_members(design, owned)?;
        engine.project(
            &Call {
                design,
                members,
                width: lane_width(settings.lanes),
                choice: settings.choice,
                cancel: Cancel(control.cancel),
                pool: control.pool,
            },
            weights,
            columns,
            coefficients,
            request,
        )
    }

    /// [`Projector::project`] from the warm start `start`: FE coefficients
    /// in the design's canonical layout (per column, every effect in effect
    /// order, channel-major within it as in [`CoefficientLayout`]:
    /// `L = sum of levels times channels` values per column, so
    /// `start.len() == L * ncols`). The solve begins at the true residual
    /// `y - D start`, formed from the columns. The returned residual is
    /// `y - D (start + increments)`, and `coefficients` (add semantics)
    /// receive `start + increments` for every residualized column, so the
    /// buffer holds the full solution when it starts at zero; after a decline
    /// with [`InputState::SameProjection`] the residualized columns' entries
    /// hold theirs and the others are unchanged.
    ///
    /// Warm starts need the full formulation ([`Reduction::Full`], or
    /// [`Reduction::Auto`] resolving to it); otherwise
    /// [`Error::Unsupported`].
    pub fn project_from(
        &mut self,
        weights: Weights<'_>,
        columns: ColumnsMut<'_>,
        start: &[f64],
        coefficients: Option<CoefficientsMut<'_>>,
        request: &Request,
        control: &Control<'_>,
    ) -> Result<Outcome> {
        let (method, action) = match self.check(&columns, coefficients.as_ref(), request)? {
            Formulation::Full(method, action) if action != full::Action::Eliminate => {
                (method, action)
            }
            _ => {
                return Err(Error::Unsupported(
                    "warm starts need the full formulation (Reduction::Full)",
                ))
            }
        };
        let per_column = self
            .design
            .effects()
            .iter()
            .try_fold(0_usize, |total, effect| {
                total.checked_add(effect.levels().checked_mul(effect.channels())?)
            });
        if per_column.and_then(|l| l.checked_mul(columns.ncols())) != Some(start.len()) {
            return Err(Error::InvalidInput(
                "warm-start coefficients do not match the design and columns",
            ));
        }
        if !start.iter().all(|value| value.is_finite()) {
            return Err(Error::InvalidData("warm-start coefficients must be finite"));
        }
        let call = full::Call {
            design: &self.design,
            method,
            action,
            cancel: Cancel(control.cancel),
        };
        self.full
            .project(&call, weights, columns, Some(start), coefficients, request)
    }

    /// The checks every projection makes before it reads data: the
    /// tolerance, supported choices and the coefficient layout.
    fn check(
        &self,
        columns: &ColumnsMut<'_>,
        coefficients: Option<&CoefficientsMut<'_>>,
        request: &Request,
    ) -> Result<Formulation> {
        if !(request.tolerance.is_finite() && request.tolerance > 0.0) {
            return Err(Error::InvalidInput("projection tolerance is invalid"));
        }
        let formulation = self.formulation?;
        #[cfg(not(feature = "cmg"))]
        if let Preconditioner::BlockJacobi(BlockSpec {
            large: LocalSolver::Cmg,
            ..
        }) = self.settings.choice.preconditioner
        {
            return Err(Error::Unsupported(
                "CMG local solves are not available in this build",
            ));
        }
        if let Some(coefficients) = coefficients {
            coefficients.check(&self.design, columns.ncols())?;
        }
        Ok(formulation)
    }

    /// Recompute every level's weighted moment of one residual column (the
    /// eliminated levels included) and certify it at `amplification` times
    /// its weighted norm. `None` when this projector did not project with
    /// these weights.
    pub fn certify_residual(
        &self,
        weights: Weights<'_>,
        column: &[f64],
        tolerance: f64,
        amplification: f64,
    ) -> Result<Option<Certificate>> {
        if self.is_full() {
            return self.full.certify_residual(
                &self.design,
                weights,
                column,
                tolerance,
                amplification,
            );
        }
        self.engine
            .certify_residual(&self.design, weights, column, tolerance, amplification)
    }

    /// The residual Gram of the last projection, when requested and certified.
    #[must_use]
    pub fn residual_gram(&self) -> Option<&Gram> {
        if self.is_full() {
            return self.full.residual_gram();
        }
        self.engine.residual_gram()
    }

    /// `sqrt(sum (w / max w) r^2)` per column of the last certified
    /// projection (empty after a decline).
    #[must_use]
    pub fn residual_norms(&self) -> &[f64] {
        if self.is_full() {
            return self.full.residual_norms();
        }
        self.engine.residual_norms()
    }

    /// Per-column achieved accuracy of the last certified projection.
    #[must_use]
    pub fn columns(&self) -> &[ColumnReport] {
        if self.is_full() {
            return self.full.columns();
        }
        self.engine.columns()
    }

    /// Bytes a projection of `ncols` columns needs beyond the caller's data,
    /// once the structure exists (escalations excluded: they only start
    /// within [`Request::memory_limit`]). CMG local solves are excluded too:
    /// they are admitted as they are built, from their graphs' sizes, and so
    /// are Schwarz subdomains, which count here once built. The full
    /// formulation counts weighted state.
    #[must_use]
    pub fn admission_bytes(&self, ncols: usize) -> Option<u128> {
        if let Ok(Formulation::Full(method, _)) = self.formulation {
            return self.full.structure().map(|st| {
                full::admission_bytes(st, method, ncols, true) + self.full.schwarz_bytes(ncols)
            });
        }
        self.engine.structure().map(|st| {
            elimination::admission_bytes(st, None, ncols)
                + elimination::explicit_block_bytes(st, self.settings.choice)
        })
    }
}

/// The eliminated effect's members, built once when the caller gave none;
/// `None` when the design is outside the route.
fn eliminated_members<'s>(
    design: &'s Design<'_>,
    owned: &'s mut Option<OwnedMembers>,
) -> Result<Option<Members<'s>>> {
    if !(2..=MAX_BLOCKS + 1).contains(&design.effects().len()) {
        return Ok(None);
    }
    let Some(eliminated) = eliminated_effect(design) else {
        return Ok(None);
    };
    let effect = &design.effects()[eliminated];
    if let Some(members) = effect.members() {
        return Ok(Some(members));
    }
    if owned.is_none() {
        *owned = Some(OwnedMembers::build(effect.codes(), effect.levels())?);
    }
    Ok(owned.as_ref().map(OwnedMembers::view))
}
