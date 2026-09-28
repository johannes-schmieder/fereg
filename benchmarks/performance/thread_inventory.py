"""Read-only thread supplement to a frozen systematic report.

The original report remains the authority for pair eligibility. This supplement
exposes selected native teams and labels cross-case clocks as observational;
it cannot establish a causal core-scaling claim or rescue a scientific NR.
"""
import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import io
import json
import math
from pathlib import Path
import statistics
import sys

SYSTEMATIC = Path(__file__).resolve().parents[1] / 'systematic'
sys.path.insert(0, str(SYSTEMATIC))
from common import cell_id
from light import groups


def positive(value):
    return type(value) in (int, float) and math.isfinite(value) and value > 0


def thread_fields(receipt, requested):
    """Keep missing/invalid metadata visible; never infer a team from a request."""
    prefix = 'fereg' if receipt['cell']['model'] == 'ols' else 'feppml'
    metadata = receipt.get('metadata', {})
    selected = metadata.get(prefix + '_threads')
    recorded_request = metadata.get(prefix + '_requested_threads')
    valid = (positive(selected) and selected == int(selected) and selected <= requested
             and type(recorded_request) in (int, float) and recorded_request == requested)
    return dict(requested_threads=requested, recorded_request=recorded_request,
                selected_threads=int(selected) if valid else None, thread_metadata_valid=valid,
                stata_processors=metadata.get('stata_processors'))


def distribution(values):
    if not values:
        return None
    quartiles = statistics.quantiles(values, n=4, method='inclusive') if len(values) > 1 else values * 3
    return dict(n=len(values), values=values, minimum=min(values), q1=quartiles[0],
                median=statistics.median(values), q3=quartiles[2], maximum=max(values))


def scaling_inventory(applications, repetitions):
    repetitions = tuple(repetitions)
    if not repetitions or len(set(repetitions)) != len(repetitions):
        raise ValueError('empty or duplicate registered repetitions')
    families = defaultdict(lambda: defaultdict(dict))
    for row in applications:
        if row['phase'] != 'warm':
            continue
        if row['repetition'] not in repetitions:
            raise ValueError('unregistered native repetition')
        cell = row['cell']
        family = json.dumps({k: v for k, v in cell.items() if k != 'threads'}, sort_keys=True)
        target = families[family][cell['threads']]
        if row['repetition'] in target:
            raise ValueError('duplicate native repetition')
        target[row['repetition']] = row
    result = []
    for family, settings in sorted(families.items()):
        requests = sorted(settings)
        for low, high in zip(requests, requests[1:]):
            pairs = []
            for rep in repetitions:
                a, b = settings[low].get(rep), settings[high].get(rep)
                reasons = []
                for label, row in [('lower', a), ('higher', b)]:
                    if row is None:
                        reasons.append(label + ':missing')
                        continue
                    if row['status'] != 'pass':
                        reasons.append(label + ':' + row['status'])
                    if not row['eligible_references']:
                        reasons.append(label + ':no_scientifically_eligible_reference')
                    if not row.get('thread_metadata_valid'):
                        reasons.append(label + ':unknown_native_team')
                    if not positive(row.get('seconds')):
                        reasons.append(label + ':invalid_command_time')
                    if row.get('converged') != 1:
                        reasons.append(label + ':native_not_converged')
                if a and b:
                    for key in ('input_sha256', 'sample_sha256'):
                        if not a.get(key) or a.get(key) != b.get(key):
                            reasons.append('different_or_missing_' + key)
                ratio = b['seconds'] / a['seconds'] if not reasons else None
                selected = [r.get('selected_threads') if r else None for r in (a, b)]
                transition = ('unknown' if None in selected else
                              'more_native_threads' if selected[1] > selected[0] else
                              'same_native_team' if selected[1] == selected[0] else
                              'fewer_native_threads')
                location = [dict(host=r.get('host'), job_id=r.get('job_id'), task_id=r.get('task_id'))
                            if r else None for r in (a, b)]
                same_node_task = (all(r and all(v is not None for v in r.values()) for r in location)
                                  and location[0] == location[1])
                pairs.append(dict(repetition=rep, selected_threads=selected, transition=transition,
                                  eligible_observation=not reasons, reasons=reasons,
                                  higher_over_lower=ratio, locations=location,
                                  same_node_task=bool(same_node_task),
                                  stata_processors=[r.get('stata_processors') if r else None for r in (a, b)]))
            complete = bool(pairs) and all(p['eligible_observation'] for p in pairs)
            ratios = [p['higher_over_lower'] for p in pairs] if complete else []
            result.append(dict(cell_without_threads=json.loads(family), requested_threads=[low, high],
                pairs=pairs, all_repetitions_eligible=complete,
                transitions=dict(Counter(p['transition'] for p in pairs)),
                observational_higher_over_lower=distribution(ratios),
                slower_on_every_repetition=min(ratios) > 1 if ratios else None,
                alternating_confirmation_required=bool(ratios and max(ratios) > 1),
                causal_scaling_established=False))
    return result


def collect(run_root, report, *, partial=False):
    bindings = {}

    def load(path, label, *, rows=False):
        data = path.read_bytes()
        bindings[label] = hashlib.sha256(data).hexdigest()
        return list(csv.DictReader(io.StringIO(data.decode()))) if rows else json.loads(data)

    manifest = load(report / 'manifest.json', 'report/manifest.json')
    frozen = load(report / 'provenance.json', 'report/provenance.json')
    if frozen != load(run_root / 'env/frozen.json', 'run/env/frozen.json'):
        raise ValueError('report/runtime identity mismatch')
    canonical = json.dumps({k: v for k, v in frozen.items() if k != 'fingerprint'}, sort_keys=True, separators=(',', ':'))
    if hashlib.sha256(canonical.encode()).hexdigest() != frozen['fingerprint']:
        raise ValueError('invalid frozen fingerprint')
    if manifest != load(run_root / 'input/manifest.json', 'run/input/manifest.json'):
        raise ValueError('report/run manifest mismatch')
    for name in ('common.py', 'light.py'):
        if hashlib.sha256((SYSTEMATIC / name).read_bytes()).hexdigest() != frozen['controller'][name]:
            raise ValueError('registered enumeration changed: ' + name)
    completeness = load(report / 'completeness.json', 'report/completeness.json')
    if completeness['status'] != 'complete' and not partial:
        raise ValueError('incomplete frozen report; use --partial for metadata only')
    registered = groups(manifest)
    if (completeness['expected_groups'] != len(registered)
            or (completeness['status'] == 'complete' and
                (completeness['complete_groups'] != len(registered) or completeness['integrity_errors']))):
        raise ValueError('inconsistent report completeness')
    expected = {(cell_id(g['cell']), g['repetition'], arm)
                for g in registered for arm in manifest['arms'][g['cell']['model']][1:]}
    pairs = load(report / 'paired-ratios.csv', 'report/paired-ratios.csv', rows=True)
    eligible = defaultdict(list)
    seen = set()
    for pair in pairs:
        key = (cell_id(pair), int(pair['repetition']), pair['comparator'])
        if key in seen or key not in expected:
            raise ValueError('duplicate or unregistered reported comparison')
        seen.add(key)
        phase = 'cold' if key[1] == 0 else 'warm'
        if pair['phase'] != phase or pair['eligible'] not in ('True', 'False'):
            raise ValueError('invalid phase or eligibility encoding')
        if pair['eligible'] == 'True':
            if pair['status'] != 'pass' or pair['failed_checks']:
                raise ValueError('eligible report row has failed science')
            eligible[key[:2]].append(pair['comparator'])
    if seen != expected:
        raise ValueError('report does not enumerate every registered comparison')
    applications = []
    for group in registered:
        cell, rep = group['cell'], group['repetition']
        phase = 'cold' if rep == 0 else 'warm'
        arm = 'fereg' if cell['model'] == 'ols' else 'feppml'
        relative = Path('results') / cell_id(cell) / f'{phase}-{rep}' / arm / 'result.json'
        receipt = load(run_root / relative, 'run/' + str(relative)) if (run_root / relative).exists() else None
        row = dict(cell=cell, phase=phase, repetition=rep, status='missing',
                   eligible_references=sorted(eligible[cell_id(cell), rep]), requested_threads=cell['threads'])
        if receipt:
            if (receipt['cell'] != cell or receipt['arm'] != arm or receipt['phase'] != phase
                    or receipt['measurement_fingerprint'] != frozen['fingerprint']):
                raise ValueError('native receipt identity mismatch: ' + str(relative))
            row.update({k: receipt.get(k) for k in ('status', 'seconds', 'elapsed_seconds', 'peak_rss_gib',
                       'input_sha256', 'sample_sha256', 'host', 'job_id', 'task_id')})
            row.update(thread_fields(receipt, cell['threads']))
            row['converged'] = receipt.get('metadata', {}).get('converged')
        if row['eligible_references'] and (row['status'] != 'pass' or not positive(row.get('seconds'))):
            raise ValueError('eligible pair has missing/failed native command')
        applications.append(row)
    scaling = scaling_inventory(applications, range(1, manifest['warm_repetitions'] + 1))
    return dict(schema='fereg-systematic-thread-inventory-v1',
                source_fingerprint=frozen['fingerprint'], report_complete=completeness['status'] == 'complete',
                applications=applications, scaling=scaling, bindings=bindings,
                interpretation='Pair eligibility is inherited unchanged from the frozen report. Cross-case clocks are observational even on the same node. Same selected teams measure request overhead, not added native cores. Confirm suspected slowdowns with alternating same-node fits before changing routing. This supplement establishes no causal scaling or qualification claim.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('run_root', type=Path)
    parser.add_argument('frozen_report', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--partial', action='store_true')
    args = parser.parse_args()
    result = collect(args.run_root, args.frozen_report, partial=args.partial)
    result['analyzer_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as stream:
        json.dump(result, stream, indent=2, allow_nan=False)
        stream.write('\n')
    print(json.dumps(dict(native_applications=len(result['applications']), scaling_transitions=len(result['scaling']),
                         metadata_failures=sum(r['status'] == 'pass' and not r.get('thread_metadata_valid') for r in result['applications']))))
