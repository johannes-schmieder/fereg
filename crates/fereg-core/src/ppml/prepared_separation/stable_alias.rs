//! Exact additive identities over two categorical intercept partitions.
use super::*;

pub(super) fn exact_two_intercepts(
    generation: &PreparedPpmlModelGeneration,
    response: &[f64],
) -> Result<bool> {
    let categorical = generation.prepared().as_ref();
    let mut effects = categorical
        .effect_specs()
        .iter()
        .enumerate()
        .filter_map(|(effect, spec)| spec.intercept.then_some(effect));
    let (Some(left), Some(right)) = (effects.next(), effects.next()) else {
        return Ok(false);
    };
    let topology = categorical.fe_topology();
    let views = fallible_collect(
        topology
            .compacted_sources()
            .iter()
            .map(|source| source.view()),
        topology.compacted_sources().len(),
        "unable to reserve exact alias categorical views",
    )?;
    let sources = topology.registry().sources();
    let left = &sources[topology.effect_to_topology()[left]];
    let right = &sources[topology.effect_to_topology()[right]];
    exact_pair(
        response,
        [left.codes(&views)?, right.codes(&views)?],
        [left.level_index(), right.level_index()],
    )
}

fn exact_pair(response: &[f64], codes: [&[u32]; 2], members: [&LevelIndex; 2]) -> Result<bool> {
    let split = members[0].n_levels();
    let vertices = split
        .checked_add(members[1].n_levels())
        .ok_or_else(|| allocation("exact alias graph dimensions overflow"))?;
    if codes.iter().any(|source| source.len() != response.len())
        || members
            .iter()
            .any(|source| source.n_levels() > response.len())
        || response
            .iter()
            .any(|v| !v.is_finite() || v.fract() != 0. || v.abs() > (1_u64 << 53) as f64)
    {
        return Ok(false);
    }
    // At most 2N vertices: i128 potentials plus usize queue use <=6N
    // double-equivalents on supported <=64-bit targets. The projection has
    // returned and released its separately admitted seven-N linear bank;
    // these arrays reuse that envelope, not the eight-N detector bank.
    let mut potentials = fallible_filled(
        vertices,
        i128::MIN,
        "unable to reserve exact alias potentials",
    )?;
    let mut queue = fallible_filled(vertices, 0_usize, "unable to reserve exact alias queue")?;
    let allocated = potentials.capacity() as u128 * std::mem::size_of::<i128>() as u128
        + queue.capacity() as u128 * std::mem::size_of::<usize>() as u128;
    if allocated > response.len() as u128 * 7 * 8 {
        return Err(allocation(
            "exact alias graph exceeds released linear workspace",
        ));
    }
    for seed in 0..vertices {
        crate::interrupt::checkpoint()?;
        if potentials[seed] != i128::MIN {
            continue;
        }
        potentials[seed] = 0;
        queue[0] = seed;
        let (mut head, mut tail) = (0, 1);
        while head < tail {
            let vertex = queue[head];
            head += 1;
            let (side, level) = if vertex < split {
                (0, vertex)
            } else {
                (1, vertex - split)
            };
            for (position, &row) in members[side].checked_members(level)?.iter().enumerate() {
                crate::interrupt::row_checkpoint(position)?;
                let other = codes[1 - side][row] as usize + if side == 0 { split } else { 0 };
                let Some(value) = (response[row] as i128).checked_sub(potentials[vertex]) else {
                    return Ok(false);
                };
                if value == i128::MIN {
                    return Ok(false);
                }
                if potentials[other] == i128::MIN {
                    potentials[other] = value;
                    queue[tail] = other;
                    tail += 1;
                } else if potentials[other] != value {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_alias_graph_checks_disconnected_edges_and_wide_integer_potentials() {
        let limit = (1_u64 << 53) as f64;
        let left = [0, 0, 1, 1, 1, 1, 2, 2];
        let right = [0, 0, 0, 0, 1, 1, 2, 2];
        let members = [
            LevelIndex::from_codes(&left, 3).unwrap(),
            LevelIndex::from_codes(&right, 3).unwrap(),
        ];
        let values = [limit, limit, -limit, -limit, limit, limit, -7., -7.];
        // A connected path needs potentials -2*limit and 3*limit; retaining
        // them in checked i128 avoids an artificial binary64 identity bound.
        assert!(exact_pair(&values, [&left, &right], [&members[0], &members[1]]).unwrap());
        let mut inconsistent = values;
        inconsistent[3] += 1.;
        assert!(!exact_pair(&inconsistent, [&left, &right], [&members[0], &members[1]]).unwrap());
    }
}
