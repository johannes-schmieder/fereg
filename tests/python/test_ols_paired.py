from __future__ import annotations

import tempfile
from pathlib import Path
import unittest

from scripts.ols_paired import (
    CANDIDATE_LOG_SENTINEL,
    ContractError,
    _arm_case,
    _plugin_name_for_platform,
    _stage_candidate,
    _write_candidate_wrapper,
    candidate_memory,
    coefficient_status,
    imposed_limit_between,
    load_map_baseline_peak_rss,
    scientific_evidence,
    stata_hex_number,
)

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "ols_paired.py"


def matrix(name: str, values: list[list[float]], names: list[str]) -> tuple[list[dict], dict]:
    entries = []
    for row, row_values in enumerate(values, 1):
        for column, value in enumerate(row_values, 1):
            sign = "+" if value >= 0 else "-"
            absolute = abs(value)
            if absolute == 0:
                encoded = "+0.0000000000000X-3ff"
            else:
                python_hex = absolute.hex()
                mantissa, exponent = python_hex[2:].split("p")
                exponent_value = int(exponent)
                exponent_sign = "+" if exponent_value >= 0 else "-"
                encoded = (
                    f"{sign}{mantissa.upper()}X{exponent_sign}"
                    f"{abs(exponent_value):03x}"
                )
            entries.append(
                {
                    "name": name,
                    "row": row,
                    "column": column,
                    "row_name": names[row - 1] if len(names) == len(values) else "y",
                    "column_name": names[column - 1],
                    "value_hex": encoded,
                }
            )
    return entries, {"rows": len(values), "columns": len(values[0])}


class PairedEvidenceTests(unittest.TestCase):
    def test_candidate_plugin_name_is_native_to_the_host_platform(self) -> None:
        self.assertEqual(_plugin_name_for_platform("darwin"), "fereg_macos.plugin")
        self.assertEqual(_plugin_name_for_platform("linux"), "fereg_linux.plugin")
        self.assertEqual(_plugin_name_for_platform("linux2"), "fereg_linux.plugin")
        self.assertEqual(_plugin_name_for_platform("win32"), "fereg_windows.plugin")
        with self.assertRaisesRegex(ValueError, "unsupported paired-qualification"):
            _plugin_name_for_platform("plan9")

    def test_candidate_staging_uses_the_native_plugin_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            candidate_root = root / "source"
            (candidate_root / "stata").mkdir(parents=True)
            (candidate_root / "stata" / "fereg.ado").write_text(
                "program fereg\nend\n", encoding="utf-8"
            )
            plugin = root / "audited-plugin"
            plugin.write_bytes(b"plugin")
            candidate_ado = _stage_candidate(candidate_root, plugin, root / "stage")
            self.assertEqual(
                sorted(path.name for path in candidate_ado.glob("fereg_*.plugin")),
                [_plugin_name_for_platform()],
            )

    def test_candidate_wrapper_preserves_the_child_return_code_numerically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wrapper = root / "run.do"
            _write_candidate_wrapper(
                wrapper,
                root / "case.do",
                root / "capture.machine",
                root / "candidate",
                "0" * 64,
                root / "sysdirs",
            )
            source = wrapper.read_text(encoding="utf-8")
        self.assertNotIn("if _rc exit _rc", source)
        self.assertIn("capture noisily do ", source)
        self.assertIn("local __frg_case_rc = _rc", source)
        self.assertIn(
            "if `__frg_case_rc' != 0 exit `__frg_case_rc'",
            source,
        )
        self.assertIn(CANDIDATE_LOG_SENTINEL, source)
        self.assertIn(f"capture findfile {_plugin_name_for_platform()}", source)

    def test_comparator_capture_uses_the_standard_fresh_process_runner(self) -> None:
        source = SCRIPT.read_text(encoding="utf-8")
        self.assertNotIn("subprocess_run", source)
        self.assertIn(
            '                    subprocess.run,\n                    "estimator",',
            source,
        )

    def test_stata_hex_decoder_handles_binary_and_hex_exponents(self) -> None:
        self.assertEqual(stata_hex_number("+1.8000000000000X+001"), 3.0)
        self.assertEqual(stata_hex_number("-1.0000000000000X-001"), -0.5)
        self.assertEqual(stata_hex_number("+1.0000000000000X+00a"), 1024.0)
        self.assertEqual(stata_hex_number("+0.0000000000000X-3ff"), 0.0)

    def test_coefficient_status_recognizes_base_and_omitted_components(self) -> None:
        self.assertEqual(coefficient_status("2000b.year"), "base")
        self.assertEqual(coefficient_status("2000b.year#co.x1"), "base")
        self.assertEqual(coefficient_status("2o.group"), "omitted")
        self.assertEqual(coefficient_status("x1"), "kept")

    def test_scientific_converter_uses_exact_b_v_dof_and_fit(self) -> None:
        b_entries, b_dimensions = matrix("b", [[0.25, 0.0]], ["x1", "2o.g"])
        v_entries, v_dimensions = matrix(
            "V", [[0.04, 0.0], [0.0, 0.0]], ["x1", "2o.g"]
        )
        case = {
            "matrices": [*b_entries, *v_entries],
            "matrix_dimensions": {"b": b_dimensions, "V": v_dimensions},
            "scalars": {
                "df_m": "+1.0000000000000X+000",
                "df_r": "+1.9000000000000X+003",
                "df_a": "+1.0000000000000X+001",
                "rss": "+1.4000000000000X+003",
                "tss": "+1.4000000000000X+004",
                "r2": "+1.ccccccccccccdX-001",
                "r2_a": "+1.c000000000000X-001",
                "rmse": "+1.0000000000000X+000",
                "converged": "+1.0000000000000X+000",
            },
        }
        evidence = scientific_evidence(case, "comparator")
        self.assertEqual(evidence["coefficient_names"], ["x1", "2o.g"])
        self.assertEqual(evidence["coefficient_status"], ["kept", "omitted"])
        self.assertEqual(evidence["coefficients"], [0.25, 0.0])
        self.assertEqual(evidence["vce"]["values"], [0.04, 0.0, 0.0, 0.0])

    def test_candidate_memory_binds_observed_model_and_leak(self) -> None:
        runtime = [[0.0, 0.0] for _ in range(30)]
        runtime[9][1] = 100.0
        runtime[10][1] = 100.0
        runtime[12][1] = 900.0
        runtime[24][1] = 1000.0
        entries, dimensions = matrix(
            "fereg_runtime", runtime, [str(index) for index in range(1, 31)]
        )
        case = {
            "matrices": entries,
            "matrix_dimensions": {"fereg_runtime": dimensions},
        }
        self.assertEqual(
            candidate_memory([case], 1200),
            {
                "native_allocation_observed_bytes": 900,
                "native_allocation_model_bytes": 1000,
                "repeated_call_leak_bytes": 0,
                "constrained_memory": None,
                "map_baseline_peak_rss_bytes": 1200,
            },
        )

        runtime[10][1] = 101.0
        entries, dimensions = matrix(
            "fereg_runtime", runtime, [str(index) for index in range(1, 31)]
        )
        case = {
            "matrices": entries,
            "matrix_dimensions": {"fereg_runtime": dimensions},
        }
        self.assertEqual(
            candidate_memory([case], 1200)["repeated_call_leak_bytes"], 1
        )

    def test_frozen_map_receipt_supplies_matched_peak_rss(self) -> None:
        receipt = (
            SCRIPT.parents[1]
            / "build_evidence/local/benchmarks/map-baseline-df53822/bench_dense_two_fe_t1.json"
        )
        self.assertEqual(
            load_map_baseline_peak_rss(receipt, "bench_dense_two_fe_t1"),
            3_600_318_464,
        )

    def test_frozen_map_receipt_uses_manifest_hash_not_checkout_permissions(self) -> None:
        source = SCRIPT.parents[1] / "build_evidence/local/benchmarks/map-baseline-df53822"
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary)
            receipt = target / "bench_dense_two_fe_t1.json"
            receipt.write_bytes((source / receipt.name).read_bytes())
            (target / "manifest.json").write_bytes((source / "manifest.json").read_bytes())
            receipt.chmod(0o644)
            self.assertEqual(
                load_map_baseline_peak_rss(receipt, "bench_dense_two_fe_t1"),
                3_600_318_464,
            )
            receipt.write_bytes(receipt.read_bytes() + b"\n")
            with self.assertRaisesRegex(ContractError, "immutable checksum manifest"):
                load_map_baseline_peak_rss(receipt, "bench_dense_two_fe_t1")

    def test_imposed_limit_admits_low_but_not_fast_after_headroom(self) -> None:
        low = 2_000_000_000
        fast = 6_000_000_000
        available = imposed_limit_between(low, fast)
        usable = available - max(512 * 1024 * 1024, available // 5)
        self.assertGreaterEqual(usable, low)
        self.assertLess(usable, fast)

    def test_warm_pair_runs_two_stabilizing_actions_at_the_bound_thread_count(self) -> None:
        case = {
            "thread_count": 8,
            "setup": ["set seed 1"],
            "arms": {"candidate": "fereg y x", "comparator": "reghdfe y x"},
            "capture": {"scalars": [], "matrices": []},
        }
        selected = _arm_case(case, "candidate", "warm")
        self.assertTrue(selected["_fresh_warmup"])
        self.assertEqual(
            selected["setup"],
            [
                "set seed 1",
                "set processors 8",
                "capture noisily fereg y x",
                "if _rc exit _rc",
                "estimates clear",
            ],
        )


if __name__ == "__main__":
    unittest.main()
