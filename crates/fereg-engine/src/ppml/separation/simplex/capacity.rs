use super::{overflow, View};
use crate::{
    error::Result,
    execution::{self, Poll, WorkPoll},
    fe::Design,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimplexCapacity {
    pub finite_columns: usize,
    pub fe_columns: usize,
    pub explicit_width: usize,
    pub kernel_cells: usize,
    pub topology_cells: usize,
    pub certificate_cells: usize,
    pub required_cells: usize,
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or_else(overflow)
}

/// Eight-byte cells; worst-case nullity is the full explicit width. The bound
/// is checked before dense design, null-space or tableau allocation.
pub fn simplex_kernel_cells(
    rows: usize,
    positive: usize,
    boundary: usize,
    width: usize,
) -> Result<usize> {
    if add(positive, boundary)? != rows {
        return Err(super::invalid(
            "simplex outcome partition differs from the sample",
        ));
    }
    let design = mul(rows, width)?;
    let square = mul(width, width)?;
    let nullspace = add(
        add(add(mul(2, rows)?, design)?, square)?,
        add(
            mul(boundary, width)?,
            add(mul(6, width)?, mul(3, boundary)?)?,
        )?,
    )?;
    let tableau_width = add(add(mul(2, width)?, mul(2, boundary)?)?, 2)?;
    let tableau = mul(add(boundary, 1)?, tableau_width)?;
    let phase_one = add(
        add(add(square, mul(boundary, width)?)?, tableau)?,
        add(add(mul(8, width)?, mul(7, boundary)?)?, 2)?,
    )?;
    let result = add(rows, width)?.max(add(rows, boundary)?);
    let reduction = add(design, nullspace.max(phase_one).max(result))?;
    // Immutable design, QR factor and transformed RHS copies, selected span,
    // coefficient bank, column scaling, permutations and reflector headers.
    let span = add(
        add(mul(4, design)?, square)?,
        add(mul(12, width)?, mul(2, rows)?)?,
    )?;
    Ok(reduction.max(span))
}
pub(super) fn estimate(
    view: View<'_>,
    design: &Design<'_>,
    finite: usize,
    poll: Poll<'_>,
) -> Result<SimplexCapacity> {
    let mut work = WorkPoll::new(poll)?;
    let rows = view.sample().n_active();
    let positive = execution::fold(&view.sample().outcome, 0, poll, |count, &v| {
        count + usize::from(v > 0.)
    })?;
    let boundary = rows - positive;
    let fe = design.layout().total;
    let width = add(finite, fe)?;
    let kernel_cells = simplex_kernel_cells(rows, positive, boundary, width)?;
    let mut topology_cells = add(mul(24, design.layout().effects.len())?, 16)?;
    // Codes, membership CSR, original levels, components, metadata and headers
    // remain resident while the explicit model and detector scratch are live.
    for (index, partition) in design.topology().partitions().iter().enumerate() {
        work.step()?;
        execution::check_chunk(poll, index)?;
        topology_cells = add(
            topology_cells,
            add(mul(3, rows)?, add(mul(4, partition.n_levels())?, 24)?)?,
        )?;
        let recipe_bytes = mul(4, partition.recipe().len())?;
        topology_cells = add(
            topology_cells,
            recipe_bytes / 8 + usize::from(recipe_bytes % 8 != 0),
        )?;
    }
    for (index, effect) in design.topology().effects().iter().enumerate() {
        work.step()?;
        execution::check_chunk(poll, index)?;
        // Effects sharing a partition still own distinct component-level
        // labels; do not count those once per deduplicated partition.
        topology_cells = add(
            topology_cells,
            design.topology().effect_partition(index)?.n_levels(),
        )?;
        let bytes = add(
            effect.label.len(),
            mul(4, effect.categorical_sources.len())?,
        )?;
        topology_cells = add(
            topology_cells,
            add(
                bytes / 8 + usize::from(bytes % 8 != 0),
                mul(8, effect.slopes.len())?,
            )?,
        )?;
        for (index, slope) in effect.slopes.iter().enumerate() {
            work.step()?;
            execution::check_chunk(poll, index)?;
            topology_cells = add(
                topology_cells,
                slope.label.len() / 8 + usize::from(slope.label.len() % 8 != 0),
            )?;
        }
    }
    // Proposal rows, original-axis direction, both reports during replay, and
    // cone/dual certificates are additional to the retained kernel bound.
    let certificate_cells = add(add(mul(8, rows)?, mul(4, width)?)?, 64)?;
    let required_cells = add(add(kernel_cells, topology_cells)?, certificate_cells)?;
    execution::check(poll)?;
    Ok(SimplexCapacity {
        finite_columns: finite,
        fe_columns: fe,
        explicit_width: width,
        kernel_cells,
        topology_cells,
        certificate_cells,
        required_cells,
    })
}
