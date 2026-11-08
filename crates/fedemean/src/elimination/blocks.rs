//! Block-Jacobi preconditioning of the reduced system:
//! `M^-1 = blkdiag(B_1, .., B_k)` over the retained blocks' ranges of the
//! reduced vector, `B_b` a local solve of the block's Laplacian `S_bb` (see
//! [`super::block`]). Small blocks are solved exactly ([`DenseLocal`]).
//! Large ones take a CMG cycle (feature `cmg`, `CmgLocal`) when CMG builds a
//! hierarchy of their graph, and otherwise their inverse diagonal, which is
//! the Jacobi preconditioner on that range. Each `B_b` is symmetric positive
//! semidefinite and its null space (the block's component constants, or its
//! levels without curvature) lies in the null space of `S`, so the
//! recurrence stays consistent on the singular reduced system.
//!
//! The blocks are built for one weight state between solves, keep their
//! buffers across rebuilds, and are applied without allocation; a CMG
//! hierarchy is fixed until the next build.
//!
//! Memory: [`bound`] covers the dense factors and the shared Laplacian from
//! the structure alone and is admitted before anything is built. A CMG solve
//! is admitted as it is built, from its graph's exact vertex and edge counts,
//! within the bytes left after the bound and the earlier blocks.

use super::block::{Admitted, BlockLaplacian, LocalSolve};
use super::dense::DenseLocal;
use super::pcg::Precondition;
use super::structure::{reserve, Structure};
use super::weights::Weights;
use super::{Cancel, BATCH, MAX_BLOCKS};
use crate::error::Result;
use crate::projector::{BlockSpec, BlockUsed, LocalSolver, LocalUsed};
use std::ops::Range;

#[cfg(feature = "cmg")]
use crate::cmg_local::CmgLocal;

/// No block has a CMG solve in a build without feature `cmg`.
#[cfg(not(feature = "cmg"))]
enum CmgLocal {}

#[cfg(not(feature = "cmg"))]
impl LocalSolve for CmgLocal {
    fn apply(
        &mut self,
        _: &[f64],
        _: &mut [f64],
        _: usize,
        _: Option<&rayon::ThreadPool>,
    ) -> Result<()> {
        match *self {}
    }

    fn bytes(&self) -> u128 {
        match *self {}
    }
}

/// Automatic dense limit on a block's levels ...
const DENSE_LEVELS: usize = 1024;
/// ... and the levels whose dense solve is cheap in any design.
const CHEAP_LEVELS: usize = 256;

/// A block is solved densely by default when its solve is cheap outright
/// (at most [`CHEAP_LEVELS`]: microseconds per apply, milliseconds to
/// factor), or when its apply (`k^2` per column) costs at most the reduced
/// operator's cell pass and its factorization (`k^3 / 6`) a few operator
/// applications of a full batch.
fn auto_dense(levels: usize, cells: usize, nb: usize) -> bool {
    let (k, work) = (levels as u128, (cells * nb) as u128);
    levels <= CHEAP_LEVELS || (levels <= DENSE_LEVELS && k * k <= work && k * k * k <= 384 * work)
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Local {
    #[default]
    Jacobi,
    Dense,
}

#[derive(Default)]
struct Block {
    effect: usize,
    range: Range<usize>,
    local: Local,
    dense: DenseLocal,
    /// The CMG solve of a large block, when it is used.
    cmg: Option<CmgLocal>,
}

/// What [`Blocks::ensure`] builds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Build {
    pub(super) spec: BlockSpec,
    /// The weight generation.
    pub(super) generation: u64,
    /// Lanes per CMG call (the batch width; applications are bitwise the
    /// same for any lane count).
    pub(super) lanes: usize,
    /// Bytes the blocks may hold (`None`: unlimited).
    pub(super) budget: Option<u128>,
}

/// The current blocks' build.
#[derive(Clone, Copy)]
struct Built {
    build: Build,
    /// Bytes admitted: [`bound`] and the CMG solves.
    admitted: u128,
    /// An automatic CMG solve exceeded the budget.
    limited: bool,
}

/// The local solves of every retained block for one weight state.
#[derive(Default)]
pub(super) struct Blocks {
    built: Option<Built>,
    blocks: Vec<Block>,
    /// Laplacian and level scratch shared by the builds.
    laplacian: BlockLaplacian,
    slot: Vec<u32>,
}

/// Levels of retained block `b`.
fn range(st: &Structure, b: usize) -> Range<usize> {
    let start = st.offsets[b] as usize;
    start..st.offsets.get(b + 1).map_or(st.reduced, |&o| o as usize)
}

fn is_dense(spec: &BlockSpec, levels: usize, st: &Structure) -> bool {
    match spec.dense_max_levels {
        Some(limit) => levels <= limit,
        None => auto_dense(levels, st.cells(), st.blocks.len()),
    }
}

/// Whether the large blocks of `spec` ask for a CMG solve (an explicit
/// request without feature `cmg` is unsupported).
fn wants_cmg(spec: &BlockSpec) -> bool {
    match spec.large {
        LocalSolver::Cmg => true,
        LocalSolver::Auto => cfg!(feature = "cmg"),
        LocalSolver::Jacobi => false,
    }
}

/// An upper bound of the bytes [`Blocks::ensure`] retains for `spec` before
/// it builds CMG solves: dense factors and the shared Laplacian.
pub(super) fn bound(st: &Structure, spec: &BlockSpec) -> u128 {
    let mut dense = 0_u128;
    let mut widest = 0_u128;
    for b in 0..st.blocks.len() {
        let k = range(st, b).len() as u128;
        if is_dense(spec, k as usize, st) {
            // Factor, scratch and the per-level and per-component indices.
            dense += k * k.saturating_sub(1) / 2 * 8 + k * BATCH as u128 * 8 + k * 48;
            widest = widest.max(k);
        } else if wants_cmg(spec) {
            widest = widest.max(k);
        }
    }
    if widest == 0 {
        return 0;
    }
    // The shared Laplacian (at most one entry per active cell) and slots.
    dense + st.cells() as u128 * 16 + (st.active.len() as u128 + 1) * 16 + widest * 4
}

/// The CMG solve of `lap` within `room` bytes, built on `pool` when given.
#[cfg(feature = "cmg")]
fn cmg(
    lap: &BlockLaplacian,
    lanes: usize,
    room: Option<u128>,
    cancel: &Cancel<'_>,
    pool: Option<&rayon::ThreadPool>,
) -> Result<Admitted<CmgLocal>> {
    CmgLocal::admit(lap, lanes, room, cancel.0, pool)
}

#[cfg(not(feature = "cmg"))]
fn cmg(
    _: &BlockLaplacian,
    _: usize,
    _: Option<u128>,
    _: &Cancel<'_>,
    _: Option<&rayon::ThreadPool>,
) -> Result<Admitted<CmgLocal>> {
    Err(crate::error::Error::Unsupported(
        "CMG local solves are not available in this build",
    ))
}

impl Blocks {
    /// Build the local solves of `build` unless they are current; `false`
    /// when [`bound`] or an explicitly requested CMG solve exceeds the
    /// budget (nothing usable is then built). An automatic CMG solve that
    /// exceeds it leaves its block with Jacobi. Current blocks are kept when
    /// they were built under the same budget, or fit the new one without
    /// such a substitution.
    pub(super) fn ensure(
        &mut self,
        st: &Structure,
        w: &Weights,
        build: Build,
        cancel: &Cancel<'_>,
        pool: Option<&rayon::ThreadPool>,
    ) -> Result<bool> {
        let nb = st.blocks.len();
        let fits = |bytes: u128| build.budget.is_none_or(|budget| bytes <= budget);
        if let Some(built) = &self.built {
            let same = built.build.spec == build.spec
                && built.build.generation == build.generation
                && self.blocks.len() == nb;
            if same
                && (built.build.budget == build.budget || !built.limited && fits(built.admitted))
            {
                return Ok(true);
            }
        }
        self.release();
        let spec = build.spec;
        let mut admitted = bound(st, &spec);
        if !fits(admitted) {
            return Ok(false);
        }
        if self.blocks.len() != nb {
            self.blocks.clear();
            reserve(&mut self.blocks, nb)?;
            self.blocks.resize_with(nb, Block::default);
        }
        let mut limited = false;
        for (b, block) in self.blocks.iter_mut().enumerate() {
            block.effect = st.blocks[b];
            block.range = range(st, b);
            block.local = Local::Jacobi;
            if is_dense(&spec, block.range.len(), st) {
                self.laplacian.fill(st, w, b, &mut self.slot, cancel)?;
                if block.dense.build(&self.laplacian, cancel)? {
                    block.local = Local::Dense;
                }
            } else if wants_cmg(&spec) {
                self.laplacian.fill(st, w, b, &mut self.slot, cancel)?;
                let room = build.budget.map(|budget| budget.saturating_sub(admitted));
                match cmg(&self.laplacian, build.lanes, room, cancel, pool)? {
                    Admitted::Solve(solve) => {
                        admitted += solve.bytes();
                        block.cmg = Some(solve);
                    }
                    Admitted::Refused => {}
                    Admitted::MemoryLimit if spec.large == LocalSolver::Cmg => {
                        self.release();
                        return Ok(false);
                    }
                    Admitted::MemoryLimit => limited = true,
                }
            }
        }
        self.built = Some(Built {
            build,
            admitted,
            limited,
        });
        Ok(true)
    }

    /// Forget the build and release its CMG hierarchies (dense factors keep
    /// their allocations).
    fn release(&mut self) {
        self.built = None;
        for block in &mut self.blocks {
            block.cmg = None;
        }
    }

    /// The blocks as reported in the route.
    pub(super) fn used(&self) -> [Option<BlockUsed>; MAX_BLOCKS] {
        std::array::from_fn(|b| {
            self.blocks.get(b).map(|block| BlockUsed {
                effect: block.effect,
                levels: block.range.len(),
                local: match (&block.cmg, block.local) {
                    (Some(_), _) => LocalUsed::Cmg,
                    (None, Local::Jacobi) => LocalUsed::Jacobi,
                    (None, Local::Dense) => LocalUsed::Dense,
                },
            })
        })
    }

    /// Bytes retained.
    pub(super) fn bytes(&self) -> u128 {
        self.blocks
            .iter()
            .map(|block| block.dense.bytes() + block.cmg.as_ref().map_or(0, LocalSolve::bytes))
            .sum::<u128>()
            + self.laplacian.bytes()
            + self.slot.capacity() as u128 * 4
    }
}

/// The blocks as one recurrence's preconditioner, with the pool its CMG
/// solves borrow as a team.
pub(super) struct OnPool<'b, 'p> {
    blocks: &'b mut Blocks,
    pool: Option<&'p rayon::ThreadPool>,
}

impl<'b, 'p> OnPool<'b, 'p> {
    pub(super) fn new(blocks: &'b mut Blocks, pool: Option<&'p rayon::ThreadPool>) -> Self {
        Self { blocks, pool }
    }
}

impl<const R: usize> Precondition<R> for OnPool<'_, '_> {
    const JACOBI: bool = false;

    fn correct(&mut self, r: &[[f64; R]], z: &mut [[f64; R]]) -> Result<()> {
        for block in &mut self.blocks.blocks {
            let range = block.range.clone();
            let (r, z) = (r[range.clone()].as_flattened(), z[range].as_flattened_mut());
            if let Some(cmg) = &mut block.cmg {
                cmg.apply(r, z, R, self.pool)?;
            } else if block.local == Local::Dense {
                block.dense.apply(r, z, R, self.pool)?;
            }
        }
        Ok(())
    }
}
