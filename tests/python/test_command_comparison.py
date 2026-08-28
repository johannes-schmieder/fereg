from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest
import csv
import sys

from scripts.command_comparison import (
    ContractError,
    expand_plan,
    load_manifest,
    merge_isolated_shards,
    render_markdown,
    run_component,
)


class CommandComparisonManifestTests(unittest.TestCase):
    def test_stata_harness_reuses_shared_component_data_outside_timing(self) -> None:
        source = (
            Path(__file__).resolve().parents[2]
            / "benchmarks/stata/command_comparison_stata19.do"
        ).read_text(encoding="utf-8")
        self.assertIn("cold_spec data_path", source)
        self.assertIn("GENERATE_SHARED|", source)
        self.assertIn("LOAD_SHARED|", source)
        self.assertIn("quietly use `\"`data_path'\"', clear", source)

    def test_full_plan_has_all_registered_cells_and_output_capabilities(self) -> None:
        manifest = load_manifest()
        plan = expand_plan(manifest)
        self.assertEqual(len(plan), 13 * 2 * 7)
        self.assertEqual({item["threads"] for item in plan}, {1, 8})
        fit_arms = {item["arm"] for item in plan if item["mode"] == "fit"}
        saved_arms = {item["arm"] for item in plan if item["mode"] == "save_fe"}
        self.assertEqual(fit_arms, {"fereg", "greg", "reghdfe", "areg"})
        self.assertEqual(saved_arms, {"fereg", "greg", "reghdfe"})

    def test_selected_plan_preserves_all_five_100k_designs(self) -> None:
        plan = expand_plan(load_manifest(), scales=["n100k"], threads=[1])
        self.assertEqual(len(plan), 5 * 7)
        self.assertEqual(
            {item["design"]["id"] for item in plan},
            {
                "one_fe",
                "iid_two_fe",
                "akm_highmob",
                "akm_standard",
                "akm_lowmob",
            },
        )

    def test_stata18_scope_keeps_areg_only_for_one_fe(self) -> None:
        plan = expand_plan(
            load_manifest(), scales=["n100k"], threads=[1], areg_scope="one_fe"
        )
        self.assertEqual(len(plan), 5 * 6 + 1)
        areg_cells = [item for item in plan if item["arm"] == "areg"]
        self.assertEqual(len(areg_cells), 1)
        self.assertEqual(areg_cells[0]["design"]["id"], "one_fe")

    def test_invalid_areg_scope_fails_closed(self) -> None:
        with self.assertRaisesRegex(ContractError, "unknown areg scope"):
            expand_plan(load_manifest(), areg_scope="sometimes")

    def test_merge_isolated_shards_reconstructs_five_warm_repetitions(self) -> None:
        fields = ["case_id", "scale", "design", "threads", "mode", "phase", "rep", "sequence", "arm", "rc", "seconds", "n", "b1", "b2", "v11", "v12", "v22", "rss", "r2", "df_a", "requested_solver", "used_solver", "iterations", "pilot_iterations", "pilot_contraction", "fallback", "native_seconds", "precommit_seconds"]
        preflight_fields = ["case_id", "scale", "design", "threads", "n", "df_a", "fereg_solver", "fereg_bmax", "fereg_vmax", "fereg_rssrel", "fereg_resmax", "areg_bmax", "areg_vmax", "areg_rssrel", "areg_resmax", "greg_bmax", "greg_resmax", "fereg_reconstruct_max", "reghdfe_reconstruct_max", "fereg_reghdfe_predmax", "fereg_greg_predmax", "science_fail"]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = []
            for offset, warm_count in ((0, 3), (3, 2)):
                shard = root / f"shard-{offset}"
                shard.mkdir()
                inputs.append(shard)
                environment = {
                    "stata_version": "19",
                    "stata_flavor": "MP",
                    "os": "Unix",
                    "machine_type": "test",
                    "configured_processors": "8",
                    "warm_repetitions": str(warm_count),
                    "threads": "1",
                    "scales": "n100k",
                    "areg_scope": "all",
                    "comparator_root": "",
                    "designs": "one_fe",
                    "rep_offset": str(offset),
                }
                (shard / "environment.txt").write_text("".join(f"{key}={value}\n" for key, value in environment.items()), encoding="utf-8")
                rows = []
                phases = [("cold", 0)] + [("warm", rep) for rep in range(offset + 1, offset + warm_count + 1)]
                for phase, rep in phases:
                    for mode, arms in (("fit", ("fereg", "greg", "reghdfe", "areg")), ("save_fe", ("fereg", "greg", "reghdfe"))):
                        for sequence, arm in enumerate(arms, 1):
                            values = {field: "." for field in fields}
                            values.update({"case_id": "one_fe_n100k", "scale": "n100k", "design": "one_fe", "threads": "1", "mode": mode, "phase": phase, "rep": str(rep), "sequence": str(sequence), "arm": arm, "rc": "0", "seconds": "1", "n": "100000"})
                            rows.append(values)
                with (shard / "command_comparison_runs.csv").open("w", newline="", encoding="utf-8") as destination:
                    writer = csv.DictWriter(destination, fieldnames=fields)
                    writer.writeheader(); writer.writerows(rows)
                preflight = {field: "0" for field in preflight_fields}
                preflight.update({"case_id": "one_fe_n100k", "scale": "n100k", "design": "one_fe", "threads": "1", "n": "100000"})
                with (shard / "command_comparison_preflights.csv").open("w", newline="", encoding="utf-8") as destination:
                    writer = csv.DictWriter(destination, fieldnames=preflight_fields)
                    writer.writeheader(); writer.writerow(preflight)
            output = root / "merged"
            summary = merge_isolated_shards(load_manifest(), inputs, output)
            self.assertEqual(summary, {"scopes": 1, "timing_rows": 42, "preflights": 1, "failures": 0})
            with (output / "command_comparison_runs.csv").open(newline="", encoding="utf-8") as source:
                rows = list(csv.DictReader(source))
            self.assertEqual({int(row["rep"]) for row in rows if row["phase"] == "warm"}, {1, 2, 3, 4, 5})

    def test_merge_accepts_one_areg_call_per_fresh_process(self) -> None:
        fields = ["case_id", "scale", "design", "threads", "mode", "phase", "rep", "sequence", "arm", "rc", "seconds", "n", "b1", "b2", "v11", "v12", "v22", "rss", "r2", "df_a", "requested_solver", "used_solver", "iterations", "pilot_iterations", "pilot_contraction", "fallback", "native_seconds", "precommit_seconds"]
        preflight_fields = ["case_id", "scale", "design", "threads", "n", "df_a", "fereg_solver", "fereg_bmax", "fereg_vmax", "fereg_rssrel", "fereg_resmax", "areg_bmax", "areg_vmax", "areg_rssrel", "areg_resmax", "greg_bmax", "greg_resmax", "fereg_reconstruct_max", "reghdfe_reconstruct_max", "fereg_reghdfe_predmax", "fereg_greg_predmax", "science_fail"]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = []

            def write_shard(name: str, rows: list[dict[str, str]], *, areg_scope: str, preflight: bool = False, offset: int = 0) -> None:
                shard = root / name
                shard.mkdir()
                inputs.append(shard)
                environment = {
                    "stata_version": "19", "stata_flavor": "MP", "os": "Unix",
                    "machine_type": "test", "configured_processors": "8",
                    "warm_repetitions": "1", "threads": "1", "scales": "n10m",
                    "areg_scope": areg_scope, "comparator_root": "",
                    "designs": "iid_two_fe", "rep_offset": str(offset),
                }
                (shard / "environment.txt").write_text("".join(f"{key}={value}\n" for key, value in environment.items()), encoding="utf-8")
                with (shard / "command_comparison_runs.csv").open("w", newline="", encoding="utf-8") as destination:
                    writer = csv.DictWriter(destination, fieldnames=fields)
                    writer.writeheader(); writer.writerows(rows)
                with (shard / "command_comparison_preflights.csv").open("w", newline="", encoding="utf-8") as destination:
                    writer = csv.DictWriter(destination, fieldnames=preflight_fields)
                    writer.writeheader()
                    if preflight:
                        values = {field: "0" for field in preflight_fields}
                        values.update({"case_id": "iid_two_fe_n10m", "scale": "n10m", "design": "iid_two_fe", "threads": "1", "n": "10000000"})
                        writer.writerow(values)

            def record(mode: str, phase: str, rep: int, arm: str) -> dict[str, str]:
                values = {field: "." for field in fields}
                values.update({"case_id": "iid_two_fe_n10m", "scale": "n10m", "design": "iid_two_fe", "threads": "1", "mode": mode, "phase": phase, "rep": str(rep), "sequence": "1", "arm": arm, "rc": "0", "seconds": "1", "n": "10000000"})
                return values

            core_rows = []
            for mode in ("fit", "save_fe"):
                for arm in ("fereg", "greg", "reghdfe"):
                    core_rows.append(record(mode, "cold", 0, arm))
                    core_rows.extend(record(mode, "warm", rep, arm) for rep in range(1, 6))
            write_shard("core", core_rows, areg_scope="off")
            write_shard("preflight", [], areg_scope="all", preflight=True)
            write_shard("areg-cold", [record("fit", "cold", 0, "areg")], areg_scope="all")
            for rep in range(1, 6):
                write_shard(f"areg-{rep}", [record("fit", "warm", rep, "areg")], areg_scope="all", offset=rep - 1)

            output = root / "merged"
            summary = merge_isolated_shards(load_manifest(), inputs, output)
            self.assertEqual(summary, {"scopes": 1, "timing_rows": 42, "preflights": 1, "failures": 0})
            with (output / "command_comparison_runs.csv").open(newline="", encoding="utf-8") as source:
                rows = list(csv.DictReader(source))
            areg = [row for row in rows if row["arm"] == "areg"]
            self.assertEqual(len(areg), 6)
            self.assertEqual({int(row["rep"]) for row in areg if row["phase"] == "warm"}, {1, 2, 3, 4, 5})

            write_shard("areg-failure", [], areg_scope="all")
            failure_shard = root / "areg-failure"
            write_shard("preflight-core", [], areg_scope="off", preflight=True)
            (failure_shard / "failure.json").write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "case_id": "iid_two_fe_n10m",
                        "scale": "n10m",
                        "design": "iid_two_fe",
                        "threads": 1,
                        "arm": "areg",
                        "mode": "fit",
                        "status": "time_limit",
                        "elapsed_seconds": 2400,
                    }
                ),
                encoding="utf-8",
            )
            failure_output = root / "merged-failure"
            failure_summary = merge_isolated_shards(
                load_manifest(),
                [root / "core", root / "preflight-core", failure_shard],
                failure_output,
            )
            self.assertEqual(
                failure_summary,
                {"scopes": 1, "timing_rows": 36, "preflights": 1, "failures": 1},
            )
            environment = (failure_output / "environment.txt").read_text(encoding="utf-8")
            self.assertIn("areg_scope=off\n", environment)
            self.assertIn("areg_unavailable_cases=iid_two_fe_n10m@1\n", environment)

    def test_component_runner_records_timeout(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            failure_path = Path(temporary) / "failure.json"
            rc = run_component(
                [sys.executable, "-c", "import time; time.sleep(10)"],
                failure_path,
                case_id="iid_two_fe_n10m",
                scale="n10m",
                design="iid_two_fe",
                threads=1,
                arm="areg",
                mode="fit",
                timeout_seconds=0.01,
            )
            self.assertEqual(rc, 124)
            failure = json.loads(failure_path.read_text(encoding="utf-8"))
            self.assertEqual(failure["status"], "time_limit")
            self.assertEqual(failure["case_id"], "iid_two_fe_n10m")

    def test_resource_limited_report_quantifies_fereg_upper_bound(self) -> None:
        report = render_markdown(
            {
                "summaries": [],
                "aggregate": [],
                "unavailable": [
                    {
                        "case_id": "iid_two_fe_n10m",
                        "scale": "n10m",
                        "design": "iid_two_fe",
                        "threads": 1,
                        "arm": "areg",
                        "mode": "fit",
                        "status": "time_limit",
                        "elapsed_seconds": 2400,
                        "fereg_warm_median_seconds": 8.4,
                        "fereg_over_areg_upper_bound": 0.0035,
                    }
                ],
            }
        )
        self.assertIn("| >=2400s | 8.4000s | <0.0035 |", report)

    def test_report_surfaces_recovery_science_and_limitations(self) -> None:
        report = render_markdown(
            {
                "summaries": [],
                "aggregate": [],
                "saved_fe_overheads": [
                    {
                        "scale": "n10m",
                        "design": "akm_standard",
                        "threads": 8,
                        "arm": "fereg",
                        "save_over_fit": 1.125,
                        "incremental_seconds": 1.25,
                    }
                ],
                "preflights": [
                    {
                        "fereg_bmax": "2e-12",
                        "fereg_vmax": "3e-13",
                        "fereg_rssrel": "4e-14",
                        "fereg_reconstruct_max": "5e-11",
                        "reghdfe_reconstruct_max": "6e-8",
                        "fereg_reghdfe_predmax": "6e-8",
                    }
                ],
                "unavailable": [],
                "limitations": {"platform": "Windows deferred."},
            }
        )
        self.assertIn("## fereg saved-FE overhead", report)
        self.assertIn("| n10m | akm_standard | 8 | 1.125 | 1.2500s |", report)
        self.assertIn("| fereg reconstruction max | 5.000e-11 |", report)
        self.assertIn("- `platform`: Windows deferred.", report)

    def test_report_omits_areg_when_scope_does_not_include_it(self) -> None:
        report = render_markdown(
            {
                "summaries": [],
                "aggregate": [],
                "unavailable": [],
                "limitations": {
                    "areg_saved_fe": "not part of this run",
                    "greg_science": "diagnostic arm",
                },
            }
        )
        self.assertNotIn("| areg |", report)
        self.assertNotIn("areg_saved_fe", report)
        self.assertIn("greg_science", report)

    def test_manifest_mutation_fails_closed(self) -> None:
        manifest = copy.deepcopy(load_manifest())
        manifest["arms"].pop()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "manifest.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "arms are incomplete"):
                load_manifest(path)

    def test_unknown_selection_fails_closed(self) -> None:
        with self.assertRaisesRegex(ContractError, "unknown selected scale"):
            expand_plan(load_manifest(), scales=["unknown"])
        with self.assertRaisesRegex(ContractError, "unknown selected design"):
            expand_plan(load_manifest(), designs=["unknown"])


if __name__ == "__main__":
    unittest.main()
