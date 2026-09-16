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
            paths = ['crates/fereg-core/src/ols/linear_session.rs',
                     'crates/fereg-core/src/ppml/memory.rs',
                     'crates/fereg-core/src/ppml_separation/runner/accounting.rs',
                     'crates/fereg-core/src/memory/new_model.rs',
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
            helper = root / 'crates/fereg-core/src/ppml/new.rs'
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
