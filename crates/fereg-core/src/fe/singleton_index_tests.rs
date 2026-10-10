//! Independent fixed-point and historical queue oracles for indexed pruning.
use super::*;

// Pre-index queue semantics, including the original observation-zero search.
// Kept test-only to compare deletion order as well as the final mask.
fn scanning_reference(
    effects: &[&[u32]],
    levels: &[usize],
    initial: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
) -> SingletonResult {
    let mut active = initial.to_vec();
    let mut counts: Vec<Vec<f64>> = levels.iter().map(|&n| vec![0.0; n]).collect();
    for (row, &keep) in initial.iter().enumerate() {
        if keep {
            for (effect, codes) in effects.iter().enumerate() {
                counts[effect][codes[row] as usize] += multiplicity.contribution(row).unwrap();
            }
        }
    }
    let mut queued: Vec<Vec<bool>> = levels.iter().map(|&n| vec![false; n]).collect();
    let mut queue = VecDeque::new();
    for (effect, effect_counts) in counts.iter().enumerate() {
        for (level, &count) in effect_counts.iter().enumerate() {
            if is_singleton(count) {
                queue.push_back((effect, level));
                queued[effect][level] = true;
            }
        }
    }
    let mut dropped_observations = Vec::new();
    while let Some((effect, level)) = queue.pop_front() {
        queued[effect][level] = false;
        if !is_singleton(counts[effect][level]) {
            continue;
        }
        let row = (0..initial.len())
            .find(|&row| {
                active[row]
                    && effects[effect][row] as usize == level
                    && multiplicity.contribution(row).unwrap() > 0.0
            })
            .unwrap();
        active[row] = false;
        dropped_observations.push(row);
        for (effect, codes) in effects.iter().enumerate() {
            let level = codes[row] as usize;
            counts[effect][level] -= multiplicity.contribution(row).unwrap();
            if counts[effect][level].abs() <= singleton_tolerance(counts[effect][level]) {
                counts[effect][level] = 0.0;
            }
            if is_singleton(counts[effect][level]) && !queued[effect][level] {
                queue.push_back((effect, level));
                queued[effect][level] = true;
            }
        }
    }
    SingletonResult {
        cascade_events: dropped_observations.len(),
        active,
        dropped_observations,
    }
}

// Independent simultaneous-pass oracle: a level is a singleton iff it has one
// physical row and that row's literal frequency is one. No floating counts,
// queue, subtraction, XOR or shared singleton predicate.
fn simultaneous_reference(
    effects: &[&[u32]],
    levels: &[usize],
    initial: &[bool],
    frequency: &[f64],
) -> Vec<bool> {
    let mut active = initial.to_vec();
    loop {
        let mut drop = vec![false; initial.len()];
        for (codes, &nlevels) in effects.iter().zip(levels) {
            let mut sole = vec![None; nlevels];
            let mut many = vec![false; nlevels];
            for (row, &code) in codes.iter().enumerate() {
                if active[row] {
                    let level = code as usize;
                    if sole[level].is_some() {
                        many[level] = true;
                    }
                    sole[level] = Some(row);
                }
            }
            for level in 0..nlevels {
                if !many[level] {
                    if let Some(row) = sole[level] {
                        if frequency[row] == 1.0 {
                            drop[row] = true;
                        }
                    }
                }
            }
        }
        if !drop.iter().any(|&v| v) {
            return active;
        }
        for (keep, drop) in active.iter_mut().zip(drop) {
            *keep &= !drop;
        }
    }
}

#[test]
fn singleton_index_matches_legacy_and_independent_masks() {
    let mut state = 941_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for case in 0..320 {
        let n = 1 + case % 79;
        let dimensions = 1 + case % 5;
        let levels: Vec<usize> = (0..dimensions)
            .map(|_| 1 + next() as usize % (n + 3))
            .collect();
        let effects: Vec<Vec<u32>> = levels
            .iter()
            .map(|&k| (0..n).map(|_| (next() as usize % k) as u32).collect())
            .collect();
        let views: Vec<&[u32]> = effects.iter().map(Vec::as_slice).collect();
        let initial: Vec<bool> = (0..n).map(|_| next() % 7 != 0).collect();
        let frequency: Vec<f64> = (0..n)
            .map(|_| match next() % 11 {
                0 => 2.0,
                1 => 7.0,
                2 => 9007199254740992.0,
                _ => 1.0,
            })
            .collect();
        for multiplicity in [
            SingletonMultiplicity::Unit,
            SingletonMultiplicity::Frequency(&frequency),
        ] {
            let actual =
                prune_singletons_with_multiplicity(&views, &levels, &initial, multiplicity, false)
                    .unwrap();
            assert_eq!(
                actual,
                scanning_reference(&views, &levels, &initial, multiplicity),
                "legacy case {case}"
            );
            let unit = vec![1.0; n];
            let weights = match multiplicity {
                SingletonMultiplicity::Unit => &unit,
                SingletonMultiplicity::Frequency(w) => w,
            };
            assert_eq!(
                actual.active,
                simultaneous_reference(&views, &levels, &initial, weights),
                "independent case {case}"
            );
        }
    }
}

fn chain_and_core(chain: usize) -> (Vec<u32>, Vec<u32>, Vec<bool>) {
    // A stable 2x2 cycle, then a long path whose endpoints trigger a cascade.
    let mut first = vec![0, 0, 1, 1];
    let mut second = vec![0, 1, 0, 1];
    for row in 0..chain {
        first.push((2 + row / 2) as u32);
        second.push((2 + (row + 1) / 2) as u32);
    }
    let mut expected = vec![false; chain + 4];
    expected[..4].fill(true);
    (first, second, expected)
}

#[test]
fn singleton_index_preserves_long_cascade_order_and_inactive_rows() {
    let (first, second, expected) = chain_and_core(1024);
    let effects = [&first[..], &second[..]];
    let levels = [515, 515];
    let mut initial = vec![true; first.len()];
    for row in [9, 511, 900] {
        initial[row] = false;
    }
    let result = prune_singletons_with_multiplicity(
        &effects,
        &levels,
        &initial,
        SingletonMultiplicity::Unit,
        false,
    )
    .unwrap();
    assert_eq!(
        result,
        scanning_reference(&effects, &levels, &initial, SingletonMultiplicity::Unit)
    );
    assert_eq!(result.active, expected);
    assert_eq!(result.dropped_observations.len(), 1021);
}

#[test]
fn singleton_index_scale_doubling_has_substantial_drops() {
    // Timings are diagnostics, not a noisy wall-clock assertion. The release
    // A/B scale experiment also runs the unchanged scanner on identical inputs.
    for n in [8192, 16384, 32768] {
        let (first, second, expected) = chain_and_core(n);
        let initial = vec![true; first.len()];
        let started = std::time::Instant::now();
        let result = prune_singletons_with_multiplicity(
            &[&first, &second],
            &[n / 2 + 3; 2],
            &initial,
            SingletonMultiplicity::Unit,
            false,
        )
        .unwrap();
        let elapsed = started.elapsed();
        assert_eq!(result.active, expected);
        assert_eq!(result.dropped_observations.len(), n);
        eprintln!(
            "SINGLETON_SCALE chain={n} dropped={} seconds={:.9}",
            result.cascade_events,
            elapsed.as_secs_f64()
        );
    }
}

#[test]
fn singleton_index_review_scale_has_half_initial_singletons_and_a_cascade() {
    // Half the rows are isolated singletons, a quarter form a path whose
    // endpoints trigger a cascade, and the remaining quarter form a core.
    // The small instances additionally bind the closed-form retained mask to
    // both the historical search and an independent simultaneous fixed point.
    for n in [256, 512, 65_536, 131_072] {
        let isolated = n / 2;
        let chain = n / 4;
        let core_level = isolated + chain / 2 + 1;
        let mut first = vec![core_level as u32; n];
        let mut second = first.clone();
        for row in 0..isolated {
            first[row] = row as u32;
            second[row] = row as u32;
        }
        for edge in 0..chain {
            first[isolated + edge] = (isolated + edge / 2) as u32;
            second[isolated + edge] = (isolated + (edge + 1) / 2) as u32;
        }
        let effects = [&first[..], &second[..]];
        let levels = [core_level + 1; 2];
        let initial = vec![true; n];
        let expected: Vec<bool> = (0..n).map(|row| row >= isolated + chain).collect();
        if n <= 512 {
            let historical =
                scanning_reference(&effects, &levels, &initial, SingletonMultiplicity::Unit);
            assert_eq!(historical.active, expected);
            assert_eq!(
                simultaneous_reference(&effects, &levels, &initial, &vec![1.0; n]),
                expected
            );
        }
        let started = std::time::Instant::now();
        let result = prune_singletons_with_multiplicity(
            &effects,
            &levels,
            &initial,
            SingletonMultiplicity::Unit,
            false,
        )
        .unwrap();
        let elapsed = started.elapsed();
        assert_eq!(result.active, expected);
        assert_eq!(result.dropped_observations.len(), isolated + chain);
        assert_eq!(result.cascade_events, isolated + chain);
        if n >= 65_536 {
            eprintln!(
                "SINGLETON_REVIEW_SCALE rows={n} initially_singleton_rows={} dropped={} retained={} seconds={:.9}",
                isolated + 2,
                result.dropped_observations.len(),
                n / 4,
                elapsed.as_secs_f64()
            );
        }
    }
}

#[derive(Default)]
struct PayloadReservation {
    calls: usize,
    bytes: usize,
}
impl SingletonReservation for PayloadReservation {
    fn reserve_vec<T>(&mut self, target: &mut Vec<T>, n: usize, _: &'static str) -> Result<()> {
        target.try_reserve_exact(n).unwrap();
        self.calls += 1;
        self.bytes += target.capacity() * std::mem::size_of::<T>();
        Ok(())
    }
    fn reserve_queue<T>(
        &mut self,
        target: &mut VecDeque<T>,
        n: usize,
        _: &'static str,
    ) -> Result<()> {
        target.try_reserve(n).unwrap();
        self.calls += 1;
        self.bytes += target.capacity() * std::mem::size_of::<T>();
        Ok(())
    }
}

#[test]
fn singleton_index_preserves_no_drop_fast_path_and_allocation_envelope() {
    for n in [4, 16, 257, 8192] {
        let codes = vec![0; n];
        let initial = vec![true; n];
        let mut reservation = PayloadReservation::default();
        let result = prune_singletons_with_reservation(
            &[&codes],
            &[1],
            &initial,
            SingletonMultiplicity::Unit,
            false,
            &mut reservation,
        )
        .unwrap();
        assert!(result.dropped_observations.is_empty());
        assert_eq!(
            reservation.calls, 3,
            "no index, markers, queue or drops allocation"
        );

        let codes: Vec<u32> = (0..n as u32).collect();
        let mut reservation = PayloadReservation::default();
        let result = prune_singletons_with_reservation(
            &[&codes],
            &[n],
            &initial,
            SingletonMultiplicity::Unit,
            false,
            &mut reservation,
        )
        .unwrap();
        assert_eq!(result.dropped_observations, (0..n).collect::<Vec<_>>());
        let mut old_queue = VecDeque::<(usize, usize)>::new();
        old_queue.try_reserve(n).unwrap();
        let old_payload = n * std::mem::size_of::<bool>()
            + 2 * std::mem::size_of::<usize>()
            + n * std::mem::size_of::<f64>()
            + n * std::mem::size_of::<bool>()
            + old_queue.capacity() * std::mem::size_of::<(usize, usize)>()
            + n * std::mem::size_of::<usize>();
        assert!(
            reservation.bytes <= old_payload,
            "{n}: {} > {old_payload}",
            reservation.bytes
        );
    }
}
