#!/usr/bin/env python3
"""Frozen complete-solve ABBA/BAAB blocks with retained failures and host noise.

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


def validate_spec(value):
    require(set(value) == {'schema', 'harness', 'design', 'comparisons', 'monitor', 'declarations'}, 'Unknown/missing block manifest fields')
    require(value['schema'] == 1, 'Unsupported block manifest schema')
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
    comparisons = value['comparisons']
    require(isinstance(comparisons, list) and 1 <= len(comparisons) <= 256, 'Require a finite 1..256 comparison list')
    require(len({item['id'] for item in comparisons}) == len(comparisons), 'Duplicate comparison')
    for item in comparisons:
        require(set(item) == {'id', 'workload', 'reference', 'candidate', 'validation'}, 'Unknown/missing comparison fields')
        require(all(isinstance(item[k], str) and re.fullmatch(r'[A-Za-z0-9_-]{1,100}', item[k]) for k in ['id', 'workload']), 'Unsafe comparison/workload identifier')
        for side in SIDES:
            require(set(item[side]) == {'label', 'plugin', 'input'} and isinstance(item[side]['label'], str) and item[side]['label'], 'Incomplete arm')
        require(item['reference']['label'] != item['candidate']['label'], 'A/A uses distinct labels even for identical binaries')
        validation = item['validation']
        require(validation.get('kind') in {'exact', 'routing'}, 'Unsupported scientific adapter')
        if validation['kind'] == 'exact':
            require(set(validation) == {'kind', 'poisson', 'qualification'} and type(validation['poisson']) is bool,
                    'Exact adapter requires model kind and bound prerequisite evidence')
        else:
            require(set(validation) == {'kind', 'manifest', 'witness', 'source', 'cell_id', 'stage', 'previous'}, 'Incomplete routing validation packet')
            require(validation['stage'] in {'A', 'B', 'C'}, 'Invalid prequalified routing stage')
    require(isinstance(value['declarations'], list) and value['declarations'], 'Bind prospective design/qualification declarations')
    monitor = value['monitor']
    require(monitor is None or set(monitor) == {'validator', 'policy', 'dependencies'}, 'Invalid external monitor contract')


def schedule(spec):
    """Exactly balanced orientations; shuffle workloads and comparisons each round."""
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


def source_bindings():
    names = ['time_blocks.py', 'time_exact.py', 'native_io.py', 'routing.py', 'routing_run.py',
             'routing_cases.py', 'routing_oracle.py', 'cmgschur.py', 'cmgschur_cases.py', 'native.py']
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
        for side in SIDES:
            with check(item[side]['input']).open('rb') as stream:
                shape, args = case_header(stream)
            require((args[0] == b'estimate-ppml') == self.poisson, 'Declared model differs from input')
            self.shapes[side] = shape
            matches = [Path(path).name for path, sha in qualified['cases'].items() if sha == item[side]['input']['sha256']]
            require(len(matches) == 1 and item[side]['plugin']['sha256'] in
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
        manifest_path = check(packet['manifest'])
        manifest = routing.unseal(read(manifest_path))
        routing.validate_manifest(manifest)
        previous = routing.unseal(read(check(packet['previous']))) if packet['previous'] else None
        candidates = routing.required_candidates(manifest, packet['stage'], previous)
        cell = next(c for c in manifest['panels'][packet['stage']] if c['id'] == packet['cell_id'])
        required = list(dict.fromkeys(['baseline', *manifest['protections'], *candidates, *cell['comparators']]))
        source = check(packet['source'])
        require(digest(source) == cell['case_sha256'], 'Routing source differs from frozen qualification')
        self.meta = inspect_case(source, cell['family'])
        require(self.meta['body_sha256'] == cell['body_sha256'], 'Routing numeric/model body changed')
        witness_path = check(packet['witness'])
        witness, dependencies = runner.evidence(witness_path, cell, manifest, required, stage=packet['stage'],
                                               selection_sha256=routing.identity(previous) if previous else None)
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
        proof = routing.unseal(read((witness_path.parent / witness['scoped_replay']['path']).resolve()))
        self.expected, self.reader = {}, {}
        for side in SIDES:
            arm = item[side]['label']
            require(arm in required and witness['arm_outcomes'][arm]['status'] == 'passed', 'Arm has no successful scientific prerequisite; do not retry unavailable arms')
            require(item[side]['plugin']['sha256'] == manifest['arms'][arm]['sha256'], 'Qualified artifact changed')
            origin = next(row for row in proof['prerequisites'] if row['arm'] == arm)
            require(item[side]['input']['sha256'] == origin['prepared_sha256'], 'Prepared routing request changed')
            self.expected[side] = origin['science_sha256']
            requested = runner.input_policy(manifest, arm)
            self.reader[side] = 'auto' if requested is not None else runner.EXPLICIT[manifest['arms'][arm].get('solver', arm)]
        self.memory_limit = cell.get('memory_limit_bytes')

    def fingerprint(self, side):
        return self.expected[side]

    def check(self, side, result, log):
        raw, detail = self.cmg.read_run(result, log, self.meta, self.reader[side])
        require(detail['science_sha256'] == self.expected[side], 'Call differs from independently qualified science/signature')
        self.runner.check_oracle(raw, result.with_suffix('.json.columns'), self.meta)
        return {'science_sha256': detail['science_sha256'], 'certificate_pass': True, 'science_pass': True}


def adapters(spec):
    return {item['id']: (ExactScience(item) if item['validation']['kind'] == 'exact' else RoutingScience(item))
            for item in spec['comparisons']}


def all_bindings(spec, science):
    values = [spec['harness'], *spec['declarations']]
    for item in spec['comparisons']:
        values += [item[side][field] for side in SIDES for field in ['plugin', 'input']]
        values += science[item['id']].bindings
        packet = item['validation']
        if packet['kind'] == 'routing':
            values += [packet[key] for key in ['manifest', 'witness', 'source']]
            if packet['previous']:
                values.append(packet['previous'])
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
    value = {'schema': 1, 'spec': spec, 'schedule': schedule(spec), 'bindings': all_bindings(spec, science),
             'clock': CLOCK, 'assumptions': ASSUMPTIONS}
    atomic_json(task_output(output), {'sha256': identity(value), 'value': value}, fresh=True)
    return value


def load_manifest(path):
    envelope = read(path)
    value = envelope['value']
    require(envelope['sha256'] == identity(value), 'Manifest seal changed')
    validate_spec(value['spec'])
    require(value['schedule'] == schedule(value['spec']) and value['clock'] == CLOCK and value['assumptions'] == ASSUMPTIONS,
            'Frozen schedule or estimand changed')
    for bound in value['bindings']:
        check(bound)
    science = adapters(value['spec'])
    require(value['bindings'] == all_bindings(value['spec'], science), 'Undeclared or changed scientific/tool dependency')
    return value, science


def emit(events, result, event, block=None, call=None, **extra):
    row = {'schema': 1, 'event': event, 'run_id': result['run_id'], 'block_id': block['block_id'] if block else None,
           'comparison_id': block['comparison_id'] if block else call.get('comparison_id') if call else None,
           'round': block['round'] if block else None, 'session_id': result['session_id'],
           'wall_time_ns': time.time_ns(), 'monotonic_ns': time.monotonic_ns(), 'tool_pid': os.getpid(),
           'native_pid': call.get('native_pid') if call else None, 'call_id': call['call_id'] if call else None,
           'process_token': call.get('process_token') if call else None, **extra}
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
    comparisons = {item['id']: item for item in spec['comparisons']}
    result = {'schema': 1, 'status': 'running', 'run_id': str(uuid.uuid4()), 'session_id': design['session_id'],
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
            call = {'call_id': f'c{serial:07d}', 'comparison_id': cid, 'side': side, 'status': 'running'}
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
                command += [spec['harness']['path'], item[side]['plugin']['path'], '--replay', item[side]['input']['path'], str(target)]
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
                emit(events, result, 'warmup_start', warmup_id=warmup_index, comparison_id=warmup['comparison_id'])
                try:
                    for side in warmup['sides']:
                        one(warmup['comparison_id'], side, None, row['calls'])
                    row['status'] = 'passed'
                finally:
                    if row['status'] != 'passed': row['status'] = 'failed'
                    emit(events, result, 'warmup_end', status=row['status'], warmup_id=warmup_index, comparison_id=warmup['comparison_id'])
                    save()
            for block in result['blocks']:
                block['status'] = 'running'
                block['start_monotonic_ns'] = emit(events, result, 'block_start', block)['monotonic_ns']
                save()
                try:
                    for side in block['sides']:
                        one(block['comparison_id'], side, block, block['calls'])
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


def block_scores(block):
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
    ratio, excess = n / b, n - b - max(.05 * b, .001)
    require(math.isfinite(ratio) and math.isfinite(excess), 'Nonfinite block score')
    return {**times, 'ratio': ratio, 'excess_seconds': excess}


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
    require(result['manifest'] == reference(manifest_path) and result['schedule'] == value['schedule']
            and result['session_id'] == value['spec']['design']['session_id'], 'Collection does not bind frozen design')
    require(len(result['blocks']) == len(value['schedule']['blocks']) and all(
        all(actual.get(key) == expected[key] for key in expected) for actual, expected in zip(result['blocks'], value['schedule']['blocks'])), 'Missing/reordered attempted block slots')
    all_calls = [call for row in [*result['warmups'], *result['blocks']] for call in row['calls']]
    require(len({call['call_id'] for call in all_calls}) == len(all_calls), 'Duplicated native call identity')
    require(all(call.get('comparison_id') == row['comparison_id'] and call.get('side') in SIDES
                for row in [*result['warmups'], *result['blocks']] for call in row['calls']), 'Misassigned native call')
    for call in all_calls:
        require(re.fullmatch(r'c[0-9]{7}', call['call_id']), 'Invalid native call identity')
        if call.get('status') == 'passed':
            require(call['science_sha256'] == science[call['comparison_id']].fingerprint(call['side']), 'Collected science differs from qualified arm')
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
        and len(row['calls']) == 2 and [call['side'] for call in row['calls']] == planned['sides']
        and all(call.get('status') == 'passed' and call.get('science_pass') is True and call.get('certificate_pass') is True for call in row['calls'])
        for row, planned in zip(result['warmups'], value['schedule']['warmups'])))
    summaries = []
    for item in value['spec']['comparisons']:
        blocks = [block for block in result['blocks'] if block['comparison_id'] == item['id']]
        complete = [(block, block_scores(block)) for block in blocks if block['status'] == 'complete']
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
    return {'schema': 1, 'manifest': reference(manifest_path), 'records': reference(records_path),
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
