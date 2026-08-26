#!/usr/bin/env python3
"""Qualify the frozen direct Schwarz-LSMR `solver(auto)` candidate.

Calibration and holdout profiles share one fail-closed harness, but the frozen
manifest prevents holdout results from changing the selected algorithm.  The
licensed-Stata residual comparison to reghdfe remains recorded; large routed
cases are published only when the 100x-tighter stability solve, native FE/OLS
certificates, and the observation-space projection audit all pass.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Any, Mapping, Sequence

try:
    from scripts import hybrid_solver_probe as probe
    from scripts import solver_development as development
except ModuleNotFoundError:
    import hybrid_solver_probe as probe  # type: ignore[no-redef]
    import solver_development as development  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/ols_direct_lsmr_qualification_v1.json"
DEFAULT_STATA = Path("/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp")


class QualificationError(RuntimeError):
    """The frozen qualification contract was malformed or failed."""


def _run_stata(
    stata: Path, do_file: Path, work: Path, success_sentinel: str
) -> tuple[str, int]:
    command = [str(stata), "-q", "-b", "do", str(do_file)]
    if Path("/usr/bin/time").is_file():
        command = ["/usr/bin/time", "-l", *command]
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
    if success_sentinel not in output:
        raise QualificationError(
            f"Stata failed for {do_file.name} with rc={completed.returncode}\n{output[-5000:]}"
        )
    peak = 0
    for pattern, multiplier in (
        (r"(?m)^\s*(\d+)\s+maximum resident set size\s*$", 1),
        (r"Maximum resident set size \(kbytes\):\s*(\d+)", 1024),
    ):
        match = re.search(pattern, output)
        if match:
            peak = int(match.group(1)) * multiplier
            break
    return output, peak


def _keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise QualificationError(
            f"{where} fields mismatch: missing={sorted(expected-set(value))}, "
            f"extra={sorted(set(value)-expected)}"
        )


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    document = json.loads(path.read_text(encoding="utf-8"))
    _keys(
        document,
        {
            "schema_version",
            "manifest_id",
            "status",
            "qualification_eligible",
            "parent_manifest",
            "frozen_candidate",
            "profiles",
            "repetitions",
            "command",
            "accuracy",
            "promotion",
            "comparison_arms",
        },
        "qualification manifest",
    )
    if (
        document["schema_version"] != 1
        or document["manifest_id"] != "ols_direct_lsmr_qualification_v1"
        or document["status"] != "candidate_frozen"
        or document["qualification_eligible"] is not True
    ):
        raise QualificationError("direct-LSMR qualification identity is not frozen")
    frozen = document["frozen_candidate"]
    _keys(
        frozen,
        {
            "algorithm",
            "internal_tolerance_rule",
            "preconditioner_rule",
            "pilot_sweeps",
            "pilot_min_contraction",
            "coefficient_cap",
            "cg_refinement_iterations",
            "fallback",
        },
        "frozen candidate",
    )
    if (
        frozen["algorithm"] != "direct_lsmr"
        or frozen["pilot_sweeps"] != 4
        or frozen["pilot_min_contraction"] != 0.008
        or frozen["coefficient_cap"] is not None
        or frozen["cg_refinement_iterations"] != 0
        or frozen["fallback"] != "certified_cg_from_saved_map_pilot"
    ):
        raise QualificationError("frozen direct-LSMR rule changed")
    profiles = {profile["id"]: profile for profile in document["profiles"]}
    if set(profiles) != {"calibration_100k", "calibration_1m", "holdout_1m", "freeze_10m"}:
        raise QualificationError("qualification profile set changed")
    if document["repetitions"] != {"cold": 1, "warm": 5}:
        raise QualificationError("qualification requires one cold and five warm repetitions")
    if "holdout" not in profiles["holdout_1m"]["id"]:
        raise QualificationError("holdout profile identity is missing")
    return document


def expand_profile(manifest: Mapping[str, Any], profile_id: str) -> list[dict[str, Any]]:
    parent = development.load_development_manifest(ROOT / str(manifest["parent_manifest"]))
    scales = {scale["id"]: scale for scale in parent["scales"]}
    designs = {design["id"]: design for design in parent["designs"]}
    profiles = {profile["id"]: profile for profile in manifest["profiles"]}
    if profile_id not in profiles:
        raise QualificationError(f"unknown qualification profile {profile_id!r}")
    profile = profiles[profile_id]
    scale = scales[profile["scale"]]
    runs: list[dict[str, Any]] = []
    for case in profile["cases"]:
        design = dict(designs[case["design"]])
        design["seed"] = case["seed"]
        for threads in profile["threads"]:
            runs.append(
                {
                    "id": f"{case['id']}_{scale['id']}_t{threads}",
                    "profile": profile_id,
                    "case_id": case["id"],
                    "scale": dict(scale),
                    "design": design,
                    "threads": int(threads),
                }
            )
    return runs


def _fereg_command(
    absorb: str,
    threads: int,
    tolerance: float,
    iterate: int,
    solver: str,
    residual: str | None = None,
) -> str:
    preconditioner = " preconditioner(schwarz)" if solver == "lsmr" else ""
    residual_option = f" residuals({residual})" if residual else ""
    return (
        f"fereg y x1 x2, absorb({absorb}) solver({solver}){preconditioner} "
        f"threads({threads}) tolerance({tolerance:.17g}) iterate({iterate}){residual_option}"
    )


def _science_do(
    path: Path,
    plugin_directory: Path,
    dataset: Path,
    run: Mapping[str, Any],
    command: Mapping[str, Any],
    accuracy: Mapping[str, Any],
) -> None:
    identifier = run["id"]
    absorb = run["design"]["absorb"]
    threads = int(run["threads"])
    iterate = int(command["iterate"])
    oracle = probe._reghdfe_command(absorb, float(command["scientific_tolerance"]), iterate)
    candidate = _fereg_command(
        absorb,
        threads,
        float(command["requested_tolerance"]),
        iterate,
        "auto",
        "__ql_can_resid",
    )
    tight = _fereg_command(
        absorb,
        threads,
        float(command["stability_tolerance"]),
        iterate,
        "lsmr",
        "__ql_tight_resid",
    )
    lines = probe._preamble(plugin_directory, threads) + [
        "capture findfile fereg_macos.plugin",
        "if _rc exit _rc",
        "display as result \"PLUGIN_PATH|path=`r(fn)'\"",
        f'use "{dataset.as_posix()}", clear',
        "capture drop __ql_*",
        f"capture noisily {oracle} residuals(__ql_ref_resid)",
        "if _rc exit _rc",
        "matrix __ql_ref_b=e(b)",
        "matrix __ql_ref_v=e(V)",
        "scalar __ql_ref_n=e(N)",
        "scalar __ql_ref_rss=e(rss)",
        "scalar __ql_ref_r2=e(r2)",
        "capture scalar __ql_ref_dfa=e(df_a)",
        "if _rc scalar __ql_ref_dfa=.",
        "capture scalar __ql_ref_sing=e(num_singletons)",
        "if _rc scalar __ql_ref_sing=.",
        "generate byte __ql_ref_sample=e(sample)",
        f"capture noisily {candidate}",
        "if _rc exit _rc",
        "matrix __ql_can_b=e(b)",
        "matrix __ql_can_v=e(V)",
        "matrix __ql_can_solver=e(fereg_solver)",
        "matrix __ql_can_cert=e(fereg_certificate)",
        "matrix __ql_can_runtime=e(fereg_runtime)",
        "scalar __ql_can_n=e(N)",
        "scalar __ql_can_rss=e(rss)",
        "scalar __ql_can_r2=e(r2)",
        "capture scalar __ql_can_dfa=e(df_a)",
        "if _rc scalar __ql_can_dfa=.",
        "capture scalar __ql_can_sing=e(num_singletons)",
        "if _rc scalar __ql_can_sing=.",
        "count if __ql_ref_sample != e(sample)",
        "scalar __ql_samplediff=r(N)",
        'mata: st_numscalar("__ql_bmax", max(abs(st_matrix("__ql_ref_b") :- st_matrix("__ql_can_b"))))',
        'mata: st_numscalar("__ql_bscale", max(abs(st_matrix("__ql_ref_b"))))',
        'mata: st_numscalar("__ql_vmax", max(abs(st_matrix("__ql_ref_v") :- st_matrix("__ql_can_v"))))',
        'mata: st_numscalar("__ql_vscale", max(abs(st_matrix("__ql_ref_v"))))',
        "scalar __ql_bscale=max(__ql_bscale,1)",
        "scalar __ql_vscale=max(__ql_vscale,1)",
        "generate double __ql_ref_diff=abs(__ql_ref_resid-__ql_can_resid) if __ql_ref_sample",
        "quietly summarize __ql_ref_diff, meanonly",
        "scalar __ql_ref_resmax=r(max)",
        "quietly summarize __ql_ref_resid if __ql_ref_sample, meanonly",
        "scalar __ql_ref_resscale=max(abs(r(min)),abs(r(max)),1)",
        "quietly summarize __ql_can_resid if __ql_ref_sample, meanonly",
        "scalar __ql_ref_resscale=max(__ql_ref_resscale,abs(r(min)),abs(r(max)))",
        f"capture noisily {tight}",
        "if _rc exit _rc",
        "matrix __ql_tight_solver=e(fereg_solver)",
        "scalar __ql_tight_rss=e(rss)",
        "generate double __ql_stab_diff=abs(__ql_can_resid-__ql_tight_resid) if __ql_ref_sample",
        f"generate double __ql_stab_ratio=__ql_stab_diff/({float(accuracy['atol']):.17g}+{float(accuracy['rtol']):.17g}*max(abs(__ql_can_resid),abs(__ql_tight_resid))) if __ql_ref_sample",
        "quietly summarize __ql_stab_diff, meanonly",
        "scalar __ql_stab_max=r(max)",
        "quietly summarize __ql_stab_ratio, meanonly",
        "scalar __ql_stab_norm=r(max)",
        "scalar __ql_rssrel=abs(__ql_ref_rss-__ql_can_rss)/max(abs(__ql_ref_rss),1)",
        "scalar __ql_tightrssrel=abs(__ql_tight_rss-__ql_can_rss)/max(abs(__ql_tight_rss),1)",
        "scalar __ql_certmin=min(__ql_can_cert[1,2],__ql_can_cert[2,2],__ql_can_cert[3,2],__ql_can_cert[24,2],__ql_can_cert[32,2])",
        f'display as result "QL_SCI_A|id={identifier}|bmax=" %21.16g __ql_bmax "|bscale=" %21.16g __ql_bscale "|vmax=" %21.16g __ql_vmax "|vscale=" %21.16g __ql_vscale',
        f'display as result "QL_SCI_B|id={identifier}|nref=" %21.16g __ql_ref_n "|ncan=" %21.16g __ql_can_n "|samplediff=" %21.16g __ql_samplediff "|rssrel=" %21.16g __ql_rssrel "|r2diff=" %21.16g abs(__ql_ref_r2-__ql_can_r2)',
        f'display as result "QL_SCI_C|id={identifier}|dfaref=" %21.16g __ql_ref_dfa "|dfacan=" %21.16g __ql_can_dfa "|singref=" %21.16g __ql_ref_sing "|singcan=" %21.16g __ql_can_sing "|refresmax=" %21.16g __ql_ref_resmax "|refresscale=" %21.16g __ql_ref_resscale',
        f'display as result "QL_SCI_D|id={identifier}|stabmax=" %21.16g __ql_stab_max "|stabnorm=" %21.16g __ql_stab_norm "|tightrssrel=" %21.16g __ql_tightrssrel "|certmin=" %21.16g __ql_certmin',
        f'display as result "QL_SCI_E|id={identifier}|requested=" %21.16g __ql_can_solver[1,1] "|used=" %21.16g __ql_can_solver[1,2] "|fallback=" %21.16g __ql_can_solver[1,11] "|usedprecond=" %21.16g __ql_can_solver[1,14] "|pilotiters=" %21.16g __ql_can_solver[1,15] "|pilotcontract=" %21.16g __ql_can_solver[1,16]',
        f'display as result "QL_SCI_F|id={identifier}|tightused=" %21.16g __ql_tight_solver[1,2] "|tightprecond=" %21.16g __ql_tight_solver[1,14] "|tightrelinf=" %21.16g __ql_tight_solver[1,6] "|executionbytes=" %21.16g __ql_can_runtime[25,2] "|nativealloc=" %21.16g __ql_can_runtime[13,2] "|rss=" %21.16g __ql_can_runtime[15,2]',
        f'display as result "QL_SCI_G|id={identifier}|relinf=" %21.16g __ql_can_solver[1,6]',
        'display as result "END_QL_SCIENCE|rc=0"',
        "exit 0",
    ]
    probe._write_do(path, lines)


def _record(output: str, prefix: str, identifier: str) -> dict[str, str]:
    record = probe._single_record(output, prefix)
    if record.get("id") != identifier:
        raise QualificationError(f"{prefix} identity mismatch")
    return record


def _number(record: Mapping[str, str], field: str) -> float:
    return probe._number(record, field)


def _parse_science(
    output: str,
    run: Mapping[str, Any],
    manifest: Mapping[str, Any],
    staged_plugin: Path,
) -> dict[str, Any]:
    plugin = probe._single_record(output, "PLUGIN_PATH")
    if Path(plugin["path"]).resolve() != staged_plugin.resolve():
        raise QualificationError("science run resolved an unstaged plugin")
    identifier = run["id"]
    a, b, c, d, e, f, g = (
        _record(output, prefix, identifier)
        for prefix in (
            "QL_SCI_A",
            "QL_SCI_B",
            "QL_SCI_C",
            "QL_SCI_D",
            "QL_SCI_E",
            "QL_SCI_F",
            "QL_SCI_G",
        )
    )
    accuracy = manifest["accuracy"]
    failures: list[str] = []
    if _number(a, "bmax") > float(accuracy["coefficient_atol"]) + float(accuracy["coefficient_rtol"]) * _number(a, "bscale"):
        failures.append("coefficient parity")
    if _number(a, "vmax") > float(accuracy["vce_atol"]) + float(accuracy["vce_rtol"]) * _number(a, "vscale"):
        failures.append("VCE parity")
    if _number(b, "nref") != _number(b, "ncan") or _number(b, "samplediff") != 0:
        failures.append("sample parity")
    if _number(b, "rssrel") > float(accuracy["rss_rtol"]) or _number(b, "r2diff") > float(accuracy["r2_atol"]):
        failures.append("fit parity")
    if _number(c, "dfaref") != _number(c, "dfacan") or _number(c, "singref") != _number(c, "singcan"):
        failures.append("DoF/singleton parity")
    if _number(d, "certmin") != 1 or _number(g, "relinf") > float(manifest["command"]["requested_tolerance"]):
        failures.append("native certificate")
    routed = _number(e, "pilotcontract") >= float(manifest["frozen_candidate"]["pilot_min_contraction"])
    if routed:
        stability_limit = float(accuracy["atol"]) + float(accuracy["rtol"]) * _number(c, "refresscale")
        if _number(d, "stabmax") > stability_limit or _number(d, "tightrssrel") > float(accuracy["rss_rtol"]):
            failures.append("100x-tighter stability")
        if not (
            _number(e, "requested") == 1
            and _number(e, "used") == 3
            and _number(e, "usedprecond") == 4
            and _number(e, "pilotiters") == 4
            and _number(e, "fallback") == 0
        ):
            failures.append("frozen direct-LSMR route")
    else:
        residual_limit = float(accuracy["atol"]) + float(accuracy["rtol"]) * _number(c, "refresscale")
        if _number(c, "refresmax") > residual_limit:
            failures.append("reghdfe residual parity on non-LSMR route")
    if _number(f, "tightused") != 3 or _number(f, "tightprecond") != 4:
        failures.append("qualification solve route")
    if failures:
        raise QualificationError(f"{identifier} scientific qualification failed: {', '.join(failures)}")
    return {
        "run_id": identifier,
        "routed": routed,
        "coefficient_max_abs_diff": _number(a, "bmax"),
        "vce_max_abs_diff": _number(a, "vmax"),
        "reghdfe_residual_max_abs_diff_reported_only": _number(c, "refresmax"),
        "stability_max_abs_diff": _number(d, "stabmax"),
        "stability_max_pointwise_envelope_ratio_diagnostic": _number(d, "stabnorm"),
        "stability_global_envelope_limit": float(accuracy["atol"]) + float(accuracy["rtol"]) * _number(c, "refresscale"),
        "pilot_contraction": _number(e, "pilotcontract"),
        "used_solver_code": _number(e, "used"),
        "used_preconditioner_code": _number(e, "usedprecond"),
        "execution_modeled_bytes": _number(f, "executionbytes"),
        "native_allocation_bytes": _number(f, "nativealloc"),
        "rss_bytes": _number(f, "rss"),
    }


def _timing_do(
    path: Path,
    plugin_directory: Path,
    dataset: Path,
    run: Mapping[str, Any],
    command: Mapping[str, Any],
    arm: str,
    repetitions: int,
) -> None:
    identifier = f"{run['id']}__{arm}"
    fereg = _fereg_command(
        run["design"]["absorb"],
        int(run["threads"]),
        float(command["requested_tolerance"]),
        int(command["iterate"]),
        "auto",
    )
    lines = probe._preamble(plugin_directory, int(run["threads"])) + [
        "capture findfile fereg_macos.plugin",
        "if _rc exit _rc",
        "display as result \"PLUGIN_PATH|path=`r(fn)'\"",
        f'use "{dataset.as_posix()}", clear',
        "timer clear 71",
        f"forvalues __ql_rep=1/{repetitions} {{",
        '    local __ql_kind=cond(`__ql_rep\'==1,"cold","warm")',
        "    timer on 71",
        f"    capture quietly {fereg}",
        "    local __ql_rc=_rc",
        "    timer off 71",
        "    quietly timer list 71",
        "    scalar __ql_seconds=r(t71)",
        "    timer clear 71",
        "    if `__ql_rc'==0 {",
        "        matrix __hyb_b=e(b)",
        "        matrix __hyb_v=e(V)",
        "        matrix __hyb_solver=e(fereg_solver)",
        "        matrix __hyb_runtime=e(fereg_runtime)",
        f'        display as result "TIME_A|id={identifier}|rep=`__ql_rep\'|kind=`__ql_kind\'|rc=`__ql_rc\'|seconds=" %21.16g __ql_seconds',
        f'        display as result "TIME_S|id={identifier}|rep=`__ql_rep\'|b1=" %21.16g __hyb_b[1,1] "|b2=" %21.16g __hyb_b[1,2] "|v11=" %21.16g __hyb_v[1,1] "|v12=" %21.16g __hyb_v[1,2] "|v22=" %21.16g __hyb_v[2,2]',
        f'        display as result "TIME_B|id={identifier}|rep=`__ql_rep\'|requested=" %21.16g __hyb_solver[1,1] "|used=" %21.16g __hyb_solver[1,2] "|iters=" %21.16g __hyb_solver[1,3] "|stop=" %21.16g __hyb_solver[1,4]',
        f'        display as result "TIME_F|id={identifier}|rep=`__ql_rep\'|absinf=" %21.16g __hyb_solver[1,5] "|relinf=" %21.16g __hyb_solver[1,6] "|rel2=" %21.16g __hyb_solver[1,7] "|neq=" %21.16g __hyb_solver[1,8]',
        f'        display as result "TIME_C|id={identifier}|rep=`__ql_rep\'|setup=" %21.16g __hyb_solver[1,9] "|solve=" %21.16g __hyb_solver[1,10] "|fallback=" %21.16g __hyb_solver[1,11] "|warnings=" %21.16g __hyb_solver[1,12]',
        f'        display as result "TIME_D|id={identifier}|rep=`__ql_rep\'|requested_precond=" %21.16g __hyb_solver[1,13] "|used_precond=" %21.16g __hyb_solver[1,14] "|pilot_iters=" %21.16g __hyb_solver[1,15]',
        f'        display as result "TIME_E|id={identifier}|rep=`__ql_rep\'|pilot_contract=" %21.16g __hyb_solver[1,16] "|native=" %21.16g __hyb_runtime[20,2] "|precommit=" %21.16g __hyb_runtime[21,2]',
        f'        display as result "QL_MEM|id={identifier}|rep=`__ql_rep\'|start=" %21.16g __hyb_runtime[11,2] "|peak=" %21.16g __hyb_runtime[12,2] "|delta=" %21.16g __hyb_runtime[13,2]',
        "    }",
        "    if `__ql_rc' exit `__ql_rc'",
        "}",
        'display as result "END_QL_TIMING|rc=0"',
        "exit 0",
    ]
    probe._write_do(path, lines)


def _parse_timing(output: str, identifier: str, repetitions: int, staged: Path) -> dict[str, Any]:
    parsed = probe._parse_timing(output, identifier, repetitions, staged)
    memory = probe._records(output, "QL_MEM")
    if len(memory) != repetitions:
        raise QualificationError(f"memory timing count mismatch for {identifier}")
    starts = [_number(record, "start") for record in memory]
    deltas = [_number(record, "delta") for record in memory]
    parsed["allocation_start_bytes"] = starts
    parsed["native_allocation_bytes"] = deltas
    parsed["repeated_call_leak_bytes"] = max(starts[1:]) - min(starts[1:])
    parsed["leak_gate_passed"] = parsed["repeated_call_leak_bytes"] == 0
    return parsed


def _solver_workspace_model(run: Mapping[str, Any], routed: bool, rhs: int = 3) -> int:
    n = int(run["scale"]["n"])
    levels = int(run["scale"]["workers"]) + int(run["scale"]["firms"])
    effects = 2
    threads = int(run["threads"])
    matrix = n * rhs * 8
    reductions = max(math.ceil(n / 32768), threads) * rhs * 8
    if not routed:
        return 3 * matrix + max(int(run["scale"]["workers"]), int(run["scale"]["firms"])) * rhs * 16 + reductions + rhs * 8 * 16
    krylov = n * 8 * 8
    common = matrix + krylov + reductions + levels * 16
    diagonal = levels * 16
    topology = n * effects + levels * 48 + n * 24
    return max(common + diagonal, matrix + topology + diagonal) + levels * 8


def run_profile(
    manifest: Mapping[str, Any],
    profile_id: str,
    stata: Path,
    candidate_plugin: Path,
    baseline_plugin: Path,
    work: Path,
) -> dict[str, Any]:
    if work.exists():
        raise QualificationError(f"work directory already exists: {work}")
    if not stata.is_file() or not candidate_plugin.is_file() or not baseline_plugin.is_file():
        raise QualificationError("Stata executable or paired plugin is missing")
    work.mkdir(parents=True)
    identities = {
        "candidate": probe._stage_plugin(candidate_plugin, work / "plugins/candidate"),
        "baseline": probe._stage_plugin(baseline_plugin, work / "plugins/baseline"),
    }
    if identities["candidate"]["sha256"] == identities["baseline"]["sha256"]:
        raise QualificationError("candidate and current-CG baseline plugins must differ")
    staged = Path(identities["candidate"]["staged_path"])
    parent = development.load_development_manifest(ROOT / str(manifest["parent_manifest"]))
    dependency_identity = development._dependency_probe(stata, work, parent)
    generator = ROOT / str(parent["generator"])
    runs = expand_profile(manifest, profile_id)
    datasets: dict[tuple[str, int], dict[str, Any]] = {}
    for run in runs:
        key = (run["case_id"], int(run["scale"]["n"]))
        if key not in datasets:
            datasets[key] = development._generate_dataset(
                stata, work, generator, run["scale"], run["design"]
            )

    scientific: list[dict[str, Any]] = []
    timings: list[dict[str, Any]] = []
    comparisons: list[dict[str, Any]] = []
    repetitions = int(manifest["repetitions"]["cold"]) + int(manifest["repetitions"]["warm"])
    for index, run in enumerate(runs):
        dataset_record = datasets[(run["case_id"], int(run["scale"]["n"]))]
        dataset = Path(dataset_record["path"])
        science_do = work / f"science_{run['id']}.do"
        _science_do(
            science_do,
            staged.parent,
            dataset,
            run,
            manifest["command"],
            manifest["accuracy"],
        )
        output, _ = _run_stata(stata, science_do, work, "END_QL_SCIENCE|rc=0")
        science = _parse_science(output, run, manifest, staged)
        scientific.append(science)

        arm_order = ("candidate", "baseline") if index % 2 == 0 else ("baseline", "candidate")
        arm_results: dict[str, dict[str, Any]] = {}
        for arm in arm_order:
            identifier = f"{run['id']}__{arm}"
            timing_do = work / f"timing_{identifier}.do"
            arm_staged = Path(identities[arm]["staged_path"])
            _timing_do(
                timing_do,
                arm_staged.parent,
                dataset,
                run,
                manifest["command"],
                arm,
                repetitions,
            )
            timing_output, peak_rss = _run_stata(
                stata, timing_do, work, "END_QL_TIMING|rc=0"
            )
            timing = _parse_timing(timing_output, identifier, repetitions, arm_staged)
            timing.update({"run_id": run["id"], "arm": arm, "process_peak_rss_bytes": peak_rss})
            timings.append(timing)
            arm_results[arm] = timing

        ratio = arm_results["candidate"]["warm_median_seconds"] / arm_results["baseline"]["warm_median_seconds"]
        routed = bool(science["routed"])
        runtime_limit = 1 - float(manifest["promotion"]["minimum_routed_speedup"]) if routed else float(manifest["promotion"]["maximum_unrouted_runtime_ratio"])
        solver_workspace = _solver_workspace_model(run, routed)
        modeled_total = science["execution_modeled_bytes"] + solver_workspace
        allocation_error = abs(modeled_total - science["native_allocation_bytes"]) / max(science["native_allocation_bytes"], 1)
        comparison = {
            "run_id": run["id"],
            "routed": routed,
            "auto_over_cg_warm_runtime": ratio,
            "runtime_gate_limit": runtime_limit,
            "runtime_gate_passed": ratio <= runtime_limit,
            "auto_over_cg_peak_rss_diagnostic": (
                arm_results["candidate"]["process_peak_rss_bytes"]
                / arm_results["baseline"]["process_peak_rss_bytes"]
                if arm_results["baseline"]["process_peak_rss_bytes"] > 0
                else None
            ),
            "modeled_total_allocation_bytes": modeled_total,
            "observed_native_allocation_bytes": science["native_allocation_bytes"],
            "modeled_allocation_relative_error": allocation_error,
            "modeled_allocation_gate_passed": allocation_error <= float(manifest["promotion"]["maximum_modeled_allocation_relative_error"]),
            "no_leak_gate_passed": arm_results["candidate"]["leak_gate_passed"] and arm_results["baseline"]["leak_gate_passed"],
        }
        comparisons.append(comparison)

    failures = [
        comparison["run_id"]
        for comparison in comparisons
        if not comparison["runtime_gate_passed"]
        or not comparison["modeled_allocation_gate_passed"]
        or not comparison["no_leak_gate_passed"]
    ]
    holdout = "holdout" in profile_id or "freeze" in profile_id
    return {
        "schema_version": 1,
        "kind": "fereg_direct_lsmr_qualification",
        "manifest_id": manifest["manifest_id"],
        "profile": profile_id,
        "holdout": holdout,
        "selection_locked_before_profile": True,
        "plugins": identities,
        "dependency_identity": dependency_identity,
        "datasets": list(datasets.values()),
        "scientific": scientific,
        "timings": timings,
        "comparisons": comparisons,
        "verdict": "pass" if not failures else "fail",
        "failures": failures,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    subparsers = parser.add_subparsers(dest="action", required=True)
    plan = subparsers.add_parser("plan")
    plan.add_argument("--profile", required=True)
    run = subparsers.add_parser("run")
    run.add_argument("--profile", required=True)
    run.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    run.add_argument("--plugin", type=Path, required=True)
    run.add_argument("--baseline-plugin", type=Path, required=True)
    run.add_argument("--work-dir", type=Path, required=True)
    run.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args(argv)
    try:
        manifest = load_manifest(arguments.manifest)
        if arguments.action == "plan":
            result: Any = expand_profile(manifest, arguments.profile)
        else:
            result = run_profile(
                manifest,
                arguments.profile,
                arguments.stata,
                arguments.plugin,
                arguments.baseline_plugin,
                arguments.work_dir,
            )
        encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if getattr(arguments, "output", None):
            arguments.output.write_text(encoded, encoding="utf-8")
        else:
            sys.stdout.write(encoded)
        return 0 if not isinstance(result, dict) or result.get("verdict") != "fail" else 1
    except (QualificationError, probe.ProbeError, development.ContractError, OSError, subprocess.SubprocessError) as error:
        print(f"direct_lsmr_qualification: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
