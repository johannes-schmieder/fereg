#!/usr/bin/env python3
"""One frozen supplementary moment-coverage preflight and timing panel.

Preflight attests completed construction and omitted work before any scored
call. This is discovery evidence; it never activates or selects a candidate.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import random
import re
import tempfile

import numpy as np
import ownership_cells as oc
import qualify as q
import routing
import routing_oracle

KIND = oc.COVERAGE_KIND
CASE_IDS = ('weak-robust', 'wide-robust', 'weak-cluster', 'wide-cluster',
            'weak-low', 'weak-iid', 'weighted-robust')
ORDER_SEED, UNCERTAINTY_SEED = 2026106307, 2026106308
require = q.require


def settings():
    result = []
    for family in CASE_IDS:
        workers = (1, 2, 4) if family == 'weak-robust' else (2, 4) if family in CASE_IDS[:4] else (4,)
        result += [dict(cell=f'{family}-t{w}', family=family, workers=w,
                        eligible=family in CASE_IDS[:4] and w > 1) for w in workers]
    return result


def schedule(plan, diagnostic=False):
    roles = oc.physical_arms(plan)
    cells = [s['cell'] for s in settings()]
    if diagnostic:
        result = []
        for cell in cells:
            result.append(dict(cell=cell, arm='reference', phase='reference', block=-1, mirror=0))
            for repeat, order in enumerate((roles, list(reversed(roles)))):
                result += [dict(cell=cell, arm=arm, phase='preflight', block=repeat, mirror=repeat)
                           for arm in order]
        return result
    result = [dict(cell=cell, arm=arm, phase='warmup', block=-1, mirror=0)
              for cell in cells for arm in roles]
    rng = random.Random(ORDER_SEED)
    for block in range(6):
        for cell in rng.sample(cells, len(cells)):
            forward = rng.sample(roles, len(roles))
            for mirror, order in enumerate((forward, list(reversed(forward)))):
                result += [dict(cell=cell, arm=arm, phase='scored', block=block, mirror=mirror) for arm in order]
    return result


def validate(plan):
    require(plan['schema'] == 1 and plan['kind'] == KIND and plan['platform'] in ('Linux', 'Darwin'), 'invalid coverage plan')
    arms = {k: list(v) for k, v in oc.mechanisms(1, 'none').items()}
    require(plan['panel'] == 1 and plan['ownership'] == 'none' and plan['arms'] == arms
            and plan['aliases'] == oc.aliases(arms), 'supplementary arm meaning changed')
    require(plan['settings'] == settings() and plan['threads'] == [1, 2, 4]
            and plan['data_seed'] == oc.DATA_SEED and plan['order_seed'] == ORDER_SEED
            and plan['uncertainty_seed'] == UNCERTAINTY_SEED and plan['bootstrap_draws'] == 4096
            and plan['blocks'] == 6 and plan['warmups'] == 1 and plan['diagnostic_repeats'] == 2
            and plan['timeout_s'] == 120 and plan['budget_s'] == 2400 and plan['diagnostic_budget_s'] == 1200,
            'frozen coverage budget changed')
    require([c['id'] for c in plan['cases']] == list(CASE_IDS)
            and all(c['family'] == c['id'] and c['oracle']['kind'] == 'routing'
                    and c['memory_limit_bytes'] is None for c in plan['cases']), 'coverage cases changed')
    require(plan['schedule'] == schedule(plan) and plan['call_cap'] == 624
            and plan['diagnostic_schedule'] == schedule(plan, True)
            and len(plan['diagnostic_schedule']) == 108, 'coverage schedule changed')
    require(plan['preflight_output'] == 'moments-preflight'
            and plan['observer'] == ('whole-host' if plan['platform'] == 'Linux' else 'desktop'), 'coverage prerequisites changed')
    require(plan['control_expectations'] == {
        'weak-robust-t1': 'no construction or omission at W1',
        'weak-low-t4': 'preserve baseline construction state; no omission or score ownership',
        'weak-iid-t4': 'preserve positive needed construction; no omission or score ownership',
        'weighted-robust-t4': 'no construction, omission or score ownership'}, 'control contract changed')


def freeze(config, output):
    config, output = Path(config).resolve(), Path(output).resolve()
    cfg = q.read(config)
    require(set(cfg) == {'platform', 'inputs', 'harness', 'candidate', 'reference'}, 'invalid coverage configuration')
    files = {}
    def bind(path):
        path = (config.parent / path).resolve(strict=True)
        name = os.path.relpath(path, output.parent)
        files[name] = q.digest(path)
        return name
    inputs = bind(cfg['inputs']); panel_path = q.resolve(output, inputs); panel = q.read(panel_path)
    require(panel['kind'] == KIND + '-inputs' and panel['settings'] == settings(), 'wrong coverage inputs')
    require(panel['native_calls'] == 0 and panel['seed'] == oc.DATA_SEED, 'invalid input preparation')
    for name, digest in panel['files'].items():
        path = q.resolve(panel_path, name)
        require(q.digest(path) == digest, 'frozen supplementary input changed')
        bind(str(path))
    items, expected = [], {}
    for item in panel['cases']:
        source = q.resolve(panel_path, item['input'])
        meta_path = q.resolve(panel_path, item['metadata']); meta = q.read(meta_path)
        observed = oc.routing_cases.inspect_case(source, item['id'])
        require(meta['original_sha256'] == panel['original_inputs'][meta['original_file']]
                and meta['candidate_all_verified'] is True and meta['input_complete'] is False,
                'original data or frontend amendment identity changed')
        require(meta['case_sha256'] == q.digest(source) and meta['body_sha256'] == observed['body_sha256'], 'changed coverage request')
        with source.open('rb') as stream:
            shape, arguments = q.case_header(stream)
        fields = dict(a.decode().split('=', 1) for a in arguments[1:])
        require(fields['candidate_all'] == '1' and fields['input_complete'] == '0', 'frontend hints changed')
        # The independent reader also verifies finite numeric inputs and an all-true candidate column.
        data = routing_oracle.read_case(source)
        require(data['rows'] == shape[0], 'oracle input shape changed')
        expected_vce = 'cluster' if item['id'].endswith('-cluster') else 'iid' if item['id']=='weak-iid' else 'robust'
        require(fields['vce'] == expected_vce and fields['memory'] == ('low' if item['id']=='weak-low' else 'fast')
                and (item['id']=='weighted-robust' or np.all(data['weights']==1)), 'coverage request contract changed')
        columns = int(fields['term_count']) + int(fields['include_intercept']) + 1
        expected[item['id']] = dict(constructed_moment_rows=shape[0], constructed_moment_columns=columns,
            moment_workspace_bound_bytes=columns * 3 * (min(64, max(1, (shape[0]+16383)//16384))+1) * 8)
        copied = copy.deepcopy(item)
        copied['input'] = bind(str(source)); copied['metadata'] = bind(str(meta_path))
        ref = q.resolve(panel_path, item['oracle']['path'])
        _, witnesses = routing_oracle.load_reference(ref, meta['case_sha256'], meta['body_sha256'])
        copied['oracle']['path'] = bind(str(ref))
        for path, digest in witnesses.items():
            require(q.digest(path) == digest, 'oracle changed'); bind(str(path))
        items.append(copied)
    artifacts = {}
    for role in ('reference', 'candidate'):
        art = cfg[role]
        require(set(art) == {'path', 'revision', 'source_manifest', 'build'}
                and re.fullmatch('[0-9a-f]{40}', art['revision']) and art['build'], 'invalid artifact identity')
        artifacts[role] = {**art, 'path': bind(art['path']), 'source_manifest': bind(art['source_manifest'])}
        require(q.read(q.resolve(output, artifacts[role]['source_manifest']))['fereg'] == art['revision'], 'source revision differs')
    require(artifacts['reference']['revision'] == oc.REFERENCE_REVISION, 'accepted reference changed')
    arms = {k: list(v) for k,v in oc.mechanisms(1, 'none').items()}
    plan = dict(schema=1,kind=KIND,platform=cfg['platform'],panel=1,ownership='none',data_seed=oc.DATA_SEED,
        order_seed=ORDER_SEED,uncertainty_seed=UNCERTAINTY_SEED,bootstrap_draws=4096,threads=[1,2,4],
        blocks=6,warmups=1,timeout_s=120,budget_s=2400,diagnostic_budget_s=1200,diagnostic_repeats=2,
        observer='whole-host' if cfg['platform']=='Linux' else 'desktop',arms=arms,aliases=oc.aliases(arms),
        cases=items,settings=settings(),expected_moments=expected,inputs=inputs,harness=bind(cfg['harness']),
        artifacts=artifacts,preflight_output='moments-preflight',
        control_expectations={'weak-robust-t1':'no construction or omission at W1',
            'weak-low-t4':'preserve baseline construction state; no omission or score ownership',
            'weak-iid-t4':'preserve positive needed construction; no omission or score ownership',
            'weighted-robust-t4':'no construction, omission or score ownership'},
        execution_sources={n:q.digest(p) for n,p in oc.sources().items()})
    plan.update(schedule=schedule(plan),diagnostic_schedule=schedule(plan,True),call_cap=624,files=files)
    validate(plan);oc.save_new(output,plan)
    return plan


def execution_evidence(plan, result):
    calls = result['calls']
    require([{k:c[k] for k in ('cell','arm','phase','block','mirror')} for c in calls] == plan['diagnostic_schedule']
            and all(c['status']=='passed' and c['oracle']=='passed' for c in calls), 'preflight calls incomplete')
    evidence = []
    for setting in settings():
        rows = [c for c in calls if c['cell']==setting['cell']]
        reference = rows[0]
        require(reference['arm']=='reference', 'accepted reference missing')
        def semantic_actions(call):
            # ffi.rs reserves argv/String payloads exactly; metadata_memory.rs
            # charges one 24-byte String descriptor plus injected UTF-8 bytes
            # on both supported 64-bit hosts. Keep raw caps in every call.
            token = None if call['arm']=='reference' else oc.token(plan['arms'][call['arm']])
            extra = 0 if token is None else 24 + len(('inject='+token).encode())
            return [{k:(v-extra if k=='memory_bytes_used' else v) for k,v in a.items()
                     if k!='memory_bytes_limit'} for a in call['action_resources']]
        def route(call):
            phases = {'two_fe_gates', 'remaining_checkpoint', 'auto_direct_route',
                      'auto_direct_certified', 'auto_kernel_route'}
            records = [dict(part.split('=',1) for part in line.split('|')[1:] if '=' in part)
                       for line in call['solver_trace_lines']]
            return (call['route_trace'], call['route_hierarchy'],
                    [record for record in records if record.get('phase') in phases])
        for c in rows:
            require(bool(c['action_resources']), 'missing actual routing action admission')
            require(c['fingerprint']==reference['fingerprint'] and c['worker_signature']==reference['worker_signature'],
                    'preflight ownership/reference science or signature differs')
        states = {}
        for arm in oc.physical_arms(plan):
            repeated = [c for c in rows if c['arm']==arm]
            require(len(repeated)==2, 'preflight repeat missing')
            states[arm] = []
            for c in repeated:
                events = c['ownership_diagnostic']['events']
                built = [e for e in events if 'constructed_moment_rows' in e]
                omitted = sum(e.get('unused_moment_bytes',0) for e in events)
                score = next(e for e in events if 'score_admitted' in e)
                require(not built or built == [plan['expected_moments'][setting['family']]], 'constructed shape/workspace differs')
                if setting['eligible']:
                    require(built == [plan['expected_moments'][setting['family']]] if arm in ('B','scores')
                            else not built and omitted == plan['expected_moments'][setting['family']]['moment_workspace_bound_bytes'] > 0,
                            'positive baseline construction/omission prerequisite failed')
                    require(score['score_admitted'] == int(arm in ('scores','both')), 'eligible score ownership missing')
                else:
                    require(omitted==0, 'ineligible control omitted moments')
                    if setting['family']=='weak-iid': require(bool(built), 'IID must retain needed moment construction')
                    if setting['family']=='weighted-robust' or setting['workers']==1: require(not built, 'ineligible control constructed moments')
                    if setting['family'] in ('weak-low','weak-iid','weighted-robust'):
                        require(score['score_admitted']==0, 'ineligible control retained scores')
                states[arm].append(dict(constructed=built,omitted_workspace_bound_bytes=omitted,score=score))
            require(states[arm][0]==states[arm][1], 'work diagnostics changed across fixed-worker repeats')
            require(semantic_actions(repeated[0])==semantic_actions(repeated[1])
                    and route(repeated[0])==route(repeated[1]), 'repeat resource admission or route changed')
            require(route(repeated[0])==route(reference), 'ownership changed accepted routing decisions')
        if setting['family']=='weak-low':
            require(all(states[a][0]['constructed']==states['B'][0]['constructed'] for a in states), 'Low construction behavior changed')
        for a,b in [('B','moments'),('scores','both')]:
            aa=next(c for c in rows if c['arm']==a);bb=next(c for c in rows if c['arm']==b)
            require(semantic_actions(aa)==semantic_actions(bb), 'omission changed resource admission or route')
        evidence.append(dict(**setting,arms=states,accepted_exact=True))
    return dict(status='passed',settings=evidence,eligible_settings=8,control_settings=4,
                native_calls=108, argument_parent_deltas={arm:0 if oc.token(plan['arms'][arm]) is None
                    else 24+len(('inject='+oc.token(plan['arms'][arm])).encode()) for arm in oc.physical_arms(plan)},
                limitation='Workspace bounds and completed construction attest removed work, not isolated kernel time or lower admitted solver memory.')


def validate_preflight(plan, manifest, path):
    path = Path(path); result = q.read(path)
    require(result['kind']==KIND and result['status']=='complete' and result['diagnostic'] is True
            and result['manifest_sha256']==routing.identity(plan) and result['native_calls_started']==108
            and result['failures']==[] and result['retries']==0 and result['exclusions']==[]
            and result['elapsed_s']<1200 and result['host']['isolation_ok'] and not result['host']['errors'],
            'preflight execution prerequisite failed')
    require(result['execution_evidence']==execution_evidence(plan,result), 'preflight work evidence changed')
    require(q.digest(path.parent/'host/host-observations.jsonl')==result['host_observations_sha256'], 'preflight observers changed')
    for call in result['calls']:
        target,log=path.parent/call['target'],path.parent/call['log']
        require(q.digest(target)==call['result_sha256'] and q.digest(log)==call['log_sha256'], 'preflight raw evidence changed')
        raw=q.read(target);require(raw.pop('columns')==[] and raw==call['raw_result'], 'preflight head differs')
        require(call['fingerprint']==routing.identity({**q.exact_view(raw,False),'columns_sha256':call['columns_sha256']}), 'preflight fingerprint differs')
        timer = re.findall(r'^NATIVE_SECONDS=([0-9.]+)$', log.read_text(), re.M)
        require(len(timer)==1 and 0<float(timer[0])<=120 and float(timer[0])==call['seconds']
                and call['timing_field']=='NATIVE_SECONDS' and call['worker_signature']==q.worker_signature(raw),
                'preflight timer or signature changed')
        resource=q.science.resource_diagnostics(log.read_text(),None,False,{})
        require(all(call[k]==v for k,v in resource.items()), 'preflight resource evidence differs')
        if call['arm']!='reference':
            require(call['ownership_diagnostic']==oc.ownership_diagnostic(log.read_text(),plan['arms'][call['arm']]), 'preflight work counters differ')
    verify_requests(plan, manifest, path.parent, result)
    verify_arrays(plan, manifest, path.parent, result)
    oc.verify_files(plan,manifest)
    return result


def verify_requests(plan, manifest, directory, result):
    """Bind normalization to exactly the executed argv and unchanged data body."""
    directory = Path(directory)
    for item in plan['cases']:
        source = q.resolve(manifest,item['input'])
        with source.open('rb') as stream:
            original_shape, original_args = q.case_header(stream)
            body = hashlib.sha256(stream.read()).hexdigest()
        for setting in [s for s in settings() if s['family']==item['id']]:
            for arm in [*oc.physical_arms(plan),'reference']:
                request = directory/setting['cell']/(arm+'.case')
                with request.open('rb') as stream:
                    shape,args=q.case_header(stream)
                    require(hashlib.sha256(stream.read()).hexdigest()==body, 'executed request data changed')
                changes = {'solver':'auto','preconditioner':'auto','threads':str(setting['workers'])}
                expected = [a for a in original_args if a.split(b'=',1)[0].decode() not in changes]
                expected += [f'{k}={v}'.encode() for k,v in changes.items()]
                token = None if arm=='reference' else oc.token(plan['arms'][arm])
                if token is not None: expected += [('inject='+token).encode()]
                target_shape = list(original_shape); target_shape[2]=len(expected)
                require(shape==target_shape and args==expected, 'executed request argument contract changed')
                require(all(c['input_sha256']==q.digest(request) for c in result['calls']
                            if c['cell']==setting['cell'] and c['arm']==arm), 'executed request identity changed')


def verify_arrays(plan, manifest, directory, result):
    """Reconstruct full native columns, preserving input bits and output channels."""
    directory = Path(directory)
    for item in plan['cases']:
        source = q.resolve(manifest, item['input'])
        meta = q.metadata(plan, item, manifest)
        with source.open('rb') as stream:
            shape, _ = q.case_header(stream)
            rows, width = shape[:2]
            original = np.empty((width, rows), dtype=np.uint64)
            for j in range(width):
                require(stream.readline()==b'0\n', 'non-numeric coverage column')
                encoded = stream.read(rows*17)
                require(len(encoded)==rows*17 and all(encoded[i]==10 for i in range(16,len(encoded),17)),
                        'invalid coverage input framing')
                original[j] = np.frombuffer(bytes.fromhex(encoded.decode('ascii')),dtype='>u8').astype(np.uint64)
        first = {}
        for setting in [s for s in settings() if s['family']==item['id']]:
            cell = result['cells'][setting['cell']]
            for arm, ref in cell['references'].items():
                base = directory/ref['path']
                require(q.digest(base.with_suffix('.json'))==ref['head']
                        and q.digest(base.with_suffix('.npz'))==ref['outputs'], 'preflight retained arrays changed')
                data = original.copy()
                with np.load(base.with_suffix('.npz'),allow_pickle=False) as arrays:
                    require(set(arrays.files)=={str(i) for i in set(meta['outputs'].values())|set(meta.get('saved',[]))},
                            'preflight output channels changed')
                    for key in arrays.files:
                        require(arrays[key].dtype==np.uint64 and arrays[key].shape==(rows,), 'preflight output shape changed')
                        data[int(key)] = arrays[key]
                raw = q.read(base.with_suffix('.json'))
                fingerprint = routing.identity({**q.exact_view(raw,False),
                    'columns_sha256':hashlib.sha256(data.tobytes(order='C')).hexdigest()})
                require(fingerprint==cell['fingerprints'][arm] and q.worker_signature(raw)==cell['worker_signatures'][arm],
                        'preflight reconstructed scientific identity changed')
                require(all(c['fingerprint']==fingerprint for c in result['calls']
                            if c['cell']==setting['cell'] and c['arm']==arm), 'preflight repeats changed')
                with tempfile.NamedTemporaryFile(dir=directory,suffix='.columns') as temp:
                    data.tofile(temp); temp.flush()
                    q.science.check_oracle(raw,Path(temp.name),meta)
                if arm in first:
                    q.compare_saved(first[arm],base,meta)
                    require(q.worker_signature(q.read(first[arm].with_suffix('.json')))==q.worker_signature(raw),
                            'preflight cross-worker decisions changed')
                else:
                    first[arm]=base


def uncertainty(plan, comparisons):
    # Draw entire mirrored blocks jointly across all arms/settings. This is
    # conditional within-host uncertainty, never a host-population interval.
    draws=np.random.default_rng(UNCERTAINTY_SEED).integers(0,6,size=(4096,6))
    rows=[]
    for c in comparisons:
        value={k:c[k] for k in ('cell','arm')}
        for key in ('difference_s','excess_s','ratio'):
            paired=np.asarray([p[key] for p in c['paired_blocks']])
            value[key+'_bootstrap_95']=np.quantile(paired[draws].mean(axis=1),[.025,.975]).tolist()
        rows.append(value)
    return dict(uncertainty=rows,bootstrap_draws=4096,bootstrap_seed=UNCERTAINTY_SEED,
        uncertainty_scope='Descriptive percentile intervals from six paired mirrored blocks on one host; all arms/settings share block resampling. Not activation qualification.',
        discovery_nonregression=all(c['mean_excess_s']<=0 for c in comparisons),
        configuration_O='both',selection_authority='Johannes explicitly selected combined ownership after closing actual moment-work coverage; no nominal-winner reselection.')


def main():
    parser=argparse.ArgumentParser(description=__doc__); sub=parser.add_subparsers(dest='command',required=True)
    p=sub.add_parser('prepare');p.add_argument('original_inputs',type=Path);p.add_argument('output',type=Path)
    p=sub.add_parser('plan');p.add_argument('config',type=Path);p.add_argument('output',type=Path)
    for name in ('preflight','run'):
        p=sub.add_parser(name);p.add_argument('plan',type=Path);p.add_argument('output',type=Path)
        if name=='run':p.add_argument('--preflight-result',type=Path,required=True)
    p=sub.add_parser('report');p.add_argument('plan',type=Path);p.add_argument('directory',type=Path);p.add_argument('output',type=Path);p.add_argument('--accounting',type=Path,required=True)
    args=parser.parse_args()
    if args.command=='prepare':
        from ownership_moments_cases import prepare_inputs
        value=prepare_inputs(args.original_inputs,args.output)
    elif args.command=='plan': value=freeze(args.config,args.output)
    elif args.command in ('preflight','run'):
        value=oc.run(args.plan,args.output,diagnostic=args.command=='preflight',preflight_result=getattr(args,'preflight_result',None))
    else:value=oc.report(args.plan,args.directory,args.output,accounting=args.accounting)
    print(json.dumps({'status':value.get('status','prepared'),'kind':value['kind']}))


if __name__=='__main__':main()
