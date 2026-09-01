#!/usr/bin/env python3
"""Parse the closed, lossless Stata qualification line protocol.

Stata is deliberately not asked to produce JSON.  The generated qualification
programs write one tab-delimited record per fact, render every finite numeric
value with ``%24.17g``, and finish with a mandatory terminal record.  This
parser owns all structural interpretation and produces the exact raw mapping
accepted by :mod:`normalize_feppml_stata_raw`.

Comparator coefficient statuses are never accepted from the child process.
They are injected from the immutable recipe's ordered expected-status map.
Candidate statuses, by contrast, must be present in the native result.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import struct
from typing import Any, Mapping, Sequence


PROTOCOL_NAME = "FEPPML_STATA_PROTOCOL"
PROTOCOL_VERSION = "1"
TERMINAL_SENTINEL = "FEPPML_STATA_RAW_COMPLETE"
GENERIC_SEPARATION_CODE = 1

STATUS_CODES = frozenset(range(1, 8))
OUTPUT_KINDS = {
    "fit": None,
    "prediction": "prediction_probe",
    "total_absorbed_effect": "total_absorbed_effect_probe",
    "individual_fe": "individual_fe_probe",
}
PROBE_RECORD_KINDS = {value for value in OUTPUT_KINDS.values() if value is not None}
DOF_FIELDS = {
    "N_physical", "N_effective", "finite_rank", "absorbed_rank", "df"
}
OBJECTIVE_FIELDS = {
    "deviance", "log_likelihood", "null_log_likelihood", "pseudo_r2"
}
CERTIFICATE_BOOL_FIELDS = {
    "two_consecutive_iterations", "strict_final_solve", "rank_frozen"
}
CERTIFICATE_NUMERIC_FIELDS = {
    "score_linf", "fe_score_linf", "vce_symmetry_linf", "reconstruction_linf"
}
DIAGNOSTIC_TEXT_FIELDS = {"requested_memory", "effective_route", "linear_route"}
DIAGNOSTIC_NUMERIC_FIELDS = {
    "requested_threads", "effective_threads", "fallback_mask", "fallback_count",
    "outer_iterations", "inner_iterations", "step_halvings",
}
DIAGNOSTIC_BOOL_FIELDS = {
    "workspace_allocation_free", "whole_iteration_allocation_free"
}
EXPECTED_PHASES = (
    "preparation", "separation_fe", "separation_simplex", "separation_ir",
    "separation_mu", "weight_response_refresh", "fe_projection", "finite_solve",
    "objective_line_search", "strict_final_solve", "inference", "fe_recovery",
    "publication",
)
MEMORY_NUMERIC_FIELDS = {
    "modeled_peak_bytes", "native_peak_bytes", "process_peak_rss_bytes",
    "repeated_call_retained_bytes", "measured_steady_state_iterations",
    "allocator_event_count_start", "allocator_event_count_end",
    "allocator_current_bytes_start", "allocator_current_bytes_end",
    "allocator_peak_bytes_start", "allocator_peak_bytes_end",
    "workspace_capacity_start", "workspace_capacity_end", "scratch_bytes_start",
    "scratch_bytes_end", "scratch_files_after", "median_steady_iteration_seconds",
}
MEMORY_BOOL_FIELDS = {"linear_session_allocation_free", "fresh_warm_equivalent"}
INTEGER_MEMORY_FIELDS = MEMORY_NUMERIC_FIELDS - {"median_steady_iteration_seconds"}
FALLBACK_NAMES = ("warm_projection", "moment", "rank", "bread", "route")
MASK_FORMAT = "ASCII_MASK_V1"
MASK_SUFFIX = ".mask"
SAMPLE_HASH_PREFIX = b"FEPPML_SAMPLE_MASK_V1\0"
SEPARATION_HASH_PREFIX = b"FEPPML_GENERIC_SEPARATION_MASK_V1\0"

_NUMBER = re.compile(r"^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")
_SAFE_NAME = re.compile(r"^[A-Za-z0-9_.:#-]+$")
_SAFE_SOURCE = re.compile(r"^(?:e\([A-Za-z0-9_]+\)|[A-Za-z_][A-Za-z0-9_]*)$")


class StataProtocolError(ValueError):
    """The Stata child output is incomplete, unsafe, or internally inconsistent."""


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise StataProtocolError(
            f"{where} is not closed; missing={sorted(fields-set(value))}, "
            f"extra={sorted(set(value)-fields)}"
        )


def _number(token: str, where: str, *, allow_missing: bool = False) -> float | None:
    if token == "." and allow_missing:
        return None
    stripped = token.strip()
    if not stripped or _NUMBER.fullmatch(stripped) is None:
        raise StataProtocolError(f"{where} is not a %24.17g decimal")
    try:
        value = float(stripped)
    except ValueError as error:
        raise StataProtocolError(f"{where} is not numeric") from error
    if not math.isfinite(value):
        raise StataProtocolError(f"{where} is not finite")
    return value


def _integer(token: str, where: str, *, minimum: int | None = None) -> int:
    value = _number(token, where)
    assert value is not None
    integer = int(value)
    if value != integer or (minimum is not None and integer < minimum):
        raise StataProtocolError(f"{where} is not an admissible integer")
    return integer


def _boolean(token: str, where: str) -> bool:
    value = _integer(token, where)
    if value not in (0, 1):
        raise StataProtocolError(f"{where} is not boolean")
    return bool(value)


def _text(length: str, value: str, where: str, *, pattern: re.Pattern[str] | None = None) -> str:
    expected = _integer(length, f"{where}.length", minimum=1)
    try:
        encoded = value.encode("ascii")
    except UnicodeEncodeError as error:
        raise StataProtocolError(f"{where} is not ASCII-safe") from error
    if len(encoded) != expected or any(character in value for character in ("\t", "\r", "\n", "\x00")):
        raise StataProtocolError(f"{where} length or line safety is invalid")
    if pattern is not None and pattern.fullmatch(value) is None:
        raise StataProtocolError(f"{where} contains a Stata-unsafe token")
    return value


def _put_once(target: dict[str, Any], key: str, value: Any, where: str) -> None:
    if key in target:
        raise StataProtocolError(f"duplicate {where}: {key}")
    target[key] = value


def _expected_map(value: Sequence[Mapping[str, Any]]) -> tuple[list[str], list[int]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or not value:
        raise StataProtocolError("expected status map must be a nonempty ordered list")
    names: list[str] = []
    codes: list[int] = []
    for index, raw in enumerate(value):
        if not isinstance(raw, Mapping):
            raise StataProtocolError(f"expected_status_map[{index}] must be an object")
        _closed(raw, {"name", "status_code"}, f"expected_status_map[{index}]")
        name = raw["name"]
        code = raw["status_code"]
        if not isinstance(name, str) or _SAFE_NAME.fullmatch(name) is None:
            raise StataProtocolError(f"expected_status_map[{index}].name is unsafe")
        if isinstance(code, bool) or not isinstance(code, int) or code not in STATUS_CODES:
            raise StataProtocolError(f"expected_status_map[{index}].status_code is invalid")
        names.append(name)
        codes.append(code)
    if len(set(names)) != len(names):
        raise StataProtocolError("expected status map contains duplicate coefficient names")
    return names, codes


def _ordered(indexed: Mapping[int, Any], where: str, *, allow_empty: bool = False) -> list[Any]:
    if not indexed:
        if allow_empty:
            return []
        raise StataProtocolError(f"{where} is empty")
    expected = list(range(1, len(indexed) + 1))
    if sorted(indexed) != expected:
        raise StataProtocolError(f"{where} indices are not contiguous and one-based")
    return [indexed[index] for index in expected]


def _fallbacks(mask: int, count: int) -> list[str]:
    if mask < 0 or mask & ~0b1_1111 or count < bin(mask).count("1") or ((mask == 0) != (count == 0)):
        raise StataProtocolError("fallback mask/count is inconsistent")
    return [name for bit, name in enumerate(FALLBACK_NAMES) if mask & (1 << bit)]


def _probe_positions(row_count: int, probe_rows: int) -> list[int]:
    count = min(row_count, probe_rows)
    if count == 1:
        return [1]
    return [1 + (index * (row_count - 1)) // (count - 1) for index in range(count)]


def _parse_mask_sidecar(
    path: Path,
    declaration: Mapping[str, int | str],
    probes: Mapping[int, tuple[int, bool, bool, int]],
    *,
    expected_probe_rows: int,
) -> tuple[dict[str, Any], dict[str, Any]]:
    row_count = int(declaration["row_count"])
    chunk_bytes = int(declaration["chunk_bytes"])
    expected_positions = _probe_positions(row_count, expected_probe_rows)
    ordered_probes = _ordered(probes, "MASK_PROBE records")
    if len(ordered_probes) != len(expected_positions):
        raise StataProtocolError("MASK_PROBE inventory differs from the immutable recipe")

    sample_digest = hashlib.sha256()
    sample_digest.update(SAMPLE_HASH_PREFIX)
    sample_digest.update(struct.pack(">Q", row_count))
    separation_digest = hashlib.sha256()
    separation_digest.update(SEPARATION_HASH_PREFIX)
    separation_digest.update(struct.pack(">Q", row_count))
    counts = {"eligible": 0, "retained": 0, "separated": 0}
    observed_codes: dict[int, int] = {}
    first_separated: list[list[int]] = []
    cursor = 0
    translation = bytes.maketrans(b"012", b"001")
    try:
        source = path.open("rb")
    except OSError as error:
        raise StataProtocolError(f"cannot read sample-mask sidecar: {error}") from error
    with source:
        for physical_line, raw_line in enumerate(source, 1):
            if not raw_line.endswith(b"\n"):
                raise StataProtocolError("sample-mask sidecar line lacks newline termination")
            chunk = raw_line[:-1]
            if not chunk or len(chunk) > chunk_bytes or any(code not in b"012" for code in chunk):
                raise StataProtocolError(f"sample-mask sidecar chunk {physical_line} is invalid")
            if cursor + len(chunk) > row_count:
                raise StataProtocolError("sample-mask sidecar contains excess rows")
            sample_digest.update(chunk)
            separation_digest.update(chunk.translate(translation))
            counts["eligible"] += len(chunk) - chunk.count(b"0")
            counts["retained"] += chunk.count(b"1")
            counts["separated"] += chunk.count(b"2")
            lo = cursor + 1
            hi = cursor + len(chunk)
            for position in expected_positions:
                if lo <= position <= hi:
                    observed_codes[position] = chunk[position - lo]
            if len(first_separated) < expected_probe_rows:
                for offset, code in enumerate(chunk):
                    if code == ord("2"):
                        first_separated.append([lo + offset, GENERIC_SEPARATION_CODE])
                        if len(first_separated) == expected_probe_rows:
                            break
            cursor = hi
    if cursor != row_count:
        raise StataProtocolError("sample-mask sidecar row count is incomplete")
    for field, observed in counts.items():
        if observed != declaration[f"{field}_count"]:
            raise StataProtocolError(f"sample-mask {field} count disagrees with the protocol")

    sample_probes: list[list[int]] = []
    for expected_position, record in zip(expected_positions, ordered_probes):
        obs_id, eligible, retained, reason = record
        if obs_id != expected_position:
            raise StataProtocolError("MASK_PROBE observation position is not deterministic")
        code = observed_codes.get(expected_position)
        expected_code = 1 if retained else (2 if eligible else 0)
        if code != ord(str(expected_code)):
            raise StataProtocolError("MASK_PROBE differs from the bound sidecar")
        if reason != (GENERIC_SEPARATION_CODE if expected_code == 2 else 0):
            raise StataProtocolError("MASK_PROBE normalized reason is invalid")
        sample_probes.append([obs_id, int(eligible), int(retained), reason])

    sample_identity = {
        "algorithm": "sha256",
        "encoding": "feppml-sample-mask-v1",
        "row_count": row_count,
        "eligible_count": counts["eligible"],
        "retained_count": counts["retained"],
        "separated_count": counts["separated"],
        "sha256": sample_digest.hexdigest(),
        "probes": sample_probes,
    }
    separation_ledger = {
        "algorithm": "sha256",
        "encoding": "feppml-generic-separation-mask-v1",
        "row_count": row_count,
        "separated_count": counts["separated"],
        "generic_reason_code": GENERIC_SEPARATION_CODE,
        "sha256": separation_digest.hexdigest(),
        "probes": first_separated,
    }
    return sample_identity, separation_ledger


def parse_science_protocol(
    path: Path,
    *,
    arm: str,
    expected_status_map: Sequence[Mapping[str, Any]],
    expected_output: str,
    expected_probe_rows: int,
    expected_cluster_dimensions: int,
    expected_saved_channels: int,
) -> dict[str, Any]:
    """Parse one complete science export and return its raw/envelope records."""
    if arm not in {"candidate", "comparator"}:
        raise StataProtocolError("requested arm is invalid")
    if expected_output not in OUTPUT_KINDS:
        raise StataProtocolError("requested output kind is invalid")
    if isinstance(expected_probe_rows, bool) or not isinstance(expected_probe_rows, int) or expected_probe_rows < 1:
        raise StataProtocolError("requested probe-row count is invalid")
    if isinstance(expected_cluster_dimensions, bool) or not isinstance(expected_cluster_dimensions, int) or expected_cluster_dimensions < 0:
        raise StataProtocolError("requested cluster dimension is invalid")
    if isinstance(expected_saved_channels, bool) or not isinstance(expected_saved_channels, int) or expected_saved_channels < 0:
        raise StataProtocolError("requested saved-channel count is invalid")
    expected_names, frozen_codes = _expected_map(expected_status_map)
    try:
        payload = path.read_bytes()
    except OSError as error:
        raise StataProtocolError(f"cannot read Stata protocol: {error}") from error
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise StataProtocolError("Stata protocol is not ASCII") from error
    if not text.endswith("\n"):
        raise StataProtocolError("Stata protocol lacks a terminal newline")
    lines = text.splitlines()
    if not lines or lines[0] != f"HEADER\t{PROTOCOL_NAME}\t{PROTOCOL_VERSION}\t{arm}":
        raise StataProtocolError("Stata protocol header/arm is invalid")
    if lines[-1] != f"END\t{TERMINAL_SENTINEL}":
        raise StataProtocolError("Stata protocol terminal sentinel is absent or displaced")
    if any(not line or len(line) > 1_048_576 for line in lines):
        raise StataProtocolError("Stata protocol contains an empty or overlong record")

    result: dict[str, Any] = {}
    dof: dict[str, Any] = {}
    objective: dict[str, Any] = {}
    coefficients: dict[int, tuple[str, int | None, float]] = {}
    vce: dict[tuple[int, int], float] = {}
    clusters: dict[int, int] = {}
    mask_declaration: dict[str, int | str] | None = None
    mask_probes: dict[int, tuple[int, bool, bool, int]] = {}
    drop_reason_audit: dict[str, int | str] | None = None
    probes: dict[str, dict[int, float]] = {kind: {} for kind in PROBE_RECORD_KINDS}
    cert_bools: dict[str, bool] = {}
    cert_nums: dict[str, float] = {}
    stata: dict[str, Any] = {}
    sources: dict[str, list[str]] = {kind: [] for kind in ("matrix", "scalar", "variable")}
    source_seen: set[tuple[str, str]] = set()
    receipt: dict[str, bool] = {}
    diagnostic_text: dict[str, str] = {}
    diagnostic_num: dict[str, float] = {}
    diagnostic_bool: dict[str, bool] = {}
    phases: dict[str, float] = {}
    memory_num: dict[str, float] = {}
    memory_bool: dict[str, bool] = {}
    modeled: dict[str, int] = {}
    observed: dict[str, int | None] = {}
    output_declaration: tuple[str, str, int, int] | None = None
    slowdown_seen = False

    for line_number, line in enumerate(lines[1:-1], 2):
        fields = line.split("\t")
        record = fields[0]
        where = f"line {line_number} {record}"
        if record == "RESULT" and len(fields) == 3:
            key = fields[1]
            if key == "rc":
                value: Any = _integer(fields[2], f"{where}.rc")
            elif key == "converged":
                value = _boolean(fields[2], f"{where}.converged")
            else:
                raise StataProtocolError(f"{where} has an unknown key")
            _put_once(result, key, value, "RESULT")
        elif record == "COEF" and len(fields) == 7:
            index = _integer(fields[1], f"{where}.index", minimum=1)
            if index in coefficients:
                raise StataProtocolError(f"duplicate COEF index: {index}")
            name = _text(fields[2], fields[3], f"{where}.name", pattern=_SAFE_NAME)
            if arm == "candidate":
                code = _integer(fields[4], f"{where}.status")
                if code not in STATUS_CODES:
                    raise StataProtocolError(f"{where}.status is unknown")
            else:
                if fields[4] != ".":
                    raise StataProtocolError("comparator status metadata must come only from the frozen recipe")
                code = None
            value = _number(fields[5], f"{where}.value")
            assert value is not None
            if fields[6] != "B":
                raise StataProtocolError(f"{where} lacks its fixed B field")
            coefficients[index] = (name, code, value)
        elif record == "VCE" and len(fields) == 4:
            row = _integer(fields[1], f"{where}.row", minimum=1)
            column = _integer(fields[2], f"{where}.column", minimum=1)
            value = _number(fields[3], f"{where}.value")
            assert value is not None
            if (row, column) in vce:
                raise StataProtocolError(f"duplicate VCE cell: {row},{column}")
            vce[(row, column)] = value
        elif record == "DOF" and len(fields) == 3 and fields[1] in DOF_FIELDS:
            value = _number(fields[2], f"{where}.{fields[1]}")
            assert value is not None
            _put_once(dof, fields[1], value, "DOF")
        elif record == "CLUSTER" and len(fields) == 3:
            index = _integer(fields[1], f"{where}.index", minimum=1)
            if index in clusters:
                raise StataProtocolError(f"duplicate CLUSTER index: {index}")
            clusters[index] = _integer(fields[2], f"{where}.count", minimum=2)
        elif record == "OBJECTIVE" and len(fields) == 3 and fields[1] in OBJECTIVE_FIELDS:
            value = _number(fields[2], f"{where}.{fields[1]}")
            assert value is not None
            _put_once(objective, fields[1], value, "OBJECTIVE")
        elif record == "MASK" and len(fields) == 8:
            if mask_declaration is not None or fields[1] != MASK_FORMAT or fields[7] != "SIDECAR":
                raise StataProtocolError("MASK declaration is duplicate or invalid")
            mask_declaration = {
                "format": fields[1],
                "row_count": _integer(fields[2], f"{where}.row_count", minimum=1),
                "eligible_count": _integer(fields[3], f"{where}.eligible_count", minimum=0),
                "retained_count": _integer(fields[4], f"{where}.retained_count", minimum=0),
                "separated_count": _integer(fields[5], f"{where}.separated_count", minimum=0),
                "chunk_bytes": _integer(fields[6], f"{where}.chunk_bytes", minimum=1),
            }
            if mask_declaration["chunk_bytes"] > 1_048_576:
                raise StataProtocolError("MASK chunk size is unbounded")
        elif record == "MASK_PROBE" and len(fields) == 6:
            index = _integer(fields[1], f"{where}.index", minimum=1)
            if index in mask_probes:
                raise StataProtocolError(f"duplicate MASK_PROBE index: {index}")
            obs_id = _integer(fields[2], f"{where}.obs_id", minimum=1)
            eligible = _boolean(fields[3], f"{where}.eligible")
            retained = _boolean(fields[4], f"{where}.retained")
            raw_reason = _integer(fields[5], f"{where}.normalized_reason", minimum=0)
            if retained and not eligible:
                raise StataProtocolError("a retained observation cannot be ineligible")
            if retained and raw_reason != 0:
                raise StataProtocolError("a retained observation has a nonzero drop reason")
            mask_probes[index] = (obs_id, eligible, retained, raw_reason)
        elif record == "DROP_REASON_AUDIT" and len(fields) == 5:
            if drop_reason_audit is not None or fields[1] != arm:
                raise StataProtocolError("DROP_REASON_AUDIT is duplicate or names another arm")
            drop_reason_audit = {
                "arm": fields[1],
                "eligible_nonretained_count": _integer(fields[2], f"{where}.eligible_nonretained", minimum=0),
                "positive_raw_count": _integer(fields[3], f"{where}.positive_raw", minimum=0),
                "invalid_raw_count": _integer(fields[4], f"{where}.invalid_raw", minimum=0),
            }
        elif record == "OUTPUT" and len(fields) == 5:
            if output_declaration is not None or fields[1] not in OUTPUT_KINDS:
                raise StataProtocolError("OUTPUT declaration is duplicate or invalid")
            mode = fields[2]
            probe_count = _integer(fields[3], f"{where}.probe_count", minimum=0)
            channel_count = _integer(fields[4], f"{where}.channel_count", minimum=0)
            if mode not in {"direct", "saved_channels"}:
                raise StataProtocolError("OUTPUT provenance mode is invalid")
            output_declaration = (fields[1], mode, probe_count, channel_count)
        elif record == "PROBE" and len(fields) == 4 and fields[1] in PROBE_RECORD_KINDS:
            index = _integer(fields[2], f"{where}.index", minimum=1)
            if index in probes[fields[1]]:
                raise StataProtocolError(f"duplicate PROBE index: {fields[1]}[{index}]")
            value = _number(fields[3], f"{where}.value")
            assert value is not None
            probes[fields[1]][index] = value
        elif record == "CERT_BOOL" and len(fields) == 3 and fields[1] in CERTIFICATE_BOOL_FIELDS:
            _put_once(cert_bools, fields[1], _boolean(fields[2], where), "CERT_BOOL")
        elif record == "CERT_NUM" and len(fields) == 3 and fields[1] in CERTIFICATE_NUMERIC_FIELDS:
            value = _number(fields[2], where)
            assert value is not None
            _put_once(cert_nums, fields[1], value, "CERT_NUM")
        elif record == "SOURCE" and len(fields) == 5 and fields[1] in sources:
            name = _text(fields[2], fields[3], f"{where}.name", pattern=_SAFE_SOURCE)
            if fields[4] != "NATIVE":
                raise StataProtocolError(f"{where} lacks its fixed NATIVE field")
            identity = (fields[1], name)
            if identity in source_seen:
                raise StataProtocolError(f"duplicate SOURCE record: {identity}")
            source_seen.add(identity)
            sources[fields[1]].append(name)
        elif record == "RECEIPT" and len(fields) == 3 and fields[1] == "complete":
            _put_once(receipt, fields[1], _boolean(fields[2], where), "RECEIPT")
        elif record == "STATA" and len(fields) == 3 and fields[1] == "licensed":
            _put_once(stata, "licensed", _boolean(fields[2], where), "STATA")
        elif record == "STATA" and len(fields) == 5 and fields[1] == "version":
            value = _text(fields[2], fields[3], f"{where}.version")
            if fields[4] != "LICENSED":
                raise StataProtocolError(f"{where} lacks its fixed LICENSED field")
            _put_once(stata, "version", value, "STATA")
        elif record == "DIAG_TEXT" and len(fields) == 4 and fields[1] in DIAGNOSTIC_TEXT_FIELDS:
            value = _text(fields[2], fields[3], f"{where}.{fields[1]}", pattern=_SAFE_NAME)
            _put_once(diagnostic_text, fields[1], value, "DIAG_TEXT")
        elif record == "DIAG_NUM" and len(fields) == 3 and fields[1] in DIAGNOSTIC_NUMERIC_FIELDS:
            value = _number(fields[2], f"{where}.{fields[1]}")
            assert value is not None
            _put_once(diagnostic_num, fields[1], value, "DIAG_NUM")
        elif record == "DIAG_BOOL" and len(fields) == 3 and fields[1] in DIAGNOSTIC_BOOL_FIELDS:
            _put_once(diagnostic_bool, fields[1], _boolean(fields[2], where), "DIAG_BOOL")
        elif record == "PHASE" and len(fields) == 3 and fields[1] in EXPECTED_PHASES:
            value = _number(fields[2], f"{where}.{fields[1]}")
            assert value is not None
            _put_once(phases, fields[1], value, "PHASE")
        elif record == "MEMORY_NUM" and len(fields) == 3 and fields[1] in MEMORY_NUMERIC_FIELDS:
            value = _number(fields[2], f"{where}.{fields[1]}")
            assert value is not None
            _put_once(memory_num, fields[1], value, "MEMORY_NUM")
        elif record == "MEMORY_BOOL" and len(fields) == 3 and fields[1] in MEMORY_BOOL_FIELDS:
            _put_once(memory_bool, fields[1], _boolean(fields[2], where), "MEMORY_BOOL")
        elif record == "MEMORY_MODELED" and len(fields) == 3 and fields[1] in EXPECTED_PHASES:
            _put_once(modeled, fields[1], _integer(fields[2], where, minimum=0), "MEMORY_MODELED")
        elif record == "MEMORY_OBSERVED" and len(fields) == 3 and fields[1] in EXPECTED_PHASES:
            value = _number(fields[2], where, allow_missing=True)
            if value is not None and (value != int(value) or value < 0):
                raise StataProtocolError(f"{where} is not a nonnegative byte count")
            _put_once(observed, fields[1], None if value is None else int(value), "MEMORY_OBSERVED")
        elif record == "SLOWDOWN" and fields == ["SLOWDOWN", "NONE"]:
            if slowdown_seen:
                raise StataProtocolError("duplicate SLOWDOWN record")
            slowdown_seen = True
        else:
            raise StataProtocolError(f"unknown or malformed record at line {line_number}")

    if set(result) != {"rc", "converged"}:
        raise StataProtocolError("RESULT inventory is incomplete")
    if set(dof) != DOF_FIELDS or set(objective) != OBJECTIVE_FIELDS:
        raise StataProtocolError("DoF/objective inventory is incomplete")
    if set(cert_bools) != CERTIFICATE_BOOL_FIELDS or set(cert_nums) != CERTIFICATE_NUMERIC_FIELDS:
        raise StataProtocolError("certificate inventory is incomplete")
    if output_declaration is None or output_declaration[0] != expected_output:
        raise StataProtocolError("OUTPUT declaration differs from the immutable recipe")
    if stata != {"licensed": True, **({"version": stata["version"]} if "version" in stata else {})} or "version" not in stata:
        raise StataProtocolError("licensed Stata identity is incomplete")
    if set(receipt) != {"complete"} or (arm == "candidate" and receipt["complete"] is not True):
        raise StataProtocolError("native receipt completeness is absent")
    if not source_seen:
        raise StataProtocolError("native source inventory is absent")
    if not slowdown_seen:
        raise StataProtocolError("SLOWDOWN declaration is absent")

    ordered_coefficients = _ordered(coefficients, "COEF records")
    names = [record[0] for record in ordered_coefficients]
    if names != expected_names:
        raise StataProtocolError("coefficient stripe differs from the frozen recipe")
    codes = (
        [record[1] for record in ordered_coefficients]
        if arm == "candidate"
        else list(frozen_codes)
    )
    if any(code is None for code in codes):
        raise StataProtocolError("candidate coefficient statuses are incomplete")
    dimension = len(names)
    expected_vce = {(row, column) for row in range(1, dimension + 1) for column in range(1, dimension + 1)}
    if set(vce) != expected_vce:
        raise StataProtocolError("VCE cells are incomplete or outside the frozen dimension")
    cluster_counts = _ordered(clusters, "CLUSTER records", allow_empty=True)
    if len(cluster_counts) != expected_cluster_dimensions:
        raise StataProtocolError("cluster dimension differs from the immutable recipe")
    if mask_declaration is None:
        raise StataProtocolError("MASK declaration is absent")
    sample_identity, separation_ledger = _parse_mask_sidecar(
        Path(str(path) + MASK_SUFFIX),
        mask_declaration,
        mask_probes,
        expected_probe_rows=expected_probe_rows,
    )
    if drop_reason_audit is None:
        raise StataProtocolError("DROP_REASON_AUDIT is absent")
    if (
        drop_reason_audit["eligible_nonretained_count"] != sample_identity["separated_count"]
        or drop_reason_audit["invalid_raw_count"] != 0
        or (
            arm == "candidate"
            and drop_reason_audit["positive_raw_count"] != sample_identity["separated_count"]
        )
        or (arm == "comparator" and drop_reason_audit["positive_raw_count"] != 0)
    ):
        raise StataProtocolError("drop-reason audit does not certify normalized eligible drops")

    outputs: dict[str, list[float] | None] = {}
    expected_probe = OUTPUT_KINDS[expected_output]
    declared_output, output_mode, declared_probe_count, saved_channel_count = output_declaration
    expected_count = min(sample_identity["retained_count"], expected_probe_rows)
    if expected_output == "fit":
        if (output_mode, declared_probe_count, saved_channel_count) != ("direct", 0, 0):
            raise StataProtocolError("fit output provenance is not empty and closed")
    elif expected_output == "individual_fe":
        if (
            output_mode != "saved_channels"
            or declared_probe_count != expected_count * expected_saved_channels
            or saved_channel_count != expected_saved_channels
            or expected_saved_channels < 1
        ):
            raise StataProtocolError("individual-FE sum provenance or probe count is invalid")
        expected_saved_sources = {f"__q_fe{index}" for index in range(1, saved_channel_count + 1)}
        if not expected_saved_sources <= set(sources["variable"]):
            raise StataProtocolError("individual-FE saved-channel provenance is incomplete")
    elif (output_mode, declared_probe_count, saved_channel_count) != ("direct", expected_count, 1):
        raise StataProtocolError("direct output provenance or probe count is invalid")
    for kind in sorted(PROBE_RECORD_KINDS):
        values = _ordered(probes[kind], f"PROBE {kind}", allow_empty=True)
        if kind == expected_probe:
            required_count = (
                expected_count * expected_saved_channels
                if expected_output == "individual_fe" else expected_count
            )
            if len(values) != required_count:
                raise StataProtocolError(
                    f"requested output probe {kind} count differs from retained-sample provenance"
                )
            outputs[kind] = values
        else:
            if values:
                raise StataProtocolError(f"unrequested output probe {kind} is present")
            outputs[kind] = None

    diagnostics = None
    memory = None
    if arm == "candidate":
        if set(diagnostic_text) != DIAGNOSTIC_TEXT_FIELDS or set(diagnostic_num) != DIAGNOSTIC_NUMERIC_FIELDS or set(diagnostic_bool) != DIAGNOSTIC_BOOL_FIELDS or set(phases) != set(EXPECTED_PHASES):
            raise StataProtocolError("candidate diagnostic inventory is incomplete")
        for field in ("requested_threads", "effective_threads", "fallback_mask", "fallback_count", "outer_iterations", "inner_iterations", "step_halvings"):
            value = diagnostic_num[field]
            if value != int(value) or value < 0:
                raise StataProtocolError(f"candidate diagnostic {field} is not a nonnegative integer")
            diagnostic_num[field] = int(value)
        diagnostics = {
            **diagnostic_text,
            "requested_threads": diagnostic_num["requested_threads"],
            "effective_threads": diagnostic_num["effective_threads"],
            "fallbacks": _fallbacks(diagnostic_num["fallback_mask"], diagnostic_num["fallback_count"]),
            "outer_iterations": diagnostic_num["outer_iterations"],
            "inner_iterations": diagnostic_num["inner_iterations"],
            "step_halvings": diagnostic_num["step_halvings"],
            **diagnostic_bool,
            "phase_seconds": {phase: phases[phase] for phase in EXPECTED_PHASES},
        }
        if set(memory_num) != MEMORY_NUMERIC_FIELDS or set(memory_bool) != MEMORY_BOOL_FIELDS or set(modeled) != set(EXPECTED_PHASES) or set(observed) != set(EXPECTED_PHASES):
            raise StataProtocolError("candidate memory inventory is incomplete")
        for field in INTEGER_MEMORY_FIELDS:
            value = memory_num[field]
            if value != int(value):
                raise StataProtocolError(f"candidate memory {field} is not an integer")
            memory_num[field] = int(value)
        memory = {
            **memory_num,
            **memory_bool,
            "phase_modeled_peak_bytes": {phase: modeled[phase] for phase in EXPECTED_PHASES},
            "phase_observed_peak_bytes": {phase: observed[phase] for phase in EXPECTED_PHASES},
        }
    elif diagnostic_text or diagnostic_num or diagnostic_bool or phases or memory_num or memory_bool or modeled or observed:
        raise StataProtocolError("comparator must not claim candidate-native diagnostics or memory telemetry")

    raw = {
        "arm": arm,
        "rc": result["rc"],
        "converged": result["converged"],
        "sample_identity": sample_identity,
        "separation_ledger": separation_ledger,
        "coefficient_names": names,
        "coefficient_status_codes": [int(code) for code in codes],
        "coefficients": [record[2] for record in ordered_coefficients],
        "vce": [vce[(row, column)] for row in range(1, dimension + 1) for column in range(1, dimension + 1)],
        "dof": {**dof, "cluster_counts": cluster_counts},
        "objective": objective,
        "outputs": outputs,
        "certificates": {**cert_bools, **cert_nums},
        "source_inventory": [f"{kind}:{name}" for kind in ("matrix", "scalar", "variable") for name in sources[kind]],
        "complete_sentinel": TERMINAL_SENTINEL,
    }
    return {
        "raw": raw,
        "diagnostics": diagnostics,
        "memory": memory,
        "slowdown_explanation": None,
        "stata": stata,
        "source_provenance": {
            "matrices": sources["matrix"],
            "scalars": sources["scalar"],
            "variables": sources["variable"],
            "receipt_complete": receipt["complete"],
        },
        "complete_sentinel": "FEPPML_STATA_SCIENCE_COMPLETE",
    }


def parse_raw_protocol(
    path: Path,
    *,
    arm: str,
    expected_status_map: Sequence[Mapping[str, Any]],
    expected_output: str,
    expected_probe_rows: int,
    expected_cluster_dimensions: int,
    expected_saved_channels: int,
) -> dict[str, Any]:
    """Return only the closed raw object consumed by the normalizer."""
    return parse_science_protocol(
        path,
        arm=arm,
        expected_status_map=expected_status_map,
        expected_output=expected_output,
        expected_probe_rows=expected_probe_rows,
        expected_cluster_dimensions=expected_cluster_dimensions,
        expected_saved_channels=expected_saved_channels,
    )["raw"]


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--arm", choices=("candidate", "comparator"), required=True)
    parser.add_argument("--expected-status-map", type=Path, required=True)
    parser.add_argument("--expected-output", choices=tuple(OUTPUT_KINDS), required=True)
    parser.add_argument("--expected-probe-rows", type=int, required=True)
    parser.add_argument("--expected-cluster-dimensions", type=int, required=True)
    parser.add_argument("--expected-saved-channels", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    expected = json.loads(args.expected_status_map.read_text(encoding="utf-8"))
    raw = parse_raw_protocol(
        args.input,
        arm=args.arm,
        expected_status_map=expected,
        expected_output=args.expected_output,
        expected_probe_rows=args.expected_probe_rows,
        expected_cluster_dimensions=args.expected_cluster_dimensions,
        expected_saved_channels=args.expected_saved_channels,
    )
    args.output.write_text(
        json.dumps(raw, sort_keys=True, indent=2, ensure_ascii=True, allow_nan=False) + "\n",
        encoding="ascii",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
