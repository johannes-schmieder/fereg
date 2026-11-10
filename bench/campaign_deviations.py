"""Numerical observations, independent of the campaign's pass/fail gates.

Row-level data never leave the run root. Compact exports contain coefficients,
SEs and aggregate deviations only. The reference is named, never assumed true.
"""
from __future__ import annotations
import hashlib
from pathlib import Path
import numpy as np
import pandas as pd
from scipy.special import xlogy, gammaln


def finite(value):
    try:
        value = float(value)
    except (ValueError,TypeError):
        return None
    return value if np.isfinite(value) else None


def raw_capture(directory, cell, frame, metadata, output_directory=None):
    directory = Path(directory)
    b = pd.read_csv(directory/'coefficients.csv', float_precision='round_trip')
    vpath = directory/'covariance-stock.csv'
    if not vpath.exists(): vpath = directory/'covariance.csv'
    v = pd.read_csv(vpath, float_precision='round_trip')
    names = [f'x{i}' for i in range(1, cell.get('regressors', 16)+1)]
    beta = dict(zip(b.name, b.value))
    covariance = {(r.row, r.column): r.value for r in v.itertuples()}
    # Keep finite estimable slots; report omitted and nonfinite slots separately.
    active = [n for n in names if n in beta and np.isfinite(beta[n])
              and np.isfinite(covariance.get((n,n), np.nan)) and covariance[n,n] >= 0]
    V = np.array([[covariance.get((a,c),np.nan) for c in active] for a in active])
    row_path = directory/'rows.csv'
    if not row_path.exists(): row_path = directory/'rows.csv.gz'
    rows = pd.read_csv(row_path, na_values=['.'], float_precision='round_trip')
    if not np.array_equal(rows.obs_id.to_numpy(), frame.obs_id.to_numpy()):
        raise ValueError('numerical capture: original row identities changed')
    if not np.isin(rows.retained, [0,1]).all(): raise ValueError('invalid sample indicator')
    selected = rows.retained.to_numpy() == 1
    result = dict(names=active, beta=[finite(beta[n]) for n in active],
                  covariance=[[finite(x) for x in row] for row in V],
                  standard_errors=[finite(np.sqrt(covariance[n,n])) for n in active],
                  omitted_or_nonfinite=[n for n in names if n not in active],
                  N=int(selected.sum()), input_N=len(frame),
                  sample_sha256=hashlib.sha256(rows.obs_id.to_numpy(dtype='<i8')[selected].tobytes()).hexdigest(),
                  covariance_source=vpath.name,
                  inference_policy=metadata.get('inference_policy','legacy-harmonized'),
                  reported_objective=finite(metadata.get('rss' if cell['model']=='ols' else 'll',np.nan)),
                  objective_kind='rss' if cell['model']=='ols' else 'loglik-with-constants',
                  reported_absorbed_dof=finite(metadata.get('df_a',np.nan)),
                  residual_dof=finite(metadata.get('df_r',np.nan)))
    # Arrays are retained privately; comparisons may include differing samples.
    arrays = dict(retained=selected, obs_id=rows.obs_id.to_numpy())
    if 'fitted' in rows:
        prediction = rows.fitted.to_numpy(float)
        arrays['prediction'] = prediction
        arrays['outcome'] = frame.y.to_numpy(float)
        if np.isfinite(prediction[selected]).all():
            y, p = arrays['outcome'][selected], prediction[selected]
            objective = np.sum((y-p)**2) if cell['model']=='ols' else np.sum(xlogy(y,p)-p-gammaln(y+1))
            result['replayed_objective'] = finite(objective)
    destination=Path(output_directory) if output_directory else directory
    destination.mkdir(parents=True,exist_ok=True)
    np.savez_compressed(destination/'numerical-rows.npz', **arrays)
    return result


def compare(reference, other, reference_directory=None, other_directory=None):
    """Compact actual differences; unlike an agreement gate, never drops a fit."""
    common = [n for n in reference['names'] if n in other['names']]
    ri = [reference['names'].index(n) for n in common]
    oi = [other['names'].index(n) for n in common]
    a = np.array(reference['beta'],float)[ri]; b = np.array(other['beta'],float)[oi]
    sa = np.array(reference['standard_errors'],float)[ri]; sb = np.array(other['standard_errors'],float)[oi]
    d = b-a
    standardized = np.divide(d,sa,out=np.full(len(d),np.nan),where=sa>0)
    se_relative = np.divide(sb-sa,sa,out=np.full(len(d),np.nan),where=sa>0)
    result = dict(reference_N=reference['N'], comparator_N=other['N'],
                  same_sample=reference['sample_sha256']==other['sample_sha256'],
                  reference_sample_sha256=reference['sample_sha256'], comparator_sample_sha256=other['sample_sha256'],
                  reference_omitted=reference['omitted_or_nonfinite'], comparator_omitted=other['omitted_or_nonfinite'],
                  coefficients=[dict(name=n,reference_beta=finite(a[j]),beta=finite(b[j]),
                          difference=finite(d[j]),difference_in_reference_se=finite(standardized[j]),
                          reference_se=finite(sa[j]),se=finite(sb[j]),se_difference=finite(sb[j]-sa[j]),
                          se_relative_difference=finite(se_relative[j])) for j,n in enumerate(common)])
    def maximum(x): return finite(np.max(np.abs(x))) if len(x) and np.isfinite(x).all() else None
    result['coefficient_max_reference_se'] = maximum(standardized)
    result['se_max_relative_difference'] = maximum(se_relative)
    if common:
        A=np.array(reference['covariance'],float)[np.ix_(ri,ri)]
        B=np.array(other['covariance'],float)[np.ix_(oi,oi)]
        result['covariance_max_absolute_difference']=maximum((B-A).ravel())
        norm=np.linalg.norm(A)
        result['covariance_relative_frobenius']=finite(np.linalg.norm(B-A)/norm) if norm>0 else None
        # Descriptive scalar only: do not call it a DoF explanation without formulas.
        scale=np.sum(A*B)/np.sum(A*A) if norm>0 else np.nan
        result['covariance_best_scalar']=finite(scale)
        result['covariance_residual_after_scalar']=finite(np.linalg.norm(B-scale*A)/norm) if norm>0 else None
    if result['same_sample'] and reference['objective_kind']==other['objective_kind']:
        for key in ['reported_objective','replayed_objective']:
            a,b=reference.get(key),other.get(key)
            if a is not None and b is not None:
                result[key+'_difference']=finite(b-a)
                result[key+'_difference_per_observation']=finite((b-a)/max(1,reference['N']))
    if reference_directory and other_directory:
        paths=[Path(p)/'numerical-rows.npz' for p in [reference_directory,other_directory]]
        if all(p.exists() for p in paths):
            with np.load(paths[0]) as a, np.load(paths[1]) as b:
                if not np.array_equal(a['obs_id'],b['obs_id']): raise ValueError('different observation identities')
                intersection=a['retained']&b['retained']
                result.update(sample_intersection=int(intersection.sum()),
                              reference_only=int((a['retained']&~b['retained']).sum()),
                              comparator_only=int((b['retained']&~a['retained']).sum()))
                if 'prediction' in a and 'prediction' in b:
                    p,q=a['prediction'][intersection],b['prediction'][intersection]
                    if len(p) and np.isfinite(p).all() and np.isfinite(q).all():
                        delta=q-p; rmse=np.sqrt(np.mean(delta*delta)); scale=np.sqrt(np.mean(p*p))
                        result.update(prediction_common_N=len(p),prediction_max_absolute_difference=maximum(delta),
                                      prediction_rmse=finite(rmse),prediction_relative_rmse=finite(rmse/scale) if scale>0 else None)
    return result


def comparisons(records):
    """Every repetition; prefer the same-repetition tighter native reference."""
    by={(r['cell'],r['arm'],r['repetition']):r for r in records}
    out=[]
    for r in records:
        if 'numerical_capture' not in r: continue
        ref=by.get((r['cell'],'fereg-strict',r['repetition']))
        if not ref or ref['status']!='ok' or 'numerical_capture' not in ref:
            ref=by.get((r['cell'],'fereg',r['repetition']))
        if not ref or ref['status']!='ok' or 'numerical_capture' not in ref: continue
        item=dict(cell=r['cell'],arm=r['arm'],repetition=r['repetition'],reference_arm=ref['arm'],
                  reference_status=ref['status'],status=r['status'],host=r.get('host'),job=r.get('job'),
                  **compare(ref['numerical_capture'],r['numerical_capture'],ref['directory'],r['directory']))
        rd,od=ref.get('prediction_diagnostic'),r.get('prediction_diagnostic')
        if rd and od and 'numerical_capture' in rd and 'numerical_capture' in od:
            diag=compare(rd['numerical_capture'],od['numerical_capture'],rd['directory'],od['directory'])
            item['separate_prediction_diagnostic']=diag
            item['diagnostic_matches_timed_coefficients']=all(
                x['numerical_capture']['names']==y['numerical_capture']['names'] and
                x['numerical_capture']['omitted_or_nonfinite']==y['numerical_capture']['omitted_or_nonfinite'] and
                x['numerical_capture']['beta']==y['numerical_capture']['beta'] and
                x['numerical_capture']['sample_sha256']==y['numerical_capture']['sample_sha256']
                for x,y in [(ref,rd),(r,od)])
        out.append(item)
    return out
