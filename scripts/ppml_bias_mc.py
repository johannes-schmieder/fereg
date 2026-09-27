#!/usr/bin/env python3
"""Stage a test-only native adapter and fresh-seed frozen-design MC jobs.

The supplied Separations directory is read-only. The generator binds every
reference byte and changes only the PPML invocation in a new adapter. Ordinary
wage/FEVC code and the external moment formulas are copied verbatim.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def stage(reference: Path, work: Path):
    root = Path(__file__).resolve().parents[1]
    work.mkdir(parents=True, exist_ok=False)
    runtime = work / "runtime"
    shutil.copytree(reference / "runtime", runtime)
    production = (runtime / "estimate_varcomponents.ado").read_text()
    begin = production.index("    quietly set seed `seed'\n    capture noisily mata: ev__main")
    end = production.index("    matrix rownames `out'", begin)
    replacement = (root / "tests/stata/bias/native_splitbranch_block.do").read_text()
    native = production[:begin] + replacement + production[end:]
    native = native.replace("program define estimate_varcomponents,", "program define fereg_bias_splitbranch,", 1)
    # Retain the frozen moment computation byte-for-byte. Feed it separately
    # saved native effects, already re-anchored to the same reference level.
    moment_start = production.index("    O=J(10,3,.)")
    moment_end = production.index("    st_matrix(outname,O);st_matrix(infoname,D)", moment_start)
    covariance = production[moment_start:moment_end]
    native += '''\ncapture mata: mata drop ev_native__moments()
mata:
void ev_native__moments(string scalar bv,string scalar rv,string scalar pv,
    string scalar av,string scalar fv,string scalar sv,string scalar tv,string scalar outname)
{
    real colvector ref,b,p,own,opp
    real matrix A,F,S,T,S0,S1,S2,T0,T1,T2,O,RR,QQ
    real rowvector mr,mq
    real scalar i,j,k,a,t
    ref=selectindex(st_data(.,rv):==1);b=st_data(.,bv);p=st_data(ref,pv)
    A=st_data(ref,tokens(av));F=st_data(ref,tokens(fv))
    S=st_data(ref,tokens(sv));T=st_data(ref,tokens(tv))
    S0=S[.,1..2];S1=S[.,3..4];S2=S[.,5..6]
    T0=T[.,1..2];T1=T[.,3..4];T2=T[.,5..6]
''' + covariance + '''    st_matrix(outname,O)
}
end
'''
    (runtime / "fereg_bias_splitbranch.ado").write_text(native)
    source = (reference / "mc_thorough.do").read_text()
    # Developer/pilot ranges are disjoint from the fixed confirmation range.
    for engine in ("legacy", "native"):
        driver = source
        if engine == "native":
            driver = driver.replace("        estimate_varcomponents,", "        fereg_bias_splitbranch,")
            driver = driver.replace("        timer off 1\n        timer list 1", """        if inrange(`rep',10001,10020) save "`out'/prefit_`rep'.dta", replace
        timer off 1
        timer list 1""", 1)
        driver = driver.replace("    local rc=_rc\n", """    local rc=_rc
    if `rc' capture save "`out'/failure_`rep'.dta", replace
""", 1)
        driver = driver.replace("exit, clear", 'display "END_FIXTURE|bias_mc|rc=0"')
        (work / f"mc_{engine}.do").write_text(driver)
    files = {str(p.relative_to(reference)): sha(p) for p in sorted((reference / "runtime").rglob("*")) if p.is_file()}
    files["mc_thorough.do"] = sha(reference / "mc_thorough.do")
    manifest = dict(schema=1, reference_files=files,
                    adapter_sha256=sha(runtime / "fereg_bias_splitbranch.ado"),
                    replacement_sha256=sha(root / "tests/stata/bias/native_splitbranch_block.do"),
                    confirmation=dict(first=10001,last=10300,exact_last=10020,large_probe_last=10050,
                                      probes=256,large_probes=1024,kss_probes=512),
                    branch_streams=dict(full=0,A=1,B=2))
    (work / "manifest.json").write_text(json.dumps(manifest,indent=2,sort_keys=True)+"\n")
    return manifest


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference",type=Path,required=True)
    parser.add_argument("--work",type=Path,required=True)
    args=parser.parse_args()
    stage(args.reference.resolve(),args.work.resolve())

if __name__ == "__main__":
    main()
