"""Narrow prefix eligibility never repairs the original failed full protocol."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_discovery as discovery
import routing_window as window
from test_routing_discovery import producer, save_json, spec42
from test_routing import h


NS = 1_000_000_000


class WindowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.spec = spec42()
        self.plan = {'kind': discovery.KIND + '_plan', 'design': discovery.DESIGN,
                     'tournament': routing.seal(self.spec), 'producer_tools': {'test': h('source')},
                     'plugins': {a: '/artifacts/' + a for a in self.spec['arms']},
                     'harness': '/artifacts/native-harness', 'output_root': str(self.root),
                     'library_limits': discovery.LIBRARY_LIMITS}
        self.machine = {'hostname': 'test', 'job_id': '42', 'slots': 28,
                        'machine': 'x86_64', 'library_limits': discovery.LIBRARY_LIMITS}
        original = producer(self.spec, True)
        self.proof = {'kind': discovery.KIND + '_prerequisites', 'status': 'ready',
                      'plan_sha256': routing.identity(self.plan), 'host': self.machine,
                      'records': save_json(self.root / 'prerequisites.json', original), 'wrappers': {}}
        for cell in self.spec['panels']['A']:
            self.proof['wrappers'][cell['id']] = save_json(self.root / (cell['id'] + '.json'), {'ok': cell['id']})
        self.collection = {'kind': discovery.KIND + '_collection', 'status': 'complete',
                           'plan_sha256': routing.identity(self.plan),
                           'prerequisites_sha256': routing.identity(self.proof), 'host': self.machine,
                           'collection_pid': 31, 'rounds': [],
                           'started_monotonic_ns': NS, 'completed_monotonic_ns': 100 * NS}
        for i in (1, 2, 3):
            value = producer(self.spec)
            value['elapsed_s'] = 23.8
            for row in [*value['prerequisites'], *value['records']]:
                row['witness_sha256'] = self.proof['wrappers'][row['cell']]['sha256']
            directory = self.root / f'collection/round-{i}'
            directory.mkdir(parents=True)
            self.collection['rounds'].append(save_json(directory / 'records.json', value))
        self.raw = {}
        for key in discovery.EVIDENCE:
            self.raw[key] = save_json(self.root / (key + '.json'), {'evidence': key})
        scheduler = [{'kind': 'snapshot', 'stage': 'before' if i == 0 else 'periodic',
                      'monotonic_ns': i * NS} for i in range(0, 81, 10)]
        scheduler += [{'kind': 'error', 'error': 'qstat unavailable'},
                      {'kind': 'snapshot', 'stage': 'after', 'monotonic_ns': 110 * NS}]
        self.write_rows('scheduler', scheduler)
        command = [self.plan['harness'], self.plan['plugins']['baseline'], '--replay',
                   str(self.root / 'collection/round-3/input-99-12.case'),
                   str(self.root / 'collection/round-3/current.json')]
        self.process = {'kind': 'snapshot', 'monotonic_ns': 75 * NS, 'finished_monotonic_ns': 76 * NS,
                        'processes': [{'pid': 31, 'ppid': 1, 'start': 'owner-start', 'command': 'python collect'},
                                      {'pid': 47, 'ppid': 31, 'start': 'child-start', 'command': ' '.join(command)}]}
        self.write_rows('process_observations', [self.process])
        def rows(path):
            for line in Path(path).read_text().splitlines():
                yield json.loads(line)
        self.host = SimpleNamespace(SLOTS=28, rows=rows, parsed_processes=lambda r: r,
                                    stable_pair=Mock(), boundary_pair=Mock(return_value=[{}, {}]),
                                    check_configuration=Mock(return_value='queue@test'),
                                    accounting=Mock(return_value={'failed': 0, 'exit_status': 0}),
                                    process_facts=Mock(return_value=[]), check_scheduler_snapshot=Mock(),
                                    check_scheduler=Mock(side_effect=ValueError('scheduler observer error')))

    def write_rows(self, key, values):
        path = Path(self.raw[key]['path'])
        path.write_text(''.join(json.dumps(v) + '\n' for v in values))
        self.raw[key] = discovery.reference(path)

    def certify(self):
        with patch.object(discovery, 'validate_plan', return_value=self.spec), \
             patch.object(discovery, 'checked_wrappers'), \
             patch.object(discovery, 'validate_prerequisite_identity'), \
             patch.object(window, 'source_semantics', return_value={'reviewed': h('source')}), \
             patch.object(discovery, 'score', side_effect=AssertionError('no ranking permitted')):
            return window._certify(self.plan, self.proof, self.collection, self.raw, discovery, self.host)

    def test_both_independent_bounds_certify_only_whole_first_two_rounds(self):
        value = self.certify()
        self.assertEqual(value['kind'], window.KIND)
        self.assertEqual(value['rounds'], self.collection['rounds'][:2])
        self.assertEqual(value['window']['completed_monotonic_ns'], 76 * NS)
        self.assertEqual(value['bounds']['elapsed_round3']['completed_monotonic_ns'], 78 * NS)
        self.assertEqual(value['scheduler_prefix']['completed_monotonic_ns'], 80 * NS)
        self.assertEqual(value['raw_evidence'], self.raw)
        self.assertEqual(value['outside_window_errors'][0]['observation']['error'], 'qstat unavailable')
        self.assertEqual(value['original_protocol_status'], 'failed')
        self.assertFalse(value['promotion'])
        for key in ('ranking', 'shortlist', 'timings', 'ratios'):
            self.assertNotIn(key, value)
        self.host.process_facts.assert_called_once_with(Path(self.raw['process_observations']['path']),
                                                       self.plan, self.collection)
        self.assertEqual(self.host.check_configuration.call_count, 2)
        self.host.accounting.assert_called_once()

    def test_elapsed_bound_uses_conservative_floor_and_collection_end(self):
        bound = window.elapsed_bound(self.collection, {'elapsed_s': 23.999999})
        self.assertEqual(bound['conservative_duration_ns'], 22 * NS)
        self.assertEqual(bound['completed_monotonic_ns'], 78 * NS)
        for bad in (None, -1, 1.9, float('nan'), float('inf'), True, 1000):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                window.elapsed_bound(self.collection, {'elapsed_s': bad})

    def test_child_attestation_requires_actual_direct_child_and_both_round_paths(self):
        for change in ('ppid', 'harness', 'input', 'output', 'plugin', 'missing'):
            row = copy.deepcopy(self.process)
            child = row['processes'][1]
            if change == 'ppid':
                child['ppid'] = 999
            elif change == 'missing':
                row['processes'].pop()
            else:
                index = {'harness': 0, 'plugin': 1, 'input': 3, 'output': 4}[change]
                args = child['command'].split()
                args[index] = '/unrelated/path'
                child['command'] = ' '.join(args)
            self.write_rows('process_observations', [row])
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'direct-child'):
                self.certify()

    def test_prefix_cannot_skip_error_gap_or_missing_bracket(self):
        original = list(self.host.rows(self.raw['scheduler']['path']))
        for change in ('error', 'gap', 'lower', 'upper', 'second_bound', 'missing_clock'):
            rows = copy.deepcopy(original)
            if change == 'error':
                rows[3] = {'kind': 'error', 'error': 'clockless interior error'}
            elif change == 'gap':
                rows = [rows[0], {'kind': 'snapshot', 'monotonic_ns': 80 * NS}, *rows[-2:]]
            elif change == 'lower':
                rows[0]['monotonic_ns'] = 2 * NS
            elif change == 'upper':
                rows = rows[:5]
            elif change == 'missing_clock':
                del rows[3]['monotonic_ns']
            else:
                # The tighter 76s bound alone is insufficient: both must bracket.
                rows = [*rows[:8], {'kind': 'snapshot', 'monotonic_ns': 76 * NS}]
            self.write_rows('scheduler', rows)
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.certify()

    def test_all_three_scientific_rounds_are_checked_including_excluded_third(self):
        for index in (0, 1, 2):
            ref = self.collection['rounds'][index]
            record = json.loads(Path(ref['path']).read_text())
            original = copy.deepcopy(record)
            record['records'][0]['science_sha256'] = h('changed science')
            self.collection['rounds'][index] = save_json(Path(ref['path']), record)
            with self.subTest(round=index + 1), self.assertRaises(ValueError):
                self.certify()
            self.collection['rounds'][index] = save_json(Path(ref['path']), original)
        ref = self.collection['rounds'][2]
        record = json.loads(Path(ref['path']).read_text())
        record['records'][0].update(status='failed', failure={'kind': 'timeout'})
        self.collection['rounds'][2] = save_json(Path(ref['path']), record)
        with self.assertRaises(ValueError):
            self.certify()

    def test_full_process_accounting_and_configuration_prerequisites_are_mandatory(self):
        for key in ('process_facts', 'accounting', 'check_configuration', 'stable_pair', 'boundary_pair'):
            method = getattr(self.host, key)
            original = method.side_effect
            method.side_effect = ValueError('bad full evidence')
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'bad full evidence'):
                self.certify()
            method.side_effect = original
        self.host.process_facts.return_value = [{'competing': 'compute'}]
        with self.assertRaisesRegex(ValueError, 'competing compute'):
            self.certify()

    def test_original_success_cannot_be_relabelled_as_failed(self):
        self.host.check_scheduler.side_effect = None
        with self.assertRaisesRegex(ValueError, 'did not fail'):
            self.certify()

    def test_original_key_error_from_clockless_late_error_is_retained(self):
        self.host.check_scheduler.side_effect = KeyError('monotonic_ns')
        value = self.certify()
        self.assertEqual(value['original_protocol_status'], 'failed')
        self.assertEqual(value['original_protocol_error_type'], 'KeyError')
        self.assertEqual(value['outside_window_errors'][0]['observation'],
                         {'kind': 'error', 'error': 'qstat unavailable'})

    def test_modified_source_and_unfrozen_generator_are_rejected_before_import(self):
        generator = self.root / 'scc_routing_host.py'
        generator.write_text('raise AssertionError("must never import changed source")\n')
        ref = discovery.reference(generator)
        plan = {'receipt_generator': ref, 'frozen_files': {str(generator): ref['sha256']}}
        for name in ('routing_discovery.py', 'routing_run.py'):
            (self.root / name).write_text('# wrong original source\n')
        with self.assertRaisesRegex(ValueError, 'reviewed original source'):
            window.original_modules(plan)
        plan['frozen_files'] = {}
        with self.assertRaisesRegex(ValueError, 'not frozen'):
            window.certify(plan, {}, {}, {})

    def test_reviewed_09f7_source_contract_matches_original_frozen_metadata(self):
        # Compact fixture of the original freeze, independent of the current
        # checkout and git executable. The reviewed old producer starts its
        # monotonic duration inside invoke and stores elapsed_s in finally before
        # returning. These exact whole-source identities bind that clock proof.
        frozen = {
            '/original/bench/routing_discovery.py': '232a46e6bef4a69a12a9a9eef1783c364a4df809ff28ce25b493f91631d06a59',
            '/original/bench/routing_run.py': 'e720375050281f609f02eef2e894fbb2fd8300a4b39a0bfbfb8de986b9ac7e15',
            '/original/bench/scc_routing_host.py': '8ea1e9b3361a3c161afb409e5b879c9354da819bf26773b6b9031188bf90e087',
        }
        refs = lambda path: {'path': path, 'sha256': frozen[path]}
        old = SimpleNamespace(__file__='/original/bench/routing_discovery.py', reference=refs,
                              routing_run=SimpleNamespace(__file__='/original/bench/routing_run.py'))
        host = SimpleNamespace(__file__='/original/bench/scc_routing_host.py', check_sources=Mock())
        plan = {'frozen_files': dict(frozen)}
        self.assertEqual(window.source_semantics(plan, old, host), frozen)
        host.check_sources.assert_called_once_with(plan)
        plan['frozen_files']['/original/bench/routing_run.py'] = h('different producer')
        with self.assertRaisesRegex(ValueError, 'reviewed original source'):
            window.source_semantics(plan, old, host)

    def test_driver_imports_no_current_scientific_modules(self):
        program = ('import runpy,sys; runpy.run_path(sys.argv[1],run_name="window_test"); '
                   'assert not ({"routing","routing_run","routing_discovery","scc_routing_host"} & sys.modules.keys())')
        result = subprocess.run([sys.executable, '-c', program, window.__file__], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_raw_reference_changes_and_certificate_forgery_fail(self):
        Path(self.raw['accounting']['path']).write_text('{"modified":true}')
        with self.assertRaisesRegex(ValueError, 'frozen file changed'):
            self.certify()
        certificate = {'collection': routing.seal(self.collection), 'raw_evidence': self.raw, 'status': 'forged'}
        with patch.object(window, 'certify', return_value={**certificate, 'status': 'verified'}):
            with self.assertRaisesRegex(ValueError, 'differs'):
                window.validate(self.plan, self.proof, certificate)


if __name__ == '__main__':
    unittest.main()
