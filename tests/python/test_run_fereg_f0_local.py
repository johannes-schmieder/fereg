from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts import fereg_f0_regression as evaluator
from scripts import fereg_f0_stata_capture as capture
from scripts import run_fereg_f0_local as local


class FeregF0LocalRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.manifest = evaluator.load_manifest(
            local.DEFAULT_MANIFEST, verify_git_object=False
        )
        self.manifest_sha = evaluator.sha256_file(local.DEFAULT_MANIFEST)
        self.config = self.root / "local-input.json"
        self.document = self._input_document()
        self.config.write_text(json.dumps(self.document), encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _input_document(self) -> dict:
        datasets = []
        cells = []
        registered = [
            cell for cell in self.manifest["cells"] if cell["profile"] == local.PROFILE
        ]
        for index, cell in enumerate(registered):
            dataset_id = f"dataset_{index:02d}"
            dataset = self.root / f"{dataset_id}.dta"
            dataset.write_bytes(f"immutable-{cell['id']}".encode("ascii"))
            datasets.append(
                {
                    "id": dataset_id,
                    "path": dataset.name,
                    "sha256": hashlib.sha256(dataset.read_bytes()).hexdigest(),
                    "observations": cell["n"],
                }
            )
            absorb = "worker firm"
            saved = []
            if cell["design"] == "absorbed_slope":
                absorb = "worker firm##c.year"
            if cell["outputs"] == "save_fe":
                if cell["design"] == "saved_slope":
                    absorb = "__f0_worker=worker __f0_firm=firm##c.year"
                    saved = ["__f0_worker", "__f0_firm", "__f0_firm_Slope1"]
                else:
                    absorb = "__f0_worker=worker __f0_firm=firm"
                    saved = ["__f0_worker", "__f0_firm"]
            weight = ""
            if cell["weight"] != "unweighted":
                abbreviation = local.WEIGHT_ABBREVIATIONS[cell["weight"]]
                variable = local.WEIGHT_VARIABLES[cell["weight"]]
                weight = f" [{abbreviation}={variable}]"
            command = (
                f"fereg y_linear c.x1 c.x2{weight}, absorb({absorb}) "
                f"threads({cell['threads']}) solver({cell['solver']}) "
                f"memory({cell['memory']})"
            )
            if cell["preconditioner"] != "auto":
                command += f" preconditioner({cell['preconditioner']})"
            if cell["vce"] == "cluster":
                variables = (
                    ("firm", "year")
                    if cell["design"] == "clustered_vce"
                    else local.CLUSTER_VARIABLES[: cell["cluster_dimensions"]]
                )
                command += f" vce(cluster {' '.join(variables)})"
            elif cell["vce"] == "robust":
                command += " vce(robust)"
            cells.append(
                {
                    "cell_id": cell["id"],
                    "dataset_id": dataset_id,
                    "command": command,
                    "saved_outputs": saved,
                }
            )
        return {
            "schema_version": 1,
            "input_type": local.INPUT_TYPE,
            "manifest_sha256": self.manifest_sha,
            "obs_id": "obs_id",
            "datasets": datasets,
            "cells": cells,
        }

    def test_input_contract_consumes_exact_manifest_local_inventory(self) -> None:
        loaded = local.load_local_inputs(
            self.config, self.manifest, self.manifest_sha
        )
        self.assertEqual(
            tuple(cell.cell_id for cell in loaded.cells),
            evaluator.EXPECTED_CELLS_BY_PROFILE["local_screen"],
        )
        self.assertEqual(
            len(loaded.cells),
            len(evaluator.EXPECTED_CELLS_BY_PROFILE["local_screen"]),
        )
        self.assertEqual(loaded.obs_id, "obs_id")
        self.assertTrue(all(cell.dataset.path.is_absolute() for cell in loaded.cells))

    def test_input_contract_rejects_reorder_and_dataset_mutation(self) -> None:
        self.document["cells"][0], self.document["cells"][1] = (
            self.document["cells"][1],
            self.document["cells"][0],
        )
        self.config.write_text(json.dumps(self.document), encoding="utf-8")
        with self.assertRaisesRegex(local.LocalRunContractError, "inventory/order"):
            local.load_local_inputs(self.config, self.manifest, self.manifest_sha)

        self.document = self._input_document()
        self.config.write_text(json.dumps(self.document), encoding="utf-8")
        first = self.root / self.document["datasets"][0]["path"]
        first.write_bytes(first.read_bytes() + b"changed")
        with self.assertRaisesRegex(local.LocalRunContractError, "SHA-256 mismatch"):
            local.load_local_inputs(self.config, self.manifest, self.manifest_sha)

    def test_complete_package_stage_uses_only_selected_native_artifact(self) -> None:
        source = self.root / "source"
        stata = source / "stata"
        stata.mkdir(parents=True)
        (stata / "fereg.pkg").write_text(
            "\n".join(
                (
                    "v 3",
                    "f fereg.ado",
                    "f helper.ado",
                    "f fereg_macos.plugin",
                    "f fereg_linux.plugin",
                    "f fereg_windows.plugin",
                )
            )
            + "\n",
            encoding="utf-8",
        )
        (stata / "stata.toc").write_text("v 3\n", encoding="utf-8")
        (stata / "fereg.ado").write_text("program fereg\nend\n", encoding="utf-8")
        (stata / "helper.ado").write_text("program helper\nend\n", encoding="utf-8")
        plugin = self.root / "candidate.plugin"
        plugin.write_bytes(b"native-artifact")
        destination = self.root / "staged"
        local.stage_package(source, plugin, destination)
        self.assertEqual(
            {path.name for path in destination.iterdir()},
            {"fereg.pkg", "stata.toc", "fereg.ado", "helper.ado", "fereg_macos.plugin"},
        )
        self.assertEqual((destination / "fereg_macos.plugin").read_bytes(), plugin.read_bytes())
        with self.assertRaisesRegex(local.LocalRunContractError, "overwrite"):
            local.stage_package(source, plugin, destination)

    def test_scientific_preflight_precedes_exact_nine_fresh_pairs(self) -> None:
        loaded = local.load_local_inputs(
            self.config, self.manifest, self.manifest_sha
        )
        cell = loaded.cells[0]
        registered = next(item for item in self.manifest["cells"] if item["id"] == cell.cell_id)
        calls: list[tuple[str, str]] = []

        def fake_runner(stata, spec, *, timeout_seconds):
            calls.append((spec.arm, str(spec.output_dir.relative_to(self.root))))
            result = capture.ArmCaptureResult(
                scientific={"arm": spec.arm, "certificates": {"digests": {}}},
                seconds=1.0,
                environment={
                    "stata_version": "19.0",
                    "stata_flavor": "MP",
                    "os": "MacOSX",
                    "machine_type": "Macintosh",
                    "requested_threads": "1",
                    "stata_processors": "1",
                },
                identities={},
                raw={},
            )
            return local.ArmRun(result=result, peak_rss_bytes=1024)

        identities = {
            arm: {
                "source": {"commit": character * 40},
                "artifact": {"sha256": character * 64},
            }
            for arm, character in (("baseline", "a"), ("candidate", "b"))
        }
        with (
            mock.patch.object(local.evaluator, "compare_scientific", return_value=[]),
            mock.patch.object(local, "_matrix_value", return_value=0.0),
            mock.patch.object(local, "_memory_record", return_value={"closed": True}),
            mock.patch.object(local, "_confirmation_required", return_value=False),
        ):
            evidence, _ = local.run_cell(
                cell=cell,
                registered=registered,
                manifest=self.manifest,
                manifest_sha256=self.manifest_sha,
                input_sha256=loaded.input_sha256,
                preparation_sha256="c" * 64,
                identities=identities,
                packages={"baseline": self.root, "candidate": self.root},
                obs_id=loaded.obs_id,
                cell_root=self.root / "cell",
                stata="StataMP",
                timeout_seconds=60,
                runner=fake_runner,
            )
        self.assertEqual([arm for arm, _ in calls[:2]], ["baseline", "candidate"])
        self.assertTrue(all("preflight" in path for _, path in calls[:2]))
        expected = []
        for order in local.PAIR_ORDERS:
            expected.extend(
                ("baseline", "candidate") if order == "AB" else ("candidate", "baseline")
            )
        self.assertEqual([arm for arm, _ in calls[2:]], expected)
        self.assertEqual(len(calls), 20)
        self.assertEqual([pair["order"] for pair in evidence["pairs"]], list(local.PAIR_ORDERS))
        self.assertIsNone(evidence["confirmation_pairs"])
        expected_input_digests = {
            "local_manifest": self.manifest_sha,
            "local_input_config": loaded.input_sha256,
            "local_preparation_receipt": "c" * 64,
            "local_dataset": cell.dataset.sha256,
            "local_command": hashlib.sha256(cell.command.encode("utf-8")).hexdigest(),
        }
        for arm in ("baseline", "candidate"):
            digests = evidence["scientific"][arm]["certificates"]["digests"]
            for name, value in expected_input_digests.items():
                self.assertEqual(digests[name], value)
            self.assertRegex(digests["local_input_contract"], r"^[0-9a-f]{64}$")

    def test_failed_preflight_never_enters_pair_timing(self) -> None:
        loaded = local.load_local_inputs(
            self.config, self.manifest, self.manifest_sha
        )
        cell = loaded.cells[0]
        registered = next(item for item in self.manifest["cells"] if item["id"] == cell.cell_id)
        calls = []

        def fake_runner(stata, spec, *, timeout_seconds):
            calls.append(spec.output_dir)
            return local.ArmRun(
                result=capture.ArmCaptureResult(
                    scientific={"certificates": {"digests": {}}},
                    seconds=1.0,
                    environment={
                        "stata_version": "19.0",
                        "stata_flavor": "MP",
                        "os": "MacOSX",
                        "machine_type": "Macintosh",
                        "requested_threads": "1",
                        "stata_processors": "1",
                    },
                    identities={},
                    raw={},
                ),
                peak_rss_bytes=1024,
            )

        identities = {
            arm: {"source": {"commit": "a" * 40}, "artifact": {"sha256": "b" * 64}}
            for arm in ("baseline", "candidate")
        }
        with mock.patch.object(local.evaluator, "compare_scientific", return_value=["coefficients"]):
            with self.assertRaisesRegex(local.LocalRunContractError, "before timing"):
                local.run_cell(
                    cell=cell,
                    registered=registered,
                    manifest=self.manifest,
                    manifest_sha256=self.manifest_sha,
                    input_sha256=loaded.input_sha256,
                    preparation_sha256="c" * 64,
                    identities=identities,
                    packages={"baseline": self.root, "candidate": self.root},
                    obs_id=loaded.obs_id,
                    cell_root=self.root / "cell",
                    stata="StataMP",
                    timeout_seconds=60,
                    runner=fake_runner,
                )
        self.assertEqual(len(calls), 2)

    def test_preparation_generates_closed_datasets_and_seals_registered_cell_config(self) -> None:
        source = self.root / "f0-source"
        generator = source / "benchmarks/generators/generate_panel.do"
        ols_path = source / "benchmarks/manifests/ols_v1.json"
        generator.parent.mkdir(parents=True)
        ols_path.parent.mkdir(parents=True, exist_ok=True)
        frozen_ols = json.loads((local.ROOT / "benchmarks/manifests/ols_v1.json").read_text())
        generator_bytes = (local.ROOT / "benchmarks/generators/generate_panel.do").read_bytes()
        generator.write_bytes(generator_bytes)
        ols_path.write_text(json.dumps(frozen_ols), encoding="utf-8")
        output = self.root / "prepared"

        def fake_process(command, *, cwd, check, stdout, stderr, text):
            wrapper = (cwd / "generate.do").read_text(encoding="utf-8")
            dataset_id = cwd.name
            spec = next(item for item in local._prepared_dataset_specs() if item.id == dataset_id)
            destination = output / "datasets" / f"{dataset_id}.dta"
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((dataset_id + "-with-obs-id").encode("ascii"))
            (cwd / "generate.log").write_text(
                f"FEREG_F0_DATASET_SUCCESS|{dataset_id}\n", encoding="utf-8"
            )
            (cwd / "generation.machine").write_text(
                "\n".join(
                    (
                        "schema=1",
                        "stata_version=19.0",
                        "stata_flavor=MP",
                        "os=MacOSX",
                        "machine_type=Macintosh",
                        f"dataset_id={dataset_id}",
                        f"variant={spec.variant}",
                    )
                )
                + "\n",
                encoding="utf-8",
            )
            self.assertIn("generate long obs_id = _n", wrapper)
            if spec.variant == "qualified_dense":
                self.assertIn("generate byte __f0_cluster5", wrapper)
                self.assertIn("generate byte __f0_fweight", wrapper)
            if spec.variant == "disconnected":
                self.assertIn("replace worker = 2*worker + __f0_component", wrapper)
            return __import__("subprocess").CompletedProcess(command, 0, stdout="")

        args = type(
            "Args",
            (),
            {
                "manifest": local.DEFAULT_MANIFEST,
                "baseline_source": source,
                "stata": "StataMP",
                "output_root": output,
            },
        )()
        with (
            mock.patch.object(local.evaluator, "load_manifest", return_value=self.manifest),
            mock.patch.object(
                local.evaluator,
                "source_identity",
                return_value=self.manifest["baseline_candidate"]["source"],
            ),
        ):
            receipt = local.prepare_local(args, process_runner=fake_process)
        config = output / "local-input.json"
        loaded = local.load_local_inputs(config, self.manifest, self.manifest_sha)
        self.assertEqual(
            len({cell.dataset.id for cell in loaded.cells}),
            len(local._prepared_dataset_specs()),
        )
        self.assertEqual(
            len(loaded.cells),
            len(evaluator.EXPECTED_CELLS_BY_PROFILE["local_screen"]),
        )
        self.assertIn("i.year##c.(x1 x2)", next(
            cell.command for cell in loaded.cells if "moderate_k" in cell.cell_id
        ))
        by_id = {cell.cell_id: cell for cell in loaded.cells}
        self.assertIn(
            "vce(robust)",
            by_id["local_n100k_robust_t8_fast_fit"].command,
        )
        self.assertIn(
            "[fw=__f0_fweight]",
            by_id["local_n100k_fweight_robust_t1_fast_fit"].command,
        )
        cluster5 = by_id["local_n100k_cluster5_t1_low_fit"]
        self.assertIn(
            "vce(cluster __f0_cluster1 __f0_cluster2 __f0_cluster3 "
            "__f0_cluster4 __f0_cluster5)",
            cluster5.command,
        )
        self.assertEqual(cluster5.dataset.id, "local_n100k_qualified_dense")
        moderate_cluster = by_id["local_n100k_moderate_k_cluster2_t8_fast_fit"]
        self.assertIn("i.year##c.(x1 x2)", moderate_cluster.command)
        self.assertIn("threads(8)", moderate_cluster.command)
        disconnected = by_id["local_n1m_disconnected_two_fe_t1_fast_save_fe"]
        self.assertEqual(disconnected.dataset.id, "local_n1m_disconnected")
        self.assertEqual(
            disconnected.saved_outputs, ("__f0_worker", "__f0_firm")
        )
        saved_slope = by_id["local_n1m_saved_slope_t1_fast_save_fe"]
        self.assertEqual(
            saved_slope.saved_outputs,
            ("__f0_worker", "__f0_firm", "__f0_firm_Slope1"),
        )
        self.assertIn("__f0_firm=firm##c.year", saved_slope.command)
        for solver, preconditioner in (
            ("map", "off"),
            ("cg", "off"),
            ("it", "off"),
            ("squarem", "off"),
            ("lsmr_off", "off"),
            ("lsmr_diagonal", "diagonal"),
            ("lsmr_schwarz", "schwarz"),
            ("cmg", "off"),
        ):
            cell = by_id[f"local_n100k_solver_{solver}_t1_fast_fit"]
            requested_solver = "lsmr" if solver.startswith("lsmr_") else solver
            self.assertIn(f"solver({requested_solver})", cell.command)
            self.assertIn(f"preconditioner({preconditioner})", cell.command)
        self.assertEqual(receipt["payload"]["terminal_sentinel"], local.PREPARATION_SENTINEL)
        receipt_path = output / "preparation-receipt.json"
        receipt_sha, stata_environment = local.verify_preparation(
            receipt_path, config, loaded, self.manifest, source
        )
        self.assertEqual(receipt_sha, local.capture.sha256_file(receipt_path))
        self.assertEqual(stata_environment["stata_version"], "19.0")


if __name__ == "__main__":
    unittest.main()
