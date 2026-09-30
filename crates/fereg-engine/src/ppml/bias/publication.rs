//! All ordinary stripes and requested-only FE banks are validated before return.
use super::{Channel, Coefficients, Information};
use crate::{
    data::{CompiledColumns, DenseMatrix, Prepared},
    error::{copied_string, reserved, Result},
    outputs::{self, Kind, Outputs},
};
pub(super) fn ordinary(
    columns: &CompiledColumns,
    info: &Information<'_, '_>,
    coefficients: &Coefficients,
) -> Result<(Vec<String>, DenseMatrix)> {
    let mut stripes = reserved(columns.slots.len())?;
    for slot in &columns.slots {
        stripes.push(copied_string(&slot.name)?);
    }
    let mut values = DenseMatrix::zeros(9, columns.slots.len())?;
    let levels = info.first_levels() + info.second_levels();
    // The accepted compact basis is the sole authority; omitted/deferred slots
    // remain exactly zero, and the anchor's component constants stay in FE1.
    for (j, &slot) in info.finite_slots().iter().enumerate() {
        for channel in Channel::ALL {
            values[(channel as usize, slot)] = coefficients.value(levels + j, channel)?;
        }
    }
    Ok((stripes, values))
}
pub(super) fn saved(
    sample: &Prepared,
    first: usize,
    coefficients: &Coefficients,
    enabled: bool,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outputs> {
    let mut kinds = reserved(if enabled { 18 } else { 0 })?;
    if enabled {
        for effect in 0..2 {
            for channel in Channel::ALL {
                kinds.push(Kind::Bias { effect, channel });
            }
        }
    }
    let mut result = outputs::allocate(sample, kinds)?;
    if !enabled {
        return Ok(result);
    }
    let topology = sample
        .topology
        .as_ref()
        .ok_or_else(|| super::invalid("bias topology is absent"))?;
    for effect in 0..2 {
        let codes = topology.effect_partition(effect)?.codes();
        for channel in Channel::ALL {
            interrupt()?;
            let destination = &mut result.columns[effect * 9 + channel as usize].values;
            for (row, &original) in sample.retained_rows().iter().enumerate() {
                destination[original] = coefficients.value(
                    if effect == 0 { 0 } else { first } + codes[row] as usize,
                    channel,
                )?;
            }
        }
    }
    outputs::validate_scatter(sample, &result)?;
    Ok(result)
}
