from pathlib import Path
import json,csv,re,statistics,hashlib,shutil,subprocess
from collect_ppml_preparation import collect
from common import read_json,write_json,sha256
from ppml_preparation_compare import named_runtime
r=Path(__file__).parent;repo=Path.cwd();out=repo/'benchmarks/reports/development/ppml-wide-moments-local-2026-09-13'
collect(r,out,prefix='ppmlonefe-wide-moments',science_file='science.json',primary_cases={'unit_65','fw_129','pw_257'})
d=read_json(out/'comparisons.json');assert len(d['pairs'])==17
primary={'unit_65','fw_129','pw_257'};groups={};audit={};checks=[]
science_names=['b','V','feppml_fit','feppml_status','feppml_dof','feppml_fe','feppml_cluster','feppml_subset','feppml_separation','feppml_inference','feppml_drop_counts','feppml_separation_sequence','feppml_certificate','feppml_receipt']
for rec in d['pairs']:
 rec['group']='profile' if '-trace-' in rec['pair'] else 'primary' if rec['case'] in primary else 'control'
 paths={a:r/'runs'/f"{rec['pair']}-{a}" for a in ['baseline','candidate']}
 unequal=[name for name in science_names if (paths['baseline']/(name+'.csv')).read_bytes()!=(paths['candidate']/(name+'.csv')).read_bytes()]
 assert not unequal,(rec['pair'],unequal)
 for arm,p in paths.items():
  rt=named_runtime(p)
  rec[arm]['publication_seconds']=rt['time_publication'];rec[arm]['native_peak_delta_bytes']=rt['allocation_peak_delta']
  assert rt['allocation_peak_delta']<=rt['modeled_selected_bytes'],(p.name,'admission')
  assert rt['fallback_count']==rt['step_halvings']==rt['strict_profile_rejections']==0
  if rec['group']=='profile':
   audit[arm]={k:v for k,v in rt.items() if any(word in k for word in ['audit','allocation','capacity','fresh_warm'])}
   assert rt['audit_events_delta']==rt['audit_current_growth']==rt['audit_peak_growth']==0
   for key in ['ppml_capacity_stable','linear_capacity_stable','audit_scratch_stable','linear_session_allocation_free','fresh_warm_equivalent','allocation_certificate_passed','iteration_allocation_free']:assert rt[key]==1,(arm,key)
 groups.setdefault((rec['group'],rec['case']),[]).append(rec)
 checks.append(dict(pair=rec['pair'],all_scientific_csv_exact=True,admission_pass=True))
summary=[]
for (group,case),records in sorted(groups.items()):
 row=dict(group=group,case=case,pairs=len(records))
 for arm in ['baseline','candidate']:
  for key in records[0][arm]:
   vs=[x[arm][key] for x in records if x[arm][key] is not None]
   row[f'{arm}_{key}']=statistics.median(vs) if vs else None
 row['median_paired_change_percent']=100*(statistics.median(x['ratio'] for x in records)-1)
 summary.append(row)
d['summary']=summary;d['note']='17 new pairs against repaired accepted 9ec47b8: nine primary pairs (three predeclared seeds each), seven controls and one separately timed private audit. The legacy recheck prefix means planned repeat seeds here, not replacement observations. Earlier two-pair probe is not pooled. Full scientific CSVs are exact; no qualification or comparator claim.'
write_json(out/'comparisons.json',d)
with (out/'summary.csv').open('w') as f:
 w=csv.DictWriter(f,fieldnames=list(summary[0]));w.writeheader();w.writerows(summary)
a=read_json(out/'archive.json');a.update(primary_pairs=9,control_pairs=7,profile_pairs=1);write_json(out/'archive.json',a)
write_json(out/'science-review.json',checks);write_json(out/'allocation-audit.json',audit)
for name in ['identities.json','protocol.md','resume-protocol.md','logical-work.json','control-review.json','compare.py','run_oracles.py','oracle-results.json','implementation.patch']:
 shutil.copy2(r/name,out/name)
tests=[];passed=set()
for x in read_json(r/'test-runs.json'):
 s=(r/x['log']).read_text();names=re.findall(r'^test (\S+) \.\.\. ok$',s,re.M);passed.update(names)
 tests.append(dict(filter=x['filter'],rc=x['rc'],passed=names,log_sha256=sha256(r/x['log'])))
assert len(passed)==62
write_json(out/'validation.json',dict(baseline_commit='9ec47b88c9c78773fa4c8b7cba955ba397ba9ab3',candidate_main_before_commit='374d257',distinct_tests=len(passed),test_runs=tests,independent_oracles=read_json(r/'oracle-results.json'),
 build_log_sha256=sha256(r/'build.log'),universal_static_audit_pass=True,arm64_actual_abi_pass=True,x86_runtime_executed=False,
 scope='Local development: no installed dist replacement, agents, SCC, Windows, qualification or push.',
 notes=['Initial --lib filter selection exercised 61 distinct tests; the one matching numeric-sample integration oracle was then run explicitly after timing, completing all 62 planned checks. No failed numerical test or fit was retried.','The supplemental PSD control-label correction is separate in evidence-only 374d257; numerical baseline remains 9ec47b8.']))
identity=read_json(r/'identities.json')
for n,h in identity['inputs'].items():assert sha256(repo/n)==h,n
assert sha256(repo/'dist/fereg_macos.plugin')==identity['installed_plugin_sha256']
for row in summary:print(row['group'],row['case'],row['baseline_command_seconds'],row['candidate_command_seconds'],row['median_paired_change_percent'])
