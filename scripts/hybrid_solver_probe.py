#!/usr/bin/env python3
"""Run a paired, development-only LSMR-CG prototype probe.

The baseline and prototype plugins run in separate Stata processes against the
same generated datasets.  The probe is deliberately ineligible for release
qualification; it records scientific parity, solver routing, warm runtime, and
whole-process peak RSS for the next optimization decision.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import sys
from typing import Any, Iterable, Mapping, Sequence

try:
    from scripts import solver_development as development
except ModuleNotFoundError:  # Direct execution from scripts/.
    import solver_development as development  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/ols_lsmr_cg_prototype_v1.json"
DEFAULT_STATA = Path("/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp")
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]*$")


class ProbeError(RuntimeError):
    """The paired prototype probe failed a closed contract."""


def _expect_keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise ProbeError(
            f"{where} fields mismatch: "
            f"missing={sorted(expected - set(value))}, extra={sorted(set(value) - expected)}"
        )


def load_probe_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ProbeError(f"unable to load hybrid probe manifest: {error}") from error
    if not isinstance(document, dict):
        raise ProbeError("hybrid probe manifest must be an object")
    _expect_keys(
        document,
        {
            "schema_version",
            "manifest_id",
            "status",
            "qualification_eligible",
            "parent_manifest",
            "cases",
            "repetitions",
            "command",
            "selection_rules",
        },
        "hybrid probe manifest",
    )
    if (
        document["schema_version"] != 1
        or document["manifest_id"] != "ols_lsmr_cg_prototype_v1"
        or document["status"] != "development_only"
        or document["qualification_eligible"] is not False
    ):
        raise ProbeError("hybrid probe identity or development-only status changed")

    parent_path = ROOT / str(document["parent_manifest"])
    parent = development.load_development_manifest(parent_path)
    scales = {item["id"]: item for item in parent["scales"]}
    designs = {item["id"]: item for item in parent["designs"]}
    seen: set[str] = set()
    for case in document["cases"]:
        _expect_keys(case, {"id", "scale", "design", "threads"}, "probe case")
        identifier = case["id"]
        if (
            not isinstance(identifier, str)
            or not SAFE_ID.fullmatch(identifier)
            or identifier in seen
        ):
            raise ProbeError("probe case ids must be unique safe identifiers")
        seen.add(identifier)
        if case["scale"] not in scales or case["design"] not in designs:
            raise ProbeError(f"probe case {identifier} references an unknown parent case")
        if not case["threads"] or any(
            not isinstance(value, int) or value <= 0 for value in case["threads"]
        ):
            raise ProbeError(f"probe case {identifier} has invalid thread counts")

    _expect_keys(document["repetitions"], {"cold", "warm"}, "probe repetitions")
    if document["repetitions"] != {"cold": 1, "warm": 3}:
        raise ProbeError("prototype probe repetition cadence changed")
    _expect_keys(
        document["command"],
        {"tolerance", "preflight_tolerance", "iterate"},
        "probe command",
    )
    command = document["command"]
    if (
        float(command["tolerance"]) <= 0
        or float(command["preflight_tolerance"]) <= 0
        or not isinstance(command["iterate"], int)
        or command["iterate"] <= 4
    ):
        raise ProbeError("prototype probe solver command is invalid")
    expected_rules = {
        "coefficient_rtol",
        "coefficient_atol",
        "vce_rtol",
        "vce_atol",
        "residual_rtol",
        "residual_atol",
        "route_min_pilot_contraction",
        "route_min_fe_coefficients",
        "required_requested_solver_code",
        "required_used_solver_code",
        "required_used_preconditioner_code",
        "maximum_warm_runtime_ratio",
        "maximum_peak_rss_ratio",
    }
    _expect_keys(document["selection_rules"], expected_rules, "probe selection rules")
    return document


def expand_cases(probe: Mapping[str, Any]) -> list[dict[str, Any]]:
    parent = development.load_development_manifest(ROOT / str(probe["parent_manifest"]))
    scales = {item["id"]: item for item in parent["scales"]}
    designs = {item["id"]: item for item in parent["designs"]}
    expanded: list[dict[str, Any]] = []
    for case in probe["cases"]:
        for threads in case["threads"]:
            expanded.append(
                {
                    "id": f"{case['id']}_t{threads}",
                    "case_id": case["id"],
                    "scale": dict(scales[case["scale"]]),
                    "design": dict(designs[case["design"]]),
                    "threads": threads,
                }
            )
    return expanded


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _git_head(path: Path) -> str | None:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=path,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return completed.stdout.strip() if completed.returncode == 0 else None


def _source_tree_sha256(path: Path) -> str | None:
    roots = [path / "crates", path / "legacy", path / "stata"]
    fixed = [path / "Cargo.toml", path / "Cargo.lock"]
    if any(not item.exists() for item in [*roots, *fixed]):
        return None
    files = list(fixed)
    for root in roots:
        files.extend(item for item in root.rglob("*") if item.is_file())
    digest = hashlib.sha256()
    for item in sorted(files):
        relative = item.relative_to(path).as_posix().encode()
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(item.read_bytes())
    return digest.hexdigest()


def _stage_plugin(plugin: Path, target: Path) -> dict[str, Any]:
    plugin = plugin.resolve()
    if not plugin.is_file():
        raise ProbeError(f"plugin is missing: {plugin}")
    target.mkdir(parents=True)
    staged = target / "fereg_macos.plugin"
    shutil.copy2(plugin, staged)
    if _sha256(plugin) != _sha256(staged):
        raise ProbeError("staged plugin hash differs from its source")
    source_root = plugin.parent.parent
    return {
        "source_path": str(plugin),
        "staged_path": str(staged),
        "sha256": _sha256(staged),
        "source_git_head": _git_head(source_root),
        "source_tree_sha256": _source_tree_sha256(source_root),
    }


def _preamble(plugin_directory: Path, threads: int) -> list[str]:
    return [
        "version 18.0",
        "set more off",
        "set linesize 255",
        f"set processors {threads}",
        f'adopath ++ "{(ROOT / "stata").as_posix()}"',
        f'adopath ++ "{plugin_directory.as_posix()}"',
    ]


def _write_do(path: Path, lines: Iterable[str]) -> None:
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _run_stata(stata: Path, do_file: Path, work: Path) -> tuple[str, int]:
    time_binary = Path("/usr/bin/time")
    command = [str(stata), "-q", "-b", "do", str(do_file)]
    if time_binary.is_file():
        command = [str(time_binary), "-l", *command]
    completed = subprocess.run(
        command,
        cwd=work,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    output = completed.stdout
    log = work / f"{do_file.stem}.log"
    if log.is_file():
        output += "\n" + log.read_text(encoding="utf-8", errors="replace")
    if completed.returncode != 0:
        raise ProbeError(
            f"Stata failed for {do_file.name} with rc={completed.returncode}\n{output[-5000:]}"
        )
    peak = None
    for pattern, multiplier in (
        (r"(?m)^\s*(\d+)\s+maximum resident set size\s*$", 1),
        (r"Maximum resident set size \(kbytes\):\s*(\d+)", 1024),
    ):
        match = re.search(pattern, output)
        if match:
            peak = int(match.group(1)) * multiplier
            break
    if peak is None or peak <= 0:
        raise ProbeError(f"peak RSS was not reported for {do_file.name}")
    return output, peak


def _records(output: str, prefix: str) -> list[dict[str, str]]:
    return development._records(output, prefix)


def _number(record: Mapping[str, str], field: str) -> float:
    return development._number(record, field)


def _fereg_command(absorb: str, threads: int, tolerance: float, iterate: int) -> str:
    return (
        f"fereg y x1 x2, absorb({absorb}) solver(auto) preconditioner(auto) "
        f"threads({threads}) tolerance({tolerance:.17g}) iterate({iterate})"
    )


def _reghdfe_command(absorb: str, tolerance: float, iterate: int) -> str:
    return (
        f"reghdfe y x1 x2, absorb({absorb}) transform(symmetric_kaczmarz) "
        f"acceleration(conjugate_gradient) tolerance({tolerance:.17g}) iterate({iterate})"
    )


def _preflight_do(
    path: Path,
    plugin_directory: Path,
    dataset: Path,
    run_id: str,
    absorb: str,
    threads: int,
    tolerance: float,
    iterate: int,
) -> None:
    oracle = _reghdfe_command(absorb, tolerance, iterate)
    candidate = _fereg_command(absorb, threads, tolerance, iterate)
    lines = _preamble(plugin_directory, threads) + [
        "capture findfile fereg_macos.plugin",
        "if _rc exit _rc",
        "display as result \"PLUGIN_PATH|path=`r(fn)'\"",
        f'use "{dataset.as_posix()}", clear',
        "capture drop __hyb_ref_resid __hyb_can_resid __hyb_ref_sample __hyb_resdiff __hyb_resratio",
        f"capture noisily {oracle} residuals(__hyb_ref_resid)",
        "local __hyb_rc=_rc",
        "if `__hyb_rc' exit `__hyb_rc'",
        "matrix __hyb_ref_b=e(b)",
        "matrix __hyb_ref_v=e(V)",
        "scalar __hyb_ref_n=e(N)",
        "scalar __hyb_ref_rss=e(rss)",
        "scalar __hyb_ref_r2=e(r2)",
        "capture scalar __hyb_ref_dfa=e(df_a)",
        "if _rc scalar __hyb_ref_dfa=.",
        "capture scalar __hyb_ref_sing=e(num_singletons)",
        "if _rc scalar __hyb_ref_sing=.",
        "generate byte __hyb_ref_sample=e(sample)",
        f"capture noisily {candidate} residuals(__hyb_can_resid)",
        "local __hyb_rc=_rc",
        "if `__hyb_rc' exit `__hyb_rc'",
        "matrix __hyb_can_b=e(b)",
        "matrix __hyb_can_v=e(V)",
        "matrix __hyb_solver=e(fereg_solver)",
        "scalar __hyb_can_n=e(N)",
        "scalar __hyb_can_rss=e(rss)",
        "scalar __hyb_can_r2=e(r2)",
        "capture scalar __hyb_can_dfa=e(df_a)",
        "if _rc scalar __hyb_can_dfa=.",
        "capture scalar __hyb_can_sing=e(num_singletons)",
        "if _rc scalar __hyb_can_sing=.",
        "count if __hyb_ref_sample != e(sample)",
        "scalar __hyb_samplediff=r(N)",
        'mata: st_numscalar("__hyb_bmax", max(abs(st_matrix("__hyb_ref_b") :- st_matrix("__hyb_can_b"))))',
        'mata: st_numscalar("__hyb_bscale", max(abs(st_matrix("__hyb_ref_b"))))',
        'mata: st_numscalar("__hyb_vmax", max(abs(st_matrix("__hyb_ref_v") :- st_matrix("__hyb_can_v"))))',
        'mata: st_numscalar("__hyb_vscale", max(abs(st_matrix("__hyb_ref_v"))))',
        "scalar __hyb_bscale=max(__hyb_bscale,1)",
        "scalar __hyb_vscale=max(__hyb_vscale,1)",
        "generate double __hyb_resdiff=abs(__hyb_ref_resid-__hyb_can_resid) if __hyb_ref_sample",
        "quietly summarize __hyb_resdiff, meanonly",
        "scalar __hyb_resmax=r(max)",
        "quietly summarize __hyb_ref_resid if __hyb_ref_sample, meanonly",
        "scalar __hyb_resscale=max(abs(r(min)),abs(r(max)),1)",
        "quietly summarize __hyb_can_resid if __hyb_ref_sample, meanonly",
        "scalar __hyb_resscale=max(__hyb_resscale,abs(r(min)),abs(r(max)))",
        "generate double __hyb_resratio=__hyb_resdiff/(1e-9+1e-7*max(abs(__hyb_ref_resid),abs(__hyb_can_resid))) if __hyb_ref_sample",
        "quietly summarize __hyb_resratio, meanonly",
        "scalar __hyb_resnorm=r(max)",
        "scalar __hyb_rssrel=abs(__hyb_ref_rss-__hyb_can_rss)/max(abs(__hyb_ref_rss),1)",
        f'display as result "PREFLIGHT_A|id={run_id}|bmax=" %21.16g __hyb_bmax "|bscale=" %21.16g __hyb_bscale "|vmax=" %21.16g __hyb_vmax "|vscale=" %21.16g __hyb_vscale',
        f'display as result "PREFLIGHT_B|id={run_id}|nref=" %21.16g __hyb_ref_n "|ncan=" %21.16g __hyb_can_n "|samplediff=" %21.16g __hyb_samplediff "|rssrel=" %21.16g __hyb_rssrel "|r2diff=" %21.16g abs(__hyb_ref_r2-__hyb_can_r2)',
        f'display as result "PREFLIGHT_C|id={run_id}|dfaref=" %21.16g __hyb_ref_dfa "|dfacan=" %21.16g __hyb_can_dfa "|singref=" %21.16g __hyb_ref_sing "|singcan=" %21.16g __hyb_can_sing',
        f'display as result "PREFLIGHT_D|id={run_id}|b1=" %21.16g __hyb_can_b[1,1] "|b2=" %21.16g __hyb_can_b[1,2] "|v11=" %21.16g __hyb_can_v[1,1] "|v12=" %21.16g __hyb_can_v[1,2] "|v22=" %21.16g __hyb_can_v[2,2]',
        f'display as result "PREFLIGHT_E|id={run_id}|resmax=" %21.16g __hyb_resmax "|resscale=" %21.16g __hyb_resscale "|resnorm=" %21.16g __hyb_resnorm',
        f'display as result "PREFLIGHT_F|id={run_id}|rss=" %21.16g __hyb_can_rss "|r2=" %21.16g __hyb_can_r2',
        f'display as result "PREFLIGHT_DIAG_A|id={run_id}|requested=" %21.16g __hyb_solver[1,1] "|used=" %21.16g __hyb_solver[1,2] "|iters=" %21.16g __hyb_solver[1,3] "|stop=" %21.16g __hyb_solver[1,4] "|relinf=" %21.16g __hyb_solver[1,6] "|warnings=" %21.16g __hyb_solver[1,12]',
        f'display as result "PREFLIGHT_DIAG_B|id={run_id}|requested_precond=" %21.16g __hyb_solver[1,13] "|used_precond=" %21.16g __hyb_solver[1,14] "|pilot_iters=" %21.16g __hyb_solver[1,15] "|pilot_contract=" %21.16g __hyb_solver[1,16]',
        'display as result "END_HYBRID_PREFLIGHT|rc=0"',
        "exit 0",
    ]
    _write_do(path, lines)


def _single_record(output: str, prefix: str) -> dict[str, str]:
    records = _records(output, prefix)
    if len(records) != 1:
        raise ProbeError(f"expected one {prefix} record, observed {len(records)}")
    return records[0]


def _parse_preflight(
    output: str, run_id: str, rules: Mapping[str, Any], staged_plugin: Path
) -> dict[str, Any]:
    plugin_record = _single_record(output, "PLUGIN_PATH")
    if Path(plugin_record["path"]).resolve() != staged_plugin.resolve():
        raise ProbeError("Stata resolved a plugin outside the isolated variant directory")
    a = _single_record(output, "PREFLIGHT_A")
    b = _single_record(output, "PREFLIGHT_B")
    c = _single_record(output, "PREFLIGHT_C")
    d = _single_record(output, "PREFLIGHT_D")
    e = _single_record(output, "PREFLIGHT_E")
    f = _single_record(output, "PREFLIGHT_F")
    diag_a = _single_record(output, "PREFLIGHT_DIAG_A")
    diag_b = _single_record(output, "PREFLIGHT_DIAG_B")
    if any(
        record.get("id") != run_id
        for record in (a, b, c, d, e, f, diag_a, diag_b)
    ):
        raise ProbeError("preflight record identity mismatch")
    failures: list[str] = []
    coefficient_limit = float(rules["coefficient_atol"]) + float(
        rules["coefficient_rtol"]
    ) * _number(a, "bscale")
    vce_limit = float(rules["vce_atol"]) + float(rules["vce_rtol"]) * _number(
        a, "vscale"
    )
    residual_limit = float(rules["residual_atol"]) + float(
        rules["residual_rtol"]
    ) * _number(e, "resscale")
    if _number(a, "bmax") > coefficient_limit:
        failures.append("coefficient parity")
    if _number(a, "vmax") > vce_limit:
        failures.append("VCE parity")
    if _number(b, "nref") != _number(b, "ncan") or _number(b, "samplediff") != 0:
        failures.append("sample parity")
    if _number(b, "rssrel") > 1e-7 or _number(b, "r2diff") > 1e-10:
        failures.append("fit parity")
    if _number(c, "dfaref") != _number(c, "dfacan"):
        failures.append("absorbed DoF parity")
    if math.isfinite(_number(c, "singref")) and _number(c, "singref") != _number(
        c, "singcan"
    ):
        failures.append("singleton parity")
    if _number(e, "resmax") > residual_limit:
        failures.append("residual parity")
    if failures:
        raise ProbeError(f"scientific preflight failed for {run_id}: {', '.join(failures)}")
    return {
        "coefficient_max_abs_diff": _number(a, "bmax"),
        "vce_max_abs_diff": _number(a, "vmax"),
        "sample_mismatches": int(_number(b, "samplediff")),
        "rss_relative_diff": _number(b, "rssrel"),
        "r2_abs_diff": _number(b, "r2diff"),
        "residual_max_abs_diff": _number(e, "resmax"),
        "residual_max_normalized_diff": _number(e, "resnorm"),
        "coefficients": [_number(d, "b1"), _number(d, "b2")],
        "vce": [_number(d, "v11"), _number(d, "v12"), _number(d, "v22")],
        "rss": _number(f, "rss"),
        "r2": _number(f, "r2"),
        "solver": {
            key: _number({**diag_a, **diag_b}, key)
            for key in (
                "requested",
                "used",
                "iters",
                "stop",
                "relinf",
                "warnings",
                "requested_precond",
                "used_precond",
                "pilot_iters",
                "pilot_contract",
            )
        },
    }


def _timing_do(
    path: Path,
    plugin_directory: Path,
    dataset: Path,
    run_id: str,
    absorb: str,
    threads: int,
    tolerance: float,
    iterate: int,
    repetitions: int,
) -> None:
    command = _fereg_command(absorb, threads, tolerance, iterate)
    lines = _preamble(plugin_directory, threads) + [
        "capture findfile fereg_macos.plugin",
        "if _rc exit _rc",
        "display as result \"PLUGIN_PATH|path=`r(fn)'\"",
        f'use "{dataset.as_posix()}", clear',
        "timer clear 71",
        f"forvalues __hyb_rep=1/{repetitions} {{",
        '    local __hyb_kind=cond(`__hyb_rep\'==1,"cold","warm")',
        "    timer on 71",
        f"    capture quietly {command}",
        "    local __hyb_rc=_rc",
        "    timer off 71",
        "    quietly timer list 71",
        "    scalar __hyb_seconds=r(t71)",
        "    timer clear 71",
        "    if `__hyb_rc'==0 {",
        "        matrix __hyb_b=e(b)",
        "        matrix __hyb_v=e(V)",
        "        matrix __hyb_solver=e(fereg_solver)",
        "        matrix __hyb_runtime=e(fereg_runtime)",
        f'        display as result "TIME_A|id={run_id}|rep=`__hyb_rep\'|kind=`__hyb_kind\'|rc=`__hyb_rc\'|seconds=" %21.16g __hyb_seconds',
        f'        display as result "TIME_S|id={run_id}|rep=`__hyb_rep\'|b1=" %21.16g __hyb_b[1,1] "|b2=" %21.16g __hyb_b[1,2] "|v11=" %21.16g __hyb_v[1,1] "|v12=" %21.16g __hyb_v[1,2] "|v22=" %21.16g __hyb_v[2,2]',
        f'        display as result "TIME_B|id={run_id}|rep=`__hyb_rep\'|requested=" %21.16g __hyb_solver[1,1] "|used=" %21.16g __hyb_solver[1,2] "|iters=" %21.16g __hyb_solver[1,3] "|stop=" %21.16g __hyb_solver[1,4]',
        f'        display as result "TIME_F|id={run_id}|rep=`__hyb_rep\'|absinf=" %21.16g __hyb_solver[1,5] "|relinf=" %21.16g __hyb_solver[1,6] "|rel2=" %21.16g __hyb_solver[1,7] "|neq=" %21.16g __hyb_solver[1,8]',
        f'        display as result "TIME_C|id={run_id}|rep=`__hyb_rep\'|setup=" %21.16g __hyb_solver[1,9] "|solve=" %21.16g __hyb_solver[1,10] "|fallback=" %21.16g __hyb_solver[1,11] "|warnings=" %21.16g __hyb_solver[1,12]',
        f'        display as result "TIME_D|id={run_id}|rep=`__hyb_rep\'|requested_precond=" %21.16g __hyb_solver[1,13] "|used_precond=" %21.16g __hyb_solver[1,14] "|pilot_iters=" %21.16g __hyb_solver[1,15]',
        f'        display as result "TIME_E|id={run_id}|rep=`__hyb_rep\'|pilot_contract=" %21.16g __hyb_solver[1,16] "|native=" %21.16g __hyb_runtime[20,2] "|precommit=" %21.16g __hyb_runtime[21,2]',
        "    }",
        f'    if `__hyb_rc\'!=0 display as result "TIME_A|id={run_id}|rep=`__hyb_rep\'|kind=`__hyb_kind\'|rc=`__hyb_rc\'|seconds=" %21.16g __hyb_seconds',
        "    if `__hyb_rc' exit `__hyb_rc'",
        "}",
        'display as result "END_HYBRID_TIMING|rc=0"',
        "exit 0",
    ]
    _write_do(path, lines)


def _parse_timing(
    output: str, run_id: str, repetitions: int, staged_plugin: Path
) -> dict[str, Any]:
    plugin_record = _single_record(output, "PLUGIN_PATH")
    if Path(plugin_record["path"]).resolve() != staged_plugin.resolve():
        raise ProbeError("timing run resolved a plugin outside its variant directory")
    a_records = _records(output, "TIME_A")
    s_records = _records(output, "TIME_S")
    b_records = _records(output, "TIME_B")
    f_records = _records(output, "TIME_F")
    c_records = _records(output, "TIME_C")
    d_records = _records(output, "TIME_D")
    e_records = _records(output, "TIME_E")
    if any(
        len(records) != repetitions
        for records in (
            a_records,
            s_records,
            b_records,
            f_records,
            c_records,
            d_records,
            e_records,
        )
    ):
        raise ProbeError(f"timing record count mismatch for {run_id}")
    s_by_rep = {int(_number(record, "rep")): record for record in s_records}
    b_by_rep = {int(_number(record, "rep")): record for record in b_records}
    f_by_rep = {int(_number(record, "rep")): record for record in f_records}
    c_by_rep = {int(_number(record, "rep")): record for record in c_records}
    d_by_rep = {int(_number(record, "rep")): record for record in d_records}
    e_by_rep = {int(_number(record, "rep")): record for record in e_records}
    parsed: list[dict[str, Any]] = []
    for a in a_records:
        rep = int(_number(a, "rep"))
        s = s_by_rep.get(rep)
        b = b_by_rep.get(rep)
        f = f_by_rep.get(rep)
        c = c_by_rep.get(rep)
        d = d_by_rep.get(rep)
        e = e_by_rep.get(rep)
        if (
            s is None
            or b is None
            or f is None
            or c is None
            or d is None
            or e is None
            or any(record.get("id") != run_id for record in (a, s, b, f, c, d, e))
            or int(_number(a, "rc")) != 0
        ):
            raise ProbeError(f"timing repetition failed for {run_id}/{rep}")
        seconds = _number(a, "seconds")
        if not math.isfinite(seconds) or seconds <= 0:
            raise ProbeError("timing must be finite and positive")
        parsed.append(
            {
                "repetition": rep,
                "kind": a["kind"],
                "seconds": seconds,
                "coefficients": [_number(s, "b1"), _number(s, "b2")],
                "vce": [_number(s, "v11"), _number(s, "v12"), _number(s, "v22")],
                "solver": {
                    key: _number({**b, **f, **c, **d, **e}, key)
                    for key in (
                        "requested",
                        "used",
                        "iters",
                        "stop",
                        "absinf",
                        "relinf",
                        "rel2",
                        "neq",
                        "setup",
                        "solve",
                        "fallback",
                        "warnings",
                        "requested_precond",
                        "used_precond",
                        "pilot_iters",
                        "pilot_contract",
                        "native",
                        "precommit",
                    )
                },
            }
        )
    parsed.sort(key=lambda item: item["repetition"])
    warm = [item["seconds"] for item in parsed if item["kind"] == "warm"]
    if len(warm) != repetitions - 1:
        raise ProbeError("warm timing count mismatch")
    median = statistics.median(warm)
    return {
        "repetitions": parsed,
        "cold_seconds": parsed[0]["seconds"],
        "warm_median_seconds": median,
        "warm_mad_seconds": statistics.median(abs(value - median) for value in warm),
        "warm_min_seconds": min(warm),
        "warm_max_seconds": max(warm),
    }


def _paired_comparison(
    run: Mapping[str, Any],
    baseline: Mapping[str, Any],
    prototype: Mapping[str, Any],
    rules: Mapping[str, Any],
) -> dict[str, Any]:
    baseline_solver = baseline["timing"]["repetitions"][-1]["solver"]
    prototype_solver = prototype["timing"]["repetitions"][-1]["solver"]
    total_coefficients = int(run["scale"]["workers"]) + int(run["scale"]["firms"])
    route_applicable = (
        total_coefficients >= int(rules["route_min_fe_coefficients"])
        and prototype_solver["pilot_contract"]
        >= float(rules["route_min_pilot_contraction"])
    )
    route_observed = (
        prototype_solver["requested"] == float(rules["required_requested_solver_code"])
        and prototype_solver["used"] == float(rules["required_used_solver_code"])
        and prototype_solver["used_precond"]
        == float(rules["required_used_preconditioner_code"])
        and prototype_solver["warnings"] >= 1
    )
    runtime_ratio = (
        float(prototype["timing"]["warm_median_seconds"])
        / float(baseline["timing"]["warm_median_seconds"])
    )
    rss_ratio = float(prototype["peak_rss_bytes"]) / float(baseline["peak_rss_bytes"])
    coefficient_diff = max(
        abs(left - right)
        for left, right in zip(
            baseline["preflight"]["coefficients"],
            prototype["preflight"]["coefficients"],
        )
    )
    vce_diff = max(
        abs(left - right)
        for left, right in zip(
            baseline["preflight"]["vce"], prototype["preflight"]["vce"]
        )
    )
    return {
        "run_id": run["id"],
        "case_id": run["case_id"],
        "threads": run["threads"],
        "total_declared_fe_coefficients": total_coefficients,
        "route_applicable": route_applicable,
        "route_observed": route_observed,
        "baseline_solver": baseline_solver,
        "prototype_solver": prototype_solver,
        "prototype_over_baseline_warm_runtime": runtime_ratio,
        "prototype_over_baseline_peak_rss": rss_ratio,
        "coefficient_max_abs_diff": coefficient_diff,
        "vce_max_abs_diff": vce_diff,
        "runtime_gate_passed": runtime_ratio
        <= float(rules["maximum_warm_runtime_ratio"]),
        "memory_gate_passed": rss_ratio <= float(rules["maximum_peak_rss_ratio"]),
    }


def run_probe(
    probe: Mapping[str, Any],
    stata: Path,
    baseline_plugin: Path,
    prototype_plugin: Path,
    work: Path,
    *,
    case_filters: Sequence[str] | None = None,
) -> dict[str, Any]:
    if work.exists():
        raise ProbeError(f"work directory already exists: {work}")
    if not stata.is_file():
        raise ProbeError(f"Stata executable is missing: {stata}")
    work.mkdir(parents=True)
    variants = {
        "baseline": _stage_plugin(baseline_plugin, work / "plugins/baseline"),
        "prototype": _stage_plugin(prototype_plugin, work / "plugins/prototype"),
    }
    if variants["baseline"]["sha256"] == variants["prototype"]["sha256"]:
        raise ProbeError("baseline and prototype plugin hashes must differ")

    runs = expand_cases(probe)
    known_cases = {run["case_id"] for run in runs}
    unknown = set(case_filters or ()) - known_cases
    if unknown:
        raise ProbeError(f"unknown case filters: {sorted(unknown)}")
    if case_filters:
        runs = [run for run in runs if run["case_id"] in case_filters]
    if not runs:
        raise ProbeError("case filters selected no probe runs")

    parent = development.load_development_manifest(ROOT / str(probe["parent_manifest"]))
    dependency_identity = development._dependency_probe(stata, work, parent)
    generator = ROOT / str(parent["generator"])
    datasets: dict[tuple[str, str], dict[str, Any]] = {}
    for run in runs:
        key = (run["design"]["id"], run["scale"]["id"])
        if key not in datasets:
            datasets[key] = development._generate_dataset(
                stata, work, generator, run["scale"], run["design"]
            )

    command = probe["command"]
    rules = probe["selection_rules"]
    results: dict[str, dict[str, Any]] = {}
    for run in runs:
        dataset_record = datasets[(run["design"]["id"], run["scale"]["id"])]
        dataset = Path(dataset_record["path"])
        for variant, identity in variants.items():
            staged = Path(identity["staged_path"])
            variant_directory = staged.parent
            run_key = f"{run['id']}__{variant}"
            preflight_do = work / f"preflight_{run_key}.do"
            _preflight_do(
                preflight_do,
                variant_directory,
                dataset,
                run_key,
                run["design"]["absorb"],
                int(run["threads"]),
                float(command["preflight_tolerance"]),
                int(command["iterate"]),
            )
            preflight_output, _ = _run_stata(stata, preflight_do, work)
            preflight = _parse_preflight(preflight_output, run_key, rules, staged)

            timing_do = work / f"timing_{run_key}.do"
            repetitions = int(probe["repetitions"]["cold"]) + int(
                probe["repetitions"]["warm"]
            )
            _timing_do(
                timing_do,
                variant_directory,
                dataset,
                run_key,
                run["design"]["absorb"],
                int(run["threads"]),
                float(command["tolerance"]),
                int(command["iterate"]),
                repetitions,
            )
            timing_output, peak_rss = _run_stata(stata, timing_do, work)
            timing = _parse_timing(timing_output, run_key, repetitions, staged)
            results[run_key] = {
                "id": run_key,
                "variant": variant,
                "case_id": run["case_id"],
                "threads": run["threads"],
                "scale": run["scale"],
                "design": run["design"],
                "dataset_sha256": dataset_record["sha256"],
                "plugin_sha256": identity["sha256"],
                "preflight": preflight,
                "timing": timing,
                "peak_rss_bytes": peak_rss,
            }

    comparisons = [
        _paired_comparison(
            run,
            results[f"{run['id']}__baseline"],
            results[f"{run['id']}__prototype"],
            rules,
        )
        for run in runs
    ]
    applicable = [item for item in comparisons if item["route_applicable"]]
    reasons: list[str] = []
    if not applicable:
        reasons.append("no selected case crossed both routing thresholds")
    if any(not item["route_observed"] for item in applicable):
        reasons.append("the prototype route was not observed on every applicable case")
    if any(not item["runtime_gate_passed"] for item in applicable):
        reasons.append("the prototype missed the paired warm-runtime gate")
    if any(not item["memory_gate_passed"] for item in applicable):
        reasons.append("the prototype missed the paired peak-RSS gate")
    verdict = "promising" if applicable and not reasons else "not_yet_promising"
    return {
        "schema_version": 1,
        "kind": "fereg_lsmr_cg_prototype_probe",
        "qualification_eligible": False,
        "manifest_id": probe["manifest_id"],
        "stata_executable": str(stata),
        "plugins": variants,
        "dependency_identity": dependency_identity,
        "datasets": list(datasets.values()),
        "runs": [results[key] for key in sorted(results)],
        "comparisons": comparisons,
        "verdict": {"status": verdict, "reasons": reasons},
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    subparsers = parser.add_subparsers(dest="action", required=True)
    plan = subparsers.add_parser("plan", help="validate and print the paired case plan")
    plan.add_argument("--output", type=Path)
    run = subparsers.add_parser("run", help="execute the paired prototype probe")
    run.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    run.add_argument("--baseline-plugin", type=Path, required=True)
    run.add_argument("--prototype-plugin", type=Path, required=True)
    run.add_argument("--work-dir", type=Path, required=True)
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--case-id", action="append", dest="case_ids")
    args = parser.parse_args(argv)
    try:
        probe = load_probe_manifest(args.manifest)
        if args.action == "plan":
            document: Any = expand_cases(probe)
        else:
            document = run_probe(
                probe,
                args.stata,
                args.baseline_plugin,
                args.prototype_plugin,
                args.work_dir,
                case_filters=args.case_ids,
            )
        encoded = json.dumps(document, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.write_text(encoded, encoding="utf-8")
        else:
            sys.stdout.write(encoded)
        return 0
    except (ProbeError, development.ContractError, OSError, subprocess.SubprocessError) as error:
        print(f"hybrid_solver_probe: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
