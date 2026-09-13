import paired,compare
spec=dict(next(s for s in paired.CONTROLS if s['name']=='low_manyway'),n=122880,k=10,weight='[aw=w]',missing=True)
paths=compare.pair(spec,123456789,1,trace=True,prefix='batch-v2-profile-')
for name in ['b','V','fereg_fit','fereg_certificate']:
 a,b=(paths[arm]/(name+'.csv') for arm in ['baseline','candidate'])
 assert a.read_bytes()==b.read_bytes(),name
