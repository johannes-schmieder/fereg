"""Inventory every original failed/ineligible comparison without reclassifying science.

The output is a dated evidence snapshot, not a mutable task tracker. Each row
retains its frozen CSV location and captured input/result receipt identity.
"""
import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import json
from pathlib import Path


def read(path):
    return list(csv.DictReader(path.open()))


def family(row, kind):
    arm = row.get("arm", row.get("comparator"))
    status = row["status"]
    if status in {"unsupported_surface", "budget_exclusion"}:
        return status
    if kind == "run":
        if arm in {"fereg", "feppml"}:
            if row["section"] == "zeros":
                return "native_sparse_zero"
            assert status == "command_timeout", row
            return "native_timeout"
        return "reference_execution"
    if status == "unavailable":
        return "unavailable_pair"
    if status == "pass":
        assert arm in {"greg", "gglm"}, row
        return "gtools_exact_sample_unavailable"
    assert status == "accuracy_failure", row
    checks = set(row["failed_checks"].split(";"))
    if row["model"] == "ols":
        return "ols_stopping_or_fit" if checks & {"coefficients", "rss"} else "ols_inference"
    if checks & {"sample_count", "sample_exact"}:
        return "ppml_sample"
    if "coefficients" in checks:
        return "ppml_coefficients"
    return "ppml_inference_or_objective"


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("captured",type=Path)
    parser.add_argument("output",type=Path)
    args=parser.parse_args()
    paths={name:args.captured/name for name in
           ("runs.csv","paired-ratios.csv","summary.csv","captured-sha256.csv","provenance.json")}
    hashes={r["path"]:r["sha256"] for r in read(paths["captured-sha256.csv"])}
    issues=defaultdict(list)
    totals=Counter()
    for kind,name in [("run","runs.csv"),("pair","paired-ratios.csv")]:
        rows=read(paths[name]); totals[kind+"_rows"]=len(rows)
        for line,row in enumerate(rows,2):
            excluded = row["status"] != "pass" if kind=="run" else row["eligible"] != "True"
            if not excluded:
                continue
            cell="{model}-{design}-n{n}-k{k}-t{threads}-{section}".format(**row)
            group=f"results/{cell}/{row['phase']}-{row['repetition']}"
            arm=row.get("arm",row.get("comparator"))
            receipts={}
            for path in (group+"/input.json",group+f"/{arm}/result.json"):
                if path in hashes:
                    receipts[path]=hashes[path]
            assert group+"/input.json" in receipts, group
            key=family(row,kind)
            issues[key].append(dict(kind=kind,csv=name,line=line,cell=cell,
                phase=row["phase"],repetition=int(row["repetition"]),arm=arm,
                status=row["status"],failed_checks=row.get("failed_checks", "").split(";")
                    if row.get("failed_checks") else [],reason=row.get("reason", ""),receipts=receipts))
            totals[kind+"_excluded"]+=1
    losses=[]
    for line,row in enumerate(read(paths["summary.csv"]),2):
        if row["comparator"] in {"fixest","fixest_strict"} and row["complete"]=="True" and float(row["ratio_median"])>1.2:
            losses.append(dict(csv="summary.csv",line=line,**row))
    result=dict(schema="fereg-original-issue-inventory-v1",
        source=json.loads(paths["provenance.json"].read_text())["candidate"],
        source_files={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()},
        counts=dict(totals),family_counts={k:len(v) for k,v in sorted(issues.items())},
        issues=dict(sorted(issues.items())),comparable_120_percent_breaches=losses,
        interpretation="Families organize symptoms, not proven root causes. An unavailable pair can also have a failed-run entry. No old failure is repaired or ranked by this inventory.")
    assert sum(result["family_counts"].values()) == totals["run_excluded"]+totals["pair_excluded"]
    args.output.parent.mkdir(parents=True,exist_ok=True)
    with args.output.open("x") as out:
        json.dump(result,out,indent=2);out.write("\n")
    print(json.dumps(dict(counts=result["counts"],families=result["family_counts"],performance_breaches=len(losses)),indent=2))


if __name__=="__main__":
    main()
