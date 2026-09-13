import paired,compare
spec=next(s for s in paired.CONTROLS if s['name']=='weak_multiway')
for i,seed in enumerate([31415927,1299709]):
 paths=compare.pair(spec,seed,i+1,trace=True,prefix='batch-v2-memory-recheck-')
 for name in ['b','V','fereg_fit','fereg_certificate']:
  a,b=(paths[arm]/(name+'.csv') for arm in ['baseline','candidate'])
  assert a.read_bytes()==b.read_bytes(),name
