from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "scripts/generate_feppml_qualification_recipes.py"
SPEC = importlib.util.spec_from_file_location("feppml_recipe_test", PATH)
assert SPEC is not None and SPEC.loader is not None
RECIPES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RECIPES)


class FeppmlQualificationRecipeTests(unittest.TestCase):
    def test_compiler_closes_33_cells_and_separates_timing_from_audit(self) -> None:
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
            self.assertEqual(len(document["cells"]), 33)
            self.assertEqual(len({cell["cell_id"] for cell in document["cells"]}), 33)
            self.assertTrue(document["normal_timing_excludes_allocation_audit"])
            for cell in document["cells"]:
                commands = cell["commands"]
                self.assertTrue(commands["candidate_science"].startswith("_fereg_estimate_ppml "))
                self.assertIn("dropreason(__q_drop)", commands["candidate_science"])
                self.assertIn("allocationaudit", commands["candidate_science"])
                self.assertNotIn("allocationaudit", commands["candidate_timing"])
                self.assertIn("allocationaudit", commands["candidate_allocation_audit"])
                self.assertTrue(commands["comparator_timing"].startswith("ppmlhdfe "))
                self.assertTrue((root / cell["candidate_science_do"]).is_file())
                self.assertTrue((root / cell["comparator_science_do"]).is_file())
                self.assertTrue(cell["expected_status_map"])
                self.assertEqual(cell["probe_rows"], 128)
                self.assertIn("candidate_source_provenance", cell)
                self.assertIn("comparator_source_provenance", cell)
                self.assertEqual(
                    len(cell["candidate_source_provenance"]["scalars"])
                    - len(cell["comparator_source_provenance"]["scalars"]),
                    6,
                )
                generator = root / cell["generator"]
                self.assertTrue(generator.is_file())
                source = generator.read_text(encoding="ascii")
                self.assertIn(f"set seed {cell['seed']}", source)
                self.assertIn(f"set obs {cell['n']}", source)
                self.assertIn("sort obs_id", source)
                for index in range(1, 6):
                    self.assertIn(f"generate long cluster{index}", source)
                self.assertIn("FEPPML_DATASET_COMPLETE", source)
            by_id = {cell["cell_id"]: cell for cell in document["cells"]}
            self.assertEqual(
                by_id["route_n1m_two_fe_low_t8"]["dataset_id"],
                by_id["route_n1m_two_fe_auto_t8"]["dataset_id"],
            )
            self.assertEqual(
                by_id["gov_two_fe_standard_k10_t1"]["dataset_id"],
                by_id["gov_two_fe_standard_k10_t16"]["dataset_id"],
            )
            self.assertEqual(
                by_id["save_prediction_two_fe_t8"]["dataset_id"],
                by_id["gov_two_fe_standard_k10_t8"]["dataset_id"],
            )

    def test_recipe_matrix_contains_required_extremes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            document = RECIPES.compile_recipes(
                ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json",
                Path(temporary),
            )
            commands = [cell["commands"] for cell in document["cells"]]
            joined = "\n".join(value for command in commands for value in command.values())
            for token in (
                "threads(1)", "threads(16)", "memory(fast)", "memory(low)",
                "fweight(fw)", "pweight(pw)", "cluster1 cluster2 cluster3 cluster4 cluster5",
                "exposure(exposure)", "offset(offset)", "fe1##c.slope1",
            ):
                self.assertIn(token, joined)

    def test_checked_in_source_never_routes_timing_through_audit(self) -> None:
        source = PATH.read_text(encoding="utf-8")
        self.assertIn('candidate_audit = candidate + " allocationaudit"', source)
        self.assertIn('"candidate_timing": candidate', source)
        self.assertNotIn('"candidate_timing": candidate_audit', source)

    def test_science_export_is_lossless_closed_protocol_not_json(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            document = RECIPES.compile_recipes(
                ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json", root
            )
            for cell in document["cells"]:
                candidate = (root / cell["candidate_science_do"]).read_text(encoding="ascii")
                comparator = (root / cell["comparator_science_do"]).read_text(encoding="ascii")
                for source in (candidate, comparator):
                    self.assertIn("%24.17g", source)
                    self.assertIn('"HEADER" _tab "FEPPML_STATA_PROTOCOL"', source)
                    self.assertIn('"MASK" _tab "ASCII_MASK_V1"', source)
                    self.assertIn('"MASK_PROBE"', source)
                    self.assertIn('"END" _tab "FEPPML_STATA_RAW_COMPLETE"', source)
                    self.assertIn('"STATA" _tab "version"', source)
                    self.assertIn('_tab "LICENSED" _n', source)
                    self.assertNotIn('file write __q_protocol "{', source)
                    self.assertNotIn('"SAMPLE"', source)
                self.assertIn("e(feppml_status)", candidate)
                self.assertIn('local __q_text "`__q_effective_route\'"', candidate)
                self.assertIn("cond(missing(e(feppml_runtime)[28,1]),0,e(feppml_runtime)[28,1])", candidate)
                self.assertIn('local __q_status_text "."', comparator)
                self.assertNotIn("e(feppml_status)", comparator)
                self.assertIn("(e(df))", comparator)
                self.assertNotIn("(e(df_resid))", comparator)
                self.assertIn("local __q_N_physical_final = `__q_N_physical'", comparator)
                self.assertIn("local __q_null_mu = `__q_null_sum_qy' / `__q_null_sum_q'", comparator)
                self.assertIn("e(ll) * `__q_fitted_scale'", comparator)
                self.assertNotIn("offset=", comparator)
                self.assertIn('"`artifact_dir\'/`package\'/src"', comparator)
                self.assertIn("findfile ppmlhdfe.ado", comparator)
                self.assertIn("adopath ++", comparator)
                self.assertNotIn("adopath + \"", comparator)
                for dependency in ("reghdfe", "ftools", "require"):
                    self.assertIn(f"DEPENDENCY|`__q_resolved_{dependency}'", comparator)
                self.assertIn("set processors", comparator)

            saved = next(
                cell for cell in document["cells"]
                if cell["cell_id"] == "save_individual_fe_three_fe_t8"
            )
            for key in ("candidate_science_do", "comparator_science_do"):
                source = (root / saved[key]).read_text(encoding="ascii")
                self.assertNotIn("rowtotal", source)
                self.assertIn('"OUTPUT" _tab "individual_fe" _tab "saved_channels"', source)
            collinear = next(
                cell for cell in document["cells"]
                if cell["cell_id"] == "gov_collinear_two_fe_k50_t8"
            )
            comparator = (root / collinear["comparator_science_do"]).read_text(encoding="ascii")
            self.assertIn("if __q_native_b[1,`__q_native_", comparator)
            self.assertIn("if __q_native_V[`__q_native_", comparator)
            prediction = next(
                cell for cell in document["cells"]
                if cell["cell_id"] == "save_prediction_two_fe_t8"
            )
            self.assertIn("d(__q_d)", prediction["commands"]["comparator_science"])
            prediction_source = (root / prediction["comparator_science_do"]).read_text(encoding="ascii")
            self.assertIn('"__q_d" _tab "NATIVE"', prediction_source)

    def test_frozen_status_map_marks_collinearity_and_absorbed_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            document = RECIPES.compile_recipes(
                ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json",
                Path(temporary),
            )
            collinear = next(
                cell for cell in document["cells"]
                if cell["cell_id"] == "gov_collinear_two_fe_k50_t8"
            )
            indexed = {record["name"]: record["status_code"] for record in collinear["expected_status_map"]}
            self.assertEqual(indexed["o.c2"], 6)
            self.assertEqual(indexed["o._cons"], 7)
            self.assertNotIn("c2", indexed)

            generator = (Path(temporary) / collinear["generator"]).read_text(encoding="ascii")
            for control in range(1, 51):
                multiplier = 2 * control + 1
                self.assertNotEqual(multiplier % 1009, 0)
                self.assertIn(
                    f"generate double c{control} = mod({multiplier} * _n, 1009)",
                    generator,
                )
                if control != 2:
                    self.assertEqual(indexed[f"c{control}"], 1)

    def test_comparator_proof_mutations_fail_recipe_validation(self) -> None:
        manifest = RECIPES.performance.load_manifest(
            ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json"
        )
        cells = {cell["cell_id"]: cell for cell in RECIPES.performance.expand_cells(manifest)}
        for cell_id in ("gov_collinear_two_fe_k50_t8", "save_individual_fe_three_fe_t8"):
            cell = cells[cell_id]
            statuses = RECIPES.expected_status_map(cell)
            source = RECIPES.science_do(
                cell, RECIPES.commands(cell)["comparator_science"],
                candidate=False, statuses=statuses,
                probe_rows=manifest["generator_contract"]["probe_rows"],
            )
            proofs = [
                "_ms_omit_info __q_native_b",
                "if e(rank) != ",
            ]
            if cell["output"] in {"total_absorbed_effect", "individual_fe"}:
                proofs.append("replace __q_d = __q_d + _b[_cons] if e(sample)")
            if cell["output"] == "individual_fe":
                proofs.append("replace __q_fe1 = __q_fe1 + _b[_cons] if e(sample)")
            if cell["finite_design"] == "collinear":
                proofs.extend([
                    'local __q_native_2 = colnumb(__q_native_b, "o.c2")',
                    "if __q_native_b[1,`__q_native_2'] != 0 exit 459",
                    "if __q_native_V[`__q_native_2',`__q_native_column'] != 0 exit 459",
                    "if __q_native_V[`__q_native_column',`__q_native_2'] != 0 exit 459",
                ])
            for proof in proofs:
                with self.subTest(cell=cell_id, proof=proof):
                    mutated = source.replace(proof, "MUTATED", 1)
                    with self.assertRaises(RECIPES.RecipeError):
                        RECIPES.validate_science_recipe(
                            mutated, cell, candidate=False, statuses=statuses
                        )

    def test_compiled_command_semantic_mutations_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json"
            document = RECIPES.compile_recipes(manifest, root)
            for mutation in ("tiny_model", "option", "timed_work"):
                changed = json.loads(json.dumps(document))
                commands = changed["cells"][0]["commands"]
                if mutation == "tiny_model":
                    commands["candidate_timing"] = commands["candidate_timing"].replace("y", "y c.c1", 1)
                elif mutation == "option":
                    commands["candidate_science"] = commands["candidate_science"].replace(" deterministic", "", 1)
                else:
                    commands["comparator_timing"] += " quietly count"
                with self.subTest(mutation=mutation), self.assertRaises(RECIPES.RecipeError):
                    RECIPES.validate_compiled_recipes(changed, manifest, root)


if __name__ == "__main__":
    unittest.main()
