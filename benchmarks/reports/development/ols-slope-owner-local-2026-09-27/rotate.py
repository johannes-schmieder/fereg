"""Replay the frozen fresh block on a quiet Mac, with stock arms kept separate."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

import probe


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def competing(group=None):
    processes = subprocess.check_output(["ps", "-axo", "pgid=,comm="], text=True)
    forbidden = {"rustc", "cargo", "clang", "clang++", "gcc", "g++", "stata-mp", "R"}
    return [line.strip() for line in processes.splitlines()
            if line.split() and int(line.split()[0])!=group
            and Path(line.split(None, 1)[1]).name in forbidden]


def quiet():
    busy=competing()
    if busy:
        raise RuntimeError("Competing compute before timed arm: " + repr(busy))


def main():
    report = Path(__file__).resolve().parent
    root = Path(sys.argv[1]).resolve()
    output = root / "fresh04"
    candidate = json.loads((report / "candidate02-source.json").read_text())
    baseline = json.loads((report / "baseline-source.json").read_text())
    reference = json.loads((report.parent / "ols-comparator-adjudication-local-2026-09-27/reference-build.json").read_text())
    for name, digest in candidate["files"].items():
        assert sha(probe.REPO / name) == digest, name
    assert sha(probe.REPO / "dist/fereg_macos.plugin") == candidate["plugin_sha256"]
    assert sha(root / "baseline/dist/fereg_macos.plugin") == baseline["plugin_sha256"]
    for name, digest in reference["installed_files"].items():
        assert sha(probe.R_STOCK / "fixest" / name) == digest, name
    registration = json.loads((report / "fresh-registration.json").read_text())
    consecutive_quiet = 0
    for _ in range(90):
        busy = competing()
        consecutive_quiet = 0 if busy else consecutive_quiet + 1
        if consecutive_quiet == 3:
            break
        print("Waiting for three quiet preflight samples; competing:", busy, flush=True)
        time.sleep(10)
    else:
        raise RuntimeError("Machine remained busy; no timed fits started")
    output.mkdir(exist_ok=False)
    identity = dict(candidate=candidate["plugin_sha256"], baseline=baseline["plugin_sha256"],
                    stock=reference["installed_files"]["libs/fixest.so"],
                    controllers={p.name:sha(p) for p in [Path(__file__), report / "probe.py"]},
                    registration=sha(report / "fresh-registration.json"))
    (output / "identity.json").write_text(json.dumps(identity,indent=2)+"\n")
    # The interrupted preflight generated these bytes but ran no fits. Preserve
    # them literally rather than regenerating supposedly identical inputs.
    inputs=[]
    for seed in registration["seeds"]:
        for case in registration["cases"]:
            data=root / "fresh02/inputs" / str(seed) / case["id"]
            info=json.loads((data / "input.json").read_text())
            manifest=json.loads((probe.REPO / "benchmarks/systematic/manifest-light-v2.json").read_text())
            cell=dict(case,model="ols",section="slopes",effects=manifest["designs"][case["design"]]["effects"],
                      slopes={"g1":["z1"],"g2":["z2"]})
            assert info["seed"]==seed and info["n"]==case["n"] and info["k"]==case["k"]
            assert sha(data / "input.dta")==info["sha256"]
            inputs.append((seed,cell,info,data))
    records=[]
    for rep in range(registration["rotations_per_seed"]):
        for seed,cell,info,data in inputs:
            arms=["baseline","candidate02","default","strict"]
            if rep%2: arms.reverse()
            for arm in arms:
                quiet()
                directory=output / str(seed) / cell["id"] / str(rep) / arm
                result=probe.run_case(cell,data,directory,
                    "native" if arm in ("baseline","candidate02") else arm,
                    root / "baseline/dist" if arm=="baseline" else probe.REPO / "dist", monitor=competing)
                records.append(dict(seed=seed,case=cell["id"],repetition=rep,arm=arm,
                                    input_sha256=info["sha256"],result=result))
                (output / "results.json").write_text(json.dumps(records,indent=2,allow_nan=False)+"\n")
                print(seed,cell["id"],rep,arm,result.get("metadata",result),flush=True)
                if result["competing_work"]:
                    raise RuntimeError("Timed attempt retained with competing-work evidence")
    (output / "complete.json").write_text(json.dumps(dict(completed=True, fits=len(records)),indent=2)+"\n")


if __name__ == "__main__":
    main()
