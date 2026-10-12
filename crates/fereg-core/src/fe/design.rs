use crate::error::{allocation, ErrorCode, FeregError, Result};
use crate::memory::SystemVecReservation as SystemReservation;
use crate::memory::VecReservation as LevelIndexReservation;
use rayon::prelude::*;

/// Range checks have a single Boolean result and do not allocate or reduce
/// floating-point values. Direct callers never create a global worker pool.
pub(super) fn codes_in_range(codes: &[u32], levels: usize) -> bool {
    let workers = rayon::current_thread_index().map_or(1, |_| rayon::current_num_threads());
    let plan = crate::parallel::StagePlan::new(codes.len() as u128, codes.len(), workers);
    let valid = |chunk: &[u32]| chunk.iter().all(|&code| (code as usize) < levels);
    if plan.lanes > 1 {
        codes.par_chunks(plan.chunk(codes.len())).all(valid)
    } else {
        valid(codes)
    }
}

/// Compressed observation membership for one categorical effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelIndex {
    offsets: Vec<usize>,
    observations: Vec<usize>,
}

impl LevelIndex {
    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.offsets)
            + crate::memory::vec_capacity_bytes(&self.observations)
    }

    pub fn from_codes(codes: &[u32], n_levels: usize) -> Result<Self> {
        let mut reservation = SystemReservation;
        Self::from_codes_with_reservation(codes, n_levels, &mut reservation)
    }

    /// Maximum optional transient preparation scratch. The caller admits this
    /// beside existing owners; source constructions consume it sequentially.
    #[must_use]
    pub fn parallel_scratch_bytes(rows: usize) -> u128 {
        (rows as u128 * 8).min(128 * 1024 * 1024)
    }

    /// Stable membership with caller-admitted optional scratch. Low-memory and
    /// unadmitted callers keep `from_codes`; every declined attempt releases
    /// all temporary owners before the original constructor runs.
    pub fn from_codes_with_scratch(codes: &[u32], n_levels: usize, admitted: u128) -> Result<Self> {
        let mut reservation = SystemReservation;
        Self::with_scratch_reservation(codes, n_levels, admitted, &mut reservation)
    }

    fn with_scratch_reservation<R: LevelIndexReservation>(
        codes: &[u32],
        n_levels: usize,
        admitted: u128,
        reservation: &mut R,
    ) -> Result<Self> {
        let workers = rayon::current_thread_index().map_or(1, |_| rayon::current_num_threads());
        let per_block = n_levels as u128
            * (std::mem::size_of::<usize>() + std::mem::size_of::<&mut [usize]>()) as u128
            + std::mem::size_of::<Vec<&mut [usize]>>() as u128;
        let budget = admitted.min(Self::parallel_scratch_bytes(codes.len()));
        // Bound serial histogram/descriptor construction to an eighth of N.
        let lanes = crate::parallel::StagePlan::new(codes.len() as u128, codes.len(), workers)
            .lanes
            .min(codes.len() / n_levels.max(1) / 8)
            .min(usize::try_from(budget / per_block).unwrap_or(usize::MAX));
        // The extra count/scatter passes and descriptor construction need at
        // least four useful blocks to amortize their setup. Smaller schedules
        // keep the original constructor, even if more memory is available.
        if n_levels > 0 && lanes >= 4 {
            if let Some(index) = parallel_level_index(codes, n_levels, lanes, budget, reservation) {
                return Ok(index);
            }
        }
        Self::from_codes_with_reservation(codes, n_levels, reservation)
    }

    /// Ordered codes already have the final membership order. Reject the
    /// optional route before allocation when that identity does not hold.
    pub(crate) fn try_from_ordered_codes(codes: &[u32], n_levels: usize) -> Result<Option<Self>> {
        if codes.last().is_some_and(|code| *code as usize >= n_levels)
            || codes.windows(2).any(|pair| pair[0] > pair[1])
        {
            return Ok(None);
        }
        let offset_count = n_levels
            .checked_add(1)
            .ok_or_else(|| allocation("fixed-effect level-index offset count overflows"))?;
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(offset_count)
            .map_err(|_| allocation("unable to reserve fixed-effect level offsets"))?;
        offsets.push(0);
        for (observation, &code) in codes.iter().enumerate() {
            while offsets.len() <= code as usize {
                offsets.push(observation);
            }
        }
        offsets.resize(offset_count, codes.len());
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(codes.len())
            .map_err(|_| allocation("unable to reserve fixed-effect level observations"))?;
        observations.extend(0..codes.len());
        Ok(Some(Self {
            offsets,
            observations,
        }))
    }

    fn from_codes_with_reservation<R: LevelIndexReservation>(
        codes: &[u32],
        n_levels: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        let offset_count = n_levels
            .checked_add(1)
            .ok_or_else(|| allocation("fixed-effect level-index offset count overflows"))?;
        let mut counts = reserved_filled(
            reservation,
            n_levels,
            0_usize,
            "unable to reserve fixed-effect level counts",
        )?;
        for code in codes {
            let level = *code as usize;
            if level >= n_levels {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    format!("fixed-effect code {level} exceeds level count {n_levels}"),
                ));
            }
            counts[level] = counts[level]
                .checked_add(1)
                .ok_or_else(|| allocation("fixed-effect level membership count overflows"))?;
        }
        let mut offsets = reserved_filled(
            reservation,
            offset_count,
            0_usize,
            "unable to reserve fixed-effect level offsets",
        )?;
        for level in 0..n_levels {
            offsets[level + 1] = offsets[level]
                .checked_add(counts[level])
                .ok_or_else(|| allocation("fixed-effect cumulative membership count overflows"))?;
        }
        if offsets[n_levels] != codes.len() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "fixed-effect level membership count is inconsistent with codes",
            ));
        }
        let mut next = Vec::new();
        reservation.reserve_vec(
            &mut next,
            n_levels,
            "unable to reserve fixed-effect level cursors",
        )?;
        next.extend_from_slice(&offsets[..n_levels]);
        let mut observations = reserved_filled(
            reservation,
            codes.len(),
            0_usize,
            "unable to reserve fixed-effect level observations",
        )?;
        for (observation, code) in codes.iter().enumerate() {
            let level = *code as usize;
            let destination = next[level];
            if destination >= observations.len() {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "fixed-effect level cursor exceeds membership storage",
                ));
            }
            observations[destination] = observation;
            next[level] = destination
                .checked_add(1)
                .ok_or_else(|| allocation("fixed-effect level cursor overflows"))?;
        }
        Ok(Self {
            offsets,
            observations,
        })
    }

    #[must_use]
    pub fn members(&self, level: usize) -> &[usize] {
        &self.observations[self.offsets[level]..self.offsets[level + 1]]
    }

    #[must_use]
    pub fn n_levels(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    #[must_use]
    pub fn nobs(&self) -> usize {
        self.observations.len()
    }

    pub(crate) fn membership_offsets(&self) -> &[usize] {
        &self.offsets
    }

    fn storage_capacities(&self) -> (usize, usize) {
        (self.offsets.capacity(), self.observations.capacity())
    }

    pub fn checked_members(&self, level: usize) -> Result<&[usize]> {
        Ok(&self.observations[self.checked_member_range(level)?])
    }

    pub(crate) fn checked_member_range(&self, level: usize) -> Result<std::ops::Range<usize>> {
        let start = self.offsets.get(level).copied().ok_or_else(|| {
            FeregError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect level index is out of range",
            )
        })?;
        let end = self.offsets.get(level + 1).copied().ok_or_else(|| {
            FeregError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect level index is out of range",
            )
        })?;
        self.observations.get(start..end).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "fixed-effect membership offsets are inconsistent",
            )
        })?;
        Ok(start..end)
    }
}

fn optional_level_storage<T, R: LevelIndexReservation>(
    count: usize,
    reservation: &mut R,
) -> Option<Vec<T>> {
    let mut values = Vec::new();
    reservation
        .reserve_vec(
            &mut values,
            count,
            "unable to reserve parallel fixed-effect storage",
        )
        .ok()?;
    Some(values)
}

fn parallel_level_index<R: LevelIndexReservation>(
    codes: &[u32],
    levels: usize,
    blocks: usize,
    admitted: u128,
    reservation: &mut R,
) -> Option<LevelIndex> {
    let cells = levels.checked_mul(blocks)?;
    let mut counts = optional_level_storage::<usize, _>(cells, reservation)?;
    let mut destinations: Vec<Vec<&mut [usize]>> = optional_level_storage(blocks, reservation)?;
    for _ in 0..blocks {
        destinations.push(optional_level_storage(levels, reservation)?);
    }
    let scratch = crate::memory::vec_capacity_bytes(&counts)
        + crate::memory::vec_capacity_bytes(&destinations)
        + destinations
            .iter()
            .map(crate::memory::vec_capacity_bytes)
            .sum::<u128>();
    if scratch > admitted {
        return None;
    }
    counts.resize(cells, 0);
    let chunk = codes.len().div_ceil(blocks).max(1);
    let valid = codes
        .par_chunks(chunk)
        .zip(counts.par_chunks_mut(levels))
        .map(|(rows, counts)| {
            for &code in rows {
                let Some(count) = counts.get_mut(code as usize) else {
                    return false;
                };
                *count += 1;
            }
            true
        })
        .all(|valid| valid);
    if !valid {
        return None;
    }
    let mut offsets = optional_level_storage(levels.checked_add(1)?, reservation)?;
    offsets.push(0_usize);
    for level in 0..levels {
        let size: usize = (0..blocks)
            .map(|block| counts[block * levels + level])
            .sum();
        offsets.push(offsets[level].checked_add(size)?);
    }
    let mut observations = optional_level_storage(codes.len(), reservation)?;
    observations.resize(codes.len(), 0_usize);
    let mut tail = observations.as_mut_slice();
    for level in 0..levels {
        for block in 0..blocks {
            let (segment, rest) = tail.split_at_mut(counts[block * levels + level]);
            destinations[block].push(segment);
            tail = rest;
        }
    }
    debug_assert!(tail.is_empty());
    destinations
        .into_par_iter()
        .zip(counts.par_chunks_mut(levels))
        .zip(codes.par_chunks(chunk))
        .enumerate()
        .for_each(|(block, ((mut destinations, cursors), rows))| {
            cursors.fill(0);
            for (offset, &code) in rows.iter().enumerate() {
                let level = code as usize;
                destinations[level][cursors[level]] = block * chunk + offset;
                cursors[level] += 1;
            }
        });
    Some(LevelIndex {
        offsets,
        observations,
    })
}

/// One intercept-only fixed-effect term borrowed from validated prepared data.
///
/// This view never owns or copies observation-scale codes or level membership.
/// Its deliberately narrow type prevents the current preparation boundary from
/// silently representing slope-bearing effects as intercept-only effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowedInterceptEffect<'a> {
    codes: &'a [u32],
    n_levels: usize,
    level_index: &'a LevelIndex,
}

/// A unique prepared FE source after one complete structural scan.
///
/// Construction is O(N) in the source observation count. Producing any number
/// of wire-term views from this certificate is O(1) per term.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ValidatedBorrowedInterceptSource<'a> {
    codes: &'a [u32],
    n_levels: usize,
    level_index: &'a LevelIndex,
}

impl<'a> ValidatedBorrowedInterceptSource<'a> {
    /// The owner has checked the exact partition and membership together and
    /// exposes neither mutation nor replacement of either buffer.
    pub(crate) fn from_finalized_single_effect(
        owner: &'a super::topology_finalize::ValidatedFinalizedTopology,
    ) -> Option<Self> {
        let topology = owner.get();
        if topology.effect_to_topology().len() != 1 {
            return None;
        }
        Self::from_finalized_source(owner, topology.effect_to_topology()[0])
    }

    /// Codes and membership belong to the same immutable validated owner.
    pub(crate) fn from_finalized_source(
        owner: &'a super::topology_finalize::ValidatedFinalizedTopology,
        topology_index: usize,
    ) -> Option<Self> {
        let topology = owner.get();
        let source = topology.registry().sources().get(topology_index)?;
        let partition = source.partition();
        let codes = if partition.is_interaction() {
            partition.owned_interaction_codes()?
        } else {
            let indices = partition.categorical_source_indices();
            if indices.len() != 1 {
                return None;
            }
            topology.compacted_sources().get(indices[0])?.codes()
        };
        Some(Self {
            codes,
            n_levels: source.n_levels(),
            level_index: source.level_index(),
        })
    }

    pub(crate) fn from_finalized_owner(
        owner: &'a super::topology_finalize::ValidatedFinalizedTopology,
    ) -> Option<Self> {
        let topology = owner.get();
        if topology.compacted_sources().len() != 1
            || topology.registry().sources().len() != 1
            || topology.effect_to_topology().len() != 1
        {
            return None;
        }
        let source = &topology.registry().sources()[0];
        if source.partition().is_interaction()
            || source.partition().categorical_source_indices() != [0]
        {
            return None;
        }
        Some(Self {
            codes: topology.compacted_sources()[0].codes(),
            n_levels: source.n_levels(),
            level_index: source.level_index(),
        })
    }

    pub(crate) fn new(
        codes: &'a [u32],
        n_levels: usize,
        level_index: &'a LevelIndex,
    ) -> Result<Self> {
        if codes.len() != level_index.nobs() || n_levels != level_index.n_levels() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "borrowed fixed-effect codes and membership dimensions differ",
            ));
        }
        if codes.is_empty() != (n_levels == 0) {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "borrowed fixed-effect empty observation and level counts disagree",
            ));
        }
        if !codes_in_range(codes, n_levels) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "borrowed fixed-effect code exceeds its declared level count",
            ));
        }

        let validate_level = |level| {
            let members = level_index.checked_members(level)?;
            if members.is_empty() {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "borrowed fixed-effect membership contains an empty final level",
                ));
            }
            for observation in members {
                if *observation >= codes.len() || codes[*observation] as usize != level {
                    return Err(FeregError::new(
                        ErrorCode::InvalidArgument,
                        "borrowed fixed-effect membership does not match its codes",
                    ));
                }
            }
            Ok(())
        };
        let workers = rayon::current_thread_index().map_or(1, |_| rayon::current_num_threads());
        if crate::parallel::StagePlan::new(codes.len() as u128, n_levels, workers).lanes > 1 {
            // Ordered joins retain the first failing level's original error.
            crate::parallel::stage_try_for_each(
                (0..n_levels).into_par_iter(),
                codes.len() as u128,
                validate_level,
            )?;
        } else {
            (0..n_levels).try_for_each(validate_level)?;
        }
        // Accepted adjacent ranges share endpoints and lie within observations.
        // Their lengths telescope; no accepted prefix can overflow usize.
        let membership_count = if n_levels == 0 {
            0
        } else {
            level_index.offsets[n_levels] - level_index.offsets[0]
        };
        if membership_count != codes.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "borrowed fixed-effect membership does not cover its observation axis",
            ));
        }

        Ok(Self {
            codes,
            n_levels,
            level_index,
        })
    }

    #[must_use]
    pub(crate) const fn effect(&self) -> BorrowedInterceptEffect<'a> {
        BorrowedInterceptEffect {
            codes: self.codes,
            n_levels: self.n_levels,
            level_index: self.level_index,
        }
    }
}

impl<'a> BorrowedInterceptEffect<'a> {
    #[cfg(test)]
    pub(crate) fn new(
        codes: &'a [u32],
        n_levels: usize,
        level_index: &'a LevelIndex,
    ) -> Result<Self> {
        Ok(ValidatedBorrowedInterceptSource::new(codes, n_levels, level_index)?.effect())
    }

    #[must_use]
    pub const fn codes(&self) -> &'a [u32] {
        self.codes
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }

    #[must_use]
    pub const fn level_index(&self) -> &'a LevelIndex {
        self.level_index
    }
}

/// Validated intercept-only FE design borrowing all observation-scale state.
///
/// The effect table is small, wire ordered, and owned by the view. Codes and
/// compressed membership remain owned by the prepared-data object.
#[derive(Debug, PartialEq, Eq)]
pub struct BorrowedInterceptFeDesign<'a> {
    nobs: usize,
    effects: Vec<BorrowedInterceptEffect<'a>>,
    total_term_slots: usize,
}

impl<'a> BorrowedInterceptFeDesign<'a> {
    pub(crate) fn new(nobs: usize, effects: Vec<BorrowedInterceptEffect<'a>>) -> Result<Self> {
        if effects.iter().any(|effect| effect.codes().len() != nobs) {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "borrowed fixed-effect terms do not share the declared observation count",
            ));
        }
        let total_term_slots =
            checked_intercept_term_slots(effects.iter().map(BorrowedInterceptEffect::n_levels))?;
        Ok(Self {
            nobs,
            effects,
            total_term_slots,
        })
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub fn effects(&self) -> &[BorrowedInterceptEffect<'a>] {
        &self.effects
    }

    pub(crate) fn descriptor_bytes(&self) -> u128 {
        self.effects.capacity() as u128 * std::mem::size_of::<BorrowedInterceptEffect<'_>>() as u128
    }

    /// Total intercept coefficient slots before identification normalization.
    #[must_use]
    pub const fn total_term_slots(&self) -> usize {
        self.total_term_slots
    }
}

fn checked_intercept_term_slots(level_counts: impl IntoIterator<Item = usize>) -> Result<usize> {
    level_counts.into_iter().try_fold(0_usize, |total, levels| {
        total
            .checked_add(levels)
            .ok_or_else(|| allocation("borrowed fixed-effect term-slot count overflows"))
    })
}

/// Rank retained categorical effects without changing their canonical order.
///
/// Solver-specific graph roles use descending retained level count. Original
/// effect position is the deterministic tie-breaker, while coefficient and
/// result storage continue to follow the caller's original effect order.
pub(crate) fn retained_effect_order_by_levels(
    effect_count: usize,
    mut level_count: impl FnMut(usize) -> usize,
) -> Result<Vec<usize>> {
    let mut order = Vec::new();
    order
        .try_reserve_exact(effect_count)
        .map_err(|_| allocation("unable to reserve retained fixed-effect role order"))?;
    order.extend(0..effect_count);
    order.sort_unstable_by(|left, right| {
        level_count(*right)
            .cmp(&level_count(*left))
            .then_with(|| left.cmp(right))
    });
    Ok(order)
}

fn reserved_filled<T: Clone, R: LevelIndexReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

/// One categorical fixed-effect term with optional group-specific slopes.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectTerm {
    codes: Vec<u32>,
    n_levels: usize,
    intercept: bool,
    slopes: Vec<Vec<f64>>,
    level_index: LevelIndex,
}

impl EffectTerm {
    pub fn new(
        codes: Vec<u32>,
        n_levels: usize,
        intercept: bool,
        slopes: Vec<Vec<f64>>,
    ) -> Result<Self> {
        if n_levels == 0 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect term must contain at least one level",
            ));
        }
        if !intercept && slopes.is_empty() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect term must contain an intercept or slope",
            ));
        }
        if slopes.iter().any(|slope| slope.len() != codes.len()) {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect slope length does not match codes",
            ));
        }
        if slopes
            .iter()
            .flat_map(|slope| slope.iter())
            .any(|value| !value.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "fixed-effect slopes must be finite",
            ));
        }
        let level_index = LevelIndex::from_codes(&codes, n_levels)?;
        Ok(Self {
            codes,
            n_levels,
            intercept,
            slopes,
            level_index,
        })
    }

    #[must_use]
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }

    #[must_use]
    pub const fn has_intercept(&self) -> bool {
        self.intercept
    }

    #[must_use]
    pub fn slopes(&self) -> &[Vec<f64>] {
        &self.slopes
    }

    #[must_use]
    pub fn level_index(&self) -> &LevelIndex {
        &self.level_index
    }

    #[must_use]
    pub fn channels(&self) -> usize {
        (if self.intercept { 1 } else { 0 }) + self.slopes.len()
    }

    #[must_use]
    pub fn basis_value(&self, channel: usize, observation: usize) -> f64 {
        if self.intercept {
            if channel == 0 {
                1.0
            } else {
                self.slopes[channel - 1][observation]
            }
        } else {
            self.slopes[channel][observation]
        }
    }
}

/// Validated collection of fixed-effect terms sharing one observation axis.
#[derive(Debug, Clone, PartialEq)]
pub struct FeDesign {
    nobs: usize,
    effects: Vec<EffectTerm>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FeDesignStorageCapacities {
    pub(crate) effects: usize,
    pub(crate) codes: usize,
    pub(crate) slope_vectors: usize,
    pub(crate) slope_values: usize,
    pub(crate) level_offsets: usize,
    pub(crate) level_observations: usize,
    pub(crate) resident_bytes: u128,
}

impl FeDesign {
    pub fn new(nobs: usize, effects: Vec<EffectTerm>) -> Result<Self> {
        if effects.iter().any(|effect| effect.codes().len() != nobs) {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect terms do not share the declared observation count",
            ));
        }
        Ok(Self { nobs, effects })
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub fn effects(&self) -> &[EffectTerm] {
        &self.effects
    }

    #[must_use]
    pub fn max_channels(&self) -> usize {
        self.effects
            .iter()
            .map(EffectTerm::channels)
            .max()
            .unwrap_or(0)
    }

    /// Total coefficient slots before identification normalization.
    #[must_use]
    pub fn total_coefficients(&self) -> usize {
        self.effects
            .iter()
            .map(|effect| effect.n_levels().saturating_mul(effect.channels()))
            .sum()
    }

    pub(crate) fn storage_capacities(&self) -> FeDesignStorageCapacities {
        let mut codes = 0_usize;
        let mut slope_vectors = 0_usize;
        let mut slope_values = 0_usize;
        let mut level_offsets = 0_usize;
        let mut level_observations = 0_usize;
        for effect in &self.effects {
            codes = codes.saturating_add(effect.codes.capacity());
            slope_vectors = slope_vectors.saturating_add(effect.slopes.capacity());
            for slope in &effect.slopes {
                slope_values = slope_values.saturating_add(slope.capacity());
            }
            let (offsets, observations) = effect.level_index.storage_capacities();
            level_offsets = level_offsets.saturating_add(offsets);
            level_observations = level_observations.saturating_add(observations);
        }
        let resident_bytes = (self.effects.capacity() as u128)
            .saturating_mul(std::mem::size_of::<EffectTerm>() as u128)
            .saturating_add((codes as u128).saturating_mul(std::mem::size_of::<u32>() as u128))
            .saturating_add(
                (slope_vectors as u128).saturating_mul(std::mem::size_of::<Vec<f64>>() as u128),
            )
            .saturating_add(
                (slope_values as u128).saturating_mul(std::mem::size_of::<f64>() as u128),
            )
            .saturating_add(
                (level_offsets as u128).saturating_mul(std::mem::size_of::<usize>() as u128),
            )
            .saturating_add(
                (level_observations as u128).saturating_mul(std::mem::size_of::<usize>() as u128),
            );
        FeDesignStorageCapacities {
            effects: self.effects.capacity(),
            codes,
            slope_vectors,
            slope_values,
            level_offsets,
            level_observations,
            resident_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct InjectedReservation {
        fail_at: Option<usize>,
        calls: usize,
    }

    impl LevelIndexReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected fixed-effect level-index allocation failure",
                ));
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test fixed-effect level-index reservation failed"))
        }
    }

    #[test]
    fn level_index_membership_is_exact_and_empty_safe() {
        let index = LevelIndex::from_codes(&[1, 0, 1, 2, 0], 3).expect("level index");
        assert_eq!(index.n_levels(), 3);
        assert_eq!(index.nobs(), 5);
        assert_eq!(index.checked_members(0).expect("level zero"), &[1, 4]);
        assert_eq!(index.checked_members(1).expect("level one"), &[0, 2]);
        assert_eq!(index.checked_members(2).expect("level two"), &[3]);
        assert_eq!(
            index.checked_members(3).expect_err("out of range").code,
            ErrorCode::InvalidArgument
        );

        let empty = LevelIndex::from_codes(&[], 0).expect("empty level index");
        assert_eq!(empty.n_levels(), 0);
        assert_eq!(empty.nobs(), 0);
    }

    #[test]
    fn every_level_index_reservation_fails_closed() {
        let codes = [1, 0, 1, 2, 0];
        let mut successful = InjectedReservation::default();
        LevelIndex::from_codes_with_reservation(&codes, 3, &mut successful)
            .expect("record reservations");
        assert_eq!(successful.calls, 4);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                calls: 0,
            };
            let error = LevelIndex::from_codes_with_reservation(&codes, 3, &mut injected)
                .expect_err("injected reservation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }
    }

    #[test]
    fn parallel_membership_matches_row_filter_and_every_optional_failure_falls_back() {
        let codes: Vec<_> = (0..131_073).map(|row| ((row * 73) % 11) as u32).collect();
        let reference = LevelIndex::from_codes(&codes, 14).unwrap();
        for workers in [1, 2, 3, 4, 16] {
            let team = crate::ThreadTeam::new(
                crate::ThreadConfig::from_requested(workers, false).unwrap(),
            );
            team.install(|| {
                let mut successful = InjectedReservation::default();
                let candidate =
                    LevelIndex::with_scratch_reservation(&codes, 14, u128::MAX, &mut successful)
                        .unwrap();
                assert_eq!(candidate, reference);
                for level in 0..14 {
                    let expected: Vec<_> = codes
                        .iter()
                        .enumerate()
                        .filter_map(|(row, &code)| (code as usize == level).then_some(row))
                        .collect();
                    assert_eq!(candidate.members(level), expected);
                }
                if workers >= 4 {
                    assert!(successful.calls > 4, "exercise parallel construction");
                    for fail_at in 0..successful.calls {
                        let mut injected = InjectedReservation {
                            fail_at: Some(fail_at),
                            calls: 0,
                        };
                        assert_eq!(
                            LevelIndex::with_scratch_reservation(
                                &codes,
                                14,
                                u128::MAX,
                                &mut injected
                            )
                            .unwrap(),
                            reference
                        );
                    }
                } else {
                    assert_eq!(
                        successful.calls, 4,
                        "small schedules keep serial construction"
                    );
                }
                for budget in [0, 1439, 1440, 1441, 131_073 * 8] {
                    let mut reservations = InjectedReservation::default();
                    assert_eq!(
                        LevelIndex::with_scratch_reservation(&codes, 14, budget, &mut reservations)
                            .unwrap(),
                        reference
                    );
                    assert_eq!(reservations.calls > 4, workers >= 4 && budget >= 1440);
                }
                let mut invalid = codes.clone();
                invalid[17] = 16;
                invalid[90_000] = u32::MAX;
                assert_eq!(
                    LevelIndex::from_codes_with_scratch(&invalid, 14, u128::MAX).unwrap_err(),
                    LevelIndex::from_codes(&invalid, 14).unwrap_err()
                );
            })
            .unwrap();
        }
        // Direct callers keep the serial path, even with an unlimited offer.
        assert_eq!(
            LevelIndex::from_codes_with_scratch(&codes, 14, u128::MAX).unwrap(),
            reference
        );
    }

    #[test]
    fn parallel_membership_declines_excess_actual_capacity_and_high_cardinality() {
        struct ExtraCapacity;
        impl LevelIndexReservation for ExtraCapacity {
            fn reserve_vec<T>(
                &mut self,
                target: &mut Vec<T>,
                additional: usize,
                _: &'static str,
            ) -> Result<()> {
                target
                    .try_reserve_exact(additional + 1000)
                    .map_err(|_| allocation("test reserve"))
            }
        }
        let codes: Vec<_> = (0..100_001).map(|row| (row % 7) as u32).collect();
        let team = crate::ThreadTeam::new(crate::ThreadConfig::from_requested(16, false).unwrap());
        team.install(|| {
            assert!(parallel_level_index(&codes, 7, 4, 768, &mut ExtraCapacity).is_none());
            assert_eq!(
                LevelIndex::with_scratch_reservation(&codes, 7, 768, &mut ExtraCapacity).unwrap(),
                LevelIndex::from_codes(&codes, 7).unwrap()
            );
            let mut reservations = InjectedReservation::default();
            LevelIndex::with_scratch_reservation(&codes, 50_001, u128::MAX, &mut reservations)
                .unwrap();
            assert_eq!(
                reservations.calls, 4,
                "histogram overhead declines before allocation"
            );
            // The row/level work cap admits four blocks at 3,125 levels,
            // but only three at 3,126, which must decline before allocation.
            for (levels, calls) in [(3125, 8), (3126, 4)] {
                let mut reservations = InjectedReservation::default();
                assert_eq!(
                    LevelIndex::with_scratch_reservation(
                        &codes,
                        levels,
                        u128::MAX,
                        &mut reservations
                    )
                    .unwrap(),
                    LevelIndex::from_codes(&codes, levels).unwrap()
                );
                assert_eq!(reservations.calls, calls);
            }
            for (codes, levels) in [(vec![], 0), (vec![], 3), (vec![0], 1), (vec![0], 0)] {
                assert_eq!(
                    LevelIndex::from_codes_with_scratch(&codes, levels, u128::MAX),
                    LevelIndex::from_codes(&codes, levels)
                );
            }
        })
        .unwrap();
    }

    #[test]
    fn level_index_rejects_out_of_range_codes() {
        assert_eq!(
            LevelIndex::from_codes(&[0, 2], 2)
                .expect_err("out-of-range code")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            LevelIndex::from_codes(&[], usize::MAX)
                .expect_err("offset-count overflow")
                .code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn borrowed_intercept_design_preserves_empty_effects() {
        let index = LevelIndex::from_codes(&[], 0).expect("empty level index");
        let source = ValidatedBorrowedInterceptSource::new(&[], 0, &index).expect("empty source");
        let effect = source.effect();
        let design = BorrowedInterceptFeDesign::new(0, vec![effect]).expect("empty design");
        assert_eq!(design.nobs(), 0);
        assert_eq!(design.effects().len(), 1);
        assert_eq!(design.total_term_slots(), 0);
    }

    #[test]
    fn borrowed_intercept_effect_rejects_misaligned_membership() {
        let index = LevelIndex::from_codes(&[0, 0, 1, 1], 2).expect("level index");
        let error = ValidatedBorrowedInterceptSource::new(&[0, 1, 0, 1], 2, &index)
            .expect_err("membership mismatch");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn borrowed_intercept_slot_overflow_fails_closed() {
        let error = checked_intercept_term_slots([usize::MAX, 1])
            .expect_err("term-slot overflow must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn retained_effect_roles_rank_levels_and_break_ties_by_original_position() {
        let levels = [5, 17, 9, 17, 2];
        assert_eq!(
            retained_effect_order_by_levels(levels.len(), |effect| levels[effect])
                .expect("role order"),
            vec![1, 3, 2, 0, 4]
        );
    }
}

#[cfg(test)]
mod ordered_index_tests {
    use super::*;

    #[test]
    fn ordered_index_matches_general_membership_and_rejects_before_fallback() {
        for codes in [
            vec![],
            vec![0],
            vec![0, 0, 1, 3, 3],
            vec![2, 2, 5],
            (0..16387).map(|r| (r / 5) as u32).collect(),
        ] {
            let levels = codes.last().map_or(0, |code| *code as usize + 1);
            for padding in [0, 3] {
                let count = levels + padding;
                assert_eq!(
                    LevelIndex::try_from_ordered_codes(&codes, count)
                        .unwrap()
                        .unwrap(),
                    LevelIndex::from_codes(&codes, count).unwrap()
                );
            }
        }
        for (codes, levels) in [(vec![0, 1, 0], 2), (vec![0, 3], 3), (vec![u32::MAX, 0], 2)] {
            assert_eq!(
                LevelIndex::try_from_ordered_codes(&codes, levels).unwrap(),
                None
            );
        }
        assert_eq!(
            LevelIndex::try_from_ordered_codes(&[], usize::MAX).unwrap_err(),
            LevelIndex::from_codes(&[], usize::MAX).unwrap_err()
        );
    }
}

#[cfg(test)]
mod parallel_validation_tests {
    use super::*;
    use crate::{ThreadConfig, ThreadTeam};

    #[test]
    fn code_range_checks_match_integer_bounds_at_stage_edges() {
        assert!(rayon::current_thread_index().is_none());
        for workers in [1, 2, 16] {
            let team = ThreadTeam::new(ThreadConfig::from_requested(workers, false).unwrap());
            for rows in [0_usize, 1, 16_383, 16_384, 131_073] {
                let mut codes: Vec<_> = (0..rows).map(|row| (row % 17) as u32).collect();
                assert!(codes_in_range(&codes, 17));
                assert!(team.install(|| codes_in_range(&codes, 17)).unwrap());
                for row in [0, rows / 2, rows.saturating_sub(1)] {
                    if rows == 0 {
                        continue;
                    }
                    let original = codes[row];
                    for invalid in [17, u32::MAX] {
                        codes[row] = invalid;
                        assert!(!codes_in_range(&codes, 17));
                        assert!(!team.install(|| codes_in_range(&codes, 17)).unwrap());
                    }
                    codes[row] = original;
                }
            }
        }
    }

    #[test]
    fn parallel_membership_checks_preserve_error_order_and_coverage() {
        let rows = 131_073;
        let levels = 17;
        let original: Vec<_> = (0..rows).map(|row| (row % levels) as u32).collect();
        let index = LevelIndex::from_codes(&original, levels).unwrap();
        for workers in [1, 2, 16] {
            let team = ThreadTeam::new(ThreadConfig::from_requested(workers, false).unwrap());
            for case in 0..10 {
                let mut codes = original.clone();
                let mut index = index.clone();
                let expected = match case {
                    0 => None,
                    1 => {
                        codes[rows - 1] = 17;
                        index.observations[0] = rows;
                        Some((
                            ErrorCode::InvalidData,
                            "borrowed fixed-effect code exceeds its declared level count",
                        ))
                    }
                    2 => {
                        index.offsets[1] = 0;
                        index.observations[rows - 1] = rows;
                        Some((
                            ErrorCode::InvalidArgument,
                            "borrowed fixed-effect membership contains an empty final level",
                        ))
                    }
                    3 => {
                        index.observations[0] = rows;
                        index.offsets[levels] = index.offsets[levels - 1];
                        Some((
                            ErrorCode::InvalidArgument,
                            "borrowed fixed-effect membership does not match its codes",
                        ))
                    }
                    4 => {
                        index.offsets[0] = 1;
                        Some((
                            ErrorCode::InvalidArgument,
                            "borrowed fixed-effect membership does not cover its observation axis",
                        ))
                    }
                    5 => {
                        index.observations[rows - 1] = rows;
                        Some((
                            ErrorCode::InvalidArgument,
                            "borrowed fixed-effect membership does not match its codes",
                        ))
                    }
                    6 | 7 => {
                        index.observations[0] = rows;
                        index.offsets[levels] = if case == 6 {
                            index.offsets[levels - 1] - 1
                        } else {
                            rows + 1
                        };
                        Some((
                            ErrorCode::InvalidArgument,
                            "borrowed fixed-effect membership does not match its codes",
                        ))
                    }
                    _ => {
                        if case == 8 {
                            index.offsets[0] = index.offsets[1] + 1;
                        } else {
                            index.offsets[1] = rows + 1;
                        }
                        index.observations[rows - 1] = rows;
                        Some((
                            ErrorCode::Internal,
                            "fixed-effect membership offsets are inconsistent",
                        ))
                    }
                };
                for _ in 0..2 {
                    let actual = team
                        .install(|| {
                            ValidatedBorrowedInterceptSource::new(&codes, levels, &index)
                                .map(|_| ())
                        })
                        .unwrap();
                    match expected {
                        None => assert_eq!(actual, Ok(())),
                        Some((code, message)) => {
                            assert_eq!(actual.unwrap_err(), FeregError::new(code, message))
                        }
                    }
                }
            }
            for offsets in [vec![], vec![0]] {
                let empty = LevelIndex {
                    offsets,
                    observations: vec![],
                };
                assert!(team
                    .install(|| ValidatedBorrowedInterceptSource::new(&[], 0, &empty).map(|_| ()))
                    .unwrap()
                    .is_ok());
            }
        }
    }
}
