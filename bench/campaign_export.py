#!/usr/bin/env python3
"""Export compact, committed evidence from a collected packed SCC campaign.

Writes PREFIX-summary.csv (one row per configuration and arm), PREFIX-observations.csv
(every planned position) and PREFIX-inputs.json (identities, policy, accounting,
host replay, validation counts and capture hashes). Inputs, row captures and
saved effects stay on SCC.
"""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

import pandas as pd

from campaign_report import load_campaign, summarize, check_native_repetitions, check_workers
from qualify_scc import validate_reservation, validate_census

FIELDS = ['cell', 'arm', 'repetition', 'status', 'host', 'job', 'task', 'process_rc', 'wall_seconds', 'seconds',
          'error', 'estimator_tree_rss_bytes', 'memory_sweeps', 'memory_exact', 'ready_rss_bytes', 'peak_rss_bytes',
          'done_rss_bytes', 'timed_out_rows']
# fereg's own phase clocks (OLS) and total; they may nest and must not be summed.
METADATA = ['preflight_seconds', 'input_seconds', 'native_seconds', 'total_seconds',
            'warm_seconds', 'iterations', 'tolerance', 'fereg_threads', 'feppml_threads', 'fixest_version',
            'pyfixest_version', 'pyfixest_commit', 'julia_version', 'fixedeffectmodels_version']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()).hexdigest()


def host_replay(root, manifest):
    hosts = []
    for label in ['smoke', *[f'task-{i}' for i in range(1, manifest['policy']['bundles']+1)]]:
        directory = root/('host-'+label)
        report = json.loads((directory/'host.json').read_text())
        if not report['isolation_ok'] or report['errors']:
            raise ValueError('host isolation failed: '+label)
        events = [json.loads(line) for line in (directory/'host-observations.jsonl').read_text().splitlines()]
        for event in events:
            if event['kind'] == 'reservation':
                validate_reservation(event['raw'], report['host'], max(manifest['policy']['threads']))
            elif event['kind'] == 'census':
                validate_census(event['raw']['job'], event['raw']['queues'], report['host'])
            elif event['kind'] == 'process':
                if event['competing_compute']:
                    raise ValueError('competing compute observed: '+label)
            else:
                raise ValueError('unexpected host observation: '+event['kind'])
        samples = [e for e in events if e['kind'] == 'process']
        gaps = [(b['monotonic_ns']-a['monotonic_ns'])/1e9 for a, b in zip(samples, samples[1:])]
        if max(gaps) > 7.5:
            raise ValueError('missing host sample: '+label)
        hosts.append(dict(label=label, host=report['host'], isolation_ok=True, scheduler_checks_replayed=True,
                          process_samples=len(samples), maximum_process_gap_s=max(gaps),
                          observations_sha256=digest(directory/'host-observations.jsonl')))
    return hosts


def export(root, report, output, prefix):
    campaign = load_campaign(root)
    summary = summarize(campaign)
    context, manifest = campaign['context'], campaign['manifest']
    repeats, workers = check_native_repetitions(campaign), check_workers(campaign)
    native = [r for r in campaign['records'] if r['arm'] == 'fereg']
    # A native command limit stays visible as a result; every other native failure stops the export.
    limited = {r['cell'] for r in native if r['status'] in ['command-timeout', 'timeout']}
    unexplained = [r for r in native if r['status'] != 'ok' and r['cell'] not in limited]
    if (unexplained or any(v for k, v in repeats.items() if k not in limited)
            or any(v for k, v in workers.items() if k not in limited)
            or any(r['status'] not in ['command-timeout', 'timeout', 'skipped-after-failure']
                   for r in native if r['cell'] in limited)):
        raise ValueError('native evidence failed; do not export or resample')
    hosts = host_replay(root, manifest)
    output.mkdir(parents=True, exist_ok=True)
    summary.to_csv(output/(prefix+'-summary.csv'), index=False)
    rows = []
    # Superseded cells keep their positions here; the summary omits them.
    for record in campaign['records']+campaign['superseded_records']:
        meta = record.get('metadata', {})
        row = {key: record.get(key) for key in FIELDS}
        row.update(seconds=meta.get('seconds'), error=record.get('error', meta.get('error')),
                   **{key: meta.get(key) for key in METADATA})
        rows.append(row)
    pd.DataFrame(rows, columns=FIELDS+METADATA).to_csv(output/(prefix+'-observations.csv'), index=False)
    inventory = json.loads((root/'environment/runtime.json').read_text())
    harness = (root/'logs/harness-tests.log').read_text()
    smoke = campaign['smoke']['independent_science']
    captured = {f"{r['cell']}/{r['arm']}-r{r['repetition']}": canonical(dict(
                    capture=r['capture'], objective=float(r['metadata']['rss' if r['cell'].startswith('ols-') else 'll'])))
                for r in campaign['records'] if 'capture' in r}
    accounting_keys = ['jobnumber', 'taskid', 'hostname', 'qname', 'slots', 'failed', 'exit_status',
                       'start_time', 'end_time', 'ru_wallclock', 'cpu', 'maxvmem']
    references = summary[(summary.section == 'reference')]
    pdf = report/'fereg-scc-benchmark.pdf'
    pins = dict(pyfixest=manifest['policy']['pyfixest'], julia=manifest['policy'].get('julia'),
                freeze_sha256={name: digest(root/'environment'/name) for name in
                               [f'{arm}-freeze.txt' for arm in manifest['policy']['pyfixest']]},
                fixest_versions=sorted({r['metadata']['fixest_version'] for r in campaign['records']
                                        if r.get('metadata', {}).get('fixest_version')}))
    if 'julia' in manifest['policy']:
        pins['julia_manifest_sha256'] = digest(root/'environment/julia/Manifest.toml')
    metadata = dict(
        schema=2, run_id=context['run_id'], remote_root=context['remote_root'], purpose=manifest['policy']['purpose'],
        source_commit=context['source'], source_archive_sha256=context['source_sha256'],
        plugin_sha256=inventory['source/dist/fereg_linux.plugin'], plugin_origin=context.get('plugin', 'build'),
        packaged_runtime_sha256=json.loads((root/'input/source.json').read_text())['runtime_sha256'],
        comparators=pins, policy=manifest['policy'], configurations=manifest['cells'], bundles=manifest['bundles'],
        datasets=campaign['datasets'],
        accounting=[{k: r[k] for k in accounting_keys if k in r} for r in campaign['accounting']['jobs']],
        launchers=context.get('launcher_sha256', {}),
        stopped_tasks=context.get('stopped_tasks', []), task_jobs=context.get('task_jobs', {}),
        superseded_cells={cell: dict(value, statuses=dict(Counter(r['status'] for r in campaign['superseded_records']
                                                                if r['cell'] == cell)))
                          for cell, value in context.get('superseded_cells', {}).items()},
        scc_benchmark_tests=dict(count=int(re.findall(r'^Ran (\d+) tests', harness, re.M)[-1]),
                                 passed=harness.rstrip().endswith('OK')),
        host_isolation=hosts,
        node_reference=[dict(task=int(r.cell.rsplit('ref', 1)[1]), host=r.host, seconds=r.seconds,
                             seconds_min=r.seconds_min, seconds_max=r.seconds_max)
                        for r in references.itertuples()],
        validation=dict(planned_positions=len(campaign['records'])+len(campaign['superseded_records']),
                        arm_cells=len(summary),
                        eligible_arm_cells=int(summary.eligible.sum()),
                        convention_arm_cells=int((summary.agreement == 'convention').sum()),
                        statuses=dict(Counter(r['status'] for r in campaign['records'])),
                        native_fits=len(native), native_all_pass=not limited,
                        native_command_limits=sorted(limited),
                        exact_memory_positions=sum(bool(r.get('memory_exact')) for r in campaign['records']),
                        smoke_oracle_counts={a: dict(passed=sum(x['pass'] for x in smoke if x['arm'] == a),
                                                     total=sum(x['arm'] == a for x in smoke))
                                             for a in sorted({x['arm'] for x in smoke})}),
        smoke_independent_science=smoke, production_comparisons=campaign['comparisons'],
        production_comparison_ratio_convention='Per-fit ratio is fereg/comparator; summary CSV speedup is the '
                                               'median paired comparator/fereg ratio over agreeing repetitions.',
        capture_hash_encoding="SHA-256 of UTF-8 json.dumps({capture, objective}, sort_keys=True, "
                              "separators=(',', ':'), allow_nan=False). Objective is parsed as binary64.",
        capture_sha256=captured, artifacts=json.loads((root/'collection-sha256.json').read_text()),
        runtime_inventory_sha256=digest(root/'environment/runtime.json'), exporter_sha256=digest(Path(__file__)),
        report_pdf_sha256=digest(pdf), report_pdf_pages=len(re.findall(rb'/Type\s*/Page[^s]', pdf.read_bytes())),
        retained_csv_sha256={name: digest(output/name) for name in [prefix+'-summary.csv', prefix+'-observations.csv']},
        limitations=[
            'Descriptive Linux comparison on one fixed seed; three to five repetitions per configuration.',
            'Each design and model ran on one node; absolute times across designs and nodes also reflect node speed, '
            'recorded by the per-node reference cell.',
            'Agreement by convention removes one covariance scalar from absorbed degrees-of-freedom counting; '
            'strict eligibility and tolerances are unchanged.',
            'A command that reached the time limit was not attempted at larger sizes of its series.'])
    (output/(prefix+'-inputs.json')).write_text(json.dumps(metadata, indent=2, allow_nan=False)+'\n')
    return metadata


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path, help='collected campaign directory')
    parser.add_argument('--report', type=Path, required=True, help='rendered report directory')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--prefix', required=True, help='for example scc-20261028-beta8')
    args = parser.parse_args()
    result = export(args.root, args.report, args.output, args.prefix)
    print(json.dumps(dict(plugin=result['plugin_sha256'][:12], validation=result['validation']), indent=1))
