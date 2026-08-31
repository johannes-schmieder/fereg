from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json"
MODULE_PATH = ROOT / "scripts/ppml_memory_performance.py"
SPEC = importlib.util.spec_from_file_location("ppml_memory_performance", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
PERF = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PERF)


class PpmlMemoryPerformanceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = PERF.load_manifest(MANIFEST_PATH)
        cls.manifest_sha256 = PERF.sha256_file(MANIFEST_PATH)

    @staticmethod
    def _result(n: int, output: str) -> dict:
        probes = [1.0, 2.0, 3.0]
        outputs = {
            "prediction_probe": probes if output == "prediction" else None,
            "total_absorbed_effect_probe": (
                probes if output == "total_absorbed_effect" else None
            ),
            "individual_fe_probe": probes if output == "individual_fe" else None,
        }
        return {
            "rc": 0,
            "status": "success",
            "sample_identity_sha256": "1" * 64,
            "separation_ledger_sha256": "2" * 64,
            "coefficient_names": ["_cons", "x1"],
            "coefficient_status": ["kept", "kept"],
            "coefficients": [0.25, -0.5],
            "vce_shape": [2, 2],
            "vce": [0.04, 0.001, 0.001, 0.09],
            "dof": {
                "N_physical": n,
                "N_effective": n,
                "finite_rank": 2,
                "absorbed_rank": 100,
                "df": n - 102,
                "cluster_counts": [],
            },
            "objective": {
                "deviance": 100.0,
                "log_likelihood": -60.0,
                "null_log_likelihood": -100.0,
                "pseudo_r2": 0.4,
            },
            "outputs": outputs,
            "certificates": {
                "converged": True,
                "two_consecutive_iterations": True,
                "strict_final_solve": True,
                "rank_frozen": True,
                "score_linf": 1e-10,
                "fe_score_linf": 2e-10,
                "vce_symmetry_linf": 0.0,
                "reconstruction_linf": 3e-10,
            },
        }

    @staticmethod
    def _constrained() -> list[dict]:
        return [
            {
                "id": "low_succeeds_fast_fails",
                "attempts": [
                    {
                        "id": "low",
                        "status": "success",
                        "rc": 0,
                        "selected_route": "low",
                        "large_allocation_started": True,
                        "allocation_bytes": 100_000_000,
                    },
                    {
                        "id": "fast",
                        "status": "preflight_failure",
                        "rc": 909,
                        "selected_route": None,
                        "large_allocation_started": False,
                        "allocation_bytes": 4096,
                    },
                ],
            },
            {
                "id": "no_route_fits",
                "attempts": [
                    {
                        "id": "auto",
                        "status": "preflight_failure",
                        "rc": 909,
                        "selected_route": None,
                        "large_allocation_started": False,
                        "allocation_bytes": 4096,
                    }
                ],
            },
        ]

    def _evidence(self, manifest=None) -> dict:
        active = manifest or self.manifest
        manifest_sha = (
            self.manifest_sha256
            if active is self.manifest
            else hashlib.sha256(PERF.canonical_json_bytes(active)).hexdigest()
        )
        pins = {
            name: record["commit"]
            for name, record in active["dependencies"].items()
            if isinstance(record, dict)
        }
        cells = []
        for cell_index, plan in enumerate(PERF.expand_cells(active), start=1):
            n = plan["scale_spec"]["n"]
            data_key = json.dumps(
                [
                    plan["scale"],
                    plan["design"],
                    plan["controls"],
                    plan["outcome"],
                    plan["finite_design"],
                    plan["weight"],
                    plan["vce"],
                ],
                separators=(",", ":"),
            ).encode("ascii")
            result = self._result(n, plan["output"])
            cluster_dimensions = (
                0 if plan["vce"] == "robust" else int(plan["vce"].removeprefix("cluster"))
            )
            result["dof"]["cluster_counts"] = [100 + index for index in range(cluster_dimensions)]
            repetitions = 6
            base_pid = cell_index * 100
            cells.append(
                {
                    "cell_id": plan["cell_id"],
                    "data_sha256": hashlib.sha256(data_key).hexdigest(),
                    "scientific": {
                        "candidate": copy.deepcopy(result),
                        "comparator": copy.deepcopy(result),
                        "oracle": copy.deepcopy(result) if plan["oracle_required"] else None,
                    },
                    "timing": {
                        "candidate": {
                            "cold_seconds": 3.5,
                            "warm_seconds": [3.0] * 5,
                        },
                        "comparator": {
                            "cold_seconds": 10.5,
                            "warm_seconds": [10.0] * 5,
                        },
                    },
                    "execution": {
                        "job_id": "job-1",
                        "node_id": "node-1",
                        "candidate_process_ids": [base_pid + index for index in range(repetitions)],
                        "comparator_process_ids": [
                            base_pid + repetitions + index for index in range(repetitions)
                        ],
                        "arm_order": [
                            arm
                            for _ in range(repetitions)
                            for arm in ("candidate", "comparator")
                        ],
                        "requested_threads": plan["requested_threads"],
                        "candidate_effective_threads": plan["requested_threads"],
                        "comparator_stata_processors": min(plan["requested_threads"], 4),
                        "comparator_processor_limit": 4,
                    },
                    "diagnostics": {
                        "requested_threads": plan["requested_threads"],
                        "effective_threads": plan["requested_threads"],
                        "requested_memory": plan["memory"],
                        "effective_route": "fast" if plan["memory"] in {"fast", "auto"} else "low",
                        "fallbacks": [],
                        "outer_iterations": 8,
                        "inner_iterations": 64,
                        "step_halvings": 0,
                        "phase_seconds": {
                            phase: 0.01 for phase in active["instrumentation"]["phases"]
                        },
                    },
                    "memory": {
                        "modeled_peak_bytes": 100_000_000,
                        "native_peak_bytes": 100_000_000,
                        "scratch_peak_bytes": 0,
                        "allocator_start_bytes": 1024,
                        "allocator_current_bytes": 1024,
                        "allocator_peak_bytes": 100_001_024,
                        "process_peak_rss_bytes": 200_000_000,
                        "repeated_call_retained_bytes": 0,
                        "steady_state_data_sized_allocations": 0,
                        "workspace_capacity_before": 50_000_000,
                        "workspace_capacity_after": 50_000_000,
                        "scratch_files_after": 0,
                        "median_steady_iteration_seconds": n * 1e-8,
                    },
                    "slowdown_explanation": None,
                }
            )
        return {
            "schema_version": 1,
            "evidence_type": "ppml-memory-performance-v1-evidence",
            "manifest": {"id": active["manifest_id"], "sha256": manifest_sha},
            "identity": {
                "source_commit": "3" * 40,
                "source_tree": "4" * 40,
                "cargo_lock_sha256": "5" * 64,
                "references_lock_sha256": "6" * 64,
                "plugin_sha256": "7" * 64,
                "manifest_sha256": manifest_sha,
                "generator_sha256": "8" * 64,
                "comparator_bundle_sha256": "9" * 64,
                "os": "Linux",
                "cpu": "test-cpu",
                "stata_version": "19.0",
                "stata_flavor": "MP",
                "rustc_version": "rustc-test",
                "scheduler_job_id": "job-1",
                "node_id": "node-1",
                "reference_commits": pins,
            },
            "cells": cells,
            "constrained_memory": self._constrained(),
        }

    @staticmethod
    def _cell(evidence: dict, prefix: str, thread: int | None = None) -> dict:
        suffix = f"_t{thread}" if thread is not None else ""
        return next(
            cell
            for cell in evidence["cells"]
            if cell["cell_id"] == f"{prefix}{suffix}"
        )

    def _qualification_manifest(self) -> dict:
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "qualification_frozen"
        manifest["qualification_eligible"] = True
        PERF.validate_manifest(manifest)
        return manifest

    def test_manifest_freezes_complete_matrix_and_dependency_closure(self) -> None:
        cells = PERF.expand_cells(self.manifest)
        self.assertEqual(len(cells), 33)
        self.assertEqual(
            len([cell for cell in cells if cell["performance_role"] == "primary"]),
            20,
        )
        self.assertEqual({cell["scale_spec"]["n"] for cell in cells}, {100_000, 1_000_000, 10_000_000})
        self.assertEqual({cell["requested_threads"] for cell in cells}, {1, 2, 4, 8, 16})
        self.assertEqual({cell["memory"] for cell in cells}, {"fast", "low", "auto"})
        self.assertEqual(
            self.manifest["dependencies"]["ppmlhdfe"]["commit"],
            "b85665d8f674e93c1e07446a9a2b29be7f797910",
        )
        self.assertFalse(self.manifest["qualification_eligible"])

    def test_manifest_mutations_fail_closed(self) -> None:
        changed = copy.deepcopy(self.manifest)
        changed["performance_gates"]["maximum_equal_cell_geometric_mean_ratio"] = 0.41
        with self.assertRaisesRegex(PERF.ContractError, "performance gates"):
            PERF.validate_manifest(changed)
        changed = copy.deepcopy(self.manifest)
        changed["cases"].pop()
        with self.assertRaisesRegex(PERF.ContractError, "inventory or order"):
            PERF.validate_manifest(changed)

    def test_green_development_evidence_is_blocked_not_qualified(self) -> None:
        result = PERF.evaluate_evidence(
            self.manifest, self._evidence(), manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reasons"], ["development_manifest_not_qualification_eligible"])
        self.assertAlmostEqual(result["governing_equal_cell_geometric_mean_ratio"], 0.3)
        self.assertEqual(result["governing_twice_as_fast_share"], 1.0)

    def test_synthetic_qualification_manifest_can_pass(self) -> None:
        manifest = self._qualification_manifest()
        result = PERF.evaluate_evidence(manifest, self._evidence(manifest))
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["reasons"], [])

    def test_science_failure_preempts_malformed_timing(self) -> None:
        evidence = self._evidence()
        evidence["cells"][0]["scientific"]["candidate"]["coefficients"][0] += 0.1
        evidence["cells"][0]["timing"] = "timing must not be inspected"
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["stage"], "scientific")
        self.assertFalse(result["performance_evaluated"])
        self.assertTrue(any(reason.endswith(":coefficients") for reason in result["reasons"]))

    def test_memory_routes_must_be_scientifically_equivalent(self) -> None:
        evidence = self._evidence()
        low = self._cell(evidence, "route_n1m_two_fe_low", 8)
        low["scientific"]["candidate"]["coefficients"][0] += 1e-4
        low["scientific"]["comparator"]["coefficients"][0] += 1e-4
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(
            "memory_equivalence:n1m_standard_two_fe_routes:coefficients",
            result["reasons"],
        )

    def test_governing_geomean_share_and_single_thread_gates(self) -> None:
        evidence = self._evidence()
        primary_ids = {
            cell["cell_id"]
            for cell in PERF.expand_cells(self.manifest)
            if cell["performance_role"] == "primary"
        }
        for cell in evidence["cells"]:
            if cell["cell_id"] in primary_ids:
                cell["timing"]["candidate"]["warm_seconds"] = [5.0] * 5
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn("governing_geometric_mean_above_0.40", result["reasons"])

        evidence = self._evidence()
        non_single = [
            cell
            for cell in PERF.expand_cells(self.manifest)
            if cell["performance_role"] == "primary" and cell["requested_threads"] != 1
        ][:5]
        for plan in non_single:
            self._cell(evidence, plan["id"], plan["requested_threads"])["timing"]["candidate"]["warm_seconds"] = [6.0] * 5
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn("governing_twice_as_fast_share_below_0.80", result["reasons"])

        evidence = self._evidence()
        slow = self._cell(evidence, "gov_no_fe_easy_k0", 1)
        slow["timing"]["candidate"]["warm_seconds"] = [7.0] * 5
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(
            "gov_no_fe_easy_k0_t1:single_thread_ratio_above_two_thirds",
            result["reasons"],
        )

    def test_unexplained_slowdown_requires_registered_evidence(self) -> None:
        evidence = self._evidence()
        slow = self._cell(evidence, "gov_two_fe_standard_k10", 8)
        slow["timing"]["candidate"]["warm_seconds"] = [13.0] * 5
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn("gov_two_fe_standard_k10_t8:unexplained_slowdown", result["reasons"])
        slow["slowdown_explanation"] = {
            "ticket": "PPML-PERF-1",
            "classification": "external_noise",
            "rationale": "Independent node telemetry identifies a host event.",
            "evidence_sha256": "a" * 64,
            "accepted_by": "decision-owner",
        }
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertNotIn("gov_two_fe_standard_k10_t8:unexplained_slowdown", result["reasons"])

    def test_memory_model_leak_capacity_and_scratch_gates(self) -> None:
        mutations = {
            "native_peak_bytes": (140_000_000, "allocation_model_error"),
            "repeated_call_retained_bytes": (1, "repeated_call_leak"),
            "steady_state_data_sized_allocations": (
                1,
                "steady_state_data_sized_allocation",
            ),
            "workspace_capacity_after": (50_000_001, "workspace_capacity_growth"),
            "scratch_files_after": (1, "scratch_leak"),
        }
        for field, (value, expected) in mutations.items():
            with self.subTest(field=field):
                evidence = self._evidence()
                evidence["cells"][0]["memory"][field] = value
                result = PERF.evaluate_evidence(
                    self.manifest, evidence, manifest_sha256=self.manifest_sha256
                )
                self.assertTrue(any(reason.endswith(expected) for reason in result["reasons"]))

    def test_constrained_memory_fixtures_fail_before_large_allocation(self) -> None:
        evidence = self._evidence()
        evidence["constrained_memory"][0]["attempts"][1]["large_allocation_started"] = True
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(
            "constrained_memory:fast_did_not_fail_before_large_allocation",
            result["reasons"],
        )
        evidence = self._evidence()
        evidence["constrained_memory"][1]["attempts"][0]["rc"] = 0
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(
            "constrained_memory:no_route_did_not_fail_before_large_allocation",
            result["reasons"],
        )

    def test_saved_output_and_scaling_gates(self) -> None:
        evidence = self._evidence()
        saved = self._cell(evidence, "save_prediction_two_fe", 8)
        saved["scientific"]["candidate"]["coefficients"][0] += 1e-4
        saved["scientific"]["comparator"]["coefficients"][0] += 1e-4
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(
            "save_prediction_two_fe_t8:fit_invariance:coefficients",
            result["reasons"],
        )

        evidence = self._evidence()
        saved = self._cell(evidence, "save_prediction_two_fe", 8)
        saved["timing"]["candidate"]["warm_seconds"] = [3.2] * 5
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn("save_prediction_two_fe_t8:saved_output_overhead", result["reasons"])

        evidence = self._evidence()
        scale = self._cell(evidence, "scale_n1m_two_fe_fast", 8)
        scale["memory"]["median_steady_iteration_seconds"] *= 2.0
        result = PERF.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertTrue(any("superlinear_iteration_cost" in reason for reason in result["reasons"]))

    def test_warm_cadence_and_fresh_process_contract_are_binding(self) -> None:
        evidence = self._evidence()
        evidence["cells"][0]["timing"]["candidate"]["warm_seconds"].pop()
        with self.assertRaisesRegex(PERF.ContractError, "at least 5 warm"):
            PERF.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )
        evidence = self._evidence()
        execution = evidence["cells"][0]["execution"]
        execution["candidate_process_ids"][1] = execution["candidate_process_ids"][0]
        with self.assertRaisesRegex(PERF.ContractError, "fresh isolated processes"):
            PERF.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )

    def test_cli_plan_is_closed_and_profile_selectable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "plan.json"
            self.assertEqual(
                PERF.main(["--manifest", str(MANIFEST_PATH), "plan", "--profile", "scc_governing", "--output", str(output)]),
                0,
            )
            plan = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(len(plan), 23)
            self.assertEqual({cell["profile"] for cell in plan}, {"scc_governing"})

    def test_ppml_memory_boundary_does_not_leak_into_ols_or_legacy_driver(self) -> None:
        ols_root = ROOT / "crates/fereg-core/src/ols"
        forbidden = ("plan_ppml_memory", "PpmlIterationWorkspace", "PpmlMemoryPlan", "ppml::memory")
        for path in ols_root.glob("*.rs"):
            source = path.read_text(encoding="utf-8")
            for token in forbidden:
                self.assertNotIn(token, source, f"{path.name} imports PPML policy: {token}")
        prepared = (ROOT / "crates/fereg-core/src/ppml/prepared_irls.rs").read_text(encoding="utf-8")
        self.assertIn("fit_prepared_ppml_irls", prepared)
        self.assertNotIn("profile_irls_step", prepared)
        plugin = (ROOT / "crates/fereg-plugin/src/ppml_estimate.rs").read_text(encoding="utf-8")
        if "fit_prepared_ppml" in plugin or "prepared_irls" in plugin:
            self.assertNotIn("fit_ppml_separated(", plugin)


if __name__ == "__main__":
    unittest.main()
