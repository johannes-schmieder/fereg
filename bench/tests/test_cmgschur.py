"""CMG-Schur benchmark witnesses: topology, transport and scientific guards."""
import copy
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cmgschur
import cmgschur_cases
from native_io import case_header


def bits(value):
    return struct.pack('>d', value).hex()


def result(arm='auto'):
    arm = cmgschur.solver_arm(arm)
    shapes = {'RB': (1, 1), 'RV': (1, 1), 'RSTATUS': (1, 4), 'RFIT': (12, 2),
              'RDOF': (20, 2), 'RCERT': (32, 2), 'RSOLVER': (2, 16), 'RRUNTIME': (30, 2),
              'RFE': (2, 8), 'RCLUSTER': (1, 4), 'RSUBSET': (1, 4), 'RMARKB': (1, 1),
              'RMARKV': (1, 1), 'RREC': (1, 16), 'RMEMORY': (11, 2)}
    tables = {name: {'rows': r, 'columns': c, 'bits': [bits(1.)] * (r*c)}
              for name, (r, c) in shapes.items()}
    solver = tables['RSOLVER']['bits']
    for row in range(2):
        solver[row*16] = bits(cmgschur.EXPECTED_SOLVER[arm])
        solver[row*16+1] = bits(4 if arm == 'auto' else cmgschur.EXPECTED_SOLVER[arm])
    return {'schema': 1, 'return_code': 0, 'matrices': tables, 'error': ''}


def case_bytes(arguments=None):
    arguments = arguments or [b'estimate-ols', b'threads=1', b'solver=auto', b'preconditioner=auto']
    return f'FEREG_REPLAY_V1\n2 6 {len(arguments)} 1 0\n'.encode() + b''.join(
        str(len(a)).encode() + b'\n' + a + b'\n' for a in arguments) + b'body\n9\nthreads=1\n'


class CmgSchurBenchmark(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.meta = {'rows': 2, 'columns': 6,
                     'outputs': dict(zip(['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed'], range(6)))}
        self.data = np.array([[1., 1.], [.2, -.2], [0., 0.], [.3, .4], [.5, .2], [.2, -.2]])
        self.left, self.right = self.root / 'left', self.root / 'right'
        self.data.tofile(self.left)
        self.data.tofile(self.right)

    def test_long_and_weak_have_cutoff_discrimination_and_duplicate_links(self):
        for kind in ['long', 'weak']:
            columns = cmgschur_cases.synthetic(kind, 150, 100, 20)
            summary = cmgschur_cases.graph_summary(columns)
            for degree in ['4', '5', '6', '7', '10']:
                self.assertGreater(summary['degree_histogram'][degree], 0)
            self.assertLess(summary['distinct_links'], summary['rows'])
            self.assertEqual(summary['workers'], 150)
            self.assertEqual(summary['firms'], 100)
        short = cmgschur_cases.graph_summary(cmgschur_cases.synthetic('short', 50, 20, 2))
        self.assertEqual(set(short['degree_histogram']), {'1', '2'})

    def test_source_identity_records_git_head_and_actual_dirty_flag(self):
        for dirty in ['', ' M source.rs\n']:
            replies = [str(self.root), 'a' * 40, dirty]
            with patch.object(cmgschur.subprocess, 'run', side_effect=[
                    subprocess.CompletedProcess([], 0, stdout=value) for value in replies]):
                identity = cmgschur.source_identity(self.root)
            self.assertEqual(identity['git'], {'available': True, 'head': 'a' * 40, 'dirty': bool(dirty)})
            self.assertEqual(set(identity['tool_sha256']), {'cmgschur.py', 'cmgschur_cases.py'})
            self.assertTrue(all(len(value) == 64 for value in identity['tool_sha256'].values()))

    def test_archived_source_declaration_does_not_claim_verified_clean_git(self):
        declaration = {'revision': 'a' * 40, 'dirty': False, 'source_tar_sha256': 'b' * 64}
        (self.root / '.fereg-source.json').write_text(json.dumps(declaration))
        with patch.object(cmgschur.subprocess, 'run', side_effect=FileNotFoundError('git')):
            identity = cmgschur.source_identity(self.root)
        self.assertEqual(identity['git'], {'available': False, 'head': None, 'dirty': None})
        self.assertEqual(identity['archive_manifest']['declaration'], declaration)
        self.assertFalse(identity['archive_manifest']['verified_against_checkout'])
        with patch.object(cmgschur.subprocess, 'run', return_value=subprocess.CompletedProcess(
                [], 0, stdout=str(self.root.parent))):
            nested = cmgschur.source_identity(self.root)
        self.assertEqual(nested, identity)

    def test_unavailable_git_without_manifest_leaves_identity_unknown(self):
        with patch.object(cmgschur.subprocess, 'run', side_effect=subprocess.TimeoutExpired('git', 5)):
            identity = cmgschur.source_identity(self.root)
        self.assertEqual(identity['git'], {'available': False, 'head': None, 'dirty': None})
        self.assertNotIn('archive_manifest', identity)

    def test_dense_dummy_oracle_is_independent_and_stationary(self):
        columns = cmgschur_cases.synthetic('long', 50, 20, 12)
        target = cmgschur_cases.dense_oracle(columns)
        residual = np.array(target['residual'])
        self.assertEqual(target['rank'], 71)
        self.assertLess(abs(residual.sum()), 1e-9)
        for column in columns[1:3]:
            self.assertLess(abs(residual @ column), 1e-9)
        for ids in columns[-2:]:
            for code in np.unique(ids):
                self.assertLess(abs(residual[ids == code].sum()), 1e-9)
        self.assertGreater(np.linalg.eigvalsh(target['covariance']).min(), 0)

    def test_optional_fixtures_have_mixed_and_exactly_orthogonal_rhs(self):
        wide = cmgschur_cases.additional_case('wide', 50, 20, 12)
        self.assertEqual(len(wide) - 3, 16)
        orthogonal = cmgschur_cases.additional_case('orthogonal', 50, 20, 12)
        for columns, targets in [(wide, [wide[1+j] for j in range(0, 16, 4)]),
                                 (orthogonal, orthogonal[:-2])]:
            for values in targets:
                for ids in columns[-2:]:
                    sums = np.bincount(ids.astype(int), weights=values)
                    self.assertTrue(np.array_equal(sums, np.zeros_like(sums)))
        worker_sums = np.bincount(wide[-2].astype(int), weights=wide[2])
        firm_sums = np.bincount(wide[-1].astype(int), weights=wide[3])
        self.assertGreater(np.abs(worker_sums).max(), 1)
        self.assertGreater(np.abs(firm_sums).max(), 1)
        witness = cmgschur_cases.dense_oracle(orthogonal)
        self.assertEqual(witness['rank'], 73)
        self.assertGreater(np.linalg.eigvalsh(witness['covariance']).min(), 0)
        nocons = cmgschur_cases.dense_oracle(orthogonal, include_intercept=False)
        self.assertEqual(nocons['rank'], witness['rank'])
        for field in ['beta', 'covariance', 'fitted', 'residual']:
            np.testing.assert_allclose(nocons[field], witness[field], rtol=1e-10, atol=1e-12)

    def test_high_degree_fixtures_are_connected_repeated_and_reproducible(self):
        sizes = {'sampled-oracle': (60, 40, 50), 'd30-mixed': (80, 60, 60),
                 'd30-weak': (330, 300, 60), 'd60-stress': (100, 80, 120)}
        for kind, size in sizes.items():
            with self.subTest(kind=kind):
                columns = cmgschur_cases.high_degree_case(kind, *size)
                repeated = cmgschur_cases.high_degree_case(kind, *size)
                self.assertTrue(all(np.array_equal(a, b) for a, b in zip(columns, repeated)))
                summary = cmgschur_cases.graph_summary(columns)
                expected_degree = 60 if kind == 'd60-stress' else 30
                self.assertEqual(max(map(int, summary['degree_histogram'])), expected_degree)
                self.assertGreater(summary['degree_histogram'][str(expected_degree)], 0)
                self.assertEqual(summary['rows'], size[0] * size[2])
                self.assertLess(summary['distinct_links'], summary['rows'])
                self.assertEqual((summary['workers'], summary['firms']), size[:2])
                parent = list(range(size[1]))
                def find(i):
                    while parent[i] != i:
                        i = parent[i]
                    return i
                cross_group = 0
                for worker in range(size[0]):
                    firms = np.unique(columns[-1][columns[-2] == worker]).astype(int)
                    for firm in firms[1:]:
                        parent[find(int(firm))] = find(int(firms[0]))
                    if kind == 'd30-weak' and len(np.unique(firms // (size[1] // 10))) > 1:
                        cross_group += 1
                self.assertEqual(len({find(i) for i in range(size[1])}), 1)
                if kind == 'd30-weak':
                    self.assertEqual(cross_group, 9)
                if kind == 'sampled-oracle':
                    self.assertEqual(summary['degree_histogram'], {'2': 40, '30': 20})
                    oracle = cmgschur_cases.dense_oracle(columns)
                    self.assertEqual(oracle['rank'], 101)
                    residual = np.array(oracle['residual'])
                    for values in columns[1:3]:
                        self.assertLess(abs(residual @ values), 1e-9)
                    for ids in columns[-2:]:
                        self.assertLess(np.abs(np.bincount(ids.astype(int), weights=residual)).max(), 1e-9)

    def test_sampled_scratch_root_is_allowed_without_opening_other_task_roots(self):
        expected = cmgschur_cases.ROOT / 'target/tmp/cmgschur-sampled/cases'
        self.assertEqual(cmgschur_cases.scratch(expected), expected.resolve())
        with self.assertRaises(ValueError):
            cmgschur_cases.scratch(cmgschur_cases.ROOT / 'target/tmp/cmgschur-sampled-other/cases')

    def test_nocons_template_resizes_coefficient_buffers_only(self):
        shapes = {'RB': (1, 5), 'RV': (5, 5), 'RSTATUS': (5, 4),
                  'RMARKB': (1, 5), 'RMARKV': (5, 5), 'RFE': (2, 8)}
        raw = b''.join(f'{len(name)}\n{name}\n{rows} {cols}\n'.encode() +
                       (b'3ff0000000000000\n' if name == 'RFE' else b'0000000000000000\n') * (rows*cols)
                       for name, (rows, cols) in shapes.items())
        args, output = cmgschur_cases.noconstant_template([b'include_intercept=1'], raw, 4, len(shapes))
        self.assertEqual(args, [b'include_intercept=0'])
        stream = io.BytesIO(output)
        observed = {}
        for _ in shapes:
            length = int(stream.readline())
            name = stream.read(length).decode()
            self.assertEqual(stream.read(1), b'\n')
            rows, cols = map(int, stream.readline().split())
            observed[name] = (rows, cols)
            values = [stream.readline() for _ in range(rows*cols)]
            if name == 'RFE':
                self.assertEqual(values, [b'3ff0000000000000\n'] * 16)
        self.assertEqual(observed, {'RB': (1, 4), 'RV': (4, 4), 'RSTATUS': (4, 4),
                                    'RMARKB': (1, 4), 'RMARKV': (4, 4), 'RFE': (2, 8)})
        self.assertEqual(stream.read(), b'')

    def test_default_case_selection_is_unchanged_and_extra_inputs_are_optional(self):
        for extras in [[], ['wide', 'orthogonal'], list(cmgschur_cases.HIGH_DEGREE_SIZES)]:
            root = self.root / ('-'.join(extras) if extras else 'default')
            generator = self.root / 'generator'
            generator.write_bytes(b'generator')
            calls = []
            def data(kind, *sizes):
                calls.append((kind, sizes))
                return [np.array([1., 2.])] * 5
            def write(generator, path, columns, tolerance, include_intercept=True):
                self.assertEqual(include_intercept, path.stem != 'orthogonal')
                path.write_bytes(b'fixed-input')
                return self.meta['outputs']
            argv = ['cases', '--output', str(root), '--generator', str(generator)]
            if extras:
                argv += ['--extra-cases', *extras]
            with patch.object(sys, 'argv', argv), patch.object(cmgschur_cases, 'scratch', side_effect=lambda p: p), \
                    patch.object(cmgschur_cases, 'synthetic', side_effect=data), \
                    patch.object(cmgschur_cases, 'additional_case', side_effect=data), \
                    patch.object(cmgschur_cases, 'high_degree_case', side_effect=data), \
                    patch.object(cmgschur_cases, 'write_case', side_effect=write), \
                    patch.object(cmgschur_cases, 'dense_oracle', return_value={}), patch('builtins.print'):
                cmgschur_cases.main()
            self.assertEqual(calls[:4], [('long', (50, 20, 12)), ('short', (50000, 500, 2)),
                                         ('long', (50000, 3000, 20)), ('weak', (5000, 500, 20))])
            self.assertEqual({p.stem for p in root.glob('*.case')}, {'oracle', 'short', 'long', 'weak', *extras})
            if extras == ['wide', 'orthogonal']:
                self.assertEqual(calls[4:], [('wide', (5000, 500, 20)), ('orthogonal', (50, 20, 12))])
            elif extras:
                self.assertEqual(calls[4:], list(cmgschur_cases.HIGH_DEGREE_SIZES.items()))

    def test_csv_preserves_rows_and_rejects_missing_or_inexact_ids(self):
        path = self.root / 'source.csv'
        path.write_text('y,i,j,x\n2,1,3,0.5\n4,1,3,-0.5\n')
        columns = cmgschur_cases.csv_columns(path, 'y', 'i', 'j', ['x'])
        self.assertEqual([list(c) for c in columns], [[2., 4.], [.5, -.5], [1., 1.], [3., 3.]])
        for bad in ['nan', '', '9007199254740994', '1.5']:
            path.write_text(f'y,i,j\n2,{bad},3\n')
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                cmgschur_cases.csv_columns(path, 'y', 'i', 'j', [])

    def test_framed_variant_only_changes_options(self):
        source, target = self.root / 'source.case', self.root / 'target.case'
        source.write_bytes(case_bytes())
        cmgschur.prepare(source, target, 'cmgschur5', 4)
        with target.open('rb') as stream:
            shape, args = case_header(stream)
            self.assertEqual(shape[2], 5)
            self.assertIn(b'solver=cmgschur', args)
            self.assertIn(b'cmgdegree=5', args)
            self.assertIn(b'threads=4', args)
            self.assertIn(b'preconditioner=off', args)
            self.assertEqual(stream.read(), b'body\n9\nthreads=1\n')
        strict = self.root / 'strict.case'
        cmgschur.prepare(source, strict, 'lsmr-reference', 1)
        with strict.open('rb') as stream:
            _, args = case_header(stream)
            self.assertIn(b'solver=lsmr', args)
            self.assertIn(b'tolerance=1e-12', args)
            self.assertIn(b'preconditioner=auto', args)
        baseline = self.root / 'baseline.case'
        cmgschur.prepare(source, baseline, 'baseline-cmgschur5', 4)
        self.assertEqual(baseline.read_bytes(), target.read_bytes())

    def test_sampled_options_are_explicit_and_never_leak_into_other_arms(self):
        source = self.root / 'sampled-source.case'
        source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=cmgschur',
                                       b'preconditioner=off', b'cmgdegree=6', b'cmgsamples=99', b'cmgseed=123']))
        arms = cmgschur.ARMS + cmgschur.BASELINE_ARMS + ['lsmr-reference', 'baseline-lsmr-reference']
        for arm in arms + list(cmgschur.SAMPLED_ARMS):
            target = self.root / (arm + '.case')
            cmgschur.prepare(source, target, arm, 4)
            with target.open('rb') as stream:
                _, args = case_header(stream)
                self.assertEqual(stream.read(), b'body\n9\nthreads=1\n')
            fields = dict(a.decode().split('=', 1) for a in args[1:])
            self.assertEqual(len(fields), len(args) - 1)
            if arm in cmgschur.SAMPLED_ARMS:
                self.assertEqual(fields['solver'], 'cmgschur')
                self.assertEqual(fields['cmgdegree'], '4')
                self.assertEqual(fields['cmgsamples'], str(cmgschur.SAMPLED_ARMS[arm]))
                self.assertEqual(fields['cmgseed'], '0')
            else:
                self.assertNotIn('cmgsamples', fields)
                self.assertNotIn('cmgseed', fields)
        with self.assertRaises(ValueError):
            cmgschur.prepare(source, self.root / 'invalid.case', 'baseline-cmgschur4-sample1', 1)

    def test_all_arms_once_in_each_rotating_repetition(self):
        self.assertEqual(cmgschur.order(-1), cmgschur.ARMS)
        for repetition in range(7):
            self.assertEqual(set(cmgschur.order(repetition)), set(cmgschur.ARMS))
            self.assertEqual(len(cmgschur.order(repetition)), 5)
        self.assertGreater(len({tuple(cmgschur.order(i)) for i in range(7)}), 4)
        selected = cmgschur.selected_arms(None, True)
        self.assertEqual(selected, cmgschur.ARMS + cmgschur.BASELINE_ARMS)
        self.assertEqual(cmgschur.selected_arms(None, False), cmgschur.ARMS)
        self.assertEqual(cmgschur.selected_arms(list(cmgschur.SAMPLED_ARMS), False), list(cmgschur.SAMPLED_ARMS))
        for repetition in range(7):
            self.assertEqual(sorted(cmgschur.order(repetition, selected)), sorted(selected))
        subset = ['baseline-cmgschur4', 'cmgschur4']
        self.assertEqual(cmgschur.selected_arms(subset, True), subset)
        self.assertEqual({tuple(cmgschur.order(i, subset)) for i in range(7)},
                         {tuple(subset), tuple(reversed(subset))})
        for bad, baseline in [([], True), (['auto', 'auto'], False), (subset, False),
                              (['baseline-cmgschur4-sample1'], True)]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                cmgschur.selected_arms(bad, baseline)

    def test_optional_exact_baseline_guard_is_order_independent_and_fail_fast(self):
        pair = ['cmgschur4', 'baseline-cmgschur4']
        for order in [pair, list(reversed(pair))]:
            observed = {order[0]: 'same-science-and-signature'}
            self.assertIsNone(cmgschur.check_exact_baseline(order[0], observed, ['cmgschur4']))
            observed[order[1]] = observed[order[0]]
            self.assertTrue(cmgschur.check_exact_baseline(order[1], observed, ['cmgschur4']))
            observed[order[1]] = 'different-science-or-signature'
            with self.assertRaisesRegex(ValueError, 'nonexact baseline science or solver signature'):
                cmgschur.check_exact_baseline(order[1], observed, ['cmgschur4'])
            self.assertIsNone(cmgschur.check_exact_baseline(order[1], observed, []))

    def test_science_guards_accept_roundoff_but_reject_wrong_science(self):
        base = result()
        altered = copy.deepcopy(base)
        altered['matrices']['RB']['bits'][0] = bits(1. + 1e-9)
        cmgschur.scientific_comparison(base, altered, self.left, self.right, self.meta)
        for name in ['RB', 'RV', 'RSTATUS', 'RDOF']:
            altered = copy.deepcopy(base)
            altered['matrices'][name]['bits'][1 if name == 'RDOF' else 0] = bits(2.)
            with self.subTest(name=name), self.assertRaises(ValueError):
                cmgschur.scientific_comparison(base, altered, self.left, self.right, self.meta)
        for column, value in [(0, 0.), (1, 1.), (2, 1.), (4, float.fromhex('0x1p+1023'))]:
            data = self.data.copy()
            data[column, 0] = value
            data.tofile(self.right)
            with self.subTest(column=column), self.assertRaises(ValueError):
                cmgschur.scientific_comparison(base, base, self.left, self.right, self.meta)

    def test_original_failure_is_retained_and_new_solver_failure_stops(self):
        for failed in ['cmg', 'cmgschur5', 'cmgschur4-sample2']:
            with self.subTest(failed=failed):
                root = self.root / failed
                root.mkdir()
                oracle, case = root / 'oracle.case', root / 'timing.case'
                for path in [oracle, case]:
                    path.write_bytes(case_bytes())
                    meta = {**self.meta, 'case_sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
                    if path == oracle:
                        meta['oracle'] = {}
                    path.with_suffix('.json').write_text(json.dumps(meta))
                plugin = root / 'plugin'
                plugin.write_bytes(b'plugin')
                def build(path, *unused):
                    path.write_bytes(b'harness')
                    return ['mock-build']
                def run(command, log, timeout):
                    stem = Path(command[-2]).stem
                    arm = 'lsmr-reference' if stem == 'reference' else stem.removeprefix('current-')
                    if arm == failed:
                        raise RuntimeError('intentional failed cell')
                    value = {**result(arm), 'columns': []}
                    path = Path(command[-1])
                    path.write_text(json.dumps(value))
                    self.data.tofile(path.with_suffix('.json.columns'))
                    log.write_text('NATIVE_SECONDS=0.1\n')
                    return {'wall_s': .1, 'maxrss_bytes': 10}
                output = root / 'output'
                argv = ['cmgschur', '--plugin', str(plugin), '--oracle', str(oracle),
                        '--cases', str(case), '--output', str(output), '--threads', '1']
                if failed in cmgschur.SAMPLED_ARMS:
                    argv += ['--arms', *cmgschur.ARMS, failed]
                with patch.object(sys, 'argv', argv), patch.object(cmgschur, 'scratch', side_effect=lambda p: p), \
                        patch.object(cmgschur, 'build_harness', side_effect=build), \
                        patch.object(cmgschur, 'run_process', side_effect=run), \
                        patch.object(cmgschur, 'oracle_check'), patch('builtins.print'):
                    self.assertEqual(cmgschur.main(), 1)
                summary = json.loads((output / 'summary.json').read_text())
                self.assertTrue(any(r['status'] == 'failed' and r['arm'] == failed for r in summary['records']))
                if failed == 'cmg':
                    self.assertEqual(summary['status'], 'complete_with_failed_cells')
                    self.assertEqual(len(summary['summary']), 1)
                    self.assertNotIn('cmg', summary['summary'][0]['medians'])
                else:
                    self.assertEqual(summary['status'], 'failed')
                    self.assertEqual(summary['summary'], [])
                if failed in cmgschur.SAMPLED_ARMS:
                    self.assertEqual(summary['sampled_preconditioner'],
                                     {failed: {'cmgdegree': 4, 'cmgsamples': 2, 'cmgseed': 0}})

    def test_baseline_cli_pairs_numerical_changes_and_guards_both_artifact_hashes(self):
        for mutation in [None, 'baseline', 'candidate', 'exact_required']:
            with self.subTest(mutation=mutation):
                root = self.root / (mutation or 'success')
                root.mkdir()
                oracle, case = root / 'oracle.case', root / 'timing.case'
                for path in [oracle, case]:
                    path.write_bytes(case_bytes())
                    meta = {**self.meta, 'case_sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
                    if path == oracle:
                        meta['oracle'] = {}
                    path.with_suffix('.json').write_text(json.dumps(meta))
                plugins = {label: root / label for label in ['candidate', 'baseline']}
                for label, path in plugins.items():
                    path.write_bytes(label.encode())
                def build(path, *unused):
                    path.write_bytes(b'harness')
                    return ['mock-build']
                def run(command, log, timeout):
                    artifact = Path(command[1]).name
                    stem = Path(command[-2]).stem
                    arm = 'lsmr-reference' if stem == 'reference' else stem.removeprefix('current-')
                    value = {**result(arm), 'columns': []}
                    if artifact == 'baseline' and cmgschur.solver_arm(arm).startswith('cmgschur'):
                        value['matrices']['RB']['bits'][0] = bits(1. + 1e-9)
                    path = Path(command[-1])
                    path.write_text(json.dumps(value))
                    self.data.tofile(path.with_suffix('.json.columns'))
                    log.write_text('NATIVE_SECONDS=' + ('.2' if artifact == 'baseline' else '.1') + '\n')
                    if mutation in plugins:
                        plugins[mutation].write_bytes(b'changed artifact')
                    return {'wall_s': .3, 'maxrss_bytes': 10}
                output = root / 'output'
                selected = ['baseline-cmgschur4', 'cmgschur4'] + (list(cmgschur.SAMPLED_ARMS) if mutation is None else [])
                argv = ['cmgschur', '--plugin', str(plugins['candidate']), '--baseline-plugin', str(plugins['baseline']),
                        '--arms', *selected, '--oracle', str(oracle),
                        '--cases', str(case), '--output', str(output), '--threads', '1']
                if mutation == 'exact_required':
                    argv += ['--exact-baseline-arms', 'cmgschur4']
                with patch.object(sys, 'argv', argv), patch.object(cmgschur, 'scratch', side_effect=lambda p: p), \
                        patch.object(cmgschur, 'build_harness', side_effect=build), \
                        patch.object(cmgschur, 'run_process', side_effect=run), \
                        patch.object(cmgschur, 'oracle_check'), patch('builtins.print'):
                    code = cmgschur.main()
                summary = json.loads((output / 'summary.json').read_text())
                self.assertEqual(set(summary['plugins']), {'candidate', 'baseline'})
                self.assertEqual(summary['arms'], selected)
                for record in summary['records']:
                    self.assertEqual(record['plugin_sha256'], summary['plugins'][record['plugin']]['sha256'])
                references = [r for r in summary['records'] if r['phase'] == 'reference']
                self.assertEqual(len(references), 2 if mutation == 'exact_required' else 4)
                if mutation == 'exact_required':
                    self.assertEqual(code, 1)
                    self.assertEqual(summary['status'], 'failed')
                    self.assertIn('nonexact baseline science or solver signature', summary['error'])
                    self.assertEqual(summary['summary'], [])
                    self.assertFalse(any(r['phase'] == 'timing' for r in summary['records']))
                elif mutation:
                    self.assertEqual(code, 1)
                    self.assertEqual(summary['status'], 'failed')
                    self.assertIn('plugin or harness changed', summary['error'])
                else:
                    self.assertEqual(code, 0)
                    self.assertEqual(summary['status'], 'complete')
                    row = summary['summary'][0]
                    self.assertEqual(row['candidate_over_baseline']['cmgschur4']['median_ratio'], .5)
                    for arm, count in cmgschur.SAMPLED_ARMS.items():
                        self.assertEqual(row['candidate_over_baseline'][arm]['median_ratio'], .5)
                        self.assertEqual(row['comparisons_to_cmgschur4'][arm]['median_ratio'], 1.)
                        self.assertEqual(summary['sampled_preconditioner'][arm],
                                         {'cmgdegree': 4, 'cmgsamples': count, 'cmgseed': 0})
                    timed = [r for r in summary['records'] if r['phase'] == 'timing']
                    for arm in summary['arms']:
                        records = [r for r in timed if r['arm'] == arm]
                        self.assertEqual(len(records), 8)
                        self.assertEqual(len({r['science_sha256'] for r in records}), 1)
                    self.assertEqual(len({r['science_sha256'] for r in timed}), 2)


if __name__ == '__main__':
    unittest.main()
