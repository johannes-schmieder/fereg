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
    for rep in range(1, repeats + 1):
        native, reference = [result(root / 'runs' / f'{prefix}-r{rep}-{v}', v, identities)
                             for v in variants]
        assert native['cell'] == reference['cell']
        assert native['input_sha256'] == reference['input_sha256']
        assert native['sample_sha256'] == reference['sample_sha256']
        pairs.append(dict(rep=rep, native_seconds=native['seconds'],
                          reference_seconds=reference['seconds'],
                          ratio=native['seconds'] / reference['seconds']))
        if variants[1] == 'fixest':
            assert reference['cell']['fixest_tolerance'] == 1e-6
            assert reference['cell']['fixest_recovery_tolerance'] == 1e-5
            fingerprint = reference['measurement_fingerprint']
            reference_libraries.append(fingerprint['fixest_library'])
    if reference_libraries:
        assert all(x == reference_libraries[0] for x in reference_libraries)
    return dict(pairs=pairs, median_ratio=statistics.median(p['ratio'] for p in pairs),
                wins=sum(p['ratio'] < 1 for p in pairs),
                native_median_seconds=statistics.median(p['native_seconds'] for p in pairs),
                reference_median_seconds=statistics.median(p['reference_seconds'] for p in pairs))


def audit(root):
    spec = read(root / 'acceptance-20260920.json')
    identities = {v: read(root / f'identity-{v}.json') for v in ['baseline', 'densecore']}
    assert identities['densecore']['base_commit'] == spec['source_baseline']
    source_matches = {p: sha(REPO / p) == h for p, h in identities['densecore']['source'].items()}
    assert all(source_matches.values()), [p for p, matches in source_matches.items() if not matches]
    for variant, identity in identities.items():
        assert sha(root / variant / 'dist/fereg_macos.plugin') == identity['plugin']
        assert identity['abi'] == 'pass'
    fixtures = read(root / 'fixtures-densecore.json')
    assert len(fixtures) == 3 and all(f['status'] == 'pass' and f['terminal_sentinel'] for f in fixtures)
    science = completed(root, 'densecore-output-science', 8)
    assert all(c['outputs']['status'] == 'pass' for c in science['checks'])
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
                    measurement = paired(root, prefix, ['densecore', 'fixest'], 5, identities)
                    cells.append(dict(case=case, k=k, seed=seed, shuffled=shuffled, **measurement))
        assert len(cells) == 24
        final[block] = cells
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
                                   **paired(breadth_root, prefix, ['densecore', 'baseline'], 3, identities)))
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
                               **paired(controls_root, prefix, ['densecore', 'baseline'], 3, identities)))
    for n in [245760, 983040]:
        for threads in [1, 4]:
            label = f'regression-dense-n{n}-t{threads}'
            completed(root, label, 6)
            for k in spec['regressor_counts']:
                prefix = f'{label}-iid-n{n}-k{k}-s8675309-shuffle0-t{threads}'
                regression.append(dict(cell=f'two-fe-dense-n{n}-k{k}-t{threads}',
                                       **paired(root, prefix, ['densecore', 'baseline'], 3, identities)))
    completed(root, 'regression-sparse', 6)
    for k in spec['regressor_counts']:
        prefix = f'regression-sparse-iid-n245760-k{k}-s8675309-shuffle0-t16'
        regression.append(dict(cell=f'two-fe-sparse-n245760-k{k}-t16',
                               **paired(root, prefix, ['densecore', 'baseline'], 3, identities)))
    assert len(regression) == 41
    geomean = math.exp(statistics.mean(math.log(c['median_ratio']) for c in regression))
    failures = [c for c in regression if c['median_ratio'] > spec['regression']['maximum_cell_median_ratio']]
    regression_pass = not failures and geomean <= spec['regression']['maximum_geometric_mean_ratio']
    return dict(status='pass' if final_pass and regression_pass else 'target_not_met',
                artifact=identities['densecore']['plugin'], source_entries_matching=len(source_matches),
                final=final, final_runtime_pass=final_pass, wins_per_cell=wins,
                default_reference_output_disagreements=output_disagreements,
                tight_reference_output_science=science, fixtures=fixtures,
                regression=regression, regression_geomean=geomean,
                regression_initial_failures=failures, regression_pass=regression_pass,
                scope='Local Mac development; independent output audit and focused unit evidence are separate required receipts.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    args = parser.parse_args()
    report = audit(args.root)
    path = args.root / 'acceptance-audit.json'
    assert not path.exists(), 'preserve prior audit evidence'
    path.write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + '\n')
    print(report['status'], 'final', report['final_runtime_pass'], 'regression', report['regression_pass'])
