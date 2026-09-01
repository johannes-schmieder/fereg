#!/usr/bin/env python3
"""Normalize closed raw Stata PPML exports into qualification science records.

Exact fields (sample/ledger identities, coefficient order/status, dimensions,
DoF and cluster counts) are never tolerance-normalized. Numeric b/V,
likelihood and probes remain unrounded IEEE values for the frozen evaluator.
"""
from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import re
from typing import Any, Mapping, Sequence


STATUS = {1: "identified", 2: "base_level", 3: "syntax_omitted", 4: "collinear_with_fixed_effects", 5: "collinear_after_residualization", 6: "numerically_rank_deficient", 7: "normalized_structural_constant"}
FIELDS = {"arm", "rc", "converged", "sample_identity", "separation_ledger", "coefficient_names", "coefficient_status_codes", "coefficients", "vce", "dof", "objective", "outputs", "certificates", "source_inventory", "complete_sentinel"}
GENERIC_SEPARATION_CODE = 1
SHA256 = re.compile(r"^[0-9a-f]{64}$")


class RawNormalizationError(ValueError):
    pass


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise RawNormalizationError(f"{where} must be an object")
    return value


def _finite(value: Any, where: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise RawNormalizationError(f"{where} must be finite numeric")
    return float(value)


def _identity_records(raw_sample: Any, raw_separation: Any) -> tuple[dict[str, Any], dict[str, Any]]:
    sample = dict(_mapping(raw_sample, "sample identity"))
    separation = dict(_mapping(raw_separation, "separation ledger"))
    if set(sample) != {
        "algorithm", "encoding", "row_count", "eligible_count", "retained_count",
        "separated_count", "sha256", "probes",
    }:
        raise RawNormalizationError("sample identity schema is incomplete")
    if set(separation) != {
        "algorithm", "encoding", "row_count", "separated_count",
        "generic_reason_code", "sha256", "probes",
    }:
        raise RawNormalizationError("separation ledger schema is incomplete")
    if (
        sample["algorithm"] != "sha256"
        or sample["encoding"] != "feppml-sample-mask-v1"
        or separation["algorithm"] != "sha256"
        or separation["encoding"] != "feppml-generic-separation-mask-v1"
        or separation["generic_reason_code"] != GENERIC_SEPARATION_CODE
        or not isinstance(sample["sha256"], str)
        or SHA256.fullmatch(sample["sha256"]) is None
        or not isinstance(separation["sha256"], str)
        or SHA256.fullmatch(separation["sha256"]) is None
    ):
        raise RawNormalizationError("sample/separation identity algorithm or digest is invalid")
    integer_fields = ("row_count", "eligible_count", "retained_count", "separated_count")
    for field in integer_fields:
        value = sample[field]
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise RawNormalizationError(f"sample identity {field} is invalid")
    if (
        sample["row_count"] < 1
        or sample["retained_count"] + sample["separated_count"] != sample["eligible_count"]
        or sample["eligible_count"] > sample["row_count"]
        or separation["row_count"] != sample["row_count"]
        or separation["separated_count"] != sample["separated_count"]
    ):
        raise RawNormalizationError("sample/separation identity counts disagree")
    sample_probes = sample["probes"]
    if not isinstance(sample_probes, list) or not sample_probes:
        raise RawNormalizationError("sample identity probes are absent")
    prior_id = 0
    for row in sample_probes:
        if (
            not isinstance(row, list)
            or len(row) != 4
            or any(isinstance(value, bool) or not isinstance(value, int) for value in row)
        ):
            raise RawNormalizationError("sample identity probe is invalid")
        obs_id, eligible, retained, reason = row
        if (
            not prior_id < obs_id <= sample["row_count"]
            or eligible not in (0, 1)
            or retained not in (0, 1)
            or retained > eligible
            or reason != (GENERIC_SEPARATION_CODE if eligible and not retained else 0)
        ):
            raise RawNormalizationError("sample identity probe is not normalized")
        prior_id = obs_id
    separation_probes = separation["probes"]
    if not isinstance(separation_probes, list) or (
        sample["separated_count"] > 0 and not separation_probes
    ):
        raise RawNormalizationError("separation ledger probes are absent")
    prior_id = 0
    for row in separation_probes:
        if (
            not isinstance(row, list)
            or len(row) != 2
            or any(isinstance(value, bool) or not isinstance(value, int) for value in row)
            or not prior_id < row[0] <= sample["row_count"]
            or row[1] != GENERIC_SEPARATION_CODE
        ):
            raise RawNormalizationError("separation ledger probe is invalid")
        prior_id = row[0]
    return sample, separation


def normalize(raw: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping(raw, "raw Stata result")
    if set(raw) != FIELDS or raw["complete_sentinel"] != "FEPPML_STATA_RAW_COMPLETE":
        raise RawNormalizationError("raw Stata result schema/sentinel is incomplete")
    if raw["arm"] not in {"candidate", "comparator"}:
        raise RawNormalizationError("raw Stata arm is invalid")
    if isinstance(raw["rc"], bool) or not isinstance(raw["rc"], int):
        raise RawNormalizationError("raw Stata return code is not an integer")
    if not isinstance(raw["converged"], bool):
        raise RawNormalizationError("raw Stata convergence flag is not boolean")
    sample_identity, separation_ledger = _identity_records(
        raw["sample_identity"], raw["separation_ledger"]
    )
    names = raw["coefficient_names"]
    codes = raw["coefficient_status_codes"]
    coefficients = raw["coefficients"]
    if not isinstance(names, list) or not names or not all(isinstance(value, str) and value for value in names):
        raise RawNormalizationError("coefficient names are invalid")
    if len(set(names)) != len(names) or not (len(names) == len(codes) == len(coefficients)):
        raise RawNormalizationError("coefficient stripe/status/value lengths differ")
    try:
        statuses = [STATUS[int(value)] for value in codes]
    except (KeyError, TypeError, ValueError) as error:
        raise RawNormalizationError("coefficient status code is unknown") from error
    coefficients = [_finite(value, "coefficient") for value in coefficients]
    vce = raw["vce"]
    if not isinstance(vce, list) or len(vce) != len(names) ** 2:
        raise RawNormalizationError("VCE is not a full row-major square matrix")
    vce = [_finite(value, "VCE") for value in vce]
    for slot, status in enumerate(statuses):
        if status != "identified":
            if coefficients[slot] != 0.0 or any(vce[slot * len(names) + j] != 0.0 or vce[j * len(names) + slot] != 0.0 for j in range(len(names))):
                raise RawNormalizationError("nonidentified coefficient is not exact zero in b/V")
    dof = dict(_mapping(raw["dof"], "DoF"))
    if set(dof) != {"N_physical", "N_effective", "finite_rank", "absorbed_rank", "df", "cluster_counts"}:
        raise RawNormalizationError("DoF schema is incomplete")
    for field in ("N_physical", "N_effective", "finite_rank", "absorbed_rank", "df"):
        dof[field] = _finite(dof[field], f"DoF.{field}")
    if not isinstance(dof["cluster_counts"], list) or not all(isinstance(value, int) and value >= 2 for value in dof["cluster_counts"]):
        raise RawNormalizationError("cluster counts are invalid")
    objective = dict(_mapping(raw["objective"], "objective"))
    if set(objective) != {"deviance", "log_likelihood", "null_log_likelihood", "pseudo_r2"}:
        raise RawNormalizationError("objective schema is incomplete")
    objective = {field: _finite(value, f"objective.{field}") for field, value in objective.items()}
    outputs = dict(_mapping(raw["outputs"], "outputs"))
    if set(outputs) != {"prediction_probe", "total_absorbed_effect_probe", "individual_fe_probe"}:
        raise RawNormalizationError("output probe schema is incomplete")
    for field, values in outputs.items():
        if values is not None:
            if not isinstance(values, list) or not values:
                raise RawNormalizationError(f"{field} must be null or nonempty")
            outputs[field] = [_finite(value, field) for value in values]
    certificates = dict(_mapping(raw["certificates"], "certificates"))
    expected_certificates = {"two_consecutive_iterations", "strict_final_solve", "rank_frozen", "score_linf", "fe_score_linf", "vce_symmetry_linf", "reconstruction_linf"}
    if set(certificates) != expected_certificates:
        raise RawNormalizationError("certificate schema is incomplete")
    for field in ("two_consecutive_iterations", "strict_final_solve", "rank_frozen"):
        if not isinstance(certificates[field], bool):
            raise RawNormalizationError(f"certificate.{field} must be boolean")
    for field in ("score_linf", "fe_score_linf", "vce_symmetry_linf", "reconstruction_linf"):
        certificates[field] = _finite(certificates[field], f"certificate.{field}")
    if raw["arm"] == "candidate" and not all(certificates[field] is True for field in ("two_consecutive_iterations", "strict_final_solve", "rank_frozen")):
        raise RawNormalizationError("candidate lacks mandatory native convergence certificates")
    inventory = raw["source_inventory"]
    if (
        not isinstance(inventory, list)
        or not inventory
        or not all(isinstance(value, str) and value for value in inventory)
        or len(set(inventory)) != len(inventory)
    ):
        raise RawNormalizationError("raw Stata source inventory is absent")
    return {"rc": raw["rc"], "status": "success" if raw["rc"] == 0 else "failure", "sample_identity_sha256": sample_identity["sha256"], "separation_ledger_sha256": separation_ledger["sha256"], "coefficient_names": list(names), "coefficient_status": statuses, "coefficients": coefficients, "vce_shape": [len(names), len(names)], "vce": vce, "dof": dof, "objective": objective, "outputs": outputs, "certificates": {"converged": raw["converged"], **certificates}}


def normalize_file(source: Path, destination: Path) -> None:
    raw = json.loads(source.read_text(encoding="utf-8"))
    destination.write_text(json.dumps(normalize(raw), sort_keys=True, indent=2) + "\n", encoding="ascii")
