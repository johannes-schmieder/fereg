"""Ten sequential SCC bundles; exact per-call receipts and bounded deadlines."""
import argparse
import csv
import hashlib
import math
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import time

import numpy as np
import pandas as pd
import psutil

from adapters import stata_request
from common import ARMS, HERE, SOURCE, arm_order, cell_id, manifest, read_json, sha256, supported, tasks, thread_environment, write_json
from inputs import generate, store
from memory import Observer
from science import compare


def metadata(path):
    if (path/"metadata.json").exists():
        return read_json(path/"metadata.json")
    value = {}
    if (path/"metadata.tsv").exists():
        with (path/"metadata.tsv").open() as stream:
            for key, item in csv.reader(stream, delimiter="\t"):
                try:
                    number = float(item)
                    value[key] = number if math.isfinite(number) else None
                except ValueError:
                    value[key] = item
    return value


def stop_process(process):
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


def physical_cpus():
    selected = {}
    for cpu in sorted(os.sched_getaffinity(0)):
        p = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        key = ((p/"physical_package_id").read_text().strip(), (p/"core_id").read_text().strip())
        selected.setdefault(key,cpu)
    if len(selected)!=28:
        raise RuntimeError("benchmark requires 28 allocated physical cores")
    return [selected[k] for k in sorted(selected)]


def import_audit(output, data):
    info = read_json(data.with_suffix(".json"))
    hashes = {}
    with (output/"import.bin").open("rb") as stream:
        for column in info["columns"]:
            digest = hashlib.sha256()
            remaining = info["n"]*8
            while remaining:
                block = stream.read(min(remaining,2**20))
                if not block:
                    raise ValueError("truncated import capture")
                digest.update(block)
                remaining -= len(block)
            hashes[column] = digest.hexdigest()
        if stream.read(1):
            raise ValueError("extra import capture bytes")
    if hashes != info["column_sha256"]:
        raise ValueError("Stata input values differ")
    (output/"import.bin").unlink()
    write_json(output/"import-hashes.json",hashes)
    (output/"IMPORT.CHECKED").write_text("PASS\n")


def measure(root,c,arm,threads,rep,data,warm,output,cpus,identity,deadline,command_timeout=300):
    output.mkdir(parents=True,exist_ok=False)
    cfg = dict(cell=c,arm=arm,threads=threads,repetition=rep,data=str(data),warm=str(warm),output=str(output),
               source=str(root/"candidate"),refs=str(root/"references"),
               inputvars=" ".join(read_json(data.with_suffix(".json"))["columns"]))
    write_json(output/"request.json",cfg)
    env = os.environ.copy()
    for key in list(env):
        if key.startswith("FEREG_") or key in ("PYTHONPATH","LD_PRELOAD"):
            del env[key]
    env.update(thread_environment(threads))
    temporary = data.parent/f"tmp-{arm}-t{threads}-r{rep}"
    temporary.mkdir(exist_ok=False)
    env.update(R_LIBS_USER=str(root/"env/R"),TMPDIR=str(temporary),PYTHONDONTWRITEBYTECODE="1",MPLCONFIGDIR=str(temporary/"mpl"))
    if arm == "fixest":
        command = ["Rscript",str(HERE/"r_worker.R"),str(output/"request.json")]
    elif arm == "pyfixest":
        command = [str(root/"env/python/bin/python"),str(HERE/"python_worker.py"),str(output/"request.json")]
    else:
        (output/"request.do").write_text(stata_request(cfg))
        command = ["stata-mp","-b","do",str(HERE/"stata_worker.do"),str(output/"request.do")]
    prefix = ["taskset","-c",",".join(map(str,cpus[:threads]))]
    start = time.monotonic()
    begun, finished, reason = None,None,None
    observer = None
    with (output/"stdout.txt").open("w") as stdout,(output/"stderr.txt").open("w") as stderr:
        process = subprocess.Popen(prefix+command,cwd=output,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
        observer = Observer(process.pid,output,.02 if not c["private"] and c["n"]<=400000 else .05,cpus[:threads])
        observer.start()
        try:
            while process.poll() is None:
                now = time.monotonic()
                if (output/"INPUT.READY").exists() and not (output/"IMPORT.CHECKED").exists():
                    import_audit(output,data)
                if (output/"START.CHECKED").exists() and begun is None:
                    begun = now
                if (output/"DONE").exists() and finished is None:
                    finished = now
                if observer.error:
                    reason = "memory_observer_failure"
                elif now>=deadline:
                    reason = "task_deadline"
                elif begun is None and now-start>180:
                    reason = "startup_timeout"
                elif begun is not None and finished is None and now-begun>command_timeout:
                    reason = "command_timeout"
                elif finished is not None and now-finished>180:
                    reason = "extraction_timeout"
                if reason:
                    stop_process(process)
                    break
                time.sleep(.01)
        finally:
            stop_process(process)
            memory = observer.finish()
    meta = metadata(output)
    sentinel = any("LIGHT_WORKER_COMPLETE rc=0" in p.read_text(errors="replace")
                   for p in [output/"stdout.txt",*output.glob("*.log")])
    passed = process.returncode==0 and meta.get("rc")==0 and sentinel and (output/"state.bin").exists()
    status = reason or ("pass" if passed else "worker_failure")
    result = dict(schema="fereg-light-arm-v1",cell=c,arm=arm,threads=threads,repetition=rep,
                  status=status,metadata=meta,seconds=meta.get("seconds"),process_seconds=time.monotonic()-start,
                  process_returncode=process.returncode,sentinel=sentinel,memory=memory,
                  input_sha256=sha256(data),runtime_identity=identity,cpu_affinity=cpus[:threads],
                  host=socket.gethostname(),job_id=os.environ.get("JOB_ID"),task_id=os.environ.get("LIGHT_TASK_ID"),
                  command_timeout_seconds=command_timeout,
                  command_time_lower_bound=command_timeout if reason=="command_timeout" else None)
    write_json(output/"result.json",result)
    if status=="pass":
        # Independent numerical work runs after DONE and outside memory/timing.
        with (output/"audit.log").open("w") as log:
            audit = subprocess.Popen([str(root/"env/python/bin/python"),str(HERE/"science.py"),str(output/"request.json")],env=env,stdout=log,stderr=log,start_new_session=True)
            try:
                audit.wait(timeout=max(1,min(120,deadline-time.monotonic())))
            except subprocess.TimeoutExpired:
                stop_process(audit)
                partial=read_json(output/"science.json") if (output/"science.json").exists() else {}
                partial.update(status="indeterminate",reason="independent audit time budget")
                write_json(output/"science.json",partial)
        if not (output/"science.json").exists():
            write_json(output/"science.json",dict(status="indeterminate",reason="independent audit failed",returncode=audit.returncode))
    shutil.rmtree(temporary)
    return result


def fixture(c,path):
    n = 360
    i = np.arange(n)
    rng = np.random.default_rng(104729)
    frame = pd.DataFrame(dict(obs_id=i+1,g1=i//6+1,g2=(i//6+7*(i%6))%15+1))
    for j in range(1,c["k"]+1):
        frame[f"x{j}"] = rng.normal(size=n)
    eta = .3*frame.x1-.1*frame.x2 + .12*np.sin(frame.g1)+.1*np.cos(frame.g2)
    if c["slopes"]:
        frame["z"] = rng.normal(size=n)
        eta += .08*np.sin(frame.g2)*frame.z
    frame["y"] = eta+rng.normal(size=n)*.25 if c["model"]=="ols" else 1+rng.poisson(np.exp(eta))
    cc = dict(c,n=n,effects=["g1","g2"],private=False)
    return store(frame,path,cc,{"kind":"small independent startup fixture"}),cc


def startup(root,cpus,identity,scratch,deadline):
    from oracle import check_fixture
    from common import cell
    cases = [dict(cell("strong_d3",360,"startup",k=2,save=True),model=m) for m in ARMS]
    cases += [dict(cell("strong_d3",360,"startup-cluster",k=2,cluster=("g1","g2")),model=m) for m in ARMS]
    cases += [dict(cell("strong_d3",360,"startup-slope",k=2,slopes=True,save=True),model=m) for m in ARMS]
    findings=[]
    for c in cases:
        data = scratch/(cell_id(c)+".dta")
        _,c=fixture(c,data)
        for arm in ARMS[c["model"]]:
            if not supported(c,arm)[0]:
                continue
            output=root/"startup"/cell_id(c)/arm
            result=measure(root,c,arm,4,1,data,data,output,cpus,identity,deadline)
            check=check_fixture(data,c,output) if result["status"]=="pass" else dict(status="worker_failure")
            findings.append(dict(cell=cell_id(c),arm=arm,worker=result["status"],oracle=check))
            write_json(root/"startup/results.json",findings)
    # Comparator disagreements stay failed scientific checks. They do not
    # approve a speed ratio or require changing the default solver tolerance.
    failed=[x for x in findings if x["worker"]!="pass" or
            (x["arm"] in ("fereg","feppml") and x["oracle"]["status"]!="pass")]
    if failed:
        raise RuntimeError("startup failures: "+", ".join(x["cell"]+"/"+x["arm"] for x in failed))
    write_json(root/"STARTUP.SUCCESS",dict(identity=identity,checks=len(findings),job_id=os.environ.get("JOB_ID"),
        comparator_diagnostics=[x for x in findings if x["oracle"]["status"]!="pass"],
        policy="native oracles pass; comparator discrepancies retain failed scientific eligibility"))


def run(root,task_id):
    from provenance import verify
    config=manifest()
    task=tasks()[task_id-1]
    if os.environ.get("NSLOTS")!="28" or "E5-2680 v4" not in Path("/proc/cpuinfo").read_text():
        raise RuntimeError("wrong allocated node class")
    os.environ["LIGHT_TASK_ID"]=str(task_id)
    cpus=physical_cpus()
    identity=verify(root)
    begin=float(os.environ.get("LIGHT_TASK_STARTED",time.time()))
    deadline=time.monotonic()+max(0,43200-900-(time.time()-begin))
    scratch=Path(os.environ["TMPDIR"])/f"fereg-light-{task_id}"
    scratch.mkdir(mode=0o700,exist_ok=False)
    if task_id==1:
        startup(root,cpus,identity,scratch,deadline)
    elif not (root/"STARTUP.SUCCESS").exists():
        raise RuntimeError("startup receipt missing")
    output_root=root/("private/results" if task_id==10 else "results")
    inventory=[]
    disabled=set()
    for c in task["cells"]:
        data=scratch/(cell_id(c)+".dta")
        available=True
        if c["private"]:
            source=read_json(root/"private/inputs.json").get(c["design"],{}).get(c["model"])
            available=source is not None
            if available:
                original=Path(source["path"])
                if sha256(original)!=source["sha256"]:
                    raise ValueError("private input identity changed")
                shutil.copyfile(original,data)
                shutil.copyfile(original.with_suffix(".json"),data.with_suffix(".json"))
        else:
            generate(c,data)
        warm=scratch/(cell_id(c)+"-warm.dta")
        if c["private"] and available:
            fixture(c,warm)
        elif available:
            generate(dict(c,n=7680),warm)
        for rep in (1,2,3):
            for threads in ((4,28) if rep%2 else (28,4)):
                group=output_root/cell_id(c)/f"t{threads}-r{rep}"
                for arm in arm_order(c["model"],rep):
                    output=group/arm
                    key=(cell_id(c),threads,arm)
                    eligible,why=supported(c,arm)
                    if not eligible or not available or key in disabled or time.monotonic()+660>deadline:
                        status=("unsupported" if not eligible else "unattempted_input_unavailable" if not available
                                else "skipped_after_failure" if key in disabled else "unattempted_deadline")
                        output.mkdir(parents=True,exist_ok=True)
                        receipt=dict(cell=c,arm=arm,threads=threads,repetition=rep,status=status,
                                     reason=why if available else "verified raw private input unavailable")
                        write_json(output/"result.json",receipt)
                    else:
                        receipt=measure(root,c,arm,threads,rep,data,warm,output,cpus,identity,deadline)
                        if receipt["status"]!="pass":
                            disabled.add(key)
                    inventory.append(dict(cell=cell_id(c),arm=arm,threads=threads,repetition=rep,status=receipt["status"]))
                    write_json(root/"tasks"/f"{task_id}.json",dict(task=task_id,job_id=os.environ.get("JOB_ID"),status="running",inventory=inventory))
                    print("LIGHT_CALL",cell_id(c),threads,rep,arm,receipt["status"],flush=True)
                candidate="fereg" if c["model"]=="ols" else "feppml"
                comparisons={arm:compare(group/candidate,group/arm,c) for arm in ARMS[c["model"]] if arm!=candidate}
                write_json(group/"comparisons.json",comparisons)
                # Row-level captures are temporary; private diagnostics never leave owner-only storage.
                for file in group.glob("*/state.bin"):
                    file.unlink()
        for p in (data,data.with_suffix(".json"),warm,warm.with_suffix(".json")):
            p.unlink(missing_ok=True)
    write_json(root/"tasks"/f"{task_id}.json",dict(task=task_id,job_id=os.environ.get("JOB_ID"),status="terminal",inventory=inventory,host=socket.gethostname(),runtime_identity=identity))
    print("LIGHT_TASK_COMPLETE",task_id,flush=True)


if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("root",type=Path)
    p.add_argument("task",type=int,choices=range(1,11))
    args=p.parse_args()
    run(args.root.resolve(),args.task)
