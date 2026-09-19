"""Linux process-tree PSS sweeps, following the fevc paper's measurement definition.

The sampled maximum is not a simultaneous kernel snapshot or minimum RAM.
Reporting allocations and observer work remain outside estimator processes.
"""
import os
from pathlib import Path
import threading
import time

import psutil

from common import write_json

FIELDS = ("Pss", "Rss", "Private_Clean", "Private_Dirty", "Swap", "SwapPss",
          "Private_Hugetlb", "Shared_Hugetlb")


def parse(text):
    values = {}
    for line in text.splitlines():
        bits = line.split()
        if len(bits) == 3 and bits[0].rstrip(":") in FIELDS and bits[2] == "kB":
            key = bits[0].rstrip(":")
            value = int(bits[1])
            if value < 0:
                raise ValueError("negative memory counter")
            values[key] = values.get(key, 0) + value
    for key in ("Pss", "Rss", "Private_Clean", "Private_Dirty", "Swap"):
        if key not in values:
            raise ValueError("missing memory counter: " + key)
    values["Uss"] = values["Private_Clean"] + values["Private_Dirty"]
    if not 0 <= values["Uss"] <= values["Pss"] + 2 <= values["Rss"] + 4:
        raise ValueError("invalid USS/PSS/RSS ordering")
    return values


def process_memory(process):
    before = process.create_time()
    folder = Path("/proc") / str(process.pid)
    try:
        text = (folder / "smaps_rollup").read_text()
    except FileNotFoundError:
        text = (folder / "smaps").read_text()
    values = parse(text)
    if psutil.Process(process.pid).create_time() != before:
        raise ProcessLookupError("PID reused during scan")
    return values


def tree(pid):
    parent = psutil.Process(pid)
    return {p.pid: p for p in [parent, *parent.children(recursive=True)]
            if p.is_running() and p.status() != psutil.STATUS_ZOMBIE}


class Observer:
    def __init__(self, pid, output, interval, estimator_cpus):
        self.pid, self.output, self.interval = pid, Path(output), interval
        self.estimator_cpus = estimator_cpus
        self.stop = threading.Event()
        self.samples = []
        self.error = None
        self.thread = threading.Thread(target=self.run, daemon=True)

    def start(self):
        self.thread.start()

    def run(self):
        # Linux affinity on pid=0 applies to this observer thread only.
        spare = set(os.sched_getaffinity(0)) - set(self.estimator_cpus)
        if spare:
            os.sched_setaffinity(0, spare)
        try:
            while not self.stop.is_set():
                if not (self.output / "START").exists() or (self.output / "DONE").exists():
                    self.stop.wait(.005)
                    continue
                tick, cpu = time.monotonic(), time.thread_time()
                complete, errors, values = True, [], []
                try:
                    before = tree(self.pid)
                    identities = {pid: p.create_time() for pid, p in before.items()}
                    for pid, proc in before.items():
                        try:
                            values.append(process_memory(proc))
                        except (OSError, ValueError, psutil.Error) as exc:
                            errors.append(type(exc).__name__)
                    after = {pid: p.create_time() for pid, p in tree(self.pid).items()}
                    complete = bool(values) and not errors and identities == after and not (self.output / "DONE").exists()
                except (OSError, psutil.Error) as exc:
                    complete = False
                    errors.append(type(exc).__name__)
                total = {key: sum(v.get(key, 0) for v in values) for key in (*FIELDS, "Uss")}
                self.samples.append(dict(started=tick, duration=time.monotonic()-tick,
                                         cpu=time.thread_time()-cpu, complete=complete,
                                         process_count=len(values), kib=total, errors=errors))
                # The first scan is a boundary handshake; waiting is untimed.
                if complete and not (self.output / "START.CHECKED").exists():
                    (self.output / "START.CHECKED").write_text("PSS_READY\n")
                self.stop.wait(max(0, self.interval - (time.monotonic()-tick)))
        except Exception as exc:
            self.error = type(exc).__name__ + ": " + str(exc)

    def finish(self):
        self.stop.set()
        self.thread.join(timeout=10)
        if self.thread.is_alive():
            raise RuntimeError("PSS observer did not stop")
        # Exclude the pre-command handshake sweep from coverage counts.
        valid = [s for s in self.samples[1:] if s["complete"]]
        peaks = {key: max((s["kib"][key] for s in valid), default=None) for key in (*FIELDS, "Uss")}
        gaps = [b["started"]-a["started"] for a, b in zip(valid, valid[1:])]
        result = dict(schema="light-pss-v1", complete_sweeps=len(valid),
                      incomplete_sweeps=sum(not s["complete"] for s in self.samples),
                      nominal_interval_seconds=self.interval, peak_kib=peaks,
                      observer_cpu_seconds=sum(s["cpu"] for s in self.samples),
                      scan_wall_seconds=sum(s["duration"] for s in self.samples),
                      max_scan_seconds=max((s["duration"] for s in self.samples), default=None),
                      max_gap_seconds=max(gaps, default=None), error=self.error,
                      coverage="adequate" if len(valid) >= 10 and not self.error and not any(peaks.get(k) for k in ("Private_Hugetlb", "Shared_Hugetlb")) else "under_resolved")
        write_json(self.output / "memory.json", result)
        write_json(self.output / "memory-samples.json", self.samples)
        return result
