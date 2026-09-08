#!/usr/bin/env python3
"""Bounded source-bound actual-auto capture/audit; never automatic admission.

This runner executes one frozen manifest scope. Comparator availability and
optimum-reference adjudication remain separate evidence, never inferred here.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT/'benchmarks/manifests/feppml_auto_science_admission_v3.json'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def exclusive_json(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write('\n')


def manifest_scope(task):
    manifest = json.loads(MANIFEST.read_text())
    scopes = [{**d, 'threads': threads} for d in manifest['datasets'] for threads in d['threads']]
    if (manifest['schema_version'] != 3 or manifest['qualification_eligible']
            or manifest['policy_refit'] or len(scopes) != manifest['expected_scopes']
            or type(task) is not int or not 1 <= task <= len(scopes)):
        raise ValueError('invalid prospective scope inventory or task')
    return manifest, scopes[task-1]


def verify_source(root, expected):
    if not expected:
        raise ValueError('empty source identity')
    for relative, checksum in expected.items():
        path = Path(relative)
        if path.is_absolute() or '..' in path.parts or not path.parts:
            raise ValueError('source identity path escapes root')
        if digest(root/path) != checksum:
            raise ValueError('source identity changed: '+relative)


def run_process(command, environment, output, seconds, sentinel):
    if seconds <= 0:
        raise ValueError('process budget exhausted')
    start = time.monotonic()
    expired = False
    with output.open('xb') as stream:
        process = subprocess.Popen(command, cwd=output.parent, env=environment,
                                   stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            rc = process.wait(timeout=seconds)
        except BaseException as error:
            expired = isinstance(error, subprocess.TimeoutExpired)
            try:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=2)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                pass
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
            if not expired:
                raise
            rc = 124
    # Stata's console exit alone is not a fixture result.
    log = Path(environment['FEREG_PPML_SOLVER_STATA_LOG']) if 'FEREG_PPML_SOLVER_STATA_LOG' in environment else output
    text = log.read_text(errors='replace') if log.is_file() else ''
    accepted = rc == 0 and not expired and sentinel in text
    record = {'command': command, 'returncode': rc, 'timed_out': expired,
              'elapsed_seconds': time.monotonic()-start, 'budget_seconds': seconds,
              'sentinel_present': sentinel in text, 'accepted': accepted,
              'console_sha256': digest(output), 'application_log_sha256': digest(log) if log.is_file() else None}
    exclusive_json(output.with_suffix('.process.json'), record)
    if not accepted:
        raise ValueError('process/fixture failed; inspect '+str(output))
    return record


def run(args):
    manifest, scope = manifest_scope(args.task)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    candidate = args.candidate.resolve()
    identity = json.loads(args.source_identity.read_text())
    verify_source(candidate, identity)
    original_hash = digest(args.dataset)
    if original_hash != args.dataset_sha256:
        raise ValueError('dataset differs from registered input')
    plugin_name = 'fereg_macos.plugin' if sys.platform == 'darwin' else 'fereg_linux.plugin'
    if args.plugin.resolve() != candidate/'dist'/plugin_name:
        raise ValueError('plugin identity does not name the candidate loader artifact')
    plugin_hash = digest(args.plugin)
    controller = {str(p.relative_to(ROOT)): digest(p) for p in (
        Path(__file__), MANIFEST, ROOT/'scripts/ppml_auto_science.py',
        ROOT/'scripts/ppml_coefficient_accuracy.py',
        ROOT/'tests/stata/development/feppml_auto_science_driver.do',
        ROOT/'tests/stata/development/feppml_auto_science_capture.do')}
    deadline = time.monotonic()+manifest['seconds']['scope']
    clean = {k:v for k,v in os.environ.items() if not k.startswith('FEREG_PPML_SOLVER_')}
    effects = 3 if scope['topology'].endswith('3fe') else 2
    processes = []
    for label, tolerance in [('default', '1e-8'), ('tighter', '1e-10')]:
        env = {**clean, 'FEREG_PPML_SOLVER_REPOSITORY': str(candidate),
               'FEREG_PPML_SOLVER_DATASET': str(args.dataset.resolve()),
               'FEREG_PPML_SOLVER_RESULT': str(output/label),
               'FEREG_PPML_SOLVER_K': str(scope['controls']),
               'FEREG_PPML_SOLVER_N': str(scope['n']),
               'FEREG_PPML_SOLVER_EFFECTS': str(effects),
               'FEREG_PPML_SOLVER_THREADS': str(scope['threads']),
               'FEREG_PPML_SOLVER_OUTER_TOLERANCE': tolerance,
               'FEREG_PPML_SOLVER_CAPTURE_DRIVER': str(ROOT/'tests/stata/development/feppml_auto_science_capture.do'),
               'FEREG_PPML_SOLVER_STATA_LOG': str(output/(label+'.stata.log'))}
        if label == 'default':
            env['FEREG_PPML_SOLVER_ORIGINAL_CSV'] = str(output/'original.csv')
        processes.append(run_process([args.stata, '-b', 'do', str(ROOT/'tests/stata/development/feppml_auto_science_driver.do')],
            env, output/(label+'.console'), min(manifest['seconds']['candidate_fit'], deadline-time.monotonic()), 'AUTO_SCIENCE_DRIVER_RC=0'))
        verify_source(candidate, identity)
    command = [sys.executable, str(ROOT/'scripts/ppml_auto_science.py'), '--input-csv', str(output/'original.csv'),
               '--capture-prefix', str(output/'default'), '--tighter-prefix', str(output/'tighter'),
               '--controls', str(scope['controls']), '--effects', str(effects), '--threads', str(scope['threads']),
               '--output', str(output/'audit.json')]
    if args.ring_multiplier is not None:
        command += ['--ring-multiplier', str(args.ring_multiplier)]
    processes.append(run_process(command, clean, output/'audit.console',
        min(manifest['seconds']['independent_audit_pair'], deadline-time.monotonic()), 'INDEPENDENT_AUTO_AUDIT_PASS'))
    verify_source(ROOT, controller)
    verify_source(candidate, identity)
    if original_hash != digest(args.dataset) or plugin_hash != digest(args.plugin):
        raise ValueError('input or plugin changed during capture')
    audit = json.loads((output/'audit.json').read_text())
    if not audit['outer_score_stability_pass'] or any(not a['independent_candidate_pass'] for a in audit['independent_audits']):
        raise ValueError('independent audit incomplete')
    report = {'schema_version': 1, 'campaign_admission': False, 'qualification_eligible': False,
              'actual_auto_independent_scope_pass': True, 'task': args.task, 'scope': scope,
              'source_identity': identity, 'controller_identity': controller,
              'input_dta_sha256': original_hash, 'plugin_sha256': plugin_hash,
              'processes': processes, 'audit_sha256': digest(output/'audit.json'),
              'remaining': 'Comparator/exception integration and scheduler acceptance required.'}
    exclusive_json(output/'scope.json', report)
    print('ACTUAL_AUTO_INDEPENDENT_SCOPE_PASS')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--task', type=int, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--source-identity', type=Path, required=True)
    parser.add_argument('--plugin', type=Path, required=True)
    parser.add_argument('--dataset', type=Path, required=True)
    parser.add_argument('--dataset-sha256', required=True)
    parser.add_argument('--stata', required=True)
    parser.add_argument('--ring-multiplier', type=int)
    parser.add_argument('--output', type=Path, required=True)
    run(parser.parse_args())


if __name__ == '__main__':
    main()
