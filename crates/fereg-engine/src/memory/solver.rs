//! Estimates cover retained primary state, simultaneous fallback and RHS banks.
use super::{
    checked::{add_count, array, matrix, mul, overflow, sum},
    Effect, Request,
};
use crate::{
    error::Result,
    execution,
    fe::{Backend, Plan},
    linalg::WeightedQr,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct SolverStorage {
    pub retained: u128,
    pub scratch: u128,
}
pub(super) fn estimate(
    request: Request<'_>,
    rhs: usize,
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<SolverStorage> {
    poll()?;
    let primary = Plan::default_backend(request.effects.len(), request.solver);
    let first = backend(request, rhs, primary, poll)?;
    let fallback = match Plan::default_fallback(primary) {
        Some(backend_kind) => backend(request, rhs, backend_kind, poll)?,
        None => SolverStorage {
            retained: 0,
            scratch: 0,
        },
    };
    poll()?;
    Ok(SolverStorage {
        // The engine keeps the primary session while constructing LSMR.
        retained: sum(&[first.retained, fallback.retained])?,
        scratch: first.scratch.max(fallback.scratch),
    })
}
fn backend(
    request: Request<'_>,
    rhs: usize,
    backend: Backend,
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<SolverStorage> {
    let n = request.inputs.active_rows;
    let parameters = parameters(request.effects, poll)?;
    let coefficients = matrix(parameters, rhs)?;
    let observations = matrix(n, rhs)?;
    let mut retained = 0;
    match backend {
        Backend::NoEffects => (),
        Backend::Direct | Backend::SchurPcg => {
            for (index, effect) in request.effects.iter().enumerate() {
                execution::check_chunk_with(index, poll)?;
                retained = sum(&[retained, blocks(n, *effect)?])?;
            }
            if backend == Backend::SchurPcg {
                retained = sum(&[retained, coefficients])?;
            }
        }
        Backend::Lsmr => {
            retained = lsmr(request, poll)?;
        }
        Backend::Cmg => {
            retained = cmg(request, rhs, poll)?;
        }
    }
    // Includes raw/corrected candidate banks, independent certificates, bank
    // refinement and temporary per-column views; it is independent of workers.
    let mut maximum_channels = 0;
    for (index, effect) in request.effects.iter().enumerate() {
        execution::check_chunk_with(index, poll)?;
        maximum_channels = maximum_channels.max(effect.channels);
    }
    let scratch = sum(&[
        mul(observations, 12)?,
        mul(coefficients, 16)?,
        mul(matrix(n, maximum_channels)?, 4)?,
        array(add_count(parameters, rhs)?, 256)?,
    ])?;
    Ok(SolverStorage { retained, scratch })
}
fn blocks(rows: usize, effect: Effect) -> Result<u128> {
    sum(&[
        matrix(rows, effect.channels)?,
        matrix(rows, 1)?,
        array(effect.levels, size_of::<WeightedQr>())?,
        mul(matrix(effect.levels, effect.channels)?, 4)?,
        array(effect.levels, size_of::<Vec<f64>>())?,
    ])
}
fn lsmr(request: Request<'_>, poll: &mut impl FnMut() -> Result<()>) -> Result<u128> {
    let n = request.inputs.active_rows;
    let parameters = parameters(request.effects, poll)?;
    let mut channels = 0;
    for (index, effect) in request.effects.iter().enumerate() {
        execution::check_chunk_with(index, poll)?;
        channels = add_count(channels, effect.channels)?;
    }
    let mut total = sum(&[
        // Prepared labels/membership, locality maps, sqrt weights and compact
        // additive contribution storage are retained by the pinned raw solver.
        array(n, 64)?,
        mul(matrix(n, channels)?, 4)?,
        array(parameters, 128)?,
    ])?;
    for (i, left) in request.effects.iter().enumerate() {
        execution::check_chunk_with(i, poll)?;
        for (index, right) in request.effects[i + 1..].iter().enumerate() {
            execution::check_chunk_with(index, poll)?;
            let coordinates = add_count(
                left.levels
                    .checked_mul(left.channels)
                    .ok_or_else(overflow)?,
                right
                    .levels
                    .checked_mul(right.channels)
                    .ok_or_else(overflow)?,
            )?;
            // Raw signed blocks can use a Gremban cover. The default sampled
            // Schur and AC2 storage is modeled in sparse coordinates; never
            // assume a dense square over every FE coordinate.
            let vertices = mul(coordinates as u128, 2)?;
            let width = add_count(left.channels, right.channels)? as u128;
            let edges = mul(mul(n as u128, width)?, mul(width, 4)?)?;
            let logarithm = u128::from(coordinates.max(1).ilog2()) + 2;
            let fill = mul(mul(edges, logarithm)?, 8)?.min(mul(mul(vertices, vertices)?, 2)?);
            let dense_threshold = within::LocalSolverConfig::default().dense_threshold as u128;
            total = sum(&[
                total,
                mul(vertices, 160)?,
                mul(edges, 96)?,
                mul(fill, 24)?,
                mul(mul(vertices, dense_threshold)?, 16)?,
            ])?;
        }
    }
    Ok(total)
}
fn cmg(request: Request<'_>, rhs: usize, poll: &mut impl FnMut() -> Result<()>) -> Result<u128> {
    let vertices = parameters(request.effects, poll)?;
    let options = cmg::CmgOptions::default();
    let levels = options.max_levels as u128;
    let nnz = sum(&[
        vertices as u128,
        mul(request.inputs.active_rows as u128, 2)?,
    ])?;
    // The fill guard is evaluated after a crossing level; a direct terminal
    // may bypass it. Vertex vectors also include coarse isolated components.
    let hierarchy_nnz = mul(nnz, (options.max_hierarchy_nnz_factor.ceil() as u128) + 1)?;
    let hierarchy_vertices = mul(vertices as u128, levels)?;
    let terminal = vertices.min(options.direct_threshold);
    let nuisance = if request.effects.len() > 2 { 64 } else { 0 };
    let workspaces = rhs.max(nuisance);
    sum(&[
        // Serial construction has no dependency executor. Account separately
        // for graph/topology copies, hierarchy descriptors, forest/aggregation
        // temporaries and component owners, beyond principal-array estimates.
        mul(hierarchy_nnz, 32)?,
        mul(hierarchy_vertices, 96)?,
        mul(matrix(terminal, terminal)?, 3)?,
        mul(request.inputs.active_rows as u128, 160)?,
        mul(vertices as u128, 320)?,
        mul(levels, 2048)?,
        mul(
            sum(&[mul(vertices as u128, 256)?, mul(hierarchy_vertices, 64)?])?,
            workspaces as u128,
        )?,
        mul(matrix(request.inputs.active_rows, nuisance)?, 3)?,
        matrix(vertices, nuisance)?,
    ])
}
pub(super) fn parameters(
    effects: &[Effect],
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<usize> {
    let mut total = 0;
    for (index, effect) in effects.iter().enumerate() {
        execution::check_chunk_with(index, poll)?;
        total = add_count(
            total,
            effect
                .levels
                .checked_mul(effect.channels)
                .ok_or_else(overflow)?,
        )?;
    }
    Ok(total)
}
