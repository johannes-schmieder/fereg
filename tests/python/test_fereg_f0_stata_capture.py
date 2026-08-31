from __future__ import annotations

import hashlib
import math
from pathlib import Path
import tempfile
import unittest

from scripts import fereg_f0_stata_capture as capture


class FeregF0StataCaptureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        root = Path(self.temporary.name)
        self.package = root / "package"
        self.package.mkdir()
        (self.package / "fereg.pkg").write_text(
            "\n".join(
                [
                    "v 3",
                    "p fereg test package",
                    "f fereg.ado",
                    "f fereg.sthlp",
                    "f fereg_macos.plugin",
                    "f fereg_linux.plugin",
                    "f fereg_windows.plugin",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (self.package / "stata.toc").write_text("v 3\n", encoding="utf-8")
        (self.package / "fereg.ado").write_text("program fereg\nend\n", encoding="utf-8")
        (self.package / "fereg.sthlp").write_text("{smcl}\n", encoding="utf-8")
        self.plugin = self.package / "fereg_macos.plugin"
        self.plugin.write_bytes(b"audited-native-plugin")
        self.dataset = root / "immutable.dta"
        self.dataset.write_bytes(b"pre-generated-dataset")
        self.output = root / "arm-output"
        self.spec = capture.ArmCaptureSpec(
            profile="local_screen",
            arm="baseline",
            case_id="local_n100k_dense_t1",
            manifest_sha256="a" * 64,
            source_commit="b" * 40,
            package_root=self.package,
            plugin_filename="fereg_macos.plugin",
            plugin_sha256=capture.sha256_file(self.plugin),
            dataset_path=self.dataset,
            dataset_sha256=capture.sha256_file(self.dataset),
            expected_observations=100_000,
            command=(
                "fereg y x1 x2, absorb(worker firm) residuals(f0_resid) "
                "threads(1) memory(fast)"
            ),
            requested_threads=1,
            output_dir=self.output,
            saved_outputs=("f0_resid",),
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_renderer_is_isolated_scientific_first_and_one_call_timed(self) -> None:
        rendered = capture.render_arm_do(self.spec)
        self.assertNotIn("@@", rendered)
        self.assertIn("forvalues stabilizer = 1/2", rendered)
        self.assertEqual(rendered.count(self.spec.command), 3)
        self.assertLess(rendered.index("phase_begin|science"), rendered.index("timer on 97"))
        self.assertLess(rendered.index("/READY"), rendered.index("/START"))
        self.assertLess(rendered.index("/START"), rendered.index("/DONE"))
        measured = rendered.split("timer on 97", 1)[1].split("timer off 97", 1)[0]
        self.assertEqual(measured.count(self.spec.command), 1)
        self.assertNotIn(" use ", measured.lower())
        self.assertNotIn("generate", measured.lower())
        for forbidden in ("reghdfe", "areg y", "set obs", "simulate"):
            self.assertNotIn(forbidden, rendered.lower())
        self.assertIn(str((self.package / "fereg.ado").resolve()), rendered)
        self.assertIn(str(self.plugin.resolve()), rendered)
        self.assertIn("__fereg_f0_capture_e", rendered)
        self.assertIn("__fereg_f0_capture_sample", rendered)
        self.assertIn("__fereg_f0_capture_saved", rendered)

    def test_file_contract_and_writer_create_only_arm_scoped_runtime_files(self) -> None:
        contract = capture.file_contract(self.spec)
        self.assertEqual(contract["schema_version"], 1)
        self.assertEqual(set(contract["markers"]), {"ready", "start", "done"})
        self.assertEqual(contract["rss_scope"], "fresh_process_between_ready_start_done")
        driver = capture.write_arm_do(self.spec)
        self.assertEqual(driver, self.output / "run.do")
        self.assertTrue(driver.is_file())
        for name in ("personal", "plus", "oldplace", "site"):
            self.assertTrue((self.output / "sysdirs" / "baseline" / name).is_dir())
        self.assertFalse((self.output / "capture.machine").exists())

    def test_hash_and_complete_package_checks_fail_closed(self) -> None:
        changed = self.dataset.read_bytes() + b"mutation"
        self.dataset.write_bytes(changed)
        with self.assertRaisesRegex(capture.CaptureContractError, "dataset SHA-256"):
            capture.render_arm_do(self.spec)
        self.dataset.write_bytes(b"pre-generated-dataset")
        (self.package / "fereg.sthlp").unlink()
        with self.assertRaisesRegex(capture.CaptureContractError, "member is absent"):
            capture.render_arm_do(self.spec)

    def test_log_sentinel_is_unique_and_arm_bound(self) -> None:
        expected = capture.expected_log_sentinel(self.spec)
        capture.validate_log_sentinel("noise\n" + expected + "\n", self.spec)
        for malformed in ("", expected + "\n" + expected, expected.replace("rc=0", "rc=9")):
            with self.subTest(malformed=malformed):
                with self.assertRaises(capture.CaptureContractError):
                    capture.validate_log_sentinel(malformed, self.spec)

    def test_machine_parser_derives_closed_science_and_exact_hex_digest(self) -> None:
        result = capture.parse_arm_capture(self._machine_capture(), self.spec)
        self.assertEqual(result.scientific["coefficient_names"], ["x1", "x2"])
        self.assertEqual(result.scientific["coefficient_status"], [1, 2])
        self.assertEqual(result.scientific["coefficients"], [1.0, 2.0])
        self.assertEqual(result.scientific["vce_shape"], [2, 2])
        self.assertTrue(result.scientific["certificates"]["converged"])
        self.assertRegex(result.scientific["numeric_hex_sha256"], r"^[0-9a-f]{64}$")
        self.assertEqual(result.seconds, 0.5)
        self.assertEqual(set(result.scientific["outputs"]), {"f0_resid"})

    def _machine_capture(self) -> str:
        records = [capture.MACHINE_PREFIX + "schema|1"]
        identities = {
            "profile": self.spec.profile,
            "arm": self.spec.arm,
            "case_id": self.spec.case_id,
            "manifest_sha256": self.spec.manifest_sha256,
            "source_commit": self.spec.source_commit,
            "plugin_sha256": self.spec.plugin_sha256,
            "dataset_sha256": self.spec.dataset_sha256,
            "command_sha256": hashlib.sha256(self.spec.command.encode()).hexdigest(),
        }
        records.extend(
            capture.MACHINE_PREFIX + f"identity|{name}|{value}"
            for name, value in identities.items()
        )
        for name, value in {
            "stata_version": "19.0",
            "stata_flavor": "MP",
            "os": "MacOSX",
            "machine_type": "Macintosh",
            "requested_threads": "1",
            "stata_processors": "1",
        }.items():
            records.append(capture.MACHINE_PREFIX + f"environment|{name}|{value}")
        for name, path, digest in capture._package_inventory(self.spec):
            records.append(
                capture.MACHINE_PREFIX
                + f"resolver|{name}|{str(path).replace(chr(92), '/')}|{digest}"
            )
        records.extend(
            [
                capture.MACHINE_PREFIX + "dataset|observations|100000",
                capture.MACHINE_PREFIX + "dataset|stata_signature|data-signature",
                capture.MACHINE_PREFIX + "phase_begin|science",
            ]
        )
        matrices = self._matrices()
        for phase in ("science", "timed"):
            records.append(capture.MACHINE_PREFIX + f"inventory|{phase}|scalar|1|converged")
            records.append(capture.MACHINE_PREFIX + f"scalar|{phase}|converged|{self._hex(1)}")
            for position, (name, matrix) in enumerate(matrices.items(), 1):
                records.append(
                    capture.MACHINE_PREFIX
                    + f"inventory|{phase}|matrix|{position}|{name}"
                )
                rows, columns, values, row_names, column_names = matrix
                records.append(
                    capture.MACHINE_PREFIX
                    + f"matrix_dimensions|{phase}|{name}|{rows}|{columns}"
                )
                for row in range(rows):
                    for column in range(columns):
                        records.append(
                            capture.MACHINE_PREFIX
                            + "matrix|"
                            + f"{phase}|{name}|{row + 1}|{column + 1}|"
                            + f"{row_names[row]}|{column_names[column]}|"
                            + self._hex(values[row * columns + column])
                        )
            records.append(
                capture.MACHINE_PREFIX
                + f"sample|{phase}|99998|2|mask-signature|inside-signature|outside-signature"
            )
            records.append(
                capture.MACHINE_PREFIX
                + f"saved_output|{phase}|f0_resid|99998|resid-signature|"
                + f"{self._hex(-1)}|{self._hex(1)}|{self._hex(0)}"
            )
            if phase == "science":
                records.append(capture.MACHINE_PREFIX + "return_code|science|0")
                records.append(capture.MACHINE_PREFIX + "phase_end|science")
                records.extend(
                    [
                        capture.MACHINE_PREFIX + "stabilizer|1|0",
                        capture.MACHINE_PREFIX + "stabilizer|2|0",
                    ]
                )
        records.extend(
            [
                capture.MACHINE_PREFIX + "return_code|timed|0",
                capture.MACHINE_PREFIX + f"timing_seconds|{self._hex(0.5)}",
                capture.MACHINE_PREFIX
                + f"capture_complete|arm|{self.spec.arm}|case_id|{self.spec.case_id}|rc|0",
            ]
        )
        return "\n".join(records) + "\n"

    def _matrices(self):
        zero = 0.0
        status = [1, 1, 0, 1, 1, 2, 0, 2]
        solver = [zero] * 32
        certificate = [zero] * 64
        receipt = [zero] * 15 + [1.0]
        return {
            "b": (1, 2, [1.0, 2.0], ["y"], ["x1", "x2"]),
            "V": (2, 2, [1.0, 0.0, 0.0, 2.0], ["x1", "x2"], ["x1", "x2"]),
            "fereg_status": (2, 4, status, ["x1", "x2"], ["slot", "status", "kept", "coord"]),
            "fereg_fit": (2, 2, [1.0, 100.0, 1.0, 2.0], ["N", "rss"], ["defined", "value"]),
            "fereg_dof": (2, 2, [1.0, 10.0, 1.0, 90.0], ["df_m", "df_r"], ["defined", "value"]),
            "fereg_solver": (2, 16, solver, ["fit", "residual"], [f"c{i}" for i in range(1, 17)]),
            "fereg_certificate": (32, 2, certificate, [f"r{i}" for i in range(1, 33)], ["defined", "value"]),
            "fereg_runtime": (30, 2, [zero] * 60, [f"r{i}" for i in range(1, 31)], ["defined", "value"]),
            "fereg_receipt": (1, 16, receipt, ["receipt"], [f"c{i}" for i in range(1, 17)]),
        }

    @staticmethod
    def _hex(value: float) -> str:
        sign = "+" if value >= 0 else "-"
        absolute = abs(float(value))
        mantissa, exponent = absolute.hex().split("p")
        mantissa = mantissa.removeprefix("0x")
        exponent_value = int(exponent)
        exponent_sign = "+" if exponent_value >= 0 else "-"
        return f"{sign}{mantissa}X{exponent_sign}{abs(exponent_value):03x}"


if __name__ == "__main__":
    unittest.main()
