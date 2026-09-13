from pathlib import Path
import copy
from measure import measure
from common import sha256,write_json
from run import BASE,MANIFEST
from ppml_one_fe_compare import compare
records=[];paths={}
for n,seed in [(7680,104730),(30720,104729)]:
    data=BASE/'psd-data'/f'pw_257-n{n}-s{seed}.dta'
    cell=dict(name=f'pw_257_{n}',model='ppml',section='width',design='one_fe',n=n,k=257,
              threads=16,effects=['g1'],weight='[pw=w]',vce='cluster c1 c2',fit_options='offset(off)',missing=True)
    for arm in ['baseline','candidate']:
        source=BASE/arm;path=BASE/'runs'/f'psd-functional-n{n}-{arm}'
        manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
        result=measure(BASE,cell,'feppml','cold',data,data,path,manifest,
            identity=dict(base_commit='fd6b970a24f79a78e05fda585760beed59eb74dc',variant=arm,seed=seed,
                          plugin_sha256=sha256(source/'dist/fereg_macos.plugin')),source=source)
        expected=430 if n==7680 and arm=='baseline' else 0
        records.append(dict(n=n,arm=arm,expected_rc=expected,rc=result['metadata'].get('rc'),status=result['status'],
            sentinel=result['sentinel'],scientific_pass=result['status']=='pass'))
        write_json(BASE/'functional-results.json',records)
        print(n,arm,result['status'],result['metadata'].get('rc'),flush=True)
        assert result['sentinel'] and result['metadata']['rc']==expected
        if n==30720:paths[arm]=path
compare(paths,unaffected=True)
for name in ['b.csv','V.csv']:assert (paths['baseline']/name).read_bytes()==(paths['candidate']/name).read_bytes()
