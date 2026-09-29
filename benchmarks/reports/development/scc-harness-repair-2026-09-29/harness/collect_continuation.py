"""Accept completion coverage and render through the frozen benchmark controller."""
import json
from pathlib import Path
import subprocess
import sys


def main(root):
    sys.path.insert(0,str(root/'controller'))
    from accounting import parse
    from common import read_json,write_json,sha256,manifest_for
    from provenance import verify
    from campaign import audit
    from light_report import render
    before=read_json(root/'reports/continuation-20260929-preflight.json')
    assert verify(root)==before['measurement_fingerprint']
    job='7778971'
    raw=subprocess.run(['qacct','-j',job],capture_output=True,text=True,check=True,timeout=120)
    account=parse(raw.stdout,job,[1]);assert account['status']=='pass'
    write_json(root/'accounting'/f'{job}.json',account)
    plan_path=root/'plans/production-continuation-20260929.json'
    assert sha256(plan_path)==before['plan_sha256']
    plan=read_json(plan_path)
    result=read_json(root/'bundle-results/production-continuation-20260929-1.json')
    assert result['status']=='complete' and not result['pending']
    assert result['completed']==plan['bundles'][0]['groups']
    assert result['plan_sha256']==before['plan_sha256'] and result['measurement_fingerprint']==before['measurement_fingerprint']
    log=root/'logs'/('fereg_pending.o'+job)
    assert log.read_text().splitlines().count('SYSTEMATIC_BUNDLE_COMPLETE')==1
    for name,digest in before['original_complete_markers'].items():assert sha256(root/name)==digest,name
    old=Path('/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76')
    inputs={}
    for p in (root/'results').glob('*/*/input.json'):
        name=p.relative_to(root/'results')
        assert read_json(p)==read_json(old/'results'/name),str(name)
        inputs[str(name)]=sha256(p)
    complete=audit(root,manifest_for(root))
    assert complete['status']=='complete' and complete['complete_groups']==568 and not complete['integrity_errors']
    report=root/'reports/completed-20260929';assert not report.exists()
    render(root,report)
    write_json(report/'continuation-acceptance.json',dict(job_id=job,accounting=account,
        plan_sha256=sha256(plan_path),measurement_fingerprint=before['measurement_fingerprint'],
        original_541_markers_unchanged=True,all_568_inputs_match_original=True,
        input_receipts=inputs,bundle_result_sha256=sha256(root/'bundle-results/production-continuation-20260929-1.json'),
        terminal_log_sha256=sha256(log),scope='Development coverage completion; scientific discrepancies remain unranked'))
    print(json.dumps(dict(status='complete',groups=568,failures=len(complete['failures']),comparisons=complete['comparison_status'])))


if __name__=='__main__':main(Path(sys.argv[1]))
