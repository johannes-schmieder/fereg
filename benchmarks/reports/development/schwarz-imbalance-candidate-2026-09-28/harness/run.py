"""Isolated core validation of a smaller conservative Schwarz scratch bound."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--run-root', type=Path, required=True)
    root = parser.parse_args().run_root
    for name, digest in json.loads((root / 'deployment.json').read_text()).items():
        assert sha(root / name) == digest, name
    registration = json.loads((root / 'registration.json').read_text())
    baseline = json.loads((root / 'base-source.json').read_text())
    original = Path(registration['accepted_source'])
    source, output = root / 'source', root / 'output'
    source.mkdir()
    output.mkdir()
    for name, digest in baseline['files'].items():
        assert sha(original / name) == digest, name
        path = source / name
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original / name, path)
    shutil.copy2(root / 'rust-toolchain.toml', source / 'rust-toolchain.toml')
    subprocess.run(['git', 'apply', '--check', str(root / 'candidate.patch')], cwd=source, check=True)
    subprocess.run(['git', 'apply', str(root / 'candidate.patch')], cwd=source, check=True)
    files = {name: sha(source / name) for name in baseline['files']}
    changed = [name for name in files if files[name] != baseline['files'][name]]
    assert changed == ['crates/fereg-core/src/memory/categorical_schwarz.rs'], changed
    assert files[changed[0]] == registration['candidate_file_sha256']
    identity = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    write(output / 'source.json', dict(snapshot=identity, files=files))
    env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'),
               FEREG_GIT_COMMIT=identity, FEREG_BUILD_DATE='2026-09-28')
    log = output / 'build.jsonl'
    with log.open('w') as out, (output / 'build.stderr').open('w') as err:
        build = subprocess.run(['cargo', 'test', '--locked', '--offline', '-p', 'fereg-core',
            '--lib', '--no-run', '--message-format=json'], cwd=source, env=env, stdout=out, stderr=err)
    assert build.returncode == 0, 'candidate test build failed; preserved stderr'
    records = [json.loads(line) for line in log.read_text().splitlines() if line.startswith('{')]
    binaries = [r['executable'] for r in records if r.get('reason') == 'compiler-artifact'
        and r.get('executable') and r.get('target', {}).get('name') == 'fereg_core'
        and r.get('profile', {}).get('test')]
    assert len(binaries) == 1
    binary = binaries[0]
    listing = subprocess.run([binary, '--list'], env=env, check=True, capture_output=True, text=True).stdout
    names = [line.removesuffix(': test') for line in listing.splitlines() if line.endswith(': test')]
    for required in registration['required_new_tests'] + registration['exact_tests']:
        assert required in names, required
    selected, results = set(), []
    for selector in registration['filters'] + registration['exact_tests']:
        exact = selector in registration['exact_tests']
        expected = [selector] if exact else [name for name in names if selector in name]
        assert expected and not selected.intersection(expected), selector
        selected.update(expected)
        path = output / ('test-' + str(len(results) + 1) + '.txt')
        command = [binary, selector, '--nocapture', '--test-threads=1'] + (['--exact'] if exact else [])
        with path.open('w') as stream:
            try:
                run = subprocess.run(command, cwd=source, env=env, stdout=stream,
                    stderr=subprocess.STDOUT, timeout=300)
            except subprocess.TimeoutExpired:
                write(output / 'timeout.json', dict(selector=selector, seconds=300, log_sha256=sha(path)))
                raise
        passed = re.search(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;', path.read_text())
        result = dict(selector=selector, expected=expected, process_rc=run.returncode,
                      passed=run.returncode == 0 and passed is not None and int(passed[1]) == len(expected),
                      log_sha256=sha(path))
        results.append(result)
        write(output / 'progress.json', results)
    assert set(registration['required_new_tests']).issubset(selected)
    summary = dict(job_id=os.environ['JOB_ID'], source_snapshot=identity, binary_sha256=sha(Path(binary)),
        distinct_tests=len(selected), tests=results, all_passed=all(r['passed'] for r in results),
        scope='core candidate only; public validation and performance required before adoption')
    write(output / 'summary.json', summary)
    print('FEREG_SCHWARZ_CANDIDATE_CORE_COMPLETE', flush=True)
    if not summary['all_passed']:
        raise SystemExit(2)
    print('FEREG_SCHWARZ_CANDIDATE_CORE_PASS', flush=True)


if __name__ == '__main__':
    main()
