from __future__ import annotations

import copy
import hashlib
import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts.ols_reference import (
    CAPTURE_SENTINEL,
    ContractError,
    DEFAULT_LOCK,
    DEFAULT_MANIFEST,
    PreflightError,
    ReferenceIdentity,
    capture_reference,
    evaluate_benchmark_gates,
    load_manifest,
    load_reference_lock,
    parse_capture,
    preflight_reference_root,
    render_capture_do,
    _run_fresh_process,
    seal_payload,
    sha256_file,
    validate_manifest,
    validate_terminal_sentinel,
    verify_receipt,
    write_sealed_receipt,
)

ROOT = Path(__file__).resolve().parents[2]


def git(repository: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return completed.stdout.strip()


def make_reference_checkout(root: Path, name: str, required: list[str]) -> tuple[str, str]:
    checkout = root / name
    checkout.mkdir()
    git(checkout, "init", "--quiet")
    git(checkout, "config", "user.name", "StFixest Test")
    git(checkout, "config", "user.email", "stfixest-test@example.invalid")
    for relative in required:
        path = checkout / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"*! deterministic {name} test file\n", encoding="utf-8")
    git(checkout, "add", ".")
    git(checkout, "commit", "--quiet", "-m", "fixture")
    return git(checkout, "rev-parse", "HEAD"), git(checkout, "rev-parse", "HEAD^{tree}")


def reference_lock(
    reghdfe_commit: str,
    ftools_commit: str,
    require_commit: str = "3" * 40,
    *,
    root: Path | None = None,
    ftools_status: str = "resolved",
    require_status: str = "resolved",
) -> dict:
    files = {
        "reghdfe": ["src/reghdfe.ado", "src/reghdfe.mata"],
        "ftools": ["src/ftools.ado", "src/ftools.mata", "src/ftools.pkg", "src/ms_parse_vce.ado"],
        "require": ["src/require.ado", "src/require.pkg"],
    }

    def hashes(name: str) -> dict[str, str]:
        if root is None:
            return {relative: hashlib.sha256(f"{name}:{relative}".encode()).hexdigest() for relative in files[name]}
        return {relative: sha256_file(root / name / relative) for relative in files[name]}

    return {
        "schema_version": 2,
        "reference": [
            {
                "name": "reghdfe",
                "pin_status": "resolved",
                "commit": reghdfe_commit,
                "checkout_dir": "reghdfe",
                "capture_ado_subdir": "src",
                "required_capture_files": files["reghdfe"],
                "required_capture_sha256": hashes("reghdfe"),
            },
            {
                "name": "ftools",
                "pin_status": ftools_status,
                "commit": ftools_commit if ftools_status == "resolved" else "",
                "checkout_dir": "ftools",
                "capture_ado_subdir": "src",
                "required_capture_files": files["ftools"],
                "required_capture_sha256": hashes("ftools"),
            },
            {
                "name": "require",
                "pin_status": require_status,
                "commit": require_commit if require_status == "resolved" else "",
                "checkout_dir": "require",
                "capture_ado_subdir": "src",
                "required_capture_files": files["require"],
                "required_capture_sha256": hashes("require"),
            },
        ],
    }


class OlsManifestTests(unittest.TestCase):
    def test_frozen_manifest_is_complete_and_hash_bound(self):
        manifest = load_manifest()
        self.assertTrue(manifest["frozen"])
        self.assertGreaterEqual(len(manifest["semantic_cases"]), 30)
        self.assertGreaterEqual(len(manifest["benchmark_cases"]), 5)
        for case in manifest["benchmark_cases"]:
            self.assertEqual(case["repetitions"], {"cold": 1, "warm": 5})
            self.assertGreaterEqual(next(item for item in manifest["datasets"] if item["id"] == case["dataset"])["arguments"][0], 10_000_000)
            self.assertEqual(set(case["arms"]), {"comparator", "candidate"})
            self.assertEqual(set(case["capture"]["identities"]), {"sample", "singletons"})
        self.assertEqual({case["thread_count"] for case in manifest["benchmark_cases"]}, {1, 8})
        self.assertEqual(manifest["benchmark_contract"]["thresholds"]["geometric_mean_speedup_min"], 1.5)
        ts_commands = " ".join(case["command"] for case in manifest["semantic_cases"] if "models:time_series_operators" in case["covers"])
        self.assertIn("L.x", ts_commands)
        self.assertIn("F.x", ts_commands)
        self.assertIn("D.x", ts_commands)
        ts_rows = (ROOT / "tests/fixtures/ols/ols_ts.csv").read_text(encoding="utf-8")
        self.assertIn(",2003,", ts_rows)
        self.assertIn(",2007,", ts_rows)
        precedence = next(case for case in manifest["semantic_cases"] if case["id"] == "invalid_weight_outside_candidate")
        self.assertEqual(precedence["expected_sample_count"], 11)
        self.assertIn("if obs_id >= 2", precedence["command"])

    def test_stale_fixture_is_rejected(self):
        manifest = load_manifest()
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            copied = copy.deepcopy(manifest)
            specs = [
                copied["capture"]["script"],
                copied["capture"]["runner"],
                copied["capture"]["generator"],
            ]
            specs.extend(dataset["source"] for dataset in copied["datasets"])
            for spec in specs:
                source = ROOT / spec["path"]
                target = temporary_root / spec["path"]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes())
            first = temporary_root / copied["datasets"][0]["source"]["path"]
            first.write_text(first.read_text(encoding="utf-8") + "mutated\n", encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "stale or mutable input"):
                validate_manifest(copied, temporary_root)

    def test_unknown_command_control_syntax_is_rejected(self):
        manifest = load_manifest()
        manifest["semantic_cases"][0]["command"] += "; shell echo unsafe"
        with self.assertRaisesRegex(ContractError, "control syntax"):
            validate_manifest(manifest)

    def test_missing_contract_surface_is_rejected(self):
        manifest = load_manifest()
        token = manifest["semantic_cases"][0]["covers"].pop()
        for case in [*manifest["semantic_cases"], *manifest["benchmark_cases"]]:
            case["covers"] = [value for value in case["covers"] if value != token]
        with self.assertRaisesRegex(ContractError, "does not cover"):
            validate_manifest(manifest)

    def test_rendered_capture_is_reference_only_and_terminal(self):
        manifest = load_manifest()
        staged = {dataset["id"]: Path("/private/tmp") / f"{dataset['id']}.csv" for dataset in manifest["datasets"]}
        staged["__generator__"] = Path("/private/tmp/generate_panel.do")
        rendered = render_capture_do(manifest, staged, Path("/private/tmp"), "a" * 64)
        self.assertIn("reghdfe y", rendered)
        self.assertNotIn("stfeols", rendered)
        self.assertNotIn("stfepois", rendered)
        self.assertEqual(rendered.strip().splitlines()[-2], f'file write __stfx_capture "{CAPTURE_SENTINEL}" _n')


class ReferencePreflightTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.repository = self.base / "repository"
        self.repository.mkdir()
        self.reference_root = self.base / "references"
        self.reference_root.mkdir()
        reghdfe_commit, self.reghdfe_tree = make_reference_checkout(
            self.reference_root, "reghdfe", ["src/reghdfe.ado", "src/reghdfe.mata"]
        )
        ftools_commit, self.ftools_tree = make_reference_checkout(
            self.reference_root, "ftools", ["src/ftools.ado", "src/ftools.mata", "src/ftools.pkg", "src/ms_parse_vce.ado"]
        )
        require_commit, self.require_tree = make_reference_checkout(
            self.reference_root, "require", ["src/require.ado", "src/require.pkg"]
        )
        self.lock = reference_lock(reghdfe_commit, ftools_commit, require_commit, root=self.reference_root)

    def tearDown(self):
        self.temporary.cleanup()

    def test_exact_clean_isolated_checkouts_pass(self):
        identities = preflight_reference_root(self.reference_root, self.lock, self.repository)
        self.assertEqual(identities["reghdfe"].tree, self.reghdfe_tree)
        self.assertEqual(identities["ftools"].tree, self.ftools_tree)
        self.assertEqual(identities["require"].tree, self.require_tree)
        self.assertEqual(identities["reghdfe"].ado_dir, (self.reference_root / "reghdfe/src").resolve())

    def test_dirty_checkout_is_rejected(self):
        (self.reference_root / "reghdfe/src/reghdfe.ado").write_text("dirty\n", encoding="utf-8")
        with self.assertRaisesRegex(PreflightError, "mutable or dirty"):
            preflight_reference_root(self.reference_root, self.lock, self.repository)

    def test_stale_commit_is_rejected(self):
        self.lock["reference"][0]["commit"] = "0" * 40
        with self.assertRaisesRegex(PreflightError, "stale"):
            preflight_reference_root(self.reference_root, self.lock, self.repository)

    def test_symlink_checkout_is_rejected(self):
        real = self.reference_root / "reghdfe"
        moved = self.base / "real_reghdfe"
        real.rename(moved)
        real.symlink_to(moved, target_is_directory=True)
        with self.assertRaisesRegex(PreflightError, "non-symlink direct child"):
            preflight_reference_root(self.reference_root, self.lock, self.repository)

    def test_unresolved_ftools_pin_fails_before_checkout_inspection(self):
        lock = reference_lock(self.lock["reference"][0]["commit"], "", self.lock["reference"][2]["commit"], root=self.reference_root, ftools_status="unresolved")
        (self.reference_root / "reghdfe").rename(self.base / "removed_reghdfe")
        with self.assertRaisesRegex(PreflightError, "ftools exact pin is unresolved"):
            preflight_reference_root(self.reference_root, lock, self.repository)

    def test_checked_in_lock_resolves_full_source_only_dependency_closure(self):
        lock = load_reference_lock(DEFAULT_LOCK)
        ftools = next(entry for entry in lock["reference"] if entry["name"] == "ftools")
        require = next(entry for entry in lock["reference"] if entry["name"] == "require")
        self.assertEqual(ftools["commit"], "d6e24d66fd02ba1b37325c82c5040444c5b4c636")
        self.assertEqual(require["commit"], "f4760b86e55cf73b82041ce59cf20e8e8224afb0")
        self.assertNotIn("src/lftools.mlib", ftools["required_capture_files"])
        self.assertEqual(set(ftools["required_capture_files"]), set(ftools["required_capture_sha256"]))

    def test_tracked_file_hash_mismatch_is_rejected_even_at_locked_commit(self):
        self.lock["reference"][0]["required_capture_sha256"]["src/reghdfe.ado"] = "0" * 64
        with self.assertRaisesRegex(PreflightError, "hash mismatch"):
            preflight_reference_root(self.reference_root, self.lock, self.repository)


class BenchmarkGateTests(unittest.TestCase):
    @staticmethod
    def evidence(manifest: dict, *, candidate_time: float = 0.5) -> list[dict]:
        records = []
        host = {"system": "Darwin", "machine": "arm64", "cpu_count": 8}
        stata = {"version": "18", "flavor": "MP", "processors_limit": "8"}
        for case in manifest["benchmark_cases"]:
            for arm, seconds in (("comparator", 1.0), ("candidate", candidate_time)):
                scientific = {
                    "coefficient_names": ["x1"],
                    "coefficient_status": ["kept"],
                    "coefficients": [0.25],
                    "vce": {"rows": 1, "columns": 1, "values": [0.04]},
                    "dof": {"df_m": 1, "df_r": 100},
                    "fit": {"rss": 10.0, "r2": 0.5},
                    "objective": {"name": "rss", "value": 10.0},
                    "certificates": {"converged": True, "normal_equation_inf_norm": 1e-10, "fe_orthogonality_inf_norm": 1e-10, "vce_symmetry_inf_norm": 0.0},
                }
                records.append(
                    {
                        "case_id": case["id"],
                        "arm": arm,
                        "thread_count": case["thread_count"],
                        "runs": [
                            {
                                "repetition": repetition,
                                "kind": "cold" if repetition == 1 else "warm",
                                "seconds": seconds * 1.1 if repetition == 1 else seconds,
                                "peak_rss": {"value": 1000 if arm == "comparator" else 900, "unit": "bytes", "method": "os_wait4_ru_maxrss"},
                                "fresh_process": True,
                                "stata_processors": case["thread_count"],
                            }
                            for repetition in range(1, 7)
                        ],
                        "sample_identity_sha256": "a" * 64,
                        "singleton_identity_sha256": "b" * 64,
                        "host": host,
                        "stata": stata,
                        "scientific": scientific,
                    }
                )
        return records

    def test_disabled_candidate_can_never_claim_a_pass(self):
        manifest = load_manifest()
        result = evaluate_benchmark_gates(manifest, [])
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reason"], "candidate_stfeols_disabled")

    def test_future_complete_two_arm_evidence_enforces_all_thresholds(self):
        manifest = load_manifest()
        manifest["benchmark_contract"]["candidate_state"] = "enabled"
        self.assertEqual(evaluate_benchmark_gates(manifest, self.evidence(manifest))["status"], "pass")
        slower = evaluate_benchmark_gates(manifest, self.evidence(manifest, candidate_time=1.01))
        self.assertEqual(slower["status"], "fail")
        self.assertTrue(slower["reason"].startswith("candidate_slower:"))

    def test_future_gate_rejects_incomplete_or_identity_mismatched_evidence(self):
        manifest = load_manifest()
        manifest["benchmark_contract"]["candidate_state"] = "enabled"
        evidence = self.evidence(manifest)
        with self.assertRaisesRegex(ContractError, "both arms"):
            evaluate_benchmark_gates(manifest, evidence[:-1])
        evidence = self.evidence(manifest)
        evidence[1]["sample_identity_sha256"] = "c" * 64
        with self.assertRaisesRegex(ContractError, "sample or singleton"):
            evaluate_benchmark_gates(manifest, evidence)

    def test_scientific_mismatch_and_failed_certificate_block_speed_pass(self):
        manifest = load_manifest()
        manifest["benchmark_contract"]["candidate_state"] = "enabled"
        evidence = self.evidence(manifest)
        evidence[1]["scientific"]["coefficients"] = [0.5]
        result = evaluate_benchmark_gates(manifest, evidence)
        self.assertEqual(result["status"], "fail")
        self.assertIn("scientific_gate", result["reason"])
        evidence = self.evidence(manifest)
        evidence[1]["scientific"]["certificates"]["normal_equation_inf_norm"] = 1e-4
        result = evaluate_benchmark_gates(manifest, evidence)
        self.assertEqual(result["status"], "fail")
        self.assertIn("certificate_failed", result["reason"])

    def test_rss_regression_and_nonfresh_run_block_performance_pass(self):
        manifest = load_manifest()
        manifest["benchmark_contract"]["candidate_state"] = "enabled"
        evidence = self.evidence(manifest)
        for run in evidence[1]["runs"]:
            run["peak_rss"]["value"] = 1001
        result = evaluate_benchmark_gates(manifest, evidence)
        self.assertEqual(result["status"], "fail")
        self.assertIn("candidate_rss_worse", result["reason"])
        evidence = self.evidence(manifest)
        evidence[1]["runs"][0]["fresh_process"] = False
        with self.assertRaisesRegex(ContractError, "process-isolated"):
            evaluate_benchmark_gates(manifest, evidence)

    def test_fresh_process_monitor_records_method_and_bytes(self):
        if os.name != "posix" or not hasattr(os, "wait4"):
            self.skipTest("POSIX wait4 monitor required")
        completed, rss = _run_fresh_process(["/usr/bin/true"], ROOT)
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(rss["unit"], "bytes")
        self.assertEqual(rss["method"], "os_wait4_ru_maxrss")
        self.assertGreater(rss["value"], 0)


class ReceiptAndSentinelTests(unittest.TestCase):
    @staticmethod
    def minimal_capture_manifest() -> dict:
        return {
            "semantic_cases": [
                {
                    "id": "semantic",
                    "expected": "must_succeed",
                    "repetitions": {"cold": 1, "warm": 0},
                    "capture": {
                        "scalars": ["N"],
                        "macros": ["cmd"],
                        "matrices": ["b"],
                        "outputs": ["sample"],
                    },
                }
            ],
            "benchmark_cases": [],
        }

    @staticmethod
    def minimal_capture_text(*, terminal: bool = True) -> str:
        lines = [
            "STFIXEST_OLS_CAPTURE|schema|1",
            "STFIXEST_OLS_CAPTURE|manifest_sha256|" + "a" * 64,
            "STFIXEST_OLS_CAPTURE|environment|stata_version|18",
            "STFIXEST_OLS_CAPTURE|environment|stata_flavor|MP",
            "STFIXEST_OLS_CAPTURE|environment|stata_processors|8",
            "STFIXEST_OLS_CAPTURE|environment|stata_processors_limit|8",
            "STFIXEST_OLS_CAPTURE|environment|os|Unix",
            "STFIXEST_OLS_CAPTURE|environment|machine_type|Mac",
            "STFIXEST_OLS_CAPTURE|case_begin|semantic",
            "STFIXEST_OLS_CAPTURE|timing|semantic|1|cold|+0000000000000X+000",
            "STFIXEST_OLS_CAPTURE|return_code|semantic|0",
            "STFIXEST_OLS_CAPTURE|scalar|semantic|N|+0000000000001X+000",
            "STFIXEST_OLS_CAPTURE|macro|semantic|cmd|reghdfe",
            "STFIXEST_OLS_CAPTURE|matrix_dimensions|semantic|b|1|1",
            "STFIXEST_OLS_CAPTURE|matrix|semantic|b|1|1|y|x|+0000000000001X+000",
            "STFIXEST_OLS_CAPTURE|observation_count|semantic|1",
            "STFIXEST_OLS_CAPTURE|observation|semantic|+0000000000001X+000|sample|+0000000000001X+000",
            "STFIXEST_OLS_CAPTURE|case_end|semantic",
        ]
        if terminal:
            lines.append(CAPTURE_SENTINEL)
        return "\n".join(lines) + "\n"

    def test_structured_capture_requires_all_requested_fields(self):
        parsed = parse_capture(self.minimal_capture_text(), self.minimal_capture_manifest(), "a" * 64)
        self.assertEqual(parsed.cases[0]["return_code"], 0)
        missing = self.minimal_capture_text().replace(
            "STFIXEST_OLS_CAPTURE|observation|semantic|+0000000000001X+000|sample|+0000000000001X+000\n",
            "",
        )
        with self.assertRaisesRegex(ContractError, "observation evidence"):
            parse_capture(missing, self.minimal_capture_manifest(), "a" * 64)

    def test_requested_absent_and_bad_hex_are_rejected(self):
        absent = self.minimal_capture_text().replace(
            "STFIXEST_OLS_CAPTURE|scalar|semantic|N|+0000000000001X+000",
            "STFIXEST_OLS_CAPTURE|absent|semantic|scalar|N",
        )
        with self.assertRaisesRegex(ContractError, "cannot be absent"):
            parse_capture(absent, self.minimal_capture_manifest(), "a" * 64)
        malformed = self.minimal_capture_text().replace("+0000000000001X+000|sample", "not-hex|sample")
        with self.assertRaisesRegex(ContractError, "coordinate or value"):
            parse_capture(malformed, self.minimal_capture_manifest(), "a" * 64)

    def test_duplicate_observation_and_matrix_coordinates_are_rejected(self):
        duplicate_observation = self.minimal_capture_text().replace(
            "STFIXEST_OLS_CAPTURE|case_end|semantic",
            "STFIXEST_OLS_CAPTURE|observation|semantic|+0000000000001X+000|sample|+0000000000001X+000\nSTFIXEST_OLS_CAPTURE|case_end|semantic",
        )
        with self.assertRaisesRegex(ContractError, "observation evidence is incomplete|duplicate observation"):
            parse_capture(duplicate_observation, self.minimal_capture_manifest(), "a" * 64)
        duplicate_matrix = self.minimal_capture_text().replace(
            "STFIXEST_OLS_CAPTURE|observation_count",
            "STFIXEST_OLS_CAPTURE|matrix|semantic|b|1|1|y|x|+0000000000001X+000\nSTFIXEST_OLS_CAPTURE|observation_count",
        )
        with self.assertRaisesRegex(ContractError, "duplicate matrix coordinate"):
            parse_capture(duplicate_matrix, self.minimal_capture_manifest(), "a" * 64)
        incomplete_declared_shape = self.minimal_capture_text().replace(
            "matrix_dimensions|semantic|b|1|1", "matrix_dimensions|semantic|b|1|2"
        )
        with self.assertRaisesRegex(ContractError, "declared dimensions"):
            parse_capture(incomplete_declared_shape, self.minimal_capture_manifest(), "a" * 64)

    def test_terminal_sentinel_must_be_unique_successful_and_last(self):
        validate_terminal_sentinel("header\n" + CAPTURE_SENTINEL + "\n")
        for text in (
            "header\n",
            CAPTURE_SENTINEL + "\ntrailing\n",
            CAPTURE_SENTINEL + "\n" + CAPTURE_SENTINEL + "\n",
            "END_FIXTURE|ols_v1_reference_capture|rc=9\n",
        ):
            with self.subTest(text=text), self.assertRaises(ContractError):
                validate_terminal_sentinel(text)

    def test_redaction_rejects_local_paths_and_license_identity(self):
        with self.assertRaisesRegex(ContractError, "absolute local path"):
            seal_payload({"note": "/Users/alice/private/reference.log"})
        with self.assertRaisesRegex(ContractError, "licensed-user text"):
            seal_payload({"note": "Licensed to Alice Example"})
        with self.assertRaisesRegex(ContractError, "sensitive field"):
            seal_payload({"raw_log": "not allowed even when sanitized"})

    def test_receipt_is_exclusive_and_read_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            write_sealed_receipt(path, {"safe": "value"})
            self.assertEqual(path.stat().st_mode & 0o222, 0)
            with self.assertRaisesRegex(ContractError, "refusing to overwrite"):
                write_sealed_receipt(path, {"safe": "replacement"})

    def test_verifier_rejects_mutation_staleness_and_writable_receipt(self):
        manifest = {
            "manifest_id": "ols_v1",
            "benchmark_contract": {
                "candidate_state": "disabled_fail_closed",
                "thresholds": {"geometric_mean_speedup_min": 1.5, "primary_case_time_ratio_max": 1.0, "primary_case_rss_ratio_max": 1.0},
            },
            "datasets": [],
            "semantic_cases": [
                {
                    "id": "semantic",
                    "expected": "must_succeed",
                    "repetitions": {"cold": 1, "warm": 0},
                    "capture": {"scalars": [], "macros": [], "matrices": [], "outputs": []},
                }
            ],
            "benchmark_cases": [],
        }
        lock = reference_lock("1" * 40, "2" * 40, "3" * 40)
        payload = {
            "schema_version": 1,
            "receipt_type": "stfixest-ols-v1-reference",
            "manifest": {"id": "ols_v1", "sha256": ""},
            "inputs": {"fixture.csv": "3" * 64},
            "generated_datasets": {},
            "references": {
                "reghdfe": {"commit": "1" * 40, "git_tree": "4" * 40},
                "ftools": {"commit": "2" * 40, "git_tree": "5" * 40},
                "require": {"commit": "3" * 40, "git_tree": "6" * 40},
            },
            "environment": {"stata_version": "18", "stata_flavor": "MP", "stata_processors": "8", "stata_processors_limit": "8", "os": "Unix", "machine_type": "Mac"},
            "host": {"system": "Darwin", "machine": "arm64", "cpu_count": 8},
            "benchmark_evidence": [],
            "benchmark_gate": {
                "status": "blocked",
                "reason": "candidate_stfeols_disabled",
                "thresholds": {"geometric_mean_speedup_min": 1.5, "primary_case_time_ratio_max": 1.0, "primary_case_rss_ratio_max": 1.0},
            },
            "cases": [
                {
                    "id": case_id,
                    "return_code": 0,
                    "runtime_thread_count": None,
                    "timings": [{"repetition": 1, "kind": "cold", "seconds_hex": "+0X+000"}],
                    "scalars": {},
                    "macros": {},
                    "matrices": [],
                    "matrix_dimensions": {},
                    "observation_count": None,
                    "observations": [],
                    "identities": {},
                    "postestimation": [],
                    "absent": [],
                }
                for case_id in ("semantic",)
            ],
        }
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = Path(temporary) / "manifest.json"
            manifest_path.write_text("frozen-manifest\n", encoding="utf-8")
            payload["manifest"]["sha256"] = sha256_file(manifest_path)
            receipt = Path(temporary) / "receipt.json"
            write_sealed_receipt(receipt, payload)
            with mock.patch("scripts.ols_reference.load_manifest", return_value=manifest), mock.patch(
                "scripts.ols_reference.manifest_bound_inputs", return_value=payload["inputs"]
            ), mock.patch("scripts.ols_reference.load_reference_lock", return_value=lock):
                verify_receipt(receipt, manifest_path, Path("unused"), Path(temporary))
                receipt.chmod(stat.S_IRUSR | stat.S_IWUSR)
                with self.assertRaisesRegex(ContractError, "writable"):
                    verify_receipt(receipt, manifest_path, Path("unused"), Path(temporary))
                receipt.chmod(stat.S_IRUSR)
                document = json.loads(receipt.read_text(encoding="utf-8"))
                document["payload"]["cases"][0]["id"] = "tampered"
                receipt.chmod(stat.S_IRUSR | stat.S_IWUSR)
                receipt.write_text(json.dumps(document), encoding="utf-8")
                receipt.chmod(stat.S_IRUSR)
                with self.assertRaisesRegex(ContractError, "seal mismatch"):
                    verify_receipt(receipt, manifest_path, Path("unused"), Path(temporary))

    def test_capture_never_launches_stata_while_transitive_pin_is_unresolved(self):
        with tempfile.TemporaryDirectory() as temporary:
            reference_root = Path(temporary).resolve()
            runner = mock.Mock(side_effect=AssertionError("Stata must not start"))
            unresolved = reference_lock("1" * 40, "2" * 40, require_status="unresolved")
            with mock.patch("scripts.ols_reference.load_reference_lock", return_value=unresolved):
                with self.assertRaisesRegex(PreflightError, "require exact pin is unresolved"):
                    capture_reference(
                        "stata",
                        reference_root,
                        reference_root / "receipt.json",
                        runner=runner,
                    )
            runner.assert_not_called()


if __name__ == "__main__":
    unittest.main()
