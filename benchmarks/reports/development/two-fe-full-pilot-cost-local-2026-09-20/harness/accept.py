"""Audit the frozen local dense two-FE target and its regression controls."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import statistics


REPO = Path(__file__).resolve().parents[2]


def read(path):
    return json.loads(path.read_text())


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(2**20), b''):
            digest.update(block)
    return digest.hexdigest()


def result(path, variant, identities):
    value = read(path / 'result.json')
    assert value['status'] == 'pass' and value['process_returncode'] == 0
    assert value['sentinel'] and value['seconds'] > 0
    assert value['sample_rows'] == value['cell']['n']
    if variant != 'fixest':
        fingerprint = value['measurement_fingerprint']
        assert fingerprint.get('native', fingerprint) == identities[variant]
    return value


def completed(root, label, count, allow_reference_mismatch=False, controls=False):
    suffix = '-controls.json' if controls else '-complete.json'
    value = read(root / (label + suffix))
    permitted = ['pass', 'reference_output_mismatch'] if allow_reference_mismatch else ['pass']
    assert value['status'] in permitted
    assert len(value['checks']) == count
    assert all(c['science']['sample_exact'] for c in value['checks'])
    return value


def paired(root, prefix, variants, repeats, identities):
    pairs = []
    reference_libraries = []
    reference_archives = []
    cells = []
    inputs = []
    for rep in range(1, repeats + 1):
        native, reference = [result(root / 'runs' / f'{prefix}-r{rep}-{v}', v, identities)
                             for v in variants]
        assert native['cell'] == reference['cell']
        assert native['input_sha256'] == reference['input_sha256']
        assert native['sample_sha256'] == reference['sample_sha256']
        cells.append(native['cell'])
        inputs.append(native['input_sha256'])
        pairs.append(dict(rep=rep, native_seconds=native['seconds'],
                          reference_seconds=reference['seconds'],
                          ratio=native['seconds'] / reference['seconds']))
        if variants[1] == 'fixest':
            assert reference['cell']['fixest_tolerance'] == 1e-6
            assert reference['cell']['fixest_recovery_tolerance'] == 1e-5
            fingerprint = reference['measurement_fingerprint']
            reference_libraries.append(fingerprint['fixest_library'])
            reference_archives.append(fingerprint['fixest_archive_sha256'])
    if reference_libraries:
        assert all(x == reference_libraries[0] for x in reference_libraries)
        assert all(x == reference_archives[0] for x in reference_archives)
    assert all(c == cells[0] for c in cells)
    assert len(set(inputs)) == 1
    return dict(pairs=pairs, median_ratio=statistics.median(p['ratio'] for p in pairs),
                wins=sum(p['ratio'] < 1 for p in pairs),
                native_median_seconds=statistics.median(p['native_seconds'] for p in pairs),
                reference_median_seconds=statistics.median(p['reference_seconds'] for p in pairs),
                measurement_cell=cells[0], input_sha256=inputs[0],
                reference_library_digest=hashlib.sha256(json.dumps(reference_libraries[0], sort_keys=True).encode()).hexdigest()
                    if reference_libraries else None,
                reference_archive_sha256=reference_archives[0] if reference_archives else None)


def audit(root, candidate):
    spec = read(root / 'acceptance-20260920.json')
    identities = {v: read(root / f'identity-{v}.json') for v in ['baseline', candidate]}
    assert identities[candidate]['base_commit'] == spec['source_baseline']
    provenance = read(root / 'baseline-provenance.json')
    assert provenance['committed_source'] == spec['source_baseline']
    assert provenance['artifact_identity'] == identities['baseline']
    assert provenance['source_files_verified'] == len(identities['baseline']['source'])
    source_matches = {p: sha(REPO / p) == h for p, h in identities[candidate]['source'].items()}
    assert all(source_matches.values()), [p for p, matches in source_matches.items() if not matches]
    for variant, identity in identities.items():
        assert sha(root / variant / 'dist/fereg_macos.plugin') == identity['plugin']
        assert identity['abi'] == 'pass'
    fixtures = read(root / f'fixtures-{candidate}.json')
    assert len(fixtures) == 3 and all(f['status'] == 'pass' and f['terminal_sentinel'] for f in fixtures)
    science = completed(root, f'{candidate}-output-science', 8)
    assert all(c['outputs']['status'] == 'pass' for c in science['checks'])
    independent = read(root / f'{candidate}-output-science-{candidate}-independent-outputs.json')
    assert len(independent) == 8 and all(c['status'] == 'pass' for c in independent)
    final, output_disagreements = {}, {}
    for block in ['a', 'b']:
        cells = []
        for kind, seed, shuffled, cases in [
                ('primary', spec['primary_seed'], False, spec['cases']),
                ('ordered', 104729, False, ['iid']),
                ('shuffled', 31415927, True, ['iid'])]:
            label = f'final-{block}-{kind}'
            receipt = completed(root, label, len(cases) * 2 * 5, kind == 'primary')
            output_disagreements[label] = [c for c in receipt['checks']
                                           if c.get('outputs', {}).get('status') == 'mismatch']
            for k in spec['regressor_counts']:
                for case in cases:
                    prefix = f'{label}-{case}-n983040-k{k}-s{seed}-shuffle{int(shuffled)}-t16'
                    measurement = paired(root, prefix, [candidate, 'fixest'], 5, identities)
                    cells.append(dict(case=case, k=k, seed=seed, shuffled=shuffled, **measurement))
        assert len(cells) == 24
        final[block] = cells
    assert len({c['reference_library_digest'] for cells in final.values() for c in cells}) == 1
    assert len({c['reference_archive_sha256'] for cells in final.values() for c in cells}) == 1
    final_pass = all(c['median_ratio'] <= spec['maximum_median_paired_fereg_fixest_ratio_per_cell_per_block']
                     for cells in final.values() for c in cells)
    wins = []
    for a, b in zip(final['a'], final['b']):
        assert all(a[key] == b[key] for key in ['case', 'k', 'seed', 'shuffled'])
        wins.append(a['wins'] + b['wins'])
    final_pass &= min(wins) >= spec['minimum_fereg_wins_across_both_blocks_per_cell']

    regression = []
    breadth_root = root / 'regression-breadth'
    completed(breadth_root, 'regression-breadth', 60)
    for k in spec['regressor_counts']:
        for case in spec['cases']:
            prefix = f'regression-breadth-{case}-n983040-k{k}'
            regression.append(dict(cell=f'one-fe-{case}-k{k}',
                                   **paired(breadth_root, prefix, [candidate, 'baseline'], 3, identities)))
    controls_root = root / 'regression-controls'
    completed(controls_root, 'regression-controls', 66, controls=True)
    controls = [(n, k, t, None, False) for n, k in [(245760, 10), (983040, 10), (983040, 50)]
                for t in [1, 4, 16] if not (n == 983040 and t == 16)]
    controls += [(983040, 50, t, g, shuffled) for t in [4, 16]
                 for g, shuffled in [(1000, False), (None, True)]]
    for n, k, t, g, shuffled in controls:
        tag = f'n{n}-k{k}-g{g or "default"}-shuffle{int(shuffled)}-t{t}'
        prefix = f'regression-controls-controls-{tag}'
        regression.append(dict(cell=f'one-fe-{tag}',
                               **paired(controls_root, prefix, [candidate, 'baseline'], 3, identities)))
    for n in [245760, 983040]:
        for threads in [1, 4]:
            label = f'regression-dense-n{n}-t{threads}'
            completed(root, label, 6)
            for k in spec['regressor_counts']:
                prefix = f'{label}-iid-n{n}-k{k}-s8675309-shuffle0-t{threads}'
                regression.append(dict(cell=f'two-fe-dense-n{n}-k{k}-t{threads}',
                                       **paired(root, prefix, [candidate, 'baseline'], 3, identities)))
    completed(root, 'regression-sparse', 6)
    for k in spec['regressor_counts']:
        prefix = f'regression-sparse-iid-n245760-k{k}-s8675309-shuffle0-t16'
        regression.append(dict(cell=f'two-fe-sparse-n245760-k{k}-t16',
                               **paired(root, prefix, [candidate, 'baseline'], 3, identities)))
    assert len(regression) == 41
    initial_failures = [c.copy() for c in regression
                        if c['median_ratio'] > spec['regression']['maximum_cell_median_ratio']]
    confirmation_path = root / 'regression-confirmations.json'
    confirmations = read(confirmation_path) if confirmation_path.exists() else []
    assert len({c['cell'] for c in confirmations}) == len(confirmations)
    assert {c['cell'] for c in confirmations} <= {c['cell'] for c in initial_failures}
    by_cell = {c['cell']: c for c in regression}
    for confirmation in confirmations:
        initial = by_cell[confirmation['cell']]
        relative_root = Path(confirmation['root'])
        assert not relative_root.is_absolute() and '..' not in relative_root.parts
        check_root = root / relative_root
        controls = confirmation.get('controls', False)
        completed(check_root, confirmation['label'], 10 if controls else 5, controls=controls)
        checked = paired(check_root, confirmation['prefix'], [candidate, 'baseline'], 5, identities)
        assert checked['measurement_cell'] == initial['measurement_cell']
        assert checked['input_sha256'] == initial['input_sha256']
        initial['initial_measurement'] = initial.copy()
        initial.update(checked)
        initial['confirmation'] = confirmation
    geomean = math.exp(statistics.mean(math.log(c['median_ratio']) for c in regression))
    failures = [c for c in regression if c['median_ratio'] > spec['regression']['maximum_cell_median_ratio']]
    regression_pass = not failures and geomean <= spec['regression']['maximum_geometric_mean_ratio']
    return dict(status='pass' if final_pass and regression_pass else 'target_not_met',
                artifact=identities[candidate]['plugin'], source_entries_matching=len(source_matches),
                final=final, final_runtime_pass=final_pass, wins_per_cell=wins,
                default_reference_output_disagreements=output_disagreements,
                tight_reference_output_science=science, independent_output_audit=independent, fixtures=fixtures,
                regression=regression, regression_geomean=geomean,
                regression_initial_failures=initial_failures, regression_failures=failures,
                regression_confirmations=confirmations, regression_pass=regression_pass,
                scope='Local Mac development; independent output audit and focused unit evidence are separate required receipts.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('--candidate', default='densewin')
    args = parser.parse_args()
    report = audit(args.root, args.candidate)
    path = args.root / 'acceptance-audit.json'
    assert not path.exists(), 'preserve prior audit evidence'
    path.write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + '\n')
    print(report['status'], 'final', report['final_runtime_pass'], 'regression', report['regression_pass'])
