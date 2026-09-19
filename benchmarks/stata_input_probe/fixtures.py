"""Run existing focused scientific/transaction fixtures against a local variant."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

REPO = Path(__file__).resolve().parents[2]


def main(root, variant, fixtures=None, label=""):
    root = root.resolve()
    source = root / variant
    shutil.copytree(REPO / "tests/fixtures", source / "tests/fixtures", dirs_exist_ok=True)
    records = []
    selected = fixtures or ("development/ols_native_input.do", "development/candidate_input_oracle.do", "installation/estimate_ols_eclass_transaction.do")
    for relative in selected:
        fixture = REPO / "tests/stata" / relative
        output = root / "fixtures" / (variant + label) / fixture.stem
        output.mkdir(parents=True, exist_ok=False)
        shutil.copyfile(fixture, output / fixture.name)
        script = f'''version 19.0
clear all
set more off
set processors 8
sysdir set PLUS "{root}/work/refs"
sysdir set PERSONAL "{output}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{source}/stata"
adopath ++ "{source}/dist"
do "{output / fixture.name}"
display "ACCESSOR_FIXTURE_COMPLETE"
'''
        (output / "run.do").write_text(script)
        with (output / "stdout.txt").open("w") as out:
            result = subprocess.run([os.environ["STATA_MP_PATH"], "-b", "do", str(output / "run.do")], cwd=output, stdout=out, stderr=subprocess.STDOUT, timeout=180)
        log = (output / "run.log").read_text()
        sentinel = f"END_FIXTURE|{fixture.stem}|rc=0"
        assert result.returncode == 0 and "\n" + sentinel + "\n" in log and "\nACCESSOR_FIXTURE_COMPLETE\n" in log, output
        record = dict(fixture=relative, sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(), status="pass", process_returncode=result.returncode, fixture_sentinel=sentinel, terminal_sentinel=True)
        records.append(record)
        print(variant, fixture.stem, "pass", flush=True)
    (root / ("fixtures-" + variant + label + ".json")).write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("variant", choices=("baseline", "fast"))
    a = p.parse_args()
    main(a.root, a.variant)
