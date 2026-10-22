"""Support cannot be inferred from pooled dimensions or incomplete coverage."""
import copy
from fractions import Fraction
from itertools import product
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import two_fe_policy_calibrate as calibration
from two_fe_policy_fit import Work
from test_two_fe_policy_labels import projection


def observation(**changed):
    value = dict(platform='Darwin', seed=2026102201, family='weak', memory='fast',
                 workers=4, control='force16', action=0,
                 checkpoint=projection('packed', end=2, width=3)['checkpoints'][0],
                 labels=dict(I=1000, B=200, startup=100, R=100, C=200, observed_suffix_gain=-80))
    value.update(changed)
    value['checkpoint']['workers'] = value['workers']
    return value


def coverage():
    return [observation(seed=seed, platform=platform, workers=workers)
            for seed, platform, workers in sorted(calibration.REQUIRED_COVERAGE)]


class Support(unittest.TestCase):
    def test_full_cartesian_coverage_and_each_live_class_required(self):
        values = coverage()
        table, _ = calibration.support(values)
        self.assertEqual(table[0], 3)
        self.assertTrue(calibration.admit(values[0]['checkpoint'], table))
        missing, _ = calibration.support(values[:-1])
        self.assertEqual(missing, [0]*96)
        point = dict(values[0]['checkpoint'], classes=3)
        self.assertFalse(calibration.admit(point, table))
        for value in values:
            value['checkpoint']['classes'] = 3
        table, _ = calibration.support(values)
        self.assertEqual(table[:3], [3, 3, 0])
        self.assertTrue(calibration.admit(point, table))

    def test_key_separates_role_weights_scaling_and_correction_not_family(self):
        values = coverage()
        for index, value in enumerate(values):
            value['family'] = 'dense' if index % 2 else 'weak'
        table, _ = calibration.support(values)
        for field, value in (('weighted', 1), ('scaled', 1), ('epoch', 1)):
            point = dict(values[0]['checkpoint'], **{field: value})
            with self.subTest(field=field):
                self.assertFalse(calibration.admit(point, table))
        self.assertTrue(calibration.admit(values[0]['checkpoint'], table))

    def test_completed_history_and_actual_tail_generate_three_groups(self):
        point = projection('regressor', total=9, start=2, end=4, width=2,
                           history=(2, 1, 7, 9, 2))['checkpoints'][0]
        future, mode = calibration.future_at(point)
        self.assertEqual(mode, 2)
        self.assertEqual([(g.width, g.actions, g.work) for g in future],
                         [(2, 2, Work(7, 9)), (1, 1, None), (1, 1, None)])
        # A valid zero-step past block is history, rather than an absent label.
        point.update(history_steps=0, history_live=0)
        self.assertEqual(calibration.future_at(point)[0][0].work, Work(0, 0))

    def test_future_support_modes_do_not_cross_admit(self):
        values = coverage()
        for value in values:
            value['checkpoint'] = projection('regressor', total=8, start=2, end=4,
                width=2, history=(2, 1, 7, 9, 2))['checkpoints'][0]
            value['checkpoint']['workers'] = value['workers']
        table, _ = calibration.support(values)
        self.assertTrue(calibration.admit(values[0]['checkpoint'], table))
        self.assertEqual(table[48], 5)  # regressor current + compatible history
        mixed = dict(values[0]['checkpoint'], total=9)
        self.assertFalse(calibration.admit(mixed, table))

    def test_final_has_no_future_or_regressor_history_credit(self):
        point = projection('final', total=0, block=0)['checkpoints'][0]
        self.assertEqual(calibration.future_at(point), ((), 0))
        self.assertEqual(calibration.keys(point), (72,))

    def test_label_adapter_uses_original_width_and_preintervention_information(self):
        value = observation()
        value['labels']['observed_suffix_gain'] = Fraction(-81, 2)
        row = calibration.label_row(value)
        self.assertEqual(row.execution.r, 3)
        self.assertEqual(row.work, Work(70, 210))
        self.assertEqual(row.observed_suffix_gain, Fraction(-81, 2))
        self.assertEqual([(g.width, g.actions, g.work) for g in row.future], [(1, 1, None)])
        self.assertEqual(calibration.encode(row.observed_suffix_gain),
                         dict(numerator=-81, denominator=2))

    def test_missing_rank_never_emits_default_coefficients(self):
        missing = [dict(reason='constructor_refused')]
        result = calibration.freeze([observation()], missing)
        self.assertEqual(result['status'], 'unsupported')
        self.assertIn('rank', result['reason'])
        self.assertIsNone(result['coefficients'])
        self.assertEqual(result['unavailable'], missing)
        self.assertEqual(result['support'], [0]*96)

    def test_partial_history_and_outside_budget_support_witness_fail(self):
        point = projection('packed', end=2, width=3)['checkpoints'][0]
        point['history_width'] = 3
        with self.assertRaisesRegex(ValueError, 'partial'):
            calibration.future_at(point)
        with self.assertRaisesRegex(ValueError, 'outside'):
            calibration.support([observation(seed=2026104201)])


class PlatformBinding(unittest.TestCase):
    def test_pooling_requires_identical_source_and_full_inputs_before_fit(self):
        base = dict(platform='Darwin', revision='a'*40, bodies={'case': 'body'},
                    inputs={'case': 'header-and-body'}, execution_sources={'runner': 'source'},
                    build='clang-Mac', artifact_sha256='Mac-binary')
        linux = dict(base, platform='Linux', build='gcc-Linux', artifact_sha256='Linux-binary')
        changes = {'revision': 'b'*40, 'bodies': {'case': 'changed-body'},
                   'inputs': {'case': 'same-body-changed-header'},
                   'execution_sources': {'runner': 'different-source'}, 'platform': 'Darwin'}
        for field, value in changes.items():
            with self.subTest(field=field), tempfile.TemporaryDirectory() as folder:
                output = Path(folder)/'fit.json'
                args = ['calibrate', '--platform', 'mac-plan', 'mac-results', '--platform',
                        'linux-plan', 'linux-results', '--output', str(output)]
                results = [([], [], base), ([], [], dict(linux, **{field: value}))]
                with patch('sys.argv', args), patch.object(calibration, 'collect', side_effect=results), \
                        patch.object(calibration, 'freeze') as fit:
                    with self.assertRaisesRegex(ValueError, 'platforms differ'):
                        calibration.main()
                    fit.assert_not_called()
                    self.assertFalse(output.exists())

    def test_platform_specific_binary_and_build_are_retained(self):
        base = dict(platform='Darwin', revision='a'*40, bodies={'case': 'body'},
                    inputs={'case': 'header-and-body'}, execution_sources={'runner': 'source'},
                    build='clang-Mac', artifact_sha256='Mac-binary')
        linux = dict(base, platform='Linux', build='gcc-Linux', artifact_sha256='Linux-binary')
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)/'fit.json'
            args = ['calibrate', '--platform', 'mac-plan', 'mac-results', '--platform',
                    'linux-plan', 'linux-results', '--output', str(output)]
            with patch('sys.argv', args), \
                    patch.object(calibration, 'collect', side_effect=[([], [], base), ([], [], linux)]), \
                    patch.object(calibration, 'freeze', return_value={'status': 'fitted'}) as fit:
                self.assertEqual(calibration.main(), 0)
                fit.assert_called_once_with([], [])
            import json
            retained = json.loads(output.read_text())
            self.assertEqual(retained['bindings'], [base, linux])


if __name__ == '__main__':
    unittest.main()
