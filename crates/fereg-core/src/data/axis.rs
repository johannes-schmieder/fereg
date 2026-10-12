//! Logical row and level maps with allocation-free identity ownership.

use crate::memory::vec_capacity_bytes;

/// Storage for logical positions; construction alone does not validate a sample.
#[derive(Debug, Clone)]
pub enum FinalizedAxis<T> {
    Identity { len: usize },
    Selected(Vec<T>),
}

impl<T> FinalizedAxis<T> {
    pub fn len(&self) -> usize {
        match self {
            Self::Identity { len } => *len,
            Self::Selected(indices) => indices.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn selected(&self) -> Option<&[T]> {
        match self {
            Self::Identity { .. } => None,
            Self::Selected(indices) => Some(indices),
        }
    }

    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        match self {
            Self::Identity { .. } => 0,
            Self::Selected(indices) => vec_capacity_bytes(indices),
        }
    }
}

impl FinalizedAxis<u32> {
    pub(crate) fn original_level(&self, final_level: u32) -> Option<u32> {
        match self {
            Self::Identity { len } => ((final_level as usize) < *len).then_some(final_level),
            Self::Selected(levels) => levels.get(final_level as usize).copied(),
        }
    }

    pub(crate) fn final_level(&self, original_level: u32) -> Option<u32> {
        match self {
            Self::Identity { len } => ((original_level as usize) < *len).then_some(original_level),
            Self::Selected(levels) => levels
                .binary_search(&original_level)
                .ok()
                .and_then(|level| u32::try_from(level).ok()),
        }
    }
}

// Equality compares logical axes, so transfer and explicit rebasing witnesses
// remain exact even when one representation has no identity heap allocation.
impl<T: PartialEq + TryFrom<usize>> PartialEq for FinalizedAxis<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Identity { len: left }, Self::Identity { len: right }) => left == right,
            (Self::Selected(left), Self::Selected(right)) => left == right,
            (Self::Identity { len }, Self::Selected(indices))
            | (Self::Selected(indices), Self::Identity { len }) => {
                *len == indices.len()
                    && indices
                        .iter()
                        .enumerate()
                        .all(|(i, value)| T::try_from(i).is_ok_and(|identity| &identity == value))
            }
        }
    }
}

impl<T: Eq + TryFrom<usize>> Eq for FinalizedAxis<T> {}

impl FinalizedAxis<usize> {
    /// Read one logical original row without materializing an identity axis.
    pub fn get(&self, index: usize) -> Option<usize> {
        match self {
            Self::Identity { len } => (index < *len).then_some(index),
            Self::Selected(rows) => rows.get(index).copied(),
        }
    }

    /// Iterate original rows in their stored order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = usize> + DoubleEndedIterator + '_ {
        (0..self.len()).map(|index| self.get(index).expect("axis index is bounded"))
    }

    /// Consume selected storage unchanged; materialize identity only for an
    /// ownership boundary that requires an explicit vector.
    pub fn into_vec(self) -> crate::Result<Vec<usize>> {
        match self {
            Self::Selected(rows) => Ok(rows),
            Self::Identity { len } => {
                let mut rows = Vec::new();
                rows.try_reserve_exact(len).map_err(|_| {
                    crate::FeregError::new(
                        crate::ErrorCode::Allocation,
                        "unable to reserve explicit observation map",
                    )
                })?;
                rows.extend(0..len);
                Ok(rows)
            }
        }
    }
}

impl<T> From<Vec<T>> for FinalizedAxis<T> {
    fn from(rows: Vec<T>) -> Self {
        Self::Selected(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_identity_access_equality_and_fallible_materialization() {
        for len in [0, 1, 17] {
            let identity = FinalizedAxis::<usize>::Identity { len };
            assert_eq!(identity.owned_buffer_bytes(), 0);
            assert!(identity.selected().is_none());
            assert_eq!(identity.get(len), None);
            assert!(identity.iter().eq(0..len));
            assert!(identity.iter().rev().eq((0..len).rev()));
            assert_eq!(identity, FinalizedAxis::Selected((0..len).collect()));
            assert_eq!(identity.into_vec().unwrap(), (0..len).collect::<Vec<_>>());
        }
        assert_eq!(
            FinalizedAxis::<usize>::Identity { len: usize::MAX }
                .into_vec()
                .unwrap_err()
                .code,
            crate::ErrorCode::Allocation,
        );
    }

    #[test]
    fn axis_selected_conversion_preserves_pointer_capacity_and_order() {
        let mut rows = Vec::with_capacity(23);
        rows.extend([0, 4, 9]);
        let pointer = rows.as_ptr();
        let capacity = rows.capacity();
        let axis = FinalizedAxis::Selected(rows);
        assert_eq!(
            axis.owned_buffer_bytes(),
            (capacity * std::mem::size_of::<usize>()) as u128
        );
        assert_eq!(axis.get(1), Some(4));
        let rows = axis.into_vec().unwrap();
        assert_eq!(rows.as_ptr(), pointer);
        assert_eq!(rows.capacity(), capacity);
        assert_eq!(rows, [0, 4, 9]);
    }
}
