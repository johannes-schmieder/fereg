//! The full formulation's fixed geometry: each effect's coefficient offset
//! and how the adjoint `D' t` sums observations into each effect's levels.
//!
//! Internally an effect's coefficients are level-major: channel `c` of
//! level `l` is coefficient `offset + l * channels + c`, so a level's
//! channels are adjacent (one effect with one channel: `offset + l`). The
//! public layouts are channel-major and are converted on input and output.

use super::{filled, with_capacity};
use crate::design::Design;
use crate::elimination::structure::eliminated_effect;
use crate::elimination::Cancel;
use crate::error::{Error, Result};
use rayon::prelude::*;

/// Effects with at most this many levels sum the adjoint by scattering fixed
/// row blocks into private accumulators, merged in block order; larger
/// effects gather each level's rows in ascending order.
const SCATTER_LEVELS: usize = 1024;
/// Scatter blocks: at most this many ...
const MAX_SCATTER_BLOCKS: usize = 64;
/// ... of at least this many rows (both fixed by the observation count, so
/// results never depend on the worker count).
const MIN_SCATTER_ROWS: usize = 1 << 14;

/// The level index of one gathered effect.
pub(super) struct Gathered {
    pub(super) effect: usize,
    /// Level `l` owns `rows[starts[l]..starts[l + 1]]`, in ascending order.
    pub(super) starts: Vec<usize>,
    pub(super) rows: Vec<u32>,
}

/// Coefficient offsets and adjoint geometry of one design (independent of
/// weights and columns).
pub(crate) struct Structure {
    pub(super) nobs: usize,
    /// Each effect's first coefficient; the last entry is the coefficient
    /// count `L` (levels times channels, summed over the effects).
    pub(super) offsets: Vec<usize>,
    /// Whether an effect has slopes or no intercept (the observation passes
    /// then read each effect's channels).
    pub(super) sloped: bool,
    /// The scattered effects and their levels' first slots in one scatter
    /// accumulator (with a trailing total).
    pub(super) scattered: Vec<usize>,
    pub(super) scatter_offsets: Vec<usize>,
    /// Rows per scatter block.
    pub(super) scatter_rows: usize,
    pub(super) gathered: Vec<Gathered>,
    /// The effect with the most levels: the column's spread about its level
    /// means sets the recurrence goal, as for the elimination.
    pub(super) largest: usize,
    pub(super) bytes: u128,
    /// Bytes of the channel blocks of the weight state (designs with
    /// slopes; see [`super::blocks`]).
    pub(super) block_bytes: u128,
}

impl Structure {
    /// The coefficient count `L`.
    pub(crate) fn coefficients(&self) -> usize {
        self.offsets[self.offsets.len() - 1]
    }

    /// Slots of one scatter accumulator.
    pub(super) fn scatter_levels(&self) -> usize {
        self.scatter_offsets[self.scatter_offsets.len() - 1]
    }

    pub(super) fn scatter_blocks(&self) -> usize {
        if self.scattered.is_empty() {
            0
        } else {
            self.nobs.div_ceil(self.scatter_rows)
        }
    }

    /// Build the geometry, checking every code against its level count;
    /// `None` when the design has no effect, no observation or more rows
    /// than 32-bit row indices address.
    pub(crate) fn build(design: &Design<'_>, cancel: &Cancel<'_>) -> Result<Option<Self>> {
        let effects = design.effects();
        let n = design.nobs();
        if effects.is_empty() || n == 0 || n >= u32::MAX as usize {
            return Ok(None);
        }
        let mut offsets = with_capacity(effects.len() + 1)?;
        let mut total = 0_usize;
        for effect in effects {
            offsets.push(total);
            total = effect
                .levels()
                .checked_mul(effect.channels())
                .and_then(|count| total.checked_add(count))
                .ok_or(Error::InvalidInput("the coefficient count overflows"))?;
        }
        offsets.push(total);
        if total == 0 {
            return Err(Error::InvalidInput("FE code exceeds its level count"));
        }
        let small = |e: &usize| effects[*e].levels() <= SCATTER_LEVELS;
        let mut scattered = with_capacity(effects.len())?;
        scattered.extend((0..effects.len()).filter(small));
        let mut scatter_offsets = with_capacity(scattered.len() + 1)?;
        let mut slots = 0;
        for &e in &scattered {
            scatter_offsets.push(slots);
            slots += effects[e].levels() * effects[e].channels();
        }
        scatter_offsets.push(slots);
        // The scattered effects' codes are checked here, the gathered ones'
        // while their level index is sorted.
        for &e in &scattered {
            cancel.check()?;
            let levels = effects[e].levels();
            let valid = effects[e]
                .codes()
                .par_chunks(1 << 16)
                .all(|codes| codes.iter().all(|&code| (code as usize) < levels));
            if !valid {
                return Err(Error::InvalidInput("FE code exceeds its level count"));
            }
        }
        let mut gathered = with_capacity(effects.len() - scattered.len())?;
        for e in (0..effects.len()).filter(|e| !small(e)) {
            gathered.push(Gathered {
                effect: e,
                starts: filled(effects[e].levels() + 1, 0)?,
                rows: filled(n, 0)?,
            });
        }
        cancel.check()?;
        gathered
            .par_iter_mut()
            .try_for_each(|index| sort_levels(effects[index.effect].codes(), index))?;
        cancel.check()?;
        let bytes = gathered
            .iter()
            .map(|g| g.starts.capacity() as u128 * 8 + g.rows.capacity() as u128 * 4)
            .sum::<u128>()
            + (offsets.capacity() + scattered.capacity() + scatter_offsets.capacity()) as u128 * 8;
        let mut st = Self {
            nobs: n,
            offsets,
            sloped: design.sloped(),
            scattered,
            scatter_offsets,
            scatter_rows: n.div_ceil(MAX_SCATTER_BLOCKS).max(MIN_SCATTER_ROWS),
            gathered,
            largest: eliminated_effect(design).expect("nonempty design"),
            bytes,
            block_bytes: 0,
        };
        st.block_bytes = super::blocks::block_bytes(&st, effects);
        Ok(Some(st))
    }
}

/// Stable counting sort of one effect's codes into its level index.
pub(super) fn sort_levels(codes: &[u32], index: &mut Gathered) -> Result<()> {
    let levels = index.starts.len() - 1;
    let starts = &mut index.starts;
    for &code in codes {
        let code = code as usize;
        if code >= levels {
            return Err(Error::InvalidInput("FE code exceeds its level count"));
        }
        starts[code + 1] += 1;
    }
    for level in 0..levels {
        starts[level + 1] += starts[level];
    }
    let mut cursor = filled(levels, 0_usize)?;
    cursor.copy_from_slice(&starts[..levels]);
    for (row, &code) in codes.iter().enumerate() {
        let slot = &mut cursor[code as usize];
        index.rows[*slot] = row as u32;
        *slot += 1;
    }
    Ok(())
}
