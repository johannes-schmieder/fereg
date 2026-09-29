"""Evidence-bound production continuation; never rewrites a rejected pilot.

Deploy separately from the frozen measurement controller. ADR0226 specifies
which original receipts and independent adjudication authorize this plan.
"""
import argparse
import importlib.util
import hashlib
import json
import math
from pathlib import Path
import sys


def require(condition, message):
    if not condition:
        raise ValueError(message)


def clean_accounting(receipt, job, tasks):
    require(receipt['job_id']==str(job) and receipt['status']=='pass' and not receipt['missing_tasks'], 'unaccepted accounting')
    rows=receipt['records']
    require(sorted(r['task_id'] for r in rows)==sorted(tasks), 'missing or duplicate accounting task')
    require(all(r['failed']==0 and r['exit_status']==0 and r['slots']==28 and r['jobnumber']==str(job) for r in rows), 'failed accounting record')


def oracle_certificates(oracle, cell):
    require(oracle['status']=='pass' and oracle['N']==cell['n'], 'oracle sample/solve failed')
    score_bound=1e-10 if cell['model']=='ols' else 1e-11
    require(0<=oracle['maximum_scaled_score']<=score_bound, 'oracle score failed')
    for name in ('maximum_scaled_projection','inverse_error'):
        require(0<=oracle['certificates'][name]<=1e-9, 'oracle Hessian/projection failed')
    trace=oracle['factor_trace']
    require(bool(trace) and trace[-1]['rank']==oracle['model_rank'], 'missing final-weight factor')
    require(oracle['fe_rank']+cell['k']==oracle['model_rank'], 'inconsistent independent rank')
    for factor in trace:
        require(0<=factor['largest_discarded']<factor['lower_rank_cutoff']<=factor['cutoff']
                <factor['upper_rank_cutoff']<factor['smallest_kept'], 'ambiguous explicit rank')
    if cell['model']=='ppml':
        require(len(trace)==len(oracle['iteration_trace'])+1, 'missing fresh final-Hessian factor')


def accept(root, attempt, publish=True):
    from common import cell_id, manifest_for, read_json, sha256, write_json
    from provenance import verify
    from light_control import verify_acceptance
    from light import active_arms, skips
    from runner import compare
    identity=verify(root);verify_acceptance(root,'smoke');manifest=manifest_for(root)
    profile=read_json(Path(__file__).with_name('adjudication-launch.json'))
    pilot_profile=read_json(Path(__file__).with_name('pilot-launch.json'))
    require(identity==profile['parent_fingerprint']==pilot_profile['measurement_fingerprint'], 'unregistered parent runtime')
    require(sha256(root/'plans/pilot.json')==pilot_profile['pilot_plan_sha256'], 'registered pilot plan changed')
    evidence={}
    def bind(path):
        evidence[str(path.relative_to(root))]=sha256(path)
        return read_json(path) if path.suffix=='.json' else None
    for name in ['env/SMOKE.ACCEPTED.json','env/frozen.json','input/manifest.json','input/historical-runs.csv']:
        bind(root/name)
    pilot=bind(root/'plans/pilot.json');rejected=bind(root/'env/PILOT.REJECTED.json')
    require(pilot['mode']=='pilot' and pilot['manifest_sha256']==sha256(root/'input/manifest.json'),'pilot manifest changed')
    expected_failures={tuple(f[0]) for f in rejected['failures']}
    require(len(expected_failures)==len(rejected['failures']) and all(f[1]=='no scientifically comparable pilot reference' for f in rejected['failures']), 'unsupported pilot failure')
    unresolved={};completed=[]
    for number,bundle in enumerate(pilot['bundles'],1):
        receipt=bind(root/'bundle-results'/f'pilot-{number}.json')
        require(receipt['status']=='complete' and not receipt['pending'] and receipt['completed']==bundle['groups']
                and receipt['measurement_fingerprint']==identity and receipt['plan_sha256']==sha256(root/'plans/pilot.json'), 'pilot bundle incomplete or changed')
        job=receipt['job_id'];account=bind(root/'accounting'/f'{job}.json')
        clean_accounting(account,job,list(range(1,len(pilot['bundles'])+1)))
        for group in bundle['groups']:
            cell,rep=group['cell'],group['repetition'];key=(cell_id(cell),rep)
            require(key not in completed,'duplicate pilot group');completed.append(key)
            directory=root/'results'/key[0]/f'warm-{rep}'
            original=bind(directory/'GROUP.COMPLETE.json')
            require(original['cell']==cell and original['repetition']==rep and original['measurement_fingerprint']==identity
                    and str(original['job_id'])==str(job) and original['skips']==skips(cell,rep,manifest),'pilot group identity changed')
            arms=active_arms(group,manifest);native=arms[0]
            require(set(original['arm_status'])==set(arms) and original['arm_status'][native]=='pass','native pilot fit failed')
            bind(directory/'input.json')
            for arm in arms:
                record=bind(directory/arm/'result.json')
                require(record['measurement_fingerprint']==identity and record['status']==original['arm_status'][arm]
                        and str(record['job_id'])==str(job) and str(record['task_id'])==str(number),'arm identity/accounting mismatch')
                for name in ('coefficients.csv','covariance.csv'):
                    if (directory/arm/name).exists():bind(directory/arm/name)
            comparisons=[compare(directory/native,directory/arm,manifest['science']) for arm in arms[1:]]
            require(comparisons==original['comparisons'],'captured pilot comparison changed')
            if not any(c['status']=='pass' and c['checks'].get('sample_exact') for c in comparisons):
                unresolved[key]=directory
    require(len(completed)==pilot['planned_groups']==12 and set(unresolved)==expected_failures,'rejection does not describe complete pilot')
    source=bind(attempt/'source.json')
    require(source['commit']==profile['diagnostic_source_commit'] and source['files']==profile['diagnostic_source_files'], 'unregistered diagnostic source')
    diagnostic=bind(attempt/'result/result.json');plan=bind(attempt/'result/plan.json')
    require(str(plan['job_id'])==profile['job_id'], 'unregistered diagnostic job')
    require(diagnostic['plan']==plan and diagnostic['status']=='complete' and diagnostic['all_science_pass'] is True
            and plan['parent_fingerprint']==identity and plan['source']==source['files'],'diagnostic identity or completion failed')
    for name,digest in source['files'].items():
        require(Path(name).name==name and sha256(attempt/'source'/name)==digest,'diagnostic source changed')
        bind(attempt/'source'/name)
    clean_accounting(bind(root/'accounting'/f'{plan["job_id"]}.json'),plan['job_id'],[1])
    log=attempt/'job.log';bind(log)
    require('PILOT_ADJUDICATION_COMPLETE True' in log.read_text(),'diagnostic success marker missing')
    spec=importlib.util.spec_from_file_location('frozen_adjudication',attempt/'source/run.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    require(plan['cases']==[list(c) for c in module.CASES] and plan['diagnostic_id']==hashlib.sha256(json.dumps(plan['source'],sort_keys=True).encode()).hexdigest(),'diagnostic plan changed')
    records=diagnostic['records'];covered=set();forecast=[]
    for record in records:
        cell=record['cell'];key=(cell_id(cell),1)
        require(key in unresolved and key not in covered,'unexpected or duplicate diagnostic case');covered.add(key)
        original=unresolved[key];directory=attempt/'result'/key[0]
        require(cell==read_json(original/'GROUP.COMPLETE.json')['cell'],'adjudicated model differs from pilot')
        native='fereg' if cell['model']=='ols' else 'feppml'
        arm=dict(plan['cases'])[key[0]]
        require(arm==('reghdfe' if cell['model']=='ols' else 'fixest_strict'),'unregistered reference')
        require(bind(directory/'cell.json')==cell,'diagnostic cell changed')
        regenerated=bind(directory/'input.json');captured=read_json(original/'input.json')
        require(regenerated['sha256']==captured['sha256']==record['original_input_sha256']
                and regenerated['column_sha256']==captured['column_sha256'],'diagnostic inputs differ')
        for path,digest in [(original/'GROUP.COMPLETE.json',record['original_group_sha256']),
                            (original/native/'result.json',record['native_result_sha256']),
                            (directory/arm/'result.json',record['reference_result_sha256']),
                            (directory/'oracle.json',record['oracle_sha256'])]:
            require(sha256(path)==digest,'adjudicated evidence changed');bind(path)
        for base,hashes in [(original/native,record['native_output_hashes']),(directory/arm,record['reference_output_hashes'])]:
            require({'coefficients.csv','covariance.csv'}<=set(hashes),'missing adjudicated numeric outputs')
            for name,digest in hashes.items():
                require(Path(name).name==name and sha256(base/name)==digest,'numeric output changed');bind(base/name)
        oracle=read_json(directory/'oracle.json');oracle_certificates(oracle,cell)
        for label,path in [('native',original/native),('reference',directory/arm)]:
            computed=module.compare(path,oracle,cell,manifest['science'])
            require(computed==record[label] and computed['status']=='pass' and all(computed['checks'].values()),'independent comparison failed')
        reference=read_json(directory/arm/'result.json')
        require(reference['status']=='pass' and reference['sentinel'] and str(reference['job_id'])==str(plan['job_id'])
                and reference['input_sha256']==captured['sha256']
                and reference['measurement_fingerprint']=='diagnostic-'+plan['diagnostic_id'],'new reference identity failed')
        require(math.isfinite(reference['seconds']) and reference['seconds']>0,'invalid diagnostic forecast')
        forecast.append(dict(cell=cell,arm=arm,seconds=reference['seconds']))
    require(covered==set(unresolved),'unadjudicated pilot case')
    target=root/'env/PILOT.ADJUDICATED.json'
    value=dict(schema='pilot-adjudicated-production-gate-v1',measurement_fingerprint=identity,
               planner_sha256=sha256(Path(__file__)),profile_sha256=sha256(Path(__file__).with_name('adjudication-launch.json')),
               pilot_profile_sha256=sha256(Path(__file__).with_name('pilot-launch.json')),evidence=evidence,
               completed_groups=[list(k) for k in completed],
               adjudicated_groups=[list(k) for k in sorted(covered)],forecast_only=forecast,
               policy='ADR0226: original failed pairs remain unranked; diagnostic clocks are never timing pairs')
    if publish:
        if target.exists():require(read_json(target)==value,'existing gate changed')
        else:write_json(target,value)
    return target,value


def production(root,gate_path,gate):
    from common import cell_id,manifest_for,read_json,sha256
    from light import groups,historical_costs,configure_group,pack
    manifest=manifest_for(root);history=root/'input/historical-runs.csv'
    require(sha256(history)==manifest['historical_cost_sha256'],'historical costs changed')
    costs=historical_costs(history);results=root/'results'
    completed=set(tuple(g) for g in gate['completed_groups'])
    registered=groups(manifest);keys={(cell_id(g['cell']),g['repetition']) for g in registered}
    require(len(keys)==len(registered)==568 and completed<=keys,'registered grid changed')
    # This entry point creates the first production plan only. Later pending
    # recovery must audit terminal tasks; it cannot silently repeat live work.
    found=set()
    measured={}
    for path in results.glob('**/GROUP.COMPLETE.json'):
        record=read_json(path);key=(cell_id(record['cell']),record['repetition']);found.add(key)
        require(key in completed,'unexpected production work already exists')
        measured[key[0]]=record['elapsed_seconds']
    require(found==completed,'missing accepted pilot group')
    for path in results.glob('**/result.json'):
        record=read_json(path)
        require(record['measurement_fingerprint']==gate['measurement_fingerprint'],'foreign result')
        if record['status']=='pass':
            c=record['cell'];costs[c['model'],c['design'],c['n'],c['k'],c['threads'],record['arm']]=[record['seconds']]
    for record in gate['forecast_only']:
        c=record['cell'];costs[c['model'],c['design'],c['n'],c['k'],c['threads'],record['arm']]=[record['seconds']]
    prepared=[configure_group(g,manifest,costs) for g in registered if (cell_id(g['cell']),g['repetition']) not in completed]
    for group in prepared:
        group['predicted_seconds']=max(group['predicted_seconds'],measured.get(cell_id(group['cell']),0)*1.2)
    wall=manifest['scc']['wall_seconds'];bundles=pack(prepared,manifest['scc']['target_bundle_seconds'],wall)
    return dict(schema='systematic-light-plan-v2',mode='production',bundles=bundles,wall_seconds=wall,
                results_directory='results',planned_groups=len(prepared),mem_per_core_gib=3,
                predicted_node_hours=sum(g['predicted_seconds'] for g in prepared)/3600,
                manifest_sha256=sha256(root/'input/manifest.json'),
                continuation_gate=dict(path=str(gate_path.relative_to(root)),sha256=sha256(gate_path),planner_sha256=sha256(Path(__file__))))


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('root',type=Path);parser.add_argument('attempt',type=Path);parser.add_argument('plan',type=Path,nargs='?');parser.add_argument('--check-only',action='store_true')
    args=parser.parse_args();sys.path.insert(0,str(args.root/'controller'))
    from common import write_json
    if args.check_only:
        _,gate=accept(args.root,args.attempt,publish=False)
        print('ADJUDICATION_GATE_CHECK_PASS',len(gate['completed_groups']),len(gate['adjudicated_groups']))
    else:
        require(args.plan is not None and not args.plan.exists(),'provide a new immutable production plan path')
        gate_path,gate=accept(args.root,args.attempt)
        value=production(args.root,gate_path,gate);write_json(args.plan,value)
        print({k:v for k,v in value.items() if k!='bundles'})
