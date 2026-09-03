#!/usr/bin/env python3
"""Close the Phase-12/public-activation evidence set for one exact source.

This is the final *aggregation* boundary.  It does not run tests and it never
turns an unsealed log into evidence.  Every prerequisite must already be a
closed ``feppml-phase12-check-v1`` receipt.  The aggregate gate verifies each
seal and byte identity, requires a complete state-specific inventory, rejects
mixed source or artifact identities, independently audits the committed public
``feppml`` source contract, and writes one immutable merged receipt last.

The SHA-256 envelopes provide integrity and binding, not third-party
authentication.  Producer access controls and evidence retention remain part
of the release process.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from typing import Any, Mapping, Sequence


SCHEMA_VERSION = 1
CHECK_RECEIPT_TYPE = "feppml-phase12-check-v1"
INDEX_TYPE = "feppml-phase12-aggregate-index-v1"
AGGREGATE_RECEIPT_TYPE = "feppml-phase12-aggregate-v1"
RESULT_RECEIPT_TYPE = "feppml-phase12-result-v1"
NATIVE_RECEIPT_TYPE = "feppml-native-platform-qualification-v1"
CHECK_SENTINEL = "FEPPML_PHASE12_CHECK_COMPLETE"
RESULT_SENTINEL = "FEPPML_PHASE12_RESULT_COMPLETE"
INDEX_SENTINEL = "FEPPML_PHASE12_INPUT_COMPLETE"
AGGREGATE_SENTINEL = "FEPPML_PHASE12_AGGREGATE_COMPLETE"
CANONICALIZATION = "json-sort-keys-compact-ascii-newline-v1"

HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
PUBLIC_STATES = {"preactivation_stub", "activated"}
ARTIFACT_PLATFORMS = {
    "source", "all", "macos-universal", "linux-x86_64", "windows-x86_64"
}

ROOT = Path(__file__).resolve().parents[1]


class GateError(ValueError):
    """The evidence set is incomplete, inconsistent, or not tamper-evident."""


COMMON_CHECK_CLAIMS: dict[str, tuple[str, ...]] = {
    "source_identity": (
        "clean_committed_source", "exact_commit", "exact_tree", "public_source_contract"
    ),
    "repository": ("repository_check",),
    "python": ("python_source_contracts",),
    "rust": ("rust_workspace_all_targets",),
    "fmt": ("cargo_fmt_check",),
    "clippy": ("warnings_denied_clippy",),
    "c_mock": ("c_mock_spi",),
    "ffi": ("ffi_feature_tests",),
    "native_build": (
        "linux_x86_64", "macos_universal", "source_embedded", "windows_x86_64"
    ),
    "native_audit": (
        "binary_policy", "linux_x86_64", "macos_universal", "windows_x86_64"
    ),
    "native_abi": (
        "abi_contract", "linux_x86_64", "macos_universal", "windows_x86_64"
    ),
    "fereg_f0": (
        "behavioral", "failure_atomicity", "memory", "package", "performance", "scientific"
    ),
    "feppml_qualification": (
        "comparator", "failure_atomicity", "memory", "oracle", "performance",
        "saved_outputs", "scientific"
    ),
    "platform_macos": (
        "artifact_bound", "licensed_stata_load", "macos_universal", "source_bound"
    ),
    "platform_linux": (
        "artifact_bound", "licensed_stata_load", "linux_x86_64", "source_bound"
    ),
    "platform_windows": (
        "artifact_bound", "licensed_stata_load", "source_bound", "windows_x86_64"
    ),
    "staged_release": (
        "archive_bound", "complete_inventory", "licensed_installation", "plugin_bound",
        "source_bound", "staged_fixture_sentinels"
    ),
    "scc_fereg_f0": (
        "accounting_complete", "qacct_success", "qualification_bound", "terminal_success"
    ),
    "scc_feppml": (
        "accounting_complete", "qacct_success", "qualification_bound", "terminal_success"
    ),
}


def _stata_claims(profile: str) -> tuple[str, ...]:
    return (
        "artifact_bound", "fixture_sentinels", "licensed_stata", f"profile:{profile}"
    )


# Kept explicit so an added/removed release profile necessarily changes this
# machine-reviewed inventory and its tests.
for _profile in (
    "syntax", "quick", "numerical-small", "differential", "fereg-regression",
    "ppml-hidden", "full", "ppml-fail-closed", "ppml-scientific",
):
    COMMON_CHECK_CLAIMS[f"stata_{_profile.replace('-', '_')}"] = _stata_claims(_profile)
COMMON_CHECK_CLAIMS["stata_break_fereg"] = (
    "artifact_bound", "break_observed", "licensed_stata", "receipt_last", "rollback"
)
COMMON_CHECK_CLAIMS["stata_break_feppml"] = (
    "artifact_bound", "break_observed", "licensed_stata", "receipt_last", "rollback"
)

COMMON_REQUIRED = frozenset(
    {
        "source_identity", "repository", "python", "rust", "fmt", "clippy", "c_mock",
        "ffi", "native_build", "native_audit", "native_abi", "stata_syntax",
        "stata_quick", "stata_numerical_small", "stata_differential",
        "stata_fereg_regression", "stata_ppml_hidden", "stata_full",
        "stata_break_fereg", "stata_break_feppml", "fereg_f0",
        "feppml_qualification", "platform_macos", "platform_linux",
        "platform_windows", "staged_release", "scc_fereg_f0", "scc_feppml",
    }
)
STATE_REQUIRED = {
    "preactivation_stub": COMMON_REQUIRED | {"stata_ppml_fail_closed"},
    "activated": COMMON_REQUIRED | {"stata_ppml_scientific"},
}

REQUIRED_ARTIFACT_KEYS = frozenset(
    {
        ("source_archive", "source"),
        ("plugin", "macos-universal"),
        ("plugin", "linux-x86_64"),
        ("plugin", "windows-x86_64"),
        ("release_archive", "all"),
    }
)

ACTIVATION_ALLOWED_PATHS = frozenset(
    {
        ".github/workflows/stata-ci.yml",
        "CHANGELOG.md", "README.md", "STATUS.md", "STATA_CI_RUNNER.md",
        "ci/stata_profiles.json", "docs/architecture.md",
        "docs/clean_machine_test_plan.md", "docs/compatibility_matrix.md",
        "docs/feppml_activation_checklist.md", "docs/phase12_activation_gate.md",
        "docs/plugin_protocol.md", "docs/release_process.md",
        "docs/scientific_contracts.md", "docs/user_guide.md",
        "scripts/stage_release.py", "stata/feppml.ado", "stata/feppml.sthlp",
        "stata/fereg.pkg", "stata/fereg.sthlp", "stata/stata.toc",
    }
)
ACTIVATION_REQUIRED_CHANGED_PATHS = frozenset(
    {
        ".github/workflows/stata-ci.yml",
        "CHANGELOG.md", "README.md", "STATUS.md", "STATA_CI_RUNNER.md",
        "ci/stata_profiles.json", "docs/architecture.md",
        "docs/clean_machine_test_plan.md", "docs/compatibility_matrix.md",
        "docs/plugin_protocol.md", "docs/release_process.md",
        "docs/scientific_contracts.md", "docs/user_guide.md",
        "scripts/stage_release.py", "stata/feppml.ado", "stata/feppml.sthlp",
        "stata/fereg.pkg", "stata/fereg.sthlp", "stata/stata.toc",
    }
)
PRIVATE_PPML_ADO_FILES = {
    "_fereg_ppml.ado", "_fereg_estimate_ppml.ado", "feppml_p.ado"
}
PRIVATE_PPML_ADO_ORDER = (
    "_fereg_ppml.ado", "_fereg_estimate_ppml.ado", "feppml_p.ado"
)
PUBLIC_PPML_FIXTURES = {
    "tests/stata/parity_ppmlhdfe/feppml_command_contract.do",
    "tests/stata/parity_ppmlhdfe/feppml_saved_slopes.do",
    "tests/stata/parity_ppmlhdfe/feppml_scientific_contract.do",
    "tests/stata/parity_ppmlhdfe/feppml_separation_methods.do",
    "tests/stata/parity_ppmlhdfe/feppml_separation_outputs.do",
    "tests/stata/parity_ppmlhdfe/feppml_time_series.do",
    "tests/stata/parity_ppmlhdfe/feppml_weights_inference.do",
}
PUBLIC_PPML_FIXTURE_ORDER = tuple(sorted(PUBLIC_PPML_FIXTURES))
ACTIVATED_HELP_FIXTURE = "tests/stata/documentation/feppml_help_examples.do"
FAIL_CLOSED_PPML_FIXTURE = "tests/stata/parity_ppmlhdfe/feppml_fail_closed.do"
FROZEN_PREACTIVATION_STAGER_SHA256 = (
    "d82c4a817122f926a205a73f2edd5aa24bdc30558a88ce7ab1ef3f8ae3d20c7c"
)


def canonical_json_bytes(value: Any) -> bytes:
    try:
        encoded = json.dumps(
            value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False
        )
    except (TypeError, ValueError) as error:
        raise GateError(f"value is not canonical finite JSON: {error}") from error
    return (encoded + "\n").encode("ascii")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise GateError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def seal_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "payload": dict(payload),
        "seal": {
            "algorithm": "sha256",
            "canonicalization": CANONICALIZATION,
            "payload_sha256": sha256_bytes(canonical_json_bytes(payload)),
        },
    }


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise GateError(f"{where} must be an object")
    return value


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise GateError(
            f"{where} is not closed; missing={sorted(fields - set(value))}, "
            f"extra={sorted(set(value) - fields)}"
        )


def _sha(value: Any, where: str, length: int) -> str:
    pattern = HEX_SHA1 if length == 40 else HEX_SHA256
    if not isinstance(value, str) or pattern.fullmatch(value) is None:
        raise GateError(f"{where} must be a lowercase {length}-hex digest")
    return value


def _integer(value: Any, where: str, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise GateError(f"{where} must be an integer >= {minimum}")
    return value


def _read_json(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise GateError(f"cannot read {where}: {error}") from error
    return _mapping(value, where)


def _source(value: Any, where: str = "source") -> dict[str, str]:
    record = _mapping(value, where)
    _closed(record, {"commit", "tree"}, where)
    return {
        "commit": _sha(record["commit"], f"{where}.commit", 40),
        "tree": _sha(record["tree"], f"{where}.tree", 40),
    }


ARTIFACT_FIELDS = {
    "role", "platform", "filename", "sha256", "size_bytes", "embedded_git_commit"
}
ARTIFACT_FILE_FIELDS = ARTIFACT_FIELDS | {"path"}


def _artifact_identity(value: Any, where: str, source: Mapping[str, str]) -> dict[str, Any]:
    record = _mapping(value, where)
    _closed(record, ARTIFACT_FIELDS, where)
    for field in ("role", "filename"):
        if not isinstance(record[field], str) or not record[field]:
            raise GateError(f"{where}.{field} must be nonempty")
    if record["platform"] not in ARTIFACT_PLATFORMS:
        raise GateError(f"{where}.platform is unsupported")
    _sha(record["sha256"], f"{where}.sha256", 64)
    _integer(record["size_bytes"], f"{where}.size_bytes", 1)
    if record["embedded_git_commit"] != source["commit"][:12]:
        raise GateError(f"{where} is not source-embedded")
    return dict(record)


def _artifact_file(
    value: Any, where: str, source: Mapping[str, str], base: Path
) -> tuple[dict[str, Any], dict[str, Any]]:
    record = _mapping(value, where)
    _closed(record, ARTIFACT_FILE_FIELDS, where)
    raw_path = Path(str(record["path"]))
    path = (base / raw_path).resolve() if not raw_path.is_absolute() else raw_path.resolve()
    identity = _artifact_identity(
        {field: record[field] for field in ARTIFACT_FIELDS}, where, source
    )
    if (
        raw_path.is_symlink() or path.is_symlink() or not path.is_file()
        or path.name != identity["filename"]
        or path.stat().st_size != identity["size_bytes"]
        or sha256_file(path) != identity["sha256"]
    ):
        raise GateError(f"{where} bytes are absent, substituted, or changed")
    if identity["role"] == "plugin" and source["commit"][:12].encode("ascii") not in path.read_bytes():
        raise GateError(f"{where} plugin bytes omit the exact embedded source commit")
    return identity, {**identity, "path": str(path)}


RECEIPT_RECORD_FIELDS = {"id", "path", "sha256", "size_bytes", "payload_sha256"}
CHECK_PAYLOAD_FIELDS = {
    "schema_version", "receipt_type", "check_id", "source", "public_state",
    "artifacts", "claims", "producer", "upstream", "status", "waivers",
    "complete_sentinel",
}
PRODUCER_FIELDS = {"id", "version", "entrypoint_sha256"}
UPSTREAM_FIELDS = {
    "role", "receipt_type", "encoding", "path", "file_sha256", "payload_sha256",
    "size_bytes"
}
RESULT_PAYLOAD_FIELDS = {
    "schema_version", "receipt_type", "check_id", "source", "public_state",
    "artifacts", "claims", "producer", "execution", "outputs", "status",
    "completed", "complete_sentinel",
}
RESULT_EXECUTION_FIELDS = {
    "mode", "argv", "argv_sha256", "exit_code", "stdout", "stderr",
    "terminal_sentinel", "environment", "toolchain", "profile",
}
STREAM_FIELDS = {"sha256", "size_bytes"}
ENVIRONMENT_FIELDS = {"os", "arch", "python", "variables"}
ENVIRONMENT_VARIABLE_FIELDS = {"name", "value_sha256"}
RESULT_OUTPUT_FIELDS = {"role", "path", "filename", "sha256", "size_bytes"}

GENERIC_COMMAND_CHECKS = frozenset(
    {
        "repository", "python", "rust", "fmt", "clippy", "c_mock", "ffi",
    }
)
STATA_PROFILE_CHECKS = frozenset(
    check_id for check_id in COMMON_CHECK_CLAIMS if check_id.startswith("stata_")
)

# A check receipt is not a bag of self-asserted booleans.  Each check id has
# one closed upstream inventory.  Roles are semantic and therefore also bind
# companion files needed by an authoritative verifier (for example a PPML
# evaluation receipt plus the evidence that must reproduce it).
UPSTREAM_ROLE_TYPES: dict[str, tuple[tuple[str, str], ...]] = {
    "source_identity": (),
    **{
        check_id: (("producer_result", RESULT_RECEIPT_TYPE),)
        for check_id in GENERIC_COMMAND_CHECKS | STATA_PROFILE_CHECKS
    },
    "native_build": (("native_qualification", NATIVE_RECEIPT_TYPE),),
    "native_audit": (("native_qualification", NATIVE_RECEIPT_TYPE),),
    "native_abi": (("native_qualification", NATIVE_RECEIPT_TYPE),),
    "fereg_f0": (("f0_qualification", "fereg-f0-regression-v1"),),
    "feppml_qualification": (
        ("qualification_evidence", "feppml-qualification-v1-evidence"),
        ("qualification_receipt", "feppml_qualification_evaluation"),
    ),
    "platform_macos": (
        ("platform_record", "feppml-platform-evidence-record-v1"),
        ("platform_receipt", "feppml_platform_licensed_stata_load"),
    ),
    "platform_linux": (
        ("linux_bootstrap_close", "feppml_linux_platform_bootstrap_close"),
        ("platform_record", "feppml-platform-evidence-record-v1"),
        ("platform_receipt", "feppml_platform_licensed_stata_load"),
    ),
    "platform_windows": (
        ("platform_record", "feppml-platform-evidence-record-v1"),
        ("platform_receipt", "feppml_platform_licensed_stata_load"),
    ),
    "staged_release": (("producer_result", RESULT_RECEIPT_TYPE),),
    "scc_fereg_f0": (("scc_close", "fereg-f0-scc-closed-run-v1"),),
    "scc_feppml": (("scc_close", "feppml_qualification_scc_close"),),
}

RAW_UPSTREAM_TYPES = frozenset(
    {
        "feppml-qualification-v1-evidence",
        "feppml-platform-evidence-record-v1",
        "fereg-windows-controller-qualification-v1",
    }
)


def result_sentinel(check_id: str) -> str:
    return f"FEPPML_PHASE12_{check_id.upper()}_PASS"


def validate_check_payload(value: Any) -> dict[str, Any]:
    payload = _mapping(value, "check payload")
    _closed(payload, CHECK_PAYLOAD_FIELDS, "check payload")
    check_id = payload["check_id"]
    if check_id not in COMMON_CHECK_CLAIMS:
        raise GateError(f"unsupported check id: {check_id!r}")
    source = _source(payload["source"], "check source")
    if payload["schema_version"] != SCHEMA_VERSION or payload["receipt_type"] != CHECK_RECEIPT_TYPE:
        raise GateError("check receipt identity is invalid")
    if payload["public_state"] not in PUBLIC_STATES:
        raise GateError("check public_state is invalid")
    artifacts = payload["artifacts"]
    if not isinstance(artifacts, list) or not artifacts:
        raise GateError("every check must bind the complete nonempty artifact inventory")
    normalized = [
        _artifact_identity(item, f"check artifacts[{index}]", source)
        for index, item in enumerate(artifacts)
    ]
    keys = [(item["role"], item["platform"]) for item in normalized]
    if len(keys) != len(set(keys)) or keys != sorted(keys):
        raise GateError("check artifact inventory must be unique and canonically ordered")
    claims = payload["claims"]
    if claims != list(COMMON_CHECK_CLAIMS[check_id]):
        raise GateError(f"{check_id} claims are incomplete or not canonical")
    producer = _mapping(payload["producer"], "check producer")
    _closed(producer, PRODUCER_FIELDS, "check producer")
    for field in ("id", "version"):
        if not isinstance(producer[field], str) or not producer[field]:
            raise GateError(f"check producer.{field} must be nonempty")
    _sha(producer["entrypoint_sha256"], "check producer.entrypoint_sha256", 64)
    upstream = payload["upstream"]
    if not isinstance(upstream, list):
        raise GateError("check upstream must be a list")
    roles: list[str] = []
    observed_role_types: list[tuple[str, str]] = []
    for index, raw in enumerate(upstream):
        item = _mapping(raw, f"check upstream[{index}]")
        _closed(item, UPSTREAM_FIELDS, f"check upstream[{index}]")
        for field in ("role", "receipt_type"):
            if not isinstance(item[field], str) or not item[field]:
                raise GateError(f"check upstream[{index}].{field} must be nonempty")
        if item["encoding"] not in {"sealed_json", "json"}:
            raise GateError(f"check upstream[{index}].encoding is unsupported")
        expected_encoding = (
            "json"
            if item["receipt_type"] in RAW_UPSTREAM_TYPES
            else "sealed_json"
        )
        if item["encoding"] != expected_encoding:
            raise GateError(
                f"check upstream[{index}] uses the wrong encoding for its receipt type"
            )
        _sha(item["file_sha256"], f"check upstream[{index}].file_sha256", 64)
        _sha(item["payload_sha256"], f"check upstream[{index}].payload_sha256", 64)
        _integer(item["size_bytes"], f"check upstream[{index}].size_bytes", 1)
        if not isinstance(item["path"], str) or not item["path"]:
            raise GateError(f"check upstream[{index}].path must be nonempty")
        roles.append(item["role"])
        observed_role_types.append((item["role"], item["receipt_type"]))
    if roles != sorted(set(roles)):
        raise GateError("check upstream roles must be unique and canonically ordered")
    if tuple(observed_role_types) != UPSTREAM_ROLE_TYPES[check_id]:
        raise GateError(
            f"{check_id} upstream inventory is not exact; "
            f"expected={UPSTREAM_ROLE_TYPES[check_id]!r}, "
            f"observed={tuple(observed_role_types)!r}"
        )
    if (
        payload["status"] != "pass" or payload["waivers"] != []
        or payload["complete_sentinel"] != CHECK_SENTINEL
    ):
        raise GateError(f"{check_id} did not close without waivers")
    return {
        **dict(payload), "source": source, "artifacts": normalized,
        "producer": dict(producer), "upstream": [dict(item) for item in upstream],
    }


def _sealed_upstream(path: Path, where: str) -> tuple[Mapping[str, Any], str, int, str]:
    try:
        raw = path.read_bytes()
        document = json.loads(raw)
    except (OSError, json.JSONDecodeError) as error:
        raise GateError(f"cannot read {where}: {error}") from error
    envelope = _mapping(document, where)
    _closed(envelope, {"payload", "seal"}, where)
    payload = _mapping(envelope["payload"], f"{where} payload")
    seal = _mapping(envelope["seal"], f"{where} seal")
    _closed(seal, {"algorithm", "canonicalization", "payload_sha256"}, f"{where} seal")
    if (
        seal["algorithm"] != "sha256"
        or seal["canonicalization"] not in {
            CANONICALIZATION, "json-sort-keys-compact-ascii-v1"
        }
        or seal["payload_sha256"] != sha256_bytes(canonical_json_bytes(payload))
    ):
        raise GateError(f"{where} seal mismatch")
    receipt_type = payload.get("receipt_type", payload.get("kind"))
    if not isinstance(receipt_type, str) or not receipt_type:
        raise GateError(f"{where} has no receipt type")
    return payload, sha256_bytes(raw), len(raw), receipt_type


def _raw_upstream(
    path: Path, where: str, expected_type: str
) -> tuple[Mapping[str, Any], str, int, str]:
    try:
        raw = path.read_bytes()
        payload = _mapping(json.loads(raw), where)
    except (OSError, json.JSONDecodeError) as error:
        raise GateError(f"cannot read {where}: {error}") from error
    observed = payload.get(
        "receipt_type",
        payload.get("kind", payload.get("evidence_type", payload.get("record_type"))),
    )
    if observed != expected_type:
        raise GateError(f"{where} has the wrong raw evidence type")
    return payload, sha256_bytes(raw), len(raw), expected_type


def _read_upstream(
    path: Path, where: str, expected_type: str
) -> tuple[Mapping[str, Any], str, int, str, str]:
    if expected_type in RAW_UPSTREAM_TYPES:
        payload, digest, size, receipt_type = _raw_upstream(
            path, where, expected_type
        )
        return payload, digest, size, receipt_type, "json"
    payload, digest, size, receipt_type = _sealed_upstream(path, where)
    return payload, digest, size, receipt_type, "sealed_json"


def validate_result_payload(
    value: Any,
    *,
    expected_check_id: str | None = None,
    expected_source: Mapping[str, str] | None = None,
    expected_public_state: str | None = None,
    expected_artifacts: Sequence[Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    payload = _mapping(value, "phase12 result payload")
    _closed(payload, RESULT_PAYLOAD_FIELDS, "phase12 result payload")
    check_id = payload["check_id"]
    if check_id not in COMMON_CHECK_CLAIMS:
        raise GateError("phase12 result check id is unsupported")
    source = _source(payload["source"], "phase12 result source")
    if (
        payload["schema_version"] != 1
        or payload["receipt_type"] != RESULT_RECEIPT_TYPE
        or payload["public_state"] not in PUBLIC_STATES
        or payload["claims"] != list(COMMON_CHECK_CLAIMS[check_id])
        or payload["status"] != "pass"
        or payload["completed"] is not True
        or payload["complete_sentinel"] != RESULT_SENTINEL
    ):
        raise GateError("phase12 result identity or pass verdict is invalid")
    artifacts_raw = payload["artifacts"]
    if not isinstance(artifacts_raw, list) or not artifacts_raw:
        raise GateError("phase12 result artifact inventory is empty")
    artifacts = [
        _artifact_identity(item, f"phase12 result artifacts[{index}]", source)
        for index, item in enumerate(artifacts_raw)
    ]
    keys = [(item["role"], item["platform"]) for item in artifacts]
    if keys != sorted(set(keys)):
        raise GateError("phase12 result artifacts are not unique/canonical")
    producer = _mapping(payload["producer"], "phase12 result producer")
    _closed(producer, PRODUCER_FIELDS, "phase12 result producer")
    if not all(isinstance(producer[field], str) and producer[field] for field in ("id", "version")):
        raise GateError("phase12 result producer identity is incomplete")
    _sha(producer["entrypoint_sha256"], "phase12 result producer entrypoint", 64)
    execution = _mapping(payload["execution"], "phase12 result execution")
    _closed(execution, RESULT_EXECUTION_FIELDS, "phase12 result execution")
    if execution["mode"] not in {
        "closed_argv", "licensed_stata_profile", "staged_release_installation"
    }:
        raise GateError("phase12 result execution mode is unsupported")
    argv = execution["argv"]
    if not isinstance(argv, list) or not all(isinstance(item, str) and item for item in argv):
        raise GateError("phase12 result argv is not a closed string array")
    expected_argv_sha = sha256_bytes(canonical_json_bytes(argv))
    if execution["argv_sha256"] != expected_argv_sha or execution["exit_code"] != 0:
        raise GateError("phase12 result argv identity or exit status is invalid")
    for field in ("stdout", "stderr"):
        stream = _mapping(execution[field], f"phase12 result {field}")
        _closed(stream, STREAM_FIELDS, f"phase12 result {field}")
        _sha(stream["sha256"], f"phase12 result {field}.sha256", 64)
        _integer(stream["size_bytes"], f"phase12 result {field}.size_bytes")
    if execution["terminal_sentinel"] != result_sentinel(check_id):
        raise GateError("phase12 result terminal sentinel is not check-specific")
    environment = _mapping(execution["environment"], "phase12 result environment")
    _closed(environment, ENVIRONMENT_FIELDS, "phase12 result environment")
    for field in ("os", "arch", "python"):
        if not isinstance(environment[field], str) or not environment[field]:
            raise GateError(f"phase12 result environment.{field} must be nonempty")
    variables = environment["variables"]
    if not isinstance(variables, list) or not variables:
        raise GateError("phase12 result must bind at least one selected environment variable")
    names: list[str] = []
    for index, raw in enumerate(variables):
        item = _mapping(raw, f"phase12 result environment.variables[{index}]")
        _closed(item, ENVIRONMENT_VARIABLE_FIELDS, f"phase12 result environment.variables[{index}]")
        if not isinstance(item["name"], str) or not item["name"]:
            raise GateError("phase12 result environment variable name is empty")
        _sha(item["value_sha256"], "phase12 result environment variable digest", 64)
        names.append(item["name"])
    if names != sorted(set(names)):
        raise GateError("phase12 result environment variables are not unique/canonical")
    for field in ("toolchain", "profile"):
        if not isinstance(execution[field], str) or not execution[field]:
            raise GateError(f"phase12 result execution.{field} must be nonempty")
    outputs = payload["outputs"]
    if not isinstance(outputs, list) or not outputs:
        raise GateError("phase12 result must bind at least one declared output")
    roles: list[str] = []
    for index, raw in enumerate(outputs):
        item = _mapping(raw, f"phase12 result outputs[{index}]")
        _closed(item, RESULT_OUTPUT_FIELDS, f"phase12 result outputs[{index}]")
        for field in ("role", "path", "filename"):
            if not isinstance(item[field], str) or not item[field]:
                raise GateError(f"phase12 result output {field} is empty")
        _sha(item["sha256"], f"phase12 result outputs[{index}].sha256", 64)
        _integer(item["size_bytes"], f"phase12 result outputs[{index}].size_bytes", 0)
        roles.append(item["role"])
    if roles != sorted(set(roles)):
        raise GateError("phase12 result output roles are not unique/canonical")
    if execution["mode"] == "closed_argv":
        if check_id not in GENERIC_COMMAND_CHECKS or not argv:
            raise GateError("generic closed-argv evidence is not eligible for this check")
    elif execution["mode"] == "licensed_stata_profile" and (
        check_id not in STATA_PROFILE_CHECKS or not argv
    ):
        raise GateError("licensed-Stata summary evidence is not eligible for this check")
    elif execution["mode"] == "staged_release_installation" and (
        check_id != "staged_release" or not argv
    ):
        raise GateError("staged-release evidence is not eligible for this check")
    if expected_check_id is not None and check_id != expected_check_id:
        raise GateError("phase12 result belongs to another check")
    if expected_source is not None and source != expected_source:
        raise GateError("phase12 result belongs to another source")
    if expected_public_state is not None and payload["public_state"] != expected_public_state:
        raise GateError("phase12 result belongs to another public state")
    if expected_artifacts is not None and artifacts != list(expected_artifacts):
        raise GateError("phase12 result belongs to another artifact inventory")
    return {**dict(payload), "source": source, "artifacts": artifacts}


def _verify_result_output_bytes(payload: Mapping[str, Any]) -> dict[str, Path]:
    paths: dict[str, Path] = {}
    for raw in payload["outputs"]:
        item = _mapping(raw, "phase12 result output")
        raw_path = Path(str(item["path"]))
        path = raw_path.resolve()
        if (
            raw_path.is_symlink() or path.is_symlink() or not path.is_file()
            or path.name != item["filename"]
            or path.stat().st_size != item["size_bytes"]
            or sha256_file(path) != item["sha256"]
        ):
            raise GateError(f"phase12 result output changed: {item['role']}")
        paths[str(item["role"])] = path
    return paths


def _validate_generic_result(
    payload: Mapping[str, Any], source_root: Path, source: Mapping[str, str]
) -> None:
    check_id = str(payload["check_id"])
    producer = payload["producer"]
    entrypoint = source_root / "scripts/feppml_phase12_local.py"
    if (
        producer != {
            "id": "feppml-phase12-local", "version": "1",
            "entrypoint_sha256": sha256_bytes(
                _git_blob(source_root, source["commit"], "scripts/feppml_phase12_local.py")
            ),
        }
    ):
        raise GateError("generic check result does not name the exact committed producer")
    outputs = _verify_result_output_bytes(payload)
    if "summary" not in outputs:
        raise GateError("generic check result omits its exact summary")
    summary = _read_json(outputs["summary"], "generic check summary")
    _closed(
        summary,
        {
            "schema_version", "result_type", "check_id", "source", "commands",
            "status", "complete_sentinel",
        },
        "generic check summary",
    )
    module = _source_module(
        source_root, source, "scripts/feppml_phase12_local.py",
        f"_feppml_phase12_local_{source['commit']}",
    )
    expected_commands = module.commands(check_id)
    summary_path = outputs["summary"]
    raw_root = summary_path.with_name(f"{summary_path.name}.raw")
    manifest_path = raw_root / "processes.json"
    outer_stdout = summary_path.with_name(f"{summary_path.name}.producer.stdout.log")
    outer_stderr = summary_path.with_name(f"{summary_path.name}.producer.stderr.log")
    manifest = _read_json(manifest_path, "generic raw process manifest")
    _closed(
        manifest,
        {
            "schema_version", "record_type", "check_id", "source", "commands",
            "toolchain", "status", "complete_sentinel",
        },
        "generic raw process manifest",
    )
    raw_commands = manifest["commands"]
    if not isinstance(raw_commands, list) or len(raw_commands) != len(expected_commands):
        raise GateError("generic raw process inventory is incomplete")
    expected_output_paths: dict[str, Path] = {
        "process_manifest": manifest_path.resolve(),
        "producer_stderr": outer_stderr.resolve(),
        "producer_stdout": outer_stdout.resolve(),
        "summary": summary_path.resolve(),
    }
    expected_processes = []
    for index, (argv, raw_record) in enumerate(zip(expected_commands, raw_commands)):
        record = _mapping(raw_record, f"generic raw command[{index}]")
        _closed(
            record,
            {"index", "argv", "argv_sha256", "exit_code", "stdout", "stderr"},
            f"generic raw command[{index}]",
        )
        streams: dict[str, dict[str, Any]] = {}
        for stream in ("stdout", "stderr"):
            stream_record = _mapping(
                record[stream], f"generic raw command[{index}] {stream}"
            )
            _closed(
                stream_record, {"filename", "sha256", "size_bytes"},
                f"generic raw command[{index}] {stream}",
            )
            path = raw_root / f"command-{index:03d}.{stream}.log"
            expected_stream = {
                "filename": path.name,
                "sha256": sha256_file(path),
                "size_bytes": path.stat().st_size,
            }
            if stream_record != expected_stream:
                raise GateError(f"generic raw command[{index}] {stream} bytes changed")
            role = f"command_{index:03d}_{stream}"
            expected_output_paths[role] = path.resolve()
            streams[stream] = expected_stream
        expected_processes.append(
            {
                "index": index,
                "argv": argv,
                "argv_sha256": sha256_bytes(canonical_json_bytes(argv)),
                "exit_code": 0,
                **streams,
            }
        )
        if record != expected_processes[-1]:
            raise GateError(f"generic raw command[{index}] did not pass exactly")
    expected_toolchain = module.toolchain(check_id)
    if (
        manifest != {
            "schema_version": 1,
            "record_type": "feppml-phase12-local-processes-v1",
            "check_id": check_id,
            "source": dict(source),
            "commands": expected_processes,
            "toolchain": expected_toolchain,
            "status": "pass",
            "complete_sentinel": result_sentinel(check_id),
        }
        or outer_stdout.read_bytes() != (result_sentinel(check_id) + "\n").encode("ascii")
        or outer_stderr.read_bytes() != b""
        or outputs != dict(sorted(expected_output_paths.items()))
    ):
        raise GateError("generic raw execution evidence is incomplete or noncanonical")
    if (
        summary != {
            "schema_version": 1,
            "result_type": "feppml-phase12-local-command-summary-v1",
            "check_id": check_id,
            "source": dict(source),
            "commands": expected_commands,
            "status": "pass",
            "complete_sentinel": result_sentinel(check_id),
        }
        or payload["execution"]["argv"] != [
            str(Path(sys.executable).resolve()), str(entrypoint.resolve()), check_id,
            "--output", str(outputs["summary"]),
            "--evidence-dir", str(raw_root.resolve()),
        ]
        or payload["execution"]["profile"] != f"phase12-local:{check_id}"
        or payload["execution"]["toolchain"] != expected_toolchain
        or payload["execution"]["stdout"] != {
            "sha256": sha256_file(outer_stdout),
            "size_bytes": outer_stdout.stat().st_size,
        }
        or payload["execution"]["stderr"] != {
            "sha256": sha256_file(outer_stderr),
            "size_bytes": outer_stderr.stat().st_size,
        }
    ):
        raise GateError("generic result differs from its committed argv/output registry")


def _artifact_for_platform(
    artifacts: Sequence[Mapping[str, Any]], platform_name: str
) -> Mapping[str, Any]:
    matches = [
        item for item in artifacts
        if item["role"] == "plugin" and item["platform"] == platform_name
    ]
    if len(matches) != 1:
        raise GateError(f"aggregate has no unique plugin for {platform_name}")
    return matches[0]


def _validate_upstream_outcome(
    check_id: str,
    inputs: Mapping[str, tuple[Mapping[str, Any], Path, str]],
    source: Mapping[str, str],
    public_state: str,
    artifacts: Sequence[Mapping[str, Any]],
    source_root: Path,
) -> None:
    """Invoke the authoritative verifier registered for one exact check."""
    if check_id == "source_identity":
        return
    if "producer_result" in inputs:
        payload, _path, receipt_type = inputs["producer_result"]
        if receipt_type != RESULT_RECEIPT_TYPE:
            raise GateError("producer result has the wrong receipt type")
        result = validate_result_payload(
            payload, expected_check_id=check_id, expected_source=source,
            expected_public_state=public_state, expected_artifacts=artifacts,
        )
        if check_id in GENERIC_COMMAND_CHECKS:
            _validate_generic_result(result, source_root, source)
        elif check_id in STATA_PROFILE_CHECKS:
            _validate_stata_result(result, source_root, source)
        elif check_id == "staged_release":
            _validate_staged_result(result, source_root, source)
        else:
            raise GateError("producer result has no authoritative verifier")
        return
    if check_id == "fereg_f0":
        _payload, path, _receipt_type = inputs["f0_qualification"]
        module = _source_module(
            source_root, source, "scripts/fereg_f0_regression.py",
            f"_fereg_f0_regression_{source['commit']}",
        )
        manifest_path = source_root / "benchmarks/manifests/fereg_f0_regression_v1.json"
        try:
            document = module.verify_receipt(path, manifest_path)
        except Exception as error:
            raise GateError(f"authoritative F0 verification failed: {error}") from error
        verified = _mapping(document.get("payload"), "verified F0 payload")
        evidence = _mapping(verified.get("evidence"), "verified F0 evidence")
        candidate = _mapping(evidence.get("candidate"), "verified F0 candidate")
        candidate_source = _mapping(candidate.get("source"), "verified F0 source")
        evaluation = _mapping(verified.get("evaluation"), "verified F0 evaluation")
        candidate_artifact = _mapping(
            candidate.get("artifact"), "verified F0 candidate artifact"
        )
        platform_name = candidate_artifact.get("platform")
        aggregate_artifact = (
            _artifact_for_platform(artifacts, str(platform_name))
            if platform_name in {"macos-universal", "linux-x86_64", "windows-x86_64"}
            else None
        )
        behavior = _mapping(
            evaluation.get("behavior"), "verified F0 licensed-host behavior"
        )
        behavior_source = _mapping(
            behavior.get("candidate_source"),
            "verified F0 licensed-host candidate source",
        )
        behavior_artifact = _mapping(
            behavior.get("candidate_artifact"),
            "verified F0 licensed-host candidate artifact",
        )
        behavior_platform = behavior_artifact.get("platform")
        aggregate_behavior_artifact = (
            _artifact_for_platform(artifacts, str(behavior_platform))
            if behavior_platform
            in {"macos-universal", "linux-x86_64", "windows-x86_64"}
            else None
        )
        if (
            candidate_source.get("commit") != source["commit"]
            or candidate_source.get("tree") != source["tree"]
            or aggregate_artifact is None
            or candidate_artifact.get("sha256") != aggregate_artifact["sha256"]
            or candidate_artifact.get("size_bytes") != aggregate_artifact["size_bytes"]
            or candidate_artifact.get("embedded_git_commit") != source["commit"][:12]
            or behavior_source.get("commit") != source["commit"]
            or behavior_source.get("tree") != source["tree"]
            or aggregate_behavior_artifact is None
            or behavior_artifact.get("sha256")
            != aggregate_behavior_artifact["sha256"]
            or behavior_artifact.get("size_bytes")
            != aggregate_behavior_artifact["size_bytes"]
            or behavior_artifact.get("embedded_git_commit")
            != source["commit"][:12]
            or evaluation.get("status") != "pass"
            or evaluation.get("stage") != "complete"
            or evaluation.get("performance_evaluated") is not True
        ):
            raise GateError("authoritative F0 result is incomplete or source-mixed")
        return
    if check_id == "feppml_qualification":
        _evaluation, receipt_path, _ = inputs["qualification_receipt"]
        _evidence, evidence_path, _ = inputs["qualification_evidence"]
        module = _source_module(
            source_root, source, "scripts/feppml_qualification.py",
            f"_feppml_qualification_{source['commit']}",
        )
        verifier = getattr(module, "verify_qualification_receipt", None)
        if verifier is None:
            raise GateError("qualification module has no authoritative recomputation verifier")
        try:
            result = verifier(
                source_root / "benchmarks/manifests/feppml_qualification_v1.json",
                evidence_path,
                receipt_path,
                source_root=source_root,
            )
        except Exception as error:
            raise GateError(f"authoritative feppml verification failed: {error}") from error
        candidate = _mapping(_evidence.get("candidate"), "verified feppml candidate")
        linux_artifact = _artifact_for_platform(artifacts, "linux-x86_64")
        source_archive = next(
            item for item in artifacts
            if item["role"] == "source_archive" and item["platform"] == "source"
        )
        if (
            result.get("status") != "pass" or result.get("stage") != "complete"
            or result.get("performance_evaluated") is not True
            or candidate.get("source_commit") != source["commit"]
            or candidate.get("source_tree") != source["tree"]
            or candidate.get("source_archive_sha256") != source_archive["sha256"]
            or candidate.get("source_archive_size_bytes") != source_archive["size_bytes"]
            or candidate.get("performance_plugin_sha256") != linux_artifact["sha256"]
            or candidate.get("performance_plugin_size_bytes") != linux_artifact["size_bytes"]
        ):
            raise GateError("authoritative feppml result is incomplete or source-mixed")
        return
    if check_id.startswith("platform_"):
        platform_name = {
            "platform_macos": "macos-universal",
            "platform_linux": "linux-x86_64",
            "platform_windows": "windows-x86_64",
        }[check_id]
        _receipt_payload, receipt_path, _ = inputs["platform_receipt"]
        _record_payload, record_path, _ = inputs["platform_record"]
        module = _source_module(
            source_root, source, "scripts/feppml_platform_load_receipt.py",
            f"_feppml_platform_receipt_{source['commit']}",
        )
        try:
            verified = module.verify(receipt_path, record_path)
        except Exception as error:
            raise GateError(f"authoritative platform verification failed: {error}") from error
        artifact = _artifact_for_platform(artifacts, platform_name)
        source_archive = next(
            item for item in artifacts
            if item["role"] == "source_archive" and item["platform"] == "source"
        )
        if (
            verified.get("qualification_profile") != "candidate"
            or verified.get("platform") != platform_name
            or verified.get("artifact_sha256") != artifact["sha256"]
            or verified.get("artifact_size_bytes") != artifact["size_bytes"]
            or verified.get("source_commit") != source["commit"]
            or verified.get("source_tree") != source["tree"]
            or verified.get("source_archive_sha256") != source_archive["sha256"]
            or verified.get("source_archive_size_bytes") != source_archive["size_bytes"]
            or verified.get("build_date_utc") != _commit_build_date(
                source_root, source["commit"]
            )
            or (
                platform_name != "windows-x86_64"
                and (
                    verified.get("controller_commit") != source["commit"]
                    or verified.get("controller_tree") != source["tree"]
                )
            )
        ):
            raise GateError("platform evidence is source/archive/artifact mixed")
        if check_id == "platform_linux":
            _close, close_path, _ = inputs["linux_bootstrap_close"]
            scc = _source_module(
                source_root, source, "benchmarks/scc/feppml_linux_platform_scc.py",
                f"_feppml_linux_platform_scc_{source['commit']}",
            )
            verifier = getattr(scc, "verify_closed_run", None)
            if verifier is None:
                raise GateError("Linux platform module has no offline closed-run verifier")
            try:
                verified_run = verifier(
                    close_path.parent.parent,
                    candidate_repo=source_root,
                    harness_repo=source_root,
                    expected_source_commit=source["commit"],
                    expected_harness_commit=source["commit"],
                )
            except Exception as error:
                raise GateError(
                    f"authoritative Linux platform closed-run verification failed: {error}"
                ) from error
            if (
                verified_run.get("source", {}).get("commit") != source["commit"]
                or verified_run.get("build_date_utc") != _commit_build_date(
                    source_root, source["commit"]
                )
                or verified_run.get("artifact", {}).get("sha256") != artifact["sha256"]
                or verified_run.get("artifact", {}).get("size_bytes")
                != artifact["size_bytes"]
            ):
                raise GateError("Linux platform closed run belongs to another source")
        if check_id == "platform_windows":
            provenance = _mapping(
                verified.get("platform_provenance"), "Windows platform provenance"
            )
            if (
                provenance.get("record_type")
                != "fereg-windows-controller-qualification-v1"
                or provenance.get("target_source_commit") != source["commit"]
                or provenance.get("target_source_tree") != source["tree"]
                or provenance.get("harness_commit") != source["commit"]
                or provenance.get("harness_tree") != source["tree"]
                or provenance.get("status") != "PASS"
                or provenance.get("instance_stopped") is not True
                or provenance.get("transient_objects_deleted") is not True
                or provenance.get("lock_released") is not True
            ):
                raise GateError("Windows controller evidence is not an accepted closed PASS")
        return
    if check_id in {"scc_fereg_f0", "scc_feppml"}:
        _close, close_path, _ = inputs["scc_close"]
        relative = {
            "scc_fereg_f0": "benchmarks/scc/fereg_f0_scc.py",
            "scc_feppml": "benchmarks/scc/feppml_qualification_scc.py",
        }[check_id]
        module = _source_module(
            source_root, source, relative,
            f"_phase12_{check_id}_{source['commit']}",
        )
        verifier = getattr(module, "verify_closed_run", None)
        if verifier is None:
            raise GateError(f"{check_id} module has no offline closed-run verifier")
        run_root = close_path.parent.parent
        try:
            verified = (
                verifier(
                    run_root,
                    source_root / "benchmarks/manifests/fereg_f0_regression_v1.json",
                )
                if check_id == "scc_fereg_f0"
                else verifier(run_root)
            )
        except Exception as error:
            raise GateError(f"authoritative {check_id} verification failed: {error}") from error
        candidate = _mapping(verified.get("candidate"), f"{check_id} candidate")
        candidate_source = _mapping(
            candidate.get("source", candidate), f"{check_id} candidate source"
        )
        candidate_commit = candidate_source.get("commit")
        source_archive = next(
            item for item in artifacts
            if item["role"] == "source_archive" and item["platform"] == "source"
        )
        if candidate_commit != source["commit"]:
            raise GateError(f"{check_id} closed run belongs to another source")
        if check_id == "scc_fereg_f0":
            artifact = _mapping(
                candidate.get("artifact"), "scc_fereg_f0 candidate artifact"
            )
            platform_name = artifact.get("platform")
            aggregate_artifact = (
                _artifact_for_platform(artifacts, str(platform_name))
                if platform_name in {
                    "macos-universal", "linux-x86_64", "windows-x86_64"
                }
                else None
            )
            if (
                aggregate_artifact is None
                or artifact.get("sha256") != aggregate_artifact["sha256"]
                or artifact.get("size_bytes") != aggregate_artifact["size_bytes"]
            ):
                raise GateError("scc_fereg_f0 uses another candidate artifact")
        elif (
            candidate_source.get("tree") != source["tree"]
            or candidate_source.get("archive_sha256") != source_archive["sha256"]
            or candidate_source.get("archive_size_bytes") != source_archive["size_bytes"]
        ):
            raise GateError("scc_feppml uses another candidate source archive")
        return
    if check_id in {"native_build", "native_audit", "native_abi"}:
        _payload, path, _receipt_type = inputs["native_qualification"]
        module = _source_module(
            source_root, source, "scripts/feppml_native_qualification.py",
            f"_feppml_native_qualification_{source['commit']}",
        )
        verifier = getattr(module, "verify", None)
        if verifier is None:
            raise GateError("native qualification module has no authoritative verifier")
        try:
            verified = verifier(
                path, source_root=source_root, expected_source=source,
                expected_artifacts=artifacts,
            )
        except Exception as error:
            raise GateError(f"authoritative native verification failed: {error}") from error
        if (
            verified.get("status") != "pass"
            or verified.get("claims") != ["build", "binary_audit", "actual_abi_loader"]
        ):
            raise GateError("native qualification is incomplete")
        return
    raise GateError(f"check has no authoritative verifier: {check_id}")


def _verify_check_upstreams(payload: Mapping[str, Any], source_root: Path) -> None:
    inputs: dict[str, tuple[Mapping[str, Any], Path, str]] = {}
    for record in payload["upstream"]:
        raw_path = Path(record["path"])
        path = raw_path.resolve()
        if raw_path.is_symlink() or path.is_symlink() or not path.is_file():
            raise GateError(f"upstream is absent or a symlink: {record['role']}")
        upstream, file_sha, size_bytes, receipt_type, encoding = _read_upstream(
            path, f"upstream {record['role']}", str(record["receipt_type"])
        )
        if (
            receipt_type != record["receipt_type"]
            or encoding != record["encoding"]
            or file_sha != record["file_sha256"]
            or size_bytes != record["size_bytes"]
            or sha256_bytes(canonical_json_bytes(upstream)) != record["payload_sha256"]
        ):
            raise GateError(f"upstream bytes changed: {record['role']}")
        inputs[str(record["role"])] = (upstream, path, receipt_type)
    _validate_upstream_outcome(
        payload["check_id"], inputs, payload["source"], payload["public_state"],
        payload["artifacts"], source_root.resolve(),
    )


def verify_check_receipt(
    path: Path, *, source_root: Path | None = None
) -> tuple[dict[str, Any], str, int]:
    raw = path.read_bytes()
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as error:
        raise GateError(f"check receipt is not JSON: {path}: {error}") from error
    envelope = _mapping(document, f"check receipt {path}")
    _closed(envelope, {"payload", "seal"}, f"check receipt {path}")
    payload = validate_check_payload(envelope["payload"])
    expected = seal_payload(envelope["payload"])["seal"]
    if envelope["seal"] != expected:
        raise GateError(f"check receipt seal mismatch: {path}")
    if source_root is None:
        raise GateError("check verification requires the exact committed source root")
    _verify_check_upstreams(payload, source_root)
    return payload, sha256_bytes(raw), len(raw)


def _git(root: Path, args: Sequence[str]) -> str:
    try:
        return subprocess.check_output(
            ["git", "-C", str(root), *args], stderr=subprocess.STDOUT
        ).decode("utf-8", errors="strict").strip()
    except (OSError, subprocess.CalledProcessError, UnicodeDecodeError) as error:
        raise GateError(f"git source audit failed: {error}") from error


def _git_blob(root: Path, commit: str, path: str) -> bytes:
    try:
        return subprocess.check_output(
            ["git", "-C", str(root), "show", f"{commit}:{path}"], stderr=subprocess.STDOUT
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise GateError(f"committed source omits {path}") from error


def _git_archive(root: Path, commit: str) -> bytes:
    try:
        return subprocess.check_output(
            ["git", "-C", str(root), "archive", "--format=tar", commit],
            stderr=subprocess.STDOUT,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise GateError(f"cannot materialize exact committed source archive: {error}") from error


def _commit_build_date(root: Path, commit: str) -> str:
    raw = _git(root, ["show", "-s", "--format=%cI", commit])
    try:
        timestamp = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError as error:
        raise GateError("target commit timestamp is not exact ISO-8601") from error
    if timestamp.tzinfo is None:
        raise GateError("target commit timestamp has no timezone")
    return timestamp.astimezone(timezone.utc).strftime("%Y-%m-%d")


def _source_module(
    root: Path, source: Mapping[str, str], relative: str, name: str
) -> Any:
    path = (root / relative).resolve()
    if not path.is_file() or root.resolve() not in path.parents:
        raise GateError(f"authoritative verifier is absent: {relative}")
    committed = _git_blob(root, source["commit"], relative)
    if sha256_file(path) != sha256_bytes(committed):
        raise GateError(f"authoritative verifier differs from committed source: {relative}")
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise GateError(f"cannot load authoritative verifier: {relative}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    previous_dont_write_bytecode = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    except Exception as error:
        raise GateError(f"cannot initialize authoritative verifier {relative}: {error}") from error
    finally:
        sys.dont_write_bytecode = previous_dont_write_bytecode
    return module


def _verify_exact_source_archive(
    root: Path, source: Mapping[str, str], artifact: Mapping[str, Any]
) -> None:
    path = Path(str(artifact["path"])).resolve()
    if path.name != "source.tar":
        raise GateError("canonical source archive must be the raw deterministic source.tar")
    expected = _git_archive(root, source["commit"])
    if (
        path.read_bytes() != expected
        or artifact["sha256"] != sha256_bytes(expected)
        or artifact["size_bytes"] != len(expected)
    ):
        raise GateError("source archive bytes do not equal git archive of the aggregate commit")


def audit_source(root: Path, source: Mapping[str, str], public_state: str) -> dict[str, Any]:
    root = root.resolve()
    if public_state not in PUBLIC_STATES:
        raise GateError("public_state is invalid")
    commit = _git(root, ["rev-parse", "HEAD^{commit}"])
    tree = _git(root, ["rev-parse", "HEAD^{tree}"])
    if commit != source["commit"] or tree != source["tree"]:
        raise GateError("aggregate source is not the checked-out full HEAD commit/tree")
    if _git(root, ["status", "--porcelain=v1", "--untracked-files=all"]):
        raise GateError("aggregate source checkout is not completely clean")

    feppml = _git_blob(root, commit, "stata/feppml.ado")
    package = _git_blob(root, commit, "stata/fereg.pkg")
    stager = _git_blob(root, commit, "scripts/stage_release.py")
    profiles_raw = _git_blob(root, commit, "ci/stata_profiles.json")
    try:
        profiles = json.loads(profiles_raw)
    except json.JSONDecodeError as error:
        raise GateError("committed Stata profile registry is invalid JSON") from error
    profile_names = set(_mapping(profiles, "Stata profiles").get("profiles", {}))
    private = ("_fereg_ppml.ado", "_fereg_estimate_ppml.ado", "feppml_p.ado")
    package_text = package.decode("utf-8", errors="strict")
    stager_text = stager.decode("utf-8", errors="strict")
    feppml_text = feppml.decode("utf-8", errors="strict")
    package_files = _package_files(package, "committed fereg.pkg")
    stage_files = _production_stata_files(stager, "committed release stager")

    if public_state == "preactivation_stub":
        if sha256_bytes(stager) != FROZEN_PREACTIVATION_STAGER_SHA256:
            raise GateError(
                "preactivation release stager differs from its frozen F0 bytes"
            )
        if (
            "exit 498" not in feppml_text or "_fereg_ppml" in feppml_text
            or any(name in package_files for name in private)
            or any(name in stage_files for name in private)
            or "ppml-fail-closed" not in profile_names
            or "ppml-scientific" in profile_names
        ):
            raise GateError("committed public source does not satisfy preactivation_stub")
        verdict = "fail_closed_r498_private_files_unpublished"
    else:
        substantive = [
            line.strip() for line in feppml_text.splitlines()
            if line.strip() and not line.lstrip().startswith("*!")
        ]
        expected = [
            "program define feppml, eclass sortpreserve",
            "version 18.0",
            "_fereg_ppml `0'",
            "end",
        ]
        if (
            substantive != expected
            or tuple(name for name in package_files if name in PRIVATE_PPML_ADO_FILES)
            != PRIVATE_PPML_ADO_ORDER
            or tuple(name for name in stage_files if name in PRIVATE_PPML_ADO_FILES)
            != PRIVATE_PPML_ADO_ORDER
            or "ppml-scientific" not in profile_names
            or "ppml-fail-closed" in profile_names
        ):
            raise GateError("committed public source does not satisfy activated")
        verdict = "one_call_wrapper_private_files_published"
    return {
        "status": "pass", "public_state": public_state, "verdict": verdict,
        "files": [
            {"path": path, "sha256": sha256_bytes(content)}
            for path, content in (
                ("ci/stata_profiles.json", profiles_raw),
                ("scripts/stage_release.py", stager),
                ("stata/feppml.ado", feppml),
                ("stata/fereg.pkg", package),
            )
        ],
    }


def _git_blob_optional(root: Path, commit: str, path: str) -> bytes | None:
    completed = subprocess.run(
        ["git", "-C", str(root), "show", f"{commit}:{path}"],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
    )
    return completed.stdout if completed.returncode == 0 else None


def _production_stata_files(blob: bytes, where: str) -> tuple[str, ...]:
    try:
        tree = ast.parse(blob.decode("utf-8", errors="strict"))
    except (UnicodeDecodeError, SyntaxError) as error:
        raise GateError(f"{where} is not valid Python") from error
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "PRODUCTION_STATA_FILES"
            for target in node.targets
        ):
            try:
                value = ast.literal_eval(node.value)
            except (ValueError, TypeError) as error:
                raise GateError(f"{where} production inventory is not literal") from error
            if (
                not isinstance(value, tuple)
                or not all(isinstance(item, str) and item for item in value)
                or len(value) != len(set(value))
            ):
                raise GateError(f"{where} production inventory is not a string tuple")
            return value
    raise GateError(f"{where} has no PRODUCTION_STATA_FILES assignment")


def _replace_activation_fragment(
    value: bytes, old: bytes, new: bytes, where: str
) -> bytes:
    if value.count(old) != 1:
        raise GateError(f"parent release stager lacks one exact {where} fragment")
    return value.replace(old, new, 1)


def _expected_activated_stager(parent: bytes) -> bytes:
    """Return the sole reviewed executable release-stager transformation."""

    value = _replace_activation_fragment(
        parent,
        b'    "feppml.ado",\n',
        (
            b'    "feppml.ado",\n'
            b'    "_fereg_ppml.ado",\n'
            b'    "_fereg_estimate_ppml.ado",\n'
            b'    "feppml_p.ado",\n'
        ),
        "private PPML inventory insertion",
    )
    value = _replace_activation_fragment(
        value,
        (
            b'    "Licensed Windows Stata qualification is deferred; Windows x64 is "\n'
            b'    "qualified by hosted MSVC build, binary audit, and mock SPI only.",\n'
        ),
        b"",
        "obsolete Windows caveat",
    )
    value = _replace_activation_fragment(
        value,
        (
            b'    "feppml is installed only as a fail-closed r(498) stub and is not part "\n'
            b'    "of the OLS qualification.",\n'
        ),
        b"",
        "obsolete fail-closed PPML caveat",
    )
    value = _replace_activation_fragment(
        value,
        b'        "ols_result_wire": "1.4",\n',
        (
            b'        "ols_result_wire": "1.4",\n'
            b'        "ppml_request_wire": "1.5",\n'
            b'        "ppml_result_wire": "1.2",\n'
        ),
        "additive PPML manifest wire insertion",
    )
    return value


def _require_exact_activation_stager(parent: bytes, active: bytes) -> None:
    if active != _expected_activated_stager(parent):
        raise GateError(
            "activation release stager is not the exact reviewed inventory, "
            "caveat, and additive-wire transform"
        )


def _package_files(blob: bytes, where: str) -> tuple[str, ...]:
    try:
        lines = blob.decode("utf-8", errors="strict").splitlines()
    except UnicodeDecodeError as error:
        raise GateError(f"{where} is not UTF-8") from error
    files = tuple(line[2:].strip() for line in lines if line.startswith("f "))
    if not files or any(not name for name in files) or len(files) != len(set(files)):
        raise GateError(f"{where} package inventory is malformed")
    return files


def _changed_paths(root: Path, parent: str, commit: str) -> list[dict[str, Any]]:
    raw = subprocess.check_output(
        [
            "git", "-C", str(root), "diff-tree", "--no-commit-id", "--name-status",
            "-r", "-z", parent, commit,
        ]
    )
    fields = raw.split(b"\0")
    if fields and fields[-1] == b"":
        fields.pop()
    if len(fields) % 2:
        raise GateError("activation diff name/status stream is malformed")
    records = []
    for index in range(0, len(fields), 2):
        status = fields[index].decode("ascii", errors="strict")
        path = fields[index + 1].decode("utf-8", errors="strict")
        if status not in {"A", "M"}:
            raise GateError(f"activation diff uses forbidden rename/copy/type status: {status}")
        if path not in ACTIVATION_ALLOWED_PATHS:
            raise GateError(f"activation changed a forbidden path: {path}")
        before = _git_blob_optional(root, parent, path)
        after = _git_blob_optional(root, commit, path)
        records.append(
            {
                "status": status, "path": path,
                "before_sha256": sha256_bytes(before) if before is not None else None,
                "after_sha256": sha256_bytes(after) if after is not None else None,
            }
        )
    paths = {item["path"] for item in records}
    if not ACTIVATION_REQUIRED_CHANGED_PATHS.issubset(paths):
        raise GateError(
            "activation diff omits required control-plane changes: "
            f"{sorted(ACTIVATION_REQUIRED_CHANGED_PATHS - paths)}"
        )
    if any(
        item["status"] != "M" and item["path"] in ACTIVATION_REQUIRED_CHANGED_PATHS
        for item in records
    ):
        raise GateError("activation replaced rather than modified a required control-plane file")
    return sorted(records, key=lambda item: item["path"])


def _require_exact_activation_workflow(parent: bytes, active: bytes) -> None:
    old = b"          - ppml-fail-closed\n"
    new = b"          - ppml-scientific\n"
    if (
        parent.count(old) != 1 or parent.count(new) != 0
        or active != parent.replace(old, new)
        or active.count(new) != 1 or active.count(old) != 0
    ):
        raise GateError(
            "activation workflow is not the exact fail-closed to scientific choice transform"
        )


PREACTIVATION_RECEIPT_RECORD_FIELDS = {
    "receipt_path", "receipt_sha256", "receipt_size_bytes",
    "receipt_payload_sha256", "index_path", "index_sha256", "index_size_bytes",
    "source_root",
}


def audit_activation_lineage(
    root: Path,
    source: Mapping[str, str],
    public_state: str,
    lineage: Any,
    *,
    index_base: Path,
) -> dict[str, Any] | None:
    if public_state == "preactivation_stub":
        if lineage is not None:
            raise GateError("preactivation index must not claim activation lineage")
        return None
    record = _mapping(lineage, "activation lineage")
    _closed(record, PREACTIVATION_RECEIPT_RECORD_FIELDS, "activation lineage")
    raw_path = Path(str(record["receipt_path"]))
    path = (index_base / raw_path).resolve() if not raw_path.is_absolute() else raw_path.resolve()
    if raw_path.is_symlink() or path.is_symlink() or not path.is_file():
        raise GateError("preactivation aggregate receipt is absent or a symlink")
    raw_index = Path(str(record["index_path"]))
    prior_index = (
        (index_base / raw_index).resolve()
        if not raw_index.is_absolute() else raw_index.resolve()
    )
    parent_root_raw = Path(str(record["source_root"]))
    parent_root = (
        (index_base / parent_root_raw).resolve()
        if not parent_root_raw.is_absolute() else parent_root_raw.resolve()
    )
    if (
        raw_index.is_symlink() or prior_index.is_symlink() or not prior_index.is_file()
        or parent_root_raw.is_symlink() or parent_root.is_symlink()
        or not (parent_root / ".git").exists()
    ):
        raise GateError("preactivation lineage index or source checkout is absent/unsafe")
    if (
        record["index_sha256"] != sha256_file(prior_index)
        or record["index_size_bytes"] != prior_index.stat().st_size
    ):
        raise GateError("preactivation lineage index bytes changed")
    raw = path.read_bytes()
    prior = verify_aggregate_receipt(path, prior_index, parent_root)
    expected_record = {
        "receipt_path": record["receipt_path"],
        "receipt_sha256": sha256_bytes(raw),
        "receipt_size_bytes": len(raw),
        "receipt_payload_sha256": sha256_bytes(canonical_json_bytes(prior)),
    }
    for field, value in expected_record.items():
        if record[field] != value:
            raise GateError("preactivation aggregate receipt file binding changed")
    if prior["public_state"] != "preactivation_stub":
        raise GateError("activation lineage does not name a preactivation aggregate")
    parents = _git(root, ["rev-list", "--parents", "-n", "1", source["commit"]]).split()
    if len(parents) != 2:
        raise GateError("activation commit must have exactly one parent")
    parent = parents[1]
    if prior["source"]["commit"] != parent:
        raise GateError("passing preactivation aggregate is not the exact activation parent")
    parent_tree = _git(root, ["rev-parse", f"{parent}^{{tree}}"])
    if prior["source"]["tree"] != parent_tree:
        raise GateError("preactivation aggregate parent tree is mixed or stale")
    changes = _changed_paths(root, parent, source["commit"])
    _require_exact_activation_workflow(
        _git_blob(root, parent, ".github/workflows/stata-ci.yml"),
        _git_blob(root, source["commit"], ".github/workflows/stata-ci.yml"),
    )

    for name in sorted(PRIVATE_PPML_ADO_FILES):
        path = f"stata/{name}"
        before = _git_blob_optional(root, parent, path)
        after = _git_blob_optional(root, source["commit"], path)
        if before is None or after != before:
            raise GateError(f"activation changed or lacked prequalified private ado: {path}")
    for fixture in sorted(PUBLIC_PPML_FIXTURES | {ACTIVATED_HELP_FIXTURE}):
        before = _git_blob_optional(root, parent, fixture)
        after = _git_blob_optional(root, source["commit"], fixture)
        if before is None or after != before:
            raise GateError(
                f"activation changed or lacked a prequalified shared fixture: {fixture}"
            )

    parent_pkg = _package_files(_git_blob(root, parent, "stata/fereg.pkg"), "parent fereg.pkg")
    active_pkg = _package_files(
        _git_blob(root, source["commit"], "stata/fereg.pkg"), "activated fereg.pkg"
    )
    if (
        tuple(item for item in active_pkg if item not in PRIVATE_PPML_ADO_FILES)
        != parent_pkg
        or tuple(item for item in active_pkg if item in PRIVATE_PPML_ADO_FILES)
        != PRIVATE_PPML_ADO_ORDER
    ):
        raise GateError("activation package inventory changed beyond the three private PPML ado files")
    parent_stage_blob = _git_blob(root, parent, "scripts/stage_release.py")
    active_stage_blob = _git_blob(
        root, source["commit"], "scripts/stage_release.py"
    )
    _require_exact_activation_stager(parent_stage_blob, active_stage_blob)
    parent_stage = _production_stata_files(
        parent_stage_blob, "parent release stager"
    )
    active_stage = _production_stata_files(
        active_stage_blob, "activated release stager"
    )
    if (
        tuple(item for item in active_stage if item not in PRIVATE_PPML_ADO_FILES)
        != parent_stage
        or tuple(item for item in active_stage if item in PRIVATE_PPML_ADO_FILES)
        != PRIVATE_PPML_ADO_ORDER
    ):
        raise GateError("activation staging inventory changed beyond the three private PPML ado files")

    try:
        parent_profiles = json.loads(_git_blob(root, parent, "ci/stata_profiles.json"))["profiles"]
        active_profiles = json.loads(_git_blob(root, source["commit"], "ci/stata_profiles.json"))["profiles"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise GateError("activation Stata profile diff is malformed") from error
    mutable_profiles = {"quick", "full", "ppml-fail-closed", "ppml-scientific"}
    for name in set(parent_profiles) | set(active_profiles):
        if name not in mutable_profiles and parent_profiles.get(name) != active_profiles.get(name):
            raise GateError(f"activation changed a non-public Stata profile: {name}")
    for name in ("quick", "full"):
        parent_spec = dict(_mapping(parent_profiles.get(name), f"parent {name} profile"))
        active_spec = dict(_mapping(active_profiles.get(name), f"activated {name} profile"))
        parent_fixtures = parent_spec.pop("fixtures", None)
        active_fixtures = active_spec.pop("fixtures", None)
        parent_timeout = parent_spec.pop("timeout_seconds", None)
        active_timeout = active_spec.pop("timeout_seconds", None)
        if (
            not isinstance(parent_fixtures, list)
            or not isinstance(active_fixtures, list)
            or len(parent_fixtures) != len(set(parent_fixtures))
            or len(active_fixtures) != len(set(active_fixtures))
            or parent_spec != active_spec
            or isinstance(parent_timeout, bool)
            or not isinstance(parent_timeout, int)
            or isinstance(active_timeout, bool)
            or not isinstance(active_timeout, int)
            or active_timeout < parent_timeout
        ):
            raise GateError(f"activation weakened or replaced the {name} profile")
        retained = [
            fixture for fixture in parent_fixtures
            if fixture != FAIL_CLOSED_PPML_FIXTURE
        ]
        additions = (
            [*PUBLIC_PPML_FIXTURE_ORDER, ACTIVATED_HELP_FIXTURE]
            if name == "full" else []
        )
        if active_fixtures != retained + additions:
            raise GateError(
                f"activation {name} fixture inventory is not the exact additive transform"
            )
    science = _mapping(active_profiles.get("ppml-scientific"), "public PPML profile")
    fixtures = science.get("fixtures")
    if (
        set(science) != {
            "driver", "build_plugin", "fixtures", "timeout_seconds",
            "required_outputs",
        }
        or science.get("driver") != "fereg-fixtures"
        or science.get("build_plugin") is not True
        or fixtures != list(PUBLIC_PPML_FIXTURE_ORDER)
        or isinstance(science.get("timeout_seconds"), bool)
        or not isinstance(science.get("timeout_seconds"), int)
        or science["timeout_seconds"] < 900
        or science.get("required_outputs")
        != parent_profiles["full"].get("required_outputs")
    ):
        raise GateError("public PPML profile does not contain the exact qualified fixture set")
    full = _mapping(active_profiles.get("full"), "activated full profile")
    full_fixtures = full.get("fixtures")
    if (
        not isinstance(full_fixtures, list) or not PUBLIC_PPML_FIXTURES.issubset(full_fixtures)
        or ACTIVATED_HELP_FIXTURE not in full_fixtures
        or full.get("run_break_fixture") is not True
        or full.get("run_ppml_break_fixture") is not True
    ):
        raise GateError("activated full profile omits public PPML or Break coverage")
    if "ppml-fail-closed" in active_profiles:
        raise GateError("activated profile registry retains ppml-fail-closed")
    if any(
        FAIL_CLOSED_PPML_FIXTURE in spec.get("fixtures", [])
        for spec in active_profiles.values() if isinstance(spec, Mapping)
    ):
        raise GateError("activated profile registry still schedules the public fail-closed fixture")

    public_help = _git_blob(root, source["commit"], "stata/feppml.sthlp")
    qualified_help = _git_blob(
        root, source["commit"], "tests/stata/documentation/feppml_activation.sthlp"
    )
    help_contract = _source_module(
        root, source, "scripts/feppml_help_contract.py",
        f"_phase12_help_contract_{source['commit']}",
    )
    try:
        help_contract.require_activated_public_help_bytes(public_help, qualified_help)
    except Exception as error:
        raise GateError(
            f"activated public help fails its exact semantic contract: {error}"
        ) from error
    stage_blob = active_stage_blob
    try:
        stage_tree = ast.parse(stage_blob.decode("utf-8", errors="strict"))
        support_caveats: tuple[str, ...] | None = None
        for node in stage_tree.body:
            if isinstance(node, (ast.Assign, ast.AnnAssign)):
                targets = node.targets if isinstance(node, ast.Assign) else [node.target]
                if any(isinstance(target, ast.Name) and target.id == "SUPPORT_CAVEATS" for target in targets):
                    literal = ast.literal_eval(node.value)
                    if isinstance(literal, tuple) and all(
                        isinstance(item, str) for item in literal
                    ):
                        support_caveats = literal
                    break
    except (UnicodeDecodeError, SyntaxError, ValueError, TypeError) as error:
        raise GateError("activated release support caveats are not literal") from error
    if support_caveats is None or any(
        re.search(r"(?:r\(498\)|fail[- ]closed|ols[- ]only|not part of the public)", item, re.I)
        for item in support_caveats
    ):
        raise GateError("activated release manifest retains preactivation support caveats")
    toc = _git_blob(root, source["commit"], "stata/stata.toc").decode(
        "utf-8", errors="strict"
    )
    if "feppml" not in toc.lower() or re.search(r"r\(498\)|fail[- ]closed", toc, re.I):
        raise GateError("activated package index does not publish qualified feppml")

    return {
        "status": "pass", "verdict": "minimal_control_plane_activation",
        "parent_source": dict(prior["source"]),
        "preactivation_receipt": {
            "file_sha256": expected_record["receipt_sha256"],
            "payload_sha256": expected_record["receipt_payload_sha256"],
            "size_bytes": expected_record["receipt_size_bytes"],
            "index_sha256": record["index_sha256"],
        },
        "allowed_paths_contract_sha256": sha256_bytes(
            canonical_json_bytes(sorted(ACTIVATION_ALLOWED_PATHS))
        ),
        "changed_paths": changes,
    }


def _load_artifact_index(
    artifact_index: Path, source: Mapping[str, str], *, source_root: Path | None = None
) -> list[dict[str, Any]]:
    raw_artifacts = _read_json(artifact_index, "artifact index")
    _closed(raw_artifacts, {"schema_version", "source", "artifacts"}, "artifact index")
    if raw_artifacts["schema_version"] != 1 or _source(raw_artifacts["source"]) != source:
        raise GateError("artifact index is bound to another source")
    values = raw_artifacts["artifacts"]
    if not isinstance(values, list):
        raise GateError("artifact index artifacts must be a list")
    parsed = [
        _artifact_file(raw, f"artifact index[{index}]", source, artifact_index.parent)
        for index, raw in enumerate(values)
    ]
    artifacts = [identity for identity, _record in parsed]
    artifacts.sort(key=lambda item: (item["role"], item["platform"]))
    keys = {(item["role"], item["platform"]) for item in artifacts}
    if len(keys) != len(artifacts) or keys != REQUIRED_ARTIFACT_KEYS:
        raise GateError("artifact index does not contain the exact required release inventory")
    if source_root is not None:
        source_record = next(
            record
            for identity, record in parsed
            if identity["role"] == "source_archive" and identity["platform"] == "source"
        )
        _verify_exact_source_archive(source_root.resolve(), source, source_record)
    return artifacts


def _execution_environment(names: Sequence[str]) -> dict[str, Any]:
    if not names:
        raise GateError("at least one --environment-key is required")
    if list(names) != sorted(set(names)):
        raise GateError("--environment-key values must be unique and canonically ordered")
    variables = []
    for name in names:
        if not re.fullmatch(r"[A-Z][A-Z0-9_]*", name):
            raise GateError(f"environment key is not a closed uppercase name: {name}")
        if name not in os.environ:
            raise GateError(f"selected environment key is absent: {name}")
        variables.append(
            {"name": name, "value_sha256": sha256_bytes(os.environ[name].encode("utf-8"))}
        )
    return {
        "os": platform.system(), "arch": platform.machine(),
        "python": platform.python_version(), "variables": variables,
    }


def _stream_record(path: Path, sentinel: bytes | None = None) -> tuple[dict[str, Any], int]:
    digest = hashlib.sha256()
    size = 0
    occurrences = 0
    with path.open("rb") as source:
        for line in source:
            digest.update(line)
            size += len(line)
            if sentinel is not None and line.rstrip(b"\r\n") == sentinel:
                occurrences += 1
    return {"sha256": digest.hexdigest(), "size_bytes": size}, occurrences


def _parse_declared_outputs(values: Sequence[Sequence[str]]) -> list[tuple[str, Path, str]]:
    parsed: list[tuple[str, Path, str]] = []
    for value in values:
        if len(value) != 3:
            raise GateError("--output requires ROLE PATH SHA256")
        role, path, digest = value
        if not role or not re.fullmatch(r"[a-z][a-z0-9_]*", role):
            raise GateError(f"output role is invalid: {role!r}")
        _sha(digest, f"expected output digest for {role}", 64)
        parsed.append((role, Path(path), digest))
    if not parsed or [item[0] for item in parsed] != sorted({item[0] for item in parsed}):
        raise GateError("declared outputs must be nonempty, unique, and canonically ordered")
    return parsed


def _output_records(values: Sequence[tuple[str, Path, str]]) -> list[dict[str, Any]]:
    records = []
    for role, raw_path, expected_sha in values:
        path = raw_path.resolve()
        if raw_path.is_symlink() or path.is_symlink() or not path.is_file():
            raise GateError(f"declared output is absent or a symlink: {role}")
        digest = sha256_file(path)
        size = path.stat().st_size
        if digest != expected_sha or size < 0:
            raise GateError(f"declared output hash/size differs: {role}")
        records.append(
            {
                "role": role, "path": str(path), "filename": path.name,
                "sha256": digest, "size_bytes": size,
            }
        )
    return records


def run_closed_argv_result(
    *, check_id: str, source_root: Path, source_sha: str, public_state: str,
    artifact_index: Path, producer_id: str, producer_version: str,
    producer_entrypoint: Path, environment_keys: Sequence[str], toolchain: str,
    profile_name: str, declared_outputs: Sequence[Sequence[str]], timeout_seconds: int,
    argv: Sequence[str], output: Path,
) -> None:
    if check_id not in GENERIC_COMMAND_CHECKS:
        raise GateError("closed-argv producer is not eligible for this check id")
    if not argv or any(not isinstance(item, str) or not item for item in argv):
        raise GateError("closed command argv must be a nonempty string array")
    if timeout_seconds <= 0:
        raise GateError("command timeout must be positive")
    source_root = source_root.resolve()
    commit = _git(source_root, ["rev-parse", f"{source_sha}^{{commit}}"])
    source = {"commit": commit, "tree": _git(source_root, ["rev-parse", f"{commit}^{{tree}}"])}
    audit_source(source_root, source, public_state)
    artifacts = _load_artifact_index(
        artifact_index.resolve(), source, source_root=source_root
    )
    producer_entrypoint = producer_entrypoint.resolve()
    required_entrypoint = (source_root / "scripts/feppml_phase12_local.py").resolve()
    committed_entrypoint = _git_blob(
        source_root, source["commit"], "scripts/feppml_phase12_local.py"
    )
    if (
        producer_entrypoint != required_entrypoint or not producer_entrypoint.is_file()
        or sha256_file(producer_entrypoint) != sha256_bytes(committed_entrypoint)
        or producer_id != "feppml-phase12-local" or producer_version != "1"
    ):
        raise GateError("generic result must use the exact committed Phase-12 local producer")
    if profile_name != f"phase12-local:{check_id}":
        raise GateError("generic result profile name is not the registered check profile")
    environment = _execution_environment(environment_keys)
    outputs = _parse_declared_outputs(declared_outputs)
    if len(outputs) != 1 or outputs[0][0] != "summary":
        raise GateError("closed local producer requires exactly one summary output")
    summary_path = outputs[0][1].resolve()
    raw_root = summary_path.with_name(f"{summary_path.name}.raw")
    stdout_path = summary_path.with_name(f"{summary_path.name}.producer.stdout.log")
    stderr_path = summary_path.with_name(f"{summary_path.name}.producer.stderr.log")
    if raw_root.exists() or stdout_path.exists() or stderr_path.exists():
        raise GateError("closed local producer raw evidence output already exists")
    expected_argv = [
        str(Path(sys.executable).resolve()), str(required_entrypoint), check_id,
        "--output", str(summary_path),
        "--evidence-dir", str(raw_root),
    ]
    if list(argv) != expected_argv:
        raise GateError("closed argv does not match the exact check-id producer template")
    local_module = _source_module(
        source_root, source, "scripts/feppml_phase12_local.py",
        f"_phase12_local_producer_{source['commit']}",
    )
    expected_toolchain = local_module.toolchain(check_id)
    if toolchain != expected_toolchain:
        raise GateError("closed local producer toolchain is not exact")
    sentinel = result_sentinel(check_id).encode("ascii")
    producer_environment = os.environ.copy()
    producer_environment["PYTHONDONTWRITEBYTECODE"] = "1"
    try:
        with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
            completed = subprocess.run(
                list(argv), cwd=source_root, stdin=subprocess.DEVNULL,
                stdout=stdout, stderr=stderr, check=False, timeout=timeout_seconds,
                shell=False, env=producer_environment,
            )
        stdout_record, stdout_sentinels = _stream_record(stdout_path, sentinel)
        stderr_record, stderr_sentinels = _stream_record(stderr_path, sentinel)
    except subprocess.TimeoutExpired as error:
        raise GateError(f"closed command timed out after {timeout_seconds} seconds") from error
    if completed.returncode != 0:
        raise GateError(f"closed command returned {completed.returncode}")
    if stdout_sentinels + stderr_sentinels != 1:
        raise GateError("closed command must emit its exact terminal sentinel once")
    output_records = _output_records(outputs)
    summary = _read_json(outputs[0][1].resolve(), "closed local producer summary")
    _closed(
        summary,
        {
            "schema_version", "result_type", "check_id", "source", "commands",
            "status", "complete_sentinel",
        },
        "closed local producer summary",
    )
    commands = summary["commands"]
    if (
        summary["schema_version"] != 1
        or summary["result_type"] != "feppml-phase12-local-command-summary-v1"
        or summary["check_id"] != check_id
        or _source(summary["source"], "closed local summary source") != source
        or not isinstance(commands, list) or not commands
        or not all(
            isinstance(command, list) and command
            and all(isinstance(item, str) and item for item in command)
            for command in commands
        )
        or summary["status"] != "pass"
        or summary["complete_sentinel"] != sentinel.decode("ascii")
    ):
        raise GateError("closed local producer summary is invalid or source-mixed")
    expected_commands = local_module.commands(check_id)
    if commands != expected_commands:
        raise GateError("closed local producer command registry differs from committed code")
    expected_raw_names = {"processes.json"}
    raw_values: list[tuple[str, Path]] = [
        ("process_manifest", raw_root / "processes.json"),
        ("producer_stderr", stderr_path),
        ("producer_stdout", stdout_path),
    ]
    for index in range(len(expected_commands)):
        for stream in ("stderr", "stdout"):
            name = f"command-{index:03d}.{stream}.log"
            expected_raw_names.add(name)
            raw_values.append((f"command_{index:03d}_{stream}", raw_root / name))
    if (
        not raw_root.is_dir() or raw_root.is_symlink()
        or {item.name for item in raw_root.iterdir()} != expected_raw_names
    ):
        raise GateError("closed local producer raw evidence layout is not exact")
    output_records.extend(_bound_output(path, role) for role, path in raw_values)
    output_records.sort(key=lambda item: item["role"])
    audit_source(source_root, source, public_state)
    if _load_artifact_index(
        artifact_index.resolve(), source, source_root=source_root
    ) != artifacts:
        raise GateError("artifact inventory changed during the closed command")
    payload = {
        "schema_version": 1, "receipt_type": RESULT_RECEIPT_TYPE, "check_id": check_id,
        "source": source, "public_state": public_state, "artifacts": artifacts,
        "claims": list(COMMON_CHECK_CLAIMS[check_id]),
        "producer": {
            "id": producer_id, "version": producer_version,
            "entrypoint_sha256": sha256_file(producer_entrypoint),
        },
        "execution": {
            "mode": "closed_argv", "argv": list(argv),
            "argv_sha256": sha256_bytes(canonical_json_bytes(list(argv))),
            "exit_code": completed.returncode, "stdout": stdout_record,
            "stderr": stderr_record, "terminal_sentinel": sentinel.decode("ascii"),
            "environment": environment, "toolchain": expected_toolchain,
            "profile": profile_name,
        },
        "outputs": output_records, "status": "pass", "completed": True,
        "complete_sentinel": RESULT_SENTINEL,
    }
    validate_result_payload(payload)
    _validate_generic_result(payload, source_root, source)
    _write_immutable(output, seal_payload(payload))


STATA_PROFILE_BY_CHECK = {
    "stata_syntax": "syntax", "stata_quick": "quick",
    "stata_numerical_small": "numerical-small", "stata_differential": "differential",
    "stata_fereg_regression": "fereg-regression", "stata_ppml_hidden": "ppml-hidden",
    "stata_full": "full", "stata_ppml_fail_closed": "ppml-fail-closed",
    "stata_ppml_scientific": "ppml-scientific",
    "stata_break_fereg": "fereg-regression", "stata_break_feppml": "ppml-hidden",
}

STATA_CI_FIELDS = {
    "schema_version", "tested_sha", "repository", "ref", "run_id",
    "run_attempt", "profile", "driver", "status", "stata_status",
    "rust_status", "rust_rc", "failure_kind", "failure_detail", "process_rc",
    "stata_rc", "stata_version", "stata_edition", "stata_bundle_version",
    "stata_executable", "platform", "stata_processors", "runner_name",
    "started_at", "completed_at", "duration_seconds", "tests_passed",
    "tests_failed", "plugin_sha256", "required_outputs",
}
STATA_PROFILE_FIELDS = {
    "schema_version", "profile", "completed", "status", "failure_kind",
    "failure_detail", "stata_rc", "stata_version", "stata_edition", "stata_os",
    "stata_machine_type", "stata_processors", "tests_passed", "tests_failed",
    "plugin_sha256", "build_rc", "results", "break_receipts",
}
STATA_FIXTURE_SUMMARY_FIELDS = {
    "schema_version", "completed", "status", "failure_kind", "failure_detail",
    "stata_rc", "stata_version", "stata_edition", "stata_os",
    "stata_machine_type", "stata_processors", "tests_passed", "tests_failed",
    "results",
}
STATA_RESULT_FIELDS = {
    "fixture", "status", "failure_kind", "failure_detail", "process_rc", "stata_rc"
}
STATA_STATUS_FIELDS = {
    "schema_version", "completed", "stata_rc", "stata_version", "stata_edition",
    "stata_os", "stata_machine_type", "stata_processors",
}
STATA_SOURCE_RECORD_FIELDS = {
    "schema_version", "record_type", "source", "build_date_utc", "archive",
}
STATA_SOURCE_ARCHIVE_FIELDS = {"filename", "sha256", "size_bytes"}
STATA_PROCESS_ENVIRONMENT = (
    "FEREG_BUILD_DATE", "FEREG_GIT_COMMIT", "PATH", "RUSTUP_BIN",
    "RUST_TOOLCHAIN", "STATA_BIN",
)


def _bound_output(path: Path, role: str) -> dict[str, Any]:
    raw = Path(path)
    resolved = raw.resolve()
    if raw.is_symlink() or resolved.is_symlink() or not resolved.is_file():
        raise GateError(f"authoritative output is absent or a symlink: {role}")
    return {
        "role": role, "path": str(resolved), "filename": resolved.name,
        "sha256": sha256_file(resolved), "size_bytes": resolved.stat().st_size,
    }


def _read_fixture_status(path: Path, where: str) -> dict[str, str]:
    if not path.is_file() or path.is_symlink():
        raise GateError(f"{where} is absent or a symlink")
    values: dict[str, str] = {}
    try:
        for raw_line in path.read_text(encoding="utf-8").splitlines():
            key, separator, value = raw_line.partition("=")
            if not separator or not key or key in values:
                raise GateError(f"{where} contains a malformed or duplicate field")
            values[key] = value
    except (OSError, UnicodeDecodeError) as error:
        raise GateError(f"cannot read {where}: {error}") from error
    if set(values) != STATA_STATUS_FIELDS:
        raise GateError(f"{where} fields are not closed")
    if (
        values["schema_version"] != "1" or values["completed"] != "1"
        or values["stata_rc"] != "0"
        or any(not values[field] for field in STATA_STATUS_FIELDS - {
            "schema_version", "completed", "stata_rc"
        })
    ):
        raise GateError(f"{where} is not a successful completed Stata status")
    return values


def _validate_stata_evidence_files(
    *, check_id: str, source_root: Path, source: Mapping[str, str],
    artifacts: Sequence[Mapping[str, Any]], ci_receipt: Path,
    profile_summary: Path,
) -> tuple[
    Mapping[str, Any], Mapping[str, Any], dict[str, Path], list[str]
]:
    """Recompute a licensed profile verdict from every retained raw file."""

    expected_profile = STATA_PROFILE_BY_CHECK[check_id]
    evidence_root = ci_receipt.resolve().parent
    if (
        ci_receipt.resolve() != evidence_root / "receipt.json"
        or profile_summary.resolve() != evidence_root / "profile-summary.json"
    ):
        raise GateError("Stata evidence files are outside the exact CI artifact layout")
    ci = _read_json(ci_receipt, "Stata CI receipt")
    summary = _read_json(profile_summary, "Stata profile summary")
    _closed(ci, STATA_CI_FIELDS, "Stata CI receipt")
    source_record_path = evidence_root / "source.json"
    source_archive_path = evidence_root / "source.tar"
    source_record = _read_json(source_record_path, "Stata exact-source record")
    _closed(source_record, STATA_SOURCE_RECORD_FIELDS, "Stata exact-source record")
    archive_record = _mapping(
        source_record["archive"], "Stata exact-source archive record"
    )
    _closed(
        archive_record, STATA_SOURCE_ARCHIVE_FIELDS,
        "Stata exact-source archive record",
    )
    expected_archive = _git_archive(source_root, source["commit"])
    if (
        source_record["schema_version"] != 1
        or source_record["record_type"] != "fereg-exact-source-archive-v1"
        or _source(source_record["source"], "Stata exact-source identity") != source
        or source_record["build_date_utc"] != _commit_build_date(
            source_root, source["commit"]
        )
        or source_archive_path.is_symlink() or not source_archive_path.is_file()
        or archive_record != {
            "filename": "source.tar",
            "sha256": sha256_bytes(expected_archive),
            "size_bytes": len(expected_archive),
        }
        or source_archive_path.read_bytes() != expected_archive
    ):
        raise GateError(
            "licensed-Stata evidence was not executed from the exact committed archive"
        )
    try:
        profiles = json.loads(
            _git_blob(source_root, source["commit"], "ci/stata_profiles.json")
        )["profiles"]
        profile_spec = _mapping(profiles[expected_profile], "committed Stata profile")
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise GateError("committed licensed-Stata profile is absent or malformed") from error
    expected_summary_fields = set(STATA_PROFILE_FIELDS)
    if profile_spec.get("build_plugin") is True:
        expected_summary_fields.add("plugin_filename")
    _closed(summary, expected_summary_fields, "Stata profile summary")
    if (
        ci["schema_version"] != 1 or ci["tested_sha"] != source["commit"]
        or ci["profile"] != expected_profile or ci["driver"] != "fereg-fixtures"
        or ci["status"] != "success" or ci["stata_status"] != "success"
        or ci["rust_status"] != "not_run" or ci["rust_rc"] is not None
        or ci["failure_kind"] is not None or ci["failure_detail"] is not None
        or ci["process_rc"] != 0 or ci["stata_rc"] != 0
        or ci["tests_failed"] != 0
        or not isinstance(ci["tests_passed"], int) or ci["tests_passed"] < 1
        or summary["schema_version"] != 1 or summary["profile"] != expected_profile
        or summary["completed"] is not True or summary["status"] != "success"
        or summary["failure_kind"] is not None or summary["failure_detail"] is not None
        or summary["stata_rc"] != 0 or summary["tests_failed"] != 0
        or summary["tests_passed"] != ci["tests_passed"]
    ):
        raise GateError("Stata CI/profile summary is not an exact successful run")

    process_path = evidence_root / "process.json"
    process_stdout = evidence_root / "process.stdout.log"
    process_stderr = evidence_root / "process.stderr.log"
    process = _read_json(process_path, "Stata supervisor process")
    required_process = {
        "schema_version", "started_at", "command_executable", "launch_error",
        "timed_out", "process_rc", "pid", "completed_at", "duration_seconds",
        "argv", "cwd", "environment",
    }
    _closed(process, required_process, "Stata supervisor process")
    if (
        process["schema_version"] != 1 or process["launch_error"] is not None
        or process["timed_out"] is not False or process["process_rc"] != 0
        or not isinstance(process["pid"], int) or isinstance(process["pid"], bool)
        or process["pid"] < 1 or not isinstance(process["command_executable"], str)
        or not process["command_executable"] or not process_stdout.is_file()
        or process_stdout.is_symlink() or not process_stderr.is_file()
        or process_stderr.is_symlink()
    ):
        raise GateError("Stata supervisor did not retain an exact successful process record")
    process_cwd = Path(str(process["cwd"]))
    staged_root = process_cwd / "repo"
    expected_command = [
        str(Path(sys.executable).resolve()),
        str(staged_root / "ci/run_fixture_profile.py"),
        "--profile-config", str(staged_root / "ci/stata_profiles.json"),
        "--profile", expected_profile,
        "--repo-root", str(staged_root),
        "--run-root", str(process_cwd),
        "--summary-json", str(process_cwd / "profile-summary.json"),
        "--stata", str(ci["stata_executable"]),
        "--tested-sha", source["commit"],
    ]
    environment = process["environment"]
    if (
        not process_cwd.is_absolute()
        or not process_cwd.name.startswith("fereg-stata-ci-")
        or process["command_executable"] != expected_command[0]
        or process["argv"] != expected_command
        or not isinstance(environment, list)
        or [item.get("name") for item in environment if isinstance(item, Mapping)]
        != list(STATA_PROCESS_ENVIRONMENT)
    ):
        raise GateError("Stata supervisor argv/cwd/environment is not the exact profile run")
    expected_env = {
        "FEREG_BUILD_DATE": str(source_record["build_date_utc"]),
        "FEREG_GIT_COMMIT": source["commit"][:12],
    }
    for index, raw_environment in enumerate(environment):
        item = _mapping(raw_environment, f"Stata process environment[{index}]")
        _closed(
            item, {"name", "present", "value_sha256"},
            f"Stata process environment[{index}]",
        )
        name = str(item["name"])
        if not isinstance(item["present"], bool):
            raise GateError("Stata process environment presence flag is malformed")
        if item["present"]:
            _sha(
                item["value_sha256"],
                f"Stata process environment[{index}].value_sha256", 64,
            )
        elif item["value_sha256"] is not None:
            raise GateError("absent Stata process environment has a value digest")
        if name in expected_env and (
            item["present"] is not True
            or item["value_sha256"]
            != sha256_bytes(expected_env[name].encode("utf-8"))
        ):
            raise GateError(f"Stata process environment {name} is source-mixed")

    fixture_summary_path = evidence_root / "fixture-summary.json"
    fixture_summary = _read_json(fixture_summary_path, "Stata fixture summary")
    _closed(fixture_summary, STATA_FIXTURE_SUMMARY_FIELDS, "Stata fixture summary")
    expected_regular = list(profile_spec.get("fixtures", []))
    if not expected_regular or not all(isinstance(item, str) and item for item in expected_regular):
        raise GateError("committed Stata fixture inventory is empty or malformed")
    regular_results = fixture_summary.get("results")
    if (
        fixture_summary["schema_version"] != 1
        or fixture_summary["completed"] is not True
        or fixture_summary["status"] != "success"
        or fixture_summary["failure_kind"] is not None
        or fixture_summary["failure_detail"] is not None
        or fixture_summary["stata_rc"] != 0
        or fixture_summary["tests_failed"] != 0
        or fixture_summary["tests_passed"] != len(expected_regular)
        or not isinstance(regular_results, list)
        or [item.get("fixture") for item in regular_results if isinstance(item, Mapping)]
        != expected_regular
    ):
        raise GateError("Stata fixture summary differs from the committed profile")
    fixture_runner = _source_module(
        source_root, source, "scripts/run_stata_tests.py",
        f"_phase12_stata_runner_{source['commit']}",
    )
    outputs: dict[str, Path] = {
        "ci_receipt": ci_receipt.resolve(),
        "fixture_summary": fixture_summary_path.resolve(),
        "process_json": process_path.resolve(),
        "process_stderr": process_stderr.resolve(),
        "process_stdout": process_stdout.resolve(),
        "profile_summary": profile_summary.resolve(),
        "source_archive": source_archive_path.resolve(),
        "source_record": source_record_path.resolve(),
    }
    fixture_logs = evidence_root / "fixture-logs"
    for index, (fixture, raw_result) in enumerate(zip(expected_regular, regular_results)):
        result = _mapping(raw_result, f"Stata fixture result[{index}]")
        _closed(result, STATA_RESULT_FIELDS, f"Stata fixture result[{index}]")
        if result != {
            "fixture": fixture, "status": "success", "failure_kind": None,
            "failure_detail": None, "process_rc": 0, "stata_rc": 0,
        }:
            raise GateError(f"Stata fixture did not pass exactly: {fixture}")
        fixture_blob = _git_blob(source_root, source["commit"], fixture)
        fixture_path = source_root / fixture
        if not fixture_path.is_file() or fixture_path.read_bytes() != fixture_blob:
            raise GateError(f"Stata fixture bytes differ from committed source: {fixture}")
        stable = fixture.replace("/", "__")
        log_path = fixture_logs / f"{stable}.log"
        status_path = fixture_logs / f"{stable}.status"
        try:
            log_text = log_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            raise GateError(f"cannot read retained Stata fixture log: {fixture}: {error}") from error
        if fixture_runner.validate_fixture_log(fixture_path, log_text) is not None:
            raise GateError(f"retained Stata fixture log lacks its exact sentinel: {fixture}")
        metadata = _read_fixture_status(status_path, f"Stata fixture status {fixture}")
        for key in (
            "stata_version", "stata_edition", "stata_os", "stata_machine_type",
            "stata_processors",
        ):
            if str(fixture_summary[key]) != metadata[key]:
                raise GateError(f"Stata fixture status metadata is mixed: {fixture}")
        outputs[f"fixture_{index:03d}_log"] = log_path.resolve()
        outputs[f"fixture_{index:03d}_status"] = status_path.resolve()

    expected_break_kinds: list[str] = []
    if profile_spec.get("run_break_fixture") is True:
        expected_break_kinds.append("ols")
    if profile_spec.get("run_ppml_break_fixture") is True:
        expected_break_kinds.append("ppml")
    break_receipts = summary.get("break_receipts")
    if (
        not isinstance(break_receipts, list)
        or [item.get("fixture_kind") for item in break_receipts if isinstance(item, Mapping)]
        != expected_break_kinds
    ):
        raise GateError("Stata Break receipt inventory differs from the committed profile")
    expected_results = list(regular_results)
    for offset, (fixture_kind, raw_receipt) in enumerate(
        zip(expected_break_kinds, break_receipts), start=len(expected_regular)
    ):
        fixture_name = {
            "ols": "estimate_ols_eclass_break", "ppml": "estimate_ppml_eclass_break"
        }[fixture_kind]
        fixture = f"tests/licensed_stata/{fixture_name}.do"
        receipt = _mapping(raw_receipt, f"Stata Break receipt {fixture_kind}")
        _closed(
            receipt,
            {
                "schema_version", "receipt_type", "fixture_kind", "source_sha",
                "plugin_sha256", "fixture_sha256", "log_sha256", "process_rc",
                "terminal_sentinel", "rollback_validated", "receipt_last_validated",
                "status",
            },
            f"Stata Break receipt {fixture_kind}",
        )
        sentinel = f"END_FIXTURE|{fixture_name}|rc=0"
        log_path = fixture_logs / f"{fixture_name}.log"
        log_raw = log_path.read_bytes()
        if (
            receipt["schema_version"] != 1
            or receipt["receipt_type"] != "fereg-licensed-break-v1"
            or receipt["fixture_kind"] != fixture_kind
            or receipt["source_sha"] != source["commit"]
            or receipt["fixture_sha256"] != sha256_bytes(
                _git_blob(source_root, source["commit"], fixture)
            )
            or receipt["log_sha256"] != sha256_bytes(log_raw)
            or log_raw.splitlines().count(f"PASS: {sentinel}".encode("utf-8")) != 1
            or receipt["process_rc"] != 0 or receipt["terminal_sentinel"] != sentinel
            or receipt["rollback_validated"] is not True
            or receipt["receipt_last_validated"] is not True
            or receipt["status"] != "pass"
        ):
            raise GateError(f"Stata Break receipt/log is incomplete: {fixture_kind}")
        expected_results.append(
            {
                "fixture": fixture, "status": "success", "failure_kind": None,
                "failure_detail": None, "process_rc": 0, "stata_rc": 0,
            }
        )
        outputs[f"fixture_{offset:03d}_log"] = log_path.resolve()
    if summary["results"] != expected_results or summary["tests_passed"] != len(expected_results):
        raise GateError("Stata profile result rows cannot be recomputed from raw evidence")

    build_plugin = profile_spec.get("build_plugin") is True
    if build_plugin:
        plugin_name = summary.get("plugin_filename")
        plugin = next(
            (
                item for item in artifacts
                if item["role"] == "plugin" and item["filename"] == plugin_name
            ),
            None,
        )
        plugin_path = evidence_root / "plugin-evidence" / str(plugin_name)
        checksum_path = evidence_root / "plugin-evidence" / f"{plugin_name}.sha256"
        if (
            plugin is None or summary["build_rc"] != 0
            or summary["plugin_sha256"] != plugin["sha256"]
            or ci["plugin_sha256"] != plugin["sha256"]
            or not plugin_path.is_file() or plugin_path.is_symlink()
            or sha256_file(plugin_path) != plugin["sha256"]
            or plugin_path.stat().st_size != plugin["size_bytes"]
            or source["commit"][:12].encode("ascii") not in plugin_path.read_bytes()
            or str(source_record["build_date_utc"]).encode("ascii")
            not in plugin_path.read_bytes()
            or checksum_path.read_text(encoding="ascii").strip().split()
            != [plugin["sha256"], str(plugin_name)]
        ):
            raise GateError("Stata plugin evidence differs from the aggregate artifact")
        outputs["plugin_binary"] = plugin_path.resolve()
        outputs["plugin_checksum"] = checksum_path.resolve()
    elif (
        summary["build_rc"] is not None or summary["plugin_sha256"] is not None
        or ci["plugin_sha256"] is not None or "plugin_filename" in summary
    ):
        raise GateError("non-build Stata profile fabricated plugin evidence")

    required_paths: list[str] = []
    for value in profile_spec.get("required_outputs", []):
        if not isinstance(value, str):
            raise GateError("committed Stata required-output path is malformed")
        required_paths.append(
            value.format(platform_plugin=summary.get("plugin_filename", ""))
        )
    if ci["required_outputs"] != [
        {"path": path, "present": True} for path in required_paths
    ]:
        raise GateError("Stata CI required-output inventory differs from committed profile")
    for relative in required_paths:
        expected = (evidence_root / relative).resolve()
        if expected not in outputs.values() or not expected.is_file():
            raise GateError(f"Stata CI required output is not byte-bound: {relative}")
    return ci, summary, outputs, expected_command


def _validate_stata_result(
    payload: Mapping[str, Any], source_root: Path, source: Mapping[str, str],
) -> None:
    producer = payload["producer"]
    committed = _git_blob(source_root, source["commit"], "ci/run_stata_ci.py")
    if producer != {
        "id": "licensed-stata-profile", "version": "1",
        "entrypoint_sha256": sha256_bytes(committed),
    }:
        raise GateError("Stata result does not name the exact committed CI producer")
    output_paths = _verify_result_output_bytes(payload)
    if "ci_receipt" not in output_paths or "profile_summary" not in output_paths:
        raise GateError("Stata result omits its primary raw evidence")
    ci, summary, expected_paths, expected_command = _validate_stata_evidence_files(
        check_id=str(payload["check_id"]), source_root=source_root, source=source,
        artifacts=payload["artifacts"], ci_receipt=output_paths["ci_receipt"],
        profile_summary=output_paths["profile_summary"],
    )
    if output_paths != expected_paths:
        raise GateError("Stata result output registry is not exact")
    process_raw = output_paths["process_stdout"].read_bytes()
    process_error_raw = output_paths["process_stderr"].read_bytes()
    execution = payload["execution"]
    values = {
        "RUNNER_NAME": str(ci["runner_name"]),
        "STATA_BUNDLE_VERSION": str(ci["stata_bundle_version"]),
        "STATA_PLATFORM": str(ci["platform"]),
        "STATA_VERSION": str(ci["stata_version"]),
    }
    if (
        execution["profile"] != STATA_PROFILE_BY_CHECK[payload["check_id"]]
        or execution["argv"] != expected_command
        or execution["stdout"] != {
            "sha256": sha256_bytes(process_raw), "size_bytes": len(process_raw)
        }
        or execution["stderr"] != {
            "sha256": sha256_bytes(process_error_raw),
            "size_bytes": len(process_error_raw),
        }
        or execution["environment"] != {
            "os": str(ci["platform"]),
            "arch": str(summary["stata_machine_type"]),
            "python": platform.python_version(),
            "variables": [
                {
                    "name": name,
                    "value_sha256": sha256_bytes(value.encode("utf-8")),
                }
                for name, value in sorted(values.items())
            ],
        }
        or execution["toolchain"]
        != f"Stata {ci['stata_version']} {ci['stata_bundle_version']}"
    ):
        raise GateError("Stata execution registry differs from retained raw evidence")


def _legacy_stata_profile_adapter_do_not_call(
    *, check_id: str, source_root: Path, source_sha: str, public_state: str,
    artifact_index: Path, ci_receipt: Path, profile_summary: Path,
    producer_entrypoint: Path, output: Path,
) -> None:
    raise GateError("the summary-only Stata adapter was removed; retain raw logs/status")
    if check_id not in STATA_PROFILE_BY_CHECK:
        raise GateError("licensed-Stata adapter is not eligible for this check id")
    source_root = source_root.resolve()
    commit = _git(source_root, ["rev-parse", f"{source_sha}^{{commit}}"])
    source = {"commit": commit, "tree": _git(source_root, ["rev-parse", f"{commit}^{{tree}}"])}
    audit_source(source_root, source, public_state)
    artifacts = _load_artifact_index(
        artifact_index.resolve(), source, source_root=source_root
    )
    ci = _read_json(ci_receipt, "Stata CI receipt")
    summary = _read_json(profile_summary, "Stata profile summary")
    expected_profile = STATA_PROFILE_BY_CHECK[check_id]
    try:
        committed_profiles = json.loads(
            _git_blob(source_root, source["commit"], "ci/stata_profiles.json")
        )["profiles"]
        committed_profile = committed_profiles[expected_profile]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise GateError("committed licensed-Stata profile is absent or malformed") from error
    if (
        ci.get("schema_version") != 1 or ci.get("tested_sha") != source["commit"]
        or ci.get("profile") != expected_profile or ci.get("status") != "success"
        or ci.get("stata_status") != "success" or ci.get("failure_kind") is not None
        or ci.get("process_rc") not in (None, 0) or ci.get("stata_rc") not in (None, 0)
        or ci.get("tests_failed") != 0 or not isinstance(ci.get("tests_passed"), int)
        or ci.get("tests_passed", 0) <= 0
    ):
        raise GateError("Stata CI receipt is not a successful exact-source profile")
    if (
        summary.get("schema_version") != 1 or summary.get("profile") != expected_profile
        or summary.get("completed") is not True or summary.get("status") != "success"
        or summary.get("failure_kind") is not None or summary.get("tests_failed") != 0
        or not isinstance(summary.get("tests_passed"), int) or summary.get("tests_passed", 0) <= 0
    ):
        raise GateError("Stata profile summary is not closed successful evidence")
    results = summary.get("results")
    if not isinstance(results, list) or not results:
        raise GateError("Stata profile summary has no fixture inventory")
    for index, raw in enumerate(results):
        result = _mapping(raw, f"Stata result[{index}]")
        if (
            result.get("status") != "success" or result.get("failure_kind") is not None
            or result.get("process_rc") not in (None, 0) or result.get("stata_rc") not in (None, 0)
            or not isinstance(result.get("fixture"), str) or not result.get("fixture")
        ):
            raise GateError("Stata profile contains a failed or incomplete fixture")
    expected_fixtures = list(committed_profile.get("fixtures", []))
    expected_break_kinds: list[str] = []
    if committed_profile.get("run_break_fixture") is True:
        expected_fixtures.append("tests/licensed_stata/estimate_ols_eclass_break.do")
        expected_break_kinds.append("ols")
    if committed_profile.get("run_ppml_break_fixture") is True:
        expected_fixtures.append("tests/licensed_stata/estimate_ppml_eclass_break.do")
        expected_break_kinds.append("ppml")
    observed_fixtures = [item["fixture"] for item in results]
    if observed_fixtures != expected_fixtures:
        raise GateError("Stata result inventory differs from the exact committed profile")
    break_receipts = summary.get("break_receipts")
    if not isinstance(break_receipts, list) or [
        item.get("fixture_kind") for item in break_receipts if isinstance(item, Mapping)
    ] != expected_break_kinds:
        raise GateError("Stata Break receipt inventory differs from the committed profile")
    plugin_sha = summary.get("plugin_sha256")
    plugin_name = summary.get("plugin_filename")
    candidate = next(
        (
            item for item in artifacts
            if item["role"] == "plugin" and item["filename"] == plugin_name
        ),
        None,
    )
    if (
        plugin_sha is None or candidate is None or plugin_sha != candidate["sha256"]
        or ci.get("plugin_sha256") != plugin_sha
    ):
        raise GateError("Stata profile plugin is absent, source-mixed, or artifact-mixed")
    for index, raw in enumerate(break_receipts):
        item = _mapping(raw, f"Stata Break receipt[{index}]")
        _closed(
            item,
            {
                "schema_version", "receipt_type", "fixture_kind", "source_sha",
                "plugin_sha256", "fixture_sha256", "log_sha256", "process_rc",
                "terminal_sentinel", "rollback_validated", "receipt_last_validated",
                "status",
            },
            f"Stata Break receipt[{index}]",
        )
        fixture_kind = item["fixture_kind"]
        fixture_path = {
            "ols": "tests/licensed_stata/estimate_ols_eclass_break.do",
            "ppml": "tests/licensed_stata/estimate_ppml_eclass_break.do",
        }[fixture_kind]
        expected_sentinel = {
            "ols": "END_FIXTURE|estimate_ols_eclass_break|rc=0",
            "ppml": "END_FIXTURE|estimate_ppml_eclass_break|rc=0",
        }[fixture_kind]
        if (
            item["schema_version"] != 1 or item["receipt_type"] != "fereg-licensed-break-v1"
            or item["source_sha"] != source["commit"] or item["plugin_sha256"] != plugin_sha
            or item["fixture_sha256"] != sha256_bytes(
                _git_blob(source_root, source["commit"], fixture_path)
            )
            or HEX_SHA256.fullmatch(str(item["log_sha256"])) is None
            or item["process_rc"] != 0 or item["terminal_sentinel"] != expected_sentinel
            or item["rollback_validated"] is not True
            or item["receipt_last_validated"] is not True or item["status"] != "pass"
        ):
            raise GateError("licensed Break receipt is incomplete or source/artifact-mixed")
    required_break = {
        "stata_break_fereg": "ols", "stata_break_feppml": "ppml"
    }.get(check_id)
    if required_break is not None and required_break not in expected_break_kinds:
        raise GateError("requested Break check is not enabled by its committed profile")
    required_outputs = ci.get("required_outputs")
    if not isinstance(required_outputs, list) or any(
        not isinstance(item, Mapping) or item.get("present") is not True
        for item in required_outputs
    ):
        raise GateError("Stata CI required-output inventory is incomplete")
    producer_entrypoint = producer_entrypoint.resolve()
    if not producer_entrypoint.is_file():
        raise GateError("Stata producer entrypoint is absent")
    ci_raw = ci_receipt.read_bytes()
    summary_raw = profile_summary.read_bytes()
    values = {
        "RUNNER_NAME": str(ci.get("runner_name", "")),
        "STATA_BUNDLE_VERSION": str(ci.get("stata_bundle_version", "")),
        "STATA_PLATFORM": str(ci.get("platform", "")),
        "STATA_VERSION": str(ci.get("stata_version", "")),
    }
    if any(not value for value in values.values()):
        raise GateError("Stata environment/toolchain identity is incomplete")
    environment = {
        "os": str(ci["platform"]), "arch": str(summary.get("stata_machine_type") or ci["platform"]),
        "python": platform.python_version(),
        "variables": [
            {"name": name, "value_sha256": sha256_bytes(value.encode("utf-8"))}
            for name, value in sorted(values.items())
        ],
    }
    outputs = [
        {
            "role": "ci_receipt", "filename": ci_receipt.name,
            "path": str(ci_receipt.resolve()),
            "sha256": sha256_bytes(ci_raw), "size_bytes": len(ci_raw),
        },
        {
            "role": "profile_summary", "filename": profile_summary.name,
            "path": str(profile_summary.resolve()),
            "sha256": sha256_bytes(summary_raw), "size_bytes": len(summary_raw),
        },
    ]
    payload = {
        "schema_version": 1, "receipt_type": RESULT_RECEIPT_TYPE, "check_id": check_id,
        "source": source, "public_state": public_state, "artifacts": artifacts,
        "claims": list(COMMON_CHECK_CLAIMS[check_id]),
        "producer": {
            "id": "licensed-stata-profile-adapter", "version": "1",
            "entrypoint_sha256": sha256_file(producer_entrypoint),
        },
        "execution": {
            "mode": "licensed_stata_profile", "argv": [],
            "argv_sha256": sha256_bytes(canonical_json_bytes([])), "exit_code": 0,
            "stdout": {"sha256": sha256_bytes(ci_raw), "size_bytes": len(ci_raw)},
            "stderr": {"sha256": sha256_bytes(summary_raw), "size_bytes": len(summary_raw)},
            "terminal_sentinel": result_sentinel(check_id), "environment": environment,
            "toolchain": f"Stata {ci['stata_version']} {ci['stata_bundle_version']}",
            "profile": expected_profile,
        },
        "outputs": outputs, "status": "pass", "completed": True,
        "complete_sentinel": RESULT_SENTINEL,
    }
    validate_result_payload(payload)
    _write_immutable(output, seal_payload(payload))


def seal_stata_profile_result(
    *, check_id: str, source_root: Path, source_sha: str, public_state: str,
    artifact_index: Path, ci_receipt: Path, profile_summary: Path,
    producer_entrypoint: Path, output: Path,
) -> None:
    """Seal only a fully retained, independently recomputable CI profile."""

    if check_id not in STATA_PROFILE_BY_CHECK:
        raise GateError("licensed-Stata adapter is not eligible for this check id")
    source_root = source_root.resolve()
    commit = _git(source_root, ["rev-parse", f"{source_sha}^{{commit}}"])
    source = {
        "commit": commit,
        "tree": _git(source_root, ["rev-parse", f"{commit}^{{tree}}"]),
    }
    audit_source(source_root, source, public_state)
    artifacts = _load_artifact_index(
        artifact_index.resolve(), source, source_root=source_root
    )
    exact_producer = (source_root / "ci/run_stata_ci.py").resolve()
    committed_producer = _git_blob(
        source_root, source["commit"], "ci/run_stata_ci.py"
    )
    if (
        producer_entrypoint.resolve() != exact_producer
        or exact_producer.read_bytes() != committed_producer
    ):
        raise GateError("Stata adapter requires the exact committed CI producer")
    ci, summary, raw_outputs, expected_command = _validate_stata_evidence_files(
        check_id=check_id, source_root=source_root, source=source,
        artifacts=artifacts, ci_receipt=ci_receipt.resolve(),
        profile_summary=profile_summary.resolve(),
    )
    values = {
        "RUNNER_NAME": str(ci["runner_name"]),
        "STATA_BUNDLE_VERSION": str(ci["stata_bundle_version"]),
        "STATA_PLATFORM": str(ci["platform"]),
        "STATA_VERSION": str(ci["stata_version"]),
    }
    if any(not value for value in values.values()):
        raise GateError("Stata environment/toolchain identity is incomplete")
    outputs = [
        _bound_output(path, role) for role, path in sorted(raw_outputs.items())
    ]
    process_stdout = raw_outputs["process_stdout"].read_bytes()
    process_stderr = raw_outputs["process_stderr"].read_bytes()
    payload = {
        "schema_version": 1,
        "receipt_type": RESULT_RECEIPT_TYPE,
        "check_id": check_id,
        "source": source,
        "public_state": public_state,
        "artifacts": artifacts,
        "claims": list(COMMON_CHECK_CLAIMS[check_id]),
        "producer": {
            "id": "licensed-stata-profile", "version": "1",
            "entrypoint_sha256": sha256_bytes(committed_producer),
        },
        "execution": {
            "mode": "licensed_stata_profile",
            "argv": expected_command,
            "argv_sha256": sha256_bytes(canonical_json_bytes(expected_command)),
            "exit_code": 0,
            "stdout": {
                "sha256": sha256_bytes(process_stdout),
                "size_bytes": len(process_stdout),
            },
            "stderr": {
                "sha256": sha256_bytes(process_stderr),
                "size_bytes": len(process_stderr),
            },
            "terminal_sentinel": result_sentinel(check_id),
            "environment": {
                "os": str(ci["platform"]),
                "arch": str(summary["stata_machine_type"]),
                "python": platform.python_version(),
                "variables": [
                    {
                        "name": name,
                        "value_sha256": sha256_bytes(value.encode("utf-8")),
                    }
                    for name, value in sorted(values.items())
                ],
            },
            "toolchain": f"Stata {ci['stata_version']} {ci['stata_bundle_version']}",
            "profile": STATA_PROFILE_BY_CHECK[check_id],
        },
        "outputs": outputs,
        "status": "pass",
        "completed": True,
        "complete_sentinel": RESULT_SENTINEL,
    }
    validate_result_payload(payload)
    _validate_stata_result(payload, source_root, source)
    _write_immutable(output, seal_payload(payload))


STAGED_SUMMARY_FIELDS = {
    "schema_version", "completed", "status", "archive_sha256", "archive_bytes",
    "source_sha", "manifest_sha256", "plugin", "plugin_sha256", "plugin_bytes",
    "stata_executable", "stata_executable_sha256", "stata_executable_size_bytes",
    "stata_version", "stata_edition", "stata_os", "stata_machine_type",
    "stata_processors", "tests_passed", "tests_failed", "results",
}


def _validate_staged_evidence_files(
    *, source_root: Path, source: Mapping[str, str], public_state: str,
    artifacts: Sequence[Mapping[str, Any]], summary_path: Path, archive_path: Path,
) -> tuple[Mapping[str, Any], dict[str, Path], list[str]]:
    evidence_root = summary_path.resolve().parent
    if summary_path.resolve() != evidence_root / "staged-summary.json":
        raise GateError("staged summary is outside the exact retained evidence layout")
    summary = _read_json(summary_path, "staged-release summary")
    _closed(summary, STAGED_SUMMARY_FIELDS, "staged-release summary")
    release = next(
        item for item in artifacts
        if item["role"] == "release_archive" and item["platform"] == "all"
    )
    archive = archive_path.resolve()
    if (
        archive.is_symlink() or not archive.is_file()
        or archive.name != release["filename"]
        or sha256_file(archive) != release["sha256"]
        or archive.stat().st_size != release["size_bytes"]
    ):
        raise GateError("staged release archive is absent or artifact-mixed")
    expected_fixtures = ["tests/stata/installation/installation_smoke.do"]
    if public_state == "activated":
        expected_fixtures.append(
            "tests/stata/installation/feppml_installation_smoke.do"
        )
    results = summary.get("results")
    if (
        summary["schema_version"] != 1 or summary["completed"] is not True
        or summary["status"] != "success" or summary["source_sha"] != source["commit"]
        or summary["archive_sha256"] != release["sha256"]
        or summary["archive_bytes"] != release["size_bytes"]
        or summary["tests_failed"] != 0
        or summary["tests_passed"] != len(expected_fixtures)
        or not isinstance(results, list)
        or [item.get("fixture") for item in results if isinstance(item, Mapping)]
        != expected_fixtures
    ):
        raise GateError("staged-release summary/inventory is incomplete or source-mixed")
    stata_executable = Path(str(summary["stata_executable"])).resolve()
    if (
        not stata_executable.is_file() or stata_executable.is_symlink()
        or sha256_file(stata_executable) != summary["stata_executable_sha256"]
        or stata_executable.stat().st_size != summary["stata_executable_size_bytes"]
    ):
        raise GateError("staged-release Stata executable identity changed")
    staged_module = _source_module(
        source_root, source, "scripts/run_staged_stata_tests.py",
        f"scripts._phase12_staged_runner_{source['commit']}",
    )
    release_module = _source_module(
        source_root, source, "scripts/stage_release.py",
        f"scripts._phase12_release_stager_{source['commit']}",
    )
    fixture_module = _source_module(
        source_root, source, "scripts/run_stata_tests.py",
        f"scripts._phase12_staged_fixture_runner_{source['commit']}",
    )
    with tempfile.TemporaryDirectory(prefix="phase12-staged-verify-") as temporary:
        try:
            staged = staged_module.extract_staged_release(
                archive, release["sha256"], Path(temporary) / "extracted"
            )
        except Exception as error:
            raise GateError(f"authoritative staged archive verification failed: {error}") from error
        plugin = next(
            (
                item for item in artifacts
                if item["role"] == "plugin" and item["filename"] == staged.plugin.name
            ),
            None,
        )
        if (
            staged.source_sha != source["commit"]
            or staged.archive_sha256 != release["sha256"]
            or staged.archive_bytes != release["size_bytes"]
            or plugin is None or staged.plugin_sha256 != plugin["sha256"]
            or staged.plugin_bytes != plugin["size_bytes"]
            or summary["manifest_sha256"] != staged.manifest_sha256
            or summary["plugin"] != staged.plugin.name
            or summary["plugin_sha256"] != staged.plugin_sha256
            or summary["plugin_bytes"] != staged.plugin_bytes
        ):
            raise GateError("staged archive/plugin manifest differs from aggregate artifacts")
        package_root = staged.package_root
        expected_plugins = tuple(release_module.EXPECTED_PLUGINS)
        plugin_artifacts = {
            str(item["filename"]): item for item in artifacts
            if item["role"] == "plugin"
        }
        if set(plugin_artifacts) != set(expected_plugins):
            raise GateError("release archive plugin inventory is not the exact qualified set")
        expected_names = (
            set(release_module.PRODUCTION_STATA_FILES)
            | set(release_module.ROOT_FILES)
            | set(expected_plugins)
            | {f"{name}.sha256" for name in expected_plugins}
            | {
                "DEPENDENCY_AUDIT.json", "SBOM.spdx.json",
                "MANIFEST.sha256", "RELEASE_MANIFEST.json",
            }
        )
        observed_names = {
            item.name for item in package_root.iterdir() if item.is_file()
        }
        if observed_names != expected_names:
            raise GateError("release archive member inventory is not exact")
        for name in release_module.PRODUCTION_STATA_FILES:
            if (package_root / name).read_bytes() != _git_blob(
                source_root, source["commit"], f"stata/{name}"
            ):
                raise GateError(f"release archive changed committed Stata file: {name}")
        for name in release_module.ROOT_FILES:
            if (package_root / name).read_bytes() != _git_blob(
                source_root, source["commit"], str(name)
            ):
                raise GateError(f"release archive changed committed root file: {name}")
        for name in expected_plugins:
            record = plugin_artifacts[name]
            plugin_path = package_root / name
            checksum_path = package_root / f"{name}.sha256"
            try:
                checksum_fields = checksum_path.read_text(encoding="ascii").strip().split()
            except (OSError, UnicodeDecodeError) as error:
                raise GateError(f"cannot read packaged plugin checksum: {name}") from error
            if (
                sha256_file(plugin_path) != record["sha256"]
                or plugin_path.stat().st_size != record["size_bytes"]
                or source["commit"][:12].encode("ascii") not in plugin_path.read_bytes()
                or checksum_fields != [record["sha256"], name]
            ):
                raise GateError(f"packaged plugin differs from qualified artifact: {name}")
        dependency_audit = package_root / "DEPENDENCY_AUDIT.json"
        try:
            release_module.validate_dependency_audit(
                source_root, dependency_audit, source["commit"]
            )
        except Exception as error:
            raise GateError(f"packaged dependency audit is invalid: {error}") from error
        expected_root = Path(temporary) / "expected-package"
        expected_root.mkdir()
        for name in release_module.PRODUCTION_STATA_FILES:
            (expected_root / name).write_bytes(
                _git_blob(source_root, source["commit"], f"stata/{name}")
            )
        for name in release_module.ROOT_FILES:
            (expected_root / name).write_bytes(
                _git_blob(source_root, source["commit"], str(name))
            )
        for name in expected_plugins:
            (expected_root / name).write_bytes((package_root / name).read_bytes())
            (expected_root / f"{name}.sha256").write_bytes(
                (package_root / f"{name}.sha256").read_bytes()
            )
        (expected_root / "DEPENDENCY_AUDIT.json").write_bytes(
            dependency_audit.read_bytes()
        )
        try:
            release_module.write_sbom(
                source_root, expected_root / "SBOM.spdx.json", source["commit"]
            )
            release_module.write_manifest(expected_root, source["commit"], False)
        except Exception as error:
            raise GateError(f"cannot reconstruct deterministic release metadata: {error}") from error
        for name in ("SBOM.spdx.json", "MANIFEST.sha256", "RELEASE_MANIFEST.json"):
            if (package_root / name).read_bytes() != (expected_root / name).read_bytes():
                raise GateError(f"packaged generated metadata is not deterministic: {name}")
    process_path = (evidence_root / "process.json").resolve()
    process_stdout = (evidence_root / "process.stdout.log").resolve()
    process_stderr = (evidence_root / "process.stderr.log").resolve()
    process = _read_json(process_path, "staged-release supervisor process")
    _closed(
        process,
        {
            "schema_version", "started_at", "command_executable", "argv", "cwd",
            "environment", "launch_error", "timed_out", "process_rc", "pid",
            "completed_at", "duration_seconds",
        },
        "staged-release supervisor process",
    )
    expected_command = [
        str(Path(sys.executable).resolve()),
        str((source_root / "scripts/run_staged_stata_tests.py").resolve()),
        "--stata", str(stata_executable),
        "--archive", str(archive),
        "--archive-sha256", str(release["sha256"]),
        "--log-dir", str((evidence_root / "fixture-logs").resolve()),
        "--summary-json", str(summary_path.resolve()),
        *[str((source_root / fixture).resolve()) for fixture in expected_fixtures],
    ]
    environment = process["environment"]
    if (
        process["schema_version"] != 1 or process["launch_error"] is not None
        or process["timed_out"] is not False or process["process_rc"] != 0
        or not isinstance(process["pid"], int) or isinstance(process["pid"], bool)
        or process["pid"] < 1 or process["command_executable"] != expected_command[0]
        or process["argv"] != expected_command
        or Path(str(process["cwd"])).resolve() != evidence_root
        or not isinstance(environment, list)
        or [item.get("name") for item in environment if isinstance(item, Mapping)]
        != list(STATA_PROCESS_ENVIRONMENT)
        or not process_stdout.is_file() or process_stdout.is_symlink()
        or not process_stderr.is_file() or process_stderr.is_symlink()
    ):
        raise GateError("staged-release supervisor invocation is not exact")
    expected_env = {
        "FEREG_BUILD_DATE": _commit_build_date(source_root, source["commit"]),
        "FEREG_GIT_COMMIT": source["commit"][:12],
    }
    for index, raw_environment in enumerate(environment):
        item = _mapping(raw_environment, f"staged process environment[{index}]")
        _closed(
            item, {"name", "present", "value_sha256"},
            f"staged process environment[{index}]",
        )
        name = str(item["name"])
        if not isinstance(item["present"], bool):
            raise GateError("staged process environment presence is malformed")
        if item["present"]:
            _sha(
                item["value_sha256"],
                f"staged process environment[{index}].value_sha256", 64,
            )
        elif item["value_sha256"] is not None:
            raise GateError("absent staged process environment has a value digest")
        if name in expected_env and (
            item["present"] is not True
            or item["value_sha256"]
            != sha256_bytes(expected_env[name].encode("utf-8"))
        ):
            raise GateError(f"staged process environment {name} is source-mixed")
    outputs: dict[str, Path] = {
        "process_json": process_path,
        "process_stderr": process_stderr,
        "process_stdout": process_stdout,
        "release_archive": archive,
        "staged_summary": summary_path.resolve(),
    }
    stdout_path = outputs["process_stdout"]
    if not stdout_path.is_file() or stdout_path.is_symlink():
        raise GateError("staged runner raw stdout is absent")
    stdout_lines = stdout_path.read_text(encoding="utf-8").splitlines()
    log_root = evidence_root / "fixture-logs"
    for index, (fixture, raw_result) in enumerate(zip(expected_fixtures, results)):
        result = _mapping(raw_result, f"staged fixture result[{index}]")
        _closed(result, STATA_RESULT_FIELDS, f"staged fixture result[{index}]")
        if result != {
            "fixture": fixture, "status": "success", "failure_kind": None,
            "failure_detail": None, "process_rc": 0, "stata_rc": 0,
        }:
            raise GateError(f"staged fixture did not pass exactly: {fixture}")
        fixture_path = source_root / fixture
        if (
            not fixture_path.is_file()
            or fixture_path.read_bytes() != _git_blob(source_root, source["commit"], fixture)
        ):
            raise GateError(f"staged fixture bytes differ from committed source: {fixture}")
        stable = fixture.replace("/", "__")
        log_path = log_root / f"{stable}.log"
        status_path = log_root / f"{stable}.status"
        log_text = log_path.read_text(encoding="utf-8")
        sentinel = fixture_module.expected_sentinel(fixture_path)
        if (
            fixture_module.validate_fixture_log(fixture_path, log_text) is not None
            or stdout_lines.count(f"PASS: {sentinel}") != 1
        ):
            raise GateError(f"staged fixture lacks its exact raw sentinel: {fixture}")
        metadata = _read_fixture_status(status_path, f"staged fixture status {fixture}")
        for key in (
            "stata_version", "stata_edition", "stata_os", "stata_machine_type",
            "stata_processors",
        ):
            if str(summary[key]) != metadata[key]:
                raise GateError(f"staged fixture status metadata is mixed: {fixture}")
        outputs[f"fixture_{index:03d}_log"] = log_path.resolve()
        outputs[f"fixture_{index:03d}_status"] = status_path.resolve()
    return summary, outputs, expected_command


def _validate_staged_result(
    payload: Mapping[str, Any], source_root: Path, source: Mapping[str, str]
) -> None:
    committed = _git_blob(
        source_root, source["commit"], "scripts/run_staged_stata_tests.py"
    )
    if payload["producer"] != {
        "id": "staged-release-installation", "version": "1",
        "entrypoint_sha256": sha256_bytes(committed),
    }:
        raise GateError("staged result does not name the exact committed producer")
    outputs = _verify_result_output_bytes(payload)
    if "staged_summary" not in outputs or "release_archive" not in outputs:
        raise GateError("staged result omits its primary raw evidence")
    summary, expected, expected_command = _validate_staged_evidence_files(
        source_root=source_root, source=source,
        public_state=str(payload["public_state"]), artifacts=payload["artifacts"],
        summary_path=outputs["staged_summary"],
        archive_path=outputs["release_archive"],
    )
    if outputs != expected:
        raise GateError("staged result output registry is not exact")
    process_raw = outputs["process_stdout"].read_bytes()
    process_error_raw = outputs["process_stderr"].read_bytes()
    values = {
        "STATA_EDITION": str(summary["stata_edition"]),
        "STATA_MACHINE_TYPE": str(summary["stata_machine_type"]),
        "STATA_OS": str(summary["stata_os"]),
        "STATA_VERSION": str(summary["stata_version"]),
    }
    execution = payload["execution"]
    if (
        execution["profile"] != "staged-release-installation"
        or execution["argv"] != expected_command
        or execution["stdout"] != {
            "sha256": sha256_bytes(process_raw), "size_bytes": len(process_raw)
        }
        or execution["stderr"] != {
            "sha256": sha256_bytes(process_error_raw),
            "size_bytes": len(process_error_raw),
        }
        or execution["environment"] != {
            "os": str(summary["stata_os"]),
            "arch": str(summary["stata_machine_type"]),
            "python": platform.python_version(),
            "variables": [
                {
                    "name": name,
                    "value_sha256": sha256_bytes(value.encode("utf-8")),
                }
                for name, value in sorted(values.items())
            ],
        }
        or execution["toolchain"]
        != f"Stata {summary['stata_version']} {summary['stata_edition']}"
    ):
        raise GateError("staged execution registry differs from retained raw evidence")


def seal_staged_release_result(
    *, source_root: Path, source_sha: str, public_state: str,
    artifact_index: Path, summary_path: Path, archive_path: Path,
    producer_entrypoint: Path, output: Path,
) -> None:
    source_root = source_root.resolve()
    commit = _git(source_root, ["rev-parse", f"{source_sha}^{{commit}}"])
    source = {
        "commit": commit,
        "tree": _git(source_root, ["rev-parse", f"{commit}^{{tree}}"]),
    }
    audit_source(source_root, source, public_state)
    artifacts = _load_artifact_index(
        artifact_index.resolve(), source, source_root=source_root
    )
    exact_producer = (source_root / "scripts/run_staged_stata_tests.py").resolve()
    committed_producer = _git_blob(
        source_root, source["commit"], "scripts/run_staged_stata_tests.py"
    )
    if (
        producer_entrypoint.resolve() != exact_producer
        or exact_producer.read_bytes() != committed_producer
    ):
        raise GateError("staged adapter requires the exact committed producer")
    summary, raw_outputs, expected_command = _validate_staged_evidence_files(
        source_root=source_root, source=source, public_state=public_state,
        artifacts=artifacts, summary_path=summary_path.resolve(),
        archive_path=archive_path.resolve(),
    )
    values = {
        "STATA_EDITION": str(summary["stata_edition"]),
        "STATA_MACHINE_TYPE": str(summary["stata_machine_type"]),
        "STATA_OS": str(summary["stata_os"]),
        "STATA_VERSION": str(summary["stata_version"]),
    }
    process_stdout = raw_outputs["process_stdout"].read_bytes()
    process_stderr = raw_outputs["process_stderr"].read_bytes()
    payload = {
        "schema_version": 1,
        "receipt_type": RESULT_RECEIPT_TYPE,
        "check_id": "staged_release",
        "source": source,
        "public_state": public_state,
        "artifacts": artifacts,
        "claims": list(COMMON_CHECK_CLAIMS["staged_release"]),
        "producer": {
            "id": "staged-release-installation", "version": "1",
            "entrypoint_sha256": sha256_bytes(committed_producer),
        },
        "execution": {
            "mode": "staged_release_installation",
            "argv": expected_command,
            "argv_sha256": sha256_bytes(canonical_json_bytes(expected_command)),
            "exit_code": 0,
            "stdout": {
                "sha256": sha256_bytes(process_stdout),
                "size_bytes": len(process_stdout),
            },
            "stderr": {
                "sha256": sha256_bytes(process_stderr),
                "size_bytes": len(process_stderr),
            },
            "terminal_sentinel": result_sentinel("staged_release"),
            "environment": {
                "os": str(summary["stata_os"]),
                "arch": str(summary["stata_machine_type"]),
                "python": platform.python_version(),
                "variables": [
                    {
                        "name": name,
                        "value_sha256": sha256_bytes(value.encode("utf-8")),
                    }
                    for name, value in sorted(values.items())
                ],
            },
            "toolchain": f"Stata {summary['stata_version']} {summary['stata_edition']}",
            "profile": "staged-release-installation",
        },
        "outputs": [
            _bound_output(path, role) for role, path in sorted(raw_outputs.items())
        ],
        "status": "pass",
        "completed": True,
        "complete_sentinel": RESULT_SENTINEL,
    }
    validate_result_payload(payload)
    _validate_staged_result(payload, source_root, source)
    _write_immutable(output, seal_payload(payload))


def make_check_receipt(
    *, check_id: str, source_root: Path, source_sha: str, public_state: str,
    artifact_index: Path, producer_id: str, producer_version: str,
    producer_entrypoint: Path, upstream: Sequence[tuple[str, Path]], output: Path,
) -> None:
    if check_id not in COMMON_CHECK_CLAIMS:
        raise GateError(f"unsupported check id: {check_id}")
    source_root = source_root.resolve()
    commit = _git(source_root, ["rev-parse", f"{source_sha}^{{commit}}"])
    tree = _git(source_root, ["rev-parse", f"{commit}^{{tree}}"])
    source = {"commit": commit, "tree": tree}
    audit_source(source_root, source, public_state)
    artifacts = _load_artifact_index(
        artifact_index.resolve(), source, source_root=source_root
    )
    gate_entrypoint = (source_root / "scripts/feppml_activation_gate.py").resolve()
    if (
        producer_id != "feppml-activation-gate" or producer_version != "1"
        or producer_entrypoint.resolve() != gate_entrypoint
        or sha256_file(gate_entrypoint) != sha256_bytes(
            _git_blob(source_root, commit, "scripts/feppml_activation_gate.py")
        )
    ):
        raise GateError("check sealer must be the exact committed activation gate")
    expected_roles = dict(UPSTREAM_ROLE_TYPES[check_id])
    supplied_roles = [role for role, _path in sorted(upstream, key=lambda item: item[0])]
    if supplied_roles != sorted(expected_roles):
        raise GateError(
            f"{check_id} upstream arguments are not exact; "
            f"expected={sorted(expected_roles)}, observed={supplied_roles}"
        )
    upstream_records = []
    authoritative_inputs: dict[str, tuple[Mapping[str, Any], Path, str]] = {}
    for role, path in sorted(upstream, key=lambda item: item[0]):
        path = path.resolve()
        expected_type = expected_roles[role]
        payload, file_sha, size_bytes, receipt_type, encoding = _read_upstream(
            path, f"upstream receipt {role}", expected_type
        )
        authoritative_inputs[role] = (payload, path, receipt_type)
        upstream_records.append(
            {
                "role": role, "receipt_type": receipt_type, "encoding": encoding,
                "path": str(path), "file_sha256": file_sha,
                "payload_sha256": sha256_bytes(canonical_json_bytes(payload)),
                "size_bytes": size_bytes,
            }
        )
    _validate_upstream_outcome(
        check_id, authoritative_inputs, source, public_state, artifacts, source_root
    )
    payload = {
        "schema_version": 1, "receipt_type": CHECK_RECEIPT_TYPE, "check_id": check_id,
        "source": source, "public_state": public_state, "artifacts": artifacts,
        "claims": list(COMMON_CHECK_CLAIMS[check_id]),
        "producer": {
            "id": producer_id, "version": producer_version,
            "entrypoint_sha256": sha256_file(gate_entrypoint),
        },
        "upstream": upstream_records, "status": "pass", "waivers": [],
        "complete_sentinel": CHECK_SENTINEL,
    }
    validate_check_payload(payload)
    _write_immutable(output, seal_payload(payload))


def _write_immutable(path: Path, document: Mapping[str, Any]) -> None:
    data = canonical_json_bytes(document)
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise GateError(f"refusing to overwrite immutable receipt: {path}") from error
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
    except BaseException:
        try:
            path.unlink()
        except OSError:
            pass
        raise


def evaluate(index_path: Path, source_root: Path) -> dict[str, Any]:
    index = _read_json(index_path, "aggregate index")
    _closed(
        index,
        {
            "schema_version", "evidence_type", "public_state", "source", "artifacts",
            "receipts", "activation_lineage", "waivers", "complete_sentinel",
        },
        "aggregate index",
    )
    if (
        index["schema_version"] != 1 or index["evidence_type"] != INDEX_TYPE
        or index["public_state"] not in PUBLIC_STATES or index["waivers"] != []
        or index["complete_sentinel"] != INDEX_SENTINEL
    ):
        raise GateError("aggregate index identity, state, waiver, or sentinel is invalid")
    public_state = str(index["public_state"])
    source = _source(index["source"], "aggregate source")
    source_contract = audit_source(source_root, source, public_state)
    activation_lineage = audit_activation_lineage(
        source_root.resolve(), source, public_state, index["activation_lineage"],
        index_base=index_path.parent,
    )

    artifact_identities: list[dict[str, Any]] = []
    artifact_records: dict[tuple[str, str], dict[str, Any]] = {}
    seen_artifacts: set[tuple[str, str]] = set()
    for number, raw in enumerate(index["artifacts"]):
        identity, file_record = _artifact_file(
            raw, f"aggregate artifacts[{number}]", source, index_path.parent
        )
        key = (identity["role"], identity["platform"])
        if key in seen_artifacts:
            raise GateError(f"duplicate aggregate artifact identity: {key}")
        seen_artifacts.add(key)
        artifact_identities.append(identity)
        artifact_records[key] = file_record
    artifact_identities.sort(key=lambda item: (item["role"], item["platform"]))
    if seen_artifacts != REQUIRED_ARTIFACT_KEYS:
        raise GateError(
            f"aggregate artifact inventory is incomplete; missing={sorted(REQUIRED_ARTIFACT_KEYS-seen_artifacts)}, "
            f"extra={sorted(seen_artifacts-REQUIRED_ARTIFACT_KEYS)}"
        )
    _verify_exact_source_archive(
        source_root.resolve(), source, artifact_records[("source_archive", "source")]
    )

    receipts = index["receipts"]
    if not isinstance(receipts, list):
        raise GateError("aggregate receipts must be a list")
    required = STATE_REQUIRED[public_state]
    observed: dict[str, dict[str, Any]] = {}
    for number, raw in enumerate(receipts):
        record = _mapping(raw, f"aggregate receipts[{number}]")
        _closed(record, RECEIPT_RECORD_FIELDS, f"aggregate receipts[{number}]")
        check_id = record["id"]
        if not isinstance(check_id, str) or check_id in observed:
            raise GateError("aggregate receipt ids must be unique strings")
        raw_path = Path(str(record["path"]))
        path = (
            (index_path.parent / raw_path).resolve()
            if not raw_path.is_absolute() else raw_path.resolve()
        )
        if raw_path.is_symlink() or path.is_symlink() or not path.is_file():
            raise GateError(f"aggregate check receipt is absent or a symlink: {check_id}")
        payload, file_sha, size_bytes = verify_check_receipt(
            path, source_root=source_root.resolve()
        )
        expected_record = {
            "id": check_id, "path": record["path"], "sha256": file_sha,
            "size_bytes": size_bytes,
            "payload_sha256": seal_payload(payload)["seal"]["payload_sha256"],
        }
        if dict(record) != expected_record:
            raise GateError(f"aggregate receipt file binding changed: {check_id}")
        if payload["check_id"] != check_id:
            raise GateError(f"aggregate receipt id/payload mismatch: {check_id}")
        if payload["source"] != source:
            raise GateError(f"mixed source SHA/tree in aggregate receipt: {check_id}")
        if payload["public_state"] != public_state:
            raise GateError(f"mixed public_state in aggregate receipt: {check_id}")
        if payload["artifacts"] != artifact_identities:
            raise GateError(f"mixed or incomplete artifact identities: {check_id}")
        observed[check_id] = {
            "id": check_id, "file_sha256": file_sha,
            "payload_sha256": expected_record["payload_sha256"],
            "size_bytes": size_bytes, "claims": payload["claims"],
            "producer": payload["producer"], "upstream": payload["upstream"],
        }
    if set(observed) != required:
        raise GateError(
            f"aggregate prerequisite inventory is not exact; "
            f"missing={sorted(required-set(observed))}, extra={sorted(set(observed)-required)}"
        )
    qualification_upstream = {
        item["role"]: Path(str(item["path"])).resolve()
        for item in observed["feppml_qualification"]["upstream"]
    }
    scc_upstream = {
        item["role"]: Path(str(item["path"])).resolve()
        for item in observed["scc_feppml"]["upstream"]
    }
    scc_root = scc_upstream["scc_close"].parent.parent
    if (
        qualification_upstream["qualification_receipt"]
        != (scc_root / "receipts/qualification.json").resolve()
        or qualification_upstream["qualification_evidence"]
        != (scc_root / "output/evidence.json").resolve()
    ):
        raise GateError(
            "feppml qualification is not the exact evidence/receipt from the verified SCC run"
        )
    f0_upstream = {
        item["role"]: Path(str(item["path"])).resolve()
        for item in observed["fereg_f0"]["upstream"]
    }
    f0_scc_upstream = {
        item["role"]: Path(str(item["path"])).resolve()
        for item in observed["scc_fereg_f0"]["upstream"]
    }
    f0_scc_root = f0_scc_upstream["scc_close"].parent.parent
    if f0_upstream["f0_qualification"] != (
        f0_scc_root / "receipts/qualification.json"
    ).resolve():
        raise GateError(
            "F0 qualification is not the exact receipt from the verified SCC run"
        )
    return {
        "schema_version": 1, "receipt_type": AGGREGATE_RECEIPT_TYPE,
        "public_state": public_state, "source": source, "source_contract": source_contract,
        "activation_lineage": activation_lineage,
        "input_index_sha256": sha256_file(index_path), "artifacts": artifact_identities,
        "prerequisites": [observed[name] for name in sorted(observed)],
        "status": "pass", "waivers": [], "complete_sentinel": AGGREGATE_SENTINEL,
    }


def _parse_aggregate_receipt(path: Path) -> dict[str, Any]:
    raw = path.read_bytes()
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as error:
        raise GateError(f"aggregate receipt is not JSON: {error}") from error
    envelope = _mapping(document, "aggregate receipt")
    _closed(envelope, {"payload", "seal"}, "aggregate receipt")
    if raw != canonical_json_bytes(envelope):
        raise GateError("aggregate receipt is not in canonical immutable encoding")
    payload = _mapping(envelope["payload"], "aggregate receipt payload")
    expected_fields = {
        "schema_version", "receipt_type", "public_state", "source", "source_contract",
        "activation_lineage", "input_index_sha256", "artifacts", "prerequisites",
        "status", "waivers",
        "complete_sentinel",
    }
    _closed(payload, expected_fields, "aggregate receipt payload")
    if envelope["seal"] != seal_payload(payload)["seal"]:
        raise GateError("aggregate receipt seal mismatch")
    if (
        payload["schema_version"] != 1
        or payload["receipt_type"] != AGGREGATE_RECEIPT_TYPE
        or payload["public_state"] not in PUBLIC_STATES
        or payload["status"] != "pass" or payload["waivers"] != []
        or payload["complete_sentinel"] != AGGREGATE_SENTINEL
    ):
        raise GateError("aggregate receipt identity or verdict is invalid")
    _source(payload["source"], "aggregate receipt source")
    _sha(payload["input_index_sha256"], "aggregate receipt input_index_sha256", 64)
    contract = _mapping(payload["source_contract"], "aggregate receipt source_contract")
    if contract.get("status") != "pass" or contract.get("public_state") != payload["public_state"]:
        raise GateError("aggregate source-contract verdict is invalid")
    expected_contract_verdict = {
        "preactivation_stub": "fail_closed_r498_private_files_unpublished",
        "activated": "one_call_wrapper_private_files_published",
    }[payload["public_state"]]
    if contract.get("verdict") != expected_contract_verdict:
        raise GateError("aggregate source-contract verdict does not match public_state")
    source = _source(payload["source"], "aggregate receipt source")
    artifacts_raw = payload["artifacts"]
    if not isinstance(artifacts_raw, list) or not isinstance(payload["prerequisites"], list):
        raise GateError("aggregate receipt inventories are malformed")
    artifacts = [
        _artifact_identity(item, f"aggregate receipt artifacts[{index}]", source)
        for index, item in enumerate(artifacts_raw)
    ]
    if {(item["role"], item["platform"]) for item in artifacts} != REQUIRED_ARTIFACT_KEYS:
        raise GateError("aggregate receipt artifact inventory is incomplete")
    prerequisite_ids: list[str] = []
    for index, raw_record in enumerate(payload["prerequisites"]):
        record = _mapping(raw_record, f"aggregate receipt prerequisites[{index}]")
        _closed(
            record,
            {
                "id", "file_sha256", "payload_sha256", "size_bytes", "claims",
                "producer", "upstream",
            },
            f"aggregate receipt prerequisites[{index}]",
        )
        check_id = record["id"]
        if check_id not in COMMON_CHECK_CLAIMS or record["claims"] != list(COMMON_CHECK_CLAIMS[check_id]):
            raise GateError("aggregate receipt prerequisite claims are invalid")
        _sha(record["file_sha256"], "aggregate prerequisite file digest", 64)
        _sha(record["payload_sha256"], "aggregate prerequisite payload digest", 64)
        _integer(record["size_bytes"], "aggregate prerequisite size", 1)
        prerequisite_ids.append(check_id)
    if prerequisite_ids != sorted(STATE_REQUIRED[payload["public_state"]]):
        raise GateError("aggregate receipt prerequisite inventory is not state-complete")
    lineage = payload["activation_lineage"]
    if payload["public_state"] == "preactivation_stub":
        if lineage is not None:
            raise GateError("preactivation aggregate receipt contains activation lineage")
    else:
        lineage_record = _mapping(lineage, "aggregate activation lineage")
        if (
            lineage_record.get("status") != "pass"
            or lineage_record.get("verdict") != "minimal_control_plane_activation"
            or _source(lineage_record.get("parent_source"), "lineage parent") == source
            or not isinstance(lineage_record.get("changed_paths"), list)
        ):
            raise GateError("activated aggregate receipt has no passing minimal lineage")
    return dict(payload)


def verify_aggregate_receipt(
    path: Path, index_path: Path, source_root: Path
) -> dict[str, Any]:
    """Reopen every authoritative input and reproduce an aggregate receipt."""

    receipt = path.resolve()
    index = index_path.resolve()
    source = source_root.resolve()
    payload = _parse_aggregate_receipt(receipt)
    if not index.is_file() or index.is_symlink():
        raise GateError("aggregate verification index is absent or a symlink")
    if payload["input_index_sha256"] != sha256_file(index):
        raise GateError("aggregate verification index bytes differ from the receipt")
    recomputed = evaluate(index, source)
    try:
        document = json.loads(receipt.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise GateError(f"cannot reopen aggregate receipt: {error}") from error
    if recomputed != payload or document != seal_payload(recomputed):
        raise GateError(
            "aggregate receipt cannot be reproduced from authoritative inputs"
        )
    return payload


def _parse_upstream(values: Sequence[str]) -> list[tuple[str, Path]]:
    parsed: list[tuple[str, Path]] = []
    for value in values:
        role, separator, path = value.partition("=")
        if not separator or not role or not path:
            raise GateError("--upstream must be ROLE=PATH")
        parsed.append((role, Path(path)))
    return parsed


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    close = commands.add_parser("seal-check", help="seal one already-passing prerequisite")
    close.add_argument("--check-id", choices=sorted(COMMON_CHECK_CLAIMS), required=True)
    close.add_argument("--source-root", type=Path, required=True)
    close.add_argument("--source-sha", required=True)
    close.add_argument("--public-state", choices=sorted(PUBLIC_STATES), required=True)
    close.add_argument("--artifact-index", type=Path, required=True)
    close.add_argument("--producer-id", required=True)
    close.add_argument("--producer-version", required=True)
    close.add_argument("--producer-entrypoint", type=Path, required=True)
    close.add_argument("--upstream", action="append", default=[], metavar="ROLE=PATH")
    close.add_argument("--receipt", type=Path, required=True)
    run = commands.add_parser(
        "run-check", help="execute and seal one closed-argv local/native prerequisite"
    )
    run.add_argument("--check-id", choices=sorted(GENERIC_COMMAND_CHECKS), required=True)
    run.add_argument("--source-root", type=Path, required=True)
    run.add_argument("--source-sha", required=True)
    run.add_argument("--public-state", choices=sorted(PUBLIC_STATES), required=True)
    run.add_argument("--artifact-index", type=Path, required=True)
    run.add_argument("--producer-id", required=True)
    run.add_argument("--producer-version", required=True)
    run.add_argument("--producer-entrypoint", type=Path, required=True)
    run.add_argument("--environment-key", action="append", default=[])
    run.add_argument("--toolchain", required=True)
    run.add_argument("--profile", required=True)
    run.add_argument(
        "--output", action="append", nargs=3, default=[],
        metavar=("ROLE", "PATH", "SHA256"),
    )
    run.add_argument("--timeout-seconds", type=int, required=True)
    run.add_argument("--receipt", type=Path, required=True)
    run.add_argument("argv", nargs=argparse.REMAINDER)
    stata = commands.add_parser(
        "seal-stata-profile", help="adapt an exact-source licensed Stata profile"
    )
    stata.add_argument("--check-id", choices=sorted(STATA_PROFILE_BY_CHECK), required=True)
    stata.add_argument("--source-root", type=Path, required=True)
    stata.add_argument("--source-sha", required=True)
    stata.add_argument("--public-state", choices=sorted(PUBLIC_STATES), required=True)
    stata.add_argument("--artifact-index", type=Path, required=True)
    stata.add_argument("--ci-receipt", type=Path, required=True)
    stata.add_argument("--profile-summary", type=Path, required=True)
    stata.add_argument("--producer-entrypoint", type=Path, required=True)
    stata.add_argument("--receipt", type=Path, required=True)
    staged = commands.add_parser(
        "seal-staged-release", help="adapt the licensed staged-installation summary"
    )
    staged.add_argument("--source-root", type=Path, required=True)
    staged.add_argument("--source-sha", required=True)
    staged.add_argument("--public-state", choices=sorted(PUBLIC_STATES), required=True)
    staged.add_argument("--artifact-index", type=Path, required=True)
    staged.add_argument("--summary", type=Path, required=True)
    staged.add_argument("--archive", type=Path, required=True)
    staged.add_argument("--producer-entrypoint", type=Path, required=True)
    staged.add_argument("--receipt", type=Path, required=True)
    aggregate = commands.add_parser("evaluate", help="close the complete evidence set")
    aggregate.add_argument("--index", type=Path, required=True)
    aggregate.add_argument("--source-root", type=Path, required=True)
    aggregate.add_argument("--receipt", type=Path, required=True)
    verify = commands.add_parser(
        "verify-receipt",
        help="recompute a merged receipt from its authoritative index and source",
    )
    verify.add_argument("--receipt", type=Path, required=True)
    verify.add_argument("--index", type=Path, required=True)
    verify.add_argument("--source-root", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if args.command == "seal-check":
            make_check_receipt(
                check_id=args.check_id, source_root=args.source_root,
                source_sha=args.source_sha, public_state=args.public_state,
                artifact_index=args.artifact_index, producer_id=args.producer_id,
                producer_version=args.producer_version,
                producer_entrypoint=args.producer_entrypoint,
                upstream=_parse_upstream(args.upstream), output=args.receipt,
            )
            print(f"FEPPML_PHASE12_CHECK_SEALED|{args.check_id}")
            return 0
        if args.command == "run-check":
            argv = list(args.argv)
            if argv and argv[0] == "--":
                argv.pop(0)
            run_closed_argv_result(
                check_id=args.check_id, source_root=args.source_root,
                source_sha=args.source_sha, public_state=args.public_state,
                artifact_index=args.artifact_index, producer_id=args.producer_id,
                producer_version=args.producer_version,
                producer_entrypoint=args.producer_entrypoint,
                environment_keys=sorted(args.environment_key), toolchain=args.toolchain,
                profile_name=args.profile, declared_outputs=args.output,
                timeout_seconds=args.timeout_seconds, argv=argv, output=args.receipt,
            )
            print(f"FEPPML_PHASE12_RESULT_SEALED|{args.check_id}")
            return 0
        if args.command == "seal-stata-profile":
            seal_stata_profile_result(
                check_id=args.check_id, source_root=args.source_root,
                source_sha=args.source_sha, public_state=args.public_state,
                artifact_index=args.artifact_index, ci_receipt=args.ci_receipt,
                profile_summary=args.profile_summary,
                producer_entrypoint=args.producer_entrypoint, output=args.receipt,
            )
            print(f"FEPPML_PHASE12_STATA_RESULT_SEALED|{args.check_id}")
            return 0
        if args.command == "seal-staged-release":
            seal_staged_release_result(
                source_root=args.source_root, source_sha=args.source_sha,
                public_state=args.public_state, artifact_index=args.artifact_index,
                summary_path=args.summary, archive_path=args.archive,
                producer_entrypoint=args.producer_entrypoint, output=args.receipt,
            )
            print("FEPPML_PHASE12_STAGED_RESULT_SEALED|staged_release")
            return 0
        if args.command == "evaluate":
            payload = evaluate(args.index.resolve(), args.source_root)
            # The aggregate receipt is deliberately the final filesystem write.
            _write_immutable(args.receipt, seal_payload(payload))
            print(f"FEPPML_PHASE12_AGGREGATE_PASS|{payload['public_state']}")
            return 0
        payload = verify_aggregate_receipt(
            args.receipt, args.index, args.source_root
        )
        print(f"FEPPML_PHASE12_AGGREGATE_RECEIPT_OK|{payload['public_state']}")
        return 0
    except (GateError, OSError) as error:
        print(f"feppml activation gate failed closed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
