"""Measure two-step graph locality on captured training inputs.

The pair-collision statistic estimates concentration of the right-side Schur
edges induced by sampled left groups. It is descriptive, not a solver rule.
Component sizes keep disconnected dense blocks distinct from a connected graph
with similarly local neighborhoods. Sampling, unequal degrees and weights can
affect the estimate; retain each sampling seed rather than choosing a result.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

import numpy as np


def inspect(path, sample_groups, sample_seed):
    metadata = json.loads(path.read_text())
    # The new routing holdouts stay blind until a rule has been frozen.
    assert metadata['seed'] not in (202609211, 202609212)
    n = metadata['n']
    data = np.memmap(path.with_suffix('.bin'), dtype='<f8', mode='r',
                     shape=(len(metadata['columns']), n))
    column = lambda name: data[metadata['columns'].index(name)]
    used = ['g1', 'g2', 'component'] + (['w'] if metadata['weight_design'] != 'none' else [])
    for name in used:
        assert hashlib.sha256(column(name).tobytes()).hexdigest() == metadata['column_sha256'][name]
    left, right = column('g1'), column('g2')
    weights = column('w') if metadata['weight_design'] != 'none' else np.ones(n)
    assert np.isfinite(weights).all() and np.all(weights > 0)
    order = np.argsort(left, kind='stable')
    boundaries = np.r_[0, 1 + np.flatnonzero(np.diff(left[order])), n]
    levels = len(boundaries) - 1
    selected = np.random.default_rng(sample_seed).choice(
        levels, size=min(sample_groups, levels), replace=False)
    pairs = Counter()
    weighted_pairs = Counter()
    degrees = []
    skipped = 0
    for level in selected:
        rows = order[boundaries[level]:boundaries[level + 1]]
        codes, inverse = np.unique(right[rows], return_inverse=True)
        degrees.append(len(codes))
        # Bound the diagnostic's quadratic expansion. These exclusions remain
        # visible and must not be treated as evidence of favorable mixing.
        if len(codes) > 64:
            skipped += 1
            continue
        sums = np.bincount(inverse, weights=weights[rows])
        total = sums.sum()
        for i, a in enumerate(codes):
            for j in range(i + 1, len(codes)):
                key = (int(a), int(codes[j]))
                pairs[key] += 1
                weighted_pairs[key] += float((sums[i] / total) * (sums[j] / total))
    occurrences = sum(pairs.values())
    collisions = sum(count * (count - 1) // 2 for count in pairs.values())
    component = column('component')
    codes, first, inverse = np.unique(right, return_index=True, return_inverse=True)
    assert np.array_equal(component, component[first][inverse])
    component_sizes = Counter(int(value) for value in component[first])
    possible_pairs = sum(size * (size - 1) // 2 for size in component_sizes.values())
    effective_pairs = occurrences * (occurrences - 1) / (2 * collisions) if collisions else None
    mass = sum(weighted_pairs.values())
    effective_weighted_pairs = mass * mass / sum(value * value for value in weighted_pairs.values()) if mass else None
    return dict(
        input=path.name, input_sha256=metadata['sha256'],
        membership_sha256={name: metadata['column_sha256'][name] for name in ['g1', 'g2', 'component']},
        weight_sha256=metadata['column_sha256'].get('w'),
        design=metadata['design'], n=n, data_seed=metadata['seed'],
        weight_design=metadata['weight_design'], shuffled=metadata['shuffled'],
        sample_seed=sample_seed, sampled_groups=len(selected), total_groups=levels,
        right_levels=len(codes), components=len(component_sizes),
        component_right_sizes=sorted(Counter(component_sizes.values()).items()),
        sampled_distinct_degree_quantiles=np.quantile(degrees, [0, .5, .9, 1]).tolist(),
        skipped_degree_above_64=skipped, pair_occurrences=occurrences,
        distinct_pairs=len(pairs), collision_pairs=collisions,
        effective_pairs_from_collisions=effective_pairs,
        possible_within_component_pairs=possible_pairs,
        effective_pair_fraction=effective_pairs / possible_pairs if effective_pairs is not None and possible_pairs else None,
        weighted_effective_observed_pairs=effective_weighted_pairs,
        weighted_statistic_scope='Observed-sample concentration; not extrapolated to the full graph.')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('output', type=Path)
    p.add_argument('inputs', type=Path, nargs='+')
    p.add_argument('--sample-groups', type=int, default=512)
    p.add_argument('--sample-seeds', type=int, nargs='+', default=[1729, 65537, 99991])
    a = p.parse_args()
    assert a.sample_groups > 0 and not a.output.exists()
    results = [inspect(path, a.sample_groups, seed) for path in a.inputs for seed in a.sample_seeds]
    a.output.write_text(json.dumps(dict(
        schema='two-fe-training-locality-diagnostic-v1',
        source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        acceptance='No routing rule selected or accepted.', results=results), indent=2, allow_nan=False) + '\n')
    print('GRAPH_LOCALITY_DIAGNOSTIC', len(results))
