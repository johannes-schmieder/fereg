import argparse, importlib.util, os
from pathlib import Path

repo = Path(__file__).resolve().parents[4]
path = repo / 'benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/preliminary/compare.py'
spec = importlib.util.spec_from_file_location('existing_comparison', path)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
m.BASELINE = '4e18c223a6b741b04bbcee35d10790729f5c8171'

PRIMARY = [
    dict(name='low_aw_narrow', design='one_fe', n=983040, k=1, threads=1, pool=1, weight='[aw=w]'),
    dict(name='low_unit', design='one_fe', n=245760, k=10, threads=4, pool=10),
    dict(name='low_pw_cluster', design='one_fe', n=245760, k=16, threads=16, pool=5, weight='[pw=w]', vce='cluster c1 c2', missing=True),
]
CONTROLS = [
    dict(name='small_aw', design='one_fe', n=7680, k=1, threads=1, pool=1, weight='[aw=w]', vce='robust'),
    dict(name='low_fw_interaction', design='one_fe', n=30720, k=10, threads=4, pool=1, absorb='g1#c2', weight='[fw=w]', vce='cluster c1', missing=True),
    dict(name='low_saved', design='one_fe', n=30720, k=16, threads=4, pool=5, absorb='saved_g=g1#c2', weight='[aw=w]', fit_options='memory(low) residuals(saved_e)', missing=True),
    dict(name='low_slope', design='one_fe', n=30720, k=16, threads=4, pool=5, absorb='g1#c2##c.x1', weight='[aw=w]', vce='robust'),
    dict(name='low_weak', design='weak_d3', n=30720, k=10, threads=4, pool=5, weight='[aw=w]', vce='robust'),
    dict(name='low_no_fe', design='one_fe', n=30720, k=10, threads=4, pool=5, effects=[], weight='[pw=w]', vce='robust'),
    dict(name='fast_iid', design='one_fe', n=245760, k=50, threads=16, pool=10, fit_options='memory(fast)'),
]
for cell in PRIMARY + CONTROLS:
    cell.setdefault('fit_options', 'memory(low)')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('part', choices=['screen', 'repeat', 'controls', 'profile'])
    args = parser.parse_args()
    if args.part == 'controls':
        runs = [(cell, 31415927, j, 'low-control-', False) for j, cell in enumerate(CONTROLS)]
    elif args.part == 'profile':
        runs = [(PRIMARY[0], 67867967, 1, 'low-profile-', True)]
    else:
        seeds = [104729] if args.part == 'screen' else [8675309, 20260819]
        runs = [(cell, seed, j+i+(args.part == 'repeat'), 'low-paired-', False)
                for i, seed in enumerate(seeds) for j, cell in enumerate(PRIMARY)]
    for cell, seed, rotation, prefix, trace in runs:
        os.environ['FEREG_LOCAL_POOL'] = str(cell['pool'])
        m.pair(cell, seed, rotation, trace=trace, prefix=prefix)
