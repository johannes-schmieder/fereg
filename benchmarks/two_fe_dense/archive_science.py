"""Portable small-graph science receipts, including failures and output vectors."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile


def sha(data): return hashlib.sha256(data).hexdigest()


def archive(roots, output, compact=False):
    repo = Path(__file__).resolve().parents[2]
    output.mkdir(parents=True, exist_ok=False)
    records, hashes, campaigns = [], {}, []
    objects = set()
    def portable(data):
        return data.replace(str(repo).encode(), b'<repository>').replace(b'/tmp/fereg-fixest-local-20260911/work/R', b'<reference-library>')
    with zipfile.ZipFile(output / 'evidence.zip', 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
        def object_reference(data, suffix):
            name = 'objects/' + sha(data) + suffix
            if name not in objects:
                bundle.writestr(name, data)
                objects.add(name)
            return name

        def add(root, path):
            relative = root.name + '/' + str(path.relative_to(root))
            raw = path.read_bytes(); clean = raw if path.suffix == '.bin' else portable(raw)
            entry = dict(original_sha256=sha(raw), portable_sha256=sha(clean))
            if compact and path.name == 'outputs.bin':
                entry['archive_member'] = object_reference(clean, '.bin')
                hashes[relative] = entry
                return
            if compact and path.name == 'result.json':
                result = json.loads(clean)
                fingerprint = json.dumps(result['measurement_fingerprint'], indent=2, sort_keys=True).encode()
                result['measurement_fingerprint'] = {'$archive_ref': object_reference(fingerprint, '.json')}
                clean = (json.dumps(result, indent=2, sort_keys=True) + '\n').encode()
                entry['archive_sha256'] = sha(clean)
                entry['encoding'] = 'externalized_measurement_fingerprint'
            hashes[relative] = entry
            bundle.writestr(relative, clean)
        for root in roots:
            root = root.resolve()
            for plan in sorted(root.glob('*-plan.json')):
                label = plan.name.removesuffix('-plan.json')
                complete = root / (label + '-complete.json')
                campaigns.append(dict(root=root.name, label=label, plan_sha256=sha(plan.read_bytes()),
                    completion=json.loads(complete.read_text()) if complete.exists() else None))
            for path in sorted(root.glob('*')):
                if path.is_file() and (path.suffix == '.json' or path.name.endswith('.stdout.txt')): add(root, path)
            for path in sorted((root / 'controller').glob('*')):
                if path.is_file() and path.suffix in ('.json', '.py', '.R', '.do'): add(root, path)
            for path in sorted((root / 'data').glob('*.json')): add(root, path)
            for run in sorted((root / 'runs').iterdir()):
                if not run.is_dir(): continue
                for path in sorted(run.iterdir()):
                    if path.suffix in ('.json', '.csv') or path.name == 'outputs.bin': add(root, path)
                if (run / 'result.json').exists():
                    result = json.loads((run / 'result.json').read_text())
                    records.append(dict(root=root.name, run=run.name, cell=result['cell'], arm=result['arm'],
                        status=result['status'], sentinel=result['sentinel'], process_returncode=result['process_returncode'],
                        input_sha256=result['input_sha256'], sample_sha256=result.get('sample_sha256'),
                        plugin=(result['measurement_fingerprint'].get('native') or {}).get('plugin')))
        for name in ('sparse-oracle-crosscheck.txt', 'sparse-oracle-crosscheck.json'):
            path = repo / 'target/ols-next-20260921' / name
            if path.exists(): add(path.parent, path)
    for name, data in [('measurements', records), ('campaigns', campaigns), ('archive-hashes', hashes)]:
        (output / (name + '.json')).write_bytes(portable((json.dumps(data, indent=2, sort_keys=True) + '\n').encode()))
    print('SMALL_GRAPH_SCIENCE_ARCHIVE', len(records), len(campaigns), flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('output', type=Path); p.add_argument('roots', nargs='+', type=Path)
    p.add_argument('--compact', action='store_true', help='store shared fingerprints and duplicate vectors once, with explicit hash references')
    a = p.parse_args(); archive(a.roots, a.output, a.compact)
