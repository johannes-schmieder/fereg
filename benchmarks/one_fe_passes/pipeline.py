"""Isolated tiled transport overlap probe; never installs or changes fereg."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

REPO = Path(__file__).resolve().parents[2]


def main(root, previous):
    root, previous = root.resolve(), previous.resolve()
    root.mkdir(parents=True, exist_ok=False)
    for name in ('pipeline_probe.c', 'pipeline_guard_test.c'):
        shutil.copyfile(REPO / 'benchmarks/one_fe_passes' / name, root / name)
    env = dict(os.environ, DEVELOPER_DIR='/Library/Developer/CommandLineTools')
    common = ['/usr/bin/clang', '-std=c11', '-O3', '-Wall', '-Wextra', '-Werror', '-DSYSTEM=APPLEMAC', '-I' + str(REPO / 'plugin')]
    subprocess.run(common + ['-bundle', str(REPO / 'plugin/stplugin.c'), str(root / 'pipeline_probe.c'), '-o', str(root / 'probe.plugin')], env=env, check=True)
    subprocess.run(common + ['-fsanitize=undefined', '-fno-sanitize-recover=all', str(REPO / 'plugin/stplugin.c'), str(root / 'pipeline_guard_test.c'), '-o', str(root / 'guard')], env=env, check=True)
    with (root / 'guard.log').open('w') as log:
        subprocess.run([str(root / 'guard')], env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=20)
    assert 'PIPELINE_GUARD_PASS' in (root / 'guard.log').read_text()
    script = '''version 19.0
clear all
set more off
program __pipeline_probe, plugin using("ROOT/probe.plugin")
foreach k in 10 50 {
    use "OLD/data/n983040-k`k'-gdefault-shuffle0.dta", clear
    plugin call __pipeline_probe y x1-x`k' g1, "ROOT/kernel-k`k'.tsv"
}
display "PIPELINE_PROBE_COMPLETE"
'''.replace('ROOT', str(root)).replace('OLD', str(previous))
    (root / 'probe.do').write_text(script)
    with (root / 'stdout.txt').open('w') as out:
        result = subprocess.run([os.environ['STATA_MP_PATH'], '-b', 'do', str(root / 'probe.do')], cwd=root, stdout=out, stderr=subprocess.STDOUT, timeout=180)
    assert result.returncode == 0 and '\nPIPELINE_PROBE_COMPLETE\n' in (root / 'probe.log').read_text()
    (root / 'identity.json').write_text(json.dumps(dict(status='pass', guard='pass', files={p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in root.iterdir() if p.suffix in ('.c', '.plugin', '.tsv')}), indent=2)+'\n')
    print('PIPELINE_COMPLETE', flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('root', type=Path)
    p.add_argument('previous', type=Path)
    a = p.parse_args()
    main(a.root, a.previous)
