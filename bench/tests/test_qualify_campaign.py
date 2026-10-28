"""Campaign coverage, blocked categorical oracle and public two-FE policy declarations."""
import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cmgschur
import qualify as q
import routing
import routing_cases
import routing_oracle as oracle
import test_qualify
from native_io import digest
import test_qualify_policy

OUTPUT_NAMES = ('sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed')


def akm(workers=60, firms=8, periods=5, k=3, seed=5, stayer_firm=False, third=False):
    """A small AKM panel; optionally one firm whose workers never move."""
    rng = np.random.default_rng(seed)
    worker = np.repeat(np.arange(workers), periods)
    firm = np.empty((workers, periods), dtype=np.int64)
    firm[:, 0] = np.arange(workers) % firms
    for t in range(1, periods):
        move = rng.random(workers) < .3
        firm[:, t] = np.where(move, rng.integers(0, firms, workers), firm[:, t-1])
    firm = firm.ravel()
    if stayer_firm:
        # Workers 0..2 always work at a new firm nobody else visits.
        firm[:3*periods] = firms
    factors = [worker, firm]
    if third:
        factors.append((firm + 3*worker + np.arange(len(worker)) % periods) % 4)
    n = len(worker)
    x = rng.standard_normal((n, k))
    y = x @ np.linspace(.5, -.5, k) + np.sin(worker) + .4*np.cos(firm) + .3*rng.standard_normal(n)
    return y, x, factors


def full_dummy(y, x, factors, robust):
    """Observation-by-dummy pseudo-inverse with an absorbed-constant normalization."""
    dummies = np.column_stack([np.eye(len(np.unique(f)))[np.unique(f, return_inverse=True)[1]] for f in factors])
    design = np.column_stack([x, dummies])
    inverse = np.linalg.pinv(design, rcond=1e-12)
    full = inverse @ y
    residual = y - design @ full
    k, n = x.shape[1], len(y)
    transform = np.zeros((k+1, design.shape[1]))
    transform[:k, :k] = np.eye(k)
    transform[-1, k:] = dummies.mean(axis=0)
    influence = transform @ inverse
    rank = np.linalg.matrix_rank(design)
    if robust:
        covariance = (influence*residual) @ (influence*residual).T * n/(n-rank)
    else:
        covariance = influence @ influence.T * (residual @ residual)/(n-rank)
    return transform @ full, covariance, residual, rank - k


def blocked(y, x, factors, **options):
    z = np.asfortranarray(np.column_stack([y, x]))
    return oracle.solve_blocked(z, factors, **options)


class BlockedOracle(unittest.TestCase):
    def test_one_two_and_three_fe_match_full_dummy_regression(self):
        designs = {'one': lambda: (lambda y, x, f: (y, x, f[:1]))(*akm()),
                   'two': akm, 'three-stayer': lambda: akm(stayer_firm=True, third=True)}
        for name, make in designs.items():
            for vce in ('iid', 'robust'):
                with self.subTest(design=name, vce=vce):
                    y, x, factors = make()
                    reference, arrays = blocked(y, x, factors, vce=vce)
                    beta, covariance, residual, rank = full_dummy(y, x, factors, vce == 'robust')
                    np.testing.assert_allclose(reference['beta'], beta, rtol=1e-9, atol=1e-11)
                    np.testing.assert_allclose(reference['covariance'], covariance, rtol=1e-8, atol=1e-12)
                    np.testing.assert_allclose(arrays['residual'], residual, rtol=1e-9, atol=1e-11)
                    np.testing.assert_allclose(arrays['fitted'], y - residual, rtol=1e-9, atol=1e-11)
                    self.assertEqual(reference['true_fe_rank'], rank)
                    certificate = reference['certificate']
                    self.assertEqual(certificate['method'], 'group-means' if name == 'one'
                                     else 'eliminated-schur-shifted-cholesky')
                    oracle._blocked_certificate(certificate, len(factors), x.shape[1])

    def test_stayer_firm_is_a_pair_component_gauge_not_an_alias(self):
        y, x, factors = akm(stayer_firm=True, third=True)
        reference, _ = blocked(y, x, factors)
        certificate = reference['certificate']
        self.assertEqual(certificate['components'], 1)
        self.assertEqual(certificate['pair_components'][0], 2)
        self.assertEqual(certificate['known_nullity'], sum(certificate['pair_components']))
        self.assertEqual(reference['pairwise_fe_rank'], reference['true_fe_rank'])
        with self.assertRaisesRegex(ValueError, 'nonpositive diagonal|alias'):
            oracle.solve(y, x, factors, include_intercept=True, vce='iid')

    def test_two_fe_blocked_and_dense_paths_agree(self):
        y, x, factors = akm()
        dense, dense_arrays = oracle.solve(y, x, factors, vce='iid')
        reference, arrays = blocked(y, x, factors, vce='iid')
        for key in ('dof_rows', 'fe_rows', 'true_fe_rank', 'pairwise_fe_rank'):
            self.assertEqual(reference[key], dense[key])
        np.testing.assert_allclose(reference['beta'], dense['beta'], rtol=1e-11, atol=1e-13)
        np.testing.assert_allclose(reference['covariance'], dense['covariance'], rtol=1e-10, atol=1e-14)
        for key in ('residual', 'xb', 'absorbed'):
            np.testing.assert_allclose(arrays[key], dense_arrays[key], rtol=1e-11, atol=1e-12)

    def test_additional_alias_and_memory_model_refuse(self):
        y, x, factors = akm(third=True)
        with self.assertRaisesRegex(ValueError, 'alias|conditioning|positive definite'):
            blocked(y, x, [factors[0], factors[1], factors[1] + 100])
        with patch.object(oracle, 'blocked_memory_model', return_value=(1 << 30) + 1), \
                self.assertRaisesRegex(ValueError, 'modeled memory'):
            blocked(y, x, factors)
        self.assertGreater(oracle.blocked_memory_model(1_000_000, 1_000_000, 64, 2, 200_000, 10_000, 9_999),
                           8 * 1_000_000 * 65 + 8 * 9_999**2)

    def test_certificate_tampering_and_cluster_requests_fail(self):
        y, x, factors = akm()
        reference, _ = blocked(y, x, factors)
        certificate = reference['certificate']
        for key, value in (('shift', certificate['schur_certificate']['shift'] * 2),
                           ('minimum_eigenvalue_lower_bound', 1.),
                           ('factor_frobenius_squared', 0.)):
            changed = copy.deepcopy(certificate)
            changed['schur_certificate'][key] = value
            with self.subTest(field=key), self.assertRaisesRegex(ValueError, 'rank certificate'):
                oracle._blocked_certificate(changed, 2, x.shape[1])
        with self.assertRaisesRegex(ValueError, 'HC1 or unit-weight IID'):
            blocked(y, x, factors, vce='cluster')
        with self.assertRaisesRegex(ValueError, 'Fortran'):
            oracle.solve_blocked(np.column_stack([y, x]), factors)

    def write_case(self, path, y, x, factors, outputs):
        n, k, q = len(y), x.shape[1], len(factors)
        inputs = k + q + 2
        fields = dict(term_count=str(k), absorb_term_count=str(q), vce='iid', dof='pairwise',
                      account_cluster_nesting='0', cluster_term_count='0', cluster_component_count='0',
                      model_major='2', model_minor='1', keep_singletons='0', include_intercept='1', threads='1',
                      output_saved_channels='|'.join(['1:0']*q))
        fields.update({'output_'+key: str(inputs+outputs.index(key)+1) if key in outputs else '0'
                       for key in OUTPUT_NAMES})
        tables = {'source_matrix': [[j, 0] for j in range(1, k+q+1)],
                  'term_matrix': [[j, j, j, 1, 1, 0, 0, 0] for j in range(1, k+1)],
                  'component_matrix': [[j, 1, j, 1, 1, 0, 0, 2, 1, j, j, 0, 0] for j in range(1, k+1)],
                  'absorb_term_matrix': [[j, 1, 1, 0] for j in range(1, q+1)],
                  'absorb_component_matrix': [[j, 1, 1, k+j] for j in range(1, q+1)],
                  'binding_matrix': [[1, 1, 0], [2, 2, 0], *[[j+2, 3, j] for j in range(1, k+q+1)]]}
        fields.update({key: key for key in tables})
        arguments = ['estimate-ols', *[key+'='+value for key, value in fields.items()]]

        def values(stream, column):
            stream.write(''.join(struct.pack('>d', float(v)).hex()+'\n' for v in np.asarray(column).ravel()).encode())
        with path.open('wb') as stream:
            stream.write(f'FEREG_REPLAY_V1\n{n} {inputs+len(outputs)} {len(arguments)} {len(tables)} 0\n'.encode())
            for arg in arguments:
                stream.write(f'{len(arg)}\n{arg}\n'.encode())
            for column in [np.ones(n), y, *x.T, *factors, *[np.zeros(n)]*len(outputs)]:
                stream.write(b'0\n')
                values(stream, column)
            for key, table in tables.items():
                array = np.asarray(table)
                stream.write(f'{len(key)}\n{key}\n{array.shape[0]} {array.shape[1]}\n'.encode())
                values(stream, array)

    def test_sample_only_case_reference_rereads_bound_input_and_checks_published_fields(self):
        y, x, factors = akm()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for outputs in (OUTPUT_NAMES, ('sample',)):
                path = root / f'case-{len(outputs)}.case'
                self.write_case(path, y, x, factors[:1], list(outputs))
                parsed = oracle.read_case(path, blocked=True)
                self.assertTrue(parsed['z'].flags.f_contiguous)
                np.testing.assert_array_equal(parsed['z'][:, 1:], x)
                plain = oracle.read_case(path)
                self.assertTrue(plain['x'].flags.c_contiguous)
                np.testing.assert_array_equal(plain['x'], x)
                self.assertEqual(list(parsed['outputs']), list(outputs))
                output = root / f'reference-{len(outputs)}.json'
                reference = oracle.write_reference(path, output)
                self.assertEqual(reference['kind'], oracle.BLOCKED_KIND)
                loaded, _ = oracle.load_reference(output, reference['case_sha256'], reference['body_sha256'])
                meta = dict(rows=len(y), columns=parsed['columns'], outputs=parsed['outputs'])

                def table(values):
                    v = np.atleast_2d(values)
                    return dict(rows=v.shape[0], columns=v.shape[1],
                                bits=[struct.pack('>d', float(e)).hex() for e in v.ravel()])
                dof = np.zeros((20, 2))
                for key, value in loaded['dof_rows'].items():
                    dof[int(key)] = [1, value]
                status = np.zeros((x.shape[1]+1, 4))
                status[:, 1] = [1]*x.shape[1] + [5]
                raw = {'matrices': {'RB': table(loaded['beta']), 'RV': table(loaded['covariance']),
                                    'RFE': table(loaded['fe_rows']), 'RDOF': table(dof), 'RSTATUS': table(status)}}
                with np.load(loaded['_arrays_path']) as arrays:
                    published = {'sample': arrays['sample'], 'residual': arrays['residual'],
                                 'drop_reason': arrays['drop_reason'], 'xb': arrays['xb'],
                                 'xbd': arrays['fitted'], 'absorbed': arrays['absorbed']}
                data = np.zeros((parsed['columns'], len(y)))
                for name, position in parsed['outputs'].items():
                    data[position] = published[name]
                binary = root / 'columns'
                data.tofile(binary)
                oracle.check_reference(loaded, raw, binary, meta)
                bad = copy.deepcopy(raw)
                bad['matrices']['RV']['bits'][0] = struct.pack('>d', 7.).hex()
                with self.assertRaisesRegex(ValueError, 'covariance'):
                    oracle.check_reference(loaded, bad, binary, meta)
                tampered = json.loads(output.read_text())
                tampered['certificate']['known_nullity'] += 1
                output.write_text(json.dumps(tampered))
                with self.assertRaisesRegex(ValueError, 'rank/resource'):
                    oracle.load_reference(output, reference['case_sha256'], reference['body_sha256'])


class CampaignCases(unittest.TestCase):
    def case(self, path, count, outputs, slopes=False):
        args = [b'estimate-ols', b'threads=1', b'output_saved_channels=1:0', f'absorb_term_count={count}'.encode(),
                b'absorb_term_matrix=A'] + [f'output_{name}={outputs.get(name, 0)}'.encode() for name in OUTPUT_NAMES]
        prefix = f'FEREG_REPLAY_V1\n2 {max(outputs.values())} {len(args)} 1 0\n'.encode()
        framed = b''.join(str(len(a)).encode() + b'\n' + a + b'\n' for a in args)
        column = b'0\n' + b'0000000000000000\n' * 2
        term = [[j, 1, 1, float(slopes)] for j in range(1, count+1)]
        table = (f'1\nA\n{count} 4\n'.encode()
                 + b''.join(struct.pack('>d', v).hex().encode() + b'\n' for row in term for v in row))
        path.write_bytes(prefix + framed + column * max(outputs.values()) + table)

    def test_campaign_families_check_terms_and_declared_output_variant(self):
        self.assertEqual(len(routing_cases.CAMPAIGN_CASES), 8)
        self.assertFalse(set(routing_cases.CAMPAIGN_CASES) & q.FAMILIES)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'claimed.case'
            full = {name: 4 + i for i, name in enumerate(OUTPUT_NAMES)}
            self.case(path, 1, full)
            meta = routing_cases.inspect_case(path, 'campaign-one-fe')
            self.assertEqual(meta['outputs'], {name: 3 + i for i, name in enumerate(OUTPUT_NAMES)})
            for wrong in ('campaign-one-fe-nosave', 'campaign-akm-k16'):
                with self.subTest(claimed=wrong), self.assertRaises(ValueError):
                    routing_cases.inspect_case(path, wrong)
            self.case(path, 2, {'sample': 4})
            self.assertEqual(routing_cases.inspect_case(path, 'campaign-akm-k16-nosave')['outputs'], {'sample': 3})
            with self.assertRaisesRegex(ValueError, 'variant'):
                routing_cases.inspect_case(path, 'campaign-akm-k16')
            self.case(path, 3, full, slopes=True)
            with self.assertRaisesRegex(ValueError, 'categorical'):
                routing_cases.inspect_case(path, 'campaign-three-dense')

    def test_campaign_witness_binds_identifiers_and_reports_platform_draws(self):
        record = json.loads(routing_cases.CAMPAIGN_RECORD.read_text())
        key = 'ols-two-dense-n1000000'
        expected = record['datasets'][key]
        metadata = {'column_sha256': dict(expected['column_sha256']), 'rows': expected['rows'],
                    'seed': expected['seed'], 'topology': expected['topology']}
        self.assertEqual(routing_cases.campaign_witness(key, metadata, False)['differing_float_columns'], [])
        metadata['column_sha256']['x2'] = '0' * 64
        metadata['column_sha256']['y'] = '0' * 64
        self.assertEqual(routing_cases.campaign_witness(key, metadata, False)['differing_float_columns'], ['y', 'x2'])
        for field, value in (('g1', '0' * 64), ('cluster', '0' * 64)):
            changed = copy.deepcopy(metadata)
            changed['column_sha256'][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'identifiers or topology'):
                routing_cases.campaign_witness(key, changed, False)
        self.assertEqual(routing_cases.campaign_witness(key, {}, True)['status'], 'not applicable: small tooling fixture')

    def test_campaign_coverage_takes_no_imports_or_routing_seed(self):
        for extra in (['--seed', '1'], ['--source', 'no-fe=x'], ['--low-saved', 'x']):
            with self.subTest(extra=extra), patch.object(sys, 'argv', ['routing_cases.py', '--coverage', 'campaign',
                                                                       '--output', 'unused', *extra]), \
                    patch('sys.stderr'), self.assertRaises(SystemExit) as caught:
                routing_cases.main()
            self.assertEqual(caught.exception.code, 2)

    def test_sample_only_outputs_compare_without_missing_channels(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = np.array([[1., 1.], [.2, -.2], [.3, .1]])
            left, right = root / 'left', root / 'right'
            data.tofile(left)
            data.tofile(right)
            meta = {'rows': 2, 'columns': 3, 'outputs': {'sample': 0}}
            raw = test_qualify.result()
            cmgschur.scientific_comparison(raw, raw, left, right, meta)
            changed = data.copy()
            changed[2, 0] = .4
            changed.tofile(right)
            with self.assertRaisesRegex(ValueError, 'input/sample/drop'):
                cmgschur.scientific_comparison(raw, raw, left, right, meta)


class Coverage(unittest.TestCase):
    setUp, plan = test_qualify.Qualification.setUp, test_qualify.Qualification.plan

    def campaign(self, families, platform='Darwin', blocks=6):
        self.config['cases'] = [{'id': family, 'family': family, 'input': 'input.case', 'oracle': {'kind': 'dense'},
                                 'memory_limit_bytes': None} for family in families]
        self.config.update(coverage='campaign', platform=platform, blocks=blocks)

    def test_campaign_and_combined_coverages_require_their_complete_family_sets(self):
        self.assertEqual(len(q.COVERAGES['routing-campaign']), 29)
        self.campaign(sorted(q.CAMPAIGN_FAMILIES))
        self.plan()
        self.campaign(sorted(q.CAMPAIGN_FAMILIES)[1:])
        self.manifest.unlink()
        with self.assertRaisesRegex(ValueError, 'campaign qualification requires all 16 workload cells'):
            self.plan()
        self.campaign(sorted(q.CAMPAIGN_FAMILIES), blocks=2)
        with self.assertRaisesRegex(ValueError, 'balanced blocks'):
            self.plan()
        self.campaign(sorted(q.FAMILIES | q.CAMPAIGN_FAMILIES))
        self.config['coverage'] = 'routing-campaign'
        self.plan()
        self.assertEqual(len(q.read(self.manifest)['cases']) * 2, 58)


class PublicPolicy(unittest.TestCase):
    evidence = test_qualify_policy.PolicyEvidence
    setUp, plan, execute = evidence.setUp, evidence.plan, evidence.execute
    completed, public = evidence.completed, evidence.public

    def declare(self, policy):
        self.config['artifacts']['incumbent'].update(families=['many', 'two'])
        if policy:
            self.config['artifacts']['incumbent']['public_policy'] = policy
        self.config['artifacts']['candidate'].update(request=routing.GATES_V2, families=['many', 'two'])
        self.presets = {'candidate': 'routing_two_gates_v2', 'incumbent': 'routing_two_gates_v2'}

    def test_declared_public_policy_attests_the_incumbent_gate_preset(self):
        self.declare(routing.GATES_V2)
        record = self.completed()
        incumbent = next(c for c in record['cases'][0]['warmups'] if c['execution'] == 'incumbent')
        self.assertIn('gate_events', incumbent)
        self.assertEqual({a['preset'] for a in incumbent['action_resources']}, {'routing_two_gates_v2'})

    def test_undeclared_policy_keeps_gates_v1_and_stops_on_a_v2_incumbent(self):
        self.declare(None)
        self.assertEqual(q.resource_policy(self.config, 'incumbent', self.meta), routing.GATES)
        self.plan()
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertIn('requested preset', record['failures'][-1]['error'])

    def test_policy_field_is_family_scoped_and_hidden_binding_is_unchanged(self):
        self.declare(routing.GATES_V2)
        self.assertEqual(q.resource_policy(self.config, 'incumbent', self.meta), routing.GATES_V2)
        self.assertEqual(q.resource_policy(self.config, 'incumbent', dict(self.meta, absorbed_terms=4)),
                         'incremental-many')
        self.public()
        self.config['artifacts']['candidate']['public_policy'] = routing.GATES
        self.config['artifacts']['hidden']['request'] = routing.GATES_V2
        self.assertEqual(q.resource_policy(self.config, 'candidate', self.meta), routing.GATES_V2)

    def test_plan_rejects_unknown_or_non_public_policy_declarations(self):
        for role, policy, request in (('incumbent', 'two-gates-v3', 'auto'),
                                      ('incumbent', routing.GATES_V2, 'preserve'),
                                      ('candidate', routing.GATES_V2, routing.GATES_V2)):
            with self.subTest(role=role, policy=policy, request=request):
                self.setUp()
                self.declare(None)
                self.config['artifacts'][role].update(public_policy=policy, request=request)
                with self.assertRaisesRegex(ValueError, 'public two-FE policy'):
                    self.plan()


if __name__ == '__main__':
    unittest.main()
