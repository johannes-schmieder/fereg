from __future__ import annotations

import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "scripts/generate_feppml_constrained_recipes.py"
SPEC = importlib.util.spec_from_file_location("feppml_constrained_recipes_test", PATH)
assert SPEC is not None and SPEC.loader is not None
RECIPES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RECIPES)

CAPTURE_PATH = ROOT / "scripts/feppml_qualification_capture.py"
CAPTURE_SPEC = importlib.util.spec_from_file_location(
    "feppml_capture_constrained_argv_test", CAPTURE_PATH
)
assert CAPTURE_SPEC is not None and CAPTURE_SPEC.loader is not None
CAPTURE = importlib.util.module_from_spec(CAPTURE_SPEC)
CAPTURE_SPEC.loader.exec_module(CAPTURE)


class GenerateFeppmlConstrainedRecipesTests(unittest.TestCase):
    def test_exact_fixture_attempts_and_limits_are_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            document = RECIPES.compile_recipes(
                ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json", root
            )
            RECIPES.validate_compiled_recipes(
                document,
                ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json",
                root,
            )
            self.assertEqual(
                [record["id"] for record in document["fixtures"]],
                ["low_succeeds_fast_fails", "no_route_fits"],
            )
            self.assertEqual(
                [[attempt["id"] for attempt in record["attempts"]] for record in document["fixtures"]],
                [["low", "fast"], ["auto"]],
            )
            self.assertEqual(
                [record["dataset_cell_id"] for record in document["fixtures"]],
                ["gov_collinear_two_fe_k50_t1"] * 2,
            )
            self.assertEqual(
                [
                    [attempt["memory_limit_bytes"] for attempt in record["attempts"]]
                    for record in document["fixtures"]
                ],
                [[20_000_000_000, 20_000_000_000], [1_500_000_000]],
            )
            for fixture in document["fixtures"]:
                for attempt in fixture["attempts"]:
                    self.assertGreater(attempt["memory_limit_bytes"], 512 * 1024 * 1024)
                    source = (root / attempt["do_file"]).read_text(encoding="ascii")
                    self.assertIn(f"memory({attempt['id']})", source)
                    self.assertIn("capture noisily _fereg_estimate_ppml", source)
                    self.assertIn("FEPPML_STATA_CONSTRAINED_COMPLETE", source)
                    self.assertIn("findfile _fereg_estimate_ppml.ado", source)
                    self.assertIn('file write __resolution "ARTIFACT|`__q_resolved_plugin\'"', source)
                    self.assertNotIn("ppmlhdfe", source)
                    self.assertNotIn("large_allocation_started", source)
                    self.assertNotIn("preflight_failure", source)
                    self.assertNotIn("selected_route", source)
                    self.assertNotIn("planner_allocator", source)

            for mutation in ("limit", "mode", "extra_work"):
                changed = copy.deepcopy(document)
                if mutation == "limit":
                    changed["fixtures"][0]["attempts"][0]["memory_limit_bytes"] += 1
                elif mutation == "mode":
                    changed["fixtures"][0]["attempts"][0]["id"] = "auto"
                else:
                    path = root / changed["fixtures"][0]["attempts"][0]["do_file"]
                    path.chmod(0o644)
                    path.write_text(path.read_text(encoding="ascii") + "quietly count\n", encoding="ascii")
                with self.subTest(mutation=mutation), self.assertRaises(RECIPES.ConstrainedRecipeError):
                    RECIPES.validate_compiled_recipes(
                        changed,
                        ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json",
                        root,
                    )
                if mutation == "extra_work":
                    break

    def test_constrained_child_argv_preserves_all_twelve_positions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ("stata-mp", "attempt.do", "data.dta", "plugin"):
                (root / name).write_bytes(name.encode("ascii"))
            plan = {
                "stata": {
                    "executable": root / "stata-mp",
                    "batch_arguments": ["-q", "-b", "do"],
                    "comparator_processor_limit": 16,
                }
            }
            cell = {
                "cell_id": "low_succeeds_fast_fails",
                "data": {"path": str(root / "data.dta")},
                "candidate": {
                    "do_file": {"path": str(root / "attempt.do")},
                    "artifact": {"path": str(root / "plugin")},
                    "science_output": "low.json",
                },
            }
            work = root / "raw"
            command, result, do_field = CAPTURE._child_command(
                plan,
                cell,
                "candidate",
                work,
                requested_threads=1,
                pair_index=-1,
                temperature="low",
                science_protocol=False,
                data_override=None,
            )
            self.assertEqual(do_field, "do_file")
            self.assertEqual(result, work / "low.json")
            self.assertEqual(
                command[-12:],
                [
                    str(root / "data.dta"),
                    str(root / "plugin"),
                    str(work / "low.json"),
                    str(work / "READY"),
                    str(work / "START"),
                    str(work / "DONE"),
                    "1",
                    "16",
                    "-1",
                    "low",
                    "low_succeeds_fast_fails",
                    "candidate",
                ],
            )

    def test_sources_are_reproducible_and_nonoverwriting(self) -> None:
        manifest = ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json"
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            left, right = Path(first), Path(second)
            RECIPES.compile_recipes(manifest, left)
            RECIPES.compile_recipes(manifest, right)
            self.assertEqual(
                {path.relative_to(left): path.read_bytes() for path in left.rglob("*") if path.is_file()},
                {path.relative_to(right): path.read_bytes() for path in right.rglob("*") if path.is_file()},
            )
            with self.assertRaisesRegex(RECIPES.ConstrainedRecipeError, "overwrite"):
                RECIPES.compile_recipes(manifest, left)


if __name__ == "__main__":
    unittest.main()
