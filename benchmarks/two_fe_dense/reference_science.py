"""Separate small-graph reference accuracy diagnostics; never headline timings."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

import broad


def main(a):
    summary = json.loads(a.summary.read_text())
    cells = [c for c in summary['cells'] if c['cell']['case'] == 'cluster2_cross_all'
             and c['cell']['weight_kind'] != 'fw']
    assert len(cells) == 66
    a.root.mkdir(parents=True, exist_ok=False)
    plan = []
    for item in cells[a.shard::a.shards]:
        old_root = a.inputs / item['root']
        reference = next((old_root / 'runs').glob(item['label'] + '-*-r1-fixest/result.json'))
        native = next((old_root / 'runs').glob(item['label'] + '-*-r1-' + summary['candidate'] + '/result.json'))
        old_ref = json.loads(reference.read_text())['metadata']
        old_native = json.loads(native.read_text())['metadata']
        full_dof = old_ref['N'] - old_ref['df_K_vcov'] != old_native['fereg_df_correction']
        # run.py independently checks that neither FE is nested before allowing
        # full-rank correction. Preserve any failed diagnostic as an attempt.
        plan.append(dict(**item, reference_full_dof=full_dof,
            default_reference_result_sha256=broad.sha(reference),
            default_native_result_sha256=broad.sha(native)))
    broad.write(a.root / 'diagnostic-plan.json', dict(cells=plan,
        summary_sha256=broad.sha(a.summary), runner_sha256=broad.sha(Path(__file__)),
        scope='66 graph/width/weight full-output representatives; not every VCE request',
        reference_tolerance=1e-10, timing='diagnostic_only'))
    progress = []
    for item in plan:
        label = 'accuracy-' + str(item['index'])
        command = [sys.executable, str(Path(__file__).with_name('run.py')), str(a.root),
            '--artifacts', str(a.artifacts), '--label', label,
            '--variants', summary['candidate'], 'fixest', '--repeats', '1', '--oracle',
            '--reference-precision', '--record-reference-science-disagreement',
            '--record-reference-output-disagreement', *broad.arguments(item['cell'])]
        if item['reference_full_dof']: command.append('--reference-full-dof')
        output = a.root / (label + '.stdout.txt')
        with output.open('w') as stream: code = subprocess.call(command, stdout=stream, stderr=subprocess.STDOUT)
        complete = a.root / (label + '-complete.json')
        status = json.loads(complete.read_text())['status'] if complete.exists() else 'incomplete'
        progress.append(dict(index=item['index'], label=label, process_returncode=code,
            status=status, stdout_sha256=broad.sha(output)))
        broad.write(a.root / 'diagnostic-progress.json', progress)
        print(label, code, status, flush=True)
    broad.write(a.root / 'diagnostic-complete.json', dict(status='attempts_complete',
        planned=len(plan), attempted=len(progress), process_failures=sum(r['process_returncode'] != 0 for r in progress)))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('root', type=Path); p.add_argument('--summary', required=True, type=Path)
    p.add_argument('--inputs', required=True, type=Path); p.add_argument('--artifacts', required=True, type=Path)
    p.add_argument('--shard', type=int, required=True); p.add_argument('--shards', type=int, default=4)
    a = p.parse_args(); assert 0 <= a.shard < a.shards; main(a)
