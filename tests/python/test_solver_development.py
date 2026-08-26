from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest

from scripts.solver_development import (
    ContractError,
    _parse_preflight,
    _parse_timing,
    _validate_timed_science,
    arm_command,
    expand_run_plan,
    load_development_manifest,
)


ROOT = Path(__file__).resolve().parents[2]


class SolverDevelopmentManifestTests(unittest.TestCase):
    def test_profiles_expand_to_the_frozen_balanced_cadence(self) -> None:
        development = load_development_manifest()
        expected = {
            "quick": 5 * 2 * 4,
            "matrix": 5 * 2 * 15,
            "milestone": 4 * 2 * 15,
            "freeze": 4 * 2 * 3,
        }
        for profile, count in expected.items():
            with self.subTest(profile=profile):
                plan = expand_run_plan(development, profile)
                self.assertEqual(len(plan), count)
                self.assertEqual({run["threads"] for run in plan}, {1, 8})
                self.assertTrue(all(run["profile"] == profile for run in plan))

    def test_quick_profile_has_the_intended_default_diagnostics(self) -> None:
        plan = expand_run_plan(load_development_manifest(), "quick")
        self.assertEqual(
            {run["arm"]["id"] for run in plan},
            {"reghdfe_default", "fereg_auto", "fereg_cg", "gregress_cg"},
        )
        self.assertTrue(
            all(run["repetitions"] == {"cold": 1, "warm": 3} for run in plan)
        )

    def test_algorithm_commands_are_explicit_and_correctly_mapped(self) -> None:
        commands = {
            run["arm"]["id"]: run["command"]
            for run in expand_run_plan(load_development_manifest(), "matrix")
            if run["case_id"] == "akm_standard_n100k_t8"
        }
        self.assertIn("transform(symmetric_kaczmarz)", commands["reghdfe_default"])
        self.assertIn("acceleration(conjugate_gradient)", commands["reghdfe_default"])
        self.assertIn("acceleration(none)", commands["reghdfe_map"])
        self.assertIn("solver(auto)", commands["fereg_auto"])
        self.assertIn("solver(cg)", commands["fereg_cg"])
        self.assertIn(
            "solver(lsmr) preconditioner(schwarz)", commands["fereg_lsmr_schwarz"]
        )
        for algorithm in ("cg", "map", "it", "squarem"):
            self.assertIn(
                f"algorithm({algorithm})", commands[f"gregress_{algorithm}"]
            )
            self.assertIn("mata(GtoolsRegress)", commands[f"gregress_{algorithm}"])

    def test_arm_renderer_keeps_threading_on_fereg_only(self) -> None:
        self.assertIn(
            "threads(8)",
            arm_command({"estimator": "fereg", "algorithm": "cg"}, "worker firm", 8),
        )
        self.assertNotIn(
            "threads(",
            arm_command(
                {"estimator": "reghdfe", "algorithm": "cg"}, "worker firm", 8
            ),
        )

    def test_missing_arm_design_or_dependency_hash_fails_closed(self) -> None:
        development = load_development_manifest()
        mutations = []
        missing_arm = copy.deepcopy(development)
        missing_arm["arms"].pop()
        mutations.append(missing_arm)
        missing_design = copy.deepcopy(development)
        missing_design["designs"].pop()
        mutations.append(missing_design)
        bad_hash = copy.deepcopy(development)
        bad_hash["diagnostic_dependencies"]["gtools"]["gregress_ado_sha256"] = "bad"
        mutations.append(bad_hash)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "development.json"
            for mutation in mutations:
                path.write_text(json.dumps(mutation), encoding="utf-8")
                with self.assertRaises(ContractError):
                    load_development_manifest(path)

    def test_generator_has_a_deterministic_parameterized_contract(self) -> None:
        source = (ROOT / "benchmarks/generators/generate_akm.do").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "args n workers firms periods mobility assignment seed output", source
        )
        self.assertIn("set rng mt64", source)
        self.assertIn("set seed `seed'", source)
        self.assertIn("END_AKM_GENERATOR|rc=0", source)


class SolverDevelopmentParserTests(unittest.TestCase):
    RULES = {
        "coefficient_rtol": 1e-7,
        "coefficient_atol": 1e-10,
        "vce_rtol": 1e-6,
        "vce_atol": 1e-10,
    }

    @staticmethod
    def preflight(*, bmax: str = "1e-12", samplediff: str = "0") -> str:
        return "\n".join(
            [
                f"PREFLIGHT_A|case=c|bmax={bmax}|bscale=1|vmax=1e-12|vscale=1",
                f"PREFLIGHT_B|case=c|nref=100|ncan=100|samplediff={samplediff}|rssrel=1e-12|r2diff=1e-12",
                "PREFLIGHT_C|case=c|dfaref=10|dfacan=10|singref=0|singcan=0|resmax=1e-10|resscale=1|resnorm=.01",
                "PREFLIGHT_D|case=c|n=100|b1=.2|b2=-.1|rss=10",
                "PREFLIGHT_E|case=c|v11=.01|v12=0|v22=.01|r2=.5|dfa=10",
                "GREG_PREFLIGHT|case=c|n=100|b1=.2|b2=-.1|bmax=1e-10",
            ]
        )

    def test_preflight_accepts_certified_candidate_and_records_gregress(self) -> None:
        result = _parse_preflight(self.preflight(), self.RULES)
        self.assertEqual(result["candidate"]["sample_mismatches"], 0)
        self.assertEqual(result["gregress_diagnostic"]["n"], 100)
        self.assertEqual(result["oracle"]["coefficients"], [0.2, -0.1])

    def test_preflight_rejects_scientific_parity_failures(self) -> None:
        with self.assertRaisesRegex(ContractError, "coefficient parity"):
            _parse_preflight(self.preflight(bmax="1e-3"), self.RULES)
        with self.assertRaisesRegex(ContractError, "sample parity"):
            _parse_preflight(self.preflight(samplediff="1"), self.RULES)

    def test_timing_parser_extracts_median_dispersion_and_diagnostics(self) -> None:
        development = load_development_manifest()
        run = next(
            item
            for item in expand_run_plan(development, "quick")
            if item["arm"]["id"] == "fereg_auto"
        )
        lines = []
        for rep, seconds in enumerate((1.0, 0.9, 1.1, 1.0), start=1):
            kind = "cold" if rep == 1 else "warm"
            lines.extend(
                [
                    f"RUN_A|id={run['id']}|rep={rep}|kind={kind}|rc=0|seconds={seconds}|n=100000|b1=.2|b2=-.1",
                    f"RUN_B|id={run['id']}|rep={rep}|v11=.01|v12=0|v22=.01|rss=10|r2=.5|dfa=100",
                    f"DIAG_A|id={run['id']}|rep={rep}|requested=1|used=4|iters=7|stop=1|absinf=1e-10|relinf=1e-10",
                    f"DIAG_B|id={run['id']}|rep={rep}|rel2=1e-10|neq=1e-10|setup=.01|solve=.1|fallback=0",
                    f"DIAG_C|id={run['id']}|rep={rep}|pilot_iters=4|pilot_contract=.2|native=.2|precommit=.3|threads=8",
                ]
            )
        result = _parse_timing("\n".join(lines), run)
        self.assertAlmostEqual(result["warm_median_seconds"], 1.0)
        self.assertAlmostEqual(result["warm_mad_seconds"], 0.1)
        self.assertEqual(result["repetitions"][-1]["solver"]["pilot_iters"], 4)

    def test_timing_parser_reports_the_original_estimator_return_code(self) -> None:
        run = next(
            item
            for item in expand_run_plan(load_development_manifest(), "matrix")
            if item["arm"]["id"] == "fereg_it"
        )
        output = (
            f"RUN_A|id={run['id']}|rep=1|kind=cold|rc=459|seconds=.2|"
            "n=.|b1=.|b2=."
        )
        with self.assertRaisesRegex(ContractError, r"repetition 1 rc=459"):
            _parse_timing(output, run)

    def test_timed_science_rejects_oracle_mismatch_but_not_gregress(self) -> None:
        preflights = {"c": _parse_preflight(self.preflight(), self.RULES)}
        repetition = {
            "repetition": 1,
            "n": 100,
            "coefficients": [0.2, -0.1],
            "vce": [0.01, 0.0, 0.01],
            "rss": 10.0,
            "r2": 0.5,
            "df_a": 10.0,
        }
        fereg = {
            "id": "c__fereg_auto",
            "case_id": "c",
            "arm": {"estimator": "fereg"},
            "repetitions": [repetition],
        }
        _validate_timed_science([fereg], preflights, self.RULES)
        bad = copy.deepcopy(fereg)
        bad["repetitions"][0]["coefficients"][0] = 0.3
        with self.assertRaisesRegex(ContractError, "timed scientific parity"):
            _validate_timed_science([bad], preflights, self.RULES)
        bad["arm"]["estimator"] = "gregress"
        _validate_timed_science([bad], preflights, self.RULES)


if __name__ == "__main__":
    unittest.main()
