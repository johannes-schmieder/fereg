#!/usr/bin/env python3
"""Validate and expand the short fereg solver-development matrix."""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import re
from typing import Any, Mapping

try:
    from scripts.ols_reference import ContractError, ROOT, load_manifest
except ModuleNotFoundError:  # Direct ``python scripts/solver_development.py`` use.
    from ols_reference import ContractError, ROOT, load_manifest  # type: ignore[no-redef]


DEFAULT_DEVELOPMENT_MANIFEST = (
    ROOT / "benchmarks/manifests/ols_solver_development_v1.json"
)
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]*$")
EXPECTED_SOLVERS = {
    "auto": "solver(auto)",
    "map": "solver(map)",
    "cg": "solver(cg)",
    "it": "solver(it)",
    "squarem": "solver(squarem)",
    "lsmr_off": "solver(lsmr) preconditioner(off)",
    "lsmr_diagonal": "solver(lsmr) preconditioner(diagonal)",
    "lsmr_schwarz": "solver(lsmr) preconditioner(schwarz)",
    "lsmr_auto": "solver(lsmr) preconditioner(auto)",
}
EXPECTED_FAMILIES = {
    "dense_two_fe",
    "sparse_two_fe",
    "absorbed_slope",
    "clustered_vce",
    "moderate_k",
}


def _expect_keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise ContractError(f"{where} fields are incomplete or unknown")


def load_development_manifest(
    path: Path = DEFAULT_DEVELOPMENT_MANIFEST,
    release_manifest_path: Path | None = None,
) -> dict[str, Any]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, dict):
        raise ContractError("solver development manifest must be an object")
    _expect_keys(
        document,
        {
            "schema_version",
            "manifest_id",
            "status",
            "scientific_contract",
            "minimum_observations",
            "thread_counts",
            "repetitions",
            "candidate_arms",
            "families",
            "selection_rules",
        },
        "solver development manifest",
    )
    if (
        document["schema_version"] != 1
        or document["manifest_id"] != "ols_solver_development_v1"
        or document["status"] != "development_only"
        or document["minimum_observations"] != 10_000_000
        or document["thread_counts"] != [1, 8]
        or document["repetitions"] != {"cold": 1, "warm": 2}
    ):
        raise ContractError("solver development matrix identity is not frozen")
    arms = document["candidate_arms"]
    if not isinstance(arms, list) or {
        arm.get("id"): arm.get("options")
        for arm in arms
        if isinstance(arm, Mapping)
    } != EXPECTED_SOLVERS:
        raise ContractError("solver development candidate inventory is not frozen")
    families = document["families"]
    if not isinstance(families, list) or {
        family.get("id") for family in families if isinstance(family, Mapping)
    } != EXPECTED_FAMILIES:
        raise ContractError("solver development family inventory is not frozen")
    contract_path = (
        release_manifest_path
        if release_manifest_path is not None
        else ROOT / document["scientific_contract"]
    )
    release = load_manifest(contract_path)
    release_cases = {case["id"]: case for case in release["benchmark_cases"]}
    seen: set[str] = set()
    for family in families:
        _expect_keys(
            family, {"id", "ols_v1_cases", "gregress_diagnostic"}, "solver family"
        )
        if not SAFE_ID.fullmatch(family["id"]):
            raise ContractError("solver family id is malformed")
        cases = family["ols_v1_cases"]
        if not isinstance(cases, list) or len(cases) != 2:
            raise ContractError("solver family must bind one and eight-thread cases")
        threads = set()
        for case_id in cases:
            if case_id not in release_cases or case_id in seen:
                raise ContractError("solver family case binding is stale or duplicated")
            seen.add(case_id)
            threads.add(release_cases[case_id]["thread_count"])
        if threads != {1, 8}:
            raise ContractError("solver family does not bind one and eight threads")
    if seen != set(release_cases):
        raise ContractError("solver development matrix does not cover release cases")
    if document["selection_rules"] != {
        "scientific_parity_required": True,
        "setup_cost_included": True,
        "default_auto_only_release_gated": True,
        "development_results_do_not_qualify_release": True,
    }:
        raise ContractError("solver development selection rules are not frozen")
    return document


def expand_run_plan(
    development: Mapping[str, Any], release: Mapping[str, Any]
) -> list[dict[str, Any]]:
    """Return deterministic candidate/comparator/diagnostic run descriptors."""
    release_cases = {case["id"]: case for case in release["benchmark_cases"]}
    plan: list[dict[str, Any]] = []
    for family in development["families"]:
        for case_id in family["ols_v1_cases"]:
            case = release_cases[case_id]
            base_candidate = case["arms"]["candidate"].split(" solver(", 1)[0]
            plan.append(
                {
                    "id": f"{case_id}__reghdfe",
                    "family": family["id"],
                    "case_id": case_id,
                    "arm": "reghdfe",
                    "thread_count": case["thread_count"],
                    "command": case["arms"]["comparator"],
                    "repetitions": copy.deepcopy(development["repetitions"]),
                }
            )
            for arm in development["candidate_arms"]:
                plan.append(
                    {
                        "id": f"{case_id}__{arm['id']}",
                        "family": family["id"],
                        "case_id": case_id,
                        "arm": arm["id"],
                        "thread_count": case["thread_count"],
                        "command": f"{base_candidate} {arm['options']}",
                        "repetitions": copy.deepcopy(development["repetitions"]),
                    }
                )
            if family["gregress_diagnostic"]:
                comparator = case["arms"]["comparator"]
                plan.append(
                    {
                        "id": f"{case_id}__gregress",
                        "family": family["id"],
                        "case_id": case_id,
                        "arm": "gregress",
                        "thread_count": case["thread_count"],
                        "command": comparator.replace("reghdfe ", "gregress ", 1),
                        "repetitions": copy.deepcopy(development["repetitions"]),
                    }
                )
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_DEVELOPMENT_MANIFEST)
    parser.add_argument("--release-manifest", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    development = load_development_manifest(args.manifest, args.release_manifest)
    release = load_manifest(args.release_manifest) if args.release_manifest else load_manifest()
    plan = expand_run_plan(development, release)
    encoded = json.dumps(plan, sort_keys=True, indent=2) + "\n"
    if args.output is None:
        print(encoded, end="")
    else:
        args.output.write_text(encoded, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
