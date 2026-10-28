"""Derived residual rows in qualification scopes and the family-bound hidden arm."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify
import routing
import routing_science as science
from test_cmgschur import bits
from test_routing_science import PortfolioAutoScope

LEAF = '/matrices/RSOLVER/bits/{}'.format


def set_row(raw, index, fields):
    for column, value in fields.items():
        raw['matrices']['RSOLVER']['bits'][16 * index + column] = bits(float(value))


def projected():
    """Both rows certified projections of the two-FE direct route (used 2)."""
    return PortfolioAutoScope.raw('prefix')


def derived(reference=None):
    raw = copy.deepcopy(projected() if reference is None else reference)
    set_row(raw, 1, {1: 2, 2: 0, 3: 5, 4: 1e-16, 5: 2e-16, 6: 1e-16, 7: 0,
                     10: 0, 11: 0, 12: 0, 13: 0, 14: 0, 15: 0})
    return raw


def meta(terms=1, **outputs):
    return {'categorical_intercepts': True, 'absorbed_terms': terms, 'rows': 2,
            'outputs': {'sample': 4, **outputs}}


class DerivedResidualRows(unittest.TestCase):
    def test_leaves_name_only_the_route_fields_of_the_residual_row(self):
        before, after = projected(), derived()
        self.assertEqual(science.derived_residual_leaves(before, after, meta()), [LEAF(17)])
        changed_method = copy.deepcopy(before)
        set_row(changed_method, 1, {1: 4})
        self.assertEqual(science.derived_residual_leaves(changed_method, after, meta()), [LEAF(15), LEAF(17)])
        self.assertEqual(science.derived_residual_leaves(before, before, meta()), [])
        self.assertEqual(science.derived_residual_leaves(after, after, meta()), [])
        self.assertEqual(science.derived_residual_leaves({}, after, meta()), [])

    def test_invalid_or_undeclarable_rows_are_rejected(self):
        before, after = projected(), derived()
        with self.assertRaisesRegex(ValueError, 'certified projection'):
            science.derived_residual_leaves(after, before, meta())  # a projection replacing a derived row
        for label, mutate in [
                ('iterations', lambda r: set_row(r, 1, {2: 1})),
                ('method differs from absorption', lambda r: set_row(r, 1, {1: 3})),
                ('stop code 3', lambda r: set_row(r, 1, {3: 3})),
                ('pilot work', lambda r: set_row(r, 1, {14: 1})),
                ('fallback', lambda r: set_row(r, 1, {10: 1})),
                ('certificate above tolerance', lambda r: set_row(r, 1, {5: 2e-8})),
                ('return code', lambda r: r.update(return_code=430))]:
            changed = copy.deepcopy(after)
            mutate(changed)
            with self.subTest(label), self.assertRaises(ValueError):
                science.derived_residual_leaves(before, changed, meta())
        for label, requested in [('residual', meta(residual=5)), ('xbd', meta(xbd=5)),
                                 ('absorbed', meta(absorbed=5)), ('saved', {**meta(), 'saved': [5]})]:
            with self.subTest(label), self.assertRaisesRegex(ValueError, 'without residual'):
                science.derived_residual_leaves(before, after, requested)
        with self.assertRaisesRegex(ValueError, 'output metadata'):
            science.derived_residual_leaves(before, after, {'absorbed_terms': 1})

    def test_direct_and_geometry_scopes_admit_the_validated_row(self):
        before, after = projected(), derived()
        two = meta(2)
        scope = science.portfolio_scope(before, after, two, 'auto', routing.policy(routing.PORTFOLIO_UNSEEDED))
        self.assertEqual(set(scope) - set(science.replay_scope(after, 'auto', two)), {LEAF(17)})
        three = meta(3)
        scope = science.portfolio_scope(before, after, three, 'auto', routing.policy(routing.GEOMETRY_MANY))
        self.assertIn(LEAF(17), scope)
        for current in (two, three):
            with patch.object(science.cmgschur, 'scientific_comparison') as checked:
                science.compatibility(before, after, None, None, current, 'auto',
                                      selected_policy=routing.policy(routing.GEOMETRY_MANY if current is three
                                                                     else routing.PORTFOLIO_UNSEEDED))
                checked.assert_called_once()
        invalid = copy.deepcopy(after)
        set_row(invalid, 1, {2: 3})
        with self.assertRaises(ValueError):
            science.portfolio_scope(before, invalid, two, 'auto', routing.policy(routing.PORTFOLIO_UNSEEDED))

    def test_derived_only_requires_every_other_leaf_exact(self):
        before, after = projected(), derived()
        self.assertTrue(science.derived_residual_only(before, after, meta()))
        self.assertFalse(science.derived_residual_only(before, before, meta()))
        changed = copy.deepcopy(after)
        changed['matrices']['RB']['bits'][0] = bits(2.)
        self.assertFalse(science.derived_residual_only(before, changed, meta()))
        changed = copy.deepcopy(after)
        set_row(changed, 0, {2: 7})
        self.assertFalse(science.derived_residual_only(before, changed, meta()))

    def test_compare_artifacts_accepts_the_row_only_with_identical_columns(self):
        before, after = projected(), derived()
        manifest = {'scope': 'many', 'artifacts': {'candidate': {'families': ['many']}}}
        with tempfile.TemporaryDirectory() as directory:
            same, other = Path(directory, 'a.columns'), Path(directory, 'b.columns')
            same.write_bytes(b'\0' * 16)
            other.write_bytes(b'\1' * 16)
            reference = (before, same, {'fingerprint': 'one'})
            candidate = (after, same, {'fingerprint': 'two'})
            qualify.compare_artifacts(manifest, meta(), {'candidate': candidate, 'incumbent': reference})
            qualify.compare_artifacts(manifest, meta(), {'candidate': candidate, 'original': reference})
            with self.assertRaisesRegex(ValueError, 'unaffected'):
                qualify.compare_artifacts(manifest, meta(), {'candidate': (after, other, {'fingerprint': 'two'}),
                                                             'incumbent': reference})
            with self.assertRaisesRegex(ValueError, 'hidden'):
                qualify.compare_artifacts(manifest, meta(), {'candidate': candidate, 'hidden': reference})
            with self.assertRaisesRegex(ValueError, 'unaffected'):
                qualify.compare_artifacts({**manifest, 'scope': 'exact'}, meta(),
                                          {'candidate': candidate, 'incumbent': reference})
            with self.assertRaises(ValueError):
                invalid = copy.deepcopy(after)
                set_row(invalid, 1, {2: 1})
                qualify.compare_artifacts(manifest, meta(), {'candidate': (invalid, same, {'fingerprint': 'two'}),
                                                             'incumbent': reference})


class FamilyBoundHiddenArm(unittest.TestCase):
    def manifest(self, hidden=routing.GEOMETRY_MANY, candidate_policy=routing.GATES_V2):
        candidate = {'request': 'auto', 'families': ['two', 'many']}
        if candidate_policy:
            candidate['public_policy'] = candidate_policy
        return {'scope': 'many', 'artifacts': {
            'original': {'request': 'auto', 'families': []},
            'incumbent': {'request': 'auto', 'families': ['two'], 'public_policy': routing.GATES_V2},
            'candidate': candidate,
            'hidden': {'request': hidden, 'families': ['many']}}}

    def test_candidate_keeps_its_public_two_fe_policy_beside_a_many_hidden_arm(self):
        two, many, one = meta(2), meta(4), meta(1)
        manifest = self.manifest()
        self.assertEqual(qualify.resource_policy(manifest, 'candidate', two), routing.GATES_V2)
        self.assertEqual(qualify.resource_policy(manifest, 'candidate', many), routing.GEOMETRY_MANY)
        self.assertIsNone(qualify.resource_policy(manifest, 'candidate', one))
        self.assertEqual(qualify.resource_policy(self.manifest(candidate_policy=None), 'candidate', two), routing.GATES)
        self.assertEqual(qualify.resource_policy(manifest, 'incumbent', two), routing.GATES_V2)
        self.assertIsNone(qualify.resource_policy(manifest, 'incumbent', many))
        self.assertIsNone(qualify.resource_policy(manifest, 'hidden', two))
        self.assertEqual(qualify.resource_policy(manifest, 'hidden', many), routing.GEOMETRY_MANY)
        self.assertIsNone(qualify.resource_policy(manifest, 'original', two))

    def test_two_fe_hidden_arm_still_binds_the_candidate_on_two_fe_cells(self):
        manifest = self.manifest(hidden=routing.GATES_V2, candidate_policy=None)
        manifest['artifacts']['hidden']['families'] = ['two']
        self.assertEqual(qualify.resource_policy(manifest, 'candidate', meta(2)), routing.GATES_V2)
        self.assertEqual(qualify.resource_policy(manifest, 'candidate', meta(4)), 'incremental-many')


class ManyScopePlan(unittest.TestCase):
    """A public candidate may declare both families with a many-FE hidden arm."""

    def setUp(self):
        import test_qualify
        test_qualify.Qualification.setUp(self)
        self.config['scope'] = 'many'
        self.config['artifacts']['incumbent'].update(families=['two'], public_policy=routing.GATES_V2)
        self.config['artifacts']['candidate'].update(request='auto', families=['two', 'many'],
                                                     public_policy=routing.GATES_V2)
        self.config['artifacts']['hidden'] = dict(self.config['artifacts']['candidate'],
                                                  request=routing.GEOMETRY_MANY, families=['many'])
        del self.config['artifacts']['hidden']['public_policy']

    def plan(self):
        import test_qualify
        for role, artifact in self.config['artifacts'].items():
            (self.root / role).write_bytes(('distinct artifact ' + role).encode())
            artifact['path'] = role
        return test_qualify.Qualification.plan(self)

    def test_plan_accepts_the_declaration_and_rejects_a_two_fe_hidden_arm(self):
        self.plan()
        manifest = qualify.read(self.manifest)
        self.assertEqual(manifest['artifacts']['candidate']['families'], ['two', 'many'])
        self.assertEqual(qualify.resource_policy(manifest, 'candidate', meta(2)), routing.GATES_V2)
        self.assertEqual(qualify.resource_policy(manifest, 'candidate', meta(3)), routing.GEOMETRY_MANY)
        self.config['artifacts']['hidden'].update(request=routing.GATES_V2, families=['two'])
        self.manifest.unlink()
        with self.assertRaisesRegex(ValueError, 'selected hidden'):
            self.plan()


if __name__ == '__main__':
    unittest.main()
