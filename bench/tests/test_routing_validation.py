"""Synthetic integrity tests; no campaign inputs or native estimator access."""
import copy
import hashlib
import json
import mmap
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing_validation as validation


def empty(*args, **kwargs):
    return {'empty': True}


def modules(load=empty, bindings=empty, activation=empty, deployment_bindings=empty):
    blocks = types.SimpleNamespace(__name__='routing_blocks', __file__=__file__,
                                   load_context=load, context_bindings=bindings,
                                   qualification=empty, decide=empty)
    deployment = types.SimpleNamespace(__name__='routing_block_deployment', __file__=__file__,
                                       activation_core=activation, context_bindings=deployment_bindings,
                                       equivalence_calls=empty, decide=empty)
    return blocks, deployment


class VerificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='routing-validation-')
        self.root = Path(self.temp.name)
        self.path = self.root / 'metadata.json'
        self.path.write_text('{"value":[1]}')
        self.operations = []

    def tearDown(self):
        for operation in self.operations:
            if isinstance(operation, validation.MetadataValidation):
                operation.restore()
            operation.closed = True
        self.temp.cleanup()

    def byte_pass(self, **kwargs):
        operation = validation.ByteVerificationPass(**kwargs)
        self.operations.append(operation)
        return operation

    def metadata(self, pair, **kwargs):
        operation = validation.MetadataValidation(**kwargs)
        self.operations.append(operation)
        operation.install(*pair)
        return operation

    def test_first_full_hash_then_per_use_stamps_and_full_brackets(self):
        original = validation._full_digest
        with patch.object(validation, '_full_digest', wraps=original) as hashed:
            operation = self.byte_pass()
            sha = operation.digest(self.path)
            self.assertEqual(sha, hashlib.sha256(self.path.read_bytes()).hexdigest())
            self.assertEqual(operation.digest(self.path), sha)
            count = lambda: sum(Path(call.args[0]) == self.path.resolve()
                                for call in hashed.call_args_list)
            self.assertEqual(count(), 1)
            operation.before_producer()
            self.assertEqual(count(), 2)
            report = operation.finish()
            self.assertEqual(count(), 3)
            self.assertEqual(report['full_byte_checks'], 2)
            self.assertIn(str(Path(sys.executable).resolve()), operation.files)
            self.assertIn(str(Path(validation.__file__).resolve()), operation.files)
        with self.assertRaisesRegex(ValueError, 'closed'):
            operation.digest(self.path)

    def test_same_size_mutation_and_same_bytes_rewrite_poison(self):
        for content in ['{"value":[2]}', '{"value":[1]}']:
            with self.subTest(content=content):
                self.path.write_text('{"value":[1]}')
                operation = self.byte_pass()
                operation.digest(self.path)
                before = self.path.stat()
                self.path.write_text(content)
                os.utime(self.path, ns=(before.st_atime_ns, before.st_mtime_ns))
                with self.assertRaisesRegex(ValueError, 'identity changed'):
                    operation.digest(self.path)
                with self.assertRaises(ValueError):
                    operation.digest(Path(__file__))

    def test_replacement_identical_bytes_and_mode_changes_are_rejected(self):
        operation = self.byte_pass()
        operation.digest(self.path)
        replacement = self.root / 'replacement'
        replacement.write_bytes(self.path.read_bytes())
        replacement.replace(self.path)
        with self.assertRaisesRegex(ValueError, 'identity changed'):
            operation.digest(self.path)
        operation = self.byte_pass()
        operation.digest(self.path)
        self.path.chmod(0o400)
        with self.assertRaisesRegex(ValueError, 'identity changed'):
            operation.check_stats()

    def test_aliases_share_hash_and_retargeting_fails_through_canonical_name(self):
        alias = self.root / 'alias'
        alias.symlink_to(self.path)
        other = self.root / 'other'
        other.write_bytes(self.path.read_bytes())
        operation = self.byte_pass()
        self.assertEqual(operation.digest(alias), operation.digest(self.path))
        self.assertEqual(operation.aliases[str(alias)], str(self.path.resolve()))
        alias.unlink()
        alias.symlink_to(other)
        with self.assertRaisesRegex(ValueError, 'path target changed'):
            operation.digest(self.path)

    def test_missing_directory_and_hash_errors_poison(self):
        for path in [self.root / 'absent', self.root]:
            operation = self.byte_pass()
            with self.assertRaises((OSError, ValueError)):
                operation.digest(path)
            self.assertIsNotNone(operation.poisoned)
        operation = self.byte_pass()
        with patch.object(validation, '_full_digest', side_effect=OSError('read error')):
            with self.assertRaises(OSError):
                operation.digest(self.path)
        with self.assertRaises(ValueError):
            operation.digest(self.path)

    def test_drift_during_initial_hash_poison(self):
        operation = self.byte_pass()
        original = validation._full_digest
        def drift(path, observed=None):
            sha = original(path, observed)
            Path(path).write_text('{"value":[2]}')
            return sha
        with patch.object(validation, '_full_digest', side_effect=drift):
            with self.assertRaisesRegex(ValueError, 'during first hash'):
                operation.digest(self.path)
        self.assertIsNotNone(operation.poisoned)

    def test_final_byte_check_rehashes_even_with_indistinguishable_stats(self):
        operation = self.byte_pass()
        operation.digest(self.path)
        frozen = operation.files[str(self.path.resolve())]['stamp']
        original = validation._stamp
        self.path.write_text('{"value":[2]}')
        def same_stamp(path):
            return frozen if Path(path) == self.path.resolve() else original(path)
        with patch.object(validation, '_stamp', side_effect=same_stamp):
            with self.assertRaisesRegex(ValueError, 'bytes changed'):
                operation.finish()
        self.assertTrue(operation.closed)

    def test_bound_identity_and_closed_check_fail(self):
        operation = self.byte_pass()
        bound = {'path': str(self.path.resolve()), 'sha256': operation.digest(self.path)}
        self.assertEqual(operation.check(bound), self.path.resolve())
        operation.finish()
        with self.assertRaises(ValueError):
            operation.check(bound)
        operation = self.byte_pass()
        with self.assertRaisesRegex(ValueError, 'binding changed'):
            operation.check({**bound, 'sha256': '0' * 64})
        self.assertIsNotNone(operation.poisoned)

    def test_metadata_reuses_exact_arguments_and_deep_copies_results(self):
        calls = []
        def activation(value, *, equivalence):
            calls.append((copy.deepcopy(value), equivalence))
            return json.loads(self.path.read_text()), {'argument': value}
        pair = modules(activation=activation)
        operation = self.metadata(pair)
        f = pair[1].activation_core
        first = f({'x': 1}, equivalence=False)
        first[0]['value'].append(99)
        first[1]['argument']['x'] = 9
        second = f({'x': 1}, equivalence=False)
        self.assertEqual(second, ({'value': [1]}, {'argument': {'x': 1}}))
        second[0]['value'].append(77)
        self.assertEqual(f({'x': 1}, equivalence=False)[0], {'value': [1]})
        for value, equivalence in [({'x': 1}, True), ({'x': True}, False),
                                   ({'x': 1.0}, False), ({'x': 2}, False)]:
            f(value, equivalence=equivalence)
        self.assertEqual(len(calls), 5)
        self.assertEqual(operation.counts['routing_block_deployment.activation_core'],
                         {'hits': 2, 'misses': 5, 'uncached_executes': 5})
        operation.finish()
        with self.assertRaisesRegex(ValueError, 'closed'):
            f({'x': 1}, equivalence=False)

    def test_argument_container_type_order_and_keyword_presence_are_distinct(self):
        calls = []
        def load(*args, **kwargs):
            calls.append(1)
            return 1
        pair = modules(load=load)
        operation = self.metadata(pair)
        f = pair[0].load_context
        for args, kwargs in [(([1],), {}), (((1,),), {}), (({'a': 1, 'b': 2},), {}),
                              (({'b': 2, 'a': 1},), {}), ((), {}), ((), {'equivalence': False})]:
            f(*args, **kwargs)
            f(*args, **kwargs)
        self.assertEqual(len(calls), 6)
        operation.finish()

    def test_nested_cached_child_propagates_actual_external_read_closure(self):
        # Temporary file is outside the repository; no root exemption applies.
        pair = modules(activation=lambda *a, **k: self.path.read_text())
        pair[0].load_context = lambda bound: {'child': pair[1].activation_core(equivalence=False)}
        operation = self.metadata(pair)
        pair[1].activation_core(equivalence=False)
        pair[0].load_context({'path': 'logical'})
        entry = next(value for key, value in operation.cache.items()
                     if key[0] == 'routing_blocks.load_context')
        self.assertIn(str(self.path.resolve()), entry[1])
        self.path.write_text('{"value":[2]}')
        with self.assertRaisesRegex(ValueError, 'identity changed'):
            pair[0].load_context({'path': 'logical'})
        self.assertIsNotNone(operation.poisoned)

    def test_failed_validators_and_mutated_arguments_poison_without_cache(self):
        def broken(*args):
            self.path.read_bytes()
            raise RuntimeError('original failure')
        pair = modules(load=broken)
        operation = self.metadata(pair)
        with self.assertRaisesRegex(RuntimeError, 'original failure'):
            pair[0].load_context()
        self.assertEqual(operation.cache, {})
        with self.assertRaises(ValueError):
            pair[0].load_context()
        operation.restore()
        pair = modules(load=lambda value: value.update(changed=True))
        operation = self.metadata(pair)
        with self.assertRaisesRegex(ValueError, 'mutated its arguments'):
            pair[0].load_context({})
        self.assertEqual(operation.cache, {})

    def test_only_four_slots_memoize_and_hooks_restore_on_success_and_failure(self):
        pair = modules()
        originals = [(module, name, getattr(module, name)) for module, names in
                     [(pair[0], ('load_context', 'context_bindings', 'qualification', 'decide')),
                      (pair[1], ('activation_core', 'context_bindings', 'equivalence_calls', 'decide'))]
                     for name in names]
        operation = self.metadata(pair)
        self.assertIs(pair[0].qualification, empty)
        self.assertIs(pair[1].equivalence_calls, empty)
        self.assertIs(pair[1].decide, empty)
        operation.finish()
        for module, name, original in originals:
            self.assertIs(getattr(module, name), original)
        operation = self.metadata(pair)
        with self.assertRaisesRegex(RuntimeError, 'producer failed'):
            with operation:
                raise RuntimeError('producer failed')
        self.assertTrue(operation.closed)
        for module, name, original in originals:
            self.assertIs(getattr(module, name), original)

    def test_digest_aliases_intercept_only_metadata_and_restore_late_imports(self):
        import native_io
        original = native_io.digest
        alias_module = types.ModuleType('routing_validation_test_alias')
        alias_module.digest = original
        alias_module.unrelated_digest = lambda path: 'unrelated'
        sys.modules[alias_module.__name__] = alias_module
        self.addCleanup(sys.modules.pop, alias_module.__name__)
        calls = []
        def load(value):
            calls.append(1)
            return alias_module.digest(self.path), native_io.digest(self.path)
        pair = modules(load=load)
        operation = self.metadata(pair)
        with patch.object(validation, '_full_digest', wraps=validation._full_digest) as hashed:
            expected = pair[0].load_context(1)
            pair[0].load_context(2)
            self.assertEqual(sum(Path(call.args[0]) == self.path.resolve()
                                 for call in hashed.call_args_list), 1)
        self.assertEqual(alias_module.unrelated_digest(self.path), 'unrelated')
        # Outside metadata, the exact original function is invoked; changed bytes
        # are visible there, rather than being mistaken for immutable dependencies.
        self.path.write_text('{"value":[2]}')
        self.assertNotEqual(alias_module.digest(self.path), expected[0])
        alias_module.late_alias = native_io.digest
        operation.restore()
        self.assertIs(native_io.digest, original)
        self.assertIs(alias_module.digest, original)
        self.assertIs(alias_module.late_alias, original)

    def test_first_hash_precedes_validator_content_read(self):
        pair = modules(load=lambda: self.path.read_text())
        operation = self.metadata(pair)
        with patch.object(validation, '_full_digest', side_effect=OSError('first hash denied')):
            with self.assertRaisesRegex(OSError, 'first hash denied'):
                pair[0].load_context()
        self.assertEqual(operation.cache, {})

    def test_audit_denies_writes_descriptors_mmap_subprocess_and_mutations(self):
        with self.path.open('rb') as existing:
            attempts = [lambda: self.path.write_text('changed'),
                        lambda: open(existing.fileno(), 'rb', closefd=False),
                        lambda: os.open(self.path, os.O_RDONLY),
                        lambda: mmap.mmap(existing.fileno(), 0, access=mmap.ACCESS_READ),
                        lambda: subprocess.run([sys.executable, '-c', 'pass'], check=True),
                        lambda: self.path.unlink()]
            for attempt in attempts:
                pair = modules(load=attempt)
                operation = self.metadata(pair)
                with self.subTest(attempt=attempt), self.assertRaises(ValueError):
                    pair[0].load_context()
                self.assertIsNotNone(operation.poisoned)
                operation.restore()
                self.assertEqual(self.path.read_text(), '{"value":[1]}')
        # Audit dispatcher is inert after metadata scopes, including failures.
        self.path.write_text('outside writes remain permitted')

    def test_final_failure_restores_hooks_and_never_reports_success(self):
        pair = modules(load=lambda: self.path.read_text())
        original = pair[0].load_context
        operation = self.metadata(pair)
        pair[0].load_context()
        self.path.write_text('{"value":[2]}')
        with self.assertRaises(ValueError):
            operation.finish()
        self.assertTrue(operation.closed)
        self.assertIs(pair[0].load_context, original)
        self.assertIsNotNone(operation.report()['poisoned'])

    def test_sources_and_review_are_bound_and_nested_install_rejected(self):
        review = self.root / 'review.json'
        review.write_text('{"status":"passed"}')
        operation = self.metadata(modules(), sources=(review,))
        self.assertIn(str(review.resolve()), operation.files)
        self.assertIn(str(Path(__file__).resolve()), operation.files)
        second = validation.MetadataValidation()
        self.operations.append(second)
        with self.assertRaisesRegex(ValueError, 'another metadata operation'):
            second.install(*modules())
        operation.finish()


if __name__ == '__main__':
    unittest.main()
