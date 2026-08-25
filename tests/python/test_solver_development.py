from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest

from scripts.ols_reference import ContractError, load_manifest
from scripts.solver_development import expand_run_plan, load_development_manifest


class SolverDevelopmentManifestTests(unittest.TestCase):
    def test_short_matrix_is_complete_and_development_only(self) -> None:
        development = load_development_manifest()
        release = load_manifest()
        plan = expand_run_plan(development, release)
        self.assertEqual(len(plan), 104)
        self.assertTrue(all(run["repetitions"] == {"cold": 1, "warm": 2} for run in plan))
        self.assertEqual(
            {run["family"] for run in plan},
            {"dense_two_fe", "sparse_two_fe", "absorbed_slope", "clustered_vce", "moderate_k"},
        )
        auto = [run for run in plan if run["arm"] == "auto"]
        self.assertEqual(len(auto), 10)
        self.assertTrue(all("solver(auto)" in run["command"] for run in auto))
        self.assertTrue(all("deterministic" not in run["command"] for run in auto))
        self.assertEqual(len([run for run in plan if run["arm"] == "gregress"]), 4)

    def test_missing_solver_or_family_binding_fails_closed(self) -> None:
        development = load_development_manifest()
        missing_solver = copy.deepcopy(development)
        missing_solver["candidate_arms"].pop()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "development.json"
            path.write_text(json.dumps(missing_solver), encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "candidate inventory"):
                load_development_manifest(path)

            stale_family = copy.deepcopy(development)
            stale_family["families"][0]["ols_v1_cases"][0] = "missing_case"
            path.write_text(json.dumps(stale_family), encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "stale or duplicated"):
                load_development_manifest(path)


if __name__ == "__main__":
    unittest.main()
