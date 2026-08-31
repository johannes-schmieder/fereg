import copy
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json"
MODULE_PATH = ROOT / "scripts/fereg_f0_regression.py"
SPEC = importlib.util.spec_from_file_location("fereg_f0_regression", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
F0 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(F0)


class F0RegressionContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = F0.load_manifest(MANIFEST)
        cls.manifest_sha256 = F0.sha256_file(MANIFEST)

    def _artifact(self, manifest=None, platform="macos-universal"):
        active = manifest or self.manifest
        record = next(
            item
            for item in active["baseline_candidate"]["artifacts"]
            if item["platform"] == platform
        )
        return {
            field: record[field]
            for field in F0.ARTIFACT_FIELDS
        }

    @staticmethod
    def _candidate_identity(platform="macos-universal"):
        commit = "1" * 40
        return {
            "source": {
                "commit": commit,
                "tree": "2" * 40,
                "cargo_lock_sha256": "3" * 64,
                "references_lock_sha256": "4" * 64,
            },
            "artifact": {
                "platform": platform,
                "sha256": "5" * 64,
                "size_bytes": 9_000_000,
                "embedded_git_commit": commit[:12],
            },
        }

    @staticmethod
    def _scientific():
        return {
            "stata_rc": 0,
            "sample_identity_sha256": "6" * 64,
            "singleton_identity_sha256": "7" * 64,
            "coefficient_names": ["x1", "x2"],
            "coefficient_status": ["kept", "kept"],
            "numeric_hex_sha256": "b" * 64,
            "coefficients": [1.25, -0.5],
            "vce_shape": [2, 2],
            "vce": [0.04, 0.001, 0.001, 0.09],
            "dof": {"N": 10_000_000, "df_m": 2, "df_r": 9_000_000},
            "fit": {"rss": 100.0, "r2": 0.75},
            "outputs": {},
            "solver": {
                "route": "map",
                "iterations": 4,
                "identity_sha256": "8" * 64,
            },
            "certificates": {
                "converged": True,
                "values": {
                    "normal_equation_linf": 1e-12,
                    "fe_projection_linf": 2e-12,
                    "vce_symmetry_linf": 0.0,
                    "reconstruction_linf": 3e-12,
                },
                "digests": {"result": "9" * 64},
            },
        }

    @staticmethod
    def _pairs(ratios=None, *, baseline_rss=1_000_000_000, candidate_rss=None):
        ratios = ratios or [1.0] * 9
        if candidate_rss is None:
            candidate_rss = baseline_rss
        orders = ["AB", "BA", "AB", "BA", "AB", "BA", "AB", "BA", "AB"]
        return [
            {
                "index": index,
                "order": orders[index - 1],
                "baseline": {"seconds": 10.0, "peak_rss_bytes": baseline_rss},
                "candidate": {
                    "seconds": 10.0 * ratios[index - 1],
                    "peak_rss_bytes": candidate_rss,
                },
            }
            for index in range(1, 10)
        ]

    @staticmethod
    def _memory(constrained=None):
        return {
            "native_allocation_bytes": 100_000_000,
            "allocation_classes": {
                "observations": 4,
                "observation_columns": 7,
                "fe_levels": 2,
                "clusters": 0,
            },
            "repeated_call_retained_bytes": 0,
            "constrained_memory": constrained,
        }

    def _evidence(self, manifest=None, profile="local_screen"):
        active = manifest or self.manifest
        manifest_hash = (
            self.manifest_sha256
            if active is self.manifest
            else hashlib.sha256(F0.canonical_json_bytes(active)).hexdigest()
        )
        platform = next(
            item["platforms"][0]
            for item in active["profiles"]
            if item["id"] == profile
        )
        cell_ids = F0.EXPECTED_CELLS_BY_PROFILE[profile]
        constrained_contract = {
            "limit_bytes": 500_000_000,
            "low": {"status": "success", "rc": 0},
            "fast": {"status": "preflight_failure", "rc": 909},
        }
        cells = []
        for cell_id in cell_ids:
            scientific = self._scientific()
            if "save_fe" in cell_id:
                scientific["outputs"] = {
                    "saved_effects_sha256": "a" * 64,
                    "saved_variables": ["__hdfe1", "__hdfe2"],
                }
            constrained = (
                constrained_contract
                if cell_id == active["memory_rules"]["constrained_cell"]
                else None
            )
            cells.append(
                {
                    "cell_id": cell_id,
                    "environment": {
                        "host_id": "same-node-cell-host",
                        "os": "macOS" if platform == "macos-universal" else "Linux",
                        "arch": "universal" if platform == "macos-universal" else "x86_64",
                        "cpu": "test-cpu",
                        "stata_version": "19.0.115",
                        "stata_flavor": "MP",
                        "toolchain": "rustc-1.85.1",
                        "physical_memory_bytes": 64_000_000_000,
                    },
                    "scientific": {
                        "baseline": copy.deepcopy(scientific),
                        "candidate": copy.deepcopy(scientific),
                    },
                    "pairs": self._pairs(),
                    "confirmation_pairs": None,
                    "memory": {
                        "baseline": self._memory(copy.deepcopy(constrained)),
                        "candidate": self._memory(copy.deepcopy(constrained)),
                    },
                }
            )
        return {
            "schema_version": 1,
            "evidence_type": "fereg-f0-regression-v1-evidence",
            "profile": profile,
            "manifest": {"id": active["manifest_id"], "sha256": manifest_hash},
            "baseline": {
                "source": copy.deepcopy(active["baseline_candidate"]["source"]),
                "artifact": self._artifact(active, platform),
            },
            "candidate": self._candidate_identity(platform),
            "cells": cells,
        }

    def _frozen_manifest(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "frozen"
        manifest["sealing"]["state"] = "sealed"
        manifest["sealing"]["missing_required_evidence"] = []
        for index, artifact in enumerate(manifest["baseline_candidate"]["artifacts"]):
            artifact["status"] = "qualified"
            if artifact["sha256"] is None:
                artifact["sha256"] = format(index + 10, "064x")
                artifact["size_bytes"] = 9_000_000 + index
                artifact["embedded_git_commit"] = manifest["baseline_candidate"]["source"]["commit"][:12]
        F0.validate_manifest(manifest)
        return manifest

    def test_manifest_binds_audited_candidate_but_remains_unsealed(self):
        baseline = self.manifest["baseline_candidate"]
        self.assertEqual(
            baseline["source"]["commit"],
            "2dee5d8c4cd99b51bbaca600348f66f325b58ab4",
        )
        self.assertEqual(
            baseline["source"]["tree"],
            "cc10b50077f162cab8be10702a9627b99ee5eea2",
        )
        macos = next(item for item in baseline["artifacts"] if item["platform"] == "macos-universal")
        self.assertEqual(
            macos["sha256"],
            "633fcd1a0b91617ea5c1aff10f17facf0990bd6ca72893cb81f0c100dfeb2298",
        )
        self.assertEqual(self.manifest["status"], "candidate_unsealed")
        self.assertTrue(self.manifest["sealing"]["missing_required_evidence"])
        local_receipt = self.manifest["sealing"]["observed_local_evidence"][0]
        self.assertEqual(
            local_receipt["receipt_sha256"],
            "5813d66caa6f6b1d1a4369e56c9d90a435f6973f2b62fcfc169b3e91ba5a4b71",
        )
        self.assertEqual(
            local_receipt["raw_receipt_sha256"],
            "4c18f1b7561aed1ac5e5488c37c4d6a6c1d981990dadbe4bc1fb68eb37d39da5",
        )
        tracked_receipt = ROOT / local_receipt["receipt_path"]
        self.assertEqual(F0.sha256_file(tracked_receipt), local_receipt["receipt_sha256"])
        receipt_payload = json.loads(tracked_receipt.read_text(encoding="utf-8"))
        self.assertEqual(receipt_payload["stata_executable"], "platform-default-stata-mp")

    def test_closed_local_and_scc_inventory(self):
        self.assertEqual(len(F0.LOCAL_SCREEN_CELL_IDS), 18)
        self.assertEqual(len(F0.SCC_GOVERNING_CELL_IDS), 14)
        mutation = copy.deepcopy(self.manifest)
        mutation["cells"][0]["id"] = "invented"
        with self.assertRaisesRegex(F0.ContractError, "inventory or order"):
            F0.validate_manifest(mutation)
        evidence = self._evidence()
        evidence["cells"].pop()
        with self.assertRaisesRegex(F0.ContractError, "inventory or order"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )

    def test_green_unsealed_evidence_is_blocked_not_falsely_qualified(self):
        result = F0.evaluate_evidence(
            self.manifest,
            self._evidence(),
            manifest_sha256=self.manifest_sha256,
        )
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reasons"], ["baseline_candidate_unsealed"])
        self.assertEqual(result["scientific_policy"], "extraction_exact")
        self.assertAlmostEqual(result["aggregate_primary_geometric_mean_ratio"], 1.0)

    def test_frozen_synthetic_manifest_can_pass(self):
        manifest = self._frozen_manifest()
        evidence = self._evidence(manifest)
        result = F0.evaluate_evidence(manifest, evidence)
        self.assertEqual(result["status"], "pass")

    def test_extraction_policy_is_exact_and_registered_fallback_requires_adr(self):
        baseline = self._scientific()
        candidate = copy.deepcopy(baseline)
        candidate["coefficients"][0] += 1e-12
        self.assertIn("coefficients", F0.compare_scientific(self.manifest, baseline, candidate))

        tolerant = copy.deepcopy(self.manifest)
        tolerant["scientific_rules"]["active_policy"] = "registered_tolerance"
        tolerant["scientific_rules"]["registered_tolerance_activation"] = {
            "authorized": True,
            "adr": "ADR-test-only",
        }
        F0.validate_manifest(tolerant)
        self.assertEqual(F0.compare_scientific(tolerant, baseline, candidate), [])
        unauthorized = copy.deepcopy(tolerant)
        unauthorized["scientific_rules"]["registered_tolerance_activation"] = {
            "authorized": False,
            "adr": None,
        }
        with self.assertRaisesRegex(F0.ContractError, "accepted ADR"):
            F0.validate_manifest(unauthorized)

    def test_scientific_failure_preempts_malformed_timing(self):
        evidence = self._evidence()
        evidence["cells"][0]["scientific"]["candidate"]["coefficients"][0] += 0.01
        evidence["cells"][0]["pairs"] = [{"not": "timing"}]
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["stage"], "scientific")
        self.assertFalse(result["performance_evaluated"])
        self.assertTrue(any(reason.endswith(":coefficients") for reason in result["reasons"]))

    def test_nine_pair_alternation_is_binding(self):
        evidence = self._evidence()
        evidence["cells"][0]["pairs"][1]["order"] = "AB"
        with self.assertRaisesRegex(F0.ContractError, "AB/BA cadence"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )
        evidence = self._evidence()
        evidence["cells"][0]["pairs"].pop()
        with self.assertRaisesRegex(F0.ContractError, "nine warm pairs"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )

    def test_student_t_log_ratio_ci_detects_reproducible_slowdown(self):
        evidence = self._evidence()
        cell = evidence["cells"][0]
        cell["pairs"] = self._pairs([1.001] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["status"], "blocked")
        self.assertIn(
            f"confirmation_required:{F0.LOCAL_SCREEN_CELL_IDS[0]}",
            result["reasons"],
        )
        metric = result["cells"][0]["initial"]
        self.assertAlmostEqual(metric["confidence_interval_ratio"][0], 1.001)
        cell["confirmation_pairs"] = self._pairs([1.001] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["status"], "fail")
        self.assertIn(
            f"confirmed_reproducible_paired_slowdown:{cell['cell_id']}",
            result["reasons"],
        )

    def test_aggregate_102_gate_is_equal_weighted_across_primary_cells(self):
        evidence = self._evidence()
        for cell in evidence["cells"]:
            if "save_fe" not in cell["cell_id"]:
                cell["pairs"] = self._pairs([1.03] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn("aggregate_geometric_mean_ratio_above_1.02", result["reasons"])
        self.assertAlmostEqual(result["aggregate_primary_geometric_mean_ratio"], 1.03)

    def test_individual_105_gate_requires_and_uses_confirmation(self):
        evidence = self._evidence()
        cell = evidence["cells"][0]
        noisy_over_105 = [0.5, 2.0, 0.5, 2.0, 0.5, 2.0, 0.5, 2.0, 2.0]
        cell["pairs"] = self._pairs(noisy_over_105)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(f"confirmation_required:{cell['cell_id']}", result["reasons"])
        cell["confirmation_pairs"] = self._pairs([1.0] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertNotIn(f"confirmation_required:{cell['cell_id']}", result["reasons"])
        self.assertEqual(result["status"], "blocked")

    def test_rss_allocation_class_allocation_bytes_and_leak_gates(self):
        evidence = self._evidence()
        cell = evidence["cells"][0]
        cell["pairs"] = self._pairs(candidate_rss=1_070_000_000)
        candidate_memory = cell["memory"]["candidate"]
        candidate_memory["allocation_classes"]["observations"] += 1
        candidate_memory["native_allocation_bytes"] += 1_048_577
        candidate_memory["repeated_call_retained_bytes"] = 1
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(f"peak_rss_regression:{cell['cell_id']}", result["reasons"])
        self.assertIn(f"data_sized_allocation_class_regression:{cell['cell_id']}", result["reasons"])
        self.assertIn(f"native_allocation_regression:{cell['cell_id']}", result["reasons"])
        self.assertIn(f"repeated_call_leak:{cell['cell_id']}", result["reasons"])

    def test_constrained_memory_requires_low_success_and_fast_r909(self):
        manifest = self._frozen_manifest()
        evidence = self._evidence(manifest, "scc_governing")
        result = F0.evaluate_evidence(manifest, evidence)
        self.assertEqual(result["status"], "pass")
        constrained = next(
            cell
            for cell in evidence["cells"]
            if cell["cell_id"] == "constrained_memory_t1_low"
        )
        constrained["memory"]["candidate"]["constrained_memory"]["fast"]["rc"] = 0
        result = F0.evaluate_evidence(manifest, evidence)
        self.assertIn(
            "constrained_memory_contract:constrained_memory_t1_low",
            result["reasons"],
        )

    def test_identity_substitution_and_manifest_tampering_are_rejected(self):
        evidence = self._evidence()
        evidence["manifest"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(F0.ContractError, "manifest identity"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )
        evidence = self._evidence()
        evidence["baseline"]["source"]["tree"] = "0" * 40
        with self.assertRaisesRegex(F0.ContractError, "baseline source"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )
        mutation = copy.deepcopy(self.manifest)
        mutation["performance_rules"]["aggregate_geometric_mean_ratio_max"] = 1.2
        with self.assertRaisesRegex(F0.ContractError, "not frozen"):
            F0.validate_manifest(mutation)

    def test_receipt_seal_and_external_identity_reject_tampering_and_resealing(self):
        evidence = self._evidence()
        document = F0.build_receipt(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "receipt.json"
            identity = Path(directory) / "identity.json"
            F0.write_receipt(receipt, document, identity_path=identity)
            verified = F0.verify_receipt(receipt, MANIFEST, identity_path=identity)
            self.assertEqual(verified["payload"]["evaluation"]["status"], "blocked")

            tampered = copy.deepcopy(document)
            tampered["payload"]["evaluation"]["status"] = "pass"
            receipt.write_bytes(F0.canonical_json_bytes(tampered))
            with self.assertRaisesRegex(F0.ContractError, "seal mismatch"):
                F0.verify_receipt(receipt, MANIFEST, identity_path=identity)

            resealed = F0.build_receipt(
                self.manifest,
                evidence,
                manifest_sha256=self.manifest_sha256,
            )
            resealed["payload"]["evidence"]["candidate"]["artifact"]["size_bytes"] += 1
            resealed = F0.seal_payload(resealed["payload"])
            receipt.write_bytes(F0.canonical_json_bytes(resealed))
            with self.assertRaises(F0.ContractError):
                F0.verify_receipt(receipt, MANIFEST, identity_path=identity)

    def test_source_identity_binds_commit_tree_and_lockfiles_and_rejects_dirty(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            subprocess.run(["git", "-C", str(repository), "config", "user.email", "test@example.com"], check=True)
            subprocess.run(["git", "-C", str(repository), "config", "user.name", "Test"], check=True)
            (repository / "Cargo.lock").write_text("cargo\n", encoding="utf-8")
            (repository / "references.lock.toml").write_text("references\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repository), "add", "Cargo.lock", "references.lock.toml"], check=True)
            subprocess.run(["git", "-C", str(repository), "commit", "-qm", "fixture"], check=True)
            identity = F0.source_identity(repository)
            self.assertEqual(F0.verify_source_identity(identity, repository), identity)
            (repository / "Cargo.lock").write_text("dirty\n", encoding="utf-8")
            with self.assertRaisesRegex(F0.ContractError, "tracked modifications"):
                F0.verify_source_identity(identity, repository)


if __name__ == "__main__":
    unittest.main()
