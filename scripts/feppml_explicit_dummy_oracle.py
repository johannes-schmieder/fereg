#!/usr/bin/env python3
"""Independent explicit-dummy PPML oracle for bounded qualification fixtures.

The governing 100k/1m/10m cells remain the performance workloads.  The seven
registered oracle cells additionally carry a small, low-cardinality fixture
on which candidate, pinned comparator, and this program all run.  This module
never imports fereg production code and never consumes either implementation's
output.  Its only numerical dependency is the frozen dependency-free oracle in
``tests/python/ppml_explicit_dummy_oracle.py``.

Input data are a canonical CSV export of the exact small DTA used by both
licensed-Stata arms.  The closed model JSON declares finite columns, absorbed
intercept/slope channels, adjustment, weights, clusters, separation rule,
coefficient status map, and probes.  The output is the ordinary closed science
record consumed by ``ppml_memory_performance.py``.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
ORACLE_ROOT = ROOT / "tests/python"


def _load_exact_module(name: str, path: Path):
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load sealed oracle dependency: {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


_explicit = _load_exact_module(
    "ppml_explicit_dummy_oracle", ORACLE_ROOT / "ppml_explicit_dummy_oracle.py"
)
_multiway = _load_exact_module(
    "ppml_multiway_cluster_oracle", ORACLE_ROOT / "ppml_multiway_cluster_oracle.py"
)
fit_explicit_ppml = _explicit.fit_explicit_ppml
sandwich_covariance = _explicit.sandwich_covariance
explicit_multiway_cluster_covariance = _multiway.explicit_multiway_cluster_covariance
repair_multiway_psd = _multiway._repair_multiway_psd


MODEL_TYPE = "feppml-explicit-dummy-model-v1"
MODEL_FIELDS = {
    "schema_version",
    "model_type",
    "cell_id",
    "row_count",
    "outcome",
    "finite_terms",
    "fixed_effects",
    "adjustment",
    "weight",
    "clusters",
    "vce",
    "separation",
    "expected_status_map",
    "expected_output",
    "probe_count",
    "complete_sentinel",
}
STATUS = {
    1: "identified",
    2: "base_level",
    3: "syntax_omitted",
    4: "collinear_with_fixed_effects",
    5: "collinear_after_residualization",
    6: "numerically_rank_deficient",
    7: "normalized_structural_constant",
}


class OracleFailure(ValueError):
    """A fixture, mathematical certificate, or requested output is invalid."""


def canonical(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def digest_preimage(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def sample_mask_digest(mask: Sequence[int]) -> str:
    """Hash the frozen cross-estimator eligible/final-sample mask."""

    if not mask or any(value not in {0, 1, 2} for value in mask):
        raise OracleFailure("sample mask must use only frozen codes 0, 1, and 2")
    preimage = (
        b"FEPPML_SAMPLE_MASK_V1\0"
        + len(mask).to_bytes(8, "big", signed=False)
        + "".join(str(value) for value in mask).encode("ascii")
    )
    return hashlib.sha256(preimage).hexdigest()


def separation_mask_digest(mask: Sequence[int]) -> str:
    if not mask or any(value not in {0, 1, 2} for value in mask):
        raise OracleFailure("separation source mask is outside the frozen domain")
    separation = "".join("1" if value == 2 else "0" for value in mask).encode("ascii")
    preimage = (
        b"FEPPML_GENERIC_SEPARATION_MASK_V1\0"
        + len(mask).to_bytes(8, "big", signed=False)
        + separation
    )
    return hashlib.sha256(preimage).hexdigest()


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise OracleFailure(f"{where} must be an object")
    return value


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise OracleFailure(
            f"{where} is not closed; missing={sorted(fields-set(value))}, "
            f"extra={sorted(set(value)-fields)}"
        )


def _safe_name(value: Any, where: str) -> str:
    if not isinstance(value, str) or not value or value.strip() != value:
        raise OracleFailure(f"{where} must be a nonempty canonical name")
    return value


def load_model(path: Path) -> dict[str, Any]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise OracleFailure(f"cannot read oracle model: {error}") from error
    model = dict(_mapping(raw, "oracle model"))
    _closed(model, MODEL_FIELDS, "oracle model")
    if (
        model["schema_version"] != 1
        or model["model_type"] != MODEL_TYPE
        or model["complete_sentinel"] != "FEPPML_EXPLICIT_DUMMY_MODEL_COMPLETE"
    ):
        raise OracleFailure("oracle model identity/sentinel is invalid")
    _safe_name(model["cell_id"], "cell_id")
    if (
        isinstance(model["row_count"], bool)
        or not isinstance(model["row_count"], int)
        or not 8 <= model["row_count"] <= 512
    ):
        raise OracleFailure("row_count must be a bounded integer in [8, 512]")
    _safe_name(model["outcome"], "outcome")

    finite_terms = model["finite_terms"]
    if not isinstance(finite_terms, list) or not finite_terms:
        raise OracleFailure("finite_terms must be a nonempty ordered list")
    names: list[str] = []
    for index, raw_term in enumerate(finite_terms):
        term = _mapping(raw_term, f"finite_terms[{index}]")
        _closed(term, {"name", "column"}, f"finite_terms[{index}]")
        names.append(_safe_name(term["name"], f"finite_terms[{index}].name"))
        if term["column"] is not None:
            _safe_name(term["column"], f"finite_terms[{index}].column")
        elif term["name"] not in {"_cons", "o._cons"}:
            raise OracleFailure("only _cons may have a null source column")
    if len(set(names)) != len(names):
        raise OracleFailure("finite term names must be unique")

    statuses = model["expected_status_map"]
    if not isinstance(statuses, list) or len(statuses) != len(finite_terms):
        raise OracleFailure("expected_status_map must align with finite_terms")
    observed_status_names = []
    for index, raw_status in enumerate(statuses):
        status = _mapping(raw_status, f"expected_status_map[{index}]")
        _closed(status, {"name", "status_code"}, f"expected_status_map[{index}]")
        observed_status_names.append(status["name"])
        if status["status_code"] not in STATUS:
            raise OracleFailure("expected status code is outside the frozen domain")
    if observed_status_names != names:
        raise OracleFailure("expected_status_map order differs from finite_terms")

    effects = model["fixed_effects"]
    if not isinstance(effects, list) or len(effects) > 3:
        raise OracleFailure("fixed_effects must contain zero through three channels")
    for index, raw_effect in enumerate(effects):
        effect = _mapping(raw_effect, f"fixed_effects[{index}]")
        _closed(effect, {"column", "slope"}, f"fixed_effects[{index}]")
        _safe_name(effect["column"], f"fixed_effects[{index}].column")
        if effect["slope"] is not None:
            _safe_name(effect["slope"], f"fixed_effects[{index}].slope")

    adjustment = _mapping(model["adjustment"], "adjustment")
    _closed(adjustment, {"kind", "column"}, "adjustment")
    if adjustment["kind"] not in {"none", "offset", "exposure"}:
        raise OracleFailure("adjustment.kind is invalid")
    if (adjustment["kind"] == "none") != (adjustment["column"] is None):
        raise OracleFailure("adjustment column/kind are inconsistent")
    if adjustment["column"] is not None:
        _safe_name(adjustment["column"], "adjustment.column")

    weight = _mapping(model["weight"], "weight")
    _closed(weight, {"kind", "column"}, "weight")
    if weight["kind"] not in {"none", "fweight", "pweight"}:
        raise OracleFailure("weight.kind is invalid")
    if (weight["kind"] == "none") != (weight["column"] is None):
        raise OracleFailure("weight column/kind are inconsistent")
    if weight["column"] is not None:
        _safe_name(weight["column"], "weight.column")

    clusters = model["clusters"]
    if not isinstance(clusters, list) or len(clusters) > 5:
        raise OracleFailure("clusters must contain zero through five dimensions")
    if len(set(clusters)) != len(clusters):
        raise OracleFailure("cluster columns must be unique")
    for index, column in enumerate(clusters):
        _safe_name(column, f"clusters[{index}]")
    if model["vce"] not in {"robust", "cluster"}:
        raise OracleFailure("vce must be robust or cluster")
    if (model["vce"] == "cluster") != bool(clusters):
        raise OracleFailure("cluster VCE and cluster columns are inconsistent")

    separation = _mapping(model["separation"], "separation")
    _closed(separation, {"kind", "fe_columns", "reason_code"}, "separation")
    if separation["kind"] not in {"none", "zero_fe_levels"}:
        raise OracleFailure("separation.kind is invalid")
    if not isinstance(separation["fe_columns"], list):
        raise OracleFailure("separation.fe_columns must be a list")
    for index, column in enumerate(separation["fe_columns"]):
        _safe_name(column, f"separation.fe_columns[{index}]")
    if separation["kind"] == "none" and separation["fe_columns"]:
        raise OracleFailure("none separation cannot name FE columns")
    if separation["kind"] == "zero_fe_levels" and not separation["fe_columns"]:
        raise OracleFailure("zero-FE separation requires at least one FE column")
    if separation["reason_code"] != 1:
        raise OracleFailure("oracle normalized separation reason code must be frozen at 1")

    if model["expected_output"] not in {
        "fit", "prediction", "total_absorbed_effect", "individual_fe"
    }:
        raise OracleFailure("expected_output is invalid")
    probe_count = model["probe_count"]
    if (
        isinstance(probe_count, bool)
        or not isinstance(probe_count, int)
        or probe_count < 0
        or probe_count > model["row_count"]
    ):
        raise OracleFailure("probe_count must be a bounded nonnegative integer")
    if model["expected_output"] == "fit" and probe_count != 0:
        raise OracleFailure("fit-only oracle must not declare probes")
    if model["expected_output"] != "fit" and probe_count < 1:
        raise OracleFailure("requested oracle output requires probe rows")
    return model


def _numeric(value: str, where: str) -> float:
    try:
        result = float(value)
    except ValueError as error:
        raise OracleFailure(f"{where} is not numeric") from error
    if not math.isfinite(result):
        raise OracleFailure(f"{where} must be finite")
    return result


def load_rows(path: Path, model: Mapping[str, Any]) -> list[dict[str, str]]:
    try:
        with path.open("r", encoding="ascii", newline="") as source:
            reader = csv.DictReader(source)
            if reader.fieldnames is None or len(reader.fieldnames) != len(set(reader.fieldnames)):
                raise OracleFailure("canonical export has an invalid header")
            rows = [dict(row) for row in reader]
    except OSError as error:
        raise OracleFailure(f"cannot read canonical export: {error}") from error
    if len(rows) != model["row_count"]:
        raise OracleFailure("canonical export row count differs from the closed model")
    if "obs_id" not in reader.fieldnames:
        raise OracleFailure("canonical export omits obs_id")
    obs_ids = []
    for index, row in enumerate(rows):
        if set(row) != set(reader.fieldnames) or any(value is None or value == "" for value in row.values()):
            raise OracleFailure(f"canonical export row {index} is ragged or missing")
        value = _numeric(row["obs_id"], f"row {index}.obs_id")
        if value != int(value) or value < 1:
            raise OracleFailure("obs_id must be a positive integer")
        obs_ids.append(int(value))
    if obs_ids != sorted(obs_ids) or len(obs_ids) != len(set(obs_ids)):
        raise OracleFailure("canonical export must have unique increasing obs_id")
    return rows


def _levels(rows: Sequence[Mapping[str, str]], column: str) -> list[str]:
    return list(dict.fromkeys(row[column] for row in rows))


def _separated_indices(rows: Sequence[Mapping[str, str]], model: Mapping[str, Any]) -> set[int]:
    rule = model["separation"]
    if rule["kind"] == "none":
        return set()
    outcome = model["outcome"]
    active = set(range(len(rows)))
    separated: set[int] = set()
    while True:
        newly: set[int] = set()
        for column in rule["fe_columns"]:
            has_positive: dict[str, bool] = {}
            for index in active:
                level = rows[index][column]
                has_positive[level] = has_positive.get(level, False) or _numeric(
                    rows[index][outcome], f"row {index}.{outcome}"
                ) > 0.0
            zero_levels = {level for level, positive in has_positive.items() if not positive}
            newly.update(index for index in active if rows[index][column] in zero_levels)
        if not newly:
            break
        active.difference_update(newly)
        separated.update(newly)
    if not active:
        raise OracleFailure("separation removed the entire oracle fixture")
    return separated


def _append_column(design: list[list[float]], values: Sequence[float]) -> int:
    if len(design) != len(values):
        raise OracleFailure("oracle design column length mismatch")
    index = len(design[0]) if design and design[0] else 0
    for row, value in zip(design, values):
        row.append(float(value))
    return index


def _explicit_design(
    rows: Sequence[Mapping[str, str]], model: Mapping[str, Any]
) -> tuple[list[list[float]], list[int | None], list[list[int]]]:
    design: list[list[float]] = [[] for _ in rows]
    reported: list[int | None] = []
    for term, status in zip(model["finite_terms"], model["expected_status_map"]):
        if status["status_code"] == 7:
            reported.append(None)
            continue
        if term["column"] is None:
            values = [1.0] * len(rows)
        else:
            values = [
                _numeric(row[term["column"]], f"{term['column']}") for row in rows
            ]
        reported.append(_append_column(design, values))

    channel_columns: list[list[int]] = []
    for effect_index, effect in enumerate(model["fixed_effects"]):
        intercept_columns: list[int] = []
        if effect_index == 0:
            # The absorbed span owns the structural intercept. Attribute its
            # reference-coded contribution to the anchor intercept channel.
            intercept_columns.append(_append_column(design, [1.0] * len(rows)))
        levels = _levels(rows, effect["column"])
        for level in levels[1:]:
            intercept_columns.append(
                _append_column(
                    design, [float(row[effect["column"]] == level) for row in rows]
                )
            )
        channel_columns.append(intercept_columns)
        if effect["slope"] is not None:
            slope_columns: list[int] = []
            for level in levels:
                slope_columns.append(
                    _append_column(
                        design,
                        [
                            _numeric(row[effect["slope"]], effect["slope"])
                            if row[effect["column"]] == level
                            else 0.0
                            for row in rows
                        ],
                    )
                )
            channel_columns.append(slope_columns)
    if not design or not design[0]:
        raise OracleFailure("oracle design has no columns")
    return design, reported, channel_columns


def _symmetry_error(matrix: Sequence[Sequence[float]]) -> float:
    return max(
        (abs(matrix[row][column] - matrix[column][row])
         for row in range(len(matrix)) for column in range(len(matrix))),
        default=0.0,
    )


def evaluate(data_path: Path, model_path: Path) -> dict[str, Any]:
    model = load_model(model_path)
    all_rows = load_rows(data_path, model)
    separated = _separated_indices(all_rows, model)
    rows = [row for index, row in enumerate(all_rows) if index not in separated]
    obs_ids = [int(_numeric(row["obs_id"], "obs_id")) for row in all_rows]
    # Every row in a closed oracle fixture is eligible.  Code 1 is retained;
    # code 2 is eligible but removed by the generic normalized separation
    # decision.  Ineligible code 0 is reserved for full-data capture masks.
    sample_mask = [
        2 if index in separated else 1 for index in range(len(all_rows))
    ]

    outcome = [
        _numeric(row[model["outcome"]], model["outcome"]) for row in rows
    ]
    if any(value < 0.0 for value in outcome):
        raise OracleFailure("oracle outcome must be nonnegative")
    adjustment = model["adjustment"]
    offset = None
    exposure = None
    if adjustment["kind"] == "offset":
        offset = [_numeric(row[adjustment["column"]], "offset") for row in rows]
    elif adjustment["kind"] == "exposure":
        exposure = [_numeric(row[adjustment["column"]], "exposure") for row in rows]
        if any(value <= 0.0 for value in exposure):
            raise OracleFailure("oracle exposure must be positive")

    weight = model["weight"]
    raw_weights = (
        None
        if weight["kind"] == "none"
        else [_numeric(row[weight["column"]], weight["column"]) for row in rows]
    )
    if raw_weights is None:
        weights = [1.0] * len(rows)
        oracle_weights = None
        effective_n = len(rows)
    elif weight["kind"] == "pweight":
        if any(value <= 0.0 for value in raw_weights):
            raise OracleFailure("pweights must be positive")
        scale = len(rows) / sum(raw_weights)
        weights = [value * scale for value in raw_weights]
        oracle_weights = weights
        effective_n = len(rows)
    else:
        if any(value <= 0.0 or value != math.floor(value) for value in raw_weights):
            raise OracleFailure("fweights must be positive integers")
        weights = raw_weights
        oracle_weights = weights
        effective_n = int(sum(weights))

    design, reported_columns, channel_columns = _explicit_design(rows, model)
    fit = fit_explicit_ppml(
        design,
        outcome,
        offset=offset,
        exposure=exposure,
        weights=oracle_weights,
        weight_kind=weight["kind"],
        vce="iid",
        tolerance=1e-12,
        max_iterations=1000,
    )
    kept_positions = {column: index for index, column in enumerate(fit.kept_columns)}
    for column, status in zip(reported_columns, model["expected_status_map"]):
        identified = column is not None and column in kept_positions
        if (status["status_code"] == 1) != identified:
            raise OracleFailure(
                f"explicit rank contradicts frozen status for {status['name']}"
            )

    compact_design = [
        [row[column] for column in fit.kept_columns] for row in design
    ]
    if model["vce"] == "robust":
        correction = effective_n / (effective_n - 1)
        compact_vce = sandwich_covariance(
            compact_design,
            outcome,
            fit.mean,
            weights,
            weight_kind=weight["kind"],
            correction=correction,
        )
        cluster_counts: list[int] = []
    else:
        dimensions = [
            [row[column] for row in rows] for column in model["clusters"]
        ]
        cluster_counts = [len(set(axis)) for axis in dimensions]
        if any(groups < 2 for groups in cluster_counts):
            raise OracleFailure("cluster oracle has fewer than two groups")
        correction = min(cluster_counts) / (min(cluster_counts) - 1)
        compact_vce, _ = explicit_multiway_cluster_covariance(
            compact_design,
            outcome,
            fit.mean,
            fit.hessian,
            weights,
            dimensions,
            correction=correction,
        )

    names = [record["name"] for record in model["expected_status_map"]]
    statuses = [STATUS[record["status_code"]] for record in model["expected_status_map"]]
    coefficients = []
    for column, status in zip(reported_columns, model["expected_status_map"]):
        coefficients.append(
            fit.compact_coefficients[kept_positions[column]]
            if status["status_code"] == 1 and column is not None
            else 0.0
        )
    vce = [[0.0] * len(names) for _ in names]
    for left, left_column in enumerate(reported_columns):
        if left_column not in kept_positions:
            continue
        for right, right_column in enumerate(reported_columns):
            if right_column in kept_positions:
                vce[left][right] = compact_vce[
                    kept_positions[left_column]
                ][kept_positions[right_column]]
    if len(cluster_counts) > 1:
        identified = [
            index for index, (column, status) in enumerate(
                zip(reported_columns, model["expected_status_map"])
            )
            if status["status_code"] == 1 and column in kept_positions
        ]
        reported_block = [[vce[row][column] for column in identified] for row in identified]
        repaired_block = repair_multiway_psd(reported_block)
        for block_row, row in enumerate(identified):
            for block_column, column in enumerate(identified):
                vce[row][column] = repaired_block[block_row][block_column]

    finite_rank = sum(record["status_code"] in {1, 7} for record in model["expected_status_map"])
    absorbed_rank = len(fit.kept_columns) - finite_rank
    if absorbed_rank < 0:
        raise OracleFailure("explicit rank cannot support the reported finite rank")
    structural_df = effective_n - finite_rank - absorbed_rank
    reported_df = min(structural_df, min(cluster_counts) - 1) if cluster_counts else structural_df
    if reported_df <= 0:
        raise OracleFailure("oracle fixture has nonpositive residual DoF")

    log_factorial = sum(
        weight_value * math.lgamma(y_value + 1.0)
        for y_value, weight_value in zip(outcome, weights)
    )
    log_likelihood = fit.log_likelihood_kernel - log_factorial
    null_fit = fit_explicit_ppml(
        [[1.0] for _ in rows],
        outcome,
        # ADR 0016 freezes the null comparator as a constant-only likelihood.
        # The fitted model's offset/exposure is deliberately excluded.
        offset=None,
        exposure=None,
        weights=oracle_weights,
        weight_kind=weight["kind"],
        vce="iid",
        tolerance=1e-12,
        max_iterations=1000,
    )
    null_log_likelihood = null_fit.log_likelihood_kernel - log_factorial

    retained_ids = [int(_numeric(row["obs_id"], "obs_id")) for row in rows]
    output = model["expected_output"]
    outputs: dict[str, list[float] | None] = {
        "prediction_probe": None,
        "total_absorbed_effect_probe": None,
        "individual_fe_probe": None,
    }
    probe_count = min(len(rows), model["probe_count"])
    probes = (
        []
        if probe_count == 0
        else [
            0
            if probe_count == 1
            else math.floor(index * (len(rows) - 1) / (probe_count - 1))
            for index in range(probe_count)
        ]
    )
    if output == "prediction":
        outputs["prediction_probe"] = [fit.mean[row] for row in probes]
    elif output in {"total_absorbed_effect", "individual_fe"}:
        finite_prediction = []
        for row_index, row in enumerate(design):
            total = 0.0
            for column, status in zip(reported_columns, model["expected_status_map"]):
                if status["status_code"] == 1 and column is not None:
                    total += row[column] * fit.compact_coefficients[kept_positions[column]]
            finite_prediction.append(total)
        total_effect = [
            fit.eta[index]
            - (0.0 if offset is None and exposure is None else (
                offset[index] if offset is not None else math.log(exposure[index])
            ))
            - finite_prediction[index]
            for index in range(len(rows))
        ]
        if output == "total_absorbed_effect":
            outputs["total_absorbed_effect_probe"] = [total_effect[row] for row in probes]
        else:
            coefficient_by_column = {
                column: fit.compact_coefficients[position]
                for position, column in enumerate(fit.kept_columns)
            }
            channels = [
                [
                    sum(design[row][column] * coefficient_by_column.get(column, 0.0) for column in columns)
                    for row in range(len(rows))
                ]
                for columns in channel_columns
            ]
            # Match native recovery: among intercept channels, center every
            # non-anchor channel by final q*mu within each connected component
            # and transfer that mean to the anchor. Slope-only channels are
            # already identified conditional on their intercept channel.
            intercept_channels = []
            cursor = 0
            for effect in model["fixed_effects"]:
                intercept_channels.append(cursor)
                cursor += 1 + int(effect["slope"] is not None)
            if len(intercept_channels) > 1:
                parent = list(range(len(rows)))
                def find(node: int) -> int:
                    while parent[node] != node:
                        parent[node] = parent[parent[node]]
                        node = parent[node]
                    return node
                def union(left: int, right: int) -> None:
                    left, right = find(left), find(right)
                    if left != right:
                        parent[right] = left
                for effect in model["fixed_effects"]:
                    first: dict[str, int] = {}
                    for row, source in enumerate(rows):
                        level = source[effect["column"]]
                        if level in first:
                            union(row, first[level])
                        else:
                            first[level] = row
                component_rows: dict[int, list[int]] = {}
                for row in range(len(rows)):
                    component_rows.setdefault(find(row), []).append(row)
                hessian_weights = [weight_value * mu for weight_value, mu in zip(weights, fit.mean)]
                anchor = channels[intercept_channels[0]]
                for channel_index in intercept_channels[1:]:
                    channel = channels[channel_index]
                    for members in component_rows.values():
                        denominator = sum(hessian_weights[row] for row in members)
                        mean = sum(hessian_weights[row] * channel[row] for row in members) / denominator
                        for row in members:
                            channel[row] -= mean
                            anchor[row] += mean
            outputs["individual_fe_probe"] = [
                channels[channel][row]
                for channel in range(len(channels))
                for row in probes
            ]

    full_vce = [value for row in vce for value in row]
    reconstruction = max(
        (
            abs(
                fit.eta[index]
                - (0.0 if offset is None and exposure is None else (
                    offset[index] if offset is not None else math.log(exposure[index])
                ))
                - sum(
                    design[index][column] * fit.compact_coefficients[position]
                    for position, column in enumerate(fit.kept_columns)
                )
            )
            for index in range(len(rows))
        ),
        default=0.0,
    )
    score_scale = max(1.0, sum(weights))
    score_linf = max((abs(value) for value in fit.score), default=0.0) / score_scale
    return {
        "rc": 0,
        "status": "success",
        "sample_identity_sha256": sample_mask_digest(sample_mask),
        "separation_ledger_sha256": separation_mask_digest(sample_mask),
        "coefficient_names": names,
        "coefficient_status": statuses,
        "coefficients": coefficients,
        "vce_shape": [len(names), len(names)],
        "vce": full_vce,
        "dof": {
            "N_physical": len(all_rows),
            "N_effective": (
                int(sum(_numeric(row[model["weight"]["column"]], model["weight"]["column"]) for row in all_rows))
                if model["weight"]["kind"] == "fweight" else len(all_rows)
            ),
            "finite_rank": finite_rank,
            "absorbed_rank": absorbed_rank,
            "df": reported_df,
            "cluster_counts": cluster_counts,
        },
        "objective": {
            "deviance": fit.deviance,
            "log_likelihood": log_likelihood,
            "null_log_likelihood": null_log_likelihood,
            "pseudo_r2": 1.0 - log_likelihood / null_log_likelihood,
        },
        "outputs": outputs,
        "certificates": {
            "converged": True,
            "two_consecutive_iterations": True,
            "strict_final_solve": True,
            "rank_frozen": True,
            "score_linf": score_linf,
            "fe_score_linf": score_linf,
            "vce_symmetry_linf": _symmetry_error(vce),
            "reconstruction_linf": reconstruction,
        },
    }


def _write_new(path: Path, value: Mapping[str, Any]) -> None:
    data = json.dumps(
        value, sort_keys=True, indent=2, ensure_ascii=True, allow_nan=False
    ) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise OracleFailure(f"refusing to overwrite oracle output: {path}") from error
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        result = evaluate(args.data.resolve(), args.model.resolve())
        _write_new(args.output.resolve(), result)
    except (OracleFailure, ArithmeticError, ValueError) as error:
        raise SystemExit(f"feppml independent oracle failed closed: {error}") from error
    print("FEPPML_EXPLICIT_DUMMY_ORACLE_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
