"""Sequential local campaign, immutable per-invocation results, restartable."""
import argparse
import json
import random
import shutil
import subprocess
import time
from pathlib import Path

import numpy as np
import pandas as pd
import psutil

from common import sha256, write_json, read_json
from measure import measure

ROOT = Path(__file__).resolve().parents[1]
ARMS = {'ols': ['fereg', 'reghdfe', 'greg', 'areg', 'fixest', 'pyfixest'],
        'ppml': ['feppml', 'ppmlhdfe', 'gglm', 'fixest', 'pyfixest']}


def wait_for_competing_work():
    """Wait for observed local KSS/check workloads; never signal their processes."""
    announced = False
    while True:
        competing = []
        for proc in psutil.process_iter(['pid','name','cmdline']):
            try:
                if not any(s in (proc.info['name'] or '').lower() for s in ['python','stata']):
                    continue
                command = ' '.join(proc.info['cmdline'] or [])
                if any(s in command for s in ['KSS_Veneto_replication/', 'fevc/tools/run_checks.py', '/fevc/tests/stata/']):
                    competing.append(proc.pid)
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                continue
        if not competing:
            if announced: print('Competing local workload finished; measurements resume.',flush=True)
            return
        if not announced:
            print(f'Waiting for {len(competing)} competing local KSS/check processes before the next measurement.',flush=True)
            announced = True
        time.sleep(30)


def initialize():
    info = read_json(ROOT / 'results/sample.json')
    k = info['k']
    candidate = ROOT / 'work/candidate'
    if not candidate.exists():
        shutil.copytree(ROOT.parent / 'stata', candidate / 'stata')
        (candidate / 'dist').mkdir()
        shutil.copy2(ROOT.parent / 'dist/fereg_macos.plugin', candidate / 'dist/fereg_macos.plugin')
    rng = np.random.default_rng(20260911)
    n = 7680
    warm = pd.DataFrame({'obs_id': np.arange(1, n+1, dtype=np.int32),
                         'g1': np.repeat(np.arange(1, 769, dtype=np.int32), 10),
                         'g2': rng.integers(1, 101, n, dtype=np.int32)})
    for j in range(1, k+1):
        warm[f'x{j}'] = rng.normal(size=n)
    eta = .2*warm.x1 + .1*warm.x2 + rng.normal(0,.1,n)
    for model in ARMS:
        warm['y'] = eta if model=='ols' else np.exp(eta)
        warm.to_stata(ROOT / f'work/warm-{model}.dta', write_index=False, version=118)
        with (ROOT / f'work/warm-{model}.bin').open('wb') as stream:
            for column in warm:
                stream.write(np.asarray(warm[column], dtype='<f8').tobytes())
        write_json(ROOT/f'work/warm-{model}.json', {'n':n, 'columns':list(warm)})
        p = ROOT / f'work/{model}.json'
        meta = read_json(p)
        meta.update(n=info['N'], columns=list(meta['column_sha256']))
        write_json(p, meta)
    provenance = {'source_sha': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT.parent,text=True).strip(),
                  'hardware': {'chip':'Apple M2 Ultra','physical_cores':24,'performance_cores':16,
                               'efficiency_cores':8,'memory_gib':192},
                  'affinity': 'unavailable on macOS; requests are library/Stata limits, not pinned cores',
                  'runtime_sha256': {str(p.relative_to(candidate)):sha256(p) for p in candidate.rglob('*') if p.is_file()},
                  'reference_sha256': {str(p.relative_to(ROOT/'work/refs')):sha256(p) for p in (ROOT/'work/refs').rglob('*') if p.is_file()}}
    write_json(ROOT / 'results/runtime.json', provenance)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--pilot', action='store_true')
    p.add_argument('--threads', nargs='+', type=int, default=[1,2,4,8,16,24])
    p.add_argument('--repetitions', type=int, default=3)
    p.add_argument('--timeout', type=int, default=600)
    p.add_argument('--wait-for-pid', type=int, help='Wait for an already-running competing local workload')
    args = p.parse_args()
    if args.wait_for_pid:
        while psutil.pid_exists(args.wait_for_pid):
            print('Waiting for the competing local workload to finish before measurements.',flush=True)
            time.sleep(30)
    if not (ROOT/'results/runtime.json').exists():
        initialize()
    info = read_json(ROOT/'results/sample.json')
    manifest = {'measurement': {'startup_timeout_seconds':600, 'command_timeout_seconds':args.timeout,
                                'rss_interval_seconds':.01},
                'threads':args.threads, 'warm_repetitions':args.repetitions,
                'cold_repetitions':1, 'arms':ARMS, 'timing':'public command including covariance',
                'memory':'absolute sampled process-tree RSS during command; data/runtime included',
                'native_defaults':True}
    identity = sha256(ROOT/'results/runtime.json')
    suite = 'pilot' if args.pilot else 'campaign'
    manifest_path = ROOT/f'results/{suite}-manifest.json'
    if manifest_path.exists():
        assert read_json(manifest_path)==manifest, 'A changed plan needs a new campaign namespace'
    else:
        write_json(manifest_path,manifest)
    controller = {p.name:sha256(p) for p in (ROOT/'scripts').iterdir()
                  if p.name in ['measure.py','run.py','common.py','python_worker.py','stata_worker.do','r_worker.R']}
    controller_path = ROOT/f'results/{suite}-controller.json'
    if controller_path.exists():
        assert read_json(controller_path)==controller, 'Controller changed after campaign start'
    else:
        write_json(controller_path,controller)
    calls = []
    for rep in (range(1) if args.pilot else range(args.repetitions+1)):
        phase = 'cold' if rep==0 else 'warm'
        cores = [4] if args.pilot else args.threads.copy()
        random.Random(107+rep).shuffle(cores)
        for t in cores:
            for model, arms in ARMS.items():
                order = arms.copy()
                random.Random(301+rep*29+t).shuffle(order)
                calls.extend((rep,phase,t,model,arm) for arm in order)
    for i,(rep,phase,t,model,arm) in enumerate(calls):
        output = ROOT / f'work/{suite}/{model}-t{t}-r{rep}-{arm}'
        result = ROOT / f'results/{suite}/{output.name}.json'
        if result.exists():
            continue
        if output.exists():
            raise RuntimeError(f'Unfinished invocation needs inspection: {output}')
        if not args.pilot:
            wait_for_competing_work()
        cell = {'model':model,'threads':t,'k':info['k'],'n':info['N'], 'effects':['g1','g2']}
        print(f'{i+1}/{len(calls)} {output.name}',flush=True)
        receipt = measure(ROOT,cell,arm,phase,ROOT/f'work/{model}.dta',
                          ROOT/f'work/warm-{model}.dta',output,manifest,identity)
        receipt['repetition'] = rep
        cp = output/'coefficients.csv'
        if cp.exists():
            receipt['coefficients'] = pd.read_csv(cp).to_dict('records')
        write_json(result,receipt)
        print(f"  {receipt['status']} time={receipt['seconds']} RSS={receipt['peak_rss_gib']}",flush=True)
    write_json(ROOT/f'results/{suite}-complete.json',{'calls':len(calls),'runtime_fingerprint':identity})


if __name__=='__main__':
    main()
