import paired, compare
small=next(s for s in paired.CONTROLS if s['name']=='low_manyway')
large=dict(small,n=122880,k=10,weight='[aw=w]',missing=True)
for label,spec,seeds in [('small',small,[15485863,32452843,49979687]),('large',large,[67450611,86028121,104395301])]:
 for i,seed in enumerate(seeds):
  paths=compare.pair(spec,seed,i+(label=='large'),prefix='batch-v2-many-'+label+'-')
  for name in ['b','V','fereg_fit','fereg_certificate']:
   a,b=(paths[arm]/(name+'.csv') for arm in ['baseline','candidate'])
   assert a.read_bytes()==b.read_bytes(),name
