"""Vendored runtime changes must invalidate development source identities."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from scripts import ols_arithmetic_development as ols
from scripts import ppml_auto_calibration as ppml


class VendoredSourceIdentityTests(unittest.TestCase):
    def test_runtime_changes_are_bound_but_reports_are_not(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / 'vendor/rayon-core/src/sleep/mod.rs'
            report = root / 'benchmarks/reports/receipt.json'
            artifact = root / 'vendor/rayon-core/target/debug/build-output'
            for path in [runtime, report, artifact]:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('before')
            names = '\0'.join(str(p.relative_to(root)) for p in [runtime, report, artifact]) + '\0'
            with patch.object(ppml.subprocess, 'check_output', return_value=names.encode()):
                for identity in [ols.source_identity, ppml.source_files]:
                    with self.subTest(identity=identity.__module__):
                        runtime.write_text('before')
                        before = identity(root)
                        report.write_text('after')
                        artifact.write_text('after')
                        self.assertEqual(identity(root), before)
                        runtime.write_text('after')
                        after = identity(root)
                        self.assertEqual(set(after), {'vendor/rayon-core/src/sleep/mod.rs'})
                        self.assertNotEqual(after, before)
