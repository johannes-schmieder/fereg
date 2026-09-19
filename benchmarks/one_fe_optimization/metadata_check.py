"""Check public coefficient metadata against the unchanged dispatcher."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def main(root, baseline, candidate, label):
    root = root.resolve()
    output = root / ("metadata-check-" + candidate + label)
    output.mkdir(exist_ok=False)
    reference = root / baseline / "stata/_fereg_estimate_public.ado"
    hidden_reference = root / baseline / "stata/_fereg_estimate_ols.ado"
    reference_text = reference.read_text().replace("program define _fereg_estimate_public,", "program define _fereg_public_reference,")
    reference_text = reference_text.replace("_fereg_estimate_ols", "_fereg_ols_reference")
    hidden_text = hidden_reference.read_text().replace("_fereg_estimate_ols", "_fereg_ols_reference")
    hidden_text = hidden_text.replace("_fereg_eols_validate", "_fereg_eols_validate_reference")
    (output / "reference.do").write_text(reference_text + "\n" + hidden_text)
    source = root / candidate
    script = f'''version 19.0
clear all
set more off
sysdir set PLUS "{root}/work/refs"
sysdir set PERSONAL "{output}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{source}/stata"
adopath ++ "{source}/dist"
do "{output}/reference.do"
set obs 500
generate long group = mod(_n,10)
generate long time = _n
tsset time
generate double x = sin(_n/7)
generate double z = cos(_n/11)
generate double copyx = x
generate double y = .3*x + .2*z + group/10 + sin(_n/13)
generate double w = 1 + mod(_n,5)
foreach command in "y x z, absorb(group)" "y x copyx, absorb(group)" ///
    "y o.x z, absorb(group)" "y i.group z" "y x z, noconstant" ///
    "y, absorb(group)" "y, absorb(group) noconstant" ///
    "y c.x##i.group z" "y L.x z, absorb(group)" ///
    "y x z [aw=w], absorb(group)" {{
    capture quietly _fereg_public_reference `command'
    local expected_rc = _rc
    if `expected_rc' {{
        capture quietly _fereg_estimate_public `command'
        assert _rc == `expected_rc'
        continue
    }}
    local count = e(fereg_receipt)[1,5]
    if `count' > 0 {{
        matrix old_b = e(b)
        matrix old_V = e(V)
        local old_names : colfullnames e(b)
        local old_vnames : colfullnames e(V)
    }}
    matrix old_status = e(fereg_status)
    local old_indepvars `"`e(indepvars)'"'
    local old_macros : e(macros)
    local macro_index 0
    foreach item of local old_macros {{
        local ++macro_index
        local old_m`macro_index' `"`e(`item')'"'
    }}
    quietly _fereg_estimate_public `command'
    assert e(fereg_receipt)[1,5] == `count'
    if `count' > 0 {{
        assert mreldif(e(b),old_b) == 0
        assert mreldif(e(V),old_V) == 0
        local names : colfullnames e(b)
        local vnames : colfullnames e(V)
        assert `"`names'"' == `"`old_names'"'
        assert `"`vnames'"' == `"`old_vnames'"'
    }}
    assert mreldif(e(fereg_status),old_status) == 0
    assert `"`e(indepvars)'"' == `"`old_indepvars'"'
    local macro_index 0
    foreach item of local old_macros {{
        local ++macro_index
        assert `"`e(`item')'"' == `"`old_m`macro_index''"'
    }}
    local new_macros : e(macros)
    local old_macros : list sort old_macros
    local new_macros : list sort new_macros
    assert `"`old_macros'"' == `"`new_macros'"'
}}
display "PUBLIC_METADATA_IDENTITY_PASS"
'''
    (output / "run.do").write_text(script)
    with (output / "stdout.txt").open("w") as stdout:
        result = subprocess.run([os.environ["STATA_MP_PATH"], "-b", "do", str(output / "run.do")], cwd=output,
            stdout=stdout, stderr=subprocess.STDOUT, timeout=180)
    assert result.returncode == 0 and "\nPUBLIC_METADATA_IDENTITY_PASS\n" in (output / "run.log").read_text(), output
    sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    record = dict(status="pass", cases=10, baseline=sha(reference), baseline_hidden=sha(hidden_reference), candidate=sha(source / "stata/_fereg_estimate_public.ado"), script=sha(output / "run.do"), log=sha(output / "run.log"))
    (root / ("metadata-check-" + candidate + ".json")).write_text(json.dumps(record, indent=2) + "\n")
    print("PUBLIC_METADATA_IDENTITY_PASS", candidate, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("baseline")
    p.add_argument("candidate")
    p.add_argument("--label", default="")
    a = p.parse_args()
    main(a.root, a.baseline, a.candidate, a.label)
