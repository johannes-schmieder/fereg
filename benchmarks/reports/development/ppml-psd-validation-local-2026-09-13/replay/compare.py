import ppml_one_fe_compare as ppml
import preparation_compare as ols
from ppml_preparation_compare import PRIMARY,CONTROLS
ppml.BASELINE=ols.BASELINE='fd6b970a24f79a78e05fda585760beed59eb74dc'
original=ppml.compare
def exact(paths,unaffected=False):
    original(paths,unaffected=True)
    for name in ['b.csv','V.csv']:assert (paths['candidate']/name).read_bytes()==(paths['baseline']/name).read_bytes()
ppml.compare=exact
if __name__=='__main__':
    cases=[PRIMARY[0],dict(name='fw_129',design='one_fe',n=61440,k=129,threads=4,
             weight='[fw=w]',fit_options='exposure(exposure)',missing=True),PRIMARY[2]]
    cases.extend(next(c for c in CONTROLS if c['name']==name) for name in ['weak_d3','low_fw','small_narrow','saved_separation'])
    for i,case in enumerate(cases):ppml.pair(case,31415927,rotation=i,prefix='psd-')
    ols.pair(dict(name='aw_129',design='one_fe',n=61440,k=129,threads=4,weight='[aw=w]',vce='robust',missing=True),31415927,rotation=0,prefix='psd-')
    ols.pair(dict(name='narrow_no_fe',design='one_fe',n=7680,k=2,threads=1,effects=[],vce='robust'),31415927,rotation=1,prefix='psd-')
