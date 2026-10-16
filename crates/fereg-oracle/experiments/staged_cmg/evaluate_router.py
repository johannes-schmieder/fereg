#!/usr/bin/env python3
"""Auditable prerequisite and descriptive selector reduction; never fills missing data."""
import argparse
import json
import math
from pathlib import Path
import statistics
import run_scc as runner

SCIENCE = ['digest','iterations','upgrades','certificates','relative_residual','graph_builds','coarse_n','tail_levels']


def read_rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def constructor_gate(output, admitted=False):
    arm_names = runner.ADMITTED_GATE_ARMS if admitted else runner.GUARDED_ARMS
    rows=read_rows(output/'observations.jsonl')
    preflight=read_rows(output/'preflight.jsonl')
    assert len(rows)==130 and len(preflight)==10,'incomplete constructor gate'
    expected={(case,n,rhs,kind,arm) for case,n,rhs,kind in runner.GUARDED_CELLS for arm in arm_names}
    key=lambda r:(r['case'],r['n'],r['rhs_count'],r['rhs_kind'],r['arm'])
    assert {key(r) for r in preflight}==expected and {key(r) for r in rows}==expected
    for identity in expected:
        observed=[r for r in rows if key(r)==identity]
        assert len(observed)==13
        assert sum(r['phase']=='warmup' and r['block']==-1 for r in observed)==1
        assert all(sum(r['phase']=='measure' and r['block']==block for r in observed)==2 for block in range(6))
    signatures={}
    for r in preflight+rows:
        key=(r['case'],r['n'],r['rhs_count'],r['rhs_kind'],r['arm'])
        sig=runner.signature(r['result'])
        assert signatures.setdefault(key,sig)==sig,('repeat mismatch',key)
    cells=[]
    for case,n,rhs,kind in runner.GUARDED_CELLS:
        data=[r for r in rows if (r['case'],r['n'],r['rhs_count'],r['rhs_kind'])==(case,n,rhs,kind)]
        assert len(data)==26
        failures=[r['result'] for r in data if r['result']['status']!='ok']
        cell=dict(case=case,n=n,rhs=rhs,kind=kind,failures=len(failures),failure_reasons=sorted({r.get('reason',r['status']) for r in failures}))
        if failures:
            assert not admitted, 'admitted constructor must succeed on every gate cell'
            assert all(r['arm']=='full-guarded' for r in data if r['result']['status']!='ok')
            assert case=='hubs' and len(failures)==13 and all('memory_refused' in r.get('reason','') for r in failures),'unexpected failure'
            cell.update(gate_required=False,passed=False)
        else:
            reference=next(r['result'] for r in data if r['arm']=='full')
            assert all([r['result'][k] for k in SCIENCE]==[reference[k] for k in SCIENCE] for r in data),('cross-arm science',case)
            paired={}
            for metric in ['seconds','setup_seconds']:
                scores=[]
                for block in range(6):
                    groups=[[r['result'][metric] for r in data if r['block']==block and r['arm']==arm] for arm in arm_names]
                    assert all(len(g)==2 for g in groups),'missing pair'
                    b,a=[math.exp(statistics.mean(math.log(x) for x in g)) for g in groups]
                    scores.append(dict(block=block,reference=b,candidate=a,ratio=a/b,excess=a-b-max(.5*b,.001)))
                paired[metric]=dict(blocks=scores,median_ratio=statistics.median(s['ratio'] for s in scores),median_excess=statistics.median(s['excess'] for s in scores))
            cell.update(gate_required=admitted or case!='hubs',paired=paired,passed=all(p['median_excess']<=0 for p in paired.values()))
        cells.append(cell)
    return dict(passed=all(c['passed'] for c in cells if c['gate_required']),observations=len(rows),preflights=len(preflight),cells=cells)


def selector_report(output, candidate="auto-commit", arm_names=None, manifest=None, prior="diagonal-full"):
    arm_names = arm_names or [*runner.ROUTER_ARMS[:-1], candidate]
    manifest = manifest or runner.ROUTER_CELLS
    rows = read_rows(output / 'observations.jsonl')
    preflight = read_rows(output / 'preflight.jsonl')
    assert len(rows) == 910 and len(preflight) == 70, 'incomplete selector panel'
    cells = []
    for case, n, rhs, kind in manifest:
        key = (case, n, rhs, kind)
        matches = lambda r: (r['case'], r['n'], r['rhs_count'], r['rhs_kind']) == key
        data = [r for r in rows if matches(r)]
        before = [r for r in preflight if matches(r)]
        assert len(data) == 65 and len(before) == 5, ('cell completeness', key)
        arms = {}
        for arm in arm_names:
            initial = [r for r in before if r['arm'] == arm]
            observed = [r for r in data if r['arm'] == arm]
            assert len(initial) == 1 and len(observed) == 13
            assert sum(r['phase'] == 'warmup' and r['block'] == -1 for r in observed) == 1
            results = [r['result'] for r in initial + observed]
            assert all(runner.signature(r) == runner.signature(results[0]) for r in results), ('repeat mismatch', key, arm)
            blocks = []
            for block in range(6):
                paired = [r['result'] for r in observed if r['phase'] == 'measure' and r['block'] == block]
                assert len(paired) == 2, ('missing pair', key, arm, block)
                assert all(r['status'] != 'ok' or (math.isfinite(r['seconds']) and r['seconds'] > 0 and 0 <= r['relative_residual'] <= 1e-8) for r in paired)
                blocks.append(math.sqrt(paired[0]['seconds'] * paired[1]['seconds']) if all(r['status'] == 'ok' for r in paired) else None)
            failures = [r for r in results if r['status'] != 'ok']
            arms[arm] = dict(failures=len(failures), reasons=sorted({r.get('reason', r['status']) for r in failures}),
                             block_seconds=blocks, signature=runner.signature(results[0]))
        easy = case in ['expander', 'fringe', 'hubs']
        comparator = 'forest' if case == 'fringe' else 'diagonal'
        if not easy:
            eligible = [a for a in ['diagonal', 'forest', 'full'] if not arms[a]['failures']]
            comparator = min(eligible, key=lambda a: statistics.median(arms[a]['block_seconds'])) if eligible else None
        paired = []
        old_ratios = []
        for block, cost in enumerate(arms[candidate]['block_seconds']):
            baseline = arms[comparator]['block_seconds'][block] if comparator else None
            old = arms[prior]['block_seconds'][block]
            if cost is not None and baseline is not None:
                allowance = max(.05 * baseline, .001) if easy else baseline
                paired.append(dict(block=block, reference=baseline, candidate=cost, ratio=cost/baseline, excess=cost-baseline-allowance))
            if cost is not None and old is not None:
                old_ratios.append(dict(block=block, ratio=cost/old))
        passed = bool(comparator and not arms[comparator]['failures'] and not arms[candidate]['failures'] and len(paired) == 6 and all(r['excess'] <= 0 for r in paired))
        cells.append(dict(case=case, n=n, rhs=rhs, kind=kind, easy=easy, comparator=comparator,
                          arms=arms, paired=paired, old_trigger_ratios=old_ratios, passed=passed))
    return dict(scope='descriptive SCC prepared-Schur discovery; no independent-session confidence or release qualification',
                observations=len(rows), preflights=len(preflight), passed=all(c['passed'] for c in cells), cells=cells)


def main():
    p=argparse.ArgumentParser();p.add_argument('mode',choices=['gate', 'selector']);p.add_argument('output',type=Path)
    args=p.parse_args();result=(constructor_gate if args.mode == 'gate' else selector_report)(args.output)
    print(json.dumps(result,indent=2,allow_nan=False))
    if not result['passed']:raise SystemExit(1)


if __name__=='__main__':main()
