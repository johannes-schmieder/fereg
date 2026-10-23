"""One hash-bound 1+79 continuation of the missing REPLAY_TIMING transport.

Original failures stay immutable. Only the first successful incumbent invocation
may be decoded offline; no native timer or RSS measurement is manufactured.
"""
from __future__ import annotations

import copy
import hashlib
import math
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile

import numpy as np
import qualify as q
import qualify_ownership as own
import routing

require = q.require
KIND = 'ownership-diagnostic-inline-transport-v1'
TIMER = 'observed_process_span_upper_bound'
OLD_DIAGNOSTIC_SHA = '8198e8d94a9536091cf0c2134db8ccafaeba09f449d885c4c787465eb8386f55'


def normalized_manifest(value):
    """Permit paths to move, and the corrected harness alone to change bytes."""
    value = copy.deepcopy(value)
    files = value.pop('files')
    harness = value.pop('harness')
    def visit(item):
        if isinstance(item, str):
            return {'file_sha256': files[item]} if item in files else item
        if isinstance(item, list):
            return [visit(v) for v in item]
        if isinstance(item, dict):
            return {k: visit(v) for k, v in item.items()}
        return item
    return visit(value), sorted(v for k, v in files.items() if k != harness)


def inline_words(raw, meta):
    """No conversion through floating point: preserve every uint64 bit pattern."""
    columns = raw.get('columns')
    require(isinstance(columns, list) and len(columns) == meta['columns'], 'inline column count differs')
    arrays = []
    for column in columns:
        require(isinstance(column, list) and len(column) == meta['rows']
                and all(isinstance(v, str) and re.fullmatch('[0-9a-f]{16}', v) for v in column),
                'inline numeric word/row framing differs')
        words = np.array([int(v, 16) for v in column], dtype=np.uint64)
        require([format(int(v), '016x') for v in words] == column, 'inline bit roundtrip failed')
        arrays.append(words)
    require(sys.byteorder == 'little', 'continuation supports the bound little-endian platforms only')
    return arrays


def timing(call):
    start, end = call['started_monotonic_ns'], call['finished_monotonic_ns']
    require(type(start) is int and type(end) is int and 0 < start < end, 'invalid prior process span')
    seconds = (end-start)/1e9
    require(0 < seconds <= 120, 'prior complete-process upper bound exceeds call budget')
    return seconds


def freeze(qualification, previous_plan, previous_results, old_build, new_build,
           attempt_seconds, output, *, accounting=None, job_log=None):
    import ownership_qualification_diagnostics as d
    output = Path(output).resolve()
    old_path = Path(previous_results).resolve()
    old = q.read(old_path)
    call = old['calls'][0]
    entries = dict(previous_plan=previous_plan, previous_results=previous_results,
                   old_build=old_build, new_build=new_build,
                   raw_head=old_path.parent/call['target'], raw_log=old_path.parent/call['log'],
                   request=old_path.parent/call['cell']/'incumbent.case',
                   observations=old_path.parent/'host/host-observations.jsonl')
    old_plan = q.read(Path(previous_plan))
    entries['previous_qualification'] = q.resolve(previous_plan, old_plan['qualification'])
    if accounting is not None: entries['accounting'] = accounting
    if job_log is not None: entries['job_log'] = job_log
    require(math.isfinite(attempt_seconds) and old['elapsed_s'] <= attempt_seconds < 2400,
            'attempt charge does not cover the failed run')
    amendment = dict(kind=KIND, files={k:dict(path=os.path.relpath(Path(v).resolve(), output.parent),
                         sha256=q.digest(Path(v))) for k,v in entries.items()},
                     attempt_seconds=attempt_seconds, charged_seconds=math.ceil(attempt_seconds),
                     reused_calls=1, new_calls=79, total_calls=80)
    return d.freeze(qualification, output, repair=amendment)


def validate(plan, manifest, plan_path):
    """Revalidate immutable prior failure and the exact prospective amendment."""
    import ownership_qualification_diagnostics as d
    from qualify_scc import fields, short_host
    amendment = plan['transport_repair']
    require(set(amendment) == {'kind','files','attempt_seconds','charged_seconds','reused_calls','new_calls','total_calls'}
            and amendment['kind'] == KIND
            and (amendment['reused_calls'],amendment['new_calls'],amendment['total_calls']) == (1,79,80),
            'transport continuation contract changed')
    names = {'previous_plan','previous_results','previous_qualification','old_build','new_build',
             'raw_head','raw_log','request','observations'}
    if plan['platform']=='Linux': names |= {'accounting','job_log'}
    require(set(amendment['files']) == names, 'continuation evidence coverage differs')
    paths, stamps = {}, {}
    for key, binding in amendment['files'].items():
        require(set(binding) == {'path','sha256'}, 'invalid continuation file binding')
        path = q.resolve(plan_path,binding['path']); before=q.stamp(path)
        require(q.digest(path)==binding['sha256'] and q.stamp(path)==before, 'prior evidence drift: '+key)
        paths[key]=path;stamps[path]=before
    old, prior = q.read(paths['previous_results']), q.read(paths['previous_plan'])
    before = q.read(paths['previous_qualification'])
    require(prior['schema']==1 and prior['kind']==d.KIND and 'transport_repair' not in prior
            and prior['qualification_sha256']==q.digest(paths['previous_qualification'])
            and prior['qualification_identity']==routing.identity(before)
            and prior['platform']==plan['platform'] and prior['schedule']==plan['schedule']
            and prior['candidate_sha256']==plan['candidate_sha256']
            and prior['incumbent_sha256']==plan['incumbent_sha256']
            and prior['request']==plan['request']
            and [prior[k] for k in ('native_calls','timeout_s','budget_s','retries')]==[80,120,2400,0],
            'original fixed diagnostic plan changed')
    old_sources=dict(prior['execution_sources']);new_sources=dict(plan['execution_sources'])
    require(old_sources.pop('ownership_qualification_diagnostics')==OLD_DIAGNOSTIC_SHA,
            'not the diagnosed diagnostic source')
    new_sources.pop('ownership_qualification_diagnostics');new_sources.pop('ownership_diagnostic_transport')
    require(old_sources==new_sources and normalized_manifest(before)==normalized_manifest(manifest),
            'continuation changes more than the harness/repair tooling')
    require(old['status']=='failed' and old['native_calls_started']==len(old['calls'])==1
            and old['cells']=={} and old['retries']==0 and old['exclusions']==[]
            and old['failures']==[dict(type='ValueError',message='native harness must write binary columns')]
            and old['plan_sha256']==q.digest(paths['previous_plan'])
            and old['qualification_sha256']==prior['qualification_sha256']
            and old['schedule']==prior['schedule'] and old['platform']==plan['platform']
            and old['candidate_sha256']==plan['candidate_sha256']
            and old['incumbent_sha256']==plan['incumbent_sha256'], 'not the single diagnosed transport failure')
    call=old['calls'][0]
    require({k:call[k] for k in ('cell','family','workers','arm','repeat')}==plan['schedule'][0]
            and call['status']=='failed' and call['arm']=='incumbent' and call['workers']==1
            and call['result_sha256']==q.digest(paths['raw_head'])
            and call['log_sha256']==q.digest(paths['raw_log'])
            and call['request_sha256']==q.digest(paths['request'])
            and not re.search(r'^NATIVE_SECONDS=',paths['raw_log'].read_text(),re.M),
            'original first invocation identity changed')
    duration=amendment['attempt_seconds'];charge=amendment['charged_seconds']
    require(type(duration) in (float,int) and math.isfinite(duration)
            and 0 < old['elapsed_s'] <= duration < 2400 and charge==math.ceil(duration),
            'invalid remaining application budget')
    timing(call)
    observed={**old,'host_observations_sha256':q.digest(paths['observations'])}
    require(paths['observations']==paths['previous_results'].parent/'host/host-observations.jsonl',
            'observer path is not the original cohort')
    d.verify_host(observed,paths['previous_results'].parent,plan['platform'])
    if plan['platform']=='Linux':
        account=fields(paths['accounting'].read_text());host=old['host']['host']
        require(account['failed']=='0' and account['exit_status']=='1'
                and account.get('project')=='welfgr' and bool(account.get('end_time'))
                and str(host['task_id']) in ('0','undefined')
                and account['jobnumber']==host['job_id'] and account['slots']=='28'
                and account['granted_pe']==host['pe']
                and short_host(account['hostname'])==short_host(host['hostname'])
                and account.get('taskid') in ('undefined','0')
                and int(account['ru_wallclock'])>=math.ceil(old['elapsed_s'])
                and d.MARKER not in paths['job_log'].read_text(), 'failed allocation accounting differs')
    old_build,new_build=(q.read(paths[n]) for n in ('old_build','new_build'))
    for build,value in ((old_build,before),(new_build,manifest)):
        require(build['sha256']==value['files'][value['harness']]
                and build['native_calls']==0 and build['max_rows']==1000000
                and build['max_cells']==10000000, 'harness identity/capacity differs')
    require(all(new_build['source_sha256'].get(k)==v for k,v in old_build['source_sha256'].items() if k!='bench/native.py'),
            'harness source changed')
    required_sources={'plugin/stplugin.h','tests/mock_spi/actual_plugin_loader_test.c',
                      'tests/mock_spi/replay.h','tests/mock_spi/replay_io.h'}
    require(set(new_build['source_sha256'])==required_sources, 'compiled harness source coverage differs')
    for name,expected in new_build['source_sha256'].items():
        source=q.ROOT/name;before_stamp=q.stamp(source)
        require(q.digest(source)==expected and q.stamp(source)==before_stamp, 'compiled harness source drift')
        stamps[source]=before_stamp
    command=new_build['command']
    base_command=['cc','-std=c11','-O2','-Wall','-Wextra','-Wpedantic','-Wconversion',
                  '-Wshadow','-Wstrict-prototypes','-Wmissing-prototypes','-Werror',
                  '-DSYSTEM='+('APPLEMAC' if plan['platform']=='Darwin' else 'OPUNIX'),
                  '-DREPLAY_MAX_ROWS=1000000','-DREPLAY_MAX_CELLS=10000000ULL',
                  '-I<repo>/plugin','<repo>/tests/mock_spi/actual_plugin_loader_test.c',
                  '-lc' if plan['platform']=='Darwin' else '-ldl','-o','<output>/actual_plugin_loader_test']
    expected_command=base_command[:11]+['-D_POSIX_C_SOURCE=200809L','-DREPLAY_TIMING']+base_command[11:]
    require(command==expected_command, 'harness compiler flags exceed the transport amendment')
    if 'command' in old_build:
        old_command=list(old_build['command']);old_command[-1]='<output>/actual_plugin_loader_test'
        require(old_command==base_command, 'old harness build flags differ from the diagnosed recipe')
    require(new_build['platform']==plan['platform'] and bool(new_build['compiler'])
            and new_build['preprocessor_macros']==dict(REPLAY_MAX_ROWS='1000000',
                REPLAY_MAX_CELLS='10000000ULL',REPLAY_TIMING='1',_POSIX_C_SOURCE='200809L'),
            'corrected harness actual compiler/preprocessor evidence differs')
    require(command.count('-DREPLAY_TIMING')==1 and '-DREPLAY_MAX_ROWS=1000000' in command
            and '-DREPLAY_MAX_CELLS=10000000ULL' in command
            and new_build['source_sha256'].get('tests/mock_spi/replay.h')==q.digest(q.ROOT/'tests/mock_spi/replay.h'),
            'corrected harness timing/header contract missing')
    return paths, old, stamps


def recovered(plan, manifest, qualification, plan_path, folder):
    """Create separately named derived evidence; never touch the failed cohort."""
    paths,old,_=validate(plan,manifest,plan_path)
    first=old['calls'][0];item=next(c for c in manifest['cases'] if c['family']==first['family'])
    meta=q.metadata(manifest,item,qualification)
    raw=q.read(paths['raw_head']);words=inline_words(raw,meta);raw.pop('columns');q.validate_result(raw)
    source=q.resolve(qualification,item['input'])
    with source.open('rb') as stream:shape,args=q.case_header(stream);body=hashlib.sha256(stream.read()).hexdigest()
    with paths['request'].open('rb') as stream:
        request_shape,request_args=q.case_header(stream)
        shape[2]=len(request_args)
        require(request_shape==shape and request_args==own.arguments(args,'auto',1)
                and hashlib.sha256(stream.read()).hexdigest()==body,'imported request body/argv changed')
    folder=Path(folder);folder.mkdir(parents=True,exist_ok=True)
    target=folder/'call-00.json';columns=target.with_suffix('.json.columns');log=target.with_suffix('.log')
    require(not any(p.exists() for p in (target,columns,log)), 'derived evidence already exists')
    with columns.open('xb') as stream:
        for column in words:stream.write(column.tobytes())
    q.science.check_oracle(raw,columns,meta)
    q.write(target,{**raw,'columns':[]});shutil.copyfile(paths['raw_log'],log)
    detail=q.diagnostics(raw,False)
    require(detail['solver_rows'][0][0]==q.cmgschur.EXPECTED_SOLVER['auto'],
            'imported invocation is not Auto')
    detail.update(own.resource_detail(manifest,'incumbent',meta,log.read_text(),request_args,args,1))
    detail.update(seconds=timing(first),timing_field=TIMER,
                  fingerprint=routing.identity({**q.exact_view(raw,False),'columns_sha256':q.digest(columns)}),
                  worker_signature=q.worker_signature(raw),raw_result=raw,columns_sha256=q.digest(columns),
                  maxrss_bytes=None,maxrss_status='unavailable_before_parser_failure')
    call={**first,**detail}
    call.update(status='passed',oracle='passed',result_sha256=q.digest(target),
          log_sha256=q.digest(log),transport_recovery=dict(original_status='failed',
          original_result_sha256=first['result_sha256'],original_log_sha256=first['log_sha256'],
          original_summary_sha256=q.digest(paths['previous_results']),conversion='uint64-column-major-bit-exact'))
    return raw,columns,call


def same_host(host, old):
    from qualify_scc import short_host
    require(short_host(host['host']['hostname'])==short_host(old['host']['host']['hostname']),
            'continuation requires the original physical host')


def verify_recovered(plan,manifest,qualification,plan_path,directory,call):
    with tempfile.TemporaryDirectory(prefix='inline-verification-',dir=directory) as temp:
        _,_,expected=recovered(plan,manifest,qualification,plan_path,Path(temp))
        expected['reference']=call['reference']
        require(call==expected,'imported scientific/transport evidence differs')
