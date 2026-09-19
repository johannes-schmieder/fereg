"""Source-bound Stata boundary timers reusing an isolated native artifact."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sys

REPO = Path(__file__).resolve().parents[2]


def stage(root, baseline, name):
    root = root.resolve()
    source = root / name
    source.mkdir(exist_ok=False)
    for folder in ("stata", "dist"):
        shutil.copytree(root / baseline / folder, source / folder)
    sys.path.insert(0, str(REPO / "benchmarks/one_fe_profile"))
    spec = importlib.util.spec_from_file_location("instrument_fereg", REPO / "benchmarks/one_fe_profile/instrument_fereg.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.instrument_ado(source)

    def bracket(path, before, after, number, label):
        text = path.read_text()
        assert text.count(before) == text.count(after) == 1
        text = text.replace(before, f"    timer clear {number}\n    timer on {number}\n" + before)
        text = text.replace(after, after + f'''\n    timer off {number}
    quietly timer list {number}
    noisily display "FEREG_ADO_DETAIL|{label}|seconds=" %12.6f r(t{number})
''')
        path.write_text(text)

    bracket(source / "stata/_fereg_estimate_public.ado", "    syntax varlist(fv ts numeric min=1)", "         NOCONStant]", 86, "public_syntax")
    bracket(source / "stata/_fereg_estimate_ols.ado", "    syntax varlist(fv ts numeric min=1)", "         INJECT(string)]", 87, "transaction_syntax")
    bracket(source / "stata/_fereg_estimate_public.ado", "    local coefficient_count = e(fereg_receipt)[1,5]", "    local intended_indepvars = strtrim(`\"`intended_indepvars'\"')", 88, "public_names")
    path = source / "stata/_fereg_estimate_public.ado"
    body = path.read_text()
    index = body.rindex("\nend")
    body = body[:index] + """
    tempname phase_file
    file open `phase_file' using "ado-phases.tsv", write append text
    forvalues phase_timer = 81/88 {
        quietly timer list `phase_timer'
        file write `phase_file' %12.0f (_N) _tab %4.0f (`phase_timer') _tab %24.17g (r(t`phase_timer')) _n
    }
    file close `phase_file'
""" + body[index:]
    path.write_text(body)
    identity = json.loads((root / ("identity-" + baseline + ".json")).read_text())
    identity["native_reused_from"] = baseline
    identity["diagnostic"] = "Stata transaction and syntax timers"
    for path in (source / "stata").glob("*.ado"):
        identity["source"][str(path.relative_to(source))] = hashlib.sha256(path.read_bytes()).hexdigest()
    identity["harness"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (root / ("identity-" + name + ".json")).write_text(json.dumps(identity, indent=2) + "\n")
    print("ADO_PROFILE_STAGED", name)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("baseline")
    p.add_argument("name")
    a = p.parse_args()
    stage(a.root, a.baseline, a.name)
