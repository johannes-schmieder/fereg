//! Expected requested heap from constructed ownership and actual hierarchy
//! dimensions. These engineering estimates never participate in admission.

use super::*;

pub(super) fn hierarchy_peak(
    graph: &Laplacian,
    executor: &ParallelExecutor,
    preconditioner: &CmgPreconditioner,
    prepared: Option<&PreparedCmgTopology>,
    components: bool,
) -> Result<u128> {
    let retained = admission::frame_storage(&[graph, executor, preconditioner], prepared)?;
    let report = preconditioner.hierarchy().report();
    let mut scratch = 0_u128;
    // Each nonterminal performs forest grouping then contraction/pruning.
    // CMG's reviewed banks are 57 bytes/vertex for forest scratch, two compact
    // edge banks plus an Option<Edge> gather bank for contraction, and 48
    // bytes/vertex for simultaneous old/new labels, diagonals and transfers.
    // nnz/2 safely includes edges even when some diagonals are zero.
    for (&vertices, &nnz) in report.vertex_counts().iter()
        .zip(report.matrix_nonzeros()).take(report.vertex_counts().len().saturating_sub(1))
    {
        let n = vertices as u128;
        let edges = (nnz / 2) as u128;
        scratch = scratch.max(57 * n).max(56 * edges + 48 * n);
    }
    // Component factors provision sparse rows per block. Model their numeric
    // and conversion banks from the actual filled factor, not a triangular
    // matrix spanning all disconnected vertices. Ordinary LDL constructs two
    // dense active-coordinate banks before conversion. Retained factor storage
    // is already in the ownership union above.
    if let Some(factor) = preconditioner.terminal_factor() {
        let n = factor.vertex_count() as u128;
        let active = factor.active_dimension() as u128;
        let nnz = factor.factor_nonzeros() as u128;
        let terminal_nnz = report.matrix_nonzeros().last().copied().unwrap_or(0) as u128;
        let numeric = if components {
            32 * nnz + 8 * terminal_nnz + 64 * n
        } else {
            active.checked_mul(active).and_then(|v| v.checked_mul(16))
                .and_then(|v| v.checked_add(96 * n))
                .ok_or_else(|| allocation("CMG factor heap forecast overflows"))?
        };
        scratch = scratch.max(numeric);
    }
    retained.checked_add(scratch)
        .ok_or_else(|| allocation("CMG construction heap forecast overflows"))
}
