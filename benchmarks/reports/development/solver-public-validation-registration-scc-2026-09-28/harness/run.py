"""Public science for independently gated candidates; no headline timing."""
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import time

from accounting import parse as parse_accounting
from artifact_source import snapshot, verify_source


def read(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')
    temporary.replace(path)


def core_passed(arm, summary, accounting, log):
    if accounting['status'] != 'pass' or summary.get('job_id') != arm['job_id']:
        return False
    if any(log.splitlines().count(marker) != 1 for marker in arm['markers']):
        return False
    if arm['name'] == 'nonretaining-cg':
        if not all(summary.get(k) is True for k in ('baseline_failure_reproduced',
                   'candidate_probes_passed', 'controls_passed', 'ready_for_public_validation')):
            return False
        candidate = summary.get('arms', {}).get('candidate', {}).get('tests', [])
        baseline = summary.get('arms', {}).get('baseline', {}).get('tests', [])
        if len(candidate) != 12 or len(baseline) != 12:
            return False
        expected = {(kind,t,w,c) for kind,ts,cs in [('direct',[1,4],[1,4,64,65,129]),
                    ('cmg',[1,8],[0,1,2,3])] for t in ts for w in range(4) for c in cs}
        for tests in (candidate, baseline):
            if sorted(t['name'] for t in tests) != sorted(arm['test_names']):
                return False
            cases = [c for test in tests for c in test.get('cases', [])]
            actual = {(c['kind'],c['threads'],c['frame'],c['case']) for c in cases}
            if len(cases) != 72 or actual != expected:
                return False
            for test in tests:
                if 'cases' not in test and not (test.get('passed') is True and test.get('returncode') == 0):
                    return False
        cases = [c for test in candidate for c in test.get('cases', [])]
        return (all(c.get('passed') is True for c in cases)
                and any(c.get('passed') is False for t in baseline for c in t.get('cases', [])))
    if arm['name'] != 'schwarz-imbalance':
        return False
    tests = summary.get('tests', [])
    return (summary.get('all_passed') is True and len(tests) == 7
            and all(t.get('passed') is True and t.get('process_rc') == 0 and t['expected'] for t in tests)
            and set(arm['required_tests']).issubset({n for t in tests for n in t['expected']})
            and sum(len(t['expected']) for t in tests) == len({n for t in tests for n in t['expected']})
            and summary.get('distinct_tests', 0) == len({n for t in tests for n in t['expected']}))


def fixture_passed(document, paths):
    results = document.get('results', [])
    return (document.get('completed') is True and document.get('status') == 'success' and document.get('tests_failed') == 0
            and document.get('tests_passed') == len(paths)
            and len(results) == len(paths)
            and sorted(r.get('fixture') for r in results) == sorted(paths)
            and all(r.get('status') == 'success' and r.get('process_rc') == 0
                    and r.get('stata_rc') == 0 for r in results))


def command(argv, cwd, env, output, timeout, deadline):
    seconds = min(timeout, deadline - time.monotonic())
    if seconds <= 0:
        return dict(returncode=None, status='controller_deadline')
    with output.open('w') as stream:
        child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=stream,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        try:
            child.wait(timeout=seconds)
            status = 'finished'
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
            status = 'timeout'
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
    return dict(returncode=child.returncode, status=status, log_sha256=sha(output))


def main(root):
    for name, digest in read(root / 'deployment.json').items():
        assert sha(root / name) == digest, name
    registration = read(root / 'registration.json')
    assert {a['name'] for a in registration['arms']} == {'nonretaining-cg','schwarz-imbalance'}
    assert registration['fixtures'] and len(set(registration['fixtures'])) == len(registration['fixtures'])
    base = read(root / 'base-source.json')
    origin = Path(registration['accepted_source'])
    for name, digest in base['files'].items():
        assert sha(origin / name) == digest, name
    for name, digest in registration['input_hashes'].items():
        assert sha(origin / name) == digest, name
    for name, digest in registration['build_input_hashes'].items():
        owner = root if name in registration['staged_package_inputs'] else origin
        assert sha(owner / name) == digest, name
    baseline_plugin = origin / 'dist/fereg_linux.plugin'
    assert sha(baseline_plugin) == registration['baseline_plugin_sha256']
    output = root / 'output'
    output.mkdir()
    work = root / 'work'
    work.mkdir()
    deadline = time.monotonic() + 35 * 60
    receipt = dict(job_id=os.environ['JOB_ID'], arms={}, performance_eligible=False,
                   scope='Public scientific validation only; no production adoption or timing acceptance')
    for arm in registration['arms']:
        name = arm['name']
        core = Path(arm['core_root'])
        for path, digest in read(core / 'deployment.json').items():
            assert sha(core / path) == digest, path
        assert sha(core / 'candidate.patch') == sha(root / (name + '.patch'))
        accounting = None
        for attempt in range(4):
            result = subprocess.run(['qacct', '-j', arm['job_id']], capture_output=True, text=True, timeout=30)
            accounting = parse_accounting(result.stdout, arm['job_id'], [1], expected_slots=8)
            if accounting['status'] != 'pending':
                break
            if attempt < 3:
                time.sleep(20)
        write(output / (name + '-core-accounting.json'), accounting)
        summary_path = core / 'output/summary.json'
        logs = list((core / 'logs').glob('*.o' + arm['job_id']))
        core_summary = read(summary_path) if summary_path.exists() else {}
        log = logs[0].read_text() if len(logs) == 1 else ''
        record = dict(core_accounting=accounting, core_summary_sha256=sha(summary_path) if summary_path.exists() else None,
                      core_log_sha256=sha(logs[0]) if len(logs) == 1 else None,
                      status='core_gate_refused', tests=[])
        receipt['arms'][name] = record
        write(output / 'progress.json', receipt)
        if not core_passed(arm, core_summary, accounting, log):
            continue
        source = work / name
        source.mkdir()
        paths = set(registration['build_input_hashes'])
        paths.update(registration['build_helpers'])
        paths.update(registration['fixtures'])
        for relative in paths:
            target = source / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            owner = root if relative in registration['staged_package_inputs'] else origin
            shutil.copy2(owner / relative, target)
        probe = 'tests/stata/development/ols_near_within_cmg_oracle.do'
        shutil.copy2(root / Path(probe).name, source / probe)
        paths.add(probe)
        subprocess.run(['git', 'apply', '--check', str(root / (name + '.patch'))], cwd=source, check=True)
        subprocess.run(['git', 'apply', str(root / (name + '.patch'))], cwd=source, check=True)
        files = {p:sha(source / p) for p in sorted(paths)}
        identity = dict(snapshot=snapshot(files), files=files)
        verify_source(source, identity)
        write(output / (name + '-source.json'), identity)
        arm_output = output / name
        arm_output.mkdir()
        scratch = work / (name + '-stata-tmp')
        scratch.mkdir()
        env = {k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
        env.update(FEREG_GIT_COMMIT=identity['snapshot'][:12], FEREG_BUILD_DATE='2026-09-28',
                   CARGO_TARGET_DIR=str(work / ('target-' + name)), TMPDIR=str(scratch))
        if name == 'nonretaining-cg':
            target = output / 'baseline-probe'
            target.mkdir()
            result = command([sys.executable,'scripts/run_stata_tests.py','--stata',shutil.which('stata-mp'),
                '--plugin',str(baseline_plugin),'--plugin-sha256',registration['baseline_plugin_sha256'],
                '--log-dir',str(target/'logs'),'--summary-json',str(target/'summary.json'),probe],
                source,env,target/'runner.log',240,deadline)
            result['plugin_sha256'] = sha(baseline_plugin)
            assert result['plugin_sha256'] == registration['baseline_plugin_sha256']
            result['scope'] = 'Diagnostic baseline on new oracle; failure does not authorize candidate acceptance'
            receipt['baseline_probe'] = result
        record['snapshot'] = identity['snapshot']
        record['build'] = command(['bash','scripts/build_linux.sh'], source, env,
                                  arm_output / 'build.log', 900, deadline)
        record['status'] = 'build_failed'
        write(output / 'progress.json', receipt)
        plugin = source / 'dist/fereg_linux.plugin'
        if record['build']['status'] != 'finished' or record['build']['returncode'] != 0:
            continue
        record['plugin_sha256'] = sha(plugin)
        fixtures = registration['fixtures'] + ([probe] if name == 'nonretaining-cg' else [])
        for index, fixture in enumerate(fixtures):
            target = arm_output / ('fixture-' + str(index))
            target.mkdir()
            result = command([sys.executable, 'scripts/run_stata_tests.py', '--stata', shutil.which('stata-mp'),
                              '--plugin', str(plugin), '--plugin-sha256', record['plugin_sha256'],
                              '--log-dir', str(target / 'logs'), '--summary-json', str(target / 'summary.json'),
                              fixture], source, env, target / 'runner.log', 240, deadline)
            document = read(target / 'summary.json') if (target / 'summary.json').exists() else {}
            result.update(fixture=fixture, passed=(result['status'] == 'finished' and result['returncode'] == 0
                          and fixture_passed(document,[fixture])))
            record['tests'].append(result)
            write(output / 'progress.json', receipt)
        verify_source(source, identity)
        assert sha(plugin) == record['plugin_sha256']
        record['status'] = 'public_science_passed' if all(t['passed'] for t in record['tests']) else 'fixture_failed'
        write(output / 'progress.json', receipt)
    write(output / 'summary.json', receipt)
    print('FEREG_SOLVER_PUBLIC_COMPLETE', flush=True)
    if not any(a['status'] == 'public_science_passed' for a in receipt['arms'].values()):
        raise SystemExit(2)
    print('FEREG_SOLVER_PUBLIC_AT_LEAST_ONE_PASS', flush=True)


if __name__ == '__main__':
    main(Path(sys.argv[1]).resolve())
