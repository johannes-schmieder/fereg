"""Separate post-timing CPU attribution; never a headline measurement."""
from pathlib import Path
import argparse,hashlib,json,os,signal,subprocess,sys,time

p=argparse.ArgumentParser()
for n in ('reference','sources-root'):p.add_argument('--'+n,type=Path,required=True)
a=p.parse_args()
root=Path(__file__).resolve().parent
sys.path.insert(0,str(root/'systematic'))
from generate import generate
from common import thread_environment

def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,d):p.write_text(json.dumps(d,indent=2)+'\n')

plan=json.loads((root/'profile-registration.json').read_text())
reference=a.reference
manifest=json.loads((reference/'input/manifest.json').read_text())
out=root/'diagnostic01';out.mkdir(exist_ok=False)
cpus=sorted(os.sched_getaffinity(0));assert len(cpus)>=4 and os.environ['NSLOTS']=='4'
records=[]
for case in plan['cases']:
    folder=out/case['id'];folder.mkdir()
    source=a.sources_root/case['source']/'source'
    assert digest(source/'dist/fereg_linux.plugin')==case['plugin_sha256']
    data=folder/'input.dta';info=generate(case['cell'],case['seed'],data,manifest)
    assert not case.get('input_sha256') or info['sha256']==case['input_sha256']
    command=case['command'];threads=case['cell']['threads']
    script=folder/'fit.do'
    script.write_text(f'''version 19.0
clear all
set more off
set varabbrev off
set processors `=min({threads},c(processors_lic),c(processors_mach))'
sysdir set PLUS "{folder}"
sysdir set PERSONAL "{folder}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{source}/stata"
adopath ++ "{source}/dist"
use "{data}", clear
forvalues repetition=1/{case['repetitions']} {{
    quietly {command}
    assert e(converged)==1
}}
tempname meta
file open `meta' using "{folder}/metadata.tsv", write text
local scalars : e(scalars)
foreach name of local scalars {{
    file write `meta' "`name'" _tab %24.17g (e(`name')) _n
}}
file close `meta'
display "FEREG_CPU_PROFILE_FITS_SUCCESS"
exit 0
''')
    env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
    env.update(thread_environment(threads))
    env.update(FEREG_SOLVER_TRACE='1',FEREG_SOLVER_TRACE_FILE=str(folder/'solver.trace'))
    argv=['perf','record','-F',str(plan['frequency_hz']),'-e','cycles:u','--call-graph',plan['callgraph'],'-o',str(folder/'perf.data'),'--','taskset','-c',','.join(map(str,cpus[:threads])),'stata-mp','-b','do',str(script)]
    start=time.monotonic();timeout=False
    with (folder/'stdout.txt').open('w') as stdout,(folder/'stderr.txt').open('w') as stderr:
        child=subprocess.Popen(argv,cwd=folder,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
        try:child.wait(timeout=180)
        except subprocess.TimeoutExpired:
            timeout=True;os.killpg(child.pid,signal.SIGKILL);child.wait()
    log=folder/'fit.log';success=child.returncode==0 and not timeout and log.exists() and any(l.strip()=='FEREG_CPU_PROFILE_FITS_SUCCESS' for l in log.read_text(errors='replace').splitlines())
    result=dict(case=case,diagnostic_only=True,process_rc=child.returncode,timed_out=timeout,sentinel=success,elapsed_seconds=time.monotonic()-start,input_sha256=info['sha256'],script_sha256=digest(script))
    if success:
        p=subprocess.run(['perf','report','--stdio','--no-children','--percent-limit','0.5','-g','graph,0.5,caller','-s','dso,symbol','-i',str(folder/'perf.data')],capture_output=True,text=True,timeout=60)
        result.update(report_rc=p.returncode,report=p.stdout,report_error=p.stderr)
        success=success and p.returncode==0
    result['status']='success' if success else 'failure'
    result['bindings']={p.name:digest(p) for p in folder.iterdir() if p.is_file()}
    write(folder/'receipt.json',result);records.append(result);write(out/'results.json',records)
    print(case['id'],result['status'],flush=True)
    if not success:raise RuntimeError('CPU attribution failed; preserve without repeating fits')
    data.unlink()
write(out/'complete.json',dict(complete=True,cases=len(records),diagnostic_only=True))
print('FEREG_CPU_PROFILE_COMPLETE',flush=True)
