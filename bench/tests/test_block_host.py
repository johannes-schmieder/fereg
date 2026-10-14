"""Frozen host eligibility from process evidence; no native workload or timings."""
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import block_host as host


def process(pid=9, start="start-a", parent=1, command="/usr/bin/WindowServer", cpu=0.0,
            percent=0.0, state="S", executable=None):
    return {"pid": pid, "ppid": parent, "start": start, "command": command,
            "executable": executable or Path(command.split()[0]).name,
            "cpu_seconds": cpu, "cpu_percent": percent, "state": state, "rss_kib": 100}


def sample(second, processes):
    return {"kind": "snapshot", "monotonic_ns": second * 10**9,
            "finished_monotonic_ns": second * 10**9 + 1000,
            "wall_time_ns": (100 + second) * 10**9, "processes": processes,
            "load_average": [0.5, 0.5, 0.5]}


def event(kind, second, block=None, pid=None, token=None):
    return {"schema": 1, "event": kind, "run_id": "frozen-run", "block_id": block,
            "monotonic_ns": second * 10**9, "wall_time_ns": (100 + second) * 10**9,
            "tool_pid": 2, "native_pid": pid, "process_token": token}


class ProcessEvidence(unittest.TestCase):
    def test_ps_fields_and_cpu_clock_are_parsed_without_cpu_admission(self):
        row = " 42 1 Wed Oct 14 13:15:59 2026 R+ 1-02:03:04.50 999.0 123 cargo\n"
        parsed = host.parse_ps(row)[0]
        self.assertEqual(host.identity(parsed), (42, "Wed Oct 14 13:15:59 2026"))
        self.assertEqual(parsed["cpu_seconds"], 93784.5)
        self.assertEqual(parsed["cpu_percent"], 999.0)
        self.assertEqual(parsed["executable"], "cargo")
        for bad in ["", "42 1 broken", row.replace("999.0", "nan")]:
            with self.assertRaises(ValueError):
                host.parse_ps(bad)

    def test_classifier_is_explicit_and_preserves_desktop_unknown_python(self):
        for command in ["/usr/bin/cargo test", "/opt/bin/python3.14 -m pytest tests",
                        "/opt/bin/python3 bench/routing.py run", "bash scripts/run_tests.sh",
                        "target/ci/deps/fereg_core-ab123456", "/opt/bin/codex exec", "Rscript fit.R"]:
            self.assertIsNotNone(host.known_compute(command, host.POLICY), command)
        for command in ["/System/WindowServer", "/Applications/Codex.app/Contents/MacOS/Codex",
                        "python3 desktop_helper.py", "Safari", "unknown_compute --busy"]:
            self.assertIsNone(host.known_compute(command, host.POLICY), command)

    def test_os_executable_name_handles_unquoted_paths_and_narrow_agent_runtimes(self):
        for executable, command in [
            ("stata-mp", "/Applications/Stata 19/StataMP.app/Contents/MacOS/stata-mp -b do fit.do"),
            ("cargo", "/Volumes/Build Disk/tools/cargo test"),
            ("node_repl", "/Applications/Codex.app/tool runtimes/node_repl"),
            ("uvx", "/Users/test/Tool Runtimes/uvx some-tool"),
        ]:
            self.assertEqual(host.known_compute(command, host.POLICY, executable), executable)
            p = process(command=command, executable=executable, state="R", percent=2)
            self.assertEqual(len(host.process_activity([sample(1, [p])], [], [], host.POLICY)), 1)
        for executable, command in [("node", "node desktop.js"),
                                    ("Codex Helper", "/Applications/Codex.app/Contents/Frameworks/Codex Helper --type=renderer")]:
            self.assertIsNone(host.known_compute(command, host.POLICY, executable))

    def test_argument_view_is_joined_by_pid_and_start_identity(self):
        processes = host.parse_ps("42 1 Wed Oct 14 13:15:59 2026 S 0:00.00 0 12 Stata MP\n")
        self.assertEqual(processes[0]["executable"], "Stata MP")
        host.attach_ps_arguments(processes, "42 Wed Oct 14 13:16:01 2026 unrelated replacement\n")
        self.assertIsNone(processes[0]["command"])
        host.attach_ps_arguments(processes, "42 Wed Oct 14 13:15:59 2026 /Applications/Stata 19/stata-mp -b do file.do\n")
        self.assertEqual(processes[0]["command"], "/Applications/Stata 19/stata-mp -b do file.do")
        with self.assertRaises(ValueError):
            host.attach_ps_arguments(processes, "unparsed row")

    def test_only_known_active_compute_is_evidence(self):
        initial = [process(command="cargo build", cpu=1), process(pid=10, percent=999, state="R")]
        dormant = [process(command="cargo build", cpu=1), process(pid=10, cpu=20, percent=999, state="R")]
        self.assertEqual(host.process_activity([sample(1, initial), sample(6, dormant)], [], [], host.POLICY), [])
        active = copy.deepcopy(dormant)
        active[0]["cpu_seconds"] = 1.1
        hits = host.process_activity([sample(1, initial), sample(6, active)], [], [], host.POLICY)
        self.assertEqual(len(hits), 1)
        self.assertEqual(hits[0]["pid"], 9)
        self.assertEqual(hits[0]["start_monotonic_ns"], 10**9)

    def test_first_seen_runnable_compute_has_point_evidence(self):
        p = process(command="rustc file.rs", percent=1, state="R")
        hits = host.process_activity([sample(1, [p])], [], [], host.POLICY)
        self.assertEqual(hits[0]["start_monotonic_ns"], hits[0]["end_monotonic_ns"])
        self.assertIsNone(hits[0]["cpu_delta_seconds"])

    def test_owned_descendants_and_reparented_identity_stay_owned_but_pid_reuse_does_not(self):
        root = process(pid=1, parent=0, command="python bench/block_host.py")
        child = process(pid=2, parent=1, command="cargo build", percent=100, state="R")
        grandchild = process(pid=3, parent=2, command="rustc x.rs", percent=100, state="R")
        orphan = {**grandchild, "ppid": 0}
        reused = {**grandchild, "start": "new-process", "ppid": 0}
        samples = [sample(1, [root, child, grandchild]), sample(2, [orphan]), sample(3, [reused])]
        hits = host.process_activity(samples, [], [host.identity(root)], host.POLICY)
        self.assertEqual([(hit["pid"], hit["start"]) for hit in hits], [(3, "new-process")])

    def test_call_token_interval_and_start_identity_limit_native_ownership(self):
        start, end = event("call_start", 2, "b1", 9, "run/c1"), event("call_end", 4, "b1", 9, "run/c1")
        native = process(command="stata-mp -b do x.do", percent=100, state="R")
        reused = {**native, "start": "reused"}
        hits = host.process_activity([sample(3, [native]), sample(4, [reused])], [[start, end]], [], host.POLICY)
        self.assertEqual([hit["start"] for hit in hits], ["reused"])
        outside = host.process_activity([sample(5, [native])], [[start, end]], [], host.POLICY)
        self.assertEqual(len(outside), 1)


class ReceiptEvidence(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.policy = self.write_json("policy.json", host.POLICY)
        self.manifest = self.write_json("manifest.json", {"plan": "frozen"})
        self.records = self.write_json("records.json", {"scientific_status": "checked-by-runner"})
        self.events = [event("run_start", 1), event("block_start", 2, "b1"),
                       event("block_end", 3, "b1"), event("block_start", 5, "b2"),
                       event("block_end", 6, "b2"), event("run_end", 7)]
        self.events_path = self.root / "host-events.jsonl"
        self.write_lines(self.events_path, self.events)
        self.context = {"run_id": "frozen-run", "manifest": host.reference(self.manifest),
                        "records": host.reference(self.records), "events": host.reference(self.events_path),
                        "schedule": [{"block_id": block} for block in ["b1", "b2"]],
                        "blocks": [{"block_id": "b1", "start_monotonic_ns": 2 * 10**9, "end_monotonic_ns": 3 * 10**9},
                                   {"block_id": "b2", "start_monotonic_ns": 5 * 10**9, "end_monotonic_ns": 6 * 10**9}]}
        self.raw = [{"kind": "observer_start", "source": host.reference(host.__file__),
                     "manifest": self.context["manifest"], "policy": host.reference(self.policy), "owned_roots": []},
                    sample(1, [process()]), sample(4, [process()]), sample(7, [process()]),
                    {"kind": "observer_end", "events": self.context["events"], "records": self.context["records"]}]

    def write_json(self, name, data):
        path = self.root / name
        path.write_text(json.dumps(data))
        return path

    def write_lines(self, path, data):
        path.write_text("".join(json.dumps(row) + "\n" for row in data))

    def receipt(self):
        raw_path = self.root / "host-observations.jsonl"
        self.write_lines(raw_path, self.raw)
        return self.write_json("host-receipt.json", {"schema": 1, "run_id": "frozen-run",
            **{key: self.context[key] for key in ["manifest", "records", "events"]},
            "source": host.reference(host.__file__), "policy": host.reference(self.policy),
            "observations": host.reference(raw_path)})

    def test_all_blocks_are_retained_and_unknown_bursts_do_not_exclude(self):
        self.raw[2]["processes"] = [process(cpu=999, percent=999, state="R")]
        result = host.validate_receipt(self.receipt(), self.context)
        self.assertEqual([row["status"] for row in result["blocks"]], ["eligible", "eligible"])
        self.assertEqual(result["status"], "complete")

    def test_activity_excludes_whole_affected_block_with_hashed_external_evidence(self):
        for index, cpu in [(1, 0), (2, 0.1), (3, 0.1)]:
            self.raw[index]["processes"] = [process(command="cargo test", cpu=cpu)]
        result = host.validate_receipt(self.receipt(), self.context)
        self.assertEqual([row["status"] for row in result["blocks"]], ["excluded", "eligible"])
        evidence = result["blocks"][0]["evidence"][0]
        self.assertEqual(host.digest(evidence["path"]), evidence["sha256"])

    def test_missing_monitor_coverage_or_unknown_fault_fails_closed(self):
        for change in [lambda: self.raw.pop(2),
                       lambda: self.raw[1].update(error="ps failed"),
                       lambda: self.raw.insert(2, {"kind": "host_fault", "reason": "unverified pressure"})]:
            saved = copy.deepcopy(self.raw)
            change()
            if len(self.raw) == 4:
                self.raw[2]["monotonic_ns"] = 10 * 10**9
            with self.assertRaises(ValueError):
                host.validate_receipt(self.receipt(), self.context)
            self.raw = saved

    def test_missing_duplicate_or_incomplete_planned_block_fails_closed(self):
        for key, replacement in [("schedule", [{"block_id": "b1"}]),
                                 ("schedule", [{"block_id": "b1"}, {"block_id": "b1"}]),
                                 ("blocks", [self.context["blocks"][0]])]:
            context = {**self.context, key: replacement}
            with self.assertRaises(ValueError):
                host.validate_receipt(self.receipt(), context)

    def test_mutated_event_policy_source_or_raw_evidence_is_rejected(self):
        for field in ["source", "policy", "events", "observations"]:
            path = self.receipt()
            value = json.loads(path.read_text())
            value[field]["sha256"] = "0" * 64
            path.write_text(json.dumps(value))
            with self.assertRaises(ValueError):
                host.validate_receipt(path, self.context)

    def test_eligibility_does_not_read_scientific_status_or_timing(self):
        self.records.write_text(json.dumps({"timeout": True, "science": "failed", "candidate_time": 1e99}))
        self.context["records"] = host.reference(self.records)
        self.raw[-1]["records"] = self.context["records"]
        self.assertTrue(all(row["status"] == "eligible" for row in host.validate_receipt(self.receipt(), self.context)["blocks"]))
        # The runner separately refuses these failure statuses; they cannot be
        # converted into host exclusions by this outcome-blind observer.

    def test_partial_event_tail_is_incomplete(self):
        self.events_path.write_text(self.events_path.read_text() + '{"event":')
        self.context["events"] = host.reference(self.events_path)
        self.raw[-1]["events"] = self.context["events"]
        with self.assertRaises(ValueError):
            host.validate_receipt(self.receipt(), self.context)

    def test_snapshot_failure_records_error_without_process_control(self):
        with patch.object(host.subprocess, "run", side_effect=OSError("denied")):
            result = host.snapshot("test")
        self.assertIn("denied", result["error"])

    def test_wrapper_uses_separate_outputs_samples_boundaries_and_never_signals(self):
        run = self.root / "runner"
        events_path, records_path = run / "host-events.jsonl", run / "records.json"
        output = self.root / "observer"
        root_process = process(pid=os.getpid(), parent=0, command="python bench/block_host.py")

        def launch(_command):
            self.assertFalse(run.exists())
            run.mkdir()
            self.write_lines(events_path, self.events)
            records_path.write_text("{}")
            return child

        with patch.object(host.subprocess, "Popen", side_effect=launch) as popen, \
                patch.object(host, "snapshot", side_effect=[sample(t, [root_process]) for t in [0, 4, 8]]) as snapshots, \
                patch.object(host.time, "sleep"), \
                patch.object(host.subprocess, "run") as other_process:
            child = unittest.mock.Mock(returncode=0)
            child.poll.side_effect = [None, 0]
            self.assertEqual(host.observe(["runner"], self.manifest, self.policy, events_path,
                                          records_path, output), 0)
            child.kill.assert_not_called()
            child.terminate.assert_not_called()
            other_process.assert_not_called()
            self.assertEqual([call.args[0] for call in snapshots.call_args_list], ["before", "block", "after"])
            popen.assert_called_once_with(["runner"])
        context = {**self.context, "records": host.reference(records_path), "events": host.reference(events_path)}
        result = host.validate_receipt(output / "host-receipt.json", context)
        self.assertEqual([row["status"] for row in result["blocks"]], ["eligible", "eligible"])
        with self.assertRaises(ValueError):
            host.observe(["runner"], self.manifest, self.policy, events_path, records_path, output)

    def test_event_prefix_rewrite_at_exit_is_preserved_as_invalid_coverage(self):
        run = self.root / "runner"
        run.mkdir()
        events_path, records_path = run / "host-events.jsonl", run / "records.json"
        records_path.write_text("{}")
        output = self.root / "observer"
        root_process = process(pid=os.getpid(), parent=0)
        child = unittest.mock.Mock(returncode=0)

        def poll():
            if not events_path.exists():
                self.write_lines(events_path, self.events)
                return None
            events_path.write_text(events_path.read_text().replace("frozen-run", "edited-run"))
            return 0

        child.poll.side_effect = poll
        with patch.object(host.subprocess, "Popen", return_value=child), \
                patch.object(host, "snapshot", side_effect=[sample(t, [root_process]) for t in [0, 4, 8]]), \
                patch.object(host.time, "sleep"):
            host.observe(["runner"], self.manifest, self.policy, events_path, records_path, output)
        raw = host.read_lines(output / "host-observations.jsonl")
        self.assertTrue(any(row.get("kind") == "observer_error" for row in raw))
        child.kill.assert_not_called()


if __name__ == "__main__":
    unittest.main()
