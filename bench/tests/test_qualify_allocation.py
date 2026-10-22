"""Synthetic report-only allocation amendments; no native or scheduler calls."""
import copy
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify as q
import test_qualify as fixtures
from test_qualify_scc import allocation, census


class AllocationAmendmentTests(unittest.TestCase):
    def setUp(self):
        self.f = fixtures.Qualification('runTest')
        self.f.setUp()
        self.addCleanup(self.f.doCleanups)
        self.root = self.f.root
        # Real source-vector cardinality, with isolated bytes for source mutation tests.
        sources = {}
        for name in q.execution_sources():
            path = self.root / 'execution' / (name + '.py')
            path.parent.mkdir(exist_ok=True)
            path.write_text('# frozen synthetic ' + name + '\n')
            sources[name] = path
        self.source_patch = patch.object(q, 'execution_sources', return_value=sources)
        self.source_patch.start()
        self.addCleanup(self.source_patch.stop)
        self.before, self.accounting = self.f.linux_host_round(
            [[f'node-{i}' for i in range(12)] for _ in range(3)], 12)
        self.new = q.read(self.f.manifest)
        self.old = copy.deepcopy(self.new)
        for cohort in ('original', 'amended'):
            for name, source in sources.items():
                target = self.root / cohort / source.relative_to(self.root)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes() + (b'# old allocation\n' if
                    cohort == 'original' and name in ('qualify', 'qualify_scc') else b''))
                if cohort == 'original':
                    self.old['execution_sources'][name] = q.digest(target)
        self.old_path = self.root / 'original-plan.json'
        q.write(self.old_path, self.old)
        self.completed = [0, 2, 3, 4]
        frozen, observations = {}, {}
        for replica in range(12):
            for shard in range(3):
                relative = f'run-{replica}-{shard}/results.json'
                path = self.f.output / relative
                result = q.read(path)
                granted, raw = allocation()
                actual = result['host']['host']
                granted.update(actual)
                granted['queue'] = 'batch@' + actual['hostname']
                job, queues = census(task='undefined')
                job = job.replace('123', actual['job_id'])
                queues = queues.replace('123', actual['job_id'])
                raw = json.loads(json.dumps(raw).replace('scc-a.scc.bu.edu', actual['hostname'])
                                 .replace('scc-a', actual['hostname'].split('.')[0]))
                queues = queues.replace('scc-a.scc.bu.edu', actual['hostname'])
                pe = 'omp28'
                if replica not in self.completed:
                    pe = 'mpi_28_tasks_per_node_u'
                    granted['pe'] = pe
                    raw['pe'] = ('pe_name mpi_28_tasks_per_node_u\nallocation_rule 28\n'
                        'control_slaves TRUE\njob_is_first_task FALSE\naccounting_summary FALSE\n'
                        'start_proc_args /usr/local/sge/sge_root/mpi/startmpi.sh -catch_rsh $pe_hostfile\n'
                        'stop_proc_args /usr/local/sge/sge_root/mpi/stopmpi.sh\n')
                    job = job.replace('omp28', pe)
                result['host'].update(host=granted, errors=[], observations='host-observations.jsonl')
                calls = [c for cell in result['cases'] for c in q.cell_calls(cell)]
                start, end = calls[0]['started_monotonic_ns'], calls[-1]['finished_monotonic_ns']
                events = []
                for t in (start - 1, end + 1):
                    events += [dict(kind='reservation', monotonic_ns=t, raw=raw),
                               dict(kind='census', monotonic_ns=t, raw=dict(job=job, queues=queues)),
                               dict(kind='process', monotonic_ns=t, finished_monotonic_ns=t, competing_compute=[])]
                observer = path.parent / 'host-0/host-observations.jsonl'
                observer.write_text(''.join(json.dumps(e) + '\n' for e in events))
                account = self.accounting / f"{actual['job_id']}.undefined.txt"
                account.write_text(account.read_text() + f'granted_pe {pe}\nslots 28\n')
                if replica in self.completed:
                    result.update(manifest_sha256=q.routing.identity(self.old),
                                  execution_sources=self.old['execution_sources'])
                    observations[relative.replace('results.json', 'host-0/host-observations.jsonl')] = q.digest(observer)
                else:
                    result['host_observations_sha256'] = q.digest(observer)
                q.write(path, result)
                if replica in self.completed:
                    frozen[relative] = q.digest(path)
        self.descriptor = dict(schema=1, kind='linux-allocation-amendment-v1',
            original_plan=dict(path=self.old_path.name, sha256=q.digest(self.old_path)),
            amended_plan=dict(path=self.f.manifest.name, sha256=q.digest(self.f.manifest)),
            source_roots=dict(original='original', amended='amended'),
            completed_replicas=self.completed, amended_replicas=[1, 5, 6, 7, 8, 9, 10, 11],
            completed_results=frozen, completed_observations=observations,
            source_changes={name: dict(before=self.old['execution_sources'][name],
                                       after=self.new['execution_sources'][name])
                            for name in ('qualify', 'qualify_scc')})
        self.path = self.root / 'allocation.json'
        q.write(self.path, self.descriptor)

    def report(self):
        return q.report(self.f.manifest, self.f.output, self.accounting,
                        allocation_amendment=self.path)

    def validate(self):
        def verify(path, expected):
            q.require(q.digest(path) == expected, 'test file identity mismatch')
        return q.validate_allocation_amendment(self.path, self.f.manifest, self.f.output, verify)

    def test_mixed_cohorts_preserve_all_scores_and_original_bytes(self):
        original = {p: p.read_bytes() for p in self.f.output.glob('run-*/results.json')}
        actual = self.report()
        self.assertEqual(actual['status'], self.before['status'])
        self.assertEqual(actual['errors'], [])
        self.assertEqual(actual['comparisons'], self.before['comparisons'])
        self.assertEqual(actual['native_calls'], self.before['native_calls'])
        self.assertEqual(actual['allocation_amendment']['descriptor_sha256'], q.digest(self.path))
        self.assertTrue(all(p.read_bytes() == value for p, value in original.items()))
        self.assertEqual(len(actual['comparisons']), 6)
        self.assertTrue(all(c['distinct_host_count'] == 12 for c in actual['comparisons']))
        self.assertEqual(q.report(self.f.manifest, self.f.output, self.accounting)['status'], 'needs_discussion')

    def test_partition_and_completed_hashes_fail_closed(self):
        for field, value in (
                ('completed_replicas', [0, 2, 3, 4, 4]),
                ('amended_replicas', [0, 1, 5, 6, 7, 8, 9, 10, 11]),
                ('amended_replicas', [1, 5, 6, 7, 8, 9, 10]),
                ('completed_results', {}), ('completed_observations', {})):
            with self.subTest(field=field, value=value):
                q.write(self.path, {**self.descriptor, field: value})
                with self.assertRaises(ValueError):
                    self.validate()
        q.write(self.path, self.descriptor)
        path = self.f.output / 'run-0-0/results.json'
        path.write_text(path.read_text() + '\n')
        with self.assertRaisesRegex(ValueError, 'identity mismatch'):
            self.validate()

    def test_exact_plan_delta_includes_budget_science_and_artifacts(self):
        mutations = [('budget_s', 31), ('seed', 8), ('blocks', 4), ('threads', [1, 4]),
                     ('min_distinct_hosts_per_cell', 11), ('scope', 'two'), ('replicas', 11),
                     ('files', {**self.new['files'], 'input.case': 'a' * 64})]
        for field, value in mutations:
            with self.subTest(field=field):
                q.write(self.f.manifest, {**self.new, field: value})
                desc = copy.deepcopy(self.descriptor)
                desc['amended_plan']['sha256'] = q.digest(self.f.manifest)
                q.write(self.path, desc)
                with self.assertRaisesRegex(ValueError, 'changed inputs, artifacts, science, schedule or budgets'):
                    self.validate()

    def test_source_allowlist_and_complete_archives(self):
        changed = copy.deepcopy(self.new)
        changed['execution_sources']['routing_science'] = 'b' * 64
        q.write(self.f.manifest, changed)
        desc = copy.deepcopy(self.descriptor)
        desc['amended_plan']['sha256'] = q.digest(self.f.manifest)
        desc['source_changes']['routing_science'] = dict(
            before=self.old['execution_sources']['routing_science'], after='b' * 64)
        q.write(self.path, desc)
        with self.assertRaisesRegex(ValueError, 'unapproved allocation source delta'):
            self.validate()
        q.write(self.f.manifest, self.new)
        q.write(self.path, self.descriptor)
        (self.root / 'original/execution/replay_ols_reference.py').unlink()
        with self.assertRaises(FileNotFoundError):
            self.validate()

    def test_wrong_new_cohort_identity_and_duplicate_physical_host_rejected(self):
        path = self.f.output / 'run-1-0/results.json'
        result = q.read(path)
        result['manifest_sha256'] = q.routing.identity(self.old)
        q.write(path, result)
        self.assertTrue(any('missing, changed or failed shard' in e for e in self.report()['errors']))
        result['manifest_sha256'] = q.routing.identity(self.new)
        result['hostname'] = 'NODE-0.bu.edu'
        result['host']['host']['hostname'] = 'node-0'
        result['host']['host']['queue'] = 'batch@node-0'
        observer = path.parent / 'host-0/host-observations.jsonl'
        observer.write_text(observer.read_text().replace('node-1.bu.edu', 'node-0').replace('node-1', 'node-0'))
        result['host_observations_sha256'] = q.digest(observer)
        q.write(path, result)
        host = result['host']['host']
        accounting = self.accounting / f"{host['job_id']}.{host['task_id']}.txt"
        accounting.write_text(accounting.read_text().replace('node-1.bu.edu', 'node-0'))
        failed = self.report()
        self.assertEqual(failed['status'], 'needs_discussion')
        self.assertTrue(any('11 distinct physical hosts' in e for e in failed['errors']))

    def test_amended_host_replays_full_reservation_and_accounted_pe(self):
        path = self.f.output / 'run-1-0/results.json'
        observer = path.parent / 'host-0/host-observations.jsonl'
        result, text = q.read(path), observer.read_text()
        observer.write_text(text.replace('enabled TRUE', 'enabled FALSE'))
        q.write(path, {**result, 'host_observations_sha256': q.digest(observer)})
        self.assertTrue(any('missing all-queue full-host quota' in e for e in self.report()['errors']))
        observer.write_text(text)
        q.write(path, result)
        host = result['host']['host']
        account = self.accounting / f"{host['job_id']}.{host['task_id']}.txt"
        account.write_text(account.read_text().replace('granted_pe mpi_28_tasks_per_node_u', 'granted_pe omp28'))
        self.assertTrue(any('accounted allocation PE/slots mismatch' in e for e in self.report()['errors']))


if __name__ == '__main__':
    unittest.main()
