"""Process parser/classifier tests; no native workload or timing."""
import copy
from pathlib import Path
import sys
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

class ProcessEvidence(unittest.TestCase):
    def test_mac_recovery_keeps_small_runtime_activity_but_detects_substantial_work(self):
        first = process(command='node_repl', cpu=.03)
        small = {**first, 'cpu_seconds': .1}
        samples = [sample(1, [first]), sample(6, [small])]
        self.assertEqual(len(host.process_activity(samples, [], host.MAC_POLICY)), 1)
        self.assertEqual(host.process_activity(samples, [], host.MAC_RECOVERY_POLICY), [])
        large = {**first, 'cpu_seconds': 1.03}
        hit, = host.process_activity([samples[0], sample(6, [large])], [], host.MAC_RECOVERY_POLICY)
        self.assertEqual(hit['cpu_delta_seconds'], 1.)
        running = process(command='cargo test', percent=20, state='R')
        self.assertEqual(len(host.process_activity([sample(6, [running])], [], host.MAC_RECOVERY_POLICY)), 1)
        self.assertEqual(host.POLICY['active_cpu_seconds'], .05)
        self.assertEqual(host.MAC_POLICY['active_cpu_seconds'], .05)

    def test_mac_desktop_role_is_exact_and_does_not_exempt_cli_work(self):
        server = '/Applications/ChatGPT.app/codex -c features.code_mode_host=true app-server --analytics-default-enabled'
        role = host.desktop_role(server, 'codex')
        self.assertEqual(role['subcommand'], 'app-server')
        self.assertEqual(role['leading_config_options'], 1)
        self.assertIsNone(host.known_compute(server, host.MAC_POLICY, 'codex'))
        self.assertEqual(host.known_compute(server, host.POLICY, 'codex'), 'codex')
        self.assertEqual(host.known_compute(server, host.MAC_POLICY), 'codex')
        for command in ['codex exec app-server', 'codex -c key=app-server exec',
                        'codex --unknown app-server', 'codex -c app-server',
                        'codex "broken', None, 'other app-server']:
            with self.subTest(command=command):
                self.assertIsNone(host.desktop_role(command, 'codex'))
                self.assertEqual(host.known_compute(command, host.MAC_POLICY, 'codex'), 'codex')
        self.assertEqual(host.known_compute(server, host.MAC_POLICY, 'cargo'), 'cargo')
        self.assertIsNotNone(host.desktop_role('"/Applications/App With Spaces/codex" --config x=y app-server', 'codex'))

    def test_mac_server_cpu_is_noise_but_its_active_tool_child_is_not_exempt(self):
        server = process(pid=10, command='codex -c features.code_mode_host=true app-server', cpu=1)
        active = {**server, 'cpu_seconds': 1.0500000000010914, 'cpu_percent': 4.4}
        child = process(pid=11, parent=10, command='node_repl --session=a', percent=2, state='R')
        samples = [sample(1, [server]), sample(6, [active, child])]
        hits = host.process_activity(samples, [], host.MAC_POLICY)
        self.assertEqual([hit['pid'] for hit in hits], [11])
        self.assertEqual(hits[0]['process']['ppid'], 10)
        self.assertNotIn('command', hits[0]['process'])
        self.assertEqual(len(hits[0]['process']['command_sha256']), 64)
        # The old policy still fails on the recorded housekeeping threshold.
        self.assertEqual([hit['pid'] for hit in host.process_activity(samples, [], host.POLICY)], [10, 11])
        cli_reuse = {**active, 'start': 'new-birth', 'command': 'codex exec task', 'state': 'R', 'cpu_percent': 2}
        self.assertEqual([h['pid'] for h in host.process_activity([sample(7, [cli_reuse])], [], host.MAC_POLICY)], [10])

    def test_mac_python_executable_case_preserves_repository_work_detection(self):
        for command, category in [('python3 bench/qualify.py run', 'python-repository-script'),
                                  ('python3 -m unittest tests', 'python-module:unittest'),
                                  ('python3 desktop_helper.py', None)]:
            p = process(command=command, executable='Python', state='R', percent=50)
            hits = host.process_activity([sample(1, [p])], [], host.MAC_POLICY)
            self.assertEqual([h['category'] for h in hits], [] if category is None else [category])

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
                        "/opt/bin/python3 bench/qualify.py run", "bash scripts/run_tests.sh",
                        "target/ci/deps/fereg_core-ab123456", "/opt/bin/codex exec", "Rscript fit.R"]:
            self.assertIsNotNone(host.known_compute(command, host.POLICY), command)
        for command in ["/System/WindowServer", "/Applications/Codex.app/Contents/MacOS/Codex",
                        "python3 desktop_helper.py", "Safari", "unknown_compute --busy"]:
            self.assertIsNone(host.known_compute(command, host.POLICY), command)

    def test_padded_literal_ps_identity_reaches_argument_classification_and_activity(self):
        cases = [
            ('codex', '/opt/bin/codex exec', 'codex'),
            ('node_repl', '/Applications/Codex.app/tool runtimes/node_repl', 'node_repl'),
            ('cargo', '/opt/bin/cargo test', 'cargo'),
            ('python3', '/opt/bin/python3 -m unittest tests', 'python-module:unittest'),
            ('python3', '/opt/bin/python3 bench/qualify.py run', 'python-repository-script'),
            ('bash', '/bin/bash scripts/run_tests.sh', 'repository-shell-script'),
            ('WindowServer', '/System/WindowServer', None),
            ('python3', '/opt/bin/python3 desktop_helper.py', None),
        ]
        for executable, command, category in cases:
            with self.subTest(executable=executable, command=command):
                literal = executable + '   \t '
                row = '42 1 Wed Oct 14 13:15:59 2026 R 0:01.00 2.0 123 ' + literal + '\n'
                parsed = host.parse_ps(row)
                host.attach_ps_arguments(parsed,
                    '42 Wed Oct 14 13:15:59 2026 ' + command + '\n')
                self.assertEqual(parsed[0]['executable_raw'], literal)
                self.assertEqual(parsed[0]['executable'], executable)
                self.assertEqual(parsed[0]['command'], command)
                self.assertEqual(host.known_compute(command, host.POLICY, parsed[0]['executable']), category)
                hits = host.process_activity([sample(1, parsed)], [], host.POLICY)
                self.assertEqual([hit['category'] for hit in hits], [] if category is None else [category])

    def test_ps_normalization_preserves_internal_spaces_raw_padding_and_other_fields(self):
        prefix = '42 7 Wed Oct 14 13:15:59 2026 S 0:01.25 2.5 123 '
        padded = host.parse_ps(prefix + 'Codex  Helper   \n')[0]
        unpadded = host.parse_ps(prefix + 'Codex  Helper\n')[0]
        self.assertEqual(padded['executable_raw'], 'Codex  Helper   ')
        self.assertEqual(padded['executable'], 'Codex  Helper')
        self.assertEqual(unpadded['executable_raw'], unpadded['executable'])
        self.assertEqual({key: value for key, value in padded.items() if key != 'executable_raw'},
                         {key: value for key, value in unpadded.items() if key != 'executable_raw'})
        self.assertEqual((padded['pid'], padded['ppid'], padded['start'], padded['state'],
                          padded['cpu_seconds'], padded['cpu_percent'], padded['rss_kib']),
                         (42, 7, 'Wed Oct 14 13:15:59 2026', 'S', 1.25, 2.5, 123))

    def test_classification_reuse_preserves_cpu_and_parent_ownership_changes(self):
        command = 'python3 -m unittest tests'
        first = process(command=command, cpu=1)
        active = {**first, 'cpu_seconds': 1.25}
        root = process(pid=1, parent=0, command='unknown supervisor')
        owned = {**active, 'ppid': 1, 'cpu_seconds': 1.5, 'state': 'R', 'cpu_percent': 100}
        reused = {**first, 'start': 'reused-pid', 'state': 'R', 'cpu_percent': 2}
        samples = [sample(1, [first]), sample(2, [active]), sample(3, [root, owned]),
                   sample(4, [reused])]
        with patch.object(host.shlex, 'split', wraps=host.shlex.split) as split:
            hits = host.process_activity(samples, [host.identity(root)], host.POLICY)
        self.assertEqual([(hit['start'], hit['cpu_delta_seconds']) for hit in hits],
                         [('start-a', .25), ('reused-pid', None)])
        self.assertEqual([hit['category'] for hit in hits], ['python-module:unittest'] * 2)
        self.assertEqual(sum(call.args == (command,) for call in split.call_args_list), 1)

    def test_classification_distinguishes_commands_and_each_new_policy(self):
        initial = process(pid=9, command='python3 desktop_helper.py', state='R', percent=2)
        changed = {**initial, 'command': 'python3 -m unittest tests'}
        different_executable = {**changed, 'executable': 'desktop-python'}
        hits = host.process_activity([sample(1, [initial]), sample(2, [changed]),
                                      sample(3, [different_executable])], [], host.POLICY)
        self.assertEqual([hit['end_monotonic_ns'] for hit in hits], [2 * 10**9])
        policy = copy.deepcopy(host.POLICY)
        self.assertEqual(host.process_activity([sample(1, [initial])], [], policy), [])
        policy['executables'].append('python3')
        hits = host.process_activity([sample(1, [initial])], [], policy)
        self.assertEqual([hit['category'] for hit in hits], ['python3'])

    def test_os_executable_name_handles_unquoted_paths_and_narrow_agent_runtimes(self):
        for executable, command in [
            ("stata-mp", "/Applications/Stata 19/StataMP.app/Contents/MacOS/stata-mp -b do fit.do"),
            ("cargo", "/Volumes/Build Disk/tools/cargo test"),
            ("node_repl", "/Applications/Codex.app/tool runtimes/node_repl"),
            ("uvx", "/Users/test/Tool Runtimes/uvx some-tool"),
        ]:
            self.assertEqual(host.known_compute(command, host.POLICY, executable), executable)
            p = process(command=command, executable=executable, state="R", percent=2)
            self.assertEqual(len(host.process_activity([sample(1, [p])], [], host.POLICY)), 1)
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
        self.assertEqual(host.process_activity([sample(1, initial), sample(6, dormant)], [], host.POLICY), [])
        active = copy.deepcopy(dormant)
        active[0]["cpu_seconds"] = 1.1
        hits = host.process_activity([sample(1, initial), sample(6, active)], [], host.POLICY)
        self.assertEqual(len(hits), 1)
        self.assertEqual(hits[0]["pid"], 9)
        self.assertEqual(hits[0]["start_monotonic_ns"], 10**9)

    def test_first_seen_runnable_compute_has_point_evidence(self):
        p = process(command="rustc file.rs", percent=1, state="R")
        hits = host.process_activity([sample(1, [p])], [], host.POLICY)
        self.assertEqual(hits[0]["start_monotonic_ns"], hits[0]["end_monotonic_ns"])
        self.assertIsNone(hits[0]["cpu_delta_seconds"])

    def test_owned_descendants_and_reparented_identity_stay_owned_but_pid_reuse_does_not(self):
        root = process(pid=1, parent=0, command="python bench/qualify.py")
        child = process(pid=2, parent=1, command="cargo build", percent=100, state="R")
        grandchild = process(pid=3, parent=2, command="rustc x.rs", percent=100, state="R")
        orphan = {**grandchild, "ppid": 0}
        reused = {**grandchild, "start": "new-process", "ppid": 0}
        samples = [sample(1, [root, child, grandchild]), sample(2, [orphan]), sample(3, [reused])]
        hits = host.process_activity(samples, [host.identity(root)], host.POLICY)
        self.assertEqual([(hit["pid"], hit["start"]) for hit in hits], [(3, "new-process")])

    def test_snapshot_failure_records_error_without_process_control(self):
        with patch.object(host.subprocess, "run", side_effect=OSError("denied")):
            result = host.snapshot("test")
        self.assertIn("denied", result["error"])

if __name__ == "__main__":
    unittest.main()
