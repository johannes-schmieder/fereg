//! Block-Jacobi preconditioning of the reduced system:
//! `M^-1 = blkdiag(B_1, .., B_k)` over the retained blocks' ranges of the
//! reduced vector, `B_b` a local solve of the block's Laplacian `S_bb` (see
//! [`super::block`]). Small blocks are solved exactly ([`DenseLocal`]),
//! large ones by their inverse diagonal, which is the Jacobi preconditioner
//! on that range. Each `B_b` is symmetric positive semidefinite and its null
//! space (the block's component constants, or its levels without curvature)
//! lies in the null space of `S`, so the recurrence stays consistent on the
//! singular reduced system.
//!
//! The blocks are built for one weight state between solves, keep their
//! buffers across rebuilds, and are applied without allocation.

use super::block::{BlockLaplacian, LocalSolve};
use super::dense::DenseLocal;
use super::pcg::Precondition;
use super::structure::{reserve, Structure};
use super::weights::Weights;
use super::{Cancel, BATCH, MAX_BLOCKS};
use crate::error::{Error, Result};
use crate::projector::{BlockSpec, BlockUsed, LocalSolver, LocalUsed};
use std::ops::Range;

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
}

/// The local solves of every retained block for one weight state.
#[derive(Default)]
pub(super) struct Blocks {
    /// The weight generation and choice the blocks were built for.
    built: Option<(u64, BlockSpec)>,
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

/// An upper bound of the bytes [`Blocks::ensure`] retains for `spec`.
pub(super) fn bound(st: &Structure, spec: &BlockSpec) -> u128 {
    let mut dense = 0_u128;
    let mut widest = 0_u128;
    for b in 0..st.blocks.len() {
        let k = range(st, b).len() as u128;
        if is_dense(spec, k as usize, st) {
            // Factor, scratch and the per-level and per-component indices.
            dense += k * k.saturating_sub(1) / 2 * 8 + k * BATCH as u128 * 8 + k * 48;
            widest = widest.max(k);
        }
    }
    if widest == 0 {
        return 0;
    }
    // The shared Laplacian (at most one entry per active cell) and slots.
    dense + st.cells() as u128 * 16 + (st.active.len() as u128 + 1) * 16 + widest * 4
}

impl Blocks {
    /// Build the local solves for weight generation `generation`, unless they
    /// are current.
    pub(super) fn ensure(
        &mut self,
        st: &Structure,
        w: &Weights,
        spec: BlockSpec,
        generation: u64,
        cancel: &Cancel<'_>,
    ) -> Result<()> {
        let nb = st.blocks.len();
        if self.built == Some((generation, spec)) && self.blocks.len() == nb {
            return Ok(());
        }
        self.built = None;
        if self.blocks.len() != nb {
            self.blocks.clear();
            reserve(&mut self.blocks, nb)?;
            self.blocks.resize_with(nb, Block::default);
        }
        for (b, block) in self.blocks.iter_mut().enumerate() {
            block.effect = st.blocks[b];
            block.range = range(st, b);
            block.local = Local::Jacobi;
            if is_dense(&spec, block.range.len(), st) {
                self.laplacian.fill(st, w, b, &mut self.slot, cancel)?;
                if block.dense.build(&self.laplacian, cancel)? {
                    block.local = Local::Dense;
                }
            } else if spec.large == LocalSolver::Cmg {
                return Err(Error::Unsupported(
                    "CMG local solves are not available in this build",
                ));
            }
        }
        self.built = Some((generation, spec));
        Ok(())
    }

    /// The blocks as reported in the route.
    pub(super) fn used(&self) -> [Option<BlockUsed>; MAX_BLOCKS] {
        std::array::from_fn(|b| {
            self.blocks.get(b).map(|block| BlockUsed {
                effect: block.effect,
                levels: block.range.len(),
                local: match block.local {
                    Local::Jacobi => LocalUsed::Jacobi,
                    Local::Dense => LocalUsed::Dense,
                },
            })
        })
    }

    /// Bytes retained.
    pub(super) fn bytes(&self) -> u128 {
        self.blocks
            .iter()
            .map(|block| block.dense.bytes())
            .sum::<u128>()
            + self.laplacian.bytes()
            + self.slot.capacity() as u128 * 4
    }
}

impl<const R: usize> Precondition<R> for Blocks {
    const JACOBI: bool = false;

    fn correct(&mut self, r: &[[f64; R]], z: &mut [[f64; R]]) -> Result<()> {
        for block in &mut self.blocks {
            if block.local == Local::Dense {
                let range = block.range.clone();
                block.dense.apply(
                    r[range.clone()].as_flattened(),
                    z[range].as_flattened_mut(),
                    R,
                )?;
            }
        }
        Ok(())
    }
}
