"""Campaign PPML case writer: protocol request, column layout and CLI guards."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ppml_cases


class PpmlCases(unittest.TestCase):
    def test_template_request_fixes_the_lean_protocol(self):
        command = ppml_cases.template_command(Path('/gen'), Path('/out.case'), 16)
        self.assertEqual(command[:4], ['/gen', '--ppml', '--output', '/out.case'])
        for flag, value in [('--regressors', '16'), ('--vce', 'robust'), ('--tolerance', '1e-8'),
                            ('--iterations', '10000'), ('--separation', 'fe simplex ir'), ('--solver', 'auto')]:
            self.assertEqual(command[command.index(flag) + 1], value)

    def test_frame_columns_follow_the_case_layout(self):
        frame = pd.DataFrame({'g2': [1., 2.], 'x2': [3., 4.], 'y': [5., 6.], 'x1': [7., 8.], 'g1': [9., 10.]})
        columns = ppml_cases.frame_columns(frame, 2)
        self.assertEqual([c.tolist() for c in columns], [[5., 6.], [7., 8.], [3., 4.], [9., 10.], [1., 2.]])
        self.assertTrue(all(c.dtype == np.float64 and c.flags['C_CONTIGUOUS'] for c in columns))

    def test_write_rejects_non_ppml_template_and_bad_columns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            template_bytes = (b'FEREG_REPLAY_V1\n2 1 2 1 0\n' + b'12\nestimate-ols\n' + b'9\nthreads=1\n'
                              + b'0\n' + b'0000000000000000\n' * 2)
            def fake_run(command, **kwargs):
                Path(command[3]).write_bytes(template_bytes)
            with patch.object(ppml_cases.subprocess, 'run', side_effect=fake_run):
                with self.assertRaisesRegex(ValueError, 'PPML template'):
                    ppml_cases.write_ppml_case(Path('/gen'), root / 'a.case', [np.zeros(2)] * 4)
            self.assertFalse((root / 'a.template.case').exists())

    def test_cli_requires_task_output_under_target_tmp(self):
        with patch.object(sys, 'argv', ['ppml_cases.py', '--output', '/not/target']), \
                patch('sys.stderr'), self.assertRaises(SystemExit) as caught:
            ppml_cases.main()
        self.assertEqual(caught.exception.code, 2)


if __name__ == '__main__':
    unittest.main()
