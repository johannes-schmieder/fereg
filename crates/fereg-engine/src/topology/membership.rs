//! One CSR membership index; codes and membership are independently checked.
use crate::error::{reserved, ErrorCode, FeregError, Result};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelIndex {
    offsets: Vec<usize>,
    observations: Vec<usize>,
}
impl LevelIndex {
    pub fn from_codes(codes: &[u32], levels: usize) -> Result<Self> {
        let mut offsets = reserved(levels.checked_add(1).ok_or_else(allocation)?)?;
        offsets.resize(levels + 1, 0usize);
        for &code in codes {
            let level = code as usize;
            if level >= levels {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "membership code exceeds declared levels",
                ));
            }
            offsets[level + 1] = offsets[level + 1].checked_add(1).ok_or_else(allocation)?;
        }
        for level in 0..levels {
            offsets[level + 1] = offsets[level + 1]
                .checked_add(offsets[level])
                .ok_or_else(allocation)?;
        }
        let mut next = reserved(levels)?;
        next.extend_from_slice(&offsets[..levels]);
        let mut observations = reserved(codes.len())?;
        observations.resize(codes.len(), 0);
        for (row, &code) in codes.iter().enumerate() {
            observations[next[code as usize]] = row;
            next[code as usize] += 1;
        }
        let result = Self {
            offsets,
            observations,
        };
        result.validate(codes)?;
        Ok(result)
    }
    #[must_use]
    pub fn n_levels(&self) -> usize {
        self.offsets.len() - 1
    }
    #[must_use]
    pub fn nobs(&self) -> usize {
        self.observations.len()
    }
    pub fn members(&self, level: usize) -> Result<&[usize]> {
        if level >= self.n_levels() {
            return Err(invalid("membership level is out of range"));
        }
        Ok(&self.observations[self.offsets[level]..self.offsets[level + 1]])
    }
    pub fn validate(&self, codes: &[u32]) -> Result<()> {
        if self.offsets.is_empty()
            || self.offsets[0] != 0
            || self.offsets.last() != Some(&codes.len())
            || self.observations.len() != codes.len()
            || self.offsets.windows(2).any(|x| x[0] > x[1])
        {
            return Err(invalid("membership offsets do not cover the row axis"));
        }
        let mut count = 0usize;
        for level in 0..self.n_levels() {
            let members = self.members(level)?;
            if members.windows(2).any(|x| x[0] >= x[1])
                || members
                    .iter()
                    .any(|&row| codes.get(row).copied() != Some(level as u32))
            {
                return Err(invalid("membership rows disagree with categorical codes"));
            }
            count += members.len();
        }
        if count != codes.len() {
            return Err(invalid("membership does not cover every row exactly once"));
        }
        Ok(())
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn allocation() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "membership dimensions overflow")
}
