from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PROTOCOL = load("feppml_protocol_test", ROOT / "scripts/parse_feppml_stata_protocol.py")
NORMALIZE = load("feppml_protocol_normalize_test", ROOT / "scripts/normalize_feppml_stata_raw.py")
STATUS_MAP = [
    {"name": "x", "status_code": 1},
    {"name": "o.x2", "status_code": 6},
    {"name": "_cons", "status_code": 1},
]
EXPECTATIONS = {"expected_cluster_dimensions": 0, "expected_saved_channels": 0}
PHASES = PROTOCOL.EXPECTED_PHASES


def number(value: int | float) -> str:
    return f"{value:24.17g}"


def text(record: str, key: str, value: str) -> str:
    return f"{record}\t{key}\t{number(len(value))}\t{value}"


def protocol_lines(arm: str = "candidate", *, mask: bytes = b"12") -> list[str]:
    separated = mask.count(b"2")
    retained = mask.count(b"1")
    eligible = retained + separated
    lines = [
        f"HEADER\tFEPPML_STATA_PROTOCOL\t1\t{arm}",
        f"RESULT\trc\t{number(0)}",
        f"RESULT\tconverged\t{number(1)}",
    ]
    coefficients = (("x", 1, 0.2), ("o.x2", 6, 0.0), ("_cons", 1, 0.1))
    for index, (name, status, value) in enumerate(coefficients, 1):
        token = number(status) if arm == "candidate" else "."
        lines.append(
            f"COEF\t{number(index)}\t{number(len(name))}\t{name}\t{token}\t{number(value)}\tB"
        )
    vce = (0.04, 0.0, 0.01, 0.0, 0.0, 0.0, 0.01, 0.0, 0.03)
    cursor = 0
    for row in range(1, 4):
        for column in range(1, 4):
            lines.append(f"VCE\t{number(row)}\t{number(column)}\t{number(vce[cursor])}")
            cursor += 1
    for key, value in (
        ("N_physical", len(mask)), ("N_effective", retained), ("finite_rank", 2),
        ("absorbed_rank", 0), ("df", retained - 2),
    ):
        lines.append(f"DOF\t{key}\t{number(value)}")
    for key, value in (
        ("deviance", 1), ("log_likelihood", -2),
        ("null_log_likelihood", -3), ("pseudo_r2", 1 / 3),
    ):
        lines.append(f"OBJECTIVE\t{key}\t{number(value)}")
    lines.append(
        f"MASK\tASCII_MASK_V1\t{number(len(mask))}\t{number(eligible)}\t"
        f"{number(retained)}\t{number(separated)}\t{number(8192)}\tSIDECAR"
    )
    positions = PROTOCOL._probe_positions(len(mask), min(2, len(mask)))
    for index, position in enumerate(positions, 1):
        code = int(chr(mask[position - 1]))
        row_eligible = int(code in (1, 2))
        row_retained = int(code == 1)
        reason = int(code == 2)
        lines.append(
            f"MASK_PROBE\t{number(index)}\t{number(position)}\t{number(row_eligible)}\t"
            f"{number(row_retained)}\t{number(reason)}"
        )
    lines.append(
        f"DROP_REASON_AUDIT\t{arm}\t{number(separated)}\t"
        f"{number(separated if arm == 'candidate' else 0)}\t{number(0)}"
    )
    lines.extend([
        f"OUTPUT\tprediction\tdirect\t{number(min(retained, 2))}\t{number(1)}",
        f"PROBE\tprediction_probe\t{number(1)}\t{number(1.2)}",
        f"CERT_BOOL\ttwo_consecutive_iterations\t{number(1 if arm == 'candidate' else 0)}",
        f"CERT_BOOL\tstrict_final_solve\t{number(1 if arm == 'candidate' else 0)}",
        f"CERT_BOOL\trank_frozen\t{number(1 if arm == 'candidate' else 0)}",
        f"CERT_NUM\tscore_linf\t{number(1e-10)}",
        f"CERT_NUM\tfe_score_linf\t{number(0)}",
        f"CERT_NUM\tvce_symmetry_linf\t{number(0)}",
        f"CERT_NUM\treconstruction_linf\t{number(1e-12)}",
    ])
    sources = [("matrix", "e(b)"), ("matrix", "e(V)"), ("scalar", "e(N)"), ("variable", "e(sample)")]
    if arm == "candidate":
        sources.append(("variable", "__q_drop"))
    for kind, value in sources:
        lines.append(f"SOURCE\t{kind}\t{number(len(value))}\t{value}\tNATIVE")
    lines.extend([
        f"RECEIPT\tcomplete\t{number(1 if arm == 'candidate' else 0)}",
        f"STATA\tlicensed\t{number(1)}",
        text("STATA", "version", "19.0 MP") + "\tLICENSED",
    ])
    if arm == "candidate":
        for key, value in (
            ("requested_memory", "auto"), ("effective_route", "fast"),
            ("linear_route", "map"),
        ):
            lines.append(text("DIAG_TEXT", key, value))
        for key, value in (
            ("requested_threads", 1), ("effective_threads", 1), ("fallback_mask", 0),
            ("fallback_count", 0), ("outer_iterations", 8), ("inner_iterations", 20),
            ("step_halvings", 0),
        ):
            lines.append(f"DIAG_NUM\t{key}\t{number(value)}")
        lines.extend([
            f"DIAG_BOOL\tworkspace_allocation_free\t{number(1)}",
            f"DIAG_BOOL\twhole_iteration_allocation_free\t{number(1)}",
        ])
        for phase in PHASES:
            lines.append(f"PHASE\t{phase}\t{number(0.01)}")
        memory = {
            "modeled_peak_bytes": 1000, "native_peak_bytes": 1000,
            "process_peak_rss_bytes": 0, "repeated_call_retained_bytes": 0,
            "measured_steady_state_iterations": 16, "allocator_event_count_start": 10,
            "allocator_event_count_end": 10, "allocator_current_bytes_start": 100,
            "allocator_current_bytes_end": 100, "allocator_peak_bytes_start": 1000,
            "allocator_peak_bytes_end": 1000, "workspace_capacity_start": 20,
            "workspace_capacity_end": 20, "scratch_bytes_start": 0,
            "scratch_bytes_end": 0, "scratch_files_after": 0,
            "median_steady_iteration_seconds": 0.001,
        }
        for key, value in memory.items():
            lines.append(f"MEMORY_NUM\t{key}\t{number(value)}")
        lines.extend([
            f"MEMORY_BOOL\tlinear_session_allocation_free\t{number(1)}",
            f"MEMORY_BOOL\tfresh_warm_equivalent\t{number(1)}",
        ])
        for phase in PHASES:
            lines.append(f"MEMORY_MODELED\t{phase}\t{number(100)}")
            lines.append(f"MEMORY_OBSERVED\t{phase}\t{number(100)}")
    lines.extend(["SLOWDOWN\tNONE", "END\tFEPPML_STATA_RAW_COMPLETE"])
    return lines


def write_fixture(root: Path, arm: str = "candidate", *, mask: bytes = b"12") -> Path:
    path = root / f"{arm}.protocol"
    path.write_text("\n".join(protocol_lines(arm, mask=mask)) + "\n", encoding="ascii")
    Path(str(path) + ".mask").write_bytes(mask + b"\n")
    return path


class ParseFeppmlStataProtocolTests(unittest.TestCase):
    def test_candidate_golden_protocol_streams_mask_and_normalizes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = write_fixture(Path(temporary))
            parsed = PROTOCOL.parse_science_protocol(
                path, arm="candidate", expected_status_map=STATUS_MAP,
                expected_output="prediction", expected_probe_rows=2,
                **EXPECTATIONS,
            )
            raw = parsed["raw"]
            expected_sample = hashlib.sha256(
                b"FEPPML_SAMPLE_MASK_V1\0" + struct.pack(">Q", 2) + b"12"
            ).hexdigest()
            expected_separation = hashlib.sha256(
                b"FEPPML_GENERIC_SEPARATION_MASK_V1\0" + struct.pack(">Q", 2) + b"01"
            ).hexdigest()
            self.assertEqual(raw["sample_identity"]["sha256"], expected_sample)
            self.assertEqual(raw["separation_ledger"]["sha256"], expected_separation)
            self.assertEqual(raw["sample_identity"]["probes"], [[1, 1, 1, 0], [2, 1, 0, 1]])
            normalized = NORMALIZE.normalize(raw)
            self.assertEqual(normalized["sample_identity_sha256"], expected_sample)
            self.assertEqual(normalized["coefficient_status"][1], "numerically_rank_deficient")
            self.assertEqual(parsed["diagnostics"]["linear_route"], "map")

    def test_comparator_statuses_only_come_from_frozen_recipe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = write_fixture(Path(temporary), "comparator")
            raw = PROTOCOL.parse_raw_protocol(
                path, arm="comparator", expected_status_map=STATUS_MAP,
                expected_output="prediction", expected_probe_rows=2,
                **EXPECTATIONS,
            )
            self.assertEqual(raw["coefficient_status_codes"], [1, 6, 1])
            lines = protocol_lines("comparator")
            lines[3] = lines[3].replace("\t.\t", f"\t{number(1)}\t")
            path.write_text("\n".join(lines) + "\n", encoding="ascii")
            with self.assertRaisesRegex(PROTOCOL.StataProtocolError, "frozen recipe"):
                PROTOCOL.parse_raw_protocol(
                    path, arm="comparator", expected_status_map=STATUS_MAP,
                    expected_output="prediction", expected_probe_rows=2,
                    **EXPECTATIONS,
                )

    def test_missing_terminal_or_tampered_mask_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = write_fixture(root)
            path.write_text("\n".join(protocol_lines()[:-1]) + "\n", encoding="ascii")
            with self.assertRaisesRegex(PROTOCOL.StataProtocolError, "terminal sentinel"):
                PROTOCOL.parse_science_protocol(
                    path, arm="candidate", expected_status_map=STATUS_MAP,
                    expected_output="prediction", expected_probe_rows=2,
                    **EXPECTATIONS,
                )
            path = write_fixture(root)
            Path(str(path) + ".mask").write_bytes(b"11\n")
            with self.assertRaisesRegex(PROTOCOL.StataProtocolError, "count disagrees"):
                PROTOCOL.parse_science_protocol(
                    path, arm="candidate", expected_status_map=STATUS_MAP,
                    expected_output="prediction", expected_probe_rows=2,
                    **EXPECTATIONS,
                )

    def test_missing_candidate_telemetry_and_unsafe_name_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = write_fixture(root)
            lines = [line for line in protocol_lines() if not line.startswith("MEMORY_NUM\tnative_peak_bytes\t")]
            path.write_text("\n".join(lines) + "\n", encoding="ascii")
            with self.assertRaisesRegex(PROTOCOL.StataProtocolError, "memory inventory"):
                PROTOCOL.parse_science_protocol(
                    path, arm="candidate", expected_status_map=STATUS_MAP,
                    expected_output="prediction", expected_probe_rows=2,
                    **EXPECTATIONS,
                )

    def test_output_probe_count_and_provenance_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = write_fixture(root, mask=b"11")
            with self.assertRaisesRegex(PROTOCOL.StataProtocolError, "probe.*count"):
                PROTOCOL.parse_science_protocol(
                    path, arm="candidate", expected_status_map=STATUS_MAP,
                    expected_output="prediction", expected_probe_rows=2,
                    **EXPECTATIONS,
                )

    def test_individual_saved_channels_are_accepted_with_exact_flat_cardinality(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = write_fixture(root)
            lines = protocol_lines()
            rewritten = []
            for line in lines:
                if line.startswith("OUTPUT\tprediction\t"):
                    rewritten.append(f"OUTPUT\tindividual_fe\tsaved_channels\t{number(2)}\t{number(2)}")
                elif line.startswith("PROBE\tprediction_probe\t"):
                    rewritten.extend([
                        f"PROBE\tindividual_fe_probe\t{number(1)}\t{number(0.4)}",
                        f"PROBE\tindividual_fe_probe\t{number(2)}\t{number(0.8)}",
                    ])
                else:
                    rewritten.append(line)
            insert_at = next(index for index, line in enumerate(rewritten) if line.startswith("RECEIPT\t"))
            rewritten[insert_at:insert_at] = [
                f"SOURCE\tvariable\t{number(7)}\t__q_fe1\tNATIVE",
                f"SOURCE\tvariable\t{number(7)}\t__q_fe2\tNATIVE",
            ]
            path.write_text("\n".join(rewritten) + "\n", encoding="ascii")
            parsed = PROTOCOL.parse_science_protocol(
                path, arm="candidate", expected_status_map=STATUS_MAP,
                expected_output="individual_fe", expected_probe_rows=2,
                expected_cluster_dimensions=0, expected_saved_channels=2,
            )
            self.assertEqual(parsed["raw"]["outputs"]["individual_fe_probe"], [0.4, 0.8])
            path = write_fixture(root)
            lines = protocol_lines()
            lines = [
                line.replace("OUTPUT\tprediction\tdirect", "OUTPUT\tprediction\tsaved_channel_sum")
                for line in lines
            ]
            path.write_text("\n".join(lines) + "\n", encoding="ascii")
            with self.assertRaisesRegex(PROTOCOL.StataProtocolError, "provenance"):
                PROTOCOL.parse_science_protocol(
                    path, arm="candidate", expected_status_map=STATUS_MAP,
                    expected_output="prediction", expected_probe_rows=2,
                    **EXPECTATIONS,
                )
            path = write_fixture(root)
            lines = protocol_lines()
            lines[3] = lines[3].replace("\tx\t", "\tx bad\t")
            path.write_text("\n".join(lines) + "\n", encoding="ascii")
            with self.assertRaisesRegex(PROTOCOL.StataProtocolError, "length or line safety|unsafe"):
                PROTOCOL.parse_science_protocol(
                    path, arm="candidate", expected_status_map=STATUS_MAP,
                    expected_output="prediction", expected_probe_rows=2,
                    **EXPECTATIONS,
                )


if __name__ == "__main__":
    unittest.main()
