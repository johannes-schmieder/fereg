"""Acceptance checks must detect numerical/routing drift and distinct harness errors."""
import copy
import math
import json
import struct
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
import tempfile
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from replay_plugins import scientific_errors, affected_prefixes, digest, allowed_difference, differences, discrete_view, exact_view, expected_for_fixes
from replay_psd import eigensystem
from replay_bias_reference import enrich_expected, REFERENCE, NAMES
from replay_ols_reference import dense_targets, derive_ols, enrich_expected as enrich_ols_expected


class ReplayChecks(unittest.TestCase):
    def fixture(self):
        return {'return_code': 0, 'columns': [['3ff0000000000000']], 'matrices': {
            'RB': {'bits': ['3ff0000000000000']},
            'RSOLVER': {'columns': 16, 'bits': ['0000000000000000'] * 32},
            'RRUNTIME': {'bits': ['0000000000000000'] * 60},
            'RMEMORY': {'bits': ['0000000000000000']}}}

    def test_one_bit_coefficient_and_route_changes_are_visible(self):
        base = self.fixture()
        for name, index in [('RB', 0), ('RSOLVER', 1), ('RRUNTIME', 1), ('RRUNTIME', 57)]:
            changed = copy.deepcopy(base)
            changed['matrices'][name]['bits'][index] = '3ff0000000000001'
            self.assertTrue(differences(exact_view(base, False), exact_view(changed, False)))

    def test_only_documented_process_fields_are_excluded(self):
        base = self.fixture()
        changed = copy.deepcopy(base)
        for name, index in [('RSOLVER', 8), ('RRUNTIME', 35), ('RMEMORY', 0)]:
            changed['matrices'][name]['bits'][index] = '3ff0000000000001'
        self.assertEqual(differences(exact_view(base, False), exact_view(changed, False)), [])

    def test_return_codes_and_observation_outputs_are_exact(self):
        base = self.fixture()
        changed = copy.deepcopy(base)
        changed['return_code'] = 498
        changed['columns'][0][0] = '0000000000000000'
        self.assertEqual(len(differences(exact_view(base, False), exact_view(changed, False))), 2)

    def test_intentional_changes_are_field_scoped_and_science_is_required(self):
        self.assertTrue(allowed_difference('/matrices/RV/bits/1', ['/matrices/RV/bits']))
        self.assertFalse(allowed_difference('/matrices/RB/bits/1', ['/matrices/RV/bits']))
        self.assertFalse(allowed_difference('/matrices/RV/bits/19', ['/matrices/RV/bits/1']))
        self.assertFalse(allowed_difference('fixed-thread-repeat/matrices/RV/bits/1', ['/matrices/RV/bits']))
        old = {'known_defect': 'slope-dof', 'return_code': 498, 'beta': [1.]}
        repaired = expected_for_fixes(old, {'slope-dof'})
        self.assertEqual(repaired['return_code'], 0)
        self.assertEqual(repaired['beta'], old['beta'])
        self.assertNotIn('known_defect', repaired)
        self.assertEqual(old['return_code'], 498)

    def test_thread_checks_keep_discrete_decisions_without_requiring_float_bits(self):
        base = self.fixture()
        base['matrices']['RDOF'] = {'bits': ['0000000000000000'] * 40}
        base['matrices']['RSTATUS'] = {'bits': ['0000000000000000'] * 4}
        base['columns'].append(['0000000000000000'])
        expected = {'poisson': False, 'outputs': {'sample': 0, 'drop_reason': 1}}
        changed = copy.deepcopy(base)
        changed['matrices']['RB']['bits'][0] = '3ff0000000000001'
        changed['matrices']['RDOF']['bits'][9] = '3ff0000000000001'
        self.assertEqual(discrete_view(base, expected), discrete_view(changed, expected))
        for table, index in [('RDOF', 19), ('RSTATUS', 1)]:
            bad = copy.deepcopy(changed)
            bad['matrices'][table]['bits'][index] = '3ff0000000000000'
            self.assertNotEqual(discrete_view(base, expected), discrete_view(bad, expected))
        changed['columns'][1][0] = '3ff0000000000000'
        self.assertNotEqual(discrete_view(base, expected), discrete_view(changed, expected))

    def test_independent_psd_target_has_analytic_spectrum(self):
        values, fixed = eigensystem([[1., 2., 0.], [2., 1., 0.], [0., 0., 4.]])
        for a,b in zip(sorted(values), [-1., 3., 4.]): self.assertAlmostEqual(a,b,places=13)
        target = [[1.5,1.5,0.],[1.5,1.5,0.],[0.,0.,4.]]
        for row, expected in zip(fixed,target):
            for a,b in zip(row,expected): self.assertTrue(math.isclose(a,b,abs_tol=1e-14))


if __name__ == '__main__': unittest.main()

class AffectedScopeChecks(unittest.TestCase):
    def test_input_bound_numerical_scope_cannot_exclude_scientific_decisions(self):
        with tempfile.TemporaryDirectory() as root:
            case = Path(root) / 'example.case'
            case.write_text('frozen input')
            expected = {'outputs': {'sample': 0, 'drop_reason': 1, 'residual': 2}, 'saved': [3]}
            entry = {'input_sha256': digest(case), 'reason': 'declared algorithm change',
                     'allowed_paths': ['/matrices/RB/bits', '/columns/2', '/columns/3', '/runtime_signature']}
            scope = {'cases': {'example': entry}}
            self.assertEqual(affected_prefixes(scope, case, expected), entry['allowed_paths'])
            self.assertFalse(allowed_difference('fixed-thread-repeat/matrices/RB/bits/0', entry['allowed_paths']))
            for path in ['/columns/0', '/columns/1', '/matrices/RDOF/bits', '/matrices/RSTATUS/bits',
                         '/matrices/RB', '/return_code', '/matrices/RREC/bits']:
                entry['allowed_paths'] = [path]
                with self.assertRaises(RuntimeError): affected_prefixes(scope, case, expected)
            entry['allowed_paths'] = ['/matrices/RB/bits']
            case.write_text('changed input')
            with self.assertRaises(RuntimeError): affected_prefixes(scope, case, expected)

    def test_bias_scope_keeps_controls_support_and_zero_coordinates_exact(self):
        with tempfile.TemporaryDirectory() as root:
            case = Path(root) / 'bias.case'
            case.write_text('frozen bias input')
            expected = {'outputs': {'sample': 0, 'drop_reason': 1},
                        'bias': {'method': 'exact', 'coefficient_columns': 3, 'zero_slots': [2]}}
            entry = {'input_sha256': digest(case), 'reason': 'declared downstream contractions',
                     'allowed_paths': []}
            scope = {'cases': {'bias': entry}}
            for path in ['/matrices/RBIAS/bits/3', '/matrices/RBDIAG/bits/7', '/matrices/RBSUPPORT/bits/8']:
                entry['allowed_paths'] = [path]
                self.assertEqual(affected_prefixes(scope, case, expected), [path])
            for path in ['/matrices/RBIAS/bits', '/matrices/RBIAS/bits/2', '/matrices/RBIAS/bits/15',
                         '/matrices/RBDIAG/bits/0', '/matrices/RBDIAG/bits/18', '/matrices/RBSUPPORT/bits/7']:
                entry['allowed_paths'] = [path]
                with self.assertRaises(RuntimeError): affected_prefixes(scope, case, expected)

    def test_bias_support_and_controls_remain_in_discrete_cross_worker_view(self):
        result = {'matrices': {'RDOF': {'bits': ['0'] * 40}, 'RSTATUS': {'bits': ['0'] * 4},
                              'RBDIAG': {'bits': ['0'] * 21},
                              'RBSUPPORT': {'columns': 11, 'bits': ['0'] * 22}},
                  'columns': [['0'], ['0']]}
        expected = {'poisson': True, 'outputs': {'sample': 0, 'drop_reason': 1}}
        changed = copy.deepcopy(result)
        changed['matrices']['RBDIAG']['bits'][7] = 'floating score'
        changed['matrices']['RBSUPPORT']['bits'][8] = 'floating information'
        self.assertEqual(discrete_view(result, expected), discrete_view(changed, expected))
        for table, index in [('RBDIAG', 3), ('RBSUPPORT', 5), ('RBSUPPORT', 14)]:
            bad = copy.deepcopy(changed)
            bad['matrices'][table]['bits'][index] = 'changed decision'
            self.assertNotEqual(discrete_view(result, expected), discrete_view(bad, expected))


class IndependentOlsReferenceChecks(unittest.TestCase):
    def analytic_targets(self, frequency=False):
        # Orthogonal centered slopes and residual; nonzero raw slope means
        # distinguish the required original norms from within-column norms.
        first = [-1., 1.] * 4
        second = [-1., -1., 1., 1.] * 2
        residual = [-1.] * 4 + [1.] * 4
        outcome = [2 + 3 * a + 4 * b + e for a, b, e in zip(first, second, residual)]
        regressors = [[a + 1 for a in first], [b + 2 for b in second]]
        return dense_targets(outcome, regressors, [[0.] * 8],
                             [2. if frequency else 1.] * 8, frequency)

    def test_dense_weighted_witness_has_analytic_targets(self):
        for frequency, adjusted in [(False, 123 / 130), (True, 323 / 338)]:
            targets = self.analytic_targets(frequency)
            self.assertAlmostEqual(targets['RFIT']['11'], adjusted, places=14)
            self.assertAlmostEqual(targets['RCERT']['11'], math.sqrt(2.5), places=14)

    def test_enrichment_rejects_changed_model_and_skips_unrelated_cases(self):
        expected = {'poisson': False}
        self.assertIs(enrich_ols_expected(Path('ols-09.case'), expected), expected)
        # The declared fixture has two finite columns. A third term must fail
        # before any direct solve, even when the file keeps a scoped case name.
        with patch('replay_ols_reference.read_case', return_value=(
                {'term_count': '3'}, [[1.] * 150 for _ in range(13)], {})):
            with self.assertRaisesRegex(ValueError, 'model differs'):
                derive_ols(Path('ols-10.case'))

    def result_fixture(self):
        targets = self.analytic_targets()
        expected = {'return_code': 0, 'poisson': False, 'beta': [0.], 'covariance': [[1.]],
                    'outputs': {'sample': 0, 'residual': 1, 'xbd': 2},
                    'sample': [1.], 'rows': [0], 'residual': [0.], 'fitted': [1.],
                    'ols_reference': {'targets': targets}}
        def table(values):
            return {'columns': 1, 'bits': [struct.pack('>d', value).hex() for value in values]}
        matrices = {'RREC': table([1.]), 'RB': table([0.]), 'RV': table([1.])}
        for name, values in targets.items():
            data = [1., 0.] * 12
            data[-1] = 1.  # Complete certificate in the existing scientific check.
            data[11] = values['11']
            matrices[name] = table(data)
        result = {'return_code': 0, 'matrices': matrices,
                  'columns': [['3ff0000000000000'], ['0000000000000000'], ['3ff0000000000000']]}
        self.assertEqual(scientific_errors(result, expected), [])
        return expected, result

    def check_mutated_target(self, name):
        expected, result = self.result_fixture()
        target = expected['ols_reference']['targets'][name]['11']
        for value in [target + 1e-8, float('nan')]:
            bad = copy.deepcopy(result)
            bad['matrices'][name]['bits'][11] = struct.pack('>d', value).hex()
            self.assertTrue(any(f'independent OLS {name}/11' in error
                                for error in scientific_errors(bad, expected)))
        bad = copy.deepcopy(result)
        bad['matrices'][name]['bits'][10] = '0000000000000000'
        self.assertIn(f'independent OLS {name}/11: missing value', scientific_errors(bad, expected))
        rounded = copy.deepcopy(result)
        rounded['matrices'][name]['bits'][11] = struct.pack('>d', math.nextafter(target, math.inf)).hex()
        self.assertEqual(scientific_errors(rounded, expected), [])

    def test_changed_adjusted_r_squared_is_rejected(self):
        self.check_mutated_target('RFIT')

    def test_changed_condition_proxy_is_rejected(self):
        self.check_mutated_target('RCERT')


class IndependentBiasReferenceChecks(unittest.TestCase):
    def test_input_and_derivation_sources_are_bound_before_scope_is_allowed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pinned = json.loads(REFERENCE.read_text())
            reference = root / 'reference.json'
            for name in NAMES:
                case = root / (name + '.case')
                case.write_text('synthetic frozen input for binding validation')
                pinned['cases'][name]['input_sha256'] = digest(case)
                reference.write_text(json.dumps(pinned))
                original = {'outputs': {'sample': 0, 'drop_reason': 1}}
                expected = enrich_expected(case, original, reference)
                self.assertNotIn('bias', original)
                path = '/matrices/RBIAS/bits/3'
                entry = {'input_sha256': digest(case), 'reason': 'declared arithmetic change',
                         'allowed_paths': [path]}
                self.assertEqual(affected_prefixes({'cases': {name: entry}}, case, expected), [path])
                with self.assertRaises(RuntimeError):
                    affected_prefixes({'cases': {name: entry}}, case, original)
                case.write_text('changed input')
                with self.assertRaisesRegex(RuntimeError, 'input differs'):
                    enrich_expected(case, original, reference)
            pinned['sources']['scripts/ppml_bias_oracle.py'] = 'changed source'
            reference.write_text(json.dumps(pinned))
            with self.assertRaisesRegex(RuntimeError, 'source changed'):
                enrich_expected(case, original, reference)

    def bias_fixture(self, name):
        bias = json.loads(REFERENCE.read_text())['cases'][name]['bias']
        expected = {'return_code': 0, 'poisson': True, 'beta': [0.], 'covariance': [[1.]],
                    'outputs': {'sample': 0, 'residual': 1, 'mean': 2},
                    'sample': [1.], 'rows': [0], 'residual': [0.], 'fitted': [1.], 'bias': bias}
        diagnostic = [0.] * 21
        for index, value in {**bias['controls'], **bias['diagnostics']}.items():
            diagnostic[int(index)] = value
        def table(values, columns=1):
            return {'columns': columns, 'bits': [struct.pack('>d', x).hex() for x in values]}
        result = {'return_code': 0, 'matrices': {
            'RREC': table([1.]), 'RCERT': table([1.]), 'RB': table([0.]), 'RV': table([1.]),
            'RBIAS': table(bias['coefficients'], 3), 'RBDIAG': table(diagnostic),
            'RBSUPPORT': table(bias['support'], 11)},
            'columns': [['3ff0000000000000'], ['0000000000000000'], ['3ff0000000000000']]}
        return expected, result

    def test_every_bias_channel_support_and_numerical_diagnostic_is_checked(self):
        for name in NAMES:
            expected, result = self.bias_fixture(name)
            self.assertEqual(scientific_errors(result, expected), [])
            positions = ([('RBIAS', 3 * row) for row in range(9)] +
                         [('RBSUPPORT', i) for i in range(22)] +
                         [('RBDIAG', int(i)) for i in expected['bias']['diagnostics']] +
                         [('RBDIAG', int(i)) for i in expected['bias']['controls']])
            for table, index in positions:
                bad = copy.deepcopy(result)
                bad['matrices'][table]['bits'][index] = struct.pack('>d', 1000.).hex()
                self.assertTrue(scientific_errors(bad, expected), (name, table, index))

    def test_zero_slots_support_counts_and_controls_have_exact_checks(self):
        for name in NAMES:
            expected, result = self.bias_fixture(name)
            # Changes below the floating-point comparison allowance must still fail.
            for table, index in [('RBIAS', 2), ('RBSUPPORT', 6), ('RBDIAG', 4)]:
                bad = copy.deepcopy(result)
                bad['matrices'][table]['bits'][index] = struct.pack('>d', 1e-15).hex()
                self.assertTrue(scientific_errors(bad, expected), (name, table, index))
        expected, result = self.bias_fixture('ppml-bias-exact')
        result['matrices']['RBIAS']['bits'][15] = struct.pack('>d', 1e-15).hex()
        self.assertIn('exact bias uncertainty is nonzero', scientific_errors(result, expected))
