//! Checked element counts shared by concrete owners and admission envelopes.
use crate::error::{allocation, FeregError, Result};
use std::{marker::PhantomData, mem::size_of};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Capacity<T> {
    elements: u128,
    element: PhantomData<T>,
}
impl<T> Capacity<T> {
    pub(crate) const fn new(elements: u128) -> Self {
        Self {
            elements,
            element: PhantomData,
        }
    }
    pub(crate) const fn elements(&self) -> u128 {
        self.elements
    }
    pub(crate) fn matrix(rows: u128, columns: u128, message: &str) -> Result<Self> {
        rows.checked_mul(columns)
            .map(Self::new)
            .ok_or_else(|| allocation(message))
    }
    pub(crate) fn bytes(&self, message: &str) -> Result<u128> {
        self.elements
            .checked_mul(size_of::<T>() as u128)
            .ok_or_else(|| allocation(message))
    }
    pub(crate) fn len(&self, message: &str) -> Result<usize> {
        usize::try_from(self.elements).map_err(|_| allocation(message))
    }
    pub(crate) fn filled(&self, value: T, message: &str) -> Result<Vec<T>>
    where
        T: Clone,
    {
        self.filled_with(value, || allocation(message))
    }
    pub(crate) fn filled_with(&self, value: T, error: impl FnOnce() -> FeregError) -> Result<Vec<T>>
    where
        T: Clone,
    {
        let Ok(count) = usize::try_from(self.elements) else {
            return Err(error());
        };
        let mut values = Vec::new();
        if values.try_reserve_exact(count).is_err() {
            return Err(error());
        }

        values.resize(count, value);
        Ok(values)
    }
}
pub(crate) fn matrix_bytes<T>(rows: u128, columns: u128, message: &str) -> Result<u128> {
    Capacity::<T>::matrix(rows, columns, message)?.bytes(message)
}

/// Push into a vector whose capacity its owner already reserved. Never
/// reallocates; exhausting the reservation is an allocation error.
pub(crate) fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

/// Optional installation consumes a conservative admitted lifetime charge.
/// Recompute the constructor-owned requirement before creating its storage.
pub(crate) fn require_admitted_capacity(required: u128, admitted: u128) -> Result<()> {
    if required > admitted {
        Err(allocation(
            "workspace capacity exceeds its admitted lifetime charge",
        ))
    } else {
        Ok(())
    }
}
