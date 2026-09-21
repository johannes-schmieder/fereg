"""Inspect stopped receipts, data geometry and frozen failure paths; never fit."""
import csv
import json
from collections import Counter
from pathlib import Path

import numpy as np
import pandas as pd

from accounting import parse
from audit_existing import covariance
from common import read_json, write_json, sha256
from outcomes import audited_outcome

BASE = Path(__file__).resolve().parents[1]
OUT = BASE / 'results/scc/stopped-analysis'


def main():
    stop = read_json(BASE / 'results/scc/campaign-stopped.json')
    snap = Path(stop['snapshot'])
    audit = read_json(OUT / 'audit.json')
    accounting = parse((snap / 'accounting.txt').read_text(), '7529406', range(1, 13))
    assert not accounting['missing_tasks']
    assert [(r['task_id'], r['failed'], r['exit_status']) for r in accounting['records'] if r['failed']] == [(11, 100, 137), (12, 100, 137)]
    assert '7529406' not in (snap / 'qstat.txt').read_text()
    accounts = {r['task_id']: r for r in accounting['records']}
    auth = read_json(BASE / 'results/scc/data-authorization.json')
    rows = []
    for task in sorted((snap / 'results/campaign').glob('task-*')):
        task_id = int(task.name[-2:])
        for p in sorted(task.glob('*-r*-*.json')):
            r = audited_outcome(read_json(p))
            assert r['measurement_fingerprint'] == audit['fingerprint']
            assert r['job_id'] == '7529406' and int(r['task_id']) == task_id
            assert r['input_sha256'] == auth['files'][f"veneto-{r['cell']['model']}.dta"]['sha256']
            assert r['cell']['threads'] == [1, 2, 4, 8, 14, 28][(task_id - 1) % 6]
            assert r['repetition'] == (task_id - 1) // 6
            assert r['host'].split('.')[0] == accounts[task_id]['hostname'].split('.')[0]
            assert len(r['cpu_affinity']) == r['cell']['threads']
            if r['status'] != 'command_timeout':
                assert r['sentinel'] and r['process_returncode'] == 0
                assert (snap / 'work' / task.name / p.stem / 'DONE').exists()
            r['accepted_complete_task'] = task_id in audit['completed_tasks']
            r['receipt'] = str(p.relative_to(snap))
            rows.append(r)
    assert len(rows) == 124
    assert sum(r['accepted_complete_task'] for r in rows) == 110
    for name in ['task-11/ppml-t14-r1-ppmlhdfe', 'task-12/ppml-t28-r1-gglm']:
        folder = snap / 'work' / name
        assert (folder / 'START').exists() and not (folder / 'DONE').exists()
    supplemental = [r for r in rows if not r['accepted_complete_task']]
    checks = []
    for r in supplemental:
        if r['cell']['model'] != 'ols':
            continue
        ref = next(q for q in supplemental if q['task_id'] == r['task_id'] and q['arm'] == 'reghdfe')
        aa = pd.DataFrame(r['coefficients']).set_index('name').sort_index()
        bb = pd.DataFrame(ref['coefficients']).set_index('name').sort_index()
        folder = snap / 'work' / f"task-{int(r['task_id']):02d}"
        stem = f"ols-t{r['cell']['threads']}-r1-"
        v = covariance(folder / (stem + r['arm']) / 'covariance.csv')
        v0 = covariance(folder / (stem + 'reghdfe') / 'covariance.csv')
        checks.append({'task': int(r['task_id']), 'arm': r['arm'], 'status': r['status'],
                       'coefficient_pass': bool(np.allclose(aa.value, bb.value, rtol=1e-7, atol=1e-10)),
                       'SE_pass': bool(np.allclose(aa.se, bb.se, rtol=1e-6, atol=1e-10)),
                       'covariance_pass': bool(np.allclose(v, v0, rtol=1e-6, atol=1e-10)),
                       'sample_exact': r.get('sample_sha256') == ref.get('sample_sha256') if r.get('sample_sha256') else None})
    data = BASE / 'work/ppml.dta'
    assert sha256(data) == auth['files']['veneto-ppml.dta']['sha256']
    d = pd.read_stata(data, columns=['g1', 'g2', 'y'], convert_categoricals=False)
    assert len(d) == 3159720 and np.isfinite(d.y).all() and (d.y > 0).all()
    distinct = d[['g1', 'g2']].drop_duplicates().groupby('g1').size()
    geometry = {'positive_outcomes': len(d), 'zero_outcomes': int((d.y == 0).sum()),
                'outcome_quantiles': {str(q): float(d.y.quantile(q)) for q in [0, .01, .5, .99, 1]},
                'outcome_max_min_ratio': float(d.y.max() / d.y.min()),
                'workers_observed_at_multiple_firms': int((distinct > 1).sum()),
                'worker_mobility_fraction': float((distinct > 1).mean()),
                'unique_worker_firm_pairs': int(distinct.sum())}
    frozen = read_json(BASE / 'work/scc-stage/frozen-source.json')['runtime']
    source = BASE / 'work/failure-source-d44e36b'
    source_evidence = []
    for path, lines, role in [
        ('crates/fereg-core/src/error.rs', '37-39', '430 numerical nonconvergence; 909 allocation; 1 interruption'),
        ('crates/fereg-core/src/ppml/prepared_irls.rs', '1001-1007,1640-1674,1894-1904,2480-2485', 'several independent r430 paths; strict rejection can resume'),
        ('crates/fereg-core/src/ppml/categorical_lsmr.rs', '651-718,201-220', 'independent recurrence and normal-equation certificates; optional trace'),
        ('stata/_fereg_estimate_ppml.ado', '37-44', 'default solver auto')]:
        source_evidence.append({'revision': 'd44e36bfa5ad7e41a06752d1008f0b9683f8c8dd', 'path': path,
                                'sha256': sha256(source / path), 'lines': lines, 'role': role})
    pyroot = BASE / 'work/python/lib/python3.12/site-packages/pyfixest'
    for path, lines, role in [
        ('estimation/internals/demean_.py', '70-79', 'exception raised for failed inner demeaning'),
        ('estimation/internals/fit_glm_.py', '206-242', 'weighted FE projection within each outer IRLS iteration'),
        ('estimation/models/feglm_.py', '182-204', 'IRLS entry and demeaning callback'),
        ('estimation/api/fepois.py', '30-31,125-128', 'outer 25 iterations distinct from inner cap'),
        ('demeaners.py', '104,112-135', 'default Rust MAP, initial tol 1e-6, inner cap 10000')]:
        h = sha256(pyroot / path)
        remote = 'env/python/lib/python3.12/site-packages/pyfixest/' + path
        assert frozen[remote] == h
        source_evidence.append({'path': remote, 'sha256': h, 'lines': lines, 'role': role})
    main = [r for r in rows if r['accepted_complete_task']]
    ppml = [r for r in rows if r['cell']['model'] == 'ppml']
    fe = [r for r in ppml if r['arm'] == 'feppml']
    py = [r for r in ppml if r['arm'] == 'pyfixest']
    assert all(r['metadata'].get('rc') == 430 for r in fe if r['status'] != 'pass')
    assert all(r['metadata']['error'] == 'ValueError: Demeaning failed after 10000 iterations.' for r in py if r['status'] == 'worker_failure')
    write_json(OUT / 'failure-audit.json', {
        'scope': 'Existing evidence only; no new estimator runs or solver changes',
        'completed_task_calls': len(main), 'supplemental_calls': len(supplemental),
        'interrupted_calls': ['task-11/ppml-t14-r1-ppmlhdfe', 'task-12/ppml-t28-r1-gglm'],
        'not_started_tasks': list(range(13, 25)), 'data_geometry': geometry,
        'source_evidence': source_evidence, 'supplemental_checks': checks,
        'accounted_production_node_hours': sum(r['wall_seconds'] for r in accounting['records']) / 3600,
        'complete_task_node_hours': sum(r['wall_seconds'] for r in accounting['records'] if r['task_id'] <= 10) / 3600,
        'unsuccessful_pyfixest_gglm_command_share': (audit['command_budget']['ppml/gglm'] + audit['command_budget']['ppml/pyfixest']) / sum(audit['command_budget'].values()),
        'feppml_all_finished_statuses': dict(Counter(r['status'] for r in fe)),
        'ppml_rows': ppml})
    write_json(OUT / 'stopped-accounting.json', accounting)
    write_json(OUT / 'all-finished-calls.json', rows)
    with (OUT / 'all-finished-calls.csv').open('w') as f:
        w = csv.writer(f)
        w.writerow(['task', 'complete_task', 'model', 'command', 'cores', 'phase', 'status', 'seconds', 'timeout_lower_bound', 'peak_rss_gib'])
        for r in rows:
            w.writerow([r['task_id'], r['accepted_complete_task'], r['cell']['model'], r['arm'], r['cell']['threads'], r['phase'], r['status'], r['seconds'], r['command_time_lower_bound'], r['peak_rss_gib']])
    print(json.dumps({'geometry': geometry, 'calls': len(rows), 'supplemental_fereg': [c for c in checks if c['arm'] == 'fereg']}, indent=2))


if __name__ == '__main__':
    main()
