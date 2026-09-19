"""Allowlisted aggregate export; private row captures and licensed logs stay remote."""
import argparse
import csv
import math
from pathlib import Path

from common import ARMS, SOURCE, cell_id, manifest, read_json, sha256, supported, tasks, write_json

META = {"rc","N","rss","ll","loglik","deviance","converged","rank","df_a","df_r","df_resid","df","df_k",
        "stata_processors","processors_lic","processors_mach","stata_version","R_version","fixest_version",
        "pyfixest_version","effective_threads","covariance_multiplier"}


def collect(root):
    calls=[]
    for task in tasks():
        base=root/("private/results" if task["id"]==10 else "results")
        for c in task["cells"]:
            for threads in (4,28):
                for rep in (1,2,3):
                    group=base/cell_id(c)/f"t{threads}-r{rep}"
                    pairs=read_json(group/"comparisons.json") if (group/"comparisons.json").exists() else {}
                    for arm in ARMS[c["model"]]:
                        directory=group/arm
                        path=directory/"result.json"
                        call=dict(task=task["id"],cell=c,arm=arm,threads=threads,repetition=rep)
                        if not path.exists():
                            ok,reason=supported(c,arm)
                            call.update(status="unattempted" if ok else "unsupported",reason=reason)
                        else:
                            raw=read_json(path)
                            for name in ("status","reason","seconds","process_seconds","process_returncode","sentinel","memory",
                                         "input_sha256","runtime_identity","cpu_affinity","host","job_id","task_id","command_time_lower_bound","command_timeout_seconds"):
                                if name in raw:
                                    call[name]=raw[name]
                            call["metadata"]={k:v for k,v in raw.get("metadata",{}).items() if k in META}
                            if raw["status"]=="worker_failure":
                                error=raw.get("metadata",{}).get("error") or ""
                                call["failure_kind"]=("demeaning_iteration_limit" if "Demeaning failed after" in error
                                    else "convergence_failure" if "converg" in error.lower()
                                    else "estimator_return_code" if raw.get("metadata",{}).get("rc",0)!=0
                                    else "capture_or_worker_failure")
                            call["native_scalars"]={k:v for k,v in raw.get("metadata",{}).items()
                                if k.startswith(("fereg_","feppml_")) and isinstance(v,(int,float))}
                            matrices={}
                            for name in ("fereg_solver","fereg_runtime","feppml_solver","feppml_runtime"):
                                matrix=directory/(name+".csv")
                                if matrix.exists():
                                    values=[]
                                    for entry in csv.DictReader(matrix.open()):
                                        try:
                                            value=float(entry["value"])
                                        except ValueError:
                                            value=None
                                        values.append(dict(row=int(entry["row"]),column=entry["column"],
                                                           value=value if value is None or math.isfinite(value) else None))
                                    matrices[name]=values
                            call["native_matrices"]=matrices
                            call["receipt_sha256"]=sha256(path)
                            if (directory/"science.json").exists():
                                call["science"]=read_json(directory/"science.json")
                            if arm in pairs:
                                call["comparison"]=pairs[arm]
                        calls.append(call)
    runtime=read_json(root/"runtime.json") if (root/"runtime.json").exists() else {}
    artifact=runtime.get("files",{}).get("candidate/dist/fereg_linux.plugin")
    return dict(schema="fereg-light-aggregate-v1",source=SOURCE,artifact_sha256=artifact,
                collector_sha256=sha256(Path(__file__)),
                runtime_sha256=sha256(root/"runtime.json") if runtime else None,
                comparator_fingerprint=runtime.get("comparator_fingerprint"),
                startup=read_json(root/"STARTUP.SUCCESS") if (root/"STARTUP.SUCCESS").exists() else None,
                manifest=manifest(),calls=calls,
                tasks=[read_json(p) for p in sorted((root/"tasks").glob("*.json"))],
                accounting=[read_json(p) for p in sorted((root/"accounting").glob("*.json"))])


if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("root",type=Path)
    parser.add_argument("output",type=Path)
    parser.add_argument("--omit-short",action="store_true")
    parser.add_argument("--followup",type=Path)
    args=parser.parse_args()
    data=collect(args.root)
    if args.omit_short:
        data["omitted_calls"]=[x for x in data["calls"] if x["cell"]["design"]=="veneto_short"]
        data["calls"]=[x for x in data["calls"] if x["cell"]["design"]!="veneto_short"]
        data["scope_amendment"]="Short Veneto omitted at the owner's request; original unattempted receipts preserved."
    if args.followup:
        plan=read_json(args.followup/"followup-plan.json")
        keys={(cell_id(b["cell"]),threads,arm,rep) for b in plan["cases"] for threads in b["threads"]
              for arm in b["arms"] for rep in range(1,b["repetitions"]+1)}
        followup=collect(args.followup)
        followup["calls"]=[x for x in followup["calls"] if (cell_id(x["cell"]),x["threads"],x["arm"],x["repetition"]) in keys]
        public_plan=dict(plan,cases=[dict(b,input={k:v for k,v in b["input"].items() if k!="path"})
                                    if "input" in b else b for b in plan["cases"]])
        followup["manifest"]=public_plan
        followup["plan_sha256"]=sha256(args.followup/"followup-plan.json")
        followup["tasks"]=[read_json(args.followup/"task.json")]
        data["followup"]=followup
    write_json(args.output,data)
