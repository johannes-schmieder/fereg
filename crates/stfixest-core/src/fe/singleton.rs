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
    if keep_singletons || codes_by_effect.is_empty() {
        return Ok(SingletonResult {
            active: initially_active.to_vec(),
            dropped_observations: Vec::new(),
            cascade_events: 0,
        });
    }

    let mut active = initially_active.to_vec();
    let mut counts: Vec<Vec<u64>> = level_counts
        .iter()
        .map(|levels| vec![0_u64; *levels])
        .collect();
    let mut members: Vec<Vec<Vec<usize>>> = level_counts
        .iter()
        .map(|levels| vec![Vec::new(); *levels])
        .collect();
    for (effect, codes) in codes_by_effect.iter().enumerate() {
        for (observation, code) in codes.iter().enumerate() {
            let level = *code as usize;
            if level >= level_counts[effect] {
                return Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    "singleton code exceeds declared level count",
                ));
            }
            members[effect][level].push(observation);
            if active[observation] {
                let observation_multiplicity = frequency_weights.map_or(1_u64, |weights| {
                    // The domain was validated above, so this conversion is exact.
                    weights[observation] as u64
                });
                counts[effect][level] = counts[effect][level]
                    .checked_add(observation_multiplicity)
                    .ok_or_else(|| {
                        StFixestError::new(
                            ErrorCode::InvalidData,
                            "singleton frequency-weight total overflows u64",
                        )
                    })?;
            }
        }
    }

    let mut queue = VecDeque::new();
    for (effect, effect_counts) in counts.iter().enumerate() {
        for (level, count) in effect_counts.iter().enumerate() {
            if *count == 1 {
                queue.push_back((effect, level));
            }
        }
    }

    let mut dropped_observations = Vec::new();
    let mut cascade_events = 0;
    while let Some((effect, level)) = queue.pop_front() {
        if counts[effect][level] != 1 {
            continue;
        }
        let observation = members[effect][level]
            .iter()
            .copied()
            .find(|candidate| active[*candidate]);
        let Some(observation) = observation else {
            counts[effect][level] = 0;
            continue;
        };
        active[observation] = false;
        dropped_observations.push(observation);
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
                    queue.push_back((other_effect, other_level));
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
