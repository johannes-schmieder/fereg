#!/usr/bin/env python3
"""Compile the frozen 33-cell PPML matrix into deterministic Stata recipes.

The compiler emits source programs only. Dataset and artifact hashes are added
by SCC setup after exact builds and one untimed licensed-Stata materialization.
Normal timing and allocation-audit commands are deliberately distinct.
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


class RecipeError(RuntimeError):
    pass


def _write_new(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise RecipeError(f"refusing to overwrite recipe: {path}") from error
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(text)
        target.flush()
        os.fsync(target.fileno())


def _controls(count: int) -> str:
    return " ".join(f"c.c{index}" for index in range(1, count + 1))


def _absorb(design: str, *, save: bool) -> str:
    names = {
        "no_fe": [], "one_fe": ["fe1"],
        "two_fe_standard": ["fe1", "fe2"],
        "two_fe_lowmob": ["fe1", "fe2"],
        "two_fe_disconnected": ["fe1", "fe2"],
        "three_fe_standard": ["fe1", "fe2", "fe3"],
        "two_fe_slopes": ["fe1##c.slope1", "fe2##c.slope2"],
    }[design]
    if save:
        expanded: list[str] = []
        channel = 1
        for name in names:
            if "##c." in name:
                group, slope = name.split("##c.", 1)
                expanded.extend([
                    f"__q_fe{channel}={group}",
                    f"__q_fe{channel + 1}={group}#c.{slope}",
                ])
                channel += 2
            else:
                expanded.append(f"__q_fe{channel}={name}")
                channel += 1
        names = expanded
    return "" if not names else f" absorb({' '.join(names)})"


def _clusters(vce: str) -> list[str]:
    return [f"cluster{index}" for index in range(1, int(vce.removeprefix("cluster")) + 1)] if vce.startswith("cluster") else []


def expected_status_map(cell: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Freeze comparator metadata from the recipe, never from candidate output."""
    result = []
    for index in range(1, int(cell["controls"]) + 1):
        omitted = cell["finite_design"] == "collinear" and index == 2
        result.append({
            "name": f"o.c{index}" if omitted else f"c{index}",
            "status_code": 6 if omitted else 1,
        })
    absorbed_constant = cell["design"] != "no_fe"
    result.append({
        "name": "o._cons" if absorbed_constant else "_cons",
        "status_code": 7 if absorbed_constant else 1,
    })
    return result


def commands(cell: Mapping[str, Any]) -> dict[str, str]:
    controls = _controls(cell["controls"])
    varlist = " ".join(value for value in ("y", controls) if value)
    save = cell["output"] == "individual_fe"
    absorb = _absorb(cell["design"], save=save)
    clusters = _clusters(cell["vce"])
    adjustment = ""
    if cell["outcome"] in {"easy_high_zero_offset", "long_offset"}:
        adjustment = " offset(offset)"
    elif cell["outcome"] == "long_exposure":
        adjustment = " exposure(exposure)"
    weight_hidden = ""
    weight_public = ""
    if cell["weight"] == "fweight":
        weight_hidden, weight_public = " fweight(fw)", " [fw=fw]"
    elif cell["weight"] == "pweight":
        weight_hidden, weight_public = " pweight(pw)", " [pw=pw]"
    vce_hidden = " vce(robust)" if not clusters else f" vce(cluster {' '.join(clusters)})"
    vce_public = " vce(robust)" if not clusters else f" vce(cluster {' '.join(clusters)})"
    candidate_outputs = {
        "fit": "",
        "prediction": " mu(__q_mu)",
        "total_absorbed_effect": " d(__q_d)",
        "individual_fe": " d(__q_d)",
    }[cell["output"]]
    comparator_outputs = (
        " d(__q_d)"
        if cell["output"] in {"total_absorbed_effect", "individual_fe"}
        or (cell["output"] == "prediction" and cell["design"] != "no_fe")
        else ""
    )
    common = f"{absorb}{adjustment} separation(fe simplex ir) memory({cell['memory']}) threads({cell['requested_threads']}) deterministic dropreason(__q_drop){candidate_outputs}"
    candidate = f"_fereg_estimate_ppml {varlist},{weight_hidden}{vce_hidden}{common}".replace(", ", ", ")
    candidate_audit = candidate + " allocationaudit"
    comparator_options = f"{absorb}{adjustment}{vce_public} separation(fe simplex ir){comparator_outputs}"
    comparator = f"ppmlhdfe {varlist}{weight_public},{comparator_options}"
    return {"candidate_science": candidate_audit, "candidate_timing": candidate, "candidate_allocation_audit": candidate_audit, "comparator_science": comparator, "comparator_timing": comparator}


def generator_do(cell: Mapping[str, Any], seed: int) -> str:
    n = int(cell["scale_spec"]["n"])
    controls = int(cell["controls"])
    lines = [
        "version 19.0", "clear all", "set more off", f"set seed {seed}",
        f"set obs {n}", "generate long obs_id = _n",
        "generate long fe1 = 1 + mod(_n - 1, 2000)",
        "generate long fe2 = 1 + mod(floor((_n - 1) / 17), 2500)",
        "generate long fe3 = 1 + mod(floor((_n - 1) / 31), 500)",
        "generate long cluster1 = 1 + mod(_n - 1, 997)",
        "generate long cluster2 = 1 + mod(floor((_n - 1) / 7), 991)",
        "generate long cluster3 = 1 + mod(floor((_n - 1) / 11), 983)",
        "generate long cluster4 = 1 + mod(floor((_n - 1) / 13), 977)",
        "generate long cluster5 = 1 + mod(floor((_n - 1) / 19), 971)",
        "generate double slope1 = mod(37 * _n, 997) / 498 - 1",
        "generate double slope2 = mod(53 * _n, 991) / 495 - 1",
    ]
    if cell["design"] == "two_fe_lowmob" or cell["design"] == "two_fe_slopes":
        lines.append("replace fe2 = 1 + mod(fe1 + mod(_n, 3), 2500)")
    if cell["design"] == "two_fe_disconnected":
        lines.extend(["replace fe1 = fe1 + 2000 if mod(_n, 2)", "replace fe2 = fe2 + 2500 if mod(_n, 2)"])
    for index in range(1, controls + 1):
        lines.append(f"generate double c{index} = mod({2*index+1} * _n, 1009) / 504 - 1")
    if cell["finite_design"] == "collinear" and controls >= 2:
        lines.append("replace c2 = 2 * c1")
    if cell["finite_design"] == "ill_conditioned" and controls >= 2:
        lines.append("replace c2 = c1 + 1e-7 * (mod(_n, 7) - 3)")
    lines.extend([
        "generate double offset = 0.02 * (mod(_n, 9) - 4)",
        "generate double exposure = exp(offset)",
        "generate long fw = 1 + mod(_n, 3)",
        "generate double pw = 0.5 + mod(_n, 11) / 5",
        "generate double __eta = 0.15 + 0.2 * slope1 - 0.1 * slope2",
    ])
    if controls:
        lines.append("replace __eta = __eta + 0.08 * c1")
    if cell["design"] != "no_fe":
        lines.append("replace __eta = __eta + 0.0002 * fe1")
    if cell["design"] in {"two_fe_standard", "two_fe_lowmob", "two_fe_disconnected", "three_fe_standard", "two_fe_slopes"}:
        lines.append("replace __eta = __eta - 0.0001 * fe2")
    if cell["design"] == "three_fe_standard":
        lines.append("replace __eta = __eta + 0.0003 * fe3")
    if cell["outcome"] in {"easy_high_zero_offset", "long_offset"}:
        lines.append("replace __eta = __eta + offset")
    elif cell["outcome"] == "long_exposure":
        lines.append("replace __eta = __eta + log(exposure)")
    lines.extend([
        "generate double __mu = exp(__eta)",
        "generate double y = floor(__mu) + (mod(_n, 1000) < 1000 * (__mu - floor(__mu)))",
    ])
    zero_threshold = {"easy_low_zero": 100, "easy_high_zero_offset": 800, "long_exposure": 350, "separated_high_zero": 800, "long_offset": 550}[cell["outcome"]]
    lines.append(f"replace y = 0 if mod(97 * _n, 1000) < {zero_threshold}")
    if cell["outcome"] in {"separated_high_zero", "long_offset"}:
        lines.append("replace y = 0 if fe1 <= 400")
    lines.extend(["drop __eta __mu", "compress", "sort obs_id", "save \"`1'\", replace", 'display as result "FEPPML_DATASET_COMPLETE"'])
    return "\n".join(lines) + "\n"


def _child_preamble(command: str, *, candidate: bool) -> list[str]:
    lines = [
        "version 19.0", "clear all", "set more off",
        "args data artifact result ready start done requested comparator_limit pair temperature cell arm",
        "use \"`data'\", clear",
        "assert obs_id == _n",
        "generate byte __q_eligible = 1",
        "local artifact_dir = substr(\"`artifact'\", 1, strrpos(\"`artifact'\", \"/\") - 1)",
    ]
    if candidate:
        lines.extend([
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
        ])
    else:
        lines.extend([
            "foreach package in require ftools reghdfe ppmlhdfe {",
            "    adopath ++ \"`artifact_dir'/`package'\"",
            "    adopath ++ \"`artifact_dir'/`package'/src\"",
            "}",
            "findfile ppmlhdfe.ado",
            "local __q_resolved_ado = r(fn)",
            "assert substr(\"`__q_resolved_ado'\", 1, strlen(\"`artifact_dir'/ppmlhdfe/\")) == \"`artifact_dir'/ppmlhdfe/\"",
            "foreach dependency in reghdfe ftools require {",
            "    findfile `dependency'.ado",
            "    local __q_resolved_`dependency' = r(fn)",
            "    assert substr(\"`__q_resolved_`dependency''\", 1, strlen(\"`artifact_dir'/`dependency'/\")) == \"`artifact_dir'/`dependency'/\"",
            "}",
            "local __q_resolved_plugin \"`artifact'\"",
            "local __q_comparator_processors = min(`requested', `comparator_limit')",
            "set processors `__q_comparator_processors'",
        ])
    lines.extend([
        "file open __resolution using \"RESOLUTION\", write text",
        "file write __resolution \"ADO|`__q_resolved_ado'\" _n",
        "file write __resolution \"ARTIFACT|`__q_resolved_plugin'\" _n",
        *( [
            "file write __resolution \"DEPENDENCY|`__q_resolved_parse'\" _n",
            "file write __resolution \"DEPENDENCY|`__q_resolved_load'\" _n",
        ] if candidate else [
            "file write __resolution \"DEPENDENCY|`__q_resolved_reghdfe'\" _n",
            "file write __resolution \"DEPENDENCY|`__q_resolved_ftools'\" _n",
            "file write __resolution \"DEPENDENCY|`__q_resolved_require'\" _n",
        ]),
        "file close __resolution",
        "file open __ready using \"`ready'\", write text",
        "file write __ready \"FEPPML_STATA_READY|`cell'|`arm'\" _n",
        "file close __ready",
        "file open __start using \"`start'\", write text",
        "file write __start \"FEPPML_STATA_START|`cell'|`arm'\" _n",
        "file close __start",
        "local __q_N_physical = _N",
        command,
    ])
    return lines


def _emit_number(lines: list[str], record: str, key: str, expression: str) -> None:
    lines.extend([
        f"local __q_rendered : display %24.17g ({expression})",
        f"file write __q_protocol \"{record}\" _tab \"{key}\" _tab \"`__q_rendered'\" _n",
    ])


def _emit_text(
    lines: list[str],
    record: str,
    key: str,
    value: str,
    *,
    fixed: str | None = None,
) -> None:
    fixed_field = f' _tab "{fixed}"' if fixed is not None else ""
    lines.extend([
        f"local __q_text {value}",
        "local __q_text_length = strlen(`\"`__q_text'\"')",
        "local __q_rendered : display %24.17g (`__q_text_length')",
        f"file write __q_protocol \"{record}\" _tab \"{key}\" _tab \"`__q_rendered'\" _tab `\"`__q_text'\"'{fixed_field} _n",
    ])


def timing_do(command: str, *, candidate: bool, output: str) -> str:
    effective = "e(feppml_runtime)[11,1]" if candidate else "c(processors)"
    lines = _child_preamble(command, candidate=candidate)
    if not candidate and output == "prediction":
        lines.append("predict double __q_mu if e(sample), mu")
    lines.extend([
        f"local effective = {effective}",
        "local processors = c(processors)",
        "file open __result using \"`result'\", write text",
        "file write __result \"{\\\"control\\\":{\\\"effective_threads\\\":`effective',\\\"stata_processors\\\":`processors'},\\\"stata\\\":{\\\"licensed\\\":true,\\\"version\\\":\\\"`c(stata_version)' `c(flavor)'\\\"},\\\"complete_sentinel\\\":\\\"FEPPML_STATA_ARM_COMPLETE\\\"}\" _n",
        "file close __result",
        "file open __done using \"`done'\", write text",
        "file write __done \"FEPPML_STATA_DONE|`cell'|`arm'|SUCCESS\" _n",
        "file close __done",
        'display as result "FEPPML_STATA_ARM_COMPLETE"',
    ])
    return "\n".join(lines) + "\n"


def science_do(
    cell: Mapping[str, Any], command: str, *, candidate: bool,
    statuses: Sequence[Mapping[str, Any]], probe_rows: int,
) -> str:
    """Emit one native science arm using the closed line protocol."""
    lines = _child_preamble(command, candidate=candidate)
    expected_names = " ".join(str(record["name"]) for record in statuses)
    expected_codes = [int(record["status_code"]) for record in statuses]
    clusters = _clusters(str(cell["vce"]))
    if not candidate and cell["output"] == "prediction":
        lines.append("predict double __q_mu if e(sample), mu")
    if not candidate and cell["output"] in {"total_absorbed_effect", "individual_fe"}:
        lines.append("replace __q_d = __q_d + _b[_cons] if e(sample)")
    if not candidate and cell["output"] == "individual_fe":
        lines.append("replace __q_fe1 = __q_fe1 + _b[_cons] if e(sample)")
    if candidate:
        lines.extend([
            "generate byte __q_sample = e(sample)",
            "quietly count if __q_eligible & !__q_sample",
            "local __q_eligible_nonretained = r(N)",
            "quietly count if __q_eligible & !__q_sample & __q_drop > 0 & __q_drop < . & __q_drop == int(__q_drop)",
            "local __q_positive_raw = r(N)",
            "quietly count if (__q_eligible & !__q_sample & !(__q_drop > 0 & __q_drop < . & __q_drop == int(__q_drop))) | (__q_sample & __q_drop != 0)",
            "local __q_invalid_raw = r(N)",
            "matrix __q_b = e(b)", "matrix __q_V = e(V)",
            "matrix __q_status = e(feppml_status)",
            "local __q_native_names : colfullnames e(b)",
            f"if `\"`__q_native_names'\"' != `\"{expected_names}\"' exit 459",
        ])
    else:
        lines.extend([
            "generate byte __q_sample = e(sample)",
            "generate long __q_drop = 0",
            "quietly count if __q_eligible & !__q_sample",
            "local __q_eligible_nonretained = r(N)",
            "local __q_positive_raw = 0",
            "quietly count if __q_drop != 0",
            "local __q_invalid_raw = r(N)",
            "matrix __q_native_b = e(b)", "matrix __q_native_V = e(V)",
            "_ms_omit_info __q_native_b",
            "matrix __q_native_omit = r(omit)",
            f"if colsof(__q_native_b) != {len(statuses)} exit 459",
            f"matrix __q_b = J(1, {len(statuses)}, 0)",
            f"matrix __q_V = J({len(statuses)}, {len(statuses)}, 0)",
        ])
        for index, record in enumerate(statuses, 1):
            name = str(record["name"])
            native_name = "_cons" if int(record["status_code"]) == 7 else name
            lines.extend([
                f"local __q_native_{index} = colnumb(__q_native_b, \"{native_name}\")",
                f"if missing(`__q_native_{index}') exit 459",
                f"if __q_native_omit[1,`__q_native_{index}'] != {1 if int(record['status_code']) == 6 else 0} exit 459",
            ])
            if int(record["status_code"]) == 1:
                lines.append(f"matrix __q_b[1,{index}] = __q_native_b[1,`__q_native_{index}']")
            elif int(record["status_code"]) == 6:
                lines.extend([
                    f"if __q_native_b[1,`__q_native_{index}'] != 0 exit 459",
                    "forvalues __q_native_column = 1/`=colsof(__q_native_b)' {",
                    f"    if __q_native_V[`__q_native_{index}',`__q_native_column'] != 0 exit 459",
                    f"    if __q_native_V[`__q_native_column',`__q_native_{index}'] != 0 exit 459",
                    "}",
                ])
        lines.append(f"if e(rank) != {sum(int(record['status_code']) in (1, 7) for record in statuses)} exit 459")
        for row, row_record in enumerate(statuses, 1):
            if int(row_record["status_code"]) != 1:
                continue
            for column, column_record in enumerate(statuses, 1):
                if int(column_record["status_code"]) != 1:
                    continue
                lines.append(
                    f"matrix __q_V[{row},{column}] = __q_native_V[`__q_native_{row}',`__q_native_{column}']"
                )
        lines.extend([
            f"matrix colnames __q_b = {expected_names}",
            f"matrix colnames __q_V = {expected_names}",
            f"matrix rownames __q_V = {expected_names}",
        ])

    lines.extend([
        "quietly count if __q_sample",
        "local __q_N_retained_final = r(N)",
        "local __q_N_physical_final = `__q_N_physical'",
    ])
    if cell["weight"] == "fweight":
        lines.extend([
            "quietly summarize fw, meanonly",
            "local __q_N_effective_final = r(sum)",
        ])
    else:
        lines.append("local __q_N_effective_final = `__q_N_physical_final'")
    if not candidate:
        lines.extend([
            "generate double __q_null_weight = __q_sample",
            "local __q_fitted_scale = 1",
        ])
        if cell["weight"] == "fweight":
            lines.append("replace __q_null_weight = fw if __q_sample")
        elif cell["weight"] == "pweight":
            lines.extend([
                "quietly summarize pw if __q_sample, meanonly",
                "local __q_fitted_scale = `__q_N_retained_final' / r(sum)",
                "replace __q_null_weight = pw * `__q_N_retained_final' / r(sum) if __q_sample",
            ])
        lines.extend([
            "generate double __q_null_qy = __q_null_weight * y if __q_sample",
            "quietly summarize __q_null_qy if __q_sample, meanonly",
            "local __q_null_sum_qy = r(sum)",
            "quietly summarize __q_null_weight if __q_sample, meanonly",
            "local __q_null_sum_q = r(sum)",
            "local __q_null_mu = `__q_null_sum_qy' / `__q_null_sum_q'",
            "generate double __q_null_ll_i = __q_null_weight * (y * ln(`__q_null_mu') - `__q_null_mu' - lngamma(y + 1)) if __q_sample",
            "quietly summarize __q_null_ll_i if __q_sample, meanonly",
            "local __q_null_ll = r(sum)",
        ])

    lines.extend([
        "generate byte __q_mask = cond(!__q_eligible, 0, cond(__q_sample, 1, 2))",
        "quietly count", "local __q_rows = r(N)",
        "quietly count if __q_eligible", "local __q_eligible_count = r(N)",
        "quietly count if __q_sample", "local __q_retained_count = r(N)",
        "quietly count if __q_mask == 2", "local __q_separated_count = r(N)",
        "local __q_mask_chunk = 8192",
        "mata:",
        "__q_mask_fh = fopen(st_local(\"result\") + \".mask\", \"w\")",
        "if (__q_mask_fh < 0) _error(603)",
        "__q_mask_n = st_nobs()",
        "for (__q_mask_lo=1; __q_mask_lo<=__q_mask_n; __q_mask_lo=__q_mask_lo+8192) {",
        "    __q_mask_hi = min((__q_mask_lo+8191, __q_mask_n))",
        "    __q_mask_codes = st_data((__q_mask_lo::__q_mask_hi), \"__q_mask\")",
        "    fput(__q_mask_fh, invtokens(strofreal(__q_mask_codes', \"%1.0f\"), \"\"))",
        "}",
        "fclose(__q_mask_fh)",
        "end",
        "file open __q_protocol using \"`result'\", write text",
        f"file write __q_protocol \"HEADER\" _tab \"FEPPML_STATA_PROTOCOL\" _tab \"1\" _tab \"{'candidate' if candidate else 'comparator'}\" _n",
    ])
    _emit_number(lines, "RESULT", "rc", "0")
    _emit_number(lines, "RESULT", "converged", "e(converged)")

    lines.extend([
        f"local __q_expected_names {expected_names}",
        f"local __q_dimension = {len(statuses)}",
        "forvalues __q_index = 1/`__q_dimension' {",
        "    local __q_name : word `__q_index' of `__q_expected_names'",
        "    local __q_name_length = strlen(\"`__q_name'\")",
        "    local __q_index_text : display %24.17g (`__q_index')",
        "    local __q_length_text : display %24.17g (`__q_name_length')",
    ])
    if candidate:
        lines.append("    local __q_status_text : display %24.17g (__q_status[`__q_index',2])")
    else:
        lines.append('    local __q_status_text "."')
    lines.extend([
        "    local __q_value_text : display %24.17g (__q_b[1,`__q_index'])",
        "    file write __q_protocol \"COEF\" _tab \"`__q_index_text'\" _tab \"`__q_length_text'\" _tab \"`__q_name'\" _tab \"`__q_status_text'\" _tab \"`__q_value_text'\" _tab \"B\" _n",
        "}",
        "forvalues __q_row = 1/`__q_dimension' {",
        "    forvalues __q_column = 1/`__q_dimension' {",
        "        local __q_row_text : display %24.17g (`__q_row')",
        "        local __q_column_text : display %24.17g (`__q_column')",
        "        local __q_value_text : display %24.17g (__q_V[`__q_row',`__q_column'])",
        "        file write __q_protocol \"VCE\" _tab \"`__q_row_text'\" _tab \"`__q_column_text'\" _tab \"`__q_value_text'\" _n",
        "    }",
        "}",
    ])

    dof_sources = {
        "N_physical": "e(N_physical)" if candidate else "`__q_N_physical_final'",
        "N_effective": "e(N_effective)" if candidate else "`__q_N_effective_final'",
        "finite_rank": "e(rank)", "absorbed_rank": "e(df_a)",
        "df": "e(df_resid)" if candidate else "e(df)",
    }
    for key, expression in dof_sources.items():
        _emit_number(lines, "DOF", key, expression)
    for index, _cluster in enumerate(clusters, 1):
        lines.extend([
            f"local __q_rendered_index : display %24.17g ({index})",
            f"local __q_rendered : display %24.17g (e(N_clust{index}))",
            "file write __q_protocol \"CLUSTER\" _tab \"`__q_rendered_index'\" _tab \"`__q_rendered'\" _n",
        ])
    for key, expression in (
        ("deviance", "e(deviance)" if candidate else "e(deviance) * `__q_fitted_scale'"),
        ("log_likelihood", "e(ll)" if candidate else "e(ll) * `__q_fitted_scale'"),
        ("null_log_likelihood", "e(ll_0)" if candidate else "`__q_null_ll'"),
        ("pseudo_r2", "e(r2_p)" if candidate else "1 - (e(ll) * `__q_fitted_scale') / `__q_null_ll'"),
    ):
        _emit_number(lines, "OBJECTIVE", key, expression)

    lines.extend([
        "local __q_rows_text : display %24.17g (`__q_rows')",
        "local __q_eligible_text : display %24.17g (`__q_eligible_count')",
        "local __q_retained_text : display %24.17g (`__q_retained_count')",
        "local __q_separated_text : display %24.17g (`__q_separated_count')",
        "local __q_chunk_text : display %24.17g (`__q_mask_chunk')",
        "file write __q_protocol \"MASK\" _tab \"ASCII_MASK_V1\" _tab \"`__q_rows_text'\" _tab \"`__q_eligible_text'\" _tab \"`__q_retained_text'\" _tab \"`__q_separated_text'\" _tab \"`__q_chunk_text'\" _tab \"SIDECAR\" _n",
        f"local __q_mask_probe_count = min(_N, {probe_rows})",
        "forvalues __q_probe = 1/`__q_mask_probe_count' {",
        "    local __q_position = cond(`__q_mask_probe_count' == 1, 1, 1 + floor((`__q_probe' - 1) * (_N - 1) / (`__q_mask_probe_count' - 1)))",
        "    local __q_probe_text : display %24.17g (`__q_probe')",
        "    local __q_position_text : display %24.17g (`__q_position')",
        "    local __q_eligible_value : display %24.17g (__q_eligible[`__q_position'])",
        "    local __q_retained_value : display %24.17g (__q_sample[`__q_position'])",
        "    local __q_reason_value : display %24.17g (cond(__q_mask[`__q_position'] == 2, 1, 0))",
        "    file write __q_protocol \"MASK_PROBE\" _tab \"`__q_probe_text'\" _tab \"`__q_position_text'\" _tab \"`__q_eligible_value'\" _tab \"`__q_retained_value'\" _tab \"`__q_reason_value'\" _n",
        "}",
        "local __q_nonretained_text : display %24.17g (`__q_eligible_nonretained')",
        "local __q_positive_text : display %24.17g (`__q_positive_raw')",
        "local __q_invalid_text : display %24.17g (`__q_invalid_raw')",
        f"file write __q_protocol \"DROP_REASON_AUDIT\" _tab \"{'candidate' if candidate else 'comparator'}\" _tab \"`__q_nonretained_text'\" _tab \"`__q_positive_text'\" _tab \"`__q_invalid_text'\" _n",
    ])

    saved_output_variables = {
        "fit": [], "prediction": ["__q_mu"], "total_absorbed_effect": ["__q_d"],
        "individual_fe": [f"__q_fe{index}" for index in range(1, int(cell["design_spec"]["fe_dimensions"]) + int(cell["design_spec"]["slope_channels"]) + 1)],
    }[str(cell["output"])]
    output_variables = list(saved_output_variables)
    output_mode = "direct"
    if cell["output"] == "individual_fe":
        output_mode = "saved_channels"
    output_record = {
        "prediction": "prediction_probe", "total_absorbed_effect": "total_absorbed_effect_probe",
        "individual_fe": "individual_fe_probe",
    }.get(str(cell["output"]))
    if output_variables:
        lines.extend([
            "generate long __q_retained_rank = sum(__q_sample)",
            f"local __q_output_probe_count = min(`__q_retained_count', {probe_rows})",
            "local __q_output_index = 0",
        ])
        for variable in output_variables:
            lines.extend([
                "forvalues __q_probe = 1/`__q_output_probe_count' {",
                "    local __q_target_rank = cond(`__q_output_probe_count' == 1, 1, 1 + floor((`__q_probe' - 1) * (`__q_retained_count' - 1) / (`__q_output_probe_count' - 1)))",
                "    quietly summarize obs_id if __q_sample & __q_retained_rank == `__q_target_rank', meanonly",
                "    local __q_position = r(min)",
                "    local ++__q_output_index",
                "    local __q_output_index_text : display %24.17g (`__q_output_index')",
                f"    local __q_output_value : display %24.17g ({variable}[`__q_position'])",
                f"    file write __q_protocol \"PROBE\" _tab \"{output_record}\" _tab \"`__q_output_index_text'\" _tab \"`__q_output_value'\" _n",
                "}",
            ])
    lines.append(
        f'file write __q_protocol "OUTPUT" _tab "{cell["output"]}" _tab "{output_mode}" '
        '_tab "`__q_output_index\'" _tab "'
        + str(len(saved_output_variables) if cell["output"] == "individual_fe" else len(output_variables))
        + '" _n'
        if output_variables
        else f'file write __q_protocol "OUTPUT" _tab "{cell["output"]}" _tab "direct" _tab "0" _tab "0" _n'
    )

    if candidate:
        certificate_values = {
            "two_consecutive_iterations": "e(feppml_runtime)[77,1]",
            "strict_final_solve": "e(feppml_certificate)[16,1]",
            "rank_frozen": "e(feppml_runtime)[78,1]",
            "score_linf": "e(feppml_certificate)[1,1]",
            "fe_score_linf": "e(feppml_certificate)[2,1]",
            "vce_symmetry_linf": "e(feppml_certificate)[14,1]",
            "reconstruction_linf": "max(max(e(feppml_certificate)[6,1],e(feppml_certificate)[7,1]),max(e(feppml_certificate)[8,1],max(e(feppml_certificate)[9,1],e(feppml_certificate)[10,1])))",
        }
    else:
        lines.append("mata: st_numscalar(\"__q_vce_symmetry\", max(abs(st_matrix(\"__q_V\") :- st_matrix(\"__q_V\")'))) ")
        certificate_values = {
            "two_consecutive_iterations": "0", "strict_final_solve": "0", "rank_frozen": "0",
            "score_linf": "8.9884656743115785e307", "fe_score_linf": "8.9884656743115785e307",
            "vce_symmetry_linf": "scalar(__q_vce_symmetry)", "reconstruction_linf": "8.9884656743115785e307",
        }
    for key in ("two_consecutive_iterations", "strict_final_solve", "rank_frozen"):
        _emit_number(lines, "CERT_BOOL", key, certificate_values[key])
    for key in ("score_linf", "fe_score_linf", "vce_symmetry_linf", "reconstruction_linf"):
        _emit_number(lines, "CERT_NUM", key, certificate_values[key])

    candidate_matrices = (
        "e(b)", "e(V)", "e(feppml_status)", "e(feppml_fit)", "e(feppml_dof)",
        "e(feppml_fe)", "e(feppml_cluster)", "e(feppml_subset)",
        "e(feppml_separation)", "e(feppml_inference)",
        "e(feppml_drop_counts)", "e(feppml_separation_sequence)",
        "e(feppml_certificate)", "e(feppml_runtime)", "e(feppml_receipt)",
    )
    matrices = candidate_matrices if candidate else ("e(b)", "e(V)")
    scalars = (
        "e(N)", "e(N_physical)", "e(N_effective)", "e(N_full)", "e(rank)",
        "e(df_a)", "e(df_resid)", "e(deviance)", "e(ll)", "e(ll_0)",
        "e(r2_p)", "e(ic)", "e(step_halvings)", "e(separated)", "e(converged)",
        *(f"e(N_clust{index})" for index in range(1, len(clusters) + 1)),
    ) if candidate else (
        "e(N)", "e(rank)", "e(df_a)", "e(df)", "e(deviance)", "e(ll)",
        "e(ll_0)", "e(r2_p)", "e(converged)",
        *(f"e(N_clust{index})" for index in range(1, len(clusters) + 1)),
    )
    variables = ["e(sample)", "__q_drop"] + saved_output_variables
    if not candidate:
        if cell["output"] == "prediction" and cell["design"] != "no_fe":
            variables.append("__q_d")
        variables.append("y")
        if cell["weight"] != "none":
            variables.append({"fweight": "fw", "pweight": "pw"}[cell["weight"]])
    for kind, values in (("matrix", matrices), ("scalar", scalars), ("variable", variables)):
        for value in values:
            lines.extend([
                f"local __q_source_length : display %24.17g ({len(value)})",
                f"file write __q_protocol \"SOURCE\" _tab \"{kind}\" _tab \"`__q_source_length'\" _tab \"{value}\" _tab \"NATIVE\" _n",
            ])
    _emit_number(lines, "RECEIPT", "complete", "e(feppml_receipt)[1,33]" if candidate else "0")
    _emit_number(lines, "STATA", "licensed", "1")
    _emit_text(
        lines,
        "STATA",
        "version",
        "`\"`c(stata_version)' `c(flavor)'\"'",
        fixed="LICENSED",
    )

    if candidate:
        _emit_text(lines, "DIAG_TEXT", "requested_memory", f'"{cell["memory"]}"')
        lines.extend([
            "local __q_effective_route = cond(e(feppml_runtime)[6,1] == 1, \"fast\", cond(e(feppml_runtime)[6,1] == 2, \"low\", \"invalid\"))",
        ])
        _emit_text(lines, "DIAG_TEXT", "effective_route", '"`__q_effective_route\'"')
        lines.extend([
            "local __q_linear_route = cond(e(feppml_runtime)[7,1] == 2, \"map\", cond(e(feppml_runtime)[7,1] == 3, \"lsmr\", cond(e(feppml_runtime)[7,1] == 4, \"cg\", cond(e(feppml_runtime)[7,1] == 5, \"irons_tuck\", cond(e(feppml_runtime)[7,1] == 6, \"squarem\", cond(e(feppml_runtime)[7,1] == 7, \"cmg\", \"invalid\"))))))",
        ])
        _emit_text(lines, "DIAG_TEXT", "linear_route", '"`__q_linear_route\'"')
        for key, row in (
            ("requested_threads", 10), ("effective_threads", 11), ("fallback_mask", 8),
            ("fallback_count", 9), ("outer_iterations", 2), ("inner_iterations", 3),
            ("step_halvings", 4),
        ):
            _emit_number(lines, "DIAG_NUM", key, f"e(feppml_runtime)[{row},1]")
        _emit_number(lines, "DIAG_BOOL", "workspace_allocation_free", "e(feppml_runtime)[24,1]")
        _emit_number(lines, "DIAG_BOOL", "whole_iteration_allocation_free", "e(feppml_runtime)[25,1]")
        phase_names = (
            "preparation", "separation_fe", "separation_simplex", "separation_ir",
            "separation_mu", "weight_response_refresh", "fe_projection", "finite_solve",
            "objective_line_search", "strict_final_solve", "inference", "fe_recovery",
            "publication",
        )
        for row, phase in enumerate(phase_names, 28):
            phase_value = f"e(feppml_runtime)[{row},1]"
            _emit_number(
                lines,
                "PHASE",
                phase,
                f"cond(missing({phase_value}),0,{phase_value})",
            )
        memory_values = {
            "modeled_peak_bytes": "e(feppml_runtime)[14,1]",
            "native_peak_bytes": "e(feppml_runtime)[21,1]",
            "process_peak_rss_bytes": "0",
            "repeated_call_retained_bytes": "e(feppml_runtime)[79,1]",
            "measured_steady_state_iterations": "e(feppml_runtime)[42,1]",
            "allocator_event_count_start": "e(feppml_runtime)[43,1]",
            "allocator_event_count_end": "e(feppml_runtime)[44,1]",
            "allocator_current_bytes_start": "e(feppml_runtime)[46,1]",
            "allocator_current_bytes_end": "e(feppml_runtime)[47,1]",
            "allocator_peak_bytes_start": "e(feppml_runtime)[49,1]",
            "allocator_peak_bytes_end": "e(feppml_runtime)[50,1]",
            "workspace_capacity_start": "e(feppml_runtime)[52,1]+e(feppml_runtime)[53,1]+e(feppml_runtime)[54,1]",
            "workspace_capacity_end": "e(feppml_runtime)[55,1]+e(feppml_runtime)[56,1]+e(feppml_runtime)[57,1]",
            "scratch_bytes_start": "e(feppml_runtime)[68,1]",
            "scratch_bytes_end": "e(feppml_runtime)[69,1]",
            "scratch_files_after": "0",
            "median_steady_iteration_seconds": "e(feppml_runtime)[75,1]/1e9",
        }
        for key, expression in memory_values.items():
            _emit_number(lines, "MEMORY_NUM", key, expression)
        _emit_number(lines, "MEMORY_BOOL", "linear_session_allocation_free", "e(feppml_runtime)[73,1]")
        _emit_number(lines, "MEMORY_BOOL", "fresh_warm_equivalent", "e(feppml_runtime)[74,1]")
        for index, phase in enumerate(phase_names):
            _emit_number(lines, "MEMORY_MODELED", phase, f"e(feppml_runtime)[{83 + index},1]")
            _emit_number(lines, "MEMORY_OBSERVED", phase, f"e(feppml_runtime)[{96 + index},1]")

    lines.extend([
        "file write __q_protocol \"SLOWDOWN\" _tab \"NONE\" _n",
        "file write __q_protocol \"END\" _tab \"FEPPML_STATA_RAW_COMPLETE\" _n",
        "file close __q_protocol",
        "file open __done using \"`done'\", write text",
        "file write __done \"FEPPML_STATA_DONE|`cell'|`arm'|SUCCESS\" _n",
        "file close __done",
        'display as result "FEPPML_STATA_RAW_COMPLETE"',
    ])
    return "\n".join(lines) + "\n"


def validate_science_recipe(
    source: str,
    cell: Mapping[str, Any],
    *,
    candidate: bool,
    statuses: Sequence[Mapping[str, Any]],
) -> None:
    """Fail closed if generated science code loses a frozen proof step."""
    if candidate:
        return
    required = [
        "_ms_omit_info __q_native_b",
        f"if colsof(__q_native_b) != {len(statuses)} exit 459",
        f"if e(rank) != {sum(int(record['status_code']) in (1, 7) for record in statuses)} exit 459",
    ]
    if cell["output"] in {"total_absorbed_effect", "individual_fe"}:
        required.append("replace __q_d = __q_d + _b[_cons] if e(sample)")
    if cell["output"] == "individual_fe":
        required.append("replace __q_fe1 = __q_fe1 + _b[_cons] if e(sample)")
    for index, record in enumerate(statuses, 1):
        if int(record["status_code"]) == 6:
            required.extend([
                f'local __q_native_{index} = colnumb(__q_native_b, "{record["name"]}")',
                f"if __q_native_omit[1,`__q_native_{index}'] != 1 exit 459",
                f"if __q_native_b[1,`__q_native_{index}'] != 0 exit 459",
                f"if __q_native_V[`__q_native_{index}',`__q_native_column'] != 0 exit 459",
                f"if __q_native_V[`__q_native_column',`__q_native_{index}'] != 0 exit 459",
            ])
    missing = [proof for proof in required if source.count(proof) != 1]
    if missing:
        raise RecipeError(f"generated comparator science proof is incomplete: {missing}")


def compile_recipes(manifest_path: Path, output: Path) -> Mapping[str, Any]:
    manifest = performance.load_manifest(manifest_path)
    cells = performance.expand_cells(manifest)
    seed_base = int(manifest["generator_contract"]["seed_namespace"])
    probe_rows = int(manifest["generator_contract"]["probe_rows"])
    records = []
    written_datasets = set()
    for cell in cells:
        cell_id = cell["cell_id"]
        dataset_key = cell.get("paired_fit_case") or cell.get("equivalence_group") or cell["id"]
        dataset_id = f"{dataset_key}_{cell['scale']}"
        seed = seed_base + int.from_bytes(hashlib.sha256(dataset_id.encode("ascii")).digest()[:4], "big") % 100_000_000
        relative = Path("datasets") / dataset_id / "generate.do"
        if dataset_id not in written_datasets:
            _write_new(output / relative, generator_do(cell, seed))
            written_datasets.add(dataset_id)
        cell_commands = commands(cell)
        statuses = expected_status_map(cell)
        candidate_science = Path("cells") / cell_id / "candidate_science.do"
        comparator_science = Path("cells") / cell_id / "comparator_science.do"
        candidate_timing = Path("cells") / cell_id / "candidate_timing.do"
        comparator_timing = Path("cells") / cell_id / "comparator_timing.do"
        candidate_source = science_do(cell, cell_commands["candidate_science"], candidate=True, statuses=statuses, probe_rows=probe_rows)
        comparator_source = science_do(cell, cell_commands["comparator_science"], candidate=False, statuses=statuses, probe_rows=probe_rows)
        validate_science_recipe(candidate_source, cell, candidate=True, statuses=statuses)
        validate_science_recipe(comparator_source, cell, candidate=False, statuses=statuses)
        _write_new(output / candidate_science, candidate_source)
        _write_new(output / comparator_science, comparator_source)
        _write_new(output / candidate_timing, timing_do(cell_commands["candidate_timing"], candidate=True, output=cell["output"]))
        _write_new(output / comparator_timing, timing_do(cell_commands["comparator_timing"], candidate=False, output=cell["output"]))
        cluster_dimensions = len(_clusters(cell["vce"]))
        saved_channels = (
            int(cell["design_spec"]["fe_dimensions"])
            + int(cell["design_spec"]["slope_channels"])
            if cell["output"] == "individual_fe" else 0
        )
        output_variables = {
            "fit": [], "prediction": ["__q_mu"],
            "total_absorbed_effect": ["__q_d"],
            "individual_fe": [f"__q_fe{index}" for index in range(1, saved_channels + 1)],
        }[cell["output"]]
        candidate_provenance = {
            "matrices": [
                "e(b)", "e(V)", "e(feppml_status)", "e(feppml_fit)",
                "e(feppml_dof)", "e(feppml_fe)", "e(feppml_cluster)",
                "e(feppml_subset)", "e(feppml_separation)",
                "e(feppml_inference)", "e(feppml_drop_counts)",
                "e(feppml_separation_sequence)", "e(feppml_certificate)",
                "e(feppml_runtime)", "e(feppml_receipt)",
            ],
            "scalars": [
                "e(N)", "e(N_physical)", "e(N_effective)", "e(N_full)",
                "e(rank)", "e(df_a)", "e(df_resid)", "e(deviance)",
                "e(ll)", "e(ll_0)", "e(r2_p)", "e(ic)",
                "e(step_halvings)", "e(separated)", "e(converged)",
                *(f"e(N_clust{index})" for index in range(1, cluster_dimensions + 1)),
            ],
            "variables": ["e(sample)", "__q_drop", *output_variables],
            "receipt_complete": True,
        }
        comparator_provenance = {
            "matrices": ["e(b)", "e(V)"],
            "scalars": [
                "e(N)", "e(rank)", "e(df_a)", "e(df)", "e(deviance)",
                "e(ll)", "e(ll_0)", "e(r2_p)", "e(converged)",
                *(f"e(N_clust{index})" for index in range(1, cluster_dimensions + 1)),
            ],
            "variables": [
                "e(sample)", "__q_drop", *output_variables,
                *(["__q_d"] if cell["output"] == "prediction" and cell["design"] != "no_fe" else []),
                "y",
                *([] if cell["weight"] == "none" else [
                    {"fweight": "fw", "pweight": "pw"}[cell["weight"]]
                ]),
            ],
            "receipt_complete": False,
        }
        records.append({
            "cell_id": cell_id, "dataset_id": dataset_id, "seed": seed,
            "n": cell["scale_spec"]["n"], "generator": relative.as_posix(),
            "candidate_science_do": candidate_science.as_posix(),
            "comparator_science_do": comparator_science.as_posix(),
            "candidate_timing_do": candidate_timing.as_posix(),
            "comparator_timing_do": comparator_timing.as_posix(),
            "expected_status_map": statuses, "expected_output": cell["output"],
            "probe_rows": probe_rows,
            "expected_cluster_dimensions": cluster_dimensions,
            "expected_saved_channels": saved_channels,
            "candidate_source_provenance": candidate_provenance,
            "comparator_source_provenance": comparator_provenance,
            "commands": cell_commands,
        })
    document = {
        "schema_version": 1,
        "recipe_type": "feppml-qualification-recipes-v1",
        "development_manifest_sha256": performance.hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        "protocol": {
            "name": "FEPPML_STATA_PROTOCOL", "version": 1,
            "numeric_format": "%24.17g", "mask_format": "ASCII_MASK_V1",
            "terminal_sentinel": "FEPPML_STATA_RAW_COMPLETE",
        },
        "cells": records,
        "normal_timing_excludes_allocation_audit": True,
        "science_requires_allocation_audit": True,
        "complete_sentinel": "FEPPML_RECIPES_COMPLETE",
    }
    validate_compiled_recipes(document, manifest_path, output)
    _write_new(output / "recipes.json", json.dumps(document, sort_keys=True, indent=2) + "\n")
    return document


def validate_compiled_recipes(
    document: Mapping[str, Any], manifest_path: Path, generated_root: Path
) -> None:
    """Validate exact expanded-cell semantics independently of output hashes."""
    manifest = performance.load_manifest(manifest_path)
    expanded = performance.expand_cells(manifest)
    if set(document) != {"schema_version", "recipe_type", "development_manifest_sha256", "protocol", "cells", "normal_timing_excludes_allocation_audit", "science_requires_allocation_audit", "complete_sentinel"}:
        raise RecipeError("compiled recipe document fields are not closed")
    records = document.get("cells")
    if not isinstance(records, list) or len(records) != len(expanded):
        raise RecipeError("compiled recipe cell count differs from expanded manifest")
    by_id = {record.get("cell_id"): record for record in records if isinstance(record, Mapping)}
    if list(by_id) != [cell["cell_id"] for cell in expanded] or len(by_id) != len(records):
        raise RecipeError("compiled recipe cell inventory/order is not exact")
    for cell in expanded:
        record = by_id[cell["cell_id"]]
        if set(record) != {"cell_id", "dataset_id", "seed", "n", "generator", "candidate_science_do", "comparator_science_do", "candidate_timing_do", "comparator_timing_do", "expected_status_map", "expected_output", "probe_rows", "expected_cluster_dimensions", "expected_saved_channels", "candidate_source_provenance", "comparator_source_provenance", "commands"}:
            raise RecipeError("compiled recipe record fields are not closed")
        expected = commands(cell)
        if record.get("commands") != expected or record.get("n") != cell["scale_spec"]["n"]:
            raise RecipeError("compiled command/model semantics differ from expanded cell")
        if expected["candidate_science"] != expected["candidate_timing"] + " allocationaudit":
            raise RecipeError("candidate science/timing delta is not exactly allocationaudit")
        if expected["candidate_allocation_audit"] != expected["candidate_science"]:
            raise RecipeError("candidate allocation-audit command differs from science")
        if expected["comparator_science"] != expected["comparator_timing"]:
            raise RecipeError("comparator science/timing commands differ")
        for arm in ("candidate", "comparator"):
            science = (generated_root / record[f"{arm}_science_do"]).read_text(encoding="ascii")
            timing = (generated_root / record[f"{arm}_timing_do"]).read_text(encoding="ascii")
            statuses = expected_status_map(cell)
            if science != science_do(cell, expected[f"{arm}_science"], candidate=arm == "candidate", statuses=statuses, probe_rows=int(manifest["generator_contract"]["probe_rows"])):
                raise RecipeError("generated science source differs byte-for-byte")
            if timing != timing_do(expected[f"{arm}_timing"], candidate=arm == "candidate", output=cell["output"]):
                raise RecipeError("generated timing source differs byte-for-byte")
        dataset_key = cell.get("paired_fit_case") or cell.get("equivalence_group") or cell["id"]
        dataset_id = f"{dataset_key}_{cell['scale']}"
        seed = int(manifest["generator_contract"]["seed_namespace"]) + int.from_bytes(hashlib.sha256(dataset_id.encode("ascii")).digest()[:4], "big") % 100_000_000
        if (generated_root / record["generator"]).read_text(encoding="ascii") != generator_do(cell, seed):
            raise RecipeError("generated dataset source differs byte-for-byte")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--development-manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.output.exists():
        raise SystemExit("recipe output already exists")
    args.output.mkdir(parents=True)
    compile_recipes(args.development_manifest.resolve(), args.output.resolve())
    print("FEPPML_RECIPES_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
