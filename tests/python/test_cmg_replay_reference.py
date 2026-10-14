"""Analytic/replication witnesses for the independent CMG replay enrichment."""
import copy
import math
import struct
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'crates/fereg-oracle/python'))
from replay_cmg_reference import normalized_targets
from replay_plugins import scientific_errors


class CmgNormalizedReference(unittest.TestCase):
    def design(self):
        walsh = lambda row, mask: -1. if bin(row & mask).count('1') % 2 else 1.
        x = [[2. + walsh(r, 1) for r in range(8)], [-1. + walsh(r, 3) for r in range(8)]]
        effects = [[(r >> 1) & 1 for r in range(8)], [(r >> 2) & 1 for r in range(8)]]
        channels = [[2. * walsh(r, 2) for r in range(8)], [-walsh(r, 4) for r in range(8)]]
        residual = [walsh(r, 7) for r in range(8)]
        y = [3. + .5 * x[0][r] - .25 * x[1][r] + channels[0][r] + channels[1][r] + residual[r] for r in range(8)]
        return y, x, effects, channels, residual

    def assert_close(self, left, right):
        self.assertEqual(len(left), len(right))
        for a, b in zip(left, right):
            if isinstance(a, list): self.assert_close(a, b)
            else: self.assertTrue(math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-12), (a, b))

    def test_full_constant_cross_covariance_and_saved_gauge_have_analytic_targets(self):
        y, x, effects, channels, residual = self.design()
        expected = [[1/3, 0., -2/3], [0., 1/3, 1/3], [-2/3, 1/3, 2.]]
        for vce in ('iid', 'robust'):
            result = normalized_targets(y, x, effects, [1.] * 8, vce=vce)
            self.assert_close(result['beta'], [.5, -.25, 3.])
            self.assert_close(result['covariance'], expected)
            self.assert_close(result['residual'], residual)
            self.assert_close(result['saved_channels'], channels)
            self.assertEqual((result['rank'], result['fe_rank'], result['precision_digits']), (5, 3, 70))

    def test_cluster_covariance_matches_hand_summed_four_cluster_scores(self):
        y, x, effects, _, _ = self.design()
        result = normalized_targets(y, x, effects, [1.] * 8, vce='cluster', clusters=[r // 2 for r in range(8)])
        # Four cluster influence sums: finite diagonal meat1/4, cross0;
        # constant influence is -2*first+second. Correction=(4/3)*(7/3).
        self.assert_close(result['covariance'], [[7/9, 0., -14/9], [0., 7/9, 7/9], [-14/9, 7/9, 35/9]])

    def test_frequency_weights_equal_explicitly_duplicated_observations(self):
        y, x, effects, _, _ = self.design()
        weights = [1, 2, 1, 3, 2, 1, 4, 2]
        expand = lambda column: [value for value, count in zip(column, weights) for _ in range(count)]
        for vce in ('iid', 'robust'):
            weighted = normalized_targets(y, x, effects, weights, frequency=True, vce=vce)
            duplicated = normalized_targets(expand(y), [expand(c) for c in x], [expand(c) for c in effects],
                                            [1.] * sum(weights), vce=vce)
            self.assert_close(weighted['beta'], duplicated['beta'])
            self.assert_close(weighted['covariance'], duplicated['covariance'])
            self.assert_close([expand(c) for c in weighted['saved_channels']], duplicated['saved_channels'])

    def test_analytic_weight_common_scale_preserves_normalization_and_hc1(self):
        y, x, effects, _, _ = self.design()
        weights = [1., 2., 1., 3., 2., 1., 4., 2.]
        reference = normalized_targets(y, x, effects, weights, vce='robust')
        for scale in (1e-100, 1e100):
            scaled = normalized_targets(y, x, effects, [w * scale for w in weights], vce='robust')
            self.assert_close(scaled['beta'], reference['beta'])
            self.assert_close(scaled['covariance'], reference['covariance'])
            self.assert_close(scaled['saved_channels'], reference['saved_channels'])


    def test_science_guard_detects_constant_cross_covariance_gauge_and_certificate_corruption(self):
        y, x, effects, _, _ = self.design()
        target = normalized_targets(y, x, effects, [1.] * 8)
        bits = lambda value: struct.pack('>d', float(value)).hex()
        table = lambda values: {'rows': 1, 'columns': len(values), 'bits': list(map(bits, values))}
        expected = {'return_code': 0, 'poisson': False, 'rows': list(range(8)), 'sample': [1] * 8,
                    'beta': target['beta'], 'covariance': target['covariance'],
                    'residual': target['residual'], 'fitted': target['fitted'],
                    'outputs': {'sample': 0, 'residual': 1, 'xbd': 2, 'absorbed': 3, 'xb': 4},
                    'saved': [5, 6], 'saved_reference': dict(zip([5, 6], target['saved_channels'])),
                    'column_reference': {3: target['absorbed'], 4: target['xb']},
                    'cmg_diagnostics': {'targets': target['targets'], 'tolerance': 1e-10,
                        'bounds': {'RCERT': {'13': [0., 1e-10]}, 'RSOLVER': {'5': [0., 1e-10]}},
                        'moment_inputs': {'regressors': x, 'weights': [1.] * 8, 'effects': effects}}}
        raw = {'return_code': 0, 'columns': [list(map(bits, c)) for c in
               [[1.] * 8, target['residual'], target['fitted'], target['absorbed'], target['xb'], *target['saved_channels']]],
               'matrices': {'RREC': table([1.]), 'RCERT': table([1.] * 64),
                   'RFIT': table([1.] * 24), 'RSOLVER': table([0.] * 32), 'RB': table(target['beta']),
                   'RV': {'rows': 3, 'columns': 3, 'bits': list(map(bits, [v for row in target['covariance'] for v in row]))}}}
        for name, targets in target['targets'].items():
            for index, value in targets.items(): raw['matrices'][name]['bits'][int(index)] = bits(value)
        raw['matrices']['RCERT']['bits'][13] = bits(0)
        self.assertEqual(scientific_errors(raw, expected), [])
        for name, index, value, label in [('RB', 2, 4., 'coefficients'), ('RV', 2, 0., 'covariance'),
                                        ('RFIT', 11, 0., 'RFIT'), ('RCERT', 13, 1e-8, 'certificate'),
                                        ('RSOLVER', 5, 1e-8, 'certificate')]:
            bad = copy.deepcopy(raw)
            bad['matrices'][name]['bits'][index] = bits(value)
            self.assertTrue(any(label in error for error in scientific_errors(bad, expected)), (name, index))
        bad = copy.deepcopy(raw)
        for i in range(8):
            bad['columns'][5][i] = bits(target['saved_channels'][0][i] + 1.)
            bad['columns'][6][i] = bits(target['saved_channels'][1][i] - 1.)
        self.assertTrue(any('saved FE gauge' in e for e in scientific_errors(bad, expected)))
        bad = copy.deepcopy(raw)
        bad['columns'][1] = list(map(bits, [r + 1e-5 for r in target['residual']]))
        self.assertTrue(any('original-design residual moment' in e for e in scientific_errors(bad, expected)))

    def test_nonidentified_model_and_invalid_frequency_weights_fail_closed(self):
        y, x, effects, _, _ = self.design()
        with self.assertRaises(ValueError): normalized_targets(y, [x[0], x[0]], effects, [1.] * 8)
        with self.assertRaises(ValueError): normalized_targets(y, x, effects, [1.5] * 8, frequency=True)


if __name__ == '__main__':
    unittest.main()
