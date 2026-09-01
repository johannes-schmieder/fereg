#!/usr/bin/env python3
"""Compile the two frozen PPML constrained-memory proof fixtures."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Any, Mapping, Sequence

try:
    from scripts import generate_feppml_qualification_recipes as recipes
    from scripts import ppml_memory_performance as performance
except ImportError:
    import generate_feppml_qualification_recipes as recipes  # type: ignore[no-redef]
    import ppml_memory_performance as performance  # type: ignore[no-redef]


DATA_CELL_ID = "gov_collinear_two_fe_k50_t1"
# These imposed totals leave the production planner's fixed/20% headroom in
# place.  The evidence gate, not this source file, decides whether the intended
# low/fast/no-route outcomes actually occurred.
LOW_FAST_LIMIT_BYTES = 20_000_000_000
NO_ROUTE_LIMIT_BYTES = 1_500_000_000


class ConstrainedRecipeError(RuntimeError):
    pass


def _write_new(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise ConstrainedRecipeError(f"refusing to overwrite constrained recipe: {path}") from error
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(text)
        target.flush()
        os.fsync(target.fileno())


def attempt_do(command: str) -> str:
    return "\n".join(
        [
            "version 19.0",
            "clear all",
            "set more off",
            "args data artifact result ready start done requested comparator_limit pair temperature cell arm",
            "use \"`data'\", clear",
            "local artifact_dir = substr(\"`artifact'\", 1, strrpos(\"`artifact'\", \"/\") - 1)",
            "adopath ++ \"`artifact_dir'\"",
            "findfile _fereg_estimate_ppml.ado",
            "local __q_resolved_ado = r(fn)",
            "assert \"`__q_resolved_ado'\" == \"`artifact_dir'/_fereg_estimate_ppml.ado\"",
            "findfile fereg_parse.ado",
            "local __q_resolved_parse = r(fn)",
            "assert \"`__q_resolved_parse'\" == \"`artifact_dir'/fereg_parse.ado\"",
            "findfile _fereg_load.ado",
            "local __q_resolved_load = r(fn)",
            "assert \"`__q_resolved_load'\" == \"`artifact_dir'/_fereg_load.ado\"",
            "local __q_plugin_name = substr(\"`artifact'\", strrpos(\"`artifact'\", \"/\") + 1, .)",
            "findfile `__q_plugin_name'",
            "local __q_resolved_plugin = r(fn)",
            "assert \"`__q_resolved_plugin'\" == \"`artifact'\"",
            "file open __resolution using \"RESOLUTION\", write text",
            "file write __resolution \"ADO|`__q_resolved_ado'\" _n",
            "file write __resolution \"ARTIFACT|`__q_resolved_plugin'\" _n",
            "file write __resolution \"DEPENDENCY|`__q_resolved_parse'\" _n",
            "file write __resolution \"DEPENDENCY|`__q_resolved_load'\" _n",
            "file close __resolution",
            "file open __ready using \"`ready'\", write text new",
            "file write __ready \"FEPPML_STATA_READY|`cell'|`arm'\" _n",
            "file close __ready",
            "file open __start using \"`start'\", write text new",
            "file write __start \"FEPPML_STATA_START|`cell'|`arm'\" _n",
            "file close __start",
            "capture noisily " + command,
            "local rc = _rc",
            'local status "failure"',
            "if `rc' == 0 {",
            '    local status "success"',
            "}",
            "file open __result using \"`result'\", write text new",
            "file write __result \"{\\\"attempt\\\":{\\\"id\\\":\\\"`temperature'\\\",\\\"status\\\":\\\"`status'\\\",\\\"rc\\\":`rc'},\\\"stata\\\":{\\\"licensed\\\":true,\\\"version\\\":\\\"`c(stata_version)' `c(flavor)'\\\"},\\\"complete_sentinel\\\":\\\"FEPPML_STATA_CONSTRAINED_COMPLETE\\\"}\" _n",
            "file close __result",
            "file open __done using \"`done'\", write text new",
            "file write __done \"FEPPML_STATA_DONE|`cell'|`arm'|SUCCESS\" _n",
            "file close __done",
            'display as result "FEPPML_STATA_CONSTRAINED_COMPLETE"',
        ]
    ) + "\n"


def compile_recipes(manifest_path: Path, output: Path) -> Mapping[str, Any]:
    manifest = performance.load_manifest(manifest_path)
    cells = {cell["cell_id"]: cell for cell in performance.expand_cells(manifest)}
    if DATA_CELL_ID not in cells:
        raise ConstrainedRecipeError("constrained data source cell is absent")
    base = cells[DATA_CELL_ID]
    fixtures = [
        ("low_succeeds_fast_fails", LOW_FAST_LIMIT_BYTES, ("low", "fast")),
        ("no_route_fits", NO_ROUTE_LIMIT_BYTES, ("auto",)),
    ]
    records = []
    for fixture_id, limit, modes in fixtures:
        attempts = []
        for mode in modes:
            configured = dict(base)
            configured["memory"] = mode
            command = recipes.commands(configured)["candidate_timing"]
            relative = Path("fixtures") / fixture_id / f"{mode}.do"
            _write_new(output / relative, attempt_do(command))
            attempts.append(
                {
                    "id": mode,
                    "do_file": relative.as_posix(),
                    "memory_limit_bytes": limit,
                    "science_output": f"{mode}.json",
                }
            )
        records.append(
            {
                "id": fixture_id,
                "dataset_cell_id": DATA_CELL_ID,
                "attempts": attempts,
            }
        )
    document = {
        "schema_version": 1,
        "recipe_type": "feppml-constrained-memory-recipes-v1",
        "development_manifest_sha256": performance.sha256_file(manifest_path),
        "fixtures": records,
        "complete_sentinel": "FEPPML_CONSTRAINED_RECIPES_COMPLETE",
    }
    validate_compiled_recipes(document, manifest_path, output)
    _write_new(
        output / "constrained.json",
        json.dumps(document, sort_keys=True, indent=2, ensure_ascii=True) + "\n",
    )
    return document


def validate_compiled_recipes(
    document: Mapping[str, Any], manifest_path: Path, generated_root: Path
) -> None:
    manifest = performance.load_manifest(manifest_path)
    cells = {cell["cell_id"]: cell for cell in performance.expand_cells(manifest)}
    base = cells[DATA_CELL_ID]
    expected = [
        ("low_succeeds_fast_fails", LOW_FAST_LIMIT_BYTES, ("low", "fast")),
        ("no_route_fits", NO_ROUTE_LIMIT_BYTES, ("auto",)),
    ]
    fixtures = document.get("fixtures")
    if not isinstance(fixtures, list) or len(fixtures) != 2:
        raise ConstrainedRecipeError("constrained fixture count is not exact")
    for record, (fixture_id, limit, modes) in zip(fixtures, expected):
        if record.get("id") != fixture_id or record.get("dataset_cell_id") != DATA_CELL_ID:
            raise ConstrainedRecipeError("constrained fixture identity is invalid")
        attempts = record.get("attempts")
        if not isinstance(attempts, list) or [attempt.get("id") for attempt in attempts] != list(modes):
            raise ConstrainedRecipeError("constrained attempt inventory/order is invalid")
        for attempt, mode in zip(attempts, modes):
            configured = dict(base)
            configured["memory"] = mode
            expected_command = recipes.commands(configured)["candidate_timing"]
            expected_attempt = {
                "id": mode,
                "do_file": f"fixtures/{fixture_id}/{mode}.do",
                "memory_limit_bytes": limit,
                "science_output": f"{mode}.json",
            }
            if dict(attempt) != expected_attempt:
                raise ConstrainedRecipeError("constrained attempt semantics differ")
            if (generated_root / attempt["do_file"]).read_text(encoding="ascii") != attempt_do(expected_command):
                raise ConstrainedRecipeError("constrained source contains extra or changed work")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--development-manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.output.exists():
        raise SystemExit("constrained recipe output already exists")
    args.output.mkdir(parents=True)
    compile_recipes(args.development_manifest.resolve(), args.output.resolve())
    print("FEPPML_CONSTRAINED_RECIPES_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
