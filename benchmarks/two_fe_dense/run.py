"""Local two-FE comparisons on audited shared data and pinned application workers."""
import argparse
import datetime
import hashlib
import importlib.util
import itertools
import json
import os
from pathlib import Path
import sys

import numpy as np
import pandas as pd
from scipy.linalg import lstsq
from scipy.sparse import coo_matrix
from scipy.sparse.csgraph import connected_components

REPO = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


breadth = load("dense_breadth", REPO / "benchmarks/one_fe_breadth/run.py")
profile = load("dense_profile", REPO / "benchmarks/one_fe_profile/run.py")
CASES = breadth.CASES


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")


def stage(root):
    folder = breadth.stage(root, profile)
    p = folder / "stata_worker.do"
    text = p.read_text().replace('local outputvars out_fe\n', 'local outputvars out_fe1 out_fe2\n')
    text = text.replace('local outputvars out_fe out_resid out_xb out_xbd',
                        'local outputvars out_fe1 out_fe2 out_resid out_xb out_xbd')
    p.write_text(text)
    p = folder / "r_worker.R"
    text = p.read_text()
    text = breadth.replace_once(text, 'ssc(K.adj=TRUE, K.fixef="nonnested",',
                                'ssc(K.adj=TRUE, K.fixef="nonnested", K.exact=isTRUE(cell$exact_dof),')
    text = breadth.replace_once(text, 'nthreads=cell$threads, vcov=if',
                                'nthreads=cell$threads, fixef.tol=cell$fixest_tolerance, vcov=if')
    start = text.index('feols(formula,')
    end = text.index('notes=FALSE)',start)+len('notes=FALSE)')
    unweighted = text[start:end]
    weighted = unweighted.replace('data=data,','data=data, weights=~w,')
    text = text[:start] + 'if (cell$weight_kind == "none") {\n' + unweighted + '\n} else {\n' + weighted + '\n}' + text[end:]
    text = breadth.replace_once(text, 'meta$rss <- sum(resid(fit)^2)',
                                'meta$rss <- if (cell$weight_kind == "none") sum(resid(fit)^2) else sum(data$w[obs(fit)] * resid(fit)^2) * nobs(fit) / sum(data$w[obs(fit)])')
    start, end = text.index('recover_outputs <- function'), text.index('if (c$phase == "warm")')
    text = text[:start] + '''recover_outputs <- function(fit, data) {
    if (is.null(fit) || cell$outputs == "none") return(NULL)
    effects <- fixef(fit, sorted=FALSE, notes=FALSE, fixef.tol=cell$fixest_recovery_tolerance)
    left <- unname(effects$g1[fit$fixef_id$g1])
    right <- unname(effects$g2[fit$fixef_id$g2])
    contribution <- left + right
    component <- as.integer(data$component[obs(fit)])
    if (cell$weight_kind == "none") {
        intercept <- mean(contribution)
        centers <- as.vector(rowsum(right, component, reorder=FALSE)) /
                   as.vector(rowsum(rep(1, length(right)), component, reorder=FALSE))
    } else {
        weight <- data$w[obs(fit)]
        intercept <- weighted.mean(contribution, weight)
        centers <- as.vector(rowsum(weight * right, component, reorder=FALSE)) /
                   as.vector(rowsum(weight, component, reorder=FALSE))
    }
    centers <- centers[match(component, unique(component))]
    saved_outputs <- list(left + centers - intercept, right - centers)
    if (cell$outputs == "all") {
        residual <- resid(fit)
        prediction <- fitted(fit)
        finite <- prediction - contribution + intercept
        saved_outputs <- c(saved_outputs, list(residual, finite, prediction))
    }
    saved_outputs
}
''' + text[end:]
    text = breadth.replace_once(text, 'recover_outputs(fit_model(warm))', 'recover_outputs(fit_model(warm), warm)')
    text = breadth.replace_once(text, 'saved_outputs <- recover_outputs(fit)', 'saved_outputs <- recover_outputs(fit, data)')
    p.write_text(text)
    return folder


def dataset(root, n, k, seed, shuffled=False, design="two_fe_dense", weight_design="none"):
    suffix = '' if weight_design == 'none' else '-w' + weight_design
    path = root / "data" / f"{design}-n{n}-k{k}-s{seed}-shuffle{int(shuffled)}{suffix}.dta"
    if path.exists():
        assert breadth.sha(path) == json.loads(path.with_suffix('.json').read_text())["sha256"]
        if not path.with_suffix('.bin').exists():
            frame = pd.read_stata(path, convert_categoricals=False)
            with path.with_suffix('.bin').open('wb') as stream:
                for column in frame:
                    stream.write(np.asarray(frame[column], dtype='<f8').tobytes())
        return path
    sys.path.insert(0, str(REPO / 'benchmarks/systematic'))
    generator = load('dense_generator', REPO / 'benchmarks/systematic/generate.py')
    manifest = json.loads((REPO / 'benchmarks/systematic/manifest.json').read_text())
    parent = root / 'generated' / path.name
    generator.generate(dict(model='ols', design=design, n=n, k=k, section='width'), seed, parent, manifest)
    frame = pd.read_stata(parent, convert_categoricals=False)
    _, left = np.unique(frame.g1, return_inverse=True)
    _, right = np.unique(frame.g2, return_inverse=True)
    graph = coo_matrix((np.ones(n), (left, right + left.max() + 1)),
                       shape=(left.max()+right.max()+2,)*2).tocsr()
    components, labels = connected_components(graph, directed=False)
    frame['component'] = (labels[left] + 1).astype(float)
    ids = frame.obs_id.to_numpy().astype(np.int64) - 1
    frame['c1'] = (ids % (31 if n < 7680 else 1024) + 1).astype(float)
    frame['c2'] = ((ids // 7) % (17 if n < 7680 else 257) + 1).astype(float)
    if weight_design == 'constant':
        frame['w'] = 4.0
    elif weight_design == 'moderate':
        frame['w'] = (np.random.default_rng(seed + 524287).integers(1, 6, n)).astype(float)
    elif weight_design == 'dispersed':
        frame['w'] = 10.0 ** np.random.default_rng(seed + 524287).integers(-4, 5, n)
    else:
        assert weight_design == 'none'
    if shuffled:
        frame = frame.iloc[np.random.default_rng(seed + 1).permutation(n)].reset_index(drop=True)
    path.parent.mkdir(parents=True, exist_ok=True)
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000,1,1))
    np.testing.assert_array_equal(frame.to_numpy(), pd.read_stata(path).to_numpy())
    write(path.with_suffix('.json'), dict(n=n,k=k,seed=seed,design=design,shuffled=shuffled,weight_design=weight_design,
          components=int(components), columns=list(frame), parent_sha256=breadth.sha(parent), sha256=breadth.sha(path),
          generator_sha256=breadth.sha(REPO/'benchmarks/systematic/generate.py'),
          levels={c:int(frame[c].nunique()) for c in ['g1','g2','c1','c2']},
          column_sha256={c:hashlib.sha256(np.asarray(frame[c],dtype='<f8').tobytes()).hexdigest() for c in frame}))
    with path.with_suffix('.bin').open('wb') as stream:
        for column in frame:
            stream.write(np.asarray(frame[column], dtype='<f8').tobytes())
    return path


def compare_outputs(a, b, n, mode, record_disagreement=False):
    if mode == 'none':
        return {}
    count = 5 if mode == 'all' else 2
    x,y = [np.fromfile(p/'outputs.bin',dtype='<f8').reshape(count,n) for p in [a,b]]
    mismatch = np.abs(x-y) > 1e-9 + 1e-7*np.abs(y)
    finite = bool(np.isfinite(x).all() and np.isfinite(y).all())
    if not record_disagreement:
        np.testing.assert_allclose(x,y,rtol=1e-7,atol=1e-9)
    assert finite, 'nonfinite saved outputs'
    return dict(names=['fe1','fe2','residual','xb','xbd'][:count],
                status='mismatch' if mismatch.any() else 'pass',
                mismatched_values=mismatch.sum(axis=1).tolist(),
                max_abs_errors=np.max(np.abs(x-y),axis=1).tolist())


def oracle(data, output, cell):
    frame = pd.read_stata(data)
    k,n = cell['k'],len(frame)
    X = frame[[f'x{j}' for j in range(1,k+1)]].to_numpy()
    y = frame.y.to_numpy()
    kind = cell.get('weight_kind', 'none')
    raw_weight = np.ones(n) if kind == 'none' else frame.w.to_numpy()
    weight = raw_weight if kind in ('none', 'fw') else raw_weight * n / raw_weight.sum()
    effective_n = int(raw_weight.sum()) if kind == 'fw' else n
    sqrt_weight = np.sqrt(weight)
    blocks = [pd.get_dummies(frame[c],dtype=float).to_numpy() for c in ['g1','g2']]
    full_D = np.column_stack(blocks)
    # Ground one right-side level per component. Keeping both complete dummy
    # blocks makes the weighted SVD's default cutoff depend on roundoff in the
    # exact null directions rather than the graph's known rank.
    keep = np.ones(full_D.shape[1],dtype=bool)
    anchors = frame.groupby('component').g2.min().to_numpy()
    keep[blocks[0].shape[1]+np.searchsorted(np.unique(frame.g2),anchors)] = False
    D = full_D[:,keep]
    grounded_coefficients,_,fe_rank,_ = lstsq(D*sqrt_weight[:,None],
        np.column_stack([y,X])*sqrt_weight[:,None],lapack_driver='gelsd')
    assert fe_rank == D.shape[1]
    fe_coefficients = np.zeros((full_D.shape[1],k+1))
    fe_coefficients[keep] = grounded_coefficients
    within = np.column_stack([y,X])-D@grounded_coefficients
    beta,_,rank,_ = lstsq(within[:,1:]*sqrt_weight[:,None],within[:,0]*sqrt_weight,lapack_driver='gelsd')
    assert rank == k
    residual = within[:,0]-within[:,1:]@beta
    bread = np.linalg.inv(within[:,1:].T@(within[:,1:]*weight[:,None]))
    influence = (within[:,1:]@bread)*residual[:,None]
    full_rank = int(fe_rank)+k
    vce = cell['vce']
    if not vce:
        V = bread*(residual@(weight*residual))/(effective_n-full_rank)
    elif vce == 'robust':
        score_weight = sqrt_weight if kind == 'fw' else weight
        scores = influence*score_weight[:,None]
        V = scores.T@scores*effective_n/(effective_n-full_rank)
    else:
        names = vce.split()[1:]
        V = np.zeros((k,k))
        for count in range(1,len(names)+1):
            for subset in itertools.combinations(names,count):
                _, codes = np.unique(frame[list(subset)].to_numpy(),axis=0,return_inverse=True)
                sums = np.zeros((codes.max()+1,k))
                np.add.at(sums,codes,influence*weight[:,None])
                V += (-1)**(count+1)*(sums.T@sums)
        nonnested = [block for name,block in zip(['g1','g2'],blocks)
                     if not any(frame.groupby(name)[cluster].nunique().max()==1 for cluster in names)]
        df_model = k+(np.linalg.matrix_rank(np.column_stack(nonnested)) if nonnested else 1)
        g = min(frame[c].nunique() for c in names)
        V *= (effective_n-1)/(effective_n-df_model)*g/(g-1)
        eig,vectors = np.linalg.eigh(V)
        if eig.min()<0:
            V = (vectors*np.maximum(eig,0))@vectors.T
    names = [f'x{j}' for j in range(1,k+1)]
    actual = pd.read_csv(output/'coefficients.csv').set_index('name').loc[names,'value'].to_numpy()
    actual_v = pd.read_csv(output/'covariance.csv').pivot(index='row',columns='column',values='value').loc[names,names].to_numpy()
    np.testing.assert_allclose(actual,beta,rtol=1e-7,atol=1e-10)
    np.testing.assert_allclose(actual_v,V,rtol=1e-6,atol=1e-10)
    output_error = None
    if cell['outputs'] != 'none':
        coefficients = fe_coefficients[:,0]-fe_coefficients[:,1:]@beta
        left = blocks[0]@coefficients[:blocks[0].shape[1]]
        right = blocks[1]@coefficients[blocks[0].shape[1]:]
        intercept = np.average(left+right,weights=weight)
        for component in frame.component.unique():
            active = frame.component.to_numpy()==component
            center = np.average(right[active],weights=weight[active])
            right[active] -= center
            left[active] += center
        left -= intercept
        expected = [left,right]
        if cell['outputs']=='all':
            expected += [residual,X@beta+intercept,y-residual]
        actual = np.fromfile(output/'outputs.bin',dtype='<f8').reshape(len(expected),n)
        np.testing.assert_allclose(actual,expected,rtol=1e-7,atol=1e-9)
        output_error = float(np.max(np.abs(actual-expected)))
    result = json.loads((output/'result.json').read_text())
    assert result['metadata']['N']==effective_n
    np.testing.assert_allclose(result['metadata']['rss'],residual@(weight*residual),rtol=1e-8)
    if kind == 'fw':
        # Literal expansion is a separate numerical witness for the FW contract.
        Z = np.column_stack([X,D])
        copies = raw_weight.astype(int)
        expanded_Z,expanded_y = np.repeat(Z,copies,axis=0),np.repeat(y,copies)
        expanded_beta,_,expanded_rank,_ = lstsq(expanded_Z,expanded_y,lapack_driver='gelsd')
        np.testing.assert_allclose(beta,expanded_beta[:k],rtol=1e-7,atol=1e-10)
        assert int(expanded_rank)==full_rank
    return dict(status='pass',rank=full_rank,covariance_max_error=float(np.max(np.abs(actual_v-V))),output_max_error=output_error)


def main(a):
    root=a.root.resolve();root.mkdir(parents=True,exist_ok=True)
    folder=root/'controller'
    if not folder.exists():
        folder=stage(root)
    sys.path.insert(0,str(folder))
    measure=load('dense_measure',folder/'measure.py').measure
    manifest=json.loads((REPO/'benchmarks/systematic/manifest.json').read_text())
    manifest['measurement'].update(command_timeout_seconds=240,rss_interval_seconds=.05)
    (root/'work/refs').mkdir(parents=True,exist_ok=True)
    artifacts=(a.artifacts or a.root).resolve()
    identities={}
    for variant in a.variants:
        if variant!='fixest':
            identity=json.loads((artifacts/f'identity-{variant}.json').read_text())
            assert breadth.sha(artifacts/variant/'dist/fereg_macos.plugin')==identity['plugin']
            identities[variant]=(artifacts/variant,identity)
    shared=dict(fixest_archive_sha256=breadth.sha(breadth.DATA/'fixest.tar.gz'),
                controller={p.name:breadth.sha(p) for p in folder.iterdir() if p.is_file()},
                harness_sha256=breadth.sha(Path(__file__)), audit_sha256=breadth.sha(REPO/'benchmarks/one_fe_profile/run.py'))
    if 'fixest' in a.variants:
        libraries = [Path(p)/'fixest' for p in os.environ['R_LIBS_USER'].split(':')]
        library = next(p for p in libraries if (p/'DESCRIPTION').exists())
        shared['fixest_library'] = {str(p.relative_to(library)):breadth.sha(p)
                                   for p in sorted(library.rglob('*')) if p.is_file()}
    # Build every requested input before any timed command in this series.
    inputs={k:(dataset(root,a.n,k,a.seed,a.shuffled,a.design,a.weight_design),
               dataset(root,7680,k,a.seed,False,a.design,a.weight_design)) for k in a.widths}
    checks=[]
    for k in a.widths:
        data,warm=inputs[k]
        data_info = json.loads(data.with_suffix('.json').read_text())
        for case in a.cases:
            vce,r_vce,mode=CASES[case]
            cell=dict(model='ols',design=a.design,n=a.n,k=k,threads=a.threads,effects=['g1','g2'],
                      case=case,seed=a.seed,shuffled=a.shuffled,vce=vce,r_vce=r_vce,outputs=mode,
                      weight_kind=a.weight_kind,weight_design=a.weight_design,
                      weight='' if a.weight_kind=='none' else f'[{a.weight_kind}=w]',
                      exact_dof=data_info['components'] > 1,
                      fixest_tolerance=a.reference_tolerance if a.reference_precision else 1e-6,
                      fixest_recovery_tolerance=a.reference_tolerance if a.reference_precision or a.reference_recovery_precision else 1e-5,
                      absorb='out_fe1=g1 out_fe2=g2' if mode!='none' else 'g1 g2',
                      fit_options='residuals(out_resid)' if mode=='all' else '')
            os.environ['ONE_FE_BREADTH_OUTPUTS']=mode
            for rep in range(1,a.repeats+1):
                rotation=(rep-1)%len(a.variants);order=a.variants[rotation:]+a.variants[:rotation]
                outputs={}
                for variant in order:
                    arm='fixest' if variant=='fixest' else 'fereg'
                    source,identity=identities.get(variant,(None,None))
                    output=root/'runs'/f'{a.label}-{case}-n{a.n}-k{k}-s{a.seed}-shuffle{int(a.shuffled)}-t{a.threads}-r{rep}-{variant}'
                    result=measure(root,cell,arm,'warm',data,warm,output,manifest,
                                   identity=dict(shared,native=identity),source=source,trace=a.trace,solver=a.solver)
                    print(output.name,result['status'],result['seconds'],flush=True)
                    assert result['status']=='pass',output
                    outputs[variant]=output
                    if a.oracle:
                        checks.append(dict(run=output.name,oracle=oracle(data,output,cell)))
                for variant in a.variants[1:]:
                    reference,actual=outputs[a.variants[0]],outputs[variant]
                    comparison=profile.compare(reference,actual,exact_native=False)
                    if variant!='fixest' and a.variants[0]!='fixest':
                        for name in ['fereg_status','fereg_dof']:
                            np.testing.assert_array_equal(pd.read_csv(reference/(name+'.csv')),pd.read_csv(actual/(name+'.csv')))
                    checks.append(dict(case=case,n=a.n,k=k,rep=rep,variant=variant,science=comparison,
                                       outputs=compare_outputs(reference,actual,a.n,mode,
                                           a.record_reference_output_disagreement and variant=='fixest')))
                write(root/(a.label+'-progress.json'),checks)
    status = 'reference_output_mismatch' if any(c.get('outputs',{}).get('status')=='mismatch' for c in checks) else 'pass'
    write(root/(a.label+'-complete.json'),dict(status=status,checks=checks))
    print('TWO_FE_DENSE_COMPLETE',a.label,status,flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser()
    p.add_argument('root',type=Path);p.add_argument('--artifacts',type=Path)
    p.add_argument('--label',required=True);p.add_argument('--variants',nargs='+',default=['baseline','fixest'])
    p.add_argument('--cases',choices=list(CASES),nargs='+',default=['iid'])
    p.add_argument('--widths',nargs='+',type=int,default=[10,50]);p.add_argument('--n',type=int,default=983040)
    p.add_argument('--seed',type=int,default=8675309);p.add_argument('--shuffled',action='store_true')
    p.add_argument('--threads',type=int,default=16);p.add_argument('--repeats',type=int,default=3)
    p.add_argument('--design',choices=['two_fe_dense','two_fe_sparse'],default='two_fe_dense')
    p.add_argument('--trace',action='store_true');p.add_argument('--oracle',action='store_true')
    p.add_argument('--reference-precision',action='store_true',help='separate tight-reference diagnostic; headline timing retains defaults')
    p.add_argument('--reference-recovery-precision',action='store_true',help='tighten only fixef recovery for a separate output-accuracy diagnostic')
    p.add_argument('--reference-tolerance',type=float,default=1e-10,help='precision for explicitly requested tight-reference diagnostics')
    p.add_argument('--record-reference-output-disagreement',action='store_true',
                   help='record observed reference output mismatches explicitly; never classify them as passing science')
    p.add_argument('--solver',choices=['auto','map','cg','lsmr','cmg'],default='auto')
    p.add_argument('--weight-kind',choices=['none','aw','pw','fw'],default='none')
    p.add_argument('--weight-design',choices=['none','constant','moderate','dispersed'],default='none')
    a=p.parse_args()
    if a.oracle and a.n>3840:p.error('explicit dummy oracle is bounded to small inputs')
    if (a.weight_kind=='none') != (a.weight_design=='none'):p.error('weight kind and design must both be supplied')
    if a.weight_kind=='fw' and ('fixest' in a.variants or a.weight_design=='dispersed'):
        p.error('FW uses native comparisons and independent literal-expansion checks; fixest weights are not frequency semantics')
    if a.weight_kind=='pw' and any(CASES[c][0]=='' for c in a.cases):p.error('pweights require robust or clustered inference')
    main(a)
