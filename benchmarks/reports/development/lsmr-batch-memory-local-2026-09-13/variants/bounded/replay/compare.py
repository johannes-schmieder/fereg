import preparation_compare as ols
original_measure=ols.measure
ols.measure=lambda *a,**kw:original_measure(*a,solver=a[1].get('solver','auto'),**kw)
ols.BASELINE='ac47b1907c7bc7bc8ee78de85621b0cc96546a4c'
primary=dict(name='weak_aw_cluster',design='weak_d3',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1')
controls=[
 dict(name='small_narrow',design='weak_d3',n=7680,k=1,threads=1,weight='[aw=w]',vce='robust'),
 dict(name='wide_weak',design='weak_d3',n=122880,k=65,threads=4,weight='[aw=w]',vce='robust'),
 dict(name='strong_schur',design='two_fe_dense',n=61440,k=50,threads=4,weight='[aw=w]'),
 dict(name='complete_iid',design='one_fe',n=983040,k=50,threads=16),
 dict(name='low_one_fe',design='one_fe',n=30720,k=10,threads=4,weight='[fw=w]',vce='cluster c1',fit_options='memory(low)'),
]
for index,seed in enumerate([104729,8675309,20260819]):
 ols.pair(primary,seed,index,prefix='joint-model-primary-')
for index,spec in enumerate(controls):
 for repeat,seed in enumerate([104729,8675309,20260819] if spec['name']=='wide_weak' else [31415927]):
  ols.pair(spec,seed,index+repeat,trace=True,prefix='joint-model-control-')
ols.pair(dict(name='generic_lsmr',design='weak_d3',n=30720,k=10,threads=4,weight='[aw=w]',vce='robust',solver='lsmr',fit_options='preconditioner(schwarz)'),15485863,1,trace=True,prefix='joint-model-control-')
