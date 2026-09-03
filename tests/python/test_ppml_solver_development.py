from __future__ import annotations

import copy
import csv
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/ppml_solver_development.py"
MANIFEST = ROOT / "benchmarks/manifests/feppml_solver_development_v2.json"
DRIVER = ROOT / "tests/stata/development/feppml_solver_development.do"

SPEC = importlib.util.spec_from_file_location("ppml_solver_development", SCRIPT)
assert SPEC and SPEC.loader
DEVELOPMENT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEVELOPMENT)


def result_row(scope: dict[str, object], arm: str, repetition: int) -> dict[str, object]:
    solver = arm in DEVELOPMENT.SOLVERS
    return {
        **scope,
        "scope_id": scope["id"],
        "implementation": "candidate" if solver else "reference",
        "arm_type": "solver" if solver else "comparator",
        "arm": arm,
        "repetition": repetition,
        "rc": 0,
        "timed": True,
        "seconds": 1.0,
        "sample_n": 100.0,
        "ll": -10.0,
        "deviance": 2.0,
        "converged": 1.0,
        "rank": 1.0,
        "df_a": 18.0,
        "df_resid": 81.0,
        "inner_iterations": 4.0,
        "route_code": float(DEVELOPMENT.ROUTE_CODES[arm]) if solver else None,
        "selected_engine_code": ({"map": 1.0, "lsmr": 3.0, "cmg": 4.0}[arm] if solver else None),
        "effective_engine_code": ({"map": 1.0, "lsmr": 3.0, "cmg": 4.0}[arm] if solver else None),
        "fallback_count": 0.0 if solver else None,
        "effective_threads": float(scope["threads"]) if solver else None,
        "b_names": ["x1"],
        "b_values": [0.25],
        "v_names": ["x1"],
        "v_values": [0.01],
        "certificate_values": [1.0] * 16 if solver else [],
        "runtime_values": [0.0] * 108 if solver else [],
        "reference_acceleration": "conjugate_gradient" if not solver else "",
        "reference_transform": "symmetric_kaczmarz" if not solver else "",
    }


def adjudicator_row(scope: dict[str, object], source: dict[str, object]) -> dict[str, object]:
    return {
        **copy.deepcopy(source), **scope, "scope_id": scope["id"],
        "implementation": "adjudicator", "arm_type": "adjudicator",
        "arm": DEVELOPMENT.ADJUDICATOR, "repetition": 1, "timed": False,
        "seconds": None, "reference_acceleration": "conjugate_gradient",
        "reference_transform": "symmetric_kaczmarz",
    }


def evidence_receipt(name: str) -> dict[str, object]:
    return {"path": f"/tmp/{name}", "size": 10, "sha256": name[0] * 64}


def add_public_state_kkt(document: dict[str, object]) -> dict[str, object]:
    scope = document["scopes"][0]
    candidate = next(
        row for row in document["records"]
        if row["implementation"] == "candidate" and row["arm"] == "map"
    )
    default = next(row for row in document["records"] if row["arm"] == "ppmlhdfe")
    strict = next(row for row in document["records"] if row["arm"] == DEVELOPMENT.ADJUDICATOR)

    def fit(source: dict[str, object], implementation: str, arm: str) -> dict[str, object]:
        candidate_arm = arm == "map"
        return {
            **scope, "scope_id": scope["id"], "implementation": implementation,
            "arm": arm, "sample_n": source["sample_n"], "ll": source["ll"],
            "deviance": source["deviance"], "converged": source["converged"],
            "rank": source["rank"], "df_a": source["df_a"],
            "df_resid": source["df_resid"],
            "route_code": 2.0 if candidate_arm else None,
            "selected_engine_code": 1.0 if candidate_arm else None,
            "effective_engine_code": 1.0 if candidate_arm else None,
            "fallback_count": 0.0 if candidate_arm else None,
            "effective_threads": float(scope["threads"]) if candidate_arm else None,
            "certificate_passed": 1.0 if candidate_arm else None,
            "b_names": source["b_names"], "b_values": source["b_values"],
            "reference_acceleration": "" if candidate_arm else "conjugate_gradient",
            "reference_transform": "" if candidate_arm else "symmetric_kaczmarz",
        }

    states = {
        "map": evidence_receipt("a-state"),
        "ppmlhdfe": evidence_receipt("b-state"),
        DEVELOPMENT.ADJUDICATOR: evidence_receipt("c-state"),
    }
    audit_result = evidence_receipt("d-audit")
    document["public_state_adjudications"] = [{
        **scope, "scope_id": scope["id"], "implementation": "candidate",
        "method": DEVELOPMENT.KKT_ADJUDICATOR,
        "fits": {
            "map": fit(candidate, "candidate", "map"),
            "ppmlhdfe": fit(default, "reference", "ppmlhdfe"),
            DEVELOPMENT.ADJUDICATOR: fit(
                strict, "reference", DEVELOPMENT.ADJUDICATOR,
            ),
        },
        "states": states, "audit_result": audit_result,
        "metrics": {
            "map": {
                "finite_score_absinf": 1e-10, "fe_score_absinf": 1e-11,
                "score_relinf": 1e-12, "score_rel2": 1e-13, "score_scale": 10.0,
                "recomputed_ll": -10.0, "recomputed_deviance": 2.0,
                "min_mu": 0.5, "max_mu": 2.0,
            },
            "ppmlhdfe": {
                "finite_score_absinf": 1e-3, "fe_score_absinf": 1e-4,
                "score_relinf": 2e-7, "score_rel2": 2e-9, "score_scale": 10.0,
                "recomputed_ll": -10.01, "recomputed_deviance": 2.02,
                "min_mu": 0.5, "max_mu": 2.0,
            },
            DEVELOPMENT.ADJUDICATOR: {
                "finite_score_absinf": 1e-4, "fe_score_absinf": 1e-5,
                "score_relinf": 2e-8, "score_rel2": 2e-9, "score_scale": 10.0,
                "recomputed_ll": -10.001, "recomputed_deviance": 2.002,
                "min_mu": 0.5, "max_mu": 2.0,
            },
        },
    }]
    evidence = document["public_state_adjudications"][0]
    dataset_sha = document["datasets"][0]["dta"]["sha256"]
    candidate_source = DEVELOPMENT.canonical_sha256(document["sources"]["candidate"])
    reference_source = DEVELOPMENT.canonical_sha256(document["references"])
    artifact_sha = document["artifacts"]["candidate"]["sha256"]
    common = {
        "scope_id": scope["id"], "dataset_id": "unit-dataset",
        "dataset_sha256": dataset_sha, "accepted_rc": 0,
    }
    map_result = evidence_receipt("e-map-result")
    default_result = evidence_receipt("f-default-result")
    strict_result = evidence_receipt("0-strict-result")
    document["processes"].extend([
        {
            **common, "stage": "public-state-fit", "implementation": "candidate",
            "arm": "map", "source_sha256": candidate_source,
            "artifact_sha256": artifact_sha, "state": states["map"],
            "result": map_result,
            "parsed_result_sha256": DEVELOPMENT.parsed_file_binding(
                map_result, evidence["fits"]["map"],
            ),
        },
        {
            **common, "stage": "public-state-fit", "implementation": "reference",
            "arm": "ppmlhdfe", "source_sha256": reference_source,
            "artifact_sha256": None, "state": states["ppmlhdfe"],
            "result": default_result,
            "parsed_result_sha256": DEVELOPMENT.parsed_file_binding(
                default_result, evidence["fits"]["ppmlhdfe"],
            ),
        },
        {
            **common, "stage": "public-state-fit", "implementation": "reference",
            "arm": DEVELOPMENT.ADJUDICATOR, "source_sha256": reference_source,
            "artifact_sha256": None, "state": states[DEVELOPMENT.ADJUDICATOR],
            "result": strict_result,
            "parsed_result_sha256": DEVELOPMENT.parsed_file_binding(
                strict_result, evidence["fits"][DEVELOPMENT.ADJUDICATOR],
            ),
        },
        {
            **common, "stage": "public-state-audit", "implementation": "candidate",
            "arm": DEVELOPMENT.KKT_ADJUDICATOR, "source_sha256": candidate_source,
            "artifact_sha256": artifact_sha, "inputs": states,
            "result": audit_result,
            "parsed_result_sha256": DEVELOPMENT.parsed_file_binding(
                audit_result, evidence["metrics"],
            ),
        },
    ])
    return document


def run_document() -> dict[str, object]:
    scope = {
        "id": "unit-scope", "n": 100, "topology": "balanced_2fe", "controls": 1,
        "difficulty": "easy", "threads": 7, "seed": 11,
    }
    records = [
        result_row(scope, arm, repetition)
        for repetition in (1, 2)
        for arm in (*DEVELOPMENT.SOLVERS, *DEVELOPMENT.COMPARATORS)
    ]
    metadata = {"N": 100, "levels1": 10, "levels2": 8, "levels3": 0}
    eligibility = DEVELOPMENT.solver_eligibility(
        metadata, str(scope["topology"]), DEVELOPMENT.load_manifest(),
    )
    return {
        "schema_version": 2,
        "kind": "feppml_solver_development_run",
        "qualification_eligible": False,
        "auto_calibration": False,
        "repetitions": 2,
        "include_comparators": True,
        "include_oracle": False,
        "sources": {"candidate": {"commit": "a" * 40}},
        "artifacts": {"candidate": {"sha256": "9" * 64}},
        "references": {"ppmlhdfe": {"commit": "8" * 40}},
        "scopes": [scope],
        "datasets": [{
            "scope": copy.deepcopy(scope), "dataset_id": "unit-dataset",
            "generated_by_scope_id": "unit-scope", "metadata": metadata,
            "dta": {"sha256": "7" * 64, "size": 100},
            "solver_eligibility": eligibility,
        }],
        "records": records,
        "public_state_adjudications": [],
        "processes": [
            {
                "stage": "generate", "scope_id": "unit-scope",
                "dataset_id": "unit-dataset", "accepted_rc": 0,
            },
            *(
                {
                    "stage": "run", "scope_id": "unit-scope",
                    "implementation": "candidate" if arm in DEVELOPMENT.SOLVERS else "reference",
                    "arm": arm, "accepted_rc": 0,
                }
                for arm in (*DEVELOPMENT.SOLVERS, *DEVELOPMENT.COMPARATORS)
            ),
        ],
    }


class PpmlSolverDevelopmentTests(unittest.TestCase):
    def test_manifest_has_only_public_explicit_arms_and_no_auto_policy(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        self.assertFalse(manifest["qualification_eligible"])
        self.assertFalse(manifest["auto_calibration"])
        self.assertEqual([row["id"] for row in manifest["solvers"]], ["map", "lsmr", "cmg"])
        self.assertEqual([row["id"] for row in manifest["comparators"]], ["ppmlhdfe"])
        adjudication = manifest["scientific_adjudication"]
        self.assertFalse(adjudication["strict_reference"]["timed"])
        self.assertEqual(adjudication["strict_reference"]["tolerance"], 1e-10)
        self.assertEqual(adjudication["strict_reference"]["itolerance"], 1e-12)
        self.assertEqual(
            adjudication["public_state_kkt"]["id"],
            "independent_public_state_kkt_v1",
        )
        self.assertEqual(adjudication["public_state_kkt"]["score_tolerance"], 1e-8)
        serialized = json.dumps(manifest).lower()
        self.assertNotIn("recommendation", serialized)
        self.assertNotIn("candidate_routes", serialized)
        self.assertNotIn("trace_schema", serialized)
        self.assertNotIn("timed_trace", serialized)
        self.assertEqual(manifest["protocols"]["development_result_table"], "2.4")

    def test_solver_eligibility_uses_the_prepared_cmg_core_order_and_cap(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        two_fe = DEVELOPMENT.solver_eligibility(
            {"N": 100, "levels1": 80, "levels2": 64, "levels3": 0},
            "balanced_2fe", manifest,
        )
        self.assertEqual(two_fe["eligible_solvers"], ["map", "lsmr", "cmg"])
        self.assertEqual(two_fe["cmg_nuisance_columns"], 0)

        at_cap = DEVELOPMENT.solver_eligibility(
            {"N": 100, "levels1": 65, "levels2": 100, "levels3": 90},
            "balanced_3fe", manifest,
        )
        self.assertEqual(at_cap["level_counts_descending"], [100, 90, 65])
        self.assertEqual(at_cap["cmg_nuisance_columns"], 64)
        self.assertIn("cmg", at_cap["eligible_solvers"])

        above_cap = DEVELOPMENT.solver_eligibility(
            {"N": 100, "levels1": 66, "levels2": 100, "levels3": 90},
            "balanced_3fe", manifest,
        )
        self.assertEqual(above_cap["eligible_solvers"], ["map", "lsmr"])
        self.assertEqual(
            above_cap["excluded_solvers"],
            {"cmg": "three_fe_nuisance_columns_exceed_64"},
        )
        configurations = DEVELOPMENT._configurations(
            {"candidate": ROOT}, above_cap["eligible_solvers"], True, 0,
        )
        self.assertNotIn(("candidate", "cmg"), configurations)
        self.assertIn(("reference", "ppmlhdfe"), configurations)

    def test_solver_eligibility_rejects_impossible_or_mislabeled_metadata(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        invalid = [
            ({"N": 100, "levels1": 10.5, "levels2": 8, "levels3": 0}, "balanced_2fe"),
            ({"N": 100, "levels1": True, "levels2": 8, "levels3": 0}, "balanced_2fe"),
            ({"N": 100, "levels1": 101, "levels2": 8, "levels3": 0}, "balanced_2fe"),
            ({"N": 100, "levels1": 10, "levels2": 0, "levels3": 8}, "balanced_2fe"),
            ({"N": 100, "levels1": 10, "levels2": 8, "levels3": 0}, "balanced_3fe"),
        ]
        for metadata, topology in invalid:
            with self.subTest(metadata=metadata, topology=topology):
                with self.assertRaises(DEVELOPMENT.DevelopmentError):
                    DEVELOPMENT.solver_eligibility(metadata, topology, manifest)

    def test_analysis_excludes_ineligible_cmg_without_masking_unexpected_rows(self) -> None:
        document = run_document()
        scope = document["scopes"][0]
        scope["topology"] = "balanced_3fe"
        document["datasets"][0]["scope"] = copy.deepcopy(scope)
        metadata = {"N": 100, "levels1": 100, "levels2": 90, "levels3": 66}
        eligibility = DEVELOPMENT.solver_eligibility(
            metadata, "balanced_3fe", DEVELOPMENT.load_manifest(),
        )
        document["datasets"][0]["metadata"] = metadata
        document["datasets"][0]["solver_eligibility"] = eligibility
        document["records"] = [row for row in document["records"] if row["arm"] != "cmg"]
        document["processes"] = [
            process for process in document["processes"] if process.get("arm") != "cmg"
        ]
        for row in document["records"]:
            row["topology"] = "balanced_3fe"
        analysis = DEVELOPMENT.analyze_document(document)
        self.assertTrue(analysis["science_pass"])
        self.assertEqual(
            analysis["solver_exclusions"],
            [{
                "scope_id": "unit-scope", "solver": "cmg",
                "reason": "three_fe_nuisance_columns_exceed_64",
                "effect_count": 3, "cmg_nuisance_columns": 65,
            }],
        )
        self.assertEqual(
            [row["solver"] for row in analysis["solver_summary"]], ["map", "lsmr"],
        )

        unexpected = copy.deepcopy(document)
        unexpected["records"].append(result_row(scope, "cmg", 1))
        rejected = DEVELOPMENT.analyze_document(unexpected)
        self.assertFalse(rejected["science_pass"])
        self.assertTrue(any("ineligible_solver" in item for item in rejected["science_failures"]))

        stale = copy.deepcopy(document)
        stale["datasets"][0]["solver_eligibility"]["eligible_solvers"].append("cmg")
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.analyze_document(stale)

    def test_analysis_rejects_unexpected_result_oracle_process_and_sample(self) -> None:
        result = run_document()
        result["records"].append({
            **copy.deepcopy(result["records"][0]),
            "implementation": "reference", "arm": "map", "arm_type": "solver",
        })
        self.assertFalse(DEVELOPMENT.analyze_document(result)["science_pass"])

        scope_mismatch = run_document()
        scope_mismatch["records"][0]["topology"] = "sparse_3fe"
        self.assertFalse(DEVELOPMENT.analyze_document(scope_mismatch)["science_pass"])

        oracle = run_document()
        oracle["records"].append({
            "scope_id": "unit-scope", "implementation": "independent",
            "arm_type": "oracle", "arm": "bogus", "sample_n": 100.0,
        })
        self.assertFalse(DEVELOPMENT.analyze_document(oracle)["science_pass"])

        process = run_document()
        process["processes"].append({
            "stage": "run", "scope_id": "unit-scope",
            "implementation": "candidate", "arm": "bogus", "accepted_rc": 0,
        })
        self.assertFalse(DEVELOPMENT.analyze_document(process)["science_pass"])

        duplicate_process = run_document()
        duplicate_process["processes"].append(copy.deepcopy(duplicate_process["processes"][1]))
        self.assertFalse(DEVELOPMENT.analyze_document(duplicate_process)["science_pass"])

        missing_process = run_document()
        missing_process["processes"] = []
        self.assertFalse(DEVELOPMENT.analyze_document(missing_process)["science_pass"])

        sample = run_document()
        sample["records"][0]["sample_n"] = 99.0
        failed = DEVELOPMENT.analyze_document(sample)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("sample_differs_from_metadata" in item for item in failed["science_failures"]))

    def test_local_profiles_are_bounded_and_campaign_is_16_bundled_tasks(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        self.assertEqual(len(DEVELOPMENT.profile_scopes(manifest, "local_science")), 8)
        self.assertEqual(len(DEVELOPMENT.profile_scopes(manifest, "local_smoke")), 3)
        tasks = DEVELOPMENT.campaign_tasks(manifest)
        self.assertEqual(len(tasks), 16)
        self.assertEqual(sum(len(task["scopes"]) for task in tasks), 56)
        self.assertTrue(all(
            scope["n"] == 10_000
            for task in tasks for scope in task["scopes"]
            if scope["difficulty"] == "long"
        ))
        self.assertEqual(
            {scope["n"] for task in tasks for scope in task["scopes"] if scope["difficulty"] == "easy"},
            {50_000, 100_000},
        )
        self.assertEqual([len(task["scopes"]) for task in tasks[:8]], [3] * 8)
        self.assertEqual([len(task["scopes"]) for task in tasks[8:]], [4] * 8)
        self.assertTrue(all(2 <= len(task["scopes"]) <= 4 for task in tasks))
        for task in tasks[12:]:
            self.assertEqual(len({scope["seed"] for scope in task["scopes"]}), 1)
            self.assertEqual(len({DEVELOPMENT.dataset_id(scope) for scope in task["scopes"]}), 1)
        self.assertEqual(
            len({scope["seed"] for task in tasks[12:] for scope in task["scopes"]}),
            4,
        )
        self.assertEqual(
            manifest["scc_campaign"]["scheduler"],
            {
                "project": "welfgr", "parallel_environment": "omp", "slots": 28,
                "m_core": 28, "cpu_arch": "broadwell", "cpu_type": "E5-2680v4",
                "mem_per_core": "4G", "h_rt": "02:00:00",
            },
        )

    def test_manifest_rejects_any_auto_calibration_role(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        manifest["auto_calibration"] = True
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.validate_manifest(manifest)

        manifest = DEVELOPMENT.load_manifest()
        manifest["generator"]["same_data_for_thread_scaling_within_bundle"] = False
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.validate_manifest(manifest)

    def test_qhost_class_is_exact(self) -> None:
        source = "HOSTNAME ARCH NCPU\nhl:m_core=28.000000\nhl:cpu_arch=broadwell\nhl:cpu_type=E5-2680v4\n"
        self.assertEqual(
            DEVELOPMENT.parse_qhost_class(source),
            {"m_core": 28, "cpu_arch": "broadwell", "cpu_type": "E5-2680v4"},
        )
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.parse_qhost_class(source.replace("m_core=28.000000", "m_core=32.000000"))
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.parse_qhost_class(source.replace("m_core=28.000000", "m_core=28.5"))

    def test_analysis_checks_every_repetition_and_emits_no_auto_recommendation(self) -> None:
        document = run_document()
        analysis = DEVELOPMENT.analyze_document(document)
        self.assertTrue(analysis["science_pass"])
        self.assertTrue(analysis["route_pass"])
        self.assertNotIn("ppmlhdfe_lsmr_median_seconds", analysis["solver_summary"][0])
        lsmr_summary = next(row for row in analysis["solver_summary"] if row["solver"] == "lsmr")
        self.assertEqual(lsmr_summary["selected_engine_codes"], [3])
        self.assertEqual(lsmr_summary["effective_engine_codes"], [3])
        self.assertNotIn("comparator_diagnostics", analysis)
        self.assertEqual(analysis["reference_adjudications"], [])
        self.assertNotIn("route_recommendation", analysis)
        self.assertNotIn("recommendation", json.dumps(analysis).lower())

        changed = copy.deepcopy(document)
        target = next(row for row in changed["records"] if row["arm"] == "lsmr" and row["repetition"] == 2)
        target["b_values"] = [0.5]
        failed = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("lsmr:r2:map_coefficients" in item for item in failed["science_failures"]))

    def test_strict_reference_adjudicates_only_a_default_stopping_mismatch(self) -> None:
        changed = run_document()
        default = next(
            row for row in changed["records"]
            if row["arm"] == "ppmlhdfe" and row["repetition"] == 2
        )
        default["b_values"] = [0.2501]
        without_strict = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(without_strict["science_pass"])
        self.assertTrue(any("ppmlhdfe_coefficients" in item for item in without_strict["science_failures"]))

        scope = changed["scopes"][0]
        matching_map = next(
            row for row in changed["records"]
            if row["implementation"] == "candidate" and row["arm"] == "map"
            and row["repetition"] == 1
        )
        changed["records"].append(adjudicator_row(scope, matching_map))
        changed["processes"].append({
            "stage": "adjudicate", "scope_id": scope["id"],
            "implementation": "adjudicator", "arm": DEVELOPMENT.ADJUDICATOR,
            "accepted_rc": 0,
        })
        adjudicated = DEVELOPMENT.analyze_document(changed)
        self.assertTrue(adjudicated["science_pass"])
        self.assertEqual(len(adjudicated["reference_adjudications"]), 3)
        self.assertTrue(all(
            row["default_science_mismatches"] == ["ppmlhdfe_coefficients"]
            for row in adjudicated["reference_adjudications"]
        ))
        self.assertTrue(all(row["timed"] is False for row in adjudicated["reference_adjudications"]))

        changed["records"][-1]["b_values"] = [0.5]
        rejected = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(rejected["science_pass"])
        self.assertTrue(any("strict_ppmlhdfe_coefficients" in item for item in rejected["science_failures"]))

    def test_strict_reference_is_rejected_when_default_reference_agrees(self) -> None:
        document = run_document()
        scope = document["scopes"][0]
        matching_map = next(
            row for row in document["records"]
            if row["implementation"] == "candidate" and row["arm"] == "map"
        )
        document["records"].append(adjudicator_row(scope, matching_map))
        analysis = DEVELOPMENT.analyze_document(document)
        self.assertFalse(analysis["science_pass"])
        self.assertTrue(any("not_required" in item for item in analysis["science_failures"]))

    def test_public_state_kkt_adjudicates_only_absolute_coefficient_conflicts(self) -> None:
        document = run_document()
        for row in document["records"]:
            if row["arm"] == "ppmlhdfe":
                row["b_values"] = [0.2501]
        scope = document["scopes"][0]
        strict_source = next(
            row for row in document["records"]
            if row["implementation"] == "candidate" and row["arm"] == "map"
        )
        strict = adjudicator_row(scope, strict_source)
        strict["b_values"] = [0.2502]
        document["records"].append(strict)
        document["processes"].append({
            "stage": "adjudicate", "scope_id": scope["id"],
            "implementation": "adjudicator", "arm": DEVELOPMENT.ADJUDICATOR,
            "accepted_rc": 0,
        })
        add_public_state_kkt(document)
        analysis = DEVELOPMENT.analyze_document(document)
        self.assertTrue(analysis["science_pass"])
        kkt_rows = [
            row for row in analysis["reference_adjudications"]
            if row["adjudicator"] == DEVELOPMENT.KKT_ADJUDICATOR
        ]
        self.assertEqual(len(kkt_rows), 6)
        self.assertTrue(all(
            row["default_science_mismatches"] == ["ppmlhdfe_coefficients"]
            and row["strict_science_mismatches"] == ["strict_ppmlhdfe_coefficients"]
            for row in kkt_rows
        ))

        candidate_failure = copy.deepcopy(document)
        candidate_failure["public_state_adjudications"][0]["metrics"]["map"]["score_relinf"] = 2e-8
        failed = DEVELOPMENT.analyze_document(candidate_failure)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any(
            "candidate_score_certificate" in issue
            for issue in failed["science_failures"]
        ))

        reference_pass = copy.deepcopy(document)
        reference_pass["public_state_adjudications"][0]["metrics"]["ppmlhdfe_strict"]["score_relinf"] = 1e-9
        failed = DEVELOPMENT.analyze_document(reference_pass)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any(
            "score_certificate_passed" in issue
            for issue in failed["science_failures"]
        ))

        noncoefficient = copy.deepcopy(document)
        strict_row = next(
            row for row in noncoefficient["records"]
            if row["arm"] == DEVELOPMENT.ADJUDICATOR
        )
        strict_row["v_values"] = [0.02]
        failed = DEVELOPMENT.analyze_document(noncoefficient)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("strict_ppmlhdfe_vce" in issue for issue in failed["science_failures"]))

    def test_public_state_kkt_evidence_and_processes_are_exactly_bound(self) -> None:
        document = run_document()
        for row in document["records"]:
            if row["arm"] == "ppmlhdfe":
                row["b_values"] = [0.2501]
        scope = document["scopes"][0]
        strict_source = next(
            row for row in document["records"]
            if row["implementation"] == "candidate" and row["arm"] == "map"
        )
        strict = adjudicator_row(scope, strict_source)
        strict["b_values"] = [0.2502]
        document["records"].append(strict)
        document["processes"].append({
            "stage": "adjudicate", "scope_id": scope["id"],
            "implementation": "adjudicator", "arm": DEVELOPMENT.ADJUDICATOR,
            "accepted_rc": 0,
        })
        add_public_state_kkt(document)

        tampered = copy.deepcopy(document)
        audit = next(
            process for process in tampered["processes"]
            if process["stage"] == "public-state-audit"
        )
        audit["inputs"] = copy.deepcopy(audit["inputs"])
        audit["inputs"]["map"]["sha256"] = "f" * 64
        failed = DEVELOPMENT.analyze_document(tampered)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("process_binding" in issue for issue in failed["science_failures"]))

        missing = copy.deepcopy(document)
        missing["processes"] = [
            process for process in missing["processes"]
            if process["stage"] != "public-state-audit"
        ]
        failed = DEVELOPMENT.analyze_document(missing)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any(
            "missing_public_state_audit_process" in issue
            for issue in failed["science_failures"]
        ))

        metrics_tamper = copy.deepcopy(document)
        metrics_tamper["public_state_adjudications"][0]["metrics"]["map"][
            "score_relinf"
        ] = 9e-9
        failed = DEVELOPMENT.analyze_document(metrics_tamper)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("process_binding" in issue for issue in failed["science_failures"]))

        fit_result_tamper = copy.deepcopy(document)
        map_fit = next(
            process for process in fit_result_tamper["processes"]
            if process["stage"] == "public-state-fit" and process["arm"] == "map"
        )
        map_fit["result"]["sha256"] = "1" * 64
        failed = DEVELOPMENT.analyze_document(fit_result_tamper)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("process_binding" in issue for issue in failed["science_failures"]))

        dataset_tamper = copy.deepcopy(document)
        audit = next(
            process for process in dataset_tamper["processes"]
            if process["stage"] == "public-state-audit"
        )
        audit["dataset_id"] = "unrelated-dataset"
        failed = DEVELOPMENT.analyze_document(dataset_tamper)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("unexpected_kkt_process" in issue for issue in failed["science_failures"]))

        digest_tamper = copy.deepcopy(document)
        digest_tamper["datasets"][0]["dta"]["sha256"] = ""
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.analyze_document(digest_tamper)

    def test_analysis_rejects_fallback_and_missing_comparator(self) -> None:
        changed = run_document()
        target = next(row for row in changed["records"] if row["arm"] == "cmg" and row["repetition"] == 2)
        target["fallback_count"] = 1.0
        target["effective_engine_code"] = 1.0
        changed["records"] = [
            row for row in changed["records"]
            if not (row["arm"] == "ppmlhdfe" and row["repetition"] == 2)
        ]
        analysis = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(analysis["science_pass"])
        self.assertFalse(analysis["route_pass"])
        self.assertTrue(any("ppmlhdfe:r2:missing" in item for item in analysis["science_failures"]))
        self.assertTrue(any("cmg:r2:fallback" in item for item in analysis["route_failures"]))
        self.assertTrue(any("cmg:r2:effective_engine" in item for item in analysis["route_failures"]))
        cmg = next(row for row in analysis["solver_summary"] if row["solver"] == "cmg")
        self.assertFalse(cmg["performance_eligible"])

    def test_analysis_preserves_candidate_dof_and_accepts_reference_partition(self) -> None:
        document = run_document()
        for row in document["records"]:
            if row["implementation"] == "reference":
                row["rank"] = 0.0
                row["df_a"] = 19.0
                row["df_resid"] = None
        analysis = DEVELOPMENT.analyze_document(document)
        self.assertTrue(analysis["science_pass"])
        self.assertEqual(len(analysis["dof_convention_notes"]), 2)

        changed = copy.deepcopy(document)
        lsmr = next(
            row for row in changed["records"]
            if row["arm"] == "lsmr" and row["repetition"] == 2
        )
        lsmr["rank"] = 0.0
        lsmr["df_a"] = 19.0
        failed = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("lsmr:r2:map_rank" in item for item in failed["science_failures"]))

        changed = copy.deepcopy(document)
        reference = next(
            row for row in changed["records"]
            if row["arm"] == "ppmlhdfe" and row["repetition"] == 2
        )
        reference["df_a"] = 18.0
        failed = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("ppmlhdfe_model_dof" in item for item in failed["science_failures"]))

    def test_control_covariance_uses_the_full_matrix_stride(self) -> None:
        row = {
            "v_names": ["x1", "x2", "_cons"],
            "v_values": [1.0, 2.0, 3.0, 2.0, 4.0, 5.0, 3.0, 5.0, 6.0],
        }
        self.assertEqual(DEVELOPMENT._control_covariance(row, 2), [1.0, 2.0, 2.0, 4.0])
        self.assertEqual(DEVELOPMENT._optional_vector("1 . .a 4"), [1.0, None, None, 4.0])
        self.assertTrue(DEVELOPMENT._close(1.0, 1.00000010005, 1e-7, 1e-10))

    def test_result_parser_keeps_strict_adjudication_untimed_and_route_verified(self) -> None:
        fields = [
            "arm", "repetition", "rc", "timed", "seconds", "N", "ll",
            "deviance", "converged", "rank", "df_a", "df_resid",
            "inner_iterations", "route_code", "selected_engine_code",
            "effective_engine_code", "fallback_count", "effective_threads",
            "b_names", "b_values", "v_names", "v_values",
            "certificate_values", "runtime_values", "reference_acceleration",
            "reference_transform",
        ]
        row = {field: "" for field in fields}
        row.update({
            "arm": DEVELOPMENT.ADJUDICATOR, "repetition": "1", "rc": "0",
            "timed": "0", "seconds": ".", "N": "100", "ll": "-10",
            "deviance": "2", "converged": "1", "rank": "1", "df_a": "18",
            "df_resid": "81", "inner_iterations": "4", "b_names": "x1",
            "b_values": ".25", "v_names": "x1", "v_values": ".01",
            "reference_acceleration": "conjugate_gradient",
            "reference_transform": "symmetric_kaczmarz",
        })
        scope = {
            "id": "parser-scope", "n": 100, "topology": "balanced_2fe",
            "controls": 1, "difficulty": "easy", "threads": 1, "seed": 11,
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "result.tsv"
            with path.open("w", encoding="utf-8", newline="") as target:
                writer = csv.DictWriter(target, fieldnames=fields, delimiter="\t")
                writer.writeheader()
                writer.writerow(row)
            parsed = DEVELOPMENT.parse_table(path, scope, "adjudicator")
            self.assertFalse(parsed[0]["timed"])
            self.assertIsNone(parsed[0]["seconds"])
            row["reference_transform"] = "kaczmarz"
            with path.open("w", encoding="utf-8", newline="") as target:
                writer = csv.DictWriter(target, fieldnames=fields, delimiter="\t")
                writer.writeheader()
                writer.writerow(row)
            with self.assertRaises(DEVELOPMENT.DevelopmentError):
                DEVELOPMENT.parse_table(path, scope, "adjudicator")

    def test_exclusive_json_refuses_to_overwrite_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            DEVELOPMENT.exclusive_json(path, {"qualification_eligible": False})
            with self.assertRaises(FileExistsError):
                DEVELOPMENT.exclusive_json(path, {"qualification_eligible": False})
            self.assertFalse(json.loads(path.read_text())["qualification_eligible"])

    def test_stata_status_requires_terminal_sentinel_and_ignores_echoed_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            console = Path(temporary) / "arm.log"
            console.write_text("")
            self.assertEqual(DEVELOPMENT.stata_stage_status(console, 0, "PASS"), (999, "missing-success-sentinel"))
            console.write_text('. display as error "FEREG_PPML_SOLVER_RUN_FAIL|arm=map|rc=603"\nPASS\n')
            self.assertEqual(DEVELOPMENT.stata_stage_status(console, 0, "PASS"), (0, "success-sentinel"))
            console.with_suffix(".stata.log").write_text("FEREG_PPML_SOLVER_RUN_FAIL|arm=map|rc=430\n")
            self.assertEqual(DEVELOPMENT.stata_stage_status(console, 0, "PASS"), (430, "failure-sentinel"))

    def test_reference_route_requires_observed_cg_symmetric_kaczmarz(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            console = Path(temporary) / "reference.log"
            console.write_text(
                "Running solver (acceleration=conjugate_gradient, "
                "transform=symmetric_kaczmarz tol=1.0e-09)\n"
            )
            self.assertEqual(
                DEVELOPMENT.verified_reference_route(console),
                {
                    "acceleration": "conjugate_gradient",
                    "transform": "symmetric_kaczmarz",
                },
            )
            console.write_text(
                "Running solver (acceleration=lsmr, transform=symmetric_kaczmarz tol=1.0e-09)\n"
            )
            with self.assertRaises(DEVELOPMENT.DevelopmentError):
                DEVELOPMENT.verified_reference_route(console)

    def test_driver_uses_public_solver_surface_and_default_comparator(self) -> None:
        source = DRIVER.read_text(encoding="utf-8")
        kkt_source = DEVELOPMENT.KKT_DRIVER.read_text(encoding="utf-8")
        self.assertIn("feppml y `control_list'", source)
        self.assertIn("solver(`arm')", source)
        self.assertNotIn("_fereg_estimate_ppml", source)
        self.assertNotIn("fesolver(", source)
        self.assertNotIn("set processors `threads'", source)
        self.assertNotIn("ppmlhdfe_lsmr", source)
        self.assertNotIn("accel(lsmr)", source)
        self.assertNotIn("accel(", source)
        self.assertNotIn("transform(", source)
        self.assertIn("tolerance(1e-10) itolerance(1e-12) verbose(2)", source)
        self.assertIn("local route_command `\"`command' verbose(2)\"'", source)
        self.assertIn("FEREG_PPML_SOLVER_REFERENCE_ROUTE_PASS", source)
        self.assertIn("e(feppml_certificate)", source)
        self.assertIn("e(feppml_solver)[1,4]", source)
        self.assertIn("e(feppml_solver)[1,3]", source)
        self.assertIn("e(feppml_solver)[1,5]", source)
        self.assertIn("e(feppml_solver_fallback_count)", source)
        self.assertNotIn("e(feppml_linear_route)", source)
        self.assertNotIn("e(feppml_fallback_count)", source)
        self.assertIn("data_meta", source)
        self.assertIn("solver(map) mu(`fitted_mu')", kkt_source)
        self.assertIn("tolerance(1e-10)", kkt_source)
        self.assertIn("itolerance(1e-12)", kkt_source)
        self.assertNotIn("accel(lsmr)", kkt_source)
        self.assertNotIn("ppmlhdfe_lsmr", kkt_source)

    def test_scc_scripts_encode_exact_array_and_full_lifecycle(self) -> None:
        array = (ROOT / "benchmarks/scc/feppml_solver_dev_job.sh").read_text()
        for expected in (
            "#$ -pe omp 28", "#$ -l m_core=28", "#$ -l cpu_arch=broadwell",
            "#$ -l cpu_type=E5-2680v4", "#$ -t 1-16", "run-task",
        ):
            self.assertIn(expected, array)
        self.assertLess(array.index("source /etc/bashrc"), array.index("set -euo pipefail"))
        for name, marker in (
            ("feppml_solver_dev_setup_job.sh", "record-setup"),
            ("feppml_solver_dev_collector_job.sh", "merge --run-root"),
            ("feppml_solver_dev_submit.sh", "record-submission"),
            ("feppml_solver_dev_close.sh", "qacct -j"),
        ):
            source = (ROOT / "benchmarks/scc" / name).read_text()
            self.assertIn(marker, source)
            self.assertLess(source.index("source /etc/bashrc"), source.index("set -euo pipefail"))
        submit = (ROOT / "benchmarks/scc/feppml_solver_dev_submit.sh").read_text()
        self.assertIn('--local-science "$local_science" --local-smoke "$local_smoke"', submit)
        setup = (ROOT / "benchmarks/scc/feppml_solver_dev_setup_job.sh").read_text()
        self.assertIn('cd "$candidate"', setup)
        self.assertIn('cd "$baseline"', setup)

    def test_qacct_acceptance_requires_complete_array_and_success(self) -> None:
        required = DEVELOPMENT.load_manifest()["receipt_contract"]["qacct_required"]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "qacct.txt"
            blocks = []
            for task in (1, 2):
                blocks.append("\n".join([
                    "jobnumber 123", f"taskid {task}", "owner johannes", "project welfgr",
                    "hostname node", "slots 28", "failed 0", "exit_status 0",
                    "ru_wallclock 1.0", "cpu 1.0", "maxvmem 1G",
                ]))
            path.write_text("\n==============================================================\n".join(blocks))
            rows = DEVELOPMENT._accept_qacct(path, "123", [1, 2], 28, required)
            self.assertEqual(len(rows), 2)
            path.write_text(path.read_text().replace("exit_status 0", "exit_status 1", 1))
            with self.assertRaises(DEVELOPMENT.DevelopmentError):
                DEVELOPMENT._accept_qacct(path, "123", [1, 2], 28, required)


if __name__ == "__main__":
    unittest.main()
