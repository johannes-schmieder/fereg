"""Profile accepted PPML without changing its import/work/science boundary."""
from pathlib import Path
import json,hashlib,os,sys,subprocess
root=Path(__file__).resolve().parent
sys.path.insert(0,str(root/'systematic'))
from common import read_json,write_json,sha256
from generate import generate
from runner import measure,compare
plan=read_json(root/'plan.json');source=Path('/projectnb/welfgr/fereg/dev/20260928-public-host-01/source');reference=Path('/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76')
assert os.environ['NSLOTS']=='28' and len(os.sched_getaffinity(0))==28
assert 'E5-2680 v4' in Path('/proc/cpuinfo').read_text()
for f,h in read_json(root/'deployment.json').items():assert sha256(root/f)==h,f
for f,h in read_json(root/'source-manifest.json')['files'].items():assert sha256(source/f)==h,f
assert sha256(source/'dist/fereg_linux.plugin')==plan['plugin_sha256']
manifest=read_json(reference/'input/manifest.json')
out=root/'diagnostic01';out.mkdir();scratch=Path(os.environ['TMPDIR'])/'ppml-thread-cost';scratch.mkdir()
variant=root/'variant';variant.mkdir()
for name,target in [('candidate',source),('env',reference/'env'),('references',reference/'references')]: (variant/name).symlink_to(target,target_is_directory=True)
inputs={};paths={}
for case in plan['cases']:
    cell=case['cell'];key=(cell['design'],cell['k'])
    if key not in inputs:
        folder=scratch/(cell['design']+'-k'+str(cell['k']));folder.mkdir();data=folder/'input.dta';warm=folder/'warm.dta'
        info=generate(cell,plan['seed'],data,manifest)
        generate(dict(cell,n=manifest['measurement']['warmup_rows'],section='main'),plan['seed'],warm,manifest)
        write_json(out/(folder.name+'-input.json'),info);inputs[key]=(data,warm)
    paths[case['id']]=inputs[key]
identity=sha256(root/'deployment.json');records=[];first={}
for rep in range(1,plan['repetitions']+1):
    cases=plan['cases'] if rep%2 else list(reversed(plan['cases']))
    for case in cases:
        cell=case['cell'];data,warm=paths[case['id']];dest=out/case['id']/('rep-'+str(rep));dest.parent.mkdir(exist_ok=True)
        result=measure(variant,cell,'feppml','warm',data,warm,dest,manifest,identity,limits=dict(startup=180,command=180,extraction=120))
        assert result['status']=='pass',(case['id'],result['status'])
        assert result['metadata']['feppml_threads']==cell['threads']
        previous=first.setdefault(case['id'],dest)
        science=compare(dest,previous,manifest['science']);assert science['status']=='pass',science
        # Native thread/memory alternatives share identical generated inputs.
        counterpart=next((first[c['id']] for c in plan['cases'] if c['id'] in first and paths[c['id']][0]==data),dest)
        cross=compare(dest,counterpart,manifest['science']);assert cross['status']=='pass',cross
        write_json(dest/'science.json',dict(repeated=science,cross_configuration=cross,reference=str(counterpart.relative_to(out))))
        for name,extra in [('self',['--no-children']),('inclusive',['--children'])]:
            command=['perf','report','--stdio','--percent-limit','0.2','-g','none','-s','dso,symbol','--field-separator','\t','-i',str(dest/'perf.data')]+extra
            result_perf=subprocess.run(command,capture_output=True,text=True,timeout=60);assert result_perf.returncode==0,result_perf.stderr
            (dest/('perf-'+name+'.txt')).write_text(result_perf.stdout)
        records.append(dict(case=case['id'],repetition=rep,status='pass',seconds=result['seconds'],diagnostic_only=True,science=science['status'],cross=cross['status']))
        write_json(out/'progress.json',records);print(case['id'],rep,'PASS',flush=True)
write_json(out/'complete.json',dict(complete=True,diagnostic_only=True,applications=len(records),source_snapshot=plan['source_snapshot'],plugin_sha256=plan['plugin_sha256'],all_science_passed=True))
print('FEREG_THREAD_COST_PROFILE_COMPLETE',flush=True)
