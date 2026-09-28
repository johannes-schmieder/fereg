"""Linux perf attribution on immutable completed diagnostic fit scripts.

This never emits a headline timing rank. Optional passive-wait diagnostics
separate embedding worker activity from native numerical work.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def write(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def run(args):
    args.output.mkdir(exist_ok=False)
    records = []
    for threads, passive in ((1, False), (8, False), (8, True)):
        name = f"t{threads}-" + ("passive" if passive else "default")
        previous = args.diagnostics / f"two-dense-larger-t{threads}" / "candidate"
        output = args.output / name
        output.mkdir()
        script = previous / "fit.do"
        (output / "fit.do").write_text(script.read_text().replace(str(previous), str(output)))
        env = {k: v for k, v in os.environ.items() if not k.startswith("FEREG_")}
        env.update(OPENBLAS_NUM_THREADS="1", OMP_NUM_THREADS=str(threads),
                   MKL_NUM_THREADS="1", NUMEXPR_NUM_THREADS="1")
        if passive:
            env.update(OMP_WAIT_POLICY="PASSIVE", KMP_BLOCKTIME="0")
        allocated = sorted(os.sched_getaffinity(0))
        if len(allocated) < threads or int(env.get("NSLOTS", 0)) < threads:
            raise ValueError("profile exceeds allocation")
        command = ["perf", "record", "-F", "499", "-e", "cycles:u", "-g", "-o", str(output / "perf.data"),
                   "--", "taskset", "-c", ",".join(map(str, allocated[:threads])),
                   "stata-mp", "-b", "do", str(output / "fit.do")]
        started = time.monotonic()
        with (output / "stdout.txt").open("w") as stdout, (output / "stderr.txt").open("w") as stderr:
            child = subprocess.Popen(command, cwd=output, env=env, stdout=stdout,
                                     stderr=stderr, start_new_session=True)
            timeout = False
            try:
                child.wait(timeout=120)
            except subprocess.TimeoutExpired:
                timeout = True
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
        log = output / "fit.log"
        success = child.returncode == 0 and not timeout and log.exists() and any(
            line.strip() == "OLS_SLOPE_PROBE_SUCCESS" for line in log.read_text(errors="replace").splitlines())
        record = dict(case=name, diagnostic=True, timed_out=timeout, process_rc=child.returncode,
                      sentinel=success, elapsed_seconds=time.monotonic()-started,
                      parent_script_sha256=hashlib.sha256(script.read_bytes()).hexdigest(),
                      environment={k:env[k] for k in ("OMP_NUM_THREADS", "OMP_WAIT_POLICY", "KMP_BLOCKTIME") if k in env})
        if success:
            report = subprocess.run(["perf", "report", "--stdio", "--no-children", "--percent-limit", "0.5",
                                     "-s", "dso,symbol", "-i", str(output / "perf.data")],
                                    capture_output=True, text=True, timeout=60)
            record.update(report_rc=report.returncode, report=report.stdout, report_error=report.stderr)
        else:
            record["error"] = (output / "stderr.txt").read_text(errors="replace")
        records.append(record)
        write(args.output / "results.json", records)
        print(name, success, flush=True)
        if not success:
            raise RuntimeError("profile failed; preserve the failure without rerunning fits")
    write(args.output / "complete.json", dict(complete=True, cases=len(records)))
    print("FEREG_SLOPE_PERF_COMPLETE", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--diagnostics", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    run(parser.parse_args())
