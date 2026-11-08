//! Local solves on the retained effects' diagonal blocks of the reduced
//! matrix (the building blocks of block-Jacobi preconditioning).
//!
//! With the largest effect eliminated, the reduced matrix over the retained
//! effects is `S = Σ_s (C_s' W_s C_s - C_s' w_s w_s' C_s / W_s)` over the
//! active segments `s` (eliminated levels with at least two cells). Its
//! diagonal block for one retained effect `b` is
//! `S_bb = Σ_s (diag(a_s) - a_s a_s' / W_s)`, where `a_sj` is the weight of
//! segment `s` at level `j` of `b` and `W_s = Σ_j a_sj` the segment's total
//! weight. Every observation has exactly one level of `b`, so the rows of
//! `S_bb` sum to zero: `S_bb` is a weighted graph Laplacian (an edge
//! `a_sj a_sk / W_s` between the levels of each segment), and constants on
//! each connected component of its graph lie in the null space of `S`. The
//! cross blocks `S_bc` have mixed signs, so `S` itself is not a Laplacian for
//! two or more retained effects; local solves only ever see `S_bb`.

use super::structure::{refill, Structure};
use super::weights::Weights;
use super::Cancel;
use crate::error::{Error, Result};

/// One retained block's Laplacian `S_bb`, as per-segment level sums.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct BlockLaplacian {
    /// Levels of the retained effect (the block's dimension).
    pub(crate) levels: usize,
    /// Segment boundaries into `entries` (`segments + 1` offsets).
    pub(crate) offsets: Vec<usize>,
    /// `(level, a_sj)` of each segment, levels distinct within a segment,
    /// weights normalized as in the reduced operator (divided by the maximum
    /// raw weight).
    pub(crate) entries: Vec<(u32, f64)>,
    /// `W_s`, the total (normalized) weight of each segment.
    pub(crate) totals: Vec<f64>,
}

/// A fixed, symmetric positive semidefinite approximation of the
/// pseudo-inverse of one block's Laplacian, applied to `width` lockstep
/// columns. `r` and `z` are level-major and interleaved: column `c` of level
/// `j` is at `j * width + c`. An implementation removes each connected
/// component's mean from its input and output (the component constants lie
/// in the null space of the reduced matrix), gives isolated levels a zero
/// image, and keeps its action unchanged between rebuilds, which happen only
/// between solves or at an explicit restart. `pool`, the caller's pool, may
/// run the solve's rows (CMG); the action never depends on it.
pub(crate) trait LocalSolve: Send {
    fn apply(
        &mut self,
        r: &[f64],
        z: &mut [f64],
        width: usize,
        pool: Option<&rayon::ThreadPool>,
    ) -> Result<()>;
    /// Bytes retained by the solve.
    fn bytes(&self) -> u128;
}

/// A local solve built within a byte budget (only CMG solves are, so a
/// build without feature `cmg` constructs none).
#[cfg_attr(not(feature = "cmg"), allow(dead_code))]
pub(crate) enum Admitted<S> {
    Solve(S),
    /// The block has no such solve (too few connected levels, or a numerical
    /// refusal of its graph); it keeps another local solve.
    Refused,
    /// The solve would exceed the budget.
    MemoryLimit,
}

/// Unused level slot of [`BlockLaplacian::fill`].
const FREE: u32 = u32::MAX;

/// `values.push(value)`, growing the allocation fallibly (amortized).
fn push<T>(values: &mut Vec<T>, value: T) -> Result<()> {
    if values.len() == values.capacity() {
        values
            .try_reserve(1)
            .map_err(|_| Error::OutOfMemory("unable to reserve a block Laplacian"))?;
    }
    values.push(value);
    Ok(())
}

impl BlockLaplacian {
    /// Form retained block `b`'s Laplacian for the current weight state in
    /// place: per active segment, the cell weights summed by the block's
    /// level, and `W_s` from the segment inverse. Segments with a single
    /// level of the block add no edge and are left out. `slot` is level
    /// scratch kept by the caller. Allocations are kept, and the entry
    /// count depends on the structure only, so rebuilds for new weights
    /// allocate nothing.
    pub(crate) fn fill(
        &mut self,
        st: &Structure,
        w: &Weights,
        b: usize,
        slot: &mut Vec<u32>,
        cancel: &Cancel<'_>,
    ) -> Result<()> {
        let nb = st.blocks.len();
        let offset = st.offsets[b] as usize;
        let end = st.offsets.get(b + 1).map_or(st.reduced, |&o| o as usize);
        self.levels = end - offset;
        refill(slot, self.levels, FREE)?;
        self.offsets.clear();
        self.entries.clear();
        self.totals.clear();
        push(&mut self.offsets, 0)?;
        let unit = w.cell.is_empty();
        for (step, &s) in st.active.iter().enumerate() {
            if step % 4096 == 0 {
                cancel.check()?;
            }
            let s = s as usize;
            let start = self.entries.len();
            for c in st.seg_cells[s]..st.seg_cells[s + 1] {
                let level = st.codes[c * nb + b] as usize - offset;
                let a = if unit { 1.0 } else { w.cell[c] };
                match slot[level] {
                    FREE => {
                        slot[level] = (self.entries.len() - start) as u32;
                        push(&mut self.entries, (level as u32, a))?;
                    }
                    k => self.entries[start + k as usize].1 += a,
                }
            }
            for &(level, _) in &self.entries[start..] {
                slot[level as usize] = FREE;
            }
            if self.entries.len() - start < 2 {
                self.entries.truncate(start);
                continue;
            }
            push(&mut self.offsets, self.entries.len())?;
            push(&mut self.totals, 1.0 / w.seg_inverse[s])?;
        }
        Ok(())
    }

    /// The `(level, a_sj)` entries of segment `s`.
    pub(crate) fn segment(&self, s: usize) -> &[(u32, f64)] {
        &self.entries[self.offsets[s]..self.offsets[s + 1]]
    }

    /// Bytes retained.
    pub(crate) fn bytes(&self) -> u128 {
        (self.offsets.capacity() * 8 + self.entries.capacity() * 16 + self.totals.capacity() * 8)
            as u128
    }
}
