"""Recheck every native protection pair and record exact-result boundaries.

This complements, rather than replaces, independent inference/output oracles.
Numerical tolerances and exact status/DoF rules are the existing runner rules.
Missing and failed attempts remain in the result instead of disappearing.
"""
import argparse
import json
from pathlib import Path

import numpy as np
import pandas as pd

import run
from score import read, sha


EXPORTS = ['coefficients.csv', 'covariance.csv', 'b.csv', 'V.csv',
           'fereg_fit.csv', 'fereg_dof.csv']


def audit(root, label, output):
    assert not output.exists()
    plan_path = root / (label + '-plan.json')
    plan = read(plan_path)
    assert plan['inventory_kind'] == 'controls' and len(plan['variants']) == 2
    progress_path = root / (label + '-progress.json')
    progress = read(progress_path)
    attempts = {item['index']: item for item in progress}
    assert len(attempts) == len(progress)
    records = []
    for index, cell in enumerate(plan['cells']):
        attempt = attempts.get(index)
        record = dict(index=index, cell=cell, pairs=[], status='not_attempted')
        records.append(record)
        if attempt is None:
            continue
        assert attempt['cell'] == cell
        for rep in range(1, plan['repetitions'] + 1):
            paths = [list((root / 'runs').glob(f"{attempt['label']}-*-r{rep}-{variant}/result.json"))
                     for variant in plan['variants']]
            pair = dict(rep=rep, status='missing_application')
            record['pairs'].append(pair)
            if not all(len(found) == 1 for found in paths):
                continue
            paths = [found[0] for found in paths]
            results = [read(path) for path in paths]
            pair['result_sha256'] = {variant: sha(path) for variant, path in zip(plan['variants'], paths)}
            pair['application_status'] = [result['status'] for result in results]
            if not all(result['status'] == 'pass' and result['sentinel']
                       and result['process_returncode'] == 0 for result in results):
                pair['status'] = 'application_failure'
                continue
            a, b = [path.parent for path in paths]
            try:
                assert results[0]['input_sha256'] == results[1]['input_sha256']
                assert results[0]['cell'] == results[1]['cell']
                pair['science'] = run.profile.compare(a, b, exact_native=False)
                for name in ['fereg_status.csv', 'fereg_dof.csv']:
                    np.testing.assert_array_equal(pd.read_csv(a / name), pd.read_csv(b / name))
                mode = results[0]['cell']['outputs']
                pair['outputs'] = run.compare_outputs(a, b, cell['n'], mode, effect_count=len(cell['effects']))
                names = EXPORTS + (['outputs.bin'] if mode != 'none' else [])
                pair['exact_exports'] = {name: sha(a / name) == sha(b / name) for name in names}
                pair['status'] = 'pass'
            except (AssertionError, ValueError, KeyError, OSError) as error:
                pair.update(status='mismatch', detail=str(error))
        record['status'] = 'pass' if all(pair['status'] == 'pass' for pair in record['pairs']) else 'incomplete_or_mismatch'
        print(index, record['status'], flush=True)
    receipt = dict(schema='native-protection-science-v1',
        controller_sha256=sha(Path(__file__)), runner_sha256=sha(Path(run.__file__)),
        numerical_comparator_sha256=sha(Path(run.profile.__file__)),
        plan_sha256=sha(plan_path), progress_sha256=sha(progress_path),
        planned=len(plan['cells']), passing=sum(record['status'] == 'pass' for record in records),
        records=records)
    output.write_text(json.dumps(receipt, indent=2, sort_keys=True, allow_nan=False) + '\n')
    print('NATIVE_PROTECTION_AUDIT', receipt['passing'], '/', receipt['planned'], flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('root', type=Path)
    p.add_argument('label')
    p.add_argument('output', type=Path)
    a = p.parse_args()
    audit(a.root, a.label, a.output)
