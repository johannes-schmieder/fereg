//! Common caller-thread scalar matrix publication; receipt ordering belongs to each estimator.
use crate::input::{PreparedMatrixStoreName, SpiApi};
use fereg_core::error::Result;

pub(crate) fn store_matrix(
    spi: &mut dyn SpiApi,
    name: &PreparedMatrixStoreName,
    rows: usize,
    columns: usize,
    values: &[f64],
) -> Result<()> {
    debug_assert_eq!(values.len(), rows * columns);
    for row in 1..=rows {
        for column in 1..=columns {
            spi.store_matrix(name, row, column, values[(row - 1) * columns + column - 1])?;
        }
    }
    Ok(())
}
