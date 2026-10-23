"""Exact 30+50 continuation of the no-intercept moment expectation failure.

No new model, native retry or general resume protocol. The two failed observer
cohorts stay immutable; all imported columns and mathematics are rechecked.
"""
from __future__ import annotations

import copy
import hashlib
import math
import os
from pathlib import Path
import shutil
import tempfile

import numpy as np
import qualify as q
import routing
import ownership_diagnostic_transport as transport

require=q.require
KIND='ownership-diagnostic-no-intercept-eligibility-v1'
PREVIOUS_SOURCES={
    'ownership_qualification_diagnostics':'84ecb0ab795e9373caf8823a7285792f77dd413a2025a78182c2b48149950fbf',
    'ownership_diagnostic_transport':'e4bba493fdd6f4bfe539bee32e578918f77095ac779f13eac6bb7931d66b369f',
}
ERROR='positive actual baseline construction/finalist omission missing'


def cohort_files(directory, result):
    """Bind every retained first-call bank and all 30 raw outputs/requests."""
    directory=Path(directory).resolve();entries={}
    for index,call in enumerate(result['calls']):
        for name in ('target','log'):
            entries[f'call-{index}-{name}']=directory/call[name]
        entries[f'call-{index}-request']=directory/call['cell']/(call['arm']+'.case')
        if 'reference' in call:
            for ext in ('.json','.npz'):
                entries[f'call-{index}-reference{ext}']=(directory/call['reference']['path']).with_suffix(ext)
    require(all(p.resolve().is_relative_to(directory) for p in entries.values()),'prior call path escapes its cohort')
    return entries


def freeze(qualification, previous_plan, previous_results, attempt_seconds, output, *, accounting=None, job_log=None):
    import ownership_qualification_diagnostics as d
    output=Path(output).resolve();old_path=Path(previous_results).resolve();old=q.read(old_path)
    prior=q.read(previous_plan)
    entries=dict(previous_plan=previous_plan,previous_results=previous_results,
                 previous_qualification=q.resolve(previous_plan,prior['qualification']),
                 observations=old_path.parent/'host/host-observations.jsonl',
                 **cohort_files(old_path.parent,old))
    if accounting is not None:entries['accounting']=accounting
    if job_log is not None:entries['job_log']=job_log
    amendment=dict(kind=KIND,attempt_seconds=attempt_seconds,charged_seconds=math.ceil(attempt_seconds),
                   reused_calls=30,new_calls=50,total_calls=80,
                   files={k:dict(path=os.path.relpath(Path(v).resolve(),output.parent),sha256=q.digest(Path(v)))
                          for k,v in entries.items()})
    original=copy.deepcopy(prior['transport_repair'])
    for binding in original['files'].values():
        binding['path']=os.path.relpath(q.resolve(previous_plan,binding['path']),output.parent)
    return d.freeze(qualification,output,repair=original,eligibility=amendment)


def signature(repair):
    return {**repair,'files':{k:v['sha256'] for k,v in repair['files'].items()}}


def validate(plan,manifest,plan_path):
    import ownership_qualification_diagnostics as d
    from qualify_scc import fields,short_host
    amendment=plan['eligibility_amendment']
    require(set(amendment)=={'kind','attempt_seconds','charged_seconds','reused_calls','new_calls','total_calls','files'}
            and amendment['kind']==KIND and (amendment['reused_calls'],amendment['new_calls'],amendment['total_calls'])==(30,50,80),
            'eligibility continuation contract differs')
    paths,stamps={},{}
    for key,binding in amendment['files'].items():
        require(set(binding)=={'path','sha256'},'invalid eligibility evidence binding')
        path=q.resolve(plan_path,binding['path']);before=q.stamp(path)
        require(q.digest(path)==binding['sha256'] and q.stamp(path)==before,'eligibility evidence drift: '+key)
        paths[key]=path;stamps[path]=before
    old,prior=q.read(paths['previous_results']),q.read(paths['previous_plan'])
    before=q.read(paths['previous_qualification'])
    expected={'previous_results','previous_plan','previous_qualification','observations',
              *cohort_files(paths['previous_results'].parent,old)}
    if plan['platform']=='Linux':expected|={'accounting','job_log'}
    require(set(paths)==expected,'eligibility evidence coverage differs')
    for key,path in cohort_files(paths['previous_results'].parent,old).items():
        require(paths[key]==path.resolve(),'raw cohort evidence path differs')
    require('eligibility_amendment' not in prior and prior['kind']==d.KIND and prior['schema']==1
            and prior['qualification_sha256']==q.digest(paths['previous_qualification'])
            and prior['qualification_identity']==routing.identity(before)
            and signature(prior['transport_repair'])==signature(plan['transport_repair'])
            and transport.normalized_manifest(before)==transport.normalized_manifest(manifest)
            and before['files'][before['harness']]==manifest['files'][manifest['harness']],
            'not the exact preceding corrected-harness plan')
    for key in ('platform','schedule','candidate_sha256','incumbent_sha256','request','native_calls','timeout_s','budget_s','retries','observer'):
        require(prior[key]==plan[key],'prior diagnostic protocol differs: '+key)
    old_sources=dict(prior['execution_sources']);new_sources=dict(plan['execution_sources'])
    for key,sha in PREVIOUS_SOURCES.items():
        require(old_sources.pop(key)==sha,'not the diagnosed eligibility source')
        new_sources.pop(key)
    new_sources.pop('ownership_diagnostic_eligibility')
    require(old_sources==new_sources,'unrelated execution source changed')
    require(old['status']=='failed' and old['native_calls_started']==len(old['calls'])==30
            and old['native_calls_reused']==1 and old['native_calls_executed_here']==29
            and old['failures']==[dict(type='ValueError',message=ERROR)]
            and old['retries']==0 and old['exclusions']==[]
            and old['plan_sha256']==q.digest(paths['previous_plan'])
            and old['qualification_sha256']==prior['qualification_sha256']
            and old['transport_repair']==prior['transport_repair']
            and old['schedule']==plan['schedule'] and old['platform']==plan['platform']
            and old['candidate_sha256']==plan['candidate_sha256'] and old['incumbent_sha256']==plan['incumbent_sha256']
            and all(c['status']=='passed' and c['oracle']=='passed' for c in old['calls'])
            and [{k:c[k] for k in ('cell','family','workers','arm','repeat')} for c in old['calls']]==plan['schedule'][:30]
            and old['calls'][-1]['cell']=='ownership-inactive-zero-t4'
            and set(old['cells'])=={c['cell'] for c in old['calls'][:25]}, 'not the diagnosed six-cell prefix')
    duration=amendment['attempt_seconds'];charge=amendment['charged_seconds']
    original_charge=plan['transport_repair']['charged_seconds']
    require(type(duration) in (int,float) and math.isfinite(duration)
            and old['charged_prior_seconds']==original_charge
            and old['elapsed_s']==original_charge+old['continuation_elapsed_s']
            and 0<old['continuation_elapsed_s']<=duration<2400
            and charge==math.ceil(duration)
            and (original_charge,charge)==((2,6) if plan['platform']=='Darwin' else (20,37)),
            'wrong preserved application charge')
    require(paths['observations']==paths['previous_results'].parent/'host/host-observations.jsonl','prior observer location differs')
    observed={**old,'calls':old['calls'][1:],'host_observations_sha256':q.digest(paths['observations'])}
    d.verify_host(observed,paths['previous_results'].parent,plan['platform'])
    _,original,_=transport.validate(plan,manifest,plan_path);transport.same_host(old['host'],original)
    if plan['platform']=='Linux':
        account=fields(paths['accounting'].read_text());host=old['host']['host']
        require(account.get('project')=='welfgr' and account.get('end_time') and account['failed']=='0'
                and account['exit_status']=='1' and account['jobnumber']==host['job_id']
                and account['slots']=='28' and account['granted_pe']==host['pe']
                and account['taskid'] in ('0','undefined') and str(host['task_id']) in ('0','undefined')
                and short_host(account['hostname'])==short_host(host['hostname'])
                and account['ru_wallclock']=='46' and d.MARKER not in paths['job_log'].read_text(),
                'failed continuation terminal accounting differs')
    return paths,old,stamps


def reconstruct(source,base,meta,destination,expected):
    """Independent full-column reconstruction from frozen input plus retained outputs."""
    with source.open('rb') as stream:
        shape,_=q.case_header(stream);rows,width=shape[:2]
        require((rows,width)==(meta['rows'],meta['columns']),'original column shape differs')
        data=np.empty((width,rows),dtype=np.uint64)
        for column in range(width):
            require(stream.readline()==b'0\n','non-numeric prefix input')
            encoded=stream.read(rows*17)
            require(len(encoded)==rows*17 and all(encoded[i]==10 for i in range(16,len(encoded),17)),
                    'malformed original input words')
            data[column]=np.frombuffer(bytes.fromhex(encoded.decode('ascii')),dtype='>u8').astype(np.uint64)
    with np.load(base.with_suffix('.npz'),allow_pickle=False) as arrays:
        require(set(arrays.files)=={str(i) for i in set(meta['outputs'].values())|set(meta.get('saved',[]))},
                'retained output channel set differs')
        for key in arrays.files:
            values=arrays[key]
            require(values.dtype==np.uint64 and values.shape==(rows,),'retained output dtype/shape differs')
            data[int(key)]=values
    require(hashlib.sha256(data.tobytes(order='C')).hexdigest()==expected,'reconstructed full columns differ')
    with Path(destination).open('xb') as stream:data.tofile(stream)
    q.science.check_oracle(q.read(base.with_suffix('.json')),Path(destination),meta)


def recover(plan,manifest,qualification,plan_path,output,*,copy_files):
    """Revalidate every retained invocation before any new native work."""
    import ownership_qualification_diagnostics as d
    paths,old,stamps=validate(plan,manifest,plan_path)
    directory=paths['previous_results'].parent
    with tempfile.TemporaryDirectory(prefix='eligibility-validation-',dir=output) as temp:
        transport.verify_recovered(plan,manifest,qualification,plan_path,Path(temp),old['calls'][0])
        checked=copy.deepcopy(old)
        for offset in range(0,30,5):
            calls=checked['calls'][offset:offset+5]
            evidence=d.mechanism_evidence(calls[0]['family'],calls[0]['workers'],calls,plan['request'])
            if calls[0]['cell'] in old['cells']:
                require(evidence==old['cells'][calls[0]['cell']],'previously completed mechanism record changed')
            checked['cells'][calls[0]['cell']]=evidence
        stamps.update(d.verify_calls(plan,manifest,qualification,directory,checked))
        families={c['family']:c for c in manifest['cases']}
        for index,call in enumerate(old['calls']):
            if 'reference' not in call:continue
            item=families[call['family']];meta=q.metadata(manifest,item,qualification)
            reconstruct(q.resolve(qualification,item['input']),directory/call['reference']['path'],meta,
                        Path(temp)/f'columns-{index}',call['columns_sha256'])
    require(all(q.stamp(p)==v for p,v in stamps.items()),'prefix evidence drift during validation')
    if copy_files:
        for source in set(cohort_files(directory,old).values()):
            target=Path(output)/source.relative_to(directory)
            target.parent.mkdir(parents=True,exist_ok=True)
            require(not target.exists(),'prefix destination already exists')
            shutil.copyfile(source,target)
            require(q.digest(source)==q.digest(target),'prefix copy differs')
    return checked,old,stamps
