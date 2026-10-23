#!/usr/bin/env python3
"""Fixed 80-call ownership qualification prerequisite; no selection or timing score.

Consume one fully frozen qualification manifest. Diagnostic calls never replace
qualification observations. Failures, including partial native output, remain in
place; there is no retry, resume, overwrite or automatic finalist selection.
"""
from __future__ import annotations

import argparse
import copy
import json
import hashlib
import os
from pathlib import Path
import platform
import re
import time

import ownership_cells as oc
import qualify as q
import qualify_ownership as own
import routing
import two_fe_policy_development as gates

KIND = 'ownership-qualification-diagnostics-v1'
MARKER = 'OWNERSHIP_QUALIFICATION_DIAGNOSTICS_COMPLETE 80'
require = q.require


def sources():
    return {**q.execution_sources(), 'ownership_cells': Path(oc.__file__).resolve(),
            'ownership_qualification_diagnostics': Path(__file__).resolve()}


def save_new(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('x') as stream:
        import json
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write('\n')


def schedule(manifest):
    return own.diagnostic_proposal(manifest)['schedule']


def verify_bound(manifest, path):
    """One full hash pass; later call boundaries use lightweight drift stamps."""
    stamps = q.verify_sources(manifest['execution_sources'])
    for name, expected in manifest['files'].items():
        source = q.resolve(path, name)
        before = q.stamp(source)
        require(q.digest(source) == expected and q.stamp(source) == before, 'qualification bound file changed: '+name)
        stamps[source] = before
    own.validate_bindings(manifest, path)
    return stamps


def freeze(qualification, output):
    qualification, output = Path(qualification).resolve(), Path(output).resolve()
    require(not output.exists(), 'diagnostic plan already exists')
    manifest = q.read(qualification)
    own.validate(manifest)
    stamps = verify_bound(manifest, qualification)
    plan = dict(schema=1, kind=KIND, platform=manifest['platform'],
                qualification=os.path.relpath(qualification, output.parent),
                qualification_sha256=q.digest(qualification), qualification_identity=routing.identity(manifest),
                candidate_sha256=manifest['files'][manifest['artifacts']['candidate']['path']],
                incumbent_sha256=manifest['files'][manifest['artifacts']['incumbent']['path']],
                request=manifest['artifacts']['candidate']['request'], schedule=schedule(manifest),
                native_calls=80, timeout_s=120, budget_s=2400, retries=0,
                observer='whole-host' if manifest['platform']=='Linux' else 'desktop',
                execution_sources={k:q.digest(p) for k,p in sources().items()})
    require(all(q.stamp(p)==v for p,v in stamps.items()), 'freeze input/source drift')
    save_new(output, plan)
    return plan


def load(plan_path):
    plan_path = Path(plan_path).resolve()
    plan = q.read(plan_path)
    qualification = q.resolve(plan_path, plan['qualification'])
    require(q.digest(qualification)==plan['qualification_sha256'], 'qualification manifest bytes changed')
    manifest = q.read(qualification)
    own.validate(manifest)
    require(plan['schema']==1 and plan['kind']==KIND and plan['platform']==manifest['platform']
            and plan['qualification_identity']==routing.identity(manifest)
            and plan['request']==manifest['artifacts']['candidate']['request']
            and plan['candidate_sha256']==manifest['files'][manifest['artifacts']['candidate']['path']]
            and plan['incumbent_sha256']==manifest['files'][manifest['artifacts']['incumbent']['path']]
            and plan['schedule']==schedule(manifest) and len(plan['schedule'])==plan['native_calls']==80
            and plan['timeout_s']==120 and plan['budget_s']==2400 and plan['retries']==0
            and plan['observer']==('whole-host' if manifest['platform']=='Linux' else 'desktop'),
            'diagnostic identity/schedule/budget differs')
    require(set(plan['execution_sources'])==set(sources()), 'diagnostic source coverage changed')
    stamps = verify_bound(manifest, qualification)
    for key, source in sources().items():
        before=q.stamp(source)
        require(q.digest(source)==plan['execution_sources'][key] and q.stamp(source)==before,
                'diagnostic source changed: '+key)
        stamps[source]=before
    stamps.update({plan_path:q.stamp(plan_path), qualification:q.stamp(qualification)})
    return plan, manifest, qualification, stamps


def arm_manifest(manifest):
    value=copy.deepcopy(manifest)
    value['artifacts']['uninjected_candidate']={**value['artifacts']['candidate'],'request':'auto'}
    return value


def mechanism(arm, request):
    return ('none','none') if arm!='candidate' else ('both','eager' if request=='cells_eager_both' else 'none')


def semantic(call):
    return call['route_trace'], call['route_hierarchy'], call['gate_events']


def expected_scores(spec, arm):
    """Independent frozen-shape prediction of the existing packed-cache guard.

    prepared_fit::fast_score_cache_fits excludes an absorbed intercept. The
    oracle requires full finite rank K, so finite bread keeps K columns while
    the generated model has K+include_intercept. one_shot ownership admission
    has no worker-count condition. Its pre-QR capacity guard charges the full
    projected N*(generated+1) matrix plus 16*generated metadata bytes, bounded
    by the identical plugin OlsFrameMemory reservation. With all rows retained,
    positive ownership therefore requires exactly the logical matrix capacity.
    Above 32MiB, strided-means fallback reports no avoided packed copy, even if
    its internal representation retains a Selected matrix.
    """
    n, k = spec['rows'], spec['regressors']
    generated = k + int(spec['include_intercept'])
    fits = n*k*8 <= 32*1024*1024
    admitted = arm == 'candidate' and fits
    return dict(score_admitted=int(admitted), avoided_packed_elements=n*k if admitted else 0,
                avoided_packed_bytes=n*k*8 if admitted else 0,
                retained_matrix_capacity_bytes=n*(generated+1)*8 if admitted else 0,
                generated_columns=generated, kept_columns=k)


def mechanism_evidence(family, workers, calls, request):
    require([c['arm'] for c in calls]==['incumbent','uninjected_candidate','uninjected_candidate','candidate','candidate']
            and all(c['status']=='passed' and c['oracle']=='passed' for c in calls), 'diagnostic cell incomplete')
    spec=own.added_cases.specification(family)
    n, width=spec['rows'],spec['regressors']+int(spec['include_intercept'])+1
    expected=dict(constructed_moment_rows=n,constructed_moment_columns=width,
                  moment_workspace_bound_bytes=width*3*(min(64,max(1,(n+16383)//16384))+1)*8)
    eligible=workers==4 and family not in own.added_cases.BOUNDARY
    for arm in ('uninjected_candidate','candidate'):
        a,b=[c for c in calls if c['arm']==arm]
        require(a['fingerprint']==b['fingerprint'] and a['worker_signature']==b['worker_signature']
                and semantic(a)==semantic(b)
                and a['ownership_diagnostic']==b['ownership_diagnostic']
                and a['cell_diagnostic']==b['cell_diagnostic']
                and a['ownership_transport']['normalized_action_resources']==b['ownership_transport']['normalized_action_resources'],
                'fixed-worker science/decisions/resource work changed')
        for call in (a,b):
            require(bool(call['action_resources']) and bool(call['solver_trace_lines']), 'missing routing/resource evidence')
            if arm=='candidate' and request=='cells_eager_both':
                require(call['cell_diagnostic']['bound'], 'requested eligible eager-cell binding trace missing')
            events=call['ownership_diagnostic']['events']
            scores=[event for event in events if 'score_admitted' in event]
            require(scores==[expected_scores(spec,arm)],
                    'actual score admission/avoided work/full retained capacity/finite width differs')
            built=[e for e in events if 'constructed_moment_rows' in e]
            omitted=sum(e.get('unused_moment_bytes',0) for e in events)
            if eligible:
                require((built==[expected] and omitted==0) if arm=='uninjected_candidate'
                        else (not built and omitted==expected['moment_workspace_bound_bytes']>0),
                        'positive actual baseline construction/finalist omission missing')
            elif workers==4:
                # Cache-boundary controls cannot omit raw moments: the K+2
                # bound exceeds 32MiB, but unit-weight parallel construction
                # remains admitted in both uninjected and ownership branches.
                require(built==[expected] and omitted==0, 'boundary raw moment construction was not preserved')
            else:
                require(not built and omitted==0, 'W1 moment construction/omission control changed')
    return dict(family=family,workers=workers,moment_omission_eligible=eligible,
                baseline_construction_required=workers==4,expected_moments=expected,
                request=request,positive_omission_demonstrated=eligible,
                packed_cache_bytes=spec['rows']*spec['regressors']*8,
                score_cache_ownership_eligible=spec['rows']*spec['regressors']*8<=32*1024*1024,
                expected_finalist_scores=expected_scores(spec,'candidate'))


def compare_cell(manifest, meta, snapshots):
    incumbent, baseline, finalist=(snapshots[a] for a in ('incumbent','uninjected_candidate','candidate'))
    require(incumbent[2]['fingerprint']==baseline[2]['fingerprint']
            and semantic(incumbent[2])==semantic(baseline[2]), 'uninjected candidate changed accepted c16 controls')
    if manifest['artifacts']['candidate']['request']=='ownership_both':
        require(finalist[2]['fingerprint']==incumbent[2]['fingerprint']
                and semantic(finalist[2])==semantic(incumbent[2]), 'ownership exact science/decisions changed')
    else:
        q.science.compatibility(incumbent[0],finalist[0],incumbent[1],finalist[1],meta,'auto')
        a,b=own.installed_hierarchies(incumbent[2]),own.installed_hierarchies(finalist[2])
        if a and b:
            require(a==b,'original hierarchy changed when both built')


def run(plan_path, output, *, execute=None, monitor_factory=None):
    started=time.monotonic()
    output=Path(output).resolve()
    require(output.is_relative_to((q.ROOT/'target/tmp').resolve()) and not output.exists(), 'new task output required')
    output.mkdir(parents=True)
    result=dict(schema=1,kind=KIND,status='running',calls=[],cells={},failures=[],native_calls_started=0,
                retries=0,exclusions=[],elapsed_s=0,performance_claim=False,activation_qualified=False)
    host=None
    def persist():
        result['elapsed_s']=time.monotonic()-started
        q.write(output/'results.json',result)
    persist()
    execute=q.run_process if execute is None else execute
    try:
        with q.Deadline(2400,started) as deadline:
            plan,manifest,qualification,stamps=load(plan_path)
            require(platform.system()==plan['platform'],'wrong diagnostic platform')
            result.update(plan_sha256=q.digest(plan_path),qualification_sha256=plan['qualification_sha256'],
                          candidate_sha256=plan['candidate_sha256'],incumbent_sha256=plan['incumbent_sha256'],
                          schedule=plan['schedule'],platform=plan['platform'])
            persist()  # The complete finite schedule exists before any native call.
            from qualify_scc import HostMonitor, MacMonitor
            factory=monitor_factory or (HostMonitor if plan['platform']=='Linux' else MacMonitor)
            host=factory(output/'host',4)
            diagnostic=arm_manifest(manifest)
            families={c['family']:c for c in manifest['cases']}
            first_worker={}
            with deadline.observing(host):
                for offset in range(0,80,5):
                    entries=plan['schedule'][offset:offset+5]
                    entry=entries[0]; family,workers,cell=entry['family'],entry['workers'],entry['cell']
                    item=families[family];meta=q.metadata(manifest,item,qualification)
                    source=q.resolve(qualification,item['input'])
                    with source.open('rb') as stream: _,source_args=q.case_header(stream)
                    folder=output/cell;folder.mkdir()
                    snapshots={};cell_calls=[];requests={}
                    for arm in ('incumbent','uninjected_candidate','candidate'):
                        request=folder/(arm+'.case')
                        args=own.prepare(source,request,diagnostic['artifacts'][arm]['request'],workers)
                        requests[arm]=(request,args);stamps[request]=q.stamp(request)
                    for index,entry in enumerate(entries,offset):
                        deadline.check();host.check()
                        require(all(q.stamp(p)==v for p,v in stamps.items()),'source/input/request drift')
                        arm=entry['arm'];request,args=requests[arm]
                        target=folder/f'call-{index:02d}.json';log=target.with_suffix('.log')
                        call={**entry,'status':'started','target':str(target.relative_to(output)),
                              'log':str(log.relative_to(output)),'request_sha256':q.digest(request)}
                        result['calls'].append(call);result['native_calls_started']+=1;persist()
                        env=['env',*[v for key in q.ENVIRONMENT for v in ('-u',key)],
                             *[key+'=1' for key in q.THREAD_ENV], 'FEREG_SOLVER_TRACE=1','FEREG_PHASE_TIMING=1']
                        artifact=diagnostic['artifacts'][arm]
                        call['started_monotonic_ns']=time.monotonic_ns()
                        resources=execute([*env,str(q.resolve(qualification,manifest['harness'])),
                                           str(q.resolve(qualification,artifact['path'])),'--replay',str(request),str(target)],
                                          log,min(120,deadline.check()))
                        call['finished_monotonic_ns']=time.monotonic_ns()
                        # Retain hashes even if subsequent parsing/science fails.
                        call.update(result_sha256=q.digest(target),log_sha256=q.digest(log));persist()
                        raw,columns,detail=q.read_native(target,log,meta)
                        require(detail['seconds']<=120 and detail['solver_rows'][0][0]==q.cmgschur.EXPECTED_SOLVER['auto'],
                                'native call limit/Auto identity changed')
                        text=log.read_text();mode=mechanism(arm,plan['request'])
                        detail.update(own.resource_detail(diagnostic,arm,meta,text,args,source_args,workers))
                        detail.update(worker_signature=q.worker_signature(raw),raw_result=raw,
                                      columns_sha256=q.digest(columns),maxrss_bytes=resources['maxrss_bytes'])
                        if arm!='incumbent':
                            detail.update(ownership_diagnostic=oc.ownership_diagnostic(text,mode),
                                          cell_diagnostic=oc.cell_diagnostic(text,mode))
                        call.update(detail,status='passed',oracle='passed');cell_calls.append(call)
                        if arm not in snapshots:
                            snapshots[arm]=(raw,columns,call)
                            base=folder/('reference-'+arm)
                            call['reference']={'path':str(base.relative_to(output)),**q.retained_snapshot(base,raw,columns,meta)}
                            if (family,arm) in first_worker:
                                previous,earlier=first_worker[family,arm]
                                q.compare_saved(previous,base,meta)
                                require(earlier['worker_signature']==call['worker_signature']
                                        and gates.semantic_signature(earlier)==gates.semantic_signature(call),
                                        'cross-worker science/discrete routing changed')
                            else:
                                first_worker[family,arm]=(base,call)
                        else:
                            require(call['fingerprint']==snapshots[arm][2]['fingerprint'],'fixed-worker full-column repeat changed')
                            columns.unlink()
                        deadline.check();persist()
                    compare_cell(manifest,meta,snapshots)
                    result['cells'][cell]=mechanism_evidence(family,workers,cell_calls,plan['request'])
                    for _,columns,_ in snapshots.values(): columns.unlink()
                    persist()
            result['host']=host.report()
            require(result['host']['isolation_ok'] and not result['host']['errors'],'invalid diagnostic host observation')
            result['host_observations_sha256']=q.digest(output/'host/host-observations.jsonl')
            require(result['native_calls_started']==80 and len(result['cells'])==16,'incomplete fixed schedule')
            require(all(q.stamp(p)==v for p,v in stamps.items()),'final source/input/request drift')
            deadline.check();result['status']='complete';persist();deadline.check()
    except BaseException as error:
        if result['calls'] and result['calls'][-1]['status']=='started':
            failed=result['calls'][-1]
            failed['status']='failed'
            for key in ('target','log'):
                path=output/failed[key]
                if path.is_file():failed[('result' if key=='target' else key)+'_sha256']=q.digest(path)
        result['status']='failed';result['failures'].append(dict(type=type(error).__name__,message=str(error)))
        if host is not None:result['host']=host.report()
        persist()
        raise
    return result


def verify_host(result, directory, platform_name):
    """Recheck recorded observation coverage and SCC reservation/census assertions."""
    from qualify_scc import validate_reservation, validate_census
    host=result['host'];path=directory/'host/host-observations.jsonl'
    require(host['isolation_ok'] and not host['errors']
            and q.digest(path)==result['host_observations_sha256'],'diagnostic observer failed or changed')
    events=[json.loads(line) for line in path.read_text().splitlines()]
    require(events and not any(e['kind']=='error' for e in events), 'observer errors or empty record')
    process=[e for e in events if e['kind']=='process']
    require(len(process)>=2 and all(not e['competing_compute'] for e in process)
            and all(e['monotonic_ns']<=e['finished_monotonic_ns'] for e in process)
            and all(a['finished_monotonic_ns']<=b['monotonic_ns'] for a,b in zip(process,process[1:])),
            'invalid/competing process observations')
    calls=result['calls']
    require(process[0]['finished_monotonic_ns']<=calls[0]['started_monotonic_ns']
            and process[-1]['monotonic_ns']>=calls[-1]['finished_monotonic_ns'], 'native observation endpoints missing')
    for call in calls:
        require(host['started_monotonic_ns']<=call['started_monotonic_ns']<call['finished_monotonic_ns']
                <=host['measurement_end_monotonic_ns'], 'native call outside observed host span')
    if platform_name=='Linux':
        reservations=[e for e in events if e['kind']=='reservation']
        censuses=[e for e in events if e['kind']=='census']
        require(len(reservations)>=2 and len(censuses)>=2,'missing SCC reservation/census endpoints')
        for event in reservations:validate_reservation(event['raw'],host['host'],4)
        for event in censuses:validate_census(event['raw']['job'],event['raw']['queues'],host['host'])
    return host


def prove(plan_path, result_path, output, *, accounting=None, job_log=None):
    """Wrapper prerequisite: immutable bindings, retained evidence and terminal host evidence."""
    output=Path(output).resolve();require(not output.exists(),'proof already exists')
    plan,manifest,qualification,stamps=load(plan_path)
    result_path=Path(result_path).resolve();directory=result_path.parent
    stamps[result_path]=q.stamp(result_path);result=q.read(result_path)
    require(result['platform']==plan['platform'] and result['status']=='complete' and result['failures']==[] and result['retries']==0 and result['exclusions']==[]
            and result['native_calls_started']==len(result['calls'])==80 and len(result['cells'])==16
            and 0<result['elapsed_s']<=2400 and result['plan_sha256']==q.digest(plan_path)
            and result['qualification_sha256']==plan['qualification_sha256']
            and result['candidate_sha256']==plan['candidate_sha256'] and result['incumbent_sha256']==plan['incumbent_sha256']
            and result['schedule']==plan['schedule']
            and [{k:c[k] for k in ('cell','family','workers','arm','repeat')} for c in result['calls']]==plan['schedule'],
            'incomplete/stale/failed diagnostic result')
    stamps[directory/'host/host-observations.jsonl']=q.stamp(directory/'host/host-observations.jsonl')
    host=verify_host(result,directory,plan['platform'])
    accounts={}
    if plan['platform']=='Linux':
        require(accounting is not None and job_log is not None,'Linux proof requires terminal accounting and completion marker')
        account=Path(accounting);log=Path(job_log)
        stamps.update({p:q.stamp(p) for p in (account,log)})
        q.accounting(account.read_text(),host['host'],normalize=True)
        fields=dict(line.split(None,1) for line in account.read_text().splitlines() if len(line.split(None,1))==2)
        require(fields.get('slots')==str(host['host']['slots'])
                and fields.get('granted_pe')==host['host']['pe'],'accounted reservation differs')
        require(log.read_text().splitlines().count(MARKER)==1,'Linux completion marker missing/duplicated')
        accounts=dict(accounting_sha256=q.digest(account),job_log_sha256=q.digest(log))
    diagnostic=arm_manifest(manifest);families={c['family']:c for c in manifest['cases']}
    first_worker={};requests_checked={}
    for offset in range(0,80,5):
        calls=result['calls'][offset:offset+5];family=calls[0]['family'];workers=calls[0]['workers']
        require(result['cells'][calls[0]['cell']]==mechanism_evidence(family,workers,calls,plan['request']),
                'mechanism evidence changed')
        item=families[family];meta=q.routing_cases.inspect_case(q.resolve(qualification,item['input']),family)
        with q.resolve(qualification,item['input']).open('rb') as stream:_,source_args=q.case_header(stream)
        for call in calls:
            target,log=(directory/call[k] for k in ('target','log'))
            require(all(p.resolve().is_relative_to(directory) for p in (target,log)),'call evidence outside output')
            stamps.update({p:q.stamp(p) for p in (target,log)})
            require(q.digest(target)==call['result_sha256'] and q.digest(log)==call['log_sha256'],'raw diagnostic evidence changed')
            raw=q.read(target);require(raw.pop('columns')==[] and raw==call['raw_result'],'raw diagnostic head differs')
            q.validate_result(raw)
            timers=re.findall(r'^NATIVE_SECONDS=([0-9.]+)$',log.read_text(),re.M)
            require(len(timers)==1 and 0<float(timers[0])==call['seconds']<=120
                    and call['timing_field']=='NATIVE_SECONDS','complete-call diagnostic timer differs')
            require(call['fingerprint']==routing.identity({**q.exact_view(raw,False),'columns_sha256':call['columns_sha256']})
                    and call['worker_signature']==q.worker_signature(raw),'raw diagnostic signature differs')
            argv=own.arguments(source_args,diagnostic['artifacts'][call['arm']]['request'],workers)
            request=directory/call['cell']/(call['arm']+'.case')
            if request not in requests_checked:
                require(request.resolve().is_relative_to(directory),'request outside diagnostic directory')
                stamps[request]=q.stamp(request)
                with q.resolve(qualification,item['input']).open('rb') as source, request.open('rb') as prepared:
                    shape,_=q.case_header(source);actual_shape,actual_args=q.case_header(prepared)
                    shape[2]=len(argv)
                    require(shape==actual_shape and argv==actual_args,'retained request/model framing differs')
                    hashes=[]
                    for stream in (source,prepared):
                        body=hashlib.sha256()
                        for block in iter(lambda:stream.read(1<<20),b''):body.update(block)
                        hashes.append(body.digest())
                    require(hashes[0]==hashes[1],'retained request numeric/model body differs')
                requests_checked[request]=q.digest(request)
            require(requests_checked[request]==call['request_sha256'],'retained request hash differs')
            detail=own.resource_detail(diagnostic,call['arm'],meta,log.read_text(),argv,source_args,workers)
            require(all(call[k]==v for k,v in detail.items()),'raw diagnostic resource/route differs')
            if call['arm']!='incumbent':
                mode=mechanism(call['arm'],plan['request'])
                require(call['ownership_diagnostic']==oc.ownership_diagnostic(log.read_text(),mode)
                        and call['cell_diagnostic']==oc.cell_diagnostic(log.read_text(),mode),'raw mechanism trace differs')
            if 'reference' in call:
                ref=call['reference'];base=directory/ref['path']
                stamps.update({base.with_suffix(ext):q.stamp(base.with_suffix(ext)) for ext in ('.json','.npz')})
                require(base.resolve().is_relative_to(directory) and q.digest(base.with_suffix('.json'))==ref['head']
                        and q.digest(base.with_suffix('.npz'))==ref['outputs']
                        and q.read(base.with_suffix('.json'))==raw,'retained diagnostic reference changed')
        first={a:next(c for c in calls if c['arm']==a) for a in ('incumbent','uninjected_candidate','candidate')}
        require(sum('reference' in c for c in calls)==3 and all('reference' in c for c in first.values()),
                'diagnostic first-call reference coverage changed')
        inc,base,final=(first[a] for a in ('incumbent','uninjected_candidate','candidate'))
        require(inc['fingerprint']==base['fingerprint'] and semantic(inc)==semantic(base),
                'retained uninjected controls differ')
        if plan['request']=='ownership_both':
            require(inc['fingerprint']==final['fingerprint'] and semantic(inc)==semantic(final),
                    'retained ownership exact science/decisions differ')
        else:
            q.compare_saved(directory/inc['reference']['path'],directory/final['reference']['path'],meta)
            from replay_plugins import differences, allowed_difference
            scope=q.science.replay_scope(final['raw_result'],'auto',meta)
            require(not [p for p in differences(q.exact_view(inc['raw_result'],False),q.exact_view(final['raw_result'],False))
                         if not allowed_difference(p,scope)]
                    and q.science.model_discrete_sha256(inc['raw_result'])==q.science.model_discrete_sha256(final['raw_result']),
                    'retained cell science scope/model changed')
            a,b=own.installed_hierarchies(inc),own.installed_hierarchies(final)
            if a and b:require(a==b,'retained original hierarchies differ')
        for arm,call in first.items():
            path=directory/call['reference']['path']
            if (family,arm) in first_worker:
                previous,earlier=first_worker[family,arm]
                q.compare_saved(previous,path,meta)
                require(earlier['worker_signature']==call['worker_signature']
                        and gates.semantic_signature(earlier)==gates.semantic_signature(call),'retained cross-worker decisions differ')
            else:first_worker[family,arm]=(path,call)
    require(all(q.stamp(p)==v for p,v in stamps.items()),'proof source/input drift')
    proof=dict(schema=1,kind=KIND,status='passed',qualification_sha256=plan['qualification_sha256'],
               qualification_identity=plan['qualification_identity'],plan_sha256=q.digest(plan_path),
               candidate_sha256=plan['candidate_sha256'],incumbent_sha256=plan['incumbent_sha256'],request=plan['request'],
               result_sha256=q.digest(result_path),native_calls=80,cells=16,failures=[],retries=0,
               host_observations_sha256=result['host_observations_sha256'],**accounts,
               execution_sources=plan['execution_sources'],mechanism_evidence=result['cells'],
               performance_claim=False,activation_qualified=False,
               limits=['Diagnostic correctness/mechanism prerequisite only; all fixed qualification timing and hidden/public activation gates remain.'])
    save_new(output,proof);return proof


def main():
    parser=argparse.ArgumentParser(description=__doc__);sub=parser.add_subparsers(dest='command',required=True)
    f=sub.add_parser('freeze');f.add_argument('--qualification',type=Path,required=True);f.add_argument('--output',type=Path,required=True)
    r=sub.add_parser('run');r.add_argument('--plan',type=Path,required=True);r.add_argument('--output',type=Path,required=True)
    p=sub.add_parser('prove');p.add_argument('--plan',type=Path,required=True);p.add_argument('--results',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True);p.add_argument('--accounting',type=Path);p.add_argument('--job-log',type=Path)
    args=parser.parse_args()
    if args.command=='freeze':freeze(args.qualification,args.output)
    elif args.command=='run':run(args.plan,args.output);print(MARKER)
    else:prove(args.plan,args.results,args.output,accounting=args.accounting,job_log=args.job_log)


if __name__=='__main__':main()
