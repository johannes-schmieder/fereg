"""Read saved IID covariance exports; never fit, regrade, or rank an estimator."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path


CELL = 'ols-three_fe_sparse-n3932160-k10-t4-main'
ARCHIVE_SHA = '17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def decompose(native, stock, covariance_native, covariance_stock, se_native, se_stock):
    names = set(se_native)
    assert names == set(se_stock) and names
    square = {(i, j) for i in names for j in names}
    assert square == set(covariance_native) == set(covariance_stock)
    assert native['N'] == stock['N'] and native['df_r'] > 0
    assert 0 < stock['df_k'] < stock['N'] and native['rss'] > 0 and stock['rss'] > 0
    assert all(math.isfinite(v) for d in [covariance_native, covariance_stock, se_native, se_stock] for v in d.values())
    assert all(v > 0 for d in [se_native, se_stock] for v in d.values())
    stock_df = stock['N'] - stock['df_k']
    # IID covariance = (RSS / residual DoF) times the inverse within Gram.
    scale_native = native['rss'] / native['df_r']
    scale_stock = stock['rss'] / stock_df
    factor = math.sqrt(scale_native / scale_stock)
    bn = {ij: v / scale_native for ij, v in covariance_native.items()}
    bs = {ij: v / scale_stock for ij, v in covariance_stock.items()}
    assert all(bn[i, i] > 0 and bs[i, i] > 0 for i in names)
    observed = [se_native[i] / se_stock[i] for i in names]
    return dict(native_df=native['df_r'], stock_df=stock_df,
                df_difference=native['df_r'] - stock_df,
                native_rss=native['rss'], stock_rss=stock['rss'],
                native_rss_lower=native['rss'] < stock['rss'],
                predicted_se_ratio=factor, observed_se_ratio_range=[min(observed), max(observed)],
                original_max_se_difference=max(abs(se_native[i] - se_stock[i]) for i in names),
                matched_variance_scale_max_se_difference=max(abs(se_native[i] - factor * se_stock[i]) for i in names),
                max_normalized_inverse_gram_difference=max(abs(bn[i, j] - bs[i, j]) / math.sqrt(bn[i, i] * bn[j, j]) for i, j in square),
                eligible=False, adjudication='variance-scale attribution only; original NR remains')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--current', type=Path, required=True)
    parser.add_argument('--original', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert not args.output.exists()
    receipt = dict(cell=CELL, records=[], source_roots=dict(current=str(args.current), original=str(args.original)),
                   files={}, script_sha256=sha(Path(__file__)))

    def read(path, root, tag):
        receipt['files'][tag + '/' + str(path.relative_to(root))] = sha(path)
        return json.loads(path.read_text())

    for rep in range(1, 5):
        group = args.current / 'results' / CELL / f'warm-{rep}'
        captured = read(group / 'GROUP.COMPLETE.json', args.current, 'current')
        native = read(group / 'fereg/result.json', args.current, 'current')
        stock = read(group / 'fixest/result.json', args.current, 'current')
        reghdfe = read(args.original / 'results' / CELL / f'warm-{rep}' / 'reghdfe/result.json', args.original, 'original')
        assert all(r['status'] == 'pass' for r in [native, stock, reghdfe])
        assert native['input_sha256'] == stock['input_sha256'] == reghdfe['input_sha256']
        assert native['sample_sha256'] == stock['sample_sha256'] == reghdfe['sample_sha256']
        covariances, errors = {}, {}
        for arm in ['fereg', 'fixest']:
            for filename in ['coefficients.csv', 'covariance.csv']:
                p = group / arm / filename
                receipt['files']['current/' + str(p.relative_to(args.current))] = sha(p)
            rows = list(csv.DictReader((group / arm / 'covariance.csv').open()))
            covariances[arm] = {(r['row'], r['column']): float(r['value']) for r in rows}
            assert len(rows) == len(covariances[arm])
            rows = list(csv.DictReader((group / arm / 'coefficients.csv').open()))
            errors[arm] = {r['name']: float(r['se']) for r in rows}
            assert len(rows) == len(errors[arm])
        result = decompose(native['metadata'], stock['metadata'], covariances['fereg'], covariances['fixest'], errors['fereg'], errors['fixest'])
        result.update(repetition=rep, input_sha256=native['input_sha256'], sample_sha256=native['sample_sha256'],
                      original_comparisons=captured['comparisons'],
                      native_df_matches_original_reghdfe=native['metadata']['df_r'] == reghdfe['metadata']['df_r'],
                      native_absorbed_df_matches_original_reghdfe=native['metadata']['df_a'] == reghdfe['metadata']['df_a'],
                      native_reghdfe_rss_difference=native['metadata']['rss'] - reghdfe['metadata']['rss'],
                      reghdfe_comparison_is_same_input_only_not_new_paired_timing=True)
        receipt['records'].append(result)
    archive = args.original / 'input/archives/fixest.tar.gz'
    assert sha(archive) == ARCHIVE_SHA
    receipt['fixest_archive_sha256'] = ARCHIVE_SHA
    receipt['current_measurement_fingerprint'] = native['measurement_fingerprint']
    receipt['original_measurement_fingerprint'] = reghdfe['measurement_fingerprint']
    args.output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    print(json.dumps(receipt['records'], indent=2))


if __name__ == '__main__':
    main()
