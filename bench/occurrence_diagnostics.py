#!/usr/bin/env python3
"""Bounded scheduler-association diagnostic; never supplies performance scores."""
from __future__ import annotations
import argparse
import os
from pathlib import Path
import random
import time
import compact_cells as c

CONTRACT = {
 'kind':'occurrence_scheduler_diagnostic_v1', 'parent_revision':'2dfc0e86',
 'arms':['baseline','cells_m'], 'repetitions':{'1':12,'4':24}, 'seed':2026101721,
 'slow_native_seconds':.0105, 'wait_threshold_ns':2_000_000,
 'association_fraction':.8, 'minimum_slow_calls':3,
 'classification':'four-worker slow calls; association if at least three and >=80% have >=2ms summed runqueue wait or an involuntary switch during absorption',
 'consequence':'association selects five physical CPUs for all D2/C2 calls; otherwise retain four; no warm interval or candidate retuning',
 'caveat':'association is not causal attribution; instrumentation perturbs all reported timings; frequency/run-time differences alone are inconclusive',
 'budget_s':600, 'timeout_s':120, 'exclusions':'none', 'extensions':'none',
 'warmups':0, 'controls':'one uninstrumented same-allocation call per arm/worker; every diagnostic signature must match it',
}


def parse_probe(text):
    records=[]
    for line in text.splitlines():
        if line.startswith('FEREG_SCHED|'):
            fields={}
            for part in line.split('|')[1:]:
                k,sep,v=part.partition('=')
                c.require(sep and k not in fields,'malformed scheduler probe')
                fields[k]=v
            records.append(fields)
    c.require(records,'missing scheduler probe')
    intervals=[]
    for scope,id_ in sorted({(r['scope'],r['id']) for r in records}):
        selected=[r for r in records if (r['scope'],r['id'])==(scope,id_)]
        edges={}
        for edge in ['start','end']:
            rows=[r for r in selected if r['edge']==edge]
            boundary=[r for r in rows if r['record']=='boundary']
            overhead=[r for r in rows if r['record']=='overhead']
            c.require(len(boundary)==len(overhead)==1,'missing probe boundary/overhead')
            tasks={int(r['tid']):r for r in rows if r['record']=='task'}
            c.require(tasks and len(tasks)==sum(r['record']=='task' for r in rows),'missing/duplicate task')
            edges[edge]={'boundary':boundary[0],'tasks':tasks,'read_ns':int(overhead[0]['read_ns']),
                         'frequency':[r for r in rows if r['record']=='frequency']}
        a,b=edges['start'],edges['end']
        common=a['tasks'].keys() & b['tasks'].keys()
        if scope=='absorb': c.require(a['tasks'].keys()==b['tasks'].keys(),'absorption task set changed; diagnostic incomplete')
        deltas=[]
        for tid in sorted(common):
            row={'tid':tid,'cpu_start':int(a['tasks'][tid]['cpu']),'cpu_end':int(b['tasks'][tid]['cpu'])}
            for key in ['run_ns','wait_ns','slices','voluntary','involuntary']:
                row[key]=int(b['tasks'][tid][key])-int(a['tasks'][tid][key])
                c.require(row[key]>=0,'scheduler counter went backwards')
            deltas.append(row)
        intervals.append({'scope':scope,'id':int(id_),'start':a['boundary'],'end':b['boundary'],
                          'task_deltas':deltas,'tasks_added':sorted(b['tasks'].keys()-a['tasks'].keys()),
                          'tasks_removed':sorted(a['tasks'].keys()-b['tasks'].keys()),
                          'read_ns':a['read_ns']+b['read_ns'],
                          'frequency_start':a['frequency'],'frequency_end':b['frequency']})
    c.require(sum(r['scope']=='native' for r in intervals)==1 and any(r['scope']=='absorb' for r in intervals),
              'incomplete native/absorption diagnostic')
    absorptions=[r for r in intervals if r['scope']=='absorb']
    return {'intervals':intervals,'raw_records':records,
            **{key:sum(t[key] for r in absorptions for t in r['task_deltas'])
               for key in ['run_ns','wait_ns','voluntary','involuntary']}}


def classify(rows):
    slow=[r for r in rows if r['threads']==4 and r['seconds']>CONTRACT['slow_native_seconds']]
    linked=[r for r in slow if r['probe']['wait_ns']>=CONTRACT['wait_threshold_ns'] or r['probe']['involuntary']>0]
    associated=len(slow)>=CONTRACT['minimum_slow_calls'] and len(linked)/len(slow)>=CONTRACT['association_fraction']
    return {'slow_calls':len(slow),'associated_slow_calls':len(linked),
            'association':associated,'affinity_cores':5 if associated else 4,
            'claim':CONTRACT['caveat']}


def expected_order(cells, smoke=False):
    rng=random.Random(CONTRACT['seed']);schedule=[]
    for cell in cells:
        count=1 if smoke else CONTRACT['repetitions'][str(cell['threads'])]
        initial=rng.sample(CONTRACT['arms'],2)
        for rep in range(count):
            for arm in (initial if rep%2==0 else list(reversed(initial))):
                schedule.append((cell,arm,rep))
    return schedule


def validate_admission(admission):
    from datetime import datetime, timezone
    c.require(admission['kind']==CONTRACT['kind']+'_admission' and admission['status']=='passed',
              'scheduler diagnostic admission failed')
    receipt=c.routing.unseal(c.read(c.checked(admission['diagnostic'])))
    c.require(receipt['status']=='passed' and receipt['smoke'] is False and receipt['contract']==CONTRACT,
              'complete scheduler diagnostic required')
    files={item['path']:item for item in receipt['files']}
    c.require(len(files)==len(receipt['files']), 'duplicate diagnostic file')
    for item in files.values():c.checked(item)
    c.require(files.get(receipt['plan']['path'])==receipt['plan'], 'unbound diagnostic plan')
    plan=c.routing.unseal(c.read(c.checked(receipt['plan'])))
    c.require(c.routing.identity(plan)==receipt['plan_sha256'] and plan['contract']==CONTRACT
              and plan['smoke'] is False, 'diagnostic plan binding differs')
    for key in ['source','probe_source','harness','diagnostic_plugin']:c.checked(plan[key])
    for path,sha in plan['tools'].items():c.checked({'path':path,'sha256':sha})
    provenance=c.read(c.checked(plan['provenance']))
    c.require(provenance['parent_revision']==CONTRACT['parent_revision']
              and provenance['plugin']==plan['diagnostic_plugin'], 'diagnostic build identity differs')
    for key in ['manifest','build_script','build_log','preparation_source']:c.checked(provenance[key])
    manifest=c.read(provenance['manifest']['path'])
    for path,sha in manifest.items():c.checked({'path':str(Path(provenance['source_root'])/path),'sha256':sha})
    c.require(manifest['crates/fereg-core/src/scheduler_probe.rs']==plan['probe_source']['sha256'],
              'diagnostic instrumentation differs')
    parent=c.routing.unseal(c.read(c.checked(plan['parent_plan'])))
    c.require(parent['kind']==c.MEMBERSHIP_KIND and parent['harness']==plan['harness'], 'wrong diagnostic parent')
    c.checked(parent['plugin'])
    cells=[cell for cell in parent['cells'] if cell['family']=='easy']
    c.require([cell['threads'] for cell in cells]==[1,4], 'diagnostic easy coverage differs')
    controls=receipt['controls']; records=receipt['records']
    c.require([(r['cell'],r['arm']) for r in controls]==[(x['id'],a) for x in cells for a in CONTRACT['arms']],
              'diagnostic controls incomplete')
    c.require([(r['cell'],r['arm'],r['repeat']) for r in records]==[(x['id'],a,r) for x,a,r in expected_order(cells)],
              'diagnostic schedule differs')
    fingerprints={}; previous=receipt['started_monotonic_ns']; seen=set()
    directory=Path(receipt['plan']['path']).parent
    for row in [*controls,*records]:
        cell=next(x for x in cells if x['id']==row['cell']);c.checked(cell['metadata']);c.checked(cell['inputs'][row['arm']])
        traced='repeat' in row
        label=(f"diagnostic-{cell['id']}-{row['arm']}-{row['repeat']}" if traced
               else f"control-{cell['id']}-{row['arm']}")
        target=directory/(label+'.json'); log=target.with_suffix('.log')
        c.require(row['label']==label and row['target']==str(target) and row['log']==str(log)
                  and label not in seen, 'diagnostic call path/label reused or changed')
        seen.add(label)
        command=c.routing_run.native_command(Path(plan['harness']['path']),
                    Path((plan['diagnostic_plugin'] if traced else parent['plugin'])['path']),
                    Path(cell['inputs'][row['arm']]['path']),target,cell,traced)
        if traced:command.insert(-5,'FEREG_PHASE_TIMING=1')
        command=['taskset','-c',','.join(map(str,plan['native_affinity'])),*command]
        c.require(row['command']==command, 'diagnostic artifact/request/affinity command differs')
        a,b=row['started_monotonic_ns'],row['completed_monotonic_ns']
        c.require(type(a) is int and type(b) is int and previous<=a<b<=receipt['completed_monotonic_ns']
                  and type(row['started_wall_ns']) is int and row['started_wall_ns']>0,
                  'diagnostic call chronology overlaps or exceeds observation')
        previous=b
        for path in [row['target'],row['log'],str(Path(row['target']).with_suffix('.json.columns'))]:
            c.require(path in files, 'unbound diagnostic raw output')
        c.require(row['status']=='passed','diagnostic native failure')
        _,result,_=c.parse_run({**row,'status':'executed'},c.read(cell['metadata']['path']))
        c.require(result['science_sha256']==row['science_sha256'] and result['seconds']==row['seconds'],
                  'diagnostic parsed result differs')
        key=(row['cell'],row['arm'])
        c.require(fingerprints.setdefault(key,result['science_sha256'])==result['science_sha256'],
                  'diagnostic changed parent science/signature')
        if 'repeat' in row:
            c.require(row['threads']==cell['threads'] and parse_probe(Path(row['log']).read_text())==row['probe'],
                      'diagnostic raw probe differs')
    classification=classify(records)
    c.require(classification==receipt['classification'], 'diagnostic classification changed')
    values=c.scc.fields(c.scc.output(c.read(c.checked(admission['accounting'])),['qacct','-j',receipt['job_id']]))
    c.require(values['jobnumber']==receipt['job_id'] and values['project']=='welfgr'
              and values['slots']=='28' and values['granted_pe']=='omp28'
              and c.scc.hostname(values['hostname'])==c.scc.hostname(receipt['host']['hostname'])
              and int(values['failed'].split()[0])==0 and int(values['exit_status'])==0,
              'scheduler diagnostic accounting failed')
    stamp=lambda value:datetime.strptime(value,'%a %b %d %H:%M:%S %Y').replace(tzinfo=timezone.utc).timestamp()*1e9
    c.require(stamp(values['start_time'])<=min(r['started_wall_ns'] for r in [*controls,*records])
              and stamp(values['end_time'])+1e9>=max(r['started_wall_ns']+r['completed_monotonic_ns']-r['started_monotonic_ns']
                                                   for r in [*controls,*records]), 'diagnostic accounting window differs')
    review=c.routing.unseal(c.read(c.checked(admission['isolation_review'])))
    c.require(review['status']=='passed' and review['diagnostic']==admission['diagnostic']
              and review['accounting']==admission['accounting'] and review['observation_complete'] is True
              and review['observed_compute_overlap']==[] and review['observed_host_faults']==[]
              and review['exclusions']==[] and bool(review['reviewer']), 'diagnostic isolation review incomplete')
    required=['affinity','job_environment','parallel_environment','processes','queue_configuration','resource_quota','scheduler']
    c.require(set(review['raw_evidence'])==set(required), 'missing diagnostic isolation evidence')
    for name,item in review['raw_evidence'].items():
        c.require(item==files.get(item['path']) and Path(item['path']).name==name+'.jsonl',
                  'diagnostic isolation evidence binding differs')
    directory=Path(receipt['plan']['path']).parent
    raw={name:directory/(name+'.jsonl') for name in required}
    c.scc.reservation(raw,receipt['host'],receipt)
    observer=c.Observers(directory,receipt['host'],receipt['queue']);observer.pid=receipt['collection_pid']
    observer.validate(receipt['started_monotonic_ns'],receipt['completed_monotonic_ns'])
    return classification


def run(parent_path, plugin, output, smoke=False, provenance=None):
    parent=c.routing.unseal(c.read(parent_path))
    c.require(parent['kind']==c.MEMBERSHIP_KIND,'diagnostic requires frozen membership parent')
    for key in ['plugin','harness']:c.checked(parent[key])
    binding=c.ref(plugin); output.mkdir(parents=True,exist_ok=False)
    cells=[cell for cell in parent['cells'] if cell['family']=='easy' and (not smoke or cell['threads']==1)]
    for cell in cells:
        for arm in CONTRACT['arms']:c.checked(cell['inputs'][arm])
    c.require(os.environ.get('JOB_ID') and int(os.environ.get('NSLOTS','0'))>=(2 if smoke else 28),'compute allocation required')
    host=c.scc.discovery.host(); before=queue=observer=None
    records=[];controls=[];failure=None;start=time.monotonic();begin=time.monotonic_ns()
    cpus=sorted(host['affinity'])[:4] if not smoke else sorted(host['affinity'])[:1]
    os.environ['FEREG_SCHED_CPUS']=','.join(map(str,cpus))
    freeze={'contract':CONTRACT,'parent_plan':c.ref(parent_path),'diagnostic_plugin':binding,
            'provenance':c.ref(provenance) if provenance else None,
            'harness':parent['harness'],'source':c.ref(Path(__file__)),
            'probe_source':c.ref(Path(__file__).with_name('scheduler_probe.rs')),
            'tools':{str(path):c.digest(path) for path in c.tool_files()},
            'created_wall_ns':time.time_ns(),'smoke':smoke,'native_affinity':cpus}
    c.save(output/'plan.json',c.routing.seal(freeze))
    try:
        if not smoke:
            before,snapshot=c.scc.configuration(output,host,'before')
            queue=c.scc.check_configuration(before,host,os.getpid())
            c.scc.check_scheduler_snapshot(snapshot,host,queue)
            observer=c.Observers(output,host,queue);observer.start()
            observer.census("diagnostic-before");begin=time.monotonic_ns()
        fingerprints={}
        for cell in cells:
            for arm in CONTRACT['arms']:
                row=c.native(parent,cell,arm,output,f'control-{cell["id"]}-{arm}',cpus=cpus)
                controls.append(row);_,result,_=c.parse_run(row,c.read(cell['metadata']['path']))
                row.update(result,status='passed');fingerprints[cell['id'],arm]=result['science_sha256']
        schedule=expected_order(cells,smoke)
        for cell,arm,rep in schedule:
            remaining=CONTRACT['budget_s']-(time.monotonic()-start)
            c.require(remaining>0 and not(observer and observer.errors),'diagnostic budget/observer failed')
            row=c.native({**parent,'plugin':binding},cell,arm,output,f'diagnostic-{cell["id"]}-{arm}-{rep}',
                         traced=True,phase_timing=True,cpus=cpus,timeout=min(remaining,CONTRACT['timeout_s']))
            row.update(threads=cell['threads'],repeat=rep);records.append(row)
            _,result,_=c.parse_run(row,c.read(cell['metadata']['path']))
            c.require(result['science_sha256']==fingerprints[cell['id'],arm],'diagnostic changed parent science/signature')
            row.update(result,probe=parse_probe(Path(row['log']).read_text()),status='passed')
        c.require(len(records)==(2 if smoke else 72),'incomplete diagnostic panel')
        c.require(time.monotonic()-start<=CONTRACT['budget_s'],'diagnostic budget exhausted')
    except Exception as error:failure=f'{type(error).__name__}: {error}'
    finally:
        end=time.monotonic_ns()
        if observer:
            try:
                observer.census("diagnostic-after");observer.close()
                after,snapshot=c.scc.configuration(output,host,'after')
                c.require(c.scc.check_configuration(after,host,os.getpid())==queue,'allocation changed')
                c.scc.check_scheduler_snapshot(snapshot,host,queue)
                c.check_stable_configuration(before,after)
                observer.validate(begin,end)
            except Exception as error:failure=failure or f'{type(error).__name__}: {error}'
    receipt={'kind':CONTRACT['kind'],'status':'failed' if failure else 'passed','failure':failure,'smoke':smoke,
             'job_id':os.environ['JOB_ID'],'host':host,'collection_pid':os.getpid(),'queue':queue,
             'plan':c.ref(output/'plan.json'),'contract':CONTRACT,'plan_sha256':c.routing.identity(freeze),
             'records':records,'controls':controls,'started_monotonic_ns':begin,'completed_monotonic_ns':end,
             'classification':None if failure or smoke else classify(records),
             'files':[c.ref(p) for p in sorted(output.iterdir()) if p.is_file()]}
    c.save(output/'diagnostic.json',c.routing.seal(receipt))
    c.require(not failure,failure)
    print('OCCURRENCE_SCHEDULER_DIAGNOSTIC_PASS',flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--parent-plan',type=Path,required=True)
    parser.add_argument('--plugin',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--smoke',action='store_true')
    parser.add_argument('--provenance',type=Path,required=True)
    a=parser.parse_args();run(a.parent_plan,a.plugin,a.output,a.smoke,a.provenance)
