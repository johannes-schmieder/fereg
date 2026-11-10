#!/usr/bin/env python3
"""Qualify a revision campaign's smoke before production; write qualification.json.

Generalizes the November 6 receipt (repair-no-fe-v2/input/qualify_smoke_8480700.py)
with the same gates: successful setup and smoke accounting, the harness tests,
setup and smoke sentinels, complete coverage, exact native memory, host isolation,
both native profiles passing the independent oracle in every smoke case with clean
application logs (diagnostic refits included), numerical captures for completed
comparator fits, no retained row arrays, and comparator numerical comparisons with
their expected samples. Counts come from the manifest and the smoke cells instead
of being fixed. Run in a compute-node job or on collected evidence.
"""
from __future__ import annotations
import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

NATIVE = ['fereg', 'fereg-strict']
COMPARATORS = ['reghdfejl', 'reghdfejl-strict', 'glfixedeffects', 'glfixedeffects-strict']


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def qualify(root, accounting=None):
    sys.path.insert(0, str(root/'source/bench'))
    from campaign_collect import accounting_records
    read = lambda name: json.loads((root/name).read_text())
    context, smoke, manifest = read('context.json'), read('smoke/results.json'), read('manifest.json')
    accounts = {}
    for role in ['setup', 'smoke']:
        job = str(context[role+'_job'])
        raw = (accounting or {}).get(role) or subprocess.run(['qacct', '-j', job], check=True, capture_output=True,
                                                             text=True).stdout
        rows = accounting_records(raw)
        assert len(rows) == 1 and rows[0]['jobnumber'] == job and rows[0]['failed'] == '0' \
            and rows[0]['exit_status'] == '0', (role, rows)
        assert int(rows[0]['slots']) == context['resources'][role]['slots'], (role, rows[0]['slots'])
        accounts[role] = rows[0]
    harness = (root/'logs/harness-tests.log').read_text()
    assert re.search(r'^OK$', harness, re.M) and not re.search(r'^FAILED', harness, re.M)
    assert (root/'environment/COMPLETE').read_text().strip() == 'CAMPAIGN_SETUP_COMPLETE'
    assert (root/'smoke/COMPLETE').read_text().strip() == 'CAMPAIGN_TASK_COMPLETE'
    assert f"CAMPAIGN_JOB_COMPLETE job={context['smoke_job']} task=" in (root/'logs/smoke.sge').read_text()
    host = read('host-smoke/host.json')
    assert smoke['complete'] and smoke['coverage_complete'] and smoke['native_gate_pass'] and smoke['memory_exact']
    assert smoke['expected_fits'] == len(smoke['records'])
    assert host['isolation_ok'] and not host['errors'] and host['host']['job_id'] == str(context['smoke_job'])
    assert host['host']['hostname'].split('.')[0] == smoke['host'].split('.')[0] == accounts['smoke']['hostname'].split('.')[0]
    cells = {c['id']: c for c in smoke['cells']}
    expected = {(c, a) for c in cells for a in NATIVE}
    native = [x for x in smoke['records'] if x['arm'] in NATIVE]
    assert {(x['cell'], x['arm']) for x in native} == expected and len(native) == len(expected)
    assert {(x['cell'], x['arm']) for x in smoke['independent_science'] if x['arm'] in NATIVE and x['pass']} == expected
    logs = 0
    for x in native:
        assert x['status'] == 'ok' and x['process_rc'] == 0 and x['memory_exact'], (x['cell'], x['arm'])
        assert float(x['metadata']['rc']) == 0 and float(x['metadata']['complete']) == 1
        for y in [x]+([x['prediction_diagnostic']] if x.get('prediction_diagnostic') else []):
            assert y['status'] == 'ok' and y['process_rc'] == 0, (x['cell'], x['arm'], y['status'])
            log = (Path(y['directory'])/'command.log').read_text(errors='replace')
            assert not re.search(r'^r\([1-9][0-9]*\);', log, re.M) and 'end of do-file' in log, (x['cell'], x['arm'])
            logs += 1
    timeout = manifest['policy']['command_timeout_seconds']
    for x in smoke['records']:
        if x['status'] == 'error':
            # GLFixedEffectModels.jl fits no model without absorbed effects.
            assert cells[x['cell']]['effects'] == 0 and x['arm'].startswith('glfixedeffects'), (x['cell'], x['arm'])
        if x['status'] in ['ok', 'capture-error']:
            assert x.get('numerical_capture') and float(x['metadata']['seconds']) > 0
            assert int(x['command_timeout_seconds']) == timeout
            for name in ['coefficients.csv', 'covariance.csv', 'metadata.tsv', 'record.json']:
                assert (Path(x['directory'])/name).is_file(), (x['cell'], x['arm'], name)
        if 'directory' in x:
            assert not (Path(x['directory'])/'rows.csv').exists(), (x['cell'], x['arm'])
    comparators = {}
    for arm in COMPARATORS:
        rows = [x for x in smoke['numerical_comparisons'] if x['arm'] == arm and cells[x['cell']]['effects'] > 0]
        assert rows, arm
        for x in rows:
            assert x['coefficient_max_reference_se'] is not None and x['prediction_relative_rmse'] is not None
            different = arm.startswith('glfixedeffects') and cells[x['cell']]['design'] == 'gravity'
            assert x['same_sample'] != different, (arm, x['cell'])
            if not different:
                assert x['replayed_objective_difference_per_observation'] is not None
        comparators[arm] = dict(fits=len(rows), same_sample=sum(x['same_sample'] for x in rows),
                                max_coefficient_reference_se=max(x['coefficient_max_reference_se'] for x in rows),
                                max_prediction_relative_rmse=max(x['prediction_relative_rmse'] for x in rows),
                                different_samples=[x['cell'] for x in rows if not x['same_sample']])
    assert len(manifest['bundles']) == manifest['policy']['bundles']
    assert all(c['effects'] > 0 for c in manifest['cells'])
    assert not manifest['policy']['skip_larger_after_timeout']
    source = read('input/source.json')
    assert sha256(root/'input/source.tar') == source['archive_sha256'] == context['source_sha256']
    plugin = sha256(root/'source/dist/fereg_linux.plugin')
    if context.get('candidate_plugin_sha256'):
        assert plugin == context['candidate_plugin_sha256']
    receipt = dict(
        qualified=True, accounting=accounts, native_profile_passes={a: len(cells) for a in NATIVE},
        native_application_logs=logs, coverage=len(smoke['records']),
        statuses=dict(collections.Counter(x['status'] for x in smoke['records'])), host_isolation=True,
        smoke_host=smoke['host'], comparators=comparators, source_commit=source['commit'],
        source_archive_sha256=source['archive_sha256'], native_plugin_sha256=plugin,
        policy_sha256=sha256(root/'input/policy.json'), manifest_sha256=sha256(root/'manifest.json'),
        smoke_results_sha256=sha256(root/'smoke/results.json'), production_allocations=len(manifest['bundles']),
        production_blocks=len(manifest['cells']), production_fits=manifest['timed_fits'],
        diagnostic_fits=manifest.get('diagnostic_fits'))
    out = root/'qualification.json'
    if out.exists():
        raise RuntimeError('refusing to overwrite a qualification receipt')
    out.write_text(json.dumps(receipt, indent=2)+'\n')
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('root', type=Path)
    args = parser.parse_args()
    receipt = qualify(args.root)
    print(json.dumps({k: receipt[k] for k in ['native_profile_passes', 'coverage', 'statuses', 'comparators',
                                              'manifest_sha256', 'production_allocations', 'production_fits']}, indent=2))
