import os
from pathlib import Path
import json,hashlib,copy
import measure as measure_module
from measure import measure
original_thread_environment=measure_module.thread_environment
def trace_environment(threads):return {**original_thread_environment(threads),'FEREG_SOLVER_TRACE':'1'}
measure_module.thread_environment=trace_environment
from run import MANIFEST
from ppml_preparation_compare import compare,named_runtime
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());source=r/'candidate';assert all(sha(source/p)==h for p,h in ids['candidate_inputs'].items());assert sha(source/'dist/fereg_macos.plugin')==ids['candidate_plugin']
requests=json.loads((r/'requests.json').read_text())+[q for q in json.loads((r/'followup-requests.json').read_text()) if q['name'] in ['weak_d3_wide','weighted_cut','bottleneck_ring_wide','cycle_wide','disconnected_blocks_wide','unbalanced_wide']];assert len(requests)==9
rows=[]
for req in requests:
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 out=r/'trace-runs'/req['name'];manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
 result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,manifest,source=source,solver='auto',trace=True,identity={'artifact':ids['candidate_plugin'],'protocol':sha(r/'trace-protocol.md'),'purpose':'Separate checkpoint Schur attempt tracing, excluded from speed aggregates'})
 row={'name':req['name'],'result':result};rows.append(row);(r/'trace-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(req['name'],result['status'],flush=True);assert result['status']=='pass'
 original=r/('followup-runs' if 'kind' in req else 'screen-runs')/(req['name']+'-candidate')
 compare({'baseline':original,'candidate':out});row['exact_science']=json.loads((out/'exact-science.json').read_text());row['runtime']=named_runtime(out)
 row['lines']=[line for line in (out/'stderr.txt').read_text().splitlines() if line.startswith(('FEREG_PHASE|schur|','FEREG_SOLVER_TRACE|phase=checkpoint_schur_attempt|','FEREG_SOLVER_TRACE|phase=checkpoint_lsmr_attempt|'))];assert any('checkpoint_schur_attempt' in x for x in row['lines'])
 (r/'trace-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('CHECKPOINT_SCHUR_TRACES_TERMINAL_PASS',flush=True)
