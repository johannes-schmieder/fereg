from pathlib import Path
import argparse,subprocess,json,os,hashlib,time
p=argparse.ArgumentParser();p.add_argument('--root',type=Path,required=True);p.add_argument('--smoke',action='store_true');p.add_argument('--native-memory-bytes',type=int,required=True);p.add_argument('--input',type=Path);p.add_argument('--family',choices=['ols','ppml']);a=p.parse_args();root=a.root
assert (root/'output/DEV_SMOKE.SUCCESS').exists(), 'native build/smoke must pass first'
summary=json.loads((root/'output/summary.json').read_text());assert summary['status']=='success'
plugin=root/'source/dist/fereg_linux.plugin';assert hashlib.sha256(plugin.read_bytes()).hexdigest()==summary['plugin_sha256']
refs=json.loads((root/'references.json').read_text())
for name,ref in refs.items():
 for f,d in ref['required_files'].items():assert hashlib.sha256((root/'references'/name/f).read_bytes()).hexdigest()==d
threads=min(4,int(os.environ['NSLOTS']));env=dict(os.environ,OMP_NUM_THREADS=str(threads),OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1',NUMEXPR_NUM_THREADS='1',RAYON_NUM_THREADS=str(threads))
for key in ['FEREG_MEMORY_LIMIT_BYTES','FEREG_PPML_PREFLIGHT_RECEIPT','FEREG_SOLVER_TRACE','FEREG_PPML_MEMORY_TRACE']:env.pop(key,None)
assert a.native_memory_bytes>0
env['FEREG_MEMORY_LIMIT_BYTES']=str(a.native_memory_bytes)
work=root/('launcher-smoke-v2' if a.smoke else 'comparison-'+a.family);work.mkdir()

def run_stata(out,body,sentinel):
 out.mkdir(exist_ok=True)
 for name in ['empty-plus','empty-personal']: (out/name).mkdir(exist_ok=True)
 (out/'job.do').write_text(body)
 (out/'run.do').write_text(f'capture noisily do "{out}/job.do"\nlocal rc=_rc\nfile open receipt using "{out}/result.txt", write replace\nfile write receipt "STATA_RC=`rc\'" _n\nfile close receipt\nexit `rc\'\n')
 start=time.time()
 with (out/'process.stdout').open('w') as stdout,(out/'process.stderr').open('w') as stderr:
  res=subprocess.run(['/usr/bin/time','-v','stata-mp','-b','do','run.do'],cwd=out,env=env,stdout=stdout,stderr=stderr)
 ok=res.returncode==0 and (out/'result.txt').exists() and (out/'result.txt').read_text().strip()=='STATA_RC=0' and sentinel in (out/'run.log').read_text()
 (out/'completion.json').write_text(json.dumps(dict(process_rc=res.returncode,elapsed_seconds=time.time()-start,accepted=ok),indent=2)+'\n')
 assert ok,str(out)

bootstrap='version 18.0\nclear all\nset more off\n'+f'sysdir set PLUS "{work}/bootstrap/empty-plus"\nsysdir set PERSONAL "{work}/bootstrap/empty-personal"\n'
for name in refs:bootstrap+=f'adopath ++ "{root}/references/{name}/src"\n'
bootstrap+='ftools, check\nwhich reghdfe\nwhich ppmlhdfe\ndisplay as result "REFERENCE_BOOTSTRAP_COMPLETE"\n'
run_stata(work/'bootstrap',bootstrap,'REFERENCE_BOOTSTRAP_COMPLETE')
if a.smoke:
 data=work/'input.dta'
 body=f'''version 18.0
clear all
set more off
set obs 9000
generate long obs_id=_n
generate long persid=1+floor((_n-1)/9)
generate int year=2001+mod(_n-1,9)
generate long betnr=1+mod(persid+3*(year-2001),37)
generate int age=25+mod(persid,25)+year-2001
generate byte edyrs=cond(mod(persid,5)==0,.,cond(mod(persid,5)==1,10,cond(mod(persid,5)==2,13,cond(mod(persid,5)==3,16,18))))
generate double logrwage2=4+.002*(age-40)^2+.15*sin(persid)+.04*cos(betnr)+.12*sin(_n*1.31)
save "{data}", replace
display as result "SYNTHETIC_INPUT_COMPLETE"
'''
 run_stata(work/'prepare',body,'SYNTHETIC_INPUT_COMPLETE')
else:
 data=a.input;assert data and data.is_file()
 assert (root/'sample-authorization.json').exists(), 'data classification and sample specification must be recorded'
families=['ols','ppml'] if a.smoke else [a.family];jobs=[]
for family in families:
 for rep in range(1,(1 if a.smoke else 2)+1):
  for arm in (['native','reference'] if rep%2 else ['reference','native']):
   out=work/f'{family}-{arm}-{rep}';print('START',out.name,flush=True)
   try:run_stata(out,f'do "{root}/harness/fit.do" "{root}" "{data}" "{out}" {family} {arm} {threads}\n','GERMAN_FIVE_PERCENT_FIT_COMPLETE')
   except AssertionError:
    jobs.append(dict(family=family,arm=arm,rep=rep,directory=out.name,accepted=False));print('FAILED',out.name,flush=True);continue
   assert (out/'fit-result.txt').read_text().startswith('FIT_RC=0\n')
   assert (out/'outputs.dta').is_file()
   jobs.append(dict(family=family,arm=arm,rep=rep,directory=out.name,accepted=True));print('END',out.name,flush=True)
  if any(not x['accepted'] for x in jobs):break
(work/'runs.json').write_text(json.dumps(dict(job_id=os.environ['JOB_ID'],host=os.uname().nodename,slots=int(os.environ['NSLOTS']),threads=threads,native_memory_limit_bytes=a.native_memory_bytes,harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),fit_do_sha256=hashlib.sha256((root/'harness/fit.do').read_bytes()).hexdigest(),plugin_sha256=summary['plugin_sha256'],runs=jobs),indent=2)+'\n')
assert all(x['accepted'] for x in jobs),'a command failed; failed arm was not retried'
print('GERMAN_FIVE_PERCENT_COMMANDS_COMPLETE',flush=True)
