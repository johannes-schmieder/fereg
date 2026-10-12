//! Logical values with allocation-free storage for a repeated scalar.

use crate::error::{allocation, Result};
use crate::memory::vec_capacity_bytes;

/// A storage representation, not a validation receipt for aligned sample data.
#[derive(Debug)]
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

    pub fn get(&self, index: usize) -> Option<&T> {
        match self {
            Self::Repeated { len, value } => (index < *len).then_some(value),
            Self::Explicit(values) => values.get(index),
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> + DoubleEndedIterator {
        (0..self.len()).map(|index| self.get(index).expect("uniform value index is bounded"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DropReason;

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
