"""Small request-framing and fixed-policy contract tests."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
from native_io import case_header
from test_native import case_bytes


class RoutingRequests(unittest.TestCase):
    def test_framed_route_injection_changes_options_only_and_baseline_is_plain_auto(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'source.case'
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=cmgschur',
                                           b'cmgsamples=2', b'cmgseed=5', b'cmgdegree=4']))
            for arm in ['baseline', 'h32-k2']:
                target = root / (arm + '.case')
                routing.prepare(source, target, arm, 4)
                with target.open('rb') as stream:
                    _, args = case_header(stream)
                    self.assertEqual(stream.read(), b'body\n9\nthreads=1\n')
                fields = dict(a.decode().split('=', 1) for a in args[1:])
                self.assertEqual(fields['solver'], 'auto')
                self.assertEqual(fields['threads'], '4')
                self.assertNotIn('cmgsamples', fields)
                self.assertEqual(fields.get('inject'), None if arm == 'baseline' else 'routing_h32_k2')
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'inject=allocation']))
            with self.assertRaisesRegex(ValueError, 'already contains'):
                routing.prepare(source, root / 'rejected.case', 'h4-k0', 1)
            self.assertFalse((root / 'rejected.case').exists())


    def test_unseeded_headers_preserve_model_and_v1_header_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); source = root / 'source.case'
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'memory=low', b'tolerance=1e-8']))
            old, new = root / 'v1.case', root / 'unseeded.case'
            routing.prepare(source, old, routing.PORTFOLIO_V1, 4)
            routing.prepare(source, new, routing.PORTFOLIO_UNSEEDED, 4)
            self.assertEqual(new.read_bytes(), old.read_bytes().replace(
                b'27\ninject=routing_portfolio_v1', b'33\ninject=routing_portfolio_unseeded'))
            with new.open('rb') as stream:
                _, arguments = case_header(stream)
            self.assertIn(b'memory=low', arguments)
            self.assertIn(b'tolerance=1e-8', arguments)
            self.assertIn(b'preconditioner=auto', arguments)


    def test_new_presets_preserve_model_body_and_do_not_stack_injections(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'source.case'
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=auto']))
            for name in [*routing.PORTFOLIOS, *routing.INCREMENTAL, *routing.REMAINING, routing.GATES, *routing.PREFIXES, 'overlap-h8-k1', routing.COMPACT_IT]:
                target = root / (name + '.case')
                routing.prepare(source, target, name, 4)
                with target.open('rb') as stream:
                    _, args = case_header(stream)
                    self.assertEqual(stream.read(), b'body\n9\nthreads=1\n')
                fields = dict(a.decode().split('=', 1) for a in args[1:])
                self.assertEqual(fields['solver'], 'it' if name == routing.COMPACT_IT else 'auto')
                self.assertEqual(fields['inject'], 'it_coefficients' if name == routing.COMPACT_IT else routing.policy(name)['injection'])
                with self.assertRaisesRegex(ValueError, 'already contains'):
                    routing.prepare(target, root / ('stacked-' + name), name, 4)


    def test_request_rejects_ppml_break_injection_bad_workers_and_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, target = root / 'source', root / 'target'
            for arguments in [[b'estimate-ppml', b'threads=1'],
                              [b'estimate-ols', b'threads=1', b'break_marker_token=1']]:
                source.write_bytes(case_bytes(arguments))
                with self.subTest(arguments=arguments), self.assertRaises(ValueError):
                    routing.prepare(source, target, 'baseline', 1)
                self.assertFalse(target.exists())
            source.write_bytes(case_bytes())
            for workers in [True, 0, 1025, 1.5]:
                with self.subTest(workers=workers), self.assertRaises(ValueError):
                    routing.prepare(source, target, 'baseline', workers)
            target.write_text('retained')
            with self.assertRaises(FileExistsError):
                routing.prepare(source, target, 'baseline', 1)
            self.assertEqual(target.read_text(), 'retained')

    def test_policy_rejects_invalid_arms(self):
        self.assertIsNone(routing.policy('baseline'))
        for name in ['overlap-h4-k0', 'h16-k3', 'composed-map4', 'routing_composed_map4']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                routing.policy(name)

    def test_policy_types_and_direct_work_budget_are_part_of_identity(self):
        for name in [*routing.INCREMENTAL, *routing.PORTFOLIOS]:
            for path, values in [(('steps',), [False, 0.0]), (('two_fe', 'trees'), [True, 1.0]),
                    (('two_fe', 'small_direct', 'preflight_work'), [0, 1024.0, True]),
                    (('two_fe', 'small_direct', 'structural_iteration_allowance'), [5, 4.0, True]),
                    (('two_fe', 'small_direct', 'choice'), ['direct_then_cmg'])]:
                for value in values:
                    selected = routing.policy(name)
                    owner = selected
                    for field in path[:-1]:
                        owner = owner[field]
                    owner[path[-1]] = value
                    with self.subTest(name=name, path=path, value=value):
                        self.assertNotEqual(routing.identity(selected), routing.identity(routing.policy(name)))
        selected = routing.policy('h16-k1')
        selected['trees'] = True
        self.assertNotEqual(routing.identity(selected), routing.identity(routing.policy('h16-k1')))

    def test_unseeded_policy_does_not_mutate_the_retained_policy(self):
        old = routing.policy(routing.PORTFOLIO_V1)
        self.assertEqual(routing.identity(old), 'daca7ef0e399c7fa8aadaef6de43e24b1b1b326383578dbc532f9225bbcfecec')
        selected = routing.policy(routing.PORTFOLIO_UNSEEDED)
        self.assertEqual(selected, {**old, 'prefix': 'none', 'steps': 0, 'map_sweeps': 0,
                                   'injection': 'routing_portfolio_unseeded'})
        selected['two_fe']['trees'] = 2
        self.assertEqual(routing.policy(routing.PORTFOLIO_V1), old)
        self.assertEqual(routing.policy(routing.PORTFOLIO_UNSEEDED)['two_fe']['trees'], 1)

    def test_geometry_many_policy_extends_the_unseeded_portfolio_identity(self):
        selected = routing.policy(routing.GEOMETRY_MANY)
        self.assertEqual(selected['injection'], 'routing_many_geometry_v1')
        self.assertEqual(selected['family'], 'many')
        base = routing.policy(routing.PORTFOLIO_UNSEEDED)
        self.assertEqual((base['certificate'], selected['certificate']), ('private_auto', 'requested'))
        changed = {'injection', 'certificate'}
        self.assertEqual({k: v for k, v in selected.items() if k in base and k not in changed},
                         {k: v for k, v in base.items() if k not in changed})
        self.assertNotEqual(routing.identity(selected), routing.identity(routing.policy('incremental-many')))

    def test_incremental_scope_requires_inspected_categorical_metadata(self):
        for count in [0, 1, 2, 3, 16, True, 3.0, None]:
            for categorical in [False, True, 1]:
                meta = {'absorbed_terms': count, 'categorical_intercepts': categorical}
                self.assertEqual(routing.incremental_eligible(meta, 'many'),
                                 categorical is True and type(count) is int and count >= 3)
                self.assertEqual(routing.incremental_eligible(meta, 'incremental-two'),
                                 categorical is True and type(count) is int and count == 2)
        with self.assertRaises(ValueError):
            routing.incremental_eligible({}, 'unknown')

    def test_replay_envelope_and_json_reject_mutation_and_duplicate_keys(self):
        envelope = routing.seal({'request': 'auto', 'workers': 4})
        self.assertEqual(routing.unseal(envelope), {'request': 'auto', 'workers': 4})
        envelope['value']['workers'] = 8
        with self.assertRaisesRegex(ValueError, 'changed'):
            routing.unseal(envelope)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'manifest.json'
            path.write_text('{"schema":1,"schema":2}')
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                routing.read_json(path)
            path.write_text(json.dumps({'schema': 1}))
            self.assertEqual(routing.read_json(path), {'schema': 1})


if __name__ == '__main__':
    unittest.main()
