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
import shutil
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
from placement import socket_ordered_cpus, placement_metadata, bound_command

HERE = Path(__file__).resolve().parent
POLICY = json.loads((HERE/'campaign.json').read_text())
# A run-scoped subset (for example the baseline cells as one bundle on another
# node type) is set at prepare time and recorded in the manifest's policy.
POLICY.update(json.loads(os.environ.get('CAMPAIGN_POLICY_OVERRIDE','{}')))
PYFIXEST_ARMS = list(POLICY['pyfixest'])
# Strict arms reuse their base arm's environment with tighter tolerances.
PYFIXEST_STRICT_ARMS = [f'{arm}-strict' for arm in PYFIXEST_ARMS] if 'pyfixest_strict_tolerance' in POLICY else []
# FixedEffectModels.jl at its default tolerance and at 1e-10 (linear models only).
JULIA_ARMS = ['julia','julia-strict'] if 'julia' in POLICY else []
STATA_REFERENCES = ['reghdfe','ppmlhdfe','areg']
# Reported absorbed/residual counts follow each package's convention; see agreement().
COUNT_CONVENTIONS = {'absorbed_dof','residual_dof','correction_dof'}


def write_json(path, value):
    path = Path(path)
    temporary = path.with_suffix(path.suffix+'.tmp')
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False)+'\n')
    temporary.replace(path)


def verify_runtime(root):
    for name,expected in json.loads((root/'environment/runtime.json').read_text()).items():
        if sha256(root/name)!=expected:raise RuntimeError('frozen runtime changed: '+name)


def repetitions(cell):
    return POLICY['large_repetitions'] if cell['rows']>POLICY['large_rows'] else POLICY['repetitions']


def configurations(smoke=False, select=True):
    result = []
    def add(model, design='akm-baseline', rows=POLICY['baseline_rows'], threads=POLICY['main_threads'], vce=None,
            save_fe=False, section='baseline', regressors=POLICY['controls']):
        vce = vce or ('iid' if model=='ols' else 'robust')
        cell = dict(model=model,design=design,rows=rows,threads=threads,vce=vce,
                    save_fe=save_fe,section=section,regressors=regressors,weights='unit')
        cell['effects'] = 1 if design=='one-fe' else 3 if design.startswith('three-') or design=='gravity' else 2
        cell['id'] = f'{model}-{design}-n{rows}-t{threads}-{vce}-fe{int(save_fe)}'+controls_suffix(cell)
        cell['arms'] = arms(cell)
        cell['repetitions'] = 1 if smoke else repetitions(cell)
        if not any(c['id']==cell['id'] for c in result): result.append(cell)
    if smoke:
        for model in ['ols','ppml']:
            for vce in (['iid','robust','cluster'] if model=='ols' else ['robust','cluster']):
                add(model,rows=1200,vce=vce,save_fe=True,section='smoke')
            for design in ['two-sparse','three-dense','three-sparse','akm-clustered',*(['gravity'] if model=='ppml' else [])]:
                add(model,design,rows=1200,save_fe=True,section='smoke')
        return result
    for model in ['ols','ppml']:
        for vce in (['iid','robust','cluster'] if model=='ols' else ['robust','cluster']):
            for saved in [False,True]: add(model,vce=vce,save_fe=saved)
        for threads in POLICY['threads']:
            if threads!=POLICY['main_threads']: add(model,threads=threads,section='scaling')
        add(model,threads=16,vce='cluster',save_fe=True,section='scaling')
        for k in POLICY['control_variants']: add(model,regressors=k,section='controls')
        for design in ['one-fe','two-dense','two-sparse','three-dense','three-sparse','akm-clustered',
                       *(['gravity'] if model=='ppml' else [])]: add(model,design,section='design')
        for design in ['two-dense','three-sparse']: add(model,design,threads=16,section='scaling')
    # Size series repeat the four-core default specification; their
    # one-million-row cells are the option and design cells above.
    for model,design in POLICY['size_series']:
        for rows in POLICY['sizes']: add(model,design,rows=rows,section='size')
    # A partial run (another node type, or cells rerun on corrected inputs)
    # selects sections and designs of the full plan.
    if select and 'sections' in POLICY: result=[c for c in result if c['section'] in POLICY['sections']]
    if select and 'designs' in POLICY: result=[c for c in result if c['design'] in POLICY['designs']]
    return result


def series_key(cell):
    """Cells that differ only in their number of rows form one size series."""
    return (cell['model'],cell['design'],cell['threads'],cell['vce'],cell['save_fe'],
            cell.get('regressors',POLICY['controls']))


def controls_suffix(cell):
    """Synthetic cells keep their historical identities at the standard control
    count; real panels carry their own fixed controls and inputs."""
    k = cell.get('regressors', POLICY['controls'])
    return '' if k == POLICY['controls'] or cell.get('section') == 'veneto' else f'-k{k}'


def arms(cell):
    if 'arms' in cell: return list(cell['arms'])
    result = ['fereg','fixest',*PYFIXEST_ARMS]
    if cell['model']=='ols': result.extend(JULIA_ARMS)
    if cell['threads']<=4: result.append('reghdfe' if cell['model']=='ols' else 'ppmlhdfe')
    # Stata 19 areg absorbs several categorical effects for linear models; it
    # cannot save individual effects, and Stata is licensed for four cores.
    if cell['model']=='ols' and cell['threads']<=4 and not cell['save_fe']:
        result.append('areg')
    if (cell['model']=='ppml' and cell['design'] in STRICT_PPML_DESIGNS and cell['rows']==1000000
            and cell['threads']==4) or cell['model']=='ols' or cell['section']=='smoke':
        result.extend(['fixest-strict',*PYFIXEST_STRICT_ARMS])
    return result


STRICT_PPML_DESIGNS = ['akm-baseline','two-sparse','three-sparse','akm-clustered','gravity']
# Designs whose PPML sample and rank depend on separation by construction.
SEPARATION_DESIGNS = {'gravity'}


def dataset_key(cell):
    return f"{cell['model']}-{cell['design']}-n{cell['rows']}"+controls_suffix(cell)


# Approximate seconds per fit at one million rows and four cores, used only to
# balance bundles (beta.7 SCC medians, rounded). Each fresh process also loads
# its input, and a timeout stops a command's later repetitions and sizes.
PACKING_SECONDS = {'fereg':2,'fixest':3,'fixest-strict':4,'pyfixest-within':3,'pyfixest-within-strict':4,
                   'julia':3,'julia-strict':4,
                   'reghdfe':25,'areg':25,'ppmlhdfe':90}


DESIGN_COST = {'akm-clustered':2,'gravity':4}


def estimated_seconds(cell):
    scale = cell['rows']/1e6*(4 if cell['model']=='ppml' else 1)*DESIGN_COST.get(
        cell['design'],4 if 'sparse' in cell['design'] else 1)
    total = 0.
    for arm in arms(cell):
        command = PACKING_SECONDS[arm]*scale*(cell.get('regressors',16)/16)**.5
        if command>=POLICY['command_timeout_seconds']: total += POLICY['command_timeout_seconds']
        else: total += cell['repetitions']*(command+10+5*cell['rows']/1e6)
    return total


def prepare(root):
    cells = configurations()
    # One design and model per node: its size, core, option and control series
    # then compare absolute times on one host.
    units = {}
    for cell in cells: units.setdefault(f"{cell['model']}-{cell['design']}",[]).append(cell)
    bundles = [[] for _ in range(POLICY['bundles'])]
    costs = [0.0]*len(bundles)
    for key, group in sorted(units.items(),key=lambda kv:(-sum(map(estimated_seconds,kv[1])),kv[0])):
        slot = min(range(len(bundles)),key=costs.__getitem__)
        # Ascending sizes let a timeout stop the remainder of its series.
        bundles[slot].extend(sorted(group,key=lambda c:c['rows']))
        costs[slot] += sum(map(estimated_seconds,group))
    # Every node first times the same native reference cell, recording its speed.
    reference = next(c for c in configurations(select=False) if c['section']=='baseline' and c['model']=='ols'
                     and c['vce']=='iid' and not c['save_fe'])
    references = []
    for task, bundle in enumerate(bundles, 1):
        cell = dict(reference,id=f"{reference['id']}-ref{task}",section='reference',arms=['fereg'],
                    repetitions=POLICY['reference_repetitions'])
        bundle.insert(0,cell); references.append(cell)
    cells = references+cells
    write_json(root/'manifest.json',dict(schema=2,policy=POLICY,cells=cells,bundles=bundles,
               estimated_bundle_hours=[round(sum(map(estimated_seconds,b))/3600,2) for b in bundles],
               timed_fits=sum(len(c['arms'])*c['repetitions'] for c in cells)))


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
        if design=='akm-clustered': return clustered_topology(n,rng)
        if design=='gravity': return gravity_topology(n)
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
    if design=='gravity': metadata['separation']=gravity_outcome(frame,spec)
    # A natural AKM worker cluster, including nested-FE small-sample corrections.
    frame['cluster']=frame['g1']
    from datetime import datetime
    frame.to_stata(path,write_index=False,version=114,time_stamp=datetime(2000,1,1))
    decoded=pd.read_stata(path,convert_categoricals=False)
    assert all(np.array_equal(frame[c],decoded[c]) for c in frame)
    codes=[pd.factorize(frame[f'g{j}'],sort=True)[0] for j in range(1,q+1)]
    levels=[int(c.max()+1) for c in codes]
    graph=dict(levels=levels,mobility=.2 if design=='akm-baseline' else .02 if 'sparse' in design
               else CLUSTERED['mobility'] if design=='akm-clustered' else None)
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


# Heavy-tailed firm sizes and mobility concentrated within regions: weakly
# linked clusters of firms, unlike the well-mixed baseline.
CLUSTERED = dict(periods=5,mobility=.1,within_region=.95,pareto_shape=1.5,max_regions=50)


def clustered_topology(n, rng):
    c = CLUSTERED
    periods = c['periods']
    workers = (n+periods-1)//periods
    firms = max(8,workers//20)
    regions = max(2,min(c['max_regions'],firms//20))
    size = rng.pareto(c['pareto_shape'],firms)+1
    region = rng.integers(0,regions,firms)
    order = np.lexsort((np.arange(firms),region))
    # key[i] = region + cumulative within-region size share, so that a region r
    # and u in (0,1] select firm order[searchsorted(key, r+u)] with probability
    # proportional to its size.
    sorted_region, sorted_size = region[order], size[order]
    totals = np.bincount(sorted_region,weights=sorted_size,minlength=regions)
    within = np.cumsum(sorted_size)-np.r_[0,np.cumsum(totals)][sorted_region]
    key = sorted_region+within/totals[sorted_region]
    occupied = np.unique(region)
    def draw(target):
        u = 1-rng.random(len(target))
        return order[np.minimum(np.searchsorted(key,target+u),firms-1)]
    assignment = np.empty((workers,periods),dtype=np.int64)
    assignment[:,0] = draw(rng.choice(occupied,workers,p=totals[occupied]/totals.sum()))
    for t in range(1,periods):
        stay_region = rng.random(workers)<c['within_region']
        target = np.where(stay_region,region[assignment[:,t-1]],rng.choice(occupied,workers))
        moves = rng.random(workers)<c['mobility']
        assignment[:,t] = np.where(moves,draw(target),assignment[:,t-1])
    # Pareto sizes can leave a firm with a single worker-year. That observation
    # stays at the worker's firm in the neighbouring period, so that, as in the
    # other designs, no command deletes singletons from the frozen input.
    counts = np.bincount(assignment.reshape(-1)[:n],minlength=firms)
    for w,t in zip(*np.nonzero(counts[assignment]==1)):
        if w*periods+t<n: assignment[w,t] = assignment[w,t-1] if t else assignment[w,1]
    if (np.bincount(assignment.reshape(-1)[:n])==1).any(): raise RuntimeError('singleton firm in clustered topology')
    row = np.arange(n,dtype=np.int64)
    return {'g1':row//periods+1,'g2':assignment.reshape(-1)[:n]+1}


def gravity_topology(n):
    """Directed country pairs over years: exporter-year, importer-year and pair effects."""
    years = 25 if n>=100000 else 5
    countries = 2
    while countries*(countries-1)*years<n: countries += 1
    exporter, importer = np.nonzero(~np.eye(countries,dtype=bool))
    pair = np.repeat(np.arange(len(exporter)),years)[:n]
    year = np.tile(np.arange(years),len(exporter))[:n]
    return {'g1':exporter[pair]*years+year+1,'g2':importer[pair]*years+year+1,'g3':pair+1}


GRAVITY = dict(zero_share=.45,separated_exporter_years=.03,separated_by_covariate=.01)


def gravity_outcome(frame, spec):
    """Poisson flows with many zeros, all-zero exporter-years (separated by the
    fixed effects) and a binary last control that is one only where trade is zero
    (separated by a covariate). The other controls keep the generator's slopes."""
    g = GRAVITY
    k = spec['regressors']
    rng = np.random.default_rng(np.random.SeedSequence([POLICY['seed'],907]))
    xb = sum((.30/j if j%2 else -.20/j)*frame[f'x{j}'].to_numpy() for j in range(1,k))
    eta = sum(.15/j*np.sin(frame[f'g{j}'].to_numpy()/(5+j)) for j in range(1,4))
    low, high = -10., 10.
    for _ in range(60):
        middle = (low+high)/2
        if np.mean(np.exp(-np.exp(middle+xb+eta)))>g['zero_share']: low = middle
        else: high = middle
    y = rng.poisson(np.exp(high+xb+eta)).astype(float)
    groups = np.unique(frame.g1.to_numpy())
    empty = rng.choice(groups,max(1,int(g['separated_exporter_years']*len(groups))),replace=False)
    by_fe = np.isin(frame.g1.to_numpy(),empty)
    y[by_fe] = 0.
    candidates = np.flatnonzero(~by_fe)
    flagged = rng.choice(candidates,max(1,int(g['separated_by_covariate']*len(frame))),replace=False)
    indicator = np.zeros(len(frame))
    indicator[flagged] = 1.
    y[flagged] = 0.
    frame['y'] = y
    frame[f'x{k}'] = indicator
    return dict(g,intercept=high,zero_outcomes=int((y==0).sum()),fe_separated_rows=int(by_fe.sum()),
                covariate_separated_rows=int(len(flagged)))


def physical_cpus():
    allowed=set(psutil.Process().cpu_affinity())
    cpus=socket_ordered_cpus(Path('/proc/cpuinfo').read_text(),allowed)
    if len(cpus)<28: raise RuntimeError('whole-node physical-core allocation is smaller than 28')
    return cpus


def frozen_dataset(cell, destination, inventory):
    """Copy retained inputs and verify both file bytes and decoded columns."""
    key = dataset_key(cell)
    expected = inventory['datasets'][key]
    matches = list(Path(inventory['remote_root']).glob('task-*/data/' + key + '.dta'))
    if len(matches) != 1:
        raise RuntimeError('missing or ambiguous frozen dataset: ' + key)
    source = matches[0]
    metadata = json.loads(source.with_suffix('.json').read_text())
    if metadata != expected or sha256(source) != expected['sha256']:
        raise RuntimeError('frozen input identity mismatch: ' + key)
    for suffix in ['.dta', '.json', '.components.npz']:
        original, copied = source.with_suffix(suffix), destination.with_suffix(suffix)
        shutil.copyfile(original, copied)
        if sha256(original) != sha256(copied):
            raise RuntimeError('frozen copy mismatch: ' + key + suffix)
    frame = pd.read_stata(destination, convert_categoricals=False)
    hashes = {c: hashlib.sha256(frame[c].to_numpy(dtype='<f8').tobytes()).hexdigest() for c in frame}
    if hashes != expected['column_sha256'] or len(frame) != expected['rows']:
        raise RuntimeError('decoded frozen input mismatch: ' + key)
    return frame, metadata


def reset_peak(pid):
    # Linux 4.0+: writing 5 resets the process's peak resident set to its current size.
    Path(f'/proc/{pid}/clear_refs').write_text('5')


def process_memory(pid, field):
    for line in Path(f'/proc/{pid}/status').read_text().splitlines():
        if line.startswith(field+':'):
            return int(line.split()[1])*1024
    raise ValueError('missing '+field)


def execute(command, directory, cpus, timeout):
    """Run one arm; the timed window lies between its READY and DONE files.

    After READY the arm waits for GO while the kernel's peak resident set
    (VmHWM) of its process tree is reset, so the peak read at DONE covers the
    timed command exactly, with the loaded data. The arm then waits for ACK,
    keeping output writing out of that peak. The 20 ms RSS samples remain a
    cross-check.
    """
    placement=placement_metadata(cpus)
    env=dict(os.environ,OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1',NUMEXPR_NUM_THREADS='1',
             OMP_NUM_THREADS=str(len(cpus)))
    start=time.monotonic();peak=0;sweeps=0;monitor_started=None
    memory=dict(memory_exact=False)
    def tree(pid):
        parent=psutil.Process(pid);return [parent,*parent.children(recursive=True)]
    with (directory/'process.log').open('w') as log:
        process=subprocess.Popen(bound_command(command,placement),cwd=directory,
                                 stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
        status='ok'
        while process.poll() is None:
            if time.monotonic()-start>timeout:
                status='timeout'; os.killpg(process.pid,signal.SIGTERM)
                try:process.wait(timeout=3)
                except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
                break
            ready=(directory/'READY').exists();done=(directory/'DONE').exists()
            if ready and not (directory/'GO').exists():
                try:
                    processes=tree(process.pid)
                    for p in processes:reset_peak(p.pid)
                    memory.update(ready_rss_bytes=sum(process_memory(p.pid,'VmRSS') for p in processes),
                                  reset_pids=sorted(p.pid for p in processes),reset=True)
                except (OSError,ValueError,psutil.Error) as error:
                    memory.update(reset=False,memory_error=str(error))
                (directory/'GO').write_text('go\n')
            if done and not (directory/'ACK').exists():
                try:
                    processes=tree(process.pid)
                    memory.update(peak_rss_bytes=sum(process_memory(p.pid,'VmHWM') for p in processes),
                                  done_rss_bytes=sum(process_memory(p.pid,'VmRSS') for p in processes),
                                  peak_pids=sorted(p.pid for p in processes))
                    # A process that exited during the command would take its peak with it.
                    memory['memory_exact']=bool(memory.get('reset')) and set(memory['reset_pids'])<=set(memory['peak_pids'])
                except (OSError,ValueError,psutil.Error) as error:
                    memory.update(memory_exact=False,memory_error=str(error))
                (directory/'ACK').write_text('ack\n')
            if ready and not done:
                if monitor_started is None: monitor_started=time.monotonic()
                if time.monotonic()-monitor_started>POLICY['command_timeout_seconds']:
                    status='command-timeout';os.killpg(process.pid,signal.SIGTERM)
                    try:process.wait(timeout=3)
                    except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
                    break
                try:
                    processes=tree(process.pid)
                    peak=max(peak,sum(p.memory_info().rss for p in processes));sweeps+=1
                except (psutil.NoSuchProcess,psutil.AccessDenied):pass
            time.sleep(.02)
        rc=process.wait()
    if status=='ok' and rc!=0:status='error'
    return dict(status=status,process_rc=rc,wall_seconds=time.monotonic()-start,
                estimator_tree_rss_bytes=peak,memory_sweeps=sweeps,placement=placement,**memory)


def absorbed_rank(metadata, cell, frame):
    """Put categorical PPML nuisance counts in the common full FE span.

    Native PPML transfers one intercept direction to structural coordinates
    (ppml/result.rs); clustered Stata counts exclude the nested worker FE.
    fixest's PPML count retains the full nuisance span with K.adj disabled.
    Raw published counts remain in every observation's metadata and capture.
    """
    rank=float(metadata['df_a'])
    if metadata.get('arm')=='areg':
        # areg reports the intercept outside e(df_a); reghdfe counts it there.
        rank+=1
    if cell['model']!='ppml':return rank
    native=metadata.get('native_arm')=='1'
    stata=native or metadata['covariance_policy'].startswith('stock-ppml-')
    if native:rank+=1
    if stata and cell['vce']=='cluster':
        if cell['effects']!=2 or not np.array_equal(frame.cluster,frame.g1):
            raise ValueError('unverified PPML nested-FE count convention')
        rank+=frame.g1.nunique()
    return rank


def capture(directory, cell, frame, metadata):
    k=cell.get('regressors',16)
    names=[f'x{i}' for i in range(1,k+1)]
    b={r['name']:float(r['value']) for r in csv.DictReader((directory/'coefficients.csv').open())}
    V={(r['row'],r['column']):float(r['value']) for r in csv.DictReader((directory/'covariance.csv').open())}
    # Only separation designs may delete rows or omit their separating control;
    # Stata posts an omitted slope as o.x, the other packages drop it.
    separation=cell['design'] in SEPARATION_DESIGNS
    finite=[n for n in names if n in b];omitted=[n for n in names if n not in b]
    if omitted and not separation:raise ValueError('omitted/missing finite coefficient')
    beta=np.array([b[n] for n in finite]);cov=np.array([[V[a,c] for c in finite] for a in finite])
    if not np.isfinite(beta).all() or not np.isfinite(cov).all():raise ValueError('nonfinite coefficient/covariance')
    rows=pd.read_csv(directory/'rows.csv',na_values=['.'],float_precision='round_trip')
    if not np.array_equal(rows.obs_id.to_numpy(),frame.obs_id.to_numpy()):raise ValueError('original row order changed')
    selected=rows.retained.to_numpy()==1
    if not np.isin(rows.retained,[0,1]).all():raise ValueError('invalid sample indicator')
    if not separation and not selected.all():raise ValueError('unexpected sample deletion in frozen retained inputs')
    required=['finite_slope_rank','slope_omissions','df_a','covariance_policy','N']
    if cell['model']=='ppml':required.append('converged')
    if cell['vce']=='cluster':required.append('N_clust')
    if any(not metadata.get(key) or metadata[key]=='.' for key in required):raise ValueError('missing scientific metadata')
    if float(metadata['finite_slope_rank'])!=len(finite) or float(metadata['slope_omissions'])!=len(omitted):raise ValueError('finite rank/omission mismatch')
    if cell['model']=='ppml' and float(metadata['converged'])!=1:raise ValueError('missing affirmative PPML convergence')
    if float(metadata['N'])!=selected.sum():raise ValueError('reported sample count mismatch')
    if not np.isfinite(float(metadata['df_a'])):raise ValueError('invalid absorbed DoF evidence')
    clusters=int(frame.cluster[selected].nunique()) if cell['vce']=='cluster' else None
    if cell['vce']=='cluster' and float(metadata['N_clust'])!=clusters:raise ValueError('cluster count mismatch')
    # The gravity design has no singletons, so every deleted row is separated.
    result=dict(beta=beta.tolist(),covariance=cov.tolist(),names=finite,N=int(selected.sum()),
                sample_sha256=hashlib.sha256(rows.obs_id.to_numpy(dtype='<i8')[selected].tobytes()).hexdigest(),
                omitted=['o.'+name for name in omitted],
                publication_omitted=[name for name in b if name.startswith('o.')],retained_all=bool(selected.all()),
                finite_slope_rank=len(finite),singleton_deletions=0,separation_deletions=int((~selected).sum()),
                covariance_policy=metadata['covariance_policy'],absorbed_dof=absorbed_rank(metadata,cell,frame),
                reported_absorbed_dof=float(metadata['df_a']),
                clusters=clusters)
    if cell['model']=='ols':
        if not metadata.get('df_r') or metadata['df_r']=='.':raise ValueError('missing residual DoF')
        result['residual_dof']=float(metadata['df_r'])
        result['correction_dof']=float(metadata['correction_df']) if 'correction_df' in metadata else len(frame)-k-result['absorbed_dof']-(cell['vce']=='cluster')
    if cell['save_fe']:
        fitted=rows.fitted.to_numpy(float)
        if not np.isfinite(fitted[selected]).all():raise ValueError('missing saved prediction')
        fitted[~selected]=np.nan
        np.savez_compressed(directory/'fitted.npz',fitted=fitted)
        # Scores and objectives use the estimation sample only.
        data=frame[selected];fitted=fitted[selected];y=data.y.to_numpy()
        residual=y-fitted
        normalizer=np.maximum(1,np.bincount(data.g1.to_numpy(dtype=int),weights=np.abs(y)+np.abs(fitted)))
        moment=np.bincount(data.g1.to_numpy(dtype=int),weights=residual)
        result['maximum_worker_score']=float(np.max(np.abs(moment)/normalizer))
        result['objective']=float(residual@residual) if cell['model']=='ols' else float(np.sum(fitted-y*np.log(fitted)))
        # Per-level channels must be constant on each retained categorical level.
        result['saved_channels_finite']=all(np.isfinite(rows[f'saved{j}'][selected]).all() for j in range(1,cell['effects']+1))
        if not result['saved_channels_finite']:raise ValueError('nonfinite saved FE channel')
        moments=[]
        for j in range(1,cell['effects']+1):
            code=data[f'g{j}'].to_numpy(dtype=int)
            channel=rows[f'saved{j}'].to_numpy(float)[selected]
            minimum=np.full(code.max()+1,np.inf);maximum=np.full(code.max()+1,-np.inf)
            np.minimum.at(minimum,code,channel);np.maximum.at(maximum,code,channel)
            active=np.isfinite(minimum)
            if not close_values(minimum[active],maximum[active],1e-12,1e-10):raise ValueError('saved channel varies within level')
            moment=np.bincount(code,weights=residual)
            scale=np.maximum(1,np.bincount(code,weights=np.abs(y)+np.abs(fitted)))
            moments.append(float(np.max(abs(moment)/scale)))
        X=data[finite].to_numpy()
        moments.append(float(np.max(abs(X.T@residual)/np.maximum(1,abs(X).T@(abs(y)+abs(fitted))))))
        result['maximum_model_score']=max(moments)
        # A package that keeps separated rows cannot meet the score, because
        # the separating slope diverges; its comparisons record the failure.
        if result['maximum_model_score']>1e-7 and not separation:raise ValueError('saved predictor fails independent model score')
        objective=float(residual@residual) if cell['model']=='ols' else float(np.sum(y*np.log(fitted)-fitted-gammaln(y+1)))
        reported=metadata.get('rss' if cell['model']=='ols' else 'll')
        if reported is None or not close_values([objective],[float(reported)],POLICY['objective_rtol'],POLICY['objective_atol']):
            raise ValueError('saved predictor does not replay reported objective')
        if metadata.get('native_arm')=='1' and cell['effects']>=2:
            # Retained components refine the full-sample ones, so zero means
            # on each retained component imply zero means on these.
            components=np.load(directory.parents[2]/'data'/(dataset_key(cell)+'.components.npz'))['component'][selected]
            weights=fitted if cell['model']=='ppml' else np.ones(len(data))
            mass=np.bincount(components,weights=weights)
            active=mass>0
            for j in range(2,cell['effects']+1):
                channel=rows[f'saved{j}'].to_numpy(float)[selected]
                means=np.bincount(components,weights=weights*channel)[active]/mass[active]
                scale=np.bincount(components,weights=weights*abs(channel))[active]/mass[active]
                if np.any(abs(means)>1e-9+1e-7*scale):raise ValueError('native saved FE component gauge mismatch')
    # Full row captures remain on SCC; compact collection uses exact hashes.
    write_json(directory/'capture.json',result)
    return result


def fit(root, cell, arm, repetition, frame, cpus):
    directory=root/'output'/cell['id']/f'{arm}-r{repetition}'
    directory.mkdir(parents=True,exist_ok=False)
    settings=dict(source=root/'source',refs=root/'environment/references',r_library=root/'environment/r-library',
        output=directory,dataset=root/'data'/(dataset_key(cell)+'.dta'),warm=root/'data/warm.dta',
        arm=arm,model=cell['model'],k=cell.get('regressors',16),effects=cell['effects'],threads=cell['threads'],
        save_fe=int(cell['save_fe']),vce=cell['vce'],tolerance='',clock_plugin=root/'environment/command-clock.plugin')
    if arm in JULIA_ARMS:
        settings.update(julia_version=POLICY['julia']['julia'],fixedeffectmodels=POLICY['julia']['FixedEffectModels'])
    write_configuration(directory,settings)
    if arm.startswith('fixest'):
        command=['Rscript',str(HERE/'campaign_r.R'),str(directory/'config.tsv')]
    elif arm in JULIA_ARMS:
        # JULIA_DEPOT_PATH (set by the job) holds the instantiated, precompiled pins.
        command=[os.environ.get('JULIA_BIN','julia'),'--startup-file=no',f"--threads={cell['threads']}",
                 f"--project={root/'environment/julia'}",str(HERE/'campaign_julia.jl'),str(directory/'config.tsv')]
    elif arm in PYFIXEST_ARMS or arm in PYFIXEST_STRICT_ARMS:
        interpreter=root/'environment'/arm.removesuffix('-strict')/'bin/python'
        command=[str(interpreter),'-I','-B',str(HERE/'campaign_pyfixest.py'),str(directory/'config.tsv')]
    else:
        command_file=directory/'command.do'
        command_file.write_text(f'do "{HERE}/campaign_stata.do" "{directory}/config.do"\n')
        command=[os.environ.get('STATA_BIN','stata-mp'),'-b','do',str(command_file)]
    observation=execute(command,directory,cpus[:cell['threads']],POLICY['process_timeout_seconds'])
    meta=read_metadata(directory/'metadata.tsv')
    meta['native_arm']='1' if arm=='fereg' else '0'
    meta['arm']=arm
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
            if arm=='fereg' or arm in STATA_REFERENCES:
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
    if a['status']!='ok' or b['status']!='ok':
        return dict(eligible=False,reasons=['application unavailable'],agreement='unavailable')
    x,y=a['capture'],b['capture'];reasons=[]
    for key in ['N','sample_sha256','omitted','finite_slope_rank','singleton_deletions','separation_deletions','clusters','absorbed_dof']:
        if key not in x or key not in y:reasons.append('missing '+key);continue
        if x[key]!=y[key]:reasons.append(key)
    if any(not c.get('covariance_policy') for c in [x,y]):reasons.append('missing covariance policy')
    allowed={f"stock-{cell['model']}-{cell['vce']}",f"{cell['model']}-{cell['vce']}-"+('K.adj=FALSE' if cell['model']=='ppml' else 'stock')}
    if cell['model']=='ols': allowed.add(f"ols-{cell['vce']}-K.exact=TRUE")
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
        # Predictions are missing outside the estimation sample; a sample
        # difference is already a reason of its own.
        retained=np.isfinite(p)
        if x['sample_sha256']==y['sample_sha256'] and (not np.array_equal(retained,np.isfinite(q))
                or not close_values(p[retained],q[retained],POLICY['fitted_rtol'],POLICY['fitted_atol'])):
            reasons.append('saved prediction')
        if max(x['maximum_model_score'],y['maximum_model_score'])>1e-7:reasons.append('model score')
    result=dict(eligible=not reasons,reasons=reasons,**agreement(x,y,reasons))
    if 'names' in x and 'names' in y and x['names']!=y['names']:
        # Informational: whether the slopes both packages estimate agree when
        # one of them keeps a separated control.
        common=[n for n in x['names'] if n in y['names']]
        result['common_coefficients']=close_values([x['beta'][x['names'].index(n)] for n in common],
                                                   [y['beta'][y['names'].index(n)] for n in common],
                                                   POLICY['coefficient_rtol'],POLICY['coefficient_atol'])
    return result


def agreement(x, y, reasons):
    """Separate counting conventions from numerical differences.

    Packages count absorbed degrees of freedom differently (connected
    components, levels, or nesting in clusters), which changes reported counts
    and multiplies the covariance by one small-sample scalar. A comparison
    agrees by convention when every other check passes and the covariance
    matches after one scalar within covariance_scale_bound of one.
    """
    if not reasons:
        return dict(agreement='exact',covariance_scale=1.)
    scale=None
    if 'covariance' in reasons:
        a,b=np.asarray(x['covariance'],dtype=float),np.asarray(y['covariance'],dtype=float)
        if a.shape==b.shape and np.isfinite(a).all() and np.isfinite(b).all() and (np.diag(a)>0).all():
            candidate=float(np.median(np.diag(b)/np.diag(a)))
            if (abs(candidate-1)<=POLICY['covariance_scale_bound']
                    and close_values(a,b/candidate,POLICY['vce_rtol'],POLICY['vce_atol'])):
                scale=candidate
    numerical=set(reasons)-COUNT_CONVENTIONS-({'covariance'} if scale is not None else set())
    return dict(agreement='differs' if numerical else 'convention',covariance_scale=scale,
                convention_reasons=sorted(set(reasons)-numerical))


def run_bundle(root, task, smoke=False, frozen_inputs=None, monitor=None):
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
    # The warm input must carry every control a cell in this bundle uses.
    warmcell=dict(cells[0],model='ppml',design='one-fe',effects=1,rows=512,
                  regressors=max(c.get('regressors',POLICY['controls']) for c in cells))
    make_dataset(warmcell,taskroot/'data/warm.dta')
    exhausted={}
    for cell in cells:
        if monitor:monitor.check()
        if time.monotonic()-started>POLICY['job_cleanup_seconds']:break
        key=dataset_key(cell)
        path=taskroot/'data'/(key+'.dta')
        if not path.exists():
            frozen=frozen_inputs and not smoke and key in frozen_inputs['datasets']
            frame,metadata=frozen_dataset(cell,path,frozen_inputs) if frozen else make_dataset(cell,path)
            datasets[key]=metadata
        else:frame=pd.read_stata(path,convert_categoricals=False)
        selected=arms(cell);failed=set()
        # A command that timed out at a smaller size is not attempted again in its series.
        limits=exhausted.get(series_key(cell),{})
        for repetition in range(1,cell['repetitions']+1):
            current={}
            order=selected[repetition%len(selected):]+selected[:repetition%len(selected)]
            for arm in order:
                if arm in limits and cell['rows']>limits[arm]:
                    records.append(dict(cell=cell['id'],arm=arm,repetition=repetition,status='skipped-after-timeout',
                                        timed_out_rows=limits[arm],metadata={}))
                    continue
                if arm in failed:
                    records.append(dict(cell=cell['id'],arm=arm,repetition=repetition,status='skipped-after-failure',metadata={}))
                    continue
                if time.monotonic()-started+POLICY['process_timeout_seconds']>POLICY['job_cleanup_seconds']:break
                record=fit(taskroot,cell,arm,repetition,frame,cpus)
                if monitor:monitor.check()
                records.append(record);current[arm]=record
                if record['status']!='ok':failed.add(arm)
                if record['status'] in ['timeout','command-timeout']:
                    exhausted.setdefault(series_key(cell),{}).setdefault(arm,cell['rows'])
                write_json(taskroot/'results.json',dict(cells=cells,records=records,datasets=datasets,comparisons=comparisons,complete=False))
            if 'fereg' in current:
                for arm in selected:
                    if arm=='fereg' or arm not in current:continue
                    result=compare_records(current['fereg'],current[arm],cell)
                    result.update(cell=cell['id'],arm=arm,repetition=repetition)
                    if result['agreement'] in ['exact','convention']:
                        result['ratio']=float(current['fereg']['metadata']['seconds'])/float(current[arm]['metadata']['seconds'])
                    comparisons.append(result)
        del frame
        if sha256(path)!=datasets[key]['sha256']:raise RuntimeError('input dataset changed')
    expected=sum(len(arms(c))*c['repetitions'] for c in cells)
    seen={(r['cell'],r['arm'],r['repetition']) for r in records}
    for cell in cells:
        for arm in arms(cell):
            for repetition in range(1,cell['repetitions']+1):
                if (cell['id'],arm,repetition) not in seen:
                    records.append(dict(cell=cell['id'],arm=arm,repetition=repetition,status='unattempted-budget',metadata={}))
    skipped=['unattempted-budget','skipped-after-failure','skipped-after-timeout']
    result=dict(cells=cells,records=records,datasets=datasets,comparisons=comparisons,
                complete=len(records)==expected and all(r['status'] not in skipped for r in records),
                coverage_complete=len(records)==expected,expected_fits=expected,host=socket.gethostname(),physical_cpus=cpus,
                wall_seconds=time.monotonic()-started)
    if smoke:
        from campaign_oracle import validate_smoke
        result['independent_science']=validate_smoke(result)
        accepted=all(x['pass'] for x in result['independent_science'] if x['arm']=='fereg')
        # Production relies on exact kernel peaks; every completed smoke fit must provide one.
        result['memory_exact']=all(r.get('memory_exact') for r in records if r['status']=='ok')
        result['native_gate_pass']=(accepted and result['memory_exact']
                                    and sum(r['arm']=='fereg' for r in records)==len(cells))
    write_json(taskroot/'results.json',result)
    verify_runtime(root)
    if smoke and not result['native_gate_pass']:raise RuntimeError('independent native smoke failed')
    (taskroot/'COMPLETE').write_text('CAMPAIGN_TASK_COMPLETE\n')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=['prepare','run','smoke'])
    parser.add_argument('root',type=Path)
    parser.add_argument('--task',type=int,default=1)
    parser.add_argument('--frozen-inputs',type=Path)
    parser.add_argument('--observe-host',action='store_true')
    args=parser.parse_args()
    if args.mode=='prepare':prepare(args.root)
    else:
        frozen=json.loads(args.frozen_inputs.read_text()) if args.frozen_inputs else None
        if args.observe_host:
            from qualify_scc import HostMonitor
            label='smoke' if args.mode=='smoke' else f'task-{args.task}'
            with HostMonitor(args.root/('host-'+label),max(POLICY['threads'])) as monitor:
                run_bundle(args.root,args.task,args.mode=='smoke',frozen,monitor)
        else:run_bundle(args.root,args.task,args.mode=='smoke',frozen)
