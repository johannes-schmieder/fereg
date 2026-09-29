from __future__ import annotations

import copy
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts/feppml_qualification.py"
PLATFORM_PATH = ROOT / "scripts/feppml_platform_load_receipt.py"
MANIFEST_PATH = ROOT / "benchmarks/manifests/feppml_qualification_v1.json"
SPEC = importlib.util.spec_from_file_location("feppml_qualification", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
QUAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUAL)
PLATFORM_SPEC = importlib.util.spec_from_file_location(
    "feppml_platform_load_receipt_qualification_test", PLATFORM_PATH
)
assert PLATFORM_SPEC is not None and PLATFORM_SPEC.loader is not None
PLATFORM = importlib.util.module_from_spec(PLATFORM_SPEC)
PLATFORM_SPEC.loader.exec_module(PLATFORM)


def initialize_source_repository(repo: Path, files: dict[str, str]) -> None:
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(
        ["git", "-C", str(repo), "config", "user.email", "test@example.invalid"],
        check=True,
    )
    subprocess.run(
        ["git", "-C", str(repo), "config", "user.name", "Qualification Test"],
        check=True,
    )
    defaults = {
        path.relative_to(ROOT).as_posix(): path.read_text(encoding="utf-8")
        for path in sorted((ROOT / "crates").glob("*/src/**/*.rs"))
    }
    defaults.update(files)
    for relative, content in defaults.items():
        path = repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def commit_source_repository(repo: Path, message: str) -> str:
    subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
    subprocess.run(["git", "-C", str(repo), "commit", "-qm", message], check=True)
    return subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()


class FakeDevelopment:
    def __init__(self, qualification_manifest: dict) -> None:
        self.qualification_manifest = qualification_manifest

    def expand_cells(self, _manifest: dict) -> list[dict]:
        oracle = set(
            self.qualification_manifest["matrix_contract"]["oracle_required_cell_ids"]
        )
        cells = []
        for index, cell_id in enumerate(
            self.qualification_manifest["matrix_contract"]["expected_cell_ids"]
        ):
            cells.append(
                {
                    "cell_id": cell_id,
                    "oracle_required": cell_id in oracle,
                    "performance_role": "primary" if 10 <= index < 30 else (
                        "saved_output" if index >= 30 else "diagnostic"
                    ),
                    "requested_threads": int(cell_id.rsplit("_t", 1)[1]),
                }
            )
        return cells

    def evaluate_evidence(
        self, _manifest: dict, evidence: dict, *, manifest_sha256: str
    ) -> dict:
        del manifest_sha256
        if evidence.get("force_science_failure"):
            return {
                "status": "fail",
                "stage": "scientific",
                "performance_evaluated": False,
                "reasons": ["cell:oracle:coefficients"],
                "cells": [],
            }
        return {
            "status": "blocked",
            "stage": "complete",
            "performance_evaluated": True,
            "reasons": ["development_manifest_not_qualification_eligible"],
            "cells": [
                {
                    "cell_id": item["cell_id"],
                    "performance_role": item["performance_role"],
                    "requested_threads": item["requested_threads"],
                }
                for item in self.expand_cells({})
            ],
        }


class FeppmlQualificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source_commit = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
        cls.source_tree = subprocess.check_output(
            ["git", "rev-parse", "HEAD^{tree}"], cwd=ROOT, text=True
        ).strip()
        cls.build_date_utc = datetime.fromtimestamp(
            int(
                subprocess.check_output(
                    ["git", "show", "-s", "--format=%ct", cls.source_commit],
                    cwd=ROOT,
                    text=True,
                ).strip()
            ),
            timezone.utc,
        ).date().isoformat()
        # The v1 qualification manifest is immutable historical evidence. Its
        # frozen file hashes intentionally do not describe the live prototype.
        cls.manifest = QUAL.load_manifest(MANIFEST_PATH, verify_files=False)
        cls.manifest_sha = QUAL.sha256_file(MANIFEST_PATH)
        cls.platform_records = []
        for platform in cls.manifest["platform_gates"]["required_artifacts"]:
            program_sha = cls.manifest["platform_gates"]["capture_program"][
                "sha256"
            ]

            def file_record(name: str, sha256: str, size: int) -> dict:
                return {
                    "path": f"/sealed/{platform}/{name}",
                    "sha256": sha256,
                    "size_bytes": size,
                }

            receipt = file_record("receipt.json", "6" * 64, 100)
            receipt["payload_sha256"] = "9" * 64
            cls.platform_records.append(
                {
                    "record_type": PLATFORM.RECORD_TYPE,
                    "qualification_profile": "candidate",
                    "platform": platform,
                    "source_commit": cls.source_commit,
                    "source_tree": cls.source_tree,
                    "source_archive_sha256": "4" * 64,
                    "source_archive_size_bytes": 1000,
                    "embedded_git_commit": cls.source_commit[:12],
                    "build_date_utc": cls.build_date_utc,
                    "controller_commit": cls.source_commit,
                    "controller_tree": cls.source_tree,
                    "artifact": file_record("fereg.plugin", "7" * 64, 500),
                    "capture": file_record("capture.json", "a" * 64, 200),
                    "source_archive": file_record(
                        "candidate.tar", "4" * 64, 1000
                    ),
                    "capture_program": file_record(
                        "capture.do", program_sha, 300
                    ),
                    "receipt": receipt,
                    "platform_provenance": {
                        "record_type": "qualification-test-platform-provenance-v1",
                        "harness_commit": cls.source_commit,
                        "harness_tree": cls.source_tree,
                        "controller_commit": cls.source_commit,
                        "controller_tree": cls.source_tree,
                    },
                    "load_status": "success",
                    "licensed_stata_load": True,
                    "mock_only": False,
                    "scientific_claim": False,
                    "receipt_scope": "artifact_and_licensed_stata_load_only",
                }
            )

    def setUp(self) -> None:
        verifier = mock.patch(
            "scripts.feppml_platform_load_receipt.verify_evidence",
            side_effect=self._authoritative_platform_payload,
        )
        verifier.start()
        self.addCleanup(verifier.stop)

    @staticmethod
    def _authoritative_platform_payload(record: dict) -> dict:
        if set(record) != PLATFORM.RECORD_FIELDS:
            raise PLATFORM.PlatformReceiptError(
                "qualification test platform record is not closed"
            )
        return {
            "schema_version": 1,
            "kind": "feppml_platform_licensed_stata_load",
            "qualification_profile": record["qualification_profile"],
            "platform": record["platform"],
            "artifact_sha256": record["artifact"]["sha256"],
            "artifact_size_bytes": record["artifact"]["size_bytes"],
            "stata_version": "19.0",
            "stata_flavor": "MP",
            "capture_program_sha256": record["capture_program"]["sha256"],
            "capture_output_sha256": record["capture"]["sha256"],
            "source_commit": record["source_commit"],
            "source_tree": record["source_tree"],
            "source_archive_sha256": record["source_archive_sha256"],
            "source_archive_size_bytes": record["source_archive_size_bytes"],
            "embedded_git_commit": record["embedded_git_commit"],
            "build_date_utc": record["build_date_utc"],
            "controller_commit": record["controller_commit"],
            "controller_tree": record["controller_tree"],
            "platform_provenance": copy.deepcopy(record["platform_provenance"]),
            "licensed_stata_load": record["licensed_stata_load"],
            "mock_only": record["mock_only"],
            "scientific_claim": record["scientific_claim"],
            "receipt_scope": record["receipt_scope"],
            "complete_sentinel": "FEPPML_PLATFORM_LOAD_RECEIPT_COMPLETE",
        }

    def _measurement(
        self, *, seconds: float, process_id: int, artifact: str
    ) -> dict:
        return {
            "seconds": seconds,
            "peak_rss_bytes": 10_000_000,
            "process_id": process_id,
            "node_id": "qualification-node",
            "stata_version": "19.0 MP",
            "artifact_sha256": artifact,
            "fresh_process": True,
        }

    def _evidence(self) -> dict:
        candidate_artifact = "7" * 64
        comparator_artifact = "8" * 64
        oracle_dimensions = {
            cell_id: dimension
            for cell_id, dimension in zip(
                self.manifest["matrix_contract"]["oracle_required_cell_ids"],
                (0, 1, 2, 3, 4, 5, 0),
            )
        }
        benchmark_cells = []
        paired_cells = []
        order = self.manifest["capture_contract"]["warm_pair_order"]
        for cell_index, cell_id in enumerate(
            self.manifest["matrix_contract"]["expected_cell_ids"]
        ):
            base_pid = 10_000 + 100 * cell_index
            cold = {
                "index": 0,
                "order": "AB",
                "candidate": self._measurement(
                    seconds=1.0, process_id=base_pid, artifact=candidate_artifact
                ),
                "comparator": self._measurement(
                    seconds=4.0,
                    process_id=base_pid + 1,
                    artifact=comparator_artifact,
                ),
            }
            warm = []
            for index, pair_order in enumerate(order, 1):
                warm.append(
                    {
                        "index": index,
                        "order": pair_order,
                        "candidate": self._measurement(
                            seconds=1.0,
                            process_id=base_pid + 2 * index,
                            artifact=candidate_artifact,
                        ),
                        "comparator": self._measurement(
                            seconds=4.0,
                            process_id=base_pid + 2 * index + 1,
                            artifact=comparator_artifact,
                        ),
                    }
                )
            paired_cells.append(
                {"cell_id": cell_id, "cold_pair": cold, "warm_pairs": warm}
            )
            arm_order = []
            for pair in (cold, *warm):
                arm_order.append(
                    ["candidate", "comparator"]
                    if pair["order"] == "AB"
                    else ["comparator", "candidate"]
                )
            benchmark_cells.append(
                {
                    "cell_id": cell_id,
                    "timing": {
                        "candidate": {
                            "cold_seconds": cold["candidate"]["seconds"],
                            "warm_seconds": [pair["candidate"]["seconds"] for pair in warm],
                        },
                        "comparator": {
                            "cold_seconds": cold["comparator"]["seconds"],
                            "warm_seconds": [pair["comparator"]["seconds"] for pair in warm],
                        },
                    },
                    "execution": {
                        "scheduler_job_id": "7337000",
                        "node_id": "qualification-node",
                        "plugin_sha256": candidate_artifact,
                        "comparator_bundle_sha256": comparator_artifact,
                        "os": "Linux 5.14",
                        "cpu": "Intel Xeon Gold",
                        "stata_version": "19.0 MP",
                        "stata_flavor": "MP",
                        "rustc_version": "rustc 1.89.0",
                        "candidate_process_ids": [
                            cold["candidate"]["process_id"],
                            *[pair["candidate"]["process_id"] for pair in warm],
                        ],
                        "comparator_process_ids": [
                            cold["comparator"]["process_id"],
                            *[pair["comparator"]["process_id"] for pair in warm],
                        ],
                        "arm_order": arm_order,
                    },
                    "memory": {"process_peak_rss_bytes": 10_000_000},
                }
            )
            if cell_id in oracle_dimensions:
                benchmark_cells[-1]["scientific"] = {
                    "oracle": {
                        "schema_version": 1,
                        "check_type": "ppml-explicit-dummy-oracle-v1",
                        "cell_id": cell_id,
                        "independent": {
                            "dof": {
                                "cluster_counts": [6] * oracle_dimensions[cell_id]
                            }
                        },
                        "complete_sentinel": "FEPPML_ORACLE_CHECK_COMPLETE",
                    }
                }
        references = {
            name: {
                "commit": commit,
                "tree": "2" * 40,
                "archive_sha256": "3" * 64,
                "archive_size_bytes": 1024,
            }
            for name, commit in QUAL._expected_reference_commits().items()
        }
        evidence = {
            "schema_version": 1,
            "evidence_type": "feppml-qualification-v1-evidence",
            "qualification_manifest": {
                "id": self.manifest["manifest_id"],
                "sha256": self.manifest_sha,
            },
            "candidate": {
                "source_commit": self.source_commit,
                "source_tree": self.source_tree,
                "cargo_lock_sha256": "3" * 64,
                "references_lock_sha256": self.manifest["reference_closure"][
                    "references_lock_sha256"
                ],
                "source_archive_sha256": "4" * 64,
                "source_archive_size_bytes": 1000,
                "package_sha256": "5" * 64,
                "performance_plugin_sha256": candidate_artifact,
                "performance_plugin_size_bytes": 500,
                "embedded_git_commit": self.source_commit[:12],
                "build_date_utc": self.build_date_utc,
            },
            "comparator": {
                "bundle_sha256": comparator_artifact,
                "bundle_size_bytes": 2000,
                "references": references,
            },
            "oracles": [],
            "source_audit": {
                "source_commit": self.source_commit,
                "path": self.manifest["source_gate"]["production_path"],
                "sha256": "9" * 64,
                "forbidden_symbols_absent": True,
                "forbidden_symbols_found": [],
            },
            "platform_artifacts": copy.deepcopy(self.platform_records),
            "paired_cells": paired_cells,
            "benchmark_evidence": {
                "identity": {
                    "source_commit": self.source_commit,
                    "source_tree": self.source_tree,
                    "cargo_lock_sha256": "3" * 64,
                    "references_lock_sha256": self.manifest["reference_closure"][
                        "references_lock_sha256"
                    ],
                    "manifest_sha256": "c" * 64,
                    "generator_sha256": "d" * 64,
                    "reference_commits": {
                        name: record["commit"] for name, record in references.items()
                    },
                },
                "cells": benchmark_cells,
            },
            "complete_sentinel": "FEPPML_QUALIFICATION_COMPLETE",
        }
        evidence["oracles"] = QUAL.oracle_evidence_records(
            self.manifest, evidence["benchmark_evidence"]
        )
        return evidence

    def _fake_load(self, *_args, **_kwargs):
        return FakeDevelopment(self.manifest), {}, "c" * 64

    def test_manifest_freezes_exact_sources_matrix_and_capture_cadence(self) -> None:
        self.assertTrue(self.manifest["qualification_eligible"])
        self.assertEqual(
            self.manifest["reference_closure"]["ppmlhdfe"]["commit"],
            "b85665d8f674e93c1e07446a9a2b29be7f797910",
        )
        self.assertEqual(len(self.manifest["matrix_contract"]["expected_cell_ids"]), 33)
        self.assertEqual(
            self.manifest["capture_contract"]["warm_pair_order"],
            ["AB", "BA", "AB", "BA", "AB", "BA", "AB", "BA", "AB"],
        )
        self.assertEqual(
            self.manifest["source_gate"]["forbidden_route_symbols"],
            ["fit_ppml_separated", "profile_irls_step"],
        )
        capture_program = self.manifest["platform_gates"]["capture_program"]
        self.assertEqual(
            capture_program["path"],
            "tests/stata/installation/feppml_platform_load_capture.do",
        )
        self.assertEqual(
            capture_program["sha256"], QUAL.sha256_file(ROOT / capture_program["path"])
        )

    def test_manifest_mutations_fail_closed(self) -> None:
        changed = copy.deepcopy(self.manifest)
        changed["performance_gates"]["maximum_equal_cell_geometric_mean_ratio"] = 0.41
        with self.assertRaisesRegex(QUAL.QualificationError, "performance gate"):
            QUAL.validate_manifest(changed, root=None)
        changed = copy.deepcopy(self.manifest)
        changed["capture_contract"]["warm_pair_order"][0] = "BA"
        with self.assertRaisesRegex(QUAL.QualificationError, "cadence"):
            QUAL.validate_manifest(changed, root=None)
        changed = copy.deepcopy(self.manifest)
        changed["matrix_contract"]["expected_cell_ids"].pop()
        with self.assertRaisesRegex(QUAL.QualificationError, "inventory"):
            QUAL.validate_manifest(changed, root=None)

    def test_green_exact_evidence_passes_and_reports_paired_metrics(self) -> None:
        evidence = self._evidence()
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            result = QUAL.evaluate_evidence(
                self.manifest,
                evidence,
                manifest_sha256=self.manifest_sha,
                manifest_root=None,
            )
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["stage"], "complete")
        self.assertTrue(result["performance_evaluated"])
        self.assertAlmostEqual(
            result["paired_governing_equal_cell_geometric_mean_ratio"], 0.25
        )
        self.assertEqual(result["paired_governing_twice_as_fast_share"], 1.0)

    def test_science_failure_preempts_even_malformed_pair_timing(self) -> None:
        evidence = self._evidence()
        evidence["benchmark_evidence"] = {"force_science_failure": True}
        evidence["paired_cells"] = "timing must not be inspected"
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            result = QUAL.evaluate_evidence(
                self.manifest,
                evidence,
                manifest_sha256=self.manifest_sha,
                manifest_root=None,
            )
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["stage"], "scientific")
        self.assertFalse(result["performance_evaluated"])

    def test_pair_process_artifact_cadence_and_benchmark_binding_fail_closed(self) -> None:
        mutations = []
        evidence = self._evidence()
        evidence["paired_cells"][0]["warm_pairs"].pop()
        mutations.append((evidence, "nine warm"))
        evidence = self._evidence()
        evidence["paired_cells"][0]["warm_pairs"][0]["order"] = "BA"
        mutations.append((evidence, "cadence"))
        evidence = self._evidence()
        reused = evidence["paired_cells"][0]["cold_pair"]["candidate"]["process_id"]
        evidence["paired_cells"][0]["warm_pairs"][0]["candidate"]["process_id"] = reused
        mutations.append((evidence, "reused a process"))
        evidence = self._evidence()
        evidence["paired_cells"][0]["cold_pair"]["candidate"]["artifact_sha256"] = "f" * 64
        mutations.append((evidence, "wrong arm artifact"))
        evidence = self._evidence()
        evidence["benchmark_evidence"]["cells"][0]["timing"]["candidate"]["cold_seconds"] = 9.0
        mutations.append((evidence, "differs from benchmark"))
        evidence = self._evidence()
        evidence["benchmark_evidence"]["cells"][0]["execution"]["plugin_sha256"] = "f" * 64
        mutations.append((evidence, "environment/artifacts"))
        for evidence, message in mutations:
            with self.subTest(message=message), mock.patch.object(
                QUAL, "_load_development", self._fake_load
            ):
                with self.assertRaisesRegex(QUAL.QualificationError, message):
                    QUAL.evaluate_evidence(
                        self.manifest,
                        evidence,
                        manifest_sha256=self.manifest_sha,
                        manifest_root=None,
                    )

    def test_mock_only_platform_receipt_cannot_claim_licensed_load_or_science(self) -> None:
        for field, value in (
            ("mock_only", True),
            ("licensed_stata_load", False),
            ("scientific_claim", True),
        ):
            evidence = self._evidence()
            evidence["platform_artifacts"][2][field] = value
            with self.subTest(field=field), mock.patch.object(
                QUAL, "_load_development", self._fake_load
            ):
                with self.assertRaisesRegex(
                    QUAL.QualificationError, "qualification eligible"
                ):
                    QUAL.evaluate_evidence(
                        self.manifest,
                        evidence,
                        manifest_sha256=self.manifest_sha,
                        manifest_root=None,
                    )

    def test_stale_platform_source_cannot_match_candidate(self) -> None:
        evidence = self._evidence()
        evidence["candidate"]["source_commit"] = "a" * 40
        evidence["candidate"]["source_tree"] = "b" * 40
        evidence["candidate"]["embedded_git_commit"] = "a" * 12
        evidence["source_audit"]["source_commit"] = "a" * 40
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            with self.assertRaisesRegex(QUAL.QualificationError, "stale relative"):
                QUAL.evaluate_evidence(
                    self.manifest,
                    evidence,
                    manifest_sha256=self.manifest_sha,
                    manifest_root=None,
                )

    def test_platform_capture_program_is_the_frozen_controller(self) -> None:
        evidence = self._evidence()
        evidence["platform_artifacts"][0]["capture_program"]["sha256"] = "e" * 64
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            with self.assertRaisesRegex(
                QUAL.QualificationError, "qualification eligible"
            ):
                QUAL.evaluate_evidence(
                    self.manifest,
                    evidence,
                    manifest_sha256=self.manifest_sha,
                    manifest_root=None,
                )

    def test_candidate_and_platform_build_date_are_exact(self) -> None:
        evidence = self._evidence()
        evidence["candidate"]["build_date_utc"] = "host-clock"
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            with self.assertRaisesRegex(
                QUAL.QualificationError, "deterministic UTC build date"
            ):
                QUAL.evaluate_evidence(
                    self.manifest,
                    evidence,
                    manifest_sha256=self.manifest_sha,
                    manifest_root=None,
                )

        evidence = self._evidence()
        evidence["candidate"]["build_date_utc"] = "2000-01-01"
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            with self.assertRaisesRegex(QUAL.QualificationError, "stale relative"):
                QUAL.evaluate_evidence(
                    self.manifest,
                    evidence,
                    manifest_sha256=self.manifest_sha,
                    manifest_root=None,
                )

    def test_oracle_receipt_digest_is_derived_from_actual_registered_bundles(self) -> None:
        evidence = self._evidence()
        evidence["oracles"][0]["evidence_sha256"] = "6" * 64
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            with self.assertRaisesRegex(
                QUAL.QualificationError, "registered cell bundles"
            ):
                QUAL.evaluate_evidence(
                    self.manifest,
                    evidence,
                    manifest_sha256=self.manifest_sha,
                    manifest_root=None,
                )

        evidence = self._evidence()
        oracle_cell = self.manifest["matrix_contract"]["oracle_required_cell_ids"][1]
        cell = next(
            item
            for item in evidence["benchmark_evidence"]["cells"]
            if item["cell_id"] == oracle_cell
        )
        cell["scientific"]["oracle"]["independent"]["dof"]["cluster_counts"] = []
        with mock.patch.object(QUAL, "_load_development", self._fake_load):
            with self.assertRaisesRegex(
                QUAL.QualificationError, "one-through-five-way"
            ):
                QUAL.evaluate_evidence(
                    self.manifest,
                    evidence,
                    manifest_sha256=self.manifest_sha,
                    manifest_root=None,
                )

    def _check_source_audit_uses_committed_bytes_and_rejects_legacy_route(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            initialize_source_repository(repo, {})
            path = repo / self.manifest["source_gate"]["production_path"]
            safe = commit_source_repository(repo, "safe")
            audit = QUAL.audit_source(repo, safe, self.manifest)
            self.assertTrue(audit["forbidden_symbols_absent"])

            path.write_text("fn live() { fit_ppml_separated(); }\n", encoding="utf-8")
            self.assertTrue(
                QUAL.audit_source(repo, safe, self.manifest)["forbidden_symbols_absent"],
                "an uncommitted working-tree mutation must not affect the exact-SHA audit",
            )
            legacy = commit_source_repository(repo, "legacy")
            audit = QUAL.audit_source(repo, legacy, self.manifest)
            self.assertFalse(audit["forbidden_symbols_absent"])
            self.assertTrue(
                any("`fit_ppml_separated`" in item for item in audit["forbidden_symbols_found"])
            )

    def _check_source_audit_rejects_a_production_wrapper_call(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            initialize_source_repository(
                repo,
                {
                    "crates/fereg-plugin/src/wrapper.rs": "fn wrapper() { fit_prepared_ppml(); }\n",
                },
            )
            safe = commit_source_repository(repo, "safe wrapper")
            self.assertTrue(
                QUAL.audit_source(repo, safe, self.manifest)["forbidden_symbols_absent"]
            )

            (repo / "crates/fereg-plugin/src/wrapper.rs").write_text(
                "fn wrapper() { fereg_core::ppml::fit_ppml(); }\n",
                encoding="utf-8",
            )
            wrapped = commit_source_repository(repo, "route through wrapper")
            audit = QUAL.audit_source(repo, wrapped, self.manifest)
            self.assertFalse(audit["forbidden_symbols_absent"])
            self.assertTrue(
                any("`fit_ppml`" in item for item in audit["forbidden_symbols_found"])
            )

    def _check_source_audit_rejects_ungating_a_legacy_separation_module(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            initialize_source_repository(repo, {})
            safe = commit_source_repository(repo, "test-only legacy module")
            self.assertTrue(
                QUAL.audit_source(repo, safe, self.manifest)["forbidden_symbols_absent"]
            )

            owner = repo / "legacy/fereg-core/src/ppml_separation/mod.rs"
            owner.write_text(
                owner.read_text(encoding="utf-8").replace(
                    "#[cfg(test)]\nmod runner;", "mod runner;", 1
                ),
                encoding="utf-8",
            )
            ungated = commit_source_repository(repo, "ungate legacy module")
            audit = QUAL.audit_source(repo, ungated, self.manifest)
            self.assertFalse(audit["forbidden_symbols_absent"])
            self.assertTrue(
                any(
                    "legacy module `runner` is not cfg(test)-only" in item
                    for item in audit["forbidden_symbols_found"]
                )
            )

    def _check_source_audit_rejects_ungating_the_legacy_start_type(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            initialize_source_repository(repo, {})
            safe = commit_source_repository(repo, "test-only legacy start")
            self.assertTrue(
                QUAL.audit_source(repo, safe, self.manifest)["forbidden_symbols_absent"]
            )

            owner = repo / "legacy/fereg-core/src/ppml/mod.rs"
            owner.write_text(
                owner.read_text(encoding="utf-8").replace(
                    "#[cfg(test)]\n#[derive(Debug, Clone, PartialEq)]\npub(crate) struct PpmlStart",
                    "#[derive(Debug, Clone, PartialEq)]\npub struct PpmlStart",
                    1,
                ),
                encoding="utf-8",
            )
            ungated = commit_source_repository(repo, "ungate legacy start")
            audit = QUAL.audit_source(repo, ungated, self.manifest)
            self.assertFalse(audit["forbidden_symbols_absent"])
            self.assertTrue(
                any("`PpmlStart`" in item for item in audit["forbidden_symbols_found"])
            )

    def test_receipts_are_exclusive_and_tamper_evident(self) -> None:
        payload = {
            "kind": "feppml_qualification_evaluation",
            "status": "pass",
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            QUAL.write_receipt(path, payload)
            self.assertEqual(QUAL.verify_receipt(path), payload)
            with self.assertRaisesRegex(QUAL.QualificationError, "overwrite"):
                QUAL.write_receipt(path, payload)
            document = json.loads(path.read_text(encoding="utf-8"))
            document["payload"]["status"] = "fail"
            path.chmod(0o644)
            path.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(QUAL.QualificationError, "seal mismatch"):
                QUAL.verify_receipt(path)


class FeppmlSourceGuardTests(unittest.TestCase):
    """Source-graph mutations do not depend on still-unfrozen contract files."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))

    def test_source_audit_uses_committed_bytes_and_rejects_legacy_route(self) -> None:
        FeppmlQualificationTests._check_source_audit_uses_committed_bytes_and_rejects_legacy_route(
            self
        )

    def test_source_audit_rejects_a_production_wrapper_call(self) -> None:
        FeppmlQualificationTests._check_source_audit_rejects_a_production_wrapper_call(self)

    def test_source_audit_rejects_ungating_a_legacy_separation_module(self) -> None:
        FeppmlQualificationTests._check_source_audit_rejects_ungating_a_legacy_separation_module(
            self
        )

    def test_source_audit_rejects_ungating_the_legacy_start_type(self) -> None:
        FeppmlQualificationTests._check_source_audit_rejects_ungating_the_legacy_start_type(self)


if __name__ == "__main__":
    unittest.main()
