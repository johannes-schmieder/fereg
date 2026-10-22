"""Prospective two-gate offline fitter. Importing/collecting never fits a model.

The explicit fit() entrypoint enumerates exactly one declared class. There is
no CLI, runtime mutation, alternative fit, unpooled selection or resampling.
Only committed, hash-bound calibration evidence is accepted by collect().
"""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path

from two_fe_policy import ACTION, require
from two_fe_policy_labels import action_key, paired_cell, stable_actions

FEATURES = ('n', 'p', 'q', 'd', 'workers', 'r', 'stride', 'bitlen_h', 'bitlen_d',
            'decreasing', 'mixed', 'stagnant', 'weighted', 'scaled', 'packed',
            'outcome', 'final', 'future_calls', 'future_columns', 'future_width',
            'final_pending')
BOOLEANS = frozenset(range(9, 17)) | {20}
PROGRESS = frozenset(range(7, 12))
MAX_PAIRS = 8464
U128 = (1 << 128) - 1
ARMS = ('never', 'force16', 'force32')
SOURCE = 'cdc478dd0cac7a6d4d5c0861fb4e380820987022'
OUTCOME = 'bench/results/two-fe-policy-calibration-outcome-20261022.json'
OUTCOME_SHA = 'fdd330db09e7a6e4eb4b977120abadc0484598c329df3fe625afffa559d73230'
PRESETS = {a: 'routing_two_remaining_' + a for a in ARMS}


def integer(value, name, minimum=0, maximum=U128):
    require(type(value) is int and minimum <= value <= maximum, 'invalid ' + name)
    return value


def features(point):
    """The frozen 21 pre-intervention fields, never future realized outcomes."""
    for name in ('n', 'p', 'q', 'd', 'workers', 'r', 'stride'):
        integer(point[name], name, 1)
    require(point['stride'] >= point['r'], 'RHS width exceeds stride')
    require(point['role'] in ('packed', 'outcome', 'final'), 'unobserved episode role')
    require(point['epoch'] == 0, 'unobserved correction episode')
    h = integer(point['forecast_h'], 'progress H', 1)
    d = integer(point['forecast_d'], 'progress D', h)
    mask = integer(point['classes'], 'progress classes', 1, 7)
    total = integer(point['total'], 'regressor count')
    role = point['role']
    if role == 'outcome':
        block = integer(point['block'], 'regressor block', 1)
        require(point['r'] == 1, 'outcome width')
        calls, columns, width = (total + block - 1) // block, total, min(total, block)
    else:
        require(point['r'] == (total + 1 if role == 'packed' else 1), 'role width')
        calls = columns = width = 0
    values = tuple(point[k] for k in ('n', 'p', 'q', 'd', 'workers', 'r', 'stride'))
    values += (h.bit_length(), d.bit_length(), *(int(bool(mask & b)) for b in (1, 2, 4)),
               point['weighted'], point['scaled'], int(role == 'packed'),
               int(role == 'outcome'), int(role == 'final'), calls, columns, width,
               int(role != 'final'))
    validate_features(values)
    return values


def validate_features(values):
    require(len(values) == len(FEATURES), 'feature schema length')
    for i, value in enumerate(values):
        integer(value, FEATURES[i], maximum=1 if i in BOOLEANS else U128)


@dataclass(frozen=True, order=True)
class Gate:
    kind: int = 0
    feature: int = 0
    cut: int = 0
    build_if_le: int = 0

    def __post_init__(self):
        integer(self.kind, 'gate kind', maximum=2)
        integer(self.feature, 'feature ID', maximum=20)
        integer(self.cut, 'cut')
        integer(self.build_if_le, 'orientation', maximum=1)
        require(self.kind == 2 or (self.feature, self.cut, self.build_if_le) == (0, 0, 0),
                'noncanonical constant gate')
        require(self.kind != 2 or self.feature not in BOOLEANS or self.cut == 0,
                'noncanonical boolean cut')

    @property
    def progress(self):
        return self.kind == 2 and self.feature in PROGRESS

    def build(self, values):
        validate_features(values)
        return self.kind == 1 or (self.kind == 2 and
                (values[self.feature] <= self.cut) == bool(self.build_if_le))


DEFER, BUILD = Gate(), Gate(1)


def encode(gates):
    require(len(gates) == 2, 'gate count')
    payload = b''.join(bytes((g.kind, g.feature, g.build_if_le)) + bytes(13)
                       + g.cut.to_bytes(16, 'little') for g in gates)
    return (b'2FEGATE2' + (1).to_bytes(2, 'little') * 2 + (128).to_bytes(4, 'little')
            + bytes((2,)) + bytes(15) + hashlib.sha256(payload).digest() + payload)


def decode(data):
    require(type(data) is bytes and len(data) == 128, 'wire length/type')
    require(data[:17] == b'2FEGATE2\x01\x00\x01\x00\x80\x00\x00\x00\x02'
            and data[17:32] == bytes(15), 'wire header/reserved')
    require(hashlib.sha256(data[64:]).digest() == data[32:64], 'wire hash')
    gates = []
    for start in (64, 96):
        raw = data[start:start+32]
        require(raw[3:16] == bytes(13), 'gate reserved')
        gates.append(Gate(raw[0], raw[1], int.from_bytes(raw[16:], 'little'), raw[2]))
    require(encode(gates) == data, 'noncanonical wire')
    return tuple(gates)


@dataclass(frozen=True)
class Context:
    cell: tuple
    body: str
    x16: tuple
    x32: tuple | None
    raw: tuple  # Three exact recorded-execution mean costs, all from local16.
    pooled: tuple
    metrics: dict  # Reporting only, keyed by 16/32. Missing components are None.

    def __post_init__(self):
        validate_features(self.x16)
        if self.x32 is not None:
            validate_features(self.x32)
        require(isinstance(self.body, str) and bool(re.fullmatch('[0-9a-f]{64}', self.body)),
                'input body identity')
        for costs in (self.raw, self.pooled):
            require(len(costs) == 3 and all(type(v) is Fraction and v >= 0 for v in costs),
                    'invalid rational common-origin costs')
        require(self.x32 is not None or self.pooled[0] == self.pooled[2],
                'expiration controls not pooled')


def candidate_gates(values):
    result = {DEFER, BUILD}
    for x in values:
        validate_features(x)
    for f in range(len(FEATURES)):
        v = sorted({x[f] for x in values})
        if len(v) < 2:
            continue
        cuts = {v[min(len(v)-2, max(0, k*len(v)//4-1))] for k in (1, 2, 3)}
        for cut in cuts:
            result.update(Gate(2, f, cut, side) for side in (0, 1))
    require(len(result) <= 92, 'gate class exceeded')
    return tuple(sorted(result))


def admissible(gate, rows, stage):
    if gate == DEFER:
        return True
    children = {}
    for row in rows:
        x = row.x16 if stage == 16 else row.x32
        require(x is not None, 'unreachable stage entered')
        child = x[gate.feature] <= gate.cut if gate.kind == 2 else True
        children.setdefault(child, set()).add(row.body)
    if gate.kind == 2:
        return set(children) == {False, True} and all(len(v) >= 2 for v in children.values())
    return len(children.get(True, ())) >= 2


def programme(gates, row):
    if gates[0].build(row.x16):
        return 1
    if row.x32 is not None and gates[1].build(row.x32):
        return 2
    return 0


def tie_key(gates):
    return (sum(g.progress for g in gates), sum(g.kind == 2 for g in gates), gates)


def fit(rows, *, total_cells):
    """Explicit one-pass selection. Never called by evidence collection."""
    require(rows and total_cells >= len(rows), 'empty/invalid fit population')
    require(len({r.cell for r in rows}) == len(rows), 'duplicate fitting cells')
    g16s = candidate_gates([r.x16 for r in rows])
    g32s = candidate_gates([r.x32 for r in rows if r.x32 is not None])
    require(len(g16s)*len(g32s) <= MAX_PAIRS, 'pair budget exceeded')
    best, structural_loss, evaluated = None, None, 0
    for a in g16s:
        if not admissible(a, rows, 16):
            continue
        reached = [r for r in rows if not a.build(r.x16) and r.x32 is not None]
        for b in (g32s if reached else (DEFER,)):
            if not admissible(b, reached, 32):
                continue
            gates = (a, b)
            loss = sum((r.pooled[programme(gates, r)] - min(r.pooled) for r in rows), Fraction())
            key = (loss, *tie_key(gates))
            evaluated += 1
            if not any(g.progress for g in gates):
                structural_loss = loss if structural_loss is None else min(structural_loss, loss)
            if best is None or key < best[0]:
                best = (key, gates)
    require(best is not None, 'no admissible gate pair')
    gates = best[1]
    result = audit(gates, rows, total_cells=total_cells)
    result.update(candidate_gates=[len(g16s), len(g32s)], pairs_evaluated=evaluated,
                  maximum_pairs=MAX_PAIRS, encoded_hex=encode(gates).hex(),
                  gates=[g.__dict__ for g in gates], feature_schema=list(FEATURES))
    result['selected_advantage_over_structural_ns'] = (structural_loss-best[0][0])/total_cells
    require(decode(bytes.fromhex(result['encoded_hex'])) == gates, 'gate round trip')
    return result


def mean(values):
    require(values, 'mean of missing values')
    return sum(values, Fraction()) / len(values)


def audit(gates, rows, *, total_cells):
    """Same selected model under pooled/unpooled costs, never a second search."""
    require(total_cells >= len(rows) > 0, 'audit population')
    costs = {}
    for label, attribute in (('pooled', 'pooled'), ('unpooled', 'raw')):
        selected = sum((getattr(r, attribute)[programme(gates, r)] for r in rows), Fraction())
        oracle = sum((min(getattr(r, attribute)) for r in rows), Fraction())
        costs[label] = dict(selected_ns=selected, selected_ns_per_all_cells=selected/total_cells,
            empirical_regret_ns=(selected-oracle)/total_cells,
            constant_reference_ns={name: sum((getattr(r, attribute)[j] for r in rows), Fraction())
                                   for j, name in enumerate(ARMS)})
    cells = []
    for row in rows:
        selected = programme(gates, row)
        q = row.pooled
        cells.append(dict(cell=row.cell, body=row.body, programme=ARMS[selected],
            raw_ns=row.raw, pooled_ns=q, empirical_regret_ns=q[selected]-min(q),
            missed_ns=max(Fraction(), q[0]-min(q[1:])) if selected == 0 else Fraction(),
            harmful_vs_never_ns=max(Fraction(), q[selected]-q[0]) if selected else Fraction(),
            mistimed_vs_other_install_ns=max(Fraction(), q[selected]-q[3-selected])
                if selected and row.x32 is not None else None))
    breakdowns = {}
    for axis, index in (('platform', 0), ('workers', 4), ('body', None)):
        groups = {}
        for row, cell in zip(rows, cells):
            if index is not None and len(row.cell) != 5:
                continue  # Synthetic fitting contexts need no empirical metadata.
            key = row.body if index is None else row.cell[index]
            groups.setdefault(key, []).append(cell)
        breakdowns[axis] = [dict(value=key, cells=len(group),
            empirical_regret_ns=sum((c['empirical_regret_ns'] for c in group), Fraction()),
            programmes={arm: sum(c['programme'] == arm for c in group) for arm in ARMS})
            for key, group in sorted(groups.items())]
    leaves = []
    for stage, gate in zip((16, 32), gates):
        reached = [r for r in rows if stage == 16 or
                   (not gates[0].build(r.x16) and r.x32 is not None)]
        for branch in ((False, True) if gate.kind == 2 else (True,)):
            selected = [r for r in reached if gate.kind != 2 or
                        ((r.x16 if stage == 16 else r.x32)[gate.feature] <= gate.cut) == branch]
            if not selected:
                continue
            names = sorted({k for r in selected for k in r.metrics.get(stage, {})})
            metrics = {}
            for name in names:
                values = [r.metrics.get(stage, {}).get(name) for r in selected]
                present = [Fraction(x) for x in values if x is not None]
                metrics[name] = dict(mean=mean(present) if present else None,
                                     available=len(present), unavailable=len(values)-len(present))
            if stage == 16:
                continuation = [r.pooled[2 if r.x32 is not None and gates[1].build(r.x32) else 0]
                                for r in selected]
                metrics['fitted_continuation_programme_ns'] = dict(
                    mean=mean(continuation), available=len(continuation), unavailable=0)
            leaves.append(dict(stage=stage, le=branch if gate.kind == 2 else None,
                               cells=len(selected), bodies=len({r.body for r in selected}),
                               means=metrics))
    return dict(status='fitted', scope='empirical projection-suffix costs, not complete-command qualification',
        costs=costs, cells=cells, leaves=leaves, breakdowns=breakdowns,
        error_categories='Missed/harmful/mistimed diagnostics can overlap; empirical regret counts each command once.',
        coverage=dict(contexts=len(rows), total_cells=total_cells,
                      no_episode=total_cells-len(rows), no_episode_treatment_regret=None,
                      at32=sum(r.x32 is not None for r in rows),
                      expired_before32=sum(r.x32 is None for r in rows),
                      required_feature_unknown=0, bodies=len({r.body for r in rows})),
        observation_footprint=dict(wire_bytes=128, gate16_progress=gates[0].progress,
            gate32_progress=gates[1].progress, gate16_samples=[0, 8, 16] if gates[0].progress else [],
            waiting32_samples=[16, 24, 32] if gates[1].progress else [],
            shared_sample16=gates[0].progress and gates[1].progress,
            earlier_short_action_samples=[0, 8] if gates[0].progress else [],
            proxy_entry_visits=None, note='Actual live widths/sample reachability need native observation; no ns estimate.'))


def point_at(action, age):
    points = [p for p in action['checkpoints'] if (p['epoch'], p['local']) == (0, age)]
    require(len(points) <= 1, 'duplicate checkpoint')
    return points[0] if points else None


def stable_point(point):
    return {k: v for k, v in point.items() if k not in ('build', 'stamp_ns')}


def elapsed(action):
    return integer(action['end']['elapsed_ns'], 'action elapsed')


def suffix_cost(call, index, age=16):
    action = call['policy_actions'][index]
    point = point_at(action, age)
    require(point is not None, 'missing common origin')
    stamp = integer(point['stamp_ns'], 'checkpoint stamp')
    require(stamp <= elapsed(action), 'checkpoint after action')
    return elapsed(action)-stamp + sum(elapsed(a) for a in call['policy_actions'][index+1:])


def validate_pool(never, forced, *, source):
    """Exact identical-work check with only source-proven transport normalization.

    Parent shadow accounting charges actual retained allocation capacities.
    At this measured source these exact controls have an observed fixed +2
    Force32 delta, including warmups. This is not a universal string-length
    theorem: a new source/allocation path must establish its own contract.
    """
    require(source == SOURCE, 'unbound transport normalization')
    require(never['execution'] == 'never' and forced['execution'] == 'force32', 'pool controls')
    for call in (never, forced):
        require(not call['route_trace'] and not call['route_hierarchy'], 'pool constructor outcome')
        require(all(not p['build'] for a in call['policy_actions'] for p in a['checkpoints']),
                'pool constructor grant')
        for resource in call['action_resources']:
            for used, limit in (('memory_bytes_used', 'memory_bytes_limit'),
                                ('iterations_used', 'iterations_limit')):
                integer(resource[limit], limit)
                integer(resource[used], used, maximum=resource[limit])
    require(stable_actions(never['policy_actions']) == stable_actions(forced['policy_actions']),
            'pool non-timing trajectory')
    for key in ('fingerprint', 'resource_checks', 'resource_basis'):
        require(never[key] == forced[key], 'pool science/resource classification')
    def solver(call):
        return [row[:8] + row[10:] for row in call['solver_rows']]
    require(solver(never) == solver(forced), 'pool solver signature/science')
    require(len(never['action_resources']) == len(forced['action_resources']), 'pool action count')
    for n, f in zip(never['action_resources'], forced['action_resources']):
        require(n['preset'] == PRESETS['never'] and f['preset'] == PRESETS['force32'],
                'pool preset mismatch')
        require(f['memory_bytes_used'] == n['memory_bytes_used'] + 2, 'pool transport bytes')
        skip = {'preset', 'memory_bytes_limit', 'memory_bytes_used'}
        require({k: v for k, v in n.items() if k not in skip} ==
                {k: v for k, v in f.items() if k not in skip}, 'pool accounted work')


def validate_intervals(call):
    for action in call['policy_actions']:
        end = action['end']
        duration = elapsed(action)
        previous = -1
        for point in action['checkpoints']:
            stamp = integer(point['stamp_ns'], 'checkpoint stamp', maximum=duration)
            require(stamp >= previous, 'nonmonotone economic stamps')
            previous = stamp
            require(all(point[k] == end[k] for k in ACTION), 'checkpoint action identity')
            if point['build'] and end['installed']:
                partition = sum(integer(end[k], k) for k in
                                ('build_ns', 'restart_ns', 'startup_ns', 'continuation_ns'))
                require(duration-stamp == partition, 'economic partition changed')


def reconstruct(cell, *, platform, seed, family, memory, workers, body, source):
    """Validate all nine calls; compile common origins without selecting gates."""
    good, missing = paired_cell(cell, platform=platform, seed=seed, family=family,
                                memory=memory, workers=workers)
    require(all(m['reason'] == 'no_intervention' for m in missing), 'unmatched intervention')
    for call in cell['warmups'] + cell['blocks'][0]['calls']:
        validate_intervals(call)
    warmups = {c['execution']: c for c in cell['warmups']}
    arms = {a: [c for c in cell['blocks'][0]['calls'] if c['execution'] == a] for a in ARMS}
    first = next((i for i, a in enumerate(warmups['never']['policy_actions'])
                  if point_at(a, 16) is not None), None)
    if first is None:
        require(not good and len(missing) == 2, 'installation without first16')
        return None, good, missing
    reference = warmups['never']['policy_actions']
    origin = point_at(reference[first], 16)
    require(all(not a['checkpoints'] for a in reference[:first]), 'earlier decision episode')
    require(origin['role'] in ('packed', 'outcome', 'final'), 'unsupported first16 role')
    for arm in ARMS:
        for call in [warmups[arm], *arms[arm]]:
            actions = call['policy_actions']
            require([action_key(a) for a in actions] == [action_key(a) for a in reference],
                    'immutable action schedule')
            p = point_at(actions[first], 16)
            require(p is not None and stable_point(p) == stable_point(origin), 'first16 state drift')
            require(stable_actions(actions[:first]) == stable_actions(reference[:first]),
                    'pre-episode trajectory drift')
    interventions = {a: [(i, p['local']) for i, action in enumerate(warmups[a]['policy_actions'])
                         for p in action['checkpoints'] if p['build']] for a in ARMS}
    require(interventions['never'] == [] and interventions['force16'] == [(first, 16)],
            'first16 intervention mismatch')
    p32 = point_at(reference[first], 32)
    if p32 is None:
        require(interventions['force32'] == [] and origin['role'] == 'final',
                'expiration or later-action contamination')
        for n, f in zip([warmups['never'], *arms['never']], [warmups['force32'], *arms['force32']]):
            validate_pool(n, f, source=source)
    else:
        require(interventions['force32'] == [(first, 32)], 'same-action32 mismatch')
        for call in [warmups['force32'], *arms['force32']]:
            p = point_at(call['policy_actions'][first], 32)
            require(p is not None and stable_point(p) == stable_point(p32), 'same-action32 state drift')
    raw = tuple(mean([Fraction(suffix_cost(c, first)) for c in arms[a]]) for a in ARMS)
    pooled = raw if p32 is not None else ((raw[0]+raw[2])/2, raw[1], (raw[0]+raw[2])/2)
    metrics = {}
    for age in (16, 32):
        p = point_at(reference[first], age)
        if p is None:
            continue
        end = reference[first]['end']
        steps = integer(end['steps'], 'remaining steps', age)-age
        current = mean([Fraction(elapsed(c['policy_actions'][first]) -
                       point_at(c['policy_actions'][first], age)['stamp_ns']) for c in arms['never']])
        item = dict(remaining_steps=Fraction(steps), incumbent_current_ns=current)
        observation = next((o for o in good if o['control'] == f'force{age}'), None)
        for key in ('B', 'R', 'startup', 'C', 'observed_suffix_gain'):
            item[key] = observation['labels'][key] if observation is not None else None
        item['build_programme_ns'] = mean([Fraction(suffix_cost(c, first, age))
                                          for c in arms[f'force{age}']])
        item['never_programme_ns'] = mean([Fraction(suffix_cost(c, first, age)) for c in arms['never']])
        metrics[age] = item
    return Context((platform, seed, family, memory, workers), body, features(origin),
                   features(p32) if p32 is not None else None, raw, pooled, metrics), good, missing


def unpack(shapes, node):
    if isinstance(node, dict):
        require(len(node) == 1, 'malformed shape node')
        key, values = next(iter(node.items()))
        require(isinstance(key, str) and key.isascii() and key.isdigit()
                and str(int(key)) == key and int(key) < len(shapes), 'shape index')
        fields = shapes[int(key)]
        require(isinstance(values, list) and len(values) == len(fields), 'shape arity')
        return dict(zip(fields, (unpack(shapes, v) for v in values)))
    return [unpack(shapes, v) for v in node] if isinstance(node, list) else node


def validate_run_part(run, record, summary, binding):
    """The frozen Mac recovery schema and Linux no-recovery schema differ.

    Linux has no native_calls_started/retry/discard counters. Its bound runner
    forbids diagnostic resume, has no Linux retry branch, and retains failures.
    Require that exact schema plus the complete retained call census, rather
    than manufacturing empty counters for missing fields.
    """
    platform = record['platform']
    require(platform in ('Darwin', 'Linux'), 'unknown run platform')
    require(summary['platform'] == platform and summary['status'] == 'diagnostic_complete'
            and summary['errors'] == [] and summary['native_calls'] == 864
            and summary['calibration_result_sha256'] == binding['result_sha256']
            and summary['manifest_sha256'] == binding['manifest_sha256'], 'summary incomplete/unbound')
    require(run['platform'] == platform and run['status'] == 'complete'
            and run['failures'] == [] and run['budget_compliant'] is True
            and run['replica'] == 0 and run['shard'] == 0 and len(run['cases']) == 48
            and run['execution_sources'] == binding['execution_sources']
            and run['manifest_sha256'] == binding['manifest_sha256'], 'run incomplete/unbound')
    mac_fields = {'discarded_attempts', 'retry_cells', 'native_calls_started'}
    if platform == 'Darwin':
        require(mac_fields <= run.keys() and run['discarded_attempts'] == []
                and run['retry_cells'] == [] and run['native_calls_started'] == 864,
                'Mac retry/call counters changed')
    else:
        require(mac_fields.isdisjoint(run), 'unexpected Linux retry/call counters')
    warmups = recorded = 0
    for cell in run['cases']:
        require(cell['status'] == 'complete', 'incomplete retained cell')
        integer(cell['host_attempt'], 'host attempt', maximum=0)
        require('reused' not in cell, 'unexpected reused calibration cell')
        if platform == 'Linux':
            require('attempt' not in cell, 'unexpected Linux recovery attempt')
        else:
            integer(cell['attempt'], 'Mac attempt', maximum=0)
        require(len(cell['warmups']) == 3
                and {c['execution'] for c in cell['warmups']} == set(ARMS), 'warmup call census')
        require(len(cell['blocks']) == 1 and cell['blocks'][0]['status'] == 'complete'
                and cell['blocks'][0]['index'] == 0, 'recorded block census')
        calls = cell['blocks'][0]['calls']
        require(len(calls) == 6 and all(sum(c['execution'] == arm for c in calls) == 2
                                      for arm in ARMS), 'recorded call census')
        warmups += len(cell['warmups'])
        recorded += len(calls)
    require((warmups, recorded, warmups+recorded) ==
            (record['warmups'], record['recorded'], record['native_calls']) == (144, 288, 432),
            'retained part call census')


def collect(root):
    """Load this completed historical calibration only; never invoke fit()."""
    root = Path(root)
    def read_bound(path, sha):
        require(not Path(path).is_absolute() and '..' not in Path(path).parts, 'unsafe evidence path')
        data = (root/path).read_bytes()
        require(hashlib.sha256(data).hexdigest() == sha, 'evidence hash changed: '+path)
        return json.loads(data)
    outcome = read_bound(OUTCOME, OUTCOME_SHA)
    require(outcome['measured_source'] == SOURCE, 'measured source changed')
    old = read_bound(outcome['fit']['file'], outcome['fit']['sha256'])
    read_bound(outcome['frozen_protocol']['file'], outcome['frozen_protocol']['sha256'])
    bindings = {b['platform']: b for b in old['bindings']}
    require(set(bindings) == {'Darwin', 'Linux'} and len(old['bindings']) == 2, 'platform provenance')
    contexts, absent, observations, unavailable, files = [], [], [], [], []
    seen, all_bodies = set(), set()
    for platform in ('Darwin', 'Linux'):
        retained, binding = outcome['platforms'][platform], bindings[platform]
        require(binding['result_sha256'] == retained['result_sha256']
                and binding['summary_sha256'] == retained['summary_sha256'], 'result provenance')
        require(len(retained['call_parts']) == 2, 'part count')
        for part in retained['call_parts']:
            record = read_bound(part['file'], part['sha256'])
            require(record['format'] == 'dictionary-shapes-v1' and record['schema'] == 1
                    and record['platform'] == platform and record['native_calls'] == 432
                    and record['warmups'] == 144 and record['recorded'] == 288
                    and record['source_result_sha256'] == binding['result_sha256']
                    and record['source_summary_sha256'] == binding['summary_sha256'], 'part provenance')
            for fields in record['shapes']:
                require(isinstance(fields, list) and all(isinstance(f, str) for f in fields)
                        and len(set(fields)) == len(fields), 'shape fields')
            run = unpack(record['shapes'], record['data'])
            validate_run_part(run, record, retained['summary'], binding)
            for cell in run['cases']:
                match = re.fullmatch(r's(\d+)-(.+)-(fast|low)-t(1|4|8)', cell['id'])
                require(match is not None and cell['status'] == 'complete', 'invalid cell')
                seed, family, memory, workers = match.groups()
                require(int(seed) == record['seed'], 'seed part mismatch')
                identity = (platform, cell['id'])
                require(identity not in seen, 'duplicate cell')
                seen.add(identity)
                body = binding['bodies'][cell['id'].rsplit('-t', 1)[0]]
                all_bodies.add(body)
                row, good, missing = reconstruct(cell, platform=platform, seed=int(seed), family=family,
                    memory=memory, workers=int(workers), body=body, source=SOURCE)
                observations.extend(good)
                unavailable.extend(missing)
                if row is None:
                    absent.append(dict(cell=identity, body=body, reason='no_first16', treatment_regret=None))
                else:
                    contexts.append(row)
            files.append(dict(path=part['file'], sha256=part['sha256']))
    require(len(seen) == 192 and len(all_bodies) == 16 and len(contexts) == 108
            and sum(c.x32 is not None for c in contexts) == 72 and len(absent) == 84
            and len(observations) == 180 and len(unavailable) == 204,
            'frozen calibration coverage changed')
    require(len({c.body for c in contexts}) == 12
            and len({c.body for c in contexts if c.x32 is not None}) == 6, 'episode body coverage')
    return dict(contexts=contexts, no_episode=absent, unavailable=unavailable,
                total_cells=192, native_calls=1728,
                bindings=dict(outcome_sha256=OUTCOME_SHA, calls=files,
                    provenance_fit=outcome['fit'], protocol=outcome['frozen_protocol'],
                    measured_source=SOURCE, platforms=bindings),
                limits=['Parsed records retain online science attestations, not original numeric arrays.',
                        'Training projection suffixes exclude observer/caller costs; no performance qualification.'])


def json_value(value):
    """Lossless report encoding; unknowns remain null."""
    if isinstance(value, Fraction):
        return dict(numerator=value.numerator, denominator=value.denominator)
    if isinstance(value, dict):
        return {str(k): json_value(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [json_value(v) for v in value]
    return value
