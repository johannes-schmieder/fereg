from pathlib import Path
import json,hashlib,re,statistics,subprocess,collections,shutil
r=Path('benchmarks/reports/development/ppml-thread-cost-scc-2026-09-28');raw=Path(__import__('sys').argv[1])
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,d):p.write_text(json.dumps(d,indent=2)+'\n')
sub=json.loads((r/'submission01.json').read_text());sub.update(job_id='7763785',run_root='/projectnb/welfgr/fereg/dev/20260928-thread-cost-02',deployment_sha256=sha(r/'deployment02.json'));write(r/'submission02.json',sub)
records=[];summary={}
for case in sorted(p for p in raw.iterdir() if p.is_dir()):
 aggregate=collections.defaultdict(float);total=0;samples=0;phases=collections.defaultdict(list)
 for rep in sorted(case.iterdir()):
  text=(rep/'perf-self.txt').read_text();sample=int(re.search(r'# Samples: ([0-9,]+)',text)[1].replace(',',''));assert sample>0 and '# Total Lost Samples: 0' in text
  cycles=int(re.search(r'Event count \(approx.\): (\d+)',text)[1]);total+=cycles;samples+=sample
  for line in text.splitlines():
   cols=[s.strip() for s in line.split('\t')]
   if len(cols)>=3 and cols[0].endswith('%'):aggregate[cols[1]+' '+cols[2]]+=float(cols[0][:-1])*cycles/100
  result=json.loads((rep/'result.json').read_text());science=json.loads((rep/'science.json').read_text());perf=json.loads((rep/'perf-receipt.json').read_text());assert result['status']=='pass' and science['repeated']['status']=='pass' and science['cross_configuration']['status']=='pass'
  assert perf['process_rc'] in [0,-2,130] and perf['requested_sigint']
  for line in (rep/'feppml_runtime.csv').read_text().splitlines()[1:]:
   row,col,val=line.split(',');
   if val.strip()!='.':phases[int(row)].append(float(val))
  records.append(dict(case=case.name,repetition=rep.name,application_status=result['status'],science=science,perf_receipt=perf,samples=sample,approximate_cycles=cycles,files={str(p.relative_to(rep)):sha(p) for p in rep.iterdir() if p.is_file()}))
 summary[case.name]=dict(samples=samples,approximate_cycles=total,top_self_percent=[dict(symbol=k,percent=100*v/total) for k,v in sorted(aggregate.items(),key=lambda kv:-kv[1])[:16]],diagnostic_runtime_row_medians={str(k):statistics.median(v) for k,v in phases.items()})
write(r/'applications02.json',records);write(r/'profile-summary02.json',summary)
for name in ['complete.json','progress.json']:shutil.copyfile(raw/name,r/('diagnostic02-'+name))
for attempt,job in [('01','7763777'),('02','7763785')]:
 root='/projectnb/welfgr/fereg/dev/20260928-thread-cost-'+attempt
 cmd='qacct -j '+job+'; cat '+root+'/job.log'
 out=subprocess.run(['ssh','scc',cmd],check=True,stdout=subprocess.PIPE,universal_newlines=True).stdout
 Path('/tmp/fereg-thread-cost-'+attempt+'-accounting.log').write_text(out)
 fields={key:re.search(r'^'+key+r'\s+(.*)$',out,re.M)[1].strip() for key in ['jobnumber','failed','exit_status','slots','hostname','ru_wallclock','maxvmem','start_time','end_time']}
 fields.update(application_sentinel='FEREG_THREAD_COST_SUCCESS' in out,raw_accounting_and_job_log_sha256=hashlib.sha256(out.encode()).hexdigest(),source_snapshot=sub['source_snapshot'])
 if attempt=='02':assert fields['failed']=='0' and fields['exit_status']=='0' and fields['application_sentinel']
 else:fields['failure']='The diagnostic controller rejected its intentional SIGINT stop of perf (rc=-2). Production source unchanged; no completed diagnostic application accepted.'
 write(r/('accounting'+attempt+'.json'),fields)
print(json.dumps({k:dict(samples=v['samples'],seconds=v['diagnostic_runtime_row_medians']['1'],top=v['top_self_percent'][:3]) for k,v in summary.items()},indent=2))
