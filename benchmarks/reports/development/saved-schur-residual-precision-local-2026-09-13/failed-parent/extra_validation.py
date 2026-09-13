import os
from pathlib import Path
import copy,hashlib,json,os,re,subprocess
from measure import measure
from run import MANIFEST
from final_compare import certify
ROOT=Path(__file__).parent
PREVIOUS=Path(os.environ['SAVED_SCREEN_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((ROOT/'built-identities.json').read_text());assert sha(ROOT/'candidate/dist/fereg_macos.plugin')==ids['candidate_plugin']
results=[]
fixture=Path(os.path.join(os.environ['OMISSION_ROOT'], 'omitted_outputs_oracle.do'))
work=ROOT/'validation/omitted-candidate';work.mkdir(parents=True,exist_ok=False);source=ROOT/'candidate'
(work/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}" 0\nlocal actual=_rc\ndisplay "VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(work/'run.do')],cwd=work,env=env,capture_output=True,timeout=180)
(work/'process.stdout').write_bytes(p.stdout);(work/'process.stderr').write_bytes(p.stderr)
log=(work/'run.log').read_text(errors='replace');sentinel='OMITTED_OUTPUTS_ORACLE|fits=4|expected_rc=0|rc=0'
row=dict(name='omitted-oracle',process_rc=p.returncode,sentinel=bool(re.search('^'+re.escape(sentinel)+r'\s*$',log,re.M)),log_sha256=sha(work/'run.log'),fixture_sha256=sha(fixture),plugin=ids['candidate_plugin'])
results.append(row);(ROOT/'extra-validation-results.json').write_text(json.dumps(results,indent=2)+'\n');print(row,flush=True);assert p.returncode==0 and row['sentinel']
for name in ['weak_d3','disconnected_blocks','unbalanced']:
 request=next(r for r in json.loads((PREVIOUS/'control-requests.json').read_text()) if r['name']==name)
 for key in ['data','warm']:assert sha(Path(request[key]))==request[key+'_sha256']
 manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
 out=ROOT/'functional'/name;old=PREVIOUS/'runs'/f'control-{name}-s31415927-candidate'
 result=measure(ROOT,request['cell'],'fereg','warm',Path(request['data']),Path(request['warm']),out,manifest,source=ROOT/'candidate',identity={'candidate_plugin':ids['candidate_plugin'],'baseline_commit':ids['baseline_commit'],'purpose':'provenance against previously independent-certified candidate; outside paired timing aggregates'})
 row=dict(name=name,status=result['status'],metadata=result['metadata']);results.append(row);(ROOT/'extra-validation-results.json').write_text(json.dumps(results,indent=2)+'\n')
 assert result['status']=='pass',name
 row['native_science']=certify(out);row['exact_exports']={}
 for filename in ['b.csv','V.csv','fereg_fit.csv','fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv','fereg_certificate.csv','saved_outputs.csv']:
  a,b=old/filename,out/filename;assert a.exists()==b.exists(),(name,filename)
  if a.exists():assert a.read_bytes()==b.read_bytes(),(name,filename);row['exact_exports'][filename]=sha(b)
 (ROOT/'extra-validation-results.json').write_text(json.dumps(results,indent=2)+'\n');print({'name':name,'native_science':row['native_science'],'exact_exports':len(row['exact_exports'])},flush=True)
