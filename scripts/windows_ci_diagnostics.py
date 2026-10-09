"""Fixed Windows collector: structured diagnostics only, never raw Stata logs.

Installed, hash-pinned copies run on the Windows controller and local wrapper.
The source archive cannot select a collector, filename or output field.
"""
import argparse
import hashlib
import json
import time
import os
from pathlib import Path
import re
import stat

STAGES = (
    'startup', 'toolchain', 'source_identity', 'cargo_build', 'exact_fma',
    'fused_kernels', 'c_bridge', 'numeric_guards', 'plugin_link', 'binary_audit',
    'native_loader', 'build_complete', 'checksum', 'stata_smoke',
    'single_runtime', 'complete',
)
REASONS = ('none', 'command_failed', 'check_failed', 'tool_missing',
           'file_missing', 'stata_error')
OUTCOMES = ('not_run', 'executable_missing', 'launch_failed', 'exited_without_status',
            'invalid_status', 'reported_failure', 'timeout', 'cleanup_failed', 'passed')
HEX = re.compile(r'[0-9a-f]{64}\Z')
RAW_KEYS = {'schema', 'status', 'stage', 'completed', 'native_rc', 'stata_rc',
            'reason', 'plugin_sha256', 'stage_started_ms'}
DIAGNOSTIC_KEYS = (RAW_KEYS - {'schema', 'stage_started_ms'}) | {
    'available', 'stage_elapsed_ms', 'compiler_errors'}


def strict_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError('duplicate_key')
            result[key] = value
        return result

    def invalid(_):
        raise ValueError('nonfinite_number')
    return json.loads(data, object_pairs_hook=pairs, parse_constant=invalid)


def read_bound(path, limit):
    path = Path(os.path.abspath(path))
    for part in (path, *path.parents):
        info = part.lstat()
        if stat.S_ISLNK(info.st_mode) or getattr(info, 'st_file_attributes', 0) & 0x400:
            raise ValueError('linked_path')
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_BINARY', 0))
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or not 0 < before.st_size <= limit:
            raise ValueError('file_bound')
        with os.fdopen(fd, 'rb', closefd=False) as stream:
            data = stream.read(limit + 1)
        after = os.fstat(fd)
        fields = ('st_dev', 'st_ino', 'st_size', 'st_mtime_ns', 'st_nlink')
        if len(data) != before.st_size or any(getattr(before, x) != getattr(after, x) for x in fields):
            raise ValueError('file_changed')
        return data
    finally:
        os.close(fd)


def integer(value, low, high):
    return type(value) is int and low <= value <= high


def validate_state(state):
    if state['status'] not in ('RUNNING', 'FAIL', 'PASS') or state['stage'] not in STAGES:
        raise ValueError('state_enum')
    if state['reason'] not in REASONS:
        raise ValueError('reason_enum')
    index = STAGES.index(state['stage'])
    if state['completed'] != list(STAGES[:index]):
        raise ValueError('stage_order')
    if state['native_rc'] is not None and not (integer(state['native_rc'], -2147483648, 2147483647) and state['native_rc'] != 0):
        raise ValueError('native_rc')
    if state['stata_rc'] is not None and not integer(state['stata_rc'], 1, 99999):
        raise ValueError('stata_rc')
    if state['plugin_sha256'] is not None and not (isinstance(state['plugin_sha256'], str) and HEX.fullmatch(state['plugin_sha256'])):
        raise ValueError('plugin_hash')
    if state['status'] == 'PASS' and (state['stage'] != 'complete' or state['plugin_sha256'] is None):
        raise ValueError('incomplete_pass')
    if state['status'] != 'FAIL' and (state['reason'] != 'none' or state['native_rc'] is not None or state['stata_rc'] is not None):
        raise ValueError('nonfailure_error')
    if state['status'] == 'FAIL' and state['reason'] == 'none':
        raise ValueError('missing_failure_reason')


def compiler_errors(root):
    """Only Rust error codes and repository-relative source locations escape."""
    errors = []
    try:
        data = read_bound(Path(root) / 'target/native-windows/cargo-build.jsonl', 4 * 1024 * 1024)
        for line in data.splitlines():
            message = strict_json(line)
            if not isinstance(message, dict) or message.get('reason') != 'compiler-message':
                continue
            detail = message.get('message', {})
            if not isinstance(detail, dict) or detail.get('level') != 'error':
                continue
            code = detail.get('code') or {}
            code = code.get('code') if isinstance(code, dict) else None
            if not isinstance(code, str) or not re.fullmatch(r'E[0-9]{4}', code):
                continue
            for span in detail.get('spans', []):
                if not isinstance(span, dict) or span.get('is_primary') is not True:
                    continue
                filename, number = span.get('file_name'), span.get('line_start')
                if isinstance(filename, str):
                    filename = filename.replace('\\', '/')
                if (isinstance(filename, str) and len(filename) <= 200 and
                    re.fullmatch(r'(?:crates|plugin)/[A-Za-z0-9_/-]+\.(?:rs|c|h)', filename) and
                    '..' not in filename and integer(number, 1, 1000000)):
                    entry = {'code': code, 'file': filename, 'line': number}
                    if entry not in errors:
                        errors.append(entry)
                    if len(errors) == 8:
                        return errors
    except (OSError, ValueError, TypeError, UnicodeError):
        pass
    return errors


def unavailable():
    return {'available': False, 'status': 'UNAVAILABLE', 'stage': 'unknown',
            'completed': [], 'native_rc': None, 'stata_rc': None, 'reason': 'unavailable',
            'plugin_sha256': None, 'stage_elapsed_ms': None, 'compiler_errors': []}


def collect(root):
    try:
        raw = strict_json(read_bound(Path(root) / 'windows-ci-diagnostics.json', 8192))
        if not isinstance(raw, dict) or set(raw) != RAW_KEYS or raw['schema'] != 'FEREG-WINDOWS-DIAGNOSTICS-V1':
            raise ValueError('diagnostic_schema')
        validate_state(raw)
        if raw['status'] == 'PASS':
            plugin = read_bound(Path(root) / 'dist/fereg_windows.plugin', 64 * 1024 * 1024)
            if hashlib.sha256(plugin).hexdigest() != raw['plugin_sha256']:
                raise ValueError('plugin_hash_mismatch')
        started = raw['stage_started_ms']
        if not integer(started, 1, 4000000000000):
            raise ValueError('timestamp')
        elapsed = int(time.time() * 1000) - started
        if not 0 <= elapsed <= 3600000:
            raise ValueError('elapsed_bound')
        result = {k: raw[k] for k in RAW_KEYS - {'schema', 'stage_started_ms'}}
        result.update(available=True, stage_elapsed_ms=elapsed, compiler_errors=compiler_errors(root))
        return result
    except (OSError, ValueError, TypeError, KeyError, UnicodeError):
        return unavailable()


def validate_diagnostics(value):
    if not isinstance(value, dict) or set(value) != DIAGNOSTIC_KEYS or type(value['available']) is not bool:
        raise ValueError('diagnostic_fields')
    if not value['available']:
        if value != unavailable():
            raise ValueError('unavailable_fields')
        return
    validate_state(value)
    if not integer(value['stage_elapsed_ms'], 0, 3600000):
        raise ValueError('elapsed_bound')
    errors = value['compiler_errors']
    if not isinstance(errors, list) or len(errors) > 8:
        raise ValueError('compiler_errors')
    for entry in errors:
        if (not isinstance(entry, dict) or set(entry) != {'code', 'file', 'line'} or
            not isinstance(entry['code'], str) or not re.fullmatch(r'E[0-9]{4}', entry['code']) or
            not isinstance(entry['file'], str) or len(entry['file']) > 200 or
            not re.fullmatch(r'(?:crates|plugin)/[A-Za-z0-9_/-]+\.(?:rs|c|h)', entry['file']) or
            '..' in entry['file'] or not integer(entry['line'], 1, 1000000)):
            raise ValueError('compiler_location')


def validate_receipt(value, run, revision, source_hash, controller_hash, guard_hash):
    keys = {'schema_version', 'run_id', 'project', 'profile', 'source_revision',
            'source_sha256', 'windows_target', 'execution_identity', 'status',
            'error_code', 'checks', 'completed_at_utc', 'controller_sha256',
            'diagnostic_guard_sha256', 'process', 'diagnostics'}
    if not isinstance(value, dict) or set(value) != keys or value['schema_version'] != 4:
        raise ValueError('receipt_schema')
    if (value['run_id'], value['source_revision'], value['source_sha256'], value['controller_sha256'], value['diagnostic_guard_sha256']) != (run, revision, source_hash, controller_hash, guard_hash):
        raise ValueError('receipt_identity')
    if value['project'] != 'fereg' or value['windows_target'] != 'x86_64' or value['execution_identity'] != 'SYSTEM':
        raise ValueError('receipt_target')
    if value['profile'] not in ('stata-do', 'stata-smoke', 'fail-smoke', 'cargo-test') or value['status'] not in ('PASS', 'FAIL'):
        raise ValueError('receipt_status')
    if value['error_code'] is not None and (not isinstance(value['error_code'], str) or not re.fullmatch(r'[A-Z][A-Z0-9_]{1,63}', value['error_code'])):
        raise ValueError('controller_error_code')
    process = value['process']
    if not isinstance(process, dict) or set(process) != {'outcome', 'elapsed_ms', 'exit_code'} or process['outcome'] not in OUTCOMES:
        raise ValueError('process_schema')
    if not integer(process['elapsed_ms'], 0, 1000000) or (process['exit_code'] is not None and not integer(process['exit_code'], -2147483648, 2147483647)):
        raise ValueError('process_numbers')
    checks = value['checks']
    required = {'execution_identity', 'source_hash', 'archive_paths', 'source_manifest', 'shutdown_backstop', 'stata', 'project_test'}
    if not isinstance(checks, dict) or set(checks) != required or any(v is not None and type(v) is not bool for v in checks.values()):
        raise ValueError('checks_schema')
    validate_diagnostics(value['diagnostics'])
    if value['status'] == 'PASS':
        if value['profile'] == 'fail-smoke':
            raise ValueError('deliberate_failure_cannot_pass')
        if value['error_code'] is not None or not all(checks[x] is True for x in required - {'stata'}):
            raise ValueError('pass_checks')
        if value['profile'] in ('stata-do', 'stata-smoke') and (checks['stata'] is not True or process['outcome'] != 'passed'):
            raise ValueError('pass_process')
        if value['profile'] == 'stata-do' and (not value['diagnostics']['available'] or value['diagnostics']['status'] != 'PASS'):
            raise ValueError('pass_diagnostics')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=('collect', 'receipt'))
    parser.add_argument('--root', type=Path, required=True)
    for flag in ('run', 'revision', 'source-hash', 'controller-hash', 'guard-hash'):
        parser.add_argument('--' + flag)
    args = parser.parse_args()
    if args.action == 'collect':
        print(json.dumps(collect(args.root), sort_keys=True, separators=(',', ':')))
        return
    value = strict_json(read_bound(args.root, 32768))
    validate_receipt(value, args.run, args.revision, args.source_hash, args.controller_hash, args.guard_hash)
    d, process = value['diagnostics'], value['process']
    print('WINDOWS_CONTROLLER_OUTCOME=' + process['outcome'])
    print('WINDOWS_PROCESS_ELAPSED_MS=' + str(process['elapsed_ms']))
    print('WINDOWS_LAST_STAGE=' + d['stage'])
    print('WINDOWS_STAGE_REASON=' + d['reason'])
    for field in ('native_rc', 'stata_rc'):
        if d[field] is not None:
            print('WINDOWS_' + field.upper() + '=' + str(d[field]))
    for entry in d['compiler_errors']:
        print('WINDOWS_RUST_ERROR=' + entry['code'] + ':' + entry['file'] + ':' + str(entry['line']))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, TypeError, KeyError, UnicodeError):
        print('WINDOWS_DIAGNOSTIC_VALIDATION=FAIL')
        raise SystemExit(1)
