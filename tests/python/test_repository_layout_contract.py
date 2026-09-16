import unittest
from pathlib import Path
from scripts.repository_layout_contract import (
    public_ppml_wrapper_is_exact, status_errors, status_paths,
)

ROOT = Path(__file__).resolve().parents[2]


class RepositoryLayoutContractTests(unittest.TestCase):
    def test_archived_status_is_evidence_and_active_duplicates_fail(self):
        paths = ["STATUS.md", "benchmarks/reports/development/case/source/STATUS.md",
                 "build_evidence/source/STATUS.md", "docs/memory_model.md"]
        self.assertEqual(status_paths(paths), ["STATUS.md"])
        for duplicate in ("docs/STATUS.md", "HANDOFF.md", "research/validation.md"):
            self.assertEqual(len(status_paths(paths + [duplicate])), 2)

    def test_mutable_status_keeps_metadata_and_open_gates(self):
        source = (ROOT / "STATUS.md").read_text()
        self.assertEqual(status_errors(source), [])
        self.assertTrue(status_errors(source.replace("**Branch:**", "Branch:")))
        self.assertTrue(status_errors(source.replace("- [ ]", "- [x]")))

    def test_wrapper_remains_exact_after_adr0041(self):
        source = (ROOT / "stata/feppml.ado").read_text()
        self.assertTrue(public_ppml_wrapper_is_exact(source))
        self.assertTrue(public_ppml_wrapper_is_exact("// comment\n" + source))
        for mutation in (source.replace("eclass", "eclass sortpreserve"),
                         source.replace("end", "_fereg_ppml `0'\nend"),
                         source.replace("end", "plugin call __fereg_plugin\nend"),
                         source.replace("`0'", "anything")):
            self.assertFalse(public_ppml_wrapper_is_exact(mutation))
