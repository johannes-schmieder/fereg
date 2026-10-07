from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
GENERATOR_PATH = ROOT / "scripts/generate_feppml_oracle_fixtures.py"
ORACLE_PATH = ROOT / "scripts/feppml_explicit_dummy_oracle.py"


def module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


GENERATOR = module("feppml_oracle_fixture_generator_test", GENERATOR_PATH)
ORACLE = module("feppml_oracle_fixture_loader_test", ORACLE_PATH)


class GenerateFeppmlOracleFixturesTests(unittest.TestCase):
    def test_compiler_emits_exact_seven_bounded_independent_models(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            document = GENERATOR.compile_fixtures(
                ROOT / "tests/fixtures/ppml_oracle_models.json", root
            )
            GENERATOR.validate_compiled_fixtures(
                document,
                ROOT / "tests/fixtures/ppml_oracle_models.json",
                root,
            )
            self.assertEqual(document["oracle_cell_ids"], list(GENERATOR.ORACLE_CELL_IDS))
            self.assertEqual(len(document["cells"]), 7)
            for record in document["cells"]:
                self.assertEqual(record["row_count"], 256)
                model = ORACLE.load_model(root / record["model"])
                self.assertEqual(model["cell_id"], record["cell_id"])
                generator = (root / record["generator"]).read_text(encoding="ascii")
                self.assertIn("set obs 256", generator)
                self.assertIn("save \"`1'\", replace", generator)
                self.assertIn("export delimited using \"`2'\"", generator)
                self.assertIn("FEPPML_ORACLE_FIXTURE_COMPLETE", generator)
                for index in range(1, 6):
                    self.assertIn(f"generate long cluster{index}", generator)
                for index in range(1, len(model["finite_terms"])):
                    self.assertIn(
                        f"c{index} = mod({2 * index + 1} * _n, 1009)",
                        generator,
                    )
                expected_probe_count = (
                    0
                    if model["expected_output"] == "fit"
                    else GENERATOR.OUTPUT_PROBE_COUNT
                )
                self.assertEqual(model["probe_count"], expected_probe_count)

    def test_registered_shapes_cover_all_oracle_dimensions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            document = GENERATOR.compile_fixtures(
                ROOT / "tests/fixtures/ppml_oracle_models.json", root
            )
            models = [ORACLE.load_model(root / record["model"]) for record in document["cells"]]
        self.assertEqual({len(item["fixed_effects"]) for item in models}, {0, 1, 2, 3})
        self.assertEqual({len(item["clusters"]) for item in models}, {0, 1, 2, 3, 4, 5})
        self.assertEqual({item["weight"]["kind"] for item in models}, {"none", "fweight", "pweight"})
        self.assertEqual({item["adjustment"]["kind"] for item in models}, {"none", "offset", "exposure"})
        self.assertTrue(any(item["separation"]["kind"] == "zero_fe_levels" for item in models))
        self.assertTrue(any(any(effect["slope"] for effect in item["fixed_effects"]) for item in models))
        self.assertTrue(any(any(status["status_code"] == 6 for status in item["expected_status_map"]) for item in models))

    def test_sources_are_reproducible_and_non_overwriting(self) -> None:
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            left = Path(first)
            right = Path(second)
            manifest = ROOT / "tests/fixtures/ppml_oracle_models.json"
            GENERATOR.compile_fixtures(manifest, left)
            GENERATOR.compile_fixtures(manifest, right)
            left_files = {
                path.relative_to(left): path.read_bytes()
                for path in left.rglob("*") if path.is_file()
            }
            right_files = {
                path.relative_to(right): path.read_bytes()
                for path in right.rglob("*") if path.is_file()
            }
            self.assertEqual(left_files, right_files)
            with self.assertRaisesRegex(GENERATOR.FixtureCompileError, "overwrite"):
                GENERATOR.compile_fixtures(manifest, left)

    def test_fixture_identity_seed_and_model_mutations_fail_closed(self) -> None:
        import copy
        import json
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = ROOT / "tests/fixtures/ppml_oracle_models.json"
            document = GENERATOR.compile_fixtures(manifest, root)
            for field, value in (("cell_id", "easy"), ("seed", 1), ("row_count", 1)):
                mutated = copy.deepcopy(document)
                mutated["cells"][0][field] = value
                with self.subTest(field=field), self.assertRaises(GENERATOR.FixtureCompileError):
                    GENERATOR.validate_compiled_fixtures(mutated, manifest, root)
            model_path = root / document["cells"][0]["model"]
            original = model_path.read_text(encoding="ascii")
            model_record = json.loads(original)
            model_record["weight"] = {"kind": "pweight", "column": "pw"}
            model_path.chmod(0o644)
            model_path.write_text(json.dumps(model_record), encoding="ascii")
            with self.assertRaises(GENERATOR.FixtureCompileError):
                GENERATOR.validate_compiled_fixtures(document, manifest, root)


if __name__ == "__main__":
    unittest.main()
