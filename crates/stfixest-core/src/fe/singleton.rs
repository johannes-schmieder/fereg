//! Iterative singleton pruning using level counts and a deletion queue.

use std::collections::VecDeque;

use crate::error::{ErrorCode, Result, StFixestError};

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
    effect_codes: &[&[u32]],
    effect_levels: &[usize],
    initially_active: &[bool],
    keep_singletons: bool,
) -> Result<SingletonResult> {
    prune_singletons_with_multiplicity(
        effect_codes,
        effect_levels,
        initially_active,
        SingletonMultiplicity::Unit,
        keep_singletons,
    )
}

pub fn prune_singletons_with_multiplicity(
    effect_codes: &[&[u32]],
    effect_levels: &[usize],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
) -> Result<SingletonResult> {
    let mut reservation = SystemSingletonReservation;
    prune_singletons_with_reservation(
        effect_codes,
        effect_levels,
        initially_active,
        multiplicity,
        keep_singletons,
        &mut reservation,
    )
}

trait SingletonReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_queue<T>(
        &mut self,
        target: &mut VecDeque<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemSingletonReservation;

impl SingletonReservation for SystemSingletonReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }

    fn reserve_queue<T>(
        &mut self,
        target: &mut VecDeque<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve(additional)
            .map_err(|_| allocation(label))
    }
}

fn prune_singletons_with_reservation<R: SingletonReservation>(
    effect_codes: &[&[u32]],
    effect_levels: &[usize],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
    reservation: &mut R,
) -> Result<SingletonResult> {
    validate_inputs(effect_codes, effect_levels, initially_active, multiplicity)?;
    let mut active = reserved_copy(
        reservation,
        initially_active,
        "unable to reserve singleton active mask",
    )?;
    if keep_singletons || effect_codes.is_empty() || initially_active.is_empty() {
        return Ok(SingletonResult {
            active,
            dropped_observations: Vec::new(),
            cascade_events: 0,
        });
    }

    let counts_len = effect_levels.iter().try_fold(0_usize, |total, levels| {
        total
            .checked_add(*levels)
            .ok_or_else(|| allocation("singleton count table length overflows"))
    })?;
    let offsets_len = effect_levels
        .len()
        .checked_add(1)
        .ok_or_else(|| allocation("singleton effect-offset length overflows"))?;
    let mut effect_offsets = reserved_filled(
        reservation,
        offsets_len,
        0_usize,
        "unable to reserve singleton effect offsets",
    )?;
    for effect in 0..effect_levels.len() {
        effect_offsets[effect + 1] = effect_offsets[effect]
            .checked_add(effect_levels[effect])
            .ok_or_else(|| allocation("singleton effect offsets overflow"))?;
    }
    if effect_offsets[effect_levels.len()] != counts_len {
        return Err(StFixestError::new(
            ErrorCode::Internal,
            "singleton effect offsets are inconsistent with level totals",
        ));
    }
    let mut counts = reserved_filled(
        reservation,
        counts_len,
        0.0_f64,
        "unable to reserve singleton level counts",
    )?;
    for (observation, is_active) in initially_active.iter().copied().enumerate() {
        if !is_active {
            continue;
        }
        let contribution = multiplicity.contribution(observation)?;
        for effect in 0..effect_codes.len() {
            let level = effect_codes[effect][observation] as usize;
            let index = effect_offsets[effect]
                .checked_add(level)
                .ok_or_else(|| allocation("singleton count index overflows"))?;
            counts[index] += contribution;
            if !counts[index].is_finite() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    "singleton level count became non-finite",
                ));
            }
        }
    }

    let queued_len = counts_len;
    let mut queued = reserved_filled(
        reservation,
        queued_len,
        false,
        "unable to reserve singleton queue markers",
    )?;
    let mut queue = VecDeque::new();
    reservation.reserve_queue(
        &mut queue,
        queued_len,
        "unable to reserve singleton level queue",
    )?;
    for effect in 0..effect_levels.len() {
        for level in 0..effect_levels[effect] {
            let index = effect_offsets[effect] + level;
            if is_singleton(counts[index]) {
                queue_push_reserved(
                    &mut queue,
                    (effect, level),
                    "singleton level queue capacity exhausted",
                )?;
                queued[index] = true;
            }
        }
    }

    let mut dropped_observations = Vec::new();
    reservation.reserve_vec(
        &mut dropped_observations,
        initially_active.len(),
        "unable to reserve singleton dropped-observation list",
    )?;
    let mut cascade_events = 0_usize;
    while let Some((effect, level)) = queue.pop_front() {
        let index = effect_offsets[effect]
            .checked_add(level)
            .ok_or_else(|| allocation("singleton queue count index overflows"))?;
        queued[index] = false;
        if !is_singleton(counts[index]) {
            continue;
        }
        let observation =
            find_active_observation(effect_codes[effect], level, &active, multiplicity)?
                .ok_or_else(|| {
                    StFixestError::new(
                        ErrorCode::Internal,
                        "singleton level count has no matching active observation",
                    )
                })?;
        active[observation] = false;
        push_reserved(
            &mut dropped_observations,
            observation,
            "singleton dropped-observation capacity exhausted",
        )?;
        cascade_events = cascade_events
            .checked_add(1)
            .ok_or_else(|| allocation("singleton cascade count overflows"))?;
        let contribution = multiplicity.contribution(observation)?;
        for affected_effect in 0..effect_codes.len() {
            let affected_level = effect_codes[affected_effect][observation] as usize;
            let affected_index = effect_offsets[affected_effect]
                .checked_add(affected_level)
                .ok_or_else(|| allocation("singleton affected count index overflows"))?;
            counts[affected_index] -= contribution;
            if counts[affected_index].abs() <= singleton_tolerance(counts[affected_index]) {
                counts[affected_index] = 0.0;
            }
            if counts[affected_index] < 0.0 || !counts[affected_index].is_finite() {
                return Err(StFixestError::new(
                    ErrorCode::Internal,
                    "singleton level count became invalid during pruning",
                ));
            }
            if is_singleton(counts[affected_index]) && !queued[affected_index] {
                queue_push_reserved(
                    &mut queue,
                    (affected_effect, affected_level),
                    "singleton level queue capacity exhausted",
                )?;
                queued[affected_index] = true;
            }
        }
    }

    Ok(SingletonResult {
        active,
        dropped_observations,
        cascade_events,
    })
}

impl SingletonMultiplicity<'_> {
    fn validate(self, nobs: usize) -> Result<()> {
        if let Self::Frequency(values) = self {
            if values.len() != nobs {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    "singleton frequency multiplicity length differs from sample",
                ));
            }
            if values
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0 || value.fract() != 0.0)
            {
                return Err(StFixestError::new(
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
                StFixestError::new(
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
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "singleton effect codes and level counts differ in length",
        ));
    }
    multiplicity.validate(initially_active.len())?;
    for (effect, codes) in effect_codes.iter().enumerate() {
        if codes.len() != initially_active.len() {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "singleton effect length differs from sample",
            ));
        }
        if effect_levels[effect] == 0 && !codes.is_empty() {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "nonempty singleton effect declares zero levels",
            ));
        }
        if codes
            .iter()
            .any(|code| *code as usize >= effect_levels[effect])
        {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "singleton effect code exceeds its declared level count",
            ));
        }
    }
    Ok(())
}

fn find_active_observation(
    codes: &[u32],
    level: usize,
    active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
) -> Result<Option<usize>> {
    for observation in 0..codes.len() {
        if active[observation]
            && codes[observation] as usize == level
            && multiplicity.contribution(observation)? > 0.0
        {
            return Ok(Some(observation));
        }
    }
    Ok(None)
}

fn is_singleton(count: f64) -> bool {
    (count - 1.0).abs() <= singleton_tolerance(count)
}

fn singleton_tolerance(count: f64) -> f64 {
    64.0 * f64::EPSILON * count.abs().max(1.0)
}

fn reserved_copy<T: Clone, R: SingletonReservation>(
    reservation: &mut R,
    input: &[T],
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, input.len(), label)?;
    if output.capacity() < input.len() {
        return Err(allocation(
            "singleton reservation returned insufficient capacity",
        ));
    }
    output.extend_from_slice(input);
    Ok(output)
}

fn reserved_filled<T: Clone, R: SingletonReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "singleton reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, value);
    Ok(output)
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn queue_push_reserved<T>(target: &mut VecDeque<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push_back(value);
    Ok(())
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        skip_reserve_at: Option<usize>,
    }

    impl SingletonReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation("injected singleton allocation failure"));
            }
            if self.skip_reserve_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("singleton test allocation failure"))
        }

        fn reserve_queue<T>(
            &mut self,
            target: &mut VecDeque<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation("injected singleton allocation failure"));
            }
            if self.skip_reserve_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve(additional)
                .map_err(|_| allocation("singleton test queue allocation failure"))
        }
    }

    #[test]
    fn drops_singletons_to_fixed_point() {
        let first = vec![0, 0, 1, 2];
        let second = vec![0, 1, 1, 2];
        let result = prune_singletons(
            &[&first, &second],
            &[3, 3],
            &[true, true, true, true],
            false,
        )
        .expect("singleton pruning");
        assert_eq!(result.active, vec![false, false, false, false]);
        assert_eq!(result.dropped_observations.len(), 4);
        assert_eq!(result.cascade_events, 4);
    }

    #[test]
    fn respects_keep_singletons() {
        let codes = vec![0, 1];
        let result = prune_singletons(&[&codes], &[2], &[true, true], true).expect("kept");
        assert_eq!(result.active, vec![true, true]);
        assert!(result.dropped_observations.is_empty());
    }

    #[test]
    fn rejects_invalid_codes() {
        let codes = vec![0, 2];
        let error =
            prune_singletons(&[&codes], &[2], &[true, true], false).expect_err("invalid code");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn frequency_multiplicity_changes_singleton_status() {
        let codes = vec![0, 1];
        let frequency = vec![2.0, 1.0];
        let result = prune_singletons_with_multiplicity(
            &[&codes],
            &[2],
            &[true, true],
            SingletonMultiplicity::Frequency(&frequency),
            false,
        )
        .expect("frequency singleton pruning");
        assert_eq!(result.active, vec![true, false]);
        assert_eq!(result.dropped_observations, vec![1]);
    }

    #[test]
    fn inactive_frequency_rows_do_not_contribute() {
        let codes = vec![0, 0, 1];
        let frequency = vec![1.0, 100.0, 2.0];
        let result = prune_singletons_with_multiplicity(
            &[&codes],
            &[2],
            &[true, false, true],
            SingletonMultiplicity::Frequency(&frequency),
            false,
        )
        .expect("frequency singleton pruning");
        assert_eq!(result.active, vec![false, false, true]);
        assert_eq!(result.dropped_observations, vec![0]);
    }

    #[test]
    fn rejects_noninteger_frequency_multiplicity() {
        let codes = vec![0, 0];
        let frequency = vec![1.5, 1.0];
        let error = prune_singletons_with_multiplicity(
            &[&codes],
            &[1],
            &[true, true],
            SingletonMultiplicity::Frequency(&frequency),
            false,
        )
        .expect_err("invalid frequency multiplicity");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn every_singleton_reservation_failure_is_atomic() {
        let first = vec![0, 0, 1, 2];
        let second = vec![0, 1, 1, 2];
        let effects: [&[u32]; 2] = [&first, &second];
        let levels = [3_usize, 3_usize];
        let active = [true, true, true, true];
        let frequency = [1.0, 1.0, 1.0, 1.0];

        let mut successful = InjectedReservation::default();
        let result = prune_singletons_with_reservation(
            &effects,
            &levels,
            &active,
            SingletonMultiplicity::Frequency(&frequency),
            false,
            &mut successful,
        )
        .expect("successful singleton reservation audit");
        assert_eq!(result.active, vec![false; 4]);
        assert_eq!(successful.calls, 6);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                calls: 0,
                fail_at: Some(fail_at),
                skip_reserve_at: None,
            };
            let error = prune_singletons_with_reservation(
                &effects,
                &levels,
                &active,
                SingletonMultiplicity::Frequency(&frequency),
                false,
                &mut injected,
            )
            .expect_err("injected singleton allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }
    }

    #[test]
    fn every_singleton_capacity_guard_fails_closed() {
        let first = vec![0, 0, 1, 2];
        let second = vec![0, 1, 1, 2];
        let effects: [&[u32]; 2] = [&first, &second];
        let levels = [3_usize, 3_usize];
        let active = [true, true, true, true];

        let mut successful = InjectedReservation::default();
        prune_singletons_with_reservation(
            &effects,
            &levels,
            &active,
            SingletonMultiplicity::Unit,
            false,
            &mut successful,
        )
        .expect("successful singleton capacity audit");
        assert_eq!(successful.calls, 6);

        for skip_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                calls: 0,
                fail_at: None,
                skip_reserve_at: Some(skip_at),
            };
            let error = prune_singletons_with_reservation(
                &effects,
                &levels,
                &active,
                SingletonMultiplicity::Unit,
                false,
                &mut injected,
            )
            .expect_err("singleton capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }

    #[test]
    fn rejects_overflowing_level_totals_before_allocation() {
        let active = [true];
        let codes = [0_u32];
        let effects: [&[u32]; 2] = [&codes, &codes];
        let error = prune_singletons_with_reservation(
            &effects,
            &[usize::MAX, 1],
            &active,
            SingletonMultiplicity::Unit,
            false,
            &mut InjectedReservation::default(),
        )
        .expect_err("overflowing singleton level totals");
        assert_eq!(error.code, ErrorCode::Allocation);
    }
}
