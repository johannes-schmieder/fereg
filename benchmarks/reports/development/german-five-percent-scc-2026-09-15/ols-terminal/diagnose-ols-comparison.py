"""Read saved outputs to localize the failed comparison; never re-estimate or accept it."""
from pathlib import Path
import argparse, ast, hashlib, json, os, time
import numpy as np
import pandas as pd
p=argparse.ArgumentParser();p.add_argument('--root',type=Path,required=True);a=p.parse_args();root=a.root
# Reuse the exact four numerical helpers, without executing the failed runner.
source=root/'harness/analyze-selected.py';tree=ast.parse(source.read_text())
import csv
selected=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ('scalars','table','controls','discrepancy')]
assert len(selected)==4
exec(compile(ast.Module(body=selected,type_ignores=[]),str(source),'exec'))
TERMS=[f'age{power}_ed{edu}' for edu in (10,13,16,18,99) for power in (2,3)]+[f'{y}.year' for y in range(2002,2010)]
assert discrepancy(np.array([1.]),np.array([1.]),1e-7,1e-9)['passed']
assert not discrepancy(np.array([1.]),np.array([1.001]),1e-7,1e-9)['passed']
work=root/'comparison-ols-isolated-1';out=root/'diagnosis-ols-comparison.json';assert not out.exists()
runs=json.loads((work/'runs.json').read_text());assert runs['complete'] and len(runs['runs'])==4 and all(r['accepted'] for r in runs['runs'])
result=dict(purpose='localize preserved comparison failure; no fit or acceptance retry',qualification_eligible=False,comparison_accepted=False,job_id=os.environ['JOB_ID'],host=os.uname().nodename,helper_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),pairs=[],scores={},model_space='not evaluated; independent feasibility remains required')
start=time.time()
for rep in (1,2):
 left=work/f'ols-native-{rep}';right=work/f'ols-reference-{rep}';ls,rs=scalars(left),scalars(right)
 item=dict(rep=rep,dof={k:dict(native=ls[k],reference=rs[k],equal=ls[k]==rs[k]) for k in ('N','df_m','df_r','df_a')},b=discrepancy(table(left,'b')[TERMS].to_numpy(),table(right,'b')[TERMS].to_numpy(),1e-7,1e-10),V=discrepancy(table(left,'V').loc[TERMS,TERMS].to_numpy(),table(right,'V').loc[TERMS,TERMS].to_numpy(),1e-6,1e-10))
 cols=['obs_id','persid','betnr','year','logrwage2','age','edyrs','sample','residual','fitted_outcome'];equal={k:True for k in cols[:-2]};rows=active_rows=0;max_abs=max_ratio=0.;outside=0;diff_squares=left_squares=right_squares=0.
 with pd.read_stata(left/'outputs.dta',columns=cols,convert_categoricals=False,chunksize=65536) as lr,pd.read_stata(right/'outputs.dta',columns=cols,convert_categoricals=False,chunksize=65536) as rr:
  for lf,rf in zip(lr,rr,strict=True):
   assert len(lf)==len(rf)
   for k in equal:equal[k]&=bool(np.array_equal(lf[k],rf[k],equal_nan=True))
   active=lf['sample'].to_numpy()==1;rows+=len(lf);active_rows+=int(active.sum());x=lf.loc[active,'residual'].to_numpy();y=rf.loc[active,'residual'].to_numpy()
   assert np.isfinite(x).all() and np.isfinite(y).all();d=abs(x-y);ratio=d/(1e-9+1e-7*np.maximum(abs(x),abs(y)))
   max_abs=max(max_abs,float(d.max(initial=0)));max_ratio=max(max_ratio,float(ratio.max(initial=0)));outside+=int((ratio>1).sum());diff_squares+=float((x-y)@(x-y));left_squares+=float(x@x);right_squares+=float(y@y)
 item.update(input_rows=rows,retained_rows=active_rows,all_raw_and_sample_equal=equal,residual=dict(max_abs=max_abs,envelope_ratio=max_ratio,rows_outside_envelope=outside,passed=outside==0,l2_difference=diff_squares**.5,native_rss=left_squares,reference_rss=right_squares))
 result['pairs'].append(item)
 print('PAIR_LOCALIZED',rep,json.dumps({k:item[k]for k in ('b','V','residual')}),flush=True)
# Independent equations are diagnostic and do not replace the missing FE-span witness.
for arm in ('native','reference'):
 path=work/f'ols-{arm}-1';s=scalars(path);n=int(s['N']);assert n<=6000000
 modeled=8*n*32+64*1024**2;assert modeled<=12*1024**3
 groups=np.empty((n,2));residual=np.empty(n);moments=np.zeros(19);squares=np.zeros(19);ynorm2=rnorm2=0.;kept=0
 with pd.read_stata(path/'outputs.dta',convert_categoricals=False,chunksize=65536) as reader:
  for df in reader:
   df=df[df['sample']==1];m=len(df)
   if m==0:continue
   x=np.column_stack((controls(df),np.ones(m)));r=df['residual'].to_numpy(float);y=df['logrwage2'].to_numpy(float)
   assert np.isfinite(x).all() and np.isfinite(r).all() and np.isfinite(y).all()
   moments+=x.T@r;squares+=np.sum(x*x,axis=0);ynorm2+=float(y@y);rnorm2+=float(r@r)
   groups[kept:kept+m]=df[['persid','betnr']];residual[kept:kept+m]=r;kept+=m
 assert kept==n
 scale=max(1.,ynorm2**.5,rnorm2**.5);finite=float(np.max(np.divide(abs(moments),np.sqrt(squares)*scale,out=np.zeros(19),where=squares>0)));fe=[]
 for j in range(2):
  _,codes=np.unique(groups[:,j],return_inverse=True);counts=np.bincount(codes);mom=np.bincount(codes,weights=residual);fe.append(float(np.max(abs(mom)/np.sqrt(counts))/scale))
 result['scores'][arm]=dict(finite_max=finite,fe_maxima=fe,stationarity_pass=max(finite,*fe)<=1e-8,rss=rnorm2,reported_rss=s['rss'],modeled_bytes=modeled)
 del groups,residual
result['elapsed_seconds']=time.time()-start
with out.open('x') as f:json.dump(result,f,indent=2,allow_nan=False);f.write('\n')
print('OLS_COMPARISON_DIAGNOSIS_COMPLETE',flush=True)
