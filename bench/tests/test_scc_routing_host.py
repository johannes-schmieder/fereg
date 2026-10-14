"""Pure synthetic SCC evidence replay; no scheduler, native work or process signals."""
from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import scc_routing_host as host


def command(argv, stdout):
    return {'argv': argv, 'stdout': stdout, 'stderr': '', 'returncode': 0, 'locale': 'C', 'timezone': 'UTC'}


def process_text(cpu='00:00:01', other=None):
    text = f'42 1 Wed Oct 14 20:00:00 2026 S {cpu} 0.0 100 python3\n'
    args = '42 Wed Oct 14 20:00:00 2026 python3 bench/scc_routing_host.py collect\n'
    if other:
        pid, parent, name, clock, percent, state = other
        text += f'{pid} {parent} Wed Oct 14 20:00:00 2026 {state} {clock} {percent} 100 {name}\n'
        args += f'{pid} Wed Oct 14 20:00:00 2026 /opt/{name}\n'
    return text, args


class Fixture:
    def __init__(self, root):
        self.root = root
        self.plan = {'receipt_generator': host.discovery.reference(host.__file__),
                     'harness': '/opt/timing-harness', 'frozen_files': {}, 'output_root': str(root)}
        for path in [host.__file__, host.block_host.__file__, host.discovery.__file__, host.routing.__file__]:
            ref = host.discovery.reference(path)
            self.plan['frozen_files'][ref['path']] = ref['sha256']
        machine = {'hostname': 'scc-test1', 'job_id': '42', 'slots': 28,
                   'affinity': list(range(28)), 'machine': 'x86_64',
                   'library_limits': host.discovery.LIBRARY_LIMITS}
        wall = int(datetime(2026, 10, 14, 20, 0, tzinfo=timezone.utc).timestamp() * 1e9)
        self.collection = {'plan_sha256': host.routing.identity(self.plan), 'status': 'complete',
            'host': machine, 'collection_pid': 42, 'rounds': [{'round': n} for n in range(3)],
            'started_monotonic_ns': 2_000_000_000, 'completed_monotonic_ns': 6_000_000_000,
            'started_wall_time_ns': wall + 2_000_000_000, 'completed_wall_time_ns': wall + 6_000_000_000}
        env = {'environment': {'JOB_ID': '42', 'NSLOTS': '28', 'PE': 'omp28', 'QUEUE': 'econ'},
               'library_limits': machine['library_limits'], 'pe_hostfile': 'scc-test1 28 econ@scc-test1 UNDEFINED\n'}
        config = {'complexes': command(['qconf', '-sc'], 'num_proc p INT == YES NO 0 0\nslots s INT <= YES YES 1 1\n'),
                  'host': command(['qconf', '-se', 'scc-test1'], 'hostname scc-test1\ncomplex_values mem_per_core=9G\nload_values num_proc=28,mem_total=128G,np_load_avg=0.01\nprocessors 28\n'),
                  'queue': command(['qconf', '-sq', 'econ'], 'qname econ\nslots 28\n')}
        pe = command(['qconf', '-sp', 'omp28'], 'pe_name omp28\nallocation_rule $pe_slots\n')
        quota = command(['qconf', '-srqs', 'max_slots_per_host'], '{\nname max_slots_per_host\nenabled TRUE\nlimit hosts {*} to slots=$num_proc\n}\n')
        affinity = {'proc_status': 'Pid:\t42\nCpus_allowed_list:\t0-27\nVmRSS:\t100 kB\n',
                    'topology': command(['lscpu', '-p=CPU,CORE,SOCKET,ONLINE'], '#comment\n' + ''.join(f'{i},{i % 14},{i // 14},Y\n' for i in range(28)))}
        for key, value in [('job_environment', env), ('queue_configuration', config),
                           ('parallel_environment', pe), ('resource_quota', quota), ('affinity', affinity)]:
            self.write(key, [{'stage': phase, 'value': value} for phase in ['before', 'after']])
        xml = '<job_info><queue_info><Queue-List><name>econ@scc-test1.scc.bu.edu</name><job_list state="running"><JB_job_number>42</JB_job_number><slots>28</slots></job_list></Queue-List></queue_info></job_info>'
        self.write('scheduler', [{'stage': phase, 'monotonic_ns': second * 10**9,
            'job': command(['qstat', '-j', '42'], 'job_number: 42\nproject: welfgr\nparallel environment: omp28 range: 28\nhard resource_list: num_proc=28,h_rt=43200\n'),
            'queues': command(['qstat', '-f', '-xml', '-q', '*@scc-test1', '-u', '*'], xml)}
            for phase, second in [('before', 1), ('after', 7)]])
        self.processes()
        account = command(['qacct', '-j', '42'], 'hostname scc-test1.scc.bu.edu\nproject welfgr\njobnumber 42\ngranted_pe omp28\nslots 28\nfailed 0\nexit_status 0\nstart_time Wed Oct 14 20:00:00 2026\nend_time Wed Oct 14 20:00:10 2026\n')
        (root / 'accounting.json').write_text(json.dumps(account))

    def write(self, key, values):
        (self.root / (key + '.jsonl')).write_text(''.join(json.dumps(v) + '\n' for v in values))

    def mutate(self, key, operation):
        values = list(host.rows(self.root / (key + '.jsonl')))
        operation(values)
        self.write(key, values)

    def processes(self, other=None):
        text, args = process_text(other=other)
        values = [{'kind': 'start', 'pid': 42, 'plan_sha256': host.routing.identity(self.plan), 'policy': host.policy(self.plan)}]
        values += [{'kind': 'snapshot', 'monotonic_ns': second * 10**9,
                    'finished_monotonic_ns': second * 10**9 + 1000,
                    'ps': command(['ps', '-axo', host.PS_FIELDS], text),
                    'args': command(['ps', '-axo', host.PS_ARGS], args)} for second in [1, 7]]
        values += [{'kind': 'end', 'status': 'complete', 'collection_sha256': host.routing.identity(self.collection)}]
        self.write('process_observations', values)

    def receipt(self):
        raw = {key: host.discovery.reference(self.root / (key + ('.json' if key == 'accounting' else '.jsonl'))) for key in host.KINDS}
        return {'kind': host.discovery.KIND + '_isolation', 'schema': 1,
                'plan_sha256': host.routing.identity(self.plan), 'collection_sha256': host.routing.identity(self.collection),
                'generator': self.plan['receipt_generator'], 'host': self.collection['host'],
                'rounds': self.collection['rounds'], 'raw_evidence': raw,
                **host.derived(self.plan, self.collection, raw)}


class SCCEvidence(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.fixture = Fixture(Path(self.directory.name))

    def verify(self):
        f = self.fixture
        return host.validate(f.plan, f.collection, f.receipt())

    def test_complete_raw_proof_and_dynamic_os_values(self):
        self.fixture.mutate('queue_configuration', lambda v: v[1]['value']['host'].update(stdout=v[1]['value']['host']['stdout'].replace('0.01', '8.5')))
        self.fixture.mutate('affinity', lambda v: v[1]['value'].update(proc_status=v[1]['value']['proc_status'].replace('100 kB', '800 kB')))
        self.assertEqual(self.verify()['status'], 'verified')

    def test_slots_alone_without_global_all_queue_rule_refuse(self):
        for bad in ['limit queues {econ} hosts {*} to slots=$num_proc', 'limit hosts {*} to slots=56']:
            with self.subTest(bad=bad):
                self.fixture = Fixture(Path(self.directory.name))
                self.fixture.mutate('resource_quota', lambda v: [r['value'].update(stdout=r['value']['stdout'].replace('limit hosts {*} to slots=$num_proc', bad)) for r in v])
                with self.assertRaises(ValueError): self.verify()

    def test_disabled_quota_and_changed_num_proc_refuse(self):
        self.fixture.mutate('resource_quota', lambda v: v[1]['value'].update(stdout=v[1]['value']['stdout'].replace('TRUE', 'FALSE')))
        with self.assertRaises(ValueError): self.verify()

    def test_conflicting_static_capacity_or_processors_refuses(self):
        for old, new in [('mem_per_core=9G', 'mem_per_core=9G,num_proc=56'), ('processors 28', 'processors 56')]:
            with self.subTest(new=new):
                self.fixture = Fixture(Path(self.directory.name))
                self.fixture.mutate('queue_configuration', lambda v: [r['value']['host'].update(
                    stdout=r['value']['host']['stdout'].replace(old, new)) for r in v])
                with self.assertRaises(ValueError): self.verify()

    def test_nonexclusive_affinity_and_smt_refuse(self):
        self.fixture.mutate('affinity', lambda v: v[1]['value'].update(proc_status='Pid: 42\nCpus_allowed_list: 0-26\n'))
        with self.assertRaises(ValueError): self.verify()

    def test_second_pe_host_refuses(self):
        self.fixture.mutate('job_environment', lambda v: [r['value'].update(pe_hostfile=r['value']['pe_hostfile'] + 'scc-test2 28 econ@scc-test2 X\n') for r in v])
        with self.assertRaises(ValueError): self.verify()

    def test_scheduler_cotenancy_refuses(self):
        extra = '<job_list><JB_job_number>99</JB_job_number><slots>1</slots></job_list>'
        self.fixture.mutate('scheduler', lambda v: v[0]['queues'].update(stdout=v[0]['queues']['stdout'].replace('</Queue-List>', extra + '</Queue-List>')))
        with self.assertRaises(ValueError): self.verify()

    def test_unrelated_native_compute_refuses_but_owned_child_is_allowed(self):
        self.fixture.processes(other=(43, 42, 'timing-harness', '00:00:01', 200, 'R'))
        self.assertEqual(self.verify()['status'], 'verified')
        self.fixture.processes(other=(99, 1, 'timing-harness', '00:00:01', 200, 'R'))
        with self.assertRaises(ValueError): self.verify()

    def test_ordinary_os_cpu_is_preserved_not_an_admission_threshold(self):
        self.fixture.processes(other=(99, 1, 'system-service', '00:00:01', 500, 'R'))
        self.assertEqual(self.verify()['status'], 'verified')

    def test_missing_sample_and_asserted_flags_cannot_pass(self):
        f = self.fixture
        receipt = f.receipt()
        receipt['reservation']['rqs_all_queues'] = False
        with self.assertRaises(ValueError): host.validate(f.plan, f.collection, receipt)
        f.mutate('process_observations', lambda v: v.pop(2))
        with self.assertRaises(ValueError): self.verify()

    def test_uncovered_scheduler_window_refuses(self):
        self.fixture.mutate('scheduler', lambda v: v[1].update(monotonic_ns=5_000_000_000))
        with self.assertRaises(ValueError): self.verify()

    def test_accounting_and_job_identity_refuse(self):
        path = self.fixture.root / 'accounting.json'
        value = json.loads(path.read_text())
        value['stdout'] = value['stdout'].replace('exit_status 0', 'exit_status 1')
        path.write_text(json.dumps(value))
        with self.assertRaises(ValueError): self.verify()

    def test_raw_hash_mutation_and_unbound_tool_refuse(self):
        f = self.fixture
        receipt = f.receipt()
        with (f.root / 'scheduler.jsonl').open('a') as stream: stream.write('{}\n')
        with self.assertRaises(ValueError): host.validate(f.plan, f.collection, receipt)
        f.plan['frozen_files'].clear()
        with self.assertRaises(ValueError): host.check_sources(f.plan)

    def test_missing_or_excessive_scheduler_wall_budget_refuses(self):
        self.fixture.mutate('scheduler', lambda v: v[0]['job'].update(stdout=v[0]['job']['stdout'].replace('43200', '43201')))
        with self.assertRaises(ValueError): self.verify()

    def test_invalid_initial_reservation_prevents_native_collection(self):
        f = self.fixture
        values = {key: list(host.rows(f.root / (key + '.jsonl')))[0]['value'] for key in
                  ('job_environment', 'queue_configuration', 'parallel_environment', 'resource_quota', 'affinity')}
        values['resource_quota']['stdout'] = values['resource_quota']['stdout'].replace('TRUE', 'FALSE')
        census = next(host.rows(f.root / 'scheduler.jsonl'))
        with patch.object(host.discovery, 'validate_plan'), patch.object(host.discovery, 'host', return_value=f.collection['host']), \
             patch.object(host.os, 'getpid', return_value=42), patch.object(host, 'configuration', return_value=(values, census)), \
             patch.object(host.discovery, 'collect') as collect:
            with self.assertRaises(ValueError): host.observe(f.plan, {})
        collect.assert_not_called()

    def test_absolute_periodic_cadence_does_not_add_command_latency(self):
        clock, starts = [0.0], []
        stop = Mock()
        def wait(seconds):
            clock[0] += seconds
            return len(starts) == 3
        stop.wait.side_effect = wait
        def action():
            starts.append(clock[0])
            clock[0] += 20.0
        with patch.object(host.time, 'monotonic', side_effect=lambda: clock[0]):
            host.periodic(stop, 60, action, self.fixture.root / 'errors.jsonl', 0)
        self.assertEqual(starts, [60.0, 120.0, 180.0])

    def test_scheduler_io_has_a_separate_thread_from_process_coverage(self):
        with patch.object(host.threading, 'Thread') as constructor:
            host.start_observers(Mock(), self.fixture.root, self.fixture.collection['host'], 123, 456)
        calls = constructor.call_args_list
        self.assertEqual(len(calls), 2)
        self.assertEqual([c.kwargs['args'][1] for c in calls], [5, 60])
        self.assertEqual([c.kwargs['args'][-1] for c in calls], [123, 456])
        self.assertTrue(all(c.kwargs['target'] is host.periodic for c in calls))

    def test_first_deadline_is_anchored_before_initial_command_latency(self):
        for interval, initial_latency, expected in [(5, 6, 6), (60, 26, 60)]:
            with self.subTest(interval=interval):
                clock, starts = [float(initial_latency)], []
                stop = Mock()
                def wait(seconds):
                    clock[0] += seconds
                    return bool(starts)
                stop.wait.side_effect = wait
                with patch.object(host.time, 'monotonic', side_effect=lambda: clock[0]):
                    host.periodic(stop, interval, lambda: starts.append(clock[0]),
                                  self.fixture.root / 'errors.jsonl', 0)
                self.assertEqual(starts, [expected])

    def test_validate_stdin_emits_only_bound_response(self):
        f = self.fixture
        receipt = f.receipt()
        result = subprocess.run([sys.executable, host.__file__, 'validate-stdin'],
            input=json.dumps({'plan': f.plan, 'collection': f.collection, 'receipt': receipt}),
            text=True, capture_output=True, timeout=30, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), host.validate(f.plan, f.collection, receipt))
        self.assertEqual(result.stderr, '')


if __name__ == '__main__':
    unittest.main()
