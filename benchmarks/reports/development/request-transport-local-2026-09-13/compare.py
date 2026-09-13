import ppml_one_fe_compare as ppml
import preparation_compare as ols
from ppml_preparation_compare import PRIMARY, CONTROLS

BASELINE='22b5106fcffb40dbc729832c45a269890daa5c0b'
ppml.BASELINE=ols.BASELINE=BASELINE
original=ppml.compare
def compare(paths,unaffected=False):
    original(paths,unaffected=unaffected)
    for name in ['b.csv','V.csv']:
        assert (paths['candidate']/name).read_bytes()==(paths['baseline']/name).read_bytes(),name
ppml.compare=compare

if __name__=='__main__':
    cells=[PRIMARY[0],dict(name='fw_129',design='one_fe',n=61440,k=129,threads=4,
              weight='[fw=w]',fit_options='exposure(exposure)',missing=True)]
    cells.extend(next(c for c in CONTROLS if c['name']==name)
                 for name in ['weak_d3','low_fw','small_narrow','saved_separation'])
    for i,cell in enumerate(cells):ppml.pair(cell,31415927,rotation=i,prefix='transport-')
    ols.pair(dict(name='aw_129',design='one_fe',n=61440,k=129,threads=4,
                 weight='[aw=w]',vce='robust',missing=True),31415927,rotation=0,prefix='transport-')
    ols.pair(dict(name='narrow_no_fe',design='one_fe',n=7680,k=2,threads=1,effects=[],
                 vce='robust'),31415927,rotation=1,prefix='transport-')
