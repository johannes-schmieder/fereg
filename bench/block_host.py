#!/usr/bin/env python3
"""Passive process evidence for frozen paired blocks; never stops other processes.

The observer records sampled activity, not proof that no unsampled work ran.
Ordinary OS/desktop load is retained. Eligibility is recomputed from the raw
observations without reading solve times, scientific results or arm labels.
"""
import argparse
from functools import lru_cache
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time


POLICY = {
    "schema": 1,
    "name": "known-compute-blocks-v1",
    "sample_seconds": 5.0,
    "maximum_gap_seconds": 7.5,
    "active_cpu_seconds": 0.05,
    "runnable_cpu_percent": 1.0,
    "executables": ["cargo", "rustc", "rustdoc", "clang", "clang++", "gcc", "g++",
                    "cc1", "cc1plus", "ld", "ld.lld", "make", "gmake", "ninja",
                    "cmake", "pytest", "py.test", "R", "Rscript", "stata", "stata-mp",
                    "stata-se", "StataMP", "StataSE", "julia", "codex", "claude",
                    "node_repl", "uvx"],
    "executable_identity": "macOS ucomm or Linux comm; args joined by PID and lstart",
    "python_modules": ["pytest", "unittest", "compileall"],
    "script_directories": ["bench", "scripts", "tests"],
    "rust_test_path": "target/(debug|release|ci)/deps/[^/ ]+-[0-9a-f]{8,}$",
    "fault_capabilities": [],
    "unknown_activity": "retain",
    "coverage_failure": "incomplete-no-replacement",
}
EVENTS = {"run_start", "run_end", "warmup_start", "warmup_end", "block_start",
          "block_end", "call_start", "call_end"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def reference(path):
    return {"path": str(Path(path).resolve()), "sha256": digest(path)}


def read_reference(ref):
    require(set(ref) == {"path", "sha256"}, "invalid evidence reference")
    require(digest(ref["path"]) == ref["sha256"], "evidence hash changed")
    return json.loads(Path(ref["path"]).read_text())


def read_lines(path):
    raw = Path(path).read_bytes()
    require(not raw or raw.endswith(b"\n"), "partial append-only record")
    return [json.loads(line) for line in raw.splitlines()]


def identity(process):
    return process["pid"], process["start"]


def cpu_seconds(value):
    days, _, clock = value.rpartition("-")
    parts = [float(part) for part in clock.split(":")]
    require(1 <= len(parts) <= 3, "invalid process CPU clock")
    return (int(days) * 86400 if days else 0) + sum(
        part * 60 ** power for power, part in enumerate(reversed(parts)))


def parse_ps(output):
    processes = []
    for line in output.splitlines():
        parts = line.split(None, 11)
        require(len(parts) == 12, "unparsed ps row")
        process = {"pid": int(parts[0]), "ppid": int(parts[1]),
                   "start": " ".join(parts[2:7]), "state": parts[7],
                   "cpu_seconds": cpu_seconds(parts[8]), "cpu_percent": float(parts[9]),
                   "rss_kib": int(parts[10]), "executable_raw": parts[11],
                   "executable": parts[11].rstrip(), "command": None}
        # Keep lossless final-field identity evidence alongside its unpadded name.
        require(process["pid"] > 0 and process["ppid"] >= 0, "invalid process PID")
        require(all(math.isfinite(process[key]) and process[key] >= 0
                    for key in ["cpu_seconds", "cpu_percent", "rss_kib"]), "invalid ps metric")
        processes.append(process)
    require(processes and len({p["pid"] for p in processes}) == len(processes), "invalid ps census")
    return processes


def attach_ps_arguments(processes, output):
    # Separate ps views preserve executable names and paths containing spaces.
    # A process exiting between views simply has unavailable argument evidence.
    arguments = {}
    for line in output.splitlines():
        parts = line.split(None, 6)
        require(len(parts) == 7, "unparsed ps argument row")
        key = int(parts[0]), " ".join(parts[1:6])
        require(key not in arguments, "duplicate ps argument identity")
        arguments[key] = parts[6]
    for process in processes:
        process["command"] = arguments.get(identity(process))


def snapshot(reason):
    wall, mono = time.time_ns(), time.monotonic_ns()
    result = {"kind": "snapshot", "wall_time_ns": wall, "monotonic_ns": mono,
              "reason": reason, "load_average": list(os.getloadavg())}
    try:
        def ps(fields):
            return subprocess.run(
                ["ps", "-axo", fields], check=True, capture_output=True,
                text=True, timeout=3, env={**os.environ, "LC_ALL": "C"})
        name_field = "ucomm" if sys.platform == "darwin" else "comm"
        output = ps("pid=,ppid=,lstart=,state=,time=,pcpu=,rss=," + name_field + "=")
        result["processes"] = parse_ps(output.stdout)
        attach_ps_arguments(result["processes"], ps("pid=,lstart=,args=").stdout)
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        result["error"] = str(error)
    result["finished_monotonic_ns"] = time.monotonic_ns()
    return result


def known_compute(command, policy, executable=None):
    """Explicit executable/script classification; GUI apps and unknown Python stay noise."""
    if executable in policy["executables"]:
        return executable
    try:
        argv = shlex.split(command or "")
    except ValueError:
        return None
    if not argv:
        return None
    executable = executable if executable is not None else Path(argv[0]).name
    if executable in policy["executables"]:
        return executable
    if re.search(policy["rust_test_path"], argv[0]):
        return "rust-test"
    if re.fullmatch(r"python(?:\d+(?:\.\d+)*)?", executable):
        if len(argv) > 2 and argv[1] == "-m" and argv[2] in policy["python_modules"]:
            return "python-module:" + argv[2]
        script = next((arg for arg in argv[1:] if not arg.startswith("-")), "")
        if script.endswith(".py") and set(Path(script).parts) & set(policy["script_directories"]):
            return "python-repository-script"
    if executable in {"bash", "sh", "zsh"}:
        script = next((arg for arg in argv[1:] if not arg.startswith("-")), "")
        if script.endswith(".sh") and set(Path(script).parts) & set(policy["script_directories"]):
            return "repository-shell-script"
    return None


def validate_events(events, run_id):
    require(events and events[0]["event"] == "run_start" and events[-1]["event"] == "run_end",
            "missing run boundary")
    prior = -1
    calls, blocks = {}, {}
    for event in events:
        require(event.get("schema") == 1 and event.get("run_id") == run_id, "event run binding mismatch")
        require(event.get("event") in EVENTS, "unknown host event")
        now = event["monotonic_ns"]
        require(isinstance(now, int) and now >= prior, "event clock/order invalid")
        prior = now
        kind = event["event"]
        if kind in {"call_start", "call_end"}:
            token = event.get("process_token")
            require(isinstance(token, str) and token, "missing call process token")
            if kind == "call_start":
                require(token not in calls and isinstance(event.get("native_pid"), int), "duplicate/invalid call start")
                calls[token] = [event, None]
            else:
                require(token in calls and calls[token][1] is None, "unmatched call end")
                require(event["native_pid"] == calls[token][0]["native_pid"], "call PID changed")
                calls[token][1] = event
        if kind in {"block_start", "block_end"}:
            block = event["block_id"]
            if kind == "block_start":
                require(block not in blocks, "duplicate block start")
                blocks[block] = [now, None]
            else:
                require(block in blocks and blocks[block][1] is None, "unmatched block end")
                blocks[block][1] = now
    require(all(end is not None for _, end in calls.values()), "incomplete call events")
    require(all(end is not None for _, end in blocks.values()), "incomplete block events")
    return list(calls.values()), blocks


def process_activity(samples, calls, roots, policy):
    """Track identities through sampled parent edges; never keep a bare PID exemption."""
    @lru_cache(maxsize=8192)
    def classify(command, executable):
        return known_compute(command, policy, executable)

    owned = {tuple(root) for root in roots}
    call_identities, prior, evidence = {}, {}, []
    previous_time = None
    for sample in samples:
        now = sample["monotonic_ns"]
        processes = sample["processes"]
        by_pid = {p["pid"]: p for p in processes}
        current_owned = {identity(p) for p in processes if identity(p) in owned}
        for start, end in calls:
            token, pid = start["process_token"], start["native_pid"]
            if start["monotonic_ns"] <= now <= end["monotonic_ns"] and pid in by_pid:
                candidate = identity(by_pid[pid])
                if token not in call_identities:
                    call_identities[token] = candidate
                if candidate == call_identities[token]:
                    current_owned.add(candidate)
        changed = True
        while changed:
            changed = False
            for process in processes:
                parent = by_pid.get(process["ppid"])
                key = identity(process)
                if parent and identity(parent) in current_owned and key not in current_owned:
                    current_owned.add(key)
                    changed = True
        owned.update(current_owned)
        for process in processes:
            key = identity(process)
            category = classify(process["command"], process["executable"])
            if key in current_owned or category is None:
                continue
            previous = prior.get(key)
            delta = None if previous is None else process["cpu_seconds"] - previous["cpu_seconds"]
            interval = None if previous_time is None else (now - previous_time) / 1e9
            accumulating = (delta is not None and interval is not None and 0 < interval <= policy["maximum_gap_seconds"]
                            and delta >= policy["active_cpu_seconds"])
            runnable = "R" in process["state"] and process["cpu_percent"] >= policy["runnable_cpu_percent"]
            if accumulating or runnable:
                evidence.append({"start_monotonic_ns": previous_time if accumulating else now,
                                 "end_monotonic_ns": now, "pid": key[0], "start": key[1],
                                 "category": category, "cpu_delta_seconds": delta,
                                 "cpu_percent": process["cpu_percent"], "state": process["state"]})
        prior = {identity(p): p for p in processes}
        previous_time = now
    return evidence


def validate_receipt(receipt_path, context):
    """Fail closed on incomplete evidence; return every planned block, including exclusions."""
    receipt = json.loads(Path(receipt_path).read_text())
    require(receipt.get("schema") == 1 and receipt.get("run_id") == context["run_id"], "receipt run binding mismatch")
    for key in ["manifest", "records", "events"]:
        require(receipt[key] == context[key], key + " reference mismatch")
        require(digest(context[key]["path"]) == context[key]["sha256"], key + " changed")
    require(receipt["source"] == reference(__file__), "observer source binding changed")
    policy = read_reference(receipt["policy"])
    require(policy == POLICY, "unsupported or changed host policy")
    require(digest(receipt["observations"]["path"]) == receipt["observations"]["sha256"], "observations changed")
    raw = read_lines(receipt["observations"]["path"])
    require(raw and raw[0].get("kind") == "observer_start" and raw[-1].get("kind") == "observer_end", "missing observer boundary")
    require(raw[0]["source"] == receipt["source"] and raw[0]["policy"] == receipt["policy"]
            and raw[0]["manifest"] == receipt["manifest"], "observer initial binding mismatch")
    require(raw[-1]["events"] == receipt["events"] and raw[-1]["records"] == receipt["records"], "observer final binding mismatch")
    require(all(row.get("kind") in {"observer_start", "snapshot", "observer_end"} for row in raw),
            "observer error or unsupported host fault")
    samples = [row for row in raw if row.get("kind") == "snapshot"]
    require(len(samples) >= 2 and not any("error" in row for row in samples), "missing/failed process coverage")
    require(all(0 <= row["finished_monotonic_ns"] - row["monotonic_ns"] <= policy["maximum_gap_seconds"] * 1e9
                for row in samples), "invalid process sample duration")
    for left, right in zip(samples, samples[1:]):
        gap = (right["monotonic_ns"] - left["monotonic_ns"]) / 1e9
        require(0 < gap <= policy["maximum_gap_seconds"], "process coverage gap")
    events = read_lines(context["events"]["path"])
    calls, intervals = validate_events(events, context["run_id"])
    require(samples[0]["monotonic_ns"] <= events[0]["monotonic_ns"]
            and samples[-1]["monotonic_ns"] >= events[-1]["monotonic_ns"], "uncovered run boundary")
    planned = [slot["block_id"] for slot in context["schedule"]]
    require(len(planned) == len(set(planned)) and set(planned) == set(intervals), "planned block coverage mismatch")
    declared = {block["block_id"]: block for block in context["blocks"]}
    require(len(declared) == len(context["blocks"]) and set(declared) == set(planned), "block receipt coverage mismatch")
    activity = process_activity(samples, calls, raw[0]["owned_roots"], policy)
    results = []
    for block in planned:
        start, end = intervals[block]
        require([declared[block]["start_monotonic_ns"], declared[block]["end_monotonic_ns"]] == [start, end], "block interval mismatch")
        hits = [item for item in activity if item["start_monotonic_ns"] <= end and item["end_monotonic_ns"] >= start]
        results.append({"block_id": block, "status": "excluded" if hits else "eligible",
                        "reason": "known_competing_compute" if hits else None,
                        "evidence": [receipt["observations"]], "activity": hits})
    return {"status": "complete", "policy_sha256": receipt["policy"]["sha256"],
            "manifest_sha256": receipt["manifest"]["sha256"], "records_sha256": receipt["records"]["sha256"],
            "events_sha256": receipt["events"]["sha256"], "blocks": results}


def write_line(handle, record):
    handle.write(json.dumps(record, sort_keys=True) + "\n")
    handle.flush()


def observe(command, manifest, policy_path, events_path, records_path, output):
    """Wrap one runner. Observation faults are recorded; the runner is never killed."""
    policy_ref, source_ref, manifest_ref = reference(policy_path), reference(__file__), reference(manifest)
    require(read_reference(policy_ref) == POLICY, "unsupported policy")
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    raw_path, receipt_path = output / "host-observations.jsonl", output / "host-receipt.json"
    require(not receipt_path.exists() and not Path(events_path).exists(), "fresh output/events required")
    with raw_path.open("x") as handle:
        first = snapshot("before")
        require("error" not in first, "initial process snapshot failed")
        own = next(identity(p) for p in first["processes"] if p["pid"] == os.getpid())
        # The observer owns only its identity and subsequently sampled descendants.
        write_line(handle, {"kind": "observer_start", "source": source_ref, "policy": policy_ref,
                            "manifest": manifest_ref, "owned_roots": [own],
                            "capabilities": {"ps": True, "memory_pressure": False, "suspend": False},
                            "limits": "Sampled activity only: first-seen sleeping work, short-lived unsampled work, unavailable argument evidence and PID reuse within ps start-time resolution may be missed."})
        write_line(handle, first)
        child = subprocess.Popen(command)
        prefix, next_sample = b"", time.monotonic() + POLICY["sample_seconds"]
        while child.poll() is None:
            try:
                data = Path(events_path).read_bytes() if Path(events_path).exists() else b""
                require(data.startswith(prefix), "host event prefix changed")
                complete = data[:data.rfind(b"\n") + 1]
                lines = complete[len(prefix):].splitlines()
                boundary = any(json.loads(line)["event"] in {"block_start", "block_end"} for line in lines)
                prefix = complete
                if boundary or time.monotonic() >= next_sample:
                    write_line(handle, snapshot("block" if boundary else "periodic"))
                    next_sample = time.monotonic() + POLICY["sample_seconds"]
            except (OSError, ValueError, KeyError) as error:
                write_line(handle, {"kind": "observer_error", "error": str(error)})
            time.sleep(0.05)
        write_line(handle, snapshot("after"))
        if not Path(events_path).read_bytes().startswith(prefix):
            write_line(handle, {"kind": "observer_error", "error": "host event prefix changed at exit"})
        final = {"kind": "observer_end", "runner_exit_code": child.returncode,
                 "events": reference(events_path), "records": reference(records_path)}
        write_line(handle, final)
    events = read_lines(events_path)
    receipt = {"schema": 1, "run_id": events[0]["run_id"], "source": source_ref,
               "policy": policy_ref, "manifest": manifest_ref, "events": final["events"],
               "records": final["records"], "observations": reference(raw_path)}
    with receipt_path.open("x") as handle:
        json.dump(receipt, handle, indent=2)
        handle.write("\n")
    return child.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["manifest", "policy", "events", "records", "output"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    require(command, "missing wrapped runner command")
    return observe(command, args.manifest, args.policy, args.events, args.records, args.output)


if __name__ == "__main__":
    sys.exit(main())
