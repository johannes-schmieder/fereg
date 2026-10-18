"""Passive process classification for native qualification host monitoring."""

from functools import lru_cache
import hashlib
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
    "unknown_activity": "retain",
}

# The desktop coordinator also uses the lowercase CLI executable name. Its
# housekeeping CPU is not evidence that a local agent tool is doing computation.
# This role exception is prospective and Mac-only; descendants are never exempt.
MAC_POLICY = {**POLICY, "name": "mac-desktop-roles-v2",
              "desktop_services": ["codex-app-server"]}


def desktop_role(command, executable=None):
    """Recognize a first subcommand, never an arbitrary argv substring."""
    if executable != "codex":
        return None
    try:
        argv = shlex.split(command or "")
    except ValueError:
        return None
    if not argv or Path(argv[0]).name != "codex":
        return None
    index = 1
    while index + 1 < len(argv) and argv[index] in {"-c", "--config"}:
        if "=" not in argv[index + 1]:
            return None
        index += 2
    if index < len(argv) and argv[index] == "app-server":
        return {"role": "codex-app-server", "program": argv[0],
                "subcommand": "app-server", "leading_config_options": (index - 1) // 2}
    return None


def process_evidence(process):
    """Keep identity and metrics without copying possibly secret argv values."""
    return {key: process.get(key) for key in
            ("pid", "ppid", "start", "executable", "executable_raw", "state",
             "cpu_seconds", "cpu_percent", "rss_kib")} | {
        "command_sha256": hashlib.sha256(process["command"].encode()).hexdigest()
        if process.get("command") is not None else None}


def require(condition, message):
    if not condition:
        raise ValueError(message)


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
    if policy.get("desktop_services"):
        role = desktop_role(command, executable)
        if role and role["role"] in policy["desktop_services"]:
            return None
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
    if re.fullmatch(r"python(?:\d+(?:\.\d+)*)?", executable, re.IGNORECASE):
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


def process_activity(samples, roots, policy):
    """Track identities through sampled parent edges; never keep a bare PID exemption."""
    @lru_cache(maxsize=8192)
    def classify(command, executable):
        return known_compute(command, policy, executable)

    owned = {tuple(root) for root in roots}
    prior, evidence = {}, []
    previous_time = None
    for sample in samples:
        now = sample["monotonic_ns"]
        processes = sample["processes"]
        by_pid = {p["pid"]: p for p in processes}
        current_owned = {identity(p) for p in processes if identity(p) in owned}
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
                                 "cpu_percent": process["cpu_percent"], "state": process["state"],
                                 "process": process_evidence(process),
                                 "previous_process": process_evidence(previous) if previous else None})
        prior = {identity(p): p for p in processes}
        previous_time = now
    return evidence
