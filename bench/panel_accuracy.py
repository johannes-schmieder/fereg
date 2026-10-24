#!/usr/bin/env python3
"""Quantify existing Veneto saved-output discrepancies, without refitting."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path

import numpy as np
import pandas as pd


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024*1024), b''):
            digest.update(block)
    return digest.hexdigest()


def differences(reference, other, *, rtol, atol, weights=None):
    if reference.shape != other.shape or not np.isfinite(reference).all() or not np.isfinite(other).all():
        raise ValueError('nonfinite or mismatched accuracy coordinates')
    delta = other-reference
    absolute = np.abs(delta)
    tolerance = atol+rtol*np.maximum(np.abs(reference), np.abs(other))
    scale = np.sqrt(np.mean((reference-reference.mean())**2))
    result = dict(coordinates=int(delta.size), maximum_absolute_error=float(absolute.max()),
                  rmse=float(np.sqrt(np.mean(delta**2))), p99_absolute_error=float(np.quantile(absolute, .99)),
                  outside_frozen_tolerance=int(np.count_nonzero(absolute > tolerance)),
                  maximum_tolerance_multiple=float(np.max(absolute/tolerance)),
                  reference_standard_deviation=float(scale))
    if weights is not None:
        result['observation_weighted_rmse'] = float(np.sqrt(np.average(delta**2, weights=weights)))
    return result


def centered_levels(values, codes):
    counts = np.bincount(codes)
    if not counts.size or not (counts > 0).all():
        raise ValueError('expected contiguous retained FE codes')
    levels = np.bincount(codes, weights=values)/counts
    return levels-np.average(levels, weights=counts), counts


def model_scores(y, fitted, codes, X):
    residual = y-fitted
    normalizer = np.abs(y)+np.abs(fitted)
    scores = {}
    for name, code in zip(['worker', 'firm'], codes):
        moments = np.bincount(code, weights=residual)
        scale = np.maximum(1, np.bincount(code, weights=normalizer))
        scores[name] = float(np.max(np.abs(moments)/scale))
    scores['controls'] = float(np.max(np.abs(X.T@residual)/np.maximum(1, np.abs(X).T@normalizer)))
    scores['maximum'] = max(scores.values())
    return scores


def coordinates(directory, k):
    names = [f'x{i}' for i in range(1, k+1)]
    beta = {r['name']: float(r['value']) for r in csv.DictReader((directory/'coefficients.csv').open())}
    covariance = {(r['row'], r['column']): float(r['value'])
                  for r in csv.DictReader((directory/'covariance.csv').open())}
    return (np.array([beta[n] for n in names]),
            np.array([[covariance[a, b] for b in names] for a in names]))


def diagnose(root, panel):
    task = 1 if panel == 'long' else 2
    campaign = json.loads((root/f'task-{task}/results.json').read_text())
    cell = f'veneto-{panel}-t4'
    records = {r['arm']: r for r in campaign['records'] if r['cell'] == cell and r['repetition'] == 1}
    manifest = json.loads((root/'manifest.json').read_text())
    specification = next(c for c in manifest['cells'] if c['id'] == cell)
    path = root/'data'/f"ols-veneto-{panel}-n{specification['rows']}.dta"
    identity = json.loads(path.with_suffix('.json').read_text())
    if sha256(path) != identity['sha256']:
        raise ValueError('frozen model input hash differs')
    frame = pd.read_stata(path, convert_categoricals=False)
    y = frame.y.to_numpy(dtype=float)
    X = frame[[f'x{i}' for i in range(1, specification['regressors']+1)]].to_numpy(dtype=float)
    codes = [frame[f'g{i}'].to_numpy(dtype=int)-1 for i in [1, 2]]
    native_beta = native_covariance = native_fitted = native_effects = None
    rows = []
    input_hashes = {str(path.relative_to(root)): identity['sha256']}
    for arm in ['fereg', 'fixest', 'fixest-strict', 'reghdfe']:
        record = records[arm]
        if record['process_rc'] != 0 or record['metadata']['rc'].strip() != '0' or record['metadata']['complete'].strip() != '1':
            raise ValueError('diagnostic requires a completed original command')
        directory = root/f'task-{task}/output'/cell/(arm+'-r1')
        path = directory/'rows.csv.gz'
        if not path.exists():
            path = directory/'rows.csv'
        values = pd.read_csv(path, float_precision='round_trip', na_values=['.'])
        if (not np.array_equal(values.obs_id.to_numpy(), frame.obs_id.to_numpy())
                or not (values.retained == 1).all()):
            raise ValueError('frozen sample or row alignment differs')
        input_hashes[str(path.relative_to(root))] = sha256(path)
        for name in ['coefficients.csv', 'covariance.csv', 'record.json']:
            input_hashes[str((directory/name).relative_to(root))] = sha256(directory/name)
        fitted = values.fitted.to_numpy(dtype=float)
        effects = [centered_levels(values[f'saved{i}'].to_numpy(dtype=float), codes[i-1]) for i in [1, 2]]
        if not np.isfinite(fitted).all() or not all(np.isfinite(e[0]).all() for e in effects):
            raise ValueError('nonfinite saved outputs')
        beta, covariance = coordinates(directory, specification['regressors'])
        scores = model_scores(y, fitted, codes, X)
        rss = float((y-fitted)@(y-fitted))
        if arm == 'fereg':
            native_beta, native_covariance, native_fitted, native_effects = beta, covariance, fitted.copy(), effects
            if scores['maximum'] > 1e-7:
                raise ValueError('native independent model score failed')
        data = dict(arm=arm, original_status=record['status'], independent_model_scores=scores,
                    reported_rss=float(record['metadata']['rss']), reconstructed_rss=rss,
                    coefficient_differences=differences(native_beta, beta, rtol=1e-7, atol=1e-10),
                    covariance_differences=differences(native_covariance, covariance, rtol=1e-6, atol=1e-10),
                    fitted_differences=differences(native_fitted, fitted, rtol=1e-7, atol=1e-9))
        data['maximum_coefficient_difference_in_native_se'] = float(np.max(np.abs(beta-native_beta)/np.sqrt(np.diag(native_covariance))))
        data['maximum_standard_error_relative_difference'] = float(np.max(np.abs(np.sqrt(np.diag(covariance))/np.sqrt(np.diag(native_covariance))-1)))
        native_rss = float(records['fereg']['metadata']['rss'])
        data['relative_reported_rss_difference'] = (data['reported_rss']-native_rss)/native_rss
        data['relative_reconstructed_rss_difference'] = (rss-native_rss)/native_rss
        data['centered_fe_differences'] = {name: differences(a[0], b[0], rtol=1e-7, atol=1e-9, weights=a[1])
                                          for name, a, b in zip(['worker', 'firm'], native_effects, effects)}
        rows.append(data)
        del values
    return dict(schema=1, panel=panel, cell=cell, repetition=1, rows=len(frame),
                comparison='Existing four-core first repetitions; no refitting or gate changes. Differences use fereg as the reference, not a full-panel exact oracle.',
                definitions=dict(model_score='Maximum absolute residual moment over worker levels, firm levels and controls, divided by max(1, corresponding sum of abs(y)+abs(fitted)).',
                                 centered_fe='Each level effect minus its observation-weighted channel mean; FE error quantiles/RMSE give each level equal weight unless marked observation_weighted.',
                                 output_errors='Absolute errors in log daily wage units, relative to the native saved outputs.'),
                inputs_sha256=input_hashes, records=rows)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--panel', choices=['long', 'short'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = diagnose(args.root, args.panel)
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print('PANEL_ACCURACY_COMPLETE '+args.panel)
