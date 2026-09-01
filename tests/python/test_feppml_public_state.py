from __future__ import annotations

import unittest

from scripts.feppml_public_state import (
    ACTIVATED,
    FROZEN_PREACTIVATION_ADO,
    PREACTIVATION,
    PublicStateError,
    classify_public_ado,
    public_state,
)


ACTIVATED_ADO = """*! activated
program define feppml, eclass sortpreserve
    version 18.0
    _fereg_ppml `0'
end
"""

PREACTIVATION_ADO = FROZEN_PREACTIVATION_ADO


class FeppmlPublicStateTests(unittest.TestCase):
    def test_repository_is_in_one_exact_reviewed_state(self) -> None:
        self.assertIn(public_state(), {PREACTIVATION, ACTIVATED})

    def test_both_reviewed_states_are_recognized(self) -> None:
        self.assertEqual(classify_public_ado(PREACTIVATION_ADO), PREACTIVATION)
        self.assertEqual(classify_public_ado(ACTIVATED_ADO), ACTIVATED)

    def test_wrapper_mutations_fail_closed(self) -> None:
        mutations = (
            ACTIVATED_ADO.replace("sortpreserve", ""),
            ACTIVATED_ADO.replace("version 18.0", "version 17.0"),
            ACTIVATED_ADO.replace("_fereg_ppml `0'", "plugin call fereg_macos.plugin"),
            ACTIVATED_ADO.replace("_fereg_ppml `0'", "_fereg_ppml `0'\n    exit 0"),
        )
        for source in mutations:
            with self.subTest(source=source), self.assertRaises(PublicStateError):
                classify_public_ado(source)

    def test_stub_mutations_fail_closed(self) -> None:
        mutations = (
            PREACTIVATION_ADO.replace("exit 498", "exit 0"),
            PREACTIVATION_ADO.replace("marksample touse, novarlist\n", ""),
            PREACTIVATION_ADO.replace(
                "No partial or uncertified PPML estimate has been posted.",
                "PPML is ready.",
            ),
            PREACTIVATION_ADO.replace("exit 498", "_fereg_ppml `0'\n    exit 498"),
            PREACTIVATION_ADO.replace(
                "    exit 498\nend\n",
                "    _fereg_estimate_ppml y x\n    exit 498\nend\n",
            ),
        )
        for source in mutations:
            with self.subTest(source=source), self.assertRaises(PublicStateError):
                classify_public_ado(source)


if __name__ == "__main__":
    unittest.main()
