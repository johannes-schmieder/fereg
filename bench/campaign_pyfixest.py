#!/usr/bin/env python3
"""Pinned PyFixest campaign arm: one warmed, timed fit per process.

Mirrors campaign_r.R: the same inputs, memory handshake, timing window,
saved-FE recovery and output files. `pyfixest-within` is a pinned main
revision whose default is the `within` LSMR demeaner; `pyfixest`, the 0.60.0
release with its default Rust MAP demeaner, remains readable for earlier
campaigns. Both keep their default estimation tolerances.
"""
from __future__ import annotations

import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import sys
import time

HERE = Path(__file__).resolve().parent
# Saved effects use the same recovery tolerance as the fixest arms.
FIXEF_TOLERANCE = 1e-11


def check_pin(arm):
    """Reject any interpreter other than the arm's pinned installation."""
    pin = json.loads((HERE/'campaign.json').read_text())['pyfixest'][arm]
    version = importlib.metadata.version('pyfixest')
    if version != pin['version']:
        raise RuntimeError(f'pyfixest {version} is not the pinned {pin["version"]}')
    origin = importlib.metadata.distribution('pyfixest').read_text('direct_url.json')
    commit = json.loads(origin).get('vcs_info', {}).get('commit_id') if origin else None
    if commit != pin.get('commit'):
        raise RuntimeError(f'pyfixest source {commit} is not the pinned {pin.get("commit")}')
    within = pin.get('within_py')
    if within and importlib.metadata.version('within-py') != within:
        raise RuntimeError('within-py is not the pinned version')
    return version, commit


class Counts:
    """Sample, rank and DoF counts from either PyFixest result API."""

    def __init__(self, fit):
        if hasattr(fit, 'variance_covariance'):
            vcov = fit.variance_covariance
            self.N = int(fit.sample_info.n_obs)
            self.dropped = set(fit.sample_info.dropped_row_index)
            self.levels = [int(n) for n in fit.fixef_counts.n_levels_by_fe]
            self.collinear = list(fit.collinearity.dropped_coef_names)
            self.df_k, self.df_t = float(vcov.df_k), float(vcov.df_t)
            statistics = fit.fitstat
            self.loglik, self.deviance = statistics.loglik, statistics.deviance
            self.converged = getattr(fit, 'convergence', True)
        else:
            self.N = int(fit._N)
            self.dropped = set(fit._na_index)
            self.levels = [int(n) for n in fit._k_fe]
            self.collinear = list(fit._collin_vars)
            self.df_k, self.df_t = float(fit._df_k), float(fit._df_t)
            self.loglik, self.deviance = getattr(fit, '_loglik', None), getattr(fit, 'deviance', None)
            self.converged = getattr(fit, '_convergence', True)


def saved_effects(fit, data, q):
    """Recover every FE and map it to observations by numeric level ID."""
    import numpy as np
    import pandas as pd
    effects = fit.fixef(atol=FIXEF_TOLERANCE, btol=FIXEF_TOLERANCE)
    # 0.60.0 returns dictionaries that omit the zero reference level of the
    # second and later effects; main returns a table that includes it.
    omitted = not isinstance(effects, pd.DataFrame)
    if omitted:
        tables = {name[2:-1] if name.startswith('C(') else name: values
                  for name, values in effects.items()}
    else:
        tables = {name: dict(zip(group.level, group.coefficient))
                  for name, group in effects.groupby('variable')}
    saved = []
    for j in range(1, q+1):
        # A second effect with one level has only the omitted reference.
        table = {float(key): value for key, value in
                 (tables.get(f'g{j}', {}) if omitted and j > 1 else tables[f'g{j}']).items()}
        if omitted and j > 1:
            missing = set(np.unique(data[f'g{j}'].to_numpy(dtype=float)))-set(table)
            if len(missing) != 1:
                raise ValueError('expected one omitted reference level')
            table[missing.pop()] = 0.
        keys = np.array(list(table), dtype=float)
        values = np.array(list(table.values()), dtype=float)
        if not np.isfinite(keys).all() or len(np.unique(keys)) != len(keys):
            raise ValueError('invalid or ambiguous numeric saved-FE IDs')
        order = np.argsort(keys)
        ids = data[f'g{j}'].to_numpy(dtype=float)
        positions = np.searchsorted(keys[order], ids)
        positions = np.minimum(positions, len(keys)-1)
        if not np.array_equal(keys[order][positions], ids):
            raise ValueError('saved-FE ID is absent from pyfixest effects')
        channel = values[order][positions]
        if not np.isfinite(channel).all():
            raise ValueError('nonfinite saved FE')
        saved.append(channel)
    return saved


def wait_for(path, seconds=120):
    started = time.monotonic()
    while not path.exists():
        if time.monotonic()-started > seconds:
            raise RuntimeError('campaign handshake timed out: '+path.name)
        time.sleep(.001)


def write_metadata(path, meta):
    path.write_text(''.join(f'{key}\t{str(value).replace(chr(9), " ").replace(chr(10), " ")}\n'
                            for key, value in meta.items()))


def main(configuration):
    c = dict(line.split('\t', 1) for line in Path(configuration).read_text().splitlines())
    threads = int(c['threads'])
    # Both Rust cores use Rayon's global pool; size it before the first import.
    os.environ['RAYON_NUM_THREADS'] = str(threads)
    import numpy as np
    import pandas as pd
    import pyfixest as pf
    version, commit = check_pin(c['arm'])
    output = Path(c['output'])
    model, k, q = c['model'], int(c['k']), int(c['effects'])
    terms = [f'x{i}' for i in range(1, k+1)]
    absorb = ' + '.join(f'g{j}' for j in range(1, q+1))
    vcov = {'iid': 'iid', 'robust': 'hetero', 'cluster': {'CRV1': 'cluster'}}[c['vce']]
    options = dict(vcov=vcov)
    if model == 'ppml':
        options['ssc'] = pf.ssc(k_adj=False, G_adj=True)
    estimator = pf.feols if model == 'ols' else pf.fepois

    def fit_model(data, fixed_effects):
        return estimator(f"y ~ {' + '.join(terms)} | {fixed_effects}", data, **options)

    fit_model(pd.read_stata(c['warm'], convert_categoricals=False), 'g1')
    data = pd.read_stata(c['dataset'], convert_categoricals=False)
    identity = json.loads(Path(c['dataset']).with_suffix('.json').read_text())
    for name, expected in identity['column_sha256'].items():
        if hashlib.sha256(data[name].to_numpy(dtype='<f8').tobytes()).hexdigest() != expected:
            raise RuntimeError('decoded input mismatch: '+name)
    (output/'READY').write_text('ready\n')
    # The harness resets the kernel memory peak before GO and reads it before ACK.
    wait_for(output/'GO')
    error, fit, saved = None, None, None
    start = time.perf_counter()
    try:
        fit = fit_model(data, absorb)
        if model == 'ppml' and not Counts(fit).converged:
            raise RuntimeError('pyfixest did not converge')
        if c['save_fe'] == '1':
            saved = saved_effects(fit, data, q)
    except Exception as failure:  # The record keeps every failed command.
        error, fit = f'{type(failure).__name__}: {failure}', None
    seconds = time.perf_counter()-start
    (output/'DONE').write_text('done\n')
    wait_for(output/'ACK')
    meta = dict(rc=1 if fit is None else 0, seconds=repr(seconds), complete=1,
                pyfixest_version=version, pyfixest_commit=commit or 'release',
                requested_threads=threads, configured_threads=os.environ['RAYON_NUM_THREADS'],
                reference_profile=c['arm'])
    if fit is not None:
        counts = Counts(fit)
        b = fit.coef()
        names = list(b.index)
        V = np.asarray(fit._vcov, dtype=float)
        # Stata's robust PPML covariance carries n/(n-1); fixest's does not.
        multiplier = counts.N/(counts.N-1) if model == 'ppml' and c['vce'] == 'robust' else 1.
        pd.DataFrame(dict(name=names, value=b.to_numpy())).to_csv(output/'coefficients.csv', index=False, float_format='%.17g')
        def write_covariance(matrix, name):
            rows, columns = np.meshgrid(names, names, indexing='ij')
            pd.DataFrame(dict(row=rows.ravel(), column=columns.ravel(), value=matrix.ravel())).to_csv(
                output/name, index=False, float_format='%.17g')
        write_covariance(V*multiplier, 'covariance.csv')
        write_covariance(V, 'covariance-stock.csv')
        retained = np.ones(len(data), dtype=np.int8)
        retained[sorted(counts.dropped)] = 0
        rows = pd.DataFrame(dict(obs_id=data.obs_id.to_numpy(), retained=retained))
        if saved is not None:
            selected = retained == 1
            predictor = data[names].to_numpy()@b.to_numpy()+np.sum(saved, axis=0)
            fitted = np.full(len(data), np.nan)
            fitted[selected] = (np.exp(predictor) if model == 'ppml' else predictor)[selected]
            rows['fitted'] = fitted
            for j, channel in enumerate(saved, 1):
                rows[f'saved{j}'] = np.where(selected, channel, np.nan)
        rows.to_csv(output/'rows.csv', index=False, na_rep='.', float_format='%.17g')
        # fixest's nparams convention: one reference level per additional FE.
        absorbed = sum(counts.levels)-(q-1)
        if model == 'ols' and c['vce'] == 'cluster':
            absorbed = counts.df_k-len(b)-1
        meta.update(N=counts.N, df_a=absorbed, df_a_unadjusted=sum(counts.levels)-(q-1), df_m=len(b),
                    converged=1, covariance_multiplier=repr(multiplier), finite_slope_rank=len(b),
                    slope_omissions=len(counts.collinear),
                    covariance_policy=f"{model}-{c['vce']}-"+('K.adj=FALSE' if model == 'ppml' else 'stock'))
        if model == 'ols':
            meta.update(df_r=repr(counts.df_t), correction_df=repr(counts.N-counts.df_k),
                        rss=repr(float(np.sum(np.asarray(fit.resid(), dtype=float)**2))))
        else:
            meta.update(ll=repr(float(counts.loglik)), deviance=repr(float(counts.deviance)))
        if c['vce'] == 'cluster':
            meta['N_clust'] = int(data.cluster[retained == 1].nunique())
    if error is not None:
        meta['error'] = error
    write_metadata(output/'metadata.tsv', meta)
    print(f'CAMPAIGN_COMPLETE rc={meta["rc"]}', flush=True)
    return int(meta['rc'])


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
