"""Saved-export native thread consistency; never changes timing eligibility."""
import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import itertools
import json
from pathlib import Path
import sys


def main():
    parser = argparse.ArgumentParser()
    for name in ('run', 'report', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    bindings = {}
    def read(path):
        data = path.read_bytes()
        key = str(path.relative_to(args.run)) if path.is_relative_to(args.run) else 'report/' + path.name
        bindings[key] = hashlib.sha256(data).hexdigest()
        return data.decode()
    manifest = json.loads(read(args.run / 'input/manifest.json'))
    frozen = json.loads(read(args.run / 'env/frozen.json'))
    source = json.loads(read(args.run / 'input/source.json'))
    assert source['snapshot'] == 'c09eeab353a8445aae85f75891f112964246e876c4865ec9e2f757d83922e236'
    assert json.loads(read(args.report / 'manifest.json')) == manifest
    assert json.loads(read(args.report / 'provenance.json')) == frozen
    for name in ['runner.py', 'common.py', 'light.py']:
        assert hashlib.sha256(read(args.run / 'controller' / name).encode()).hexdigest() == frozen['controller'][name]
    sys.path.insert(0, str(args.run / 'controller'))
    from runner import compare
    families = defaultdict(dict)
    runs = list(csv.DictReader(read(args.report / 'runs.csv').splitlines()))
    attempted, unavailable = 0, Counter()
    for row in runs:
        if row['arm'] not in ['fereg', 'feppml'] or row['phase'] != 'warm':
            continue
        attempted += 1
        if row['status'] != 'pass':
            unavailable[row['status']] += 1
            continue
        cell_id = '{model}-{design}-n{n}-k{k}-t{threads}-{section}'.format(**row)
        path = args.run / 'results' / cell_id / ('warm-' + row['repetition']) / row['arm']
        receipt = json.loads(read(path / 'result.json'))
        cell = receipt['cell']
        assert receipt['status'] == 'pass' and receipt['arm'] == row['arm']
        assert receipt['measurement_fingerprint'] == frozen['fingerprint']
        assert all(str(cell[key]) == row[key] for key in ['model', 'design', 'n', 'k', 'threads', 'section'])
        key = (json.dumps({k:v for k,v in cell.items() if k != 'threads'}, sort_keys=True), int(row['repetition']))
        assert cell['threads'] not in families[key]
        families[key][cell['threads']] = (path, receipt)
        for filename in ['coefficients.csv', 'covariance.csv']:
            read(path / filename)
    pairs = []
    for (family, rep), settings in sorted(families.items()):
        for lower, higher in itertools.combinations(sorted(settings), 2):
            left, a = settings[lower]
            right, b = settings[higher]
            comparison = compare(left, right, manifest['science'])
            # Cross-node execution is the object of this scientific diagnostic.
            # Keep the frozen comparison verbatim; do not promote either clock.
            checks = {k:v for k,v in comparison['checks'].items() if k != 'same_node_task'}
            required = ['same_input', 'same_runtime', 'sample_count', 'sample_exact',
                        'coefficients', 'standard_errors', 'covariance',
                        'candidate_converged', 'comparator_converged']
            checks['required_checks_present'] = all(k in checks for k in required)
            fields = ['N', 'N_full', 'num_singletons', 'df_a', 'df_m', 'rank']
            if a['cell']['model'] == 'ols':
                fields += ['df_r']
            else:
                fields += ['df_resid', 'df_structural', 'feppml_model_rank', 'feppml_rank_frozen']
            for field in fields:
                checks[field] = a['metadata'].get(field) is not None and a['metadata'].get(field) == b['metadata'].get(field)
            prefix = 'fereg' if a['cell']['model'] == 'ols' else 'feppml'
            selected = [r['metadata'].get(prefix + '_threads') for r in [a,b]]
            pairs.append(dict(cell=json.loads(family), repetition=rep,
                requested_threads=[lower,higher], selected_threads=selected,
                locations=[{k:r.get(k) for k in ['host','job_id','task_id']} for r in [a,b]],
                frozen_comparison=comparison, science_checks=checks,
                science_passed=all(checks.values()),
                coefficient_export_identical=(left/'coefficients.csv').read_bytes() == (right/'coefficients.csv').read_bytes(),
                covariance_export_identical=(left/'covariance.csv').read_bytes() == (right/'covariance.csv').read_bytes()))
    result = dict(source_fingerprint=frozen['fingerprint'], source_snapshot=source['snapshot'],
        analyzer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        scope='Regressor-only saved b/SE/full-V and scalar/sample consistency across requested threads; no intercept or row-output coverage, independent optimum proof, timing eligibility or causal scaling claim.',
        native_warm_attempts_registered=attempted, unavailable=dict(unavailable),
        pairs=pairs, pair_count=len(pairs), science_passed=sum(p['science_passed'] for p in pairs),
        bindings=bindings)
    with args.output.open('x') as stream:
        json.dump(result,stream,indent=2,allow_nan=False)
        stream.write('\n')
    print(json.dumps({k:result[k] for k in ['pair_count','science_passed','unavailable']},indent=2))
    print(json.dumps([p for p in pairs if not p['science_passed']],indent=2))


if __name__ == '__main__':
    main()
