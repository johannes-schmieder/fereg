//! Include spare capacities and string payload of already-owned input banks.
use super::{
    checked::{add_count, array, mul, sum},
    Inputs,
};
use crate::{
    data::{Loaded, LoadedAdjustment, LoadedSource, NumericColumn},
    error::Result,
};

pub(super) fn inputs(loaded: &Loaded, metadata: u128, parent: u128) -> Result<Inputs> {
    let n = loaded.candidate.len();
    let mut resident = sum(&[
        array(loaded.candidate.capacity(), 1)?,
        array(loaded.sources.capacity(), size_of::<LoadedSource>())?,
        numeric(&loaded.outcome)?,
    ])?;
    let mut strings = 0;
    for source in &loaded.sources {
        resident = sum(&[
            resident,
            match source {
                LoadedSource::Numeric(column) => numeric(column)?,
                LoadedSource::Strings(values) => {
                    strings += 1;
                    let mut bytes = array(values.capacity(), size_of::<String>())?;
                    for value in values {
                        bytes = sum(&[bytes, value.capacity() as u128])?;
                    }
                    bytes
                }
            },
        ])?;
    }
    if let Some((_, column)) = &loaded.weights {
        resident = sum(&[resident, numeric(column)?])?;
    }
    match &loaded.adjustment {
        LoadedAdjustment::None => (),
        LoadedAdjustment::Offset(column) | LoadedAdjustment::Exposure(column) => {
            resident = sum(&[resident, numeric(column)?])?;
        }
    }
    let numeric_sources = add_count(
        loaded.sources.len() - strings,
        1 + usize::from(loaded.weights.is_some())
            + usize::from(!matches!(loaded.adjustment, LoadedAdjustment::None)),
    )?;
    let declared_minimum = sum(&[
        mul(array(n, 8)?, numeric_sources as u128)?,
        mul(array(n, size_of::<String>())?, strings as u128)?,
        array(n, 1)?,
    ])?;
    Ok(Inputs {
        original_rows: n,
        active_rows: loaded.candidate.iter().filter(|&&v| v).count(),
        sources: loaded.sources.len(),
        string_sources: strings,
        // Actual payload/capacity is already in the known resident excess.
        string_bytes: 0,
        metadata_bytes: metadata,
        weighted: loaded.weights.is_some(),
        adjustment: !matches!(loaded.adjustment, LoadedAdjustment::None),
        parent_bytes: sum(&[parent, resident.saturating_sub(declared_minimum)])?,
    })
}
fn numeric(column: &NumericColumn) -> Result<u128> {
    sum(&[
        array(column.values.capacity(), 8)?,
        match &column.missing {
            Some(values) => array(values.capacity(), 1)?,
            None => 0,
        },
    ])
}
