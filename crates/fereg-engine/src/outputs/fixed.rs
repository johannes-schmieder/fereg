//! Shared declared-channel mapping and ordinary FE normalization.
use super::{invalid, Request};
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    execution::Poll,
    fe::Design,
    threads::ThreadTeam,
    topology::FeTopology,
};
pub(super) fn normalized(
    design: &Design<'_>,
    coefficients: &DenseMatrix,
    tolerance: f64,
    team: &ThreadTeam,
    poll: Poll<'_>,
) -> Result<DenseMatrix> {
    let mut values = reserved(coefficients.as_slice().len())?;
    values.extend_from_slice(coefficients.as_slice());
    let mut alpha =
        DenseMatrix::from_row_major(coefficients.nrows(), coefficients.ncols(), values)?;
    design.normalize_coefficients_polled(&mut alpha, tolerance, team, poll)?;
    Ok(alpha)
}
pub(super) fn contribution(
    design: &Design<'_>,
    alpha: &DenseMatrix,
    effect: usize,
    channel: usize,
    row: usize,
) -> Result<f64> {
    let level = design.topology().effect_partition(effect)?.codes()[row] as usize;
    let index = design.coefficient_index(effect, channel, level)?;
    Ok(alpha[(index, 0)] * design.basis_value(effect, channel, row)?)
}
pub(super) fn validate_channels(topology: &FeTopology, request: &Request) -> Result<()> {
    for &(effect, channel) in &request.saved {
        let effect = topology
            .effects()
            .get(effect)
            .ok_or_else(|| invalid("saved FE effect is out of range"))?;
        if channel >= effect.slopes.len() + usize::from(effect.intercept) {
            return Err(invalid("saved FE channel is out of range"));
        }
    }
    Ok(())
}
