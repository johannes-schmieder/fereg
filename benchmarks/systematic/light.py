"""The explicitly bounded successor campaign; legacy manifests stay unchanged."""
from collections import defaultdict
import csv
import math
from pathlib import Path
import statistics


def enabled(manifest):
    return manifest.get('schema') == 'fereg-systematic-light-v2'


def cells(manifest):
    result = []
    for model in manifest['arms']:
        for design, spec in manifest['designs'].items():
            settings = {(n, 10, t, 'main') for n in manifest['sizes'] for t in (4, 28)}
            if design in manifest['scaling_designs']:
                settings |= {(manifest['scaling_n'], 10, t, 'main') for t in manifest['threads']}
            if design in manifest['width_designs']:
                settings |= {(manifest['scaling_n'], 50, t, 'width') for t in (4, 28)}
            if model == 'ppml' and design in manifest['zero_designs']:
                settings.add((61440, 10, 4, 'zeros'))
            for n, k, threads, section in sorted(settings):
                result.append(dict(model=model, design=design, suite=spec['suite'],
                                   n=n, k=k, threads=threads, section=section, effects=spec['effects']))
        for design in ('two_fe_dense', 'three_fe_sparse'):
            for memory, k, threads in [('fast', 10, 28), ('low', 6, 4)]:
                spec = manifest['designs'][design]
                slopes = {'g1': ['z1'], 'g2': ['z2']} if len(spec['effects']) == 2 else {
                    'g1': ['z1', 'z2'], 'g2': ['z3', 'z4']}
                result.append(dict(model=model, design=design, suite=spec['suite'], n=61440,
                                   k=k, threads=threads, section='slopes_'+memory,
                                   effects=spec['effects'], slopes=slopes, memory=memory))
    return result


def repetitions(cell, manifest):
    warm = list(range(1, manifest.get('warm_repetitions', 6)+1))
    if not enabled(manifest) or (cell['section'] == 'main' and cell['n'] == 61440
                                and cell['threads'] == 4 and cell['design'] in manifest['scaling_designs']):
        return [0, *warm]
    return warm


def groups(manifest):
    from common import cells as enumerate_cells
    return [dict(cell=c, repetition=r) for c in enumerate_cells(manifest) for r in repetitions(c, manifest)]


def skips(cell, repetition, manifest):
    result = {}
    if cell.get('slopes'):
        for arm in ('areg', 'greg', 'gglm', 'pyfixest'):
            if arm in manifest['arms'][cell['model']]:
                result[arm] = dict(status='unsupported_surface', reason='Pinned worker has no validated absorbed-slope API; no expanded-dummy substitute.')
    for rule in manifest.get('skip_rules', []):
        if all(cell.get(k) == v for k, v in rule['match'].items()) and repetition in rule['repetitions']:
            result[rule['arm']] = dict(status='budget_exclusion', reason=rule['reason'], evidence=rule['evidence'])
    return result


def active_arms(group, manifest):
    omitted = skips(group['cell'], group['repetition'], manifest)
    return [a for a in manifest['arms'][group['cell']['model']] if a not in omitted]


def historical_costs(path):
    records = defaultdict(list)
    with Path(path).open() as f:
        for row in csv.DictReader(f):
            seconds = float(row.get('seconds') or row.get('timeout_bound') or 0)
            if math.isfinite(seconds) and seconds > 0:
                arm = row['arm']
                key = (row['model'], row['design'], int(row['n']), int(row['k']), int(row['threads']), arm)
                records[key].append(seconds)
    return records


def estimate_arm(cell, arm, records):
    exact = (cell['model'], cell['design'], cell['n'], cell['k'], cell['threads'], arm)
    if exact in records:
        return statistics.median(records[exact])
    source_arm = 'fixest' if arm == 'fixest_strict' else arm
    key = (cell['model'], cell['design'], cell['n'], cell['k'], cell['threads'], source_arm)
    if key in records:
        value = statistics.median(records[key])
    else:
        near = [(k, v) for k, v in records.items() if k[0] == cell['model'] and k[1] == cell['design'] and k[5] == source_arm]
        if not near:
            near = [(k, v) for k, v in records.items() if k[0] == cell['model'] and k[5] == source_arm]
        if not near:
            raise ValueError(f'No cost evidence for {cell["model"]}/{arm}')
        k, values = min(near, key=lambda kv: abs(math.log(cell['n']/kv[0][2])) + abs(math.log(cell['k']/kv[0][3])))
        value = statistics.median(values) * (cell['n']/k[2])**1.3 * max(1, cell['k']/k[3])
    return min(2400., max(.1, value * (2 if cell.get('slopes') else 1) * (1.5 if arm == 'fixest_strict' else 1)))


def configure_group(group, manifest, records):
    cell = group['cell']
    predicted, limits = {}, {}
    for arm in active_arms(group, manifest):
        estimate = estimate_arm(cell, arm, records)
        # Generous fixed deadlines are frozen before the production attempt.
        limits[arm] = dict(startup=600, command=min(2400, max(120, math.ceil(estimate*6))), extraction=600)
        predicted[arm] = estimate + 45 + cell['n']*cell['k']*8/100_000_000
    return dict(group, limits=limits, predicted_seconds=sum(predicted.values())+15,
                arm_predictions=predicted, skips=skips(cell, group['repetition'], manifest))


def worst_seconds(group):
    return 300 + sum(sum(limits.values())+5 for limits in group['limits'].values())


def pack(groups_to_pack, target_seconds, wall_seconds):
    """Mix models and sizes, then fill long bundles without splitting a pair."""
    if not 0 < target_seconds < wall_seconds:
        raise ValueError('Packing target must be below wall time')
    queues = defaultdict(list)
    for group in groups_to_pack:
        if worst_seconds(group) > wall_seconds-600:
            raise ValueError('One complete comparison exceeds the task deadline')
        kind = 'slopes' if group['cell'].get('slopes') else group['cell']['model']
        queues[kind].append(group)
    for queue in queues.values():
        queue.sort(key=lambda g: (-g['predicted_seconds'], g['repetition']))
    ordered = []
    while any(queues.values()):
        for kind in ('ols', 'ppml', 'slopes'):
            if queues[kind]:ordered.append(queues[kind].pop(0))
    total = sum(g['predicted_seconds'] for g in ordered)
    count = max(1, math.ceil(total/target_seconds))
    bundles = [dict(groups=[], predicted_seconds=0.) for _ in range(count)]
    for group in ordered:
        target = min(bundles, key=lambda b: b['predicted_seconds'])
        target['groups'].append(group)
        target['predicted_seconds'] += group['predicted_seconds']
    return bundles


def choose_fitting(pending, remaining_seconds):
    # Near the deadline, a smaller complete group can use an otherwise idle tail.
    return next((i for i, g in enumerate(pending) if worst_seconds(g)+600 <= remaining_seconds), None)


def smoke_groups(manifest):
    selected = []
    for model in manifest['arms']:
        for design in manifest['designs']:
            c = next(c for c in cells(manifest) if c['model'] == model and c['design'] == design and c['section'] == 'main')
            selected.append(dict(cell=dict(c, n=1920 if len(c['effects']) == 3 else 7680, threads=4), repetition=1))
        for c in cells(manifest):
            if c['model'] == model and (c.get('slopes') or c['section']=='zeros'):
                selected.append(dict(cell=dict(c, n=960, threads=4), repetition=1))
    return selected
