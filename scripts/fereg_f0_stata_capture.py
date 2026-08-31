#!/usr/bin/env python3
"""Render and decode one licensed-Stata F0 regression arm.

This module is intentionally below ``fereg_f0_regression.py``.  It owns the
Stata process file contract but neither schedules arm pairs nor evaluates the
regression gates.  Callers run each rendered driver in a fresh process and use
an external watcher to sample RSS between READY, START, and DONE.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import re
from typing import Any, Mapping
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "benchmarks/stata/fereg_f0_arm_template.do"
HELPERS = ROOT / "benchmarks/stata/fereg_f0_capture_helpers.do"
CAPTURE_FILENAME = "capture.machine"
DRIVER_FILENAME = "run.do"
MARKER_FILENAMES = ("READY", "START", "DONE")
MACHINE_PREFIX = "FEREG_F0_CAPTURE|1|"
LOG_SENTINEL_PREFIX = "END_FIXTURE|fereg_f0_arm_capture_v1|"
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]{1,95}$")
SAFE_VARIABLE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{0,31}$")
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
STATA_HEX = re.compile(
    r"^(?:\.|[+-][0-9A-Fa-f]+(?:\.[0-9A-Fa-f]+)?X[+-][0-9A-Fa-f]+)$"
)
PLUGIN_FILENAMES = {
    "fereg_macos.plugin",
    "fereg_linux.plugin",
    "fereg_windows.plugin",
}
REQUIRED_MATRICES = {
    "b",
    "V",
    "fereg_status",
    "fereg_fit",
    "fereg_dof",
    "fereg_solver",
    "fereg_certificate",
    "fereg_runtime",
    "fereg_receipt",
}


class CaptureContractError(ValueError):
    """The render input or Stata capture fails closed."""


@dataclass(frozen=True)
class ArmCaptureSpec:
    profile: str
    arm: str
    case_id: str
    manifest_sha256: str
    source_commit: str
    package_root: Path
    plugin_filename: str
    plugin_sha256: str
    dataset_path: Path
    dataset_sha256: str
    expected_observations: int
    command: str
    requested_threads: int
    output_dir: Path
    saved_outputs: tuple[str, ...] = ()
    obs_id: str = "obs_id"


@dataclass(frozen=True)
class ArmCaptureResult:
    scientific: dict[str, Any]
    seconds: float
    environment: dict[str, str]
    identities: dict[str, str]
    raw: dict[str, Any]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise CaptureContractError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def file_contract(spec: ArmCaptureSpec) -> dict[str, Any]:
    """Return the closed set of files and sentinels produced by one process."""
    _validate_spec(spec)
    return {
        "schema_version": 1,
        "driver": str((spec.output_dir / DRIVER_FILENAME).resolve()),
        "capture": str((spec.output_dir / CAPTURE_FILENAME).resolve()),
        "markers": {
            name.lower(): str((spec.output_dir / name).resolve())
            for name in MARKER_FILENAMES
        },
        "log_sentinel": expected_log_sentinel(spec),
        "rss_scope": "fresh_process_between_ready_start_done",
    }


def expected_log_sentinel(spec: ArmCaptureSpec) -> str:
    return (
        f"{LOG_SENTINEL_PREFIX}arm={spec.arm}|case={spec.case_id}|rc=0"
    )


def _validate_spec(spec: ArmCaptureSpec) -> None:
    if spec.profile != "local_screen":
        raise CaptureContractError("local Stata capture requires profile=local_screen")
    if spec.arm not in {"baseline", "candidate"}:
        raise CaptureContractError("arm must be baseline or candidate")
    if SAFE_ID.fullmatch(spec.case_id) is None:
        raise CaptureContractError("case_id is not a safe machine identifier")
    if HEX_SHA256.fullmatch(spec.manifest_sha256) is None:
        raise CaptureContractError("manifest_sha256 is malformed")
    if HEX_SHA1.fullmatch(spec.source_commit) is None:
        raise CaptureContractError("source_commit is malformed")
    if HEX_SHA256.fullmatch(spec.plugin_sha256) is None:
        raise CaptureContractError("plugin_sha256 is malformed")
    if HEX_SHA256.fullmatch(spec.dataset_sha256) is None:
        raise CaptureContractError("dataset_sha256 is malformed")
    if spec.plugin_filename not in PLUGIN_FILENAMES:
        raise CaptureContractError("plugin filename is not a supported native artifact")
    if (
        isinstance(spec.expected_observations, bool)
        or not isinstance(spec.expected_observations, int)
        or spec.expected_observations <= 0
        or isinstance(spec.requested_threads, bool)
        or not isinstance(spec.requested_threads, int)
        or spec.requested_threads <= 0
    ):
        raise CaptureContractError("observation and thread counts must be positive integers")
    if SAFE_VARIABLE.fullmatch(spec.obs_id) is None:
        raise CaptureContractError("obs_id is not a safe Stata variable name")
    if len(set(spec.saved_outputs)) != len(spec.saved_outputs):
        raise CaptureContractError("saved output names are duplicated")
    for name in spec.saved_outputs:
        if SAFE_VARIABLE.fullmatch(name) is None or name in {
            spec.obs_id,
            "__fereg_f0_sample",
        }:
            raise CaptureContractError(f"unsafe saved output name {name!r}")
    command = spec.command.strip()
    if command != spec.command or not command.lower().startswith("fereg "):
        raise CaptureContractError("capture command must be one exact fereg invocation")
    lowered = command.lower()
    if any(token in command for token in ("\n", "\r", ";")) or any(
        token in lowered for token in ("#delimit", " shell ", " run ", " do ")
    ):
        raise CaptureContractError("capture command contains a control directive")
    for path in (
        spec.package_root,
        spec.dataset_path,
        spec.output_dir,
        HELPERS,
        TEMPLATE,
    ):
        raw = str(path.resolve())
        if any(character in raw for character in ('"', "|", "\n", "\r")):
            raise CaptureContractError("capture path contains an unsupported character")
    if not spec.package_root.is_dir() or spec.package_root.is_symlink():
        raise CaptureContractError("isolated package root is absent or a symlink")
    if not spec.dataset_path.is_file() or spec.dataset_path.is_symlink():
        raise CaptureContractError("immutable dataset is absent or a symlink")
    if sha256_file(spec.dataset_path) != spec.dataset_sha256:
        raise CaptureContractError("immutable dataset SHA-256 mismatch")
    plugin = spec.package_root / spec.plugin_filename
    if not plugin.is_file() or plugin.is_symlink():
        raise CaptureContractError("native plugin is absent or a symlink")
    if sha256_file(plugin) != spec.plugin_sha256:
        raise CaptureContractError("native plugin SHA-256 mismatch")
    _package_inventory(spec)


def _package_inventory(spec: ArmCaptureSpec) -> list[tuple[str, Path, str]]:
    package = spec.package_root / "fereg.pkg"
    toc = spec.package_root / "stata.toc"
    if not package.is_file() or package.is_symlink() or not toc.is_file() or toc.is_symlink():
        raise CaptureContractError("isolated package lacks fereg.pkg or stata.toc")
    members = ["fereg.pkg", "stata.toc"]
    for line in package.read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"f\s+([^\s]+)", line.strip())
        if match is None:
            continue
        name = match.group(1)
        if Path(name).name != name or name in members:
            raise CaptureContractError("fereg.pkg contains an unsafe or duplicate member")
        if name.endswith(".plugin") and name != spec.plugin_filename:
            continue
        members.append(name)
    if spec.plugin_filename not in members or "fereg.ado" not in members:
        raise CaptureContractError("package inventory omits fereg or the selected plugin")
    inventory: list[tuple[str, Path, str]] = []
    for name in members:
        path = spec.package_root / name
        if not path.is_file() or path.is_symlink():
            raise CaptureContractError(f"isolated package member is absent: {name}")
        inventory.append((name, path.resolve(), sha256_file(path)))
    return inventory


def _stata_quote(path: Path) -> str:
    return f'"{str(path.resolve()).replace(chr(92), "/")}"'


def _indent(lines: list[str], spaces: int = 0) -> str:
    prefix = " " * spaces
    return "\n".join(prefix + line for line in lines)


def _resolver_lines(spec: ArmCaptureSpec) -> str:
    lines: list[str] = []
    for index, (name, path, digest) in enumerate(_package_inventory(spec), 1):
        expected = str(path).replace("\\", "/")
        lines.extend(
            [
                f"capture findfile {name}",
                "if _rc {",
                "    file close __fereg_f0_capture",
                "    exit 111",
                "}",
                f"local __fereg_f0_resolved_{index} `\"`r(fn)'\"'",
                f"if `\"`__fereg_f0_resolved_{index}'\"' != \"{expected}\" {{",
                "    file close __fereg_f0_capture",
                "    exit 603",
                "}",
                "file write __fereg_f0_capture ///",
                f'    "FEREG_F0_CAPTURE|1|resolver|{name}|{expected}|{digest}" _n',
            ]
        )
    return _indent(lines)


def _saved_blocks(spec: ArmCaptureSpec) -> tuple[str, str, str, str]:
    if not spec.saved_outputs:
        comment = "/* no saved outputs requested */"
        return comment, comment, comment, comment
    precheck: list[str] = []
    science: list[str] = []
    cleanup: list[str] = []
    timed: list[str] = []
    for name in spec.saved_outputs:
        precheck.extend(
            [
                f"capture confirm variable {name}",
                "if !_rc {",
                "    file close __fereg_f0_capture",
                "    exit 110",
                "}",
            ]
        )
        call = (
            f"__fereg_f0_capture_saved {spec.obs_id} {name}, "
            "filehandle(__fereg_f0_capture) "
            "phase({phase}) arm(`arm') caseid(`case_id') output("
            f"{name})"
        )
        science.append(call.format(phase="science"))
        timed.append(call.format(phase="timed"))
        cleanup.append(f"capture drop {name}")
    return tuple(_indent(block) for block in (precheck, science, cleanup, timed))


def render_arm_do(spec: ArmCaptureSpec) -> str:
    """Render a self-contained one-process arm driver without executing it."""
    _validate_spec(spec)
    precheck, science, cleanup, timed = _saved_blocks(spec)
    sysdir_lines = []
    for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
        target = spec.output_dir / "sysdirs" / spec.arm / name.lower()
        sysdir_lines.append(f"sysdir set {name} {_stata_quote(target)}")
    replacements = {
        "PROFILE": spec.profile,
        "ARM": spec.arm,
        "CASE_ID": spec.case_id,
        "REQUESTED_THREADS": str(spec.requested_threads),
        "SYSDIR_SETUP": _indent(sysdir_lines),
        "PACKAGE_ROOT_QUOTED": _stata_quote(spec.package_root),
        "HELPER_PATH_QUOTED": _stata_quote(HELPERS),
        "CAPTURE_PATH_QUOTED": _stata_quote(spec.output_dir / CAPTURE_FILENAME),
        "MANIFEST_SHA256": spec.manifest_sha256,
        "SOURCE_COMMIT": spec.source_commit,
        "PLUGIN_SHA256": spec.plugin_sha256,
        "DATASET_SHA256": spec.dataset_sha256,
        "COMMAND_SHA256": hashlib.sha256(spec.command.encode("utf-8")).hexdigest(),
        "RESOLVER_CHECKS": _resolver_lines(spec),
        "PLUGIN_PATH_QUOTED": _stata_quote(
            spec.package_root / spec.plugin_filename
        ),
        "DATASET_PATH_QUOTED": _stata_quote(spec.dataset_path),
        "EXPECTED_OBSERVATIONS": str(spec.expected_observations),
        "OBS_ID": spec.obs_id,
        "SAVED_OUTPUT_PRECHECKS": precheck,
        "COMMAND": spec.command,
        "SAVED_OUTPUT_CAPTURE_SCIENCE": science,
        "SAVED_OUTPUT_CLEANUP": cleanup,
        "READY_PATH_QUOTED": _stata_quote(spec.output_dir / "READY"),
        "START_PATH_QUOTED": _stata_quote(spec.output_dir / "START"),
        "DONE_PATH_QUOTED": _stata_quote(spec.output_dir / "DONE"),
        "SAVED_OUTPUT_CAPTURE_TIMED": timed,
    }
    rendered = TEMPLATE.read_text(encoding="utf-8")
    for name, value in replacements.items():
        rendered = rendered.replace(f"@@{name}@@", value)
    unresolved = sorted(set(re.findall(r"@@[A-Z0-9_]+@@", rendered)))
    if unresolved:
        raise CaptureContractError(f"unresolved driver template tokens: {unresolved}")
    return rendered


def write_arm_do(spec: ArmCaptureSpec) -> Path:
    """Create isolated sysdirs and write the rendered driver."""
    rendered = render_arm_do(spec)
    spec.output_dir.mkdir(parents=True, exist_ok=False)
    for name in ("personal", "plus", "oldplace", "site"):
        (spec.output_dir / "sysdirs" / spec.arm / name).mkdir(parents=True)
    target = spec.output_dir / DRIVER_FILENAME
    target.write_text(rendered, encoding="utf-8")
    return target


def validate_log_sentinel(text: str, spec: ArmCaptureSpec) -> None:
    observed = [
        line.strip()
        for line in text.splitlines()
        if line.strip().startswith("END_FIXTURE|")
    ]
    expected = expected_log_sentinel(spec)
    if observed != [expected]:
        raise CaptureContractError(
            f"arm log must contain exactly one terminal sentinel {expected!r}; "
            f"observed={observed!r}"
        )


def stata_hex_to_float(value: str) -> float:
    if value == ".":
        return math.nan
    match = re.fullmatch(
        r"([+-])([0-9a-fA-F]+(?:\.[0-9a-fA-F]+)?)X([+-])([0-9a-fA-F]+)",
        value,
    )
    if match is None:
        raise CaptureContractError(f"invalid canonical Stata hex value {value!r}")
    exponent = int(match.group(4), 16)
    if match.group(3) == "-":
        exponent = -exponent
    result = float.fromhex(f"{match.group(1)}0x{match.group(2)}p{exponent}")
    if not math.isfinite(result):
        raise CaptureContractError("scientific evidence contains a missing/nonfinite value")
    return result


def _digest(value: Any) -> str:
    encoded = (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")
    return hashlib.sha256(encoded).hexdigest()


def _parse_machine(text: str, spec: ArmCaptureSpec) -> dict[str, Any]:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if not lines or lines[0] != MACHINE_PREFIX + "schema|1":
        raise CaptureContractError("machine capture lacks its schema header")
    complete = (
        f"{MACHINE_PREFIX}capture_complete|arm|{spec.arm}|case_id|"
        f"{spec.case_id}|rc|0"
    )
    if lines[-1] != complete or lines.count(complete) != 1:
        raise CaptureContractError("machine capture lacks its unique final completion record")

    raw: dict[str, Any] = {
        "identities": {},
        "environment": {},
        "resolver": {},
        "dataset": {},
        "inventory": {phase: {} for phase in ("science", "timed")},
        "scalars": {phase: {} for phase in ("science", "timed")},
        "macros": {phase: {} for phase in ("science", "timed")},
        "dimensions": {phase: {} for phase in ("science", "timed")},
        "matrix_cells": {phase: {} for phase in ("science", "timed")},
        "samples": {},
        "saved_outputs": {phase: {} for phase in ("science", "timed")},
        "return_codes": {},
        "stabilizers": {},
        "phase_records": [],
        "seconds_hex": None,
    }

    def unique(target: dict[str, Any], key: str, value: Any, where: str) -> None:
        if key in target:
            raise CaptureContractError(f"duplicate {where} record {key}")
        target[key] = value

    for line in lines[1:-1]:
        parts = line.split("|")
        if len(parts) < 4 or parts[:2] != ["FEREG_F0_CAPTURE", "1"]:
            raise CaptureContractError(f"malformed machine record {line!r}")
        kind = parts[2]
        values = parts[3:]
        if kind in {"phase_begin", "phase_end"} and len(values) == 1:
            raw["phase_records"].append((kind, values[0]))
        elif kind == "identity" and len(values) == 2:
            unique(raw["identities"], values[0], unquote(values[1]), "identity")
        elif kind == "environment" and len(values) == 2:
            unique(raw["environment"], values[0], unquote(values[1]), "environment")
        elif kind == "resolver" and len(values) == 3:
            unique(
                raw["resolver"],
                values[0],
                {"path": values[1], "sha256": values[2]},
                "resolver",
            )
        elif kind == "dataset" and len(values) == 2:
            unique(raw["dataset"], values[0], unquote(values[1]), "dataset")
        elif kind == "inventory" and len(values) == 4:
            phase, entry_kind, position, name = values
            if phase not in raw["inventory"] or entry_kind not in {
                "scalar",
                "macro",
                "matrix",
                "function",
            }:
                raise CaptureContractError("unknown e() inventory phase or kind")
            raw["inventory"][phase].setdefault(entry_kind, []).append(
                (int(position), name)
            )
        elif kind == "scalar" and len(values) == 3:
            phase, name, value = values
            if phase not in raw["scalars"] or STATA_HEX.fullmatch(value) is None:
                raise CaptureContractError("invalid e() scalar record")
            unique(raw["scalars"][phase], name, value, "scalar")
        elif kind == "macro" and len(values) == 3:
            phase, name, value = values
            if phase not in raw["macros"]:
                raise CaptureContractError("invalid e() macro phase")
            unique(raw["macros"][phase], name, unquote(value), "macro")
        elif kind == "matrix_dimensions" and len(values) == 4:
            phase, name, rows, columns = values
            if phase not in raw["dimensions"]:
                raise CaptureContractError("invalid e() matrix phase")
            unique(
                raw["dimensions"][phase],
                name,
                (int(rows), int(columns)),
                "matrix dimensions",
            )
        elif kind == "matrix" and len(values) == 7:
            phase, name, row, column, row_name, column_name, value = values
            if phase not in raw["matrix_cells"] or STATA_HEX.fullmatch(value) is None:
                raise CaptureContractError("invalid e() matrix record")
            raw["matrix_cells"][phase].setdefault(name, []).append(
                (
                    int(row),
                    int(column),
                    unquote(row_name),
                    unquote(column_name),
                    value,
                )
            )
        elif kind == "sample" and len(values) == 6:
            phase, included, excluded, mask, inside, outside = values
            if phase in raw["samples"]:
                raise CaptureContractError("duplicate sample record")
            raw["samples"][phase] = {
                "included": int(included),
                "excluded": int(excluded),
                "mask": unquote(mask),
                "included_signature": unquote(inside),
                "excluded_signature": unquote(outside),
            }
        elif kind == "saved_output" and len(values) == 7:
            phase, name, nonmissing, signature, minimum, maximum, total = values
            if phase not in raw["saved_outputs"] or any(
                STATA_HEX.fullmatch(value) is None
                for value in (minimum, maximum, total)
            ):
                raise CaptureContractError("invalid saved-output record")
            unique(
                raw["saved_outputs"][phase],
                name,
                {
                    "nonmissing": int(nonmissing),
                    "signature": unquote(signature),
                    "minimum_hex": minimum,
                    "maximum_hex": maximum,
                    "sum_hex": total,
                },
                "saved output",
            )
        elif kind == "return_code" and len(values) == 2:
            unique(raw["return_codes"], values[0], int(values[1]), "return code")
        elif kind == "stabilizer" and len(values) == 2:
            unique(raw["stabilizers"], values[0], int(values[1]), "stabilizer")
        elif kind == "timing_seconds" and len(values) == 1:
            if raw["seconds_hex"] is not None or STATA_HEX.fullmatch(values[0]) is None:
                raise CaptureContractError("invalid timing record")
            raw["seconds_hex"] = values[0]
        elif kind == "absent":
            raise CaptureContractError(f"requested e() result was absent: {values!r}")
        else:
            raise CaptureContractError(f"unknown or malformed machine record {line!r}")

    expected_identities = {
        "profile": spec.profile,
        "arm": spec.arm,
        "case_id": spec.case_id,
        "manifest_sha256": spec.manifest_sha256,
        "source_commit": spec.source_commit,
        "plugin_sha256": spec.plugin_sha256,
        "dataset_sha256": spec.dataset_sha256,
        "command_sha256": hashlib.sha256(spec.command.encode("utf-8")).hexdigest(),
    }
    if raw["identities"] != expected_identities:
        raise CaptureContractError("machine capture identity does not match its render spec")
    expected_resolver = {
        name: {"path": str(path).replace("\\", "/"), "sha256": digest}
        for name, path, digest in _package_inventory(spec)
    }
    if raw["resolver"] != expected_resolver:
        raise CaptureContractError("isolated package resolver evidence is incomplete")
    if raw["dataset"].get("observations") != str(spec.expected_observations) or not raw[
        "dataset"
    ].get("stata_signature"):
        raise CaptureContractError("immutable dataset evidence is incomplete")
    required_environment = {
        "stata_version",
        "stata_flavor",
        "os",
        "machine_type",
        "requested_threads",
        "stata_processors",
    }
    if set(raw["environment"]) != required_environment:
        raise CaptureContractError("Stata environment evidence is incomplete")
    if raw["environment"]["requested_threads"] != str(spec.requested_threads):
        raise CaptureContractError("Stata environment reports the wrong thread request")
    if raw["return_codes"] != {"science": 0, "timed": 0}:
        raise CaptureContractError("science or measured estimator call failed")
    if raw["stabilizers"] != {"1": 0, "2": 0}:
        raise CaptureContractError("exactly two successful stabilizers are required")
    if raw["phase_records"] != [
        ("phase_begin", "science"),
        ("phase_end", "science"),
    ]:
        raise CaptureContractError("science phase boundary is incomplete or out of order")
    if set(raw["samples"]) != {"science", "timed"}:
        raise CaptureContractError("science/timed sample evidence is incomplete")
    if raw["samples"]["science"] != raw["samples"]["timed"]:
        raise CaptureContractError("timed sample differs from scientific preflight")
    if set(raw["saved_outputs"]["science"]) != set(spec.saved_outputs):
        raise CaptureContractError("scientific saved-output inventory is incomplete")
    if raw["saved_outputs"]["science"] != raw["saved_outputs"]["timed"]:
        raise CaptureContractError("timed saved outputs differ from scientific preflight")
    seconds = stata_hex_to_float(raw["seconds_hex"])
    if seconds <= 0.0:
        raise CaptureContractError("measured duration must be positive")

    for phase in ("science", "timed"):
        for kind, entries in raw["inventory"][phase].items():
            if entries != [(index, name) for index, (_, name) in enumerate(entries, 1)]:
                raise CaptureContractError(f"{phase} e({kind}) inventory is not contiguous")
        if set(raw["dimensions"][phase]) != set(raw["matrix_cells"][phase]):
            raise CaptureContractError(f"{phase} matrix dimensions/cells disagree")
        for name, shape in raw["dimensions"][phase].items():
            rows, columns = shape
            cells = raw["matrix_cells"][phase][name]
            coordinates = {(row, column) for row, column, *_ in cells}
            expected = {
                (row, column)
                for row in range(1, rows + 1)
                for column in range(1, columns + 1)
            }
            if coordinates != expected or len(cells) != len(expected):
                raise CaptureContractError(f"{phase} matrix {name} is incomplete")
        inventory_names = {
            kind: [name for _, name in entries]
            for kind, entries in raw["inventory"][phase].items()
        }
        if set(inventory_names.get("scalar", [])) != set(raw["scalars"][phase]):
            raise CaptureContractError(f"{phase} scalar inventory disagrees with values")
        if set(inventory_names.get("macro", [])) != set(raw["macros"][phase]):
            raise CaptureContractError(f"{phase} macro inventory disagrees with values")
        if set(inventory_names.get("matrix", [])) != set(raw["dimensions"][phase]):
            raise CaptureContractError(f"{phase} matrix inventory disagrees with values")
    if not REQUIRED_MATRICES.issubset(raw["dimensions"]["science"]):
        raise CaptureContractError("scientific capture lacks mandatory fereg matrices")
    return raw


def _matrix(raw: Mapping[str, Any], phase: str, name: str) -> dict[str, Any]:
    rows, columns = raw["dimensions"][phase][name]
    cells = sorted(raw["matrix_cells"][phase][name])
    return {
        "rows": rows,
        "columns": columns,
        "row_names": [
            next(cell[2] for cell in cells if cell[0] == row)
            for row in range(1, rows + 1)
        ],
        "column_names": [
            next(cell[3] for cell in cells if cell[1] == column)
            for column in range(1, columns + 1)
        ],
        "hex": [cell[4] for cell in cells],
    }


def _finite_matrix(matrix: Mapping[str, Any]) -> list[float]:
    return [stata_hex_to_float(value) for value in matrix["hex"]]


def _solver_contract(matrix: Mapping[str, Any]) -> dict[str, Any]:
    """Remove only the two inherently environmental solver timing columns."""
    retained_columns = [
        column for column in range(matrix["columns"]) if column not in {8, 9}
    ]
    values = _finite_matrix(matrix)
    return {
        "source_columns": [column + 1 for column in retained_columns],
        "values": [
            values[row * matrix["columns"] + column]
            for row in range(matrix["rows"])
            for column in retained_columns
        ],
    }


def parse_arm_capture(text: str, spec: ArmCaptureSpec) -> ArmCaptureResult:
    """Validate raw machine records and derive evaluator-facing science data."""
    raw = _parse_machine(text, spec)
    b = _matrix(raw, "science", "b")
    covariance = _matrix(raw, "science", "V")
    status = _matrix(raw, "science", "fereg_status")
    fit = _matrix(raw, "science", "fereg_fit")
    dof = _matrix(raw, "science", "fereg_dof")
    solver = _matrix(raw, "science", "fereg_solver")
    certificate = _matrix(raw, "science", "fereg_certificate")
    receipt = _matrix(raw, "science", "fereg_receipt")
    timed_science_names = {
        "b",
        "V",
        "fereg_status",
        "fereg_fit",
        "fereg_dof",
        "fereg_certificate",
        "fereg_receipt",
    }
    for name in timed_science_names:
        if _matrix(raw, "science", name) != _matrix(raw, "timed", name):
            raise CaptureContractError(
                f"timed {name} differs from the scientific preflight"
            )
    if _solver_contract(solver) != _solver_contract(
        _matrix(raw, "timed", "fereg_solver")
    ):
        raise CaptureContractError("timed solver result differs from scientific preflight")
    if b["rows"] != 1 or covariance["rows"] != b["columns"] or covariance[
        "columns"
    ] != b["columns"]:
        raise CaptureContractError("coefficient and covariance dimensions disagree")
    if status["rows"] != b["columns"] or status["columns"] < 2:
        raise CaptureContractError("coefficient status is not coefficient aligned")

    coefficient_names = b["column_names"]
    coefficients = _finite_matrix(b)
    status_values = _finite_matrix(status)
    coefficient_status = []
    for row in range(status["rows"]):
        value = status_values[row * status["columns"] + 1]
        if value != int(value):
            raise CaptureContractError("coefficient status code is not integral")
        coefficient_status.append(int(value))

    solver_values = _finite_matrix(solver)
    certificate_values = _finite_matrix(certificate)
    if solver["columns"] < 8 or certificate["rows"] < 19 or certificate["columns"] < 2:
        raise CaptureContractError("fereg certificate matrices are undersized")
    normal_equation = max(
        abs(solver_values[row * solver["columns"] + 7])
        for row in range(solver["rows"])
    )
    fe_projection = max(
        abs(solver_values[row * solver["columns"] + 5])
        for row in range(solver["rows"])
    )
    vce_symmetry = abs(certificate_values[18 * certificate["columns"] + 1])
    reconstruction = max(
        abs(certificate_values[row * certificate["columns"] + 1])
        for row in range(8, 12)
    )
    scalars = raw["scalars"]["science"]
    if "converged" not in scalars:
        raise CaptureContractError("e(converged) is absent")
    converged = stata_hex_to_float(scalars["converged"]) == 1.0
    receipt_values = _finite_matrix(receipt)
    if not receipt_values or receipt_values[-1] != 1.0:
        raise CaptureContractError("fereg receipt is incomplete")

    sample = raw["samples"]["science"]
    sample_identity = _digest(
        {
            "count": sample["included"],
            "signature": sample["included_signature"],
            "mask": sample["mask"],
        }
    )
    singleton_identity = _digest(
        {
            "count": sample["excluded"],
            "signature": sample["excluded_signature"],
        }
    )
    outputs = {
        name: {
            "nonmissing": value["nonmissing"],
            "identity_sha256": _digest(
                {"name": name, "signature": value["signature"]}
            ),
            "minimum_hex": value["minimum_hex"],
            "maximum_hex": value["maximum_hex"],
            "sum_hex": value["sum_hex"],
        }
        for name, value in sorted(raw["saved_outputs"]["science"].items())
    }
    diagnostic_names = sorted(
        {
            "fereg_status",
            "fereg_dof",
            "fereg_certificate",
            "fereg_receipt",
            "dof_table",
            "fereg_fe",
            "fereg_cluster",
            "fereg_subset",
        }
        & set(raw["dimensions"]["science"])
    )
    digests = {
        name: _digest(_matrix(raw, "science", name)) for name in diagnostic_names
    }
    digests["solver_contract"] = _digest(_solver_contract(solver))
    digests["e_inventory"] = _digest(raw["inventory"]["science"])
    digests["sample"] = sample_identity
    digests["saved_outputs"] = _digest(outputs)

    scientific = {
        "stata_rc": 0,
        "sample_identity_sha256": sample_identity,
        "singleton_identity_sha256": singleton_identity,
        "numeric_hex_sha256": _digest(
            {
                "coefficients": b["hex"],
                "vce": covariance["hex"],
                "fit": fit["hex"],
                "certificates": certificate["hex"],
            }
        ),
        "coefficient_names": coefficient_names,
        "coefficient_status": coefficient_status,
        "coefficients": coefficients,
        "vce_shape": [covariance["rows"], covariance["columns"]],
        "vce": _finite_matrix(covariance),
        "dof": {"fereg_dof": _finite_matrix(dof)},
        "fit": {"fereg_fit": _finite_matrix(fit)},
        "outputs": outputs,
        "solver": _solver_contract(solver),
        "certificates": {
            "converged": converged,
            "values": {
                "normal_equation_linf": normal_equation,
                "fe_projection_linf": fe_projection,
                "vce_symmetry_linf": vce_symmetry,
                "reconstruction_linf": reconstruction,
            },
            "digests": digests,
        },
    }
    return ArmCaptureResult(
        scientific=scientific,
        seconds=stata_hex_to_float(raw["seconds_hex"]),
        environment=dict(raw["environment"]),
        identities=dict(raw["identities"]),
        raw=raw,
    )


def load_completed_capture(spec: ArmCaptureSpec, log_text: str) -> ArmCaptureResult:
    """Validate marker files, log sentinel, and the final machine record."""
    validate_log_sentinel(log_text, spec)
    for name in MARKER_FILENAMES:
        path = spec.output_dir / name
        if not path.is_file():
            raise CaptureContractError(f"arm process omitted RSS marker {name}")
        expected_prefix = f"phase={name}|arm={spec.arm}|case_id={spec.case_id}"
        if not path.read_text(encoding="utf-8").strip().startswith(expected_prefix):
            raise CaptureContractError(f"arm RSS marker {name} has the wrong identity")
    capture = spec.output_dir / CAPTURE_FILENAME
    if not capture.is_file():
        raise CaptureContractError("arm process produced no machine capture")
    return parse_arm_capture(capture.read_text(encoding="utf-8"), spec)


__all__ = [
    "ArmCaptureResult",
    "ArmCaptureSpec",
    "CAPTURE_FILENAME",
    "CaptureContractError",
    "DRIVER_FILENAME",
    "MARKER_FILENAMES",
    "expected_log_sentinel",
    "file_contract",
    "load_completed_capture",
    "parse_arm_capture",
    "render_arm_do",
    "sha256_file",
    "stata_hex_to_float",
    "validate_log_sentinel",
    "write_arm_do",
]
