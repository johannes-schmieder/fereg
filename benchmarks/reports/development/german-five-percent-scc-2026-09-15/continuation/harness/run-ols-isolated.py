from pathlib import Path
import argparse,concurrent.futures,hashlib,json,os,shutil,subprocess,time
p=argparse.ArgumentParser();p.add_argument('--root',type=Path,required=True);p.add_argument('--work-name',required=True);p.add_argument('--smoke',action='store_true');p.add_argument('--native-memory-bytes',type=int,required=True);a=p.parse_args();root=a.root
assert Path(a.work_name).name==a.work_name
summary=json.loads((root/'output/summary.json').read_text());assert summary['status']=='success'
plugin=root/'source/dist/fereg_linux.plugin';assert hashlib.sha256(plugin.read_bytes()).hexdigest()==summary['plugin_sha256']
refs=json.loads((root/'references.json').read_text())
for name,ref in refs.items():
 for rel,digest in ref['required_files'].items():assert hashlib.sha256((root/'references'/name/rel).read_bytes()).hexdigest()==digest
shared_dirs=[root/'references',root/'source/stata',root/'source/dist']
def shared_libraries():return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest()for d in shared_dirs for p in d.rglob('*.mlib')}
shared_before=shared_libraries();work=root/a.work_name;work.mkdir()
threads=min(4,int(os.environ['NSLOTS']));assert a.native_memory_bytes>0
base_env=dict(os.environ,OMP_NUM_THREADS=str(threads),OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1',NUMEXPR_NUM_THREADS='1',RAYON_NUM_THREADS=str(threads))
for key in ['FEREG_SOLVER_TRACE','FEREG_PPML_MEMORY_TRACE','FEREG_PPML_PREFLIGHT_RECEIPT']:base_env.pop(key,None)
base_env['FEREG_MEMORY_LIMIT_BYTES']=str(a.native_memory_bytes)
def run_stata(out,body,sentinel,app_threads=threads):
 out.mkdir()
 for name in ['empty-plus','empty-personal','compiled/l','scratch']:(out/name).mkdir(parents=True)
 for name,ref in refs.items():
  shutil.copytree(root/'references'/name,out/'references'/name,ignore=shutil.ignore_patterns('*.mlib','.git'))
  for rel,digest in ref['required_files'].items():assert hashlib.sha256((out/'references'/name/rel).read_bytes()).hexdigest()==digest
 (out/'job.do').write_text(body)
 (out/'run.do').write_text(f'capture noisily do "{out}/job.do"\nlocal rc=_rc\nfile open receipt using "{out}/result.txt", write replace\nfile write receipt "STATA_RC=`rc\'" _n\nfile close receipt\nexit `rc\'\n')
 env=dict(base_env,TMPDIR=str(out/'scratch'),OMP_NUM_THREADS=str(app_threads),RAYON_NUM_THREADS=str(app_threads));start=time.time()
 with (out/'process.stdout').open('w')as stdout,(out/'process.stderr').open('w')as stderr:
  result=subprocess.run(['/usr/bin/time','-v','stata-mp','-b','do','run.do'],cwd=out,env=env,stdout=stdout,stderr=stderr)
 ok=result.returncode==0 and (out/'result.txt').exists()and(out/'result.txt').read_text().strip()=='STATA_RC=0'and sentinel in(out/'run.log').read_text()
 libraries={str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest()for p in(out/'compiled').rglob('*.mlib')}
 receipt=dict(process_rc=result.returncode,elapsed_seconds=time.time()-start,accepted=ok,compiled_libraries=libraries,private_references_verified=True)
 (out/'completion.json').write_text(json.dumps(receipt,indent=2)+'\n')
 assert ok,str(out)
 assert libraries,'private compiled library missing'
 return receipt

def bootstrap(i):
 out=work/f'bootstrap-{i}'
 body=f'version 18.0\nclear all\nset more off\nset processors 1\nglobal S_ADO "BASE"\nsysdir set PLUS "{out}/empty-plus"\nsysdir set PERSONAL "{out}/empty-personal"\n'
 for name in refs:body+=f'adopath ++ "{out}/references/{name}/src"\n'
 body+=f'adopath ++ "{out}/compiled"\nftools, check\nfindfile lftools.mlib\nassert substr("`r(fn)\'",1,length("{out}/compiled/"))=="{out}/compiled/"\ndisplay as result "BOOTSTRAP_ISOLATION_COMPLETE"\n'
 return run_stata(out,body,'BOOTSTRAP_ISOLATION_COMPLETE',1)
if a.smoke:
 with concurrent.futures.ThreadPoolExecutor(max_workers=2)as team:bootstraps=list(team.map(bootstrap,[1,2]))
 assert shared_libraries()==shared_before
 (work/'bootstrap-isolation.json').write_text(json.dumps(dict(passed=True,parallel_processes=2,threads_per_process=1,private_compilation=True,shared_libraries_unchanged=True),indent=2)+'\n')
 data=root/'launcher-smoke-v2/input.dta'
else:
 assert json.loads((root/'analysis-ols-isolation-smoke/summary.json').read_text())['passed']
 data=root/'input/sample.dta';sample=json.loads((root/'input/sample-receipt.json').read_text());assert sample['roundtrip_passed']
 h=hashlib.sha256()
 with data.open('rb')as f:
  for block in iter(lambda:f.read(8*1024**2),b''):h.update(block)
 assert h.hexdigest()==sample['sample_sha256']
assert data.exists();jobs=[];repetitions=1 if a.smoke else 2
meta=dict(job_id=os.environ['JOB_ID'],host=os.uname().nodename,slots=int(os.environ['NSLOTS']),threads=threads,native_memory_limit_bytes=a.native_memory_bytes,harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),fit_do_sha256=hashlib.sha256((root/'harness/fit-isolated.do').read_bytes()).hexdigest(),plugin_sha256=summary['plugin_sha256'],expected_commands=2*repetitions,runs=jobs,complete=False)
def record():
 meta['complete']=len(jobs)==meta['expected_commands']and all(x['accepted']for x in jobs)
 temp=work/'runs.partial.json';temp.write_text(json.dumps(meta,indent=2)+'\n');temp.replace(work/'runs.json')
record()
for rep in range(1,repetitions+1):
 for arm in (['native','reference']if rep%2 else['reference','native']):
  out=work/f'ols-{arm}-{rep}';print('START',out.name,flush=True)
  accepted=False
  try:
   run_stata(out,f'do "{root}/harness/fit-isolated.do" "{root}" "{data}" "{out}" ols {arm} {threads} "{out}/references"\n','GERMAN_FIVE_PERCENT_FIT_COMPLETE')
   assert (out/'fit-result.txt').read_text().startswith('FIT_RC=0\n')and(out/'outputs.dta').exists()
   accepted=True
  finally:
   jobs.append(dict(family='ols',arm=arm,rep=rep,directory=out.name,accepted=accepted));record()
  print('END',out.name,flush=True)
assert shared_libraries()==shared_before,'shared compiled libraries changed'
assert meta['complete']
print('ISOLATED_OLS_COMMANDS_COMPLETE',flush=True)
