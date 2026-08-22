use crate::error::{ErrorCode, Result, StFixestError};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingletonResult {
    pub active: Vec<bool>,
    pub dropped_observations: Vec<usize>,
    pub cascade_events: usize,
}

/// Observation multiplicity used by the reference singleton definition.
///
/// Analytic and probability weights do not change categorical degree. Stata
/// frequency weights represent replicated observations, so their exact
/// positive-integer values do.
#[derive(Debug, Clone, Copy)]
pub enum SingletonMultiplicity<'a> {
    Unit,
    Frequency(&'a [f64]),
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

struct SystemReservation;

impl SingletonReservation for SystemReservation {
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
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

struct EffectMembers {
    offsets: Vec<usize>,
    observations: Vec<usize>,
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn push_reserved_queue<T>(target: &mut VecDeque<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push_back(value);
    Ok(())
}

/// Queue-based fixed-point singleton pruning across categorical effects.
pub fn prune_singletons(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    initially_active: &[bool],
    keep_singletons: bool,
) -> Result<SingletonResult> {
    prune_singletons_with_multiplicity(
        codes_by_effect,
        level_counts,
        initially_active,
        SingletonMultiplicity::Unit,
        keep_singletons,
    )
}

/// Queue-based fixed-point singleton pruning with optional frequency-weight
/// multiplicities.
pub fn prune_singletons_with_multiplicity(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
) -> Result<SingletonResult> {
    let mut reservation = SystemReservation;
    prune_singletons_with_reservation(
        codes_by_effect,
        level_counts,
        initially_active,
        multiplicity,
        keep_singletons,
        &mut reservation,
    )
}

fn prune_singletons_with_reservation<R: SingletonReservation>(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
    reservation: &mut R,
) -> Result<SingletonResult> {
    if codes_by_effect.len() != level_counts.len() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "singleton effect and level-count tables have different lengths",
        ));
    }
    let nobs = initially_active.len();
    if codes_by_effect.iter().any(|codes| codes.len() != nobs) {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "singleton code columns have inconsistent lengths",
        ));
    }
    let frequency_weights = match multiplicity {
        SingletonMultiplicity::Unit => None,
        SingletonMultiplicity::Frequency(weights) => {
            if weights.len() != nobs {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    "singleton frequency-weight column has inconsistent length",
                ));
            }
            for value in weights {
                if !value.is_finite()
                    || *value <= 0.0
                    || value.fract() != 0.0
                    || *value > f64::from(u32::MAX)
                {
                    return Err(StFixestError::new(
                        ErrorCode::InvalidData,
                        "singleton frequency weights must be positive integers representable as u32",
                    ));
                }
            }
            Some(weights)
        }
    };
    let mut active = Vec::new();
    reservation.reserve_vec(&mut active, nobs, "unable to reserve singleton active mask")?;
    active.extend_from_slice(initially_active);
    if keep_singletons || codes_by_effect.is_empty() {
        return Ok(SingletonResult {
            active,
            dropped_observations: Vec::new(),
            cascade_events: 0,
        });
    }

    let total_levels = level_counts
        .iter()
        .try_fold(0_usize, |total, levels| total.checked_add(*levels))
        .ok_or_else(|| allocation("singleton level count overflows addressable storage"))?;
    let mut counts = Vec::new();
    reservation.reserve_vec(
        &mut counts,
        codes_by_effect.len(),
        "unable to reserve singleton count table",
    )?;
    let mut members = Vec::new();
    reservation.reserve_vec(
        &mut members,
        codes_by_effect.len(),
        "unable to reserve singleton member table",
    )?;

    for (effect, codes) in codes_by_effect.iter().enumerate() {
        let levels = level_counts[effect];
        let offset_count = levels
            .checked_add(1)
            .ok_or_else(|| allocation("singleton member-offset count overflows"))?;
        let mut effect_counts = Vec::new();
        reservation.reserve_vec(
            &mut effect_counts,
            levels,
            "unable to reserve singleton level counts",
        )?;
        effect_counts.resize(levels, 0_u64);
        let mut offsets = Vec::new();
        reservation.reserve_vec(
            &mut offsets,
            offset_count,
            "unable to reserve singleton member offsets",
        )?;
        offsets.resize(offset_count, 0_usize);

        for (observation, code) in codes.iter().enumerate() {
            let level = *code as usize;
            if level >= levels {
                return Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    "singleton code exceeds declared level count",
                ));
            }
            offsets[level + 1] = offsets[level + 1].checked_add(1).ok_or_else(|| {
                StFixestError::new(
                    ErrorCode::InvalidData,
                    "singleton level membership count overflows usize",
                )
            })?;
            if active[observation] {
                let observation_multiplicity = frequency_weights.map_or(1_u64, |weights| {
                    // The domain was validated above, so this conversion is exact.
                    weights[observation] as u64
                });
                effect_counts[level] = effect_counts[level]
                    .checked_add(observation_multiplicity)
                    .ok_or_else(|| {
                    StFixestError::new(
                        ErrorCode::InvalidData,
                        "singleton frequency-weight total overflows u64",
                    )
                })?;
            }
        }

        for level in 0..levels {
            offsets[level + 1] =
                offsets[level]
                    .checked_add(offsets[level + 1])
                    .ok_or_else(|| {
                        StFixestError::new(
                            ErrorCode::InvalidData,
                            "singleton cumulative membership count overflows usize",
                        )
                    })?;
        }
        let mut observations = Vec::new();
        reservation.reserve_vec(
            &mut observations,
            nobs,
            "unable to reserve singleton member observations",
        )?;
        observations.resize(nobs, 0_usize);
        let mut cursors = Vec::new();
        reservation.reserve_vec(
            &mut cursors,
            levels,
            "unable to reserve singleton member cursors",
        )?;
        cursors.extend_from_slice(&offsets[..levels]);
        for (observation, code) in codes.iter().enumerate() {
            let level = *code as usize;
            let destination = cursors[level];
            observations[destination] = observation;
            cursors[level] = destination.checked_add(1).ok_or_else(|| {
                StFixestError::new(
                    ErrorCode::InvalidData,
                    "singleton member cursor overflows usize",
                )
            })?;
        }
        push_reserved(
            &mut counts,
            effect_counts,
            "singleton count table capacity exhausted",
        )?;
        push_reserved(
            &mut members,
            EffectMembers {
                offsets,
                observations,
            },
            "singleton member table capacity exhausted",
        )?;
    }

    let mut queue = VecDeque::new();
    reservation.reserve_queue(
        &mut queue,
        total_levels,
        "unable to reserve singleton work queue",
    )?;
    for (effect, effect_counts) in counts.iter().enumerate() {
        for (level, count) in effect_counts.iter().enumerate() {
            if *count == 1 {
                push_reserved_queue(
                    &mut queue,
                    (effect, level),
                    "singleton work queue capacity exhausted",
                )?;
            }
        }
    }

    let mut dropped_observations = Vec::new();
    reservation.reserve_vec(
        &mut dropped_observations,
        nobs,
        "unable to reserve singleton dropped-observation list",
    )?;
    let mut cascade_events = 0;
    while let Some((effect, level)) = queue.pop_front() {
        if counts[effect][level] != 1 {
            continue;
        }
        let effect_members = &members[effect];
        let observation = effect_members.observations
            [effect_members.offsets[level]..effect_members.offsets[level + 1]]
            .iter()
            .copied()
            .find(|candidate| active[*candidate]);
        let Some(observation) = observation else {
            counts[effect][level] = 0;
            continue;
        };
        active[observation] = false;
        push_reserved(
            &mut dropped_observations,
            observation,
            "singleton dropped-observation capacity exhausted",
        )?;
        cascade_events += 1;
        let observation_multiplicity = frequency_weights.map_or(1_u64, |weights| {
            // A positive-integer level total of one implies this is exactly one.
            weights[observation] as u64
        });
        for (other_effect, codes) in codes_by_effect.iter().enumerate() {
            let other_level = codes[observation] as usize;
            let previous = counts[other_effect][other_level];
            if previous > 0 {
                let updated = previous
                    .checked_sub(observation_multiplicity)
                    .ok_or_else(|| {
                        StFixestError::new(
                            ErrorCode::InvalidData,
                            "singleton multiplicity exceeds its fixed-effect level total",
                        )
                    })?;
                counts[other_effect][other_level] = updated;
                if previous != 1 && updated == 1 {
                    push_reserved_queue(
                        &mut queue,
                        (other_effect, other_level),
                        "singleton work queue capacity exhausted",
                    )?;
                }
            }
        }
    }

    Ok(SingletonResult {
        active,
        dropped_observations,
        cascade_events,
    })
}

fn allocation(message: &'static str) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct InjectedReservation {
        fail_at: Option<usize>,
        calls: usize,
    }

    impl InjectedReservation {
        fn check(&mut self) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                Err(allocation("injected singleton allocation failure"))
            } else {
                Ok(())
            }
        }
    }

    impl SingletonReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.check()?;
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test singleton vector reservation failed"))
        }

        fn reserve_queue<T>(
            &mut self,
            target: &mut VecDeque<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.check()?;
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test singleton queue reservation failed"))
        }
    }

    #[test]
    fn removes_singleton_cascade_to_fixed_point() {
        let first = [0, 0, 1, 2];
        let second = [0, 1, 1, 2];
        let result =
            prune_singletons(&[&first, &second], &[3, 3], &[true; 4], false).expect("prune");
        assert_eq!(result.active, vec![false; 4]);
        assert_eq!(result.dropped_observations.len(), 4);
    }

    #[test]
    fn frequency_weight_multiplicity_changes_singleton_identity() {
        let effect = [0, 1];
        let frequency_weights = [2.0, 1.0];
        let weighted = prune_singletons_with_multiplicity(
            &[&effect],
            &[2],
            &[true; 2],
            SingletonMultiplicity::Frequency(&frequency_weights),
            false,
        )
        .expect("frequency-weight pruning");
        assert_eq!(weighted.active, vec![true, false]);
        assert_eq!(weighted.dropped_observations, vec![1]);

        let unit =
            prune_singletons(&[&effect], &[2], &[true; 2], false).expect("unit-weight pruning");
        assert_eq!(unit.active, vec![false, false]);
    }

    #[test]
    fn keep_singletons_is_an_exact_no_op() {
        let effect = [0, 1];
        let result = prune_singletons_with_multiplicity(
            &[&effect],
            &[2],
            &[true, false],
            SingletonMultiplicity::Frequency(&[2.0, 1.0]),
            true,
        )
        .expect("keep singletons");
        assert_eq!(result.active, vec![true, false]);
        assert!(result.dropped_observations.is_empty());
        assert_eq!(result.cascade_events, 0);
    }

    #[test]
    fn invalid_frequency_multiplicity_fails_closed() {
        let effect = [0];
        for weights in [vec![0.0], vec![1.5], vec![f64::INFINITY]] {
            assert_eq!(
                prune_singletons_with_multiplicity(
                    &[&effect],
                    &[1],
                    &[true],
                    SingletonMultiplicity::Frequency(&weights),
                    false,
                )
                .expect_err("invalid frequency weight")
                .code,
                ErrorCode::InvalidData
            );
        }
    }

    #[test]
    fn every_singleton_workspace_reservation_fails_closed_as_allocation() {
        let first = [0, 0, 1, 2];
        let second = [0, 1, 1, 2];
        let codes: [&[u32]; 2] = [&first, &second];
        let mut successful = InjectedReservation::default();
        prune_singletons_with_reservation(
            &codes,
            &[3, 3],
            &[true; 4],
            SingletonMultiplicity::Unit,
            false,
            &mut successful,
        )
        .expect("record singleton reservations");
        assert!(successful.calls >= 10);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                calls: 0,
            };
            let error = prune_singletons_with_reservation(
                &codes,
                &[3, 3],
                &[true; 4],
                SingletonMultiplicity::Unit,
                false,
                &mut injected,
            )
            .expect_err("injected singleton reservation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }
    }

    #[test]
    fn keepsingletons_active_copy_and_level_capacity_overflow_fail_closed() {
        let effect = [0];
        let mut injected = InjectedReservation {
            fail_at: Some(0),
            calls: 0,
        };
        let error = prune_singletons_with_reservation(
            &[&effect],
            &[1],
            &[true],
            SingletonMultiplicity::Unit,
            true,
            &mut injected,
        )
        .expect_err("keepsingletons active-mask reservation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);

        let error = prune_singletons(&[&[]], &[usize::MAX], &[], false)
            .expect_err("impossible level allocation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn reserved_push_guards_fail_closed_before_growth() {
        let mut vector = Vec::new();
        let error = push_reserved(&mut vector, 1_usize, "vector growth")
            .expect_err("unreserved vector push must fail");
        assert_eq!(error.code, ErrorCode::Allocation);

        let mut queue = VecDeque::new();
        let error = push_reserved_queue(&mut queue, 1_usize, "queue growth")
            .expect_err("unreserved queue push must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    fn naive_fixed_point(
        codes_by_effect: &[&[u32]],
        level_counts: &[usize],
        initially_active: &[bool],
        frequency_weights: Option<&[f64]>,
    ) -> Vec<bool> {
        let mut active = initially_active.to_vec();
        loop {
            let counts = codes_by_effect
                .iter()
                .zip(level_counts)
                .map(|(codes, levels)| {
                    let mut counts = vec![0_u64; *levels];
                    for (row, code) in codes.iter().enumerate() {
                        if active[row] {
                            counts[*code as usize] +=
                                frequency_weights.map_or(1_u64, |weights| weights[row] as u64);
                        }
                    }
                    counts
                })
                .collect::<Vec<_>>();
            let drop = (0..active.len())
                .map(|row| {
                    active[row]
                        && codes_by_effect
                            .iter()
                            .enumerate()
                            .any(|(effect, codes)| counts[effect][codes[row] as usize] == 1)
                })
                .collect::<Vec<_>>();
            if !drop.iter().any(|drop| *drop) {
                return active;
            }
            for (row, drop) in drop.into_iter().enumerate() {
                if drop {
                    active[row] = false;
                }
            }
        }
    }

    #[test]
    fn csr_queue_matches_naive_fixed_point_over_small_two_way_designs() {
        for nobs in 0..=5 {
            let assignments = 1_usize << (2 * nobs);
            for assignment in 0..assignments {
                let first = (0..nobs)
                    .map(|row| ((assignment >> row) & 1) as u32)
                    .collect::<Vec<_>>();
                let second = (0..nobs)
                    .map(|row| ((assignment >> (nobs + row)) & 1) as u32)
                    .collect::<Vec<_>>();
                let initially_active = (0..nobs).map(|row| row % 3 != 1).collect::<Vec<_>>();
                let frequency_weights = (0..nobs)
                    .map(|row| if row % 2 == 0 { 1.0 } else { 2.0 })
                    .collect::<Vec<_>>();
                let codes: [&[u32]; 2] = [&first, &second];

                for multiplicity in [
                    SingletonMultiplicity::Unit,
                    SingletonMultiplicity::Frequency(&frequency_weights),
                ] {
                    let expected_weights = match multiplicity {
                        SingletonMultiplicity::Unit => None,
                        SingletonMultiplicity::Frequency(weights) => Some(weights),
                    };
                    let expected =
                        naive_fixed_point(&codes, &[2, 2], &initially_active, expected_weights);
                    let actual = prune_singletons_with_multiplicity(
                        &codes,
                        &[2, 2],
                        &initially_active,
                        multiplicity,
                        false,
                    )
                    .expect("small fixed point");
                    assert_eq!(
                        actual.active, expected,
                        "nobs={nobs} assignment={assignment}"
                    );
                }
            }
        }
    }
}
