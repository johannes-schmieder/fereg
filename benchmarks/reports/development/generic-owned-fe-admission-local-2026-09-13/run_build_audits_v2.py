import os
from pathlib import Path
import json,subprocess,hashlib
root=Path(__file__).resolve().parent;candidate=root/'candidate-v2';repo=Path(os.environ['REPOSITORY_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
expected='4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08'
assert sha(repo/'dist/fereg_macos.plugin')==expected
ids=json.loads((root/'prebuild-identities.json').read_text())
assert all(sha(candidate/p)==h for p,h in ids['candidate_inputs'].items())
audits=json.loads((root/'build-audit-inputs.json').read_text());assert all(sha(candidate/p)==h for p,h in audits.items())
log=(root/'build-isolated.log').read_text();assert log.count('Compiling fereg-core')==2 and log.count('Compiling fereg-plugin')==2
results=[]
with (root/'build-audits.log').open('wb') as out:
 for cmd in [['python3',str(candidate/'scripts/audit_binary.py'),str(candidate/'dist/fereg_macos.plugin')],['bash',str(candidate/'tests/mock_spi/run_actual_unix_plugin_test.sh'),str(candidate/'dist/fereg_macos.plugin')]]:
  p=subprocess.run(cmd,cwd=candidate,stdout=out,stderr=subprocess.STDOUT);results.append({'command':cmd,'rc':p.returncode});assert p.returncode==0
assert 'actual-plugin ABI/self-test harness: PASS' in (root/'build-audits.log').read_text()
ids.update(candidate_plugin=sha(candidate/'dist/fereg_macos.plugin'),build_audit_inputs=audits,build_audit_results=results,build_log_sha256=sha(root/'build-isolated.log'),build_audit_log_sha256=sha(root/'build-audits.log'),build_failure='Both architectures compiled; initial source-only snapshot lacked audit harness. Exact HEAD harness added; audit and ABI completed without numerical rebuild.')
assert sha(repo/'dist/fereg_macos.plugin')==expected
(root/'built-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
print('ISOLATED_BUILD_AND_ABI_PASS',ids['candidate_plugin'],flush=True)
