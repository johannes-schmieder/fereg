"""Weighted, inference and output checks for one-partition absorbed slopes.

Inputs and application workers are hashed before dispatch. Small fixtures use
an independent explicit-dummy SVD oracle, including literal FW expansion.
"""
import argparse
import datetime
import itertools
import json
import os
from pathlib import Path
import sys
import numpy as np
import pandas as pd
from scipy.linalg import lstsq
import slopes as base

REPO=base.REPO
breadth=base.load('slope_breadth_workers', REPO/'benchmarks/one_fe_breadth/run.py')
CASES=dict(breadth.CASES)
CASES['cluster2_outputs']=('cluster c1 c2','~c1+c2','all')
CASES['robust_outputs']=('robust','hetero','all')


def stage(root, names, intercept, memory_limit=None):
    folder=breadth.stage(root,base.profile)
    if memory_limit is not None:
        p = folder / 'measure.py'
        marker = '    environment.update(thread_environment(threads))'
        p.write_text(breadth.replace_once(p.read_text(), marker,
            marker + '\n    environment["FEREG_MEMORY_LIMIT_BYTES"] = ' + repr(str(memory_limit))))
    p=folder/'r_worker.R'; text=p.read_text()
    effect='g1['+('' if intercept else '[')+','.join(names)+']'+('' if intercept else ']')
    text=breadth.replace_once(text,'paste(cell$effects, collapse=" + ")',json.dumps(effect))
    if not intercept:
        text=breadth.replace_once(text,'paste(paste0("x", seq_len(cell$k)), collapse=" + ")',
            'paste(paste(paste0("x", seq_len(cell$k)), collapse=" + "), "- 1")')
    first=text.index('feols(formula,');last=text.index('notes=FALSE)',first)+len('notes=FALSE)')
    unweighted=text[first:last];weighted=unweighted.replace('data=data,','data=data, weights=~w,')
    text=text[:first]+'if (cell$weight_kind=="none") {\n'+unweighted+'\n} else {\n'+weighted+'\n}'+text[last:]
    text=breadth.replace_once(text,'meta$rss <- sum(resid(fit)^2)',
        'meta$rss <- if (cell$weight_kind=="none") sum(resid(fit)^2) else sum(data$w[obs(fit)]*resid(fit)^2)*nobs(fit)/sum(data$w[obs(fit)])')
    first=text.index('recover_outputs <- function');last=text.index('if (c$phase == "warm")',first)
    helper='''recover_outputs <- function(fit, data) {
    if (is.null(fit) || cell$outputs=="none") return(NULL)
    recovered <- fixef(fit, sorted=FALSE, notes=FALSE)
    selected <- obs(fit)
    ids <- fit$fixef_id$g1
    channels <- list()
    contribution <- numeric(length(ids))
    if (cell$intercept) {
        channels[[1]] <- unname(recovered$g1[ids])
        contribution <- channels[[1]]
    }
    for (name in cell$slope_names) {
        channel <- unname(recovered[[paste0("g1[[",name,"]]")]][ids])
        channel <- channel*data[[name]][selected]
        channels[[length(channels)+1]] <- channel
        contribution <- contribution+channel
    }
    constant <- if (!cell$intercept) 0 else if (cell$weight_kind=="none") mean(contribution) else weighted.mean(contribution,data$w[selected])
    if (cell$intercept) channels[[1]] <- channels[[1]]-constant
    if (cell$outputs=="all") {
        channels <- c(channels,list(resid(fit),fitted(fit)-contribution+constant,fitted(fit)))
    }
    channels
}
'''
    text=text[:first]+helper+text[last:]
    text=text.replace('recover_outputs(fit_model(warm))','recover_outputs(fit_model(warm),warm)')
    text=text.replace('saved_outputs <- recover_outputs(fit)','saved_outputs <- recover_outputs(fit,data)')
    p.write_text(text)
    p=folder/'stata_worker.do';text=p.read_text()
    saved=(['out_fe'] if intercept else [])+[f'out_fe_Slope{i+1}' for i in range(len(names))]
    text=breadth.replace_once(text,'local outputvars out_fe\n','local outputvars '+' '.join(saved)+'\n')
    text=breadth.replace_once(text,'local outputvars out_fe out_resid out_xb out_xbd',"local outputvars `outputvars' out_resid out_xb out_xbd")
    p.write_text(text)
    return folder


def dataset(root,n,k,seed,names,intercept,weight):
    path=base.dataset(root,n,k,seed)
    frame=pd.read_stata(path,convert_categoricals=False)
    for i,name in enumerate(names[1:],start=2):
        frame[name]=np.random.default_rng(seed+7919*i).normal(size=n)
        frame['y']+=0.2/i*np.cos(frame.g1)*frame[name]
    ids=frame.obs_id.to_numpy().astype(np.int64)-1
    frame['c1']=(ids%(31 if n<=1920 else 1024)+1).astype(float)
    frame['c2']=((ids//7)%(17 if n<=1920 else 257)+1).astype(float)
    if weight!='none':frame['w']=(1+ids%7).astype(float)
    frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
    np.testing.assert_array_equal(frame.to_numpy(),pd.read_stata(path).to_numpy())
    info=json.loads(path.with_suffix('.json').read_text())
    info.update(columns=list(frame),sha256=base.sha(path),breadth_generator_sha256=base.sha(Path(__file__)),
        column_sha256={c:__import__('hashlib').sha256(np.asarray(frame[c],dtype='<f8').tobytes()).hexdigest() for c in frame})
    base.write(path.with_suffix('.json'),info)
    with path.with_suffix('.bin').open('wb') as f:
        for c in frame:f.write(np.asarray(frame[c],dtype='<f8').tobytes())
    return path


def oracle(path,output,cell):
    frame=pd.read_stata(path);n=len(frame);k=cell['k'];assert n<=1920
    X=frame[[f'x{i+1}' for i in range(k)]].to_numpy();y=frame.y.to_numpy()
    G=pd.get_dummies(frame.g1,dtype=float).to_numpy()
    blocks=([G] if cell['intercept'] else [])+[G*frame[name].to_numpy()[:,None] for name in cell['slope_names']]
    D=np.column_stack(blocks);Z=np.column_stack([X,D])
    kind=cell['weight_kind'];w=np.ones(n) if kind=='none' else frame.w.to_numpy()
    effective_n=int(w.sum()) if kind=='fw' else n
    weights=w if kind in ('none','fw') else w*n/w.sum();roots=np.sqrt(weights)
    b,_,rank,_=lstsq(Z*roots[:,None],y*roots,lapack_driver='gelsd')
    residual=y-Z@b
    projected,_,fe_rank,_=lstsq(D*roots[:,None],X*roots[:,None],lapack_driver='gelsd')
    within=X-D@projected
    bread=np.linalg.inv(within.T@(within*weights[:,None]))
    influence=(within@bread)*residual[:,None]
    vce=cell['vce']
    if not vce:V=bread*(residual@(weights*residual))/(effective_n-rank)
    elif vce=='robust':
        scores=influence*(roots if kind=='fw' else weights)[:,None]
        V=scores.T@scores*effective_n/(effective_n-rank)
    else:
        cluster_names=vce.split()[1:];V=np.zeros((k,k))
        for size in range(1,len(cluster_names)+1):
            for subset in itertools.combinations(cluster_names,size):
                _,codes=np.unique(frame[list(subset)].to_numpy(),axis=0,return_inverse=True)
                sums=np.zeros((codes.max()+1,k));np.add.at(sums,codes,influence*weights[:,None])
                V+=(-1)**(size+1)*(sums.T@sums)
        nested=any(frame.groupby('g1')[c].nunique().max()==1 for c in cluster_names)
        # Stata nesting exempts the partition's intercepts, not its slope
        # channels. The full dummy rank remains the fit-DoF authority.
        df=k+fe_rank-(G.shape[1]-1 if nested and cell['intercept'] else 0)
        clusters=min(frame[c].nunique() for c in cluster_names)
        V*=(effective_n-1)/(effective_n-df)*clusters/(clusters-1)
        eig,vectors=np.linalg.eigh(V)
        if eig.min()<0:V=(vectors*np.maximum(eig,0))@vectors.T
    names=[f'x{i+1}' for i in range(k)]
    actual_b=pd.read_csv(output/'coefficients.csv').set_index('name').loc[names,'value'].to_numpy()
    actual_V=pd.read_csv(output/'covariance.csv').pivot(index='row',columns='column',values='value').loc[names,names].to_numpy()
    np.testing.assert_allclose(actual_b,b[:k],rtol=1e-7,atol=1e-10)
    np.testing.assert_allclose(actual_V,V,rtol=1e-6,atol=1e-10)
    meta=json.loads((output/'result.json').read_text())['metadata']
    assert meta['N']==effective_n
    np.testing.assert_allclose(meta['rss'],residual@(weights*residual),rtol=1e-8,atol=1e-10)
    if kind=='fw':
        index=np.repeat(np.arange(n),w.astype(int))
        expanded,_,expanded_rank,_=lstsq(Z[index],y[index],lapack_driver='gelsd')
        np.testing.assert_allclose(expanded[:k],b[:k],rtol=1e-9,atol=1e-10)
        assert expanded_rank==rank
        expanded_projection,_,_,_=lstsq(D[index],X[index],lapack_driver='gelsd')
        expanded_within=X[index]-D[index]@expanded_projection
        expanded_bread=np.linalg.inv(expanded_within.T@expanded_within)
        expanded_residual=y[index]-Z[index]@expanded
        expanded_scores=(expanded_within@expanded_bread)*expanded_residual[:,None]
        if not vce:
            expanded_V=expanded_bread*(expanded_residual@expanded_residual)/(len(index)-expanded_rank)
        elif vce=='robust':
            expanded_V=expanded_scores.T@expanded_scores*len(index)/(len(index)-expanded_rank)
        else:
            expanded_V=np.zeros((k,k))
            for size in range(1,len(cluster_names)+1):
                for subset in itertools.combinations(cluster_names,size):
                    _,codes=np.unique(frame[list(subset)].to_numpy()[index],axis=0,return_inverse=True)
                    sums=np.zeros((codes.max()+1,k));np.add.at(sums,codes,expanded_scores)
                    expanded_V+=(-1)**(size+1)*(sums.T@sums)
            expanded_V*=(len(index)-1)/(len(index)-df)*clusters/(clusters-1)
            eig,vectors=np.linalg.eigh(expanded_V)
            if eig.min()<0:expanded_V=(vectors*np.maximum(eig,0))@vectors.T
        np.testing.assert_allclose(expanded_V,V,rtol=1e-9,atol=1e-10)
    output_check={}
    if cell['outputs']!='none':
        count=len(blocks)+(3 if cell['outputs']=='all' else 0)
        outputs=np.fromfile(output/'outputs.bin',dtype='<f8').reshape(count,n)
        contribution=outputs[:len(blocks)].sum(axis=0)
        constant=np.average(y-X@b[:k],weights=weights) if cell['intercept'] else 0.0
        np.testing.assert_allclose(contribution+constant+X@b[:k],y-residual,rtol=1e-7,atol=1e-8)
        if cell['outputs']=='all':
            np.testing.assert_allclose(outputs[-3],residual,rtol=1e-7,atol=1e-8)
            np.testing.assert_allclose(outputs[-2],X@b[:k]+constant,rtol=1e-7,atol=1e-8)
            np.testing.assert_allclose(outputs[-1],y-residual,rtol=1e-7,atol=1e-8)
        output_check=dict(reconstruction='pass')
    return dict(status='pass',rank=int(rank),fe_rank=int(fe_rank),effective_n=effective_n,
        coefficient_max_error=float(np.max(np.abs(actual_b-b[:k]))),covariance_max_error=float(np.max(np.abs(actual_V-V))),outputs=output_check)


def audit_outputs(path,output,cell):
    """Independent span, normal-equation and reconstruction check at large N."""
    if cell['outputs']=='none':return dict(status='not_requested')
    frame=pd.read_stata(path);n=len(frame);k=cell['k']
    blocks=len(cell['slope_names'])+int(cell['intercept'])
    values=np.fromfile(output/'outputs.bin',dtype='<f8').reshape(blocks+(3 if cell['outputs']=='all' else 0),n)
    assert np.isfinite(values).all()
    names=[f'x{i+1}' for i in range(k)]
    b=pd.read_csv(output/'coefficients.csv').set_index('name').loc[names,'value'].to_numpy()
    X=frame[names].to_numpy();y=frame.y.to_numpy()
    weight=np.ones(n) if cell['weight_kind']=='none' else frame.w.to_numpy()
    _,first,codes=np.unique(frame.g1,return_index=True,return_inverse=True)
    group_weight=np.bincount(codes,weights=weight)
    constant=np.average(y-X@b,weights=weight) if cell['intercept'] else 0.0
    xb=X@b+constant;prediction=xb+values[:blocks].sum(axis=0);residual=y-prediction
    moments=[]
    if cell['intercept']:
        np.testing.assert_array_equal(values[0],values[0,first][codes])
        moments.append(np.bincount(codes,weights=weight*residual)/group_weight)
    for j,name in enumerate(cell['slope_names'],start=int(cell['intercept'])):
        z=frame[name].to_numpy()
        coefficients=np.bincount(codes,weights=z*values[j])/np.bincount(codes,weights=z*z)
        np.testing.assert_allclose(values[j],z*coefficients[codes],rtol=1e-7,atol=1e-9)
        moments.append(np.bincount(codes,weights=weight*z*residual)/group_weight)
    fe_score=max(float(np.max(np.abs(v))) for v in moments)
    finite_score=float(np.max(np.abs(X.T@(weight*residual)))/weight.sum())
    assert fe_score<1e-9 and finite_score<1e-9,(fe_score,finite_score)
    if cell['outputs']=='all':
        np.testing.assert_allclose(values[-3:],np.array([residual,xb,prediction]),rtol=1e-7,atol=1e-9)
    rss=float(residual@(weight*residual))
    if cell['weight_kind'] in ('aw','pw'):rss*=n/weight.sum()
    meta=json.loads((output/'result.json').read_text())['metadata']
    np.testing.assert_allclose(meta['rss'],rss,rtol=1e-8,atol=1e-10)
    assert meta['N']==(int(weight.sum()) if cell['weight_kind']=='fw' else n)
    return dict(status='pass',fe_score=fe_score,finite_score=finite_score)


def main(a):
    if a.weight_kind=='fw' and 'fixest' in a.variants:raise ValueError('Ordinary fixest weights are not an FW inference comparator')
    if a.weight_kind=='pw' and any(not CASES[c][0] for c in a.cases):raise ValueError('PW requires robust or clustered inference')
    if a.memory_limit_bytes is not None:
        if a.memory_limit_bytes <= 0: raise ValueError('Memory limit must be positive')
        os.environ['FEREG_MEMORY_LIMIT_BYTES'] = str(a.memory_limit_bytes)
    memory_limit = os.environ.get('FEREG_MEMORY_LIMIT_BYTES')
    root=a.root.resolve();root.mkdir(parents=True,exist_ok=False);(root/'work/refs').mkdir(parents=True)
    names=['z']+[f'z{i}' for i in range(2,a.slope_count+1)];intercept=not a.slope_only
    folder=stage(root,names,intercept,memory_limit);sys.path.insert(0,str(folder));measure=base.load('slope_measure_breadth',folder/'measure.py').measure
    artifacts=a.artifacts.resolve();identities={}
    for variant in a.variants:
        if variant!='fixest':
            identity=json.loads((artifacts/f'identity-{variant}.json').read_text())
            assert base.sha(artifacts/variant/'dist/fereg_macos.plugin')==identity['plugin']
            for path,digest in identity['source'].items():assert base.sha(artifacts/variant/path)==digest,path
            identities[variant]=identity
    library=next(Path(p)/'fixest' for p in os.environ['R_LIBS_USER'].split(':') if (Path(p)/'fixest/DESCRIPTION').exists())
    reference=dict(archive_sha256=base.sha(a.fixest_archive),library={str(p.relative_to(library)):base.sha(p) for p in sorted(library.rglob('*')) if p.is_file()})
    cells=[]
    for k,t,case in itertools.product(a.widths,a.threads,a.cases):
        vce,r_vce,outputs=CASES[case]
        term='g1'+('##' if intercept else '#')+'c.('+ ' '.join(names)+')'
        fit_options = ['residuals(out_resid)'] if outputs == 'all' else []
        if a.memory: fit_options.append(f'memory({a.memory})')
        cells.append(dict(model='ols',design='one_fe',n=a.n,k=k,threads=t,effects=['g1'],
            absorb=('out_fe=' if outputs!='none' else '')+term,vce=vce,r_vce=r_vce,outputs=outputs,case=case,
            weight_kind=a.weight_kind,weight='' if a.weight_kind=='none' else f'[{a.weight_kind}=w]',
            fit_options=' '.join(fit_options),intercept=intercept,slope_names=names,seed=a.seed,section='width'))
    plan=dict(schema='one-fe-slope-breadth-v1',cells=cells,variants=a.variants,repeats=a.repeats,diagnostic=a.trace,solver=a.solver,
        identities=identities,reference=reference,controller={p.name:base.sha(p) for p in folder.iterdir() if p.is_file()},
        runner_sha256=base.sha(Path(__file__)),parent_runner_sha256=base.sha(Path(base.__file__)),
        memory_limit_bytes=int(memory_limit) if memory_limit else None)
    base.write(root/'plan.json',plan)
    manifest=json.loads((REPO/'benchmarks/systematic/manifest.json').read_text());manifest['measurement'].update(command_timeout_seconds=120,rss_interval_seconds=.05)
    inputs={}
    for k in a.widths:
        data=dataset(root,a.n,k,a.seed,names,intercept,a.weight_kind)
        warm=data if a.n<=7680 else dataset(root,7680,k,a.seed,names,intercept,a.weight_kind)
        inputs[k]=(data,warm)
    checks=[];reference_discrepancies=[]
    for i,cell in enumerate(cells):
        os.environ['ONE_FE_BREADTH_OUTPUTS']=cell['outputs'];data,warm=inputs[cell['k']]
        for rep in range(a.repeats):
            rotation=rep%len(a.variants);order=a.variants[rotation:]+a.variants[:rotation];outputs={}
            for variant in order:
                out=root/'runs'/f'c{i:03d}-r{rep}-{variant}'
                os.environ['ONE_FE_BREADTH_TRACE_DIR']=str(out)
                result=measure(root,cell,'fixest' if variant=='fixest' else 'fereg','warm',data,warm,out,manifest,
                    source=None if variant=='fixest' else artifacts/variant,identity=dict(native=identities.get(variant),reference=reference,
                        plan_sha256=base.sha(root/'plan.json')),trace=a.trace,solver=a.solver)
                print(out.name,result['status'],result['seconds'],flush=True);assert result['status']=='pass',out
                outputs[variant]=out
                if a.n<=1920:
                    try:check=oracle(data,out,cell)
                    except AssertionError as error:
                        if variant!='fixest':raise
                        check=dict(status='reference_discrepancy',detail=str(error))
                        reference_discrepancies.append(dict(run=out.name,oracle=check))
                    checks.append(dict(run=out.name,oracle=check))
                try:check=audit_outputs(data,out,cell)
                except AssertionError as error:
                    if variant!='fixest':raise
                    check=dict(status='reference_discrepancy',detail=str(error))
                    reference_discrepancies.append(dict(run=out.name,outputs=check))
                checks.append(dict(run=out.name,outputs=check))
            for variant in a.variants[1:]:
                try:check=base.profile.compare(outputs[a.variants[0]],outputs[variant],exact_native=False)
                except AssertionError as error:
                    if 'fixest' not in (a.variants[0],variant):raise
                    check=dict(status='reference_discrepancy',detail=str(error))
                    reference_discrepancies.append(dict(cell=i,rep=rep,variant=variant,science=check))
                checks.append(dict(cell=i,rep=rep,variant=variant,science=check))
            base.write(root/'progress.json',checks)
            base.write(root/'reference-discrepancies.json',reference_discrepancies)
    base.write(root/'complete.json',dict(status='pass' if not reference_discrepancies else 'native_pass_reference_discrepancies',checks=checks,reference_discrepancies=reference_discrepancies,plan_sha256=base.sha(root/'plan.json')))
    print('SLOPE_BREADTH_COMPLETE',flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path)
    p.add_argument('--artifacts',type=Path,required=True);p.add_argument('--fixest-archive',type=Path,required=True)
    p.add_argument('--variants',nargs='+',default=['baseline','fixest']);p.add_argument('--n',type=int,default=245760)
    p.add_argument('--widths',type=int,nargs='+',default=[10,50]);p.add_argument('--threads',type=int,nargs='+',default=[1,16])
    p.add_argument('--seed',type=int,default=104729);p.add_argument('--repeats',type=int,default=3);p.add_argument('--trace',action='store_true')
    p.add_argument('--solver',choices=['auto','map','cg','lsmr'],default='auto');p.add_argument('--weight-kind',choices=['none','aw','pw','fw'],default='none')
    p.add_argument('--slope-count',type=int,choices=[1,2,3],default=1);p.add_argument('--slope-only',action='store_true')
    p.add_argument('--cases',choices=list(CASES),nargs='+',default=['iid'])
    p.add_argument('--memory', choices=['auto', 'fast', 'low'])
    p.add_argument('--memory-limit-bytes', type=int, help='recorded development whole-action memory test seam')
    main(p.parse_args())
