"""Fail-closed handoff of a real bounded failure, without a timing decision."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import routing
import routing_blocks as blocks
import routing_block_deployment as deployment
import routing_failure as failure
from test_routing import h
from test_routing_incremental_mac import IncrementalMac


class FailureHandoff(unittest.TestCase):
    def setUp(self):
        f = IncrementalMac()
        f.setUp()
        self.addCleanup(f.doCleanups)
        self.write, self.original = f.write, f.spec
        source = self.write({'source': 'frozen fixture'})
        self.area = Path(source['path']).parent
        self.public = {'sha256': h('public'), 'policy': None, 'role': 'candidate'}
        declaration = self.write(routing.seal({'b_context': f.bound, 'design': f.design}))
        activation = self.write({'complete': True})
        manifest = deployment.holdout_manifest(f.spec, 'incremental-many', self.public)
        context = {'kind': deployment.KIND, 'stage': 'C', 'experiment_kind': 'incremental',
            'inputs': {'declaration': declaration, 'activation': activation},
            'selected': 'incremental-many', 'manifest': manifest,
            'cells': f.spec['panels']['C'], 'design': f.design}
        context_ref = self.write(routing.seal(context))
        commands = [{'name': 'witnesses', 'outer_seconds': 2100},
                    {'name': 'science', 'outer_seconds': 15000}]
        start = {'status': 'started', 'results': [], 'phase': 'C', 'pid': 12, 'started_unix': 1000.,
            'context': context_ref, 'commands': commands, 'native_timeout_s': 120, 'native_budget_s': 14400,
            **{k: source for k in ('supervisor', 'owned_runner', 'witness_adapter', 'source_review')}}
        outcome = {**start, 'status': 'terminal', 'exit_code': 1, 'elapsed_seconds': 2100.5,
            'completed_unix': 3100.5, 'error': "TimeoutError('runner exceeded 2100s; see witness log')",
            'results': [{'name': 'witnesses', 'ownership': {'identities': {'13': 'birth'},
                'cleanup': {'complete': True, 'survivors': []}}}]}
        outcome_ref = self.write(outcome)
        external = self.write({'external': 'admission-only runtime'})
        self.external = external
        st = Path(external['path']).stat()
        metadata = {'status': 'passed', 'action': 'derive-c', 'outputs': {'context': context_ref},
            'completed_unix': 900., 'sources': {'driver': source},
            'metadata': {'poisoned': None, 'full_byte_checks': 2,
                'closure': [{**external, 'stamp': [st.st_dev, st.st_ino, st.st_mode,
                              st.st_size, st.st_mtime_ns, st.st_ctime_ns]}],
                'path_aliases': {external['path']: external['path']}}}
        metadata_ref = self.write(metadata)
        admitted = self.write({'status': 'passed', 'action': 'derive-c', 'outputs': metadata['outputs'],
                               'metadata_outcome': metadata_ref})
        admission = self.write({'kind': 'independent_C_admission_review', 'status': 'passed',
            'context': context_ref, 'complete_activation': activation, 'declaration': declaration,
            'metadata_outcome': metadata_ref, 'supervisor_outcome': admitted})
        handoff = self.write(routing.seal({'phase': 'C', 'context': context_ref,
            'context_identity': routing.identity(context), 'manifest_identity': routing.identity(manifest),
            'adapter': source, 'closure': [source],
            'config_snapshot': self.write({'activation_complete': activation})}))
        ready = self.area / 'C/ready'
        ready.mkdir(parents=True)
        (ready / 'partial.json').write_text('{}')
        cleanup = self.write({'status': 'passed_failure_validation', 'outcome': outcome_ref,
            'owned_cleanup_complete': True, 'current_owned_census': [
                {'pid': 12, 'birth': 'supervisor birth', 'ps_exit': 1, 'ps': ''},
                {'pid': 13, 'birth': 'birth', 'ps_exit': 1, 'ps': ''}]})
        packet = {'schema': 1, 'kind': failure.KIND, 'context': context_ref,
            'start': self.write(start), 'outcome': outcome_ref, 'handoff': handoff,
            'admission_outcome': metadata_ref, 'admission_review': admission,
            'partial_outputs': [blocks.reference(ready / 'partial.json')]}
        raw = {k: packet[k] for k in (*failure.REFERENCES[:-1], 'partial_outputs')}
        raw.update(kind=failure.KIND + '_review', status='passed_failure_review',
            no_native_science=True, no_timing=True, cleanup_complete=True,
            lead_failure_validation=cleanup,
            **{k: source for k in ('audit_source', 'closure_capture', 'supervisor_log', 'witness_log')})
        packet['raw_review'] = self.write(raw)
        self.packet, self.source_ref = packet, source

    def basis(self, packet=None):
        return failure.failure_basis(self.write(packet or self.packet))

    def test_actual_failure_has_no_performance_or_science_pass(self):
        original, basis = self.basis()
        self.assertEqual(original, self.original)
        self.assertFalse(basis['promotion'])
        self.assertFalse(basis['qualification_pass'])
        self.assertEqual(basis['native_calls'], 0)
        self.assertNotIn('timing_manifest', basis)

    def test_changed_bound_source_rejected(self):
        Path(self.source_ref['path']).write_text('{}')
        with self.assertRaises(ValueError): self.basis()

    def test_external_metadata_only_dependency_rejected(self):
        Path(self.external['path']).write_text('{}')
        with self.assertRaises(ValueError): self.basis()

    def test_unrelated_process_census_rejected(self):
        raw = blocks.read(self.packet['raw_review'])
        cleanup = blocks.read(raw['lead_failure_validation'])
        cleanup['current_owned_census'][1]['pid'] = 99
        raw['lead_failure_validation'] = self.write(cleanup)
        with self.assertRaises(ValueError):
            self.basis({**self.packet, 'raw_review': self.write(raw)})

    def test_incomplete_cleanup_wrong_failure_or_changed_start_rejected(self):
        outcome = blocks.read(self.packet['outcome'])
        variants = [dict(outcome, exit_code=0), dict(outcome, started_unix=1001.),
                    dict(outcome, error='ValueError'), dict(outcome, elapsed_seconds=2000.)]
        missing_cleanup = copy.deepcopy(outcome)
        missing_cleanup['results'][0]['ownership']['cleanup']['complete'] = False
        variants.append(missing_cleanup)
        for bad in variants:
            with self.subTest(bad=bad):
                with self.assertRaises(ValueError):
                    self.basis({**self.packet, 'outcome': self.write(bad)})

    def test_native_or_timing_presence_blocks_pre_science_claim(self):
        for name in ('C-science.log', 'C/science', 'C/timing-manifest.json', 'C/timing',
                     'C/decision.json', 'C-freeze-timing-supervisor-start.json'):
            path = self.area / name
            path.write_text('{}')
            with self.assertRaises(ValueError): self.basis()
            path.unlink()

    def test_incomplete_partial_inventory_or_missing_review_rejected(self):
        (self.area / 'C/ready/unlisted.json').write_text('{}')
        with self.assertRaises(ValueError): self.basis()
        (self.area / 'C/ready/unlisted.json').unlink()
        raw = blocks.read(self.packet['raw_review'])
        with self.assertRaises(ValueError):
            self.basis({**self.packet, 'raw_review': self.write({**raw, 'no_native_science': False})})

    def terminal_inputs(self):
        bound = self.write(self.packet)
        _, basis = failure.failure_basis(bound)
        source = {'revision': self.original['incremental']['incumbent_revision'],
            'artifacts': {'Darwin': self.original['arms']['incumbent']['sha256'], 'Linux': h('linux')},
            'production_tokens_sha256': h('production'), 'artifact_inputs_sha256': h('inputs')}
        source_ref = self.write(source)
        evidence = [self.source_ref]
        review = {'kind': deployment.TERMINAL + '_operational_review', 'status': 'passed',
            'outcome': 'retained', 'source_decision_sha256': routing.identity(basis),
            'evidence_kind': basis['evidence_kind'], 'failure_validator': blocks.reference(failure.__file__),
            'incumbent_source': source_ref, 'evidence': evidence,
            'platform_builds': {host: self.write({**source, 'status': 'passed', 'platform': host,
                'plugin_sha256': sha, 'evidence': self.source_ref}) for host, sha in source['artifacts'].items()}}
        exported = {k: review[k] for k in ('status', 'outcome', 'source_decision_sha256',
                                         'evidence_kind', 'failure_validator', 'evidence')}
        exported.update(kind=deployment.TERMINAL + '_export_review', stage_family='many',
            incumbent_source_sha256=source_ref['sha256'], prior_manifest_sha256=routing.identity(self.original),
            incumbent_revision=source['revision'], incumbent_artifacts=source['artifacts'],
            incumbent_source_identity={k: source[k] for k in ('production_tokens_sha256', 'artifact_inputs_sha256')})
        review['independent_export_review'] = self.write(exported)
        return {'confirmation_failure': bound, 'operational_review': self.write(review)}, review

    def test_retention_portable_and_local_replay(self):
        inputs, _ = self.terminal_inputs()
        value = failure.terminal_stage(inputs)
        self.assertEqual(value['status'], 'retained')
        with failure.install():
            self.assertEqual(deployment.validate_terminal(value), value)
        self.assertEqual(deployment.validate_terminal(value, portable=True), value)

    def test_acceptance_and_wrong_incumbent_rejected(self):
        inputs, review = self.terminal_inputs()
        with self.assertRaises(ValueError):
            failure.terminal_stage({**inputs, 'operational_review': self.write({**review, 'outcome': 'accepted'})})
        source = blocks.read(review['incumbent_source'])
        review['incumbent_source'] = self.write({**source, 'revision': 'b' * 40})
        with self.assertRaises(ValueError):
            failure.terminal_stage({**inputs, 'operational_review': self.write(review)})

    def test_scope_delegates_ordinary_inputs_and_restores_on_failure(self):
        with patch.object(deployment, 'terminal_stage', return_value='ordinary') as old:
            with self.assertRaises(RuntimeError):
                with failure.install():
                    self.assertEqual(deployment.terminal_stage({'config': {}}), 'ordinary')
                    old.assert_called_once_with({'config': {}})
                    raise RuntimeError('caller failed')
            self.assertIs(deployment.terminal_stage, old)


if __name__ == '__main__':
    unittest.main()
