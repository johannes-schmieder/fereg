"""Binding and drift guards only: no subprocess, native call or timing panel."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cmg_setup as setup
import cmg_setup_profile as profile
import qualify as q

REPO = Path(q.__file__).resolve().parents[1]


class ProfileBindings(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.plan = self.root / 'plan-Linux.json'
        self.base_path = self.root / 'runner/target/tmp/cmg-upgrade/diagnostic-Linux/run-0-0/results.json'
        self.plugin = self.root / 'artifacts/Linux/setup-profile.plugin'
        self.plugin.parent.mkdir(parents=True)
        self.plugin.write_bytes(b'instrumented plugin fixture')
        (self.root / 'setup-profile.patch').write_bytes((REPO / 'bench/cmg_setup_profile.patch').read_bytes())
        for name in ('current.plugin', 'old.plugin', 'harness', 'input.case', 'oracle.json'):
            (self.root / name).write_bytes(name.encode())
        self.manifest = {**setup.fixed('Linux'), 'schema': 1, 'warmups': 1, 'platform': 'Linux',
                         'seed': 2026102103, 'harness': 'harness', 'artifacts': {}, 'cases': [],
                         'execution_sources': {name: q.digest(path) for name, path in q.execution_sources().items()},
                         'files': {name: q.digest(self.root / name) for name in
                                   ('current.plugin', 'old.plugin', 'harness', 'input.case', 'oracle.json')}}
        for role in setup.ARMS:
            self.manifest['artifacts'][role] = {'path': 'old.plugin' if role == setup.PRE else 'current.plugin',
                'revision': setup.REVISIONS[role], 'build': 'synthetic', **setup.artifact_policy(role)}
        for name in setup.CASES:
            self.manifest['cases'].append({'id': name, 'family': name, 'input': 'input.case',
                'memory_limit_bytes': None, 'oracle': {'kind': 'routing', 'path': 'oracle.json'}})
        self.base = {'status': 'complete', 'failures': [], 'budget_compliant': True, 'platform': 'Linux',
                     'replica': 0, 'shard': 0, 'execution_sources': self.manifest['execution_sources'],
                     'cases': [{'id': f'{name}-t{w}', 'status': 'complete',
                                'aliases': q.artifact_aliases(self.manifest),
                                'warmups': [{'execution': setup.POST, 'fingerprint': 'a' * 64}]}
                               for name in setup.CASES for w in (1, 4, 8)]}
        self.save()
        self.native = patch.object(profile.subprocess, 'run', side_effect=AssertionError('native call forbidden')).start()
        self.addCleanup(patch.stopall)
        self.addCleanup(lambda: self.native.assert_not_called())

    def save(self):
        q.write(self.plan, self.manifest)
        self.base['manifest_sha256'] = q.routing.identity(self.manifest)
        q.write(self.base_path, self.base)

    def load(self):
        return profile.load_context(REPO, self.root, 'Linux')

    def test_binds_all_inputs_and_sources_once_then_only_stats(self):
        with patch.object(q, 'digest', wraps=q.digest) as digest:
            plan, manifest, base, plugin, identity, guard = self.load()
            count = digest.call_count
            guard.check()
            guard.check()
            self.assertEqual(digest.call_count, count)
            self.assertEqual(sum(Path(call.args[0]).resolve() == (self.root / 'current.plugin').resolve() for call in digest.call_args_list), 1)
        self.assertEqual(identity['manifest_sha256'], q.routing.identity(manifest))
        self.assertEqual(identity['baseline_result_sha256'], q.digest(self.base_path))
        self.assertEqual(identity['helper_sha256'], q.digest(Path(profile.__file__)))
        self.assertEqual(identity['plugin_sha256'], q.digest(plugin))
        self.assertEqual(identity['patch_sha256'], q.digest(REPO / 'bench/cmg_setup_profile.patch'))
        self.assertEqual(identity['calls'], 18)

    def test_stale_harness_or_other_frozen_input_rejected(self):
        for name in ('harness', 'input.case', 'current.plugin', 'oracle.json'):
            target = self.root / name
            original = target.read_bytes()
            target.write_bytes(b'stale')
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'input identity mismatch'):
                self.load()
            target.write_bytes(original)

    def test_missing_harness_binding_rejected(self):
        del self.manifest['files']['harness']
        self.save()
        with self.assertRaisesRegex(ValueError, 'omits a required file binding'):
            self.load()

    def test_stale_module_binding_rejected(self):
        self.manifest['execution_sources'] = dict(self.manifest['execution_sources'], routing_science='0' * 64)
        self.save()
        with self.assertRaisesRegex(ValueError, 'execution source identity mismatch'):
            self.load()

    def test_baseline_must_match_manifest_and_modules(self):
        for field, value in [('manifest_sha256', '0' * 64), ('execution_sources', {}), ('replica', 1),
                             ('platform', 'Darwin')]:
            changed = {**self.base, field: value}
            q.write(self.base_path, changed)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'not bound'):
                self.load()
        self.save()

    def test_wrong_hidden_revision_is_rejected(self):
        self.manifest['artifacts'][setup.POST]['revision'] = setup.REVISIONS[setup.PRE]
        self.save()
        with self.assertRaisesRegex(ValueError, 'revision'):
            self.load()

    def test_incomplete_or_failed_reference_is_rejected(self):
        for field, value in [('status', 'running'), ('failures', [{'kind': 'native'}]), ('budget_compliant', False)]:
            q.write(self.base_path, {**self.base, field: value})
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'completed baseline'):
                self.load()
        changed = copy.deepcopy(self.base)
        changed['cases'][0]['warmups'] = []
        q.write(self.base_path, changed)
        with self.assertRaisesRegex(ValueError, 'scientific fingerprint'):
            self.load()

    def test_patch_must_equal_maintained_instrumentation(self):
        (self.root / 'setup-profile.patch').write_text('unreviewed patch')
        with self.assertRaisesRegex(ValueError, 'input identity mismatch'):
            self.load()

    def test_drift_in_plan_baseline_plugin_patch_and_harness_is_detected(self):
        for path in (self.plan, self.base_path, self.plugin, self.root / 'setup-profile.patch', self.root / 'harness'):
            guard = self.load()[-1]
            original = path.read_bytes()
            path.write_bytes(original + b' ')
            with self.subTest(path=path.name), self.assertRaisesRegex(ValueError, 'changed during execution'):
                guard.check()
            path.write_bytes(original)

    def test_plugin_alias_retarget_is_detected_even_for_identical_bytes(self):
        original = self.plugin.read_bytes()
        self.plugin.unlink()
        first, second = self.root / 'first.plugin', self.root / 'second.plugin'
        first.write_bytes(original)
        second.write_bytes(original)
        self.plugin.symlink_to(first)
        guard = self.load()[-1]
        self.plugin.unlink()
        self.plugin.symlink_to(second)
        with self.assertRaisesRegex(ValueError, 'alias changed'):
            guard.check()

    def test_execution_source_alias_change_is_detected(self):
        guard = self.load()[-1]
        changed = {**q.execution_sources(), 'qualify': self.root / 'harness'}
        with patch.object(q, 'execution_sources', return_value=changed), \
             self.assertRaisesRegex(ValueError, 'source aliases changed'):
            guard.check()

    def test_generated_request_drift_is_detected(self):
        guard = self.load()[-1]
        request = self.root / 'prepared.case'
        request.write_bytes(b'prepared request')
        guard.watch_request(request)
        request.write_bytes(b'changed request')
        with self.assertRaisesRegex(ValueError, 'changed during execution'):
            guard.check()


if __name__ == '__main__':
    unittest.main()
