"""Source-transition and missing-repeat boundaries; no native execution."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import routing
import routing_blocks as blocks
import routing_run as runner
import routing_science_completion as completion
import routing_block_deployment as deployment
from test_routing import h


class Completion(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.serial = 0

    def write(self, value, name=None):
        self.serial += 1
        path = self.root / (name or f'{self.serial}.json')
        path.write_text(json.dumps(value))
        return blocks.reference(path)

    def fixture(self):
        labels = ['lsmr', 'map', 'it', 'it-coefficients']
        plugin, harness = self.write('plugin'), self.write('harness')
        manifest = {'arms': {a: {'sha256': plugin['sha256'],
            'role': 'required_reference' if a == 'baseline' else 'optional_comparator'} for a in ['baseline', *labels]},
            'candidates': ['baseline'], 'protections': ['baseline'], 'harness_sha256': harness['sha256']}
        cells = [{'id': cid, 'case_sha256': self.write(cid)['sha256'], 'comparators': labels} for cid in ['b1', 'b4', 'other']]
        context = {'cells': cells, 'candidates': ['baseline'], 'protections': ['baseline']}
        old = {'prerequisites': [], 'records': [], 'prepared_sha256': {}, 'qualified_wrappers': {}}
        execution = {'harness': harness, 'cells': {}}
        for cell in cells:
            cid = cell['id']
            if cid != 'other':
                execution['cells'][cid] = {'source': self.write(cid), 'witness': self.write({'ready': cid}), 'arms': {}}
            for arm in ['baseline', *labels]:
                prepared = self.write(cid + arm)
                row = {'cell': cid, 'arm': arm, 'prepared_sha256': prepared['sha256'], 'status': 'passed',
                       'certificate_pass': True, 'science_pass': True, 'science_sha256': h(cid+arm),
                       'witness_sha256': h('witness'), 'resource_checks': {'iteration_caps': 'passed',
                       'memory_caps': 'passed', 'setup_caps': 'not_applicable'}}
                old['prepared_sha256'][cid+'/'+arm] = prepared['sha256']
                if cid != 'other' and arm != 'baseline':
                    artifacts = {k: self.write(k+cid+arm) for k in ['result', 'columns', 'log']}
                    row.update(status='blocked', error=completion.ERROR, process_completed=True,
                        dispatched=True, call_limit_s=120, failure={'kind': 'science_or_resource_failed'},
                        failure_artifacts=artifacts)
                    repeat = {'cell': cid, 'arm': arm, 'prepared_sha256': prepared['sha256'],
                        'status': 'not_run', 'repetition': 0, 'blocked_by_sha256': routing.observation_identity(row)}
                    execution['cells'][cid]['arms'][arm] = {'plugin': plugin, 'input': prepared}
                else:
                    repeat = {**row, 'repetition': 0}
                old['prerequisites'].append(row)
                old['records'].append(repeat)
        old['qualified_wrappers']['other'] = self.write({'old': 'wrapper'})
        inputs = {'old_records': self.write(old), 'execution': execution}
        state = (context, manifest, old, blocks.checked, [])
        return inputs, state

    def declared(self, inputs, state):
        with patch.object(completion, '_state', return_value=state), \
             patch.object(completion, '_proof', return_value=({}, {}, [])), \
             patch.object(runner, 'evidence', return_value=({}, {})):
            return completion.declare(inputs)

    def test_exact_eight_plan_preserves_original_records_and_requests(self):
        inputs, state = self.fixture()
        original = copy.deepcopy(state[2])
        plan = self.declared(inputs, state)
        self.assertEqual(len(plan['requests']), 8)
        self.assertEqual(plan['native_calls'], 8)
        self.assertEqual(plan['budget_s'], 1200)
        self.assertEqual(state[2], original)
        self.assertEqual({r['cell'] for r in plan['requests']}, {'b1', 'b4'})

    def test_duplicate_missing_or_unrelated_failures_cannot_enter_completion(self):
        for mutation in ['duplicate', 'missing', 'other_error', 'already_run', 'wrong_blocker']:
            inputs, state = self.fixture()
            old = state[2]
            if mutation == 'duplicate': old['prerequisites'].append(copy.deepcopy(old['prerequisites'][0]))
            elif mutation == 'missing': old['records'].pop()
            elif mutation == 'other_error': old['prerequisites'][1]['error'] = 'numerical tolerance failure'
            elif mutation == 'already_run': old['records'][1]['status'] = 'failed'
            else: old['records'][1]['blocked_by_sha256'] = h('different observation')
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.declared(inputs, state)

    def test_changed_native_input_or_artifact_rejected_before_dispatch(self):
        for field in ['plugin', 'input']:
            inputs, state = self.fixture()
            inputs['execution']['cells']['b1']['arms']['lsmr'][field] = self.write('different')
            with self.subTest(field=field), self.assertRaises(ValueError): self.declared(inputs, state)

    def test_archive_resolver_only_substitutes_exact_named_source_and_hash(self):
        archives = {name: self.write('old'+name) for name in completion.SOURCES}
        dependencies = []
        resolve = completion._resolver(archives, dependencies)
        for name, ref in archives.items():
            logical = {'path': str(Path(completion.__file__).with_name(name).resolve()), 'sha256': ref['sha256']}
            self.assertEqual(resolve(logical), Path(ref['path']))
            with self.assertRaises(ValueError): resolve({**logical, 'sha256': h('forged')})
        with self.assertRaises((ValueError, FileNotFoundError)):
            resolve({'path': str(self.root/'data.case'), 'sha256': archives['routing_run.py']['sha256']})
        with self.assertRaises(ValueError): completion._resolver({**archives, 'native.py': self.write('native')}, [])

    def test_inherited_proof_rows_must_match_original_records_exactly(self):
        proof = {'prerequisites': [{'cell': 'b', 'arm': 'baseline'}], 'repeat_checks': [{'cell': 'b', 'arm': 'baseline', 'repetition': 0}]}
        proof_ref = self.write(routing.seal(proof))
        witness = self.write({'scoped_replay': proof_ref})
        old = {'prerequisites': copy.deepcopy(proof['prerequisites']), 'records': copy.deepcopy(proof['repeat_checks']),
               'selection_sha256': h('oldcontext'), 'tool_sha256': {'old': h('tool')}}
        with patch.object(runner, 'evidence', return_value=({'scoped_replay': proof_ref}, {})) as evidence:
            completion._proof(witness, {'id': 'b'}, {}, ['baseline'], old, blocks.checked)
            self.assertEqual(evidence.call_args.kwargs['expected_tools'], old['tool_sha256'])
            old['records'][0]['science_sha256'] = h('changed')
            with self.assertRaises(ValueError): completion._proof(witness, {'id': 'b'}, {}, ['baseline'], old, blocks.checked)

    def transition_fixture(self):
        archives = {name: self.write('before '+name) for name in completion.SOURCES}
        old_context = {'kind': blocks.KIND, 'stage': 'B', 'cells': [], 'candidates': [], 'protections': [],
                       'design': {'attempts': 12}, 'coordinator_sha256': archives['routing_blocks.py']['sha256']}
        context = {**old_context, 'coordinator_sha256': h('current coordinator')}
        old_context_ref, context_ref = self.write(routing.seal(old_context)), self.write(routing.seal(context))
        source = self.write('current source')
        sources = [source]
        old_decl = {'b_context': old_context_ref, 'sources': sources, 'design': {'attempts': 24}, 'contract': {'unchanged': True}}
        new_decl = {**old_decl, 'b_context': context_ref}
        manifest = {'harness_sha256': h('harness'), 'failure_policy': {'per_call_timeout_s': 120, 'aggregate_budget_s': 14400}}
        tools = {'routing_run.py': archives['routing_run.py']['sha256']}
        old = {'status': 'complete_with_failed_cells', 'platform': 'Darwin-test', 'purpose': 'scientific_prerequisites',
               'qualification_context': old_context_ref, 'stage': 'B', 'selection_sha256': routing.identity(old_context),
               'manifest_sha256': routing.identity(manifest), 'harness_sha256': manifest['harness_sha256'],
               'artifacts': {}, 'cases': {}, 'tool_sha256': tools, 'warmups': 0, 'repetitions': 1,
               'timeout_s': 120, 'budget_s': 14400, 'elapsed_s': 100, 'cmg_worker_checks': [], 'generic_it_worker_checks': []}
        old_ref = self.write(old)
        reviewed_raw = {'path': str(Path(completion.__file__).with_name('routing_run.py').resolve()),
                        'sha256': archives['routing_run.py']['sha256']}
        diagnostic = {'kind': 'independent_routing_scientific_checker_failure_review', 'status': 'passed',
                      'original_records': old_ref, 'original_tool_sha256': tools, 'raw_evidence': [reviewed_raw]}
        inputs = {'old_context': old_context_ref, 'old_declaration': self.write(routing.seal(old_decl)),
                  'old_records': old_ref, 'new_context': context_ref, 'new_declaration': self.write(routing.seal(new_decl)),
                  'diagnostic_review': self.write(diagnostic), 'archives': archives, 'execution': {}}
        executor = self.write('executor')
        transition = {**{k: inputs[k] for k in ['old_records', 'old_context', 'new_context', 'old_declaration', 'new_declaration', 'archives']},
                      'kind': completion.KIND+'_source_review', 'status': 'passed', 'current_sources': sources,
                      'executor': executor, 'evidence': [executor]}
        inputs['transition_review'] = self.write(transition)
        return inputs, context, manifest, new_decl, sources, executor

    def state(self, fixture):
        inputs, context, manifest, declaration, sources, _ = fixture
        with patch.object(blocks, 'load_context', return_value=(context, manifest)), \
             patch.object(deployment, 'declare', return_value=declaration), \
             patch.object(completion, 'current_sources', return_value=sources), \
             patch.object(runner, 'tool_identities', return_value={Path(runner.__file__): h('current runner')}), \
             patch.object(runner, 'cmg_worker_checks', return_value=([], set())), \
             patch.object(runner, 'generic_it_worker_checks', return_value=([], set())):
            return completion._state(inputs)

    def test_transition_retains_old_hashes_and_rechecks_executor_evidence_bytes(self):
        fixture = self.transition_fixture()
        inputs, _, _, _, _, executor = fixture
        state = self.state(fixture)
        self.assertEqual(state[2]['tool_sha256']['routing_run.py'], inputs['archives']['routing_run.py']['sha256'])
        self.assertIn(inputs['archives']['routing_run.py'], state[4])
        Path(executor['path']).write_text('changed after review')
        with self.assertRaises(ValueError): self.state(fixture)

    def test_original_cohort_accepts_actual_macOS_platform_and_rejects_other_hosts(self):
        valid = ['macOS-26.6.2-arm64-arm-64bit', 'Darwin-25.6.0-arm64-arm-64bit']
        invalid = ['Linux-6.8-x86_64', 'Windows-11', 'Darwinian-host', 'macOSish', 'Darwin', '', None, {}, [], 1, True]
        for value in [*valid, *invalid]:
            fixture = self.transition_fixture()
            inputs = fixture[0]
            records = blocks.read(inputs['old_records'])
            records['platform'] = value
            inputs['old_records'] = self.write(records)
            diagnostic = blocks.read(inputs['diagnostic_review'])
            diagnostic['original_records'] = inputs['old_records']
            inputs['diagnostic_review'] = self.write(diagnostic)
            review = blocks.read(inputs['transition_review'])
            review['old_records'] = inputs['old_records']
            inputs['transition_review'] = self.write(review)
            with self.subTest(platform=value):
                if value in valid:
                    self.state(fixture)
                else:
                    with self.assertRaisesRegex(ValueError, 'original scientific cohort'):
                        self.state(fixture)

    def test_transition_cannot_change_design_contract_or_original_record_identity(self):
        for mutation in ['Bdesign', 'Ccontract', 'oldrecords']:
            fixture = self.transition_fixture()
            inputs = fixture[0]
            if mutation == 'Bdesign':
                original = blocks.read(inputs['old_context'], sealed=True)
                original['design']['attempts'] = 13
                inputs['old_context'] = self.write(routing.seal(original))
            elif mutation == 'Ccontract':
                original = blocks.read(inputs['old_declaration'], sealed=True)
                original['contract']['unchanged'] = False
                inputs['old_declaration'] = self.write(routing.seal(original))
            else:
                diagnostic = blocks.read(inputs['diagnostic_review'])
                diagnostic['original_records'] = self.write({'other': 'cohort'})
                inputs['diagnostic_review'] = self.write(diagnostic)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): self.state(fixture)

    def test_C_source_inventory_cannot_drop_duplicate_or_replace_an_original(self):
        for mutation in ['drop', 'duplicate', 'replace']:
            fixture = self.transition_fixture()
            inputs = fixture[0]
            original = blocks.read(inputs['old_declaration'], sealed=True)
            if mutation == 'drop': original['sources'] = []
            elif mutation == 'duplicate': original['sources'] *= 2
            else: original['sources'][0] = self.write('unrelated source')
            inputs['old_declaration'] = self.write(routing.seal(original))
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, 'source inventory'):
                self.state(fixture)

    def test_hash_reuse_is_local_and_invalidated_even_when_mtime_is_restored(self):
        ref = self.write('original')
        path = Path(ref['path'])
        original = path.read_bytes()
        stamp = path.stat()
        verifier = completion.VerificationPass()
        with patch.object(completion, 'digest', wraps=completion.digest) as hashed:
            verifier.check(ref)
            verifier.check(ref)
            self.assertEqual(hashed.call_count, 1)
            path.write_bytes(b'x' * len(original))
            os.utime(path, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            with self.assertRaisesRegex(ValueError, 'within one validation'): verifier.check(ref)
            self.assertEqual(hashed.call_count, 1)
            path.write_bytes(original)
            os.utime(path, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            with self.assertRaisesRegex(ValueError, 'within one validation'): verifier.check(ref)
            self.assertEqual(hashed.call_count, 1)
            completion.VerificationPass().check(ref)
            self.assertEqual(hashed.call_count, 2)

    def test_hash_reuse_rejects_a_file_changing_during_hashing(self):
        ref = self.write('before')
        def racing_hash(path):
            path.write_text('after')
            return ref['sha256']
        verifier = completion.VerificationPass()
        with patch.object(completion, 'digest', side_effect=racing_hash), self.assertRaisesRegex(ValueError, 'while its hash'):
            verifier.check(ref)
        self.assertFalse(verifier.verified)

    def test_repeat_requires_actual_dispatch_and_same_full_science_fingerprint(self):
        request = {'cell': 'b', 'arm': 'lsmr', 'prepared': self.write('input')}
        call = {**request, 'process': {'dispatched': True, 'process_completed': True, 'call_limit_s': 120}, 'artifacts': {}}
        first = {'science_sha256': h('first'), 'status': 'passed'}
        with patch.object(completion, 'validate_first', return_value=first), \
             patch.object(completion, '_metadata', return_value=({}, {}, {}, {})), \
             patch.object(runner, 'reader_arm', return_value='lsmr'), \
             patch.object(completion, '_read_call', return_value=({}, {'science_sha256': h('first'), 'solver_rows': []})) as read:
            self.assertEqual(completion.validate_repeat({}, request, call)['repetition'], 0)
            call['process']['dispatched'] = False
            with self.assertRaises(ValueError): completion.validate_repeat({}, request, call)
            call['process']['dispatched'] = True
            read.return_value = {}, {'science_sha256': h('different columns'), 'solver_rows': []}
            with self.assertRaisesRegex(ValueError, 'exact science'): completion.validate_repeat({}, request, call)

    def report_fixture(self):
        old_dir = self.root/'old'/'science'
        old_dir.mkdir(parents=True)
        old_path = old_dir/'records.json'
        old_path.write_text('{}')
        requests = [{'cell': f'b{i//4}', 'arm': f'arm{i%4}', 'prepared': self.write(str(i))} for i in range(8)]
        executor = self.write('reviewed executor')
        review = self.write({'executor': executor, 'evidence': [executor]})
        plan = {'inputs': {'old_records': blocks.reference(old_path), 'transition_review': review}, 'requests': requests, 'budget_s': 1200}
        plan_ref = self.write(routing.seal(plan))
        claim = {'kind': completion.KIND+'_native_claim', 'plan': plan_ref, 'native_calls': 8, 'requests': requests, 'executor': executor}
        claim_path = old_dir.parent/'science-completion-native-claim.json'
        claim_path.write_text(json.dumps(claim))
        report = {'kind': completion.KIND+'_native_calls', 'status': 'complete', 'plan': plan_ref,
                  'elapsed_s': 20., 'calls': requests, 'claim': blocks.reference(claim_path)}
        return plan, plan_ref, claim_path, report

    def test_plain_report_and_canonical_once_claim_required(self):
        plan, bound, claim_path, report = self.report_fixture()
        with patch.object(completion, 'load_plan', return_value=plan), \
             patch.object(completion, 'validate_first', side_effect=lambda p, r: r), \
             patch.object(completion, 'validate_repeat', side_effect=lambda p, r, c: c):
            result = completion.finalize(bound, self.write(report))
            self.assertEqual(result['original_outcome'], 'complete_with_failed_cells; preserved unchanged')
            report['claim'] = self.write(json.loads(claim_path.read_text()))
            with self.assertRaisesRegex(ValueError, 'unique'): completion.finalize(bound, self.write(report))

    def test_partial_failed_overbudget_or_wrong_plan_reports_rejected(self):
        for mutation in ['partial', 'failed', 'overbudget', 'nan', 'wrongplan']:
            plan, bound, _, report = self.report_fixture()
            if mutation == 'partial': report['calls'].pop()
            elif mutation == 'failed': report['status'] = 'failed'
            elif mutation == 'overbudget': report['elapsed_s'] = 1201.
            elif mutation == 'nan': report['elapsed_s'] = float('nan')
            else: report['plan'] = self.write({'different': 'plan'})
            with self.subTest(mutation=mutation), patch.object(completion, 'load_plan', return_value=plan), self.assertRaises(ValueError):
                completion.finalize(bound, self.write(report))
            # Each subtest owns a distinct cohort directory.
            import shutil
            shutil.rmtree(self.root/'old')

    def test_missing_unreviewed_or_changed_executor_rejected(self):
        for mutation in ['missing', 'unreviewed', 'changed']:
            plan, bound, claim_path, report = self.report_fixture()
            claim = json.loads(claim_path.read_text())
            if mutation == 'missing': claim.pop('executor')
            elif mutation == 'unreviewed': claim['executor'] = self.write('unreviewed executor')
            else: Path(claim['executor']['path']).write_text('changed source')
            claim_path.write_text(json.dumps(claim))
            report['claim'] = blocks.reference(claim_path)
            with self.subTest(mutation=mutation), patch.object(completion, 'load_plan', return_value=plan), self.assertRaises(ValueError):
                completion.finalize(bound, self.write(report))
            import shutil
            shutil.rmtree(self.root/'old')


if __name__ == '__main__':
    unittest.main()
