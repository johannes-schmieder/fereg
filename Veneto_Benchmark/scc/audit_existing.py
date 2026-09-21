"""Audit one fixed, already-collected snapshot; never rerun an estimator."""
import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path
import numpy as np
import pandas as pd
from scipy import sparse
from scipy.sparse.csgraph import connected_components
from accounting import parse
from common import read_json, write_json, sha256
from outcomes import audited_outcome
from run_scc import ARMS, THREADS

BASE=Path(__file__).resolve().parents[1]


def covariance(path):
    if not path.exists(): return None
    values=pd.read_csv(path)
    return values.pivot(index='row',columns='column',values='value').loc[['x1','x2','x3','x4'],['x1','x2','x3','x4']].to_numpy()


def main():
    parser=argparse.ArgumentParser();parser.add_argument('snapshot',type=Path);parser.add_argument('output',type=Path)
    args=parser.parse_args();snap=args.snapshot;out=args.output
    assert not out.exists(),'Audit outputs are immutable; choose a new snapshot label'
    data_auth=read_json(BASE/'results/scc/data-authorization.json')
    fingerprint=read_json(snap/'results/smoke-accepted.json')['runtime_fingerprint']
    tasks=[p for p in sorted((snap/'results/campaign').glob('task-*')) if (p/'COMPLETE.json').exists()]
    task_ids=[int(p.name[-2:]) for p in tasks]
    accounting=parse((snap/'accounting.txt').read_text(),'7529406',range(1,25))
    accts={r['task_id']:r for r in accounting['records']}
    rows=[];comparisons=[];task_summaries=[];hashes={}
    for task in tasks:
        i=int(task.name[-2:]);done=read_json(task/'COMPLETE.json');a=accts[i]
        assert a['failed']==a['exit_status']==0 and a['slots']==28
        assert done['calls']==11 and done['runtime_fingerprint']==fingerprint
        log=list((snap/'logs').glob(f'*7529406.{i}'))
        assert len(log)==1 and 'VENETO_SCC_TASK_COMPLETE' in log[0].read_text()
        raw=[read_json(p) for p in sorted(task.glob('*-r*-*.json'))]
        assert len(raw)==11
        assert {(r['cell']['model'],r['arm']) for r in raw}=={(m,arm) for m,arms in ARMS.items() for arm in arms}
        current=[]
        for r in raw:
            assert r['measurement_fingerprint']==fingerprint
            assert r['input_sha256']==data_auth['files'][f"veneto-{r['cell']['model']}.dta"]['sha256']
            assert r['host'].split('.')[0]==a['hostname'].split('.')[0]
            assert r['cell']['threads']==THREADS[(i-1)%6] and r['repetition']==(i-1)//6
            assert len(r['cpu_affinity'])==r['cell']['threads']
            r=audited_outcome(r);r['audit_task_id']=i
            r['command_budget_seconds']=r.get('seconds') if r.get('seconds') is not None else r.get('command_time_lower_bound')
            r['average_rss_sample_spacing_seconds']=(r['command_budget_seconds']/r['rss_samples']) if r['rss_samples'] else None
            current.append(r);rows.append(r)
        task_summaries.append({'task':i,'threads':THREADS[(i-1)%6],'phase':'cold' if i<=6 else 'warm',
                               'wall_seconds':a['wall_seconds'],'host':a['hostname'],'maxvmem':a['maxvmem'],
                               'statuses':dict(Counter(r['status'] for r in current)),
                               'command_seconds_or_bounds':sum(r['command_budget_seconds'] or 0 for r in current)})
        for r in current:
            model=r['cell']['model'];refarm='reghdfe' if model=='ols' else 'ppmlhdfe'
            ref=next(x for x in current if x['arm']==refarm and x['cell']['model']==model)
            info={'task':i,'model':model,'arm':r['arm'],'threads':r['cell']['threads'],'phase':r['phase'],
                  'status':r['status'],'reference':refarm,'native_rc':r['metadata'].get('rc'),
                  'N':r['metadata'].get('N'),'sample_sha256':r.get('sample_sha256'),
                  'sample_count_matches_input':r['metadata'].get('N')==3159720}
            if r.get('coefficients') and ref.get('coefficients'):
                A=pd.DataFrame(r['coefficients']).set_index('name').sort_index();B=pd.DataFrame(ref['coefficients']).set_index('name').sort_index()
                b=A.value.to_numpy();b0=B.value.to_numpy();se=A.se.to_numpy();se0=B.se.to_numpy()
                folder=snap/f'work/task-{i:02d}'
                stem=f"{model}-t{r['cell']['threads']}-r{r['repetition']}"
                V=covariance(folder/f'{stem}-{r["arm"]}/covariance.csv');V0=covariance(folder/f'{stem}-{refarm}/covariance.csv')
                info.update(coefficient_pass=bool(np.allclose(b,b0,rtol=1e-7,atol=1e-10)),se_pass=bool(np.allclose(se,se0,rtol=1e-6,atol=1e-10)),
                            max_absolute_b_difference=float(abs(b-b0).max()),max_b_difference_in_reference_SE=float((abs(b-b0)/se0).max()),
                            max_relative_SE_difference=float((abs(se/se0-1)).max()),
                            exact_sample_matches_reference=r.get('sample_sha256')==ref.get('sample_sha256') if r.get('sample_sha256') else None)
                if V is not None and V0 is not None:
                    info.update(covariance_pass=bool(np.allclose(V,V0,rtol=1e-6,atol=1e-10)),
                                covariance_relative_infinity_norm=float(np.linalg.norm(V-V0,np.inf)/np.linalg.norm(V0,np.inf)))
                if model=='ols':
                    rss=r['metadata'].get('rss');rss0=ref['metadata']['rss'];df0=ref['metadata']['df_r']
                    df=r['metadata'].get('df_r')
                    if df is None and 'df_k' in r['metadata']:df=r['metadata']['N']-r['metadata']['df_k']
                    if df is not None:
                        normalized_se=se*np.sqrt(df/df0)
                        info.update(df_residual=df,df_reference=df0,
                                    max_relative_SE_difference_after_df_rescale=float(abs(normalized_se/se0-1).max()))
                        if V is not None and rss is not None:
                            bread=V*df/rss;bread0=V0*df0/rss0
                            info['bread_relative_infinity_norm']=float(np.linalg.norm(bread-bread0,np.inf)/np.linalg.norm(bread0,np.inf))
                    if rss is not None:info.update(rss_difference=rss-rss0,rss_relative_difference=(rss-rss0)/rss0)
                else:
                    for key in ['deviance','ll']:
                        val=r['metadata'].get(key,r['metadata'].get('loglik') if key=='ll' else None)
                        refval=ref['metadata'].get(key)
                        if val is not None and refval is not None:info[key+'_difference']=val-refval
                info['coefficients']=r['coefficients']
            comparisons.append(info)
    groups=[]
    for model,arms in ARMS.items():
        for arm in arms:
            for t in THREADS:
                rr=[r for r in rows if r['cell']['model']==model and r['arm']==arm and r['cell']['threads']==t]
                warm=[r for r in rr if r['phase']=='warm'];cold=[r for r in rr if r['phase']=='cold']
                for phase,rrr in [('cold',cold),('warm',warm)]:
                    if not rrr:continue
                    groups.append({'model':model,'arm':arm,'threads':t,'phase':phase,'n':len(rrr),'statuses':dict(Counter(r['status'] for r in rrr)),
                                   'seconds_or_bound':float(np.median([r['command_budget_seconds'] for r in rrr])),
                                   'peak_rss_gib':float(np.median([r['peak_rss_gib'] for r in rrr]))})
    # Independent graph topology, using IDs only; no estimator or large design solve.
    frame=pd.read_stata(BASE/'work/ols.dta',columns=['g1','g2'],convert_categoricals=False)
    g1=frame.g1.to_numpy(dtype=np.int64)-1;g2=frame.g2.to_numpy(dtype=np.int64)-1;w=int(g1.max()+1);f=int(g2.max()+1)
    G=sparse.coo_matrix((np.ones(2*len(frame),dtype=np.int8),(np.r_[g1,g2+w],np.r_[g2+w,g1])),shape=(w+f,w+f)).tocsr()
    count,labels=connected_components(G,directed=False)
    sizes=np.bincount(labels);observations=np.bincount(labels[g1],minlength=count)
    graph={'workers':w,'firms':f,'components':int(count),'fe_indicator_rank':w+f-int(count),
           'residual_df_with_four_identified_controls':len(frame)-(w+f-count)-4,
           'components_with_one_firm':int(np.sum(np.bincount(labels[w:],minlength=count)==1)),
           'largest_component_observations':int(observations.max()),'largest_component_observation_fraction':float(observations.max()/len(frame)),
           'expected_naive_SE_multiplier':float(np.sqrt((len(frame)-(w+f-count)-4)/(len(frame)-(w+f-1)-4)))}
    for p in snap.rglob('*'):
        if p.is_file():hashes[str(p.relative_to(snap))]=sha256(p)
    summaries=[]
    for model,arms in ARMS.items():
        for arm in arms:
            cc=[c for c in comparisons if c['model']==model and c['arm']==arm];valid=[c for c in cc if 'coefficient_pass' in c]
            summaries.append({'model':model,'arm':arm,'calls':len(cc),'statuses':dict(Counter(c['status'] for c in cc)),
                              'slope_pass':sum(c['coefficient_pass'] for c in valid),'SE_pass':sum(c['se_pass'] for c in valid),'returned_comparisons':len(valid),
                              'max_abs_b_diff':max([c['max_absolute_b_difference'] for c in valid],default=None),
                              'max_b_diff_in_SE':max([c['max_b_difference_in_reference_SE'] for c in valid],default=None),
                              'max_SE_relative_diff':max([c['max_relative_SE_difference'] for c in valid],default=None)})
    budget={}
    for r in rows:
        key=r['cell']['model']+'/'+r['arm'];budget[key]=budget.get(key,0)+(r['command_budget_seconds'] or 0)
    write_json(out/'audit.json',{'snapshot':snap.name,'completed_tasks':task_ids,'completed_calls':len(rows),
                                'fingerprint':fingerprint,'candidate_source':read_json(tasks[0]/'runtime.json')['runtime']['candidate'],
                                'graph':graph,'tasks':task_summaries,'command_budget':budget,'summary':summaries,
                                'groups':groups,'comparisons':comparisons,'outcome_reclassification':[{'task':r['audit_task_id'],'model':r['cell']['model'],'arm':r['arm'],'recorded_status':r['recorded_status'],'status':r['status']} for r in rows if r['recorded_status']!=r['status']]})
    write_json(out/'snapshot-hashes.json',hashes)
    write_json(out/'accounting.json',accounting)
    pd.DataFrame(groups).to_csv(out/'time-memory.csv',index=False)
    print(json.dumps({'completed_tasks':task_ids,'graph':graph,'summary':summaries},indent=2))


if __name__=='__main__':main()
