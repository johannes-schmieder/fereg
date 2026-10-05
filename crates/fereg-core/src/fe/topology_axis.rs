//! Final row and level maps distinguish a transferred identity axis from rebasing.

use crate::memory::vec_capacity_bytes;

#[derive(Debug, Clone)]
pub(super) enum FinalizedAxis<T> {
    Identity { len: usize },
    Selected(Vec<T>),
}

impl<T> FinalizedAxis<T> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Identity { len } => *len,
            Self::Selected(indices) => indices.len(),
        }
    }

    pub(super) fn selected(&self) -> Option<&[T]> {
        match self {
            Self::Identity { .. } => None,
            Self::Selected(indices) => Some(indices),
        }
    }

    pub(super) fn owned_buffer_bytes(&self) -> u128 {
        match self {
            Self::Identity { .. } => 0,
            Self::Selected(indices) => vec_capacity_bytes(indices),
        }
    }
}

impl FinalizedAxis<u32> {
    pub(super) fn original_level(&self, final_level: u32) -> Option<u32> {
        match self {
            Self::Identity { len } => ((final_level as usize) < *len).then_some(final_level),
            Self::Selected(levels) => levels.get(final_level as usize).copied(),
        }
    }

    pub(super) fn final_level(&self, original_level: u32) -> Option<u32> {
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
