"""Collect the fixed pivot-cache protocol and check its complete receipts."""
import csv,json,os,re,shutil,statistics,zipfile
from pathlib import Path
from common import read_json,write_json,sha256
from collect_ppml_preparation import collect
from ppml_preparation_compare import named_runtime
r=Path(os.environ['FEREG_LOCAL_BENCH_ROOT']);repo=Path.cwd()
out=repo/'benchmarks/reports/development/ppml-pivot-cache-local-2026-09-13'
collect(r,out,prefix='pivot',science_file='science.json',primary_cases={'unit_65','fw_129','pw_257'})
d=read_json(out/'comparisons.json');assert len(d['pairs'])==19
science_names=['b','V','feppml_fit','feppml_status','feppml_dof','feppml_fe','feppml_cluster','feppml_subset','feppml_separation','feppml_inference','feppml_drop_counts','feppml_separation_sequence','feppml_certificate','feppml_receipt']
groups={};checks=[];audit={};review=[]
for rec in d['pairs']:
 paths={a:r/'runs'/f"{rec['pair']}-{a}" for a in ['baseline','candidate']}
 assert all((paths['baseline']/(s+'.csv')).read_bytes()==(paths['candidate']/(s+'.csv')).read_bytes() for s in science_names),rec['pair']
 runtimes={a:named_runtime(p) for a,p in paths.items()}
 for arm,rt in runtimes.items():
  rec[arm]['publication_seconds']=rt['time_publication'];rec[arm]['native_peak_delta_bytes']=rt['allocation_peak_delta']
  assert rt['allocation_peak_delta']<=rt['modeled_selected_bytes'],(rec['pair'],arm,'admission')
  assert rt['fallback_count']==rt['step_halvings']==rt['strict_profile_rejections']==0,(rec['pair'],arm,'retry')
  if rec['group']=='profile':
   audit[arm]={k:v for k,v in rt.items() if any(w in k for w in ['audit','allocation','capacity','fresh_warm'])}
   assert rt['audit_events_delta']==rt['audit_current_growth']==rt['audit_peak_growth']==0
   for k in ['ppml_capacity_stable','linear_capacity_stable','audit_scratch_stable','linear_session_allocation_free','fresh_warm_equivalent','allocation_certificate_passed','iteration_allocation_free']:assert rt[k]==1,(arm,k)
 groups.setdefault((rec['group'],rec['case']),[]).append(rec)
 checks.append(dict(pair=rec['pair'],scientific_csv_exact=True,admission_pass=True,native_peak_exact=runtimes['baseline']['allocation_peak_delta']==runtimes['candidate']['allocation_peak_delta'],modeled_phases_exact=all(runtimes['baseline'][k]==runtimes['candidate'][k] for k in runtimes['baseline'] if k.startswith('modeled_'))))
 if rec['group']=='control':
  delta=rec['candidate']['command_seconds']-rec['baseline']['command_seconds']
  review.append(dict(pair=rec['pair'],baseline_seconds=rec['baseline']['command_seconds'],candidate_seconds=rec['candidate']['command_seconds'],increase_seconds=delta,increase_percent=(rec['ratio']-1)*100,review_required=delta>max(.010,.05*rec['baseline']['command_seconds'])))
summary=[]
for (group,case),records in sorted(groups.items()):
 row=dict(group=group,case=case,pairs=len(records))
 for arm in ['baseline','candidate']:
  for key in records[0][arm]:
   vs=[x[arm][key] for x in records if x[arm][key] is not None]
   row[f'{arm}_{key}']=statistics.median(vs) if vs else None
 row['median_paired_change_percent']=100*(statistics.median(x['ratio'] for x in records)-1)
 summary.append(row)
d['summary']=summary;d['note']='19 paired comparisons against d9aa8c0: nine primary (three predeclared seeds), nine protected controls and one separate private allocation audit. No cold/oracle/standalone timing is pooled. Local development, no qualification or external-comparator claim.'
write_json(out/'comparisons.json',d)
with (out/'summary.csv').open('w') as f:
 w=csv.DictWriter(f,fieldnames=list(summary[0]));w.writeheader();w.writerows(summary)
a=read_json(out/'archive.json');a.update(primary_pairs=9,control_pairs=9,profile_pairs=1);write_json(out/'archive.json',a)
write_json(out/'science-review.json',checks);write_json(out/'allocation-audit.json',audit);write_json(out/'control-review.json',review)
for name in ['identities.json','protocol.md','diagnostic-protocol.md','matrix-identities.json','diagnostic-summary.json','standalone-summary.json','compare.py','run_oracles.py','oracle-results.json','profiled_oracle.py','profiled-oracle.json','implementation.patch']:
 shutil.copy2(r/name,out/name)
if Path(__file__).resolve() != (out/'collect_results.py').resolve():
 shutil.copy2(__file__,out/'collect_results.py')
tests=[];distinct={}
for crate,entries in [('core',read_json(r/'test-runs.json')),('plugin',[dict(filter='ppml_estimate::tests',rc=0,log='plugin-tests.log')])]:
 passed=set()
 for x in entries:
  s=(r/x['log']).read_text();names=sorted(set(re.findall(r'\btest (\S+) \.\.\. ok\b',s)));passed.update(names)
  assert x['rc']==0 and names and 'test result: FAILED' not in s
  tests.append(dict(crate=crate,filter=x['filter'],rc=x['rc'],passed=names,log_sha256=sha256(r/x['log'])))
 distinct[crate]=len(passed)
assert distinct=={'core':49,'plugin':22},distinct
build=(r/'build.log').read_text();assert 'actual-plugin ABI/self-test harness: PASS' in build and '"audit_passed": true' in build and build.count('Compiling fereg-core')==2 and build.count('Compiling fereg-plugin')==2
write_json(out/'validation.json',dict(baseline_commit=read_json(r/'identities.json')['baseline_commit'],distinct_tests=distinct,test_runs=tests,independent_oracles=read_json(r/'oracle-results.json'),profiled_oracle=read_json(r/'profiled-oracle.json'),build_log_sha256=sha256(r/'build.log'),universal_static_audit_pass=True,arm64_actual_abi_pass=True,x86_runtime_executed=False,notes=['One read-only identity checker initially requested candidate_inputs instead of the recorded inputs key; the corrected checker verifies all 176 inputs in both isolated trees and the candidate worktree. No source or fit was changed.', 'The first collector counted 21 plugin tests because child-process output placed one successful test mid-line. The terminal harness reports 22 passed; parsing test markers independent of line starts recovers all 22 distinct names. Original logs and failed collection log are retained; no fit or test was rerun.']))
with zipfile.ZipFile(out/'independent-evidence.zip','w',zipfile.ZIP_DEFLATED) as z:
 for name in ['profiled-oracle.npz','fw_129.bin','pw_257.bin']:
  z.write(r/name,name)
 for sub in ['diagnostic','standalone-candidate','standalone-baseline']:
  for p in sorted((r/sub).glob('*.rs')):z.write(p,f'{sub}/{p.name}')
 p=r/'runs/pivot-oracle-warm-candidate'
 for f in sorted(p.iterdir()):
  if f.suffix=='.csv' or f.name in ['result.json','input-experiment-identity.json','import-hashes.json','stata_worker.do']:z.write(f,f'oracle-warm/{f.name}')
write_json(out/'independent-archive.json',dict(sha256=sha256(out/'independent-evidence.zip'),scope='Synthetic identified covariance matrices, standalone Rust source, independent NumPy arrays and cold functional candidate numeric results; no licensed logs, binaries or datasets.'))
identity=read_json(r/'identities.json')
for n,h in identity['inputs'].items():assert sha256(repo/n)==h,n
assert sha256(repo/'dist/fereg_macos.plugin')==identity['installed_plugin_sha256']
for row in summary:print(row['group'],row['case'],row['baseline_command_seconds'],row['candidate_command_seconds'],row['median_paired_change_percent'])
print('controls requiring review',[x for x in review if x['review_required']])
print('nonexact peaks/models',[x for x in checks if not x['native_peak_exact'] or not x['modeled_phases_exact']])
