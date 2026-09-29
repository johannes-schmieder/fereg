//! Harvested queue/XOR singleton fixed point; frequency mass is not a row count.
use crate::error::{reserved, ErrorCode, FeregError, Result};
use std::collections::VecDeque;
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SingletonMultiplicity<'a> {
    Unit,
    Frequency(&'a [f64]),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingletonResult {
    pub active: Vec<bool>,
    pub dropped_observations: Vec<usize>,
    pub cascade_events: usize,
}

pub fn prune_singletons(
    codes: &[&[u32]],
    levels: &[usize],
    active: &[bool],
    keep: bool,
) -> Result<SingletonResult> {
    prune_singletons_with_multiplicity(codes, levels, active, SingletonMultiplicity::Unit, keep)
}
pub fn prune_singletons_with_multiplicity(
    codes: &[&[u32]],
    levels: &[usize],
    active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep: bool,
) -> Result<SingletonResult> {
    validate_inputs(codes, levels, active, multiplicity)?;
    let mut retained = reserved(active.len())?;
    retained.extend_from_slice(active);
    if keep || codes.is_empty() || active.is_empty() {
        return Ok(SingletonResult {
            active: retained,
            dropped_observations: Vec::new(),
            cascade_events: 0,
        });
    }
    let mut offsets = reserved(levels.len().checked_add(1).ok_or_else(allocation)?)?;
    offsets.push(0usize);
    for &level in levels {
        offsets.push(
            offsets
                .last()
                .unwrap()
                .checked_add(level)
                .ok_or_else(allocation)?,
        );
    }
    let count = *offsets.last().unwrap();
    let mut counts = reserved(count)?;
    counts.resize(count, 0.0);
    for (row, &keep) in active.iter().enumerate() {
        if !keep {
            continue;
        }
        let weight = multiplicity.contribution(row)?;
        for (effect, column) in codes.iter().enumerate() {
            let cell = offsets[effect] + column[row] as usize;
            counts[cell] += weight;
            if !counts[cell].is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "singleton level count became non-finite",
                ));
            }
        }
    }
    if !counts.iter().copied().any(is_singleton) {
        return Ok(SingletonResult {
            active: retained,
            dropped_observations: Vec::new(),
            cascade_events: 0,
        });
    }
    let mut state = QueueState::new(codes, active, offsets, counts)?;
    let mut dropped = reserved(active.len())?;
    while let Some(index) = state.queue.pop_front() {
        state.queued[index] = false;
        if !is_singleton(state.counts[index]) {
            continue;
        }
        let row = state.rows_xor[index];
        let effect = state.offsets.partition_point(|&offset| offset <= index) - 1;
        let level = index - state.offsets[effect];
        if retained.get(row) != Some(&true)
            || codes[effect][row] as usize != level
            || multiplicity.contribution(row)? != 1.0
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "singleton level count has no matching active observation",
            ));
        }
        retained[row] = false;
        dropped.push(row);
        state.remove(codes, row, multiplicity.contribution(row)?)?;
    }
    let cascade_events = dropped.len();
    Ok(SingletonResult {
        active: retained,
        dropped_observations: dropped,
        cascade_events,
    })
}
struct QueueState {
    offsets: Vec<usize>,
    counts: Vec<f64>,
    rows_xor: Vec<usize>,
    queued: Vec<bool>,
    queue: VecDeque<usize>,
}
impl QueueState {
    fn new(
        codes: &[&[u32]],
        active: &[bool],
        offsets: Vec<usize>,
        counts: Vec<f64>,
    ) -> Result<Self> {
        let cells = counts.len();
        let mut rows_xor = reserved(cells)?;
        rows_xor.resize(cells, 0usize);
        for (effect, column) in codes.iter().enumerate() {
            for (row, &code) in column.iter().enumerate() {
                if active[row] {
                    rows_xor[offsets[effect] + code as usize] ^= row;
                }
            }
        }
        let mut queued = reserved(cells)?;
        queued.resize(cells, false);
        let mut queue = VecDeque::new();
        queue.try_reserve(cells).map_err(|_| allocation())?;
        for (index, &count) in counts.iter().enumerate() {
            if is_singleton(count) {
                queue.push_back(index);
                queued[index] = true;
            }
        }
        Ok(Self {
            offsets,
            counts,
            rows_xor,
            queued,
            queue,
        })
    }
    fn remove(&mut self, codes: &[&[u32]], row: usize, weight: f64) -> Result<()> {
        for (effect, column) in codes.iter().enumerate() {
            let index = self.offsets[effect] + column[row] as usize;
            self.rows_xor[index] ^= row;
            self.counts[index] -= weight;
            if self.counts[index].abs() <= singleton_tolerance(self.counts[index]) {
                self.counts[index] = 0.0;
            }
            if self.counts[index] < 0.0 || !self.counts[index].is_finite() {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "singleton level count became invalid during pruning",
                ));
            }
            if is_singleton(self.counts[index]) && !self.queued[index] {
                // At most one queued entry per level, within the upfront reservation.
                self.queue.push_back(index);
                self.queued[index] = true;
            }
        }
        Ok(())
    }
}

impl SingletonMultiplicity<'_> {
    fn validate(self, nobs: usize) -> Result<()> {
        if let Self::Frequency(values) = self {
            if values.len() != nobs {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "singleton frequency multiplicity length differs from sample",
                ));
            }
            if values
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0 || value.fract() != 0.0)
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "singleton frequency multiplicities must be positive finite integers",
                ));
            }
        }
        Ok(())
    }

    fn contribution(self, observation: usize) -> Result<f64> {
        match self {
            Self::Unit => Ok(1.0),
            Self::Frequency(values) => values.get(observation).copied().ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "singleton frequency multiplicity index is out of range",
                )
            }),
        }
    }
}

fn validate_inputs(
    effect_codes: &[&[u32]],
    effect_levels: &[usize],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
) -> Result<()> {
    if effect_codes.len() != effect_levels.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "singleton effect codes and level counts differ in length",
        ));
    }
    multiplicity.validate(initially_active.len())?;
    for (effect, codes) in effect_codes.iter().enumerate() {
        if codes.len() != initially_active.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "singleton effect length differs from sample",
            ));
        }
        if effect_levels[effect] == 0 && !codes.is_empty() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "nonempty singleton effect declares zero levels",
            ));
        }
        if codes
            .iter()
            .any(|code| *code as usize >= effect_levels[effect])
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "singleton effect code exceeds its declared level count",
            ));
        }
    }
    Ok(())
}

fn is_singleton(count: f64) -> bool {
    (count - 1.0).abs() <= singleton_tolerance(count)
}

fn singleton_tolerance(count: f64) -> f64 {
    64.0 * f64::EPSILON * count.abs().max(1.0)
}

fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "unable to allocate singleton workspace",
    )
}
