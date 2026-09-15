"""Prepared local German PPML public-mean audit; not yet numerically validated.

Run only after timed fits have finished. Finite moments are streamed; the
registered independent FE-span checker receives the complete fixed X beta.
No estimation, trimming, model change, or benchmark admission happens here.
"""
from pathlib import Path
import argparse
import csv
import hashlib
import json
import math
import re
import sys

import numpy as np
import pandas as pd

REPO = Path(__file__).resolve().parents[1] / 'source'
sys.path.insert(0, str(REPO / 'scripts'))
from ppml_auto_science import coefficient_binding, objective_score

TERMS = [f'age{p}_ed{e}' for e in (10, 13, 16, 18, 99) for p in (2, 3)]
TERMS += [f'{y}.year' for y in range(2002, 2010)]
RAW = ['obs_id', 'persid', 'betnr', 'year', 'logrwage2', 'age', 'edyrs']


def controls(frame):
    age = frame['age'].to_numpy(dtype=float) - 40
    edu = frame['edyrs'].to_numpy(dtype=float)
    edu = np.where(np.isnan(edu), 99, edu)
    assert np.isin(edu, (10, 13, 16, 18, 99)).all()
    return np.column_stack(
        [age ** p / (100 if p == 2 else 1000) * (edu == e)
         for e in (10, 13, 16, 18, 99) for p in (2, 3)]
        + [(frame['year'].to_numpy() == year).astype(float)
           for year in range(2002, 2010)])


def report(args):
    directory = args.directory
    fit = dict(line.split('=', 1) for line in
               (directory / 'fit-result.txt').read_text().splitlines())
    assert fit['FIT_RC'] == '0'
    assert (directory / 'result.txt').read_text().strip() == 'STATA_RC=0'
    assert re.search(r'^' + re.escape(args.sentinel) + r'(?:\||$)',
                     (directory / 'run.log').read_text(), re.M)
    terminal = directory / 'completion.json'
    if terminal.exists():
        assert json.loads(terminal.read_text())['accepted'] is True
    scalars = {k: float(v) if v.strip() != '.' else None
               for k, v in csv.reader((directory / 'scalars.tsv').open(), delimiter='\t')}
    assert scalars['converged'] == 1
    assert scalars['N'] == args.expected_retained
    assert scalars['N_full'] == args.expected_input
    beta = pd.read_csv(directory / 'b.tsv', sep='\t', index_col=0,
                       na_values=['.'], skipinitialspace=True)[TERMS].to_numpy(float).ravel()
    assert beta.shape == (18,) and np.isfinite(beta).all()
    n = args.expected_retained
    # The helper's existing worst-case conversion/sparse/recurrence envelope
    # uses one fixed predictor and two effects. Charge our retained vectors and
    # a conservative parsing/control chunk bank in addition, before allocation.
    binding_bytes = 8 * (4 * n + 64 * n * 2 + 32 * n)
    retained_bytes = 5 * 8 * n  # y, mu, X beta, and two original FE columns
    chunk_bytes = 1024 * args.chunk_rows
    modeled_bytes = binding_bytes + retained_bytes + chunk_bytes
    assert modeled_bytes <= args.memory_bytes, (modeled_bytes, args.memory_bytes)
    if args.calibrate:
        assert args.expected_input <= 200000, 'calibration remains bounded'
    y = np.empty(n)
    mu = np.empty(n)
    xb = np.empty(n)
    groups = np.empty((n, 2))
    moments, squares = [[] for _ in range(19)], [[] for _ in range(19)]
    present = np.zeros(19, dtype=bool)
    ll_parts, dev_parts = [], []
    residual_norm = y_norm = 0.0
    input_count = kept = extreme_input = extreme_kept = 0
    log_input_max = log_kept_max = -math.inf
    mask_hash = hashlib.sha256()
    calibration_x = []
    columns = RAW + ['sample', 'fitted_mu', 'absorbed']
    with pd.read_stata(directory / 'outputs.dta', columns=columns,
                       convert_categoricals=False, chunksize=args.chunk_rows) as reader:
        for frame in reader:
            assert np.array_equal(frame['obs_id'], np.arange(
                input_count + 1, input_count + len(frame) + 1))
            mask = frame['sample'].to_numpy()
            assert np.isin(mask, [0, 1]).all()
            mask_hash.update(np.asarray(mask, dtype=np.uint8).tobytes())
            active = mask == 1
            input_count += len(frame)
            log_y = frame['logrwage2'].to_numpy(float)
            assert np.isfinite(log_y).all()
            extreme_input += int(np.sum(log_y > 20))
            log_input_max = max(log_input_max, float(log_y.max()))
            assert np.isnan(frame['fitted_mu'].to_numpy(float)[~active]).all()
            selected = frame.loc[active]
            if len(selected) == 0:
                continue
            end = kept + len(selected)
            assert end <= n
            yi = np.exp(log_y[active])
            mi = selected['fitted_mu'].to_numpy(float)
            gi = selected[['persid', 'betnr']].to_numpy(float)
            xi = controls(selected)
            assert all(np.isfinite(a).all() for a in (yi, mi, gi, xi))
            assert np.all(yi > 0) and np.all(mi > 0)
            assert np.all(gi == np.floor(gi)) and np.all(np.abs(gi) < 2**53)
            assert np.isfinite(selected['absorbed'].to_numpy(float)).all()
            y[kept:end], mu[kept:end], groups[kept:end] = yi, mi, gi
            xb[kept:end] = xi @ beta
            if args.calibrate:
                calibration_x.append(xi.copy())
            residual = yi - mi
            residual_norm = math.hypot(residual_norm, float(np.linalg.norm(residual / np.sqrt(mi))))
            y_norm = math.hypot(y_norm, float(np.linalg.norm(yi / np.sqrt(mi))))
            finite = np.column_stack((xi, np.ones(len(selected))))
            present |= np.any(finite != 0, axis=0)
            m = finite.T @ residual
            s = (finite * finite).T @ mi
            assert np.isfinite(m).all() and np.isfinite(s).all()
            for j in range(19):
                moments[j].append(float(m[j]))
                squares[j].append(float(s[j]))
            delta = (mi - yi) / yi
            log_ratio = np.log(mi) - np.log(yi)
            near = np.abs(delta) < 0.5
            log_ratio[near] = np.log1p(delta[near])
            dev_parts.append(2 * math.fsum((mi - yi - yi * log_ratio).tolist()))
            # This is the registered helper's formula, including its use of
            # lgamma. Extreme-outcome cancellation must be investigated if
            # observed; this audit does not silently replace that definition.
            ll_parts.append(math.fsum(float(a * math.log(b) - b - math.lgamma(a + 1))
                                      for a, b in zip(yi, mi)))
            extreme_kept += int(np.sum(log_y[active] > 20))
            log_kept_max = max(log_kept_max, float(log_y[active].max()))
            kept = end
            print(f'AUDIT_ROWS {input_count} retained={kept}', flush=True)
    assert input_count == args.expected_input and kept == n
    scale = max(residual_norm, y_norm)
    moments = np.array([math.fsum(a) for a in moments])
    squares = np.array([math.fsum(a) for a in squares])
    assert math.isfinite(scale) and scale > 0
    assert not np.any((squares == 0) & present)
    denominator = np.sqrt(squares) * scale
    assert np.isfinite(denominator).all()
    relative = np.divide(np.abs(moments), denominator,
                         out=np.zeros(19), where=squares > 0)
    residual = y - mu
    fe_maxima = []
    for effect in range(2):
        _, codes = np.unique(groups[:, effect], return_inverse=True)
        fe_moments = np.bincount(codes, weights=residual)
        fe_squares = np.bincount(codes, weights=mu)
        denominator = np.sqrt(fe_squares) * scale
        assert np.isfinite(fe_moments).all() and np.isfinite(denominator).all()
        assert np.all(fe_squares > 0)
        fe_maxima.append(float(np.max(np.abs(fe_moments) / denominator)))
        del codes, fe_moments, fe_squares, denominator
    del residual
    score = dict(log_likelihood=math.fsum(ll_parts), deviance=math.fsum(dev_parts),
                 score_scale=scale, finite_score_max=float(relative.max()),
                 fe_score_maxima=fe_maxima,
                 stationarity_pass=max(float(relative.max()), *fe_maxima) <= 1e-8)
    assert all(math.isfinite(score[k]) for k in
               ('log_likelihood', 'deviance', 'score_scale', 'finite_score_max'))
    assert score['deviance'] >= 0
    calibration = None
    if args.calibrate:
        full_x = np.concatenate(calibration_x)
        registered = objective_score(y, full_x, mu, groups)
        for key in ('log_likelihood', 'deviance', 'score_scale'):
            assert math.isclose(score[key], registered[key], rel_tol=1e-12, abs_tol=1e-9), key
        assert math.isclose(score['finite_score_max'], registered['finite_score_max'],
                            rel_tol=1e-8, abs_tol=1e-14)
        assert np.allclose(score['fe_score_maxima'], registered['fe_score_maxima'],
                           rtol=1e-8, atol=1e-14)
        assert score['stationarity_pass'] == registered['stationarity_pass']
        full_binding = coefficient_binding(full_x, beta, mu, groups,
                                           memory_bytes=args.memory_bytes,
                                           max_iterations=args.max_iterations)
        calibration = dict(registered_objective_score=registered,
                           original_design_binding=full_binding,
                           streamed_score_agrees=True)
        del full_x, calibration_x
    binding = coefficient_binding(xb[:, None], np.ones(1), mu, groups,
                                  memory_bytes=binding_bytes,
                                  max_iterations=args.max_iterations)
    if calibration is not None:
        assert binding['coefficient_binding_pass'] == calibration['original_design_binding']['coefficient_binding_pass']
    replay = {name: math.isclose(scalars[name], score[key], rel_tol=1e-10, abs_tol=1e-8)
              for name, key in [('ll', 'log_likelihood'), ('deviance', 'deviance')]}
    result = dict(schema_version=1, qualification_eligible=False, campaign_admission=False,
                  directory_basename=directory.name, input_rows=input_count, retained_rows=n,
                  sample_uint8_sha256=mask_hash.hexdigest(),
                  log_outcome_input_max=log_input_max, log_outcome_retained_max=log_kept_max,
                  input_log_outcomes_above_20=extreme_input,
                  retained_log_outcomes_above_20=extreme_kept,
                  objective_score=score, model_space=binding,
                  reported_objective={name: scalars[name] for name in ('ll', 'deviance')},
                  public_mean_replays_reported_objective=replay,
                  full_public_mean_pass=all(replay.values()) and score['stationarity_pass']
                      and binding['coefficient_binding_pass'],
                  model_space_above_threshold_is_indeterminate=True,
                  finite_score_terms=TERMS + ['_cons'],
                  binding_input='complete fixed 18-column X beta represented as one predictor',
                  chunk_rows=args.chunk_rows, modeled_audit_bytes=modeled_bytes,
                  calibration=calibration,
                  local_audit_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  registered_helper_sha256=hashlib.sha256((REPO / 'scripts/ppml_auto_science.py').read_bytes()).hexdigest())
    with args.output.open('x') as stream:
        json.dump(result, stream, indent=2, allow_nan=False)
        stream.write('\n')
    # The reference remains diagnostic under ADR 0026. Callers explicitly opt
    # into the strict native self-consistency gate; no failure is overwritten.
    if args.require_native_pass:
        assert result['full_public_mean_pass'], 'native public-mean audit failed'
    print('PUBLIC_MEAN_AUDIT_COMPLETE', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--sentinel', required=True)
    parser.add_argument('--expected-input', type=int, default=91045290)
    parser.add_argument('--expected-retained', type=int, default=88876264)
    parser.add_argument('--chunk-rows', type=int, default=262144)
    parser.add_argument('--memory-bytes', type=int, required=True)
    parser.add_argument('--max-iterations', type=int, default=5000)
    parser.add_argument('--calibrate', action='store_true')
    parser.add_argument('--require-native-pass', action='store_true')
    args = parser.parse_args()
    assert 1 <= args.chunk_rows <= 1048576
    assert 2 <= args.expected_retained <= args.expected_input
    assert args.memory_bytes > 0 and args.max_iterations > 0
    assert not args.output.exists(), 'preserve prior audit receipts'
    report(args)
