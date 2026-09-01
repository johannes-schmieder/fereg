from __future__ import annotations

import copy
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
PROGRAM = ROOT / "scripts/feppml_qualification.py"
SPEC = importlib.util.spec_from_file_location("feppml_qualification_strict_test", PROGRAM)
assert SPEC is not None and SPEC.loader is not None
QUAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUAL)


class FeppmlQualificationStrictReceiptTests(unittest.TestCase):
    def test_qualification_manifest_pins_development_bytes_and_current_memory_gate(self) -> None:
        manifest = json.loads(
            (ROOT / "benchmarks/manifests/feppml_qualification_v1.json").read_text(
                encoding="utf-8"
            )
        )
        development = manifest["matrix_contract"]["development_manifest"]
        self.assertEqual(
            development["path"],
            "benchmarks/manifests/ppml_memory_performance_v1.json",
        )
        self.assertEqual(
            development["sha256"],
            QUAL.sha256_file(ROOT / development["path"]),
        )
        QUAL.validate_manifest(manifest, root=None)
        changed = copy.deepcopy(manifest)
        changed["memory_gates"]["maximum_allocator_event_growth"] = 1
        with self.assertRaisesRegex(QUAL.QualificationError, "memory gate"):
            QUAL.validate_manifest(changed, root=None)

    def _paths(self, root: Path) -> tuple[Path, Path, Path]:
        manifest = root / "manifest.json"
        evidence = root / "evidence.json"
        receipt = root / "receipt.json"
        manifest.write_text("{}\n", encoding="ascii")
        evidence.write_text("{}\n", encoding="ascii")
        return manifest, evidence, receipt

    def test_authoritative_verifier_recomputes_and_checks_committed_source(self) -> None:
        result = {
            "kind": "feppml_qualification_evaluation",
            "status": "pass",
            "stage": "complete",
            "performance_evaluated": True,
            "reasons": [],
        }
        with tempfile.TemporaryDirectory() as temporary:
            manifest, evidence, receipt = self._paths(Path(temporary))
            source_archive = Path(temporary) / "candidate.tar"
            source_archive.write_bytes(b"sealed archive")
            QUAL.write_receipt(receipt, result)
            with (
                mock.patch.object(QUAL, "load_manifest", return_value={}),
                mock.patch.object(QUAL, "evaluate_evidence", return_value=result),
                mock.patch.object(QUAL, "_verify_committed_candidate") as source_check,
            ):
                self.assertEqual(
                    QUAL.verify_qualification_receipt(
                        manifest,
                        evidence,
                        receipt,
                        source_archive_path=source_archive,
                    ),
                    result,
                )
            source_check.assert_called_once()
            self.assertEqual(
                source_check.call_args.kwargs["source_archive_path"], source_archive
            )

    def test_valid_envelope_cannot_replace_recomputed_result_or_pass_gate(self) -> None:
        passing = {
            "kind": "feppml_qualification_evaluation",
            "status": "pass",
            "stage": "complete",
            "performance_evaluated": True,
            "reasons": [],
        }
        with tempfile.TemporaryDirectory() as temporary:
            manifest, evidence, receipt = self._paths(Path(temporary))
            forged = {**passing, "candidate_source_commit": "f" * 40}
            QUAL.write_receipt(receipt, forged)
            with (
                mock.patch.object(QUAL, "load_manifest", return_value={}),
                mock.patch.object(QUAL, "evaluate_evidence", return_value=passing),
                self.assertRaisesRegex(QUAL.QualificationError, "fresh evidence"),
            ):
                QUAL.verify_qualification_receipt(manifest, evidence, receipt)

        blocked = {**passing, "status": "blocked", "reasons": ["not complete"]}
        with tempfile.TemporaryDirectory() as temporary:
            manifest, evidence, receipt = self._paths(Path(temporary))
            QUAL.write_receipt(receipt, blocked)
            with (
                mock.patch.object(QUAL, "load_manifest", return_value={}),
                mock.patch.object(QUAL, "evaluate_evidence", return_value=blocked),
                self.assertRaisesRegex(QUAL.QualificationError, "not a passing"),
            ):
                QUAL.verify_qualification_receipt(manifest, evidence, receipt)

    def test_committed_candidate_reconstructs_tree_locks_archive_and_audit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            subprocess.run(["git", "init", "-q", str(repo)], check=True)
            subprocess.run(
                ["git", "-C", str(repo), "config", "user.email", "strict@test.invalid"],
                check=True,
            )
            subprocess.run(
                ["git", "-C", str(repo), "config", "user.name", "Strict Test"],
                check=True,
            )
            production = repo / "crates/fereg-plugin/src/ppml_estimate.rs"
            production.parent.mkdir(parents=True)
            production.write_text("fn prepared_ppml_entry() {}\n", encoding="ascii")
            (repo / "Cargo.lock").write_text("# exact lock\n", encoding="ascii")
            (repo / "references.lock.toml").write_text(
                "schema_version = 2\n", encoding="ascii"
            )
            development_path = (
                repo / "benchmarks/manifests/ppml_memory_performance_v1.json"
            )
            development_path.parent.mkdir(parents=True)
            development_path.write_text("{}\n", encoding="ascii")
            qualification_path = (
                repo / "benchmarks/manifests/feppml_qualification_v1.json"
            )
            qualification_path.write_text('{"frozen":true}\n', encoding="ascii")
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            subprocess.run(
                ["git", "-C", str(repo), "commit", "-qm", "candidate"], check=True
            )
            commit = subprocess.check_output(
                ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
            ).strip()
            tree = subprocess.check_output(
                ["git", "-C", str(repo), "rev-parse", "HEAD^{tree}"], text=True
            ).strip()
            archive = subprocess.check_output(
                ["git", "-C", str(repo), "archive", "--format=tar", commit]
            )
            manifest = {
                "source_gate": {
                    "production_path": "crates/fereg-plugin/src/ppml_estimate.rs"
                },
                "matrix_contract": {
                    "development_manifest": {
                        "path": "benchmarks/manifests/ppml_memory_performance_v1.json",
                        "sha256": hashlib.sha256(
                            development_path.read_bytes()
                        ).hexdigest(),
                    }
                },
            }
            candidate = {
                "source_commit": commit,
                "source_tree": tree,
                "cargo_lock_sha256": hashlib.sha256(
                    (repo / "Cargo.lock").read_bytes()
                ).hexdigest(),
                "references_lock_sha256": hashlib.sha256(
                    (repo / "references.lock.toml").read_bytes()
                ).hexdigest(),
                "source_archive_sha256": hashlib.sha256(archive).hexdigest(),
                "source_archive_size_bytes": len(archive),
                "package_sha256": "1" * 64,
                "performance_plugin_sha256": "2" * 64,
                "performance_plugin_size_bytes": 1,
                "embedded_git_commit": commit[:12],
                "build_date_utc": datetime.fromtimestamp(
                    int(
                        subprocess.check_output(
                            ["git", "-C", str(repo), "show", "-s", "--format=%ct", commit],
                            text=True,
                        ).strip()
                    ),
                    timezone.utc,
                ).date().isoformat(),
            }
            evidence = {
                "candidate": candidate,
                "source_audit": QUAL.audit_source(repo, commit, manifest),
            }
            QUAL._verify_committed_candidate(
                repo, manifest, evidence, manifest_path=qualification_path
            )

            archive_path = repo / "candidate.tar"
            archive_path.write_bytes(archive)
            extracted = repo / "extracted"
            extracted.mkdir()
            with tarfile.open(archive_path, mode="r:") as source:
                source.extractall(extracted)
            QUAL._verify_committed_candidate(
                extracted,
                manifest,
                evidence,
                manifest_path=(
                    extracted
                    / "benchmarks/manifests/feppml_qualification_v1.json"
                ),
                source_archive_path=archive_path,
            )

            extra_build = extracted / "crates/forged/build.rs"
            extra_build.parent.mkdir()
            extra_build.write_text("fn main() {}\n", encoding="ascii")
            with self.assertRaisesRegex(
                QUAL.QualificationError, "inventory differs"
            ):
                QUAL._verify_committed_candidate(
                    extracted,
                    manifest,
                    evidence,
                    manifest_path=(
                        extracted
                        / "benchmarks/manifests/feppml_qualification_v1.json"
                    ),
                    source_archive_path=archive_path,
                )
            extra_build.unlink()
            extra_build.parent.rmdir()

            cargo_config = extracted / ".cargo/config.toml"
            cargo_config.parent.mkdir()
            cargo_config.write_text("[build]\nrustflags=[]\n", encoding="ascii")
            with self.assertRaisesRegex(
                QUAL.QualificationError, "inventory differs"
            ):
                QUAL._verify_committed_candidate(
                    extracted,
                    manifest,
                    evidence,
                    manifest_path=(
                        extracted
                        / "benchmarks/manifests/feppml_qualification_v1.json"
                    ),
                    source_archive_path=archive_path,
                )
            cargo_config.unlink()
            cargo_config.parent.rmdir()

            crates = extracted / "crates"
            real_crates = extracted / "crates-real"
            crates.rename(real_crates)
            crates.symlink_to(real_crates, target_is_directory=True)
            with self.assertRaisesRegex(
                QUAL.QualificationError, "symlink|inventory differs"
            ):
                QUAL._verify_committed_candidate(
                    extracted,
                    manifest,
                    evidence,
                    manifest_path=(
                        extracted
                        / "benchmarks/manifests/feppml_qualification_v1.json"
                    ),
                    source_archive_path=archive_path,
                )
            crates.unlink()
            real_crates.rename(crates)

            extracted_lock = extracted / "Cargo.lock"
            extracted_lock.write_text("# changed after extraction\n", encoding="ascii")
            with self.assertRaisesRegex(
                QUAL.QualificationError, "extracted candidate source changed"
            ):
                QUAL._verify_committed_candidate(
                    extracted,
                    manifest,
                    evidence,
                    manifest_path=(
                        extracted
                        / "benchmarks/manifests/feppml_qualification_v1.json"
                    ),
                    source_archive_path=archive_path,
                )

            corrupt_archive = repo / "candidate-corrupt.tar"
            corrupt_archive.write_bytes(archive + b"tamper")
            with self.assertRaisesRegex(
                QUAL.QualificationError, "archive bytes differ"
            ):
                QUAL._verify_committed_candidate(
                    extracted,
                    manifest,
                    evidence,
                    manifest_path=(
                        extracted
                        / "benchmarks/manifests/feppml_qualification_v1.json"
                    ),
                    source_archive_path=corrupt_archive,
                )

            changed = copy.deepcopy(evidence)
            changed["candidate"]["source_archive_sha256"] = "f" * 64
            with self.assertRaisesRegex(QUAL.QualificationError, "exact git archive"):
                QUAL._verify_committed_candidate(
                    repo, manifest, changed, manifest_path=qualification_path
                )


if __name__ == "__main__":
    unittest.main()
