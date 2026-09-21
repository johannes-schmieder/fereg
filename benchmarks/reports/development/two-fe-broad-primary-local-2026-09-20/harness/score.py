"""Score every planned broad-development cell, retaining missing/failed attempts.

This is a mechanical timing/identity audit. It never adjudicates scientific
reference disagreements or declares the broader milestone accepted.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import statistics


def read(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def peak(run):
    with (run / 'fereg_runtime.csv').open() as stream:
        return int(next(r['value'] for r in csv.DictReader(stream)
                        if r['row'] == '13' and r['column'] == '2'))


def score(root, label):
    plan_path = root / (label + '-plan.json')
    plan = read(plan_path)
    progress_path = root / (label + '-progress.json')
    progress = read(progress_path) if progress_path.exists() else []
    by_index = {p['index']: p for p in progress}
    assert len(by_index) == len(progress)
    assert all(0 <= i < len(plan['cells']) for i in by_index)
    controls = plan['inventory_kind'] == 'controls'
    numerator, denominator = plan['variants'][::-1] if controls else plan['variants']
    cells, identities, references = [], {}, set()
    for index, cell in enumerate(plan['cells']):
        attempt = by_index.get(index)
        item = dict(index=index, cell=cell, issues=[], pairs=[],
                    scientific_comparison=attempt['comparison_status'] if attempt else 'not_attempted')
        if not attempt:
            item['issues'].append('not_attempted')
        else:
            assert attempt['cell'] == cell
            assert attempt['label'] == f'{label}-c{index:03d}'
            if attempt['process_returncode'] != 0:
                item['issues'].append('cell_process_failure')
            for rep in range(1, plan['repetitions'] + 1):
                pair = []
                for variant in [numerator, denominator]:
                    matches = list((root / 'runs').glob(f"{attempt['label']}-*-r{rep}-{variant}/result.json"))
                    if len(matches) != 1:
                        item['issues'].append(f'rep{rep}:{variant}:result_count={len(matches)}')
                        continue
                    path = matches[0]
                    result = read(path)
                    if result['status'] != 'pass' or result['process_returncode'] != 0 or not result['sentinel']:
                        item['issues'].append(f'rep{rep}:{variant}:application_failure')
                        continue
                    if not result['seconds'] > 0:
                        item['issues'].append(f'rep{rep}:{variant}:invalid_time')
                        continue
                    request = read(path.parent / 'request.json')
                    assert request['solver'] == cell['solver']
                    for key in ['design', 'n', 'k', 'threads', 'seed', 'shuffled', 'weight_kind', 'weight_design', 'case', 'effects']:
                        assert result['cell'][key] == cell[key], (path.name, key)
                    assert result['cell']['fit_options'].endswith(f"memory({cell['memory']})")
                    fingerprint = result['measurement_fingerprint']
                    if variant == 'fixest':
                        assert result['cell']['fixest_tolerance'] == 1e-6
                        assert result['cell']['fixest_recovery_tolerance'] == 1e-5
                        references.add(hashlib.sha256(json.dumps({k: fingerprint[k] for k in
                            ['fixest_archive_sha256', 'fixest_library']}, sort_keys=True).encode()).hexdigest())
                    else:
                        identity = fingerprint['native']
                        assert identity['abi'] == 'pass'
                        assert identities.setdefault(variant, identity) == identity
                    pair.append((path, result))
                if len(pair) != 2:
                    continue
                (ap, a), (bp, b) = pair
                assert a['cell'] == b['cell']
                assert a['input_sha256'] == b['input_sha256']
                assert a['sample_sha256'] == b['sample_sha256']
                assert a['sample_rows'] == b['sample_rows'] == cell['n']
                record = dict(rep=rep, candidate_seconds=a['seconds'], comparator_seconds=b['seconds'],
                              ratio=a['seconds'] / b['seconds'], input_sha256=a['input_sha256'],
                              sample_sha256=a['sample_sha256'], candidate_result_sha256=sha(ap),
                              comparator_result_sha256=sha(bp), candidate_peak_bytes=peak(ap.parent))
                if controls:
                    record['baseline_peak_bytes'] = peak(bp.parent)
                item['pairs'].append(record)
        if len(item['pairs']) == plan['repetitions']:
            assert len({p['input_sha256'] for p in item['pairs']}) == 1
            item['median_paired_ratio'] = statistics.median(p['ratio'] for p in item['pairs'])
            item['candidate_median_seconds'] = statistics.median(p['candidate_seconds'] for p in item['pairs'])
            item['comparator_median_seconds'] = statistics.median(p['comparator_seconds'] for p in item['pairs'])
            item['wins'] = sum(p['ratio'] < 1 for p in item['pairs'])
        cells.append(item)
    assert len(references) <= 1, 'reference identity changed within the block'
    complete = all(not c['issues'] and len(c['pairs']) == plan['repetitions'] for c in cells)
    measured = [c for c in cells if 'median_paired_ratio' in c]
    threshold = plan['manifest']['acceptance'][
        'native_control_max_confirmed_regression' if controls else 'primary_max_median_paired_ratio']
    return dict(schema='two-fe-broad-score-v1', phase=plan['phase'], label=label,
                plan_sha256=sha(plan_path), progress_sha256=sha(progress_path) if progress_path.exists() else None,
                planned=len(cells), completely_measured=len(measured), identity_audit_complete=complete,
                repetitions=plan['repetitions'], candidate=numerator, comparator=denominator,
                candidate_plugin=identities.get(numerator, {}).get('plugin'),
                reference_identity=next(iter(references), None), threshold=threshold,
                timing_threshold_met=complete and all(c['median_paired_ratio'] <= threshold for c in cells),
                wins=sum(c['median_paired_ratio'] < 1 for c in measured),
                ties=sum(c['median_paired_ratio'] == 1 for c in measured),
                losses=sum(c['median_paired_ratio'] > 1 for c in measured),
                worst_ratio=max((c['median_paired_ratio'] for c in measured), default=None),
                scientific_acceptance='requires_independent_adjudication', cells=cells)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('label')
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    assert not args.output.exists(), 'never overwrite an earlier score receipt'
    result = score(args.root, args.label)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + '\n')
    print('BROAD_SCORE', result['planned'], result['wins'], result['ties'], result['losses'])
