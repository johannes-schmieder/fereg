import argparse
import ppml_one_fe_compare as m
from ppml_preparation_compare import PRIMARY, CONTROLS

m.BASELINE = '6ad8b201b73dcc1e550827a6b713011288292910'
# Preserve exact complete b/V for this arithmetic-preserving reuse.
original_compare = m.compare
def compare(paths, unaffected=False):
    original_compare(paths, unaffected=unaffected)
    for name in ['b.csv','V.csv']:
        assert (paths['candidate']/name).read_bytes() == (paths['baseline']/name).read_bytes(), name
m.compare = compare
original_measure = m.measure
def measure(*args, **kwargs):
    if args[1].get('explicit_solver'):
        kwargs['solver'] = args[1]['explicit_solver']
    return original_measure(*args, **kwargs)
m.measure = measure

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('part', choices=['screen', 'controls', 'audit', 'recheck', 'wide-control'])
    parser.add_argument('--name')
    args = parser.parse_args()
    if args.part == 'screen':
        for i, cell in enumerate(PRIMARY):m.pair(cell, 104729, rotation=i, prefix='moments-')
    elif args.part == 'controls':
        cells = [next(c for c in CONTROLS if c['name']==name)
                 for name in ['weak_d3','low_fw','small_narrow','saved_separation']]
        cells.append(dict(name='strong_explicit_map', design='strong_d3', n=7680, k=10,
                          threads=4, explicit_solver='map'))
        cells.append(dict(name='wide_65', design='one_fe', n=30720, k=65, threads=4))
        for i, cell in enumerate(cells):m.pair(cell,31415927,rotation=i,prefix='moments-control-')
    elif args.part == 'wide-control':
        cell = dict(name='wide_65', design='one_fe', n=30720, k=65, threads=4)
        m.pair(cell,31415927,rotation=5,prefix='moments-control-fixed-')
    elif args.part == 'audit':
        m.pair(PRIMARY[2],67280421,rotation=1,trace=True,prefix='moments-trace-')
    else:
        cell = next(c for c in PRIMARY if c['name']==args.name)
        for i, seed in enumerate([8675309,20260819]):
            m.pair(cell,seed,rotation=i+1,prefix='moments-recheck-')
