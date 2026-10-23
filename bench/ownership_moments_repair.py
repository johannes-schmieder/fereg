"""One bounded continuation of the diagnosed unsupported cluster request.

The failed cohort remains failed. Its 45 complete HC1 calls are imported by
hash and original provenance, never executed again. Only the two clustered
requests change to the existing frontend contract; production bytes are fixed.
"""
import copy
import hashlib
import math
import os
from pathlib import Path

import qualify as q
import qualify_scc as scc
import routing

# Canonical identity from the frozen manifest, not its pretty-printed file hash.
ORIGINAL_PLAN = '480eac7a73f50deabc1b0f44402a3c3c3a1912ffae69332343993191377ee3a4'
ERROR = 'the guarded clustered OLS action requires cluster-aware FE nesting'
require = q.require


def freeze_amendment(config, bind, config_path):
    require(set(config) == {'plan', 'result', 'accounting', 'job_log'}, 'invalid continuation configuration')
    result_path = q.resolve(config_path,config['result'])
    old = q.read(result_path)
    extra = dict(failed_target=result_path.parent/old['calls'][-1]['target'],
                 failed_log=result_path.parent/old['calls'][-1]['log'],
                 host_observations=result_path.parent/'host/host-observations.jsonl',
                 host_summary=result_path.parent/'host/host.json')
    return dict(kind='cluster-request-repair-8261951',
                **{key: bind(str(value)) for key, value in {**config,**extra}.items()},
                original_plan_identity=ORIGINAL_PLAN, imported_calls=45, remaining_calls=63,
                failed_invocations=1, total_preflight_invocations=109, charged_seconds=36,
                remaining_diagnostic_seconds=1164, scheduler_seconds_charged=48,
                remaining_scheduler_seconds=4152, required_host='scc-ua1')


def validate_amendment(plan):
    amendment = plan['preflight_reuse']
    require(amendment['kind'] == 'cluster-request-repair-8261951'
            and amendment['original_plan_identity'] == ORIGINAL_PLAN
            and [amendment[k] for k in ('imported_calls', 'remaining_calls', 'failed_invocations',
                 'total_preflight_invocations', 'charged_seconds', 'remaining_diagnostic_seconds',
                 'scheduler_seconds_charged', 'remaining_scheduler_seconds')] == [45,63,1,109,36,1164,48,4152]
            and amendment['required_host'] == 'scc-ua1' and plan['platform'] == 'Linux',
            'bounded continuation contract changed')
    require(all(amendment[k] in plan['files'] for k in ('plan','result','accounting','job_log',
                'failed_target','failed_log','host_observations','host_summary')),
            'continuation evidence is not hash bound')


def previous(plan, manifest):
    validate_amendment(plan)
    amendment = plan['preflight_reuse']
    paths = {k:q.resolve(manifest,amendment[k]) for k in ('plan','result','accounting','job_log',
             'failed_target','failed_log','host_observations','host_summary')}
    require(all(q.digest(path)==plan['files'][amendment[k]] for k,path in paths.items()), 'prior evidence changed')
    old_plan, old = q.read(paths['plan']), q.read(paths['result'])
    require(routing.identity(old_plan)==old['manifest_sha256']==ORIGINAL_PLAN
            and old['status']=='failed' and old['native_calls_started']==len(old['calls'])==46
            and old['retries']==0 and old['exclusions']==[] and len(old['failures'])==1
            and ERROR in old['failures'][0]['message'] and math.ceil(old['elapsed_s'])==36,
            'not the diagnosed failed cohort')
    fields = scc.fields(paths['accounting'].read_text())
    require(all(fields.get(k)==v for k,v in {'jobnumber':'8261951','failed':'0','exit_status':'1',
            'slots':'28','ru_wallclock':'48','granted_pe':'mpi_28_tasks_per_node_u'}.items()),
            'failed job accounting differs')
    require(scc.short_host(fields['hostname'])==amendment['required_host']
            and old['host']['isolation_ok'] and not old['host']['errors']
            and q.read(paths['host_summary'])==old['host'], 'prior host evidence invalid')
    failed = old['calls'][-1]
    require(failed['cell']=='weak-cluster-t2' and failed['arm']=='reference'
            and failed['status']=='started' and q.read(paths['failed_target'])['return_code']==498
            and ERROR in paths['failed_log'].read_text(), 'failed invocation differs')
    require('MOMENTS_PREFLIGHT_COMPLETE' not in paths['job_log'].read_text()
            and 'MOMENTS_COVERAGE_PANEL_COMPLETE' not in paths['job_log'].read_text(), 'failed job was relabelled')
    require(old_plan['schedule']==plan['schedule'] and old_plan['diagnostic_schedule']==plan['diagnostic_schedule']
            and all(old_plan[k]==plan[k] for k in ('arms','expected_moments','control_expectations')),
            'comparison or work contract changed')
    for role in ('candidate','reference'):
        before, after = old_plan['artifacts'][role],plan['artifacts'][role]
        require(before['revision']==after['revision'] and old_plan['files'][before['path']]==plan['files'][after['path']],
                'native artifact changed')
    require(old_plan['files'][old_plan['harness']]==plan['files'][plan['harness']], 'native launcher changed')
    for before, after in zip(old_plan['cases'],plan['cases']):
        require(before['id']==after['id'], 'case identity changed')
        if before['id'].endswith('-cluster'):
            old_case,new_case = q.resolve(paths['plan'],before['input']),q.resolve(manifest,after['input'])
            require(q.digest(old_case)==old_plan['files'][before['input']], 'original cluster input changed')
            with old_case.open('rb') as stream:
                shape,args = q.case_header(stream)
                body = hashlib.sha256(stream.read()).hexdigest()
            with new_case.open('rb') as stream:
                new_shape,new_args = q.case_header(stream)
                require(new_shape==shape and hashlib.sha256(stream.read()).hexdigest()==body,
                        'cluster numeric data or model tables changed')
            require(args.count(b'account_cluster_nesting=0')==1
                    and new_args==[b'account_cluster_nesting=1' if a==b'account_cluster_nesting=0' else a for a in args],
                    'cluster amendment exceeds the frontend nesting flag')
            continue
        for b,a in ((before['input'],after['input']), (before['metadata'],after['metadata']),
                    (before['oracle']['path'],after['oracle']['path'])):
            require(old_plan['files'][b]==plan['files'][a], 'unchanged noncluster request/reference replaced')
    for index,call in enumerate(old['calls'][:45]):
        require(call['status']=='passed' and call['oracle']=='passed'
                and {k:call[k] for k in ('cell','arm','phase','block','mirror')}==plan['diagnostic_schedule'][index],
                'imported schedule changed')
        for field in ('target','log'):
            require(q.digest(paths['result'].parent/call[field])==call['result_sha256' if field=='target' else 'log_sha256'],
                    'imported raw evidence changed')
    for cell in dict.fromkeys(c['cell'] for c in old['calls'][:45]):
        for ref in old['cells'][cell]['references'].values():
            base = paths['result'].parent/ref['path']
            require(q.digest(base.with_suffix('.json'))==ref['head']
                    and q.digest(base.with_suffix('.npz'))==ref['outputs'], 'imported retained arrays changed')
    return paths['result'],old


def mapped_prefix(plan, manifest, output):
    path,old = previous(plan,manifest)
    prefix = os.path.relpath(path.parent,output)
    calls = copy.deepcopy(old['calls'][:45])
    for call in calls:
        for key in ('target','log'):
            call[key] = str(Path(prefix)/call[key])
    cells = {cell:copy.deepcopy(old['cells'][cell]) for cell in dict.fromkeys(c['cell'] for c in calls)}
    for cell in cells.values():
        for ref in cell['references'].values():
            ref['path'] = str(Path(prefix)/ref['path'])
    provenance = dict(original_result_sha256=q.digest(path), original_plan_identity=ORIGINAL_PLAN,
                      original_job='8261951', imported_calls=45, retained_failed_invocations=1,
                      total_preflight_invocations=109, original_elapsed_s=old['elapsed_s'], charged_seconds=36,
                      original_calls_sha256=routing.identity(old['calls'][:45]))
    return calls,cells,provenance


def import_prefix(plan, manifest, output, result, host):
    require(scc.short_host(host['host']['hostname'])==plan['preflight_reuse']['required_host'],
            'continuation requires the original physical host')
    calls,cells,provenance = mapped_prefix(plan,manifest,output)
    result['calls'] = calls
    result['cells'].update(cells)
    result.update(native_calls_started=45,native_calls_reused=45,native_calls_executed_here=0,
                  reused_preflight=provenance)


def verify_prefix(plan, manifest, output, result):
    calls,cells,provenance = mapped_prefix(plan,manifest,output)
    require(result['calls'][:45]==calls and all(result['cells'][k]==v for k,v in cells.items())
            and result['reused_preflight']==provenance and result['native_calls_reused']==45
            and result['native_calls_executed_here']==63, 'imported scientific provenance changed')
