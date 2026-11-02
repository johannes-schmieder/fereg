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
import input_diagnostics as diagnostics


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

    def test_preflight_checks_each_worker_and_rejects_cross_worker_drift(self):
        with tempfile.TemporaryDirectory() as name, patch.object(subject, 'CELLS', ('c11',)):
            calls = []
            def run(config, arm, args, environment):
                threads = int(args.split('"')[-2])
                calls.append((arm, threads))
                trace = self.trace().replace('threads=4', f'threads={threads}')
                if threads == 8:
                    trace = trace.replace('0123456789abcdef', 'fedcba9876543210')
                Path(environment['FEREG_SOLVER_TRACE_FILE']).write_text(trace)
            with patch.object(subject, 'run_arm', run):
                with self.assertRaisesRegex(RuntimeError, 'across arms or workers'):
                    subject.command_verify(dict(output=name, data_dir=name, threads=[4, 8],
                                                arms={'incumbent': {}, 'candidate': {}}))
            self.assertEqual(calls, [('incumbent', 4), ('incumbent', 8),
                                     ('candidate', 4), ('candidate', 8)])

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


class InputDiagnostics(unittest.TestCase):
    def test_scope_instrumentation_keeps_calls_and_drops_only_after_commit(self):
        root = Path(__file__).resolve().parents[2] / 'stata'
        for kind, wrapper in [('ols', '_fereg_estimate_public'), ('ppml', '_fereg_ppml')]:
            original = (root / f'_fereg_estimate_{kind}.ado').read_text()
            automatic = diagnostics.instrument_ado(original)
            explicit = diagnostics.instrument_ado(original, True)
            drop = "if scalar(__diag_marker_live) quietly drop `touse'"
            self.assertNotIn(drop, automatic)
            self.assertEqual(explicit.count(drop), 1)
            self.assertLess(explicit.index('_estimates unhold'), explicit.index(drop))
            self.assertLess(explicit.index(drop), explicit.index('\nend'))
            self.assertEqual(explicit.count("plugin call `plugin_handle'"), 1)
            outer = diagnostics.instrument_wrapper((root / f'{wrapper}.ado').read_text(),
                                                   f'_fereg_estimate_{kind}')
            call = outer.index(f'    _fereg_estimate_{kind} ')
            self.assertLess(outer.index('timer on 95'), call)
            self.assertGreater(outer.index('timer off 95'), call)
            self.assertLess(outer.index('scalar __diag_public_post'), outer.index('\nend'))

    def test_residual_timings_are_computed_per_call_and_grid_is_complete(self):
        # Medians of differences differ from differences of medians here.
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            config = dict(arms={'incumbent': {}, 'candidate': {}},
                          placements=['default'], host_modes=['default'],
                          blocks=2, reps=3, cells=['c3'], threads=[16])
            rows = []
            for arm in config['arms']:
                for block in (1, 2):
                    for rep, (wall, plugin, snapshot) in enumerate(
                            [(10, 9, 8), (11, 3, 2), (100, 99, 1)], 1):
                        rows.append(dict(zip(diagnostics.FIELDS,
                            [arm, 'default', 'default', 4, block, 'c3', 16, rep,
                             1000, wall, .1, plugin, .1, .1, .1, snapshot,
                             .1, plugin + 1, .1, .01, 0, 1, 0])))
            def report(records):
                with (root / 'results.csv').open('w') as handle:
                    writer = csv.DictWriter(handle, diagnostics.FIELDS)
                    writer.writeheader()
                    writer.writerows(records)
                return diagnostics.summarize(config, root)
            result = report(rows)
            self.assertEqual(result['observations'], 12)
            for arm in config['arms']:
                metrics = result['rows'][0][arm]
                self.assertEqual(metrics['outside_plugin_s'], 1)
                self.assertEqual(metrics['plugin_tail_s'], 1)
            for broken in (rows[:-1], [rows[0], *rows[:-1]]):
                with self.assertRaises(RuntimeError):
                    report(broken)
            for value in ('nan', 'inf', -1):
                bad = [dict(row) for row in rows]
                bad[0]['plugin_s'] = value
                with self.assertRaises(RuntimeError):
                    report(bad)


if __name__ == '__main__':
    unittest.main()
