"""Reject incomplete input-transport qualification evidence."""
import csv
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import parallel_input as subject


class InputQualification(unittest.TestCase):
    def trace(self, cell='c11'):
        return '\n'.join([
            f'FEREG_SOLVER_TRACE|phase=input_benchmark|cell={cell}|threads=4',
            *(['FEREG_SOLVER_TRACE|phase=input_transport|mode=parallel|range_first=4995000|range_end=5005000',
               'FEREG_SOLVER_TRACE|phase=input_digest|digest=0123456789abcdef'] * 2),
        ])

    def test_cell_path_and_digest_coverage(self):
        with patch.object(subject, 'CELLS', ('c11',)):
            trace = self.trace()
            self.assertEqual(len(subject.verify_trace(trace, 'candidate', 4)['c11']), 2)
            for broken in [trace.replace('|mode=parallel', '|mode=serial'),
                           trace.replace('range_first=4995000', 'range_first=0'),
                           trace.rsplit('\n', 1)[0], trace + '\n' + trace]:
                with self.assertRaises(RuntimeError):
                    subject.verify_trace(broken, 'candidate', 4)
        with patch.object(subject, 'CELLS', ('c1', 'c11')):
            with self.assertRaises(RuntimeError):
                subject.verify_trace(trace, 'candidate', 4)

    def test_report_requires_complete_finite_unique_observations(self):
        with tempfile.TemporaryDirectory() as name, patch.object(subject, 'CELLS', ('c1',)):
            root = Path(name)
            config = dict(output=name, threads=[4], blocks=2, reps=2)
            rows = [dict(zip(subject.FIELDS, [arm, block, 'c1', 4, rep, 1000, 1000,
                                            .01, .003, .008, 1000]))
                    for arm in ('incumbent', 'candidate') for block in (1, 2) for rep in (1, 2)]
            def report(records):
                with (root / 'results.csv').open('w') as handle:
                    writer = csv.DictWriter(handle, subject.FIELDS)
                    writer.writeheader()
                    writer.writerows(records)
                return subject.command_report(config)
            self.assertEqual(report(rows)['status'], 'passed')
            self.assertEqual(json.loads((root / 'summary.json').read_text())['observations'], 8)
            with self.assertRaises(RuntimeError):
                report([rows[0], *rows[:-1]])
            for value in ('nan', 'inf', -1):
                bad = [dict(row) for row in rows]
                bad[0]['wall_s'] = value
                with self.assertRaises(RuntimeError):
                    report(bad)
            bad = [dict(row) for row in rows]
            for row in bad:
                if row['arm'] == 'candidate': row['wall_s'] = .02
            self.assertEqual(report(bad)['status'], 'regression')


if __name__ == '__main__':
    unittest.main()
