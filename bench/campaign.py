#!/usr/bin/env python3
"""Packed SCC comparisons of one frozen runtime and pinned public estimators."""
from __future__ import annotations
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time

import numpy as np
import pandas as pd
import psutil
from scipy import sparse
from scipy.sparse.csgraph import connected_components
from scipy.special import gammaln

from generate import generate, sha256
from run import read_metadata, write_configuration

HERE = Path(__file__).resolve().parent
POLICY = json.loads((HERE/'campaign.json').read_text())


def write_json(path, value):
    path = Path(path)
    temporary = path.with_suffix(path.suffix+'.tmp')
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False)+'\n')
    temporary.replace(path)


def verify_runtime(root):
    for name,expected in json.loads((root/'environment/runtime.json').read_text()).items():
        if sha256(root/name)!=expected:raise RuntimeError('frozen runtime changed: '+name)


def configurations(smoke=False):
    result = []
    def add(model, design='akm-baseline', rows=1000000, threads=4, vce=None, save_fe=False, section='baseline'):
        vce = vce or ('iid' if model=='ols' else 'robust')
        cell = dict(model=model,design=design,rows=rows,threads=threads,vce=vce,
                    save_fe=save_fe,section=section,regressors=16,weights='unit')
        cell['effects'] = 1 if design=='one-fe' else 3 if design.startswith('three-') else 2
        cell['id'] = f'{model}-{design}-n{rows}-t{threads}-{vce}-fe{int(save_fe)}'
        if not any(c['id']==cell['id'] for c in result): result.append(cell)
    if smoke:
        for model in ['ols','ppml']:
            for vce in (['iid','robust','cluster'] if model=='ols' else ['robust','cluster']):
                add(model,rows=1200,vce=vce,save_fe=True,section='smoke')
            for design in ['two-sparse','three-dense','three-sparse']:
                add(model,design,rows=1200,save_fe=True,section='smoke')
        return result
    for model in ['ols','ppml']:
        for vce in (['iid','robust','cluster'] if model=='ols' else ['robust','cluster']):
            for saved in [False,True]: add(model,vce=vce,save_fe=saved)
        for threads in [1,8,16]: add(model,threads=threads,section='scaling')
        add(model,threads=16,vce='cluster',save_fe=True,section='scaling')
        for rows in [100000,4000000]: add(model,rows=rows,section='size')
        for design in ['one-fe','two-dense','two-sparse','three-dense','three-sparse']:
            for rows in POLICY['sizes']: add(model,design,rows=rows,section='breadth')
        for design in ['two-dense','three-sparse']: add(model,design,threads=16,section='scaling')
    return result


def arms(cell):
    result = ['fereg','fixest']
    if cell['threads']<=4: result.append('reghdfe' if cell['model']=='ols' else 'ppmlhdfe')
    if cell['model']=='ppml' and cell['design'] in ['akm-baseline','two-sparse','three-sparse'] and cell['rows']==1000000 and cell['threads']==4:
        result.append('fixest-strict')
    if cell['section']=='smoke' and cell['model']=='ppml': result.append('fixest-strict')
    return result


def dataset_key(cell):
    return f"{cell['model']}-{cell['design']}-n{cell['rows']}"


def prepare(root):
    cells = configurations()
    # Dataset units keep the complete baseline thread/VCE/output panel together.
    units = {}
    for cell in cells: units.setdefault(dataset_key(cell),[]).append(cell)
    bundles = [[] for _ in range(POLICY['bundles'])]
    costs = [0.0]*len(bundles)
    for key, group in sorted(units.items(),key=lambda kv:sum(c['rows']*len(arms(c)) for c in kv[1]),reverse=True):
        slot = min(range(len(bundles)),key=costs.__getitem__)
        bundles[slot].extend(group)
        costs[slot] += sum(c['rows']*len(arms(c)) for c in group)
    write_json(root/'manifest.json',dict(schema=1,policy=POLICY,cells=cells,bundles=bundles,
               timed_fits=sum(len(arms(c)) for c in cells)*POLICY['repetitions']))


def make_dataset(cell, path):
    spec = dict(cell,vce='cluster',program_data=True)
    n,q = spec['rows'],spec['effects']
    design = spec['design']
    # Use the maintained generator for outcomes and columns, replacing topology
    # through a local wrapper so the local benchmark's contract remains intact.
    import generate as generator
    original = generator.groups
    def topology(_cell,rng):
        row=np.arange(n,dtype=np.int64)
        if design in ['two-dense','three-dense']:
            levels=max(8,int((n/4)**(1/q)))
            return {f'g{j+1}':(row//levels**j)%levels+1 for j in range(q)}
        periods=5
        workers=(n+periods-1)//periods
        firms=max(8,workers//20)
        assignment=np.empty((workers,periods),dtype=np.int64)
        assignment[:,0]=np.arange(workers)%firms
        mobility=.2 if design=='akm-baseline' else .02
        for t in range(1,periods):
            destination=rng.integers(0,firms,workers)
            assignment[:,t]=np.where(rng.random(workers)<mobility,destination,assignment[:,t-1])
        data={'g1':row//periods+1}
        if q>=2: data['g2']=assignment.reshape(-1)[:n]+1
        if q==3: data['g3']=(data['g2']+17*data['g1']+row%periods)%32+1
        return data
    generator.groups=topology
    try: metadata=generate(spec,POLICY['seed'],path)
    finally: generator.groups=original
    frame=pd.read_stata(path,convert_categoricals=False)
    # A natural AKM worker cluster, including nested-FE small-sample corrections.
    frame['cluster']=frame['g1']
    from datetime import datetime
    frame.to_stata(path,write_index=False,version=114,time_stamp=datetime(2000,1,1))
    decoded=pd.read_stata(path,convert_categoricals=False)
    assert all(np.array_equal(frame[c],decoded[c]) for c in frame)
    codes=[pd.factorize(frame[f'g{j}'],sort=True)[0] for j in range(1,q+1)]
    levels=[int(c.max()+1) for c in codes]
    graph=dict(levels=levels,mobility=.2 if design=='akm-baseline' else .02 if 'sparse' in design else None)
    if q>=2:
        a,b=codes[0],codes[1]+levels[0]
        G=sparse.coo_matrix((np.ones(2*n,dtype=np.int8),(np.r_[a,b],np.r_[b,a])),shape=(sum(levels[:2]),)*2).tocsr()
        components, labels=connected_components(G,directed=False)
        graph.update(first_pair_components=int(components),first_pair_rank=int(sum(levels[:2])-components),
                     distinct_first_pair_links=int(G.nnz//2),first_pair_mean_worker_degree=float(G.nnz/2/levels[0]))
        if q==3:
            third=codes[2]+sum(levels[:2])
            G=sparse.coo_matrix((np.ones(4*n,dtype=np.int8),(np.r_[a,b,a,third],np.r_[b,a,third,a])),shape=(sum(levels),)*2).tocsr()
            components,labels=connected_components(G,directed=False)
        np.savez_compressed(path.with_suffix('.components.npz'),component=labels[codes[0]])
        graph['components']=int(components)
    else:
        np.savez_compressed(path.with_suffix('.components.npz'),component=codes[0])
        graph['components']=levels[0]
    metadata.update(sha256=sha256(path),topology=graph,
                    column_sha256={c:hashlib.sha256(frame[c].to_numpy(dtype='<f8').tobytes()).hexdigest() for c in frame})
    write_json(path.with_suffix('.json'),metadata)
    return frame,metadata


def physical_cpus():
    allowed=set(psutil.Process().cpu_affinity())
    seen=set(); cpus=[]
    for block in Path('/proc/cpuinfo').read_text().split('\n\n'):
        values=dict(line.split(':',1) for line in block.splitlines() if ':' in line)
        values={k.strip():v.strip() for k,v in values.items()}
        if 'processor' not in values: continue
        cpu=int(values['processor']); key=(values.get('physical id'),values.get('core id',str(cpu)))
        if cpu in allowed and key not in seen: cpus.append(cpu);seen.add(key)
    if len(cpus)<28: raise RuntimeError('whole-node physical-core allocation is smaller than 28')
    return cpus


def execute(command, directory, cpus, timeout):
    env=dict(os.environ,OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1',NUMEXPR_NUM_THREADS='1',
             OMP_NUM_THREADS=str(len(cpus)))
    start=time.monotonic();peak=0;sweeps=0;monitor_started=None
    with (directory/'process.log').open('w') as log:
        process=subprocess.Popen(['taskset','-c',','.join(map(str,cpus)),*command],cwd=directory,
                                 stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
        status='ok'
        while process.poll() is None:
            if time.monotonic()-start>timeout:
                status='timeout'; os.killpg(process.pid,signal.SIGTERM)
                try:process.wait(timeout=3)
                except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
                break
            ready=(directory/'READY').exists();done=(directory/'DONE').exists()
            if ready and not done:
                if monitor_started is None: monitor_started=time.monotonic()
                if time.monotonic()-monitor_started>POLICY['command_timeout_seconds']:
                    status='command-timeout';os.killpg(process.pid,signal.SIGTERM)
                    try:process.wait(timeout=3)
                    except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
                    break
                try:
                    parent=psutil.Process(process.pid);tree=[parent,*parent.children(recursive=True)]
                    peak=max(peak,sum(p.memory_info().rss for p in tree));sweeps+=1
                except (psutil.NoSuchProcess,psutil.AccessDenied):pass
            time.sleep(.02)
        rc=process.wait()
    if status=='ok' and rc!=0:status='error'
    return dict(status=status,process_rc=rc,wall_seconds=time.monotonic()-start,
                estimator_tree_rss_bytes=peak,memory_sweeps=sweeps)


def capture(directory, cell, frame, metadata):
    names=[f'x{i}' for i in range(1,17)]
    b={r['name']:float(r['value']) for r in csv.DictReader((directory/'coefficients.csv').open())}
    V={(r['row'],r['column']):float(r['value']) for r in csv.DictReader((directory/'covariance.csv').open())}
    if not set(names)<=b.keys(): raise ValueError('omitted/missing finite coefficient')
    beta=np.array([b[n] for n in names]);cov=np.array([[V[a,c] for c in names] for a in names])
    if not np.isfinite(beta).all() or not np.isfinite(cov).all():raise ValueError('nonfinite coefficient/covariance')
    rows=pd.read_csv(directory/'rows.csv',na_values=['.'],float_precision='round_trip')
    if not np.array_equal(rows.obs_id.to_numpy(),frame.obs_id.to_numpy()):raise ValueError('original row order changed')
    selected=rows.retained.to_numpy()==1
    if not np.isin(rows.retained,[0,1]).all():raise ValueError('invalid sample indicator')
    if not selected.all():raise ValueError('unexpected sample deletion in positive-outcome balanced inputs')
    required=['finite_slope_rank','slope_omissions','df_a','covariance_policy','N']
    if cell['model']=='ppml':required.append('converged')
    if cell['vce']=='cluster':required.append('N_clust')
    if any(not metadata.get(key) or metadata[key]=='.' for key in required):raise ValueError('missing scientific metadata')
    if float(metadata['finite_slope_rank'])!=16 or float(metadata['slope_omissions'])!=0:raise ValueError('finite rank/omission mismatch')
    if cell['model']=='ppml' and float(metadata['converged'])!=1:raise ValueError('missing affirmative PPML convergence')
    if float(metadata['N'])!=selected.sum():raise ValueError('reported sample count mismatch')
    if not np.isfinite(float(metadata['df_a'])):raise ValueError('invalid absorbed DoF evidence')
    if cell['vce']=='cluster' and float(metadata['N_clust'])!=frame.cluster.nunique():raise ValueError('cluster count mismatch')
    result=dict(beta=beta.tolist(),covariance=cov.tolist(),N=int(selected.sum()),
                sample_sha256=hashlib.sha256(rows.obs_id.to_numpy(dtype='<i8')[selected].tobytes()).hexdigest(),
                omitted=[name for name in b if name.startswith('o.')],retained_all=bool(selected.all()),
                finite_slope_rank=int(float(metadata['finite_slope_rank'])),singleton_deletions=0,separation_deletions=0,
                covariance_policy=metadata['covariance_policy'],absorbed_dof=float(metadata['df_a']),
                clusters=int(frame.cluster.nunique()) if cell['vce']=='cluster' else None)
    if cell['model']=='ols':
        if not metadata.get('df_r') or metadata['df_r']=='.':raise ValueError('missing residual DoF')
        result['residual_dof']=float(metadata['df_r'])
        result['correction_dof']=float(metadata['correction_df']) if 'correction_df' in metadata else len(frame)-16-result['absorbed_dof']-(cell['vce']=='cluster')
    if cell['save_fe']:
        fitted=rows.fitted.to_numpy(float)
        if not np.isfinite(fitted[selected]).all():raise ValueError('missing saved prediction')
        residual=frame.y.to_numpy()-fitted
        normalizer=np.maximum(1,np.bincount(frame.g1.to_numpy(dtype=int),weights=np.abs(frame.y)+np.abs(fitted)))
        moment=np.bincount(frame.g1.to_numpy(dtype=int),weights=residual)
        result['maximum_worker_score']=float(np.max(np.abs(moment)/normalizer))
        result['objective']=float(residual@residual) if cell['model']=='ols' else float(np.sum(fitted-frame.y*np.log(fitted)))
        np.savez_compressed(directory/'fitted.npz',fitted=fitted)
        # Per-level channels must be constant on each retained categorical level.
        result['saved_channels_finite']=all(np.isfinite(rows[f'saved{j}'][selected]).all() for j in range(1,cell['effects']+1))
        if not result['saved_channels_finite']:raise ValueError('nonfinite saved FE channel')
        moments=[]
        for j in range(1,cell['effects']+1):
            code=frame[f'g{j}'].to_numpy(dtype=int)
            channel=rows[f'saved{j}'].to_numpy(float)
            minimum=np.full(code.max()+1,np.inf);maximum=np.full(code.max()+1,-np.inf)
            np.minimum.at(minimum,code,channel);np.maximum.at(maximum,code,channel)
            active=np.isfinite(minimum)
            if not close_values(minimum[active],maximum[active],1e-12,1e-10):raise ValueError('saved channel varies within level')
            moment=np.bincount(code,weights=residual)
            scale=np.maximum(1,np.bincount(code,weights=np.abs(frame.y)+np.abs(fitted)))
            moments.append(float(np.max(abs(moment)/scale)))
        X=frame[[f'x{i}' for i in range(1,17)]].to_numpy()
        moments.append(float(np.max(abs(X.T@residual)/np.maximum(1,abs(X).T@(abs(frame.y)+abs(fitted))))))
        result['maximum_model_score']=max(moments)
        if result['maximum_model_score']>1e-7:raise ValueError('saved predictor fails independent model score')
        objective=float(residual@residual) if cell['model']=='ols' else float(np.sum(frame.y*np.log(fitted)-fitted-gammaln(frame.y+1)))
        reported=metadata.get('rss' if cell['model']=='ols' else 'll')
        if reported is None or not close_values([objective],[float(reported)],POLICY['objective_rtol'],POLICY['objective_atol']):
            raise ValueError('saved predictor does not replay reported objective')
        if metadata.get('native_arm')=='1' and cell['effects']>=2:
            components=np.load(directory.parents[2]/'data'/(dataset_key(cell)+'.components.npz'))['component']
            weights=fitted if cell['model']=='ppml' else np.ones(len(frame))
            mass=np.bincount(components,weights=weights)
            for j in range(2,cell['effects']+1):
                channel=rows[f'saved{j}'].to_numpy(float)
                means=np.bincount(components,weights=weights*channel)/mass
                scale=np.bincount(components,weights=weights*abs(channel))/mass
                if np.any(abs(means)>1e-9+1e-7*scale):raise ValueError('native saved FE component gauge mismatch')
    # Full row captures remain on SCC; compact collection uses exact hashes.
    write_json(directory/'capture.json',result)
    return result


def fit(root, cell, arm, repetition, frame, cpus):
    directory=root/'output'/cell['id']/f'{arm}-r{repetition}'
    directory.mkdir(parents=True,exist_ok=False)
    settings=dict(source=root/'source',refs=root/'environment/references',r_library=root/'environment/r-library',
        output=directory,dataset=root/'data'/(dataset_key(cell)+'.dta'),warm=root/'data/warm.dta',
        arm=arm,model=cell['model'],k=16,effects=cell['effects'],threads=cell['threads'],
        save_fe=int(cell['save_fe']),vce=cell['vce'],tolerance='',clock_plugin=root/'environment/command-clock.plugin')
    write_configuration(directory,settings)
    if arm.startswith('fixest'):
        command=['Rscript',str(HERE/'campaign_r.R'),str(directory/'config.tsv')]
    else:
        command_file=directory/'command.do'
        command_file.write_text(f'do "{HERE}/campaign_stata.do" "{directory}/config.do"\n')
        command=['stata-mp','-b','do',str(command_file)]
    observation=execute(command,directory,cpus[:cell['threads']],POLICY['process_timeout_seconds'])
    meta=read_metadata(directory/'metadata.tsv')
    meta['native_arm']='1' if arm=='fereg' else '0'
    observation.update(cell=cell['id'],arm=arm,repetition=repetition,directory=str(directory),
                       host=socket.gethostname(),job=os.environ.get('JOB_ID'),task=os.environ.get('SGE_TASK_ID'),metadata=meta)
    logs='\n'.join(p.read_text(errors='replace') for p in directory.glob('*.log'))
    if observation['status']=='ok' and (meta.get('rc')!='0' or meta.get('complete')!='1' or 'CAMPAIGN_COMPLETE rc=0' not in logs):
        observation['status']='incomplete'
    if observation['status']=='ok':
        try:
            observation['capture']=capture(directory,cell,frame,meta)
            seconds=float(meta['seconds'])
            if not np.isfinite(seconds) or seconds<=0:raise ValueError('invalid timer')
            if arm=='fereg' or arm in ['reghdfe','ppmlhdfe']:
                resolution=float(meta['clock_resolution'])
                if not 0<resolution<=5e-7:raise ValueError('clock resolution is unqualified')
        except (OSError,ValueError,KeyError) as error:
            observation.update(status='capture-error',error=str(error))
    write_json(directory/'record.json',observation)
    print(f"{cell['id']} {arm} r{repetition}: {observation['status']}",flush=True)
    return observation


def close_values(a,b,rtol,atol):
    a,b=np.asarray(a),np.asarray(b)
    return bool(a.shape==b.shape and np.isfinite(a).all() and np.isfinite(b).all()
                and np.all(abs(a-b)<=atol+rtol*np.maximum(abs(a),abs(b))))


def compare_records(a,b,cell):
    if a['status']!='ok' or b['status']!='ok':return dict(eligible=False,reasons=['application unavailable'])
    x,y=a['capture'],b['capture'];reasons=[]
    for key in ['N','sample_sha256','omitted','finite_slope_rank','singleton_deletions','separation_deletions','clusters','absorbed_dof']:
        if key not in x or key not in y:reasons.append('missing '+key);continue
        if x[key]!=y[key]:reasons.append(key)
    if any(not c.get('covariance_policy') for c in [x,y]):reasons.append('missing covariance policy')
    allowed={f"stock-{cell['model']}-{cell['vce']}",f"{cell['model']}-{cell['vce']}-"+('K.adj=FALSE' if cell['model']=='ppml' else 'stock')}
    if any(c.get('covariance_policy') not in allowed for c in [x,y]):reasons.append('unsupported covariance policy')
    if cell['model']=='ols':
        for key in ['residual_dof','correction_dof']:
            if key not in x or key not in y:reasons.append('missing '+key)
            elif x[key]!=y[key]:reasons.append(key)
    if not close_values(x['beta'],y['beta'],POLICY['coefficient_rtol'],POLICY['coefficient_atol']):reasons.append('coefficients')
    if not close_values(x['covariance'],y['covariance'],POLICY['vce_rtol'],POLICY['vce_atol']):reasons.append('covariance')
    metric='rss' if cell['model']=='ols' else 'll'
    va=a['metadata'].get(metric);vb=b['metadata'].get(metric)
    if va is None or vb is None or not close_values([float(va)],[float(vb)],POLICY['objective_rtol'],POLICY['objective_atol']):reasons.append('objective')
    if cell['save_fe']:
        p=np.load(Path(a['directory'])/'fitted.npz')['fitted'];q=np.load(Path(b['directory'])/'fitted.npz')['fitted']
        if not close_values(p,q,POLICY['fitted_rtol'],POLICY['fitted_atol']):reasons.append('saved prediction')
        if max(x['maximum_model_score'],y['maximum_model_score'])>1e-7:reasons.append('model score')
    return dict(eligible=not reasons,reasons=reasons)


def run_bundle(root, task, smoke=False):
    verify_runtime(root)
    manifest=json.loads((root/'manifest.json').read_text())
    cells=configurations(True) if smoke else manifest['bundles'][task-1]
    cpus=physical_cpus();started=time.monotonic();records=[];datasets={};comparisons=[]
    (root/'data').mkdir(exist_ok=True);(root/'output').mkdir(exist_ok=True)
    # Separate task directories avoid any concurrent shared-file mutation.
    taskroot=root/('smoke' if smoke else f'task-{task}')
    taskroot.mkdir(exist_ok=False)
    for name in ['source','environment']:(taskroot/name).symlink_to(root/name,target_is_directory=True)
    (taskroot/'data').mkdir();(taskroot/'output').mkdir()
    warmcell=dict(cells[0],model='ppml',design='one-fe',effects=1,rows=512)
    make_dataset(warmcell,taskroot/'data/warm.dta')
    for cell in cells:
        if time.monotonic()-started>POLICY['job_cleanup_seconds']:break
        key=dataset_key(cell)
        path=taskroot/'data'/(key+'.dta')
        if not path.exists(): frame,metadata=make_dataset(cell,path);datasets[key]=metadata
        else:frame=pd.read_stata(path,convert_categoricals=False)
        selected=arms(cell);failed=set()
        for repetition in range(1,(1 if smoke else POLICY['repetitions'])+1):
            current={}
            order=selected[repetition%len(selected):]+selected[:repetition%len(selected)]
            for arm in order:
                if arm in failed:
                    records.append(dict(cell=cell['id'],arm=arm,repetition=repetition,status='skipped-after-failure',metadata={}))
                    continue
                if time.monotonic()-started+POLICY['process_timeout_seconds']>POLICY['job_cleanup_seconds']:break
                record=fit(taskroot,cell,arm,repetition,frame,cpus)
                records.append(record);current[arm]=record
                if record['status']!='ok':failed.add(arm)
                write_json(taskroot/'results.json',dict(cells=cells,records=records,datasets=datasets,comparisons=comparisons,complete=False))
            if 'fereg' in current:
                for arm in selected:
                    if arm=='fereg' or arm not in current:continue
                    result=compare_records(current['fereg'],current[arm],cell)
                    result.update(cell=cell['id'],arm=arm,repetition=repetition)
                    if result['eligible']:result['ratio']=float(current['fereg']['metadata']['seconds'])/float(current[arm]['metadata']['seconds'])
                    comparisons.append(result)
        del frame
        if sha256(path)!=datasets[key]['sha256']:raise RuntimeError('input dataset changed')
    expected=sum(len(arms(c)) for c in cells)*(1 if smoke else POLICY['repetitions'])
    seen={(r['cell'],r['arm'],r['repetition']) for r in records}
    for cell in cells:
        for arm in arms(cell):
            for repetition in range(1,(1 if smoke else POLICY['repetitions'])+1):
                if (cell['id'],arm,repetition) not in seen:
                    records.append(dict(cell=cell['id'],arm=arm,repetition=repetition,status='unattempted-budget',metadata={}))
    result=dict(cells=cells,records=records,datasets=datasets,comparisons=comparisons,
                complete=len(records)==expected and all(r['status'] not in ['unattempted-budget','skipped-after-failure'] for r in records),
                coverage_complete=len(records)==expected,expected_fits=expected,host=socket.gethostname(),physical_cpus=cpus,
                wall_seconds=time.monotonic()-started)
    if smoke:
        from campaign_oracle import validate_smoke
        result['independent_science']=validate_smoke(result)
        accepted=all(x['pass'] for x in result['independent_science'] if x['arm']=='fereg')
        result['native_gate_pass']=accepted and sum(r['arm']=='fereg' for r in records)==len(cells)
    write_json(taskroot/'results.json',result)
    verify_runtime(root)
    if smoke and not result['native_gate_pass']:raise RuntimeError('independent native smoke failed')
    (taskroot/'COMPLETE').write_text('CAMPAIGN_TASK_COMPLETE\n')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=['prepare','run','smoke'])
    parser.add_argument('root',type=Path)
    parser.add_argument('--task',type=int,default=1)
    args=parser.parse_args()
    if args.mode=='prepare':prepare(args.root)
    else:run_bundle(args.root,args.task,args.mode=='smoke')
