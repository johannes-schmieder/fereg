#!/usr/bin/env python3
"""Private Veneto AKM comparisons using the maintained campaign estimators.

Only aggregate results may leave the private run directory. Input preparation,
row captures, saved effects and dense smoke witnesses remain private.
"""
from __future__ import annotations

import argparse
from datetime import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import time

import numpy as np
import pandas as pd
from scipy import sparse
from scipy.sparse.csgraph import connected_components

import campaign
from campaign import close_values, dataset_key, sha256, write_json
from campaign_oracle import oracle
from qualify_scc import HostMonitor

PANELS = {
    'long': dict(years=list(range(1996, 2002)), rows=5163428, workers=1106419,
                 firms=86647, regressors=7),
    'short': dict(years=[1999, 2001], rows=1319994, workers=659997,
                  firms=42489, regressors=1),
}
# The largest connected set of the long panel (KSS's connected_set before the
# leave-one-out pruning, then iterative removal of singleton workers and firms),
# prepared from the same raw data; it has no retained fevc counterpart, so its
# input audit rebuilds it from the recorded source.
LCS_PANELS = {
    'long-lcs': dict(years=list(range(1996, 2002)), rows=5509415, workers=1182935,
                     firms=117208, regressors=7),
}


def panel_spec(panel):
    return PANELS[panel] if panel in PANELS else LCS_PANELS[panel]
THREADS = [1, 2, 4, 8, 16]
REPETITIONS = 3
# Fit budget of one reused panel run; two runs share a twelve-hour pair job.
REUSE_CLEANUP_SECONDS = 20000


def cell_for(panel, frame, threads, save_fe=True):
    return dict(id=f'veneto-{panel}-t{threads}'+('' if save_fe else '-fe0'), model='ols', design='veneto-'+panel,
                section='veneto', rows=len(frame), regressors=len([c for c in frame if c.startswith('x')]),
                effects=2, save_fe=save_fe, vce='iid', weights='unit', threads=threads,
                arms=['fereg', 'fixest', 'fixest-strict', *campaign.PYFIXEST_ARMS, *campaign.PYFIXEST_STRICT_ARMS,
                      *campaign.JULIA_ARMS]
                + (['reghdfe'] if threads <= 4 else []))


def smoke_fits(panels=PANELS):
    """One four-worker fit of every arm on each scheduled panel's real-data smoke."""
    return len(panels)*len(cell_for('long', pd.DataFrame(), 4)['arms'])


def save_dataset(frame, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    frame.to_stata(path, write_index=False, version=114, time_stamp=datetime(2000, 1, 1))
    decoded = pd.read_stata(path, convert_categoricals=False)
    if not np.array_equal(frame.to_numpy(), decoded.to_numpy()):
        raise ValueError('panel Stata encoding changed values')
    codes = [pd.factorize(frame[f'g{j}'], sort=True)[0] for j in [1, 2]]
    levels = [int(c.max()+1) for c in codes]
    a, b = codes[0], codes[1]+levels[0]
    graph = sparse.coo_matrix((np.ones(2*len(frame), dtype=np.int8),
                              (np.r_[a, b], np.r_[b, a])), shape=(sum(levels),)*2).tocsr()
    count, labels = connected_components(graph, directed=False)
    if count != 1 or any(np.bincount(c).min() < 2 for c in codes):
        raise ValueError('expected connected retained panel without singleton levels')
    np.savez_compressed(path.with_suffix('.components.npz'), component=labels[a])
    metadata = dict(rows=len(frame), sha256=sha256(path),
                    column_sha256={c: hashlib.sha256(frame[c].to_numpy(dtype='<f8').tobytes()).hexdigest()
                                   for c in frame},
                    topology=dict(levels=levels, components=int(count),
                                  first_pair_rank=int(sum(levels)-count),
                                  distinct_first_pair_links=int(graph.nnz//2)))
    write_json(path.with_suffix('.json'), metadata)
    return metadata


def prepare_panel(path, panel):
    source = pd.read_csv(path, float_precision='round_trip')
    long = panel in ('long', 'long-lcs')
    required = ['year', 'worker', 'firm', 'y']+(['observation_key'] if panel in PANELS else [])+(['age'] if long else [])
    if source[required].isna().any().any() or source.duplicated(['worker', 'year']).any():
        raise ValueError('missing or duplicate private panel records')
    if panel not in PANELS:
        source = source.sort_values(['worker', 'year']).reset_index(drop=True)
        source['observation_key'] = np.arange(1, len(source)+1)
    p = panel_spec(panel)
    if (len(source) != p['rows'] or source.worker.nunique() != p['workers']
            or source.firm.nunique() != p['firms'] or sorted(source.year.unique()) != p['years']
            or not source.observation_key.is_unique):
        raise ValueError('panel differs from the established fevc sample; do not substitute it')
    source = source.sort_values('observation_key').reset_index(drop=True)
    data = dict(obs_id=source.observation_key.to_numpy(dtype=np.int32),
                g1=pd.factorize(source.worker, sort=True)[0].astype(np.int32)+1,
                g2=pd.factorize(source.firm, sort=True)[0].astype(np.int32)+1,
                y=source.y.to_numpy(dtype=float))
    if long:
        data.update(x1=((source.age.to_numpy(dtype=float)-40)/40)**2,
                    x2=((source.age.to_numpy(dtype=float)-40)/40)**3)
        for j, year in enumerate(p['years'][:-1], 3):
            data[f'x{j}'] = (source.year == year).to_numpy(dtype=np.int8)
    else:
        data['x1'] = (source.year == 2001).to_numpy(dtype=np.int8)
    return pd.DataFrame(data)


def small_panel(frame):
    # A deterministic real-data smoke; production always uses the entire panel.
    firm = frame.g2.value_counts().index[0]
    workers = sorted(frame.loc[frame.g2 == firm, 'g1'].unique())[:200]
    small = frame.loc[frame.g1.isin(workers)].copy()
    while True:
        keep = np.ones(len(small), dtype=bool)
        for column in ['g1', 'g2']:
            keep &= small.groupby(column)[column].transform('size').to_numpy() > 1
        if keep.all():
            break
        small = small.loc[keep].copy()
    if not 20 <= len(small) <= 1200:
        raise ValueError('real-input smoke size is outside the dense oracle bound')
    for column in ['g1', 'g2']:
        small[column] = pd.factorize(small[column], sort=True)[0].astype(np.int32)+1
    return small.reset_index(drop=True)


def prepare(root, long=None, short=None, long_lcs=None, save_fe=True):
    datasets, cells = {}, []
    selected = [(panel, path) for panel, path in [('long', long), ('short', short), ('long-lcs', long_lcs)] if path]
    for panel, path in selected:
        frame = prepare_panel(path, panel)
        cell = cell_for(panel, frame, 4)
        datasets[panel] = save_dataset(frame, root/'data'/(dataset_key(cell)+'.dta'))
        small = small_panel(frame)
        smoke_cell = cell_for(panel, small, 4)
        datasets['smoke-'+panel] = save_dataset(small, root/'smoke-data'/(dataset_key(smoke_cell)+'.dta'))
        # The independently assembled explicit-design oracle also checks slope rank.
        expected = oracle(small, smoke_cell)
        if expected['residual_score'] > 1e-10:
            raise ValueError('dense real-data smoke oracle failed')
        cells.extend(cell_for(panel, frame, t, save_fe) for t in THREADS)
    warm = dict(model='ols', design='one-fe', rows=512, effects=1, regressors=7, weights='unit')
    campaign.make_dataset(warm, root/'data/warm.dta')
    shutil.copy2(root/'data/warm.dta', root/'smoke-data/warm.dta')
    write_json(root/'manifest.json', dict(schema=1, panels={panel: panel_spec(panel) for panel, _ in selected},
               cells=cells, datasets=datasets,
               policy=dict(threads=THREADS, repetitions=REPETITIONS, command_timeout_seconds=600,
                           process_timeout_seconds=900, bundles=len(selected), save_fe=save_fe,
                           job_cleanup_seconds=13800 if save_fe else REUSE_CLEANUP_SECONDS,
                           scientific_tolerances={k: v for k, v in campaign.POLICY.items()
                                                  if k.endswith(('_rtol', '_atol'))},
                           interference='No resampling. Whole-host reservation and passive observation required.'),
               timed_fits=sum(len(c['arms']) for c in cells)*REPETITIONS))


def reuse(root, previous, save_fe=True, panels=None):
    """Schedule the current arms on a prior run's audited panel inputs.

    The datasets, smoke witnesses, retained-input record and policy are copied
    byte for byte; only the per-cell arm lists, the saved-FE request and the
    optional panel subset are rebuilt. Without saved FE, commands time
    estimation and inference only.
    """
    old = json.loads((previous/'manifest.json').read_text())
    for folder in ['data', 'smoke-data']:
        shutil.copytree(previous/folder, root/folder)
    (root/'input').mkdir(exist_ok=True)
    shutil.copy2(previous/'input/prior-panels.json', root/'input/prior-panels.json')
    for name, identity in old['datasets'].items():
        folder, panel = ('smoke-data', name[6:]) if name.startswith('smoke-') else ('data', name)
        matches = list((root/folder).glob(f'ols-veneto-{panel}-n*.dta'))
        if len(matches) != 1 or sha256(matches[0]) != identity['sha256']:
            raise ValueError('prior panel input changed')
    selected = [panel for panel in old['panels'] if panel in (panels or old['panels'])]
    if not selected or set(panels or []) - set(selected):
        raise ValueError('unknown panel subset')
    cells = [dict(c, **{k: v for k, v in cell_for(c['design'][7:], pd.DataFrame(), c['threads'], save_fe).items()
                        if k in ('id', 'arms', 'save_fe')}) for c in old['cells'] if c['design'][7:] in selected]
    write_json(root/'manifest.json', dict(old, cells=cells, reused_inputs=str(previous),
               panels={panel: old['panels'][panel] for panel in selected},
               policy=dict(old['policy'], save_fe=save_fe, job_cleanup_seconds=REUSE_CLEANUP_SECONDS),
               timed_fits=sum(len(c['arms']) for c in cells)*REPETITIONS))


def audit_prior(frame, prior):
    """Check retained identities and undo only fevc's recorded nuisance fit.

    Existing fevc inputs retain adjusted wages. Their difference from the raw
    response must lie in precisely the declared control span; this checks the
    rebuilt input against retained evidence without refitting the sample.
    """
    prior = prior.sort_values('observation_key').reset_index(drop=True)
    for current, old in [('obs_id', 'observation_key'), ('g1', 'worker'), ('g2', 'firm')]:
        if not np.array_equal(frame[current].to_numpy(), prior[old].to_numpy()):
            raise ValueError('rebuilt panel identities differ from retained fevc input')
    names = [c for c in frame if c.startswith('x')]
    X = frame[names].to_numpy(dtype=float)
    difference = frame.y.to_numpy()-prior.y_adjusted.to_numpy()
    beta, _, rank, _ = np.linalg.lstsq(X, difference, rcond=None)
    residual = difference-X@beta
    maximum = float(np.max(np.abs(residual)))
    if rank != len(names) or maximum > 1e-12:
        raise ValueError('rebuilt wages do not reproduce the retained fevc control adjustment')
    return dict(rows=len(frame), exact_identifiers=True, control_rank=int(rank),
                adjustment_max_error=maximum, adjustment_coefficients=beta.tolist())


def audit_inputs(root):
    previous = json.loads((root/'input/prior-panels.json').read_text())
    panels = json.loads((root/'manifest.json').read_text())['panels']
    results = {}
    for panel in panels:
        path = Path(previous[panel]['path'])
        if sha256(path) != previous[panel]['sha256']:
            raise ValueError('retained panel input hash differs')
        frame = pd.read_stata(root/'data'/f"ols-veneto-{panel}-n{panel_spec(panel)['rows']}.dta", convert_categoricals=False)
        if panel in PANELS:
            results[panel] = audit_prior(frame, pd.read_csv(path, float_precision='round_trip'))
        else:
            rebuilt = prepare_panel(path, panel)
            if list(rebuilt) != list(frame) or not all(np.array_equal(rebuilt[c].to_numpy(), frame[c].to_numpy())
                                                         for c in rebuilt):
                raise ValueError('rebuilt panel differs from the scheduled input')
            results[panel] = dict(rows=len(frame), rebuilt_from_source=True)
    write_json(root/'environment/input-audit.json', results)
    print('PANEL_INPUT_AUDIT_PASS', flush=True)


def effects_capture(record, frame, save_fe=True):
    """Retain both FE channels and a comparable, observation-weighted gauge.

    Individual levels are identified up to one constant on this connected graph.
    Centering each channel makes both worker and firm effects comparable without
    asserting equality between the estimators' raw normalization conventions.
    Without saved FE only the scientific output hashes are retained.
    """
    directory = Path(record['directory'])
    if save_fe:
        rows = pd.read_csv(directory/'rows.csv', na_values=['.'], float_precision='round_trip')
        effect_hashes, effects = {}, {}
        for j in [1, 2]:
            values = rows[f'saved{j}'].to_numpy(dtype=float)
            effect_hashes[f'saved{j}'] = hashlib.sha256(values.astype('<f8').tobytes()).hexdigest()
            codes = frame[f'g{j}'].to_numpy(dtype=int)-1
            counts = np.bincount(codes)
            levels = np.bincount(codes, weights=values)/counts
            effects[f'saved{j}'] = levels - np.average(levels, weights=counts)
        np.savez_compressed(directory/'effects.npz', **effects)
        record['capture']['saved_fe_sha256'] = effect_hashes
    record['capture']['scientific_sha256'] = {
        name: sha256(directory/name) for name in ['coefficients.csv', 'covariance.csv', 'rows.csv']}
    # Compress outside the measured command; complete raw output stays on SCC.
    with (directory/'rows.csv').open('rb') as source, gzip.open(directory/'rows.csv.gz', 'wb', compresslevel=1) as dest:
        shutil.copyfileobj(source, dest)
    (directory/'rows.csv').unlink()
    write_json(directory/'record.json', record)


def compare(a, b, cell):
    result = campaign.compare_records(a, b, cell)
    if a['status'] == b['status'] == 'ok' and cell['save_fe']:
        x, y = [np.load(Path(r['directory'])/'effects.npz') for r in [a, b]]
        for name in ['saved1', 'saved2']:
            if not close_values(x[name], y[name], campaign.POLICY['fitted_rtol'], campaign.POLICY['fitted_atol']):
                result['reasons'].append('centered '+name)
        result['eligible'] = not result['reasons']
        result.update(campaign.agreement(a['capture'], b['capture'], result['reasons']))
    return result


def run(root, task, smoke):
    campaign.verify_runtime(root)
    manifest = json.loads((root/'manifest.json').read_text())
    scheduled = list(manifest['panels'])
    selected = scheduled if smoke else [scheduled[task-1]]
    label = 'smoke' if smoke else f'task-{task}'
    taskroot = root/label
    taskroot.mkdir(exist_ok=False)
    (taskroot/'data').symlink_to(root/('smoke-data' if smoke else 'data'), target_is_directory=True)
    for name in ['source', 'environment']:
        (taskroot/name).symlink_to(root/name, target_is_directory=True)
    (taskroot/'output').mkdir()
    records, comparisons, science = [], [], []
    def checkpoint():
        write_json(taskroot/'results.json', dict(records=records, comparisons=comparisons,
                                                science=science, complete=False))
    started = time.monotonic()
    monitor = HostMonitor(root/('host-'+label), 16) if not smoke else None
    # Smoke exercises the real entrypoint with four slots. Production proves
    # the whole-host reservation before timing any estimator.
    from contextlib import nullcontext
    with monitor if monitor else nullcontext():
        cpus = campaign.physical_cpus() if not smoke else campaign.socket_ordered_cpus(
            Path('/proc/cpuinfo').read_text(), os.sched_getaffinity(0))
        if len(cpus) < (4 if smoke else 16):
            raise ValueError('insufficient physical cores')
        for panel in selected:
            specification = panel_spec(panel)
            if smoke:
                matches = list((root/'smoke-data').glob(f'ols-veneto-{panel}-n*.dta'))
            else:
                matches = [root/'data'/f"ols-veneto-{panel}-n{specification['rows']}.dta"]
            if len(matches) != 1:
                raise ValueError('ambiguous panel input')
            path = matches[0]
            frame = pd.read_stata(path, convert_categoricals=False)
            identity = json.loads(path.with_suffix('.json').read_text())
            if sha256(path) != identity['sha256']:
                raise ValueError('panel input changed')
            rank = identity['topology']['first_pair_rank']
            threads = [4] if smoke else THREADS
            reference = None
            for t in threads:
                cell = cell_for(panel, frame, t, manifest['policy'].get('save_fe', True))
                baseline, failed = {}, set()
                for rep in range(1, (1 if smoke else REPETITIONS)+1):
                    current = {}
                    order = cell['arms'][rep % len(cell['arms']):]+cell['arms'][:rep % len(cell['arms'])]
                    for arm in order:
                        if arm in failed:
                            records.append(dict(cell=cell['id'], arm=arm, repetition=rep,
                                                status='skipped-after-failure', metadata={}))
                            continue
                        if time.monotonic()-started+campaign.POLICY['process_timeout_seconds'] > manifest['policy']['job_cleanup_seconds']:
                            raise RuntimeError('fixed campaign budget exhausted; partial evidence retained')
                        if monitor:
                            monitor.check()
                        record = campaign.fit(taskroot, cell, arm, rep, frame, cpus)
                        if record['status'] == 'ok':
                            try:
                                effects_capture(record, frame, cell['save_fe'])
                                # OLS absorbed rank includes the global intercept;
                                # FixedEffectModels.jl counts every level instead.
                                expected = sum(identity['topology']['levels']) if arm in campaign.JULIA_ARMS else rank
                                if record['capture']['absorbed_dof'] != expected:
                                    raise ValueError('independent graph rank differs')
                                if arm == 'fereg' and float(record['metadata'].get('fereg_threads', 'nan')) != t:
                                    raise ValueError('native worker budget is unverified')
                            except (ValueError, OSError, KeyError) as error:
                                record.update(status='capture-error', error=str(error))
                                write_json(Path(record['directory'])/'record.json', record)
                        records.append(record)
                        current[arm] = record
                        checkpoint()
                        if record['status'] != 'ok':
                            failed.add(arm)
                            if arm == 'fereg':
                                write_json(taskroot/'results.json', dict(records=records, comparisons=comparisons, science=science, complete=False))
                                raise RuntimeError('native panel failure; do not resample')
                        elif arm == 'fereg':
                            if rep == 1:
                                baseline[arm] = record
                            else:
                                exact = record['capture'] == baseline[arm]['capture']
                                science.append(dict(panel=panel, threads=t, repetition=rep, kind='fixed-worker-exact', passed=exact))
                                checkpoint()
                                if not exact:
                                    raise RuntimeError('native fixed-worker science changed; do not resample')
                            if reference is None:
                                reference = record
                            agreement = compare(reference, record, cell)
                            science.append(dict(panel=panel, threads=t, repetition=rep, kind='cross-worker', **agreement))
                            checkpoint()
                            if not agreement['eligible']:
                                raise RuntimeError('native cross-worker science failed; do not resample')
                        write_json(taskroot/'results.json', dict(records=records, comparisons=comparisons, science=science, complete=False))
                    for arm, other in current.items():
                        if arm != 'fereg':
                            comparisons.append(dict(cell=cell['id'], arm=arm, repetition=rep,
                                                    **compare(current['fereg'], other, cell)))
                    if smoke:
                        expected = oracle(frame, cell)
                        for arm, record in current.items():
                            checks = {}
                            if record['status'] == 'ok':
                                capture = record['capture']
                                checks['coefficients'] = close_values(capture['beta'], expected['beta'], 1e-7, 1e-10)
                                checks['covariance'] = close_values(capture['covariance'], expected['covariance'], 1e-6, 1e-10)
                                if cell['save_fe']:
                                    fitted = np.load(Path(record['directory'])/'fitted.npz')['fitted']
                                    checks['fitted'] = close_values(fitted, expected['fitted'], 1e-7, 1e-9)
                            science.append(dict(panel=panel, arm=arm, kind='dense-oracle', checks=checks,
                                                passed=bool(checks) and all(checks.values())))
                            if arm == 'fereg' and not (checks and all(checks.values())):
                                raise RuntimeError('independent real-panel native smoke failed')
                del baseline
            if sha256(path) != identity['sha256']:
                raise ValueError('panel input changed during timing')
    campaign.verify_runtime(root)
    expected_count = smoke_fits(scheduled) if smoke else sum(len(c['arms']) for c in manifest['cells'] if c['design'] == 'veneto-'+selected[0])*REPETITIONS
    result = dict(records=records, comparisons=comparisons, science=science, complete=len(records) == expected_count,
                  expected_fits=expected_count, host=socket.gethostname(), job=os.environ['JOB_ID'],
                  task=os.environ.get('SGE_TASK_ID'), native_gate_pass=True)
    write_json(taskroot/'results.json', result)
    if not result['complete']:
        raise RuntimeError('panel campaign coverage incomplete')
    (taskroot/'COMPLETE').write_text('PANEL_TASK_COMPLETE\n')
    print('PANEL_TASK_COMPLETE', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['prepare', 'reuse', 'audit', 'smoke', 'run'])
    parser.add_argument('root', type=Path)
    parser.add_argument('--long', type=Path)
    parser.add_argument('--short', type=Path)
    parser.add_argument('--long-lcs', type=Path, help='prepare: the largest-connected-set long panel')
    parser.add_argument('--previous', type=Path, help='prior run whose audited inputs are reused')
    parser.add_argument('--without-saved-fe', action='store_true',
                        help='prepare/reuse: time estimation and inference without saving FE')
    parser.add_argument('--panels', nargs='+', choices=list(PANELS),
                        help='reuse: schedule only these panels, one production task each')
    parser.add_argument('--task', type=int, choices=[1, 2], default=1)
    args = parser.parse_args()
    if args.mode == 'prepare':
        prepare(args.root, args.long, args.short, args.long_lcs, not args.without_saved_fe)
    elif args.mode == 'reuse':
        reuse(args.root, args.previous, not args.without_saved_fe, args.panels)
    elif args.mode == 'audit':
        audit_inputs(args.root)
    else:
        run(args.root, args.task, args.mode == 'smoke')
