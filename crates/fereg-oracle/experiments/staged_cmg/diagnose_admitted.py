#!/usr/bin/env python3
"""One frozen admitted-constructor diagnostic, then a budgeted routing panel."""
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
    pre = evaluate.read_rows(root/'admitted-pilot/preflight.jsonl')
    rows = evaluate.read_rows(root/'admitted-pilot/observations.jsonl')
    def key(r):
        return r['case'],r['n'],r['rhs_count'],r['rhs_kind'],r['arm']
    expected={(case,n,rhs,kind,arm) for case,n,rhs,kind in evaluate.runner.ROUTER_CELLS for arm in evaluate.runner.ADMITTED_ARMS}
    assert len(pre)==len(rows)==len(expected)==70
    assert {key(r) for r in pre}=={key(r) for r in rows}==expected
    assert all(r['phase']=='warmup' and r['block']==-1 for r in rows)
    prior={key(r):r['result']['process_seconds'] for r in pre}
    budget = 60 * math.ceil((120 + 1.5*14*sum(max(prior[key(r)],r['result']['process_seconds']) for r in rows))/60)
    (root/'admitted-pilot/budget.json').write_text(json.dumps(dict(budget_seconds=budget))+'\n')
    assert budget <= 1800, 'pilot predicts panel beyond frozen budget cap'
    stage('measure', 'admitted-panel', 'admitted-router', budget)
    report = evaluate.selector_report(root/'admitted-panel', candidate='auto-admitted')
    (root/'admitted-panel/verdict.json').write_text(json.dumps(report,indent=2)+'\n')
    print('ADMITTED_DIAGNOSTIC_COMPLETE selector_targets='+str(report['passed']),flush=True)


if __name__ == '__main__':
    run(Path(sys.argv[1]).resolve())
