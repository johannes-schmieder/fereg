//! Borrowed categorical fixed-effect designs.

use crate::error::{Error, Result};
use rayon::prelude::*;

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

/// One categorical effect: a code in `0..levels` per observation and its
/// channels. An intercept effect ([`Effect::new`]) has one channel, the
/// level dummy. An effect with varying slopes ([`Effect::with_slopes`]) has
/// an optional intercept channel followed by one channel per slope
/// variable: observation `i` of level `l` contributes `beta_{l,0}` (the
/// intercept) plus `x_s[i] beta_{l,s}` for every slope `s`.
///
/// Its coefficients are channel-major: channel `c` of level `l` is at
/// `c * levels + l` within the effect (see [`crate::CoefficientLayout`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Effect<'a> {
    codes: &'a [u32],
    levels: usize,
    members: Option<Members<'a>>,
    intercept: bool,
    slopes: Vec<&'a [f64]>,
}

impl<'a> Effect<'a> {
    /// An intercept effect. The retained effects' codes are checked against
    /// `levels` inside the elimination's structure pass, which reads every
    /// code anyway. The eliminated effect's codes are checked when the
    /// projector builds its members; caller-supplied [`Members`] must match
    /// the codes (see there), otherwise a projection may panic or return
    /// wrong residuals.
    #[must_use]
    pub const fn new(codes: &'a [u32], levels: usize) -> Self {
        Self {
            codes,
            levels,
            members: None,
            intercept: true,
            slopes: Vec::new(),
        }
    }

    /// Varying slopes: the effect's channels become its intercept (when
    /// `intercept`) followed by one channel per slope column, in order.
    /// Every slope column has one finite value per observation, and the
    /// effect keeps at least one channel. Slopes are supported by the full
    /// formulation ([`crate::Reduction::Full`]).
    pub fn with_slopes<I>(mut self, intercept: bool, slopes: I) -> Result<Self>
    where
        I: IntoIterator<Item = &'a [f64]>,
    {
        let mut columns: Vec<&'a [f64]> = Vec::new();
        for slope in slopes {
            columns
                .try_reserve(1)
                .map_err(|_| Error::OutOfMemory("unable to reserve the slope columns"))?;
            columns.push(slope);
        }
        if !intercept && columns.is_empty() {
            return Err(Error::InvalidInput(
                "an effect needs an intercept or a slope",
            ));
        }
        if columns.iter().any(|slope| slope.len() != self.codes.len()) {
            return Err(Error::InvalidInput(
                "slope values do not match their effect's observations",
            ));
        }
        let finite = |slope: &&[f64]| {
            slope
                .par_chunks(1 << 16)
                .all(|values| values.iter().all(|v| v.is_finite()))
        };
        if !columns.iter().all(finite) {
            return Err(Error::InvalidData("slope values must be finite"));
        }
        self.intercept = intercept;
        self.slopes = columns;
        Ok(self)
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

    /// Whether the effect has an intercept channel (its first).
    #[must_use]
    #[inline]
    pub const fn intercept(&self) -> bool {
        self.intercept
    }

    /// The slope columns, after the intercept channel.
    #[must_use]
    #[inline]
    pub fn slopes(&self) -> &[&'a [f64]] {
        &self.slopes
    }

    /// Channels per level: the intercept (if any) plus the slopes.
    #[must_use]
    #[inline]
    pub fn channels(&self) -> usize {
        usize::from(self.intercept) + self.slopes.len()
    }

    /// Whether the effect is anything but one intercept channel.
    #[must_use]
    #[inline]
    pub(crate) fn sloped(&self) -> bool {
        !self.slopes.is_empty() || !self.intercept
    }
}

/// A categorical design over `nobs` observations. The effect table is
/// allocated once, when the design is built.
#[derive(Debug, Clone, PartialEq)]
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

    /// Whether any effect has slopes (or lacks an intercept).
    #[must_use]
    pub(crate) fn sloped(&self) -> bool {
        self.effects.iter().any(Effect::sloped)
    }
}
