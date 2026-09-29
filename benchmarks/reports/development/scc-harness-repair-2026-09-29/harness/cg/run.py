"""Isolated, untimed red/green CG diagnosis; never changes the accepted source."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
from test_inputs import stage_test_inputs


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def probe_records(text, kind):
    records = []
    for line in text.splitlines():
        if 'FEREG_CG_PROBE|' not in line:
            continue
        fields = line[line.index('FEREG_CG_PROBE|'):].split('|', 8)
        if len(fields) != 9 or fields[1] != kind or fields[5] not in ('true', 'false'):
            raise ValueError('malformed probe record')
        records.append(dict(kind=kind, threads=int(fields[2]), frame=int(fields[3]),
                            case=int(fields[4]), passed=fields[5] == 'true',
                            error=fields[6], iterations_or_code=fields[7], detail=fields[8]))
    threads = [1, 4] if kind == 'direct' else [1, 8]
    cases = [1, 4, 64, 65, 129] if kind == 'direct' else [0, 1, 2, 3]
    expected = {(t, w, c) for t in threads for w in range(4) for c in cases}
    observed = [(r['threads'], r['frame'], r['case']) for r in records]
    if len(observed) != len(expected) or set(observed) != expected:
        raise ValueError('missing, repeated or unexpected probe case')
    return records


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--run-root', type=Path, required=True)
    args = parser.parse_args()
    root = args.run_root
    registration = json.loads((root / 'registration.json').read_text())
    for name, digest in json.loads((root / 'deployment.json').read_text()).items():
        assert sha(root / name) == digest, name
    source = Path(registration['accepted_source'])
    accepted = json.loads((root / 'base-source.json').read_text())
    for name, digest in accepted['files'].items():
        assert sha(source / name) == digest, name
    output = root / 'output'
    output.mkdir()
    work = root / 'work'
    work.mkdir()
    receipt = dict(job_id=os.environ['JOB_ID'], slots=os.environ['NSLOTS'],
                   baseline_package=accepted['snapshot'], arms={})
    write(output / 'progress.json', receipt)

    for arm in ['baseline', 'candidate']:
        arm_root = work / arm
        arm_root.mkdir()
        # Copy only source inputs. Never copy a previous target, dist or binary.
        for name in accepted['files']:
            target = arm_root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / name, target)
        shutil.copy2(root / 'rust-toolchain.toml', arm_root / 'rust-toolchain.toml')
        stage_test_inputs(root, arm_root, registration['test_inputs'])
        if arm == 'candidate':
            subprocess.run(['git', 'apply', '--check', str(root / 'candidate.patch')], cwd=arm_root, check=True)
            subprocess.run(['git', 'apply', str(root / 'candidate.patch')], cwd=arm_root, check=True)
        for module in ['borrowed_map', 'cmg']:
            target = arm_root / 'crates/fereg-core/src/fe' / (module + '.rs')
            text = target.read_text()
            assert text.rstrip().endswith('}')
            probe = root / ('borrowed_probe.rs' if module == 'borrowed_map' else 'cmg_probe.rs')
            cut = text.rfind('\n}')
            target.write_text(text[:cut] + '\n' + probe.read_text() + text[cut:])
        bindings = {str(p.relative_to(arm_root)): sha(p) for p in sorted(arm_root.rglob('*')) if p.is_file()}
        identity = hashlib.sha256(json.dumps(bindings, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        write(output / (arm + '-source.json'), dict(snapshot=identity, files=bindings))
        env = dict(os.environ, CARGO_TARGET_DIR=str(work / ('target-' + arm)),
                   FEREG_GIT_COMMIT=identity, FEREG_BUILD_DATE='2026-09-28')
        build_log = output / (arm + '-build.jsonl')
        with build_log.open('w') as stdout, (output / (arm + '-build.stderr')).open('w') as stderr:
            build = subprocess.run(['cargo', 'test', '--locked', '--offline', '-p', 'fereg-core',
                                    '--lib', '--no-run', '--message-format=json'], cwd=arm_root,
                                   env=env, stdout=stdout, stderr=stderr)
        if build.returncode:
            raise RuntimeError(arm + ' test build failed; see preserved stderr')
        artifacts = [json.loads(line) for line in build_log.read_text().splitlines() if line.startswith('{')]
        binaries = [a['executable'] for a in artifacts if a.get('reason') == 'compiler-artifact'
                    and a.get('executable') and a.get('target', {}).get('name') == 'fereg_core'
                    and a.get('profile', {}).get('test')]
        assert len(binaries) == 1, binaries
        binary = Path(binaries[0])
        records = []
        for test in registration['tests']:
            name = test['name']
            log_path = output / (arm + '-' + name.rsplit('::', 1)[1] + '.txt')
            with log_path.open('w') as log_stream:
                try:
                    result = subprocess.run([str(binary), name, '--exact', '--nocapture', '--test-threads=1'],
                                            cwd=arm_root, env=env, stdout=log_stream,
                                            stderr=subprocess.STDOUT, timeout=300)
                except subprocess.TimeoutExpired:
                    write(output / (arm + '-timeout.json'), dict(test=name, timeout_seconds=300,
                          log_sha256=sha(log_path), status='timeout', performance_eligible=False))
                    raise
            log = log_path.read_text()
            assert 'running 1 test' in log, name
            record = dict(name=name, returncode=result.returncode, log_sha256=sha(log_path))
            if test.get('probe'):
                record['cases'] = probe_records(log, test['probe'])
                expected_return = 0 if all(r['passed'] for r in record['cases']) else 101
                assert result.returncode == expected_return, record
            else:
                record['passed'] = result.returncode == 0 and 'test result: ok. 1 passed;' in log
            records.append(record)
            receipt['arms'][arm] = dict(source_snapshot=identity, binary_sha256=sha(binary), tests=records)
            write(output / 'progress.json', receipt)
    baseline = receipt['arms']['baseline']['tests']
    candidate = receipt['arms']['candidate']['tests']
    receipt['baseline_failure_reproduced'] = any(not c['passed'] for t in baseline for c in t.get('cases', []))
    receipt['candidate_probes_passed'] = all(c['passed'] for t in candidate for c in t.get('cases', []))
    receipt['controls_passed'] = all(t['passed'] for arm in [baseline, candidate] for t in arm if 'cases' not in t)
    receipt['ready_for_public_validation'] = all(receipt[k] for k in
        ['baseline_failure_reproduced', 'candidate_probes_passed', 'controls_passed'])
    receipt['scope'] = 'core diagnosis only; no public artifact, performance acceptance or qualification'
    write(output / 'summary.json', receipt)
    print('FEREG_CG_DIAGNOSTIC_COMPLETE', flush=True)
    if not receipt['ready_for_public_validation']:
        raise SystemExit(2)
    print('FEREG_CG_CANDIDATE_CORE_PASS', flush=True)


if __name__ == '__main__':
    main()
