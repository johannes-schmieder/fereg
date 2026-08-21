use crate::error::{ErrorCode, Result, StFixestError};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingletonResult {
    pub active: Vec<bool>,
    pub dropped_observations: Vec<usize>,
    pub cascade_events: usize,
}

/// Queue-based fixed-point singleton pruning across categorical effects.
pub fn prune_singletons(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    initially_active: &[bool],
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
    if keep_singletons || codes_by_effect.is_empty() {
        return Ok(SingletonResult {
            active: initially_active.to_vec(),
            dropped_observations: Vec::new(),
            cascade_events: 0,
        });
    }

    let mut active = initially_active.to_vec();
    let mut counts: Vec<Vec<usize>> = level_counts
        .iter()
        .map(|levels| vec![0_usize; *levels])
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
                counts[effect][level] += 1;
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
        for (other_effect, codes) in codes_by_effect.iter().enumerate() {
            let other_level = codes[observation] as usize;
            let previous = counts[other_effect][other_level];
            if previous > 0 {
                counts[other_effect][other_level] = previous - 1;
                if previous == 2 {
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
}
