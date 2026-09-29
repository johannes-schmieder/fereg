"""Audit complete expanded-graph cells independently, retaining incomplete cells."""
import argparse
import json
from pathlib import Path

import graph_oracle
from score import read, sha


EXPORTS = ['coefficients.csv', 'covariance.csv', 'b.csv', 'V.csv', 'fereg_fit.csv', 'fereg_dof.csv']


def load_oracle_cache(path):
    receipt = read(path)
    assert receipt['complete'] and receipt['native_complete'] == receipt['planned']
    assert len(receipt['records']) == receipt['planned']
    assert receipt['oracle_source_sha256'] == sha(Path(graph_oracle.__file__))
    cache = {}
    for record in receipt['records']:
        assert record['status'] == 'native_pass'
        for checked in record['attempts']:
            if checked['variant'] == 'fixest':
                continue
            assert checked['status'] == 'pass'
            oracle = checked['oracle']
            key = (checked['input_sha256'], record['cell']['weight_kind'], oracle['vce'])
            assert cache.setdefault(key, oracle) == oracle
    assert cache
    return cache


def audit(root, label, output, oracle_cache_path=None):
    assert not output.exists()
    plan_path = root / (label + '-plan.json'); plan = read(plan_path)
    progress_path = root / (label + '-progress.json'); progress = read(progress_path)
    repetitions = plan['repeats'] if 'repeats' in plan else plan['repetitions']
    assert repetitions > 0
    planned_variants = plan.get('variants')
    if planned_variants is None:
        planned_variants = [plan['baseline'], plan['candidate'], 'fixest']
    assert len(set(planned_variants)) == len(planned_variants)
    native_variants = [v for v in planned_variants if v != 'fixest']
    assert native_variants
    records, oracles, oracle_origins, checked_inputs = [], {}, {}, {}
    cached_oracles = load_oracle_cache(oracle_cache_path) if oracle_cache_path else {}
    for index, cell in enumerate(plan['cells']):
        attempt = progress[index] if index < len(progress) else None
        record = dict(index=index, cell=cell, attempts=[], application_counts={},
                      repeated_native_exports=[], status='not_attempted')
        if not attempt:
            records.append(record); continue
        assert attempt['cell'] == cell and attempt['index'] == index
        variants = [v for v in planned_variants if v != 'fixest' or cell['weight_kind'] != 'fw']
        for variant in variants:
            runs = sorted((root / 'runs').glob(f"{attempt['label']}-*-{variant}/result.json"))
            passing = [p for p in runs if read(p)['status'] == 'pass']
            record['application_counts'][variant] = dict(planned=repetitions, attempted=len(runs), passing=len(passing))
            if not passing:
                record['attempts'].append(dict(variant=variant, status='no_successful_application'))
                continue
            first = passing[0]; result = read(first); data = Path(read(first.parent / 'request.json')['data'])
            key = (data, cell['weight_kind'], result['cell']['vce'])
            if oracle_cache_path:
                if data not in checked_inputs:
                    checked_inputs[data] = sha(data)
                assert checked_inputs[data] == result['input_sha256']
            if key not in oracles:
                cached = cached_oracles.get((result.get('input_sha256'), cell['weight_kind'], result['cell']['vce']))
                if cached is not None:
                    # The cache stores independently solved equations, never
                    # native FE state. Verify the actual original input before
                    # applying that same oracle to another artifact's exports.
                    oracles[key] = cached
                    oracle_origins[key] = 'verified_cache'
                else:
                    oracles[key] = graph_oracle.solve(*key)
                    oracle_origins[key] = 'fresh_solve'
            checked = graph_oracle.compare(first, oracles[key]); checked['variant'] = variant
            checked['oracle_origin'] = oracle_origins[key]
            record['attempts'].append(checked)
            if variant != 'fixest':
                exports = EXPORTS + (['outputs.bin'] if result['cell']['outputs'] != 'none' else [])
                # Export bytes include coefficient order, sample and all output
                # values. Runtime, allocation and solver diagnostics vary.
                reference = {name: sha(first.parent / name) for name in exports}
                for path in passing:
                    equal = {name: sha(path.parent / name) == digest for name, digest in reference.items()}
                    equal['sample'] = read(path)['sample_sha256'] == result['sample_sha256']
                    record['repeated_native_exports'].append(dict(run=path.parent.name, reference=first.parent.name,
                        checks=equal, status='pass' if all(equal.values()) else 'mismatch'))
        native = [r for r in record['attempts'] if r['variant'] != 'fixest']
        record['status'] = 'native_pass' if len(native) == len(native_variants) and all(r['status'] == 'pass' for r in native) \
            and all(record['application_counts'][v]['attempted'] == record['application_counts'][v]['passing'] == repetitions
                    for v in native_variants) \
            and len(record['repeated_native_exports']) == len(native_variants) * repetitions \
            and all(r['status'] == 'pass' for r in record['repeated_native_exports']) else 'native_incomplete_or_mismatch'
        records.append(record)
        print(index, record['status'], [r['status'] for r in record['attempts']], flush=True)
        output.write_text(json.dumps(dict(schema='expanded-graph-independent-audit-v3',
            plan_sha256=sha(plan_path), progress_sha256=sha(progress_path),
            oracle_source_sha256=sha(Path(graph_oracle.__file__)), controller_sha256=sha(Path(__file__)),
            native_variants=native_variants, repetitions=repetitions,
            oracle_cache_sha256=sha(oracle_cache_path) if oracle_cache_path else None,
            complete=False, records=records), indent=2, sort_keys=True) + '\n')
    receipt = read(output); receipt.update(complete=True, records=records,
        planned=len(plan['cells']), native_complete=sum(r['status'] == 'native_pass' for r in records))
    output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    print('EXPANDED_GRAPH_INDEPENDENT_AUDIT', receipt['native_complete'], '/', receipt['planned'], flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('root', type=Path)
    p.add_argument('label'); p.add_argument('output', type=Path)
    p.add_argument('--oracle-cache', type=Path)
    a = p.parse_args(); audit(a.root, a.label, a.output, a.oracle_cache)
