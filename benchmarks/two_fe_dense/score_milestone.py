"""Score the expanded graph matrix without dropping failures or FW controls."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import statistics

from score import peak, read, sha


def score(root, label):
    plan_path = root / (label + '-plan.json'); plan = read(plan_path)
    assert plan['schema'] == 'ols-graph-coverage-execution-v1' and plan['phase'] != 'small'
    progress_path = root / (label + '-progress.json')
    progress = read(progress_path) if progress_path.exists() else []
    assert [r['index'] for r in progress] == list(range(len(progress)))
    candidate, baseline = plan['candidate'], plan['baseline']
    cells, identities, references = [], {}, set()
    for index, cell in enumerate(plan['cells']):
        attempt = progress[index] if index < len(progress) else None
        comparator = baseline if cell['weight_kind'] == 'fw' else 'fixest'
        item = dict(index=index, cell=cell, comparator=comparator, issues=[], pairs=[], comparator_pairs=[],
            timing_isolation=attempt.get('timing_isolation', {}).get('status', 'unrecorded') if attempt else 'not_attempted',
            scientific_comparison=attempt['comparison_status'] if attempt else 'not_attempted')
        if not attempt:
            item['issues'].append('not_attempted'); cells.append(item); continue
        assert attempt['cell'] == cell and attempt['label'] == f'{label}-c{index:04d}'
        if attempt['process_returncode']: item['issues'].append('cell_process_failure')
        variants = [baseline, candidate] + ([] if comparator == baseline else ['fixest'])
        for rep in range(1, plan['repeats'] + 1):
            rows = {}
            for variant in variants:
                matches = list((root / 'runs').glob(f"{attempt['label']}-*-r{rep}-{variant}/result.json"))
                if len(matches) != 1:
                    item['issues'].append(f'rep{rep}:{variant}:result_count={len(matches)}'); continue
                path = matches[0]; result = read(path); request = read(path.parent / 'request.json')
                if result['status'] != 'pass' or result['process_returncode'] or not result['sentinel'] or not result['seconds'] > 0:
                    item['issues'].append(f'rep{rep}:{variant}:application_failure'); continue
                assert request['solver'] == cell['solver']
                for key in ['design', 'n', 'k', 'threads', 'seed', 'shuffled', 'weight_kind', 'weight_design', 'case', 'effects']:
                    assert result['cell'][key] == cell[key], (path, key)
                assert result['cell']['fit_options'].endswith(f"memory({cell['memory']})")
                fingerprint = result['measurement_fingerprint']
                if variant == 'fixest':
                    assert result['cell']['fixest_tolerance'] == 1e-6 and result['cell']['fixest_recovery_tolerance'] == 1e-5
                    reference = {k: fingerprint[k] for k in ['fixest_archive_sha256', 'fixest_library']}
                    references.add(hashlib.sha256(json.dumps(reference, sort_keys=True).encode()).hexdigest())
                else:
                    identity = fingerprint['native']; assert identity['abi'] == 'pass'
                    assert identities.setdefault(variant, identity) == identity
                rows[variant] = (path, result)
            if not rows: continue
            assert len({r['input_sha256'] for _, r in rows.values()}) == 1
            assert len({r['sample_sha256'] for _, r in rows.values()}) == 1
            assert all(r['sample_rows'] == cell['n'] for _, r in rows.values())
            first = next(iter(rows.values()))[1]
            assert all(r['cell'] == first['cell'] for _, r in rows.values())
            # A failed baseline remains a failed native control. It must not
            # erase an independently complete candidate/reference timing pair.
            if candidate in rows and comparator in rows:
                compared = {v: rows[v] for v in [candidate, comparator]}
                seconds = {v: r['seconds'] for v, (_, r) in compared.items()}
                item['comparator_pairs'].append(dict(rep=rep, seconds=seconds,
                    ratio=seconds[candidate] / seconds[comparator],
                    input_sha256=rows[candidate][1]['input_sha256'],
                    result_hashes={v: sha(p) for v, (p, _) in compared.items()}))
            if set(rows) != set(variants): continue
            seconds = {v: r['seconds'] for v, (_, r) in rows.items()}
            item['pairs'].append(dict(rep=rep, seconds=seconds, ratio=seconds[candidate] / seconds[comparator],
                native_ratio=seconds[candidate] / seconds[baseline],
                native_peaks={v: peak(rows[v][0].parent) for v in [baseline, candidate]},
                input_sha256=rows[candidate][1]['input_sha256'],
                result_hashes={v: sha(p) for v, (p, _) in rows.items()}))
        if len(item['pairs']) == plan['repeats'] and not item['issues']:
            assert len({r['input_sha256'] for r in item['pairs']}) == 1
            item['median_paired_ratio'] = statistics.median(r['ratio'] for r in item['pairs'])
            item['median_native_ratio'] = statistics.median(r['native_ratio'] for r in item['pairs'])
            item['median_seconds'] = {v: statistics.median(r['seconds'][v] for r in item['pairs']) for v in variants}
        if len(item['comparator_pairs']) == plan['repeats']:
            assert len({r['input_sha256'] for r in item['comparator_pairs']}) == 1
            item['median_comparator_ratio'] = statistics.median(r['ratio'] for r in item['comparator_pairs'])
        cells.append(item)
    measured = [c for c in cells if 'median_paired_ratio' in c]
    comparator_complete = [c for c in cells if 'median_comparator_ratio' in c]
    comparable = [c for c in comparator_complete if c['comparator'] == 'fixest'
                  and c['timing_isolation'] == 'no_detected_overlap']
    assert len(references) <= 1, 'reference identity changed within the block'
    return dict(schema='expanded-graph-score-v2', label=label, phase=plan['phase'],
        planned=len(cells), completely_measured=len(measured),
        comparator_complete=len(comparator_complete), comparable_timing_eligible=len(comparable),
        plan_sha256=sha(plan_path), progress_sha256=sha(progress_path) if progress_path.exists() else None,
        identity_audit_complete=len(measured) == len(cells) and len(references) == 1,
        native_plugins={v: i['plugin'] for v, i in identities.items()}, reference_identities=sorted(references),
        comparable_outcomes=dict(Counter('win' if c['median_comparator_ratio'] < 1 else 'tie' if c['median_comparator_ratio'] == 1 else 'loss' for c in comparable)),
        fw_native_controls=sum(c['comparator'] == baseline for c in measured),
        scientific_acceptance='Requires separate independent checks and reference adjudication.', cells=cells)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('root', type=Path)
    p.add_argument('label'); p.add_argument('output', type=Path)
    a = p.parse_args(); assert not a.output.exists()
    result = score(a.root, a.label); a.output.write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    print('EXPANDED_GRAPH_SCORE', result['planned'], result['completely_measured'], result['comparable_outcomes'], flush=True)
