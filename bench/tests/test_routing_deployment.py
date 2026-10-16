"""Synthetic-only public-default deployment contract and tamper checks."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import routing
import routing_deployment as deployment
from test_routing import h, manifest, records, portfolio_manifest


class DeploymentGate(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.index = 0
        self.spec = manifest()
        hidden = self.write('hidden plugin')['sha256']
        for arm in self.spec['arms'].values():
            if arm['sha256'] == h('candidate'):
                arm['sha256'] = hidden
        self.old_tokens = {'crates/fereg-plugin/src/estimate.rs': ['old'], 'other.rs': ['fixed']}
        self.new_tokens = {**self.old_tokens, 'crates/fereg-plugin/src/estimate.rs': ['new']}
        self.spec['candidate_source_identity'] = {'plugin_sha256': hidden,
            'production_tokens_sha256': routing.identity(self.old_tokens), 'artifact_inputs_sha256': h('inputs')}
        self.spec['protections'].append('incumbent')
        self.spec['arms']['incumbent'] = copy.deepcopy(self.spec['arms']['baseline'])
        self.original = routing.seal(self.spec)
        self.declaration = routing.seal(deployment.declare(self.original))
        times = {'h8-k0': .8}
        self.ar = records(self.spec, 'A', times=times)
        self.a = routing.seal(routing.evaluate(self.spec, 'A', self.ar))
        self.br = records(self.spec, 'B', self.a['value'], times)
        self.b = routing.seal(routing.evaluate(self.spec, 'B', self.br, self.a['value']))
        self.assertEqual(self.b['value']['selected'], 'h8-k0')
        plugin = self.write('new public plugin')
        full_suite, build = self.write('full suite passes'), self.write('build and ABI pass')
        source = self.write({'plugin_sha256': plugin['sha256'],
            'production_tokens_sha256': routing.identity(self.new_tokens), 'artifact_inputs_sha256': h('inputs'),
            'full_tests_log_sha256': full_suite['sha256'], 'native_build_log_sha256': build['sha256']})
        self.packet = {'schema': 1, 'kind': deployment.KIND, 'selected': 'h8-k0',
            'hidden_sha256': hidden, 'public_plugin': plugin, 'public_source': source, 'reports': {}}
        common = {'status': 'passed', 'selected': 'h8-k0', 'hidden_sha256': hidden,
            'public_sha256': plugin['sha256'], 'source_sha256': source['sha256'],
            'declaration_sha256': self.declaration['sha256']}
        self.reports = {name: {**common, 'kind': deployment.KIND + ':' + name}
                       for name in ['source_review', 'correctness', 'equivalence']}
        self.reports['source_review'].update(activation_only=True,
            public_invocation=copy.deepcopy(deployment.CONTRACT['public_invocation']),
            effective_policy=routing.policy('h8-k0'), changed_production_paths=['crates/fereg-plugin/src/estimate.rs'],
            independent_review=self.write('Independent actual-diff review passes'),
            production_inventory={'hidden': self.write(self.old_tokens), 'public': self.write(self.new_tokens)})
        self.reports['correctness']['checks'] = {name: {'status': 'passed', 'evidence': self.write(name)}
                                               for name in deployment.CHECKS}
        self.reports['correctness']['checks']['full_suite']['evidence'] = full_suite
        self.reports['correctness']['checks']['build_abi'] = {
            'status': 'passed', 'evidence': build, 'build_log_sha256': build['sha256']}
        self.reports['equivalence'].update(invocations={
            'hidden': {'solver': 'auto', 'injection': 'routing_h8_k0'},
            'public': copy.deepcopy(deployment.CONTRACT['public_invocation'])}, cells=[])
        for cell in self.declaration['value']['non_c_cells']:
            calls = []
            for arm in ['hidden', 'public']:
                for repeat in [0, 1]:
                    calls.append({'arm': arm, 'repeat': repeat, 'status': 'passed', 'certificate_pass': True,
                        'science_pass': True, 'science_sha256': h(cell['id']), 'solver_signature_sha256': h('signature'),
                        'witness_sha256': h('witness'), 'plugin_sha256': hidden if arm == 'hidden' else plugin['sha256'],
                        'resource_checks': {key: 'passed' for key in ['iteration_caps', 'memory_caps', 'setup_caps']},
                        'evidence': self.write(f"native output {cell['id']} {arm} {repeat}")})
            self.reports['equivalence']['cells'].append({**cell, 'calls': calls})
        self.bind_reports()

    def write(self, value):
        self.index += 1
        path = self.root / str(self.index)
        path.write_text(value if isinstance(value, str) else json.dumps(value))
        return deployment.reference(path)

    def bind_reports(self):
        self.packet['reports'] = {name: self.write(report) for name, report in self.reports.items()}

    def derive(self):
        return deployment.derive(self.declaration, self.original, self.a, self.ar, self.b, self.br, self.packet)

    def test_exact_portfolio_origin_preserves_activation_and_untouched_c_contract(self):
        spec = portfolio_manifest()
        spec['candidate_source_identity'] = {'plugin_sha256': h('candidate'),
            'production_tokens_sha256': h('production'), 'artifact_inputs_sha256': h('inputs')}
        original = copy.deepcopy(spec)
        self.assertEqual(deployment.origin(routing.seal(spec)), original)
        declared = deployment.declare(routing.seal(spec))
        self.assertEqual(declared['contract'], deployment.CONTRACT)
        self.assertEqual(spec['panels']['C'], original['panels']['C'])
        self.assertEqual({c['stage'] for c in declared['non_c_cells']}, {'A', 'B'})
        for kind in ['prefix', 'overlap', 'composed', 'routing']:
            bad = copy.deepcopy(spec)
            bad['experiment_kind'] = kind
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                deployment.origin(routing.seal(bad))
        for change in ['order', 'policy', 'sampling', 'source', 'incumbent']:
            bad = copy.deepcopy(spec)
            if change == 'order': bad['candidates'].reverse()
            elif change == 'policy': bad['arms'][routing.PORTFOLIO_V1]['policy']['two_fe']['trees'] = 2
            elif change == 'sampling': bad['sampled_qualification'] = {}
            elif change == 'source': bad['candidate_source_identity']['plugin_sha256'] = h('different')
            else:
                bad['protections'].append('incumbent')
                bad['arms']['incumbent'] = {**bad['arms']['baseline'], 'sha256': h('different')}
            with self.subTest(change=change), self.assertRaises(ValueError):
                deployment.origin(routing.seal(bad))

    def test_exact_public_holdout_preserves_original_contract_and_hidden_protection(self):
        value = self.derive()
        self.assertEqual(value['panels'], {'C': self.spec['panels']['C']})
        for name in ['baseline', 'lsmr']:
            self.assertEqual(value['arms'][name], self.spec['arms'][name])
        self.assertEqual(value['arms']['incumbent'], {**self.spec['arms']['h8-k0'], 'role': 'required_reference'})
        self.assertEqual(value['arms']['public-auto']['policy'], None)
        self.assertEqual(value['public_invocation'], {'solver': 'auto', 'injection': None})
        for key in ['warmups', 'repetitions', 'failure_policy', 'harness_sha256']:
            self.assertEqual(value[key], self.spec[key])
        inputs = (self.declaration, self.original, self.a, self.ar, self.b, self.br, self.packet)
        deployment.validate_derived(routing.seal(value), *inputs)
        for key, changed in [('warmups', 2), ('protections', ['baseline']), ('selected', 'h16-k0')]:
            bad = copy.deepcopy(value)
            bad[key] = changed
            with self.subTest(key=key), self.assertRaises(ValueError):
                deployment.validate_derived(routing.seal(bad), *inputs)
        with self.assertRaises(ValueError):
            routing.validate_manifest(value)  # Execution hooks intentionally deferred.

    def test_declaration_source_statistics_and_seal_tampering_fails(self):
        for key in ['selector_source_sha256', 'statistic_source_sha256', 'deployment_validator_sha256']:
            bad = copy.deepcopy(self.declaration['value'])
            bad[key] = h('changed')
            with self.subTest(key=key), self.assertRaises(ValueError):
                deployment.validate_declaration(routing.seal(bad), self.original)
        bad = copy.deepcopy(self.declaration)
        bad['value']['contract']['bootstrap_draws'] = 100
        with self.assertRaises(ValueError):
            deployment.validate_declaration(bad, self.original)

    def test_decisions_are_recomputed_and_baseline_is_not_a_deployment(self):
        bad = copy.deepcopy(self.b['value'])
        bad['selected'] = 'h16-k0'
        with self.assertRaises(ValueError):
            deployment.selected_b(self.spec, self.a, self.ar, routing.seal(bad), self.br)
        altered = copy.deepcopy(self.br)
        altered['records'].pop()
        with self.assertRaises(ValueError):
            deployment.selected_b(self.spec, self.a, self.ar, self.b, altered)
        ar = records(self.spec, 'A')
        a = routing.evaluate(self.spec, 'A', ar)
        br = records(self.spec, 'B', a)
        b = routing.evaluate(self.spec, 'B', br, a)
        with self.assertRaisesRegex(ValueError, 'nonbaseline'):
            deployment.selected_b(self.spec, routing.seal(a), ar, routing.seal(b), br)

    def test_distinct_incumbent_and_comparator_collisions_rejected(self):
        for kind in ['distinct', 'incumbent-comparator', 'public-comparator']:
            spec = copy.deepcopy(self.spec)
            if kind == 'distinct':
                spec['arms']['incumbent']['sha256'] = h('different protection')
            else:
                arm = 'incumbent' if kind.startswith('incumbent') else 'public-auto'
                spec['panels']['C'][0]['comparators'].append(arm)
                if arm == 'public-auto':
                    spec['arms'][arm] = {'sha256': h('other'), 'role': 'optional_comparator', 'solver': 'lsmr'}
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                deployment.declare(routing.seal(spec))

    def test_declaration_requires_candidate_source_binding_before_selection(self):
        for key in ['plugin_sha256', 'production_tokens_sha256', 'artifact_inputs_sha256']:
            spec = copy.deepcopy(self.spec)
            del spec['candidate_source_identity'][key]
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'source/artifact'):
                deployment.declare(routing.seal(spec))
        spec = copy.deepcopy(self.spec)
        spec['candidate_source_identity']['plugin_sha256'] = h('different plugin')
        with self.assertRaisesRegex(ValueError, 'source/artifact'):
            deployment.declare(routing.seal(spec))

    def test_non_c_complete_identity_and_exact_repeats_are_mandatory(self):
        original = copy.deepcopy(self.reports['equivalence'])
        for kind in ['missing-cell', 'c-body', 'missing-repeat', 'changed-science', 'changed-signature',
                     'changed-witness', 'wrong-plugin', 'failed-science', 'resource-failure', 'injected-public']:
            self.reports['equivalence'] = copy.deepcopy(original)
            report = self.reports['equivalence']
            row = report['cells'][0]
            call = row['calls'][0]
            if kind == 'missing-cell': report['cells'].pop()
            elif kind == 'c-body': row['body_sha256'] = self.spec['panels']['C'][0]['body_sha256']
            elif kind == 'missing-repeat': row['calls'].pop()
            elif kind.startswith('changed-'):
                call[{'changed-science': 'science_sha256', 'changed-signature': 'solver_signature_sha256',
                      'changed-witness': 'witness_sha256'}[kind]] = h(kind)
            elif kind == 'wrong-plugin': call['plugin_sha256'] = h('wrong')
            elif kind == 'failed-science': call['science_pass'] = False
            elif kind == 'resource-failure': call['resource_checks']['memory_caps'] = 'failed'
            else: report['invocations']['public']['injection'] = 'routing_h8_k0'
            self.bind_reports()
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                self.derive()

    def test_actual_source_diff_and_fresh_correctness_are_required(self):
        original = copy.deepcopy(self.reports)
        for kind in ['unreviewed', 'extra-production', 'wrong-source', 'failed-check', 'missing-check', 'stale-log']:
            self.reports = copy.deepcopy(original)
            if kind == 'unreviewed': self.reports['source_review']['activation_only'] = False
            elif kind == 'extra-production':
                self.reports['source_review']['production_inventory']['public'] = self.write({**self.new_tokens, 'other.rs': ['changed']})
            elif kind == 'wrong-source': self.reports['source_review']['source_sha256'] = h('old')
            elif kind == 'failed-check': self.reports['correctness']['checks']['transactions_break']['status'] = 'failed'
            elif kind == 'missing-check': del self.reports['correctness']['checks']['independent_science']
            else: self.reports['correctness']['checks']['full_suite']['evidence'] = self.write('old log')
            self.bind_reports()
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                self.derive()

    def test_bound_output_tampering_is_rejected(self):
        Path(self.packet['public_plugin']['path']).write_text('changed artifact')
        with self.assertRaisesRegex(ValueError, 'identity changed'):
            self.derive()


if __name__ == '__main__':
    unittest.main()
