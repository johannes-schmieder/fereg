from pathlib import Path
import json,hashlib,subprocess,re,sys
remote=len(sys.argv)>1 and sys.argv[1]=='linux'
r=Path('/projectnb/welfgr/fereg/dev/20260928-private-low-01') if remote else Path.cwd()
out=r/'output' if remote else r/'target/implicit-private-20260928/evidence'
m=json.loads((r/'staged/candidate-source.json' if remote else r/'benchmarks/reports/development/ppml-private-low-scc-2026-09-28/candidate-source.json').read_text());source=r/'source' if remote else r
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert all(sha(source/f)==h for f,h in m['files'].items())
logs=list(out.glob('*tests.log'))+[out/('profile-tests.log' if remote else 'one_fe_profile.log')]
# Local test suite names contain Rust paths; remote names are compact.
if not remote:logs=list(out.glob('*.log'));logs=[p for p in logs if '::' in p.name or p.name in ['one_fe_profile.log','implicit_centered.log']]
passed=set();counts={}
for p in logs:
 t=p.read_text();tests=re.findall(r'^test (\S+) \.\.\. ok$',t,re.M);assert 'test result: FAILED' not in t and re.search(r'test result: ok\.',t);passed.update(tests);counts[p.name]=len(tests)
assert len(passed)==101,(len(passed),counts)
d=json.loads((out/'oracles.json').read_text());assert d['status']=='success' and d['tests_passed']==6 and d['tests_failed']==0
receipt=dict(status='success',snapshot=m['snapshot'],plugin_sha256=sha(source/('dist/fereg_linux.plugin' if remote else 'dist/fereg_macos.plugin')),rust_distinct_passed=len(passed),rust_test_names=sorted(passed),suite_counts=counts,tests_passed=6,tests_failed=0,fixtures=d['results'],evidence_sha256={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file()})
if remote:
 a=subprocess.check_output(['qacct','-j','7763918'],universal_newlines=True);v={l.split(None,1)[0]:l.split(None,1)[1].strip() for l in a.splitlines() if len(l.split())>=2};assert v['failed']==v['exit_status']=='0'
 smoke=json.loads((out/'summary.json').read_text());assert smoke['status']=='success' and smoke['tests_passed']==1 and smoke['tests_failed']==0
 assert 'FEREG_PRIVATE_LOW_VALIDATION_SUCCESS job=7763918' in (r/'job.log').read_text()
 receipt.update(job_id='7763918',run_root=str(r),smoke=smoke['results'],job_log_sha256=sha(r/'job.log'),accounting={k:v[k] for k in ['jobnumber','hostname','slots','failed','exit_status','ru_wallclock','cpu','maxvmem','start_time','end_time']})
else:
 assert 'FEREG_PRIVATE_LOW_MAC_SUCCESS' in Path('/tmp/fereg-private-low-local.log').read_text()
 assert 'actual-plugin ABI/self-test harness: PASS' in (out/'build.log').read_text()
print(json.dumps(receipt,indent=2))
