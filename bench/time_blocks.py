#!/usr/bin/env python3
"""Frozen complete-solve mirrored blocks with retained failures and host noise.

Seal a finite comparison manifest, collect once into a fresh task directory, then
analyze against an independently validated host-observer receipt. Every attempted
block consumes its slot; exclusions never generate replacements. Native time is
CLOCK_MONOTONIC around the complete plugin call, excluding replay file I/O.

The descriptive estimand is the median of block geometric-mean time ratios.
Acceptance instead tests the median of E=N-B-max(.05*B,.001), where B and N are
the block's geometric means of two reference/candidate solves. The sign/order-
statistic upper bound uses alpha=.025. Its finite-sample coverage is conditional
on independent scores with a common median; randomization, multiple session
labels and A/A observations do not establish that assumption. This tool reports
conditional statistical results, never an unconditional release qualification.

Schema 1 retains pairwise ABBA/BAAB. Schema 2 executes M arms once each in a
permutation and its reverse (2M calls), storing each solve once. Reversed
permutations paired across rounds balance every pair's orientation exactly.
Every derived comparison shares the entire block's host eligibility. Protected
references use 5%, internal references 20%, both with the 1 ms absolute margin.
Normalized regret (N-B)/max(relative*B,.001) is reported for ranking, with a
threshold of one. Shared-block comparisons are correlated; their individual
bounds do not provide a simultaneous confidence statement or independence.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import random
import re
import statistics
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
ALPHA = Fraction(1, 40)
SIDES = ('reference', 'candidate')
CLOCK = 'CLOCK_MONOTONIC around the complete native plugin call; replay I/O excluded'
ASSUMPTIONS = 'Independent analysis-unit scores with a common median; not implied by exchangeability, randomized order, A/A, or session count.'
CLEARED_ENV = ('FEREG_SOLVER_TRACE', 'FEREG_SOLVER_TRACE_FILE', 'FEREG_PHASE_TIMING',
               'FEREG_CMG_ORDINARY', 'FEREG_ACTION_MEMORY_AUDIT', 'FEREG_PPML_MEMORY_TRACE',
               'FEREG_MEMORY_LIMIT_BYTES', 'LLVM_PROFILE_FILE')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b''):
            result.update(chunk)
    return result.hexdigest()


def identity(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()).hexdigest()


def read(path):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            require(key not in out, 'Duplicate JSON key ' + key)
            out[key] = value
        return out
    return json.loads(Path(path).read_text(), object_pairs_hook=unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)))


def reference(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'sha256': digest(path)}


def check(bound):
    require(isinstance(bound, dict) and set(bound) == {'path', 'sha256'} and reference(bound['path']) == bound,
            'Frozen path/hash binding changed: ' + str(bound))
    return Path(bound['path'])


def atomic_json(path, value, *, fresh=False):
    """Readers see either the previous checkpoint or this complete checkpoint."""
    path = Path(path)
    require(not fresh or not path.exists(), 'Refusing to overwrite ' + str(path))
    temporary = path.with_name(path.name + '.pending')
    with temporary.open('x') as stream:
        try:
            json.dump(value, stream, indent=2, allow_nan=False)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
            os.replace(temporary, path)
        except BaseException:
            temporary.unlink(missing_ok=True)
            raise


def task_output(path):
    path = Path(path).resolve()
    require(path.is_relative_to((ROOT / 'target/tmp').resolve()), 'Outputs belong under target/tmp/<task>/')
    return path


def module_at(path):
    spec = importlib.util.spec_from_file_location('block_bound_' + digest(path)[:16], path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def units(spec):
    return spec['comparisons'] if spec['schema'] == 1 else spec['cells']


def execution_arms(item):
    return {side: item[side] for side in SIDES} if 'arms' not in item else item['arms']


def unit_id(row):
    return row.get('cell_id', row.get('comparison_id'))


def arm_id(call):
    return call.get('arm_id', call.get('side'))


def planned_arms(row):
    return row.get('arms', row.get('sides'))


def validate_spec(value):
    require(value.get('schema') in {1, 2}, 'Unsupported block manifest schema')
    multi = value['schema'] == 2
    require(set(value) == {'schema', 'harness', 'design', 'cells' if multi else 'comparisons', 'monitor', 'declarations'}, 'Unknown/missing block manifest fields')
    design = value['design']
    require(set(design) == {'attempts', 'warmups', 'seed', 'timeout_s', 'budget_s', 'session_id', 'analysis_unit', 'alpha'},
            'Freeze all block design fields')
    require(type(design['attempts']) is int and 2 <= design['attempts'] <= 1000 and design['attempts'] % 2 == 0,
            'Require an even, fixed 2..1000 attempted blocks per comparison')
    require(type(design['warmups']) is int and 1 <= design['warmups'] <= 5, 'Freeze 1..5 warmups per arm')
    require(type(design['seed']) is int and 0 <= design['seed'] < 2**64, 'Invalid schedule seed')
    require(isinstance(design['session_id'], str) and design['session_id'] and design['analysis_unit'] == 'block'
            and design['alpha'] == {'numerator': 1, 'denominator': 40}, 'Freeze session, block analysis unit and alpha=1/40')
    for key, limit in [('timeout_s', 3600), ('budget_s', 86400)]:
        require(type(design[key]) in (int, float) and math.isfinite(design[key]) and 0 < design[key] <= limit, 'Invalid ' + key)
    comparisons = units(value)
    require(isinstance(comparisons, list) and 1 <= len(comparisons) <= 256, 'Require a finite 1..256 comparison list')
    require(len({item['id'] for item in comparisons}) == len(comparisons), 'Duplicate comparison')
    pair_ids = set()
    for item in comparisons:
        require(set(item) == ({'id', 'workload', 'arms', 'comparisons', 'validation'} if multi else
                             {'id', 'workload', 'reference', 'candidate', 'validation'}), 'Unknown/missing comparison fields')
        require(all(isinstance(item[k], str) and re.fullmatch(r'[A-Za-z0-9_-]{1,100}', item[k]) for k in ['id', 'workload']), 'Unsafe comparison/workload identifier')
        arms = execution_arms(item)
        require(isinstance(arms, dict) and 2 <= len(arms) <= 32, 'Require 2..32 execution arms per cell')
        for key, arm in arms.items():
            require(isinstance(key, str) and re.fullmatch(r'[A-Za-z0-9_-]{1,100}', key), 'Unsafe arm identifier')
            require(set(arm) == {'label', 'plugin', 'input'} and isinstance(arm['label'], str) and arm['label'], 'Incomplete arm')
        require(len({arm['label'] for arm in arms.values()}) == len(arms), 'A/A uses distinct labels even for identical binaries')
        if multi:
            pairs, roles, directed, used = item['comparisons'], {}, set(), set()
            require(isinstance(pairs, list) and 1 <= len(pairs) <= len(arms) * (len(arms)-1), 'Require bounded declared comparisons')
            for pair in pairs:
                require(set(pair) == {'id', 'reference', 'candidate', 'reference_role'}, 'Unknown/missing comparison role or IDs')
                require(isinstance(pair['id'], str) and re.fullmatch(r'[A-Za-z0-9_-]{1,100}', pair['id'])
                        and pair['id'] not in pair_ids, 'Duplicate/unsafe derived comparison ID')
                pair_ids.add(pair['id'])
                reference_id, candidate_id = pair['reference'], pair['candidate']
                require(reference_id in arms and candidate_id in arms and reference_id != candidate_id
                        and (reference_id, candidate_id) not in directed, 'Unknown/self/duplicate arm comparison')
                require(pair['reference_role'] in {'protected', 'internal'}, 'Freeze protected/internal reference role')
                require(reference_id not in roles or roles[reference_id] == pair['reference_role'], 'Inconsistent role for shared reference')
                roles[reference_id] = pair['reference_role']
                directed.add((reference_id, candidate_id)); used.update([reference_id, candidate_id])
            require(used == set(arms), 'Every execution arm needs a declared comparison')
        validation = item['validation']
        require(validation.get('kind') in ({'exact', 'routing', 'routing_blocks'} if multi else {'exact', 'routing'}), 'Unsupported scientific adapter')
        if validation['kind'] == 'exact':
            require(set(validation) == {'kind', 'poisson', 'qualification'} and type(validation['poisson']) is bool,
                    'Exact adapter requires model kind and bound prerequisite evidence')
        elif validation['kind'] == 'routing':
            require(set(validation) == {'kind', 'manifest', 'witness', 'source', 'cell_id', 'stage', 'previous'}, 'Incomplete routing validation packet')
            require(validation['stage'] in {'A', 'B', 'C'}, 'Invalid prequalified routing stage')
        else:
            require(set(validation) == {'kind', 'context', 'qualification', 'witness', 'source', 'cell_id'}, 'Incomplete block-routing science context')
    require(not multi or (len(pair_ids) <= 4096 and sum(len(execution_arms(item)) * (design['warmups'] + 2*design['attempts'])
                for item in comparisons) <= 1_000_000), 'Declared panel exceeds finite call/comparison bound')
    require(isinstance(value['declarations'], list) and value['declarations'], 'Bind prospective design/qualification declarations')
    monitor = value['monitor']
    require(monitor is None or set(monitor) == {'validator', 'policy', 'dependencies'}, 'Invalid external monitor contract')


def schedule(spec):
    """Exactly balanced orientations; shuffle workloads and comparisons each round."""
    if spec['schema'] == 2:
        return mirrored_schedule(spec)
    design = spec['design']
    rng = random.Random(design['seed'])
    groups = {}
    for item in spec['comparisons']:
        groups.setdefault(item['workload'], []).append(item['id'])
    orientations = {}
    for item in spec['comparisons']:
        values = ['ABBA', 'BAAB'] * (design['attempts'] // 2)
        rng.shuffle(values)
        orientations[item['id']] = values
    def ordered():
        workloads = list(groups)
        rng.shuffle(workloads)
        for workload in workloads:
            comparisons = groups[workload].copy()
            rng.shuffle(comparisons)
            yield from comparisons
    warmups = []
    for repetition in range(design['warmups']):
        for cid in ordered():
            sides = list(SIDES)
            rng.shuffle(sides)
            warmups.append({'comparison_id': cid, 'repetition': repetition, 'sides': sides})
    blocks = []
    for round_id in range(design['attempts']):
        for cid in ordered():
            orientation = orientations[cid][round_id]
            blocks.append({'block_id': f'b{len(blocks):06d}', 'comparison_id': cid, 'round': round_id,
                           'orientation': orientation, 'sides': ['reference' if c == 'A' else 'candidate' for c in orientation]})
    return {'warmups': warmups, 'blocks': blocks}


def mirrored_schedule(spec):
    """Pair each first-half permutation with its reverse; never choose from timings."""
    rng, design = random.Random(spec['design']['seed']), spec['design']
    permutations, groups = {}, {}
    for cell in spec['cells']:
        values = []
        for _ in range(design['attempts'] // 2):
            first = list(cell['arms']); rng.shuffle(first)
            values.extend([first, first[::-1]])
        rng.shuffle(values)
        permutations[cell['id']] = values
        groups.setdefault(cell['workload'], []).append(cell)
    def ordered():
        workloads = list(groups); rng.shuffle(workloads)
        for workload in workloads:
            cells = groups[workload].copy(); rng.shuffle(cells)
            yield from cells
    warmups, blocks = [], []
    for repetition in range(design['warmups']):
        for cell in ordered():
            arms = list(cell['arms']); rng.shuffle(arms)
            warmups.append({'cell_id': cell['id'], 'repetition': repetition, 'arms': arms})
    for round_id in range(design['attempts']):
        for cell in ordered():
            first = permutations[cell['id']][round_id]
            blocks.append({'block_id': f'b{len(blocks):06d}', 'cell_id': cell['id'], 'round': round_id,
                           'permutation': first, 'arms': first + first[::-1]})
    return {'warmups': warmups, 'blocks': blocks}


def source_bindings():
    names = ['time_blocks.py', 'time_exact.py', 'native_io.py', 'routing.py', 'routing_run.py',
             'routing_cases.py', 'routing_oracle.py', 'cmgschur.py', 'cmgschur_cases.py', 'native.py',
             'routing_science_completion.py']
    paths = [Path(__file__).with_name(name) for name in names]
    paths += [ROOT / 'scripts' / name for name in ['replay_plugins.py', 'replay_psd.py', 'replay_bias_reference.py', 'replay_cmg_polish.py']]
    paths.append(ROOT / 'crates/fereg-oracle/python/replay_ols_reference.py')
    return [reference(path) for path in paths]


class ExactScience:
    def __init__(self, item):
        self.poisson = item['validation']['poisson']
        self.expected = None
        self.bindings = [item['validation']['qualification']]
        qualified = read(check(self.bindings[0]))
        require(qualified.get('status') == 'complete' and qualified.get('all_exact') is True
                and qualified.get('exactness') == 'all plugins', 'Exact adapter requires a complete exact-science qualification')
        from native_io import case_header
        self.shapes = {}
        for side, arm in execution_arms(item).items():
            with check(arm['input']).open('rb') as stream:
                shape, args = case_header(stream)
            require((args[0] == b'estimate-ppml') == self.poisson, 'Declared model differs from input')
            self.shapes[side] = shape
            matches = [Path(path).name for path, sha in qualified['cases'].items() if sha == arm['input']['sha256']]
            require(len(matches) == 1 and arm['plugin']['sha256'] in
                    {entry['sha256'] for entry in qualified['plugins'].values()}, 'Exact input/plugin lacks prior qualification')
            rows = [row for row in qualified['records'] if row['case'] == matches[0]]
            slots = [(row['plugin'], row['repetition']) for row in rows]
            expected_slots = {(label, repetition) for label in qualified['plugins']
                              for repetition in range(-qualified['warmups'], qualified['repetitions'])}
            require(len(slots) == len(set(slots)) and set(slots) == expected_slots,
                    'Qualified case lacks complete per-artifact scientific observations')
            fingerprints = {row['science_sha256'] for row in rows}
            require(len(fingerprints) == 1, 'Qualified case has missing or inconsistent science fingerprints')
            expected = next(iter(fingerprints))
            if self.expected is None:
                self.expected = expected
            require(self.expected == expected, 'Exact comparison arms have different qualified science')

    def fingerprint(self, side):
        return self.expected

    def check(self, side, result, log):
        from replay_plugins import exact_view, matrix
        raw = read(result)
        require(raw['return_code'] == 0 and matrix(raw, 'RREC')[-1] == 1 and matrix(raw, 'RCERT')[-1] == 1,
                'Native scientific certificate failed')
        require(raw['columns'] == [], 'Timing harness must emit binary columns')
        columns = result.with_suffix('.json.columns')
        require(columns.stat().st_size == 8 * self.shapes[side][0] * self.shapes[side][1], 'Native output shape changed')
        view = exact_view(raw, self.poisson)
        view['columns_sha256'] = digest(columns)
        fingerprint = identity(view)
        require(fingerprint == self.expected, 'Exact science or solver signature differs within/between arms')
        return {'science_sha256': fingerprint, 'certificate_pass': True, 'science_pass': True}


class RoutingScience:
    """Existing independently qualified per-arm science, never comparator parity alone."""
    def __init__(self, item):
        import cmgschur
        import routing
        import routing_oracle
        import routing_run as runner
        from routing_cases import inspect_case, inactive_case_oracle
        self.item, self.runner, self.cmg = item, runner, cmgschur
        packet = item['validation']
        context_bindings = []
        if packet['kind'] == 'routing_blocks':
            import routing_blocks
            context = routing_blocks.science_context(packet)
            manifest, cell, required = context['manifest'], context['cell'], context['required']
            stage, selection_sha256 = context['stage'], context['selection_sha256']
            context_bindings = context['bindings']
            for bound in context_bindings:
                check(bound)
        else:
            manifest_path = check(packet['manifest'])
            manifest = routing.unseal(read(manifest_path))
            routing.validate_manifest(manifest)
            previous = routing.unseal(read(check(packet['previous']))) if packet['previous'] else None
            candidates = routing.required_candidates(manifest, packet['stage'], previous)
            cell = next(c for c in manifest['panels'][packet['stage']] if c['id'] == packet['cell_id'])
            required = list(dict.fromkeys(['baseline', *manifest['protections'], *candidates, *cell['comparators']]))
            stage, selection_sha256 = packet['stage'], routing.identity(previous) if previous else None
        validate_reference_roles(item, manifest['protections'])
        source = check(packet['source'])
        require(digest(source) == cell['case_sha256'], 'Routing source differs from frozen qualification')
        self.meta = inspect_case(source, cell['family'])
        require(self.meta['body_sha256'] == cell['body_sha256'], 'Routing numeric/model body changed')
        witness_path = check(packet['witness'])
        if packet['kind'] == 'routing_blocks':
            bundle = context['science']
            witness, proof, witness_path = bundle['witness'], bundle['proof'], bundle['witness_path']
            self.bindings = context_bindings
        else:
            witness, dependencies = runner.evidence(witness_path, cell, manifest, required, stage=stage,
                                                   selection_sha256=selection_sha256)
            proof = routing.unseal(read((witness_path.parent / witness['scoped_replay']['path']).resolve()))
            self.bindings = [reference(path) for path in dependencies]
        sidecar = source.with_suffix('.json')
        if sidecar.exists():
            side = read(sidecar)
            require(side.get('case_sha256') == cell['case_sha256'], 'Oracle sidecar input binding changed')
            self.bindings.append(reference(sidecar))
            if 'oracle' in side:
                self.meta['oracle'] = side['oracle']
            if 'replay_reference' in side:
                target = side['replay_reference']
                path = (sidecar.parent / target['file']).resolve()
                require(digest(path) == target['sha256'], 'Replay oracle binding changed')
                self.bindings.append(reference(path))
                self.meta['replay_reference'] = read(path)
                require(self.meta['replay_reference'].get('outputs') == self.meta['outputs']
                        and self.meta['replay_reference'].get('saved', []) == self.meta.get('saved', []), 'Oracle output mapping differs')
        if cell['family'] == 'inactive-columns' and 'oracle' not in self.meta:
            self.meta['oracle'] = inactive_case_oracle(source)
        independent = witness['independent_witness']
        oracle_path = (witness_path.parent / independent['path']).resolve()
        descriptor = read(oracle_path)
        if isinstance(descriptor, dict) and descriptor.get('kind') == routing_oracle.KIND:
            self.meta['routing_oracle'], paths = routing_oracle.load_reference(oracle_path, cell['case_sha256'], cell['body_sha256'])
            self.bindings.extend(reference(path) for path in paths)
        require(any(key in self.meta for key in ['oracle', 'replay_reference', 'routing_oracle']), 'Missing executable independent truth')
        self.expected, self.reader = {}, {}
        for side, execution in execution_arms(item).items():
            arm = execution['label']
            require(arm in required and witness['arm_outcomes'][arm]['status'] == 'passed', 'Arm has no successful scientific prerequisite; do not retry unavailable arms')
            require(execution['plugin']['sha256'] == manifest['arms'][arm]['sha256'], 'Qualified artifact changed')
            origin = next(row for row in proof['prerequisites'] if row['arm'] == arm)
            require(execution['input']['sha256'] == origin['prepared_sha256'], 'Prepared routing request changed')
            self.expected[side] = origin['science_sha256']
            self.reader[side] = runner.reader_arm(manifest, arm)
            runner.verify_public_request(check(execution['input']), manifest, arm)
        self.memory_limit = cell.get('memory_limit_bytes')

    def fingerprint(self, side):
        return self.expected[side]

    def check(self, side, result, log):
        raw, detail = self.cmg.read_run(result, log, self.meta, self.reader[side])
        require(detail['science_sha256'] == self.expected[side], 'Call differs from independently qualified science/signature')
        self.runner.check_oracle(raw, result.with_suffix('.json.columns'), self.meta)
        return {'science_sha256': detail['science_sha256'], 'certificate_pass': True, 'science_pass': True}


def validate_reference_roles(item, protections):
    for pair in item.get('comparisons', []):
        label = item['arms'][pair['reference']]['label']
        expected = 'protected' if label in protections else 'internal'
        require(pair['reference_role'] == expected, 'Reference allowance differs from qualified protection role')


def adapters(spec):
    return {item['id']: (ExactScience(item) if item['validation']['kind'] == 'exact' else RoutingScience(item))
            for item in units(spec)}


def all_bindings(spec, science):
    values = [spec['harness'], *spec['declarations']]
    for item in units(spec):
        values += [arm[field] for arm in execution_arms(item).values() for field in ['plugin', 'input']]
        values += science[item['id']].bindings
        packet = item['validation']
        if packet['kind'] == 'routing':
            values += [packet[key] for key in ['manifest', 'witness', 'source']]
            if packet['previous']:
                values.append(packet['previous'])
        elif packet['kind'] == 'routing_blocks':
            values += [packet[key] for key in ['context', 'qualification', 'witness', 'source']]
            values.append(reference(Path(__file__).with_name('routing_blocks.py')))
    if spec['monitor']:
        values += [spec['monitor']['validator'], spec['monitor']['policy'], *spec['monitor']['dependencies']]
    values += source_bindings()
    values.append(reference(sys.executable))
    unique = {}
    for bound in values:
        check(bound)
        require(bound['path'] not in unique or unique[bound['path']] == bound['sha256'], 'Conflicting binding')
        unique[bound['path']] = bound['sha256']
    return [{'path': path, 'sha256': unique[path]} for path in sorted(unique)]


def freeze(spec, output):
    validate_spec(spec)
    science = adapters(spec)
    value = {'schema': spec['schema'], 'spec': spec, 'schedule': schedule(spec), 'bindings': all_bindings(spec, science),
             'clock': CLOCK, 'assumptions': ASSUMPTIONS}
    atomic_json(task_output(output), {'sha256': identity(value), 'value': value}, fresh=True)
    return value


def load_manifest(path):
    envelope = read(path)
    value = envelope['value']
    require(envelope['sha256'] == identity(value), 'Manifest seal changed')
    validate_spec(value['spec'])
    require(value['schema'] == value['spec']['schema'], 'Manifest/schema version mismatch')
    require(value['schedule'] == schedule(value['spec']) and value['clock'] == CLOCK and value['assumptions'] == ASSUMPTIONS,
            'Frozen schedule or estimand changed')
    for bound in value['bindings']:
        check(bound)
    science = adapters(value['spec'])
    require(value['bindings'] == all_bindings(value['spec'], science), 'Undeclared or changed scientific/tool dependency')
    return value, science


def emit(events, result, event, block=None, call=None, **extra):
    row = {'schema': 1, 'event': event, 'run_id': result['run_id'], 'block_id': block['block_id'] if block else None,
           'comparison_id': block.get('comparison_id') if block else call.get('comparison_id') if call else None,
           'round': block['round'] if block else None, 'session_id': result['session_id'],
           'wall_time_ns': time.time_ns(), 'monotonic_ns': time.monotonic_ns(), 'tool_pid': os.getpid(),
           'native_pid': call.get('native_pid') if call else None, 'call_id': call['call_id'] if call else None,
           'process_token': call.get('process_token') if call else None, **extra}
    if result['schema'] == 2:
        row['cell_id'] = block['cell_id'] if block else call.get('cell_id') if call else extra.get('cell_id')
        row['arm_id'] = call.get('arm_id') if call else None
    events.write(json.dumps(row, allow_nan=False) + '\n')
    events.flush()
    os.fsync(events.fileno())
    return row


def run_native(command, log, timeout, on_start):
    """No detached child group; cancellation preserves the original error."""
    with log.open('xb') as stream:
        process = subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT)
        try:
            on_start(process.pid)
            code = process.wait(timeout=timeout)
        except BaseException as error:
            try:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=5)
            except BaseException as cleanup:
                error.__notes__ = [*getattr(error, '__notes__', []),
                                   'Native cleanup could not be confirmed: ' + repr(cleanup)]
            raise
    require(code == 0, 'Native process return code ' + str(code))


def collect(manifest_path, output, *, execute=run_native):
    value, science = load_manifest(manifest_path)
    spec, planned = value['spec'], value['schedule']
    design = spec['design']
    output = task_output(output)
    output.mkdir(parents=True, exist_ok=False)
    comparisons = {item['id']: item for item in units(spec)}
    multi = spec['schema'] == 2
    result = {'schema': spec['schema'], 'status': 'running', 'run_id': str(uuid.uuid4()), 'session_id': design['session_id'],
              'manifest': reference(manifest_path), 'platform': platform.platform(), 'clock': CLOCK,
              'schedule': planned, 'warmups': [], 'blocks': [{**block, 'status': 'not_run', 'calls': [],
                  'start_monotonic_ns': None, 'end_monotonic_ns': None} for block in planned['blocks']]}
    began = time.monotonic()
    def stat_identity(path):
        stat = Path(path).stat()
        return stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns
    frozen_stats = {bound['path']: stat_identity(bound['path']) for bound in value['bindings']}
    def save():
        result['elapsed_s'] = time.monotonic() - began
        atomic_json(output / 'records.json', result)
    def unchanged(full=False):
        # Full bytes are checked before/after the panel. Stat guards between
        # calls avoid repeatedly streaming every unrelated large input.
        require(all(stat_identity(path) == expected for path, expected in frozen_stats.items()), 'Frozen input/artifact/source metadata changed')
        if full:
            for bound in value['bindings']:
                check(bound)
        require(reference(manifest_path) == result['manifest'], 'Manifest changed during collection')
    serial = 0
    with (output / 'host-events.jsonl').open('x') as events:
        emit(events, result, 'run_start')
        save()
        def one(cid, side, block, calls):
            nonlocal serial
            item, adapter = comparisons[cid], science[cid]
            call = {'call_id': f'c{serial:07d}', 'cell_id' if multi else 'comparison_id': cid,
                    'arm_id' if multi else 'side': side, 'status': 'running'}
            serial += 1
            calls.append(call)
            target = output / (call['call_id'] + '.json')
            log = output / (call['call_id'] + '.log')
            try:
                unchanged()
                remaining = design['budget_s'] - (time.monotonic() - began)
                if remaining <= 0:
                    raise TimeoutError('Aggregate collection budget exhausted')
                timeout = min(design['timeout_s'], remaining)
                call['timeout_s'] = timeout
                command = ['env', *(part for key in CLEARED_ENV for part in ('-u', key))]
                if getattr(adapter, 'memory_limit', None) is not None:
                    command.append('FEREG_MEMORY_LIMIT_BYTES=' + str(adapter.memory_limit))
                arm = execution_arms(item)[side]
                command += [spec['harness']['path'], arm['plugin']['path'], '--replay', arm['input']['path'], str(target)]
                def started(pid):
                    call.update(native_pid=pid, process_token=result['run_id'] + '/' + call['call_id'])
                    emit(events, result, 'call_start', block, call)
                execute(command, log, timeout, started)
                unchanged()
                call.update(adapter.check(side, target, log))
                values = re.findall(r'^NATIVE_SECONDS=([0-9.eE+-]+)$', log.read_text(errors='replace'), re.MULTILINE)
                require(len(values) == 1 and math.isfinite(float(values[0])) and float(values[0]) > 0, 'Missing/nonfinite complete native-call timing')
                call.update(status='passed', seconds=float(values[0]), result_sha256=digest(target),
                            columns_sha256=digest(target.with_suffix('.json.columns')), log_sha256=digest(log))
                # Successful numeric outputs are exactly fingerprinted; failures
                # retain their full result/columns/log for numerical diagnosis.
                target.unlink()
                target.with_suffix('.json.columns').unlink()
            except BaseException as error:
                call.update(status='failed', error=type(error).__name__ + ': ' + str(error),
                            error_notes=getattr(error, '__notes__', []),
                            failure_kind='timeout' if isinstance(error, (TimeoutError, subprocess.TimeoutExpired)) else 'science_or_infrastructure')
                call['artifacts'] = {name: reference(path) for name, path in [('result', target), ('columns', target.with_suffix('.json.columns')), ('log', log)] if path.exists()}
                raise
            finally:
                emit(events, result, 'call_end', block, call, status=call['status'])
        try:
            for warmup_index, warmup in enumerate(planned['warmups']):
                row = {**warmup, 'status': 'running', 'calls': []}
                result['warmups'].append(row)
                event_unit = {'cell_id' if multi else 'comparison_id': unit_id(warmup)}
                emit(events, result, 'warmup_start', warmup_id=warmup_index, **event_unit)
                try:
                    for side in planned_arms(warmup):
                        one(unit_id(warmup), side, None, row['calls'])
                    row['status'] = 'passed'
                finally:
                    if row['status'] != 'passed': row['status'] = 'failed'
                    emit(events, result, 'warmup_end', status=row['status'], warmup_id=warmup_index, **event_unit)
                    save()
            for block in result['blocks']:
                block['status'] = 'running'
                block['start_monotonic_ns'] = emit(events, result, 'block_start', block)['monotonic_ns']
                save()
                try:
                    for side in planned_arms(block):
                        one(unit_id(block), side, block, block['calls'])
                    block['status'] = 'complete'
                finally:
                    if block['status'] != 'complete': block['status'] = 'failed'
                    block['end_monotonic_ns'] = emit(events, result, 'block_end', block, status=block['status'])['monotonic_ns']
                    save()  # atomic checkpoint after every complete or failed block
            unchanged(full=True)
            result['status'] = 'complete'
        except BaseException as error:
            result.update(status='failed', error=type(error).__name__ + ': ' + str(error))
            raise
        finally:
            emit(events, result, 'run_end', status=result['status'])
            save()
    return result


def block_scores(block, relative=.05):
    require(block['status'] == 'complete' and len(block['calls']) == 4
            and [call['side'] for call in block['calls']] == block['sides'], 'Incomplete or reordered block')
    times = {}
    for side in SIDES:
        rows = [call for call in block['calls'] if call['side'] == side]
        require(len(rows) == 2 and all(call.get('status') == 'passed' and call.get('science_pass') is True
                and call.get('certificate_pass') is True and type(call.get('seconds')) in (int, float)
                and math.isfinite(call['seconds']) and call['seconds'] > 0 for call in rows), 'Failed or nonfinite solve in block')
        times[side] = math.exp(statistics.mean(math.log(call['seconds']) for call in rows))
    b, n = times['reference'], times['candidate']
    allowance = max(relative * b, .001)
    ratio, excess, regret = n / b, n - b - allowance, (n - b) / allowance
    require(all(math.isfinite(x) for x in [ratio, excess, regret]), 'Nonfinite block score')
    return {**times, 'ratio': ratio, 'excess_seconds': excess, 'normalized_regret': regret}


def comparison_views(spec, result):
    """Derived four-call views reference shared stored calls; no pairwise exclusions."""
    if spec['schema'] == 1:
        for item in spec['comparisons']:
            yield item, [block for block in result['blocks'] if unit_id(block) == item['id']], None, None
        return
    for cell in spec['cells']:
        blocks = [block for block in result['blocks'] if unit_id(block) == cell['id']]
        for block in blocks:
            if block['status'] == 'complete':
                require(len(block['calls']) == 2*len(cell['arms']) and [arm_id(call) for call in block['calls']] == block['arms']
                        and all(call.get('status') == 'passed' and call.get('science_pass') is True
                                and call.get('certificate_pass') is True and type(call.get('seconds')) in (int, float)
                                and math.isfinite(call['seconds']) and call['seconds'] > 0 for call in block['calls']),
                        'Incomplete scientific mirrored block; no clean pairwise subset')
        for pair in cell['comparisons']:
            names = {pair[side]: side for side in SIDES}
            views = []
            for block in blocks:
                calls = [{**call, 'side': names[arm_id(call)]} for call in block['calls'] if arm_id(call) in names]
                sides = ['reference' if arm == pair['reference'] else 'candidate' for arm in block['arms'] if arm in names]
                views.append({**block, 'calls': calls, 'sides': sides,
                              'orientation': ''.join('A' if side == 'reference' else 'B' for side in sides)})
            yield {'id': pair['id'], 'execution_ids': {side: pair[side] for side in SIDES},
                   **{side: cell['arms'][pair[side]] for side in SIDES}}, views, pair['reference_role'], cell['id']


def median_interval(values):
    """One-sided upper alpha=.025; ties retained, unbounded when unattainable."""
    require(values and all(math.isfinite(value) for value in values), 'Median interval needs finite observations')
    ordered, n = sorted(values), len(values)
    denominator = 2**n
    tail = denominator
    rank = None
    for k in range(1, n + 1):
        tail -= math.comb(n, k - 1)
        if Fraction(tail, denominator) <= ALPHA:
            rank = k
            break
    return {'median': statistics.median(ordered), 'lower': ordered[n-rank] if rank else None,
            'upper': ordered[rank-1] if rank else None, 'upper_unbounded': rank is None,
            'upper_rank_1based': rank, 'n': n, 'alpha': {'numerator': 1, 'denominator': 40},
            'attained_upper_tail': {'numerator': tail, 'denominator': denominator} if rank else None,
            'coverage': 'Conditional on independent scores with a common median; ties are retained conservatively.'}


def lag_one(values):
    if len(values) < 3:
        return None
    left, right = values[:-1], values[1:]
    a, b = statistics.mean(left), statistics.mean(right)
    denominator = math.sqrt(sum((x-a)**2 for x in left) * sum((x-b)**2 for x in right))
    return sum((x-a)*(y-b) for x,y in zip(left,right)) / denominator if denominator else None


def monitor_context(manifest_path, records_path):
    records_path = Path(records_path).resolve()
    result = read(records_path)
    return {'manifest': reference(manifest_path), 'records': reference(records_path),
            'events': reference(records_path.parent / 'host-events.jsonl'), 'run_id': result['run_id'],
            'schedule': result['schedule']['blocks'],
            'blocks': [{key: block[key] for key in ['block_id', 'start_monotonic_ns', 'end_monotonic_ns', 'status']} for block in result['blocks']]}


def host_classification(spec, context, receipt):
    if spec['monitor'] is None or receipt is None:
        return None
    contract = spec['monitor']
    for bound in [contract['validator'], contract['policy'], *contract['dependencies']]:
        check(bound)
    module = module_at(check(contract['validator']))
    result = module.validate_receipt(Path(receipt).resolve(), context)
    require(result.get('status') == 'complete' and result.get('policy_sha256') == contract['policy']['sha256']
            and all(result.get(key + '_sha256') == context[key]['sha256'] for key in ['manifest', 'records', 'events']), 'Monitor receipt does not bind this collection')
    ids = [block['block_id'] for block in context['schedule']]
    rows = result['blocks']
    require(len(rows) == len(ids) and {row['block_id'] for row in rows} == set(ids), 'Monitor omitted/duplicated blocks')
    for row in rows:
        require(row['status'] in {'eligible', 'excluded'} and isinstance(row.get('evidence'), list) and row['evidence'], 'Missing block monitoring evidence')
        require(row.get('reason') is None if row['status'] == 'eligible' else row.get('reason') in {'known_competing_compute', 'host_fault'}, 'Outcome/ordinary OS noise is not an exclusion reason')
        for bound in row['evidence']:
            check(bound)
    return {row['block_id']: row for row in rows}


def analyze(manifest_path, records_path, monitor_receipt=None):
    value, science = load_manifest(manifest_path)
    result = read(records_path)
    require(result['schema'] == value['spec']['schema'] and result['manifest'] == reference(manifest_path) and result['schedule'] == value['schedule']
            and result['session_id'] == value['spec']['design']['session_id'], 'Collection does not bind frozen design')
    require(len(result['blocks']) == len(value['schedule']['blocks']) and all(
        all(actual.get(key) == expected[key] for key in expected) for actual, expected in zip(result['blocks'], value['schedule']['blocks'])), 'Missing/reordered attempted block slots')
    all_calls = [call for row in [*result['warmups'], *result['blocks']] for call in row['calls']]
    require(len({call['call_id'] for call in all_calls}) == len(all_calls), 'Duplicated native call identity')
    arm_sets = {item['id']: set(execution_arms(item)) for item in units(value['spec'])}
    require(all(unit_id(call) == unit_id(row) and arm_id(call) in arm_sets[unit_id(row)]
                for row in [*result['warmups'], *result['blocks']] for call in row['calls']), 'Misassigned native call')
    for call in all_calls:
        require(re.fullmatch(r'c[0-9]{7}', call['call_id']), 'Invalid native call identity')
        if call.get('status') == 'passed':
            require(call['science_sha256'] == science[unit_id(call)].fingerprint(arm_id(call)), 'Collected science differs from qualified arm')
            log = Path(records_path).parent / (call['call_id'] + '.log')
            require(digest(log) == call['log_sha256'], 'Native clock/scientific log changed')
            clocks = re.findall(r'^NATIVE_SECONDS=([0-9.eE+-]+)$', log.read_text(errors='replace'), re.MULTILINE)
            require(len(clocks) == 1 and type(call['seconds']) in (int, float)
                    and math.isfinite(call['seconds']) and call['seconds'] > 0
                    and float(clocks[0]) == call['seconds'], 'Recorded time differs from native clock')
    context = monitor_context(manifest_path, records_path)
    monitor_error = None
    try:
        host = host_classification(value['spec'], context, monitor_receipt)
    except (ValueError, OSError, KeyError) as error:
        host, monitor_error = None, str(error)
    failures = [block['block_id'] for block in result['blocks'] if block['status'] != 'complete']
    warmups_ok = (len(result['warmups']) == len(value['schedule']['warmups']) and all(
        all(row.get(key) == planned[key] for key in planned) and row.get('status') == 'passed'
        and len(row['calls']) == len(planned_arms(planned)) and [arm_id(call) for call in row['calls']] == planned_arms(planned)
        and all(call.get('status') == 'passed' and call.get('science_pass') is True and call.get('certificate_pass') is True for call in row['calls'])
        for row, planned in zip(result['warmups'], value['schedule']['warmups'])))
    summaries = []
    for item, blocks, role, cell_id in comparison_views(value['spec'], result):
        relative = .20 if role == 'internal' else .05
        complete = [(block, block_scores(block, relative)) for block in blocks if block['status'] == 'complete']
        eligible = [(block, score) for block, score in complete if host is not None and host[block['block_id']]['status'] == 'eligible']
        ratios = [score['ratio'] for _, score in eligible]
        excess = [score['excess_seconds'] for _, score in eligible]
        interval = median_interval(excess) if excess else None
        valid = result['status'] == 'complete' and not failures and warmups_ok and host is not None
        conditional_pass = bool(valid and interval and interval['upper'] is not None and interval['upper'] <= 0)
        all_times = {side: [call['seconds'] for block, _ in complete for call in block['calls'] if call['side'] == side] for side in SIDES}
        oriented = {order: [score['ratio'] for block, score in complete if block['orientation'] == order] for order in ['ABBA', 'BAAB']}
        complete_ratios = [score['ratio'] for _, score in complete]
        complete_excess = [score['excess_seconds'] for _, score in complete]
        summaries.append({'comparison_id': item['id'], 'attempted_slots': len(blocks), 'eligible_blocks': len(eligible),
            'excluded_blocks': [block['block_id'] for block in blocks if host is not None and host[block['block_id']]['status'] == 'excluded'],
            'failed_or_unrun_blocks': [block['block_id'] for block in blocks if block['status'] != 'complete'],
            'conditional_nonregression_pass': conditional_pass, 'excess_median_interval': interval,
            'ratio_median_interval_descriptive': median_interval(ratios) if ratios else None,
            'all_complete_block_ratios_descriptive': complete_ratios,
            'order_diagnostics': {order: {'n': len(times), 'median_ratio': statistics.median(times) if times else None} for order, times in oriented.items()},
            'dependence_diagnostics': {'session_id': result['session_id'], 'analysis_unit': 'block',
                'lag1_log_ratio': lag_one([math.log(x) for x in complete_ratios]), 'lag1_excess': lag_one(complete_excess),
                'rounds': [block['round'] for block, _ in complete], 'uses_all_complete_blocks': True, 'not_an_independence_certificate': True},
            'all_complete_solve_times_descriptive': {side: {'minimum': min(times) if times else None,
                'mean': statistics.mean(times) if times else None, 'p95_nearest_rank': sorted(times)[math.ceil(.95 * len(times))-1] if times else None,
                'maximum': max(times) if times else None} for side, times in all_times.items()},
            'is_aa': item['reference']['plugin']['sha256'] == item['candidate']['plugin']['sha256'] and item['reference']['input']['sha256'] == item['candidate']['input']['sha256']})
        if role is not None:
            regrets = [score['normalized_regret'] for _, score in eligible]
            summaries[-1].update(cell_id=cell_id, reference_role=role,
                reference_arm=item['execution_ids']['reference'], candidate_arm=item['execution_ids']['candidate'],
                relative_allowance=relative, absolute_allowance_s=.001, normalized_regret_threshold=1,
                normalized_regret_median_interval=median_interval(regrets) if regrets else None,
                shared_block_ids=[block['block_id'] for block in blocks],
                shared_blocks_not_independent_comparisons=True,
                confidence_scope='Marginal comparison bound conditional on independent common-median blocks; no simultaneous or selected-winner coverage claim.')
    return {'schema': value['spec']['schema'], 'manifest': reference(manifest_path), 'records': reference(records_path),
            'monitor_receipt': reference(monitor_receipt) if monitor_receipt else None, 'summary': summaries,
            'conditional_statistical_pass': all(row['conditional_nonregression_pass'] for row in summaries),
            'qualification_pass': False, 'qualification_requires': ['Independent/common-median analysis-unit assumption review; these diagnostics do not prove it.'],
            'missing_monitor_evidence': host is None, 'monitor_error': monitor_error, 'scientific_or_execution_failure': bool(failures or not warmups_ok or result['status'] != 'complete'),
            'assumptions': ASSUMPTIONS, 'estimand': 'Median direct block excess; median block ratio is descriptive.',
            'scope': 'One frozen finite attempt schedule; all failures/ordinary OS noise retained, whole host exclusions consume slots, no replacement or old observations.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    seal = commands.add_parser('seal'); seal.add_argument('--spec', type=Path, required=True)
    collect_parser = commands.add_parser('collect'); collect_parser.add_argument('--manifest', type=Path, required=True)
    report = commands.add_parser('analyze'); report.add_argument('--manifest', type=Path, required=True)
    report.add_argument('--records', type=Path, required=True); report.add_argument('--monitor-receipt', type=Path)
    for command in [seal, collect_parser, report]: command.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'seal': freeze(read(args.spec), args.output)
    elif args.command == 'collect': collect(args.manifest, args.output)
    else: atomic_json(task_output(args.output), analyze(args.manifest, args.records, args.monitor_receipt), fresh=True)


if __name__ == '__main__': main()
