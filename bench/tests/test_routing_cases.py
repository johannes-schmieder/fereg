"""Fixture shapes and input preparation for the single qualification round."""
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing_cases
from cmgschur_cases import additional_case, dense_oracle
from native_io import case_header


class RoutingCases(unittest.TestCase):
    def test_imported_family_coverage_checks_canonical_terms_not_filenames(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / 'claimed-slopes.case'
            for family, count, term in [('no-fe', 0, [0., 0., 0., 0.]),
                                         ('one-fe', 1, [1., 1., 1., 0.]),
                                         ('slopes', 1, [1., 1., 1., 1.])]:
                args = [b'estimate-ols', b'threads=1', b'output_saved_channels=1:2', f'absorb_term_count={count}'.encode(),
                        b'absorb_term_matrix=A'] + [f'output_{name}=1'.encode() for name in
                        ['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed']]
                prefix = f'FEREG_REPLAY_V1\n2 2 {len(args)} 1 0\n'.encode()
                framed = b''.join(str(len(a)).encode() + b'\n' + a + b'\n' for a in args)
                data = b'0\n' + b'0000000000000000\n' * 2
                table = b'1\nA\n1 4\n' + b''.join(struct.pack('>d', v).hex().encode() + b'\n' for v in term)
                path.write_bytes(prefix + framed + data * 2 + table)
                inspected = routing_cases.inspect_case(path, family)
                self.assertEqual(inspected['absorbed_terms'], count)
                self.assertEqual(inspected['saved'], [1])
                for wrong in set(routing_cases.REQUIRED_IMPORTS) - {family}:
                    with self.subTest(actual=family, claimed=wrong), self.assertRaises(ValueError):
                        routing_cases.inspect_case(path, wrong)


    def test_extended_families_have_the_declared_mathematical_structure(self):
        columns, weights, count, intercept = routing_cases.extended_columns('disconnected', 19, 300, 100, 20)
        worker, firm = columns[-2:]
        self.assertTrue(np.array_equal(worker < 150, firm < 50))
        self.assertEqual((weights, count, intercept), (None, 2, True))
        columns, weights, count, _ = routing_cases.extended_columns('near-confounded-third', 19, 300, 100, 20)
        mismatch = np.mean(columns[-1] != columns[-2] % 20)
        self.assertGreater(mismatch, 0.)
        self.assertLess(mismatch, .02)
        self.assertEqual(count, 3)
        _, weights, _, _ = routing_cases.extended_columns('weight-dynamic', 19, 300, 100, 20)
        self.assertTrue(np.all(np.isfinite(weights)) and np.all(weights > 0))
        self.assertGreater(weights.max() / weights.min(), 1e6)
        columns, weights, count, intercept = routing_cases.extended_columns('inactive-columns', 19, 300, 100, 20)
        for values in columns[:-2]:
            self.assertTrue(np.array_equal(values.reshape(-1, 2).sum(axis=1), np.zeros(len(values)//2)))
        self.assertFalse(intercept)


    def test_paired_construction_witness_agrees_with_independent_dense_dummies(self):
        columns = additional_case('orthogonal', 50, 20, 12, 41)
        witness = routing_cases.paired_orthogonal_oracle(columns)
        dense = dense_oracle(columns, include_intercept=False)
        self.assertEqual(witness['rank'], dense['rank'])
        for name in ['beta', 'covariance', 'residual', 'fitted']:
            np.testing.assert_allclose(witness[name], dense[name], rtol=1e-9, atol=1e-11)
        columns[0][0] += 1e-10
        with self.assertRaisesRegex(ValueError, 'not exact'):
            routing_cases.paired_orthogonal_oracle(columns)


    def test_complete_coverage_and_explicit_distinct_imports(self):
        families = (set(routing_cases.NATIVE_CASES) | set(routing_cases.SYNTHETIC_CASES)
                    | set(routing_cases.REQUIRED_IMPORTS) | set(routing_cases.SAMPLED_CASES)
                    | set(routing_cases.EXTENDED_CASES) | {'low-saved'})
        self.assertEqual(len(families), 21)
        with self.assertRaisesRegex(ValueError, 'require distinct'):
            routing_cases.sources([])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            arguments = []
            for name in routing_cases.REQUIRED_IMPORTS:
                path = root / name
                path.write_text('input-' + name)
                arguments.append(f'{name}={path}')
            self.assertEqual(set(routing_cases.sources(arguments)), set(routing_cases.REQUIRED_IMPORTS))
            with self.assertRaisesRegex(ValueError, 'exactly once'):
                routing_cases.sources(arguments + arguments[:1])
            with self.assertRaisesRegex(ValueError, 'distinct'):
                routing_cases.sources([f'{name}={root / "no-fe"}' for name in routing_cases.REQUIRED_IMPORTS])

    def test_small_high_degree_fixtures_keep_distinct_employer_requirements(self):
        for kind, sizes in routing_cases.SMALL_HIGH_DEGREE.items():
            columns = routing_cases.high_degree_case(kind, *sizes, seed=18)
            summary = routing_cases.graph_summary(columns)
            self.assertLess(summary['rows'], 10000)
            self.assertGreaterEqual(max(map(int, summary['degree_histogram'])), 60 if kind == 'd60-stress' else 30)

    def test_low_saved_copy_changes_only_the_memory_request(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, target = root / 'source', root / 'target'
            args = [b'estimate-ols', b'memory=fast', b'output_saved_channels=1:2', b'threads=1']
            prefix = f'FEREG_REPLAY_V1\n2 2 {len(args)} 1 0\n'.encode()
            source.write_bytes(prefix + b''.join(str(len(a)).encode() + b'\n' + a + b'\n' for a in args) + b'body')
            original = source.read_bytes()
            routing_cases.copy_import(source, target, low=True)
            self.assertEqual(source.read_bytes(), original)
            with target.open('rb') as stream:
                _, actual = case_header(stream)
                self.assertEqual(actual, [b'estimate-ols', b'memory=low', *args[2:]])
                self.assertEqual(stream.read(), b'body')
            source.write_bytes(original.replace(b'11\nmemory=fast', b'11\nnotmem=fast'))
            with self.assertRaisesRegex(ValueError, 'canonical memory'):
                routing_cases.copy_import(source, root / 'bad', low=True)

    def test_seed_is_required_and_no_stage_or_comparator_flags_remain(self):
        with patch.object(sys, 'argv', ['routing_cases.py', '--output', 'unused']), \
                patch('sys.stderr'), self.assertRaises(SystemExit) as caught:
            routing_cases.main()
        self.assertEqual(caught.exception.code, 2)
        with patch.object(sys, 'argv', ['routing_cases.py', '--help']), \
                patch('sys.stdout') as output, self.assertRaises(SystemExit):
            routing_cases.main()
        help_text = ''.join(call.args[0] for call in output.write.call_args_list)
        self.assertNotIn('--panel', help_text)
        self.assertNotIn('--compact-it', help_text)
        self.assertIn('--small', help_text)

    def test_seed_and_full_family_set_are_written_once_without_comparators(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            generator, low = root / 'generator', root / 'low'
            generator.write_text('mock generator')
            low.write_text('mock saved fixture')
            output = root / 'inputs'
            args = ['routing_cases.py', '--seed', '987654321', '--output', str(output),
                    '--generator', str(generator), '--low-saved', str(low)]
            # Exercise the real CLI/file schema; numerical fixture construction
            # is checked independently above and does not need native binaries.
            def generate(command, **kwargs):
                Path(command[command.index('--output') + 1]).write_text('mock case')
            def copy(source, destination, low=False):
                destination.write_text('mock case')
            def write(generator, destination, columns, **kwargs):
                destination.write_text('mock case')
            metadata = {'body_sha256': 'b' * 64, 'absorbed_terms': 2, 'categorical_intercepts': True}
            with patch.object(sys, 'argv', args), patch('builtins.print'), \
                    patch.object(routing_cases, 'sources', return_value={name: low for name in routing_cases.REQUIRED_IMPORTS}), \
                    patch.object(routing_cases, 'scratch', return_value=output), \
                    patch.object(routing_cases, 'copy_import', side_effect=copy), \
                    patch.object(routing_cases, 'digest', return_value='a' * 64), \
                    patch.object(routing_cases, 'inspect_case', return_value=metadata), \
                    patch.object(routing_cases, 'case_header', return_value=([2, 2, 1, 0, 0], [b'estimate-ols', b'output_saved_channels=1:2'])), \
                    patch.object(routing_cases, 'extended_columns', return_value=([np.ones(2)] * 5, None, 2, True)), \
                    patch.object(routing_cases, 'synthetic', return_value=[np.ones(2)] * 5), \
                    patch.object(routing_cases, 'additional_case', return_value=[np.ones(2)] * 5), \
                    patch.object(routing_cases, 'high_degree_case', return_value=[np.ones(2)] * 5), \
                    patch.object(routing_cases, 'write_case', side_effect=write), \
                    patch.object(routing_cases.subprocess, 'run', side_effect=generate) as native:
                routing_cases.main()
            panel = json.loads((output / 'panel.json').read_text())
            self.assertEqual(panel['seed'], 987654321)
            self.assertEqual(len(panel['cells']), 42)
            self.assertFalse(panel['small'])
            self.assertFalse(any('comparators' in cell for cell in panel['cells']))
            self.assertEqual(json.loads((output / 'dense.json').read_text())['seed'], 987654321)
            for call in native.call_args_list:
                command = call.args[0]
                self.assertEqual(command[command.index('--seed') + 1], '987654321')


if __name__ == '__main__':
    unittest.main()
