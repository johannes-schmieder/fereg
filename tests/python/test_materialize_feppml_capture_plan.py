from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "scripts/materialize_feppml_capture_plan.py"
SPEC = importlib.util.spec_from_file_location("materialize_feppml_capture_plan_test", PATH)
assert SPEC is not None and SPEC.loader is not None
MATERIALIZE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MATERIALIZE)


def write(path: Path, content: bytes) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return path


class MaterializeFeppmlCapturePlanTests(unittest.TestCase):
    def test_file_and_bundle_symlinks_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.bin"
            source.write_bytes(b"source")
            linked_file = root / "linked.bin"
            linked_file.symlink_to(source)
            with self.assertRaisesRegex(MATERIALIZE.MaterializationError, "non-symlink"):
                MATERIALIZE.file_record(linked_file)
            directory = root / "bundle"
            directory.mkdir()
            (directory / "file").write_bytes(b"bundle")
            linked_directory = root / "linked-bundle"
            linked_directory.symlink_to(directory, target_is_directory=True)
            with self.assertRaisesRegex(MATERIALIZE.MaterializationError, "real directory"):
                MATERIALIZE.tree_inventory(linked_directory)

    def inputs(self, root: Path) -> dict:
        development = write(root / "input/manifests/development.json", b"{}\n")
        request = {
            "schema_version": 1,
            "request_type": "feppml-capture-materialization-request-v1",
            "qualification_manifest_sha256": "a" * 64,
            "development_manifest_sha256": MATERIALIZE.digest_file(development),
            "cell_ids": ["cell-a", "cell-b"],
            "oracle_cell_ids": ["cell-a"],
            "constrained_fixture_ids": ["low_succeeds_fast_fails"],
            "stata_batch_arguments": ["-q", "-b", "do"],
            "comparator_processor_limit": 16,
            "complete_sentinel": "FEPPML_CAPTURE_MATERIALIZATION_REQUEST_COMPLETE",
        }
        request_path = root / "input/capture/materialization-request.json"
        request_path.parent.mkdir(parents=True, exist_ok=True)
        request_path.write_text(json.dumps(request, sort_keys=True), encoding="ascii")
        stata = write(root / "bin/stata-mp", b"#!/bin/sh\nexit 0\n")
        stata.chmod(0o755)
        recipe_root = root / "recipes"
        recipe_manifest = write(recipe_root / "recipes.json", b"{\"closed\":true}\n")
        write(recipe_root / "cells/cell-a/science.do", b"version 19\n")
        write(recipe_root / "cells/cell-a/timing.do", b"version 19\n")
        dataset_a = write(root / "datasets/a.dta", b"DTA-A")
        dataset_b = write(root / "datasets/b.dta", b"DTA-B")
        oracle_data = write(root / "oracle/a.dta", b"ORACLE-DTA")
        oracle_export = write(root / "oracle/a.csv", b"obs_id,y\n1,1\n")
        oracle_model = write(root / "oracle/model.json", b"{}\n")
        candidate_root = root / "candidate"
        plugin = write(candidate_root / "fereg_linux.plugin", b"plugin")
        write(candidate_root / "fereg.ado", b"program fereg\n")
        comparator_root = root / "comparator"
        write(comparator_root / "ppmlhdfe/ppmlhdfe.ado", b"program ppmlhdfe\n")
        tool = write(root / "harness/capture.py", b"# capture\n")

        def record(path: Path, **extra):
            return {**MATERIALIZE.file_record(path), **extra}

        cells = [
            {"cell_id": "cell-a", "oracle_fixture": {"data": MATERIALIZE.immutable(oracle_data)}},
            {"cell_id": "cell-b", "oracle_fixture": None},
        ]
        arguments = {
            "request_path": request_path,
            "development_manifest_path": development,
            "stata_executable": stata,
            "identity": {
                "source_commit": "1" * 40,
                "source_tree": "2" * 40,
                "cargo_lock_sha256": "3" * 64,
                "references_lock_sha256": "4" * 64,
                "manifest_sha256": MATERIALIZE.digest_file(development),
                "generator_sha256": "5" * 64,
                "reference_commits": {},
            },
            "cells": cells,
            "constrained_memory": [{"id": "low_succeeds_fast_fails", "attempts": []}],
            "recipe_manifest_path": recipe_manifest,
            "recipe_root": recipe_root,
            "datasets": [record(dataset_a, dataset_id="a"), record(dataset_b, dataset_id="b")],
            "oracle_fixtures": [
                {
                    "fixture_id": "cell-a__explicit_v1",
                    "data": record(oracle_data),
                    "canonical_export": record(oracle_export),
                    "model": record(oracle_model),
                }
            ],
            "candidate_source": {"commit": "1" * 40, "tree": "2" * 40},
            "candidate_plugin_path": plugin,
            "candidate_bundle_root": candidate_root,
            "comparator_sources": {"ppmlhdfe": {"commit": "6" * 40, "tree": "7" * 40}},
            "comparator_bundle_root": comparator_root,
            "tools": [tool],
        }
        return {"arguments": arguments, "dataset_a": dataset_a, "cells": cells}

    def test_golden_materialization_binds_every_inventory_before_capture(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            values = self.inputs(Path(temporary))
            first_plan, first_payload = MATERIALIZE.materialize(**values["arguments"])
            second_plan, second_payload = MATERIALIZE.materialize(**values["arguments"])
            self.assertEqual(first_plan, second_plan)
            self.assertEqual(first_payload, second_payload)
            closure = first_plan["closure"]
            self.assertEqual(set(closure), MATERIALIZE.CLOSURE_FIELDS)
            self.assertEqual(closure["candidate_source_tree"], "2" * 40)
            self.assertEqual(
                closure["dataset_inventory_sha256"],
                MATERIALIZE.digest(first_payload["datasets"]),
            )
            self.assertEqual(
                closure["oracle_fixture_inventory_sha256"],
                MATERIALIZE.digest(first_payload["oracle_fixtures"]),
            )
            plan_path = Path(temporary) / "work/capture/plan.json"
            receipt_path = Path(temporary) / "receipts/capture-plan.json"
            MATERIALIZE.write_new(plan_path, first_plan)
            MATERIALIZE.write_new(receipt_path, MATERIALIZE.seal_payload(first_payload))
            verified = MATERIALIZE.verify_receipt(plan_path, receipt_path)
            self.assertEqual(verified["plan_sha256"], hashlib.sha256(MATERIALIZE.canonical(first_plan)).hexdigest())

    def test_mutated_dataset_is_rejected_after_seal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            values = self.inputs(root)
            plan, payload = MATERIALIZE.materialize(**values["arguments"])
            plan_path = root / "work/capture/plan.json"
            receipt_path = root / "receipts/capture-plan.json"
            MATERIALIZE.write_new(plan_path, plan)
            MATERIALIZE.write_new(receipt_path, MATERIALIZE.seal_payload(payload))
            values["dataset_a"].write_bytes(b"changed")
            with self.assertRaisesRegex(MATERIALIZE.MaterializationError, "bytes changed"):
                MATERIALIZE.verify_receipt(plan_path, receipt_path)

    def test_portable_receipt_reopens_only_relocated_run_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run_id = "20260831T120000Z-feppml-q"
            original = root / "original" / run_id
            values = self.inputs(original)
            plan, payload = MATERIALIZE.materialize(**values["arguments"])
            plan_path = original / "work/capture/plan.json"
            receipt_path = original / "receipts/capture-plan.json"
            MATERIALIZE.write_new(plan_path, plan)
            MATERIALIZE.write_new(receipt_path, MATERIALIZE.seal_payload(payload))
            relocated = root / "collected" / run_id
            shutil.copytree(original, relocated)
            shutil.rmtree(original)
            verified = MATERIALIZE.verify_receipt_portable(
                relocated / "work/capture/plan.json",
                relocated / "receipts/capture-plan.json",
                relocated,
            )
            self.assertEqual(verified["plan_file_sha256"], payload["plan_file_sha256"])
            (relocated / "datasets/a.dta").write_bytes(b"portable-substitution")
            with self.assertRaisesRegex(
                MATERIALIZE.MaterializationError, "bytes changed"
            ):
                MATERIALIZE.verify_receipt_portable(
                    relocated / "work/capture/plan.json",
                    relocated / "receipts/capture-plan.json",
                    relocated,
                )

    def test_missing_oracle_or_constrained_inventory_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            values = self.inputs(Path(temporary))
            values["arguments"]["cells"][0]["oracle_fixture"] = None
            with self.assertRaisesRegex(MATERIALIZE.MaterializationError, "oracle inventory"):
                MATERIALIZE.materialize(**values["arguments"])
        with tempfile.TemporaryDirectory() as temporary:
            values = self.inputs(Path(temporary))
            values["arguments"]["constrained_memory"] = []
            with self.assertRaisesRegex(MATERIALIZE.MaterializationError, "constrained inventory"):
                MATERIALIZE.materialize(**values["arguments"])

    def test_request_rejects_extra_fields_and_unbounded_oracle_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            values = self.inputs(Path(temporary))
            request_path = values["arguments"]["request_path"]
            request = json.loads(request_path.read_text(encoding="ascii"))
            request["mutable_override"] = True
            request_path.write_text(json.dumps(request), encoding="ascii")
            with self.assertRaisesRegex(MATERIALIZE.MaterializationError, "not closed"):
                MATERIALIZE.load_request(request_path)

    def test_request_rejects_any_stata_batch_argv_prefix_or_suffix(self) -> None:
        for arguments in (
            ["/usr/bin/env", "-q", "-b", "do"],
            ["-q", "-b", "do", "injected.do"],
        ):
            with self.subTest(arguments=arguments), tempfile.TemporaryDirectory() as temporary:
                values = self.inputs(Path(temporary))
                request_path = values["arguments"]["request_path"]
                request = json.loads(request_path.read_text(encoding="ascii"))
                request["stata_batch_arguments"] = arguments
                request_path.write_text(json.dumps(request), encoding="ascii")
                with self.assertRaisesRegex(
                    MATERIALIZE.MaterializationError,
                    "exact noninteractive argv -q -b do",
                ):
                    MATERIALIZE.load_request(request_path)

    def test_request_rejects_throttled_comparator_processor_limits(self) -> None:
        for limit in (1, 8, 15):
            with self.subTest(limit=limit), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                values = self.inputs(root)
                request_path = values["arguments"]["request_path"]
                request = json.loads(request_path.read_text(encoding="ascii"))
                request["comparator_processor_limit"] = limit
                request_path.write_text(json.dumps(request), encoding="ascii")
                with self.assertRaisesRegex(
                    MATERIALIZE.MaterializationError, "frozen 16-thread ceiling"
                ):
                    MATERIALIZE.load_request(request_path)


if __name__ == "__main__":
    unittest.main()
