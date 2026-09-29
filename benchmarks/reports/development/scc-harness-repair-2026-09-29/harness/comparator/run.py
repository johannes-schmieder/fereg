"""Bounded comparator diagnosis; reconstructed inference is never stock output."""
import argparse
from gtools_check import fixture_passed
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import traceback

import numpy as np
import pandas as pd
import pyfixest as pf


def main():
    parser = argparse.ArgumentParser()
    for name in ('run-root', 'reference', 'scratch'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    root, reference = args.run_root.resolve(), args.reference.resolve()
    sys.path[:0] = [str(root / 'benchmarks/systematic'), str(root / 'scripts')]
    from common import read_json, write_json, sha256
    from generate import generate
    from two_fe_ppml_oracle import fit
    from two_fe_direct_oracle import solve
    from test_two_fe_ppml_oracle import validate
    from ppml_coefficient_accuracy import two_fe_coefficient_accuracy, Decimal, LOW, HIGH, point
    from ppml_auto_science import compare_optimum_intervals

    assert os.environ['NSLOTS'] == '4'
    plan = read_json(root / 'registration.json')
    for name, digest in read_json(root / 'deployment.json').items():
        assert sha256(root / name) == digest, name
    frozen = read_json(reference / 'env/frozen.json')
    assert frozen['fingerprint'] == plan['original_fingerprint']
    bindings = {}
    for name, digest in frozen['runtime'].items():
        if ('site-packages/pyfixest/' in name or name.startswith('references/')
                or name == 'env/python-freeze.txt'):
            assert sha256(reference / name) == digest, name
            bindings[name] = digest
    assert sha256(root / 'benchmarks/systematic/generate.py') == frozen['controller']['generate.py']
    for name, digest in plan['original_receipts'].items():
        assert sha256(reference / name) == digest, name
    manifest = read_json(reference / 'input/manifest.json')
    assert manifest['science'] == plan['science']
    output = root / 'results'
    output.mkdir(exist_ok=False)
    args.scratch.mkdir(exist_ok=False)
    write_json(output / 'bindings.json', dict(runtime=bindings, job=os.environ['JOB_ID'],
        deployment=sha256(root / 'deployment.json'), registration=sha256(root / 'registration.json'),
        pyfixest_version=pf.__version__, python=sys.version.split()[0],
        thread_environment={k: os.environ.get(k) for k in ('OMP_NUM_THREADS', 'RAYON_NUM_THREADS',
                                                         'NUMBA_NUM_THREADS', 'OPENBLAS_NUM_THREADS')}))
    write_json(output / 'oracle-tests.json', validate())
    print('COMPARATOR_INDEPENDENT_ORACLES_PASS', flush=True)
    tolerance = plan['science']

    def error_units(value, expected, family):
        value, expected = np.asarray(value), np.asarray(expected)
        return float(np.max(np.abs(value - expected) /
                     (tolerance[family + '_atol'] + tolerance[family + '_rtol'] * np.abs(expected))))

    def assess(beta, covariance, se, metadata, sample, anchor, expected_sample, intervals):
        tokens = [format(float(b), '.17g') for b in beta]
        certificate = compare_optimum_intervals(beta, tokens, intervals,
            rtol=str(tolerance['coefficient_rtol']), atol=str(tolerance['coefficient_atol']))
        units = dict(coefficient=error_units(beta, anchor['beta'], 'coefficient'),
                     covariance=error_units(covariance, anchor['covariance'], 'vce'),
                     standard_error=error_units(se, np.sqrt(np.diag(anchor['covariance'])), 'vce'),
                     deviance=error_units(metadata['deviance'], anchor['deviance'], 'objective'))
        checks = dict(coefficients_certified=certificate['candidate_inside_optimum_envelope'],
                      covariance_point=units['covariance'] <= 1,
                      standard_errors_point=units['standard_error'] <= 1,
                      objective=units['deviance'] <= 1, exact_sample=sample == expected_sample,
                      exact_rank=metadata.get('df_k', metadata.get('rank', 0) + metadata.get('df_a', 0)) == anchor['rank'],
                      converged=bool(metadata['converged']))
        return dict(checks=checks, pass_all=all(checks.values()), error_envelope_units=units,
                    coefficient_certificate=certificate)

    def ids_hash(ids):
        return hashlib.sha256(np.asarray(ids, dtype='<i8').tobytes()).hexdigest()

    def covariance_at(y, X, g, h, mu):
        # Independent direct WLS at the supplied current mean, never a stored
        # comparator absorption or score. This is a diagnostic, not an export fix.
        geometry = solve(np.zeros(len(y)), X, g, h, weights=mu)
        Xp, bread = geometry['projected_X'], geometry['bread']
        influence = ((y - mu)[:, None] * Xp) @ bread
        return influence.T @ influence * len(y) / (len(y) - 1), geometry

    conclusions = {}
    for case in plan['cases']:
        folder = output / case['id']
        folder.mkdir()
        cell = case['cell']
        data = args.scratch / (case['id'] + '.dta')
        info = generate(cell, case['seed'], data, manifest)
        if case.get('original'):
            original_info = read_json(reference / case['original'] / 'input.json')
            assert info == original_info, case['id']
        write_json(folder / 'input.json', info)
        frame = pd.read_stata(data, convert_categoricals=False)
        names = [f'x{j}' for j in range(1, cell['k'] + 1)]
        y, X = frame.y.to_numpy(), frame[names].to_numpy()
        g = frame.g1.to_numpy()
        h = frame.g2.to_numpy() if 'g2' in frame else np.ones(len(y), dtype=int)
        assert (y > 0).all() and min(info['minimum_level_rows'].values()) >= 2
        groups = np.column_stack((g, h))
        anchor = fit(y, X, g, h)
        proof = two_fe_coefficient_accuracy(y, X, anchor['beta'], anchor['mu'], groups)
        write_json(folder / 'optimum-certificate.json', proof)
        assert proof['coefficient_accuracy_certified']
        radius = Decimal(proof['coefficient_radius_upper'])
        intervals = [(str(LOW.subtract(point(b)[0], radius)), str(HIGH.add(point(b)[1], radius)))
                     for b in anchor['beta']]
        write_json(folder / 'optimum-intervals.json', intervals)
        np.savez(folder / 'anchor.npz', beta=anchor['beta'], mu=anchor['mu'], covariance=anchor['covariance'])
        write_json(folder / 'oracle.json', {key: value.tolist() if isinstance(value, np.ndarray) else value
                                         for key, value in anchor.items() if key != 'mu'})
        fresh_V, _ = covariance_at(y, X, g, h, anchor['mu'])
        np.testing.assert_allclose(fresh_V, anchor['covariance'], rtol=1e-9, atol=1e-12)
        expected_sample = ids_hash(frame.obs_id)
        result = dict(historical={}, profiles={})
        if case.get('original'):
            for arm in case['original_arms']:
                historical = reference / case['original'] / arm
                receipt = read_json(historical / 'result.json')
                assert receipt['status'] == 'pass' and receipt['input_sha256'] == info['sha256']
                coefficients = pd.read_csv(historical / 'coefficients.csv').set_index('name').loc[names]
                V = pd.read_csv(historical / 'covariance.csv').pivot(index='row', columns='column', values='value').loc[names, names].to_numpy()
                result['historical'][arm] = assess(coefficients.value.to_numpy(), V, coefficients.se,
                    receipt['metadata'], receipt['sample_sha256'], anchor, expected_sample, intervals)
        for profile, settings in plan['profiles'].items():
            profile_folder = folder / profile
            profile_folder.mkdir()
            kwargs = dict(vcov='hetero', ssc=pf.ssc(k_adj=False))
            if 'iwls_tol' in settings:
                kwargs['iwls_tol'] = settings['iwls_tol']
            if 'fixef_tol' in settings:
                kwargs['demeaner'] = pf.MapDemeaner(fixef_tol=settings['fixef_tol'])
            formula = 'y ~ ' + ' + '.join(names) + ' | ' + ' + '.join(cell['effects'])
            try:
                estimator = pf.fepois(formula, data=frame, **kwargs)
                assert list(estimator.coef().index) == names
                assert estimator._N == len(y) and np.array_equal(estimator._data.obs_id, frame.obs_id)
                beta = estimator.coef().to_numpy()
                correction = len(y) / (len(y) - 1)
                V = np.asarray(estimator._vcov) * correction
                mu = np.asarray(estimator._Y_hat_response).ravel()
                W = np.asarray(estimator._irls_weights).ravel()
                assert mu.shape == W.shape == y.shape and (W > 0).all() and (mu > 0).all()
                V_fresh, current_geometry = covariance_at(y, X, g, h, mu)
                stored_geometry = solve(np.zeros(len(y)), X, g, h, weights=W)
                Xp = np.asarray(estimator._X) / np.sqrt(W)[:, None]
                scores = np.asarray(estimator._scores)
                bread = np.asarray(estimator._bread)
                replay = bread @ (scores.T @ scores) @ bread * correction
                Xp_current = current_geometry['projected_X']
                Xp_old = stored_geometry['projected_X']
                # Identify whether stored inference is internally consistent at
                # the previous frame and quantify its departure from fresh work.
                H_old = Xp_old.T @ (W[:, None] * Xp_old)
                H_current = Xp_current.T @ (mu[:, None] * Xp_current)
                H = np.asarray(estimator._hessian)
                old_linearized_score = ((y - W) - W * (np.log(mu) - np.log(W)))[:, None] * Xp_old
                old_bread = stored_geometry['bread']
                old_linearized_V = old_bread @ (old_linearized_score.T @ old_linearized_score) @ old_bread * correction
                relative = lambda observed, expected: float(np.linalg.norm(observed - expected) /
                                                           max(np.linalg.norm(expected), np.finfo(float).tiny))
                diagnostics = dict(
                    ssc=np.asarray(estimator._ssc).tolist(), normalization=correction,
                    stored_weight_vs_current_mean_max_relative=float(np.max(abs(W - mu) / mu)),
                    current_mean_vs_anchor_max_relative=float(np.max(abs(mu - anchor['mu']) / anchor['mu'])),
                    stored_projection_vs_direct_old_frame_relative=relative(Xp, Xp_old),
                    stored_hessian_vs_direct_old_frame_relative=relative(H, H_old),
                    stored_hessian_vs_direct_current_frame_relative=relative(H, H_current),
                    stored_score_vs_direct_old_linearized_relative=relative(scores, old_linearized_score),
                    covariance_vs_direct_old_linearized_relative=relative(V, old_linearized_V),
                    stored_score_vs_fresh_current_score_relative=relative(scores, (y - mu)[:, None] * Xp_current),
                    covariance_replay_max_abs=float(np.max(abs(V - replay))),
                    stored_se_vs_anchor_units=error_units(np.sqrt(np.diag(V)), np.sqrt(np.diag(anchor['covariance'])), 'vce'),
                    fresh_current_se_vs_anchor_units=error_units(np.sqrt(np.diag(V_fresh)), np.sqrt(np.diag(anchor['covariance'])), 'vce'),
                    fresh_current_covariance_vs_anchor_units=error_units(V_fresh, anchor['covariance'], 'vce'))
                metadata = dict(deviance=float(estimator.deviance), converged=bool(estimator.convergence),
                                df_k=float(estimator._df_k), df_t=float(estimator._df_t), N=len(y))
                assessed = assess(beta, V, estimator.se().to_numpy() * np.sqrt(correction), metadata,
                                  ids_hash(estimator._data.obs_id), anchor, expected_sample, intervals)
                result['profiles'][profile] = dict(application='pass', metadata=metadata,
                                                    assessment=assessed, diagnostics=diagnostics)
                np.savez(profile_folder / 'state.npz', beta=beta, mu=mu, weights=W,
                         covariance=V, covariance_fresh=V_fresh, hessian=H)
                print('COMPARATOR_PROFILE_COMPLETE', case['id'], profile, assessed['pass_all'], diagnostics, flush=True)
            except Exception as error:
                result['profiles'][profile] = dict(application='failure', error=f'{type(error).__name__}: {error}')
                traceback.print_exc()
            write_json(folder / 'comparison.json', result)
        conclusions[case['id']] = result
        write_json(output / 'comparison.json', conclusions)

    # Small licensed fixture exercises gtools' own optional e-class publication.
    # Reference files are copied because Stata packages may compile Mata helpers.
    refs = args.scratch / 'references'
    shutil.copytree(reference / 'references', refs)
    gtools_dir = output / 'gtools'
    gtools_dir.mkdir()
    command = ['stata-mp', '-b', 'do', str(root / 'gtools_sample.do'), str(refs), str(gtools_dir)]
    run = subprocess.run(command, cwd=gtools_dir, timeout=180, capture_output=True, text=True)
    (gtools_dir / 'process.txt').write_text(run.stdout + run.stderr)
    log = gtools_dir / 'gtools.log'
    passed = fixture_passed(gtools_dir, run.returncode)
    write_json(gtools_dir / 'receipt.json', dict(process_returncode=run.returncode, passed=passed,
               log_sha256=sha256(log) if log.exists() else None))
    assert passed, 'gtools licensed fixture failed; inspect its preserved log'
    applications = [v for case in conclusions.values() for v in case['profiles'].values()]
    write_json(output / 'complete.json', dict(diagnostic_complete=True, cases=len(conclusions),
               pyfixest_applications=len(applications),
               pyfixest_application_failures=sum(v['application'] != 'pass' for v in applications),
               gtools_fixture_passed=passed, timing_eligible=False))
    print('COMPARATOR_STATE_DIAGNOSTIC_COMPLETE', flush=True)


if __name__ == '__main__':
    main()
