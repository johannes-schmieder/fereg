"""Bounded public-command diagnostics; these clocks are never headline timings.

Each attempt owns a fresh Stata process, log, trace and output directory. The
frozen systematic generator supplies exact original observations. Failed fits
are evidence, not reasons to repeat the attempt automatically.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "systematic"))
from common import read_json, sha256, thread_environment, write_json
from generate import generate


def bounded_environment(case, output, inherited, available_bytes=None):
    env = {key: value for key, value in inherited.items() if not key.startswith("FEREG_")}
    env.update(thread_environment(case["cell"]["threads"]))
    env.update(FEREG_SOLVER_TRACE="1", FEREG_SOLVER_TRACE_FILE=str(output / "solver.trace"))
    if case.get("phase_timing", False):
        env["FEREG_PHASE_TIMING"] = "1"
    limit = case.get("memory_limit_bytes")
    if limit is not None:
        # The developer seam must only restrict the real available-memory frame.
        # A large admission forecast is not authority to invent physical memory.
        if type(limit) is not int or limit <= 0 or available_bytes is None or limit > available_bytes:
            raise ValueError("diagnostic memory limit must be positive and within current available memory")
        env["FEREG_MEMORY_LIMIT_BYTES"] = str(limit)
    return env


def available_linux_memory():
    for line in Path("/proc/meminfo").read_text().splitlines():
        fields = line.split()
        if fields[0] == "MemAvailable:" and fields[2:] == ["kB"]:
            return int(fields[1]) * 1024
    raise ValueError("Linux MemAvailable is missing")


def wait_bounded(child, timeout, rss_limit=None):
    """Diagnostic-only safety monitor; never produces eligible timing evidence."""
    if rss_limit is None:
        try:
            child.wait(timeout=timeout)
            return None, None
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
            return "process_timeout", None
    if type(rss_limit) is not int or rss_limit <= 0:
        raise ValueError("RSS limit must be a positive integer")
    started, peak, reason = time.monotonic(), 0, None
    try:
        import psutil
        parent = psutil.Process(child.pid)
        while child.poll() is None:
            try:
                rss = 0
                for process in [parent, *parent.children(recursive=True)]:
                    try:
                        rss += process.memory_info().rss
                    except psutil.NoSuchProcess:
                        pass
                peak = max(peak, rss)
            except psutil.NoSuchProcess:
                pass
            if peak > rss_limit:
                reason = "rss_limit"
            elif time.monotonic() - started > timeout:
                reason = "process_timeout"
            if reason:
                break
            time.sleep(.1)
    finally:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGKILL)
        child.wait()
    return reason, peak


def stata_script(source, refs, data, output, case):
    cell = case["cell"]
    absorption = " ".join(e + ("##c.(" + " ".join(cell["slopes"][e]) + ")"
                               if e in cell.get("slopes", {}) else "")
                          for e in cell["effects"])
    arm = "fereg" if cell["model"] == "ols" else "feppml"
    options = f"absorb({absorption}) threads({cell['threads']}) solver({case.get('solver', 'auto')})"
    if arm == "feppml":
        options += " vce(robust)"
    if "memory" in cell:
        options += f" memory({cell['memory']})"
    command = f"{arm} y " + " ".join(f"x{i}" for i in range(1, cell["k"] + 1)) + ", " + options
    return f'''version 19.0
clear all
set more off
set varabbrev off
set linesize 255
set processors `=min({cell['threads']},c(processors_lic),c(processors_mach))'
sysdir set PLUS "{refs}"
sysdir set PERSONAL "{output}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{source}/stata"
adopath ++ "{source}/dist"
display "PERFORMANCE_STAGE loading"
use "{data}", clear
display "PERFORMANCE_STAGE estimation"
timer on 91
capture noisily {command}
local fit_rc = _rc
timer off 91
quietly timer list 91
local elapsed = r(t91)
tempname meta
file open `meta' using "{output}/metadata.tsv", write text
file write `meta' "rc" _tab "`fit_rc'" _n "diagnostic_seconds" _tab %24.17g (`elapsed') _n
file write `meta' "requested_threads" _tab "{cell['threads']}" _n
file write `meta' "stata_processors" _tab "`c(processors)'" _n
if !`fit_rc' {{
    display "PERFORMANCE_STAGE extraction"
    local scalars : e(scalars)
    foreach name of local scalars {{
        file write `meta' "`name'" _tab %24.17g (e(`name')) _n
    }}
    matrix list e(b), format(%24.17g)
    matrix list e(V), format(%24.17g)
    gen byte audit_sample = e(sample)
    keep obs_id audit_sample
    export delimited using "{output}/sample.csv", replace
}}
file close `meta'
display "PERFORMANCE_COMPLETE rc=`fit_rc'"
exit 0
'''


def run(args):
    plan = read_json(args.plan)
    args.output.mkdir(parents=True, exist_ok=False)
    write_json(args.output / "plan.json", plan)
    plugin = args.source / "dist/fereg_linux.plugin"
    assert sha256(plugin) == plan["plugin_sha256"], "plugin identity differs"
    for name, digest in read_json(args.source_manifest)["files"].items():
        assert sha256(args.source / name) == digest, name
    manifest = read_json(args.manifest)
    records = []
    for case in plan["cases"]:
        output = args.output / case["id"]
        output.mkdir()
        data = args.scratch / (case["id"] + ".dta")
        info = generate(case["cell"], case["seed"], data, manifest)
        write_json(output / "input.json", info)
        if "expected_input_sha256" in case:
            assert info["sha256"] == case["expected_input_sha256"], "regenerated input differs"
        script = output / "fit.do"
        script.write_text(stata_script(args.source, args.refs, data, output, case))
        available = available_linux_memory() if "memory_limit_bytes" in case else None
        env = bounded_environment(case, output, os.environ, available)
        rss_limit = case.get("rss_limit_bytes")
        if rss_limit is not None and (type(rss_limit) is not int or rss_limit <= 0):
            raise ValueError("RSS limit must be a positive integer")
        cpus = sorted(os.sched_getaffinity(0))[:case["cell"]["threads"]]
        assert len(cpus) == case["cell"]["threads"], "insufficient allocated cores"
        command = ["taskset", "-c", ",".join(map(str, cpus)), "stata-mp", "-b", "do", str(script)]
        started = time.monotonic()
        with (output / "stdout.txt").open("w") as stdout, (output / "stderr.txt").open("w") as stderr:
            child = subprocess.Popen(command, cwd=output, env=env, stdout=stdout,
                                     stderr=stderr, start_new_session=True)
            stop_reason, peak_rss = wait_bounded(child, case["timeout_seconds"], rss_limit)
        log = (output / "fit.log").read_text(errors="replace") if (output / "fit.log").exists() else ""
        record = dict(case=case, input_sha256=info["sha256"], process_rc=child.returncode,
                      timed_out=stop_reason == "process_timeout", stop_reason=stop_reason,
                      elapsed_seconds=time.monotonic() - started,
                      available_bytes_before=available, peak_process_tree_rss_bytes=peak_rss,
                      performance_eligible=False, cpu_affinity=cpus,
                      sentinel=child.returncode == 0 and stop_reason is None and "PERFORMANCE_COMPLETE rc=0" in log,
                      last_stage=next((line.strip() for line in reversed(log.splitlines())
                                       if line.startswith("PERFORMANCE_STAGE ")), None),
                      files={p.name: sha256(p) for p in output.iterdir() if p.is_file()})
        write_json(output / "receipt.json", record)
        records.append(record)
        write_json(args.output / "progress.json", records)
        print(json.dumps({k: record[k] for k in ("case", "timed_out", "sentinel", "elapsed_seconds")}), flush=True)
        data.unlink()
        data.with_suffix(".json").unlink()
    write_json(args.output / "complete.json", dict(completed=True, cases=len(records),
                                                  all_fits_passed=all(r["sentinel"] for r in records)))
    print("PERFORMANCE_DIAGNOSTICS_COMPLETE", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("source", "refs", "manifest", "source-manifest", "plan", "output", "scratch"):
        parser.add_argument("--" + name, type=Path, required=True)
    run(parser.parse_args())
