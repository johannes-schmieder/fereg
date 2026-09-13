import importlib.util, os
from pathlib import Path
p=Path(__file__).resolve().parents[4]/'benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/preliminary/compare.py'
s=importlib.util.spec_from_file_location('existing_comparison',p)
m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
m.BASELINE='7663a87614e37bea73e5dbc037c3ad0c7afccdda'
PRIMARY=[
 dict(name='interaction_iid',design='one_fe',n=983040,k=50,threads=16,absorb='g1#c2'),
 dict(name='interaction_aw_robust',design='one_fe',n=245760,k=50,threads=16,absorb='g1#c2',weight='[aw=w]',vce='robust',missing=True),
 dict(name='interaction_pw_cluster',design='one_fe',n=245760,k=50,threads=4,absorb='g1#c2',weight='[pw=w]',vce='cluster c1 c2'),
]
CONTROLS=[
 dict(name='small_interaction',design='one_fe',n=7680,k=1,threads=1,absorb='g1#c2',weight='[aw=w]',vce='robust'),
 dict(name='saved_interaction',design='one_fe',n=30720,k=16,threads=4,absorb='saved_g=g1#c2',weight='[aw=w]',fit_options='residuals(saved_e)',missing=True),
 dict(name='slope_interaction',design='one_fe',n=30720,k=16,threads=4,absorb='g1#c2##c.x1',weight='[aw=w]',vce='robust'),
 dict(name='low_interaction',design='one_fe',n=30720,k=10,threads=4,absorb='g1#c2',weight='[fw=w]',vce='cluster c1',fit_options='memory(low)',pool=1,missing=True),
 dict(name='wide65_interaction',design='one_fe',n=30720,k=65,threads=4,absorb='g1#c2',weight='[aw=w]',vce='robust'),
 dict(name='weak_robust',design='weak_d3',n=30720,k=10,threads=4,weight='[aw=w]',vce='robust'),
 dict(name='simple_iid',design='one_fe',n=245760,k=50,threads=16),
]
if __name__=='__main__':
 for i,seed in enumerate([104729,8675309,20260819]):
  for j,spec in enumerate(PRIMARY):m.pair(spec,seed,i+j,prefix='onefe-')
 for i,spec in enumerate(CONTROLS):
  os.environ['FEREG_LOCAL_POOL']=str(spec.get('pool',10))
  m.pair(spec,31415927,i,prefix='onefe-control-')
 os.environ['FEREG_LOCAL_POOL']='10'
 m.pair(PRIMARY[1],67867967,1,trace=True,prefix='onefe-trace-')
