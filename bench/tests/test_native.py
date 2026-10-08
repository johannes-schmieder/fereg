"""Native tooling witnesses: framed transport, full repetitions and bad output."""
import copy
import hashlib
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import native
import native_io


def bits(value):
    return struct.pack('>d', value).hex()


def result(poisson=False):
    matrices = {name: {'rows': 1, 'columns': 1, 'bits': [bits(1)]}
                for name in ('RB', 'RV', 'RSTATUS', 'RFIT', 'RDOF', 'RFE', 'RCLUSTER',
                             'RSUBSET', 'RCERT', 'RMEMORY', 'RREC', 'RMARKB', 'RMARKV',
                             'RINF', 'RDROP', 'RSEP', 'RSEQ')}
    matrices['RRUNTIME'] = {'rows': 108 if poisson else 30, 'columns': 1 if poisson else 2,
                           'bits': [bits(1)] * (108 if poisson else 60)}
    matrices['RSOLVER'] = {'rows': 4 if poisson else 2, 'columns': 22 if poisson else 16,
                          'bits': [bits(0)] * (88 if poisson else 32)}
    return {'schema': 1, 'return_code': 0, 'matrices': matrices,
            'columns': [[bits(1), bits(2)], [bits(3), bits(4)]], 'error': ''}


def case_bytes(arguments=None):
    arguments = arguments or [b'estimate-ols', b'threads=1']
    prefix = f'FEREG_REPLAY_V1\n2 2 {len(arguments)} 1 0\n'.encode()
    return prefix + b''.join(str(len(a)).encode() + b'\n' + a + b'\n' for a in arguments) + b'body\n9\nthreads=1\n'


class NativeTransport(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)

    def write_result(self, value):
        path = self.root / 'result.json'
        path.write_text(json.dumps(value, separators=(',', ':')) + '\n')
        return path

    def test_streaming_head_tail_and_hash(self):
        value = result()
        value['columns'] = [[bits(float(i)) for i in range(9000)]] * 2
        path = self.write_result(value)
        with patch.object(Path, 'read_bytes', side_effect=AssertionError('unbounded read')):
            parsed = native_io.read_result(path, 9000, 2)
        self.assertNotIn('columns', parsed['head'])
        self.assertEqual(parsed['columns_sha256'], hashlib.sha256(
            json.dumps(value['columns'], separators=(',', ':')).encode()).hexdigest())
        self.assertEqual(native_io.diagnostics(parsed['head'], False)['native_s'], 1)

    def test_every_truncated_tail_fails(self):
        path = self.write_result(result())
        raw = path.read_bytes()
        split = raw.index(b'},"columns":[')
        for offset in range(split, len(raw) - 2):
            path.write_bytes(raw[:offset])
            with self.subTest(offset=offset), self.assertRaises((ValueError, json.JSONDecodeError)):
                native_io.read_result(path, 2, 2)

    def test_bad_numeric_cells_and_dimensions(self):
        for columns in ([[bits(1)], [bits(1)]], [[bits(1), 'z' * 16]] * 2,
                        [[bits(1), bits(2)]] * 3):
            value = result()
            value['columns'] = columns
            with self.subTest(columns=columns), self.assertRaises(ValueError):
                native_io.read_result(self.write_result(value), 2, 2)

    def test_native_failure_missing_matrix_and_certificate(self):
        for change in ('native', 'missing', 'certificate', 'layout'):
            value = result()
            if change == 'native':
                value['return_code'] = 498
            elif change == 'missing':
                del value['matrices']['RB']
            elif change == 'certificate':
                value['matrices']['RCERT']['bits'] = [bits(0)]
            else:
                value['matrices']['RV']['rows'] = 2
            with self.subTest(change=change), self.assertRaises(ValueError):
                native_io.read_result(self.write_result(value), 2, 2)

    def test_error_is_retained_and_duplicate_keys_fail(self):
        value = result()
        value['error'] = 'message with "] and \\" escapes'
        path = self.write_result(value)
        with self.assertRaises(ValueError):
            native_io.read_result(path, 2, 2)
        path.write_text(path.read_text().replace('"schema":1', '"schema":1,"schema":1'))
        with self.assertRaises(ValueError):
            native_io.read_result(path, 2, 2)

    def test_framed_rewrite_never_changes_body(self):
        source, target = self.root / 'in.case', self.root / 'out.case'
        source.write_bytes(case_bytes())
        meta = native_io.prepare_case(source, target, 16)
        with target.open('rb') as stream:
            _, args = native_io.case_header(stream)
            self.assertEqual(args[-1], b'threads=16')
            self.assertEqual(stream.read(), b'body\n9\nthreads=1\n')
        self.assertEqual(meta, {'rows': 2, 'columns': 2, 'poisson': False})

    def test_bad_frames_and_duplicate_threads(self):
        malformed = [case_bytes().replace(b'9\nthreads', b'8\nthreads'),
                     case_bytes([b'estimate-ols', b'threads=1', b'threads=2']),
                     case_bytes([b'estimate-ols']), b'bad\n']
        for raw in malformed:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                native_io.case_header(io.BytesIO(raw))

    def test_every_repeat_and_cross_plugin_compared(self):
        parsed = native_io.read_result(self.write_result(result()), 2, 2)
        refs = {}
        native.compare_run(refs, 'A', parsed, False)
        native.compare_run(refs, 'B', parsed, False)
        for label in ('A', 'B'):
            later = copy.deepcopy(parsed)
            later['head']['matrices']['RB']['bits'][0] = bits(2)
            with self.subTest(label=label), self.assertRaises(ValueError):
                native.compare_run(refs, label, later, False)
        columns = copy.deepcopy(parsed)
        columns['columns_sha256'] = 'wrong'
        with self.assertRaises(ValueError):
            native.compare_run(refs, 'B', columns, False)

    def test_only_telemetry_changes_are_ignored(self):
        parsed = native_io.read_result(self.write_result(result()), 2, 2)
        later = copy.deepcopy(parsed)
        later['head']['matrices']['RRUNTIME']['bits'][39] = bits(0.123456789)
        later['head']['matrices']['RSOLVER']['bits'][8] = bits(0.987654321)
        self.assertEqual(native_io.exact_differences(parsed, later, False), [])
        later['head']['matrices']['RSOLVER']['bits'][1] = bits(3)
        self.assertTrue(native_io.exact_differences(parsed, later, False))

    def test_ppml_runtime_is_total_not_ols_offsets(self):
        parsed = native_io.read_result(self.write_result(result(True)), 2, 2)
        diag = native_io.diagnostics(parsed['head'], True)
        self.assertEqual(diag['timing_field'], 'total_s')
        self.assertNotIn('native_s', diag)
        self.assertEqual(len(diag['solver_rows']), 4)
        self.assertEqual(len(diag['solver_rows'][0]), 22)

    def test_cachegrind_compressed_functions_and_totals(self):
        path = self.root / 'cachegrind'
        path.write_text('events: Ir\nfn=(1) first\n10 5\n+1 2\n'
                        'fn=(2) second\n20 3\nfn=(1)\n* 4\nsummary: 14\n')
        self.assertEqual(native_io.cachegrind_counts(path),
                         {'total_Ir': 14, 'functions_Ir': {'first': 11, 'second': 3}})
        path.write_text(path.read_text().replace('summary: 14', 'summary: 15'))
        with self.assertRaises(ValueError):
            native_io.cachegrind_counts(path)

    def test_runner_fails_on_later_repeat_and_preserves_partial_manifest(self):
        source = self.root / 'in.case'
        source.write_bytes(case_bytes())
        plugin = self.root / 'plugin'
        plugin.write_bytes(b'plugin')
        output = self.root / 'run'
        def build(directory, rows, cells, timeout):
            binary = directory / 'harness'
            binary.write_bytes(b'harness')
            return binary, ['fake-build']
        calls = []
        def run(command, log, timeout):
            calls.append(command)
            value = result()
            if len(calls) == 3:
                value['matrices']['RB']['bits'][0] = bits(2)
            Path(command[-1]).write_text(json.dumps(value, separators=(',', ':')))
            return {'wall_s': 0.1, 'user_s': 0.05, 'system_s': 0.01, 'maxrss_bytes': 1000}
        argv = ['native.py', '--plugin-a', str(plugin), '--plugin-b', str(plugin),
                '--cases', str(source), '--threads', '1', '--repeats', '2',
                '--timeout', '10', '--budget', '100', '--output', str(output)]
        with patch.object(sys, 'argv', argv), patch.object(native, 'output_under_target', lambda p: p), \
                patch.object(native, 'build_harness', build), patch.object(native, 'run_process', run), \
                patch.object(sys, 'stderr', io.StringIO()):
            self.assertEqual(native.main(), 1)
        manifest = json.loads((output / 'run.json').read_text())
        self.assertEqual(manifest['status'], 'failed')
        self.assertEqual([r['plugin'] for r in manifest['records']], ['A', 'B', 'B'])
        self.assertIn('differs', manifest['error'])
        self.assertEqual(manifest['summary'], [])

    def test_rss_units_and_child_failure_timeout(self):
        self.assertEqual(native.rss_bytes(2048, 'Darwin'), 2048)
        self.assertEqual(native.rss_bytes(2, 'Linux'), 2048)
        with self.assertRaises(RuntimeError):
            native.run_process([sys.executable, '-c', 'raise SystemExit(4)'], self.root / 'fail.log', 2)
        with self.assertRaises(TimeoutError):
            native.run_process([sys.executable, '-c', 'import time; time.sleep(5)'], self.root / 'timeout.log', 0.05)
        record = native.run_process([sys.executable, '-c', 'pass'], self.root / 'ok.log', 2)
        self.assertGreater(record['maxrss_bytes'], 0)


if __name__ == '__main__':
    unittest.main()
