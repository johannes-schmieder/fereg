"""Typed CMG diagnostic allowances must not excuse science or routing failures."""
import copy
from argparse import Namespace
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import replay_cmg_polish as cmg
import replay_plugins as replay


def table(values, rows=1, columns=None):
    return {'rows': rows, 'columns': columns or len(values),
            'bits': [struct.pack('>d', float(value)).hex() for value in values]}


def result(mode='fast', parent=False):
    primary = [7, 7, 5, 2, 0., 0., 0., 0., 0., 0., 0, 0 if parent else 1, 0, 0, 0, 0]
    final = [7, 7, 5, 2, 0., 0., 0., 0., 0., 0., 0, 0 if parent else 1, 0, 0, 0, 0]
    if mode == 'fast' and not parent:
        final[1], final[10] = 4, 1
    runtime = [1., 1., 1., 1. if mode == 'fast' else 2., 1., 10.] + [0.] * 54
    return {'return_code': 0, 'error': '', 'columns': [['3ff0000000000000'], ['0' * 16]],
            'matrices': {'RSOLVER': table(primary + final, 2, 16),
                         'RRUNTIME': table(runtime), 'RMEMORY': table([0]),
                         'RSTATUS': table([1, 1, 0, 1] * 3, 3, 4),
                         'RDOF': table([0.] * 40), 'RCERT': table([1.] * 64),
                         'RREC': table([1.]), 'RB': table([1.]), 'RV': table([1.])}}


def set_value(value, name, index, new):
    value['matrices'][name]['bits'][index] = struct.pack('>d', float(new)).hex()


class TypedCmgDiagnostics(unittest.TestCase):
    def test_only_known_case_diagnostic_paths_are_allowed(self):
        self.assertEqual(cmg.diagnostic_paths('fast'), [f'/matrices/RSOLVER/bits/{i}' for i in [9, 15, 22, 23]])
        self.assertEqual(cmg.diagnostic_paths('low'), [f'/matrices/RSOLVER/bits/{i}' for i in [9, 23]])
        for mode in ('fast', 'low'):
            cmg.validate_rows(result(mode), mode, 10)
            cmg.validate_rows(result(mode, True), mode, 10, parent=True)

    def test_primary_fallback_and_wrong_request_or_pilot_are_rejected(self):
        for index, value in [(0, 1), (1, 4), (3, 3), (10, 1), (12, 1), (13, 1), (14, 1), (15, .1)]:
            bad = result()
            set_value(bad, 'RSOLVER', index, value)
            with self.assertRaises(ValueError): cmg.validate_rows(bad, 'fast', 10)

    def test_final_families_are_mode_specific(self):
        for mode, index, value in [('fast', 17, 7), ('fast', 26, 0), ('fast', 27, 0),
                                   ('low', 17, 4), ('low', 26, 1), ('low', 27, 2)]:
            bad = result(mode)
            set_value(bad, 'RSOLVER', index, value)
            with self.assertRaises(ValueError): cmg.validate_rows(bad, mode, 10)

    def test_fast_final_verification_can_have_zero_updates(self):
        value = result()
        set_value(value, 'RSOLVER', 18, 0)
        cmg.validate_rows(value, 'fast', 10)
        set_value(value, 'RSOLVER', 2, 0)
        with self.assertRaises(ValueError): cmg.validate_rows(value, 'fast', 10)

    def test_low_warning_bound_is_source_action_count_not_iteration_count(self):
        value = result('low')
        set_value(value, 'RSOLVER', 2, 1)
        set_value(value, 'RSOLVER', 11, 2)  # 1 + ceil(C=3/P=10)
        cmg.validate_rows(value, 'low', 10)
        for index, bad_count in [(11, 3), (11, -1), (11, .5), (11, float('nan')), (27, 2)]:
            bad = copy.deepcopy(value)
            set_value(bad, 'RSOLVER', index, bad_count)
            with self.assertRaises(ValueError): cmg.validate_rows(bad, 'low', 10)

    def test_actual_mode_pool_shape_and_parent_are_checked(self):
        for name, index, value in [('RRUNTIME', 3, 2), ('RRUNTIME', 5, 999), ('RRUNTIME', 4, 0)]:
            bad = result()
            set_value(bad, name, index, value)
            with self.assertRaises(ValueError): cmg.validate_rows(bad, 'fast', 10)
        bad = result()
        bad['matrices']['RSTATUS']['rows'] = 99
        with self.assertRaises(ValueError): cmg.validate_rows(bad, 'fast', 10)
        with self.assertRaises(ValueError): cmg.validate_rows(result(), 'fast', 10, parent=True)

    def test_parent_normalization_does_not_hide_other_decisions_or_worker_drift(self):
        expected = {'poisson': False, 'outputs': {'sample': 0, 'drop_reason': 1}}
        parent, candidate = result(parent=True), result()
        views = [replay.discrete_view(value, expected) for value in (parent, candidate)]
        self.assertEqual(cmg.parent_discrete_view(views[0], 'fast'), cmg.parent_discrete_view(views[1], 'fast'))
        self.assertNotEqual(views[0], views[1])  # unchanged worker view sees concrete diagnostics
        for name, index in [('RSTATUS', 1), ('RCERT', 63), ('RDOF', 19), ('RSOLVER', 12)]:
            bad = copy.deepcopy(candidate)
            set_value(bad, name, index, 123)
            self.assertNotEqual(cmg.parent_discrete_view(replay.discrete_view(bad, expected), 'fast'),
                                cmg.parent_discrete_view(views[1], 'fast'))
        bad = copy.deepcopy(candidate)
        set_value(bad, 'RSOLVER', 11, 0)
        self.assertNotEqual(replay.discrete_view(bad, expected), views[1])
        self.assertTrue(replay.differences(replay.exact_view(bad, False), replay.exact_view(candidate, False)))

    def test_numerical_and_certificate_failures_remain_visible(self):
        candidate = result()
        expected = {'return_code': 0, 'poisson': False, 'beta': [1.], 'covariance': [[1.]],
                    'outputs': {'sample': 0, 'drop_reason': 1, 'residual': 1, 'xbd': 1},
                    'sample': [1.], 'rows': [0], 'residual': [0.], 'fitted': [0.]}
        self.assertEqual(replay.scientific_errors(candidate, expected), [])
        for name, index, value in [('RB', 0, 2), ('RCERT', 63, 0), ('RREC', 0, 0)]:
            bad = copy.deepcopy(candidate)
            set_value(bad, name, index, value)
            cmg.validate_rows(bad, 'fast', 10)  # diagnostics alone never imply science
            self.assertTrue(replay.scientific_errors(bad, expected))

    def test_every_worker_result_and_repeat_runs_the_typed_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            case = root / 'framed.case'
            case.write_text('9\nthreads=1\n')
            args = Namespace(thread_counts=[4], output=root, harness=Path('unused'), plugin=Path('unused'))
            bad = result()
            set_value(bad, 'RSOLVER', 1, 4)
            expected = {'poisson': False}
            for returns in ([bad, result()], [result(), bad]):
                with patch.object(replay, 'run_case', side_effect=returns):
                    with self.assertRaises(ValueError):
                        replay.check_threads(args, case, expected, result(), ('fast', 10))


class TypedCmgBindings(unittest.TestCase):
    def scope(self):
        return {'cases': {name: {'input_sha256': identity, 'diagnostic_contract': cmg.KIND}
                          for name, (identity, _) in cmg.CASES.items()}}

    def test_wrong_case_ppml_and_unknown_contract_fail_before_dispatch(self):
        for name in ('ols-09', 'ppml-08'):
            with self.assertRaises(ValueError): cmg.case_contract(Path(name + '.case'))
        for mutation in ('extra', 'missing', 'kind'):
            scope = self.scope()
            if mutation == 'extra': scope['cases']['ppml-08'] = scope['cases']['ols-08']
            elif mutation == 'missing': del scope['cases']['ols-08']
            else: scope['cases']['ols-08']['diagnostic_contract'] = 'anything'
            with self.assertRaises(RuntimeError):
                cmg.validate_bindings(scope, {}, None, None, None, None, None, ROOT)

    def test_exact_input_hash_does_not_replace_solver_mode_and_pool_checks(self):
        case = Path('ols-08.case')
        for bad in ({'solver': 'auto', 'memory': 'fast', 'poolsize': '10'},
                    {'solver': 'cmg', 'memory': 'low', 'poolsize': '10'},
                    {'solver': 'cmg', 'memory': 'fast', 'poolsize': '0'}):
            with patch.object(cmg, 'digest', return_value=cmg.CASES['ols-08'][0]), \
                    patch.object(cmg, 'read_case', return_value=(bad, [], {})):
                with self.assertRaises(ValueError): cmg.case_contract(case)

    def test_missing_bindings_stop_main_without_a_native_call(self):
        with tempfile.TemporaryDirectory() as directory:
            scope = Path(directory) / 'scope.json'
            scope.write_text(json.dumps(self.scope()))
            with patch.object(sys, 'argv', ['replay_plugins.py', '--plugin', 'unused', '--baseline', 'unused',
                                           '--affected-scope', str(scope)]), patch.object(replay, 'run_case') as native:
                with self.assertRaises(RuntimeError): replay.main()
                native.assert_not_called()

    def test_each_artifact_source_and_reference_binding_is_mandatory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in cmg.SOURCES:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name)
            paths = [root / name for name in ('scope', 'candidate', 'parent', 'harness')]
            for path in paths: path.write_text(path.name)
            for name in cmg.CASES: (root / (name + '.json')).write_text(name)
            value = {'schema': 1, 'kind': cmg.KIND,
                     **{name: cmg.digest(path) for name, path in zip(
                         ['scope_sha256', 'plugin_sha256', 'baseline_sha256', 'harness_sha256'], paths)},
                     'sources': {name: cmg.digest(root / name) for name in cmg.SOURCES},
                     'references': {name: cmg.digest(root / (name + '.json')) for name in cmg.CASES}}
            def verify(bindings):
                cmg.validate_bindings(self.scope(), bindings, *paths, root, root)
            with patch.object(cmg, 'case_contract', return_value=('fast', 10)):
                verify(value)
                for key in value:
                    bad = copy.deepcopy(value)
                    del bad[key]
                    with self.assertRaises(RuntimeError): verify(bad)
                for key in ('scope_sha256', 'plugin_sha256', 'baseline_sha256', 'harness_sha256'):
                    bad = copy.deepcopy(value)
                    bad[key] = '0' * 64
                    with self.assertRaises(RuntimeError): verify(bad)
                for key in ('sources', 'references'):
                    bad = copy.deepcopy(value)
                    bad[key][next(iter(bad[key]))] = '0' * 64
                    with self.assertRaises(RuntimeError): verify(bad)


if __name__ == '__main__':
    unittest.main()
