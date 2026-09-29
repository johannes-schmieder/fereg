"""Pack only checkpointed groups, preserving every original per-group setting."""
import argparse
import copy
from pathlib import Path
import subprocess
import sys


def pending_plan(original, bundles, completed_keys):
    pending=[];seen=set()
    for planned, receipt in zip(original['bundles'], bundles):
        if receipt['status'] not in ('complete','checkpointed'):
            raise ValueError('original task is not terminal')
        combined=receipt['completed']+receipt['pending']
        key=lambda g:(str(sorted(g['cell'].items())),g['repetition'])
        expected={key(g):g for g in planned['groups']}
        if len(combined)!=len(expected) or {key(g) for g in combined}!=set(expected):
            raise ValueError('checkpoint does not cover original groups')
        for group in combined:
            k=key(group)
            if group!=expected[k] or k in seen:raise ValueError('changed or duplicate group')
            seen.add(k)
        for group in receipt['completed']:
            if key(group) not in completed_keys:raise ValueError('completed group lost marker')
        for group in receipt['pending']:
            if key(group) in completed_keys:raise ValueError('pending group already completed')
            pending.append(group)
    if len(bundles)!=len(original['bundles']):raise ValueError('missing original bundle')
    plan=copy.deepcopy(original)
    plan.update(bundles=[dict(groups=pending,predicted_seconds=sum(g['predicted_seconds'] for g in pending))],
                wall_seconds=10800,planned_groups=len(pending),
                predicted_node_hours=sum(g['predicted_seconds'] for g in pending)/3600)
    return plan


def main(root):
    sys.path.insert(0,str(root/'controller'))
    from accounting import parse
    from campaign import audit
    from common import read_json,write_json,sha256,manifest_for
    from provenance import verify
    from light_control import verify_acceptance
    identity=verify(root)
    assert identity=='fb2ad5407afcea22d56155aa7858203bfcaea590f750ca5d06eeccce746cffc6'
    raw=subprocess.run(['qacct','-j','7766045'],capture_output=True,text=True,check=True,timeout=120)
    accounting=parse(raw.stdout,'7766045',[1,2,3,4]);assert accounting['status']=='pass'
    account_path=root/'accounting/7766045.json'
    if account_path.exists():assert read_json(account_path)==accounting
    else:write_json(account_path,accounting)
    verify_acceptance(root,'smoke');verify_acceptance(root,'pilot')
    before=audit(root,manifest_for(root));assert not before['integrity_errors']
    assert before['complete_groups']==541 and len(before['missing'])==27
    original_path=root/'plans/production.json'
    assert sha256(original_path)=='dbface536d3ecc31473eb75843ef29bd30502a10313d7fd5e7e65f439eecaa06'
    original=read_json(original_path);bundles=[];markers={};inputs={}
    old=Path('/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76')
    for p in (root/'results').glob('*/*/GROUP.COMPLETE.json'):
        d=read_json(p);markers[str(p.relative_to(root))]=sha256(p)
        name=p.parent.relative_to(root/'results')/'input.json'
        assert read_json(p.parent/'input.json')==read_json(old/'results'/name),str(name)
        inputs[str(name)]=sha256(p.parent/'input.json')
    completed_keys=set()
    for p in (root/'results').glob('*/*/GROUP.COMPLETE.json'):
        d=read_json(p);completed_keys.add((str(sorted(d['cell'].items())),d['repetition']))
    for task in range(1,5):
        p=root/'bundle-results'/('production-'+str(task)+'.json');d=read_json(p)
        assert d['plan_sha256']==sha256(original_path) and d['measurement_fingerprint']==identity
        log=root/'logs'/('fereg_current_sweep.o7766045.'+str(task))
        assert log.read_text().splitlines().count('SYSTEMATIC_BUNDLE_COMPLETE')==1
        bundles.append(d)
    plan=pending_plan(original,bundles,completed_keys);assert plan['planned_groups']==27
    path=root/'plans/production-continuation-20260929.json';assert not path.exists()
    write_json(path,plan)
    receipt=dict(original_accounting=accounting,audit=before,original_complete_markers=markers,
                 original_input_receipts=inputs,plan_sha256=sha256(path),measurement_fingerprint=identity,
                 preserved_group_settings=True,groups=27,applications=sum(len(g['limits']) for g in plan['bundles'][0]['groups']),
                 forecast_hours=plan['predicted_node_hours'],wall_seconds=10800)
    write_json(root/'reports/continuation-20260929-preflight.json',receipt)
    print({k:v for k,v in receipt.items() if k not in ['audit','original_complete_markers','original_input_receipts','original_accounting']})


if __name__=='__main__':main(Path(sys.argv[1]))
