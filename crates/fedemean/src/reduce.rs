//! Deterministic parallel reductions.

use crate::error::Result;

/// Reduce `0..items` over fixed leaves of `leaf` items merged over a fixed
/// binary tree: the result depends only on the inputs, never on the worker
/// count or scheduling. Outside a worker pool the same tree is evaluated in
/// order.
pub(crate) fn fixed_tree_reduce<T, L, M>(
    items: usize,
    leaf: usize,
    leaf_fn: &L,
    merge: &M,
) -> Result<T>
where
    T: Send,
    L: Fn(std::ops::Range<usize>) -> Result<T> + Sync,
    M: Fn(T, T) -> T + Sync,
{
    fn node<T, L, M>(start: usize, end: usize, leaf: usize, leaf_fn: &L, merge: &M) -> Result<T>
    where
        T: Send,
        L: Fn(std::ops::Range<usize>) -> Result<T> + Sync,
        M: Fn(T, T) -> T + Sync,
    {
        let blocks = (end - start).div_ceil(leaf);
        if blocks <= 1 {
            return leaf_fn(start..end);
        }
        let middle = start + blocks / 2 * leaf;
        // Outside a worker pool a join would inject (and allocate) jobs:
        // evaluate the same tree in order instead.
        let (left, right) = if rayon::current_thread_index().is_some() {
            rayon::join(
                || node(start, middle, leaf, leaf_fn, merge),
                || node(middle, end, leaf, leaf_fn, merge),
            )
        } else {
            (
                node(start, middle, leaf, leaf_fn, merge),
                node(middle, end, leaf, leaf_fn, merge),
            )
        };
        Ok(merge(left?, right?))
    }
    node(0, items, leaf.max(1), leaf_fn, merge)
}
