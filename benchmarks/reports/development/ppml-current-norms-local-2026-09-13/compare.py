import argparse
import ppml_one_fe_compare as m
from ppml_preparation_compare import PRIMARY, CONTROLS

m.BASELINE = 'c6ad4c4c6ee7fa84c0d34583a769e366a3f198a8'
# Fused multi-FE checkpoints also refresh their numerical norm diagnostics.
# Keep the existing complete numerical/discrete checks for every case.
original_compare = m.compare
m.compare = lambda paths, unaffected=False: original_compare(paths, unaffected=False)
original_measure = m.measure
def measure(*args, **kwargs):
    if args[1].get('explicit_solver'):
        kwargs['solver'] = args[1]['explicit_solver']
    return original_measure(*args, **kwargs)
m.measure = measure

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('part', choices=['screen', 'controls', 'audit', 'recheck'])
    parser.add_argument('--name')
    args = parser.parse_args()
    if args.part == 'screen':
        for i, cell in enumerate(PRIMARY):m.pair(cell, 104729, rotation=i, prefix='norms-')
    elif args.part == 'controls':
        cells = [next(c for c in CONTROLS if c['name']==name)
                 for name in ['weak_d3','low_fw','small_narrow','saved_separation']]
        cells.append(dict(name='strong_explicit_map', design='strong_d3', n=7680, k=10,
                          threads=4, explicit_solver='map'))
        for i, cell in enumerate(cells):m.pair(cell,31415927,rotation=i,prefix='norms-control-')
    elif args.part == 'audit':
        m.pair(PRIMARY[2],67280421,rotation=1,trace=True,prefix='norms-trace-')
    else:
        cell = next(c for c in PRIMARY if c['name']==args.name)
        for i, seed in enumerate([8675309,20260819]):
            m.pair(cell,seed,rotation=i+1,prefix='norms-recheck-')
