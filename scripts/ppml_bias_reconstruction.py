#!/usr/bin/env python3
"""Regression for a retained synthetic duration draw with tiny accepted means."""
import argparse
import json
from pathlib import Path

import numpy as np
import pandas as pd

from ppml_bias_oracle import correction, fit_poisson, randomized_correction, two_fe_design
from run_stata_tests import run_fixture, sha256_file


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--stata', required=True)
    for name in ('plugin', 'source-manifest', 'work'):
        p.add_argument('--' + name, type=Path, required=True)
    args = p.parse_args()
    root = Path(__file__).resolve().parents[1]
    fixture = root / 'tests/stata/bias/duration_reconstruction.csv.gz'
    if sha256_file(fixture) != '6412be729bc44fa2144afb3d453a412e573443df1dc7926cfd3076cdc1f205c0':
        raise ValueError('Frozen duration regression input changed')
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    data = pd.read_csv(fixture, float_precision='round_trip')
    data.to_stata(work / 'input.dta', write_index=False, version=118)
    lines = ['version 18.0', 'clear all', 'set more off', 'set processors 2']
    cases = []
    for ordering in range(4):
        for branch in range(3):
            for method in (['randomized', 'exact'] if ordering == 0 else ['randomized']):
                name = f'{ordering}-{branch}-{method}'
                use = 'sb_fits==1' + (f' & sb_branch=={branch}' if branch else '')
                command = f'feppml d x z if {use} [pw=sw], absorb(sb_sworker sb_sfirm) exposure(exposure) keepsingletons tolerance(1e-9) iterate(300) threads(1)'
                options = '' if method == 'exact' else f' biasprobes(256) biasseed(1411710131) biasstream({branch})'
                lines += [f'use "{work}/input.dta", clear']
                if ordering == 1:
                    lines += ['generate long reverse=-_n', 'sort reverse']
                elif ordering == 2:
                    lines += ['sort sb_sworker sb_sfirm worker seq']
                elif ordering == 3:
                    lines += ['sort sb_sfirm sb_sworker worker seq']
                lines += [command + ' mu(ordinary_mu)',
                          'matrix ordinary_b=e(b)', 'matrix ordinary_v=e(V)',
                          'generate byte ordinary_sample=e(sample)',
                          command + f' biascorrection({method}) biasunit(worker) biasfe(bf){options} mu(native_mu)',
                          'assert mreldif(ordinary_b,e(b))==0', 'assert mreldif(ordinary_v,e(V))==0',
                          'assert ordinary_sample==e(sample)', 'assert ordinary_mu==native_mu']
                for label, matrix in [('raw', 'b_bias_raw'), ('d', 'b_bias_d'), ('c', 'b_bias_c'), ('bc', 'b_bc')]:
                    for j, control in enumerate(['x', 'z'], 1):
                        lines += [f'generate double {label}_{control}=e({matrix})[1,{j}]']
                lines += ['keep if ordinary_sample', 'sort worker seq',
                          'format bf* native_mu raw_* d_* c_* bc_* %24.17g',
                          f'export delimited worker seq bf* native_mu raw_* d_* c_* bc_* using "{work}/{name}.csv", replace datafmt']
                cases.append((name, ordering, branch, method))
    lines += ['display "END_FIXTURE|bias_reconstruction|rc=0"']
    program = work / 'bias_reconstruction.do'
    program.write_text('\n'.join(lines) + '\n')
    result = run_fixture(args.stata, program, root, plugin=args.plugin,
                         plugin_sha256=sha256_file(args.plugin), log_dir=work / 'logs', require_status=True)
    if result.error or result.stata_rc != 0:
        print(result.log_text[-16000:])
        raise RuntimeError(result.error or f'Stata returned {result.stata_rc}')
    oracles = {}
    for branch in range(3):
        frame = data[data.sb_fits.eq(1) & (data.sb_branch.eq(branch) if branch else True)].sort_values(['worker', 'seq'])
        matrix, first, second = two_fe_design(frame)
        beta, mu, _ = fit_poisson(matrix, frame.d, frame.sw, frame.exposure)
        d, c = correction(matrix, frame.d, frame.sw, mu, frame.worker)
        dr, cr, _ = randomized_correction(matrix, frame.d, frame.sw, mu, frame.worker, 256, 1411710131, branch, batch=8)
        oracles[branch] = frame, first, second, beta, mu, {'exact': (d, c), 'randomized': (dr, cr)}
    records = []
    for name, ordering, branch, method in cases:
        frame, first, second, beta, mu, increments = oracles[branch]
        d, c = increments[method]
        actual = pd.read_csv(work / f'{name}.csv', float_precision='round_trip')
        np.testing.assert_array_equal(actual[['worker', 'seq']], frame[['worker', 'seq']])
        errors = {'mean': float(np.max(np.abs(actual.native_mu - mu)))}
        np.testing.assert_allclose(actual.native_mu, mu, rtol=1e-8, atol=1e-10)
        for label, vector in [('raw', beta), ('d', d), ('c', c), ('bc', beta+d+c)]:
            expected = np.column_stack([vector[:len(first)][np.searchsorted(first, frame.sb_sworker)],
                        np.r_[0., vector[len(first):-2]][np.searchsorted(second, frame.sb_sfirm)],
                        np.full(len(frame), vector[-2]), np.full(len(frame), vector[-1])])
            observed = actual[[f'bf_1_{label}', f'bf_2_{label}', f'{label}_x', f'{label}_z']]
            np.testing.assert_allclose(observed, expected, rtol=2e-7, atol=2e-9)
            errors[label] = float(np.max(np.abs(observed.to_numpy()-expected)))
        records.append(dict(case=name, rows=len(frame), minimum_mean=float(mu.min()), maximum_errors=errors))
    receipt = dict(schema=1, passed=True, source=json.loads(args.source_manifest.read_text()),
                   plugin_sha256=sha256_file(args.plugin), input_sha256=sha256_file(fixture),
                   script_sha256=sha256_file(Path(__file__)), fixture_sha256=sha256_file(program),
                   stata=result.metadata, cases=records)
    (work / 'summary.json').write_text(json.dumps(receipt, indent=2, sort_keys=True)+'\n')
    print(f'Passed {len(records)} duration reconstruction cases')


if __name__ == '__main__':
    main()
