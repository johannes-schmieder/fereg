"""Replay bounded PPML singleton algorithm comparisons in a fresh directory."""
import argparse
import json
from pathlib import Path
import statistics
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--candidate-source', type=Path, required=True)
parser.add_argument('--core-rlib', type=Path, required=True)
parser.add_argument('--dependency-dir', type=Path, action='append', required=True)
parser.add_argument('--rustc', required=True)
parser.add_argument('--target')
parser.add_argument('--output', type=Path, required=True)
a = parser.parse_args()
a.output.mkdir(parents=True, exist_ok=False)
module = a.candidate_source.resolve() / 'crates/fereg-core/src/ppml/prepared_singleton.rs'
driver = a.output / 'driver.rs'
driver.write_text(Path(__file__).with_name('ppml-scale.rs.in').read_text().replace(
    'CANDIDATE_SINGLETON_PATH', json.dumps(str(module))))
binary = a.output / 'ppml-scale'
command = [a.rustc, '--edition=2021', '-O', str(driver), '--extern',
           'fereg_core=' + str(a.core_rlib.resolve()), '-o', str(binary)]
if a.target:
    command += ['--target', a.target]
for directory in a.dependency_dir:
    command += ['-L', 'dependency=' + str(directory.resolve())]
subprocess.run(command, check=True)
records = []
with (a.output / 'scale.jsonl').open('w') as output:
    for shape in ['chain', 'half', 'balanced']:
        sizes = [262144, 524288, 1048576] if shape == 'balanced' else [8192, 16384, 32768]
        for n in sizes:
            for repeat in range(3):
                methods = ['legacy', 'indexed'] if repeat % 2 == 0 else ['indexed', 'legacy']
                for method in methods:
                    record = json.loads(subprocess.check_output(
                        [str(binary), method, shape, str(n)], text=True))
                    record['rep'] = repeat
                    records.append(record)
                    output.write(json.dumps(record) + '\n')
                    output.flush()
summary = []
for shape, n in sorted({(r['shape'], r['n']) for r in records}):
    group = [r for r in records if (r['shape'], r['n']) == (shape, n)]
    assert len({r['mask_hash'] for r in group}) == 1
    summary.append({'shape': shape, 'n': n, **{
        method: {'seconds': statistics.median(r['seconds'] for r in group if r['method'] == method),
                 'peak_bytes': max(r['peak_bytes'] for r in group if r['method'] == method)}
        for method in ['legacy', 'indexed']}})
(a.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
