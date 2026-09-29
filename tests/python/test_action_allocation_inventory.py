from pathlib import Path
import json
import tempfile
import unittest
from scripts import action_allocation_inventory as inventory

ROOT = Path(__file__).resolve().parents[2]


class ActionAllocationInventoryTests(unittest.TestCase):
    def test_current_source_matches(self):
        self.assertEqual(inventory.audit(ROOT, inventory.DEFAULT_INVENTORY), [])

    def test_ppml_session_helpers_literals_and_lock_are_closed(self):
        # Tiny isolated source tree keeps the mutation oracle independent of
        # the production inventory and exercises every separately watched scope.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 3\n')
            paths = ['legacy/fereg-core/src/ols/linear_session.rs',
                     'legacy/fereg-core/src/ppml/memory.rs',
                     'legacy/fereg-core/src/ppml_separation/runner/accounting.rs',
                     'legacy/fereg-core/src/memory/new_model.rs',
                     'crates/fereg-plugin/src/ppml_result.rs']
            original = 'fn work(n: usize) { let x = vec![0.0; n]; let y = "limit"; }\n'
            for name in paths:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(original)
            manifest = root / 'inventory.json'
            manifest.write_text(json.dumps(inventory.build_inventory(root)))
            self.assertEqual(inventory.audit(root, manifest), [])
            for name in paths:
                path = root / name
                for changed in [original.replace('n];', '2 * n];'), original.replace('"limit"', '"other"')]:
                    path.write_text(changed)
                    self.assertEqual(inventory.audit(root, manifest), [f'action allocation source changed: {name}'])
                path.write_text('// harmless formatting\n' + original)
                self.assertEqual(inventory.audit(root, manifest), [])
            helper = root / 'legacy/fereg-core/src/ppml/new.rs'
            helper.write_text(original)
            self.assertIn('action allocation source set changed', inventory.audit(root, manifest))
            helper.unlink()
            (root / 'Cargo.lock').write_text('version = 4\n')
            self.assertIn('action inventory policy changed: dependency_lock_sha256', inventory.audit(root, manifest))

    def test_inventory_cannot_claim_byte_coverage(self):
        with tempfile.TemporaryDirectory() as directory:
            changed = json.loads(inventory.DEFAULT_INVENTORY.read_text())
            changed['coverage'] = 'complete'
            path = Path(directory) / 'invalid.json'
            path.write_text(json.dumps(changed))
            self.assertIn('action inventory policy changed: coverage', inventory.audit(ROOT, path))

    def test_local_runtime_source_and_helpers_are_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 3\n')
            runtime = root / 'vendor/rayon-core/src/sleep/mod.rs'
            runtime.parent.mkdir(parents=True)
            original = 'fn sleep(n: usize) { let work = vec![0_u8; n]; }\n'
            runtime.write_text(original)
            manifest = root / 'inventory.json'
            manifest.write_text(json.dumps(inventory.build_inventory(root)))
            runtime.write_text(original.replace('n];', '2 * n];'))
            self.assertIn('action allocation source changed: vendor/rayon-core/src/sleep/mod.rs',
                          inventory.audit(root, manifest))
            runtime.write_text(original)
            helper = runtime.with_name('helper.rs')
            helper.write_text('fn prepare() { let work = Vec::<u8>::new(); }\n')
            self.assertIn('action allocation source set changed', inventory.audit(root, manifest))
            helper.unlink()
            runtime.unlink()
            self.assertIn('action allocation source set changed', inventory.audit(root, manifest))

    def test_build_wiring_is_closed_even_with_unchanged_lock(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 3\n')
            names = ['Cargo.toml', '.cargo/config.toml', 'rust-toolchain.toml',
                     'legacy/fereg-core/Cargo.toml', 'legacy/fereg-core/build.rs',
                     'crates/fereg-plugin/Cargo.toml', 'vendor/rayon-core/Cargo.toml',
                     'vendor/rayon-core/Cargo.lock', 'vendor/rayon-core/build.rs']
            for name in names:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('original\n')
            manifest = root / 'inventory.json'
            manifest.write_text(json.dumps(inventory.build_inventory(root)))
            for name in names:
                path = root / name
                path.write_text('changed\n')
                self.assertIn(f'action build input changed: {name}', inventory.audit(root, manifest))
                path.write_text('original\n')
            self.assertEqual(inventory.audit(root, manifest), [])
            extra = root / 'crates/fereg-plugin/build.rs'
            extra.write_text('fn main() {}\n')
            self.assertIn('action build input set changed', inventory.audit(root, manifest))
            extra.unlink()
            (root / 'Cargo.toml').unlink()
            self.assertIn('action build input set changed', inventory.audit(root, manifest))

    def test_vendor_build_products_are_not_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 3\n')
            source = root / 'vendor/rayon-core/src/lib.rs'
            source.parent.mkdir(parents=True)
            source.write_text('pub fn prepare() {}\n')
            manifest = root / 'inventory.json'
            manifest.write_text(json.dumps(inventory.build_inventory(root)))
            artifact = root / 'vendor/rayon-core/target/debug/build/dependency/out/generated.rs'
            artifact.parent.mkdir(parents=True)
            artifact.write_text('generated source belongs to the build, not the source tree\n')
            self.assertEqual(inventory.audit(root, manifest), [])

    def test_old_schema_does_not_hide_existing_source_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 3\n')
            source = root / 'legacy/fereg-core/src/ppml/memory.rs'
            source.parent.mkdir(parents=True)
            source.write_text('fn work(n: usize) { let x = vec![0.; n]; }\n')
            old = inventory.build_inventory(root)
            old.pop('build_inputs', None)
            old.pop('build_input_globs', None)
            old['schema_version'] = 1
            manifest = root / 'inventory.json'
            manifest.write_text(json.dumps(old))
            source.write_text('fn work(n: usize) { let x = vec![0.; 2 * n]; }\n')
            errors = inventory.audit(root, manifest)
            self.assertIn('action inventory fields changed', errors)
            self.assertIn('action allocation source changed: legacy/fereg-core/src/ppml/memory.rs', errors)

    def test_duplicate_build_input_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 3\n')
            (root / 'Cargo.toml').write_text('[workspace]\n')
            expected = inventory.build_inventory(root)
            self.assertIn('build_inputs', expected)
            expected['build_inputs'].append(expected['build_inputs'][0])
            manifest = root / 'inventory.json'
            manifest.write_text(json.dumps(expected))
            self.assertIn('duplicate action build input', inventory.audit(root, manifest))
