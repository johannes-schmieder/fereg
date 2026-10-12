//! Logical values with allocation-free storage for a repeated scalar.

use crate::error::{allocation, Result};
use crate::memory::vec_capacity_bytes;

/// A storage representation, not a validation receipt for aligned sample data.
#[derive(Debug, Clone)]
pub enum UniformValues<T> {
    Repeated { len: usize, value: T },
    Explicit(Vec<T>),
}

impl<T> UniformValues<T> {
    pub fn len(&self) -> usize {
        match self {
            Self::Repeated { len, .. } => *len,
            Self::Explicit(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn view(&self) -> UniformValuesRef<'_, T> {
        match self {
            Self::Repeated { len, value } => UniformValuesRef::Repeated { len: *len, value },
            Self::Explicit(values) => values.as_slice().into(),
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.view().get(index)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> + DoubleEndedIterator {
        self.view().iter()
    }

    /// Borrow already explicit storage; never materialize a repeated owner.
    pub fn explicit_mut(&mut self) -> Option<&mut [T]> {
        match self {
            Self::Repeated { .. } => None,
            Self::Explicit(values) => Some(values),
        }
    }

    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        match self {
            Self::Repeated { .. } => 0,
            Self::Explicit(values) => vec_capacity_bytes(values),
        }
    }
}

impl<T: Copy> UniformValues<T> {
    /// Move explicit storage intact, or fallibly materialize repeated values
    /// at an admitted mutation/ownership boundary.
    pub fn into_vec(self) -> Result<Vec<T>> {
        match self {
            Self::Explicit(values) => Ok(values),
            Self::Repeated { len, value } => {
                let mut values = Vec::new();
                values
                    .try_reserve_exact(len)
                    .map_err(|_| allocation("unable to reserve explicit uniform values"))?;
                values.resize(len, value);
                Ok(values)
            }
        }
    }
}

impl<T> std::ops::Index<usize> for UniformValues<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        self.get(index)
            .expect("uniform value index is out of bounds")
    }
}

impl<T> From<Vec<T>> for UniformValues<T> {
    fn from(values: Vec<T>) -> Self {
        Self::Explicit(values)
    }
}

impl<T: PartialEq> PartialEq for UniformValues<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        if self.is_empty() {
            return true;
        }
        match (self, other) {
            (Self::Repeated { value: left, .. }, Self::Repeated { value: right, .. }) => {
                left == right
            }
            _ => self.iter().eq(other.iter()),
        }
    }
}

impl<T: Eq> Eq for UniformValues<T> {}

/// A nonallocating view of an owned repeated value or a borrowed raw slice.
#[derive(Debug)]
pub enum UniformValuesRef<'a, T> {
    Repeated { len: usize, value: &'a T },
    Explicit(&'a [T]),
}

impl<T> Copy for UniformValuesRef<'_, T> {}
impl<T> Clone for UniformValuesRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> UniformValuesRef<'a, T> {
    pub fn len(self) -> usize {
        match self {
            Self::Repeated { len, .. } => len,
            Self::Explicit(values) => values.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn get(self, index: usize) -> Option<&'a T> {
        match self {
            Self::Repeated { len, value } => (index < len).then_some(value),
            Self::Explicit(values) => values.get(index),
        }
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = &'a T> + DoubleEndedIterator {
        (0..self.len()).map(move |index| self.get(index).expect("uniform view index is bounded"))
    }

    pub fn slice(self, range: std::ops::Range<usize>) -> Self {
        assert!(
            range.start <= range.end && range.end <= self.len(),
            "uniform view range is out of bounds"
        );
        match self {
            Self::Repeated { value, .. } => Self::Repeated {
                len: range.len(),
                value,
            },
            Self::Explicit(values) => Self::Explicit(&values[range]),
        }
    }
}

impl UniformValuesRef<'_, bool> {
    pub fn count_true(self) -> usize {
        match self {
            Self::Repeated { len, value } => {
                if *value {
                    len
                } else {
                    0
                }
            }
            Self::Explicit(values) => values.iter().filter(|value| **value).count(),
        }
    }

    pub fn all_true(self) -> bool {
        self.is_empty()
            || match self {
                Self::Repeated { value, .. } => *value,
                Self::Explicit(values) => values.iter().all(|value| *value),
            }
    }
}

impl<T> std::ops::Index<usize> for UniformValuesRef<'_, T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        self.get(index)
            .expect("uniform view index is out of bounds")
    }
}

impl<'a, T> From<&'a [T]> for UniformValuesRef<'a, T> {
    fn from(values: &'a [T]) -> Self {
        Self::Explicit(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DropReason;

    #[test]
    fn candidate_uniform_views_preserve_logical_ranges_without_materialization() {
        for len in [0, 1, 37] {
            for value in [false, true] {
                let mut owner = UniformValues::Repeated { len, value };
                let explicit = vec![value; len];
                assert!(owner.explicit_mut().is_none());
                assert_eq!(owner.owned_buffer_bytes(), 0);
                let view = owner.view();
                assert_eq!(view.count_true(), explicit.iter().filter(|v| **v).count());
                assert_eq!(view.all_true(), explicit.iter().all(|v| *v));
                assert_eq!(view.get(len), None);
                for start in 0..=len {
                    let slice = view.slice(start..len);
                    assert_eq!(slice.iter().copied().collect::<Vec<_>>(), explicit[start..]);
                    assert_eq!(slice.iter().rev().count(), len - start);
                }
            }
        }
        let mut raw = Vec::with_capacity(41);
        raw.extend([true, false, true]);
        let identity = (raw.as_ptr(), raw.capacity());
        let mut owner = UniformValues::Explicit(raw);
        owner.explicit_mut().unwrap()[1] = true;
        assert!(owner.view().all_true());
        let raw = owner.into_vec().unwrap();
        assert_eq!((raw.as_ptr(), raw.capacity()), identity);
    }

    #[test]
    fn drop_uniform_sequence_access_and_fallible_materialization() {
        for len in [0, 1, 19] {
            for value in [DropReason::Retained, DropReason::MissingOutcome] {
                let repeated = UniformValues::Repeated { len, value };
                assert_eq!(repeated.len(), len);
                assert_eq!(repeated.get(len), None);
                assert_eq!(repeated.owned_buffer_bytes(), 0);
                assert!(repeated.iter().all(|v| *v == value));
                assert_eq!(repeated, UniformValues::Explicit(vec![value; len]));
                assert_eq!(repeated.into_vec().unwrap(), vec![value; len]);
            }
        }
        assert_eq!(
            UniformValues::Repeated { len: 0, value: 1 },
            UniformValues::Repeated { len: 0, value: 2 },
        );
        assert_ne!(
            UniformValues::Repeated { len: 1, value: 1 },
            UniformValues::Repeated { len: 1, value: 2 },
        );
        let impossible = UniformValues::Repeated {
            len: usize::MAX,
            value: DropReason::Retained,
        };
        assert_eq!(
            impossible.into_vec().unwrap_err().code,
            crate::ErrorCode::Allocation
        );
    }

    #[test]
    fn drop_uniform_explicit_conversion_retains_actual_capacity_and_pointer() {
        let mut values = Vec::with_capacity(29);
        values.extend([DropReason::Retained, DropReason::Singleton]);
        let pointer = values.as_ptr();
        let capacity = values.capacity();
        let explicit = UniformValues::Explicit(values);
        assert_eq!(explicit[1], DropReason::Singleton);
        assert_eq!(
            explicit.owned_buffer_bytes(),
            (capacity * std::mem::size_of::<DropReason>()) as u128
        );
        let values = explicit.into_vec().unwrap();
        assert_eq!((values.as_ptr(), values.capacity()), (pointer, capacity));
    }
}
