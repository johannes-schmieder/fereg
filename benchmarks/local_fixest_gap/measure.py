"""Sequential, fresh-process, same-node comparison groups with RSS sampling."""
from __future__ import annotations

import argparse
import csv
import math
import os
import platform
import signal
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
import psutil

from common import HERE, read_json, sha256, thread_environment, write_json


def numeric(value):
    try:
        number = float(value)
        return number if math.isfinite(number) else None
    except (ValueError, TypeError):
        return value


def metadata(output):
    if (output / "metadata.json").exists():
        return read_json(output / "metadata.json")
    if (output / "metadata.tsv").exists():
        with (output / "metadata.tsv").open() as f:
            return {k: numeric(v.strip()) for k, v in csv.reader(f, delimiter="\t")}
    return {}


def sample_hash(path):
    digest = __import__("hashlib").sha256()
    with path.open() as f:
        reader = csv.DictReader(f)
        rows = 0
        for row in reader:
            digest.update(int(float(row["obs_id"])).to_bytes(8, "little"))
            rows += 1
    return digest.hexdigest(), rows


def measure(root, cell, arm, phase, data, warm, output, manifest, identity=None, source=None, trace=False, solver="auto"):
    # Fail before launching a worker if sandbox policy prevents RSS sampling.
    psutil.pids()
    output.mkdir(parents=True, exist_ok=False)
    threads = cell["threads"]
    config = {"output": str(output), "cell": cell, "data": str(data), "warm": str(warm), "phase": phase, "trace": trace, "solver": solver, "source": str(source) if source else None, "block_width": os.environ.get("FEREG_LOCAL_BLOCK_WIDTH", "1")}
    write_json(output / "request.json", config)
    environment = os.environ.copy()
    # Prevent inherited user/developer overrides from changing the estimator.
    for key in list(environment):
        if key.startswith("FEREG_") or key in ("PYTHONPATH", "LD_PRELOAD"):
            del environment[key]
    environment.update(thread_environment(threads))
    if trace:
        environment["FEREG_PHASE_TIMING"] = "1"
        environment["FEREG_LOCAL_TRACE"] = "1"
    environment["FEREG_LOCAL_BLOCK_WIDTH"] = os.environ.get("FEREG_LOCAL_BLOCK_WIDTH", "1")
    environment["FEREG_LOCAL_POOL"] = os.environ.get("FEREG_LOCAL_POOL", "10")
    environment["FEREG_LOCAL_SOLVER"] = solver
    temporary = data.parent / f"worker-{arm}"
    temporary.mkdir(exist_ok=False)
    environment.update({"R_LIBS_USER": os.environ.get("R_LIBS_USER", str(root / "work/R")), "MPLCONFIGDIR": str(root / "work/mpl"),
                        "TMPDIR": str(temporary), "PYTHONDONTWRITEBYTECODE": "1"})
    # macOS chooses the physical cores; limits are native/Stata thread requests.
    cpu_ids = None  # macOS has no supported process CPU-affinity control.
    prefix = []
    import_path = temporary / "import.bin"
    if arm == "fixest":
        command = ["Rscript", str(HERE / "r_worker.R"), str(output / "request.json")]
    elif arm == "pyfixest":
        command = [str(root / "work/python/bin/python"), str(HERE / "python_worker.py"), str(output / "request.json")]
    else:
        # Stata reads do-files incrementally. Bind each run to its own copy so
        # concurrent harness edits cannot change an active command's script.
        worker = output / "stata_worker.do"
        shutil.copyfile(HERE / "stata_worker.do", worker)
        command = [os.environ["STATA_MP_PATH"], "-b", "do", str(worker),
                   str(source or root / "work/candidate"), str(root / "work/refs"), str(data), str(warm), str(output),
                   arm, cell["model"], str(cell["k"]), str(threads), str(len(cell["effects"])), phase, str(import_path)]
        command.extend('"' + str(cell.get(key, "")) + '"' for key in
                       ("weight", "vce", "absorb", "restriction", "fit_options"))
    maximum_rss = 0
    samples = 0
    peaks_threads = 0
    started = None
    reason = None
    wall_start = time.monotonic()
    load_at_launch = list(os.getloadavg())
    import_verified = False
    with (output / "stdout.txt").open("w") as stdout, (output / "stderr.txt").open("w") as stderr:
        process = subprocess.Popen(prefix + command, cwd=output, env=environment,
                                   stdout=stdout, stderr=stderr, start_new_session=True)
        while process.poll() is None:
            now = time.monotonic()
            if (output / "INPUT.READY").exists() and not import_verified:
                info = read_json(data.with_suffix(".json"))
                hashes = {}
                with import_path.open("rb") as stream:
                    for column in info["columns"]:
                        digest = __import__("hashlib").sha256()
                        remaining = info["n"] * 8
                        while remaining:
                            block = stream.read(min(remaining, 2**20))
                            if not block:
                                raise ValueError("truncated Stata import audit")
                            digest.update(block)
                            remaining -= len(block)
                        hashes[column] = digest.hexdigest()
                    if stream.read(1):
                        raise ValueError("extra Stata import audit bytes")
                if hashes != info["column_sha256"]:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    raise ValueError("Stata imported values differ from canonical hashes")
                import_path.unlink()
                write_json(output / "import-hashes.json", hashes)
                (output / "IMPORT.CHECKED").write_text("PASS\n")
                import_verified = True
            active = (output / "START").exists() and not (output / "DONE").exists()
            if active and started is None:
                started = now
            if active:
                try:
                    parent = psutil.Process(process.pid)
                    tree = [parent, *parent.children(recursive=True)]
                    rss, nthreads = 0, 0
                    for child in tree:
                        try:
                            rss += child.memory_info().rss
                            nthreads += child.num_threads()
                        except (psutil.NoSuchProcess, psutil.AccessDenied):
                            pass
                    maximum_rss = max(maximum_rss, rss)
                    peaks_threads = max(peaks_threads, nthreads)
                    samples += 1
                    # A pre-invocation boundary sample also covers commands
                    # shorter than the periodic sampler interval. Handshake
                    # waiting is excluded from the command's own timer.
                    if not (output / "START.CHECKED").exists():
                        (output / "START.CHECKED").write_text("SAMPLED\n")
                except psutil.NoSuchProcess:
                    pass
            if started is None and now - wall_start > manifest["measurement"]["startup_timeout_seconds"]:
                reason = "startup_timeout"
            elif started is not None and active and now - started > manifest["measurement"]["command_timeout_seconds"]:
                reason = "command_timeout"
            elif started is not None and not active and now - started > manifest["measurement"]["command_timeout_seconds"] + 600:
                reason = "extraction_timeout"
            if reason:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                break
            time.sleep(manifest["measurement"]["rss_interval_seconds"])
    meta = metadata(output)
    sentinel = any("SYSTEMATIC_WORKER_COMPLETE" in p.read_text(errors="replace")
                   for p in [output / "stdout.txt", *output.glob("*.log")] if p.exists())
    status = reason or ("pass" if process.returncode == 0 and meta.get("rc") == 0 and sentinel else "worker_failure")
    if status == "pass" and meta.get("converged") is False:
        status = "nonconvergence"
    if status == "pass" and (samples < 1 or maximum_rss <= 0 or not (output / "DONE").exists()
                             or not isinstance(meta.get("seconds"), (int, float)) or meta["seconds"] <= 0):
        status = "measurement_failure"
    receipt = {"schema": "fixest-gap-local-arm-v1", "cell": cell, "arm": arm, "phase": phase,
               "status": status, "process_returncode": process.returncode, "sentinel": sentinel,
               "measurement_fingerprint": identity,
               "metadata": meta, "seconds": meta.get("seconds"), "peak_rss_gib": maximum_rss / 2**30 if samples else None,
               "failure_stage": None if status == "pass" else ("before_target_invocation" if started is None else
                                  "target_invocation" if not (output / "DONE").exists() else "result_extraction_or_status"),
               "rss_samples": samples, "rss_interval_seconds": manifest["measurement"]["rss_interval_seconds"],
               "input_value_audit": "columnwise IEEE-754 SHA-256",
               "peak_process_tree_threads": peaks_threads, "cpu_affinity": cpu_ids,
               "system_load_average_at_launch": load_at_launch,
               "job_id": os.environ.get("JOB_ID"), "task_id": os.environ.get("SGE_TASK_ID"),
               "platform": platform.platform(), "machine": platform.machine(), "input_sha256": sha256(data),
               "command_time_lower_bound": manifest["measurement"]["command_timeout_seconds"] if reason == "command_timeout" else None}
    if (output / "stata_worker.do").exists():
        receipt["stata_worker_sha256"] = sha256(output / "stata_worker.do")
    if (output / "sample.csv").exists():
        receipt["sample_sha256"], receipt["sample_rows"] = sample_hash(output / "sample.csv")
        (output / "sample.csv").unlink()
    write_json(output / "result.json", receipt)
    shutil.rmtree(temporary)
    return receipt
