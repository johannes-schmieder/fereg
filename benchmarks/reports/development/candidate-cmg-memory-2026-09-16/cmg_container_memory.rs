//! Supplemental capacity bounds for the pinned CMG principal-array reports.
//!
//! The reviewed dependency is CMG 90e1fe0 (Cargo.lock). These are container and
//! terminal-construction allowances, not a claim about its Rayon executor or
//! every hierarchy-construction temporary. See docs/allocation_inventory.md.

use crate::error::{ErrorCode, FeregError, Result};
use cmg::{
    CmgOptions, CmgPreconditioner, CmgProblemSize, Components, CsrLaplacian, HierarchyLevel,
    PcgBatchWorkspace,
};
use std::mem::size_of;

/// Descriptor banks, Arc allocations and spare component/permutation capacity.
/// Payload bytes already counted by CMG are intentionally left to its report.
pub(super) fn retained(preconditioner: &CmgPreconditioner, workspaces: usize) -> Result<u128> {
    let levels = preconditioner.hierarchy().levels();
    let vertices = levels.iter().try_fold(0_u128, |sum, level| {
        add(sum, level.graph().vertex_count() as u128)
    })?;
    let terminal = preconditioner
        .terminal_factor()
        .map_or(0, |factor| factor.vertex_count());
    containers(
        levels.len() as u128,
        vertices,
        terminal as u128,
        workspaces as u128,
    )
}

/// Called before either ordinary or prepared CMG constructs a hierarchy.
pub(super) fn construction(problem: CmgProblemSize, workspaces: usize) -> Result<u128> {
    let options = CmgOptions::default();
    // The fill guard tests after including the current level. Coarsening never
    // increases nnz, so at most one finest-sized level exceeds the factor-five
    // threshold. Integer arithmetic avoids rounding the ceiling down.
    if options.max_hierarchy_nnz_factor != 5.0 {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "CMG memory proof requires reviewed fill factor",
        ));
    }
    let finest_nnz = add(
        problem.vertices as u128,
        mul(problem.canonical_edges as u128, 2)?,
    )?;
    let levels = options.max_levels as u128;
    let vertices = mul(problem.vertices as u128, levels)?.min(mul(finest_nnz, 6)?);
    let terminal = options.direct_threshold.min(problem.vertices) as u128;
    let retained = containers(levels, vertices, terminal, workspaces as u128)?;
    // Vec growth may retain an old descriptor bank while allocating its
    // replacement. Two complete bounds cover that transient conservatively.
    // LDL keeps matrix + dense_lower (the upstream 16*d^2 allowance), then
    // builds a <= packed lower factor before either dense matrix is released.
    // Its row descriptors, anchor/index/component arrays and sort scratch
    // coexist; charge 2 Vec descriptors and 16 machine words per terminal row.
    let terminal_extra = add(
        mul(mul(terminal, terminal)?, 4)?,
        mul(
            terminal,
            (2 * size_of::<Vec<f64>>() + 16 * size_of::<usize>()) as u128,
        )?,
    )?;
    add(mul(retained, 2)?, terminal_extra)
}

fn containers(levels: u128, vertices: u128, terminal: u128, workspaces: u128) -> Result<u128> {
    let word = size_of::<usize>() as u128;
    let vector = size_of::<Vec<f64>>() as u128;
    // Push/Result-collect banks grow geometrically from a minimum of four.
    let level_capacity = mul(levels, 2)?.max(4);
    let workspace_capacity = mul(workspaces, 2)?.max(4);
    let hierarchy = mul(level_capacity, size_of::<HierarchyLevel>() as u128)?;
    // CenteringPlan = vertex count + tagged Vec + sizes Vec. The plan banks
    // own CSR descriptors and lineage handles, not the shared lineage again.
    let plans = mul(
        level_capacity,
        add(mul(vector, 2)?, mul(word, 2)?)? + size_of::<Option<CsrLaplacian>>() as u128 + word,
    )?;
    // Each graph can own two Arc<Vec<_>>, two Arc<()> and Arc<Components>.
    // Counting shared finest component/lineage allocations again is an upper
    // bound and covers the separately retained prepared current graph.
    let graph_headers = add(
        mul(add(vector, mul(word, 2)?)?, 2)?,
        add(mul(word, 4)?, size_of::<Components>() as u128 + 2 * word)?,
    )?;
    let arcs = mul(add(levels, 1)?, graph_headers)?;
    // Component size vectors grow by push. Compact labels may reuse a usize
    // allocation, retaining its byte capacity after conversion to u32. LDL
    // owns an additional component map and filtered permutation.
    let spare = mul(
        add(
            add(mul(vertices, 2)?, mul(levels, 4)?)?,
            add(mul(terminal, 2)?, 4)?,
        )?,
        word,
    )?;
    // CmgWorkspace owns five-Vec LevelWorkspace entries, three-Vec centering
    // entries, and dimensions. The outer PCG pool owns whole workspace values.
    let nested_workspace = mul(level_capacity, add(mul(vector, 8)?, word)?)?;
    let pools = add(
        mul(workspaces, nested_workspace)?,
        mul(workspace_capacity, size_of::<PcgBatchWorkspace>() as u128)?,
    )?;
    [plans, arcs, spare, pools]
        .into_iter()
        .try_fold(hierarchy, add)
}

fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn overflow() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "CMG container memory size overflows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmg::Laplacian;

    #[test]
    fn ordinary_and_disconnected_hierarchies_fit_the_prebuild_container_bound() {
        for n in [1, 2, 31, 701, 1409] {
            for disconnected in [false, true] {
                let edges: Vec<_> = (1..n)
                    .filter(|i| !disconnected || i % 7 != 0)
                    .map(|i| (i - 1, i, if i % 13 == 0 { 1e-10 } else { 1.0 }))
                    .collect();
                let problem = CmgProblemSize {
                    vertices: n,
                    input_edges: edges.len(),
                    canonical_edges: edges.len(),
                    right_hand_sides: 5,
                };
                let graph = Laplacian::from_edges(n, edges).unwrap();
                let preconditioner =
                    CmgPreconditioner::build(&graph, CmgOptions::default()).unwrap();
                for workers in [1, 4, 5] {
                    let bound = construction(problem, workers).unwrap();
                    let actual_shape = retained(&preconditioner, workers).unwrap();
                    assert!(bound >= actual_shape);
                    assert!(actual_shape > 0);
                }
            }
        }
    }

    #[test]
    fn container_counting_is_checked_and_monotone_in_lazy_pool_growth() {
        let one = containers(7, 3000, 699, 1).unwrap();
        let four = containers(7, 3000, 699, 4).unwrap();
        assert!(four > one);
        assert_eq!(
            containers(u128::MAX, 1, 1, 1).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert_eq!(
            containers(1, u128::MAX, 1, 1).unwrap_err().code,
            ErrorCode::Allocation
        );
    }
}
