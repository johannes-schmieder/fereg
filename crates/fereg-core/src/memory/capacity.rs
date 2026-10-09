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

/// Exactly reserve an empty vector; reservation failure is an allocation error.
pub(crate) fn fallible_empty<T>(capacity: usize, message: &'static str) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| allocation(message))?;
    Ok(values)
}

/// Exact fallible vector reservation. Owners take it as a generic parameter so
/// tests can fail any individual request; production passes
/// [`SystemVecReservation`].
pub(crate) trait VecReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

/// Reservation through the process allocator.
pub(crate) struct SystemVecReservation;

impl VecReservation for SystemVecReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

/// Exactly reserved vector holding `length` copies of `value`.
pub(crate) fn fallible_filled<T: Clone>(
    length: usize,
    value: T,
    message: &'static str,
) -> Result<Vec<T>> {
    let mut values = fallible_empty(length, message)?;
    values.resize(length, value);
    Ok(values)
}

/// Exactly reserved copy of `source`.
pub(crate) fn fallible_copy<T: Clone>(source: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut values = fallible_empty(source.len(), message)?;
    values.extend_from_slice(source);
    Ok(values)
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
