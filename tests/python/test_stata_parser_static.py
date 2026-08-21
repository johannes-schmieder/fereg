from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PARSER = (ROOT / "stata/stfixest_parse.ado").read_text(encoding="utf-8")


class StataParserStaticTests(unittest.TestCase):
    def test_candidate_sample_is_caller_owned(self):
        lower = PARSER.lower()
        self.assertIn("touse(varname numeric)", lower)
        self.assertNotIn("marksample touse", lower)
        self.assertIn("fvexpand `rhs' if `touse'", lower)

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

    def test_model_wire_is_v2_and_has_strict_component_registries(self):
        lower = PARSER.lower()
        self.assertIn("model_wire_major = 2", lower)
        self.assertIn("component_raw_operators", lower)
        self.assertIn("component_ts_operators", lower)
        self.assertIn("raw_op_slot", lower)
        self.assertIn("j(1, 13, 0)", lower)
        self.assertIn("emptymodel", lower)
        self.assertNotIn("local type_code 9", lower)

    def test_absorbed_slopes_and_cluster_tuples_have_numeric_rows(self):
        self.assertIn("absorb_component_meta", PARSER)
        self.assertIn("cluster_component_meta", PARSER)
        self.assertRegex(PARSER, re.compile(r'"##c\."'))
        self.assertRegex(PARSER, re.compile(r'"#c\."'))


if __name__ == "__main__":
    unittest.main()
