import compare
from compare import ppml
case=dict(name='fw_129',design='one_fe',n=61440,k=129,threads=4,weight='[fw=w]',fit_options='exposure(exposure)',missing=True)
for rotation,seed in enumerate([8675309,20260819]):
    ppml.pair(case,seed,rotation=rotation,prefix='psd-repeat-')
