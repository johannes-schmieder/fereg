//! The prepared projection problem and its public inputs and outputs.

use crate::certificate::Certificate;
use crate::design::{Design, Members, OwnedMembers};
use crate::elimination::structure::{eliminated_effect, lane_width};
use crate::elimination::{self, Call, Cancel, Engine, MAX_BLOCKS};
use crate::error::{Error, Result};
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

/// Coefficient storage: per column `per_rhs` values; effect `e`'s levels at
/// `effect_offsets[e]..effect_offsets[e] + level_counts[e]` within a column.
#[derive(Debug, Clone, Copy)]
pub struct CoefficientLayout<'a> {
    pub(crate) per_rhs: usize,
    pub(crate) effect_offsets: &'a [usize],
    pub(crate) level_counts: &'a [usize],
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
        }
    }
}

/// Caller-owned FE coefficients. A projection *adds* the coefficients whose
/// fitted contribution it removed, so increments survive a decline (see
/// [`InputState`]). Coefficients are unnormalized: eliminated-effect levels
/// follow by exact back-substitution and the retained blocks are the PCG
/// iterate from zero, so the fitted contribution is unique while individual
/// coefficients are determined only up to the design's null space.
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
        let fits = layout.effect_offsets.len() == effects.len()
            && layout.level_counts.len() == effects.len()
            && effects.iter().enumerate().all(|(e, effect)| {
                layout.level_counts[e] == effect.levels()
                    && layout.effect_offsets[e]
                        .checked_add(effect.levels())
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

/// Outer problem. Explicit choices constrain [`Reduction::Auto`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Reduction {
    #[default]
    Auto,
    /// Eliminate the effect with the most levels exactly (global Schur
    /// complement) and iterate on the remaining effects.
    EliminateLargest,
}

/// Iterative method on the outer problem.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Method {
    #[default]
    Auto,
    /// Conjugate gradients on the reduced normal equations, in lockstep over
    /// up to eight columns.
    Pcg,
}

/// Preconditioner of the iterative method.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Preconditioner {
    /// Jacobi. With [`Fallback::Escalate`], a column batch whose Jacobi
    /// recurrence stalls or reaches the iteration ceiling continues from its
    /// current iterate with [`Preconditioner::BlockJacobi`] at the default
    /// [`BlockSpec`].
    #[default]
    Auto,
    /// Inverse diagonal of the reduced matrix.
    Jacobi,
    /// Block-Jacobi over the retained effects: a local solve of each
    /// retained effect's diagonal block of the reduced matrix (a weighted
    /// graph Laplacian), from the first iteration.
    BlockJacobi(BlockSpec),
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
    /// The best local solve this build has (currently [`LocalSolver::Jacobi`]).
    #[default]
    Auto,
    /// Combinatorial multigrid (not available in this build:
    /// [`Error::Unsupported`]).
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
    /// With [`Preconditioner::Auto`], escalate a batch whose recurrence
    /// stalls or reaches the iteration ceiling (see [`Preconditioner::Auto`]);
    /// decline when the escalated recurrence cannot finish either.
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
    /// Iteration ceiling (clamped to 1..=1500).
    pub max_iterations: usize,
    /// Also stop each column on the Jacobi-scaled residual `D^-1 r` within
    /// `tolerance` of the column's root-mean-square. A stopping heuristic
    /// for callers that refit predictors row by row, not a bound on the
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
}

impl<'a> Control<'a> {
    /// `cancel` is polled at iteration boundaries; `true` stops the
    /// projection with [`Error::Cancelled`].
    #[must_use]
    pub fn with_cancel(cancel: &'a (dyn Fn() -> bool + Sync)) -> Self {
        Self {
            cancel: Some(cancel),
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
    Eliminated { effect: usize },
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodUsed {
    Pcg,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreconditionerUsed {
    Jacobi,
    /// Block-Jacobi, one entry per retained effect in effect order.
    Block([Option<BlockUsed>; MAX_BLOCKS]),
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
    /// Block-Jacobi when any batch used it (explicitly or escalated).
    pub preconditioner: PreconditionerUsed,
    /// Column batches that escalated (saturating).
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
    pub cells: usize,
    pub active_segments: usize,
    pub reduced: usize,
    pub lanes: usize,
    pub structure_seconds: f64,
    pub weights_seconds: f64,
    pub rhs_seconds: f64,
    pub pcg_seconds: f64,
    pub residual_seconds: f64,
    pub solve_seconds: f64,
    /// Column batches that escalated to block-Jacobi.
    pub escalations: usize,
    /// Building block-Jacobi local solves.
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

/// One prepared projection problem: a fixed design generation, its retained
/// structure, current-weight state and workspaces. Weight changes refresh the
/// exact weighted state; repeated projections of one width allocate nothing.
pub struct Projector<'a> {
    design: Design<'a>,
    settings: Settings,
    owned: Option<OwnedMembers>,
    engine: Engine,
}

impl<'a> Projector<'a> {
    #[must_use]
    pub fn new(design: Design<'a>, settings: Settings) -> Self {
        Self {
            design,
            settings,
            owned: None,
            engine: Engine::default(),
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

    /// Build the retained structure now; `false` when the design is outside
    /// the route (remembered: later projections decline at once).
    pub fn prepare(&mut self, control: &Control<'_>) -> Result<bool> {
        let Self {
            design,
            settings,
            owned,
            engine,
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
        if !(request.tolerance.is_finite() && request.tolerance > 0.0) {
            return Err(Error::InvalidInput("projection tolerance is invalid"));
        }
        if let Preconditioner::BlockJacobi(BlockSpec {
            large: LocalSolver::Cmg,
            ..
        }) = self.settings.choice.preconditioner
        {
            return Err(Error::Unsupported(
                "CMG local solves are not available in this build",
            ));
        }
        if let Some(coefficients) = &coefficients {
            coefficients.check(&self.design, columns.ncols())?;
        }
        let Self {
            design,
            settings,
            owned,
            engine,
        } = self;
        let members = eliminated_members(design, owned)?;
        engine.project(
            &Call {
                design,
                members,
                width: lane_width(settings.lanes),
                choice: settings.choice,
                cancel: Cancel(control.cancel),
            },
            weights,
            columns,
            coefficients,
            request,
        )
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
        self.engine
            .certify_residual(&self.design, weights, column, tolerance, amplification)
    }

    /// The residual Gram of the last projection, when requested and certified.
    #[must_use]
    pub fn residual_gram(&self) -> Option<&Gram> {
        self.engine.residual_gram()
    }

    /// `sqrt(sum (w / max w) r^2)` per column of the last certified
    /// projection (empty after a decline).
    #[must_use]
    pub fn residual_norms(&self) -> &[f64] {
        self.engine.residual_norms()
    }

    /// Per-column achieved accuracy of the last certified projection.
    #[must_use]
    pub fn columns(&self) -> &[ColumnReport] {
        self.engine.columns()
    }

    /// Bytes a projection of `ncols` columns needs beyond the caller's data,
    /// once the structure exists (escalations excluded: they only start
    /// within [`Request::memory_limit`]).
    #[must_use]
    pub fn admission_bytes(&self, ncols: usize) -> Option<u128> {
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
