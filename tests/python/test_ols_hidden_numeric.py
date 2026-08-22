from __future__ import annotations

import copy
import hashlib
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.ols_hidden_numeric import (
    BLOCKERS,
    EXECUTABLE_CASE_IDS,
    Q_ANCHORS,
    SEALED_DEFAULT_V_ABS_CAPS,
    _assert_committed_git_blob,
    _candidate_source_identity,
    _supplied_plugin_identity,
    _validate_internal_candidate,
    _validate_plugin_identity,
    _validate_warning_audit,
    _verify_candidate_source_identity,
    _warning_audit,
    _write_exclusive_receipt,
    load_projection,
    projected_manifest,
    stata_hex_to_float,
)
from scripts.ols_reference import ContractError, seal_payload


ROOT = Path(__file__).resolve().parents[2]


def internal_q_case(case_id: str) -> dict:
    one = "+1.0000000000000X+000"
    receipt = [
        {
            "name": "stfixest_receipt",
            "row": 1,
            "column": column,
            "row_name": "r1",
            "column_name": f"c{column}",
            "value_hex": (
                "+1.46b5490000000X+005" if column == 1 else one if column == 16 else "+0.0000000000000X-3ff"
            ),
        }
        for column in range(1, 17)
    ]
    certificate_values = {
        (row, column): one
        for row in (1, 2, 3, 24, 32)
        for column in (1, 2)
    }
    certificate_values[(31, 1)] = one
    certificate_values[(31, 2)] = Q_ANCHORS[case_id]["determinant"]
    certificate = [
        {
            "name": "stfixest_certificate",
            "row": row,
            "column": column,
            "row_name": f"r{row}",
            "column_name": f"c{column}",
            "value_hex": value,
        }
        for (row, column), value in certificate_values.items()
    ]
    axis = ("x1", "_cons")
    covariance = []
    for value, (row, column) in zip(
        Q_ANCHORS[case_id]["V"], ((1, 1), (1, 2), (2, 1), (2, 2))
    ):
        covariance.append(
            {
                "name": "V",
                "row": row,
                "column": column,
                "row_name": axis[row - 1],
                "column_name": axis[column - 1],
                "value_hex": value,
            }
        )
    return {
        "id": case_id,
        "return_code": 0,
        "matrices": [
            *receipt,
            *certificate,
            {
                "name": "stfixest_status",
                "row": 1,
                "column": 1,
                "row_name": "r1",
                "column_name": "c1",
                "value_hex": one,
            },
            *covariance,
        ],
        "scalars": {"stfixest_v_determinant": Q_ANCHORS[case_id]["determinant"]},
    }


class HiddenNumericProjectionTests(unittest.TestCase):
    def test_projection_exactly_partitions_parent_and_stays_developer_only(self):
        projection = load_projection()
        self.assertEqual(tuple(projection["executable_case_ids"]), EXECUTABLE_CASE_IDS)
        self.assertEqual(
            tuple((item["case_id"], item["reason"]) for item in projection["blockers"]),
            BLOCKERS,
        )
        manifest = projected_manifest(projection)
        self.assertEqual(len(manifest["semantic_cases"]), 30)
        self.assertFalse(manifest["benchmark_cases"])
        for case in manifest["semantic_cases"]:
            self.assertTrue(case["command"].startswith("_stfixest_ols_hidden_numeric_v1 "))
            self.assertNotIn("stfeols ", case["command"])

    def test_projection_mutations_fail_closed(self):
        source = json.loads(
            (ROOT / "benchmarks/manifests/ols_hidden_numeric_v1.json").read_text(encoding="utf-8")
        )
        mutations = []
        public = copy.deepcopy(source)
        public["public_activation"] = True
        mutations.append(public)
        reordered = copy.deepcopy(source)
        reordered["executable_case_ids"][0:2] = reversed(reordered["executable_case_ids"][0:2])
        mutations.append(reordered)
        blocker = copy.deepcopy(source)
        blocker["blockers"][0]["reason"] = "changed"
        mutations.append(blocker)
        tolerance = copy.deepcopy(source)
        tolerance["tolerances"]["vce_atol"] = 1e-4
        mutations.append(tolerance)
        with tempfile.TemporaryDirectory() as raw:
            for index, mutation in enumerate(mutations):
                candidate = Path(raw) / f"projection-{index}.json"
                candidate.write_text(json.dumps(mutation), encoding="utf-8")
                with self.assertRaises(ContractError):
                    load_projection(candidate)

    def test_q2_q3_internal_anchors_are_exact_and_mutation_fatal(self):
        for case_id in ("cluster_twoway", "cluster_threeway"):
            case = internal_q_case(case_id)
            _validate_internal_candidate(case)
            mutated = copy.deepcopy(case)
            next(item for item in mutated["matrices"] if item["name"] == "V")[
                "value_hex"
            ] = "+0.0000000000000X-3ff"
            with self.assertRaisesRegex(ContractError, "exact internal covariance"):
                _validate_internal_candidate(mutated)

            determinant_mutation = copy.deepcopy(case)
            next(
                item
                for item in determinant_mutation["matrices"]
                if item["name"] == "stfixest_certificate"
                and item["row"] == 31
                and item["column"] == 2
            )["value_hex"] = "+0.0000000000000X-3ff"
            with self.assertRaisesRegex(ContractError, "not native-bound"):
                _validate_internal_candidate(determinant_mutation)

    def test_receipt_caps_are_narrow_and_hex_decoder_is_exact(self):
        self.assertEqual(
            SEALED_DEFAULT_V_ABS_CAPS,
            {
                "cluster_oneway": 1e-7,
                "cluster_twoway": 2e-6,
                "cluster_threeway": 4e-6,
                "cluster_interaction": 1e-7,
            },
        )
        self.assertEqual(stata_hex_to_float("+1.0000000000000X+000"), 1.0)
        self.assertEqual(stata_hex_to_float("-1.0000000000000X-001"), -0.5)
        with self.assertRaises(ContractError):
            stata_hex_to_float("1.0")

    def test_hidden_adapter_never_claims_public_command_identity(self):
        adapter = (ROOT / "stata/_stfixest_ols_hidden_numeric_v1.ado").read_text(
            encoding="utf-8"
        )
        self.assertIn('ereturn local cmd "_stfixest_ols_hidden_numeric_v1"', adapter)
        self.assertIn('ereturn local stfixest_intended_public_command "stfeols"', adapter)
        self.assertNotIn('ereturn local cmd "stfeols"', adapter)

    def test_supplied_plugin_identity_rejects_selection_mismatch(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            wrong = directory / "not-the-loader-name.plugin"
            wrong.write_bytes(b"plugin")
            with self.assertRaisesRegex(ContractError, "must be an existing"):
                _supplied_plugin_identity(wrong, "stfixest_macos.plugin")
            selected = directory / "stfixest_macos.plugin"
            selected.write_bytes(b"exact-plugin-bytes")
            path, identity = _supplied_plugin_identity(
                selected, "stfixest_macos.plugin"
            )
            self.assertEqual(path, selected.resolve())
            self.assertEqual(
                identity["sha256"], hashlib.sha256(b"exact-plugin-bytes").hexdigest()
            )
            self.assertEqual(identity["size_bytes"], len(b"exact-plugin-bytes"))
            identity["embedded_git_commit"] = "0123456789ab"
            _validate_plugin_identity(identity)
            for key, value in (
                ("filename", "stfixest_other.plugin"),
                ("sha256", "0" * 63),
                ("size_bytes", 0),
                ("loader_policy", "implicit-dist-search"),
                ("embedded_git_commit", "stale"),
            ):
                mutation = copy.deepcopy(identity)
                mutation[key] = value
                with self.assertRaisesRegex(ContractError, "plugin identity"):
                    _validate_plugin_identity(mutation)

    def test_source_identity_binds_complete_commit_tree_and_rejects_staleness(self):
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw)
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            source = repository / "source.rs"
            source.write_text("exact source\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repository), "add", "source.rs"], check=True)
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(repository),
                    "-c",
                    "user.name=StFixest Test",
                    "-c",
                    "user.email=stfixest@example.invalid",
                    "commit",
                    "-qm",
                    "source",
                ],
                check=True,
            )
            identity = _candidate_source_identity(repository)
            _verify_candidate_source_identity(identity, repository)
            stale_tree = copy.deepcopy(identity)
            stale_tree["tree"] = "0" * len(identity["tree"])
            with self.assertRaisesRegex(ContractError, "stale"):
                _verify_candidate_source_identity(stale_tree, repository)
            stale_commit = copy.deepcopy(identity)
            stale_commit["commit"] = "0" * len(identity["commit"])
            with self.assertRaisesRegex(ContractError, "unavailable or stale"):
                _verify_candidate_source_identity(stale_commit, repository)
            source.write_text("mutated source\n", encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "clean tracked worktree"):
                _candidate_source_identity(repository)

    def test_git_normal_mode_is_immutable_and_reseal_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw)
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            receipt = repository / "receipt.json"
            document = seal_payload({"evidence": "original"})
            receipt.write_text(json.dumps(document), encoding="utf-8")
            subprocess.run(["git", "-C", str(repository), "add", "receipt.json"], check=True)
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(repository),
                    "-c",
                    "user.name=StFixest Test",
                    "-c",
                    "user.email=stfixest@example.invalid",
                    "commit",
                    "-qm",
                    "seal",
                ],
                check=True,
            )
            receipt.chmod(0o644)
            _assert_committed_git_blob(receipt)

            seal_mutation = copy.deepcopy(document)
            seal_mutation["payload"]["evidence"] = "unsealed mutation"
            receipt.write_text(json.dumps(seal_mutation), encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "committed Git blob"):
                _assert_committed_git_blob(receipt)

            tampered = seal_payload({"evidence": "resealed mutation"})
            receipt.write_text(json.dumps(tampered), encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "committed Git blob"):
                _assert_committed_git_blob(receipt)

    def test_receipt_writer_is_exclusive_without_chmod_contract(self):
        with tempfile.TemporaryDirectory() as raw:
            receipt = Path(raw) / "receipt.json"
            _write_exclusive_receipt(receipt, "{}\n")
            self.assertTrue(os.access(receipt, os.W_OK))
            with self.assertRaisesRegex(ContractError, "refusing to overwrite"):
                _write_exclusive_receipt(receipt, '{"replacement":true}\n')

    def test_warning_absence_is_bound_to_complete_framed_transcript(self):
        audit = _warning_audit([b"stata stdout", b"clean log"])
        _validate_warning_audit(audit)
        changed = _warning_audit([b"stata stdout", b"different clean log"])
        self.assertNotEqual(audit["transcript_sha256"], changed["transcript_sha256"])
        warned = _warning_audit(
            [b"Warning: variance matrix is nonsymmetric or highly singular."]
        )
        with self.assertRaisesRegex(ContractError, "warning audit"):
            _validate_warning_audit(warned)

    def test_post_plugin_stata_boundary_has_no_observation_scale_scan(self):
        source = (ROOT / "stata/_stfixest_estimate_ols_test.ado").read_text(
            encoding="utf-8"
        )
        post_plugin = source.split(
            'plugin call __stfixest_estimate_plugin', 1
        )[1].split("program define _stfixest_eols_validate", 1)[0]
        for forbidden in ("count if", "capture assert", "summarize", "egen "):
            self.assertNotIn(forbidden, post_plugin.lower())
        native = (ROOT / "crates/stfixest-plugin/src/estimate.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("OLS shadow observation outputs failed final preflight", native)
        self.assertIn(".eq(shadow.retained_rows.iter().copied())", native)
        self.assertIn("if retained { 1.0 } else { 0.0 }", native)

    def test_hidden_adapter_uses_native_determinant_and_handles_k0(self):
        adapter = (ROOT / "stata/_stfixest_ols_hidden_numeric_v1.ado").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("det(e(V))", adapter)
        self.assertIn("e(stfixest_certificate)[31,2]", adapter)
        self.assertIn("if `coefficient_count' > 0", adapter)


if __name__ == "__main__":
    unittest.main()
