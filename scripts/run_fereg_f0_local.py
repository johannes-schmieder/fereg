#!/usr/bin/env python3
"""Prepare and run the closed licensed-Stata fereg candidate-versus-F0 screen.

The ``prepare`` command generates and seals four deterministic ``.dta`` files
outside all measurement.  The ``run`` command never generates data or runs a
comparator: it stages two complete exact-source fereg packages, verifies the
preparation receipt, performs a scientific preflight for every cell, and only
then starts the registered nine alternating AB/BA pairs.  Every arm invocation
is a new Stata process whose RSS is sampled between READY/START/DONE markers.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import socket
import subprocess
import sys
import time
from typing import Any, Callable, Mapping, Sequence

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts import fereg_f0_regression as evaluator
from scripts import fereg_f0_stata_capture as capture


DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json"
PROFILE = "local_screen"
INPUT_TYPE = "fereg-f0-local-input-v1"
PLUGIN_FILENAME = "fereg_macos.plugin"
PLATFORM = "macos-universal"
PAIR_ORDERS = ("AB", "BA", "AB", "BA", "AB", "BA", "AB", "BA", "AB")
BASE_CASE_BY_DESIGN = {
    "dense_two_fe": "bench_dense_two_fe_t{threads}",
    "sparse_two_fe": "bench_sparse_two_fe_t{threads}",
    "absorbed_slope": "bench_absorbed_slope_t{threads}",
    "clustered_vce": "bench_clustered_vce_t{threads}",
    "moderate_k": "bench_moderate_k_t{threads}",
}
PREPARATION_TYPE = "fereg-f0-local-preparation-v1"
PREPARATION_SENTINEL = "FEREG_F0_LOCAL_PREPARATION_SUCCESS"
SHA256 = re.compile(r"^[0-9a-f]{64}$")
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]{1,95}$")
OPTION = r"(?:^|\s){name}\(([^)]*)\)(?=\s|$)"


class LocalRunContractError(RuntimeError):
    """A local-run input, child, identity, or evidence contract failed."""


@dataclass(frozen=True)
class DatasetInput:
    id: str
    path: Path
    sha256: str
    observations: int


@dataclass(frozen=True)
class CellInput:
    cell_id: str
    dataset: DatasetInput
    command: str
    saved_outputs: tuple[str, ...]


@dataclass(frozen=True)
class LocalInputs:
    manifest_sha256: str
    input_sha256: str
    obs_id: str
    cells: tuple[CellInput, ...]


@dataclass(frozen=True)
class ArmRun:
    result: capture.ArmCaptureResult
    peak_rss_bytes: int


def _closed(value: Any, fields: set[str], where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise LocalRunContractError(f"{where} must be an object")
    if set(value) != fields:
        missing = sorted(fields - set(value))
        extra = sorted(set(value) - fields)
        raise LocalRunContractError(
            f"{where} fields are not closed; missing={missing}, extra={extra}"
        )
    return value


def _positive_integer(value: Any, where: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise LocalRunContractError(f"{where} must be a positive integer")
    return value


def _option(command: str, name: str) -> str | None:
    matches = re.findall(OPTION.format(name=re.escape(name)), command, re.IGNORECASE)
    if len(matches) > 1:
        raise LocalRunContractError(f"command repeats {name}()")
    return matches[0].strip() if matches else None


def _validate_command(cell: Mapping[str, Any], command: str, outputs: tuple[str, ...]) -> None:
    if not isinstance(command, str) or command.strip() != command:
        raise LocalRunContractError(f"{cell['id']} command is not exact text")
    lowered = command.lower()
    if not lowered.startswith("fereg "):
        raise LocalRunContractError(f"{cell['id']} command is not fereg")
    if any(token in command for token in ("\n", "\r", ";")):
        raise LocalRunContractError(f"{cell['id']} command contains a directive")
    if any(token in lowered for token in ("reghdfe", " areg ", " use ", " set obs ")):
        raise LocalRunContractError(f"{cell['id']} command contains out-of-scope work")
    if _option(command, "threads") != str(cell["threads"]):
        raise LocalRunContractError(f"{cell['id']} command has the wrong threads()")
    if _option(command, "memory") != cell["memory"]:
        raise LocalRunContractError(f"{cell['id']} command has the wrong memory()")
    vce = _option(command, "vce")
    if cell["vce"] == "cluster":
        if vce is None or not vce.lower().startswith("cluster "):
            raise LocalRunContractError(f"{cell['id']} command lacks clustered VCE")
    elif vce is not None and vce.lower() != "unadjusted":
        raise LocalRunContractError(f"{cell['id']} command changes the registered VCE")
    if cell["outputs"] == "save_fe":
        if len(outputs) != 2 or _option(command, "absorb") is None:
            raise LocalRunContractError(
                f"{cell['id']} must bind exactly two saved FE variables"
            )
        absorb = _option(command, "absorb") or ""
        if not all(f"{name}=" in absorb for name in outputs):
            raise LocalRunContractError(
                f"{cell['id']} saved outputs are not bound in absorb()"
            )
    elif outputs:
        raise LocalRunContractError(f"{cell['id']} unexpectedly saves variables")


def load_local_inputs(
    path: Path,
    manifest: Mapping[str, Any],
    manifest_sha256: str,
) -> LocalInputs:
    """Load and verify the immutable datasets and exact 18-cell command map."""
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise LocalRunContractError(f"cannot read local input contract: {error}") from error
    document = _closed(
        raw,
        {"schema_version", "input_type", "manifest_sha256", "obs_id", "datasets", "cells"},
        "local input",
    )
    if (
        document["schema_version"] != 1
        or document["input_type"] != INPUT_TYPE
        or document["manifest_sha256"] != manifest_sha256
    ):
        raise LocalRunContractError("local input identity or manifest binding is stale")
    obs_id = document["obs_id"]
    if not isinstance(obs_id, str) or capture.SAFE_VARIABLE.fullmatch(obs_id) is None:
        raise LocalRunContractError("obs_id is not a safe Stata variable")

    dataset_values = document["datasets"]
    if not isinstance(dataset_values, list) or not dataset_values:
        raise LocalRunContractError("local input has no immutable datasets")
    datasets: dict[str, DatasetInput] = {}
    for index, value in enumerate(dataset_values):
        item = _closed(
            value, {"id", "path", "sha256", "observations"}, f"dataset[{index}]"
        )
        dataset_id = item["id"]
        if (
            not isinstance(dataset_id, str)
            or SAFE_ID.fullmatch(dataset_id) is None
            or dataset_id in datasets
        ):
            raise LocalRunContractError("dataset ids are malformed or duplicated")
        if not isinstance(item["path"], str) or not item["path"]:
            raise LocalRunContractError(f"dataset {dataset_id} path is malformed")
        digest = item["sha256"]
        if not isinstance(digest, str) or SHA256.fullmatch(digest) is None:
            raise LocalRunContractError(f"dataset {dataset_id} SHA-256 is malformed")
        dataset_path = Path(item["path"])
        if not dataset_path.is_absolute():
            dataset_path = path.parent / dataset_path
        dataset_path = dataset_path.resolve()
        if not dataset_path.is_file() or dataset_path.is_symlink():
            raise LocalRunContractError(f"dataset {dataset_id} is absent or a symlink")
        if capture.sha256_file(dataset_path) != digest:
            raise LocalRunContractError(f"dataset {dataset_id} SHA-256 mismatch")
        datasets[dataset_id] = DatasetInput(
            id=dataset_id,
            path=dataset_path,
            sha256=digest,
            observations=_positive_integer(item["observations"], f"dataset {dataset_id}.observations"),
        )

    manifest_cells = {
        item["id"]: item for item in manifest["cells"] if item["profile"] == PROFILE
    }
    expected_ids = evaluator.EXPECTED_CELLS_BY_PROFILE[PROFILE]
    if tuple(manifest_cells) != expected_ids:
        raise LocalRunContractError("manifest local cell order differs from evaluator")
    cell_values = document["cells"]
    if not isinstance(cell_values, list):
        raise LocalRunContractError("local input cells must be a list")
    cells: list[CellInput] = []
    for index, value in enumerate(cell_values):
        item = _closed(
            value,
            {"cell_id", "dataset_id", "command", "saved_outputs"},
            f"cell[{index}]",
        )
        if index >= len(expected_ids) or item["cell_id"] != expected_ids[index]:
            raise LocalRunContractError("local input cell inventory/order is incomplete")
        if item["dataset_id"] not in datasets:
            raise LocalRunContractError(f"{item['cell_id']} references an unknown dataset")
        dataset = datasets[item["dataset_id"]]
        registered = manifest_cells[item["cell_id"]]
        if dataset.observations != registered["n"]:
            raise LocalRunContractError(f"{item['cell_id']} dataset scale changed")
        output_values = item["saved_outputs"]
        if not isinstance(output_values, list) or not all(
            isinstance(name, str) and capture.SAFE_VARIABLE.fullmatch(name)
            for name in output_values
        ):
            raise LocalRunContractError(f"{item['cell_id']} saved_outputs is malformed")
        outputs = tuple(output_values)
        if len(outputs) != len(set(outputs)):
            raise LocalRunContractError(f"{item['cell_id']} repeats a saved output")
        _validate_command(registered, item["command"], outputs)
        cells.append(
            CellInput(
                cell_id=item["cell_id"],
                dataset=dataset,
                command=item["command"],
                saved_outputs=outputs,
            )
        )
    if len(cells) != len(expected_ids):
        raise LocalRunContractError("local input does not contain exactly 18 cells")
    return LocalInputs(
        manifest_sha256=manifest_sha256,
        input_sha256=capture.sha256_file(path),
        obs_id=obs_id,
        cells=tuple(cells),
    )


def _package_members(package: Path) -> tuple[str, ...]:
    if not package.is_file() or package.is_symlink():
        raise LocalRunContractError("source package lacks a regular fereg.pkg")
    members = ["fereg.pkg", "stata.toc"]
    for line in package.read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"f\s+([^\s]+)", line.strip())
        if match is None:
            continue
        name = match.group(1)
        if Path(name).name != name or name in members:
            raise LocalRunContractError("fereg.pkg contains an unsafe or duplicate member")
        if name.endswith(".plugin") and name != PLUGIN_FILENAME:
            continue
        members.append(name)
    if "fereg.ado" not in members or PLUGIN_FILENAME not in members:
        raise LocalRunContractError("fereg.pkg omits fereg or the macOS plugin")
    return tuple(members)


def stage_package(source_root: Path, plugin: Path, destination: Path) -> Path:
    """Stage one complete package from a source checkout plus one native artifact."""
    source_stata = source_root.resolve() / "stata"
    plugin = plugin.resolve()
    if not source_stata.is_dir() or source_stata.is_symlink():
        raise LocalRunContractError("source checkout has no regular stata directory")
    if not plugin.is_file() or plugin.is_symlink():
        raise LocalRunContractError("native artifact is absent or a symlink")
    members = _package_members(source_stata / "fereg.pkg")
    if destination.exists():
        raise LocalRunContractError(f"refusing to overwrite staged package {destination}")
    destination.mkdir(parents=True)
    try:
        for name in members:
            source = plugin if name == PLUGIN_FILENAME else source_stata / name
            if not source.is_file() or source.is_symlink():
                raise LocalRunContractError(f"source package member is absent: {name}")
            shutil.copyfile(source, destination / name)
    except BaseException:
        shutil.rmtree(destination, ignore_errors=True)
        raise
    if tuple(sorted(path.name for path in destination.iterdir())) != tuple(sorted(members)):
        raise LocalRunContractError("staged package inventory is not closed")
    return destination.resolve()


def _artifact(plugin: Path, source: Mapping[str, Any]) -> dict[str, Any]:
    data = plugin.read_bytes()
    embedded = str(source["commit"])[:12]
    if embedded.encode("ascii") not in data:
        raise LocalRunContractError("plugin does not embed its source commit prefix")
    return {
        "platform": PLATFORM,
        "sha256": capture.sha256_file(plugin),
        "size_bytes": plugin.stat().st_size,
        "embedded_git_commit": embedded,
    }


def _replace_option(command: str, name: str, value: str) -> str:
    pattern = re.compile(rf"\b{re.escape(name)}\([^)]*\)", re.IGNORECASE)
    replacement = f"{name}({value})"
    return pattern.sub(replacement, command) if pattern.search(command) else f"{command} {replacement}"


def _stata_quote(value: Path) -> str:
    text = str(value.resolve()).replace("\\", "/")
    if any(character in text for character in ('"', "\n", "\r")):
        raise LocalRunContractError("Stata path contains an unsupported character")
    return f'"{text}"'


def _generate_dataset(
    *,
    stata: str,
    generator: Path,
    arguments: Sequence[int],
    destination: Path,
    run_dir: Path,
    dataset_id: str,
    process_runner: Callable[..., subprocess.CompletedProcess[Any]] = subprocess.run,
) -> dict[str, str]:
    """Generate and close one immutable input in its own untimed Stata process."""
    run_dir.mkdir(parents=True, exist_ok=False)
    destination.parent.mkdir(parents=True, exist_ok=True)
    wrapper = run_dir / "generate.do"
    machine = run_dir / "generation.machine"
    argument_text = " ".join(str(value) for value in arguments)
    wrapper.write_text(
        "\n".join(
            (
                "version 18.0",
                "clear all",
                "set more off",
                f"capture noisily do {_stata_quote(generator)} {argument_text} {_stata_quote(destination)}",
                "local rc = _rc",
                "if `rc' != 0 exit `rc'",
                f"quietly use {_stata_quote(destination)}, clear",
                "capture confirm variable obs_id",
                "if !_rc exit 110",
                "generate long obs_id = _n",
                "isid obs_id",
                "order obs_id",
                f"quietly save {_stata_quote(destination)}, replace",
                f"file open __local_generation using {_stata_quote(machine)}, write text replace",
                'file write __local_generation "schema=1" _n',
                "file write __local_generation \"stata_version=`c(stata_version)'\" _n",
                "file write __local_generation \"stata_flavor=`c(flavor)'\" _n",
                "file write __local_generation \"os=`c(os)'\" _n",
                "file write __local_generation \"machine_type=`c(machine_type)'\" _n",
                f'file write __local_generation "dataset_id={dataset_id}" _n',
                "file close __local_generation",
                f'display as result "FEREG_F0_DATASET_SUCCESS|{dataset_id}"',
                "exit 0",
                "",
            )
        ),
        encoding="utf-8",
    )
    completed = process_runner(
        [stata, "-b", "do", str(wrapper)],
        cwd=run_dir,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    (run_dir / "process.stdout").write_text(completed.stdout or "", encoding="utf-8")
    log = run_dir / "generate.log"
    text = log.read_text(encoding="utf-8", errors="replace") if log.is_file() else ""
    sentinel = f"FEREG_F0_DATASET_SUCCESS|{dataset_id}"
    observed = [line.strip() for line in text.splitlines() if line.strip().startswith("FEREG_F0_DATASET_SUCCESS|")]
    if completed.returncode != 0 or observed != [sentinel] or not destination.is_file():
        raise LocalRunContractError(f"dataset generation failed closed: {dataset_id}")
    if not machine.is_file():
        raise LocalRunContractError(f"dataset generation omitted environment: {dataset_id}")
    environment: dict[str, str] = {}
    for line in machine.read_text(encoding="utf-8").splitlines():
        if "=" not in line:
            raise LocalRunContractError("generation environment record is malformed")
        key, value = line.split("=", 1)
        if key in environment or not value:
            raise LocalRunContractError("generation environment record is duplicated/empty")
        environment[key] = value
    expected = {"schema", "stata_version", "stata_flavor", "os", "machine_type", "dataset_id"}
    if set(environment) != expected or environment["schema"] != "1" or environment["dataset_id"] != dataset_id:
        raise LocalRunContractError("generation environment record is incomplete")
    return {key: environment[key] for key in ("stata_version", "stata_flavor", "os", "machine_type")}


def _load_ols_manifest(source_root: Path) -> tuple[Mapping[str, Any], Path]:
    path = source_root.resolve() / "benchmarks/manifests/ols_v1.json"
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise LocalRunContractError(f"cannot read F0 OLS manifest: {error}") from error
    if not isinstance(value, Mapping) or value.get("manifest_id") != "ols_v1":
        raise LocalRunContractError("F0 source lacks the registered OLS manifest")
    return value, path


def _preparation_commands(
    manifest: Mapping[str, Any],
    ols_manifest: Mapping[str, Any],
    datasets: Mapping[tuple[int, str], str],
) -> list[dict[str, Any]]:
    cases = {item["id"]: item for item in ols_manifest["benchmark_cases"]}
    output: list[dict[str, Any]] = []
    for cell_id in evaluator.EXPECTED_CELLS_BY_PROFILE[PROFILE]:
        cell = next(item for item in manifest["cells"] if item["id"] == cell_id)
        base_id = BASE_CASE_BY_DESIGN[cell["design"]].format(threads=cell["threads"])
        if base_id not in cases:
            raise LocalRunContractError(f"F0 OLS manifest lacks {base_id}")
        command = str(cases[base_id]["arms"]["candidate"])
        command = _replace_option(command, "threads", str(cell["threads"]))
        command = _replace_option(command, "memory", str(cell["memory"]))
        saved: list[str] = []
        if cell["outputs"] == "save_fe":
            before = "absorb(worker firm)"
            after = "absorb(__f0_worker=worker __f0_firm=firm)"
            if before not in command:
                raise LocalRunContractError("saved-FE base command changed")
            command = command.replace(before, after)
            saved = ["__f0_worker", "__f0_firm"]
        _validate_command(cell, command, tuple(saved))
        density = "sparse" if cell["design"] == "sparse_two_fe" else "dense"
        output.append(
            {
                "cell_id": cell_id,
                "dataset_id": datasets[(cell["n"], density)],
                "command": command,
                "saved_outputs": saved,
            }
        )
    return output


def prepare_local(
    args: argparse.Namespace,
    *,
    process_runner: Callable[..., subprocess.CompletedProcess[Any]] = subprocess.run,
) -> dict[str, Any]:
    """Create, hash, seal, and freeze all local inputs outside measured runs."""
    manifest_path = args.manifest.resolve()
    manifest = evaluator.load_manifest(manifest_path)
    manifest_sha = evaluator.sha256_file(manifest_path)
    source_root = args.baseline_source.resolve()
    source = evaluator.source_identity(source_root, "HEAD", require_clean=True)
    if source != manifest["baseline_candidate"]["source"]:
        raise LocalRunContractError("preparation source is not frozen F0")
    ols_manifest, ols_path = _load_ols_manifest(source_root)
    registered_datasets = {
        item["id"]: item for item in ols_manifest["datasets"] if item["id"] in {"generated_dense", "generated_sparse"}
    }
    if set(registered_datasets) != {"generated_dense", "generated_sparse"}:
        raise LocalRunContractError("F0 OLS manifest lacks registered generators")
    generator_rel = registered_datasets["generated_dense"]["source"]["path"]
    generator = source_root / generator_rel
    generator_sha = capture.sha256_file(generator)
    expected_generator_shas = {
        item["source"]["sha256"] for item in registered_datasets.values()
    }
    if expected_generator_shas != {generator_sha} or any(
        item["source"]["path"] != generator_rel for item in registered_datasets.values()
    ):
        raise LocalRunContractError("F0 generator identity differs across registered datasets")
    output_root = args.output_root.resolve()
    if output_root.exists():
        raise LocalRunContractError(f"refusing to reuse preparation root {output_root}")
    output_root.mkdir(parents=True)
    dataset_records: list[dict[str, Any]] = []
    dataset_ids: dict[tuple[int, str], str] = {}
    common_environment: Mapping[str, str] | None = None
    for n in (100_000, 1_000_000):
        for density in ("dense", "sparse"):
            dataset_id = f"local_n{n // 1000}k_{density}"
            dataset_ids[(n, density)] = dataset_id
            workers, firms = (
                (n // 1000, n // 10000)
                if density == "dense"
                else (n // 20, n // 50)
            )
            arguments = [n, workers, firms, 10, 20260821]
            destination = output_root / "datasets" / f"{dataset_id}.dta"
            environment = _generate_dataset(
                stata=args.stata,
                generator=generator,
                arguments=arguments,
                destination=destination,
                run_dir=output_root / "logs" / dataset_id,
                dataset_id=dataset_id,
                process_runner=process_runner,
            )
            if common_environment is None:
                common_environment = environment
            elif environment != common_environment:
                raise LocalRunContractError("Stata environment changed during preparation")
            dataset_records.append(
                {
                    "id": dataset_id,
                    "path": f"datasets/{dataset_id}.dta",
                    "sha256": capture.sha256_file(destination),
                    "size_bytes": destination.stat().st_size,
                    "observations": n,
                    "arguments": arguments,
                }
            )
            destination.chmod(0o444)
    config = {
        "schema_version": 1,
        "input_type": INPUT_TYPE,
        "manifest_sha256": manifest_sha,
        "obs_id": "obs_id",
        "datasets": [
            {
                key: record[key]
                for key in ("id", "path", "sha256", "observations")
            }
            for record in dataset_records
        ],
        "cells": _preparation_commands(manifest, ols_manifest, dataset_ids),
    }
    config_path = output_root / "local-input.json"
    _write_new(config_path, evaluator.canonical_json_bytes(config))
    # Re-read the finished contract before sealing it.
    load_local_inputs(config_path, manifest, manifest_sha)
    payload = {
        "schema_version": 1,
        "receipt_type": PREPARATION_TYPE,
        "manifest": {"id": manifest["manifest_id"], "sha256": manifest_sha},
        "baseline_source": source,
        "ols_manifest": {
            "id": ols_manifest["manifest_id"],
            "sha256": capture.sha256_file(ols_path),
        },
        "generator": {"path": generator_rel, "sha256": generator_sha},
        "datasets": dataset_records,
        "input_config": {
            "path": "local-input.json",
            "sha256": capture.sha256_file(config_path),
        },
        "stata_environment": dict(common_environment or {}),
        "terminal_sentinel": PREPARATION_SENTINEL,
    }
    receipt = evaluator.seal_payload(payload)
    receipt_path = output_root / "preparation-receipt.json"
    _write_new(receipt_path, evaluator.canonical_json_bytes(receipt))
    print(PREPARATION_SENTINEL, flush=True)
    return receipt


def verify_preparation(
    path: Path,
    input_path: Path,
    inputs: LocalInputs,
    manifest: Mapping[str, Any],
    baseline_source: Path,
) -> tuple[str, dict[str, str]]:
    try:
        encoded = path.read_bytes()
        document = json.loads(encoded)
    except (OSError, json.JSONDecodeError) as error:
        raise LocalRunContractError(f"cannot read preparation receipt: {error}") from error
    if encoded != evaluator.canonical_json_bytes(document):
        raise LocalRunContractError("preparation receipt is not canonical JSON")
    envelope = _closed(document, {"payload", "seal"}, "preparation receipt")
    payload = envelope["payload"]
    expected_seal = evaluator.seal_payload(payload)["seal"]
    if envelope["seal"] != expected_seal:
        raise LocalRunContractError("preparation receipt seal mismatch")
    record = _closed(
        payload,
        {
            "schema_version", "receipt_type", "manifest", "baseline_source",
            "ols_manifest", "generator", "datasets", "input_config",
            "stata_environment", "terminal_sentinel",
        },
        "preparation payload",
    )
    if (
        record["schema_version"] != 1
        or record["receipt_type"] != PREPARATION_TYPE
        or record["terminal_sentinel"] != PREPARATION_SENTINEL
        or record["manifest"] != {"id": manifest["manifest_id"], "sha256": inputs.manifest_sha256}
        or record["baseline_source"] != manifest["baseline_candidate"]["source"]
    ):
        raise LocalRunContractError("preparation receipt identity is stale")
    ols_manifest, ols_path = _load_ols_manifest(baseline_source)
    ols_identity = _closed(record["ols_manifest"], {"id", "sha256"}, "prepared OLS manifest")
    if ols_identity != {
        "id": "ols_v1",
        "sha256": capture.sha256_file(ols_path),
    }:
        raise LocalRunContractError("preparation OLS manifest identity changed")
    registered_generators = [
        item for item in ols_manifest["datasets"] if item["id"] in {"generated_dense", "generated_sparse"}
    ]
    if len(registered_generators) != 2:
        raise LocalRunContractError("audited OLS generator inventory changed")
    generator_paths = {item["source"]["path"] for item in registered_generators}
    generator_shas = {item["source"]["sha256"] for item in registered_generators}
    generator_identity = _closed(record["generator"], {"path", "sha256"}, "prepared generator")
    if len(generator_paths) != 1 or len(generator_shas) != 1 or generator_identity != {
        "path": next(iter(generator_paths)),
        "sha256": next(iter(generator_shas)),
    }:
        raise LocalRunContractError("preparation generator identity changed")
    generator_path = baseline_source.resolve() / str(generator_identity["path"])
    if capture.sha256_file(generator_path) != generator_identity["sha256"]:
        raise LocalRunContractError("audited generator file changed")
    config = _closed(record["input_config"], {"path", "sha256"}, "prepared input config")
    expected_config = (path.parent / str(config["path"])).resolve()
    if expected_config != input_path.resolve() or config["sha256"] != inputs.input_sha256:
        raise LocalRunContractError("preparation receipt does not bind the input config")
    if input_path.stat().st_mode & 0o222:
        raise LocalRunContractError("prepared input config is not immutable")
    prepared = record["datasets"]
    if not isinstance(prepared, list) or len(prepared) != 4:
        raise LocalRunContractError("preparation receipt dataset inventory is incomplete")
    by_id = {cell.dataset.id: cell.dataset for cell in inputs.cells}
    expected_datasets: list[tuple[str, list[int]]] = []
    expected_ids: dict[tuple[int, str], str] = {}
    for n in (100_000, 1_000_000):
        for density in ("dense", "sparse"):
            dataset_id = f"local_n{n // 1000}k_{density}"
            expected_ids[(n, density)] = dataset_id
            workers, firms = (
                (n // 1000, n // 10000)
                if density == "dense"
                else (n // 20, n // 50)
            )
            expected_datasets.append((dataset_id, [n, workers, firms, 10, 20260821]))
    for index, (value, expected_dataset) in enumerate(zip(prepared, expected_datasets)):
        item = _closed(
            value,
            {"id", "path", "sha256", "size_bytes", "observations", "arguments"},
            f"prepared dataset[{index}]",
        )
        expected_id, expected_arguments = expected_dataset
        dataset = by_id.get(item["id"])
        expected_path = (path.parent / str(item["path"])).resolve()
        if (
            item["id"] != expected_id
            or item["path"] != f"datasets/{expected_id}.dta"
            or item["arguments"] != expected_arguments
            or dataset is None
            or dataset.path != expected_path
            or dataset.sha256 != item["sha256"]
            or dataset.observations != item["observations"]
            or dataset.path.stat().st_size != item["size_bytes"]
        ):
            raise LocalRunContractError("preparation dataset binding changed")
        if dataset.path.stat().st_mode & 0o222:
            raise LocalRunContractError("prepared dataset is not immutable")
    expected_commands = _preparation_commands(manifest, ols_manifest, expected_ids)
    observed_commands = [
        {
            "cell_id": cell.cell_id,
            "dataset_id": cell.dataset.id,
            "command": cell.command,
            "saved_outputs": list(cell.saved_outputs),
        }
        for cell in inputs.cells
    ]
    if observed_commands != expected_commands:
        raise LocalRunContractError("prepared 18-cell command map differs from audited F0")
    environment = _closed(
        record["stata_environment"],
        {"stata_version", "stata_flavor", "os", "machine_type"},
        "preparation Stata environment",
    )
    if not all(isinstance(value, str) and value for value in environment.values()):
        raise LocalRunContractError("preparation Stata environment is incomplete")
    return capture.sha256_file(path), dict(environment)


def prepare_arms(
    manifest: Mapping[str, Any],
    baseline_source: Path,
    baseline_plugin: Path,
    candidate_source: Path,
    candidate_plugin: Path,
    run_root: Path,
) -> tuple[dict[str, Any], dict[str, Path]]:
    """Verify clean exact source identities and stage both isolated packages."""
    sources = {
        "baseline": evaluator.source_identity(baseline_source, "HEAD", require_clean=True),
        "candidate": evaluator.source_identity(candidate_source, "HEAD", require_clean=True),
    }
    if sources["baseline"] != manifest["baseline_candidate"]["source"]:
        raise LocalRunContractError("baseline checkout is not the frozen F0 source")
    if sources["baseline"] == sources["candidate"]:
        raise LocalRunContractError("candidate source identity is vacuously equal to F0")
    plugins = {
        "baseline": baseline_plugin.resolve(),
        "candidate": candidate_plugin.resolve(),
    }
    identities = {
        arm: {"source": sources[arm], "artifact": _artifact(plugins[arm], sources[arm])}
        for arm in ("baseline", "candidate")
    }
    expected = next(
        item
        for item in manifest["baseline_candidate"]["artifacts"]
        if item["platform"] == PLATFORM
    )
    expected_artifact = {field: expected[field] for field in evaluator.ARTIFACT_FIELDS}
    if identities["baseline"]["artifact"] != expected_artifact:
        raise LocalRunContractError("baseline plugin does not equal the frozen macOS F0 artifact")
    packages = {
        "baseline": stage_package(
            baseline_source, plugins["baseline"], run_root / "bundles/baseline"
        ),
        "candidate": stage_package(
            candidate_source, plugins["candidate"], run_root / "bundles/candidate"
        ),
    }
    return identities, packages


def _rss_bytes(pid: int) -> int | None:
    completed = subprocess.run(
        ["ps", "-o", "rss=", "-p", str(pid)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    value = completed.stdout.strip()
    if completed.returncode != 0 or not value.isdigit():
        return None
    return int(value) * 1024


def execute_arm(
    stata: str,
    spec: capture.ArmCaptureSpec,
    *,
    timeout_seconds: float,
    poll_seconds: float = 0.01,
) -> ArmRun:
    """Execute one rendered driver in a fresh process and sample child RSS."""
    driver = capture.write_arm_do(spec)
    stdout_path = spec.output_dir / "process.stdout"
    stderr_path = spec.output_dir / "process.stderr"
    started = time.monotonic()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        try:
            process = subprocess.Popen(
                [stata, "-b", "do", str(driver)],
                cwd=spec.output_dir,
                stdout=stdout,
                stderr=stderr,
            )
        except OSError as error:
            raise LocalRunContractError(f"cannot start licensed Stata: {error}") from error
        saw_ready = False
        saw_start = False
        saw_done = False
        peak = 0
        while process.poll() is None:
            ready = (spec.output_dir / "READY").is_file()
            start = (spec.output_dir / "START").is_file()
            done = (spec.output_dir / "DONE").is_file()
            if start and not ready:
                process.kill()
                process.wait()
                raise LocalRunContractError("Stata child wrote START before READY")
            if done and not start:
                process.kill()
                process.wait()
                raise LocalRunContractError("Stata child wrote DONE before START")
            saw_ready = saw_ready or ready
            saw_start = saw_start or start
            if start and not done:
                observed = _rss_bytes(process.pid)
                if observed is not None:
                    peak = max(peak, observed)
            saw_done = saw_done or done
            if time.monotonic() - started > timeout_seconds:
                process.kill()
                process.wait()
                raise LocalRunContractError("licensed Stata arm timed out")
            time.sleep(poll_seconds)
        return_code = process.wait()
    saw_ready = saw_ready or (spec.output_dir / "READY").is_file()
    saw_start = saw_start or (spec.output_dir / "START").is_file()
    saw_done = saw_done or (spec.output_dir / "DONE").is_file()
    if return_code != 0:
        raise LocalRunContractError(
            f"licensed Stata arm failed rc={return_code}: {spec.case_id}/{spec.arm}"
        )
    if not (saw_ready and saw_start and saw_done):
        raise LocalRunContractError("licensed Stata child omitted an observed RSS phase")
    if peak <= 0:
        raise LocalRunContractError("licensed Stata child yielded no in-window RSS sample")
    log_path = spec.output_dir / "run.log"
    if not log_path.is_file():
        logs = sorted(spec.output_dir.glob("*.log"))
        if len(logs) != 1:
            raise LocalRunContractError("licensed Stata child produced no unique batch log")
        log_path = logs[0]
    log_text = log_path.read_text(encoding="utf-8", errors="replace")
    return ArmRun(
        result=capture.load_completed_capture(spec, log_text),
        peak_rss_bytes=peak,
    )


def _matrix_value(result: capture.ArmCaptureResult, name: str, row: int, column: int) -> float:
    cells = result.raw["matrix_cells"]["science"].get(name, [])
    selected = [cell[4] for cell in cells if cell[0] == row and cell[1] == column]
    if len(selected) != 1:
        raise LocalRunContractError(f"science capture lacks {name}[{row},{column}]")
    return capture.stata_hex_to_float(selected[0])


def _integer_value(value: float, where: str) -> int:
    if not math.isfinite(value) or value < 0 or value != round(value):
        raise LocalRunContractError(f"{where} is not a nonnegative integer")
    return round(value)


def _matrix_column_sum(result: capture.ArmCaptureResult, name: str, column: int) -> int:
    cells = result.raw["matrix_cells"]["science"].get(name, [])
    return sum(
        _integer_value(capture.stata_hex_to_float(cell[4]), f"{name}[{cell[0]},{column}]")
        for cell in cells
        if cell[1] == column
    )


def _memory_record(
    preflight: capture.ArmCaptureResult,
    retained_bytes: int,
) -> dict[str, Any]:
    scalars = preflight.raw["scalars"]["science"]
    if "N" not in scalars:
        raise LocalRunContractError("science capture lacks e(N) for memory accounting")
    observations = _integer_value(capture.stata_hex_to_float(scalars["N"]), "e(N)")
    return {
        "native_allocation_bytes": _integer_value(
            _matrix_value(preflight, "fereg_runtime", 13, 2),
            "fereg_runtime native allocation",
        ),
        "allocation_classes": {
            "observations": observations,
            "observation_columns": len(preflight.scientific["coefficient_names"]),
            "fe_levels": _matrix_column_sum(preflight, "fereg_fe", 4),
            "clusters": _matrix_column_sum(preflight, "fereg_cluster", 3),
        },
        "repeated_call_retained_bytes": retained_bytes,
        "constrained_memory": None,
    }


def _arm_spec(
    *,
    arm: str,
    cell: CellInput,
    source: Mapping[str, Any],
    artifact: Mapping[str, Any],
    package: Path,
    manifest_sha256: str,
    obs_id: str,
    output_dir: Path,
    registered: Mapping[str, Any],
) -> capture.ArmCaptureSpec:
    return capture.ArmCaptureSpec(
        profile=PROFILE,
        arm=arm,
        case_id=cell.cell_id,
        manifest_sha256=manifest_sha256,
        source_commit=str(source["commit"]),
        package_root=package,
        plugin_filename=PLUGIN_FILENAME,
        plugin_sha256=str(artifact["sha256"]),
        dataset_path=cell.dataset.path,
        dataset_sha256=cell.dataset.sha256,
        expected_observations=cell.dataset.observations,
        command=cell.command,
        requested_threads=int(registered["threads"]),
        output_dir=output_dir,
        saved_outputs=cell.saved_outputs,
        obs_id=obs_id,
    )


def _confirmation_required(pairs: Sequence[Mapping[str, Any]]) -> bool:
    logs = [
        math.log(float(pair["candidate"]["seconds"]))
        - math.log(float(pair["baseline"]["seconds"]))
        for pair in pairs
    ]
    mean = sum(logs) / len(logs)
    variance = sum((value - mean) ** 2 for value in logs) / (len(logs) - 1)
    half_width = 2.306004135 * math.sqrt(variance / len(logs))
    return math.exp(mean) > 1.05 or mean - half_width > 0.0


def _same_stata_environment(left: Mapping[str, str], right: Mapping[str, str]) -> None:
    if left != right:
        raise LocalRunContractError("Stata environment changed between arms or runs")


def _same_stata_host(left: Mapping[str, str], right: Mapping[str, str]) -> None:
    stable = {"stata_version", "stata_flavor", "os", "machine_type"}
    if {key: left.get(key) for key in stable} != {
        key: right.get(key) for key in stable
    }:
        raise LocalRunContractError("Stata host identity changed between cells")


def _bind_input_contract(
    result: capture.ArmCaptureResult,
    *,
    cell: CellInput,
    manifest_sha256: str,
    input_sha256: str,
    preparation_sha256: str,
    obs_id: str,
) -> None:
    """Retain the exact local input/config identity in evaluated science."""
    payload = {
        "cell_id": cell.cell_id,
        "manifest_sha256": manifest_sha256,
        "input_config_sha256": input_sha256,
        "preparation_receipt_sha256": preparation_sha256,
        "dataset_sha256": cell.dataset.sha256,
        "dataset_observations": cell.dataset.observations,
        "command_sha256": hashlib.sha256(cell.command.encode("utf-8")).hexdigest(),
        "saved_outputs": list(cell.saved_outputs),
        "obs_id": obs_id,
    }
    certificates = result.scientific.get("certificates")
    if not isinstance(certificates, dict) or not isinstance(certificates.get("digests"), dict):
        raise LocalRunContractError("scientific record cannot retain its input binding")
    certificates["digests"].update(
        {
            "local_manifest": manifest_sha256,
            "local_input_config": input_sha256,
            "local_preparation_receipt": preparation_sha256,
            "local_dataset": cell.dataset.sha256,
            "local_command": payload["command_sha256"],
            "local_input_contract": hashlib.sha256(
                evaluator.canonical_json_bytes(payload)
            ).hexdigest(),
        }
    )


def run_cell(
    *,
    cell: CellInput,
    registered: Mapping[str, Any],
    manifest: Mapping[str, Any],
    manifest_sha256: str,
    input_sha256: str,
    preparation_sha256: str,
    identities: Mapping[str, Mapping[str, Any]],
    packages: Mapping[str, Path],
    obs_id: str,
    cell_root: Path,
    stata: str,
    timeout_seconds: float,
    runner: Callable[..., ArmRun] = execute_arm,
) -> tuple[dict[str, Any], Mapping[str, str]]:
    """Preflight one cell, then run its exact alternating pair schedule."""
    preflight: dict[str, ArmRun] = {}
    for arm in ("baseline", "candidate"):
        spec = _arm_spec(
            arm=arm,
            cell=cell,
            source=identities[arm]["source"],
            artifact=identities[arm]["artifact"],
            package=packages[arm],
            manifest_sha256=manifest_sha256,
            obs_id=obs_id,
            output_dir=cell_root / "preflight" / arm,
            registered=registered,
        )
        preflight[arm] = runner(stata, spec, timeout_seconds=timeout_seconds)
        _bind_input_contract(
            preflight[arm].result,
            cell=cell,
            manifest_sha256=manifest_sha256,
            input_sha256=input_sha256,
            preparation_sha256=preparation_sha256,
            obs_id=obs_id,
        )
    _same_stata_environment(
        preflight["baseline"].result.environment,
        preflight["candidate"].result.environment,
    )
    mismatches = evaluator.compare_scientific(
        manifest,
        preflight["baseline"].result.scientific,
        preflight["candidate"].result.scientific,
    )
    if mismatches:
        raise LocalRunContractError(
            f"scientific preflight failed before timing for {cell.cell_id}: "
            + ",".join(mismatches)
        )

    retained = {"baseline": 0, "candidate": 0}

    def run_pairs(label: str) -> list[dict[str, Any]]:
        pairs: list[dict[str, Any]] = []
        for index, order in enumerate(PAIR_ORDERS, 1):
            measurements: dict[str, Any] = {}
            arms = ("baseline", "candidate") if order == "AB" else ("candidate", "baseline")
            for arm in arms:
                spec = _arm_spec(
                    arm=arm,
                    cell=cell,
                    source=identities[arm]["source"],
                    artifact=identities[arm]["artifact"],
                    package=packages[arm],
                    manifest_sha256=manifest_sha256,
                    obs_id=obs_id,
                    output_dir=cell_root / label / f"pair-{index:02d}-{arm}",
                    registered=registered,
                )
                observed = runner(stata, spec, timeout_seconds=timeout_seconds)
                _bind_input_contract(
                    observed.result,
                    cell=cell,
                    manifest_sha256=manifest_sha256,
                    input_sha256=input_sha256,
                    preparation_sha256=preparation_sha256,
                    obs_id=obs_id,
                )
                _same_stata_environment(
                    preflight[arm].result.environment, observed.result.environment
                )
                drift = evaluator.compare_scientific(
                    manifest,
                    preflight[arm].result.scientific,
                    observed.result.scientific,
                )
                if drift:
                    raise LocalRunContractError(
                        f"{arm} science changed during {cell.cell_id}: " + ",".join(drift)
                    )
                before = _integer_value(
                    _matrix_value(observed.result, "fereg_runtime", 10, 2),
                    "runtime retained-before",
                )
                after = _integer_value(
                    _matrix_value(observed.result, "fereg_runtime", 11, 2),
                    "runtime retained-after",
                )
                retained[arm] = max(retained[arm], max(0, after - before))
                measurements[arm] = {
                    "seconds": observed.result.seconds,
                    "peak_rss_bytes": observed.peak_rss_bytes,
                }
            pairs.append({"index": index, "order": order, **measurements})
        return pairs

    pairs = run_pairs("pairs")
    confirmation = run_pairs("confirmation") if _confirmation_required(pairs) else None
    evidence = {
        "cell_id": cell.cell_id,
        "environment": None,
        "scientific": {
            arm: preflight[arm].result.scientific for arm in ("baseline", "candidate")
        },
        "pairs": pairs,
        "confirmation_pairs": confirmation,
        "memory": {
            arm: _memory_record(preflight[arm].result, retained[arm])
            for arm in ("baseline", "candidate")
        },
    }
    return evidence, preflight["baseline"].result.environment


def _sysctl(name: str) -> str:
    completed = subprocess.run(
        ["sysctl", "-n", name],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    value = completed.stdout.strip()
    if completed.returncode != 0 or not value:
        raise LocalRunContractError(f"cannot capture local host identity {name}")
    return value


def local_environment(stata_environment: Mapping[str, str], toolchain: str) -> dict[str, Any]:
    if sys.platform != "darwin":
        raise LocalRunContractError("local_screen is bound to macOS universal artifacts")
    if not toolchain.strip():
        raise LocalRunContractError("toolchain identity must be nonempty")
    return {
        "host_id": socket.gethostname(),
        "os": platform.platform(),
        "arch": platform.machine(),
        "cpu": _sysctl("machdep.cpu.brand_string"),
        "stata_version": stata_environment["stata_version"],
        "stata_flavor": stata_environment["stata_flavor"],
        "toolchain": toolchain,
        "physical_memory_bytes": _positive_integer(
            int(_sysctl("hw.memsize")), "physical memory"
        ),
    }


def _write_new(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise LocalRunContractError(f"refusing to overwrite output {path}") from error
    with os.fdopen(descriptor, "wb") as output:
        output.write(data)
        output.flush()
        os.fsync(output.fileno())


def run_local(args: argparse.Namespace) -> dict[str, Any]:
    manifest_path = args.manifest.resolve()
    manifest = evaluator.load_manifest(manifest_path)
    manifest_sha256 = evaluator.sha256_file(manifest_path)
    input_path = args.input_config.resolve()
    inputs = load_local_inputs(input_path, manifest, manifest_sha256)
    preparation_sha256, preparation_stata = verify_preparation(
        args.preparation_receipt.resolve(),
        input_path,
        inputs,
        manifest,
        args.baseline_source.resolve(),
    )
    run_root = args.run_root.resolve()
    if run_root.exists():
        raise LocalRunContractError(f"refusing to reuse run root {run_root}")
    run_root.mkdir(parents=True)
    identities, packages = prepare_arms(
        manifest,
        args.baseline_source.resolve(),
        args.baseline_plugin.resolve(),
        args.candidate_source.resolve(),
        args.candidate_plugin.resolve(),
        run_root,
    )
    registered = {
        item["id"]: item for item in manifest["cells"] if item["profile"] == PROFILE
    }
    cell_evidence: list[dict[str, Any]] = []
    common_stata: Mapping[str, str] | None = None
    for cell in inputs.cells:
        result, stata_environment = run_cell(
            cell=cell,
            registered=registered[cell.cell_id],
            manifest=manifest,
            manifest_sha256=manifest_sha256,
            input_sha256=inputs.input_sha256,
            preparation_sha256=preparation_sha256,
            identities=identities,
            packages=packages,
            obs_id=inputs.obs_id,
            cell_root=run_root / "cells" / cell.cell_id,
            stata=args.stata,
            timeout_seconds=args.timeout_seconds,
        )
        if common_stata is None:
            common_stata = stata_environment
            _same_stata_host(preparation_stata, stata_environment)
        else:
            _same_stata_host(common_stata, stata_environment)
        result["environment"] = local_environment(stata_environment, args.toolchain)
        cell_evidence.append(result)
    evidence = {
        "schema_version": 1,
        "evidence_type": manifest["evidence_contract"]["evidence_type"],
        "profile": PROFILE,
        "manifest": {"id": manifest["manifest_id"], "sha256": manifest_sha256},
        "baseline": identities["baseline"],
        "candidate": identities["candidate"],
        "cells": cell_evidence,
    }
    evidence_path = args.evidence.resolve()
    receipt_path = args.receipt.resolve()
    identity_path = args.identity_output.resolve()
    for output in (evidence_path, receipt_path, identity_path):
        if output.exists():
            raise LocalRunContractError(f"refusing to overwrite output {output}")
    _write_new(evidence_path, evaluator.canonical_json_bytes(evidence))
    receipt = evaluator.build_receipt(
        manifest, evidence, manifest_sha256=manifest_sha256
    )
    evaluator.write_receipt(receipt_path, receipt, identity_path=identity_path)
    return receipt


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare", help="create and seal the four immutable datasets")
    prepare.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    prepare.add_argument("--baseline-source", type=Path, required=True)
    prepare.add_argument("--stata", required=True)
    prepare.add_argument("--output-root", type=Path, required=True)

    run = commands.add_parser("run", help="execute the closed 18-cell paired screen")
    run.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    run.add_argument("--input-config", type=Path, required=True)
    run.add_argument("--preparation-receipt", type=Path, required=True)
    run.add_argument("--baseline-source", type=Path, required=True)
    run.add_argument("--baseline-plugin", type=Path, required=True)
    run.add_argument("--candidate-source", type=Path, required=True)
    run.add_argument("--candidate-plugin", type=Path, required=True)
    run.add_argument("--stata", required=True)
    run.add_argument("--toolchain", required=True)
    run.add_argument("--run-root", type=Path, required=True)
    run.add_argument("--evidence", type=Path, required=True)
    run.add_argument("--receipt", type=Path, required=True)
    run.add_argument("--identity-output", type=Path, required=True)
    run.add_argument("--timeout-seconds", type=float, default=3600.0)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "prepare":
            receipt = prepare_local(args)
            print(
                json.dumps(
                    {
                        "status": "prepared",
                        "receipt": str((args.output_root / "preparation-receipt.json").resolve()),
                    },
                    sort_keys=True,
                )
            )
            return 0
        if not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0:
            raise LocalRunContractError("--timeout-seconds must be positive and finite")
        receipt = run_local(args)
    except (LocalRunContractError, capture.CaptureContractError, evaluator.ContractError) as error:
        raise SystemExit(f"fereg F0 local run failed closed: {error}") from error
    evaluation = receipt["payload"]["evaluation"]
    print(
        json.dumps(
            {
                "status": evaluation["status"],
                "stage": evaluation["stage"],
                "receipt": str(args.receipt.resolve()),
            },
            sort_keys=True,
        )
    )
    return 0 if evaluation["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
