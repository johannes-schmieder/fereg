"""Two bounded diagnostic fits; preserve the frozen benchmark runtime and inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(2**20), b''):
            h.update(block)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('benchmark', type=Path)
    parser.add_argument('diagnostic', type=Path)
    args = parser.parse_args()
    root, target = args.benchmark, args.diagnostic
    inventory = json.loads((root / 'input/runtime-inventory.json').read_text())
    data = root / 'input/veneto-ppml.dta'
    assert digest(data) == '92062a6bccaa7b55c3a213e288cf0d22e68668e21f454119ac168dbd938cb08d'
    for name, expected in inventory['runtime'].items():
        if name.startswith('candidate/'):
            assert digest(Path(inventory['runtime_root']) / name) == expected, name
    assert int(os.environ['NSLOTS']) == 14
    affinity = sorted(os.sched_getaffinity(0))
    assert len(affinity) >= 14
    results = []
    for threads in [4, 14]:
        output = target / f't{threads}'
        output.mkdir(exist_ok=False)
        env = os.environ.copy()
        env.update(OMP_NUM_THREADS=str(threads), RAYON_NUM_THREADS=str(threads),
                   OPENBLAS_NUM_THREADS='1', MKL_NUM_THREADS='1', NUMEXPR_NUM_THREADS='1',
                   FEREG_SOLVER_TRACE='1', FEREG_SOLVER_TRACE_FILE=str(output / 'solver-trace.txt'))
        command = ['taskset', '-c', ','.join(str(i) for i in affinity[:threads]),
                   'stata-mp', '-b', 'do', str(target / 'diagnose_ppml.do'),
                   str(root / 'candidate'), str(root / 'references'), str(data), str(threads), str(output)]
        with (output / 'stdout.txt').open('w') as out, (output / 'stderr.txt').open('w') as err:
            process = subprocess.Popen(command, cwd=output, env=env, stdout=out, stderr=err, start_new_session=True)
            timeout = False
            try:
                process.wait(timeout=600)
            except subprocess.TimeoutExpired:
                timeout = True
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
        meta = {}
        if (output / 'metadata.tsv').exists():
            for line in (output / 'metadata.tsv').read_text().splitlines():
                k, value = line.split('\t', 1)
                meta[k] = float(value)
        log = output / 'diagnose_ppml.log'
        sentinel = log.exists() and 'VENETO_PPML_DIAGNOSTIC_COMPLETE' in log.read_text()
        if not timeout:
            assert process.returncode == 0 and sentinel and meta.get('rc') in [0, 430], (process.returncode, meta)
        record = {'threads': threads, 'cpu_affinity': affinity[:threads], 'job_id': os.environ['JOB_ID'],
                  'host': socket.gethostname(), 'timeout': timeout, 'process_returncode': process.returncode,
                  'sentinel': sentinel, 'metadata': meta, 'input_sha256': digest(data),
                  'plugin_sha256': digest(root / 'candidate/dist/fereg_linux.plugin')}
        (output / 'receipt.json').write_text(json.dumps(record, indent=2) + '\n')
        results.append(record)
        print('VENETO_DIAGNOSTIC_CALL', threads, 'timeout' if timeout else meta['rc'], flush=True)
    (target / 'COMPLETE.json').write_text(json.dumps(results, indent=2) + '\n')
    print('VENETO_PPML_DIAGNOSTIC_JOB_COMPLETE', flush=True)


if __name__ == '__main__':
    main()
