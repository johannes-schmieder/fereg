#!/usr/bin/env python3
"""Run the reproducible fereg solver-development benchmark matrix.

This is a development harness, not a release-qualification runner.  reghdfe is
the scientific and hard-runtime oracle; gregress is recorded as a diagnostic
speed target only.
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


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DEVELOPMENT_MANIFEST = (
    ROOT / "benchmarks/manifests/ols_solver_development_v2.json"
)
DEFAULT_STATA = Path("/Applications/Stata18/StataMP.app/Contents/MacOS/stata-mp")
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]*$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")

EXPECTED_INSTALLED_HASHES = {
    "reghdfe_ado": "e9b96033602a6f1785f1be0b6393b44d48ef0974009ee2bffee308bc251a6abc",
    "reghdfe_mata": "55827e1e3c9bec3a0c9310c1147a24c3b997ca0a5dd256a353d98407f876dcf6",
    "ftools_ado": "c7eba3272b5fcd1dfa157e65c37840a051c6e2bf327d5ee31d42df65cf330b00",
    "require_ado": "0f5a7bad374ade6c6ba0f05973666bb06b63d4acc36ddb0e277a060249a0e6de",
}


class ContractError(RuntimeError):
    """A benchmark input or result failed closed."""


def _expect_keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        missing = sorted(expected - set(value))
        extra = sorted(set(value) - expected)
        raise ContractError(f"{where} fields mismatch: missing={missing}, extra={extra}")


def _unique_ids(values: Sequence[Mapping[str, Any]], where: str) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    for value in values:
        identifier = value.get("id")
        if not isinstance(identifier, str) or not SAFE_ID.fullmatch(identifier):
            raise ContractError(f"{where} id is malformed")
        if identifier in result:
            raise ContractError(f"duplicate {where} id: {identifier}")
        result[identifier] = value
    return result


def load_development_manifest(
    path: Path = DEFAULT_DEVELOPMENT_MANIFEST,
) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to load development manifest: {error}") from error
    if not isinstance(document, dict):
        raise ContractError("solver development manifest must be an object")
    _expect_keys(
        document,
        {
            "schema_version",
            "manifest_id",
            "status",
            "scientific_contract",
            "generator",
            "diagnostic_dependencies",
            "scales",
            "designs",
            "arms",
            "profiles",
            "selection_rules",
        },
        "solver development manifest",
    )
    if (
        document["schema_version"] != 2
        or document["manifest_id"] != "ols_solver_development_v2"
        or document["status"] != "development_only"
    ):
        raise ContractError("solver development matrix identity is not frozen")
    generator = ROOT / str(document["generator"])
    if not generator.is_file():
        raise ContractError("AKM generator is missing")

    dependencies = document["diagnostic_dependencies"]
    if not isinstance(dependencies, dict):
        raise ContractError("diagnostic dependency inventory is malformed")
    _expect_keys(dependencies, {"reghdfe", "gtools"}, "diagnostic dependencies")
    _expect_keys(dependencies["reghdfe"], {"version", "commit"}, "reghdfe pin")
    _expect_keys(
        dependencies["gtools"],
        {"version", "gregress_ado_sha256", "macos_plugin_sha256"},
        "gtools pin",
    )
    if dependencies["reghdfe"] != {
        "version": "6.14.1",
        "commit": "29b2b203f534413369d675d0c0f674d61291f536",
    }:
        raise ContractError("reghdfe dependency is not pinned to 6.14.1")
    if dependencies["gtools"]["version"] != "1.11.8":
        raise ContractError("gtools dependency is not pinned to 1.11.8")
    for field in ("gregress_ado_sha256", "macos_plugin_sha256"):
        if not SHA256.fullmatch(str(dependencies["gtools"][field])):
            raise ContractError(f"gtools {field} is malformed")

    scales = _unique_ids(document["scales"], "scale")
    designs = _unique_ids(document["designs"], "design")
    arms = _unique_ids(document["arms"], "arm")
    profiles = _unique_ids(document["profiles"], "profile")
    if set(scales) != {"n100k", "n1m", "n10m"}:
        raise ContractError("scale inventory is incomplete")
    if set(designs) != {
        "one_fe",
        "iid_two_fe",
        "akm_highmob",
        "akm_standard",
        "akm_lowmob",
    }:
        raise ContractError("design inventory is incomplete")
    expected_arms = {
        "reghdfe_default",
        "reghdfe_map",
        "fereg_auto",
        "fereg_map",
        "fereg_cg",
        "fereg_it",
        "fereg_squarem",
        "fereg_lsmr_off",
        "fereg_lsmr_diagonal",
        "fereg_lsmr_schwarz",
        "fereg_lsmr_auto",
        "gregress_cg",
        "gregress_map",
        "gregress_it",
        "gregress_squarem",
    }
    if set(arms) != expected_arms:
        raise ContractError("algorithm-arm inventory is incomplete")
    if set(profiles) != {"quick", "matrix", "milestone", "freeze"}:
        raise ContractError("profile inventory is incomplete")

    for scale in scales.values():
        _expect_keys(scale, {"id", "n", "workers", "firms", "periods"}, "scale")
        if any(not isinstance(scale[field], int) or scale[field] <= 0 for field in ("n", "workers", "firms", "periods")):
            raise ContractError("scale dimensions must be positive integers")
    for design in designs.values():
        _expect_keys(
            design,
            {"id", "assignment", "mobility", "absorb", "seed"},
            "design",
        )
        if design["assignment"] not in {"akm", "iid"}:
            raise ContractError("design assignment is unknown")
        if not 0 <= float(design["mobility"]) <= 1:
            raise ContractError("design mobility is outside [0,1]")
        if design["absorb"] not in {"worker", "worker firm"}:
            raise ContractError("design absorb topology is unknown")
    for arm in arms.values():
        expected = {"id", "estimator", "algorithm"}
        if arm.get("estimator") == "fereg" and arm.get("algorithm") == "lsmr":
            expected.add("preconditioner")
        _expect_keys(arm, expected, "arm")
        if arm["estimator"] not in {"reghdfe", "fereg", "gregress"}:
            raise ContractError("arm estimator is unknown")
    for profile in profiles.values():
        _expect_keys(
            profile,
            {"id", "scale", "designs", "threads", "arms", "repetitions"},
            "profile",
        )
        if profile["scale"] not in scales:
            raise ContractError("profile binds an unknown scale")
        if not profile["designs"] or any(item not in designs for item in profile["designs"]):
            raise ContractError("profile binds an unknown design")
        if profile["threads"] != [1, 8]:
            raise ContractError("profile thread counts are not frozen")
        if not profile["arms"] or any(item not in arms for item in profile["arms"]):
            raise ContractError("profile binds an unknown arm")
        _expect_keys(profile["repetitions"], {"cold", "warm"}, "repetitions")
        if profile["repetitions"]["cold"] != 1 or profile["repetitions"]["warm"] < 1:
            raise ContractError("profile repetition policy is malformed")

    expected_rules = {
        "scientific_oracle": "reghdfe",
        "gregress_role": "diagnostic_only",
        "setup_cost_included": True,
        "default_auto_only_release_gated": True,
        "development_results_do_not_qualify_release": True,
        "coefficient_rtol": 1e-7,
        "coefficient_atol": 1e-10,
        "vce_rtol": 1e-6,
        "vce_atol": 1e-10,
    }
    if document["selection_rules"] != expected_rules:
        raise ContractError("solver development selection rules are not frozen")
    return document


def arm_command(arm: Mapping[str, Any], absorb: str, threads: int) -> str:
    """Render one explicit, comparator-auditable Stata command."""
    estimator = arm["estimator"]
    algorithm = arm["algorithm"]
    base = f"y x1 x2, absorb({absorb})"
    if estimator == "reghdfe":
        acceleration = "conjugate_gradient" if algorithm == "cg" else "none"
        return (
            f"reghdfe {base} transform(symmetric_kaczmarz) "
            f"acceleration({acceleration}) tolerance(1e-8) iterate(16000)"
        )
    if estimator == "fereg":
        options = f"solver({algorithm})"
        if algorithm == "lsmr":
            options += f" preconditioner({arm['preconditioner']})"
        return (
            f"fereg {base} {options} threads({threads}) "
            "tolerance(1e-8) iterate(16000)"
        )
    if estimator == "gregress":
        return (
            f"gregress {base} algorithm({algorithm}) hdfetol(1e-8) "
            "maxiter(16000) mata(GtoolsRegress)"
        )
    raise ContractError(f"unknown estimator: {estimator}")


def expand_run_plan(
    development: Mapping[str, Any], profile_id: str = "quick"
) -> list[dict[str, Any]]:
    scales = {item["id"]: item for item in development["scales"]}
    designs = {item["id"]: item for item in development["designs"]}
    arms = {item["id"]: item for item in development["arms"]}
    profiles = {item["id"]: item for item in development["profiles"]}
    if profile_id not in profiles:
        raise ContractError(f"unknown profile: {profile_id}")
    profile = profiles[profile_id]
    scale = scales[profile["scale"]]
    plan: list[dict[str, Any]] = []
    for design_id in profile["designs"]:
        design = designs[design_id]
        for threads in profile["threads"]:
            case_id = f"{design_id}_{scale['id']}_t{threads}"
            for arm_id in profile["arms"]:
                arm = arms[arm_id]
                plan.append(
                    {
                        "id": f"{case_id}__{arm_id}",
                        "case_id": case_id,
                        "profile": profile_id,
                        "scale": dict(scale),
                        "design": dict(design),
                        "threads": threads,
                        "arm": dict(arm),
                        "command": arm_command(arm, design["absorb"], threads),
                        "repetitions": dict(profile["repetitions"]),
                    }
                )
    return plan


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _source_identity() -> dict[str, Any]:
    tracked_roots = [ROOT / "crates", ROOT / "legacy", ROOT / "stata"]
    files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    for directory in tracked_roots:
        files.extend(path for path in directory.rglob("*") if path.is_file())
    digest = hashlib.sha256()
    for path in sorted(files):
        relative = path.relative_to(ROOT).as_posix().encode()
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(path.read_bytes())
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    plugin = ROOT / "dist/fereg_macos.plugin"
    if not plugin.is_file():
        raise ContractError("dist/fereg_macos.plugin is missing; build before benchmarking")
    return {
        "git_head": completed.stdout.strip(),
        "source_tree_sha256": digest.hexdigest(),
        "plugin_path": str(plugin),
        "plugin_sha256": _sha256(plugin),
    }


def _stata_preamble(threads: int) -> list[str]:
    return [
        "version 18.0",
        "set more off",
        "set linesize 255",
        f"set processors {threads}",
        f'adopath ++ "{(ROOT / "stata").as_posix()}"',
        f'adopath ++ "{(ROOT / "dist").as_posix()}"',
        'global GTOOLS_BETA "I KNOW WHAT I AM DOING"',
    ]


def _run_stata(stata: Path, do_file: Path, cwd: Path) -> str:
    completed = subprocess.run(
        [str(stata), "-q", "-b", "do", str(do_file)],
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    log = cwd / f"{do_file.stem}.log"
    output = completed.stdout
    if log.is_file():
        output += "\n" + log.read_text(encoding="utf-8", errors="replace")
    if completed.returncode != 0:
        raise ContractError(
            f"Stata process failed for {do_file.name} with {completed.returncode}\n{output[-4000:]}"
        )
    return output


def _write_do(path: Path, lines: Iterable[str]) -> None:
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _dependency_probe(stata: Path, work: Path, development: Mapping[str, Any]) -> dict[str, Any]:
    do_file = work / "dependency_probe.do"
    names = {
        "reghdfe_ado": "reghdfe.ado",
        "reghdfe_mata": "reghdfe.mata",
        "ftools_ado": "ftools.ado",
        "require_ado": "require.ado",
        "gregress_ado": "gregress.ado",
        "gtools_plugin": "gtools_macosx_v3.plugin",
    }
    lines = _stata_preamble(1)
    for key, filename in names.items():
        lines.extend(
            [
                f"capture findfile {filename}",
                "if _rc exit _rc",
                f"display as result \"DEP|name={key}|path=`r(fn)'\"",
            ]
        )
    lines.extend(
        [
            "clear",
            "set obs 20",
            "generate int worker=ceil(_n/2)",
            "generate double x=_n",
            "generate double y=2*x+worker",
            "capture noisily reghdfe y x, absorb(worker)",
            "local rc=_rc",
            "display as result \"DEP_SMOKE|rc=`rc'|b=\" %21.16g _b[x]",
            "if `rc' exit `rc'",
            'display as result "END_DEPENDENCY_PROBE|rc=0"',
            "exit 0",
        ]
    )
    _write_do(do_file, lines)
    output = _run_stata(stata, do_file, work)
    records = _records(output, "DEP")
    found = {record["name"]: Path(record["path"]) for record in records}
    if set(found) != set(names):
        raise ContractError("dependency probe did not resolve the exact inventory")
    hashes = {name: _sha256(path) for name, path in found.items()}
    expected = dict(EXPECTED_INSTALLED_HASHES)
    gtools = development["diagnostic_dependencies"]["gtools"]
    expected["gregress_ado"] = gtools["gregress_ado_sha256"]
    expected["gtools_plugin"] = gtools["macos_plugin_sha256"]
    mismatches = {
        name: {"expected": expected[name], "observed": hashes[name]}
        for name in expected
        if hashes.get(name) != expected[name]
    }
    if mismatches:
        raise ContractError(f"installed dependency hash mismatch: {mismatches}")
    return {
        name: {"path": str(found[name]), "sha256": hashes[name]}
        for name in sorted(found)
    }


def _generate_dataset(
    stata: Path,
    work: Path,
    generator: Path,
    scale: Mapping[str, Any],
    design: Mapping[str, Any],
) -> dict[str, Any]:
    dataset = work / f"{design['id']}_{scale['id']}.dta"
    do_file = work / f"generate_{design['id']}_{scale['id']}.do"
    args = [
        scale["n"],
        scale["workers"],
        scale["firms"],
        scale["periods"],
        design["mobility"],
        design["assignment"],
        design["seed"],
        f'"{dataset.as_posix()}"',
    ]
    lines = _stata_preamble(1) + [
        f'do "{generator.as_posix()}" ' + " ".join(map(str, args))
    ]
    _write_do(do_file, lines)
    output = _run_stata(stata, do_file, work)
    if "END_AKM_GENERATOR|rc=0" not in output or not dataset.is_file():
        raise ContractError(f"AKM generation failed for {design['id']}/{scale['id']}")
    return {
        "path": str(dataset),
        "sha256": _sha256(dataset),
        "bytes": dataset.stat().st_size,
        "generator_sha256": _sha256(generator),
        "parameters": {**dict(scale), **dict(design)},
    }


def _preflight_do(
    path: Path,
    dataset: Path,
    case_id: str,
    absorb: str,
    threads: int,
) -> None:
    reference = arm_command(
        {"estimator": "reghdfe", "algorithm": "cg"}, absorb, threads
    ).replace("tolerance(1e-8)", "tolerance(1e-10)")
    candidate = arm_command(
        {"estimator": "fereg", "algorithm": "auto"}, absorb, threads
    ).replace("tolerance(1e-8)", "tolerance(1e-10)")
    diagnostic = arm_command(
        {"estimator": "gregress", "algorithm": "cg"}, absorb, threads
    ).replace("hdfetol(1e-8)", "hdfetol(1e-10)")
    lines = _stata_preamble(threads) + [
        f'use "{dataset.as_posix()}", clear',
        "capture drop __frg_ref_resid __frg_can_resid __frg_ref_sample __frg_resdiff __frg_resratio",
        f"capture noisily {reference} residuals(__frg_ref_resid)",
        "local __frg_rc=_rc",
        "if `__frg_rc' exit `__frg_rc'",
        "matrix __frg_ref_b=e(b)",
        "matrix __frg_ref_v=e(V)",
        "scalar __frg_ref_n=e(N)",
        "scalar __frg_ref_rss=e(rss)",
        "scalar __frg_ref_r2=e(r2)",
        "capture scalar __frg_ref_dfa=e(df_a)",
        "if _rc scalar __frg_ref_dfa=.",
        "capture scalar __frg_ref_sing=e(num_singletons)",
        "if _rc scalar __frg_ref_sing=.",
        "generate byte __frg_ref_sample=e(sample)",
        f"capture noisily {candidate} residuals(__frg_can_resid)",
        "local __frg_rc=_rc",
        "if `__frg_rc' exit `__frg_rc'",
        "matrix __frg_can_b=e(b)",
        "matrix __frg_can_v=e(V)",
        "scalar __frg_can_n=e(N)",
        "scalar __frg_can_rss=e(rss)",
        "scalar __frg_can_r2=e(r2)",
        "capture scalar __frg_can_dfa=e(df_a)",
        "if _rc scalar __frg_can_dfa=.",
        "capture scalar __frg_can_sing=e(num_singletons)",
        "if _rc scalar __frg_can_sing=.",
        "count if __frg_ref_sample != e(sample)",
        "scalar __frg_samplediff=r(N)",
        "mata: st_numscalar(\"__frg_bmax\", max(abs(st_matrix(\"__frg_ref_b\") :- st_matrix(\"__frg_can_b\"))))",
        "mata: st_numscalar(\"__frg_bscale\", max(abs(st_matrix(\"__frg_ref_b\"))))",
        "mata: st_numscalar(\"__frg_vmax\", max(abs(st_matrix(\"__frg_ref_v\") :- st_matrix(\"__frg_can_v\"))))",
        "mata: st_numscalar(\"__frg_vscale\", max(abs(st_matrix(\"__frg_ref_v\"))))",
        "scalar __frg_bscale=max(__frg_bscale,1)",
        "scalar __frg_vscale=max(__frg_vscale,1)",
        "generate double __frg_resdiff=abs(__frg_ref_resid-__frg_can_resid) if __frg_ref_sample",
        "quietly summarize __frg_resdiff, meanonly",
        "scalar __frg_resmax=r(max)",
        "quietly summarize __frg_ref_resid if __frg_ref_sample, meanonly",
        "scalar __frg_resscale=max(abs(r(min)),abs(r(max)),1)",
        "quietly summarize __frg_can_resid if __frg_ref_sample, meanonly",
        "scalar __frg_resscale=max(__frg_resscale,abs(r(min)),abs(r(max)))",
        "generate double __frg_resratio=__frg_resdiff/(1e-9+1e-7*max(abs(__frg_ref_resid),abs(__frg_can_resid))) if __frg_ref_sample",
        "quietly summarize __frg_resratio, meanonly",
        "scalar __frg_resnorm=r(max)",
        "scalar __frg_rssrel=abs(__frg_ref_rss-__frg_can_rss)/max(abs(__frg_ref_rss),1)",
        f'display as result "PREFLIGHT_A|case={case_id}|bmax=" %21.16g __frg_bmax "|bscale=" %21.16g __frg_bscale "|vmax=" %21.16g __frg_vmax "|vscale=" %21.16g __frg_vscale',
        f'display as result "PREFLIGHT_B|case={case_id}|nref=" %21.16g __frg_ref_n "|ncan=" %21.16g __frg_can_n "|samplediff=" %21.16g __frg_samplediff "|rssrel=" %21.16g __frg_rssrel "|r2diff=" %21.16g abs(__frg_ref_r2-__frg_can_r2)',
        f'display as result "PREFLIGHT_C|case={case_id}|dfaref=" %21.16g __frg_ref_dfa "|dfacan=" %21.16g __frg_can_dfa "|singref=" %21.16g __frg_ref_sing "|singcan=" %21.16g __frg_can_sing "|resmax=" %21.16g __frg_resmax "|resscale=" %21.16g __frg_resscale "|resnorm=" %21.16g __frg_resnorm',
        f'display as result "PREFLIGHT_D|case={case_id}|n=" %21.16g __frg_ref_n "|b1=" %21.16g __frg_ref_b[1,1] "|b2=" %21.16g __frg_ref_b[1,2] "|rss=" %21.16g __frg_ref_rss',
        f'display as result "PREFLIGHT_E|case={case_id}|v11=" %21.16g __frg_ref_v[1,1] "|v12=" %21.16g __frg_ref_v[1,2] "|v22=" %21.16g __frg_ref_v[2,2] "|r2=" %21.16g __frg_ref_r2 "|dfa=" %21.16g __frg_ref_dfa',
        "matrix __frg_oracle_b=__frg_ref_b",
        "matrix __frg_oracle_v=__frg_ref_v",
        f"capture noisily {diagnostic}",
        "local __frg_grc=_rc",
        "if `__frg_grc' exit `__frg_grc'",
        "scalar __frg_gn=r(N)",
        "mata: st_numscalar(\"__frg_gb1\", GtoolsRegress.b[1,1])",
        "mata: st_numscalar(\"__frg_gb2\", GtoolsRegress.b[1,2])",
        "scalar __frg_gmax=max(abs(__frg_oracle_b[1,1]-__frg_gb1),abs(__frg_oracle_b[1,2]-__frg_gb2))",
        f'display as result "GREG_PREFLIGHT|case={case_id}|n=" %21.16g __frg_gn "|b1=" %21.16g __frg_gb1 "|b2=" %21.16g __frg_gb2 "|bmax=" %21.16g __frg_gmax',
        'display as result "END_PREFLIGHT|rc=0"',
        "exit 0",
    ]
    _write_do(path, lines)


def _timing_do(path: Path, dataset: Path, run: Mapping[str, Any]) -> None:
    arm = run["arm"]
    estimator = arm["estimator"]
    repetitions = int(run["repetitions"]["cold"]) + int(run["repetitions"]["warm"])
    lines = _stata_preamble(int(run["threads"])) + [
        f'use "{dataset.as_posix()}", clear',
        "timer clear 71",
        f"forvalues __frg_rep=1/{repetitions} {{",
        "    local __frg_kind=cond(`__frg_rep'==1,\"cold\",\"warm\")",
        "    scalar __frg_n=.",
        "    scalar __frg_b1=.",
        "    scalar __frg_b2=.",
        "    scalar __frg_v11=.",
        "    scalar __frg_v12=.",
        "    scalar __frg_v22=.",
        "    scalar __frg_rss=.",
        "    scalar __frg_r2=.",
        "    scalar __frg_dfa=.",
        "    timer on 71",
        # Estimator benchmarks time computation and e-class posting, not log
        # table rendering. Scientific diagnostics are extracted immediately
        # afterward, while preflights remain noisy for actionable failures.
        f"    capture quietly {run['command']}",
        "    local __frg_rc=_rc",
    ]
    if estimator == "gregress":
        lines.extend(
            [
                "    if `__frg_rc'==0 {",
                "        scalar __frg_n=r(N)",
                "        mata: st_numscalar(\"__frg_b1\", GtoolsRegress.b[1,1])",
                "        mata: st_numscalar(\"__frg_b2\", GtoolsRegress.b[1,2])",
                "        mata: st_numscalar(\"__frg_v11\", GtoolsRegress.Vcov[1,1])",
                "        mata: st_numscalar(\"__frg_v12\", GtoolsRegress.Vcov[1,2])",
                "        mata: st_numscalar(\"__frg_v22\", GtoolsRegress.Vcov[2,2])",
                "        scalar __frg_rss=. ",
                "        scalar __frg_r2=. ",
                "        scalar __frg_dfa=. ",
                "    }",
            ]
        )
    else:
        lines.extend(
            [
                "    if `__frg_rc'==0 {",
                "        matrix __frg_b=e(b)",
                "        matrix __frg_v=e(V)",
                "        scalar __frg_n=e(N)",
                "        scalar __frg_b1=__frg_b[1,1]",
                "        scalar __frg_b2=__frg_b[1,2]",
                "        scalar __frg_v11=__frg_v[1,1]",
                "        scalar __frg_v12=__frg_v[1,2]",
                "        scalar __frg_v22=__frg_v[2,2]",
                "        scalar __frg_rss=e(rss)",
                "        scalar __frg_r2=e(r2)",
                "        capture scalar __frg_dfa=e(df_a)",
                "        if _rc scalar __frg_dfa=. ",
                "    }",
            ]
        )
    if estimator == "fereg":
        lines.extend(
            [
                "    if `__frg_rc'==0 {",
                "        matrix __frg_solver=e(fereg_solver)",
                "        matrix __frg_runtime=e(fereg_runtime)",
                "    }",
            ]
        )
    lines.extend(
        [
            "    timer off 71",
            "    quietly timer list 71",
            "    scalar __frg_seconds=r(t71)",
            "    timer clear 71",
            f"    display as result \"RUN_A|id={run['id']}|rep=`__frg_rep'|kind=`__frg_kind'|rc=`__frg_rc'|seconds=\" %21.16g __frg_seconds \"|n=\" %21.16g __frg_n \"|b1=\" %21.16g __frg_b1 \"|b2=\" %21.16g __frg_b2",
            "    if `__frg_rc'==0 {",
            f"    display as result \"RUN_B|id={run['id']}|rep=`__frg_rep'|v11=\" %21.16g __frg_v11 \"|v12=\" %21.16g __frg_v12 \"|v22=\" %21.16g __frg_v22 \"|rss=\" %21.16g __frg_rss \"|r2=\" %21.16g __frg_r2 \"|dfa=\" %21.16g __frg_dfa",
        ]
    )
    if estimator == "fereg":
        lines.extend(
            [
                f"    display as result \"DIAG_A|id={run['id']}|rep=`__frg_rep'|requested=\" %21.16g __frg_solver[1,1] \"|used=\" %21.16g __frg_solver[1,2] \"|iters=\" %21.16g __frg_solver[1,3] \"|stop=\" %21.16g __frg_solver[1,4] \"|absinf=\" %21.16g __frg_solver[1,5] \"|relinf=\" %21.16g __frg_solver[1,6]",
                f"    display as result \"DIAG_B|id={run['id']}|rep=`__frg_rep'|rel2=\" %21.16g __frg_solver[1,7] \"|neq=\" %21.16g __frg_solver[1,8] \"|setup=\" %21.16g __frg_solver[1,9] \"|solve=\" %21.16g __frg_solver[1,10] \"|fallback=\" %21.16g __frg_solver[1,11]",
                f"    display as result \"DIAG_C|id={run['id']}|rep=`__frg_rep'|pilot_iters=\" %21.16g __frg_solver[1,15] \"|pilot_contract=\" %21.16g __frg_solver[1,16] \"|native=\" %21.16g __frg_runtime[20,2] \"|precommit=\" %21.16g __frg_runtime[21,2] \"|threads=\" %21.16g __frg_runtime[29,2]",
            ]
        )
    lines.extend(
        [
            "    }",
            "    if `__frg_rc' exit `__frg_rc'",
            "}",
            'display as result "END_TIMING|rc=0"',
            "exit 0",
        ]
    )
    _write_do(path, lines)


def _records(output: str, prefix: str) -> list[dict[str, str]]:
    records: list[dict[str, str]] = []
    marker = prefix + "|"
    for raw in output.splitlines():
        line = raw.strip()
        if not line.startswith(marker):
            continue
        record: dict[str, str] = {}
        for field in line[len(marker) :].split("|"):
            if "=" not in field:
                raise ContractError(f"malformed {prefix} record: {line}")
            key, value = field.split("=", 1)
            record[key.strip()] = value.strip()
        records.append(record)
    return records


def _number(record: Mapping[str, str], field: str) -> float:
    try:
        raw = record[field]
        value = math.nan if raw == "." or re.fullmatch(r"\.[a-z]", raw) else float(raw)
    except (KeyError, ValueError) as error:
        raise ContractError(f"missing or malformed numeric field {field}") from error
    return value


def _nullable_number(record: Mapping[str, str], field: str) -> float | None:
    value = _number(record, field)
    return value if math.isfinite(value) else None


def _parse_preflight(output: str, rules: Mapping[str, Any]) -> dict[str, Any]:
    parts: dict[str, dict[str, str]] = {}
    for prefix in (
        "PREFLIGHT_A",
        "PREFLIGHT_B",
        "PREFLIGHT_C",
        "PREFLIGHT_D",
        "PREFLIGHT_E",
        "GREG_PREFLIGHT",
    ):
        records = _records(output, prefix)
        if len(records) != 1:
            raise ContractError(f"preflight emitted {len(records)} {prefix} records")
        parts[prefix] = records[0]
    a, b, c, d, e, g = (
        parts[name]
        for name in (
            "PREFLIGHT_A",
            "PREFLIGHT_B",
            "PREFLIGHT_C",
            "PREFLIGHT_D",
            "PREFLIGHT_E",
            "GREG_PREFLIGHT",
        )
    )
    if len({part["case"] for part in (a, b, c, d, e, g)}) != 1:
        raise ContractError("preflight case ids disagree")
    coefficient_limit = float(rules["coefficient_atol"]) + float(rules["coefficient_rtol"]) * _number(a, "bscale")
    vce_limit = float(rules["vce_atol"]) + float(rules["vce_rtol"]) * _number(a, "vscale")
    failures: list[str] = []
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
    singref, singcan = _number(c, "singref"), _number(c, "singcan")
    if math.isfinite(singref) and singref != singcan:
        failures.append("singleton parity")
    residual_limit = 1e-9 + 1e-7 * _number(c, "resscale")
    if _number(c, "resmax") > residual_limit:
        failures.append("residual parity")
    if failures:
        raise ContractError("scientific preflight failed: " + ", ".join(failures))
    return {
        "case_id": a["case"],
        "oracle": {
            "n": int(_number(d, "n")),
            "coefficients": [_number(d, "b1"), _number(d, "b2")],
            "vce": [_number(e, "v11"), _number(e, "v12"), _number(e, "v22")],
            "rss": _number(d, "rss"),
            "r2": _number(e, "r2"),
            "df_a": _nullable_number(e, "dfa"),
        },
        "candidate": {
            "coefficient_max_abs_diff": _number(a, "bmax"),
            "vce_max_abs_diff": _number(a, "vmax"),
            "n_reference": int(_number(b, "nref")),
            "n_candidate": int(_number(b, "ncan")),
            "sample_mismatches": int(_number(b, "samplediff")),
            "rss_relative_diff": _number(b, "rssrel"),
            "r2_abs_diff": _number(b, "r2diff"),
            "df_a_reference": _number(c, "dfaref"),
            "df_a_candidate": _number(c, "dfacan"),
            "singleton_reference": singref,
            "singleton_candidate": singcan,
            "residual_max_abs_diff": _number(c, "resmax"),
            "residual_scale": _number(c, "resscale"),
            "residual_max_normalized_diff": _number(c, "resnorm"),
        },
        "gregress_diagnostic": {
            "n": int(_number(g, "n")),
            "b1": _number(g, "b1"),
            "b2": _number(g, "b2"),
            "coefficient_max_abs_diff": _number(g, "bmax"),
        },
    }


def _parse_timing(output: str, run: Mapping[str, Any]) -> dict[str, Any]:
    expected = int(run["repetitions"]["cold"]) + int(run["repetitions"]["warm"])
    a_records = _records(output, "RUN_A")
    b_records = _records(output, "RUN_B")
    for record in a_records:
        if record.get("id") != run["id"]:
            raise ContractError("timing record identity mismatch")
        return_code = int(_number(record, "rc"))
        if return_code != 0:
            repetition = int(_number(record, "rep"))
            raise ContractError(
                f"benchmark arm failed: {run['id']} repetition {repetition} rc={return_code}"
            )
    if len(a_records) != expected or len(b_records) != expected:
        raise ContractError(f"timing record count mismatch for {run['id']}")
    b_by_rep = {int(_number(record, "rep")): record for record in b_records}
    diagnostics: dict[int, dict[str, str]] = {}
    if run["arm"]["estimator"] == "fereg":
        for prefix in ("DIAG_A", "DIAG_B", "DIAG_C"):
            records = _records(output, prefix)
            if len(records) != expected:
                raise ContractError(f"{prefix} record count mismatch for {run['id']}")
            for record in records:
                diagnostics.setdefault(int(_number(record, "rep")), {}).update(record)
    repetitions: list[dict[str, Any]] = []
    for a in a_records:
        rep = int(_number(a, "rep"))
        b = b_by_rep.get(rep)
        if b is None or a.get("id") != run["id"] or b.get("id") != run["id"]:
            raise ContractError("timing record identity mismatch")
        if int(_number(a, "rc")) != 0:
            raise ContractError(f"benchmark arm failed: {run['id']}")
        seconds = _number(a, "seconds")
        if not math.isfinite(seconds) or seconds <= 0:
            raise ContractError("benchmark timing is nonpositive or nonfinite")
        item: dict[str, Any] = {
            "repetition": rep,
            "kind": a["kind"],
            "seconds": seconds,
            "n": int(_number(a, "n")),
            "coefficients": [_number(a, "b1"), _number(a, "b2")],
            "vce": [_number(b, "v11"), _number(b, "v12"), _number(b, "v22")],
            "rss": _nullable_number(b, "rss"),
            "r2": _nullable_number(b, "r2"),
            "df_a": _nullable_number(b, "dfa"),
        }
        if diagnostics:
            diag = diagnostics[rep]
            item["solver"] = {
                key: _number(diag, key)
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
                    "pilot_iters",
                    "pilot_contract",
                    "native",
                    "precommit",
                    "threads",
                )
            }
        repetitions.append(item)
    repetitions.sort(key=lambda item: item["repetition"])
    warm = [item["seconds"] for item in repetitions if item["kind"] == "warm"]
    if len(warm) != int(run["repetitions"]["warm"]):
        raise ContractError("warm timing count mismatch")
    median = statistics.median(warm)
    mad = statistics.median(abs(value - median) for value in warm)
    return {
        **{key: run[key] for key in ("id", "case_id", "profile", "threads", "arm", "command")},
        "repetitions": repetitions,
        "cold_seconds": repetitions[0]["seconds"],
        "warm_median_seconds": median,
        "warm_mad_seconds": mad,
        "warm_min_seconds": min(warm),
        "warm_max_seconds": max(warm),
    }


def _within_tolerance(actual: float, expected: float, atol: float, rtol: float) -> bool:
    return math.isfinite(actual) and math.isfinite(expected) and abs(actual - expected) <= (
        atol + rtol * max(abs(actual), abs(expected), 1.0)
    )


def _validate_timed_science(
    results: Sequence[Mapping[str, Any]],
    preflights: Mapping[str, Mapping[str, Any]],
    rules: Mapping[str, Any],
) -> None:
    """Require every timed reghdfe/fereg result to match the tight oracle.

    gregress remains diagnostic-only because its beta Mata adapter does not
    expose the full fit/DoF surface and can retain a different singleton sample.
    """
    coefficient_atol = float(rules["coefficient_atol"])
    coefficient_rtol = float(rules["coefficient_rtol"])
    vce_atol = float(rules["vce_atol"])
    vce_rtol = float(rules["vce_rtol"])
    for result in results:
        if result["arm"]["estimator"] == "gregress":
            continue
        oracle = preflights[result["case_id"]]["oracle"]
        for repetition in result["repetitions"]:
            failures: list[str] = []
            if repetition["n"] != oracle["n"]:
                failures.append("sample")
            if len(repetition["coefficients"]) != len(oracle["coefficients"]) or any(
                not _within_tolerance(actual, expected, coefficient_atol, coefficient_rtol)
                for actual, expected in zip(
                    repetition["coefficients"], oracle["coefficients"]
                )
            ):
                failures.append("coefficients")
            if len(repetition["vce"]) != len(oracle["vce"]) or any(
                not _within_tolerance(actual, expected, vce_atol, vce_rtol)
                for actual, expected in zip(repetition["vce"], oracle["vce"])
            ):
                failures.append("VCE")
            if repetition["rss"] is None or not _within_tolerance(
                repetition["rss"], oracle["rss"], 1e-9, 1e-7
            ):
                failures.append("RSS")
            if repetition["r2"] is None or not _within_tolerance(
                repetition["r2"], oracle["r2"], 1e-10, 1e-10
            ):
                failures.append("R2")
            if repetition["df_a"] != oracle["df_a"]:
                failures.append("absorbed DoF")
            if failures:
                raise ContractError(
                    f"timed scientific parity failed for {result['id']} repetition "
                    f"{repetition['repetition']}: {', '.join(failures)}"
                )


def _matches_filter(value: str, filters: Sequence[str] | None) -> bool:
    return not filters or value in filters


def run_profile(
    development: Mapping[str, Any],
    profile_id: str,
    stata: Path,
    work: Path,
    *,
    case_filters: Sequence[str] | None = None,
    arm_filters: Sequence[str] | None = None,
    continue_on_arm_failure: bool = False,
) -> dict[str, Any]:
    if work.exists():
        raise ContractError(f"work directory already exists: {work}")
    work.mkdir(parents=True)
    plan = [
        run
        for run in expand_run_plan(development, profile_id)
        if _matches_filter(run["case_id"], case_filters)
        and _matches_filter(run["arm"]["id"], arm_filters)
    ]
    if not plan:
        raise ContractError("filters selected no benchmark runs")
    unknown_cases = set(case_filters or ()) - {run["case_id"] for run in expand_run_plan(development, profile_id)}
    unknown_arms = set(arm_filters or ()) - {run["arm"]["id"] for run in expand_run_plan(development, profile_id)}
    if unknown_cases or unknown_arms:
        raise ContractError(f"unknown filters: cases={sorted(unknown_cases)}, arms={sorted(unknown_arms)}")
    if not stata.is_file():
        raise ContractError(f"Stata executable is missing: {stata}")

    dependency_identity = _dependency_probe(stata, work, development)
    generator = ROOT / development["generator"]
    datasets: dict[tuple[str, str], dict[str, Any]] = {}
    for run in plan:
        key = (run["design"]["id"], run["scale"]["id"])
        if key not in datasets:
            datasets[key] = _generate_dataset(
                stata, work, generator, run["scale"], run["design"]
            )

    preflights: dict[str, dict[str, Any]] = {}
    for run in plan:
        if run["case_id"] in preflights:
            continue
        dataset = Path(datasets[(run["design"]["id"], run["scale"]["id"])]["path"])
        do_file = work / f"preflight_{run['case_id']}.do"
        _preflight_do(
            do_file,
            dataset,
            run["case_id"],
            run["design"]["absorb"],
            int(run["threads"]),
        )
        output = _run_stata(stata, do_file, work)
        preflights[run["case_id"]] = _parse_preflight(
            output, development["selection_rules"]
        )

    results: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for run in plan:
        dataset = Path(datasets[(run["design"]["id"], run["scale"]["id"])]["path"])
        do_file = work / f"timing_{run['id']}.do"
        _timing_do(do_file, dataset, run)
        output = _run_stata(stata, do_file, work)
        try:
            parsed = _parse_timing(output, run)
        except ContractError as error:
            if not continue_on_arm_failure:
                raise
            failures.append(
                {
                    **{
                        key: run[key]
                        for key in ("id", "case_id", "profile", "threads", "arm", "command")
                    },
                    "stage": "timing",
                    "error": str(error),
                }
            )
            continue
        parsed["dataset_sha256"] = datasets[(run["design"]["id"], run["scale"]["id"])]["sha256"]
        results.append(parsed)

    if continue_on_arm_failure:
        certified_results: list[dict[str, Any]] = []
        for result in results:
            try:
                _validate_timed_science(
                    [result], preflights, development["selection_rules"]
                )
            except ContractError as error:
                failures.append(
                    {
                        **{
                            key: result[key]
                            for key in ("id", "case_id", "profile", "threads", "arm", "command")
                        },
                        "stage": "scientific_parity",
                        "error": str(error),
                    }
                )
            else:
                certified_results.append(result)
        results = certified_results
    else:
        _validate_timed_science(results, preflights, development["selection_rules"])

    return {
        "schema_version": 1,
        "kind": "fereg_solver_development",
        "qualification_eligible": False,
        "manifest_id": development["manifest_id"],
        "profile": profile_id,
        "stata_executable": str(stata),
        "source_identity": _source_identity(),
        "dependency_identity": dependency_identity,
        "datasets": list(datasets.values()),
        "preflights": [preflights[key] for key in sorted(preflights)],
        "runs": results,
        "failures": failures,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_DEVELOPMENT_MANIFEST)
    subparsers = parser.add_subparsers(dest="action", required=True)
    plan_parser = subparsers.add_parser("plan", help="validate and print a run plan")
    plan_parser.add_argument("--profile", choices=("quick", "matrix", "milestone", "freeze"), default="quick")
    plan_parser.add_argument("--output", type=Path)
    run_parser = subparsers.add_parser("run", help="execute a development profile")
    run_parser.add_argument("--profile", choices=("quick", "matrix", "milestone", "freeze"), default="quick")
    run_parser.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    run_parser.add_argument("--work-dir", type=Path, required=True)
    run_parser.add_argument("--output", type=Path, required=True)
    run_parser.add_argument("--case-id", action="append", dest="case_ids")
    run_parser.add_argument("--arm", action="append", dest="arms")
    run_parser.add_argument(
        "--continue-on-arm-failure",
        action="store_true",
        help="record a failed diagnostic arm and continue the exploratory profile",
    )
    args = parser.parse_args(argv)
    try:
        development = load_development_manifest(args.manifest)
        if args.action == "plan":
            document: Any = expand_run_plan(development, args.profile)
            encoded = json.dumps(document, sort_keys=True, indent=2) + "\n"
            if args.output:
                args.output.write_text(encoded, encoding="utf-8")
            else:
                sys.stdout.write(encoded)
            return 0
        evidence = run_profile(
            development,
            args.profile,
            args.stata,
            args.work_dir,
            case_filters=args.case_ids,
            arm_filters=args.arms,
            continue_on_arm_failure=args.continue_on_arm_failure,
        )
        args.output.write_text(json.dumps(evidence, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        return 0
    except (ContractError, OSError, subprocess.SubprocessError) as error:
        print(f"solver_development: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
