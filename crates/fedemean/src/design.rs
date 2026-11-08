//! Borrowed categorical fixed-effect designs.

use crate::error::{Error, Result};

/// Observations of each level of one effect, in compressed sparse row form:
/// level `l` owns `rows[offsets[l]..offsets[l + 1]]`.
///
/// Precondition (not rechecked row by row): the rows of level `l` are exactly
/// the observations whose code is `l`, each listed once, in ascending order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Members<'a> {
    offsets: &'a [usize],
    rows: &'a [usize],
}

impl<'a> Members<'a> {
    /// Checks the offsets (nonempty, starting at zero, nondecreasing, ending
    /// at `rows.len()`) in one pass over the levels.
    pub fn new(offsets: &'a [usize], rows: &'a [usize]) -> Result<Self> {
        let (Some(&first), Some(&last)) = (offsets.first(), offsets.last()) else {
            return Err(Error::InvalidInput(
                "level members need at least one offset",
            ));
        };
        if first != 0 || last != rows.len() || offsets.windows(2).any(|w| w[0] > w[1]) {
            return Err(Error::InvalidInput("level member offsets are inconsistent"));
        }
        Ok(Self { offsets, rows })
    }

    #[must_use]
    pub fn levels(&self) -> usize {
        self.offsets.len() - 1
    }

    /// The observations of `level`.
    #[must_use]
    #[inline]
    pub fn level(&self, level: usize) -> &'a [usize] {
        &self.rows[self.offsets[level]..self.offsets[level + 1]]
    }

    #[must_use]
    pub fn offsets(&self) -> &'a [usize] {
        self.offsets
    }
}

/// Owned members built when the caller supplies none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedMembers {
    offsets: Vec<usize>,
    rows: Vec<usize>,
}

impl OwnedMembers {
    /// Stable counting sort of `codes`.
    pub(crate) fn build(codes: &[u32], levels: usize) -> Result<Self> {
        let mut offsets = filled(levels + 1, 0_usize)?;
        for &code in codes {
            let code = code as usize;
            if code >= levels {
                return Err(Error::InvalidInput("FE code exceeds its level count"));
            }
            offsets[code + 1] += 1;
        }
        for level in 0..levels {
            offsets[level + 1] += offsets[level];
        }
        let mut cursor = filled(levels, 0_usize)?;
        cursor.copy_from_slice(&offsets[..levels]);
        let mut rows = filled(codes.len(), 0_usize)?;
        for (row, &code) in codes.iter().enumerate() {
            let slot = &mut cursor[code as usize];
            rows[*slot] = row;
            *slot += 1;
        }
        Ok(Self { offsets, rows })
    }

    pub(crate) fn view(&self) -> Members<'_> {
        Members {
            offsets: &self.offsets,
            rows: &self.rows,
        }
    }
}

fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::OutOfMemory("unable to reserve level members"))?;
    values.resize(count, value);
    Ok(values)
}

/// One categorical intercept effect: a code in `0..levels` per observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect<'a> {
    codes: &'a [u32],
    levels: usize,
    members: Option<Members<'a>>,
}

impl<'a> Effect<'a> {
    /// Codes are checked against `levels` inside the elimination's structure
    /// pass, which reads every code anyway.
    #[must_use]
    pub const fn new(codes: &'a [u32], levels: usize) -> Self {
        Self {
            codes,
            levels,
            members: None,
        }
    }

    /// Attach the effect's level members (only the eliminated effect's are
    /// read; without them the projector builds its own).
    pub fn with_members(mut self, members: Members<'a>) -> Result<Self> {
        if members.levels() != self.levels || members.rows.len() != self.codes.len() {
            return Err(Error::InvalidInput(
                "level members do not match their effect",
            ));
        }
        self.members = Some(members);
        Ok(self)
    }

    #[must_use]
    #[inline]
    pub const fn codes(&self) -> &'a [u32] {
        self.codes
    }

    #[must_use]
    #[inline]
    pub const fn levels(&self) -> usize {
        self.levels
    }

    #[must_use]
    pub const fn members(&self) -> Option<Members<'a>> {
        self.members
    }
}

/// A categorical intercept design over `nobs` observations. The effect table
/// is allocated once, when the design is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Design<'a> {
    nobs: usize,
    effects: Vec<Effect<'a>>,
}

impl<'a> Design<'a> {
    pub fn new(nobs: usize, effects: Vec<Effect<'a>>) -> Result<Self> {
        if effects.iter().any(|effect| effect.codes.len() != nobs) {
            return Err(Error::InvalidInput(
                "fixed-effect codes do not share the observation count",
            ));
        }
        Ok(Self { nobs, effects })
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub fn effects(&self) -> &[Effect<'a>] {
        &self.effects
    }
}
