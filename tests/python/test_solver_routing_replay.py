"""The supplementary native qualification scope cannot hide route/model drift."""
import copy
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
import replay_solver_routing as routing_replay


def bits(value):
    return struct.pack('>d', value).hex()


def fixture():
    def table(length, columns=1):
        return {'columns': columns, 'bits': [bits(0.)] * length}
    result = {'return_code': 0, 'matrices': {
        'RB': table(2), 'RV': table(4, 2), 'RFIT': table(24),
        'RCERT': table(64), 'RSOLVER': table(32, 16), 'RRUNTIME': table(60),
        'RMEMORY': table(1), 'RDOF': table(40), 'RSTATUS': table(8, 4)},
        'columns': [[bits(1.)], [bits(0.)], [bits(2.)], [bits(3.)]]}
    expected = {'poisson': False,
                'outputs': {'sample': 0, 'drop_reason': 1, 'residual': 2}, 'saved': [3]}
    return result, expected


def direct_fixture():
    result, expected = fixture()
    expected['saved'] = [2, 3]
    expected['rows'] = list(range(128))
    expected['direct_reference'] = {
        'require_admission': True, 'rhs': 65, 'finite_rank': 63,
        'reported_columns': 64, 'outcome_columns': 1,
        'absorbed_rank': 33, 'residual_dof': 32, 'weighted_rss': 1.,
        'saved_channels': [[2.], [3.]]}
    result['matrices']['RSTATUS']['bits'] = [
        bits(value) for status in [1.] * 63 + [5.] for value in [0., status, 0., 0.]]
    dof = result['matrices']['RDOF']['bits']
    for row, value in {0: 128, 1: 128, 2: 128, 3: 128, 5: 63, 7: 33,
                       8: 96, 9: 32, 10: 32, 19: 0}.items():
        dof[2 * row:2 * row + 2] = [bits(1.), bits(value)]
    result['matrices']['RFIT']['bits'][:2] = [bits(1.), bits(1.)]
    return result, expected


class SolverRoutingReplay(unittest.TestCase):
    def test_finite_panel_preserves_both_memory_modes_and_no_unsupported_injections(self):
        pairs = routing_replay.PAIRS
        self.assertEqual(len(pairs), 23)
        self.assertEqual(len(pairs), len(set(pairs)))
        self.assertEqual({source for source, arm in pairs if arm == 'it'},
                         set(routing_replay.IT_CASES))
        for source in ('ols-08', 'ols-25'):
            self.assertTrue(set(routing_replay.ROUTE_ARMS) <=
                            {arm for name, arm in pairs if name == source})
        self.assertTrue({('ols-00', 'h4-k1'), ('ols-04', 'h4-k1'),
                         ('ols-20', 'h4-k1')} <= set(pairs))

    def test_scope_admits_only_numeric_outputs_and_named_inner_iterations(self):
        result, expected = fixture()
        # Missing numeric fields must not acquire a fictitious finite witness.
        result['matrices']['RCERT']['bits'][11] = bits(8.98846567431158e307)
        paths = routing_replay.numerical_paths(result, expected)
        self.assertIn('/matrices/RSOLVER/bits/2', paths)
        self.assertIn('/matrices/RSOLVER/bits/16', paths)
        self.assertIn('/columns/2', paths)
        self.assertIn('/columns/3', paths)
        for protected in ('/columns/0', '/columns/1', '/matrices/RSOLVER/bits/1',
                          '/matrices/RCERT/bits/11', '/matrices/RCERT/bits/63',
                          '/matrices/RSTATUS/bits/1', '/runtime_signature/0'):
            self.assertNotIn(protected, paths)
        with tempfile.TemporaryDirectory() as directory:
            case = Path(directory) / 'bounded.case'
            case.write_text('input identity only; no native execution')
            scope = {'cases': {case.stem: {'input_sha256': routing_replay.replay.digest(case),
                'reason': 'numerical algorithm change', 'allowed_paths': paths}}}
            self.assertEqual(routing_replay.replay.affected_prefixes(scope, case, expected), paths)
            routing_replay.replay.validate_scope_targets(paths, result, False)

    def test_allowed_saved_coefficients_do_not_hide_omission_or_missingness(self):
        result, expected = fixture()
        protected = routing_replay.replay.discrete_view(result, expected)
        changed = copy.deepcopy(result)
        changed['columns'][3][0] = bits(8.98846567431158e307)
        self.assertNotEqual(protected, routing_replay.replay.discrete_view(changed, expected))
        changed = copy.deepcopy(result)
        changed['matrices']['RSTATUS']['bits'][1] = bits(1.)
        self.assertNotEqual(protected, routing_replay.replay.discrete_view(changed, expected))

    def test_direct_trace_requires_actual_full_width_certification_within_budget(self):
        _, expected = direct_fixture()
        event = ('FEREG_SOLVER_TRACE|phase=auto_direct_certified|rows=128|rhs=65|'
                 'iterations=0|work=200|limit=300')
        self.assertEqual(routing_replay.direct_trace_errors(event, expected), [])
        for invalid in ('', event.replace('certified', 'route'),
                        event.replace('rhs=65', 'rhs=1'), event.replace('rhs=65', 'rhs=64'),
                        event.replace('rows=128', 'rows=64'),
                        event.replace('work=200', 'work=301'), event.replace('work=200', 'work=0'),
                        event.replace('iterations=0', 'iterations=1'),
                        event.replace('limit=300', 'limit=invalid'), event.replace('|limit=300', '')):
            with self.subTest(invalid=invalid):
                self.assertTrue(routing_replay.direct_trace_errors(invalid, expected))
        expected['direct_reference']['require_admission'] = False
        self.assertEqual(routing_replay.direct_trace_errors('', expected), [])
        self.assertTrue(routing_replay.direct_trace_errors(event, expected))

    def test_direct_contract_distinguishes_reported_columns_and_packed_rhs(self):
        _, expected = direct_fixture()
        expected.update(rows=list(range(128)), beta=[0.] * 64, covariance=[[0.] * 64] * 64)
        routing_replay.validate_direct_contract(expected, 'direct-unit-fast')
        for field, value in [('rhs', 64), ('rhs', 66), ('reported_columns', 63),
                             ('outcome_columns', 0)]:
            changed = copy.deepcopy(expected)
            changed['direct_reference'][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                routing_replay.validate_direct_contract(changed, 'direct-unit-fast')
        changed = copy.deepcopy(expected)
        changed['covariance'][0].pop()
        with self.assertRaises(ValueError):
            routing_replay.validate_direct_contract(changed, 'direct-unit-fast')

    def test_independent_direct_witness_protects_gauge_rank_and_omission(self):
        result, expected = direct_fixture()
        self.assertEqual(routing_replay.direct_science_errors(result, expected), [])
        changed = copy.deepcopy(result)
        # The saved sum remains identical, but the individual gauge is wrong.
        changed['columns'][2][0], changed['columns'][3][0] = bits(2.5), bits(2.5)
        self.assertTrue(routing_replay.direct_science_errors(changed, expected))
        changed = copy.deepcopy(result)
        changed['columns'][3] = []
        self.assertTrue(routing_replay.direct_science_errors(changed, expected))
        for matrix, index in [('RDOF', 11), ('RDOF', 19), ('RSTATUS', 1), ('RFIT', 1)]:
            changed = copy.deepcopy(result)
            changed['matrices'][matrix]['bits'][index] = bits(0.)
            self.assertTrue(routing_replay.direct_science_errors(changed, expected))

    def test_candidate_only_it_injection_preserves_framed_body(self):
        from native_io import case_header
        with tempfile.TemporaryDirectory() as directory:
            source, candidate = [Path(directory) / name for name in ('parent.case', 'candidate.case')]
            args = [b'estimate-ols', b'threads=4', b'solver=it']
            header = b'FEREG_REPLAY_V1\n1 1 3 1 0\n' + b''.join(
                str(len(arg)).encode() + b'\n' + arg + b'\n' for arg in args)
            source.write_bytes(header + b'opaque unchanged numeric model/data body\n')
            routing_replay.prepare_injected_case(source, candidate, 'it_coefficients')
            with source.open('rb') as stream:
                _, before = case_header(stream)
            with candidate.open('rb') as stream:
                _, after = case_header(stream)
            self.assertEqual(after, before + [b'inject=it_coefficients'])
            self.assertEqual(routing_replay.body_digest(source), routing_replay.body_digest(candidate))
            with self.assertRaises(ValueError):
                routing_replay.prepare_injected_case(candidate, Path(directory) / 'twice.case',
                                                      'it_coefficients')
            with self.assertRaises(ValueError):
                routing_replay.prepare_injected_case(source, Path(directory) / 'wrong.case',
                                                      'ppml_topology')

    def test_ppml_topology_requires_accepted_reuse_under_changed_weights(self):
        def event(kind, weights, accepted, pending, topology=2):
            return (f'FEREG_SOLVER_TRACE|phase=ppml_topology|event={kind}|sample=1|'
                    f'topology={topology}|weights={weights}|accepted={accepted}|'
                    f'pending={pending}|bytes=256')
        built = event('built', 3, 0, 1)
        accepted = event('accepted', 3, 1, 0)
        reused = event('reused', 4, 1, 1)
        self.assertEqual(routing_replay.topology_trace_errors('\n'.join([built, accepted, reused]), True), [])
        for events in ([built, reused], [accepted, reused], [built, accepted],
                       [built, accepted, event('reused', 3, 1, 1)],
                       [built, accepted, event('reused', 4, 1, 1, topology=9)],
                       [built, accepted, event('reused', 4, 0, 1)],
                       [built, accepted, reused.replace('bytes=256', 'bytes=invalid')]):
            self.assertTrue(routing_replay.topology_trace_errors('\n'.join(events), True))
        self.assertEqual(routing_replay.topology_trace_errors('', False), [])
        self.assertEqual(len(routing_replay.TOPOLOGY_CASES), 4)
        self.assertTrue({'ppml-bias-exact', 'ppml-bias-randomized', 'ppml-23'} <=
                        set(routing_replay.TOPOLOGY_FROZEN))

    def test_retired_overlap_and_routing_stages_are_not_injected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'plain.case'
            args = [b'estimate-ols', b'threads=2', b'solver=auto']
            source.write_bytes(b'FEREG_REPLAY_V1\n1 1 3 1 0\n' + b''.join(
                str(len(arg)).encode() + b'\n' + arg + b'\n' for arg in args) + b'fixed body\n')
            for stage in ('routing_h8_k1', 'routing_overlap_h8_k1', 'routing_builder_break'):
                with self.assertRaises(ValueError):
                    routing_replay.prepare_injected_case(source, Path(directory) / (stage + '.case'),
                                                        stage)
        self.assertFalse(hasattr(routing_replay, 'prepare_overlap'))


if __name__ == '__main__':
    unittest.main()
