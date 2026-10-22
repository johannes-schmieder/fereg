"""Synthetic preparation guards; generation/native calls are always mocked."""
import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import two_fe_policy_inputs as inputs
from native_io import case_header, digest


def write_case(path, token=1, *, solver='auto', duplicate=False):
    """Tiny numeric framing/model fixture, not a substitute calibration input."""
    args = [b'estimate-ols', b'threads=1', b'memory=fast', f'solver={solver}'.encode(),
            b'preconditioner=auto', b'absorb_term_count=2', b'absorb_term_matrix=A',
            b'output_saved_channels=1:0|1:0']
    args += [f'output_{name}=1'.encode() for name in
             ('sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed')]
    if duplicate:
        args.append(b'solver=auto')
    raw = f'FEREG_REPLAY_V1\n4 2 {len(args)} 1 0\n'.encode()
    raw += b''.join(str(len(arg)).encode() + b'\n' + arg + b'\n' for arg in args)
    encode = lambda number: struct.pack('>d', number).hex().encode() + b'\n'
    raw += (b'0\n' + b''.join(encode(float(token + i)) for i in range(4))) * 2
    raw += b'1\nA\n2 4\n' + b''.join(encode(v) for v in (1, 1, 1, 0, 2, 1, 1, 0))
    path.write_bytes(raw)


class InputPreparation(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.output = self.root / 'panel'
        self.generator = self.root / 'replay_large'
        self.generator.write_bytes(b'synthetic executable identity; never executed')
        self.generated = []
        self.reference_calls = []
        self.addCleanup(patch.stopall)
        patch.object(inputs.routing_cases, 'scratch', side_effect=lambda path: path.resolve()).start()
        patch.object(inputs, 'source_hashes', return_value={'synthetic.py': 'a' * 64}).start()
        patch.object(inputs.importlib.metadata, 'version', return_value='synthetic').start()
        patch.object(inputs, '_generate', side_effect=self.generate).start()
        patch.object(inputs.routing_oracle, 'write_reference', side_effect=self.reference).start()
        patch.object(inputs.routing_oracle, 'load_reference', side_effect=self.load_reference).start()
        patch.object(inputs.routing_cases, 'inactive_case_oracle', return_value={
            'method': 'synthetic dispatch witness', 'rank': 7, 'components': 1, 'regressors': 4}).start()

    def generate(self, generator, path, family, seed):
        self.generated.append((generator, family, seed))
        write_case(path, token=len(self.generated))
        return {'generator_options': {'synthetic': True}}

    def reference(self, case, output):
        self.reference_calls.append(case)
        npz = output.with_suffix('.npz')
        npz.write_bytes(b'synthetic reference arrays ' + case.name.encode())
        result = {'schema': 1, 'case_sha256': digest(case),
                  'body_sha256': inputs.routing_cases.inspect_case(case)['body_sha256'],
                  'arrays': {'file': npz.name, 'sha256': digest(npz)},
                  'untouched_science': [2, 3, 5]}
        output.write_text(json.dumps(result))
        return result

    def load_reference(self, path, case_hash, body_hash):
        value = json.loads(path.read_text())
        self.assertEqual((value['case_sha256'], value['body_sha256']), (case_hash, body_hash))
        self.assertEqual(digest(path.parent / value['arrays']['file']), value['arrays']['sha256'])
        return value, {}

    def prepare(self):
        return inputs.prepare(self.output, generator=self.generator)

    def test_development_uses_only_its_fixed_seed_and_same_independent_pairing(self):
        import two_fe_policy_development as development
        panel = inputs.prepare(self.output, generator=self.generator, development=True)
        self.assertEqual((panel['input_count'], panel['case_count']), (8, 16))
        self.assertEqual(panel['seeds'], [2026104201])
        self.assertEqual([c['id'] for c in panel['cases']], list(development.CASE_IDS))
        self.assertEqual(len(self.reference_calls), 7)
        self.assertTrue(all(seed == 2026104201 for _, _, seed in self.generated))
        for item in panel['cases']:
            development.validate_case(self.output / item['input'], item)
        self.assertEqual(inputs.policy.SEEDS, (2026102201, 2026103201))
        self.assertFalse(any('2026105201' in name for name in panel['files']))

    def test_exact_schedule_body_pairing_and_compatible_config_entries(self):
        panel = self.prepare()
        self.assertEqual([item['id'] for item in panel['cases']], list(inputs.policy.CASE_IDS))
        self.assertEqual((panel['input_count'], panel['case_count']), (16, 32))
        self.assertEqual([(family, seed) for _, family, seed in self.generated],
                         [(family, seed) for seed in inputs.policy.SEEDS for family in inputs.policy.FAMILIES])
        self.assertEqual(len(self.reference_calls), 14)  # Once per non-inactive input, not per memory mode.
        for item in panel['cases']:
            self.assertEqual(set(item), {'id', 'family', 'input', 'oracle', 'memory_limit_bytes'})
            self.assertIsNone(item['memory_limit_bytes'])
            source = self.output / item['input']
            meta = json.loads(source.with_suffix('.json').read_text())
            inputs.policy.validate_case(source, item)
            self.assertEqual(meta['case_sha256'], digest(source))
            self.assertEqual(meta['generator_sha256'], digest(self.generator))
            self.assertEqual(meta['generator_sources_sha256'], inputs.routing.identity(panel['sources']))
            self.assertEqual(meta['random_streams'], inputs.policy.streams(meta['seed'], item['family']))
            self.assertFalse(meta['small'])
            self.assertNotIn('artifacts', panel)
        for name, expected in panel['files'].items():
            self.assertEqual(digest(self.output / name), expected)
        self.assertEqual(json.loads((self.output / 'panel.json').read_text()), panel)

    def test_low_reference_changes_only_transport_binding(self):
        panel = self.prepare()
        for fast, low in zip(panel['cases'][::2], panel['cases'][1::2]):
            if fast['oracle']['kind'] == 'inactive':
                self.assertEqual(low['oracle'], {'kind': 'inactive'})
                continue
            a_path = self.output / fast['oracle']['path']
            a = json.loads(a_path.read_text())
            b = json.loads((self.output / low['oracle']['path']).read_text())
            self.assertEqual(b.pop('memory_pair_reference_sha256'), digest(a_path))
            self.assertEqual(b.pop('case_sha256'), digest(self.output / low['input']))
            a.pop('case_sha256')
            self.assertEqual(a, b)

    def test_existing_output_and_generator_failure_do_not_resume(self):
        self.output.mkdir()
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.prepare()
        self.assertEqual(self.generated, [])
        self.output.rmdir()
        with patch.object(inputs, '_generate', side_effect=RuntimeError('generator failed')):
            with self.assertRaisesRegex(RuntimeError, 'generator failed'):
                self.prepare()
        self.assertTrue(self.output.is_dir())
        self.assertFalse((self.output / 'panel.json').exists())
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.prepare()

    def test_source_or_binary_drift_blocks_final_panel(self):
        with patch.object(inputs, 'source_hashes', side_effect=[{'a': '1'}, {'a': '2'}]):
            with self.assertRaisesRegex(ValueError, 'drifted'):
                self.prepare()
        self.assertTrue(any(self.output.glob('*.case')))
        self.assertFalse((self.output / 'panel.json').exists())

    def test_binary_drift_is_not_hidden_by_stable_source_hash(self):
        original = self.generate
        def changed(*args):
            result = original(*args)
            self.generator.write_bytes(b'changed')
            return result
        with patch.object(inputs, '_generate', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'drifted'):
                self.prepare()
        self.assertFalse((self.output / 'panel.json').exists())

    def test_body_collision_or_missing_reference_keeps_failure(self):
        with patch.object(inputs, '_generate', side_effect=lambda gen, path, family, seed:
                          (write_case(path, 1) or {})):
            with self.assertRaisesRegex(ValueError, 'bodies collide'):
                self.prepare()
        self.assertFalse((self.output / 'panel.json').exists())

    def test_oracle_failure_is_not_replaced_or_skipped(self):
        with patch.object(inputs.routing_oracle, 'write_reference', side_effect=ValueError('rank failed')):
            with self.assertRaisesRegex(ValueError, 'rank failed'):
                self.prepare()
        self.assertEqual(len(self.generated), 1)
        self.assertFalse((self.output / 'panel.json').exists())

    def test_auto_and_unique_options_are_required_without_silent_repair(self):
        for index, options in enumerate(({'solver': 'map'}, {'duplicate': True})):
            path = self.root / f'bad-{index}.case'
            write_case(path, **options)
            with self.assertRaises(ValueError):
                inputs._pair(path, self.root / f'bad-{index}-low.case', 'dense')

    def test_copy_tampering_is_detected_by_actual_body_hash(self):
        fast, low = self.root / 'fast.case', self.root / 'low.case'
        write_case(fast)
        original = inputs.routing_cases.copy_import
        def tamper(source, target, low=False):
            original(source, target, low=low)
            raw = target.read_bytes()
            target.write_bytes(raw.replace(b'3ff0000000000000\n', b'4000000000000000\n', 1))
        with patch.object(inputs.routing_cases, 'copy_import', side_effect=tamper):
            with self.assertRaisesRegex(ValueError, 'model/body mismatch'):
                inputs._pair(fast, low, 'dense')


class FrozenDefinitions(unittest.TestCase):
    def test_random_streams_are_separated_by_stage_not_claimed_independent_within_seed(self):
        plan = inputs.stream_plan()
        seed = inputs.policy.SEEDS[0]
        self.assertEqual(plan[str(seed)]['disconnected'], [seed, seed + 1])
        self.assertEqual(plan[str(seed)]['wide-rhs'], [seed, seed + 1])
        self.assertEqual(plan[str(seed)]['weight-dynamic'], [seed, seed + 17])
        with patch.object(inputs.policy, 'streams', return_value=[17]):
            with self.assertRaisesRegex(ValueError, 'streams collide'):
                inputs.stream_plan()

    def test_sources_cover_generators_serializer_parser_and_reference(self):
        paths = inputs.source_files()
        for name in ('bench/two_fe_policy_inputs.py', 'bench/routing_cases.py',
                     'bench/cmgschur_cases.py', 'bench/native_io.py', 'bench/routing_oracle.py',
                     'crates/fereg-oracle/src/designs/sample.rs',
                     'crates/fereg-oracle/src/designs/topology.rs',
                     'crates/fereg-oracle/examples/replay/wire.rs',
                     'crates/fereg-core/src/metadata.rs', 'Cargo.lock'):
            self.assertIn(name, paths)
        self.assertTrue(all(path.is_file() for path in paths.values()))

    def test_dense_dispatch_retains_full_maintained_arguments(self):
        with patch.object(inputs.subprocess, 'run') as run:
            details = inputs._generate(Path('/explicit/generator'), Path('/explicit/case'), 'dense', 2026102201)
        args = run.call_args.args[0]
        values = dict(zip(args[1::2], args[2::2]))
        self.assertEqual(details['generator_options'], inputs.routing_cases.NATIVE_CASES['dense'])
        self.assertEqual(values['--seed'], '2026102201')
        self.assertEqual(values['--rows'], '100000')
        self.assertEqual(values['--levels'], '500,100')
        self.assertEqual(values['--solver'], 'auto')
        self.assertEqual(values['--tolerance'], '1e-8')
        self.assertEqual(run.call_args.kwargs['timeout'], 120)

    def test_python_dispatch_uses_unchanged_full_generators(self):
        columns = [[0.], [0.], [0.], [0.], [0.]]
        with patch.object(inputs.cmgschur_cases, 'high_degree_case', return_value=columns) as high, \
             patch.object(inputs.cmgschur_cases, 'synthetic', return_value=columns) as synthetic, \
             patch.object(inputs.cmgschur_cases, 'additional_case', return_value=columns) as additional, \
             patch.object(inputs.routing_cases, 'extended_columns', return_value=(columns, None, 2, True)) as extended, \
             patch.object(inputs.cmgschur_cases, 'write_case') as write, \
             patch.object(inputs.cmgschur_cases, 'graph_summary', return_value={}):
            for family in inputs.policy.FAMILIES[1:]:
                inputs._generate(Path('generator'), Path('output'), family, 2026102201)
        high.assert_called_once_with('d30-weak', 10000, 1000, 60, seed=2026102201)
        self.assertEqual([call.args for call in synthetic.call_args_list],
                         [('weak', 5000, 500, 20), ('long', 5000, 500, 20)])
        additional.assert_called_once_with('wide', 5000, 500, 20, seed=2026102201)
        self.assertEqual([call.args for call in extended.call_args_list],
                         [(family, 2026102201) for family in ('disconnected', 'weight-dynamic', 'inactive-columns')])
        self.assertEqual(write.call_count, 7)


if __name__ == '__main__':
    unittest.main()
