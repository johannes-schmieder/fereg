//! Read-only numeric column access shared by raw and finalized samples.

pub(crate) trait SealedNumericSources {}

/// A source-indexed bank. Encoded-only finalized sources have no numeric column.
/// Implementations are sealed; raw public samples retain their Vec bank.
#[allow(private_bounds)]
pub trait NumericSourceBank: SealedNumericSources + Sync {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> Option<&Vec<f64>>;
    fn owned_buffer_bytes(&self) -> u128;
    fn aligned(&self, rows: usize) -> bool;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate numeric columns only; encoded slots are omitted. Use get for source indices.
    fn raw_columns(&self) -> impl Iterator<Item = &Vec<f64>> {
        (0..self.len()).filter_map(|index| self.get(index))
    }
}

impl SealedNumericSources for Vec<Vec<f64>> {}
impl NumericSourceBank for Vec<Vec<f64>> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn get(&self, index: usize) -> Option<&Vec<f64>> {
        self.as_slice().get(index)
    }
    fn owned_buffer_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(self)
            + self
                .as_slice()
                .iter()
                .map(crate::memory::vec_capacity_bytes)
                .sum::<u128>()
    }
    fn aligned(&self, rows: usize) -> bool {
        self.as_slice().iter().all(|column| column.len() == rows)
    }
}

impl<const N: usize> SealedNumericSources for [Vec<f64>; N] {}
impl<const N: usize> NumericSourceBank for [Vec<f64>; N] {
    fn len(&self) -> usize {
        N
    }
    fn get(&self, index: usize) -> Option<&Vec<f64>> {
        self.as_slice().get(index)
    }
    fn owned_buffer_bytes(&self) -> u128 {
        self.as_slice()
            .iter()
            .map(crate::memory::vec_capacity_bytes)
            .sum()
    }
    fn aligned(&self, rows: usize) -> bool {
        self.as_slice().iter().all(|column| column.len() == rows)
    }
}
