"""Preserve sanitized local development measurements without licensed logs."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import shutil
import statistics


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(2**20), b''):
            digest.update(block)
    return digest.hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + '\n')


def archive(root, output, labels):
    output.mkdir(parents=True, exist_ok=False)
    records, profiles, references, plugins = [], {}, {}, set()
    incomplete = []
    series = {}
    for label in labels:
        complete = root / f'{label}-complete.json'
        progress = root / f'{label}-progress.json'
        series[label] = dict(complete=complete.exists(), checks=json.loads(
            (complete if complete.exists() else progress).read_text())
            if complete.exists() or progress.exists() else None)
        for request in sorted((root/'runs').glob(f'{label}-*/request.json')):
            cell = json.loads(request.read_text())['cell']
            if not request.parent.name.startswith(f'{label}-{cell["case"]}-n'):
                continue
            if not (request.parent/'result.json').exists():
                incomplete.append(dict(run=request.parent.name, series=label,
                    cell=cell,
                    status='no_completed_result', request_sha256=sha(request)))
        for path in sorted((root/'runs').glob(f'{label}-*/result.json')):
            result = json.loads(path.read_text())
            run = path.parent
            if not run.name.startswith(f'{label}-{result["cell"]["case"]}-n'):
                continue
            variant = run.name.rsplit('-', 1)[-1]
            fingerprint = result['measurement_fingerprint']
            record = dict(run=run.name, series=label, variant=variant, cell=result['cell'],
                          status=result['status'], seconds=result['seconds'],
                          returncode=result['process_returncode'], sentinel=result['sentinel'],
                          input_sha256=result['input_sha256'], sample_sha256=result.get('sample_sha256'),
                          sample_rows=result.get('sample_rows'), peak_rss_gib=result['peak_rss_gib'],
                          result_sha256=sha(path),
                          scientific_files={p.name:sha(p) for p in sorted(run.glob('*.csv'))},
                          controller=fingerprint.get('controller'),
                          harness_sha256=fingerprint.get('harness_sha256'))
            if (run/'outputs.bin').exists():
                record['outputs_sha256'] = sha(run/'outputs.bin')
            if result['arm'] == 'fereg':
                plugins.add(variant)
                record['plugin'] = fingerprint['native']['plugin']
                runtime = {}
                if (run/'fereg_runtime.csv').exists():
                    with (run/'fereg_runtime.csv').open() as stream:
                        runtime = {int(r['row']):float(r['value']) for r in csv.DictReader(stream)
                                   if int(r['column']) == 2}
                else:
                    assert result['status']!='pass', 'successful native run lacks runtime receipt'
                if 21 in runtime:
                    record.update(input_seconds=runtime[19], native_seconds=runtime[20],
                                  native_allocation_peak_bytes=int(runtime[13]),
                                  outer_seconds=result['seconds']-runtime[21])
                trace = run/'stderr.txt'
                if trace.exists():
                    phases = [line for line in trace.read_text().splitlines()
                              if line.startswith('FEREG_PHASE|')]
                    if phases:
                        profiles[run.name] = dict(warmup_then_measured=phases)
            else:
                for field in ('fixest_archive_sha256', 'fixest_library'):
                    if field in fingerprint:
                        references.setdefault(field, {})[sha(path)] = fingerprint[field]
                profile = run/'profile.json'
                if profile.exists():
                    profiles[run.name] = json.loads(profile.read_text())
            records.append(record)
    assert records
    assert len({record['run'] for record in records}) == len(records), 'overlapping series selection'
    summary = []
    for label in labels:
        cells = {}
        for record in records:
            if record['series'] != label:
                continue
            key = json.dumps(record['cell'], sort_keys=True)
            cells.setdefault(key, []).append(record)
        for key, cell_records in sorted(cells.items()):
            variants = {}
            reference = {r['run'].rsplit('-',1)[0]:r for r in cell_records
                         if r['variant']=='fixest' and r['status']=='pass' and r['seconds']>0}
            for variant in sorted({r['variant'] for r in cell_records}):
                current = [r for r in cell_records if r['variant']==variant and r['status']=='pass']
                if not current:
                    continue
                values = {field:statistics.median(r[field] for r in current)
                          for field in ('seconds','input_seconds','native_seconds','outer_seconds',
                                        'native_allocation_peak_bytes','peak_rss_gib') if field in current[0]}
                pairs = [(r,reference[r['run'].rsplit('-',1)[0]]) for r in current
                         if r['run'].rsplit('-',1)[0] in reference]
                values.update(calls=len(current), minimum_seconds=min(r['seconds'] for r in current),
                              maximum_seconds=max(r['seconds'] for r in current))
                if pairs:
                    values.update(median_paired_fixest_ratio=statistics.median(a['seconds']/b['seconds'] for a,b in pairs),
                                  fixest_wins=sum(a['seconds']<b['seconds'] for a,b in pairs))
                variants[variant] = values
            summary.append(dict(series=label, cell=json.loads(key), variants=variants))
    identities = {variant:json.loads((root/f'identity-{variant}.json').read_text()) for variant in sorted(plugins)}
    for field, values in references.items():
        unique = {json.dumps(value,sort_keys=True) for value in values.values()}
        references[field] = [json.loads(value) for value in sorted(unique)]
    for name, value in [('measurements',records),('summary',summary),('series',series),
                        ('identities',identities),('references',references),('profiles',profiles),
                        ('incomplete',incomplete)]:
        write(output/f'{name}.json', value)
    (output/'harness').mkdir()
    for path in Path(__file__).parent.glob('*'):
        if path.suffix in ('.py','.json'):
            shutil.copyfile(path,output/'harness'/path.name)
    for path in root.glob('fixtures-*.json'):
        if any(path.name.startswith(f'fixtures-{variant}') for variant in plugins):
            shutil.copyfile(path,output/path.name)
    for name in ('acceptance-20260920.json','baseline-provenance.json'):
        if (root/name).exists():
            shutil.copyfile(root/name,output/name)
    write(output/'files.json',{str(p.relative_to(output)):sha(p) for p in sorted(output.rglob('*')) if p.is_file()})
    print('ARCHIVE_COMPLETE', len(records), 'calls', len(summary), 'cells')


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('root',type=Path);p.add_argument('output',type=Path)
    p.add_argument('labels',nargs='+')
    a = p.parse_args()
    archive(a.root,a.output,a.labels)
