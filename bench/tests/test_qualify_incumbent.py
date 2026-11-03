"""An accepted historical scope must never excuse a new candidate difference."""
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify as q
import routing
import test_qualify
import test_routing_science
from test_cmgschur import bits


class IncumbentExact(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.config['scope'] = q.INCUMBENT_EXACT
        for role in ('incumbent', 'candidate'):
            self.config['artifacts'][role].update(families=['two', 'many'],
                public_policy=routing.GATES_V2, public_many_policy=routing.GEOMETRY_MANY)

    def plan(self):
        return test_qualify.Qualification.plan(self)

    def test_complete_round_and_report_keep_exact_candidate_contract(self):
        self.plan()
        record = q.run(self.manifest, self.output,
                       execute=lambda *a: test_qualify.Qualification.execute(self, *a))
        self.assertEqual(record['status'], 'complete', record['failures'])
        self.assertEqual(q.report(self.manifest, self.output)['status'], 'smoke_complete')

    def test_plan_rejects_new_family_policy_injection_or_hidden_arm(self):
        for mutation in (
            lambda c: c['artifacts']['candidate'].update(families=['two']),
            lambda c: c['artifacts']['candidate'].update(public_policy=routing.GATES),
            lambda c: c['artifacts']['candidate'].pop('public_many_policy'),
            lambda c: c['artifacts']['candidate'].update(request=routing.GEOMETRY_MANY),
            lambda c: c['artifacts'].update(hidden=dict(c['artifacts']['candidate'])),
            lambda c: c['artifacts']['incumbent'].update(public_many_policy='unknown'),
        ):
            changed = copy.deepcopy(self.config)
            mutation(changed)
            with self.subTest(config=changed), self.assertRaises(ValueError):
                q.plan(self.root / 'config.json', self.root / 'invalid-plan.json', configuration=changed)

    def test_public_many_policy_resolves_each_artifact_independently(self):
        meta = dict(self.meta, absorbed_terms=3)
        for role in ('incumbent', 'candidate'):
            self.assertEqual(q.resource_policy(self.config, role, meta), routing.GEOMETRY_MANY)
        self.assertIsNone(q.resource_policy(self.config, 'original', meta))
        historical = copy.deepcopy(self.config)
        historical['artifacts']['incumbent'].pop('public_many_policy')
        self.assertEqual(q.resource_policy(historical, 'incumbent', meta), 'incremental-many')

    def snapshots(self):
        return {role: ({'role': role}, None, {'fingerprint': fingerprint})
                for role, fingerprint in [('original', 'old'), ('incumbent', 'accepted'), ('candidate', 'accepted')]}

    def test_candidate_must_remain_exact_in_every_family(self):
        for effects in (0, 1, 2, 3, 16):
            snapshots = self.snapshots()
            snapshots['candidate'][2]['fingerprint'] = 'new'
            with patch.object(q.science, 'compatibility') as history:
                with self.subTest(effects=effects), self.assertRaisesRegex(ValueError, 'incumbent: unaffected'):
                    q.compare_artifacts(self.config, dict(self.meta, absorbed_terms=effects), snapshots)
                history.assert_not_called()

    def test_historical_allowance_checks_incumbent_and_cannot_expand_families(self):
        snapshots = self.snapshots()
        with patch.object(q.science, 'compatibility') as history:
            q.compare_artifacts(self.config, dict(self.meta, absorbed_terms=3), snapshots)
            self.assertIs(history.call_args.args[1], snapshots['incumbent'][0])
            self.assertEqual(history.call_args.kwargs['selected_policy'], routing.policy(routing.GEOMETRY_MANY))
        for effects in (0, 1):
            with self.subTest(effects=effects), self.assertRaisesRegex(ValueError, 'outside accepted historical'):
                q.compare_artifacts(self.config, dict(self.meta, absorbed_terms=effects), snapshots)

    def test_historical_rank_and_large_coefficient_errors_still_fail(self):
        raw = test_routing_science.PortfolioAutoScope.raw()
        columns = self.root / 'actual.columns'
        self.data.tofile(columns)
        meta = dict(self.meta, absorbed_terms=3)
        for name, index, value in [('RSTATUS', 1, 9.), ('RB', 0, 1.01)]:
            changed = copy.deepcopy(raw)
            changed['matrices'][name]['bits'][index] = bits(value)
            original = (raw, columns, {'fingerprint': 'old'})
            accepted = (changed, columns, {'fingerprint': 'accepted'})
            with self.subTest(name=name), self.assertRaises(ValueError):
                q.compare_artifacts(self.config, meta,
                    {'original': original, 'incumbent': accepted, 'candidate': accepted})

    def derived_snapshots(self):
        before = test_routing_science.PortfolioAutoScope.raw('map-no-lsmr')
        after = copy.deepcopy(before)
        solver = after['matrices']['RSOLVER']['bits']
        for column, value in {2: 0., 3: 5., **dict.fromkeys(range(10, 16), 0.)}.items():
            solver[16 + column] = bits(value)
        left, right = self.root / 'original.columns', self.root / 'incumbent.columns'
        self.data.tofile(left)
        self.data.tofile(right)
        old = before, left, {'fingerprint': 'old'}
        accepted = after, right, {'fingerprint': 'accepted'}
        return {'original': old, 'incumbent': accepted, 'candidate': accepted}

    def test_historical_one_fe_derived_row_requires_exact_columns_and_other_science(self):
        snapshots = self.derived_snapshots()
        meta = dict(self.meta, absorbed_terms=1, outputs={'sample': 0})
        with patch.object(q.science, 'compatibility') as history:
            q.compare_artifacts(self.config, meta, snapshots)
            history.assert_not_called()
        snapshots['incumbent'][1].write_bytes(snapshots['incumbent'][1].read_bytes() + b'changed')
        with self.assertRaisesRegex(ValueError, 'historical derived residual output columns'):
            q.compare_artifacts(self.config, meta, snapshots)

    def test_historical_derived_row_cannot_waive_new_candidate_or_other_changes(self):
        meta = dict(self.meta, absorbed_terms=1, outputs={'sample': 0})
        snapshots = self.derived_snapshots()
        snapshots['candidate'] = (*snapshots['candidate'][:2], {'fingerprint': 'new'})
        with patch.object(q.science, 'derived_residual_only') as derived:
            with self.assertRaisesRegex(ValueError, 'incumbent: unaffected'):
                q.compare_artifacts(self.config, meta, snapshots)
            derived.assert_not_called()
        for table, index, value in [('RSTATUS', 1, 9.), ('RB', 0, 1.01)]:
            snapshots = self.derived_snapshots()
            snapshots['incumbent'][0]['matrices'][table]['bits'][index] = bits(value)
            with self.subTest(table=table), self.assertRaisesRegex(ValueError, 'outside accepted historical'):
                q.compare_artifacts(self.config, meta, snapshots)

    def test_historical_derived_row_still_validates_request_certificate_and_route(self):
        meta = dict(self.meta, absorbed_terms=1, outputs={'sample': 0})
        for column, value in [(2, 1.), (3, 3.), (5, 1e-2), (10, 1.)]:
            snapshots = self.derived_snapshots()
            snapshots['incumbent'][0]['matrices']['RSOLVER']['bits'][16 + column] = bits(value)
            with self.subTest(column=column), self.assertRaisesRegex(ValueError, 'derived residual row tuple'):
                q.compare_artifacts(self.config, meta, snapshots)
        for changed in (dict(meta, outputs={'residual': 0}), dict(meta, saved=[1])):
            with self.subTest(meta=changed), self.assertRaisesRegex(ValueError, 'requires a request without'):
                q.compare_artifacts(self.config, changed, self.derived_snapshots())


if __name__ == '__main__':
    unittest.main()
