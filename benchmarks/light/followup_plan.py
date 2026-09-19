"""Select only original PPML command timeouts and paired native controls."""
import argparse
from pathlib import Path

from common import ARMS, cell_id, read_json, sha256, write_json
from provenance import verify


def select_cases(records):
    groups={}
    for receipt,path in records:
        if receipt["status"]!="command_timeout" or receipt["cell"]["model"]!="ppml":
            continue
        c=receipt["cell"]
        if c["design"]=="veneto_short":
            continue
        key=(cell_id(c),receipt["threads"])
        block=groups.setdefault(key,dict(cell=c,threads=[receipt["threads"]],arms=["feppml"],
                                       repetitions=3,input_sha256=receipt["input_sha256"],original_timeouts=[]))
        if receipt["input_sha256"]!=block["input_sha256"]:
            raise ValueError("paired input mismatch")
        if receipt["arm"] not in block["arms"]:
            block["arms"].append(receipt["arm"])
        block["original_timeouts"].append(dict(arm=receipt["arm"],repetition=receipt["repetition"],
                                                receipt=path,command_time_lower_bound=receipt.get("command_time_lower_bound")))
    for block in groups.values():
        block["arms"].sort(key=ARMS["ppml"].index)
    # Native timeouts are the first unresolved performance questions.
    return sorted(groups.values(),key=lambda b:(not any(x["arm"]=="feppml" for x in b["original_timeouts"]),
                                                cell_id(b["cell"]),b["threads"]))


def make(root,parent):
    parent_identity=verify(parent)
    expected=read_json(root/"continuation.json")
    if parent_identity!=expected["parent_runtime_sha256"]:
        raise ValueError("parent runtime differs")
    for i in range(1,11):
        task=read_json(parent/"tasks"/f"{i}.json")
        if task["status"]!="terminal" or len(task["inventory"])!=120:
            raise ValueError("original task inventory is incomplete")
    records=[]
    hashes={}
    for base in (parent/"results",parent/"private/results"):
        for path in sorted(base.glob("*/t*-r*/*/result.json")):
            receipt=read_json(path)
            if receipt["status"]=="command_timeout" and receipt["cell"]["model"]=="ppml":
                name=str(path.relative_to(parent))
                records.append((receipt,name));hashes[name]=sha256(path)
    cases=select_cases(records)
    inputs=read_json(parent/"private/inputs.json")
    for block in cases:
        c=block["cell"]
        if c["private"]:
            block["input"]=dict(inputs[c["design"]][c["model"]])
            source=Path(block["input"]["path"])
            block["input"]["metadata_sha256"]=sha256(source.with_suffix(".json"))
            if block["input"]["sha256"]!=block["input_sha256"]:
                raise ValueError("private input differs from original timed-out attempt")
    plan=dict(schema="fereg-light-followup-v1",id="ppml-15min",wall_seconds=43200,
              command_timeout_seconds=900,parent_runtime_sha256=parent_identity,
              original_timeout_receipts=hashes,cases=cases,
              max_fits=sum(len(b["arms"])*len(b["threads"])*b["repetitions"] for b in cases),
              purpose="owner-requested longer PPML allowance; only timed-out configurations and paired native controls",
              stop_rule="no increase beyond 15 minutes; skip remaining repetitions after an execution failure")
    write_json(root/"followup-plan.json",plan)
    print("LIGHT_FOLLOWUP_PLAN",len(cases),plan["max_fits"],flush=True)


if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("root",type=Path)
    parser.add_argument("parent",type=Path)
    args=parser.parse_args()
    make(args.root.resolve(),args.parent.resolve())
