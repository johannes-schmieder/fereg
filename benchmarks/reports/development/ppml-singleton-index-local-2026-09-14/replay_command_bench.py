from pathlib import Path
import subprocess,json,time,statistics,hashlib,sys
root=Path(sys.argv[1]).resolve()
stata=sys.argv[2]
results=[]
fields=['N_physical','num_singletons','separated','rank','df_a','df_resid','deviance','ic','inner_iterations','step_halvings','feppml_fallback_count','feppml_solver_fallback_count','feppml_time_preparation','feppml_time_separation_fe','feppml_allocation_peak_delta','feppml_memory_selected_bytes','feppml_scratch_bytes']
cases=[('chain',8192,'low'),('chain',16384,'low'),('chain',32768,'low'),('half',32768,'low'),('balanced',131072,'fast'),('balanced',131072,'low')]
for shape,n,mode in cases:
 for rep in range(3):
  for method in (['baseline','candidate'] if rep%2==0 else ['candidate','baseline']):
   folder=root/'command-bench'/f'{shape}-{n}-{mode}-{rep}-{method}';folder.mkdir(parents=True)
   script=f'''clear all
set more off
set linesize 240
set seed 19811231
adopath ++ "{root}/{method}/stata"
adopath ++ "{root}/{method}/dist"
set obs {n+4096}
generate long row=_n
generate long g=1+mod(_n-1,16)
generate long h=1+mod(floor((_n-1)/16),16)
generate double x=rnormal()
generate double y=1+mod(_n,7)
generate byte expected=1
'''
   if shape=='chain':
    script+='replace g=1000+floor((_n-4097)/2) if row>4096\nreplace h=1000+floor((_n-4096)/2) if row>4096\nreplace expected=0 if row>4096\n'
   elif shape=='half':
    script+=f'replace g=0 if row>4096\nreplace h=0 if row>4096\nreplace g=1000+row if inrange(row,4097,{4096+n//2})\nreplace expected=0 if inrange(row,4097,{4096+n//2})\n'
   script+=f'''timer clear
timer on 1
capture noisily feppml y x, absorb(g h) memory({mode}) threads(4) separation(fe) timeit
timer off 1
local rc=_rc
file open out using "{folder}/result.txt", write replace
file write out "rc=" (_rc) _n
if `rc'==0 {{
    assert e(sample)==expected
    assert e(converged)==1
    assert row==_n
    quietly timer list 1
    file write out "command_seconds=" %21.14e (r(t1)) _n
    file write out "b_x=" %21.14e (_b[x]) _n
    file write out "v_x=" %21.14e (e(V)[1,1]) _n
'''
   for field in fields:script+=f'    file write out "{field}=" %21.14e (e({field})) _n\n'
   script+='    matrix list e(feppml_solver)\n    matrix list e(feppml_runtime)\n    matrix list e(feppml_certificate)\n    display as result "PPML_COMMAND_BENCH_PASS"\n}\nfile close out\nexit `rc\'\n'
   # Capture rc immediately after estimation, before timer commands.
   script=script.replace('timer off 1\nlocal rc=_rc','local rc=_rc\ntimer off 1').replace('"rc=" (_rc)','"rc=" (`rc\')')
   (folder/'run.do').write_text(script)
   started=time.monotonic()
   subprocess.run([stata,'-b','do','run.do'],cwd=folder,check=True)
   elapsed=time.monotonic()-started
   log=(folder/'run.log').read_text(errors='replace')
   assert 'PPML_COMMAND_BENCH_PASS\n' in log,folder
   record={}
   for line in (folder/'result.txt').read_text().splitlines():
    k,v=line.split('=',1);v=v.strip();record[k]=None if v=='.' else float(v)
   assert record['rc']==0,folder
   record.update(shape=shape,n=n+4096,mode=mode,method=method,rep=rep,process_seconds=elapsed)
   results.append(record)
   with (root/'command-bench.jsonl').open('a') as f:f.write(json.dumps(record)+'\n')
   print(json.dumps({'case':folder.name,'seconds':record['command_seconds'],'N':record['N_physical']}),flush=True)
for shape,n,mode in cases:
 for rep in range(3):
  pair=[r for r in results if (r['shape'],r['n'],r['mode'],r['rep'])==(shape,n+4096,mode,rep)]
  for field in ['b_x','v_x',*fields[:12]]:assert pair[0][field]==pair[1][field],(shape,n,mode,rep,field,pair)
(root/'command-bench-comparison.json').write_text(json.dumps({'runs':len(results),'paired_result_fields_exact':True},indent=2)+'\n')
