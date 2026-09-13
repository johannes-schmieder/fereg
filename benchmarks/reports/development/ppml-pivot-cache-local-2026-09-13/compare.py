"""Paired pivot-cache evaluation; actual FE counts are explicit in each cell."""
import argparse,copy
from pathlib import Path
from common import sha256,write_json
from measure import measure
from run import BASE,MANIFEST
from ppml_one_fe_compare import compare as base_compare
from ppml_preparation_compare import PRIMARY as OLD_PRIMARY,CONTROLS,input_for
BASELINE=(BASE/'baseline-commit.txt').read_text().strip()
PRIMARY=[dict(name='unit_65',design='one_fe',n=122880,k=65,threads=4),
 dict(name='fw_129',design='one_fe',n=61440,k=129,threads=4,weight='[fw=w]',fit_options='exposure(exposure)',missing=True),
 dict(name='pw_257',design='one_fe',n=30720,k=257,threads=16,weight='[pw=w]',vce='cluster c1 c2',fit_options='offset(off)',missing=True)]
PROTECTED=[OLD_PRIMARY[0],OLD_PRIMARY[1],
 *[next(c for c in CONTROLS if c['name']==name) for name in ['low_fw','small_narrow','saved_separation']],
 dict(name='weak_pw_cluster_65',design='weak_d3',n=7680,k=65,threads=4,weight='[pw=w]',vce='cluster c1',fit_options='offset(off)'),
 dict(name='three_sparse_65',design='three_fe_sparse',n=7680,k=65,threads=4),
 dict(name='unit_64',design='one_fe',n=30720,k=64,threads=4),
 dict(name='no_fe_narrow',design='one_fe',n=7680,k=2,threads=1,effects=[])]
def pair(spec,seed,rotation=0,trace=False,prefix=''):
    data,warm=input_for(spec,seed),input_for(spec,seed+1,warm=True)
    cell=dict(model='ppml',section='width',**spec)
    cell['effects']=spec.get('effects',MANIFEST['designs'][spec['design']]['effects'])
    command='feppml'
    if trace:
        command='_fereg_estimate_ppml'
        option={'':'','[fw=w]':'fweight(w)','[pw=w]':'pweight(w)'}[spec.get('weight','')]
        cell['weight']='';cell['fit_options']=f"{spec.get('fit_options','')} allocationaudit {option}".strip()
    paths={}
    for arm in (['candidate','baseline'] if rotation%2 else ['baseline','candidate']):
        source=BASE/arm;path=BASE/'runs'/f'pivot-{prefix}{spec["name"]}-s{seed}-{arm}';paths[arm]=path
        manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
        result=measure(BASE,cell,command,'warm',data,warm,path,manifest,
            identity=dict(base_commit=BASELINE,variant=arm,seed=seed,trace=trace,plugin_sha256=sha256(source/'dist/fereg_macos.plugin')),
            source=source,trace=trace)
        print(path.name,result['status'],result['seconds'],flush=True)
        if result['status']!='pass':raise RuntimeError(path)
    base_compare(paths,unaffected=True)
    return paths
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('part',choices=['screen','repeat','controls','audit']);p.add_argument('--name');args=p.parse_args()
    if args.part=='screen':
        for i,s in enumerate(PRIMARY):pair(s,104729,rotation=i)
    elif args.part=='repeat':
        s=next(s for s in PRIMARY if s['name']==args.name)
        for i,seed in enumerate([8675309,20260819]):pair(s,seed,rotation=i+1)
    elif args.part=='controls':
        for i,s in enumerate(PROTECTED):pair(s,31415927,rotation=i,prefix='control-')
    else:pair(PRIMARY[2],67280421,rotation=1,trace=True,prefix='trace-')
