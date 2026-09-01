#!/usr/bin/env python3
"""Independent canonical semantic profile for PPML qualification compilers."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Mapping

try:
    from scripts import ppml_memory_performance as performance
    from scripts import feppml_separation_recipe_contract as separation_contract
except ImportError:
    import ppml_memory_performance as performance  # type: ignore[no-redef]
    import feppml_separation_recipe_contract as separation_contract  # type: ignore[no-redef]


class QualificationRecipeContractError(RuntimeError):
    pass


ORACLE_IDS = (
    "dev_n100k_no_fe_k0_t1", "dev_n100k_one_fe_offset_fw_c1_t2",
    "dev_n100k_lowmob_exposure_pw_c2_t4", "dev_n100k_three_fe_separated_c3_t8",
    "dev_n100k_slopes_collinear_c4_t16", "dev_n100k_disconnected_illcond_c5_t1",
    "scale_n100k_two_fe_fast_t8",
)
MAIN_GENERATOR_SHA256 = dict(line.split("=", 1) for line in """dev_n100k_no_fe_k0_n100k=a426393526b7075d4ed8929349ab74c56d85f919b52a812100bf8c6e94d03adc
dev_n100k_one_fe_offset_fw_c1_n100k=2ba8904c4803c68cfcdd9e8db0e6efcd90960679de86de1cbdb2a661d4a8bca7
dev_n100k_lowmob_exposure_pw_c2_n100k=6c29baebebff52ed156db1da08e820a28f8d59f858f9daff880149fad6e9fff1
dev_n100k_three_fe_separated_c3_n100k=7afe6882addaab08932d2b79e973416d11a66871bf10130ff7fc72ec1acf96e6
dev_n100k_slopes_collinear_c4_n100k=c9f03d6ce88b8a097f30ef1f4b1df818f38617980143c6a57b28e409ce368fc5
dev_n100k_disconnected_illcond_c5_n100k=c4427b8603f81e723735a3021e63e890df4b2849a39d410f2159084ff3d77246
scale_n100k_two_fe_fast_n100k=f32296dd0d196597cc76faa252f1b0e49e2c1f625bea5d0a7f9aa21623881247
n1m_standard_two_fe_routes_n1m=1a9b0d85ce6e916b013453dc0da5535f4c811c29f17e5f62e00435cc221b4605
gov_no_fe_easy_k0_n10m=2235fd0f2f6073992bdcadcf659763d015a5fe987b92aeaf129c75c3dc5dc4b0
gov_one_fe_offset_k10_fw_c1_n10m=315aa911add363c4fe964260dc257c945be045aff69f78ba883d8fd2ded5ef41
gov_two_fe_standard_k10_n10m=f3c70b81e3662cc34e4efaed29164eed0d4ac1e7211b96ef0e561f0abbd1bb13
gov_two_fe_lowmob_long_k50_pw_c2_n10m=eff6a3d11f7a34c6971890d8b428ade629a0329330d8b46aa2a80f1415076d52
gov_disconnected_separated_k10_c3_n10m=aea0e81cb46691a04289ed86da321126f0b47fd764aa3f3171c2f8a426944cf7
gov_three_fe_exposure_k10_fw_c4_n10m=3e23d98f516b305b258e57518d57e5d7a3f91751b8af55ae7b6b5801272367ca
gov_slopes_illcond_k50_pw_c5_n10m=e0b5d736c5872773f53a820d2af40dda93901d1ae8996b344199d7aae50b3f27
gov_collinear_two_fe_k50_n10m=b3e87cac5d5f1b0996d27e6b6aee23b79bfb177a7d8035d486d8374c8cb01122""".splitlines())
ORACLE_GENERATOR_SHA256 = dict(zip(ORACLE_IDS, (
    "1aa80440e4eba356f41acb18c22a1fa9f627160ca9661ff15c5fb2e1cafcc415",
    "1596d0635b7edcf03e716d5d06e61e78f32ae4fe71921c175233fc4472ef8880",
    "266ca65670753e841c017785d924e5658da5d6c7e4b599d1191fc9d29313602b",
    "c96dd926429e0b1792409d469e274dbe7e9a5fc2ca918343a96b739f59cb8ba6",
    "bffa709c286d6a4046e95e709085d71b097ec0e4341f2883420672b91de43c64",
    "50307f78db418a8eb0752390fb5506824c53896fcf3c6b48c545fb141f8eb0f5",
    "7c900396def19bf8ee6b86d5751f8cc9b7890787f4459775302489049872b8bc",
)))
EXECUTABLE_SOURCES_ROOT_SHA256 = (
    "308c1ef826bc6217af91d66fb4a14922b5313db8c6241aebc35f34e6d0376988"
)
SEPARATION_AUDIT_CELL_ID = "dev_n100k_no_fe_k0_t1"


def _status(cell: Mapping[str, Any]) -> list[dict[str, Any]]:
    result = [
        {"name": f"o.c{i}" if cell["finite_design"] == "collinear" and i == 2 else f"c{i}",
         "status_code": 6 if cell["finite_design"] == "collinear" and i == 2 else 1}
        for i in range(1, int(cell["controls"]) + 1)
    ]
    absorbed = cell["design"] != "no_fe"
    result.append({"name": "o._cons" if absorbed else "_cons", "status_code": 7 if absorbed else 1})
    return result


def _absorb(cell: Mapping[str, Any], save: bool) -> str:
    terms = {
        "no_fe": [], "one_fe": ["fe1"], "two_fe_standard": ["fe1", "fe2"],
        "two_fe_lowmob": ["fe1", "fe2"], "two_fe_disconnected": ["fe1", "fe2"],
        "three_fe_standard": ["fe1", "fe2", "fe3"],
        "two_fe_slopes": ["fe1##c.slope1", "fe2##c.slope2"],
    }[cell["design"]]
    if save:
        saved, channel = [], 1
        for term in terms:
            if "##c." in term:
                group, slope = term.split("##c.")
                saved += [f"__q_fe{channel}={group}", f"__q_fe{channel+1}={group}#c.{slope}"]
                channel += 2
            else:
                saved.append(f"__q_fe{channel}={term}"); channel += 1
        terms = saved
    return "" if not terms else f" absorb({' '.join(terms)})"


def _commands(cell: Mapping[str, Any]) -> dict[str, str]:
    controls = " ".join(f"c.c{i}" for i in range(1, int(cell["controls"]) + 1))
    varlist = " ".join(value for value in ("y", controls) if value)
    absorb = _absorb(cell, cell["output"] == "individual_fe")
    clusters = [] if cell["vce"] == "robust" else [f"cluster{i}" for i in range(1, int(cell["vce"].removeprefix("cluster")) + 1)]
    adjustment = " offset(offset)" if cell["outcome"] in {"easy_high_zero_offset", "long_offset"} else (" exposure(exposure)" if cell["outcome"] == "long_exposure" else "")
    hidden_weight = {"none": "", "fweight": " fweight(fw)", "pweight": " pweight(pw)"}[cell["weight"]]
    public_weight = {"none": "", "fweight": " [fw=fw]", "pweight": " [pw=pw]"}[cell["weight"]]
    vce = " vce(robust)" if not clusters else f" vce(cluster {' '.join(clusters)})"
    candidate_output = {"fit": "", "prediction": " mu(__q_mu)", "total_absorbed_effect": " d(__q_d)", "individual_fe": " d(__q_d)"}[cell["output"]]
    comparator_output = " d(__q_d)" if cell["output"] in {"total_absorbed_effect", "individual_fe"} or (cell["output"] == "prediction" and cell["design"] != "no_fe") else ""
    common = f"{absorb}{adjustment} separation(fe simplex ir) memory({cell['memory']}) threads({cell['requested_threads']}) deterministic dropreason(__q_drop){candidate_output}"
    candidate = f"_fereg_estimate_ppml {varlist},{hidden_weight}{vce}{common}".replace(", ", ", ")
    comparator = f"ppmlhdfe {varlist}{public_weight},{absorb}{adjustment}{vce} separation(fe simplex ir){comparator_output}"
    return {"candidate_science": candidate + " allocationaudit", "candidate_timing": candidate,
            "candidate_allocation_audit": candidate + " allocationaudit",
            "comparator_science": comparator, "comparator_timing": comparator}


def canonical_profile(manifest_path: Path) -> dict[str, Any]:
    manifest = performance.load_manifest(manifest_path)
    cells = []
    for cell in performance.expand_cells(manifest):
        dataset_key = cell.get("paired_fit_case") or cell.get("equivalence_group") or cell["id"]
        dataset_id = f"{dataset_key}_{cell['scale']}"
        seed = int(manifest["generator_contract"]["seed_namespace"]) + int.from_bytes(hashlib.sha256(dataset_id.encode("ascii")).digest()[:4], "big") % 100_000_000
        channels = int(cell["design_spec"]["fe_dimensions"]) + int(cell["design_spec"]["slope_channels"]) if cell["output"] == "individual_fe" else 0
        cells.append({"cell_id": cell["cell_id"], "n": int(cell["scale_spec"]["n"]), "dataset_id": dataset_id,
                      "seed": seed, "generator_sha256": MAIN_GENERATOR_SHA256[dataset_id], "design": cell["design"], "controls": int(cell["controls"]),
                      "weight": cell["weight"], "outcome": cell["outcome"], "finite_design": cell["finite_design"], "adjustment": "offset" if cell["outcome"] in {"easy_high_zero_offset", "long_offset"} else ("exposure" if cell["outcome"] == "long_exposure" else "none"),
                      "vce": cell["vce"], "status": _status(cell), "output": cell["output"],
                      "probe_rows": int(manifest["generator_contract"]["probe_rows"]), "saved_channels": channels,
                      "commands": _commands(cell)})
    oracle = []
    indexed = {cell["cell_id"]: cell for cell in performance.expand_cells(manifest)}
    for cell_id in ORACLE_IDS:
        cell = indexed[cell_id]
        seed = 2_026_083_100 + int.from_bytes(hashlib.sha256((cell_id + ":oracle-v1").encode("ascii")).digest()[:4], "big") % 100_000_000
        oracle.append({"cell_id": cell_id, "fixture_id": f"{cell_id}__explicit_v1", "rows": 256, "seed": seed, "generator_sha256": ORACLE_GENERATOR_SHA256[cell_id],
                       "design": cell["design"], "controls": int(cell["controls"]), "weight": cell["weight"], "outcome": cell["outcome"], "finite_design": cell["finite_design"],
                       "adjustment": "offset" if cell["outcome"] in {"easy_high_zero_offset", "long_offset"} else ("exposure" if cell["outcome"] == "long_exposure" else "none"),
                       "vce": cell["vce"], "status": _status(cell), "output": cell["output"],
                       "probe_count": 0 if cell["output"] == "fit" else 128})
    base_command = next(cell for cell in cells if cell["cell_id"] == "gov_collinear_two_fe_k50_t1")["commands"]["candidate_timing"]
    constrained = []
    for fixture_id, limit, modes in (("low_succeeds_fast_fails", 20_000_000_000, ["low", "fast"]), ("no_route_fits", 1_500_000_000, ["auto"])):
        wrapper_sha256 = {}
        for mode in modes:
            start = base_command.index("memory(") + len("memory(")
            end = base_command.index(")", start)
            command = base_command[:start] + mode + base_command[end:]
            wrapper_sha256[mode] = hashlib.sha256(_constrained_attempt_source(command).encode("ascii")).hexdigest()
        constrained.append({"id": fixture_id, "dataset_cell_id": "gov_collinear_two_fe_k50_t1", "limit": limit, "modes": modes, "wrapper_sha256": wrapper_sha256})
    return {"schema_version": 1, "profile_type": "feppml-qualification-recipe-contract-v1", "cells": cells,
            "oracle": oracle,
            "executable_sources_root_sha256": EXECUTABLE_SOURCES_ROOT_SHA256,
            "generator_contract": {
                "fw": "generate long fw = 1 + mod(_n, 3)",
                "pw": "generate double pw = 0.5 + mod(_n, 11) / 5",
                "offset": "generate double offset = 0.02 * (mod(_n, 9) - 4)",
                "exposure": "generate double exposure = exp(offset)",
                "control": "generate double c{i} = mod({2i+1} * _n, 1009) / 504 - 1",
                "main_outcome": "generate double y = floor(__mu) + (mod(_n, 1000) < 1000 * (__mu - floor(__mu)))",
                "oracle_outcome": "generate double y = floor(__mu) + (mod(37 * _n, 100) < 100 * (__mu - floor(__mu)))",
            },
            "constrained": constrained}


def profile_digest(profile: Mapping[str, Any]) -> str:
    return hashlib.sha256(json.dumps(profile, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")).hexdigest()


def validate_documents(profile: Mapping[str, Any], recipes: Mapping[str, Any], fixtures: Mapping[str, Any], constrained: Mapping[str, Any]) -> None:
    expected_cells = profile["cells"]
    records = recipes.get("cells")
    if not isinstance(records, list) or len(records) != 33:
        raise QualificationRecipeContractError("recipe cell count is not canonical")
    for expected, record in zip(expected_cells, records):
        for key in ("cell_id", "n", "dataset_id", "seed"):
            if record.get(key) != expected[key]: raise QualificationRecipeContractError(f"recipe {key} differs")
        if record.get("commands") != expected["commands"] or record.get("expected_status_map") != expected["status"] or record.get("expected_output") != expected["output"] or record.get("probe_rows") != expected["probe_rows"] or record.get("expected_saved_channels") != expected["saved_channels"]:
            raise QualificationRecipeContractError("recipe model/options differ from canonical profile")
    oracle_records = fixtures.get("cells")
    if not isinstance(oracle_records, list) or len(oracle_records) != 7: raise QualificationRecipeContractError("oracle count differs")
    for expected, record in zip(profile["oracle"], oracle_records):
        if (record.get("cell_id"), record.get("fixture_id"), record.get("row_count"), record.get("seed")) != (expected["cell_id"], expected["fixture_id"], expected["rows"], expected["seed"]): raise QualificationRecipeContractError("oracle identity/rows/seed differ")
    for expected, record in zip(profile["constrained"], constrained.get("fixtures", [])):
        if record.get("id") != expected["id"] or record.get("dataset_cell_id") != expected["dataset_cell_id"] or [a.get("id") for a in record.get("attempts", [])] != expected["modes"] or any(a.get("memory_limit_bytes") != expected["limit"] for a in record.get("attempts", [])): raise QualificationRecipeContractError("constrained profile differs")


def validate_executable_sources(
    profile: Mapping[str, Any], recipes: Mapping[str, Any], generated_root: Path
) -> None:
    """Bind every executable science/timing program to frozen full-file bytes."""
    records = recipes.get("cells")
    if not isinstance(records, list) or len(records) != len(profile["cells"]):
        raise QualificationRecipeContractError("executable recipe inventory is not exact")
    inventory: dict[str, str] = {}
    expected_paths: list[str] = []
    for expected, record in zip(profile["cells"], records):
        cell_id = expected["cell_id"]
        if record.get("cell_id") != cell_id:
            raise QualificationRecipeContractError("executable recipe order differs")
        for arm in ("candidate", "comparator"):
            for kind in ("science", "timing"):
                key = f"{arm}_{kind}_do"
                relative = f"cells/{cell_id}/{arm}_{kind}.do"
                expected_paths.append(relative)
                if record.get(key) != relative:
                    raise QualificationRecipeContractError(
                        f"{key} path differs from the frozen executable inventory"
                    )
                path = generated_root / relative
                if not path.is_file() or path.is_symlink():
                    raise QualificationRecipeContractError(
                        f"frozen executable source is missing or not a regular file: {relative}"
                    )
                inventory[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    observed_paths = sorted(
        path.relative_to(generated_root).as_posix()
        for path in generated_root.glob("cells/*/*.do")
        if path.is_file() or path.is_symlink()
    )
    if observed_paths != sorted(expected_paths):
        raise QualificationRecipeContractError(
            "executable source file inventory contains missing or extra programs"
        )
    root_sha256 = hashlib.sha256(
        json.dumps(
            inventory,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
        ).encode("ascii")
    ).hexdigest()
    if root_sha256 != profile.get("executable_sources_root_sha256"):
        raise QualificationRecipeContractError(
            "executable science/timing bytes differ from the frozen canonical identity"
        )
    expected_audit = separation_contract.stata_comparator_audit(
        separation_contract.load_manifest()
    )
    audit_hits: list[str] = []
    for record in records:
        relative = record["comparator_science_do"]
        source = (generated_root / relative).read_text(encoding="ascii")
        if "PHASE7_COMPARATOR_BEGIN|" in source:
            audit_hits.append(record["cell_id"])
            if expected_audit not in source:
                raise QualificationRecipeContractError(
                    "pinned comparator science arm differs from the exact separation audit"
                )
    if audit_hits != [SEPARATION_AUDIT_CELL_ID]:
        raise QualificationRecipeContractError(
            "exact separation comparator audit is missing or executes more than once"
        )


def validate_oracle_models(profile: Mapping[str, Any], fixtures: Mapping[str, Any], generated_root: Path) -> None:
    effects = {
        "no_fe": [], "one_fe": [{"column": "fe1", "slope": None}],
        "two_fe_standard": [{"column": "fe1", "slope": None}, {"column": "fe2", "slope": None}],
        "two_fe_lowmob": [{"column": "fe1", "slope": None}, {"column": "fe2", "slope": None}],
        "two_fe_disconnected": [{"column": "fe1", "slope": None}, {"column": "fe2", "slope": None}],
        "three_fe_standard": [{"column": "fe1", "slope": None}, {"column": "fe2", "slope": None}, {"column": "fe3", "slope": None}],
        "two_fe_slopes": [{"column": "fe1", "slope": "slope1"}, {"column": "fe2", "slope": "slope2"}],
    }
    for expected, record in zip(profile["oracle"], fixtures["cells"]):
        model = json.loads((generated_root / record["model"]).read_text(encoding="ascii"))
        checks = {
            "cell_id": expected["cell_id"], "row_count": 256,
            "fixed_effects": effects[expected["design"]],
            "adjustment": {"kind": expected["adjustment"], "column": None if expected["adjustment"] == "none" else expected["adjustment"]},
            "weight": {"kind": expected["weight"], "column": {"none": None, "fweight": "fw", "pweight": "pw"}[expected["weight"]]},
            "vce": "robust" if expected["vce"] == "robust" else "cluster",
            "expected_status_map": expected["status"], "expected_output": expected["output"],
            "probe_count": expected["probe_count"],
        }
        if any(model.get(key) != value for key, value in checks.items()):
            raise QualificationRecipeContractError("oracle model semantics differ from canonical profile")
        expected_clusters = [] if expected["vce"] == "robust" else [f"cluster{i}" for i in range(1, int(expected["vce"].removeprefix("cluster")) + 1)]
        if model.get("clusters") != expected_clusters:
            raise QualificationRecipeContractError("oracle cluster dimensions differ")


def _require_source_lines(source: str, required: list[str], where: str) -> None:
    missing = [line for line in required if source.splitlines().count(line) != 1]
    if missing:
        raise QualificationRecipeContractError(f"{where} generator formula contract differs: {missing}")


def _close_assignments(source: str, allowed: Mapping[str, set[str]], where: str) -> None:
    lines = source.splitlines()
    for variable, expected in allowed.items():
        observed = {
            line for line in lines
            if line.startswith(f"generate double {variable} ")
            or line.startswith(f"generate long {variable} ")
            or line.startswith(f"replace {variable} ")
            or line.startswith(f"by ") and f": replace {variable} " in line
        }
        if observed != expected:
            raise QualificationRecipeContractError(
                f"{where} protected assignment inventory differs for {variable}"
            )


def validate_generator_sources(
    profile: Mapping[str, Any], recipes: Mapping[str, Any], recipe_root: Path,
    fixtures: Mapping[str, Any], oracle_root: Path,
) -> None:
    """Validate canonical data-generating equations without compiler imports."""
    for expected, record in zip(profile["cells"], recipes["cells"]):
        payload = (recipe_root / record["generator"]).read_bytes()
        if hashlib.sha256(payload).hexdigest() != expected["generator_sha256"]:
            raise QualificationRecipeContractError("main generator bytes differ from frozen canonical identity")
        source = payload.decode("ascii")
        required = [
            f"set seed {expected['seed']}", f"set obs {expected['n']}",
            "generate long obs_id = _n",
            "generate long fw = 1 + mod(_n, 3)",
            "generate double pw = 0.5 + mod(_n, 11) / 5",
            "generate double offset = 0.02 * (mod(_n, 9) - 4)",
            "generate double exposure = exp(offset)",
            "generate double __mu = exp(__eta)",
            "generate double y = floor(__mu) + (mod(_n, 1000) < 1000 * (__mu - floor(__mu)))",
        ]
        required += [
            f"generate double c{i} = mod({2*i+1} * _n, 1009) / 504 - 1"
            for i in range(1, expected["controls"] + 1)
        ]
        _require_source_lines(source, required, f"main {expected['cell_id']}")
        threshold = {"easy_low_zero": 100, "easy_high_zero_offset": 800, "long_exposure": 350, "separated_high_zero": 800, "long_offset": 550}[expected["outcome"]]
        y_lines = {required[8], f"replace y = 0 if mod(97 * _n, 1000) < {threshold}"}
        if expected["outcome"] in {"separated_high_zero", "long_offset"}: y_lines.add("replace y = 0 if fe1 <= 400")
        allowed = {"fw": {required[3]}, "pw": {required[4]}, "offset": {required[5]}, "exposure": {required[6]}, "__mu": {required[7]}, "y": y_lines}
        for i in range(1, expected["controls"] + 1):
            statements = {f"generate double c{i} = mod({2*i+1} * _n, 1009) / 504 - 1"}
            if i == 2 and expected["finite_design"] == "collinear": statements.add("replace c2 = 2 * c1")
            if i == 2 and expected["finite_design"] == "ill_conditioned": statements.add("replace c2 = c1 + 1e-7 * (mod(_n, 7) - 3)")
            allowed[f"c{i}"] = statements
        _close_assignments(source, allowed, f"main {expected['cell_id']}")
    for expected, record in zip(profile["oracle"], fixtures["cells"]):
        payload = (oracle_root / record["generator"]).read_bytes()
        if hashlib.sha256(payload).hexdigest() != expected["generator_sha256"]:
            raise QualificationRecipeContractError("oracle generator bytes differ from frozen canonical identity")
        source = payload.decode("ascii")
        required = [
            f"set seed {expected['seed']}", "set obs 256", "generate long obs_id = _n",
            "generate long fw = 1 + mod(_n, 3)",
            "generate double pw = 0.5 + mod(_n, 11) / 5",
            "generate double offset = 0.02 * (mod(_n, 9) - 4)",
            "generate double exposure = exp(offset)",
            "generate double __mu = exp(__eta)",
            "generate double y = floor(__mu) + (mod(37 * _n, 100) < 100 * (__mu - floor(__mu)))",
        ]
        required += [
            f"generate double c{i} = mod({2*i+1} * _n, 1009) / 504 - 1"
            for i in range(1, expected["controls"] + 1)
        ]
        _require_source_lines(source, required, f"oracle {expected['cell_id']}")
        threshold = {"easy_low_zero": 10, "easy_high_zero_offset": 55, "long_exposure": 30, "separated_high_zero": 55, "long_offset": 40}[expected["outcome"]]
        y_lines = {required[8], f"replace y = 0 if mod(97 * _n, 100) < {threshold}"}
        separated = expected["outcome"] in {"separated_high_zero", "long_offset"}
        if separated: y_lines.add("replace y = 0 if fe1 == 1")
        effect_columns = {"no_fe": (), "one_fe": ("fe1",), "two_fe_standard": ("fe1","fe2"), "two_fe_lowmob": ("fe1","fe2"), "two_fe_disconnected": ("fe1","fe2"), "three_fe_standard": ("fe1","fe2","fe3"), "two_fe_slopes": ("fe1","fe2")}[expected["design"]]
        for effect in effect_columns: y_lines.add(f"by {effect}: replace y = max(y, 1) if _n == _N" + (" & fe1 != 1" if separated else ""))
        allowed = {"fw": {required[3]}, "pw": {required[4]}, "offset": {required[5]}, "exposure": {required[6]}, "__mu": {required[7]}, "y": y_lines}
        for i in range(1, expected["controls"] + 1):
            statements={f"generate double c{i} = mod({2*i+1} * _n, 1009) / 504 - 1"}
            if i == 2 and expected["finite_design"] == "collinear": statements.add("replace c2 = 2 * c1")
            if i == 2 and expected["finite_design"] == "ill_conditioned": statements.add("replace c2 = c1 + 1e-7 * (mod(_n, 7) - 3)")
            allowed[f"c{i}"]=statements
        _close_assignments(source, allowed, f"oracle {expected['cell_id']}")


def _constrained_attempt_source(command: str) -> str:
    return "\n".join([
        "version 19.0", "clear all", "set more off",
        "args data artifact result ready start done requested comparator_limit pair temperature cell arm",
        "use \"`data'\", clear",
        "local artifact_dir = substr(\"`artifact'\", 1, strrpos(\"`artifact'\", \"/\") - 1)",
        "adopath ++ \"`artifact_dir'\"",
        "findfile _fereg_estimate_ppml.ado", "local __q_resolved_ado = r(fn)",
        "assert \"`__q_resolved_ado'\" == \"`artifact_dir'/_fereg_estimate_ppml.ado\"",
        "findfile fereg_parse.ado", "local __q_resolved_parse = r(fn)",
        "assert \"`__q_resolved_parse'\" == \"`artifact_dir'/fereg_parse.ado\"",
        "findfile _fereg_load.ado", "local __q_resolved_load = r(fn)",
        "assert \"`__q_resolved_load'\" == \"`artifact_dir'/_fereg_load.ado\"",
        "local __q_plugin_name = substr(\"`artifact'\", strrpos(\"`artifact'\", \"/\") + 1, .)",
        "findfile `__q_plugin_name'", "local __q_resolved_plugin = r(fn)",
        "assert \"`__q_resolved_plugin'\" == \"`artifact'\"",
        "file open __resolution using \"RESOLUTION\", write text",
        "file write __resolution \"ADO|`__q_resolved_ado'\" _n",
        "file write __resolution \"ARTIFACT|`__q_resolved_plugin'\" _n",
        "file write __resolution \"DEPENDENCY|`__q_resolved_parse'\" _n",
        "file write __resolution \"DEPENDENCY|`__q_resolved_load'\" _n",
        "file close __resolution",
        "file open __ready using \"`ready'\", write text new",
        "file write __ready \"FEPPML_STATA_READY|`cell'|`arm'\" _n", "file close __ready",
        "file open __start using \"`start'\", write text new",
        "file write __start \"FEPPML_STATA_START|`cell'|`arm'\" _n", "file close __start",
        "capture noisily " + command, "local rc = _rc", 'local status "failure"',
        "if `rc' == 0 {", '    local status "success"', "}",
        "file open __result using \"`result'\", write text new",
        "file write __result \"{\\\"attempt\\\":{\\\"id\\\":\\\"`temperature'\\\",\\\"status\\\":\\\"`status'\\\",\\\"rc\\\":`rc'},\\\"stata\\\":{\\\"licensed\\\":true,\\\"version\\\":\\\"`c(stata_version)' `c(flavor)'\\\"},\\\"complete_sentinel\\\":\\\"FEPPML_STATA_CONSTRAINED_COMPLETE\\\"}\" _n",
        "file close __result", "file open __done using \"`done'\", write text new",
        "file write __done \"FEPPML_STATA_DONE|`cell'|`arm'|SUCCESS\" _n", "file close __done",
        'display as result "FEPPML_STATA_CONSTRAINED_COMPLETE"',
    ]) + "\n"


def validate_constrained_sources(profile: Mapping[str, Any], constrained: Mapping[str, Any], generated_root: Path) -> None:
    base = next(cell for cell in profile["cells"] if cell["cell_id"] == "gov_collinear_two_fe_k50_t1")
    base_command = base["commands"]["candidate_timing"]
    for expected, record in zip(profile["constrained"], constrained["fixtures"]):
        for mode, attempt in zip(expected["modes"], record["attempts"]):
            start = base_command.index("memory(") + len("memory(")
            end = base_command.index(")", start)
            command = base_command[:start] + mode + base_command[end:]
            observed = (generated_root / attempt["do_file"]).read_text(encoding="ascii")
            if observed != _constrained_attempt_source(command):
                raise QualificationRecipeContractError("constrained attempt command/wrapper differs")
            if hashlib.sha256(observed.encode("ascii")).hexdigest() != expected["wrapper_sha256"][mode]:
                raise QualificationRecipeContractError("constrained wrapper digest differs")
