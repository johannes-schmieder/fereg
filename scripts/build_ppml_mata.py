#!/usr/bin/env python3
"""Rebuild the bounded PPML transport objects with the oldest supported Stata."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts.run_stata_tests import run_fixture, stata_string  # noqa: E402

SOURCE = "_fereg_ppml_result_mata.ado"
RECEIPT = "_fereg_ppml_result_mata.json"
FUNCTIONS = (
    "_fereg_ppml_validation_error",
    "_fereg_ppml_validate_result",
    "_fereg_ppml_post_scalars",
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(stata: str, output: Path) -> dict:
    source = ROOT / "stata" / SOURCE
    with tempfile.TemporaryDirectory(prefix="fereg-mata-build-") as temporary:
        work = Path(temporary)
        fixture = work / "build_ppml_mata.do"
        fixture.write_text(
            "version 15.0\nclear all\nset more off\n"
            # Version control does not make newer object code backwards readable.
            "assert floor(c(stata_version)) == 15\n"
            "mata: mata set matastrict on\n"
            "mata: mata set mataoptimize on\n"
            "mata: mata set matafavor space\n"
            f"run {stata_string(source)}\n"
            + "".join(
                f"mata: mata mosave {name}(), dir({stata_string(work)})\n"
                for name in FUNCTIONS
            )
            + 'display as result "END_FIXTURE|build_ppml_mata|rc=0"\n'
        )
        result = run_fixture(stata, fixture, ROOT, require_status=True)
        if result.error or result.process_returncode != 0 or result.stata_rc != 0:
            raise RuntimeError(f"Mata object build failed: {result.error}")
        if str(result.metadata.get("stata_version", "")).split(".")[0] != "15":
            raise RuntimeError("Mata objects require the Stata 15 compiler")
        receipt = dict(
            schema="fereg-ppml-mata-v1", compiler_release="15",
            source_sha256=digest(source),
            objects={name + ".mo": digest(work / (name + ".mo")) for name in FUNCTIONS},
        )
        output.mkdir(parents=True, exist_ok=True)
        # The receipt is last. An interrupted rebuild fails the staging hash check.
        for name in receipt["objects"]:
            pending = output / (name + ".tmp")
            shutil.copyfile(work / name, pending)
            os.replace(pending, output / name)
        pending = output / (RECEIPT + ".tmp")
        pending.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        os.replace(pending, output / RECEIPT)
        return receipt


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "stata")
    args = parser.parse_args()
    print(json.dumps(build(args.stata, args.output), indent=2, sort_keys=True))
