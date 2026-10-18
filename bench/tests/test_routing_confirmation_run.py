"""Synthetic driver boundaries; optional real owned-process tests run no estimator."""
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import routing
import routing_confirmation_run as driver
from routing_validation import ByteVerificationPass


class Driver(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.serial = 0

    def file(self, value):
        self.serial += 1
        path = self.root / f'{self.serial}.json'
        driver.save(path, value)
        return driver.ref(path)

    def test_exclusive_save_preserves_original(self):
        bound = self.file({'first': True})
        with self.assertRaises(FileExistsError):
            driver.save(bound['path'], {'second': True})
        self.assertEqual(driver.read(bound), {'first': True})

    def test_read_rejects_same_size_mutation(self):
        bound = self.file({'value': 1})
        with self.assertRaises(ValueError), ByteVerificationPass() as verify:
            self.assertEqual(driver.read(bound, verify), {'value': 1})
            path = Path(bound['path'])
            path.write_text(path.read_text().replace('1', '2'))
            with self.assertRaises(ValueError):
                driver.read(bound, verify)
            # A poisoned pass cannot claim a successful finish.
            with self.assertRaises(ValueError):
                verify.finish()

    def test_dependency_callback_scoped_and_never_result_cached(self):
        bound = self.file({'proof': True})
        calls = []
        def evidence(*args, **kwargs):
            calls.append(kwargs)
            return {'ordinal': len(calls)}, {Path(bound['path']): bound['sha256']}
        with ByteVerificationPass() as verify, patch.object(driver.runner, 'evidence', evidence):
            with driver.immutable_dependencies(verify):
                a = driver.runner.evidence('one')
                b = driver.runner.evidence('one')
                self.assertNotEqual(a, b)
                self.assertEqual(calls[0]['dependency_verifier'], verify.digest)
                self.assertIn(bound['path'], verify.files)
            self.assertIs(driver.runner.evidence, evidence)
            driver.runner.evidence('outside')
            self.assertNotIn('dependency_verifier', calls[-1])

    def test_dependency_hook_restores_on_failure(self):
        original = driver.runner.evidence
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(RuntimeError, 'test'):
                with driver.immutable_dependencies(verify):
                    raise RuntimeError('test')
            self.assertIs(driver.runner.evidence, original)

    def test_explicit_existing_callback_is_not_overridden(self):
        seen = []
        callback = lambda path: 'hash'
        def evidence(*args, **kwargs):
            seen.append(kwargs['dependency_verifier'])
            return {}, {}
        with ByteVerificationPass() as verify, patch.object(driver.runner, 'evidence', evidence):
            with driver.immutable_dependencies(verify):
                driver.runner.evidence(dependency_verifier=callback)
        self.assertEqual(seen, [callback])

    def test_case_escape_rejected_before_read(self):
        outside = self.root / 'outside.case'
        outside.write_text('case')
        inside = self.root / 'inside'
        inside.mkdir()
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(ValueError, 'escapes'):
                driver.source_case(inside, {'case_file': '../outside.case'}, verify)

    def test_timing_load_delegates_fresh_and_brackets_before_native(self):
        freeze = self.root / 'freeze-timing'
        freeze.mkdir()
        driver.save(freeze / 'manifest.json', {'manifest': True})
        immutable = self.file({'immutable': True})
        timeline = []
        adapter = object()
        def load(path):
            timeline.append('fresh-loader')
            return {'bindings': [immutable]}, adapter
        def collect(path, output):
            value, actual = driver.time_blocks.load_manifest(path)
            self.assertIs(actual, adapter)
            timeline.append('native')
            return {'value': value}
        with ByteVerificationPass() as verify, patch.object(driver.time_blocks, 'load_manifest', load), \
                patch.object(driver.time_blocks, 'collect', collect), \
                patch.object(verify, 'before_producer', side_effect=lambda: timeline.append('full-bytes')):
            driver.collect({'output_root': str(self.root)}, verify, self.root / 'collect')
            driver.collect({'output_root': str(self.root)}, verify, self.root / 'collect')
            self.assertIs(driver.time_blocks.load_manifest, load)
        self.assertEqual(timeline, ['fresh-loader', 'full-bytes', 'native'] * 2)

    def test_timing_hook_restores_after_error(self):
        freeze = self.root / 'freeze-timing'
        freeze.mkdir()
        driver.save(freeze / 'manifest.json', {})
        original = driver.time_blocks.load_manifest
        with ByteVerificationPass() as verify, patch.object(driver.time_blocks, 'collect', side_effect=RuntimeError('native')):
            with self.assertRaises(RuntimeError):
                driver.collect({'output_root': str(self.root)}, verify, self.root)
            self.assertIs(driver.time_blocks.load_manifest, original)

    def test_recursive_old_manifest_does_not_take_full_boundary(self):
        freeze = self.root / 'freeze-timing'
        freeze.mkdir()
        driver.save(freeze / 'manifest.json', {})
        old = self.file({'old': True})
        calls = []
        def load(path):
            calls.append(Path(path))
            if Path(path) == freeze / 'manifest.json':
                driver.time_blocks.load_manifest(Path(old['path']))
            return {'bindings': []}, object()
        def collect(path, output):
            return driver.time_blocks.load_manifest(path)[0]
        with ByteVerificationPass() as verify, patch.object(driver.time_blocks, 'load_manifest', load), \
                patch.object(driver.time_blocks, 'collect', collect), patch.object(verify, 'before_producer') as boundary:
            driver.collect({'output_root': str(self.root)}, verify, self.root / 'collect')
            self.assertEqual(boundary.call_count, 1)
        self.assertEqual(calls, [freeze / 'manifest.json', Path(old['path'])])

    def test_supervisor_finish_failure_is_persisted_failed(self):
        common = self.file({'source': True})
        config = {'output_root': str(self.root), 'sources': {'source': common}}
        def run(command, log, timeout, entry):
            driver.save(self.root / 'exposed/outcome.json', {'status': 'passed', 'action': 'exposed',
                        'config': common, 'review': common, 'sources': config['sources'], 'outputs': {}})
            return {'exit_code': 0}
        helper = SimpleNamespace(run_owned=run, interrupted=lambda *_: None,
                                 processes=lambda: {os.getpid(): {'birth': 'test-birth'}})
        with patch.object(driver, 'configuration', return_value=config), patch.object(driver, 'owned', return_value=helper), \
                patch.object(ByteVerificationPass, 'finish', side_effect=ValueError('final drift')):
            with self.assertRaisesRegex(ValueError, 'final drift'):
                driver.supervise('exposed', common, common)
        outcome = routing.read_json(self.root / 'exposed/supervisor-outcome.json')
        self.assertEqual(outcome['status'], 'failed')
        self.assertIn('final drift', outcome['error'])
        start = routing.read_json(self.root / 'exposed/supervisor-start.json')
        self.assertEqual(start['pid'], os.getpid())
        self.assertEqual(start['birth'], 'test-birth')
        self.assertEqual(start['argv'][0], sys.executable)

    def test_postflight_validates_every_checked_wrapper(self):
        common = self.file({'proof': True})
        cells = [{'id': str(i)} for i in range(42)]
        context = {'cells': cells}
        area = self.root / 'science/run'
        records = {'qualified_wrappers': {c['id']: common for c in cells}}
        driver.save(area / 'records.json', records)
        proof = {key: common for key in ('independent_witness', 'scoped_replay', 'resource_contract')}
        with ByteVerificationPass() as verify, patch.object(driver, 'cohort', return_value=(common, context, {})), \
                patch.object(driver.blocks, 'qualification', return_value=(context, {}, records)), \
                patch.object(driver.blocks, 'comparison_coverage', return_value={'comparisons': []}), \
                patch.object(driver.blocks, 'required_arms', return_value=['baseline']), \
                patch.object(driver, 'tracked_evidence', return_value=proof) as evidence:
            report = driver.postflight({'output_root': str(self.root)}, verify)
            self.assertEqual(evidence.call_count, 42)
            self.assertEqual(report['checked_wrappers'], 42)
            self.assertEqual(set(report['proofs']), {c['id'] for c in cells})
            self.assertEqual(evidence.call_args.kwargs['stage'], 'C')
            self.assertEqual(evidence.call_args.kwargs['selection_sha256'], routing.identity(context))

    def test_exposed_complete_42_wrappers_and_exact_partial(self):
        original = {'admission': 'unchanged'}
        common = self.file({'source': True})
        inputs = {'declaration': self.file(routing.seal({'b_context': common})),
                  'activation': self.file({'reports': {'equivalence': self.file({'collections': [
                      {'context': self.file(routing.seal({'inputs': {'partial': common}}))}]})}})}
        config = {'original_inputs': inputs, 'original_context': self.file(routing.seal(original)),
                  'baseline_profile': self.file({'status': 'passed', 'return_sha256': 'canonical', 'inputs': {'partial': common}}),
                  'old_b_context': common, 'old_b_sources': str(self.root), 'old_b_wrappers': str(self.root),
                  'sources': {'test': common}}
        config['synthetic_checks'] = self.file({'status': 'passed', 'sources': config['sources'],
            'checks': {'synthetic_drift': True, 'synthetic_cleanup': True}, 'evidence': [common]})
        context = {'stage': 'B', 'cells': [{'id': str(i)} for i in range(42)]}
        with ByteVerificationPass() as verify, \
                patch.object(driver.deployment, 'derive', return_value=original) as derive, \
                patch.object(driver.blocks, 'context_bindings', return_value=[common]) as bindings, \
                patch.object(driver.confirmation, 'partial_tuple_identity', return_value='canonical'), \
                patch.object(driver.blocks, 'load_context', return_value=(context, {})), \
                patch.object(driver.blocks, 'required_arms', return_value=['baseline']), \
                patch.object(driver, 'source_case', return_value=(self.root / 'case', True)), \
                patch.object(driver, 'tracked_evidence', return_value={}) as evidence:
            report = driver.exposed(config, verify, common)
            self.assertEqual(derive.call_count, 1)
            self.assertEqual(bindings.call_count, 2)
            self.assertEqual(evidence.call_count, 42)
            self.assertEqual(report['canonical_partial_tuple_sha256'], report['repaired_partial_tuple_sha256'])
            self.assertTrue(all(report['checks'].values()))
        self.assertEqual(verify.full_checks, 2)

    def prior_fixture(self, status='passed'):
        common = self.file({'immutable': True})
        config = {'output_root': str(self.root), 'sources': {'source': common}}
        area = self.root / 'derive'
        child = {'status': status, 'action': 'derive', 'config': common, 'review': common,
                 'sources': config['sources'], 'outputs': {'result': common}}
        driver.save(area / 'outcome.json', child)
        supervisor = {**child, 'child_outcome': driver.ref(area / 'outcome.json')}
        driver.save(area / 'supervisor-outcome.json', supervisor)
        return config, common

    def test_prior_requires_child_and_supervisor(self):
        config, common = self.prior_fixture()
        with ByteVerificationPass() as verify:
            driver.prior(config, common, common, 'witnesses', verify)
        (self.root / 'derive/supervisor-outcome.json').unlink()
        with ByteVerificationPass() as verify:
            with self.assertRaises(FileNotFoundError):
                driver.prior(config, common, common, 'witnesses', verify)

    def test_prior_failure_blocks_downstream(self):
        config, common = self.prior_fixture('failed')
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(ValueError, 'predecessor'):
                driver.prior(config, common, common, 'witnesses', verify)

    def test_prior_requires_matching_supervisor_attestation(self):
        config, common = self.prior_fixture()
        path = self.root / 'derive/supervisor-outcome.json'
        value = json.loads(path.read_text())
        value['child_outcome'] = common
        path.write_text(json.dumps(value))
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(ValueError, 'attest'):
                driver.prior(config, common, common, 'witnesses', verify)

    def test_science_review_required(self):
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(ValueError, 'independent'):
                driver.stage_review({}, {}, {}, None, verify)

    def test_declared_budgets_unchanged(self):
        self.assertEqual(driver.BOUNDS, {'exposed': 1800, 'derive': 1800, 'witnesses': 2100,
            'science': 15000, 'science-postflight': 1800, 'freeze-timing': 1800, 'collect': 30000, 'decide': 1800})
        self.assertNotIn('generate', driver.ACTIONS)

    def test_owned_helper_wrong_hash_rejected_without_import(self):
        bound = self.file({'not': 'code'})
        with self.assertRaisesRegex(ValueError, 'ownership|owned'):
            driver.owned({'sources': {'owned_runner': bound}})

    def test_actual_source_and_python_alias_bindings(self):
        common = self.file({'unused_generator': True})
        sources = {key: common for key in driver.confirmation.SOURCE_KEYS}
        for key, path in {'confirmation': driver.confirmation.__file__,
                          'validation': Path(driver.__file__).with_name('routing_validation.py'),
                          'failure_validator': driver.confirmation.failure.__file__,
                          'bootstrap': driver.__file__, 'supervisor': driver.__file__, 'witness_adapter': driver.__file__,
                          'driver_tests': __file__, 'python': sys.executable,
                          'python_environment': Path(sys.executable).parent.parent / 'pyvenv.cfg',
                          'owned_runner': driver.ROOT / 'target/tmp/auto-routing/many/activation/check_public.py'}.items():
            sources[key] = driver.ref(path)
        config = {'sources': sources, 'python_command': str(Path(sys.executable).absolute())}
        with ByteVerificationPass() as verify:
            driver.bind_sources(config, verify)
            self.assertIn(config['python_command'], verify.aliases)
        wrong = copy.deepcopy(config)
        wrong['sources']['bootstrap'] = common
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(ValueError, 'wrong actual source'):
                driver.bind_sources(wrong, verify)
        wrong = copy.deepcopy(config)
        wrong['python_command'] = sources['python']['path']
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(ValueError, 'invocation alias'):
                driver.bind_sources(wrong, verify)

    def test_unrelated_original_b_context_is_rejected(self):
        common = self.file({'value': True})
        config = {'original_inputs': {'declaration': self.file(routing.seal({'b_context': common}))},
                  'old_b_context': self.file({'wrong': True})}
        with ByteVerificationPass() as verify:
            with self.assertRaisesRegex(ValueError, 'original admitted B'):
                driver.exposed(config, verify, common)

    def test_worker_cannot_repeat_math_with_existing_handoff(self):
        common = self.file({'immutable': True})
        case = self.root / 'sample.case'
        case.write_text('synthetic')
        source = driver.ref(case)
        driver.save(self.root / 'derive/context.json', routing.seal({'cells': [
            {'case_sha256': source['sha256'], 'body_sha256': 'body'}]}))
        context = driver.ref(self.root / 'derive/context.json')
        config = {'sources': {'test': common}, 'declaration': common, 'output_root': str(self.root)}
        ready = self.root / 'witnesses/ready'
        ready.mkdir(parents=True)
        handoff = self.file(routing.seal({'config': common, 'review': common,
            'sources': config['sources'], 'context': context, 'ready': str(ready)}))
        output = ready / 'sample-oracle.json'
        driver.save(output.with_name(output.stem + '-start.json'), {'kind': driver.KIND + '_oracle_supervisor_start',
            'status': 'started', 'source': source, 'handoff': handoff, 'config': common, 'review': common,
            'command': driver.oracle_command(common, common, handoff, source, output)})
        driver.save(self.root / 'target/tmp/auto-routing/many/confirmation-repair-execution.json',
                    {'config': common, 'review': common, 'declaration': common})
        def write(*_):
            driver.save(output, {'synthetic_oracle': True})
        with patch.object(driver, 'ROOT', self.root), patch.object(driver, 'configuration', return_value=config), \
                patch.object(driver, 'source_case', return_value=(case, False)), \
                patch.object(driver.routing_oracle, 'write_reference', side_effect=write) as producer, \
                patch.object(driver.routing_oracle, 'load_reference'):
            driver.witness_worker(handoff, source, output, common, common)
            with self.assertRaises(FileExistsError):
                driver.witness_worker(handoff, source, output, common, common)
            self.assertEqual(producer.call_count, 1)
        outcome = routing.read_json(output.with_suffix('.worker-outcome.json'))
        self.assertEqual(outcome['status'], 'passed')
        self.assertEqual(outcome['metadata']['full_byte_checks'], 2)


@unittest.skipUnless(os.environ.get('FEREG_TEST_OWNED_PROCESS') == '1',
                     'explicit bounded ps/process integration requested separately')
class OwnedProcess(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        path = driver.ROOT / 'target/tmp/auto-routing/many/activation/check_public.py'
        self.helper = driver.owned({'sources': {'owned_runner': driver.ref(path)}})

    def test_real_success_path_types_and_pid_birth(self):
        entry = {}
        result = self.helper.run_owned([sys.executable, '-c', 'print("synthetic owned success")'],
                                      self.root / 'success.log', 5, entry)
        self.assertEqual(result['exit_code'], 0)
        self.assertTrue(entry['ownership']['identities'])
        self.assertTrue(all(isinstance(key, int) and isinstance(value, str)
                            for key, value in entry['ownership']['identities'].items()))
        self.assertIn('synthetic owned success', (self.root / 'success.log').read_text())

    def test_real_timeout_cleans_descendant_and_no_retry(self):
        entry = {}
        script = ('import subprocess,sys,time; '
                  'p=subprocess.Popen([sys.executable,"-c","import time; time.sleep(60)"]); '
                  'print(p.pid,flush=True); time.sleep(60)')
        with self.assertRaises(TimeoutError):
            self.helper.run_owned([sys.executable, '-c', script], self.root / 'timeout.log', 1, entry)
        ownership = entry['ownership']
        self.assertGreaterEqual(len(ownership['identities']), 2)
        self.assertIs(ownership['cleanup']['complete'], True)
        self.assertEqual(ownership['cleanup']['survivors'], [])
        with self.assertRaises(FileExistsError):
            self.helper.run_owned([sys.executable, '-c', 'pass'], self.root / 'timeout.log', 1, {})


if __name__ == '__main__':
    unittest.main()
