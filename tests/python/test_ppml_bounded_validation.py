from __future__ import annotations

import ast
import copy
import importlib.util
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


B = load('bounded', ROOT / 'scripts/ppml_bounded_validation.py')
F = load('fixtures', ROOT / 'tests/python/test_ppml_solver_development.py')


class BoundedValidationTests(unittest.TestCase):
    def test_fixed_inventory_retains_all_previously_missing_scopes(self):
        m = B.manifest()
        scopes = B.scopes(m)
        self.assertEqual(len(scopes), 45)
        prior = [s for t in m['prior_v3_tasks'] for s in B.cal.scopes('scc_holdout', t)]
        self.assertEqual(scopes[:33], prior)
        self.assertEqual(len({s['id'] for s in scopes}), 45)
        self.assertEqual(sum(s['n'] == 1000000 for s in prior), 8)

    def test_frozen_policy_cannot_acquire_regions(self):
        policy = {'one_fe': 'map', 'default_multi_fe': 'lsmr', 'unsupported_or_unadmitted': 'map',
                  'regions': [], 'training_ratio_to_default': 1.0}
        self.assertEqual(B.requested_solver({'topology': 'balanced_1fe'}, policy), 'map')
        self.assertEqual(B.requested_solver({'topology': 'sparse_3fe'}, policy), 'lsmr')
        policy['regions'].append({'solver': 'cmg'})
        with self.assertRaises(B.dev.DevelopmentError):
            B.requested_solver({'topology': 'sparse_3fe'}, policy)

    def test_budget_depends_on_declared_role_not_observed_speed(self):
        seconds = B.manifest()['seconds']
        for arm, expected in [('map', 120), ('lsmr', 300), ('cmg', 180), ('ppmlhdfe', 120), ('ppmlhdfe_strict', 120)]:
            self.assertEqual(B.process_budget({'FEREG_PPML_SOLVER_MODE': 'run', 'FEREG_PPML_SOLVER_ARM': arm}, 'lsmr', seconds), expected)
        self.assertEqual(B.process_budget({'FEREG_PPML_SOLVER_MODE': 'run', 'FEREG_PPML_SOLVER_ARM': 'map'}, 'map', seconds), 300)

    def test_selected_certification_is_not_full_comparison_acceptance(self):
        run = F.run_document()
        self.assertTrue(B.selected_diagnostics(run, 'lsmr')['recorded_runtime_certification_pass'])
        run['records'] = [r for r in run['records'] if r['arm'] != 'ppmlhdfe']
        self.assertTrue(B.selected_diagnostics(run, 'lsmr')['recorded_runtime_certification_pass'])
        self.assertFalse(B.dev.analyze_document(run)['science_pass'])

    def test_selected_missing_retry_fallback_or_memory_failure_never_passes(self):
        for field, value in [('rc', 459), ('fallback_count', 1), ('effective_engine_code', 1), ('converged', 0), ('effective_threads', 99), ('repetition', 9)]:
            with self.subTest(field=field):
                run = F.run_document()
                next(r for r in run['records'] if r['arm'] == 'lsmr')[field] = value
                self.assertFalse(B.selected_diagnostics(run, 'lsmr')['recorded_runtime_certification_pass'])
        for value in [float('nan'), 1.251, -1]:
            run = F.run_document()
            next(r for r in run['records'] if r['arm'] == 'lsmr')['runtime_values'][22] = value
            self.assertFalse(B.selected_diagnostics(run, 'lsmr')['recorded_runtime_certification_pass'])

    def test_selected_timeout_invalidates_valid_looking_rows(self):
        run = F.run_document()
        p = next(p for p in run['processes'] if p.get('arm') == 'lsmr')
        p['budget_exhausted'] = True
        self.assertFalse(B.selected_diagnostics(run, 'lsmr')['recorded_runtime_certification_pass'])

    def test_coefficient_disagreement_is_not_waived(self):
        run = F.run_document()
        next(r for r in run['records'] if r['arm'] == 'lsmr')['b_values'][0] += 1
        self.assertFalse(B.dev.analyze_document(run)['science_pass'])

    def test_successful_process_has_no_per_fit_bound(self):
        with tempfile.TemporaryDirectory() as temporary:
            process = mock.Mock(pid=1234)
            process.wait.return_value = 0
            runner = B.BoundedStata('lsmr', B.manifest()['seconds'], time.monotonic() + 10)
            with mock.patch.object(B.subprocess, 'Popen', return_value=process) as spawn:
                r = runner('stata', {}, Path(temporary) / 'fit.log')
            self.assertFalse(r['budget_exhausted'])
            self.assertIsNone(r['per_fit_lower_bound'])
            self.assertTrue(spawn.call_args.kwargs['start_new_session'])
            self.assertTrue((Path(temporary) / 'fit.process.json').is_file())

    def test_timeout_is_nonzero_even_if_child_exits_zero_after_term(self):
        with tempfile.TemporaryDirectory() as temporary:
            process = mock.Mock(pid=1234)
            process.wait.side_effect = [subprocess.TimeoutExpired('stata', 1), 0, 0]
            runner = B.BoundedStata('lsmr', B.manifest()['seconds'], time.monotonic() + 10)
            with mock.patch.object(B.subprocess, 'Popen', return_value=process), mock.patch.object(B.os, 'killpg') as kill:
                r = runner('stata', {}, Path(temporary) / 'fit.log')
            self.assertEqual(r['process_returncode'], 124)
            self.assertTrue(r['budget_exhausted'])
            self.assertEqual(kill.call_args_list, [mock.call(1234, signal.SIGTERM), mock.call(1234, signal.SIGKILL)])

    def test_interruption_terminates_children_and_propagates(self):
        with tempfile.TemporaryDirectory() as temporary:
            process = mock.Mock(pid=1234)
            process.wait.side_effect = [KeyboardInterrupt, 0, 0]
            runner = B.BoundedStata('lsmr', B.manifest()['seconds'], time.monotonic() + 10)
            with mock.patch.object(B.subprocess, 'Popen', return_value=process), mock.patch.object(B.os, 'killpg'):
                with self.assertRaises(KeyboardInterrupt):
                    runner('stata', {}, Path(temporary) / 'fit.log')
            self.assertFalse(runner.records)
            self.assertFalse((Path(temporary) / 'fit.process.json').exists())

    def test_deadline_prevents_launch(self):
        runner = B.BoundedStata('lsmr', B.manifest()['seconds'], time.monotonic() - 1)
        with mock.patch.object(B.subprocess, 'Popen') as spawn:
            with self.assertRaises(B.dev.DevelopmentError):
                runner('stata', {}, Path('/unused'))
            spawn.assert_not_called()

    def test_real_process_timeout_and_cleanup(self):
        original = subprocess.Popen
        children = []
        def start(*args, **kwargs):
            child = original([sys.executable, '-c', 'import time; time.sleep(30)'], **kwargs)
            children.append(child)
            return child
        with tempfile.TemporaryDirectory() as temporary:
            runner = B.BoundedStata('lsmr', B.manifest()['seconds'], time.monotonic() + 0.15)
            with mock.patch.object(B.subprocess, 'Popen', side_effect=start):
                result = runner('unused', {}, Path(temporary) / 'fit.log')
            self.assertTrue(result['budget_exhausted'])
            self.assertIsNotNone(children[0].poll())

    def test_runner_cannot_replace_scientific_analysis(self):
        tree = ast.parse((ROOT / 'scripts/ppml_solver_development.py').read_text())
        execute = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'execute_scopes')
        self.assertIn('process_runner', [a.arg for a in execute.args.kwonlyargs])
        # Both ordinary and strict result-table reads reject a killed prefix.
        guards = [n for n in ast.walk(execute) if isinstance(n, ast.If) and 'budget_exhausted' in ast.unparse(n.test)]
        self.assertEqual(len(guards), 2)


if __name__ == '__main__':
    unittest.main()
