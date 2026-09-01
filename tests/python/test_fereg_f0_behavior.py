import copy
import importlib.util
import json
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "fereg_f0_behavior.py"
SPEC = importlib.util.spec_from_file_location("fereg_f0_behavior_test", MODULE_PATH)
BEHAVIOR = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(BEHAVIOR)


class FeregF0BehaviorContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = BEHAVIOR.load_manifest()
        cls.manifest_sha = BEHAVIOR.sha256_file(BEHAVIOR.DEFAULT_MANIFEST)

    def test_normative_companion_examples_match_bundle_aware_cli(self):
        documentation = (
            ROOT / "docs" / "fereg_f0_behavior_contract.md"
        ).read_text(encoding="utf-8")
        governing_contract = (
            ROOT / "docs" / "fereg_regression_contract.md"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "(fereg_f0_behavior_contract.md)",
            governing_contract,
        )

        parsed = []
        for fenced in documentation.split("```bash")[1:]:
            block = fenced.split("```", 1)[0].strip()
            for rendered in block.split("\n\n"):
                command = rendered.replace("\\\n", " ").strip()
                if not command.startswith("python3 -I -B -S scripts/fereg_f0_behavior.py "):
                    continue
                tokens = shlex.split(command)
                parsed.append(BEHAVIOR.build_parser().parse_args(tokens[5:]))
        self.assertEqual(
            [arguments.command for arguments in parsed],
            ["capture", "capture", "evaluate", "verify"],
        )
        self.assertTrue(all(arguments.bundle_output for arguments in parsed[:2]))
        self.assertTrue(parsed[2].baseline_bundle)
        self.assertTrue(parsed[2].candidate_bundle)
        self.assertTrue(parsed[3].baseline_bundle)
        self.assertTrue(parsed[3].candidate_bundle)

    def test_cli_rejects_nonisolated_python_before_parsing(self):
        completed = subprocess.run(
            [sys.executable, str(MODULE_PATH), "--help"],
            cwd=ROOT,
            env={"PATH": "/usr/bin:/bin", "PYTHONPATH": "/tmp/hostile"},
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("require an exact Python launch with -I -B -S", completed.stdout)
        isolated = subprocess.run(
            [sys.executable, "-I", "-B", "-S", str(MODULE_PATH), "--help"],
            cwd=ROOT,
            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C"},
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        self.assertEqual(isolated.returncode, 0, isolated.stdout)
        with self.assertRaisesRegex(SystemExit, "exact Python launch"):
            BEHAVIOR.main(["verify"])
        with self.assertRaisesRegex(BEHAVIOR.BehaviorContractError, "exact Python launch"):
            BEHAVIOR.capture_observation(None)
        with self.assertRaisesRegex(BEHAVIOR.BehaviorContractError, "exact Python launch"):
            BEHAVIOR.evaluate_command(None)
        with self.assertRaisesRegex(BEHAVIOR.BehaviorContractError, "exact Python launch"):
            BEHAVIOR.verify_command(None)

    def test_fixture_capture_rejects_fake_stata_before_launch(self):
        with tempfile.TemporaryDirectory() as temp:
            fake = Path(temp) / "stata-mp"
            fake.write_text("#!/bin/sh\nexit 0\n", encoding="ascii")
            fake.chmod(0o755)
            with self.assertRaisesRegex(
                BEHAVIOR.BehaviorContractError, "exact pinned macOS Stata"
            ):
                BEHAVIOR._run_fixtures(
                    self.manifest,
                    stata=str(fake),
                    ado_root=Path(temp),
                    plugin=fake,
                    log_root=Path(temp) / "logs",
                )

    @staticmethod
    def _file(seed="a"):
        return {"sha256": seed * 64, "size_bytes": 100}

    def _source(self, *, baseline):
        if baseline:
            return copy.deepcopy(BEHAVIOR.F0_SOURCE)
        return {
            "commit": "3" * 40,
            "tree": "4" * 40,
            "cargo_lock_sha256": "5" * 64,
            "references_lock_sha256": "6" * 64,
        }

    def _package(self, *, phase):
        contract = self.manifest["package_contract"]
        members = list(contract["baseline_members"])
        if phase == "activation":
            members.extend(contract["activation"]["allowed_added_members"])
        platform = "macos-universal"
        plugin = "fereg_macos.plugin"
        staged = [
            "fereg.pkg",
            "stata.toc",
            *[
                name
                for name in members
                if not name.endswith(".plugin") or name == plugin
            ],
        ]
        files = {
            name: self._file(format(index + 1, "x")[-1])
            for index, name in enumerate(staged)
        }
        byte_files = {
            name: copy.deepcopy(files[name])
            for name in contract["byte_identical_fereg_members"]
        }
        return {
            "platform": platform,
            "package": {
                "format_version": 3,
                "package_name": "fereg",
                "package_version": "0.5.0",
                "description": "frozen test package",
                "members": members,
                "file": copy.deepcopy(files["fereg.pkg"]),
            },
            "toc": {
                "format_version": 3,
                "package_name": "fereg",
                "package_version": "0.5.0",
                "description": "frozen test toc",
                "file": copy.deepcopy(files["stata.toc"]),
            },
            "staged_members": staged,
            "staged_files": files,
            "byte_identical_fereg_files": byte_files,
            "help_examples": [
                {"id": identifier, "sha256": "d" * 64}
                for identifier in contract["help_example_ids"]
            ],
        }

    def _observation(self, *, baseline, phase="preactivation"):
        role = "baseline" if baseline else "candidate"
        actual_phase = "baseline" if baseline else phase
        source = self._source(baseline=baseline)
        artifact = {
            "platform": "macos-universal",
            "sha256": ("7" if baseline else "8") * 64,
            "size_bytes": 9_000_000,
            "embedded_git_commit": source["commit"][:12],
        }
        f0_manifest = None
        if baseline:
            f0_manifest = {
                "id": "fereg_f0_regression_v1",
                "sha256": "9" * 64,
                "status": "candidate_unsealed",
                "source": copy.deepcopy(source),
                "artifact": copy.deepcopy(artifact),
            }
        outcomes = []
        for fixture in self.manifest["fixtures"]:
            fixture_id = fixture["id"]
            log = ""
            if fixture_id == BEHAVIOR.UTILITY_FIXTURE:
                log = "\n".join(
                    (
                        "FEREG_F0_OUTPUT_BEGIN|version",
                        "fereg.plugin_version=0.5.0",
                        "fereg.ado_contract=0.5.0",
                        "fereg.action_protocol=1.5",
                        "fereg.model_wire=2.1",
                        "fereg.ols_result_wire=1.4",
                        "fereg.plugin_abi=3.0",
                        "fereg.git_commit=abc123",
                        "fereg.build_date=2026-01-01",
                        "FEREG_F0_OUTPUT_END|version",
                        "FEREG_F0_OUTPUT_BEGIN|doctor",
                        "fereg.native_selftest=pass",
                        "FEREG_F0_OUTPUT_END|doctor",
                        "FEREG_F0_OUTPUT_BEGIN|selftest",
                        "fereg.selftest.overall=pass",
                        "FEREG_F0_OUTPUT_END|selftest",
                        "",
                    )
                )
            outcomes.append(
                {
                    "id": fixture_id,
                    "process_rc": 0,
                    "stata_rc": 0,
                    "terminal_sentinel": f"END_FIXTURE|{fixture_id}|rc=0",
                    "status": "pass",
                    "log_member": f"fixture-logs/{fixture_id}.log",
                    "status_member": f"fixture-status/{fixture_id}.status",
                    "rc_member": f"fixture-rc/{fixture_id}.json",
                    "stream_member": f"fixture-streams/{fixture_id}.stdout-stderr",
                    "wrapper_member": f"fixture-wrappers/{fixture_id}.do",
                    "invocation_member": f"fixture-invocations/{fixture_id}.json",
                    "normalized_output": BEHAVIOR._normalize_fixture_output(
                        fixture_id, log
                    ),
                }
            )
        return {
            "schema_version": 1,
            "receipt_type": "fereg_f0_behavior_observation_v1",
            "status": "pass",
            "role": role,
            "phase": actual_phase,
            "contract": {
                "id": self.manifest["manifest_id"],
                "sha256": self.manifest_sha,
            },
            "f0_manifest": f0_manifest,
            "source": source,
            "controller_source": {
                "commit": "c" * 40, "tree": "d" * 40,
                "cargo_lock_sha256": "e" * 64,
                "references_lock_sha256": "f" * 64,
                "archive_sha256": "1" * 64,
                "archive_size_bytes": 12345,
                "lexical_root": str(BEHAVIOR.ROOT.resolve()),
            },
            "controller_programs": {
                relative: {
                    "sha256": BEHAVIOR.sha256_file(BEHAVIOR.ROOT / relative),
                    "size_bytes": (BEHAVIOR.ROOT / relative).stat().st_size,
                }
                for relative in ("scripts/fereg_f0_behavior.py", "scripts/run_stata_tests.py")
            },
            "artifact": artifact,
            "stata_controller": {
                "path": str(BEHAVIOR.DARWIN_STATA),
                "sha256": BEHAVIOR.DARWIN_STATA_SHA256,
                "size_bytes": BEHAVIOR.DARWIN_STATA_SIZE,
                "argv_contract": [str(BEHAVIOR.DARWIN_STATA), "-q", "-b", "do", "<retained-wrapper.do>"],
                "environment_keys": ["HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ"],
                "environment_contract": "closed-private-home-tmp-v1",
                "stdout_stderr": "merged-retained-process-stream-v1",
            },
            "stata_environment": {
                "stata_version": "19.0",
                "stata_edition": "MP",
                "stata_os": "MacOSX",
                "stata_machine_type": "MacAppleSilicon",
                "stata_processors": 8,
            },
            "package": self._package(phase=actual_phase),
            "fixture_inventory": BEHAVIOR._fixture_inventory(self.manifest),
            "fixture_outcomes": outcomes,
            "evidence_bundle": {
                "format": BEHAVIOR.BUNDLE_FORMAT,
                "filename": "behavior-evidence.tar",
                "sha256": "f" * 64,
                "size_bytes": 10240,
                "members": [
                    {
                        "path": "placeholder/member",
                        "role": "test_placeholder",
                        "sha256": "e" * 64,
                        "size_bytes": 1,
                    }
                ],
            },
            "complete_sentinel": "FEREG_F0_DIRECT_COMMAND_COMPLETE",
        }

    def _comparison(self, *, phase="preactivation"):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False, phase=phase)
        return {
            "schema_version": 1,
            "receipt_type": "fereg_f0_direct_command_contract_v1",
            "status": "pass",
            "phase": phase,
            "contract": {
                "id": self.manifest["manifest_id"],
                "sha256": self.manifest_sha,
            },
            "baseline_receipt": {
                "sha256": "a" * 64,
                "payload_sha256": "b" * 64,
                "bundle_filename": "baseline.tar",
                "bundle_sha256": "1" * 64,
                "bundle_size_bytes": 10240,
            },
            "candidate_receipt": {
                "sha256": "c" * 64,
                "payload_sha256": "d" * 64,
                "bundle_filename": "candidate.tar",
                "bundle_sha256": "2" * 64,
                "bundle_size_bytes": 10240,
            },
            "baseline": {
                "source": baseline["source"],
                "artifact": baseline["artifact"],
            },
            "candidate": {
                "source": candidate["source"],
                "artifact": candidate["artifact"],
            },
            "stata_environment": baseline["stata_environment"],
            "comparison": BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            ),
            "complete_sentinel": "FEREG_F0_DIRECT_COMMAND_COMPLETE",
        }

    def test_frozen_manifest_and_help_example_inventory_validate(self):
        BEHAVIOR.validate_manifest(self.manifest)
        examples = BEHAVIOR._help_examples(
            ROOT / "stata/fereg.sthlp",
            self.manifest["package_contract"]["help_example_ids"],
        )
        self.assertEqual(
            [item["id"] for item in examples],
            self.manifest["package_contract"]["help_example_ids"],
        )

    def test_manifest_rejects_weakened_phase13_addition_allowlist(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["package_contract"]["activation"]["allowed_added_members"].append(
            "fereg_backdoor.ado"
        )
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.validate_manifest(manifest, root=None)

    def test_manifest_rejects_refrozen_fixture_or_package_inventory(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["fixtures"][0]["sha256"] = "e" * 64
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.validate_manifest(manifest, root=None)
        manifest = copy.deepcopy(self.manifest)
        manifest["package_contract"]["baseline_members"].remove("feppml.ado")
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.validate_manifest(manifest, root=None)

    def test_preactivation_observations_compare(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        comparison = BEHAVIOR.evaluate_receipts(
            self.manifest, self.manifest_sha, baseline, candidate
        )
        self.assertTrue(comparison["fixtures_equal"])
        self.assertEqual(comparison["package"]["package_member_policy"], "exact_f0")

    def test_comparison_rejects_mixed_controller_generation(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        candidate["controller_source"]["commit"] = "a" * 40
        with self.assertRaisesRegex(BEHAVIOR.BehaviorContractError, "mixed controller"):
            BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            )

    def test_activation_allows_only_registered_ppml_additions(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False, phase="activation")
        candidate["package"]["toc"]["description"] = (
            "high-dimensional fixed-effect OLS/WLS and PPML beta"
        )
        comparison = BEHAVIOR.evaluate_receipts(
            self.manifest, self.manifest_sha, baseline, candidate
        )
        self.assertEqual(
            comparison["package"]["package_member_policy"],
            "phase13_additive_ppml_only",
        )

    def test_preactivation_package_metadata_mutation_fails(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        candidate["package"]["toc"]["description"] = "premature PPML activation"
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            )

    def test_preactivation_public_feppml_stub_mutation_fails(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        candidate["package"]["staged_files"]["feppml.ado"] = self._file("e")
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            )

    def test_activation_authorizes_only_declared_shared_file_changes(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False, phase="activation")
        for name in self.manifest["package_contract"]["activation"][
            "allowed_changed_shared_files"
        ]:
            changed = self._file("e")
            candidate["package"]["staged_files"][name] = changed
            if name == "fereg.pkg":
                candidate["package"]["package"]["file"] = copy.deepcopy(changed)
            elif name == "stata.toc":
                candidate["package"]["toc"]["file"] = copy.deepcopy(changed)
        result = BEHAVIOR.evaluate_receipts(
            self.manifest, self.manifest_sha, baseline, candidate
        )
        self.assertEqual(
            result["package"]["authorized_changed_shared_files"],
            self.manifest["package_contract"]["activation"][
                "allowed_changed_shared_files"
            ],
        )

    def test_unregistered_activation_member_fails_closed(self):
        candidate = self._observation(baseline=False, phase="activation")
        candidate["package"]["package"]["members"].append("fereg_backdoor.ado")
        candidate["package"]["staged_members"].append("fereg_backdoor.ado")
        candidate["package"]["staged_files"]["fereg_backdoor.ado"] = self._file("e")
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR._validate_observation(
                self.manifest, self.manifest_sha, candidate
            )

    def test_byte_frozen_fereg_member_mutation_fails(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        name = self.manifest["package_contract"]["byte_identical_fereg_members"][0]
        candidate["package"]["byte_identical_fereg_files"][name] = self._file("e")
        candidate["package"]["staged_files"][name] = self._file("e")
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            )

    def test_help_example_mutation_fails(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        candidate["package"]["help_examples"][0]["sha256"] = "e" * 64
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            )

    def test_fixture_failure_or_sentinel_mutation_fails(self):
        candidate = self._observation(baseline=False)
        candidate["fixture_outcomes"][0]["terminal_sentinel"] = "false success"
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR._validate_observation(
                self.manifest, self.manifest_sha, candidate
            )

    def test_normalized_public_output_rejects_added_version_key(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        output = candidate["fixture_outcomes"][0]["normalized_output"]
        output["lines"].insert(5, "version|fereg.ppml_request_wire=1.1")
        record = {
            "format": output["format"],
            "actions": output["actions"],
            "lines": output["lines"],
        }
        output["sha256"] = BEHAVIOR.sha256_bytes(
            BEHAVIOR.canonical_json_bytes(record)
        )
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            )

    def test_deterministic_bundle_reopens_and_rejects_mutation(self):
        contents = {
            "a/source.do": b"version 18.0\n",
            "z/status.txt": b"stata_rc=0\n",
        }
        records = [
            BEHAVIOR._bundle_record("a/source.do", "fixture_source", contents["a/source.do"]),
            BEHAVIOR._bundle_record("z/status.txt", "fixture_status", contents["z/status.txt"]),
        ]
        manifest = {
            "schema_version": 1,
            "bundle_type": BEHAVIOR.BUNDLE_FORMAT,
            "members": records,
        }
        contents[BEHAVIOR.BUNDLE_MANIFEST] = (
            BEHAVIOR.canonical_json_bytes(manifest) + b"\n"
        )
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / "evidence.tar"
            BEHAVIOR._write_bundle(path, contents)
            identity = BEHAVIOR._evidence_bundle_record(path, records)
            reopened = BEHAVIOR._read_verified_bundle(path, identity)
            self.assertEqual(reopened["a/source.do"], b"version 18.0\n")
            changed = copy.deepcopy(identity)
            changed["members"][0]["sha256"] = "0" * 64
            with self.assertRaises(BEHAVIOR.BehaviorContractError):
                BEHAVIOR._read_verified_bundle(path, changed)

            # A receipt can bind arbitrary trailing bytes, so member metadata
            # alone is insufficient.  Only the canonical USTAR byte stream is
            # admissible even when its new whole-file digest is self-consistent.
            noncanonical = Path(name) / "noncanonical.tar"
            noncanonical.write_bytes(path.read_bytes() + b"\0" * 10240)
            noncanonical_identity = BEHAVIOR._evidence_bundle_record(
                noncanonical, records
            )
            with self.assertRaises(BEHAVIOR.BehaviorContractError):
                BEHAVIOR._read_verified_bundle(
                    noncanonical, noncanonical_identity
                )

    def test_empty_retained_process_stream_is_narrowly_valid(self):
        empty_stream = BEHAVIOR._bundle_record(
            "fixture-streams/help_examples.stdout-stderr",
            "fixture_stdout_stderr",
            b"",
        )
        record = {
            "format": BEHAVIOR.BUNDLE_FORMAT,
            "filename": "evidence.tar",
            "sha256": "f" * 64,
            "size_bytes": 10240,
            "members": [empty_stream],
        }
        BEHAVIOR._validate_evidence_bundle_record(record, "bundle")

        for field, value in (
            ("role", "fixture_log"),
            ("path", "fixture-logs/help_examples.log"),
        ):
            changed = copy.deepcopy(record)
            changed["members"][0][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(
                BEHAVIOR.BehaviorContractError, "size is invalid"
            ):
                BEHAVIOR._validate_evidence_bundle_record(changed, "bundle")

    def test_retained_plugin_bytes_must_embed_claimed_source(self):
        source = self._source(baseline=False)
        BEHAVIOR._verify_plugin_source_bytes(
            b"prefix" + source["commit"][:12].encode("ascii") + b"suffix",
            source,
        )
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR._verify_plugin_source_bytes(b"unrelated artifact", source)

    def test_baseline_f0_manifest_status_or_fields_cannot_be_forged(self):
        baseline = self._observation(baseline=True)
        baseline["f0_manifest"]["status"] = "frozen"
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR._validate_observation(
                self.manifest, self.manifest_sha, baseline
            )
        baseline = self._observation(baseline=True)
        baseline["f0_manifest"]["waiver"] = True
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR._validate_observation(
                self.manifest, self.manifest_sha, baseline
            )

    def test_baseline_capture_accepts_only_the_exact_unsealed_native_artifact(self):
        regression = json.loads(
            (ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json").read_text(
                encoding="utf-8"
            )
        )
        artifact = next(
            {
                field: item[field]
                for field in (
                    "platform", "sha256", "size_bytes", "embedded_git_commit"
                )
            }
            for item in regression["baseline_candidate"]["artifacts"]
            if item["platform"] == "macos-universal"
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "f0.json"
            path.write_text(json.dumps(regression), encoding="utf-8")
            identity = BEHAVIOR._load_f0_identity(path, self.manifest, artifact)
            self.assertEqual(identity["artifact"], artifact)
            changed = copy.deepcopy(artifact)
            changed["sha256"] = "0" * 64
            with self.assertRaisesRegex(
                BEHAVIOR.BehaviorContractError, "F0 artifact differs"
            ):
                BEHAVIOR._load_f0_identity(path, self.manifest, changed)

    def test_platform_or_stata_mismatch_fails(self):
        baseline = self._observation(baseline=True)
        candidate = self._observation(baseline=False)
        candidate["stata_environment"]["stata_processors"] = 4
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR.evaluate_receipts(
                self.manifest, self.manifest_sha, baseline, candidate
            )

    def test_sealed_observation_round_trip_and_mutation(self):
        payload = self._observation(baseline=True)
        document = BEHAVIOR.seal_payload(payload)
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / "receipt.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            observed, _ = BEHAVIOR._read_sealed(path)
            BEHAVIOR._validate_observation(
                self.manifest, self.manifest_sha, observed
            )
            document["payload"]["status"] = "fail"
            path.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaises(BEHAVIOR.BehaviorContractError):
                BEHAVIOR._read_sealed(path)

    def test_comparison_receipt_result_mutation_fails_closed(self):
        payload = self._comparison()
        BEHAVIOR._validate_comparison(
            self.manifest, self.manifest_sha, payload
        )
        payload["comparison"]["package"]["package_member_policy"] = "waived"
        with self.assertRaises(BEHAVIOR.BehaviorContractError):
            BEHAVIOR._validate_comparison(
                self.manifest, self.manifest_sha, payload
            )

    def test_public_fixture_closes_requested_surfaces(self):
        source = (
            ROOT / "tests/stata/qualification/fereg_f0_public_contract.do"
        ).read_text(encoding="utf-8")
        for token in (
            "fereg, version",
            "fereg, doctor threads(1)",
            "fereg, selftest threads(1)",
            "threads(-1)",
            "memory(unknown)",
            "assert _rc == 2001",
            "predict double __f0_xb, xb",
            "test x1",
            "estat summarize y x1 x2",
            "assert e(sample) == __f0_prior_sample",
        ):
            self.assertIn(token, source)


if __name__ == "__main__":
    unittest.main()
