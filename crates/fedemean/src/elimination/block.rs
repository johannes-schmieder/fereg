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

use crate::error::Result;

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
/// between solves or at an explicit restart.
pub(crate) trait LocalSolve: Send {
    fn apply(&mut self, r: &[f64], z: &mut [f64], width: usize) -> Result<()>;
    /// Bytes retained by the solve.
    fn bytes(&self) -> u128;
}
