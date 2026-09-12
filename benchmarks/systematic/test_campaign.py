import tempfile
import unittest
from pathlib import Path

from common import HERE, arm_order, cell_id, cells, dataset_id, read_json, thread_environment
from campaign import audit, build_plan


class CampaignTests(unittest.TestCase):
    def setUp(self):
        self.manifest = read_json(HERE / "manifest.json")

    def test_complete_grid_and_intersection_deduplication(self):
        grid = cells(self.manifest)
        self.assertEqual(len(grid), 304)
        self.assertEqual(len({cell_id(c) for c in grid}), len(grid))
        for model in ("ols", "ppml"):
            for design in self.manifest["designs"]:
                main = [c for c in grid if c["model"] == model and c["design"] == design and c["section"] == "main"]
                self.assertEqual(len(main), 14)
                self.assertLessEqual(max(c["n"] for c in main), 4_000_000)

    def test_seed_identity_does_not_depend_on_threads(self):
        c = cells(self.manifest)[0]
        self.assertEqual(dataset_id(c, 104729), dataset_id(dict(c, threads=28), 104729))

    def test_candidate_order_balanced_against_every_comparator(self):
        for arms in self.manifest["arms"].values():
            for arm in arms[1:]:
                ahead = sum(arm_order(arms, r).index(arms[0]) < arm_order(arms, r).index(arm) for r in range(1, 7))
                self.assertEqual(ahead, 3)

    def test_missing_records_never_become_success(self):
        with tempfile.TemporaryDirectory() as directory:
            result = audit(Path(directory), self.manifest)
        self.assertEqual(result["status"], "incomplete")
        self.assertEqual(result["complete_groups"], 0)
        self.assertEqual(len(result["missing"]), 2128)

    def test_production_requires_pilot(self):
        with self.assertRaises(ValueError):
            build_plan(self.manifest, "production")
        with tempfile.TemporaryDirectory() as directory:
            results = Path(directory) / "results"
            results.mkdir()
            with self.assertRaisesRegex(ValueError, "frozen pilot"):
                build_plan(self.manifest, "production", results)

    def test_thread_pools_do_not_nest_blas(self):
        environment = thread_environment(28)
        self.assertEqual(environment["RAYON_NUM_THREADS"], "28")
        self.assertEqual(environment["OPENBLAS_NUM_THREADS"], "1")


if __name__ == "__main__":
    unittest.main()
