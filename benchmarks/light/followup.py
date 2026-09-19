"""Owner-requested continuation: bounded regressions in one fixed 12-hour bundle."""
import argparse
import os
from pathlib import Path
import shutil
import socket
import time

from common import ARMS, cell_id, read_json, sha256, write_json
from inputs import generate
from provenance import verify
from runner import fixture, measure, physical_cpus
from science import compare


def run(root):
    plan=read_json(root/"followup-plan.json")
    if plan["wall_seconds"]!=43200 or os.environ.get("NSLOTS")!="28":
        raise ValueError("follow-up resource contract changed")
    if "E5-2680 v4" not in Path("/proc/cpuinfo").read_text():
        raise ValueError("wrong node class")
    identity=verify(root)
    cpus=physical_cpus()
    os.environ["LIGHT_TASK_ID"]=plan["id"]
    began=float(os.environ.get("LIGHT_TASK_STARTED",time.time()))
    deadline=time.monotonic()+max(0,plan["wall_seconds"]-900-(time.time()-began))
    scratch=Path(os.environ["TMPDIR"])/("fereg-light-"+plan["id"])
    scratch.mkdir(mode=0o700,exist_ok=False)
    inventory=[]
    disabled=set()
    for block in plan["cases"]:
        c=block["cell"]
        data=scratch/(cell_id(c)+".dta")
        if c["private"]:
            source=block["input"]
            original=Path(source["path"])
            if sha256(original)!=source["sha256"] or sha256(original.with_suffix(".json"))!=source["metadata_sha256"]:
                raise ValueError("private follow-up input changed")
            shutil.copyfile(original,data)
            shutil.copyfile(original.with_suffix(".json"),data.with_suffix(".json"))
        else:
            generate(c,data)
        if sha256(data)!=block["input_sha256"]:
            raise ValueError("follow-up input differs from the original timed-out attempt")
        warm=scratch/(cell_id(c)+"-warm.dta")
        if c["private"]:
            fixture(c,warm)
        else:
            generate(dict(c,n=7680),warm)
        timeout=900 if c["model"]=="ppml" else 300
        for rep in range(1,block["repetitions"]+1):
            for threads in block["threads"] if rep%2 else list(reversed(block["threads"])):
                group=root/("private/results" if c["private"] else "results")/cell_id(c)/f"t{threads}-r{rep}"
                arms=block["arms"]
                offset=(rep-1)%len(arms)
                order=arms[offset:]+arms[:offset]
                if rep%2==0:
                    order.reverse()
                for arm in order:
                    key=(cell_id(c),threads,arm)
                    output=group/arm
                    if key in disabled or time.monotonic()+timeout+480>deadline:
                        status="skipped_after_failure" if key in disabled else "unattempted_deadline"
                        receipt=dict(cell=c,arm=arm,threads=threads,repetition=rep,status=status,
                                     reason="bounded owner-requested follow-up")
                        write_json(output/"result.json",receipt)
                    else:
                        receipt=measure(root,c,arm,threads,rep,data,warm,output,cpus,identity,deadline,timeout)
                        if receipt["status"]!="pass":
                            disabled.add(key)
                    inventory.append(dict(cell=cell_id(c),arm=arm,threads=threads,repetition=rep,status=receipt["status"]))
                    write_json(root/"task.json",dict(status="running",id=plan["id"],inventory=inventory))
                    print("LIGHT_FOLLOWUP_CALL",cell_id(c),threads,rep,arm,receipt["status"],flush=True)
                native=ARMS[c["model"]][0]
                comparisons={arm:compare(group/native,group/arm,c) for arm in arms if arm!=native} if native in arms else {}
                write_json(group/"comparisons.json",comparisons)
                for file in group.glob("*/state.bin"):
                    file.unlink()
        for path in (data,data.with_suffix(".json"),warm,warm.with_suffix(".json")):
            path.unlink()
    write_json(root/"task.json",dict(status="terminal",id=plan["id"],inventory=inventory,
        host=socket.gethostname(),job_id=os.environ.get("JOB_ID"),runtime_identity=identity))
    print("LIGHT_FOLLOWUP_COMPLETE",plan["id"],flush=True)


if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("root",type=Path)
    run(parser.parse_args().root.resolve())
