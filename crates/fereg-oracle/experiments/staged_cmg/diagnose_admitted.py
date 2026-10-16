#!/usr/bin/env python3
"""One frozen admitted-constructor diagnostic, then a budgeted routing panel."""
import hashlib
import os
import json
import math
from pathlib import Path
import subprocess
import sys
import evaluate_router as evaluate


def run(root):
    source = Path(__file__).resolve().parent
    binary = root / 'build/release/staged-cmg-prototype'
    smoke = evaluate.read_rows(root/'smoke/smoke.jsonl')
    weak = evaluate.read_rows(root/'smoke/cluster-diagnostic.jsonl')
    assert len(smoke)==25 and all(r['result']['status']=='ok' for r in smoke)
    assert len(weak)==5 and {r['arm'] for r in weak}==set(evaluate.runner.ADMITTED_ARMS)
    assert all(not (r['result']['status']=='failed' and 'stdout' in r['result']) for r in weak)
    def stage(mode, name, protocol, budget=None):
        argv = [sys.executable, str(source/'run_scc.py'), mode, '--binary', str(binary),
                '--output', str(root/name), '--protocol', protocol]
        if budget is not None:
            argv += ['--budget-seconds', str(budget)]
        subprocess.run(argv, check=True)
    stage('measure', 'admitted-gate', 'admitted-gate')
    verdict = evaluate.constructor_gate(root/'admitted-gate', admitted=True)
    (root/'admitted-gate/verdict.json').write_text(json.dumps(verdict, indent=2)+'\n')
    assert verdict['passed'], 'admitted constructor prerequisite failed; no selector panel'
    stage('pilot', 'admitted-pilot', 'admitted-router', 300)
    budget = pilot_budget(root/'admitted-pilot', evaluate.runner.ROUTER_CELLS, evaluate.runner.ADMITTED_ARMS)
    stage('measure', 'admitted-panel', 'admitted-router', budget)
    report = evaluate.selector_report(root/'admitted-panel', candidate='auto-admitted')
    (root/'admitted-panel/verdict.json').write_text(json.dumps(report,indent=2)+'\n')
    print('ADMITTED_DIAGNOSTIC_COMPLETE selector_targets='+str(report['passed']),flush=True)


def pilot_budget(output, cells, arms):
    pre = evaluate.read_rows(output/'preflight.jsonl')
    rows = evaluate.read_rows(output/'observations.jsonl')
    def key(r):
        return r['case'],r['n'],r['rhs_count'],r['rhs_kind'],r['arm']
    expected={(case,n,rhs,kind,arm) for case,n,rhs,kind in cells for arm in arms}
    assert len(pre)==len(rows)==len(expected)==70
    assert {key(r) for r in pre}=={key(r) for r in rows}==expected
    assert all(r['phase']=='warmup' and r['block']==-1 for r in rows)
    prior={key(r):r['result']['process_seconds'] for r in pre}
    budget = 60 * math.ceil((120 + 1.5*14*sum(max(prior[key(r)],r['result']['process_seconds']) for r in rows))/60)
    (output/'budget.json').write_text(json.dumps(dict(budget_seconds=budget))+'\n')
    assert budget <= 1800, 'pilot predicts panel beyond frozen budget cap'
    return budget


def forecast(root):
    reference=Path(os.environ['STAGED_ADMITTED_REFERENCE'])
    frozen=hashlib.sha256((root/'build/release/staged-cmg-prototype').read_bytes()).hexdigest()
    assert (root/'logs/binary.sha256').read_text().split()[0]==frozen
    old_smoke=evaluate.read_rows(reference/'smoke/smoke.jsonl')
    old_weak=evaluate.read_rows(reference/'smoke/cluster-diagnostic.jsonl')
    source=Path(__file__).resolve().parent
    smoke=evaluate.read_rows(root/'smoke/smoke.jsonl')
    weak=evaluate.read_rows(root/'smoke/cluster-diagnostic.jsonl')
    assert len(smoke)==25 and all(r['result']['status']=='ok' for r in smoke)
    assert len(weak)==5 and {r['arm'] for r in weak}==set(evaluate.runner.FORECAST_ARMS)
    assert all(not (r['result']['status']=='failed' and 'stdout' in r['result']) for r in weak)
    for rows,previous in [(smoke,old_smoke),(weak,old_weak)]:
        key=lambda r:(r.get('case'),r['arm'])
        old={key(r):evaluate.runner.signature(r['result']) for r in previous}
        overlap=[r for r in rows if key(r) in old]
        assert len(overlap)==(20 if rows is smoke else 4)
        assert all(evaluate.runner.signature(r['result'])==old[key(r)] for r in overlap)
    for protocol in ['forecast-discovery','forecast-confirmation']:
        arms,cells,_,_=evaluate.runner.protocol_config(protocol)
        def stage(mode, output, budget):
            subprocess.run([sys.executable,str(source/'run_scc.py'),mode,'--binary',
                str(root/'build/release/staged-cmg-prototype'),'--output',str(output),
                '--protocol',protocol,'--budget-seconds',str(budget)],check=True)
            assert hashlib.sha256((root/'build/release/staged-cmg-prototype').read_bytes()).hexdigest()==frozen
            assert json.loads((output/'host.json').read_text())['binary_sha256']==frozen
        pilot=root/(protocol+'-pilot')
        stage('pilot',pilot,300)
        if protocol=='forecast-discovery':
            key=lambda r:(r['case'],r['n'],r['rhs_count'],r['rhs_kind'],r['arm'])
            previous=evaluate.read_rows(reference/'admitted-panel/preflight.jsonl')
            old={key(r):evaluate.runner.signature(r['result']) for r in previous}
            observed=evaluate.read_rows(pilot/'preflight.jsonl')
            overlap=[r for r in observed if key(r) in old]
            assert len(overlap)==56
            assert all(evaluate.runner.signature(r['result'])==old[key(r)] for r in overlap)
        budget=pilot_budget(pilot,cells,arms)
        output=root/protocol
        stage('measure',output,budget)
        report=evaluate.selector_report(output,'auto-forecast',arms,cells,'auto-admitted')
        report['comparison_policy']='auto-admitted'
        (output/'verdict.json').write_text(json.dumps(report,indent=2)+'\n')
        print(protocol+' targets='+str(report['passed']),flush=True)
    print('FORECAST_PROTOTYPE_COMPLETE',flush=True)


if __name__ == '__main__':
    (forecast if len(sys.argv)>2 and sys.argv[2]=='forecast' else run)(Path(sys.argv[1]).resolve())
