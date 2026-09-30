use crate::{error::Result, ErrorCode, FeregError};
pub(super) fn overflow() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "whole-call storage size overflows")
}
pub(super) fn sum(values: &[u128]) -> Result<u128> {
    values
        .iter()
        .try_fold(0_u128, |a, &b| a.checked_add(b).ok_or_else(overflow))
}
pub(super) fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or_else(overflow)
}
pub(super) fn array(count: usize, bytes: usize) -> Result<u128> {
    let size = count.checked_mul(bytes).ok_or_else(overflow)?;
    if size > isize::MAX as usize {
        return Err(overflow());
    }
    Ok(size as u128)
}
pub(super) fn matrix(rows: usize, columns: usize) -> Result<u128> {
    array(
        rows.checked_mul(columns).ok_or_else(overflow)?,
        size_of::<f64>(),
    )
}
pub(super) fn add_count(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(overflow)
}
