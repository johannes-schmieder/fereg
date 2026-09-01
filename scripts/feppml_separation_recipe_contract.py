#!/usr/bin/env python3
"""Closed Phase-7 separation catalog, oracle, Stata, and receipt binding."""
from __future__ import annotations

import importlib.util
import itertools
import json
from fractions import Fraction
from pathlib import Path
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/feppml_separation_evidence_v1.json"
ORACLE = ROOT / "tests/python/ppml_recession_cone_oracle.py"
CASE_IDS = (
    "finite_indicator", "full_positive_rank", "boundary_infeasible",
    "duplicate_degenerate", "fe_only", "mixed_finite_fe", "absorbed_slope",
    "pweight_active", "exposure_active", "fweight_singleton_multiplicity",
    "zero_column_degenerate", "all_zero_terminal_failure",
)
COMPARATOR_ELIGIBLE_IDS = (
    "finite_indicator", "full_positive_rank", "duplicate_degenerate", "fe_only",
    "mixed_finite_fe", "absorbed_slope", "pweight_active", "exposure_active",
    "zero_column_degenerate", "all_zero_terminal_failure",
)
class SeparationRecipeError(ValueError):
    pass


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise SeparationRecipeError(
            f"{where} is not closed; missing={sorted(fields - set(value))}, "
            f"extra={sorted(set(value) - fields)}"
        )


def _oracle_module():
    spec = importlib.util.spec_from_file_location("feppml_phase7_exact_oracle", ORACLE)
    if spec is None or spec.loader is None:
        raise SeparationRecipeError("cannot load the independent recession-cone oracle")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise SeparationRecipeError(f"cannot load formal binding module: {path}")
    module = importlib.util.module_from_spec(spec)
    sibling_root = str(path.parent)
    added = sibling_root not in sys.path
    if added:
        sys.path.insert(0, sibling_root)
    try:
        spec.loader.exec_module(module)
    finally:
        if added:
            sys.path.remove(sibling_root)
    return module


def accepted_sequences(methods: Sequence[str]) -> tuple[tuple[str, ...], ...]:
    return tuple(
        sequence
        for length in range(1, len(methods) + 1)
        for sequence in itertools.permutations(methods, length)
    )


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="ascii"))
    except (OSError, json.JSONDecodeError) as error:
        raise SeparationRecipeError(f"cannot read separation catalog: {error}") from error
    if not isinstance(document, dict):
        raise SeparationRecipeError("separation catalog must be an object")
    validate_manifest(document)
    return document


def validate_manifest(document: Mapping[str, Any]) -> None:
    _closed(
        document,
        {
            "schema_version", "manifest_id", "status", "method_contract",
            "determinism_contract", "formal_binding", "cases",
        },
        "catalog",
    )
    if (
        document["schema_version"] != 1
        or document["manifest_id"] != "feppml_separation_evidence_v1"
        or document["status"] != "qualification_frozen"
    ):
        raise SeparationRecipeError("separation catalog identity is not frozen")
    method = document["method_contract"]
    if not isinstance(method, Mapping):
        raise SeparationRecipeError("method_contract must be an object")
    _closed(
        method,
        {
            "methods", "registered_order", "accepted_sequence_rule",
            "accepted_sequence_count", "special_sequences", "reason_codes",
        },
        "method_contract",
    )
    methods = ["fe", "simplex", "ir", "mu"]
    sequences = accepted_sequences(methods)
    if (
        method["methods"] != methods
        or method["registered_order"] != methods
        or method["accepted_sequence_rule"]
        != "every permutation of every nonempty subset, without repetition"
        or method["accepted_sequence_count"] != len(sequences)
        or len(sequences) != 64
        or method["special_sequences"] != ["none", "default"]
        or method["reason_codes"] != {"fe": 9, "simplex": 10, "ir": 11, "mu": 12}
    ):
        raise SeparationRecipeError("accepted detector sequence contract changed")
    determinism = document["determinism_contract"]
    if not isinstance(determinism, Mapping):
        raise SeparationRecipeError("determinism_contract must be an object")
    _closed(determinism, {"threads", "same_data", "exact_fields"}, "determinism_contract")
    if determinism != {
        "threads": [1, 4],
        "same_data": True,
        "exact_fields": [
            "sample", "drop_reasons", "coefficients", "covariance",
            "initial_generation_hash", "final_generation_hash", "trace_digest",
            "generation_hash_digest",
        ],
    }:
        raise SeparationRecipeError("same-data thread determinism contract changed")
    binding = document["formal_binding"]
    if not isinstance(binding, Mapping):
        raise SeparationRecipeError("formal_binding must be an object")
    _closed(
        binding,
        {
            "fixture", "hidden_wrapper", "stata_profile",
            "comparator_qualification_manifest", "comparator_compiler",
            "phase12_checks", "aggregate",
        },
        "formal_binding",
    )
    expected_binding = {
        "fixture": "tests/stata/parity_ppmlhdfe/feppml_separation_methods.do",
        "hidden_wrapper": "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_separation_methods.do",
        "stata_profile": "ppml-hidden",
        "comparator_qualification_manifest": "benchmarks/manifests/feppml_qualification_v1.json",
        "comparator_compiler": "scripts/generate_feppml_qualification_recipes.py",
        "phase12_checks": ["python", "stata_ppml_hidden", "feppml_qualification"],
        "aggregate": "scripts/feppml_activation_gate.py",
    }
    if binding != expected_binding:
        raise SeparationRecipeError("formal Phase-12 binding path changed")

    cases = document["cases"]
    if not isinstance(cases, list) or len(cases) != 12:
        raise SeparationRecipeError("separation case inventory must contain exactly 12 cases")
    ids: list[str] = []
    classes: set[str] = set()
    oracle = _oracle_module()
    fields = {
        "id", "design", "outcome", "expected_separated_rows", "simplex_class",
        "weight", "exposure", "fweight_groups", "expected_singleton_rows",
        "terminal_rc", "comparator", "stata_marker", "stata",
    }
    for index, case in enumerate(cases):
        if not isinstance(case, Mapping):
            raise SeparationRecipeError(f"case[{index}] must be an object")
        _closed(case, fields, f"case[{index}]")
        identifier = case["id"]
        if not isinstance(identifier, str) or not identifier or identifier in ids:
            raise SeparationRecipeError("case IDs must be unique nonempty strings")
        ids.append(identifier)
        design = case["design"]
        outcome = case["outcome"]
        if (
            not isinstance(design, list)
            or not 1 <= len(design) <= 64
            or not isinstance(outcome, list)
            or len(outcome) != len(design)
            or not isinstance(design[0], list)
            or not 1 <= len(design[0]) <= 16
            or any(not isinstance(row, list) or len(row) != len(design[0]) for row in design)
        ):
            raise SeparationRecipeError("oracle case dimensions exceed the bounded contract")
        separated_rows = case["expected_separated_rows"]
        if (
            not isinstance(separated_rows, list)
            or separated_rows != sorted(set(separated_rows))
            or any(isinstance(row, bool) or not isinstance(row, int) or not 0 <= row < len(design) for row in separated_rows)
        ):
            raise SeparationRecipeError("expected separated rows are not a bounded sorted set")
        if case["terminal_rc"] not in {0, 2001}:
            raise SeparationRecipeError("terminal return code is outside the frozen catalog")
        if not isinstance(case["stata_marker"], str) or not case["stata_marker"]:
            raise SeparationRecipeError("Stata marker must be a nonempty string")
        comparator = case["comparator"]
        if not isinstance(comparator, Mapping):
            raise SeparationRecipeError("case comparator policy must be an object")
        _closed(comparator, {"mode", "expected_rc", "rationale"}, f"{identifier}.comparator")
        if (
            comparator["mode"] not in {"pinned_ppmlhdfe", "oracle_only"}
            or not isinstance(comparator["rationale"], str)
            or not comparator["rationale"]
        ):
            raise SeparationRecipeError("case comparator adjudication policy is invalid")
        if comparator["mode"] == "pinned_ppmlhdfe":
            if comparator["expected_rc"] not in {0, "nonzero"}:
                raise SeparationRecipeError("comparator-eligible case has an invalid return contract")
        elif comparator["expected_rc"] is not None:
            raise SeparationRecipeError("oracle-only case cannot claim a comparator return code")
        stata = case["stata"]
        if not isinstance(stata, Mapping):
            raise SeparationRecipeError("Stata recipe must be an object")
        _closed(
            stata,
            {
                "columns", "rows", "finite", "absorb", "weight_column",
                "exposure_column", "method", "expected_reason",
            },
            f"{identifier}.stata",
        )
        columns = stata["columns"]
        rows = stata["rows"]
        if (
            not isinstance(columns, list)
            or not columns
            or columns[0] != "y"
            or len(columns) != len(set(columns))
            or any(not isinstance(name, str) or not name for name in columns)
            or not isinstance(rows, list)
            or len(rows) != len(outcome)
            or any(not isinstance(row, list) or len(row) != len(columns) for row in rows)
        ):
            raise SeparationRecipeError("Stata recipe data table is not closed and row-aligned")
        row_maps = [dict(zip(columns, row)) for row in rows]
        if [row[0] for row in rows] != outcome:
            raise SeparationRecipeError("Stata outcome rows differ from the oracle")
        finite = stata["finite"]
        absorb = stata["absorb"]
        if (
            not isinstance(finite, list)
            or not isinstance(absorb, list)
            or any(name not in columns[1:] for name in finite)
            or any(not isinstance(term, str) or not term for term in absorb)
        ):
            raise SeparationRecipeError("Stata finite/absorbed model recipe is invalid")
        rebuilt: list[list[Any]] = [[] for _ in rows]
        for name in finite:
            for row_index, row in enumerate(row_maps):
                rebuilt[row_index].append(row[name])
        for term in absorb:
            if "##c." in term:
                group, slope = term.split("##c.", 1)
            else:
                group, slope = term, None
            if group not in columns or (slope is not None and slope not in columns):
                raise SeparationRecipeError("Stata absorbed term names an absent source")
            levels = sorted({oracle.rational(row[group]) for row in row_maps})
            for level in levels:
                for row_index, row in enumerate(row_maps):
                    rebuilt[row_index].append(int(oracle.rational(row[group]) == level))
            if slope is not None:
                for level in levels:
                    for row_index, row in enumerate(row_maps):
                        rebuilt[row_index].append(
                            row[slope] if oracle.rational(row[group]) == level else 0
                        )
        if not absorb:
            for row in rebuilt:
                row.append(1)
        rational_rebuilt = [
            [oracle.rational(value) for value in row] for row in rebuilt
        ]
        rational_design = [
            [oracle.rational(value) for value in row] for row in design
        ]
        if rational_rebuilt != rational_design:
            raise SeparationRecipeError("Stata recipe explicit model span differs from oracle design")
        classes.add(case["simplex_class"])
        separated, witnesses = oracle.separated_rows(design, outcome)
        if list(separated) != case["expected_separated_rows"]:
            raise SeparationRecipeError(f"independent cone result changed for {identifier}")
        for row, witness in witnesses.items():
            values = oracle.evaluate_direction(case["design"], witness)
            if values[row] >= 0:
                raise SeparationRecipeError(f"oracle witness lacks a strict margin for {identifier}")
            for outcome, value in zip(case["outcome"], values):
                if oracle.rational(outcome) > 0 and value != 0:
                    raise SeparationRecipeError(f"oracle witness violates the interior for {identifier}")
                if oracle.rational(outcome) == 0 and value > 0:
                    raise SeparationRecipeError(f"oracle witness violates the boundary for {identifier}")
        weight = case["weight"]
        if not isinstance(weight, Mapping):
            raise SeparationRecipeError("case weight must be an object")
        _closed(weight, {"kind", "values"}, f"{identifier}.weight")
        if weight["kind"] not in {"none", "fweight", "pweight"}:
            raise SeparationRecipeError("unknown case weight kind")
        values = weight["values"]
        if weight["kind"] == "none":
            if values is not None:
                raise SeparationRecipeError("unweighted case supplied weights")
        elif not isinstance(values, list) or len(values) != len(case["outcome"]):
            raise SeparationRecipeError("weighted case has the wrong row count")
        elif any(oracle.rational(value) <= 0 for value in values):
            raise SeparationRecipeError("case weights must be positive")
        expected_weight_column = {
            "none": None, "fweight": stata["weight_column"],
            "pweight": stata["weight_column"],
        }[weight["kind"]]
        if (weight["kind"] == "none") != (stata["weight_column"] is None):
            raise SeparationRecipeError("Stata weight binding differs from oracle weight kind")
        if expected_weight_column is not None:
            if expected_weight_column not in columns:
                raise SeparationRecipeError("Stata weight column is absent")
            observed_weights = [row[expected_weight_column] for row in row_maps]
            if [oracle.rational(value) for value in observed_weights] != [
                oracle.rational(value) for value in values
            ]:
                raise SeparationRecipeError("Stata weights differ from oracle weights")
        if case["exposure"] is not None and (
            not isinstance(case["exposure"], list)
            or len(case["exposure"]) != len(case["outcome"])
            or any(oracle.rational(value) <= 0 for value in case["exposure"])
        ):
            raise SeparationRecipeError("exposure must be positive and row-aligned")
        if (case["exposure"] is None) != (stata["exposure_column"] is None):
            raise SeparationRecipeError("Stata exposure binding differs from oracle")
        if stata["exposure_column"] is not None:
            if stata["exposure_column"] not in columns:
                raise SeparationRecipeError("Stata exposure column is absent")
            if [oracle.rational(row[stata["exposure_column"]]) for row in row_maps] != [
                oracle.rational(value) for value in case["exposure"]
            ]:
                raise SeparationRecipeError("Stata exposures differ from oracle exposures")
        if stata["method"] not in {
            "none", "default", "fe", "simplex", "ir", "mu", "fe simplex ir"
        }:
            raise SeparationRecipeError("Stata case method is outside the frozen recipe")
        if stata["expected_reason"] is not None and stata["expected_reason"] not in {9, 10, 11, 12}:
            raise SeparationRecipeError("Stata expected reason is invalid")
        if case["terminal_rc"] == 0:
            if bool(separated_rows) != (stata["expected_reason"] is not None):
                raise SeparationRecipeError(
                    "Stata separation reason must exist exactly when rows are dropped"
                )
            if separated_rows:
                requested = (
                    methods if stata["method"] == "default"
                    else stata["method"].split()
                )
                expected_reason = next(
                    method["reason_codes"][name]
                    for name in method["registered_order"]
                    if name in requested
                )
                if stata["expected_reason"] != expected_reason:
                    raise SeparationRecipeError(
                        "Stata separation reason violates registered detector precedence"
                    )
        elif stata["expected_reason"] is not None:
            raise SeparationRecipeError("terminal failure cannot publish a drop reason")
        groups = case["fweight_groups"]
        expected_singletons = case["expected_singleton_rows"]
        if groups is None:
            if expected_singletons:
                raise SeparationRecipeError("singleton expectation lacks group topology")
        else:
            if weight["kind"] != "fweight" or len(groups) != len(case["outcome"]):
                raise SeparationRecipeError("fweight singleton topology is invalid")
            if "fe" not in columns or [row["fe"] for row in row_maps] != groups:
                raise SeparationRecipeError(
                    "fweight singleton groups differ from the executed FE topology"
                )
            totals = {
                group: sum(
                    oracle.rational(values[row])
                    for row, observed in enumerate(groups)
                    if observed == group
                )
                for group in set(groups)
            }
            singleton_rows = [row for row, group in enumerate(groups) if totals[group] == 1]
            if singleton_rows != expected_singletons:
                raise SeparationRecipeError("fweight singleton multiplicity result changed")
    if tuple(ids) != CASE_IDS:
        raise SeparationRecipeError("separation case identity or order changed")
    comparator_ids = tuple(
        case["id"] for case in cases
        if case["comparator"]["mode"] == "pinned_ppmlhdfe"
    )
    if comparator_ids != COMPARATOR_ELIGIBLE_IDS:
        raise SeparationRecipeError("comparator-eligible case inventory changed")
    if [case["terminal_rc"] for case in cases] != [0] * 11 + [2001]:
        raise SeparationRecipeError("terminal success/failure inventory changed")
    required_classes = {
        "finite_feasible", "full_positive_rank", "cone_infeasible",
        "degenerate_duplicate_columns", "fe_only", "mixed_finite_fe",
        "absorbed_slope", "weighted_invariant", "adjustment_invariant",
        "no_boundary_candidates", "zero_null_direction", "empty_terminal_sample",
    }
    if classes != required_classes:
        raise SeparationRecipeError("simplex degeneracy/infeasibility catalog is incomplete")


def sequence_recipe_lines(document: Mapping[str, Any]) -> tuple[str, ...]:
    method = document["method_contract"]
    reason_codes = method["reason_codes"]
    lines: list[str] = []
    for index, sequence in enumerate(accepted_sequences(method["methods"]), 1):
        requested = {name: int(name in sequence) for name in method["methods"]}
        reason = next(reason_codes[name] for name in method["registered_order"] if requested[name])
        lines.append(
            f'__phase7_sequence_case p7_sequence_{index:03d} "{" ".join(sequence)}" '
            f'{reason} {requested["fe"]} {requested["simplex"]} '
            f'{requested["ir"]} {requested["mu"]}'
        )
    return tuple(lines)


def _stata_number(value: Any) -> str:
    """Render a manifest rational as a lossless Stata input-table literal."""
    fraction = Fraction(value)
    if fraction.denominator == 1:
        return str(fraction.numerator)
    denominator = fraction.denominator
    twos = 0
    fives = 0
    while denominator % 2 == 0:
        denominator //= 2
        twos += 1
    while denominator % 5 == 0:
        denominator //= 5
        fives += 1
    if denominator != 1:
        raise SeparationRecipeError(
            f"Stata input recipe cannot losslessly spell rational {fraction}"
        )
    places = max(twos, fives)
    scaled = abs(fraction.numerator) * 2 ** (places - twos) * 5 ** (places - fives)
    digits = str(scaled).rjust(places + 1, "0")
    rendered = f"{digits[:-places]}.{digits[-places:]}" if places else digits
    rendered = rendered.rstrip("0").rstrip(".")
    return ("-" if fraction.numerator < 0 else "") + rendered


def stata_case_recipe(case: Mapping[str, Any]) -> str:
    """Generate the exact executable Stata block bound to one oracle case."""
    identifier = case["id"]
    stata = case["stata"]
    columns = stata["columns"]
    rows = stata["rows"]
    separated = [row + 1 for row in case["expected_separated_rows"]]
    lines = [
        f"* PHASE7_RECIPE_BEGIN|{identifier}",
        "clear",
        f"input long obs_id double({' '.join(columns)})",
    ]
    for obs_id, row in enumerate(rows, 1):
        lines.append(
            f"{obs_id} " + " ".join(_stata_number(value) for value in row)
        )
    lines.extend(("end", f"assert _N == {len(rows)}"))

    terms = " ".join(f"c.{name}" for name in stata["finite"])
    command = f"`feppml_test_command' y"
    if terms:
        command += f" {terms}"
    options: list[str] = []
    if stata["absorb"]:
        options.append(f"absorb({' '.join(stata['absorb'])})")
    weight_kind = case["weight"]["kind"]
    if weight_kind != "none":
        options.append(f"{weight_kind}({stata['weight_column']})")
    if stata["exposure_column"] is not None:
        options.append(f"exposure({stata['exposure_column']})")
    options.append(f"separation({stata['method']})")

    if case["terminal_rc"] == 0:
        options.append("dropreason(__p7_case_drop)")
        lines.append(command + ", " + " ".join(options))
        lines.extend(
            (
                "assert e(converged) == 1",
                f"assert e(separated) == {len(separated)}",
                f"assert e(N_physical) == {len(rows) - len(separated)}",
            )
        )
        if separated:
            predicate = "inlist(obs_id, " + ", ".join(map(str, separated)) + ")"
            lines.extend(
                (
                    f"generate byte __p7_expected_drop = {predicate}",
                    "assert e(sample) == !__p7_expected_drop",
                    f"assert __p7_case_drop == {stata['expected_reason']} if __p7_expected_drop",
                    "assert __p7_case_drop == 0 if !__p7_expected_drop",
                )
            )
        else:
            lines.extend(
                (
                    "assert e(sample)",
                    "assert __p7_case_drop == 0",
                )
            )
        if identifier == "boundary_infeasible":
            lines.append("assert e(separated) == 0")
        if identifier == "fweight_singleton_multiplicity":
            lines.extend(
                (
                    "assert e(num_singletons) == 0",
                    "assert e(N_physical) == 9",
                    "assert e(sample) if obs_id == 1",
                )
            )
    else:
        lines.extend(
            (
                "quietly regress y",
                "tempname __p7_before_b __p7_before_v",
                "matrix `__p7_before_b' = e(b)",
                "matrix `__p7_before_v' = e(V)",
                "datasignature, reset",
                "capture noisily " + command + ", " + " ".join(options),
                f"assert _rc == {case['terminal_rc']}",
                "datasignature confirm",
                "assert mreldif(e(b), `__p7_before_b') == 0",
                "assert mreldif(e(V), `__p7_before_v') == 0",
            )
        )
    lines.extend(
        (
            f'display as result "PHASE7_ORACLE|{identifier}|{case["stata_marker"]}"',
            f"* PHASE7_RECIPE_END|{identifier}",
        )
    )
    return "\n".join(lines)


def stata_comparator_recipe(case: Mapping[str, Any]) -> str:
    """Generate one exact retained-sample adjudication using pinned ppmlhdfe."""
    if case["comparator"]["mode"] != "pinned_ppmlhdfe":
        raise SeparationRecipeError("oracle-only case has no comparator recipe")
    identifier = case["id"]
    stata = case["stata"]
    columns = stata["columns"]
    rows = stata["rows"]
    separated = [row + 1 for row in case["expected_separated_rows"]]
    lines = [
        f"* PHASE7_COMPARATOR_BEGIN|{identifier}",
        "clear",
        f"input long obs_id double({' '.join(columns)})",
    ]
    for obs_id, row in enumerate(rows, 1):
        lines.append(
            f"{obs_id} " + " ".join(_stata_number(value) for value in row)
        )
    lines.extend(("end", f"assert _N == {len(rows)}"))
    command = "ppmlhdfe y"
    finite = " ".join(f"c.{name}" for name in stata["finite"])
    if finite:
        command += f" {finite}"
    weight_kind = case["weight"]["kind"]
    if weight_kind != "none":
        abbreviation = {"fweight": "fw", "pweight": "pw"}[weight_kind]
        command += f" [{abbreviation}={stata['weight_column']}]"
    options: list[str] = []
    if stata["absorb"]:
        options.append(f"absorb({' '.join(stata['absorb'])})")
    if stata["exposure_column"] is not None:
        options.append(f"exposure({stata['exposure_column']})")
    options.append("separation(fe simplex ir)")
    command += ", " + " ".join(options)
    expected_rc = case["comparator"]["expected_rc"]
    if expected_rc == "nonzero":
        lines.extend((f"capture noisily {command}", "assert _rc != 0"))
    else:
        lines.extend((command, "assert e(converged) == 1"))
        if separated:
            predicate = "inlist(obs_id, " + ", ".join(map(str, separated)) + ")"
            lines.extend(
                (
                    f"generate byte __p7_comparator_expected_drop = {predicate}",
                    "assert e(sample) == !__p7_comparator_expected_drop",
                )
            )
        else:
            lines.append("assert e(sample)")
        lines.extend(
            (
                "quietly count if e(sample)",
                f"assert r(N) == {len(rows) - len(separated)}",
            )
        )
    lines.extend(
        (
            f'display as result "PHASE7_COMPARATOR|{identifier}|ppmlhdfe"',
            f"* PHASE7_COMPARATOR_END|{identifier}",
        )
    )
    return "\n".join(lines)


def stata_comparator_audit(document: Mapping[str, Any]) -> str:
    """Canonical exact-case audit embedded in the pinned comparator science arm."""
    recipes = [
        stata_comparator_recipe(case)
        for case in document["cases"]
        if case["comparator"]["mode"] == "pinned_ppmlhdfe"
    ]
    return "\n\n".join(recipes)


def validate_stata_and_formal_binding(document: Mapping[str, Any], root: Path = ROOT) -> None:
    binding = document["formal_binding"]
    fixture = (root / binding["fixture"]).read_text(encoding="utf-8")
    wrapper = (root / binding["hidden_wrapper"]).read_text(encoding="utf-8")
    for line in sequence_recipe_lines(document):
        if fixture.splitlines().count(line) != 1:
            raise SeparationRecipeError(f"Stata sequence recipe is missing or duplicated: {line}")
    for case in document["cases"]:
        identifier = case["id"]
        begin = f"* PHASE7_RECIPE_BEGIN|{identifier}"
        end = f"* PHASE7_RECIPE_END|{identifier}"
        if fixture.count(begin) != 1 or fixture.count(end) != 1:
            raise SeparationRecipeError(
                f"Stata oracle recipe boundary is missing or duplicated: {identifier}"
            )
        observed = fixture[fixture.index(begin):fixture.index(end) + len(end)]
        expected = stata_case_recipe(case)
        if observed != expected:
            raise SeparationRecipeError(
                f"Stata oracle recipe differs byte-for-byte from manifest: {identifier}"
            )
    for fragment in (
        "e(feppml_separation_sequence)[2,2] == `fe_requested'",
        "e(feppml_separation_sequence)[3,2] == `simplex_requested'",
        "e(feppml_separation_sequence)[4,2] == `ir_requested'",
        "e(feppml_separation_sequence)[5,2] == `mu_requested'",
        "e(feppml_separation_sequence)[2,3] == cond(`fe_requested', 1, 0)",
        "e(feppml_separation_sequence)[3,3] == cond(`simplex_requested', 2, 0)",
        "e(feppml_separation_sequence)[4,3] == cond(`ir_requested', 3, 0)",
        "e(feppml_separation_sequence)[5,3] == cond(`mu_requested', 4, 0)",
        "assert e(num_singletons) == 0",
        "assert e(N_physical) == 9",
        "pweight(pw)",
        "exposure(exposure)",
        "threads(1) deterministic",
        "threads(4) deterministic",
        "mreldif(`__p7_t1_b', e(b)) == 0",
        "mreldif(`__p7_t1_v', e(V)) == 0",
        "assert _rc == 2001",
        "END_FIXTURE|feppml_separation_methods|rc=0",
    ):
        if fragment not in fixture:
            raise SeparationRecipeError(f"Stata Phase-7 contract lacks {fragment!r}")
    if (
        'global FEREG_PPML_TEST_COMMAND "_fereg_ppml"' not in wrapper
        or "feppml_separation_methods.do" not in wrapper
    ):
        raise SeparationRecipeError("hidden Stata wrapper no longer binds the Phase-7 fixture")
    profiles = json.loads((root / "ci/stata_profiles.json").read_text(encoding="utf-8"))["profiles"]
    profile = profiles[binding["stata_profile"]]
    if (
        profile.get("driver") != "fereg-fixtures"
        or profile.get("build_plugin") is not True
        or binding["hidden_wrapper"] not in profile.get("fixtures", [])
    ):
        raise SeparationRecipeError("ppml-hidden profile omits the Phase-7 wrapper")
    gate = _load_module(root / binding["aggregate"], "feppml_phase7_gate_binding")
    if not {"python", "stata_ppml_hidden", "feppml_qualification"}.issubset(gate.COMMON_REQUIRED):
        raise SeparationRecipeError(
            "Phase-12 aggregate omits Python, ppml-hidden, or comparator qualification evidence"
        )
    if gate.COMMON_CHECK_CLAIMS.get("stata_ppml_hidden") != (
        "artifact_bound", "fixture_sentinels", "licensed_stata", "profile:ppml-hidden"
    ):
        raise SeparationRecipeError("Phase-12 ppml-hidden receipt claims changed")
    source_claims = gate.COMMON_CHECK_CLAIMS.get("source_identity", ())
    if not {"exact_commit", "exact_tree"}.issubset(source_claims):
        raise SeparationRecipeError("Phase-12 no longer joins receipts at one exact source")
    if gate.COMMON_CHECK_CLAIMS.get("feppml_qualification") != (
        "comparator", "failure_atomicity", "memory", "oracle", "performance",
        "saved_outputs", "scientific",
    ):
        raise SeparationRecipeError("Phase-12 comparator qualification claims changed")
    qualification_manifest = json.loads(
        (root / binding["comparator_qualification_manifest"]).read_text(encoding="utf-8")
    )
    if qualification_manifest.get("reference_closure", {}).get("ppmlhdfe") != {
        "repository": "https://github.com/sergiocorreia/ppmlhdfe",
        "commit": "b85665d8f674e93c1e07446a9a2b29be7f797910",
    }:
        raise SeparationRecipeError("separation comparator is not the pinned ppmlhdfe source")
    qualification_gate = _load_module(
        root / "scripts/feppml_qualification.py",
        "feppml_phase7_qualification_manifest_binding",
    )
    scripts_root = str(root / "scripts")
    added_scripts_root = scripts_root not in sys.path
    if added_scripts_root:
        sys.path.insert(0, scripts_root)
    try:
        qualification_gate.load_manifest(
            root / binding["comparator_qualification_manifest"], root=root
        )
    finally:
        if added_scripts_root:
            sys.path.remove(scripts_root)
    compiler = binding["comparator_compiler"]
    compilers = qualification_manifest.get("recipe_contract", {}).get("compilers", [])
    if not any(record.get("path") == compiler for record in compilers):
        raise SeparationRecipeError("qualification manifest does not digest-bind comparator compiler")
    compiler_module = _load_module(root / compiler, "feppml_phase7_comparator_compiler")
    if compiler_module.separation_comparator_audit() != stata_comparator_audit(document):
        raise SeparationRecipeError(
            "qualification comparator audit differs from the exact separation manifest"
        )
    public = (root / "stata/feppml.ado").read_text(encoding="utf-8")
    if "exit 498" not in public or "_fereg_ppml" in public:
        raise SeparationRecipeError("public feppml is no longer the fail-closed stub")


def main() -> int:
    document = load_manifest()
    validate_stata_and_formal_binding(document)
    print("FEPPML_PHASE7_SEPARATION_CONTRACT_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
