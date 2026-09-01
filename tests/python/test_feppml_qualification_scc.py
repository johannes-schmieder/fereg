from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "benchmarks/scc/feppml_qualification_scc.py"
SPEC = importlib.util.spec_from_file_location("feppml_qualification_scc_test", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
SCC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCC)


class FeppmlQualificationSccTests(unittest.TestCase):
    @staticmethod
    def source_git_record() -> dict:
        return {
            **SCC.EXPECTED_SOURCE_GIT,
            "environment": SCC.formal_build._git_environment(
                Path(SCC.EXPECTED_SOURCE_GIT["resolved_path"])
            ),
            "replace_objects_disabled": True,
            "rewrite_mechanisms_absent": True,
        }

    @staticmethod
    def qacct_record(
        job_id: str,
        task_id: int | None,
        node: str = "node-a",
        *,
        slots: int = 16,
        job_name: str = "fq_test_a",
        owner: str = "johannes",
        granted_pe: str = "omp",
        extra_fields: tuple[str, ...] = (),
    ) -> str:
        fields = [
            "==============================================================",
            f"qname          all.q@{node}.scc.bu.edu",
            f"hostname       {node}.scc.bu.edu",
            "project        welfgr",
            f"jobnumber      {job_id}",
            f"owner          {owner}",
            f"jobname        {job_name}",
            "qsub_time      Sun Aug 31 12:00:00 2026",
            "start_time     Sun Aug 31 12:01:00 2026",
            "end_time       Sun Aug 31 12:02:00 2026",
            f"granted_pe     {granted_pe}",
        ]
        if task_id is not None:
            fields.append(f"taskid         {task_id}")
        fields.extend(
            [
                f"slots          {slots}",
                "failed         0",
                "exit_status    0",
                "ru_wallclock   12.5",
                "cpu            10.25",
                "maxvmem        3.5G",
                *extra_fields,
            ]
        )
        return "\n".join(fields)

    @staticmethod
    def qstat_record(
        *,
        job_id: str,
        role: str,
        run_root: Path,
        script: Path,
        tasks: int = 1,
        hold: str | None = None,
    ) -> str:
        fields = [
            f"job_number:                 {job_id}",
            "owner:                      johannes",
            f"job_name:                   {SCC._job_name(run_root, role)}",
            f"cwd:                        {run_root}",
            f"script_file:                {script}",
            (
                "hard resource_list:         h_rt=1800,mem_per_core=4G"
                if role == "collector"
                else "hard resource_list:         h_rt=42300,mem_per_core=4G"
            ),
            f"env_list:                   RUN_ROOT={run_root}",
            "shell_list:                 NONE:/bin/bash",
            f"stdout_path_list:           NONE:NONE:{run_root / 'logs'}",
            "project:                    welfgr",
            "merge:                      y",
        ]
        if role != "collector":
            fields.append("parallel environment:       omp range: 16")
        if role == "array":
            fields.append(f"job-array tasks:           1-{tasks}:1")
        if hold is not None:
            fields.append(f"jid_predecessor_list_req:  {hold}")
        return "\n".join(fields) + "\n"

    def test_harness_inventory_contains_complete_lifecycle_and_frozen_inputs(self) -> None:
        required = {
            "benchmarks/scc/feppml_qualification_scc.py",
            "benchmarks/scc/feppml_qualification_deploy.sh",
            "benchmarks/scc/feppml_qualification_setup_job.sh",
            "benchmarks/scc/feppml_qualification_job.sh",
            "benchmarks/scc/feppml_qualification_collector_job.sh",
            "benchmarks/scc/feppml_qualification_submit.sh",
            "benchmarks/scc/feppml_qualification_close.sh",
            "scripts/feppml_qualification.py",
            "scripts/feppml_qualification_capture.py",
            "scripts/generate_feppml_qualification_spec.py",
            "scripts/generate_feppml_qualification_recipes.py",
            "scripts/feppml_qualification_recipe_contract.py",
            "scripts/feppml_separation_recipe_contract.py",
            "scripts/generate_feppml_oracle_fixtures.py",
            "scripts/generate_feppml_constrained_recipes.py",
            "scripts/materialize_feppml_capture_plan.py",
            "scripts/normalize_feppml_stata_raw.py",
            "scripts/parse_feppml_stata_protocol.py",
            "scripts/feppml_explicit_dummy_oracle.py",
            "scripts/run_feppml_qualification.py",
            "scripts/ppml_memory_performance.py",
            "scripts/ppml_legacy_boundary.py",
            "scripts/prepare_windows_feppml_target.py",
            "references.lock.toml",
            "tests/python/ppml_explicit_dummy_oracle.py",
            "tests/python/ppml_multiway_cluster_oracle.py",
        }
        self.assertTrue(required <= set(SCC.HARNESS_PATHS))
        source = MODULE_PATH.read_text(encoding="utf-8")
        for token in (
            "audit_source",
            "forbidden_symbols_absent",
            "SETUP.SUCCESS",
            "_capture_pair",
            "evaluate_evidence",
            "qacct",
            "RUN.SUCCESS",
            "feppml_qualification_capture.py",
            "capture_plan_sha256",
            "capture_materialization_receipt_sha256",
            "preparation_sha256",
            "scheduler_job_id",
        ):
            self.assertIn(token, source)

    def test_direct_root_invocation_bootstraps_repository_imports(self) -> None:
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                "-B",
                "-S",
                "benchmarks/scc/feppml_qualification_scc.py",
                "--help",
            ],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("prepare", completed.stdout)

    def test_exact_staged_harness_imports_without_unstaged_repository_modules(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            staged = Path(temporary) / "harness"
            for relative in SCC.HARNESS_PATHS:
                source = ROOT / relative
                destination = staged / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, destination)
            completed = subprocess.run(
                [
                    sys.executable,
                    "-I",
                    "-B",
                    "-S",
                    str(staged / "benchmarks/scc/feppml_qualification_scc.py"),
                    "--help",
                ],
                cwd=staged,
                env={
                    **os.environ,
                    "PYTHONPATH": str(Path(temporary) / "hostile-shadow"),
                    "PYTHONHOME": str(Path(temporary) / "hostile-home"),
                },
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn("prepare", completed.stdout)
            self.assertFalse(any(staged.rglob("__pycache__")))

    def test_runtime_paths_and_execution_metadata_are_closed(self) -> None:
        run_root = Path("/private/tmp/20260831T120000Z-feppml-q")
        manifest, development, spec = SCC._paths(run_root)
        self.assertEqual(manifest, run_root / "input/manifests/qualification.json")
        self.assertEqual(development, run_root / "input/manifests/development.json")
        self.assertEqual(spec, run_root / "work/capture/spec.json")
        template = {
            "stata_version": "19.0 MP",
            "stata_flavor": "MP",
            "rustc_version": "rustc-template",
        }
        with mock.patch.object(SCC, "_run", return_value=b"rustc 1.90.0\n"):
            environment = SCC._runtime_environment(template, 4)
        self.assertEqual(
            set(environment),
            {
                "node_id", "stata_version", "stata_flavor", "scheduler_job_id",
                "comparator_processor_limit", "os", "cpu", "rustc_version",
            },
        )
        self.assertEqual(environment["comparator_processor_limit"], 4)
        with mock.patch.object(SCC, "_run", return_value=b"rustc 1.90.0\n"):
            derived = SCC._runtime_environment(
                {**template, "stata_flavor": "SE"},
                4,
                stata_flavor="MP",
            )
        self.assertEqual(derived["stata_flavor"], "MP")
        source = MODULE_PATH.read_text(encoding="utf-8")
        for field in (
            '"scheduler_job_id": environment["scheduler_job_id"]',
            '"plugin_sha256": spec["evidence_header"]["candidate"]["performance_plugin_sha256"]',
            '"comparator_bundle_sha256": spec["evidence_header"]["comparator"]["bundle_sha256"]',
            '"rustc_version": environment["rustc_version"]',
            'qualification.oracle_evidence_records(manifest, benchmark)',
        ):
            self.assertIn(field, source)

    def test_formal_build_environment_ignores_hostile_ambient_overrides(self) -> None:
        tools = {
            role: {"path": f"/sealed/tools/{role}"}
            for role in SCC.BUILD_TOOL_ROLES
        }
        candidate = {
            "commit": "1" * 40,
            "build_date_utc": "2026-08-31",
        }
        hostile = {
            "CC": "/tmp/evil-cc",
            "RUSTFLAGS": "-C link-arg=/tmp/injected.o",
            "RUSTC_WRAPPER": "/tmp/wrapper",
            "CARGO_TARGET_DIR": "/tmp/substituted-target",
            "PYTHONPATH": "/tmp/shadow",
            "LD_PRELOAD": "/tmp/injected.so",
            "PATH": "/tmp/evil-bin",
        }
        with mock.patch.dict(os.environ, hostile, clear=False):
            environment = SCC._formal_build_environment(
                Path("/projectnb/welfgr/fereg/runs/20260831T120000Z-q"),
                candidate,
                tools,
                slots=16,
            )
        self.assertEqual(environment["CC"], "/sealed/tools/cc")
        self.assertEqual(environment["RUSTC"], "/sealed/tools/rustc")
        self.assertEqual(
            environment["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"],
            "/sealed/tools/linker",
        )
        for forbidden in hostile:
            if forbidden not in {"CC", "PATH"}:
                self.assertNotIn(forbidden, environment)
        self.assertNotEqual(environment["PATH"], hostile["PATH"])
        source = MODULE_PATH.read_text(encoding="utf-8")
        stage = source[source.index("def _stage_exact_bundles"):source.index("def _materialize_datasets")]
        self.assertNotIn("dict(os.environ)", stage)
        self.assertIn("_verify_build_receipt", stage)

    def test_formal_build_receipt_binds_dependency_origin_and_runtime_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_root = Path(temporary) / "20260831T120000Z-feppml-q"
            log = run_root / "logs/build/candidate.log"
            artifact = run_root / "work/candidate/dist/fereg_linux.plugin"
            log.parent.mkdir(parents=True)
            artifact.parent.mkdir(parents=True)
            log.write_bytes(b"formal build log\n")
            artifact.write_bytes(b"candidate plugin\n")
            log_record = SCC._relative_file_record(run_root, log)
            artifact_record = SCC._relative_file_record(run_root, artifact)
            dependencies = {"receipt_type": "sealed-dependencies", "files": []}
            rebound_dependencies = {
                "receipt_type": "relocated-dependencies", "files": []
            }
            candidate = {
                "commit": "1" * 40,
                "tree": "2" * 40,
                "build_date_utc": "2026-08-31",
            }
            preparation = {
                "candidate": candidate,
                "build_dependencies": dependencies,
            }
            environment = {"CARGO_NET_OFFLINE": "true"}
            formal_closure = {
                "environment": environment,
                "dependencies": rebound_dependencies,
                "source_root": "work/candidate",
                "slots": 16,
                "build_variables": {
                    "FEREG_GIT_COMMIT": candidate["commit"][:12],
                    "FEREG_BUILD_DATE": candidate["build_date_utc"],
                },
            }
            runtime_closure = {
                "receipt_type": "linux-runtime-closure",
                "artifact": artifact_record,
            }
            receipt = {
                "schema_version": 1,
                "receipt_type": "feppml-formal-linux-build-v1",
                "source_commit": candidate["commit"],
                "source_tree": candidate["tree"],
                "build_date_utc": candidate["build_date_utc"],
                "command": ["/bin/bash", "scripts/build_linux.sh"],
                "cwd": str(run_root / "work/candidate"),
                "environment": environment,
                "formal_closure": formal_closure,
                "dependency_origin_sha256": SCC.digest_bytes(
                    SCC.canonical(dependencies)
                ),
                "runtime_closure": runtime_closure,
                "log": log_record,
                "artifact": artifact_record,
                "complete_sentinel": "FEPPML_FORMAL_LINUX_BUILD_COMPLETE",
            }

            def verify_formal(
                observed_root, observed_formal, observed_runtime, *, portable
            ):
                self.assertEqual(observed_root, run_root)
                self.assertTrue(portable)
                self.assertEqual(observed_formal["source_root"], "work/candidate")
                if observed_runtime != runtime_closure:
                    raise SCC.formal_build.ClosureError(
                        "runtime artifact receipt changed"
                    )
                return {"artifact": artifact_record}

            patches = (
                mock.patch.object(
                    SCC.formal_build,
                    "verify_qualified_formal_rust_build",
                    side_effect=verify_formal,
                ),
                mock.patch.object(
                    SCC,
                    "_rebound_dependency_receipt",
                    return_value=rebound_dependencies,
                ),
            )
            with patches[0], patches[1]:
                SCC._verify_build_receipt(
                    run_root, preparation, receipt, portable=True
                )
                with self.assertRaisesRegex(
                    SCC.SccError, "dependency relocation"
                ):
                    SCC._verify_build_receipt(
                        run_root,
                        preparation,
                        {**receipt, "dependency_origin_sha256": "0" * 64},
                        portable=True,
                    )
                with self.assertRaisesRegex(
                    SCC.SccError, "dependency relocation"
                ):
                    SCC._verify_build_receipt(
                        run_root,
                        preparation,
                        {
                            **receipt,
                            "formal_closure": {
                                **formal_closure,
                                "dependencies": {
                                    **rebound_dependencies,
                                    "files": ["substituted"],
                                },
                            },
                        },
                        portable=True,
                    )
                with self.assertRaisesRegex(
                    SCC.SccError, "runtime artifact receipt changed"
                ):
                    SCC._verify_build_receipt(
                        run_root,
                        preparation,
                        {
                            **receipt,
                            "runtime_closure": {
                                **runtime_closure,
                                "artifact": {
                                    **artifact_record,
                                    "sha256": "f" * 64,
                                },
                            },
                        },
                        portable=True,
                    )

    def test_preparation_is_sealed_and_marker_binds_its_file_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_root = Path(temporary) / "20260831T120000Z-feppml-q"
            input_root = run_root / "input"
            payload = {
                "kind": "feppml_qualification_scc_preparation",
                "run_id": run_root.name,
                "files": {},
                "source_git": self.source_git_record(),
                "build_dependencies": {},
                "complete_sentinel": "FEPPML_SCC_PREPARED",
            }
            SCC.write_sealed(input_root / "preparation.json", payload)
            digest = SCC.digest_file(input_root / "preparation.json")
            (input_root / "PREPARED").write_text(
                f"FEPPML_SCC_PREPARED|{digest}\n", encoding="ascii"
            )
            with mock.patch.object(
                SCC, "_validate_locked_reference_archives", return_value={}
            ), mock.patch.object(
                SCC.formal_build, "verify_dependency_receipt", return_value={}
            ):
                self.assertEqual(SCC.verify_input(run_root), payload)
            marker = input_root / "PREPARED"
            marker.write_text("FEPPML_SCC_PREPARED|" + "0" * 64 + "\n", encoding="ascii")
            with self.assertRaisesRegex(SCC.SccError, "identity/sentinel"):
                with mock.patch.object(
                    SCC, "_validate_locked_reference_archives", return_value={}
                ), mock.patch.object(
                    SCC.formal_build, "verify_dependency_receipt", return_value={}
                ):
                    SCC.verify_input(run_root)

    def test_deployment_receipt_preserves_local_preparation_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_root = Path(temporary) / "20260831T120000Z-feppml-q"
            input_root = run_root / "input"
            payload = {
                "kind": "feppml_qualification_scc_preparation",
                "run_id": run_root.name,
                "files": {},
                "source_git": self.source_git_record(),
                "build_dependencies": {},
                "complete_sentinel": "FEPPML_SCC_PREPARED",
            }
            SCC.write_sealed(input_root / "preparation.json", payload)
            digest = SCC.digest_file(input_root / "preparation.json")
            (input_root / "PREPARED").write_text(
                f"FEPPML_SCC_PREPARED|{digest}\n", encoding="ascii"
            )
            args = SCC.argparse.Namespace(
                run_root=run_root, preparation_sha256=digest
            )
            controller = {"role": "deployment"}
            with mock.patch.object(
                SCC, "_validate_locked_reference_archives", return_value={}
            ), mock.patch.object(
                SCC.formal_build, "verify_dependency_receipt", return_value={}
            ), mock.patch.object(
                SCC, "_controller_identity", return_value=controller
            ), mock.patch.object(
                SCC, "_verify_controller_identity", return_value=controller
            ):
                SCC.record_deployment(args)
                self.assertEqual(
                    SCC.verify_deployment(run_root)["preparation_sha256"], digest
                )
            wrong_root = Path(temporary) / "20260831T120001Z-feppml-q"
            wrong_input = wrong_root / "input"
            SCC.write_sealed(wrong_input / "preparation.json", {
                **payload, "run_id": wrong_root.name,
            })
            wrong_digest = SCC.digest_file(wrong_input / "preparation.json")
            (wrong_input / "PREPARED").write_text(
                f"FEPPML_SCC_PREPARED|{wrong_digest}\n", encoding="ascii"
            )
            with mock.patch.object(
                SCC, "_validate_locked_reference_archives", return_value={}
            ), mock.patch.object(
                SCC.formal_build, "verify_dependency_receipt", return_value={}
            ), mock.patch.object(
                SCC, "_controller_identity", return_value=controller
            ):
                with self.assertRaisesRegex(SCC.SccError, "differs from the local"):
                    SCC.record_deployment(
                        SCC.argparse.Namespace(
                            run_root=wrong_root, preparation_sha256="0" * 64
                        )
                    )

    def test_platform_inputs_are_copied_then_rebound_to_run_local_bytes(self) -> None:
        manifest = SCC.qualification.load_manifest(
            ROOT / "benchmarks/manifests/feppml_qualification_v1.json",
            verify_files=False,
        )
        source_commit = "1" * 40
        source_tree = "2" * 40
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            records = []
            for platform in manifest["platform_gates"]["required_artifacts"]:
                platform_root = source / platform
                platform_root.mkdir()
                files = {}
                for field, name in (
                    ("artifact", f"fereg_{platform}.plugin"),
                    ("capture", "capture.json"),
                    ("source_archive", "source.tar"),
                    ("capture_program", "feppml_platform_load_capture.do"),
                    ("receipt", "receipt.json"),
                ):
                    path = platform_root / name
                    path.write_bytes(f"{platform}-{field}".encode("ascii"))
                    files[field] = {
                        "path": str(path),
                        "sha256": SCC.digest_file(path),
                        "size_bytes": path.stat().st_size,
                    }
                provenance = {}
                if platform in {"macos-universal", "linux-x86_64"}:
                    for field, name in (
                        ("producer", "producer.py"),
                        ("attestation", "attestation.json"),
                        ("status", "attestation.json.status"),
                        ("stata_log", "attestation.json.stata.log"),
                        ("wrapper", "attestation.json.wrapper.do"),
                        ("invocation", "attestation.json.invocation.json"),
                    ):
                        path = platform_root / name
                        path.write_bytes(f"{platform}-{field}".encode("ascii"))
                        provenance[field] = {
                            "path": str(path),
                            "sha256": SCC.digest_file(path),
                            "size_bytes": path.stat().st_size,
                        }
                else:
                    for field in (
                        "controller_receipt", "controller_output",
                        "controller_archive", "results_archive", "source_manifest",
                    ):
                        path = platform_root / f"{field}.bin"
                        path.write_bytes(f"{platform}-{field}".encode("ascii"))
                        provenance[field] = {
                            "path": str(path),
                            "sha256": SCC.digest_file(path),
                            "size_bytes": path.stat().st_size,
                        }
                records.append(
                    {
                        "platform": platform,
                        "qualification_profile": "candidate",
                        **files,
                        "platform_provenance": provenance,
                    }
                )
            run_root = root / "20260831T120000Z-feppml-q"
            payloads = {
                record["platform"]: {
                    "platform": record["platform"],
                    "qualification_profile": "candidate",
                }
                for record in records
            }
            rebound_records = {
                platform: {
                    "platform": platform, "qualification_profile": "candidate"
                }
                for platform in payloads
            }
            candidate = {
                "source_commit": source_commit,
                "source_tree": source_tree,
                "source_archive_sha256": "3" * 64,
                "source_archive_size_bytes": 1,
                "embedded_git_commit": source_commit[:12],
                "build_date_utc": "2026-08-31",
                "performance_plugin_sha256": "4" * 64,
                "performance_plugin_size_bytes": 1,
            }

            def verify_evidence(record):
                return payloads[str(record["platform"])]

            def rebind(**arguments):
                for field in (
                    "artifact", "capture_path", "source_archive",
                    "capture_program", "receipt",
                ):
                    self.assertTrue(Path(arguments[field]).is_file())
                if arguments["platform"] == "windows-x86_64":
                    self.assertTrue(Path(arguments["windows_results_archive"]).is_file())
                else:
                    attestation = Path(arguments["native_attestation"])
                    self.assertTrue(attestation.is_file())
                    for sidecar in SCC.platform_receipts.native_sidecar_paths(attestation).values():
                        self.assertTrue(sidecar.is_file())
                return rebound_records[arguments["platform"]]

            with mock.patch.object(
                SCC.platform_receipts, "verify_evidence", side_effect=verify_evidence
            ), mock.patch.object(
                SCC.platform_receipts, "rebind", side_effect=rebind
            ), mock.patch.object(SCC.qualification, "_validate_platforms") as validate:
                staged = SCC._stage_platform_prerequisites(run_root / "input", records)
                rebound = SCC._late_platform_records(
                    run_root, {"platform_inputs": staged}, manifest, candidate
                )
            self.assertEqual(
                [record["platform"] for record in rebound],
                manifest["platform_gates"]["required_artifacts"],
            )
            validate.assert_called_once_with(manifest, rebound, candidate)
            for record in records:
                platform = record["platform"]
                self.assertEqual(
                    (run_root / staged[platform]["artifact"]).name,
                    Path(record["artifact"]["path"]).name,
                )
                self.assertEqual(
                    (run_root / staged[platform]["capture_program"]).name,
                    Path(record["capture_program"]["path"]).name,
                )

    def test_shell_entrypoints_are_syntax_valid_and_never_submit_mutable_actions(self) -> None:
        scripts = sorted((ROOT / "benchmarks/scc").glob("feppml_qualification_*.sh"))
        self.assertEqual(len(scripts), 6)
        subprocess.run(["bash", "-n", *map(str, scripts)], check=True)
        for path in scripts:
            text = path.read_text(encoding="utf-8")
            self.assertNotIn("--delete", text)
            self.assertNotIn("actions/", text)
            self.assertNotIn("--info=progress2", text)
        setup = (ROOT / "benchmarks/scc/feppml_qualification_setup_job.sh").read_text(
            encoding="utf-8"
        )
        deploy = (ROOT / "benchmarks/scc/feppml_qualification_deploy.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("fereg-rust-1.85.1", setup)
        self.assertIn("RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu", setup)
        self.assertIn('test "$(cargo --version)" = "cargo 1.85.1', setup)
        self.assertIn("FEPPML_STATA_EXECUTABLE=$(command -v stata-mp)", setup)
        for path in scripts:
            if path.name in {
                "feppml_qualification_setup_job.sh",
                "feppml_qualification_job.sh",
                "feppml_qualification_collector_job.sh",
            }:
                text = path.read_text(encoding="utf-8")
                self.assertIn("/usr/bin/env -i", text)
                self.assertIn("-I -B -S", text)
        self.assertIn("--progress", deploy)
        submit = (ROOT / "benchmarks/scc/feppml_qualification_submit.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("record-submission", submit)
        self.assertIn("/usr/local/bin/qsub", submit)
        self.assertIn("-I -B -S", submit)
        run_root = SCC.PROJECT_ROOT / "runs/20260831T120000Z-q"
        setup_argv = SCC._qsub_argv(
            run_root, "/usr/local/bin/qsub", role="setup", slots=16,
            tasks=1, script=run_root / "setup.sh", hold=None,
        )
        collector_argv = SCC._qsub_argv(
            run_root, "/usr/local/bin/qsub", role="collector", slots=1,
            tasks=1, script=run_root / "collector.sh", hold="102",
        )
        for token in ("-clear", "-P", "-S", "-pe", "h_rt=11:45:00"):
            self.assertIn(token, setup_argv)
        self.assertNotIn("-pe", collector_argv)
        self.assertIn("h_rt=00:30:00", collector_argv)
        self.assertEqual(
            collector_argv[-3:],
            ["-v", f"RUN_ROOT={run_root}", str(run_root / "collector.sh")],
        )

    def test_qacct_requires_all_tasks_success_and_project(self) -> None:
        block = self.qacct_record("123", None)
        self.assertTrue(SCC._accounting_ok(block, 1, "123"))
        self.assertFalse(SCC._accounting_ok(block, 2, "123"))
        self.assertFalse(SCC._accounting_ok(block, 1, "124"))
        self.assertFalse(SCC._accounting_ok(block.replace("failed         0", "failed         1"), 1))
        self.assertFalse(SCC._accounting_ok(block.replace("welfgr", "default"), 1))
        self.assertFalse(SCC._accounting_ok(block.replace("johannes", "attacker"), 1))
        self.assertFalse(SCC._accounting_ok(block.replace("granted_pe     omp", "granted_pe     mpi"), 1))
        self.assertFalse(SCC._accounting_ok(block.replace("maxvmem        3.5G", ""), 1))

    def test_qacct_contract_rejects_substituted_job_and_slots(self) -> None:
        contract = {
            "role": "setup",
            "job_name": "fq_20260831T120000Z_deadbeef_a",
            "owner": "johannes",
            "project": "welfgr",
            "slots": 16,
            "cwd": "/projectnb/welfgr/fereg/runs/20260831T120000Z-q",
            "script": {"path": "input/harness/benchmarks/scc/job.sh"},
            "qsub_argv": ["qsub", "job.sh"],
        }
        valid = self.qacct_record(
            "456", None, job_name=contract["job_name"], slots=16
        )
        self.assertEqual(
            len(SCC._validated_accounting(valid, 1, "456", job_contract=contract)),
            1,
        )
        for mutated in (
            valid.replace(contract["job_name"], "fq_unrelated_a"),
            valid.replace("slots          16", "slots          8"),
            valid.replace("johannes", "attacker"),
        ):
            with self.assertRaises(SCC.SccError):
                SCC._validated_accounting(
                    mutated, 1, "456", job_contract=contract
                )
        scalar_array = valid + "\ntaskid        1"
        with self.assertRaisesRegex(SCC.SccError, "scalar"):
            SCC._validated_accounting(
                scalar_array, 1, "456", job_contract=contract
            )

    def test_real_bu_qstat_shapes_and_command_fields_are_exact(self) -> None:
        run_root = SCC.PROJECT_ROOT / "runs/20260831T120000Z-feppml-q"
        cases = (
            ("101", "setup", 1, None),
            ("102", "array", 33, "101"),
            ("103", "collector", 1, "102"),
        )
        for job_id, role, tasks, hold in cases:
            relative = {
                "setup": "benchmarks/scc/feppml_qualification_setup_job.sh",
                "array": "benchmarks/scc/feppml_qualification_job.sh",
                "collector": (
                    "benchmarks/scc/feppml_qualification_collector_job.sh"
                ),
            }[role]
            script = run_root / "input/harness" / relative
            text = self.qstat_record(
                job_id=job_id,
                role=role,
                run_root=run_root,
                script=script,
                tasks=tasks,
                hold=hold,
            )
            fields = SCC._verify_qstat_attestation(
                text,
                job_id=job_id,
                role=role,
                job_name=SCC._job_name(run_root, role),
                run_root=run_root,
                script=script,
                slots=1 if role == "collector" else 16,
                tasks=tasks,
                hold=hold,
            )
            self.assertEqual(fields["shell_list"], "NONE:/bin/bash")
            self.assertEqual(
                fields["stdout_path_list"], f"NONE:NONE:{run_root / 'logs'}"
            )

            mutations = (
                text.replace(str(script), str(script.with_name("substituted.sh"))),
                text.replace("NONE:/bin/bash", "NONE:/bin/bash-evil"),
                text.replace(
                    f"NONE:NONE:{run_root / 'logs'}",
                    f"NONE:NONE:{run_root / 'other-logs'}",
                ),
                text.replace("mem_per_core=4G", "mem_per_core=4G,gpu=1"),
                text.replace(
                    f"RUN_ROOT={run_root}", f"RUN_ROOT={run_root},EVIL=1"
                ),
            )
            for mutated in mutations:
                with self.assertRaises(SCC.SccError):
                    SCC._verify_qstat_attestation(
                        mutated,
                        job_id=job_id,
                        role=role,
                        job_name=SCC._job_name(run_root, role),
                        run_root=run_root,
                        script=script,
                        slots=1 if role == "collector" else 16,
                        tasks=tasks,
                        hold=hold,
                    )

    def test_real_bu_qacct_collector_shape_rejects_command_substitution(self) -> None:
        run_root = SCC.PROJECT_ROOT / "runs/20260831T120000Z-feppml-q"
        relative_script = (
            "input/harness/benchmarks/scc/"
            "feppml_qualification_collector_job.sh"
        )
        script = run_root / relative_script
        argv = [
            "/usr/local/bin/qsub", "-clear", str(script),
        ]
        contract = {
            "role": "collector",
            "job_name": "fq_20260831T120000Z_deadbeef_c",
            "owner": "johannes",
            "project": "welfgr",
            "slots": 1,
            "cwd": str(run_root),
            "script": {"path": relative_script},
            "qsub_argv": argv,
        }
        valid = self.qacct_record(
            "103",
            None,
            slots=1,
            job_name=contract["job_name"],
            granted_pe="NONE",
            extra_fields=(
                "taskid         undefined",
                f"cwd            {run_root}",
                f"scriptfile     {script}",
                "submit_cmdline /usr/local/bin/qsub -clear " + str(script),
            ),
        )
        self.assertEqual(
            SCC._validated_accounting(
                valid, 1, "103", job_contract=contract
            )[0]["granted_pe"],
            "NONE",
        )
        for mutated in (
            valid.replace("granted_pe     NONE", "granted_pe     omp"),
            valid.replace("slots          1", "slots          16"),
            valid.replace(str(script), str(script.with_name("substituted.sh"))),
            valid.replace(
                "submit_cmdline /usr/local/bin/qsub -clear",
                "submit_cmdline /usr/local/bin/qsub -terse",
            ),
            valid.replace("taskid         undefined", "taskid         1"),
        ):
            with self.assertRaises(SCC.SccError):
                SCC._validated_accounting(
                    mutated, 1, "103", job_contract=contract
                )

    def test_submission_is_sealed_before_release_and_setup_waits_for_receipt(self) -> None:
        source = MODULE_PATH.read_text(encoding="utf-8")
        start = source.index("def record_submission(")
        end = source.index("def _job_name(", start)
        body = source[start:end]
        seal = body.index("write_sealed(\n        submission_path")
        release = body.index("released = subprocess.run(")
        release_receipt = body.index(
            'run_root / "receipts/submission-release.json"'
        )
        self.assertLess(seal, release)
        self.assertLess(release, release_receipt)

        setup_start = source.index("def setup(")
        setup_end = source.index("def task(", setup_start)
        setup_source = source[setup_start:setup_end]
        self.assertIn('run_root / "receipts/submission-release.json"', setup_source)
        self.assertIn("time.monotonic() + 30.0", setup_source)
        self.assertLess(
            setup_source.index("while not release_path.is_file()"),
            setup_source.index("_verify_submission("),
        )

    def test_portable_submission_binds_original_exact_scc_root_and_local_scripts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_id = "20260831T120000Z-feppml-q"
            run_root = Path(temporary) / run_id
            preparation_path = run_root / "input/preparation.json"
            preparation_path.parent.mkdir(parents=True)
            preparation_path.write_text("{}\n", encoding="ascii")
            submitted_root = SCC.PROJECT_ROOT / "runs" / run_id
            jobs = {
                "setup_job": "101", "array_job": "102", "collector_job": "103"
            }
            clients = {
                role: dict(record)
                for role, record in SCC.SCHEDULER_CLIENT_CONTRACT.items()
            }
            roles = {
                "setup_job": (
                    "setup", 16, 1,
                    "benchmarks/scc/feppml_qualification_setup_job.sh", None,
                ),
                "array_job": (
                    "array", 16, 33,
                    "benchmarks/scc/feppml_qualification_job.sh", "101",
                ),
                "collector_job": (
                    "collector", 1, 1,
                    "benchmarks/scc/feppml_qualification_collector_job.sh", "102",
                ),
            }
            raw_root = run_root / "receipts/submission-records"
            raw_root.mkdir(parents=True)
            job_records = {}
            for field, (role, slots, tasks, relative, hold) in roles.items():
                local_script = run_root / "input/harness" / relative
                local_script.parent.mkdir(parents=True, exist_ok=True)
                local_script.write_text(f"#!/bin/sh\n# {role}\n", encoding="ascii")
                submitted_script = submitted_root / "input/harness" / relative
                argv = SCC._qsub_argv(
                    submitted_root,
                    str(clients["qsub"]["path"]),
                    role=role,
                    slots=slots,
                    tasks=tasks,
                    script=submitted_script,
                    hold=hold,
                )
                raw_qsub_output = SCC._expected_qsub_output(
                    jobs[field], role, tasks
                )
                qstat_text = self.qstat_record(
                    job_id=jobs[field],
                    role=role,
                    run_root=submitted_root,
                    script=submitted_script,
                    tasks=tasks,
                    hold=hold,
                )
                qsub_stdout = raw_root / f"{role}.qsub.stdout"
                qsub_stderr = raw_root / f"{role}.qsub.stderr"
                qstat_stdout = raw_root / f"{role}.qstat.stdout"
                qstat_stderr = raw_root / f"{role}.qstat.stderr"
                qsub_stdout.write_text(raw_qsub_output + "\n", encoding="ascii")
                qsub_stderr.write_bytes(b"")
                qstat_stdout.write_text(qstat_text, encoding="ascii")
                qstat_stderr.write_bytes(b"")
                job_records[field] = {
                    "role": role,
                    "job_id": jobs[field],
                    "raw_qsub_output": raw_qsub_output,
                    "job_name": SCC._job_name(run_root, role),
                    "owner": "johannes", "project": "welfgr",
                    "cwd": str(submitted_root), "slots": slots,
                    "task_count": tasks, "hold_job_id": hold,
                    "script": {
                        "path": local_script.relative_to(run_root).as_posix(),
                        "sha256": SCC.digest_file(local_script),
                        "size_bytes": local_script.stat().st_size,
                    },
                    "qsub_argv": argv,
                    "qsub_stdout": SCC._relative_file_record(
                        run_root, qsub_stdout
                    ),
                    "qsub_stderr": SCC._relative_file_record(
                        run_root, qsub_stderr
                    ),
                    "qstat_argv": [
                        str(clients["qstat"]["path"]), "-j", jobs[field]
                    ],
                    "qstat_stdout": SCC._relative_file_record(
                        run_root, qstat_stdout
                    ),
                    "qstat_stderr": SCC._relative_file_record(
                        run_root, qstat_stderr
                    ),
                    "qstat_fields": SCC._qstat_fields(qstat_text),
                }
            release_argv = [str(clients["qrls"]["path"]), jobs["setup_job"]]
            controller = {
                "role": "submission",
                "environment": {"RUN_ROOT": str(submitted_root)},
            }
            SCC.write_sealed(
                run_root / "receipts/submission.json",
                {
                    "schema_version": 1,
                    "kind": "feppml_qualification_scc_submission",
                    "run_id": run_id,
                    "preparation_sha256": SCC.digest_file(preparation_path),
                    **jobs,
                    "recorded_at_utc": "2026-08-31T12:03:00Z",
                    "scheduler_clients": clients,
                    "scheduler_environment": SCC._scheduler_environment(
                        submitted_root, clients
                    ),
                    "controller": controller,
                    "jobs": job_records,
                    "release_argv": release_argv,
                    "complete_sentinel": "FEPPML_SCC_SUBMISSION_COMPLETE",
                },
            )
            release_stdout = raw_root / "setup.qrls.stdout"
            release_stderr = raw_root / "setup.qrls.stderr"
            release_stdout.write_bytes(b"")
            release_stderr.write_bytes(b"")
            SCC.write_sealed(
                run_root / "receipts/submission-release.json",
                {
                    "schema_version": 1,
                    "kind": "feppml_qualification_scc_submission_release",
                    "run_id": run_id,
                    "submission_sha256": SCC.digest_file(
                        run_root / "receipts/submission.json"
                    ),
                    "command": release_argv,
                    "stdout": SCC._relative_file_record(run_root, release_stdout),
                    "stderr": SCC._relative_file_record(run_root, release_stderr),
                    "return_code": 0,
                    "complete_sentinel": (
                        "FEPPML_SCC_SUBMISSION_RELEASE_COMPLETE"
                    ),
                },
            )
            with mock.patch.object(
                SCC, "_verify_controller_identity", return_value=controller
            ):
                verified = SCC._verify_submission(
                    run_root, {"task_count": 33}, portable=True
                )
            self.assertEqual(verified["jobs"]["setup_job"]["cwd"], str(submitted_root))
            envelope = json.loads(
                (run_root / "receipts/submission.json").read_text(encoding="ascii")
            )
            envelope["payload"]["jobs"]["setup_job"]["cwd"] = (
                f"/tmp/{run_id}"
            )
            envelope["seal"] = SCC.qualification.seal_payload(
                envelope["payload"]
            )["seal"]
            (run_root / "receipts/submission.json").chmod(0o644)
            (run_root / "receipts/submission.json").write_text(
                json.dumps(envelope), encoding="ascii"
            )
            with mock.patch.object(
                SCC, "_verify_controller_identity", return_value=controller
            ), self.assertRaisesRegex(SCC.SccError, "inconsistent|exact SCC"):
                SCC._verify_submission(run_root, {"task_count": 33}, portable=True)

    def test_prepare_parser_closes_ppmlhdfe_to_archive_bytes(self) -> None:
        actions = SCC.parser()._subparsers._group_actions[0].choices["prepare"]._actions
        destinations = {action.dest for action in actions}
        self.assertIn("ppmlhdfe_archive", destinations)
        self.assertNotIn("ppmlhdfe_repo", destinations)

    def test_preprepare_generator_never_requires_or_embeds_runtime_plan(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            development_path = root / "development.json"
            development_path.write_text("{}\n", encoding="ascii")
            development_sha = SCC.digest_file(development_path)
            manifest_sha = "1" * 64
            manifest = {
                "manifest_id": "qualification",
                "matrix_contract": {
                    "development_manifest": {
                        "path": "development.json", "sha256": development_sha,
                    },
                    "expected_cell_ids": ["cell-a", "cell-b"],
                    "oracle_required_cell_ids": ["cell-a"],
                },
                "memory_gates": {"required_fixtures": ["fixture-a"]},
                "capture_contract": {"comparator_processor_limit": 16},
            }
            header = {
                "candidate": {}, "comparator": {}, "oracles": [],
                "source_audit": {}, "platform_artifacts": [],
            }
            template = SCC.spec_generator.generate_template(
                manifest,
                manifest_sha,
                header,
                stata_version="19.0 MP",
                rustc_version="unavailable until SCC setup",
            )
            request = SCC.spec_generator.materialization_request(
                manifest, manifest_sha, development_path
            )
            self.assertNotIn("preflight_program", template)
            self.assertNotIn("arm_program", template)
            self.assertNotIn("plan", template)
            self.assertEqual(request["cell_ids"], ["cell-a", "cell-b"])
            self.assertEqual(request["stata_batch_arguments"], ["-q", "-b", "do"])
            self.assertEqual(request["comparator_processor_limit"], 16)
            self.assertEqual(
                SCC._validate_capture_template(template, manifest, manifest_sha),
                template,
            )
            with self.assertRaisesRegex(SCC.SccError, "non-executable"):
                SCC._validate_capture_template(
                    {**template, "preflight_program": {"command": ["malicious"]}},
                    manifest,
                    manifest_sha,
                )

    def test_portable_spec_reopens_committed_program_and_plan_without_local_python(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_id = "20260831T120000Z-feppml-q"
            run_root = Path(temporary) / run_id
            program = run_root / "input/harness/scripts/feppml_qualification_capture.py"
            plan = run_root / "work/capture/plan.json"
            program.parent.mkdir(parents=True)
            plan.parent.mkdir(parents=True)
            program.write_bytes(b"# committed capture\n")
            plan.write_bytes(b"{}\n")
            recorded_root = SCC.PROJECT_ROOT / "runs" / run_id
            program_record = {
                "interpreter_path": "/share/pkg/python3/3.12.4/bin/python3",
                "interpreter_sha256": "a" * 64,
                "interpreter_version": "Python 3.12.4",
                "program_path": str(
                    recorded_root
                    / "input/harness/scripts/feppml_qualification_capture.py"
                ),
                "program_sha256": SCC.digest_file(program),
                "plan_path": str(recorded_root / "work/capture/plan.json"),
                "plan_sha256": SCC.digest_file(plan),
            }
            manifest = {
                "manifest_id": "qualification",
                "matrix_contract": {"expected_cell_ids": ["cell"]},
                "capture_contract": {"comparator_processor_limit": 16},
            }
            spec = {
                "schema_version": 1,
                "spec_type": "feppml-qualification-capture-v1",
                "qualification_manifest": {
                    "id": "qualification", "sha256": "b" * 64,
                },
                "preflight_program": program_record,
                "arm_program": dict(program_record),
                "evidence_header": {
                    "candidate": {}, "comparator": {}, "oracles": [],
                    "source_audit": {}, "platform_artifacts": [],
                },
                "environment": {
                    "node_id": "node", "stata_version": "19.0 MP",
                    "stata_flavor": "MP", "scheduler_job_id": "123",
                    "comparator_processor_limit": 16, "os": "Linux",
                    "cpu": "CPU", "rustc_version": "rustc 1.85.1",
                },
                "cell_ids": ["cell"],
                "complete_sentinel": "FEPPML_CAPTURE_SPEC_COMPLETE",
            }
            spec_path = run_root / "work/capture/spec.json"
            spec_path.write_text(json.dumps(spec), encoding="ascii")
            loaded = SCC.capture.load_spec(
                spec_path, manifest, "b" * 64, portable_run_root=run_root
            )
            self.assertEqual(
                loaded["preflight_program"]["interpreter_path"],
                program_record["interpreter_path"],
            )
            program.write_bytes(b"substituted\n")
            with self.assertRaisesRegex(SCC.capture.CaptureError, "relocated program"):
                SCC.capture.load_spec(
                    spec_path, manifest, "b" * 64,
                    portable_run_root=run_root,
                )

    def test_runtime_bundles_are_exact_and_comparator_artifact_resolves_adopath(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_root = Path(temporary)
            source_stata = run_root / "work/candidate/stata"
            source_stata.mkdir(parents=True)
            (source_stata / "fereg.ado").write_bytes(b"candidate-ado")
            dist = run_root / "work/candidate/dist"
            dist.mkdir(parents=True)
            plugin = dist / "fereg_linux.plugin"
            plugin.write_bytes(b"plugin")
            plugin.chmod(0o555)
            candidate = run_root / "work/bundles/candidate/stata"
            candidate.mkdir(parents=True)
            (candidate / "fereg.ado").write_bytes(b"candidate-ado")
            target_plugin = candidate / plugin.name
            target_plugin.write_bytes(plugin.read_bytes())
            target_plugin.chmod(0o555)

            comparator = run_root / "work/bundles/comparator"
            references = {}
            for name in SCC.REFERENCE_NAMES:
                path = comparator / name / "src" / f"{name}.ado"
                path.parent.mkdir(parents=True)
                path.write_bytes(name.encode("ascii"))
                path.chmod(0o444)
                references[name] = {
                    "runtime_files": {
                        f"src/{name}.ado": {
                            "sha256": SCC.digest_file(path),
                            "size_bytes": path.stat().st_size,
                            "executable": False,
                        }
                    }
                }
            generated_mlib = comparator / "ftools/src/l/lftools.mlib"
            generated_mlib.parent.mkdir(parents=True)
            generated_mlib.write_bytes(b"controlled-licensed-stata-mlib")
            generated_mlib.chmod(0o444)
            artifact = SCC._write_comparator_artifact(
                comparator, comparator / "feppml-comparator-bundle.tar"
            )
            self.assertEqual(artifact.parent, comparator)
            for name in SCC.REFERENCE_NAMES:
                self.assertTrue((artifact.parent / name / "src").is_dir())
            preparation = {"references": references}
            SCC._verify_runtime_bundles(run_root, preparation)

            candidate_ado = candidate / "fereg.ado"
            candidate_ado.write_bytes(b"substituted")
            with self.assertRaisesRegex(SCC.SccError, "candidate runtime"):
                SCC._verify_runtime_bundles(run_root, preparation)
            candidate_ado.write_bytes(b"candidate-ado")
            (comparator / "ppmlhdfe/src/shadow.ado").write_bytes(b"shadow")
            with self.assertRaisesRegex(SCC.SccError, "identity artifact|runtime"):
                SCC._verify_runtime_bundles(run_root, preparation)

    def test_qacct_array_inventory_and_node_binding_are_exact(self) -> None:
        valid = "\n".join(
            self.qacct_record("456", task_id, f"node-{task_id}")
            for task_id in range(1, 4)
        )
        records = SCC._validated_accounting(valid, 3, "456")
        self.assertEqual([int(record["taskid"]) for record in records], [1, 2, 3])
        self.assertEqual(SCC._node(records[0]["hostname"]), "node-1")
        summary = SCC._accounting_summary(records[0])
        self.assertEqual(summary["job_id"], "456")
        self.assertEqual(summary["task_id"], 1)
        self.assertEqual(summary["slots"], 16)

        duplicate = "\n".join(
            [self.qacct_record("456", 1), self.qacct_record("456", 1), self.qacct_record("456", 3)]
        )
        self.assertFalse(SCC._accounting_ok(duplicate, 3, "456"))
        wrong_node_source = MODULE_PATH.read_text(encoding="utf-8")
        for token in (
            'setup.get("job_id") != submission["setup_job"]',
            'collection.get("job_id") != submission["collector_job"]',
            'shard.get("job_id") != submission["array_job"]',
            '_node(shard.get("node_id")) != _node(record["hostname"])',
        ):
            self.assertIn(token, wrong_node_source)

    def test_qacct_retry_is_bounded_and_waits_for_complete_inventory(self) -> None:
        valid = "\n".join(
            self.qacct_record("456", task_id) for task_id in range(1, 3)
        )
        run_root = Path("/tmp/run")
        clients = {"qacct": {"path": "/usr/local/bin/qacct"}}
        environment = {
            "LANG": "C",
            "HOME": str(run_root / "work/submission/home"),
            "TMPDIR": str(run_root / "work/submission/tmp"),
        }

        def attempt(stdout: str) -> dict:
            raw = stdout.encode("utf-8")
            return {
                "schema_version": 1,
                "record_type": "feppml-sge-qacct-attempt-v1",
                "job_id": "456",
                "argv": ["/usr/local/bin/qacct", "-j", "456"],
                "cwd": str(run_root),
                "environment": environment,
                "client": clients["qacct"],
                "timeout_seconds": SCC.QACCT_TIMEOUT_SECONDS,
                "timed_out": False,
                "return_code": 0,
                "stdout_base64": SCC.base64.b64encode(raw).decode("ascii"),
                "stdout_sha256": SCC.digest_bytes(raw),
                "stderr_base64": "",
                "stderr_sha256": SCC.digest_bytes(b""),
                "complete_sentinel": "FEPPML_SGE_QACCT_ATTEMPT_COMPLETE",
            }

        with mock.patch.object(
            SCC, "_qacct_attempt",
            side_effect=[attempt(""), attempt(self.qacct_record("456", 1)), attempt(valid)],
        ) as qacct, mock.patch.object(SCC.time, "sleep") as sleep:
            text, records, attempts = SCC._qacct_bounded(
                "456", 2, run_root=run_root, clients=clients,
                environment=environment, attempts=3, delay_seconds=0.01
            )
        self.assertEqual(text, valid)
        self.assertEqual([record["taskid"] for record in records], ["1", "2"])
        self.assertEqual(len(attempts), 3)
        self.assertEqual(qacct.call_count, 3)
        self.assertEqual(sleep.call_count, 2)
        with mock.patch.object(SCC, "_qacct_attempt", return_value=attempt("")), mock.patch.object(
            SCC.time, "sleep"
        ):
            with self.assertRaisesRegex(SCC.SccError, "after 2 attempts"):
                SCC._qacct_bounded(
                    "456", 2, run_root=run_root, clients=clients,
                    environment=environment, attempts=2, delay_seconds=0,
                )

    def test_qacct_attempt_binds_raw_streams_timeout_cwd_env_and_client(self) -> None:
        run_root = Path("/projectnb/welfgr/fereg/runs/20260831T120000Z-feppml-q")
        clients = {"qacct": dict(SCC.SCHEDULER_CLIENT_CONTRACT["qacct"])}
        environment = SCC._scheduler_environment(run_root, {
            **{name: dict(value) for name, value in SCC.SCHEDULER_CLIENT_CONTRACT.items()}
        })
        completed = subprocess.CompletedProcess(
            [clients["qacct"]["path"], "-j", "456"], 0,
            stdout=b"exact\r\nraw\n", stderr=b"",
        )
        with mock.patch.object(SCC.subprocess, "run", return_value=completed) as run:
            attempt = SCC._qacct_attempt(
                "456", run_root=run_root, clients=clients,
                environment=environment,
            )
        self.assertEqual(
            SCC.base64.b64decode(attempt["stdout_base64"]), b"exact\r\nraw\n"
        )
        self.assertEqual(run.call_args.kwargs["timeout"], SCC.QACCT_TIMEOUT_SECONDS)
        SCC._verify_qacct_attempt(
            run_root, attempt, "456", clients, environment
        )
        for field, replacement in (
            ("argv", [clients["qacct"]["path"], "-j", "999"]),
            ("cwd", "/another/root/20260831T120000Z-feppml-q"),
            ("environment", {**environment, "PYTHONPATH": "/hostile"}),
            ("client", {**clients["qacct"], "sha256": "0" * 64}),
            ("stdout_sha256", "0" * 64),
            ("timeout_seconds", 0.0),
        ):
            mutated = {**attempt, field: replacement}
            with self.assertRaisesRegex(SCC.SccError, "qacct"):
                SCC._verify_qacct_attempt(
                    run_root, mutated, "456", clients, environment
                )

        timeout = subprocess.TimeoutExpired(
            [clients["qacct"]["path"], "-j", "456"],
            SCC.QACCT_TIMEOUT_SECONDS,
            output=b"partial",
            stderr=b"blocked",
        )
        with mock.patch.object(SCC.subprocess, "run", side_effect=timeout):
            timed = SCC._qacct_attempt(
                "456", run_root=run_root, clients=clients,
                environment=environment,
            )
        self.assertTrue(timed["timed_out"])
        self.assertIsNone(timed["return_code"])
        SCC._verify_qacct_attempt(
            run_root, timed, "456", clients, environment
        )

    def test_qacct_query_receipt_replays_exact_three_job_supervisors(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_id = "20260831T120000Z-feppml-qacct"
            run_root = Path(temporary) / run_id
            recorded_root = SCC.PROJECT_ROOT / "runs" / run_id
            clients = {
                name: dict(value)
                for name, value in SCC.SCHEDULER_CLIENT_CONTRACT.items()
            }
            environment = SCC._scheduler_environment(recorded_root, clients)
            jobs = {
                "setup_job": "101",
                "array_job": "102",
                "collector_job": "103",
            }
            roles = {
                "setup_job": ("setup", 16, 1, "fq_setup", "omp"),
                "array_job": ("array", 16, 33, "fq_array", "omp"),
                "collector_job": ("collector", 1, 1, "fq_collector", "NONE"),
            }
            contracts = {}
            accounting_text = {}
            query_records = {}
            for name, (role, slots, count, job_name, pe) in roles.items():
                job_id = jobs[name]
                contracts[name] = {
                    "role": role,
                    "job_name": job_name,
                    "owner": "johannes",
                    "project": "welfgr",
                    "slots": slots,
                    "task_count": count,
                    "cwd": str(recorded_root),
                    "script": {"path": f"input/harness/{role}.sh"},
                    "qsub_argv": [clients["qsub"]["path"], "-clear"],
                }
                records = (
                    [
                        self.qacct_record(
                            job_id, task_id, slots=slots,
                            job_name=job_name, granted_pe=pe,
                        )
                        for task_id in range(1, count + 1)
                    ]
                    if role == "array"
                    else [
                        self.qacct_record(
                            job_id, None, slots=slots,
                            job_name=job_name, granted_pe=pe,
                        )
                    ]
                )
                text = "\n".join(records)
                accounting_text[name] = text
                raw = text.encode("utf-8")
                query_records[name] = {
                    "job_id": job_id,
                    "accepted_attempt": 1,
                    "attempts": [{
                        "schema_version": 1,
                        "record_type": "feppml-sge-qacct-attempt-v1",
                        "job_id": job_id,
                        "argv": [clients["qacct"]["path"], "-j", job_id],
                        "cwd": str(recorded_root),
                        "environment": environment,
                        "client": clients["qacct"],
                        "timeout_seconds": SCC.QACCT_TIMEOUT_SECONDS,
                        "timed_out": False,
                        "return_code": 0,
                        "stdout_base64": SCC.base64.b64encode(raw).decode("ascii"),
                        "stdout_sha256": SCC.digest_bytes(raw),
                        "stderr_base64": "",
                        "stderr_sha256": SCC.digest_bytes(b""),
                        "complete_sentinel": "FEPPML_SGE_QACCT_ATTEMPT_COMPLETE",
                    }],
                }
            submission = {
                **jobs,
                "jobs": contracts,
            }
            SCC.write_sealed(
                run_root / "receipts/qacct/queries.json",
                {
                    "schema_version": 1,
                    "kind": "feppml_qualification_scc_qacct_queries",
                    "run_id": run_id,
                    "scheduler_clients": clients,
                    "scheduler_environment": environment,
                    "queries": query_records,
                    "complete_sentinel": "FEPPML_SGE_QACCT_QUERIES_COMPLETE",
                },
            )
            verified = SCC._verify_qacct_queries(
                run_root, submission, clients, environment, accounting_text
            )
            self.assertEqual(set(verified["queries"]), set(jobs))

    def test_qacct_time_rejects_ambiguous_dst_rollback_hour(self) -> None:
        with self.assertRaisesRegex(SCC.SccError, "ambiguous DST"):
            SCC._qacct_time("Sun Nov 01 01:30:00 2026")
        self.assertLess(
            SCC._qacct_time("Sun Nov 01 00:59:59 2026"),
            SCC._qacct_time("Sun Nov 01 02:00:00 2026"),
        )

    def test_controller_environment_is_role_exact_and_cross_bound(self) -> None:
        run_root = SCC.PROJECT_ROOT / "runs/20260831T120000Z-feppml-q"

        def record(role: str, environment: dict) -> dict:
            return {
                "schema_version": 1,
                "record_type": "feppml-isolated-controller-v1",
                "role": role,
                "executable": dict(SCC.EXPECTED_CONTROLLER_PYTHON),
                "flags": {
                    "isolated": 1,
                    "ignore_environment": 1,
                    "no_site": 1,
                    "dont_write_bytecode": 1,
                    "safe_path": True,
                },
                "environment": environment,
                "environment_sha256": SCC.digest_bytes(SCC.canonical(environment)),
                "complete_sentinel": "FEPPML_ISOLATED_CONTROLLER_COMPLETE",
            }

        python_dir = str(Path(SCC.EXPECTED_CONTROLLER_PYTHON["path"]).parent)
        submission_environment = {
            "HOME": str(run_root / "work/submission-controller/home"),
            "TMPDIR": str(run_root / "work/submission-controller/tmp"),
            "PATH": f"{python_dir}:/usr/local/bin:/usr/bin:/bin",
            "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
            "RUN_ROOT": str(run_root), "USER": "johannes", "LOGNAME": "johannes",
            "SGE_ROOT": "/usr/local/sge/sge_root", "SGE_CELL": "default",
            "FEPPML_CONTROLLER_PYTHON": SCC.EXPECTED_CONTROLLER_PYTHON["path"],
            **{
                SCC.SCHEDULER_CLIENT_ENV[name]: value["path"]
                for name, value in SCC.SCHEDULER_CLIENT_CONTRACT.items()
            },
        }
        SCC._verify_controller_identity(
            run_root, record("submission", submission_environment),
            "submission", portable=True,
        )
        for field, replacement in (
            ("PATH", f"{python_dir}:/tmp/shadow:/usr/bin:/bin"),
            ("USER", "attacker"),
            ("SGE_ROOT", "/tmp/sge"),
            ("FEPPML_QSUB_EXECUTABLE", "/tmp/qsub"),
        ):
            mutated_environment = {**submission_environment, field: replacement}
            with self.assertRaisesRegex(SCC.SccError, "controller"):
                SCC._verify_controller_identity(
                    run_root, record("submission", mutated_environment),
                    "submission", portable=True,
                )

        stata = "/share/pkg/stata/19/stata-mp"
        setup_environment = {
            "HOME": str(run_root / "work/controller/setup/home"),
            "TMPDIR": str(run_root / "work/controller/setup/tmp"),
            "PATH": f"{python_dir}:{Path(stata).parent}:/usr/bin:/bin",
            "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
            "RUN_ROOT": str(run_root), "JOB_ID": "101", "NSLOTS": "16",
            "FEPPML_STATA_EXECUTABLE": stata,
            "FEPPML_CONTROLLER_PYTHON": SCC.EXPECTED_CONTROLLER_PYTHON["path"],
        }
        SCC._verify_controller_identity(
            run_root, record("setup", setup_environment), "setup", portable=True
        )
        for field, replacement in (
            ("PATH", f"{python_dir}:/tmp/stata:/usr/bin:/bin"),
            ("FEPPML_STATA_EXECUTABLE", "/tmp/stata-mp"),
            ("NSLOTS", "1"),
        ):
            mutated_environment = {**setup_environment, field: replacement}
            with self.assertRaisesRegex(SCC.SccError, "controller"):
                SCC._verify_controller_identity(
                    run_root, record("setup", mutated_environment),
                    "setup", portable=True,
                )

        source = MODULE_PATH.read_text(encoding="utf-8")
        for token in (
            'setup_environment["JOB_ID"] != submission["setup_job"]',
            'setup_environment["FEPPML_STATA_EXECUTABLE"]',
            'setup_environment["PATH"] != expected_setup_path',
            'task_controller["environment"]["PATH"]',
        ):
            self.assertIn(token, source)

    def test_setup_stata_wrapper_prevents_long_sentinel_wrapping(self) -> None:
        action_id = "dataset-dev_n100k_disconnected_illcond_c5_n100k"
        wrapper = SCC._setup_stata_wrapper(action_id, ["display as text \"body\""]).decode(
            "ascii"
        )
        lines = wrapper.splitlines()
        self.assertEqual(lines[:4], [
            "version 19.0", "clear all", "set more off", "set linesize 255",
        ])
        sentinel = next(
            line for line in lines
            if "FEPPML_SETUP_ACTION_COMPLETE" in line
        )
        self.assertGreater(len(sentinel), 80)
        self.assertLess(len(sentinel), 255)

    def test_tasks_verify_materializer_receipt_before_timing(self) -> None:
        source = MODULE_PATH.read_text(encoding="utf-8")
        task_start = source.index("def task(")
        capture_start = source.index("capture._capture_pair(", task_start)
        verification = source.index("_verify_materialized_capture(", task_start)
        self.assertLess(verification, capture_start)
        for token in (
            'materializer.verify_receipt(plan_path, receipt_path)',
            'run_root / "work/capture/plan.json"',
            'run_root / "work/capture/spec.json"',
            'run_root / "receipts/capture-plan.json"',
        ):
            self.assertIn(token, source)

    def test_sealed_task_receipts_are_tamper_evident_and_exclusive(self) -> None:
        payload = {"kind": "feppml_qualification_cell_shard", "cell_id": "cell"}
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "task.json"
            SCC.write_sealed(path, payload)
            self.assertEqual(SCC.read_sealed(path, payload["kind"]), payload)
            with self.assertRaisesRegex(SCC.SccError, "overwrite"):
                SCC.write_sealed(path, payload)
            document = json.loads(path.read_text(encoding="utf-8"))
            document["payload"]["cell_id"] = "changed"
            path.chmod(0o644)
            path.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(SCC.SccError, "digest mismatch"):
                SCC.read_sealed(path, payload["kind"])

    def test_exact_git_archive_identity_changes_with_committed_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            subprocess.run(["git", "init", "-q", str(repo)], check=True)
            subprocess.run(["git", "-C", str(repo), "config", "user.email", "q@example.invalid"], check=True)
            subprocess.run(["git", "-C", str(repo), "config", "user.name", "Q"], check=True)
            (repo / "file.txt").write_text("one\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            subprocess.run(["git", "-C", str(repo), "commit", "-qm", "one"], check=True)
            first = subprocess.run(
                ["git", "-C", str(repo), "rev-parse", "HEAD"], check=True,
                stdout=subprocess.PIPE, text=True,
            ).stdout.strip()
            first_archive = SCC._archive(repo, first)
            first_identity = SCC._source_record(repo, first, first_archive)
            self.assertEqual(first_identity["commit"], first)
            self.assertEqual(first_identity["archive_sha256"], SCC.digest_bytes(first_archive))

            (repo / "file.txt").write_text("two\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            subprocess.run(["git", "-C", str(repo), "commit", "-qm", "two"], check=True)
            second = subprocess.run(
                ["git", "-C", str(repo), "rev-parse", "HEAD"], check=True,
                stdout=subprocess.PIPE, text=True,
            ).stdout.strip()
            second_identity = SCC._source_record(repo, second, SCC._archive(repo, second))
            self.assertNotEqual(first_identity["tree"], second_identity["tree"])
            self.assertNotEqual(first_identity["archive_sha256"], second_identity["archive_sha256"])


if __name__ == "__main__":
    unittest.main()
