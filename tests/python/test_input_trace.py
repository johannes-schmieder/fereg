"""Unqualified-host evidence must prove absence of the numeric window."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class InputTraceTests(unittest.TestCase):
    def check_trace(self, *, window=False, same_digest=True, require_serial=True, orphan=False, wide=False, packed=None):
        lines = [
            'FEREG_SOLVER_TRACE|phase=case|label=example|serial',
            'FEREG_SOLVER_TRACE|phase=input_digest|digest=abc',
            'FEREG_SOLVER_TRACE|phase=case|label=example|parallel',
            'FEREG_SOLVER_TRACE|phase=input_digest|digest=' + ('abc' if same_digest else 'def'),
        ]
        if wide:
            lines = [line.replace('example|', 'ols_wide64|') for line in lines]
        if packed is not None:
            lines.append('FEREG_SOLVER_TRACE|phase=input_layout|reserved=true|packed_rhs=' + str(packed).lower())
        if orphan:
            lines.insert(0, 'FEREG_SOLVER_TRACE|phase=input_transport|mode=parallel')
        if window:
            lines.append('FEREG_SOLVER_TRACE|phase=input_transport|mode=parallel')
        with tempfile.TemporaryDirectory() as directory:
            trace = Path(directory) / 'trace.txt'
            trace.write_text('\n'.join(lines) + '\n')
            command = [sys.executable, str(ROOT / 'scripts/check_input_trace.py'), str(trace)]
            if require_serial:
                command.append('--require-serial')
            return subprocess.run(command, capture_output=True, text=True)

    def test_unqualified_host_keeps_exact_serial_inputs(self):
        result = self.check_trace()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('cases=1 runs=2 parallel_runs=0', result.stdout)

    def test_unqualified_host_rejects_an_open_window(self):
        result = self.check_trace(window=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('the numeric window ran', result.stdout)

    def test_unqualified_host_rejects_a_window_before_any_case(self):
        result = self.check_trace(orphan=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('the numeric window ran before the first case', result.stdout)

    def test_fallback_does_not_relax_input_equality(self):
        result = self.check_trace(same_digest=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('distinct input digests', result.stdout)

    def test_qualified_host_still_requires_its_window(self):
        self.assertNotEqual(self.check_trace(require_serial=False).returncode, 0)
        self.assertEqual(self.check_trace(window=True, require_serial=False).returncode, 0)

    def test_wide_fallback_requires_proof_of_packed_loading(self):
        for packed in (None, False):
            result = self.check_trace(wide=True, packed=packed)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('wide packed loading did not execute', result.stdout)
        self.assertEqual(self.check_trace(wide=True, packed=True).returncode, 0)
