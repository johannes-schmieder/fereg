from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest

from scripts.command_comparison_v2 import (
    ContractError,
    expand_plan,
    expand_scopes,
    load_manifest,
)
from scripts.run_command_comparison_v2 import (
    HARNESS,
    _stata_command,
    dataset_path,
    scope_name,
)


class ExpandedCommandComparisonTests(unittest.TestCase):
    def test_manifest_freezes_116_scopes_and_three_command_output_modes(self) -> None:
        manifest = load_manifest()
        scopes = expand_scopes(manifest)
        plan = expand_plan(manifest)
        self.assertEqual(len(scopes), 116)
        self.assertEqual(len(plan), 116 * 2 * 3)
        self.assertEqual({item["controls"] for item in scopes}, {0, 2, 10, 30})
        self.assertEqual(
            {item["requested_threads"] for item in scopes}, {1, 2, 4, 8, 16}
        )
        self.assertEqual({item["arm"] for item in plan}, {"fereg", "greg", "reghdfe"})

    def test_core_scaling_cells_cover_one_two_three_fes_and_wide_controls(self) -> None:
        scopes = expand_scopes(load_manifest())
        observed = {
            (
                item["scale"]["id"], item["design"]["id"], item["controls"],
                item["requested_threads"],
            )
            for item in scopes
        }
        for scale in ("n1m", "n10m"):
            for design, controls in (
                ("one_fe", (0, 30)),
                ("akm_standard", (2, 30)),
                ("akm_standard_year", (2, 30)),
            ):
                for control_count in controls:
                    for threads in (1, 2, 4, 8, 16):
                        self.assertIn((scale, design, control_count, threads), observed)

    def test_endpoint_coverage_includes_low_mobility_year_and_all_controls(self) -> None:
        scopes = expand_scopes(load_manifest())
        observed = {
            (
                item["scale"]["id"], item["design"]["id"], item["controls"],
                item["requested_threads"],
            )
            for item in scopes
        }
        for scale in ("n1m", "n10m"):
            for design in ("akm_lowmob", "akm_lowmob_year"):
                for controls in (0, 2, 10, 30):
                    for threads in (1, 16):
                        self.assertIn((scale, design, controls, threads), observed)

    def test_stata_harness_caps_stata_but_not_fereg_threads(self) -> None:
        source = (
            Path(__file__).resolve().parents[2]
            / "benchmarks/stata/command_comparison_stata19_v2.do"
        ).read_text(encoding="utf-8")
        self.assertIn("c(processors_lic)", source)
        self.assertIn("c(processors_mach)", source)
        self.assertIn("threads(`requested_threads')", source)
        self.assertIn("generate int year = 2000 + period - 1", source)
        self.assertIn("forvalues j = 1/30", source)
        self.assertIn('local absorb "worker firm year"', source)
        self.assertNotIn("quietly areg", source)

    def test_unknown_selections_and_manifest_mutations_fail_closed(self) -> None:
        manifest = load_manifest()
        with self.assertRaisesRegex(ContractError, "unknown selected V2 scale"):
            expand_scopes(manifest, scales=["n100k"])
        with self.assertRaisesRegex(ContractError, "unknown selected V2 control"):
            expand_scopes(manifest, controls=[1])
        changed = copy.deepcopy(manifest)
        changed["cases"].pop()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "manifest.json"
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(ContractError, "116 cells"):
                load_manifest(path)

    def test_resumable_runner_uses_four_shared_datasets_and_exact_arguments(self) -> None:
        scopes = expand_scopes(load_manifest())
        datasets = {str(dataset_path(Path("/run"), scope)) for scope in scopes}
        self.assertEqual(
            datasets,
            {
                "/run/datasets/n1m-standard.dta",
                "/run/datasets/n1m-lowmob.dta",
                "/run/datasets/n10m-standard.dta",
                "/run/datasets/n10m-lowmob.dta",
            },
        )
        scope = next(
            item
            for item in scopes
            if item["scale"]["id"] == "n10m"
            and item["design"]["id"] == "akm_standard_year"
            and item["controls"] == 30
            and item["requested_threads"] == 16
        )
        self.assertEqual(scope_name(scope), "n10m-akm_standard_year-k30-t16")
        command = _stata_command(
            stata=Path("/stata"),
            repo_root=Path("/repo"),
            output_dir=Path("/output"),
            comparator_root="/comparators",
            requested_threads=16,
            scale="n10m",
            design="akm_standard_year",
            controls=30,
            warm_reps=5,
            preflight="on",
            arm="all",
            mode="both",
            cold="on",
            data_path=Path("/data.dta"),
        )
        self.assertEqual(command[:4], ["/stata", "-b", "do", str(HARNESS)])
        self.assertEqual(
            command[4:],
            [
                "/repo", "/output", "16", "n10m", "5", "/comparators",
                "akm_standard_year", "30", "0", "on", "all", "both",
                "on", "/data.dta",
            ],
        )
        command_without_closure = _stata_command(
            stata=Path("/stata"), repo_root=Path("/repo"),
            output_dir=Path("/output"), comparator_root="",
            requested_threads=1, scale="n1m", design="one_fe", controls=0,
            warm_reps=0, preflight="off", arm="none", mode="none",
            cold="off", data_path=Path("/data.dta"),
        )
        self.assertEqual(command_without_closure[9], "system")


if __name__ == "__main__":
    unittest.main()
