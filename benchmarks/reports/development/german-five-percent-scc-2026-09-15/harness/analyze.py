from pathlib import Path
import argparse,csv,json,math,statistics,subprocess,sys
import numpy as np
import pandas as pd
from scipy.sparse.linalg import lsmr
p=argparse.ArgumentParser();p.add_argument('--root',type=Path,required=True);p.add_argument('--smoke',action='store_true');a=p.parse_args();root=a.root
sys.path.insert(0,str(root/'source/scripts'))
from ppml_auto_science import _indicator_design
TERMS=[f'age{power}_ed{edu}' for edu in (10,13,16,18,99) for power in (2,3)]+[f'{y}.year' for y in range(2002,2010)]
report=root/('analysis-smoke' if a.smoke else 'analysis');report.mkdir()
def scalars(path):return {k:float(v) if v.strip()!='.' else None for k,v in csv.reader((path/'scalars.tsv').open(),delimiter='\t')}
def table(path,name):return pd.read_csv(path/(name+'.tsv'),sep='\t',index_col=0,na_values=['.'],skipinitialspace=True).astype(float)
def controls(df):
 age=df['age'].to_numpy(float)-40;edu=df['edyrs'].fillna(99).to_numpy()
 return np.column_stack([age**power/(100 if power==2 else 1000)*(edu==e) for e in (10,13,16,18,99) for power in (2,3)]+[(df['year'].to_numpy()==y) for y in range(2002,2010)])
def discrepancy(left,right,rtol,atol):
 assert left.shape==right.shape and np.isfinite(left).all() and np.isfinite(right).all()
 d=abs(left-right);ratio=d/(atol+rtol*np.maximum(abs(left),abs(right)))
 return dict(max_abs=float(d.max()),envelope_ratio=float(ratio.max()),passed=bool((ratio<=1).all()))
def run_script(name,args,logname):
 with (report/logname).open('w') as log:r=subprocess.run([sys.executable,str(root/'harness'/name),*map(str,args)],stdout=log,stderr=subprocess.STDOUT)
 assert r.returncode==0,logname

def ols_pair(left,right):
 ls,rs=scalars(left),scalars(right)
 for key in ['N','df_m','df_r','df_a']:assert ls[key]==rs[key],key
 b=discrepancy(table(left,'b')[TERMS].to_numpy(),table(right,'b')[TERMS].to_numpy(),1e-7,1e-10)
 v=discrepancy(table(left,'V').loc[TERMS,TERMS].to_numpy(),table(right,'V').loc[TERMS,TERMS].to_numpy(),1e-6,1e-10)
 result=dict(b=b,V=v,sample_equal=True,normalization_dof_equal=True)
 columns=['obs_id','persid','betnr','year','logrwage2','age','edyrs','sample','residual','fitted_outcome']
 lf=pd.read_stata(left/'outputs.dta',columns=columns,convert_categoricals=False);rf=pd.read_stata(right/'outputs.dta',columns=columns,convert_categoricals=False)
 assert len(lf)==len(rf) and len(lf)<=6000000
 for key in columns[:-2]:assert np.array_equal(lf[key],rf[key],equal_nan=True),key
 active=lf['sample'].to_numpy()==1;assert active.sum()==ls['N']
 result['residual']=discrepancy(lf.loc[active,'residual'].to_numpy(),rf.loc[active,'residual'].to_numpy(),1e-7,1e-9)
 assert b['passed'] and v['passed'] and result['residual']['passed']
 return result

def ols_oracle(path):
 df=pd.read_stata(path/'outputs.dta',convert_categoricals=False);df=df[df['sample']==1];n=len(df)
 modeled=8*(4*n*18+64*n*2+32*n)+128*n;assert modeled<=12*1024**3
 x=controls(df);beta=table(path,'b')[TERMS].to_numpy().ravel();y=df['logrwage2'].to_numpy(float);r=df['residual'].to_numpy(float);fitted=df['fitted_outcome'].to_numpy(float);groups=df[['persid','betnr']].to_numpy(float)
 assert np.allclose(y,r+fitted,rtol=1e-13,atol=1e-13)
 scale=max(1.0,float(np.linalg.norm(y)),float(np.linalg.norm(r)))
 finite=np.column_stack((x,np.ones(n)));norm=np.linalg.norm(finite,axis=0);score=float(np.max(np.divide(abs(finite.T@r),norm*scale,out=np.zeros(len(norm)),where=norm>0)));del finite
 design=_indicator_design(groups);counts=np.asarray(design.sum(axis=0)).ravel();fe_score=float(np.max(abs(design.T@r)/np.sqrt(counts))/scale)
 target=fitted-x@beta;solution=lsmr(design,target,atol=1e-13,btol=1e-13,conlim=1e14,maxiter=5000);fresh=target-design@solution[0];span_scale=max(1.0,float(np.linalg.norm(target)));span_error=float(np.linalg.norm(fresh)/span_scale);span_normal=float(np.max(abs(design.T@fresh)/np.sqrt(counts))/span_scale)
 rss=float(r@r);replay=math.isclose(rss,scalars(path)['rss'],rel_tol=1e-10,abs_tol=1e-8)
 result=dict(finite_score=score,fe_score=fe_score,model_space_error=span_error,model_space_normal=span_normal,model_space_iterations=int(solution[2]),recurrence_stop=int(solution[1]),rss_replay=replay,modeled_audit_bytes=modeled)
 result['passed']=score<=1e-8 and fe_score<=1e-8 and span_error<=1e-10 and span_normal<=1e-12 and int(solution[1])in[0,1,2,4,5] and replay
 return result

results={};issues=[]
for family in ['ols','ppml']:
 try:
  work=root/('launcher-smoke-v2' if a.smoke else 'comparison-'+family);runs=json.loads((work/'runs.json').read_text());rows=[x for x in runs['runs'] if x['family']==family];assert all(x['accepted'] for x in rows)
  repetitions=sorted({x['rep'] for x in rows});pairs=[]
  for rep in repetitions:
   left=work/f'{family}-native-{rep}';right=work/f'{family}-reference-{rep}'
   if family=='ols':pairs.append(ols_pair(left,right))
   else:
    s=scalars(left);outfile=report/f'ppml-pair-{rep}.json'
    run_script('compare-full.py',['--native',left,'--reference',right,'--output',outfile,'--expected-input',int(s['N_full']),'--expected-retained',int(s['N']),'--native-sentinel','GERMAN_FIVE_PERCENT_FIT_COMPLETE','--reference-sentinel','GERMAN_FIVE_PERCENT_FIT_COMPLETE','--chunk-rows',65536],f'ppml-pair-{rep}.log');pairs.append(json.loads(outfile.read_text()))
  results[family]=dict(pairs=pairs,commands={})
  for arm in ['native','reference']:
   seconds=[];phases=[]
   for rep in repetitions:
    path=work/f'{family}-{arm}-{rep}';d=dict(x.split('=',1)for x in(path/'fit-result.txt').read_text().splitlines());seconds.append(float(d['COMMAND_SECONDS']));phases.append(scalars(path))
   results[family]['commands'][arm]=dict(seconds=seconds,median_seconds=statistics.median(seconds),scalars_by_repetition=phases)
  if family=='ols':
   result=ols_oracle(work/'ols-native-1');results[family]['independent_native']=result;assert result['passed'],'OLS independent oracle'
  else:
   audits={}
   for arm in ['native','reference']:
    path=work/f'ppml-{arm}-1';s=scalars(path);outfile=report/f'ppml-{arm}-audit.json'
    args=['--directory',path,'--output',outfile,'--sentinel','GERMAN_FIVE_PERCENT_FIT_COMPLETE','--expected-input',int(s['N_full']),'--expected-retained',int(s['N']),'--chunk-rows',65536,'--memory-bytes',12*1024**3]
    if arm=='native':args.append('--require-native-pass')
    run_script('audit-full-mean.py',args,f'ppml-{arm}-audit.log');audits[arm]=json.loads(outfile.read_text())
   c=audits['native']['objective_score'];constant=c['deviance']+2*c['log_likelihood'];checks={}
   for arm in ['native','reference']:
    fit=audits[arm]['reported_objective'];checks[arm+'_identity']=all(math.isfinite(v) for v in fit.values()) and fit['deviance']>=0 and math.isclose(fit['deviance']+2*fit['ll'],constant,rel_tol=1e-12,abs_tol=1e-8)
   witnesses=[audits['reference']['reported_objective']]
   if audits['reference']['objective_score']['stationarity_pass'] and audits['reference']['model_space']['coefficient_binding_pass']:
    z=audits['reference']['objective_score'];witnesses.append(dict(ll=z['log_likelihood'],deviance=z['deviance']))
   for i,w in enumerate(witnesses):
    checks[f'll_no_worse_{i}']=c['log_likelihood']+1e-10+1e-8*max(abs(c['log_likelihood']),abs(w['ll']))>=w['ll']
    checks[f'deviance_no_worse_{i}']=c['deviance']<=w['deviance']+1e-10+1e-8*max(abs(c['deviance']),abs(w['deviance']))
   results[family]['objective_adjudication']=checks;assert all(checks.values()),checks
 except Exception as e:issues.append(dict(family=family,error=str(e)))
(report/'summary.json').write_text(json.dumps(dict(passed=not issues,qualification_eligible=False,results=results,issues=issues),indent=2,allow_nan=False)+'\n')
assert not issues,issues
print('GERMAN_FIVE_PERCENT_ANALYSIS_COMPLETE',flush=True)
