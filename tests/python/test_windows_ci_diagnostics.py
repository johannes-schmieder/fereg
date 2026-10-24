import copy
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('windows_diagnostics', ROOT / 'scripts/windows_ci_diagnostics.py')
diag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diag)


class WindowsDiagnosticsTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.report = self.root / 'windows-ci-diagnostics.json'
        self.state = dict(schema='FEREG-WINDOWS-DIAGNOSTICS-V1', status='FAIL',
                          stage='cargo_build', completed=list(diag.STAGES[:3]),
                          native_rc=101, stata_rc=601, reason='command_failed',
                          plugin_sha256=None, stage_started_ms=int(time.time() * 1000) - 1000)

    def tearDown(self):
        self.directory.cleanup()

    def write(self):
        self.report.write_text(json.dumps(self.state))

    def receipt(self):
        return dict(schema_version=4, run_id='win-20261009T000000Z-12345678', project='fereg',
                    profile='stata-do', source_revision='commit-' + 'a' * 40,
                    source_sha256='b' * 64, controller_sha256='c' * 64,
                    diagnostic_guard_sha256='d' * 64, windows_target='x86_64',
                    execution_identity='SYSTEM', status='FAIL', error_code='STATA_DRIVER_FAILED',
                    checks=dict(execution_identity=True, source_hash=True, archive_paths=True,
                                source_manifest=True, shutdown_backstop=True, stata=False, project_test=False),
                    completed_at_utc='2026-10-09T00:00:00.0000000Z',
                    process=dict(outcome='reported_failure', elapsed_ms=3000, exit_code=601),
                    diagnostics=diag.collect(self.root))

    def validate(self, receipt):
        diag.validate_receipt(receipt, 'win-20261009T000000Z-12345678', 'commit-' + 'a' * 40,
                              'b' * 64, 'c' * 64, 'd' * 64)

    def test_windows_kernel_summary_accepts_growing_suite_but_not_skips(self):
        script = (ROOT / "scripts/build_windows.ps1").read_text()
        pattern = re.search(r"\$KernelText -match '([^']+)'", script).group(1)
        for count in (3, 8, 12):
            self.assertRegex(f"test result: ok. {count} passed; 0 failed; 0 ignored;", pattern)
        for summary in ("test result: ok. 0 passed; 0 failed; 0 ignored;",
                        "test result: FAILED. 7 passed; 1 failed; 0 ignored;",
                        "test result: ok. 7 passed; 0 failed; 1 ignored;"):
            self.assertNotRegex(summary, pattern)
        self.assertIn("skipped: this processor has no FMA/AVX hardware path", script)

    def test_native_failure_survives_stata_missing_file(self):
        self.write()
        result = diag.collect(self.root)
        self.assertEqual((result['stage'], result['native_rc'], result['stata_rc']), ('cargo_build', 101, 601))
        self.assertGreaterEqual(result['stage_elapsed_ms'], 1000)
        self.validate(self.receipt())

    def test_missing_and_oversized_reports_are_unavailable(self):
        self.assertEqual(diag.collect(self.root), diag.unavailable())
        self.report.write_text('x' * 8193)
        self.assertEqual(diag.collect(self.root), diag.unavailable())

    def test_unknown_or_sensitive_fields_do_not_escape(self):
        self.state['message'] = 'license banner, account id, password'
        self.write()
        self.assertEqual(diag.collect(self.root), diag.unavailable())

    def test_duplicate_json_key_is_rejected(self):
        self.write()
        self.report.write_text(self.report.read_text()[:-1] + ',"status":"PASS"}')
        self.assertEqual(diag.collect(self.root), diag.unavailable())

    def test_invalid_stages_codes_and_clock_fail_closed(self):
        for field, value in [('stage', 'private-path'), ('completed', ['startup']),
                             ('native_rc', True), ('native_rc', 0), ('stata_rc', -1),
                             ('reason', 'arbitrary exception'), ('stage_started_ms', float('nan')),
                             ('stage_started_ms', int(time.time() * 1000) + 100000)]:
            with self.subTest(field=field, value=value):
                saved = copy.deepcopy(self.state)
                self.state[field] = value
                self.write()
                self.assertEqual(diag.collect(self.root), diag.unavailable())
                self.state = saved

    def test_links_and_hardlinks_are_not_read(self):
        original = self.root / 'original'
        original.write_text(json.dumps(self.state))
        self.report.symlink_to(original)
        self.assertEqual(diag.collect(self.root), diag.unavailable())
        self.report.unlink()
        os.link(original, self.report)
        self.assertEqual(diag.collect(self.root), diag.unavailable())

    def test_incomplete_or_wrong_binary_cannot_pass(self):
        self.state.update(status='PASS', stage='complete', completed=list(diag.STAGES[:-1]),
                          reason='none', native_rc=None, stata_rc=None, plugin_sha256='a' * 64)
        self.write()
        self.assertFalse(diag.collect(self.root)['available'])
        (self.root / 'dist').mkdir()
        plugin = self.root / 'dist/fereg_windows.plugin'
        plugin.write_bytes(b'fixture')
        self.assertFalse(diag.collect(self.root)['available'])
        self.state['plugin_sha256'] = hashlib.sha256(plugin.read_bytes()).hexdigest()
        self.write()
        self.assertTrue(diag.collect(self.root)['available'])
        self.state['completed'].pop()
        self.write()
        self.assertFalse(diag.collect(self.root)['available'])

    def test_compiler_output_exports_only_error_code_and_safe_location(self):
        self.write()
        directory = self.root / 'target/native-windows'
        directory.mkdir(parents=True)
        message = dict(reason='compiler-message', message=dict(level='error', code=dict(code='E0063'),
                       message='must never export this arbitrary text', spans=[
                           dict(is_primary=True, file_name='crates/fereg-plugin/src/lib.rs', line_start=12),
                           dict(is_primary=True, file_name='C:/private/license.txt', line_start=1)]))
        (directory / 'cargo-build.jsonl').write_text(json.dumps(message) + '\n')
        result = diag.collect(self.root)
        self.assertEqual(result['compiler_errors'], [dict(code='E0063', file='crates/fereg-plugin/src/lib.rs', line=12)])
        self.assertNotIn('arbitrary', json.dumps(result))

    def test_powershell_utf8_bom_multiline_cargo_output(self):
        self.write()
        directory = self.root / 'target/native-windows'
        directory.mkdir(parents=True)
        rows = [dict(reason='compiler-artifact', executable='private path'),
                dict(reason='compiler-message', message=dict(level='error', code=dict(code='E0063'),
                     spans=[dict(is_primary=True, file_name=r'crates\fereg-plugin\src\lib.rs', line_start=12)]))]
        text = '\r\n'.join(json.dumps(row) for row in rows) + '\r\n'
        (directory / 'cargo-build.jsonl').write_bytes(text.encode('utf-8-sig'))
        self.assertEqual(diag.collect(self.root)['compiler_errors'], [
            dict(code='E0063', file='crates/fereg-plugin/src/lib.rs', line=12)])

    def test_receipt_identity_and_unknown_fields_are_rejected(self):
        self.write()
        for field, value in [('source_sha256', 'e' * 64), ('controller_sha256', 'e' * 64),
                             ('project', 'fevc'), ('message', 'secret')]:
            with self.subTest(field=field):
                receipt = self.receipt()
                receipt[field] = value
                with self.assertRaises(ValueError):
                    self.validate(receipt)

    def test_marker_alone_cannot_create_success(self):
        self.write()
        receipt = self.receipt()
        receipt.update(status='PASS', error_code=None)
        receipt['checks'] = {key: True for key in receipt['checks']}
        receipt['process']['outcome'] = 'passed'
        with self.assertRaises(ValueError):
            self.validate(receipt)
        receipt['diagnostics'] = diag.unavailable()
        with self.assertRaises(ValueError):
            self.validate(receipt)

    def test_timeout_is_distinct_from_failed_project_stage(self):
        self.state.update(status='RUNNING', native_rc=None, stata_rc=None, reason='none')
        self.write()
        receipt = self.receipt()
        receipt['process'].update(outcome='timeout', elapsed_ms=900000, exit_code=None)
        self.validate(receipt)
        self.assertEqual(receipt['diagnostics']['stage'], 'cargo_build')


if __name__ == '__main__':
    unittest.main()
