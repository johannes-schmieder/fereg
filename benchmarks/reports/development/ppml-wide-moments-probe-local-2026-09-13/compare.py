import argparse
import ppml_one_fe_compare as m
from ppml_preparation_compare import PRIMARY as OLD_PRIMARY, CONTROLS

m.BASELINE = '22b5106fcffb40dbc729832c45a269890daa5c0b'
PRIMARY = [
    dict(name='unit_65', design='one_fe', n=122880, k=65, threads=4),
    dict(name='fw_129', design='one_fe', n=61440, k=129, threads=4,
         weight='[fw=w]', fit_options='exposure(exposure)', missing=True),
    dict(name='pw_257', design='one_fe', n=30720, k=257, threads=16,
         weight='[pw=w]', vce='cluster c1 c2', fit_options='offset(off)', missing=True),
]
original_compare = m.compare
def compare(paths, unaffected=False):
    original_compare(paths, unaffected=unaffected)
    for name in ['b.csv','V.csv']:
        assert (paths['candidate']/name).read_bytes() == (paths['baseline']/name).read_bytes(), name
m.compare = compare

if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('part',choices=['screen','repeat','controls','audit'])
    parser.add_argument('--name')
    args=parser.parse_args()
    if args.part=='screen':
        for i,cell in enumerate(PRIMARY):m.pair(cell,104729,rotation=i,prefix='wide-moments-')
    elif args.part=='repeat':
        cell=next(c for c in PRIMARY if c['name']==args.name)
        for i,seed in enumerate([8675309,20260819]):m.pair(cell,seed,rotation=i+1,prefix='wide-moments-recheck-')
    elif args.part=='controls':
        cells=[*OLD_PRIMARY,dict(name='weak_wide',design='weak_d3',n=7680,k=65,threads=4)]
        cells.extend(next(c for c in CONTROLS if c['name']==name) for name in ['low_fw','small_narrow','saved_separation'])
        for i,cell in enumerate(cells):m.pair(cell,31415927,rotation=i,prefix='wide-moments-control-')
    else:
        m.pair(PRIMARY[2],67280421,rotation=1,trace=True,prefix='wide-moments-trace-')
