"""Compare a candidate parser's complete wire metadata with an earlier snapshot."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def main(root, baseline, candidate, label):
    root = root.resolve()
    output = root / ("parser-check-" + candidate + label)
    output.mkdir(exist_ok=False)
    reference = root / baseline / "stata/fereg_parse.ado"
    (output / "reference.do").write_text(reference.read_text().replace("program define fereg_parse,", "program define fereg_parse_reference,"))
    source = root / candidate / "stata/fereg_parse.ado"
    script = f'''version 19.0
clear all
set more off
do "{output}/reference.do"
do "{source}"
set obs 200
generate byte selected = mod(_n,3) != 0
generate byte empty = 0
generate long group = mod(_n,11)
generate long year = _n
tsset year
forvalues j=1/50 {{
    generate double x`j' = sin(_n*`j'/100)
}}
replace x1 = .a in 3
local matrices model_sources model_terms model_components absorb_terms absorb_components cluster_terms cluster_components
local strings source_vars coefficient_names component_raw_operators component_ts_operators absorb_labels absorb_saved_names absorb_label_registry absorb_saved_name_registry cluster_labels
local scalars model_wire_major model_wire_minor n_sources n_model_terms n_model_components n_absorb_terms n_absorb_components n_cluster_terms n_cluster_components
foreach rhs in "x1" "x1 x2" "x1-x50" "x*" "x1 x1 x2" "x1 c.x2" "i.group#c.x1 x2" "L.x1 x2" {{
    foreach mark in selected empty {{
        capture noisily fereg_parse_reference `rhs', touse(`mark') absorb(group) cluster(group)
        local expected_rc = _rc
        if `expected_rc' == 0 {{
            foreach m of local matrices {{
                matrix expected_`m' = r(`m')
                local names_`m' : rowfullnames r(`m')
            }}
            foreach s of local strings {{
                local e_`s' `"`r(`s')'"'
            }}
            foreach s of local scalars {{
                local e_`s' = r(`s')
            }}
        }}
        capture noisily fereg_parse `rhs', touse(`mark') absorb(group) cluster(group)
        assert _rc == `expected_rc'
        if `expected_rc' == 0 {{
            foreach m of local matrices {{
                assert mreldif(expected_`m',r(`m')) == 0
                local actual_names : rowfullnames r(`m')
                assert `"`actual_names'"' == `"`names_`m''"'
            }}
            foreach s of local strings {{
                assert `"`r(`s')'"' == `"`e_`s''"'
            }}
            foreach s of local scalars {{
                assert r(`s') == `e_`s''
            }}
        }}
    }}
}}
display "PARSER_WIRE_IDENTITY_PASS"
'''
    (output / "run.do").write_text(script)
    with (output / "stdout.txt").open("w") as stdout:
        result = subprocess.run([os.environ["STATA_MP_PATH"], "-b", "do", str(output / "run.do")], cwd=output,
            stdout=stdout, stderr=subprocess.STDOUT, timeout=120)
    assert result.returncode == 0 and "\nPARSER_WIRE_IDENTITY_PASS\n" in (output / "run.log").read_text(), output
    sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    record = dict(status="pass", cases=16, baseline=sha(reference), candidate=sha(source), script=sha(output / "run.do"), log=sha(output / "run.log"))
    (root / ("parser-check-" + candidate + ".json")).write_text(json.dumps(record, indent=2) + "\n")
    print("PARSER_WIRE_IDENTITY_PASS", candidate, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("baseline")
    p.add_argument("candidate")
    p.add_argument("--label", default="")
    a = p.parse_args()
    main(a.root, a.baseline, a.candidate, a.label)
