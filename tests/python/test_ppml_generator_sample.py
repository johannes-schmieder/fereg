from __future__ import annotations

from collections import Counter
import copy
import csv
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("generator_calibration", ROOT / "scripts/ppml_auto_calibration.py")
assert SPEC and SPEC.loader
AUTO = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUTO)
DEV = AUTO.dev


class GeneratorContractTests(unittest.TestCase):
    def test_new_manifest_changes_only_generator_and_identity(self):
        old = DEV.read_json(ROOT / "benchmarks/manifests/feppml_solver_development_v4.json")
        new = copy.deepcopy(DEV.load_manifest())
        self.assertEqual(new.pop("schema_version"), 5)
        self.assertEqual(new.pop("manifest_id"), "feppml_solver_development_v5")
        old.pop("schema_version")
        old.pop("manifest_id")
        self.assertEqual(new["generator"].pop("mixed_graph_memberships"),
                         "degree_preserving_selected_g2_permutation_fixed_mt64_seed_104729")
        self.assertEqual(new["generator"].pop("sample_preflight"),
                         "minimum_two_observations_per_retained_fe_level")
        self.assertEqual(new, old)

    def test_old_campaign_and_development_identities_are_not_reinterpreted(self):
        old_dev = ROOT / "benchmarks/manifests/feppml_solver_development_v4.json"
        old_auto = ROOT / "benchmarks/manifests/feppml_auto_calibration_v1.json"
        self.assertEqual(DEV.sha256_file(old_dev), "56d829a489ad329946dd151f114db1664df32ce9c0ecb805796dda8d32358e40")
        self.assertEqual(DEV.sha256_file(old_auto), "4220369590d158ae04d530b40dfda9635588c58ed35fcd2d8db3853cf5bb7bc6")
        with self.assertRaises(DEV.DevelopmentError):
            DEV.load_manifest(old_dev)
        old = DEV.read_json(old_auto)
        repaired_auto = ROOT / "benchmarks/manifests/feppml_auto_calibration_v2.json"
        self.assertEqual(DEV.sha256_file(repaired_auto), "9ebd7f3507c5d19de628bfa6769571ad2145b26bb2638c3f0c943e642dd6c96a")
        new = DEV.read_json(repaired_auto)
        for key in ("schema_version", "manifest_id", "development_manifest_sha256"):
            old.pop(key)
            new.pop(key)
        self.assertEqual(new, old)

    def test_generator_contract_cannot_be_disabled(self):
        for field in ("mixed_graph_memberships", "sample_preflight"):
            value = copy.deepcopy(DEV.load_manifest())
            del value["generator"][field]
            with self.assertRaises(DEV.DevelopmentError):
                DEV.validate_manifest(value)


@unittest.skipUnless(os.environ.get("FEREG_GENERATOR_STATA"), "requires explicit licensed Stata generator check")
class StataGeneratorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.output = Path(os.environ.get("FEREG_GENERATOR_TEST_OUTPUT") or tempfile.mkdtemp(prefix="fereg-generator-"))
        cls.output.mkdir(parents=True, exist_ok=True)
        cls.cache = {}

    def generate(self, *, n=400, topology="chain_2fe", mixing=0, controls=2,
                 seed=2026090203, threads=1, difficulty="long", third_fe_columns=None):
        scope = {"n": n, "topology": topology, "mixing_percent": mixing, "controls": controls,
                 "seed": seed, "threads": threads, "difficulty": difficulty}
        if third_fe_columns is not None:
            scope["third_fe_columns"] = third_fe_columns
        key = tuple(sorted(scope.items()))
        if key in self.cache:
            return self.cache[key]
        scope["id"] = f"generator-{len(self.cache):04d}"
        directory = self.output / scope["id"]
        directory.mkdir()
        env = DEV.base_environment(ROOT, ROOT, scope, directory / "data.dta", directory / "unused.tsv")
        env.update({"FEREG_PPML_SOLVER_MODE": "generate",
                    "FEREG_PPML_SOLVER_DATA_CSV": str(directory / "data.csv"),
                    "FEREG_PPML_SOLVER_DATA_META": str(directory / "data-meta.tsv")})
        process = DEV.run_stata(os.environ["FEREG_GENERATOR_STATA"], env, directory / "generate.log")
        status, reason = DEV.stata_stage_status(directory / "generate.log", process["process_returncode"],
                                                "FEREG_PPML_SOLVER_DATA_PASS")
        DEV.exclusive_json(directory / "generation.json", {"scope": scope, "process": process,
                                                           "accepted_rc": status, "reason": reason})
        result = (directory, status)
        self.cache[key] = result
        return result

    def rows(self, **kwargs):
        directory, status = self.generate(**kwargs)
        self.assertEqual(status, 0, directory)
        with (directory / "data.csv").open() as source:
            rows = list(csv.DictReader(source))
        metadata = DEV.parse_data_meta(directory / "data-meta.tsv")
        self.assertEqual(len(rows), metadata["N"])
        self.assertIn(f"FEREG_PPML_SOLVER_SAMPLE_PASS|N={len(rows)}", DEV.process_log_text(directory / "generate.log"))
        for index, group in enumerate(("g1", "g2", "g3"), start=1):
            if group in rows[0]:
                counts = Counter(row[group] for row in rows)
                self.assertGreaterEqual(min(counts.values()), 2, directory)
                self.assertEqual(len(counts), metadata[f"levels{index}"])
        return rows

    def test_mixing_preserves_every_fe_degree_and_unselected_rows(self):
        for n in (400, 10000, 50000, 75000):
            for topology in ("chain_2fe", "sparse_3fe"):
                with self.subTest(n=n, topology=topology):
                    base = self.rows(n=n, topology=topology, controls=0)
                    for mixing in (1, 10, 100):
                        mixed = self.rows(n=n, topology=topology, controls=0, mixing=mixing)
                        self.assertEqual(len(base), len(mixed))
                        for group in ("g1", "g2", "g3"):
                            if group in base[0]:
                                self.assertEqual(Counter(r[group] for r in base), Counter(r[group] for r in mixed))
                        for i, (a, b) in enumerate(zip(base, mixed), start=1):
                            self.assertEqual(a["g1"], b["g1"])
                            self.assertEqual(a.get("g3"), b.get("g3"))
                            if i % 100 >= mixing:
                                self.assertEqual(a["g2"], b["g2"])
                        self.assertTrue(any(a["g2"] != b["g2"] for a, b in zip(base, mixed)))

    def test_mixing_keeps_control_rng_and_is_thread_independent(self):
        base = self.rows()
        mixed = self.rows(mixing=10)
        for a, b in zip(base, mixed):
            self.assertEqual([a["x1"], a["x2"]], [b["x1"], b["x2"]])
        self.assertEqual(mixed, self.rows(mixing=10, threads=7))

    def test_singleton_fixture_fails_before_publication(self):
        directory, status = self.generate(n=20)
        self.assertNotEqual(status, 0)
        self.assertFalse((directory / "data-meta.tsv").exists())
        self.assertFalse((directory / "data.dta").exists())
        self.assertNotIn("\nFEREG_PPML_SOLVER_DATA_PASS|", DEV.process_log_text(directory / "generate.log"))

    @unittest.skipUnless(os.environ.get("FEREG_GENERATOR_INVENTORY"), "requires explicit full generator inventory")
    def test_registered_local_and_scc_topologies_have_no_singletons(self):
        # Memberships precede control generation, so K=0 audits exactly the same
        # graph while avoiding large, irrelevant finite-design exports.
        seen = set()
        for stage in ("scc_train", "scc_holdout"):
            for scope in AUTO.scopes(stage):
                args = {k: scope[k] for k in ("n", "topology", "seed", "difficulty")}
                args.update(controls=0, mixing=scope.get("mixing_percent", 0),
                            third_fe_columns=scope.get("third_fe_columns"))
                key = tuple(sorted(args.items()))
                if key not in seen:
                    with self.subTest(stage=stage, scope=scope["id"]):
                        self.rows(**args)
                    seen.add(key)


if __name__ == "__main__":
    unittest.main()
