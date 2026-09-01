#!/usr/bin/env python3
"""Compile the seven bounded independent-oracle fixtures.

These fixtures are linked scientific checks for the registered N=100k cells;
they do not replace those cells' full-size timing datasets.  Cardinalities are
kept deliberately small so an explicit dummy matrix is a genuine independent
oracle rather than another absorbed-estimation implementation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from typing import Any, Mapping, Sequence

try:
    from scripts import ppml_memory_performance as performance
except ImportError:
    import ppml_memory_performance as performance  # type: ignore[no-redef]


ORACLE_ROWS = 256
OUTPUT_PROBE_COUNT = 128
ORACLE_CELL_IDS = (
    "dev_n100k_no_fe_k0_t1",
    "dev_n100k_one_fe_offset_fw_c1_t2",
    "dev_n100k_lowmob_exposure_pw_c2_t4",
    "dev_n100k_three_fe_separated_c3_t8",
    "dev_n100k_slopes_collinear_c4_t16",
    "dev_n100k_disconnected_illcond_c5_t1",
    "scale_n100k_two_fe_fast_t8",
)


class FixtureCompileError(RuntimeError):
    pass


def _write_new(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise FixtureCompileError(f"refusing to overwrite oracle fixture source: {path}") from error
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(content)
        target.flush()
        os.fsync(target.fileno())


def status_map(cell: Mapping[str, Any]) -> list[dict[str, Any]]:
    records = []
    for index in range(1, int(cell["controls"]) + 1):
        code = 6 if cell["finite_design"] == "collinear" and index == 2 else 1
        records.append({"name": f"o.c{index}" if code != 1 else f"c{index}", "status_code": code})
    records.append(
        {
            "name": "_cons" if cell["design"] == "no_fe" else "o._cons",
            "status_code": 1 if cell["design"] == "no_fe" else 7,
        }
    )
    return records


def model(cell: Mapping[str, Any]) -> dict[str, Any]:
    adjustment = {"kind": "none", "column": None}
    if cell["outcome"] in {"easy_high_zero_offset", "long_offset"}:
        adjustment = {"kind": "offset", "column": "offset"}
    elif cell["outcome"] == "long_exposure":
        adjustment = {"kind": "exposure", "column": "exposure"}
    weight = {
        "kind": cell["weight"],
        "column": {"none": None, "fweight": "fw", "pweight": "pw"}[cell["weight"]],
    }
    effects = {
        "no_fe": [],
        "one_fe": [{"column": "fe1", "slope": None}],
        "two_fe_standard": [
            {"column": "fe1", "slope": None},
            {"column": "fe2", "slope": None},
        ],
        "two_fe_lowmob": [
            {"column": "fe1", "slope": None},
            {"column": "fe2", "slope": None},
        ],
        "two_fe_disconnected": [
            {"column": "fe1", "slope": None},
            {"column": "fe2", "slope": None},
        ],
        "three_fe_standard": [
            {"column": "fe1", "slope": None},
            {"column": "fe2", "slope": None},
            {"column": "fe3", "slope": None},
        ],
        "two_fe_slopes": [
            {"column": "fe1", "slope": "slope1"},
            {"column": "fe2", "slope": "slope2"},
        ],
    }[cell["design"]]
    separated = cell["outcome"] in {"separated_high_zero", "long_offset"}
    statuses = status_map(cell)
    return {
        "schema_version": 1,
        "model_type": "feppml-explicit-dummy-model-v1",
        "cell_id": cell["cell_id"],
        "row_count": ORACLE_ROWS,
        "outcome": "y",
        "finite_terms": [
            {
                "name": item["name"],
                "column": (
                    None
                    if item["name"] in {"_cons", "o._cons"}
                    else item["name"].removeprefix("o.")
                ),
            }
            for item in statuses
        ],
        "fixed_effects": effects,
        "adjustment": adjustment,
        "weight": weight,
        "clusters": [
            f"cluster{index}"
            for index in range(1, int(str(cell["vce"]).removeprefix("cluster")) + 1)
        ] if str(cell["vce"]).startswith("cluster") else [],
        "vce": "cluster" if str(cell["vce"]).startswith("cluster") else "robust",
        "separation": {
            "kind": "zero_fe_levels" if separated else "none",
            "fe_columns": ["fe1"] if separated else [],
            "reason_code": 1,
        },
        "expected_status_map": statuses,
        "expected_output": cell["output"],
        "probe_count": 0 if cell["output"] == "fit" else OUTPUT_PROBE_COUNT,
        "complete_sentinel": "FEPPML_EXPLICIT_DUMMY_MODEL_COMPLETE",
    }


def generator_do(cell: Mapping[str, Any], seed: int) -> str:
    controls = int(cell["controls"])
    lines = [
        "version 19.0",
        "clear all",
        "set more off",
        f"set seed {seed}",
        f"set obs {ORACLE_ROWS}",
        "generate long obs_id = _n",
        "generate long fe1 = 1 + mod(_n - 1, 6)",
        "generate long fe2 = 1 + mod(floor((_n - 1) / 3), 7)",
        "generate long fe3 = 1 + mod(floor((_n - 1) / 5), 5)",
        "generate long cluster1 = 1 + mod(_n - 1, 17)",
        "generate long cluster2 = 1 + mod(floor((_n - 1) / 2), 19)",
        "generate long cluster3 = 1 + mod(floor((_n - 1) / 3), 23)",
        "generate long cluster4 = 1 + mod(floor((_n - 1) / 5), 29)",
        "generate long cluster5 = 1 + mod(floor((_n - 1) / 7), 31)",
        "generate double slope1 = mod(37 * _n, 97) / 48 - 1",
        "generate double slope2 = mod(53 * _n, 89) / 44 - 1",
    ]
    if cell["design"] in {"two_fe_lowmob", "two_fe_slopes"}:
        lines.append("replace fe2 = 1 + mod(fe1 + mod(_n, 2), 7)")
    if cell["design"] == "two_fe_disconnected":
        lines.extend(
            [
                "replace fe1 = 1 + mod(_n - 1, 3) if _n <= 128",
                "replace fe2 = 1 + mod(floor((_n - 1) / 3), 3) if _n <= 128",
                "replace fe1 = 4 + mod(_n - 129, 3) if _n > 128",
                "replace fe2 = 4 + mod(floor((_n - 129) / 3), 4) if _n > 128",
            ]
        )
    for index in range(1, controls + 1):
        lines.append(
            f"generate double c{index} = mod({2 * index + 1} * _n, 1009) / 504 - 1"
        )
    if cell["finite_design"] == "collinear" and controls >= 2:
        lines.append("replace c2 = 2 * c1")
    if cell["finite_design"] == "ill_conditioned" and controls >= 2:
        lines.append("replace c2 = c1 + 1e-7 * (mod(_n, 7) - 3)")
    lines.extend(
        [
            "generate double offset = 0.02 * (mod(_n, 9) - 4)",
            "generate double exposure = exp(offset)",
            "generate long fw = 1 + mod(_n, 3)",
            "generate double pw = 0.5 + mod(_n, 11) / 5",
            "generate double __eta = 0.15 + 0.2 * slope1 - 0.1 * slope2",
        ]
    )
    if controls:
        lines.append("replace __eta = __eta + 0.08 * c1")
    if cell["design"] != "no_fe":
        lines.append("replace __eta = __eta + 0.04 * fe1")
    if cell["design"] in {
        "two_fe_standard", "two_fe_lowmob", "two_fe_disconnected",
        "three_fe_standard", "two_fe_slopes",
    }:
        lines.append("replace __eta = __eta - 0.03 * fe2")
    if cell["design"] == "three_fe_standard":
        lines.append("replace __eta = __eta + 0.02 * fe3")
    if cell["outcome"] in {"easy_high_zero_offset", "long_offset"}:
        lines.append("replace __eta = __eta + offset")
    elif cell["outcome"] == "long_exposure":
        lines.append("replace __eta = __eta + log(exposure)")
    lines.extend(
        [
            "generate double __mu = exp(__eta)",
            "generate double y = floor(__mu) + (mod(37 * _n, 100) < 100 * (__mu - floor(__mu)))",
        ]
    )
    zero_threshold = {
        "easy_low_zero": 10,
        "easy_high_zero_offset": 55,
        "long_exposure": 30,
        "separated_high_zero": 55,
        "long_offset": 40,
    }[cell["outcome"]]
    lines.append(f"replace y = 0 if mod(97 * _n, 100) < {zero_threshold}")
    separated = cell["outcome"] in {"separated_high_zero", "long_offset"}
    if separated:
        lines.append("replace y = 0 if fe1 == 1")
    effect_columns = {
        "no_fe": (),
        "one_fe": ("fe1",),
        "two_fe_standard": ("fe1", "fe2"),
        "two_fe_lowmob": ("fe1", "fe2"),
        "two_fe_disconnected": ("fe1", "fe2"),
        "three_fe_standard": ("fe1", "fe2", "fe3"),
        "two_fe_slopes": ("fe1", "fe2"),
    }[cell["design"]]
    for effect in effect_columns:
        lines.extend(
            [
                f"sort {effect} obs_id",
                f"by {effect}: replace y = max(y, 1) if _n == _N" + (" & fe1 != 1" if separated else ""),
            ]
        )
    lines.extend(
        [
            "drop __eta __mu",
            "compress",
            "sort obs_id",
            "save \"`1'\", replace",
            "format y slope1 slope2 offset exposure pw" + (" c*" if controls else "") + " %24.17g",
            "export delimited using \"`2'\", replace nolabel",
            'display as result "FEPPML_ORACLE_FIXTURE_COMPLETE"',
        ]
    )
    return "\n".join(lines) + "\n"


def compile_fixtures(manifest_path: Path, output: Path) -> Mapping[str, Any]:
    manifest = performance.load_manifest(manifest_path)
    indexed = {cell["cell_id"]: cell for cell in performance.expand_cells(manifest)}
    if not set(ORACLE_CELL_IDS) <= set(indexed):
        raise FixtureCompileError("development manifest omits a frozen oracle cell")
    records = []
    for cell_id in ORACLE_CELL_IDS:
        cell = indexed[cell_id]
        seed = 2_026_083_100 + int.from_bytes(
            hashlib.sha256((cell_id + ":oracle-v1").encode("ascii")).digest()[:4], "big"
        ) % 100_000_000
        root = Path("cells") / cell_id
        generator = root / "generate_oracle_fixture.do"
        model_path = root / "model.json"
        _write_new(output / generator, generator_do(cell, seed))
        _write_new(
            output / model_path,
            json.dumps(model(cell), sort_keys=True, indent=2, ensure_ascii=True) + "\n",
        )
        records.append(
            {
                "cell_id": cell_id,
                "fixture_id": f"{cell_id}__explicit_v1",
                "row_count": ORACLE_ROWS,
                "seed": seed,
                "generator": generator.as_posix(),
                "model": model_path.as_posix(),
            }
        )
    document = {
        "schema_version": 1,
        "fixture_type": "feppml-explicit-dummy-fixtures-v1",
        "development_manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        "oracle_cell_ids": list(ORACLE_CELL_IDS),
        "cells": records,
        "complete_sentinel": "FEPPML_ORACLE_FIXTURES_COMPLETE",
    }
    validate_compiled_fixtures(document, manifest_path, output)
    _write_new(
        output / "fixtures.json",
        json.dumps(document, sort_keys=True, indent=2, ensure_ascii=True) + "\n",
    )
    return document


def validate_compiled_fixtures(
    document: Mapping[str, Any], manifest_path: Path, generated_root: Path
) -> None:
    manifest = performance.load_manifest(manifest_path)
    indexed = {cell["cell_id"]: cell for cell in performance.expand_cells(manifest)}
    if document.get("oracle_cell_ids") != list(ORACLE_CELL_IDS):
        raise FixtureCompileError("oracle cell inventory/order differs from frozen registry")
    records = document.get("cells")
    if not isinstance(records, list) or len(records) != len(ORACLE_CELL_IDS):
        raise FixtureCompileError("oracle fixture record count is not exact")
    for record, cell_id in zip(records, ORACLE_CELL_IDS):
        cell = indexed[cell_id]
        seed = 2_026_083_100 + int.from_bytes(
            hashlib.sha256((cell_id + ":oracle-v1").encode("ascii")).digest()[:4], "big"
        ) % 100_000_000
        expected_fields = {
            "cell_id": cell_id, "fixture_id": f"{cell_id}__explicit_v1",
            "row_count": ORACLE_ROWS, "seed": seed,
            "generator": f"cells/{cell_id}/generate_oracle_fixture.do",
            "model": f"cells/{cell_id}/model.json",
        }
        if dict(record) != expected_fields:
            raise FixtureCompileError("oracle fixture identity/seed/path semantics differ")
        observed_model = json.loads((generated_root / record["model"]).read_text(encoding="ascii"))
        if observed_model != model(cell):
            raise FixtureCompileError("oracle model differs from exact expanded-cell semantics")
        observed_generator = (generated_root / record["generator"]).read_text(encoding="ascii")
        if observed_generator != generator_do(cell, seed):
            raise FixtureCompileError("oracle generator differs from exact expanded-cell semantics")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--development-manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.output.exists():
        raise SystemExit("oracle fixture output already exists")
    args.output.mkdir(parents=True)
    compile_fixtures(args.development_manifest.resolve(), args.output.resolve())
    print("FEPPML_ORACLE_FIXTURES_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
