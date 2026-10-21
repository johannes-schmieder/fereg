"""Synthetic guards for the fixed diagnostic adapter; never invoke native code."""
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cmg_setup as setup
import qualify as q
import test_qualify
from test_qualify import RecoveryMonitor


class SetupDiagnostic(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.config = {'platform': 'Darwin', 'seed': 2026102101, 'harness': 'harness',
                       'artifacts': {}, 'cases': []}
        for role in setup.ARMS:
            plugin = self.root / (role + '.plugin')
            plugin.write_bytes((setup.PRE if role == setup.PRE else setup.POST).encode())
            self.config['artifacts'][role] = {'path': plugin.name, 'revision': setup.REVISIONS[role], 'build': 'synthetic'}
        (self.root / 'oracle.json').write_text('{"arrays":{"file":"oracle.npz"}}')
        (self.root / 'oracle.npz').write_bytes(b'bound synthetic oracle')
        for name in setup.CASES:
            self.config['cases'].append({'id': name, 'family': name, 'input': 'input.case',
                                        'oracle': {'kind': 'routing', 'path': 'oracle.json'}, 'memory_limit_bytes': None})
        self.manifest = self.root / 'plan.json'
        # Only the large-input admission is replaced; run/report/schedule/fingerprints
        # and native timer parsing are the real maintained machinery.
        self.input_guard = patch.object(setup, 'validate_case')
        self.input_guard.start()
        self.compatibility = patch.object(q.science, 'compatibility').start()
        RecoveryMonitor.hits = []
        RecoveryMonitor.quiet_failure = False
        patch('qualify_scc.MacMonitor', RecoveryMonitor).start()

    def plan(self):
        config = self.root / 'config.json'
        q.write(config, self.config)
        return setup.prepare(config, self.manifest)

    def execute(self, command, log, timeout):
        return test_qualify.Qualification.execute(self, command, log, timeout)

    def test_fixed_budgets_and_source_bindings(self):
        value = self.plan()
        self.assertEqual(value['mac_cell_policy']['baseline_calls'], 900)
        self.assertEqual(value['mac_cell_policy']['new_call_cap'], 900)
        self.assertEqual(value['mac_cell_policy']['max_cell_retries'], 0)
        self.assertEqual(value['mac_cell_policy']['max_retry_cells'], 0)
        self.assertEqual(setup.fixed('Linux')['replicas'], 6)
        self.assertEqual(12 * 3 * (1 + 2 * setup.fixed('Linux')['blocks']) * 6, 1080)
        self.assertEqual(value['execution_sources']['cmg_setup'], q.digest(Path(setup.__file__)))
        self.assertEqual(value['execution_sources']['qualify'], q.digest(Path(q.__file__)))
        self.assertEqual(set(q.artifact_aliases(value).values()), set(setup.ARMS))
        self.assertEqual(q.comparison_pairs(value), ((setup.PRE, setup.POST), (setup.PUBLIC, setup.POST)))

    def test_full_shared_run_report_true_names_and_complete_call_timer(self):
        value = self.plan()
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record.get('failures'))
        self.assertEqual(len(self.calls), 900)
        self.assertEqual(self.oracle.call_count, 36)  # 4 families * 3 workers * 3 arms.
        self.assertEqual(self.compatibility.call_count, 24)
        for cell in record['cases']:
            self.assertEqual(set(cell['aliases']), set(setup.ARMS))
            for row in cell['blocks']:
                self.assertEqual(len(row['calls']), 6)
                self.assertTrue(all(c['seconds'] == .01 for c in row['calls']))
        report = q.report(self.manifest, self.output)
        self.assertEqual(report['status'], 'diagnostic_complete', report['errors'])
        self.assertIs(report['activation_qualified'], False)
        self.assertEqual(report['native_calls'], 900)
        self.assertEqual(len(report['comparisons']), 24)
        self.assertTrue(all(c['status'] == 'passed' for c in report['comparisons']))
        self.assertEqual(set(c['reference'] for c in report['comparisons']), {setup.PRE, setup.PUBLIC})
        self.assertEqual(value['warmups'], 1)

    def test_schedule_and_mode_mutations_rejected(self):
        value = self.plan()
        for field, changed in [('diagnostic', 'other'), ('threads', [1, 4]), ('blocks', 14),
                               ('budget_s', 15000), ('scope', 'exact'), ('replicas', 2)]:
            other = copy.deepcopy(value)
            other[field] = changed
            with self.subTest(field=field), self.assertRaises(ValueError):
                q.diagnostic_mode(other)
        for marker in ('continuation', 'recovery'):
            with self.assertRaises(ValueError):
                setup.validate({**value, marker: {}})
        with self.assertRaises(ValueError):
            q.run(self.manifest, self.output, resume=True, execute=self.execute)
        self.assertFalse(self.calls)
        other = copy.deepcopy(value)
        del other['mac_cell_policy']
        with self.assertRaisesRegex(ValueError, 'observation policy'):
            q.validate_cell_policy(other)

    def test_ordinary_role_validation_not_bypassed(self):
        value = self.plan()
        config = self.root / 'bad.json'
        source = {k: v for k, v in value.items() if k in {'scope', 'platform', 'artifacts', 'harness', 'cases',
                  'threads', 'blocks', 'shards', 'replicas', 'seed', 'timeout_s', 'budget_s', 'coverage'}}
        q.write(config, source)
        with self.assertRaisesRegex(ValueError, 'required artifact roles'):
            q.plan(config, self.root / 'bad-plan.json')

    def test_science_failure_retained_without_native_retry(self):
        self.plan()
        self.compatibility.side_effect = ValueError('independent scoped science failed')
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(record['failures'][-1]['kind'], 'scientific')
        self.assertEqual(len(self.calls), 3)
        self.assertFalse(record['retry_cells'])

    def test_fixed_worker_repeat_change_stops(self):
        self.plan()
        def changed(command, log, timeout):
            if len(self.calls) == 3:
                self.data[1, 0] += .01
            return self.execute(command, log, timeout)
        record = q.run(self.manifest, self.output, execute=changed)
        self.assertEqual(len(self.calls), 4)
        self.assertEqual(record['failures'][-1]['kind'], 'scientific')
        self.assertIn('fixed-worker repeat changed', record['failures'][-1]['error'])

    def test_interference_exhausts_zero_retry_budget(self):
        self.plan()
        RecoveryMonitor.hits = [True]
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(len(self.calls), 75)
        self.assertEqual(len(record['discarded_attempts']), 1)
        self.assertFalse(record['retry_cells'])

    def test_scoped_comparison_uses_existing_policy_and_no_fingerprint_equality(self):
        value = self.plan()
        snapshots = {role: (role, role + '.columns', {'fingerprint': role}) for role in setup.ARMS}
        setup.compare_science(value, self.meta, snapshots)
        self.assertEqual(self.compatibility.call_count, 2)
        for call in self.compatibility.call_args_list:
            self.assertEqual(call.args[1], setup.POST)
            self.assertEqual(call.kwargs, {'selected_policy': q.routing.policy(q.routing.PORTFOLIO_UNSEEDED)})

    def test_cross_worker_decision_change_stops(self):
        self.plan()
        count = 0
        def signature(_):
            nonlocal count
            count += 1
            return 'a' * 64 if count <= 3 else 'b' * 64
        with patch.object(q, 'worker_signature', side_effect=signature):
            record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(len(self.calls), 78)
        self.assertEqual(record['failures'][-1]['kind'], 'scientific')
        self.assertIn('cross-worker discrete decisions changed', record['failures'][-1]['error'])

    def test_artifact_drift_stops_before_native(self):
        self.plan()
        (self.root / (setup.POST + '.plugin')).write_bytes(b'changed')
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertFalse(self.calls)
        self.assertIn('input identity mismatch', record['failures'][-1]['error'])

    def test_two_fe_existing_scope_allows_iteration_changes(self):
        from test_cmgschur import result
        scope = q.science.replay_scope(result(), 'auto', self.meta)
        self.assertIn('/matrices/RSOLVER/bits/2', scope)
        self.assertNotIn('/matrices/RSOLVER/bits/3', scope)  # stopping decision

    def test_same_current_binary_required(self):
        (self.root / (setup.PUBLIC + '.plugin')).write_bytes(b'wrong current artifact')
        with self.assertRaisesRegex(ValueError, 'same actual binary'):
            self.plan()
        self.assertFalse(self.manifest.exists())

    def test_input_admission_rejects_wrong_seed_low_and_hash(self):
        self.input_guard.stop()
        self.source.with_suffix('.json').write_text('{"seed": 7}')
        with self.assertRaisesRegex(ValueError, 'seed'):
            setup.validate_case(self.source, self.config['cases'][0])
        self.source.with_suffix('.json').write_text('{"seed":2026101821}')
        with patch.object(q, 'case_header', return_value=([100000, 12], [b'estimate-ols', b'memory=default'])), \
             patch.object(q.routing_cases, 'inspect_case', return_value={'absorbed_terms': 2, 'categorical_intercepts': True}):
            with self.assertRaisesRegex(ValueError, 'memory mode'):
                setup.validate_case(self.source, self.config['cases'][1])
        with patch.object(q, 'case_header', return_value=([600000, 12], [b'estimate-ols', b'memory=default'])), \
             patch.object(q.routing_cases, 'inspect_case', return_value={'absorbed_terms': 2, 'categorical_intercepts': True}):
            with self.assertRaisesRegex(ValueError, 'not reproduced'):
                setup.validate_case(self.source, self.config['cases'][0])


class SetupCandidateDiagnostic(unittest.TestCase):
    plan = SetupDiagnostic.plan
    execute = SetupDiagnostic.execute

    def setUp(self):
        SetupDiagnostic.setUp(self)
        self.config.update(diagnostic=setup.CANDIDATE_MODE, seed=setup.CANDIDATE_SEED)
        # The shared two-row fixture has no FE route. Supply explicit empty
        # trace evidence; actual two-FE calls obtain these fields from the parser.
        read_native = q.read_native
        def read_fixture(*args, **kwargs):
            raw, columns, detail = read_native(*args, **kwargs)
            return raw, columns, {**detail, 'route_trace': [], 'route_hierarchy': []}
        patch.object(q, 'read_native', side_effect=read_fixture).start()
        self.config['artifacts'] = {}
        for role in setup.CANDIDATE_ARMS:
            plugin = self.root / (role + '.plugin')
            plugin.write_bytes(role.encode())
            self.config['artifacts'][role] = {'path': plugin.name, 'build': 'synthetic',
                'revision': setup.REVISIONS[setup.POST] if role == setup.ACCEPTED else 'a' * 40}

    def test_complete_exact_two_arm_budget_and_report(self):
        value = self.plan()
        self.assertEqual(value['scope'], 'exact')
        self.assertEqual(value['mac_cell_policy']['new_call_cap'], 600)
        self.assertEqual(value['mac_cell_policy']['max_cell_retries'], 0)
        self.assertEqual(value['mac_cell_policy']['max_retry_cells'], 0)
        self.assertEqual(q.comparison_pairs(value), ((setup.ACCEPTED, setup.SETUP),))
        self.assertTrue(all(a['request'] == 'incremental-two' for a in value['artifacts'].values()))
        linux = setup.fixed('Linux', setup.CANDIDATE_MODE)
        self.assertEqual(12 * 2 * (1 + 2 * linux['blocks']) * linux['replicas'], 720)
        self.assertEqual(value['execution_sources']['cmg_setup'], q.digest(Path(setup.__file__)))
        self.assertEqual(value['execution_sources']['qualify'], q.digest(Path(q.__file__)))
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record['failures'])
        self.assertEqual(len(self.calls), 600)
        self.assertEqual(self.oracle.call_count, 24)
        self.compatibility.assert_not_called()
        for cell in record['cases']:
            self.assertEqual(set(cell['aliases'].values()), set(setup.CANDIDATE_ARMS))
            for block in cell['blocks']:
                self.assertEqual(len(block['calls']), 4)
                self.assertTrue(all(c['seconds'] == .01 for c in block['calls']))
        report = q.report(self.manifest, self.output)
        self.assertEqual(report['status'], 'diagnostic_complete', report['errors'])
        self.assertEqual(report['target'], setup.SETUP)
        self.assertFalse(report['activation_qualified'])
        self.assertEqual(report['native_calls'], 600)
        self.assertEqual(len(report['comparisons']), 12)

    def test_strict_mode_revision_seed_and_alias_admission(self):
        value = self.plan()
        for field, changed in [('seed', 2026102103), ('scope', 'two'), ('blocks', 13), ('diagnostic', setup.MODE)]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                setup.validate({**value, field: changed})
        for role, revision in [(setup.ACCEPTED, 'b' * 40), (setup.SETUP, '7b610e1'), (setup.SETUP, 'HEAD')]:
            other = copy.deepcopy(value)
            other['artifacts'][role]['revision'] = revision
            with self.subTest(role=role, revision=revision), self.assertRaises(ValueError):
                setup.validate(other)
        other = copy.deepcopy(value)
        other['files'][other['artifacts'][setup.SETUP]['path']] = other['files'][other['artifacts'][setup.ACCEPTED]['path']]
        with self.assertRaisesRegex(ValueError, 'distinct actual binaries'):
            setup.validate(other)
        for marker in ('continuation', 'recovery'):
            with self.assertRaises(ValueError):
                setup.validate({**value, marker: {}})
        with self.assertRaises(ValueError):
            q.run(self.manifest, self.output, resume=True, execute=self.execute)
        self.assertFalse(self.calls)

    def test_cross_arm_exact_failure_retained_without_retry(self):
        self.plan()
        def changed(command, log, timeout):
            if len(self.calls) == 1:
                self.data[1, 0] += .01
            return self.execute(command, log, timeout)
        record = q.run(self.manifest, self.output, execute=changed)
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(len(self.calls), 2)
        self.assertIn('exact science/solver fingerprint', record['failures'][-1]['error'])
        self.assertEqual(record['failures'][-1]['kind'], 'scientific')
        self.assertFalse(record['retry_cells'])

    def test_iteration_change_is_exact_failure(self):
        from test_cmgschur import result, bits
        value = self.plan()
        raw, changed = result(), result()
        changed['matrices']['RSOLVER']['bits'][2] = bits(123.)
        def snapshot(head):
            return (head, None, {'fingerprint': q.routing.identity(q.exact_view(head, False))})
        with self.assertRaisesRegex(ValueError, 'exact science/solver fingerprint'):
            setup.compare_science(value, self.meta,
                                  {setup.ACCEPTED: snapshot(raw), setup.SETUP: snapshot(changed)})

    def test_current_hierarchy_stays_mandatory(self):
        log = HistoricalTraceDiagnostics.ROUTE + HistoricalTraceDiagnostics.PRIMARY
        for role in setup.CANDIDATE_ARMS:
            with self.subTest(role=role), self.assertRaises(ValueError):
                setup.resource_diagnostics(log, role, self.config['artifacts'][role]['revision'])

    def test_valid_route_and_hierarchy_changes_fail_with_unchanged_science(self):
        from test_cmgschur import result
        from test_routing_science import RouteHierarchyTrace as traces
        value = self.plan()
        action = HistoricalTraceDiagnostics.PRIMARY.replace('setup_work=794664056|setup_limit=977928192|', '')
        def snapshot(route):
            detail = setup.resource_diagnostics(route + action, setup.ACCEPTED, setup.REVISIONS[setup.POST])
            return (result(), None, {**detail, 'fingerprint': 'identical-science'})
        baseline = snapshot(traces.INSTALLED)
        changes = [traces.INSTALLED.replace('repeats=3;0', 'repeats=4;0'),
                   traces.INSTALLED.replace('terminal_factor_nonzeros=19', 'terminal_factor_nonzeros=20'),
                   traces.INSTALLED.replace('application=scalar|parallel_operators=0',
                                            'application=parallel|parallel_operators=1'),
                   traces.REFUSED]
        for route in changes:
            with self.subTest(route=route), self.assertRaisesRegex(ValueError, 'exact route_trace changed'):
                setup.compare_science(value, self.meta, {setup.ACCEPTED: baseline, setup.SETUP: snapshot(route)})
        # Test the independently stored parsed hierarchy too, rather than relying
        # on the redundant raw trace to detect every change.
        changed = copy.deepcopy(baseline)
        changed[2]['route_hierarchy'][0]['repeats'][0] += 1
        with self.assertRaisesRegex(ValueError, 'exact route_hierarchy changed'):
            setup.compare_science(value, self.meta, {setup.ACCEPTED: baseline, setup.SETUP: changed})
        refused = snapshot(traces.REFUSED)
        with self.assertRaisesRegex(ValueError, 'exact route_trace changed'):
            setup.compare_science(value, self.meta, {setup.ACCEPTED: refused,
                setup.SETUP: snapshot(traces.REFUSED.replace('Some(MemoryBudget)', 'Some(Unused)'))})

    def test_equal_empty_routes_pass_but_missing_evidence_fails(self):
        from test_cmgschur import result
        value = self.plan()
        snapshot = (result(), None, {'fingerprint': 'same', 'route_trace': [], 'route_hierarchy': []})
        setup.compare_science(value, self.meta, {setup.ACCEPTED: snapshot, setup.SETUP: snapshot})
        telemetry = copy.deepcopy(snapshot)
        telemetry[2].update(seconds=99., maxrss_bytes=123456, action_resources=[{'memory_bytes_used': 999}])
        setup.compare_science(value, self.meta, {setup.ACCEPTED: snapshot, setup.SETUP: telemetry})
        for field in ('route_trace', 'route_hierarchy'):
            missing = copy.deepcopy(snapshot)
            del missing[2][field]
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'missing ' + field):
                setup.compare_science(value, self.meta, {setup.ACCEPTED: snapshot, setup.SETUP: missing})

    def test_source_auto_auto_is_required_even_with_exact_scope(self):
        from test_cmgschur import case_bytes
        self.input_guard.stop()
        self.source.with_suffix('.json').write_text('{"seed":2026101821}')
        for solver, preconditioner in [('lsmr', 'auto'), ('auto', 'off')]:
            self.source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', ('solver=' + solver).encode(),
                                               ('preconditioner=' + preconditioner).encode()]))
            with self.subTest(solver=solver, preconditioner=preconditioner), self.assertRaisesRegex(ValueError, 'Auto/Auto'):
                self.plan()
        self.assertFalse(self.calls)

    def test_repeat_failure_does_not_create_retries(self):
        self.plan()
        def changed(command, log, timeout):
            if len(self.calls) == 2:
                self.data[1, 0] += .01
            return self.execute(command, log, timeout)
        record = q.run(self.manifest, self.output, execute=changed)
        self.assertEqual(len(self.calls), 3)
        self.assertIn('fixed-worker repeat changed', record['failures'][-1]['error'])
        self.assertFalse(record['retry_cells'])

    def test_interference_retained_with_zero_recovery(self):
        self.plan()
        RecoveryMonitor.hits = [True]
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(len(self.calls), 50)
        self.assertEqual(len(record['discarded_attempts']), 1)
        self.assertFalse(record['retry_cells'])


class HistoricalTraceDiagnostics(unittest.TestCase):
    # Captured d5b public-enabled high-degree weak route and both action records.
    ROUTE = ('FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1|iterations=16|'
             'setup_work=794664056|setup_limit=977928192|installed=1|refusal=None\n')
    PRIMARY = ('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_incremental_two|iterations=22|'
               'native_limit=16000|setup_work=794664056|setup_limit=977928192|memory_bytes=105805837|'
               'memory_limit=85591287399|final=0|certified=1\n')
    FINAL = ('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_incremental_two|iterations=8|'
             'native_limit=16000|setup_work=0|setup_limit=391938048|memory_bytes=105805837|'
             'memory_limit=85591287399|final=1|certified=1\n')

    def historical(self, text):
        return setup.resource_diagnostics(text, setup.PRE, setup.REVISIONS[setup.PRE])

    def test_captured_historical_shape_preserves_caps_and_labels_missing_hierarchy(self):
        value = self.historical(self.ROUTE + self.PRIMARY + self.FINAL)
        self.assertEqual(value['route_hierarchy'], [None])
        self.assertIn('unavailable', value['hierarchy_evidence'])
        self.assertEqual(value['resource_checks'], {'iteration_caps': 'passed', 'memory_caps': 'passed',
                                                   'setup_work_caps': 'passed'})
        self.assertEqual([(a['setup_work_used'], a['setup_work_limit']) for a in value['action_resources']],
                         [(794664056, 977928192), (0, 391938048)])
        self.assertEqual(value['action_resources'][0]['iterations_used'], 22)
        self.assertEqual(value['action_resources'][1]['memory_bytes_used'], 105805837)

    def test_current_and_ordinary_checks_still_reject_missing_hierarchy(self):
        text = self.ROUTE + self.PRIMARY
        with self.assertRaisesRegex(ValueError, 'lacks hierarchy metadata'):
            setup.resource_diagnostics(text, setup.POST, setup.REVISIONS[setup.POST])
        with self.assertRaisesRegex(ValueError, 'lacks hierarchy metadata'):
            q.science.resource_diagnostics(text, 'incremental-two', False, {},
                                            expected_preset='routing_incremental_two')
        with self.assertRaisesRegex(ValueError, 'arm/revision'):
            setup.resource_diagnostics(text, setup.PRE, setup.REVISIONS[setup.POST])

    def test_route_and_each_action_setup_cap_enforced(self):
        for text in (self.ROUTE.replace('setup_limit=977928192', 'setup_limit=1') + self.PRIMARY,
                     self.PRIMARY.replace('setup_limit=977928192', 'setup_limit=1'),
                     self.PRIMARY + self.FINAL.replace('setup_work=0', 'setup_work=391938049')):
            with self.subTest(text=text), self.assertRaisesRegex(ValueError, 'setup_work exceeded'):
                self.historical(text)
        for field in ('setup_work', 'setup_limit'):
            text = self.PRIMARY.replace(field + '=' + ('794664056' if field == 'setup_work' else '977928192') + '|', '')
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'action schema'):
                self.historical(text)

    def test_historical_schema_installation_and_counters_are_exact(self):
        invalid = [self.ROUTE.replace('installed=1', 'installed=0'),
                   self.ROUTE.replace('refusal=None', 'refusal=Some(WorkBudget)'),
                   self.ROUTE.replace('samples=1', 'samples=2'),
                   self.ROUTE.replace('iterations=16', 'iterations=15'),
                   self.ROUTE.replace('iterations=16', 'iterations=-1'),
                   self.ROUTE.replace('setup_work=794664056', 'setup_work=18446744073709551616'),
                   self.ROUTE.replace('checkpoint=16|', ''),
                   self.ROUTE.replace('checkpoint=16|', 'checkpoint=16|checkpoint=16|'),
                   self.ROUTE.replace('refusal=None', 'refusal=None|levels=1')]
        for route in invalid:
            with self.subTest(route=route), self.assertRaises(ValueError):
                self.historical(route + self.PRIMARY)
        for refusal in setup.LEGACY_REFUSALS:
            self.assertEqual(self.historical(self.ROUTE.replace('installed=1|refusal=None',
                             'installed=0|refusal=' + refusal) + self.PRIMARY)['route_hierarchy'], [None])
        with self.assertRaises(ValueError):
            self.historical(self.ROUTE.replace('installed=1|refusal=None',
                             'installed=0|refusal=Some(Unknown)') + self.PRIMARY)

    def test_legacy_iteration_memory_certificate_and_preset_checks_remain(self):
        for before, after in [('iterations=22', 'iterations=16001'),
                              ('memory_limit=85591287399', 'memory_limit=1'),
                              ('certified=1', 'certified=0'),
                              ('preset=routing_incremental_two', 'preset=routing_incremental_many')]:
            with self.subTest(field=before), self.assertRaises(ValueError):
                self.historical(self.PRIMARY.replace(before, after))
        with self.assertRaisesRegex(ValueError, 'no actual routing_action'):
            self.historical(self.ROUTE)

    def test_current_hierarchy_is_still_returned_without_historical_cap_policy(self):
        from test_routing_science import RouteHierarchyTrace
        # Current action format deliberately has no retired setup-work fields.
        action = self.PRIMARY.replace('setup_work=794664056|setup_limit=977928192|', '')
        value = setup.resource_diagnostics(RouteHierarchyTrace.INSTALLED + action,
                                           setup.POST, setup.REVISIONS[setup.POST])
        self.assertEqual(value['route_hierarchy'][0]['terminal'], 'Direct')
        self.assertNotIn('hierarchy_evidence', value)
        self.assertNotIn('setup_work_caps', value['resource_checks'])


if __name__ == '__main__':
    unittest.main()
