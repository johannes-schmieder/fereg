#!/usr/bin/env python3
"""Run the closed local feppml qualification capture protocol.

The estimator-specific capture program is deliberately separate from this
orchestrator.  It must implement two machine interfaces:

* ``--qualification-preflight ... --output PATH`` writes a complete
  science-preflight envelope containing development benchmark evidence whose
  timing fields are declared placeholders; and
* ``--qualification-arm ... --output PATH`` runs exactly one estimator arm in
  one fresh process and writes one closed measurement record.

This script verifies the scientific preflight before it starts the first timed
arm, schedules one cold plus nine warm AB/BA pairs, binds raw measurements back
into the scientific evidence, invokes the frozen qualification evaluator, and
writes immutable evidence and receipt files.  It does not invoke a shell.
"""
from __future__ import annotations

import argparse
import copy
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any, Mapping, Sequence

try:
    from scripts import feppml_qualification as qualification
    from scripts import ppml_memory_performance as development
except ImportError:  # direct ``python scripts/run_feppml_qualification.py``
    import feppml_qualification as qualification  # type: ignore[no-redef]
    import ppml_memory_performance as development  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_QUALIFICATION_MANIFEST = (
    ROOT / "benchmarks/manifests/feppml_qualification_v1.json"
)
DEFAULT_DEVELOPMENT_MANIFEST = (
    ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json"
)
CAPTURE_SPEC_TYPE = "feppml-qualification-capture-v1"
PREFLIGHT_TYPE = "feppml-science-preflight-v1"
ARM_TYPE = "feppml-qualification-arm-v1"
PYTHON_ISOLATION_FLAGS = ("-I", "-B", "-S")


class CaptureError(RuntimeError):
    """A capture input, process output, or ordering invariant failed closed."""


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise CaptureError(f"{where} must be an object")
    return value


def _keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise CaptureError(
            f"{where} fields are not closed; "
            f"missing={sorted(expected - set(value))}, extra={sorted(set(value) - expected)}"
        )


def _json(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CaptureError(f"cannot read {where}: {error}") from error
    return _mapping(value, where)


def _write_new(path: Path, value: Mapping[str, Any]) -> None:
    data = json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True).encode("ascii") + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise CaptureError(f"refusing to overwrite immutable output: {path}") from error
    with os.fdopen(descriptor, "wb") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def _portable_run_file(
    recorded: Any, run_root: Path, relative: str, where: str
) -> Path:
    source = Path(str(recorded))
    suffix = Path(relative).parts
    if (
        not source.is_absolute()
        or len(source.parts) <= len(suffix)
        or source.parts[-len(suffix):] != suffix
        or source.parts[-len(suffix) - 1] != run_root.name
    ):
        raise CaptureError(f"{where} is not scoped to the sealed run")
    path = (run_root / relative).resolve()
    try:
        path.relative_to(run_root.resolve())
    except ValueError as error:
        raise CaptureError(f"{where} escapes the relocated run") from error
    return path


def _program(
    value: Any,
    where: str,
    *,
    portable_run_root: Path | None = None,
) -> dict[str, Any]:
    record = dict(_mapping(value, where))
    _keys(
        record,
        {
            "interpreter_path", "interpreter_sha256", "interpreter_version",
            "program_path", "program_sha256", "plan_path", "plan_sha256",
        },
        where,
    )
    if portable_run_root is not None:
        interpreter = Path(record["interpreter_path"])
        program = _portable_run_file(
            record["program_path"], portable_run_root,
            "input/harness/scripts/feppml_qualification_capture.py",
            f"{where}.program_path",
        )
        plan = _portable_run_file(
            record["plan_path"], portable_run_root,
            "work/capture/plan.json", f"{where}.plan_path",
        )
        if (
            not interpreter.is_absolute()
            or not str(record["interpreter_version"]).startswith("Python 3.")
            or any(
                not isinstance(record[field], str)
                or len(record[field]) != 64
                or any(character not in "0123456789abcdef" for character in record[field])
                for field in (
                    "interpreter_sha256", "program_sha256", "plan_sha256"
                )
            )
        ):
            raise CaptureError(f"{where} portable interpreter identity is invalid")
        for path, digest_field, label in (
            (program, "program_sha256", "program"),
            (plan, "plan_sha256", "plan"),
        ):
            if not path.is_file() or qualification.sha256_file(path) != record[digest_field]:
                raise CaptureError(
                    f"{where} relocated {label} bytes differ from the capture spec"
                )
        # Preserve original SCC paths.  Retained raw supervisors bind the
        # exact recorded argv, not the collector's relocation path.
        return record

    interpreter = Path(record["interpreter_path"]).resolve()
    program = Path(record["program_path"]).resolve()
    plan = Path(record["plan_path"]).resolve()
    for path, digest_field, label in (
        (interpreter, "interpreter_sha256", "interpreter"),
        (program, "program_sha256", "program"),
        (plan, "plan_sha256", "plan"),
    ):
        if not path.is_file() or qualification.sha256_file(path) != record[digest_field]:
            raise CaptureError(f"{where} {label} bytes differ from the capture spec")
    completed = subprocess.run(
        [str(interpreter), *PYTHON_ISOLATION_FLAGS, "--version"],
        env={
            "HOME": "/nonexistent",
            "TMPDIR": "/tmp",
            "PATH": f"{interpreter.parent}:/usr/bin:/bin",
            "LANG": "C",
            "LC_ALL": "C",
            "TZ": "UTC",
        },
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )
    if completed.returncode != 0 or completed.stdout.strip() != record["interpreter_version"]:
        raise CaptureError(f"{where} interpreter version differs from the capture spec")
    record.update(
        {
            "interpreter_path": str(interpreter),
            "program_path": str(program),
            "plan_path": str(plan),
        }
    )
    return record


def program_argv(program: Mapping[str, Any]) -> list[str]:
    """Reconstruct the only executable capture command from sealed identities."""

    return [
        str(program["interpreter_path"]),
        *PYTHON_ISOLATION_FLAGS,
        str(program["program_path"]),
        "--plan",
        str(program["plan_path"]),
    ]


def _program_environment(
    program: Mapping[str, Any], output: Path
) -> dict[str, str]:
    private_root = output.parent / ".capture-python"
    return {
        "HOME": str((private_root / "home").resolve()),
        "TMPDIR": str((private_root / "tmp").resolve()),
        "PATH": f"{Path(str(program['interpreter_path'])).parent}:/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    }


def _supervisor_file_record(path: Path) -> dict[str, Any]:
    return {
        "path": str(path.resolve()),
        "sha256": qualification.sha256_file(path),
        "size_bytes": path.stat().st_size,
    }


def _logical_program_path(
    path: Path, local_run_root: Path | None, recorded_run_root: Path | None
) -> Path:
    resolved = path.resolve()
    if local_run_root is None or recorded_run_root is None:
        return resolved
    try:
        relative = resolved.relative_to(local_run_root.resolve())
    except ValueError:
        return resolved
    return recorded_run_root / relative


def verify_program_supervisor(
    program: Mapping[str, Any],
    arguments: Sequence[str],
    *,
    output: Path,
    log_prefix: Path,
    portable_run_root: Path | None = None,
) -> Mapping[str, Any]:
    supervisor_path = log_prefix.with_suffix(".supervisor.json")
    record = _json(supervisor_path, "capture program supervisor")
    _keys(
        record,
        {
            "schema_version", "supervisor_type", "command", "cwd",
            "environment", "process_id", "return_code", "stdout",
            "stderr", "output", "complete_sentinel",
        },
        "capture program supervisor",
    )
    local_run_root: Path | None = None
    recorded_run_root: Path | None = None
    if portable_run_root is not None:
        local_run_root = portable_run_root.resolve()
        try:
            relative = output.resolve().relative_to(local_run_root)
        except ValueError as error:
            raise CaptureError("portable supervisor output escapes its run") from error
        recorded_output = Path(str(_mapping(record["output"], "output")["path"]))
        parts = relative.parts
        if (
            not recorded_output.is_absolute()
            or len(recorded_output.parts) <= len(parts)
            or recorded_output.parts[-len(parts):] != parts
        ):
            raise CaptureError("portable supervisor output path is not relocatable")
        recorded_run_root = Path(*recorded_output.parts[:-len(parts)])
        if recorded_run_root.name != portable_run_root.name:
            raise CaptureError("portable supervisor run identity differs")

    logical_output = _logical_program_path(
        output, local_run_root, recorded_run_root
    )
    logical_log = _logical_program_path(
        log_prefix, local_run_root, recorded_run_root
    )
    logical_program = dict(program)
    for field in ("program_path", "plan_path"):
        logical_program[field] = str(
            _logical_program_path(
                Path(str(program[field])), local_run_root, recorded_run_root
            )
        )
    logical_arguments = [
        str(
            _logical_program_path(
                Path(argument), local_run_root, recorded_run_root
            )
        )
        if Path(argument).is_absolute()
        else argument
        for argument in arguments
    ]
    expected_command = [
        *program_argv(logical_program),
        *logical_arguments,
        "--output",
        str(logical_output),
    ]
    expected_environment = _program_environment(logical_program, logical_output)
    if (
        record["schema_version"] != 1
        or record["supervisor_type"] != "feppml-capture-program-supervisor-v1"
        or record["command"] != expected_command
        or record["cwd"] != str(logical_output.parent)
        or record["environment"] != expected_environment
        or not isinstance(record["process_id"], int)
        or record["process_id"] <= 0
        or record["return_code"] != 0
        or record["complete_sentinel"]
            != "FEPPML_CAPTURE_PROGRAM_SUPERVISOR_COMPLETE"
    ):
        raise CaptureError("capture program supervisor invocation is invalid")
    for field, local in (
        ("stdout", log_prefix.with_suffix(".stdout")),
        ("stderr", log_prefix.with_suffix(".stderr")),
        ("output", output),
    ):
        expected = _mapping(record[field], field)
        logical = (
            logical_output
            if field == "output"
            else logical_log.with_suffix(f".{field}")
        )
        if (
            set(expected) != {"path", "sha256", "size_bytes"}
            or expected["path"] != str(logical)
            or not local.is_file()
            or qualification.sha256_file(local) != expected["sha256"]
            or local.stat().st_size != expected["size_bytes"]
        ):
            raise CaptureError(f"capture program supervisor {field} bytes changed")
    if (
        log_prefix.with_suffix(".stdout").read_bytes() != b""
        or log_prefix.with_suffix(".stderr").read_bytes() != b""
    ):
        raise CaptureError("capture program emitted unexpected outer output")
    return record


def load_spec(
    path: Path,
    qualification_manifest: Mapping[str, Any],
    qualification_manifest_sha256: str,
    *,
    portable_run_root: Path | None = None,
) -> dict[str, Any]:
    spec = dict(_json(path, "capture spec"))
    _keys(
        spec,
        {
            "schema_version", "spec_type", "qualification_manifest",
            "preflight_program", "arm_program", "evidence_header",
            "environment", "cell_ids", "complete_sentinel",
        },
        "capture spec",
    )
    if (
        spec["schema_version"] != 1
        or spec["spec_type"] != CAPTURE_SPEC_TYPE
        or spec["complete_sentinel"] != "FEPPML_CAPTURE_SPEC_COMPLETE"
    ):
        raise CaptureError("capture spec identity/sentinel is invalid")
    if spec["qualification_manifest"] != {
        "id": qualification_manifest["manifest_id"],
        "sha256": qualification_manifest_sha256,
    }:
        raise CaptureError("capture spec is bound to a different qualification manifest")
    spec["preflight_program"] = _program(
        spec["preflight_program"], "preflight_program",
        portable_run_root=portable_run_root,
    )
    spec["arm_program"] = _program(
        spec["arm_program"], "arm_program",
        portable_run_root=portable_run_root,
    )
    if spec["preflight_program"] != spec["arm_program"]:
        raise CaptureError("preflight and arm programs are not one sealed boundary")
    expected_ids = qualification_manifest["matrix_contract"]["expected_cell_ids"]
    if spec["cell_ids"] != expected_ids:
        raise CaptureError("capture spec cell inventory/order differs from the frozen matrix")
    header = _mapping(spec["evidence_header"], "evidence_header")
    _keys(
        header,
        {"candidate", "comparator", "oracles", "source_audit", "platform_artifacts"},
        "evidence_header",
    )
    environment = _mapping(spec["environment"], "environment")
    _keys(
        environment,
        {
            "node_id", "stata_version", "stata_flavor", "scheduler_job_id",
            "comparator_processor_limit", "os", "cpu", "rustc_version",
        },
        "environment",
    )
    for field in (
        "node_id", "stata_version", "stata_flavor", "scheduler_job_id",
        "os", "cpu", "rustc_version",
    ):
        if not isinstance(environment[field], str) or not environment[field]:
            raise CaptureError(f"environment.{field} must be nonempty")
    limit = environment["comparator_processor_limit"]
    if limit != qualification_manifest["capture_contract"]["comparator_processor_limit"]:
        raise CaptureError(
            "environment.comparator_processor_limit differs from the frozen contract"
        )
    return spec


def _run_program(
    program: Mapping[str, Any],
    arguments: Sequence[str],
    *,
    output: Path,
    log_prefix: Path,
    timeout_seconds: float,
) -> int:
    if output.exists():
        raise CaptureError(f"child output already exists: {output}")
    if qualification.sha256_file(Path(program["program_path"])) != program["program_sha256"]:
        raise CaptureError("capture program changed after spec validation")
    output.parent.mkdir(parents=True, exist_ok=True)
    log_prefix.parent.mkdir(parents=True, exist_ok=True)
    private_root = output.parent / ".capture-python"
    if private_root.exists():
        raise CaptureError("capture subprocess private environment already exists")
    (private_root / "home").mkdir(parents=True)
    (private_root / "tmp").mkdir()
    environment = _program_environment(program, output)
    command = [
        *program_argv(program), *arguments, "--output", str(output.resolve())
    ]
    with log_prefix.with_suffix(".stdout").open("xb") as stdout, log_prefix.with_suffix(
        ".stderr"
    ).open("xb") as stderr:
        try:
            process = subprocess.Popen(
                command,
                stdout=stdout,
                stderr=stderr,
                cwd=output.parent.resolve(),
                env=environment,
            )
        except OSError as error:
            raise CaptureError(f"cannot start capture program: {error}") from error
        try:
            return_code = process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired as error:
            process.kill()
            process.wait()
            raise CaptureError("capture subprocess timed out") from error
    if return_code != 0:
        raise CaptureError(f"capture subprocess failed rc={return_code}")
    if not output.is_file():
        raise CaptureError("capture subprocess omitted its declared output")
    _write_new(
        log_prefix.with_suffix(".supervisor.json"),
        {
            "schema_version": 1,
            "supervisor_type": "feppml-capture-program-supervisor-v1",
            "command": command,
            "cwd": str(output.parent.resolve()),
            "environment": environment,
            "process_id": process.pid,
            "return_code": return_code,
            "stdout": _supervisor_file_record(
                log_prefix.with_suffix(".stdout")
            ),
            "stderr": _supervisor_file_record(
                log_prefix.with_suffix(".stderr")
            ),
            "output": _supervisor_file_record(output),
            "complete_sentinel": "FEPPML_CAPTURE_PROGRAM_SUPERVISOR_COMPLETE",
        },
    )
    verify_program_supervisor(
        program,
        arguments,
        output=output,
        log_prefix=log_prefix,
    )
    return process.pid


def _placeholder_timing_is_closed(
    benchmark: Mapping[str, Any], qualification_manifest: Mapping[str, Any]
) -> None:
    cells = benchmark.get("cells")
    if not isinstance(cells, list):
        raise CaptureError("preflight benchmark cells must be a list")
    expected_ids = qualification_manifest["matrix_contract"]["expected_cell_ids"]
    if [item.get("cell_id") for item in cells if isinstance(item, Mapping)] != expected_ids:
        raise CaptureError("preflight benchmark cell inventory is not frozen")
    expected_pairs = [["candidate", "comparator"]]
    expected_pairs.extend(
        ["candidate", "comparator"]
        if order == "AB"
        else ["comparator", "candidate"]
        for order in qualification_manifest["capture_contract"]["warm_pair_order"]
    )
    for raw in cells:
        cell = _mapping(raw, "preflight benchmark cell")
        timing = _mapping(cell.get("timing"), f"{cell.get('cell_id')}.timing")
        if any(
            _mapping(timing.get(arm), f"{cell.get('cell_id')}.{arm}").get("cold_seconds") != 1.0
            or _mapping(timing.get(arm), f"{cell.get('cell_id')}.{arm}").get("warm_seconds") != [1.0] * 9
            for arm in ("candidate", "comparator")
        ):
            raise CaptureError("preflight timing fields are not the frozen placeholders")
        execution = _mapping(cell.get("execution"), f"{cell.get('cell_id')}.execution")
        if execution.get("arm_order") != expected_pairs:
            raise CaptureError("preflight placeholder pair order differs from qualification")


def _load_scientific_preflight(
    path: Path,
    qualification_manifest: Mapping[str, Any],
    qualification_manifest_sha256: str,
    development_manifest: Mapping[str, Any],
    development_manifest_sha256: str,
    *,
    evaluator: Any = development,
) -> dict[str, Any]:
    record = dict(_json(path, "scientific preflight"))
    _keys(
        record,
        {
            "schema_version", "evidence_type", "qualification_manifest_sha256",
            "timing_placeholder", "timed_arm_processes_started",
            "benchmark_evidence", "complete_sentinel",
        },
        "scientific preflight",
    )
    if (
        record["schema_version"] != 1
        or record["evidence_type"] != PREFLIGHT_TYPE
        or record["qualification_manifest_sha256"]
            != qualification_manifest_sha256
        or record["timing_placeholder"] is not True
        or record["timed_arm_processes_started"] != 0
        or record["complete_sentinel"] != "FEPPML_SCIENCE_PREFLIGHT_COMPLETE"
    ):
        raise CaptureError("scientific preflight identity/order attestation is invalid")
    benchmark = _mapping(record["benchmark_evidence"], "preflight benchmark evidence")
    _placeholder_timing_is_closed(benchmark, qualification_manifest)
    reasons = evaluator.evaluate_science_evidence(
        development_manifest,
        benchmark,
        manifest_sha256=development_manifest_sha256,
    )
    if reasons:
        raise CaptureError(
            "scientific preflight failed before timing: " + ", ".join(reasons)
        )
    return copy.deepcopy(dict(benchmark))


ARM_FIELDS = {
    "schema_version", "evidence_type", "cell_id", "arm", "pair_index",
    "temperature", "seconds", "peak_rss_bytes", "process_id", "node_id",
    "stata_version", "artifact_sha256", "fresh_process", "effective_threads",
    "stata_processors", "complete_sentinel",
}


def _arm_measurement(
    path: Path,
    *,
    cell_id: str,
    arm: str,
    pair_index: int,
    temperature: str,
    requested_threads: int,
    artifact_sha256: str,
    environment: Mapping[str, Any],
) -> tuple[dict[str, Any], int, int]:
    record = dict(_json(path, "arm measurement"))
    _keys(record, ARM_FIELDS, "arm measurement")
    if (
        record["schema_version"] != 1
        or record["evidence_type"] != ARM_TYPE
        or record["cell_id"] != cell_id
        or record["arm"] != arm
        or record["pair_index"] != pair_index
        or record["temperature"] != temperature
        or record["complete_sentinel"] != "FEPPML_ARM_COMPLETE"
    ):
        raise CaptureError("arm measurement identity/sentinel is invalid")
    for field in ("seconds", "peak_rss_bytes", "process_id", "effective_threads", "stata_processors"):
        value = record[field]
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or value <= 0:
            raise CaptureError(f"arm measurement {field} must be positive finite")
    if not isinstance(record["process_id"], int):
        raise CaptureError("arm measurement process_id must be an integer")
    if record["fresh_process"] is not True:
        raise CaptureError("arm measurement is not a fresh process")
    if (
        record["node_id"] != environment["node_id"]
        or record["stata_version"] != environment["stata_version"]
        or record["artifact_sha256"] != artifact_sha256
    ):
        raise CaptureError("arm measurement environment/artifact identity mismatch")
    if arm == "candidate":
        if record["effective_threads"] != requested_threads:
            raise CaptureError("candidate did not honor requested native threads")
    else:
        expected = min(requested_threads, environment["comparator_processor_limit"])
        if record["stata_processors"] != expected:
            raise CaptureError("comparator processor control is inconsistent")
    measurement = {
        "seconds": float(record["seconds"]),
        "peak_rss_bytes": int(record["peak_rss_bytes"]),
        "process_id": record["process_id"],
        "node_id": record["node_id"],
        "stata_version": record["stata_version"],
        "artifact_sha256": record["artifact_sha256"],
        "fresh_process": True,
    }
    return measurement, int(record["effective_threads"]), int(record["stata_processors"])


def _capture_pair(
    *,
    run_root: Path,
    program: Mapping[str, Any],
    cell_id: str,
    requested_threads: int,
    index: int,
    order: str,
    candidate_artifact: str,
    comparator_artifact: str,
    environment: Mapping[str, Any],
    timeout_seconds: float,
) -> tuple[dict[str, Any], list[str], int, int]:
    temperature = "cold" if index == 0 else "warm"
    arm_sequence = ["candidate", "comparator"] if order == "AB" else ["comparator", "candidate"]
    measurements: dict[str, Any] = {}
    candidate_effective = 0
    comparator_processors = 0
    pair_root = run_root / "pairs" / cell_id / f"{index:02d}-{order}"
    for arm in arm_sequence:
        arm_root = pair_root / arm
        output = arm_root / "measurement.json"
        _run_program(
            program,
            [
                "--qualification-arm", "--cell-id", cell_id, "--arm", arm,
                "--pair-index", str(index), "--temperature", temperature,
                "--requested-threads", str(requested_threads),
            ],
            output=output,
            log_prefix=arm_root / "capture",
            timeout_seconds=timeout_seconds,
        )
        artifact = candidate_artifact if arm == "candidate" else comparator_artifact
        measurement, effective, processors = _arm_measurement(
            output,
            cell_id=cell_id,
            arm=arm,
            pair_index=index,
            temperature=temperature,
            requested_threads=requested_threads,
            artifact_sha256=artifact,
            environment=environment,
        )
        measurements[arm] = measurement
        if arm == "candidate":
            candidate_effective = effective
        else:
            comparator_processors = processors
    return (
        {
            "index": index,
            "order": order,
            "candidate": measurements["candidate"],
            "comparator": measurements["comparator"],
        },
        arm_sequence,
        candidate_effective,
        comparator_processors,
    )


def run_capture(args: argparse.Namespace) -> dict[str, Any]:
    if not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0:
        raise CaptureError("--timeout-seconds must be positive and finite")
    manifest = qualification.load_manifest(args.manifest)
    manifest_sha = qualification.sha256_file(args.manifest)
    dev_manifest = development.load_manifest(args.development_manifest)
    dev_manifest_sha = qualification.sha256_file(args.development_manifest)
    spec = load_spec(args.spec, manifest, manifest_sha)
    run_root = args.run_root.resolve()
    if run_root.exists():
        raise CaptureError("run root already exists; retries require a new path")
    run_root.mkdir(parents=True)

    preflight_path = run_root / "preflight/science.json"
    _run_program(
        spec["preflight_program"],
        [
            "--qualification-preflight",
            "--qualification-manifest", str(args.manifest.resolve()),
            "--development-manifest", str(args.development_manifest.resolve()),
        ],
        output=preflight_path,
        log_prefix=run_root / "preflight/science",
        timeout_seconds=args.timeout_seconds,
    )
    benchmark = _load_scientific_preflight(
        preflight_path, manifest, manifest_sha, dev_manifest, dev_manifest_sha
    )
    header = spec["evidence_header"]
    qualification._validate_benchmark_identity(
        header["candidate"], header["comparator"], benchmark, dev_manifest_sha
    )

    plans = {item["cell_id"]: item for item in development.expand_cells(dev_manifest)}
    benchmark_cells = {
        item["cell_id"]: item for item in benchmark["cells"] if isinstance(item, Mapping)
    }
    paired_cells: list[dict[str, Any]] = []
    candidate_artifact = header["candidate"]["performance_plugin_sha256"]
    comparator_artifact = header["comparator"]["bundle_sha256"]
    environment = spec["environment"]
    orders = ["AB", *manifest["capture_contract"]["warm_pair_order"]]
    for cell_id in spec["cell_ids"]:
        plan = plans[cell_id]
        requested_threads = plan["requested_threads"]
        pairs: list[dict[str, Any]] = []
        arm_orders: list[list[str]] = []
        candidate_effective: list[int] = []
        comparator_processors: list[int] = []
        for index, order in enumerate(orders):
            pair, arm_order, effective, processors = _capture_pair(
                run_root=run_root,
                program=spec["arm_program"],
                cell_id=cell_id,
                requested_threads=requested_threads,
                index=index,
                order=order,
                candidate_artifact=candidate_artifact,
                comparator_artifact=comparator_artifact,
                environment=environment,
                timeout_seconds=args.timeout_seconds,
            )
            pairs.append(pair)
            arm_orders.append(arm_order)
            candidate_effective.append(effective)
            comparator_processors.append(processors)
        if len(set(candidate_effective)) != 1 or len(set(comparator_processors)) != 1:
            raise CaptureError(f"{cell_id} thread controls changed across pairs")
        cold, warm = pairs[0], pairs[1:]
        cell = benchmark_cells[cell_id]
        cell["timing"] = {
            "candidate": {
                "cold_seconds": cold["candidate"]["seconds"],
                "warm_seconds": [pair["candidate"]["seconds"] for pair in warm],
            },
            "comparator": {
                "cold_seconds": cold["comparator"]["seconds"],
                "warm_seconds": [pair["comparator"]["seconds"] for pair in warm],
            },
        }
        cell["execution"] = {
            "scheduler_job_id": environment["scheduler_job_id"],
            "node_id": environment["node_id"],
            "plugin_sha256": candidate_artifact,
            "comparator_bundle_sha256": comparator_artifact,
            "os": environment["os"],
            "cpu": environment["cpu"],
            "stata_version": environment["stata_version"],
            "stata_flavor": environment["stata_flavor"],
            "rustc_version": environment["rustc_version"],
            "candidate_process_ids": [pair["candidate"]["process_id"] for pair in pairs],
            "comparator_process_ids": [pair["comparator"]["process_id"] for pair in pairs],
            "arm_order": arm_orders,
            "requested_threads": requested_threads,
            "candidate_effective_threads": candidate_effective[0],
            "comparator_stata_processors": comparator_processors[0],
            "comparator_processor_limit": environment["comparator_processor_limit"],
        }
        cell["memory"]["process_peak_rss_bytes"] = max(
            pair["candidate"]["peak_rss_bytes"] for pair in pairs
        )
        paired_cells.append(
            {"cell_id": cell_id, "cold_pair": cold, "warm_pairs": warm}
        )

    benchmark["cells"] = [benchmark_cells[cell_id] for cell_id in spec["cell_ids"]]
    evidence = {
        "schema_version": 1,
        "evidence_type": "feppml-qualification-v1-evidence",
        "qualification_manifest": {"id": manifest["manifest_id"], "sha256": manifest_sha},
        **copy.deepcopy(dict(header)),
        "paired_cells": paired_cells,
        "benchmark_evidence": benchmark,
        "complete_sentinel": "FEPPML_QUALIFICATION_COMPLETE",
    }
    _write_new(args.evidence.resolve(), evidence)
    evaluation = qualification.evaluate_evidence(
        manifest, evidence, manifest_sha256=manifest_sha
    )
    qualification.write_receipt(args.receipt.resolve(), evaluation)
    return evaluation


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_QUALIFICATION_MANIFEST)
    parser.add_argument("--development-manifest", type=Path, default=DEFAULT_DEVELOPMENT_MANIFEST)
    commands = parser.add_subparsers(dest="command", required=True)
    validate = commands.add_parser("validate-spec")
    validate.add_argument("--spec", type=Path, required=True)
    run = commands.add_parser("run")
    run.add_argument("--spec", type=Path, required=True)
    run.add_argument("--run-root", type=Path, required=True)
    run.add_argument("--evidence", type=Path, required=True)
    run.add_argument("--receipt", type=Path, required=True)
    run.add_argument("--timeout-seconds", type=float, default=7200.0)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        manifest = qualification.load_manifest(args.manifest)
        manifest_sha = qualification.sha256_file(args.manifest)
        if args.command == "validate-spec":
            load_spec(args.spec, manifest, manifest_sha)
            print("FEPPML_CAPTURE_SPEC_OK")
            return 0
        result = run_capture(args)
    except (CaptureError, qualification.QualificationError, development.ContractError) as error:
        raise SystemExit(f"feppml qualification capture failed closed: {error}") from error
    print(f"FEPPML_CAPTURE_{result['status'].upper()}|{result['stage']}")
    return 0 if result["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
