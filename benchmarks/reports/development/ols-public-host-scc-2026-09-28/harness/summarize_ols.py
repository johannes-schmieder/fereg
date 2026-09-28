"""Summarize captured OLS telemetry and diagnose DoF scaling; never fit or rank."""
from pathlib import Path
import csv, hashlib, json, statistics, sys
raw, output = map(Path, sys.argv[1:])
plan = json.loads((raw / 'plan.json').read_text())

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def dist(values):
    return dict(values=values, min=min(values), median=statistics.median(values), max=max(values))

def covariance(path):
    with path.open() as stream:
        return {(r['row'], r['column']): float(r['value']) for r in csv.DictReader(stream)}

phases, equality, diagnostics, bindings = [], [], [], {}
for case in plan['cases']:
    if case['cell']['model'] != 'ols':
        continue
    arms = {a: [] for a in ['baseline', 'candidate']}
    for rep in range(1, len(case['seeds']) + 1):
        root = raw / case['id'] / f'pair-{rep}'
        results = {}
        for arm in ['baseline', 'candidate', 'stock']:
            p = root / arm / 'result.json'
            bindings[str(p.relative_to(raw))] = digest(p)
            results[arm] = json.loads(p.read_text())
        for arm in arms:
            p = root / arm / 'fereg_runtime.csv'
            bindings[str(p.relative_to(raw))] = digest(p)
            with p.open() as stream:
                values = {(int(r['row']), int(r['column'])): float(r['value']) for r in csv.DictReader(stream)}
            record = {'public_seconds': results[arm]['seconds']}
            for row, key in [(18, 'preflight_seconds'), (19, 'input_load_seconds'), (20, 'native_compute_seconds'), (21, 'native_total_before_store_seconds'), (12, 'allocation_peak_bytes'), (25, 'selected_phase2_bytes')]:
                assert values[row, 1] == 1
                record[key] = values[row, 2]
            arms[arm].append(record)
        a, b = results['baseline']['metadata'], results['candidate']['metadata']
        fields = ['df_r', 'df_a', 'rank', 'fereg_threads', 'fereg_requested_threads', 'fereg_memory_selected_bytes', 'fereg_native_allocation_bytes', 'fereg_scratch_bytes']
        equality.append(dict(case=case['id'], repetition=rep, telemetry={k: dict(baseline=a[k], candidate=b[k]) for k in fields if k in a and k in b}))
        comparison = json.loads((root / 'comparison.json').read_text())['comparisons']['stock']
        if comparison['status'] == 'pass':
            continue
        if results['candidate']['status'] != 'pass' or results['stock']['status'] != 'pass':
            diagnostics.append(dict(case=case['id'], repetition=rep, status='application_failure', comparison=comparison))
            continue
        native, stock = results['candidate']['metadata'], results['stock']['metadata']
        # This is a declared-policy diagnostic only. It never changes exports,
        # scientific eligibility, tolerances or headline timing comparisons.
        n, df_native = native['N'], native['df_r']
        df_stock = n - stock['df_k']
        assert df_native > 0 and df_stock > 0
        factor = df_native / df_stock
        paths = [root / arm / 'covariance.csv' for arm in ['candidate', 'stock']]
        for p in paths:
            bindings[str(p.relative_to(raw))] = digest(p)
        u, v = map(covariance, paths)
        keys = u.keys() & v.keys()
        assert keys
        diagnostics.append(dict(case=case['id'], repetition=rep, status='unranked_policy_diagnostic', native_df_r=df_native, stock_df_k=stock['df_k'], stock_implied_df_r=df_stock, expected_stock_native_variance_factor=factor, max_covariance_gap=max(abs(u[k]-v[k]) for k in keys), max_gap_after_dof_factor=max(abs(u[k]-v[k]/factor) for k in keys), max_bread_gap=max(abs(u[k]*df_native/native['rss']-v[k]*df_stock/stock['rss']) for k in keys), comparison=comparison))
    phases.append(dict(case=case['id'], arms={a: {k: dist([v[k] for v in records]) for k in records[0]} for a, records in arms.items()}))
output.write_text(json.dumps(dict(scope='Read-only telemetry from existing fits. Native compute includes post-load validation and science; total stops before SPI stores. DoF normalization is a diagnostic, not altered eligibility or an independent rank oracle.', phases=phases, equality=equality, dof_diagnostics=diagnostics, bindings=bindings), indent=2)+'\n')
