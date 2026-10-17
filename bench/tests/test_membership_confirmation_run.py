"""Pure Python C1 execution checks; native calls and SCC commands are mocked."""
import copy
from contextlib import ExitStack
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import membership_confirmation_run as run

c, m = run.c, run.m


def sha(value):
    return hashlib.sha256(value.encode()).hexdigest()


def plan():
    cells = [{'id': f"{case['family']}-t{workers}", 'family': case['family'], 'threads': workers,
              'metadata': {'path': 'meta'}, 'witness': {'path': 'witness'}}
             for case in m.CASES for workers in [1, 4]]
    return {'kind': m.KIND, 'cells': cells, 'blocks': m.schedule([cell['id'] for cell in cells]),
            'prepared_wall_ns': 1, 'discovery_plan': {'path': 'discovery'}}


def record_fields(cell, arm, label, output):
    target = output / (label + '.json')
    return {'cell': cell['id'], 'arm': arm, 'label': label, 'status': 'executed',
            'target': str(target), 'log': str(target.with_suffix('.log')), 'threads': cell['threads'],
            'started_wall_ns': 1_700_000_000_000_000_000,
            'started_monotonic_ns': 100, 'completed_monotonic_ns': 200}


def parsed(row, meta):
    c.require(row['status'] == 'executed', 'native failure')
    raw = {'arm': row['arm'], 'threads': row['threads'], 'family': row['cell'].rsplit('-t', 1)[0]}
    return raw, {'science_sha256': sha(row['arm'] + str(row['threads'])),
                 'seconds': .9 if row['arm'] == 'cells_m' else 1.}, Path(row['target']).with_suffix('.json.columns')


class DriverAdmission(unittest.TestCase):
    def fixture(self, root):
        p = plan()
        started = 1_700_000_000_000_000_000
        p['prepared_wall_ns'] = started - 1
        d1 = {'kind': c.MEMBERSHIP_KIND}
        def native(at):
            return {'started_wall_ns': at, 'started_monotonic_ns': 100, 'completed_monotonic_ns': 200}
        science = {'host': 'scc-science', 'job_id': '8155623', 'slots': 4,
                   'records': [native(started + 3)],
                   'parent_controls': [{'parent': native(started), 'candidate': native(started + 1)}]}
        measurement = {'host': {'hostname': 'scc-timing', 'job_id': '8155634', 'slots': 28},
                       'records': [native(started + 100)], 'files': []}
        observed = {}
        for name in run.ISOLATION_FILES:
            path = root / (name + '.jsonl')
            path.write_text('{}\n')
            observed[name] = c.ref(path)
            measurement['files'].append(c.ref(path))
        def account(job, host, slots, pe):
            stamp = lambda seconds: datetime.fromtimestamp(seconds, timezone.utc).strftime('%a %b %d %H:%M:%S %Y')
            fields = {'jobnumber': job, 'project': 'welfgr', 'hostname': host, 'slots': str(slots),
                      'granted_pe': pe, 'failed': '0', 'exit_status': '0',
                      'start_time': stamp(started / 1e9 - 1), 'end_time': stamp(started / 1e9 + 1)}
            return {'argv': ['qacct', '-j', job], 'returncode': 0, 'locale': 'C', 'timezone': 'UTC',
                    'stderr': '', 'stdout': '\n'.join(f'{key} {value}' for key, value in fields.items()) + '\n'}
        smoke = copy.deepcopy(science)
        smoke.update(kind=c.MEMBERSHIP_KIND + '_science', status='passed', smoke=True,
                     plan_sha256=c.routing.identity(d1), files=[])
        for index, row in enumerate(run.native_rows(smoke)):
            row.update(target=str(root / f'smoke-{index}.json'), log=str(root / f'smoke-{index}.log'))
            for path in [Path(row['target']), Path(row['log']), Path(row['target']).with_suffix('.json.columns')]:
                path.write_text('{}')
                smoke['files'].append(c.ref(path))
        values = {'smoke': smoke, 'science': science, 'measurement': measurement,
                  'science_accounting': account('8155623', 'scc-science', 4, 'omp'),
                  'measurement_accounting': account('8155634', 'scc-timing', 28, 'omp28')}
        paths = {name: root / (name + '.json') for name in [*values, 'isolation_review']}
        def write(name):
            value = values[name] if name.endswith('accounting') else c.routing.seal(values[name])
            paths[name].write_text(json.dumps(value))
            return c.ref(paths[name])
        evidence = {name: write(name) for name in values}
        values['isolation_review'] = {'kind': c.MEMBERSHIP_KIND + '_isolation_review', 'status': 'passed',
            'plan_sha256': c.routing.identity(d1), 'measurement_sha256': c.routing.identity(measurement),
            'host': measurement['host'], 'accounting': evidence['measurement_accounting'], 'reviewer': 'test reviewer',
            'observation_complete': True, 'observed_compute_overlap': [], 'observed_host_faults': [],
            'exclusions': [], 'raw_evidence': observed}
        evidence['isolation_review'] = write('isolation_review')
        source = root / 'discovery.json'
        source.write_text(json.dumps(c.routing.seal(d1)))
        p['discovery_plan'] = c.ref(source)
        driver = run.freeze(c.routing.identity(p), root / 'driver.json', discovery_unopened=True)
        return p, driver, evidence, values, write

    def test_portable_freeze_requires_unopened_attestation_and_detects_source_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            p = plan()
            with patch.object(c, 'read', side_effect=AssertionError('freeze must not read discovery outcomes')):
                driver = run.freeze(c.routing.identity(p), root / 'driver.json', discovery_unopened=True)
            self.assertEqual(set(driver['sources']), set(run.SOURCES))
            self.assertTrue(all(not Path(name).is_absolute() for name in driver['sources']))
            run.validate_driver(p, driver)
            for mutation in ['source', 'plan', 'attestation', 'preparation']:
                changed = copy.deepcopy(driver)
                if mutation == 'source':
                    changed['sources'][run.SOURCES[0]] = '0' * 64
                elif mutation == 'plan':
                    changed['plan_sha256'] = '0' * 64
                elif mutation == 'attestation':
                    changed['discovery_outcomes_unopened'] = False
                else:
                    changed['preparation_source_sha256'] = '0' * 64
                with self.assertRaisesRegex(ValueError, 'driver freeze'):
                    run.validate_driver(p, changed)
            with self.assertRaisesRegex(ValueError, 'unopened'):
                run.freeze(c.routing.identity(p), root / 'invalid.json', discovery_unopened=False)

    def test_admission_recalculates_gate_and_validates_both_accounting_records_and_review(self):
        with tempfile.TemporaryDirectory() as directory:
            p, driver, evidence, _, _ = self.fixture(Path(directory))
            with patch.object(m, 'validate'), \
                 patch.object(c, 'membership_gate', return_value={'status': 'eligible', 'finalist': 'cells_m'}) as gate:
                admitted = run.admission(p, driver, evidence)
                self.assertEqual(admitted['status'], 'admitted')
                self.assertEqual(admitted['earliest_d1_native_wall_ns'], 1_700_000_000_000_000_000)
                run.validate_admission(p, driver, admitted)
                self.assertEqual(gate.call_count, 2)
                admitted['gate_sha256'] = 'changed'
                with self.assertRaisesRegex(ValueError, 'admission receipt changed'):
                    run.validate_admission(p, driver, admitted)

    def test_admission_rejects_gate_failure_late_freeze_and_external_evidence_failures(self):
        mutations = ['gate', 'late', 'smoke-before-freeze', 'smoke-job', 'smoke-window', 'science-failed', 'measurement-exit', 'science-slots', 'science-pe',
                     'measurement-job', 'science-window', 'missing-time', 'overlap', 'fault',
                     'excluded', 'incomplete-review', 'wrong-evidence', 'missing-accounting']
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                p, driver, evidence, values, write = self.fixture(Path(directory))
                gate = {'status': 'ineligible' if mutation == 'gate' else 'eligible', 'finalist': 'cells_m'}
                name = None
                if mutation == 'late':
                    p['prepared_wall_ns'] = 1_700_000_000_000_000_000
                    driver['plan_sha256'] = c.routing.identity(p)
                elif mutation in {'smoke-before-freeze', 'smoke-job', 'smoke-window'}:
                    name = 'smoke'
                    if mutation == 'smoke-job':
                        values[name]['job_id'] = 'other'
                    else:
                        at = 1_700_000_000_000_000_000 - (2 if mutation == 'smoke-before-freeze' else 2_000_000_000)
                        values[name]['parent_controls'][0]['parent']['started_wall_ns'] = at
                        if mutation == 'smoke-window':
                            p['prepared_wall_ns'] = at - 1
                            driver['plan_sha256'] = c.routing.identity(p)
                elif mutation in {'science-failed', 'science-slots', 'science-pe', 'science-window'}:
                    name = 'science_accounting'
                    old, new = {'science-failed': ('failed 0', 'failed 1'),
                                'science-slots': ('slots 4', 'slots 28'),
                                'science-pe': ('granted_pe omp\n', 'granted_pe omp28\n'),
                                'science-window': ('22:13:19', '22:13:21')}[mutation]
                    values[name]['stdout'] = values[name]['stdout'].replace(old, new)
                elif mutation in {'measurement-exit', 'measurement-job'}:
                    name = 'measurement_accounting'
                    old, new = ('exit_status 0', 'exit_status 1') if mutation == 'measurement-exit' else ('jobnumber 8155634', 'jobnumber 1')
                    values[name]['stdout'] = values[name]['stdout'].replace(old, new)
                elif mutation == 'missing-time':
                    name = 'science'
                    del values[name]['parent_controls'][0]['parent']['completed_monotonic_ns']
                elif mutation == 'missing-accounting':
                    del evidence['science_accounting']
                elif mutation != 'gate':
                    name = 'isolation_review'
                    review = values[name]
                    if mutation == 'overlap':
                        review['observed_compute_overlap'] = ['competing build']
                    elif mutation == 'fault':
                        review['observed_host_faults'] = ['host fault']
                    elif mutation == 'excluded':
                        review['exclusions'] = ['bad block']
                    elif mutation == 'incomplete-review':
                        review['observation_complete'] = False
                    else:
                        review['raw_evidence'].pop('processes')
                if name:
                    evidence[name] = write(name)
                with patch.object(m, 'validate'), patch.object(c, 'membership_gate', return_value=gate), \
                     self.assertRaises(ValueError):
                    run.admission(p, driver, evidence)


class ScienceExecution(unittest.TestCase):
    def execute(self, output, mode='complete'):
        p, driver, admitted = plan(), {'driver': 'bound'}, {'admission': 'bound'}
        calls = []
        def native(p, cell, arm, output, label, **options):
            calls.append(label)
            self.assertEqual(bool(options.get('traced')), label.endswith('-0'))
            self.assertLessEqual(options['timeout'], 120)
            row = record_fields(cell, arm, label, output)
            if mode == 'timeout':
                row['status'] = 'timeout'
            for path in [Path(row['target']), Path(row['log']), Path(row['target']).with_suffix('.json.columns')]:
                path.write_text('{}')
            return row
        def parse(row, meta):
            raw, result, binary = parsed(row, meta)
            if mode == 'repeat' and row['label'].endswith('-1'):
                result['science_sha256'] = sha('changed repeat')
            return raw, result, binary
        def oracle(*args):
            if mode == 'oracle':
                raise ValueError('independent oracle failed')
        def compare(left, right, *args):
            if mode == 'cross-worker' and right['arm'] == 'cells_m' and left['threads'] != right['threads']:
                raise ValueError('cross-worker decisions differ')
        with ExitStack() as stack:
            validators = stack.enter_context(patch.object(run, 'validate_admission',
                side_effect=ValueError('D1 blocks C1') if mode == 'blocked' else None))
            stack.enter_context(patch.dict(os.environ, {**c.LIBRARY_LIMITS, 'JOB_ID': '1', 'NSLOTS': '4'}))
            stack.enter_context(patch.object(c, 'native', side_effect=native))
            stack.enter_context(patch.object(c, 'parse_run', side_effect=parse))
            stack.enter_context(patch.object(c, 'read', return_value={'case_sha256': 'a', 'body_sha256': 'b'}))
            stack.enter_context(patch.object(c.routing_oracle, 'load_reference', return_value=({}, {})))
            checked = stack.enter_context(patch.object(c.routing_oracle, 'check_reference', side_effect=oracle))
            comparisons = stack.enter_context(patch.object(c.cmgschur, 'scientific_comparison', side_effect=compare))
            stack.enter_context(patch.object(c.routing_run, 'model_discrete_sha256', side_effect=lambda raw: sha(raw['family'])))
            stack.enter_context(patch.object(c.routing_run, 'resource_diagnostics', return_value={'resource': 'passed'}))
            if mode == 'budget':
                stack.enter_context(patch.object(run.time, 'monotonic', side_effect=[0, 1801, 1801]))
            if mode == 'blocked':
                with self.assertRaisesRegex(ValueError, 'D1 blocks'):
                    run.science(p, driver, admitted, output)
                self.assertEqual(calls, [])
                self.assertFalse(output.exists())
                return
            receipt = run.science(p, driver, admitted, output)
            if mode == 'complete':
                self.assertEqual(receipt['status'], 'passed', receipt['failure'])
                self.assertEqual(len(calls), 64)
                self.assertEqual(checked.call_count, 64)
                self.assertEqual(comparisons.call_count, 48)
                self.assertEqual(validators.call_count, 2)
                run.validate_science(p, driver, admitted, receipt)
            else:
                self.assertEqual(receipt['status'], 'failed')
                self.assertEqual(len(calls), {'repeat': 2, 'oracle': 1, 'cross-worker': 7,
                                              'timeout': 1, 'budget': 0}[mode])
                self.assertTrue((output / 'science.json').exists())
            return p, driver, admitted, receipt

    def test_full_science_exact_counts_independent_checks_and_strict_receipts(self):
        with tempfile.TemporaryDirectory() as directory:
            p, driver, admitted, receipt = self.execute(Path(directory) / 'science')
            for mutation in ['missing', 'extra', 'order', 'oracle', 'repeat', 'discrete', 'file', 'driver']:
                changed = copy.deepcopy(receipt)
                if mutation == 'missing':
                    changed['records'].pop()
                elif mutation == 'extra':
                    changed['records'].append(changed['records'][0])
                elif mutation == 'order':
                    changed['records'][0], changed['records'][1] = changed['records'][1], changed['records'][0]
                elif mutation == 'oracle':
                    changed['records'][0]['oracle_status'] = 'failed'
                elif mutation == 'repeat':
                    changed['records'][1]['science_sha256'] = sha('other')
                elif mutation == 'discrete':
                    changed['records'][4]['model_discrete_sha256'] = sha('other')
                elif mutation == 'file':
                    changed['files'].pop()
                else:
                    changed['driver_sha256'] = 'changed'
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    run.validate_science(p, driver, admitted, changed)

    def test_first_failure_and_budget_stop_without_retries(self):
        for mode in ['blocked', 'repeat', 'oracle', 'cross-worker', 'timeout', 'budget']:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                self.execute(Path(directory) / 'science', mode)


class ConfirmationMeasurement(unittest.TestCase):
    def records(self, p):
        rows = []
        for label, cid, arm, phase, block, position in run.measurement_order(p):
            rows.append({'label': label, 'cell': cid, 'arm': arm, 'phase': phase,
                         'block': block, 'position': position, 'status': 'passed',
                         'seconds': .9 if arm == 'cells_m' else 1., 'science_sha256': sha(cid + arm),
                         'resources': {'bound': True}})
        return rows

    def test_geometric_means_allowance_exact_rank_ties_and_complete_settings(self):
        p = plan()
        score = run.block_score(0, 'case', [{'arm': 'baseline', 'seconds': t} for t in [1., 4.]]
                               + [{'arm': 'cells_m', 'seconds': t} for t in [.5, 2.]])
        self.assertAlmostEqual(score['baseline_seconds'], 2.)
        self.assertAlmostEqual(score['candidate_seconds'], 1.)
        self.assertAlmostEqual(score['excess_seconds'], -1.1)
        rows = self.records(p)
        result = run.summarize(p, rows)
        self.assertEqual(result['status'], 'passed')
        self.assertEqual(len(result['block_scores']), 192)
        self.assertEqual(len(result['settings']), 16)
        self.assertTrue(all(row['complete_blocks'] == 12 for row in result['settings']))
        for bad in [rows[:-1], [*rows[:-1], {**rows[-1], 'status': 'failed'}]]:
            self.assertEqual(run.summarize(p, bad)['status'], 'incomplete')
        with self.assertRaisesRegex(ValueError, 'order/position'):
            run.summarize(p, [rows[1], rows[0], *rows[2:]])
        for upper, expected in [(0., 'passed'), (.001, 'failed')]:
            def score(block, cid, group, arms=m.ARMS):
                return {'block': block, 'cell': cid, 'ratio': 1.,
                        'excess_seconds': -1. if block < 9 else upper}
            with patch.object(run, 'block_score', side_effect=score):
                result = run.summarize(p, rows)
            self.assertEqual(result['status'], expected)
            self.assertTrue(all(row['upper_median_excess_seconds'] == upper for row in result['settings']))
        changed = copy.deepcopy(rows)
        for row in changed:
            if row['cell'] == p['cells'][0]['id'] and row['arm'] == 'cells_m' and row['phase'] == 'measure':
                row['seconds'] = 2.
        self.assertEqual(run.summarize(p, changed)['status'], 'failed')

    def execute(self, output, mode='complete'):
        p, calls, boundaries = plan(), [], []
        observer = Mock(errors=[])
        host = {'slots': 28, 'affinity': list(range(28)), 'hostname': 'scc-test', 'job_id': '2'}
        def native(p, cell, arm, output, label, **options):
            calls.append(label)
            self.assertEqual(options['cpus'], [0, 1, 2, 3])
            self.assertFalse(options.get('traced', False))
            self.assertLessEqual(options['timeout'], 120)
            row = record_fields(cell, arm, label, output)
            if mode == 'timeout' and label.startswith('block-'):
                row['status'] = 'timeout'
            return row
        def prerequisite(row, cell, receipt, *, traced):
            self.assertFalse(traced)
            row['resources'] = {'prerequisite_science_sha256': sha('bound science')}
            return parsed(row, {})[1]
        def parse(row, meta):
            raw, result, binary = parsed(row, meta)
            if mode == 'science' and row['label'].startswith('block-'):
                result['science_sha256'] = sha('changed same-allocation science')
            return raw, result, binary
        def configuration(output, host, stage):
            boundaries.append(stage)
            if mode == 'boundary' and stage == 'after':
                raise ValueError('missing after configuration')
            return {}, {}
        with ExitStack() as stack:
            for owner, name, options in [
                (run, 'validate_admission', {'side_effect': ValueError('D1 not admitted') if mode == 'blocked' else None}),
                (run, 'validate_science', {}), (c, 'native', {'side_effect': native}),
                (c, 'check_prerequisite', {'side_effect': prerequisite}), (c, 'parse_run', {'side_effect': parse}),
                (c, 'read', {'return_value': {}}), (c, 'Observers', {'return_value': observer}),
                (c, 'check_stable_configuration', {}), (c.scc.discovery, 'host', {'return_value': host}),
                (c.scc, 'configuration', {'side_effect': configuration}),
                (c.scc, 'check_configuration', {'return_value': 'queue'}), (c.scc, 'check_scheduler_snapshot', {}),
                (c.routing_oracle, 'check_reference', {'side_effect': AssertionError('no oracle work during measurement')}),
            ]:
                stack.enter_context(patch.object(owner, name, **options))
            stack.enter_context(patch.dict(os.environ, c.LIBRARY_LIMITS))
            if mode == 'budget':
                stack.enter_context(patch.object(run.time, 'monotonic_ns', side_effect=[1, 1, 1801_000_000_001, 1801_000_000_001]))
            if mode == 'blocked':
                with self.assertRaisesRegex(ValueError, 'D1 not admitted'):
                    run.measure(p, {}, {}, {}, output)
                self.assertEqual(calls, [])
                self.assertFalse(output.exists())
                return
            result = run.measure(p, {}, {}, {}, output)
        self.assertEqual(boundaries, ['before', 'after'])
        self.assertTrue((output / 'measurement.json').exists())
        self.assertEqual(result['exclusions'], [])
        if mode == 'complete':
            self.assertEqual(result['status'], 'complete', result['failure'])
            self.assertEqual(result['summary']['status'], 'passed')
            self.assertEqual(len(calls), 800)
            self.assertEqual(len(set(calls)), 800)
            self.assertEqual([run.row_identity(row) for row in result['records']], run.measurement_order(p))
            self.assertEqual(len((output / 'blocks.jsonl').read_text().splitlines()), 192)
            self.assertEqual(observer.census.call_count, 24)
            run.validate_measurement_records(p, result['records'], result['fingerprints'])
            changed = copy.deepcopy(result['records'])
            changed[-1]['science_sha256'] = sha('changed')
            with self.assertRaisesRegex(ValueError, 'same-allocation'):
                run.validate_measurement_records(p, changed, result['fingerprints'])
        else:
            self.assertEqual(result['status'], 'failed')
            self.assertEqual(len(calls), {'science': 33, 'timeout': 33, 'budget': 32, 'boundary': 800}[mode])
            if mode != 'boundary':
                self.assertEqual(result['summary']['status'], 'incomplete')

    def test_exact_32_warmups_and_768_untraced_calls_on_shared_four_cpu_affinity(self):
        with tempfile.TemporaryDirectory() as directory:
            self.execute(Path(directory) / 'measure')

    def test_prerequisite_failure_timeout_budget_and_final_isolation_fail_closed(self):
        for mode in ['blocked', 'science', 'timeout', 'budget', 'boundary']:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                self.execute(Path(directory) / 'measure', mode)


if __name__ == '__main__':
    unittest.main()
