"""Audit complete expanded-graph cells independently, retaining incomplete cells."""
import argparse
import json
from pathlib import Path

import graph_oracle
from score import read, sha


EXPORTS = ['coefficients.csv', 'covariance.csv', 'b.csv', 'V.csv', 'fereg_fit.csv', 'fereg_dof.csv']


def audit(root, label, output):
    assert not output.exists()
    plan_path = root / (label + '-plan.json'); plan = read(plan_path)
    progress_path = root / (label + '-progress.json'); progress = read(progress_path)
    records, oracles = [], {}
    for index, cell in enumerate(plan['cells']):
        attempt = progress[index] if index < len(progress) else None
        record = dict(index=index, cell=cell, attempts=[], repeated_native_exports=[], status='not_attempted')
        if not attempt:
            records.append(record); continue
        assert attempt['cell'] == cell and attempt['index'] == index
        reference_planned = cell['weight_kind'] != 'fw' and ('variants' not in plan or 'fixest' in plan['variants'])
        variants = [plan['baseline'], plan['candidate']] + (['fixest'] if reference_planned else [])
        for variant in variants:
            runs = sorted((root / 'runs').glob(f"{attempt['label']}-*-{variant}/result.json"))
            passing = [p for p in runs if read(p)['status'] == 'pass']
            if not passing:
                record['attempts'].append(dict(variant=variant, status='no_successful_application'))
                continue
            first = passing[0]; result = read(first); data = Path(read(first.parent / 'request.json')['data'])
            key = (data, cell['weight_kind'], result['cell']['vce'])
            if key not in oracles: oracles[key] = graph_oracle.solve(*key)
            checked = graph_oracle.compare(first, oracles[key]); checked['variant'] = variant
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
        record['status'] = 'native_pass' if len(native) == 2 and all(r['status'] == 'pass' for r in native) \
            and len(record['repeated_native_exports']) == 2 * plan['repeats'] \
            and all(r['status'] == 'pass' for r in record['repeated_native_exports']) else 'native_incomplete_or_mismatch'
        records.append(record)
        print(index, record['status'], [r['status'] for r in record['attempts']], flush=True)
        output.write_text(json.dumps(dict(schema='expanded-graph-independent-audit-v1',
            plan_sha256=sha(plan_path), progress_sha256=sha(progress_path),
            oracle_source_sha256=sha(Path(graph_oracle.__file__)), controller_sha256=sha(Path(__file__)),
            complete=False, records=records), indent=2, sort_keys=True) + '\n')
    receipt = read(output); receipt.update(complete=True, records=records,
        planned=len(plan['cells']), native_complete=sum(r['status'] == 'native_pass' for r in records))
    output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    print('EXPANDED_GRAPH_INDEPENDENT_AUDIT', receipt['native_complete'], '/', receipt['planned'], flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('root', type=Path)
    p.add_argument('label'); p.add_argument('output', type=Path)
    a = p.parse_args(); audit(a.root, a.label, a.output)
