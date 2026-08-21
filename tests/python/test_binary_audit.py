from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "stfixest_audit_binary", ROOT / "scripts" / "audit_binary.py"
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class BinaryAuditTests(unittest.TestCase):
    def test_glibc_baseline_accepts_2_28(self):
        text = "Name: GLIBC_2.2.5 Name: GLIBC_2.17 Name: GLIBC_2.28"
        self.assertEqual(MODULE.enforce_glibc_baseline(text), (2, 28))

    def test_glibc_baseline_rejects_newer_symbol(self):
        with self.assertRaises(RuntimeError):
            MODULE.enforce_glibc_baseline("Name: GLIBC_2.34")

    def test_no_glibc_symbol_is_reported_as_none(self):
        self.assertIsNone(MODULE.enforce_glibc_baseline("not an ELF version table"))


if __name__ == "__main__":
    unittest.main()
