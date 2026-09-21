"""Independently certify route attempts before comparing whole-command times."""
import argparse
import csv
import json
from pathlib import Path
import statistics

import audit_outputs
import graph_oracle
from score import read, sha


def audit(root, output):
    assert not output.exists(), 'Preserve previous route audits.'
    plan = read(root / 'route-plan.json')
    progress = read(root / 'route-progress.json')
    candidate = plan['candidate']
    seen, inputs, plugins = set(), {}, set()
    oracles, records = {}, []
    for attempt in progress:
        index, family, rep = attempt['index'], attempt['family'], attempt['rep']
        assert 0 <= index < len(plan['cells']) and family in plan['families']
        assert 1 <= rep <= plan['repeats'] and (index, family, rep) not in seen
        seen.add((index, family, rep))
        assert attempt['cell'] == dict(plan['cells'][index], solver=family)
        label = attempt['label']
        matches = list((root / 'runs').glob(f'{label}-*-{candidate}/result.json'))
        assert len(matches) <= 1
        record = dict(index=attempt['index'], original_index=attempt['original_index'],
                      family=attempt['family'], rep=attempt['rep'], label=label,
                      isolation=attempt['timing_isolation']['status'], status='no_application_result')
        if matches:
            path = matches[0]
            result = read(path)
            request = read(path.parent / 'request.json')
            assert request['solver'] == attempt['family']
            for key in ['design', 'n', 'k', 'weight_kind', 'case', 'threads', 'seed', 'effects']:
                assert result['cell'][key] == attempt['cell'][key]
            record.update(application_status=result['status'], result_sha256=sha(path),
                          input_sha256=result['input_sha256'], sample_sha256=result.get('sample_sha256'),
                          plugin=result['measurement_fingerprint']['native']['plugin'],
                          seconds=result['seconds'])
            assert inputs.setdefault(index, result['input_sha256']) == result['input_sha256']
            plugins.add(record['plugin'])
            assert len(plugins) == 1, 'Candidate artifact changed during route comparison.'
            if result['status'] != 'pass':
                record['status'] = 'application_failure'
            else:
                assert result['process_returncode'] == 0 and result['sentinel']
                key = (Path(request['data']), result['cell']['weight_kind'], result['cell']['vce'])
                if key not in oracles:
                    oracles[key] = graph_oracle.solve(*key)
                record['inference'] = graph_oracle.compare(path, oracles[key])
                record['outputs'] = dict(status='not_requested')
                if result['cell']['outputs'] != 'none':
                    try:
                        audit_outputs.audit(root, label, candidate)
                        receipt = root / f'{label}-{candidate}-independent-outputs.json'
                        record['outputs'] = dict(status='pass', receipt_sha256=sha(receipt))
                    except AssertionError as error:
                        record['outputs'] = dict(status='mismatch', detail=str(error))
                record['status'] = 'science_pass' if record['inference']['status'] == 'pass' \
                    and record['outputs']['status'] in ('pass', 'not_requested') else 'science_mismatch'
                with (path.parent / 'fereg_solver.csv').open() as stream:
                    record['solver_report'] = list(csv.DictReader(stream))
                record['solver_report_sha256'] = sha(path.parent / 'fereg_solver.csv')
        records.append(record)
        print(label, record['status'], record['isolation'], flush=True)
        output.write_text(json.dumps(dict(complete=False, records=records), indent=2) + '\n')

    cells = []
    for index, cell in enumerate(plan['cells']):
        families = {}
        for family in plan['families']:
            runs = [r for r in records if r['index'] == index and r['family'] == family]
            valid = len(runs) == plan['repeats'] and all(r['status'] == 'science_pass'
                and r['isolation'] == 'no_detected_overlap' for r in runs)
            families[family] = dict(attempted=len(runs), valid=valid,
                median_seconds=statistics.median(r['seconds'] for r in runs) if valid else None)
        valid = {name: value['median_seconds'] for name, value in families.items() if value['valid']}
        fastest = min(valid, key=valid.get) if valid else None
        auto_ratio = valid['auto'] / valid[fastest] if fastest and 'auto' in valid else None
        cells.append(dict(index=index, cell=cell, families=families, fastest_valid=fastest,
                          auto_to_fastest_valid=auto_ratio))
    receipt = dict(schema='ols-whole-command-route-audit-v1', complete=True,
        attempted=len(records), planned=len(plan['cells']) * len(plan['families']) * plan['repeats'],
        plan_sha256=sha(root / 'route-plan.json'), progress_sha256=sha(root / 'route-progress.json'),
        inference_source_sha256=sha(Path(graph_oracle.__file__)),
        output_source_sha256=sha(Path(audit_outputs.__file__)), controller_sha256=sha(Path(__file__)),
        records=records, cells=cells,
        acceptance='Exploratory only; failed and possibly overlapped runs remain excluded from rankings.')
    output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    print('WHOLE_COMMAND_ROUTE_AUDIT_COMPLETE', len(records), flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('root', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    audit(a.root, a.output)
