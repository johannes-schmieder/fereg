"""Verify complete, disjoint coverage of the approved small graph matrix."""
import argparse
from collections import Counter
import json
from pathlib import Path

import broad
import milestone


def summarize(roots, candidate):
    expected = milestone.expand(json.loads(milestone.MANIFEST.read_text()), 'small')
    seen, cells, runs = set(), [], []
    oracle_counts = Counter()
    for root in roots:
        plan = json.loads((root / 'science-plan.json').read_text())
        progress = json.loads((root / 'science-progress.json').read_text())
        terminal = json.loads((root / 'science-complete.json').read_text())
        assert terminal['plan_sha256'] == broad.sha(root / 'science-plan.json')
        assert terminal['progress_sha256'] == broad.sha(root / 'science-progress.json')
        assert terminal['planned'] == terminal['attempted'] == len(progress) == len(plan['cells'])
        assert terminal['process_failures'] == 0
        assert plan['candidate'] == candidate and plan['phase'] == 'small'
        indices = plan['selected_indices'] or list(range(len(expected)))
        for index, record in zip(indices, progress):
            assert index not in seen and expected[index] == record['cell']
            seen.add(index)
            assert record['process_returncode'] == 0
            complete = json.loads((root / (record['label'] + '-complete.json')).read_text())
            assert complete['status'] == record['comparison_status']
            checks = complete['checks']
            native = [c for c in checks if c.get('variant') == candidate]
            assert len(native) == 1 and native[0]['science'].get('status') != 'mismatch'
            assert native[0]['science']['sample_exact']
            errors = native[0]['science']['errors'].values()
            output = native[0].get('outputs')
            if output: assert output['status'] == 'pass'
            found = set()
            for check in checks:
                if 'oracle' not in check: continue
                result = json.loads((root / 'runs' / check['run'] / 'result.json').read_text())
                variant = check['run'].split('-r1-')[-1]
                found.add(variant)
                assert result['status'] == 'pass' and result['process_returncode'] == 0 and result['sentinel']
                if variant != 'fixest': assert check['oracle']['status'] == 'pass'
                oracle_counts[(variant, check['oracle']['status'])] += 1
                runs.append(dict(root=root.name, run=check['run'], variant=variant,
                    result_sha256=broad.sha(root / 'runs' / check['run'] / 'result.json')))
            assert found == ({plan['baseline'], candidate} if record['cell']['weight_kind'] == 'fw'
                             else {plan['baseline'], candidate, 'fixest'})
            cells.append(dict(index=index, cell=record['cell'], root=root.name, label=record['label'],
                status=complete['status'], native_max_science_error=max(errors),
                native_max_output_error=max(output['max_abs_errors']) if output else None))
    assert seen == set(range(len(expected)))
    return dict(schema='complete-small-graph-science-v1', status='native_science_pass',
        manifest_sha256=broad.sha(milestone.MANIFEST), candidate=candidate, cells=sorted(cells, key=lambda c: c['index']),
        counts=dict(cells=len(cells), application_calls=len(runs), statuses=dict(Counter(c['status'] for c in cells))),
        oracle_counts=[dict(variant=k[0], status=k[1], count=v) for k, v in sorted(oracle_counts.items())],
        runs=runs, timing='diagnostic_only', reference_discrepancies='Retained; not accepted as scientific passes.')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('output', type=Path); p.add_argument('roots', nargs='+', type=Path)
    p.add_argument('--candidate', required=True)
    a = p.parse_args(); assert not a.output.exists()
    result = summarize(a.roots, a.candidate); broad.write(a.output, result)
    print('SMALL_GRAPH_SCIENCE_COMPLETE', result['counts'], flush=True)
