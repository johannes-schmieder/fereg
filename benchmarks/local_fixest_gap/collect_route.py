"""Preserve local routing evidence without data, licensed logs, or host paths."""
import argparse
import json
import zipfile
from pathlib import Path

import pandas as pd
from common import read_json, write_json
from final_compare import certify


def matrix(path, name):
    return pd.read_csv(path / (name + '.csv')).pivot(index='row', columns='column', values='value')


def collect(root, output):
    output.mkdir(parents=True, exist_ok=True)
    paths = sorted(p.parent for p in (root / 'runs').glob('*/result.json'))
    rows, phases, native, comparisons = [], {}, {}, {}
    for path in paths:
        result = read_json(path / 'result.json')
        request = read_json(path / 'request.json')
        cell, meta = result['cell'], result['metadata']
        row = {'run': path.name, 'arm': result['arm'], 'design': cell['design'],
               'n': cell['n'], 'k': cell['k'], 'threads': cell['threads'],
               'trace': request['trace'], 'status': result['status'],
               'seconds': result['seconds'], 'peak_rss_gib': result['peak_rss_gib'],
               'input_sha256': result['input_sha256'],
               'load_average_1m': result['system_load_average_at_launch'][0]}
        if result['arm'] == 'fereg' and result['status'] == 'pass':
            native[path.name] = certify(path)
            runtime = matrix(path, 'fereg_runtime')
            solver = matrix(path, 'fereg_solver')
            row.update({'input_load_seconds': runtime.loc[19, 2],
                        'native_seconds': runtime.loc[20, 2],
                        'outer_seconds': result['seconds'] - runtime.loc[21, 2],
                        'preflight_seconds': runtime.loc[18, 2],
                        'native_reported_allocation_bytes': meta['fereg_native_allocation_bytes'],
                        'modeled_fast_bytes': meta['fereg_memory_fast_bytes'],
                        'initial_used_solver_code': solver.loc[1, 2],
                        'initial_iterations': solver.loc[1, 3],
                        'pilot_iterations': solver.loc[1, 15],
                        'pilot_contraction': solver.loc[1, 16],
                        'initial_fallback': solver.loc[1, 12]})
        rows.append(row)
        checks = {}
        for name in ['baseline-comparison.json', 'fixest-comparison.json', 'reference-disagreement.json']:
            if (path / name).exists():
                checks[name] = read_json(path / name)
        if checks:
            comparisons[path.name] = checks
        lines = []
        for f in [path / 'stderr.txt', *path.glob('*.log')]:
            if f.exists():
                lines.extend(line for line in f.read_text(errors='replace').splitlines()
                             if line.startswith('FEREG_PHASE|'))
        if lines:
            phases[path.name] = list(dict.fromkeys(lines))
    frame = pd.DataFrame(rows)
    frame.to_csv(output / 'measurements.csv', index=False)
    primary = frame[frame.run.str.match(r'^final-(baseline|candidate|fixest)-')].copy()
    primary['variant'] = primary.run.str.split('-').str[1]
    summary = primary.groupby(['design', 'threads', 'variant']).agg(
        runs=('seconds', 'size'), median_seconds=('seconds', 'median'),
        min_seconds=('seconds', 'min'), max_seconds=('seconds', 'max'),
        median_peak_rss_gib=('peak_rss_gib', 'median')).reset_index()
    summary.to_csv(output / 'primary-summary.csv', index=False)
    parts = primary[primary.arm == 'fereg'].groupby(['design', 'threads', 'variant'])[
        ['seconds', 'input_load_seconds', 'native_seconds', 'outer_seconds', 'preflight_seconds']].median()
    parts.to_csv(output / 'command-parts.csv')
    write_json(output / 'phase-timings.json', phases)
    science = {'schema': 'ols-schur-local-science-v1', 'native_certificates': native,
               'recorded_comparisons': comparisons,
               'worker_statuses': frame.status.value_counts().to_dict(),
               'completed_invocations': len(paths), 'final_invocations': int(frame.run.str.startswith('final-').sum()),
               'primary_invocations': len(primary), 'primary_groups': len(primary) // 3,
               'note': 'Reference disagreements remain failures of native parity; see independent adjudications.'}
    write_json(output / 'science.json', science)
    allowed = ['result.json', 'import-hashes.json', 'coefficients.csv', 'covariance.csv',
               'b.csv', 'V.csv', 'fereg_solver.csv', 'fereg_runtime.csv', 'fereg_certificate.csv',
               'fereg_fit.csv', 'fereg_status.csv', 'fereg_dof.csv', 'native-science.json',
               'baseline-comparison.json', 'fixest-comparison.json', 'reference-disagreement.json']
    with zipfile.ZipFile(output / 'evidence.zip', 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in paths:
            for name in allowed:
                source = path / name
                if source.exists():
                    info = zipfile.ZipInfo(path.name + '/' + name, date_time=(2026, 9, 12, 0, 0, 0))
                    info.compress_type = zipfile.ZIP_DEFLATED
                    archive.writestr(info, source.read_bytes())
        for source in sorted((root / 'data').glob('*.json')):
            info = zipfile.ZipInfo('input-identities/' + source.name, date_time=(2026, 9, 12, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, source.read_bytes())
    print(json.dumps({k: science[k] for k in ['worker_statuses', 'completed_invocations', 'final_invocations', 'primary_invocations', 'primary_groups']}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    collect(args.root, args.output)
