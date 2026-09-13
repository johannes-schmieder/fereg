import os
from pathlib import Path
import os,json,hashlib,subprocess
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'built-identities.json').read_text())
for arm in ['baseline','candidate']:assert all(sha(root/arm/p)==h for p,h in ids[arm+'_inputs'].items())
executables={'baseline':Path(os.path.join(os.environ['OMISSION_ROOT'], 'test-target/debug/deps/fereg_plugin-e9c3349472e39e19')),'candidate':root/'test-target/debug/deps/fereg_plugin-e9c3349472e39e19'}
assert all(p.exists() for p in executables.values())
protocol='Ten fixed alternating pairs of fresh-process exact PPML allocation-audit tests. Preserve all successes/failures. Same toolchain/environment; no builds or timings overlap. This diagnoses the unexpected one-allocation failure, not a retry-until-pass gate.'
(root/'allocation-audit-protocol.md').write_text(protocol+'\n')
work=root/'allocation-audit-rechecks';work.mkdir(exist_ok=False);rows=[]
env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')};env.update(RAYON_NUM_THREADS='4')
name='ppml_estimate::tests::allocation_audit_is_opt_in_and_publishes_real_phase_evidence'
for rep in range(10):
 for arm in (['baseline','candidate'] if rep%2==0 else ['candidate','baseline']):
  command=[str(executables[arm]),name,'--exact','--nocapture','--test-threads=1'];p=subprocess.run(command,cwd=root/arm,env=env,capture_output=True,timeout=90);log=work/f'{rep}-{arm}.log';log.write_bytes(p.stdout+p.stderr)
  row={'rep':rep,'arm':arm,'rc':p.returncode,'executable_sha256':sha(executables[arm]),'log_sha256':sha(log),'failure':log.read_text(errors='replace') if p.returncode else None};rows.append(row);(root/'allocation-audit-rechecks.json').write_text(json.dumps(rows,indent=2)+'\n');print(rep,arm,p.returncode,flush=True)
print('ALLOCATION_AUDIT_DIAGNOSTIC_TERMINAL',flush=True)
