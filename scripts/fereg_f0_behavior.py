#!/usr/bin/env python3
"""Capture and compare the frozen public-fereg behavioral contract.

The performance matrix deliberately times only representative estimation
cells.  This companion contract runs no timing loop.  It records exact package
inventories, byte-identical fereg-owned control-plane files, executable help
blocks, and normalized licensed-Stata fixture outcomes for an exact artifact.
An immutable F0 observation can therefore be compared with any later exact
candidate without changing or contaminating the performance experiment.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
from typing import Any, Mapping, Sequence

try:
    from scripts import run_stata_tests as stata_runner
except ImportError:  # direct ``python scripts/fereg_f0_behavior.py``
    _runner_path = Path(__file__).resolve().with_name("run_stata_tests.py")
    _runner_spec = importlib.util.spec_from_file_location(
        "fereg_f0_exact_run_stata_tests", _runner_path
    )
    if _runner_spec is None or _runner_spec.loader is None:
        raise RuntimeError("cannot load exact Stata runner")
    stata_runner = importlib.util.module_from_spec(_runner_spec)  # type: ignore[assignment]
    sys.modules[_runner_spec.name] = stata_runner
    _runner_spec.loader.exec_module(stata_runner)


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/fereg_f0_behavior_v1.json"
DEFAULT_F0_MANIFEST = ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json"
DARWIN_GIT = Path("/usr/bin/git")
DARWIN_GIT_SHA256 = "b8763cf250e607a778bb4603cecb5b90338814d0a3dfcba0d57b1de242f610e9"
DARWIN_GIT_SIZE = 118_640
DARWIN_STATA = Path("/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp")
DARWIN_STATA_SHA256 = "178db3b78a3768ebfe88c36e845df292611b4b8ef0342c47a58e4641d0e6a16b"
DARWIN_STATA_SIZE = 119_946_864
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
F0_SOURCE = {
    "commit": "2dee5d8c4cd99b51bbaca600348f66f325b58ab4",
    "tree": "cc10b50077f162cab8be10702a9627b99ee5eea2",
    "cargo_lock_sha256": "4dd5db4d5b7f889cc664b522b84ac46dd10b8ca83f15af371cd6acf1239b4e42",
    "references_lock_sha256": "48aec6f93ea758987734f702dde87853da56342b228bae927b7bbebcf5b3f801",
}
REQUIRED_CATEGORIES = (
    "syntax", "defaults", "validation", "error_codes", "utility_modes",
    "help_examples", "package_inventory", "prediction", "test", "estat",
    "failure_atomicity",
)
FIXTURE_IDS = (
    "fereg_f0_public_contract", "ordinary_factor_terms",
    "reject_ambiguous_absorb", "help_examples",
)
NORMALIZED_OUTPUT_FORMAT = "fereg-key-lines-v1"
OUTPUT_BEGIN = "FEREG_F0_OUTPUT_BEGIN|"
OUTPUT_END = "FEREG_F0_OUTPUT_END|"
UTILITY_ACTIONS = ("version", "doctor", "selftest")
UTILITY_FIXTURE = "fereg_f0_public_contract"
VOLATILE_OUTPUT_KEYS = {"fereg.git_commit", "fereg.build_date"}
RUNTIME_PLUGIN_PATH_OUTPUT_VALUE = "<runtime-plugin-path>"
F0_VERSION_OUTPUT_KEYS = (
    "fereg.plugin_version",
    "fereg.ado_contract",
    "fereg.action_protocol",
    "fereg.model_wire",
    "fereg.ols_result_wire",
    "fereg.plugin_abi",
    "fereg.git_commit",
    "fereg.build_date",
)
FROZEN_FIXTURES = (
    {
        "id": "fereg_f0_public_contract",
        "path": "tests/stata/qualification/fereg_f0_public_contract.do",
        "sha256": "a0a34f1542c748bd9c40427010c25849dfeee9220507e2c29a005d2c6c30e0a6",
        "categories": [
            "syntax", "defaults", "validation", "error_codes", "utility_modes",
            "prediction", "test", "estat", "failure_atomicity",
        ],
    },
    {
        "id": "ordinary_factor_terms",
        "path": "tests/stata/parser/ordinary_factor_terms.do",
        "sha256": "a49ef58cba0e6ea93a95d89c4a5303e280f75d210553f26f44010a2634d15236",
        "categories": ["syntax", "defaults"],
    },
    {
        "id": "reject_ambiguous_absorb",
        "path": "tests/stata/parser/reject_ambiguous_absorb.do",
        "sha256": "591a9b9e7d6869f7a99d5abaacc310e3127280f1bc60556ec865e17f6c11a992",
        "categories": ["validation", "error_codes"],
    },
    {
        "id": "help_examples",
        "path": "tests/stata/documentation/help_examples.do",
        "sha256": "db79bea7d1ca100f272d036bf26fc06e86f8faa526317224f7682f35edb65294",
        "categories": ["help_examples", "prediction", "test", "estat"],
    },
)
FROZEN_RESOURCE = {
    "path": "tests/fixtures/ols/ols_core.csv",
    "sha256": "cb7d7e7ae21843f7c1dc5f84ac4daffa3954c9bfc54a9a109a3e2f6b50433bca",
}
BASELINE_MEMBERS = (
    "fereg.ado", "fereg_run.ado", "_fereg_estimate_public.ado",
    "_fereg_ols.ado", "_fereg_estimate_ols.ado", "_fereg_load.ado",
    "feppml.ado", "fereg_parse.ado", "fereg_p.ado", "fereg_estat.ado",
    "fereg.sthlp", "feppml.sthlp", "fereg_parse.sthlp",
    "fereg_postestimation.sthlp", "fereg_windows.plugin",
    "fereg_linux.plugin", "fereg_macos.plugin",
)
BYTE_IDENTICAL_FEREG_MEMBERS = (
    "fereg.ado", "fereg_run.ado", "_fereg_estimate_public.ado",
    "_fereg_ols.ado", "_fereg_estimate_ols.ado", "_fereg_load.ado",
    "fereg_parse.ado", "fereg_p.ado", "fereg_estat.ado",
    "fereg_parse.sthlp", "fereg_postestimation.sthlp",
)
HELP_EXAMPLE_IDS = (
    "basic_two_way", "factor_if", "weights", "clusters", "string_fe",
    "interacted_fe", "absorbed_slope", "slope_only", "saved_outputs",
    "disconnected", "low_memory", "benchmark",
)
ACTIVATION_ADDED_MEMBERS = (
    "_fereg_ppml.ado", "_fereg_estimate_ppml.ado", "feppml_p.ado",
)
ACTIVATION_CHANGED_SHARED_FILES = (
    "fereg.pkg", "stata.toc", "fereg.sthlp", "feppml.ado", "feppml.sthlp",
)
PLATFORMS = {
    "Darwin": ("macos-universal", "fereg_macos.plugin"),
    "Linux": ("linux-x86_64", "fereg_linux.plugin"),
    "Windows": ("windows-x86_64", "fereg_windows.plugin"),
}


class BehaviorContractError(ValueError):
    """The direct F0 behavioral contract failed closed."""


def canonical_json_bytes(value: Any) -> bytes:
    try:
        return json.dumps(
            value, sort_keys=True, separators=(",", ":"), ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, ValueError) as error:
        raise BehaviorContractError(f"value is not canonical finite JSON: {error}") from error


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise BehaviorContractError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise BehaviorContractError(f"{where} must be an object")
    return value


def _keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise BehaviorContractError(
            f"{where} fields are not closed; "
            f"missing={sorted(expected - set(value))}, extra={sorted(set(value) - expected)}"
        )


def _read_json(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BehaviorContractError(f"cannot read {where}: {error}") from error
    return _mapping(value, where)


def _file_record(path: Path) -> dict[str, Any]:
    source = Path(path)
    resolved = source.resolve()
    if source.is_symlink() or not resolved.is_file() or resolved.is_symlink():
        raise BehaviorContractError(f"contract input is not a regular non-symlink file: {source}")
    return {
        "sha256": sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }


def _write_new(path: Path, value: Any) -> None:
    data = json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True).encode("ascii") + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise BehaviorContractError(f"refusing to overwrite immutable output: {path}") from error
    with os.fdopen(descriptor, "wb") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def seal_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "payload": dict(payload),
        "seal": {
            "algorithm": "sha256",
            "payload_sha256": sha256_bytes(canonical_json_bytes(payload)),
        },
    }


def _sha256(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA256.fullmatch(value) is None:
        raise BehaviorContractError(f"{where} must be a lowercase SHA-256")
    return value


def load_manifest(path: Path = DEFAULT_MANIFEST) -> Mapping[str, Any]:
    manifest = _read_json(path, "behavior manifest")
    validate_manifest(manifest, root=ROOT)
    return manifest


def validate_manifest(manifest: Mapping[str, Any], *, root: Path | None = ROOT) -> None:
    _keys(
        manifest,
        {
            "schema_version", "manifest_id", "status", "baseline_source",
            "baseline_versions", "required_categories", "fixtures", "resources",
            "package_contract", "receipt_contract",
        },
        "behavior manifest",
    )
    if (
        manifest["schema_version"] != 1
        or manifest["manifest_id"] != "fereg_f0_behavior_v1"
        or manifest["status"] != "frozen_contract"
        or manifest["baseline_source"] != F0_SOURCE
        or manifest["baseline_versions"] != {
            "ado": "0.5.0", "action_protocol": "1.6", "model_wire": "2.1",
            "ols_result_wire": "1.5",
        }
        or manifest["required_categories"] != list(REQUIRED_CATEGORIES)
    ):
        raise BehaviorContractError("behavior manifest identity or F0 contract changed")

    fixtures = manifest["fixtures"]
    if fixtures != list(FROZEN_FIXTURES):
        raise BehaviorContractError("frozen behavior fixture definitions changed")
    covered = {"package_inventory"}
    for index, raw in enumerate(fixtures):
        fixture = _mapping(raw, f"fixture[{index}]")
        _keys(fixture, {"id", "path", "sha256", "categories"}, f"fixture[{index}]")
        _sha256(fixture["sha256"], f"fixture[{index}].sha256")
        categories = fixture["categories"]
        if not isinstance(categories, list) or not categories or not set(categories) <= set(REQUIRED_CATEGORIES):
            raise BehaviorContractError(f"fixture {fixture['id']} categories are invalid")
        covered.update(categories)
        if root is not None and sha256_file(root / fixture["path"]) != fixture["sha256"]:
            raise BehaviorContractError(f"frozen fixture changed: {fixture['id']}")
    if covered != set(REQUIRED_CATEGORIES):
        raise BehaviorContractError("behavior fixture coverage is incomplete")

    resources = manifest["resources"]
    if resources != [FROZEN_RESOURCE]:
        raise BehaviorContractError("frozen behavior resource definition changed")
    resource = _mapping(resources[0], "resource[0]")
    _keys(resource, {"path", "sha256"}, "resource[0]")
    if resource["path"] != "tests/fixtures/ols/ols_core.csv":
        raise BehaviorContractError("behavior resource path changed")
    _sha256(resource["sha256"], "resource[0].sha256")
    if root is not None and sha256_file(root / resource["path"]) != resource["sha256"]:
        raise BehaviorContractError("frozen behavior resource changed")

    package = _mapping(manifest["package_contract"], "package_contract")
    _keys(
        package,
        {
            "format_version", "package_name", "package_version", "baseline_members",
            "byte_identical_fereg_members", "help_example_ids", "activation",
        },
        "package_contract",
    )
    baseline_members = package["baseline_members"]
    byte_members = package["byte_identical_fereg_members"]
    if (
        package["format_version"] != 3
        or package["package_name"] != "fereg"
        or package["package_version"] != "0.5.0"
        or baseline_members != list(BASELINE_MEMBERS)
        or byte_members != list(BYTE_IDENTICAL_FEREG_MEMBERS)
        or package["help_example_ids"] != list(HELP_EXAMPLE_IDS)
    ):
        raise BehaviorContractError("frozen fereg package contract changed")
    activation = _mapping(package["activation"], "package_contract.activation")
    _keys(
        activation,
        {
            "allowed_added_members", "allowed_changed_shared_files",
            "preserve_baseline_member_order", "preserve_fereg_help_examples",
        },
        "package_contract.activation",
    )
    if (
        activation["allowed_added_members"] != list(ACTIVATION_ADDED_MEMBERS)
        or activation["allowed_changed_shared_files"]
        != list(ACTIVATION_CHANGED_SHARED_FILES)
        or activation["preserve_baseline_member_order"] is not True
        or activation["preserve_fereg_help_examples"] is not True
    ):
        raise BehaviorContractError("Phase 13 additive package allowance changed")
    receipt = _mapping(manifest["receipt_contract"], "receipt_contract")
    if receipt != {
        "observation_type": "fereg_f0_behavior_observation_v1",
        "comparison_type": "fereg_f0_direct_command_contract_v1",
        "complete_sentinel": "FEREG_F0_DIRECT_COMMAND_COMPLETE",
    }:
        raise BehaviorContractError("behavior receipt contract changed")


def _git(source: Path, *arguments: str) -> bytes:
    if platform.system() != "Darwin":
        raise BehaviorContractError(
            "formal direct behavior source capture requires its pinned macOS controller"
        )
    if (
        DARWIN_GIT.is_symlink()
        or not DARWIN_GIT.is_file()
        or sha256_file(DARWIN_GIT) != DARWIN_GIT_SHA256
        or DARWIN_GIT.stat().st_size != DARWIN_GIT_SIZE
    ):
        raise BehaviorContractError("exact macOS Git controller changed")
    try:
        return subprocess.run(
            [str(DARWIN_GIT), "--no-replace-objects", "-C", str(source), *arguments],
            env={
                "HOME": str(source.resolve() / ".fereg-f0-controller-home"),
                "PATH": "/usr/bin:/bin",
                "LANG": "C",
                "LC_ALL": "C",
                "TZ": "UTC",
                "GIT_CONFIG_NOSYSTEM": "1",
                "GIT_CONFIG_GLOBAL": "/dev/null",
                "GIT_NO_REPLACE_OBJECTS": "1",
            },
            check=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", b"").decode("utf-8", errors="replace").strip()
        raise BehaviorContractError(f"git identity command failed: {detail or error}") from error


def source_identity(source: Path, *, require_clean: bool = True) -> dict[str, str]:
    root = source.resolve()
    commit = _git(root, "rev-parse", "HEAD^{commit}").decode("ascii").strip()
    tree = _git(root, "rev-parse", "HEAD^{tree}").decode("ascii").strip()
    if HEX_SHA1.fullmatch(commit) is None or HEX_SHA1.fullmatch(tree) is None:
        raise BehaviorContractError("source commit or tree identity is malformed")
    if require_clean and _git(root, "status", "--porcelain", "--untracked-files=all").strip():
        raise BehaviorContractError("behavior capture requires a clean exact source checkout")
    return {
        "commit": commit,
        "tree": tree,
        "cargo_lock_sha256": sha256_bytes(_git(root, "show", f"{commit}:Cargo.lock")),
        "references_lock_sha256": sha256_bytes(
            _git(root, "show", f"{commit}:references.lock.toml")
        ),
    }


def _platform() -> tuple[str, str]:
    try:
        return PLATFORMS[platform.system()]
    except KeyError as error:
        raise BehaviorContractError(f"unsupported behavior-capture platform: {platform.system()}") from error


def artifact_identity(plugin: Path, source: Mapping[str, Any]) -> dict[str, Any]:
    platform_id, filename = _platform()
    path = plugin.resolve()
    if plugin.is_symlink() or not path.is_file() or path.is_symlink() or path.name != filename:
        raise BehaviorContractError(f"artifact must be an exact regular {filename}")
    embedded = str(source["commit"])[:12]
    if embedded.encode("ascii") not in path.read_bytes():
        raise BehaviorContractError("artifact does not embed the exact source commit prefix")
    return {
        "platform": platform_id,
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
        "embedded_git_commit": embedded,
    }


def _package_document_from_text(
    text: str, file_record: Mapping[str, Any]
) -> dict[str, Any]:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if len(lines) < 3 or re.fullmatch(r"v\s+(\d+)", lines[0]) is None:
        raise BehaviorContractError("fereg.pkg format header is malformed")
    header = re.fullmatch(r"p\s+(\S+)\s+(\S+)\s+(.+)", lines[1])
    if header is None:
        raise BehaviorContractError("fereg.pkg package header is malformed")
    members: list[str] = []
    for line in lines[2:]:
        match = re.fullmatch(r"f\s+([^\s]+)", line)
        if match is None:
            raise BehaviorContractError("fereg.pkg contains a non-file trailing record")
        name = match.group(1)
        if Path(name).name != name or name in members:
            raise BehaviorContractError("fereg.pkg contains an unsafe or duplicate member")
        members.append(name)
    return {
        "format_version": int(re.fullmatch(r"v\s+(\d+)", lines[0]).group(1)),  # type: ignore[union-attr]
        "package_name": header.group(1),
        "package_version": header.group(2),
        "description": header.group(3),
        "members": members,
        "file": dict(file_record),
    }


def _package_document(path: Path) -> dict[str, Any]:
    return _package_document_from_text(
        path.read_text(encoding="utf-8"), _file_record(path)
    )


def _toc_document_from_text(text: str, file_record: Mapping[str, Any]) -> dict[str, Any]:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if len(lines) != 2:
        raise BehaviorContractError("stata.toc must contain exactly one package entry")
    version = re.fullmatch(r"v\s+(\d+)", lines[0])
    package = re.fullmatch(r"p\s+(\S+)\s+(\S+)\s+(.+)", lines[1])
    if version is None or package is None:
        raise BehaviorContractError("stata.toc identity is malformed")
    return {
        "format_version": int(version.group(1)),
        "package_name": package.group(1),
        "package_version": package.group(2),
        "description": package.group(3),
        "file": dict(file_record),
    }


def _toc_document(path: Path) -> dict[str, Any]:
    return _toc_document_from_text(path.read_text(encoding="utf-8"), _file_record(path))


def _help_examples_from_text(
    text: str, expected_ids: Sequence[str]
) -> list[dict[str, str]]:
    pattern = re.compile(
        r"^\{\* example_start - ([a-z0-9_]+)\}\{\.\.\.\}\s*$"
        r"(.*?)"
        r"^\{\* example_end\}\{\.\.\.\}\s*$",
        re.MULTILINE | re.DOTALL,
    )
    records = []
    for match in pattern.finditer(text):
        body = "\n".join(line.rstrip() for line in match.group(2).splitlines()).strip() + "\n"
        records.append({"id": match.group(1), "sha256": sha256_bytes(body.encode("utf-8"))})
    if [item["id"] for item in records] != list(expected_ids):
        raise BehaviorContractError("fereg help example inventory or order changed")
    return records


def _help_examples(path: Path, expected_ids: Sequence[str]) -> list[dict[str, str]]:
    return _help_examples_from_text(path.read_text(encoding="utf-8"), expected_ids)


def _validate_package_phase(
    manifest: Mapping[str, Any], package: Mapping[str, Any], *, role: str, phase: str
) -> None:
    contract = manifest["package_contract"]
    identity = (
        package["format_version"], package["package_name"], package["package_version"]
    )
    if identity != (
        contract["format_version"], contract["package_name"], contract["package_version"]
    ):
        raise BehaviorContractError("package format/name/version differs from F0")
    members = package["members"]
    baseline = contract["baseline_members"]
    if role == "baseline" or phase == "preactivation":
        if members != baseline:
            raise BehaviorContractError("preactivation package member inventory differs from F0")
        return
    if role != "candidate" or phase != "activation":
        raise BehaviorContractError("behavior observation role/phase is invalid")
    extras = contract["activation"]["allowed_added_members"]
    if [name for name in members if name not in extras] != baseline:
        raise BehaviorContractError("activation changed or reordered an F0 package member")
    if [name for name in members if name in extras] != extras:
        raise BehaviorContractError("activation PPML package additions are incomplete or reordered")


def _stage_package(
    source: Path, plugin: Path, destination: Path, manifest: Mapping[str, Any],
    *, role: str, phase: str,
) -> tuple[Path, dict[str, Any]]:
    source_stata = source.resolve() / "stata"
    package = _package_document(source_stata / "fereg.pkg")
    toc = _toc_document(source_stata / "stata.toc")
    _validate_package_phase(manifest, package, role=role, phase=phase)
    contract = manifest["package_contract"]
    expected_identity = (
        contract["format_version"], contract["package_name"], contract["package_version"]
    )
    if (toc["format_version"], toc["package_name"], toc["package_version"]) != expected_identity:
        raise BehaviorContractError("stata.toc format/package/version differs from F0")
    platform_id, plugin_name = _platform()
    if destination.exists():
        raise BehaviorContractError("refusing to overwrite a staged behavior package")
    ado_root = destination / "stata"
    ado_root.mkdir(parents=True)
    staged_names = ["fereg.pkg", "stata.toc"]
    for name in package["members"]:
        if name.endswith(".plugin") and name != plugin_name:
            continue
        source_file = plugin.resolve() if name == plugin_name else source_stata / name
        if source_file.is_symlink() or not source_file.is_file():
            raise BehaviorContractError(f"package member is absent or unsafe: {name}")
        shutil.copyfile(source_file, ado_root / name)
        staged_names.append(name)
    shutil.copyfile(source_stata / "fereg.pkg", ado_root / "fereg.pkg")
    shutil.copyfile(source_stata / "stata.toc", ado_root / "stata.toc")
    expected_staged = sorted(staged_names)
    observed_staged = sorted(path.name for path in ado_root.iterdir())
    if observed_staged != expected_staged:
        raise BehaviorContractError("staged package inventory is not closed")
    for resource in manifest["resources"]:
        target = destination / resource["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / resource["path"], target)
    byte_files = {
        name: _file_record(ado_root / name)
        for name in contract["byte_identical_fereg_members"]
    }
    all_files = {
        name: _file_record(ado_root / name)
        for name in staged_names
    }
    snapshot = {
        "platform": platform_id,
        "package": package,
        "toc": toc,
        "staged_members": staged_names,
        "staged_files": all_files,
        "byte_identical_fereg_files": byte_files,
        "help_examples": _help_examples(
            ado_root / "fereg.sthlp", contract["help_example_ids"]
        ),
    }
    return ado_root.resolve(), snapshot


def _fixture_inventory(manifest: Mapping[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "id": fixture["id"], "path": fixture["path"],
            "sha256": fixture["sha256"], "categories": fixture["categories"],
        }
        for fixture in manifest["fixtures"]
    ]


def _normalize_fixture_output(fixture_id: str, log_text: str) -> dict[str, Any]:
    actions: list[str] = []
    lines: list[str] = []
    active: str | None = None
    for raw in log_text.replace("\r\n", "\n").replace("\r", "\n").splitlines():
        line = raw.strip()
        if line.startswith(OUTPUT_BEGIN):
            action = line.removeprefix(OUTPUT_BEGIN)
            if active is not None or action not in UTILITY_ACTIONS or action in actions:
                raise BehaviorContractError("utility-output marker is nested, unknown, or repeated")
            active = action
            actions.append(action)
            continue
        if line.startswith(OUTPUT_END):
            action = line.removeprefix(OUTPUT_END)
            if active != action:
                raise BehaviorContractError("utility-output end marker is unmatched")
            active = None
            continue
        if active is None or not line.startswith("fereg."):
            continue
        key, separator, value = line.partition("=")
        if (
            not separator
            or re.fullmatch(r"fereg\.[a-z0-9_.]+", key) is None
            or not value
        ):
            raise BehaviorContractError("utility output contains a malformed machine line")
        if key in VOLATILE_OUTPUT_KEYS:
            normalized_value = "<provenance>"
        elif active == "doctor" and key == "fereg.plugin_filename":
            normalized_value = RUNTIME_PLUGIN_PATH_OUTPUT_VALUE
        else:
            normalized_value = value
        lines.append(f"{active}|{key}={normalized_value}")
    if active is not None:
        raise BehaviorContractError("utility-output marker is unterminated")
    expected_actions = list(UTILITY_ACTIONS) if fixture_id == UTILITY_FIXTURE else []
    if actions != expected_actions:
        raise BehaviorContractError(
            f"fixture {fixture_id} utility-output action inventory changed"
        )
    if fixture_id == UTILITY_FIXTURE:
        version_keys = [
            line.split("|", 1)[1].split("=", 1)[0]
            for line in lines
            if line.startswith("version|")
        ]
        if version_keys != list(F0_VERSION_OUTPUT_KEYS):
            raise BehaviorContractError("public version output differs from exact F0")
        if not all(any(line.startswith(f"{action}|fereg.") for line in lines) for action in UTILITY_ACTIONS):
            raise BehaviorContractError("public utility output block is empty")
    record = {"format": NORMALIZED_OUTPUT_FORMAT, "actions": actions, "lines": lines}
    return {
        **record,
        "sha256": sha256_bytes(canonical_json_bytes(record)),
    }


def _validate_normalized_output(value: Any, fixture_id: str, where: str) -> None:
    record = _mapping(value, where)
    _keys(record, {"format", "actions", "lines", "sha256"}, where)
    if record["format"] != NORMALIZED_OUTPUT_FORMAT:
        raise BehaviorContractError(f"{where}.format changed")
    actions, lines = record["actions"], record["lines"]
    if (
        not isinstance(actions, list)
        or not all(isinstance(action, str) for action in actions)
        or not isinstance(lines, list)
        or not all(isinstance(line, str) and line for line in lines)
    ):
        raise BehaviorContractError(f"{where} action/line inventory is malformed")
    expected = {"format": record["format"], "actions": actions, "lines": lines}
    if record["sha256"] != sha256_bytes(canonical_json_bytes(expected)):
        raise BehaviorContractError(f"{where} digest mismatch")
    # Re-rendering a synthetic marked log exercises the same closed parser and
    # prevents a hand-authored record from bypassing action/key validation.
    synthetic: list[str] = []
    by_action = {action: [] for action in actions}
    for line in lines:
        action, separator, machine = line.partition("|")
        if not separator or action not in by_action:
            raise BehaviorContractError(f"{where} line has an unknown action")
        by_action[action].append(machine)
    for action in actions:
        synthetic.extend(
            [f"{OUTPUT_BEGIN}{action}", *by_action[action], f"{OUTPUT_END}{action}"]
        )
    if _normalize_fixture_output(fixture_id, "\n".join(synthetic) + "\n") != record:
        raise BehaviorContractError(f"{where} cannot be reproduced by the normalizer")


def _sanitize_fixture_log(log_text: str, *, ado_root: Path) -> bytes:
    text = log_text.replace("\r\n", "\n").replace("\r", "\n")
    replacements = (
        (str(ado_root.parent.resolve()).replace("\\", "/"), "<FEREG_STAGED_ROOT>"),
        (str(ROOT.resolve()).replace("\\", "/"), "<FEREG_SOURCE_ROOT>"),
    )
    text = text.replace("\\", "/")
    for source, target in replacements:
        text = text.replace(source, target)
    text = re.sub(
        r"(?:[A-Za-z]:)?/[^\s\"']*?/fereg-stata-[A-Za-z0-9_.-]+",
        "<FEREG_STATA_RUN_ROOT>",
        text,
    )
    return text.encode("utf-8")


def _run_fixtures(
    manifest: Mapping[str, Any], *, stata: str, ado_root: Path, plugin: Path,
    log_root: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any], dict[str, tuple[str, bytes]]]:
    stata_path = Path(stata)
    if (
        platform.system() != "Darwin"
        or not stata_path.is_absolute()
        or stata_path != DARWIN_STATA
        or stata_path.is_symlink()
        or not stata_path.is_file()
        or sha256_file(stata_path) != DARWIN_STATA_SHA256
        or stata_path.stat().st_size != DARWIN_STATA_SIZE
    ):
        raise BehaviorContractError("licensed behavior capture requires the exact pinned macOS Stata/MP executable")
    controller_home = log_root.parent / "controller-home"
    controller_tmp = log_root.parent / "controller-tmp"
    controller_home.mkdir(parents=True, exist_ok=False)
    controller_tmp.mkdir(parents=True, exist_ok=False)
    stata_environment = {
        "HOME": str(controller_home.resolve()),
        "TMPDIR": str(controller_tmp.resolve()),
        "PATH": "/usr/bin:/bin",
        "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
    }
    outcomes = []
    environments = []
    bundle_files: dict[str, tuple[str, bytes]] = {}
    plugin_sha = sha256_file(plugin)
    for fixture in manifest["fixtures"]:
        path = ROOT / fixture["path"]
        if sha256_file(path) != fixture["sha256"]:
            raise BehaviorContractError(f"fixture changed before execution: {fixture['id']}")
        result = stata_runner.run_fixture(
            stata, path, ROOT, plugin=plugin, plugin_sha256=plugin_sha,
            log_dir=log_root, require_status=True, ado_root=ado_root,
            environment=stata_environment,
        )
        if result.error is not None:
            raise BehaviorContractError(f"licensed fixture {fixture['id']} failed: {result.error}")
        environment = {
            "stata_version": result.metadata.get("stata_version"),
            "stata_edition": result.metadata.get("stata_edition"),
            "stata_os": result.metadata.get("stata_os"),
            "stata_machine_type": result.metadata.get("stata_machine_type"),
            "stata_processors": stata_runner.optional_int(result.metadata.get("stata_processors")),
        }
        if any(value in (None, "") for value in environment.values()):
            raise BehaviorContractError(f"fixture {fixture['id']} omitted Stata environment")
        environments.append(environment)
        normalized = _normalize_fixture_output(fixture["id"], result.log_text)
        log_member = f"fixture-logs/{fixture['id']}.log"
        status_member = f"fixture-status/{fixture['id']}.status"
        rc_member = f"fixture-rc/{fixture['id']}.json"
        stream_member = f"fixture-streams/{fixture['id']}.stdout-stderr"
        wrapper_member = f"fixture-wrappers/{fixture['id']}.do"
        invocation_member = f"fixture-invocations/{fixture['id']}.json"
        stable_name = fixture["path"].replace("/", "__")
        status_path = log_root / f"{stable_name}.status"
        if not status_path.is_file():
            raise BehaviorContractError(f"fixture {fixture['id']} omitted retained status")
        bundle_files[log_member] = (
            "fixture_log",
            _sanitize_fixture_log(result.log_text, ado_root=ado_root),
        )
        bundle_files[status_member] = ("fixture_status", status_path.read_bytes())
        bundle_files[rc_member] = (
            "fixture_process_status",
            canonical_json_bytes(
                {
                    "schema_version": 1,
                    "fixture_id": fixture["id"],
                    "process_rc": result.process_returncode,
                    "stata_rc": result.stata_rc,
                }
            ) + b"\n",
        )
        bundle_files[stream_member] = (
            "fixture_stdout_stderr",
            result.process_output.replace("\r\n", "\n").replace("\r", "\n").encode("utf-8"),
        )
        wrapper_path = log_root / f"{stable_name}.wrapper.do"
        invocation_path = log_root / f"{stable_name}.invocation.json"
        if not wrapper_path.is_file() or not invocation_path.is_file():
            raise BehaviorContractError(f"fixture {fixture['id']} omitted retained action proof")
        bundle_files[wrapper_member] = ("fixture_wrapper", wrapper_path.read_bytes())
        invocation = json.loads(invocation_path.read_text(encoding="ascii"))
        invocation["controller_source_root"] = str(ROOT.resolve())
        bundle_files[invocation_member] = (
            "fixture_invocation", canonical_json_bytes(invocation) + b"\n"
        )
        source_member = f"fixture-sources/{fixture['path']}"
        bundle_files[source_member] = ("fixture_source", path.read_bytes())
        outcomes.append(
            {
                "id": fixture["id"],
                "process_rc": result.process_returncode,
                "stata_rc": result.stata_rc,
                "terminal_sentinel": stata_runner.expected_sentinel(path),
                "status": "pass",
                "log_member": log_member,
                "status_member": status_member,
                "rc_member": rc_member,
                "stream_member": stream_member,
                "wrapper_member": wrapper_member,
                "invocation_member": invocation_member,
                "normalized_output": normalized,
            }
        )
    if any(item != environments[0] for item in environments[1:]):
        raise BehaviorContractError("licensed fixtures used inconsistent Stata environments")
    return outcomes, environments[0], bundle_files


BUNDLE_FORMAT = "fereg-f0-behavior-evidence-v1"
BUNDLE_MANIFEST = "bundle-manifest.json"
BUNDLE_MEMBER_FIELDS = {"path", "role", "sha256", "size_bytes"}


def _safe_bundle_member(name: str) -> None:
    pure = Path(name)
    if (
        not name
        or name.startswith("/")
        or "\\" in name
        or re.match(r"^[A-Za-z]:", name) is not None
        or any(ord(character) < 32 for character in name)
        or pure.is_absolute()
        or any(part in {"", ".", ".."} for part in pure.parts)
        or pure.as_posix() != name
    ):
        raise BehaviorContractError(f"unsafe behavior-bundle member path: {name!r}")


def _bundle_record(path: str, role: str, content: bytes) -> dict[str, Any]:
    _safe_bundle_member(path)
    return {
        "path": path,
        "role": role,
        "sha256": sha256_bytes(content),
        "size_bytes": len(content),
    }


def _bundle_inputs(
    manifest: Mapping[str, Any],
    ado_root: Path,
    fixture_files: Mapping[str, tuple[str, bytes]],
) -> tuple[dict[str, bytes], list[dict[str, Any]]]:
    contents: dict[str, bytes] = {}
    roles: dict[str, str] = {}

    def add(name: str, role: str, content: bytes) -> None:
        _safe_bundle_member(name)
        if name in contents:
            raise BehaviorContractError(f"duplicate behavior-bundle member: {name}")
        contents[name] = content
        roles[name] = role

    for name, (role, content) in fixture_files.items():
        add(name, role, content)
    for path in sorted(ado_root.iterdir(), key=lambda item: item.name):
        if path.is_symlink() or not path.is_file():
            raise BehaviorContractError("staged package contains a non-regular entry")
        add(f"package/stata/{path.name}", "staged_package_member", path.read_bytes())
    for resource in manifest["resources"]:
        path = ado_root.parent / resource["path"]
        if path.is_symlink() or not path.is_file():
            raise BehaviorContractError("staged behavior resource is absent or unsafe")
        add(f"resources/{resource['path']}", "staged_resource", path.read_bytes())
    records = [
        _bundle_record(name, roles[name], contents[name]) for name in sorted(contents)
    ]
    bundle_manifest = {
        "schema_version": 1,
        "bundle_type": BUNDLE_FORMAT,
        "members": records,
    }
    contents[BUNDLE_MANIFEST] = canonical_json_bytes(bundle_manifest) + b"\n"
    return contents, records


def _canonical_bundle_bytes(contents: Mapping[str, bytes]) -> bytes:
    """Render the one canonical USTAR representation accepted by the verifier."""
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for name in sorted(contents):
            content = contents[name]
            info = tarfile.TarInfo(name)
            info.size = len(content)
            info.mode = 0o444
            info.mtime = 0
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            archive.addfile(info, io.BytesIO(content))
    return output.getvalue()


def _write_bundle(path: Path, contents: Mapping[str, bytes]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise BehaviorContractError(f"refusing to overwrite immutable bundle: {path}") from error
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(_canonical_bundle_bytes(contents))
            output.flush()
            os.fsync(output.fileno())
    except Exception:
        # The O_EXCL target may be incomplete.  Leave no apparently valid bundle.
        path.unlink(missing_ok=True)
        raise


def _evidence_bundle_record(path: Path, members: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    return {
        "format": BUNDLE_FORMAT,
        "filename": path.name,
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
        "members": [dict(item) for item in members],
    }


def _validate_evidence_bundle_record(value: Any, where: str) -> None:
    record = _mapping(value, where)
    _keys(record, {"format", "filename", "sha256", "size_bytes", "members"}, where)
    if (
        record["format"] != BUNDLE_FORMAT
        or not isinstance(record["filename"], str)
        or Path(record["filename"]).name != record["filename"]
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,126}\.tar", record["filename"])
        is None
    ):
        raise BehaviorContractError(f"{where} identity is invalid")
    _sha256(record["sha256"], f"{where}.sha256")
    if (
        isinstance(record["size_bytes"], bool)
        or not isinstance(record["size_bytes"], int)
        or record["size_bytes"] < 10240
        or record["size_bytes"] % 10240 != 0
    ):
        raise BehaviorContractError(f"{where}.size_bytes is not an uncompressed tar size")
    members = record["members"]
    if not isinstance(members, list) or not members:
        raise BehaviorContractError(f"{where} has no closed member inventory")
    names: list[str] = []
    for index, value in enumerate(members):
        item = _mapping(value, f"{where}.members[{index}]")
        _keys(item, BUNDLE_MEMBER_FIELDS, f"{where}.members[{index}]")
        _safe_bundle_member(item["path"])
        if not isinstance(item["role"], str) or not item["role"]:
            raise BehaviorContractError(f"{where}.members[{index}].role is empty")
        _sha256(item["sha256"], f"{where}.members[{index}].sha256")
        allows_empty_process_stream = (
            item["role"] == "fixture_stdout_stderr"
            and re.fullmatch(
                r"fixture-streams/[a-z][a-z0-9_]{1,63}\.stdout-stderr",
                item["path"],
            )
            is not None
        )
        if (
            isinstance(item["size_bytes"], bool)
            or not isinstance(item["size_bytes"], int)
            or item["size_bytes"] < 0
            or (item["size_bytes"] == 0 and not allows_empty_process_stream)
        ):
            raise BehaviorContractError(f"{where}.members[{index}] size is invalid")
        names.append(item["path"])
    if names != sorted(names) or len(names) != len(set(names)) or BUNDLE_MANIFEST in names:
        raise BehaviorContractError(f"{where} member inventory is unordered or duplicated")


def _read_verified_bundle(
    path: Path, expected: Mapping[str, Any]
) -> dict[str, bytes]:
    _validate_evidence_bundle_record(expected, "observation.evidence_bundle")
    if (
        path.is_symlink()
        or not path.is_file()
        or path.name != expected["filename"]
        or path.stat().st_size != expected["size_bytes"]
        or sha256_file(path) != expected["sha256"]
    ):
        raise BehaviorContractError("behavior evidence bundle identity mismatch")
    contents: dict[str, bytes] = {}
    try:
        with tarfile.open(path, mode="r:") as archive:
            members = archive.getmembers()
            names = [member.name for member in members]
            if names != sorted(names) or len(names) != len(set(names)):
                raise BehaviorContractError("behavior bundle member order/inventory is noncanonical")
            for member in members:
                _safe_bundle_member(member.name)
                if (
                    not member.isreg()
                    or member.mode != 0o444
                    or member.mtime != 0
                    or member.uid != 0
                    or member.gid != 0
                    or member.uname
                    or member.gname
                    or member.pax_headers
                ):
                    raise BehaviorContractError("behavior bundle has noncanonical tar metadata")
                extracted = archive.extractfile(member)
                if extracted is None:
                    raise BehaviorContractError("behavior bundle member cannot be read")
                contents[member.name] = extracted.read()
    except (OSError, tarfile.TarError) as error:
        raise BehaviorContractError(f"cannot read behavior evidence bundle: {error}") from error
    records = expected["members"]
    expected_names = [BUNDLE_MANIFEST, *[item["path"] for item in records]]
    if sorted(expected_names) != sorted(contents):
        raise BehaviorContractError("behavior bundle differs from its closed member inventory")
    for item in records:
        content = contents[item["path"]]
        if len(content) != item["size_bytes"] or sha256_bytes(content) != item["sha256"]:
            raise BehaviorContractError(f"behavior bundle member identity mismatch: {item['path']}")
    expected_manifest = canonical_json_bytes(
        {"schema_version": 1, "bundle_type": BUNDLE_FORMAT, "members": records}
    ) + b"\n"
    if contents[BUNDLE_MANIFEST] != expected_manifest:
        raise BehaviorContractError("behavior bundle internal manifest mismatch")
    try:
        raw_bundle = path.read_bytes()
    except OSError as error:
        raise BehaviorContractError(f"cannot reread behavior evidence bundle: {error}") from error
    if raw_bundle != _canonical_bundle_bytes(contents):
        raise BehaviorContractError("behavior bundle byte representation is noncanonical")
    return contents


def _load_f0_identity(
    path: Path, manifest: Mapping[str, Any], artifact: Mapping[str, Any]
) -> dict[str, Any]:
    document = _read_json(path, "F0 regression manifest")
    if (
        document.get("manifest_id") != "fereg_f0_regression_v1"
        or document.get("status") != "candidate_unsealed"
    ):
        raise BehaviorContractError("F0 regression manifest identity is invalid")
    baseline = _mapping(document.get("baseline_candidate"), "F0 baseline_candidate")
    if baseline.get("source") != manifest["baseline_source"]:
        raise BehaviorContractError("F0 behavior and regression manifests disagree on source")
    artifacts = baseline.get("artifacts")
    if not isinstance(artifacts, list):
        raise BehaviorContractError("F0 regression manifest artifact inventory is malformed")
    expected = next(
        (item for item in artifacts if isinstance(item, Mapping) and item.get("platform") == artifact["platform"]),
        None,
    )
    pending = (
        isinstance(expected, Mapping)
        and expected.get("status") == "pending"
        and all(
            expected.get(field) is None
            for field in ("sha256", "size_bytes", "embedded_git_commit")
        )
    )
    exact_recorded = (
        isinstance(expected, Mapping)
        and expected.get("status") in {"captured_unsealed", "qualified"}
        and all(
            expected.get(field) == artifact[field]
            for field in (
                "platform", "sha256", "size_bytes", "embedded_git_commit"
            )
        )
    )
    if not (pending or exact_recorded):
        raise BehaviorContractError("F0 artifact differs from the regression manifest")
    return {
        "id": document["manifest_id"],
        "sha256": sha256_file(path),
        "status": document.get("status"),
        "source": dict(baseline["source"]),
        "artifact": dict(artifact),
    }


def _require_isolated_controller() -> None:
    if not (
        sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode
    ):
        raise BehaviorContractError(
            "behavior evidence APIs require an exact Python launch with -I -B -S"
        )
    forbidden = {
        "PYTHONPATH", "PYTHONHOME", "PYTHONSTARTUP", "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH",
    }
    present = sorted(name for name in forbidden if os.environ.get(name))
    if present:
        raise BehaviorContractError(
            "behavior evidence APIs reject ambient loader/startup variables: "
            + ",".join(present)
        )


def capture_observation(args: argparse.Namespace) -> dict[str, Any]:
    _require_isolated_controller()
    manifest = load_manifest(args.manifest)
    manifest_sha = sha256_file(args.manifest)
    role = args.role
    phase = args.phase
    if role == "baseline" and phase != "baseline":
        raise BehaviorContractError("baseline observation phase must be baseline")
    if role == "candidate" and phase not in {"preactivation", "activation"}:
        raise BehaviorContractError("candidate phase must be preactivation or activation")
    source = source_identity(args.source)
    controller_source = source_identity(ROOT)
    controller_archive = _git(ROOT, "archive", "--format=tar", controller_source["commit"])
    controller_source = {
        **controller_source,
        "archive_sha256": sha256_bytes(controller_archive),
        "archive_size_bytes": len(controller_archive),
        "lexical_root": str(ROOT.resolve()),
    }
    controller_programs = {
        relative: {
            "sha256": sha256_file(ROOT / relative),
            "size_bytes": (ROOT / relative).stat().st_size,
        }
        for relative in ("scripts/fereg_f0_behavior.py", "scripts/run_stata_tests.py")
    }
    if role == "baseline" and source != manifest["baseline_source"]:
        raise BehaviorContractError("baseline checkout is not exact F0")
    if role == "candidate" and source == manifest["baseline_source"]:
        raise BehaviorContractError("candidate behavior capture is vacuously equal to F0")
    artifact = artifact_identity(args.plugin, source)
    f0_identity = (
        _load_f0_identity(args.f0_manifest, manifest, artifact)
        if role == "baseline"
        else None
    )
    if args.run_root.exists():
        raise BehaviorContractError("run root already exists")
    args.run_root.mkdir(parents=True)
    ado_root, package = _stage_package(
        args.source, args.plugin, args.run_root / "bundle", manifest,
        role=role, phase=phase,
    )
    staged_plugin = ado_root / _platform()[1]
    fixtures, environment, fixture_files = _run_fixtures(
        manifest, stata=args.stata, ado_root=ado_root, plugin=staged_plugin,
        log_root=args.run_root / "logs",
    )
    if artifact_identity(staged_plugin, source) != artifact:
        raise BehaviorContractError("staged artifact changed during behavior capture")
    bundle_contents, bundle_members = _bundle_inputs(
        manifest, ado_root, fixture_files
    )
    _write_bundle(args.bundle_output, bundle_contents)
    evidence_bundle = _evidence_bundle_record(args.bundle_output, bundle_members)
    payload = {
        "schema_version": 1,
        "receipt_type": manifest["receipt_contract"]["observation_type"],
        "status": "pass",
        "role": role,
        "phase": phase,
        "contract": {"id": manifest["manifest_id"], "sha256": manifest_sha},
        "f0_manifest": f0_identity,
        "source": source,
        "controller_source": controller_source,
        "controller_programs": controller_programs,
        "artifact": artifact,
        "stata_controller": {
            "path": str(DARWIN_STATA),
            "sha256": DARWIN_STATA_SHA256,
            "size_bytes": DARWIN_STATA_SIZE,
            "argv_contract": [str(DARWIN_STATA), "-q", "-b", "do", "<retained-wrapper.do>"],
            "environment_keys": ["HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ"],
            "environment_contract": "closed-private-home-tmp-v1",
            "stdout_stderr": "merged-retained-process-stream-v1",
        },
        "stata_environment": environment,
        "package": package,
        "fixture_inventory": _fixture_inventory(manifest),
        "fixture_outcomes": fixtures,
        "evidence_bundle": evidence_bundle,
        "complete_sentinel": manifest["receipt_contract"]["complete_sentinel"],
    }
    _validate_observation(manifest, manifest_sha, payload)
    document = seal_payload(payload)
    _write_new(args.output, document)
    verify_observation(
        args.output,
        args.bundle_output,
        args.manifest,
        expected_role=role,
        expected_phase=phase,
    )
    _write_identity(args.identity_output, args.output, document)
    return document


OBSERVATION_FIELDS = {
    "schema_version", "receipt_type", "status", "role", "phase", "contract",
    "f0_manifest", "source", "controller_source", "controller_programs", "artifact", "stata_controller", "stata_environment", "package",
    "fixture_inventory", "fixture_outcomes", "evidence_bundle", "complete_sentinel",
}


def _read_sealed(path: Path) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    document = _read_json(path, "sealed behavior receipt")
    _keys(document, {"payload", "seal"}, "sealed behavior receipt")
    payload = _mapping(document["payload"], "behavior receipt payload")
    expected = seal_payload(payload)["seal"]
    if document["seal"] != expected:
        raise BehaviorContractError("behavior receipt payload seal mismatch")
    return payload, document


def _validate_observation(
    manifest: Mapping[str, Any], manifest_sha: str, payload: Mapping[str, Any]
) -> None:
    _keys(payload, OBSERVATION_FIELDS, "behavior observation")
    receipt = manifest["receipt_contract"]
    if (
        payload["schema_version"] != 1
        or payload["receipt_type"] != receipt["observation_type"]
        or payload["status"] != "pass"
        or payload["complete_sentinel"] != receipt["complete_sentinel"]
        or payload["contract"] != {"id": manifest["manifest_id"], "sha256": manifest_sha}
    ):
        raise BehaviorContractError("behavior observation identity/status is invalid")
    source = _mapping(payload["source"], "observation.source")
    _validate_source_record(source, "observation.source")
    artifact = _mapping(payload["artifact"], "observation.artifact")
    _validate_artifact_record(artifact, source, "observation.artifact")
    controller_source = _mapping(payload["controller_source"], "observation.controller_source")
    _keys(
        controller_source,
        {"commit", "tree", "cargo_lock_sha256", "references_lock_sha256", "archive_sha256", "archive_size_bytes", "lexical_root"},
        "observation.controller_source",
    )
    for name in ("commit", "tree"):
        if HEX_SHA1.fullmatch(str(controller_source[name])) is None:
            raise BehaviorContractError("behavior controller source identity is malformed")
    for name in ("cargo_lock_sha256", "references_lock_sha256", "archive_sha256"):
        _sha256(controller_source[name], f"observation.controller_source.{name}")
    lexical_root = Path(str(controller_source["lexical_root"]))
    if (
        not lexical_root.is_absolute()
        or lexical_root.as_posix() != controller_source["lexical_root"]
        or any(part in {".", ".."} for part in lexical_root.parts)
        or not isinstance(controller_source["archive_size_bytes"], int)
        or controller_source["archive_size_bytes"] < 1
    ):
        raise BehaviorContractError("behavior controller lexical/archive identity is malformed")
    programs = _mapping(payload["controller_programs"], "observation.controller_programs")
    expected_programs = {}
    for relative in ("scripts/fereg_f0_behavior.py", "scripts/run_stata_tests.py"):
        expected_programs[relative] = {
            "sha256": sha256_file(ROOT / relative),
            "size_bytes": (ROOT / relative).stat().st_size,
        }
    if programs != expected_programs:
        raise BehaviorContractError("behavior controller program bytes changed")
    expected_stata_controller = {
        "path": str(DARWIN_STATA), "sha256": DARWIN_STATA_SHA256,
        "size_bytes": DARWIN_STATA_SIZE,
        "argv_contract": [str(DARWIN_STATA), "-q", "-b", "do", "<retained-wrapper.do>"],
        "environment_keys": ["HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ"],
        "environment_contract": "closed-private-home-tmp-v1",
        "stdout_stderr": "merged-retained-process-stream-v1",
    }
    if payload["stata_controller"] != expected_stata_controller:
        raise BehaviorContractError("behavior observation Stata controller changed")
    role, phase = payload["role"], payload["phase"]
    if role == "baseline":
        if phase != "baseline" or source != manifest["baseline_source"]:
            raise BehaviorContractError("baseline observation is not exact F0")
        f0 = _mapping(payload["f0_manifest"], "observation.f0_manifest")
        _keys(
            f0,
            {"id", "sha256", "status", "source", "artifact"},
            "observation.f0_manifest",
        )
        if (
            f0["id"] != "fereg_f0_regression_v1"
            or f0["status"] != "candidate_unsealed"
            or f0["source"] != source
            or f0["artifact"] != artifact
        ):
            raise BehaviorContractError("baseline observation is not bound to pre-freeze F0 identity")
        _sha256(f0["sha256"], "observation.f0_manifest.sha256")
    elif role == "candidate":
        if (
            phase not in {"preactivation", "activation"}
            or payload["f0_manifest"] is not None
            or source == manifest["baseline_source"]
        ):
            raise BehaviorContractError("candidate observation role/phase is invalid")
    else:
        raise BehaviorContractError("behavior observation role is invalid")
    environment = _mapping(payload["stata_environment"], "observation.stata_environment")
    _validate_stata_environment(environment, "observation.stata_environment")
    package = _mapping(payload["package"], "observation.package")
    _validate_package_snapshot(manifest, package, role=role, phase=phase)
    if package["platform"] != artifact["platform"]:
        raise BehaviorContractError("package and artifact platform identities differ")
    expected_fixture_inventory = _fixture_inventory(manifest)
    if payload["fixture_inventory"] != expected_fixture_inventory:
        raise BehaviorContractError("behavior observation fixture inventory changed")
    _validate_evidence_bundle_record(
        payload["evidence_bundle"], "observation.evidence_bundle"
    )
    outcomes = payload["fixture_outcomes"]
    if not isinstance(outcomes, list) or [item.get("id") for item in outcomes if isinstance(item, Mapping)] != list(FIXTURE_IDS):
        raise BehaviorContractError("behavior observation fixture outcomes are incomplete")
    for item, fixture in zip(outcomes, manifest["fixtures"]):
        outcome = _mapping(item, f"fixture outcome {fixture['id']}")
        _keys(
            outcome,
            {
                "id", "process_rc", "stata_rc", "terminal_sentinel", "status",
                "log_member", "status_member", "rc_member", "stream_member", "wrapper_member", "invocation_member", "normalized_output",
            },
            f"fixture outcome {fixture['id']}",
        )
        expected_scalar = {
            "id": fixture["id"], "process_rc": 0, "stata_rc": 0,
            "terminal_sentinel": f"END_FIXTURE|{fixture['id']}|rc=0", "status": "pass",
            "log_member": f"fixture-logs/{fixture['id']}.log",
            "status_member": f"fixture-status/{fixture['id']}.status",
            "rc_member": f"fixture-rc/{fixture['id']}.json",
            "stream_member": f"fixture-streams/{fixture['id']}.stdout-stderr",
            "wrapper_member": f"fixture-wrappers/{fixture['id']}.do",
            "invocation_member": f"fixture-invocations/{fixture['id']}.json",
        }
        if any(outcome[field] != value for field, value in expected_scalar.items()):
            raise BehaviorContractError(f"behavior fixture outcome did not pass exactly: {fixture['id']}")
        _validate_normalized_output(
            outcome["normalized_output"],
            fixture["id"],
            f"fixture outcome {fixture['id']}.normalized_output",
        )


def _validate_source_record(value: Mapping[str, Any], where: str) -> None:
    source = _mapping(value, where)
    _keys(source, set(F0_SOURCE), where)
    for field in ("commit", "tree"):
        if HEX_SHA1.fullmatch(str(source[field])) is None:
            raise BehaviorContractError(f"{where}.{field} is malformed")
    for field in ("cargo_lock_sha256", "references_lock_sha256"):
        _sha256(source[field], f"{where}.{field}")


def _validate_artifact_record(
    value: Mapping[str, Any], source: Mapping[str, Any], where: str
) -> None:
    artifact = _mapping(value, where)
    _keys(artifact, {"platform", "sha256", "size_bytes", "embedded_git_commit"}, where)
    _sha256(artifact["sha256"], f"{where}.sha256")
    if (
        artifact["platform"] not in {item[0] for item in PLATFORMS.values()}
        or isinstance(artifact["size_bytes"], bool)
        or not isinstance(artifact["size_bytes"], int)
        or artifact["size_bytes"] < 1
        or artifact["embedded_git_commit"] != source["commit"][:12]
    ):
        raise BehaviorContractError(f"{where} identity is invalid")


def _verify_plugin_source_bytes(content: bytes, source: Mapping[str, Any]) -> None:
    """Bind retained native bytes to the source identity claimed by the receipt."""
    embedded = str(source["commit"])[:12].encode("ascii")
    if embedded not in content:
        raise BehaviorContractError(
            "bundled native artifact does not embed the claimed source commit prefix"
        )


def _validate_stata_environment(value: Mapping[str, Any], where: str) -> None:
    environment = _mapping(value, where)
    _keys(
        environment,
        {"stata_version", "stata_edition", "stata_os", "stata_machine_type", "stata_processors"},
        where,
    )
    for field in ("stata_version", "stata_edition", "stata_os", "stata_machine_type"):
        if not isinstance(environment[field], str) or not environment[field]:
            raise BehaviorContractError(f"{where}.{field} must be a non-empty string")
    processors = environment["stata_processors"]
    if isinstance(processors, bool) or not isinstance(processors, int) or processors < 1:
        raise BehaviorContractError(f"{where}.stata_processors must be a positive integer")


def _validate_file_record_value(value: Any, where: str) -> None:
    record = _mapping(value, where)
    _keys(record, {"sha256", "size_bytes"}, where)
    _sha256(record["sha256"], f"{where}.sha256")
    if (
        isinstance(record["size_bytes"], bool)
        or not isinstance(record["size_bytes"], int)
        or record["size_bytes"] < 1
    ):
        raise BehaviorContractError(f"{where}.size_bytes must be a positive integer")


def _validate_package_snapshot(
    manifest: Mapping[str, Any], value: Mapping[str, Any], *, role: str, phase: str
) -> None:
    _keys(
        value,
        {
            "platform", "package", "toc", "staged_members", "staged_files",
            "byte_identical_fereg_files", "help_examples",
        },
        "observation.package",
    )
    if value["platform"] not in {item[0] for item in PLATFORMS.values()}:
        raise BehaviorContractError("package snapshot platform is invalid")
    package = _mapping(value["package"], "observation.package.package")
    _keys(
        package,
        {
            "format_version", "package_name", "package_version", "description",
            "members", "file",
        },
        "observation.package.package",
    )
    _validate_file_record_value(package["file"], "observation.package.package.file")
    if not isinstance(package["description"], str) or not package["description"]:
        raise BehaviorContractError("package description is empty")
    _validate_package_phase(manifest, package, role=role, phase=phase)
    toc = _mapping(value["toc"], "observation.package.toc")
    _keys(
        toc,
        {
            "format_version", "package_name", "package_version", "description", "file",
        },
        "observation.package.toc",
    )
    _validate_file_record_value(toc["file"], "observation.package.toc.file")
    contract = manifest["package_contract"]
    if (
        toc["format_version"], toc["package_name"], toc["package_version"]
    ) != (
        contract["format_version"], contract["package_name"], contract["package_version"]
    ) or not isinstance(toc["description"], str) or not toc["description"]:
        raise BehaviorContractError("package toc identity is invalid")
    platform_plugin = next(
        filename for platform_id, filename in PLATFORMS.values()
        if platform_id == value["platform"]
    )
    expected_staged = [
        "fereg.pkg", "stata.toc",
        *[name for name in package["members"] if not name.endswith(".plugin") or name == platform_plugin],
    ]
    if value["staged_members"] != expected_staged:
        raise BehaviorContractError("package staged member inventory is incomplete")
    staged_files = _mapping(value["staged_files"], "observation.package.staged_files")
    if set(staged_files) != set(expected_staged) or len(staged_files) != len(expected_staged):
        raise BehaviorContractError("package staged file record inventory changed")
    for name, record in staged_files.items():
        _validate_file_record_value(record, f"observation.package.staged_files.{name}")
    if package["file"] != staged_files["fereg.pkg"]:
        raise BehaviorContractError("parsed fereg.pkg record is not the staged fereg.pkg")
    if toc["file"] != staged_files["stata.toc"]:
        raise BehaviorContractError("parsed stata.toc record is not the staged stata.toc")
    byte_files = _mapping(
        value["byte_identical_fereg_files"],
        "observation.package.byte_identical_fereg_files",
    )
    if set(byte_files) != set(contract["byte_identical_fereg_members"]):
        raise BehaviorContractError("byte-identical fereg member inventory changed")
    for name, record in byte_files.items():
        _validate_file_record_value(
            record, f"observation.package.byte_identical_fereg_files.{name}"
        )
        if record != staged_files[name]:
            raise BehaviorContractError(f"byte-identical member record is not staged: {name}")
    examples = value["help_examples"]
    if (
        not isinstance(examples, list)
        or [item.get("id") for item in examples if isinstance(item, Mapping)]
        != contract["help_example_ids"]
    ):
        raise BehaviorContractError("help example digest inventory changed")
    for index, record in enumerate(examples):
        item = _mapping(record, f"help_examples[{index}]")
        _keys(item, {"id", "sha256"}, f"help_examples[{index}]")
        _sha256(item["sha256"], f"help_examples[{index}].sha256")


def _package_comparison(
    manifest: Mapping[str, Any], baseline: Mapping[str, Any], candidate: Mapping[str, Any]
) -> dict[str, Any]:
    base = baseline["package"]
    cand = candidate["package"]
    if base["byte_identical_fereg_files"] != cand["byte_identical_fereg_files"]:
        raise BehaviorContractError("candidate changed a byte-frozen fereg control-plane member")
    if base["help_examples"] != cand["help_examples"]:
        raise BehaviorContractError("candidate changed a frozen fereg help example block")
    base_toc, cand_toc = base["toc"], cand["toc"]
    for field in ("format_version", "package_name", "package_version"):
        if base_toc[field] != cand_toc[field]:
            raise BehaviorContractError("candidate changed the Stata package identity")
    phase = candidate["phase"]
    if phase == "preactivation" and (
        base["package"]["description"] != cand["package"]["description"]
        or base_toc["description"] != cand_toc["description"]
    ):
        raise BehaviorContractError("preactivation changed frozen package description metadata")
    _validate_package_phase(
        manifest, cand["package"], role="candidate", phase=phase
    )
    expected = _expected_package_comparison(
        manifest, phase=phase, platform_id=base["platform"]
    )
    for name in expected["unchanged_baseline_files"]:
        if base["staged_files"][name] != cand["staged_files"][name]:
            raise BehaviorContractError(
                f"candidate changed a baseline file outside the authorized delta: {name}"
            )
    return expected


def _expected_package_comparison(
    manifest: Mapping[str, Any], *, phase: str, platform_id: str
) -> dict[str, Any]:
    if phase not in {"preactivation", "activation"}:
        raise BehaviorContractError("comparison phase must be preactivation or activation")
    try:
        plugin = next(
            filename for known_platform, filename in PLATFORMS.values()
            if known_platform == platform_id
        )
    except StopIteration as error:
        raise BehaviorContractError("comparison platform is not a registered native platform") from error
    contract = manifest["package_contract"]
    authorized = (
        [] if phase == "preactivation"
        else list(contract["activation"]["allowed_changed_shared_files"])
    )
    baseline_staged = [
        "fereg.pkg", "stata.toc",
        *[
            name for name in contract["baseline_members"]
            if not name.endswith(".plugin") or name == plugin
        ],
    ]
    unchanged = [
        name for name in baseline_staged
        if name != plugin and name not in authorized
    ]
    return {
        "byte_identical_fereg_members": manifest["package_contract"]["byte_identical_fereg_members"],
        "fereg_help_examples_equal": True,
        "package_member_policy": (
            "exact_f0" if phase == "preactivation" else "phase13_additive_ppml_only"
        ),
        "toc_identity_equal": True,
        "description_policy": (
            "exact_f0" if phase == "preactivation" else "phase13_authorized_shared_metadata"
        ),
        "unchanged_baseline_files": unchanged,
        "authorized_changed_shared_files": authorized,
    }


def evaluate_receipts(
    manifest: Mapping[str, Any], manifest_sha: str,
    baseline: Mapping[str, Any], candidate: Mapping[str, Any],
) -> dict[str, Any]:
    _validate_observation(manifest, manifest_sha, baseline)
    _validate_observation(manifest, manifest_sha, candidate)
    if baseline["role"] != "baseline" or candidate["role"] != "candidate":
        raise BehaviorContractError("comparison requires baseline then candidate observations")
    if baseline["artifact"]["platform"] != candidate["artifact"]["platform"]:
        raise BehaviorContractError("direct behavior comparison must use the same native platform")
    if baseline["stata_environment"] != candidate["stata_environment"]:
        raise BehaviorContractError("direct behavior comparison must use the same Stata environment")
    baseline_controller = dict(baseline["controller_source"])
    candidate_controller = dict(candidate["controller_source"])
    baseline_controller.pop("lexical_root")
    candidate_controller.pop("lexical_root")
    if (
        baseline_controller != candidate_controller
        or baseline["controller_programs"] != candidate["controller_programs"]
    ):
        raise BehaviorContractError("direct behavior comparison mixed controller generations")
    if baseline["fixture_outcomes"] != candidate["fixture_outcomes"]:
        raise BehaviorContractError("candidate normalized fixture outcomes differ from F0")
    package = _package_comparison(manifest, baseline, candidate)
    return {
        "fixtures_equal": True,
        "fixture_count": len(FIXTURE_IDS),
        "package": package,
    }


def _status_from_bytes(content: bytes, where: str) -> dict[str, str]:
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BehaviorContractError(f"{where} is not UTF-8") from error
    values: dict[str, str] = {}
    for raw in text.splitlines():
        if not raw:
            continue
        key, separator, value = raw.partition("=")
        if not separator or not key or key in values:
            raise BehaviorContractError(f"{where} contains a malformed/duplicate line")
        values[key] = value
    expected = {
        "schema_version", "completed", "stata_rc", "stata_version",
        "stata_edition", "stata_os", "stata_machine_type", "stata_processors",
    }
    if set(values) != expected or values["schema_version"] != "1" or values["completed"] != "1":
        raise BehaviorContractError(f"{where} field inventory/status is invalid")
    return values


def _verify_bundle_payload(
    manifest: Mapping[str, Any], payload: Mapping[str, Any], bundle: Path
) -> None:
    contents = _read_verified_bundle(bundle, payload["evidence_bundle"])
    expected_roles: dict[str, str] = {}
    for fixture in manifest["fixtures"]:
        fixture_id = fixture["id"]
        expected_roles.update(
            {
                f"fixture-sources/{fixture['path']}": "fixture_source",
                f"fixture-logs/{fixture_id}.log": "fixture_log",
                f"fixture-status/{fixture_id}.status": "fixture_status",
                f"fixture-rc/{fixture_id}.json": "fixture_process_status",
                f"fixture-streams/{fixture_id}.stdout-stderr": "fixture_stdout_stderr",
                f"fixture-wrappers/{fixture_id}.do": "fixture_wrapper",
                f"fixture-invocations/{fixture_id}.json": "fixture_invocation",
            }
        )
    package = payload["package"]
    for name in package["staged_members"]:
        expected_roles[f"package/stata/{name}"] = "staged_package_member"
    for resource in manifest["resources"]:
        expected_roles[f"resources/{resource['path']}"] = "staged_resource"
    observed_roles = {
        item["path"]: item["role"] for item in payload["evidence_bundle"]["members"]
    }
    if observed_roles != expected_roles:
        raise BehaviorContractError("behavior bundle role/path inventory is not contract-closed")

    outcomes = {item["id"]: item for item in payload["fixture_outcomes"]}
    environment = payload["stata_environment"]
    for fixture in manifest["fixtures"]:
        fixture_id = fixture["id"]
        source = contents[f"fixture-sources/{fixture['path']}"]
        if sha256_bytes(source) != fixture["sha256"]:
            raise BehaviorContractError(f"bundled fixture source changed: {fixture_id}")
        log_member = f"fixture-logs/{fixture_id}.log"
        try:
            log_text = contents[log_member].decode("utf-8")
        except UnicodeDecodeError as error:
            raise BehaviorContractError(f"bundled fixture log is not UTF-8: {fixture_id}") from error
        log_error = stata_runner.validate_fixture_log(Path(fixture["path"]), log_text)
        if log_error is not None:
            raise BehaviorContractError(f"bundled fixture log failed: {fixture_id}: {log_error}")
        provenance = f"fereg.git_commit={payload['source']['commit'][:12]}"
        if provenance not in {line.strip() for line in log_text.splitlines()}:
            raise BehaviorContractError(
                f"bundled fixture log is not bound to source provenance: {fixture_id}"
            )
        status = _status_from_bytes(
            contents[f"fixture-status/{fixture_id}.status"],
            f"bundled fixture status {fixture_id}",
        )
        try:
            rc_record = json.loads(
                contents[f"fixture-rc/{fixture_id}.json"].decode("ascii")
            )
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BehaviorContractError(f"bundled fixture rc is malformed: {fixture_id}") from error
        expected_rc = {
            "schema_version": 1,
            "fixture_id": fixture_id,
            "process_rc": 0,
            "stata_rc": 0,
        }
        if (
            rc_record != expected_rc
            or contents[f"fixture-rc/{fixture_id}.json"]
            != canonical_json_bytes(expected_rc) + b"\n"
            or status["stata_rc"] != "0"
        ):
            raise BehaviorContractError(f"bundled fixture rc did not pass: {fixture_id}")
        invocation_bytes = contents[f"fixture-invocations/{fixture_id}.json"]
        stream_bytes = contents[f"fixture-streams/{fixture_id}.stdout-stderr"]
        try:
            stream_text = stream_bytes.decode("utf-8")
        except UnicodeDecodeError as error:
            raise BehaviorContractError(f"bundled fixture process stream is not UTF-8: {fixture_id}") from error
        if "\r" in stream_text or "\x00" in stream_text:
            raise BehaviorContractError(f"bundled fixture process stream is not canonical: {fixture_id}")
        try:
            invocation = json.loads(invocation_bytes.decode("ascii"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BehaviorContractError(f"bundled fixture invocation is malformed: {fixture_id}") from error
        invocation_environment = invocation.get("environment") if isinstance(invocation, Mapping) else None
        argv = invocation.get("argv") if isinstance(invocation, Mapping) else None
        cwd = invocation.get("cwd") if isinstance(invocation, Mapping) else None
        controller_source_root = invocation.get("controller_source_root") if isinstance(invocation, Mapping) else None
        wrapper_bytes = contents[f"fixture-wrappers/{fixture_id}.do"]
        try:
            wrapper_text = wrapper_bytes.decode("utf-8")
        except UnicodeDecodeError as error:
            raise BehaviorContractError(f"bundled fixture wrapper is not UTF-8: {fixture_id}") from error
        home_value = invocation_environment.get("HOME") if isinstance(invocation_environment, Mapping) else None
        tmp_value = invocation_environment.get("TMPDIR") if isinstance(invocation_environment, Mapping) else None
        if (
            not isinstance(invocation_environment, Mapping)
            or set(invocation) != {
                "schema_version", "argv", "cwd", "environment",
                "stdout_stderr", "controller_source_root",
            }
            or set(invocation_environment) != {"HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ"}
            or invocation_environment.get("PATH") != "/usr/bin:/bin"
            or invocation_environment.get("LANG") != "C"
            or invocation_environment.get("LC_ALL") != "C"
            or invocation_environment.get("TZ") != "UTC"
            or not isinstance(cwd, str)
            or argv != [str(DARWIN_STATA), "-q", "-b", "do", f"{cwd}/run.do"]
            or invocation.get("schema_version") != 1
            or invocation.get("stdout_stderr") != "merged"
            or not isinstance(controller_source_root, str)
            or not Path(controller_source_root).is_absolute()
            or Path(controller_source_root).as_posix() != controller_source_root
            or any(part in {".", ".."} for part in Path(controller_source_root).parts)
            or controller_source_root != payload["controller_source"]["lexical_root"]
            or not isinstance(cwd, str) or not Path(cwd).is_absolute()
            or Path(cwd).as_posix() != cwd
            or any(part in {".", ".."} for part in Path(cwd).parts)
            or not isinstance(home_value, str) or not Path(home_value).is_absolute()
            or Path(home_value).as_posix() != home_value
            or any(part in {".", ".."} for part in Path(home_value).parts)
            or not isinstance(tmp_value, str) or not Path(tmp_value).is_absolute()
            or Path(tmp_value).as_posix() != tmp_value
            or any(part in {".", ".."} for part in Path(tmp_value).parts)
            or invocation_bytes != canonical_json_bytes(invocation) + b"\n"
            or not isinstance(home_value, str) or not home_value.endswith("/controller-home")
            or not isinstance(tmp_value, str) or not tmp_value.endswith("/controller-tmp")
            or Path(home_value).parent != Path(tmp_value).parent
        ):
            raise BehaviorContractError(f"bundled fixture action proof changed: {fixture_id}")
        staged_ado = Path(home_value).parent / "bundle/stata"
        artifact_platform = payload["artifact"]["platform"]
        artifact_filename = next(
            (filename for platform_id, filename in PLATFORMS.values() if platform_id == artifact_platform),
            None,
        )
        if artifact_filename is None:
            raise BehaviorContractError("behavior artifact platform has no plugin filename")
        expected_wrapper = stata_runner.render_fixture_wrapper(
            path=(Path(controller_source_root) / fixture["path"]),
            root=Path(controller_source_root),
            run_dir=Path(cwd),
            bound_plugin=staged_ado / artifact_filename,
            bound_ado_root=staged_ado,
        )
        if wrapper_bytes != expected_wrapper:
            raise BehaviorContractError(f"bundled fixture wrapper differs byte-for-byte: {fixture_id}")
        expected_environment = {
            "stata_version": status["stata_version"],
            "stata_edition": status["stata_edition"],
            "stata_os": status["stata_os"],
            "stata_machine_type": status["stata_machine_type"],
            "stata_processors": stata_runner.optional_int(status["stata_processors"]),
        }
        if expected_environment != environment:
            raise BehaviorContractError(f"bundled fixture environment changed: {fixture_id}")
        outcome = outcomes[fixture_id]
        if (
            outcome["process_rc"] != rc_record["process_rc"]
            or outcome["stata_rc"] != int(status["stata_rc"])
            or outcome["terminal_sentinel"] != stata_runner.expected_sentinel(Path(fixture["path"]))
            or outcome["log_member"] != log_member
            or outcome["status_member"] != f"fixture-status/{fixture_id}.status"
            or outcome["rc_member"] != f"fixture-rc/{fixture_id}.json"
            or outcome["stream_member"] != f"fixture-streams/{fixture_id}.stdout-stderr"
            or outcome["wrapper_member"] != f"fixture-wrappers/{fixture_id}.do"
            or outcome["invocation_member"] != f"fixture-invocations/{fixture_id}.json"
            or outcome["normalized_output"]
            != _normalize_fixture_output(fixture_id, log_text)
        ):
            raise BehaviorContractError(f"fixture outcome is not reproduced by bundle: {fixture_id}")

    staged_files = {
        name: {
            "sha256": sha256_bytes(contents[f"package/stata/{name}"]),
            "size_bytes": len(contents[f"package/stata/{name}"]),
        }
        for name in package["staged_members"]
    }
    if staged_files != package["staged_files"]:
        raise BehaviorContractError("bundled staged-package file identities changed")
    package_doc = _package_document_from_text(
        contents["package/stata/fereg.pkg"].decode("utf-8"),
        staged_files["fereg.pkg"],
    )
    toc_doc = _toc_document_from_text(
        contents["package/stata/stata.toc"].decode("utf-8"),
        staged_files["stata.toc"],
    )
    if package_doc != package["package"] or toc_doc != package["toc"]:
        raise BehaviorContractError("bundled package metadata does not reproduce observation")
    help_examples = _help_examples_from_text(
        contents["package/stata/fereg.sthlp"].decode("utf-8"),
        manifest["package_contract"]["help_example_ids"],
    )
    if help_examples != package["help_examples"]:
        raise BehaviorContractError("bundled fereg help examples do not reproduce observation")
    for name in manifest["package_contract"]["byte_identical_fereg_members"]:
        if package["byte_identical_fereg_files"][name] != staged_files[name]:
            raise BehaviorContractError(f"bundled byte-frozen package identity changed: {name}")
    plugin_name = next(
        filename for platform_id, filename in PLATFORMS.values()
        if platform_id == payload["artifact"]["platform"]
    )
    _verify_plugin_source_bytes(
        contents[f"package/stata/{plugin_name}"], payload["source"]
    )
    if staged_files[plugin_name] != {
        "sha256": payload["artifact"]["sha256"],
        "size_bytes": payload["artifact"]["size_bytes"],
    }:
        raise BehaviorContractError("bundled native artifact differs from observation")
    for resource in manifest["resources"]:
        content = contents[f"resources/{resource['path']}"]
        if sha256_bytes(content) != resource["sha256"]:
            raise BehaviorContractError("bundled behavior resource changed")


def verify_observation(
    receipt: Path,
    bundle: Path,
    manifest_path: Path = DEFAULT_MANIFEST,
    *,
    expected_role: str | None = None,
    expected_phase: str | None = None,
) -> Mapping[str, Any]:
    """Authoritatively reopen one observation and its deterministic evidence tar."""
    manifest = load_manifest(manifest_path)
    manifest_sha = sha256_file(manifest_path)
    payload, _ = _read_sealed(receipt)
    _validate_observation(manifest, manifest_sha, payload)
    if expected_role is not None and payload["role"] != expected_role:
        raise BehaviorContractError("behavior observation role differs from caller binding")
    if expected_phase is not None and payload["phase"] != expected_phase:
        raise BehaviorContractError("behavior observation phase differs from caller binding")
    _verify_bundle_payload(manifest, payload, bundle)
    return payload


def _observation_binding(
    receipt: Path,
    bundle: Path,
    document: Mapping[str, Any],
    payload: Mapping[str, Any],
) -> dict[str, Any]:
    evidence = payload["evidence_bundle"]
    if (
        bundle.name != evidence["filename"]
        or sha256_file(bundle) != evidence["sha256"]
        or bundle.stat().st_size != evidence["size_bytes"]
    ):
        raise BehaviorContractError("observation bundle identity changed after verification")
    return {
        "sha256": sha256_file(receipt),
        "payload_sha256": document["seal"]["payload_sha256"],
        "bundle_filename": bundle.name,
        "bundle_sha256": evidence["sha256"],
        "bundle_size_bytes": evidence["size_bytes"],
    }


def verify_comparison(
    comparison: Path,
    baseline_receipt: Path,
    baseline_bundle: Path,
    candidate_receipt: Path,
    candidate_bundle: Path,
    manifest_path: Path = DEFAULT_MANIFEST,
) -> Mapping[str, Any]:
    """Reopen both evidence bundles and reproduce a sealed comparison exactly."""
    manifest = load_manifest(manifest_path)
    manifest_sha = sha256_file(manifest_path)
    baseline = verify_observation(
        baseline_receipt, baseline_bundle, manifest_path, expected_role="baseline",
        expected_phase="baseline",
    )
    candidate = verify_observation(
        candidate_receipt, candidate_bundle, manifest_path, expected_role="candidate",
    )
    baseline_payload, baseline_document = _read_sealed(baseline_receipt)
    candidate_payload, candidate_document = _read_sealed(candidate_receipt)
    if baseline_payload != baseline or candidate_payload != candidate:
        raise BehaviorContractError("observation changed during comparison verification")
    payload, _ = _read_sealed(comparison)
    _validate_comparison(manifest, manifest_sha, payload)
    expected_result = evaluate_receipts(manifest, manifest_sha, baseline, candidate)
    if (
        payload["phase"] != candidate["phase"]
        or payload["baseline_receipt"]
        != _observation_binding(
            baseline_receipt, baseline_bundle, baseline_document, baseline
        )
        or payload["candidate_receipt"]
        != _observation_binding(
            candidate_receipt, candidate_bundle, candidate_document, candidate
        )
        or payload["baseline"]
        != {"source": baseline["source"], "artifact": baseline["artifact"]}
        or payload["candidate"]
        != {"source": candidate["source"], "artifact": candidate["artifact"]}
        or payload["stata_environment"] != baseline["stata_environment"]
        or payload["comparison"] != expected_result
    ):
        raise BehaviorContractError("comparison is not reproduced by bound observations/bundles")
    return payload


def evaluate_command(args: argparse.Namespace) -> dict[str, Any]:
    _require_isolated_controller()
    manifest = load_manifest(args.manifest)
    manifest_sha = sha256_file(args.manifest)
    verify_observation(
        args.baseline_receipt,
        args.baseline_bundle,
        args.manifest,
        expected_role="baseline",
        expected_phase="baseline",
    )
    verify_observation(
        args.candidate_receipt,
        args.candidate_bundle,
        args.manifest,
        expected_role="candidate",
    )
    baseline, baseline_document = _read_sealed(args.baseline_receipt)
    candidate, candidate_document = _read_sealed(args.candidate_receipt)
    comparison = evaluate_receipts(manifest, manifest_sha, baseline, candidate)
    payload = {
        "schema_version": 1,
        "receipt_type": manifest["receipt_contract"]["comparison_type"],
        "status": "pass",
        "phase": candidate["phase"],
        "contract": {"id": manifest["manifest_id"], "sha256": manifest_sha},
        "baseline_receipt": _observation_binding(
            args.baseline_receipt,
            args.baseline_bundle,
            baseline_document,
            baseline,
        ),
        "candidate_receipt": _observation_binding(
            args.candidate_receipt,
            args.candidate_bundle,
            candidate_document,
            candidate,
        ),
        "baseline": {"source": baseline["source"], "artifact": baseline["artifact"]},
        "candidate": {"source": candidate["source"], "artifact": candidate["artifact"]},
        "stata_environment": baseline["stata_environment"],
        "comparison": comparison,
        "complete_sentinel": manifest["receipt_contract"]["complete_sentinel"],
    }
    _validate_comparison(manifest, manifest_sha, payload)
    document = seal_payload(payload)
    _write_new(args.output, document)
    verify_comparison(
        args.output,
        args.baseline_receipt,
        args.baseline_bundle,
        args.candidate_receipt,
        args.candidate_bundle,
        args.manifest,
    )
    _write_identity(args.identity_output, args.output, document)
    return document


def _write_identity(path: Path, receipt_path: Path, document: Mapping[str, Any]) -> None:
    payload = document["payload"]
    identity = {
        "schema_version": 1,
        "receipt_type": payload["receipt_type"],
        "status": payload["status"],
        "receipt_filename": receipt_path.name,
        "receipt_sha256": sha256_file(receipt_path),
        "payload_sha256": document["seal"]["payload_sha256"],
        "contract": payload["contract"],
        "complete_sentinel": payload["complete_sentinel"],
    }
    if "source" in payload:
        identity["source_commit"] = payload["source"]["commit"]
        identity["artifact_sha256"] = payload["artifact"]["sha256"]
        identity["role"] = payload["role"]
        identity["phase"] = payload["phase"]
        identity["evidence_bundle_filename"] = payload["evidence_bundle"]["filename"]
        identity["evidence_bundle_sha256"] = payload["evidence_bundle"]["sha256"]
    else:
        identity["source_commit"] = payload["candidate"]["source"]["commit"]
        identity["artifact_sha256"] = payload["candidate"]["artifact"]["sha256"]
        identity["phase"] = payload["phase"]
    _write_new(path, identity)


COMPARISON_FIELDS = {
    "schema_version", "receipt_type", "status", "phase", "contract",
    "baseline_receipt", "candidate_receipt", "baseline", "candidate",
    "stata_environment", "comparison", "complete_sentinel",
}


def _validate_comparison(
    manifest: Mapping[str, Any], manifest_sha: str, payload: Mapping[str, Any]
) -> None:
    _keys(payload, COMPARISON_FIELDS, "direct command comparison")
    phase = payload["phase"]
    if (
        payload["schema_version"] != 1
        or payload["receipt_type"] != manifest["receipt_contract"]["comparison_type"]
        or payload["status"] != "pass"
        or phase not in {"preactivation", "activation"}
        or payload["contract"] != {"id": manifest["manifest_id"], "sha256": manifest_sha}
        or payload["complete_sentinel"] != manifest["receipt_contract"]["complete_sentinel"]
    ):
        raise BehaviorContractError("direct command comparison identity/status is invalid")
    for name in ("baseline_receipt", "candidate_receipt"):
        record = _mapping(payload[name], name)
        _keys(
            record,
            {
                "sha256", "payload_sha256", "bundle_filename",
                "bundle_sha256", "bundle_size_bytes",
            },
            name,
        )
        _sha256(record["sha256"], f"{name}.sha256")
        _sha256(record["payload_sha256"], f"{name}.payload_sha256")
        _sha256(record["bundle_sha256"], f"{name}.bundle_sha256")
        if (
            not isinstance(record["bundle_filename"], str)
            or Path(record["bundle_filename"]).name != record["bundle_filename"]
            or isinstance(record["bundle_size_bytes"], bool)
            or not isinstance(record["bundle_size_bytes"], int)
            or record["bundle_size_bytes"] < 10240
        ):
            raise BehaviorContractError(f"{name} bundle identity is invalid")
    baseline = _mapping(payload["baseline"], "comparison.baseline")
    candidate = _mapping(payload["candidate"], "comparison.candidate")
    for name, record in (("baseline", baseline), ("candidate", candidate)):
        _keys(record, {"source", "artifact"}, f"comparison.{name}")
        source = _mapping(record["source"], f"comparison.{name}.source")
        artifact = _mapping(record["artifact"], f"comparison.{name}.artifact")
        _validate_source_record(source, f"comparison.{name}.source")
        _validate_artifact_record(artifact, source, f"comparison.{name}.artifact")
    if baseline["source"] != manifest["baseline_source"]:
        raise BehaviorContractError("comparison baseline source is not exact F0")
    if candidate["source"] == manifest["baseline_source"]:
        raise BehaviorContractError("comparison candidate source is vacuously F0")
    if baseline["artifact"]["platform"] != candidate["artifact"]["platform"]:
        raise BehaviorContractError("comparison artifacts are not from the same native platform")
    environment = _mapping(payload["stata_environment"], "comparison.stata_environment")
    _validate_stata_environment(environment, "comparison.stata_environment")
    comparison = _mapping(payload["comparison"], "comparison.results")
    expected_results = {
        "fixtures_equal": True,
        "fixture_count": len(FIXTURE_IDS),
        "package": _expected_package_comparison(
            manifest,
            phase=phase,
            platform_id=baseline["artifact"]["platform"],
        ),
    }
    if comparison != expected_results:
        raise BehaviorContractError("direct command comparison result fields are not exact")


def _expected_identity(
    receipt_path: Path, payload: Mapping[str, Any], document: Mapping[str, Any]
) -> dict[str, Any]:
    identity = {
        "schema_version": 1,
        "receipt_type": payload["receipt_type"],
        "status": payload["status"],
        "receipt_filename": receipt_path.name,
        "receipt_sha256": sha256_file(receipt_path),
        "payload_sha256": document["seal"]["payload_sha256"],
        "contract": payload["contract"],
        "complete_sentinel": payload["complete_sentinel"],
    }
    if "source" in payload:
        identity.update(
            {
                "source_commit": payload["source"]["commit"],
                "artifact_sha256": payload["artifact"]["sha256"],
                "role": payload["role"],
                "phase": payload["phase"],
                "evidence_bundle_filename": payload["evidence_bundle"]["filename"],
                "evidence_bundle_sha256": payload["evidence_bundle"]["sha256"],
            }
        )
    else:
        identity.update(
            {
                "source_commit": payload["candidate"]["source"]["commit"],
                "artifact_sha256": payload["candidate"]["artifact"]["sha256"],
                "phase": payload["phase"],
            }
        )
    return identity


def verify_command(args: argparse.Namespace) -> Mapping[str, Any]:
    _require_isolated_controller()
    manifest = load_manifest(args.manifest)
    manifest_sha = sha256_file(args.manifest)
    payload, document = _read_sealed(args.receipt)
    receipt_type = payload.get("receipt_type")
    if receipt_type == manifest["receipt_contract"]["observation_type"]:
        if args.bundle is None:
            raise BehaviorContractError("observation verification requires --bundle")
        verified = verify_observation(args.receipt, args.bundle, args.manifest)
    elif receipt_type == manifest["receipt_contract"]["comparison_type"]:
        required = (
            args.baseline_receipt, args.baseline_bundle,
            args.candidate_receipt, args.candidate_bundle,
        )
        if any(item is None for item in required):
            raise BehaviorContractError(
                "comparison verification requires both observation receipts and bundles"
            )
        verified = verify_comparison(
            args.receipt,
            args.baseline_receipt,
            args.baseline_bundle,
            args.candidate_receipt,
            args.candidate_bundle,
            args.manifest,
        )
    else:
        raise BehaviorContractError("unknown behavior receipt type")
    if verified != payload:
        raise BehaviorContractError("authoritative behavior verification changed payload")
    if args.identity is not None:
        identity = _read_json(args.identity, "behavior receipt identity")
        if identity != _expected_identity(args.receipt, payload, document):
            raise BehaviorContractError("behavior receipt external identity mismatch")
    return payload


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    capture = commands.add_parser("capture", help="capture one exact baseline or candidate observation")
    capture.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    capture.add_argument("--f0-manifest", type=Path, default=DEFAULT_F0_MANIFEST)
    capture.add_argument("--role", choices=("baseline", "candidate"), required=True)
    capture.add_argument("--phase", choices=("baseline", "preactivation", "activation"), required=True)
    capture.add_argument("--source", type=Path, required=True)
    capture.add_argument("--plugin", type=Path, required=True)
    capture.add_argument("--stata", required=True)
    capture.add_argument("--run-root", type=Path, required=True)
    capture.add_argument("--output", type=Path, required=True)
    capture.add_argument("--bundle-output", type=Path, required=True)
    capture.add_argument("--identity-output", type=Path, required=True)
    evaluate = commands.add_parser("evaluate", help="compare sealed F0 and candidate observations")
    evaluate.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    evaluate.add_argument("--baseline-receipt", type=Path, required=True)
    evaluate.add_argument("--baseline-bundle", type=Path, required=True)
    evaluate.add_argument("--candidate-receipt", type=Path, required=True)
    evaluate.add_argument("--candidate-bundle", type=Path, required=True)
    evaluate.add_argument("--output", type=Path, required=True)
    evaluate.add_argument("--identity-output", type=Path, required=True)
    verify = commands.add_parser("verify", help="verify an immutable observation or comparison receipt")
    verify.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    verify.add_argument("--receipt", type=Path, required=True)
    verify.add_argument("--identity", type=Path)
    verify.add_argument("--bundle", type=Path)
    verify.add_argument("--baseline-receipt", type=Path)
    verify.add_argument("--baseline-bundle", type=Path)
    verify.add_argument("--candidate-receipt", type=Path)
    verify.add_argument("--candidate-bundle", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    try:
        _require_isolated_controller()
    except BehaviorContractError as error:
        raise SystemExit(f"fereg F0 behavior contract failed closed: {error}") from error
    args = build_parser().parse_args(argv)
    try:
        if args.command == "capture":
            document = capture_observation(args)
        elif args.command == "evaluate":
            document = evaluate_command(args)
        else:
            payload = verify_command(args)
            print(f"FEREG_F0_BEHAVIOR_RECEIPT_OK|{payload['receipt_type']}")
            return 0
    except BehaviorContractError as error:
        raise SystemExit(f"fereg F0 behavior contract failed closed: {error}") from error
    print(
        f"FEREG_F0_BEHAVIOR_{document['payload']['status'].upper()}|"
        f"{document['payload']['receipt_type']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
