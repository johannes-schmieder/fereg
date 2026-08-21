from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PARSER = (ROOT / "stata/stfixest_parse.ado").read_text(encoding="utf-8")


class StataParserStaticTests(unittest.TestCase):
    def test_candidate_sample_is_marked_without_expanding_varlist(self):
        self.assertIn("marksample touse, novarlist", PARSER)

    def test_factor_metadata_is_used_without_observation_scale_expansion(self):
        lower = PARSER.lower()
        self.assertIn("fvexpand", lower)
        self.assertIn("_ms_parse_parts", lower)
        code = "\n".join(
            line for line in lower.splitlines() if not line.lstrip().startswith(("*", "/*"))
        )
        self.assertNotRegex(code, r"\bfvrevar\b")
        self.assertNotRegex(code, r"\begen\s+group\b")
        self.assertNotRegex(code, r"\btabulate\b.*\bgenerate\s*\(")

    def test_factor_flags_come_from_stata_metadata_not_operator_text(self):
        lower = PARSER.lower()
        self.assertIn("r(base)", lower)
        self.assertIn("r(level)", lower)
        self.assertIn("r(ts_op)", lower)
        self.assertNotIn("level_text", lower)

    def test_absorbed_slopes_and_cluster_tuples_have_numeric_rows(self):
        self.assertIn("absorb_component_meta", PARSER)
        self.assertIn("cluster_component_meta", PARSER)
        self.assertRegex(PARSER, re.compile(r'"##c\."'))
        self.assertRegex(PARSER, re.compile(r'"#c\."'))


if __name__ == "__main__":
    unittest.main()
