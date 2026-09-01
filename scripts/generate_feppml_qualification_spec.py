#!/usr/bin/env python3
"""Generate pre-prepare inputs or a concrete feppml capture specification.

The SCC lifecycle uses ``--template`` mode before any build product, dataset,
recipe, or Stata executable exists.  It emits a deliberately non-executable
template plus the exact materialization request.  SCC setup later constructs
the only executable spec from committed harness bytes and the sealed plan.

The legacy concrete mode remains useful for local qualification experiments,
but it requires an already materialized ``--plan`` and is never accepted by
SCC preparation.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
from typing import Any, Mapping, Sequence

try:
    from scripts import feppml_qualification as qualification
    from scripts import feppml_qualification_capture as capture
except ImportError:
    import feppml_qualification as qualification  # type: ignore[no-redef]
    import feppml_qualification_capture as capture  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
CAPTURE_PROGRAM = ROOT / "scripts/feppml_qualification_capture.py"
TEMPLATE_TYPE = "feppml-qualification-capture-template-v1"
TEMPLATE_SENTINEL = "FEPPML_CAPTURE_SPEC_TEMPLATE_COMPLETE"
REQUEST_TYPE = "feppml-capture-materialization-request-v1"
REQUEST_SENTINEL = "FEPPML_CAPTURE_MATERIALIZATION_REQUEST_COMPLETE"


class SpecFailure(RuntimeError):
    pass


def _read(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SpecFailure(f"cannot read {where}: {error}") from error
    if not isinstance(value, Mapping):
        raise SpecFailure(f"{where} must be an object")
    return value


def _write_new(path: Path, value: Mapping[str, Any]) -> None:
    data = json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise SpecFailure(f"refusing to overwrite spec: {path}") from error
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def _closed_header(path: Path) -> Mapping[str, Any]:
    header = _read(path, "evidence header")
    if set(header) != {
        "candidate", "comparator", "oracles", "source_audit",
        "platform_artifacts",
    }:
        raise SpecFailure("evidence header is not closed")
    return header


def materialization_request(
    manifest: Mapping[str, Any],
    manifest_sha: str,
    development_manifest: Path,
) -> Mapping[str, Any]:
    development_bytes = development_manifest.read_bytes()
    expected_development = manifest["matrix_contract"]["development_manifest"]
    if (
        development_manifest.name != Path(expected_development["path"]).name
        or qualification.sha256_bytes(development_bytes)
        != expected_development["sha256"]
    ):
        raise SpecFailure(
            "development manifest differs from the qualification contract"
        )
    fixtures = manifest["memory_gates"]["required_fixtures"]
    fixture_ids = [
        record["id"] if isinstance(record, Mapping) else record
        for record in fixtures
    ]
    return {
        "schema_version": 1,
        "request_type": REQUEST_TYPE,
        "qualification_manifest_sha256": manifest_sha,
        "development_manifest_sha256": qualification.sha256_bytes(
            development_bytes
        ),
        "cell_ids": list(manifest["matrix_contract"]["expected_cell_ids"]),
        "oracle_cell_ids": list(
            manifest["matrix_contract"]["oracle_required_cell_ids"]
        ),
        "constrained_fixture_ids": fixture_ids,
        "stata_batch_arguments": ["-q", "-b", "do"],
        "comparator_processor_limit": manifest["capture_contract"][
            "comparator_processor_limit"
        ],
        "complete_sentinel": REQUEST_SENTINEL,
    }


def generate_template(
    manifest: Mapping[str, Any],
    manifest_sha: str,
    header: Mapping[str, Any],
    *,
    stata_version: str,
    rustc_version: str,
) -> Mapping[str, Any]:
    if not stata_version.strip():
        raise SpecFailure("template Stata version must be nonempty")
    if not rustc_version.strip():
        raise SpecFailure("template rustc version must be nonempty")
    return {
        "schema_version": 1,
        "spec_type": TEMPLATE_TYPE,
        "qualification_manifest": {
            "id": manifest["manifest_id"], "sha256": manifest_sha,
        },
        "evidence_header": dict(header),
        "environment": {
            "stata_version": stata_version,
            "rustc_version": rustc_version,
        },
        "cell_ids": list(manifest["matrix_contract"]["expected_cell_ids"]),
        "complete_sentinel": TEMPLATE_SENTINEL,
    }


def generate(args: argparse.Namespace) -> Mapping[str, Any]:
    manifest = qualification.load_manifest(args.qualification_manifest)
    manifest_sha = qualification.sha256_file(args.qualification_manifest)
    plan = capture.load_plan(args.plan)
    if plan["qualification_manifest_sha256"] != manifest_sha:
        raise SpecFailure("capture plan is bound to another qualification manifest")
    expected = manifest["matrix_contract"]["expected_cell_ids"]
    observed = [cell["cell_id"] for cell in plan["cells"]]
    if observed != expected:
        raise SpecFailure("capture plan cell inventory/order differs from frozen qualification")
    expected_fixtures = {item["id"] for item in manifest["memory_gates"]["required_fixtures"]} if isinstance(manifest["memory_gates"]["required_fixtures"][0], Mapping) else set(manifest["memory_gates"]["required_fixtures"])
    observed_fixtures = {item["id"] for item in plan["constrained_memory"]}
    if observed_fixtures != expected_fixtures:
        raise SpecFailure("capture plan constrained-memory fixture inventory is incomplete")
    processor_limit = manifest["capture_contract"]["comparator_processor_limit"]
    if (
        args.comparator_processor_limit != processor_limit
        or plan["stata"]["comparator_processor_limit"] != processor_limit
    ):
        raise SpecFailure("comparator processor limit differs from the frozen contract")
    header = _closed_header(args.evidence_header)
    program = CAPTURE_PROGRAM.resolve()
    runtime_program = Path(args.runtime_program_path).resolve() if args.runtime_program_path else program
    plan_path = Path(args.runtime_plan_path).resolve() if args.runtime_plan_path else args.plan.resolve()
    interpreter_source = shutil.which(args.python) if not Path(args.python).is_absolute() else args.python
    if interpreter_source is None:
        raise SpecFailure("capture Python interpreter is unavailable")
    interpreter = Path(interpreter_source).resolve()
    completed = subprocess.run(
        [str(interpreter), "--version"], stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, text=True, check=False,
    )
    if completed.returncode != 0:
        raise SpecFailure("capture Python interpreter version is unavailable")
    identity = {
        "interpreter_path": str(interpreter),
        "interpreter_sha256": qualification.sha256_file(interpreter),
        "interpreter_version": completed.stdout.strip(),
        "program_path": str(runtime_program),
        "program_sha256": qualification.sha256_file(program),
        "plan_path": str(plan_path),
        "plan_sha256": qualification.sha256_file(args.plan),
    }
    return {
        "schema_version": 1,
        "spec_type": "feppml-qualification-capture-v1",
        "qualification_manifest": {"id": manifest["manifest_id"], "sha256": manifest_sha},
        "preflight_program": identity,
        "arm_program": dict(identity),
        "evidence_header": dict(header),
        "environment": {
            "node_id": args.node_id or socket.gethostname(),
            "stata_version": args.stata_version,
            "stata_flavor": args.stata_flavor,
            "scheduler_job_id": args.scheduler_job_id,
            "comparator_processor_limit": args.comparator_processor_limit,
            "os": args.os,
            "cpu": args.cpu,
            "rustc_version": args.rustc_version,
        },
        "cell_ids": expected,
        "complete_sentinel": "FEPPML_CAPTURE_SPEC_COMPLETE",
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--qualification-manifest", type=Path, required=True)
    result.add_argument(
        "--template", action="store_true",
        help="emit a non-executable SCC template and materialization request",
    )
    result.add_argument("--plan", type=Path)
    result.add_argument("--development-manifest", type=Path)
    result.add_argument("--evidence-header", type=Path, required=True)
    result.add_argument("--output", type=Path, required=True)
    result.add_argument("--output-request", type=Path)
    result.add_argument("--python", default="python3")
    result.add_argument("--runtime-program-path")
    result.add_argument("--runtime-plan-path")
    result.add_argument("--node-id")
    result.add_argument("--stata-version", required=True)
    result.add_argument("--stata-flavor")
    result.add_argument("--os")
    result.add_argument("--cpu")
    result.add_argument("--rustc-version", default="unavailable until SCC setup")
    result.add_argument("--scheduler-job-id")
    result.add_argument("--comparator-processor-limit", type=int)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        manifest = qualification.load_manifest(args.qualification_manifest)
        manifest_sha = qualification.sha256_file(args.qualification_manifest)
        if args.template:
            if args.plan is not None:
                raise SpecFailure("template generation cannot consume an executable plan")
            if args.development_manifest is None or args.output_request is None:
                raise SpecFailure(
                    "template generation requires --development-manifest and --output-request"
                )
            if any(
                value is not None
                for value in (
                    args.runtime_program_path, args.runtime_plan_path,
                    args.stata_flavor, args.os, args.cpu,
                    args.scheduler_job_id, args.comparator_processor_limit,
                )
            ):
                raise SpecFailure(
                    "template generation rejects runtime executable/environment arguments"
                )
            header = _closed_header(args.evidence_header)
            _write_new(
                args.output.resolve(),
                generate_template(
                    manifest, manifest_sha, header,
                    stata_version=args.stata_version,
                    rustc_version=args.rustc_version,
                ),
            )
            _write_new(
                args.output_request.resolve(),
                materialization_request(
                    manifest, manifest_sha, args.development_manifest.resolve()
                ),
            )
        else:
            required = {
                "plan": args.plan,
                "stata_flavor": args.stata_flavor,
                "os": args.os,
                "cpu": args.cpu,
                "scheduler_job_id": args.scheduler_job_id,
                "comparator_processor_limit": args.comparator_processor_limit,
            }
            missing = sorted(name for name, value in required.items() if value is None)
            if missing:
                raise SpecFailure(
                    f"concrete generation omits required arguments: {missing}"
                )
            if args.output_request is not None or args.development_manifest is not None:
                raise SpecFailure("concrete generation rejects template-only arguments")
            if args.comparator_processor_limit != 16:
                raise SpecFailure("comparator processor limit must be exactly 16")
            _write_new(args.output.resolve(), generate(args))
    except (SpecFailure, capture.CaptureFailure, qualification.QualificationError) as error:
        raise SystemExit(f"feppml capture spec generation failed closed: {error}") from error
    print("FEPPML_CAPTURE_SPEC_GENERATED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
