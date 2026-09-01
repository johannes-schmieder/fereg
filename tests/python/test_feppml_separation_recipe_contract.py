from __future__ import annotations

import copy
import importlib.util
import itertools
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "scripts/feppml_separation_recipe_contract.py"
ORACLE_PATH = ROOT / "tests/python/ppml_recession_cone_oracle.py"
QUALIFICATION_COMPILER_PATH = ROOT / "scripts/generate_feppml_qualification_recipes.py"
FIXTURE = ROOT / "tests/stata/parity_ppmlhdfe/feppml_separation_methods.do"


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


CONTRACT = load("feppml_phase7_contract_test", CONTRACT_PATH)
ORACLE = load("feppml_phase7_oracle_test", ORACLE_PATH)
QUALIFICATION_COMPILER = load(
    "feppml_phase7_qualification_compiler_test", QUALIFICATION_COMPILER_PATH
)


class FeppmlSeparationRecipeContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.document = CONTRACT.load_manifest()

    def test_every_accepted_subset_and_order_has_one_executable_recipe(self) -> None:
        methods = self.document["method_contract"]["methods"]
        sequences = CONTRACT.accepted_sequences(methods)
        self.assertEqual(len(sequences), 64)
        self.assertEqual(len(sequences), len(set(sequences)))
        self.assertEqual(
            {frozenset(sequence) for sequence in sequences},
            {
                frozenset(subset)
                for length in range(1, 5)
                for subset in itertools.combinations(methods, length)
            },
        )
        lines = CONTRACT.sequence_recipe_lines(self.document)
        source_lines = FIXTURE.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 64)
        self.assertTrue(all(source_lines.count(line) == 1 for line in lines))
        for method, reason in (("simplex", 10), ("ir", 11), ("mu", 12)):
            self.assertTrue(any(f'"{method}" {reason} ' in line for line in lines))

    def test_exact_oracle_witnesses_satisfy_every_sign_condition(self) -> None:
        for case in self.document["cases"]:
            with self.subTest(case=case["id"]):
                separated, witnesses = ORACLE.separated_rows(
                    case["design"], case["outcome"]
                )
                self.assertEqual(list(separated), case["expected_separated_rows"])
                self.assertEqual(set(witnesses), set(separated))
                for target, direction in witnesses.items():
                    values = ORACLE.evaluate_direction(case["design"], direction)
                    self.assertLess(values[target], 0)
                    for outcome, value in zip(case["outcome"], values):
                        if ORACLE.rational(outcome) > 0:
                            self.assertEqual(value, 0)
                        else:
                            self.assertLessEqual(value, 0)

    def test_each_stata_oracle_block_is_the_exact_manifest_recipe(self) -> None:
        fixture = FIXTURE.read_text(encoding="utf-8")
        for case in self.document["cases"]:
            with self.subTest(case=case["id"]):
                identifier = case["id"]
                begin = f"* PHASE7_RECIPE_BEGIN|{identifier}"
                end = f"* PHASE7_RECIPE_END|{identifier}"
                observed = fixture[
                    fixture.index(begin):fixture.index(end) + len(end)
                ]
                self.assertEqual(observed, CONTRACT.stata_case_recipe(case))

        slope = next(
            case for case in self.document["cases"] if case["id"] == "absorbed_slope"
        )
        self.assertEqual(len(slope["stata"]["rows"]), 8)
        self.assertEqual(len(slope["design"]), 8)
        self.assertIn("absorb(fe##c.slope)", CONTRACT.stata_case_recipe(slope))

        boundary = next(
            case for case in self.document["cases"] if case["id"] == "boundary_infeasible"
        )
        self.assertEqual(boundary["terminal_rc"], 0)
        self.assertIn("assert e(separated) == 0", CONTRACT.stata_case_recipe(boundary))
        self.assertNotIn("assert _rc == 430", CONTRACT.stata_case_recipe(boundary))

    def test_catalog_covers_numerical_weight_adjustment_and_simplex_edges(self) -> None:
        indexed = {case["id"]: case for case in self.document["cases"]}
        self.assertEqual(indexed["pweight_active"]["expected_separated_rows"], [2, 5])
        self.assertEqual(indexed["exposure_active"]["expected_separated_rows"], [2, 5])
        self.assertEqual(indexed["fweight_singleton_multiplicity"]["weight"]["values"][0], 2)
        self.assertEqual(indexed["fweight_singleton_multiplicity"]["expected_singleton_rows"], [])
        classes = {case["simplex_class"] for case in indexed.values()}
        for required in (
            "finite_feasible", "full_positive_rank", "cone_infeasible",
            "degenerate_duplicate_columns", "zero_null_direction", "fe_only",
            "mixed_finite_fe", "absorbed_slope", "empty_terminal_sample",
        ):
            self.assertIn(required, classes)

    def test_comparator_eligible_cases_execute_once_in_pinned_qualification(self) -> None:
        eligible = [
            case for case in self.document["cases"]
            if case["comparator"]["mode"] == "pinned_ppmlhdfe"
        ]
        oracle_only = [
            case for case in self.document["cases"]
            if case["comparator"]["mode"] == "oracle_only"
        ]
        self.assertEqual(
            tuple(case["id"] for case in eligible), CONTRACT.COMPARATOR_ELIGIBLE_IDS
        )
        self.assertEqual(
            {case["id"] for case in oracle_only},
            {"boundary_infeasible", "fweight_singleton_multiplicity"},
        )
        self.assertTrue(all(case["comparator"]["rationale"] for case in oracle_only))
        expected_audit = CONTRACT.stata_comparator_audit(self.document)
        self.assertEqual(expected_audit.count("PHASE7_COMPARATOR_BEGIN|"), 10)
        self.assertNotIn("PHASE7_COMPARATOR_BEGIN|boundary_infeasible", expected_audit)
        self.assertNotIn(
            "PHASE7_COMPARATOR_BEGIN|fweight_singleton_multiplicity", expected_audit
        )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            recipes = QUALIFICATION_COMPILER.compile_recipes(
                ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json", root
            )
            hits = []
            for record in recipes["cells"]:
                source = (root / record["comparator_science_do"]).read_text(
                    encoding="ascii"
                )
                if "PHASE7_COMPARATOR_BEGIN|" in source:
                    hits.append(record["cell_id"])
                    self.assertIn(expected_audit, source)
                    self.assertIn("findfile ppmlhdfe.ado", source)
                    self.assertLess(
                        source.index("findfile ppmlhdfe.ado"),
                        source.index("PHASE7_COMPARATOR_BEGIN|"),
                    )
            self.assertEqual(hits, [QUALIFICATION_COMPILER.SEPARATION_AUDIT_CELL_ID])

    def test_catalog_and_stata_contract_fail_closed_under_mutation(self) -> None:
        mutated = copy.deepcopy(self.document)
        mutated["cases"][0]["expected_separated_rows"] = []
        with self.assertRaisesRegex(CONTRACT.SeparationRecipeError, "cone result"):
            CONTRACT.validate_manifest(mutated)

        mutated = copy.deepcopy(self.document)
        mutated["method_contract"]["accepted_sequence_count"] = 63
        with self.assertRaisesRegex(CONTRACT.SeparationRecipeError, "sequence contract"):
            CONTRACT.validate_manifest(mutated)

        mutated = copy.deepcopy(self.document)
        mutated["cases"][0]["unexpected"] = True
        with self.assertRaisesRegex(CONTRACT.SeparationRecipeError, "not closed"):
            CONTRACT.validate_manifest(mutated)

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative in (
                "tests/stata/parity_ppmlhdfe", "ci", "scripts", "stata"
            ):
                (root / relative).mkdir(parents=True, exist_ok=True)
            binding = self.document["formal_binding"]
            (root / binding["fixture"]).write_text(
                FIXTURE.read_text(encoding="utf-8").replace(
                    "PHASE7_ORACLE|finite_indicator|numerical_cone",
                    "PHASE7_ORACLE|finite_indicator|tampered",
                ),
                encoding="utf-8",
            )
            for relative in (
                binding["hidden_wrapper"], "ci/stata_profiles.json",
                binding["aggregate"], "stata/feppml.ado",
            ):
                source = ROOT / relative
                (root / relative).write_bytes(source.read_bytes())
            with self.assertRaisesRegex(CONTRACT.SeparationRecipeError, "byte-for-byte"):
                CONTRACT.validate_stata_and_formal_binding(self.document, root)

    def test_formal_binding_reaches_phase12_and_public_stub_stays_closed(self) -> None:
        CONTRACT.validate_stata_and_formal_binding(self.document)
        binding = self.document["formal_binding"]
        profiles = json.loads((ROOT / "ci/stata_profiles.json").read_text(encoding="utf-8"))[
            "profiles"
        ]
        self.assertIn(binding["hidden_wrapper"], profiles["ppml-hidden"]["fixtures"])
        aggregate = (ROOT / binding["aggregate"]).read_text(encoding="utf-8")
        self.assertIn('"stata_ppml_hidden"', aggregate)
        self.assertIn('"python"', aggregate)
        public = (ROOT / "stata/feppml.ado").read_text(encoding="utf-8")
        self.assertIn("exit 498", public)
        self.assertNotIn("_fereg_ppml", public)

    def test_contract_cli_is_deterministic_and_dependency_free(self) -> None:
        completed = subprocess.run(
            [sys.executable, "-I", "-B", "-S", str(CONTRACT_PATH)],
            cwd=ROOT,
            stdin=subprocess.DEVNULL,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(
            completed.stdout.strip(), "FEPPML_PHASE7_SEPARATION_CONTRACT_COMPLETE"
        )


if __name__ == "__main__":
    unittest.main()
