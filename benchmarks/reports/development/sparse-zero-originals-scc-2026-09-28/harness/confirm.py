"""Replay eight frozen native failures and independently verify retained row IDs."""
import argparse, hashlib, json, os, sys
from pathlib import Path
import numpy as np
import pandas as pd
p=argparse.ArgumentParser()
for name in ('run-root','source','reference','scratch'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();root=a.run_root.resolve()
sys.path.insert(0,str(root/'systematic'));sys.path.insert(0,str(root))
from common import read_json,sha256,write_json
from generate import generate
from runner import measure
from independent_graph_oracle import certificate
assert os.environ['NSLOTS']=='4' and len(os.sched_getaffinity(0))>=4
plan=read_json(root/'registration.json');source=read_json(root/'source.json')
for name,digest in source['files'].items():assert sha256(a.source/name)==digest,name
assert sha256(a.source/'dist/fereg_linux.plugin')==plan['plugin_sha256']
manifest=read_json(a.reference/'input/manifest.json')
work=root/'worker-root';work.mkdir(exist_ok=False)
for name,target in [('candidate',a.source),('references',a.reference/'references'),('env',a.reference/'env')]:
    (work/name).symlink_to(target,target_is_directory=True)
output=root/'results';output.mkdir(exist_ok=False);a.scratch.mkdir(exist_ok=False)
identity=dict(source=sha256(root/'source.json'),plugin=plan['plugin_sha256'],registration=sha256(root/'registration.json'),deployment=sha256(root/'deployment.json'))
fingerprint=hashlib.sha256(json.dumps(identity,sort_keys=True).encode()).hexdigest();write_json(output/'identity.json',identity)
results=[]
for issue in plan['issues']:
    input_path=next(name for name in issue['receipts'] if name.endswith('/input.json'))
    original=a.reference/input_path;assert sha256(original)==issue['receipts'][input_path]
    info=read_json(original)
    cell=dict(model='ppml',design=info['design'],n=info['n'],k=info['k'],threads=4,section=info['section'],effects=manifest['designs'][info['design']]['effects'])
    data=a.scratch/'input.dta';warm=a.scratch/'warm.dta';generated=generate(cell,info['seed'],data,manifest)
    assert generated['sha256']==info['sha256'] and generated['column_sha256']==info['column_sha256']
    frame=pd.read_stata(data,convert_categoricals=False);proof=certificate(frame)
    keep=~frame.obs_id.isin(proof['separated_obs_id']).to_numpy()
    while True:
        rows=np.flatnonzero(keep);active=frame.iloc[rows];retained=np.ones(len(rows),dtype=bool)
        for effect in cell['effects']:
            grouped=active.groupby(effect).y
            retained &= (grouped.transform('size').to_numpy()>1)&(grouped.transform('sum').to_numpy()>0)
        if retained.all():break
        keep[rows[~retained]]=False
    expected_hash=hashlib.sha256(frame.obs_id.to_numpy()[keep].astype('<i8').tobytes()).hexdigest()
    folder=output/issue['cell']/('warm-'+str(issue['repetition']));folder.mkdir(parents=True)
    proof.update(input_sha256=info['sha256'],retained_rows=int(keep.sum()),retained_original_ids_sha256=expected_hash,closure_rows=int((~keep).sum())-proof['separated_rows'])
    write_json(folder/'independent-sample.json',proof);write_json(folder/'input.json',info)
    generate(dict(cell,n=manifest['measurement']['warmup_rows'],section='main'),info['seed'],warm,manifest)
    receipt=measure(work,cell,'feppml','warm',data,warm,folder/'feppml',manifest,fingerprint,limits=plan['limits'])
    checks=dict(application=receipt['status']=='pass',converged=receipt['metadata'].get('converged')==1,exact_sample=receipt.get('sample_sha256')==expected_hash,retained_n=receipt.get('sample_rows')==int(keep.sum()),native_threads=receipt['metadata'].get('feppml_threads')==4 and receipt['metadata'].get('feppml_requested_threads')==4)
    result=dict(cell=issue['cell'],repetition=issue['repetition'],seed=info['seed'],original_status=issue['status'],original_receipts=issue['receipts'],input_sha256=info['sha256'],checks=checks,pass_all=all(checks.values()),command_seconds=receipt.get('seconds'),peak_rss_gib=receipt.get('peak_rss_gib'),independent_sample_sha256=sha256(folder/'independent-sample.json'),result_sha256=sha256(folder/'feppml/result.json'))
    results.append(result);write_json(output/'progress.json',results);print(json.dumps(result),flush=True)
    # Preserve every registered outcome; no retries or policy changes on failure.
    for f in (data,warm,data.with_suffix('.json'),warm.with_suffix('.json')):f.unlink()
write_json(output/'complete.json',dict(complete=True,cases=len(results),passed=sum(r['pass_all'] for r in results),diagnostic_only=True))
if not all(r['pass_all'] for r in results):raise RuntimeError('Original failures still need adjudication; preserve all results')
print('FEREG_ORIGINAL_ZERO_CONFIRMATION_COMPLETE',flush=True)
