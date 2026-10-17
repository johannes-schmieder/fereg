#!/usr/bin/env python3
"""Execute the already frozen membership C1 contract after verified D1 admission.

Freeze this driver while D1 outcomes remain unopened. The portable freeze
receipt records that operator attestation; it cannot detect a human reading
results. Admission separately checks the original C1 freeze chronology, the
recomputed D1 gate, both qacct records, and an externally supplied isolation
review. No command changes the C1 preparation plan or its scientific scope.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import math
import os
from pathlib import Path
import re
import socket
import statistics
import time

import membership_confirmation as m
import occurrence_confirmation as occurrence
import admission_confirmation as admission_profile

c = m.campaign
KIND = m.KIND + '_execution_v1'
ROOT = Path(__file__).resolve().parents[1]
SOURCES = ['bench/membership_confirmation_run.py', 'bench/tests/test_membership_confirmation_run.py']
ISOLATION_FILES = ['affinity', 'job_environment', 'parallel_environment', 'processes',
                   'queue_configuration', 'resource_quota', 'scheduler']


def confirmation_module(plan):
    return {occurrence.KIND:occurrence, admission_profile.KIND:admission_profile}.get(plan.get('kind'), m)


def execution_kind(plan):
    return confirmation_module(plan).KIND + '_execution_v1'


def unsealed(path):
    return c.routing.unseal(c.read(path))


def source_hashes(module=m):
    sources = [*SOURCES]
    if module in (occurrence, admission_profile):
        sources.append('bench/tests/test_occurrence_confirmation_run.py')
    if module is admission_profile:
        sources.extend(['bench/tests/test_admission_confirmation.py', 'bench/tests/test_admission_cells.py'])
    return {name: c.digest(ROOT / name) for name in sources}


def freeze(plan_sha256, output, *, discovery_unopened, profile="membership"):
    module = {"membership": m, "occurrence": occurrence, "admission":admission_profile}[profile]
    c.require(discovery_unopened is True and re.fullmatch('[0-9a-f]{64}', plan_sha256) is not None,
              'freeze requires the C1 plan identity and unopened D1 outcomes attestation')
    value = {'kind': module.KIND + '_execution_v1_driver', 'plan_sha256': plan_sha256, 'sources': source_hashes(module),
             'preparation_source_sha256': c.digest(Path(module.__file__)),
             'frozen_wall_ns': time.time_ns(), 'created_utc': datetime.now(timezone.utc).isoformat(),
             'discovery_outcomes_unopened': True}
    c.save(output, c.routing.seal(value))
    return value


def validate_driver(plan, driver):
    m = confirmation_module(plan)
    c.require(driver.get('kind') == execution_kind(plan) + '_driver'
              and driver.get('plan_sha256') == c.routing.identity(plan)
              and driver.get('sources') == source_hashes(m)
              and driver.get('preparation_source_sha256') == c.digest(Path(m.__file__))
              and driver.get('discovery_outcomes_unopened') is True
              and type(driver.get('frozen_wall_ns')) is int and driver['frozen_wall_ns'] > 0,
              'driver freeze/source/plan binding changed')


def native_rows(receipt):
    rows = list(receipt['records'])
    for pair in receipt.get('parent_controls', []):
        rows.extend(pair[role] for role in ['parent', 'candidate'])
    c.require(rows, 'missing native chronology')
    return rows


def wall_window(receipt):
    spans = []
    for row in native_rows(receipt):
        fields = [row.get(key) for key in ['started_wall_ns', 'started_monotonic_ns', 'completed_monotonic_ns']]
        c.require(all(type(value) is int and value > 0 for value in fields) and fields[2] >= fields[1],
                  'incomplete native chronology')
        spans.append((fields[0], fields[0] + fields[2] - fields[1]))
    return min(start for start, _ in spans), max(end for _, end in spans)


def accounting(raw, receipt, *, science=False):
    host = ({'hostname': receipt['host'], 'job_id': receipt['job_id'], 'slots': receipt['slots']}
            if science else receipt['host'])
    slots, pe = (receipt.get('native_affinity_cores', 4), 'omp') if science else (28, 'omp28')
    values = c.scc.fields(c.scc.output(raw, ['qacct', '-j', host['job_id']]))
    c.require(values['jobnumber'] == host['job_id'] and values['project'] == 'welfgr'
              and c.scc.hostname(values['hostname']) == c.scc.hostname(host['hostname'])
              and host['slots'] == slots and values['slots'] == str(slots) and values['granted_pe'] == pe,
              'wrong accounted D1 allocation')
    def instant(value):
        return datetime.strptime(value, '%a %b %d %H:%M:%S %Y').replace(tzinfo=timezone.utc).timestamp() * 1e9
    start, end = wall_window(receipt)
    c.require(instant(values['start_time']) <= start and instant(values['end_time']) + 1e9 >= end,
              'D1 accounting does not cover all native calls')
    facts = {name: int(values[name].split()[0]) for name in ['failed', 'exit_status']}
    c.require(facts == {'failed': 0, 'exit_status': 0}, 'D1 scheduler job did not complete successfully')
    return facts


def isolation_review(discovery, measurement, review, accounting_ref):
    c.require(review.get('kind') == discovery['kind'] + '_isolation_review'
              and review.get('status') == 'passed'
              and review.get('plan_sha256') == c.routing.identity(discovery)
              and review.get('measurement_sha256') == c.routing.identity(measurement)
              and review.get('host') == measurement['host']
              and review.get('accounting') == accounting_ref
              and isinstance(review.get('reviewer'), str) and review['reviewer'].strip()
              and review.get('observation_complete') is True
              and review.get('observed_compute_overlap') == []
              and review.get('observed_host_faults') == [] and review.get('exclusions') == [],
              'complete external D1 isolation review required')
    evidence = review.get('raw_evidence', {})
    c.require(set(evidence) == set(ISOLATION_FILES), 'incomplete D1 isolation review evidence')
    for name, item in evidence.items():
        c.require(Path(item['path']).name == name + '.jsonl' and item in measurement['files'],
                  'isolation review does not bind the retained D1 observations')
        c.require(c.checked(item).stat().st_size > 0, 'empty D1 isolation evidence')


def admission(plan, driver, evidence):
    """Recompute all prerequisites; a saved eligible label alone never authorizes C1."""
    m = confirmation_module(plan)
    m.validate(plan)
    validate_driver(plan, driver)
    names = {'smoke', 'science', 'measurement', 'science_accounting', 'measurement_accounting', 'isolation_review'}
    c.require(set(evidence) == names, 'incomplete D1 admission evidence')
    paths = {name: c.checked(item) for name, item in evidence.items()}
    discovery = unsealed(c.checked(plan['discovery_plan']))
    smoke, science, measurement = [unsealed(paths[name]) for name in ['smoke', 'science', 'measurement']]
    c.require(smoke.get('kind') == discovery['kind'] + '_science' and smoke.get('status') == 'passed'
              and smoke.get('smoke') is True and smoke.get('plan_sha256') == c.routing.identity(discovery)
              and all(smoke.get(key) == science[key] for key in ['job_id', 'host', 'slots']),
              'complete smoke receipt from the D1 scientific prerequisites job required')
    check_record_files(native_rows(smoke), smoke['files'])
    gate = (c.admission_gate if m is admission_profile else c.occurrence_gate if m in (occurrence, admission_profile) else c.membership_gate)(discovery, science, measurement)
    c.require(gate['status'] == 'eligible' and gate['finalist'] == m.ARMS[1], 'D1 gate blocks C1 candidate calls')
    earliest = min(wall_window(receipt)[0] for receipt in [smoke, science, measurement])
    c.require(plan['prepared_wall_ns'] < earliest, 'C1 was not frozen before the first D1 native call')
    if m in (occurrence, admission_profile):
        c.require(driver['frozen_wall_ns'] < earliest, 'C2 driver was not frozen before the first D2 native call')
    for name, receipt, is_science in [('science_accounting', science, True),
                                      ('measurement_accounting', measurement, False)]:
        accounting(c.read(paths[name]), receipt, science=is_science)
    accounting(c.read(paths['science_accounting']), smoke, science=True)
    isolation_review(discovery, measurement, unsealed(paths['isolation_review']), evidence['measurement_accounting'])
    return {'kind': execution_kind(plan) + '_admission', 'plan_sha256': c.routing.identity(plan),
            'driver_sha256': c.routing.identity(driver), 'gate_sha256': c.routing.identity(gate),
            'evidence': evidence, 'status': 'admitted', 'earliest_d1_native_wall_ns': earliest,
            **({'discovery_host': measurement['host']['hostname']} if m in (occurrence, admission_profile) else {})}


def validate_admission(plan, driver, admitted):
    c.require(admitted == admission(plan, driver, admitted['evidence']), 'C1 admission receipt changed')


def check_library_limits():
    c.require({key: os.environ.get(key) for key in c.LIBRARY_LIMITS} == c.LIBRARY_LIMITS,
              'set frozen library thread limits before executing C1')


def bound_files(output):
    return [c.ref(path) for path in sorted(output.iterdir()) if path.is_file()]


def check_record_files(records, files):
    by_path = {item['path']: item for item in files}
    c.require(len(by_path) == len(files), 'duplicate C1 file binding')
    for item in files:
        c.checked(item)
    for row in records:
        for path in [Path(row['target']), Path(row['log']), Path(row['target']).with_suffix('.json.columns')]:
            c.require(str(path.resolve()) in by_path, 'unbound C1 native output')


def science_order(plan):
    m = confirmation_module(plan)
    return [(cell['id'], arm, repeat) for cell in plan['cells'] for arm in m.ARMS
            for repeat in range(m.LIMITS['science_repeats'])]


def check_science_records(plan, receipt):
    m = confirmation_module(plan)
    rows = receipt['records']
    expected = science_order(plan)
    c.require(len(rows) == m.EXECUTION['scientific_calls']
              and [(row['cell'], row['arm'], row.get('repeat')) for row in rows] == expected,
              'incomplete C1 scientific calls or changed order')
    fingerprints, discrete = {}, {}
    families = {cell['id']: cell['family'] for cell in plan['cells']}
    for row in rows:
        key = row['cell'] + '/' + row['arm']
        c.require(row['label'] == f"science-{row['cell']}-{row['arm']}-{row['repeat']}"
                  and row['status'] == 'executed' and row.get('oracle_status') == 'passed'
                  and re.fullmatch('[0-9a-f]{64}', row.get('science_sha256', '')) is not None
                  and re.fullmatch('[0-9a-f]{64}', row.get('model_discrete_sha256', '')) is not None
                  and (row['repeat'] != 0 or 'resources' in row), 'incomplete C1 science evidence')
        c.require(fingerprints.setdefault(key, row['science_sha256']) == row['science_sha256'],
                  'C1 fixed-worker repeat differs')
        c.require(discrete.setdefault(families[row['cell']], row['model_discrete_sha256'])
                  == row['model_discrete_sha256'], 'C1 cross-arm/worker model-discrete decisions differ')
    c.require(receipt['fingerprints'] == fingerprints, 'C1 scientific fingerprint coverage changed')


def validate_science(plan, driver, admitted, receipt):
    c.require(receipt.get('kind') == execution_kind(plan) + '_science' and receipt.get('status') == 'passed'
              and receipt.get('plan_sha256') == c.routing.identity(plan)
              and receipt.get('driver_sha256') == c.routing.identity(driver)
              and receipt.get('admission_sha256') == c.routing.identity(admitted),
              'complete bound C1 science prerequisite required')
    check_science_records(plan, receipt)
    check_record_files(receipt['records'], receipt['files'])


def science(plan, driver, admitted, output):
    m = confirmation_module(plan)
    validate_admission(plan, driver, admitted)
    check_library_limits()
    slots = int(os.environ.get('NSLOTS', '0'))
    cores = plan.get('native_affinity_cores', 4)
    c.require(os.environ.get('JOB_ID') and slots == cores, 'confirmation science requires its frozen compute allocation')
    cpus = c.science_affinity({'kind': c.OCCURRENCE_KIND, 'native_affinity_cores': cores}) if m in (occurrence, admission_profile) else None
    output.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    rows, fingerprints, first_worker, baselines = [], {}, {}, {}
    failure, active = None, None
    try:
        for cell in plan['cells']:
            meta = c.read(cell['metadata']['path'])
            witness, _ = c.routing_oracle.load_reference(Path(cell['witness']['path']),
                                                        meta['case_sha256'], meta['body_sha256'])
            for arm in m.ARMS:
                for repeat in range(m.LIMITS['science_repeats']):
                    left = m.LIMITS['science_budget_s'] - (time.monotonic() - started)
                    c.require(left > 0, 'C1 science budget exhausted')
                    active = c.native(plan, cell, arm, output, f"science-{cell['id']}-{arm}-{repeat}",
                                      traced=repeat == 0, cpus=cpus, timeout=min(m.LIMITS['call_timeout_s'], left))
                    active.update(phase='science', repeat=repeat)
                    rows.append(active)
                    raw, result, binary = c.parse_run(active, meta)
                    c.routing_oracle.check_reference(witness, raw, binary, meta)
                    key = cell['id'] + '/' + arm
                    c.require(fingerprints.setdefault(key, result['science_sha256']) == result['science_sha256'],
                              'C1 fixed-worker repeat differs: ' + key)
                    active.update(oracle_status='passed', science_sha256=result['science_sha256'],
                                  model_discrete_sha256=c.routing_run.model_discrete_sha256(raw))
                    if repeat == 0:
                        active['resources'] = c.routing_run.resource_diagnostics(Path(active['log']).read_text(),
                                                                                arm, False, {})
                        if arm == 'baseline':
                            baselines[cell['id']] = (raw, binary)
                        base, columns = baselines[cell['id']]
                        c.cmgschur.scientific_comparison(base, raw, columns, binary, meta)
                        family = cell['family'] + '/' + arm
                        if cell['threads'] == 1:
                            first_worker[family] = (raw, binary)
                        else:
                            base, columns = first_worker[family]
                            c.cmgschur.scientific_comparison(base, raw, columns, binary, meta)
        check_science_records(plan, {'records': rows, 'fingerprints': fingerprints})
        validate_admission(plan, driver, admitted)
        c.require(time.monotonic() - started <= m.LIMITS['science_budget_s'], 'C1 science budget exhausted')
    except Exception as error:
        failure = str(error)
        if active is not None and active['status'] == 'executed':
            active.update(status='failed', error=failure)
    value = {'kind': execution_kind(plan) + '_science', 'status': 'failed' if failure else 'passed', 'failure': failure,
             'plan_sha256': c.routing.identity(plan), 'driver_sha256': c.routing.identity(driver),
             'admission_sha256': c.routing.identity(admitted), 'host': socket.gethostname(),
             'job_id': os.environ['JOB_ID'], 'slots': slots, 'native_affinity_cores': cores, 'records': rows, 'fingerprints': fingerprints,
             'files': bound_files(output), 'seconds': time.monotonic() - started}
    c.save(output / 'science.json', c.routing.seal(value))
    return value


def measurement_order(plan):
    m = confirmation_module(plan)
    warmups = [(f"warmup-{cell['id']}-{arm}", cell['id'], arm, 'warmup', None, None)
               for cell in plan['cells'] for arm in m.ARMS]
    scored = [(f"block-{block}-{slot['cell']}-p{position}-{arm}", slot['cell'], arm, 'measure', block, position)
              for block, slots in enumerate(plan['blocks']) for slot in slots
              for position, arm in enumerate(slot['arms'])]
    return warmups + scored


def row_identity(row):
    return row['label'], row['cell'], row['arm'], row['phase'], row.get('block'), row.get('position')


def block_score(block, cell, rows, arms=m.ARMS):
    times = {arm: [row['seconds'] for row in rows if row['arm'] == arm] for arm in arms}
    c.require(all(len(values) == 2 and all(math.isfinite(value) and value > 0 for value in values)
                  for values in times.values()), 'invalid C1 complete-solve block times')
    baseline, candidate = [math.exp(sum(math.log(value) for value in times[arm]) / 2) for arm in arms]
    return {'block': block, 'cell': cell, 'baseline_seconds': baseline, 'candidate_seconds': candidate,
            'excess_seconds': candidate - baseline - max(.05 * baseline, .001), 'ratio': candidate / baseline}


def summarize(plan, rows):
    """Only complete, valid four-call units contribute; missing units cannot pass."""
    m = confirmation_module(plan)
    expected = measurement_order(plan)
    c.require(len(rows) <= len(expected) and [row_identity(row) for row in rows] == expected[:len(rows)],
              'C1 measurement order/position changed')
    groups = {}
    for row in rows:
        if row['phase'] == 'measure':
            groups.setdefault((row['block'], row['cell']), []).append(row)
    scores = [block_score(block, cid, group, m.ARMS) for (block, cid), group in groups.items()
              if len(group) == 4 and all(row['status'] == 'passed' for row in group)]
    summaries = []
    for cell in plan['cells']:
        values = [score for score in scores if score['cell'] == cell['id']]
        upper = sorted(score['excess_seconds'] for score in values)[9] if len(values) == 12 else None
        summaries.append({'cell': cell['id'], 'complete_blocks': len(values),
                          'upper_median_excess_seconds': upper,
                          'median_ratio': statistics.median(score['ratio'] for score in values) if values else None,
                          'passed': upper is not None and upper <= 0})
    complete = len(rows) == len(expected) and all(row['status'] == 'passed' for row in rows)
    return {'status': ('passed' if all(row['passed'] for row in summaries) else 'failed') if complete else 'incomplete',
            'analysis': m.ANALYSIS, 'block_scores': scores, 'settings': summaries}


def validate_measurement_records(plan, rows, fingerprints):
    m = confirmation_module(plan)
    c.require(len(rows) == m.EXECUTION['warmup_calls'] + m.EXECUTION['scored_calls']
              and [row_identity(row) for row in rows] == measurement_order(plan),
              'incomplete C1 measurement calls or changed order')
    c.require(set(fingerprints) == {cell['id'] + '/' + arm for cell in plan['cells'] for arm in m.ARMS},
              'incomplete C1 same-allocation fingerprints')
    for row in rows:
        c.require(row['status'] == 'passed' and math.isfinite(row['seconds']) and row['seconds'] > 0
                  and row['science_sha256'] == fingerprints[row['cell'] + '/' + row['arm']]
                  and 'resources' in row, 'C1 same-allocation science/time/resources differ')


def measure(plan, driver, admitted, prerequisite, output):
    m = confirmation_module(plan)
    validate_admission(plan, driver, admitted)
    validate_science(plan, driver, admitted, prerequisite)
    check_library_limits()
    host = c.scc.discovery.host()
    c.require(host['slots'] == 28, 'C1 measurement requires its 28-slot isolated allocation')
    if m in (occurrence, admission_profile):
        c.require(c.scc.hostname(host['hostname']) != c.scc.hostname(admitted['discovery_host']),
                  'C2 requires a distinct physical host from D2')
    output.mkdir(parents=True, exist_ok=False)
    records, fingerprints, resources = [], {}, {}
    failure, observer, before, queue, started, ended = None, None, None, None, None, None
    observed_started = None
    try:
        before, snapshot = c.scc.configuration(output, host, 'before')
        queue = c.scc.check_configuration(before, host, os.getpid())
        c.scc.check_scheduler_snapshot(snapshot, host, queue)
        cpus = sorted(host['affinity'])[:plan.get('native_affinity_cores', 4)]
        observer = c.Observers(output, host, queue)
        observer.start()
        observed_started = time.monotonic_ns()
        warmup_started = time.monotonic()
        for cell in plan['cells']:
            for arm in m.ARMS:
                c.require(not observer.errors, 'C1 isolation observer failed')
                left = m.LIMITS['warmup_budget_s'] - (time.monotonic() - warmup_started)
                c.require(left > 0, 'C1 warmup budget exhausted')
                row = c.native(plan, cell, arm, output, f"warmup-{cell['id']}-{arm}", cpus=cpus,
                               timeout=min(m.LIMITS['call_timeout_s'], left))
                row.update(phase='warmup')
                records.append(row)
                result = c.check_prerequisite(row, cell, prerequisite, traced=False)
                key = cell['id'] + '/' + arm
                fingerprints[key], resources[key] = result['science_sha256'], row['resources']
                row.update(result, status='passed')
        c.require(time.monotonic() - warmup_started <= m.LIMITS['warmup_budget_s'], 'C1 warmup budget exhausted')
        cells = {cell['id']: cell for cell in plan['cells']}
        started = time.monotonic_ns()
        for block, slots in enumerate(plan['blocks']):
            observer.census(f'block-{block}-before')
            for slot in slots:
                complete = []
                cell = cells[slot['cell']]
                for position, arm in enumerate(slot['arms']):
                    c.require(not observer.errors, 'C1 isolation observer failed')
                    left = m.LIMITS['measurement_budget_s'] - (time.monotonic_ns() - started) / 1e9
                    c.require(left > 0, 'C1 measurement budget exhausted')
                    row = c.native(plan, cell, arm, output, f"block-{block}-{cell['id']}-p{position}-{arm}",
                                   cpus=cpus, timeout=min(m.LIMITS['call_timeout_s'], left))
                    row.update(phase='measure', block=block, position=position,
                               resources=resources[cell['id'] + '/' + arm])
                    records.append(row)
                    _, result, _ = c.parse_run(row, c.read(cell['metadata']['path']))
                    c.require(result['science_sha256'] == fingerprints[cell['id'] + '/' + arm],
                              'C1 measured result changed same-allocation fixed-worker science')
                    row.update(result, status='passed')
                    complete.append(row)
                c.scc.append(output / 'blocks.jsonl', block_score(block, cell['id'], complete, m.ARMS))
            observer.census(f'block-{block}-after')
        ended = time.monotonic_ns()
        c.require((ended - started) / 1e9 <= m.LIMITS['measurement_budget_s'], 'C1 measurement budget exhausted')
        validate_measurement_records(plan, records, fingerprints)
    except Exception as error:
        failure = str(error)
        if records and records[-1]['status'] == 'executed':
            records[-1].update(status='failed', error=failure)
    finally:
        ended = ended or time.monotonic_ns()
        if observer:
            try:
                observer.close()
                c.require(not observer.errors, 'C1 isolation observer failed')
            except Exception as error:
                failure = failure or str(error)
        if before is not None and queue is not None:
            try:
                after, snapshot = c.scc.configuration(output, host, 'after')
                c.require(c.scc.check_configuration(after, host, os.getpid()) == queue, 'C1 allocation changed')
                c.scc.check_scheduler_snapshot(snapshot, host, queue)
                c.check_stable_configuration(before, after)
                if observer and observed_started is not None:
                    observer.validate(observed_started, ended)
            except Exception as error:
                failure = failure or str(error)
    try:
        validate_admission(plan, driver, admitted)
        validate_science(plan, driver, admitted, prerequisite)
    except Exception as error:
        failure = failure or str(error)
    value = {'kind': execution_kind(plan) + '_measurement', 'status': 'failed' if failure else 'complete', 'failure': failure,
             'plan_sha256': c.routing.identity(plan), 'driver_sha256': c.routing.identity(driver),
             'admission_sha256': c.routing.identity(admitted), 'science_sha256': c.routing.identity(prerequisite),
             'host': host, 'collection_pid': os.getpid(), 'native_affinity': sorted(host['affinity'])[:plan.get('native_affinity_cores', 4)],
             'started_monotonic_ns': started, 'completed_monotonic_ns': ended,
             'records': records, 'fingerprints': fingerprints, 'summary': summarize(plan, records),
             'files': bound_files(output), 'accounting': 'pending external C1 qacct and isolation review',
             'scope': m.ANALYSIS['scope'], 'exclusions': []}
    c.save(output / 'measurement.json', c.routing.seal(value))
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    frozen = sub.add_parser('freeze')
    frozen.add_argument('--plan-sha256', required=True)
    frozen.add_argument('--profile', choices=['membership','occurrence','admission'], default='membership')
    frozen.add_argument('--discovery-unopened', action='store_true', required=True)
    frozen.add_argument('--output', type=Path, required=True)
    admitted = sub.add_parser('admit')
    for name in ['plan', 'driver', 'd1-smoke', 'd1-science', 'd1-measurement', 'science-accounting',
                 'measurement-accounting', 'isolation-review', 'output']:
        admitted.add_argument('--' + name, type=Path, required=True)
    for name in ['science', 'measure']:
        stage = sub.add_parser(name)
        for field in ['plan', 'driver', 'admission', 'output']:
            stage.add_argument('--' + field, type=Path, required=True)
        if name == 'measure':
            stage.add_argument('--science', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'freeze':
        freeze(args.plan_sha256, args.output, discovery_unopened=args.discovery_unopened, profile=args.profile)
    elif args.command == 'admit':
        evidence = {name: c.ref(path) for name, path in [
            ('smoke', args.d1_smoke), ('science', args.d1_science), ('measurement', args.d1_measurement),
            ('science_accounting', args.science_accounting), ('measurement_accounting', args.measurement_accounting),
            ('isolation_review', args.isolation_review)]}
        value = admission(unsealed(args.plan), unsealed(args.driver), evidence)
        c.save(args.output, c.routing.seal(value))
    else:
        plan, driver, admitted = [unsealed(path) for path in [args.plan, args.driver, args.admission]]
        value = (science(plan, driver, admitted, args.output) if args.command == 'science' else
                 measure(plan, driver, admitted, unsealed(args.science), args.output))
        c.require(value['status'] in {'passed', 'complete'}, value['failure'] or 'C1 stage failed')
        if args.command == 'measure':
            c.require(value['summary']['status'] == 'passed', 'C1 confirmation did not pass all 16 settings')
    print('MEMBERSHIP_CONFIRMATION_SUCCESS stage=' + args.command, flush=True)


if __name__ == '__main__':
    main()
