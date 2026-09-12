"""Portable numeric receipts for the bounded PPML preparation experiment."""
import argparse
import csv
import statistics
import zipfile
from pathlib import Path

from common import read_json, sha256, write_json
from ppml_preparation_compare import PRIMARY


def collect(root, output, *, prefix='ppmlprep', science_file='exact-science.json', exact=True):
    output.mkdir(parents=True, exist_ok=True)
    pairs, groups, included = [], {}, set()
    for candidate in sorted((root / 'runs').glob(f'{prefix}-*-candidate')):
        if not (candidate / science_file).exists():
            continue
        paths = dict(candidate=candidate, baseline=candidate.with_name(candidate.name.replace('-candidate', '-baseline')))
        runtimes = read_json(candidate / 'paired-runtime.json')
        record = dict(pair=candidate.name.removesuffix('-candidate'), science_pass=True, exact_science=exact)
        for arm, path in paths.items():
            result = read_json(path / 'result.json')
            assert result['status'] == 'pass' and result['sentinel'] and result['metadata']['rc'] == 0
            native = {key: None if str(value).strip() == '.' or value is None else float(value)
                      for key, value in runtimes[arm].items()}
            record[arm] = dict(command_seconds=result['seconds'], plugin_seconds=native['total_seconds'],
                outer_seconds=result['seconds'] - native['total_seconds'], preparation_seconds=native['time_preparation'],
                inference_seconds=native['time_inference'], native_peak_bytes=native['allocation_peak'],
                modeled_bytes=native['modeled_selected_bytes'], observed_preparation_peak=native['observed_peak_prep'],
                modeled_preparation_peak=native['modeled_peak_prep'], rss_gib=result['peak_rss_gib'],
                outer_iterations=native['outer_iterations'], inner_iterations=native['inner_iterations'],
                step_halvings=native['step_halvings'], fallback_count=native['fallback_count'])
            for key in ('time_weight_response_refresh', 'time_fe_projection', 'time_finite_solve',
                        'time_strict_final_solve', 'time_fe_recovery', 'strict_profile_rejections',
                        'observed_peak_final_solve', 'modeled_peak_final_solve'):
                record[arm][key] = native[key]
            included.add(path)
        record['case'] = result['cell']['name']
        record['group'] = ('pilot' if '-pilot-' in record['pair'] else 'profile' if '-trace-' in record['pair']
            else 'recheck' if '-recheck-' in record['pair'] else 'primary' if record['case'] in {s['name'] for s in PRIMARY} else 'control')
        record['ratio'] = record['candidate']['command_seconds'] / record['baseline']['command_seconds']
        groups.setdefault((record['group'], record['case']), []).append(record)
        pairs.append(record)
    summary = []
    for (group, case), records in sorted(groups.items()):
        row = dict(group=group, case=case, pairs=len(records))
        for arm in ('baseline', 'candidate'):
            for key in records[0][arm]:
                values = [record[arm][key] for record in records if record[arm][key] is not None]
                row[f'{arm}_{key}'] = statistics.median(values) if values else None
        row['median_paired_change_percent'] = 100 * (statistics.median(r['ratio'] for r in records) - 1)
        summary.append(row)
    write_json(output / 'comparisons.json', dict(pairs=pairs, summary=summary,
        note='Primary: public full-command times, three seeds and rotated order. Pilot excluded. Profile: private allocationaudit plus Stata profiling, separately reported. Medians need not sum. No universal or qualification claim.'))
    with (output / 'summary.csv').open('w') as f:
        writer = csv.DictWriter(f, fieldnames=list(summary[0])); writer.writeheader(); writer.writerows(summary)
    with zipfile.ZipFile(output / 'numeric-evidence.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(included):
            for file in sorted(path.iterdir()):
                if file.suffix == '.csv' or file.name in {'result.json', science_file, 'paired-runtime.json', 'import-hashes.json', 'stata_worker.do'}:
                    archive.write(file, f'runs/{path.name}/{file.name}')
        for file in sorted((root / 'ppml-data').glob('*.json')):
            archive.write(file, f'inputs/{file.name}')
    write_json(output / 'archive.json', dict(sha256=sha256(output / 'numeric-evidence.zip'),
        completed_pairs=len(pairs), measured_calls=len(pairs)*2, all_pairs_science_pass=True, all_pairs_exact_science=exact,
        primary_pairs=sum(r['group'] == 'primary' for r in pairs),
        contents='Portable synthetic input hashes/recipes; full b/V and diagnostic tables; sample hashes; action and phase timings, allocations and sampled RSS; saved outputs; per-run worker copies where available. No licensed logs, binaries, request paths or private data.'))
    for row in summary:
        print(row['group'], row['case'], row['baseline_command_seconds'], row['candidate_command_seconds'], round(row['median_paired_change_percent'], 1))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('root', type=Path); parser.add_argument('output', type=Path)
    args = parser.parse_args(); collect(args.root, args.output)
