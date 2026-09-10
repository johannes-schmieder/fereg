#!/usr/bin/env python3
"""ADR 0030 local numerical comparison; never changes the frozen F0 evaluator."""
from __future__ import annotations
import argparse
import json
import math
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import ols_arithmetic_development as dev

MANIFEST = ROOT / 'benchmarks/manifests/ols_wide_cholesky_development_v1.json'


def compare_science(a, b, *, unchanged=False):
    """Closed scientific inventory with explicit, bounded numerical exceptions."""
    sa, sb = a['scientific'], b['scientific']
    if unchanged:
        if sa != sb:
            raise dev.ScientificMismatch('unchanged route differs from baseline')
        return {'exact': True}
    numeric = {'coefficients', 'vce', 'fit', 'numeric_hex_sha256', 'certificates', 'solver'}
    if set(sa) != set(sb) or {k: v for k, v in sa.items() if k not in numeric} != {k: v for k, v in sb.items() if k not in numeric}:
        raise dev.ScientificMismatch('sample, coefficient status, DoF, output or scientific inventory differs')
    differences = {'exact': sa == sb}
    for field, atol, rtol in [('coefficients', 1e-10, 1e-7), ('vce', 1e-10, 1e-6), ('fit', 1e-9, 1e-7)]:
        x, y = sa[field], sb[field]
        if field == 'fit':
            if set(x) != {'fereg_fit'} or set(y) != {'fereg_fit'}:
                raise dev.ScientificMismatch('fit inventory differs')
            x, y = x['fereg_fit'], y['fereg_fit']
        if len(x) != len(y) or any(not math.isfinite(u) or not math.isfinite(v) or abs(u-v) > atol + rtol*max(abs(u), abs(v)) for u, v in zip(x, y)):
            raise dev.ScientificMismatch(f'{field} exceeds registered OLS envelope')
        differences[field + '_max_abs'] = max((abs(u-v) for u, v in zip(x, y)), default=0.0)
    ca, cb = sa['certificates'], sb['certificates']
    if ca.keys() != cb.keys() or ca['converged'] is not True or cb['converged'] is not True:
        raise dev.ScientificMismatch('certificate inventory/convergence differs')
    exceptions = {'fereg_certificate', 'solver_contract'}
    if ca['digests'].keys() != cb['digests'].keys() or {k: v for k, v in ca['digests'].items() if k not in exceptions} != {k: v for k, v in cb['digests'].items() if k not in exceptions}:
        raise dev.ScientificMismatch('non-numerical diagnostic identity differs')
    cert_rows = []
    solver_rows = []
    pilot_contractions = []
    for fit in (a, b):
        c = fit['scientific']['certificates']
        bounds = {'normal_equation_linf': 5e-10, 'fe_projection_linf': 1e-10,
                  'vce_symmetry_linf': 5e-10, 'reconstruction_linf': 5e-10}
        if c['values'].keys() != bounds.keys() or any(not math.isfinite(v) or not 0 <= v <= bounds[k] for k, v in c['values'].items()):
            raise dev.ScientificMismatch('absolute certificate bound failed')
        raw = dev.capture._matrix(fit['raw'], 'science', 'fereg_certificate')
        if (raw['rows'], raw['columns']) != (32, 2):
            raise dev.ScientificMismatch('raw certificate shape changed')
        values = dev.capture._finite_matrix(raw)
        if len(values) != 64 or not all(math.isfinite(v) for v in values):
            raise dev.ScientificMismatch('raw certificate is incomplete or nonfinite')
        defined, measures = values[::2], values[1::2]
        if any(measures[i] != 1 for i in (0, 1, 2, 23)) or not 0 <= measures[4] <= measures[3] or measures[5] <= 0:
            raise dev.ScientificMismatch('rank, score, bread or inference certificate failed')
        error_rows = list(range(6, 13)) + [14, 15, 16, 17, 18, 19, 21, 22]
        if any(not 0 <= measures[i] <= 5e-10 for i in error_rows) or measures[13] <= 0 or measures[20] < 0:
            raise dev.ScientificMismatch('raw independent certificate failed')
        cert_rows.append((defined, measures))
        solver = fit['scientific']['solver']
        if solver['source_columns'] != [1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 13, 14, 15, 16] or len(solver['values']) % 14:
            raise dev.ScientificMismatch('solver inventory differs')
        rows = [solver['values'][i:i+14] for i in range(0, len(solver['values']), 14)]
        if any(not math.isfinite(v) for row in rows for v in row) or any(not 0 <= row[i] <= bound for row in rows for i, bound in ((5, 1e-10), (6, 5e-10), (7, 5e-10))):
            raise dev.ScientificMismatch('solver certificate failed')
        # The final column is measured pilot contraction, not a solver choice.
        # Tiny changes in the freshly reconstructed residual can change it.
        if any(row[13] < 0 for row in rows):
            raise dev.ScientificMismatch('negative pilot contraction')
        pilot_contractions.append([row[13] for row in rows])
        solver_rows.append([row[:4] + row[8:13] for row in rows])
    if cert_rows[0][0] != cert_rows[1][0] or cert_rows[0][1][3] != cert_rows[1][1][3] or cert_rows[0][1][24:30] != cert_rows[1][1][24:30] or cert_rows[0][1][31] != 1 or cert_rows[1][1][31] != 1 or solver_rows[0] != solver_rows[1]:
        raise dev.ScientificMismatch('certificate policy or solver decisions differ')
    # Row 31 is the determinant of the published V, not a policy identifier.
    da, db = (row[1][30] for row in cert_rows)
    if abs(da-db) > 1e-10 + 1e-6*max(abs(da), abs(db)):
        raise dev.ScientificMismatch('covariance determinant exceeds VCE envelope')
    differences['covariance_determinant_max_abs'] = abs(da-db)
    differences['condition_proxy'] = [row[1][5] for row in cert_rows]
    differences['pilot_contractions'] = pilot_contractions
    return differences


def compare(args):
    prep = json.loads((args.work / 'preparation.json').read_text())
    if prep['manifest_sha256'] != dev.SHA(dev.MANIFEST) or prep['references'] != dev.reference_identity(args.reference):
        raise ValueError('preparation binding changed')
    selected = [s for s in dev.scopes() if s['id'] in (args.scope or [])]
    if not selected or {s['id'] for s in selected} != set(args.scope):
        raise ValueError('explicit known scopes required')
    out = args.work / args.label
    out.mkdir()
    packages = {}
    for arm, source in [('baseline', args.baseline_source), ('candidate', args.source)]:
        packages[arm] = dev.local.stage_package(source, source / 'dist/fereg_macos.plugin', out / ('package_' + arm))
        dev.write_json(out / ('identity_' + arm + '.json'), dict(files=dev.source_identity(source), plugin_sha256=dev.SHA(packages[arm] / 'fereg_macos.plugin'), controller_sha256=dev.SHA(Path(__file__)), manifest_sha256=dev.SHA(dev.MANIFEST)))
    records = []
    for scope in selected:
        refpath = args.work / 'references' / (scope['id'] + '.json')
        if not refpath.exists():
            raise ValueError('complete pinned reference must exist before timing pairs')
        reference = json.loads(refpath.read_text())
        for pair in range(5):
            order = ['baseline', 'candidate'] if pair % 2 == 0 else ['candidate', 'baseline']
            record = dict(scope=scope, pair=pair+1, order=order, fits={})
            for arm in order:
                fit = dev.capture_fit(out / f'{scope["id"]}_{pair}_{arm}', scope, packages[arm], prep['datasets'][scope['design']])
                dev.compare_reference(fit, reference)
                record['fits'][arm] = fit
            a, b = [record['fits'][arm] for arm in ('baseline', 'candidate')]
            record['scientific_comparison'] = compare_science(a, b, unchanged=scope['k'] <= 64 or scope['memory'] == 'low')
            record['ratio'] = b['warm_seconds'][-1] / a['warm_seconds'][-1]
            records.append(record)
            dev.write_json(out / 'results.json', dict(qualification_eligible=False, decision='ADR0030', records=records))
            print(f'{scope["id"]} pair {pair+1}: ratio={record["ratio"]:.6f}', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['prepare', 'run', 'compare', 'oracle'])
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--source', type=Path, default=ROOT)
    parser.add_argument('--baseline-source', type=Path)
    parser.add_argument('--reference', type=Path)
    parser.add_argument('--label')
    parser.add_argument('--scope', action='append')
    parser.add_argument('--trace', action='store_true')
    args = parser.parse_args()
    dev.MANIFEST = args.manifest.resolve()
    original_scopes = dev.scopes
    def scopes():
        result = original_scopes()
        for s in json.loads(dev.MANIFEST.read_text()).get('additional_scopes', []):
            s['id'] = f'{s["design"]}_k{s["k"]}_t{s["threads"]}_{s["memory"]}' + ('_' + s['guard'] if s['guard'] else '')
            result.append(s)
        return result
    dev.scopes = scopes
    if args.action == 'compare': compare(args)
    elif args.action == 'prepare': dev.prepare(args.work, args.reference)
    elif args.action == 'oracle': dev.oracle(args)
    else: dev.run(args)


if __name__ == '__main__':
    main()
