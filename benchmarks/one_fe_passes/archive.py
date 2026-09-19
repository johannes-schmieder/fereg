"""Archive measured outcomes, exact comparisons and source patches; no licensed logs."""
import argparse
import csv
import difflib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def main(root, output):
    root = root.resolve()
    output.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location('profile', REPO / 'benchmarks/one_fe_profile/run.py')
    profile = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(profile)
    rows, science, phases = [], [], []
    for path in sorted((root / 'runs').glob('*/result.json')):
        r = json.loads(path.read_text())
        name = path.parent.name
        variant = name.rsplit('-', 1)[1]
        baseline = path.parent.with_name(name.rsplit('-', 1)[0] + '-baseline')
        assert r['status'] == 'pass' and r['process_returncode'] == 0 and r['sentinel']
        check = profile.compare(baseline, path.parent)
        science.append(dict(run=name, comparison=check, exact_native_exports=variant != 'fixest', output_hashes={p.name: sha(p) for p in path.parent.glob('*.csv') if p.name != 'fereg_runtime.csv'}))
        row = dict(run=name, case=name.rsplit('-r', 1)[0], variant=variant,
                   repetition=int(name.rsplit('-r', 1)[1].split('-')[0]), seconds=r['seconds'],
                   input_sha256=r['input_sha256'], sample_sha256=r['sample_sha256'],
                   identity=r['measurement_fingerprint'], rss_gib=r['peak_rss_gib'])
        if variant != 'fixest':
            with (path.parent / 'fereg_runtime.csv').open() as f:
                rt = {int(x['row']): float(x['value']) for x in csv.DictReader(f) if x['column'] == '2'}
            row.update(input=rt[19], native=rt[20], preflight=rt[18], remaining=r['seconds']-rt[21], reported_native_peak=rt[13])
        rows.append(row)
        for line in (path.parent / 'stderr.txt').read_text().splitlines():
            if line.startswith('FEREG_PHASE|'):
                parts = line.split('|')
                values = {}
                for part in parts[2:]:
                    key, value = part.split('=', 1)
                    try:
                        values[key] = float(value)
                    except ValueError:
                        values[key] = value
                phases.append(dict(run=name, phase=parts[1], values=values))
    summary = []
    for case in sorted({r['case'] for r in rows}):
        values = [r for r in rows if r['case'] == case]
        baselines = {r['repetition']: r for r in values if r['variant'] == 'baseline'}
        variants = {}
        for name in sorted({r['variant'] for r in values}):
            current = [r for r in values if r['variant'] == name]
            fields = ['seconds', 'rss_gib'] + ([] if name == 'fixest' else ['input', 'native', 'reported_native_peak'])
            v = {field: statistics.median(r[field] for r in current) for field in fields}
            v.update(n=len(current), minimum=min(r['seconds'] for r in current), maximum=max(r['seconds'] for r in current),
                     paired_command_ratios=[r['seconds']/baselines[r['repetition']]['seconds'] for r in current],
                     paired_command_savings=[baselines[r['repetition']]['seconds']-r['seconds'] for r in current])
            variants[name] = v
        summary.append(dict(case=case, variants=variants))
    write(output / 'measurements.json', rows)
    write(output / 'summary.json', summary)
    write(output / 'phases.json', phases)
    kernel = []
    for k in [10, 50]:
        source = root / f'pipeline/kernel-k{k}.tsv'
        raw = list(csv.DictReader(source.open(), delimiter='\t'))
        assert len(raw) == 88 and len({r['hash'] for r in raw}) == 1
        for mode in range(8):
            times = [float(r['seconds']) for r in raw if int(r['mode']) == mode and int(r['repeat']) > 0]
            kernel.append(dict(k=k, mode=mode, median=statistics.median(times), times=times))
        shutil.copyfile(source, output / source.name)
    write(output / 'kernel-summary.json', kernel)
    tests = (root / 'focused-tests.log').read_text()
    test_names = ['fresh_one_effect_nonfinite_validation_precedes_weights_and_allocation',
                  'one_way_projection_is_exact_and_invalid_shapes_and_configurations_fail',
                  'ols_norms_are_scaled_and_bit_stable_across_worker_counts']
    assert all(re.search(r'test .*::' + name + r' \.\.\. ok', tests) for name in test_names)
    assert 'PIPELINE_GUARD_PASS' in (root / 'pipeline/guard.log').read_text()
    assert 'PIPELINE_BRIDGE_GUARD_PASS' in (root / 'pipelinefit/guard.log').read_text()
    write(output / 'science.json', dict(accepted_command_calls=len(rows), checks=science,
        small_dummy_oracles=json.loads((root / 'smoke.json').read_text()),
        focused_rust_tests=test_names, focused_test_log_sha256=sha(root / 'focused-tests.log'),
        licensed_fixtures={v: json.loads((root / f'fixtures-{v}.json').read_text()) for v in ['final', 'pipelinefit']},
        pipeline_ubsan='pass', pipeline_bridge_ubsan='pass'))
    # Build.py formats the isolated sources. Normalize a separate reference to
    # show only prototype changes; never rewrite a measured source snapshot.
    reference = root / 'archive-reference'
    reference.mkdir()
    changed = ['crates/fereg-core/src/ols/prepared_fit.rs', 'crates/fereg-core/src/ols/prepared_rhs.rs',
               'crates/fereg-core/src/fe/borrowed_map.rs', 'plugin/stata_bridge.c']
    for relative in changed:
        destination = reference / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / 'baseline' / relative, destination)
        if destination.suffix == '.rs':
            subprocess.run(['rustfmt', '--edition', '2021', str(destination)], check=True)
    identities = {}
    for name in ['baseline', 'direct', 'overlap', 'combined', 'coloverlap', 'parallelcheck', 'streaminit', 'final', 'fastfinal', 'pipelinefit']:
        source = root / name
        patches = []
        for relative in changed:
            p = source / relative
            if p.exists():
                patches.extend(difflib.unified_diff((reference / relative).read_text().splitlines(True), p.read_text().splitlines(True), fromfile='a/' + relative, tofile='b/' + relative))
        if name != 'baseline':
            (output / (name + '.patch')).write_text(''.join(patches))
        identity_path = root / ('identity-' + name + '.json')
        identity = json.loads(identity_path.read_text()) if identity_path.exists() else {}
        identity['plugin'] = sha(source / 'dist/fereg_macos.plugin')
        identity['source_files'] = {str(p.relative_to(source)): sha(p) for folder in ['crates', 'plugin', 'stata'] for p in (source / folder).rglob('*') if p.is_file() and p.suffix in ['.rs', '.c', '.h', '.toml', '.ado', '.mata']}
        if name in ['fastfinal', 'pipelinefit']:
            identity['rust_source_variant'] = 'final'
        identities[name] = identity
    write(output / 'identities.json', dict(base_commit='55c19cb679d675194c6baed74b09279e1f026b16', variants=identities,
        controller={p.name: sha(p) for p in (root / 'controller').glob('*') if p.is_file()},
        harness={p.name: sha(p) for p in (REPO / 'benchmarks/one_fe_passes').glob('*') if p.is_file()},
        pipeline=json.loads((root / 'pipeline/identity.json').read_text()),
        production_borrowed_map_sha256=sha(REPO / changed[2])))
    print('ARCHIVE_PASS', len(rows), flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('root', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    main(a.root, a.output)
