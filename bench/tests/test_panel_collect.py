"""Terminal accounting and coverage guards for aggregate-only collection."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from panel_collect import audit_host_observations, load


def fixture(root):
    def write(name, value):
        path = root/name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value) if not isinstance(value, str) else value)
    context = dict(setup_job=1, smoke_job=2, production_job=3, source='abc',
                   launcher_sha256={'setup.sh': hashlib.sha256(b'setup').hexdigest(),
                                    'job.sh': hashlib.sha256(b'job').hexdigest()})
    write('context.json', context)
    write('setup.sh', 'setup'); write('job.sh', 'job')
    write('input/source.json', dict(commit='abc'))
    jobs = [dict(jobnumber=str(j), taskid=t, failed='0', exit_status='0', slots=str(n), hostname='host'+t)
            for j, t, n in [(1, 'undefined', 4), (2, 'undefined', 4), (3, '1', 28), (3, '2', 28)]]
    write('accounting.json', jobs)
    write('environment/COMPLETE', 'PANEL_SETUP_COMPLETE\n')
    write('environment/input-audit.json', {p: dict(exact_identifiers=True, adjustment_max_error=0.)
                                         for p in ['long', 'short']})
    write('logs/setup.sge', 'PANEL_SETUP_COMPLETE')
    write('logs/smoke.sge', 'PANEL_JOB_COMPLETE')
    write('smoke/results.json', dict(complete=True, native_gate_pass=True, records=[{}]*8,
          science=[dict(kind='dense-oracle', arm='fereg', passed=True)]*2))
    cells = []
    for task, panel in enumerate(['long', 'short'], 1):
        records, science = [], []
        for t in [1, 2, 4, 8, 16]:
            cell = dict(id=f'{panel}-{t}', arms=['fereg', 'fixest', 'fixest-strict']+(['reghdfe'] if t <= 4 else []))
            cells.append(cell)
            for rep in [1, 2, 3]:
                for arm in cell['arms']:
                    records.append(dict(cell=cell['id'], repetition=rep, arm=arm, status='ok',
                                        metadata=dict(rc='0', complete='1')))
                science.append(dict(kind='cross-worker', eligible=True))
                if rep > 1:
                    science.append(dict(kind='fixed-worker-exact', passed=True))
        write(f'task-{task}/results.json', dict(complete=True, native_gate_pass=True, job='3', task=str(task),
                                              records=records, comparisons=[], science=science))
        write(f'task-{task}/COMPLETE', 'PANEL_TASK_COMPLETE\n')
        write(f'host-task-{task}/host.json', dict(isolation_ok=True, errors=[],
              host=dict(job_id='3', task_id=str(task), hostname='host'+str(task))))
        write(f'logs/production.{task}.sge', f'PANEL_JOB_COMPLETE job=3 task={task}')
    write('manifest.json', dict(cells=cells, policy=dict(repetitions=3)))


class PanelCollectionTests(unittest.TestCase):
    def test_host_replay_requires_bracketed_samples_without_interference(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for task in [1, 2]:
                directory = root/f'host-task-{task}'
                directory.mkdir()
                report = dict(isolation_ok=True, errors=[], host={}, started_monotonic_ns=10**9,
                              measurement_end_monotonic_ns=5*10**9,
                              samples={kind: dict(count=2) for kind in ['process', 'census']})
                (directory/'host.json').write_text(json.dumps(report))
                events = [dict(kind='reservation', monotonic_ns=t, raw={}) for t in [0, 6*10**9]]
                events += [dict(kind=kind, monotonic_ns=t, finished_monotonic_ns=t,
                                raw=dict(job='', queues=''), competing_compute=[])
                           for kind in ['process', 'census'] for t in [0, 5*10**9]]
                (directory/'host-observations.jsonl').write_text('\n'.join(map(json.dumps, events)))
            with patch('qualify_scc.validate_reservation') as reservation, patch('qualify_scc.validate_census') as census:
                result = audit_host_observations(root)
                self.assertTrue(all(r['passed'] for r in result.values()))
                self.assertEqual((reservation.call_count, census.call_count), (4, 4))
                path = root/'host-task-1/host-observations.jsonl'
                events = [json.loads(line) for line in path.read_text().splitlines()]
                events[-1]['kind'] = 'process'
                path.write_text('\n'.join(map(json.dumps, events)))
                with self.assertRaisesRegex(ValueError, 'coverage'):
                    audit_host_observations(root)
                events[-1]['kind'] = 'census'
                events[2]['competing_compute'] = ['unexpected process']
                path.write_text('\n'.join(map(json.dumps, events)))
                with self.assertRaisesRegex(ValueError, 'competing'):
                    audit_host_observations(root)

    def test_terminal_complete_aggregate_fixture(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); fixture(root)
            result = load(root)
            self.assertEqual(len(result['records']), 108)

    def test_prepared_panel_run_with_rebuilt_audit(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); fixture(root)
            # One largest-connected-set panel, prepared in its own job.
            manifest = json.loads((root/'manifest.json').read_text())
            manifest['panels'] = {'long-lcs': dict(rows=5509415)}
            manifest['cells'] = [c for c in manifest['cells'] if c['id'].startswith('long-')]
            (root/'manifest.json').write_text(json.dumps(manifest))
            (root/'task-2/results.json').unlink()
            (root/'preparation').mkdir()
            (root/'preparation/inputs.json').write_text('{}')
            (root/'prepare.sge').write_text('prepare')
            (root/'logs/prepare.sge').write_text('PREPARE_COMPLETE\n')
            context = json.loads((root/'context.json').read_text())
            context.update(prepare_job=4, preparation_files={'preparation/inputs.json': hashlib.sha256(b'{}').hexdigest()})
            context['launcher_sha256']['prepare.sge'] = hashlib.sha256(b'prepare').hexdigest()
            (root/'context.json').write_text(json.dumps(context))
            jobs = [j for j in json.loads((root/'accounting.json').read_text()) if j['taskid'] != '2']
            jobs.append(dict(jobnumber='4', taskid='undefined', failed='0', exit_status='0', slots='4', hostname='h'))
            (root/'accounting.json').write_text(json.dumps(jobs))
            (root/'smoke/results.json').write_text(json.dumps(dict(complete=True, native_gate_pass=True, records=[{}]*8,
                science=[dict(kind='dense-oracle', arm='fereg', passed=True)])))
            (root/'environment/input-audit.json').write_text(json.dumps({'long-lcs': dict(rows=5509415, rebuilt_from_source=True)}))
            self.assertEqual(len(load(root)['records']), 54)
            (root/'preparation/inputs.json').write_text('{"changed": 1}')
            with self.assertRaisesRegex(ValueError, 'preparation record'):
                load(root)
            (root/'preparation/inputs.json').write_text('{}')
            (root/'environment/input-audit.json').write_text(json.dumps({'long-lcs': dict(rows=1, rebuilt_from_source=True)}))
            with self.assertRaisesRegex(ValueError, 'rebuilt panel audit'):
                load(root)

    def test_reject_missing_failed_or_duplicate_accounting(self):
        for change in ['missing', 'failed', 'duplicate']:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as d:
                root = Path(d); fixture(root)
                path = root/'accounting.json'; jobs = json.loads(path.read_text())
                if change == 'missing': jobs.pop()
                if change == 'failed': jobs[-1]['exit_status'] = '1'
                if change == 'duplicate': jobs.append(jobs[-1])
                path.write_text(json.dumps(jobs))
                with self.assertRaisesRegex(ValueError, 'accounting'):
                    load(root)

    def test_reject_missing_fit_and_native_error(self):
        for change in ['missing', 'native-error']:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as d:
                root = Path(d); fixture(root)
                path = root/'task-1/results.json'; result = json.loads(path.read_text())
                if change == 'missing': result['records'].pop()
                else: result['records'][0]['status'] = 'error'
                path.write_text(json.dumps(result))
                with self.assertRaises(ValueError):
                    load(root)

    def test_reject_host_and_exactness_failure(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); fixture(root)
            path = root/'host-task-2/host.json'; host = json.loads(path.read_text())
            host['isolation_ok'] = False; path.write_text(json.dumps(host))
            with self.assertRaisesRegex(ValueError, 'whole-host'):
                load(root)
            fixture(root)
            path = root/'task-2/results.json'; result = json.loads(path.read_text())
            next(s for s in result['science'] if s['kind'] == 'fixed-worker-exact')['passed'] = False
            path.write_text(json.dumps(result))
            with self.assertRaisesRegex(ValueError, 'exactness'):
                load(root)


if __name__ == '__main__':
    unittest.main()
