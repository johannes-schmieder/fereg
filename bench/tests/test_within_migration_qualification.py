"""Dependency migration gates, without native builds or benchmark measurements."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify
import routing_science as science
import test_qualify
import test_routing_science
from test_cmgschur import bits


ACTION = ('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_many_geometry_v1|'
          'iterations=10|native_limit=100|memory_bytes=64|memory_limit=128|final=0|certified=1\n')
RHS = ('FEREG_SOLVER_TRACE|phase=lsmr_rhs|rhs=0|converged=true|iterations=10|'
       'within_normal=1e-15|fereg_relinf=1e-15\n')


class MigrationScience(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.meta = {'rows': 2, 'columns': 6, 'absorbed_terms': 3, 'categorical_intercepts': True,
                     'saved': [], 'outputs': dict(zip(
                         ('sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed'), range(6)))}
        self.columns = self.root / 'columns'
        self.data = np.array([[1., 1.], [.2, -.2], [0., 0.], [.3, .4], [.5, .2], [.2, -.2]])
        self.data.tofile(self.columns)
        self.raw = test_routing_science.PortfolioAutoScope.raw()

    def snapshot(self, raw, trace=ACTION + RHS, columns=None, meta=None):
        columns = columns or self.columns
        detail = science.within_migration_trace(trace, self.meta if meta is None else meta)
        detail['fingerprint'] = qualify.routing.identity({**qualify.exact_view(raw, False),
                                                         'columns_sha256': qualify.digest(columns)})
        return raw, columns, detail

    def compare(self, raw, *, trace=ACTION + RHS, columns=None, meta=None):
        meta = self.meta if meta is None else meta
        science.within_migration_comparison(self.snapshot(self.raw, meta=meta),
            self.snapshot(raw, trace, columns, meta), meta)

    def test_numerical_and_inner_work_changes_keep_existing_scientific_bounds(self):
        changed = copy.deepcopy(self.raw)
        for table, index in [('RB', 0), ('RV', 0), ('RFIT', 1)]:
            changed['matrices'][table]['bits'][index] = bits(1. + 1e-12)
        changed['matrices']['RSOLVER']['bits'][2] = bits(11.)
        changed['matrices']['RSOLVER']['bits'][5] = bits(2e-15)
        columns = self.root / 'changed.columns'
        data = self.data.copy()
        data[1, 0] += 1e-12
        data.tofile(columns)
        self.compare(changed, columns=columns)
        changed['matrices']['RB']['bits'][0] = bits(1.01)
        with self.assertRaisesRegex(ValueError, 'coefficients: tolerance'):
            self.compare(changed, columns=columns)

    def test_model_discretes_routes_and_tolerances_are_never_waived(self):
        for table, index in [('RSTATUS', 1), ('RFE', 0), ('RDOF', 3), ('RREC', 0),
                             ('RCERT', 0), ('RRUNTIME', 45)] + [
                ('RSOLVER', index) for index in (0, 1, 3, 10, 11, 12, 13, 14)]:
            changed = copy.deepcopy(self.raw)
            changed['matrices'][table]['bits'][index] = bits(9.)
            with self.subTest(table=table, index=index), self.assertRaises(ValueError):
                self.compare(changed)
        for column in (0, 2):
            data = self.data.copy()
            data[column, 0] += 1.
            target = self.root / 'decision.columns'
            data.tofile(target)
            with self.subTest(column=column), self.assertRaisesRegex(ValueError, 'input/sample/drop'):
                self.compare(self.raw, columns=target)
        for index, value in [(2, 101.), (5, 2e-8), (21, 2e-14)]:
            changed = copy.deepcopy(self.raw)
            changed['matrices']['RSOLVER']['bits'][index] = bits(value)
            with self.subTest(index=index, value=value), self.assertRaises(ValueError):
                self.compare(changed)

    def test_pilot_only_or_missing_trace_requires_exact_science(self):
        self.compare(self.raw, trace=ACTION)
        changed = copy.deepcopy(self.raw)
        changed['matrices']['RB']['bits'][0] = bits(1. + 1e-12)
        for trace in (ACTION, ACTION + RHS):
            left = self.snapshot(self.raw, ACTION)
            right = self.snapshot(changed, trace)
            with self.subTest(trace=trace), self.assertRaisesRegex(ValueError, 'requires exact science'):
                science.within_migration_comparison(left, right, self.meta)
        with self.assertRaisesRegex(ValueError, 'requires exact science'):
            self.compare(changed, trace=ACTION)
        left, right = self.snapshot(self.raw), self.snapshot(changed)
        right[2].pop('within_migration')
        with self.assertRaisesRegex(ValueError, 'requires exact science'):
            science.within_migration_comparison(left, right, self.meta)

    def test_slope_two_fe_and_malformed_traces_cannot_claim_migrated_path(self):
        for meta in [{**self.meta, 'absorbed_terms': n} for n in (0, 1, 2)] + [
                {**self.meta, 'categorical_intercepts': False}]:
            with self.subTest(meta=meta):
                detail = science.within_migration_trace('', meta)['within_migration']
                self.assertFalse(detail['eligible'] or detail['executed'])
                with self.assertRaisesRegex(ValueError, 'ineligible'):
                    science.within_migration_trace(ACTION + RHS, meta)
        for trace in [RHS, ACTION + RHS.replace('iterations=10', 'iterations=-1'),
                      ACTION + RHS.replace('within_normal=1e-15', 'within_normal=nan'),
                      ACTION + RHS.replace('|rhs=0', ''), ACTION.replace('certified=1', 'certified=0')]:
            with self.subTest(trace=trace), self.assertRaises(ValueError):
                science.within_migration_trace(trace, self.meta)

    def test_derived_residual_route_is_not_implicitly_declared(self):
        changed = copy.deepcopy(self.raw)
        changed['matrices']['RSOLVER']['bits'][19] = bits(5.)
        with self.assertRaises(ValueError):
            self.compare(changed)


class MigrationRunner(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.config['scope'] = science.WITHIN_MIGRATION
        self.config['artifacts']['candidate']['families'] = [science.WITHIN_MIGRATION]
        self.meta['absorbed_terms'] = 3
        self.config['cases'][0]['family'] = 'four-fe'
        self.trace = ACTION + RHS

    def plan(self):
        return test_qualify.Qualification.plan(self)

    def execute(self, command, log, timeout):
        resources = test_qualify.Qualification.execute(self, command, log, timeout)
        qualify.write(Path(command[-1]), {**test_routing_science.PortfolioAutoScope.raw(), 'columns': []})
        if 'FEREG_SOLVER_TRACE=1' in command:
            with log.open('a') as stream:
                stream.write(self.trace)
        return resources

    def test_public_mode_uses_existing_hash_binding_and_report_rechecks_path(self):
        manifest = self.plan()
        self.assertEqual(manifest['files'][manifest['cases'][0]['input']], qualify.digest(self.source))
        record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record['failures'])
        self.assertEqual(qualify.report(self.manifest, self.output)['status'], 'smoke_complete')
        warmup = record['cases'][0]['warmups'][0]
        self.assertTrue(warmup['within_migration']['executed'])
        path = self.output / 'run-0-0' / warmup['trace_log']
        path.write_text(path.read_text().replace(RHS, ''))
        report = qualify.report(self.manifest, self.output)
        self.assertEqual(report['status'], 'needs_discussion')
        self.assertTrue(any('path evidence' in error for error in report['errors']))

    def test_pilot_only_round_carries_no_dependency_cost_claim(self):
        self.trace = ACTION
        self.plan()
        record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record['failures'])
        self.assertTrue(all(not call['within_migration']['executed']
                            for cell in record['cases'] for call in cell['warmups']))

    def test_numerical_change_without_path_trace_stops_before_measured_blocks(self):
        candidate = self.root / 'candidate'
        candidate.write_bytes(b'distinct candidate')
        self.config['artifacts']['candidate']['path'] = 'candidate'
        self.trace = ACTION
        self.plan()

        def execute(command, log, timeout):
            resources = self.execute(command, log, timeout)
            if Path(command[-4]) == candidate:
                target = Path(command[-1])
                raw = qualify.read(target)
                raw['matrices']['RB']['bits'][0] = bits(1. + 1e-12)
                qualify.write(target, raw)
            return resources

        record = qualify.run(self.manifest, self.output, execute=execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertIn('requires exact science', record['failures'][-1]['error'])
        self.assertEqual(len(self.calls), 2)  # Only the two declared warmups.
        self.assertEqual(record['cases'][0]['blocks'], [])

    def test_unrelated_families_and_private_requests_cannot_enter_migration_plan(self):
        original = copy.deepcopy(self.config)
        for mutate in [lambda c: c['artifacts']['candidate'].update(families=['many']),
                       lambda c: c['artifacts']['candidate'].update(request='many-geometry-v1'),
                       lambda c: c['artifacts']['candidate'].update(public_policy='two-gates-v2'),
                       lambda c: c['artifacts']['incumbent'].update(families=[science.WITHIN_MIGRATION]),
                       lambda c: c['artifacts']['candidate'].update(families=[])]:
            self.config = copy.deepcopy(original)
            mutate(self.config)
            with self.subTest(config=self.config), self.assertRaises(ValueError):
                self.plan()


if __name__ == '__main__':
    unittest.main()
