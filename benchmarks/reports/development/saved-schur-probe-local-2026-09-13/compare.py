import argparse, copy, json
import numpy as np
import pandas as pd
from common import read_json, write_json, sha256
from run import BASE, MANIFEST
from preparation_compare import input_for
from final_compare import baseline_compare, certify, matrix
from measure import measure

PRIMARY = [
    dict(name='unit_saved', design='strong_d2', n=245760, k=50, threads=16,
         absorb='saved_g=g1 saved_h=g2', fit_options='residuals(saved_e)'),
    dict(name='aw_saved', design='strong_d2', n=983040, k=50, threads=4,
         weight='[aw=w]', absorb='saved_g=g1 saved_h=g2', fit_options='residuals(saved_e)'),
    dict(name='pw_partial', design='strong_d2', n=245760, k=50, threads=16,
         weight='[pw=w]', vce='cluster c1 c2', missing=True,
         absorb='saved_g=g1 g2', fit_options='residuals(saved_e)'),
]

def pair(spec, seed, rotation, trace):
    ids=read_json(BASE/'built-identities.json')
    assert sha256(BASE/'protocol.md') == ids['protocol_sha256']
    assert sha256(__file__) == ids['harness_sha256']
    data,warm=input_for(spec,seed),input_for(spec,seed+1,True)
    cell=dict(model='ols', section='width', effects=['g1','g2'], **spec)
    paths={}
    for arm in (['baseline','candidate'] if rotation%2==0 else ['candidate','baseline']):
        source=BASE/arm
        assert sha256(source/'dist/fereg_macos.plugin')==ids[arm+'_plugin_sha256']
        path=BASE/'runs'/f"{'profile' if trace else 'screen'}-{spec['name']}-s{seed}-{arm}"
        paths[arm]=path
        manifest=copy.deepcopy(MANIFEST)
        manifest['measurement']['command_timeout_seconds']=180
        result=measure(BASE,cell,'fereg','warm',data,warm,path,manifest,source=source,
            trace=trace,identity={'variant':arm,'plugin_sha256':ids[arm+'_plugin_sha256'],
                                 'baseline_commit':ids['baseline_commit'],'seed':seed})
        print(json.dumps({'case':spec['name'],'arm':arm,'status':result['status'],
                          'seconds':result['seconds'],'native_bytes':result['metadata'].get('fereg_native_allocation_bytes')}),flush=True)
        assert result['status']=='pass',str(path)
        write_json(path/'native-science.json',certify(path))
    comparison=baseline_compare(paths['candidate'],paths['baseline'])
    for name in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:
        a,b=(paths[x]/name for x in ['baseline','candidate'])
        assert a.exists()==b.exists()
        if a.exists(): assert a.read_bytes()==b.read_bytes(),(spec['name'],name)
    a,b=(pd.read_csv(paths[x]/'saved_outputs.csv') for x in ['baseline','candidate'])
    assert list(a.columns)==list(b.columns) and a.shape==b.shape
    assert np.array_equal(a.obs_id.to_numpy(),b.obs_id.to_numpy())
    saved={}
    for name in a.columns[1:]:
        x,y=(pd.to_numeric(frame[name],errors='coerce').to_numpy() for frame in [a,b])
        assert np.array_equal(np.isnan(x),np.isnan(y))
        assert np.allclose(x,y,atol=1e-9,rtol=1e-7,equal_nan=True),(spec['name'],name)
        saved[name]=float(np.nanmax(np.abs(x-y)))
    solver=matrix(paths['candidate'],'fereg_solver')
    assert solver[0,14]==8
    selected=solver[0,15]<=.01
    if not selected:
        for name in ['b.csv','V.csv','fereg_fit.csv','fereg_certificate.csv','saved_outputs.csv']:
            assert (paths['baseline']/name).read_bytes()==(paths['candidate']/name).read_bytes(),name
    write_json(paths['candidate']/'science.json',{'comparison':comparison,
        'sample_rank_status_dof_fe_cluster_exact':True,'saved_maxabs':saved,
        'schur_probe_accepted':bool(selected),'probe_contraction':float(solver[0,15])})
    b,c=(read_json(paths[x]/'result.json') for x in ['baseline','candidate'])
    gain=b['seconds']-c['seconds'];mb=b['metadata']['fereg_native_allocation_bytes'];mc=c['metadata']['fereg_native_allocation_bytes'];memory=mb-mc
    decision={'command_change_percent':-100*gain/b['seconds'],
        'native_change_percent':-100*memory/mb,
        'promising_screen':(gain>=.010 and gain/b['seconds']>=.05) or (memory>=2*1024**2 and memory/mb>=.05 and c['seconds']<=1.05*b['seconds']),
        'above_review_envelope':-gain>max(.010,.05*b['seconds']),
        'adoption_requires_recovery_admission_proof':True}
    write_json(paths['candidate']/'screen-decision.json',decision)
    print(json.dumps({'case':spec['name'],**decision}),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--prepare',action='store_true');p.add_argument('--trace',action='store_true');p.add_argument('--name');p.add_argument('--seed',type=int,default=104729);a=p.parse_args()
    seed=67280421 if a.trace else a.seed
    for i,spec in enumerate(PRIMARY):
        if a.name and a.name!=spec['name']:continue
        if a.prepare:
            for warm in [False,True]:print(input_for(spec,seed+int(warm),warm),flush=True)
        else:pair(spec,seed,i,a.trace)
