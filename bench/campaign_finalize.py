#!/usr/bin/env python3
"""Collect, report and export a finished revision campaign inside an SCC compute job.

  campaign_finalize.py RUN_ROOT OUTPUT --prefix scc-YYYYMMDD-revision [--collector PATH]

Runs the run's own frozen bench tooling (RUN_ROOT/source/bench): campaign_collect
(with its scheduler and file commands executed on this node instead of through
`ssh scc`), campaign_report, campaign_export and the block summary. It then writes
the numerical deviations in the shape of the paper's outreach evidence
(PREFIX-outreach-deviations.json and PREFIX-coefficient-deviations.csv, with every repetition
and the unavailable positions), copies the context and qualification receipt, and
writes OUTPUT/export/SHA256SUMS. The run root is only read. --collector loads a
later campaign_collect.py (for example one with a collection fix) in place of the
run's own; PREFIX-finalize.json records the driver, collector, job and host.
"""
from __future__ import annotations
import argparse
import csv
import hashlib
import importlib.util
import json
import os
import socket
from pathlib import Path
import shutil
import subprocess
import sys

DROP = {'coefficients', 'separate_prediction_diagnostic'}
PREDICTION = ['prediction_common_N', 'prediction_max_absolute_difference', 'prediction_rmse', 'prediction_relative_rmse',
              'sample_intersection', 'reference_only', 'comparator_only', 'replayed_objective_difference',
              'replayed_objective_difference_per_observation']
COEFFICIENT_FIELDS = ['dataset', 'cell', 'block', 'arm', 'reference_arm', 'repetition', 'name', 'difference',
                      'difference_in_reference_se', 'reference_se', 'se', 'se_difference', 'se_relative_difference']


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as stream:
        for block in iter(lambda: stream.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def local(command):
    """campaign_collect.remote, executed on this compute node."""
    return subprocess.run(['bash', '-lc', command], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def dataset(cell):
    return f"veneto-{cell['model']}" if cell.get('section') == 'veneto' else 'synthetic'


def deviations(campaign, prefix, output):
    cells = {c['id']: c for c in campaign['manifest']['cells']}
    records = {(r['cell'], r['arm'], r['repetition']): r for r in campaign['records']}
    rows, coefficients, sources = [], [], []
    for i, task in enumerate(campaign['tasks'], 1):
        sources.append(dict(task=i, host=task.get('host'), results_sha256=sha256(output/'collection'/f'task-{i}/results.json')))
        for item in task.get('numerical_comparisons', []):
            cell = cells.get(item['cell'])
            if cell is None:
                continue
            record = records.get((item['cell'], item['arm'], item['repetition']), {})
            row = dict(cell=cell.get('source_cell', cell['id']), block=cell['id'], block_family=cell.get('block_family'),
                       arm=item['arm'], **{k: cell[k] for k in ['model', 'design', 'threads', 'rows', 'save_fe', 'vce', 'regressors']},
                       repetition=cell.get('block_repetition', item['repetition']), reference_arm=item['reference_arm'],
                       reference_profile=('tighter' if item['reference_arm'].endswith('-strict') else 'default')
                       + ' fereg in the same allocation; package-default inference',
                       status=item['status'], seconds=record.get('metadata', {}).get('seconds'), dataset=dataset(cell),
                       host=item.get('host'), job=item.get('job'),
                       **{k: v for k, v in item.items() if k not in DROP | {'cell', 'arm', 'repetition', 'reference_arm',
                                                                             'status', 'host', 'job'}})
            diagnostic = item.get('separate_prediction_diagnostic')
            if diagnostic:
                row['prediction_source'] = 'separate saved-effect diagnostic refit'
                row['diagnostic_matches_timed_coefficients'] = item.get('diagnostic_matches_timed_coefficients')
                for key in PREDICTION:
                    if row.get(key) is None and diagnostic.get(key) is not None:
                        row[key] = diagnostic[key]
            elif row.get('prediction_rmse') is not None:
                row['prediction_source'] = 'timed fit'
            rows.append(row)
            for c in item.get('coefficients', []):
                coefficients.append(dict(dataset=row['dataset'], cell=row['cell'], block=row['block'], arm=row['arm'],
                                         reference_arm=row['reference_arm'], repetition=row['repetition'],
                                         **{k: c.get(k) for k in COEFFICIENT_FIELDS[7:] + ['name']}))
    unavailable = []
    for r in campaign['records']:
        cell = cells[r['cell']]
        if r['arm'] in ('fereg', 'fereg-strict') or 'numerical_capture' in r:
            continue
        unavailable.append(dict(cell=cell.get('source_cell', cell['id']), block=cell['id'], arm=r['arm'],
                                repetition=cell.get('block_repetition', r['repetition']), status=r['status'],
                                error=r.get('error', r.get('metadata', {}).get('error')), dataset=dataset(cell)))
    coefficient_path = output/'export'/f'{prefix}-coefficient-deviations.csv'
    with coefficient_path.open('w', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=COEFFICIENT_FIELDS)
        writer.writeheader()
        writer.writerows(coefficients)
    document = dict(schema=1,
                    description='Every repetition of the November 10 revision campaign: each comparator relative to fereg '
                                'in the same comparison block and allocation (tighter fereg when it completed, otherwise '
                                'default). Package-default inference; raw covariance. Predictions of unsaved timing profiles '
                                'come from the separate saved-effect diagnostic refit of repetition 1.',
                    reference='fereg in the same block; not an independent accuracy truth',
                    sources=sources, comparisons=rows, unavailable=unavailable,
                    coefficient_deviations_sha256=sha256(coefficient_path))
    path = output/'export'/f'{prefix}-outreach-deviations.json'
    path.write_text(json.dumps(document, indent=2, allow_nan=False)+'\n')
    return document


def main(root, output, prefix, collector=None):
    sys.path.insert(0, str(root/'source/bench'))
    collector = Path(collector) if collector else root/'source/bench/campaign_collect.py'
    spec = importlib.util.spec_from_file_location('campaign_collect', collector)
    campaign_collect = importlib.util.module_from_spec(spec)
    sys.modules['campaign_collect'] = campaign_collect
    spec.loader.exec_module(campaign_collect)
    import campaign_report, campaign_export
    campaign_collect.remote = local
    output.mkdir(parents=False, exist_ok=False)
    subprocess.run(['sha256sum', '-c', '--quiet', 'input/SHA256SUMS'], cwd=root, check=True)
    context = json.loads((root/'context.json').read_text())
    context['retained_files'] = ['qualification.json', 'input/SHA256SUMS', 'input/policy.json', 'input/source.json',
                                 'input/frozen-inputs.json', 'input/additional-cells.json', 'input/additional-smoke-cells.json',
                                 'logs/production.submission', 'logs/qualification.sge', 'logs/expanded-environment.log',
                                 'logs/revision-inputs.log']
    (output/'collect-context.json').write_text(json.dumps(context, indent=2)+'\n')
    collection = campaign_collect.collect(output/'collect-context.json', output/'collection')
    campaign = campaign_report.load_campaign(collection)
    campaign_report.render(campaign, campaign_report.summarize(campaign), output/'report')
    metadata = campaign_export.export(collection, output/'report', output/'export', prefix)
    document = deviations(campaign, prefix, output)
    shutil.copy2(collection/'context.json', output/'export'/f'{prefix}-context.json')
    shutil.copy2(collection/'qualification.json', output/'export'/f'{prefix}-qualification.json')
    (output/'export'/f'{prefix}-finalize.json').write_text(json.dumps(dict(
        run_root=str(root), source_commit=json.loads((root/'input/source.json').read_text())['commit'],
        driver_sha256=sha256(Path(__file__)), collector=str(collector), collector_sha256=sha256(collector),
        run_collector_sha256=sha256(root/'source/bench/campaign_collect.py'),
        job=os.environ.get('JOB_ID'), host=socket.gethostname(),
        scheduling_changes=json.loads((collection/'context.json').read_text()).get('scheduling_changes', [])),
        indent=2)+'\n')
    files = sorted(p for p in (output/'export').iterdir() if p.is_file() and p.name != 'SHA256SUMS')
    (output/'export/SHA256SUMS').write_text(''.join(f'{sha256(p)}  {p.name}\n' for p in files))
    subprocess.run(['sha256sum', '-c', '--quiet', 'SHA256SUMS'], cwd=output/'export', check=True)
    statuses = metadata['validation']['statuses']
    print(json.dumps(dict(records=len(campaign['records']), blocks=len(campaign['manifest']['cells']),
                          statuses=statuses, comparisons=len(document['comparisons']),
                          unavailable=len(document['unavailable']), exports=[p.name for p in files]), indent=1))
    print('CAMPAIGN_FINALIZE_COMPLETE')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--collector', type=Path)
    args = parser.parse_args()
    main(args.root, args.output, args.prefix, args.collector)
